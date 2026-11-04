//! Temperature readings only. Never infer CPU temperature from ACPI thermal
//! zones: those can describe a motherboard zone rather than the processor.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind { Cpu, Gpu, Other }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    pub id: String,
    pub name: String,
    pub hardware: String,
    pub kind: Kind,
    pub celsius: f32,
}

impl Reading {
    pub fn valid(&self) -> bool {
        let name = self.name.to_ascii_lowercase();
        // Some providers label thresholds and sensor resolution as Temperature
        // too. They are metadata, not current component measurements.
        let metadata = ["limit", "resolution", "warning", "critical", "distance to", "tjmax", "tj max"]
            .iter().any(|label| name.contains(label));
        !metadata && self.celsius.is_finite() && match self.kind {
            Kind::Cpu | Kind::Gpu => (0.0..=150.0).contains(&self.celsius) && self.celsius != 0.0,
            Kind::Other => (-40.0..=200.0).contains(&self.celsius),
        }
    }
}

pub trait Provider { fn read(&mut self) -> Vec<Reading>; }

pub fn provider() -> Box<dyn Provider> {
    #[cfg(windows)] { Box::new(super::windows::temperatures::WindowsTemperatures::new()) }
    #[cfg(target_os = "linux")] { Box::new(super::linux::temperatures::LinuxTemperatures) }
    #[cfg(not(any(windows, target_os = "linux")))] {
        struct Unavailable;
        impl Provider for Unavailable { fn read(&mut self) -> Vec<Reading> { Vec::new() } }
        Box::new(Unavailable)
    }
}

/// Prefer CPU package/control temperature and the dedicated GPU's core sensor.
/// Hotspot and memory sensors remain available on the additional sensor pages.
pub fn primary(readings: &[Reading], kind: Kind) -> Option<&Reading> {
    let rank = |r: &Reading| {
        let name = r.name.to_ascii_lowercase();
        match kind {
            Kind::Cpu if name.contains("tctl/tdie") => Some(0),
            Kind::Cpu if name.contains("package") => Some(1),
            Kind::Cpu if name.contains("die (average)") => Some(2),
            Kind::Cpu if name.contains("tdie") || name.contains("tctl") => Some(3),
            Kind::Cpu => Some(100),
            Kind::Gpu if r.id.starts_with("nvml:") => Some(0),
            Kind::Gpu if r.id.strip_prefix("aida:TGPU").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())) => Some(1),
            Kind::Gpu if name == "gpu core" || name == "edge" => Some(1),
            Kind::Gpu if name == "gpu" || name == "gpu temperature" => Some(2),
            _ => None,
        }
    };
    readings.iter().filter(|r| r.kind == kind && r.valid()).filter_map(|r| rank(r).map(|n| (n, r)))
        .min_by(|(a, ra), (b, rb)| a.cmp(b).then_with(|| rb.celsius.total_cmp(&ra.celsius)).then_with(|| ra.id.cmp(&rb.id)))
        .map(|(_, r)| r)
}

#[cfg(test)]
mod tests {
    use super::*;
    pub fn sensor(id: &str, name: &str, kind: Kind, celsius: f32) -> Reading {
        Reading { id: id.into(), name: name.into(), hardware: "Test hardware".into(), kind, celsius }
    }
    #[test]
    fn package_and_gpu_core_do_not_get_replaced_by_hotspot_or_chiplet() {
        let readings = vec![sensor("cpu/ccd", "CCD1", Kind::Cpu, 85.0), sensor("cpu/package", "CPU (Tctl/Tdie)", Kind::Cpu, 70.0),
            sensor("gpu/hot", "GPU Hot Spot", Kind::Gpu, 95.0), sensor("gpu/core", "GPU Core", Kind::Gpu, 65.0)];
        assert_eq!(primary(&readings, Kind::Cpu).unwrap().celsius, 70.0);
        assert_eq!(primary(&readings, Kind::Gpu).unwrap().celsius, 65.0);
        assert!(primary(&readings[..1], Kind::Gpu).is_none());
        assert!(primary(&readings[2..3], Kind::Gpu).is_none());
    }
    #[test]
    fn invalid_readings_are_not_displayed_as_real_temperatures() {
        for celsius in [f32::NAN, f32::INFINITY, 0.0, -273.0, 300.0] {
            assert!(primary(&[sensor("cpu", "CPU Package", Kind::Cpu, celsius)], Kind::Cpu).is_none());
        }
        for name in ["Warning Temperature", "Critical Temperature", "Thermal Sensor High Limit", "Temperature Sensor Resolution", "Distance to TjMax"] {
            assert!(!sensor("metadata", name, Kind::Other, 85.0).valid());
        }
        assert!(sensor("ram", "DIMM #0", Kind::Other, 42.75).valid());
    }
}
