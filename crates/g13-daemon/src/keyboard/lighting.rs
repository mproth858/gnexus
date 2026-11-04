//! G915 per-key RGB: protocol IDs, full-frame planning, and transport writes.
use std::collections::{BTreeMap, BTreeSet};
use logi_hidpp::features::per_led;
use crate::{keycode::{HidUsage, KeyCode}, mouse::{Link, settings::parse_color}};

pub fn led_for_key(name: &str) -> Option<u8> {
    match name.to_ascii_lowercase().as_str() {
        "g1" => return Some(180), "g2" => return Some(181), "g3" => return Some(182),
        "g4" => return Some(183), "g5" => return Some(184), "logo" => return Some(210),
        "brightness" => return Some(153), _ => {}
    }
    let key = KeyCode::parse(name)?;
    match key {
        KeyCode::MediaPlayPause => Some(155), KeyCode::VolumeMute => Some(156),
        KeyCode::MediaNext => Some(157), KeyCode::MediaPrevious => Some(158),
        _ => match key.hid() {
            // Standard keyboard positions A through Application (Menu).
            // F13+ must not use this arithmetic: they collide with modifier LEDs.
            HidUsage::Key(usage @ 0x04..=0x65) => Some(usage - 3),
            HidUsage::Modifier(bit) => Some(104 + bit.trailing_zeros() as u8),
            _ => None,
        }
    }
}

pub fn available_keys(leds: &[u8]) -> Vec<String> {
    KeyCode::ALL.iter().map(|key| key.name().to_string())
        .chain(["G1", "G2", "G3", "G4", "G5", "Logo", "Brightness"].into_iter().map(str::to_string))
        .filter(|key| led_for_key(key).is_some_and(|id| leds.contains(&id))).collect()
}

pub fn validate(settings: &super::KeyboardSettings) -> Result<(), String> {
    if settings.per_key_enabled && settings.color.is_none() {
        return Err("Enable lighting control and choose a base color for per-key colors".into());
    }
    let mut used = BTreeSet::new();
    for (key, color) in &settings.key_colors {
        let id = led_for_key(key).ok_or_else(|| format!("No G915 lighting zone for {key}"))?;
        if !used.insert(id) { return Err(format!("Duplicate lighting key: {key}")); }
        parse_color(color)?;
    }
    Ok(())
}

pub fn discover_leds(link: &Link, feature: u8) -> Result<Vec<u8>, String> {
    let pages = (0..3).map(|page| link.call(feature, per_led::FN_INFO, &[0, page])).collect::<Result<Vec<_>, _>>()?;
    per_led::keyboard_led_ids(&pages)
}

/// WirelessDeviceStatus asks the host to restore settings after wake/power-on.
pub fn reconfigure_requested(function: u8, data: &[u8; 16]) -> bool { function == 0 && data[1] == 1 }

/// Every advertised LED gets a value, so removing an override resets its color.
pub fn frame(leds: &[u8], base: &str, colors: Option<&BTreeMap<String, String>>) -> Result<Vec<(u8, [u8; 3])>, String> {
    let base = parse_color(base)?;
    let mut frame: BTreeMap<_, _> = leds.iter().map(|id| (*id, base)).collect();
    if let Some(colors) = colors {
        for (key, color) in colors {
            let id = led_for_key(key).ok_or_else(|| format!("Unknown lighting key: {key}"))?;
            let slot = frame.get_mut(&id).ok_or_else(|| format!("{key} has no supported LED on this connection"))?;
            *slot = parse_color(color)?;
        }
    }
    Ok(frame.into_iter().collect())
}

/// Kept separate from Link so error handling/atomic commits can be tested.
pub fn send_frame(frame: &[(u8, [u8; 3])], mut call: impl FnMut(u8, &[u8]) -> Result<(), String>) -> Result<(), String> {
    for request in per_led::set_requests(frame) { call(per_led::FN_SET, &request)?; }
    call(per_led::FN_FRAME_END, &[0])
}

