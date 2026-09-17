use std::ops::BitAnd;

use esp_idf_svc::nvs::{EspNvs, NvsDefault};

use crate::{error::Result, storage::Store};

#[derive(Debug)]
pub struct Alarm {
    hour: u8,
    minute: u8,
    enabled: bool,
}

/// Namespace used for storing alarms.
pub const NAMESPACE: &str = "alarms";

/// Number of bytes a single [`Alarm`] takes up.
const ALARM_SIZE: usize = 2;

/// Maximum number of [`Alarm`]s that can be stored (arbitrary).
const MAX_ALARMS: usize = 16;

impl Store<Self> for Alarm {
    const LIST_KEY: &str = "alarms";

    fn list(storage: &EspNvs<NvsDefault>) -> Result<Vec<Self>> {
        let mut buf = [0u8; ALARM_SIZE * MAX_ALARMS];

        let Some(blob) = storage.get_blob(Self::LIST_KEY, &mut buf)? else {
            return Ok(Vec::new());
        };

        let alarms = blob
            .chunks_exact(ALARM_SIZE)
            .filter_map(|chunk| match chunk {
                [byte1, byte2] => Some(Self::from_bytes([byte1, byte2])),
                _ => None,
            })
            .collect();

        Ok(alarms)
    }
}

impl Alarm {
    /// Create an [`Alarm`] from 2 bytes.
    fn from_bytes(bytes: [&u8; 2]) -> Self {
        let enabled = bytes[0].bitand(0b1000_0000) == 0b1000_0000;
        let hour = bytes[0].bitand(0b0111_1100);
        Self {
            hour: 0,
            minute: 0,
            enabled,
        }
        // todo!("finish the Alarm");
        // todo!("comment with bit table");
    }
}

