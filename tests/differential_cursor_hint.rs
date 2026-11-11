#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (setMouseCursor, triggerDoubleClickAttackMoveGuardHint,
//     createMouseoverHint, createCommandHint, setGUICommand, placeBuildAvailable)
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/SelectionXlat.cpp (CanSelectDrawable, the
//     MSG_MOUSEOVER_DRAWABLE_HINT case)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Input/Mouse.cpp (getCursorIndex)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::in_game_ui_cursor`, the cursor halves of `InGameUiSelection::set_gui_command` and `place_build_available`,
//! and `selection_xlat`'s `can_select_drawable` and mouse-over case (GitHub #122) against the C++.
//!
//! # Method
//!
//! The C++ side is the functions cut verbatim by build.rs (`differential/wrap_cursor_hint.cpp` lists the cuts and
//! documents the words), compiled on stand-ins that answer from the case's setup, with the calls on `TheMouse`
//! (`setCursor`, `setCursorTooltip`, `resetTooltipDelay`, `capture`, `releaseCapture`) and on the radius cursor
//! recorded in order. The Rust side runs the translation on a host answering from the same words
//! ([`Host`]) and records its queued [`UiMouseCall`]s the same way.
//!
//! Each case is a random game: 1 to 8 drawables (with or without an object; their templates' kinds among selectable,
//! shrubbery, structure, ignored in the GUI, mine, always selectable, force-attackable and disguiser; dead, hidden,
//! under an opaque window, unselectable or masked, not selectable, the local player's or not; every object shroud
//! status; the cell clear or not; a container with or without an apparent player, garrisonable or not; a stealth
//! module, disguised or not, as which player and template; a mob member's slaver, present or not, with or without a
//! drawable; a warehouse's boxes), four players (allied, active, colours, playable, radar), four templates (a display
//! name, none, or `PROP`), four command buttons (the GUI command types the hints test and others; random options among
//! the targets, the context mode and others; cursor names that are cursors in their case, in another case, not cursors,
//! or empty; radius cursors, special powers and weapon slots), the windows under the mouse (0 to 3: `LeftHUDInput` or
//! not, see-through or not, the radar's or not) and the globals (multiplayer, the value of a box, double-click attack
//! move, the radar forced or hidden, playback, the mouse moved). Then 1 to 60 steps: mouse-over hints over any
//! drawable (or none), over the ground, every command hint `HintSpy` routes to `createCommandHint`, `setGUICommand` of
//! any button or none, `placeBuildAvailable` of any template or none with any builder, the scrolling and selecting
//! flags, the selection, the guard hint's trigger, the globals and windows changed, the selection translator's
//! mouse-over case with the context command valid or not, `setMouseCursor`, and the placement anchored or not. Both
//! sides write each call as it happens and the InGameUI's state after each step (the mouse mode and its cursor, the
//! moused-over drawable, the guard timer, the pending placement, its source, the prevent flag, the anchor, the pending
//! command); the words are compared whole.
//!
//! # Predictions (written before the first run, 2026-11-11)
//!
//! - 0 mismatches over 20,000 cases. Likeliest first-run misses, all of the harness: the tooltip text (the stand-in
//!   `format` against the host's), the recursion's call order in `placeBuildAvailable`, the window walk with no window.
//! - Planted bugs 9081-9094 all caught (rates in the notes of #122: 9082 and 9084 the rarest, a few percent).
//!
//! # Results (2026-11-11, `CURSOR_HINT_CASES=20000`, the default)
//!
//! The first run (2,000 cases) **missed the prediction**: `setGUICommand` in a playback changed the mouse mode and the
//! cursor where the C++ returns at once: the translation had left out the recorder's playback guard ("comes with the
//! replay viewer"). It is now translated (`InGameUiSelection::set_recorder_playback`, which the client sets from the
//! World's recorder each frame). After that: 2,000 cases, 0 mismatches; then the full run, **20,000 cases, 608,259
//! steps, 0 mismatches**. Every cursor was set somewhere (by cursor number 0..39: 341, 354, 134,754, 324, 41,475,
//! 2,653, 1,458, 6,059, 1,564, 25,709, 1,832, 1,655, 2,772, 4,478, 1,599, 3,797, 1,425, 1,508, 1,496, 1,589, 1,505,
//! 1,490, 340, 4,576, 359, 338, 338, 362, 1,553, 1,506, 377, 337, 366, 339, 5,696, 1,443, 377, 334, 1,466, 5,639);
//! 20,571 tooltips were set with a colour; the selection translator ate 12,035 mouse-overs.
//!
//! Planted bugs (same 20,000 cases): **all 14 caught**: 9081 by 3,669 cases, 9082 19, 9083 171, 9084 88, 9085 646,
//! 9086 575, 9087 1,170, 9088 4,720, 9089 1,572, 9090 8,271, 9091 1,532, 9092 3,750, 9093 9,921, 9094 7,550. Against the
//! predicted rates: 9082 (0.1 %) and 9084 (0.4 %) were rarer than the 1-5 % predicted, 9083 (0.9 %) rarer than 5 %;
//! 9088, 9090, 9092, 9093 and 9094 far more frequent than predicted (the generator gives mob members, modes and
//! placements often).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 9081 | `createCommandHint` ignores the window under the cursor |
//! | 9082 | the move hint's radar arrow drops the `isRadarForced` test |
//! | 9083 | an attack hint over a shrouded object is not made the move hint |
//! | 9084 | the guard hint's timer tested before its decrement |
//! | 9085 | the build placement mode tests `MSG_ADD_WAYPOINT_HINT` (upstream's quirk "fixed") |
//! | 9086 | the GUI command mode's valid hint shows the invalid cursor |
//! | 9087 | a GUI command cursor that is not a cursor shows the arrow, not `CROSS` |
//! | 9088 | a mob member is not redirected to its slaver |
//! | 9089 | `createMouseoverHint`'s `SELECTING` ignores who controls the object |
//! | 9090 | `createMouseoverHint` restores the mode's cursor in the build placement mode too |
//! | 9091 | `CanSelectDrawable` refuses structures outside a drag too |
//! | 9092 | the selection translator's mouse-over sets the cursor over shrubbery |
//! | 9093 | `setGUICommand` leaves `m_mouseModeCursor` as it was |
//! | 9094 | `placeBuildAvailable` skips its recursive end of a placement under way (the old translation) |

mod support;

use rts_lockstep_engine::client::control_bar::{gui, GuiButton};
use rts_lockstep_engine::client::in_game_ui::{DrawableId, InGameUiSelection, SelectionUiHost};
use rts_lockstep_engine::client::in_game_ui_cursor::{hint_spy_creates_command_hint, ContainInfo, CursorHintHost, DisguiseInfo, MouseoverHint, UiMouseCall, UiWindow};
use rts_lockstep_engine::client::selection_info::SelectionInfoHost;
use rts_lockstep_engine::client::selection_xlat::{CanSelectHost, Disposition, SelectionTranslator};
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::{ObjectShroudStatus, Relationship};
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::message_stream::MessageType;
use rts_lockstep_engine::common::message_types::*;
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

const SETUP: usize = 416;
const G: usize = 0;
const P: usize = 16;
const T: usize = 48;
const D: usize = 80;
const B: usize = 336;
const W: usize = 400;
/// The kinds of the stand-in's `kindBit`, bit by bit.
const KINDS: [KindOfType; 8] = [KINDOF_SELECTABLE, KINDOF_SHRUBBERY, KINDOF_STRUCTURE, KINDOF_IGNORED_IN_GUI, KINDOF_MINE, KINDOF_ALWAYS_SELECTABLE, KINDOF_FORCEATTACKABLE, KINDOF_DISGUISER];
/// The cursor names of the stand-in's `kNames`.
const NAMES: [&str; 8] = ["", "Target", "attackobj", "Nope", "Arrow", "ParticleUplinkCannon", "LaserGuidedMissiles", "WAYPOINT"];

fn wide(s: &str) -> Vec<u16> {
    s.bytes().map(u16::from).collect()
}

fn hash(s: &[u16]) -> i32 {
    let mut h: u32 = 2166136261;
    for &c in s {
        h ^= u32::from(c);
        h = h.wrapping_mul(16777619);
    }
    h as i32
}

/// The game of a case's setup words.
struct Host<'a> {
    w: &'a [i32],
    dcam: bool,
    playback: bool,
    moved: bool,
    radar_forced: bool,
    radar_hidden: bool,
    windows: Vec<UiWindow>,
}

