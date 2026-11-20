// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d7) of the wwshade drafts `src/drafts/ww3d/{shddef, shddeffactory, shddefmanager, shdinterface,
//! shdlib, shdloader, shdforcelinks}.rs` and the class ids, `MAX_TEXTURE_STAGES` and shader-version selection of `shdchunk.rs`, against the
//! upstream wwshade files of the same names, cut verbatim by build.rs `drafts_ww3d7` (differential/wrap_draft_ww3d7_shd.cpp lists the cuts and
//! the stand-ins).
//!
//! # Method
//! `DRAFT_CASES` (default 20,000) cases per check from a fixed seed; every word compared.
//! - definition: a `ShdDefClass` and 1..=8 operations (`Set_Name`, `Reset`, `Set_Surface_Type`, `Load` of a generated chunk stream: the
//!   `CHUNKID_VARIABLES` chunk or another id, 0..=5 micro-chunks: names with a NUL and bytes after it, surface types of 4 bytes or of 0..=8, unknown
//!   ids of 0..=10 bytes); after each the class id, surface type and name, and for a `Load` its result.
//! - manager: 1..=30 operations on eight factories (register, unregister, find by id, find by name with case variants, enumerate, `Get_Next(NULL)`,
//!   `Create_ShdDefClass_Instance`), with unique class ids, or with repeated ids (no unregister).
//! - `Load_Shader` over a class-id chunk (4 to 8 bytes) and a definition chunk, eight factories registered.
//! - interface, `ENUM_PARAM` list, class ids, `REGISTER_SHDDEF` / `SHD_Force_Links`, `shdlib.cpp`'s six functions, the two loaders.
//!
//! # Excluded inputs
//! - A name micro-chunk of length 0, or with no NUL: `READ_MICRO_CHUNK_WWSTRING` reads into `StringClass::Get_Buffer(len)`, which keeps the old
//!   buffer when `len` fits and otherwise allocates uninitialised memory, so the C string depends on StringClass internals (wwstring.cpp is not
//!   compiled: it casts pointers to 32-bit). With a NUL inside the chunk both agree. Name bytes >= 0x80 (the draft's `String` is UTF-8).
//! - `Unregister_Factory` of a factory that is not registered, or registered twice (upstream's asserts are compiled out and the list is corrupted);
//!   `unregister_factory(class_id)` with two registered factories of one id (the draft identifies a factory by its id, documented).
//! - `Load_Shader` with an unregistered class id (upstream calls through a NULL), a class-id chunk shorter than 4 bytes (reads an uninitialised `id`).
//! - `ShdDefClass::Save`, `Save_Shader` (`ChunkSaveClass` is not in the verified port; no draft).
//!
//! # Predictions (2026-11-20, before the first run)
//! Two mismatches from reading the sources: (1) `Load_Variables` reads `VARID_SURFACETYPE` straight into `SurfaceType`, so a micro-chunk shorter
//! than 4 bytes leaves the old value; the draft reads into a zeroed buffer and stores 0; (2) `MAX_TEXTURE_STAGES` is 8 in dx8wrapper.h:76, the draft says 4.
//! The manager's insertion at the head, the enum list and the class ids: 0.
//!
//! # Results
//! 2026-11-20, `DRAFT_CASES` default (20,000), fixed seed. **Both predictions held, and both defects are fixed:**
//! - `ShdDefClass`: 1,605 of 20,000 programs mismatched (and 447 of 20,000 in `Load_Shader`): a surface-type micro-chunk shorter than 4 bytes set the surface
//!   type to 0 in the draft (read into a zeroed buffer); upstream's `READ_MICRO_CHUNK` reads into the member, so the old value stays. Fixed.
//! - `ShdInterfaceClass` (the `MAX_TEXTURE_STAGES` word): 20,000 of 20,000: the draft said 4, dx8wrapper.h:76 says 8. Fixed (and `shdsubmesh.rs`, which
//!   initialised the `uv` array with four `None`s, now uses `Default`).
//! After the fixes: **0 mismatches** in definition (20,000), manager + factory (20,000), `Load_Shader` (20,000), interface (20,000), loaders (20,000),
//! `READ_MICRO_CHUNK` of Vector3, Vector2 and int (20,000; the draft's `read_micro_vector2/3` were changed to read INTO the caller's variable, as the
//! macro does: a short chunk left the draft's fresh vector at zero), the version choice of both bump shaders (20,000), the `ENUM_PARAM` list (32 names and
//! values), the class ids (10), the six `REGISTER_SHDDEF` lines and `SHD_Force_Links`, `shdlib.cpp`'s six functions. The last four are one-case table
//! comparisons; for `shdlib` the Rust side is the draft's constant and enum, so it compares the registration order and the call each function makes.
//! Upstream defects (posted on #212): `ShdDefManagerClass::Get_First(uint32)` and `Get_Next(current, uint32)` are declared in shddefmanager.h and defined
//! nowhere; `Unregister_Factory` of a factory that is not registered, and `Load_Shader` of an unregistered class id, are undefined behaviour.
//!
//! # Planted bugs (`L9_PLANT=<id>`; mismatching programs of 20,000 in the check the plant targets)
//! 19481 (`Reset` also clears the name) 5,758 (definition); 19482 (`Load` always true) 4,873 (definition); 19483 (surface type big-endian) 13,998 (definition)
//! and 10,921 (`Load_Shader`); 19484 (factory appended, not linked at the head) 8,538 (manager); 19485 (find by name case-sensitive) 4,613; 19486
//! (`Get_Next(NULL)` returns nothing) 11,365; 19487 (`Unregister` removes nothing) 2,310; 19488 (`Load_Shader` skips the definition's `Load`) 15,619
//! (`Load_Shader`); 19489 (`Greater_Than` false) 20,000 (interface); 19490 (`Pass_Selection` false) 20,000. **10 of 10 caught.**

