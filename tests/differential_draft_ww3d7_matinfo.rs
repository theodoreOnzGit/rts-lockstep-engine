// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d7) of the draft `src/drafts/ww3d/matinfo.rs` (MaterialInfoClass, MaterialRemapperClass,
//! MaterialCollectorClass) against upstream WW3D2 `matinfo.h` / `matinfo.cpp`, cut verbatim by build.rs `drafts_ww3d7`
//! (differential/wrap_draft_ww3d7_matinfo.cpp lists the cuts and the stand-ins).
//!
//! # Method
//! `DRAFT_CASES` (default 20,000) cases per check from a fixed seed; every output compared.
//! - info: 1..=30 operations (add vertex material or texture, the lookups by index and by name with case variants, `Replace_*`, `Reset`, `Clone`
//!   (which clones every vertex material: the clones get new handles), `Reset_Texture_Mappers`, `Make_Vertex_Materials_Unique`,
//!   `Has_Time_Variant_Texture_Mappers`); after each the result, then the counts and the handles in the info.
//! - remapper: two infos of 0..=8 vertex materials and textures (the destination holds the source's objects rotated by one, so a source handle can be
//!   a destination handle), a scripted mesh description (0..=3 passes, 0..=4 vertices and polygons, per-vertex
//!   and single materials, textures per stage), 1..=12 operations (`Remap_Texture`, `Remap_Vertex_Material`, `Remap_Mesh` with the destination's
//!   `Set_*` calls logged). Handles outside the remap set are included (upstream's `WWASSERT(0)` is compiled out and the answer is NULL).
//! - collector: 1..=14 operations (`Collect_Materials` of a scripted mesh, `Reset`, the three `Add_*`, the three `Find_*`), the counts after each and
//!   the lists at the end. Shaders come from a pool of four words including 0xFFFFFFFF, the initial `LastShader`.
//!
//! # Excluded inputs
//! - A NULL vertex material slot where upstream dereferences it (`Clone`, `Get_Vertex_Material_Index` and the by-name lookups, `Reset_Texture_Mappers`,
//!   `Make_Vertex_Materials_Unique`, `Has_Time_Variant_Texture_Mappers`): a crash upstream; the draft skips the slot.
//! - `Peek_Texture(const char *name)`: declared (matinfo.h:104) and defined nowhere upstream, so no call to it links; the draft's `peek_texture_by_name`
//!   has no counterpart to compare with (an upstream defect, posted on #212).
//! - `Add_Texture(NULL)` (upstream calls `Add_Ref` on it). Index arguments out of range (asserts compiled out; undefined).
//! - Reference counts are not modelled by the draft.
//!
//! # Predictions (2026-11-20, before the first run)
//! 0 mismatches. One doubt: `MaterialCollectorClass::Reset` does not clear the `Last*` caches, so an `Add_*` of the last-added object after a `Reset`
//! is ignored upstream; the draft's comment says it does the same.
//!
//! # Results
//! 2026-11-20, `DRAFT_CASES` default (20,000), fixed seed: **0 mismatches** in each of the three checks (info, remapper, collector); the prediction held,
//! including the doubt (`Reset` leaves the `Last*` caches alone in both). Upstream defect found: `MaterialInfoClass::Peek_Texture(const char *)` is declared
//! (matinfo.h:104) and defined nowhere, so a call does not link (the first build of this test failed to link on it; posted on #212).
//!
//! # Planted bugs (`L9_PLANT=<id>`; mismatching cases of 20,000)
//! 19521 (texture lookup by name case-sensitive) 969 (info); 19522 (`Clone` shares the vertex materials instead of cloning them) 3,883 (info); 19523
//! (`Add_Vertex_Material` returns the new count) 11,789 (info); 19524 (remap cache keyed on the destination) 5,273 (remapper; the first version of the test
//! gave destination handles disjoint from the source's and MISSED this plant, so the destination now holds the source's objects rotated by one); 19525 (only
//! texture stage 0 remapped) 10,491 (remapper); 19526 (collector `Reset` also clears the `Last*` caches) 1,879; 19527 (per-polygon shaders ignored) 6,288; 19528
//! (`LastShader` starts at 0, not 0xFFFFFFFF) 7,205; 19529 (`Replace_Texture` stores nothing) 4,020 (info); 19530 (`Has_Time_Variant` tests only the last
//! material) 252 (info). **10 of 10 caught.**

