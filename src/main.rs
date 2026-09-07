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

    let timer = LedcTimerDriver::new(peripherals.ledc.timer0, &TimerConfig::new().frequency(Hertz::from(2_000))).unwrap();
    let mut channel = LedcDriver::new(peripherals.ledc.channel0, &timer, peripherals.pins.gpio17).unwrap();
    let duty = channel.get_max_duty() / 2;

    for _ in 0..10 {
        channel.set_duty(duty).unwrap();
        FreeRtos::delay_ms(250);
        channel.set_duty(0).unwrap();
        FreeRtos::delay_ms(250);
    }

    ExitCode::SUCCESS
}
