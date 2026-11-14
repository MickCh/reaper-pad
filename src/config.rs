//! Configuration file (TOML). See `config.toml` in the repository root for
//! the documented default.

use crate::command::{Command, StickCommand};
use crate::logic::AxisParams;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// The default configuration, embedded into the binaries.
pub const DEFAULT_CONFIG: &str = include_str!("../config.toml");

/// Highest button number accepted in `[buttons]`.
pub const MAX_BUTTON: u32 = 32;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(rename = "buttons", default)]
    raw_buttons: HashMap<String, Command>,
    /// Button number (as printed on the pad, starting at 1) → command.
    #[serde(skip)]
    pub buttons: BTreeMap<u32, Command>,
    #[serde(default)]
    pub dpad: Dpad,
    #[serde(default)]
    pub sticks: Sticks,
    #[serde(default)]
    pub pad: PadCfg,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Sticks {
    pub left_x: Option<StickCommand>,
    pub left_y: Option<StickCommand>,
    pub right_x: Option<StickCommand>,
    pub right_y: Option<StickCommand>,
    pub deadzone: f32,
    pub min_steps_per_sec: f32,
    pub max_steps_per_sec: f32,
    pub speed_left: f32,
    pub speed_right: f32,
    pub invert_left_y: bool,
    pub invert_right_y: bool,
    /// Smooth horizontal zoom: REAPER zoom amount per stick step.
    pub zoom_amount: f64,
}

impl Default for Sticks {
    fn default() -> Self {
        Sticks {
            left_x: None,
            left_y: None,
            right_x: None,
            right_y: None,
            deadzone: 0.15,
            min_steps_per_sec: 3.0,
            max_steps_per_sec: 40.0,
            speed_left: 1.0,
            speed_right: 1.0,
            invert_left_y: false,
            invert_right_y: false,
            zoom_amount: 0.2,
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
    pub axis_left_x: u32,
    pub axis_left_y: u32,
    pub axis_right_x: u32,
    pub axis_right_y: u32,
}

impl Default for PadCfg {
    fn default() -> Self {
        PadCfg {
            debounce_ms: 120,
            rumble: true,
            axis_left_x: 0,
            axis_left_y: 1,
            axis_right_x: 3,
            axis_right_y: 4,
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
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut cfg: Config = toml::from_str(text).map_err(|e| e.to_string())?;
        cfg.buttons = std::mem::take(&mut cfg.raw_buttons)
            .into_iter()
            .map(|(k, v)| match k.trim().parse::<u32>() {
                Ok(n) if (1..=MAX_BUTTON).contains(&n) => Ok((n, v)),
                _ => Err(format!(
                    "[buttons]: invalid button number \"{k}\" (expected 1–{MAX_BUTTON})"
                )),
            })
            .collect::<Result<_, _>>()?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("error in {}: {e}", path.display()))
    }

    fn validate(&self) -> Result<(), String> {
        let s = &self.sticks;
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
        if s.speed_left < 0.0 || s.speed_right < 0.0 {
            return Err("[sticks] speed_left/speed_right must not be negative".into());
        }
        Ok(())
    }

    /// Command for a button number as printed on the pad (1-based).
    pub fn button(&self, num: u32) -> Option<&Command> {
        self.buttons.get(&num)
    }

    pub fn stick_command(&self, axis: Axis) -> Option<&StickCommand> {
        let s = &self.sticks;
        match axis {
            Axis::LeftX => s.left_x.as_ref(),
            Axis::LeftY => s.left_y.as_ref(),
            Axis::RightX => s.right_x.as_ref(),
            Axis::RightY => s.right_y.as_ref(),
        }
    }

    /// Hardware axis index for a stick axis.
    pub fn axis_index(&self, axis: Axis) -> u32 {
        let p = &self.pad;
        match axis {
            Axis::LeftX => p.axis_left_x,
            Axis::LeftY => p.axis_left_y,
            Axis::RightX => p.axis_right_x,
            Axis::RightY => p.axis_right_y,
        }
    }

    pub fn axis_params(&self, axis: Axis) -> AxisParams {
        let s = &self.sticks;
        let (speed, flip) = match axis {
            Axis::LeftX => (s.speed_left, false),
            // Gamepads report "up" as negative; flip so that up means up.
            Axis::LeftY => (s.speed_left, !s.invert_left_y),
            Axis::RightX => (s.speed_right, false),
            Axis::RightY => (s.speed_right, !s.invert_right_y),
        };
        AxisParams {
            deadzone: s.deadzone,
            min_steps_per_sec: s.min_steps_per_sec,
            max_steps_per_sec: s.max_steps_per_sec,
            speed,
            flip,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let c = Config::parse(DEFAULT_CONFIG).unwrap();
        assert_eq!(c.button(1), Some(&Command::PlayStop));
        assert_eq!(c.button(2), Some(&Command::Record));
        assert_eq!(c.buttons.len(), 12);
        assert_eq!(c.stick_command(Axis::RightX), Some(&StickCommand::ZoomX));
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
        let err = Config::parse("[buttons]\n1 = \"plya\"\n").unwrap_err();
        assert!(err.contains("unknown command \"plya\""), "{err}");
    }

    #[test]
    fn typo_in_field_name_is_reported() {
        let err = Config::parse("[sticks]\ndeadzon = 0.2\n").unwrap_err();
        assert!(err.contains("deadzon"), "{err}");
        let err = Config::parse("[stickz]\n").unwrap_err();
        assert!(err.contains("stickz"), "{err}");
    }

    #[test]
    fn invalid_button_numbers_are_reported() {
        assert!(
            Config::parse("[buttons]\n0 = \"play\"\n")
                .unwrap_err()
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
        assert!(Config::parse("[sticks]\nspeed_left = -1\n").is_err());
        assert!(
            Config::parse("[sticks]\nleft_x = \"play\"\n")
                .unwrap_err()
                .contains("unknown stick command")
        );
    }

    #[test]
    fn y_axes_are_flipped_unless_inverted() {
        let c = Config::parse("[sticks]\ninvert_left_y = true\n").unwrap();
        assert!(!c.axis_params(Axis::LeftY).flip);
        assert!(c.axis_params(Axis::RightY).flip);
        assert!(!c.axis_params(Axis::LeftX).flip);
    }

    #[test]
    fn per_stick_speed() {
        let c = Config::parse("[sticks]\nspeed_left = 2.0\nspeed_right = 0.3\n").unwrap();
        assert_eq!(c.axis_params(Axis::LeftX).speed, 2.0);
        assert_eq!(c.axis_params(Axis::RightY).speed, 0.3);
    }
}
