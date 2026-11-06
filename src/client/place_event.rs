// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/PlaceEventTranslator.cpp (PlaceEventTranslator)
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (displayCantBuildMessage; handleBuildPlacements' angle is
//     `InGameUiSelection::turn_placement`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Building placement (C2c part 2, GitHub #6): with a building at the cursor ([`InGameUiSelection::place_build_available`],
//! from a dozer's construct button), pressing the left button anchors it, dragging turns it (or stretches a wall), and the
//! click builds it: `MSG_DOZER_CONSTRUCT(template, position, angle)`, `MSG_DOZER_CONSTRUCT_LINE(..., end)` for a line build,
//! or `MSG_DO_SPECIAL_POWER_AT_LOCATION` for a construction special (the sneak attack's tunnel), when the build assistant finds
//! the place legal; else the reason's message.
//!
//! Not here: the building drawn at the cursor and its legality tint (the view, C2d), the unit's "can't build there" voice and
//! the "no can do" sound (audio).

use crate::client::command_xlat::{CmdArg, GuiButton};
use crate::client::control_bar::ControlBarHost;
use crate::client::in_game_ui::InGameUiSelection;
use crate::common::base_type::{Coord3D, ICoord2D};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::message_types::*;
use crate::logic::world::build_assistant::{CanMakeType, LegalBuildCode};

/// What placement asks of the game, beyond the control bar's questions.
pub trait PlaceHost: ControlBarHost {
    /// `TheTacticalView->screenToTerrain(pixel, &world)`.
    fn screen_to_terrain(&mut self, pixel: ICoord2D) -> Coord3D;
    /// `TheBuildAssistant->isLineBuildTemplate(build)`.
    fn is_line_build_template(&self, build: u16) -> bool;
    /// `TheBuildAssistant->isLocationLegalToBuild(world, build, angle, USE_QUICK_PATHFIND | TERRAIN_RESTRICTIONS | CLEAR_PATH |
    /// NO_OBJECT_OVERLAP | SHROUD_REVEALED | IGNORE_STEALTHED | FAIL_STEALTHED_WITHOUT_FEEDBACK, builder, NULL)`.
    fn is_location_legal_to_build(&mut self, world: Coord3D, build: u16, angle: f32, builder: Option<ObjectId>) -> LegalBuildCode;
    /// `builder->getProductionUpdateInterface()->getSpecialPowerConstructionCommandButton()`.
    fn special_power_construction_button(&mut self, builder: ObjectId) -> Option<GuiButton>;
}

/// `class PlaceEventTranslator`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaceEventTranslator {
    /// `m_frameOfUpButton` (-1: none yet).
    pub frame_of_up_button: i64,
}

impl Default for PlaceEventTranslator {
    fn default() -> Self {
        PlaceEventTranslator { frame_of_up_button: -1 }
    }
}

/// `PLACEMENT_DRAG_THRESHOLD_DIST`, in pixels away from the anchor.
const PLACEMENT_DRAG_THRESHOLD_DIST: f64 = 5.0;

/// `InGameUI::displayCantBuildMessage(lbc)`.
pub fn display_cant_build_message(lbc: LegalBuildCode, h: &mut impl ControlBarHost) {
    let label: &[u8] = match lbc {
        LegalBuildCode::RestrictedTerrain => b"GUI:CantBuildRestrictedTerrain",
        LegalBuildCode::NotFlatEnough => b"GUI:CantBuildNotFlatEnough",
        LegalBuildCode::ObjectsInTheWay => b"GUI:CantBuildObjectsInTheWay",
        LegalBuildCode::TooCloseToSupplies => b"GUI:CantBuildTooCloseToSupplies",
        LegalBuildCode::NoClearPath => b"GUI:CantBuildNoClearPath",
        LegalBuildCode::Shroud => b"GUI:CantBuildShroud",
        _ => b"GUI:CantBuildThere",
    };
    h.ui_message(label, None);
}

impl PlaceEventTranslator {
    /// `MSG_RAW_MOUSE_LEFT_BUTTON_DOWN` at `pixel`: with a building pending and not yet anchored, anchor it there (or, its
    /// builder gone, end the placement). Whether the message is destroyed.
    pub fn left_button_down(&mut self, pixel: ICoord2D, ui: &mut InGameUiSelection, h: &mut impl PlaceHost) -> bool {
        if ui.pending_place_type().is_none() || ui.is_placement_anchored() {
            return false;
        }
        if !ui.pending_place_source().is_some_and(|o| h.object_exists(o)) {
            ui.place_build_available(None, None, 0.0, h);
            return false;
        }
        ui.set_placement_start(Some(pixel));
        true
    }

