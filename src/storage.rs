use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault};

use crate::{error::Result, storage::alarms::Alarm};

mod alarms;

pub struct Storage {
    alarms: EspNvs<NvsDefault>,
}

impl Storage {
    pub fn new(partition: EspDefaultNvsPartition) -> Result<Self> {
        let alarms = EspNvs::new(partition, alarms::NAMESPACE, true)?;
        Ok(Self { alarms })
    }

    pub fn alarms(&self) -> Result<Vec<Alarm>> {
        Alarm::list(&self.alarms)
    }
}

trait Store<T> {
    /// Key used for setting and getting the list of items.
    const LIST_KEY: &str;

    /// Lists all items stored in the NVS.
    fn list(storage: &EspNvs<NvsDefault>) -> Result<Vec<T>>;
}
