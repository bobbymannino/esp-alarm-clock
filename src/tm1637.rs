use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use esp_idf_svc::hal::{
    delay::Ets,
    gpio::{InputOutput, InputPin, Output, OutputPin, PinDriver, Pull},
};

use crate::error::{Error, Result};

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

const BIT_US: u32 = 2;

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
/// Stack size, in bytes, of the thread the loading spinner runs on.
const SPINNER_STACK_SIZE: usize = 4 * 1024;

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
    pub fn segments(&mut self, segments: [u8; DIGIT_COUNT]) -> Result<()> {
        self.start()?;
        let written = self.write_byte(CMD_WRITE_AUTO);
        self.stop()?;
        written?;

        self.start()?;
        let written = (|| {
            self.write_byte(CMD_ADDRESS)?;

            for segment in segments {
                self.write_byte(segment)?;
            }

            Ok(())
        })();
        self.stop()?;

        written
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
    /// go and do the slow work the spinner is there to cover.
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

        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);

        let handle = thread::Builder::new().stack_size(SPINNER_STACK_SIZE).spawn(move || {
            for frame in SPINNER_FRAMES.into_iter().cycle() {
                if thread_stop.load(Ordering::Relaxed) {
                    break;
                }

                if let Err(err) = self.segments([BLANK, BLANK, BLANK, frame]) {
                    log::error!("Spinner failed to set segments: {err}");
                    break;
                }

                thread::sleep(SPINNER_FRAME);
            }

            self
        })?;

        Ok(Spinner { stop, handle })
    }
}

/// A running loading spinner, holding the display it is drawn on.
pub struct Spinner {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<Tm1637<'static>>,
}

impl Spinner {
    /// Stop the spinner, blank the display and hand it back.
    ///
    /// Blocks for up to one [`SPINNER_FRAME`] while the thread finishes the
    /// frame it is on.
    ///
    /// # Errors
    ///
    /// - `WorkerPanicked`: The spinner thread panicked, losing the display.
    pub fn stop(self) -> Result<Tm1637<'static>> {
        self.stop.store(true, Ordering::Relaxed);

        let mut display = self.handle.join().map_err(|_| Error::WorkerPanicked)?;

        display.segments([BLANK; DIGIT_COUNT])?;

        Ok(display)
    }
}
