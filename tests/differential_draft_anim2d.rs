#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/System/Anim2D.cpp and Include/GameClient/Anim2D.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_common_client::anim2d` against `Anim2DTemplate`, `Anim2D` and `Anim2DCollection` (#215).
//!
//! # Method
//!
//! The class bodies (Anim2D.h) and every member function of Anim2D.cpp except the INI field table, `Anim2D::xfer` and
//! `Anim2DCollection::init` are cut verbatim (build.rs) into `differential/wrap_draft_anim2d.cpp`; the wrapper stands in for the
//! display (logs `drawImage`), the logic frame, `GameClientRandomValue` (a queue the test gives; logs `lo, hi`), the mapped image
//! collection, `Image` and the `INI` parse calls. 20,000 programs of 20 to 140 ops from a fixed seed run on both sides: templates
//! (names from four, so that `findTemplate` meets duplicates; frame counts 0 to 40 and, now and then, 65,536 + k or 70,000;
//! all six modes and the invalid one; delays 0 to 4 and 65,535), images (40, some not in the collection, sizes from the whole
//! `i32` range), `parseNumImages` / `parseImage` / `parseImageSequence` / `allocateImages` / `storeImage` / `getFrame`, instances
//! (registered or not, randomised start frames), `setMinFrame` / `setMaxFrame` (including a minimum above the current frame and
//! a maximum of 0 or 65,535), status bits, alpha in [0, 1], the logic frame (steps, and jumps to near `u32::MAX` so that
//! `getFrame() - m_lastUpdateFrame` wraps), `update`, both `draw`s, `tryNextFrame`, `setCurrentFrame`, `randomizeCurrentFrame`,
//! `reset`, delete, `registerAnimation` / `unRegisterAnimation`, and dumps of every member. Every answer is an `i32` word and the
//! whole word list is compared.
//!
//! Excluded (undefined upstream): `draw` of a frame with no image (a NULL dereference; the program skips it on both sides, by
//! `getFrame`), alpha outside [0, 1] (`255 * alpha` out of the range of the `UnsignedByte` it converts to), registering an instance
//! twice or deleting or unregistering one that was already unlinked by hand (the list is corrupted upstream). Constructors of the
//! collection/templates' destructors are not tested (the draft has none).
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! Expect mismatches from the `cur + 1` / `cur - 1` on `u16` (panic by overflow check or no wrap) in ping-pong and loop modes
//! when a program sets a minimum above or a maximum below the frame; after that fix, and the wrapping coordinate adds,
//! 0 mismatches.
//!
//! # Results
//!
//! Run 2026-11-19, seed fixed: `Anim2DModeNames` 8 compared, 0 mismatches; `Anim2D programs` 20,000 compared, 0 mismatches (max ULP 0,
//! no NaN words). Prediction hit: the wrapping `u16` steps and coordinate adds had been fixed in the draft by reading upstream
//! before the first run, so the first run was already 0 (the mismatches the prediction expected were pre-empted, not observed).
//! Plants 14261..14272: all 12 caught (218 to 14,446 of 20,000 programs each); baseline 0. Not covered: the INI field table,
//! `xfer` (not in the draft) and `Anim2DCollection::init` (empty in both). Duplicate: the draft's `game_make_color` repeats
//! `crate::common::ini::game_make_color`.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 14261 `ONCE` tests `<=` max; 14262 `LOOP_BACKWARDS` tests `>=` min; 14263 ping-pong reversed tests `<=` min;
//! 14264 the delay test is `<=`; 14265 `LOOP` wraps to 0 not `m_minFrame`; 14266 `reset` of a backwards mode goes to the minimum;
//! 14267 forced-size `draw` uses the width for `y1`; 14268 `randomizeCurrentFrame` asks for `0..numFrames`; 14269 `findTemplate`
//! finds the oldest; 14270 `parseNumImages` accepts 0; 14271 `draw` ticks a registered instance; 14272 alpha rounded, not truncated.

mod support;

use rts_lockstep_engine::drafts::sweep_common_client::anim2d::*;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_anim2d_mode_name(i: i32, out: *mut std::os::raw::c_char, cap: i32) -> i32;
    fn ref_anim2d_run(input: *const i32, nin: i32, out: *mut i32, cap: i32) -> i32;
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

fn image_name(i: usize) -> Vec<u8> {
    if i < 8 {
        format!("I{i}").into_bytes()
    } else {
        format!("Sq{:03}", i - 8).into_bytes()
    }
}

