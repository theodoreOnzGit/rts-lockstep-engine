#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (setRadiusCursor, handleRadiusCursor,
//     triggerDoubleClickAttackMoveGuardHint)
//   GeneralsMD/Code/GameEngine/Source/GameClient/RadiusDecal.cpp (createRadiusDecal, clear, update, setOpacity, setPosition;
//     #130: ~RadiusDecal, for the logic's own decals)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::radius_cursor` (GitHub #124) against the C++. **(#130, batch #134)** And the client's halves of the logic's own
//! radius decals (`LogicRadiusDecals`): see [`the_logics_decals_match_upstream`].
//!
//! # Method
//!
//! The C++ side is InGameUI's `setRadiusCursor`, `handleRadiusCursor` and `triggerDoubleClickAttackMoveGuardHint` and
//! RadiusDecal.cpp's template constructor, `createRadiusDecal`, `RadiusDecal()`, `clear`, `update`, `setOpacity` and
//! `setPosition`, with `RadiusDecal.h`'s classes and InGameUI.h's `enum RadiusCursorType`, cut verbatim by build.rs
//! (`differential/wrap_radius_cursor.cpp` lists the cuts and the stand-ins).
//!
//! Each case is 30 random templates (with or without a texture, a style, opacities from -0.5 to 1.5, a throb of 1..90
//! frames, colour 0 or random, visible to the owner only or not), a local player 0..3 with four colours, four objects
//! (controlled by no one or a player; a guard range; three weapon slots, each with or without a weapon, with radii
//! from -10 to 300, zero included) and four special power templates (types, radius cursor radii, the most ready
//! shortcut object of the type), then 1 to 60 steps: the selection and the pending command (none, the shortcut special
//! power, another), `setRadiusCursor` of any type (the current one again too) with no power or any, and a slot;
//! `handleRadiusCursor`; `triggerDoubleClickAttackMoveGuardHint`; the guard hint's timer set (-3..12, as
//! `createCommandHint` counts it down); the environment (the mouse, the radar on and hitting, the logic frame, the icon
//! UI, `m_doubleClickAttackMove`). Both sides write the decal (empty, its template, the current type, the projected
//! decal's texture, style, size, angle, colour, opacity and position) and the stash after the setup and every step.
//!
//! # Predictions (written before the first run, 2026-11-11)
//!
//! - 0 mismatches over 20,000 sequences: the opacity's `sin` is petir's `sinf` on both sides (the differential build
//!   binds the C side's libm to petir), the rest is copying and comparison.
//! - Planted bugs 9175-9180 all caught.
//!
//! # Results (2026-11-11, `RADIUS_CURSOR_CASES=20000`, the default)
//!
//! 20,000 sequences, 610,859 steps: **0 mismatches** on the first full run. A projected decal existed after 54,394 steps,
//! 40,743 of them with the guard hint's timer above 0, and 43,977 with a positive opacity.
//!
//! Planted bugs (same 20,000 sequences): **6 of 6 caught**, as predicted: 9175 by 4,585 sequences, 9176 5,505, 9177 5,680,
//! 9178 4,508, 9179 1,133, 9180 1,730.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 9175 | the throb's period is one frame longer |
//! | 9176 | the icon UI's switch is ignored |
//! | 9177 | the decal is drawn for every player |
//! | 9178 | `setRadiusCursor` does not stop at the current type |
//! | 9179 | the guard area takes the power's radius |
//! | 9180 | the radar's point is never taken |

mod support;

use rts_lockstep_engine::client::in_game_ui::DrawableId;
use rts_lockstep_engine::client::radius_cursor::{InGameUiRadiusCursor, RadiusCursorHost, RadiusUiState};
use rts_lockstep_engine::common::base_type::{Coord3D, ICoord2D};
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

const SETUP: usize = 30 * 8 + 8 + 4 * 24 + 4 * 8;

fn real(w: i32) -> f32 {
    f32::from_bits(w as u32)
}

/// The setup's world and the step's environment.
#[derive(Default)]
struct Host {
    local: i32,
    colors: [i32; 4],
    /// By object id 1..4: controller, guard, (has, dmg, scatter, scalar, cont) per slot.
    objects: Vec<(i32, f32, [(bool, f32, f32, f32, f32); 3])>,
    powers: Vec<(i32, f32, i32)>,
    mouse: ICoord2D,
    radar_on: bool,
    radar_hits: bool,
    frame: u32,
    icons: bool,
    dcam: bool,
}

