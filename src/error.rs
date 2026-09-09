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
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Esp(err) => write!(f, "ESP-IDF call failed: {err}"),
            Self::NotAcknowledged => f.write_str("TM1637 did not acknowledge byte"),
            Self::InvalidSsid => write!(f, "SSID must be 1 to {MAX_SSID_LEN} bytes long"),
            Self::InvalidPassword => write!(f, "password must be {MIN_PASSWORD_LEN} to {MAX_PASSWORD_LEN} bytes long"),
        }
    }
}

impl From<EspError> for Error {
    fn from(err: EspError) -> Self {
        Self::Esp(err)
    }
}