struct AnimGen {
    templates: usize,
    // per instance: (alive, in list, may be deleted/unlinked/registered by hand)
    anims: Vec<AState>,
    nimg: usize,
}

#[derive(Clone, Copy)]
struct AState {
    alive: bool,
    in_list: bool,
    sys: bool,
    touched: bool, // unlinked by hand or put in the list by hand: no more list operations
}

fn size_val(g: &mut Gen) -> i32 {
    match g.below(6) {
        0 => 0,
        1 => g.i32(),
        2 => -(g.below(100) as i32),
        3 => i32::MAX - g.below(3) as i32,
        _ => g.below(2000) as i32,
    }
}

fn count_val(g: &mut Gen) -> i32 {
    match g.below(20) {
        0 => 0,
        1 => 65536 + g.below(5) as i32,
        2 => 70000,
        3 => 40 + g.below(5) as i32,
        _ => 1 + g.below(8) as i32,
    }
}

fn frame_val(g: &mut Gen) -> i32 {
    match g.below(8) {
        0 => 65535,
        1 => 65536 + g.below(3) as i32,
        2 => -1,
        _ => g.below(10) as i32,
    }
}

fn program(g: &mut Gen) -> Vec<i32> {
    let mut v = Vec::new();
    let nimg = 40;
    v.push(nimg as i32);
    for _ in 0..nimg {
        v.push((g.below(10) != 0) as i32);
        v.push(size_val(g));
        v.push(size_val(g));
    }
    let nr = g.below(30);
    v.push(nr as i32);
    for _ in 0..nr {
        v.push(match g.below(8) {
            0 => 65535,
            1 => 65536,
            2 => -1 - g.below(3) as i32,
            _ => g.below(12) as i32,
        });
    }
    let mut st = AnimGen { templates: 0, anims: Vec::new(), nimg: nimg as usize };
    let nops = 20 + g.below(121);
    for _ in 0..nops {
        gen_op(g, &mut st, &mut v);
    }
    v
}

fn pick_anim(g: &mut Gen, st: &AnimGen, need: fn(&AState) -> bool) -> Option<i32> {
    let ok: Vec<usize> = (0..st.anims.len()).filter(|&i| st.anims[i].alive && need(&st.anims[i])).collect();
    if ok.is_empty() {
        None
    } else {
        Some(ok[g.below(ok.len() as u32) as usize] as i32)
    }
}

