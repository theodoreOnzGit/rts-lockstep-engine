// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d4) of the draft `src/drafts/sweep_ww3d/textureloader.rs` against upstream
//! `WW3D2/textureloader.cpp`, cut VERBATIM (build.rs `drafts_ww3d4`, differential/wrap_draft_dw4_texldr.cpp):
//! `TextureLoadTaskListClass::{TextureLoadTaskListClass, Push_Front, Push_Back, Pop_Front, Pop_Back, Remove}` against
//! `TextureLoadTaskList`, and the file-static `Get_Texture_Information` against `TextureLoaderState::get_texture_information`.
//! Stand-ins (C++): the list and node classes are textureloader.h's declarations (copied, `Root` public so the test can walk it);
//! `ThumbnailManagerClass`, `DDSFileClass`, `Targa` (`Open`, `Header.Width/Height`), `Get_WW3D_Format` and `WW3D::Get_Texture_*`
//! return what the test configures. The Rust side uses a mock `TextureLoaderHost` that returns the same configuration.
//!
//! # Method
//!
//! Lists: 20,000 sequences (fixed seed, `DRAFT_CASES` overrides) of 1 to 30 operations over 2 lists and 1 to 6 tasks:
//! `Push_Front`, `Push_Back` (only for a task on no list, the precondition upstream asserts), `Pop_Front`, `Pop_Back` (empty lists
//! included), `Remove` (a task on the other list, on this one, and on none). After every operation the popped task, the content
//! of both lists front to back, `Is_Empty` and the `List` of every task are compared.
//! Info: 20,000 configurations (thumbnail present or not with every format, DDS available or not, Targa openable or not with sizes
//! 0 to 4096 and a few up to 32767, texture reduction -2 to 12, minimum dimension 0 to 16, mip counts 0 to 13), 1 to 3 calls each
//! with `compressed` both ways and an incoming reduction 0 to 20. Success, reduction, w, h, format and mip count are compared.
//!
//! # Excluded inputs
//!
//! The thumbnail branch leaves `d` untouched upstream (the caller's uninitialised local); the draft returns 1, so `d` is compared
//! only for the DDS and Targa branches. Thumbnail incoming reductions are below 32 (a shift by 32 or more is undefined in C++).
//! The caller `Begin_Compressed_Load` passes an UNINITIALISED `reduction` to this function (textureloader.cpp:1411), so with a
//! thumbnail the shift in the C++ is by an indeterminate value: an upstream defect, recorded on #215; the draft takes the
//! reduction as an argument.
//!
//! # Predictions (written before the first run, 2026-11-20)
//!
//! Lists: 0 mismatches. Info: one MISS expected for a negative requested reduction, `reqReduction >= mip_count` compares an `int` with an
//! `unsigned` upstream (so -1 is huge and is clamped to `mip_count - 1`), where the draft compares as `i32`; and `mip_count == 0`
//! gives `mip_count - 1 == UINT_MAX`, i.e. -1, in the same line. Everything else 0.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::sweep_ww3d::textureloader::*;
use rts_lockstep_engine::formats::texture::format::Ww3dFormat;
use std::sync::Mutex;
use support::{Check, Gen};
use support_sweep::{cases, finish};

