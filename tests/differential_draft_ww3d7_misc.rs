// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d7) of the drafts `src/drafts/ww3d/{polyinfo, assetstatus, renderobjectrecycler}.rs` against
//! the upstream WW3D2 files of the same names, cut verbatim by build.rs `drafts_ww3d7` (differential/wrap_draft_ww3d7_misc.cpp and
//! wrap_draft_ww3d7_recycler.cpp list the cuts and the stand-ins).
//!
//! # Method
//! `DRAFT_CASES` (default 20,000) cases per check from a fixed seed; every output compared.
//! - polyinfo: 1..=30 operations (construct, `Set_Texture`, `Set_Vertex_Material`, `Set_Shader`, `Set`) with handles 0 (NULL) to 16 and random
//!   32-bit shader words; after each the three `Peek_*` answers.
//! - asset status: 0..=40 reports of the six kinds with names from a pool of case variants, the empty name and random strings, with the two
//!   enable flags toggled between them; the text the destructor writes (WWDEBUG), compared as: the header, the category lines in order, and per
//!   category the sorted set of report lines (upstream iterates a hash table, the draft a sorted map: only the order of lines inside a category differs).
//! - recycler: 1..=10 scripted models (twelve possible names in case variants, emitter and animation flags, sub-objects), 1..=40 operations
//!   (`Get_Render_Object`, `Return_Render_Object`, `Reset`); the event log (every `Set_Transform` with the matrix bits, emitter reset, animation
//!   restart, in order) and every result compared.
//!
//! # Excluded inputs
//! - polyinfo: a NULL shader (`Set_Shader(0)` dereferences it upstream; the draft's `None` clears it).
//! - Reference counts (`Add_Ref`, `Release_Ref`) are not modelled by the drafts and are no-ops in the wrappers.
//! - assetstatus: the file write itself (the draft returns the text; `RawFileClass` is a capture in the wrapper). Names are ASCII (`_strlwr` is
//!   locale-dependent above 127).
//! - recycler: a NULL sub-object (upstream would crash).
//!
//! # Predictions (2026-11-20, before the first run)
//! polyinfo 0 mismatches. asset status 0. Recycler: a MISMATCH, which I expect from reading `multilist.h` first: `RefMultiListClass::Add` links at the
//! HEAD of the list (`Internal_Add`), the draft pushes at the end, so with two pooled models of one name the draft hands out the other one; and
//! `Add` has `onlyonce = true`, so returning a model that is already pooled is refused upstream, where the draft pools it twice.
//!
//! # Results
//! 2026-11-20, `DRAFT_CASES` default (20,000), fixed seed; the numbers are mismatching cases per check.
//! - **Recycler: the prediction held.** First run: 5,469 of 20,000 cases mismatched (the draft pooled at the end and accepted a model twice;
//!   upstream's `RefRenderObjListClass::Add` links at the head and refuses a model already in the list, multilist.cpp:97). Fixed in
//!   `Insert_Inactive_Model`; after the fix **0 of 20,000** (every `Set_Transform` with the matrix bits, emitter reset, animation restart and result).
//! - polyinfo 0 of 20,000 (prediction held); asset status 0 of 20,000 (the report text; lines sorted inside a category: upstream iterates a hash table,
//!   the draft a sorted map); material pass accessors 0 of 20,000; render info defaults and `SpecialRenderInfoClass` 0 of 200 (prediction for the last
//!   two was 0, held).
//! - Not covered: `MaterialPassClass::Install_Materials` (device; an empty stand-in in the wrapper), `Set_Shader` (differential_draft_ww3d5_misc.rs).
//!   The three `Peek_*` accessors the draft shares with `Get_*` are compared through `Get_*` (the draft has one function for each pair).
//!
//! # Planted bugs (`L9_PLANT=<id>`; mismatching cases of 20,000 in the check the plant targets)
//! polyinfo: 19461 (`Set` forgets the shader) 20,000; 19462 (`Set` swaps texture and material) 19,814; 19463 (`Set_Texture` ignores NULL) 12,630; 19464
//! (`Set_Vertex_Material` ignores NULL) 12,716; 19465 (constructor drops the material) 18,916. Recycler: 19466 (case-sensitive name) 10,123; 19467 (last match, not
//! first) 4,515; 19468 (recycled model not placed) 14,434; 19469 (no recursion into sub-objects) 6,448; 19470 (emitter not reset) 8,592; 19471 (animation not
//! restarted) 11,957; 19472 (new model not placed) 19,149; 19473 (takes the last pooled model instead of the found one) 10,618. Asset status (of 40,000 compared
//! words): 19474 (name not lower-cased) 9,540; 19475 (load-on-demand ignores its flag) 6,010; 19476 (count not incremented) 5,075; 19477 (`reported 1 times`
//! printed) 9,929; 19478 (missing reports gated by the load-on-demand flag) 5,976; 19479 (report written with reporting off) 9,367; 19480 (missing HTree in the
//! HAnim table) 8,223. Material pass: 19501 (`Set_Texture` always stage 0) 17,810; 19502 (`Set_Cull_Volume` ignores NULL) 2,395; 19503 (translucent flag
//! inverted) 15,984; 19504 (`Get_Texture` stage clamp) 11,310. Render info (of 200): 19505 (emissive override default 0) 200; 19506 (render type ignored) 199;
//! 19507 (alpha override default 0) 200. **20 plants in the draft files of this test (polyinfo 5, recycler 8, asset status 7) plus 7 for matpass and
//! rinfo: 27 of 27 caught.**