impl Host<'_> {
    fn n(&self) -> i32 {
        self.w[G]
    }
    fn dw(&self, k: i32, f: usize) -> i32 {
        self.w[D + 32 * (k as usize - 1) + f]
    }
    fn exists(&self, k: i32) -> bool {
        k >= 1 && k <= self.n()
    }
    fn template_kinds(&self, t: i32) -> i32 {
        self.w[T + 8 * t as usize + 2]
    }
    fn kind(&self, k: i32, kind: KindOfType) -> bool {
        let bit = KINDS.iter().position(|&x| x == kind).map_or(0, |i| 1 << i);
        self.template_kinds(self.dw(k, 18)) & bit != 0
    }
    fn player(&self, p: i32, f: usize) -> i32 {
        self.w[P + 8 * p as usize + f]
    }
    /// The windows' flags as the wrapper's `Wins::set` reads them; the radar is the first one flagged.
    fn set_windows(&mut self, flags: &[i32]) {
        let mut radar_seen = false;
        self.windows = flags
            .iter()
            .map(|&f| {
                let radar = f & 4 != 0 && !radar_seen;
                radar_seen |= radar;
                UiWindow { left_hud_input: f & 1 != 0, see_thru: f & 2 != 0, radar }
            })
            .collect();
    }
}

