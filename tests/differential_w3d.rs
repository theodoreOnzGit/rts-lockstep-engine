// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `ww::chunkio` and `formats::w3d` against the
//! upstream C++ (commit 0a05454d): `WWLib/chunkio.cpp`, the real
//! `WW3D2/w3d_file.h`, and the loaders of `htree.cpp`, `motchan.cpp`,
//! `hrawanim.cpp` and `hcanim.cpp` (see `differential/wrap_w3d.cpp` and
//! `build.rs`; the file underneath is the real `RAMFile.cpp`).
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
