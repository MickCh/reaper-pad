# reaper-pad

[![CI](https://github.com/MickCh/reaper-pad/actions/workflows/ci.yml/badge.svg)](https://github.com/MickCh/reaper-pad/actions/workflows/ci.yml)

Use a cheap USB gamepad as a remote control for [REAPER](https://www.reaper.fm/).

Start and stop recording with the guitar still on your lap, undo a bad take, drop markers, and scroll and zoom the arrange view with the analog sticks. The pad vibrates when recording really starts, so you don't need to look at the screen.

reaper-pad is a REAPER extension: it loads together with REAPER and calls the REAPER API directly. Nothing else needs to run and nothing needs configuring in REAPER. It works on **Linux** and **Windows**.

## Features

- Any SDL2-compatible gamepad: generic DirectInput/HID pads as well as XInput controllers.
- Buttons and D-pad mapped to transport commands or to **any REAPER action** by command ID.
- Analog sticks with **variable speed**: a slight push moves slowly, full deflection moves fast. Horizontal zoom is **smooth**.
- **Rumble feedback** when recording starts (one long pulse) and stops (two short pulses).
- Contact-bounce filter for old, worn pads.
- Works in the background while REAPER has focus.
- Config errors are reported clearly (unknown commands, typos in option names) instead of being silently ignored. Options renamed in 0.4 point to their new place.
- reaper-pad catches its own internal errors: a bug in it disables the pad, not REAPER.

## Default layout

The defaults were made for the **Art AG-16T** (a generic DragonRise-based twin-stick pad). Button numbers match its printed labels. Other pads work too; see [Other gamepads](#other-gamepads).

| Control | Function |
|---|---|
| **1** | Play / stop |
| **2** | Record |
| **3 / 4** | Undo / redo |
| **5** | Metronome on/off |
| **6** | Loop (repeat) on/off |
| **7 / 8** | Previous / next marker |
| **9** | Go to project start |
| **10** | Insert marker |
| **11** (left stick click) | Save project |
| **12** (right stick click) | Insert new track |
| **D-pad ↑ ↓** | Previous / next track |
| **D-pad ← →** | Move cursor by one grid division (repeats while held) |
| **Left stick** | Scroll (time / tracks) |
| **Right stick** | Zoom (↔ time, smooth · ↕ track height) |

## Installation

On Linux, reaper-pad uses the system SDL2 library: install `sdl2` (Arch: `sdl2-compat`; Debian/Ubuntu: `libsdl2-2.0-0`). The Windows build includes SDL2.

### With ReaPack (recommended)

[ReaPack](https://reapack.com) installs reaper-pad and keeps it up to date.

1. In REAPER: **Extensions → ReaPack → Import repositories…**
2. Paste this URL and confirm:
   ```
   https://github.com/MickCh/reaper-pad/raw/main/index.xml
   ```
3. **Extensions → ReaPack → Browse packages**, search for `reaper-pad`, right-click → **Install**, then **Apply**.
4. Restart REAPER.

About 2 seconds after startup the pad gives a short, gentle buzz: the extension is running. New versions show up in **Extensions → ReaPack → Synchronize packages**.

### Manually

Download the file for your system from [Releases](https://github.com/MickCh/reaper-pad/releases) and copy it into REAPER's `UserPlugins` folder, then restart REAPER:

| System | File | Destination |
|---|---|---|
| Linux (x86_64) | `reaper_pad-x86_64.so` | `~/.config/REAPER/UserPlugins/` |
| Windows (64-bit) | `reaper_pad-x64.dll` | `%APPDATA%\REAPER\UserPlugins\` |

> **Tip:** with a portable REAPER install, use **Options → Show REAPER resource path in explorer/finder** to find the right folder.

> **Only one copy at a time.** Two copies of the extension in `UserPlugins` (e.g. a manual install next to the ReaPack one, or the old `reaper_pad.so` / `reaper_pad.dll` names) would each run every command, so every press would act twice. Keep a single file.

### From source

Requirements: Rust 1.88+, a C/C++ compiler (needed to build reaper-rs). On Linux you also need the SDL2 development files (`libsdl2-dev` on Debian/Ubuntu). On Windows, SDL2 is built from source, which requires CMake.

```sh
git clone https://github.com/MickCh/reaper-pad
cd reaper-pad
./install.sh          # Linux: builds and installs the extension
# .\install.ps1       # Windows (PowerShell); close REAPER first
```

The scripts install under the same file name as the ReaPack package. `install.sh` replaces the library atomically, so it is safe to run while REAPER is open; the new version loads after a restart.

## Usage

### Actions

The extension adds these entries to REAPER's action list (**Actions → Show action list**, filter `reaper-pad`):

| Action | What it does |
|---|---|
| **reaper-pad: Reload configuration** | Re-reads `reaper-pad.toml` without restarting REAPER. The pad buzzes to confirm. If the file has an error, a console window explains it and the previous settings stay active. |
| **reaper-pad: Enable/disable gamepad** | Toggle. While disabled, the pad does nothing. |
| **reaper-pad: Button test (console)** | Toggle. Shows button, axis and D-pad numbers in REAPER's console. While it is on, the pad doesn't control REAPER. |

To put them in a menu or toolbar, use **Options → Customize menus/toolbars → Add…** and search for `reaper-pad`. On a toolbar, the toggles light up when active.

### Configuration

On first start, the extension writes the default configuration to `reaper-pad.toml` in REAPER's resource folder (Linux: `~/.config/REAPER/`, Windows: `%APPDATA%\REAPER\`). Edit it, then run **reaper-pad: Reload configuration**.

The [default `config.toml`](config.toml) documents every option. In short:

```toml
[buttons]
1 = "play_stop"
2 = "record"
7 = "action:40172"      # any REAPER action by command ID

[dpad]
up = "action:40286"
repeat_ms = 250

[sticks.left]
x = "scroll_x"
invert_y = true

[sticks.right]
x = "zoom_x"
speed = 0.3             # slower zoom

[pad]
debounce_ms = 120
rumble = true
```

**Button and D-pad commands:** `play_stop`, `play`, `stop`, `record`, `metronome`, `repeat`, `undo`, `redo`, `action:<id>`.

**Stick commands:** `scroll_x`, `scroll_y`, `zoom_x`, `zoom_y`.

To find an action's ID: in the action list, right-click an action → **Copy selected action command ID**. Only numeric IDs are supported for now.

**Stick speed:** `min_steps_per_sec` and `max_steps_per_sec` set the speed at a slight and at full deflection. `speed` in `[sticks.left]` / `[sticks.right]` scales each stick. `zoom_amount` sets how much one step of smooth horizontal zoom changes.

### Other gamepads

1. Run **reaper-pad: Button test (console)**.
2. Press each button and move each stick. Note the numbers it prints.
3. Adjust `[buttons]` and `axis_x` / `axis_y` in `[sticks.left]` / `[sticks.right]` accordingly.

## Troubleshooting

| Problem | Solution |
|---|---|
| Right stick acts like buttons 1–4 (e.g. starts playback) | The pad is in digital mode. Press its **Analog/Mode** button: on the Art AG-16T the LED turns from green (digital) to red (analog). Many pads fall back to digital mode after losing power. |
| Three short buzzes and *“Gamepad is in DIGITAL mode”* in the console | reaper-pad noticed the digital mode of a DragonRise pad (Art AG-16T and clones) and ignores buttons 1–4 until you press **Analog** (LED red). Turn this off with `detect_digital_mode = false` in `[pad]`. |
| Nothing happens, no startup buzz | Start REAPER from a terminal (Linux) and look for `[reaper-pad]` lines. Check that the pad is visible to the system (`ls /dev/input/by-id/`). |
| Pad is detected, but buttons do something else | Run the button test and fix `[buttons]`. |
| One press triggers twice | Increase `debounce_ms`. |
| Scrolling goes the wrong way | Set `invert_y` in `[sticks.left]` or `[sticks.right]`. |
| Error after editing the config | Run *Reload configuration*: the console window names the line and the problem. |
| *“options renamed in reaper-pad 0.4”* after an update | The file is from 0.3. Move each listed option to its new place (e.g. `[sticks] left_x` → `x` in `[sticks.left]`), or delete the file to get the new default. Other problems in the file follow after *“also:”*. |

## Development

```
src/lib.rs      REAPER side: entry point, actions, executing commands (reaper-rs)
src/pad.rs      gamepad through SDL2: devices, events, rumble
src/router.rs   what the input means: commands, test mode, digital mode, sticks, rumble feedback
src/guard.rs    plugin state guarded against re-entrancy and panics
src/logic.rs    debounce, D-pad repeat, stick curves, rumble scheduling
src/command.rs  bindable commands (names and REAPER IDs generated with each enum)
src/config.rs   configuration and validation
config.toml     documented default configuration (embedded into the extension)
```

```sh
cargo test                            # unit tests (no gamepad needed)
cargo clippy --all-targets -- -D warnings
cargo fmt --all
cargo +1.88 check --all-targets       # minimum Rust version (also checked in CI)
```

Hardware measurements (analog vs digital mode, axis values) and diagnostic
scripts: [docs/hardware.md](docs/hardware.md). What needs REAPER or a real
pad is checked by hand: [docs/manual-tests.md](docs/manual-tests.md).

The input logic (debounce, D-pad repeat, stick curves, rumble scheduling, config validation) is hardware-independent and unit-tested. The event routing (`router.rs`) is tested the same way. SDL2 lives only in `pad.rs`, and REAPER calls only in `lib.rs` (behind the `Sink` trait).

Pushing a tag `v*` (with a matching `## [x.y.z]` section in `CHANGELOG.md`) builds Linux and Windows releases through GitHub Actions and adds the version to the ReaPack index (`index.xml`, via `scripts/reapack_index.py`).

## Acknowledgements

- [reaper-rs](https://github.com/helgoboss/reaper-rs) by Benjamin Klum: Rust bindings for the REAPER API
- [SDL2](https://www.libsdl.org/) and the [sdl2](https://crates.io/crates/sdl2) crate: gamepad input and rumble

REAPER is a trademark of Cockos Incorporated. This project is not affiliated with Cockos.

## License

[MIT](LICENSE)