impl SelectionInfoHost for Host<'_> {
    fn drawable_object(&self, d: DrawableId) -> Option<ObjectId> {
        (self.exists(d) && self.dw(d, 0) != 0).then_some(d)
    }
    fn is_locally_controlled(&self, o: ObjectId) -> bool {
        self.dw(o, 7) != 0
    }
    fn is_kind_of(&self, o: ObjectId, k: KindOfType) -> bool {
        self.kind(o, k)
    }
    fn relationship_to_local_player(&self, _o: ObjectId) -> Relationship {
        Relationship::Neutral
    }
    fn can_local_player_garrison(&mut self, _o: ObjectId) -> bool {
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

impl CanSelectHost for Host<'_> {
    fn is_effectively_dead(&self, o: ObjectId) -> bool {
        self.dw(o, 2) != 0
    }
    fn is_drawable_effectively_hidden(&self, d: DrawableId) -> bool {
        self.dw(d, 3) != 0
    }
    fn windows_at_drawable(&self, d: DrawableId) -> Vec<UiWindow> {
        if self.dw(d, 4) != 0 { vec![UiWindow::default()] } else { Vec::new() }
    }
    fn drawable_is_kind_of(&self, d: DrawableId, k: KindOfType) -> bool {
        self.kind(d, k)
    }
    fn unselectable_or_masked(&self, o: ObjectId) -> bool {
        self.dw(o, 5) != 0
    }
    fn object_is_selectable(&self, o: ObjectId) -> bool {
        self.dw(o, 6) != 0
    }
}

