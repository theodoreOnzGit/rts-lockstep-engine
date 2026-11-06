// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/ControlBarCallback.cpp (LeftHUDInput's
//     GWM_LEFT_DOWN / GWM_RIGHT_DOWN case)
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (getAllSelectedLocalDrawables)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! A click on the radar (GitHub #54): `LeftHUDInput`'s button press, once the radar has turned the pixel into a point of
//! the world (`localPixelToRadar`, `radarToWorld`). With nothing of the local player's selected, or with the button that
//! looks (the right one, or the left in alternate mouse mode), the view looks there; otherwise a pending special power
//! that needs a position is carried out there, a pending attack move attack-moves there, and anything else moves there,
//! with the unit voices of the two orders.
//!
//! **Not here:** the radar itself (its pixels, `isRadarHidden`, `isRadarForced`) and the cursor kept over it; ~~the client
//! has no radar yet, so the caller supplies the world point.~~ **Corrected 2026-11-06 (#35):** the radar is
//! `client::radar`; the caller turns the pixel into the world point with it
//! (`client::radar_world::ClientRadar::local_pixel_to_world`) and passes it here, as `LeftHUDInput` does.

use crate::client::command_xlat::{CmdArg, CommandEvaluateType, CommandTranslator, CommandXlatHost};
use crate::client::control_bar::gui;
use crate::client::control_bar::opt;
use crate::client::in_game_ui::{DrawableId, InGameUiSelection};
use crate::client::unit_voice::pick_and_play_unit_voice_response;
use crate::common::base_type::Coord3D;
use crate::common::message_types::{MSG_DO_ATTACKMOVETO, MSG_DO_MOVETO};

/// What a radar click did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RadarClick {
    /// `TheTacticalView->lookAt(&world)`: the view's to do.
    LookAt(Coord3D),
    /// The click ordered the selection (or carried out the pending special power).
    Ordered,
}

/// `InGameUI::getAllSelectedLocalDrawables()`: the selected drawables whose objects are locally controlled.
pub fn all_selected_local_drawables(ui: &InGameUiSelection, h: &impl CommandXlatHost) -> Vec<DrawableId> {
    ui.all_selected_drawables().iter().copied().filter(|&d| h.drawable_object(d).is_some_and(|o| h.is_locally_controlled(o))).collect()
}

/// `LeftHUDInput`'s `GWM_LEFT_DOWN` / `GWM_RIGHT_DOWN` (`right`), at `world` on the radar.
pub fn left_hud_input_click(world: &Coord3D, right: bool, commands: &CommandTranslator, ui: &mut InGameUiSelection, h: &mut impl CommandXlatHost) -> RadarClick {
    // "No drawables, or a right click automatically means its a look at."
    let local = all_selected_local_drawables(ui, h);
    let alternate = h.use_alternate_mouse();
    if local.is_empty() || (!alternate && right) || (alternate && !right) {
        return RadarClick::LookAt(*world);
    }
    let command = ui.gui_command().cloned();
    let ct = command.as_ref().map_or(gui::NONE, |c| c.command_type);
    if (ct == gui::SPECIAL_POWER || ct == gui::SPECIAL_POWER_FROM_SHORTCUT) && command.as_ref().is_some_and(|c| c.options & opt::NEED_TARGET_POS != 0) {
        // `TheGameClient->evaluateContextCommand(NULL, &world, CommandTranslator::DO_COMMAND)`
        commands.evaluate_context_command(None, world, CommandEvaluateType::DoCommand, ui, h);
    } else if ct == gui::ATTACK_MOVE {
        // "Attack move has changed from a modifier to a command, so it moves up here."
        h.append_command(MSG_DO_ATTACKMOVETO, &[CmdArg::Location(*world)]);
        // "Play the unit voice response"
        let all = ui.all_selected_drawables().to_vec();
        pick_and_play_unit_voice_response(&all, MSG_DO_ATTACKMOVETO, None, ui, h);
    } else {
        h.append_command(MSG_DO_MOVETO, &[CmdArg::Location(*world)]);
        pick_and_play_unit_voice_response(&local, MSG_DO_MOVETO, None, ui, h);
    }
    RadarClick::Ordered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::command_xlat::GuiButton;
    use crate::client::selection_xlat::tests::Host;

    /// #54: the radar click. Predicted from ControlBarCallback.cpp, with the ground point (30, 40) and the selection test
    /// host: with an enemy (9) the only selected object, a left click looks at the point; with my unit 1 selected, a right
    /// click looks, a left click sends `MSG_DO_MOVETO(30, 40)`; with an attack move pending, `MSG_DO_ATTACKMOVETO(30, 40)`
    /// and the command stays pending (the radar does not end it).
    ///
    /// *Results (2026-11-06):* as predicted, first run.
    #[test]
    fn a_radar_click_looks_moves_or_attack_moves() {
        let mut h = Host { mine: vec![1], ..Default::default() };
        let mut ui = InGameUiSelection::new();
        let x = CommandTranslator::default();
        let at = Coord3D { x: 30.0, y: 40.0, z: 0.0 };
        ui.select_drawable(9, &mut h);
        assert_eq!(left_hud_input_click(&at, false, &x, &mut ui, &mut h), RadarClick::LookAt(at));
        ui.deselect_all_drawables(false, &mut h);
        ui.select_drawable(1, &mut h);
        assert_eq!(left_hud_input_click(&at, true, &x, &mut ui, &mut h), RadarClick::LookAt(at));
        assert!(h.cmd.commands.is_empty());
        assert_eq!(left_hud_input_click(&at, false, &x, &mut ui, &mut h), RadarClick::Ordered);
        assert_eq!(h.cmd.commands.pop(), Some((MSG_DO_MOVETO, vec![CmdArg::Location(at)])));
        let attack_move = GuiButton { index: 2, command_type: gui::ATTACK_MOVE, options: opt::NEED_TARGET_POS, ..Default::default() };
        ui.set_gui_command(Some(attack_move.clone()));
        assert_eq!(left_hud_input_click(&at, false, &x, &mut ui, &mut h), RadarClick::Ordered);
        assert_eq!(h.cmd.commands.pop(), Some((MSG_DO_ATTACKMOVETO, vec![CmdArg::Location(at)])));
        assert_eq!(ui.gui_command(), Some(&attack_move));
    }
}
