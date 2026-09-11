mod error;
mod http;
mod speaker;
mod tm1637;
mod wifi;

use std::process::ExitCode;

use chrono::{DateTime, Timelike};
use esp_idf_svc::{hal::peripherals::Peripherals, sys};

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

    // Returns the number of milliseconds since the Unix epoch
    let Ok(body) = http::get("https://bobman.dev/api/epoch") else {
        log::error!("Failed to get epoch");
        return ExitCode::FAILURE;
    };

    let Ok(ms_since_epoch) = body
        .into_iter()
        .fold(String::new(), |acc, val| format!("{}{}", acc, char::from(val)))
        .parse::<i64>()
    else {
        log::error!("Failed to parse epoch");
        return ExitCode::FAILURE;
    };
    log::info!("Milliseconds since epoch: {ms_since_epoch}");

    // `div_euclid`/`rem_euclid` so a pre-1970 timestamp still floors correctly.
    let seconds_since_epoch = ms_since_epoch.div_euclid(1000);
    // Always in `0..1_000_000`, so the conversion cannot fail.
    let micros = i32::try_from(ms_since_epoch.rem_euclid(1000).saturating_mul(1000)).unwrap_or(0);

    let tv = sys::timeval {
        tv_sec: seconds_since_epoch,
        tv_usec: micros,
    };
    let ret = unsafe { sys::settimeofday(&raw const tv, std::ptr::null()) };
    assert_eq!(ret, 0, "settimeofday failed");

    let Some(date) = DateTime::from_timestamp_millis(ms_since_epoch) else {
        log::error!("Epoch {ms_since_epoch} is out of range");
        return ExitCode::FAILURE;
    };
    log::info!("UTC: {date}");
    log::info!("Setting time to {}:{}", date.hour(), date.minute());
    let hour = date.hour().to_be_bytes()[3];
    let minute = date.minute().to_be_bytes()[3];

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
