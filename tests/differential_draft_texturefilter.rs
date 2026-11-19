// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3dlogic) of the draft `src/drafts/ww3d/texturefilter.rs`
//! (`TextureFilterTables`, `TextureFilterClass`) against upstream `WW3D2/texturefilter.cpp` compiled WHOLE
//! (differential/wrap_draft_texturefilter.cpp; build.rs `drafts_ww3dlogic` copies it with two mechanical edits: the
//! stand-in `dx8wrapper.h`, and a loop variable that MSVC 6 leaked out of its `for`). Stand-ins: the device caps
//! (`TextureFilterCaps`) and `Set_DX8_Texture_Stage_State`, which logs.
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides), each starting from zeroed tables (as the globals start): 1 to 14
//! operations from `_Init_Filters` (all three modes, random caps words made of the DX8 `D3DPTFILTERCAPS_*` bits and
//! random other bits), `_Set_Default_{Min,Mag,Mip}_Filter`, constructing a filter (every `MipCountType`), the
//! `Set_*` calls, `Apply(stage)` (stages 0 to 7) and the getters; the three tables are dumped at the end of every
//! sequence and after every `_Init_Filters`. All outputs are compared.
//!
//! # The D3D constants
//!
//! The DirectX 8 SDK headers are not in the upstream tree. The stand-in and this test use the SDK's published values
//! (`D3DPTFILTERCAPS_MINFPOINT` 0x100, `MINFLINEAR` 0x200, `MINFANISOTROPIC` 0x400, `MIPFPOINT` 0x10000, `MIPFLINEAR`
//! 0x20000, `MAGFPOINT` 0x1000000, `MAGFLINEAR` 0x2000000, `MAGFANISOTROPIC` 0x4000000; `D3DTEXF_*` 0..3,
//! `D3DTADDRESS_WRAP` 1, `CLAMP` 3), written from the SDK, not checkable against anything in this repository.
//!
//! # Predictions (before the first run, 2026-11-19)
//!
//! The draft's control flow is a line-by-line copy, so I expect 0 mismatches EXCEPT that the draft's `MINFLINEAR` (0x100)
//! and `MIPFLINEAR` (0x10000) look like the SDK's POINT bits: I expect those two constants to be the mismatches.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;

use rts_lockstep_engine::drafts::ww3d::texturefilter::*;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dww3dl_tf_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn finish(chk: Vec<Check>) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut bad = 0;
    for c in chk {
        println!("DIFF {} compared {} mismatches {} nan-payload-only {} max-ulp {}", c.name, c.compared, c.mismatches, c.nan_only, c.max_ulp);
        bad += c.mismatches;
        if !planted {
            c.finish();
        }
    }
    if planted {
        println!("PLANTED mismatches {bad}");
        assert!(bad > 0, "plant not caught");
    }
}

fn ft(i: u32) -> FilterType {
    [FilterType::None, FilterType::Fast, FilterType::Best, FilterType::Default][i as usize]
}
fn mip(i: u32) -> MipCountType {
    use MipCountType::*;
    [MipLevelsAll, MipLevels1, MipLevels2, MipLevels3, MipLevels4, MipLevels5, MipLevels6, MipLevels7, MipLevels8, MipLevels10, MipLevels11, MipLevels12][i as usize]
}
fn mode(i: u32) -> TextureFilterMode {
    [TextureFilterMode::Bilinear, TextureFilterMode::Trilinear, TextureFilterMode::Anisotropic][i as usize]
}
fn am(i: u32) -> TxtAddrMode {
    [TxtAddrMode::Repeat, TxtAddrMode::Clamp][i as usize]
}

fn dump(t: &TextureFilterTables) -> Vec<u32> {
    let mut w = Vec::new();
    for tab in [&t.min_texture_filters, &t.mag_texture_filters, &t.mip_map_filters] {
        for s in 0..8 {
            for k in 0..4 {
                w.push(tab[s][k]);
            }
        }
    }
    w
}

