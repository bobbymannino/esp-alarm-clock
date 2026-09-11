use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use esp_idf_svc::sys;

use crate::{
    error::{Error, Result},
    http,
};

/// Endpoint that answers with the number of milliseconds since the Unix epoch.
const EPOCH_URL: &str = "https://bobman.dev/api/epoch";

/// Fetches the current time from [`EPOCH_URL`] and writes it to the system clock.
///
/// # Returns
///
/// - `Ok`: The time the clock was set to, in UTC.
/// - `Err`: An error if the time could not be fetched or set.
///
/// # Errors
///
/// - `Esp`: The request failed.
/// - `HttpStatus`: A non 2xx response status.
/// - `Io`: The worker thread could not be spawned.
/// - `WorkerPanicked`: The worker thread panicked.
/// - `InvalidEpoch`: The response was not a millisecond timestamp.
/// - `EpochOutOfRange`: The timestamp is not a representable date.
/// - `SetTimeFailed`: `settimeofday` rejected the timestamp.
pub fn sync() -> Result<DateTime<Utc>> {
    let ms_since_epoch = fetch()?;
    log::info!("Milliseconds since epoch: {ms_since_epoch}");

    set(ms_since_epoch)
}

/// Reads the current system time.
///
/// The clock only holds a real date once [`sync`] has run, before that it counts
/// up from the Unix epoch.
///
/// # Returns
///
/// - `Ok`: The current time, in UTC.
/// - `Err`: An error if the clock is outside the range of a [`DateTime`].
///
/// # Errors
///
/// - `EpochOutOfRange`: The clock is not a representable date.
pub fn now() -> Result<DateTime<Utc>> {
    let ms_since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|elapsed| elapsed.as_millis())
        .and_then(|ms| i64::try_from(ms).ok())
        .ok_or(Error::EpochOutOfRange(0))?;

    from_millis(ms_since_epoch)
}

/// Splits a time into the hour and minute a 4 digit display shows.
///
/// Both always fit in a [`u8`], so the conversion cannot fail.
#[must_use]
pub fn hour_minute(date: DateTime<Utc>) -> (u8, u8) {
    use chrono::Timelike as _;

    (u8::try_from(date.hour()).unwrap_or(0), u8::try_from(date.minute()).unwrap_or(0))
}

/// Writes a millisecond timestamp to the system clock, see [`sync`].
fn set(ms_since_epoch: i64) -> Result<DateTime<Utc>> {
    let date = from_millis(ms_since_epoch)?;

    // `div_euclid`/`rem_euclid` so a pre-1970 timestamp still floors correctly.
    let seconds_since_epoch = ms_since_epoch.div_euclid(1000);
    // Always in `0..1_000_000`, so the conversion cannot fail.
    let micros = i32::try_from(ms_since_epoch.rem_euclid(1000).saturating_mul(1000)).unwrap_or(0);

    let tv = sys::timeval {
        tv_sec: seconds_since_epoch,
        tv_usec: micros,
    };
    // SAFETY: `tv` is a valid, initialised `timeval` and a null timezone is allowed.
    let ret = unsafe { sys::settimeofday(&raw const tv, std::ptr::null()) };
    if ret != 0 {
        return Err(Error::SetTimeFailed(ret));
    }

    log::info!("UTC: {date}");

    Ok(date)
}

/// Asks [`EPOCH_URL`] for the number of milliseconds since the Unix epoch.
fn fetch() -> Result<i64> {
    let body = http::get(EPOCH_URL)?;

    body.into_iter()
        .fold(String::new(), |acc, val| format!("{}{}", acc, char::from(val)))
        .parse::<i64>()
        .map_err(|_| Error::InvalidEpoch)
}

/// Turns a millisecond timestamp into a UTC date.
fn from_millis(ms_since_epoch: i64) -> Result<DateTime<Utc>> {
    DateTime::from_timestamp_millis(ms_since_epoch).ok_or(Error::EpochOutOfRange(ms_since_epoch))
}
