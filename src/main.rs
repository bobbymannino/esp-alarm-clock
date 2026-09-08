mod speaker;
mod tm1637;

use std::{process::ExitCode, time::Duration};

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

    let Ok(()) = speaker.alarm(Some(Duration::from_secs(3))) else {
        log::error!("Failed to make alarm sound");
        return ExitCode::FAILURE;
    };

    let Ok(mut display) = tm1637::Tm1637::new(peripherals.pins.gpio18, peripherals.pins.gpio19) else {
        log::error!("Failed to create TM1637 display");
        return ExitCode::FAILURE;
    };

    let Ok(()) = display.brightness(3) else {
        log::error!("Failed to set brightness");
        return ExitCode::FAILURE;
    };

    for num in 0..10_000 {
        let n1 = tm1637::DIGITS.get((num / 1000) % 10).copied().unwrap_or(0);
        let mut n2 = tm1637::DIGITS.get((num / 100) % 10).copied().unwrap_or(0);
        if (num / 100) % 10 > 4 {
            n2 |= tm1637::COLON;
        }
        let n3 = tm1637::DIGITS.get((num / 10) % 10).copied().unwrap_or(0);
        let n4 = tm1637::DIGITS.get(num % 10).copied().unwrap_or(0);
        let Ok(()) = display.segments([n1, n2, n3, n4]) else {
            log::error!("Failed to write segments");
            return ExitCode::FAILURE;
        };
        FreeRtos::delay_ms(1);
    }

    ExitCode::SUCCESS
}
