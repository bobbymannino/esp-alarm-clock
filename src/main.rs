mod error;
mod http;
mod speaker;
mod time;
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

    let Ok(display) = tm1637::Tm1637::new(peripherals.pins.gpio18, peripherals.pins.gpio19) else {
        log::error!("Failed to create TM1637 display");
        return ExitCode::FAILURE;
    };

    let Ok(spinner) = display.spinner() else {
        log::error!("Failed to create spinner");
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

    let Some(wifi) = wifi else {
        log::error!("Failed to connect to wifi");
        return ExitCode::FAILURE;
    };

    let Ok(_) = wifi.ip() else {
        log::error!("Failed to get IP");
        return ExitCode::FAILURE;
    };

    let Ok(date) = time::sync().inspect_err(|err| log::error!("Failed to sync time: {err}")) else {
        return ExitCode::FAILURE;
    };

    let (hour, minute) = time::hour_minute(date);
    log::info!("Setting time to {hour}:{minute}");

    let Ok(mut display) = spinner.stop() else {
        log::error!("Failed to stop spinner");
        return ExitCode::FAILURE;
    };

    let Ok(()) = display.time(hour, minute, true) else {
        log::error!("Failed to set time on display");
        return ExitCode::FAILURE;
    };

    ExitCode::SUCCESS
}