impl CursorHintHost for Host<'_> {
    fn find_drawable(&self, d: DrawableId) -> bool {
        self.exists(d)
    }
    fn windows_under_cursor(&self) -> Vec<UiWindow> {
        self.windows.clone()
    }
    fn recorder_playback(&self) -> bool {
        self.playback
    }
    fn mouse_moved_recently(&mut self) -> bool {
        self.moved
    }
    fn double_click_attack_move(&self) -> bool {
        self.dcam
    }
    fn radar_forced(&self) -> bool {
        self.radar_forced
    }
    fn radar_hidden(&self) -> bool {
        self.radar_hidden
    }
    fn local_player_has_radar(&self) -> bool {
        self.player(0, 5) != 0
    }
    fn object_shroud_status_for_local_player(&self, o: ObjectId) -> ObjectShroudStatus {
        match self.dw(o, 8) {
            1 => ObjectShroudStatus::Clear,
            2 => ObjectShroudStatus::PartialClear,
            3 => ObjectShroudStatus::Fogged,
            4 => ObjectShroudStatus::Shrouded,
            _ => ObjectShroudStatus::Invalid,
        }
    }
    fn mob_member_slaver(&self, o: ObjectId) -> Option<ObjectId> {
        (self.dw(o, 20) != 0).then(|| self.dw(o, 21))
    }
    fn object_drawable_of(&self, s: ObjectId) -> Option<DrawableId> {
        (self.exists(s) && self.dw(s, 0) != 0 && self.dw(s, 22) != 0).then_some(s)
    }
    fn contain_info(&mut self, o: ObjectId) -> Option<ContainInfo> {
        (self.dw(o, 10) != 0).then(|| ContainInfo { apparent_controlling_player: (self.dw(o, 11) >= 0).then(|| self.dw(o, 11)), garrisonable: self.dw(o, 12) != 0 })
    }
    fn controlling_player(&self, o: ObjectId) -> Option<i32> {
        (self.dw(o, 13) >= 0).then(|| self.dw(o, 13))
    }
    fn disguise(&self, o: ObjectId) -> Option<DisguiseInfo> {
        (self.dw(o, 14) != 0).then(|| DisguiseInfo { disguised: self.dw(o, 15) != 0, player_index: self.dw(o, 16), template: (self.dw(o, 17) >= 0).then(|| self.dw(o, 17) as u16) })
    }
    fn player_allied_to_local_default_team(&self, p: i32) -> bool {
        self.player(p, 0) != 0
    }
    fn local_player_active(&self) -> bool {
        self.player(0, 1) != 0
    }
    fn object_template(&self, o: ObjectId) -> u16 {
        self.dw(o, 18) as u16
    }
    fn template_display_name(&self, t: u16) -> Vec<u16> {
        match self.w[T + 8 * t as usize] {
            0 => Vec::new(),
            2 => wide("PROP"),
            _ => wide(&format!("Name{t}")),
        }
    }
    fn template_name(&self, t: u16) -> Vec<u8> {
        format!("Tmpl{t}").into_bytes()
    }
    fn fetch(&self, label: &[u8]) -> Vec<u16> {
        match label {
            b"OBJECT:Prop" => wide("PROP"),
            b"TOOLTIP:SupplyWarehouse" => wide(" ($%d)"),
            _ => label.iter().map(|&b| u16::from(b)).collect(),
        }
    }
    fn warehouse_boxes(&mut self, o: ObjectId) -> Option<i32> {
        (self.dw(o, 23) != 0).then(|| self.dw(o, 24))
    }
    fn base_value_per_supply_box(&self) -> i32 {
        self.w[G + 2]
    }
    fn format_int(&self, f: &[u16], v: i32) -> Vec<u16> {
        wide(&String::from_utf16_lossy(f).replacen("%d", &v.to_string(), 1))
    }
    fn recorder_multiplayer(&self) -> bool {
        self.w[G + 1] != 0
    }
    fn player_is_playable_side(&self, p: i32) -> bool {
        self.player(p, 4) != 0
    }
    fn player_display_name(&self, p: i32) -> Vec<u16> {
        wide(&format!("P{p}"))
    }
    fn object_cell_clear_for_local_player(&self, o: ObjectId) -> bool {
        self.dw(o, 9) != 0
    }
    fn player_color(&self, p: i32) -> i32 {
        self.player(p, 2)
    }
    fn indicator_color(&self, o: ObjectId) -> i32 {
        self.dw(o, 19)
    }
}

