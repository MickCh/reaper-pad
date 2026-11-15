# Changelog

All notable changes to this project are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- docs/hardware.md: Art AG-16T measurements (analog vs digital mode) and
  diagnostic scripts in scripts/diag.

### Changed
- **Configuration format:** stick settings moved into `[sticks.left]` and
  `[sticks.right]` (`x`, `y`, `speed`, `invert_y`, `axis_x`, `axis_y`);
  `[sticks]` keeps the shared ones. Replaces `left_x`, `speed_left`,
  `invert_left_y` etc. and `[pad] axis_*`. Old files report the old names
  as unknown; see the default `config.toml`.
- The button test names D-pad positions ("D-pad left-down").
- The gamepad is closed (and its motors stopped) when REAPER exits.

### Fixed
- Release notes in ReaPack no longer show Markdown backticks.
- A pad button bound to one of reaper-pad's own actions (e.g. *Reload
  configuration*) now works; it used to do nothing.
- `nan` and `inf` in `[sticks]` are rejected instead of being passed to
  REAPER.

## [0.3.0] - 2026-11-14

### Added
- ReaPack repository: install and update reaper-pad from REAPER
  (`https://github.com/MickCh/reaper-pad/raw/main/index.xml`).
- Digital-mode detection for DragonRise pads (Art AG-16T and clones): when
  the LED is green the right stick presses buttons 1–4, so reaper-pad buzzes
  three times, prints a warning in the REAPER console and ignores buttons
  1–4 until the pad is back in analog mode. Option `detect_digital_mode`.

### Changed
- Library files are named by architecture (`reaper_pad-x86_64.so`,
  `reaper_pad-x64.dll`), like other REAPER extensions. Remove an older
  manually installed `reaper_pad.so` / `reaper_pad.dll`.

## [0.2.0] - 2026-11-14

First public version.

### Added
- REAPER extension that calls the REAPER API directly (Linux and Windows).
- Actions: *Reload configuration*, *Enable/disable gamepad*, *Button test (console)*.
- Smooth horizontal zoom.
- Variable-speed sticks, D-pad auto-repeat, contact-bounce filter.
- Rumble on record start/stop and a gentle startup buzz.
- Configuration validation with clear error messages.
- Unit tests and CI for Linux and Windows.
