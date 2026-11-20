// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d4) of the draft `src/drafts/sweep_ww3d/ww3d.rs` against upstream
//! `WW3D2/ww3d.cpp`, cut VERBATIM (build.rs `drafts_ww3d4`, differential/wrap_draft_dw4_ww3d.cpp): `Set_NPatches_Gap_Filling_Mode`,
//! `Set_NPatches_Level`, `_Invalidate_Textures`, `Set_Texture_Filter`, `Normalize_Coordinates`,
//! `Enable_Large_Texture_Extra_Reduction`. Stand-ins (C++): the `WW3D` statics, `TheDX8MeshRenderer.Invalidate()` (a counter),
//! `DX8Wrapper::Get_Device_Resolution_*`, `TextureFilterClass::_Init_Filters`, the asset manager's texture hash (N textures),
//! `TextureLoader::Flush_Pending_Load_Tasks` and `TextureClass::Invalidate` (counters); the Rust `Ww3dHost` mock counts the same.
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides) of 1 to 14 operations over a random resolution (0 included), a texture
//! count 0 to 5 and an asset manager that exists or not. Levels include 0, 1, 2, 8, 9 and large ones; filters include negative
//! values and 0..=5; coordinates include negative values and ones past the resolution. After every operation the whole state
//! (mode, level, filter, last `_Init_Filters` argument, large-texture flag, counts of mesh invalidations, loader flushes and
//! texture invalidations) is compared, as are the two normalised floats bit for bit.
//!
//! # Not covered
//!
//! `Update_Pixel_Center`: its body is inside `#ifdef WW3D_DX8`, which no build file defines (grep over the submodule), so in the
//! shipped build the function is empty and `PixelCenterX/Y` stay 0; the draft implements the non-shipped branch (0.5 for DirectX).
//! The remaining functions only forward to the host (`DX8Wrapper::*`), and `Start_Movie_Capture` needs `GetWindowRect` and the
//! `FrameGrabClass` (OS / device).
//!
//! # Predictions (written before the first run, 2026-11-20)
//!
//! One MISS expected, from reading the two sources side by side: the draft's `TEXTURE_FILTER_ANISOTROPIC` is 3, upstream's enum
//! (texturefilter.h:86) makes it 2. Everything else 0 mismatches.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::sweep_ww3d::ww3d::*;
use std::sync::Mutex;
use support::{Check, Gen};
use support_sweep::{cases, finish};

extern "C" {
    fn ref_dw4_ww3d_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 8192];
    let n = unsafe { ref_dw4_ww3d_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0);
    out.truncate(n as usize);
    out
}

struct Host {
    w: i32,
    h: i32,
    ntex: u32,
    has_mgr: bool,
    mesh_inval: u32,
    flush: u32,
    tex_inval: u32,
    init_arg: i32,
}

impl Ww3dHost for Host {
    fn dx8_set_render_device_by_name(&mut self, _: &str, _: i32, _: i32, _: i32, _: i32, _: bool) -> bool { unimplemented!() }
    fn dx8_set_any_render_device(&mut self) -> bool { unimplemented!() }
    fn dx8_set_render_device(&mut self, _: i32, _: i32, _: i32, _: i32, _: i32, _: bool, _: bool, _: bool) -> bool { unimplemented!() }
    fn dx8_set_next_render_device(&mut self) -> bool { unimplemented!() }
    fn dx8_is_windowed(&self) -> bool { unimplemented!() }
    fn dx8_toggle_windowed(&mut self) -> bool { unimplemented!() }
    fn dx8_get_render_device(&self) -> i32 { unimplemented!() }
    fn dx8_get_render_device_desc(&self, _: i32) -> RenderDeviceDesc { unimplemented!() }
    fn dx8_get_render_device_count(&self) -> i32 { unimplemented!() }
    fn dx8_get_render_device_name(&self, _: i32) -> String { unimplemented!() }
    fn dx8_registry_save_render_device(&mut self, _: &str) -> bool { unimplemented!() }
    fn dx8_registry_save_render_device_with(&mut self, _: &str, _: i32, _: i32, _: i32, _: i32, _: bool, _: i32) -> bool { unimplemented!() }
    fn dx8_registry_load_render_device(&mut self, _: &str, _: bool) -> bool { unimplemented!() }
    fn dx8_registry_load_render_device_values(&mut self, _: &str, _: i32) -> Option<(String, i32, i32, i32, i32, i32)> { unimplemented!() }
    fn dx8_flip_to_primary(&mut self) { unimplemented!() }
    fn dx8_set_swap_interval(&mut self, _: i32) { unimplemented!() }
    fn dx8_get_swap_interval(&self) -> i32 { unimplemented!() }
    fn dx8_device_resolution_width(&self) -> i32 { self.w }
    fn dx8_device_resolution_height(&self) -> i32 { self.h }
    fn dx8_set_texture_bitdepth(&mut self, _: i32) { unimplemented!() }
    fn dx8_get_texture_bitdepth(&self) -> i32 { unimplemented!() }
    fn mesh_renderer_invalidate(&mut self) { self.mesh_inval += 1; }
    fn texture_loader_flush_pending_load_tasks(&mut self) { self.flush += 1; }
    fn asset_manager_exists(&self) -> bool { self.has_mgr }
    fn asset_manager_invalidate_all_textures(&mut self) { self.tex_inval += self.ntex; }
    fn texture_filter_init_filters(&mut self, mode: i32) { self.init_arg = mode; }
    fn debug_statistics_dx8_polygons(&self) -> u32 { unimplemented!() }
    fn debug_statistics_dx8_vertices(&self) -> u32 { unimplemented!() }
    fn box_set_display_mask(&mut self, _: i32) { unimplemented!() }
    fn box_get_display_mask(&self) -> i32 { unimplemented!() }
    fn frame_grab_create(&mut self, _: &str, _: i32, _: i32, _: i32, _: f32) { unimplemented!() }
    fn frame_grab_destroy(&mut self) { unimplemented!() }
    fn frame_grab_frame_rate(&self) -> f32 { unimplemented!() }
    fn window_size(&self) -> (i32, i32) { unimplemented!() }
    fn grab_front_buffer_frame(&mut self) { unimplemented!() }
    fn static_sort_lists_render_and_clear(&mut self) { unimplemented!() }
}

