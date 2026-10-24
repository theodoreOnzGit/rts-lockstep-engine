// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `formats::data_chunk` against upstream
//! `DataChunk.cpp` and `NameKeyGenerator.cpp` (commit 0a05454d), compiled
//! unchanged by g++ (see `differential/wrap_chunk.cpp`; the `Dict` that
//! `readDict` fills is a stand-in with `Dict.cpp`'s set semantics, because
//! `Dict.cpp` cannot hold the shim strings). The Rust side fills
//! `common::dict::Dict` and keys names through `common::name_key_generator`
//! (moved there 2026-10-18, `p1w2-dictmove`): the same 20,000 files gave the
//! same counts before and after the move (11,849 parsed, 136 threw, 3,953,898
//! log bytes, 249 excluded as undefined, 52 with a table past 10^5 entries,
//! 19,699 compared, 0 mismatches).
//!
//! No game maps are available, so every chunk file is **synthetic**, built
//! byte by byte in the layout the upstream reader reads: a table of contents
//! (right or wrong tag, counts equal to, above and below the names present,
//! zero and negative, duplicate names and ids, empty and 255-byte names),
//! then nested chunks whose ids name, or do not name, a table entry, with
//! data sizes that are right, short, long, zero, negative or huge. The chunk
//! data is a script that the test parser (the same interpreter on both
//! sides) executes: `readInt`, `readReal`, `readByte`, `readAsciiString`,
//! `readUnicodeString`, `readArrayOfBytes`, a nested `parse`, `readDict`
//! (all five types, unknown types, more than 32767 pairs), `readNameKey`,
//! the chunk-size queries, failing, stopping. Parsers are registered under
//! several labels and scopes, two for the same label and scope (the newest
//! must win), one that fails and one that reads nothing. Half of the files
//! are well-formed; the others also get truncations and byte mutations.
//!
//! Compared per file: the table-of-contents lookups (`getID` of 8 names,
//! `getName` of 13 ids), `isValidFileType`, everything each parser saw and
//! read (the log, byte for byte), both results of `parse` (before and after
//! `reset`), and the `ErrorCode` thrown, if any.
//!
//! Files for which the Rust reports [`ChunkError::Undefined`] are **not**
//! given to the C++ (counted per kind), nor are files whose table of
//! contents declares more than 10^5 entries past the end of the file
//! (upstream allocates every one; counted).
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::common::dict::DataType;
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::formats::data_chunk::{
    CachedFileInputStream, ChunkError, ChunkResult, DataChunkInfo, DataChunkInput,
};
use std::collections::BTreeMap;
use support::{Check, Gen};

type Log = Vec<u8>;

/// What the parsers get: the log, and the name keys (upstream's
/// `TheNameKeyGenerator`, which `readNameKey` and `readDict` take here).
#[derive(Default)]
struct Ctx {
    log: Log,
    names: NameKeyGenerator,
}

fn put_str(log: &mut Log, s: &[u8]) {
    log.extend_from_slice(&(s.len() as u32).to_le_bytes());
    log.extend_from_slice(s);
}

fn put_wide(log: &mut Log, s: &[u16]) {
    log.extend_from_slice(&(s.len() as u32).to_le_bytes());
    for u in s {
        log.extend_from_slice(&u.to_le_bytes());
    }
}