struct NoUi;
impl SelectionUiHost for NoUi {
    fn logic_frame(&self) -> u32 {
        0
    }
    fn append_message_bool(&mut self, _t: MessageType, _b: bool) {}
    fn on_selection_changed(&mut self, _d: DrawableId, _s: bool) {}
}

fn button(w: &[i32], b: usize) -> GuiButton {
    let at = B + 16 * b;
    GuiButton {
        index: b,
        command_type: w[at],
        options: w[at + 1] as u32,
        cursor_name: NAMES[w[at + 2] as usize].as_bytes().to_vec(),
        invalid_cursor_name: NAMES[w[at + 3] as usize].as_bytes().to_vec(),
        radius_cursor: w[at + 4],
        special_power: (w[at + 5] >= 0).then(|| (w[at + 5] as u32, 0)),
        weapon_slot: w[at + 6],
        ..Default::default()
    }
}

fn calls(ui: &mut InGameUiSelection, out: &mut Vec<i32>) {
    for c in ui.take_mouse_calls() {
        let r = match c {
            UiMouseCall::SetCursor(c) => [300, c, 0, 0],
            UiMouseCall::Tooltip { text, color } => [301, hash(&text), color.unwrap_or(0), i32::from(color.is_some())],
            UiMouseCall::ResetTooltipDelay => [302, 0, 0, 0],
            UiMouseCall::Capture => [303, 0, 0, 0],
            UiMouseCall::ReleaseCapture => [304, 0, 0, 0],
            UiMouseCall::RadiusCursor { kind, special_power, weapon_slot } => [305, kind, special_power.map_or(-1, |s| s as i32), weapon_slot],
        };
        out.extend(r);
        out.extend([0, 0, 0, 0]);
    }
}

fn state(ui: &InGameUiSelection, out: &mut Vec<i32>) {
    out.extend([
        200,
        ui.mouse_mode() as i32,
        ui.mouse_mode_cursor(),
        ui.moused_over_drawable().unwrap_or(0),
        ui.double_click_attack_move_guard_hint_timer(),
        ui.pending_place_type().map_or(-1, i32::from),
        ui.pending_place_source().unwrap_or(0),
        i32::from(ui.prevent_left_click_deselection_once()),
    ]);
    out.extend([201, i32::from(ui.is_placement_anchored()), ui.gui_command().map_or(-1, |b| b.index as i32), 0, 0, 0, 0, 0]);
}

