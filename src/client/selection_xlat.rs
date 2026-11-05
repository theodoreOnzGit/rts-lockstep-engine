// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/SelectionXlat.cpp (currentlyLookingForSelection,
//   areAllSelected, deselectAll, the MSG_MOUSE_LEFT_CLICK case of SelectionTranslator::translateGameMessage)
//   GeneralsMD/Code/GameEngine/Source/GameClient/SelectionInfo.cpp (addDrawableToList)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SelectionTranslator`'s left click (C2a, GitHub #6): a click, or a drag's region, turned into the selection messages the
//! logic reads (`MSG_CREATE_SELECTED_GROUP`, `MSG_REMOVE_FROM_SELECTED_GROUP`, `MSG_DESTROY_SELECTED_GROUP`).
//!
//! - **Picking is the host's:** `TheTacticalView->iterateDrawablesInRegion` needs the renderer's view. The host gives the
//!   drawables in the region, in the view's order; [`add_drawable_to_list`] filters them as upstream's callback does.
//! - **Not translated:** the debug-only modes of the click (hand of god, hurt, debug selection: `_DEBUG`/`_INTERNAL` and
//!   cheat builds only), and the sounds. The double click and the control groups are in `selection_groups`; the raw
//!   button feedback (the drag rectangle's hints) comes with the view (C2d).

use crate::client::in_game_ui::{DrawableId, InGameUiSelection, SelectionUiHost};
use crate::client::command_xlat::{CommandEvaluateType, CommandTranslator, CommandXlatHost};
use crate::client::selection_info::{context_command_for_new_selection, SelectionInfo, SelectionInfoHost};
use crate::common::game_common::Relationship;
use crate::common::game_type::ObjectId;
use crate::common::kind_of::KINDOF_STRUCTURE;
use crate::common::message_stream::MessageType;
use crate::common::message_types::{MSG_CREATE_SELECTED_GROUP, MSG_INVALID, MSG_REMOVE_FROM_SELECTED_GROUP};

/// What the selection translator asks of the game beyond [`SelectionInfoHost`] and [`SelectionUiHost`].
pub trait SelectionXlatHost: SelectionInfoHost + SelectionUiHost {
    /// `TheInGameUI->isQuitMenuVisible()`.
    fn quit_menu_visible(&self) -> bool;
    /// `TheInGameUI->getGUICommand() != NULL` (a command button waiting for its target).
    fn gui_command_pending(&self) -> bool;
    /// `draw->getTemplate()->isAnyKindOf(pds->kindofsToMatch)`, the kinds of `getPickTypesForContext`.
    fn matches_pick_kinds(&self, draw: DrawableId) -> bool;
    /// `draw->isSelectable()`.
    fn is_selectable(&self, draw: DrawableId) -> bool;
    /// `obj->getContainedBy()`'s id.
    fn contained_by(&self, obj: ObjectId) -> Option<ObjectId>;
    /// `container->getContain()->isEnclosingContainerFor(obj)`; `None` when the container has no contain module.
    fn is_enclosing_container_for(&self, container: ObjectId, obj: ObjectId) -> Option<bool>;
    /// `obj->getDrawable()`'s id.
    fn object_drawable(&self, obj: ObjectId) -> Option<DrawableId>;
    /// `obj->getShroudedStatus(localPlayerIndex) >= OBJECTSHROUD_FOGGED`.
    fn fogged_for_local_player(&self, obj: ObjectId) -> bool;
    /// `obj->testStatus(OBJECT_STATUS_STEALTHED) && !obj->testStatus(OBJECT_STATUS_DETECTED)`.
    fn stealthed_and_undetected(&self, obj: ObjectId) -> bool;
    /// `TheMessageStream->appendMessage(t)` with its arguments: a boolean first (`create`), then the object ids.
    fn append_message(&mut self, t: MessageType, create: Option<bool>, objects: &[ObjectId]);
    /// `localPlayer->getAcademyStats()->recordDragSelection()`.
    fn record_drag_selection(&mut self);
    /// `TheInGameUI->clearAttackMoveToMode()`.
    fn clear_attack_move_to_mode(&mut self);
}

/// `GameMessageDisposition`: what the translator does with the message it was given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition {
    /// `KEEP_MESSAGE`: the next translator sees it (the command translator, for a move or an attack).
    Keep,
    /// `DESTROY_MESSAGE`: the click was a selection.
    Destroy,
}

