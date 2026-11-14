//! reaper-pad REAPER extension: use a USB gamepad as a remote control.
//!
//! Installed as `reaper_pad.so` / `reaper_pad.dll` in REAPER's `UserPlugins`
//! folder. Configuration: `<REAPER resource path>/reaper-pad.toml`, created
//! with defaults on first run.
//!
//! Registers these actions (Actions → Show action list, filter "reaper-pad"):
//! - reaper-pad: Reload configuration
//! - reaper-pad: Enable/disable gamepad       (toggle)
//! - reaper-pad: Button test (console)        (toggle)
//!
//! Layout: this file is the REAPER side (entry point, actions, executing
//! commands). [`pad`] reads the gamepad through SDL2; the decisions it makes
//! live in [`logic`], [`command`] and [`config`], which are plain Rust and
//! unit-tested without a gamepad or REAPER.

mod command;
mod config;
mod logic;
mod pad;

use command::{Command, StickCommand};
use config::{Config, DEFAULT_CONFIG};
use logic::{RUMBLE_HELLO, Stepper};
use pad::{Pad, Sink};
use reaper_low::PluginContext;
use reaper_macros::reaper_extension_plugin;
use reaper_medium::{
    CommandId, HookCommand, OwnedGaccelRegister, ProjectContext, Reaper, ReaperSession,
    ToggleAction, ToggleActionResult,
};
use std::cell::RefCell;
use std::error::Error;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::OnceLock;

const CONFIG_NAME: &str = "reaper-pad.toml";

/// Greet with a short rumble after ~2 s of regular timer calls (≈30/s).
/// Counted in ticks, not wall-clock time: while starting up, REAPER may not
/// call the timer for several seconds, and a rumble started then would only
/// stop once the timer runs again.
const HELLO_AFTER_TICKS: u32 = 60;

// ---------------------------------------------------------------------------
// Running commands in REAPER

struct ReaperSink {
    reaper: Reaper,
    stepper: Stepper,
    zoom_amount: f64,
    /// Log to REAPER's console (button test) instead of stderr.
    console: bool,
}

impl Sink for ReaperSink {
    fn command(&mut self, cmd: &Command) {
        self.reaper.main_on_command_ex(
            CommandId::new(cmd.reaper_action_id()),
            0,
            ProjectContext::CurrentProject,
        );
    }

    fn stick(&mut self, cmd: &StickCommand, delta: f32) {
        let low = self.reaper.low();
        if *cmd == StickCommand::ZoomX {
            // Smooth: REAPER accepts fractional zoom amounts.
            low.adjustZoom(delta as f64 * self.zoom_amount, 0, true, -1);
            return;
        }
        // The others only move in whole steps.
        let n = self.stepper.steps(cmd.name(), delta);
        if n != 0 {
            match cmd {
                StickCommand::ScrollX => low.CSurf_OnScroll(n, 0),
                StickCommand::ScrollY => low.CSurf_OnScroll(0, n),
                StickCommand::ZoomY => low.CSurf_OnZoom(0, n),
                StickCommand::ZoomX => unreachable!(),
            }
        }
    }

    fn recording(&mut self) -> Option<bool> {
        Some(
            self.reaper
                .get_play_state_ex(ProjectContext::CurrentProject)
                .is_recording,
        )
    }

    fn log(&mut self, msg: &str) {
        if self.console {
            self.reaper.show_console_msg(format!("{msg}\n"));
        } else {
            // Visible when REAPER is started from a terminal.
            eprintln!("[reaper-pad] {msg}");
        }
    }

    fn notice(&mut self, msg: &str) {
        eprintln!("[reaper-pad] {msg}");
        self.reaper.show_console_msg(format!("reaper-pad: {msg}\n"));
    }
}

/// While disabled: events are consumed but do nothing.
struct NullSink;

impl Sink for NullSink {
    fn command(&mut self, _: &Command) {}
    fn stick(&mut self, _: &StickCommand, _: f32) {}
    fn log(&mut self, msg: &str) {
        eprintln!("[reaper-pad] {msg}");
    }
}

// ---------------------------------------------------------------------------
// Plugin state (REAPER's main thread only)

