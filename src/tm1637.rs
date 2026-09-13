use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use chrono::Timelike as _;
use esp_idf_svc::hal::{
    delay::Ets,
    gpio::{InputOutput, InputPin, Output, OutputPin, PinDriver, Pull},
};

use crate::{
    error::{Error, Result},
    time,
};

pub const DIGITS: [u8; 10] = [
    0b0011_1111, // 0
    0b0000_0110, // 1
    0b0101_1011, // 2
    0b0100_1111, // 3
    0b0110_0110, // 4
    0b0110_1101, // 5
    0b0111_1101, // 6
    0b0000_0111, // 7
    0b0111_1111, // 8
    0b0110_1111, // 9
];
pub const COLON: u8 = 0b1000_0000;
pub const BLANK: u8 = 0b0000_0000;

const BIT_US: u32 = 10;

/// The outer segments, clockwise from the top, making up one turn of the loading spinner.
const SPINNER_FRAMES: [u8; 6] = [
    0b0000_0001, // top
    0b0000_0010, // top right
    0b0000_0100, // bottom right
    0b0000_1000, // bottom
    0b0001_0000, // bottom left
    0b0010_0000, // top left
];
/// How long each frame of the loading spinner is shown for.
const SPINNER_FRAME: Duration = Duration::from_millis(100);
/// How long a worker waits after a tick it could not time against the clock.
const FALLBACK_TICK: Duration = Duration::from_secs(1);
/// Stack size, in bytes, of the thread a display worker runs on.
const WORKER_STACK_SIZE: usize = 4 * 1024;

/// Number of digits on the display.
pub const DIGIT_COUNT: usize = 4;
/// The brightest the display can be set to.
pub const MAX_BRIGHTNESS: u8 = 7;

/// Write to the display memory, auto incrementing the address after each byte.
const CMD_WRITE_AUTO: u8 = 0b0100_0000;
/// Set the address the following data is written to; OR with the address.
const CMD_ADDRESS: u8 = 0b1100_0000;
/// Turn the display on; OR with the brightness (0-7).
const CMD_DISPLAY_ON: u8 = 0b1000_1000;
/// Turn the display off, leaving the display memory untouched.
const CMD_DISPLAY_OFF: u8 = 0b1000_0000;

/// A TM1637 driven 4 digit seven segment display.
pub struct Tm1637<'d> {
    clk: PinDriver<'d, Output>,
    dio: PinDriver<'d, InputOutput>,
    brightness: u8,
    /// What the display memory currently holds, or `None` when it is unknown
    /// because a write failed part way through.
    shown: Option<[u8; DIGIT_COUNT]>,
}

impl<'d> Tm1637<'d> {
    /// Creates a new [`Tm1637`] with a blank display at full brightness.
    pub fn new(clk: impl OutputPin + 'd, dio: impl InputPin + OutputPin + 'd) -> Result<Self> {
        let mut clk = PinDriver::output(clk)?;
        let mut dio = PinDriver::input_output_od(dio, Pull::Floating)?;

        clk.set_high()?;
        dio.set_high()?;

        let mut display = Self {
            clk,
            dio,
            brightness: MAX_BRIGHTNESS,
            shown: None,
        };

        display.segments([BLANK; DIGIT_COUNT])?;
        display.off()?;

        Ok(display)
    }

    /// Turn the display on at the brightness last given to [`Self::brightness`].
    pub fn on(&mut self) -> Result<()> {
        self.command(CMD_DISPLAY_ON | self.brightness)
    }

    /// Turn the display off without clearing what is shown on it.
    pub fn off(&mut self) -> Result<()> {
        self.command(CMD_DISPLAY_OFF)
    }

    /// Set the brightness of the display and turn it on.
    ///
    /// # Arguments
    ///
    /// * `brightness` - `0` (dimmest) to [`MAX_BRIGHTNESS`], clamped.
    pub fn brightness(&mut self, brightness: u8) -> Result<()> {
        self.brightness = brightness.min(MAX_BRIGHTNESS);

        self.on()
    }

    /// Show the raw segments of each digit, left to right.
    ///
    /// Only the second digit has the colon wired up, so [`COLON`] is ignored on
    /// every other digit.
    ///
    /// Only the digits that differ from what is already on the display are sent,
    /// so a call that changes nothing costs nothing on the bus and a blinking
    /// colon costs a single byte.
    pub fn segments(&mut self, segments: [u8; DIGIT_COUNT]) -> Result<()> {
        let Some((address, count)) = self.pending(segments) else {
            return Ok(());
        };

        // Anything from here on can fail mid transfer, leaving the display
        // memory as neither the old nor the new value.
        self.shown = None;

        self.command(CMD_WRITE_AUTO)?;

        self.start()?;
        let written: Result<()> = (|| {
            self.write_byte(CMD_ADDRESS | u8::try_from(address).unwrap_or(0))?;

            for segment in segments.iter().skip(address).take(count) {
                self.write_byte(*segment)?;
            }

            Ok(())
        })();
        self.stop()?;
        written?;

        self.shown = Some(segments);

        Ok(())
    }

