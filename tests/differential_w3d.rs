// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `ww::chunkio` and `formats::w3d` against the
//! upstream C++ (commit 0a05454d): `WWLib/chunkio.cpp`, the real
//! `WW3D2/w3d_file.h`, and the loaders of `htree.cpp`, `motchan.cpp`,
//! `hrawanim.cpp` and `hcanim.cpp` (see `differential/wrap_w3d.cpp` and
//! `build.rs`; the file underneath is the real `RAMFile.cpp`), and (sections
//! 7 to 9) the mesh loader of `meshmdlio.cpp` (`differential/
//! wrap_w3d_mesh.cpp`), the HLOD definition loader of `hlod.cpp`
//! (`differential/wrap_w3d_hlod.cpp`), the collection and aggregate
//! definition loaders of `collect.cpp` and `agg_def.cpp`
//! (`differential/wrap_w3d_collect.cpp`), (section 10) the box, sphere
//! and ring prototype loaders of `boxrobj.cpp`, `sphereobj.cpp`,
//! `ringobj.cpp` and `prim_anim.h` (`differential/wrap_w3d_prim.cpp`), and
//! (section 11) the morph animation loader of `hmorphanim.cpp`
//! (`differential/wrap_w3d_morph.cpp`), and (section 12) the asset
//! manager's loading side: `assetmgr.cpp` (verbatim segments),
//! `htreemgr.cpp`, `hanimmgr.cpp` and `hash.cpp`, over the real loaders of
//! the other sections (`differential/wrap_w3d_assetmgr.cpp`), and (section
//! 13) the animation evaluation: the channels' `Get_Vector`s, time-code
//! search and adaptive-delta decompression (`motchan.cpp`), the sampling of
//! raw, compressed and morph animations (`hrawanim.cpp`, `hcanim.cpp`,
//! `hmorphanim.cpp`), the hierarchy updates (`htree.cpp`, `pivot.cpp`) and
//! `Fast_Slerp` (`quat.cpp`), driven by `differential/wrap_w3d_eval.cpp`,
//! in both branches of `WWMath::Float_To_Long` (the portable truncation and
//! the shipped Win32 `fistp`, each side in the same one; #19),
//! and (section 14) the render side: `ShaderClass::Apply`,
//! `Get_SS_Category` and `Guess_Sort_Level` (`shader.cpp`) on a recording
//! device (`differential/wrap_w3d_shader.cpp`), and the skin deformation
//! (`meshgeometry.cpp`'s `get_deformed_vertices`, `vp.cpp`'s `Transform`)
//! over hierarchies posed by `htree.cpp` (`differential/wrap_w3d_deform.cpp`),
//! and (section 15, #17) the hierarchical-model, distance-LOD and
//! particle-emitter definition loaders of `hmdldef.cpp`, `distlod.cpp` and
//! `part_ldr.cpp` (`differential/wrap_w3d_emit.cpp`), alone and through the
//! asset manager, and (section 16, #49) the particle emitter and buffer
//! render objects of `part_emt.cpp` and `part_buf.cpp`, run frame by frame
//! (`differential/wrap_w3d_part.cpp`).
//!
//! No game assets are available, so every input is **synthetic**, built
//! byte by byte in the layout the upstream readers read: chunk trees with
//! data and sub-chunk flags, micro-chunks, and sizes that are right, short,
//! long, zero or huge; W3D hierarchies and animations with every chunk
//! kind the loaders handle, unknown chunks, old (pre-3.0) and current
//! versions, edge-case counts and frames, and arbitrary float bits (NaN,
//! infinities, subnormals). Half of the inputs are well-formed; the others
//! also get truncations and byte mutations.
//!
//! Compared, byte for byte (floats as raw bits): for the chunk reader, the
//! result of every call of a random script (open/close chunk and
//! micro-chunk, the four queries, reads of any length including ones that
//! wrap the bounds check, seeks, the file position); for the structs, every
//! `sizeof`, every member's name, `offsetof` and `sizeof`, and the members
//! parsed from random bytes; for the loaders, the result code, everything
//! loaded, and the chunk depth and file position the loader leaves.
//!
//! Inputs for which the Rust reports `Undefined` are **not** given to the
//! C++ (counted per kind). Hierarchies with more than 2^20 pivots are also
//! kept out of the C++ comparison (defined upstream, but the C++ would
//! construct every pivot; counted).
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::for_each_w3d_struct;
use rts_lockstep_engine::formats::w3d::hanim::*;
use rts_lockstep_engine::ww::wwmath::FloatToLongMode;
use rts_lockstep_engine::formats::w3d::htree::{self, HTree};
use rts_lockstep_engine::formats::w3d::motchan::*;
use rts_lockstep_engine::formats::w3d::w3d_file::*;
use rts_lockstep_engine::formats::w3d::{W3dError, W3dResult};
use rts_lockstep_engine::ww::chunkio::{ChunkIoError, ChunkLoad, GameFile};
use std::collections::BTreeMap;
use std::sync::Mutex;
use support::{Check, Gen};

/// The wrapper keeps its log and hierarchies in globals.
static CPP_LOCK: Mutex<()> = Mutex::new(());

type Log = Vec<u8>;

fn p32(log: &mut Log, v: u32) {
    log.extend_from_slice(&v.to_le_bytes());
}
fn pname(log: &mut Log, s: &[u8]) {
    let k = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    p32(log, k as u32);
    log.extend_from_slice(&s[..k]);
}

fn cpp_log(mut f: impl FnMut(*mut u8, i32, &mut i32) -> i32) -> Log {
    thread_local! {
        static BUF: std::cell::RefCell<Vec<u8>> = std::cell::RefCell::new(vec![0u8; 1 << 16]);
    }
    BUF.with(|b| {
        let mut buf = b.borrow_mut();
        loop {
            let mut n = 0i32;
            let r = f(buf.as_mut_ptr(), buf.len() as i32, &mut n);
            assert_eq!(r, 0, "C++ wrapper failed");
            if n as usize <= buf.len() {
                return buf[..n as usize].to_vec();
            }
            let len = n as usize;
            buf.resize(len, 0);
        }
    })
}

// ---------------------------------------------------------------------------
// Input generation
// ---------------------------------------------------------------------------

struct Gn {
    g: Gen,
    clean: bool,
}

impl Gn {
    fn below(&mut self, n: u32) -> u32 {
        self.g.below(n)
    }
    fn chance(&mut self, one_in: u32) -> bool {
        !self.clean && self.g.below(one_in) == 0
    }
    fn coin(&mut self) -> bool {
        self.g.below(2) == 0
    }
    fn f32(&mut self) -> f32 {
        if self.below(4) == 0 { self.g.mixed() } else { self.g.uniform(-2.0, 2.0) }
    }
    fn name(&mut self) -> [u8; W3D_NAME_LEN] {
        let mut n = [0u8; W3D_NAME_LEN];
        let len = match self.below(10) {
            0 => 16,
            1 => 15,
            2 => 0,
            _ => 1 + self.below(8) as usize,
        };
        for c in n.iter_mut().take(len) {
            *c = b'A' + self.below(26) as u8;
        }
        if len < 16 && self.chance(6) {
            for c in n.iter_mut().skip(len + 1) {
                *c = self.g.u32() as u8;
            }
        }
        n
    }
}

fn w32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// A chunk: `id`, then the size (with the sub-chunk flag), then `body`; in a
/// dirty input the size is sometimes wrong.
fn chunk(g: &mut Gn, id: u32, sub: bool, body: &[u8]) -> Vec<u8> {
    let mut size = body.len() as u32;
    if g.chance(12) {
        size = match g.below(5) {
            0 => 0,
            1 => size.saturating_sub(1 + g.below(8)),
            2 => size + 1 + g.below(16),
            3 => 0x7FFF_FFF0 + g.below(16),
            _ => g.g.u32(),
        };
    }
    let mut v = Vec::with_capacity(body.len() + 8);
    w32(&mut v, id);
    w32(&mut v, size | if sub && g.coin() { 0x8000_0000 } else { 0 });
    v.extend_from_slice(body);
    v
}

fn mutate(g: &mut Gn, mut v: Vec<u8>) -> Vec<u8> {
    if g.chance(6) && !v.is_empty() {
        let n = g.below(v.len() as u32) as usize;
        v.truncate(n);
    }
    if g.chance(5) && !v.is_empty() {
        for _ in 0..1 + g.below(4) {
            let p = g.below(v.len() as u32) as usize;
            v[p] = g.g.u32() as u8;
        }
    }
    v
}

// ---------------------------------------------------------------------------
// 1. ChunkLoadClass, driven by scripts
// ---------------------------------------------------------------------------

fn gen_chunk_tree(g: &mut Gn, depth: u32) -> Vec<u8> {
    let id = if g.coin() { g.below(8) } else { g.g.u32() };
    let body = if depth < 4 && g.below(3) == 0 {
        let mut b = Vec::new();
        for _ in 0..g.below(4) {
            b.extend(gen_chunk_tree(g, depth + 1));
        }
        b
    } else if g.coin() {
        // micro-chunks
        let mut b = Vec::new();
        for _ in 0..g.below(5) {
            let n = g.below(12) as usize;
            b.push(g.below(16) as u8);
            b.push(if g.chance(5) { g.g.u32() as u8 } else { n as u8 });
            b.extend((0..n).map(|_| g.g.u32() as u8));
        }
        b
    } else {
        (0..g.below(40)).map(|_| g.g.u32() as u8).collect()
    };
    chunk(g, id, depth < 4, &body)
}

fn gen_script(g: &mut Gn) -> Vec<u8> {
    let mut s = Vec::new();
    for _ in 0..g.below(60) {
        let op = match g.below(20) {
            0..=4 => 0,
            5..=7 => 1,
            8..=9 => 10,
            10 => 11,
            11..=12 => 6,
            13 => 7,
            _ => g.below(13) as u8 as u32,
        } as u8;
        s.push(op);
        if op == 10 || op == 11 {
            let n: u32 = match g.below(12) {
                0 => 0xFFFF_FFFF - g.below(64),
                1 => 0x8000_0000 + g.below(64),
                2 => 1 + g.below(70000),
                3 => g.g.u32(),
                _ => g.below(24),
            };
            w32(&mut s, n);
        }
    }
    s
}

fn rust_chunk_script(data: &[u8], script: &[u8]) -> Result<Log, ChunkIoError> {
    let mut log = Log::new();
    let mut c = ChunkLoad::new(GameFile::new(data).unwrap());
    let mut buf = vec![0u8; 65536];
    let mut i = 0;
    while i < script.len() {
        let op = script[i];
        i += 1;
        let mut n = 0u32;
        if op == 10 || op == 11 {
            n = u32::from_le_bytes(script[i..i + 4].try_into().unwrap());
            i += 4;
        }
        let needs = matches!(op, 1 | 2 | 3 | 5 | 6 | 10 | 11);
        if needs && c.cur_chunk_depth() == 0 {
            log.push(b'z');
            continue;
        }
        match op {
            0 => log.extend_from_slice(&[b'O', u8::from(c.open_chunk()?)]),
            1 => log.extend_from_slice(&[b'C', u8::from(c.close_chunk()?)]),
            2 => {
                log.push(b'I');
                p32(&mut log, c.cur_chunk_id()?);
            }
            3 => {
                log.push(b'L');
                p32(&mut log, c.cur_chunk_length()?);
            }
            4 => {
                log.push(b'D');
                p32(&mut log, c.cur_chunk_depth() as u32);
            }
            5 => {
                log.push(b'H');
                p32(&mut log, c.contains_chunks()? as u32);
            }
            6 => log.extend_from_slice(&[b'o', u8::from(c.open_micro_chunk()?)]),
            7 => log.extend_from_slice(&[b'c', u8::from(c.close_micro_chunk()?)]),
            8 => {
                log.push(b'i');
                p32(&mut log, c.cur_micro_chunk_id());
            }
            9 => {
                log.push(b'l');
                p32(&mut log, c.cur_micro_chunk_length());
            }
            10 => {
                buf.fill(0xCD);
                log.push(b'R');
                let r = c.read(&mut buf, n)?;
                p32(&mut log, r);
                log.extend_from_slice(&buf[..(n as usize).min(64)]);
            }
            11 => {
                log.push(b'S');
                p32(&mut log, c.seek(n)?);
            }
            _ => {
                log.push(b'T');
                p32(&mut log, c.file_mut().tell() as u32);
            }
        }
    }
    Ok(log)
}

#[test]
fn chunkio_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gn { g: Gen::new(0xC10), clean: false };
    let mut check = Check::new("ChunkLoadClass (scripted calls)");
    let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
    let (mut calls, mut wrapped_reads) = (0u64, 0u64);
    const CASES: usize = 30_000;
    for case in 0..CASES {
        g.clean = case % 2 == 0;
        let mut data = Vec::new();
        for _ in 0..1 + g.below(3) {
            data.extend(gen_chunk_tree(&mut g, 0));
        }
        if case % 97 == 0 {
            // deep nesting: past the 256-entry stacks
            let depth = 250 + g.below(10);
            let mut d = Vec::new();
            for k in 0..depth {
                w32(&mut d, k);
                w32(&mut d, 0x7FFF_FFF0);
            }
            data = d;
        }
        let data = mutate(&mut g, data);
        let mut script = gen_script(&mut g);
        if case % 97 == 0 {
            script = vec![0u8; 270];
        }
        let rust = match rust_chunk_script(&data, &script) {
            Ok(l) => l,
            Err(ChunkIoError::Undefined(u)) => {
                *excluded.entry(format!("{u:?}")).or_default() += 1;
                continue;
            }
            Err(ChunkIoError::NonTerminating) => unreachable!("scripts are finite"),
        };
        calls += script.len() as u64;
        wrapped_reads += script.windows(5).filter(|w| w[0] == 10 && w[4] >= 0x80).count() as u64;
        let c = cpp_log(|log, cap, n| unsafe {
            cpp::ref_chunkio_run(
                data.as_ptr() as *const _,
                data.len() as i32,
                script.as_ptr(),
                script.len() as i32,
                log,
                cap,
                n,
            )
        });
        check.eq(&rust, &c, || format!("case {case}: data {:02x?} script {:02x?}", data, script));
    }
    println!(
        "chunkio: {CASES} files+scripts, ~{calls} script bytes, ~{wrapped_reads} reads with lengths >= 2^31; \
         excluded (undefined upstream): {} {excluded:?}",
        excluded.values().sum::<u64>()
    );
    check.finish();
}

// ---------------------------------------------------------------------------
// 2. w3d_file.h layouts
// ---------------------------------------------------------------------------

#[test]
fn w3d_struct_layouts_match_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let c = cpp_log(|log, cap, n| unsafe { cpp::ref_w3d_layouts(log, cap, n) });
    let mut rust = Log::new();
    let mut names: Vec<&'static str> = Vec::new();
    macro_rules! layout {
        ($t:ident) => {{
            rust.push(b'S');
            pname(&mut rust, $t::C_NAME.as_bytes());
            p32(&mut rust, <$t as W3dPod>::SIZE as u32);
            for f in $t::FIELDS {
                pname(&mut rust, f.name.as_bytes());
                p32(&mut rust, f.offset as u32);
                p32(&mut rust, f.size as u32);
            }
            names.push($t::C_NAME);
        }};
    }
    for_each_w3d_struct!(layout);
    let mut check = Check::new("w3d_file.h layouts (sizeof, offsetof)");
    check.eq(&rust, &c, || "layout table".to_string());
    let fields: usize = {
        let mut n = 0;
        macro_rules! count {
            ($t:ident) => {
                n += $t::FIELDS.len();
            };
        }
        for_each_w3d_struct!(count);
        n
    };
    println!("w3d_file.h: {} structs, {fields} members", names.len());
    check.finish();

    // Parsing: random bytes into every struct, compared member by member.
    let mut g = Gen::new(0x3D);
    let mut check = Check::new("w3d_file.h structs parsed from bytes");
    let mut index = 0;
    macro_rules! parse {
        ($t:ident) => {{
            for _ in 0..1000 {
                let bytes: Vec<u8> = (0..<$t as W3dPod>::SIZE).map(|_| g.u32() as u8).collect();
                let v = <$t as W3dPod>::read_le(&bytes);
                let all = v.to_bytes();
                let mut rust = Log::new();
                for f in $t::FIELDS {
                    rust.extend_from_slice(&all[f.offset..f.offset + f.size]);
                }
                let c = cpp_log(|log, cap, n| unsafe { cpp::ref_w3d_parse(index, bytes.as_ptr(), log, cap, n) });
                check.eq(&rust, &c, || format!("{} from {:02x?}", $t::C_NAME, bytes));
            }
            #[allow(unused_assignments)]
            {
                index += 1;
            }
        }};
    }
    for_each_w3d_struct!(parse);
    check.finish();
}

// ---------------------------------------------------------------------------
// 3. Loaders: shared logging
// ---------------------------------------------------------------------------

fn log_state(log: &mut Log, c: &mut ChunkLoad<'_>) {
    log.push(b's');
    p32(log, c.cur_chunk_depth() as u32);
    let t = c.file_mut().tell();
    p32(log, t as u32);
}

fn log_matrix(log: &mut Log, m: &rts_lockstep_engine::ww::matrix3d::Matrix3D) {
    for v in m.to_array() {
        p32(log, v.to_bits());
    }
}

fn log_htree(log: &mut Log, t: &HTree) {
    log.extend_from_slice(&t.name);
    p32(log, t.num_pivots as u32);
    p32(log, t.scale_factor.to_bits());
    for i in 0..t.num_pivots.clamp(0, 4096) as usize {
        let p = t.pivot(i).unwrap();
        pname(log, &p.name);
        p32(log, p.parent.unwrap_or(0xFFFF_FFFF));
        log_matrix(log, &p.base_transform);
        p32(log, p.index as u32);
    }
}

fn log_motion(log: &mut Log, c: &Option<MotionChannel>) {
    let Some(c) = c else {
        log.push(0);
        return;
    };
    log.push(1);
    for v in [c.pivot_idx, c.type_, c.vector_len as u32, c.first_frame as u32, c.last_frame as u32, c.data.len() as u32] {
        p32(log, v);
    }
    for f in &c.data {
        p32(log, f.to_bits());
    }
}

fn log_bits(log: &mut Log, c: &Option<BitChannel>) {
    let Some(c) = c else {
        log.push(0);
        return;
    };
    log.push(1);
    for v in [c.pivot_idx, c.type_, c.default_val as u32, c.first_frame as u32, c.last_frame as u32, c.bits.len() as u32] {
        p32(log, v);
    }
    log.extend_from_slice(&c.bits);
}

fn log_compressed(log: &mut Log, c: &Option<CompressedChannel>, adaptive: bool) {
    match c {
        None => log.push(0),
        Some(CompressedChannel::TimeCoded(c)) => {
            assert!(!adaptive);
            log.push(1);
            for v in [c.pivot_idx, c.type_, c.vector_len as u32, c.packet_size, c.num_time_codes, c.last_time_code_idx, 0] {
                p32(log, v);
            }
            p32(log, c.data.len() as u32);
            for w in &c.data {
                p32(log, *w);
            }
        }
        Some(CompressedChannel::AdaptiveDelta(c)) => {
            assert!(adaptive);
            log.push(2);
            for v in [c.pivot_idx, c.type_, c.vector_len as u32, c.num_frames, c.scale.to_bits(), 0x7FFF_FFFF] {
                p32(log, v);
            }
            p32(log, (c.vector_len * 2) as u32);
            p32(log, c.data.len() as u32);
            for w in &c.data {
                p32(log, *w);
            }
        }
    }
}

fn log_tc_bits(log: &mut Log, c: &Option<TimeCodedBitChannel>) {
    let Some(c) = c else {
        log.push(0);
        return;
    };
    log.push(1);
    for v in [c.pivot_idx, c.type_, c.default_val as u32, c.num_time_codes, 0, c.bits.len() as u32] {
        p32(log, v);
    }
    for w in &c.bits {
        p32(log, *w);
    }
}

/// The hierarchies registered for the animation loaders: (16 name bytes,
/// `Num_Pivots()`).
type Trees = Vec<([u8; W3D_NAME_LEN], i32)>;

/// `ref_w3d_load` of wrap_w3d.cpp, in Rust: kind 0 hierarchy, 1 raw
/// animation, 2 compressed animation.
fn rust_load(data: &[u8], kind: i32, trees: &Trees) -> W3dResult<Log> {
    let mut log = Log::new();
    let mut c = ChunkLoad::new(GameFile::new(data).unwrap());
    log.push(b'O');
    if !c.open_chunk()? {
        log.push(0);
        log_state(&mut log, &mut c);
        return Ok(log);
    }
    log.push(1);
    let mut lookup = |n: &[u8; W3D_NAME_LEN]| trees.iter().find(|t| &t.0 == n).map(|t| t.1);
    match kind {
        0 => {
            let t = htree::load_w3d(&mut c)?;
            log.extend_from_slice(&[b'h', u8::from(t.is_none())]);
            if let Some(t) = t {
                log_htree(&mut log, &t);
            }
        }
        1 => {
            let a = HRawAnim::load_w3d(&mut c, &mut lookup)?;
            log.extend_from_slice(&[b'r', u8::from(a.is_none())]);
            if let Some(a) = a {
                log.extend_from_slice(&a.name);
                log.extend_from_slice(&a.hierarchy_name);
                for v in [a.num_frames as u32, a.num_nodes as u32, a.frame_rate.to_bits()] {
                    p32(&mut log, v);
                }
                for (i, m) in &a.node_motion {
                    log.push(b'n');
                    p32(&mut log, *i);
                    for ch in [&m.x, &m.y, &m.z, &m.xr, &m.yr, &m.zr, &m.q] {
                        log_motion(&mut log, ch);
                    }
                    log_bits(&mut log, &m.vis);
                }
            }
        }
        _ => {
            let a = HCompressedAnim::load_w3d(&mut c, &mut lookup)?;
            log.extend_from_slice(&[b'c', u8::from(a.is_none())]);
            if let Some(a) = a {
                log.extend_from_slice(&a.name);
                log.extend_from_slice(&a.hierarchy_name);
                for v in [a.num_frames as u32, a.num_nodes as u32, a.frame_rate.to_bits(), a.flavor as u32] {
                    p32(&mut log, v);
                }
                let adaptive = a.flavor as u32 == ANIM_FLAVOR_ADAPTIVE_DELTA;
                for (i, m) in &a.node_motion {
                    log.push(b'n');
                    p32(&mut log, *i);
                    for ch in [&m.x, &m.y, &m.z, &m.q] {
                        log_compressed(&mut log, ch, adaptive);
                    }
                    log_tc_bits(&mut log, &m.vis);
                }
            }
        }
    }
    log_state(&mut log, &mut c);
    Ok(log)
}

fn cpp_load(data: &[u8], kind: i32) -> Log {
    cpp_log(|log, cap, n| unsafe { cpp::ref_w3d_load(data.as_ptr() as *const _, data.len() as i32, kind, log, cap, n) })
}

fn register_trees(trees: &[([u8; W3D_NAME_LEN], u32)]) -> Trees {
    unsafe { cpp::ref_w3d_reset_htrees() };
    trees
        .iter()
        .map(|(n, p)| {
            let got = unsafe { cpp::ref_w3d_add_htree(n.as_ptr() as *const _, *p) };
            assert_eq!(got, *p as i32, "Num_Pivots of a registered hierarchy");
            (*n, got)
        })
        .collect()
}

fn run_loader_cases(
    name: &str,
    kind: i32,
    cases: usize,
    seed: u64,
    trees: &Trees,
    gen: impl Fn(&mut Gn, &Trees) -> Vec<u8>,
    skip_large: impl Fn(&[u8]) -> bool,
) {
    let mut g = Gn { g: Gen::new(seed), clean: false };
    let mut check = Check::new(name);
    let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
    let (mut ok, mut failed, mut large) = (0u64, 0u64, 0u64);
    for case in 0..cases {
        g.clean = case % 2 == 0;
        let data = gen(&mut g, trees);
        let data = mutate(&mut g, data);
        let rust = match rust_load(&data, kind, trees) {
            Ok(l) => l,
            Err(W3dError::Undefined(u)) => {
                *excluded.entry(format!("{u:?}")).or_default() += 1;
                continue;
            }
            Err(e) => {
                *excluded.entry(format!("{e:?}")).or_default() += 1;
                continue;
            }
        };
        if skip_large(&rust) {
            large += 1;
            continue;
        }
        if rust.len() > 2 && rust[2] != 0 && rust[2] != b's' {
            if rust[3] == 0 {
                ok += 1;
            } else {
                failed += 1;
            }
        }
        if std::env::var_os("W3D_TRACE").is_some() {
            eprintln!("{name} case {case}: {:02x?}", data);
        }
        let c = cpp_load(&data, kind);
        check.eq(&rust, &c, || format!("case {case}: {:02x?}", data));
    }
    println!(
        "{name}: {cases} inputs, {ok} loaded, {failed} load errors; excluded (undefined upstream): {} {excluded:?}; \
         excluded (over 2^20 pivots): {large}",
        excluded.values().sum::<u64>()
    );
    check.finish();
}

// ---------------------------------------------------------------------------
// 4. Hierarchies
// ---------------------------------------------------------------------------

fn gen_version(g: &mut Gn) -> u32 {
    match g.below(6) {
        0 => w3d_make_version(2, g.below(3)),
        1 => w3d_make_version(3, 0) - 1,
        2 => w3d_make_version(3, 0),
        3 => g.g.u32(),
        _ => w3d_make_version(4, 1),
    }
}

fn gen_htree(g: &mut Gn) -> Vec<u8> {
    let version = gen_version(g);
    let n: u32 = match g.below(16) {
        0 => 0,
        1 => 0xFFFF_FFFF,
        2 => 0x8000_0000 + g.below(4),
        3 => 13_000_000 + g.below(1000),
        4 => 1000 + g.below(3000),
        _ => 1 + g.below(6),
    };
    let mut h = W3dHierarchyStruct::from_bytes(&[0; 36]);
    h.version = version;
    h.name = g.name();
    h.num_pivots = n;
    h.center = W3dVectorStruct { x: g.f32(), y: g.f32(), z: g.f32() };
    let mut body = if g.chance(20) {
        chunk(g, W3D_CHUNK_PIVOTS, false, &[])
    } else {
        let hb = h.to_bytes();
        let hb = if g.chance(10) { hb[..g.below(36) as usize].to_vec() } else { hb };
        chunk(g, W3D_CHUNK_HIERARCHY_HEADER, false, &hb)
    };
    let pre30 = version < w3d_make_version(3, 0);
    let count = (if pre30 { n.wrapping_add(1) } else { n }) as i32;
    for _ in 0..g.below(3) {
        if g.below(4) == 0 {
            let junk: Vec<u8> = (0..g.below(20)).map(|_| g.g.u32() as u8).collect();
            let id = W3D_CHUNK_PIVOT_FIXUPS + g.below(3);
            body.extend(chunk(g, id, false, &junk));
            continue;
        }
        let mut pb = Vec::new();
        let k = if count > 0 && count < 10_000 {
            let full = count as u32 - u32::from(pre30);
            if g.chance(6) { g.below(full + 1) } else { full }
        } else {
            g.below(4)
        };
        for _ in 0..k {
            let mut p = W3dPivotStruct::from_bytes(&[0; 60]);
            p.name = g.name();
            p.parent_idx = match g.below(40) {
                0..=3 => 0xFFFF_FFFF,
                4 => 0xFFFF_FFFE,
                5 => count.max(0) as u32,
                6 => count.max(0) as u32 + 1 + g.below(3),
                7 => g.g.u32(),
                _ => g.below(count.clamp(1, 8) as u32),
            };
            p.translation = W3dVectorStruct { x: g.f32(), y: g.f32(), z: g.f32() };
            p.euler_angles = W3dVectorStruct { x: g.f32(), y: g.f32(), z: g.f32() };
            let q = if g.coin() {
                [g.g.uniform(-1.0, 1.0), g.g.uniform(-1.0, 1.0), g.g.uniform(-1.0, 1.0), g.g.uniform(-1.0, 1.0)]
            } else {
                [g.f32(), g.f32(), g.f32(), g.f32()]
            };
            p.rotation = W3dQuaternionStruct { q };
            pb.extend(p.to_bytes());
        }
        body.extend(chunk(g, W3D_CHUNK_PIVOTS, false, &pb));
    }
    chunk(g, W3D_CHUNK_HIERARCHY, true, &body)
}

fn htree_too_large(log: &[u8]) -> bool {
    // 'O' 1 'h' 0 name(16) num_pivots
    log.len() >= 24 && log[2] == b'h' && log[3] == 0 && {
        let n = i32::from_le_bytes(log[20..24].try_into().unwrap());
        n > 1 << 20
    }
}

#[test]
fn htree_load_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    run_loader_cases("HTreeClass::Load_W3D", 0, 20_000, 0x47EE, &Vec::new(), |g, _| gen_htree(g), htree_too_large);
}

// ---------------------------------------------------------------------------
// 5. Animations
// ---------------------------------------------------------------------------

fn tree_names() -> Vec<([u8; W3D_NAME_LEN], u32)> {
    let mut v = Vec::new();
    for (i, n) in [(b"SKEL" as &[u8], 6u32), (b"BIG", 70_000), (b"NONE", 0), (b"NEG", 0x8000_0005), (b"SIXTEENCHARSNAME", 3), (b"P1", 1)]
        .iter()
        .enumerate()
    {
        let mut name = [0u8; W3D_NAME_LEN];
        name[..n.0.len()].copy_from_slice(n.0);
        let _ = i;
        v.push((name, n.1));
    }
    v
}

fn anim_header(g: &mut Gn, trees: &Trees, compressed: bool) -> Vec<u8> {
    let mut h = W3dCompressedAnimHeaderStruct::from_bytes(&[0; 44]);
    h.version = if compressed && g.coin() { w3d_make_version(0, 1) } else { gen_version(g) };
    h.name = g.name();
    h.hierarchy_name = if g.below(8) == 0 { g.name() } else { trees[g.below(trees.len() as u32) as usize].0 };
    h.num_frames = if g.below(6) == 0 { g.g.u32() } else { g.below(100) };
    let mut b = h.to_bytes();
    if compressed {
        let flavor: u16 = match g.below(8) {
            0 => g.g.u32() as u16,
            1..=3 => 1,
            _ => 0,
        };
        let rate = if g.below(6) == 0 { g.g.u32() as u16 } else { 15 + g.below(30) as u16 };
        b[40..42].copy_from_slice(&rate.to_le_bytes());
        b[42..44].copy_from_slice(&flavor.to_le_bytes());
    } else {
        let rate = if g.below(6) == 0 { g.g.u32() } else { 15 + g.below(30) };
        b[40..44].copy_from_slice(&rate.to_le_bytes());
    }
    b
}

fn gen_pivot(g: &mut Gn) -> u16 {
    match g.below(10) {
        0 => g.g.u32() as u16,
        1 => 0xFFFF,
        2 => 5 + g.below(3) as u16,
        _ => g.below(6) as u16,
    }
}

fn gen_raw_anim(g: &mut Gn, trees: &Trees) -> Vec<u8> {
    let hb = anim_header(g, trees, false);
    let id = if g.chance(20) { W3D_CHUNK_ANIMATION_CHANNEL } else { W3D_CHUNK_ANIMATION_HEADER };
    let mut body = chunk(g, id, false, &hb);
    for _ in 0..g.below(8) {
        match g.below(8) {
            0 => {
                let junk: Vec<u8> = (0..g.below(24)).map(|_| g.g.u32() as u8).collect();
                let id = 0x200 + g.below(8);
                body.extend(chunk(g, id, false, &junk));
            }
            1..=2 => {
                // bit channel
                let first = g.below(40) as u16;
                let last = match g.below(8) {
                    0 => first.wrapping_sub(1),
                    1 => first.wrapping_sub(1 + g.below(9) as u16),
                    2 => g.g.u32() as u16,
                    _ => first + g.below(70) as u16,
                };
                let numbits = (i32::from(last) - i32::from(first) + 1) as u32;
                let numbytes = numbits.wrapping_add(7) / 8;
                let mut b = Vec::new();
                b.extend_from_slice(&first.to_le_bytes());
                b.extend_from_slice(&last.to_le_bytes());
                b.extend_from_slice(&(if g.below(6) == 0 { 1u16 } else { 0 }).to_le_bytes());
                b.extend_from_slice(&gen_pivot(g).to_le_bytes());
                b.push(g.below(2) as u8);
                let extra = if numbytes < 64 { numbytes as usize } else { g.below(16) as usize };
                b.extend((0..extra.max(1)).map(|_| g.g.u32() as u8));
                body.extend(chunk(g, W3D_CHUNK_BIT_CHANNEL, false, &b));
            }
            _ => {
                let first = g.below(30) as u16;
                let last = match g.below(10) {
                    0 => first.wrapping_sub(1),
                    1 => first.wrapping_sub(2 + g.below(5) as u16),
                    2 => g.g.u32() as u16,
                    _ => first + g.below(12) as u16,
                };
                let vlen: u16 = match g.below(10) {
                    0 => 0,
                    1 => g.g.u32() as u16,
                    2..=4 => 4,
                    _ => 1,
                };
                let flags: u16 = if g.below(8) == 0 { g.below(20) as u16 } else { g.below(7) as u16 };
                let nf = ((i32::from(last) - i32::from(first) + 1) as u32).wrapping_mul(u32::from(vlen));
                let mut b = Vec::new();
                for v in [first, last, vlen, flags, gen_pivot(g), 0] {
                    b.extend_from_slice(&v.to_le_bytes());
                }
                let floats = if nf < 200 { nf as usize + if g.below(4) == 0 { g.below(3) as usize } else { 0 } } else { g.below(8) as usize };
                for _ in 0..floats.max(1) {
                    b.extend_from_slice(&g.f32().to_bits().to_le_bytes());
                }
                body.extend(chunk(g, W3D_CHUNK_ANIMATION_CHANNEL, false, &b));
            }
        }
    }
    chunk(g, W3D_CHUNK_ANIMATION, true, &body)
}

fn gen_compressed_anim(g: &mut Gn, trees: &Trees) -> Vec<u8> {
    let hb = anim_header(g, trees, true);
    let mut body = chunk(g, W3D_CHUNK_COMPRESSED_ANIMATION_HEADER, false, &hb);
    for _ in 0..g.below(8) {
        match g.below(7) {
            0 => {
                let junk: Vec<u8> = (0..g.below(24)).map(|_| g.g.u32() as u8).collect();
                let id = 0x280 + g.below(8);
                body.extend(chunk(g, id, false, &junk));
            }
            1..=2 => {
                let n: u32 = match g.below(8) {
                    0 => 0,
                    1 => g.g.u32(),
                    2 => 0x2000_0000 + g.below(3),
                    _ => 1 + g.below(12),
                };
                let mut b = Vec::new();
                w32(&mut b, n);
                b.extend_from_slice(&gen_pivot(g).to_le_bytes());
                b.push(if g.below(6) == 0 { 1 } else { 0 });
                b.push(g.below(2) as u8);
                let words = if n < 64 { n as usize } else { g.below(6) as usize };
                for _ in 0..words.max(1) {
                    w32(&mut b, g.g.u32());
                }
                body.extend(chunk(g, W3D_CHUNK_COMPRESSED_BIT_CHANNEL, false, &b));
            }
            _ => {
                let adaptive = g.coin();
                let mut b = Vec::new();
                w32(&mut b, if g.below(6) == 0 { g.g.u32() } else { g.below(40) });
                b.extend_from_slice(&gen_pivot(g).to_le_bytes());
                b.push(if g.below(8) == 0 { g.g.u32() as u8 } else { [1u8, 4][g.below(2) as usize] });
                b.push(if g.below(8) == 0 { g.below(16) as u8 } else { [0u8, 1, 2, 6][g.below(4) as usize] });
                if adaptive {
                    w32(&mut b, g.f32().to_bits());
                }
                let words = g.below(20) as usize + 1;
                for _ in 0..words {
                    w32(&mut b, g.g.u32());
                }
                if g.below(6) == 0 {
                    b.push(g.g.u32() as u8); // a length that is not a multiple of 4
                }
                body.extend(chunk(g, W3D_CHUNK_COMPRESSED_ANIMATION_CHANNEL, false, &b));
            }
        }
    }
    chunk(g, W3D_CHUNK_COMPRESSED_ANIMATION, true, &body)
}

#[test]
fn raw_anim_load_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let trees = register_trees(&tree_names());
    run_loader_cases("HRawAnimClass::Load_W3D", 1, 20_000, 0xA41, &trees, gen_raw_anim, |_| false);
}

#[test]
fn compressed_anim_load_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let trees = register_trees(&tree_names());
    run_loader_cases("HCompressedAnimClass::Load_W3D", 2, 20_000, 0xCA41, &trees, gen_compressed_anim, |_| false);
}

// ---------------------------------------------------------------------------
// 6. CRC_Memory, DX8Wrapper::Convert_Color
// ---------------------------------------------------------------------------

#[test]
fn crc_memory_matches_upstream() {
    use rts_lockstep_engine::ww::realcrc::crc_memory;
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0xC3C);
    let mut check = Check::new("CRC_Memory (realcrc.cpp)");
    for case in 0..20_000 {
        let n = if case < 300 { case } else { g.below(600) as usize };
        let data: Vec<u8> = (0..n).map(|_| g.u32() as u8).collect();
        let seed = if g.below(3) == 0 { g.u32() } else { 0 };
        let c = unsafe { cpp::ref_crc_memory(data.as_ptr(), n as i32, seed) };
        check.eq(crc_memory(&data, seed), c, || format!("{data:02x?} seed {seed:#x}"));
    }
    check.finish();
}

#[test]
fn dx8_convert_color_matches_upstream() {
    use rts_lockstep_engine::formats::w3d::mesh::dx8_convert_color;
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0xD8C);
    let mut check = Check::new("DX8Wrapper::Convert_Color (x87 asm)");
    let mut inputs: Vec<[f32; 4]> = Vec::new();
    for v in 0..=255u32 {
        let f = v as f32 / 255.0;
        inputs.push([f, f, f, f]);
        inputs.push([f * (v as f32 / 255.0), 1.0 - f, f, 0.5]);
    }
    for _ in 0..200_000 {
        let pick = |g: &mut Gen| match g.below(6) {
            0 => g.mixed(),
            1 => g.below(256) as f32 / 255.0,
            2 => g.uniform(-0.01, 1.01),
            _ => g.uniform(0.0, 1.0),
        };
        inputs.push([pick(&mut g), pick(&mut g), pick(&mut g), pick(&mut g)]);
    }
    for c in &inputs {
        let r = unsafe { cpp::ref_dx8_convert_color(c.as_ptr()) };
        check.eq(dx8_convert_color(*c), r, || format!("{c:?}"));
    }
    check.finish();
}

// ---------------------------------------------------------------------------
// 7. Meshes
// ---------------------------------------------------------------------------

use rts_lockstep_engine::formats::w3d::mesh::{self, Arr, MatDesc, Mesh, MeshLoadOptions, PrelitMode};

fn p_arr<T: Copy + PartialEq>(log: &mut Log, a: &Option<Arr<T>>, f: impl Fn(&mut Log, T)) {
    match a {
        None => log.push(0),
        Some(a) => {
            log.push(1);
            p32(log, a.len() as u32);
            for v in a.to_vec() {
                f(log, v);
            }
        }
    }
}

fn idx(i: Option<u32>) -> u32 {
    i.unwrap_or(0xFFFF_FFFF)
}

fn log_matdesc(log: &mut Log, d: &MatDesc) {
    for v in [d.pass_count, d.vertex_count, d.poly_count] {
        p32(log, v as u32);
    }
    for p in &d.passes {
        p32(log, idx(p.material));
        p_arr(log, &p.material_array, |l, v| p32(l, idx(v)));
        p32(log, p.shader);
        p_arr(log, &p.shader_array, p32);
        for s in 0..2 {
            p32(log, idx(p.texture[s]));
            p_arr(log, &p.texture_array[s], |l, v| p32(l, idx(v)));
            p32(log, p.uv_source[s] as u32);
        }
        p32(log, p.dcg_source);
        p32(log, p.dig_source);
    }
    p32(log, d.uv.len() as u32);
    for u in &d.uv {
        p32(log, u.uvs.len() as u32);
        p32(log, u.crc);
        for v in u.uvs.to_vec() {
            p32(log, v[0].to_bits());
            p32(log, v[1].to_bits());
        }
    }
    for c in &d.color_array {
        p_arr(log, c, p32);
    }
}

fn log_mesh(log: &mut Log, m: &Mesh) {
    log.push(b'S');
    log.extend_from_slice(&m.header.to_bytes());
    p32(log, m.name.len() as u32);
    log.extend_from_slice(&m.name);
    for v in [m.flags, m.w3d_attributes, i32::from(m.sort_level) as u32, m.poly_count as u32, m.vertex_count as u32] {
        p32(log, v);
    }
    for v in m.bound_box_min.iter().chain(&m.bound_box_max).chain(&m.bound_sphere_center).chain([&m.bound_sphere_radius]) {
        p32(log, v.to_bits());
    }
    let f3 = |l: &mut Log, v: [f32; 3]| v.iter().for_each(|x| p32(l, x.to_bits()));
    p_arr(log, &m.vertices, f3);
    p_arr(log, &m.normals, f3);
    p_arr(log, &m.polys, |l, v: [u16; 3]| v.iter().for_each(|x| l.extend_from_slice(&x.to_le_bytes())));
    p_arr(log, &m.surface_types, |l, v| l.push(v));
    p_arr(log, &m.planes, |l, v: [f32; 4]| v.iter().for_each(|x| p32(l, x.to_bits())));
    p_arr(log, &m.shade_indices, p32);
    p_arr(log, &m.bone_links, |l, v: u16| l.extend_from_slice(&v.to_le_bytes()));
    p_arr(log, &m.user_text, |l, v| l.push(v));
    match &m.cull_tree {
        None => log.push(0),
        Some(t) => {
            log.push(1);
            p32(log, t.node_count as u32);
            p32(log, t.poly_count as u32);
            for n in t.nodes.to_vec() {
                for x in n.min.iter().chain(&n.max) {
                    p32(log, x.to_bits());
                }
                p32(log, n.front_or_poly0);
                p32(log, n.back_or_poly_count);
            }
            for v in t.poly_indices.to_vec() {
                p32(log, v);
            }
        }
    }
    log.push(u8::from(m.needs_generated_cull_tree));
    p32(log, m.prelit_chunk_id);
    log.extend_from_slice(&m.mat_info.to_bytes());
    p32(log, m.shaders.len() as u32);
    for s in &m.shaders {
        p32(log, s.bits);
    }
    p32(log, m.vertex_materials.len() as u32);
    for v in &m.vertex_materials {
        log.push(u8::from(v.name.is_some()));
        let n = v.name.clone().unwrap_or_default();
        p32(log, n.len() as u32);
        log.extend_from_slice(&n);
        log.extend_from_slice(&v.info.to_bytes());
        for a in &v.mapper_args {
            log.push(u8::from(a.is_some()));
            let a = a.clone().unwrap_or_default();
            p32(log, a.len() as u32);
            log.extend_from_slice(&a);
        }
    }
    p32(log, m.textures.len() as u32);
    for t in &m.textures {
        p32(log, t.name.len() as u32);
        log.extend_from_slice(&t.name);
        p32(log, t.mip_count);
        p32(log, 0); // WW3D_FORMAT_UNKNOWN
        p32(log, if t.no_lod { 0 } else { 0xFFFF_FFFF });
        match t.clamp {
            Some((u, v)) => {
                p32(log, u32::from(u));
                p32(log, u32::from(v));
            }
            None => {
                p32(log, 0xFFFF_FFFF);
                p32(log, 0xFFFF_FFFF);
            }
        }
    }
    log_matdesc(log, &m.default_mat);
    log_matdesc(log, &m.alternate_mat);
}

fn rust_load_mesh(data: &[u8], prelit: i32) -> W3dResult<Log> {
    let mut log = Log::new();
    let mut c = ChunkLoad::new(GameFile::new(data).unwrap());
    log.push(b'O');
    if c.open_chunk()? {
        log.push(1);
        let mode = [PrelitMode::Vertex, PrelitMode::LightmapMultiPass, PrelitMode::LightmapMultiTexture][prelit as usize];
        match mesh::load_w3d(&mut c, &MeshLoadOptions { prelit_mode: mode })? {
            Some(m) => log_mesh(&mut log, &m),
            None => log.extend_from_slice(&[b'e', 2]),
        }
    } else {
        log.push(0);
    }
    log_state(&mut log, &mut c);
    Ok(log)
}

fn rand_bytes(g: &mut Gn, n: usize) -> Vec<u8> {
    (0..n).map(|_| g.g.u32() as u8).collect()
}

fn floats(g: &mut Gn, n: usize) -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..n {
        w32(&mut v, g.f32().to_bits());
    }
    v
}

fn small_or(g: &mut Gn, n: u32, count: u32) -> u32 {
    match g.below(12) {
        0 => g.g.u32(),
        1 => count,
        2 => 0xFFFF_FFFF,
        _ => g.below(n.max(1)),
    }
}

struct MeshGen {
    verts: u32,
    tris: u32,
    vmats: u32,
    shaders: u32,
    textures: u32,
}

fn c_name(g: &mut Gn, max: usize) -> Vec<u8> {
    let n = g.below(max as u32) as usize;
    let mut v: Vec<u8> = (0..n).map(|_| b'a' + g.below(26) as u8).collect();
    if !g.chance(15) {
        v.push(0);
    }
    v
}

fn gen_material_chunks(g: &mut Gn, mg: &mut MeshGen, depth: u32) -> Vec<u8> {
    let mut out = Vec::new();
    match g.below(6) {
        0 => {
            let mut b = Vec::new();
            for v in [if g.below(4) == 0 { g.below(7) } else { 1 + g.below(2) }, mg.vmats, mg.shaders, mg.textures] {
                w32(&mut b, v);
            }
            if g.chance(8) {
                b.truncate(g.below(16) as usize);
            }
            out.extend(chunk(g, W3D_CHUNK_MATERIAL_INFO, false, &b));
        }
        1 => {
            let n = 1 + g.below(4);
            mg.shaders += n;
            let mut b = Vec::new();
            for _ in 0..n {
                for k in 0..16 {
                    b.push(if g.below(10) == 0 { g.g.u32() as u8 } else { g.below([8, 2, 2, 7, 2, 6, 2, 4, 2, 13, 4, 2, 2, 13, 4, 1][k]) as u8 });
                }
            }
            out.extend(chunk(g, W3D_CHUNK_SHADERS, false, &b));
        }
        2 => {
            let mut b = Vec::new();
            for _ in 0..1 + g.below(3) {
                let mut vm = Vec::new();
                if !g.chance(10) {
                    let nm = c_name(g, 20);
                    vm.extend(chunk(g, W3D_CHUNK_VERTEX_MATERIAL_NAME, false, &nm));
                }
                if !g.chance(10) {
                    let info = floats(g, 8);
                    vm.extend(chunk(g, W3D_CHUNK_VERTEX_MATERIAL_INFO, false, &info));
                }
                if g.below(3) == 0 {
                    let a = c_name(g, 30);
                    let id = W3D_CHUNK_VERTEX_MAPPER_ARGS0 + g.below(2);
                    vm.extend(chunk(g, id, false, &a));
                }
                b.extend(chunk(g, W3D_CHUNK_VERTEX_MATERIAL, true, &vm));
                mg.vmats += 1;
            }
            out.extend(chunk(g, W3D_CHUNK_VERTEX_MATERIALS, true, &b));
        }
        3 => {
            let mut b = Vec::new();
            for _ in 0..1 + g.below(3) {
                let mut t = Vec::new();
                let nm = c_name(g, 20);
                t.extend(chunk(g, W3D_CHUNK_TEXTURE_NAME, false, &nm));
                if g.coin() {
                    let mut info = Vec::new();
                    info.extend_from_slice(&(g.g.u32() as u16 & 0x10ff).to_le_bytes());
                    info.extend_from_slice(&(g.below(4) as u16).to_le_bytes());
                    w32(&mut info, g.below(3));
                    w32(&mut info, g.f32().to_bits());
                    t.extend(chunk(g, W3D_CHUNK_TEXTURE_INFO, false, &info));
                }
                let id = if g.chance(12) { W3D_CHUNK_TEXTURE_NAME } else { W3D_CHUNK_TEXTURE };
                b.extend(chunk(g, id, true, &t));
                mg.textures += 1;
            }
            out.extend(chunk(g, W3D_CHUNK_TEXTURES, true, &b));
        }
        _ => {
            // a material pass
            let mut p = Vec::new();
            for _ in 0..1 + g.below(5) {
                match g.below(7) {
                    0 => {
                        let n = if g.coin() { 1 } else { mg.verts };
                        let mut b = Vec::new();
                        for _ in 0..n {
                            w32(&mut b, small_or(g, mg.vmats, mg.vmats));
                        }
                        p.extend(chunk(g, W3D_CHUNK_VERTEX_MATERIAL_IDS, false, &b));
                    }
                    1 => {
                        let n = if g.coin() { 1 } else { mg.tris };
                        let mut b = Vec::new();
                        for _ in 0..n {
                            w32(&mut b, small_or(g, mg.shaders, mg.shaders));
                        }
                        p.extend(chunk(g, W3D_CHUNK_SHADER_IDS, false, &b));
                    }
                    2 | 3 => {
                        let b = rand_bytes(g, 4 * mg.verts as usize);
                        let id = [W3D_CHUNK_DCG, W3D_CHUNK_DIG, W3D_CHUNK_SCG][g.below(3) as usize];
                        p.extend(chunk(g, id, false, &b));
                    }
                    _ => {
                        let mut st = Vec::new();
                        for _ in 0..1 + g.below(3) {
                            match g.below(4) {
                                0 | 1 => {
                                    let n = if g.coin() { 1 } else { mg.tris };
                                    let mut b = Vec::new();
                                    for _ in 0..n {
                                        w32(&mut b, small_or(g, mg.textures, mg.textures));
                                    }
                                    st.extend(chunk(g, W3D_CHUNK_TEXTURE_IDS, false, &b));
                                }
                                2 => {
                                    let n = if g.below(4) == 0 { g.below(5) } else { mg.verts };
                                    let mut b = floats(g, 2 * n as usize);
                                    if g.below(3) == 0 && !b.is_empty() {
                                        // repeat an earlier array: shared by CRC
                                        b = vec![0x3f; b.len()];
                                    }
                                    let id = if g.below(5) == 0 { W3D_CHUNK_TEXCOORDS } else { W3D_CHUNK_STAGE_TEXCOORDS };
                                    st.extend(chunk(g, id, false, &b));
                                }
                                _ => {
                                    let n = if g.below(3) == 0 { g.below(3) } else { mg.tris };
                                    let b = rand_bytes(g, 12 * n as usize);
                                    st.extend(chunk(g, W3D_CHUNK_PER_FACE_TEXCOORD_IDS, false, &b));
                                }
                            }
                        }
                        p.extend(chunk(g, W3D_CHUNK_TEXTURE_STAGE, true, &st));
                    }
                }
            }
            out.extend(chunk(g, W3D_CHUNK_MATERIAL_PASS, true, &p));
        }
    }
    let _ = depth;
    out
}

fn gen_mesh(g: &mut Gn) -> Vec<u8> {
    let mut h = W3dMeshHeader3Struct::from_bytes(&[0; 116]);
    h.version = match g.below(8) {
        0 => w3d_make_version(2, 0),
        1 => w3d_make_version(4, 0),
        2 => w3d_make_version(4, 1),
        3 => g.g.u32(),
        _ => w3d_make_version(4, 2),
    };
    let geom = [0u32, 0x10000, 0x20000, 0x60000, 0x30000][g.below(5) as usize];
    let mut attr = geom;
    for bit in [W3D_MESH_FLAG_TWO_SIDED, W3D_MESH_FLAG_CAST_SHADOW, W3D_MESH_FLAG_NPATCHABLE, OBSOLETE_W3D_MESH_FLAG_LIGHTMAPPED, 0x10, 0x20] {
        if g.below(4) == 0 {
            attr |= bit;
        }
    }
    if g.below(3) == 0 {
        attr |= g.below(16) << 24;
    }
    if g.below(10) == 0 {
        attr = g.g.u32();
    }
    h.attributes = attr;
    h.mesh_name = g.name();
    h.container_name = if g.coin() { [0; 16] } else { g.name() };
    let verts = if g.below(10) == 0 { 0 } else { 1 + g.below(9) };
    let tris = if g.below(10) == 0 { 0 } else { 1 + g.below(9) };
    h.num_vertices = if g.chance(30) { 0x7000_0000 } else { verts };
    h.num_tris = if g.chance(30) { 0x8000_0001 } else { tris };
    h.num_materials = if g.below(4) == 0 { g.below(4) } else { 1 };
    h.sort_level = match g.below(6) {
        0 => g.g.i32(),
        1 => 256 + g.below(3) as i32,
        2 => g.below(32) as i32,
        _ => 0,
    };
    h.min = W3dVectorStruct { x: g.f32(), y: g.f32(), z: g.f32() };
    h.max = W3dVectorStruct { x: g.f32(), y: g.f32(), z: g.f32() };
    h.sph_center = W3dVectorStruct { x: g.f32(), y: g.f32(), z: g.f32() };
    h.sph_radius = g.f32();
    let hb = h.to_bytes();
    let hid = if g.chance(25) { W3D_CHUNK_VERTICES } else { W3D_CHUNK_MESH_HEADER3 };
    let mut body = chunk(g, hid, false, &hb);
    let mut mg = MeshGen { verts, tris, vmats: 0, shaders: 0, textures: 0 };
    for _ in 0..g.below(16) {
        let c = match g.below(22) {
            0 | 1 => {
                let n = (verts + if g.below(6) == 0 { g.below(3) } else { 0 }) as usize;
                let b = floats(g, 3 * n);
                let id = [W3D_CHUNK_VERTICES, W3D_CHUNK_VERTEX_NORMALS, W3D_CHUNK_SURRENDER_NORMALS][g.below(3) as usize];
                chunk(g, id, false, &b)
            }
            2 | 3 => {
                let mut b = Vec::new();
                for _ in 0..tris {
                    for _ in 0..3 {
                        w32(&mut b, if g.below(8) == 0 { g.g.u32() } else { g.below(verts.max(1)) });
                    }
                    w32(&mut b, if g.below(4) == 0 { g.g.u32() } else { g.below(32) });
                    b.extend(floats(g, 4));
                }
                chunk(g, W3D_CHUNK_TRIANGLES, false, &b)
            }
            4 => {
                let t = c_name(g, 40);
                chunk(g, W3D_CHUNK_MESH_USER_TEXT, false, &t)
            }
            5 => {
                let mut b = Vec::new();
                for _ in 0..verts {
                    b.extend_from_slice(&(g.g.u32() as u16).to_le_bytes());
                    b.extend(rand_bytes(g, 6));
                }
                chunk(g, W3D_CHUNK_VERTEX_INFLUENCES, false, &b)
            }
            6 => {
                let b = rand_bytes(g, 4 * verts as usize);
                chunk(g, W3D_CHUNK_VERTEX_SHADE_INDICES, false, &b)
            }
            7 => {
                let b = rand_bytes(g, 4 * verts as usize);
                chunk(g, W3D_CHUNK_VERTEX_COLORS, false, &b)
            }
            8 => {
                let extra = g.below(3);
                let b = floats(g, 2 * (verts + extra) as usize);
                chunk(g, W3D_CHUNK_TEXCOORDS, false, &b)
            }
            9 => {
                let b = rand_bytes(g, 2 * tris as usize);
                chunk(g, W3D_CHUNK_PER_TRI_MATERIALS, false, &b)
            }
            10 => {
                // AABTree
                let nodes = g.below(4);
                let polys = if g.below(4) == 0 { g.below(3) } else { tris };
                let mut hb = Vec::new();
                w32(&mut hb, if g.below(10) == 0 { g.g.u32() } else { nodes });
                w32(&mut hb, polys);
                hb.extend(rand_bytes(g, 24));
                let mut b = chunk(g, W3D_CHUNK_AABTREE_HEADER, false, &hb);
                if g.below(4) != 0 {
                    let extra = if g.below(5) == 0 { 1 } else { 0 };
                    let pi = rand_bytes(g, 4 * (polys + extra) as usize);
                    b.extend(chunk(g, W3D_CHUNK_AABTREE_POLYINDICES, false, &pi));
                }
                if g.below(4) != 0 {
                    let mut nb = Vec::new();
                    for _ in 0..nodes {
                        nb.extend(floats(g, 6));
                        nb.extend(rand_bytes(g, 8));
                    }
                    if g.below(5) == 0 {
                        nb.truncate(g.below(nb.len() as u32 + 1) as usize);
                    }
                    b.extend(chunk(g, W3D_CHUNK_AABTREE_NODES, false, &nb));
                }
                chunk(g, W3D_CHUNK_AABTREE, true, &b)
            }
            11 | 12 => {
                // a pre-lit wrapper around material chunks
                let mut b = Vec::new();
                for _ in 0..1 + g.below(4) {
                    b.extend(gen_material_chunks(g, &mut mg, 1));
                }
                let id = W3D_CHUNK_PRELIT_UNLIT + g.below(4);
                chunk(g, id, true, &b)
            }
            13 => {
                let id = [W3D_CHUNK_DEFORM, W3D_CHUNK_DAMAGE, O_W3D_CHUNK_MATERIALS, O_W3D_CHUNK_SURRENDER_TRIANGLES, 0x777][g.below(5) as usize];
                let n = g.below(20) as usize;
                let b = rand_bytes(g, n);
                chunk(g, id, false, &b)
            }
            _ => gen_material_chunks(g, &mut mg, 0),
        };
        body.extend(c);
    }
    chunk(g, W3D_CHUNK_MESH, true, &body)
}

fn mesh_too_large(log: &[u8]) -> bool {
    // 'O' 1 'S' header(116) name_len name ... : the header's counts
    if log.len() < 3 + 116 || log[2] != b'S' {
        return false;
    }
    let h = &log[3..3 + 116];
    let tris = u32::from_le_bytes(h[40..44].try_into().unwrap());
    let verts = u32::from_le_bytes(h[44..48].try_into().unwrap());
    tris > 1 << 16 || verts > 1 << 16 || log.len() > 1 << 24
}

#[test]
fn mesh_load_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gn { g: Gen::new(0x3E5), clean: false };
    let mut check = Check::new("MeshModelClass::Load_W3D (meshmdlio.cpp)");
    let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
    let (mut ok, mut failed, mut large) = (0u64, 0u64, 0u64);
    const CASES: usize = 20_000;
    for case in 0..CASES {
        g.clean = case % 2 == 0;
        let mesh = gen_mesh(&mut g);
        let data = mutate(&mut g, mesh);
        let prelit = g.below(3) as i32;
        if std::env::var_os("W3D_TRACE").is_some() {
            eprintln!("mesh case {case} (rust)");
        }
        let rust = match rust_load_mesh(&data, prelit) {
            Ok(l) => l,
            Err(W3dError::Undefined(u)) => {
                *excluded.entry(format!("{u:?}")).or_default() += 1;
                continue;
            }
            Err(e) => {
                *excluded.entry(format!("{e:?}")).or_default() += 1;
                continue;
            }
        };
        if mesh_too_large(&rust) {
            large += 1;
            continue;
        }
        match rust.get(2) {
            Some(b'S') => ok += 1,
            Some(b'e') => failed += 1,
            _ => {}
        }
        if std::env::var_os("W3D_TRACE").is_some() {
            eprintln!("mesh case {case}: {:02x?}", data);
        }
        let c = cpp_log(|log, cap, n| unsafe {
            cpp::ref_w3d_load_mesh(data.as_ptr() as *const _, data.len() as i32, prelit, log, cap, n)
        });
        check.eq(&rust, &c, || format!("case {case} (prelit mode {prelit}): {:02x?}", data));
    }
    println!(
        "MeshModelClass::Load_W3D: {CASES} inputs, {ok} loaded, {failed} load errors; excluded (undefined upstream, \
         unsupported or not ending): {} {excluded:?}; excluded (over 2^16 vertices or triangles): {large}",
        excluded.values().sum::<u64>()
    );
    check.finish();
}

// ---------------------------------------------------------------------------
// 8. HLOD definitions (hlod.cpp, HLodDefClass::Load_W3D)
// ---------------------------------------------------------------------------

use rts_lockstep_engine::formats::w3d::hlod::{self, SubObjectArray};

/// A C string as wrap_w3d_hlod.cpp logs it: `strlen`, then the bytes;
/// `0xFFFFFFFF` for a null pointer.
fn pstr(log: &mut Log, s: Option<&[u8]>) {
    match s {
        None => p32(log, 0xFFFF_FFFF),
        Some(b) => {
            p32(log, b.len() as u32);
            log.extend_from_slice(b);
        }
    }
}

fn log_sub_object_array(log: &mut Log, a: &SubObjectArray) {
    p32(log, a.max_screen_size.to_bits());
    p32(log, a.model_count as u32);
    for i in 0..a.model_count.max(0) as usize {
        let s = a.sub_object(i).unwrap();
        pstr(log, s.name.as_deref());
        p32(log, s.bone_index as u32);
    }
}

/// `ref_w3d_load_hlod` of wrap_w3d_hlod.cpp, in Rust. `Ok(None)` when a
/// loaded count is over 2^16 (kept out of the comparison: the log would hold
/// every entry).
fn rust_load_hlod(data: &[u8]) -> W3dResult<Option<Log>> {
    let mut log = Log::new();
    let mut c = ChunkLoad::new(GameFile::new(data).unwrap());
    log.push(b'O');
    if c.open_chunk()? {
        log.push(1);
        match hlod::load_w3d(&mut c)? {
            Some(d) => {
                let big = |n: i32| n > 1 << 16;
                if big(d.lod_count)
                    || d.lods.iter().chain([&d.aggregates]).any(|a| big(a.model_count))
                    || d.proxy_array.as_ref().is_some_and(|p| big(p.length))
                {
                    return Ok(None);
                }
                log.push(b'H');
                pstr(&mut log, Some(&d.name));
                pstr(&mut log, Some(&d.hierarchy_tree_name));
                p32(&mut log, d.lod_count as u32);
                for a in &d.lods {
                    log_sub_object_array(&mut log, a);
                }
                log_sub_object_array(&mut log, &d.aggregates);
                match &d.proxy_array {
                    Some(p) => {
                        log.push(1);
                        p32(&mut log, p.length as u32);
                        for i in 0..p.length.max(0) as usize {
                            let r = p.record(i).unwrap();
                            p32(&mut log, r.bone_index as u32);
                            log.extend_from_slice(&r.name);
                        }
                    }
                    None => log.push(0),
                }
            }
            // 'e', WW3D_ERROR_LOAD_FAILED
            None => log.extend_from_slice(&[b'e', 2]),
        }
    } else {
        log.push(0);
    }
    log_state(&mut log, &mut c);
    Ok(Some(log))
}

/// The header's `LodCount` if the file starts like an HLOD (first chunk, then
/// a whole header chunk inside it): upstream constructs that many
/// `SubObjectArrayClass`es before it reads any LOD, so a file with more than
/// 2^16 is kept out of the C++ comparison even when the load then fails.
fn hlod_header_lod_count(data: &[u8]) -> Option<u32> {
    let mut c = ChunkLoad::new(GameFile::new(data).unwrap());
    if !c.open_chunk().ok()? || !c.open_chunk().ok()? || c.cur_chunk_id().ok()? != W3D_CHUNK_HLOD_HEADER {
        return None;
    }
    let mut hb = [0u8; W3dHLodHeaderStruct::SIZE];
    if c.read_into(&mut hb).ok()? != hb.len() as u32 {
        return None;
    }
    Some(W3dHLodHeaderStruct::from_bytes(&hb).lod_count)
}

fn cpp_load_hlod(data: &[u8]) -> Log {
    cpp_log(|log, cap, n| unsafe { cpp::ref_w3d_load_hlod(data.as_ptr() as *const _, data.len() as i32, log, cap, n) })
}

/// A chunk with its exact size (the named cases).
fn hchunk(id: u32, body: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(body.len() + 8);
    w32(&mut v, id);
    w32(&mut v, body.len() as u32);
    v.extend_from_slice(body);
    v
}

fn hlod_header(name: &[u8], hier: &[u8], lod_count: u32) -> Vec<u8> {
    let mut h = W3dHLodHeaderStruct::from_bytes(&[0; 40]);
    h.version = W3D_CURRENT_HLOD_VERSION;
    h.lod_count = lod_count;
    h.name[..name.len()].copy_from_slice(name);
    h.hierarchy_name[..hier.len()].copy_from_slice(hier);
    hchunk(W3D_CHUNK_HLOD_HEADER, &h.to_bytes())
}

fn hlod_sub_object(bone: u32, name: &[u8]) -> Vec<u8> {
    let mut s = W3dHLodSubObjectStruct::from_bytes(&[0; 36]);
    s.bone_index = bone;
    s.name[..name.len()].copy_from_slice(name);
    hchunk(W3D_CHUNK_HLOD_SUB_OBJECT, &s.to_bytes())
}

/// An array chunk `id`: its header, then the given sub-objects.
fn hlod_array(id: u32, model_count: u32, max: f32, subs: &[(u32, &[u8])]) -> Vec<u8> {
    let mut h = W3dHLodArrayHeaderStruct::from_bytes(&[0; 8]);
    h.model_count = model_count;
    h.max_screen_size = max;
    let mut body = hchunk(W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER, &h.to_bytes());
    for &(bone, name) in subs {
        body.extend(hlod_sub_object(bone, name));
    }
    hchunk(id, &body)
}

/// The named cases of `formats::w3d::hlod`'s unit tests, as whole files (the
/// HLOD chunk around them).
fn hlod_named_cases() -> Vec<(&'static str, Vec<u8>)> {
    let wrap = |body: Vec<u8>| hchunk(W3D_CHUNK_HLOD, &body);
    let mut v = Vec::new();

    let mut b = hlod_header(b"TANK", b"TANKSKL", 2);
    b.extend(hlod_array(W3D_CHUNK_HLOD_LOD_ARRAY, 2, f32::MAX, &[(1, b"TANK.BODY"), (2, b"TANK.TURRET")]));
    b.extend(hlod_array(W3D_CHUNK_HLOD_LOD_ARRAY, 2, 0.25, &[(0, b"TANKLO"), (0xFFFF_FFFF, b"X")]));
    b.extend(hchunk(0x0000_0799, &[1, 2, 3]));
    b.extend(hlod_array(W3D_CHUNK_HLOD_AGGREGATE_ARRAY, 1, 1.0, &[(3, b"FXGLOW")]));
    b.extend(hlod_array(W3D_CHUNK_HLOD_PROXY_ARRAY, 1, 0.0, &[(5, b"SPAWN")]));
    v.push(("normal file", wrap(b)));

    let mut b = hlod_header(b"H", b"", 1);
    b.extend(hlod_array(W3D_CHUNK_HLOD_LOD_ARRAY, 3, 50.0, &[(4, b"A")]));
    b.extend(hlod_array(W3D_CHUNK_HLOD_PROXY_ARRAY, 2, 0.0, &[(9, b"P")]));
    v.push(("short arrays", wrap(b)));

    let mut b = hlod_header(b"H", b"S", 1);
    let mut lod = hlod_sub_object(7, b"WRONG");
    lod.extend(hlod_array(W3D_CHUNK_HLOD_PROXY_ARRAY, 1, 0.0, &[(4, b"INNER")]));
    b.extend(hchunk(W3D_CHUNK_HLOD_LOD_ARRAY, &lod));
    b.extend(hlod_array(W3D_CHUNK_HLOD_AGGREGATE_ARRAY, 1, 5.0, &[(2, b"AGG")]));
    v.push(("failed LOD header", wrap(b)));

    let mut b = hlod_header(b"H", b"S", 0);
    b.extend(hlod_array(W3D_CHUNK_HLOD_AGGREGATE_ARRAY, 1, 2.0, &[(1, b"G")]));
    b.extend(hlod_array(W3D_CHUNK_HLOD_PROXY_ARRAY, 1, 0.0, &[(3, b"P")]));
    b.extend(hchunk(W3D_CHUNK_HLOD_AGGREGATE_ARRAY, &[]));
    b.extend(hchunk(W3D_CHUNK_HLOD_PROXY_ARRAY, &[]));
    v.push(("second failed aggregate and proxy", wrap(b)));

    v.push(("16-character name", wrap(hlod_header(b"ABCDEFGHIJKLMNOP", b"SKL", 0))));

    let mut b = hlod_header(b"N", b"S", 0);
    b.extend(hlod_array(W3D_CHUNK_HLOD_PROXY_ARRAY, 1, 0.0, &[(1, &[b'Q'; 32])]));
    v.push(("32-character proxy name", wrap(b)));

    v.push(("first chunk not the header", wrap(hlod_array(W3D_CHUNK_HLOD_LOD_ARRAY, 0, 0.0, &[]))));
    v.push(("short header", wrap(hchunk(W3D_CHUNK_HLOD_HEADER, &[0; 39]))));
    let mut b = hlod_header(b"H", b"S", 2);
    b.extend(hlod_array(W3D_CHUNK_HLOD_LOD_ARRAY, 0, 0.0, &[]));
    v.push(("fewer LOD arrays than LodCount", wrap(b.clone())));
    b.extend(hlod_array(W3D_CHUNK_HLOD_AGGREGATE_ARRAY, 0, 0.0, &[]));
    v.push(("mis-typed LOD array", wrap(b)));
    let mut b = hlod_header(b"H", b"S", 0);
    b.extend(hlod_array(W3D_CHUNK_HLOD_PROXY_ARRAY, 0, 0.0, &[]));
    v.push(("empty proxy array", wrap(b)));
    v
}

/// The named cases: the files of `formats::w3d::hlod`'s unit tests (a
/// normal file; short LOD and proxy arrays; a LOD array whose first chunk
/// is not its header; a second aggregate and proxy array that fail before
/// their header; a 16-character `Name`; a 32-character proxy name) and its
/// load failures (first chunk not the header, short header, fewer LOD arrays
/// than `LodCount`, a mis-typed LOD array), plus an empty proxy array. The
/// cases the unit tests show undefined (`StringOverrun`, `Allocation`) are
/// not given to the C++.
///
/// Compared byte for byte: the load result, the definition (name and
/// hierarchy name as `strlen` and bytes, `LodCount`, each LOD array's and the
/// aggregates' `MaxScreenSize` bits, `ModelCount`, every entry's name, or
/// null, and bone index; the proxy array's presence, `Length()` and each
/// record's bone index and 32 name bytes), and the chunk depth and file
/// position the loader leaves.
///
/// Predicted before the run: all 11 agree with upstream's own hlod.cpp, in
/// particular the quirks the unit tests pin (the unread entries of a short
/// array as null names and bone 0; the failed LOD header reading the proxy
/// array from inside the LOD array and leaving depth 2; the second
/// aggregate keeping the first and the second proxy array dropping it; the
/// 16-character name running into the hierarchy name). A disagreement would
/// mean the Rust is wrong.
#[test]
fn hlod_named_cases_match_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut check = Check::new("HLodDefClass::Load_W3D (hlod.cpp), named cases");
    let cases = hlod_named_cases();
    for (name, data) in &cases {
        let rust = rust_load_hlod(data).unwrap_or_else(|e| panic!("{name}: Rust undefined: {e:?}")).expect("small counts");
        let c = cpp_load_hlod(data);
        check.eq(&rust, &c, || format!("{name}: {:02x?}", data));
    }
    println!("HLodDefClass::Load_W3D named cases: {} compared", cases.len());
    check.finish();
}

fn gen_hlod_name(g: &mut Gn, max: usize) -> Vec<u8> {
    let len = match g.below(12) {
        0 => max,
        1 => max - 1,
        2 => 0,
        _ => 1 + g.below(10) as usize,
    };
    (0..len).map(|_| b'A' + g.below(26) as u8).collect()
}

fn gen_hlod_array(g: &mut Gn, id: u32) -> Vec<u8> {
    let full = g.below(5);
    let count = match g.below(24) {
        0 => 0xFFFF_FFFF,
        1 => 0x2000_0000,
        2 => 1000 + g.below(3000),
        3..=5 => full + 1 + g.below(3),
        _ => full,
    };
    let mut body = Vec::new();
    if g.below(16) != 0 {
        let mut h = W3dHLodArrayHeaderStruct::from_bytes(&[0; 8]);
        h.model_count = count;
        h.max_screen_size = g.f32();
        let hb = h.to_bytes();
        let hb = if g.chance(10) { hb[..g.below(8) as usize].to_vec() } else { hb };
        let hid = if g.chance(16) { W3D_CHUNK_HLOD_SUB_OBJECT } else { W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER };
        body.extend(chunk(g, hid, false, &hb));
    }
    for _ in 0..full {
        let mut s = W3dHLodSubObjectStruct::from_bytes(&[0; 36]);
        s.bone_index = if g.below(6) == 0 { g.g.u32() } else { g.below(20) };
        let n = gen_hlod_name(g, 32);
        s.name[..n.len()].copy_from_slice(&n);
        let sb = s.to_bytes();
        let sb = if g.chance(12) { sb[..g.below(36) as usize].to_vec() } else { sb };
        let sid = if g.chance(16) { W3D_CHUNK_HLOD_PROXY_ARRAY } else { W3D_CHUNK_HLOD_SUB_OBJECT };
        body.extend(chunk(g, sid, false, &sb));
    }
    if g.below(8) == 0 {
        // a nested array (read from inside this one after a failure)
        let inner = [W3D_CHUNK_HLOD_PROXY_ARRAY, W3D_CHUNK_HLOD_AGGREGATE_ARRAY, W3D_CHUNK_HLOD_LOD_ARRAY][g.below(3) as usize];
        let mut h = W3dHLodArrayHeaderStruct::from_bytes(&[0; 8]);
        h.model_count = 1;
        let mut ib = chunk(g, W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER, false, &h.to_bytes());
        ib.extend(chunk(g, W3D_CHUNK_HLOD_SUB_OBJECT, false, &W3dHLodSubObjectStruct::from_bytes(&[0; 36]).to_bytes()));
        body.extend(chunk(g, inner, true, &ib));
    }
    chunk(g, id, true, &body)
}

fn gen_hlod(g: &mut Gn) -> Vec<u8> {
    let lods = g.below(4);
    let lod_count = match g.below(30) {
        0 => 0xFFFF_FFFF,
        1 => 0x8000_0000,
        2 => 0x0800_0000,
        3..=5 => lods + 1,
        6 => lods.saturating_sub(1),
        _ => lods,
    };
    let mut h = W3dHLodHeaderStruct::from_bytes(&[0; 40]);
    h.version = if g.coin() { W3D_CURRENT_HLOD_VERSION } else { g.g.u32() };
    h.lod_count = lod_count;
    let n = gen_hlod_name(g, 16);
    h.name[..n.len()].copy_from_slice(&n);
    let n = gen_hlod_name(g, 16);
    h.hierarchy_name[..n.len()].copy_from_slice(&n);
    let mut body = if g.chance(20) {
        chunk(g, W3D_CHUNK_HLOD_LOD_ARRAY, true, &[])
    } else {
        let hb = h.to_bytes();
        let hb = if g.chance(10) { hb[..g.below(40) as usize].to_vec() } else { hb };
        chunk(g, W3D_CHUNK_HLOD_HEADER, false, &hb)
    };
    for _ in 0..lods {
        let id = if g.chance(16) { W3D_CHUNK_HLOD_AGGREGATE_ARRAY } else { W3D_CHUNK_HLOD_LOD_ARRAY };
        body.extend(gen_hlod_array(g, id));
    }
    for _ in 0..g.below(4) {
        match g.below(5) {
            0 | 1 => body.extend(gen_hlod_array(g, W3D_CHUNK_HLOD_AGGREGATE_ARRAY)),
            2 | 3 => body.extend(gen_hlod_array(g, W3D_CHUNK_HLOD_PROXY_ARRAY)),
            _ => {
                let junk: Vec<u8> = (0..g.below(20)).map(|_| g.g.u32() as u8).collect();
                let id = if g.coin() { W3D_CHUNK_HLOD_SUB_OBJECT } else { g.g.u32() };
                body.extend(chunk(g, id, false, &junk));
            }
        }
    }
    chunk(g, W3D_CHUNK_HLOD, true, &body)
}

/// Random HLOD files, half well-formed and half also with wrong chunk sizes,
/// truncations and byte mutations (as the other loaders' tests): header
/// names of 0 to 16 characters, `LodCount` matching the LOD arrays or not
/// (and negative or too large), LOD arrays mis-typed, arrays whose header is
/// missing, mis-typed or short, `ModelCount` matching the sub-objects or
/// larger (or negative or too large), sub-object names of 0 to 32
/// characters, mis-typed or short sub-object chunks, arrays nested inside
/// arrays, and any number of aggregate, proxy and unknown chunks after the
/// LODs. Compared as [`hlod_named_cases_match_upstream`]. Inputs the Rust
/// reports undefined are not given to the C++ (counted per kind), nor are
/// ones with a `LodCount` or a loaded count over 2^16 (counted).
///
/// Predicted before the run: no disagreement; roughly a quarter to a third
/// of the inputs excluded as undefined, mostly `StringOverrun` (16-character
/// hierarchy names and 32-character sub-object names) and `Allocation`.
#[test]
fn hlod_load_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gn { g: Gen::new(0x41_0D), clean: false };
    let mut check = Check::new("HLodDefClass::Load_W3D (hlod.cpp)");
    let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
    let (mut ok, mut failed, mut large) = (0u64, 0u64, 0u64);
    const CASES: usize = 20_000;
    for case in 0..CASES {
        g.clean = case % 2 == 0;
        let data = gen_hlod(&mut g);
        let data = mutate(&mut g, data);
        let rust = match rust_load_hlod(&data) {
            Ok(Some(l)) => l,
            Ok(None) => {
                large += 1;
                continue;
            }
            Err(W3dError::Undefined(u)) => {
                *excluded.entry(format!("{u:?}")).or_default() += 1;
                continue;
            }
            Err(e) => {
                *excluded.entry(format!("{e:?}")).or_default() += 1;
                continue;
            }
        };
        if hlod_header_lod_count(&data).is_some_and(|n| n > 1 << 16) {
            large += 1;
            continue;
        }
        match rust.get(2) {
            Some(b'H') => ok += 1,
            Some(b'e') => failed += 1,
            _ => {}
        }
        if std::env::var_os("W3D_TRACE").is_some() {
            eprintln!("hlod case {case}: {:02x?}", data);
        }
        let c = cpp_load_hlod(&data);
        check.eq(&rust, &c, || format!("case {case}: {:02x?}", data));
    }
    println!(
        "HLodDefClass::Load_W3D: {CASES} inputs, {ok} loaded, {failed} load errors; excluded (undefined upstream or \
         not ending): {} {excluded:?}; excluded (over 2^16 LODs or entries): {large}",
        excluded.values().sum::<u64>()
    );
    check.finish();
}

// ---------------------------------------------------------------------------
// 9. Collections and aggregates (collect.cpp, snappts.cpp, agg_def.cpp)
// ---------------------------------------------------------------------------

use rts_lockstep_engine::formats::w3d::{agg_def, collect};

/// `ref_w3d_load_collect` of wrap_w3d_collect.cpp, in Rust: kind 0
/// collection, 1 aggregate. `Ok(None)` when an object-name array is over
/// 2^16 bytes (kept out of the comparison: the log holds every byte).
fn rust_load_collect(data: &[u8], kind: i32) -> W3dResult<Option<Log>> {
    let mut log = Log::new();
    let mut c = ChunkLoad::new(GameFile::new(data).unwrap());
    log.push(b'O');
    if c.open_chunk()? {
        log.push(1);
        if kind == 0 {
            match collect::load_w3d(&mut c)? {
                Some(d) => {
                    if d.object_names.iter().any(|n| n.len > 1 << 16) {
                        return Ok(None);
                    }
                    log.push(b'C');
                    log.extend_from_slice(&d.name);
                    p32(&mut log, d.object_names.len() as u32);
                    for n in &d.object_names {
                        p32(&mut log, n.len);
                        let mut b = n.bytes.clone();
                        b.resize(n.len as usize, 0);
                        log.extend(b);
                    }
                    p32(&mut log, d.proxies.len() as u32);
                    for p in &d.proxies {
                        pstr(&mut log, Some(&p.name));
                        for row in p.transform {
                            for v in row {
                                p32(&mut log, v.to_bits());
                            }
                        }
                    }
                    match &d.snap_points {
                        None => log.push(0),
                        Some(pts) => {
                            log.push(1);
                            p32(&mut log, pts.len() as u32);
                            for v in pts.iter().flatten() {
                                p32(&mut log, v.to_bits());
                            }
                        }
                    }
                }
                None => log.extend_from_slice(&[b'e', 2]),
            }
        } else {
            match agg_def::load_w3d(&mut c)? {
                Some(d) => {
                    log.push(b'A');
                    pstr(&mut log, d.name.as_deref());
                    p32(&mut log, d.version);
                    log.extend(d.info.to_bytes());
                    log.extend(d.misc_info.to_bytes());
                    p32(&mut log, d.subobjects.len() as u32);
                    for s in &d.subobjects {
                        for name in [&s.subobject_name, &s.bone_name] {
                            let mut b = name.clone();
                            b.resize(32, 0);
                            log.extend(b);
                        }
                    }
                }
                None => log.extend_from_slice(&[b'e', 2]),
            }
        }
    } else {
        log.push(0);
    }
    log_state(&mut log, &mut c);
    Ok(Some(log))
}

fn cpp_load_collect(data: &[u8], kind: i32) -> Log {
    cpp_log(|log, cap, n| unsafe { cpp::ref_w3d_load_collect(data.as_ptr() as *const _, data.len() as i32, kind, log, cap, n) })
}

fn coll_header(name: &[u8], count: u32) -> Vec<u8> {
    let mut h = W3dCollectionHeaderStruct::from_bytes(&[0; 32]);
    h.version = W3D_CURRENT_COLLECTION_VERSION;
    h.name[..name.len()].copy_from_slice(name);
    h.render_object_count = count;
    hchunk(W3D_CHUNK_COLLECTION_HEADER, &h.to_bytes())
}

fn coll_placeholder(transform: [[f32; 3]; 4], name_len: u32, name: &[u8]) -> Vec<u8> {
    let mut p = W3dPlaceholderStruct::from_bytes(&[0; 56]);
    p.version = W3D_CURRENT_PLACEHOLDER_VERSION;
    p.transform = transform;
    p.name_len = name_len;
    let mut b = p.to_bytes();
    b.extend_from_slice(name);
    hchunk(W3D_CHUNK_PLACEHOLDER, &b)
}

fn coll_points(p: &[[f32; 3]]) -> Vec<u8> {
    let b: Vec<u8> = p.iter().flat_map(|v| v.iter().flat_map(|f| f.to_le_bytes())).collect();
    hchunk(W3D_CHUNK_POINTS, &b)
}

fn agg_header(version: u32, name: &[u8]) -> Vec<u8> {
    let mut h = W3dAggregateHeaderStruct::from_bytes(&[0; 20]);
    h.version = version;
    h.name[..name.len()].copy_from_slice(name);
    hchunk(W3D_CHUNK_AGGREGATE_HEADER, &h.to_bytes())
}

fn agg_info(base: &[u8], count: u32, subs: &[(&[u8], &[u8])]) -> Vec<u8> {
    let mut i = W3dAggregateInfoStruct::from_bytes(&[0; 36]);
    i.base_model_name[..base.len()].copy_from_slice(base);
    i.subobject_count = count;
    let mut b = i.to_bytes();
    for (n, bone) in subs {
        let mut s = W3dAggregateSubobjectStruct::from_bytes(&[0; 64]);
        s.subobject_name[..n.len()].copy_from_slice(n);
        s.bone_name[..bone.len()].copy_from_slice(bone);
        b.extend(s.to_bytes());
    }
    hchunk(W3D_CHUNK_AGGREGATE_INFO, &b)
}

fn agg_class_info(id: u32, flags: u32) -> Vec<u8> {
    let mut m = W3dAggregateMiscInfo::from_bytes(&[0; 20]);
    m.original_class_id = id;
    m.flags = flags;
    hchunk(W3D_CHUNK_AGGREGATE_CLASS_INFO, &m.to_bytes())
}

/// The named cases of `formats::w3d::collect`'s and `agg_def`'s unit
/// tests, as whole files: (kind, name, file).
fn collect_named_cases() -> Vec<(i32, &'static str, Vec<u8>)> {
    let coll = |body: Vec<u8>| hchunk(W3D_CHUNK_COLLECTION, &body);
    let agg = |body: Vec<u8>| hchunk(W3D_CHUNK_AGGREGATE, &body);
    let mut v = Vec::new();

    let mut b = coll_header(b"COLL", 2);
    b.extend(hchunk(W3D_CHUNK_COLLECTION_OBJ_NAME, b"MESH1\0"));
    b.extend(hchunk(W3D_CHUNK_TRANSFORM_NODE, &[0; 60]));
    b.extend(hchunk(W3D_CHUNK_COLLECTION_OBJ_NAME, b"MESH2\0"));
    let t = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0], [10.0, 11.0, 12.0]];
    b.extend(coll_placeholder(t, 2, b"PH"));
    b.extend(coll_points(&[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]));
    v.push((0, "collection: names, placeholder, points", coll(b)));

    let mut b = coll_header(b"ABCDEFGHIJKLMNOP", 1);
    b.extend(hchunk(W3D_CHUNK_COLLECTION_OBJ_NAME, b"ABC"));
    b.extend(hchunk(W3D_CHUNK_COLLECTION_OBJ_NAME, b"X\0"));
    b.extend(hchunk(W3D_CHUNK_COLLECTION_OBJ_NAME, b""));
    b.extend(coll_placeholder([[0.0; 3]; 4], 10, b"SHORT"));
    b.extend(coll_points(&[[1.0, 1.0, 1.0], [2.0, 2.0, 2.0]]));
    let mut p2 = coll_points(&[[7.0, 8.0, 9.0]]);
    p2[4] = 14;
    p2.extend_from_slice(&[0xAA, 0xBB]);
    b.extend(p2);
    v.push((0, "collection: unchecked reads, replaced points, 16-character name", coll(b)));

    let mut b = coll_header(b"C", 0);
    let mut n = hchunk(W3D_CHUNK_COLLECTION_OBJ_NAME, b"ABCD");
    n[4] = 100;
    b.extend(n);
    let mut f = W3D_CHUNK_COLLECTION.to_le_bytes().to_vec();
    f.extend_from_slice(&(b.len() as u32 + 96).to_le_bytes());
    f.extend(b);
    v.push((0, "collection: object name longer than the file", f));

    v.push((0, "collection: first chunk not the header", coll(hchunk(W3D_CHUNK_COLLECTION_OBJ_NAME, b"A\0"))));
    v.push((0, "collection: short header", coll(hchunk(W3D_CHUNK_COLLECTION_HEADER, &[0; 31]))));
    v.push((0, "collection: RenderObjectCount 0x1FFFFFFF", coll(coll_header(b"C", 0x1FFF_FFFF))));

    let mut b = agg_header(W3D_CURRENT_AGGREGATE_VERSION, b"AGG");
    b.extend(agg_info(b"BASE", 2, &[(b"GUN", b"BONE01"), (b"FLAG", b"BONE02")]));
    b.extend(hchunk(W3D_CHUNK_TEXTURE_REPLACER_INFO, &5u32.to_le_bytes()));
    b.extend(agg_class_info(0, 3));
    b.extend(hchunk(0x0000_06FF, &[9; 7]));
    v.push((1, "aggregate: header, info, sub-objects, class info", agg(b)));

    v.push((1, "aggregate: empty", agg(Vec::new())));
    let mut b = agg_header(1, b"ONE");
    b.extend(agg_info(b"B1", 2, &[(b"A", b"BA"), (b"B", b"BB")]));
    b.extend(hchunk(W3D_CHUNK_TEXTURE_REPLACER_INFO, &[1, 2]));
    b.extend(agg_header(2, b"TWO"));
    b.extend(agg_info(b"B2", 1, &[(b"C", b"BC")]));
    v.push((1, "aggregate: repeated header and info, short texture replacer", agg(b)));

    v.push((1, "aggregate: short header", agg(hchunk(W3D_CHUNK_AGGREGATE_HEADER, &[0; 19]))));
    v.push((1, "aggregate: short info", agg(hchunk(W3D_CHUNK_AGGREGATE_INFO, &[0; 35]))));
    v.push((1, "aggregate: fewer sub-objects than the count", agg(agg_info(b"B", 2, &[(b"A", b"BA")]))));
    v.push((1, "aggregate: short class info", agg(hchunk(W3D_CHUNK_AGGREGATE_CLASS_INFO, &[0; 19]))));
    v.push((1, "aggregate: 31-character sub-object name", agg(agg_info(b"B", 1, &[(&[b'N'; 31], b"BONE")]))));
    v
}

/// The named cases: the files of `formats::w3d::collect`'s unit tests (two
/// object names, a skipped transform node, a placeholder and snap points;
/// an unterminated object name, an empty one, a placeholder name read that
/// fails its chunk bound, a second points chunk with a partial point, a
/// 16-character name; an object-name chunk longer than the file; the two
/// header failures; a `RenderObjectCount` just under the allocation limit)
/// and of `agg_def`'s (a full aggregate; an empty one; repeated header and
/// info chunks and a short texture-replacer header; the four short-read
/// failures; a 31-character sub-object name). The undefined cases of those
/// tests are not given to the C++.
///
/// Compared byte for byte: the load result; for a collection the 16 name
/// bytes, every object-name array (its size, from the C++ `new[]` registry,
/// and all its bytes), every proxy's name and 12 transform floats, the snap
/// points; for an aggregate the name (or null), version, the bytes of
/// `m_Info` and `m_MiscInfo`, every sub-object's two 32-byte names; and the
/// chunk depth and file position the loader leaves.
///
/// Predicted before the run: all ~~15~~ **14** (corrected after the run: a
/// miscount of the list) agree, in particular the transposed
/// placeholder transform, the partial object name (`ABCD` and 96 zeros),
/// the replaced snap points, the appended aggregate sub-objects, and the
/// empty aggregate's class id 25 (`CLASSID_HLOD`, counted by hand from
/// rendobj.h's enum: the C++ checks the count).
#[test]
fn collect_named_cases_match_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut check = Check::new("CollectionDefClass::Load / AggregateDefClass::Load_W3D, named cases");
    let cases = collect_named_cases();
    for (kind, name, data) in &cases {
        let rust =
            rust_load_collect(data, *kind).unwrap_or_else(|e| panic!("{name}: Rust undefined: {e:?}")).expect("small arrays");
        let c = cpp_load_collect(data, *kind);
        check.eq(&rust, &c, || format!("{name}: {:02x?}", data));
    }
    println!("collection/aggregate named cases: {} compared", cases.len());
    check.finish();
}

fn gen_text(g: &mut Gn, max: usize) -> Vec<u8> {
    let len = match g.below(12) {
        0 => max,
        1 => max.saturating_sub(1),
        2 => 0,
        _ => 1 + g.below(10) as usize,
    };
    (0..len.min(max)).map(|_| b'A' + g.below(26) as u8).collect()
}

fn gen_collection(g: &mut Gn) -> Vec<u8> {
    let mut h = W3dCollectionHeaderStruct::from_bytes(&[0; 32]);
    h.version = if g.coin() { W3D_CURRENT_COLLECTION_VERSION } else { g.g.u32() };
    let n = gen_text(g, 16);
    h.name[..n.len()].copy_from_slice(&n);
    h.render_object_count = match g.below(20) {
        0 => 0xFFFF_FFFF,
        1 => 0x2000_0000,
        2 => 0x1FFF_FFFF,
        _ => g.below(6),
    };
    let mut body = if g.chance(20) {
        chunk(g, W3D_CHUNK_COLLECTION_OBJ_NAME, false, b"X\0")
    } else {
        let hb = h.to_bytes();
        let hb = if g.chance(10) { hb[..g.below(32) as usize].to_vec() } else { hb };
        chunk(g, W3D_CHUNK_COLLECTION_HEADER, false, &hb)
    };
    for _ in 0..g.below(7) {
        match g.below(6) {
            0 | 1 => {
                let mut b = gen_text(g, 24);
                if g.below(3) != 0 {
                    b.push(0);
                }
                if g.below(6) == 0 {
                    b.extend((0..g.below(6)).map(|_| g.g.u32() as u8));
                }
                body.extend(chunk(g, W3D_CHUNK_COLLECTION_OBJ_NAME, false, &b));
            }
            2 => {
                let mut p = W3dPlaceholderStruct::from_bytes(&[0; 56]);
                p.version = W3D_CURRENT_PLACEHOLDER_VERSION;
                for row in p.transform.iter_mut() {
                    for v in row.iter_mut() {
                        *v = g.f32();
                    }
                }
                let name = gen_text(g, 20);
                p.name_len = match g.below(12) {
                    0 => 0xFFFF_FFFF,
                    1 => 0x7FFF_FFFF,
                    2 => name.len() as u32 + 1 + g.below(8),
                    3 => name.len().saturating_sub(1) as u32,
                    _ => name.len() as u32,
                };
                let mut b = p.to_bytes();
                if g.chance(8) {
                    b.truncate(g.below(56) as usize);
                }
                b.extend(name);
                if g.below(4) == 0 {
                    b.push(0);
                }
                body.extend(chunk(g, W3D_CHUNK_PLACEHOLDER, false, &b));
            }
            3 => {
                let k = g.below(5);
                let mut b = Vec::new();
                for _ in 0..k * 3 {
                    b.extend(g.f32().to_le_bytes());
                }
                b.extend((0..g.below(3) * g.below(5)).map(|_| g.g.u32() as u8));
                body.extend(chunk(g, W3D_CHUNK_POINTS, false, &b));
            }
            4 => {
                let junk: Vec<u8> = (0..g.below(70)).map(|_| g.g.u32() as u8).collect();
                body.extend(chunk(g, W3D_CHUNK_TRANSFORM_NODE, false, &junk));
            }
            _ => {
                let junk: Vec<u8> = (0..g.below(20)).map(|_| g.g.u32() as u8).collect();
                let id = if g.coin() { W3D_CHUNK_COLLECTION_HEADER } else { g.g.u32() };
                body.extend(chunk(g, id, false, &junk));
            }
        }
    }
    chunk(g, W3D_CHUNK_COLLECTION, true, &body)
}

fn gen_aggregate(g: &mut Gn) -> Vec<u8> {
    let mut body = Vec::new();
    for _ in 0..g.below(6) {
        match g.below(6) {
            0 => {
                let mut h = W3dAggregateHeaderStruct::from_bytes(&[0; 20]);
                h.version = if g.coin() { W3D_CURRENT_AGGREGATE_VERSION } else { g.g.u32() };
                let n = gen_text(g, 16);
                h.name[..n.len()].copy_from_slice(&n);
                let mut b = h.to_bytes();
                if g.chance(8) {
                    b.truncate(g.below(20) as usize);
                }
                body.extend(chunk(g, W3D_CHUNK_AGGREGATE_HEADER, false, &b));
            }
            1 | 2 => {
                let k = g.below(4);
                let mut i = W3dAggregateInfoStruct::from_bytes(&[0; 36]);
                let n = gen_text(g, 32);
                i.base_model_name[..n.len()].copy_from_slice(&n);
                i.subobject_count = match g.below(12) {
                    0 => 0xFFFF_FFFF,
                    1 => k + 1,
                    _ => k,
                };
                let mut b = i.to_bytes();
                for _ in 0..k {
                    let mut s = W3dAggregateSubobjectStruct::from_bytes(&[0; 64]);
                    let n = gen_text(g, 32);
                    s.subobject_name[..n.len()].copy_from_slice(&n);
                    let n = gen_text(g, 32);
                    s.bone_name[..n.len()].copy_from_slice(&n);
                    b.extend(s.to_bytes());
                }
                if g.chance(8) {
                    let n = g.below(b.len() as u32 + 1) as usize;
                    b.truncate(n);
                }
                body.extend(chunk(g, W3D_CHUNK_AGGREGATE_INFO, false, &b));
            }
            3 => {
                let mut m = W3dAggregateMiscInfo::from_bytes(&[0; 20]);
                m.original_class_id = if g.coin() { g.below(30) } else { g.g.u32() };
                m.flags = g.g.u32();
                let mut b = m.to_bytes();
                if g.chance(8) {
                    b.truncate(g.below(20) as usize);
                }
                body.extend(chunk(g, W3D_CHUNK_AGGREGATE_CLASS_INFO, false, &b));
            }
            4 => {
                let b: Vec<u8> = (0..g.below(8)).map(|_| g.g.u32() as u8).collect();
                body.extend(chunk(g, W3D_CHUNK_TEXTURE_REPLACER_INFO, false, &b));
            }
            _ => {
                let junk: Vec<u8> = (0..g.below(20)).map(|_| g.g.u32() as u8).collect();
                let id = g.g.u32();
                body.extend(chunk(g, id, false, &junk));
            }
        }
    }
    chunk(g, W3D_CHUNK_AGGREGATE, true, &body)
}

fn run_collect_cases(name: &str, kind: i32, seed: u64, gen: impl Fn(&mut Gn) -> Vec<u8>) {
    let mut g = Gn { g: Gen::new(seed), clean: false };
    let mut check = Check::new(name);
    let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
    let (mut ok, mut failed, mut large) = (0u64, 0u64, 0u64);
    const CASES: usize = 20_000;
    for case in 0..CASES {
        g.clean = case % 2 == 0;
        let data = gen(&mut g);
        let data = mutate(&mut g, data);
        let rust = match rust_load_collect(&data, kind) {
            Ok(Some(l)) => l,
            Ok(None) => {
                large += 1;
                continue;
            }
            Err(W3dError::Undefined(u)) => {
                *excluded.entry(format!("{u:?}")).or_default() += 1;
                continue;
            }
            Err(e) => {
                *excluded.entry(format!("{e:?}")).or_default() += 1;
                continue;
            }
        };
        match rust.get(2) {
            Some(b'C' | b'A') => ok += 1,
            Some(b'e') => failed += 1,
            _ => {}
        }
        if std::env::var_os("W3D_TRACE").is_some() {
            eprintln!("{name} case {case}: {:02x?}", data);
        }
        let c = cpp_load_collect(&data, kind);
        check.eq(&rust, &c, || format!("case {case}: {:02x?}", data));
    }
    println!(
        "{name}: {CASES} inputs, {ok} loaded, {failed} load errors; excluded (undefined upstream or not ending): {} \
         {excluded:?}; excluded (object name over 2^16 bytes): {large}",
        excluded.values().sum::<u64>()
    );
    check.finish();
}

/// Random collection files, half well-formed and half also with wrong chunk
/// sizes, truncations and byte mutations: header names of 0 to 16
/// characters, `RenderObjectCount`s small, at the allocation limit and over
/// it, object names terminated or not (and empty), placeholders with
/// `name_len` matching the name, shorter, longer than the chunk, `2^31 - 1`
/// or `0xFFFFFFFF`, and structs cut short, points chunks with whole and
/// partial points, transform nodes and unknown chunks. Compared as
/// [`collect_named_cases_match_upstream`]; undefined inputs (counted) are
/// not given to the C++, nor are ones with an object name over 2^16 bytes.
///
/// Predicted before the run: no disagreement; about a fifth to a third
/// excluded as undefined (`UninitialisedRead` from short placeholders,
/// `IndexOutOfRange`, `Allocation`).
///
/// Result (2026-11-05): 16,580 compared, 0 mismatches; 3,256 excluded as
/// undefined (16 %, under the predicted range: 2,299 `Allocation`, 514
/// `IndexOutOfRange`, 440 `UninitialisedRead`, 3 `NonTerminating`), 164 for
/// an object name over 2^16 bytes.
#[test]
fn collection_load_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    run_collect_cases("CollectionDefClass::Load (collect.cpp)", 0, 0xC011, gen_collection);
}

/// Random aggregate files, generated as the collections: headers (names of
/// 0 to 16 characters, cut short), info chunks (base model names of 0 to
/// 32 characters, `SubobjectCount` matching the sub-objects, one more, or
/// `0xFFFFFFFF`; sub-object and bone names of 0 to 32 characters; cut
/// short), class-info chunks (cut short), texture-replacer chunks of 0 to 7
/// bytes, unknown chunks, in any order and number.
///
/// Predicted before the run: no disagreement; roughly a third to a half
/// excluded as undefined (`StringOverrun` from 16-character header names
/// and 32-character bone names, `StringOverflow` from 32-character
/// sub-object names).
///
/// Result (2026-11-05): 17,058 compared, 0 mismatches; 2,942 excluded as
/// undefined (15 %, well under the predicted third to a half: 1,757
/// `StringOverrun`, 1,184 `StringOverflow`, 1 `NonTerminating`).
#[test]
fn aggregate_load_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    run_collect_cases("AggregateDefClass::Load_W3D (agg_def.cpp)", 1, 0xA66, gen_aggregate);
}

// ---------------------------------------------------------------------------
// 10. Box, sphere and ring prototypes (boxrobj.cpp, sphereobj.cpp,
//     ringobj.cpp, prim_anim.h)
// ---------------------------------------------------------------------------

use rts_lockstep_engine::formats::w3d::prim_anim::{Channel, CHUNKID_VARIABLES, VARID_KEY};
use rts_lockstep_engine::formats::w3d::{boxrobj, ringobj, sphereobj};

fn log_channel<T: W3dPod>(log: &mut Log, c: &Channel<T>) {
    p32(log, c.keys.len() as u32);
    for k in &c.keys {
        log.extend(k.value.to_bytes());
        p32(log, k.time.to_bits());
    }
}

/// `ref_w3d_load_prim` of wrap_w3d_prim.cpp, in Rust: kind 0 box, 1 sphere,
/// 2 ring.
fn rust_load_prim(data: &[u8], kind: i32) -> W3dResult<Log> {
    let mut log = Log::new();
    let mut c = ChunkLoad::new(GameFile::new(data).unwrap());
    log.push(b'O');
    if c.open_chunk()? {
        log.push(1);
        match kind {
            0 => {
                let p = boxrobj::load_w3d(&mut c)?;
                log.push(b'B');
                log.extend(p.definition.to_bytes());
                p32(&mut log, p.class_id());
            }
            1 => {
                let p = sphereobj::load_w3d(&mut c)?;
                log.push(b'S');
                log.extend(p.definition.to_bytes());
                p32(&mut log, p.class_id());
                log_channel(&mut log, &p.color_channel);
                log_channel(&mut log, &p.alpha_channel);
                log_channel(&mut log, &p.scale_channel);
                log_channel(&mut log, &p.vector_channel);
            }
            _ => {
                let p = ringobj::load_w3d(&mut c)?;
                log.push(b'R');
                log.extend(p.definition.to_bytes());
                p32(&mut log, p.class_id());
                log_channel(&mut log, &p.color_channel);
                log_channel(&mut log, &p.alpha_channel);
                log_channel(&mut log, &p.inner_scale_channel);
                log_channel(&mut log, &p.outer_scale_channel);
            }
        }
    } else {
        log.push(0);
    }
    log_state(&mut log, &mut c);
    Ok(log)
}

fn cpp_load_prim(data: &[u8], kind: i32) -> Log {
    cpp_log(|log, cap, n| unsafe { cpp::ref_w3d_load_prim(data.as_ptr() as *const _, data.len() as i32, kind, log, cap, n) })
}

/// The layouts of `AlphaVectorStruct`, `W3dSphereStruct` and `W3dRingStruct`
/// (sphereobj.h, ringobj.h) and the sizes of the four `KeyClass` types, as
/// the real headers give them, against the Rust definitions.
///
/// Predicted before the run: identical (sizes 20, 164, 168; keys 16, 8, 12,
/// 24 bytes).
#[test]
fn prim_struct_layouts_match_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let c = cpp_log(|log, cap, n| unsafe { cpp::ref_w3d_prim_layouts(log, cap, n) });
    let mut r = Log::new();
    fn s<T: W3dPod>(r: &mut Log, name: &str, fields: &[W3dField]) {
        r.push(b'S');
        pname(r, name.as_bytes());
        p32(r, T::SIZE as u32);
        for f in fields {
            pname(r, f.name.as_bytes());
            p32(r, f.offset as u32);
            p32(r, f.size as u32);
        }
    }
    s::<AlphaVectorStruct>(&mut r, AlphaVectorStruct::C_NAME, AlphaVectorStruct::FIELDS);
    s::<W3dSphereStruct>(&mut r, W3dSphereStruct::C_NAME, W3dSphereStruct::FIELDS);
    s::<W3dRingStruct>(&mut r, W3dRingStruct::C_NAME, W3dRingStruct::FIELDS);
    for n in [16u32, 8, 12, 24] {
        p32(&mut r, n);
    }
    assert_eq!(r, c, "layouts differ");
}

fn prim_channel(id: u32, keys: &[Vec<u8>]) -> Vec<u8> {
    let mut vars = Vec::new();
    for k in keys {
        vars.push(VARID_KEY as u8);
        vars.push(k.len() as u8);
        vars.extend_from_slice(k);
    }
    hchunk(id, &hchunk(CHUNKID_VARIABLES, &vars))
}

fn prim_key(values: &[f32], time: f32) -> Vec<u8> {
    values.iter().chain([&time]).flat_map(|f| f.to_le_bytes()).collect()
}

/// The named cases of the `boxrobj`, `sphereobj`, `ringobj` and `prim_anim`
/// unit tests, as whole files: (kind, name, file).
fn prim_named_cases() -> Vec<(i32, &'static str, Vec<u8>)> {
    let mut v = Vec::new();
    let mut b = W3dBoxStruct::from_bytes(&[0; 68]);
    b.version = W3D_BOX_CURRENT_VERSION;
    b.attributes = W3D_BOX_ATTRIBUTE_ORIENTED;
    b.name[..8].copy_from_slice(b"CONT.BOX");
    b.center = W3dVectorStruct { x: 1.0, y: 2.0, z: 3.0 };
    b.extent = W3dVectorStruct { x: 4.0, y: 5.0, z: 6.0 };
    let mut body = b.to_bytes();
    body.extend([1, 2, 3]);
    v.push((0, "box: oriented, trailing bytes", hchunk(W3D_CHUNK_BOX, &body)));
    b.attributes = W3D_BOX_ATTRIBUTE_ALIGNED;
    b.name = [b'N'; 32];
    v.push((0, "box: aligned, 32-character name", hchunk(W3D_CHUNK_BOX, &b.to_bytes())));

    let mut s = W3dSphereStruct::from_bytes(&[0; 164]);
    s.version = 0x0001_0000;
    s.attributes = 3;
    s.name[..11].copy_from_slice(b"CONT.SPHERE");
    s.extent = W3dVectorStruct { x: 2.0, y: 2.0, z: 2.0 };
    s.anim_duration = 1.5;
    s.default_alpha = 0.5;
    s.default_vector = AlphaVectorStruct { angle: [0.0, 0.0, 0.0, 1.0], intensity: 2.0 };
    s.texture_name[..7].copy_from_slice(b"FX1.TGA");
    let mut body = hchunk(sphereobj::CHUNKID_SPHERE_DEF, &s.to_bytes());
    body.extend(prim_channel(sphereobj::CHUNKID_COLOR_CHANNEL, &[prim_key(&[1.0, 0.0, 0.0], 0.0), prim_key(&[0.0, 1.0, 0.0], 1.0)]));
    body.extend(prim_channel(sphereobj::CHUNKID_ALPHA_CHANNEL, &[prim_key(&[0.25], 0.5)]));
    body.extend(hchunk(99, &[1, 2, 3]));
    body.extend(prim_channel(sphereobj::CHUNKID_SCALE_CHANNEL, &[prim_key(&[2.0, 2.0, 2.0], 0.0)]));
    body.extend(prim_channel(sphereobj::CHUNKID_VECTOR_CHANNEL, &[prim_key(&[0.0, 0.0, 0.0, 1.0, 3.0], 0.75)]));
    v.push((1, "sphere: definition and four channels", hchunk(W3D_CHUNK_SPHERE, &body)));
    v.push((1, "sphere: empty", hchunk(W3D_CHUNK_SPHERE, &[])));
    let mut body = hchunk(sphereobj::CHUNKID_SPHERE_DEF, &s.to_bytes());
    body.extend(hchunk(sphereobj::CHUNKID_SPHERE_DEF, &[0xEE; 100]));
    body.extend(prim_channel(sphereobj::CHUNKID_COLOR_CHANNEL, &[prim_key(&[1.0, 1.0, 1.0], 0.0)]));
    body.extend(prim_channel(sphereobj::CHUNKID_COLOR_CHANNEL, &[prim_key(&[5.0, 5.0, 5.0], 2.0)]));
    v.push((1, "sphere: short second definition, repeated channel", hchunk(W3D_CHUNK_SPHERE, &body)));
    let mut f = hchunk(W3D_CHUNK_SPHERE, &hchunk(sphereobj::CHUNKID_SPHERE_DEF, &s.to_bytes()));
    f.truncate(16 + 10);
    v.push((1, "sphere: definition cut by the end of the file", f));
    let mut vars = vec![VARID_KEY as u8, 4];
    vars.extend(9f32.to_le_bytes());
    let short = hchunk(sphereobj::CHUNKID_VECTOR_CHANNEL, &hchunk(CHUNKID_VARIABLES, &vars));
    v.push((1, "sphere: vector key in a short micro-chunk", hchunk(W3D_CHUNK_SPHERE, &short)));
    let mut ch = prim_channel(sphereobj::CHUNKID_COLOR_CHANNEL, &[prim_key(&[1.0, 2.0, 3.0], 0.5)]);
    let mut extra = vec![2u8, 3, 0, 0, 0, VARID_KEY as u8, 16];
    extra.extend(prim_key(&[4.0, 5.0, 6.0], 1.0));
    let more = hchunk(CHUNKID_VARIABLES, &extra);
    let len = u32::from_le_bytes(ch[4..8].try_into().unwrap()) + more.len() as u32;
    ch[4..8].copy_from_slice(&len.to_le_bytes());
    ch.extend(more);
    v.push((1, "sphere: two variables chunks, unknown micro-chunk", hchunk(W3D_CHUNK_SPHERE, &ch)));

    let mut r = W3dRingStruct::from_bytes(&[0; 168]);
    r.version = 0x0001_0000;
    r.name[..9].copy_from_slice(b"CONT.RING");
    r.inner_extent = [0.5, 0.5];
    r.outer_extent = [1.5, 1.5];
    r.texture_tile_count = -2;
    let mut body = hchunk(ringobj::CHUNKID_RING_DEF, &r.to_bytes());
    body.extend(prim_channel(ringobj::CHUNKID_INNER_SCALE_CHANNEL, &[prim_key(&[1.0, 1.0], 0.0), prim_key(&[2.0, 2.0], 1.0)]));
    body.extend(prim_channel(ringobj::CHUNKID_OUTER_SCALE_CHANNEL, &[prim_key(&[3.0, 4.0], 0.5)]));
    body.extend(prim_channel(ringobj::CHUNKID_COLOR_CHANNEL, &[prim_key(&[1.0, 1.0, 1.0], 0.0)]));
    body.extend(prim_channel(ringobj::CHUNKID_ALPHA_CHANNEL, &[prim_key(&[1.0], 0.0)]));
    v.push((2, "ring: definition and four channels", hchunk(W3D_CHUNK_RING, &body)));
    v.push((2, "ring: empty", hchunk(W3D_CHUNK_RING, &[])));
    v
}

/// The named cases: the files of the `boxrobj`, `sphereobj`, `ringobj` and
/// `prim_anim` unit tests (an oriented box with trailing bytes, an aligned
/// box with a 32-character name; a sphere with its definition and four
/// channels, an empty sphere, a short second definition and a repeated
/// channel, a definition cut by the end of the file, a vector key in a
/// short micro-chunk, two variables chunks with an unknown micro-chunk; a
/// ring with its definition and four channels, an empty ring). The cases
/// those tests show undefined (a short box, short `float`/`Vector2` keys)
/// are not given to the C++.
///
/// Compared byte for byte: the definition's bytes, `Get_Class_ID()`, each
/// channel's keys (value and time bytes), the chunk depth and the file
/// position the loader leaves.
///
/// Predicted before the run: all ~~11~~ **10** (corrected after the run: a
/// miscount of the list) agree, including the class ids counted
/// by hand from rendobj.h (26, 27, 29, 30), the definition left as it was by
/// the short chunk, the 10 bytes copied from the cut file, and the default
/// `AlphaVectorStruct` key (identity, intensity 1, time 0).
#[test]
fn prim_named_cases_match_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut check = Check::new("box/sphere/ring Load_W3D, named cases");
    let cases = prim_named_cases();
    for (kind, name, data) in &cases {
        let rust = rust_load_prim(data, *kind).unwrap_or_else(|e| panic!("{name}: Rust undefined: {e:?}"));
        let c = cpp_load_prim(data, *kind);
        check.eq(&rust, &c, || format!("{name}: {:02x?}", data));
    }
    println!("box/sphere/ring named cases: {} compared", cases.len());
    check.finish();
}

fn gen_prim_channel(g: &mut Gn, id: u32, n: usize) -> Vec<u8> {
    let mut body = Vec::new();
    for _ in 0..1 + g.below(2) {
        let mut vars = Vec::new();
        for _ in 0..g.below(4) {
            let mut k: Vec<u8> = Vec::new();
            for _ in 0..=n {
                k.extend(g.f32().to_le_bytes());
            }
            let id = if g.below(10) == 0 { g.below(4) as u8 } else { VARID_KEY as u8 };
            let mut len = k.len();
            if g.chance(8) {
                len = g.below(len as u32 + 3) as usize;
            }
            k.resize(len, 0xA5);
            vars.push(id);
            vars.push(len as u8);
            vars.extend(k);
        }
        let cid = if g.below(10) == 0 { g.g.u32() } else { CHUNKID_VARIABLES };
        body.extend(chunk(g, cid, true, &vars));
    }
    chunk(g, id, true, &body)
}

fn gen_prim(g: &mut Gn, kind: i32) -> Vec<u8> {
    let rand_bytes = |g: &mut Gn, n: usize| -> Vec<u8> {
        let mut b: Vec<u8> = (0..n).map(|_| g.g.u32() as u8).collect();
        // mostly readable floats
        for c in b.chunks_mut(4) {
            if c.len() == 4 && g.coin() {
                c.copy_from_slice(&g.f32().to_le_bytes());
            }
        }
        b
    };
    if kind == 0 {
        let mut b = rand_bytes(g, 68);
        b[4..8].copy_from_slice(&g.below(4).to_le_bytes());
        if g.chance(8) {
            b.truncate(g.below(68) as usize);
        }
        if g.below(4) == 0 {
            let n = g.below(10) as usize;
            b.extend(rand_bytes(g, n));
        }
        return chunk(g, W3D_CHUNK_BOX, false, &b);
    }
    let def_size = if kind == 1 { 164 } else { 168 };
    // value lengths in floats: color 3, alpha 1, then scale 3 / vector 5 or 2, 2
    let lens: [usize; 4] = if kind == 1 { [3, 1, 3, 5] } else { [3, 1, 2, 2] };
    let mut body = Vec::new();
    for _ in 0..g.below(7) {
        let id = g.below(7);
        match id {
            1 => {
                let mut d = rand_bytes(g, def_size);
                if g.below(6) == 0 {
                    d.truncate(g.below(def_size as u32) as usize);
                }
                body.extend(chunk(g, 1, false, &d));
            }
            2..=5 => body.extend(gen_prim_channel(g, id, lens[id as usize - 2])),
            _ => {
                let n = g.below(20) as usize;
                let junk = rand_bytes(g, n);
                body.extend(chunk(g, id, false, &junk));
            }
        }
    }
    chunk(g, if kind == 1 { W3D_CHUNK_SPHERE } else { W3D_CHUNK_RING }, true, &body)
}

fn run_prim_cases(name: &str, kind: i32, seed: u64) {
    let mut g = Gn { g: Gen::new(seed), clean: false };
    let mut check = Check::new(name);
    let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
    let mut compared = 0u64;
    const CASES: usize = 20_000;
    for case in 0..CASES {
        g.clean = case % 2 == 0;
        let data = gen_prim(&mut g, kind);
        let data = mutate(&mut g, data);
        let rust = match rust_load_prim(&data, kind) {
            Ok(l) => l,
            Err(W3dError::Undefined(u)) => {
                *excluded.entry(format!("{u:?}")).or_default() += 1;
                continue;
            }
            Err(e) => {
                *excluded.entry(format!("{e:?}")).or_default() += 1;
                continue;
            }
        };
        if std::env::var_os("W3D_TRACE").is_some() {
            eprintln!("{name} case {case}: {:02x?}", data);
        }
        let c = cpp_load_prim(&data, kind);
        compared += 1;
        check.eq(&rust, &c, || format!("case {case}: {:02x?}", data));
    }
    println!(
        "{name}: {CASES} inputs, {compared} compared; excluded (undefined upstream or not ending): {} {excluded:?}",
        excluded.values().sum::<u64>()
    );
    check.finish();
}

/// Random box files, half well-formed and half also with wrong chunk sizes,
/// truncations and byte mutations: the struct with random bytes (mostly
/// plain floats) and attributes 0 to 3, sometimes cut short or followed by
/// extra bytes. Compared as [`prim_named_cases_match_upstream`]; undefined
/// inputs (a short struct) are not given to the C++ (counted).
///
/// Predicted before the run: no disagreement; roughly a tenth to a fifth
/// excluded (`UninitialisedRead`).
///
/// Result (2026-11-05): 17,302 compared, 0 mismatches; 2,698 (13 %)
/// excluded, all `UninitialisedRead`.
#[test]
fn box_load_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    run_prim_cases("BoxLoaderClass::Load_W3D (boxrobj.cpp)", 0, 0xB0C5);
}

/// Random sphere files, generated as the boxes: definition chunks (random
/// bytes, sometimes short), channel chunks of each kind holding one or two
/// variables chunks (sometimes of another id) of zero to three key
/// micro-chunks (sometimes of another id, sometimes of the wrong length),
/// and unknown chunks, in any order and number.
///
/// Predicted before the run: no disagreement; a third or fewer excluded
/// (`UninitialisedRead` from short `float`/`Vector3` keys).
///
/// Result (2026-11-05): 18,458 compared, 0 mismatches; 1,542 (8 %) excluded:
/// 1,540 `UninitialisedRead`, 2 `NonTerminating`.
#[test]
fn sphere_load_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    run_prim_cases("SphereLoaderClass::Load_W3D (sphereobj.cpp)", 1, 0x5F3);
}

/// Random ring files, generated as the spheres (with `Vector2` scale
/// channels).
///
/// Predicted before the run: as for the spheres.
///
/// Result (2026-11-05): 18,029 compared, 0 mismatches; 1,971 (10 %) excluded:
/// 1,967 `UninitialisedRead`, 4 `NonTerminating`.
#[test]
fn ring_load_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    run_prim_cases("RingLoaderClass::Load_W3D (ringobj.cpp)", 2, 0x219);
}

// ---------------------------------------------------------------------------
// 11. Morph animations (hmorphanim.cpp)
// ---------------------------------------------------------------------------

use rts_lockstep_engine::formats::w3d::hmorphanim;

/// `ref_w3d_load_morph` of wrap_w3d_morph.cpp, in Rust. `Ok(None)` when a
/// channel count or a channel's key count is over 2^16 (kept out of the
/// comparison: the C++ would allocate and log every one).
fn rust_load_morph(data: &[u8], trees: &Trees) -> W3dResult<Option<Log>> {
    let mut log = Log::new();
    let mut c = ChunkLoad::new(GameFile::new(data).unwrap());
    let mut lookup = |n: &[u8; W3D_NAME_LEN]| trees.iter().find(|t| &t.0 == n).map(|t| t.1);
    log.push(b'O');
    if c.open_chunk()? {
        log.push(1);
        match hmorphanim::load_w3d(&mut c, &mut lookup)? {
            Some(a) => {
                if a.channel_count > 1 << 16 || a.channels.iter().any(|ch| ch.keys.len() > 1 << 16) {
                    return Ok(None);
                }
                log.extend_from_slice(&[b'M', 0]);
                log.extend_from_slice(&a.name);
                log.extend_from_slice(&a.anim_name);
                log.extend_from_slice(&a.hierarchy_name);
                for v in [a.frame_count as u32, a.frame_rate.to_bits(), a.channel_count as u32, a.num_nodes as u32] {
                    p32(&mut log, v);
                }
                for v in &a.pivot_channel {
                    p32(&mut log, *v);
                }
                p32(&mut log, a.channels.len() as u32);
                for ch in &a.channels {
                    let n = ch.pose_name.as_deref().unwrap();
                    p32(&mut log, n.len() as u32);
                    log.extend_from_slice(n);
                }
                for i in 0..a.channel_count.max(0) as usize {
                    let k = a.channel(i).unwrap().keys;
                    p32(&mut log, k.len() as u32);
                    for j in 0..k.len() {
                        let key = k.get(j).unwrap();
                        p32(&mut log, key.morph_frame);
                        p32(&mut log, key.pose_frame);
                    }
                }
            }
            // LOAD_ERROR
            None => log.extend_from_slice(&[b'M', 1]),
        }
    } else {
        log.push(0);
    }
    log_state(&mut log, &mut c);
    Ok(Some(log))
}

fn cpp_load_morph(data: &[u8]) -> Log {
    cpp_log(|log, cap, n| unsafe { cpp::ref_w3d_load_morph(data.as_ptr() as *const _, data.len() as i32, log, cap, n) })
}

fn morph_header(name: &[u8], hier: &[u8], frames: u32, channels: u32) -> Vec<u8> {
    let mut h = W3dMorphAnimHeaderStruct::from_bytes(&[0; 48]);
    h.version = W3D_CURRENT_MORPH_HANIM_VERSION;
    h.name[..name.len()].copy_from_slice(name);
    h.hierarchy_name[..hier.len()].copy_from_slice(hier);
    h.frame_count = frames;
    h.frame_rate = 30.0;
    h.channel_count = channels;
    hchunk(W3D_CHUNK_MORPHANIM_HEADER, &h.to_bytes())
}

fn morph_keys(k: &[(u32, u32)]) -> Vec<u8> {
    let b: Vec<u8> = k.iter().flat_map(|(m, p)| m.to_le_bytes().into_iter().chain(p.to_le_bytes())).collect();
    hchunk(W3D_CHUNK_MORPHANIM_KEYDATA, &b)
}

fn morph_channel(pose: &[u8], k: &[(u32, u32)]) -> Vec<u8> {
    let mut b = hchunk(W3D_CHUNK_MORPHANIM_POSENAME, pose);
    b.extend(morph_keys(k));
    hchunk(W3D_CHUNK_MORPHANIM_CHANNEL, &b)
}

fn morph_pivots(v: &[u32]) -> Vec<u8> {
    let b: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    hchunk(W3D_CHUNK_MORPHANIM_PIVOTCHANNELDATA, &b)
}

/// The named cases of `formats::w3d::hmorphanim`'s unit tests, as whole
/// files, on the registered hierarchy `SKEL` (6 pivots) instead of the unit
/// tests' `SKL` (3).
fn morph_named_cases() -> Vec<(&'static str, Vec<u8>)> {
    let anim = |body: Vec<u8>| hchunk(W3D_CHUNK_MORPH_ANIMATION, &body);
    let mut v = Vec::new();
    let mut b = morph_header(b"FACE", b"SKEL", 60, 2);
    b.extend(morph_channel(b"SKEL.PHONEMES\0", &[(0, 0), (10, 4)]));
    b.extend(morph_channel(b"SKEL.EXPR\0", &[(0, 1)]));
    b.extend(morph_pivots(&[0, 1, 1, 0, 1, 1]));
    v.push(("two channels and pivot data", anim(b)));

    v.push(("unknown hierarchy", anim(morph_header(b"F", b"NOPE", 1, 0))));
    let mut b = morph_header(b"ABCDEFGHIJKLMNOP", b"SKEL", 1, 3);
    b.extend(morph_channel(b"P\0", &[(0, 0)]));
    b.extend(morph_pivots(&[7]));
    v.push(("16-character name, unread channels, short pivot data", anim(b)));
    v.push(("16-character hierarchy name", anim(morph_header(b"ANIM", b"SIXTEENCHARSNAME", 1, 0))));

    let mut kb = morph_keys(&[(1, 2), (3, 4), (5, 6)]);
    kb[4] = 40;
    kb.truncate(8 + 16 + 2);
    let mut b = morph_header(b"F", b"SKEL", 1, 1);
    let mut ch = hchunk(W3D_CHUNK_MORPHANIM_POSENAME, b"P\0");
    ch.extend(kb);
    let mut chc = W3D_CHUNK_MORPHANIM_CHANNEL.to_le_bytes().to_vec();
    chc.extend_from_slice(&(ch.len() as u32 + 22).to_le_bytes());
    chc.extend(ch);
    b.extend(chc);
    let mut f = W3D_CHUNK_MORPH_ANIMATION.to_le_bytes().to_vec();
    f.extend_from_slice(&(b.len() as u32 + 22).to_le_bytes());
    f.extend(b);
    v.push(("key reads failing after the second: stale key repeated", f));

    let mut b = morph_header(b"F", b"SKEL", 1, 1);
    b.extend(hchunk(W3D_CHUNK_MORPHANIM_CHANNEL, &[]));
    b.extend(morph_pivots(&[0, 1, 1]));
    let mut f = anim(b);
    f.extend(hchunk(0x1234, &[9; 4]));
    v.push(("empty channel chunk: misdirected closes", f));

    let mut b = morph_header(b"F", b"SKEL", 1, 1);
    b.extend(morph_channel(b"", &[(2, 3)]));
    v.push(("empty pose name", anim(b)));
    v
}

/// The named cases: the files of `formats::w3d::hmorphanim`'s unit tests (two
/// channels and pivot data; an unknown hierarchy; a 16-character name with
/// unread channels and a short pivot chunk; a key chunk whose reads fail
/// after the second key; a channel chunk with no sub-chunks), plus a
/// 16-character hierarchy name (registered as `SIXTEENCHARSNAME`) and an
/// empty pose name. The undefined cases
/// of those tests are not given to the C++.
///
/// Compared byte for byte: the load result, the three names, `FrameCount`,
/// `FrameRate`, `ChannelCount`, `NumNodes`, `PivotChannel`, the pose names
/// asked of the asset manager in order, every channel's keys, the chunk
/// depth and the file position.
///
/// Predicted before the run: all 7 agree, in particular the repeated stale
/// key (5,4), the empty channel chunk taking the pivot chunk as its keys and
/// ending at depth 0, and `SIXTEENCHARSNAME.ANIM` (the hierarchy name
/// running into the zero `FrameCount`).
#[test]
fn morph_named_cases_match_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let trees = register_trees(&tree_names());
    let mut check = Check::new("HMorphAnimClass::Load_W3D (hmorphanim.cpp), named cases");
    let cases = morph_named_cases();
    for (name, data) in &cases {
        let rust =
            rust_load_morph(data, &trees).unwrap_or_else(|e| panic!("{name}: Rust undefined: {e:?}")).expect("small counts");
        let c = cpp_load_morph(data);
        check.eq(&rust, &c, || format!("{name}: {:02x?}", data));
    }
    println!("morph named cases: {} compared", cases.len());
    check.finish();
}

fn gen_morph(g: &mut Gn, trees: &Trees) -> Vec<u8> {
    let mut h = W3dMorphAnimHeaderStruct::from_bytes(&[0; 48]);
    h.version = if g.coin() { W3D_CURRENT_MORPH_HANIM_VERSION } else { g.g.u32() };
    let n = gen_text(g, 16);
    h.name[..n.len()].copy_from_slice(&n);
    h.hierarchy_name = if g.below(8) == 0 { g.name() } else { trees[g.below(trees.len() as u32) as usize].0 };
    h.frame_count = g.g.u32();
    h.frame_rate = g.f32();
    let chans = g.below(4);
    h.channel_count = match g.below(16) {
        0 => 0xFFFF_FFFF,
        1 => 0x0800_0000,
        2 => chans.saturating_sub(1),
        3 => 1000 + g.below(1000),
        _ => chans,
    };
    let mut body = if g.chance(20) {
        Vec::new()
    } else {
        let hb = h.to_bytes();
        let hb = if g.chance(10) { hb[..g.below(48) as usize].to_vec() } else { hb };
        let id = if g.chance(10) { g.g.u32() } else { W3D_CHUNK_MORPHANIM_HEADER };
        chunk(g, id, false, &hb)
    };
    for _ in 0..chans + g.below(3) {
        match g.below(5) {
            0..=2 => {
                let mut cb = Vec::new();
                if g.below(12) != 0 {
                    let mut name = gen_text(g, 20);
                    if g.below(4) != 0 {
                        name.push(0);
                    }
                    cb.extend(chunk(g, W3D_CHUNK_MORPHANIM_POSENAME, false, &name));
                }
                if g.below(12) != 0 {
                    let mut kb = Vec::new();
                    for _ in 0..g.below(5) {
                        kb.extend(g.below(100).to_le_bytes());
                        kb.extend(g.below(10).to_le_bytes());
                    }
                    if g.below(5) == 0 {
                        kb.extend((0..g.below(8)).map(|_| g.g.u32() as u8));
                    }
                    cb.extend(chunk(g, W3D_CHUNK_MORPHANIM_KEYDATA, false, &kb));
                }
                body.extend(chunk(g, W3D_CHUNK_MORPHANIM_CHANNEL, true, &cb));
            }
            3 => {
                let k = g.below(9);
                let mut pb: Vec<u8> = (0..k).flat_map(|_| g.below(4).to_le_bytes()).collect();
                if g.below(4) == 0 {
                    pb.extend((0..g.below(4)).map(|_| g.g.u32() as u8));
                }
                body.extend(chunk(g, W3D_CHUNK_MORPHANIM_PIVOTCHANNELDATA, false, &pb));
            }
            _ => {
                let junk: Vec<u8> = (0..g.below(20)).map(|_| g.g.u32() as u8).collect();
                let id = g.g.u32();
                body.extend(chunk(g, id, false, &junk));
            }
        }
    }
    let mut f = chunk(g, W3D_CHUNK_MORPH_ANIMATION, true, &body);
    if g.below(6) == 0 {
        // a following top-level chunk, reached by misdirected closes
        let junk: Vec<u8> = (0..g.below(12)).map(|_| g.g.u32() as u8).collect();
        f.extend(chunk(g, W3D_CHUNK_MORPHANIM_PIVOTCHANNELDATA, false, &junk));
    }
    f
}

/// Random morph animation files, half well-formed and half also with wrong
/// chunk sizes, truncations and byte mutations: headers (names of 0 to 16
/// characters, the registered hierarchies `SKEL` (6 pivots), `BIG`
/// (70,000), `NONE` (0), `NEG` (negative), `SIXTEENCHARSNAME` (3), `P1` (1)
/// or an unregistered name; `ChannelCount` matching the channel chunks,
/// fewer, 1000 or more, too large or negative; header chunks of another id or
/// cut short, or missing), channel chunks with or without their pose-name
/// and key sub-chunks (pose names terminated or not, key data with partial
/// keys), pivot-channel chunks of 0 to 8 entries plus stray bytes, unknown
/// chunks, and sometimes a top-level chunk after the animation. Compared as
/// [`morph_named_cases_match_upstream`]; undefined inputs (counted) are not
/// given to the C++, nor are ones with over 2^16 channels or keys.
///
/// Predicted before the run: no disagreement; a third to a half excluded as
/// undefined (`UninitialisedRead` from missing or short headers and first
/// keys, `IndexOutOfRange` from extra channels, `BufferOverrun` from pivot
/// data longer than the pivots, `StringOverrun`, `Allocation` from `NEG`
/// and huge channel counts, and the chunk reader's `NoChunkOpen` after
/// misdirected closes).
///
/// Result (2026-11-05): 6,409 compared (3,924 loaded, 2,365 load errors,
/// the rest with no chunk to open), 0 mismatches. Excluded as undefined:
/// 13,492 (67 %, more than predicted): 4,175 `Allocation`, 3,464
/// `UninitialisedRead`, 2,105 `StringOverrun`, 1,696 `BufferOverrun`, 1,530
/// `IndexOutOfRange`, 518 `StringOverflow`, 4 `NonTerminating`; no
/// `NoChunkOpen` (predicted, not reached). 99 more over 2^16 channels or
/// keys.
#[test]
fn morph_load_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let trees = register_trees(&tree_names());
    let mut g = Gn { g: Gen::new(0x302F), clean: false };
    let mut check = Check::new("HMorphAnimClass::Load_W3D (hmorphanim.cpp)");
    let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
    let (mut ok, mut failed, mut large) = (0u64, 0u64, 0u64);
    const CASES: usize = 20_000;
    for case in 0..CASES {
        g.clean = case % 2 == 0;
        let data = gen_morph(&mut g, &trees);
        let data = mutate(&mut g, data);
        let rust = match rust_load_morph(&data, &trees) {
            Ok(Some(l)) => l,
            Ok(None) => {
                large += 1;
                continue;
            }
            Err(W3dError::Undefined(u)) => {
                *excluded.entry(format!("{u:?}")).or_default() += 1;
                continue;
            }
            Err(e) => {
                *excluded.entry(format!("{e:?}")).or_default() += 1;
                continue;
            }
        };
        match rust.get(3) {
            Some(0) if rust[2] == b'M' => ok += 1,
            Some(1) if rust[2] == b'M' => failed += 1,
            _ => {}
        }
        if std::env::var_os("W3D_TRACE").is_some() {
            eprintln!("morph case {case}: {:02x?}", data);
        }
        let c = cpp_load_morph(&data);
        check.eq(&rust, &c, || format!("case {case}: {:02x?}", data));
    }
    println!(
        "HMorphAnimClass::Load_W3D: {CASES} inputs, {ok} loaded, {failed} load errors; excluded (undefined upstream or \
         not ending): {} {excluded:?}; excluded (over 2^16 channels or keys): {large}",
        excluded.values().sum::<u64>()
    );
    check.finish();
}

// ---------------------------------------------------------------------------
// 12. The asset manager (assetmgr.cpp, htreemgr.cpp, hanimmgr.cpp, hash.cpp)
// ---------------------------------------------------------------------------

use rts_lockstep_engine::common::file_system::{FileSystem, MemoryDevice};
use rts_lockstep_engine::formats::w3d::assetmgr::{AssetManager, W3dFileFactory};

/// One operation of `ref_assetmgr_op` (wrap_w3d_assetmgr.cpp).
#[derive(Clone, Debug)]
enum AmOp {
    /// `Load_3D_Assets(FileClass &)` on these bytes.
    Bytes(Vec<u8>),
    /// `Load_3D_Assets(name)`.
    File(Vec<u8>),
    /// `Create_Render_Obj(name)`: which prototype.
    Create(Vec<u8>),
    /// `Get_HTree(name)`.
    HTree(Vec<u8>),
    /// `Get_HAnim(name)`, then `Is_Missing(name)`.
    HAnim(Vec<u8>),
    /// `Render_Obj_Exists(name)`.
    Exists(Vec<u8>),
    /// `Free_Assets()`.
    Free,
    /// `Remove_Prototype(name)`.
    Remove(Vec<u8>),
    /// `Set_WW3D_Load_On_Demand(on)`.
    OnDemand(bool),
    /// The prototypes, hierarchies and animations, in their orders.
    State,
}

impl AmOp {
    fn code_arg(&self) -> (i32, Vec<u8>) {
        match self {
            AmOp::Bytes(b) => (0, b.clone()),
            AmOp::File(n) => (1, n.clone()),
            AmOp::Create(n) => (2, n.clone()),
            AmOp::HTree(n) => (3, n.clone()),
            AmOp::HAnim(n) => (4, n.clone()),
            AmOp::Exists(n) => (5, n.clone()),
            AmOp::Free => (6, Vec::new()),
            AmOp::Remove(n) => (7, n.clone()),
            AmOp::OnDemand(on) => (8, vec![*on as u8]),
            AmOp::State => (9, Vec::new()),
        }
    }
}

/// A case: which manager, the user data directory (`None`: no
/// `TheGlobalData`), the files of `TheFileSystem`, the operations.
#[derive(Clone, Debug)]
struct AmCase {
    game: bool,
    user_data: Option<Vec<u8>>,
    files: Vec<(Vec<u8>, Vec<u8>)>,
    ops: Vec<AmOp>,
}

fn am_pstr(log: &mut Log, s: &[u8]) {
    p32(log, s.len() as u32);
    log.extend_from_slice(s);
}

/// `ref_assetmgr_op`, in Rust: the same log from `formats::w3d::assetmgr`.
fn rust_am_op(m: &mut AssetManager, fs: &FileSystem<MemoryDevice>, op: &AmOp) -> W3dResult<Log> {
    let mut log = Log::new();
    match op {
        AmOp::Bytes(b) => {
            log.push(b'B');
            log.push(m.load_3d_assets(fs, b)? as u8);
        }
        AmOp::File(n) => {
            log.push(b'L');
            log.push(m.load_3d_assets_file(fs, n)? as u8);
        }
        AmOp::Create(n) => {
            log.push(b'R');
            match m.create_render_obj(fs, n)? {
                Some(p) => {
                    let name = p.name()?;
                    let id = p.class_id();
                    log.push(1);
                    am_pstr(&mut log, &name);
                    p32(&mut log, id);
                }
                None => log.push(0),
            }
        }
        AmOp::HTree(n) => {
            log.push(b'H');
            let found = m.get_htree(fs, n)?.map(|t| t.num_pivots);
            match found {
                Some(np) => {
                    let id = m.get_tree_id(n).expect("found");
                    let name = m.htrees().nth(id).expect("tree").0.to_vec();
                    log.push(1);
                    am_pstr(&mut log, &name);
                    p32(&mut log, np as u32);
                }
                None => log.push(0),
            }
        }
        AmOp::HAnim(n) => {
            log.push(b'N');
            match m.get_hanim(fs, n)? {
                Some(a) => {
                    let name = a.name().to_vec();
                    log.push(1);
                    am_pstr(&mut log, &name);
                }
                None => log.push(0),
            }
            log.push(m.is_missing(n) as u8);
        }
        AmOp::Exists(n) => {
            log.push(b'E');
            log.push(m.render_obj_exists(n) as u8);
        }
        AmOp::Free => {
            log.push(b'F');
            m.free_assets();
        }
        AmOp::Remove(n) => {
            log.push(b'X');
            m.remove_prototype(n)?;
        }
        AmOp::OnDemand(on) => {
            log.push(b'D');
            m.load_on_demand = *on;
        }
        AmOp::State => {
            log.push(b'P');
            let protos: Vec<(Vec<u8>, u32)> = m.prototypes().map(|(n, p)| (n.to_vec(), p.class_id())).collect();
            p32(&mut log, protos.len() as u32);
            for (n, id) in protos {
                am_pstr(&mut log, &n);
                p32(&mut log, id);
            }
            log.push(b'T');
            let trees: Vec<(Vec<u8>, i32)> = m.htrees().map(|(n, t)| (n.to_vec(), t.num_pivots)).collect();
            p32(&mut log, trees.len() as u32);
            for (n, np) in trees {
                am_pstr(&mut log, &n);
                p32(&mut log, np as u32);
            }
            log.push(b'A');
            let anims: Vec<Vec<u8>> = m.anim_names().into_iter().map(|n| n.to_vec()).collect();
            p32(&mut log, anims.len() as u32);
            for n in anims {
                am_pstr(&mut log, &n);
            }
        }
    }
    Ok(log)
}

/// The whole case in Rust: the logs of every operation, or the first error.
fn rust_am_case(c: &AmCase) -> W3dResult<Vec<Log>> {
    let mut dev = MemoryDevice::new();
    for (p, d) in &c.files {
        dev.insert(p, d.clone());
    }
    let fs = FileSystem::new(dev);
    let factory = W3dFileFactory { language: b"english".to_vec(), user_data_path: c.user_data.clone() };
    let mut m = if c.game {
        AssetManager::new_game(factory)
    } else {
        let mut m = AssetManager::new();
        m.file_factory = factory;
        m
    };
    c.ops.iter().map(|op| rust_am_op(&mut m, &fs, op)).collect()
}

fn cpp_am_case(c: &AmCase) -> Vec<Log> {
    let ud = c.user_data.as_ref().map(|u| {
        let mut v = u.clone();
        v.push(0);
        v
    });
    unsafe { cpp::ref_assetmgr_reset(c.game as i32, ud.as_ref().map_or(std::ptr::null(), |v| v.as_ptr() as *const _)) };
    for (p, d) in &c.files {
        let mut p = p.clone();
        p.push(0);
        unsafe { cpp::ref_assetmgr_add_file(p.as_ptr() as *const _, d.as_ptr() as *const _, d.len() as i32) };
    }
    c.ops
        .iter()
        .map(|op| {
            let (code, mut arg) = op.code_arg();
            let len = arg.len() as i32;
            arg.push(0);
            cpp_log(|log, cap, n| unsafe { cpp::ref_assetmgr_op(code, arg.as_ptr() as *const _, len, log, cap, n) })
        })
        .collect()
}

fn am_name16(s: &[u8]) -> [u8; W3D_NAME_LEN] {
    let mut n = [0u8; W3D_NAME_LEN];
    let k = s.len().min(W3D_NAME_LEN);
    n[..k].copy_from_slice(&s[..k]);
    n
}

fn am_sub(id: u32, body: &[u8]) -> Vec<u8> {
    let mut v = id.to_le_bytes().to_vec();
    v.extend_from_slice(&(body.len() as u32 | 0x8000_0000).to_le_bytes());
    v.extend_from_slice(body);
    v
}

fn am_mesh(container: &[u8], name: &[u8]) -> Vec<u8> {
    let mut h = W3dMeshHeader3Struct::from_bytes(&[0; W3dMeshHeader3Struct::SIZE]);
    h.version = w3d_make_version(4, 2);
    h.mesh_name = am_name16(name);
    h.container_name = am_name16(container);
    am_sub(W3D_CHUNK_MESH, &hchunk(W3D_CHUNK_MESH_HEADER3, &h.to_bytes()))
}

fn am_htree(name: &[u8], n: u32) -> Vec<u8> {
    let mut h = W3dHierarchyStruct::from_bytes(&[0; W3dHierarchyStruct::SIZE]);
    h.version = w3d_make_version(4, 1);
    h.name = am_name16(name);
    h.num_pivots = n;
    let mut body = hchunk(W3D_CHUNK_HIERARCHY_HEADER, &h.to_bytes());
    let mut pivots = Vec::new();
    for _ in 0..n {
        let mut p = W3dPivotStruct::from_bytes(&[0; W3dPivotStruct::SIZE]);
        p.parent_idx = 0xFFFF_FFFF;
        p.rotation.q = [0.0, 0.0, 0.0, 1.0];
        pivots.extend(p.to_bytes());
    }
    body.extend(hchunk(W3D_CHUNK_PIVOTS, &pivots));
    am_sub(W3D_CHUNK_HIERARCHY, &body)
}

fn am_anim(hier: &[u8], name: &[u8], compressed: bool) -> Vec<u8> {
    if compressed {
        let mut h = W3dCompressedAnimHeaderStruct::from_bytes(&[0; W3dCompressedAnimHeaderStruct::SIZE]);
        h.version = w3d_make_version(0, 1);
        h.name = am_name16(name);
        h.hierarchy_name = am_name16(hier);
        h.num_frames = 5;
        h.frame_rate = 15;
        am_sub(W3D_CHUNK_COMPRESSED_ANIMATION, &hchunk(W3D_CHUNK_COMPRESSED_ANIMATION_HEADER, &h.to_bytes()))
    } else {
        let mut h = W3dAnimHeaderStruct::from_bytes(&[0; W3dAnimHeaderStruct::SIZE]);
        h.version = w3d_make_version(4, 1);
        h.name = am_name16(name);
        h.hierarchy_name = am_name16(hier);
        h.num_frames = 10;
        h.frame_rate = 30;
        am_sub(W3D_CHUNK_ANIMATION, &hchunk(W3D_CHUNK_ANIMATION_HEADER, &h.to_bytes()))
    }
}

fn am_morph(hier: &[u8], name: &[u8], poses: &[&[u8]]) -> Vec<u8> {
    let mut h = W3dMorphAnimHeaderStruct::from_bytes(&[0; W3dMorphAnimHeaderStruct::SIZE]);
    h.version = w3d_make_version(0, 1);
    h.name = am_name16(name);
    h.hierarchy_name = am_name16(hier);
    h.frame_count = 4;
    h.frame_rate = 30.0;
    h.channel_count = poses.len() as u32;
    let mut body = hchunk(W3D_CHUNK_MORPHANIM_HEADER, &h.to_bytes());
    for p in poses {
        let mut pn = p.to_vec();
        pn.push(0);
        let mut ch = hchunk(W3D_CHUNK_MORPHANIM_POSENAME, &pn);
        ch.extend(hchunk(W3D_CHUNK_MORPHANIM_KEYDATA, &[0, 0, 0, 0, 1, 0, 0, 0]));
        body.extend(am_sub(W3D_CHUNK_MORPHANIM_CHANNEL, &ch));
    }
    am_sub(W3D_CHUNK_MORPH_ANIMATION, &body)
}

fn am_hlod(name: &[u8], hier: &[u8], sub: &[u8]) -> Vec<u8> {
    let mut h = W3dHLodHeaderStruct::from_bytes(&[0; W3dHLodHeaderStruct::SIZE]);
    h.version = w3d_make_version(1, 0);
    h.lod_count = 1;
    h.name = am_name16(name);
    h.hierarchy_name = am_name16(hier);
    let mut a = W3dHLodArrayHeaderStruct::from_bytes(&[0; W3dHLodArrayHeaderStruct::SIZE]);
    a.model_count = 1;
    let mut s = W3dHLodSubObjectStruct::from_bytes(&[0; W3dHLodSubObjectStruct::SIZE]);
    let k = sub.len().min(31);
    s.name[..k].copy_from_slice(&sub[..k]);
    let mut arr = hchunk(W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER, &a.to_bytes());
    arr.extend(hchunk(W3D_CHUNK_HLOD_SUB_OBJECT, &s.to_bytes()));
    let mut body = hchunk(W3D_CHUNK_HLOD_HEADER, &h.to_bytes());
    body.extend(am_sub(W3D_CHUNK_HLOD_LOD_ARRAY, &arr));
    am_sub(W3D_CHUNK_HLOD, &body)
}

fn am_name32(s: &[u8]) -> [u8; 2 * W3D_NAME_LEN] {
    let mut n = [0u8; 2 * W3D_NAME_LEN];
    let k = s.len().min(2 * W3D_NAME_LEN);
    n[..k].copy_from_slice(&s[..k]);
    n
}

fn am_box(name: &[u8], oriented: bool) -> Vec<u8> {
    let mut b = W3dBoxStruct::from_bytes(&[0; W3dBoxStruct::SIZE]);
    b.name = am_name32(name);
    b.attributes = if oriented { W3D_BOX_ATTRIBUTE_ORIENTED } else { 0 };
    hchunk(W3D_CHUNK_BOX, &b.to_bytes())
}

fn am_null(name: &[u8]) -> Vec<u8> {
    let mut b = W3dNullObjectStruct::from_bytes(&[0; W3dNullObjectStruct::SIZE]);
    b.name = am_name32(name);
    hchunk(W3D_CHUNK_NULL_OBJECT, &b.to_bytes())
}

fn am_sphere(name: &[u8], ring: bool) -> Vec<u8> {
    if ring {
        let mut d = W3dRingStruct::from_bytes(&[0; W3dRingStruct::SIZE]);
        d.name = am_name32(name);
        am_sub(W3D_CHUNK_RING, &hchunk(ringobj::CHUNKID_RING_DEF, &d.to_bytes()))
    } else {
        let mut d = W3dSphereStruct::from_bytes(&[0; W3dSphereStruct::SIZE]);
        d.name = am_name32(name);
        am_sub(W3D_CHUNK_SPHERE, &hchunk(sphereobj::CHUNKID_SPHERE_DEF, &d.to_bytes()))
    }
}

fn am_dazzle(name: &[u8], type_name: &[u8]) -> Vec<u8> {
    let mut body = hchunk(W3D_CHUNK_DAZZLE_NAME, name);
    body.extend(hchunk(W3D_CHUNK_DAZZLE_TYPENAME, type_name));
    am_sub(W3D_CHUNK_DAZZLE, &body)
}

fn am_collection(name: &[u8], objects: &[&[u8]]) -> Vec<u8> {
    let mut h = W3dCollectionHeaderStruct::from_bytes(&[0; W3dCollectionHeaderStruct::SIZE]);
    h.version = w3d_make_version(4, 2);
    h.name = am_name16(name);
    h.render_object_count = objects.len() as u32;
    let mut body = hchunk(W3D_CHUNK_COLLECTION_HEADER, &h.to_bytes());
    for o in objects {
        let mut n = o.to_vec();
        n.push(0);
        body.extend(hchunk(W3D_CHUNK_COLLECTION_OBJ_NAME, &n));
    }
    am_sub(W3D_CHUNK_COLLECTION, &body)
}

fn am_aggregate(name: &[u8]) -> Vec<u8> {
    let mut h = W3dAggregateHeaderStruct::from_bytes(&[0; W3dAggregateHeaderStruct::SIZE]);
    h.version = w3d_make_version(1, 0);
    h.name = am_name16(name);
    am_sub(W3D_CHUNK_AGGREGATE, &hchunk(W3D_CHUNK_AGGREGATE_HEADER, &h.to_bytes()))
}

fn s(b: &[u8]) -> Vec<u8> {
    b.to_vec()
}

/// The scenarios of `formats::w3d::assetmgr`'s unit tests, with the state
/// logged after each.
fn am_named_cases() -> Vec<(&'static str, AmCase)> {
    let mut v = Vec::new();
    let mut file = am_htree(b"SKL", 3);
    file.extend(am_mesh(b"CONT", b"MESHA"));
    file.extend(am_hlod(b"CONT", b"SKL", b"CONT.MESHA"));
    file.extend(am_anim(b"SKL", b"WALK", false));
    file.extend(am_box(b"CONT.BOX", false));
    file.extend(am_null(b"NUL1"));
    v.push((
        "a file registers each asset by name",
        AmCase { game: false, user_data: None, files: vec![], ops: vec![AmOp::Bytes(file), AmOp::State] },
    ));

    let mut file = am_htree(b"SKL", 1);
    file.extend(am_mesh(b"CONT", b"MESHA"));
    file.extend(am_anim(b"SKL", b"WALK", false));
    file.extend(am_null(b"null"));
    file.extend(am_box(b"\xC9BOX", false));
    v.push((
        "lookups ignore ASCII case; NULL is built in",
        AmCase {
            game: false,
            user_data: None,
            files: vec![],
            ops: vec![
                AmOp::Bytes(file),
                AmOp::Exists(s(b"cont.mesha")),
                AmOp::HTree(s(b"Skl")),
                AmOp::HAnim(s(b"skl.walk")),
                AmOp::Create(s(b"Null")),
                AmOp::Exists(s(b"\xC9box")),
                AmOp::Exists(s(b"\xE9BOX")),
                AmOp::State,
            ],
        },
    ));

    let mut file = am_htree(b"SKL", 3);
    file.extend(am_mesh(b"CONT", b"MESHA"));
    file.extend(am_htree(b"skl", 5));
    file.extend(am_mesh(b"cont", b"mesha"));
    file.extend(am_anim(b"SKL", b"WALK", false));
    file.extend(am_anim(b"skl", b"walk", true));
    v.push((
        "duplicate names: the first from a file wins",
        AmCase {
            game: false,
            user_data: None,
            files: vec![],
            ops: vec![
                AmOp::Bytes(file),
                AmOp::State,
                AmOp::Remove(s(b"Cont.Mesha")),
                AmOp::Exists(s(b"CONT.MESHA")),
                AmOp::State,
            ],
        },
    ));

    let mut file = am_mesh(b"TANK", b"BODY");
    file.extend(am_hlod(b"TANK", b"TANKSKL", b"TANK.BODY"));
    v.push((
        "render objects on demand (game)",
        AmCase {
            game: true,
            user_data: None,
            files: vec![(s(b"art/w3d/tank.w3d"), file.clone())],
            ops: vec![
                AmOp::Create(s(b"TANK.BODY")),
                AmOp::Create(s(b"tank")),
                AmOp::Create(s(b"GHOST")),
                AmOp::State,
            ],
        },
    ));
    v.push((
        "render objects, on demand off (library)",
        AmCase {
            game: false,
            user_data: None,
            files: vec![(s(b"art/w3d/tank.w3d"), file)],
            ops: vec![AmOp::Create(s(b"TANK.BODY")), AmOp::OnDemand(true), AmOp::Create(s(b"TANK.BODY")), AmOp::State],
        },
    ));

    v.push((
        "file paths follow GameFileClass",
        AmCase {
            game: true,
            user_data: Some(s(b"User/")),
            files: vec![
                (s(b"Data/english/Art/W3D/X.w3d"), am_box(b"LOCAL", false)),
                (s(b"Art/W3D/X.w3d"), am_box(b"MAIN", false)),
                (s(b"../TestArt/Y.w3d"), am_box(b"TEST", false)),
                (s(b"User/W3D/Z.w3d"), am_box(b"USER", false)),
                (s(b"Art/W3D/..\\W.w3d"), am_box(b"PARENT", true)),
            ],
            ops: vec![
                AmOp::Create(s(b"X")),
                AmOp::Create(s(b"Y")),
                AmOp::Create(s(b"Z")),
                AmOp::Create(s(b"W")),
                AmOp::File(s(b"none.w3d")),
                AmOp::State,
            ],
        },
    ));

    v.push((
        "animations on demand: H.A loads A.w3d, its hierarchy loads H.w3d",
        AmCase {
            game: true,
            user_data: None,
            files: vec![(s(b"Art/W3D/RUN.w3d"), am_anim(b"SKL", b"RUN", false)), (s(b"Art/W3D/SKL.w3d"), am_htree(b"SKL", 4))],
            ops: vec![
                AmOp::HAnim(s(b"SKL.RUN")),
                AmOp::HAnim(s(b"SKL.JUMP")),
                AmOp::HAnim(s(b"SKL.JUMP")),
                AmOp::Free,
                AmOp::HAnim(s(b"SKL.JUMP")),
                AmOp::HAnim(s(b"NODOT")),
                AmOp::State,
            ],
        },
    ));

    let mut run = am_htree(b"SKL", 2);
    run.extend(am_anim(b"SKL", b"RUN", false));
    for game in [true, false] {
        v.push((
            if game { "the game's shortcut skips a file named like a prototype" } else { "the library reads it" },
            AmCase {
                game,
                user_data: None,
                files: vec![(s(b"Art/W3D/RUN.w3d"), run.clone())],
                ops: vec![AmOp::OnDemand(true), AmOp::Bytes(am_box(b"RUN", false)), AmOp::HAnim(s(b"SKL.RUN")), AmOp::State],
            },
        ));
    }

    let mut file = am_dazzle(b"XYZ\0", b"SUN\0");
    file.extend(am_htree(b"S", 1));
    for a in [&b"A"[..], b"B", b"C", b"D"] {
        file.extend(am_anim(b"S", a, false));
    }
    file.extend(hchunk(0x9999, &[1, 2, 3]));
    file.extend(am_sphere(b"SPH", false));
    file.extend(am_sphere(b"RNG", true));
    file.extend(am_collection(b"COLL", &[b"A.B"]));
    file.extend(am_aggregate(b"AGG"));
    file.extend(am_morph(b"S", b"M", &[b"S.A", b"S.Q"]));
    v.push((
        "dazzles, primitives, collections, aggregates, morphs, unknown chunks, anim order",
        AmCase { game: false, user_data: None, files: vec![], ops: vec![AmOp::Bytes(file), AmOp::HAnim(s(b"S.Q")), AmOp::State] },
    ));

    let mut a = am_htree(b"SIXTEENCHARSNAME", 2);
    a.extend(am_box(b"0123456789ABCDEF0123456789ABCDEF", false));
    a.extend(am_anim(b"SIXTEENCHARSNAM", b"X", false));
    v.push((
        "names that run on: a 16-character hierarchy into NumPivots, a 32-character box into Color",
        AmCase { game: false, user_data: None, files: vec![], ops: vec![AmOp::Bytes(a), AmOp::Create(s(b"0123456789abcdef0123456789abcdef")), AmOp::State] },
    ));

    let mut m = am_htree(b"S", 1);
    m.extend(am_morph(b"S", b"M", &[b"S.P"]));
    v.push((
        "a morph's pose loads its file on demand",
        AmCase {
            game: true,
            user_data: None,
            files: vec![(s(b"Art/W3D/P.w3d"), am_anim(b"S", b"P", false))],
            ops: vec![AmOp::Bytes(m), AmOp::State],
        },
    ));
    v
}

fn am_compare(check: &mut Check, name: &str, c: &AmCase) -> bool {
    let rust = match rust_am_case(c) {
        Ok(l) => l,
        Err(_) => return false,
    };
    let cpp = cpp_am_case(c);
    check.eq(&rust, &cpp, || format!("{name}: {c:02x?}"));
    true
}

/// The named cases: the scenarios of `formats::w3d::assetmgr`'s unit tests
/// (registration of each kind by name; case-insensitive lookups and the
/// built-in `NULL`; duplicates, first from a file wins, removal; render
/// objects on demand through the game's paths, and with on-demand loading
/// off; the path search order with the `..\` retry; animations on demand,
/// the missing table surviving `Free_Assets`; the game's prototype shortcut
/// against the library's plain load), plus dazzles, spheres, rings,
/// collections, aggregates, a morph animation and an unknown chunk in one
/// file; names that run on past their fields; and a morph whose pose
/// animation is loaded on demand during its load.
///
/// Compared byte for byte, after each operation: its result (which
/// prototype `Create_Render_Obj` would create from, by name and class id;
/// the hierarchy or animation found, by name; `Is_Missing`; the load
/// results), and the state (prototypes in `Prototypes` order, hierarchies in
/// load order with their pivot counts, animations in the hash table's
/// iteration order).
///
/// Predicted before the run: all 13 agree; in particular the hash-table
/// iteration order of `S.A`..`S.D`, `S.M`, the run-on hierarchy name
/// `SIXTEENCHARSNAME\x02` (NumPivots 2) that the animation naming
/// `SIXTEENCHARSNAM` does not find (no animation loads), and the morph
/// animation `S.M` registered after its pose `S.P` (loaded from `P.w3d`
/// during the morph's load).
///
/// Result (2026-11-05): 12 compared (the prediction's 13 was miscounted:
/// there are 12 cases), 0 mismatches. Sensitivity checked by hand the same
/// day: with the game's prototype shortcut disabled in the Rust, this test
/// reports 1 mismatch and [`assetmgr_matches_upstream`] 113; with the
/// animation order changed to plain load order, 1 and 26.
#[test]
fn assetmgr_named_cases_match_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut check = Check::new("WW3DAssetManager (assetmgr.cpp, htreemgr.cpp, hanimmgr.cpp), named cases");
    let cases = am_named_cases();
    for (name, c) in &cases {
        if let Err(e) = rust_am_case(c) {
            panic!("{name}: Rust error {e:?}");
        }
        am_compare(&mut check, name, c);
    }
    println!("asset manager named cases: {} compared", cases.len());
    check.finish();
}

const AM_HIER: [&[u8]; 5] = [b"SKL", b"skl", b"TNK", b"A", b"SIXTEENCHARSNAME"];
const AM_ANIM: [&[u8]; 5] = [b"RUN", b"run", b"WALK", b"A", b"B"];
const AM_PROTO: [&[u8]; 9] = [b"TNK", b"tnk", b"RUN", b"SKL", b"BOX1", b"null", b"A", b"B", b"0123456789ABCDEF"];
const AM_BASE: [&[u8]; 8] = [b"SKL", b"TNK", b"RUN", b"WALK", b"A", b"B", b"skl", b"BOX1"];
const AM_DIR: [&[u8]; 6] = [b"Art/W3D/", b"Data/english/Art/W3D/", b"../TestArt/", b"User/W3D/", b"Art/W3D/..\\", b""];

fn am_pick<'a>(g: &mut Gn, v: &[&'a [u8]]) -> &'a [u8] {
    v[g.below(v.len() as u32) as usize]
}

fn am_gen_chunk(g: &mut Gn) -> Vec<u8> {
    // the 16-character hierarchy name (undefined when an animation passes
    // it to Get_HTree) in one chunk of 16
    let h = if g.below(16) == 0 { AM_HIER[4] } else { am_pick(g, &AM_HIER[..4]) };
    let a = am_pick(g, &AM_ANIM);
    let p = am_pick(g, &AM_PROTO);
    match g.below(15) {
        0 | 1 => am_htree(h, 1 + g.below(4)),
        2 | 3 => am_anim(h, a, g.coin()),
        4 => {
            let mut pose = h.to_vec();
            pose.push(b'.');
            pose.extend_from_slice(am_pick(g, &AM_ANIM));
            am_morph(h, a, &[&pose])
        }
        5 | 6 => {
            let (c, m) = if g.coin() { (p, &b"BODY"[..]) } else { (&b""[..], p) };
            am_mesh(c, m)
        }
        7 => am_hlod(p, h, b"X.Y"),
        8 => am_box(p, g.coin()),
        9 => am_null(p),
        10 => am_sphere(p, g.coin()),
        11 => {
            let mut n = p.to_vec();
            n.push(0);
            am_dazzle(&n, b"T\0")
        }
        12 => am_collection(p, &[b"Q.R"]),
        13 => am_aggregate(p),
        _ => {
            let junk: Vec<u8> = (0..g.below(12)).map(|_| g.g.u32() as u8).collect();
            let id = [0x9999, W3D_CHUNK_HIERARCHY_HEADER, W3D_CHUNK_PIVOTS, g.g.u32()][g.below(4) as usize];
            hchunk(id, &junk)
        }
    }
}

fn am_gen_file(g: &mut Gn, gen_chunk: fn(&mut Gn) -> Vec<u8>) -> Vec<u8> {
    let mut f = Vec::new();
    for _ in 0..1 + g.below(5) {
        f.extend(gen_chunk(g));
    }
    mutate(g, f)
}

fn am_gen_name(g: &mut Gn) -> Vec<u8> {
    match g.below(6) {
        0 => am_pick(g, &AM_HIER).to_vec(),
        1 => am_pick(g, &AM_PROTO).to_vec(),
        2 => [am_pick(g, &AM_PROTO), b".BODY"].concat(),
        3 => [am_pick(g, &AM_HIER), b".", am_pick(g, &AM_ANIM)].concat(),
        4 => [am_pick(g, &AM_HIER), b".", am_pick(g, &AM_ANIM), b".x"].concat(),
        _ => s(b"NULL"),
    }
}

/// A name to remove: not `NULL` (undefined), except in one case of 16.
fn am_gen_remove(g: &mut Gn) -> Vec<u8> {
    if g.below(16) == 0 {
        return s(b"NULL");
    }
    loop {
        let n = am_gen_name(g);
        if !n.eq_ignore_ascii_case(b"NULL") {
            return n;
        }
    }
}

fn am_gen_case(g: &mut Gn, gen_chunk: fn(&mut Gn) -> Vec<u8>) -> AmCase {
    let mut files = Vec::new();
    for _ in 0..g.below(6) {
        let mut p = am_pick(g, &AM_DIR).to_vec();
        p.extend_from_slice(am_pick(g, &AM_BASE));
        p.extend_from_slice(if g.below(4) == 0 { b".W3D" } else { b".w3d" });
        files.push((p, am_gen_file(g, gen_chunk)));
    }
    let mut ops = Vec::new();
    for _ in 0..3 + g.below(10) {
        let op = match g.below(20) {
            0..=2 => AmOp::Bytes(am_gen_file(g, gen_chunk)),
            3 | 4 => {
                let mut n = am_pick(g, &AM_BASE).to_vec();
                n.extend_from_slice(b".w3d");
                AmOp::File(n)
            }
            5..=8 => AmOp::Create(am_gen_name(g)),
            9 | 10 => AmOp::HTree(am_gen_name(g)),
            11..=13 => AmOp::HAnim(am_gen_name(g)),
            14 => AmOp::Exists(am_gen_name(g)),
            15 => AmOp::Free,
            16 => AmOp::Remove(am_gen_remove(g)),
            17 => AmOp::OnDemand(g.coin()),
            _ => AmOp::State,
        };
        ops.push(op);
    }
    ops.push(AmOp::State);
    AmCase { game: g.coin(), user_data: if g.coin() { Some(s(b"User/")) } else { None }, files, ops }
}

/// Random file sets and operations: up to 5 files under the paths the
/// game's file search tries (`Art/W3D/`, the localized `Data/english/...`,
/// `../TestArt/`, the user data directory, `Art/W3D/..\`, or no directory;
/// `.w3d` or `.W3D`) holding 1 to 5 chunks each (hierarchies of 1 to 4
/// pivots, raw and compressed animations, morph animations with a pose,
/// meshes with and without a container, HLODs, boxes, null objects, spheres
/// and rings, dazzles, collections, aggregates, stray chunks; names from
/// small pools that collide in and across case, including `null`, `NULL`
/// and 16- and 32-character names), half of the cases also with truncations
/// and byte mutations; then 3 to 12 operations (loads of bytes and of file
/// names, `Create_Render_Obj`, `Get_HTree`, `Get_HAnim`, `Render_Obj_Exists`
/// on names with zero, one or two dots, `Free_Assets`, `Remove_Prototype`,
/// switching on-demand loading) in the library or the game manager, the
/// state logged at the end. Compared as [`assetmgr_named_cases_match_upstream`].
/// Cases on which the Rust reports an error (undefined upstream, a loop that
/// does not end, an untranslated loader) are not given to the C++
/// (counted).
///
/// Predicted before the run: no disagreement; a few percent excluded
/// (`StringOverrun` from 16-character collection names and from the
/// mutations, `NullPointer` from aggregates whose header is mutated away,
/// `UninitialisedRead` from cut boxes and null objects, `NonTerminating`
/// from animations whose hierarchy file is their own, `PointerOutOfRange`
/// from removing `NULL`).
///
/// Result (2026-11-05), first run: 0 mismatches, but 1,214 of 4,000 cases
/// (30 %, far more than predicted) excluded, mostly `StringOverrun` (847:
/// the 16-character hierarchy name, which every animation over it passes to
/// `Get_HTree` unterminated) and `PointerOutOfRange` (244: `Remove_Prototype
/// ("NULL")`, a sixth of the removals). The generator was changed to use
/// those two in one case of 16 (the comparison is unchanged). Second run:
/// 3,322 compared (27,600 operations), 0 mismatches; excluded 678 (17 %):
/// 441 `StringOverrun`, 90 `NonTerminating`, 77 `PointerOutOfRange`, 54
/// `UninitialisedRead`, 10 `NullPointer`, 5 `Allocation`, 1 `Unsupported`
/// (a mutated chunk id that became `W3D_CHUNK_LODMODEL`). **Corrected
/// 2026-11-06 (#17):** the distance-LOD loader is translated, so that case
/// is compared too: 3,323 compared (27,610 operations), 0 mismatches; 677
/// excluded, none `Unsupported`. The three new loaders are exercised by
/// [`assetmgr_emit_loaders_match_upstream`].
#[test]
fn assetmgr_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gn { g: Gen::new(0xA55E), clean: false };
    let mut check = Check::new("WW3DAssetManager (assetmgr.cpp, htreemgr.cpp, hanimmgr.cpp)");
    let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
    let (mut compared, mut ops) = (0u64, 0u64);
    const CASES: usize = 4_000;
    for case in 0..CASES {
        g.clean = case % 2 == 0;
        let c = am_gen_case(&mut g, am_gen_chunk);
        match rust_am_case(&c) {
            Ok(l) => {
                if std::env::var_os("W3D_TRACE").is_some() {
                    eprintln!("assetmgr case {case}: {c:02x?}");
                }
                let cpp = cpp_am_case(&c);
                check.eq(&l, &cpp, || format!("case {case}: {c:02x?}"));
                compared += 1;
                ops += c.ops.len() as u64;
            }
            Err(W3dError::Undefined(u)) => *excluded.entry(format!("{u:?}")).or_default() += 1,
            Err(e) => *excluded.entry(format!("{e:?}")).or_default() += 1,
        }
    }
    println!(
        "WW3DAssetManager: {CASES} cases, {compared} compared ({ops} operations); excluded: {} {excluded:?}",
        excluded.values().sum::<u64>()
    );
    check.finish();
}

// ---------------------------------------------------------------------------
// 13. Animation evaluation (motchan.cpp, hrawanim.cpp, hcanim.cpp,
//     hmorphanim.cpp, htree.cpp, pivot.cpp, quat.cpp)
// ---------------------------------------------------------------------------

use rts_lockstep_engine::formats::w3d::assetmgr::HAnim;
use rts_lockstep_engine::formats::w3d::hanim_combo::{ComboDataRef, HAnimCombo, HAnimComboData, PivotMap, PivotMapRef};
use rts_lockstep_engine::formats::w3d::hmorphanim::PoseLookup;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;

/// What `ref_w3d_eval` loads: the hierarchies, and the animations with
/// their kind (0 raw, 1 compressed, 2 morph).
struct EvalWorld {
    trees: Vec<HTree>,
    anims: Vec<(HAnim, u8)>,
    /// (#162) the combos (their animations by index into `anims`), the
    /// pivot maps and the shared entries the script made (ops 12 to 31).
    combos: Vec<HAnimCombo<usize>>,
    maps: Vec<PivotMapRef>,
    datas: Vec<ComboDataRef<usize>>,
    /// (#179) the render objects ops 40-49 made, each with whether its `PrevFrame` is still comparable (no blend since
    /// it was made: upstream's union overlaps it), and `WW3D::Get_Sync_Time()` (op 41)
    robjs: Vec<(md::RenderObj, bool)>,
    now: u32,
    /// (#179) the animations as the render objects share them (one object each, as upstream's `HAnimClass`: their
    /// channels' caches carry over from one render object's evaluation to another's), made on first use
    anim_datas: Vec<std::sync::Arc<md::AnimData>>,
}

/// wrap_w3d_eval.cpp's pose hook: the first raw or compressed animation of
/// that exact name.
impl PoseLookup for EvalWorld {
    fn pose(&self, name: &[u8]) -> Option<&HAnim> {
        self.anims.iter().find(|(a, k)| *k != 2 && a.name() == name).map(|(a, _)| a)
    }
}

/// The loading half of `ref_w3d_eval`, in Rust; the raw animations sample
/// with `mode`'s `Float_To_Long` (#19).
fn rust_eval_load(data: &[u8], mode: FloatToLongMode) -> W3dResult<(Log, EvalWorld)> {
    let mut log = vec![b'L'];
    let mut w = EvalWorld { trees: Vec::new(), anims: Vec::new(), combos: Vec::new(), maps: Vec::new(), datas: Vec::new(), robjs: Vec::new(), now: 0, anim_datas: Vec::new() };
    let mut c = ChunkLoad::new(GameFile::new(data).unwrap());
    while c.open_chunk()? {
        let trees = &w.trees;
        let mut lookup = |n: &[u8; W3D_NAME_LEN]| trees.iter().find(|t| &t.name == n).map(|t| t.num_pivots);
        match c.cur_chunk_id()? {
            W3D_CHUNK_HIERARCHY => {
                let t = htree::load_w3d(&mut c)?;
                log.extend_from_slice(&[0, u8::from(t.is_none())]);
                w.trees.extend(t);
            }
            W3D_CHUNK_ANIMATION => {
                let a = HRawAnim::load_w3d(&mut c, &mut lookup)?;
                log.extend_from_slice(&[1, u8::from(a.is_none())]);
                w.anims.extend(a.map(|a| (HAnim::Raw(a.with_frame_rounding(mode)), 0)));
            }
            W3D_CHUNK_COMPRESSED_ANIMATION => {
                let a = HCompressedAnim::load_w3d(&mut c, &mut lookup)?;
                log.extend_from_slice(&[2, u8::from(a.is_none())]);
                w.anims.extend(a.map(|a| (HAnim::Compressed(a), 1)));
            }
            W3D_CHUNK_MORPH_ANIMATION => {
                let a = hmorphanim::load_w3d(&mut c, &mut lookup)?;
                log.extend_from_slice(&[3, u8::from(a.is_none())]);
                w.anims.extend(a.map(|a| (HAnim::Morph(a), 2)));
            }
            _ => log.extend_from_slice(&[9, 0]),
        }
        c.close_chunk()?;
    }
    p32(&mut log, w.trees.len() as u32);
    p32(&mut log, w.anims.len() as u32);
    Ok((log, w))
}

const EVAL_SENTINEL: u32 = 0x7FC0_DEAD;

fn log_m(log: &mut Log, m: &Matrix3D) {
    for v in m.to_array() {
        p32(log, v.to_bits());
    }
}

fn log_eval_tree(log: &mut Log, t: &HTree) -> W3dResult<()> {
    for p in 0..t.num_pivots {
        log_m(log, &t.get_transform(p)?);
        log.push(u8::from(t.get_visibility(p)?));
    }
    Ok(())
}

struct ScriptReader<'a> {
    s: &'a [u8],
    i: usize,
}

impl ScriptReader<'_> {
    fn u(&mut self) -> u32 {
        let v = u32::from_le_bytes(self.s[self.i..self.i + 4].try_into().unwrap());
        self.i += 4;
        v
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(self.u())
    }
    fn m(&mut self) -> Matrix3D {
        let a: Vec<f32> = (0..12).map(|_| self.f()).collect();
        Matrix3D::new(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8], a[9], a[10], a[11])
    }
}

/// One script op in Rust (see `ref_w3d_eval`); its log bytes, or where
/// upstream is undefined.
fn rust_eval_op(w: &mut EvalWorld, r: &mut ScriptReader<'_>, log: &mut Log) -> W3dResult<()> {
    let op = r.s[r.i];
    r.i += 1;
    log.push(op);
    match op {
        1..=4 => {
            let a = r.u() as usize;
            let pivot = r.u() as i32;
            let frame = r.f();
            let anim = &w.anims[a].0;
            match op {
                1 => {
                    let t = anim.get_translation(w as &EvalWorld, pivot, frame)?;
                    for v in [t.x, t.y, t.z] {
                        p32(log, v.to_bits());
                    }
                }
                2 => match anim.get_orientation(w as &EvalWorld, pivot, frame)? {
                    Some(q) => {
                        for v in [q.x, q.y, q.z, q.w] {
                            p32(log, v.to_bits());
                        }
                    }
                    None => (0..4).for_each(|_| p32(log, EVAL_SENTINEL)),
                },
                3 => match anim.get_transform(w as &EvalWorld, pivot, frame)? {
                    Some(m) => log_m(log, &m),
                    None => (0..12).for_each(|_| p32(log, EVAL_SENTINEL)),
                },
                _ => log.push(u8::from(anim.get_visibility(pivot, frame)?)),
            }
        }
        5 | 6 => {
            let t = r.u() as usize;
            let a = r.u() as usize;
            let frame = r.f();
            let root = r.m();
            let mut tree = w.trees[t].clone();
            let res = if op == 5 {
                tree.anim_update(&root, &w.anims[a].0, &*w, frame)
            } else {
                let HAnim::Raw(raw) = &w.anims[a].0 else { panic!("op 6 on a raw animation only") };
                tree.anim_update_raw(&root, raw, frame)
            };
            w.trees[t] = tree;
            res?;
            log_eval_tree(log, &w.trees[t])?;
        }
        7 => {
            let t = r.u() as usize;
            let (a0, f0) = (r.u() as usize, r.f());
            let (a1, f1) = (r.u() as usize, r.f());
            let pct = r.f();
            let root = r.m();
            let mut tree = w.trees[t].clone();
            let res = tree.blend_update(&root, &w.anims[a0].0, f0, &w.anims[a1].0, f1, pct, &*w);
            w.trees[t] = tree;
            res?;
            log_eval_tree(log, &w.trees[t])?;
        }
        8 => {
            let t = r.u() as usize;
            let root = r.m();
            w.trees[t].base_update(&root)?;
            log_eval_tree(log, &w.trees[t])?;
        }
        9 => {
            let t = r.u() as usize;
            let a = r.u() as usize;
            let pivot = r.u() as i32;
            let frame = r.f();
            let obj = r.m();
            let (ok, m) = w.trees[t].simple_evaluate_pivot_anim(&w.anims[a].0, &*w, pivot, frame, &obj)?;
            log.push(u8::from(ok));
            log_m(log, &m);
        }
        10 => {
            let t = r.u() as usize;
            let pivot = r.u() as i32;
            let obj = r.m();
            let (ok, m) = w.trees[t].simple_evaluate_pivot(pivot, &obj)?;
            log.push(u8::from(ok));
            log_m(log, &m);
        }
        11 => {
            let t = r.u() as usize;
            let bone = r.u() as i32;
            let capture = r.u();
            let wst = r.u();
            let m = r.m();
            if capture != 0 {
                w.trees[t].capture_bone(bone)?;
                w.trees[t].control_bone(bone, &m, wst != 0)?;
            } else {
                w.trees[t].release_bone(bone)?;
            }
        }
        12..=31 => rust_combo_op(w, r, log, op)?,
        40..=49 => rust_robj_op(w, r, log, op)?,
        _ => unreachable!(),
    }
    Ok(())
}

/// (#162) A `Normalize_Weights` or `Combo_Update` whose result holds a NaN is not compared (the script stops before it,
/// counted under this reason): which NaN an operation with two NaN operands returns (their signs and payloads) depends
/// on the operand order the compiler chose, as x86 returns the first; NaN weights (the wild scripts' `mixed` values),
/// `weight / weight_total` with a zero total and `-alpha` give NaNs of both signs. The first run found one such case
/// (16,775 of 20,000: the same NaNs, with opposite signs). Defined upstream, but not reproducible bit for bit.
const NAN_NOT_COMPARED: &str = "a NaN result (its sign and payload not compared)";

/// The names op 20 adds (wrap_w3d_eval.cpp's `COMBO_NAMES`).
const COMBO_NAMES: [&[u8]; 10] = [b"B0", b"B1", b"b2", b"B3", b"B4", b"B5", b"RootTransform", b"NOPE", b"rOOTtRANSFORM", b""];

fn log_map(log: &mut Log, m: &Option<PivotMapRef>) {
    match m {
        None => p32(log, 0xFFFF_FFFF),
        Some(m) => {
            let m = m.read().unwrap();
            p32(log, m.count() as u32);
            for v in &m.weights {
                p32(log, v.to_bits());
            }
        }
    }
}

fn log_combo(log: &mut Log, c: &HAnimCombo<usize>) -> W3dResult<()> {
    p32(log, c.get_num_anims() as u32);
    for i in 0..c.get_num_anims() {
        p32(log, c.get_frame(i)?.to_bits());
        p32(log, c.get_prev_frame(i)?.to_bits());
        p32(log, c.get_weight(i)?.to_bits());
        p32(log, c.peek_motion(i)?.map_or(0xFFFF_FFFF, |a| a as u32));
        log_map(log, &c.peek_pivot_weight_map(i)?);
    }
    Ok(())
}

/// The combo ops (#162) in Rust (see `ref_w3d_eval`).
fn rust_combo_op(w: &mut EvalWorld, r: &mut ScriptReader<'_>, log: &mut Log, op: u8) -> W3dResult<()> {
    match op {
        12 => {
            let n = r.u() as i32;
            w.combos.push(if n < 0 { HAnimCombo::new() } else { HAnimCombo::with_anims(n as usize) });
        }
        13..=17 => {
            let c = r.u() as usize;
            let i = r.u() as i32;
            let v = r.u();
            let combo = &w.combos[c];
            match op {
                13 => combo.set_motion(i, if (v as i32) < 0 { None } else { Some(v as usize) })?,
                14 => combo.set_frame(i, f32::from_bits(v))?,
                15 => combo.set_prev_frame(i, f32::from_bits(v))?,
                16 => combo.set_weight(i, f32::from_bits(v))?,
                _ => combo.set_pivot_weight_map(i, if (v as i32) < 0 { None } else { Some(w.maps[v as usize].clone()) })?,
            }
        }
        18 => {
            let named = r.u();
            w.maps.push(if named != 0 { PivotMap::new_named() } else { PivotMap::new() }.shared());
        }
        19 => {
            let m = r.u() as usize;
            let v = r.f();
            w.maps[m].write().unwrap().add(v);
        }
        20 => {
            let m = r.u() as usize;
            let name = r.u() as usize;
            let v = r.f();
            w.maps[m].write().unwrap().add_named(COMBO_NAMES[name], v);
        }
        21 => {
            let m = r.u() as usize;
            let t = r.u() as usize;
            w.maps[m].write().unwrap().update_pivot_map(&w.trees[t])?;
        }
        22 => {
            let c = r.u() as usize;
            let i = r.u() as i32;
            let d = w.combos[c].peek_anim_combo_data(i)?;
            let anims = &w.anims;
            d.write().unwrap().build_active_pivot_map(|&a| &anims[a].0)?;
        }
        23 => {
            let c = r.u() as usize;
            let anims = &w.anims;
            let ok = w.combos[c].normalize_weights(|&a| &anims[a].0)?;
            log.push(u8::from(ok));
            log_combo(log, &w.combos[c])?;
            let nan = (0..w.combos[c].get_num_anims()).any(|i| {
                w.combos[c].get_weight(i).is_ok_and(f32::is_nan)
                    || w.combos[c].get_pivot_weight_map(i).ok().flatten().is_some_and(|m| m.read().unwrap().weights.iter().any(|v| v.is_nan()))
            });
            if nan {
                return Err(W3dError::Unsupported(NAN_NOT_COMPARED));
            }
        }
        24 => {
            let t = r.u() as usize;
            let c = r.u() as usize;
            let root = r.m();
            let mut tree = w.trees[t].clone();
            let res = tree.combo_update(&root, &w.combos[c], &*w, |&a| &w.anims[a].0);
            w.trees[t] = tree;
            res?;
            log_eval_tree(log, &w.trees[t])?;
            let tr = &w.trees[t];
            if (0..tr.num_pivots).any(|p| tr.get_transform(p).is_ok_and(|m| m.to_array().iter().any(|v| v.is_nan()))) {
                return Err(W3dError::Unsupported(NAN_NOT_COMPARED));
            }
        }
        25 => w.datas.push(HAnimComboData::new(true).into_ref()),
        26 | 27 => {
            let c = r.u() as usize;
            let d = w.datas[r.u() as usize].clone();
            if op == 26 {
                w.combos[c].append_anim_combo_data(d);
            } else {
                w.combos[c].remove_anim_combo_data(&d);
            }
        }
        28..=30 => {
            let c = r.u() as usize;
            match op {
                28 => w.combos[c].clear(),
                29 => w.combos[c].reset(),
                _ => log_combo(log, &w.combos[c])?,
            }
        }
        _ => {
            let m = r.u() as usize;
            log_map(log, &Some(w.maps[m].clone()));
        }
    }
    Ok(())
}

/// The ops of a script, as byte ranges.
fn script_ops(script: &[u8]) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    let mut i = 0;
    while i < script.len() {
        let words = match script[i] {
            1..=4 => 3,
            5 | 6 => 3 + 12,
            7 => 6 + 12,
            8 => 1 + 12,
            9 => 4 + 12,
            10 => 2 + 12,
            11 => 4 + 12,
            12 | 18 | 23 | 28..=31 => 1,
            13..=17 | 20 => 3,
            19 | 21 | 22 | 26 | 27 => 2,
            24 => 2 + 12,
            25 => 0,
            // (#179)
            40 => 2,
            41 | 43 => 1,
            42 => 4,
            44 => 6,
            45 | 48 | 49 => 2,
            46 => 1 + 12,
            47 => 4 + 12,
            _ => unreachable!(),
        };
        v.push((i, i + 1 + 4 * words));
        i += 1 + 4 * words;
    }
    v
}

fn cpp_eval(data: &[u8], script: &[u8]) -> Log {
    cpp_log(|log, cap, n| unsafe {
        cpp::ref_w3d_eval(data.as_ptr() as *const _, data.len() as i32, script.as_ptr(), script.len() as i32, log, cap, n)
    })
}

/// (#179) The render objects' ops (40-49, see `ref_w3d_eval`) on the port's `RenderObj`.
use rts_lockstep_engine::client::w3d_model_draw as md;

/// The animation `a` as a render object is given it: its handle with the motion's data, a morph animation's poses the
/// raw and compressed animations (the wrapper's pose hook: the first of that exact name; the generated names differ
/// case-insensitively too, so `AnimData`'s lookup finds the same one).
fn robj_handle(w: &mut EvalWorld, a: usize) -> md::AnimHandle {
    if w.anim_datas.is_empty() {
        let shared: Vec<std::sync::Arc<HAnim>> = w.anims.iter().map(|(x, _)| std::sync::Arc::new(x.clone())).collect();
        let poses: Vec<std::sync::Arc<HAnim>> = shared.iter().zip(&w.anims).filter(|(_, (_, k))| *k != 2).map(|(x, _)| x.clone()).collect();
        w.anim_datas = shared.iter().map(|m| std::sync::Arc::new(md::AnimData { motion: m.clone(), poses: poses.clone() })).collect();
    }
    let anim = &w.anims[a].0;
    md::AnimHandle {
        name: anim.name().to_vec(),
        num_frames: anim.get_num_frames(),
        frame_rate: rts_lockstep_engine::formats::w3d::render::get_frame_rate(anim),
        data: Some(w.anim_datas[a].clone()),
    }
}

/// The wrapper's `put_robj`.
fn log_robj(log: &mut Log, r: &md::RenderObj, clean: bool) {
    let mode = if r.anim.is_some() {
        2
    } else if r.mode_interp.is_some() {
        3
    } else if r.mode_combo.is_some() {
        4
    } else {
        1
    };
    log.push(mode);
    log.push(u8::from(r.is_hierarchy_valid()));
    match &r.anim {
        Some(a) => {
            p32(log, a.frame.to_bits());
            p32(log, if clean { a.prev_frame } else { 0.0 }.to_bits());
            p32(log, a.last_sync_time);
            p32(log, a.anim_direction.to_bits());
            p32(log, a.frame_rate_multiplier.to_bits());
            p32(log, md::anim_mode_index(a.anim_mode) as u32);
            log.push(u8::from(r.is_animation_complete_hlod()));
        }
        None => {
            for _ in 0..6 {
                p32(log, 0);
            }
            log.push(0);
        }
    }
}

/// Whether a loaded hierarchy can be a render object's: its pivots read, and every pivot after the first with a parent
/// the port's skeleton keeps (`Skeleton::bones_of` reads the parent's `Index`; a missing parent or one past the end,
/// undefined upstream, has no place in a skeleton).
fn robj_ok(t: &HTree) -> bool {
    let n = t.num_pivots;
    n > 0
        && t.pivots.len() == n as usize
        && t.pivots.iter().skip(1).all(|p| p.parent.is_some_and(|q| q < n as u32 && t.pivots[q as usize].index == q as i32))
}

fn rust_robj_op(w: &mut EvalWorld, r: &mut ScriptReader<'_>, log: &mut Log, op: u8) -> W3dResult<()> {
    match op {
        40 => {
            let t = r.u() as usize;
            let scale = r.f();
            // `new HTreeClass(*source)`, then `Create_Render_Obj`'s scale
            let mut tree = w.trees[t].clone();
            md::scale_render_obj_hierarchy(&mut tree, scale);
            let k = md::Skeleton { bones: md::Skeleton::bones_of(&tree), sub_objects: Vec::new(), scale_factor: tree.scale_factor };
            w.robjs.push((md::RenderObj::new(b"R", true).with_skeleton(Some(k)), true));
            let (ro, clean) = w.robjs.last().expect("pushed");
            log_robj(log, ro, *clean);
        }
        41 => w.now = r.u(),
        _ => {
            let ri = r.u() as usize;
            let now = w.now;
            match op {
                42 => {
                    let a = r.u() as i32;
                    let frame = r.f();
                    let mode = r.u() as i32;
                    let h = (a >= 0).then(|| robj_handle(w, a as usize));
                    w.robjs[ri].0.set_animation(h, frame, md::anim_mode_from_index(mode), now);
                }
                43 => w.robjs[ri].0.set_animation_base_pose(),
                44 => {
                    let (a0, f0) = (r.u() as usize, r.f());
                    let (a1, f1) = (r.u() as usize, r.f());
                    let pct = r.f();
                    let (h0, h1) = (robj_handle(w, a0), robj_handle(w, a1));
                    w.robjs[ri].0.set_animation_blend(Some(h0), f0, Some(h1), f1, pct);
                    w.robjs[ri].1 = false;
                }
                45 => {
                    let bone = r.u() as i32;
                    let m = w.robjs[ri].0.try_get_bone_transform(bone, now)?;
                    log_m(log, &m);
                }
                46 => {
                    let m = r.m();
                    w.robjs[ri].0.set_transform(&m);
                }
                47 => {
                    let bone = r.u() as i32;
                    let capture = r.u();
                    let wst = r.u();
                    let m = r.m();
                    let ro = &mut w.robjs[ri].0;
                    if capture != 0 {
                        ro.capture_bone(bone);
                        ro.control_bone_world(bone, &m, wst != 0);
                    } else {
                        ro.release_bone(bone);
                    }
                }
                48 => {
                    let ro = &mut w.robjs[ri].0;
                    ro.hidden = r.u() != 0;
                    ro.try_render(now)?;
                }
                _ => {
                    let f = r.f();
                    w.robjs[ri].0.set_animation_frame_rate_multiplier(f);
                }
            }
            let (ro, clean) = &w.robjs[ri];
            log_robj(log, ro, *clean);
        }
    }
    Ok(())
}

// ---- generation ----

fn name16(s: &[u8]) -> [u8; W3D_NAME_LEN] {
    let mut n = [0u8; W3D_NAME_LEN];
    n[..s.len()].copy_from_slice(s);
    n
}

/// A unit quaternion (normalised in `f32`), or now and then arbitrary
/// components.
fn ev_quat(g: &mut Gn) -> [f32; 4] {
    let mut q = [0.0f32; 4];
    for v in q.iter_mut() {
        *v = g.g.uniform(-1.0, 1.0);
    }
    if g.chance(10) {
        return q;
    }
    let l = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if l < 1e-3 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    q.map(|v| v / l)
}

fn ev_tree(g: &mut Gn, name: &[u8; W3D_NAME_LEN], n: u32) -> Vec<u8> {
    let pre30 = g.below(6) == 0;
    let mut h = W3dHierarchyStruct::from_bytes(&[0; 36]);
    h.version = if pre30 { w3d_make_version(2, 0) } else { w3d_make_version(4, 1) };
    h.name = *name;
    h.num_pivots = n;
    let mut body = hchunk(W3D_CHUNK_HIERARCHY_HEADER, &h.to_bytes());
    if !g.chance(25) {
        let total = if pre30 { n + 1 } else { n };
        let mut pb = Vec::new();
        for i in 0..n {
            let mut p = W3dPivotStruct::from_bytes(&[0; 60]);
            p.name = name16(format!("B{i}").as_bytes());
            let first = i == 0 && !pre30;
            p.parent_idx = match if g.clean { 30 } else { g.below(30) } {
                0 => 0xFFFF_FFFF,
                1 => i,
                2 => total - 1,
                3 if !pre30 => total,
                _ if first => 0xFFFF_FFFF,
                _ if pre30 => g.below(i + 1).wrapping_sub(1),
                _ => g.below(i),
            };
            p.translation = W3dVectorStruct { x: g.g.uniform(-2.0, 2.0), y: g.g.uniform(-2.0, 2.0), z: g.g.uniform(-2.0, 2.0) };
            p.rotation.q = ev_quat(g);
            pb.extend(p.to_bytes());
        }
        body.extend(hchunk(W3D_CHUNK_PIVOTS, &pb));
    }
    hchunk(W3D_CHUNK_HIERARCHY, &body)
}

fn ev_frames(g: &mut Gn) -> u32 {
    match g.below(12) {
        0 => 0,
        1 => 20 + g.below(60),
        _ => 1 + g.below(10),
    }
}

fn ev_raw(g: &mut Gn, hier: &[u8; W3D_NAME_LEN], npiv: u32, name: &[u8]) -> Vec<u8> {
    let nf = ev_frames(g);
    let mut h = W3dAnimHeaderStruct::from_bytes(&[0; 44]);
    h.version = if g.below(10) == 0 { w3d_make_version(2, 0) } else { w3d_make_version(4, 1) };
    h.name = name16(name);
    h.hierarchy_name = *hier;
    h.num_frames = nf;
    h.frame_rate = 30;
    let mut body = hchunk(W3D_CHUNK_ANIMATION_HEADER, &h.to_bytes());
    for pivot in 0..npiv + 1 {
        for ty in [ANIM_CHANNEL_X, ANIM_CHANNEL_Y, ANIM_CHANNEL_Z, ANIM_CHANNEL_XR, ANIM_CHANNEL_Q] {
            if g.below(if ty == ANIM_CHANNEL_XR { 8 } else { 2 }) != 0 {
                continue;
            }
            let first = g.below(3) as u16;
            let last = first + g.below(nf.max(1) + 1) as u16;
            let vlen: u16 = match (ty, if g.clean { 14 } else { g.below(14) }) {
                (ANIM_CHANNEL_Q, 0) => 3,
                (ANIM_CHANNEL_Q, 1) => 5,
                (ANIM_CHANNEL_Q, _) => 4,
                (_, 0) => 2 + g.below(3) as u16,
                _ => 1,
            };
            let mut b = Vec::new();
            for v in [first, last, vlen, ty as u16, pivot as u16, 0] {
                b.extend_from_slice(&v.to_le_bytes());
            }
            for _ in first..=last {
                if ty == ANIM_CHANNEL_Q && vlen == 4 {
                    for v in ev_quat(g) {
                        b.extend_from_slice(&v.to_bits().to_le_bytes());
                    }
                } else {
                    for _ in 0..vlen {
                        b.extend_from_slice(&g.g.uniform(-3.0, 3.0).to_bits().to_le_bytes());
                    }
                }
            }
            body.extend(hchunk(W3D_CHUNK_ANIMATION_CHANNEL, &b));
        }
        if g.below(3) == 0 {
            let first = g.below(3) as u16;
            let last = first + g.below(nf.max(1) + 2) as u16;
            let nbytes = (u32::from(last - first) + 1).div_ceil(8);
            let mut b = Vec::new();
            for v in [first, last, BIT_CHANNEL_VIS as u16, pivot as u16] {
                b.extend_from_slice(&v.to_le_bytes());
            }
            b.push(g.below(2) as u8);
            b.extend((0..nbytes).map(|_| g.g.u32() as u8));
            body.extend(hchunk(W3D_CHUNK_BIT_CHANNEL, &b));
        }
    }
    hchunk(W3D_CHUNK_ANIMATION, &body)
}

fn ev_compressed(g: &mut Gn, hier: &[u8; W3D_NAME_LEN], npiv: u32, name: &[u8]) -> Vec<u8> {
    let nf = ev_frames(g);
    let flavor: u16 = match g.below(16) {
        0 if !g.clean => 5,
        1..=8 => ANIM_FLAVOR_ADAPTIVE_DELTA as u16,
        _ => ANIM_FLAVOR_TIMECODED as u16,
    };
    let mut h = W3dCompressedAnimHeaderStruct::from_bytes(&[0; 44]);
    h.version = w3d_make_version(4, 1);
    h.name = name16(name);
    h.hierarchy_name = *hier;
    h.num_frames = nf;
    h.frame_rate = 30;
    h.flavor = flavor;
    let mut body = hchunk(W3D_CHUNK_COMPRESSED_ANIMATION_HEADER, &h.to_bytes());
    let adaptive = flavor == ANIM_FLAVOR_ADAPTIVE_DELTA as u16;
    for pivot in 0..npiv + 1 {
        for ty in [ANIM_CHANNEL_X, ANIM_CHANNEL_Y, ANIM_CHANNEL_Z, ANIM_CHANNEL_Q] {
            if g.coin() {
                continue;
            }
            let q = ty == ANIM_CHANNEL_Q;
            let vlen: u8 = match g.below(16) {
                0 if !g.clean => [1u8, 2, 3, 5][g.below(4) as usize],
                _ if q => 4,
                _ => 1,
            };
            let mut b = Vec::new();
            if adaptive {
                let frames = if g.below(8) == 0 { 1 + g.below(40) } else { nf.max(1) };
                w32(&mut b, frames);
                b.extend_from_slice(&(pivot as u16).to_le_bytes());
                b.extend_from_slice(&[vlen, ty as u8]);
                let scale = if g.below(6) == 0 { g.f32() } else { g.g.uniform(0.0001, 0.5) };
                w32(&mut b, scale.to_bits());
                let mut d = Vec::new();
                let base = ev_quat(g);
                for k in 0..vlen as usize {
                    let v = if q && vlen == 4 { base[k] } else { g.g.uniform(-3.0, 3.0) };
                    d.extend_from_slice(&v.to_bits().to_le_bytes());
                }
                let mut groups = frames.div_ceil(16) as usize;
                match g.below(10) {
                    0 if !g.clean => groups = groups.saturating_sub(1),
                    1 => groups += 1,
                    _ => {}
                }
                for _ in 0..groups * vlen as usize {
                    d.push(match g.below(4) {
                        0 => g.below(16) as u8,
                        _ => 16 + g.below(240) as u8,
                    });
                    d.extend((0..8).map(|_| g.g.u32() as u8));
                }
                while d.len() % 4 != 0 {
                    d.push(g.g.u32() as u8);
                }
                b.extend(d);
            } else {
                let k = 1 + g.below(6);
                w32(&mut b, k);
                b.extend_from_slice(&(pivot as u16).to_le_bytes());
                b.extend_from_slice(&[vlen, ty as u8]);
                let mut t = if g.chance(6) { 1 + g.below(5) } else { 0 };
                let mut words = Vec::new();
                for i in 0..k {
                    let flag = if i > 0 && g.below(5) == 0 { W3D_TIMECODED_BINARY_MOVEMENT_FLAG } else { 0 };
                    words.push(t | flag);
                    let quat = ev_quat(g);
                    for c in 0..vlen as usize {
                        let v = if q && vlen == 4 { quat[c] } else { g.g.uniform(-3.0, 3.0) };
                        words.push(v.to_bits());
                    }
                    t = match g.below(12) {
                        0 if !g.clean => t.saturating_sub(g.below(4)),
                        _ => t + 1 + g.below(8),
                    };
                }
                if g.chance(15) {
                    let cut = 1 + g.below(2) as usize;
                    words.truncate(words.len().saturating_sub(cut).max(1));
                }
                for w in words {
                    w32(&mut b, w);
                }
            }
            body.extend(hchunk(W3D_CHUNK_COMPRESSED_ANIMATION_CHANNEL, &b));
        }
        if g.below(3) == 0 {
            let k = 1 + g.below(5);
            let mut b = Vec::new();
            w32(&mut b, k);
            b.extend_from_slice(&(pivot as u16).to_le_bytes());
            b.extend_from_slice(&[BIT_CHANNEL_VIS as u8, g.below(2) as u8]);
            let mut t = if g.below(5) == 0 { 2 } else { 0 };
            for _ in 0..k {
                w32(&mut b, t | if g.coin() { W3D_TIMECODED_BIT_MASK } else { 0 });
                t += 1 + g.below(6);
            }
            body.extend(hchunk(W3D_CHUNK_COMPRESSED_BIT_CHANNEL, &b));
        }
    }
    hchunk(W3D_CHUNK_COMPRESSED_ANIMATION, &body)
}

fn ev_morph(g: &mut Gn, hier: &[u8; W3D_NAME_LEN], npiv: u32, poses: &[(Vec<u8>, u32)]) -> Vec<u8> {
    let channels = 1 + g.below(3);
    let mut h = W3dMorphAnimHeaderStruct::from_bytes(&[0; 48]);
    h.version = W3D_CURRENT_MORPH_HANIM_VERSION;
    h.name = name16(b"MORPH");
    h.hierarchy_name = *hier;
    h.frame_count = 30;
    h.frame_rate = 30.0;
    h.channel_count = channels;
    let mut body = hchunk(W3D_CHUNK_MORPHANIM_HEADER, &h.to_bytes());
    for _ in 0..channels {
        let (mut pose, pf) = if poses.is_empty() || g.chance(12) {
            (b"NOPE".to_vec(), 4)
        } else {
            poses[g.below(poses.len() as u32) as usize].clone()
        };
        pose.push(0);
        let mut cb = hchunk(W3D_CHUNK_MORPHANIM_POSENAME, &pose);
        let mut kb = Vec::new();
        let mut m: u32 = if g.below(6) == 0 { 2 } else { 0 };
        for _ in 0..1 + g.below(4) {
            kb.extend(m.to_le_bytes());
            kb.extend(g.below(pf + 2).to_le_bytes());
            m = match g.below(10) {
                0 if !g.clean => m.saturating_sub(3),
                _ => m + 1 + g.below(10),
            };
        }
        cb.extend(hchunk(W3D_CHUNK_MORPHANIM_KEYDATA, &kb));
        body.extend(hchunk(W3D_CHUNK_MORPHANIM_CHANNEL, &cb));
    }
    let pv: Vec<u8> = (0..npiv).flat_map(|_| (if g.chance(20) { channels } else { g.below(channels) }).to_le_bytes()).collect();
    body.extend(hchunk(W3D_CHUNK_MORPHANIM_PIVOTCHANNELDATA, &pv));
    hchunk(W3D_CHUNK_MORPH_ANIMATION, &body)
}

/// A file: one or two hierarchies, one to four raw or compressed
/// animations on them, then up to two morph animations over those.
fn ev_file(g: &mut Gn) -> Vec<u8> {
    let trees: Vec<([u8; W3D_NAME_LEN], u32)> =
        (0..1 + g.below(2)).map(|i| (name16(if i == 0 { b"SKEL" } else { b"SK2" }), 1 + g.below(6))).collect();
    let mut f = Vec::new();
    for (n, p) in &trees {
        f.extend(ev_tree(g, n, *p));
    }
    let mut poses = Vec::new();
    for i in 0..1 + g.below(4) {
        let (hier, npiv) = trees[g.below(trees.len() as u32) as usize];
        let name = format!("A{i}");
        let full = [&hier[..hier.iter().position(|&c| c == 0).unwrap()], b".", name.as_bytes()].concat();
        if g.coin() {
            f.extend(ev_raw(g, &hier, npiv, name.as_bytes()));
        } else {
            f.extend(ev_compressed(g, &hier, npiv, name.as_bytes()));
        }
        poses.push((full, 10));
    }
    for _ in 0..g.below(3) {
        let (hier, npiv) = trees[g.below(trees.len() as u32) as usize];
        f.extend(ev_morph(g, &hier, npiv, &poses));
    }
    f
}

fn ev_frame(g: &mut Gn, nf: i32) -> f32 {
    let nf = nf.clamp(0, 100) as f32;
    if g.clean {
        return match g.below(10) {
            0..=3 => g.below(nf as u32 + 3) as f32,
            4 => g.below(nf as u32 + 3) as f32 + 0.5,
            _ => g.g.uniform(0.0, nf + 1.5),
        };
    }
    match g.below(20) {
        0 => g.g.uniform(-3.0, 0.0),
        1 => g.f32(),
        2 => f32::NAN,
        3..=7 => g.below(nf as u32 + 3) as f32,
        8 => g.below(nf as u32 + 3) as f32 + 0.5,
        _ => g.g.uniform(-0.5, nf + 1.5),
    }
}

fn ev_matrix(g: &mut Gn, s: &mut Vec<u8>) {
    let m: Vec<f32> = if g.coin() {
        let q = ev_quat(g);
        let mut m = rts_lockstep_engine::ww::quat::build_matrix3d(&rts_lockstep_engine::ww::quat::Quaternion::new(q[0], q[1], q[2], q[3]));
        m.set_translation(&rts_lockstep_engine::ww::vector3::Vector3::new(g.g.uniform(-5.0, 5.0), g.g.uniform(-5.0, 5.0), g.g.uniform(-5.0, 5.0)));
        m.to_array().to_vec()
    } else {
        (0..12).map(|_| g.g.uniform(-2.0, 2.0)).collect()
    };
    for v in m {
        w32(s, v.to_bits());
    }
}

fn ev_script(g: &mut Gn, w: &EvalWorld) -> Vec<u8> {
    let mut s = Vec::new();
    let na = w.anims.len() as u32;
    let nt = w.trees.len() as u32;
    let raws: Vec<u32> = (0..na).filter(|&i| w.anims[i as usize].1 == 0).collect();
    for _ in 0..1 + g.below(40) {
        let op = 1 + g.below(11) as u8;
        let needs_anim = matches!(op, 1..=7 | 9);
        let needs_tree = op >= 5;
        if (needs_anim && na == 0) || (needs_tree && nt == 0) || (op == 6 && raws.is_empty()) {
            continue;
        }
        s.push(op);
        let anim = |g: &mut Gn| g.below(na);
        let pivot = |g: &mut Gn, n: i32| -> u32 {
            match if g.clean { 20 } else { g.below(20) } {
                0 => u32::MAX,
                1 => (n + 1) as u32,
                _ => g.below(n.clamp(1, 1000) as u32),
            }
        };
        match op {
            1..=4 => {
                let a = anim(g);
                let an = &w.anims[a as usize].0;
                w32(&mut s, a);
                w32(&mut s, pivot(g, an.get_num_pivots()));
                w32(&mut s, ev_frame(g, an.get_num_frames()).to_bits());
            }
            5 | 6 => {
                w32(&mut s, g.below(nt));
                let a = if op == 6 { raws[g.below(raws.len() as u32) as usize] } else { anim(g) };
                w32(&mut s, a);
                w32(&mut s, ev_frame(g, w.anims[a as usize].0.get_num_frames()).to_bits());
                ev_matrix(g, &mut s);
            }
            7 => {
                w32(&mut s, g.below(nt));
                for _ in 0..2 {
                    let a = anim(g);
                    w32(&mut s, a);
                    w32(&mut s, ev_frame(g, w.anims[a as usize].0.get_num_frames()).to_bits());
                }
                let pct = match g.below(6) {
                    0 => 0.0,
                    1 => 1.0,
                    _ => g.g.uniform(-0.2, 1.2),
                };
                w32(&mut s, f32::to_bits(pct));
                ev_matrix(g, &mut s);
            }
            8 => {
                w32(&mut s, g.below(nt));
                ev_matrix(g, &mut s);
            }
            9 => {
                let t = g.below(nt);
                w32(&mut s, t);
                let a = anim(g);
                w32(&mut s, a);
                w32(&mut s, pivot(g, w.trees[t as usize].num_pivots));
                w32(&mut s, ev_frame(g, w.anims[a as usize].0.get_num_frames()).to_bits());
                ev_matrix(g, &mut s);
            }
            10 => {
                let t = g.below(nt);
                w32(&mut s, t);
                w32(&mut s, pivot(g, w.trees[t as usize].num_pivots));
                ev_matrix(g, &mut s);
            }
            _ => {
                let t = g.below(nt);
                w32(&mut s, t);
                w32(&mut s, pivot(g, w.trees[t as usize].num_pivots));
                w32(&mut s, u32::from(g.below(4) != 0));
                w32(&mut s, g.below(2));
                ev_matrix(g, &mut s);
            }
        }
    }
    s
}

/// (#162) A script of combo ops (12 to 31; see `ref_w3d_eval`) over the
/// file's hierarchies and animations: combos of 0 to 5 entries made by
/// either constructor and by appending shared entries, their motions,
/// frames, weights and pivot maps set (plain maps of 0 to 8 weights, named
/// maps fitted to a hierarchy, active maps), then `Combo_Update`s,
/// `Normalize_Weights`, removals, clears and resets, and dumps. A tame
/// script (every other file) gives every entry an animation before it is
/// updated, keeps indices in range and fits its maps; a wild one does not.
fn combo_script(g: &mut Gn, w: &EvalWorld) -> Vec<u8> {
    let mut s = Vec::new();
    let na = w.anims.len() as u32;
    let nt = w.trees.len() as u32;
    if na == 0 || nt == 0 {
        return s;
    }
    // the generator's view: each combo's entries (None: its own, Some(d): shared entry d), the maps' kinds
    let mut combos: Vec<Vec<Option<u32>>> = Vec::new();
    let mut maps: Vec<bool> = Vec::new();
    let mut datas = 0u32;
    let weight = |g: &mut Gn| -> f32 {
        match g.below(10) {
            0 => 0.0,
            1 | 2 => 1.0,
            3 => 0.5,
            4 => 0.25,
            5 => 1.0 / 3.0,
            6 if !g.clean => g.f32(),
            _ => g.g.uniform(-0.5, 1.5),
        }
    };
    let op = |s: &mut Vec<u8>, code: u8, words: &[u32]| {
        s.push(code);
        for &v in words {
            w32(s, v);
        }
    };
    // a new combo, its entries given animations, frames and weights
    let new_combo = |g: &mut Gn, s: &mut Vec<u8>, combos: &mut Vec<Vec<Option<u32>>>| {
        let n = if g.below(6) == 0 { u32::MAX } else { g.below(5) };
        op(s, 12, &[n]);
        let c = combos.len() as u32;
        combos.push(vec![None; if n == u32::MAX { 0 } else { n as usize }]);
        for i in 0..combos[c as usize].len() as u32 {
            if !g.chance(8) {
                op(s, 13, &[c, i, g.below(na)]);
            }
            let a = g.below(na);
            op(s, 14, &[c, i, ev_frame(g, w.anims[a as usize].0.get_num_frames()).to_bits()]);
            if g.coin() {
                op(s, 16, &[c, i, weight(g).to_bits()]);
            }
        }
    };
    for _ in 0..1 + g.below(2) {
        new_combo(g, &mut s, &mut combos);
    }
    for _ in 0..2 + g.below(30) {
        let c = g.below(combos.len() as u32);
        let n = combos[c as usize].len() as u32;
        let entry = |g: &mut Gn| -> u32 {
            if n == 0 || g.chance(20) {
                n + g.below(2)
            } else {
                g.below(n)
            }
        };
        match g.below(24) {
            0 => new_combo(g, &mut s, &mut combos),
            1 if n > 0 || !g.clean => op(&mut s, 13, &[c, entry(g), if g.chance(6) { u32::MAX } else { g.below(na) }]),
            2 | 3 if n > 0 || !g.clean => {
                let a = g.below(na);
                let code = if g.below(4) == 0 { 15 } else { 14 };
                op(&mut s, code, &[c, entry(g), ev_frame(g, w.anims[a as usize].0.get_num_frames()).to_bits()]);
            }
            4 | 5 if n > 0 || !g.clean => op(&mut s, 16, &[c, entry(g), weight(g).to_bits()]),
            6 | 7 => {
                // a map: plain (0 to 8 weights) or named (fitted to a tree, perhaps with names)
                let named = g.coin();
                op(&mut s, 18, &[u32::from(named)]);
                let m = maps.len() as u32;
                maps.push(named);
                if named {
                    for _ in 0..g.below(4) {
                        op(&mut s, 20, &[m, g.below(COMBO_NAMES.len() as u32), weight(g).to_bits()]);
                    }
                    op(&mut s, 21, &[m, g.below(nt)]);
                } else {
                    let len = if g.clean { 8 } else { g.below(9) };
                    for _ in 0..len {
                        let v = match g.below(4) {
                            0 => 0.0,
                            1 => 1.0,
                            2 => 0.5,
                            _ => g.g.uniform(0.0, 1.0),
                        };
                        op(&mut s, 19, &[m, v.to_bits()]);
                    }
                }
                if n > 0 && g.coin() {
                    op(&mut s, 17, &[c, entry(g), m]);
                }
            }
            8 if !maps.is_empty() && (n > 0 || !g.clean) => {
                let m = if g.below(5) == 0 { u32::MAX } else { g.below(maps.len() as u32) };
                op(&mut s, 17, &[c, entry(g), m]);
            }
            9 if !maps.is_empty() => {
                let m = g.below(maps.len() as u32);
                if maps[m as usize] {
                    op(&mut s, 20, &[m, g.below(COMBO_NAMES.len() as u32), weight(g).to_bits()]);
                } else if !g.clean {
                    op(&mut s, 19, &[m, weight(g).to_bits()]);
                }
                op(&mut s, 21, &[m, g.below(nt)]);
            }
            10 if n > 0 || !g.clean => op(&mut s, 22, &[c, entry(g)]),
            11 | 12 => op(&mut s, 23, &[c]),
            13..=17 => {
                op(&mut s, 24, &[g.below(nt), c]);
                ev_matrix(g, &mut s);
            }
            18 => {
                op(&mut s, 25, &[]);
                let d = datas;
                datas += 1;
                op(&mut s, 26, &[c, d]);
                combos[c as usize].push(Some(d));
                let i = combos[c as usize].len() as u32 - 1;
                if !g.chance(6) {
                    op(&mut s, 13, &[c, i, g.below(na)]);
                }
                op(&mut s, 16, &[c, i, weight(g).to_bits()]);
            }
            19 if datas > 0 => {
                let d = g.below(datas);
                if g.coin() {
                    op(&mut s, 26, &[c, d]);
                    combos[c as usize].push(Some(d));
                } else {
                    op(&mut s, 27, &[c, d]);
                    if let Some(k) = combos[c as usize].iter().position(|&e| e == Some(d)) {
                        combos[c as usize].remove(k);
                    }
                }
            }
            20 if !g.clean => op(&mut s, 28, &[c]),
            21 if !g.clean || g.below(4) == 0 => {
                op(&mut s, 29, &[c]);
                combos[c as usize].clear();
            }
            22 if !maps.is_empty() => op(&mut s, 31, &[g.below(maps.len() as u32)]),
            _ => op(&mut s, 30, &[c]),
        }
    }
    s
}

/// Section 13's combos (#162): `HTreeClass::Combo_Update` and
/// `hanim.cpp`'s `HAnimComboClass`, `HAnimComboDataClass`, `PivotMapClass`
/// and `NamedPivotMapClass` (the combo ops of `ref_w3d_eval`), compiled
/// from upstream's own `htree.cpp` and `hanim.cpp` (`build.rs`), against
/// [`HTree::combo_update`] and [`rts_lockstep_engine::formats::w3d::
/// hanim_combo`]. The files are section 13's (the same generator, another
/// seed); the scripts are [`combo_script`]'s, every result compared as raw
/// bits (each transform's 12 floats, each weight and frame), the portable
/// `Float_To_Long` pair as [`animation_evaluation_matches_upstream`].
///
/// Predicted before the run (`issue162_predictions.md`): 0 mismatches,
/// max ULP 0, since the Rust adds `weight * ScaleFactor * translation` and
/// `Fast_Slerp`s at `weight / weight_total` in upstream's order; scripts
/// stopping mostly at `IndexOutOfRange` (entries and maps out of range, maps
/// shorter than the pivots), `NullPointer` (an entry without an animation
/// at `Combo_Update` or `Normalize_Weights`) and the section's own
/// undefined animation kinds.
///
/// Results (2026-11-12). First run, 6,000 files: 0 mismatches, but the
/// test binary died with SIGSEGV at exit (a 64-bit artefact of the
/// harness: `HAnimComboDataClass`'s `ObjectPoolClass` links its blocks with
/// an 8-byte pointer and puts the first object 4 bytes in; see
/// `wrap_w3d_eval.cpp`, which now makes the pool forget its block list).
/// Then 20,000 files: 1 mismatching file, the same NaNs with opposite signs
/// in a `Combo_Update` (a wild script's NaN weight): see
/// [`NAN_NOT_COMPARED`]; such results are no longer compared, and a
/// `Fast_Slerp` of a NaN cosine is undefined (`Fast_Acos`'s table). Final:
/// 20,000 files, all run (9,697 tame scripts to the end), 502,984 of
/// 708,711 ops compared, among them 39,192 `Combo_Update`s, 16,699
/// `Normalize_Weights`, 5,961 `Build_Active_Pivot_Map`s, 12,719
/// `Update_Pivot_Map`s and 57,910 dumps; **0 mismatches, max ULP 0**.
/// Scripts stopped at undefined upstream: 9,100 (`IndexOutOfRange` 5,532,
/// `NullPointer` 3,074, `FloatToInt` 256, `NonTerminating` 135,
/// `UninitialisedRead` 93) and 10 at a NaN result.
///
/// Planted bugs (`L9_PLANT`, mismatching files of 20,000): 9571 (no slerp:
/// every orientation replaces the last) 4,273; 9572 (the pivot map's weight
/// ignored) 1,206; 9573 (the translation not weighted) 5,418; 9574 (`Update_Pivot_Map` applies the pairs first to last) 987;
/// 9575 (`Get_Bone_Index` gives -1 for an unknown name) 1,280; 9576
/// (`Set_Frame` keeps the previous frame) 3,081; 9577 (`Clear` clears
/// shared entries too) 37; 9578 (`Build_Active_Pivot_Map` all 1) 346; 9579
/// (`weight_total > 0` for `!= 0`) 304; 9580 (the visibility of the last
/// animation, not the first visible one) 1,563; 9593 (the per-pivot
/// normalisation leaves the maps) 424: **11 of 11 caught**. The first 9573 (the fewest pivots
/// taken as the most) made the Rust skip work, so the C++ ran ops the Rust
/// had not checked and the process died; replaced by the unweighted
/// translation.
#[test]
fn combo_evaluation_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    eval_cases(FloatToLongMode::PortableTruncate, 0xC0B0, 20_000, "W3D anim combos (hanim.cpp, Combo_Update)", combo_script);
}

/// (#179) A script of render-object ops (40-49; see `ref_w3d_eval`) over the file's hierarchies and animations: render
/// objects made on the hierarchies a skeleton can hold ([`robj_ok`]) at a scale (1, 0.5, 2, within `scale_epsilon` of 1,
/// or any in 0.25..4), then, in any order, the sync time moved on (by 0, a few ms, a frame, seconds; a wild script also
/// moves it back and far), `Set_Animation` of any animation (or `NULL`) at a frame ([`ev_frame`]) in any of the six
/// modes, `Set_Animation()`, a blend of two animations, `Get_Bone_Transform` of any bone, `Set_Transform`, a bone
/// captured and controlled (or released), `Render` (hidden 1 in 5) and the frame-rate multiplier. Each op is followed by
/// the render object's state (mode, `IsTreeValid`, `ModeAnim`).
fn animobj_script(g: &mut Gn, w: &EvalWorld) -> Vec<u8> {
    let mut s = Vec::new();
    let na = w.anims.len() as u32;
    let ok: Vec<u32> = (0..w.trees.len() as u32).filter(|&t| robj_ok(&w.trees[t as usize])).collect();
    if na == 0 || ok.is_empty() {
        return s;
    }
    let mut robjs: Vec<u32> = Vec::new();
    let mut now = g.below(1000);
    s.push(41);
    w32(&mut s, now);
    for _ in 0..1 + g.below(60) {
        if robjs.is_empty() || g.below(14) == 0 {
            let t = ok[g.below(ok.len() as u32) as usize];
            s.push(40);
            w32(&mut s, t);
            let scale = match g.below(8) {
                0..=2 => 1.0f32,
                3 => 0.5,
                4 => 2.0,
                5 => [0.995f32, 1.005, 0.9899, 1.0101][g.below(4) as usize],
                _ => g.g.uniform(0.25, 4.0),
            };
            w32(&mut s, scale.to_bits());
            robjs.push(w.trees[t as usize].num_pivots as u32);
            continue;
        }
        let ri = g.below(robjs.len() as u32);
        let npiv = robjs[ri as usize];
        let anim = |g: &mut Gn| g.below(na);
        match g.below(100) {
            0..=14 => {
                now = match g.below(if g.clean { 8 } else { 10 }) {
                    0 => now,
                    1..=3 => now.wrapping_add(g.below(50)),
                    4 | 5 => now.wrapping_add(33),
                    6 => now.wrapping_add(100 + g.below(2000)),
                    7 => now.wrapping_add(g.below(100_000)),
                    8 => now.wrapping_sub(1 + g.below(200)),
                    _ => g.g.u32(),
                };
                s.push(41);
                w32(&mut s, now);
            }
            15..=29 => {
                s.push(42);
                w32(&mut s, ri);
                let a = if g.below(12) == 0 { u32::MAX } else { anim(g) };
                w32(&mut s, a);
                let nf = if a == u32::MAX { 10 } else { w.anims[a as usize].0.get_num_frames() };
                w32(&mut s, ev_frame(g, nf).to_bits());
                w32(&mut s, g.below(6));
            }
            30..=32 => {
                s.push(43);
                w32(&mut s, ri);
            }
            33..=37 => {
                s.push(44);
                w32(&mut s, ri);
                for _ in 0..2 {
                    let a = anim(g);
                    w32(&mut s, a);
                    w32(&mut s, ev_frame(g, w.anims[a as usize].0.get_num_frames()).to_bits());
                }
                let pct = match g.below(6) {
                    0 => 0.0f32,
                    1 => 1.0,
                    _ => g.g.uniform(-0.2, 1.2),
                };
                w32(&mut s, pct.to_bits());
            }
            38..=62 => {
                s.push(45);
                w32(&mut s, ri);
                w32(&mut s, g.below(npiv));
            }
            63..=70 => {
                s.push(46);
                w32(&mut s, ri);
                ev_matrix(g, &mut s);
            }
            71..=78 => {
                s.push(47);
                w32(&mut s, ri);
                w32(&mut s, g.below(npiv));
                w32(&mut s, u32::from(g.below(4) != 0));
                w32(&mut s, g.below(2));
                ev_matrix(g, &mut s);
            }
            79..=94 => {
                s.push(48);
                w32(&mut s, ri);
                w32(&mut s, u32::from(g.below(5) == 0));
            }
            _ => {
                s.push(49);
                w32(&mut s, ri);
                let m = match g.below(6) {
                    0 => 1.0f32,
                    1 => 0.5,
                    2 => 2.0,
                    3 => -1.0,
                    4 => 0.0,
                    _ => g.g.uniform(-3.0, 3.0),
                };
                w32(&mut s, m.to_bits());
            }
        }
    }
    s
}

/// GitHub #179: `Animatable3DObjClass`'s bone query with the animation's current pose (animobj.cpp:658-675
/// `Get_Bone_Transform`, 780-860 `Update_Sub_Object_Transforms`, 1048-1075 `Single_Anim_Progress`, 949-1045
/// `Compute_Current_Frame`, 288-305 `Render`, the `Set_Animation`s, `Set_Transform`, `Capture_Bone`, `Release_Bone`,
/// `Control_Bone`, and animobj.h's inline updates, cut verbatim into `wrap_w3d_eval.cpp`) against the port's
/// `client::w3d_model_draw::RenderObj`, over section 13's random files (the real loaders and the real `htree.cpp`,
/// `hrawanim.cpp`, `hcanim.cpp`, `hmorphanim.cpp`, `motchan.cpp` on the C++ side) with [`animobj_script`]'s scripts,
/// the portable `Float_To_Long` on both sides. Compared as raw bits after every op: each bone's 12 floats, and each render
/// object's mode, `IsTreeValid` and `ModeAnim` (frame, previous frame, last sync time, direction, multiplier, mode,
/// `Is_Animation_Complete`). A script stops before an op the Rust reports undefined upstream.
///
/// Predictions (`issue179_predictions.md`, before the first run): 0 mismatches, max ULP 0; a few scripts stopped at
/// `FloatToInt` (compressed animations at negative or NaN frames, wild scripts) and at the wild files' undefined
/// evaluations.
///
/// Results (2026-11-13). **First run:** 444 of 20,000 files mismatched, all in `Get_Bone_Transform`'s translations
/// (rotations equal): the test gave each `Set_Animation` a *clone* of the animation, so the compressed channels' caches
/// started fresh, where upstream's render objects share one `HAnimClass` (the port's do too: the asset manager's
/// `Arc`). The test's error, fixed ([`robj_handle`] shares one `AnimData` per animation). **Second run:** 20,000 files,
/// 20,000 run (9,446 tame scripts to the end), 494,431 of 595,883 ops compared (40: 51,850; 41: 82,985; 42: 64,459;
/// 43: 12,700; 44: 21,289; 45: 104,412; 46: 34,244; 47: 34,220; 48: 66,979; 49: 21,293), **0 mismatches, max ULP 0**.
/// Scripts stopped at undefined upstream: 4,429 (op 45: `IndexOutOfRange` 1,531, `FloatToInt` 841, `NonTerminating`
/// 248, `NullPointer` 178, `UninitialisedRead` 175; op 48 (`Render`'s update): 813, 320, 143, 101, 79): the wild
/// files' evaluations, as section 13's. Predictions hit, apart from the first run's test error.
///
/// Planted bugs (`L9_PLANT`, mismatching files of 20,000; those marked * hung the C++ before the end of the full run,
/// because the planted Rust stopped predicting an evaluation the C++ then ran undefined, so they were run on 300 cases,
/// or 60 for 9808): 9801 `Get_Bone_Transform` always re-evaluates 2,991; 9802* `Set_Transform` does not invalidate 139 of
/// 300; 9803* no `Single_Anim_Progress` in the update 102 of 300; 9804 progress in `MANUAL` too 1,401; 9805 a raw
/// animation through the virtual `Anim_Update` 2,148; 9806* `Single_Anim_Progress` does not invalidate 8 of 300; 9807
/// `Control_Bone` does not invalidate 7,569; 9808* `Set_Animation` does not invalidate 37 of 60; 9809* `Render` does
/// not update 14 of 20; 9810 the pose under the identity 10,114; 9811 the blend's percentage reversed 3,201; 9813 the
/// scale factor not carried 4,924; 9814 no `PrevFrame` restore 8,066; 9815 `Set_Animation()` keeps the animation
/// 5,124; 9817 `Release_Bone` does nothing 181; 9818 the sync-time difference signed 4 of 300 (only a wild script's
/// backward sync time reaches it); 9819 the live hierarchy made afresh each update (no previous transforms) 429; and
/// #172/#174's, moved into the live hierarchy: 9694 6,374, 9698 7,848, 9751 4,077. 9812 (`Combo_Update` replaced by
/// the base pose) and 9816 (`get_bone_transform_under` restores without invalidating) are not reachable here: caught by
/// the lib test `a_render_object_s_bones_follow_its_animation` (`--features differential`), which also catches 9801,
/// 9805, 9810 and 9811.
#[test]
fn animatable_bones_match_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let n = std::env::var("W3D_ANIMOBJ_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000);
    eval_cases(FloatToLongMode::PortableTruncate, 0x179_A0B1, n, "W3D render objects' animated bones (animobj.cpp)", animobj_script);
}

/// Random W3D files (one or two hierarchies of 1 to 6 pivots, sometimes
/// pre-3.0, with random base poses and parents, now and then a parent that
/// is missing, the pivot itself, a later pivot or one past the end; one to
/// four raw or compressed animations on them: raw channels of every kind
/// with vectors of the usual lengths and sometimes others, bit channels;
/// time-coded channels with increasing (sometimes not) time codes, binary
/// movements and sometimes too few words; adaptive-delta channels with
/// random packets (filter indices across the whole table), sometimes a
/// packet group short or extra; now and then an unknown flavor; then up to
/// two morph animations whose channels name those animations, or an
/// unknown one, with keys and pivot channels), loaded on both sides by the
/// real loaders, then a random script of up to 40 evaluations, at integer,
/// half, fractional, negative, out-of-range and NaN frames: the four
/// sampling functions of every animation kind, `Anim_Update` (both
/// overloads), `Blend_Update`, `Base_Update`, both
/// `Simple_Evaluate_Pivot`s, and bone capture and release, all against the
/// same objects in sequence (so the channels' caches carry over from op to
/// op). Every result compared as raw bits. A script stops before the first
/// op on which the Rust reports upstream undefined (counted per kind); a
/// file whose loading is undefined is not run.
///
/// Predicted before the run: no disagreement (the C++ is x86-64 SSE with
/// `-ffp-contract=off`, the Rust `f32` in the same order, and the WWMath
/// tables and `sinf` bound to the same petir functions). Excluded op
/// kinds: mostly `IndexOutOfRange` (pivot indices out of range, short
/// time-coded data, adaptive-delta packets past the data, `Get_QuatVector`
/// of vectors shorter than 4) and `FloatToInt` (negative and NaN frames of
/// compressed animations), some `NullPointer` (missing parents, unknown
/// poses), `UninitialisedRead` (raw quaternions of 3, unknown flavors under
/// `Anim_Update`), and a few `NonTerminating` (time codes that decrease).
///
/// Results (2026-11-05). First run (one wild generator, every file with
/// anomalies): 5,848 files run, 28 mismatching, all in components 1 to 3 of
/// adaptive-delta quaternions. Cause (prediction missed): upstream's
/// `decompress` shares one `done` flag across the components, so
/// components after the first stop after one packet; the Rust had reset it
/// per component. Translated as upstream does it (see `motchan.rs`; unit
/// test `adaptive_delta_shared_done_flag`). Then 0 mismatches, but only
/// 21,842 of 117,046 ops compared (scripts stopped early). The generator
/// was given a tame half (no anomalies: in-range pivots and bones, frames
/// from 0 to `NumFrames + 1.5`, valid parents, complete data, known
/// flavors, found poses). Final run: 6,000 files, all run (2,609 of the
/// 3,000 tame scripts to the end), 66,396 of 121,232 ops compared (about
/// 6,000 of each kind), **0 mismatches, max ULP 0**. Scripts stopped at
/// undefined upstream: 3,037, by kind about 1,900 `IndexOutOfRange`, 410
/// `FloatToInt`, 380 `NullPointer`, 230 `NonTerminating` (cycles in
/// `Simple_Evaluate_Pivot`'s parent walk and time-code searches), 76
/// `UninitialisedRead`. Planted bugs: computing `getframe`'s last case from
/// the start instead of with upstream's double-counted delta gives 506
/// mismatching files; rounding raw frames to nearest (the shipped game's
/// `fistp`) instead of the portable truncation makes the C++ reach a crash
/// the Rust no longer predicts. **Corrected 2026-11-06 (#19):** that
/// was a Rust branch compared with a different C++ branch. The Rust now
/// has both branches ([`FloatToLongMode`]), and the C++ both too
/// (`build.rs`, `wrap_w3d.cpp`): this test runs the portable pair,
/// [`animation_evaluation_win32_matches_upstream`] the Win32 pair, and the
/// crash is explained there. Debugging: `W3D_CASE=<n>` prints a case's
/// animations and ops, `W3D_TRACE` every case's file and script.
#[test]
fn animation_evaluation_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    animation_evaluation(FloatToLongMode::PortableTruncate);
}

/// Restores the C++ `Float_To_Long` branch to the portable one (0) when a
/// test that changed it ends, panicking or not.
struct CppFloatToLongMode;

impl CppFloatToLongMode {
    fn set(mode: FloatToLongMode) -> Self {
        let m = match mode {
            FloatToLongMode::PortableTruncate => 0,
            FloatToLongMode::Win32Fistp => 1,
        };
        unsafe { cpp::ref_w3d_set_float_to_long_mode(m) };
        CppFloatToLongMode
    }
}

impl Drop for CppFloatToLongMode {
    fn drop(&mut self) {
        unsafe { cpp::ref_w3d_set_float_to_long_mode(0) };
    }
}

/// Section 13 again (#19) with the shipped Win32 `Float_To_Long` on both
/// sides: the Rust's [`FloatToLongMode::Win32Fistp`] (the client's
/// default) against the same compiled upstream `hrawanim.cpp` and
/// `htree.cpp`, whose `Float_To_Long` calls now run the `fld`/`fistp` pair
/// itself (see `build.rs` and `wrap_w3d.cpp`). Same seed, so the same files
/// and scripts as [`animation_evaluation_matches_upstream`].
///
/// Predicted before the run: 0 mismatches. Fewer scripts stop at
/// `FloatToInt`, because NaN and out-of-range frames of *raw* animations
/// are defined under `fistp` (the integer indefinite); the compressed
/// animations' `uint32` casts stay undefined, so `FloatToInt` does not
/// vanish. The earlier finding (#10, the planted bug above: the C++
/// reached a crash the Rust did not predict) is expected not to recur:
/// it is a property of comparing two different branches, not of either.
///
/// Results (2026-11-06): 6,000 files, all run (2,609 tame scripts to the
/// end), 67,413 of 121,232 ops compared (portable: 66,396), **0
/// mismatches, max ULP 0**. Scripts stopped at undefined upstream: 3,007
/// (portable 3,037), `FloatToInt` 247 (portable 410; none left in op 6,
/// the raw fast path, which only converts through `Float_To_Long`; the
/// rest are `Get_Visibility`'s `(int)frame` and the compressed channels'
/// `uint32`). Predictions hit.
///
/// The earlier finding, investigated (#19). No crash recurs with the
/// branches matched, and none occurred in either *crossed* run either
/// (Rust Win32 against C++ portable: 2,244 of 6,000 files mismatch, 2,245
/// when run after the portable test in the same process;
/// Rust portable against C++ Win32: 2,193), measured with a temporary
/// hook, not kept. What a crossed run sends the C++ was counted by also
/// running the Rust in the C++'s branch over the same prefix: ops the
/// Rust's own branch found defined but the other branch finds undefined
/// are 147 `FloatToInt` (NaN and out-of-range raw frames: defined under
/// `fistp`, C++-undefined `(long)f` under truncation, which x86-64 happens
/// to turn into 0, no crash) and 26 `IndexOutOfRange` and 2
/// `UninitialisedRead` (Rust Win32 against C++ portable; the other way, 20
/// and 1). Those are the
/// frame-dependent undefined ops of `motchan.h`: a channel whose vector is
/// longer than its slot writes past the `Vector3` or `Matrix3D` on the
/// stack, and a 3-float quaternion is read as 4 (`Get_Vector_As_Quat`, or
/// `Get_Transform`'s uninitialised `vals[3]`), but only at frames inside
/// the channel; outside it the identity is written. A different branch
/// samples a different frame, so the C++ ran such ops at frames the Rust
/// had checked at another frame; a stack overrun of that kind is the
/// likely crash of the #10 planted run. It is not a negative or
/// out-of-range index: rounding's frame -1 and the indefinite `i32::MIN`
/// fall below every channel's `FirstFrame` and give the identity, in both
/// languages. With the branches matched, each of those ops is reported
/// undefined by the Rust (the kinds above) and never sent: nothing is
/// changed in behaviour, and the exclusion is counted in the printed line.
#[test]
fn animation_evaluation_win32_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _m = CppFloatToLongMode::set(FloatToLongMode::Win32Fistp);
    animation_evaluation(FloatToLongMode::Win32Fistp);
}

/// `WWMath::Float_To_Long(float)`'s Win32 branch (#19): the Rust's
/// integer-only [`wwmath::float_to_long_fistp`] against a real x87
/// `fld`/`fistp` (`ref_w3d_float_to_long` in mode 1), on every tie `k +
/// 0.5` for `|k| <= 4096`, the neighbours of each, values around +-2^31,
/// the specials (zeros, subnormals, infinities, NaNs) and 2^20 random bit
/// patterns. Predicted before the run: 0 mismatches.
#[test]
fn float_to_long_fistp_matches_x87() {
    use rts_lockstep_engine::ww::wwmath;
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _m = CppFloatToLongMode::set(FloatToLongMode::Win32Fistp);
    let mut c = Check::new("WWMath::Float_To_Long (Win32 fld/fistp)");
    let mut xs: Vec<f32> = vec![0.0, -0.0, f32::MIN_POSITIVE, -f32::MIN_POSITIVE, f32::from_bits(1), f32::INFINITY, f32::NEG_INFINITY, f32::NAN, -f32::NAN, f32::MAX, f32::MIN];
    for k in -4096i32..=4096 {
        let t = k as f32 + 0.5;
        xs.extend([t, f32::from_bits(t.to_bits() - 1), f32::from_bits(t.to_bits() + 1), k as f32]);
    }
    for e in [2_147_483_648.0f32, -2_147_483_648.0, 2_147_483_520.0, -2_147_483_520.0, 8_388_608.0, 16_777_216.0] {
        for d in -4i32..=4 {
            xs.push(f32::from_bits((e.to_bits() as i32 + d) as u32));
        }
    }
    let mut g = Gen::new(0xF157);
    for _ in 0..1 << 20 {
        xs.push(f32::from_bits(g.u32()));
    }
    for x in xs {
        c.eq(i64::from(wwmath::float_to_long_fistp(x)), unsafe { cpp::ref_w3d_float_to_long(x) }, || format!("x = {x:e} ({:#010x})", x.to_bits()));
    }
    c.finish();
}

/// Section 13 in the given `Float_To_Long` branch; the C++ side must have
/// been set to the same one.
fn animation_evaluation(mode: FloatToLongMode) {
    eval_cases(mode, 0xE7A1, 6_000, "W3D animation evaluation (motchan, hrawanim, hcanim, hmorphanim, htree)", ev_script);
}

/// Section 13's driver: `cases` files from `seed`, each with a script from
/// `script_gen`, in the given `Float_To_Long` branch.
fn eval_cases(mode: FloatToLongMode, seed: u64, cases: usize, label: &str, script_gen: fn(&mut Gn, &EvalWorld) -> Vec<u8>) {
    let mut g = Gn { g: Gen::new(seed), clean: true };
    let mut tame_full = 0u64;
    let mut check = Check::new(&format!("{label}, {mode:?}"));
    let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
    let mut load_excluded: BTreeMap<String, u64> = BTreeMap::new();
    let (mut files, mut ops_run, mut ops_total) = (0u64, 0u64, 0u64);
    let mut per_op = [0u64; 64];
    for case in 0..cases {
        g.clean = case % 2 == 0;
        let data = ev_file(&mut g);
        let (mut log, mut w) = match rust_eval_load(&data, mode) {
            Ok(x) => x,
            Err(e) => {
                *load_excluded.entry(format!("{e:?}")).or_default() += 1;
                continue;
            }
        };
        let script = script_gen(&mut g, &w);
        let ops = script_ops(&script);
        ops_total += ops.len() as u64;
        let mut end = 0;
        let mut starts = Vec::new();
        for &(a, b) in &ops {
            let mut r = ScriptReader { s: &script, i: a };
            let mut l = Log::new();
            match rust_eval_op(&mut w, &mut r, &mut l) {
                Ok(()) => {
                    assert_eq!(r.i, b);
                    starts.push((log.len(), a));
                    log.extend(l);
                    per_op[script[a] as usize] += 1;
                    ops_run += 1;
                    end = b;
                }
                Err(W3dError::Undefined(u)) => {
                    *excluded.entry(format!("op {}: {u:?}", script[a])).or_default() += 1;
                    break;
                }
                Err(e) => {
                    *excluded.entry(format!("op {}: {e:?}", script[a])).or_default() += 1;
                    break;
                }
            }
        }
        let prefix = &script[..end];
        if g.clean && end == script.len() {
            tame_full += 1;
        }
        if std::env::var("W3D_CASE").ok().and_then(|v| v.parse::<usize>().ok()) == Some(case) {
            for (i, (a, k)) in w.anims.iter().enumerate() {
                eprintln!("anim {i} kind {k}: {a:#?}");
            }
            for &(a, b) in &ops {
                eprintln!("op {:?}", &script[a..b]);
            }
        }
        if std::env::var_os("W3D_TRACE").is_some() {
            eprintln!("eval case {case}: {data:02x?} {prefix:02x?}");
        }
        let cpp = cpp_eval(&data, prefix);
        check.eq(&log, &cpp, || {
            let d = log.iter().zip(cpp.iter()).position(|(a, b)| a != b).unwrap_or(log.len().min(cpp.len()));
            let op = starts.iter().rposition(|&(at, _)| at <= d);
            let (at, sa) = op.map(|k| starts[k]).unwrap_or((0, 0));
            let ctx = |v: &Log| v[at..v.len().min(at + 80)].to_vec();
            format!(
                "case {case}: first difference at log byte {d} (lengths {} / {}), in op #{op:?} (code {}, at script byte {sa}); \
                 rust {:02x?} c++ {:02x?}",
                log.len(),
                cpp.len(),
                op.map(|_| script[sa]).unwrap_or(0),
                ctx(&log),
                ctx(&cpp)
            )
        });
        files += 1;
    }
    println!(
        "{label} ({mode:?}): {cases} files, {files} run ({tame_full} tame scripts run to the end), {ops_run} of {ops_total} ops compared (per op 1..=63: {:?}); \
         files excluded (loading undefined): {load_excluded:?}; scripts stopped at (undefined upstream): {} {excluded:?}",
        &per_op[1..],
        excluded.values().sum::<u64>()
    );
    check.finish();
}

// ---------------------------------------------------------------------------
// 14. The render side (#10, step 4): ShaderClass::Apply, Get_SS_Category and
// Guess_Sort_Level (shader.cpp, verbatim) against formats::w3d::shader, and
// the skin deformation (meshgeometry.cpp get_deformed_vertices, vp.cpp
// Transform, verbatim, over htree.cpp's Base_Update) against
// formats::w3d::meshmdl (differential/wrap_w3d_shader.cpp,
// differential/wrap_w3d_deform.cpp).
// ---------------------------------------------------------------------------

use rts_lockstep_engine::formats::w3d::meshmdl::MeshModel;
use rts_lockstep_engine::formats::w3d::shader::{self as w3dshader, DeviceCaps, ShaderClass};

/// The Rust states in the wrapper's 52-word layout.
fn rust_shader_words(shader: ShaderClass, caps: &DeviceCaps) -> W3dResult<Vec<u32>> {
    let st = w3dshader::apply(shader, caps)?;
    let mut out = Vec::with_capacity(52);
    let mut put = |w: Option<u32>| {
        out.push(u32::from(w.is_some()));
        out.push(w.unwrap_or(0));
    };
    put(Some(u32::from(st.alpha_blend_enable)));
    put(st.src_blend);
    put(st.dst_blend);
    put(Some(u32::from(st.alpha_test_enable)));
    put(st.alpha_ref);
    put(st.alpha_func);
    put(Some(u32::from(st.fog_enable)));
    put(st.fog_color);
    put(Some(u32::from(st.specular_enable)));
    put(Some(st.zfunc));
    put(Some(u32::from(st.zwrite_enable)));
    put(Some(st.cull_mode));
    put(Some(st.patch_segments.to_bits()));
    for s in &st.stage {
        for v in [s.color_op, s.color_arg1, s.color_arg2, s.alpha_op, s.alpha_arg1, s.alpha_arg2] {
            put(Some(v));
        }
    }
    out.push(shader.get_ss_category() as u32);
    out.push(shader.guess_sort_level() as u32);
    Ok(out)
}

fn cpp_shader_words(shader: ShaderClass, caps: &DeviceCaps, fog_allowed: bool, fog_enable: bool, fog_color: u32) -> (Vec<u32>, i32) {
    let mut out = vec![0u32; 52];
    let calls = unsafe {
        cpp::ref_w3d_shader_apply(
            shader.0,
            caps.texture_op_caps,
            i32::from(caps.mod_alpha_add_clr),
            i32::from(fog_allowed),
            i32::from(fog_enable),
            fog_color,
            i32::from(caps.polygon_cull_mode == w3dshader::d3d::D3DCULL_CCW),
            caps.npatches_level as i32,
            out.as_mut_ptr(),
        )
    };
    (out, calls)
}

/// Predictions, written before the first run: every defined shader (a
/// destination blend of 0 to 5, or colour writes off) gives the same 52
/// words as upstream's `Apply` on the recording device, with no direct
/// device call; the destination blends 6 and 7 with colour writes on are
/// the only inputs left out (undefined upstream, `dstBlendLUT[6]`), about a
/// quarter of the colour-write-enabled random cases.
#[test]
fn shader_apply_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut check = Check::new("W3D ShaderClass::Apply, Get_SS_Category, Guess_Sort_Level (shader.cpp)");
    let mut g = Gen::new(0x5AD3);
    // (compared, excluded, direct device calls)
    let mut counts = (0u64, 0u64, 0i64);
    let run = |check: &mut Check, counts: &mut (u64, u64, i64), bits: u32, caps: DeviceCaps, fog_allowed: bool, fog_enable: bool, fog_color: u32| {
        let caps_rust = DeviceCaps { fog: (fog_allowed && fog_enable).then_some(fog_color), ..caps };
        let shader = ShaderClass(bits);
        match rust_shader_words(shader, &caps_rust) {
            Ok(r) => {
                let (c, calls) = cpp_shader_words(shader, &caps_rust, fog_allowed, fog_enable, fog_color);
                counts.2 += i64::from(calls);
                check.eq(&r, &c, || format!("bits {bits:#010x} caps {caps_rust:?} fog {fog_allowed}/{fog_enable}"));
                counts.0 += 1;
            }
            Err(_) => counts.1 += 1,
        }
    };
    // every combination of the fields Apply branches on (colour mask, both
    // blends, alpha test, texturing, primary gradient, post-detail colour and
    // alpha), with every op cap and with none
    let full = DeviceCaps::full();
    let none = DeviceCaps { texture_op_caps: 0, mod_alpha_add_clr: false, ..full };
    for caps in [full, none] {
        for cm in 0..2u32 {
            for dst in 0..8u32 {
                for src in 0..4u32 {
                    for at in 0..2u32 {
                        for tex in 0..2u32 {
                            for pri in 0..8u32 {
                                for pdc in 0..16u32 {
                                    for pda in 0..8u32 {
                                        let bits = 3 | 1 << 3 | cm << 4 | dst << 5 | pri << 10 | src << 14 | tex << 16 | at << 18 | 1 << 19 | pdc << 20 | pda << 24;
                                        run(&mut check, &mut counts, bits, caps, false, false, 0);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let exhaustive = counts.0 + counts.1;
    // random bits (the four unused top bits too), caps, fog, cull sense and
    // N-patch level
    for _ in 0..200_000 {
        let bits = g.u32();
        let caps = DeviceCaps {
            texture_op_caps: if g.below(3) == 0 { w3dshader::d3d::D3DTEXOPCAPS_ALL } else { g.u32() },
            mod_alpha_add_clr: g.below(2) == 0,
            fog: None,
            polygon_cull_mode: if g.below(2) == 0 { w3dshader::d3d::D3DCULL_CW } else { w3dshader::d3d::D3DCULL_CCW },
            npatches_level: 1 + g.below(4),
        };
        let (fa, fe) = (g.below(2) == 0, g.below(2) == 0);
        let fc = g.u32();
        run(&mut check, &mut counts, bits, caps, fa, fe, fc);
    }
    let (compared, excluded, direct_calls) = counts;
    println!(
        "shader apply: {exhaustive} exhaustive and 200000 random shaders, {compared} compared, {excluded} excluded (undefined: dstBlendLUT past its end), \
         {direct_calls} direct device calls"
    );
    assert_eq!(direct_calls, 0);
    check.finish();
}

/// Predictions, written before the first run: for every hierarchy the Rust
/// loads and poses (`Base_Update` defined) and bone links inside it, the
/// deformed vertices and normals are bit-identical to upstream's (both
/// overloads: the vertices-only one uses `Transform_Vector`, which has the
/// same operation order as `mulVector3Array`, so it equals the first);
/// links past the tree and undefined trees are left out.
#[test]
fn skin_deformation_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut check = Check::new("W3D skin deformation (meshgeometry.cpp get_deformed_vertices, vp.cpp Transform)");
    let mut g = Gn { g: Gen::new(0xDEF0), clean: true };
    let (mut compared, mut verts_compared) = (0u64, 0u64);
    let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
    const CASES: usize = 4_000;
    for case in 0..CASES {
        g.clean = case % 4 != 0;
        let npiv = 1 + g.below(12);
        let data = ev_tree(&mut g, &name16(b"SKEL"), npiv);
        let mut root = [0.0f32; 12];
        for v in root.iter_mut() {
            *v = if g.below(8) == 0 { g.f32() } else { g.g.uniform(-2.0, 2.0) };
        }
        let mut tree = {
            let mut c = ChunkLoad::new(GameFile::new(&data).unwrap());
            match c.open_chunk() {
                Ok(true) => {}
                _ => {
                    *excluded.entry("no chunk".into()).or_default() += 1;
                    continue;
                }
            }
            match htree::load_w3d(&mut c) {
                Ok(Some(t)) => t,
                Ok(None) => {
                    *excluded.entry("hierarchy load failed".into()).or_default() += 1;
                    continue;
                }
                Err(e) => {
                    *excluded.entry(format!("load {e:?}")).or_default() += 1;
                    continue;
                }
            }
        };
        let m = Matrix3D::new(
            root[0], root[1], root[2], root[3], root[4], root[5], root[6], root[7], root[8], root[9], root[10], root[11],
        );
        if let Err(e) = tree.base_update(&m) {
            *excluded.entry(format!("Base_Update {e:?}")).or_default() += 1;
            continue;
        }
        let n = 1 + g.below(48) as usize;
        let total = tree.num_pivots.max(1) as u32;
        let mut links = Vec::with_capacity(n);
        while links.len() < n {
            let bone = if g.below(40) == 0 { total + g.below(3) } else { g.below(total) };
            for _ in 0..1 + g.below(6) {
                links.push(bone as u16);
            }
        }
        links.truncate(n);
        let mut vtx = || [g.f32(), g.f32(), g.f32()];
        let vertices: Vec<[f32; 3]> = (0..n).map(|_| vtx()).collect();
        let normals: Vec<[f32; 3]> = (0..n).map(|_| vtx()).collect();
        let model = MeshModel {
            name: Vec::new(),
            flags: 0x0400,
            w3d_attributes: 0,
            sort_level: 0,
            vertices: vertices.clone(),
            normals: normals.clone(),
            polys: Vec::new(),
            bone_links: Some(links.clone()),
            pass_count: 0,
            passes: Vec::new(),
            uv: Vec::new(),
            color_array: [None, None],
            vertex_materials: Vec::new(),
            textures: Vec::new(),
            has_alternate: false,
        };
        let (rv, rn) = match model.get_deformed_vertices(&tree) {
            Ok(x) => x,
            Err(e) => {
                *excluded.entry(format!("deform {e:?}")).or_default() += 1;
                continue;
            }
        };
        let flat = |v: &[[f32; 3]]| v.iter().flatten().copied().collect::<Vec<f32>>();
        let (mut cv, mut cn, mut cv2) = (vec![0f32; 3 * n], vec![0f32; 3 * n], vec![0f32; 3 * n]);
        let r = unsafe {
            cpp::ref_w3d_deform(
                data.as_ptr() as *const _,
                data.len() as i32,
                root.as_ptr(),
                n as i32,
                flat(&vertices).as_ptr(),
                flat(&normals).as_ptr(),
                links.as_ptr(),
                cv.as_mut_ptr(),
                cn.as_mut_ptr(),
                cv2.as_mut_ptr(),
            )
        };
        assert_eq!(r, 0, "case {case}: the C++ did not load a hierarchy the Rust loaded");
        let bits = |v: &[f32]| v.iter().map(|f| f.to_bits()).collect::<Vec<u32>>();
        let rb = bits(&flat(&rv));
        check.eq(&rb, &bits(&cv), || format!("case {case}: vertices, links {links:?}"));
        check.eq(&bits(&flat(&rn)), &bits(&cn), || format!("case {case}: normals, links {links:?}"));
        check.eq(&rb, &bits(&cv2), || format!("case {case}: vertices-only overload, links {links:?}"));
        compared += 1;
        verts_compared += n as u64;
    }
    println!("skin deformation: {CASES} cases, {compared} compared ({verts_compared} vertices); excluded: {excluded:?}");
    check.finish();
}

// ---------------------------------------------------------------------------
// 15. Hierarchical models, distance LODs, particle emitters (hmdldef.cpp,
//     distlod.cpp, part_ldr.cpp; GitHub #17)
// ---------------------------------------------------------------------------

use rts_lockstep_engine::formats::w3d::part_ldr::{self, ParticleProperty, Vector3Randomizer};
use rts_lockstep_engine::formats::w3d::{distlod, hmdldef};

/// The largest keyframe count the comparison logs (the log holds every key).
const EMIT_MAX_KEYS: u32 = 1 << 16;

fn log_props<T: Copy + PartialEq + Default>(log: &mut Log, p: &ParticleProperty<T>, bytes: impl Fn(T) -> Vec<u8>) {
    log.extend(bytes(p.start));
    log.extend(bytes(p.rand));
    p32(log, p.num_key_frames);
    log.push(p.key_times.is_some() as u8);
    log.push(p.values.is_some() as u8);
    if p.key_times.is_some() && p.values.is_some() {
        for i in 0..p.num_key_frames {
            let (t, v) = p.key(i).expect("a key below the count");
            p32(log, t.to_bits());
            log.extend(bytes(v));
        }
    }
}

fn log_randomizer(log: &mut Log, r: &Option<Vector3Randomizer>) {
    match r {
        None => p32(log, 0xFFFF_FFFF),
        Some(r) => {
            p32(log, r.class_id());
            let v: Vec<f32> = match *r {
                Vector3Randomizer::SolidBox(e) => e.to_vec(),
                Vector3Randomizer::SolidSphere(x) | Vector3Randomizer::HollowSphere(x) => vec![x],
                Vector3Randomizer::SolidCylinder { extent, radius } => vec![extent, radius],
            };
            for x in v {
                p32(log, x.to_bits());
            }
        }
    }
}

/// `ref_w3d_load_emit` of wrap_w3d_emit.cpp, in Rust: kind 0 hmodel, 1
/// distance LOD, 2 particle emitter (`alpha`: the texture lookup's answer).
/// `Ok(None)` when an emitter keeps more than [`EMIT_MAX_KEYS`] keys (kept
/// out of the comparison).
fn rust_load_emit(data: &[u8], kind: i32, alpha: bool) -> W3dResult<Option<Log>> {
    let mut log = Log::new();
    let mut asked: Vec<Vec<u8>> = Vec::new();
    let mut c = ChunkLoad::new(GameFile::new(data).unwrap());
    log.push(b'O');
    if c.open_chunk()? {
        log.push(1);
        match kind {
            0 => match hmdldef::load_w3d(&mut c)? {
                Some(d) => {
                    log.push(b'H');
                    pstr(&mut log, Some(&d.name));
                    log.extend_from_slice(&d.model_name);
                    log.extend_from_slice(&d.base_pose_name);
                    p32(&mut log, d.sub_object_count as u32);
                    for i in 0..d.sub_object_count.max(0) as usize {
                        let n = d.sub_object(i).unwrap();
                        pstr(&mut log, Some(&n.render_obj_name));
                        p32(&mut log, n.pivot_id as u32);
                    }
                    match &d.snap_points {
                        None => log.push(0),
                        Some(pts) => {
                            log.push(1);
                            p32(&mut log, pts.len() as u32);
                            for v in pts.iter().flatten() {
                                p32(&mut log, v.to_bits());
                            }
                        }
                    }
                }
                None => log.extend_from_slice(&[b'e', 1]),
            },
            1 => match distlod::load_w3d(&mut c)? {
                Some(d) => {
                    log.push(b'D');
                    pstr(&mut log, Some(&d.name));
                    p32(&mut log, d.lod_count as u32);
                    for l in &d.lods {
                        pstr(&mut log, Some(&l.name));
                        p32(&mut log, l.res_down_dist.to_bits());
                        p32(&mut log, l.res_up_dist.to_bits());
                    }
                }
                None => log.extend_from_slice(&[b'e', 2]),
            },
            _ => {
                let r = part_ldr::load_w3d(&mut c, |n: &[u8]| {
                    asked.push(n.to_vec());
                    alpha
                })?;
                match r {
                    Some(d) => {
                        let props = [&d.opacity_keyframes, &d.size_keyframes, &d.rotation_keyframes, &d.frame_keyframes, &d.blur_time_keyframes];
                        if d.color_keyframes.num_key_frames > EMIT_MAX_KEYS || props.iter().any(|p| p.num_key_frames > EMIT_MAX_KEYS) {
                            return Ok(None);
                        }
                        log.push(b'P');
                        pstr(&mut log, d.name.as_deref());
                        match &d.user_string {
                            None => p32(&mut log, 0xFFFF_FFFF),
                            Some(s) => {
                                p32(&mut log, s.len() as u32);
                                log.extend_from_slice(s);
                            }
                        }
                        p32(&mut log, d.user_type as u32);
                        p32(&mut log, d.version);
                        log.extend(d.info.to_bytes());
                        log.extend(d.info_v2.to_bytes());
                        log.extend(d.extra_info.to_bytes());
                        log.extend(d.line_properties.to_bytes());
                        let f = |x: f32| x.to_le_bytes().to_vec();
                        let v3 = |x: [f32; 3]| x.iter().flat_map(|c| c.to_le_bytes()).collect::<Vec<u8>>();
                        log_props(&mut log, &d.color_keyframes, v3);
                        log_props(&mut log, &d.opacity_keyframes, f);
                        log_props(&mut log, &d.size_keyframes, f);
                        log_props(&mut log, &d.rotation_keyframes, f);
                        p32(&mut log, d.initial_orientation_random.to_bits());
                        log_props(&mut log, &d.frame_keyframes, f);
                        log_props(&mut log, &d.blur_time_keyframes, f);
                        log_randomizer(&mut log, &d.creation_volume);
                        log_randomizer(&mut log, &d.velocity_randomizer);
                    }
                    None => log.extend_from_slice(&[b'e', 2]),
                }
            }
        }
    } else {
        log.push(0);
    }
    log_state(&mut log, &mut c);
    if kind == 2 {
        log.push(b't');
        p32(&mut log, asked.len() as u32);
        for n in &asked {
            pstr(&mut log, Some(n));
        }
    }
    Ok(Some(log))
}

fn cpp_load_emit(data: &[u8], kind: i32, alpha: bool) -> Log {
    cpp_log(|log, cap, n| unsafe { cpp::ref_w3d_load_emit(data.as_ptr() as *const _, data.len() as i32, kind, alpha as i32, log, cap, n) })
}

fn emit_hmodel_header(version: u32, name: &[u8], hier: &[u8], n: u16) -> Vec<u8> {
    let mut h = W3dHModelHeaderStruct::from_bytes(&[0; 40]);
    h.version = version;
    h.name[..name.len()].copy_from_slice(name);
    h.hierarchy_name[..hier.len()].copy_from_slice(hier);
    h.num_connections = n;
    hchunk(W3D_CHUNK_HMODEL_HEADER, &h.to_bytes())
}

fn emit_hmodel_node(id: u32, name: &[u8], pivot: u16) -> Vec<u8> {
    let mut n = W3dHModelNodeStruct::from_bytes(&[0; 18]);
    n.render_obj_name[..name.len()].copy_from_slice(name);
    n.pivot_idx = pivot;
    hchunk(id, &n.to_bytes())
}

fn emit_lod_header(name: &[u8], n: u16) -> Vec<u8> {
    let mut h = W3dLODModelHeaderStruct::from_bytes(&[0; 24]);
    h.name[..name.len()].copy_from_slice(name);
    h.num_lods = n;
    hchunk(W3D_CHUNK_LODMODEL_HEADER, &h.to_bytes())
}

fn emit_lod(name: &[u8], min: f32, max: f32) -> Vec<u8> {
    let mut l = W3dLODStruct::from_bytes(&[0; 40]);
    l.render_obj_name[..name.len()].copy_from_slice(name);
    l.lod_min = min;
    l.lod_max = max;
    hchunk(W3D_CHUNK_LOD, &l.to_bytes())
}

fn emit_header(version: u32, name: &[u8]) -> Vec<u8> {
    let mut h = W3dEmitterHeaderStruct::from_bytes(&[0; 20]);
    h.version = version;
    h.name[..name.len()].copy_from_slice(name);
    hchunk(W3D_CHUNK_EMITTER_HEADER, &h.to_bytes())
}

fn emit_user(type_: u32, n: u32, s: &[u8]) -> Vec<u8> {
    let mut b = type_.to_le_bytes().to_vec();
    b.extend(n.to_le_bytes());
    b.extend([0u8; 4]);
    b.extend_from_slice(s);
    hchunk(W3D_CHUNK_EMITTER_USER_DATA, &b)
}

fn emit_info(texture: &[u8], f: impl FnOnce(&mut W3dEmitterInfoStruct)) -> Vec<u8> {
    let mut i = W3dEmitterInfoStruct::from_bytes(&[0; 332]);
    i.texture_filename[..texture.len()].copy_from_slice(texture);
    f(&mut i);
    hchunk(W3D_CHUNK_EMITTER_INFO, &i.to_bytes())
}

fn emit_info_v2(f: impl FnOnce(&mut W3dEmitterInfoStructV2)) -> Vec<u8> {
    let mut i = W3dEmitterInfoStructV2::from_bytes(&[0; 124]);
    f(&mut i);
    hchunk(W3D_CHUNK_EMITTER_INFOV2, &i.to_bytes())
}

fn emit_key(t: f32, v: [u8; 4]) -> Vec<u8> {
    let mut b = t.to_le_bytes().to_vec();
    b.extend(v);
    b
}

fn emit_fkey(t: f32, v: f32) -> Vec<u8> {
    emit_key(t, v.to_le_bytes())
}

fn emit_props(counts: [u32; 3], rand: [u8; 4], keys: &[Vec<u8>]) -> Vec<u8> {
    let mut p = W3dEmitterPropertyStruct::from_bytes(&[0; 40]);
    (p.color_keyframes, p.opacity_keyframes, p.size_keyframes) = (counts[0], counts[1], counts[2]);
    p.color_random = W3dRGBAStruct { r: rand[0], g: rand[1], b: rand[2], a: rand[3] };
    p.opacity_random = 0.5;
    p.size_random = 0.25;
    let mut b = p.to_bytes();
    for k in keys {
        b.extend(k);
    }
    hchunk(W3D_CHUNK_EMITTER_PROPS, &b)
}

/// A rotation, frame or blur-time chunk: the count, the random range, the
/// orientation random (rotation only), padding to the header's size, then
/// the keys.
fn emit_keyed(id: u32, count: u32, keys: &[Vec<u8>]) -> Vec<u8> {
    let mut b = count.to_le_bytes().to_vec();
    b.extend(0.1f32.to_le_bytes());
    let size = match id {
        W3D_CHUNK_EMITTER_ROTATION_KEYFRAMES => {
            b.extend(0.3f32.to_le_bytes());
            16
        }
        W3D_CHUNK_EMITTER_FRAME_KEYFRAMES => 16,
        _ => 12,
    };
    b.resize(size, 0);
    for k in keys {
        b.extend(k);
    }
    hchunk(id, &b)
}

fn emit_v2_base(name: &[u8]) -> Vec<u8> {
    let mut b = emit_header(0x0002_0000, name);
    b.extend(emit_user(0, 0, b""));
    b.extend(emit_info(b"a.tga", |_| {}));
    b.extend(emit_info_v2(|_| {}));
    let k = [emit_key(0.0, [0; 4]), emit_key(1.0, [0; 4])];
    b.extend(emit_props([2, 2, 2], [0; 4], &[k[0].clone(), k[1].clone(), k[0].clone(), k[1].clone(), k[0].clone(), k[1].clone()]));
    b
}

/// (kind, alpha, name, file).
fn emit_named_cases() -> Vec<(i32, bool, &'static str, Vec<u8>)> {
    let hm = |b: Vec<u8>| hchunk(W3D_CHUNK_HMODEL, &b);
    let dl = |b: Vec<u8>| hchunk(W3D_CHUNK_LODMODEL, &b);
    let em = |b: Vec<u8>| hchunk(W3D_CHUNK_EMITTER, &b);
    let mut v = Vec::new();

    let mut b = emit_hmodel_header(W3D_CURRENT_HMODEL_VERSION, b"TANK", b"TANKSKL", 3);
    b.extend(emit_hmodel_node(W3D_CHUNK_NODE, b"BODY", 1));
    b.extend(hchunk(0x1234, &[1, 2, 3]));
    b.extend(emit_hmodel_node(W3D_CHUNK_SKIN_NODE, b"TURRET", 65535));
    b.extend(emit_hmodel_node(W3D_CHUNK_COLLISION_NODE, b"BOX", 3));
    b.extend(hchunk(W3D_CHUNK_POINTS, &[0, 0, 128, 63, 0, 0, 0, 64, 0, 0, 64, 64]));
    v.push((0, false, "hmodel: current, three node kinds, points", hm(b)));
    let mut b = emit_hmodel_header(w3d_make_version(2, 0), b"ABCDEFGHIJKLMNOP", b"S", 3);
    b.extend(emit_hmodel_node(W3D_CHUNK_NODE, b"X", 65535));
    b.extend(emit_hmodel_node(W3D_CHUNK_NODE, b"Y", 4));
    v.push((0, false, "hmodel: pre-3.0, 16-character name, fewer nodes", hm(b)));
    v.push((0, false, "hmodel: empty chunk (false is OK)", hm(Vec::new())));
    v.push((0, false, "hmodel: first chunk not the header", hm(emit_hmodel_node(W3D_CHUNK_NODE, b"A", 0))));
    v.push((0, false, "hmodel: short header", hm(hchunk(W3D_CHUNK_HMODEL_HEADER, &[0; 39]))));
    let mut b = emit_hmodel_header(W3D_CURRENT_HMODEL_VERSION, b"M", b"S", 1);
    b.extend(emit_hmodel_node(W3D_CHUNK_NODE, b"ABCDEFGHIJKLMNOP", 0x0041));
    v.push((0, false, "hmodel: node name running into PivotIdx", hm(b)));

    let mut b = emit_lod_header(b"TREE", 2);
    b.extend(emit_lod(b"TREE_HI", 0.0, 50.0));
    b.extend(emit_lod(b"TREE_LO", 50.0, 1000.0));
    b.extend(hchunk(0x99, &[1, 2]));
    v.push((1, false, "distlod: two LODs", dl(b)));
    v.push((1, false, "distlod: 16-character name", dl(emit_lod_header(b"ABCDEFGHIJKLMNOP", 0))));
    v.push((1, false, "distlod: name into NumLODs, missing LODs", dl(emit_lod_header(b"ABCDEFGHIJKLMNOP", 0x4141))));
    let mut b = emit_lod_header(b"M", 1);
    b.extend(emit_lod(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ012345", 0.0, 1.0));
    v.push((1, false, "distlod: 32-character LOD name", dl(b)));
    let mut b = emit_lod_header(b"M", 1);
    b.extend(hchunk(W3D_CHUNK_LOD, &[0; 39]));
    v.push((1, false, "distlod: short LOD", dl(b)));

    let mut b = emit_header(0x0002_0000, b"SMOKE");
    b.extend(emit_user(7, 6, b"hello\0"));
    b.extend(emit_info(b"smoke.tga", |i| i.lifetime = 2.0));
    b.extend(emit_info_v2(|i| {
        i.burst_size = 3;
        i.creation_volume.class_id = 1;
        i.creation_volume.value1 = 2.5;
        i.vel_random.class_id = 3;
        i.vel_random.value1 = 1.0;
        i.vel_random.value2 = -2.0;
    }));
    b.extend(emit_props(
        [3, 2, 1],
        [51, 0, 0, 0],
        &[
            emit_key(0.0, [255, 0, 0, 0]),
            emit_key(0.5, [0, 255, 0, 0]),
            emit_key(1.0, [0, 0, 0, 0]),
            emit_fkey(0.0, 1.0),
            emit_fkey(1.0, 0.0),
            emit_fkey(0.0, 4.0),
            emit_fkey(9.0, 9.0),
        ],
    ));
    let mut lp = W3dEmitterLinePropertiesStruct::from_bytes(&[0; 64]);
    lp.flags = 5;
    b.extend(hchunk(W3D_CHUNK_EMITTER_LINE_PROPERTIES, &lp.to_bytes()));
    b.extend(emit_keyed(W3D_CHUNK_EMITTER_ROTATION_KEYFRAMES, 2, &[emit_fkey(0.0, 0.5), emit_fkey(0.2, 1.0), emit_fkey(0.8, 2.0)]));
    b.extend(emit_keyed(W3D_CHUNK_EMITTER_FRAME_KEYFRAMES, 1, &[emit_fkey(0.0, 3.0), emit_fkey(0.5, 7.0)]));
    b.extend(emit_keyed(W3D_CHUNK_EMITTER_BLUR_TIME_KEYFRAMES, 0, &[emit_fkey(0.0, 0.1)]));
    b.extend(hchunk(OBSOLETE_W3D_CHUNK_EMITTER_COLOR_KEYFRAME, &[1, 2, 3]));
    let mut eb = 1.5f32.to_le_bytes().to_vec();
    eb.extend([0u8; 36]);
    b.extend(hchunk(W3D_CHUNK_EMITTER_EXTRA_INFO, &eb));
    v.push((2, false, "emitter: version 2, every chunk", em(b)));

    let v1 = |texture: &[u8]| {
        let mut b = emit_header(0x0001_0000, b"SPARK");
        b.extend(emit_user(0, 0, b""));
        b.extend(emit_info(texture, |i| {
            i.position_random = 500.0;
            i.velocity_random = 2.0;
            i.fade_time = 0.75;
            i.start_color = W3dRGBAStruct { r: 255, g: 128, b: 0, a: 255 };
            i.end_color = W3dRGBAStruct { r: 0, g: 0, b: 0, a: 51 };
            i.start_size = 1.5;
        }));
        b.extend(emit_info_v2(|i| i.burst_size = 9));
        b
    };
    v.push((2, true, "emitter: version 1, alpha texture", em(v1(b"fx\\spark.tga"))));
    v.push((2, false, "emitter: version 1, texture without alpha", em(v1(b"fx\\spark.tga"))));
    v.push((2, true, "emitter: version 1, no texture", em(v1(b""))));

    let mut b = emit_header(0x0002_0000, b"Q");
    b.extend(emit_user(0, 0, b""));
    b.extend(emit_info(b"", |_| {}));
    b.extend(emit_info_v2(|_| {}));
    b.extend(emit_props([2, 1, 1], [9, 0, 0, 0], &[emit_key(0.0, [1, 1, 1, 0]), emit_key(1.0, [0; 4])]));
    v.push((2, false, "emitter: one black keyframe not tweaked", em(b)));
    let mut b = emit_header(0x0002_0000, b"Q");
    b.extend(emit_user(0, 3, b"abc"));
    b.extend(emit_info(b"", |_| {}));
    b.extend(emit_info_v2(|_| {}));
    b.extend(emit_props([4, 2, 2], [0; 4], &[]));
    b.extend(emit_keyed(W3D_CHUNK_EMITTER_ROTATION_KEYFRAMES, 1, &[emit_fkey(0.0, 0.5), emit_fkey(0.25, 1.0)]));
    b.extend(emit_keyed(W3D_CHUNK_EMITTER_ROTATION_KEYFRAMES, 0, &[]));
    v.push((2, false, "emitter: keyless props, unterminated user string, second rotation", em(b)));
    let mut b = emit_v2_base(b"Q");
    b.extend(emit_keyed(W3D_CHUNK_EMITTER_ROTATION_KEYFRAMES, 3, &[emit_fkey(0.0, 0.5), emit_fkey(0.25, 1.0)]));
    b.extend(hchunk(W3D_CHUNK_EMITTER_EXTRA_INFO, &[0; 40]));
    v.push((2, false, "emitter: short rotation keys, one more chunk opened", em(b)));
    v.push((2, false, "emitter: user data where the header should be", em(emit_user(0, 0, b""))));
    v.push((2, false, "emitter: empty", em(Vec::new())));
    let mut b = hchunk(W3D_CHUNK_EMITTER_HEADER, &[0; 19]);
    b.extend(emit_user(0, 0, b""));
    v.push((2, false, "emitter: short header", em(b)));
    v
}

/// The named cases: the files of the unit tests of `formats::w3d::hmdldef`
/// (a current model with the three node kinds, a skipped chunk and snap
/// points; a pre-3.0 model with a 16-character name and fewer nodes than
/// `NumConnections`; an empty hmodel chunk, which loads; the two header
/// failures; a node name running into `PivotIdx`), of `distlod` (two LODs
/// and a trailing chunk; 16-character names stopping at and running into
/// `NumLODs`; a 32-character LOD name; a short LOD) and of `part_ldr` (a
/// version-2 emitter with every chunk kind; a version-1 emitter converted
/// with and without an alpha texture, and with none; a single black
/// keyframe; a keyless props chunk, an unterminated user string and a
/// second rotation chunk of count 0; short rotation keys; the three load
/// errors before the loop). Their undefined cases are not given to the C++.
///
/// Compared byte for byte: the load result; for an hmodel `Name`, the two
/// 16-byte names, `SubObjectCount`, every node's name and pivot, the snap
/// points; for a distance LOD the name, count, each LOD's name and two
/// distances; for an emitter the name, the user string's bytes (from its
/// `malloc` size), type and version, the bytes of `m_Info`, `m_InfoV2`,
/// `m_ExtraInfo` and `m_LineProperties`, the six keyframe sets (start,
/// random range, count, which arrays exist, every key), the orientation
/// random and the two randomizers; the chunk depth and file position the
/// loader leaves; and the texture names `Convert_To_Ver2` asked for.
///
/// Predicted before the run: all ~~23~~ **21** (corrected after the run:
/// a miscount of the list) agree, in particular the black
/// keyframe at minus the random range (`-0.2, -0, -0`), the empty hmodel
/// that loads with an empty name, the extra chunk the emitter's loop opens
/// after a failure, the shaders of the version-1 conversion (`SrcBlend` 2
/// and `DestBlend` 5 with alpha, 1 and 1 without) and the backslash kept in
/// the texture name asked for.
///
/// Result (2026-11-06): 21 compared, 0 mismatches.
#[test]
fn emit_named_cases_match_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut check = Check::new("HModelDefClass / DistLODDefClass / ParticleEmitterDefClass::Load_W3D, named cases");
    let cases = emit_named_cases();
    for (kind, alpha, name, data) in &cases {
        let rust = rust_load_emit(data, *kind, *alpha).unwrap_or_else(|e| panic!("{name}: Rust undefined: {e:?}")).expect("few keys");
        let c = cpp_load_emit(data, *kind, *alpha);
        check.eq(&rust, &c, || format!("{name}: {:02x?}", data));
    }
    println!("hmodel/distlod/emitter named cases: {} compared", cases.len());
    check.finish();
}

fn gen_hmodel(g: &mut Gn) -> Vec<u8> {
    let mut body = Vec::new();
    if g.below(12) != 0 {
        let mut h = W3dHModelHeaderStruct::from_bytes(&[0; 40]);
        h.version = match g.below(4) {
            0 => w3d_make_version(2, 0),
            1 => w3d_make_version(3, 0),
            2 => g.g.u32(),
            _ => W3D_CURRENT_HMODEL_VERSION,
        };
        let n = gen_text(g, 16);
        h.name[..n.len()].copy_from_slice(&n);
        let n = gen_text(g, 16);
        h.hierarchy_name[..n.len()].copy_from_slice(&n);
        h.num_connections = match g.below(16) {
            0 => g.g.u32() as u16,
            1 => 0,
            _ => g.below(6) as u16,
        };
        let mut hb = h.to_bytes();
        if g.chance(10) {
            hb.truncate(g.below(40) as usize);
        }
        let id = if g.chance(12) { W3D_CHUNK_HMODEL } else { W3D_CHUNK_HMODEL_HEADER };
        body.extend(chunk(g, id, false, &hb));
    }
    for _ in 0..g.below(7) {
        match g.below(6) {
            0..=2 => {
                let id = [W3D_CHUNK_NODE, W3D_CHUNK_COLLISION_NODE, W3D_CHUNK_SKIN_NODE][g.below(3) as usize];
                let mut n = W3dHModelNodeStruct::from_bytes(&[0; 18]);
                let t = gen_text(g, 16);
                n.render_obj_name[..t.len()].copy_from_slice(&t);
                n.pivot_idx = match g.below(5) {
                    0 => 65535,
                    1 => g.g.u32() as u16,
                    _ => g.below(10) as u16,
                };
                let mut b = n.to_bytes();
                if g.chance(10) {
                    b.truncate(g.below(18) as usize);
                }
                body.extend(chunk(g, id, false, &b));
            }
            3 => {
                let mut b = Vec::new();
                for _ in 0..g.below(4) * 3 {
                    b.extend(g.f32().to_le_bytes());
                }
                b.extend((0..g.below(3)).map(|_| g.g.u32() as u8));
                body.extend(chunk(g, W3D_CHUNK_POINTS, false, &b));
            }
            _ => {
                let junk: Vec<u8> = (0..g.below(20)).map(|_| g.g.u32() as u8).collect();
                let id = if g.coin() { OBSOLETE_W3D_CHUNK_HMODEL_AUX_DATA } else { g.g.u32() };
                body.extend(chunk(g, id, false, &junk));
            }
        }
    }
    chunk(g, W3D_CHUNK_HMODEL, true, &body)
}

fn gen_distlod(g: &mut Gn) -> Vec<u8> {
    let mut h = W3dLODModelHeaderStruct::from_bytes(&[0; 24]);
    h.version = g.g.u32();
    let n = gen_text(g, 16);
    h.name[..n.len()].copy_from_slice(&n);
    let count = match g.below(14) {
        0 => g.g.u32() as u16,
        1 => 0,
        _ => g.below(5) as u16,
    };
    h.num_lods = count;
    let mut hb = h.to_bytes();
    if g.chance(10) {
        hb.truncate(g.below(24) as usize);
    }
    let id = if g.chance(12) { W3D_CHUNK_LOD } else { W3D_CHUNK_LODMODEL_HEADER };
    let mut body = chunk(g, id, false, &hb);
    let lods = if g.coin() { u32::from(count) } else { g.below(6) };
    for _ in 0..lods.min(8) {
        let mut l = W3dLODStruct::from_bytes(&[0; 40]);
        let t = gen_text(g, 32);
        l.render_obj_name[..t.len()].copy_from_slice(&t);
        l.lod_min = g.f32();
        l.lod_max = g.f32();
        let mut b = l.to_bytes();
        if g.chance(10) {
            b.truncate(g.below(40) as usize);
        }
        if g.chance(8) {
            b.extend((0..g.below(8)).map(|_| g.g.u32() as u8));
        }
        let id = if g.chance(12) { g.g.u32() } else { W3D_CHUNK_LOD };
        body.extend(chunk(g, id, false, &b));
    }
    chunk(g, W3D_CHUNK_LODMODEL, true, &body)
}

fn gen_emit_keys(g: &mut Gn, n: u32, color: bool) -> Vec<Vec<u8>> {
    (0..n)
        .map(|_| {
            let t = g.f32();
            if color && g.coin() {
                let c = if g.below(4) == 0 { [0, 0, 0, g.g.u32() as u8] } else { (g.g.u32()).to_le_bytes() };
                emit_key(t, c)
            } else {
                let v = g.f32();
                emit_fkey(t, v)
            }
        })
        .collect()
}

fn gen_emitter(g: &mut Gn) -> Vec<u8> {
    let version = match g.below(8) {
        0 | 1 => 0x0001_0000,
        2 => g.below(0x0003_0000),
        3 => g.g.u32(),
        _ => 0x0002_0000,
    };
    let mut parts: Vec<Vec<u8>> = Vec::new();
    // header
    let mut h = W3dEmitterHeaderStruct::from_bytes(&[0; 20]);
    h.version = version;
    let n = gen_text(g, 16);
    h.name[..n.len()].copy_from_slice(&n);
    let mut hb = h.to_bytes();
    if g.chance(12) {
        hb.truncate(g.below(20) as usize);
    }
    parts.push(chunk(g, W3D_CHUNK_EMITTER_HEADER, false, &hb));
    // user data
    let s = gen_text(g, 12);
    let mut sb = s.clone();
    if g.coin() {
        sb.push(0);
    }
    let n = match g.below(16) {
        0 => 0xFFFF_FFFF,
        1 => 0x7FFF_FFFF,
        2 => sb.len() as u32 + 1 + g.below(4),
        3..=6 => 0,
        _ => sb.len() as u32,
    };
    let mut ub = g.g.u32().to_le_bytes().to_vec();
    ub.extend(n.to_le_bytes());
    ub.extend((0..4).map(|_| g.g.u32() as u8));
    if g.chance(12) {
        ub.truncate(g.below(12) as usize);
    }
    ub.extend(sb);
    parts.push(chunk(g, W3D_CHUNK_EMITTER_USER_DATA, false, &ub));
    // info
    let mut i = W3dEmitterInfoStruct::from_bytes(&[0; 332]);
    let tex: Vec<u8> = match g.below(8) {
        0 => Vec::new(),
        1 => vec![b'x'; 260],
        2 => {
            let mut t = b"art\\fx\\".to_vec();
            t.extend(gen_text(g, 12));
            t.extend(b".tga");
            t
        }
        _ => {
            let mut t = gen_text(g, 12);
            t.extend(b".tga");
            t
        }
    };
    i.texture_filename[..tex.len()].copy_from_slice(&tex);
    for f in [
        &mut i.start_size,
        &mut i.end_size,
        &mut i.lifetime,
        &mut i.emission_rate,
        &mut i.max_emissions,
        &mut i.velocity_random,
        &mut i.position_random,
        &mut i.fade_time,
        &mut i.gravity,
        &mut i.elasticity,
    ] {
        *f = g.f32();
    }
    for v in [&mut i.velocity, &mut i.acceleration] {
        (v.x, v.y, v.z) = (g.f32(), g.f32(), g.f32());
    }
    let c = g.g.u32().to_le_bytes();
    i.start_color = W3dRGBAStruct { r: c[0], g: c[1], b: c[2], a: c[3] };
    let c = g.g.u32().to_le_bytes();
    i.end_color = W3dRGBAStruct { r: c[0], g: c[1], b: c[2], a: c[3] };
    let mut ib = i.to_bytes();
    if g.chance(12) {
        ib.truncate(g.below(332) as usize);
    }
    parts.push(chunk(g, W3D_CHUNK_EMITTER_INFO, false, &ib));
    // info v2
    if version > 0x0001_0000 || g.below(4) == 0 {
        let mut v = W3dEmitterInfoStructV2::from_bytes(&[0; 124]);
        v.burst_size = if g.coin() { g.below(5) } else { g.g.u32() };
        for r in [&mut v.creation_volume, &mut v.vel_random] {
            r.class_id = if g.below(6) == 0 { g.g.u32() } else { g.below(5) };
            (r.value1, r.value2, r.value3) = (g.f32(), g.f32(), g.f32());
        }
        v.outward_vel = g.f32();
        v.vel_inherit = g.f32();
        let s = g.g.u64().to_le_bytes();
        let mut sh = [0u8; 16];
        sh[..8].copy_from_slice(&s);
        v.shader = W3dShaderStruct::from_bytes(&sh);
        v.render_mode = g.below(4);
        v.frame_mode = g.below(4);
        let mut vb = v.to_bytes();
        if g.chance(12) {
            vb.truncate(g.below(124) as usize);
        }
        parts.push(chunk(g, W3D_CHUNK_EMITTER_INFOV2, false, &vb));
        // props
        let counts = [0, 1, 2].map(|_| match g.below(20) {
            0 => 0,
            1 => g.g.u32(),
            _ => 1 + g.below(5),
        });
        let mut p = W3dEmitterPropertyStruct::from_bytes(&[0; 40]);
        (p.color_keyframes, p.opacity_keyframes, p.size_keyframes) = (counts[0], counts[1], counts[2]);
        let c = if g.coin() { [0u8; 4] } else { g.g.u32().to_le_bytes() };
        p.color_random = W3dRGBAStruct { r: c[0], g: c[1], b: c[2], a: c[3] };
        p.opacity_random = g.f32();
        p.size_random = g.f32();
        let mut pb = p.to_bytes();
        if g.chance(12) {
            pb.truncate(g.below(40) as usize);
        }
        let keys = counts.iter().map(|&k| k.min(6)).sum::<u32>();
        let keys = if g.below(4) == 0 { g.below(keys + 1) } else { keys };
        for k in gen_emit_keys(g, keys, true) {
            pb.extend(k);
        }
        parts.push(chunk(g, W3D_CHUNK_EMITTER_PROPS, false, &pb));
    }
    if g.chance(10) && !parts.is_empty() {
        let a = g.below(parts.len() as u32) as usize;
        let b = g.below(parts.len() as u32) as usize;
        parts.swap(a, b);
    }
    if g.chance(12) && !parts.is_empty() {
        let a = g.below(parts.len() as u32) as usize;
        parts.remove(a);
    }
    // later chunks
    for _ in 0..g.below(6) {
        let p = match g.below(8) {
            0 => {
                let mut b: Vec<u8> = (0..64).map(|_| g.g.u32() as u8).collect();
                if g.chance(8) {
                    b.truncate(g.below(64) as usize);
                }
                chunk(g, W3D_CHUNK_EMITTER_LINE_PROPERTIES, false, &b)
            }
            1..=3 => {
                let id = [W3D_CHUNK_EMITTER_ROTATION_KEYFRAMES, W3D_CHUNK_EMITTER_FRAME_KEYFRAMES, W3D_CHUNK_EMITTER_BLUR_TIME_KEYFRAMES]
                    [g.below(3) as usize];
                let count = match g.below(16) {
                    0 => g.g.u32(),
                    1 => 0x2000_0000,
                    _ => g.below(5),
                };
                let mut b = count.to_le_bytes().to_vec();
                let size = if id == W3D_CHUNK_EMITTER_BLUR_TIME_KEYFRAMES { 12 } else { 16 };
                while b.len() < size {
                    b.extend(g.f32().to_le_bytes());
                }
                if g.chance(10) {
                    b.truncate(g.below(size as u32) as usize);
                }
                let keys = if g.below(4) == 0 { g.below(count.min(5) + 2) } else { count.min(5) + 1 };
                for k in gen_emit_keys(g, keys, false) {
                    b.extend(k);
                }
                if g.chance(10) {
                    b.extend((0..g.below(8)).map(|_| g.g.u32() as u8));
                }
                chunk(g, id, false, &b)
            }
            4 => {
                let mut b: Vec<u8> = g.f32().to_le_bytes().to_vec();
                b.extend((0..36).map(|_| g.g.u32() as u8));
                if g.chance(8) {
                    b.truncate(g.below(40) as usize);
                }
                chunk(g, W3D_CHUNK_EMITTER_EXTRA_INFO, false, &b)
            }
            5 => {
                let junk: Vec<u8> = (0..g.below(24)).map(|_| g.g.u32() as u8).collect();
                let id = OBSOLETE_W3D_CHUNK_EMITTER_COLOR_KEYFRAME + g.below(3);
                chunk(g, id, false, &junk)
            }
            _ => {
                let junk: Vec<u8> = (0..g.below(20)).map(|_| g.g.u32() as u8).collect();
                let id = if g.coin() { W3D_CHUNK_EMITTER_HEADER + g.below(14) } else { g.g.u32() };
                chunk(g, id, false, &junk)
            }
        };
        parts.push(p);
    }
    chunk(g, W3D_CHUNK_EMITTER, true, &parts.concat())
}

/// The random comparison of section 15 for one kind.
fn run_emit_cases(name: &str, kind: i32, seed: u64, cases: usize, gen: impl Fn(&mut Gn) -> Vec<u8>) {
    let mut g = Gn { g: Gen::new(seed), clean: false };
    let mut check = Check::new(name);
    let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
    let (mut ok, mut failed, mut large, mut compared) = (0u64, 0u64, 0u64, 0u64);
    for case in 0..cases {
        g.clean = case % 2 == 0;
        let data = gen(&mut g);
        let data = mutate(&mut g, data);
        let alpha = g.coin();
        let rust = match rust_load_emit(&data, kind, alpha) {
            Ok(Some(l)) => l,
            Ok(None) => {
                large += 1;
                continue;
            }
            Err(W3dError::Undefined(u)) => {
                *excluded.entry(format!("{u:?}")).or_default() += 1;
                continue;
            }
            Err(e) => {
                *excluded.entry(format!("{e:?}")).or_default() += 1;
                continue;
            }
        };
        match rust.get(2) {
            Some(b'H' | b'D' | b'P') => ok += 1,
            Some(b'e') => failed += 1,
            _ => {}
        }
        if std::env::var_os("W3D_TRACE").is_some() {
            eprintln!("{name} case {case}: {:02x?}", data);
        }
        let c = cpp_load_emit(&data, kind, alpha);
        check.eq(&rust, &c, || format!("case {case}: {:02x?}", data));
        compared += 1;
    }
    println!(
        "{name}: {cases} inputs, {compared} compared, {ok} loaded, {failed} load errors; excluded (undefined upstream or not \
         ending): {} {excluded:?}; excluded (over {EMIT_MAX_KEYS} keys): {large}",
        excluded.values().sum::<u64>()
    );
    check.finish();
}

/// Random hmodel files, half well-formed and half also with wrong chunk
/// sizes, truncations and byte mutations: headers of pre-3.0, 3.0, current
/// and random versions with names of 0 to 16 characters, `NumConnections`
/// small, 0 or random (up to 65535), cut short or replaced by another
/// chunk; then nodes of the three kinds (names of 0 to 16 characters,
/// pivots small, 65535 or random, structs cut short), points chunks with
/// whole and partial points, the obsolete aux-data chunk and unknown
/// chunks, in any order; sometimes no header at all (an empty chunk).
/// Compared as [`emit_named_cases_match_upstream`].
///
/// Predicted before the run: no disagreement; about a fifth to a third
/// excluded as undefined (`IndexOutOfRange` from more nodes than
/// `NumConnections`, `StringOverrun` from 16-character node names,
/// `StringOverflow`).
///
/// Result (2026-11-06): 16,089 compared, 0 mismatches (11,293 loaded, 4,638
/// load errors); 3,911 excluded (20 %, in the predicted range): 3,415
/// `IndexOutOfRange`, 398 `StringOverrun`, 97 `StringOverflow`, 1
/// `NonTerminating`. Planted bug (pre-3.0 pivots not moved up): 1,257
/// mismatches here, 1 in the named cases.
#[test]
fn hmodel_load_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    run_emit_cases("HModelDefClass::Load_W3D (hmdldef.cpp)", 0, 0x4D0D, 20_000, gen_hmodel);
}

/// Random distance-LOD files, generated as the hmodels: a header (names of
/// 0 to 16 characters, `NumLODs` small, 0 or random, cut short or of the
/// wrong type), then as many LOD chunks as the header says or 0 to 5
/// (names of 0 to 32 characters, random distances, cut short, extended, of
/// the wrong type).
///
/// Predicted before the run: no disagreement; a few percent excluded as
/// undefined (`StringOverrun` from names that fill their struct).
///
/// Result (2026-11-06): 18,390 compared, 0 mismatches (10,249 loaded, 8,007
/// load errors); 1,610 excluded (8 %, more than the predicted few
/// percent), all `StringOverrun`. Planted bug (`ResUpDist` and
/// `ResDownDist` swapped): 6,173 mismatches here, 2 in the named cases.
#[test]
fn distlod_load_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    run_emit_cases("DistLODDefClass::Load_W3D (distlod.cpp)", 1, 0xD157, 20_000, gen_distlod);
}

/// Random particle-emitter files: versions 1.0 (converted), 2.0, small and
/// random; headers with names of 0 to 16 characters; user data with types,
/// strings terminated or not, `SizeofStringParam` matching, longer than
/// the chunk, 0, `2^31 - 1` or `0xFFFFFFFF`; info with texture names empty,
/// short, with a path, or of 260 characters, and random floats and colours;
/// info v2 with known and unknown randomizer classes and random values,
/// sometimes in a version-1 file; props with keyframe counts 0 (wraps),
/// small or random and as many keys as they need or fewer; the six
/// structures in order, sometimes swapped or one dropped; then line
/// properties, rotation, frame and blur-time chunks (counts small, random,
/// or 2^29; headers and keys cut short; extra bytes), extra info, obsolete
/// keyframe chunks and unknown chunks. Each case answers the texture lookup
/// with alpha or not at random.
///
/// Predicted before the run: no disagreement; about a third excluded as
/// undefined (`StringOverrun` from 16-character names and 260-character
/// textures, `Allocation` from wrapped and random counts,
/// `UninitialisedRead` from short headers and frame/blur keys,
/// `WriteToEmptyArray`); a handful over the key limit.
///
/// Result (2026-11-06): 12,688 compared, 0 mismatches (5,699 loaded, 6,965
/// load errors); 7,245 excluded (36 %, about the predicted third): 3,689
/// `Allocation`, 1,583 `StringOverrun`, 1,000 `UninitialisedRead`, 973
/// `WriteToEmptyArray`; 67 over the key limit. Planted bugs: the black
/// keyframe tweaked from index 0 (`>= 0`): 65 mismatches here, 1 in the
/// named cases; the loop not opening a chunk after a failure: 5,594 and 3;
/// the texture's alpha ignored (and the lookup skipped): 2,056 and 2.
#[test]
fn emitter_load_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    run_emit_cases("ParticleEmitterDefClass::Load_W3D (part_ldr.cpp)", 2, 0xE717, 20_000, gen_emitter);
}

/// An hmodel `name` over the hierarchy `hier` with one node.
fn am_hmodel(name: &[u8], hier: &[u8]) -> Vec<u8> {
    let mut b = emit_hmodel_header(W3D_CURRENT_HMODEL_VERSION, name, hier, 1);
    b.extend(emit_hmodel_node(W3D_CHUNK_NODE, b"BODY", 1));
    am_sub(W3D_CHUNK_HMODEL, &b)
}

/// A distance LOD `name` with one LOD.
fn am_distlod(name: &[u8]) -> Vec<u8> {
    let mut b = emit_lod_header(name, 1);
    b.extend(emit_lod(b"X.Y", 0.0, 100.0));
    am_sub(W3D_CHUNK_LODMODEL, &b)
}

/// A version-1 particle emitter `name` (with a texture, which the C++ and
/// the Rust managers both answer "no alpha" for).
fn am_emitter(name: &[u8]) -> Vec<u8> {
    let mut b = emit_header(0x0001_0000, name);
    b.extend(emit_user(0, 0, b""));
    b.extend(emit_info(b"spark.tga", |i| i.fade_time = 1.0));
    am_sub(W3D_CHUNK_EMITTER, &b)
}

/// [`am_gen_chunk`], with one chunk in four an hmodel, a distance LOD, an
/// emitter or an empty hmodel chunk (#17).
fn am_gen_chunk_17(g: &mut Gn) -> Vec<u8> {
    if g.below(4) != 0 {
        return am_gen_chunk(g);
    }
    let p = am_pick(g, &AM_PROTO);
    match g.below(4) {
        0 => am_hmodel(p, am_pick(g, &AM_HIER[..4])),
        1 => am_distlod(p),
        2 => am_emitter(p),
        _ => am_sub(W3D_CHUNK_HMODEL, &[]),
    }
}

/// The asset manager with the loaders of #17 registered: named cases (each
/// kind registered by name and class id in the library and the game
/// manager; an emitter in the library manager, which has no emitter loader;
/// an empty hmodel chunk, registered as `""`), then 2,000 random cases as
/// [`assetmgr_matches_upstream`] with one chunk in four of the new kinds
/// (names from the same pools). Compared as there.
///
/// Predicted before the run: no disagreement; the named cases' state shows
/// `HM` (class 24), `DL` (class 2), `EM` (class 14, game manager only) and
/// `""` (24); about a fifth of the random cases excluded, as in
/// [`assetmgr_matches_upstream`].
///
/// Result (2026-11-06): the 2 named cases and 1,692 of the 2,000 random
/// cases (14,109 operations) compared, 0 mismatches; 308 excluded (15 %):
/// 210 `StringOverrun`, 50 `PointerOutOfRange`, 30 `NonTerminating`, 15
/// `UninitialisedRead`, 2 `NoPivots`, 1 `NullPointer`. The named cases'
/// state was not inspected beyond the comparison; the unit test
/// `hmodel_distlod_and_emitter_prototypes` asserts the same names and class
/// ids on the Rust side.
#[test]
fn assetmgr_emit_loaders_match_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut check = Check::new("WW3DAssetManager with the hmodel, distlod and emitter loaders");
    let mut file = am_hmodel(b"HM", b"SKL");
    file.extend(am_distlod(b"DL"));
    file.extend(am_emitter(b"EM"));
    file.extend(am_sub(W3D_CHUNK_HMODEL, &[]));
    for game in [false, true] {
        let c = AmCase {
            game,
            user_data: None,
            files: vec![(s(b"Art/W3D/EM2.w3d"), am_emitter(b"EM2"))],
            ops: vec![AmOp::Bytes(file.clone()), AmOp::Create(s(b"em2")), AmOp::Create(s(b"hm")), AmOp::State],
        };
        if let Err(e) = rust_am_case(&c) {
            panic!("named case (game {game}): Rust error {e:?}");
        }
        assert!(am_compare(&mut check, "named", &c));
    }
    let mut g = Gn { g: Gen::new(0xA517), clean: false };
    let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
    let (mut compared, mut ops) = (0u64, 0u64);
    const CASES: usize = 2_000;
    for case in 0..CASES {
        g.clean = case % 2 == 0;
        let c = am_gen_case(&mut g, am_gen_chunk_17);
        match rust_am_case(&c) {
            Ok(l) => {
                let cpp = cpp_am_case(&c);
                check.eq(&l, &cpp, || format!("case {case}: {c:02x?}"));
                compared += 1;
                ops += c.ops.len() as u64;
            }
            Err(W3dError::Undefined(u)) => *excluded.entry(format!("{u:?}")).or_default() += 1,
            Err(e) => *excluded.entry(format!("{e:?}")).or_default() += 1,
        }
    }
    println!(
        "WW3DAssetManager (#17 loaders): {CASES} cases, {compared} compared ({ops} operations); excluded: {} {excluded:?}",
        excluded.values().sum::<u64>()
    );
    check.finish();
}

// ---------------------------------------------------------------------------
// 16. Particle emitters as render objects (part_emt.cpp, part_buf.cpp,
//     v3_rnd.cpp, random.cpp, quat.cpp's slerp; GitHub #49)
// ---------------------------------------------------------------------------

mod part16 {
    use super::*;
    use rts_lockstep_engine::formats::w3d::part_buf::{KeyTrack, ParticleDraw, RandomTable, Ww3dClock};
    use rts_lockstep_engine::formats::w3d::part_emt::{ParticleEmitter, ParticleRandom};
    use rts_lockstep_engine::formats::w3d::part_ldr;
    use rts_lockstep_engine::ww::matrix3d::Matrix3D;
    use rts_lockstep_engine::ww::quat::{build_matrix3d, Quaternion};
    use rts_lockstep_engine::ww::random::{Random3, Random4};
    use rts_lockstep_engine::ww::vector3::Vector3;
    use rts_lockstep_engine::ww::vector4::Vector4;

    /// The particle comparison's limit on the buffer size (larger buffers
    /// are kept out, for time).
    const PART_MAX_NUM: u32 = 100_000;

    /// One run: the emitter chunk, the seeds (Random3's two, Random4's),
    /// the creation time, and per frame the clock, operation and transform
    /// (`mats[0]` is the transform before the first frame).
    #[derive(Clone, Debug)]
    pub struct PartCase {
        pub data: Vec<u8>,
        pub seeds: [u32; 3],
        pub create_sync: u32,
        pub script: Vec<[u32; 5]>,
        pub mats: Vec<[f32; 12]>,
    }

    fn pf(log: &mut Log, f: f32) {
        p32(log, f.to_bits());
    }
    fn pv3(log: &mut Log, v: Vector3) {
        pf(log, v.x);
        pf(log, v.y);
        pf(log, v.z);
    }
    fn pv4(log: &mut Log, v: Vector4) {
        pf(log, v.x);
        pf(log, v.y);
        pf(log, v.z);
        pf(log, v.w);
    }

    fn log_track<T: Copy>(log: &mut Log, k: &KeyTrack<T>, val: fn(&mut Log, T)) {
        p32(log, k.num);
        let m = k.num.max(1) as usize;
        log.push(k.times.is_some() as u8);
        if let Some(t) = &k.times {
            t.iter().take(k.num as usize).for_each(|&x| p32(log, x));
        }
        log.push(k.values.is_some() as u8);
        if let Some(v) = &k.values {
            v.iter().take(m).for_each(|&x| val(log, x));
        }
        log.push(k.deltas.is_some() as u8);
        if let Some(d) = &k.deltas {
            d.iter().take(k.num as usize).for_each(|&x| val(log, x));
        }
    }

    fn log_table<T: Copy>(log: &mut Log, t: &RandomTable<T>, val: fn(&mut Log, T)) {
        p32(log, t.minus1);
        log.push(t.entries.is_some() as u8);
        if let Some(e) = &t.entries {
            e.iter().take(t.minus1 as usize + 1).for_each(|&x| val(log, x));
        }
    }

    /// wrap_w3d_part.cpp's log_created.
    fn log_created(log: &mut Log, e: &ParticleEmitter) {
        let b = &e.buffer;
        log.push(b'C');
        p32(log, e.emit_rate);
        p32(log, e.burst_size);
        p32(log, e.max_particles as u32);
        pv3(log, e.base_vel);
        pf(log, e.outward_vel);
        pf(log, e.vel_inherit_factor);
        p32(log, b.max_num);
        p32(log, b.max_age);
        p32(log, b.future_start_time);
        p32(log, b.render_mode as u32);
        log.push(b.has_accel as u8);
        pv3(log, b.accel);
        pf(log, b.max_size);
        p32(log, if b.has_point_group || b.has_line_group || b.has_line_renderer { b.shader.0 } else { 0xFFFF_FFFF });
        for x in [b.color.is_some(), b.alpha.is_some(), b.size.is_some(), b.orientation.is_some(), b.frame.is_some(), b.ucoord.is_some()] {
            log.push(x as u8);
        }
        log_track(log, &b.color_keys, pv3);
        log_track(log, &b.alpha_keys, pf);
        log_track(log, &b.size_keys, pf);
        log_track(log, &b.frame_keys, pf);
        log_track(log, &b.blur_time_keys, pf);
        let r = &b.rotation_keys;
        p32(log, r.num);
        log.push(r.times.is_some() as u8);
        if let Some(t) = &r.times {
            for i in 0..r.num as usize {
                p32(log, t[i]);
                pf(log, r.values.as_ref().unwrap()[i]);
                pf(log, r.half_deltas.as_ref().unwrap()[i]);
                pf(log, r.orientations.as_ref().unwrap()[i]);
            }
        }
        if b.color.is_some() {
            log_table(log, &b.random_color, pv3);
        }
        if b.alpha.is_some() {
            log_table(log, &b.random_alpha, pf);
        }
        if b.size.is_some() {
            log_table(log, &b.random_size, pf);
        }
        if b.orientation.is_some() {
            log_table(log, &b.random_rotation, pf);
            log_table(log, &b.random_orientation, pf);
        }
        if b.frame.is_some() || b.ucoord.is_some() {
            log_table(log, &b.random_frame, pf);
        }
        if b.blur_time_keys.times.is_some() {
            log_table(log, &b.random_blur_time, pf);
        }
    }

    /// wrap_w3d_part.cpp's log_state.
    fn log_state(log: &mut Log, e: &ParticleEmitter) {
        let b = &e.buffer;
        log.push(b'S');
        p32(log, e.emit_remain);
        p32(log, e.particles_left as u32);
        log.push(e.is_complete as u8);
        log.push(e.active as u8);
        log.push(e.group_id);
        pv3(log, e.prev_orig);
        for x in [e.prev_q.x, e.prev_q.y, e.prev_q.z, e.prev_q.w] {
            pf(log, x);
        }
        for x in [b.start, b.end, b.new_end, b.non_new_num as u32, b.new_num as u32] {
            p32(log, x);
        }
        for x in [b.new_particle_queue_start, b.new_particle_queue_end, b.new_particle_queue_count as u32, b.last_update_time] {
            p32(log, x);
        }
        pv3(log, b.bounding_box.center);
        pv3(log, b.bounding_box.extent);
    }

    /// wrap_w3d_part.cpp's log_points, log_lines, log_line_group.
    fn log_draw(log: &mut Log, d: &ParticleDraw) -> u64 {
        match d {
            ParticleDraw::Points(p) => {
                log.push(b'P');
                log.push(p.quads as u8);
                log.push(p.frame_row_column_count_log2);
                p32(log, p.points.len() as u32);
                for s in &p.points {
                    p32(log, s.index);
                    pv3(log, s.position);
                    pv4(log, s.diffuse);
                    pf(log, s.size);
                    log.push(s.orientation);
                    log.push(s.frame);
                }
                p.points.len() as u64
            }
            ParticleDraw::Lines { width, runs } => {
                log.push(b'L');
                pf(log, *width);
                p32(log, runs.len() as u32);
                let mut n = 0;
                for r in runs {
                    p32(log, r.points.len() as u32);
                    for (p, c) in r.points.iter().zip(&r.colors) {
                        pv3(log, *p);
                        pv4(log, *c);
                    }
                    n += r.points.len() as u64;
                }
                n
            }
            ParticleDraw::LineGroup(g) => {
                log.push(b'G');
                log.push(g.prism as u8);
                p32(log, g.lines.len() as u32);
                for l in &g.lines {
                    p32(log, l.index);
                    pv3(log, l.head);
                    pv3(log, l.tail);
                    pv4(log, l.head_diffuse);
                    pv4(log, l.tail_diffuse);
                    pf(log, l.size);
                    pf(log, l.ucoord);
                }
                g.lines.len() as u64
            }
            ParticleDraw::Nothing => {
                log.push(b'X');
                0
            }
        }
    }

    fn mat(m: &[f32; 12]) -> Matrix3D {
        Matrix3D::new(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], m[10], m[11])
    }

    /// `ref_w3d_part_run` in Rust: `Ok(None)` for a buffer over
    /// [`PART_MAX_NUM`]; the second value counts the particles drawn.
    pub fn rust_part_run(c: &PartCase) -> W3dResult<Option<(Log, u64)>> {
        let mut log = Log::new();
        let mut cl = ChunkLoad::new(GameFile::new(&c.data).unwrap());
        assert!(cl.open_chunk()?);
        let Some(def) = part_ldr::load_w3d(&mut cl, |_| panic!("version 1"))? else {
            log.push(b'e');
            return Ok(Some((log, 0)));
        };
        let mut rng = ParticleRandom { v3: Random3::new(c.seeds[0], c.seeds[1]), buf: Random4::new(c.seeds[2]) };
        let mut clock = Ww3dClock { sync_time: c.create_sync, frame_time: 0, frame_count: 0 };
        let mut e = ParticleEmitter::create_from_definition(&def, &mut rng, &clock)?;
        if e.buffer.max_num > PART_MAX_NUM {
            return Ok(None);
        }
        log_created(&mut log, &e);
        e.set_transform(mat(&c.mats[0]));
        e.notify_added();
        e.set_hidden(false);
        let mut drawn = 0;
        for (f, s) in c.script.iter().enumerate() {
            clock = Ww3dClock { sync_time: s[0], frame_time: s[1], frame_count: s[2] };
            e.set_transform(mat(&c.mats[f + 1]));
            match s[3] {
                1 => e.set_hidden(true),
                2 => e.set_hidden(false),
                3 => e.set_one_time_burst(s[4] as i32),
                4 => e.reset(),
                5 => e.stop(),
                6 => e.start(),
                _ => {}
            }
            log.push(b'F');
            e.on_frame_update();
            if e.buffer_in_scene {
                e.emit(&clock, &mut rng)?;
                let at = (e.get_position(), e.is_stopped());
                let d = e.buffer.render(&clock, Some(at))?;
                drawn += log_draw(&mut log, &d);
            } else {
                log.push(b'N');
            }
            log_state(&mut log, &e);
        }
        Ok(Some((log, drawn)))
    }

    pub fn cpp_part_run(c: &PartCase) -> Log {
        let script: Vec<u32> = c.script.iter().flatten().copied().collect();
        let mats: Vec<f32> = c.mats.iter().flatten().copied().collect();
        cpp_log(|log, cap, n| unsafe {
            cpp::ref_w3d_part_run(
                c.data.as_ptr() as *const _,
                c.data.len() as i32,
                0,
                c.seeds.as_ptr(),
                c.create_sync,
                script.as_ptr(),
                mats.as_ptr(),
                c.script.len() as i32,
                log,
                cap,
                n,
            )
        })
    }

    fn key8(t: f32, v: [u8; 4]) -> Vec<u8> {
        let mut b = t.to_le_bytes().to_vec();
        b.extend(v);
        b
    }

    /// Increasing key times up to about `span` seconds; sometimes equal,
    /// decreasing, past the lifetime, or a hair below 0 (truncated to 0).
    fn key_times(g: &mut Gn, n: usize, span: f32) -> Vec<f32> {
        let mut t = 0.0f32;
        (0..n)
            .map(|_| {
                t += g.g.uniform(0.005, span.max(0.01) * 0.6);
                match g.below(30) {
                    0 => t - 0.5 * span,
                    1 => -0.0004,
                    2 => t * 3.0,
                    _ => t,
                }
            })
            .collect()
    }

    /// A version-2 emitter that mostly loads: lifetime, rate, burst and
    /// maximum emissions in game-like ranges (with 0, negative and rate
    /// over 1000 sometimes), velocities, accelerations, each randomizer
    /// class (an unknown one rarely), outward velocity, inheritance, a
    /// random shader, every render and frame mode (and unknown ones),
    /// colour, opacity and size keyframes with random ranges or none,
    /// rotation, frame and blur-time keyframes, line properties and a future
    /// start time.
    pub fn gen_part_def(g: &mut Gn) -> Vec<u8> {
        let lifetime = match g.below(20) {
            0 => 0.0,
            1 => -1.0,
            2 => g.g.uniform(0.0005, 0.01),
            _ => g.g.uniform(0.05, 4.0),
        };
        let mut body = emit_header(0x0002_0000, b"PART");
        body.extend(emit_user(0, 0, b""));
        let tex: &[u8] = if g.coin() { b"" } else { b"fx.tga" };
        body.extend(emit_info(tex, |i| {
            i.lifetime = lifetime;
            i.emission_rate = match g.below(20) {
                0 => 0.0,
                1 => -2.0,
                2 => 1500.0,
                _ => g.g.uniform(0.5, 60.0),
            };
            i.max_emissions = match g.below(4) {
                0 => g.below(40) as f32,
                1 => -3.0,
                _ => 0.0,
            };
            (i.velocity.x, i.velocity.y, i.velocity.z) = (g.g.uniform(-20.0, 20.0), g.g.uniform(-20.0, 20.0), g.g.uniform(-20.0, 20.0));
            if g.coin() {
                (i.acceleration.x, i.acceleration.y, i.acceleration.z) = (g.g.uniform(-10.0, 10.0), g.g.uniform(-10.0, 10.0), g.g.uniform(-30.0, 5.0));
            }
        }));
        body.extend(emit_info_v2(|v| {
            v.burst_size = match g.below(10) {
                0 => 0,
                1 => 12 + g.below(20),
                _ => 1 + g.below(4),
            };
            for r in [&mut v.creation_volume, &mut v.vel_random] {
                r.class_id = if g.below(60) == 0 { 7 } else { g.below(4) };
                (r.value1, r.value2, r.value3) = (g.g.uniform(-0.5, 5.0), g.g.uniform(-0.5, 5.0), g.g.uniform(-0.5, 5.0));
            }
            v.outward_vel = if g.coin() { 0.0 } else { g.g.uniform(-5.0, 5.0) };
            v.vel_inherit = if g.coin() { 0.0 } else { g.g.uniform(-1.0, 2.0) };
            let s = g.g.u64().to_le_bytes();
            let mut sh = [0u8; 16];
            for (k, b) in sh.iter_mut().enumerate().take(14) {
                *b = s[k % 8] % 4;
            }
            v.shader = W3dShaderStruct::from_bytes(&sh);
            v.render_mode = if g.below(30) == 0 { 7 } else { g.below(5) };
            v.frame_mode = if g.below(20) == 0 { 9 } else { g.below(5) };
        }));
        // props: colour, opacity and size keyframes
        let span = if lifetime > 0.0 { lifetime } else { 1.0 };
        let counts = [g.below(4), g.below(4), g.below(4)];
        let mut p = W3dEmitterPropertyStruct::from_bytes(&[0; 40]);
        (p.color_keyframes, p.opacity_keyframes, p.size_keyframes) = (counts[0] + 1, counts[1] + 1, counts[2] + 1);
        let c = if g.coin() { [0u8; 4] } else { g.g.u32().to_le_bytes() };
        p.color_random = W3dRGBAStruct { r: c[0], g: c[1], b: c[2], a: c[3] };
        p.opacity_random = if g.coin() { 0.0 } else { g.g.uniform(0.0, 0.5) };
        p.size_random = if g.coin() { 0.0 } else { g.g.uniform(0.0, 1.0) };
        let mut pb = p.to_bytes();
        pb.extend(key8(0.0, g.g.u32().to_le_bytes()));
        for t in key_times(g, counts[0] as usize, span) {
            pb.extend(key8(t, if g.below(4) == 0 { [0; 4] } else { g.g.u32().to_le_bytes() }));
        }
        pb.extend(key8(0.0, g.g.uniform(0.0, 1.0).to_le_bytes()));
        for t in key_times(g, counts[1] as usize, span) {
            pb.extend(key8(t, g.g.uniform(-0.2, 1.2).to_le_bytes()));
        }
        pb.extend(key8(0.0, g.g.uniform(0.0, 3.0).to_le_bytes()));
        for t in key_times(g, counts[2] as usize, span) {
            pb.extend(key8(t, g.g.uniform(-1.0, 6.0).to_le_bytes()));
        }
        body.extend(hchunk(W3D_CHUNK_EMITTER_PROPS, &pb));
        if g.coin() {
            let mut lp = W3dEmitterLinePropertiesStruct::from_bytes(&[0; 64]);
            lp.flags = g.g.u32() & 0x0300_000F;
            lp.subdivision_level = g.below(5);
            lp.noise_amplitude = g.g.uniform(0.0, 2.0);
            lp.texture_tile_factor = g.g.uniform(0.0, 4.0);
            body.extend(hchunk(W3D_CHUNK_EMITTER_LINE_PROPERTIES, &lp.to_bytes()));
        }
        // rotation (header: count, random, orientation random, pad), frame (count, random, 8 pad), blur (count, random, pad)
        for (id, chance, hsize, range) in [
            (W3D_CHUNK_EMITTER_ROTATION_KEYFRAMES, 2, 16, 3.0f32),
            (W3D_CHUNK_EMITTER_FRAME_KEYFRAMES, 2, 16, 16.0),
            (W3D_CHUNK_EMITTER_BLUR_TIME_KEYFRAMES, 3, 12, 0.2),
        ] {
            if g.below(chance) != 0 {
                continue;
            }
            let n = g.below(4);
            let mut b = n.to_le_bytes().to_vec();
            b.extend((if g.coin() { 0.0 } else { g.g.uniform(0.0, range * 0.3) }).to_le_bytes());
            if hsize == 16 {
                let o = if id == W3D_CHUNK_EMITTER_ROTATION_KEYFRAMES && g.coin() { g.g.uniform(0.0, 1.0) } else { 0.0f32 };
                b.extend(o.to_le_bytes());
            }
            b.extend([0u8; 4]);
            let start = if g.below(3) == 0 { 0.0 } else { g.g.uniform(-range, range) };
            b.extend(key8(0.0, start.to_le_bytes()));
            for t in key_times(g, n as usize, span) {
                b.extend(key8(t, g.g.uniform(-range, range).to_le_bytes()));
            }
            body.extend(hchunk(id, &b));
        }
        if g.coin() {
            let mut eb = (if g.below(6) == 0 { -0.5f32 } else { g.g.uniform(0.0, 2.0) }).to_le_bytes().to_vec();
            eb.extend([0u8; 36]);
            body.extend(hchunk(W3D_CHUNK_EMITTER_EXTRA_INFO, &eb));
        }
        hchunk(W3D_CHUNK_EMITTER, &body)
    }

    /// A random run: 10 to 40 frames of 0 to 50 ms mostly, longer ones
    /// sometimes (up to 20 s, which takes the emission time modulo the
    /// buffer's cycle), with hides, shows, one-time bursts, resets, stops
    /// and starts; the emitter moving and turning (a unit quaternion made a
    /// matrix), still or teleported sometimes.
    pub fn gen_part_case(g: &mut Gn) -> PartCase {
        let data = gen_part_def(g);
        let seeds = [g.g.u32(), g.g.u32(), g.g.u32()];
        let create_sync = g.below(100_000);
        let frames = 10 + g.below(31) as usize;
        let mut sync = create_sync;
        let mut script = Vec::with_capacity(frames);
        for f in 0..frames {
            let dt = match g.below(20) {
                0 | 1 => 0,
                2..=13 => 1 + g.below(50),
                14..=18 => 50 + g.below(350),
                _ => 3000 + g.below(17_000),
            };
            sync = sync.wrapping_add(dt);
            let op = match g.below(50) {
                0..=2 => 1,
                3..=5 => 2,
                6..=8 => 3,
                9 => 4,
                10 => 5,
                11 => 6,
                _ => 0,
            };
            script.push([sync, dt, f as u32 + 1, op, g.below(6)]);
        }
        let mut pos = [g.g.uniform(-50.0, 50.0), g.g.uniform(-50.0, 50.0), g.g.uniform(0.0, 20.0)];
        let vel = [g.g.uniform(-0.05, 0.05), g.g.uniform(-0.05, 0.05), 0.0];
        let spin = g.g.uniform(-0.003, 0.003);
        let still = g.below(4) == 0;
        let mut angle = g.g.uniform(-3.0, 3.0);
        let mut mats = Vec::with_capacity(frames + 1);
        for k in 0..=frames {
            if k > 0 && !still {
                let dt = script[k - 1][1] as f32;
                for i in 0..3 {
                    pos[i] += vel[i] * dt;
                }
                angle += spin * dt;
                if g.below(40) == 0 {
                    pos[0] += g.g.uniform(-100.0, 100.0);
                }
            }
            let h = angle * 0.5;
            let tilt = 0.3f32;
            let q = Quaternion::new(
                rts_lockstep_engine::ww::wwmath::sin(tilt) * rts_lockstep_engine::ww::wwmath::cos(h),
                0.0,
                rts_lockstep_engine::ww::wwmath::sin(h),
                rts_lockstep_engine::ww::wwmath::cos(h) * rts_lockstep_engine::ww::wwmath::cos(tilt),
            );
            let mut m = build_matrix3d(&q);
            m.set_translation(&Vector3::new(pos[0], pos[1], pos[2]));
            mats.push(m.to_array());
        }
        PartCase { data, seeds, create_sync, script, mats }
    }

    /// Particle emitters run frame by frame against the compiled
    /// part_emt.cpp and part_buf.cpp (see wrap_w3d_part.cpp): 3,000 random
    /// runs ([`gen_part_case`]); compared, byte for byte (floats as raw
    /// bits), the emitter and buffer as created (emission period, burst
    /// size, buffer size, lifetime, acceleration, shader after `Enable_Fog`,
    /// every keyframe time, value and delta, the rotation's integrated
    /// orientations, every randomizer table) and after each frame the
    /// emitter's counters and previous transform, the ring's and queue's
    /// indices, the bounding box, and what the renderer was given: every
    /// point's slot, position, colour and alpha, size, orientation and frame
    /// (triangles and quads), every line run's points and colours (line
    /// mode), every line's head, tail, colours, size and u coordinate (line
    /// groups). The same seeds on both sides.
    ///
    /// Predicted before the run: no disagreement; a few percent excluded as
    /// undefined: `NullPointer` (an unknown randomizer class, about 1 in 30),
    /// `FloatToInt` (lifetimes or key times that make a negative or huge
    /// `unsigned`, a hair-below-0 key is defined), `NonTerminating` (a rate
    /// over 1000 with no maximum, about 1 in 30); a third or so of the runs
    /// drawing nothing in some frames (hidden, rate 0, or tiny lifetimes).
    ///
    /// Result (2026-11-07): 2,261 of the 3,000 runs compared (56,870 frames,
    /// 3,360,104 particles or line points drawn; runs drawing points 993,
    /// line runs 684, line groups 1,003, an unknown render mode 357), 0
    /// mismatches. **Prediction missed:** 736 runs (25 %, not a few percent)
    /// excluded as undefined: 561 `FloatToInt` (most from the generator's
    /// negative future start times, -0.5 s, and its key times half a span
    /// back, which upstream converts to `unsigned`), 98 `NullPointer`, 77
    /// `NonTerminating`; 3 over the buffer limit. Planted bugs, each run
    /// once on the 3,000 runs: `Cached_Slerp` "fixed" to divide by sin theta:
    /// 1,158 mismatching runs; a particle kept while its age equals `MaxAge`
    /// (`<=` in `Kill_Old_Particles`): 231; the colour randomizer's three
    /// draws taken right to left (MSVC's argument order): 1,148;
    /// `VectorProcessorClass::Clamp` as if its arrays were distinct (the top
    /// clamp only): 1,119.
    #[test]
    fn particle_emitters_match_upstream() {
        let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let cases: usize = std::env::var("PART_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(3_000);
        let mut g = Gn { g: Gen::new(0x9A47), clean: true };
        let mut check = Check::new("ParticleEmitterClass/ParticleBufferClass");
        let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
        let (mut compared, mut frames, mut drawn, mut large, mut load_errors) = (0u64, 0u64, 0u64, 0u64, 0u64);
        let mut modes = [0u64; 4];
        for case in 0..cases {
            let c = gen_part_case(&mut g);
            let (rust, n) = match rust_part_run(&c) {
                Ok(Some(r)) => r,
                Ok(None) => {
                    large += 1;
                    continue;
                }
                Err(W3dError::Undefined(u)) => {
                    *excluded.entry(format!("{u:?}")).or_default() += 1;
                    continue;
                }
                Err(e) => {
                    *excluded.entry(format!("{e:?}")).or_default() += 1;
                    continue;
                }
            };
            if rust == b"e" {
                load_errors += 1;
            }
            for (k, tag) in [b'P', b'L', b'G', b'X'].iter().enumerate() {
                if rust.windows(2).any(|w| w[0] == b'F' && w[1] == *tag) {
                    modes[k] += 1;
                }
            }
            let cpp = cpp_part_run(&c);
            check.eq(&rust, &cpp, || format!("case {case}: {c:?}"));
            compared += 1;
            frames += c.script.len() as u64;
            drawn += n;
        }
        println!(
            "particles: {cases} runs, {compared} compared ({frames} frames, {drawn} particles or line points drawn; runs with \
             points {}, lines {}, line groups {}, no renderer {}; {load_errors} definitions not loading); excluded (undefined \
             upstream): {} {excluded:?}; over {PART_MAX_NUM} particles: {large}",
            modes[0],
            modes[1],
            modes[2],
            modes[3],
            excluded.values().sum::<u64>()
        );
        check.finish();
    }
}
