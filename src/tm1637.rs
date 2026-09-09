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

    /// Display a time.
    pub fn time(&mut self, hour: u8, minute: u8, colon: bool) -> Result<()> {
        let d1 = DIGITS.get(usize::from(hour / 10)).copied().unwrap_or(0);
        let mut d2 = DIGITS.get(usize::from(hour % 10)).copied().unwrap_or(0);
        if colon {
            d2 |= COLON;
        }
        let d3 = DIGITS.get(usize::from(minute / 10)).copied().unwrap_or(0);
        let d4 = DIGITS.get(usize::from(minute % 10)).copied().unwrap_or(0);

        self.segments([d1, d2, d3, d4])?;

        Ok(())
    }
}
