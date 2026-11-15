//! Configuration file (TOML). See `config.toml` in the repository root for
//! the documented default.

use crate::command::{Command, StickCommand};
use crate::logic::AxisParams;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};

/// The default configuration, embedded into the binaries.
pub const DEFAULT_CONFIG: &str = include_str!("../config.toml");

/// Highest button number accepted in `[buttons]`.
pub const MAX_BUTTON: u32 = 32;

/// What is wrong with a configuration.
#[derive(Debug)]
pub enum ConfigError {
    /// Not valid TOML, or an unknown option or command (the message names it).
    Syntax(toml::de::Error),
    /// Valid TOML with an unacceptable value.
    Invalid(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Syntax(e) => write!(f, "{e}"),
            ConfigError::Invalid(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Syntax(e) => Some(e),
            ConfigError::Invalid(_) => None,
        }
    }
}

/// A configuration file that could not be used.
#[derive(Debug)]
pub enum LoadError {
    Read(PathBuf, std::io::Error),
    Write(PathBuf, std::io::Error),
    Config(PathBuf, ConfigError),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Read(p, e) => write!(f, "cannot read {}: {e}", p.display()),
            LoadError::Write(p, e) => write!(f, "cannot write {}: {e}", p.display()),
            LoadError::Config(p, e) => write!(f, "error in {}: {e}", p.display()),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LoadError::Read(_, e) | LoadError::Write(_, e) => Some(e),
            LoadError::Config(_, e) => Some(e),
        }
    }
}

/// A parsed and validated configuration. Only built through
/// [`Config::parse`] / [`Config::load`], so it is always valid.
#[derive(Debug)]
pub struct Config {
    /// Button number (as printed on the pad, starting at 1) → command.
    buttons: BTreeMap<u32, Command>,
    pub dpad: Dpad,
    pub sticks: Sticks,
    pub pad: PadCfg,
}

/// The file as written: button numbers are still strings (TOML keys).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    buttons: HashMap<String, Command>,
    #[serde(default)]
    dpad: Dpad,
    #[serde(default)]
    sticks: RawSticks,
    #[serde(default)]
    pad: PadCfg,
}

impl TryFrom<RawConfig> for Config {
    type Error = ConfigError;

