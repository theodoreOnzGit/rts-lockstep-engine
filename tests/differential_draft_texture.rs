// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d4) of the draft `src/drafts/sweep_ww3d/texture.rs` against upstream
//! `WW3D2/texture.cpp`, cut VERBATIM (build.rs `drafts_ww3d4`, differential/wrap_draft_dw4_texture.cpp):
//! `TextureBaseClass::{Invalidate_Old_Unused_Textures, Invalidate, Peek_D3D_Base_Texture, Set_D3D_Base_Texture,
//! Is_Missing_Texture, Get_Reduction, Set_HSV_Shift}` and the eight `_Get_Total_*` statistics. Stand-ins (C++): the fields and
//! members of `TextureBaseClass` the cuts use, `IDirect3DBaseTexture8` (logs `AddRef`/`Release`), `WW3D::Get_*`, the asset
//! manager's texture hash (the test's textures), `MissingTexture::_Get_Missing_Texture`. The Rust side has a mock `TextureHost`.
//!
//! # Method
//!
//! 20,000 cases (fixed seed, `DRAFT_CASES` overrides): 0 to 6 textures with random flags (initialised, load task, thumbnail task,
//! procedural, lightmap), inactivation times, access times near 0 and near the `u32` wrap, sizes 0 to 600 (the 32 and 256 limits
//! hit exactly), mip counts 0 to 9, D3D handles, memory sizes including ones whose sum passes `i32::MAX`; a global texture
//! reduction -2 to 6 (negative included), the large-texture option, thumbnails on or off; then 1 to 8 operations over those textures.
//! After every operation the result, every texture's state, `TexturesAppliedPerFrame` and the log of D3D `AddRef`/`Release`
//! calls are compared.
//!
//! # Predictions (written before the first run, 2026-11-20)
//!
//! Three MISSES expected from reading the sources: (1) upstream `Invalidate` ends with `LastAccessed=WW3D::Get_Sync_Time();`,
//! the draft omits it; (2) `Get_Reduction` compares `reduction>MipLevelCount` as `unsigned` (int against unsigned), so a negative
//! global reduction is huge and is capped to `MipLevelCount`, where the draft compares as `i32`; (3) the size totals add an
//! `unsigned` to an `int` (wrapping), where the draft's `sum::<i32>()` panics on overflow in release (overflow checks are on).
//! Everything else 0.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::sweep_ww3d::texture::*;
use std::cell::RefCell;
use std::sync::Mutex;
use support::{Check, Gen};
use support_sweep::{cases, finish};

extern "C" {
    fn ref_dw4_tex_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 16384];
    let n = unsafe { ref_dw4_tex_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0);
    out.truncate(n as usize);
    out
}

struct Host {
    sync: u32,
    thumb: bool,
    reduction: i32,
    large: bool,
    missing: Option<usize>,
    events: RefCell<Vec<u32>>,
}

impl TextureHost for Host {
    fn ww3d_thumbnail_enabled(&self) -> bool { self.thumb }
    fn ww3d_sync_time(&self) -> u32 { self.sync }
    fn ww3d_texture_reduction(&self) -> i32 { self.reduction }
    fn ww3d_large_texture_extra_reduction_enabled(&self) -> bool { self.large }
    fn d3d_release(&mut self, t: usize) { self.events.borrow_mut().push(t as u32); }
    fn d3d_add_ref(&mut self, t: usize) { self.events.borrow_mut().push(t as u32 | 0x10000); }
    fn missing_texture(&self) -> Option<usize> { self.missing }
    fn dx8_set_texture_null(&mut self, _: u32) { unimplemented!() }
    fn d3d_level_sizes(&self, _: usize) -> Vec<u32> { unimplemented!() }
    fn d3d_get_surface_level(&mut self, _: usize, _: u32) -> usize { unimplemented!() }
}

fn dump(texs: &[TextureBase], st: &TextureStatics, host: &Host) -> Vec<u32> {
    let mut o = Vec::new();
    for t in texs {
        o.extend([t.initialized as u32, t.d3d_texture.unwrap_or(0) as u32, t.last_accessed, t.last_inactivation_sync_time]);
        o.extend(t.hsv_shift.iter().map(|x| x.to_bits()));
    }
    o.push(st.textures_applied_per_frame);
    let ev = host.events.borrow();
    o.push(ev.len() as u32);
    o.extend(ev.iter());
    o
}

