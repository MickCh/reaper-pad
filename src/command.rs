//! Commands that can be bound to buttons, the D-pad and sticks.
//!
//! Each command enum is generated together with its names (and REAPER
//! command IDs) by a macro, so every variant has them: a missing one is a
//! compile error, not a panic at run time.

use serde::Deserialize;
use std::fmt;
use std::str::FromStr;

/// Why a command in the configuration was not accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    /// Not a button / D-pad command.
    Unknown(String),
    /// Not a stick command.
    UnknownStick(String),
    /// `action:` followed by something other than a positive number.
    InvalidActionId(String),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::Unknown(s) => write!(
                f,
                "unknown command \"{s}\" (expected one of: {}, action:<id>)",
                Command::NAMES.join(", ")
            ),
            CommandError::UnknownStick(s) => write!(
                f,
                "unknown stick command \"{s}\" (expected one of: {})",
                StickCommand::NAMES.join(", ")
            ),
            CommandError::InvalidActionId(s) => write!(
                f,
                "invalid action ID in \"{s}\" (expected a positive number, e.g. action:40044)"
            ),
        }
    }
}

impl std::error::Error for CommandError {}

/// `Command`: the named commands (variant, name in the config file, REAPER
/// command ID), plus `Action(id)` for any other REAPER action.
macro_rules! commands {
    ($($variant:ident = $name:literal, $id:literal;)*) => {
        /// A command triggered by a button or the D-pad.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
        #[serde(try_from = "String")]
        pub enum Command {
            $($variant,)*
            /// Any REAPER action by its numeric command ID.
            Action(u32),
        }

        impl Command {
            /// The named commands, in the order of the help text.
            #[cfg(test)]
            const ALL_NAMED: &[Command] = &[$(Command::$variant),*];
            const NAMES: &[&str] = &[$($name),*];

            /// The REAPER command ID to run.
            pub fn reaper_action_id(self) -> u32 {
                match self {
                    $(Command::$variant => $id,)*
                    Command::Action(id) => id,
                }
            }

            fn from_name(s: &str) -> Option<Self> {
                match s {
                    $($name => Some(Command::$variant),)*
                    _ => None,
                }
            }
        }

        impl fmt::Display for Command {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    $(Command::$variant => f.write_str($name),)*
                    Command::Action(id) => write!(f, "action:{id}"),
                }
            }
        }
    };
}

commands! {
    PlayStop = "play_stop", 40044; // Transport: Play/stop
    Play = "play", 1007;           // Transport: Play
    Stop = "stop", 1016;           // Transport: Stop
    Record = "record", 1013;       // Transport: Record
    Metronome = "metronome", 40364; // Options: Toggle metronome
    Repeat = "repeat", 1068;       // Transport: Toggle repeat
    Undo = "undo", 40029;          // Edit: Undo
    Redo = "redo", 40030;          // Edit: Redo
}

impl FromStr for Command {
    type Err = CommandError;

    fn from_str(s: &str) -> Result<Self, CommandError> {
        let s = s.trim();
        if let Some(cmd) = Command::from_name(s) {
            return Ok(cmd);
        }
        match s.split_once(':') {
            Some(("action", id)) => match id.trim().parse::<u32>() {
                Ok(n) if n > 0 => Ok(Command::Action(n)),
                _ => Err(CommandError::InvalidActionId(s.into())),
            },
            _ => Err(CommandError::Unknown(s.into())),
        }
    }
}

impl TryFrom<String> for Command {
    type Error = CommandError;
    fn try_from(s: String) -> Result<Self, CommandError> {
        s.parse()
    }
}

