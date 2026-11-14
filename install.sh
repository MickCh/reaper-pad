#!/usr/bin/env bash
# Build the REAPER extension and install it (Linux).
# REAPER picks up the new version after a restart.
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release
dest="${REAPER_RESOURCE_PATH:-$HOME/.config/REAPER}/UserPlugins"
mkdir -p "$dest"
# Replace via rename: overwriting a library REAPER has loaded can crash it.
# Same file name as the ReaPack package, so the two never load side by side.
file=reaper_pad-x86_64.so
cp target/release/libreaper_pad.so "$dest/.$file.new"
mv -f "$dest/.$file.new" "$dest/$file"
echo "Installed: $dest/$file"
# Older versions were installed as reaper_pad.so — two copies would run every command twice.
if [ -e "$dest/reaper_pad.so" ]; then
    rm -f "$dest/reaper_pad.so"
    echo "Removed the old $dest/reaper_pad.so"
fi
if pgrep -x reaper >/dev/null; then
    echo "REAPER is running — restart it to load the new version."
fi
