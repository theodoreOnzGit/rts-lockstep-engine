// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/ShellHooks.h (the SHELL_SCRIPT_HOOK_* enumeration)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/Scripts.cpp (TheShellHookNames, :84;
//     SignalUIInteraction, :125)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #142 (#140, #139): the shell's script hooks, and when the client may hand them to the script engine.
//!
//! # As upstream does it
//!
//! - **The names.** `ShellHooks.h` numbers the hooks (`SHELL_SCRIPT_HOOK_MAIN_MENU_CAMPAIGN_SELECTED = 0` to
//!   `SHELL_SCRIPT_HOOK_LAN_ENTERED_FROM_GAME`, then `SHELL_SCRIPT_HOOK_TOTAL`), and `Scripts.cpp:84` names them in the
//!   same order ([`THE_SHELL_HOOK_NAMES`]): `"ShellMainMenuExitPushed"`, `"ShellOptionsOpened"`, ... Those strings are the
//!   flag names a map's scripts test (`ScriptEngine::evaluateFlag`, ScriptEngine.cpp:6478), and the names WorldBuilder
//!   offers.
//! - **The call.** The menus call `TheScriptEngine->signalUIInteract(TheShellHookNames[hook])` themselves (MainMenu.cpp)
//!   or through `SignalUIInteraction(hook)` (Scripts.cpp:125; the skirmish options, the options menu, the LAN and online
//!   menus). `signalUIInteract` (ScriptEngine.cpp:7266) is `m_uiInteractions.push_front(hookName)`: logic state, read by
//!   FLAG conditions, cleared at the end of each script update (:5602), saved (:9071), not in the CRC (:8525). The
//!   logic half is `logic::world::script_engine` (`signal_ui_interact`, `evaluate_flag`, the clear, the xfer),
//!   differential-tested by `tests/differential_script_engine.rs`.
//!
//! # The owner's exception (#139, owner decision 2026-11-11)
//!
//! Upstream writes that logic state from the client with no game message, so a hook raised on one machine (the options
//! screen also opens in a game) would make a map's script diverge between peers and in a replay, and the empty CRC would
//! not see it. The port's rule is that the client never changes the simulation. The owner made one named exception:
//! **the client hands the hooks to the script engine only when this machine is the sole player and no replay is being
//! recorded** (a campaign mission, a challenge, a skirmish against the AI only with the recorder off). Everywhere else
//! the hooks are only recorded (`ShellGui::ui_interacts`) and dropped. [`ui_interact_gate`] decides; it reads only the
//! World, as the client does:
//!
//! 1. **The game mode** (`TheGameLogic->getGameMode()`): `GAME_SINGLE_PLAYER`, `GAME_SKIRMISH` or `GAME_SHELL`. Not
//!    `GAME_LAN` or `GAME_INTERNET` (other machines), not `GAME_REPLAY` (a replay plays back the recorded game), and not
//!    `GAME_NONE` (no game: upstream's push there is cleared by the next map's `newMap` before any script reads it, so
//!    withholding it changes nothing).
//! 2. **No network** (`TheNetwork == NULL`, `GameLoopCuts::has_network`).
//! 3. **The recorder** (`TheRecorder`): not playing back (`RECORDERMODETYPE_PLAYBACK`) and no replay file open
//!    (`m_file == NULL`). `RecorderClass::updateRecord` opens one for every new game but `GAME_SHELL`,
//!    `GAME_SINGLE_PLAYER` and `GAME_NONE` (Recorder.cpp:485), so a campaign mission or a challenge is never recorded,
//!    and a skirmish always is: upstream has no switch that turns the recorder off. **So today a skirmish's hooks are
//!    always withheld**; the skirmish case of the owner's list applies only if the recorder is ever off for one.
//! 4. **At most one human** among the World's players (`getPlayerType() == PLAYER_HUMAN` for `0..m_playerCount`).
//!
//! The hooks a frame recorded are delivered in the order they were raised, each by `signalUIInteract`, so the list is
//! the one upstream's immediate calls leave (the differential test compares them). The client's menus run before the
//! next logic frame, as upstream's `GameClient::update` runs before `GameLogic::update`.

use crate::common::recorder::{game_mode, RecorderMode};
use crate::common::game_common::PlayerType;
use crate::logic::modules::plant;
use crate::logic::world::game_logic::World;
use crate::logic::world::script_engine::ScriptEngine;