#[test]
fn texture_base_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_d405);
    let names = [
        "Invalidate_Old_Unused_Textures",
        "Invalidate",
        "Peek_D3D_Base_Texture",
        "Set_D3D_Base_Texture",
        "Get_Reduction",
        "Set_HSV_Shift",
        "_Get_Total_* (8 statistics)",
        "Is_Missing_Texture",
    ];
    let mut c: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    for case in 0..cases() {
        let near_wrap = g.below(4) == 0;
        let sync = if near_wrap { u32::MAX - g.below(2000) } else { g.below(100000) };
        let thumb = g.below(5) != 0;
        let reduction = g.below(9) as i32 - 2;
        let large = g.below(2) == 0;
        let missing_id = if g.below(3) == 0 { 0 } else { 1 + g.below(20) };
        let ovr = [250, 1000, 1][g.below(3) as usize];
        let n = g.below(7) as usize;
        let mut inp = vec![sync, thumb as u32, reduction as u32, large as u32, missing_id, n as u32];
        let mut texs: Vec<TextureBase> = Vec::new();
        for _ in 0..n {
            let size = |g: &mut Gen| match g.below(4) {
                0 => [0, 1, 32, 33, 256, 257][g.below(6) as usize],
                _ => g.below(600),
            };
            let inact = if g.below(3) == 0 { 0 } else { g.below(3000) };
            let ext = if g.below(3) == 0 { g.below(3000) } else { 0 };
            // exact ties: the age equal to the override, or to the inactivation time plus the extended one
            let last_accessed = match g.below(5) {
                0 => sync.wrapping_sub(ovr),
                1 => sync.wrapping_sub(inact.wrapping_add(ext)),
                _ => {
                    if near_wrap {
                        u32::MAX - g.below(5000)
                    } else {
                        g.below(100000)
                    }
                }
            };
            let t = TextureBase {
                mip_level_count: [0, 1, 2, 5, 9][g.below(5) as usize],
                width: size(&mut g),
                height: size(&mut g),
                d3d_texture: if g.below(4) == 0 { None } else { Some(1 + g.below(20) as usize) },
                initialized: g.below(3) != 0,
                name: String::new(),
                is_lightmap: g.below(3) == 0,
                is_procedural: g.below(4) == 0,
                inactivation_time: inact,
                extended_inactivation_time: ext,
                last_inactivation_sync_time: g.below(1000),
                last_accessed,
                texture_load_task: g.below(6) == 0,
                thumbnail_load_task: g.below(6) == 0,
                hsv_shift: [g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)],
                memory_usage: if g.below(4) == 0 { 0x4000_0000 + g.below(0x4000_0000) } else { g.below(1_000_000) },
            };
            inp.extend([
                t.initialized as u32, t.inactivation_time, t.extended_inactivation_time, t.last_inactivation_sync_time, t.last_accessed, t.width, t.height,
                t.mip_level_count, t.d3d_texture.unwrap_or(0) as u32, t.texture_load_task as u32, t.thumbnail_load_task as u32, t.is_procedural as u32,
                t.is_lightmap as u32, t.memory_usage,
            ]);
            inp.extend(t.hsv_shift.iter().map(|x| x.to_bits()));
            texs.push(t);
        }
        let mut host = Host { sync, thumb, reduction, large, missing: if missing_id == 0 { None } else { Some(missing_id as usize) }, events: RefCell::new(Vec::new()) };
        let mut st = TextureStatics { textures_applied_per_frame: 7 };
        let nops = 1 + g.below(8);
        inp.push(nops);
        let mut rust: Vec<(usize, Vec<u32>)> = Vec::new();
        for _ in 0..nops {
            let ti = if n == 0 { 0 } else { g.below(n as u32) as usize };
            let mut op = g.below(8) as usize;
            if n == 0 && matches!(op, 1 | 2 | 3 | 4 | 5 | 7) {
                op = 0;
            }
            let (a0, a1, a2) = match op {
                0 => (if g.below(2) == 0 { 0 } else { ovr }, 0, 0),
                3 => (if g.below(3) == 0 { 0 } else { 1 + g.below(20) }, 0, 0),
                5 => (g.uniform(-1.0, 1.0).to_bits(), g.uniform(-1.0, 1.0).to_bits(), g.uniform(-1.0, 1.0).to_bits()),
                _ => (0, 0, 0),
            };
            inp.extend([op as u32, ti as u32, a0, a1, a2]);
            host.events.borrow_mut().clear();
            let mut res: Vec<u32> = Vec::new();
            match op {
                0 => TextureBase::invalidate_old_unused_textures(&mut host, &mut st, &mut texs, a0),
                1 => texs[ti].invalidate(&mut host),
                2 => res.push(texs[ti].peek_d3d_base_texture(&host).unwrap_or(0) as u32),
                3 => texs[ti].set_d3d_base_texture(&mut host, if a0 == 0 { None } else { Some(a0 as usize) }),
                4 => res.push(texs[ti].get_reduction(&host)),
                5 => texs[ti].set_hsv_shift(&mut host, [f32::from_bits(a0), f32::from_bits(a1), f32::from_bits(a2)]),
                6 => res.extend([
                    get_total_locked_surface_size(&texs) as u32,
                    get_total_texture_size(&texs) as u32,
                    get_total_lightmap_texture_size(&texs) as u32,
                    get_total_procedural_texture_size(&texs) as u32,
                    get_total_texture_count(&texs) as u32,
                    get_total_lightmap_texture_count(&texs) as u32,
                    get_total_procedural_texture_count(&texs) as u32,
                    get_total_locked_surface_count(&texs) as u32,
                ]),
                _ => {
                    res.push(texs[ti].is_missing_texture(&host) as u32);
                    host.events.borrow_mut().clear();
                }
            }
            res.extend(dump(&texs, &st, &host));
            rust.push((op, res));
        }
        let cpp = run(&inp);
        let mut at = 0;
        for (op, r) in rust {
            let len = cpp[at] as usize;
            c[op].eq(r, cpp[at + 1..at + 1 + len].to_vec(), || format!("case {case}: {inp:?}"));
            at += 1 + len;
        }
        assert_eq!(at, cpp.len());
    }
    finish(c);
}