/// `StickCommand`: (doc, variant, name in the config file).
macro_rules! stick_commands {
    ($($(#[$doc:meta])* $variant:ident = $name:literal;)*) => {
        /// A continuous command driven by a stick axis.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
        #[serde(try_from = "String")]
        pub enum StickCommand {
            $($(#[$doc])* $variant,)*
        }

        impl StickCommand {
            #[cfg(test)]
            const ALL: &[StickCommand] = &[$(StickCommand::$variant),*];
            const NAMES: &[&str] = &[$($name),*];

            pub fn name(self) -> &'static str {
                match self {
                    $(StickCommand::$variant => $name,)*
                }
            }

            fn from_name(s: &str) -> Option<Self> {
                match s {
                    $($name => Some(StickCommand::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

stick_commands! {
    /// Scroll the arrange view horizontally (time).
    ScrollX = "scroll_x";
    /// Scroll the arrange view vertically (tracks).
    ScrollY = "scroll_y";
    /// Zoom horizontally (time).
    ZoomX = "zoom_x";
    /// Zoom vertically (track height).
    ZoomY = "zoom_y";
}

impl FromStr for StickCommand {
    type Err = CommandError;

    fn from_str(s: &str) -> Result<Self, CommandError> {
        let s = s.trim();
        StickCommand::from_name(s).ok_or_else(|| CommandError::UnknownStick(s.into()))
    }
}

impl TryFrom<String> for StickCommand {
    type Error = CommandError;
    fn try_from(s: String) -> Result<Self, CommandError> {
        s.parse()
    }
}

impl fmt::Display for StickCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_commands_round_trip() {
        assert_eq!(Command::ALL_NAMED.len(), Command::NAMES.len());
        for (&cmd, &name) in Command::ALL_NAMED.iter().zip(Command::NAMES) {
            assert_eq!(name.parse(), Ok(cmd));
            assert_eq!(cmd.to_string(), name);
        }
        assert_eq!(Command::NAMES.len(), 8);
    }

    #[test]
    fn names_and_ids_are_unique() {
        // Duplicate names would be unreachable `match` arms (a warning, an
        // error in CI); duplicate IDs are only caught here.
        for (i, a) in Command::ALL_NAMED.iter().enumerate() {
            for b in &Command::ALL_NAMED[i + 1..] {
                assert_ne!(a.reaper_action_id(), b.reaper_action_id(), "{a} / {b}");
            }
        }
    }

    #[test]
    fn action_commands() {
        assert_eq!("action:40157".parse(), Ok(Command::Action(40157)));
        assert_eq!(" action: 42 ".parse(), Ok(Command::Action(42)));
        assert_eq!(Command::Action(7).reaper_action_id(), 7);
        assert_eq!(Command::Action(7).to_string(), "action:7");
    }

    #[test]
    fn well_known_action_ids() {
        assert_eq!(Command::PlayStop.reaper_action_id(), 40044);
        assert_eq!(Command::Record.reaper_action_id(), 1013);
        assert_eq!(Command::Undo.reaper_action_id(), 40029);
    }

    #[test]
    fn invalid_commands_are_rejected_with_help() {
        let err = "plya".parse::<Command>().unwrap_err();
        assert_eq!(err, CommandError::Unknown("plya".into()));
        let msg = err.to_string();
        assert!(
            msg.contains("unknown command \"plya\"") && msg.contains("play_stop, play,"),
            "{msg}"
        );
        assert_eq!(
            "action:abc".parse::<Command>(),
            Err(CommandError::InvalidActionId("action:abc".into()))
        );
        assert!(
            "action:0"
                .parse::<Command>()
                .unwrap_err()
                .to_string()
                .contains("invalid action ID")
        );
        assert!(matches!(
            "osc:/record".parse::<Command>(),
            Err(CommandError::Unknown(_))
        ));
    }

    #[test]
    fn stick_commands() {
        assert_eq!(StickCommand::ALL.len(), StickCommand::NAMES.len());
        for (&cmd, &name) in StickCommand::ALL.iter().zip(StickCommand::NAMES) {
            assert_eq!(name.parse(), Ok(cmd));
            assert_eq!(cmd.to_string(), name);
        }
        let err = "play".parse::<StickCommand>().unwrap_err();
        assert_eq!(err, CommandError::UnknownStick("play".into()));
        assert!(
            err.to_string()
                .contains("expected one of: scroll_x, scroll_y"),
            "{err}"
        );
    }
}
