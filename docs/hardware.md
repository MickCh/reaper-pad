# Hardware notes: Art AG-16T

Measurements behind reaper-pad's defaults, taken on Linux with the kernel
driver `hid-dragonrise`. Other DragonRise pads (USB vendor `0079`) are
probably similar.

## Device

| | |
|---|---|
| USB ID | `0079:0006` (DragonRise Inc. Generic USB Joystick) |
| SDL GUID | `03 00 45 7e 79 00 00 00 06 00 00 00 10 01 00 00` (bus, CRC, vendor `0x0079` in bytes 4–5, product `6` in bytes 8–9) |
| Buttons | printed 1–10; 11/12 = left/right stick press. SDL index = number − 1 |
| Axes (SDL) | left stick 0/1, right stick 3/4 (evdev `ABS_RX`/`ABS_RZ`); axis 2 (`ABS_Z`) follows the left stick and is ignored |
| Axis range | 8 bits, raw 0–255 |
| Motors | two (strong/weak), both work through SDL |

## Analog vs digital mode

The mode LED shows the mode; the **Analog** button switches it. The pad
falls back to digital whenever it loses power (e.g. after reconnecting).

| | Analog (LED red) | Digital (LED green) |
|---|---|---|
| Sticks at rest (`X Y RX RZ`) | 128 | 127 |
| Left stick moving | many values over 0–255 | only 0 / 127 / 255 |
| Right stick moving | many values over 0–255 | axes stay at 127; the stick presses **buttons 1–4** |
| `Z` | ≈143–145, analog | ≈142–145, analog |

SDL scales raw values as `raw × 257 − 32768`, so analog rest is **+128** and
digital rest **−129**. reaper-pad treats a DragonRise pad as digital when
both right-stick axes sit at the same value just below the centre for
500 ms (`ModeDetector` in `src/logic.rs`); option `[pad] detect_digital_mode`.

## Quirks

- Face buttons bounce: one click can register 2–3 presses → `debounce_ms`.
- The left stick's Y axis is upside down for scrolling → `invert_left_y`.
- SDL ignores the pad while another window has focus unless
  `SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS=1` (set by reaper-pad).
- On Linux SDL ends a rumble only on the next poll. REAPER's timer stalls
  during startup, so reaper-pad waits 60 timer ticks before the hello buzz,
  sends explicit stop commands and skips pulses that are too late.

## Diagnostic scripts (Linux)

Standard-library Python, reading the evdev device directly (default: the
first DragonRise pad in `/dev/input/by-id`):

```sh
scripts/diag/padstate.py [DEVICE] [LABEL]     # resting axis values, held buttons
scripts/diag/padrec.py SECONDS [DEVICE] [LABEL]  # record and summarise movement
```

Inside REAPER, the action *reaper-pad: Button test (console)* shows button
and axis numbers as SDL sees them.
