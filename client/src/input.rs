// SPDX-License-Identifier: GPL-3.0-only
//! The player's input, milestone C2d (GitHub #6): pointer events become the translated client's calls, in the order the
//! game's message stream runs its translators (placement, then selection, then commands; `GameClient.cpp`'s translator
//! list). Plain Rust, no Bevy: the application supplies what the view knows (the objects under the pointer or in a
//! dragged box, the ground point under a pixel), and its tests drive the same calls without a window.
//!
//! The client still never changes the simulation: everything here reaches the logic as the game's own command messages
//! on the World's command list ([`WorldClient`]); `rts_lockstep_engine::client::world_host`'s test checks that the questions
//! asked here leave the CRCs alone.

use rts_lockstep_engine::client::command_xlat::{CommandEvaluateType, CommandTranslator, CommandXlatHost, GuiButton};
use rts_lockstep_engine::client::control_bar::{process_command_ui, CbCommandStatus, PressedControl};
use rts_lockstep_engine::client::in_game_ui::InGameUiSelection;
use rts_lockstep_engine::client::place_event::PlaceEventTranslator;
use rts_lockstep_engine::client::selection_info::SelectionInfoHost;
use rts_lockstep_engine::client::selection_xlat::{Disposition, SelectionTranslator};
use rts_lockstep_engine::client::world_host::WorldClient;
use rts_lockstep_engine::common::base_type::{Coord3D, ICoord2D};
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::message_stream::MessageType;
use rts_lockstep_engine::logic::world::game_logic::World;

/// How far, in pixels, the pointer may move between press and release for a click rather than a drag (upstream's
/// `Mouse::isClick` uses the drag tolerance from `Mouse.ini`, 4 by default in the shipped options).
pub const CLICK_TOLERANCE: i32 = 4;

/// The client's state: the World host and InGameUI, with the translators that act on them.
pub struct Controls {
    pub client: WorldClient,
    pub ui: InGameUiSelection,
    pub selection: SelectionTranslator,
    pub commands: CommandTranslator,
    pub place: PlaceEventTranslator,
    /// The last hint the pointer made (for the cursor).
    pub hint: Option<MessageType>,
}

/// A button of the selected unit's command set, with its label (the button's name, until the game's text is loaded).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarButton {
    pub label: String,
    pub button: GuiButton,
}

impl Controls {
    pub fn new(world: World, player: i32) -> Self {
        Controls {
            client: WorldClient::new(world, player),
            ui: InGameUiSelection::new(),
            selection: SelectionTranslator::default(),
            commands: CommandTranslator::default(),
            place: PlaceEventTranslator::default(),
            hint: None,
        }
    }

    /// The World.
    pub fn world(&mut self) -> &mut World {
        &mut self.client.world
    }

    /// Before each logic frame: the command translator sees the group messages sent since (as the message stream passes
    /// them through `CommandTranslator` after `SelectionTranslator`).
    pub fn observe_sent(&mut self) {
        for t in self.client.sent.drain(..) {
            self.commands.observe_message(t);
        }
    }

    /// The left button pressed at `pixel`: a building at the cursor is anchored there. Whether the press was taken.
    pub fn left_press(&mut self, pixel: ICoord2D, ground: Coord3D) -> bool {
        self.client.view.terrain = vec![(pixel, ground)];
        self.place.left_button_down(pixel, &mut self.ui, &mut self.client)
    }

    /// The pointer moved to `pixel`: an anchored building stretches toward it and turns to face it.
    pub fn pointer_moved(&mut self, pixel: ICoord2D, ground: Coord3D) {
        if self.place.mouse_position(pixel, &mut self.ui) {
            let (start, _) = self.ui.placement_points();
            let start_ground = self.client.view.terrain.iter().find(|(p, _)| *p == start).map_or(ground, |(_, g)| *g);
            self.client.view.terrain.push((pixel, ground));
            self.ui.turn_placement(start_ground, ground);
        }
    }

