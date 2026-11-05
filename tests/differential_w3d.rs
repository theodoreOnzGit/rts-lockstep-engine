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
//! (`differential/wrap_w3d_morph.cpp`).
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