struct State {
    pad: Pad,
    sink: ReaperSink,
    enabled: bool,
    ticks: u32,
}

thread_local! {
    // The timer and actions are always called on REAPER's main thread.
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    static REAPER: RefCell<Option<Reaper>> = const { RefCell::new(None) };
}

/// Run `f` on the plugin state. Safe against panics (a bug here must not take
/// REAPER down — the pad is disabled instead) and re-entrancy (e.g. a pad
/// button bound to one of this plugin's own actions).
fn with_state<R>(f: impl FnOnce(&mut Option<State>) -> R) -> Option<R> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        STATE.with(|s| s.try_borrow_mut().ok().map(|mut st| f(&mut st)))
    }));
    match result {
        Ok(r) => r,
        Err(_) => {
            let _ = catch_unwind(|| STATE.with(|s| s.try_borrow_mut().map(|mut st| st.take())));
            eprintln!(
                "[reaper-pad] internal error — gamepad disabled (try \"Reload configuration\")"
            );
            None
        }
    }
}

fn reaper() -> Option<Reaper> {
    REAPER.with(|r| r.borrow().clone())
}

fn config_path(reaper: &Reaper) -> PathBuf {
    reaper.get_resource_path(|p| p.join(CONFIG_NAME).into_std_path_buf())
}

/// Load the configuration; write the default one on first run.
fn load_config(reaper: &Reaper) -> Result<Config, String> {
    let path = config_path(reaper);
    if !path.exists() {
        std::fs::write(&path, DEFAULT_CONFIG)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        eprintln!(
            "[reaper-pad] created default configuration: {}",
            path.display()
        );
    }
    Config::load(&path)
}

fn init_state(reaper: &Reaper) -> Result<State, String> {
    let cfg = load_config(reaper)?;
    let zoom_amount = cfg.sticks.zoom_amount;
    let mut pad = Pad::new(cfg)?;
    let mut sink = ReaperSink {
        reaper: reaper.clone(),
        stepper: Stepper::default(),
        zoom_amount,
        console: false,
    };
    pad.open_first(&mut sink);
    Ok(State {
        pad,
        sink,
        enabled: true,
        ticks: 0,
    })
}

extern "C" fn timer() {
    with_state(|s| {
        let Some(st) = s.as_mut() else { return };
        if st.ticks < HELLO_AFTER_TICKS {
            st.ticks += 1;
            if st.ticks == HELLO_AFTER_TICKS {
                st.pad.rumble(RUMBLE_HELLO);
            }
        }
        if st.enabled {
            st.pad.poll(&mut st.sink);
        } else {
            st.pad.poll(&mut NullSink);
        }
    });
}

// ---------------------------------------------------------------------------
// Actions

struct Ids {
    reload: CommandId,
    toggle: CommandId,
    test: CommandId,
}

static IDS: OnceLock<Ids> = OnceLock::new();

fn reload() {
    let Some(reaper) = reaper() else { return };
    let cfg = match load_config(&reaper) {
        Ok(cfg) => cfg,
        Err(e) => {
            reaper.show_console_msg(format!(
                "reaper-pad: {e}\nKeeping the previous configuration.\n"
            ));
            return;
        }
    };
    let mut cfg = Some(cfg);
    let reloaded = with_state(|s| match s.as_mut() {
        Some(st) => {
            let cfg = cfg.take().expect("config");
            st.sink.zoom_amount = cfg.sticks.zoom_amount;
            st.pad.set_config(cfg);
            st.pad.rumble(RUMBLE_HELLO); // confirmation
            true
        }
        None => false,
    });
    if reloaded == Some(false) {
        // Startup failed earlier (e.g. an error in the file) — start from scratch.
        match init_state(&reaper) {
            Ok(mut st) => {
                st.ticks = HELLO_AFTER_TICKS - 1; // rumble on the next tick
                with_state(|s| *s = Some(st));
            }
            Err(e) => {
                reaper.show_console_msg(format!("reaper-pad: {e}\n"));
                return;
            }
        }
    }
    eprintln!(
        "[reaper-pad] configuration loaded: {}",
        config_path(&reaper).display()
    );
}