fn rust_seq(input: &[i32]) -> Vec<i32> {
    let mut out = Vec::new();
    let mut h = Host { w: input, dcam: input[G + 3] != 0, playback: input[G + 6] != 0, moved: input[G + 7] != 0, radar_forced: input[G + 4] != 0, radar_hidden: input[G + 5] != 0, windows: Vec::new() };
    let count = input[W] as usize;
    let flags: Vec<i32> = (0..count).map(|i| input[W + 1 + 3 * i] | (input[W + 2 + 3 * i] << 1) | (input[W + 3 + 3 * i] << 2)).collect();
    h.set_windows(&flags);
    let mut ui = InGameUiSelection::new();
    ui.set_recorder_playback(h.playback);
    let mut sel = SelectionTranslator::default();
    state(&ui, &mut out);
    for w in input[SETUP..].chunks(8) {
        match w[0] {
            0 => ui.create_mouseover_hint(&MouseoverHint::Drawable(w[1]), &mut h),
            1 => ui.create_mouseover_hint(&MouseoverHint::Location(Coord3D::default()), &mut h),
            2 => ui.create_command_hint(MessageType(w[1]), &mut h),
            3 => ui.set_gui_command((w[1] >= 0).then(|| button(input, w[1] as usize))),
            4 => {
                let build = (w[1] >= 0).then_some(w[1] as u16);
                let builder = h.exists(w[2]).then_some(w[2]);
                ui.place_build_available(build, builder, 0.0, &h);
            }
            5 => {
                ui.set_is_scrolling(w[1] != 0);
                ui.set_selecting(w[2] != 0);
            }
            6 => {
                ui.deselect_all_drawables(false, &mut NoUi);
                for i in (0..w[1] as usize).rev() {
                    ui.select_drawable(w[2 + i], &mut NoUi);
                }
            }
            7 => ui.trigger_double_click_attack_move_guard_hint(),
            8 => {
                h.dcam = w[1] != 0;
                h.playback = w[2] != 0;
                ui.set_recorder_playback(h.playback);
                h.moved = w[3] != 0;
                h.radar_forced = w[4] != 0;
                h.radar_hidden = w[5] != 0;
            }
            9 => h.set_windows(&w[2..2 + w[1] as usize]),
            10 => {
                let answer = MessageType(w[2]);
                let d = sel.mouseover_drawable_hint(w[1], &mut ui, &mut h, |_, _| answer);
                calls(&mut ui, &mut out);
                out.extend([306, i32::from(d == Disposition::Destroy), 0, 0, 0, 0, 0, 0]);
            }
            11 => ui.set_mouse_cursor(w[1]),
            _ => ui.set_placement_start((w[1] != 0).then(Default::default)),
        }
        calls(&mut ui, &mut out);
        state(&ui, &mut out);
    }
    out
}

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let cap = 8 * (4 + 40 * (input.len() / 8 + 2));
    let mut out = vec![0i32; cap];
    let n = unsafe { cpp::ref_cursor_hint_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), cap as i32) };
    out.truncate(n as usize);
    out
}

/// The command hints `HintSpy` gives `createCommandHint`.
fn hints() -> Vec<MessageType> {
    (0..1100).map(MessageType).filter(|&t| hint_spy_creates_command_hint(t)).collect()
}