    /// The left button released at `release`, pressed at `press`: `in_region` are the objects under the click or in the
    /// dragged box, `picked` the one the view picks under the pointer (for an order), `ground` the terrain under the
    /// release. Returns whether a placement, a selection or an order took it.
    pub fn left_release(&mut self, press: ICoord2D, release: ICoord2D, in_region: &[ObjectId], picked: Option<ObjectId>, ground: Coord3D) -> bool {
        let frame = self.client.world.frame();
        if self.place.left_click(frame, &mut self.ui, &mut self.client) {
            return true;
        }
        let is_point = (release.x - press.x).abs() <= CLICK_TOLERANCE && (release.y - press.y).abs() <= CLICK_TOLERANCE;
        self.client.sync_modes(&self.ui);
        if self.selection.left_click(in_region, is_point, &self.commands, &mut self.ui, &mut self.client) == Disposition::Destroy {
            return true;
        }
        self.client.sync_modes(&self.ui);
        self.commands.click(false, is_point, &ground, picked, &mut self.ui, &mut self.client)
    }

    /// The pointer rests over `picked` (or the ground at `ground`): the hint of what a click would order.
    pub fn hover(&mut self, picked: Option<ObjectId>, ground: Coord3D) -> Option<MessageType> {
        self.client.sync_modes(&self.ui);
        self.client.hints.clear();
        self.commands.mouse_over(picked, &ground, &mut self.ui, &mut self.client);
        self.hint = self.client.hints.last().map(|(t, _)| *t);
        self.hint
    }

    /// The command set of the first selected object (the control bar's context for one selected unit).
    pub fn bar_buttons(&mut self) -> Vec<BarButton> {
        let Some(o) = self.ui.first_selected_drawable() else { return Vec::new() };
        if !self.client.is_locally_controlled(o) {
            return Vec::new();
        }
        let w = &self.client.world;
        let name = w.object_get_command_set_string(o);
        let Some(set) = w.control_bar_find_command_set(&name) else { return Vec::new() };
        (0..rts_lockstep_engine::logic::command_button::MAX_COMMANDS_PER_SET)
            .filter_map(|i| w.command_set_get_command_button(set, i))
            .filter_map(|b| {
                let label = String::from_utf8_lossy(&w.cuts.command_buttons.get(b)?.name).into_owned();
                Some(BarButton { label, button: rts_lockstep_engine::client::command_world::gui_button(w, b)? })
            })
            .collect()
    }

    /// A control bar button pressed.
    pub fn press(&mut self, button: &GuiButton) -> CbCommandStatus {
        let selected = self.ui.first_selected_drawable();
        let multi = self.ui.select_count() > 1;
        process_command_ui(button, PressedControl::Command, selected, multi, selected.is_none(), &mut self.ui, &mut self.client)
    }

