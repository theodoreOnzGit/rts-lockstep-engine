// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d7) of the vertex-stream copy functions of `src/drafts/ww3d/shdchunk.rs`
//! (`copy_vertex_stream_xyzn_duv1`, `..._tg3`, `..._cubemap`) against the nine `Copy_Vertex_Stream` members of the upstream wwshade shaders
//! (Shd6Simple, Shd6GlossMask, Shd6BumpDiff, Shd6BumpSpec: UV1; Shd7BumpDiff, Shd7BumpSpec, Shd8BumpDiff, Shd8BumpSpec: TG3; Shd6CubeMap),
//! cut verbatim by build.rs `drafts_ww3d7` (differential/wrap_draft_ww3d7_shdcopy.cpp lists the cuts and the stand-ins). Also compared: the
//! three vertex layouts' sizes (the words per vertex). `MAX_TEXTURE_STAGES` is compared in differential_draft_ww3d7_shd.rs.
//!
//! # Method
//! `DRAFT_CASES` (default 20,000) cases per kind from a fixed seed: 0..=6 vertices, each stream present or NULL by a random mask (a UV stream in
//! slot 1 only is included), values from `Gen::mixed` (bit patterns, NaN, infinities, subnormals included) and random diffuse words. Every output
//! word of the vertex array is compared bit for bit (the C++ buffer is pre-filled with 0xA5A5A5A5, so an unwritten word shows).
//!
//! # Excluded inputs
//! None (all streams NULL, zero vertices and NaN payloads included).
//!
//! # Predictions (2026-11-20, before the first run)
//! 0 mismatches (I diffed the nine upstream bodies: identical within each layout, as the draft says).
//!
//! # Results
//! 2026-11-20, `DRAFT_CASES` default (20,000), fixed seed: **0 mismatches** in all nine shaders (20,000 each; 180,000 vertex arrays compared word for word);
//! the prediction held. (`MAX_TEXTURE_STAGES`, 4 in the draft and 8 upstream, was found by differential_draft_ww3d7_shd.rs and fixed before this ran.)
//!
//! # Planted bugs (`L9_PLANT=<id>`; mismatching cases of 20,000, per shader)
//! 19511 (missing diffuse defaults to 0) about 8,500 in each of the nine; 19512 (u and v swapped) about 8,500 in each of the eight with UVs; 19513 (S and T
//! swapped) 12,805 to 13,075 in the four TG3 shaders; 19514 (cube map drops the diffuse alpha) 17,184; 19515 (missing locations default to 1) about 8,500 in
//! each of the nine; 19516 (UV slot 1 read) 11,267 to 11,580 in the eight with UVs. **6 of 6 caught.**

mod support;

use rts_lockstep_engine::drafts::ww3d::shdchunk::{
    copy_vertex_stream_cubemap, copy_vertex_stream_xyzn_duv1, copy_vertex_stream_xyzn_duv1_tg3, VertexStreamStruct, };
use rts_lockstep_engine::ww::vector2::Vector2;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dw7_shdcopy(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn finish(chk: Vec<Check>) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut bad = 0;
    for c in &chk {
        println!("DIFF {} compared {} mismatches {} nan-payload-only {} max-ulp {}", c.name, c.compared, c.mismatches, c.nan_only, c.max_ulp);
        for e in c.examples.iter().take(2) {
            println!("EX {}: {}", c.name, &e[..e.len().min(1200)]);
        }
        bad += c.mismatches;
    }
    if !planted {
        assert_eq!(bad, 0, "mismatches against the C++");
    } else {
        println!("PLANTED mismatches {bad}");
        assert!(bad > 0, "plant not caught");
    }
}

fn vec3s(g: &mut Gen, n: usize, inp: &mut Vec<u32>) -> Vec<Vector3> {
    (0..n)
        .map(|_| {
            let v = Vector3::new(g.mixed(), g.mixed(), g.mixed());
            inp.extend([v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]);
            v
        })
        .collect()
}