fn gen_case(g: &mut Gen, hint_types: &[MessageType]) -> Vec<i32> {
    let mut v = vec![0i32; SETUP];
    let n = 1 + g.below(8) as i32;
    v[G] = n;
    v[G + 1] = g.below(2) as i32;
    v[G + 2] = 50 + g.below(100) as i32;
    v[G + 3] = i32::from(g.below(3) == 0);
    v[G + 4] = i32::from(g.below(4) == 0);
    v[G + 5] = i32::from(g.below(3) == 0);
    v[G + 6] = i32::from(g.below(8) == 0);
    v[G + 7] = g.below(2) as i32;
    v[G + 8] = g.below(2) as i32;
    for p in 0..4 {
        let at = P + 8 * p;
        v[at] = g.below(2) as i32;
        v[at + 1] = i32::from(g.below(5) != 0);
        v[at + 2] = g.i32();
        v[at + 3] = g.i32();
        v[at + 4] = g.below(2) as i32;
        v[at + 5] = i32::from(g.below(3) != 0);
    }
    for t in 0..4 {
        let at = T + 8 * t;
        v[at] = [0, 1, 1, 2][g.below(4) as usize];
        v[at + 1] = g.uniform(0.0, 6.0).to_bits() as i32;
        let mut kinds = g.below(256) as i32;
        if g.below(3) != 0 {
            kinds |= 1; // selectable, mostly
        }
        if g.below(2) == 0 {
            kinds &= !(8 | 128); // fewer mob members and disguisers
        }
        v[at + 2] = kinds;
    }
    for k in 1..=n {
        let at = D + 32 * (k as usize - 1);
        let t = g.below(4) as i32;
        let disguiser = v[T + 8 * t as usize + 2] & 128 != 0;
        let f = [
            i32::from(g.below(6) != 0),                          // 0 has object
            0,                                                   // 1
            i32::from(g.below(6) == 0),                          // 2 dead
            i32::from(g.below(8) == 0),                          // 3 hidden
            i32::from(g.below(8) == 0),                          // 4 obscured
            i32::from(g.below(8) == 0),                          // 5 unselectable or masked
            i32::from(g.below(6) != 0),                          // 6 selectable
            g.below(2) as i32,                                   // 7 local
            g.below(5) as i32,                                   // 8 shroud
            i32::from(g.below(4) != 0),                          // 9 cell clear
            i32::from(g.below(3) == 0),                          // 10 contain
            g.below(5) as i32 - 1,                               // 11 apparent player
            g.below(2) as i32,                                   // 12 garrisonable
            if disguiser { g.below(4) as i32 } else { g.below(5) as i32 - 1 }, // 13 controlling player
            i32::from(g.below(2) == 0),                          // 14 stealth
            g.below(2) as i32,                                   // 15 disguised
            g.below(4) as i32,                                   // 16 disguised player
            g.below(5) as i32 - 1,                               // 17 disguised template
            t,                                                   // 18 template
            g.i32(),                                             // 19 indicator colour
            i32::from(g.below(2) == 0),                          // 20 mob module
            g.below(n as u32 + 2) as i32,                        // 21 slaver
            i32::from(g.below(5) != 0),                          // 22 has drawable
            i32::from(g.below(4) == 0),                          // 23 warehouse
            g.below(20) as i32,                                  // 24 boxes
        ];
        v[at..at + f.len()].copy_from_slice(&f);
    }
    let types = [gui::SPECIAL_POWER, gui::SPECIAL_POWER_FROM_SHORTCUT, gui::ATTACK_MOVE, gui::GUARD, gui::GUARD_WITHOUT_PURSUIT, gui::GUARD_FLYING_UNITS_ONLY, gui::FIRE_WEAPON, gui::DOZER_CONSTRUCT];
    let option_bits = [0x1, 0x2, 0x4, 0x20, 0x200, 0x10, 0x100, 0x800];
    for b in 0..4 {
        let at = B + 16 * b;
        v[at] = types[g.below(types.len() as u32) as usize];
        let mut o = 0;
        for bit in option_bits {
            if g.below(3) == 0 {
                o |= bit;
            }
        }
        v[at + 1] = o;
        v[at + 2] = g.below(NAMES.len() as u32) as i32;
        v[at + 3] = g.below(NAMES.len() as u32) as i32;
        v[at + 4] = g.below(29) as i32;
        v[at + 5] = g.below(65) as i32 - 1;
        v[at + 6] = g.below(3) as i32;
    }
    let windows = |g: &mut Gen| if g.below(2) == 0 { 0 } else { 1 + g.below(3) };
    let wn = windows(g);
    v[W] = wn as i32;
    for i in 0..wn as usize {
        v[W + 1 + 3 * i] = g.below(2) as i32;
        v[W + 2 + 3 * i] = g.below(2) as i32;
        v[W + 3 + 3 * i] = i32::from(g.below(3) == 0);
    }
    // a builder the C++ can use: its object (if any) has a controlling player
    let builder_ok = |v: &[i32], k: i32| v[D + 32 * (k as usize - 1)] == 0 || v[D + 32 * (k as usize - 1) + 13] >= 0;
    let steps = 1 + g.below(60);
    let drawable = |g: &mut Gen| g.below(n as u32 + 2) as i32;
    for _ in 0..steps {
        let s: [i32; 8] = match g.below(100) {
            0..=14 => [0, drawable(g), 0, 0, 0, 0, 0, 0],
            15..=22 => [1, 0, 0, 0, 0, 0, 0, 0],
            23..=47 => [2, hint_types[g.below(hint_types.len() as u32) as usize].0, 0, 0, 0, 0, 0, 0],
            48..=55 => [3, g.below(5) as i32 - 1, 0, 0, 0, 0, 0, 0],
            56..=61 => {
                let build = g.below(5) as i32 - 1;
                let mut b = g.below(n as u32 + 1) as i32;
                if b > 0 && !builder_ok(&v, b) {
                    b = 0;
                }
                [4, build, b, 0, 0, 0, 0, 0]
            }
            62..=65 => [5, i32::from(g.below(4) == 0), i32::from(g.below(4) == 0), 0, 0, 0, 0, 0],
            66..=73 => {
                let count = g.below(4) as usize;
                let mut ids: Vec<i32> = (1..=n).collect();
                for i in 0..ids.len() {
                    let j = i + g.below((ids.len() - i) as u32) as usize;
                    ids.swap(i, j);
                }
                let count = count.min(ids.len());
                let mut s = [6, count as i32, 0, 0, 0, 0, 0, 0];
                s[2..2 + count].copy_from_slice(&ids[..count]);
                s
            }
            74..=77 => [7, 0, 0, 0, 0, 0, 0, 0],
            78..=82 => [8, i32::from(g.below(2) == 0), i32::from(g.below(8) == 0), g.below(2) as i32, i32::from(g.below(4) == 0), i32::from(g.below(3) == 0), 0, 0],
            83..=88 => {
                let c = windows(g) as usize;
                let mut s = [9, c as i32, 0, 0, 0, 0, 0, 0];
                for i in 0..c {
                    s[2 + i] = g.below(2) as i32 | ((g.below(2) as i32) << 1) | (i32::from(g.below(3) == 0) << 2);
                }
                s
            }
            89..=95 => [10, drawable(g), if g.below(2) == 0 { MSG_INVALID.0 } else { MSG_DO_MOVETO_HINT.0 }, 0, 0, 0, 0, 0],
            96..=97 => [11, g.below(40) as i32, 0, 0, 0, 0, 0, 0],
            _ => [12, g.below(2) as i32, 0, 0, 0, 0, 0, 0],
        };
        v.extend(s);
    }
    v
}

