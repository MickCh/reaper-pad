//! SDL2 glue: the device, its events and motors. What the input means is
//! decided by the testable [`Router`].

use crate::config::Config;
use crate::logic::Pattern;
use crate::router::{Device, InputEvent, Log, Router, Sink};
use sdl2::event::Event;
use sdl2::joystick::{HatState, Joystick};
use sdl2::{EventPump, JoystickSubsystem, Sdl};
use std::time::Instant;

/// USB vendor of DragonRise pads (Art AG-16T and many cheap clones).
const DRAGONRISE_VENDOR: u16 = 0x0079;

pub struct Pad {
    router: Router,
    js: JoystickSubsystem,
    events: EventPump,
    joy: Option<Joystick>,
    /// Dropped last: SDL shuts down when its last handle goes.
    _sdl: Sdl,
}

impl Drop for Pad {
    fn drop(&mut self) {
        // Some platforms only end a rumble on the next poll, which won't come.
        if let Some(joy) = self.joy.as_mut() {
            let _ = joy.set_rumble(0, 0, 0);
        }
    }
}

impl Pad {
    pub fn new(cfg: Config) -> Result<Self, String> {
        // Otherwise SDL ignores the pad while another window (REAPER) has focus.
        sdl2::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");
        // We run inside REAPER: SDL must not take over the process's signal handlers.
        sdl2::hint::set("SDL_NO_SIGNAL_HANDLERS", "1");
        let sdl = sdl2::init()?;
        let js = sdl.joystick()?;
        let events = sdl.event_pump()?;
        Ok(Pad {
            router: Router::new(cfg, Instant::now()),
            js,
            events,
            joy: None,
            _sdl: sdl,
        })
    }

    /// Replace the configuration (e.g. after editing the file).
    pub fn set_config(&mut self, cfg: Config) {
        self.router.set_config(cfg);
    }

    pub fn test_mode(&self) -> bool {
        self.router.test_mode()
    }

    pub fn set_test_mode(&mut self, on: bool) {
        self.router.set_test_mode(on);
    }

    /// Name of the connected pad, if any.
    pub fn connected(&self) -> Option<String> {
        self.joy.as_ref().map(|j| j.name())
    }

    /// Open the first available pad (if any).
    pub fn open_first(&mut self, log: &mut dyn Log) {
        let n = self.js.num_joysticks().unwrap_or(0);
        let joy = (0..n).find_map(|i| self.js.open(i).ok());
        match &joy {
            Some(j) => log.log(&format!(
                "Gamepad: {} ({} buttons, {} axes, {} hats)",
                j.name(),
                j.num_buttons(),
                j.num_axes(),
                j.num_hats()
            )),
            None => log.log("Waiting for a gamepad…"),
        }
        attach(&mut self.router, &mut self.joy, joy);
    }

    pub fn rumble(&mut self, pattern: Pattern) {
        self.router.rumble(pattern, Instant::now());
    }

    /// Handle events, sticks and rumble. Call regularly (REAPER's timer, ~30×/s).
    pub fn poll(&mut self, sink: &mut dyn Sink) {
        let now = Instant::now();
        // Borrows only `self.events`; the loop uses the other fields.
        for ev in self.events.poll_iter() {
            match ev {
                Event::JoyDeviceAdded { which, .. } => {
                    if self.joy.is_none() {
                        let joy = self.js.open(which).ok();
                        if let Some(j) = &joy {
                            sink.log(&format!("Gamepad connected: {}", j.name()));
                        }
                        attach(&mut self.router, &mut self.joy, joy);
                    }
                }
                Event::JoyDeviceRemoved { which, .. } => {
                    if self.joy.as_ref().is_some_and(|j| j.instance_id() == which) {
                        sink.log("Gamepad disconnected — waiting…");
                        self.joy = None;
                        self.router.disconnected(now);
                    }
                }
                ev => {
                    if let Some(input) = input_event(&ev) {
                        self.router.event(input, now, sink);
                    }
                }
            }
        }

        let joy = self.joy.as_ref();
        self.router
            .tick(now, |axis| joy.and_then(|j| j.axis(axis).ok()), sink);

        if let Some(joy) = self.joy.as_mut() {
            for (lo, hi, ms) in self.router.rumble_due(now) {
                let _ = joy.set_rumble(lo, hi, ms);
            }
        }
    }
}

/// Make `joy` the connected pad and tell the router. A free function, so
/// that `poll` can call it while iterating over the event pump.
fn attach(router: &mut Router, slot: &mut Option<Joystick>, joy: Option<Joystick>) {
    if let Some(j) = &joy {
        router.connected(Device {
            dragonrise: vendor(j) == Some(DRAGONRISE_VENDOR),
        });
    }
    *slot = joy;
}

/// The part of an SDL event the router cares about.
fn input_event(ev: &Event) -> Option<InputEvent> {
    match *ev {
        Event::JoyButtonDown { button_idx, .. } => Some(InputEvent::Button(button_idx)),
        Event::JoyAxisMotion {
            axis_idx, value, ..
        } => Some(InputEvent::Axis(axis_idx, value)),
        Event::JoyHatMotion { state, .. } => Some(InputEvent::Hat(hat_xy(state))),
        _ => None,
    }
}

/// USB vendor ID from SDL's joystick GUID (bytes 4–5, little endian), if the
/// GUID carries one.
fn vendor(joy: &Joystick) -> Option<u16> {
    let g = joy.guid().raw().data;
    let v = u16::from_le_bytes([g[4], g[5]]);
    (v != 0 && g[6] == 0 && g[7] == 0).then_some(v)
}

fn hat_xy(h: HatState) -> (i8, i8) {
    match h {
        HatState::Centered => (0, 0),
        HatState::Up => (0, 1),
        HatState::Down => (0, -1),
        HatState::Left => (-1, 0),
        HatState::Right => (1, 0),
        HatState::LeftUp => (-1, 1),
        HatState::LeftDown => (-1, -1),
        HatState::RightUp => (1, 1),
        HatState::RightDown => (1, -1),
    }
}
