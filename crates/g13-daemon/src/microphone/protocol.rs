//! Yeti GX feature 0x8370. Field layout documented from firmware 2.00 USB
//! captures by hicham-bouchikhi/yeti-gx-linux; indexes are resolved at runtime.
pub const FEATURE: u16 = 0x8370;
pub const READ: u8 = 2;
pub const WRITE: u8 = 3;

#[derive(Debug, Clone, Copy)]
pub enum Control { Gain(u8), Mute(bool), SmartLock(bool) }
impl Control {
    pub fn params(self) -> Result<[u8; 3], String> {
        match self {
            Self::Gain(value) if value <= 100 => Ok([0, 1, value]),
            Self::Gain(_) => Err("Microphone gain must be between 0 and 100".into()),
            Self::Mute(muted) => Ok([0, 0, u8::from(!muted)]),
            Self::SmartLock(enabled) => Ok([0, 5, if enabled { 7 } else { 6 }]),
        }
    }
}

pub fn value(data: &[u8; 16], field: u8) -> Result<u8, String> {
    let expected_type = match field { 0 => 0, 1 => 1, 2 => 5, _ => return Err("Unknown microphone field".into()) };
    if data[..3] != [0, field, expected_type] { return Err("Unexpected Yeti GX control reply".into()); }
    let value = data[3];
    if value > if field == 1 { 100 } else { 1 } { return Err("Invalid Yeti GX control value".into()); }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mute_polarity_and_smart_lock_write_selector_match_captures() {
        assert_eq!(Control::Mute(true).params().unwrap(), [0, 0, 0]);
        assert_eq!(Control::Mute(false).params().unwrap(), [0, 0, 1]);
        assert_eq!(Control::SmartLock(true).params().unwrap(), [0, 5, 7]);
        assert_eq!(Control::SmartLock(false).params().unwrap(), [0, 5, 6]);
        assert!(Control::Gain(101).params().is_err());
        let request = logi_hidpp::Request::new(0xff, 13, WRITE, &Control::Gain(75).params().unwrap()).with_sw_id(12);
        assert_eq!(request.to_short_bytes().unwrap(), [0x10, 0xff, 13, 0x3c, 0, 1, 75]);
        assert_eq!(request.to_bytes()[0], 0x11, "existing long transport stays unchanged");
        assert!(logi_hidpp::Request::new(0xff, 13, WRITE, &[0, 1, 2, 3]).to_short_bytes().is_none());
    }
    #[test]
    fn values_follow_field_and_type_echo_not_notification_offsets() {
        let mut read = [0; 16]; read[..4].copy_from_slice(&[0, 1, 1, 71]);
        assert_eq!(value(&read, 1).unwrap(), 71);
        assert!(value(&read, 0).is_err());
        read[..4].copy_from_slice(&[0, 2, 5, 1]);
        assert_eq!(value(&read, 2).unwrap(), 1);
        read[3] = 6;
        assert!(value(&read, 2).is_err());
    }
}
