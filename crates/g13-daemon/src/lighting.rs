//! SignalRGB canvas bridge. Only device workers write HID; this module selects
//! their color source. UDP accepts bounded color frames on loopback, never commands.
use std::{collections::{BTreeMap, BTreeSet}, net::UdpSocket, path::PathBuf, sync::{Arc, Mutex, OnceLock, atomic::{AtomicBool, Ordering}}, thread::{self, JoinHandle}, time::{Duration, Instant}};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

pub const PORT: u16 = 19738;
pub const COMMAND_PORT: u16 = 19739;
const FRESH: Duration = Duration::from_secs(2);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Device { G13, G915, Mouse, Microphone }
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode { #[default] Auto, Signalrgb, Nexus }
pub fn parse_mode(value: &str) -> Result<Mode, String> {
    match value { "auto" => Ok(Mode::Auto), "signalrgb" => Ok(Mode::Signalrgb), "nexus" => Ok(Mode::Nexus),
        _ => Err("Lighting mode must be auto, signalrgb or nexus".into()) }
}
/// Early CLI path: never starts a second HID runtime or opens the main window.
pub fn remote_select(mode: Mode) -> Result<(), String> {
    let mut stream = std::net::TcpStream::connect_timeout(&std::net::SocketAddr::from(([127,0,0,1], COMMAND_PORT)), Duration::from_secs(1))
        .map_err(|_| "G Nexus is not running or its lighting command service is unavailable".to_string())?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).map_err(|e| e.to_string())?;
    stream.set_write_timeout(Some(Duration::from_secs(2))).map_err(|e| e.to_string())?;
    let word = match mode { Mode::Auto => "auto", Mode::Signalrgb => "signalrgb", Mode::Nexus => "nexus" };
    stream.write_all(format!("GNEXUS-LIGHTING/1 {word}\n").as_bytes()).map_err(|e| e.to_string())?;
    let mut reply = String::new(); stream.take(1024).read_to_string(&mut reply).map_err(|e| e.to_string())?;
    if reply == "OK\n" { Ok(()) } else { Err(reply.trim().to_string()) }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings { pub enabled: bool, pub idle_minutes: u32, pub devices: BTreeSet<Device> }
impl Default for Settings {
    fn default() -> Self { Self { enabled: false, idle_minutes: 10, devices: [Device::G13, Device::G915, Device::Mouse, Device::Microphone].into() } }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=240).contains(&self.idle_minutes) { return Err("Idle timeout must be between 1 and 240 minutes".into()); }
        Ok(())
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Packet { version: u8, device: Device, colors: Vec<[u8; 4]> }
#[derive(Debug, Clone)]
pub struct Frame { pub colors: BTreeMap<u8, [u8; 3]>, pub sequence: u64 }
struct LiveFrame { frame: Frame, received: Instant }
#[derive(Default)]
struct State { settings: Settings, mode: Mode, idle: Option<Duration>, last_device_input: Option<Instant>, frames: BTreeMap<Device, LiveFrame>, sequence: u64, error: Option<String>, device_errors: BTreeMap<Device, String> }
#[derive(Default)]
pub struct Hub { state: Mutex<State>, path: Mutex<Option<PathBuf>> }
#[derive(Serialize)]
pub struct Status {
    pub settings: Settings, pub mode: Mode, pub idle_seconds: Option<u64>, pub idle_supported: bool,
    pub signal_requested: bool, pub streaming: Vec<Device>, pub error: Option<String>, pub port: u16,
    pub device_errors: BTreeMap<Device, String>,
}
static HUB: OnceLock<Hub> = OnceLock::new();
pub fn hub() -> &'static Hub { HUB.get_or_init(Hub::default) }
fn requested(state: &State) -> bool {
    state.settings.enabled && match state.mode { Mode::Nexus => false, Mode::Signalrgb => true,
        Mode::Auto => state.idle.is_some_and(|idle| idle >= Duration::from_secs(u64::from(state.settings.idle_minutes) * 60)) }
}
impl Hub {
    pub fn configure(&self, settings: Settings) -> Result<(), String> {
        settings.validate()?;
        let mut state = self.state.lock().unwrap();
        if let Some(path) = self.path.lock().unwrap().as_ref() {
            if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            std::fs::rename(tmp, path).map_err(|e| e.to_string())?;
        }
        state.settings = settings;
        if !state.settings.enabled { state.mode = Mode::Auto; state.frames.clear(); state.device_errors.clear(); }
        Ok(())
    }
    pub fn select(&self, mode: Mode) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        if mode == Mode::Signalrgb && !state.settings.enabled { return Err("Enable the SignalRGB bridge in Settings first".into()); }
        state.mode = mode; Ok(())
    }
    fn ingest(&self, bytes: &[u8], now: Instant) -> bool {
        let Ok(packet) = serde_json::from_slice::<Packet>(bytes) else { return false };
        if packet.version != 1 || packet.colors.is_empty() || packet.colors.len() > 256 { return false; }
        let colors: BTreeMap<_, _> = packet.colors.iter().map(|[id,r,g,b]| (*id,[*r,*g,*b])).collect();
        if colors.len() != packet.colors.len() { return false; }
        let mut state = self.state.lock().unwrap();
        if !state.settings.enabled { return false; }
        state.sequence = state.sequence.wrapping_add(1);
        let sequence = state.sequence;
        state.frames.insert(packet.device, LiveFrame { frame: Frame { colors, sequence }, received: now }); true
    }
    pub fn frame(&self, device: Device) -> Option<Frame> { self.frame_at(device, Instant::now()) }
    /// Diverted G/M keys may never become normal Windows keyboard input.
    pub fn activity(&self) {
        let mut state = self.state.lock().unwrap();
        state.last_device_input = Some(Instant::now());
        if state.idle.is_some() { state.idle = Some(Duration::ZERO); }
    }
    pub fn report(&self, device: Device, result: &Result<(), String>) {
        let mut state = self.state.lock().unwrap();
        if let Err(error) = result { state.device_errors.insert(device, error.clone()); }
        else { state.device_errors.remove(&device); }
    }
    fn frame_at(&self, device: Device, now: Instant) -> Option<Frame> {
        let state = self.state.lock().unwrap();
        if !requested(&state) || !state.settings.devices.contains(&device) { return None; }
        state.frames.get(&device).filter(|f| now.saturating_duration_since(f.received) < FRESH).map(|f| f.frame.clone())
    }
    pub fn status(&self) -> Status {
        let state = self.state.lock().unwrap();
        Status { settings: state.settings.clone(), mode: state.mode, idle_seconds: state.idle.map(|d| d.as_secs()), idle_supported: cfg!(windows),
            signal_requested: requested(&state), streaming: state.frames.iter().filter(|(_,f)| f.received.elapsed() < FRESH).map(|(d,_)| *d).collect(), error: state.error.clone(), device_errors: state.device_errors.clone(), port: PORT }
    }
}

