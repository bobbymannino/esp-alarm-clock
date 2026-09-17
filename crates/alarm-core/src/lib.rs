#[derive(Debug)]
pub struct Alarm {
    pub hour: u8,
    pub minute: u8,
    pub enabled: bool,
}

impl Alarm {
    /// Create an [`Alarm`] from 2 bytes.
    #[must_use]
    pub fn from_bytes(bytes: [&u8; 2]) -> Self {
        // moves the first bit up to the least significant bit
        let enabled = bytes[0] >> 7 == 1;
        let hour = (bytes[0] >> 3) & 0b1_1111;
        Self { hour, minute: 0, enabled }
        // todo!("finish the Alarm");
        // todo!("comment with bit table");
        // todo!("is this the most efficient way to bit convert?");
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

    #[test]
    fn test_alarm_from_bytes_hour_zero() {
        let bytes = [&u8::MIN, &u8::MIN];
        let alarm = Alarm::from_bytes(bytes);
        assert_eq!(alarm.hour, 0);
    }

    #[test]
    fn test_alarm_from_bytes_hour_23() {
        let bytes = [&(23 << 3), &u8::MIN];
        let alarm = Alarm::from_bytes(bytes);
        assert_eq!(alarm.hour, 23);
    }

    #[test]
    fn test_alarm_from_bytes_hour_12() {
        let bytes = [&(12 << 3), &u8::MIN];
        let alarm = Alarm::from_bytes(bytes);
        assert_eq!(alarm.hour, 12);
    }

    #[test]
    fn test_alarm_from_bytes_hour_1() {
        let bytes = [&(1 << 3), &u8::MIN];
        let alarm = Alarm::from_bytes(bytes);
        assert_eq!(alarm.hour, 1);
    }
}
