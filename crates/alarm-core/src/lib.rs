use std::ops::BitAnd as _;

#[derive(Debug)]
pub struct Alarm {
    pub hour: u8,
    pub minute: u8,
    pub enabled: bool,
}

impl Alarm {
    /// Create an [`Alarm`] from 2 bytes.
    pub fn from_bytes(bytes: [&u8; 2]) -> Self {
        // moves the first bit up to the least significant bit
        let enabled = bytes[0] >> 7 == 1;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alarm_from_bytes_enabled() {
        let bytes = [&0b1000_0000, &u8::MIN];
        let alarm = Alarm::from_bytes(bytes);
        assert!(alarm.enabled);
    }

    #[test]
    fn test_alarm_from_bytes_disabled() {
        let bytes = [&u8::MIN, &u8::MIN];
        let alarm = Alarm::from_bytes(bytes);
        assert!(!alarm.enabled);
    }
}
