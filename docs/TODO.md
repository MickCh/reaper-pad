# To do

Open items from the code review of 0.3.0, most useful first. Remove an item
in the same commit that does it.

## Structure

- **One table of commands.** Adding a button command means editing
  `FromStr`, `Display`, `reaper_action_id` and `BUTTON_HELP` in
  `src/command.rs`. Derive all four from a single table.
- **`RawConfig` → `Config`.** `Config` is only consistent after `parse()`
  (`raw_buttons` vs. `#[serde(skip)] buttons`, a public field). Deserialize
  into a private `RawConfig` and build `Config` with
  `#[serde(try_from = "RawConfig")]`.
- **Per-stick settings.** `Axis` is mapped to its settings by three parallel
  `match`es in `src/config.rs`. A struct per stick would remove them, but it
  changes the config file format (`[sticks.left]` …), so it needs a
  migration path for existing `reaper-pad.toml` files.
- **Split `Sink` (ISP).** It combines running commands, querying the
  recording state and logging. Fine at this size; split the logger off if
  the trait grows.

## Rust

- **Error type.** Errors are `String`s. They are only ever shown to the
  user, so this is fine for now; switch to an enum (e.g. with `thiserror`)
  once callers need to tell errors apart.
- **Per-tick allocations.** `HatRepeat::set/tick` and `directions()` return
  `Vec`s and `Pad::poll()` collects events into a `Vec`. Harmless at 30 Hz;
  a fixed array would be more idiomatic.
- **SDL is never shut down** when REAPER exits (`Box::leak` of the session is
  intentional). Harmless in practice.
- **`clippy::pedantic`.** About 24 warnings, mostly `as` casts that could be
  `From`/`u32::from`, `map().unwrap_or()` → `map_or`, `StickCommand` passed by
  reference instead of by value.

## Tests

- `src/lib.rs` (REAPER side: reload, deferred actions) has no tests; it
  needs REAPER, so it is only tested by hand.