/// `interp` of wrap_chunk.cpp.
fn interp(file: &mut DataChunkInput<Ctx>, info: &DataChunkInfo, ctx: &mut Ctx) -> ChunkResult<bool> {
    ctx.log.push(b'I');
    put_str(&mut ctx.log, &info.label);
    put_str(&mut ctx.log, &info.parent_label);
    ctx.log.extend_from_slice(&info.version.to_le_bytes());
    ctx.log.extend_from_slice(&info.data_size.to_le_bytes());
    ctx.log.extend_from_slice(&file.get_chunk_data_size().to_le_bytes());
    ctx.log.extend_from_slice(&file.get_chunk_data_size_left().to_le_bytes());
    ctx.log.extend_from_slice(&file.get_chunk_version().to_le_bytes());
    put_str(&mut ctx.log, &file.get_chunk_label());
    for _ in 0..64 {
        if file.at_end_of_chunk() {
            ctx.log.push(b'E');
            break;
        }
        let op = file.read_byte()? as u8;
        ctx.log.extend_from_slice(&[b'o', op]);
        match op % 12 {
            0 => ctx.log.extend_from_slice(&file.read_int()?.to_le_bytes()),
            1 => ctx.log.extend_from_slice(&file.read_real()?.to_le_bytes()),
            2 => ctx.log.push(file.read_byte()? as u8),
            3 => {
                let s = file.read_ascii_string()?;
                put_str(&mut ctx.log, &s);
            }
            4 => {
                let s = file.read_unicode_string()?;
                put_wide(&mut ctx.log, &s);
            }
            5 => {
                let n = (file.read_byte()? as u8 & 0x3f) as usize;
                let mut buf = [0u8; 64];
                file.read_array_of_bytes(&mut buf, n as i32)?;
                ctx.log.push(n as u8);
                ctx.log.extend_from_slice(&buf[..n]);
            }
            6 => {
                let r = file.parse(ctx)?;
                ctx.log.extend_from_slice(&[b'p', u8::from(r)]);
            }
            7 => {
                let d = file.read_dict(&mut ctx.names)?;
                let mut v: Vec<(Vec<u8>, i32)> =
                    (0..d.get_pair_count()).map(|n| (ctx.names.key_to_name(d.get_nth_key(n)).to_vec(), n)).collect();
                v.sort_by(|a, b| a.0.cmp(&b.0));
                ctx.log.extend_from_slice(&(v.len() as u32).to_le_bytes());
                for (name, n) in v {
                    put_str(&mut ctx.log, &name);
                    match d.get_nth_type(n) {
                        DataType::Bool => ctx.log.extend_from_slice(&[0, u8::from(d.try_get_nth_bool(n).expect("in range"))]),
                        DataType::Int => {
                            ctx.log.push(1);
                            ctx.log.extend_from_slice(&d.try_get_nth_int(n).expect("in range").to_le_bytes());
                        }
                        DataType::Real => {
                            ctx.log.push(2);
                            ctx.log.extend_from_slice(&d.try_get_nth_real(n).expect("in range").to_le_bytes());
                        }
                        DataType::AsciiString => {
                            ctx.log.push(3);
                            put_str(&mut ctx.log, &d.try_get_nth_ascii_string(n).expect("in range"));
                        }
                        DataType::UnicodeString => {
                            ctx.log.push(4);
                            put_wide(&mut ctx.log, &d.try_get_nth_unicode_string(n).expect("in range"));
                        }
                        DataType::None => unreachable!("a pair always has a type"),
                    }
                }
            }
            8 => {
                let k = file.read_name_key(&mut ctx.names)?;
                put_str(&mut ctx.log, ctx.names.key_to_name(k));
            }
            9 => {
                ctx.log.push(b'z');
                ctx.log.extend_from_slice(&file.get_chunk_data_size_left().to_le_bytes());
                ctx.log.push(u8::from(file.at_end_of_file()));
            }
            10 => {
                ctx.log.push(b'F');
                return Ok(false);
            }
            _ => {
                ctx.log.push(b'S');
                return Ok(true);
            }
        }
    }
    Ok(true)
}

fn fail_parser(_: &mut DataChunkInput<Ctx>, _: &DataChunkInfo, ctx: &mut Ctx) -> ChunkResult<bool> {
    ctx.log.push(b'X');
    Ok(false)
}

fn quiet_parser(_: &mut DataChunkInput<Ctx>, info: &DataChunkInfo, ctx: &mut Ctx) -> ChunkResult<bool> {
    ctx.log.push(b'Q');
    ctx.log.extend_from_slice(&info.data_size.to_le_bytes());
    Ok(true)
}

/// `ref_chunk_run` of wrap_chunk.cpp: (status, code, log).
/// Also the number of table entries past the end of the file.
fn rust_run(data: &[u8]) -> Result<(i32, u32, Log, u64), ChunkError> {
    let mut ctx = Ctx::default();
    ctx.names.init(); // as the wrapper's `new NameKeyGenerator` + init()
    let mut eof_mappings = 0;
    let r = (|| -> ChunkResult<()> {
        let mut input = DataChunkInput::new(CachedFileInputStream::new(data.to_vec()).expect("non-empty"))?;
        eof_mappings = input.contents().eof_mappings();
        ctx.log.push(u8::from(input.is_valid_file_type()));
        for name in [&b""[..], b"A", b"B", b"C", b"D", b"Fail", b"Unknown", b"a"] {
            ctx.log.extend_from_slice(&input.contents().get_id(name).to_le_bytes());
        }
        for id in (0..12).chain([0xFFFF_FFFF]) {
            put_str(&mut ctx.log, &input.contents().get_name(id));
        }
        input.register_parser(b"A", b"", fail_parser);
        input.register_parser(b"A", b"", interp);
        input.register_parser(b"B", b"A", interp);
        input.register_parser(b"C", b"", interp);
        input.register_parser(b"C", b"B", interp);
        input.register_parser(b"A", b"A", interp);
        input.register_parser(b"B", b"B", interp);
        input.register_parser(b"Fail", b"", fail_parser);
        input.register_parser(b"D", b"", quiet_parser);
        input.register_parser(b"D", b"A", quiet_parser);
        let r = input.parse(&mut ctx)?;
        ctx.log.extend_from_slice(&[b'R', u8::from(r)]);
        input.reset();
        let r = input.parse(&mut ctx)?;
        ctx.log.extend_from_slice(&[b'R', u8::from(r)]);
        Ok(())
    })();
    match r {
        Ok(()) => Ok((0, 0, ctx.log, eof_mappings)),
        Err(ChunkError::Thrown(c)) => Ok((3, c, ctx.log, eof_mappings)),
        Err(e) => Err(e),
    }
}

