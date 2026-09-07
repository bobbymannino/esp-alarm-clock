use anyhow::Result;
use esp_idf_svc::hal::{
    gpio::OutputPin,
    ledc::{LedcChannel, LedcDriver, LedcTimer, LedcTimerDriver, SpeedMode, config::TimerConfig},
    units::Hertz,
};

pub struct Speaker<'d, S: SpeedMode> {
    /// Held so the timer keeps running for as long as the speaker exists;
    /// dropping it resets the timer and silences the channel.
    _timer: LedcTimerDriver<'d, S>,
    channel: LedcDriver<'d>,
    /// Duty cycle that corresponds to 100% volume, i.e. a 50% duty square wave.
    full_duty: u32,
}

impl<'d, S: SpeedMode> Speaker<'d, S> {
    /// Creates a new [`Speaker`] at 2KHz.
    pub fn new<T: LedcTimer<SpeedMode = S> + 'd, C: LedcChannel<SpeedMode = S> + 'd>(
        timer: T,
        channel: C,
        pin: impl OutputPin + 'd,
    ) -> Result<Self> {
        let timer_config = TimerConfig::new().frequency(Hertz::from(2_000));
        let timer = LedcTimerDriver::new(timer, &timer_config)?;

        let channel = LedcDriver::new(channel, &timer, pin)?;

        let full_duty = channel.get_max_duty() / 2;

        Ok(Self {
            _timer: timer,
            channel,
            full_duty,
        })
    }

    /// Set the volume of the speaker to the given percentage.
    pub fn volume(&mut self, percent: u8) -> Result<()> {
        let percent = u32::from(percent.min(100));

        self.channel.set_duty(self.full_duty * percent / 100)?;

        Ok(())
    }
}
