//! Commands that can be bound to buttons, the D-pad and sticks.

use serde::Deserialize;
use std::fmt;
use std::str::FromStr;

/// A command triggered by a button or the D-pad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub enum Command {
    PlayStop,
    Play,
    Stop,
    Record,
    Metronome,
    Repeat,
    Undo,
    Redo,
    /// Any REAPER action by its numeric command ID.
    Action(u32),
}

/// The named commands: name in the config file, command, REAPER command ID.
/// Everything about a named command is defined here, and only here.
const NAMED: &[(&str, Command, u32)] = &[
    ("play_stop", Command::PlayStop, 40044), // Transport: Play/stop
    ("play", Command::Play, 1007),           // Transport: Play
    ("stop", Command::Stop, 1016),           // Transport: Stop
    ("record", Command::Record, 1013),       // Transport: Record
    ("metronome", Command::Metronome, 40364), // Options: Toggle metronome
    ("repeat", Command::Repeat, 1068),       // Transport: Toggle repeat
    ("undo", Command::Undo, 40029),          // Edit: Undo
    ("redo", Command::Redo, 40030),          // Edit: Redo
];

/// The stick commands and their names in the config file.
const STICK: &[(&str, StickCommand)] = &[
    ("scroll_x", StickCommand::ScrollX),
    ("scroll_y", StickCommand::ScrollY),
    ("zoom_x", StickCommand::ZoomX),
    ("zoom_y", StickCommand::ZoomY),
];

/// "a, b, c" from the names in a table.
fn names<T>(table: &[(&str, T)]) -> String {
    table.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
}

impl Command {
    /// Name and REAPER command ID of a named command; `None` for `Action`.
    fn named(self) -> Option<(&'static str, u32)> {
        NAMED
            .iter()
            .find(|(_, c, _)| *c == self)
            .map(|(n, _, id)| (*n, *id))
    }

    /// The REAPER command ID to run.
    pub fn reaper_action_id(self) -> u32 {
        match self {
            Command::Action(id) => id,
            named => named.named().expect("named command missing from NAMED").1,
        }
    }
}

impl FromStr for Command {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let s = s.trim();
        if let Some((_, cmd, _)) = NAMED.iter().find(|(n, _, _)| *n == s) {
            return Ok(*cmd);
        }
        match s.split_once(':') {
            Some(("action", id)) => Ok(Command::Action(parse_action_id(s, id)?)),
            _ => {
                let help: Vec<_> = NAMED.iter().map(|(n, _, _)| *n).collect();
                Err(format!(
                    "unknown command \"{s}\" (expected one of: {}, action:<id>)",
                    help.join(", ")
                ))
            }
        }
    }
}

impl TryFrom<String> for Command {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        s.parse()
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Command::Action(id) => write!(f, "action:{id}"),
            named => f.write_str(named.named().expect("named command missing from NAMED").0),
        }
    }
}

/// A continuous command driven by a stick axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub enum StickCommand {
    /// Scroll the arrange view horizontally (time).
    ScrollX,
    /// Scroll the arrange view vertically (tracks).
    ScrollY,
    /// Zoom horizontally (time).
    ZoomX,
    /// Zoom vertically (track height).
    ZoomY,
}

impl StickCommand {
    pub fn name(self) -> &'static str {
        STICK
            .iter()
            .find(|(_, c)| *c == self)
            .map(|(n, _)| *n)
            .expect("stick command missing from STICK")
    }
}

impl FromStr for StickCommand {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let s = s.trim();
        STICK
            .iter()
            .find(|(n, _)| *n == s)
            .map(|(_, c)| *c)
            .ok_or_else(|| {
                format!(
                    "unknown stick command \"{s}\" (expected one of: {})",
                    names(STICK)
                )
            })
    }
}

impl TryFrom<String> for StickCommand {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        s.parse()
    }
}

impl fmt::Display for StickCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

fn parse_action_id(full: &str, id: &str) -> Result<u32, String> {
    match id.trim().parse::<u32>() {
        Ok(0) | Err(_) => Err(format!(
            "invalid action ID in \"{full}\" (expected a positive number, e.g. action:40044)"
        )),
        Ok(n) => Ok(n),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_commands_round_trip() {
        for name in [
            "play_stop",
            "play",
            "stop",
            "record",
            "metronome",
            "repeat",
            "undo",
            "redo",
        ] {
            let cmd: Command = name.parse().unwrap();
            assert_eq!(cmd.to_string(), name);
        }
    }

    /// Every variant has to be in the tables, or `named()`/`name()` panic.
    /// Adding a variant without a table row breaks the `match`es below.
    #[test]
    fn tables_cover_every_variant() {
        for cmd in [
            Command::PlayStop,
            Command::Play,
            Command::Stop,
            Command::Record,
            Command::Metronome,
            Command::Repeat,
            Command::Undo,
            Command::Redo,
        ] {
            match cmd {
                Command::PlayStop
                | Command::Play
                | Command::Stop
                | Command::Record
                | Command::Metronome
                | Command::Repeat
                | Command::Undo
                | Command::Redo
                | Command::Action(_) => {}
            }
            assert!(cmd.named().is_some(), "{cmd:?} missing from NAMED");
        }
        assert_eq!(NAMED.len(), 8, "a NAMED row without a test above");
        for cmd in [
            StickCommand::ScrollX,
            StickCommand::ScrollY,
            StickCommand::ZoomX,
            StickCommand::ZoomY,
        ] {
            match cmd {
                StickCommand::ScrollX
                | StickCommand::ScrollY
                | StickCommand::ZoomX
                | StickCommand::ZoomY => {}
            }
            assert_eq!(cmd.name().parse::<StickCommand>(), Ok(cmd));
        }
        assert_eq!(STICK.len(), 4);
    }

    #[test]
    fn table_names_and_ids_are_unique() {
        for (i, (name, cmd, id)) in NAMED.iter().enumerate() {
            for (n, c, d) in &NAMED[i + 1..] {
                assert!(name != n && cmd != c && id != d, "duplicate row: {name}");
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
        assert!(
            err.contains("unknown command \"plya\"") && err.contains("play_stop"),
            "{err}"
        );
        assert!(
            "action:abc"
                .parse::<Command>()
                .unwrap_err()
                .contains("invalid action ID")
        );
        assert!("action:0".parse::<Command>().is_err());
        assert!("osc:/record".parse::<Command>().is_err());
    }

    #[test]
    fn stick_commands() {
        for name in ["scroll_x", "scroll_y", "zoom_x", "zoom_y"] {
            assert_eq!(name.parse::<StickCommand>().unwrap().to_string(), name);
        }
        assert!(
            "play"
                .parse::<StickCommand>()
                .unwrap_err()
                .contains("unknown stick command")
        );
    }
}