/// ShellHooks.h's enumeration (an anonymous `enum`, so `Int`), in its order.
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_CAMPAIGN_SELECTED: i32 = 0;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_CAMPAIGN_HIGHLIGHTED: i32 = 1;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_CAMPAIGN_UNHIGHLIGHTED: i32 = 2;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_SELECTED: i32 = 3;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_HIGHLIGHTED: i32 = 4;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_UNHIGHLIGHTED: i32 = 5;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_SELECTED: i32 = 6;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_HIGHLIGHTED: i32 = 7;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_UNHIGHLIGHTED: i32 = 8;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_SELECTED: i32 = 9;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_HIGHLIGHTED: i32 = 10;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_UNHIGHLIGHTED: i32 = 11;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_SELECTED: i32 = 12;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_HIGHLIGHTED: i32 = 13;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_UNHIGHLIGHTED: i32 = 14;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_SELECTED: i32 = 15;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_HIGHLIGHTED: i32 = 16;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_UNHIGHLIGHTED: i32 = 17;
pub const SHELL_SCRIPT_HOOK_GENERALS_ONLINE_LOGIN: i32 = 18;
pub const SHELL_SCRIPT_HOOK_GENERALS_ONLINE_LOGOUT: i32 = 19;
pub const SHELL_SCRIPT_HOOK_GENERALS_ONLINE_ENTERED_FROM_GAME: i32 = 20;
pub const SHELL_SCRIPT_HOOK_OPTIONS_OPENED: i32 = 21;
pub const SHELL_SCRIPT_HOOK_OPTIONS_CLOSED: i32 = 22;
pub const SHELL_SCRIPT_HOOK_SKIRMISH_OPENED: i32 = 23;
pub const SHELL_SCRIPT_HOOK_SKIRMISH_CLOSED: i32 = 24;
pub const SHELL_SCRIPT_HOOK_SKIRMISH_ENTERED_FROM_GAME: i32 = 25;
pub const SHELL_SCRIPT_HOOK_LAN_OPENED: i32 = 26;
pub const SHELL_SCRIPT_HOOK_LAN_CLOSED: i32 = 27;
pub const SHELL_SCRIPT_HOOK_LAN_ENTERED_FROM_GAME: i32 = 28;
/// `SHELL_SCRIPT_HOOK_TOTAL` ("Keep this guy last!").
pub const SHELL_SCRIPT_HOOK_TOTAL: i32 = 29;

/// Every value of the enumeration with its name, in order (for the differential test against ShellHooks.h).
pub const SHELL_SCRIPT_HOOK_VALUES: [(&str, i32); 30] = [
    ("SHELL_SCRIPT_HOOK_MAIN_MENU_CAMPAIGN_SELECTED", SHELL_SCRIPT_HOOK_MAIN_MENU_CAMPAIGN_SELECTED),
    ("SHELL_SCRIPT_HOOK_MAIN_MENU_CAMPAIGN_HIGHLIGHTED", SHELL_SCRIPT_HOOK_MAIN_MENU_CAMPAIGN_HIGHLIGHTED),
    ("SHELL_SCRIPT_HOOK_MAIN_MENU_CAMPAIGN_UNHIGHLIGHTED", SHELL_SCRIPT_HOOK_MAIN_MENU_CAMPAIGN_UNHIGHLIGHTED),
    ("SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_SELECTED", SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_SELECTED),
    ("SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_HIGHLIGHTED", SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_HIGHLIGHTED),
    ("SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_UNHIGHLIGHTED", SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_UNHIGHLIGHTED),
    ("SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_SELECTED", SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_SELECTED),
    ("SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_HIGHLIGHTED", SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_HIGHLIGHTED),
    ("SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_UNHIGHLIGHTED", SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_UNHIGHLIGHTED),
    ("SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_SELECTED", SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_SELECTED),
    ("SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_HIGHLIGHTED", SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_HIGHLIGHTED),
    ("SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_UNHIGHLIGHTED", SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_UNHIGHLIGHTED),
    ("SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_SELECTED", SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_SELECTED),
    ("SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_HIGHLIGHTED", SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_HIGHLIGHTED),
    ("SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_UNHIGHLIGHTED", SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_UNHIGHLIGHTED),
    ("SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_SELECTED", SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_SELECTED),
    ("SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_HIGHLIGHTED", SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_HIGHLIGHTED),
    ("SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_UNHIGHLIGHTED", SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_UNHIGHLIGHTED),
    ("SHELL_SCRIPT_HOOK_GENERALS_ONLINE_LOGIN", SHELL_SCRIPT_HOOK_GENERALS_ONLINE_LOGIN),
    ("SHELL_SCRIPT_HOOK_GENERALS_ONLINE_LOGOUT", SHELL_SCRIPT_HOOK_GENERALS_ONLINE_LOGOUT),
    ("SHELL_SCRIPT_HOOK_GENERALS_ONLINE_ENTERED_FROM_GAME", SHELL_SCRIPT_HOOK_GENERALS_ONLINE_ENTERED_FROM_GAME),
    ("SHELL_SCRIPT_HOOK_OPTIONS_OPENED", SHELL_SCRIPT_HOOK_OPTIONS_OPENED),
    ("SHELL_SCRIPT_HOOK_OPTIONS_CLOSED", SHELL_SCRIPT_HOOK_OPTIONS_CLOSED),
    ("SHELL_SCRIPT_HOOK_SKIRMISH_OPENED", SHELL_SCRIPT_HOOK_SKIRMISH_OPENED),
    ("SHELL_SCRIPT_HOOK_SKIRMISH_CLOSED", SHELL_SCRIPT_HOOK_SKIRMISH_CLOSED),
    ("SHELL_SCRIPT_HOOK_SKIRMISH_ENTERED_FROM_GAME", SHELL_SCRIPT_HOOK_SKIRMISH_ENTERED_FROM_GAME),
    ("SHELL_SCRIPT_HOOK_LAN_OPENED", SHELL_SCRIPT_HOOK_LAN_OPENED),
    ("SHELL_SCRIPT_HOOK_LAN_CLOSED", SHELL_SCRIPT_HOOK_LAN_CLOSED),
    ("SHELL_SCRIPT_HOOK_LAN_ENTERED_FROM_GAME", SHELL_SCRIPT_HOOK_LAN_ENTERED_FROM_GAME),
    ("SHELL_SCRIPT_HOOK_TOTAL", SHELL_SCRIPT_HOOK_TOTAL),
];

