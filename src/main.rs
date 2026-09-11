mod error;
mod http;
mod speaker;
mod time;
mod tm1637;
mod wifi;

use std::process::ExitCode;

use esp_idf_svc::hal::peripherals::Peripherals;

use crate::{error::Result, wifi::Wifi};

fn main() -> ExitCode {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            log::error!("Firmware stopped: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let peripherals = Peripherals::take()?;

    let display = tm1637::Tm1637::new(peripherals.pins.gpio18, peripherals.pins.gpio19)?;
    let spinner = display.spinner()?;

    let wifi = option_env!("WIFI_SSID")
        .zip(option_env!("WIFI_PASSWORD"))
        .and_then(|(ssid, password)| connect(peripherals.modem, ssid, password));

    if let Some(wifi) = wifi {
        wifi.ip()?;
        time::sync()?;
    }

    let display = spinner.stop()?;
    let _time = display.continuous_time()?;

    loop {
        std::thread::park();
    }
}

/// Joins the given network, logging rather than propagating a failure so the
/// clock can still run offline.
fn connect<'d>(modem: impl esp_idf_svc::hal::modem::WifiModemPeripheral + 'd, ssid: &str, password: &str) -> Option<Wifi<'d>> {
    Wifi::new(modem)
        .and_then(|mut wifi| {
            let ip = wifi.connect(ssid, password)?;
            log::info!("Connected to {ssid} with IP {ip}");

            Ok(wifi)
        })
        .inspect_err(|err| log::error!("Failed to connect to wifi: {err}"))
        .ok()
}
