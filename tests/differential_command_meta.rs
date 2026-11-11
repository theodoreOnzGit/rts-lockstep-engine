#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/CommandXlat.cpp (translateGameMessage's meta-key and
//     selected-group cases, viewCommandCenter, iNeedAHero)
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (kindOfUnitSelection, similarUnitSelection,
//     selectAllUnitsByType*, selectMatching*, selectUnitsMatchingCurrentSelection)
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/MetaEvent.cpp (GameMessageMetaTypeNames,
//     MetaEventTranslator::translateGameMessage's key part, MetaMap::getMetaMapRec)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! GitHub #125: `client::command_xlat_meta` (the command translator's meta-key cases) and `client::meta_event` (the key
//! map and `MetaEventTranslator`'s key part) against the C++.
//!
//! # Method
//!
//! The C++ side is the upstream text cut verbatim by build.rs (`differential/wrap_command_meta.cpp` lists the cuts),
//! compiled on stand-ins for the client's drawable list, the objects, the InGameUI's selection and modes, the tactical
//! view, the radar, the players, the game logic, the audio and the stream, with the real `GameMessage`. Three families:
//!
//! - [`meta_cases_match_upstream`]: one meta message (the 28 types `CommandTranslator::meta` has cases for, the
//!   selection walks weighted up) through `translateGameMessage`; each message it appends then goes through the
//!   translator too, for `m_teamExists`. Random: 0 to 8 drawables in the list, each with or without an object (a third
//!   of the time none past the first), 8 kinds (structure, aircraft, dozer, harvester, command center, hero, ignores
//!   select all, no select), mobile, locally controlled, contained by another, dead, mass-selectable, off the map, on
//!   the screen, one of 4 templates, a car bomb; 0 to 4 selected (each with an object), the local player present and its
//!   objects in a random order, the selection cap (none, 1, 2 or 3) and its warning, the radar's last event, the
//!   multiplayer, replay, active, net-min-players, game-info and beacon facts, the 6 InGameUI flags, `m_teamExists`, the
//!   input on 7 times in 8. Compared: the disposition, every appended message with its arguments, every `lookAt`
//!   position's bits, the selection after (front first), the UI messages (label, count), the sounds (the template's
//!   select voice, the cheer), the `setGUICommand` calls, the flags and select count after, and `m_teamExists` after.
//!   Not generated (upstream crashes, see `command_xlat_meta`'s docs): a selected drawable without an object, a list of
//!   one selected drawable for the next walks, a beacon case without a local player.
//! - [`meta_event_keys_match_upstream`]: one `MSG_RAW_KEY_DOWN` / `UP` through `MetaEventTranslator::translateGameMessage`
//!   on 0 to 12 records made with `getMetaMapRec` (repeats reuse a record), each a meta of the release names, one of 6
//!   keys or `MK_NONE`, a transition (with `DOUBLEDOWN`), one of the 8 modifier states, usable in none / shell / game /
//!   both; the key one of those 6 or a modifier key, its state up or down with random modifier and auto-repeat bits,
//!   the client frame 0 or 1, the shell, the replay, the last modifiers, the TiVO mode. Compared: the disposition, the
//!   metas appended, the last modifiers and key after, the TiVO mode, the message count, the records' list order.
//! - [`meta_names_match_upstream`]: every entry of `GameMessageMetaTypeNames` (name and value) against
//!   `meta_event::GAME_MESSAGE_META_TYPE_NAMES`.
//!
//! *Predictions, before the first run* (`issue125_progress.md`): 0 mismatches in the three families; every plant 9121-9145
//! caught.
//!
//! # Results (2026-11-11, `COMMAND_META_CASES=20000`, the default)
//!
//! - Meta cases: 20,000, **0 mismatches**. 3,062 looked somewhere, 1,496 were selection walks that selected, 3,275 gave a
//!   UI message, 4,457 were kept. **The first run hung** after case 432: upstream's next walk never ends when the first
//!   selected drawable is the list's last and no drawable qualifies (see `command_xlat_meta`'s docs). The Rust ends the
//!   walk on a second wrap; the generator no longer makes such a case (it drops that drawable from the selection).
//! - Keys: 20,000, **0 mismatches**; 3,996 made a meta, 230 were eaten without one (auto-repeats), 101 toggled the
//!   replay's fast forward. The first generator (keys and modifiers at random) made a meta in only 1,102 cases; half the
//!   cases are now aimed at a record.
//! - Names: 104 entries and the end, **0 mismatches**.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints `MEASURED` lines and fails on purpose): **25 of 25 caught** (2026-11-11).
//!
//! | id | planted bug | caught by |
//! |---|---|---|
//! | 9121 | the next walk without the `hack` re-step after the wrap | 116 meta cases |
//! | 9122 | the previous walk with nothing selected walks the whole list (upstream checks only the first) | 87 |
//! | 9123 | the previous walk's wrap case without `getNextDrawable() == selected` | 42 |
//! | 9124 | the next worker with nothing selected needs `isMobile` too | 21 |
//! | 9125 | the next worker plays no voice | 92 |
//! | 9126 | the hero not swapped for its container | 25 |
//! | 9127 | `viewCommandCenter` keeps the first command center (as its comment says; upstream keeps the last) | 57 |
//! | 9128 | `viewCommandCenter` counts a non-structure before a command center | 176 |
//! | 9129 | the last radar event not looked at | 762 |
//! | 9130 | select all without deselecting an incompatible selection (the SCUD storm fix) | 1,479 |
//! | 9131 | `kindOfUnitSelection` without the dead test | 93 |
//! | 9132 | `selectAllUnitsByType`'s group message always "new group" | 32 |
//! | 9133 | `selectAllUnitsByType` across the map also when the screen selected some | 185 |
//! | 9134 | scatter sends `MSG_DO_STOP` | 324 |
//! | 9135 | deploy kept | 334 |
//! | 9136 | the beacon without its cap | 71 |
//! | 9137 | remove beacon in a replay too | 46 |
//! | 9138 | toggle attack-move destroys the message | 304 |
//! | 9139 | the cheer outside a multiplayer game | 99 |
//! | 9140 | end waypoints sets waypoint mode | 306 |
//! | 9141 | an auto-repeat emits its meta | 230 key cases |
//! | 9142 | the modifiers-only rule compares the new modifiers on `UP` | 1,878 |
//! | 9143 | game-only commands at client frame 0 | 532 |
//! | 9144 | `getMetaMapRec` appends a new record (upstream prepends) | 16,834 |
//! | 9145 | matching units also across the map when the screen selected some | 40 meta cases |

mod support;

use rts_lockstep_engine::client::command_xlat::{input_disabled_destroys, ActionType, CmdArg, CommandTranslator, CommandXlatHost, GuiButton, TargetMode};
use rts_lockstep_engine::client::command_xlat_meta::CommandMetaHost;
use rts_lockstep_engine::client::control_bar::{gui, opt};
use rts_lockstep_engine::client::in_game_ui::{DrawableId, InGameUiSelection, SelectionUiHost};
use rts_lockstep_engine::client::meta_event::{MetaEventTranslator, MetaMap, KeyContext, GAME_MESSAGE_META_TYPE_NAMES};
use rts_lockstep_engine::client::selection_groups::SelectionGroupsHost;
use rts_lockstep_engine::client::selection_info::SelectionInfoHost;
use rts_lockstep_engine::client::selection_xlat::{Disposition, SelectionXlatHost};
use rts_lockstep_engine::client::unit_voice::{UnitVoice, UnitVoiceHost};
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::message_stream::MessageType;
use rts_lockstep_engine::common::message_types::*;
use rts_lockstep_engine::common::special_power_type::SpecialPowerType;
use rts_lockstep_engine::common::thing_template::ThingTemplateAudioType;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::ai::ai_update_types::{CanAttackResultValue, ATTACKRESULT_NOT_POSSIBLE};
use support::{Check, Gen};

fn cpp_case(kind: i32, input: &[i32]) -> Vec<i32> {
    // (#126: the wrapper's state is global and the tests run on several threads)
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _held = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = vec![0i32; 4096];
    let n = unsafe { cpp::ref_command_meta_case(kind, input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn disp(d: Disposition) -> i32 {
    match d {
        Disposition::Keep => 0,
        Disposition::Destroy => 1,
    }
}

// The C++ wrapper's layout (`META_*`, `DRAW_*`).
const META_TYPE: usize = 0;
const META_INPUT: usize = 1;
const META_NDRAW: usize = 2;
const META_LOCAL: usize = 3;
const META_MAXSEL: usize = 4;
const META_MAXWARN: usize = 5;
const META_RADAR: usize = 6;
const META_RADAR_X: usize = 7;
const META_MP: usize = 10;
const META_REPLAY: usize = 11;
const META_ACTIVE: usize = 12;
const META_NETMIN: usize = 13;
const META_GAMEINFO_MP: usize = 14;
const META_BEACONS: usize = 15;
const META_MAXBEACONS: usize = 16;
const META_BEACON_BUTTON: usize = 17;
const META_ATTACKMOVE: usize = 18;
const META_TEAM: usize = 24;
const META_NSEL: usize = 25;
const META_SEL0: usize = 26;
const META_NOBJ: usize = META_SEL0 + 8;
const META_OBJ0: usize = META_NOBJ + 1;
const META_HEADER: usize = 48;
const DRAW_WORDS: usize = 12;
const META_COSTS: usize = META_HEADER + 8 * DRAW_WORDS;
const KINDS: [KindOfType; 8] = [KINDOF_STRUCTURE, KINDOF_AIRCRAFT, KINDOF_DOZER, KINDOF_HARVESTER, KINDOF_COMMANDCENTER, KINDOF_HERO, KINDOF_IGNORES_SELECT_ALL, KINDOF_NO_SELECT];

/// One drawable of a case (drawable n is object n when it has one).
#[derive(Clone, Copy, Debug, Default)]
struct Draw {
    has_obj: bool,
    kinds: u32,
    mobile: bool,
    local: bool,
    container: u32,
    dead: bool,
    mass: bool,
    off_map: bool,
    screen: bool,
    template: u32,
    car_bomb: bool,
}

/// The meta cases' host: the case's facts, and the calls in the C++ stand-ins' words.
#[derive(Default)]
struct MetaHost {
    draws: Vec<Draw>,
    local: bool,
    objects: Vec<ObjectId>,
    costs: [i32; 4],
    radar: Option<Coord3D>,
    mp: bool,
    replay: bool,
    active: bool,
    net_min: i32,
    game_info_mp: bool,
    beacons: i32,
    max_beacons: i32,
    beacon_button: bool,
    msgs: Vec<i32>,
    looks: Vec<i32>,
    ui_msgs: Vec<i32>,
    audio: Vec<i32>,
    gui: Vec<i32>,
    sent: Vec<MessageType>,
}

impl MetaHost {
    fn d(&self, id: DrawableId) -> Option<&Draw> {
        self.draws.get((id as usize).wrapping_sub(1))
    }
    fn log(&mut self, t: MessageType, args: &[CmdArg]) {
        // (`GameMessageArgumentDataType`: 0 integer, 2 boolean, 3 object id, 6 location)
        self.sent.push(t);
        self.msgs.extend([t.0 as i32, args.len() as i32]);
        for a in args {
            match *a {
                CmdArg::Int(i) => self.msgs.extend([0, i]),
                CmdArg::Bool(b) => self.msgs.extend([2, i32::from(b)]),
                CmdArg::Object(o) => self.msgs.extend([3, o as i32]),
                CmdArg::Location(l) => self.msgs.extend([6, l.x.to_bits() as i32, l.y.to_bits() as i32, l.z.to_bits() as i32]),
                CmdArg::Real(_) => self.msgs.extend([99, 0]),
            }
        }
    }
}

impl SelectionUiHost for MetaHost {
    fn logic_frame(&self) -> u32 {
        0
    }
    fn append_message_bool(&mut self, t: MessageType, b: bool) {
        self.log(t, &[CmdArg::Bool(b)]);
    }
    fn on_selection_changed(&mut self, _draw: DrawableId, _selected: bool) {}
}
impl SelectionInfoHost for MetaHost {
    fn drawable_object(&self, draw: DrawableId) -> Option<ObjectId> {
        self.d(draw).filter(|d| d.has_obj).map(|_| draw)
    }
    fn is_locally_controlled(&self, obj: ObjectId) -> bool {
        self.d(obj).is_some_and(|d| d.local)
    }
    fn is_kind_of(&self, obj: ObjectId, k: KindOfType) -> bool {
        self.d(obj).is_some_and(|d| KINDS.iter().position(|&x| x == k).is_some_and(|b| (d.kinds >> b) & 1 != 0))
    }
    fn relationship_to_local_player(&self, _obj: ObjectId) -> Relationship {
        Relationship::Neutral
    }
    fn can_local_player_garrison(&mut self, _obj: ObjectId) -> bool {
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
impl SelectionXlatHost for MetaHost {
    fn quit_menu_visible(&self) -> bool {
        false
    }
    fn gui_command_pending(&self) -> bool {
        false
    }
    fn matches_pick_kinds(&self, _draw: DrawableId) -> bool {
        false
    }
    fn is_selectable(&self, _draw: DrawableId) -> bool {
        true
    }
    fn contained_by(&self, obj: ObjectId) -> Option<ObjectId> {
        self.d(obj).map(|d| d.container as ObjectId).filter(|&c| c != 0)
    }
    fn is_enclosing_container_for(&mut self, _container: ObjectId, _obj: ObjectId) -> Option<bool> {
        None
    }
    fn object_drawable(&self, obj: ObjectId) -> Option<DrawableId> {
        Some(obj)
    }
    fn fogged_for_local_player(&self, _obj: ObjectId) -> bool {
        false
    }
    fn stealthed_and_undetected(&self, _obj: ObjectId) -> bool {
        false
    }
    fn append_message(&mut self, t: MessageType, create: Option<bool>, objects: &[ObjectId]) {
        let mut args: Vec<CmdArg> = create.map(CmdArg::Bool).into_iter().collect();
        args.extend(objects.iter().map(|&o| CmdArg::Object(o)));
        self.log(t, &args);
    }
    fn record_drag_selection(&mut self) {}
    fn clear_attack_move_to_mode(&mut self) {}
}
impl SelectionGroupsHost for MetaHost {
    fn all_drawables(&self) -> Vec<DrawableId> {
        (1..=self.draws.len() as ObjectId).collect()
    }
    fn drawables_on_screen(&self) -> Vec<DrawableId> {
        (1..=self.draws.len() as ObjectId).filter(|&i| self.d(i).is_some_and(|d| d.screen)).collect()
    }
    fn pick_selectable_at_click(&self) -> Option<DrawableId> {
        None
    }
    fn is_mass_selectable(&self, obj: ObjectId) -> bool {
        self.d(obj).is_some_and(|d| d.mass)
    }
    fn object_template(&self, obj: ObjectId) -> u32 {
        self.d(obj).map_or(0, |d| d.template)
    }
    fn is_equivalent_to(&self, obj: ObjectId, template: u32) -> bool {
        self.object_template(obj) == template
    }
    fn is_car_bomb(&self, obj: ObjectId) -> bool {
        self.d(obj).is_some_and(|d| d.car_bomb)
    }
    fn is_off_map(&self, obj: ObjectId) -> bool {
        self.d(obj).is_some_and(|d| d.off_map)
    }
    fn hotkey_squad_live_objects(&mut self, _group: i32) -> Option<Vec<ObjectId>> {
        None
    }
    fn look_at(&mut self, _obj: ObjectId) {}
    fn ui_message(&mut self, label: &[u8], count: Option<i32>) {
        let code = match label {
            b"GUI:NothingSelected" => 1,
            b"GUI:SelectedAcrossScreen" => 2,
            b"GUI:SelectedAcrossMap" => 3,
            b"GUI:MaxSelectionSize" => 4,
            _ => 7,
        };
        self.ui_msgs.extend([code, count.unwrap_or(0)]);
    }
    fn append_message_drawable(&mut self, _t: MessageType, _draw: DrawableId) {}
}
impl UnitVoiceHost for MetaHost {
    fn voice_template_name(&self, obj: ObjectId) -> Vec<u8> {
        vec![b'0' + self.object_template(obj) as u8]
    }
    fn template_audio(&self, obj: ObjectId, _t: ThingTemplateAudioType) -> Vec<u8> {
        // (the stand-in's select voice of template k has the tag 100 + k)
        (100 + self.object_template(obj)).to_string().into_bytes()
    }
    fn play_unit_voice(&mut self, v: UnitVoice) {
        let tag: i32 = String::from_utf8_lossy(&v.name).parse().unwrap_or(-99);
        self.audio.extend([tag, v.object as i32]);
    }
}
impl CommandXlatHost for MetaHost {
    fn object_is_masked(&self, _obj: ObjectId) -> bool {
        false
    }
    fn object_position(&self, obj: ObjectId) -> Coord3D {
        Coord3D { x: obj as f32 * 7.0 + 1.25, y: obj as f32 * -2.0, z: 0.5 }
    }
    fn object_relationship(&self, _src: ObjectId, _target: ObjectId) -> Relationship {
        Relationship::Neutral
    }
    fn object_can_do_action(&mut self, _src: Option<ObjectId>, _action: ActionType, _target: Option<ObjectId>, _additional: bool) -> bool {
        false
    }
    fn object_can_attack(&mut self, _src: Option<ObjectId>, _target: Option<ObjectId>, _forced: bool) -> CanAttackResultValue {
        ATTACKRESULT_NOT_POSSIBLE
    }
    fn object_can_force_attack(&mut self, _src: ObjectId, _victim: Option<ObjectId>, _pos: Option<Coord3D>) -> CanAttackResultValue {
        ATTACKRESULT_NOT_POSSIBLE
    }
    fn object_can_override_special_power_destination(&mut self, _src: Option<ObjectId>, _loc: &Coord3D, _sp: SpecialPowerType) -> bool {
        false
    }
    fn is_salvage_crate(&mut self, _obj: ObjectId) -> bool {
        false
    }
    fn shroud_clear_for_local_player(&mut self, _pos: &Coord3D) -> bool {
        false
    }
    fn object_has_ai(&self, _obj: ObjectId) -> bool {
        false
    }
    fn quick_path_available(&mut self, _obj: ObjectId, _pos: &Coord3D) -> bool {
        false
    }
    fn has_cliff_locomotor(&mut self, _obj: ObjectId) -> bool {
        false
    }
    fn is_cliff_cell(&mut self, _x: f32, _y: f32) -> bool {
        false
    }
    fn special_power_buttons(&mut self, _obj: ObjectId) -> Vec<GuiButton> {
        Vec::new()
    }
    fn object_can_do_special_power(&mut self, _src: Option<ObjectId>, _button: &GuiButton, _mode: TargetMode) -> bool {
        false
    }
    fn object_can_fire_weapon(&mut self, _src: Option<ObjectId>, _button: &GuiButton, _mode: TargetMode) -> bool {
        false
    }
    fn most_ready_shortcut_special_power(&mut self, _sp: SpecialPowerType) -> Option<ObjectId> {
        None
    }
    fn has_overridable_destination_special_power(&mut self, _obj: ObjectId) -> bool {
        false
    }
    fn append_command(&mut self, t: MessageType, args: &[CmdArg]) {
        self.log(t, args);
    }
    fn append_hint(&mut self, _t: MessageType, _args: &[CmdArg]) {}
    fn double_click_attack_move(&self) -> bool {
        false
    }
    fn record_double_click_attack_move_order_given(&mut self) {}
}
impl rts_lockstep_engine::client::command_xlat::MouseClickHost for MetaHost {
    fn mouse_is_click(&self, _anchor: &rts_lockstep_engine::common::base_type::ICoord2D, _dest: &rts_lockstep_engine::common::base_type::ICoord2D, _previous: u32, _current: u32) -> bool {
        true
    }
}
impl CommandMetaHost for MetaHost {
    fn object_is_mobile(&self, obj: ObjectId) -> bool {
        self.d(obj).is_some_and(|d| d.mobile)
    }
    fn object_is_effectively_dead(&self, obj: ObjectId) -> bool {
        self.d(obj).is_some_and(|d| d.dead)
    }
    fn drawable_is_kind_of(&self, draw: DrawableId, k: KindOfType) -> bool {
        self.is_kind_of(draw, k)
    }
    fn drawable_position(&self, draw: DrawableId) -> Coord3D {
        Coord3D { x: draw as f32 * 10.0 + 0.5, y: draw as f32 * 3.0 - 2.0, z: draw as f32 * 0.25 }
    }
    fn local_player_objects(&mut self) -> Option<Vec<ObjectId>> {
        self.local.then(|| self.objects.clone())
    }
    fn object_cost_to_build(&mut self, obj: ObjectId) -> i32 {
        self.costs[self.object_template(obj) as usize]
    }
    fn radar_last_event_loc(&self) -> Option<Coord3D> {
        self.radar
    }
    fn look_at_position(&mut self, pos: &Coord3D) {
        self.looks.extend([pos.x.to_bits() as i32, pos.y.to_bits() as i32, pos.z.to_bits() as i32]);
    }
    fn is_in_multiplayer_game(&self) -> bool {
        self.mp
    }
    fn is_in_replay_game(&self) -> bool {
        self.replay
    }
    fn local_player_active(&self) -> bool {
        self.active
    }
    fn beacons_allowed_by_game_info(&self) -> bool {
        self.net_min == 0 || self.game_info_mp
    }
    fn local_beacon_count(&mut self) -> i32 {
        self.beacons
    }
    fn max_beacons_per_player(&self) -> i32 {
        self.max_beacons
    }
    fn place_beacon_button(&mut self) -> Option<GuiButton> {
        self.gui.push(if self.beacon_button { gui::PLACE_BEACON } else { -2 });
        self.beacon_button.then(|| GuiButton { command_type: gui::PLACE_BEACON, options: opt::NEED_TARGET_POS, ..Default::default() })
    }
    fn play_all_cheer_sound(&mut self) {
        self.audio.extend([-7, 0]);
    }
}
impl rts_lockstep_engine::client::command_xlat_meta::CameraMetaHost for MetaHost {
    fn reset_camera(&mut self) {}
}

const META_TYPES: [MessageType; 28] = [
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
];

fn flag(g: &mut Gen) -> bool {
    g.below(2) == 1
}

/// One meta case: its input words for the C++ and the Rust's output.
fn meta_case(g: &mut Gen, stats: &mut [u64; 4]) -> (Vec<i32>, Vec<i32>) {
    // the walks, the hero, select all and matching weighted up
    let t = if g.below(2) == 0 { META_TYPES[g.below(10) as usize] } else { META_TYPES[g.below(28) as usize] };
    let walk = (MSG_META_SELECT_NEXT_UNIT.0..=MSG_META_SELECT_PREV_WORKER.0).contains(&t.0);
    let next_walk = t == MSG_META_SELECT_NEXT_UNIT || t == MSG_META_SELECT_NEXT_WORKER;
    let mut input = vec![0i32; META_COSTS + 4];
    let on = g.below(8) != 0;
    let n = if g.below(10) == 0 { 0 } else { 1 + g.below(8) as usize };
    let mut draws = vec![Draw::default(); n];
    for (i, d) in draws.iter_mut().enumerate() {
        d.has_obj = i == 0 || g.below(3) != 0;
        // a kind each a quarter of the time, structure and dozer and hero more often
        for b in 0..8 {
            let p = if b == 0 || b == 2 || b == 5 { 3 } else { 5 };
            if g.below(p) == 0 {
                d.kinds |= 1 << b;
            }
        }
        d.mobile = g.below(4) != 0;
        d.local = g.below(4) != 0;
        d.dead = g.below(6) == 0;
        d.mass = g.below(4) != 0;
        d.off_map = g.below(8) == 0;
        d.screen = flag(g);
        d.template = g.below(4);
        d.car_bomb = g.below(8) == 0;
    }
    // containers: another drawable with an object, a sixth of the time
    for i in 0..n {
        if g.below(6) == 0 {
            let c = g.below(n as u32) as usize;
            if c != i && draws[c].has_obj && draws[c].container == 0 {
                draws[i].container = c as u32 + 1;
            }
        }
    }
    // the selection: drawables with objects; the next walks need more than one drawable to select from
    let with_obj: Vec<usize> = (0..n).filter(|&i| draws[i].has_obj).collect();
    let mut sel: Vec<usize> = Vec::new();
    if !with_obj.is_empty() && !(next_walk && n == 1) {
        for _ in 0..g.below(5) {
            let i = with_obj[g.below(with_obj.len() as u32) as usize];
            if !sel.contains(&i) {
                sel.push(i);
            }
        }
    }
    // (upstream's next walk never ends when the first selected drawable, locally controlled, is the list's last and no
    // drawable qualifies: such a case is not generated; the last drawable is taken out of the selection)
    if next_walk {
        let worker = t == MSG_META_SELECT_NEXT_WORKER;
        let qualifies = |d: &Draw| {
            let has = |b: u32| (d.kinds >> b) & 1 != 0;
            d.has_obj && d.local && d.container == 0 && if worker { has(2) } else { d.mobile && !has(7) }
        };
        if sel.last() == Some(&(n - 1)) && draws[n - 1].local && !draws.iter().any(qualifies) {
            sel.pop();
        }
    }
    let local = t == MSG_META_PLACE_BEACON || g.below(8) != 0;
    let mut objects: Vec<usize> = with_obj.clone();
    for i in (1..objects.len()).rev() {
        let j = g.below(i as u32 + 1) as usize;
        objects.swap(i, j);
    }
    objects.truncate(g.below(objects.len() as u32 + 1) as usize);
    let maxsel = [-1, 0, 1, 2, 3][g.below(5) as usize];
    let maxwarn = flag(g);
    let radar = (g.below(3) != 0).then(|| Coord3D { x: (g.below(4001) as f32 - 2000.0) / 4.0, y: (g.below(4001) as f32) / 8.0, z: g.below(17) as f32 / 2.0 });
    let (mp, replay, active) = (g.below(3) != 0, g.below(4) == 0, g.below(5) != 0);
    let net_min = g.below(3) as i32;
    let game_info_mp = flag(g);
    let beacons = g.below(5) as i32;
    let max_beacons = g.below(5) as i32;
    let beacon_button = g.below(5) != 0;
    let flags: Vec<bool> = (0..6).map(|_| flag(g)).collect();
    let team = flag(g);
    let costs: [i32; 4] = [g.below(2000) as i32, g.below(2000) as i32, g.below(2000) as i32, g.below(2000) as i32 - 1000];

    input[META_TYPE] = t.0 as i32;
    input[META_INPUT] = i32::from(on);
    input[META_NDRAW] = n as i32;
    input[META_LOCAL] = i32::from(local);
    input[META_MAXSEL] = maxsel;
    input[META_MAXWARN] = i32::from(maxwarn);
    if let Some(r) = radar {
        input[META_RADAR] = 1;
        input[META_RADAR_X] = r.x.to_bits() as i32;
        input[META_RADAR_X + 1] = r.y.to_bits() as i32;
        input[META_RADAR_X + 2] = r.z.to_bits() as i32;
    }
    input[META_MP] = i32::from(mp);
    input[META_REPLAY] = i32::from(replay);
    input[META_ACTIVE] = i32::from(active);
    input[META_NETMIN] = net_min;
    input[META_GAMEINFO_MP] = i32::from(game_info_mp);
    input[META_BEACONS] = beacons;
    input[META_MAXBEACONS] = max_beacons;
    input[META_BEACON_BUTTON] = i32::from(beacon_button);
    for (k, &f) in flags.iter().enumerate() {
        input[META_ATTACKMOVE + k] = i32::from(f);
    }
    input[META_TEAM] = i32::from(team);
    input[META_NSEL] = sel.len() as i32;
    for (k, &s) in sel.iter().enumerate() {
        input[META_SEL0 + k] = s as i32 + 1;
    }
    input[META_NOBJ] = objects.len() as i32;
    for (k, &o) in objects.iter().enumerate() {
        input[META_OBJ0 + k] = o as i32 + 1;
    }
    for (i, d) in draws.iter().enumerate() {
        let w = &mut input[META_HEADER + i * DRAW_WORDS..META_HEADER + (i + 1) * DRAW_WORDS];
        w.copy_from_slice(&[
            i32::from(d.has_obj),
            d.kinds as i32,
            i32::from(d.mobile),
            i32::from(d.local),
            d.container as i32,
            i32::from(d.dead),
            i32::from(d.mass),
            i32::from(d.off_map),
            i32::from(d.screen),
            d.template as i32,
            i32::from(d.car_bomb),
            0,
        ]);
    }
    input[META_COSTS..META_COSTS + 4].copy_from_slice(&costs);

    let mut h = MetaHost {
        draws,
        local,
        objects: objects.iter().map(|&o| o as ObjectId + 1).collect(),
        costs,
        radar,
        mp,
        replay,
        active,
        net_min,
        game_info_mp,
        beacons,
        max_beacons,
        beacon_button,
        ..Default::default()
    };
    let mut ui = InGameUiSelection::new();
    // (first: turning the input off clears the held modes, which the case sets after)
    ui.set_input_enabled(on);
    ui.set_max_select_count(maxsel);
    ui.set_displayed_max_warning(maxwarn);
    for &s in &sel {
        ui.select_drawable(s as DrawableId + 1, &mut h);
    }
    ui.set_attack_move_to_mode(flags[0]);
    ui.set_force_attack_mode(flags[1]);
    ui.set_force_move_mode(flags[2]);
    ui.set_waypoint_mode(flags[3]);
    ui.set_prefer_selection_mode(flags[4]);
    ui.set_camera_tracking_drawable(flags[5]);
    let mut x = CommandTranslator { team_exists: team, ..Default::default() };
    let d = x.meta(t, &mut ui, &mut h).expect("a meta case");
    // the appended messages through the translator: its gate, then the selected-group cases
    for m in std::mem::take(&mut h.sent) {
        if !input_disabled_destroys(m, ui.input_enabled()) {
            x.observe_message(m);
        }
    }
    stats[0] += u64::from(!h.looks.is_empty());
    stats[1] += u64::from(walk && !h.msgs.is_empty());
    stats[2] += u64::from(!h.ui_msgs.is_empty());
    stats[3] += u64::from(d == Disposition::Keep);
    let mut rust = vec![disp(d), -1];
    rust.extend(&h.msgs);
    rust.push(-2);
    rust.extend(&h.looks);
    rust.push(-3);
    rust.extend(ui.all_selected_drawables().iter().map(|&d| d as i32));
    rust.push(-4);
    rust.extend(&h.ui_msgs);
    rust.push(-5);
    rust.extend(&h.audio);
    rust.push(-6);
    rust.extend(&h.gui);
    rust.push(-7);
    rust.extend([
        i32::from(ui.is_in_attack_move_to_mode()),
        i32::from(ui.is_in_force_attack_mode()),
        i32::from(ui.is_in_force_move_to_mode()),
        i32::from(ui.is_in_waypoint_mode()),
        i32::from(ui.is_in_prefer_selection_mode()),
        i32::from(ui.is_camera_tracking_drawable()),
        i32::from(ui.displayed_max_warning()),
        ui.select_count(),
        -8,
        i32::from(x.team_exists),
    ]);
    (input, rust)
}

/// The meta cases against `translateGameMessage` (see the module docs).
#[test]
fn meta_cases_match_upstream() {
    let cases: u64 = std::env::var("COMMAND_META_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x125_1250);
    let mut c = Check::new("CommandTranslator meta cases");
    let mut stats = [0u64; 4];
    for i in 0..cases {
        let (input, rust) = meta_case(&mut g, &mut stats);
        let t = MessageType(input[META_TYPE] as _);

        c.eq(rust, cpp_case(0, &input), || format!("case {i} {t:?} {input:?}"));
    }
    println!("meta cases: {} looked, {} walks that selected, {} with UI messages, {} kept", stats[0], stats[1], stats[2], stats[3]);
    if planted {
        c.report();
    } else {
        c.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED lines (a mismatch is the plant caught)");
}

/// One key case: its input words and the Rust's output.
fn key_case(g: &mut Gen, stats: &mut [u64; 3]) -> (Vec<i32>, Vec<i32>) {
    // the release's meta names, the keys a case uses (tab, A, 1, space, F1, up) and the modifier keys
    const KEYS: [i32; 6] = [0x0F, 0x1E, 0x02, 0x39, 0x3B, 0xC8];
    const MODKEYS: [i32; 6] = [0x2A, 0x36, 0x1D, 0x9D, 0x38, 0xB8];
    const MODS: [i32; 8] = [0, 4, 64, 16, 68, 20, 80, 84];
    let nrec = g.below(13) as usize;
    let mut recs = Vec::new();
    for _ in 0..nrec {
        let meta = GAME_MESSAGE_META_TYPE_NAMES[g.below(GAME_MESSAGE_META_TYPE_NAMES.len() as u32) as usize].1;
        // a few metas repeated so records are reused, and the fast-forward one sometimes
        let meta = match g.below(8) {
            0 => MSG_META_SELECT_NEXT_UNIT,
            1 => MSG_META_TOGGLE_FAST_FORWARD_REPLAY,
            _ => meta,
        };
        let key = if g.below(3) == 0 { 0 } else { KEYS[g.below(6) as usize] };
        // usable in the game most of the time
        let usable = if g.below(3) == 0 { g.below(4) as i32 } else { 2 + g.below(2) as i32 };
        recs.push([meta.0 as i32, key, g.below(3) as i32, MODS[g.below(8) as usize], usable]);
    }
    let mut down = flag(g);
    let mut key = if g.below(4) == 0 { MODKEYS[g.below(6) as usize] } else { KEYS[g.below(6) as usize] };
    let mut mods = 0;
    for bit in [4, 8, 0x10, 0x20, 0x40, 0x80, 0x400] {
        if g.below(4) == 0 {
            mods |= bit;
        }
    }
    // half the time aimed at a record: its key (a modifier key for none), its modifiers (either side), its transition
    if !recs.is_empty() && flag(g) {
        let r = recs[g.below(recs.len() as u32) as usize];
        key = if r[1] == 0 { MODKEYS[g.below(6) as usize] } else { r[1] };
        mods = 0;
        for (m, sides) in [(4, [4, 8]), (16, [0x10, 0x20]), (64, [0x40, 0x80])] {
            if r[3] & m != 0 {
                mods |= sides[g.below(2) as usize];
            }
        }
        down = r[2] != 1;
    }
    let mut state = (if down { 2 } else { 1 }) | mods;
    if down && g.below(5) == 0 {
        state |= 0x100;
    }
    let frame = u32::from(g.below(4) != 0);
    let (shell, replay) = (g.below(4) == 0, flag(g));
    let last_mod = MODS[g.below(8) as usize];
    let last_key = KEYS[g.below(6) as usize];
    let tivo = g.below(2) as i32;
    let mut input = vec![if down { MSG_RAW_KEY_DOWN.0 as i32 } else { MSG_RAW_KEY_UP.0 as i32 }, key, state, frame as i32, i32::from(shell), i32::from(replay), last_mod, last_key, tivo, nrec as i32];
    for r in &recs {
        input.extend(r);
    }

    let mut map = MetaMap::new();
    for r in &recs {
        let i = map.get_meta_map_rec(MessageType(r[0] as _));
        let m = &mut map.recs[i];
        m.key = r[1];
        m.transition = r[2];
        m.mod_state = r[3];
        m.usable_in = r[4] as u32;
    }
    let mut x = MetaEventTranslator { last_key_down: last_key, last_mod_state: last_mod, ..Default::default() };
    let ctx = KeyContext { client_frame: frame, shell_active: shell, in_replay: replay };
    let r = x.translate_key(&map, down, key, state, &ctx);
    let new_tivo = if r.fast_forward_toggled { 1 - tivo } else { tivo };
    stats[0] += u64::from(!r.metas.is_empty());
    stats[1] += u64::from(r.disposition == Some(Disposition::Destroy) && r.metas.is_empty());
    stats[2] += u64::from(r.fast_forward_toggled);
    let mut rust = vec![disp(r.disposition.expect("a key")), -1];
    rust.extend(r.metas.iter().map(|t| t.0 as i32));
    rust.extend([-2, x.last_mod_state, x.last_key_down, new_tivo, i32::from(r.fast_forward_toggled), -3]);
    rust.extend(map.records().iter().map(|m| m.meta.0 as i32));
    (input, rust)
}

/// `MetaEventTranslator::translateGameMessage`'s key part and `getMetaMapRec`'s order (see the module docs).
#[test]
fn meta_event_keys_match_upstream() {
    let cases: u64 = std::env::var("COMMAND_META_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x125_2500);
    let mut c = Check::new("MetaEventTranslator keys");
    let mut stats = [0u64; 3];
    for i in 0..cases {
        let (input, rust) = key_case(&mut g, &mut stats);
        c.eq(rust, cpp_case(1, &input), || format!("case {i} {input:?}"));
    }
    println!("key cases: {} made metas, {} eaten without one, {} fast-forward toggles", stats[0], stats[1], stats[2]);
    if planted {
        c.report();
    } else {
        c.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED lines (a mismatch is the plant caught)");
}

/// `GameMessageMetaTypeNames` against the translation's table, entry by entry, and one past the end.
#[test]
fn meta_names_match_upstream() {
    let mut c = Check::new("GameMessageMetaTypeNames");
    for (i, (name, t)) in GAME_MESSAGE_META_TYPE_NAMES.iter().enumerate() {
        let mut rust = vec![t.0 as i32];
        rust.extend(name.bytes().map(i32::from));
        c.eq(rust, cpp_case(2, &[i as i32]), || format!("entry {i}"));
        assert_eq!(MetaMap::find_game_message_meta_type(name.to_ascii_lowercase().as_bytes()), *t);
    }
    c.eq(Vec::<i32>::new(), cpp_case(2, &[GAME_MESSAGE_META_TYPE_NAMES.len() as i32]), || "past the end".into());
    c.finish();
}