pub fn spawn(config: Option<PathBuf>, shutdown: Arc<AtomicBool>) -> std::io::Result<JoinHandle<()>> {
    if let Some(path) = config.map(|p| p.join("lighting.json")) {
        let settings = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice::<Settings>(&b).ok()).filter(|s| s.validate().is_ok()).unwrap_or_default();
        hub().state.lock().unwrap().settings = settings;
        *hub().path.lock().unwrap() = Some(path);
    }
    thread::Builder::new().name("lighting-bridge".into()).spawn(move || {
        let commands = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, COMMAND_PORT))
            .and_then(|s| { s.set_nonblocking(true)?; Ok(s) }).map_err(|e| log::warn!("Lighting commands unavailable: {e}")).ok();
        let mut socket = None;
        let mut retry = Instant::now() - Duration::from_secs(5);
        let mut buf = [0u8; 8192];
        while !shutdown.load(Ordering::Relaxed) {
            if let Some(listener) = &commands { for _ in 0..4 {
                let Ok((mut client, _)) = listener.accept() else { break };
                let _ = client.set_read_timeout(Some(Duration::from_millis(100)));
                let _ = client.set_write_timeout(Some(Duration::from_millis(100)));
                let mut line = Vec::new();
                for _ in 0..64 { let mut byte = [0]; if client.read_exact(&mut byte).is_err() { break; }
                    if byte[0] == b'\n' { break; } line.push(byte[0]); }
                let result = std::str::from_utf8(&line).ok().and_then(|s| s.strip_prefix("GNEXUS-LIGHTING/1 "))
                    .ok_or_else(|| "Invalid lighting command".to_string()).and_then(parse_mode).and_then(|m| hub().select(m));
                let _ = client.write_all(result.err().map_or("OK\n".into(), |e| format!("{e}\n")).as_bytes());
            }}
            let enabled = {
                let mut state = hub().state.lock().unwrap();
                state.idle = crate::platform::idle_time().map(|idle| state.last_device_input.map_or(idle, |last| idle.min(last.elapsed())));
                state.settings.enabled
            };
            if !enabled { socket = None; hub().state.lock().unwrap().frames.clear(); }
            else if socket.is_none() && retry.elapsed() >= Duration::from_secs(5) {
                retry = Instant::now();
                match UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, PORT)).and_then(|s| { s.set_nonblocking(true)?; Ok(s) }) {
                    Ok(s) => { socket = Some(s); hub().state.lock().unwrap().error = None; }
                    Err(e) => { hub().state.lock().unwrap().error = Some(format!("Cannot open SignalRGB bridge: {e}")); }
                }
            }
            if let Some(socket) = &socket {
                // Bound work per tick even under a noisy sender.
                for _ in 0..64 { match socket.recv_from(&mut buf) {
                    Ok((size, peer)) if peer.ip().is_loopback() => { hub().ingest(&buf[..size], Instant::now()); }
                    _ => break,
                }}
            }
            thread::sleep(Duration::from_millis(50));
        }
        let mut state = hub().state.lock().unwrap(); state.frames.clear(); state.mode = Mode::Nexus;
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_handoff_resume_manual_override_and_stream_failure() {
        let h = Hub::default(); let now = Instant::now();
        h.configure(Settings { enabled:true, ..Default::default() }).unwrap();
        assert!(h.ingest(br#"{"version":1,"device":"g915","colors":[[1,20,30,40]]}"#, now));
        h.state.lock().unwrap().idle = Some(Duration::from_secs(599));
        assert!(h.frame_at(Device::G915, now).is_none());
        h.state.lock().unwrap().idle = Some(Duration::from_secs(600));
        assert_eq!(h.frame_at(Device::G915, now).unwrap().colors[&1], [20,30,40]);
        h.state.lock().unwrap().idle = Some(Duration::ZERO);
        assert!(h.frame_at(Device::G915, now).is_none());
        h.select(Mode::Signalrgb).unwrap(); assert!(h.frame_at(Device::G915, now).is_some());
        assert!(h.frame_at(Device::G915, now + FRESH).is_none());
        h.select(Mode::Nexus).unwrap(); h.state.lock().unwrap().idle = Some(Duration::from_secs(900));
        assert!(h.frame_at(Device::G915, now).is_none());
        h.select(Mode::Auto).unwrap(); assert!(h.frame_at(Device::G915, now).is_some());
        assert!(h.frame_at(Device::Mouse, now).is_none());
    }
    #[test]
    fn frames_cannot_change_modes_settings_or_accept_ambiguous_colors() {
        let h = Hub::default(); let now = Instant::now();
        assert!(!h.ingest(br#"{"version":1,"device":"g915","colors":[[1,2,3,4]]}"#, now));
        h.configure(Settings { enabled:true, devices:[Device::G13].into(), ..Default::default() }).unwrap();
        h.select(Mode::Signalrgb).unwrap();
        for bad in [br#"{"version":2,"device":"g13","colors":[[0,1,2,3]]}"#.as_slice(),
            br#"{"version":1,"device":"g13","colors":[[0,1,2,3],[0,4,5,6]]}"#,
            br#"{"version":1,"device":"g13","colors":[],"mode":"signalrgb"}"#] { assert!(!h.ingest(bad, now)); }
        assert!(h.ingest(br#"{"version":1,"device":"g915","colors":[[1,2,3,4]]}"#, now));
        assert!(h.frame_at(Device::G915, now).is_none());
        h.configure(Settings::default()).unwrap(); assert_eq!(h.status().mode, Mode::Auto);
    }
}
