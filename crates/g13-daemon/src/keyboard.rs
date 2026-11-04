//! G915: capability-based HID++ discovery and a separate mapping engine.
//! Ordinary keyboard input stays on the OS HID path. Only G/M keys are diverted,
//! and only after the user enables them. No onboard flash sectors are written.
use std::sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc::{self, Receiver}};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use g13_core::G13Key;
use hidapi::HidApi;
use logi_hidpp::{DIRECT, features as f};
use serde::{Deserialize, Serialize};

use crate::{device::DeviceEvent, engine::Engine, mouse::Link, platform::InputInjector, profiles::{self, FileDto}};

pub mod lighting;
pub mod banks;

const GKEYS: u16 = 0x8010;
const MKEYS: u16 = 0x8020;
const BRIGHTNESS: u16 = 0x8040;
const BATTERY_VOLTAGE: u16 = 0x1001;
const KEYS: [G13Key; 5] = [G13Key::G1, G13Key::G2, G13Key::G3, G13Key::G4, G13Key::G5];

/// Ignore bits for keys this model does not have; always release before pressing.
fn gkey_edges(previous: u8, next: u8) -> Vec<(G13Key, bool)> {
    let mut edges = Vec::new();
    for pressed in [false, true] {
        for (bit, key) in KEYS.into_iter().enumerate() {
            if (previous ^ next) & (1 << bit) != 0 && (next & (1 << bit) != 0) == pressed {
                edges.push((key, pressed));
            }
        }
    }
    edges
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyboardSettings {
    #[serde(default = "banks::default_bank")]
    pub active_bank: String,
    #[serde(default)]
    pub banks: std::collections::BTreeMap<String, banks::BankSettings>,
    #[serde(default)]
    pub enabled: bool,
    /// None leaves the keyboard's lighting alone.
    pub color: Option<String>,
    /// None leaves the current brightness alone; otherwise percent 0..100.
    pub brightness: Option<u8>,
    pub profile: FileDto,
    #[serde(default)]
    pub regular_enabled: bool,
    #[serde(default)]
    pub regular_keys: std::collections::BTreeMap<String, crate::profiles::format::ActionDto>,
    #[serde(default)]
    pub per_key_enabled: bool,
    #[serde(default)]
    pub key_colors: std::collections::BTreeMap<String, String>,
}

impl Default for KeyboardSettings {
    fn default() -> Self {
        Self { active_bank: banks::default_bank(), banks: Default::default(), enabled: false, color: None, brightness: None,
            profile: FileDto { name: "G915".into(), ..FileDto::default() }, regular_enabled: false, regular_keys: Default::default(),
            per_key_enabled: false, key_colors: Default::default() }
    }
}

impl KeyboardSettings {
    pub fn validate(&self) -> Result<(), String> {
        banks::key(&self.active_bank)?;
        if self.banks.keys().any(|b| !banks::NAMES.contains(&b.as_str())) {
            return Err("G915 profiles must use M1, M2 or M3".into());
        }
        self.validate_active()?;
        for bank in banks::NAMES { self.for_bank(bank).validate_active()?; }
        Ok(())
    }

    fn validate_active(&self) -> Result<(), String> {
        if self.brightness.is_some_and(|v| v > 100) { return Err("Brightness must be between 0 and 100".into()); }
        if let Some(c) = &self.color { crate::mouse::settings::parse_color(c)?; }
        for bank in [&self.profile.banks.m1, &self.profile.banks.m2, &self.profile.banks.m3] {
            if bank.keys.keys().any(|k| !["G1", "G2", "G3", "G4", "G5"].contains(&k.as_str())) {
                return Err("G915 bindings must use G1 through G5".into());
            }
        }
        if self.profile.stick.is_some() { return Err("G915 has no stick".into()); }
        profiles::from_dto(self.profile.clone()).map_err(|e| e.to_string())?;
        crate::remap::compile(&self.regular_keys, &self.profile.macros)?;
        lighting::validate(self)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct KeyboardStatus {
    pub connected: bool,
    pub name: String,
    pub transport: String,
    pub product_id: u16,
    pub voltage_mv: Option<u16>,
    pub brightness_supported: bool,
    pub lighting_supported: bool,
    pub gkeys_supported: bool,
    pub bank: String,
    pub held: u8,
    pub settings: KeyboardSettings,
    pub message: Option<String>,
    pub error: bool,
    pub revision: u64,
    pub regular_supported: bool,
    pub regular_active: bool,
    pub per_key_supported: bool,
    pub color_keys: Vec<String>,
}

pub enum KeyboardCommand { Apply(Box<KeyboardSettings>), SwitchBank(String), PauseRegular }

struct Keyboard {
    link: Link,
    status: KeyboardStatus,
    gkeys: Option<u8>,
    mkeys: Option<u8>,
    rgb: Option<u8>,
    brightness: Option<u8>,
    battery: Option<u8>,
    onboard: Option<u8>,
    original_mode: Option<u8>,
    claimed: bool,
    diverted: bool,
    per_key: Option<u8>,
    leds: Vec<u8>,
    wireless: Option<u8>,
}

fn find(api: &HidApi) -> Option<Keyboard> {
    for info in api.device_list().filter(|d| d.vendor_id() == 0x046D && d.usage_page() == 0xFF00 && d.usage() == 2) {
        let Ok(mut dev) = info.open_device(api) else { continue };
        for index in [DIRECT, 1, 2, 3, 4, 5, 6] {
            let mut link = Link { dev, index, timeout: Duration::from_millis(120),
                sw_id: 0x0C, events: Default::default() };
            if link.call(0, 1, &[0, 0, 0x5A]).is_ok_and(|d| d[0] >= 2 && d[2] == 0x5A) {
                link.timeout = Duration::from_millis(400);
                if let Some(name) = link.name().filter(|n| n.starts_with("G915 ") && !n.contains("TKL") && !n.starts_with("G915 X")) {
                    let gkeys = link.feature(GKEYS);
                    let mkeys = link.feature(MKEYS);
                    let rgb = link.feature(f::RGB_EFFECTS);
                    let brightness = link.feature(BRIGHTNESS);
                    let battery = link.feature(BATTERY_VOLTAGE);
                    let onboard = link.feature(f::ONBOARD_PROFILES);
                    let per_key = link.feature(f::per_led::FEATURE);
                    let leds = per_key.and_then(|i| lighting::discover_leds(&link, i).ok()).unwrap_or_default();
                    let color_keys = lighting::available_keys(&leds);
                    let wireless = link.feature(0x1D4B);
                    let transport = if matches!(info.bus_type(), hidapi::BusType::Bluetooth) { "Bluetooth" }
                        else if index == DIRECT { "USB" } else { "LIGHTSPEED" };
                    return Some(Keyboard { link, status: KeyboardStatus { connected: true, name, transport: transport.into(),
                        product_id: info.product_id(), gkeys_supported: gkeys.is_some(), lighting_supported: rgb.is_some(),
                        brightness_supported: brightness.is_some(), regular_supported: cfg!(windows), bank: "M1".into(),
                        per_key_supported: rgb.is_some() && per_key.is_some() && !leds.is_empty(), color_keys, ..Default::default() },
                        gkeys, mkeys, rgb, brightness, battery, onboard, original_mode: None, claimed: false, diverted: false, per_key, leds, wireless });
                }
            }
            dev = link.dev;
        }
    }
    None
}

impl Keyboard {
    fn release(&mut self) {
        if self.diverted {
            if let Some(i) = self.gkeys { let _ = self.link.call(i, 2, &[0]); }
            self.diverted = false;
        }
        if self.claimed {
            if let Some(i) = self.rgb { let _ = self.link.call(i, 5, &f::rgb::SW_CONTROL_RELEASE); }
            self.claimed = false;
        }
        if let (Some(i), Some(mode)) = (self.onboard, self.original_mode.take()) {
            let _ = self.link.call(i, 1, &[mode]);
        }
    }

    fn apply(&mut self, settings: &KeyboardSettings) -> Result<(), String> {
        settings.validate()?;
        if (settings.controls_banks() || settings.brightness.is_some())
            && crate::mouse::worker::GhubWatch::new().running() {
            return Err("Close G HUB before applying G915 settings".into());
        }
        if settings.enabled && self.gkeys.is_none() { return Err("G-key control is unavailable on this connection".into()); }
        if settings.color.is_some() && self.rgb.is_none() { return Err("Lighting control is unavailable on this connection".into()); }
        if settings.brightness.is_some() && self.brightness.is_none() { return Err("Brightness control is unavailable on this connection".into()); }
        if settings.per_key_enabled && (self.per_key.is_none() || self.leds.is_empty()) {
            return Err("Per-key lighting is unavailable on this connection".into());
        }
        // Validate every requested LED before taking control or changing any lights.
        let frame = settings.color.as_ref().map(|color| lighting::frame(&self.leds, color,
            if settings.per_key_enabled { Some(&settings.key_colors) } else { None })).transpose()?;
        self.release();
        if settings.controls_banks() && let Some(i) = self.onboard {
            let mode = self.link.call(i, 2, &[])?[0];
            if ![1, 2].contains(&mode) { return Err("Unknown onboard profile mode".into()); }
            self.original_mode = Some(mode);
            self.link.call(i, 1, &[2])?;
        }
        if let (Some(i), Some(color)) = (self.rgb, &settings.color) {
            let color = crate::mouse::settings::parse_color(color)?;
            // Discover zone/effect indices, rather than assuming mouse layouts.
            let zones = self.link.call(i, 0, &[0xFF, 0xFF, 0])?[2];
            let mut effects = Vec::new();
            for zone in 0..zones {
                let count = self.link.call(i, 0, &[zone, 0xFF, 0])?[4];
                let mut found = None;
                for effect in 0..count {
                    let d = self.link.call(i, 0, &[zone, effect, 0])?;
                    if u16::from_be_bytes([d[2], d[3]]) == f::rgb::EFFECT_STATIC { found = Some(effect); break; }
                }
                effects.push((zone, found.ok_or("Static lighting is unavailable")?));
            }
            if effects.is_empty() { return Err("No lighting zones reported".into()); }
            self.claimed = true;
            self.link.call(i, 5, &[1, 3, 5])?;
            for (zone, effect) in effects {
                self.link.call(i, 1, &f::rgb::cluster_effect(zone, effect, &f::rgb::LiveEffect::Static(color)))?;
            }
            // A complete frame also clears stale per-key colors when returning
            // to one base color. Commit only after every write succeeded.
            if let (Some(per_key), Some(frame)) = (self.per_key, &frame)
                && !frame.is_empty() {
                lighting::write_frame(&self.link, per_key, frame)?;
                // Replay the first frame after a software-control claim: the
                // firmware may still be leaving onboard mode during the first send.
                lighting::write_frame(&self.link, per_key, frame)?;
            }
        }
        if let (Some(i), Some(percent)) = (self.brightness, settings.brightness) {
            let info = self.link.call(i, 0, &[])?;
            let max = u16::from_be_bytes([info[0], info[1]]);
            let value = (u32::from(max) * u32::from(percent) / 100) as u16;
            if info[3] & 4 != 0 { self.link.call(i, 4, &[u8::from(percent != 0)])?; }
            if percent != 0 || info[3] & 4 == 0 { self.link.call(i, 2, &value.to_be_bytes())?; }
        }
        self.link.events.borrow_mut().clear();
        if settings.controls_banks() && let Some(i) = self.gkeys {
            self.diverted = true;
            self.link.call(i, 2, &[1])?;
            if let Some(mkeys) = self.mkeys {
                let bit = banks::NAMES.iter().position(|b| *b == settings.active_bank).ok_or("Unknown G915 profile")?;
                self.link.call(mkeys, 1, &[1 << bit])?;
            }
        }
        Ok(())
    }
}

/// Read-only hardware check using the same discovery code as the app.
pub fn probe() -> Result<KeyboardStatus, String> {
    let api = HidApi::new().map_err(|e| e.to_string())?;
    let mut keyboard = find(&api).ok_or("No G915 HID++ connection found")?;
    if let Some(i) = keyboard.battery {
        keyboard.status.voltage_mv = keyboard.link.call(i, 0, &[]).ok()
            .map(|d| u16::from_be_bytes([d[0], d[1]]));
    }
    Ok(keyboard.status)
}

pub fn spawn(commands: Receiver<KeyboardCommand>, publish: impl Fn(KeyboardStatus) + Send + 'static,
    shutdown: Arc<AtomicBool>, injector: Arc<dyn InputInjector>, config_dir: Option<std::path::PathBuf>) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new().name("g915".into()).spawn(move || {
        let settings_path = config_dir.map(|p| p.join("g915.json"));
        let mut settings = settings_path.as_ref().and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<KeyboardSettings>(&b).ok())
            .filter(|s| s.validate().is_ok()).unwrap_or_default();
        let mut remapper = crate::remap::Service::new(injector.clone());
        let mut api = match HidApi::new() { Ok(a) => a, Err(e) => {
            publish(KeyboardStatus { error: true, message: Some(e.to_string()), ..Default::default() }); return;
        }};
        while !shutdown.load(Ordering::Relaxed) {
            let _ = api.refresh_devices();
            if let Some(mut keyboard) = find(&api) {
                let (tx, rx) = mpsc::channel();
                let mut engine = Engine::new(injector.clone(), tx, profiles::from_dto(settings.profile.clone()).unwrap().profile);
                engine.handle(DeviceEvent::Connected);
                if let Err(error) = activate(&mut keyboard, &mut engine, &mut remapper, &settings) {
                    keyboard.release();
                    let _ = remapper.configure(Default::default());
                    keyboard.status.error = true;
                    keyboard.status.message = Some(error);
                }
                keyboard.status.settings = settings.clone();
                keyboard.status.regular_active = remapper.is_active();
                publish(keyboard.status.clone());
                let mut health = Instant::now();
                let mut reconfigured = Instant::now() - Duration::from_secs(2);
                while !shutdown.load(Ordering::Relaxed) {
                    while let Ok(command) = commands.try_recv() {
                        let next = match command {
                            KeyboardCommand::Apply(next) => next,
                            KeyboardCommand::SwitchBank(bank) => {
                                let mut next = settings.clone();
                                next.active_bank = bank;
                                Box::new(next)
                            }
                            KeyboardCommand::PauseRegular => {
                                let result = remapper.configure(Default::default());
                                keyboard.status.regular_active = remapper.is_active();
                                keyboard.status.error = result.is_err();
                                keyboard.status.message = Some(result.err().unwrap_or_else(|| "Regular-key assignments paused. Apply to resume.".into()));
                                publish(keyboard.status.clone());
                                continue;
                            }
                        };
                        let result = activate(&mut keyboard, &mut engine, &mut remapper, &next).and_then(|()| {
                            let path = settings_path.as_ref().ok_or("No configuration directory")?;
                            if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
                            let temp = path.with_extension("json.tmp");
                            std::fs::write(&temp, serde_json::to_vec_pretty(&next).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
                            std::fs::rename(temp, path).map_err(|e| e.to_string())
                        });
                        if let Err(e) = result {
                            engine.handle(DeviceEvent::Disconnected);
                            keyboard.release();
                            let _ = remapper.configure(Default::default());
                            keyboard.status.error = true;
                            keyboard.status.message = Some(e);
                        } else {
                            settings = *next;
                            keyboard.status.settings = settings.clone();
                            keyboard.status.error = false;
                            keyboard.status.message = Some("G915 settings saved".into());
                        }
                        keyboard.status.held = 0;
                        keyboard.status.regular_active = remapper.is_active();
                        keyboard.status.revision += 1;
                        publish(keyboard.status.clone());
                    }
                    if let Some((feature, function, data)) = keyboard.link.event(Duration::from_millis(10)) {
                        if Some(feature) == keyboard.wireless && lighting::reconfigure_requested(function, &data)
                            && reconfigured.elapsed() >= Duration::from_secs(2) {
                            reconfigured = Instant::now();
                            engine.handle(DeviceEvent::Disconnected);
                            let _ = remapper.configure(Default::default());
                            let result = activate(&mut keyboard, &mut engine, &mut remapper, &settings);
                            keyboard.status.error = result.is_err();
                            keyboard.status.message = result.err();
                            if !keyboard.status.error { engine.handle(DeviceEvent::Connected); }
                            else { keyboard.release(); let _ = remapper.configure(Default::default()); }
                            keyboard.status.held = 0;
                            keyboard.status.regular_active = remapper.is_active();
                            publish(keyboard.status.clone());
                        } else if keyboard.diverted && !keyboard.status.error && function == 0 {
                            if Some(feature) == keyboard.gkeys && settings.enabled {
                                let mask = data[0] & 0x1F;
                                for (key, pressed) in gkey_edges(keyboard.status.held, mask) {
                                    engine.handle(DeviceEvent::Key { key, pressed });
                                }
                                keyboard.status.held = mask;
                            } else if Some(feature) == keyboard.mkeys
                                && let Some(bank) = banks::from_mask(data[0])
                                && bank != settings.active_bank {
                                let mut next = settings.clone();
                                next.active_bank = bank.into();
                                match activate(&mut keyboard, &mut engine, &mut remapper, &next) {
                                    Ok(()) => {
                                        settings = next;
                                        keyboard.status.settings = settings.clone();
                                        keyboard.status.message = Some(format!("G915 profile {bank} active"));
                                    }
                                    Err(error) => {
                                        keyboard.release();
                                        let _ = remapper.configure(Default::default());
                                        keyboard.status.error = true;
                                        keyboard.status.message = Some(error);
                                    }
                                }
                                keyboard.status.regular_active = remapper.is_active();
                            }
                            keyboard.status.bank = engine.active_bank_name().into();
                            publish(keyboard.status.clone());
                        }
                    }
                    engine.tick();
                    // Device output is G13-specific; never send it to this keyboard.
                    while rx.try_recv().is_ok() {}
                    if health.elapsed() >= Duration::from_secs(2) {
                        if (keyboard.diverted || keyboard.claimed || remapper.is_active()) && crate::mouse::worker::GhubWatch::new().running() {
                            engine.handle(DeviceEvent::Disconnected);
                            keyboard.release();
                            let _ = remapper.configure(Default::default());
                            keyboard.status.held = 0;
                            keyboard.status.error = true;
                            keyboard.status.message = Some("G HUB started. Close it, then apply your G915 settings again.".into());
                        }
                        if keyboard.link.call(0, 1, &[0, 0, 0x5A]).is_err() { break; }
                        if let Some(i) = keyboard.battery {
                            keyboard.status.voltage_mv = keyboard.link.call(i, 0, &[]).ok()
                                .map(|d| u16::from_be_bytes([d[0], d[1]]));
                        }
                        keyboard.status.regular_active = remapper.is_active();
                        publish(keyboard.status.clone());
                        health = Instant::now();
                    }
                }
                engine.shutdown();
                let _ = remapper.configure(Default::default());
                keyboard.release();
            }
            publish(KeyboardStatus { settings: settings.clone(), ..Default::default() });
            // Discard stale changes queued before disconnect; they must be explicitly retried.
            while commands.try_recv().is_ok() {}
            for _ in 0..30 {
                if shutdown.load(Ordering::Relaxed) { return; }
                thread::sleep(Duration::from_millis(100));
            }
        }
    })
}

// All bank changes release held outputs and stop macro players before replacing
// either map. A failure leaves input unmapped, never a partially active profile.
fn activate(keyboard: &mut Keyboard, engine: &mut Engine, remapper: &mut crate::remap::Service,
    settings: &KeyboardSettings) -> Result<(), String> {
    settings.validate()?;
    engine.handle(DeviceEvent::Disconnected);
    remapper.configure(Default::default())?;
    keyboard.status.held = 0;
    let effective = settings.for_bank(&settings.active_bank);
    keyboard.apply(&effective)?;
    apply_regular(remapper, &effective)?;
    engine.set_profile(profiles::from_dto(settings.profile.clone()).map_err(|e| e.to_string())?.profile);
    engine.handle(DeviceEvent::Connected);
    let key = banks::key(&settings.active_bank)?;
    engine.handle(DeviceEvent::Key { key, pressed: true });
    engine.handle(DeviceEvent::Key { key, pressed: false });
    keyboard.status.bank = engine.active_bank_name().into();
    Ok(())
}

fn apply_regular(remapper: &mut crate::remap::Service, settings: &KeyboardSettings) -> Result<(), String> {
    let assignments = if settings.regular_enabled {
        crate::remap::compile(&settings.regular_keys, &settings.profile.macros)?
    } else { Default::default() };
    remapper.configure(assignments)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn simultaneous_gkey_changes_release_first_and_ignore_other_bits() {
        assert_eq!(gkey_edges(0b00010, 0b11100001), vec![(G13Key::G2, false), (G13Key::G1, true)]);
        assert!(gkey_edges(0b11111, 0b11111).is_empty());
    }

    #[test]
    fn g915_disconnect_releases_chords_and_profile_edits_do_not_rebind_held_keys() {
        use crate::{keycode::KeyCode, platform::mock::{Injected, RecordingInjector}};
        let dto: FileDto = serde_json::from_str(r#"{"name":"G915","banks":{"M1":{"G1":"Ctrl+C"},"M2":{"G1":"F5"}}}"#).unwrap();
        let injector = Arc::new(RecordingInjector::new());
        let (tx, _rx) = mpsc::channel();
        let mut engine = Engine::new(injector.clone(), tx, profiles::from_dto(dto).unwrap().profile);
        for (key, pressed) in gkey_edges(0, 1) { engine.handle(DeviceEvent::Key { key, pressed }); }
        injector.take();
        engine.handle(DeviceEvent::Key { key: G13Key::M2, pressed: true });
        for (key, pressed) in gkey_edges(1, 0) { engine.handle(DeviceEvent::Key { key, pressed }); }
        assert_eq!(injector.take(), vec![Injected::Key(KeyCode::C, false), Injected::Key(KeyCode::LeftCtrl, false)]);
        for (key, pressed) in gkey_edges(0, 1) { engine.handle(DeviceEvent::Key { key, pressed }); }
        assert_eq!(injector.take(), vec![Injected::Key(KeyCode::F5, true)]);
        engine.handle(DeviceEvent::Disconnected);
        assert_eq!(injector.take(), vec![Injected::Key(KeyCode::F5, false)]);
    }
    #[test]
    fn reject_invalid_settings_before_device_io() {
        let mut s = KeyboardSettings::default();
        assert!(s.validate().is_ok());
        s.brightness = Some(101);
        assert!(s.validate().is_err());
        s.brightness = None;
        s.color = Some("invalid".into());
        assert!(s.validate().is_err());
        s.color = None;
        s.profile.banks.m1.keys.insert("G6".into(), profiles::format::ActionDto::Chord("A".into()));
        assert!(s.validate().is_err());
    }
}
