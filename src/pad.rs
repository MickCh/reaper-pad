//! SDL2 glue: device handling, events and motors. The decisions are made by
//! the testable pieces in [`crate::logic`].

use crate::command::{Command, StickCommand};
use crate::config::{Axis, Config};
use crate::logic::{
    AxisTracker, Debouncer, Direction, HatRepeat, ModeDetector, Pattern, RUMBLE_DIGITAL,
    RUMBLE_START, RUMBLE_STOP, RumbleQueue,
};
use sdl2::event::Event;
use sdl2::joystick::{HatState, Joystick};
use sdl2::{EventPump, JoystickSubsystem, Sdl};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// USB vendor of DragonRise pads (Art AG-16T and many cheap clones).
const DRAGONRISE_VENDOR: u16 = 0x0079;

/// Receives what the pad does. Implemented by the REAPER side; keeps this
/// module free of REAPER calls.
pub trait Sink {
    /// A button or D-pad command.
    fn command(&mut self, cmd: &Command);
    /// Stick movement: a fractional number of "steps" since the last call;
    /// the sign is the direction. A fresh deflection starts with exactly ±1.
    fn stick(&mut self, cmd: &StickCommand, delta: f32);
    /// Whether REAPER is recording right now (`None` = unknown).
    fn recording(&mut self) -> Option<bool> {
        None
    }
    /// Status and diagnostic messages.
    fn log(&mut self, msg: &str);
    /// Something the user has to act on: always shown.
    fn notice(&mut self, msg: &str) {
        self.log(msg);
    }
}