    /// `MSG_RAW_MOUSE_POSITION` at `pixel`: an anchored placement dragged 5 pixels or more stretches to it. Whether the message
    /// is destroyed.
    pub fn mouse_position(&mut self, pixel: ICoord2D, ui: &mut InGameUiSelection) -> bool {
        if !ui.is_placement_anchored() {
            return false;
        }
        let (start, _) = ui.placement_points();
        let (x, y) = (f64::from(pixel.x - start.x), f64::from(pixel.y - start.y));
        if petir::Real::sqrt(x * x + y * y) >= PLACEMENT_DRAG_THRESHOLD_DIST {
            ui.set_placement_end(pixel);
            return true;
        }
        false
    }

    /// `MSG_MOUSE_LEFT_CLICK` / `MSG_MOUSE_LEFT_DOUBLE_CLICK`: an anchored placement is built if the builder can make it and
    /// the place is legal. Whether the message is destroyed (which also clears the attack move mode).
    pub fn left_click(&mut self, logic_frame: u32, ui: &mut InGameUiSelection, h: &mut impl PlaceHost) -> bool {
        // (`setRadiusCursorNone()` is the view's)
        let Some(build) = ui.pending_place_type() else { return false };
        if !ui.is_placement_anchored() {
            return false;
        }
        let is_line = h.is_line_build_template(build);
        let angle = ui.placement_angle();
        let (anchor_start, anchor_end) = ui.placement_points();
        let world = h.screen_to_terrain(anchor_start);
        let builder = ui.pending_place_source().filter(|&o| h.object_exists(o));
        let cmt = h.can_make_unit(builder, Some(build));
        if cmt != CanMakeType::Ok {
            let label: &[u8] = match cmt {
                CanMakeType::NoMoney => {
                    h.eva_insufficient_funds();
                    b"GUI:NotEnoughMoneyToBuild"
                }
                CanMakeType::QueueFull => b"GUI:ProductionQueueFull",
                CanMakeType::ParkingPlacesFull => b"GUI:ParkingPlacesFull",
                CanMakeType::MaxedOutForPlayer => b"GUI:UnitMaxedOut",
                _ => {
                    ui.place_build_available(None, None, 0.0, h);
                    return false;
                }
            };
            // (these `break` before the message is destroyed: the placement stays)
            h.ui_message(label, None);
            return false;
        }
        let lbc = h.is_location_legal_to_build(world, build, angle, builder);
        if lbc == LegalBuildCode::Ok {
            if let Some(b) = builder {
                if let Some(button) = h.special_power_construction_button(b) {
                    // (upstream dereferences the button's template)
                    let id = button.special_power.map_or(0, |(id, _)| id as i32);
                    let args = [
                        CmdArg::Int(id),
                        CmdArg::Location(world),
                        CmdArg::Real(angle),
                        CmdArg::Object(INVALID_ID),
                        CmdArg::Int(button.options as i32),
                        CmdArg::Object(b),
                    ];
                    h.append_command(MSG_DO_SPECIAL_POWER_AT_LOCATION, &args);
                    ui.place_build_available(None, None, 0.0, h);
                    self.frame_of_up_button = i64::from(logic_frame);
                    ui.clear_attack_move_to_mode();
                    return true;
                }
            }
            let mut args = vec![CmdArg::Int(i32::from(build)), CmdArg::Location(world), CmdArg::Real(angle)];
            if is_line {
                args.push(CmdArg::Location(h.screen_to_terrain(anchor_end)));
            }
            h.append_command(if is_line { MSG_DOZER_CONSTRUCT_LINE } else { MSG_DOZER_CONSTRUCT }, &args);
            ui.place_build_available(None, None, 0.0, h);
        } else {
            display_cant_build_message(lbc, h);
            ui.set_placement_start(None);
        }
        self.frame_of_up_button = i64::from(logic_frame);
        ui.clear_attack_move_to_mode();
        true
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::client::control_bar::{gui, process_command_ui, PressedControl};
    use crate::client::selection_xlat::tests::Host;

    /// Placement's answers, on the selection test host.
    #[derive(Default)]
    pub(crate) struct PlaceFacts {
        lines: Vec<u16>,
        illegal: bool,
        construction: Option<GuiButton>,
    }

    impl PlaceHost for Host {
        fn screen_to_terrain(&mut self, pixel: ICoord2D) -> Coord3D {
            Coord3D { x: pixel.x as f32 * 2.0, y: pixel.y as f32 * 2.0, z: 0.0 }
        }
        fn is_line_build_template(&self, build: u16) -> bool {
            self.place.lines.contains(&build)
        }
        fn is_location_legal_to_build(&mut self, _world: Coord3D, _build: u16, _angle: f32, _builder: Option<ObjectId>) -> LegalBuildCode {
            if self.place.illegal {
                LegalBuildCode::ObjectsInTheWay
            } else {
                LegalBuildCode::Ok
            }
        }
        fn special_power_construction_button(&mut self, _builder: ObjectId) -> Option<GuiButton> {
            self.place.construction.clone()
        }
    }

    /// C2c part 2: placement. Predicted from PlaceEventTranslator.cpp and InGameUI.cpp, with my dozer 1 shown and the view
    /// mapping a pixel (x, y) to the ground (2x, 2y):
    /// - the dozer's construct button for template 20 starts the placement at the template's view angle (0.5 here);
    /// - a click without an anchor is not the placement's; the button down at (10, 10) anchors it and is destroyed;
    /// - moving 3 pixels does not stretch it, 10 pixels does (destroyed), and the icon turns to face the drag: from (20, 20)
    ///   to (40, 20) on the ground is angle 0;
    /// - the click builds: `MSG_DOZER_CONSTRUCT(20, (20, 20), 0)`, and the placement ends;
    /// - a wall (a line build) sends `MSG_DOZER_CONSTRUCT_LINE` with the end (40, 20);
    /// - an illegal place shows `GUI:CantBuildObjectsInTheWay`, sends nothing, and releases the anchor but keeps the
    ///   building at the cursor;
    /// - a construction special's button on the builder sends `MSG_DO_SPECIAL_POWER_AT_LOCATION(power, place, angle,
    ///   INVALID_ID, options, builder)` instead.
    #[test]
    fn a_building_is_placed_by_anchor_drag_and_click() {
        let mut h = Host { mine: vec![1], ..Default::default() };
        let mut ui = InGameUiSelection::new();
        let mut x = PlaceEventTranslator::default();
        let construct = GuiButton { command_type: gui::DOZER_CONSTRUCT, thing_template: Some(20), ..Default::default() };
        let start = |ui: &mut InGameUiSelection, h: &mut Host| {
            process_command_ui(&construct, PressedControl::Command, Some(1), false, false, ui, h);
        };
        start(&mut ui, &mut h);
        assert_eq!((ui.pending_place_type(), ui.placement_angle()), (Some(20), 0.5));
        assert!(!x.left_click(3, &mut ui, &mut h), "not anchored yet");
        assert!(x.left_button_down(ICoord2D { x: 10, y: 10 }, &mut ui, &mut h));
        assert!(!x.mouse_position(ICoord2D { x: 13, y: 10 }, &mut ui));
        assert!(x.mouse_position(ICoord2D { x: 20, y: 10 }, &mut ui));
        ui.turn_placement(h.screen_to_terrain(ICoord2D { x: 10, y: 10 }), h.screen_to_terrain(ICoord2D { x: 20, y: 10 }));
        assert_eq!(ui.placement_angle(), 0.0);
        assert!(x.left_click(3, &mut ui, &mut h));
        let ground = Coord3D { x: 20.0, y: 20.0, z: 0.0 };
        assert_eq!(h.cmd.commands.pop(), Some((MSG_DOZER_CONSTRUCT, vec![CmdArg::Int(20), CmdArg::Location(ground), CmdArg::Real(0.0)])));
        assert_eq!((ui.pending_place_type(), x.frame_of_up_button), (None, 3));

        h.place.lines.push(20);
        start(&mut ui, &mut h);
        x.left_button_down(ICoord2D { x: 10, y: 10 }, &mut ui, &mut h);
        x.mouse_position(ICoord2D { x: 20, y: 10 }, &mut ui);
        x.left_click(4, &mut ui, &mut h);
        assert_eq!(h.cmd.commands.pop().map(|(t, a)| (t, a[3])), Some((MSG_DOZER_CONSTRUCT_LINE, CmdArg::Location(Coord3D { x: 40.0, y: 20.0, z: 0.0 }))));

        h.place.lines.clear();
        h.place.illegal = true;
        start(&mut ui, &mut h);
        x.left_button_down(ICoord2D { x: 10, y: 10 }, &mut ui, &mut h);
        assert!(x.left_click(5, &mut ui, &mut h));
        assert!(h.cmd.commands.is_empty());
        assert_eq!(h.ui_messages.pop(), Some(b"GUI:CantBuildObjectsInTheWay".to_vec()));
        assert_eq!((ui.is_placement_anchored(), ui.pending_place_type()), (false, Some(20)));

        h.place.illegal = false;
        h.place.construction = Some(GuiButton { command_type: gui::SPECIAL_POWER_CONSTRUCT, special_power: Some((9, 3)), options: 0x20, ..Default::default() });
        x.left_button_down(ICoord2D { x: 10, y: 10 }, &mut ui, &mut h);
        assert!(x.left_click(6, &mut ui, &mut h));
        let args = vec![CmdArg::Int(9), CmdArg::Location(ground), CmdArg::Real(0.5), CmdArg::Object(INVALID_ID), CmdArg::Int(0x20), CmdArg::Object(1)];
        assert_eq!(h.cmd.commands.pop(), Some((MSG_DO_SPECIAL_POWER_AT_LOCATION, args)));
    }
}