fn toggle_enabled() {
    with_state(|s| {
        if let Some(st) = s.as_mut() {
            st.enabled = !st.enabled;
            eprintln!(
                "[reaper-pad] gamepad {}",
                if st.enabled { "enabled" } else { "disabled" }
            );
            if st.enabled {
                st.pad.rumble(RUMBLE_HELLO);
            }
        }
    });
}

fn toggle_test() {
    let msg = with_state(|s| {
        let st = s.as_mut()?;
        st.pad.test_mode = !st.pad.test_mode;
        st.sink.console = st.pad.test_mode;
        Some(if st.pad.test_mode {
            let name = st.pad.connected().unwrap_or_else(|| "no gamepad connected".into());
            format!(
                "reaper-pad — BUTTON TEST ({name})\n\
                 Press buttons and move the sticks. The gamepad does not control REAPER meanwhile.\n\
                 Run the same action again to finish.\n"
            )
        } else {
            "reaper-pad — test finished, the gamepad controls REAPER again.\n".into()
        })
    })
    .flatten();
    if let (Some(m), Some(r)) = (msg, reaper()) {
        r.show_console_msg(m);
    }
}

struct Actions;

impl HookCommand for Actions {
    fn call(command_id: CommandId, _flag: i32) -> bool {
        let Some(ids) = IDS.get() else { return false };
        if command_id == ids.reload {
            reload();
        } else if command_id == ids.toggle {
            toggle_enabled();
        } else if command_id == ids.test {
            toggle_test();
        } else {
            return false;
        }
        true
    }
}

impl ToggleAction for Actions {
    fn call(command_id: CommandId) -> ToggleActionResult {
        let Some(ids) = IDS.get() else {
            return ToggleActionResult::NotRelevant;
        };
        let on = if command_id == ids.toggle {
            with_state(|s| s.as_ref().map(|st| st.enabled)).flatten()
        } else if command_id == ids.test {
            with_state(|s| s.as_ref().map(|st| st.pad.test_mode)).flatten()
        } else {
            return ToggleActionResult::NotRelevant;
        };
        if on == Some(true) {
            ToggleActionResult::On
        } else {
            ToggleActionResult::Off
        }
    }
}

fn register_actions(session: &mut ReaperSession) -> Result<(), Box<dyn Error>> {
    let reload = session.plugin_register_add_command_id("REAPER_PAD_RELOAD")?;
    let toggle = session.plugin_register_add_command_id("REAPER_PAD_TOGGLE")?;
    let test = session.plugin_register_add_command_id("REAPER_PAD_TEST")?;
    for (id, desc) in [
        (reload, "reaper-pad: Reload configuration"),
        (toggle, "reaper-pad: Enable/disable gamepad"),
        (test, "reaper-pad: Button test (console)"),
    ] {
        session.plugin_register_add_gaccel(OwnedGaccelRegister::without_key_binding(id, desc))?;
    }
    let _ = IDS.set(Ids {
        reload,
        toggle,
        test,
    });
    session.plugin_register_add_hook_command::<Actions>()?;
    session.plugin_register_add_toggle_action::<Actions>()?;
    Ok(())
}

// ---------------------------------------------------------------------------

#[reaper_extension_plugin]
fn plugin_main(context: PluginContext) -> Result<(), Box<dyn Error>> {
    let mut session = ReaperSession::load(context);
    let reaper = session.reaper().clone();
    REAPER.with(|r| *r.borrow_mut() = Some(reaper.clone()));

    // Always register actions, so "Reload configuration" works even after a config error.
    register_actions(&mut session)?;

    match init_state(&reaper) {
        Ok(st) => STATE.with(|s| *s.borrow_mut() = Some(st)),
        Err(e) => reaper.show_console_msg(format!(
            "reaper-pad: {e}\nGamepad disabled. Fix the file and run \"reaper-pad: Reload configuration\".\n"
        )),
    }

    session.plugin_register_add_timer(timer)?;
    // The session must live as long as REAPER (timer and action registrations).
    Box::leak(Box::new(session));
    Ok(())
}