/// `TheShellHookNames[]` (Scripts.cpp:84): "These strings must be in the same order as they are in their definitions".
pub const THE_SHELL_HOOK_NAMES: [&str; SHELL_SCRIPT_HOOK_TOTAL as usize] = [
    "ShellMainMenuCampaignPushed",
    "ShellMainMenuCampaignHighlighted",
    "ShellMainMenuCampaignUnhighlighted",
    "ShellMainMenuSkirmishPushed",
    "ShellMainMenuSkirmishHighlighted",
    "ShellMainMenuSkirmishUnhighlighted",
    "ShellMainMenuOptionsPushed",
    "ShellMainMenuOptionsHighlighted",
    "ShellMainMenuOptionsUnhighlighted",
    "ShellMainMenuOnlinePushed",
    "ShellMainMenuOnlineHighlighted",
    "ShellMainMenuOnlineUnhighlighted",
    "ShellMainMenuNetworkPushed",
    "ShellMainMenuNetworkHighlighted",
    "ShellMainMenuNetworkUnhighlighted",
    "ShellMainMenuExitPushed",
    "ShellMainMenuExitHighlighted",
    "ShellMainMenuExitUnhighlighted",
    "ShellGeneralsOnlineLogin",
    "ShellGeneralsOnlineLogout",
    "ShellGeneralsOnlineEnteredFromGame",
    "ShellOptionsOpened",
    "ShellOptionsClosed",
    "ShellSkirmishOpened",
    "ShellSkirmishClosed",
    "ShellSkirmishEnteredFromGame",
    "ShellLANOpened",
    "ShellLANClosed",
    "ShellLANEnteredFromGame",
];

/// `TheShellHookNames[hook]`. `None` outside `0..SHELL_SCRIPT_HOOK_TOTAL`, where upstream reads past the array (undefined);
/// every caller passes one of the constants.
pub fn the_shell_hook_name(hook: i32) -> Option<&'static str> {
    // (planted: 9301 the first two names exchanged; 9302 the next hook's name; 9303 the last hook missing)
    let mut i = usize::try_from(hook).ok().filter(|&i| i < THE_SHELL_HOOK_NAMES.len())?;
    if plant(9301) && i < 2 {
        i ^= 1;
    }
    if plant(9302) && i + 1 < THE_SHELL_HOOK_NAMES.len() {
        i += 1;
    }
    if plant(9303) && i + 1 == THE_SHELL_HOOK_NAMES.len() {
        return None;
    }
    Some(THE_SHELL_HOOK_NAMES[i])
}

