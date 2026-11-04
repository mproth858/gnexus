//! Yeti GX (046d:0afc): a dedicated HID owner, with readback after every control
//! write. Audio settings are never restored automatically (especially unmute).
pub mod protocol;
use std::{collections::BTreeMap, path::PathBuf, sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc::Receiver}, thread::{self, JoinHandle}, time::{Duration, Instant}};
use hidapi::HidApi;
use serde::{Deserialize, Serialize};
use logi_hidpp::{DIRECT, features as f};
use crate::{mouse::Link, platform};
pub const PRODUCT_ID: u16 = 0x0afc;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// None leaves the microphone's hardware lighting in control. Zone indexes
    /// are discovered, not assumed to mean ring/logo before a physical test.
    pub colors: Option<BTreeMap<u8, String>>,
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(colors) = &self.colors {
            if colors.is_empty() { return Err("Choose a color for at least one lighting zone".into()); }
            for color in colors.values() { crate::mouse::settings::parse_color(color)?; }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Zone { pub id: u8, pub static_effect: u8 }
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Status {
    pub present: bool,
    pub connected: bool,
    pub name: String,
    pub gain: Option<u8>,
    pub muted: Option<bool>,
    pub smart_lock: Option<bool>,
    pub zones: Vec<Zone>,
    pub settings: Settings,
    pub lighting_active: bool,
    pub input: Option<platform::CaptureLevel>,
    pub input_supported: bool,
    pub input_error: Option<String>,
    pub message: Option<String>,
    pub error: bool,
    pub revision: u64,
}
pub enum Command { Control(protocol::Control), Lighting(Settings) }

struct Device { link: Link, control: Option<u8>, rgb: Option<u8>, claimed: bool, status: Status }
fn feature(link: &Link, id: u16) -> Option<u8> {
    f::root_get_feature(&link.call_short(0, 0, &id.to_be_bytes()).ok()?).map(|f| f.index)
}
fn find(api: &HidApi, sw_id: u8) -> Option<Device> {
    for info in api.device_list().filter(|i| i.vendor_id() == 0x046d && i.product_id() == PRODUCT_ID && i.usage_page() == 0xff43) {
        let Ok(dev) = info.open_device(api) else { continue };
        let link = Link { dev, index: DIRECT, timeout: Duration::from_millis(350), sw_id, events: Default::default() };
        if !link.call_short(0, 1, &[0, 0, 0x5a]).is_ok_and(|data| data[0] >= 2 && data[2] == 0x5a) { continue; }
        let control = feature(&link, protocol::FEATURE);
        let rgb = feature(&link, f::RGB_EFFECTS);
        let zones = rgb.map(|index| zones(&link, index)).unwrap_or_default();
        let mut device = Device { link, control, rgb, claimed: false, status: Status { present: true, connected: true,
            name: "Yeti GX".into(), input_supported: cfg!(windows), zones, ..Default::default() } };
        device.refresh();
        return Some(device);
    }
    None
}

fn zones(link: &Link, feature: u8) -> Vec<Zone> {
    let Ok(info) = link.call(feature, 0, &[0xff, 0xff, 0]) else { return Vec::new() };
    let count = info[2];
    if count == 0 || count > 8 { return Vec::new(); }
    let mut zones = Vec::new();
    for id in 0..count {
        let Ok(info) = link.call(feature, 0, &[id, 0xff, 0]) else { continue };
        for effect in 0..info[4].min(32) {
            if link.call(feature, 0, &[id, effect, 0]).is_ok_and(|d| u16::from_be_bytes([d[2], d[3]]) == f::rgb::EFFECT_STATIC) {
                zones.push(Zone { id, static_effect: effect }); break;
            }
        }
    }
    zones
}

fn lighting_requests(settings: &Settings, zones: &[Zone]) -> Result<Vec<[u8; 13]>, String> {
    settings.validate()?;
    let Some(colors) = &settings.colors else { return Ok(Vec::new()) };
    colors.iter().map(|(id, color)| {
        let zone = zones.iter().find(|z| z.id == *id).ok_or("A requested lighting zone is not supported by this microphone")?;
        let rgb = crate::mouse::settings::parse_color(color)?;
        Ok(f::rgb::cluster_effect(zone.id, zone.static_effect, &f::rgb::LiveEffect::Static(rgb)))
    }).collect()
}

impl Device {
    fn read(&self, field: u8) -> Option<u8> {
        let index = self.control?;
        protocol::value(&self.link.call_short(index, protocol::READ, &[0, field]).ok()?, field).ok()
    }
    fn refresh(&mut self) {
        self.status.gain = self.read(1);
        self.status.muted = self.read(0).map(|v| v == 0);
        self.status.smart_lock = self.read(2).map(|v| v != 0);
    }
    fn release(&mut self) {
        if self.claimed {
            if let Some(index) = self.rgb { let _ = self.link.call(index, 5, &f::rgb::SW_CONTROL_RELEASE); }
            self.claimed = false;
            self.status.lighting_active = false;
        }
    }
    fn control(&mut self, control: protocol::Control) -> Result<(), String> {
        let params = control.params()?;
        let available = match control { protocol::Control::Gain(_) => self.status.gain.is_some(),
            protocol::Control::Mute(_) => self.status.muted.is_some(), protocol::Control::SmartLock(_) => self.status.smart_lock.is_some() };
        if !available { return Err("This microphone control is not available on the connected firmware".into()); }
        self.link.call_short(self.control.ok_or("Microphone controls are unavailable")?, protocol::WRITE, &params)?;
        for attempt in 0..4 {
            self.refresh();
            let confirmed = match control { protocol::Control::Gain(v) => self.status.gain == Some(v),
                protocol::Control::Mute(v) => self.status.muted == Some(v), protocol::Control::SmartLock(v) => self.status.smart_lock == Some(v) };
            if confirmed { return Ok(()); }
            if attempt < 3 { thread::sleep(Duration::from_millis(30)); }
        }
        Err("The microphone did not confirm the requested setting. Its current state is shown.".into())
    }
    fn lighting(&mut self, settings: &Settings) -> Result<(), String> {
        settings.validate()?;
        if settings.colors.is_none() { self.release(); return Ok(()); }
        // Build/validate the complete plan before claiming control or writing.
        let requests = lighting_requests(settings, &self.status.zones)?;
        let index = self.rgb.ok_or("RGB lighting is not exposed by this microphone")?;
        if !self.claimed {
            self.claimed = true;
            self.link.call(index, 5, &f::rgb::SW_CONTROL_TAKE)?;
        }
        for request in requests {
            self.link.call(index, 1, &request)?;
        }
        self.status.lighting_active = true;
        Ok(())
    }
}

fn input(status: &mut Status) {
    match platform::capture_level("Yeti GX") {
        Ok(level) => { status.input = level; status.input_error = None; }
        Err(error) => { status.input = None; status.input_error = Some(error.detail()); }
    }
}
fn offline(api: &HidApi, settings: &Settings) -> Status {
    let present = api.device_list().any(|i| i.vendor_id() == 0x046d && i.product_id() == PRODUCT_ID);
    Status { present, name: "Yeti GX".into(), input_supported: cfg!(windows), settings: settings.clone(),
        message: Some(if present { "Yeti GX detected, but its control interface is unavailable. Close G HUB and reconnect USB." }
            else { "Yeti GX not connected. Connect the microphone using USB." }.into()), ..Default::default() }
}

/// Read-only inventory for the first physical test; no configuration is applied.
pub fn probe() -> Result<serde_json::Value, String> {
    let api = HidApi::new().map_err(|e| e.to_string())?;
    let interfaces: Vec<_> = api.device_list().filter(|i| i.vendor_id() == 0x046d && (i.product_id() == PRODUCT_ID || i.product_string().unwrap_or("").to_ascii_lowercase().contains("yeti")))
        .map(|i| serde_json::json!({"product_id":i.product_id(),"product":i.product_string(),"interface":i.interface_number(),"usage_page":i.usage_page(),"usage":i.usage()})).collect();
    let mut features = Vec::new();
    let mut fields = Vec::new();
    let mut rgb_info = None;
    let mut status = if let Some(device) = find(&api, 11) {
        if let Some(index) = feature(&device.link, f::FEATURE_SET)
            && let Ok(info) = device.link.call_short(index, 0, &[]) {
            for i in 1..=info[0].min(64) {
                if let Ok(data) = device.link.call_short(index, 1, &[i]) {
                    let id = u16::from_be_bytes([data[0], data[1]]);
                    features.push(serde_json::json!({"index":i,"id":format!("0x{id:04X}"),"name":f::name(id)}));
                }
            }
        }
        if let Some(index) = device.control {
            for field in [0, 1, 2] {
                let result = device.link.call_short(index, protocol::READ, &[0, field]);
                fields.push(serde_json::json!({"field":field,"reply":result.as_ref().ok(),"error":result.err()}));
            }
        }
        if let Some(index) = device.rgb { rgb_info = device.link.call(index, 0, &[0xff, 0xff, 0]).ok(); }
        device.status
    } else { offline(&api, &Settings::default()) };
    input(&mut status);
    Ok(serde_json::json!({"usb_id":"046d:0afc","interfaces":interfaces,"features":features,"microphone_fields":fields,"rgb_info":rgb_info,"status":status}))
}

pub fn spawn(commands: Receiver<Command>, publish: impl Fn(Status) + Send + 'static, shutdown: Arc<AtomicBool>, config: Option<PathBuf>) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new().name("yeti-gx".into()).spawn(move || {
        let path = config.map(|dir| dir.join("microphone.json"));
        let mut settings: Settings = path.as_ref().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok())
            .filter(|s: &Settings| s.validate().is_ok()).unwrap_or_default();
        let Ok(mut api) = HidApi::new() else { return };
        while !shutdown.load(Ordering::Relaxed) {
            let _ = api.refresh_devices();
            if let Some(mut device) = find(&api, 12) {
                device.status.settings = settings.clone();
                if settings.colors.is_some() && !crate::mouse::worker::GhubWatch::new().running()
                    && let Err(error) = device.lighting(&settings) {
                    device.release(); device.status.message = Some(error); device.status.error = true;
                }
                let mut refresh = Instant::now() - Duration::from_secs(1);
                let mut health = Instant::now();
                while !shutdown.load(Ordering::Relaxed) {
                    while let Ok(command) = commands.try_recv() {
                        let result = if crate::mouse::worker::GhubWatch::new().running() { Err("Close G HUB before changing Yeti GX settings".into()) }
                        else { match command {
                            Command::Control(control) => device.control(control),
                            Command::Lighting(next) => device.lighting(&next).and_then(|()| {
                                let path = path.as_ref().ok_or("No configuration directory")?;
                                if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
                                let temp = path.with_extension("json.tmp");
                                std::fs::write(&temp, serde_json::to_vec_pretty(&next).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
                                std::fs::rename(temp, path).map_err(|e| e.to_string())?;
                                settings = next; device.status.settings = settings.clone(); Ok(())
                            }),
                        }};
                        device.status.error = result.is_err();
                        device.status.message = Some(result.err().unwrap_or_else(|| "Yeti GX settings applied".into()));
                        if device.status.error { device.release(); }
                        device.status.revision += 1;
                        publish(device.status.clone());
                    }
                    if refresh.elapsed() >= Duration::from_millis(750) {
                        device.refresh(); refresh = Instant::now();
                    }
                    if health.elapsed() >= Duration::from_secs(2) {
                        if device.link.call_short(0, 1, &[0, 0, 0x5a]).is_err() { break; }
                        if device.claimed && crate::mouse::worker::GhubWatch::new().running() {
                            device.release(); device.status.error = true; device.status.message = Some("G HUB started. Yeti GX lighting control released.".into());
                        }
                        health = Instant::now();
                    }
                    input(&mut device.status);
                    publish(device.status.clone());
                    // Notifications include a different value offset; always
                    // re-read authoritative fields instead of treating them as replies.
                    if let Some((index, _, _)) = device.link.event(Duration::from_millis(100))
                        && Some(index) == device.control { refresh = Instant::now() - Duration::from_secs(1); }
                }
                device.release();
            }
            let mut status = offline(&api, &settings); input(&mut status); publish(status);
            while commands.try_recv().is_ok() {} // never replay stale writes on reconnect
            for _ in 0..15 { if shutdown.load(Ordering::Relaxed) { return; } thread::sleep(Duration::from_millis(100)); }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lighting_configuration_rejects_empty_or_invalid_colors() {
        assert!(Settings::default().validate().is_ok());
        assert!(Settings { colors: Some(BTreeMap::new()) }.validate().is_err());
        assert!(Settings { colors: Some(BTreeMap::from([(0, "#123456".into())])) }.validate().is_ok());
        assert!(Settings { colors: Some(BTreeMap::from([(0, "red".into())])) }.validate().is_err());
    }
    #[test]
    fn rgb_plan_uses_discovered_zone_and_effect_indexes_and_rejects_unknown_zones() {
        let settings = Settings { colors: Some(BTreeMap::from([(4, "#123456".into())])) };
        let requests = lighting_requests(&settings, &[Zone { id: 4, static_effect: 7 }]).unwrap();
        assert_eq!(&requests[0][..6], &[4, 7, 0x12, 0x34, 0x56, 2]);
        assert!(lighting_requests(&settings, &[Zone { id: 0, static_effect: 1 }]).is_err());
        assert!(lighting_requests(&Settings::default(), &[]).unwrap().is_empty());
    }
}
