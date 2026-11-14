//! Commands that can be bound to buttons, the D-pad and sticks.

use serde::Deserialize;
use std::fmt;
use std::str::FromStr;

const BUTTON_HELP: &str =
    "play_stop, play, stop, record, metronome, repeat, undo, redo, action:<id>";
const STICK_HELP: &str = "scroll_x, scroll_y, zoom_x, zoom_y";

/// A command triggered by a button or the D-pad.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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

impl Command {
    /// The REAPER command ID to run.
    pub fn reaper_action_id(&self) -> u32 {
        match self {
            Command::PlayStop => 40044,  // Transport: Play/stop
            Command::Play => 1007,       // Transport: Play
            Command::Stop => 1016,       // Transport: Stop
            Command::Record => 1013,     // Transport: Record
            Command::Metronome => 40364, // Options: Toggle metronome
            Command::Repeat => 1068,     // Transport: Toggle repeat
            Command::Undo => 40029,      // Edit: Undo
            Command::Redo => 40030,      // Edit: Redo
            Command::Action(id) => *id,
        }
    }
}

impl FromStr for Command {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let s = s.trim();
        Ok(match s {
            "play_stop" => Command::PlayStop,
            "play" => Command::Play,
            "stop" => Command::Stop,
            "record" => Command::Record,
            "metronome" => Command::Metronome,
            "repeat" => Command::Repeat,
            "undo" => Command::Undo,
            "redo" => Command::Redo,
            _ => match s.split_once(':') {
                Some(("action", id)) => Command::Action(parse_action_id(s, id)?),
                _ => {
                    return Err(format!(
                        "unknown command \"{s}\" (expected one of: {BUTTON_HELP})"
                    ));
                }
            },
        })
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
            Command::PlayStop => f.write_str("play_stop"),
            Command::Play => f.write_str("play"),
            Command::Stop => f.write_str("stop"),
            Command::Record => f.write_str("record"),
            Command::Metronome => f.write_str("metronome"),
            Command::Repeat => f.write_str("repeat"),
            Command::Undo => f.write_str("undo"),
            Command::Redo => f.write_str("redo"),
            Command::Action(id) => write!(f, "action:{id}"),
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
        match self {
            StickCommand::ScrollX => "scroll_x",
            StickCommand::ScrollY => "scroll_y",
            StickCommand::ZoomX => "zoom_x",
            StickCommand::ZoomY => "zoom_y",
        }
    }
}

impl FromStr for StickCommand {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let s = s.trim();
        Ok(match s {
            "scroll_x" => StickCommand::ScrollX,
            "scroll_y" => StickCommand::ScrollY,
            "zoom_x" => StickCommand::ZoomX,
            "zoom_y" => StickCommand::ZoomY,
            _ => {
                return Err(format!(
                    "unknown stick command \"{s}\" (expected one of: {STICK_HELP})"
                ));
            }
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
