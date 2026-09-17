use std::{
    num::NonZeroU32,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use esp_idf_svc::hal::{
    delay::{BLOCK, TickType},
    gpio::{Input, InputPin, InterruptType, PinDriver, Pull},
    task::notification::Notification,
};

use crate::error::Result;

/// Stack size, in bytes, of the thread the encoder is watched on.
const WORKER_STACK_SIZE: usize = 4 * 1024;

/// How long the key has to hold a level before it counts as pressed or released.
const DEBOUNCE: Duration = Duration::from_millis(20);

/// Level of both rotary contacts while the dial rests in a detent, as pulled up.
const REST: u8 = 0b11;

/// Steps between two readings of the rotary contacts, indexed by the previous
/// then the current `s1 << 1 | s2` level. Transitions that skip a state are
/// contact bounce and count as nothing.
const STEPS: [[i8; 4]; 4] = [[0, -1, 1, 0], [1, 0, 0, -1], [-1, 0, 0, 1], [0, 1, -1, 0]];

/// Something the user did to the dial.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Anticlockwise,
    Clockwise,
    Pressed,
    Released,
}

pub struct Ec11<'d> {
    s1: PinDriver<'d, Input>,
    s2: PinDriver<'d, Input>,
    key: PinDriver<'d, Input>,
}

impl<'d> Ec11<'d> {
    pub fn new(s1: impl InputPin + 'd, s2: impl InputPin + 'd, key: impl InputPin + 'd) -> Result<Self> {
        let s1 = PinDriver::input(s1, Pull::Up)?;
        let s2 = PinDriver::input(s2, Pull::Up)?;
        let key = PinDriver::input(key, Pull::Up)?;

        Ok(Self { s1, s2, key })
    }
}

impl Ec11<'static> {
    /// Watches the dial on its own thread, sending an [`Event`] each time it is
    /// turned a detent or its key changes.
    ///
    /// The thread sleeps until a pin interrupt wakes it, so nothing is polled.
    /// It stops once the returned [`Receiver`] is dropped and the next event
    /// fails to send.
    ///
    /// # Returns
    ///
    /// - `Ok`: The receiving end of the events.
    /// - `Err`: An error if the thread could not be spawned.
    ///
    /// # Errors
    ///
    /// - `Io`: The thread could not be spawned.
    pub fn events(self) -> Result<Receiver<Event>> {
        let (sender, receiver) = mpsc::channel();

        thread::Builder::new().stack_size(WORKER_STACK_SIZE).spawn(move || {
            if let Err(err) = self.watch(&sender) {
                log::error!("Stopped watching the dial: {err}");
            }
        })?;

        Ok(receiver)
    }

    /// Sends events until the receiver is dropped.
    fn watch(mut self, sender: &Sender<Event>) -> Result<()> {
        // Must be made on the thread that waits on it, and outlive the drivers
        // below so no interrupt can notify a task that has gone.
        let notification = Notification::new();

        for pin in [&mut self.s1, &mut self.s2, &mut self.key] {
            let notifier = notification.notifier();

            pin.set_interrupt_type(InterruptType::AnyEdge)?;
            // SAFETY: Notifying a task is one of the few calls allowed from an
            // ISR, and the notifier cannot outlive this thread's task.
            unsafe {
                pin.subscribe(move || {
                    notifier.notify_and_yield(NonZeroU32::MIN);
                })?;
            }
            pin.enable_interrupt()?;
        }

        let mut rotary = self.rotary();
        let mut steps: i8 = 0;

        let mut key = self.key.is_low();
        let mut reported_key = key;
        let mut key_changed_at = Instant::now();

        loop {
            // Wake again once the key has settled if it is still bouncing.
            let timeout = if key == reported_key {
                BLOCK
            } else {
                TickType::from(DEBOUNCE.saturating_sub(key_changed_at.elapsed())).ticks()
            };
            notification.wait(timeout);

            // Each interrupt disables itself, so enable them again before
            // reading to catch any edge that lands while the levels are read.
            self.s1.enable_interrupt()?;
            self.s2.enable_interrupt()?;
            self.key.enable_interrupt()?;

            let current = self.rotary();
            let step = STEPS
                .get(usize::from(rotary))
                .and_then(|row| row.get(usize::from(current)))
                .copied()
                .unwrap_or_default();
            steps = steps.saturating_add(step);
            rotary = current;

            // Only count a turn once the dial is back in a detent, so bouncing
            // between two states cancels itself out.
            if rotary == REST {
                let event = match steps {
                    ..=-2 => Some(Event::Anticlockwise),
                    2.. => Some(Event::Clockwise),
                    _ => None,
                };
                steps = 0;

                if let Some(event) = event
                    && sender.send(event).is_err()
                {
                    return Ok(());
                }
            }

            let pressed = self.key.is_low();
            if pressed != key {
                key = pressed;
                key_changed_at = Instant::now();
            }

            if key != reported_key && key_changed_at.elapsed() >= DEBOUNCE {
                reported_key = key;
                let event = if key { Event::Pressed } else { Event::Released };

                if sender.send(event).is_err() {
                    return Ok(());
                }
            }
        }
    }

    /// Level of the rotary contacts as `s1 << 1 | s2`.
    fn rotary(&self) -> u8 {
        u8::from(self.s1.is_high()) << 1 | u8::from(self.s2.is_high())
    }
}