#[test]
fn the_in_game_ui_cursors_match_upstream() {
    let cases: u64 = std::env::var("CURSOR_HINT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let hint_types = hints();
    assert_eq!(hint_types.len(), 26, "HintSpy's list");
    let mut g = Gen::new(0x122_1220);
    let mut whole = Check::new("InGameUI cursor sequences");
    let (mut steps, mut cursors, mut tooltips, mut destroyed, mut shown) = (0u64, [0u64; 40], 0u64, 0u64, 0u32);
    for i in 0..cases {
        let input = gen_case(&mut g, &hint_types);
        steps += ((input.len() - SETUP) / 8) as u64;
        let r = rust_seq(&input);
        let c = cpp_seq(&input);
        for rec in c.chunks(8) {
            match rec[0] {
                300 if (0..40).contains(&rec[1]) => cursors[rec[1] as usize] += 1,
                301 if rec[3] == 1 => tooltips += 1,
                306 if rec[1] == 1 => destroyed += 1,
                _ => {}
            }
        }
        if r != c && shown < 5 {
            shown += 1;
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len())) / 8 * 8;
            println!("case {i}: rust {:?}\n        c++  {:?}", &r[k.saturating_sub(16).min(r.len())..(k + 24).min(r.len())], &c[k.saturating_sub(16).min(c.len())..(k + 24).min(c.len())]);
        }
        whole.eq(r, c, || format!("case {i}"));
    }
    println!("{cases} cases, {steps} steps; cursors set (by cursor) {cursors:?}; tooltips with a colour {tooltips}; mouse-overs the selection translator destroyed {destroyed}");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
