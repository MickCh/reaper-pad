//! Hardware-independent input logic. Everything here takes time as a
//! parameter, so it can be unit-tested without a gamepad.

use crate::command::StickCommand;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Filters contact bounce: a press closer than `window` to the previous press
/// (or bounce) of the same button is ignored.
#[derive(Debug)]
pub struct Debouncer {
    window: Duration,
    last: HashMap<u32, Instant>,
}

impl Debouncer {
    pub fn new(window: Duration) -> Self {
        Debouncer {
            window,
            last: HashMap::new(),
        }
    }

    pub fn set_window(&mut self, window: Duration) {
        self.window = window;
    }

    /// Returns `true` if this press should be acted on.
    pub fn accept(&mut self, button: u32, now: Instant) -> bool {
        let bounced = self
            .last
            .get(&button)
            .is_some_and(|t| now.saturating_duration_since(*t) < self.window);
        self.last.insert(button, now);
        !bounced
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// The directions of one D-pad position: up to two (a diagonal), without
/// allocating. Vertical comes first.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Directions([Option<Direction>; 2]);

impl IntoIterator for Directions {
    type Item = Direction;
    type IntoIter = std::iter::Flatten<std::array::IntoIter<Option<Direction>, 2>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter().flatten()
    }
}

/// D-pad with auto-repeat while held.
#[derive(Debug)]
pub struct HatRepeat {
    interval: Duration,
    state: (i8, i8),
    next: Option<Instant>,
}

impl HatRepeat {
    pub fn new(interval: Duration) -> Self {
        HatRepeat {
            interval,
            state: (0, 0),
            next: None,
        }
    }

    pub fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    /// New D-pad position `(x, y)`, with x: -1 = left, y: 1 = up.
    /// Fires immediately on any non-neutral position.
    pub fn set(&mut self, state: (i8, i8), now: Instant) -> Directions {
        self.state = state;
        if state == (0, 0) {
            self.next = None;
            return Directions::default();
        }
        self.next = Some(now + self.interval);
        directions(state)
    }