/// `SelectionTranslator`'s state that the left click reads and writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionTranslator {
    /// `m_lastGroupSelGroup` (`-1`: none), which the control groups' double press reads.
    pub last_group_sel_group: i32,
    /// `m_lastGroupSelTime` (the logic frame of the last group press; `0`: none).
    pub last_group_sel_time: u32,
}

impl Default for SelectionTranslator {
    fn default() -> Self {
        SelectionTranslator { last_group_sel_group: -1, last_group_sel_time: 0 }
    }
}

/// `addDrawableToList(draw, pds)`: appends `draw` to `list` when the click may pick it. A drawable that is not selectable
/// but is held by a container that does not enclose it (a fire base's gunners) picks the container instead.
///
/// Upstream's fall-through is kept: an unselectable drawable whose object is contained, but enclosed (or whose container has
/// no drawable), is **not** refused here; the click drops it later, because a contained object is never selected.
pub fn add_drawable_to_list(draw: DrawableId, list: &mut Vec<DrawableId>, h: &impl SelectionXlatHost) -> bool {
    if !h.matches_pick_kinds(draw) {
        return false;
    }
    if !h.is_selectable(draw) {
        let obj = h.drawable_object(draw);
        match obj.and_then(|o| h.contained_by(o).map(|c| (o, c))) {
            Some((o, container)) => {
                let contain_draw = h.object_drawable(container);
                if let (Some(false), Some(cd)) = (h.is_enclosing_container_for(container, o), contain_draw) {
                    return add_drawable_to_list(cd, list, h);
                }
            }
            None => return false,
        }
    }
    if let Some(obj) = h.drawable_object(draw) {
        let rel = h.relationship_to_local_player(obj);
        if matches!(rel, Relationship::Neutral | Relationship::Enemies)
            && (h.fogged_for_local_player(obj) || h.stealthed_and_undetected(obj))
        {
            return false;
        }
    }
    list.push(draw);
    true
}

