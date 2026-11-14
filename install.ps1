# Build the REAPER extension and install it (Windows).
# Close REAPER first: Windows locks a loaded DLL.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
cargo build --release
$dest = Join-Path $env:APPDATA "REAPER\UserPlugins"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
# Same file name as the ReaPack package, so the two never load side by side.
Copy-Item "target\release\reaper_pad.dll" (Join-Path $dest "reaper_pad-x64.dll") -Force
Write-Host "Installed: $dest\reaper_pad-x64.dll"
# Older versions were installed as reaper_pad.dll — two copies would run every command twice.
$old = Join-Path $dest "reaper_pad.dll"
if (Test-Path $old) { Remove-Item $old; Write-Host "Removed the old $old" }
