# To do

Open items from the code review of 0.3.0, most useful first. Remove an item
in the same commit that does it.

## Rust

- **`clippy::pedantic`.** About 24 warnings, mostly `as` casts that could be
  `From`/`u32::from`, `map().unwrap_or()` → `map_or`, `StickCommand` passed by
  reference instead of by value.

## Tests

- `src/lib.rs` (REAPER side: reload, deferred actions) has no tests; it
  needs REAPER, so it is only tested by hand.