    /// Repeats while the D-pad is held.
    pub fn tick(&mut self, now: Instant) -> Directions {
        match self.next {
            Some(t) if now >= t => {
                self.next = Some(now + self.interval);
                directions(self.state)
            }
            _ => Directions::default(),
        }
    }
}

fn directions((x, y): (i8, i8)) -> Directions {
    let vertical = match y {
        1 => Some(Direction::Up),
        -1 => Some(Direction::Down),
        _ => None,
    };
    let horizontal = match x {
        -1 => Some(Direction::Left),
        1 => Some(Direction::Right),
        _ => None,
    };
    Directions([vertical, horizontal])
}

/// Parameters for one stick axis.
#[derive(Debug, Clone, Copy)]
pub struct AxisParams {
    pub deadzone: f32,
    pub min_steps_per_sec: f32,
    pub max_steps_per_sec: f32,
    pub speed: f32,
    /// Flip the direction (e.g. so that pushing up means "up").
    pub flip: bool,
}

impl AxisParams {
    /// Signed speed in steps per second for a normalized axis value (-1..1).
    /// Zero inside the dead zone; quadratic curve for fine control near the centre.
    pub fn rate(&self, value: f32) -> f32 {
        let v = value.clamp(-1.0, 1.0);
        if v.abs() < self.deadzone || self.deadzone >= 1.0 {
            return 0.0;
        }
        let mag = (v.abs() - self.deadzone) / (1.0 - self.deadzone);
        let rate = (self.min_steps_per_sec
            + (self.max_steps_per_sec - self.min_steps_per_sec) * mag * mag)
            * self.speed;
        let sign = if (v > 0.0) == self.flip { -1.0 } else { 1.0 };
        sign * rate
    }
}

/// Turns axis positions into movement "deltas" (fractional steps).
/// A new deflection (or direction change) yields an immediate ±1 step, so the
/// response feels instant; holding it then moves at `rate` steps per second.
#[derive(Debug, Default)]
pub struct AxisTracker {
    dir: i8,
}

impl AxisTracker {
    pub fn update(&mut self, params: &AxisParams, value: f32, dt: f32) -> Option<f32> {
        let rate = params.rate(value);
        if rate == 0.0 {
            self.dir = 0;
            return None;
        }
        let dir: i8 = if rate > 0.0 { 1 } else { -1 };
        if dir == self.dir {
            Some(rate * dt)
        } else {
            self.dir = dir;
            Some(f32::from(dir))
        }
    }
}

/// Accumulates fractional deltas into whole steps, for targets that only
/// understand ±1 (REAPER's `CSurf_OnScroll` / `CSurf_OnZoom`).
#[derive(Debug, Default)]
pub struct Stepper(HashMap<StickCommand, f32>);

impl Stepper {
    // The result is a whole number of steps per tick: far inside i32.
    #[allow(clippy::cast_possible_truncation)]
    pub fn steps(&mut self, key: StickCommand, delta: f32) -> i32 {
        let acc = self.0.entry(key).or_insert(0.0);
        if *acc * delta < 0.0 {
            *acc = 0.0; // direction changed: drop the leftover
        }
        *acc += delta;
        let n = acc.trunc();
        *acc -= n;
        n as i32
    }
}

/// A rumble pattern: (strong motor?, power 0.0–1.0, duration ms, pause after ms).
pub type Pattern = &'static [(bool, f32, u32, u32)];

/// Rumble when recording starts.
pub const RUMBLE_START: Pattern = &[(true, 1.0, 400, 0)];
/// Rumble when recording stops.
pub const RUMBLE_STOP: Pattern = &[(false, 0.8, 120, 100), (false, 0.8, 120, 0)];
/// Short, gentle "hello"/confirmation.
pub const RUMBLE_HELLO: Pattern = &[(false, 1.0, 120, 0)];

/// Three short buzzes: the pad is in digital mode.
pub const RUMBLE_DIGITAL: Pattern = &[
    (false, 1.0, 90, 110),
    (false, 1.0, 90, 110),
    (false, 1.0, 90, 0),
];

/// One motor command: (strong motor, weak motor, duration ms). (0, 0) = stop.
pub type MotorCmd = (u16, u16, u32);

/// Schedules rumble patterns. Every pulse is followed by an explicit stop, and
/// pulses that are due too late (e.g. the host froze) are skipped — some
/// platforms only end a rumble when the device is polled again, so a late
/// pulse could otherwise run for seconds.
#[derive(Debug, Default)]
pub struct RumbleQueue {
    entries: Vec<(Instant, MotorCmd)>,
}

impl RumbleQueue {
    pub const MAX_LATE: Duration = Duration::from_millis(300);

    pub fn schedule(&mut self, pattern: Pattern, start: Instant) {
        let mut t = start;
        for &(strong, level, ms, pause) in pattern {
            // Clamped to 0..=65535 first, so the cast is exact up to rounding.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let v = (level.clamp(0.0, 1.0) * 65535.0) as u16;
            let (lo, hi) = if strong { (v, 0) } else { (0, v) };
            self.entries.push((t, (lo, hi, ms)));
            t += Duration::from_millis(u64::from(ms));
            self.entries.push((t, (0, 0, 0)));
            t += Duration::from_millis(u64::from(pause));
        }
    }