mod support;

use rts_lockstep_engine::drafts::ww3d::shdchunk as sc;
use rts_lockstep_engine::drafts::ww3d::shdbumpdiff::ShdBumpDiffDefClass;
use rts_lockstep_engine::drafts::ww3d::shdbumpspec::ShdBumpSpecDefClass;
use rts_lockstep_engine::drafts::ww3d::shddef::{ShdDefClass, ShdVersion, CHUNKID_VARIABLES, SHDVER, SURFACE_TYPE_ENUM_PARAM};
use rts_lockstep_engine::drafts::ww3d::shddeffactory::ShdDefFactoryClass;
use rts_lockstep_engine::drafts::ww3d::shddefmanager::{ShdDefLoad, ShdDefManagerClass};
use rts_lockstep_engine::drafts::ww3d::shdforcelinks::FORCED_SHADER_DEFS;
use rts_lockstep_engine::drafts::ww3d::shdinterface::ShdInterfaceClass;
use rts_lockstep_engine::drafts::ww3d::shdlib::{ShdLibCall, SHD_REGISTER_LOADER_ORDER};
use rts_lockstep_engine::drafts::ww3d::shdloader::{shd_mesh_legacy_load_w3d, shd_mesh_load_w3d};
use rts_lockstep_engine::formats::w3d::w3d_file::{W3D_CHUNK_SHDSUBMESH_SHADER_CLASSID, W3D_CHUNK_SHDSUBMESH_SHADER_DEF};
use rts_lockstep_engine::formats::w3d::W3dResult;
use rts_lockstep_engine::ww::chunkio::{ChunkLoad, GameFile};
use rts_lockstep_engine::ww::vector2::Vector2;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use support::{Check, Gen};

