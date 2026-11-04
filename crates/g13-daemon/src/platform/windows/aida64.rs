//! AIDA64's documented read-only sensor sharing. No additional sensor driver
//! or helper is needed on a PC where AIDA64 already supplies the readings.
use std::mem::size_of;
use windows_sys::Win32::{Foundation::{CloseHandle, INVALID_HANDLE_VALUE}, System::{
    Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS},
    Memory::{OpenFileMappingW, MapViewOfFile, UnmapViewOfFile, VirtualQuery, FILE_MAP_READ, MEMORY_BASIC_INFORMATION},
}};
use crate::platform::temperatures::{Kind, Reading};

pub fn read() -> Vec<Reading> {
    // Other consumers can keep an old mapping alive after AIDA64 exits.
    if !running() { return Vec::new(); }
    let Some(bytes) = shared_bytes("AIDA64_SensorValues") else { return Vec::new() };
    decode(&bytes).map(|xml| parse(&xml)).unwrap_or_default()
}

fn running() -> bool {
    // SAFETY: snapshot and entry buffers are initialized and the handle is
    // closed on both found/not-found paths. No process is opened or modified.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE { return false; }
        let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut more = Process32FirstW(snapshot, &mut entry);
        let mut found = false;
        while more != 0 {
            let length = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..length]);
            if name.eq_ignore_ascii_case("aida64.exe") || name.eq_ignore_ascii_case("aida64x64.exe") { found = true; break; }
            more = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
        found
    }
}

fn shared_bytes(name: &str) -> Option<Vec<u8>> {
    let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    // SAFETY: only FILE_MAP_READ is requested. VirtualQuery bounds the view;
    // volatile byte reads copy externally updated memory into owned storage.
    unsafe {
        let handle = OpenFileMappingW(FILE_MAP_READ, 0, name.as_ptr());
        if handle.is_null() { return None; }
        let view = MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0);
        if view.Value.is_null() { CloseHandle(handle); return None; }
        let mut region = MEMORY_BASIC_INFORMATION::default();
        let bytes = if VirtualQuery(view.Value, &mut region, size_of::<MEMORY_BASIC_INFORMATION>()) != 0 {
            let length = region.RegionSize.min(1_048_576);
            let pointer = view.Value.cast::<u8>();
            Some((0..length).map(|i| std::ptr::read_volatile(pointer.add(i))).collect())
        } else { None };
        UnmapViewOfFile(view);
        CloseHandle(handle);
        bytes
    }
}

fn decode(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 2 || bytes[0] == 0 { return None; }
    if bytes.starts_with(&[0xff, 0xfe]) || bytes[1] == 0 {
        // Accommodate the short-lived Unicode AIDA64 builds as well as ANSI.
        let start = usize::from(bytes.starts_with(&[0xff, 0xfe])) * 2;
        let units: Vec<u16> = bytes[start..].as_chunks::<2>().0.iter().map(|p| u16::from_le_bytes(*p)).take_while(|u| *u != 0).collect();
        String::from_utf16(&units).ok()
    } else {
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        Some(String::from_utf8_lossy(&bytes[..end]).into_owned())
    }
}

fn parse(fragment: &str) -> Vec<Reading> {
    // AIDA64 shares sibling XML elements, not a complete XML document.
    let wrapped = format!("<aida>{fragment}</aida>");
    let Ok(document) = roxmltree::Document::parse(&wrapped) else { return Vec::new() };
    document.descendants().filter(|n| n.has_tag_name("temp")).filter_map(|sensor| {
        let field = |name: &str| sensor.children().find(|n| n.has_tag_name(name)).and_then(|n| n.text()).unwrap_or("").trim();
        let id = field("id");
        if !id.starts_with('T') { return None; }
        let celsius = field("value").replace(',', ".").parse::<f32>().ok()?;
        let label = field("label");
        let kind = if id.starts_with("TCPU") || id.starts_with("TCC") { Kind::Cpu }
            else if id.starts_with("TGPU") || id == "TIGPU" { Kind::Gpu } else { Kind::Other };
        let reading = Reading { id: format!("aida:{id}"), name: if label.is_empty() { id.into() } else { label.into() },
            hardware: "AIDA64".into(), kind, celsius };
        reading.valid().then_some(reading)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::temperatures::primary;
    const SHARED: &str = "<sys><id>SCPUUTI</id><label>CPU Utilization</label><value>20</value></sys><temp><id>TCPUPKG</id><label>CPU Package</label><value>62.5</value></temp><temp><id>TGPU1</id><label>GPU1</label><value>55</value></temp><temp><id>TGPU1HOT</id><label>GPU1 Hotspot</label><value>78</value></temp><temp><id>TDISK1</id><label>SSD</label><value>40,5</value></temp><volt><id>VCPU</id><label>CPU Voltage</label><value>1.2</value></volt>";
    #[test]
    fn reads_only_temperatures_and_keeps_hotspot_separate() {
        let readings = parse(SHARED);
        assert_eq!(readings.len(), 4);
        assert_eq!(primary(&readings, Kind::Cpu).unwrap().celsius, 62.5);
        assert_eq!(primary(&readings, Kind::Gpu).unwrap().celsius, 55.0);
        assert_eq!(readings[3].celsius, 40.5);
        assert!(parse("<temp><id>TCPU</id>").is_empty());
        assert!(parse("<temp><id>TCPU</id><value>TRIAL</value></temp>").is_empty());
    }
    #[test]
    fn supports_ansi_and_utf16_shared_memory_with_terminators() {
        assert_eq!(decode(format!("{SHARED}\0ignored").as_bytes()).unwrap(), SHARED);
        let utf16: Vec<_> = SHARED.encode_utf16().chain(Some(0)).flat_map(u16::to_le_bytes).collect();
        assert_eq!(decode(&utf16).unwrap(), SHARED);
        let mut bom = vec![0xff, 0xfe]; bom.extend(utf16);
        assert_eq!(decode(&bom).unwrap(), SHARED);
    }
    #[test]
    fn reads_an_actual_windows_mapping_without_an_aida64_installation() {
        use windows_sys::Win32::System::Memory::{CreateFileMappingW, FILE_MAP_WRITE, PAGE_READWRITE};
        let name = format!("Local\\GNexusSensorTest-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let bytes = format!("{SHARED}\0").into_bytes();
        // SAFETY: unique temporary test mapping, owned writable view, and a
        // bounded copy smaller than the 4 KB allocation; all handles are closed.
        unsafe {
            let mapping = CreateFileMappingW(INVALID_HANDLE_VALUE, std::ptr::null(), PAGE_READWRITE, 0, 4096, wide.as_ptr());
            assert!(!mapping.is_null());
            let view = MapViewOfFile(mapping, FILE_MAP_WRITE, 0, 0, 4096);
            assert!(!view.Value.is_null());
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), view.Value.cast::<u8>(), bytes.len());
            let copied = shared_bytes(&name).unwrap();
            UnmapViewOfFile(view);
            CloseHandle(mapping);
            assert_eq!(parse(&decode(&copied).unwrap()).len(), 4);
        }
        assert!(shared_bytes("GNexusNonexistentMappingForTest").is_none());
    }
}
