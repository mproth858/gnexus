# G13 temperatures

Press BD to reach **Temperatures**, immediately after **System**.
The first page shows CPU and GPU temperatures in Celsius. L1/L2 browse extra
sensors; L3 returns to CPU/GPU. `--` means the reading is unavailable or stale.

NVIDIA GPU core temperature is read directly from the installed NVIDIA driver.
On the development PC, the RTX 4000 Ada was verified at approximately 59–60 C.
CPU temperature was not exposed by stock Windows, so it requires a provider.

## Windows CPU and other sensors

Existing Libre/Open Hardware Monitor installations that publish WMI work
through `ROOT\LibreHardwareMonitor` or `ROOT\OpenHardwareMonitor`.

For modern [Libre Hardware Monitor 0.9.6](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/releases/tag/v0.9.6):

1. Extract the official `LibreHardwareMonitor.zip` (.NET Framework build).
2. Install its official PawnIO sensor driver. This is a separate Windows driver
   installation requiring administrator approval; G Nexus does not install it.
3. Run the read-only bridge using Windows PowerShell 5.1 as administrator:

```powershell
.\tools\temperature-bridge.ps1 -LibraryPath "C:\path\to\LibreHardwareMonitorLib.dll" -OutputPath "$env:APPDATA\g13d\config\temperatures.json"
```

Pass the actual normal user's output path if the administrator account differs.
The bridge must stay running while readings are wanted. It updates a local file
every two seconds. It does not start a network server or change hardware
controls. `-Once` collects one snapshot for diagnosis. CPU access requires the
monitor's driver; the NVIDIA reading works independently of that setup.

Library and driver binaries are external dependencies, not committed in this
repository. The library's license is MPL 2.0; see the official release/source.

## Other possible temperature pages

The exact choices depend on sensors exposed by the hardware/provider:

- CPU package/control, die/chiplet and individual core temperatures.
- GPU core, hotspot and memory junction temperature; integrated GPU where supported.
- Motherboard, chipset and voltage-regulator temperatures.
- NVMe SSD and other storage temperatures.
- Memory module temperatures where supported.
- Liquid cooler/coolant and PSU temperatures on supported monitoring devices.

Only reported temperature sensors are listed. A disconnected or unsupported
sensor is not replaced with an invented value. An unlabeled motherboard sensor
retains the provider's label; do not assume it identifies a particular component.

For a read-only inventory:

```powershell
cargo run -p g13-daemon --bin temperature_probe
```

GPU API reference: [NVIDIA NVML device queries](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceQueries.html).
The library exposes sensor categories through its [Computer API](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/blob/v0.9.6/LibreHardwareMonitorLib/Hardware/Computer.cs).