impl Host {
    fn weapon(&self, obj: ObjectId, slot: i32) -> Option<(f32, f32, f32, f32)> {
        let w = self.objects[obj as usize - 1].2[slot as usize];
        w.0.then_some((w.1, w.2, w.3, w.4))
    }
}

impl RadiusCursorHost for Host {
    fn radius_drawable_object(&self, draw: DrawableId) -> Option<ObjectId> {
        (1..=4).contains(&draw).then_some(draw)
    }
    fn radius_most_ready_shortcut_special_power(&mut self, sp: i32) -> Option<ObjectId> {
        let p = self.powers.iter().find(|p| p.0 == sp)?;
        (p.2 != 0).then_some(p.2 as ObjectId)
    }
    fn special_power_radius(&self, sp: u32) -> Option<(i32, f32)> {
        self.powers.get(sp as usize).map(|p| (p.0, p.1))
    }
    fn radius_controlling_player(&self, obj: ObjectId) -> Option<(i32, i32)> {
        let c = self.objects[obj as usize - 1].0;
        (c >= 0).then(|| (c, self.colors[c as usize]))
    }
    fn weapon_primary_damage_radius(&mut self, obj: ObjectId, slot: i32) -> Option<f32> {
        self.weapon(obj, slot).map(|w| w.0)
    }
    fn weapon_scatter_radius(&mut self, obj: ObjectId, slot: i32) -> Option<f32> {
        self.weapon(obj, slot).map(|w| w.1 + w.2)
    }
    fn weapon_continue_attack_range(&mut self, obj: ObjectId, slot: i32) -> Option<f32> {
        self.weapon(obj, slot).map(|w| w.3)
    }
    fn std_guard_range(&mut self, obj: ObjectId) -> f32 {
        self.objects[obj as usize - 1].1
    }
    fn radius_local_player(&self) -> i32 {
        self.local
    }
    fn radius_mouse_pos(&self) -> ICoord2D {
        self.mouse
    }
    fn radius_radar_on(&self) -> bool {
        self.radar_on
    }
    fn radar_screen_pixel_to_world(&self, p: ICoord2D) -> Option<Coord3D> {
        self.radar_hits.then(|| Coord3D { x: 2.0 * p.x as f32, y: 2.0 * p.y as f32, z: 7.0 })
    }
    fn radius_screen_to_terrain(&mut self, p: ICoord2D) -> Coord3D {
        Coord3D { x: p.x as f32, y: p.y as f32, z: 3.0 }
    }
    fn radius_double_click_attack_move(&self) -> bool {
        self.dcam
    }
    fn radius_logic_frame(&self) -> u32 {
        self.frame
    }
    fn draw_icon_ui(&self) -> bool {
        self.icons
    }
}

fn hash(b: &[u8]) -> i32 {
    let mut h: u32 = 2166136261;
    for &c in b {
        h ^= u32::from(c);
        h = h.wrapping_mul(16777619);
    }
    h as i32
}

fn state(rc: &InGameUiRadiusCursor, ui: &RadiusUiState, out: &mut Vec<i32>) {
    let d = &rc.cur_radius_cursor;
    out.extend([300, i32::from(d.empty), d.template.unwrap_or(-1), rc.cur_rc_type, i32::from(d.decal.is_some()), ui.guard_hint_timer, 0, 0]);
    match &d.decal {
        Some(s) => {
            out.extend([301, hash(&s.texture), s.shadow_type as i32, s.size.0.to_bits() as i32, s.size.1.to_bits() as i32, s.angle.to_bits() as i32, s.color, s.opacity.unwrap_or(-1)]);
            out.extend([302, s.position.x.to_bits() as i32, s.position.y.to_bits() as i32, s.position.z.to_bits() as i32]);
        }
        None => {
            out.extend([301, 0, 0, 0, 0, 0, 0, 0]);
            out.extend([302, 0, 0, 0]);
        }
    }
    let st = ui.guard_hint_stash;
    out.extend([st.x.to_bits() as i32, st.y.to_bits() as i32, st.z.to_bits() as i32, 0]);
}

