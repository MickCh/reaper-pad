# Changelog

All notable changes to this project are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- ReaPack repository: install and update reaper-pad from REAPER
  (`https://github.com/MickCh/reaper-pad/raw/main/index.xml`).

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
