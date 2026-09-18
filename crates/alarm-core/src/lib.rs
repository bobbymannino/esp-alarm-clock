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
        // Moves the first bit up to the least significant bit
        let enabled = bytes[0] >> 7 == 1;
        let hour = (bytes[0] >> 2) & 0b1_1111;
        let minute = (u16::from(bytes[0].clone()) << 8) | u16::from(bytes[1].clone());
        let minute = ((minute >> 4) & 0b0011_1111) as u8;
        Self {
            hour: hour.min(23),
            minute: minute.min(59),
            enabled,
        }
        // todo!("comment with bit table");
        // todo!("is this the most efficient way to bit convert?");
    }

    pub fn to_bytes(&self) -> [u8; 2] {
        todo!();
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
        let bytes = [&(23 << 2), &u8::MIN];
        let alarm = Alarm::from_bytes(bytes);
        assert_eq!(alarm.hour, 23);
    }

    #[test]
    fn test_alarm_from_bytes_hour_25() {
        let bytes = [&(25 << 2), &u8::MIN];
        let alarm = Alarm::from_bytes(bytes);
        assert_eq!(alarm.hour, 23);
    }

    #[test]
    fn test_alarm_from_bytes_hour_12() {
        let bytes = [&(12 << 2), &u8::MIN];
        let alarm = Alarm::from_bytes(bytes);
        assert_eq!(alarm.hour, 12);
    }

    #[test]
    fn test_alarm_from_bytes_hour_1() {
        let bytes = [&(1 << 2), &u8::MIN];
        let alarm = Alarm::from_bytes(bytes);
        assert_eq!(alarm.hour, 1);
    }

    #[test]
    fn test_alarm_from_bytes_minute_0() {
        let bytes = [&u8::MIN, &u8::MIN];
        let alarm = Alarm::from_bytes(bytes);
        assert_eq!(alarm.minute, 0);
    }

    #[test]
    fn test_alarm_from_bytes_minute_1() {
        let bytes = [&u8::MIN, &(1 << 4)];
        let alarm = Alarm::from_bytes(bytes);
        assert_eq!(alarm.minute, 1);
    }

    #[test]
    fn test_alarm_from_bytes_minute_10() {
        let bytes = [&u8::MIN, &(10 << 4)];
        let alarm = Alarm::from_bytes(bytes);
        assert_eq!(alarm.minute, 10);
    }

    #[test]
    fn test_alarm_from_bytes_minute_15() {
        // The largest minute that fits entirely in the high nibble of byte 1
        let bytes = [&u8::MIN, &(15 << 4)];
        let alarm = Alarm::from_bytes(bytes);
        assert_eq!(alarm.minute, 15);
    }

    #[test]
    fn test_alarm_from_bytes_minute_16() {
        // The smallest minute that needs the low 2 bits of byte 0
        let bytes = [&0b0000_0001, &u8::MIN];
        let alarm = Alarm::from_bytes(bytes);
        assert_eq!(alarm.minute, 16);
    }

    #[test]
    fn test_alarm_from_bytes_minute_30() {
        let bytes = [&0b0000_0001, &0b1110_0000];
        let alarm = Alarm::from_bytes(bytes);
        assert_eq!(alarm.minute, 30);
    }

    #[test]
    fn test_alarm_from_bytes_minute_45() {
        let bytes = [&0b0000_0010, &0b1101_0000];
        let alarm = Alarm::from_bytes(bytes);
        assert_eq!(alarm.minute, 45);
    }

    #[test]
    fn test_alarm_from_bytes_minute_59() {
        let bytes = [&0b0000_0011, &0b1011_0000];
        let alarm = Alarm::from_bytes(bytes);
        assert_eq!(alarm.minute, 59);
    }

    #[test]
    fn test_alarm_from_bytes_minute_63() {
        // The largest value the 6 minute bits can hold
        // Should be capped at 59
        let bytes = [&0b0000_0011, &0b1111_0000];
        let alarm = Alarm::from_bytes(bytes);
        assert_eq!(alarm.minute, 59);
    }
}
