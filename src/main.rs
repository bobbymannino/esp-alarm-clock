mod speaker;

use std::process::ExitCode;

use esp_idf_svc::hal::peripherals::Peripherals;

fn main() -> ExitCode {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let Ok(peripherals) = Peripherals::take() else {
        log::error!("Failed to take peripherals");
        return ExitCode::FAILURE;
    };

    let Ok(mut speaker) = speaker::Speaker::new(peripherals.ledc.timer0, peripherals.ledc.channel0, peripherals.pins.gpio17) else {
        log::error!("Failed to create speaker");
        return ExitCode::FAILURE;
    };

    let Ok(_) = speaker.alarm(None) else {
        log::error!("Failed to make alarm sound");
        return ExitCode::FAILURE;
    };

    ExitCode::SUCCESS
}
