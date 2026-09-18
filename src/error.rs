use core::fmt::{self, Display, Formatter};

use esp_idf_svc::sys::EspError;

use crate::wifi::{MAX_PASSWORD_LEN, MAX_SSID_LEN, MIN_PASSWORD_LEN};

/// A [`core::result::Result`] that defaults to failing with [`Error`].
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// Everything that can go wrong in the firmware.
#[derive(Debug)]
pub enum Error {
    /// An ESP-IDF call returned a non `ESP_OK` code.
    Esp(EspError),
    /// The TM1637 did not pull the data line low on the ninth clock.
    NotAcknowledged,
    /// The SSID was empty or longer than [`MAX_SSID_LEN`] bytes.
    InvalidSsid,
    /// The pre shared key was outside the length the standard allows.
    InvalidPassword,
    /// A non 2xx HTTP response status was returned.
    HttpStatus(String, u16),
    /// A worker thread could not be spawned.
    Io(std::io::Error),
    /// A worker thread panicked instead of returning a result.
    WorkerPanicked,
    /// The epoch endpoint answered with something other than a timestamp.
    InvalidEpoch,
    /// A millisecond timestamp is not a representable date.
    EpochOutOfRange(i64),
    /// `settimeofday` returned a non zero code.
    SetTimeFailed(i32),
    /// Memory has been corrupted.
    MemoryCorruption,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Esp(err) => write!(f, "ESP-IDF call failed: {err}"),
            Self::NotAcknowledged => f.write_str("TM1637 did not acknowledge byte"),
            Self::InvalidSsid => write!(f, "SSID must be 1 to {MAX_SSID_LEN} bytes long"),
            Self::InvalidPassword => write!(f, "password must be {MIN_PASSWORD_LEN} to {MAX_PASSWORD_LEN} bytes long"),
            Self::HttpStatus(url, status) => write!(f, "{url} returned {status}"),
            Self::Io(err) => write!(f, "IO call failed: {err}"),
            Self::WorkerPanicked => f.write_str("worker thread panicked"),
            Self::InvalidEpoch => f.write_str("epoch endpoint did not return a millisecond timestamp"),
            Self::EpochOutOfRange(ms) => write!(f, "epoch {ms} is out of range"),
            Self::SetTimeFailed(ret) => write!(f, "settimeofday failed with {ret}"),
            Self::MemoryCorruption => f.write_str("memory corruption"),
        }
    }
}

impl From<EspError> for Error {
    fn from(err: EspError) -> Self {
        Self::Esp(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}
