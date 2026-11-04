# Read-only temperature bridge for the official LibreHardwareMonitor .NET 4.7.2
# library. Modern releases no longer expose WMI. This helper publishes to a local
# file, with no web server, listening socket, fan controls or clock changes.
param(
    [Parameter(Mandatory=$true)][string]$LibraryPath,
    [Parameter(Mandatory=$true)][string]$OutputPath,
    [switch]$Once
)
$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSEdition -ne 'Desktop') { throw 'Run this helper with Windows PowerShell 5.1 (powershell.exe).' }
$bridgeIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
$bridgePrincipal = New-Object Security.Principal.WindowsPrincipal($bridgeIdentity)
if (!$bridgePrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'CPU sensors require administrator access. Run the sensor helper as administrator.'
}
if (!(Test-Path -LiteralPath $LibraryPath -PathType Leaf)) { throw 'LibreHardwareMonitorLib.dll was not found.' }
if (!(Get-Service -Name 'PawnIO' -ErrorAction SilentlyContinue)) {
    throw 'Libre Hardware Monitor 0.9.6 requires its PawnIO sensor driver. Install it from the official Libre Hardware Monitor setup first.'
}
$bridgeDestination = [IO.Path]::GetFullPath($OutputPath)
[IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($bridgeDestination)) | Out-Null
$bridgeLockName = 'Local\GNexusTemperatureBridge'
$bridgeMutex = New-Object Threading.Mutex($false, $bridgeLockName)
if (!$bridgeMutex.WaitOne(0)) { throw 'The G Nexus temperature helper is already running.' }
[Reflection.Assembly]::LoadFrom([IO.Path]::GetFullPath($LibraryPath)) | Out-Null
$bridgeComputer = New-Object LibreHardwareMonitor.Hardware.Computer
$bridgeComputer.IsCpuEnabled = $true
$bridgeComputer.IsGpuEnabled = $true
$bridgeComputer.IsMotherboardEnabled = $true
$bridgeComputer.IsMemoryEnabled = $true
$bridgeComputer.IsStorageEnabled = $true
$bridgeComputer.IsControllerEnabled = $true
$bridgeComputer.IsPsuEnabled = $true
$bridgeEncoding = New-Object Text.UTF8Encoding($false)

function Read-TemperatureHardware($hardware, $items) {
    $hardware.Update()
    $hardwareType = $hardware.HardwareType.ToString()
    $kind = if ($hardwareType -eq 'Cpu') { 'cpu' } elseif ($hardwareType -like 'Gpu*') { 'gpu' } else { 'other' }
    foreach ($sensor in $hardware.Sensors) {
        if ($sensor.SensorType.ToString() -ne 'Temperature' -or $null -eq $sensor.Value) { continue }
        $items.Add([ordered]@{ id = $sensor.Identifier.ToString(); name = $sensor.Name; hardware = $hardware.Name; kind = $kind; celsius = [single]$sensor.Value })
    }
    foreach ($child in $hardware.SubHardware) { Read-TemperatureHardware $child $items }
}

try {
    $bridgeComputer.Open()
    do {
        $bridgeReadings = New-Object 'Collections.Generic.List[object]'
        foreach ($hardware in $bridgeComputer.Hardware) { Read-TemperatureHardware $hardware $bridgeReadings }
        $bridgeSnapshot = [ordered]@{ updated_unix_ms = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds(); readings = @($bridgeReadings.ToArray()) }
        $bridgeJson = ConvertTo-Json -InputObject $bridgeSnapshot -Depth 8
        $bridgeTemporary = $bridgeDestination + '.tmp'
        [IO.File]::WriteAllText($bridgeTemporary, $bridgeJson, $bridgeEncoding)
        # Windows PowerShell 5.1 coerces a null string argument to an empty path.
        # Give Replace a real backup path so repeated publication stays atomic.
        if ([IO.File]::Exists($bridgeDestination)) { [IO.File]::Replace($bridgeTemporary, $bridgeDestination, $bridgeDestination + '.previous') }
        else { [IO.File]::Move($bridgeTemporary, $bridgeDestination) }
        if (!$Once) { Start-Sleep -Seconds 2 }
    } while (!$Once)
} finally {
    $bridgeComputer.Close()
    $bridgeMutex.ReleaseMutex()
    $bridgeMutex.Dispose()
}