mod support;

use rts_lockstep_engine::drafts::ww3d::matinfo::{MaterialCollectorClass, MaterialInfoClass, MaterialRemapperClass, MeshMatDesc};
use rts_lockstep_engine::formats::w3d::shader::ShaderClass;
use std::collections::HashMap;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dw7_matinfo(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 400_000];
    let n = unsafe { ref_dw7_matinfo(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0, "wrapper refused op {}", input[0]);
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

const NAMES: [&str; 8] = ["Tex", "tex", "TEX", "Wood", "wood", "", "Stone", "stone2"];
const SHADERS: [u32; 4] = [0xFFFF_FFFF, 0, 0x1234_5678, 0xDEAD_BEEF];

fn opt(h: u32) -> Option<usize> {
    if h == 0 {
        None
    } else {
        Some(h as usize)
    }
}

fn nul(h: Option<usize>) -> u32 {
    h.map_or(0, |x| x as u32)
}

/// The material world of the wrapper: the name and the time-variant flag of each vertex material handle (clones: 100 on).
struct VmWorld {
    name: HashMap<usize, usize>,
    variant: HashMap<usize, bool>,
    next_clone: usize,
    events: Vec<u32>,
}

impl VmWorld {
    fn new(flags: u32) -> Self {
        let mut w = VmWorld { name: HashMap::new(), variant: HashMap::new(), next_clone: 100, events: Vec::new() };
        for k in 0..8 {
            w.name.insert(k + 1, k);
            w.variant.insert(k + 1, (flags >> k) & 1 != 0);
        }
        w
    }
    fn vname(&self, h: usize) -> String {
        NAMES[self.name[&h]].to_string()
    }
}

fn dump_info(mi: &MaterialInfoClass, out: &mut Vec<u32>) {
    out.push(mi.vertex_material_count() as u32);
    for k in 0..mi.vertex_material_count() {
        out.push(nul(mi.peek_vertex_material(k)));
    }
    out.push(mi.texture_count() as u32);
    for k in 0..mi.texture_count() {
        out.push(mi.peek_texture(k) as u32);
    }
}

fn info_case(g: &mut Gen, c: &mut Check, case: u64) {
    let flags = g.below(256);
    let mut inp = vec![0, flags, 0];
    let mut ops: Vec<(u32, u32, u32)> = Vec::new();
    // the simulated slots (0 = NULL) and texture count, to keep the inputs within what upstream defines
    let mut vm: Vec<u32> = Vec::new();
    let mut ntex = 0usize;
    for _ in 0..1 + g.below(30) {
        let has_null = vm.iter().any(|&x| x == 0);
        let kind = g.below(15);
        let (a, b) = match kind {
            0 => (g.below(9), 0),
            1 => (1 + g.below(8), 0),
            2 | 3 | 5 | 7 => (g.below(8), 0),
            4 => (g.below(vm.len().max(1) as u32), 0),
            6 => (g.below(ntex.max(1) as u32), 0),
            8 => (g.below(vm.len().max(1) as u32), g.below(9)),
            9 => (g.below(ntex.max(1) as u32), 1 + g.below(8)),
            _ => (0, 0),
        };
        let ok = match kind {
            7 => false, // `Peek_Texture(const char *)` is declared in matinfo.h:104 and defined nowhere: upstream cannot link a call to it
            2 | 5 | 11 | 12 | 13 | 14 => !has_null,
            4 | 8 => !vm.is_empty(),
            6 | 9 => ntex > 0,
            _ => true,
        };
        if !ok {
            continue;
        }
        match kind {
            0 => vm.push(a),
            1 => ntex += 1,
            8 => vm[a as usize] = b,
            10 => {
                vm.clear();
                ntex = 0;
            }
            _ => {}
        }
        // a clone gets new handles (100 on); the simulation only needs to know they are not NULL
        if kind == 11 {
            for x in vm.iter_mut() {
                *x = 1;
            }
        }
        ops.push((kind, a, b));
    }
    inp[2] = ops.len() as u32;
    for o in &ops {
        inp.extend([o.0, o.1, o.2]);
    }
    let cpp = run(&inp);
    let mut w = VmWorld::new(flags);
    let mut mi = MaterialInfoClass::new();
    let mut mine = Vec::new();
    for (kind, a, b) in &ops {
        match kind {
            0 => mine.push(mi.add_vertex_material(opt(*a)) as u32),
            1 => mine.push(mi.add_texture(*a as usize) as u32),
            2 => mine.push(mi.get_vertex_material_index(NAMES[*a as usize], |h| w.vname(h)) as u32),
            3 => mine.push(mi.get_texture_index(NAMES[*a as usize], |h| NAMES[h - 1].to_string()) as u32),
            4 => mine.push(nul(mi.peek_vertex_material(*a as usize))),
            5 => mine.push(nul(mi.peek_vertex_material_by_name(NAMES[*a as usize], |h| w.vname(h)))),
            6 => mine.push(mi.peek_texture(*a as usize) as u32),
            7 => mine.push(nul(mi.peek_texture_by_name(NAMES[*a as usize], |h| NAMES[h - 1].to_string()))),
            8 => mi.replace_material(*a as usize, opt(*b)),
            9 => mi.replace_texture(*a as usize, *b as usize),
            10 => mi.reset(),
            11 => {
                let mut next = w.next_clone;
                let mut newnames = Vec::new();
                let c2 = MaterialInfoClass::clone_from_src(&mi, |h| {
                    let id = next;
                    next += 1;
                    newnames.push((id, h));
                    id
                });
                for (id, h) in newnames {
                    let (n, v) = (w.name[&h], w.variant[&h]);
                    w.name.insert(id, n);
                    w.variant.insert(id, v);
                }
                w.next_clone = next;
                mi = c2;
            }
            12 => {
                w.events.clear();
                let mut ev = Vec::new();
                mi.reset_texture_mappers(|h| ev.extend([1u32, h as u32]));
                mine.extend(ev);
            }
            13 => {
                let mut ev = Vec::new();
                mi.make_vertex_materials_unique(|h| ev.extend([2u32, h as u32]));
                mine.extend(ev);
            }
            _ => mine.push(u32::from(mi.has_time_variant_texture_mappers(|h| w.variant[&h]))),
        }
        dump_info(&mi, &mut mine);
    }
    c.eq(mine, cpp, || format!("case {case} flags {flags:#x} ops {ops:?}"));
}

// ---------------- mesh descriptions ----------------
struct Pass {
    mat_array: Option<Vec<u32>>,
    single_mat: u32,
    shader_array: Option<Vec<u32>>,
    single_shader: u32,
    tex_array: [Option<Vec<u32>>; 2],
    single_tex: [u32; 2],
}

struct Mesh {
    vertex_count: usize,
    poly_count: usize,
    passes: Vec<Pass>,
    events: Vec<u32>,
}

impl MeshMatDesc for Mesh {
    fn get_pass_count(&self) -> usize {
        self.passes.len()
    }
    fn get_vertex_count(&self) -> usize {
        self.vertex_count
    }
    fn get_polygon_count(&self) -> usize {
        self.poly_count
    }
    fn has_material_array(&self, pass: usize) -> bool {
        self.passes[pass].mat_array.is_some()
    }
    fn peek_material(&self, v: usize, pass: usize) -> Option<usize> {
        opt(self.passes[pass].mat_array.as_ref().unwrap()[v])
    }
    fn peek_single_material(&self, pass: usize) -> Option<usize> {
        opt(self.passes[pass].single_mat)
    }
    fn has_shader_array(&self, pass: usize) -> bool {
        self.passes[pass].shader_array.is_some()
    }
    fn get_shader(&self, poly: usize, pass: usize) -> ShaderClass {
        ShaderClass(self.passes[pass].shader_array.as_ref().unwrap()[poly])
    }
    fn get_single_shader(&self, pass: usize) -> ShaderClass {
        ShaderClass(self.passes[pass].single_shader)
    }
    fn has_texture_array(&self, pass: usize, stage: usize) -> bool {
        self.passes[pass].tex_array[stage].is_some()
    }
    fn peek_texture(&self, poly: usize, pass: usize, stage: usize) -> Option<usize> {
        opt(self.passes[pass].tex_array[stage].as_ref().unwrap()[poly])
    }
    fn peek_single_texture(&self, pass: usize, stage: usize) -> Option<usize> {
        opt(self.passes[pass].single_tex[stage])
    }
    fn set_material(&mut self, v: usize, vmat: Option<usize>, pass: usize) {
        self.events.extend([10, v as u32, nul(vmat), pass as u32]);
    }
    fn set_single_material(&mut self, vmat: Option<usize>, pass: usize) {
        self.events.extend([11, nul(vmat), pass as u32]);
    }
    fn set_texture(&mut self, poly: usize, tex: Option<usize>, pass: usize, stage: usize) {
        self.events.extend([12, poly as u32, nul(tex), pass as u32, stage as u32]);
    }
    fn set_single_texture(&mut self, tex: Option<usize>, pass: usize, stage: usize) {
        self.events.extend([13, nul(tex), pass as u32, stage as u32]);
    }
}

fn gen_mesh(g: &mut Gen) -> (Mesh, Vec<u32>) {
    let np = g.below(4) as usize;
    let vc = g.below(5) as usize;
    let pc = g.below(5) as usize;
    let mut words = vec![np as u32, vc as u32, pc as u32];
    let mut passes = Vec::new();
    let handle = |g: &mut Gen| if g.below(5) == 0 { 0 } else { 1 + g.below(8) };
    for _ in 0..np {
        let mut p = Pass { mat_array: None, single_mat: 0, shader_array: None, single_shader: 0, tex_array: [None, None], single_tex: [0, 0] };
        if g.below(2) == 0 {
            p.mat_array = Some((0..vc).map(|_| handle(g)).collect());
        }
        p.single_mat = handle(g);
        if g.below(2) == 0 {
            p.shader_array = Some((0..pc).map(|_| SHADERS[g.below(4) as usize]).collect());
        }
        p.single_shader = SHADERS[g.below(4) as usize];
        for s in 0..2 {
            if g.below(2) == 0 {
                p.tex_array[s] = Some((0..pc).map(|_| handle(g)).collect());
            }
            p.single_tex[s] = handle(g);
        }
        words.push(u32::from(p.mat_array.is_some()));
        if let Some(a) = &p.mat_array {
            words.extend(a);
        }
        words.push(p.single_mat);
        words.push(u32::from(p.shader_array.is_some()));
        if let Some(a) = &p.shader_array {
            words.extend(a);
        }
        words.push(p.single_shader);
        for s in 0..2 {
            words.push(u32::from(p.tex_array[s].is_some()));
            if let Some(a) = &p.tex_array[s] {
                words.extend(a);
            }
            words.push(p.single_tex[s]);
        }
        passes.push(p);
    }
    (Mesh { vertex_count: vc, poly_count: pc, passes, events: Vec::new() }, words)
}

fn remapper_case(g: &mut Gen, c: &mut Check, case: u64) {
    let nv = g.below(9);
    let nt = g.below(9);
    let (mesh, words) = gen_mesh(g);
    let mut inp = vec![1, nv, nt];
    inp.extend(&words);
    let nops = 1 + g.below(12);
    inp.push(nops);
    let mut ops = Vec::new();
    for _ in 0..nops {
        let kind = g.below(3);
        let a = g.below(9);
        ops.push((kind, a));
        inp.extend([kind, a]);
    }
    let cpp = run(&inp);
    let mut src = MaterialInfoClass::new();
    let mut dest = MaterialInfoClass::new();
    for k in 0..nv as usize {
        src.add_vertex_material(Some(k + 1));
        dest.add_vertex_material(Some((k + 1) % nv as usize + 1));
    }
    for k in 0..nt as usize {
        src.add_texture(k + 1);
        dest.add_texture((k + 1) % nt as usize + 1);
    }
    let mut rm = MaterialRemapperClass::new(&src, &dest);
    let mut mine = Vec::new();
    for (kind, a) in &ops {
        match kind {
            0 => mine.push(nul(rm.remap_texture(opt(*a)))),
            1 => mine.push(nul(rm.remap_vertex_material(opt(*a)))),
            _ => {
                let mut dd = Mesh { vertex_count: 0, poly_count: 0, passes: Vec::new(), events: Vec::new() };
                rm.remap_mesh(&mesh, &mut dd);
                mine.push(dd.events.len() as u32);
                mine.extend(dd.events);
            }
        }
    }
    c.eq(mine, cpp, || format!("case {case} nv {nv} nt {nt} ops {ops:?}"));
}

fn collector_case(g: &mut Gen, c: &mut Check, case: u64) {
    let mut inp = vec![2, 0];
    let nops = 1 + g.below(14);
    let mut ops: Vec<(u32, u32)> = Vec::new();
    let mut meshes: Vec<Mesh> = Vec::new();
    for _ in 0..nops {
        let kind = g.below(8);
        inp.push(kind);
        match kind {
            0 => {
                let (m, w) = gen_mesh(g);
                inp.extend(w);
                meshes.push(m);
                ops.push((kind, (meshes.len() - 1) as u32));
            }
            1 => ops.push((kind, 0)),
            3 | 5 => {
                let a = SHADERS[g.below(4) as usize];
                inp.push(a);
                ops.push((kind, a));
            }
            _ => {
                let a = if kind == 2 || kind == 6 { g.below(9) } else { g.below(9) };
                inp.push(a);
                ops.push((kind, a));
            }
        }
    }
    inp[1] = nops;
    let cpp = run(&inp);
    let mut mc = MaterialCollectorClass::new();
    let mut mine = Vec::new();
    let counts = |mc: &MaterialCollectorClass, out: &mut Vec<u32>| {
        out.extend([mc.get_shader_count() as u32, mc.get_vertex_material_count() as u32, mc.get_texture_count() as u32]);
    };
    for (kind, a) in &ops {
        match kind {
            0 => mc.collect_materials(&meshes[*a as usize]),
            1 => mc.reset(),
            2 => mc.add_texture(opt(*a)),
            3 => mc.add_shader(ShaderClass(*a)),
            4 => mc.add_vertex_material(opt(*a)),
            5 => mine.push(mc.find_shader(&ShaderClass(*a)) as u32),
            6 => mine.push(mc.find_texture(*a as usize) as u32),
            _ => mine.push(mc.find_vertex_material(*a as usize) as u32),
        }
        counts(&mc, &mut mine);
    }
    for k in 0..mc.get_shader_count() {
        mine.push(mc.peek_shader(k).0);
    }
    for k in 0..mc.get_vertex_material_count() {
        mine.push(mc.peek_vertex_material(k) as u32);
    }
    for k in 0..mc.get_texture_count() {
        mine.push(mc.peek_texture(k) as u32);
    }
    c.eq(mine, cpp, || format!("case {case} ops {ops:?}"));
}

#[test]
fn ww3d7_matinfo_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_17_03);
    let mut c_info = Check::new("MaterialInfoClass");
    let mut c_rm = Check::new("MaterialRemapperClass");
    let mut c_mc = Check::new("MaterialCollectorClass");
    for case in 0..cases() {
        info_case(&mut g, &mut c_info, case);
        remapper_case(&mut g, &mut c_rm, case);
        collector_case(&mut g, &mut c_mc, case);
    }
    finish(vec![c_info, c_rm, c_mc]);
}
