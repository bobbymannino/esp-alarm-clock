mod error;
mod speaker;
mod tm1637;
mod wifi;

use std::process::ExitCode;

use esp_idf_svc::hal::peripherals::Peripherals;

fn main() -> ExitCode {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let Ok(peripherals) = Peripherals::take() else {
        log::error!("Failed to take peripherals");
        return ExitCode::FAILURE;
    };

    let wifi = match (option_env!("WIFI_SSID"), option_env!("WIFI_PASSWORD")) {
        (Some(ssid), Some(password)) => wifi::Wifi::new(peripherals.modem)
            .and_then(|mut wifi| {
                let ip = wifi.connect(ssid, password)?;
                log::info!("Connected to {ssid} with IP {ip}");
                Ok(wifi)
            })
            .inspect_err(|err| log::error!("Failed to connect to wifi: {err}"))
            .ok(),
        _ => None,
    };

    let Ok(mut display) = tm1637::Tm1637::new(peripherals.pins.gpio18, peripherals.pins.gpio19) else {
        log::error!("Failed to create TM1637 display");
        return ExitCode::FAILURE;
    };

    let Ok(()) = display.brightness(3) else {
        log::error!("Failed to set brightness");
        return ExitCode::FAILURE;
    };

    ExitCode::SUCCESS
}