mod support;

use rts_lockstep_engine::drafts::ww3d::assetstatus::AssetStatusClass;
use rts_lockstep_engine::drafts::ww3d::handles::RenderObjHandle;
use rts_lockstep_engine::drafts::ww3d::matpass::MaterialPassClass;
use rts_lockstep_engine::drafts::ww3d::rinfo::{
    RenderInfoClass, SpecialRenderInfoClass, MAX_ADDITIONAL_MATERIAL_PASSES, MAX_OVERRIDE_FLAG_LEVEL, RENDER_SHADOW, RENDER_VIS, RINFO_OVERRIDE_ADDITIONAL_PASSES_ONLY,
    RINFO_OVERRIDE_DEFAULT, RINFO_OVERRIDE_FORCE_SORTING, RINFO_OVERRIDE_FORCE_TWO_SIDED, RINFO_OVERRIDE_SHADOW_RENDERING,
};
use rts_lockstep_engine::drafts::ww3d::polyinfo::PolygonInfoClass;
use rts_lockstep_engine::drafts::ww3d::renderobjectrecycler::{RecyclerHost, RenderObjectRecyclerClass};
use rts_lockstep_engine::formats::w3d::shader::ShaderClass;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector4::Vector4;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dw7_misc(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
    fn ref_dw7_recycler(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 200_000];
    let n = unsafe { ref_dw7_misc(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0, "wrapper refused op {}", input[0]);
    out.truncate(n as usize);
    out
}

fn run_recycler(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 200_000];
    let n = unsafe { ref_dw7_recycler(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0, "recycler wrapper failed");
    out.truncate(n as usize);
    out
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

fn opt(h: u32) -> Option<usize> {
    if h == 0 {
        None
    } else {
        Some(h as usize)
    }
}

// ---------------- polyinfo ----------------
fn poly_case(g: &mut Gen, c: &mut Check, case: u64) {
    let mut ops: Vec<[u32; 4]> = Vec::new();
    let handle = |g: &mut Gen| if g.below(4) == 0 { 0 } else { 1 + g.below(16) };
    let bits = |g: &mut Gen| if g.below(3) == 0 { g.u32() } else { g.below(0x1000) };
    ops.push([0, handle(g), handle(g), bits(g)]);
    for _ in 0..g.below(30) {
        ops.push(match g.below(5) {
            0 => [0, handle(g), handle(g), bits(g)],
            1 => [1, handle(g), 0, 0],
            2 => [2, handle(g), 0, 0],
            3 => [3, bits(g), 0, 0],
            _ => [4, handle(g), handle(g), bits(g)],
        });
    }
    let mut inp = vec![0, ops.len() as u32];
    for o in &ops {
        inp.extend(o);
    }
    let cpp = run(&inp);
    let mut pi: Option<PolygonInfoClass> = None;
    let mut mine = Vec::new();
    for o in &ops {
        match o[0] {
            0 => pi = Some(PolygonInfoClass::new(opt(o[1]), opt(o[2]), Some(ShaderClass(o[3])))),
            1 => pi.as_mut().unwrap().set_texture(opt(o[1])),
            2 => pi.as_mut().unwrap().set_vertex_material(opt(o[1])),
            3 => pi.as_mut().unwrap().set_shader(Some(&ShaderClass(o[1]))),
            _ => pi.as_mut().unwrap().set(opt(o[1]), opt(o[2]), Some(ShaderClass(o[3]))),
        }
        let p = pi.as_ref().unwrap();
        mine.push(p.peek_texture().map_or(0, |h| h as u32));
        mine.push(p.peek_vertex_material().map_or(0, |h| h as u32));
        mine.push(p.peek_shader().map_or(0xDEAD_BEEF, |s| s.0));
    }
    c.eq(mine, cpp, || format!("case {case} ops {ops:?}"));
}

// ---------------- asset status ----------------
const NAME_POOL: [&str; 12] = ["", "Tank", "tank", "TANK", "a.w3d", "A.W3D", "very_long_asset_name_01", "VERY_LONG_ASSET_NAME_01", "x", "X", "Mixed.Case.Name", "mixed.case.name"];

fn asset_name(g: &mut Gen) -> String {
    match g.below(3) {
        0 | 1 => NAME_POOL[g.below(NAME_POOL.len() as u32) as usize].to_string(),
        _ => {
            let n = g.below(12);
            (0..n).map(|_| ['a', 'B', '.', '_', '1', 'Z'][g.below(6) as usize]).collect()
        }
    }
}

/// The header lines in order, the category lines in order, and per category the sorted report lines.
fn normalise(text: &str) -> (Vec<String>, Vec<(String, String)>, usize) {
    let mut structure = Vec::new();
    let mut lines = Vec::new();
    let mut cat = String::new();
    for l in text.split("\r\n") {
        if l.starts_with("Category: ") {
            cat = l.to_string();
            structure.push(l.to_string());
        } else if l.is_empty() || l == "Load-on-demand and missing assets report" {
            structure.push(l.to_string());
        } else {
            lines.push((cat.clone(), l.to_string()));
        }
    }
    lines.sort();
    (structure, lines, text.len())
}

fn asset_case(g: &mut Gen, c: &mut Check, case: u64) {
    let defaults = g.below(4) == 0;
    let flags = if defaults { 4 } else { g.below(4) };
    let mut inp = vec![1, flags];
    let nops = g.below(41);
    inp.push(nops);
    enum Op {
        Report(u32, String),
        Reporting(bool),
        Lod(bool),
    }
    let mut ops = Vec::new();
    for _ in 0..nops {
        let k = g.below(10);
        if k < 7 {
            let kind = g.below(6);
            let name = asset_name(g);
            inp.extend([kind, name.len() as u32]);
            inp.extend(name.bytes().map(u32::from));
            ops.push(Op::Report(kind, name));
        } else if k == 7 {
            let f = g.below(2);
            inp.extend([6, f]);
            ops.push(Op::Reporting(f != 0));
        } else {
            let f = g.below(2);
            inp.extend([7, f]);
            ops.push(Op::Lod(f != 0));
        }
    }
    let cpp = run(&inp);
    let cpp_text: String = cpp[1..].iter().map(|&w| w as u8 as char).collect();
    let mut st = AssetStatusClass::new();
    if !defaults {
        st.enable_reporting(flags & 1 != 0);
        st.enable_load_on_demand_reporting(flags & 2 != 0);
    }
    for o in &ops {
        match o {
            Op::Report(0, n) => st.report_load_on_demand_robj(n),
            Op::Report(1, n) => st.report_load_on_demand_hanim(n),
            Op::Report(2, n) => st.report_load_on_demand_htree(n),
            Op::Report(3, n) => st.report_missing_robj(n),
            Op::Report(4, n) => st.report_missing_hanim(n),
            Op::Report(_, n) => st.report_missing_htree(n),
            Op::Reporting(f) => st.enable_reporting(*f),
            Op::Lod(f) => st.enable_load_on_demand_reporting(*f),
        }
    }
    let mine = st.debug_report().unwrap_or_default();
    c.eq(cpp[0] as usize, cpp_text.len(), || "wrapper byte count".to_string());
    c.eq(normalise(&mine), normalise(&cpp_text), || format!("case {case} flags {flags} defaults {defaults} ops {:?}", inp));
}

// ---------------- recycler ----------------
const NAMES: [&str; 12] = ["Tank", "tank", "TANK", "Jeep", "jeep", "Bush", "xfail", "Xa", "", "tan", "tankk", "BUSH"];

struct Model {
    id: usize,
    name: usize,
    emitter: bool,
    anim: bool,
    subs: Vec<usize>,
}

struct World {
    models: Vec<Model>,
    log: Vec<u32>,
}

impl RecyclerHost for World {
    fn get_name(&self, robj: RenderObjHandle) -> String {
        NAMES[self.models[robj].name].to_string()
    }
    fn set_transform(&mut self, robj: RenderObjHandle, tm: &Matrix3D) {
        self.log.extend([1, self.models[robj].id as u32]);
        for r in &tm.row {
            self.log.extend([r.x.to_bits(), r.y.to_bits(), r.z.to_bits(), r.w.to_bits()]);
        }
    }
    fn create_render_obj(&mut self, name: &str) -> Option<RenderObjHandle> {
        if name.starts_with('x') || name.starts_with('X') {
            return None;
        }
        let id = self.models.len();
        let nm = NAMES.iter().position(|n| *n == name).unwrap();
        self.models.push(Model { id, name: nm, anim: name.len() % 2 == 0, emitter: name.len() % 3 == 0, subs: Vec::new() });
        Some(id)
    }
    fn get_num_sub_objects(&self, robj: RenderObjHandle) -> usize {
        self.models[robj].subs.len()
    }
    fn get_sub_object(&self, robj: RenderObjHandle, i: usize) -> Option<RenderObjHandle> {
        Some(self.models[robj].subs[i])
    }
    fn is_particle_emitter(&self, robj: RenderObjHandle) -> bool {
        self.models[robj].emitter
    }
    fn reset_emitter(&mut self, robj: RenderObjHandle) {
        self.log.extend([2, self.models[robj].id as u32]);
    }
    fn has_animation(&self, robj: RenderObjHandle) -> bool {
        self.models[robj].anim
    }
    fn restart_animation_once(&mut self, robj: RenderObjHandle) {
        self.log.extend([3, self.models[robj].id as u32]);
    }
}

fn recycler_case(g: &mut Gen, c: &mut Check, case: u64) {
    let nm = 1 + g.below(10);
    let mut inp = vec![nm];
    let mut world = World { models: Vec::new(), log: Vec::new() };
    for k in 0..nm {
        let name = g.below(NAMES.len() as u32);
        let (emitter, anim) = (g.below(3) == 0, g.below(3) == 0);
        let mut subs = Vec::new();
        if k + 1 < nm {
            for _ in 0..g.below(4) {
                subs.push(k as usize + 1 + g.below(nm - k - 1) as usize);
            }
        }
        inp.extend([name, u32::from(emitter), u32::from(anim), subs.len() as u32]);
        inp.extend(subs.iter().map(|&s| s as u32));
        world.models.push(Model { id: k as usize, name: name as usize, emitter, anim, subs });
    }
    let nops = 1 + g.below(40);
    inp.push(nops);
    let mut ops: Vec<(u32, u32, [u32; 12])> = Vec::new();
    for _ in 0..nops {
        let kind = match g.below(10) {
            0..=4 => 0,
            5..=8 => 1,
            _ => 2,
        };
        let a = if kind == 0 { g.below(NAMES.len() as u32) } else { g.below(64) };
        let mut tm = [0u32; 12];
        for w in tm.iter_mut() {
            *w = g.u32();
        }
        ops.push((kind, a, tm));
        inp.extend([kind, a]);
        inp.extend(tm);
    }
    let cpp = run_recycler(&inp);
    let mut rec = RenderObjectRecyclerClass::default();
    for (kind, a, tm) in &ops {
        match kind {
            0 => {
                let m = Matrix3D {
                    row: [
                        Vector4 { x: f32::from_bits(tm[0]), y: f32::from_bits(tm[1]), z: f32::from_bits(tm[2]), w: f32::from_bits(tm[3]) },
                        Vector4 { x: f32::from_bits(tm[4]), y: f32::from_bits(tm[5]), z: f32::from_bits(tm[6]), w: f32::from_bits(tm[7]) },
                        Vector4 { x: f32::from_bits(tm[8]), y: f32::from_bits(tm[9]), z: f32::from_bits(tm[10]), w: f32::from_bits(tm[11]) },
                    ],
                };
                let r = rec.get_render_object(&mut world, NAMES[*a as usize], &m);
                world.log.push(0xFFFF_FFFE);
                world.log.push(r.map_or(0, |h| world.models[h].id as u32 + 1));
            }
            1 => {
                let h = *a as usize % world.models.len();
                rec.return_render_object(h);
                world.log.push(0xFFFF_FFFE);
            }
            _ => {
                rec.reset();
                world.log.push(0xFFFF_FFFE);
            }
        }
    }
    c.eq(world.log, cpp, || format!("case {case} models {} ops {:?}", nm, ops.iter().map(|o| (o.0, o.1)).collect::<Vec<_>>()));
}

// ---------------- material pass accessors ----------------
fn matpass_case(g: &mut Gen, c: &mut Check, case: u64) {
    let nops = 1 + g.below(20);
    let mut inp = vec![2, nops];
    let mut ops: Vec<[u32; 3]> = Vec::new();
    for _ in 0..nops {
        let o = match g.below(5) {
            0 | 1 => [0, g.below(8), g.below(9)],
            2 => [1, g.below(9), 0],
            3 => [2, g.below(9), 0],
            _ => [3, g.below(2), 0],
        };
        inp.extend(o);
        ops.push(o);
    }
    let cpp = run(&inp);
    let mut mp = MaterialPassClass::new();
    let mut mine = Vec::new();
    for o in &ops {
        match o[0] {
            0 => mp.set_texture(opt(o[2]), o[1] as usize),
            1 => mp.set_material(opt(o[1])),
            2 => mp.set_cull_volume(opt(o[1])),
            _ => mp.enable_on_translucent_meshes(o[1] != 0),
        }
        for s in 0..8 {
            mine.push(mp.get_texture(s).map_or(0, |h| h as u32));
        }
        for s in 0..8 {
            mine.push(mp.get_texture(s).map_or(0, |h| h as u32));
        }
        mine.push(mp.get_material().map_or(0, |h| h as u32));
        mine.push(mp.get_material().map_or(0, |h| h as u32));
        mine.push(mp.get_cull_volume().map_or(0, |h| h as u32));
        mine.push(u32::from(mp.is_enabled_on_translucent_meshes()));
        mine.push(mp.peek_shader().0);
    }
    c.eq(mine, cpp, || format!("case {case} ops {ops:?}"));
}

// ---------------- render info defaults ----------------
fn rinfo_defaults(g: &mut Gen, c: &mut Check) {
    for k in 0..200 {
        let ty = if k < 4 { k as i32 } else { g.i32() };
        let cpp = run(&[3, ty as u32]);
        let dump = |r: &mut RenderInfoClass| -> Vec<u32> {
            vec![
                r.fog_scale.to_bits(),
                r.fog_start.to_bits(),
                r.fog_end.to_bits(),
                r.alpha_override.to_bits(),
                r.material_pass_alpha_override.to_bits(),
                r.material_pass_emissive_override.to_bits(),
                u32::from(r.light_environment.is_none()),
                u32::from(r.texture_projector.is_none()),
                r.additional_pass_count() as u32,
                *r.current_override_flags(),
                u32::from(r.camera == 7),
            ]
        };
        let mut mine = dump(&mut RenderInfoClass::new(7));
        let mut sp = SpecialRenderInfoClass::new(7, ty);
        mine.extend(dump(&mut sp.base));
        mine.extend([sp.render_type as u32, u32::from(sp.vis_rasterizer.is_none()), u32::from(sp.bw_renderer.is_none())]);
        mine.extend([RENDER_VIS as u32, RENDER_SHADOW as u32, MAX_ADDITIONAL_MATERIAL_PASSES as u32, MAX_OVERRIDE_FLAG_LEVEL as u32]);
        mine.extend([
            RINFO_OVERRIDE_DEFAULT,
            RINFO_OVERRIDE_FORCE_TWO_SIDED,
            RINFO_OVERRIDE_FORCE_SORTING,
            RINFO_OVERRIDE_ADDITIONAL_PASSES_ONLY,
            RINFO_OVERRIDE_SHADOW_RENDERING,
        ]);
        c.eq(mine, cpp, || format!("render type {ty}"));
    }
}

#[test]
fn ww3d7_misc_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_17_01);
    let mut c_poly = Check::new("PolygonInfoClass");
    let mut c_asset = Check::new("AssetStatusClass");
    let mut c_rec = Check::new("RenderObjectRecyclerClass");
    let mut c_mp = Check::new("MaterialPassClass accessors");
    let mut c_ri = Check::new("RenderInfoClass defaults, SpecialRenderInfoClass");
    rinfo_defaults(&mut g, &mut c_ri);
    for case in 0..cases() {
        matpass_case(&mut g, &mut c_mp, case);
        poly_case(&mut g, &mut c_poly, case);
        asset_case(&mut g, &mut c_asset, case);
        recycler_case(&mut g, &mut c_rec, case);
    }
    finish(vec![c_poly, c_asset, c_rec, c_mp, c_ri]);
}