extern "C" {
    fn ref_dw7_shd(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 100_000];
    let n = unsafe { ref_dw7_shd(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
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

fn chunk(id: u32, body: &[u8]) -> Vec<u8> {
    let mut v = id.to_le_bytes().to_vec();
    v.extend((body.len() as u32).to_le_bytes());
    v.extend(body);
    v
}

fn micro(id: u8, body: &[u8]) -> Vec<u8> {
    let mut v = vec![id, body.len() as u8];
    v.extend(body);
    v
}

/// The body of a definition chunk: 0..=5 micro-chunks.
fn def_body(g: &mut Gen) -> Vec<u8> {
    let mut body = Vec::new();
    for _ in 0..g.below(6) {
        match g.below(10) {
            0..=3 => {
                let len = 1 + g.below(24) as usize;
                let nul = g.below(len as u32) as usize;
                let mut b: Vec<u8> = (0..len).map(|_| 33 + g.below(94) as u8).collect();
                b[nul] = 0;
                body.extend(micro(0, &b));
            }
            4..=7 => {
                let len = if g.below(10) < 8 { 4 } else { g.below(9) as usize };
                let b: Vec<u8> = if len == 4 && g.below(3) == 0 { (g.u32() % 40).to_le_bytes().to_vec() } else { (0..len).map(|_| g.u32() as u8).collect() };
                body.extend(micro(1, &b));
            }
            _ => {
                let len = g.below(11) as usize;
                let b: Vec<u8> = (0..len).map(|_| g.u32() as u8).collect();
                body.extend(micro(2 + g.below(254) as u8, &b));
            }
        }
    }
    body
}

fn def_chunk(g: &mut Gen) -> Vec<u8> {
    let id = if g.below(8) == 0 { g.u32() } else { CHUNKID_VARIABLES };
    chunk(id, &def_body(g))
}

fn words(b: &[u8]) -> Vec<u32> {
    b.iter().map(|&x| u32::from(x)).collect()
}

fn state(d: &ShdDefClass) -> Vec<u32> {
    let mut v = vec![d.get_class_id(), d.get_surface_type() as u32, d.get_name().len() as u32];
    v.extend(d.get_name().bytes().map(u32::from));
    v
}

fn name_of(g: &mut Gen) -> String {
    (0..g.below(13)).map(|_| (33 + g.below(94) as u8) as char).collect()
}

// ---------------- definition ----------------
fn def_case(g: &mut Gen, c: &mut Check, case: u64) {
    let id = g.below(10);
    let mut inp = vec![0, id];
    let nops = 1 + g.below(8);
    inp.push(nops);
    enum Op {
        Name(String),
        Reset,
        Surface(i32),
        Load(Vec<u8>),
    }
    let mut ops = Vec::new();
    for _ in 0..nops {
        match g.below(8) {
            0 | 1 => {
                let n = name_of(g);
                inp.extend([0, n.len() as u32]);
                inp.extend(n.bytes().map(u32::from));
                ops.push(Op::Name(n));
            }
            2 => {
                inp.push(1);
                ops.push(Op::Reset);
            }
            3 => {
                let v = g.i32();
                inp.extend([2, v as u32]);
                ops.push(Op::Surface(v));
            }
            _ => {
                let b = def_chunk(g);
                inp.extend([3, b.len() as u32]);
                inp.extend(words(&b));
                ops.push(Op::Load(b));
            }
        }
    }
    let cpp = run(&inp);
    let mut d = ShdDefClass::new(id);
    let mut mine = Vec::new();
    for o in &ops {
        match o {
            Op::Name(n) => d.set_name(n),
            Op::Reset => d.reset(),
            Op::Surface(v) => d.set_surface_type(*v),
            Op::Load(b) => {
                let mut cl = ChunkLoad::new(GameFile::new(b).unwrap());
                mine.push(u32::from(d.load(&mut cl).unwrap()));
            }
        }
        mine.extend(state(&d));
    }
    c.eq(mine, cpp, || format!("case {case} id {id} inp {inp:?}"));
}

// ---------------- manager ----------------
const IDS_A: [u32; 8] = [1, 2, 3, 4, 5, 6, 7, 9];
const IDS_B: [u32; 8] = [1, 1, 2, 2, 3, 3, 4, 4];
const QN: [&str; 12] = ["Simple", "simple", "Bump", "BUMP", "Cube", "cube", "Gloss", "Legacy", "SIMPLE", "bump", "CUBE", "legacy"];

struct MockDef {
    base: ShdDefClass,
    idx: usize,
}

impl ShdDefLoad for MockDef {
    fn base(&self) -> &ShdDefClass {
        &self.base
    }
    fn load(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<bool> {
        self.base.load(cload)
    }
}

fn make_a<const I: usize>() -> MockDef {
    let mut base = ShdDefClass::new(IDS_A[I]);
    base.set_name(&format!("F{I}"));
    MockDef { base, idx: I }
}

fn make_b<const I: usize>() -> MockDef {
    let mut base = ShdDefClass::new(IDS_B[I]);
    base.set_name(&format!("F{I}"));
    MockDef { base, idx: I }
}

fn base_of(d: &MockDef) -> &ShdDefClass {
    &d.base
}

fn factory(i: usize, table_b: bool) -> ShdDefFactoryClass<MockDef> {
    let ids = if table_b { IDS_B } else { IDS_A };
    let names = ["Simple", "simple", "Bump", "BUMP", "Cube", "cube", "Gloss", "Legacy"];
    let create: fn() -> MockDef = match (table_b, i) {
        (false, 0) => make_a::<0>,
        (false, 1) => make_a::<1>,
        (false, 2) => make_a::<2>,
        (false, 3) => make_a::<3>,
        (false, 4) => make_a::<4>,
        (false, 5) => make_a::<5>,
        (false, 6) => make_a::<6>,
        (false, _) => make_a::<7>,
        (true, 0) => make_b::<0>,
        (true, 1) => make_b::<1>,
        (true, 2) => make_b::<2>,
        (true, 3) => make_b::<3>,
        (true, 4) => make_b::<4>,
        (true, 5) => make_b::<5>,
        (true, 6) => make_b::<6>,
        (true, _) => make_b::<7>,
    };
    ShdDefFactoryClass::new(ids[i], names[i], create, base_of)
}

fn manager_case(g: &mut Gen, c: &mut Check, case: u64) {
    let table_b = g.below(3) == 0;
    let mut inp = vec![1, u32::from(table_b)];
    let nops = 1 + g.below(30);
    inp.push(nops);
    let mut reg = [false; 8];
    let mut ops: Vec<(u32, u32)> = Vec::new();
    for _ in 0..nops {
        let kind = g.below(7);
        let a = match kind {
            0 | 1 => g.below(8),
            2 | 6 => g.below(11),
            3 => g.below(12),
            _ => 0,
        };
        if kind == 0 {
            if reg[a as usize] {
                continue;
            }
            reg[a as usize] = true;
        }
        if kind == 1 {
            if table_b || !reg[a as usize] {
                continue;
            }
            reg[a as usize] = false;
        }
        ops.push((kind, a));
        inp.extend([kind, a]);
    }
    inp[2] = ops.len() as u32;
    let cpp = run(&inp);
    let mut m: ShdDefManagerClass<MockDef> = ShdDefManagerClass::new();
    let mut mine = Vec::new();
    let ids = if table_b { IDS_B } else { IDS_A };
    for (kind, a) in &ops {
        match kind {
            0 => m.register_factory(factory(*a as usize, table_b)),
            1 => m.unregister_factory(ids[*a as usize]),
            2 => mine.push(m.find_factory(*a).map_or(0, |f| f.create().idx as u32 + 1)),
            3 => mine.push(m.find_factory_by_name(QN[*a as usize]).map_or(0, |f| f.create().idx as u32 + 1)),
            4 => {
                let mut seq = Vec::new();
                let mut cur = m.get_first();
                let mut pos = 0usize;
                while let Some(f) = cur {
                    if seq.len() >= 20 {
                        break;
                    }
                    seq.push(f.create().idx as u32);
                    cur = m.get_next(Some(pos));
                    pos += 1;
                }
                mine.push(seq.len() as u32);
                mine.extend(seq);
            }
            5 => mine.push(m.get_next(None).map_or(0, |f| f.create().idx as u32 + 1)),
            _ => match m.create_shd_def_class_instance(*a) {
                Some(d) => mine.extend([d.base.get_class_id(), d.idx as u32]),
                None => mine.push(0xFFFF_FFFF),
            },
        }
    }
    c.eq(mine, cpp, || format!("case {case} table_b {table_b} ops {ops:?}"));
}

fn load_shader_case(g: &mut Gen, c: &mut Check, case: u64) {
    let id = IDS_A[g.below(8) as usize];
    let mut idbody = id.to_le_bytes().to_vec();
    for _ in 0..g.below(5) {
        idbody.push(g.u32() as u8);
    }
    let mut blob = chunk(W3D_CHUNK_SHDSUBMESH_SHADER_CLASSID, &idbody);
    blob.extend(chunk(W3D_CHUNK_SHDSUBMESH_SHADER_DEF, &def_chunk(g)));
    let mut inp = vec![2, blob.len() as u32];
    inp.extend(words(&blob));
    let cpp = run(&inp);
    let mut m: ShdDefManagerClass<MockDef> = ShdDefManagerClass::new();
    for i in 0..8 {
        m.register_factory(factory(i, false));
    }
    let mut cl = ChunkLoad::new(GameFile::new(&blob).unwrap());
    let d = m.load_shader(&mut cl).unwrap().expect("a registered id");
    c.eq(state(&d.base), cpp, || format!("case {case} blob {blob:?}"));
}

// ---------------- interface and constants ----------------
fn interface_case(g: &mut Gen, c: &mut Check, case: u64) {
    let has = g.below(2);
    let did = g.below(20);
    let cid = g.i32();
    let cpp = run(&[3, has, did, cid as u32]);
    let def = if has != 0 { Some(Arc::new(ShdDefClass::new(did))) } else { None };
    let t = ShdInterfaceClass::new(def, cid);
    let mine = vec![
        t.peek_definition().map_or(0, |d| d.get_class_id() + 1),
        t.get_class_id() as u32,
        u32::from(t.greater_than(&t, 0)),
        u32::from(t.similar_enough(&t, 0)),
        u32::from(t.pass_selection(0)),
        ShdInterfaceClass::SHD_MAX_PASSES as u32,
        sc::MAX_TEXTURE_STAGES as u32,
    ];
    c.eq(mine, cpp, || format!("case {case}"));
}

fn constants(chk: &mut Vec<Check>) {
    // ENUM_PARAM list
    let mut c = Check::new("ShdDefClass ENUM_PARAM list");
    let cpp = run(&[4]);
    let mut mine = vec![SURFACE_TYPE_ENUM_PARAM.len() as u32];
    for (n, v) in SURFACE_TYPE_ENUM_PARAM {
        mine.push(n.len() as u32);
        mine.extend(n.bytes().map(u32::from));
        mine.push(v);
    }
    c.eq(mine, cpp, || "enum list".to_string());
    chk.push(c);
    // class ids
    let mut c = Check::new("shdclassids.h");
    let cpp = run(&[5]);
    let mine = vec![
        sc::SHDDEF_CLASSID_DUMMY,
        sc::SHDDEF_CLASSID_SIMPLE,
        sc::SHDDEF_CLASSID_GLOSSMASK,
        sc::SHDDEF_CLASSID_BUMPSPEC,
        sc::SHDDEF_CLASSID_BUMPDIFF,
        sc::SHDDEF_CLASSID_CUBEMAP,
        sc::SHDDEF_CLASSID_LEGACYW3D,
        sc::SHDDEF_CLASSID_LAST,
        sc::SHD_CLASSID_DUMMY,
        sc::SHD_CLASSID_LAST,
    ];
    c.eq(mine, cpp, || "class ids".to_string());
    chk.push(c);
    // REGISTER_SHDDEF and SHD_Force_Links
    let mut c = Check::new("REGISTER_SHDDEF, SHD_Force_Links");
    let cpp = run(&[6]);
    let mut mine = vec![FORCED_SHADER_DEFS.len() as u32];
    for (_, id, name) in FORCED_SHADER_DEFS {
        mine.push(id);
        mine.push(name.len() as u32);
        mine.extend(name.bytes().map(u32::from));
    }
    mine.push(FORCED_SHADER_DEFS.len() as u32);
    for (m, _, _) in FORCED_SHADER_DEFS {
        mine.push(m.len() as u32);
        mine.extend(m.bytes().map(u32::from));
    }
    c.eq(mine, cpp, || "registered definitions".to_string());
    chk.push(c);
    // shdlib
    let mut c = Check::new("shdlib.cpp");
    let cpp = run(&[7]);
    let code = |x: &ShdLibCall| match x {
        ShdLibCall::RendererInit => 1,
        ShdLibCall::RendererShutdown => 2,
        ShdLibCall::InitShaders => 3,
        ShdLibCall::ShutdownShaders => 4,
        ShdLibCall::RendererFlush => 5,
    };
    let mut mine = Vec::new();
    for call in [ShdLibCall::RendererInit, ShdLibCall::RendererShutdown, ShdLibCall::InitShaders, ShdLibCall::ShutdownShaders, ShdLibCall::RendererFlush] {
        mine.extend([1, code(&call)]);
    }
    mine.push(SHD_REGISTER_LOADER_ORDER.len() as u32);
    for n in SHD_REGISTER_LOADER_ORDER {
        mine.push(if n == "_ShdMeshLoader" { 10 } else if n == "_MeshLoader" { 11 } else { 99 });
    }
    c.eq(mine, cpp, || "shdlib".to_string());
    chk.push(c);
}

fn micro_case(g: &mut Gen, c: &mut Check, case: u64) {
    let mut body = Vec::new();
    for _ in 0..g.below(6) {
        let id = 1 + g.below(3) as u8;
        let right = match id {
            1 => 12,
            2 => 8,
            _ => 4,
        };
        let len = if g.below(10) < 7 { right } else { g.below(17) as usize };
        let b: Vec<u8> = (0..len).map(|_| g.u32() as u8).collect();
        body.extend(micro(id, &b));
    }
    let blob = chunk(g.u32(), &body);
    let mut inp = vec![9, blob.len() as u32];
    inp.extend(words(&blob));
    let cpp = run(&inp);
    let mut v = Vector3::new(7.0, 8.0, 9.0);
    let mut w = Vector2::new(5.0, 6.0);
    let mut surf = 77i32;
    let mut cl = ChunkLoad::new(GameFile::new(&blob).unwrap());
    cl.open_chunk().unwrap();
    while cl.open_micro_chunk().unwrap() {
        match cl.cur_micro_chunk_id() {
            1 => sc::read_micro_vector3(&mut cl, &mut v).unwrap(),
            2 => sc::read_micro_vector2(&mut cl, &mut w).unwrap(),
            3 => {
                let mut b = surf.to_le_bytes();
                cl.read_into(&mut b).unwrap();
                surf = i32::from_le_bytes(b);
            }
            _ => {}
        }
        cl.close_micro_chunk().unwrap();
    }
    let mine = vec![v.x.to_bits(), v.y.to_bits(), v.z.to_bits(), w.x.to_bits(), w.y.to_bits(), surf as u32];
    c.eq(mine, cpp, || format!("case {case} blob {blob:?}"));
}

fn version_case(g: &mut Gen, c: &mut Check, case: u64) {
    let (major, minor) = if g.below(4) == 0 { (g.below(300), g.below(300)) } else { (g.below(4), g.below(4)) };
    let dot3 = g.below(2);
    let cpp = run(&[10, major, minor, dot3]);
    let mut vd = ShdVersion::new();
    let mut vs = ShdVersion::new();
    let rd = ShdBumpDiffDefClass::init(&mut vd, major, minor, dot3 != 0);
    let rs = ShdBumpSpecDefClass::init(&mut vs, major, minor, dot3 != 0);
    let code = |v: SHDVER| v as u32;
    // the C++'s per-version Init each called: 8 -> 8, 7 -> 7, 6 -> 6 (a specular one is +10)
    let num = |v: SHDVER| match v {
        SHDVER::Ver8 => 8,
        SHDVER::Ver7 => 7,
        _ => 6,
    };
    let mine = vec![code(vd.get()), code(vs.get()), num(rd), 10 + num(rs)];
    c.eq(mine, cpp, || format!("case {case} ps {major}.{minor} dot3 {dot3}"));
}

fn loader_case(g: &mut Gen, c: &mut Check, case: u64) {
    let kind = g.below(2);
    let fail = g.below(2);
    let cpp = run(&[8, kind, fail]);
    let log = RefCell::new(Vec::new());
    let load = || -> Result<u32, ()> {
        log.borrow_mut().push(2u32);
        if fail != 0 {
            Err(())
        } else {
            Ok(7)
        }
    };
    let made = if kind == 0 {
        shd_mesh_load_w3d(load).is_some()
    } else {
        shd_mesh_legacy_load_w3d(load, |_m: &u32| {
            log.borrow_mut().push(6u32);
            9u32
        })
        .is_some()
    };
    let mut mine = vec![u32::from(made)];
    let l = log.borrow().clone();
    for e in l {
        mine.push(e);
        if e == 2 && made && kind == 1 {
            // the convert call comes after the load; the prototype after it
        }
    }
    if made {
        mine.push(4);
    }
    c.eq(mine, cpp, || format!("case {case} kind {kind} fail {fail}"));
}

#[test]
fn ww3d7_shd_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_17_02);
    let mut c_def = Check::new("ShdDefClass");
    let mut c_man = Check::new("ShdDefManagerClass + ShdDefFactoryClass");
    let mut c_ls = Check::new("ShdDefManagerClass::Load_Shader");
    let mut c_if = Check::new("ShdInterfaceClass");
    let mut c_ld = Check::new("shdloader.cpp");
    let mut c_mc = Check::new("READ_MICRO_CHUNK of a Vector3, Vector2, int");
    let mut c_ver = Check::new("shader version choice (ShdBump*DefClass::Init)");
    for case in 0..cases() {
        def_case(&mut g, &mut c_def, case);
        manager_case(&mut g, &mut c_man, case);
        load_shader_case(&mut g, &mut c_ls, case);
        interface_case(&mut g, &mut c_if, case);
        loader_case(&mut g, &mut c_ld, case);
        micro_case(&mut g, &mut c_mc, case);
        version_case(&mut g, &mut c_ver, case);
    }
    let mut chk = vec![c_def, c_man, c_ls, c_if, c_ld, c_mc, c_ver];
    constants(&mut chk);
    finish(chk);
}