fn gen_op(g: &mut Gen, st: &mut AnimGen, v: &mut Vec<i32>) {
    // templates first
    if st.templates == 0 || (st.templates < 6 && g.below(25) == 0) {
        v.extend_from_slice(&[0, g.below(4) as i32]);
        st.templates += 1;
        if g.below(3) != 0 {
            let t = (st.templates - 1) as i32;
            v.extend_from_slice(&[1, t, g.below(7) as i32, [0, 0, 1, 2, 3, 65535][g.below(6) as usize], (g.below(4) == 0) as i32]);
            v.extend_from_slice(&[2, t, count_val(g)]);
            if g.below(2) == 0 {
                v.extend_from_slice(&[4, t, [0, 0, 0, 1, 2][g.below(5) as usize]]);
            }
        }
        return;
    }
    let t = g.below(st.templates as u32) as i32;
    let any = |g: &mut Gen, st: &AnimGen| pick_anim(g, st, |_| true);
    match g.below(100) {
        0..=3 => v.extend_from_slice(&[1, t, g.below(7) as i32, [0, 0, 1, 2, 3, 65535][g.below(6) as usize], (g.below(4) == 0) as i32]),
        4..=5 => v.extend_from_slice(&[2, t, count_val(g)]),
        6..=8 => v.extend_from_slice(&[3, t, g.below(st.nimg as u32 + 1) as i32 - 1]),
        9..=10 => v.extend_from_slice(&[4, t, g.below(3) as i32]),
        11 => v.extend_from_slice(&[5, t, count_val(g).min(100)]),
        12..=13 => v.extend_from_slice(&[6, t, g.below(st.nimg as u32 + 1) as i32 - 1]),
        14..=15 => v.extend_from_slice(&[7, t, frame_val(g)]),
        16..=21 => {
            let reg = (g.below(2) == 0) as i32;
            v.extend_from_slice(&[8, t, reg]);
            st.anims.push(AState { alive: true, in_list: reg != 0, sys: reg != 0, touched: false });
        }
        22..=24 => {
            if let Some(a) = any(g, st) {
                v.extend_from_slice(&[9, a, frame_val(g).max(0) % 70000, frame_val(g).max(0) % 70000]);
            }
        }
        25..=27 => {
            if let Some(a) = any(g, st) {
                v.extend_from_slice(&[if g.below(2) == 0 { 10 } else { 11 }, a, g.below(8) as i32 | if g.below(8) == 0 { 0x1F0 } else { 0 }]);
            }
        }
        28..=29 => {
            if let Some(a) = any(g, st) {
                let f = match g.below(5) {
                    0 => 0.0f32,
                    1 => 1.0,
                    _ => g.uniform(0.0, 1.0),
                };
                v.extend_from_slice(&[12, a, f.to_bits() as i32]);
            }
        }
        30..=36 => {
            let f = match g.below(12) {
                0 => (u32::MAX - g.below(4)) as i32,
                1 => 0,
                _ => (g.below(40)) as i32 + 1,
            };
            v.extend_from_slice(&[13, f]);
        }
        37..=42 => v.push(14),
        43..=50 => {
            if let Some(a) = any(g, st) {
                v.extend_from_slice(&[15, a, size_val(g), size_val(g)]);
            }
        }
        51..=54 => {
            if let Some(a) = any(g, st) {
                v.extend_from_slice(&[16, a, size_val(g), size_val(g), size_val(g), size_val(g)]);
            }
        }
        55..=58 => {
            if let Some(a) = any(g, st) {
                v.extend_from_slice(&[17, a]);
            }
        }
        59..=66 => {
            if let Some(a) = any(g, st) {
                v.extend_from_slice(&[18, a]);
            }
        }
        67..=69 => {
            if let Some(a) = any(g, st) {
                v.extend_from_slice(&[19, a, frame_val(g)]);
            }
        }
        70..=71 => {
            if let Some(a) = any(g, st) {
                v.extend_from_slice(&[20, a]);
            }
        }
        72..=74 => {
            if let Some(a) = any(g, st) {
                v.extend_from_slice(&[21, a]);
            }
        }
        75..=77 => {
            if let Some(a) = pick_anim(g, st, |s| !s.touched && (s.sys || !s.in_list)) {
                v.extend_from_slice(&[22, a]);
                st.anims[a as usize].alive = false;
            }
        }
        78..=80 => v.extend_from_slice(&[23, g.below(4) as i32]),
        81..=82 => v.extend_from_slice(&[24, t]),
        83..=85 => {
            if let Some(a) = pick_anim(g, st, |s| !s.touched && !s.in_list && !s.sys) {
                v.extend_from_slice(&[25, a]);
                st.anims[a as usize].in_list = true;
                st.anims[a as usize].touched = true;
            }
        }
        86..=88 => {
            if let Some(a) = pick_anim(g, st, |s| !s.touched && s.in_list && s.sys) {
                v.extend_from_slice(&[26, a]);
                st.anims[a as usize].in_list = false;
                st.anims[a as usize].touched = true;
            }
        }
        89..=94 => {
            if let Some(a) = any(g, st) {
                v.extend_from_slice(&[27, a]);
            }
        }
        _ => v.extend_from_slice(&[28, t]),
    }
}

// ---- the Rust side -------------------------------------------------------------------------------------------------

struct Host {
    frame: u32,
    randoms: Vec<i32>,
    at: usize,
    out: Vec<i32>,
    images: Vec<AnimImage>,
    known: Vec<bool>,
}

impl Host {
    fn index_of(&self, im: &AnimImage) -> i32 {
        self.images.iter().position(|x| x.name == im.name).map_or(-2, |i| i as i32)
    }
}

impl Anim2DHost for Host {
    fn logic_frame(&self) -> u32 {
        self.frame
    }
    fn game_client_random_value(&mut self, lo: i32, hi: i32) -> i32 {
        self.out.extend_from_slice(&[2000, lo, hi]);
        if self.at < self.randoms.len() {
            self.at += 1;
            self.randoms[self.at - 1]
        } else {
            lo
        }
    }
    fn find_image_by_name(&self, name: &[u8]) -> Option<AnimImage> {
        (0..self.images.len()).find(|&i| self.known[i] && self.images[i].name == name).map(|i| self.images[i].clone())
    }
    fn draw_image(&mut self, image: &AnimImage, x0: i32, y0: i32, x1: i32, y1: i32, color: u32) {
        let i = self.index_of(image);
        self.out.extend_from_slice(&[1000, i, x0, y0, x1, y1, color as i32]);
    }
}