    fn try_from(raw: RawConfig) -> Result<Self, ConfigError> {
        let buttons = raw
            .buttons
            .into_iter()
            .map(|(k, v)| match k.trim().parse::<u32>() {
                Ok(n) if (1..=MAX_BUTTON).contains(&n) => Ok((n, v)),
                _ => Err(ConfigError::Invalid(format!(
                    "[buttons]: invalid button number \"{k}\" (expected 1–{MAX_BUTTON})"
                ))),
            })
            .collect::<Result<_, _>>()?;
        let sticks = raw.sticks.resolve();
        validate(&sticks).map_err(ConfigError::Invalid)?;
        Ok(Config {
            buttons,
            dpad: raw.dpad,
            sticks,
            pad: raw.pad,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Dpad {
    pub up: Option<Command>,
    pub down: Option<Command>,
    pub left: Option<Command>,
    pub right: Option<Command>,
    /// Auto-repeat interval while the D-pad is held, in milliseconds.
    pub repeat_ms: u64,
}

impl Default for Dpad {
    fn default() -> Self {
        Dpad {
            up: None,
            down: None,
            left: None,
            right: None,
            repeat_ms: 250,
        }
    }
}

/// `[sticks]`: settings shared by both sticks, and one [`Stick`] each.
#[derive(Debug)]
pub struct Sticks {
    pub left: Stick,
    pub right: Stick,
    pub deadzone: f32,
    pub min_steps_per_sec: f32,
    pub max_steps_per_sec: f32,
    /// Smooth horizontal zoom: REAPER zoom amount per stick step.
    pub zoom_amount: f64,
}

/// `[sticks.left]` / `[sticks.right]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Stick {
    /// Command for the horizontal axis.
    pub x: Option<StickCommand>,
    /// Command for the vertical axis.
    pub y: Option<StickCommand>,
    /// Speed multiplier (lower = slower).
    pub speed: f32,
    /// Invert the vertical direction.
    pub invert_y: bool,
    /// Hardware axis numbers as reported by SDL.
    pub axis_x: u32,
    pub axis_y: u32,
}

impl Stick {
    /// Defaults per side: only the axis numbers differ (they fit most
    /// twin-stick pads).
    fn default_for(axis_x: u32, axis_y: u32) -> Self {
        Stick {
            x: None,
            y: None,
            speed: 1.0,
            invert_y: false,
            axis_x,
            axis_y,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct RawSticks {
    left: RawStick,
    right: RawStick,
    deadzone: f32,
    min_steps_per_sec: f32,
    max_steps_per_sec: f32,
    zoom_amount: f64,
}

impl Default for RawSticks {
    fn default() -> Self {
        RawSticks {
            left: RawStick::default(),
            right: RawStick::default(),
            deadzone: 0.15,
            min_steps_per_sec: 3.0,
            max_steps_per_sec: 40.0,
            zoom_amount: 0.2,
        }
    }
}

impl RawSticks {
    fn resolve(self) -> Sticks {
        Sticks {
            left: self.left.or(&Stick::default_for(0, 1)),
            right: self.right.or(&Stick::default_for(3, 4)),
            deadzone: self.deadzone,
            min_steps_per_sec: self.min_steps_per_sec,
            max_steps_per_sec: self.max_steps_per_sec,
            zoom_amount: self.zoom_amount,
        }
    }
}

/// A stick as written: anything left out comes from that side's defaults.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStick {
    x: Option<StickCommand>,
    y: Option<StickCommand>,
    speed: Option<f32>,
    invert_y: Option<bool>,
    axis_x: Option<u32>,
    axis_y: Option<u32>,
}

impl RawStick {
    fn or(self, d: &Stick) -> Stick {
        Stick {
            x: self.x.or(d.x),
            y: self.y.or(d.y),
            speed: self.speed.unwrap_or(d.speed),
            invert_y: self.invert_y.unwrap_or(d.invert_y),
            axis_x: self.axis_x.unwrap_or(d.axis_x),
            axis_y: self.axis_y.unwrap_or(d.axis_y),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct PadCfg {
    /// Ignore repeated presses of the same button within this many ms.
    pub debounce_ms: u64,
    /// Vibrate when recording starts/stops and on startup.
    pub rumble: bool,
    /// Warn about (and ignore buttons 1–4 in) the digital mode of DragonRise
    /// pads such as the Art AG-16T.
    pub detect_digital_mode: bool,
}

impl Default for PadCfg {
    fn default() -> Self {
        PadCfg {
            debounce_ms: 120,
            rumble: true,
            detect_digital_mode: true,
        }
    }
}

/// Which stick axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    LeftX,
    LeftY,
    RightX,
    RightY,
}

impl Axis {
    pub const ALL: [Axis; 4] = [Axis::LeftX, Axis::LeftY, Axis::RightX, Axis::RightY];
}

impl Config {
    /// Parse and validate a configuration.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let raw: RawConfig = toml::from_str(text).map_err(|e| match renamed_options(text) {
            Some(hint) => ConfigError::Invalid(hint),
            None => ConfigError::Syntax(e),
        })?;
        raw.try_into()
    }

    pub fn load(path: &Path) -> Result<Self, LoadError> {
        let text = std::fs::read_to_string(path).map_err(|e| LoadError::Read(path.into(), e))?;
        Self::parse(&text).map_err(|e| LoadError::Config(path.into(), e))
    }

    /// Command for a button number as printed on the pad (1-based).
    pub fn button(&self, num: u32) -> Option<Command> {
        self.buttons.get(&num).copied()
    }

    /// The stick an axis belongs to, and whether it is the vertical one.
    fn stick(&self, axis: Axis) -> (&Stick, bool) {
        let s = &self.sticks;
        match axis {
            Axis::LeftX => (&s.left, false),
            Axis::LeftY => (&s.left, true),
            Axis::RightX => (&s.right, false),
            Axis::RightY => (&s.right, true),
        }
    }

    pub fn stick_command(&self, axis: Axis) -> Option<StickCommand> {
        let (st, vertical) = self.stick(axis);
        if vertical { st.y } else { st.x }
    }

    /// Hardware axis index for a stick axis.
    pub fn axis_index(&self, axis: Axis) -> u32 {
        let (st, vertical) = self.stick(axis);
        if vertical { st.axis_y } else { st.axis_x }
    }

    pub fn axis_params(&self, axis: Axis) -> AxisParams {
        let (st, vertical) = self.stick(axis);
        let s = &self.sticks;
        AxisParams {
            deadzone: s.deadzone,
            min_steps_per_sec: s.min_steps_per_sec,
            max_steps_per_sec: s.max_steps_per_sec,
            speed: st.speed,
            // Gamepads report "up" as negative; flip so that up means up.
            flip: vertical && !st.invert_y,
        }
    }
}

fn validate(s: &Sticks) -> Result<(), String> {
    // TOML accepts `nan` and `inf`; comparisons with NaN are always false,
    // so the range checks below would let them through.
    for (name, v) in [
        ("deadzone", f64::from(s.deadzone)),
        ("min_steps_per_sec", f64::from(s.min_steps_per_sec)),
        ("max_steps_per_sec", f64::from(s.max_steps_per_sec)),
        ("zoom_amount", s.zoom_amount),
    ] {
        if !v.is_finite() {
            return Err(format!("[sticks] {name} must be a finite number (got {v})"));
        }
    }
    if !(0.0..1.0).contains(&s.deadzone) {
        return Err(format!(
            "[sticks] deadzone must be between 0.0 and 1.0 (got {})",
            s.deadzone
        ));
    }
    if s.min_steps_per_sec < 0.0 || s.max_steps_per_sec < s.min_steps_per_sec {
        return Err("[sticks] expected 0 <= min_steps_per_sec <= max_steps_per_sec".into());
    }
    if s.zoom_amount < 0.0 {
        return Err("[sticks] zoom_amount must not be negative".into());
    }
    for (side, st) in [("left", &s.left), ("right", &s.right)] {
        if !st.speed.is_finite() || st.speed < 0.0 {
            return Err(format!(
                "[sticks.{side}] speed must be a non-negative number (got {})",
                st.speed
            ));
        }
    }
    Ok(())
}

/// Options renamed in 0.4: (old section, old name, new section, new name).
const RENAMED: [(&str, &str, &str, &str); 12] = [
    ("sticks", "left_x", "sticks.left", "x"),
    ("sticks", "left_y", "sticks.left", "y"),
    ("sticks", "right_x", "sticks.right", "x"),
    ("sticks", "right_y", "sticks.right", "y"),
    ("sticks", "speed_left", "sticks.left", "speed"),
    ("sticks", "speed_right", "sticks.right", "speed"),
    ("sticks", "invert_left_y", "sticks.left", "invert_y"),
    ("sticks", "invert_right_y", "sticks.right", "invert_y"),
    ("pad", "axis_left_x", "sticks.left", "axis_x"),
    ("pad", "axis_left_y", "sticks.left", "axis_y"),
    ("pad", "axis_right_x", "sticks.right", "axis_x"),
    ("pad", "axis_right_y", "sticks.right", "axis_y"),
];

/// If the file uses option names from before 0.4, say where they moved,
/// and what else is wrong once they are in their new place (the first
/// unknown option stops parsing, so those errors would only show up later).
/// Only called for a file that did not parse, so it may be slow.
fn renamed_options(text: &str) -> Option<String> {
    let mut table: toml::Table = toml::from_str(text).ok()?;
    let mut moved = Vec::new();
    for &(section, old, new_section, new) in &RENAMED {
        let Some(value) = table
            .get_mut(section)
            .and_then(toml::Value::as_table_mut)
            .and_then(|t| t.remove(old))
        else {
            continue;
        };
        moved.push(format!("  [{section}] {old} → [{new_section}] {new}"));
        let mut target = &mut table;
        for part in new_section.split('.') {
            target = target
                .entry(part)
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()?;
        }
        // An option already set under the new name wins.
        target.entry(new).or_insert(value);
    }
    if moved.is_empty() {
        return None;
    }
    let msg = format!(
        "options renamed in reaper-pad 0.4 (see the default config.toml):\n{}",
        moved.join("\n")
    );
    let rest = toml::Value::Table(table)
        .try_into::<RawConfig>()
        .map_err(|e| ConfigError::Syntax(e).to_string())
        .and_then(|raw| Config::try_from(raw).map_err(|e| e.to_string()));
    Some(match rest {
        Ok(_) => msg,
        Err(e) => format!("{msg}\nalso: {}", e.trim_end()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let c = Config::parse(DEFAULT_CONFIG).unwrap();
        assert_eq!(c.button(1), Some(Command::PlayStop));
        assert_eq!(c.button(2), Some(Command::Record));
        assert_eq!(c.buttons.len(), 12);
        assert_eq!(c.button(13), None);
        assert_eq!(c.stick_command(Axis::RightX), Some(StickCommand::ZoomX));
        assert!(c.dpad.up.is_some() && c.dpad.right.is_some());
    }

    #[test]
    fn minimal_config_uses_defaults() {
        let c = Config::parse("").unwrap();
        assert!(c.buttons.is_empty());
        assert_eq!(c.pad.debounce_ms, 120);
        assert_eq!(c.sticks.zoom_amount, 0.2);
        assert_eq!(c.axis_index(Axis::RightY), 4);
    }

    #[test]
    fn unknown_command_is_reported() {
        let err = Config::parse("[buttons]\n1 = \"plya\"\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("unknown command \"plya\""), "{err}");
    }

    #[test]
    fn typo_in_field_name_is_reported() {
        let err = Config::parse("[sticks]\ndeadzon = 0.2\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("deadzon"), "{err}");
        let err = Config::parse("[stickz]\n").unwrap_err().to_string();
        assert!(err.contains("stickz"), "{err}");
        let err = Config::parse("[sticks.left]\ninvert_x = true\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("invert_x"), "{err}");
    }

    #[test]
    fn options_from_0_3_point_to_their_new_place() {
        let err = Config::parse(
            "[sticks]\nleft_x = \"scroll_x\"\ndeadzone = 0.2\n\
             [pad]\naxis_right_y = 4\n",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("[sticks] left_x → [sticks.left] x"), "{err}");
        assert!(
            err.contains("[pad] axis_right_y → [sticks.right] axis_y"),
            "{err}"
        );
        assert!(!err.contains("deadzone"), "{err}");
        assert!(!err.contains("also:"), "nothing else is wrong: {err}");
        // Other errors are reported together with the renamed options.
        let err = Config::parse("[pad]\naxis_left_x = 0\nrumbl = true\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("axis_left_x → [sticks.left] axis_x"), "{err}");
        assert!(err.contains("also:") && err.contains("rumbl"), "{err}");
        let err = Config::parse("[sticks]\nleft_x = \"zoom\"\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("also:") && err.contains("\"zoom\""), "{err}");
        // The new name wins over the old one.
        let err = Config::parse("[sticks]\nleft_x = \"zoom\"\n[sticks.left]\nx = \"zoom_x\"\n")
            .unwrap_err()
            .to_string();
        assert!(!err.contains("also:"), "{err}");
        let err = Config::parse("[pad]\nrumbl = true\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("rumbl"), "{err}");
    }

    #[test]
    fn errors_say_what_kind_they_are() {
        assert!(matches!(
            Config::parse("[stickz]\n"),
            Err(ConfigError::Syntax(_))
        ));
        assert!(matches!(
            Config::parse("[sticks]\ndeadzone = 2.0\n"),
            Err(ConfigError::Invalid(_))
        ));
        let missing = Path::new("/nonexistent/reaper-pad.toml");
        let err = Config::load(missing).unwrap_err();
        assert!(matches!(err, LoadError::Read(..)));
        assert!(
            err.to_string().starts_with("cannot read /nonexistent/"),
            "{err}"
        );
    }

    #[test]
    fn invalid_button_numbers_are_reported() {
        assert!(
            Config::parse("[buttons]\n0 = \"play\"\n")
                .unwrap_err()
                .to_string()
                .contains("invalid button number")
        );
        assert!(Config::parse("[buttons]\nA = \"play\"\n").is_err());
        assert!(Config::parse("[buttons]\n33 = \"play\"\n").is_err());
        assert!(Config::parse("[buttons]\n32 = \"play\"\n").is_ok());
    }

    #[test]
    fn invalid_stick_values_are_reported() {
        assert!(Config::parse("[sticks]\ndeadzone = 1.0\n").is_err());
        assert!(
            Config::parse("[sticks]\nmin_steps_per_sec = 50\nmax_steps_per_sec = 10\n").is_err()
        );
        for bad in [
            "min_steps_per_sec = nan",
            "max_steps_per_sec = inf",
            "zoom_amount = inf",
            "deadzone = nan",
        ] {
            let err = Config::parse(&format!("[sticks]\n{bad}\n"))
                .unwrap_err()
                .to_string();
            assert!(err.contains("[sticks]"), "{bad}: {err}");
        }
        for bad in ["speed = -1", "speed = nan", "speed = -inf"] {
            let err = Config::parse(&format!("[sticks.right]\n{bad}\n"))
                .unwrap_err()
                .to_string();
            assert!(err.contains("[sticks.right] speed"), "{bad}: {err}");
        }
        assert!(
            Config::parse("[sticks.left]\nx = \"play\"\n")
                .unwrap_err()
                .to_string()
                .contains("unknown stick command")
        );
    }

    #[test]
    fn y_axes_are_flipped_unless_inverted() {
        let c = Config::parse("[sticks.left]\ninvert_y = true\n").unwrap();
        assert!(!c.axis_params(Axis::LeftY).flip);
        assert!(c.axis_params(Axis::RightY).flip);
        assert!(!c.axis_params(Axis::LeftX).flip);
    }

    #[test]
    fn per_stick_speed() {
        let c = Config::parse("[sticks.left]\nspeed = 2.0\n[sticks.right]\nspeed = 0.3\n").unwrap();
        assert_eq!(c.axis_params(Axis::LeftX).speed, 2.0);
        assert_eq!(c.axis_params(Axis::RightY).speed, 0.3);
    }

    #[test]
    fn stick_settings_fall_back_to_their_side() {
        let c = Config::parse("[sticks.right]\ny = \"zoom_y\"\naxis_x = 2\n").unwrap();
        assert_eq!(c.stick_command(Axis::RightY), Some(StickCommand::ZoomY));
        assert_eq!(c.stick_command(Axis::RightX), None);
        assert_eq!(c.axis_index(Axis::RightX), 2);
        assert_eq!(c.axis_index(Axis::RightY), 4, "right default");
        assert_eq!(c.axis_index(Axis::LeftX), 0, "left default");
        assert_eq!(c.axis_index(Axis::LeftY), 1, "left default");
        assert_eq!(c.axis_params(Axis::RightX).speed, 1.0);
    }

    #[test]
    fn default_config_layout() {
        let c = Config::parse(DEFAULT_CONFIG).unwrap();
        assert_eq!(c.stick_command(Axis::LeftX), Some(StickCommand::ScrollX));
        assert_eq!(c.stick_command(Axis::LeftY), Some(StickCommand::ScrollY));
        assert_eq!(c.stick_command(Axis::RightY), Some(StickCommand::ZoomY));
        assert!(!c.axis_params(Axis::LeftY).flip, "left Y inverted");
        assert_eq!(c.axis_params(Axis::RightX).speed, 0.3);
        assert_eq!(c.axis_index(Axis::RightY), 4);
    }
}
