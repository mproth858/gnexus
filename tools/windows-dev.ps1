param(
    [ValidateSet('Build', 'Test', 'Check')][string]$Action = 'Build',
    [string]$ToolchainBin,
    [string]$TargetDir = (Join-Path $env:LOCALAPPDATA 'G Nexus Build')
)
$ErrorActionPreference = 'Stop'
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
$env:PATH = "$cargoBin;$env:PATH"
if ($ToolchainBin) { $env:PATH = "$ToolchainBin;$env:PATH" }
$env:CARGO_TARGET_DIR = $TargetDir
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    switch ($Action) {
        'Build' { & cargo +stable-x86_64-pc-windows-gnu build -p g13-gui }
        'Test' { & cargo +stable-x86_64-pc-windows-gnu test --workspace }
        'Check' { & cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets }
    }
    if ($LASTEXITCODE -ne 0) { throw "Cargo exited with $LASTEXITCODE" }
    if ($Action -eq 'Build') {
        # GNU builds link WebView2 dynamically; Cargo does not copy this DLL.
        $lock = Get-Content -LiteralPath 'Cargo.lock' -Raw
        $loaderVersion = [regex]::Match($lock, 'name = "webview2-com-sys"\r?\nversion = "([^"]+)"').Groups[1].Value
        $cargoCache = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
        $loader = Get-ChildItem -Path (Join-Path $cargoCache "registry\src\*\webview2-com-sys-$loaderVersion\x64\WebView2Loader.dll") | Select-Object -First 1
        if (!$loader) { throw 'WebView2Loader.dll was not found in the Cargo source cache' }
        Copy-Item -LiteralPath $loader.FullName -Destination (Join-Path $TargetDir 'debug\WebView2Loader.dll') -Force
        Write-Host (Join-Path $TargetDir 'debug\g13-studio.exe')
    }
} finally { Pop-Location }
