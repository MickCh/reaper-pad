# Build the REAPER extension and install it (Windows).
# Close REAPER first: Windows locks a loaded DLL.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
cargo build --release
$dest = Join-Path $env:APPDATA "REAPER\UserPlugins"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item "target\release\reaper_pad.dll" (Join-Path $dest "reaper_pad.dll") -Force
Write-Host "Installed: $dest\reaper_pad.dll"