fn rust_seq(input: &[i32]) -> Vec<i32> {
    let mut out = Vec::new();
    let mut rc = InGameUiRadiusCursor::default();
    for i in 0..30 {
        let w = &input[i * 8..i * 8 + 8];
        let t = &mut rc.radius_cursors[i];
        t.name = if w[0] != 0 { format!("RC{i}").into_bytes() } else { Vec::new() };
        t.shadow_type = w[1] as u32;
        t.min_opacity = real(w[2]);
        t.max_opacity = real(w[3]);
        t.opacity_throb_time = w[4] as u32;
        t.color = w[5];
        t.only_visible_to_owning_player = w[6] != 0;
    }
    let g = &input[240..248];
    let mut h = Host { local: g[0], colors: [g[1], g[2], g[3], g[4]], icons: true, ..Default::default() };
    for o in 0..4 {
        let w = &input[248 + o * 24..248 + o * 24 + 24];
        let slot = |s: usize| (w[2 + s * 5] != 0, real(w[3 + s * 5]), real(w[4 + s * 5]), real(w[5 + s * 5]), real(w[6 + s * 5]));
        h.objects.push((w[0], real(w[1]), [slot(0), slot(1), slot(2)]));
    }
    for p in 0..4 {
        let w = &input[344 + p * 8..344 + p * 8 + 8];
        h.powers.push((w[0], real(w[1]), w[2]));
    }
    let mut ui = RadiusUiState::default();
    state(&rc, &ui, &mut out);
    for w in input[SETUP..].chunks(8) {
        match w[0] {
            0 => {
                ui.select_count = w[1];
                ui.first_selected = (w[2] != 0).then_some(w[2] as DrawableId);
                ui.shortcut_pending = w[3] == 31;
            }
            1 => rc.set_radius_cursor(&ui, w[1], (w[2] >= 0).then_some(w[2] as u32), w[3], &mut h),
            2 => rc.handle_radius_cursor(&ui, &mut h),
            3 => {
                // `triggerDoubleClickAttackMoveGuardHint`: the timer and the terrain under the mouse
                ui.guard_hint_timer = 11;
                ui.guard_hint_stash = h.radius_screen_to_terrain(h.mouse);
            }
            4 => ui.guard_hint_timer = w[1],
            _ => {
                h.mouse = ICoord2D { x: w[1], y: w[2] };
                h.radar_on = w[3] != 0;
                h.radar_hits = w[4] != 0;
                h.frame = w[5] as u32;
                h.icons = w[6] != 0;
                h.dcam = w[7] != 0;
            }
        }
        state(&rc, &ui, &mut out);
    }
    out
}

/// (#130, batch #134) The wrapper's globals (the shadows, the players, the logic's frame, the output) are shared by its two
/// entry points, and the two tests run on two threads: each call holds this lock.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let _held = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cap = 8 * 3 * (input.len() / 8 + 2);
    let mut out = vec![0i32; cap];
    let n = unsafe { cpp::ref_radius_cursor_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), cap as i32) };
    out.truncate(n as usize);
    out
}

