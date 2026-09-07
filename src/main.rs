mod speaker;

use std::process::ExitCode;

use esp_idf_svc::hal::{delay::FreeRtos, peripherals::Peripherals};

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

    for _ in 0..10 {
        let Ok(_) = speaker.volume(100) else {
            log::error!("Failed to set volume to 100%");
            return ExitCode::FAILURE;
        };
        FreeRtos::delay_ms(250);
        let Ok(_) = speaker.volume(0) else {
            log::error!("Failed to set volume to 0%");
            return ExitCode::FAILURE;
        };
        FreeRtos::delay_ms(250);
    }

    ExitCode::SUCCESS
}
