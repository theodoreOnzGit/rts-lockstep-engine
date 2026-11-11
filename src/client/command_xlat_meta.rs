// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/CommandXlat.cpp (CommandTranslator::translateGameMessage's
//     meta-key cases: MSG_META_SELECT_MATCHING_UNITS to MSG_META_FOLLOW, MSG_META_PLACE_BEACON, MSG_META_REMOVE_BEACON,
//     MSG_META_TOGGLE_ATTACKMOVE, MSG_META_TOGGLE_CAMERA_TRACKING_DRAWABLE, MSG_META_BEGIN/END_FORCEMOVE, WAYPOINTS,
//     PREFER_SELECTION, FORCEATTACK, MSG_META_ALL_CHEER, (#136) MSG_META_TOGGLE_CONTROL_BAR (in control_bar_stage.rs);
//     findCommandCenterOrMostExpensiveBuilding, viewCommandCenter,
//     amIAHero, iNeedAHero)
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (kindOfUnitSelection, selectAllUnitsByTypeAcrossRegion,
//     selectAllUnitsByTypeAcrossScreen, selectAllUnitsByTypeAcrossMap, selectAllUnitsByType,
//     selectUnitsMatchingCurrentSelection)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GameClient.cpp (GameClient::update's camera tracking)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #125: the command translator's meta-key cases, the ones a key of `CommandMap.ini` makes through
//! [`crate::client::meta_event`]: select the next or previous unit or worker, the hero, every unit (or aircraft), the units
//! like the selected ones; look at the command center or the last radar event; scatter, stop, make a formation; the beacon;
//! the held modes (force move, waypoints, prefer selection, force attack), attack-move and camera tracking; the cheer;
//! **(#131)** the keyboard camera's rotate and zoom flags and its reset.
//!
//! # As upstream reads it
//!
//! `CommandTranslator::translateGameMessage` (CommandXlat.cpp:2341) first destroys every message but the options, the
//! clear-game-data and the system messages while the input is off ([`input_disabled_destroys`]), then switches on the
//! message ([`CommandTranslator::meta`]):
//! - `MSG_META_SELECT_MATCHING_UNITS` (2360): `selectUnitsMatchingCurrentSelection()`, the units like the selected ones on
//!   the screen, else across the map ([`select_units_matching_current_selection`]); destroyed. `m_teamExists` is set by the
//!   `MSG_CREATE_SELECTED_GROUP_NO_SOUND` it appends when that passes the translator (3548), which
//!   [`CommandTranslator::observe_message`] already does: the case itself does not touch it.
//! - `MSG_META_SELECT_NEXT_UNIT` / `PREV_UNIT` / `NEXT_WORKER` / `PREV_WORKER` (2370-2823): walk the client's drawable list
//!   from the first selected drawable (backwards for next, "because list is prepended"; forwards for previous), wrapping
//!   at the end, for a locally controlled, uncontained mobile unit (a dozer for the workers); deselect all, make it the
//!   selected group (`MSG_CREATE_SELECTED_GROUP(TRUE, id)`), select it and look at it. With nothing selected the next
//!   starts from the last drawable, the previous checks the first. Destroyed (kept when there is no drawable).
//! - `MSG_META_SELECT_HERO` (2825): the first `KINDOF_HERO` object of the local player (its container when contained) is
//!   made the selection and looked at; destroyed (kept when there is no drawable, no hero or no drawable for it).
//! - `MSG_META_VIEW_COMMAND_CENTER` (2861): `viewCommandCenter()` looks at the local player's command center, else the
//!   structure that costs most to build; `MSG_META_VIEW_LAST_RADAR_EVENT` (2867): at the radar's last event. Destroyed.
//! - `MSG_META_SELECT_ALL` / `SELECT_ALL_AIRCRAFT` (2888): a selection the type would not take is deselected first (patch
//!   1.03's SCUD storm fix), then `selectAllUnitsByType(required, DOZER | HARVESTER | IGNORES_SELECT_ALL)`
//!   ([`select_all_units_by_type`]); destroyed.
//! - `MSG_META_SCATTER` / `STOP` / `CREATE_FORMATION` (2995-3016): `MSG_DO_SCATTER` / `MSG_DO_STOP` / `MSG_CREATE_FORMATION`
//!   for the selected group; `DEPLOY` / `FOLLOW` (3018, 3027): unimplemented upstream, destroyed. All destroyed.
//! - `MSG_META_PLACE_BEACON` (3087): in a multiplayer game, not a replay, with the local player active (and a
//!   multiplayer game info or `m_netMinPlayers == 0`), under the beacon cap, the `Command_PlaceBeacon` button becomes the
//!   pending GUI command; `MSG_META_REMOVE_BEACON` (3106): `MSG_REMOVE_BEACON` in a multiplayer game, not a replay. Destroyed.
//! - **(#131)** `MSG_META_BEGIN/END_CAMERA_ROTATE_LEFT`, `ROTATE_RIGHT`, `ZOOM_IN`, `ZOOM_OUT` (3205-3236): the InGameUI's
//!   keyboard camera flags, which `InGameUI::update` turns and zooms the view by
//!   ([`InGameUiSelection::update_keyboard_camera`]); `MSG_META_CAMERA_RESET` (3237): `TheInGameUI->resetCamera()`
//!   ([`CameraMetaHost::reset_camera`]). Kept. [`CommandTranslator::camera_meta`].
//! - **(#136)** `MSG_META_TOGGLE_CONTROL_BAR` (3168): the shell's top layout hidden or shown, else (not in a replay's
//!   playback) `ToggleControlBar()`; destroyed ([`crate::client::control_bar_stage::toggle_control_bar_meta`]).
//! - `MSG_META_TOGGLE_ATTACKMOVE` (3201), `TOGGLE_CAMERA_TRACKING_DRAWABLE` (3240), `BEGIN/END_FORCEMOVE` (3448),
//!   `BEGIN/END_WAYPOINTS`, `BEGIN/END_PREFER_SELECTION`, `BEGIN/END_FORCEATTACK` (3460-3491): InGameUI's flags; kept.
//! - `MSG_META_ALL_CHEER` (3493): in a multiplayer game, the misc audio's `AllCheerSound` and `MSG_DO_CHEER`, destroyed;
//!   else kept.
//!
//! # Upstream quirks that are kept
//!
//! - The previous unit (or worker) with nothing selected checks **only the first drawable** (`for (temp = first; temp;
//!   temp = temp->getPrevDrawable())`, and the first has no previous), and stops at a drawable without an object.
//! - The next walk's wrap (`hack`) checks the last drawable after the list's head; when the selected drawable is itself the
//!   last, the walk comes back to it and may select it again alone. The previous walk checks the first drawable twice after
//!   its wrap, once with its special test (`getNextDrawable() == selected`, with `KINDOF_NO_SELECT` for units and **no**
//!   `KINDOF_DOZER` for workers) and once with the normal test (no `KINDOF_NO_SELECT` for units).
//! - The workers' tests differ by direction: the next worker with nothing selected needs no `isMobile`, the previous one
//!   does; only the next worker with nothing selected plays the unit's select voice.
//! - `viewCommandCenter` looks at the **last** command center the player iterates (each one overwrites the location,
//!   whatever the comment says), and a non-structure before any command center does not count as "at least one".
//! - The selection walks and `SELECT_HERO` keep the message when there is no drawable at all (`break` before `disp`).
//! - `selectAllUnitsByType` goes across the map only when the screen selected none; its group message says "new group"
//!   only when nothing was selected before.
//! - `MSG_META_ALL_CHEER` outside a multiplayer game is kept.
//!
//! # Where this differs from upstream
//!
//! - **Crashes and a hang are not translated.** The next walk dereferences `NULL` when the list holds only the selected
//!   drawable (the wrap's `temp->getNextDrawable()` after `getPrevDrawable()` of the only drawable), and both walks
//!   dereference a selected drawable without an object; here the first stops the walk and the second counts as not
//!   locally controlled. **The next walk never ends** (found by the differential test's first run, which hung) when the
//!   first selected drawable is the list's last and no drawable qualifies, itself included: the wrap's `hack` moves
//!   `temp` onto the selected drawable after the loop's `temp != selectedDrawable` test, so the walk goes round again;
//!   here a second wrap ends it with nothing selected. The differential test generates none of the three.
//! - `draw->isKindOf` / `isAnyKindOf` read the **drawable's** template (a disguised unit's drawable shows another); the
//!   host answers ([`CommandMetaHost::drawable_is_kind_of`]), the World's client from the object (it has no disguise).
//! - The next worker's voice is a copy of the template's `VoiceSelect` as upstream's, played through
//!   [`UnitVoiceHost::play_unit_voice`] as a template voice (the client then keeps its playing index for the template,
//!   which upstream's copy does not write back).
//! - The camera tracking (`GameClient::update`, GameClient.cpp:611) is [`camera_tracking_look_at`], which the application
//!   calls once a frame.
//!
//! # Representation
//!
//! [`CommandTranslator::meta`] takes the message's type and answers the disposition (`None` for a type it has no case
//! for). The host ([`CommandMetaHost`]) answers what the cases read beyond the selection's and the command translator's
//! questions and takes the calls that leave the client: the tactical view's `lookAt`, the cheer. The client's drawable list
//! is [`SelectionGroupsHost::all_drawables`] in its order (`firstDrawable()` then `getNextDrawable()`); a drawable's previous
//! and next are its neighbours there.

use crate::client::command_xlat::{input_disabled_destroys, CommandTranslator, CommandXlatHost, GuiButton};
use crate::client::in_game_ui::{DrawableId, InGameUiSelection};
use crate::client::selection_groups::select_matching;
use crate::client::selection_xlat::Disposition;
use crate::client::unit_voice::{UnitVoice, VoiceSource};
use crate::common::base_type::Coord3D;
use crate::common::game_type::ObjectId;
use crate::common::kind_of::{
    KindOfType, KINDOF_AIRCRAFT, KINDOF_COMMANDCENTER, KINDOF_DOZER, KINDOF_HARVESTER, KINDOF_HERO, KINDOF_IGNORES_SELECT_ALL,
    KINDOF_NO_SELECT, KINDOF_STRUCTURE,
};
use crate::common::message_stream::MessageType;
use crate::common::message_types::*;
use crate::common::thing_template::ThingTemplateAudioType;
use crate::logic::modules::plant;

/// What the meta cases ask beyond [`CommandXlatHost`]; **(#136)** `MSG_META_TOGGLE_CONTROL_BAR`'s shell, recorder and
/// control bar ([`crate::client::control_bar_stage::ToggleControlBarHost`]).
pub trait CommandMetaHost: CommandXlatHost + CameraMetaHost + crate::client::control_bar_stage::ToggleControlBarHost {
    /// `obj->isMobile()`.
    fn object_is_mobile(&self, obj: ObjectId) -> bool;
    /// `obj->isEffectivelyDead()`.
    fn object_is_effectively_dead(&self, obj: ObjectId) -> bool;
    /// `draw->isKindOf(k)`: the drawable's template's kinds.
    fn drawable_is_kind_of(&self, draw: DrawableId, k: KindOfType) -> bool;
    /// `draw->getPosition()`.
    fn drawable_position(&self, draw: DrawableId) -> Coord3D;
    /// `ThePlayerList->getLocalPlayer()->iterateObjects(...)`'s objects in order; `None` without a local player.
    fn local_player_objects(&mut self) -> Option<Vec<ObjectId>>;
    /// `obj->getTemplate()->calcCostToBuild(obj->getControllingPlayer())`.
    fn object_cost_to_build(&mut self, obj: ObjectId) -> i32;
    /// `TheRadar->getLastEventLoc(&loc)`: the location when it answers TRUE.
    fn radar_last_event_loc(&self) -> Option<Coord3D>;
    /// `TheTacticalView->lookAt(pos)`.
    fn look_at_position(&mut self, pos: &Coord3D);
    /// `TheGameLogic->isInMultiplayerGame()`.
    fn is_in_multiplayer_game(&self) -> bool;
    /// `TheGameLogic->isInReplayGame()`.
    fn is_in_replay_game(&self) -> bool;
    /// `ThePlayerList->getLocalPlayer()->isPlayerActive()`.
    fn local_player_active(&self) -> bool;
    /// `TheGlobalData->m_netMinPlayers == 0 || TheGameInfo->isMultiPlayer()`.
    fn beacons_allowed_by_game_info(&self) -> bool;
    /// `countObjectsByThingTemplate(1, &beaconTemplate, false, &count)` of the local player: its beacons.
    fn local_beacon_count(&mut self) -> i32;
    /// `TheMultiplayerSettings->getMaxBeaconsPerPlayer()`.
    fn max_beacons_per_player(&self) -> i32;
    /// `TheControlBar->findCommandButton("Command_PlaceBeacon")`.
    fn place_beacon_button(&mut self) -> Option<GuiButton>;
    /// `TheAudio->addAudioEvent(&TheAudio->getMiscAudio()->m_allCheerSound)`.
    fn play_all_cheer_sound(&mut self);
}

/// #131: what the camera's meta cases ask beyond the InGameUI's flags.
pub trait CameraMetaHost {
    /// `TheInGameUI->resetCamera()`: the tactical view's ([`crate::client::in_game_ui::reset_camera`]), which the host
    /// owns.
    fn reset_camera(&mut self);
}

/// `kindOfUnitSelection(test, data)`: selects `test` when its object is of every `must_set` kind and none of `must_clear`,
/// locally controlled, uncontained, unselected, alive, mass-selectable and on the map, within the selection cap.
fn kind_of_unit_selection(
    test: DrawableId,
    must_set: &[KindOfType],
    must_clear: &[KindOfType],
    newly: &mut Vec<DrawableId>,
    ui: &mut InGameUiSelection,
    h: &mut impl CommandMetaHost,
) -> bool {
    // "Only things with objects can be selected"
    let Some(object) = h.drawable_object(test) else { return false };
    let is_kind_of_match = must_set.iter().all(|&k| h.is_kind_of(object, k)) && !must_clear.iter().any(|&k| h.is_kind_of(object, k));
    // "only select objects if not already selected"
    let drawable_selected = h.object_drawable(object).is_some_and(|d| ui.is_selected(d));
    if is_kind_of_match
        && h.is_locally_controlled(object)
        && h.contained_by(object).is_none()
        && !drawable_selected
        && (!h.object_is_effectively_dead(object) || plant(9131))
        && h.is_mass_selectable(object)
        && !h.is_off_map(object)
    {
        // "enforce optional unit cap"
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

/// `InGameUI::selectAllUnitsByTypeAcrossRegion(region, mustBeSet, mustBeClear)`: the screen's drawables (`across_map`
/// false: the tactical view's region) or every drawable, through [`kind_of_unit_selection`]; the logic is told with one
/// `MSG_CREATE_SELECTED_GROUP` ("new group" when nothing was selected before). How many were selected.
pub fn select_all_units_by_type_across_region(
    across_map: bool,
    must_set: &[KindOfType],
    must_clear: &[KindOfType],
    ui: &mut InGameUiSelection,
    h: &mut impl CommandMetaHost,
) -> i32 {
    let old_selection_count = ui.all_selected_drawables().len();
    let mut newly = Vec::new();
    let mut new_selection_count = 0;
    let candidates = if across_map { h.all_drawables() } else { h.drawables_on_screen() };
    for d in candidates {
        if kind_of_unit_selection(d, must_set, must_clear, &mut newly, ui, h) {
            new_selection_count += 1;
        }
    }
    ui.set_displayed_max_warning(false);
    if new_selection_count > 0 {
        let ids: Vec<ObjectId> = newly.iter().filter_map(|&d| h.drawable_object(d)).collect();
        let new_group = old_selection_count == 0 || plant(9132);
        h.append_message(MSG_CREATE_SELECTED_GROUP, Some(new_group), &ids);
    }
    new_selection_count
}

/// `selectAllUnitsByTypeAcrossScreen` (`across_map` false) / `selectAllUnitsByTypeAcrossMap` (true), with their messages.
pub fn select_all_units_by_type_across(across_map: bool, must_set: &[KindOfType], must_clear: &[KindOfType], ui: &mut InGameUiSelection, h: &mut impl CommandMetaHost) -> i32 {
    let n = select_all_units_by_type_across_region(across_map, must_set, must_clear, ui, h);
    if n == -1 {
        h.ui_message(b"GUI:NothingSelected", None);
    } else if n == 0 {
        if across_map {
            let structure = ui.first_selected_drawable().and_then(|d| h.drawable_object(d)).is_some_and(|o| h.is_kind_of(o, KINDOF_STRUCTURE));
            if !structure {
                h.ui_message(b"GUI:SelectedAcrossMap", None);
            }
        }
    } else {
        h.ui_message(if across_map { b"GUI:SelectedAcrossMap" } else { b"GUI:SelectedAcrossScreen" }, None);
    }
    n
}

/// `InGameUI::selectAllUnitsByType(mustBeSet, mustBeClear)`: on the screen, else (none there) across the map.
pub fn select_all_units_by_type(must_set: &[KindOfType], must_clear: &[KindOfType], ui: &mut InGameUiSelection, h: &mut impl CommandMetaHost) -> i32 {
    let n = select_all_units_by_type_across(false, must_set, must_clear, ui, h);
    if n == -1 {
        return n;
    }
    if n == 0 || plant(9133) {
        return select_all_units_by_type_across(true, must_set, must_clear, ui, h);
    }
    n
}

/// `InGameUI::selectUnitsMatchingCurrentSelection()`: the units like the selected ones on the screen
/// ([`select_matching`]), else (none there) across the map.
pub fn select_units_matching_current_selection(ui: &mut InGameUiSelection, h: &mut impl CommandMetaHost) -> i32 {
    let n = select_matching(false, ui, h);
    if n == -1 {
        return n;
    }
    if n == 0 || plant(9145) {
        return select_matching(true, ui, h);
    }
    n
}

/// `GameClient::update`'s camera tracking (GameClient.cpp:611), once a client frame: while
/// `TheInGameUI->isCameraTrackingDrawable()`, the view looks at the first selected drawable; with nothing selected the
/// tracking ends. The position to look at, if any.
pub fn camera_tracking_look_at(ui: &mut InGameUiSelection, h: &impl CommandMetaHost) -> Option<Coord3D> {
    if !ui.is_camera_tracking_drawable() {
        return None;
    }
    match ui.first_selected_drawable() {
        Some(d) => Some(h.drawable_position(d)),
        None => {
            ui.set_camera_tracking_drawable(false);
            None
        }
    }
}

/// The selected group the walks make: deselect all (with `deselect`), `MSG_CREATE_SELECTED_GROUP(TRUE, id)`, select, look.
fn make_the_selection(draw: DrawableId, obj: ObjectId, deselect: bool, ui: &mut InGameUiSelection, h: &mut impl CommandMetaHost) {
    if deselect {
        // "deselect other units"
        ui.deselect_all_drawables(true, h);
    }
    // "create a new group." ("New group or add to group? Passed in value is true if we are creating a new group.")
    h.append_message(MSG_CREATE_SELECTED_GROUP, Some(true), &[obj]);
    ui.select_drawable(draw, h);
    // "center on the unit"
    let at = h.drawable_position(draw);
    h.look_at_position(&at);
}

/// The next worker's voice: `AudioEventRTS soundEvent = *temp->getTemplate()->getVoiceSelect();
/// soundEvent.setObjectID(object->getID()); TheAudio->addAudioEvent(&soundEvent)`.
fn play_voice_select(obj: ObjectId, h: &mut impl CommandMetaHost) {
    let t = ThingTemplateAudioType::VoiceSelect;
    let v = UnitVoice { name: h.template_audio(obj, t), object: obj, template: h.voice_template_name(obj), source: VoiceSource::Template(t) };
    h.play_unit_voice(v);
}

/// What the walks test of a drawable's object.
struct Facts {
    mobile: bool,
    local: bool,
    contained: bool,
    no_select: bool,
    dozer: bool,
}

fn facts(obj: ObjectId, h: &impl CommandMetaHost) -> Facts {
    Facts {
        mobile: h.object_is_mobile(obj),
        local: h.is_locally_controlled(obj),
        contained: h.contained_by(obj).is_some(),
        no_select: h.is_kind_of(obj, KINDOF_NO_SELECT),
        dozer: h.is_kind_of(obj, KINDOF_DOZER),
    }
}

impl CommandTranslator {
    /// #131: the keyboard camera's cases of `translateGameMessage` (CommandXlat.cpp:3205-3239), after the input gate:
    /// `MSG_META_BEGIN/END_CAMERA_ROTATE_LEFT`, `ROTATE_RIGHT`, `ZOOM_IN` and `ZOOM_OUT` set and clear the InGameUI's
    /// flags (the `DEBUG_ASSERTCRASH`es that a flag was not already so are debug only); `MSG_META_CAMERA_RESET` asks
    /// `TheInGameUI->resetCamera()`. All kept. `None` for another message.
    pub fn camera_meta(t: MessageType, ui: &mut InGameUiSelection, h: &mut impl CameraMetaHost) -> Option<Disposition> {
        use Disposition::{Destroy, Keep};
        let camera = [
            MSG_META_BEGIN_CAMERA_ROTATE_LEFT,
            MSG_META_END_CAMERA_ROTATE_LEFT,
            MSG_META_BEGIN_CAMERA_ROTATE_RIGHT,
            MSG_META_END_CAMERA_ROTATE_RIGHT,
            MSG_META_BEGIN_CAMERA_ZOOM_IN,
            MSG_META_END_CAMERA_ZOOM_IN,
            MSG_META_BEGIN_CAMERA_ZOOM_OUT,
            MSG_META_END_CAMERA_ZOOM_OUT,
            MSG_META_CAMERA_RESET,
        ];
        if !camera.contains(&t) {
            return None;
        }
        if input_disabled_destroys(t, ui.input_enabled()) {
            return Some(Destroy);
        }
        match t {
            MSG_META_BEGIN_CAMERA_ROTATE_LEFT | MSG_META_END_CAMERA_ROTATE_LEFT => {
                if plant(9201) && t == MSG_META_END_CAMERA_ROTATE_LEFT {
                    ui.set_camera_rotate_right(false);
                } else {
                    ui.set_camera_rotate_left(t == MSG_META_BEGIN_CAMERA_ROTATE_LEFT);
                }
            }
            MSG_META_BEGIN_CAMERA_ROTATE_RIGHT | MSG_META_END_CAMERA_ROTATE_RIGHT => ui.set_camera_rotate_right(t == MSG_META_BEGIN_CAMERA_ROTATE_RIGHT),
            MSG_META_BEGIN_CAMERA_ZOOM_IN | MSG_META_END_CAMERA_ZOOM_IN => ui.set_camera_zoom_in(t == MSG_META_BEGIN_CAMERA_ZOOM_IN),
            MSG_META_BEGIN_CAMERA_ZOOM_OUT | MSG_META_END_CAMERA_ZOOM_OUT => ui.set_camera_zoom_out(t == MSG_META_BEGIN_CAMERA_ZOOM_OUT),
            _ => h.reset_camera(),
        }
        Some(Keep)
    }

    /// GitHub #125: `translateGameMessage` for the meta messages of the module docs (after its input gate): the
    /// disposition, or `None` for a message type this has no case for.
    pub fn meta(&self, t: MessageType, ui: &mut InGameUiSelection, h: &mut impl CommandMetaHost) -> Option<Disposition> {
        use Disposition::{Destroy, Keep};
        if let Some(d) = Self::camera_meta(t, ui, h) {
            return Some(d);
        }
        let handled = [
            MSG_META_SELECT_MATCHING_UNITS,
            MSG_META_SELECT_NEXT_UNIT,
            MSG_META_SELECT_PREV_UNIT,
            MSG_META_SELECT_NEXT_WORKER,
            MSG_META_SELECT_PREV_WORKER,
            MSG_META_SELECT_HERO,
            MSG_META_VIEW_COMMAND_CENTER,
            MSG_META_VIEW_LAST_RADAR_EVENT,
            MSG_META_SELECT_ALL,
            MSG_META_SELECT_ALL_AIRCRAFT,
            MSG_META_SCATTER,
            MSG_META_STOP,
            MSG_META_CREATE_FORMATION,
            MSG_META_DEPLOY,
            MSG_META_FOLLOW,
            MSG_META_PLACE_BEACON,
            MSG_META_REMOVE_BEACON,
            MSG_META_TOGGLE_ATTACKMOVE,
            MSG_META_TOGGLE_CAMERA_TRACKING_DRAWABLE,
            MSG_META_BEGIN_FORCEMOVE,
            MSG_META_END_FORCEMOVE,
            MSG_META_BEGIN_WAYPOINTS,
            MSG_META_END_WAYPOINTS,
            MSG_META_BEGIN_PREFER_SELECTION,
            MSG_META_END_PREFER_SELECTION,
            MSG_META_BEGIN_FORCEATTACK,
            MSG_META_END_FORCEATTACK,
            MSG_META_ALL_CHEER,
            MSG_META_TOGGLE_CONTROL_BAR,
        ];
        if !handled.contains(&t) {
            return None;
        }
        // "We want to always be able to get to the options menu even during no input times ..."
        if input_disabled_destroys(t, ui.input_enabled()) {
            return Some(Destroy);
        }
        let d = match t {
            // #136: CommandXlat.cpp:3168
            MSG_META_TOGGLE_CONTROL_BAR => crate::client::control_bar_stage::toggle_control_bar_meta(h),
            MSG_META_SELECT_MATCHING_UNITS => {
                select_units_matching_current_selection(ui, h);
                Destroy
            }
            MSG_META_SELECT_NEXT_UNIT => Self::select_next(false, ui, h),
            MSG_META_SELECT_PREV_UNIT => Self::select_prev(false, ui, h),
            MSG_META_SELECT_NEXT_WORKER => Self::select_next(true, ui, h),
            MSG_META_SELECT_PREV_WORKER => Self::select_prev(true, ui, h),
            MSG_META_SELECT_HERO => Self::select_hero(ui, h),
            MSG_META_VIEW_COMMAND_CENTER => {
                Self::view_command_center(h);
                Destroy
            }
            MSG_META_VIEW_LAST_RADAR_EVENT => {
                // ("Excuse me?  You don't need radar for the spacebar.  That's silly.": the radar test is commented out)
                if !plant(9129) {
                    if let Some(last_event) = h.radar_last_event_loc() {
                        h.look_at_position(&last_event);
                    }
                }
                Destroy
            }
            MSG_META_SELECT_ALL | MSG_META_SELECT_ALL_AIRCRAFT => {
                let disqualifying = [KINDOF_DOZER, KINDOF_HARVESTER, KINDOF_IGNORES_SELECT_ALL];
                let select_aircraft = t == MSG_META_SELECT_ALL_AIRCRAFT;
                let required: &[KindOfType] = if select_aircraft { &[KINDOF_AIRCRAFT] } else { &[] };
                // "Kris: Patch 1.03. We need to deselect all the units if any of the units we have selected are
                // incompatible with the select all type we are triggering. This is a fix for the SCUDSTORM exploit."
                if !plant(9130) {
                    for draw in ui.all_selected_drawables().to_vec() {
                        let any_disqualifying = disqualifying.iter().any(|&k| h.drawable_is_kind_of(draw, k));
                        // (upstream's two branches, aircraft and not, each deselect all and stop)
                        let incompatible = if select_aircraft {
                            any_disqualifying || !h.drawable_is_kind_of(draw, KINDOF_AIRCRAFT)
                        } else {
                            any_disqualifying || h.drawable_is_kind_of(draw, KINDOF_STRUCTURE)
                        };
                        if incompatible {
                            ui.deselect_all_drawables(true, h);
                            break;
                        }
                    }
                }
                select_all_units_by_type(required, &disqualifying, ui, h);
                Destroy
            }
            // "This message always works on the currently selected team"
            MSG_META_SCATTER => {
                h.append_command(if plant(9134) { MSG_DO_STOP } else { MSG_DO_SCATTER }, &[]);
                Destroy
            }
            MSG_META_STOP => {
                h.append_command(MSG_DO_STOP, &[]);
                Destroy
            }
            MSG_META_CREATE_FORMATION => {
                h.append_command(MSG_CREATE_FORMATION, &[]);
                Destroy
            }
            // ("@todo srj implement me")
            MSG_META_DEPLOY => {
                if plant(9135) {
                    Keep
                } else {
                    Destroy
                }
            }
            MSG_META_FOLLOW => Destroy,
            MSG_META_PLACE_BEACON => {
                if h.is_in_multiplayer_game() && !h.is_in_replay_game() && h.local_player_active() && h.beacons_allowed_by_game_info() {
                    let count = h.local_beacon_count();
                    if count < h.max_beacons_per_player() || plant(9136) {
                        let command_button = h.place_beacon_button();
                        ui.set_gui_command(command_button);
                    }
                }
                Destroy
            }
            MSG_META_REMOVE_BEACON => {
                if h.is_in_multiplayer_game() && (!h.is_in_replay_game() || plant(9137)) {
                    h.append_command(MSG_REMOVE_BEACON, &[]);
                }
                Destroy
            }
            MSG_META_TOGGLE_ATTACKMOVE => {
                ui.toggle_attack_move_to_mode();
                if plant(9138) {
                    Destroy
                } else {
                    Keep
                }
            }
            MSG_META_TOGGLE_CAMERA_TRACKING_DRAWABLE => {
                ui.set_camera_tracking_drawable(true);
                Keep
            }
            MSG_META_BEGIN_FORCEMOVE | MSG_META_END_FORCEMOVE => {
                ui.set_force_move_mode(t == MSG_META_BEGIN_FORCEMOVE);
                Keep
            }
            MSG_META_BEGIN_WAYPOINTS | MSG_META_END_WAYPOINTS => {
                ui.set_waypoint_mode(t == MSG_META_BEGIN_WAYPOINTS || plant(9140));
                Keep
            }
            MSG_META_BEGIN_PREFER_SELECTION | MSG_META_END_PREFER_SELECTION => {
                ui.set_prefer_selection_mode(t == MSG_META_BEGIN_PREFER_SELECTION);
                Keep
            }
            MSG_META_BEGIN_FORCEATTACK | MSG_META_END_FORCEATTACK => {
                ui.set_force_attack_mode(t == MSG_META_BEGIN_FORCEATTACK);
                Keep
            }
            MSG_META_ALL_CHEER => {
                if h.is_in_multiplayer_game() || plant(9139) {
                    h.play_all_cheer_sound();
                    h.append_command(MSG_DO_CHEER, &[]);
                    Destroy
                } else {
                    Keep
                }
            }
            _ => return None,
        };
        Some(d)
    }

    /// `MSG_META_SELECT_NEXT_UNIT` (`worker` false) / `MSG_META_SELECT_NEXT_WORKER` (true): "because list is prepended,
    /// iterate through backwards".
    fn select_next(worker: bool, ui: &mut InGameUiSelection, h: &mut impl CommandMetaHost) -> Disposition {
        let list = h.all_drawables();
        // "if there is nothing on the screen, bail"
        if list.is_empty() {
            return Disposition::Keep;
        }
        if ui.select_count() == 0 {
            // "get the last drawable"; "temp is the last drawable"
            for &temp in list.iter().rev() {
                // "if you've reached the end of the list, don't select anything"
                let Some(object) = h.drawable_object(temp) else { break };
                let f = facts(object, h);
                let ok = if worker {
                    // "make sure you select only workers"
                    f.local && !f.contained && f.dozer && (f.mobile || !plant(9124))
                } else {
                    f.mobile && f.local && !f.contained && !f.no_select
                };
                if ok {
                    h.append_message(MSG_CREATE_SELECTED_GROUP, Some(true), &[object]);
                    ui.select_drawable(temp, h);
                    if worker && !plant(9125) {
                        // "play the units sound"
                        play_voice_select(object, h);
                    }
                    let at = h.drawable_position(temp);
                    h.look_at_position(&at);
                    break;
                }
            }
        } else {
            let Some(selected_drawable) = ui.first_selected_drawable() else { return Disposition::Destroy };
            // (upstream dereferences the selected drawable's object; without one it crashes: taken as not controlled)
            let local = h.drawable_object(selected_drawable).is_some_and(|o| h.is_locally_controlled(o));
            let Some(sel) = list.iter().position(|&d| d == selected_drawable) else { return Disposition::Destroy };
            if local {
                let prev = |i: usize| i.checked_sub(1);
                let next = |i: usize| (i + 1 < list.len()).then_some(i + 1);
                let mut new_drawable = None;
                let mut hack = false;
                let mut wrapped = false;
                // "find the previous selectable drawable"
                let mut temp = prev(sel);
                loop {
                    if temp == Some(sel) {
                        break;
                    }
                    if hack {
                        // (upstream: `temp->getNextDrawable()` of `NULL` when the list holds one drawable; not translated)
                        let Some(t) = temp else { break };
                        temp = next(t);
                        hack = false;
                    }
                    match temp {
                        // "if temp is null, set it to the last drawable and loop back to selected drawable"
                        None => {
                            // (a second wrap: the selected drawable is the last and nothing qualifies, so upstream's
                            // `hack` steps over the loop's end test for ever; not translated, see the module docs)
                            if wrapped {
                                break;
                            }
                            wrapped = true;
                            temp = Some(list.len() - 1);
                            hack = !plant(9121);
                        }
                        // "else search for a previous selectable drawable"
                        Some(t) => {
                            if next_qualifies(worker, list[t], h) {
                                new_drawable = Some(t);
                                break;
                            }
                        }
                    }
                    temp = prev(temp.expect("set above"));
                }
                // "if there is another selectable unit, select it"
                if let Some(n) = new_drawable {
                    let object = h.drawable_object(list[n]).expect("tested");
                    make_the_selection(list[n], object, true, ui, h);
                }
            }
        }
        Disposition::Destroy
    }

    /// `MSG_META_SELECT_PREV_UNIT` (`worker` false) / `MSG_META_SELECT_PREV_WORKER` (true): "because list is prepended,
    /// iterate through forwards".
    fn select_prev(worker: bool, ui: &mut InGameUiSelection, h: &mut impl CommandMetaHost) -> Disposition {
        let list = h.all_drawables();
        // "if there is nothing on the screen, bail"
        if list.is_empty() {
            return Disposition::Keep;
        }
        if ui.select_count() == 0 {
            // "get the first drawable" (and its previous, which there is none of: only the first is checked)
            let walk: Vec<DrawableId> = if plant(9122) { list.clone() } else { vec![list[0]] };
            for temp in walk {
                // "if you've reached the end of the list, don't select anything"
                let Some(object) = h.drawable_object(temp) else { break };
                let f = facts(object, h);
                let ok = f.mobile && f.local && !f.contained && if worker { f.dozer } else { !f.no_select };
                if ok {
                    h.append_message(MSG_CREATE_SELECTED_GROUP, Some(true), &[object]);
                    ui.select_drawable(temp, h);
                    let at = h.drawable_position(temp);
                    h.look_at_position(&at);
                    break;
                }
            }
        } else {
            let Some(selected_drawable) = ui.first_selected_drawable() else { return Disposition::Destroy };
            let local = h.drawable_object(selected_drawable).is_some_and(|o| h.is_locally_controlled(o));
            let Some(sel) = list.iter().position(|&d| d == selected_drawable) else { return Disposition::Destroy };
            if local {
                let next = |i: usize| (i + 1 < list.len()).then_some(i + 1);
                let mut new_drawable = None;
                // "takes care of when for loop skips firstdrawable"
                let mut hack = false;
                // "find the next selectable drawable"
                let mut temp = next(sel);
                loop {
                    if temp == Some(sel) {
                        break;
                    }
                    if hack {
                        temp = Some(0);
                        hack = false;
                        if temp == Some(sel) {
                            break;
                        }
                    }
                    match temp {
                        // "if temp is null, set it to the first drawable and loop forward to selected drawable"
                        None => {
                            temp = Some(0);
                            hack = true;
                            // "must take case of this case here or else the loop will break without getting newDrawable"
                            if let Some(o) = h.drawable_object(list[0]) {
                                let f = facts(o, h);
                                let before_selected = next(0) == Some(sel) || plant(9123);
                                let kind = worker || !f.no_select;
                                if before_selected && !ui.is_selected(list[0]) && f.mobile && f.local && !f.contained && kind {
                                    new_drawable = Some(0);
                                    break;
                                }
                            }
                        }
                        // "else search for a next selectable drawable"
                        Some(t) => {
                            if let Some(o) = h.drawable_object(list[t]) {
                                let f = facts(o, h);
                                let kind = !worker || f.dozer;
                                if !ui.is_selected(list[t]) && f.mobile && f.local && !f.contained && kind {
                                    new_drawable = Some(t);
                                    break;
                                }
                            }
                        }
                    }
                    temp = next(temp.expect("set above"));
                }
                // "if there is another selectable unit, select it"
                if let Some(n) = new_drawable {
                    let object = h.drawable_object(list[n]).expect("tested");
                    make_the_selection(list[n], object, true, ui, h);
                }
            }
        }
        Disposition::Destroy
    }

    /// `MSG_META_SELECT_HERO`.
    fn select_hero(ui: &mut InGameUiSelection, h: &mut impl CommandMetaHost) -> Disposition {
        // "if there is nothing on the screen, bail"
        if h.all_drawables().is_empty() {
            return Disposition::Keep;
        }
        // `iNeedAHero()`: the first of the local player's objects that is a hero (`amIAHero`)
        let Some(objects) = h.local_player_objects() else { return Disposition::Keep };
        let Some(mut hero) = objects.into_iter().find(|&o| h.is_kind_of(o, KINDOF_HERO)) else { return Disposition::Keep };
        if !plant(9126) {
            if let Some(c) = h.contained_by(hero) {
                hero = c;
            }
        }
        let Some(hero_draw) = h.object_drawable(hero) else { return Disposition::Keep };
        ui.deselect_all_drawables(true, h);
        h.append_message(MSG_CREATE_SELECTED_GROUP, Some(true), &[hero]);
        ui.select_drawable(hero_draw, h);
        let at = h.drawable_position(hero_draw);
        h.look_at_position(&at);
        Disposition::Destroy
    }

    /// `viewCommandCenter()`: `findCommandCenterOrMostExpensiveBuilding` over the local player's objects ("We want to get
    /// the first Command Center in the list. Barring that, we want the most expensive structure we currently own."),
    /// then the view looks there.
    fn view_command_center(h: &mut impl CommandMetaHost) {
        let Some(objects) = h.local_player_objects() else { return };
        let (mut at_least_one, mut is_command_center, mut val, mut loc) = (false, false, -1, Coord3D::default());
        for obj in objects {
            if h.is_kind_of(obj, KINDOF_COMMANDCENTER) {
                if plant(9127) && is_command_center {
                    continue;
                }
                is_command_center = true;
                loc = h.object_position(obj);
            } else if !is_command_center {
                if !h.is_kind_of(obj, KINDOF_STRUCTURE) {
                    if plant(9128) {
                        at_least_one = true;
                    }
                    continue;
                }
                let cost_to_build = h.object_cost_to_build(obj);
                if cost_to_build > val {
                    val = cost_to_build;
                    loc = h.object_position(obj);
                }
            }
            at_least_one = true;
        }
        if at_least_one {
            h.look_at_position(&loc);
        }
        // ("@todo. Find their starting position and look at that instead?")
    }
}

/// The next walk's test of a drawable when something is selected: a locally controlled, uncontained, mobile unit without
/// `KINDOF_NO_SELECT`, or (`worker`) a locally controlled, uncontained dozer.
fn next_qualifies(worker: bool, d: DrawableId, h: &impl CommandMetaHost) -> bool {
    let Some(o) = h.drawable_object(d) else { return false };
    let f = facts(o, h);
    if worker {
        f.local && !f.contained && f.dozer
    } else {
        f.mobile && f.local && !f.contained && !f.no_select
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::client::selection_xlat::tests::Host;

    /// The meta cases' answers on the selection test host (object n is drawable n, at (n, 0, 0)).
    #[derive(Default)]
    pub(crate) struct MetaFacts {
        pub immobile: Vec<ObjectId>,
        pub objects: Option<Vec<ObjectId>>,
        pub costs: Vec<(ObjectId, i32)>,
        pub radar: Option<Coord3D>,
        pub looks: Vec<Coord3D>,
        pub multiplayer: bool,
        pub cheers: u32,
        /// (#136) `ToggleControlBar` calls.
        pub bar_toggles: u32,
    }

    impl CommandMetaHost for Host {
        fn object_is_mobile(&self, obj: ObjectId) -> bool {
            !self.meta.immobile.contains(&obj)
        }
        fn object_is_effectively_dead(&self, _obj: ObjectId) -> bool {
            false
        }
        fn drawable_is_kind_of(&self, draw: DrawableId, k: KindOfType) -> bool {
            crate::client::selection_info::SelectionInfoHost::is_kind_of(self, draw, k)
        }
        fn drawable_position(&self, draw: DrawableId) -> Coord3D {
            Coord3D { x: draw as f32, y: 0.0, z: 0.0 }
        }
        fn local_player_objects(&mut self) -> Option<Vec<ObjectId>> {
            self.meta.objects.clone()
        }
        fn object_cost_to_build(&mut self, obj: ObjectId) -> i32 {
            self.meta.costs.iter().find(|c| c.0 == obj).map_or(0, |c| c.1)
        }
        fn radar_last_event_loc(&self) -> Option<Coord3D> {
            self.meta.radar
        }
        fn look_at_position(&mut self, pos: &Coord3D) {
            self.meta.looks.push(*pos);
        }
        fn is_in_multiplayer_game(&self) -> bool {
            self.meta.multiplayer
        }
        fn is_in_replay_game(&self) -> bool {
            false
        }
        fn local_player_active(&self) -> bool {
            true
        }
        fn beacons_allowed_by_game_info(&self) -> bool {
            true
        }
        fn local_beacon_count(&mut self) -> i32 {
            0
        }
        fn max_beacons_per_player(&self) -> i32 {
            3
        }
        fn place_beacon_button(&mut self) -> Option<GuiButton> {
            None
        }
        fn play_all_cheer_sound(&mut self) {
            self.meta.cheers += 1;
        }
    }
    impl CameraMetaHost for Host {
        fn reset_camera(&mut self) {}
    }
    impl crate::client::control_bar_stage::ToggleControlBarHost for Host {
        fn shell_is_active(&self) -> bool {
            false
        }
        fn shell_top_is_hidden(&self) -> Option<bool> {
            None
        }
        fn shell_top_hide(&mut self, _hide: bool) {}
        fn toggle_bar_recorder_playback(&self) -> bool {
            false
        }
        fn toggle_control_bar(&mut self, _immediate: bool) {
            self.meta.bar_toggles += 1;
        }
    }

    /// Predicted before the run: with drawables 1..=5 (all mine, 3 immobile), nothing selected, the next unit is the
    /// last (5), the next again 4, then 2 (3 is immobile); the previous unit with nothing selected checks only the
    /// first (1); scatter, stop and the formation send their orders; the radar look goes to the last event; the cheer is
    /// kept outside a multiplayer game; the held modes are kept; camera tracking looks at the first selected drawable and
    /// ends with nothing selected; with the input off a meta message is destroyed and does nothing.
    #[test]
    fn the_meta_keys_select_look_and_order() {
        let x = CommandTranslator::default();
        let mut h = Host::default();
        h.all = vec![1, 2, 3, 4, 5];
        h.mine = vec![1, 2, 3, 4, 5];
        h.meta.immobile = vec![3];
        let mut ui = InGameUiSelection::new();
        assert_eq!(x.meta(MSG_META_SELECT_NEXT_UNIT, &mut ui, &mut h), Some(Disposition::Destroy));
        assert_eq!(ui.all_selected_drawables(), &[5]);
        assert_eq!(h.meta.looks, vec![Coord3D { x: 5.0, y: 0.0, z: 0.0 }]);
        assert_eq!(h.messages.last(), Some(&(MSG_CREATE_SELECTED_GROUP, Some(true), vec![5])));
        x.meta(MSG_META_SELECT_NEXT_UNIT, &mut ui, &mut h);
        assert_eq!(ui.all_selected_drawables(), &[4]);
        x.meta(MSG_META_SELECT_NEXT_UNIT, &mut ui, &mut h);
        assert_eq!(ui.all_selected_drawables(), &[2]);
        let mut empty = InGameUiSelection::new();
        x.meta(MSG_META_SELECT_PREV_UNIT, &mut empty, &mut h);
        assert_eq!(empty.all_selected_drawables(), &[1]);
        h.cmd.commands.clear();
        for t in [MSG_META_SCATTER, MSG_META_STOP, MSG_META_CREATE_FORMATION] {
            assert_eq!(x.meta(t, &mut ui, &mut h), Some(Disposition::Destroy));
        }
        let sent: Vec<MessageType> = h.cmd.commands.iter().map(|c| c.0).collect();
        assert_eq!(sent, vec![MSG_DO_SCATTER, MSG_DO_STOP, MSG_CREATE_FORMATION]);
        h.meta.radar = Some(Coord3D { x: 7.0, y: 8.0, z: 0.0 });
        x.meta(MSG_META_VIEW_LAST_RADAR_EVENT, &mut ui, &mut h);
        assert_eq!(h.meta.looks.last(), h.meta.radar.as_ref());
        assert_eq!(x.meta(MSG_META_ALL_CHEER, &mut ui, &mut h), Some(Disposition::Keep));
        h.meta.multiplayer = true;
        assert_eq!(x.meta(MSG_META_ALL_CHEER, &mut ui, &mut h), Some(Disposition::Destroy));
        assert_eq!(h.meta.cheers, 1);
        // (#136) the control bar's toggle: `ToggleControlBar()` once, destroyed
        assert_eq!(x.meta(MSG_META_TOGGLE_CONTROL_BAR, &mut ui, &mut h), Some(Disposition::Destroy));
        assert_eq!(h.meta.bar_toggles, 1);
        assert_eq!(x.meta(MSG_META_BEGIN_FORCEMOVE, &mut ui, &mut h), Some(Disposition::Keep));
        assert!(ui.is_in_force_move_to_mode());
        x.meta(MSG_META_TOGGLE_CAMERA_TRACKING_DRAWABLE, &mut ui, &mut h);
        assert_eq!(camera_tracking_look_at(&mut ui, &h), Some(Coord3D { x: 2.0, y: 0.0, z: 0.0 }));
        ui.set_input_enabled(false);
        h.cmd.commands.clear();
        assert_eq!(x.meta(MSG_META_STOP, &mut ui, &mut h), Some(Disposition::Destroy));
        assert!(h.cmd.commands.is_empty());
        assert_eq!(x.meta(MSG_DO_MOVETO, &mut ui, &mut h), None);
        ui.set_input_enabled(true);
        ui.deselect_all_drawables(true, &mut h);
        assert_eq!(camera_tracking_look_at(&mut ui, &h), None);
        assert!(!ui.is_camera_tracking_drawable());
    }
}