fn radius(g: &mut Gen) -> f32 {
    match g.below(5) {
        0 => 0.0,
        1 => g.uniform(-10.0, 0.0),
        _ => g.uniform(0.0, 300.0),
    }
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    let mut v = Vec::new();
    for _ in 0..30 {
        let color = if g.below(2) == 0 { 0 } else { g.i32() };
        v.extend([
            i32::from(g.below(5) != 0),
            [0x20, 0x40, 0x60, 0][g.below(4) as usize],
            g.uniform(-0.5, 1.5).to_bits() as i32,
            g.uniform(-0.5, 1.5).to_bits() as i32,
            1 + g.below(90) as i32,
            color,
            g.below(2) as i32,
            0,
        ]);
    }
    v.extend([g.below(4) as i32, g.i32(), g.i32(), g.i32(), g.i32(), 0, 0, 0]);
    for _ in 0..4 {
        let mut w = vec![g.below(5) as i32 - 1, radius(g).to_bits() as i32];
        for _ in 0..3 {
            w.extend([i32::from(g.below(4) != 0), radius(g).to_bits() as i32, radius(g).to_bits() as i32, radius(g).to_bits() as i32, radius(g).to_bits() as i32]);
        }
        w.resize(24, 0);
        v.extend(w);
    }
    for _ in 0..4 {
        v.extend([g.below(6) as i32, radius(g).to_bits() as i32, g.below(5) as i32, 0, 0, 0, 0, 0]);
    }
    let n = 1 + g.below(60) as usize;
    let mut last_type = 0;
    for _ in 0..n {
        let s = match g.below(12) {
            0 | 1 => [0, g.below(3) as i32, [0, 1, 2, 3, 4, 9][g.below(6) as usize], [-1, 31, 5][g.below(3) as usize], 0, 0, 0, 0],
            2..=5 => {
                let t = if g.below(4) == 0 { last_type } else { g.below(30) as i32 };
                last_type = t;
                [1, t, g.below(5) as i32 - 1, g.below(3) as i32, 0, 0, 0, 0]
            }
            6 | 7 => [2, 0, 0, 0, 0, 0, 0, 0],
            8 => [3, 0, 0, 0, 0, 0, 0, 0],
            9 => [4, g.below(16) as i32 - 3, 0, 0, 0, 0, 0, 0],
            _ => [5, g.below(1000) as i32 - 100, g.below(800) as i32 - 100, g.below(2) as i32, g.below(2) as i32, g.u32() as i32, i32::from(g.below(6) != 0), g.below(2) as i32],
        };
        v.extend(s);
    }
    v
}

#[test]
fn the_radius_cursor_matches_upstream() {
    let cases: u64 = std::env::var("RADIUS_CURSOR_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x124_1241);
    let mut whole = Check::new("the radius cursor sequences");
    let (mut steps, mut decals, mut faded, mut throbbing) = (0u64, 0u64, 0u64, 0u64);
    for i in 0..cases {
        let input = gen_case(&mut g);
        steps += ((input.len() - SETUP) / 8) as u64;
        let r = rust_seq(&input);
        let c = cpp_seq(&input);
        for rec in c.chunks(8).collect::<Vec<_>>().chunks(3) {
            if rec[0][4] == 1 {
                decals += 1;
                if rec[0][5] > 0 && rec[1][7] >= 0 {
                    faded += 1;
                }
                if rec[1][7] > 0 {
                    throbbing += 1;
                }
            }
        }
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len())) / 8 * 8;
            println!("sequence {i}: rust {:?} c++ {:?}", &r[k.min(r.len())..(k + 24).min(r.len())], &c[k.min(c.len())..(k + 24).min(c.len())]);
        }
        whole.eq(r, c, || format!("sequence {i}"));
    }
    println!("{cases} sequences, {steps} steps: a projected decal after {decals} steps (with the guard hint's timer up {faded}, a positive opacity {throbbing})");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}

// ---- #130 (batch #134): the logic's own radius decals ----

use rts_lockstep_engine::client::radius_cursor::{LogicRadiusDecalHost, LogicRadiusDecals};
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::radius_decal::RadiusDecalTemplate;
use rts_lockstep_engine::logic::world::object_events::{GridDecalOp, ObjectClientEvent};

/// The players' colours, the local player and the icon flag of the step.
struct DecalHost {
    local: i32,
    colors: [i32; 4],
    icons: bool,
}

impl LogicRadiusDecalHost for DecalHost {
    fn decal_player_color(&self, p: i32) -> Option<i32> {
        usize::try_from(p).ok().and_then(|i| self.colors.get(i).copied())
    }
    fn decal_local_player(&self) -> i32 {
        self.local
    }
    fn decal_draw_icon_ui(&self) -> bool {
        self.icons
    }
}

fn cpp_decals(input: &[i32]) -> Vec<i32> {
    let _held = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cap = 72 * (input.len() / 16 + 1);
    let mut out = vec![0i32; cap];
    let n = unsafe { cpp::ref_logic_decals_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), cap as i32) };
    out.truncate(n as usize);
    out
}