fn case(g: &mut Gen, c: &mut Check, kind: u32, k: u64) {
    let n = g.below(7) as usize;
    let mask = g.below(256) & !(4 | 128) | if g.below(2) == 0 { 4 } else if g.below(3) == 0 { 128 } else { 0 };
    let mut inp = vec![kind, n as u32, mask];
    let mut vss = VertexStreamStruct::default();
    if mask & 1 != 0 {
        vss.locations = Some(vec3s(g, n, &mut inp));
    }
    if mask & 2 != 0 {
        vss.normals = Some(vec3s(g, n, &mut inp));
    }
    if mask & (4 | 128) != 0 {
        let uv: Vec<Vector2> = (0..n)
            .map(|_| {
                let v = Vector2::new(g.mixed(), g.mixed());
                inp.extend([v.x.to_bits(), v.y.to_bits()]);
                v
            })
            .collect();
        if mask & 4 != 0 {
            vss.uv[0] = Some(uv);
        } else {
            vss.uv[1] = Some(uv);
        }
    }
    if mask & 8 != 0 {
        let d: Vec<u32> = (0..n).map(|_| g.u32()).collect();
        inp.extend(&d);
        vss.diffuse_int = Some(d);
    }
    if mask & 16 != 0 {
        vss.s = Some(vec3s(g, n, &mut inp));
    }
    if mask & 32 != 0 {
        vss.t = Some(vec3s(g, n, &mut inp));
    }
    if mask & 64 != 0 {
        vss.sxt = Some(vec3s(g, n, &mut inp));
    }
    let mut out = vec![0u32; 200];
    let got = unsafe { ref_dw7_shdcopy(inp.as_ptr(), inp.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(got >= 0);
    out.truncate(got as usize);
    let mut mine: Vec<u32> = Vec::new();
    match kind {
        0..=3 => {
            for v in copy_vertex_stream_xyzn_duv1(&vss, n) {
                mine.extend([v.x.to_bits(), v.y.to_bits(), v.z.to_bits(), v.nx.to_bits(), v.ny.to_bits(), v.nz.to_bits(), v.diffuse, v.u1.to_bits(), v.v1.to_bits()]);
            }
        }
        4..=7 => {
            for t in copy_vertex_stream_xyzn_duv1_tg3(&vss, n) {
                let v = t.base;
                mine.extend([v.x.to_bits(), v.y.to_bits(), v.z.to_bits(), v.nx.to_bits(), v.ny.to_bits(), v.nz.to_bits(), v.diffuse, v.u1.to_bits(), v.v1.to_bits()]);
                mine.extend([t.sx, t.sy, t.sz, t.tx, t.ty, t.tz, t.sxtx, t.sxty, t.sxtz].map(f32::to_bits));
            }
        }
        _ => {
            for v in copy_vertex_stream_cubemap(&vss, n) {
                mine.extend([v.x.to_bits(), v.y.to_bits(), v.z.to_bits(), v.nx.to_bits(), v.ny.to_bits(), v.nz.to_bits(), v.diffuse]);
            }
        }
    }
    c.eq(mine, out, || format!("case {k} kind {kind} n {n} mask {mask:#x}"));
}

#[test]
fn ww3d7_shdcopy_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_17_05);
    let names = ["Shd6SimpleClass", "Shd6GlossMaskClass", "Shd6BumpDiffClass", "Shd6BumpSpecClass", "Shd7BumpDiffClass", "Shd7BumpSpecClass", "Shd8BumpDiffClass", "Shd8BumpSpecClass", "Shd6CubeMapClass"];
    let mut chk: Vec<Check> = names.iter().map(|n| Check::new(&format!("{n}::Copy_Vertex_Stream"))).collect();
    for k in 0..cases() {
        for (kind, c) in chk.iter_mut().enumerate() {
            case(&mut g, c, kind as u32, k);
        }
    }
    finish(chk);
}
