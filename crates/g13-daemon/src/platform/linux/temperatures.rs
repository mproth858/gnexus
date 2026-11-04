//! Standard Linux hwmon sensors; no privileged access or hardware writes.
use std::{fs, path::Path};
use crate::platform::temperatures::{Kind, Provider, Reading};
pub struct LinuxTemperatures;
impl Provider for LinuxTemperatures {
    fn read(&mut self) -> Vec<Reading> {
        let mut readings = Vec::new();
        let Ok(devices) = fs::read_dir("/sys/class/hwmon") else { return readings };
        for device in devices.flatten() {
            let dir = device.path();
            let hardware = text(&dir.join("name"));
            let kind = match hardware.as_str() {
                "coretemp" | "k10temp" | "zenpower" => Kind::Cpu,
                "amdgpu" | "nouveau" => Kind::Gpu, _ => Kind::Other,
            };
            let Ok(files) = fs::read_dir(&dir) else { continue };
            for file in files.flatten() {
                let filename = file.file_name().to_string_lossy().into_owned();
                if !filename.starts_with("temp") || !filename.ends_with("_input") { continue; }
                let Ok(value) = text(&file.path()).parse::<f32>() else { continue };
                let label = text(&dir.join(filename.replace("_input", "_label")));
                let name = if !label.is_empty() { label } else if kind == Kind::Gpu { "GPU Core".into() } else { filename.replace("_input", "") };
                let reading = Reading { id: file.path().to_string_lossy().into_owned(), name, hardware: hardware.clone(), kind, celsius: value / 1000.0 };
                if reading.valid() { readings.push(reading); }
            }
        }
        readings.sort_by(|a, b| a.id.cmp(&b.id));
        readings
    }
}
fn text(path: &Path) -> String { fs::read_to_string(path).unwrap_or_default().trim().into() }
