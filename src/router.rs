//! What the pad's input means: turns SDL-free [`InputEvent`]s and axis
//! readings into commands, log messages and rumble. Plain Rust with time as a
//! parameter, so it is unit-tested without a gamepad; [`crate::pad`] feeds it
//! from SDL.

use crate::command::{Command, StickCommand};
use crate::config::{Axis, Config};
use crate::logic::{
    AxisTracker, Debouncer, Direction, Directions, HatRepeat, ModeDetector, MotorCmd, Pattern,
    RUMBLE_DIGITAL, RUMBLE_START, RUMBLE_STOP, RumbleQueue,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Where status and diagnostic messages go.
pub trait Log {
    /// Status and diagnostic messages.
    fn log(&mut self, msg: &str);
    /// Something the user has to act on: always shown.
    fn notice(&mut self, msg: &str) {
        self.log(msg);
    }
}

/// Receives what the pad does. Implemented by the REAPER side; keeps the
/// input handling free of REAPER calls.
pub trait Sink: Log {
    /// A button or D-pad command.
    fn command(&mut self, cmd: Command);
    /// Stick movement: a fractional number of "steps" since the last call;
    /// the sign is the direction. A fresh deflection starts with exactly ±1.
    fn stick(&mut self, cmd: StickCommand, delta: f32);
    /// Whether REAPER is recording right now (`None` = unknown).
    fn recording(&mut self) -> Option<bool> {
        None
    }
}

/// One input event from the connected pad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    /// Hardware button index, starting at 0.
    Button(u8),
    /// Hardware axis index and raw value.
    Axis(u8, i16),
    /// D-pad position `(x, y)`, with x: -1 = left, y: 1 = up.
    Hat((i8, i8)),
}

/// The connected pad, as far as the router needs to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Device {
    /// A DragonRise pad, which has a digital mode (see [`ModeDetector`]).
    pub dragonrise: bool,
}

/// Raw axis values beyond this are reported in test mode.
const TEST_AXIS_THRESHOLD: u16 = 16000;

/// What the pad tells the user by rumbling: scheduled patterns, and the
/// recording state whose changes trigger one.
#[derive(Debug, Default)]
struct Feedback {
    /// Last recording state REAPER reported (`None` before the first).
    recording: Option<bool>,
    queue: RumbleQueue,
}

impl Feedback {
    /// Rumble `pattern` from `now`, if rumble is on (`[pad] rumble`) and a
    /// pad is connected. Both are passed in rather than kept here, so there
    /// is only one copy of each, in [`Router`].
    fn rumble(&mut self, pattern: Pattern, now: Instant, enabled: bool, connected: bool) {
        if enabled && connected {
            self.queue.schedule(pattern, now);
        }
    }

    /// The pad was unplugged: pending rumble is dropped.
    fn disconnected(&mut self) {
        self.queue.clear();
    }

    /// Motor commands to send to the pad now.
    fn due(&mut self, now: Instant) -> Vec<MotorCmd> {
        self.queue.due(now)
    }

    /// REAPER's recording state. On a change: what to log and how to rumble.
    /// Nothing for the first report, the state REAPER was already in.
    fn recording_changed(&mut self, on: bool) -> Option<(&'static str, Pattern)> {
        match self.recording.replace(on) {
            Some(was) if was != on => Some(if on {
                ("● recording", RUMBLE_START)
            } else {
                ("■ recording stopped", RUMBLE_STOP)
            }),
            _ => None,
        }
    }
}

pub struct Router {
    cfg: Config,
    /// Only report button/axis numbers, don't send commands.
    test_mode: bool,
    device: Option<Device>,
    debouncer: Debouncer,
    hat: HatRepeat,
    axes: HashMap<Axis, AxisTracker>,
    last_tick: Instant,
    mode: ModeDetector,
    feedback: Feedback,
}

