//! Read-only NVML and Libre/Open Hardware Monitor WMI access. This module
//! installs no kernel driver and never changes fan, voltage or clock settings.
use std::{collections::BTreeMap, ffi::{CStr, c_char, c_void}};
use serde::Deserialize;
use windows_sys::Win32::{Foundation::{FreeLibrary, HMODULE}, System::LibraryLoader::{GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS}};
use wmi::WMIConnection;
use crate::platform::temperatures::{Kind, Provider, Reading};

pub struct WindowsTemperatures { nvml: Option<Nvml> }
impl WindowsTemperatures { pub fn new() -> Self { Self { nvml: Nvml::open() } } }
impl Default for WindowsTemperatures { fn default() -> Self { Self::new() } }

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Sensor { identifier: String, name: String, parent: String, value: Option<f32> }
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Hardware { identifier: String, name: String }

impl Provider for WindowsTemperatures {
    fn read(&mut self) -> Vec<Reading> {
        let mut readings = super::aida64::read();
        let aida_gpu = crate::platform::temperatures::primary(&readings, Kind::Gpu).is_some();
        if readings.is_empty() { readings = bridge_readings(); }
        for namespace in ["ROOT\\LibreHardwareMonitor", "ROOT\\OpenHardwareMonitor"] {
            if !readings.is_empty() { break; }
            let Ok(connection) = WMIConnection::with_namespace_path(namespace) else { continue };
            let Ok(sensors) = connection.raw_query::<Sensor>("SELECT Identifier, Name, Parent, Value FROM Sensor WHERE SensorType = 'Temperature'") else { continue };
            let hardware: BTreeMap<_, _> = connection.raw_query::<Hardware>("SELECT Identifier, Name FROM Hardware").unwrap_or_default()
                .into_iter().map(|h| (h.identifier, h.name)).collect();
            for sensor in sensors {
                let Some(celsius) = sensor.value else { continue };
                let identifier = sensor.identifier.to_ascii_lowercase();
                let kind = if identifier.contains("cpu/") { Kind::Cpu }
                    else if identifier.contains("gpu-") { Kind::Gpu } else { Kind::Other };
                let reading = Reading { id: format!("{namespace}:{}", sensor.identifier), name: sensor.name,
                    hardware: hardware.get(&sensor.parent).cloned().unwrap_or(sensor.parent), kind, celsius };
                if reading.valid() { readings.push(reading); }
            }
            // Prefer one complete monitor, rather than duplicating every sensor.
            if !readings.is_empty() { break; }
        }
        if self.nvml.is_none() { self.nvml = Nvml::open(); }
        if let Some(nvml) = &self.nvml && !aida_gpu {
            let direct = nvml.read();
            // The helper and NVML may both report the same GPU core. Keep the
            // direct reading and retain distinct hotspot/memory/iGPU sensors.
            readings.retain(|r| !(r.kind == Kind::Gpu && r.name.eq_ignore_ascii_case("GPU Core")
                && direct.iter().any(|gpu| gpu.hardware.eq_ignore_ascii_case(&r.hardware))));
            readings.extend(direct);
        }
        readings.sort_by(|a, b| a.hardware.cmp(&b.hardware).then(a.name.cmp(&b.name)).then(a.id.cmp(&b.id)));
        readings
    }
}

#[derive(Deserialize)]
struct Bridge { updated_unix_ms: u64, readings: Vec<Reading> }
fn bridge_readings() -> Vec<Reading> {
    let Some(path) = crate::profiles::store::default_dir().and_then(|dir| dir.parent().map(|p| p.join("temperatures.json"))) else { return Vec::new() };
    if !std::fs::metadata(&path).is_ok_and(|m| m.len() <= 1_048_576) { return Vec::new(); }
    let Ok(bytes) = std::fs::read(path) else { return Vec::new() };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    parse_bridge(&bytes, now)
}
fn parse_bridge(bytes: &[u8], now: u64) -> Vec<Reading> {
    let Ok(bridge) = serde_json::from_slice::<Bridge>(bytes) else { return Vec::new() };
    if now.saturating_sub(bridge.updated_unix_ms) > 6_000 || bridge.updated_unix_ms.saturating_sub(now) > 1_000 { return Vec::new(); }
    bridge.readings.into_iter().filter(Reading::valid).collect()
}

