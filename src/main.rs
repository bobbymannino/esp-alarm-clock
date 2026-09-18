mod ec11;
mod error;
mod http;
mod speaker;
mod storage;
mod time;
mod tm1637;
mod wifi;

use std::process::ExitCode;

use alarm_core::Alarm;
use esp_idf_svc::{hal::peripherals::Peripherals, nvs::EspDefaultNvsPartition};

use crate::{error::Result, storage::Storage, wifi::Wifi};

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
    let dial = ec11::Ec11::new(peripherals.pins.gpio25, peripherals.pins.gpio26, peripherals.pins.gpio27)?;

    let nvs = EspDefaultNvsPartition::take()?;
    let storage = Storage::new(nvs.clone())?;
    let alarms = storage.alarms()?;
    log::info!("There are {} alarms", alarms.len());
    for alarm in &alarms {
        log::info!("Alarm: {alarm:?}");
    }
    if alarms.is_empty() {
        storage.set_alarms(vec![
            Alarm::new(11, 10, true),
            Alarm::new(15, 15, false),
            Alarm::new(11, 15, false),
            Alarm::new(16, 15, false),
        ])?;
    }

    let wifi = option_env!("WIFI_SSID")
        .zip(option_env!("WIFI_PASSWORD"))
        .and_then(|(ssid, password)| connect(peripherals.modem, ssid, password, nvs.clone()));

    if let Some(wifi) = wifi {
        wifi.ip()?;
        time::sync()?;
    }

    let display = spinner.stop()?;
    let _time = display.continuous_time()?;

    for event in dial.events()? {
        log::info!("Dial: {event:?}");
    }

    loop {
        std::thread::park();
    }
}

/// Joins the given network, logging rather than propagating a failure so the
/// clock can still run offline.
fn connect<'d>(
    modem: impl esp_idf_svc::hal::modem::WifiModemPeripheral + 'd,
    ssid: &str,
    password: &str,
    nvs: EspDefaultNvsPartition,
) -> Option<Wifi<'d>> {
    Wifi::new(modem, nvs)
        .and_then(|mut wifi| {
            let ip = wifi.connect(ssid, password)?;
            log::info!("Connected to {ssid} with IP {ip}");

            Ok(wifi)
        })
        .inspect_err(|err| log::error!("Failed to connect to wifi: {err}"))
        .ok()
}