    /// The address and length of the run of digits that differ from what the
    /// display is showing, or `None` if it is already showing `segments`.
    fn pending(&self, segments: [u8; DIGIT_COUNT]) -> Option<(usize, usize)> {
        let Some(shown) = self.shown else {
            return Some((0, DIGIT_COUNT));
        };

        let mut range: Option<(usize, usize)> = None;
        for (index, (old, new)) in shown.into_iter().zip(segments).enumerate() {
            if old != new {
                range = Some(range.map_or((index, index), |(first, _)| (first, index)));
            }
        }

        range.map(|(first, last)| (first, last.saturating_sub(first).saturating_add(1)))
    }

    /// Send a single command byte, framed by its own start and stop.
    fn command(&mut self, command: u8) -> Result<()> {
        self.start()?;
        let written = self.write_byte(command);
        self.stop()?;

        written
    }

    /// Pull the data line low while the clock is high to open a transfer.
    fn start(&mut self) -> Result<()> {
        self.clk.set_high()?;
        self.dio.set_high()?;
        Ets::delay_us(BIT_US);

        self.dio.set_low()?;
        Ets::delay_us(BIT_US);

        Ok(())
    }

    /// Release the data line while the clock is high to close a transfer.
    fn stop(&mut self) -> Result<()> {
        self.clk.set_low()?;
        self.dio.set_low()?;
        Ets::delay_us(BIT_US);

        self.clk.set_high()?;
        Ets::delay_us(BIT_US);

        self.dio.set_high()?;
        Ets::delay_us(BIT_US);

        Ok(())
    }

    /// Clock out a byte, least significant bit first, then wait for the ACK the
    /// TM1637 gives by pulling the data line low on the ninth clock.
    fn write_byte(&mut self, byte: u8) -> Result<()> {
        let mut byte = byte;

        for _ in 0..8 {
            self.clk.set_low()?;
            Ets::delay_us(BIT_US);

            if byte & 1 == 1 {
                self.dio.set_high()?;
            } else {
                self.dio.set_low()?;
            }
            Ets::delay_us(BIT_US);

            self.clk.set_high()?;
            Ets::delay_us(BIT_US);

            byte = byte.wrapping_shr(1);
        }

        self.clk.set_low()?;
        // Release the line so the TM1637 can drive it.
        self.dio.set_high()?;
        Ets::delay_us(BIT_US);

        self.clk.set_high()?;
        Ets::delay_us(BIT_US);
        let acked = self.dio.is_low();

        self.clk.set_low()?;
        Ets::delay_us(BIT_US);

        if !acked {
            return Err(Error::NotAcknowledged);
        }

        Ok(())
    }

    /// Given a `u8` digit value, returns the corresponding digit segment pattern.
    fn digit_from_u8(num: u8) -> u8 {
        DIGITS.get(usize::from(num)).copied().unwrap_or(0)
    }

    /// Display a time. Does not turn the screen on or set the brightness.
    pub fn time(&mut self, hour: u8, minute: u8, colon: bool) -> Result<()> {
        let d1 = Self::digit_from_u8(hour / 10);
        let mut d2 = Self::digit_from_u8(hour % 10);
        if colon {
            d2 |= COLON;
        }
        let d3 = Self::digit_from_u8(minute / 10);
        let d4 = Self::digit_from_u8(minute % 10);

        self.segments([d1, d2, d3, d4])?;

        Ok(())
    }
}