fn run_rust(p: &[i32]) -> Vec<i32> {
    let mut at = 0;
    let nimg = p[at] as usize;
    at += 1;
    let mut images = Vec::new();
    let mut known = Vec::new();
    for i in 0..nimg {
        known.push(p[at] != 0);
        images.push(AnimImage { name: image_name(i), width: p[at + 1], height: p[at + 2] });
        at += 3;
    }
    let nr = p[at] as usize;
    at += 1;
    let randoms = p[at..at + nr].to_vec();
    at += nr;
    let mut h = Host { frame: 0, randoms, at: 0, out: Vec::new(), images, known };
    let mut c = Anim2DCollection::new();
    let tnames: [&[u8]; 4] = [b"A0", b"A1", b"A2", b"A3"];
    let bases: [&[u8]; 3] = [b"Sq", b"Zz", b""];
    let mut tids: Vec<usize> = Vec::new();
    let mut aids: Vec<Anim2DId> = Vec::new();
    let img_or_none = |h: &Host, i: i32| -> Option<AnimImage> { if i >= 0 && (i as usize) < h.images.len() { Some(h.images[i as usize].clone()) } else { None } };
    while at < p.len() {
        let kind = p[at];
        at += 1;
        match kind {
            0 => {
                let t = c.new_template(tnames[p[at] as usize]);
                at += 1;
                tids.push(t);
                h.out.push(tids.len() as i32);
            }
            1 => {
                let t = &mut c.templates[tids[p[at] as usize]];
                t.anim_mode = match p[at + 1] {
                    0 => Anim2DMode::Invalid,
                    1 => Anim2DMode::Once,
                    2 => Anim2DMode::OnceBackwards,
                    3 => Anim2DMode::Loop,
                    4 => Anim2DMode::LoopBackwards,
                    5 => Anim2DMode::PingPong,
                    _ => Anim2DMode::PingPongBackwards,
                };
                t.frames_between_updates = p[at + 2] as u16;
                t.randomize_start_frame = p[at + 3] != 0;
                at += 4;
            }
            2 => {
                let r = c.templates[tids[p[at] as usize]].parse_num_images(p[at + 1] as u32);
                at += 2;
                h.out.push(r.is_err() as i32);
            }
            3 => {
                let im = img_or_none(&h, p[at + 1]);
                let r = c.templates[tids[p[at] as usize]].parse_image(im);
                at += 2;
                h.out.push(r.is_err() as i32);
            }
            4 => {
                let r = c.templates[tids[p[at] as usize]].parse_image_sequence(&h, bases[p[at + 1] as usize]);
                at += 2;
                h.out.push(r.is_err() as i32);
            }
            5 => {
                c.templates[tids[p[at] as usize]].allocate_images(p[at + 1] as u16);
                at += 2;
            }
            6 => {
                let im = img_or_none(&h, p[at + 1]);
                let r = c.templates[tids[p[at] as usize]].store_image(im);
                at += 2;
                h.out.push(r.is_err() as i32);
            }
            7 => {
                let r = c.templates[tids[p[at] as usize]].get_frame(p[at + 1] as u16).map_or(-1, |im| h.index_of(im));
                at += 2;
                h.out.push(r);
            }
            8 => {
                let reg = p[at + 1] != 0;
                let id = c.new_anim(&mut h, tids[p[at] as usize], reg);
                at += 2;
                aids.push(id);
                h.out.push(aids.len() as i32 - 1);
            }
            9 => {
                let a = c.instances[aids[p[at] as usize]].as_mut().unwrap();
                a.min_frame = p[at + 1] as u16;
                a.max_frame = p[at + 2] as u16;
                at += 3;
            }
            10 => {
                c.set_status(aids[p[at] as usize], p[at + 1] as u8);
                at += 2;
            }
            11 => {
                c.clear_status(aids[p[at] as usize], p[at + 1] as u8);
                at += 2;
            }
            12 => {
                c.instances[aids[p[at] as usize]].as_mut().unwrap().alpha = f32::from_bits(p[at + 1] as u32);
                at += 2;
            }
            13 => {
                h.frame = p[at] as u32;
                at += 1;
            }
            14 => c.update(&h),
            15 | 16 => {
                let id = aids[p[at] as usize];
                let has = {
                    let a = c.instances[id].as_ref().unwrap();
                    c.templates[a.template].get_frame(a.current_frame).is_some()
                };
                if kind == 15 {
                    let (x, y) = (p[at + 1], p[at + 2]);
                    at += 3;
                    if has {
                        c.draw(id, x, y, &mut h);
                    } else {
                        h.out.push(-9);
                    }
                } else {
                    let (x, y, w, hh) = (p[at + 1], p[at + 2], p[at + 3], p[at + 4]);
                    at += 5;
                    if has {
                        c.draw_sized(id, x, y, w, hh, &mut h);
                    } else {
                        h.out.push(-9);
                    }
                }
            }
            17 => {
                let id = aids[p[at] as usize];
                at += 1;
                let a = c.instances[id].as_ref().unwrap();
                h.out.extend_from_slice(&[a.current_frame as i32, a.status as i32, c.get_current_frame_width(id) as i32, c.get_current_frame_height(id) as i32]);
            }
            18 => {
                c.try_next_frame(aids[p[at] as usize], &h);
                at += 1;
            }
            19 => {
                c.set_current_frame(aids[p[at] as usize], p[at + 1] as u16, &h);
                at += 2;
            }
            20 => {
                c.randomize_current_frame(aids[p[at] as usize], &mut h);
                at += 1;
            }
            21 => {
                c.reset(aids[p[at] as usize], &h);
                at += 1;
            }
            22 => {
                c.delete_anim(aids[p[at] as usize]);
                at += 1;
            }
            23 => {
                let r = c.find_template(tnames[p[at] as usize]).map_or(-1, |i| tids.iter().position(|&t| t == i).unwrap() as i32);
                at += 1;
                h.out.push(r);
            }
            24 => {
                let r = c.get_next_template(Some(tids[p[at] as usize])).map_or(-1, |i| tids.iter().position(|&t| t == i).unwrap() as i32);
                at += 1;
                h.out.push(r);
            }
            25 => {
                c.register_animation(aids[p[at] as usize]);
                at += 1;
            }
            26 => {
                c.un_register_animation(aids[p[at] as usize]);
                at += 1;
            }
            27 => {
                let a = c.instances[aids[p[at] as usize]].as_ref().unwrap();
                at += 1;
                h.out.extend_from_slice(&[a.current_frame as i32, a.last_update_frame as i32, a.status as i32, a.min_frame as i32, a.max_frame as i32, a.frames_between_updates as i32, a.alpha.to_bits() as i32]);
            }
            _ => {
                let t = &c.templates[tids[p[at] as usize]];
                at += 1;
                h.out.extend_from_slice(&[t.num_frames as i32, t.frames_between_updates as i32, t.anim_mode as i32, t.randomize_start_frame as i32]);
                for i in 0..t.num_frames as usize {
                    let r = t.images.get(i).and_then(|x| x.as_ref()).map_or(-1, |im| h.index_of(im));
                    h.out.push(r);
                }
            }
        }
    }
    h.out
}