pub struct Pad {
    cfg: Config,
    /// Only report button/axis numbers, don't send commands.
    pub test_mode: bool,
    js: JoystickSubsystem,
    events: EventPump,
    joy: Option<Joystick>,
    debouncer: Debouncer,
    hat: HatRepeat,
    axes: HashMap<Axis, AxisTracker>,
    last_tick: Instant,
    recording: Option<bool>,
    rumble: RumbleQueue,
    mode: ModeDetector,
    _sdl: Sdl,
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
            debouncer: Debouncer::new(Duration::from_millis(cfg.pad.debounce_ms)),
            hat: HatRepeat::new(Duration::from_millis(cfg.dpad.repeat_ms)),
            cfg,
            test_mode: false,
            js,
            events,
            joy: None,
            axes: HashMap::new(),
            last_tick: Instant::now(),
            recording: None,
            rumble: RumbleQueue::default(),
            mode: ModeDetector::default(),
            _sdl: sdl,
        })
    }

    /// Replace the configuration (e.g. after editing the file).
    pub fn set_config(&mut self, cfg: Config) {
        self.debouncer
            .set_window(Duration::from_millis(cfg.pad.debounce_ms));
        self.hat
            .set_interval(Duration::from_millis(cfg.dpad.repeat_ms));
        self.axes.clear();
        self.mode.reset();
        self.cfg = cfg;
    }

    /// Name of the connected pad, if any.
    pub fn connected(&self) -> Option<String> {
        self.joy.as_ref().map(|j| j.name())
    }

    /// Open the first available pad (if any).
    pub fn open_first(&mut self, sink: &mut dyn Sink) {
        let n = self.js.num_joysticks().unwrap_or(0);
        self.joy = (0..n).find_map(|i| self.js.open(i).ok());
        match &self.joy {
            Some(j) => sink.log(&format!(
                "Gamepad: {} ({} buttons, {} axes, {} hats)",
                j.name(),
                j.num_buttons(),
                j.num_axes(),
                j.num_hats()
            )),
            None => sink.log("Waiting for a gamepad…"),
        }
    }

    pub fn rumble(&mut self, pattern: Pattern) {
        if self.cfg.pad.rumble && self.joy.is_some() {
            self.rumble.schedule(pattern, Instant::now());
        }
    }

    /// Handle events, sticks and rumble. Call regularly (REAPER's timer, ~30×/s).
    pub fn poll(&mut self, sink: &mut dyn Sink) {
        let now = Instant::now();
        let events: Vec<Event> = self.events.poll_iter().collect();
        for ev in events {
            match ev {
                Event::JoyDeviceAdded { which, .. } => {
                    if self.joy.is_none() {
                        self.joy = self.js.open(which).ok();
                        if let Some(j) = &self.joy {
                            sink.log(&format!("Gamepad connected: {}", j.name()));
                        }
                    }
                }
                Event::JoyDeviceRemoved { which, .. } => {
                    if self.joy.as_ref().is_some_and(|j| j.instance_id() == which) {
                        sink.log("Gamepad disconnected — waiting…");
                        self.joy = None;
                        self.axes.clear();
                        self.hat.set((0, 0), now);
                        self.rumble.clear();
                        self.mode.reset();
                    }
                }
                Event::JoyButtonDown { button_idx, .. } if self.test_mode => {
                    sink.log(&format!("button {}", button_idx as u32 + 1))
                }
                Event::JoyAxisMotion {
                    axis_idx, value, ..
                } if self.test_mode => {
                    if value.unsigned_abs() > 16000 {
                        sink.log(&format!("axis {axis_idx}: {value}"));
                    }
                }
                Event::JoyHatMotion { state, .. } if self.test_mode => {
                    sink.log(&format!("D-pad {state:?}"))
                }
                Event::JoyButtonDown { button_idx, .. } => self.button(button_idx, now, sink),
                Event::JoyHatMotion { state, .. } => {
                    let dirs = self.hat.set(hat_xy(state), now);
                    self.dpad(&dirs, sink);
                }
                _ => {}
            }
        }

        self.check_mode(now, sink);

        let dt = now.duration_since(self.last_tick).as_secs_f32().min(0.1);
        self.last_tick = now;
        if !self.test_mode {
            if let Some(on) = sink.recording() {
                self.on_record(on, sink);
            }
            let dirs = self.hat.tick(now);
            self.dpad(&dirs, sink);
            self.sticks(dt, sink);
        }
        if let Some(joy) = self.joy.as_mut() {
            for (lo, hi, ms) in self.rumble.due(now) {
                let _ = joy.set_rumble(lo, hi, ms);
            }
        }
    }

    fn on_record(&mut self, on: bool, sink: &mut dyn Sink) {
        if self.recording == Some(on) {
            return;
        }
        // No rumble for the initial state.
        if self.recording.is_some() {
            if on {
                sink.log("● recording");
                self.rumble(RUMBLE_START);
            } else {
                sink.log("■ recording stopped");
                self.rumble(RUMBLE_STOP);
            }
        }
        self.recording = Some(on);
    }

    /// Watch for the digital mode of DragonRise pads (see [`ModeDetector`]).
    fn check_mode(&mut self, now: Instant, sink: &mut dyn Sink) {
        let Some(joy) = self.joy.as_ref() else { return };
        if !self.cfg.pad.detect_digital_mode || vendor(joy) != Some(DRAGONRISE_VENDOR) {
            return;
        }
        let x = joy.axis(self.cfg.pad.axis_right_x).unwrap_or(0);
        let y = joy.axis(self.cfg.pad.axis_right_y).unwrap_or(0);
        match self.mode.update(x, y, now) {
            Some(true) => {
                sink.notice(
                    "Gamepad is in DIGITAL mode (LED green): the right stick would press \
                     buttons 1–4, so they are ignored. Press the Analog button (LED red).",
                );
                self.rumble(RUMBLE_DIGITAL);
            }
            Some(false) => sink.notice("Gamepad is in analog mode — buttons 1–4 work again."),
            None => {}
        }
    }

    fn button(&mut self, idx: u8, now: Instant, sink: &mut dyn Sink) {
        let num = idx as u32 + 1; // number as printed on the pad
        if !self.debouncer.accept(num, now) {
            return;
        }
        if self.mode.digital() && (1..=4).contains(&num) {
            sink.log(&format!(
                "[{num}] ignored — digital mode (press Analog, LED red)"
            ));
            return;
        }
        match self.cfg.button(num) {
            Some(cmd) => {
                sink.log(&format!("[{num}] {cmd}"));
                let cmd = cmd.clone();
                sink.command(&cmd);
            }
            None => sink.log(&format!("[{num}] (not assigned)")),
        }
    }

    fn dpad(&mut self, dirs: &[Direction], sink: &mut dyn Sink) {
        for dir in dirs {
            let d = &self.cfg.dpad;
            let cmd = match dir {
                Direction::Up => &d.up,
                Direction::Down => &d.down,
                Direction::Left => &d.left,
                Direction::Right => &d.right,
            };
            if let Some(cmd) = cmd.clone() {
                sink.command(&cmd);
            }
        }
    }

    fn sticks(&mut self, dt: f32, sink: &mut dyn Sink) {
        let Some(joy) = self.joy.as_ref() else { return };
        for axis in Axis::ALL {
            let Some(cmd) = self.cfg.stick_command(axis) else {
                continue;
            };
            let value = joy
                .axis(self.cfg.axis_index(axis))
                .map(|r| r as f32 / 32767.0)
                .unwrap_or(0.0);
            let params = self.cfg.axis_params(axis);
            if let Some(delta) = self
                .axes
                .entry(axis)
                .or_default()
                .update(&params, value, dt)
            {
                sink.stick(cmd, delta);
            }
        }
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