/// The Rust side of one sequence: each step made into the client event the logic would send (object `slot / 2 + 1`,
/// index `slot % 2`) and given to `LogicRadiusDecals::take_events`.
fn rust_decals(input: &[i32], stats: &mut [u64; 3]) -> Vec<i32> {
    let mut h = DecalHost { local: input[0], colors: [input[1], input[2], input[3], input[4]], icons: true };
    let mut d = LogicRadiusDecals::default();
    let mut out = Vec::new();
    let mut now = 0u32;
    for w in input[5..].chunks_exact(16) {
        let slot = w[1];
        let (object, index) = (slot / 2 + 1, (slot % 2) as u32);
        let op = match w[0] {
            0 => {
                let template = RadiusDecalTemplate {
                    name: if w[2] != 0 { format!("T{slot}").into_bytes() } else { Vec::new() },
                    shadow_type: w[3] as u32,
                    min_opacity: real(w[4]),
                    max_opacity: real(w[5]),
                    opacity_throb_time: w[6] as u32,
                    color: w[7],
                    only_visible_to_owning_player: w[8] != 0,
                };
                let position = Coord3D { x: real(w[10]), y: real(w[11]), z: real(w[12]) };
                Some(GridDecalOp::Create { template, radius: real(w[9]), position, player: (w[13] >= 0).then_some(w[13]) })
            }
            1 => Some(GridDecalOp::Clear),
            2 => Some(GridDecalOp::SetPosition(Coord3D { x: real(w[2]), y: real(w[3]), z: real(w[4]) })),
            3 => Some(GridDecalOp::SetOpacity(real(w[2]))),
            4 => {
                now = w[2] as u32;
                h.icons = w[3] != 0;
                Some(GridDecalOp::Update)
            }
            _ => None,
        };
        let ev = match op {
            Some(op) => ClientEvent::Object(ObjectClientEvent::GridDecal { object_id: object, index, op }),
            None => ClientEvent::ObjectDestroyed { object_id: object },
        };
        d.take_events(&[ev], now, &h);
        for s in 0..6 {
            let e = d.get(s / 2 + 1, (s % 2) as u32);
            let empty = e.is_none_or(|x| x.is_empty());
            match e.and_then(|x| x.decal.as_ref()) {
                Some(sh) => {
                    stats[0] += 1;
                    stats[1] += u64::from(sh.opacity.is_some_and(|o| o > 0));
                    out.extend([
                        i32::from(empty),
                        1,
                        hash(&sh.texture),
                        sh.shadow_type as i32,
                        sh.size.0.to_bits() as i32,
                        sh.size.1.to_bits() as i32,
                        sh.angle.to_bits() as i32,
                        sh.color,
                        sh.opacity.unwrap_or(-1),
                        sh.position.x.to_bits() as i32,
                        sh.position.y.to_bits() as i32,
                        sh.position.z.to_bits() as i32,
                    ]);
                }
                None => {
                    stats[2] += u64::from(!empty);
                    out.extend([i32::from(empty), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
                }
            }
        }
    }
    out
}

fn gen_decals(g: &mut Gen) -> Vec<i32> {
    let mut v = vec![g.below(4) as i32, g.i32(), g.i32(), g.i32(), g.i32()];
    for _ in 0..1 + g.below(40) {
        let slot = g.below(6) as i32;
        let mut w = [0i32; 16];
        w[1] = slot;
        let pos = |g: &mut Gen| g.uniform(-500.0, 3000.0).to_bits() as i32;
        match g.below(10) {
            0 | 1 => {
                w[0] = 0;
                w[2] = i32::from(g.below(6) != 0);
                w[3] = [0x20, 0x40, 0x60, 0][g.below(4) as usize];
                w[4] = g.uniform(-0.5, 1.5).to_bits() as i32;
                w[5] = g.uniform(-0.5, 1.5).to_bits() as i32;
                w[6] = 1 + g.below(90) as i32;
                w[7] = if g.below(2) == 0 { 0 } else { g.i32() };
                w[8] = g.below(2) as i32;
                w[9] = radius(g).to_bits() as i32;
                w[10] = pos(g);
                w[11] = pos(g);
                w[12] = g.uniform(-5.0, 50.0).to_bits() as i32;
                w[13] = g.below(5) as i32 - 1;
            }
            2 => w[0] = 1,
            3 | 4 => {
                w[0] = 2;
                w[2] = pos(g);
                w[3] = pos(g);
                w[4] = g.uniform(-5.0, 50.0).to_bits() as i32;
            }
            5 => {
                w[0] = 3;
                w[2] = g.uniform(-0.5, 1.5).to_bits() as i32;
            }
            6..=8 => {
                w[0] = 4;
                w[2] = g.u32() as i32;
                w[3] = i32::from(g.below(6) != 0);
            }
            _ => w[0] = 5,
        }
        v.extend(w);
    }
    v
}

/// #130: `LogicRadiusDecals` (the client's halves of the logic's own radius decals, fed the logic's `GridDecal` and
/// `ObjectDestroyed` events) against six real `RadiusDecal`s, each with its module's template.
///
/// # Method
///
/// The C++ side is RadiusDecal.cpp's `createRadiusDecal`, `RadiusDecal()`, `clear`, `update`, `setOpacity`,
/// `setPosition` and `~RadiusDecal`, the same cuts as the cursor's (`ref_logic_decals_seq`, documented in
/// `differential/wrap_radius_cursor.cpp`). Each sequence draws a local player 0..3 and four colours, then 1 to 40 steps
/// on one of six decals (objects 1..3, decals 0 and 1 each): a create two times in ten (a texture five times in six,
/// any of four styles, opacities from -0.5 to 1.5, a throb of 1..90 frames, colour 0 or random, owner-only or not, a
/// radius 0, negative or up to 300, a position, an owner none or 0..3), a clear, a move (two in ten), an opacity, an
/// update at any frame with the icons on five times in six (three in ten), or the object's destruction (both its
/// decals' destructors). The Rust side turns each step into the event the logic sends and gives it to
/// `take_events`; the frame of an update is the `now` passed. Compared after every step, for all six decals: empty, the
/// projected decal or none, and its texture, style, size, angle, colour, opacity and position.
///
/// *Predictions, before the first run* (`issue134_progress.md`): 0 mismatches over 20,000 sequences; plants 9222-9227 all
/// caught; a projected decal on about a third of the slot-steps (`issue134_progress.md` said a quarter).
///
/// # Results (2026-11-11, `LOGIC_DECAL_CASES=20000`, the default)
///
/// 20,000 sequences, 411,182 steps: **0 mismatches** on the first run. Of the 2,467,092 slot-steps compared, 151,669 had
/// a projected decal (6 %, a miss: most creates fail on the owner, the radius or the visibility), 31,496 of them with a
/// positive opacity; 90,408 were not empty but not drawn (another player's owner-only decal). The cursor's own test,
/// unchanged, gave its earlier numbers (610,859 steps, 54,394 decals, 0 mismatches) with the shadows' free list added.
///
/// # Planted bugs
///
/// `L9_PLANT=<id>`: **6 of 6 caught** (2026-11-11).
///
/// | id | planted bug | caught by (sequences) |
/// |---|---|---|
/// | 9222 | a clear is ignored | 3,408 |
/// | 9223 | the object's destruction keeps its decals | 5,739 |
/// | 9224 | the throb at the frame after the one the module ran in | 3,760 |
/// | 9225 | `setOpacity` is ignored | 1,971 |
/// | 9226 | every decal is visible (the owner taken for the local player) | 8,763 |
/// | 9227 | the decal's index ignored (one decal per object) | 12,355 |
#[test]
fn the_logics_decals_match_upstream() {
    let cases: u64 = std::env::var("LOGIC_DECAL_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x130_1300);
    let mut whole = Check::new("the logic's radius decals");
    let mut stats = [0u64; 3];
    let mut steps = 0u64;
    for i in 0..cases {
        let input = gen_decals(&mut g);
        steps += ((input.len() - 5) / 16) as u64;
        let r = rust_decals(&input, &mut stats);
        let c = cpp_decals(&input);
        if r != c && !planted {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len())) / 12 * 12;
            println!("sequence {i}: rust {:?} c++ {:?}", &r[k.min(r.len())..(k + 12).min(r.len())], &c[k.min(c.len())..(k + 12).min(c.len())]);
        }
        whole.eq(r, c, || format!("sequence {i}"));
    }
    println!("logic decals: {cases} sequences, {steps} steps; slot-steps with a projected decal {}, of them with a positive opacity {}; not empty but not drawn {}", stats[0], stats[1], stats[2]);
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