fn cpp_run(p: &[i32], buf: &mut Vec<i32>) -> Vec<i32> {
    let n = unsafe { ref_anim2d_run(p.as_ptr(), p.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
    assert!(n >= 0, "the output buffer is too small");
    buf[..n as usize].to_vec()
}

#[test]
fn anim2d_mode_names() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut c = Check::new("Anim2DModeNames");
    let mut buf = vec![0 as std::os::raw::c_char; 64];
    for i in 0..7 {
        let n = unsafe { ref_anim2d_mode_name(i, buf.as_mut_ptr(), 64) };
        let cpp: Vec<u8> = buf[..n as usize].iter().map(|&b| b as u8).collect();
        c.eq(ANIM2D_MODE_NAMES[i as usize].as_bytes().to_vec(), cpp, || format!("mode {i}"));
    }
    let n = unsafe { ref_anim2d_mode_name(7, buf.as_mut_ptr(), 64) };
    c.eq(n, -1, || "the eighth name is the NULL end".to_string());
    if !planted() {
        c.finish();
    }
}

#[test]
fn anim2d_programs() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_A2D2);
    let mut c = Check::new("Anim2D programs");
    let mut buf = vec![0i32; 400_000];
    for case in 0..20_000 {
        let p = program(&mut g);
        let rust = run_rust(&p);
        let cpp = cpp_run(&p, &mut buf);
        let first = rust.iter().zip(cpp.iter()).position(|(a, b)| a != b);
        c.eq(rust.len() == cpp.len() && first.is_none(), true, || format!("case {case}: first differing word {first:?} (lengths {} and {})", rust.len(), cpp.len()));
    }
    finish(c);
}