type Init = unsafe extern "C" fn() -> i32;
type Shutdown = unsafe extern "C" fn() -> i32;
type Count = unsafe extern "C" fn(*mut u32) -> i32;
type Handle = unsafe extern "C" fn(u32, *mut *mut c_void) -> i32;
type Temperature = unsafe extern "C" fn(*mut c_void, u32, *mut u32) -> i32;
type Name = unsafe extern "C" fn(*mut c_void, *mut c_char, u32) -> i32;

struct Nvml { library: HMODULE, shutdown: Shutdown, count: Count, handle: Handle, temperature: Temperature, name: Name }
impl Nvml {
    fn open() -> Option<Self> {
        // Only use absolute driver paths. Never search the working directory.
        let paths = [std::env::var_os("WINDIR").map(|p| std::path::PathBuf::from(p).join("System32/nvml.dll")),
            std::env::var_os("ProgramFiles").map(|p| std::path::PathBuf::from(p).join("NVIDIA Corporation/NVSMI/nvml.dll"))];
        for path in paths.into_iter().flatten() {
            use std::os::windows::ffi::OsStrExt;
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            // SAFETY: null-terminated absolute path; library stays loaded until Drop.
            let library = unsafe { LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS) };
            if library.is_null() { continue; }
            // SAFETY: NVIDIA's documented C ABI. Every required symbol is checked
            // before transmuting, and no function outlives its driver library.
            let loaded = unsafe {
                (|| {
                    let init: Init = std::mem::transmute(GetProcAddress(library, c"nvmlInit_v2".as_ptr().cast())?);
                    let shutdown: Shutdown = std::mem::transmute(GetProcAddress(library, c"nvmlShutdown".as_ptr().cast())?);
                    let count: Count = std::mem::transmute(GetProcAddress(library, c"nvmlDeviceGetCount_v2".as_ptr().cast())?);
                    let handle: Handle = std::mem::transmute(GetProcAddress(library, c"nvmlDeviceGetHandleByIndex_v2".as_ptr().cast())?);
                    let temperature: Temperature = std::mem::transmute(GetProcAddress(library, c"nvmlDeviceGetTemperature".as_ptr().cast())?);
                    let name: Name = std::mem::transmute(GetProcAddress(library, c"nvmlDeviceGetName".as_ptr().cast())?);
                    (init() == 0).then_some(Self { library, shutdown, count, handle, temperature, name })
                })()
            };
            if loaded.is_some() { return loaded; }
            // SAFETY: failed loads have no live function pointers or NVML owner.
            unsafe { FreeLibrary(library); }
        }
        None
    }

    fn read(&self) -> Vec<Reading> {
        let mut count = 0;
        let mut out = Vec::new();
        // SAFETY: all pointers address initialized output buffers; device handles
        // are obtained fresh each poll and only used after a successful call.
        unsafe {
            if (self.count)(&mut count) != 0 { return out; }
            for i in 0..count.min(32) {
                let mut device = std::ptr::null_mut();
                let mut temp = 0;
                let mut name = [0 as c_char; 96];
                if (self.handle)(i, &mut device) != 0 || (self.temperature)(device, 0, &mut temp) != 0 { continue; }
                let hardware = if (self.name)(device, name.as_mut_ptr(), name.len() as u32) == 0 {
                    name[95] = 0;
                    CStr::from_ptr(name.as_ptr()).to_string_lossy().into_owned()
                } else { format!("NVIDIA GPU {}", i + 1) };
                let reading = Reading { id: format!("nvml:{i}"), name: "GPU Core".into(), hardware, kind: Kind::Gpu, celsius: temp as f32 };
                if reading.valid() { out.push(reading); }
            }
        }
        out
    }
}
impl Drop for Nvml {
    fn drop(&mut self) {
        // SAFETY: balances one successful nvmlInit_v2 and one library load.
        unsafe { (self.shutdown)(); FreeLibrary(self.library); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bridge_requires_recent_timestamp_and_valid_values() {
        let payload = br#"{"updated_unix_ms":10000,"readings":[{"id":"/amdcpu/0/temperature/0","name":"CPU (Tctl/Tdie)","hardware":"Ryzen","kind":"cpu","celsius":65.5}]}"#;
        assert_eq!(parse_bridge(payload, 12_000)[0].celsius, 65.5);
        assert!(parse_bridge(payload, 17_000).is_empty());
        assert!(parse_bridge(payload, 1_000).is_empty());
        assert!(parse_bridge(b"not json", 10_000).is_empty());
    }
}