fn caps(g: &mut Gen) -> u32 {
    const BITS: [u32; 8] = [0x100, 0x200, 0x400, 0x10000, 0x20000, 0x1000000, 0x2000000, 0x4000000];
    let mut c = 0;
    match g.below(6) {
        0 => c = g.u32(),
        1 => c = 0,
        2 => c = 0x0700_0000 | 0x0003_0000 | 0x0700,
        _ => {
            for b in BITS {
                if g.below(2) == 0 {
                    c |= b;
                }
            }
        }
    }
    c
}

#[test]
fn texture_filter_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_13_03);
    let names = ["_Init_Filters + table dump", "_Set_Default_*_Filter + table dump", "TextureFilterClass getters", "TextureFilterClass::Apply"];
    let mut chk: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    for case in 0..cases() {
        let mut tables = TextureFilterTables::default();
        let mut f = TextureFilterClass::new(MipCountType::MipLevelsAll);
        let nops = 1 + g.below(14);
        let mut input = vec![nops];
        // (check index, expected words) in output order
        let mut expect: Vec<(usize, Vec<u32>)> = Vec::new();
        let mut initialised = false;
        for k in 0..nops {
            // the first operation is usually an _Init_Filters, as the game does, but not always
            let op = if k == 0 && g.below(4) != 0 { 0 } else { g.below(13) };
            match op {
                0 => {
                    let (m, c) = (g.below(3), caps(&mut g));
                    input.extend([0, m, c]);
                    tables.init_filters(mode(m), c);
                    initialised = true;
                    expect.push((0, dump(&tables)));
                }
                1 | 2 | 3 => {
                    let fi = g.below(4);
                    input.extend([op, fi]);
                    match op {
                        1 => tables.set_default_min_filter(ft(fi)),
                        2 => tables.set_default_mag_filter(ft(fi)),
                        _ => tables.set_default_mip_filter(ft(fi)),
                    }
                }
                4 => {
                    let mc = g.below(12);
                    input.extend([4, mc]);
                    f = TextureFilterClass::new(mip(mc));
                }
                5 => {
                    let fi = g.below(4);
                    input.extend([5, fi]);
                    f.set_min_filter(ft(fi));
                }
                6 => {
                    let fi = g.below(4);
                    input.extend([6, fi]);
                    f.set_mag_filter(ft(fi));
                }
                7 => {
                    let fi = g.below(4);
                    input.extend([7, fi]);
                    f.set_mip_mapping(ft(fi));
                }
                8 => {
                    let m = g.below(2);
                    input.extend([8, m]);
                    f.set_u_addr_mode(am(m));
                }
                9 => {
                    let m = g.below(2);
                    input.extend([9, m]);
                    f.set_v_addr_mode(am(m));
                }
                10 => {
                    let stage = g.below(8);
                    input.extend([10, stage]);
                    let s = f.apply_state(&tables, stage as usize);
                    // the C++ log: (stage, state, value) x 5: MINFILTER 17, MAGFILTER 16, MIPFILTER 18, ADDRESSU 13, ADDRESSV 14
                    let w = vec![
                        15, stage, 17, s.min_filter, stage, 16, s.mag_filter, stage, 18, s.mip_filter, stage, 13, s.address_u, stage, 14, s.address_v,
                    ];
                    expect.push((3, w));
                }
                11 => {
                    input.extend([11]);
                    expect.push((2, vec![f.get_min_filter() as u32, f.get_mag_filter() as u32, f.get_mip_mapping() as u32, f.get_u_addr_mode() as u32, f.get_v_addr_mode() as u32]));
                }
                _ => {
                    input.extend([12]);
                    expect.push((1, dump(&tables)));
                }
            }
        }
        let _ = initialised;
        input.push(0); // padding, never read
        let mut out = vec![0u32; 8192];
        let n = unsafe { ref_dww3dl_tf_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
        assert!(n >= 0, "case {case}: output buffer too small");
        let mut pos = 0;
        for (idx, w) in &expect {
            for (k, &rw) in w.iter().enumerate() {
                chk[*idx].eq(rw, out[pos + k], || format!("case {case} word {k} of {w:?}"));
            }
            pos += w.len();
        }
        assert_eq!(pos as i32, n, "case {case}: output length");
    }
    finish(chk);
}