/// `SignalUIInteraction(interaction)` (Scripts.cpp:125): `if (TheScriptEngine)
/// TheScriptEngine->signalUIInteract(TheShellHookNames[interaction])`. The client's script engine is the host's
/// `signal_ui_interact` (the shell's records it; the frame's hooks reach the logic only through
/// [`apply_ui_interacts`]); it always exists, so the check is always true.
pub fn signal_ui_interaction<H: crate::client::main_menu::MenuHost + ?Sized>(h: &mut H, interaction: i32) {
    if let Some(name) = the_shell_hook_name(interaction) {
        h.signal_ui_interact(name);
    }
}

/// Why the hooks a frame recorded were not handed to the script engine (the owner's exception does not apply).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Withheld {
    /// The game mode is not a single machine's game: `GAME_LAN`, `GAME_INTERNET`, `GAME_REPLAY` or `GAME_NONE`.
    GameMode(i32),
    /// `TheNetwork` exists.
    Network,
    /// `TheRecorder` is playing a replay back.
    Playback,
    /// `TheRecorder` has a replay file open: this game is being recorded.
    Recording,
    /// More than one of the World's players is human.
    Humans(usize),
}

/// (#139, owner decision 2026-11-11, new: not upstream) Whether this machine is the sole player and no replay is being
/// recorded, so that the client may call `signalUIInteract` itself (see the module's docs for each condition and its
/// upstream source). Reads only the World.
pub fn ui_interact_gate(w: &World) -> Result<(), Withheld> {
    let mode = w.cuts.game_loop.game_mode;
    if !matches!(mode, game_mode::SINGLE_PLAYER | game_mode::SKIRMISH | game_mode::SHELL) {
        return Err(Withheld::GameMode(mode));
    }
    if w.cuts.game_loop.has_network {
        return Err(Withheld::Network);
    }
    if let Some(r) = w.cuts.game_loop.recorder.as_ref() {
        if r.mode == RecorderMode::Playback {
            return Err(Withheld::Playback);
        }
        if r.file.is_some() {
            return Err(Withheld::Recording);
        }
    }
    let humans = w.players.iter().take(w.players.player_count().max(0) as usize).filter(|p| p.player_type == PlayerType::Human).count();
    if humans > 1 {
        return Err(Withheld::Humans(humans));
    }
    Ok(())
}

/// The hooks a frame recorded, each `signalUIInteract(hook)` (ScriptEngine.cpp:7266, `push_front`) in the order the
/// menus raised them, which leaves the list upstream's immediate calls leave. `hooks` is emptied.
pub fn deliver_ui_interacts(se: &mut ScriptEngine, hooks: &mut Vec<&'static str>) {
    // (planted: 9304 delivered last first; 9305 each put at the back; 9306 a hook already listed skipped; 9307 the
    // recorded hooks kept, so the next delivery repeats them)
    if plant(9304) {
        hooks.reverse();
    }
    for &h in hooks.iter() {
        if plant(9305) {
            se.ui_interactions.push(h.as_bytes().to_vec());
        } else if plant(9306) && se.ui_interactions.iter().any(|u| u == h.as_bytes()) {
            continue;
        } else {
            se.signal_ui_interact(h.as_bytes());
        }
    }
    if !plant(9307) {
        hooks.clear();
    }
}

