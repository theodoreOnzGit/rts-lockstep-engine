#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_common_client::in_game_ui` against `InGameUI` (#215, group lc3).
//!
//! # Method
//!
//! Part of InGameUI.cpp is cut verbatim by build.rs into `differential/wrap_draft_in_game_ui.cpp` (the cuts and the stand-ins are listed there): the script-hidden
//! superweapon displays and `SuperweaponInfo::drawName` / `drawTime`, `addMessageText`, `removeMessageAtIndex`, `beginAreaSelectHint`, `createMoveHint`, `expireHint`,
//! `destroyPlacementIcons`, `isDrawableSelected`, `isAllSelectedKindOf`, `canSelectedObjectsNonAttackInteractWithObject`, `updateFloatingText`, `drawFloatingText`,
//! `clearFloatingText`, and the idle-worker functions (`findIdleWorker`, `addIdleWorker`, `removeIdleWorker`, `getIdleWorkerCount`, `show` / `hide` / `update` /
//! `resetIdleWorker`). A case is a header (local player, shroud mask, vanish rate and up speed, message colours, icon mask, action mask) and a program of up to 40
//! operations (the records are listed in the code below). After every operation: the host calls in order (display strings, windows, drawables), the answer, the six
//! messages, the drag region, every non-empty move hint, the floating texts, the 16 idle-worker lists, the superweapon flags, the placement icons. 20,000 cases from a fixed seed.
//! `updateFloatingText` keeps a function-static `lastLogicFrameUpdate` upstream: it lives across cases in the C++ process, so the Rust side carries the same value from case to case.
//! NOT covered: `addFloatingText`, the world animations (they need `Anim2D`, tested in differential_draft_anim2d), the movie functions (device-only), `createReplayControl`,
//! `getIdleWorker*` beyond those listed, and `selectNextIdleWorker`. A selected-drawable list never holds NULL in the Rust model (upstream's loops skip a NULL entry): not generated.
//!
//! # Prediction (written before the first run, 2026-11-20)
//!
//! Two misses expected: `drawFloatingText` takes `pos.y -= frameCount * speed` as an integer subtraction of the truncated product where upstream subtracts as a
//! float and truncates once (differs whenever the product has a fraction); and `updateFloatingText` with a NEGATIVE vanish rate makes `amount` negative, where
//! upstream's `a -= amount` wraps the `UnsignedByte` and the draft's alpha leaks into the red channel. Everything else: 0.
//!
//! # Results
//!
//! 2026-11-20: **two prediction hits, then 0 mismatches over 20,000 cases.** First run: 2,388 cases differed (the test's own slips fixed first: the script-hidden flag's sense, and repeated selection ids).
//! Both misses were the predicted ones, both draft defects, both fixed in `in_game_ui.rs`: (1) `drawFloatingText` subtracted the truncated product from `pos.y` (the draft now subtracts as a float and truncates
//! once, as `pos.y -= m_frameCount * m_floatingTextMoveUpSpeed` does); (2) `updateFloatingText`'s alpha: upstream's `UnsignedByte a -= amount` wraps for a negative `amount`, the draft let the alpha carry into the red
//! channel (now `as u8`). Planted bugs, 22 of 22 caught (of 20,000 cases): 19911 2,104; 19912 15; 19913 3; 19914 1,068; 19915 5,630; 19916 886; 19917 1,206; 19918 1,610; 19919 251; 19920 1,847; 19921 2,859;
//! 19922 105; 19923 1,690; 19925 375; 19926 839; 19927 7,142; 19928 120; 19929 35; 19930 5,539; 19931 5,087; 19932 133; 19933 250. (19917 first panicked the draft's own index, and 19919 and 19932 were first
//! equivalent mutants with these inputs; the plant and the generators (the move hint wrap, the action mask) were changed, not dropped.)
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 19911 drawFloatingText truncates the product before subtracting; 19912 the alpha is not wrapped to a byte; 19913 addMessageText does not free the last message's string; 19914 the colour
//! rule compares with colour 1; 19915 an RGB colour loses its alpha; 19916 removeMessageAtIndex keeps the timestamp; 19917 the move hint index wraps one early; 19918 expireHint keeps the frame; 19919 the
//! frame count only advances while fading; 19920 the drop colour's alpha is 255; 19921 the text is centred on its whole width; 19922 addIdleWorker adds duplicates; 19923 updateIdleWorker hides at count < 0;
//! 19925 resetIdleWorker spares the last player; 19926 showObjectSuperweaponDisplayByScript hides instead; 19927 the script flag read is inverted; 19928 drawName adds the width; 19929 drawTime ignores
//! the info's colour; 19930 destroyPlacementIcons keeps the pointers; 19931 an empty selection is not "all of the kind"; 19932 the action loop starts at 0; 19933 findIdleWorker looks in player 0's list.