fn state(s: &Ww3dState, h: &Host) -> Vec<u32> {
    let mode = match s.n_patches_gap_filling_mode {
        NPatchesGapFillingMode::Disabled => 0,
        NPatchesGapFillingMode::Enabled => 1,
        NPatchesGapFillingMode::Strong => 2,
    };
    vec![mode, s.n_patches_level, s.texture_filter as u32, h.init_arg as u32, s.large_texture_extra_reduction_enabled as u32, h.mesh_inval, h.flush, h.tex_inval]
}

#[test]
fn ww3d_statics_match_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_d401);
    let names = ["Set_NPatches_Gap_Filling_Mode", "Set_NPatches_Level", "Set_Texture_Filter", "Normalize_Coordinates", "Enable_Large_Texture_Extra_Reduction (+ _Invalidate_Textures)"];
    let mut c: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    for case in 0..cases() {
        let (w, hh) = match g.below(6) {
            0 => (0, 0),
            1 => (1, 1),
            _ => (g.below(4000) as i32, g.below(3000) as i32),
        };
        let ntex = g.below(6);
        let has_mgr = g.below(4) != 0;
        let mut inp = vec![w as u32, hh as u32, ntex, has_mgr as u32];
        let mut host = Host { w, h: hh, ntex, has_mgr, mesh_inval: 0, flush: 0, tex_inval: 0, init_arg: -1 };
        let mut st = Ww3dState::default();
        let nops = 1 + g.below(14);
        inp.push(nops);
        let mut rust: Vec<(usize, Vec<u32>)> = Vec::new();
        for _ in 0..nops {
            match g.below(5) {
                0 => {
                    let m = g.below(3);
                    inp.extend([0, m]);
                    let mode = [NPatchesGapFillingMode::Disabled, NPatchesGapFillingMode::Enabled, NPatchesGapFillingMode::Strong][m as usize];
                    st.set_n_patches_gap_filling_mode(&mut host, mode);
                    rust.push((0, state(&st, &host)));
                }
                1 => {
                    let l = match g.below(4) {
                        0 => g.below(4),
                        1 => 7 + g.below(4),
                        2 => g.below(40),
                        _ => g.u32(),
                    };
                    inp.extend([1, l]);
                    st.set_n_patches_level(&mut host, l);
                    rust.push((1, state(&st, &host)));
                }
                2 => {
                    let f = match g.below(3) {
                        0 => g.below(7) as i32 - 2,
                        1 => g.i32(),
                        _ => g.below(4) as i32,
                    };
                    inp.extend([2, f as u32]);
                    st.set_texture_filter(&mut host, f);
                    rust.push((2, state(&st, &host)));
                }
                3 => {
                    let mut coord = |g: &mut Gen, r: i32| -> i32 {
                        match g.below(5) {
                            0 => -(g.below(100) as i32),
                            1 => r,
                            2 => r + g.below(50) as i32,
                            3 => 0,
                            _ => g.below((r.max(1)) as u32) as i32,
                        }
                    };
                    let x = coord(&mut g, w);
                    let y = coord(&mut g, hh);
                    inp.extend([3, x as u32, y as u32]);
                    let (fx, fy) = st.normalize_coordinates(&host, x, y);
                    rust.push((3, vec![fx.to_bits(), fy.to_bits()]));
                }
                _ => {
                    let on = g.below(2);
                    inp.extend([4, on]);
                    st.enable_large_texture_extra_reduction(&mut host, on != 0);
                    rust.push((4, state(&st, &host)));
                }
            }
        }
        let cpp = run(&inp);
        let mut at = 0;
        for (which, r) in rust {
            let n = r.len() + if which == 3 { 8 } else { 0 };
            // op 3 emits [fx, fy] and then the state; the state is checked for every op through the next comparison
            let take = &cpp[at..at + n];
            if which == 3 {
                c[3].eq(r.clone(), take[..2].to_vec(), || format!("case {case}: {inp:?}"));
            } else {
                c[which].eq(r.clone(), take.to_vec(), || format!("case {case}: {inp:?}"));
            }
            at += n;
        }
        assert_eq!(at, cpp.len());
    }
    finish(c);
}