fn cpp_run(data: &[u8]) -> (i32, u32, Log) {
    let mut log = vec![0u8; 1 << 22];
    let (mut n, mut code) = (0i32, 0u32);
    let status =
        unsafe { cpp::ref_chunk_run(data.as_ptr() as *const _, data.len() as i32, log.as_mut_ptr(), log.len() as i32, &mut n, &mut code) };
    assert!(n as usize <= log.len(), "C++ log overflow");
    log.truncate(n as usize);
    (status, code, log)
}

// ---------------------------------------------------------------------------
// The generator
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
}

fn w32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn w16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}

const NAMES: &[&[u8]] = &[b"A", b"B", b"C", b"D", b"Fail"];

fn gen_toc(g: &mut Gn) -> (Vec<u8>, Vec<u32>) {
    let mut entries: Vec<(Vec<u8>, u32)> = NAMES.iter().enumerate().map(|(i, n)| (n.to_vec(), i as u32 + 1)).collect();
    for _ in 0..g.below(4) {
        let name = match g.below(6) {
            0 => Vec::new(),
            1 => b"a".to_vec(),
            2 => vec![b'L'; 255],
            3 => b"x\x00y".to_vec(),
            _ => NAMES[g.below(NAMES.len() as u32) as usize].to_vec(),
        };
        let id = match g.below(5) {
            0 => 0,
            1 => g.g.u32(),
            _ => 1 + g.below(12),
        };
        entries.push((name, id));
    }
    // shuffle
    for i in (1..entries.len()).rev() {
        let j = g.below(i as u32 + 1) as usize;
        entries.swap(i, j);
    }
    let ids: Vec<u32> = entries.iter().map(|e| e.1).collect();
    let mut v = if g.chance(25) { b"CkMq".to_vec() } else { b"CkMp".to_vec() };
    let count = if g.chance(10) {
        match g.below(4) {
            0 => 0,
            1 => -1,
            2 => entries.len() as i32 + 1 + g.below(3) as i32,
            _ => entries.len() as i32 - 1 - g.below(3) as i32,
        }
    } else {
        entries.len() as i32
    };
    w32(&mut v, count as u32);
    for (name, id) in &entries {
        v.push(name.len() as u8);
        v.extend_from_slice(name);
        w32(&mut v, *id);
    }
    (v, ids)
}

