//! Independent M1–M3 lighting and ordinary-key overrides; G-key maps remain in
//! FileDto's existing banks and share its macro library. Missing banks inherit
//! legacy settings, so upgrading never loses the user's existing assignments.
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use super::KeyboardSettings;
use crate::profiles::format::ActionDto;

pub const NAMES: [&str; 3] = ["M1", "M2", "M3"];
pub fn default_bank() -> String { "M1".into() }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BankSettings {
    pub color: Option<String>,
    pub brightness: Option<u8>,
    #[serde(default)]
    pub per_key_enabled: bool,
    #[serde(default)]
    pub key_colors: BTreeMap<String, String>,
    #[serde(default)]
    pub regular_enabled: bool,
    #[serde(default)]
    pub regular_keys: BTreeMap<String, ActionDto>,
}

impl KeyboardSettings {
    // Continue listening on an unlit/empty bank if another profile uses control.
    pub fn controls_banks(&self) -> bool {
        self.enabled || self.color.is_some() || self.brightness.is_some() || self.regular_enabled
            || self.banks.values().any(|b| b.color.is_some() || b.brightness.is_some() || b.regular_enabled)
    }

    pub fn for_bank(&self, bank: &str) -> Self {
        let mut next = self.clone();
        next.active_bank = bank.into();
        if let Some(b) = self.banks.get(bank) {
            next.color = b.color.clone();
            next.brightness = b.brightness;
            next.per_key_enabled = b.per_key_enabled;
            next.key_colors = b.key_colors.clone();
            next.regular_enabled = b.regular_enabled;
            next.regular_keys = b.regular_keys.clone();
        }
        next
    }
}

pub fn key(bank: &str) -> Result<g13_core::G13Key, String> {
    match bank {
        "M1" => Ok(g13_core::G13Key::M1), "M2" => Ok(g13_core::G13Key::M2), "M3" => Ok(g13_core::G13Key::M3),
        _ => Err("G915 profile must be M1, M2 or M3".into()),
    }
}

pub fn from_mask(mask: u8) -> Option<&'static str> {
    match mask { 1 => Some("M1"), 2 => Some("M2"), 4 => Some("M3"), _ => None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_settings_keep_colors_and_regular_assignments_in_all_banks() {
        let mut original = KeyboardSettings { color: Some("#123456".into()), regular_enabled: true, ..Default::default() };
        original.regular_keys.insert("F6".into(), ActionDto::Chord("Ctrl+C".into()));
        let mut json = serde_json::to_value(&original).unwrap();
        json.as_object_mut().unwrap().remove("banks");
        json.as_object_mut().unwrap().remove("active_bank");
        let restored: KeyboardSettings = serde_json::from_value(json).unwrap();
        assert_eq!(restored.active_bank, "M1");
        for bank in NAMES {
            let effective = restored.for_bank(bank);
            assert_eq!(effective.color, original.color);
            assert_eq!(effective.regular_keys, original.regular_keys);
            assert!(effective.validate().is_ok());
        }
    }

    #[test]
    fn independent_banks_survive_serialization_and_produce_distinct_led_frames() {
        let mut settings = KeyboardSettings::default();
        settings.banks.insert("M1".into(), BankSettings {
            color: Some("#FF0000".into()), per_key_enabled: true,
            key_colors: BTreeMap::from([("W".into(), "#00FF00".into())]),
            regular_enabled: true, regular_keys: BTreeMap::from([("F6".into(), ActionDto::Chord("Ctrl+C".into()))]),
            ..Default::default()
        });
        settings.banks.insert("M2".into(), BankSettings {
            color: Some("#0000FF".into()), brightness: Some(25),
            regular_enabled: true, regular_keys: BTreeMap::from([("F6".into(), ActionDto::Chord("Ctrl+V".into()))]),
            ..Default::default()
        });
        settings.active_bank = "M2".into();
        let restored: KeyboardSettings = serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert_eq!(restored, settings);
        assert!(restored.validate().is_ok());
        let m1 = restored.for_bank("M1");
        let m2 = restored.for_bank("M2");
        assert_ne!(m1.regular_keys, m2.regular_keys);
        assert_eq!(m2.brightness, Some(25));
        assert!(m2.key_colors.is_empty());
        assert_ne!(super::super::lighting::frame(&[1, 23], m1.color.as_ref().unwrap(), Some(&m1.key_colors)).unwrap(),
            super::super::lighting::frame(&[1, 23], m2.color.as_ref().unwrap(), None).unwrap());
        // An empty profile must not stop physical M-key switching back to M1.
        assert!(restored.for_bank("M3").controls_banks());
        assert!(restored.for_bank("M3").color.is_none());
        let mut brightness_only = KeyboardSettings::default();
        brightness_only.banks.insert("M2".into(), BankSettings { brightness: Some(25), ..Default::default() });
        assert!(brightness_only.for_bank("M1").controls_banks());
    }

    #[test]
    fn validate_inactive_banks_and_reject_bad_switch_requests() {
        let mut settings = KeyboardSettings::default();
        settings.banks.insert("M3".into(), BankSettings { brightness: Some(101), ..Default::default() });
        assert!(settings.validate().is_err());
        settings.banks.clear();
        settings.active_bank = "M4".into();
        assert!(settings.validate().is_err());
        assert!(key("M4").is_err());
        assert_eq!(from_mask(2), Some("M2"));
        assert_eq!(from_mask(0), None); // release, not a profile change
        assert_eq!(from_mask(3), None); // ambiguous simultaneous press
    }
}