    /// Motor commands to send now, in order.
    pub fn due(&mut self, now: Instant) -> Vec<MotorCmd> {
        let mut out = Vec::new();
        self.entries.retain(|&(at, cmd)| {
            if at > now {
                return true;
            }
            let is_stop = cmd.0 == 0 && cmd.1 == 0;
            if is_stop || now.duration_since(at) <= Self::MAX_LATE {
                out.push(cmd);
            }
            false
        });
        out
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Detects the "digital" mode of DragonRise pads (Art AG-16T and clones;
/// LED green instead of red). In that mode the right stick is reported as
/// buttons 1–4 and its axes freeze just below the centre (raw 127 of 0–255,
/// SDL value -129), while in analog mode they rest just above it (raw 128).
/// Both right axes have to sit at the same such value for [`Self::HOLD`]
/// before the pad counts as digital; any other value means analog again.
#[derive(Debug, Default)]
pub struct ModeDetector {
    candidate_since: Option<Instant>,
    digital: bool,
}

impl ModeDetector {
    pub const HOLD: Duration = Duration::from_millis(500);

    fn looks_digital(x: i16, y: i16) -> bool {
        x == y && (-1024..0).contains(&x)
    }

    /// Feed the current right-stick axis values; returns the new mode
    /// (`true` = digital) when it changes.
    pub fn update(&mut self, x: i16, y: i16, now: Instant) -> Option<bool> {
        let digital = if Self::looks_digital(x, y) {
            let since = *self.candidate_since.get_or_insert(now);
            self.digital || now.duration_since(since) >= Self::HOLD
        } else {
            self.candidate_since = None;
            false
        };
        (digital != self.digital).then(|| {
            self.digital = digital;
            digital
        })
    }

    pub fn digital(&self) -> bool {
        self.digital
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn dirs(d: Directions) -> Vec<Direction> {
        d.into_iter().collect()
    }

    #[test]
    fn debouncer_ignores_bounces_but_not_other_buttons() {
        let t0 = Instant::now();
        let mut d = Debouncer::new(ms(120));
        assert!(d.accept(1, t0));
        assert!(!d.accept(1, t0 + ms(40)), "bounce");
        assert!(!d.accept(1, t0 + ms(140)), "bounce extends the window");
        assert!(d.accept(1, t0 + ms(300)));
        assert!(d.accept(2, t0 + ms(301)), "other button is independent");
    }

    #[test]
    fn hat_fires_immediately_and_repeats() {
        let t0 = Instant::now();
        let mut h = HatRepeat::new(ms(250));
        assert_eq!(dirs(h.set((0, 1), t0)), vec![Direction::Up]);
        assert!(dirs(h.tick(t0 + ms(100))).is_empty());
        assert_eq!(dirs(h.tick(t0 + ms(250))), vec![Direction::Up]);
        assert!(dirs(h.tick(t0 + ms(300))).is_empty());
        assert!(dirs(h.set((0, 0), t0 + ms(310))).is_empty());
        assert!(
            dirs(h.tick(t0 + ms(1000))).is_empty(),
            "released: no repeat"
        );
    }

    #[test]
    fn hat_diagonal_gives_two_directions() {
        let mut h = HatRepeat::new(ms(250));
        assert_eq!(
            dirs(h.set((1, -1), Instant::now())),
            vec![Direction::Down, Direction::Right]
        );
        assert!(dirs(directions((0, 0))).is_empty());
    }

    fn params() -> AxisParams {
        AxisParams {
            deadzone: 0.15,
            min_steps_per_sec: 3.0,
            max_steps_per_sec: 40.0,
            speed: 1.0,
            flip: false,
        }
    }

    #[test]
    fn axis_rate_curve() {
        let p = params();
        assert_eq!(p.rate(0.1), 0.0, "inside dead zone");
        assert_eq!(p.rate(-0.1), 0.0);
        assert!((p.rate(1.0) - 40.0).abs() < 1e-4, "full deflection = max");
        assert!((p.rate(-1.0) + 40.0).abs() < 1e-4);
        assert!((p.rate(0.15) - 3.0).abs() < 1e-4, "edge of dead zone = min");
        assert!(p.rate(0.5) > p.rate(0.3) && p.rate(0.3) > 0.0, "monotonic");
        assert!((p.rate(5.0) - 40.0).abs() < 1e-4, "clamped");
    }

    #[test]
    fn axis_speed_and_flip() {
        let p = AxisParams {
            speed: 0.5,
            flip: true,
            ..params()
        };
        assert!((p.rate(1.0) + 20.0).abs() < 1e-4);
    }

    #[test]
    fn axis_tracker_first_step_is_immediate() {
        let p = params();
        let mut t = AxisTracker::default();
        assert_eq!(t.update(&p, 1.0, 0.01), Some(1.0), "immediate step");
        let d = t.update(&p, 1.0, 0.01).unwrap();
        assert!((d - 0.4).abs() < 1e-4, "40/s * 10 ms");
        assert_eq!(
            t.update(&p, -1.0, 0.01),
            Some(-1.0),
            "direction change is immediate"
        );
        assert_eq!(t.update(&p, 0.0, 0.01), None);
        assert_eq!(
            t.update(&p, -1.0, 0.01),
            Some(-1.0),
            "re-deflection is immediate"
        );
    }

    #[test]
    fn stepper_counts_whole_steps() {
        let mut s = Stepper::default();
        assert_eq!(s.steps(StickCommand::ScrollX, 1.0), 1);
        assert_eq!(s.steps(StickCommand::ScrollX, 0.4), 0);
        assert_eq!(s.steps(StickCommand::ScrollX, 0.7), 1);
        assert_eq!(
            s.steps(StickCommand::ScrollX, -1.0),
            -1,
            "direction change drops leftover"
        );
        assert_eq!(
            s.steps(StickCommand::ScrollY, 2.5),
            2,
            "keys are independent"
        );
    }

    #[test]
    fn rumble_pulses_are_followed_by_stops() {
        let t0 = Instant::now();
        let mut q = RumbleQueue::default();
        q.schedule(RUMBLE_STOP, t0);
        assert_eq!(q.due(t0), vec![(0, 52428, 120)]);
        assert_eq!(q.due(t0 + ms(120)), vec![(0, 0, 0)]);
        assert!(q.due(t0 + ms(150)).is_empty());
        assert_eq!(q.due(t0 + ms(220)), vec![(0, 52428, 120)]);
        assert_eq!(q.due(t0 + ms(340)), vec![(0, 0, 0)]);
        assert!(q.is_empty());
    }

    #[test]
    fn strong_motor_uses_low_frequency_channel() {
        let t0 = Instant::now();
        let mut q = RumbleQueue::default();
        q.schedule(RUMBLE_START, t0);
        assert_eq!(q.due(t0), vec![(65535, 0, 400)]);
    }

    #[test]
    fn late_rumble_is_skipped_but_stop_is_sent() {
        let t0 = Instant::now();
        let mut q = RumbleQueue::default();
        q.schedule(RUMBLE_HELLO, t0);
        // host froze for 5 s
        assert_eq!(q.due(t0 + ms(5000)), vec![(0, 0, 0)]);
        assert!(q.is_empty());
    }

    #[test]
    fn mode_detector_needs_a_steady_digital_signature() {
        let t0 = Instant::now();
        let mut m = ModeDetector::default();
        assert_eq!(m.update(128, 128, t0), None, "analog at rest");
        assert_eq!(m.update(-129, -129, t0 + ms(100)), None);
        assert_eq!(m.update(-129, -129, t0 + ms(400)), None, "not long enough");
        assert_eq!(m.update(-129, -129, t0 + ms(600)), Some(true));
        assert!(m.digital());
        assert_eq!(m.update(-129, -129, t0 + ms(5000)), None);
        assert_eq!(
            m.update(128, 128, t0 + ms(5100)),
            Some(false),
            "LED red again"
        );
        assert!(!m.digital());
    }

    #[test]
    fn mode_detector_ignores_a_moving_stick() {
        let t0 = Instant::now();
        let mut m = ModeDetector::default();
        for i in 0..40u64 {
            // passes through equal negative values, but never for long
            let v = -200 + (i as i16 % 3) * 50;
            assert_eq!(m.update(v, v - (i % 2) as i16, t0 + ms(i * 33)), None);
        }
        assert_eq!(m.update(-129, -5000, t0 + ms(2000)), None, "axes differ");
        assert_eq!(m.update(-20000, -20000, t0 + ms(3000)), None, "deflected");
        assert_eq!(m.update(-20000, -20000, t0 + ms(4000)), None);
    }
}