fn gen_ops(g: &mut Gn, ids: &[u32], depth: u32) -> Vec<u8> {
    let mut v = Vec::new();
    let n = g.below(8);
    for _ in 0..n {
        let kind = match g.below(40) {
            0 => 10,
            1 => 11,
            _ => g.below(10),
        };
        v.push((kind + 12 * g.below(21)) as u8);
        match kind {
            0 => w32(&mut v, g.g.u32()),
            1 => w32(&mut v, [1.5f32, -0.0, 3.0e38, f32::MIN_POSITIVE, 0.1].map(f32::to_bits)[g.below(5) as usize] ^ ((g.below(2) * g.g.u32()) & 0xff)),
            2 => v.push(g.g.u32() as u8),
            3 => {
                let len = g.below(10) as usize;
                w16(&mut v, if g.chance(15) { 300 } else { len as u16 });
                for _ in 0..len {
                    v.push(if g.below(10) == 0 { 0 } else { b'a' + g.below(26) as u8 });
                }
            }
            4 => {
                let len = g.below(6) as usize;
                w16(&mut v, len as u16);
                for _ in 0..len {
                    w16(&mut v, if g.below(10) == 0 { 0 } else { g.g.u32() as u16 | 1 });
                }
            }
            5 => {
                let n = g.below(64) as usize;
                v.push(n as u8 | (g.below(4) as u8) << 6);
                for _ in 0..n {
                    v.push(g.g.u32() as u8);
                }
            }
            6 => {
                // nested chunks fill the rest of this chunk
                if depth < 3 {
                    for _ in 0..g.below(3) {
                        v.extend(gen_chunk(g, ids, depth + 1));
                    }
                }
                return v;
            }
            7 => {
                let len = if g.chance(30) { 40000 } else { g.below(5) as u16 };
                w16(&mut v, len);
                for _ in 0..len.min(5) {
                    let t = if g.chance(20) { 7 } else { g.below(5) };
                    let id = if g.below(6) == 0 { g.g.u32() >> 8 } else { ids[g.below(ids.len() as u32) as usize] };
                    w32(&mut v, (id << 8) | t);
                    match t {
                        0 => v.push(g.below(3) as u8),
                        1 | 2 => w32(&mut v, g.g.u32()),
                        3 => {
                            let l = g.below(5) as usize;
                            w16(&mut v, l as u16);
                            v.extend((0..l).map(|_| b'k' + g.below(5) as u8));
                        }
                        4 => {
                            let l = g.below(4) as usize;
                            w16(&mut v, l as u16);
                            for _ in 0..l {
                                w16(&mut v, 0x41 + g.below(3) as u16);
                            }
                        }
                        _ => {}
                    }
                }
            }
            8 => {
                let id = ids[g.below(ids.len() as u32) as usize];
                w32(&mut v, (id << 8) | 3);
            }
            _ => {}
        }
    }
    v
}

fn gen_chunk(g: &mut Gn, ids: &[u32], depth: u32) -> Vec<u8> {
    let id = if g.below(8) == 0 { g.below(20) } else { ids[g.below(ids.len() as u32) as usize] };
    let body = gen_ops(g, ids, depth);
    let size = if g.chance(8) {
        match g.below(5) {
            0 => 0,
            1 => -(1 + g.below(20) as i32),
            2 => body.len() as i32 + 1 + g.below(30) as i32,
            3 => (body.len() as i32 - 1 - g.below(10) as i32).max(0),
            _ => 0x7fff_fff0,
        }
    } else {
        body.len() as i32
    };
    let mut v = Vec::new();
    w32(&mut v, id);
    w16(&mut v, g.g.u32() as u16);
    w32(&mut v, size as u32);
    v.extend(body);
    v
}

fn gen_file(g: &mut Gn) -> Vec<u8> {
    let (mut v, ids) = gen_toc(g);
    for _ in 0..1 + g.below(4) {
        v.extend(gen_chunk(g, &ids, 0));
    }
    if g.chance(5) {
        v.truncate(g.below(v.len() as u32) as usize);
    }
    if !v.is_empty() && g.chance(5) {
        for _ in 0..1 + g.below(3) {
            let p = g.below(v.len() as u32) as usize;
            v[p] = g.g.u32() as u8;
        }
    }
    if v.is_empty() {
        v.push(b'C');
    }
    v
}

#[test]
fn data_chunk_matches_upstream() {
    let mut g = Gn { g: Gen::new(0xDC), clean: false };
    let mut check = Check::new("DataChunkInput (TOC, parsers' log, parse, throw)");
    let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
    let (mut parsed, mut thrown, mut log_bytes, mut eof_toc) = (0u64, 0u64, 0u64, 0u64);
    const CASES: usize = 20_000;
    for case in 0..CASES {
        g.clean = case % 2 == 0;
        let data = gen_file(&mut g);
        let rust = match rust_run(&data) {
            Ok((_, _, _, n)) if n > 100_000 => {
                eof_toc += 1;
                continue;
            }
            Ok((s, c, l, _)) => (s, c, l),
            Err(ChunkError::Undefined(u)) => {
                *excluded.entry(format!("{u:?}")).or_default() += 1;
                continue;
            }
            Err(e) => unreachable!("{e:?}"),
        };
        if rust.0 == 3 {
            thrown += 1;
        }
        if rust.2.windows(2).any(|w| w == [b'R', 1]) {
            parsed += 1;
        }
        log_bytes += rust.2.len() as u64;
        let c = cpp_run(&data);
        check.eq(&rust, &c, || format!("case {case}: {:02x?}", data));
    }
    println!(
        "DataChunk: {CASES} files, {parsed} with a successful parse, {thrown} threw, {log_bytes} log bytes; \
         excluded (undefined upstream): {} {excluded:?}; excluded (table count > 10^5 past the end of the file, \
         upstream allocates each entry): {eof_toc}",
        excluded.values().sum::<u64>()
    );
    check.finish();
}
