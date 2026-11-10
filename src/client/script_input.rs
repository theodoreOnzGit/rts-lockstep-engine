// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptActions.cpp (the client's half of doDisableInput,
//     doEnableInput and the actions that deselect: their calls on TheInGameUI and TheMouse)
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (setInputEnabled, deselectAllDrawables,
//     clearAttackMoveToMode, setWaypointMode, through `crate::client::in_game_ui`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The scripts' calls on the client's input (GitHub #116): what `DISABLE_INPUT`, `ENABLE_INPUT`, the victory and defeat
//! actions and the actions that deselect ask of `TheInGameUI`, `TheMouse`, `TheControlBar` and `TheLookAtTranslator`,
//! made on the client's own InGameUI and mouse from the logic's [`ClientEvent::ScriptInput`] events.
//!
//! # As upstream reads it
//!
//! `ScriptActions::doDisableInput` (release build: always) calls `TheInGameUI->setInputEnabled(false)`,
//! `TheMouse->setVisibility(false)`, `TheInGameUI->deselectAllDrawables()`, `clearAttackMoveToMode()`,
//! `setWaypointMode(FALSE)`, `TheControlBar->deleteBuildTooltipLayout()` and `TheLookAtTranslator->resetModes()`, in
//! that order. `doEnableInput` calls `setInputEnabled(true)` and `setVisibility(true)`. The calls go straight to the
//! client's singletons from the logic's script action; nothing reads an answer back.
//!
//! # Upstream quirks that are kept
//!
//! - `deselectAllDrawables()`'s default `postMsg = TRUE`: it posts `MSG_DESTROY_SELECTED_GROUP` even with nothing
//!   selected. It is a system message, so the command translator's input gate lets it through to the logic.
//! - The order: the input is off before the deselection, so the deselection's message is the only one that reaches the
//!   logic from the action.
//!
//! # Where this differs from upstream
//!
//! - The logic does not call the client: it pushes one [`ScriptInputCall`] per call
//!   (`logic::world::script_actions_host_world`), and the client makes the calls when it takes the frame's events, after
//!   the logic frame rather than inside it. The CRC test in `client/src/input.rs` checks the logic sees no difference.
//! - `deleteBuildTooltipLayout` and `LookAtTranslator::resetModes` are not made here: the port has no build tooltip
//!   window in the game, ~~and no `LookAtTranslator` (the application's camera input stands in for it)~~ **Corrected
//!   2026-11-10 (#119):** and the `LookAtTranslator` (`crate::client::look_at_xlat`) is the client's camera's. They are
//!   handed back to the caller ([`apply_script_input`]'s answer), which resets what it keeps (the client's
//!   `Controls::take_script_input` reports the reset, and its camera calls `resetModes`).
//!
//! # Representation
//!
//! A free function over the client's InGameUI ([`InGameUiSelection`]), its mouse ([`ClientMouse`], kept by
//! [`crate::client::game_pause::ClientPause`]) and the selection's host ([`SelectionUiHost`], the client's
//! [`crate::client::world_host::WorldClient`] in the game). The simulation is never touched except through the
//! deselection's own message, as upstream.

use crate::client::game_pause::ClientMouse;
use crate::client::in_game_ui::{InGameUiSelection, SelectionUiHost};
use crate::logic::client_event::{ClientEvent, ScriptInputCall};

/// Makes one logic event's call on the client's input, if it is a [`ClientEvent::ScriptInput`]. Returns the call when it
/// is the caller's to make (`DeleteBuildTooltipLayout`, `LookAtResetModes`), else `None`.
pub fn apply_script_input(e: &ClientEvent, ui: &mut InGameUiSelection, mouse: &mut ClientMouse, h: &mut impl SelectionUiHost) -> Option<ScriptInputCall> {
    let ClientEvent::ScriptInput(call) = e else { return None };
    match *call {
        ScriptInputCall::SetInputEnabled(on) => ui.set_input_enabled(on),
        ScriptInputCall::MouseSetVisibility(on) => mouse.visible = on,
        ScriptInputCall::DeselectAllDrawables => ui.deselect_all_drawables(true, h),
        ScriptInputCall::ClearAttackMoveToMode => ui.clear_attack_move_to_mode(),
        ScriptInputCall::SetWaypointMode(on) => ui.set_waypoint_mode(on),
        c @ (ScriptInputCall::DeleteBuildTooltipLayout | ScriptInputCall::LookAtResetModes) => return Some(c),
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::message_stream::MessageType;
    use crate::common::message_types::MSG_DESTROY_SELECTED_GROUP;

    #[derive(Default)]
    struct Host {
        messages: Vec<(MessageType, bool)>,
    }
    impl SelectionUiHost for Host {
        fn logic_frame(&self) -> u32 {
            7
        }
        fn append_message_bool(&mut self, t: MessageType, b: bool) {
            self.messages.push((t, b));
        }
        fn on_selection_changed(&mut self, _: crate::client::in_game_ui::DrawableId, _: bool) {}
    }

    /// #116: `doDisableInput`'s seven calls, then `doEnableInput`'s two, on a UI with two drawables selected, a drag
    /// under way, force attack, waypoint and attack move on.
    ///
    /// *Predictions, written before the first run:* after the disable the input is off, the mouse hidden, nothing
    /// selected, not selecting, every mode off (force attack and waypoint by `setInputEnabled`, attack move by
    /// `clearAttackMoveToMode`), one `MSG_DESTROY_SELECTED_GROUP(true)` posted; the tooltip and look-at calls come back
    /// to the caller in order. After the enable the input is on and the mouse shown; the modes stay off; nothing else is
    /// posted. A logic event that is not a script input call is left alone.
    #[test]
    fn the_scripts_input_calls_reach_the_ui_and_the_mouse() {
        let mut ui = InGameUiSelection::new();
        let mut mouse = ClientMouse::default();
        let mut h = Host::default();
        ui.select_drawable(3, &mut h);
        ui.select_drawable(4, &mut h);
        ui.set_selecting(true);
        ui.set_force_attack_mode(true);
        ui.set_waypoint_mode(true);
        ui.set_attack_move_to_mode(true);
        let disable = [
            ScriptInputCall::SetInputEnabled(false),
            ScriptInputCall::MouseSetVisibility(false),
            ScriptInputCall::DeselectAllDrawables,
            ScriptInputCall::ClearAttackMoveToMode,
            ScriptInputCall::SetWaypointMode(false),
            ScriptInputCall::DeleteBuildTooltipLayout,
            ScriptInputCall::LookAtResetModes,
        ];
        let back: Vec<_> = disable.iter().filter_map(|c| apply_script_input(&ClientEvent::ScriptInput(*c), &mut ui, &mut mouse, &mut h)).collect();
        assert_eq!(back, [ScriptInputCall::DeleteBuildTooltipLayout, ScriptInputCall::LookAtResetModes]);
        assert!(!ui.input_enabled() && !mouse.visible && ui.select_count() == 0 && !ui.is_selecting());
        assert!(!ui.is_in_force_attack_mode() && !ui.is_in_waypoint_mode() && !ui.is_in_attack_move_to_mode());
        assert_eq!(h.messages, [(MSG_DESTROY_SELECTED_GROUP, true)]);
        for c in [ScriptInputCall::SetInputEnabled(true), ScriptInputCall::MouseSetVisibility(true)] {
            assert_eq!(apply_script_input(&ClientEvent::ScriptInput(c), &mut ui, &mut mouse, &mut h), None);
        }
        assert!(ui.input_enabled() && mouse.visible && !ui.is_in_force_attack_mode());
        assert_eq!(h.messages.len(), 1);
        assert_eq!(apply_script_input(&ClientEvent::DrawablesOnLevelStart, &mut ui, &mut mouse, &mut h), None);
        assert!(ui.input_enabled());
    }
}
