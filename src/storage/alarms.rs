use alarm_core::Alarm;
use esp_idf_svc::nvs::{EspNvs, NvsDefault};

use crate::{error::Result, storage::Store};

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
