use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{Input, InputPin, PinDriver, Pull},
};

use crate::error::Result;

/// Gap between reads of the inputs
const GAP_MS: u32 = 5;

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

    pub fn read(self) -> ! {
        loop {
            let is_anticlockwise = self.s1.is_low();
            let is_clockwise = self.s2.is_low();
            let is_pressed = self.key.is_low();

            log::info!(
                "anticlockwise: {} clockwise: {} pressed: {}",
                is_anticlockwise,
                is_clockwise,
                is_pressed
            );

            FreeRtos::delay_ms(GAP_MS);
        }
    }
}