pub fn write_frame(link: &Link, feature: u8, frame: &[(u8, [u8; 3])]) -> Result<(), String> {
    send_frame(frame, |function, params| {
        let mut last = None;
        for retry in 0..=3 {
            match link.call(feature, function, params) {
                Ok(_) => return Ok(()),
                Err(error) if (error.contains("Busy") || error.contains("pas de réponse")) && retry < 3 => {
                    last = Some(error);
                    std::thread::sleep(std::time::Duration::from_millis(30 * (retry + 1)));
                }
                Err(error) => return Err(error),
            }
        }
        Err(last.unwrap_or_else(|| "Per-key lighting update failed".into()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_ids_do_not_confuse_hid_usages_modifiers_or_media_controls() {
        assert_eq!(led_for_key("A"), Some(1));
        assert_eq!(led_for_key("W"), Some(23));
        assert_eq!(led_for_key("LeftCtrl"), Some(104));
        assert_eq!(led_for_key("RightMeta"), Some(111));
        assert_eq!(led_for_key("RightBracket"), Some(45));
        assert_eq!(led_for_key("Backslash"), Some(46));
        assert_eq!(led_for_key("Pause"), Some(69));
        assert_eq!(led_for_key("MediaPlayPause"), Some(155));
        assert_eq!(led_for_key("G5"), Some(184));
        assert_eq!(led_for_key("Logo"), Some(210));
        assert_eq!(led_for_key("F16"), None); // would alias LeftCtrl if HID-3 were used blindly
        assert_eq!(led_for_key("VolumeUp"), None); // the roller has no individual LED
    }

    #[test]
    fn deleting_a_color_reverts_that_key_to_the_base() {
        let colors = BTreeMap::from([("W".into(), "#FF0000".into())]);
        assert_eq!(frame(&[1, 23, 104], "#001122", Some(&colors)).unwrap(),
            vec![(1, [0, 17, 34]), (23, [255, 0, 0]), (104, [0, 17, 34])]);
        assert_eq!(frame(&[1, 23, 104], "#001122", Some(&BTreeMap::new())).unwrap()[1], (23, [0, 17, 34]));
        assert_eq!(frame(&[1, 23], "#001122", None).unwrap()[1], (23, [0, 17, 34]));
        assert!(frame(&[1], "#001122", Some(&colors)).is_err(), "unsupported connection must not be silently accepted");
    }

    #[test]
    fn frames_commit_once_after_all_batches_and_never_after_partial_failure() {
        let frame: Vec<_> = (1..=9).map(|id| (id, [id, 2, 3])).collect();
        let mut sent = Vec::new();
        send_frame(&frame, |function, payload| { sent.push((function, payload.to_vec())); Ok(()) }).unwrap();
        assert_eq!(sent.iter().map(|(f, _)| *f).collect::<Vec<_>>(), vec![1, 1, 1, 7]);
        assert_eq!(sent[0].1, vec![1, 1, 2, 3, 2, 2, 2, 3, 3, 3, 2, 3, 4, 4, 2, 3]);
        assert_eq!(sent[3].1, vec![0]);
        let mut sent = Vec::new();
        let result = send_frame(&frame, |function, _| {
            sent.push(function);
            if sent.len() == 2 { Err("disconnected".into()) } else { Ok(()) }
        });
        assert!(result.is_err());
        assert_eq!(sent, vec![1, 1]);
    }

    #[test]
    fn invalid_colors_and_alias_collisions_are_rejected_before_writing() {
        let mut settings = super::super::KeyboardSettings::default();
        settings.key_colors.insert("W".into(), "invalid".into());
        assert!(validate(&settings).is_err());
        settings.key_colors = BTreeMap::from([("Ctrl".into(), "#FF0000".into()), ("LeftCtrl".into(), "#0000FF".into())]);
        assert!(validate(&settings).is_err());
        settings.key_colors.clear(); settings.per_key_enabled = true;
        assert!(validate(&settings).is_err());
        settings.color = Some("#FFFFFF".into());
        assert!(validate(&settings).is_ok());
    }

    #[test]
    fn wake_requires_an_explicit_software_reconfiguration_request() {
        let mut data = [0; 16]; data[2] = 1;
        assert!(!reconfigure_requested(0, &data));
        data[1] = 1;
        assert!(reconfigure_requested(0, &data));
        assert!(!reconfigure_requested(1, &data));
    }
}