/// (#139, owner decision 2026-11-11) The client's one direct write into the simulation: the hooks the menus recorded
/// this frame go to the World's script engine when [`ui_interact_gate`] allows it, and are dropped otherwise (recorded
/// only). `hooks` is emptied either way. The number delivered, or why none was.
pub fn apply_ui_interacts(w: &mut World, hooks: &mut Vec<&'static str>) -> Result<usize, Withheld> {
    match ui_interact_gate(w) {
        Ok(()) => {
            let n = hooks.len();
            deliver_ui_interacts(&mut w.script_engine, hooks);
            Ok(n)
        }
        Err(e) => {
            hooks.clear();
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::game_common::PlayerType;
    use crate::logic::scripts::{action_type as at, condition_type as ct, Condition, OrCondition, Parameter, Script, ScriptAction, ScriptList};
    use crate::logic::world::game_logic_update::CRC_RECALC;
    use crate::logic::world::gate_scenario::gate_map_world;
    use crate::logic::world::player_ops::player_set_player_type;

    /// The table is ShellHooks.h's order: one name per hook, `SHELL_SCRIPT_HOOK_TOTAL` of them, all different; the
    /// constants are 0 to 29 in order. (The strings themselves are compared with the C++ by
    /// `tests/differential_shell_hooks.rs`.)
    #[test]
    fn the_table_has_one_name_per_hook() {
        for (i, (_, v)) in SHELL_SCRIPT_HOOK_VALUES.iter().enumerate() {
            assert_eq!(*v, i as i32);
        }
        let mut names = THE_SHELL_HOOK_NAMES.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SHELL_SCRIPT_HOOK_TOTAL as usize);
        assert_eq!(the_shell_hook_name(SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_SELECTED), Some("ShellMainMenuExitPushed"));
        assert_eq!(the_shell_hook_name(SHELL_SCRIPT_HOOK_OPTIONS_OPENED), Some("ShellOptionsOpened"));
        assert_eq!(the_shell_hook_name(SHELL_SCRIPT_HOOK_TOTAL), None);
        assert_eq!(the_shell_hook_name(-1), None);
    }

    /// A script of every difficulty: if the flag `hook` is true (a FLAG condition, `evaluateFlag`), set the flag `Fired`.
    fn waits_on(hook: &str) -> Script {
        let mut c = Condition::new_for_read();
        c.condition_type = ct::FLAG;
        c.num_parms = 2;
        let mut p = Parameter::new(0, 0);
        p.string = hook.as_bytes().to_vec();
        c.parms[0] = Some(p);
        c.parms[1] = Some(Parameter::new(0, 1));
        let mut a = ScriptAction::new_for_read();
        a.action_type = at::SET_FLAG;
        a.num_parms = 2;
        let mut p = Parameter::new(0, 0);
        p.string = b"Fired".to_vec();
        a.parms[0] = Some(p);
        a.parms[1] = Some(Parameter::new(0, 1));
        let mut s = Script::new();
        s.script_name = b"WaitsOnTheHook".to_vec();
        s.is_active = true;
        s.easy = true;
        s.normal = true;
        s.hard = true;
        s.conditions = vec![OrCondition { conditions: vec![c] }];
        s.actions = vec![a];
        s
    }

    /// The gate's map with Player1 human and Player2 a computer (unless `two_humans`), in a single-player game, with a
    /// script on side 0 waiting on `ShellOptionsOpened`.
    fn single_player_world(two_humans: bool) -> World {
        let mut w = gate_map_world();
        if w.sides.get_num_sides() == 0 {
            w.sides.add_side(&crate::common::dict::Dict::default());
        }
        w.sides.get_side_info_mut(0).expect("side 0").scripts = Some(ScriptList { groups: Vec::new(), scripts: vec![waits_on("ShellOptionsOpened")] });
        w.script_engine_new_map().unwrap();
        player_set_player_type(&mut w, 1, PlayerType::Human, false).expect("Player1 human");
        player_set_player_type(&mut w, 2, if two_humans { PlayerType::Human } else { PlayerType::Computer }, false).expect("Player2");
        w.cuts.game_loop.game_mode = game_mode::SINGLE_PLAYER;
        w
    }

    fn fired(w: &World) -> Option<bool> {
        w.script_engine.flags.iter().find(|f| f.name == b"Fired").map(|f| f.value)
    }

    /// Runs `frames` logic frames, the options menu's `SHELL_SCRIPT_HOOK_OPTIONS_OPENED` raised before frame 3 (as the
    /// shell records it, then [`apply_ui_interacts`]); the CRCs, what the delivery answered, `Fired` after the frame
    /// the hook was raised for, and the save's bytes at the end.
    fn run(w: &mut World, frames: usize, raise: bool) -> (Vec<u32>, Option<Result<usize, Withheld>>, Option<bool>, Vec<u8>) {
        let (mut crcs, mut applied, mut after) = (Vec::new(), None, None);
        for f in 0..frames {
            if f == 3 && raise {
                let mut hooks = Vec::new();
                hooks.push(the_shell_hook_name(SHELL_SCRIPT_HOOK_OPTIONS_OPENED).expect("a hook"));
                applied = Some(apply_ui_interacts(w, &mut hooks));
                assert!(hooks.is_empty(), "the frame's hooks are used up either way");
            }
            w.logic_update().expect("a frame");
            if f == 3 {
                after = fired(w);
            }
            crcs.push(w.logic_crc(CRC_RECALC, None).expect("the CRC"));
        }
        (crcs, applied, after, w.save_game().expect("the save"))
    }

    /// #142 (a), the owner's exception pinned. Predicted before the run: in a single-player game with one human and no
    /// recorder, `ShellOptionsOpened` raised by the client reaches the script engine (1 hook delivered), the script's
    /// FLAG condition is true in the next frame, and `Fired` is set; the control run without the hook never sets it.
    /// ~~The CRCs then differ from the control's from that frame on (a flag is set), which is the exception's point.~~
    /// **Missed on the first run (2026-11-11, #142):** every CRC equals the control's: the flags are the script
    /// engine's, and `ScriptEngine::crc` is empty (ScriptEngine.cpp:8525), as #139 said. So the CRCs cannot see a hook,
    /// and the save's bytes are the instrument that can: they differ here (`Fired` is saved), which shows that the
    /// equal saves of (b) below are a check that could have failed.
    #[test]
    fn a_single_player_hook_fires_a_map_scripts_flag() {
        let mut control = single_player_world(false);
        let (c_crcs, _, c_fired, c_save) = run(&mut control, 8, false);
        let mut w = single_player_world(false);
        assert_eq!(ui_interact_gate(&w), Ok(()));
        let (crcs, applied, after, save) = run(&mut w, 8, true);
        assert!(save != c_save, "the save sees the fired flag");
        let first = crcs.iter().zip(&c_crcs).position(|(a, b)| a != b);
        eprintln!("applied {applied:?}; Fired after the frame {after:?} (control {c_fired:?}); first CRC difference at frame {first:?}");
        assert_eq!(applied, Some(Ok(1)));
        assert_eq!(after, Some(true));
        assert_ne!(c_fired, Some(true));
        assert!(w.script_engine.ui_interactions.is_empty(), "cleared at the end of the script update");
        assert_eq!(&crcs[..3], &c_crcs[..3], "nothing changes before the hook");
    }

    /// #142 (b). Predicted before the run: with the recorder recording (a skirmish, `startRecording`'s file open), or
    /// with two human players,
    /// or in a LAN game, or with a network, or during a replay's playback, the hook is withheld (the reason given),
    /// the script engine's list stays empty, `Fired` stays unset, and the CRCs of every frame and the save's bytes
    /// equal a run without the hook.
    #[test]
    fn elsewhere_the_hook_leaves_the_world_untouched() {
        let mut control = single_player_world(false);
        control.recorder_init();
        let base = run(&mut control, 8, false);
        let cases: [(&str, fn(&mut World), Withheld); 4] = [
            // (a skirmish against the AI, recorded as upstream records every skirmish: `startRecording` opens the file)
            ("recording", |w| {
                w.cuts.game_loop.game_mode = game_mode::SKIRMISH;
                w.with_recorder(|r, w| r.start_recording(1, game_mode::SKIRMISH, 0, 0, w)).expect("a recorder").expect("recording");
            }, Withheld::Recording),
            ("playback", |w| w.cuts.game_loop.recorder.as_mut().expect("a recorder").mode = RecorderMode::Playback, Withheld::Playback),
            ("lan", |w| w.cuts.game_loop.game_mode = game_mode::LAN, Withheld::GameMode(game_mode::LAN)),
            ("network", |w| w.cuts.game_loop.has_network = true, Withheld::Network),
        ];
        for (name, set, why) in cases {
            let mut w = single_player_world(false);
            w.recorder_init();
            set(&mut w);
            let mut control = single_player_world(false);
            control.recorder_init();
            set(&mut control);
            let base = run(&mut control, 8, false);
            let got = run(&mut w, 8, true);
            eprintln!("{name}: {:?}", got.1);
            assert_eq!(got.1, Some(Err(why)), "{name}");
            assert!(w.script_engine.ui_interactions.is_empty(), "{name}");
            assert_eq!(got.2, base.2, "{name}: Fired");
            assert_eq!(got.0, base.0, "{name}: the CRCs");
            assert!(got.3 == base.3, "{name}: the save's bytes");
        }
        // two humans, no recorder
        let mut w = single_player_world(true);
        let mut control = single_player_world(true);
        let two = run(&mut control, 8, false);
        let got = run(&mut w, 8, true);
        assert_eq!(got.1, Some(Err(Withheld::Humans(2))));
        assert_eq!((got.0, got.2), (two.0, two.2));
        assert!(got.3 == two.3, "two humans: the save's bytes");
        // and the single-player control with a recorder that is not recording lets it through
        let mut w = single_player_world(false);
        w.recorder_init();
        assert_eq!(ui_interact_gate(&w), Ok(()));
        assert!(base.2 != Some(true));
    }
}
