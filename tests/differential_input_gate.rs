#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/WindowXlat.cpp
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/CommandXlat.cpp (the input gate, isSystemMessage; #123, #124:
//     translateGameMessage's mouse-over, click and double-click cases)
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/SelectionXlat.cpp (the input gate)
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (setInputEnabled)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The client's input gates (GitHub #116) against the C++: `client::window_xlat`, `client::command_xlat`'s
//! `input_disabled_destroys`, `client::selection_xlat`'s `input_disabled_gate` and `client::in_game_ui`'s
//! `set_input_enabled`. **(#123, #124)** And the command translator's mouse cases (the mouse-over hints, the clicks and
//! the double clicks): see [`command_mouse_cases_match_upstream`].
//!
//! # Method
//!
//! The C++ side is the upstream text cut verbatim by build.rs (`differential/wrap_input_gate.cpp` lists the cuts; the two
//! `translateGameMessage` pieces end at the gate and are closed with `return disp; }`), compiled on stand-ins for the
//! window manager, the shell, the tactical view, the mouse, the display, the global data and the InGameUI, with the real
//! `GameMessage` of the message stream build. Each case is random: a message type (every raw mouse and key message,
//! `MSG_META_TOGGLE_ATTACKMOVE`, `MSG_META_OPTIONS`, `MSG_CLEAR_GAME_DATA`, the four system messages, and any number
//! below 1,200), its arguments (positions, a drag's delta, the wheel's count with 0 and negatives, keys with Escape one
//! time in three and integers past a byte), and every flag the gates read. The four kinds of case are drawn in turn:
//!
//! - the window translator: its disposition, the `stopMovie` calls, and every `winProcessMouseEvent` (window message,
//!   position, the data's kind and value) and `winProcessKey` (key and state bytes) call, in order;
//! - the command translator's gate: its disposition;
//! - the selection translator's gate: its disposition, `m_dragSelecting` and `isSelecting` after, and the calls that end a
//!   drag (`endAreaSelectHint`, `setMouseLock(FALSE)`), which the Rust counts as "a drag ended";
//! - `setInputEnabled`: the input and the five flags after; the four camera flags it clears ~~(not InGameUI members in
//!   the port) are compared with the branch the Rust takes (enabled before and disabling)~~. **Corrected 2026-11-11
//!   (#131):** the camera flags are InGameUI members now; they are set before the call and compared cleared or not.
//!
//! # Results (2026-11-10, `INPUT_GATE_CASES=20000`, the default)
//!
//! 20,000 cases, 5,000 of each kind: **0 mismatches** in each of the four. Of the window translator's cases 1,001 were
//! destroyed and 2 stopped a movie (the movie branch needs an unused Escape release, a movie and the exit allowed: few
//! cases reach it); 1,235 selection cases ended a drag.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints `MEASURED` lines and fails on purpose): **8 of 8 caught** (2026-11-10).
//!
//! | id | planted bug | caught by |
//! |---|---|---|
//! | 9021 | `setInputEnabled` clears the modes on every disable, not only from enabled | 1,163 cases |
//! | 9022 | `setInputEnabled` leaves prefer selection on | 627 cases |
//! | 9023 | the command gate destroys `MSG_CLEAR_GAME_DATA` | 61 cases |
//! | 9024 | `isSystemMessage` forgets `MSG_DESTROY_SELECTED_GROUP` | 68 cases |
//! | 9025 | the selection gate does not end the drag | 1,235 cases |
//! | 9026 | the window translator destroys a release while a placement is anchored | 43 cases |
//! | 9027 | the window translator lets a drag through with the input off | 37 cases |
//! | 9028 | the window translator lets an Escape release through with the input off | 3 cases (few: see the results) |

mod support;

use rts_lockstep_engine::client::command_xlat::input_disabled_destroys;
use rts_lockstep_engine::client::in_game_ui::InGameUiSelection;
use rts_lockstep_engine::client::selection_xlat::{Disposition, SelectionTranslator};
use rts_lockstep_engine::client::window_manager::WinInputReturnCode;
use rts_lockstep_engine::client::window_xlat::{translate_game_message, MouseData, RawMessage, WindowXlatHost};
use rts_lockstep_engine::common::base_type::ICoord2D;
use rts_lockstep_engine::common::message_stream::MessageType;
use rts_lockstep_engine::common::message_types::*;
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

fn cpp_case(kind: i32, input: &[i32]) -> Vec<i32> {
    // (#124: the wrapper's state is global and the two tests run on two threads; the first run of the mouse cases mixed
    // their outputs, 877 window and 2,364 mouse "mismatches")
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _held = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = vec![0i32; 256];
    let n = unsafe { cpp::ref_input_gate_case(kind, input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn disp(d: Disposition) -> i32 {
    match d {
        Disposition::Keep => 0,
        Disposition::Destroy => 1,
    }
}

/// The window translator's host: the flags of a case, and its calls in the C++ stand-ins' words.
struct Host {
    flags: i32,
    status: ICoord2D,
    out: Vec<i32>,
    stops: i32,
}

impl Host {
    fn flag(&self, bit: i32) -> bool {
        (self.flags >> bit) & 1 != 0
    }
}

impl WindowXlatHost for Host {
    fn mouse_locked(&self) -> bool {
        self.flag(0)
    }
    fn ui_is_scrolling(&self) -> bool {
        self.flag(1)
    }
    fn ui_input_enabled(&self) -> bool {
        self.flag(2)
    }
    fn ui_placement_anchored(&self) -> bool {
        self.flag(3)
    }
    fn shell_active(&self) -> bool {
        self.flag(4)
    }
    fn mouse_status_pos(&self) -> ICoord2D {
        self.status
    }
    fn win_process_mouse_event(&mut self, gwm: u32, pos: ICoord2D, data: MouseData) -> WinInputReturnCode {
        let (kind, x, y) = match data {
            MouseData::None => (0, 0, 0),
            MouseData::Delta(d) => (1, d.x, d.y),
            MouseData::Wheel(w) => (2, w, 0),
        };
        self.out.extend([gwm as i32, pos.x, pos.y, kind, x, y]);
        if self.flag(5) { WinInputReturnCode::Used } else { WinInputReturnCode::NotUsed }
    }
    fn win_process_key(&mut self, key: u8, state: u8) -> WinInputReturnCode {
        self.out.extend([100, i32::from(key), i32::from(state), 0, 0, 0]);
        if self.flag(6) { WinInputReturnCode::Used } else { WinInputReturnCode::NotUsed }
    }
    fn movie_playing(&self) -> bool {
        self.flag(7)
    }
    fn allow_exit_out_of_movies(&self) -> bool {
        self.flag(8)
    }
    fn stop_movie(&mut self) {
        self.stops += 1;
    }
}

fn gen_type(g: &mut Gen) -> MessageType {
    const POOL: [MessageType; 9] =
        [MSG_META_TOGGLE_ATTACKMOVE, MSG_META_OPTIONS, MSG_CLEAR_GAME_DATA, MSG_DESTROY_SELECTED_GROUP, MSG_LOGIC_CRC, MSG_SET_REPLAY_CAMERA, MSG_FRAME_TICK, MSG_DO_MOVETO, MSG_MOUSE_LEFT_CLICK];
    match g.below(4) {
        0 | 1 => MessageType((3 + g.below(18)) as _),
        2 => POOL[g.below(POOL.len() as u32) as usize],
        _ => MessageType(g.below(1200) as _),
    }
}

fn small(g: &mut Gen) -> i32 {
    g.below(1601) as i32 - 800
}

/// One window translator case: its input words and the Rust's output.
fn window_case(g: &mut Gen) -> (Vec<i32>, Vec<i32>) {
    let t = gen_type(g);
    let is_key = t == MSG_RAW_KEY_DOWN || t == MSG_RAW_KEY_UP;
    let pos = ICoord2D { x: small(g), y: small(g) };
    let arg0 = if g.below(3) == 0 { 1 } else if g.below(2) == 0 { g.below(256) as i32 } else { g.i32() };
    let arg1 = match g.below(6) {
        0 => 0,
        1 => 120,
        2 => -120,
        3 => g.below(4) as i32,
        _ => g.i32(),
    };
    let delta = ICoord2D { x: arg1, y: small(g) };
    let flags = g.below(512) as i32;
    let status = ICoord2D { x: small(g), y: small(g) };
    let m = RawMessage { t, pos: if is_key { ICoord2D { x: arg0, y: 0 } } else { pos }, delta, arg0, arg1 };
    let mut h = Host { flags, status, out: Vec::new(), stops: 0 };
    let d = translate_game_message(&m, &mut h);
    let mut rust = vec![disp(d), h.stops];
    rust.extend(h.out);
    let x = if is_key { arg0 } else { pos.x };
    let input = vec![t.0 as i32, x, pos.y, arg1, delta.y, flags, status.x, status.y];
    (input, rust)
}

#[test]
fn input_gates_match_upstream() {
    let cases: u64 = std::env::var("INPUT_GATE_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x116_1160);
    let (mut win, mut cmd, mut sel, mut ui) =
        (Check::new("WindowTranslator::translateGameMessage"), Check::new("CommandTranslator input gate"), Check::new("SelectionTranslator input gate"), Check::new("InGameUI::setInputEnabled"));
    let (mut destroyed, mut stops, mut ended) = (0u64, 0u64, 0u64);
    for i in 0..cases {
        match i % 4 {
            0 => {
                let (input, rust) = window_case(&mut g);
                let c = cpp_case(0, &input);
                destroyed += c.first().copied().unwrap_or(0) as u64;
                stops += c.get(1).copied().unwrap_or(0) as u64;
                win.eq(rust, c, || format!("case {i} {input:?}"));
            }
            1 => {
                let t = gen_type(&mut g);
                let on = g.below(2) == 1;
                let rust = vec![i32::from(input_disabled_destroys(t, on))];
                cmd.eq(rust, cpp_case(1, &[t.0 as i32, i32::from(on)]), || format!("case {i} {t:?} input {on}"));
            }
            2 => {
                let t = gen_type(&mut g);
                let (on, drag, selecting) = (g.below(2) == 1, g.below(2) == 1, g.below(2) == 1);
                let mut x = SelectionTranslator { drag_selecting: drag, ..Default::default() };
                let mut s = InGameUiSelection::new();
                s.set_input_enabled(on);
                s.set_selecting(selecting);
                let d = x.input_disabled_gate(&mut s).unwrap_or(Disposition::Keep);
                let end = i32::from(drag && !x.drag_selecting);
                ended += end as u64;
                let rust = vec![disp(d), i32::from(x.drag_selecting), i32::from(s.is_selecting()), end, end];
                sel.eq(rust, cpp_case(2, &[t.0 as i32, i32::from(on), i32::from(drag), i32::from(selecting)]), || format!("case {i} input {on} drag {drag} selecting {selecting}"));
            }
            _ => {
                let b: Vec<bool> = (0..7).map(|_| g.below(2) == 1).collect();
                let mut s = InGameUiSelection::new();
                s.set_input_enabled(b[1]);
                s.set_selecting(b[2]);
                s.set_force_attack_mode(b[3]);
                s.set_force_move_mode(b[4]);
                s.set_waypoint_mode(b[5]);
                s.set_prefer_selection_mode(b[6]);
                // (#131: the camera flags are InGameUI members now: all set, then whether each was cleared)
                s.set_camera_rotate_left(true);
                s.set_camera_rotate_right(true);
                s.set_camera_zoom_in(true);
                s.set_camera_zoom_out(true);
                s.set_input_enabled(b[0]);
                let rust = vec![
                    i32::from(s.input_enabled()),
                    i32::from(s.is_selecting()),
                    i32::from(s.is_in_force_attack_mode()),
                    i32::from(s.is_in_force_move_to_mode()),
                    i32::from(s.is_in_waypoint_mode()),
                    i32::from(s.is_in_prefer_selection_mode()),
                    i32::from(!s.is_camera_rotating_left()),
                    i32::from(!s.is_camera_rotating_right()),
                    i32::from(!s.is_camera_zooming_in()),
                    i32::from(!s.is_camera_zooming_out()),
                ];
                let input: Vec<i32> = b.iter().map(|&x| i32::from(x)).collect();
                ui.eq(rust, cpp_case(3, &input), || format!("case {i} {b:?}"));
            }
        }
    }
    println!("window cases destroyed {destroyed}, movies stopped {stops}; selection drags ended {ended}");
    for c in [win, cmd, sel, ui] {
        if planted {
            c.report();
        } else {
            c.finish();
        }
    }
    assert!(!planted, "a planted run: see the MEASURED lines (a mismatch is the plant caught)");
}

// ---- #123, #124: the command translator's mouse cases ----

use rts_lockstep_engine::client::command_xlat::{ActionType, CmdArg, CommandEvaluateType, CommandTranslator, CommandXlatHost, Evaluation, GuiButton, TargetMode};
use rts_lockstep_engine::client::control_bar::{gui, opt};
use rts_lockstep_engine::client::in_game_ui::{DrawableId, SelectionUiHost};
use rts_lockstep_engine::client::selection_groups::SelectionGroupsHost;
use rts_lockstep_engine::client::selection_info::SelectionInfoHost;
use rts_lockstep_engine::client::selection_xlat::SelectionXlatHost;
use rts_lockstep_engine::client::unit_voice::UnitVoiceHost;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::kind_of::KindOfType;
use rts_lockstep_engine::common::special_power_type::SpecialPowerType;
use rts_lockstep_engine::logic::ai::ai_update_types::{CanAttackResultValue, ATTACKRESULT_NOT_POSSIBLE};

/// The mouse cases' host: drawable `n` is object `n` at the C++ stand-in's position, every object locally controlled or
/// none (`controllable`); it logs the messages appended and the academy's records. The questions the cases never ask (the
/// evaluation is the test's closure) answer nothing.
#[derive(Default)]
struct MouseHost {
    alternate: bool,
    attack_move: bool,
    controllable: bool,
    /// #131: the stand-in `TheMouse->isClick`'s answer.
    is_click: bool,
    out: Vec<i32>,
    evals: Vec<i32>,
    records: i32,
    /// #132: the `MSG_DESTROY_SELECTED_GROUP`s posted (`deselectAllDrawables`); not part of the mouse cases' output.
    destroy_groups: i32,
}

fn draw_pos(o: ObjectId) -> Coord3D {
    Coord3D { x: o as f32 * 10.0 + 0.5, y: o as f32 * 3.0 - 2.0, z: o as f32 * 0.25 }
}

impl SelectionUiHost for MouseHost {
    fn logic_frame(&self) -> u32 {
        0
    }
    fn append_message_bool(&mut self, t: MessageType, _b: bool) {
        if t == MSG_DESTROY_SELECTED_GROUP {
            self.destroy_groups += 1;
        }
    }
    fn on_selection_changed(&mut self, _draw: DrawableId, _selected: bool) {}
}
impl SelectionInfoHost for MouseHost {
    fn drawable_object(&self, draw: DrawableId) -> Option<ObjectId> {
        Some(draw)
    }
    fn is_locally_controlled(&self, _obj: ObjectId) -> bool {
        self.controllable
    }
    fn is_kind_of(&self, _obj: ObjectId, _k: KindOfType) -> bool {
        false
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
        self.alternate
    }
}
impl SelectionXlatHost for MouseHost {
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
    fn contained_by(&self, _obj: ObjectId) -> Option<ObjectId> {
        None
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
    fn append_message(&mut self, _t: MessageType, _create: Option<bool>, _objects: &[ObjectId]) {}
    fn record_drag_selection(&mut self) {}
    fn clear_attack_move_to_mode(&mut self) {}
}
impl SelectionGroupsHost for MouseHost {
    fn all_drawables(&self) -> Vec<DrawableId> {
        Vec::new()
    }
    fn drawables_on_screen(&self) -> Vec<DrawableId> {
        Vec::new()
    }
    fn pick_selectable_at_click(&self) -> Option<DrawableId> {
        None
    }
    fn is_mass_selectable(&self, _obj: ObjectId) -> bool {
        false
    }
    fn object_template(&self, _obj: ObjectId) -> u32 {
        0
    }
    fn is_equivalent_to(&self, _obj: ObjectId, _template: u32) -> bool {
        false
    }
    fn is_car_bomb(&self, _obj: ObjectId) -> bool {
        false
    }
    fn is_off_map(&self, _obj: ObjectId) -> bool {
        false
    }
    fn hotkey_squad_live_objects(&mut self, _group: i32) -> Option<Vec<ObjectId>> {
        None
    }
    fn look_at(&mut self, _obj: ObjectId) {}
    fn ui_message(&mut self, _label: &[u8], _count: Option<i32>) {}
    fn append_message_drawable(&mut self, _t: MessageType, _draw: DrawableId) {}
}
impl UnitVoiceHost for MouseHost {}
impl CommandXlatHost for MouseHost {
    fn object_is_masked(&self, _obj: ObjectId) -> bool {
        false
    }
    fn object_position(&self, obj: ObjectId) -> Coord3D {
        draw_pos(obj)
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
        // (`GameMessageArgumentDataType`: 0 an integer, 6 a location)
        self.out.extend([t.0 as i32, args.len() as i32]);
        for a in args {
            match *a {
                CmdArg::Int(i) => self.out.extend([0, i]),
                CmdArg::Location(l) => self.out.extend([6, l.x.to_bits() as i32, l.y.to_bits() as i32, l.z.to_bits() as i32]),
                // (the mouse cases append no other argument)
                _ => self.out.extend([99, 0]),
            }
        }
    }
    fn append_hint(&mut self, _t: MessageType, _args: &[CmdArg]) {}
    fn double_click_attack_move(&self) -> bool {
        self.attack_move
    }
    fn record_double_click_attack_move_order_given(&mut self) {
        self.records += 1;
    }
}
impl rts_lockstep_engine::client::command_xlat::MouseClickHost for MouseHost {
    fn mouse_is_click(&self, _anchor: &ICoord2D, _dest: &ICoord2D, _previous: u32, _current: u32) -> bool {
        self.is_click
    }
}

/// The evaluation the C++ stand-ins log: (1 force attack / 2 context, drawable or 0, the position's bits, the type).
fn log_eval(e: Evaluation, d: Option<DrawableId>, p: &Coord3D, ty: CommandEvaluateType, h: &mut MouseHost) -> MessageType {
    let kind = match e {
        Evaluation::ForceAttack => 1,
        Evaluation::Context => 2,
    };
    let ty = match ty {
        CommandEvaluateType::DoCommand => 0,
        CommandEvaluateType::DoHint => 1,
        CommandEvaluateType::EvaluateOnly => 2,
    };
    h.evals.extend([kind, d.unwrap_or(0) as i32, p.x.to_bits() as i32, p.y.to_bits() as i32, p.z.to_bits() as i32, ty]);
    MSG_INVALID
}

/// One mouse case: its input words for the C++ and the Rust's output.
fn mouse_case(g: &mut Gen, stats: &mut [u64; 4]) -> (Vec<i32>, Vec<i32>) {
    const TYPES: [MessageType; 16] = [
        MSG_MOUSEOVER_DRAWABLE_HINT,
        MSG_MOUSEOVER_DRAWABLE_HINT,
        MSG_MOUSEOVER_DRAWABLE_HINT,
        MSG_MOUSEOVER_LOCATION_HINT,
        MSG_MOUSEOVER_LOCATION_HINT,
        MSG_MOUSE_LEFT_CLICK,
        MSG_MOUSE_LEFT_CLICK,
        MSG_MOUSE_LEFT_CLICK,
        MSG_MOUSE_LEFT_DOUBLE_CLICK,
        MSG_MOUSE_LEFT_DOUBLE_CLICK,
        MSG_MOUSE_LEFT_DOUBLE_CLICK,
        MSG_MOUSE_LEFT_DOUBLE_CLICK,
        MSG_MOUSE_RIGHT_CLICK,
        MSG_MOUSE_RIGHT_DOUBLE_CLICK,
        MSG_MOUSE_RIGHT_DOUBLE_CLICK,
        MSG_MOUSE_RIGHT_DOUBLE_CLICK,
    ];
    const COMMANDS: [i32; 12] = [
        gui::SPECIAL_POWER_FROM_SHORTCUT,
        gui::SPECIAL_POWER_FROM_SHORTCUT,
        gui::SPECIAL_POWER_FROM_SHORTCUT,
        gui::SPECIAL_POWER,
        gui::FIRE_WEAPON,
        gui::COMBATDROP,
        gui::HIJACK_VEHICLE,
        gui::CONVERT_TO_CARBOMB,
        gui::SABOTAGE_BUILDING,
        gui::ATTACK_MOVE,
        gui::GUARD,
        -2,
    ];
    let t = TYPES[g.below(16) as usize];
    let on = g.below(8) != 0;
    let (alternate, attack_move) = (g.below(2) == 1, g.below(2) == 1);
    let sel = g.below(4) as i32;
    let controllable = g.below(2) == 1;
    let command = if g.below(3) == 0 {
        -1
    } else {
        match COMMANDS[g.below(COMMANDS.len() as u32) as usize] {
            -2 => g.below(35) as i32,
            c => c,
        }
    };
    let (force, am) = (g.below(4) == 0, g.below(2) == 1);
    let lo = ICoord2D { x: small(g), y: small(g) };
    let hi = if g.below(2) == 0 { lo } else { ICoord2D { x: small(g), y: small(g) } };
    let picked = if g.below(3) == 0 { 0 } else { 1 + g.below(9) };
    let draw = 1 + g.below(9);
    let loc = Coord3D { x: small(g) as f32 / 4.0, y: small(g) as f32 / 4.0, z: g.below(41) as f32 / 8.0 };
    let is_click = g.below(4) != 0;
    let input = vec![
        t.0 as i32,
        i32::from(on),
        i32::from(alternate),
        i32::from(attack_move),
        sel,
        i32::from(sel > 0 && controllable),
        command,
        i32::from(force),
        i32::from(am),
        lo.x,
        lo.y,
        hi.x,
        hi.y,
        picked as i32,
        draw as i32,
        loc.x.to_bits() as i32,
        loc.y.to_bits() as i32,
        loc.z.to_bits() as i32,
        // `TheMouse->isClick`: ~~the port does not keep the right button's bookkeeping (CommandXlat.cpp:3628-3657), so the
        // stand-in always answers a click~~ **Corrected 2026-11-11 (#131):** the port keeps it and asks the host
        // (`CommandXlatHost::mouse_is_click`), so the stand-in's answer is random, a click 3 times in 4
        i32::from(is_click),
    ];

    let mut h = MouseHost { alternate, attack_move, controllable, is_click, ..Default::default() };
    let mut ui = InGameUiSelection::new();
    ui.set_input_enabled(on);
    for d in 1..=sel {
        ui.select_drawable(d as DrawableId, &mut h);
    }
    if command >= 0 {
        ui.set_gui_command(Some(GuiButton { command_type: command, options: opt::NEED_TARGET_POS, ..Default::default() }));
    }
    ui.set_force_attack_mode(force);
    ui.set_attack_move_to_mode(am);
    let x = CommandTranslator::default();
    // `screenToTerrain` of the region's `lo` (`msg->getArgument(0)->pixel`), the stand-in's
    let ground = Coord3D { x: lo.x as f32 * 0.5 + 3.0, y: lo.y as f32 * 0.25 - 7.0, z: 1.5 };
    let is_point = lo == hi;
    let picked = (picked != 0).then_some(picked as DrawableId);
    let destroyed = if t == MSG_MOUSEOVER_DRAWABLE_HINT || t == MSG_MOUSEOVER_LOCATION_HINT {
        let d = (t == MSG_MOUSEOVER_DRAWABLE_HINT).then_some(draw as DrawableId);
        x.mouse_over_with(d, &loc, &mut ui, &mut h, |e, d, p, ty, _ui, h| log_eval(e, d, p, ty, h));
        // (the hints are kept; the gate destroys them before their case)
        input_disabled_destroys(t, on)
    } else {
        let right = t == MSG_MOUSE_RIGHT_CLICK || t == MSG_MOUSE_RIGHT_DOUBLE_CLICK;
        let eval = |e, d, p: &Coord3D, ty, _ui: &mut InGameUiSelection, h: &mut MouseHost| log_eval(e, d, p, ty, h);
        if t == MSG_MOUSE_LEFT_DOUBLE_CLICK || t == MSG_MOUSE_RIGHT_DOUBLE_CLICK {
            x.double_click_with(right, is_point, &ground, picked, &mut ui, &mut h, eval)
        } else {
            x.click_with(right, is_point, &ground, picked, &mut ui, &mut h, eval)
        }
    };
    stats[0] += u64::from(!h.evals.is_empty());
    stats[1] += u64::from(h.records > 0);
    stats[2] += u64::from(destroyed);
    stats[3] += u64::from(h.evals.len() >= 6 && h.evals[5] == 1 && t == MSG_MOUSEOVER_DRAWABLE_HINT && sel == 0);
    let mut rust = vec![i32::from(destroyed)];
    rust.extend(&h.evals);
    rust.push(-1);
    rust.extend(&h.out);
    rust.extend([-2, h.records, ui.double_click_attack_move_guard_hint_timer(), i32::from(ui.is_in_attack_move_to_mode())]);
    (input, rust)
}

/// #123, #124: `CommandTranslator::translateGameMessage`'s mouse cases (the gate, then the cut from
/// `MSG_MOUSEOVER_DRAWABLE_HINT` to the ALT-F4 demo case) against `mouse_over_with`, `click_with` and
/// `double_click_with`, the evaluation replaced on both sides by a log of the call.
///
/// # Method
///
/// Each case is random: one of the six mouse messages (the drawable hint, the location hint, the left and right click and
/// double click; the double clicks weighted up), the input on 7 times in 8, `m_useAlternateMouse` and
/// `m_doubleClickAttackMove` each half the time, 0 to 3 selected (all locally controlled or none), a pending GUI command a
/// third of the time none, else the shortcut special power three times in twelve or one of the firing commands, sabotage,
/// attack move, guard or any type below 35; force attack mode a quarter of the time, attack-move mode half; the pixel
/// region a point half the time; the picked drawable none a third of the time. Compared: the disposition, every
/// `evaluateForceAttack` / `evaluateContextCommand` call (drawable, position bits, type), every message appended with its
/// arguments, the academy's records, the guard hint's timer and attack-move mode after. ~~`TheMouse->isClick` (the right
/// button's bookkeeping, not translated) answers a click.~~ **Corrected 2026-11-11 (#131):** the right button's
/// bookkeeping is translated; `TheMouse->isClick` answers a click 3 times in 4 on both sides (the stand-in's answer).
/// The bookkeeping itself is `tests/differential_meta_input.rs`'.
///
/// *Predictions, before the first run* (`issue123_progress.md`): 0 mismatches; every plant 9101-9114 caught.
///
/// # Results (2026-11-11, `COMMAND_MOUSE_CASES=20000`, the default)
///
/// 20,000 cases: **0 mismatches**. 6,246 made an evaluation, 1,920 a double-click guard order (the predicted quarter of
/// the 8,750 double clicks, less the gated eighth), 3,944 were destroyed, and 137 were drawable hints with nothing
/// selected (the shortcut special power, #123). The first run reported 877 window and 2,364 mouse mismatches: not the
/// translation but the harness, the two tests calling the wrapper's global state from two threads at once; `cpp_case`
/// now holds a lock, and the unchanged first test's numbers came back as before (1,001 destroyed, 2 movies stopped).
///
/// **Re-run 2026-11-11 (#131),** the generator now drawing `isClick` (a click 3 times in 4): 20,000 cases, **0
/// mismatches**; 6,121 evaluated, 1,968 guard orders, 3,953 destroyed, 123 drawable hints with nothing selected. The
/// plants' counts below are this run's; the earlier ones are struck through.
///
/// # Planted bugs
///
/// `L9_PLANT=<id>`: **14 of 14 caught** (2026-11-11).
///
/// | id | planted bug | caught by |
/// |---|---|---|
/// | 9101 | the drawable hint without the shortcut term (the code before #123) | ~~137 cases~~ 123 cases (#131) |
/// | 9102 | the drawable hint for any pending GUI command | ~~405 cases~~ 433 cases (#131) |
/// | 9103 | the left double click guards in alternate mouse mode too | ~~1,085 cases~~ 1,061 cases (#131) |
/// | 9104 | the guard order's mode `GUARDMODE_GUARD_WITHOUT_PURSUIT` | ~~1,920 cases~~ 1,968 cases (#131) |
/// | 9105 | the guard order destroys the message | ~~1,920 cases~~ 1,968 cases (#131) |
/// | 9106 | no `triggerDoubleClickAttackMoveGuardHint` | ~~1,920 cases~~ 1,968 cases (#131) |
/// | 9107 | the guard order falls through to the click as well | ~~448 cases~~ 410 cases (#131) |
/// | 9108 | the click without the shortcut's controllable term (the code before #124) | ~~383 cases~~ 405 cases (#131) |
/// | 9109 | the alternate mouse's left click ignores firing GUI commands (the code before #124) | ~~490 cases~~ 512 cases (#131) |
/// | 9110 | the firing commands without `GUI_COMMAND_COMBATDROP` | ~~40 cases~~ 38 cases (#131) |
/// | 9111 | the right double click's alternate-mouse test inverted | ~~1,651 cases~~ 1,647 cases (#131) |
/// | 9112 | the click does not clear attack-move mode | ~~682 cases~~ 708 cases (#131) |
/// | 9113 | no academy record | ~~1,920 cases~~ 1,968 cases (#131) |
/// | 9114 | the drawable hint at the ground, not the drawable's position | ~~2,642 cases~~ 2,554 cases (#131) |
#[test]
fn command_mouse_cases_match_upstream() {
    let cases: u64 = std::env::var("COMMAND_MOUSE_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x123_1240);
    let mut c = Check::new("CommandTranslator mouse cases");
    let mut stats = [0u64; 4];
    for i in 0..cases {
        let (input, rust) = mouse_case(&mut g, &mut stats);
        c.eq(rust, cpp_case(4, &input), || format!("case {i} {input:?}"));
    }
    println!("mouse cases: {} evaluated, {} guard orders, {} destroyed, {} shortcut hints with nothing selected", stats[0], stats[1], stats[2], stats[3]);
    if planted {
        c.report();
    } else {
        c.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED lines (a mismatch is the plant caught)");
}

// ---- #132: the selection translator's right button ----

use rts_lockstep_engine::client::selection_xlat::DragTolerances;

/// One sequence's fixed facts: `TheMouse`'s tolerances, the mouse mode, and the clock's start.
struct RightSequence {
    tol: DragTolerances,
    alternate: bool,
    time: u32,
    /// The C++ translator's state, carried from its own output: anchor x, y, `m_lastClick`, camera x, y, z bits.
    cpp_state: [i32; 6],
}

/// One message of a right-button sequence through both sides; the Rust translator `x` is carried by the caller.
fn right_message(g: &mut Gen, seq: &mut RightSequence, x: &mut SelectionTranslator, stats: &mut [u64; 5]) -> (Vec<i32>, Vec<i32>) {
    // a press two times in five, a release two in five, else the other raw button messages' neighbours (kept by both)
    let t = match g.below(10) {
        0..=3 => MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN,
        4..=8 => MSG_RAW_MOUSE_RIGHT_BUTTON_UP,
        _ => [MSG_RAW_MOUSE_RIGHT_DOUBLE_CLICK, MSG_RAW_MOUSE_RIGHT_DRAG, MSG_RAW_MOUSE_POSITION][g.below(3) as usize],
    };
    let on = g.below(8) != 0;
    let (drag, selecting) = (g.below(4) == 0, g.below(2) == 1);
    // the pointer near the last press (within twice the tolerance) most of the time, anywhere one time in eight
    // (the press's own pixel a third of the time, so that clicks are common)
    let pixel = if g.below(3) == 0 {
        x.deselect_feedback_anchor
    } else if g.below(8) == 0 {
        ICoord2D { x: g.i32(), y: g.i32() }
    } else {
        let r = (seq.tol.pixels as i32).saturating_mul(2).clamp(1, 1 << 20);
        let a = x.deselect_feedback_anchor;
        ICoord2D { x: a.x.wrapping_add(g.below(2 * r as u32 + 1) as i32 - r), y: a.y.wrapping_add(g.below(2 * r as u32 + 1) as i32 - r) }
    };
    // the clock: forward by up to twice the time tolerance, sometimes back, and wrapping past 2^32
    seq.time = match g.below(10) {
        0 => seq.time.wrapping_sub(g.below(500)),
        1 => g.u32(),
        2..=6 => seq.time.wrapping_add(g.below(seq.tol.ms.saturating_add(1))),
        _ => seq.time.wrapping_add(g.below(seq.tol.ms.saturating_mul(2).clamp(1, 1 << 30))),
    };
    // the camera at the press's point half the time, else moved by up to twice the 3-D tolerance on each axis
    let base = x.deselect_down_camera_position;
    let camera = if g.below(2) == 0 {
        base
    } else {
        let w = if seq.tol.world == 0 { 2.0f32 } else { seq.tol.world as f32 * 2.0 };
        Coord3D { x: base.x + g.uniform(-w, w), y: base.y + g.uniform(-w, w), z: base.z + g.uniform(-w, w) }
    };
    let command = g.below(3) == 0;
    let place = if g.below(4) == 0 { 1 + g.below(9) } else { 0 };
    let input = vec![
        t.0 as i32,
        i32::from(on),
        i32::from(drag),
        i32::from(selecting),
        pixel.x,
        pixel.y,
        seq.time as i32,
        camera.x.to_bits() as i32,
        camera.y.to_bits() as i32,
        camera.z.to_bits() as i32,
        seq.tol.pixels as i32,
        seq.tol.world as i32,
        seq.tol.ms as i32,
        i32::from(seq.alternate),
        i32::from(command),
        place as i32,
        seq.cpp_state[0],
        seq.cpp_state[1],
        seq.cpp_state[2],
        seq.cpp_state[3],
        seq.cpp_state[4],
        seq.cpp_state[5],
    ];

    let mut h = MouseHost::default();
    let mut ui = InGameUiSelection::new();
    ui.set_input_enabled(on);
    ui.set_selecting(selecting);
    for d in 1..=g.below(3) {
        ui.select_drawable(d as DrawableId, &mut h);
    }
    if command {
        ui.set_gui_command(Some(GuiButton { command_type: 0, options: opt::NEED_TARGET_POS, ..Default::default() }));
    }
    if place != 0 {
        ui.place_build_available(Some(1), Some(place as DrawableId), 0.0, &h);
    }
    x.drag_selecting = drag;
    let mut scroll_stops = 0;
    let d = if t == MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN {
        x.right_button_down(pixel, seq.time, camera, &mut ui)
    } else if t == MSG_RAW_MOUSE_RIGHT_BUTTON_UP {
        x.right_button_up(pixel, seq.time, camera, seq.tol, seq.alternate, &mut ui, &mut h, |b| scroll_stops += i32::from(!b))
    } else {
        // (no case: only the gate)
        x.input_disabled_gate(&mut ui).unwrap_or(Disposition::Keep)
    };
    let cleared = i32::from(command && ui.gui_command().is_none());
    stats[0] += u64::from(t == MSG_RAW_MOUSE_RIGHT_BUTTON_UP && on);
    stats[1] += u64::from(d == Disposition::Destroy);
    stats[2] += u64::from(h.destroy_groups > 0);
    stats[3] += u64::from(cleared > 0);
    stats[4] += u64::from(t == MSG_RAW_MOUSE_RIGHT_BUTTON_UP && on && !command && h.destroy_groups == 0 && (!seq.alternate || place != 0));
    let a = x.deselect_feedback_anchor;
    let c = x.deselect_down_camera_position;
    let rust = vec![
        disp(d),
        a.x,
        a.y,
        x.last_click as i32,
        c.x.to_bits() as i32,
        c.y.to_bits() as i32,
        c.z.to_bits() as i32,
        i32::from(x.drag_selecting),
        i32::from(ui.is_selecting()),
        h.destroy_groups,
        cleared,
        i32::from(ui.gui_command().is_some()),
        scroll_stops,
    ];
    (input, rust)
}

/// #132: `SelectionTranslator::translateGameMessage`'s right button (the gate, then the cut from
/// `MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN` to `MSG_META_CREATE_TEAM0`, SelectionXlat.cpp:977-1050) against
/// `SelectionTranslator::right_button_down` and `right_button_up`, over random sequences of messages.
///
/// # Method
///
/// Each sequence draws `TheMouse`'s tolerances (pixels 0-7, or up to 2^31 one time in eight; 3-D 0-39; milliseconds
/// 0-399, or 0 or past 2^31 one time in eight each), the mouse mode (alternate half the time) and a start time anywhere in
/// the 32-bit clock, then one to eight messages: the press and the release (four and five times in ten), or a double
/// click, drag or position message, which only the gate sees. Each message draws the input (on 7 times in 8), the drag
/// selection and the InGameUI's selecting flag, the pixel (the last press's a third of the time, else near it, within
/// twice the tolerance, or anywhere one time in eight), the time (forward within the tolerance half the time, else by up
/// to twice it, back by up to 499 ms, or anywhere: the unsigned difference wraps), the camera (at the press's position half the time, else moved by up to twice the 3-D tolerance per
/// axis), a pending GUI command (a third of the time), a building waiting to be placed (a quarter), and 0 to 2 selected
/// drawables. The Rust translator is carried through the sequence; the C++ one starts each message from the state its
/// own last message returned, so a divergence in state shows. Compared after each message: the disposition, the
/// translator's three right-button members (the camera as bits), the drag selection and selecting flags,
/// `deselectAllDrawables` calls (the Rust posts `MSG_DESTROY_SELECTED_GROUP`), `setGUICommand(NULL)` calls, whether a
/// GUI command is still pending, and `setScrolling(FALSE)` calls.
///
/// *Predictions, before the first run* (`issue134_progress.md`): 0 mismatches; every plant 9211-9221 caught; about a
/// third of the 20,000 sequences' releases are clicks.
///
/// # Results (2026-11-11, `SELECTION_RIGHT_SEQUENCES=20000`, the default)
///
/// First run: 20,000 sequences, 89,951 messages, **0 mismatches**, but clicks were rare (a miss): of 39,552 releases
/// with the input on, 599 cancelled a GUI command and 657 deselected, about 3 %, because the pointer, the clock and the
/// camera each left their tolerance about half the time. The generator now gives the press's own pixel a third of the
/// time and a time within the tolerance half the time. Second run: 20,000 sequences, 89,537 messages, **0 mismatches**;
/// 39,340 releases with the input on, 1,177 cancelled a GUI command (destroyed), 1,486 deselected all, 14,913 were
/// kept by a drag or the mode (6.8 % clicks, still under the predicted share).
///
/// # Planted bugs
///
/// `L9_PLANT=<id>`: **11 of 11 caught** (2026-11-11, the second run's sequences).
///
/// | id | planted bug | caught by (messages) |
/// |---|---|---|
/// | 9211 | the press's time is not kept | 60,569 |
/// | 9212 | the time's difference reversed | 2,098 |
/// | 9213 | the 3-D tolerance one unit larger | 82 |
/// | 9214 | a drag counts as a click | 26,820 |
/// | 9215 | the cancel leaves the GUI command | 1,177 |
/// | 9216 | the cancel keeps the message | 1,177 |
/// | 9217 | the cancel does not stop the scroll | 1,177 |
/// | 9218 | the alternate mouse ignores a pending placement | 309 |
/// | 9219 | the mode's `||` made `&&` | 1,201 |
/// | 9220 | the pixel test `>=` | 561 |
/// | 9221 | the pixel test ignores y | 614 |
#[test]
fn selection_right_button_matches_upstream() {
    let sequences: u64 = std::env::var("SELECTION_RIGHT_SEQUENCES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x132_1320);
    let mut c = Check::new("SelectionTranslator right button");
    let mut stats = [0u64; 5];
    let mut messages = 0u64;
    for i in 0..sequences {
        let tol = DragTolerances {
            pixels: if g.below(8) == 0 { g.below(1 << 31) } else { g.below(8) },
            world: g.below(40),
            ms: match g.below(8) {
                0 => 0,
                1 => (1u32 << 31) + g.below(1 << 31),
                _ => g.below(400),
            },
        };
        let mut seq = RightSequence { tol, alternate: g.below(2) == 1, time: g.u32(), cpp_state: [0, 0, 0, 0, 0, 0] };
        let mut x = SelectionTranslator::default();
        for m in 0..1 + g.below(8) {
            let (input, rust) = right_message(&mut g, &mut seq, &mut x, &mut stats);
            let cpp = cpp_case(5, &input);
            if cpp.len() >= 7 {
                seq.cpp_state.copy_from_slice(&cpp[1..7]);
            }
            messages += 1;
            c.eq(rust, cpp, || format!("sequence {i} message {m} {input:?}"));
        }
    }
    println!(
        "right button: {sequences} sequences, {messages} messages; {} releases with the input on, {} destroyed, {} deselected all, {} GUI commands cancelled, {} kept by a drag or the mode",
        stats[0], stats[1], stats[2], stats[3], stats[4]
    );
    if planted {
        c.report();
    } else {
        c.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED lines (a mismatch is the plant caught)");
}