impl SelectionTranslator {
    /// The `MSG_MOUSE_LEFT_CLICK` case: `in_region` are the drawables the view finds in the click's pixel region (a point
    /// when `is_point`), before [`add_drawable_to_list`]'s filter.
    pub fn left_click<H: CommandXlatHost>(
        &mut self,
        in_region: &[DrawableId],
        is_point: bool,
        cmd: &CommandTranslator,
        ui: &mut InGameUiSelection,
        h: &mut H,
    ) -> Disposition {
        if h.quit_menu_visible() {
            return Disposition::Destroy;
        }
        let mut will_select = Vec::new();
        for &d in in_region {
            add_drawable_to_list(d, &mut will_select, h);
        }
        if will_select.is_empty() {
            return Disposition::Keep;
        }
        let current = ui.all_selected_drawables().to_vec();
        if h.gui_command_pending() {
            return Disposition::Keep;
        }
        let mut si = SelectionInfo::default();
        let shown: &mut InGameUiSelection = ui;
        let mut valid = |h: &mut H, d| {
            let pos = h.drawable_object(d).map(|o| h.object_position(o)).unwrap_or_default();
            cmd.evaluate_context_command(Some(d), &pos, CommandEvaluateType::EvaluateOnly, shown, h) != MSG_INVALID
        };
        if context_command_for_new_selection(&current, &will_select, &mut si, is_point, h, &mut valid) {
            return Disposition::Keep;
        }
        let mut add_to_group = ui.is_in_prefer_selection_mode();
        if si.current_count_enemies > 0
            || si.current_count_civilians > 0
            || si.current_count_friends > 0
            || si.current_count_mine_buildings > 0
        {
            add_to_group = false;
        }
        if si.new_count_mine > 0 {
            si.select_mine = true;
            if si.new_count_mine_buildings == 1 && si.new_count_mine == 1 {
                add_to_group = false;
                si.select_mine_buildings = true;
            } else if si.new_count_mine_buildings > 0 {
                // a drag over a building whose other drawables cannot be selected anyway gets the building
                let mut only_the_one_building = true;
                let mut building = None;
                for &d in &will_select {
                    if h.drawable_object(d).is_some_and(|o| h.is_kind_of(o, KINDOF_STRUCTURE)) {
                        match building {
                            None => building = Some(d),
                            Some(b) if b != d => only_the_one_building = false,
                            _ => {}
                        }
                    } else if h.is_selectable(d) {
                        only_the_one_building = false;
                    }
                    if !only_the_one_building {
                        break;
                    }
                }
                if only_the_one_building {
                    add_to_group = false;
                    si.select_mine_buildings = true;
                }
            }
        } else if si.new_count_enemies > 0 && si.new_count_civilians > 0 && si.new_count_friends > 0 {
            return Disposition::Keep;
        } else if si.new_count_enemies == 1 {
            add_to_group = false;
            si.select_enemies = true;
        } else if si.new_count_civilians == 1 {
            add_to_group = false;
            si.select_civilians = true;
        } else if si.new_count_friends == 1 {
            add_to_group = false;
            si.select_friends = true;
        }
        if !(si.select_mine || si.select_enemies || si.select_civilians || si.select_friends) {
            return Disposition::Keep;
        }
        self.last_group_sel_group = -1;
        if ui.is_in_prefer_selection_mode() && is_point && will_select.iter().all(|&d| ui.is_selected(d)) {
            // shift and a point on what is already selected: deselect it
            let mut ids = Vec::new();
            for &d in &will_select {
                let Some(obj) = h.drawable_object(d) else { continue };
                ids.push(obj);
                ui.deselect_drawable(d, h);
            }
            h.append_message(MSG_REMOVE_FROM_SELECTED_GROUP, None, &ids);
        } else {
            if !add_to_group {
                ui.deselect_all_drawables(true, h);
            }
            let mut ids = Vec::new();
            for &d in &will_select {
                let Some(obj) = h.drawable_object(d) else { continue };
                if h.contained_by(obj).is_some() {
                    // contained, and so not selectable
                    continue;
                }
                let take = if si.select_mine && h.is_locally_controlled(obj) {
                    !h.is_kind_of(obj, KINDOF_STRUCTURE) || si.select_mine_buildings
                } else {
                    match h.relationship_to_local_player(obj) {
                        Relationship::Enemies => si.select_enemies,
                        Relationship::Neutral => si.select_civilians,
                        Relationship::Allies => si.select_friends,
                    }
                };
                if take {
                    ids.push(obj);
                    ui.select_drawable(d, h);
                }
            }
            h.append_message(MSG_CREATE_SELECTED_GROUP, Some(!add_to_group), &ids);
            if ids.len() > 1 {
                h.record_drag_selection();
            }
        }
        h.clear_attack_move_to_mode();
        Disposition::Destroy
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::common::kind_of::KindOfType;
    use crate::common::message_types::MSG_DESTROY_SELECTED_GROUP;
    use std::collections::BTreeMap;

    /// A view's facts: drawable `d`'s object is `d`. `mine` are locally controlled; the others' relationships are `rel`.
    #[derive(Default)]
    pub(crate) struct Host {
        pub mine: Vec<ObjectId>,
        pub rel: BTreeMap<ObjectId, Relationship>,
        pub structures: Vec<ObjectId>,
        pub fogged: Vec<ObjectId>,
        pub messages: Vec<(MessageType, Option<bool>, Vec<ObjectId>)>,
        pub drags: u32,
        pub frame: u32,
        pub all: Vec<DrawableId>,
        pub screen: Vec<DrawableId>,
        pub picked: Option<DrawableId>,
        pub templates: BTreeMap<ObjectId, u32>,
        pub squads: BTreeMap<i32, Vec<ObjectId>>,
        pub looks: Vec<ObjectId>,
        pub ui_messages: Vec<Vec<u8>>,
        /// The command translator's facts (C2b).
        pub cmd: crate::client::command_xlat::tests::CmdFacts,
        /// The control bar's facts (C2c).
        pub bar: crate::client::control_bar::tests::BarFacts,
    }

    impl SelectionInfoHost for Host {
        fn drawable_object(&self, draw: DrawableId) -> Option<ObjectId> {
            Some(draw)
        }
        fn is_locally_controlled(&self, obj: ObjectId) -> bool {
            self.mine.contains(&obj)
        }
        fn is_kind_of(&self, obj: ObjectId, k: KindOfType) -> bool {
            k == KINDOF_STRUCTURE && self.structures.contains(&obj)
        }
        fn relationship_to_local_player(&self, obj: ObjectId) -> Relationship {
            if self.mine.contains(&obj) {
                return Relationship::Allies;
            }
            self.rel.get(&obj).copied().unwrap_or(Relationship::Neutral)
        }
        fn can_local_player_garrison(&self, _obj: ObjectId) -> bool {
            false
        }
        fn force_fire_or_move(&self) -> bool {
            false
        }
        fn prefer_selection(&self) -> bool {
            false
        }
        fn use_alternate_mouse(&self) -> bool {
            false
        }
    }

    impl SelectionUiHost for Host {
        fn logic_frame(&self) -> u32 {
            self.frame
        }
        fn append_message_bool(&mut self, t: MessageType, b: bool) {
            self.messages.push((t, Some(b), Vec::new()));
        }
        fn on_selection_changed(&mut self, _draw: DrawableId, _selected: bool) {}
    }

    impl SelectionXlatHost for Host {
        fn quit_menu_visible(&self) -> bool {
            false
        }
        fn gui_command_pending(&self) -> bool {
            false
        }
        fn matches_pick_kinds(&self, _draw: DrawableId) -> bool {
            true
        }
        fn is_selectable(&self, _draw: DrawableId) -> bool {
            true
        }
        fn contained_by(&self, _obj: ObjectId) -> Option<ObjectId> {
            None
        }
        fn is_enclosing_container_for(&self, _container: ObjectId, _obj: ObjectId) -> Option<bool> {
            None
        }
        fn object_drawable(&self, obj: ObjectId) -> Option<DrawableId> {
            Some(obj)
        }
        fn fogged_for_local_player(&self, obj: ObjectId) -> bool {
            self.fogged.contains(&obj)
        }
        fn stealthed_and_undetected(&self, _obj: ObjectId) -> bool {
            false
        }
        fn append_message(&mut self, t: MessageType, create: Option<bool>, objects: &[ObjectId]) {
            self.messages.push((t, create, objects.to_vec()));
        }
        fn record_drag_selection(&mut self) {
            self.drags += 1;
        }
        fn clear_attack_move_to_mode(&mut self) {}
    }

    /// C2a: the left click. Predicted from SelectionXlat.cpp, with my units 1, 2, my building 5, an enemy 9 and a fogged
    /// enemy 8:
    /// - a click on unit 1: deselect all (`MSG_DESTROY_SELECTED_GROUP(true)`), then `MSG_CREATE_SELECTED_GROUP(true, 1)`;
    /// - a drag over 1, 2 and the building 5: the units only (the building is not the only one), a new group `(true, 1, 2)`
    ///   and one drag selection recorded;
    /// - with shift (prefer selection) a click on unit 1, already selected: `MSG_REMOVE_FROM_SELECTED_GROUP(1)`;
    /// - a click on the fogged enemy picks nothing: the message is kept (the command translator's);
    /// - a click on the enemy 9 with nothing of mine selected: it is selected `(true, 9)`.
    #[test]
    fn a_click_or_a_drag_makes_the_selection_messages() {
        let mut h = Host { mine: vec![1, 2, 5], structures: vec![5], fogged: vec![8], ..Default::default() };
        h.rel.insert(9, Relationship::Enemies);
        h.rel.insert(8, Relationship::Enemies);
        let (mut x, mut ui) = (SelectionTranslator::default(), InGameUiSelection::new());
        assert_eq!(x.left_click(&[1], true, &CommandTranslator::default(), &mut ui, &mut h), Disposition::Destroy);
        assert_eq!(
            h.messages,
            vec![(MSG_DESTROY_SELECTED_GROUP, Some(true), vec![]), (MSG_CREATE_SELECTED_GROUP, Some(true), vec![1])]
        );
        h.messages.clear();
        assert_eq!(x.left_click(&[1, 2, 5], false, &CommandTranslator::default(), &mut ui, &mut h), Disposition::Destroy);
        assert_eq!(h.messages.last(), Some(&(MSG_CREATE_SELECTED_GROUP, Some(true), vec![1, 2])));
        assert_eq!((ui.all_selected_drawables(), h.drags), (&[2, 1][..], 1));
        h.messages.clear();
        ui.set_prefer_selection_mode(true);
        assert_eq!(x.left_click(&[1], true, &CommandTranslator::default(), &mut ui, &mut h), Disposition::Destroy);
        assert_eq!(h.messages, vec![(MSG_REMOVE_FROM_SELECTED_GROUP, None, vec![1])]);
        assert_eq!(ui.all_selected_drawables(), &[2][..]);
        ui.set_prefer_selection_mode(false);
        ui.deselect_all_drawables(false, &mut h);
        h.messages.clear();
        assert_eq!(x.left_click(&[8], true, &CommandTranslator::default(), &mut ui, &mut h), Disposition::Keep);
        assert!(h.messages.is_empty());
        assert_eq!(x.left_click(&[9], true, &CommandTranslator::default(), &mut ui, &mut h), Disposition::Destroy);
        assert_eq!(h.messages.last(), Some(&(MSG_CREATE_SELECTED_GROUP, Some(true), vec![9])));
    }
}