    /// Whether a click on `picked` would order the selection rather than select it (`EVALUATE_ONLY`).
    pub fn would_order(&mut self, picked: ObjectId) -> bool {
        let at = self.client.object_position(picked);
        self.client.sync_modes(&self.ui);
        self.commands.evaluate_context_command(Some(picked), &at, CommandEvaluateType::EvaluateOnly, &mut self.ui, &mut self.client)
            != rts_lockstep_engine::common::message_types::MSG_INVALID
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo_world;
    use rts_lockstep_engine::common::message_types::{MSG_DO_ATTACK_OBJECT, MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT, MSG_DO_MOVETO, MSG_DO_MOVETO_HINT};

    fn pos(c: &mut Controls, o: ObjectId) -> Coord3D {
        c.client.object_position(o)
    }

    /// C2d: the input glue in the demo game, as the application drives it. Predicted, from the translators' own tests:
    /// - a click on the first Runner (in the region and picked) selects it; after the next frame player 1's selection in
    ///   the logic is `[runner]`;
    /// - the hover over the ground at (200, 60) hints a move; a click there sends `MSG_DO_MOVETO`, and after 90 frames the
    ///   Runner is closer to it;
    /// - a drag box over both Soldiers selects them (both newly in the selection, the runner out);
    /// - the hover over an enemy hints an attack after moving; a click on it is an order, not a selection (the enemy is not
    ///   selected) and sends `MSG_DO_ATTACK_OBJECT`;
    /// - the dozer's control bar shows its one button, `Command_BuildDepot`; pressing it starts the Depot's placement, and
    ///   a press then a release on the ground at (150, 200) sends the construct order; a Depot of player 1's stands there a
    ///   frame later.
    #[test]
    fn the_input_glue_follows_the_translators() {
        let (w, o) = demo_world();
        let mut c = Controls::new(w, 1);
        let frame = |c: &mut Controls| {
            c.observe_sent();
            c.world().logic_update().expect("a frame");
        };
        let at = |x: i32, y: i32| ICoord2D { x, y };
        let ground = |x: f32, y: f32| Coord3D { x, y, z: 0.0 };
        let selection = |c: &Controls| c.client.world.players.get_nth(1).and_then(|p| p.current_selection.as_ref()).map(|s| s.object_ids.clone()).unwrap_or_default();

        let r = o.runners[0];
        let rp = pos(&mut c, r);
        assert!(c.left_release(at(10, 10), at(10, 10), &[r], Some(r), rp));
        frame(&mut c);
        assert_eq!(selection(&c), vec![r]);
        assert_eq!(c.hover(None, ground(200.0, 60.0)), Some(MSG_DO_MOVETO_HINT));
        assert!(c.left_release(at(50, 10), at(50, 10), &[], None, ground(200.0, 60.0)));
        assert_eq!(c.client.sent, vec![MSG_DO_MOVETO]);
        let d = |c: &mut Controls| {
            let p = pos(c, r);
            ((p.x - 200.0).powi(2) + (p.y - 60.0).powi(2)).sqrt()
        };
        let before = d(&mut c);
        for _ in 0..90 {
            frame(&mut c);
        }
        let after = d(&mut c);
        eprintln!("C2d glue: the Runner {before} -> {after} from the point");
        assert!(after < before);

        assert!(c.left_release(at(0, 0), at(60, 60), &o.soldiers, None, ground(0.0, 0.0)));
        frame(&mut c);
        let mut sel = selection(&c);
        sel.sort();
        let mut want = o.soldiers.clone();
        want.sort();
        assert_eq!(sel, want);

        let e = o.enemies[0];
        let ep = pos(&mut c, e);
        assert_eq!(c.hover(Some(e), ep), Some(MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT));
        assert!(c.would_order(e));
        assert!(c.left_release(at(90, 90), at(90, 90), &[e], Some(e), ep));
        assert_eq!(c.client.sent, vec![MSG_DO_ATTACK_OBJECT]);
        assert!(!c.ui.is_selected(e));
        frame(&mut c);

        let dz = o.dozer;
        let dp = pos(&mut c, dz);
        assert!(c.left_release(at(20, 20), at(20, 20), &[dz], Some(dz), dp));
        frame(&mut c);
        let bar = c.bar_buttons();
        assert_eq!(bar.iter().map(|b| b.label.as_str()).collect::<Vec<_>>(), vec!["Command_BuildDepot"]);
        assert_eq!(c.press(&bar[0].button), CbCommandStatus::Used);
        assert!(c.ui.pending_place_type().is_some());
        assert!(c.left_press(at(150, 200), ground(150.0, 200.0)));
        assert!(c.left_release(at(150, 200), at(150, 200), &[], None, ground(150.0, 200.0)));
        frame(&mut c);
        let depots: Vec<_> = c.client.world.objects.ids_in_order().into_iter().filter(|&x| c.client.world.find_object(x).is_some_and(|y| y.template().get_name() == b"Depot")).collect();
        assert_eq!(depots.len(), 1, "sent {:?}, messages {:?}", c.client.sent, c.client.ui_messages);
        assert_eq!(c.client.world.get_controlling_player(depots[0]), Some(1));
        assert!(c.client.world.cuts.team_live_errors.is_empty(), "{:?}", c.client.world.cuts.team_live_errors);
    }
}