impl Tm1637<'static> {
    /// Clear the display, turn it on and spin a single bar around the last
    /// digit until [`Spinner::stop`] is called.
    ///
    /// The display is driven from a dedicated thread, so the caller is free to
    /// go and do the slow work the spinner is there to cover. Only the digit
    /// the bar is drawn on is rewritten each frame.
    ///
    /// # Returns
    ///
    /// - `Ok`: A handle that gives the display back when stopped.
    /// - `Err`: An error if the display could not be cleared or the thread could not be spawned.
    ///
    /// # Errors
    ///
    /// - `Esp`: A GPIO call failed.
    /// - `NotAcknowledged`: The TM1637 did not acknowledge a byte.
    /// - `Io`: The thread could not be spawned.
    pub fn spinner(mut self) -> Result<Spinner> {
        self.segments([BLANK; DIGIT_COUNT])?;
        self.on()?;

        let mut frames = SPINNER_FRAMES.into_iter().cycle();

        Worker::spawn(self, move |display| {
            let frame = frames.next().unwrap_or(BLANK);

            display
                .segments([BLANK, BLANK, BLANK, frame])
                .inspect_err(|err| log::error!("Spinner failed to set segments: {err}"))
                .ok()?;

            Some(SPINNER_FRAME)
        })
        .map(|worker| Spinner { worker })
    }

    /// Displays the current time and keeps it up to date on a separate thread.
    ///
    /// The thread wakes on the second boundary rather than every second from
    /// whenever it started, so the colon stays in step with the clock instead of
    /// drifting, and only the digits that actually changed are sent.
    ///
    /// # Errors
    ///
    /// - `Esp`: A GPIO call failed.
    /// - `NotAcknowledged`: The TM1637 did not acknowledge a byte.
    /// - `Io`: The thread could not be spawned.
    pub fn continuous_time(mut self) -> Result<ContinuousTime> {
        self.segments([BLANK; DIGIT_COUNT])?;
        self.on()?;

        Worker::spawn(self, |display| {
            let Ok(now) = time::now().inspect_err(|err| log::error!("Failed to get current time: {err}")) else {
                // Keep the clock running, the next tick may well read fine.
                return Some(FALLBACK_TICK);
            };

            let (hour, minute) = time::hour_minute(now);
            // Taken from the clock rather than toggled, so a missed tick cannot
            // leave the colon blinking out of phase with the seconds.
            let colon = now.second() % 2 == 0;

            if let Err(err) = display.time(hour, minute, colon) {
                log::error!("Failed to set continuous time: {err}");
            }

            Some(time::until_next_second(now))
        })
        .map(|worker| ContinuousTime { worker })
    }
}

/// A worker thread drawing to the display it owns until it is stopped.
struct Worker {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<Tm1637<'static>>,
}

impl Worker {
    /// Run `tick` on its own thread, waiting for the [`Duration`] it returns
    /// between calls, until it returns `None` or the worker is stopped.
    fn spawn(
        mut display: Tm1637<'static>,
        mut tick: impl FnMut(&mut Tm1637<'static>) -> Option<Duration> + Send + 'static,
    ) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);

        let handle = thread::Builder::new().stack_size(WORKER_STACK_SIZE).spawn(move || {
            while !thread_stop.load(Ordering::Acquire) {
                let Some(wait) = tick(&mut display) else {
                    break;
                };

                park(&thread_stop, wait);
            }

            display
        })?;

        Ok(Self { stop, handle })
    }

    /// Stop the worker, blank the display and hand it back.
    fn stop(self) -> Result<Tm1637<'static>> {
        self.stop.store(true, Ordering::Release);
        // Cut the current wait short rather than letting it run out.
        self.handle.thread().unpark();

        let mut display = self.handle.join().map_err(|_| Error::WorkerPanicked)?;

        display.segments([BLANK; DIGIT_COUNT])?;

        Ok(display)
    }
}

/// Sleep for `timeout`, returning early once `stop` is set.
///
/// [`thread::park_timeout`] can wake on its own, so the remaining time is
/// measured rather than assumed.
fn park(stop: &AtomicBool, timeout: Duration) {
    let start = Instant::now();
    let mut remaining = timeout;

    while !remaining.is_zero() && !stop.load(Ordering::Acquire) {
        thread::park_timeout(remaining);

        remaining = timeout.saturating_sub(start.elapsed());
    }
}

/// A running clock, holding the display it is drawn on.
pub struct ContinuousTime {
    worker: Worker,
}

impl ContinuousTime {
    /// Stop the clock, blank the display and hand it back.
    ///
    /// # Errors
    ///
    /// - `WorkerPanicked`: The worker thread panicked, losing the display.
    /// - `Esp`: A GPIO call failed while blanking the display.
    /// - `NotAcknowledged`: The TM1637 did not acknowledge a byte.
    pub fn stop(self) -> Result<Tm1637<'static>> {
        self.worker.stop()
    }
}

/// A running loading spinner, holding the display it is drawn on.
pub struct Spinner {
    worker: Worker,
}

impl Spinner {
    /// Stop the spinner, blank the display and hand it back.
    ///
    /// # Errors
    ///
    /// - `WorkerPanicked`: The spinner thread panicked, losing the display.
    /// - `Esp`: A GPIO call failed while blanking the display.
    /// - `NotAcknowledged`: The TM1637 did not acknowledge a byte.
    pub fn stop(self) -> Result<Tm1637<'static>> {
        self.worker.stop()
    }
}
