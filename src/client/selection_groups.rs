// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/SelectionXlat.cpp (selectSingleDrawableWithoutSound, the
//   MSG_MOUSE_LEFT_DOUBLE_CLICK, MSG_META_BEGIN/END_FORCEATTACK and MSG_META_CREATE/SELECT/ADD/VIEW_TEAMn cases)
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (similarUnitSelection, selectMatchingAcrossRegion,
//   selectMatchingAcrossScreen, selectMatchingAcrossMap)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The rest of C2a (GitHub #6): the double click (select every unit like the one clicked, on the screen or, with alt, across
//! the map) and the control groups (ctrl+n makes group n, n selects it, shift+n adds it, a double press looks at it).
//!
//! **One difference from upstream:** `selectMatchingAcrossRegion` keeps the selected templates in a
//! `std::set<const ThingTemplate*>`, ordered by **pointer**, so the order in which the templates are matched (and so the order
//! of the ids in its `MSG_CREATE_SELECTED_GROUP_NO_SOUND`) depends on where the templates were allocated. Here they are
//! ordered by [`SelectionGroupsHost::object_template`]'s number, which the host gives in the template store's order.

use std::collections::BTreeSet;

use crate::client::in_game_ui::{DrawableId, InGameUiSelection};
use crate::client::selection_xlat::{Disposition, SelectionTranslator, SelectionXlatHost};
use crate::common::game_type::ObjectId;
use crate::common::kind_of::KINDOF_STRUCTURE;
use crate::common::message_stream::MessageType;
use crate::common::message_types::{
    MSG_ADD_TEAM0, MSG_AREA_SELECTION, MSG_CREATE_SELECTED_GROUP_NO_SOUND, MSG_CREATE_TEAM0, MSG_SELECT_TEAM0,
};

/// What the double click and the control groups ask beyond [`SelectionXlatHost`].
pub trait SelectionGroupsHost: SelectionXlatHost {
    /// `TheGameClient->getDrawableList()`, in its order.
    fn all_drawables(&self) -> Vec<DrawableId>;
    /// `TheTacticalView->iterateDrawablesInRegion` over the whole view (`selectMatchingAcrossScreen`'s region).
    fn drawables_on_screen(&self) -> Vec<DrawableId>;
    /// `TheTacticalView->pickDrawable(point, FALSE, PICK_TYPE_SELECTABLE)` at the double click's point.
    fn pick_selectable_at_click(&self) -> Option<DrawableId>;
    /// `draw->isMassSelectable()` / `obj->isMassSelectable()`.
    fn is_mass_selectable(&self, obj: ObjectId) -> bool;
    /// The object's thing template, as a number in the template store's order (see the module docs).
    fn object_template(&self, obj: ObjectId) -> u32;
    /// `obj->getTemplate()->isEquivalentTo(template)`.
    fn is_equivalent_to(&self, obj: ObjectId, template: u32) -> bool;
    /// `obj->testStatus(OBJECT_STATUS_IS_CARBOMB)`.
    fn is_car_bomb(&self, obj: ObjectId) -> bool;
    /// `obj->isOffMap()`.
    fn is_off_map(&self, obj: ObjectId) -> bool;
    /// `localPlayer->getHotkeySquad(group)->getLiveObjects()`; `None` when the player or the squad is absent.
    fn hotkey_squad_live_objects(&mut self, group: i32) -> Option<Vec<ObjectId>>;
    /// `TheTacticalView->lookAt(obj->getDrawable()->getPosition())`.
    fn look_at(&mut self, obj: ObjectId);
    /// `TheInGameUI->message(TheGameText->fetch(label))`; `count` for `GUI:MaxSelectionSize`'s `%d`.
    fn ui_message(&mut self, label: &[u8], count: Option<i32>);
    /// `TheMessageStream->appendMessage(t)` with one drawable id argument.
    fn append_message_drawable(&mut self, t: MessageType, draw: DrawableId);
}

/// `selectSingleDrawableWithoutSound(draw)`: deselect all, select `draw`, and tell the logic
/// (`MSG_CREATE_SELECTED_GROUP_NO_SOUND(true, obj)`).
pub fn select_single_drawable_without_sound(draw: DrawableId, ui: &mut InGameUiSelection, h: &mut impl SelectionGroupsHost) {
    ui.deselect_all_drawables(true, h);
    ui.select_drawable(draw, h);
    if let Some(obj) = h.drawable_object(draw) {
        h.append_message(MSG_CREATE_SELECTED_GROUP_NO_SOUND, Some(true), &[obj]);
    }
}

/// `similarUnitSelection(test, data)`: selects `test` when it is a locally controlled, uncontained, mass-selectable unit on
/// the map like `template` (or any car bomb, when the selection had one), within the selection cap.
fn similar_unit_selection(
    test: DrawableId,
    template: u32,
    car_bomb: bool,
    newly: &mut Vec<DrawableId>,
    ui: &mut InGameUiSelection,
    h: &mut impl SelectionGroupsHost,
) -> bool {
    let Some(obj) = h.drawable_object(test) else { return false };
    let mut equivalent = h.is_equivalent_to(obj, template);
    if car_bomb && !equivalent && h.is_car_bomb(obj) {
        equivalent = true;
    }
    let drawable_selected = h.object_drawable(obj).is_some_and(|d| ui.is_selected(d));
    if equivalent
        && h.is_locally_controlled(obj)
        && h.contained_by(obj).is_none()
        && !drawable_selected
        && h.is_mass_selectable(obj)
        && !h.is_off_map(obj)
    {
        if ui.max_select_count() > 0 && ui.select_count() >= ui.max_select_count() {
            if !ui.displayed_max_warning() {
                ui.set_displayed_max_warning(true);
                h.ui_message(b"GUI:MaxSelectionSize", Some(ui.max_select_count()));
            }
        } else {
            ui.select_drawable(test, h);
            ui.set_displayed_max_warning(false);
            newly.push(test);
            return true;
        }
    }
    false
}

/// `selectMatchingAcrossRegion(region)`: every unit like one of the selected (locally controlled) ones, among `candidates`
/// (the screen's drawables, or every drawable for the map), is selected, and the logic told with one
/// `MSG_CREATE_SELECTED_GROUP_NO_SOUND(false, ...)`. `-1` when nothing useful was selected, else how many were added.
pub fn select_matching_across_region(
    across_map: bool,
    ui: &mut InGameUiSelection,
    h: &mut impl SelectionGroupsHost,
) -> i32 {
    let mut templates = BTreeSet::new();
    let mut car_bomb = false;
    for d in ui.all_selected_drawables().to_vec() {
        let Some(obj) = h.drawable_object(d) else { continue };
        if h.is_locally_controlled(obj) {
            templates.insert(h.object_template(obj));
            if h.is_car_bomb(obj) {
                car_bomb = true;
            }
        }
    }
    if templates.is_empty() {
        return -1;
    }
    let mut newly = Vec::new();
    let mut count = 0;
    for t in templates {
        let candidates = if across_map { h.all_drawables() } else { h.drawables_on_screen() };
        for d in candidates {
            count += i32::from(similar_unit_selection(d, t, car_bomb, &mut newly, ui, h));
        }
        ui.set_displayed_max_warning(false);
    }
    if count > 0 {
        let ids: Vec<ObjectId> = newly.iter().filter_map(|&d| h.drawable_object(d)).collect();
        h.append_message(MSG_CREATE_SELECTED_GROUP_NO_SOUND, Some(false), &ids);
    }
    count
}

/// `selectMatchingAcrossScreen()` (`across_map` false) / `selectMatchingAcrossMap()` (true), with their messages.
pub fn select_matching(across_map: bool, ui: &mut InGameUiSelection, h: &mut impl SelectionGroupsHost) -> i32 {
    let n = select_matching_across_region(across_map, ui, h);
    if n == -1 {
        h.ui_message(b"GUI:NothingSelected", None);
    } else if n > 0 {
        h.ui_message(if across_map { b"GUI:SelectedAcrossMap" } else { b"GUI:SelectedAcrossScreen" }, None);
    } else if across_map {
        let structure = ui
            .first_selected_drawable()
            .and_then(|d| h.drawable_object(d))
            .is_some_and(|o| h.is_kind_of(o, KINDOF_STRUCTURE));
        if !structure {
            h.ui_message(b"GUI:SelectedAcrossMap", None);
        }
    }
    n
}

impl SelectionTranslator {
    /// `MSG_META_BEGIN_FORCEATTACK` / `MSG_META_END_FORCEATTACK`.
    pub fn set_force_attack(&mut self, on: bool, ui: &mut InGameUiSelection) -> Disposition {
        ui.set_force_attack_mode(on);
        Disposition::Keep
    }

    /// `MSG_MOUSE_LEFT_DOUBLE_CLICK`: a double click at a point (`is_point`) on a unit of mine selects it and every unit like
    /// it on the screen (`alt`: across the map); with shift (prefer selection) the old selection is kept as well.
    pub fn left_double_click(
        &mut self,
        is_point: bool,
        alt: bool,
        ui: &mut InGameUiSelection,
        h: &mut impl SelectionGroupsHost,
    ) -> Disposition {
        if ui.is_in_force_attack_mode() || !is_point {
            return Disposition::Keep;
        }
        let Some(picked) = h.pick_selectable_at_click() else { return Disposition::Keep };
        let Some(obj) = h.drawable_object(picked) else { return Disposition::Keep };
        if !h.is_mass_selectable(obj) || !h.is_locally_controlled(obj) {
            return Disposition::Keep;
        }
        let old = if ui.is_in_prefer_selection_mode() { ui.all_selected_drawables().to_vec() } else { Vec::new() };
        select_single_drawable_without_sound(picked, ui, h);
        select_matching(alt, ui, h);
        h.append_message_drawable(MSG_AREA_SELECTION, picked);
        if ui.is_in_prefer_selection_mode() && !old.is_empty() {
            let mut ids = Vec::new();
            for d in old {
                if h.is_selectable(d) {
                    ui.select_drawable(d, h);
                    if let Some(o) = h.drawable_object(d) {
                        ids.push(o);
                    }
                }
            }
            h.append_message(MSG_CREATE_SELECTED_GROUP_NO_SOUND, Some(false), &ids);
        }
        Disposition::Destroy
    }

    /// `MSG_META_CREATE_TEAMn`: group `n` is made of the selected drawables of mine, in the client's drawable order.
    pub fn create_team(&mut self, group: i32, ui: &InGameUiSelection, h: &mut impl SelectionGroupsHost) -> Disposition {
        if (0..10).contains(&group) {
            let ids: Vec<ObjectId> = h
                .all_drawables()
                .into_iter()
                .filter(|&d| ui.is_selected(d))
                .filter_map(|d| h.drawable_object(d))
                .filter(|&o| h.is_locally_controlled(o))
                .collect();
            h.append_message(MessageType(MSG_CREATE_TEAM0.0 + group), None, &ids);
        }
        Disposition::Destroy
    }

    /// The double press of `MSG_META_SELECT_TEAMn` / `ADD_TEAMn` (within 20 frames, the same group): look at the group's
    /// last live object.
    fn is_double_press(&mut self, group: i32, now: u32) -> bool {
        if self.last_group_sel_time == 0 {
            self.last_group_sel_time = now;
        }
        now.wrapping_sub(self.last_group_sel_time) < 20 && group == self.last_group_sel_group
    }

    fn look_at_group(group: i32, h: &mut impl SelectionGroupsHost) {
        if let Some(objs) = h.hotkey_squad_live_objects(group) {
            if let Some(&last) = objs.last() {
                h.look_at(last);
            }
        }
    }

    /// `MSG_META_SELECT_TEAMn`: the selection becomes group `n` (`MSG_SELECT_TEAMn`; the client selects the group's units of
    /// mine without telling the logic again), or, pressed twice, the view looks at it.
    pub fn select_team(&mut self, group: i32, ui: &mut InGameUiSelection, h: &mut impl SelectionGroupsHost) -> Disposition {
        if (0..10).contains(&group) {
            let now = h.logic_frame();
            if self.is_double_press(group, now) {
                Self::look_at_group(group, h);
            } else {
                ui.deselect_all_drawables(false, h);
                h.append_message(MessageType(MSG_SELECT_TEAM0.0 + group), None, &[]);
                if let Some(objs) = h.hotkey_squad_live_objects(group) {
                    for o in objs {
                        if h.is_locally_controlled(o) {
                            if let Some(d) = h.object_drawable(o) {
                                ui.select_drawable(d, h);
                            }
                        }
                    }
                }
            }
            self.last_group_sel_time = now;
            self.last_group_sel_group = group;
        }
        Disposition::Destroy
    }

    /// `MSG_META_ADD_TEAMn`: group `n` is added to the selection (`MSG_ADD_TEAMn`); a selected structure is deselected first
    /// (the group force-attack exploit), or, pressed twice, the view looks at it.
    pub fn add_team(&mut self, group: i32, ui: &mut InGameUiSelection, h: &mut impl SelectionGroupsHost) -> Disposition {
        if (0..10).contains(&group) {
            let now = h.logic_frame();
            if self.is_double_press(group, now) {
                Self::look_at_group(group, h);
            } else {
                let structure = ui
                    .first_selected_drawable()
                    .and_then(|d| h.drawable_object(d))
                    .is_some_and(|o| h.is_kind_of(o, KINDOF_STRUCTURE));
                if structure {
                    ui.deselect_all_drawables(true, h);
                }
                h.append_message(MessageType(MSG_ADD_TEAM0.0 + group), None, &[]);
                if let Some(objs) = h.hotkey_squad_live_objects(group) {
                    for o in objs {
                        if let Some(d) = h.object_drawable(o) {
                            ui.select_drawable(d, h);
                        }
                    }
                }
            }
            self.last_group_sel_time = now;
            self.last_group_sel_group = group;
        }
        Disposition::Destroy
    }

    /// `MSG_META_VIEW_TEAMn`: the view looks at group `n`'s last live object. Upstream's range is kept: `group >= 1 && group
    /// <= 10`, so group 0 (`VIEW_TEAM0`) does nothing.
    pub fn view_team(&mut self, group: i32, h: &mut impl SelectionGroupsHost) -> Disposition {
        if (1..=10).contains(&group) {
            Self::look_at_group(group, h);
        }
        Disposition::Destroy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::selection_xlat::tests::Host;
    use crate::common::message_types::MSG_DESTROY_SELECTED_GROUP;

    impl SelectionGroupsHost for Host {
        fn all_drawables(&self) -> Vec<DrawableId> {
            self.all.clone()
        }
        fn drawables_on_screen(&self) -> Vec<DrawableId> {
            self.screen.clone()
        }
        fn pick_selectable_at_click(&self) -> Option<DrawableId> {
            self.picked
        }
        fn is_mass_selectable(&self, obj: ObjectId) -> bool {
            !self.structures.contains(&obj)
        }
        fn object_template(&self, obj: ObjectId) -> u32 {
            self.templates.get(&obj).copied().unwrap_or(0)
        }
        fn is_equivalent_to(&self, obj: ObjectId, template: u32) -> bool {
            self.object_template(obj) == template
        }
        fn is_car_bomb(&self, _obj: ObjectId) -> bool {
            false
        }
        fn is_off_map(&self, _obj: ObjectId) -> bool {
            false
        }
        fn hotkey_squad_live_objects(&mut self, group: i32) -> Option<Vec<ObjectId>> {
            self.squads.get(&group).cloned()
        }
        fn look_at(&mut self, obj: ObjectId) {
            self.looks.push(obj);
        }
        fn ui_message(&mut self, label: &[u8], _count: Option<i32>) {
            self.ui_messages.push(label.to_vec());
        }
        fn append_message_drawable(&mut self, t: MessageType, draw: DrawableId) {
            self.messages.push((t, None, vec![-draw]));
        }
    }

    /// C2a: the double click and the control groups. Predicted from SelectionXlat.cpp and InGameUI.cpp, with my units 1, 2, 3
    /// on the screen and 4 off it (all of template 7) and my building 5; a drawable argument is written negated:
    /// - a double click on 1: `DESTROY_SELECTED_GROUP(true)`, `CREATE_SELECTED_GROUP_NO_SOUND(true, 1)`, then
    ///   `(false, 2, 3)` (the screen's others like it), then `AREA_SELECTION(drawable 1)`; "GUI:SelectedAcrossScreen";
    /// - with alt, across the map: the others are `(false, 2, 3, 4)`;
    /// - ctrl+3 with 1, 2, 3, 4 selected: `CREATE_TEAM3(1, 2, 3, 4)`, in the drawable list's order;
    /// - 3 at frame 100, with group 3 being 1 and 2: no `DESTROY` (the group replaces it), `SELECT_TEAM3`, and the client
    ///   selects 1 and 2; 3 again at frame 105: the view looks at 2, the group's last, and no message; at frame 130 it is a
    ///   new press;
    /// - view group 0 does nothing (upstream's range is 1 to 10); view group 3 looks at 2.
    #[test]
    fn a_double_click_and_the_control_groups() {
        let mut h = Host { mine: vec![1, 2, 3, 4, 5], structures: vec![5], ..Default::default() };
        h.all = vec![1, 2, 3, 4, 5];
        h.screen = vec![1, 2, 3, 5];
        for o in 1..=4 {
            h.templates.insert(o, 7);
        }
        h.picked = Some(1);
        let (mut x, mut ui) = (SelectionTranslator::default(), InGameUiSelection::new());
        assert_eq!(x.left_double_click(true, false, &mut ui, &mut h), Disposition::Destroy);
        assert_eq!(
            h.messages,
            vec![
                (MSG_DESTROY_SELECTED_GROUP, Some(true), vec![]),
                (MSG_CREATE_SELECTED_GROUP_NO_SOUND, Some(true), vec![1]),
                (MSG_CREATE_SELECTED_GROUP_NO_SOUND, Some(false), vec![2, 3]),
                (MSG_AREA_SELECTION, None, vec![-1]),
            ]
        );
        assert_eq!(h.ui_messages, vec![b"GUI:SelectedAcrossScreen".to_vec()]);
        h.messages.clear();
        x.left_double_click(true, true, &mut ui, &mut h);
        assert_eq!(h.messages[2], (MSG_CREATE_SELECTED_GROUP_NO_SOUND, Some(false), vec![2, 3, 4]));
        h.messages.clear();
        x.create_team(3, &ui, &mut h);
        assert_eq!(h.messages, vec![(MessageType(MSG_CREATE_TEAM0.0 + 3), None, vec![1, 2, 3, 4])]);
        h.messages.clear();
        h.squads.insert(3, vec![1, 2]);
        h.frame = 100;
        x.select_team(3, &mut ui, &mut h);
        assert_eq!(h.messages, vec![(MessageType(MSG_SELECT_TEAM0.0 + 3), None, vec![])]);
        assert_eq!(ui.all_selected_drawables(), &[2, 1][..]);
        h.messages.clear();
        h.frame = 105;
        x.select_team(3, &mut ui, &mut h);
        assert_eq!((h.messages.len(), h.looks.clone()), (0, vec![2]));
        h.frame = 130;
        x.select_team(3, &mut ui, &mut h);
        assert_eq!(h.messages.len(), 1, "a new press after 20 frames");
        x.view_team(0, &mut h);
        x.view_team(3, &mut h);
        assert_eq!(h.looks, vec![2, 2]);
    }
}
