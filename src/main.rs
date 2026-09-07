use std::process::ExitCode;

use esp_idf_svc::hal::{
    delay::FreeRtos,
    ledc::{LedcDriver, LedcTimerDriver, config::TimerConfig},
    peripherals::Peripherals,
    units::Hertz,
};

fn main() -> ExitCode {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let Ok(peripherals) = Peripherals::take() else {
        log::error!("Failed to take peripherals");
        return ExitCode::FAILURE;
    };

    let frequency = Hertz::from(440);
    let timer = LedcTimerDriver::new(peripherals.ledc.timer0, &TimerConfig::new().frequency(frequency)).unwrap();

    let mut channel = LedcDriver::new(peripherals.ledc.channel0, timer, peripherals.pins.gpio17).unwrap();

    let steps = 10;
    let max_duty = channel.get_max_duty();
    let duty_step = max_duty / 10;
    let mut duty: u32 = duty_step;

    for _ in 0..steps {
        channel.set_duty(duty).unwrap();
        log::info!("beep");
        FreeRtos::delay_ms(500);

        channel.set_duty(0).unwrap();
        duty += duty_step;
        FreeRtos::delay_ms(500);
    }

    channel.set_duty(max_duty).unwrap();
    FreeRtos::delay_ms(500);
    channel.set_duty(duty_step).unwrap();
    FreeRtos::delay_ms(500);

    ExitCode::SUCCESS
}