mod support;

use rts_lockstep_engine::drafts::sweep_common_client::anim2d::{Anim2DHost, AnimImage};
use rts_lockstep_engine::drafts::sweep_common_client::in_game_ui::{
    can_selected_objects_non_attack_interact_with_object, is_all_selected_kind_of, is_drawable_selected, FloatingTextData, HintType, InGameUiExtras, InGameUiHost, SuperweaponInfo,
};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_lc3_igu_run(prog: *const i32, nprog: i32, out: *mut i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish(c: Check) {
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}

fn sum_of(v: &[u16]) -> i32 {
    let mut s = 0i32;
    for &u in v {
        s = s.wrapping_mul(31).wrapping_add(u as i32);
    }
    s
}

struct Host {
    ev: Vec<i32>,
    next_ds: u32,
    logic_frame: u32,
    client_frame: u32,
    local: usize,
    shroud: u64,
    idle_win_found: bool,
    max_line: usize,
}

impl Anim2DHost for Host {
    fn logic_frame(&self) -> u32 {
        self.logic_frame
    }
    fn game_client_random_value(&mut self, _lo: i32, _hi: i32) -> i32 {
        0
    }
    fn find_image_by_name(&self, _name: &[u8]) -> Option<AnimImage> {
        None
    }
    fn draw_image(&mut self, _image: &AnimImage, _x0: i32, _y0: i32, _x1: i32, _y1: i32, _color: u32) {}
}

impl InGameUiHost for Host {
    fn new_display_string(&mut self) -> u32 {
        let id = self.next_ds;
        self.next_ds += 1;
        self.ev.extend([1, id as i32]);
        id
    }
    fn free_display_string(&mut self, handle: u32) {
        self.ev.extend([4, handle as i32]);
    }
    fn set_message_font(&mut self, handle: u32) {
        self.ev.extend([2, handle as i32]);
    }
    fn display_string_set_text(&mut self, handle: u32, text: &[u16]) {
        self.ev.extend([3, handle as i32, text.len() as i32, sum_of(text)]);
    }
    fn display_string_width(&self, handle: u32) -> i32 {
        (handle as i32 * 13) % 40 + 5
    }
    fn display_string_size(&self, handle: u32) -> (i32, i32) {
        (self.display_string_width(handle), 9)
    }
    fn display_string_font_height(&self, _handle: u32) -> f32 {
        0.0
    }
    fn display_string_draw(&mut self, handle: u32, x: i32, y: i32, color: u32, drop_color: u32) {
        self.ev.extend([5, handle as i32, x, y, color as i32, drop_color as i32]);
    }
    fn game_paused(&self) -> bool {
        false
    }
    fn client_frame(&self) -> u32 {
        self.client_frame
    }
    fn world_to_screen(&self, p: (f32, f32, f32)) -> Option<(i32, i32)> {
        if p.0 < 0.0 {
            None
        } else {
            Some(((p.0 * 1.5f32) as i32, (p.1 * 2.5f32 + 3.0f32) as i32))
        }
    }
    fn view_max_zoom_and_zoom(&self) -> (f32, f32) {
        (1.0, 1.0)
    }
    fn pos_clear_for_local_player(&self, p: (f32, f32, f32)) -> bool {
        self.cell_of_pos_clear_for_local_player(p.0, p.1)
    }
    fn cell_of_pos_clear_for_local_player(&self, x: f32, y: f32) -> bool {
        let cx = (x / 10.0f32) as i32;
        let cy = (y / 10.0f32) as i32;
        let b = (cx.rem_euclid(8)) * 8 + cy.rem_euclid(8);
        (self.shroud >> b) & 1 != 0
    }
    fn local_player_index(&self) -> usize {
        self.local
    }
    fn video_open(&mut self, _movie_name: &[u8]) -> Option<u32> {
        None
    }
    fn window_set_video_buffer(&mut self, _window_name: &str, _buffer: Option<u32>) {}
    fn win_create_from_script(&mut self, _name: &str) -> Option<u32> {
        None
    }
    fn find_idle_worker_window(&mut self) -> Option<u32> {
        self.ev.push(8);
        if self.idle_win_found {
            Some(77)
        } else {
            None
        }
    }
    fn idle_worker_window_enable(&mut self, _win: u32, enable: bool) {
        self.ev.extend([6, enable as i32]);
    }
    fn idle_worker_window_clear_text(&mut self, _win: u32) {
        self.ev.push(7);
    }
    fn destroy_placement_icon(&mut self, drawable: u32) {
        self.ev.extend([11, drawable as i32, 9, drawable as i32]);
    }
    fn remove_all_bibs(&mut self) {
        self.ev.push(10);
    }
    fn max_line_build_objects(&self) -> usize {
        self.max_line
    }
}

fn fb(f: f32) -> i32 {
    f.to_bits() as i32
}

static LAST_FLOAT_FRAME: Mutex<Option<u32>> = Mutex::new(None);

#[test]
fn in_game_ui_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_16B);
    let mut c = Check::new("InGameUI messages, hints, floating text, idle workers");
    for case in 0..20_000 {
        let local = g.below(16) as i32;
        let input_enabled = g.below(5) != 0;
        let idle_win_found = g.below(4) != 0;
        let shroud = g.u64();
        let vanish = if g.below(6) == 0 { g.uniform(-1.0, 0.0) } else { g.uniform(0.0, 3.0) };
        let up = g.uniform(0.0, 3.0);
        let color1 = g.u32() as i32;
        let color2 = if g.below(3) == 0 { color1 } else { g.u32() as i32 };
        let max_line = g.below(9) as i32;
        let icon_mask = g.below(256) as i32;
        let can_mask = match g.below(4) {
            0 => 0,
            1 => g.u32() as i32,
            _ => (1i32 << g.below(18)) | if g.below(2) == 0 { 1i32 << g.below(18) } else { 0 },
        };
        let nops = 1 + g.below(40);
        let mut prog: Vec<i32> = vec![local, input_enabled as i32, idle_win_found as i32, shroud as u32 as i32, (shroud >> 32) as u32 as i32, fb(vanish), fb(up), color1, color2, max_line, icon_mask, can_mask, nops as i32];
        let mut frame = g.below(100);
        let mut ops: Vec<[i32; 16]> = Vec::new();
        for _ in 0..nops {
            frame += [0, 0, 1, 1, 1, 2, 5, 40][g.below(8) as usize];
            let kind = [0, 0, 1, 2, 3, 3, 4, 5, 5, 5, 6, 6, 7, 7, 8, 9, 9, 9, 10, 10, 11, 12, 13, 14, 15, 15, 16, 17, 17, 18, 19, 20, 21, 22, 22, 23, 24, 25, 26, 27, 28, 29, 29][g.below(43) as usize];
            let mut r = [0i32; 16];
            r[0] = kind;
            r[1] = frame as i32;
            r[2] = g.below(1000) as i32;
            let a = &mut r[3..];
            match kind {
                0 => {
                    a[0] = g.below(7) as i32;
                    for i in 0..6 {
                        a[1 + i] = 0x100 + g.below(200) as i32;
                    }
                    a[7] = g.below(2) as i32;
                    a[8] = (g.u32() & 0xffffff) as i32;
                }
                1 => a[0] = g.below(6) as i32,
                2 => {
                    for i in 0..4 {
                        a[i] = g.below(2000) as i32 - 500;
                    }
                }
                3 => {
                    a[0] = g.below(6) as i32;
                    a[1] = fb(g.uniform(-50.0, 200.0));
                    a[2] = fb(g.uniform(-50.0, 200.0));
                    a[3] = fb(g.uniform(-10.0, 50.0));
                }
                4 => a[0] = [0, 1, 2, 3, 255, 256, 257, 300][g.below(8) as usize],
                5 => {
                    a[0] = g.u32() as i32 | 0x7fff_ffffu32 as i32 & g.u32() as i32;
                    a[0] = (g.u32() | if g.below(2) == 0 { 0xff00_0000 } else { 0 }) as i32;
                    a[1] = fb(g.uniform(-5.0, 200.0));
                    a[2] = fb(g.uniform(-5.0, 200.0));
                    a[3] = fb(g.uniform(0.0, 20.0));
                    a[4] = g.below(60) as i32;
                    a[5] = g.below(200) as i32;
                    a[6] = if g.below(5) == 0 { 0 } else { 1 + g.below(30) as i32 };
                }
                9 => {
                    a[0] = g.below(12) as i32;
                    a[1] = g.below(16) as i32;
                }
                10 | 11 => {
                    a[0] = g.below(12) as i32;
                    a[1] = if kind == 10 { g.below(20) as i32 - 2 } else { g.below(16) as i32 };
                }
                17 => {
                    a[0] = g.below(16) as i32;
                    a[1] = g.below(3) as i32;
                    a[2] = g.below(6) as i32;
                    a[3] = g.below(2) as i32;
                    a[4] = g.u32() as i32;
                    a[5] = 1 + g.below(30) as i32;
                    a[6] = 31 + g.below(30) as i32;
                }
                18 => a[0] = g.below(6) as i32,
                29 => a[0] = 250 + g.below(6) as i32,
                19 | 28 => a[0] = g.below(2) as i32,
                22 => {
                    a[0] = g.below(5) as i32;
                    let base = 1 + g.below(3) as i32;
                    for i in 0..4 {
                        a[1 + 3 * i] = base + i as i32; // distinct ids: the closure is asked by id
                        a[2 + 3 * i] = g.below(64) as i32;
                        a[3 + 3 * i] = g.below(4) as i32;
                    }
                }
                23 => a[0] = g.below(7) as i32,
                24 => a[0] = [3, 5][g.below(2) as usize],
                26 | 27 => {
                    a[0] = g.below(16) as i32;
                    a[1] = g.below(3) as i32;
                    a[2] = g.below(300) as i32 - 50;
                    a[3] = g.below(300) as i32 - 50;
                    a[4] = if g.below(3) == 0 { 0 } else { g.u32() as i32 };
                    a[5] = g.u32() as i32;
                }
                _ => {}
            }
            prog.extend(r);
            ops.push(r);
        }
        let mut cpp = vec![0i32; 3_000_000];
        let len = unsafe { ref_lc3_igu_run(prog.as_ptr(), prog.len() as i32, cpp.as_mut_ptr()) } as usize;
        cpp.truncate(len);

        // the Rust side
        let mut ui = InGameUiExtras::new();
        ui.input_enabled = input_enabled;
        ui.floating_text_move_vanish_rate = vanish;
        ui.floating_text_move_up_speed = up;
        ui.message_color_1 = color1 as u32;
        ui.message_color_2 = color2 as u32;
        ui.place_icon = (0..8).map(|i| if (icon_mask >> i) & 1 != 0 { Some(100 + i as u32) } else { None }).collect();
        ui.last_logic_frame_update = *LAST_FLOAT_FRAME.lock().unwrap();
        let mut host = Host { ev: Vec::new(), next_ds: 1, logic_frame: 0, client_frame: 0, local: local as usize, shroud, idle_win_found, max_line: max_line as usize };
        // objects: id -> player is set at each add / find
        let mut selection: Vec<(u32, u32, i32)> = Vec::new(); // drawable id, kinds, flags
        let mut rust: Vec<i32> = Vec::new();
        let mut chunks: Vec<(usize, usize)> = Vec::new();
        for r in &ops {
            let kind = r[0];
            let chunk_start = rust.len();
            rust.push(0);
            host.logic_frame = r[1] as u32;
            host.client_frame = r[2] as u32;
            let a = &r[3..];
            host.ev.clear();
            let mut ans = 0i32;
            match kind {
                0 => {
                    let text: Vec<u16> = (0..a[0] as usize).map(|i| a[1 + i] as u16).collect();
                    ui.add_message_text(&mut host, &text, if a[7] != 0 { Some(a[8] as u32) } else { None });
                }
                1 => ui.remove_message_at_index(&mut host, a[0] as usize),
                2 => ui.begin_area_select_hint((a[0], a[1], a[2], a[3])),
                3 => {
                    let lone = selection.len() == 1 && selection[0].2 & 1 != 0 && selection[0].2 & 2 != 0;
                    let loc = (f32::from_bits(a[1] as u32), f32::from_bits(a[2] as u32), f32::from_bits(a[3] as u32));
                    ui.create_move_hint(&host, a[0] as u32, loc, lone);
                }
                4 => ui.expire_hint(HintType::MoveHint, a[0] as u32 as usize),
                5 => {
                    ui.floating_text_list.insert(
                        0,
                        FloatingTextData {
                            color: a[0] as u32,
                            pos_3d: (f32::from_bits(a[1] as u32), f32::from_bits(a[2] as u32), f32::from_bits(a[3] as u32)),
                            frame_time_out: a[4] as u32,
                            frame_count: a[5] as u32,
                            d_string: if a[6] != 0 { Some(a[6] as u32) } else { None },
                        },
                    );
                }
                6 => ui.update_floating_text(&mut host),
                7 => ui.draw_floating_text(&mut host),
                8 => ui.clear_floating_text(&mut host),
                9 => ui.add_idle_worker(if a[0] != 0 { Some((a[0] as u32, a[1] as usize)) } else { None }),
                10 => ui.remove_idle_worker(if a[0] != 0 { Some(a[0] as u32) } else { None }, a[1]),
                11 => ans = ui.find_idle_worker(if a[0] != 0 { Some((a[0] as u32, a[1] as usize)) } else { None }).map_or(-1, |v| v as i32),
                12 => ans = ui.get_idle_worker_count(&host),
                13 => ui.show_idle_worker_layout(&mut host),
                14 => ui.hide_idle_worker_layout(&mut host),
                15 => ui.update_idle_worker(&mut host),
                16 => ui.reset_idle_worker(&mut host),
                17 => {
                    let info = SuperweaponInfo { id: a[2] as u32, hidden_by_script: a[3] != 0, color: a[4] as u32, name_display_string: a[5] as u32, time_display_string: a[6] as u32 };
                    ui.superweapons[a[0] as usize].entry(vec![b'n', b'0' + a[1] as u8]).or_default().push(info);
                }
                18 => ui.show_object_superweapon_display_by_script(a[0] as u32),
                19 => ui.superweapon_hidden_by_script = a[0] != 0,
                20 => ans = ui.get_superweapon_display_enabled_by_script() as i32,
                21 => ui.destroy_placement_icons(&mut host),
                22 => {
                    selection.clear();
                    for i in 0..a[0] as usize {
                        selection.push((a[1 + 3 * i] as u32, a[2 + 3 * i] as u32, a[3 + 3 * i]));
                    }
                }
                23 => {
                    let ids: Vec<u32> = selection.iter().map(|s| s.0).collect();
                    ans = is_drawable_selected(&ids, a[0] as u32) as i32;
                }
                24 => {
                    let ids: Vec<u32> = selection.iter().map(|s| s.0).collect();
                    let k = a[0] as u32;
                    let kinds: Vec<(u32, u32)> = selection.iter().map(|s| (s.0, s.1)).collect();
                    ans = is_all_selected_kind_of(&ids, &|id| kinds.iter().find(|x| x.0 == id).map_or(false, |x| (x.1 >> k) & 1 != 0)) as i32;
                }
                25 => ans = can_selected_objects_non_attack_interact_with_object(18, 1, &mut |i| (can_mask >> i) & 1 != 0) as i32,
                26 | 27 => {
                    if let Some(info) = ui.superweapons[a[0] as usize].get(&vec![b'n', b'0' + a[1] as u8]).and_then(|l| l.first()) {
                        let info = info.clone();
                        if kind == 26 {
                            info.draw_name(&mut host, a[2], a[3], a[4] as u32, a[5] as u32);
                        } else {
                            info.draw_time(&mut host, a[2], a[3], a[4] as u32, a[5] as u32);
                        }
                    }
                }
                29 => ui.next_move_hint = a[0] as usize,
                _ => ui.input_enabled = a[0] != 0,
            }
            rust.push(host.ev.len() as i32);
            rust.extend(host.ev.iter().copied());
            rust.push(ans);
            for m in &ui.ui_messages {
                rust.extend([m.full_text.len() as i32, sum_of(&m.full_text), m.display_string.map_or(0, |v| v as i32), m.timestamp as i32, m.color as i32]);
            }
            rust.push(ui.is_drag_selecting as i32);
            rust.extend([ui.drag_select_region.0, ui.drag_select_region.1, ui.drag_select_region.2, ui.drag_select_region.3]);
            let live: Vec<usize> = (0..ui.move_hint.len()).filter(|&i| ui.move_hint[i].frame != 0 || ui.move_hint[i].source_id != 0).collect();
            rust.push(live.len() as i32);
            for &i in &live {
                let m = &ui.move_hint[i];
                rust.extend([i as i32, fb(m.pos.0), fb(m.pos.1), fb(m.pos.2), m.frame as i32, m.source_id as i32]);
            }
            rust.push(ui.next_move_hint as i32);
            rust.push(ui.floating_text_list.len() as i32);
            for f in &ui.floating_text_list {
                rust.extend([f.color as i32, fb(f.pos_3d.0), fb(f.pos_3d.1), fb(f.pos_3d.2), f.frame_time_out as i32, f.frame_count as i32, f.d_string.map_or(0, |v| v as i32)]);
            }
            for pl in 0..16 {
                rust.push(ui.idle_workers[pl].len() as i32);
                rust.extend(ui.idle_workers[pl].iter().map(|&v| v as i32));
            }
            rust.extend([ui.idle_worker_win.is_some() as i32, ui.current_idle_worker_display, ui.input_enabled as i32, ui.superweapon_hidden_by_script as i32]);
            for pl in 0..16 {
                for nm in 0..3u8 {
                    match ui.superweapons[pl].get(&vec![b'n', b'0' + nm]) {
                        None => rust.push(-1),
                        Some(l) => {
                            rust.push(l.len() as i32);
                            for s in l {
                                rust.extend([s.id as i32, s.hidden_by_script as i32]);
                            }
                        }
                    }
                }
            }
            for i in 0..8 {
                rust.push(ui.place_icon.get(i).copied().flatten().map_or(0, |v| v as i32));
            }
            rust[chunk_start] = (rust.len() - chunk_start - 1) as i32;
            chunks.push((chunk_start, rust.len()));
        }
        *LAST_FLOAT_FRAME.lock().unwrap() = ui.last_logic_frame_update;
        let detail = || {
            // the first operation whose words differ
            let mut cp = 0usize;
            for (i, &(s0, s1)) in chunks.iter().enumerate() {
                let clen = cpp.get(cp).copied().unwrap_or(0) as usize + 1;
                let cchunk = cpp.get(cp..(cp + clen).min(cpp.len())).unwrap_or(&[]).to_vec();
                if rust[s0..s1] != cchunk[..] {
                    let at = rust[s0..s1].iter().zip(cchunk.iter()).position(|(a, b)| a != b).unwrap_or(0);
                    return format!("first differing op #{i} {:?} at word {at}: rust {:?} c++ {:?}", ops[i], &rust[s0..s1][at.saturating_sub(3)..(at + 4).min(s1 - s0)], &cchunk[at.saturating_sub(3)..(at + 4).min(cchunk.len())]);
                }
                cp += clen;
            }
            "no differing op".to_string()
        };
        c.eq(rust.clone(), cpp.clone(), || format!("case {case}: header local {local} vanish {vanish} up {up} up_speed, {}", detail()));
    }
    finish(c);
}
