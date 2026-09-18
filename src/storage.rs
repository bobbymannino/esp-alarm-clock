use alarm_core::Alarm;
use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault};

use crate::error::Result;

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

    pub fn set_alarms(&self, alarms: Vec<Alarm>) -> Result<()> {
        Alarm::store(&self.alarms, alarms)
    }
}

trait Store<T> {
    /// Key used for setting and getting the list of items.
    const LIST_KEY: &str;

    /// Lists all items stored in the NVS.
    fn list(storage: &EspNvs<NvsDefault>) -> Result<Vec<T>>;

    /// Store a list of items in the NVS.
    fn store(storage: &EspNvs<NvsDefault>, list: Vec<T>) -> Result<()>;
}