extern "C" {
    fn ref_dw4_tl_list(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
    fn ref_dw4_tl_info(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn call(f: unsafe extern "C" fn(*const u32, i32, *mut u32, i32) -> i32, input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 16384];
    let n = unsafe { f(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0);
    out.truncate(n as usize);
    out
}

#[test]
fn task_lists_match_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_d402);
    let names = ["TextureLoadTaskListClass push/pop/remove"];
    let mut c: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    for case in 0..cases() {
        let nl = 2usize;
        let nt = 1 + g.below(6) as usize;
        let mut inp = vec![nl as u32, nt as u32];
        let mut state = TextureLoaderState::default();
        let ids = [ListId::Foreground, ListId::Background];
        let mut lists = [TextureLoadTaskList::new(ids[0]), TextureLoadTaskList::new(ids[1])];
        let mut tasks: Vec<TextureLoadTask> = (0..nt).map(|_| TextureLoadTask::new(TaskKind::Regular)).collect();
        let _ = &mut state;
        // the generator's own model of the two lists (correct semantics), so that the inputs sent to the C++ never depend on the draft
        let mut model: [std::collections::VecDeque<usize>; 2] = [Default::default(), Default::default()];
        let nops = 1 + g.below(30);
        inp.push(nops);
        let mut rust: Vec<u32> = Vec::new();
        for _ in 0..nops {
            let l = g.below(2) as usize;
            let t = g.below(nt as u32) as usize;
            let mut res = 0xFFFF_FFFFu32;
            match g.below(5) {
                0 | 1 => {
                    if model.iter().any(|m| m.contains(&t)) {
                        // push precondition (WWASSERT upstream): fall back to Remove
                        inp.extend([4, l as u32, t as u32]);
                        lists[l].remove(&mut tasks, t);
                        model[l].retain(|&x| x != t);
                    } else if g.below(2) == 0 {
                        inp.extend([0, l as u32, t as u32]);
                        lists[l].push_front(&mut tasks, t);
                        model[l].push_front(t);
                    } else {
                        inp.extend([1, l as u32, t as u32]);
                        lists[l].push_back(&mut tasks, t);
                        model[l].push_back(t);
                    }
                }
                2 => {
                    inp.extend([2, l as u32]);
                    if let Some(x) = lists[l].pop_front(&mut tasks) {
                        res = x as u32;
                    }
                    model[l].pop_front();
                }
                3 => {
                    inp.extend([3, l as u32]);
                    if let Some(x) = lists[l].pop_back(&mut tasks) {
                        res = x as u32;
                    }
                    model[l].pop_back();
                }
                _ => {
                    inp.extend([4, l as u32, t as u32]);
                    lists[l].remove(&mut tasks, t);
                    model[l].retain(|&x| x != t);
                }
            }
            rust.push(res);
            for list in &lists {
                rust.extend(list.items.iter().map(|&x| x as u32));
                rust.push(0xFFFF_FFFE);
                rust.push(list.is_empty() as u32);
            }
            for task in &tasks {
                rust.push(match task.list {
                    Some(id) if id == ids[0] => 0,
                    Some(_) => 1,
                    None => 0xFFFF_FFFF,
                });
            }
        }
        let cpp = call(ref_dw4_tl_list, &inp);
        c[0].eq(rust, cpp, || format!("case {case}: {inp:?}"));
    }
    finish(c);
}

struct Host {
    thumb: Option<ThumbnailInfo>,
    dds: Option<(u32, u32, u32, Ww3dFormat, u32)>,
    tga: Option<(u32, u32, Ww3dFormat)>,
    reduction: i32,
    min_dim: i32,
}

impl TextureLoaderHost for Host {
    fn is_dx8_thread(&self) -> bool { unimplemented!() }
    fn tex_asset_type(&self, _: usize) -> TaskKind { unimplemented!() }
    fn tex_full_path(&self, _: usize) -> String { unimplemented!() }
    fn tex_mip_level_count(&self, _: usize) -> u32 { unimplemented!() }
    fn tex_reduction(&self, _: usize) -> u32 { unimplemented!() }
    fn tex_hsv_shift(&self, _: usize) -> [f32; 3] { unimplemented!() }
    fn tex_format(&self, _: usize) -> Ww3dFormat { unimplemented!() }
    fn tex_is_compression_allowed(&self, _: usize) -> bool { unimplemented!() }
    fn tex_is_reducible(&self, _: usize) -> bool { unimplemented!() }
    fn tex_is_initialized(&self, _: usize) -> bool { unimplemented!() }
    fn tex_has_d3d_texture(&self, _: usize) -> bool { unimplemented!() }
    fn tex_apply_new_surface(&mut self, _: usize, _: usize, _: bool) { unimplemented!() }
    fn max_texture_extents(&self) -> (u32, u32, u32) { unimplemented!() }
    fn get_valid_texture_format(&self, _: Ww3dFormat, _: bool) -> Ww3dFormat { unimplemented!() }
    fn create_texture(&mut self, _: u32, _: u32, _: Ww3dFormat, _: u32) -> usize { unimplemented!() }
    fn create_cube_texture(&mut self, _: u32, _: u32, _: Ww3dFormat, _: u32) -> usize { unimplemented!() }
    fn create_volume_texture(&mut self, _: u32, _: u32, _: u32, _: Ww3dFormat, _: u32) -> usize { unimplemented!() }
    fn texture_level_count(&self, _: usize) -> u32 { unimplemented!() }
    fn lock_rect(&mut self, _: usize, _: u32) -> LockedSurface { unimplemented!() }
    fn lock_cube_rect(&mut self, _: usize, _: u32, _: u32) -> LockedSurface { unimplemented!() }
    fn lock_box(&mut self, _: usize, _: u32) -> LockedSurface { unimplemented!() }
    fn unlock_rect(&mut self, _: usize, _: u32) { unimplemented!() }
    fn unlock_cube_rect(&mut self, _: usize, _: u32, _: u32) { unimplemented!() }
    fn unlock_box(&mut self, _: usize, _: u32) { unimplemented!() }
    fn release_texture(&mut self, _: usize) { unimplemented!() }
    fn missing_texture(&mut self) -> usize { unimplemented!() }
    fn load_dds_levels(&mut self, _: &TextureLoadTask, _: u32) -> bool { unimplemented!() }
    fn load_targa_levels(&mut self, _: &TextureLoadTask) -> bool { unimplemented!() }
    fn invalidate_old_unused_textures(&mut self, _: i32) { unimplemented!() }
    fn thread_switch(&mut self) { unimplemented!() }
    fn thumbnail_info(&self, _: &str) -> Option<ThumbnailInfo> { self.thumb }
    fn dds_info(&self, _: &str) -> Option<(u32, u32, u32, Ww3dFormat, u32)> { self.dds }
    fn targa_info(&self, _: &str) -> Option<(u32, u32, Ww3dFormat)> { self.tga }
    fn ww3d_texture_reduction(&self) -> i32 { self.reduction }
    fn ww3d_texture_min_dimension(&self) -> i32 { self.min_dim }
}

fn fmt(i: u32) -> Ww3dFormat {
    Ww3dFormat::ALL[i as usize]
}

#[test]
fn texture_information_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_d403);
    let mut c = Check::new("Get_Texture_Information");
    let state = TextureLoaderState::default();
    for case in 0..cases() {
        let size = |g: &mut Gen| -> u32 {
            match g.below(6) {
                0 => 0,
                1 => 1 << g.below(13),
                2 => 1 + g.below(32767),
                _ => 1 + g.below(1100),
            }
        };
        let thumb_present = g.below(3) == 0;
        let tfmt = if g.below(2) == 0 { 20 + g.below(5) } else { g.below(25) };
        let (tw, th, tm) = (size(&mut g), size(&mut g), g.below(14));
        let dds_ok = g.below(4) != 0;
        let (dw, dh, dd, dm) = (size(&mut g), size(&mut g), g.below(40), g.below(14));
        let dfmt = g.below(25);
        let tga_ok = g.below(4) != 0;
        let (gw, gh) = (size(&mut g), size(&mut g));
        let gfmt = g.below(25);
        let reduction = g.below(15) as i32 - 2;
        let min_dim = g.below(17) as i32;
        let mut inp = vec![thumb_present as u32, tfmt, tw, th, tm, dds_ok as u32, dw, dh, dd, dfmt, dm, tga_ok as u32, gw, gh, gfmt, reduction as u32, min_dim as u32];
        let host = Host {
            thumb: thumb_present.then(|| ThumbnailInfo { original_width: tw, original_height: th, original_mip_level_count: tm, original_format: fmt(tfmt) }),
            dds: dds_ok.then(|| (dw, dh, dd, fmt(dfmt), dm)),
            tga: tga_ok.then(|| (gw, gh, fmt(gfmt))),
            reduction,
            min_dim,
        };
        let ncalls = 1 + g.below(3);
        inp.push(ncalls);
        let mut rust: Vec<u32> = Vec::new();
        for _ in 0..ncalls {
            let rin = g.below(21);
            let comp = g.below(2);
            inp.extend([rin, comp]);
            match state.get_texture_information(&host, "x", rin, comp != 0) {
                None => rust.push(0),
                Some(i) => {
                    // `d` is left untouched by the thumbnail branch upstream (see Excluded inputs)
                    let d = if thumb_present { 0 } else { i.d };
                    rust.extend([1, i.reduction, i.w, i.h, d, Ww3dFormat::ALL.iter().position(|&f| f == i.format).unwrap() as u32, i.mip_count]);
                }
            }
        }
        let mut cpp = call(ref_dw4_tl_info, &inp);
        if thumb_present {
            // blank `d` (index 4 of each success record) as the Rust side did
            let mut k = 0;
            while k < cpp.len() {
                if cpp[k] == 1 {
                    cpp[k + 4] = 0;
                    k += 7;
                } else {
                    k += 1;
                }
            }
        }
        c.eq(rust, cpp, || format!("case {case}: {inp:?}"));
    }
    finish(vec![c]);
}
