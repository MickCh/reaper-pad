#!/usr/bin/env bash
# Build the REAPER extension and install it (Linux).
# REAPER picks up the new version after a restart.
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release
dest="${REAPER_RESOURCE_PATH:-$HOME/.config/REAPER}/UserPlugins"
mkdir -p "$dest"
# Replace via rename: overwriting a library REAPER has loaded can crash it.
cp target/release/libreaper_pad.so "$dest/.reaper_pad.so.new"
mv -f "$dest/.reaper_pad.so.new" "$dest/reaper_pad.so"
echo "Installed: $dest/reaper_pad.so"
if pgrep -x reaper >/dev/null; then
    echo "REAPER is running — restart it to load the new version."
fi