impl Router {
    pub fn new(cfg: Config, now: Instant) -> Self {
        Router {
            debouncer: Debouncer::new(Duration::from_millis(cfg.pad.debounce_ms)),
            hat: HatRepeat::new(Duration::from_millis(cfg.dpad.repeat_ms)),
            cfg,
            test_mode: false,
            device: None,
            axes: HashMap::new(),
            last_tick: now,
            mode: ModeDetector::default(),
            feedback: Feedback::default(),
        }
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

    pub fn test_mode(&self) -> bool {
        self.test_mode
    }

    pub fn set_test_mode(&mut self, on: bool) {
        self.test_mode = on;
    }

    pub fn connected(&mut self, device: Device) {
        self.device = Some(device);
    }

    /// The pad was unplugged: forget everything about its state.
    pub fn disconnected(&mut self, now: Instant) {
        self.device = None;
        self.axes.clear();
        self.hat.set((0, 0), now);
        self.feedback.disconnected();
        self.mode.reset();
    }

    pub fn rumble(&mut self, pattern: Pattern, now: Instant) {
        self.feedback
            .rumble(pattern, now, self.cfg.pad.rumble, self.device.is_some());
    }

    /// Motor commands to send to the pad now.
    pub fn rumble_due(&mut self, now: Instant) -> Vec<MotorCmd> {
        self.feedback.due(now)
    }

    pub fn event(&mut self, ev: InputEvent, now: Instant, sink: &mut dyn Sink) {
        if self.test_mode {
            match ev {
                InputEvent::Button(idx) => sink.log(&format!("button {}", u32::from(idx) + 1)),
                InputEvent::Axis(idx, value) => {
                    if value.unsigned_abs() > TEST_AXIS_THRESHOLD {
                        sink.log(&format!("axis {idx}: {value}"));
                    }
                }
                InputEvent::Hat(xy) => sink.log(&format!("D-pad {}", hat_name(xy))),
            }
            return;
        }
        match ev {
            InputEvent::Button(idx) => self.button(idx, now, sink),
            InputEvent::Hat(xy) => {
                let dirs = self.hat.set(xy, now);
                self.dpad(dirs, sink);
            }
            // Sticks are read in `tick`.
            InputEvent::Axis(..) => {}
        }
    }

    /// Everything that runs on time rather than on events: digital-mode
    /// detection, recording feedback, D-pad repeat and the sticks. Call once
    /// per poll, after the events; `read_axis` gives the current raw value of
    /// a hardware axis of the connected pad.
    pub fn tick(
        &mut self,
        now: Instant,
        read_axis: impl Fn(u32) -> Option<i16>,
        sink: &mut dyn Sink,
    ) {
        self.check_mode(now, &read_axis, sink);

        let dt = now.duration_since(self.last_tick).as_secs_f32().min(0.1);
        self.last_tick = now;
        if self.test_mode {
            return;
        }
        if let Some(on) = sink.recording() {
            self.on_record(on, now, sink);
        }
        let dirs = self.hat.tick(now);
        self.dpad(dirs, sink);
        self.sticks(dt, &read_axis, sink);
    }

    fn on_record(&mut self, on: bool, now: Instant, log: &mut dyn Log) {
        if let Some((msg, pattern)) = self.feedback.recording_changed(on) {
            log.log(msg);
            self.rumble(pattern, now);
        }
    }

    /// Watch for the digital mode of DragonRise pads (see [`ModeDetector`]).
    fn check_mode(
        &mut self,
        now: Instant,
        read_axis: &impl Fn(u32) -> Option<i16>,
        log: &mut dyn Log,
    ) {
        if !self.cfg.pad.detect_digital_mode || !self.device.is_some_and(|d| d.dragonrise) {
            return;
        }
        let x = read_axis(self.cfg.sticks.right.axis_x).unwrap_or(0);
        let y = read_axis(self.cfg.sticks.right.axis_y).unwrap_or(0);
        match self.mode.update(x, y, now) {
            Some(true) => {
                log.notice(
                    "Gamepad is in DIGITAL mode (LED green): the right stick would press \
                     buttons 1–4, so they are ignored. Press the Analog button (LED red).",
                );
                self.rumble(RUMBLE_DIGITAL, now);
            }
            Some(false) => log.notice("Gamepad is in analog mode — buttons 1–4 work again."),
            None => {}
        }
    }

    fn button(&mut self, idx: u8, now: Instant, sink: &mut dyn Sink) {
        let num = u32::from(idx) + 1; // number as printed on the pad
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
                sink.command(cmd);
            }
            None => sink.log(&format!("[{num}] (not assigned)")),
        }
    }

    fn dpad(&self, dirs: Directions, sink: &mut dyn Sink) {
        let d = &self.cfg.dpad;
        for dir in dirs {
            let cmd = match dir {
                Direction::Up => &d.up,
                Direction::Down => &d.down,
                Direction::Left => &d.left,
                Direction::Right => &d.right,
            };
            if let Some(cmd) = *cmd {
                sink.command(cmd);
            }
        }
    }

    fn sticks(&mut self, dt: f32, read_axis: &impl Fn(u32) -> Option<i16>, sink: &mut dyn Sink) {
        if self.device.is_none() {
            return;
        }
        for axis in Axis::ALL {
            let Some(cmd) = self.cfg.stick_command(axis) else {
                continue;
            };
            let value =
                read_axis(self.cfg.axis_index(axis)).map_or(0.0, |r| f32::from(r) / 32767.0);
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

/// D-pad position for the button test, e.g. "left-up".
fn hat_name((x, y): (i8, i8)) -> String {
    let vertical = match y {
        1 => Some("up"),
        -1 => Some("down"),
        _ => None,
    };
    let horizontal = match x {
        -1 => Some("left"),
        1 => Some("right"),
        _ => None,
    };
    match (horizontal, vertical) {
        (Some(h), Some(v)) => format!("{h}-{v}"),
        (Some(d), None) | (None, Some(d)) => d.into(),
        (None, None) => "centred".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    enum Out {
        Cmd(Command),
        Stick(StickCommand, f32),
        Log(String),
        Notice(String),
    }

    #[derive(Default)]
    struct TestSink {
        out: Vec<Out>,
        recording: Option<bool>,
    }

    impl TestSink {
        fn commands(&self) -> Vec<Command> {
            self.out
                .iter()
                .filter_map(|o| match o {
                    Out::Cmd(c) => Some(*c),
                    _ => None,
                })
                .collect()
        }

        fn logs(&self) -> Vec<&str> {
            self.out
                .iter()
                .filter_map(|o| match o {
                    Out::Log(m) | Out::Notice(m) => Some(m.as_str()),
                    _ => None,
                })
                .collect()
        }

        fn sticks(&self) -> Vec<(StickCommand, f32)> {
            self.out
                .iter()
                .filter_map(|o| match o {
                    Out::Stick(c, d) => Some((*c, *d)),
                    _ => None,
                })
                .collect()
        }
    }

    impl Sink for TestSink {
        fn command(&mut self, cmd: Command) {
            self.out.push(Out::Cmd(cmd));
        }
        fn stick(&mut self, cmd: StickCommand, delta: f32) {
            self.out.push(Out::Stick(cmd, delta));
        }
        fn recording(&mut self) -> Option<bool> {
            self.recording
        }
    }

    impl Log for TestSink {
        fn log(&mut self, msg: &str) {
            self.out.push(Out::Log(msg.into()));
        }
        fn notice(&mut self, msg: &str) {
            self.out.push(Out::Notice(msg.into()));
        }
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    const CFG: &str = r#"
        [buttons]
        1 = "play_stop"
        2 = "record"
        5 = "metronome"
        [dpad]
        up = "undo"
        right = "redo"
        repeat_ms = 200
        [sticks.left]
        x = "scroll_x"
        [pad]
        debounce_ms = 100
    "#;

    fn router(t0: Instant, dragonrise: bool) -> Router {
        let mut r = Router::new(Config::parse(CFG).unwrap(), t0);
        r.connected(Device { dragonrise });
        r
    }

    fn centred(_: u32) -> Option<i16> {
        Some(0)
    }

    /// Right stick axes frozen as in digital mode; everything else centred.
    fn digital_axes(axis: u32) -> Option<i16> {
        Some(if axis == 3 || axis == 4 { -129 } else { 0 })
    }

    #[test]
    fn buttons_run_their_commands() {
        let t0 = Instant::now();
        let mut r = router(t0, false);
        let mut sink = TestSink::default();
        r.event(InputEvent::Button(0), t0, &mut sink);
        r.event(InputEvent::Button(1), t0, &mut sink);
        r.event(InputEvent::Button(2), t0, &mut sink);
        assert_eq!(sink.commands(), vec![Command::PlayStop, Command::Record]);
        assert!(sink.logs().contains(&"[3] (not assigned)"));
    }

    #[test]
    fn bouncing_button_runs_once() {
        let t0 = Instant::now();
        let mut r = router(t0, false);
        let mut sink = TestSink::default();
        r.event(InputEvent::Button(0), t0, &mut sink);
        r.event(InputEvent::Button(0), t0 + ms(30), &mut sink);
        r.event(InputEvent::Button(0), t0 + ms(300), &mut sink);
        assert_eq!(sink.commands(), vec![Command::PlayStop, Command::PlayStop]);
    }

    #[test]
    fn test_mode_only_reports() {
        let t0 = Instant::now();
        let mut r = router(t0, false);
        r.set_test_mode(true);
        let mut sink = TestSink::default();
        r.event(InputEvent::Button(1), t0, &mut sink);
        r.event(InputEvent::Hat((0, 1)), t0, &mut sink);
        r.event(InputEvent::Hat((-1, -1)), t0, &mut sink);
        r.event(InputEvent::Hat((0, 0)), t0, &mut sink);
        r.event(InputEvent::Axis(0, 100), t0, &mut sink);
        r.event(InputEvent::Axis(0, -30000), t0, &mut sink);
        r.tick(t0 + ms(500), |_| Some(32767), &mut sink);
        assert!(sink.commands().is_empty());
        assert!(sink.sticks().is_empty());
        assert_eq!(
            sink.logs(),
            vec![
                "button 2",
                "D-pad up",
                "D-pad left-down",
                "D-pad centred",
                "axis 0: -30000"
            ]
        );
    }

    #[test]
    fn dpad_fires_and_repeats_while_held() {
        let t0 = Instant::now();
        let mut r = router(t0, false);
        let mut sink = TestSink::default();
        r.event(InputEvent::Hat((1, 1)), t0, &mut sink);
        assert_eq!(sink.commands(), vec![Command::Undo, Command::Redo]);
        r.tick(t0 + ms(100), centred, &mut sink);
        assert_eq!(sink.commands().len(), 2);
        r.tick(t0 + ms(200), centred, &mut sink);
        assert_eq!(sink.commands().len(), 4);
        r.event(InputEvent::Hat((0, 0)), t0 + ms(250), &mut sink);
        r.tick(t0 + ms(1000), centred, &mut sink);
        assert_eq!(sink.commands().len(), 4);
    }

    #[test]
    fn unbound_dpad_direction_does_nothing() {
        let t0 = Instant::now();
        let mut r = router(t0, false);
        let mut sink = TestSink::default();
        r.event(InputEvent::Hat((0, -1)), t0, &mut sink);
        assert!(sink.commands().is_empty());
    }

    #[test]
    fn stick_moves_only_while_connected() {
        let t0 = Instant::now();
        let mut r = router(t0, false);
        let mut sink = TestSink::default();
        r.tick(
            t0 + ms(33),
            |a| Some(if a == 0 { 32767 } else { 0 }),
            &mut sink,
        );
        assert_eq!(sink.sticks(), vec![(StickCommand::ScrollX, 1.0)]);

        r.disconnected(t0 + ms(40));
        r.tick(
            t0 + ms(66),
            |a| Some(if a == 0 { 32767 } else { 0 }),
            &mut sink,
        );
        assert_eq!(sink.sticks().len(), 1);
    }

    #[test]
    fn digital_mode_ignores_buttons_1_to_4() {
        let t0 = Instant::now();
        let mut r = router(t0, true);
        let mut sink = TestSink::default();
        r.tick(t0, digital_axes, &mut sink);
        r.tick(t0 + ModeDetector::HOLD, digital_axes, &mut sink);
        assert!(sink.logs().iter().any(|m| m.contains("DIGITAL mode")));

        let t1 = t0 + ms(1000);
        r.event(InputEvent::Button(1), t1, &mut sink);
        r.event(InputEvent::Button(4), t1, &mut sink);
        assert_eq!(sink.commands(), vec![Command::Metronome]);
        assert!(
            sink.logs()
                .contains(&"[2] ignored — digital mode (press Analog, LED red)")
        );

        r.tick(t1, centred, &mut sink);
        assert!(sink.logs().iter().any(|m| m.contains("analog mode")));
        r.event(InputEvent::Button(1), t1 + ms(500), &mut sink);
        assert_eq!(sink.commands().last(), Some(&Command::Record));
    }

    #[test]
    fn digital_mode_is_only_detected_on_dragonrise_pads() {
        let t0 = Instant::now();
        let mut r = router(t0, false);
        let mut sink = TestSink::default();
        r.tick(t0, digital_axes, &mut sink);
        r.tick(t0 + ms(1000), digital_axes, &mut sink);
        r.event(InputEvent::Button(1), t0 + ms(1000), &mut sink);
        assert_eq!(sink.commands(), vec![Command::Record]);
    }

    #[test]
    fn recording_changes_rumble_but_initial_state_does_not() {
        let t0 = Instant::now();
        let mut r = router(t0, false);
        let mut sink = TestSink {
            recording: Some(false),
            ..Default::default()
        };
        r.tick(t0, centred, &mut sink);
        assert!(
            r.rumble_due(t0).is_empty(),
            "no rumble for the initial state"
        );

        sink.recording = Some(true);
        r.tick(t0 + ms(33), centred, &mut sink);
        assert_eq!(r.rumble_due(t0 + ms(33)), vec![(65535, 0, 400)]);
        assert!(sink.logs().contains(&"● recording"));
    }

    #[test]
    fn only_changes_of_the_recording_state_count() {
        let mut f = Feedback::default();
        assert_eq!(f.recording_changed(true), None, "initial state");
        assert_eq!(f.recording_changed(true), None, "no change");
        assert_eq!(
            f.recording_changed(false),
            Some(("■ recording stopped", RUMBLE_STOP))
        );
        assert_eq!(
            f.recording_changed(true),
            Some(("● recording", RUMBLE_START))
        );
    }

    #[test]
    fn no_rumble_without_a_pad_or_when_disabled() {
        let t0 = Instant::now();
        let mut r = Router::new(Config::parse(CFG).unwrap(), t0);
        r.rumble(RUMBLE_START, t0);
        assert!(r.rumble_due(t0).is_empty(), "not connected");

        let mut r = Router::new(Config::parse("[pad]\nrumble = false\n").unwrap(), t0);
        r.connected(Device { dragonrise: false });
        r.rumble(RUMBLE_START, t0);
        assert!(r.rumble_due(t0).is_empty(), "rumble = false");
    }
}
