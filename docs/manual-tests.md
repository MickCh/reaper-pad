# Manual tests

What the unit tests cannot cover: everything that needs REAPER or a real
gamepad (`src/lib.rs`, `src/pad.rs`). Run through this before a release.
Start REAPER from a terminal to see the `[reaper-pad]` lines.

## Startup and configuration

- [ ] Without `reaper-pad.toml`: the default file is created, the pad gives
      the hello buzz about 2 s after startup.
- [ ] A typo in the file (e.g. `[sticks.left] invert_x = true`): the console
      names the option; *Reload configuration* after fixing it starts the pad.
- [ ] A file from 0.3 (e.g. `[sticks] left_x = "scroll_x"` plus a typo
      elsewhere): the console lists each renamed option with its new place,
      and the typo after "also:" with its line in the file.
- [ ] *Reload configuration* with a valid change: short buzz, the change
      applies (e.g. `[sticks.right] speed`).

## Actions

- [ ] *Enable/disable gamepad*: buttons stop and start working; the toggle
      state shows in the action list / toolbar, and the toolbar button does
      not flicker while pad buttons are pressed.
- [ ] *Button test (console)*: buttons, axes and D-pad positions are printed
      ("D-pad left-down"), REAPER is not controlled; running it again ends it.
- [ ] (Optional) A pad button bound to one of these actions works like
      running it from the action list. Only reachable with the action's
      numeric ID, which REAPER assigns per session (see TODO: named IDs).

## Gamepad

- [ ] Unplug and replug: "disconnected" / "connected" messages, the pad
      works again without a restart.
- [ ] Recording start / stop: one long / two short pulses.
- [ ] Sticks: left scrolls (time / tracks), right zooms (smooth horizontally);
      up means up.
- [ ] DragonRise pad in digital mode (LED green): three buzzes, warning,
      buttons 1–4 ignored; Analog (LED red) brings them back.

## Shutdown

- [ ] Quit REAPER: it exits cleanly, no hang, no errors in the terminal.
      (Stopping a running rumble on exit is a safeguard; rumbles last at most
      0.4 s, so it is not practical to test.)
