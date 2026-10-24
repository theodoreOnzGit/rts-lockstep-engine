// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::map_object`'s chunk reader against
//! upstream `ParseObjectDataChunk` (`GameClient/MapUtil.cpp`, commit
//! 0a05454d), cut out of that file verbatim by build.rs and compiled with
//! `differential/wrap_map_object.cpp`. The objects' constructor is the
//! non-NULL branch of `MapObject::MapObject` (its angle normalised, its
//! property sheet copied), the template lookup answers NULL (no template
//! exists in this build), and the reader underneath is upstream's
//! `DataChunk.cpp` and `NameKeyGenerator.cpp` (the chunk build).
//!
//! **Methodology.** Each case is a synthetic chunk file: a table of contents
//! naming `Object` and a few property keys (sometimes with a wrong tag), then
//! one to four top-level `Object` chunks of version 0 to 4, each with a
//! position (any bits: NaN and infinities are copied, not computed), an angle
//! (finite, or infinite sometimes), flags, a name (sometimes empty, with a NUL,
//! or repeated), and for version 2 and up a property dictionary: up to five
//! pairs, mostly with keys from the table and types 0 to 4, sometimes an
//! unknown type (`ERROR_CORRUPT_FILE_FORMAT`) or more than 32767 pairs
//! (`ERROR_OUT_OF_MEMORY`), and a third of the objects are waypoints
//! (`waypointID` an `Int`, `waypointName` a string, from a small set so that
//! names repeat). Half the cases are clean, half are damaged (truncated, bytes
//! overwritten). Compared, per case:
//!
//! - every object at its deletion (upstream's destructor order is the file's
//!   order): name, position and angle on their bits, flags, the property sheet
//!   pair by pair (sorted by key name: name, type and value, the reals on
//!   their bits), the waypoint flag after the classification, and the template
//!   name it was looked up by;
//! - the waypoint map (`m_waypoints`, name order, the later waypoint wins);
//! - the result of `parse`: 1, 0, or a thrown `ErrorCode` (the objects read
//!   before a throw are still deleted, and logged, on both sides).
//!
//! Undefined upstream, and not sent to the C++: a read that runs past the end
//! of the file (the C++ reads uninitialised values); an angle that
//! `normalizeAngle` never reduces (upstream's constructor would hang); a table
//! of contents whose count runs past the end of the file by over 10^5 entries
//! (upstream allocates every one).
//!
//! **Planted bugs.** See the results below; each was put by hand into
//! `src/logic/map_object.rs` and removed after the run was seen to fail.
//!
//! # Results
//!
//! See the `DIFF` lines of `cargo test --release --features differential
//! --test differential_map_object -- --nocapture`, and the summary in
//! `docs/phase1-porting-graph.md`.

#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::dict::DataType;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::data_chunk::{CachedFileInputStream, ChunkError, DataChunkInput};
use rts_lockstep_engine::logic::map_object::{parse_object_data_chunk, ObjectParse};
use std::collections::BTreeMap;
use support::{Check, Gen};

/// Cases per run.
const CASES: usize = 6000;
/// Property-sheet keys the table of contents names (the others get ids
/// that name nothing, or name `""`).
const KEYS: &[&[u8]] = &[
    b"waypointID",
    b"waypointName",
    b"originalOwner",
    b"objectInitialHealth",
    b"objectEnabled",
    b"teamName",
    b"Player_1_Start",
    b"objectSelectable",
];
const NAMES: &[&[u8]] = &[b"Tree", b"Waypoint", b"Bridge", b"", b" ", b"x\x00y", b"Barracks", b"Tank"];
const WAYPOINT_NAMES: &[&[u8]] = &[b"Start", b"Way 2", b"Start"];

fn put_str(v: &mut Vec<u8>, s: &[u8]) {
    v.extend_from_slice(&(s.len() as u16).to_le_bytes());
    v.extend_from_slice(s);
}

/// The table of contents: `Object` is id 1, the keys follow.
fn toc(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut v = if !clean && g.below(12) == 0 { b"CkMq".to_vec() } else { b"CkMp".to_vec() };
    let mut entries: Vec<(&[u8], u32)> = vec![(b"Object", 1)];
    for (i, k) in KEYS.iter().enumerate() {
        entries.push((k, 2 + i as u32));
    }
    v.extend_from_slice(&(entries.len() as i32).to_le_bytes());
    for (name, id) in entries {
        v.push(name.len() as u8);
        v.extend_from_slice(name);
        v.extend_from_slice(&id.to_le_bytes());
    }
    v
}

fn key_id(g: &mut Gen) -> u32 {
    if g.below(12) == 0 { g.u32() >> 8 } else { 2 + g.below(KEYS.len() as u32) }
}

fn property_sheet(g: &mut Gen, body: &mut Vec<u8>, waypoint: bool) {
    let mut pairs: Vec<(u32, u32, Vec<u8>)> = Vec::new();
    if waypoint {
        pairs.push((2, 1, 7i32.to_le_bytes().to_vec())); // waypointID: Int
        let name = WAYPOINT_NAMES[g.below(WAYPOINT_NAMES.len() as u32) as usize];
        let mut s = Vec::new();
        put_str(&mut s, name);
        pairs.push((3, 3, s)); // waypointName: string
    }
    for _ in 0..g.below(4) {
        let id = key_id(g);
        let ty = if g.below(40) == 0 { 5 + g.below(4) } else { g.below(5) };
        let mut val = Vec::new();
        match ty {
            0 => val.push(g.u32() as u8),
            1 => val.extend_from_slice(&g.i32().to_le_bytes()),
            2 => val.extend_from_slice(&g.mixed().to_le_bytes()),
            3 => {
                let n = g.below(6) as usize;
                let s: Vec<u8> = (0..n).map(|_| if g.below(8) == 0 { 0 } else { b'a' + g.below(26) as u8 }).collect();
                put_str(&mut val, &s);
            }
            4 => {
                let n = g.below(4) as usize;
                val.extend_from_slice(&(n as u16).to_le_bytes());
                for _ in 0..n {
                    let u = if g.below(6) == 0 { 0 } else { 0x41 + g.below(26) as u16 };
                    val.extend_from_slice(&u.to_le_bytes());
                }
            }
            _ => {}
        }
        pairs.push((id, ty, val));
    }
    let count: u16 = if g.below(60) == 0 { 32768 } else { pairs.len() as u16 };
    body.extend_from_slice(&count.to_le_bytes());
    for (id, ty, val) in pairs {
        body.extend_from_slice(&((id << 8) | ty).to_le_bytes());
        body.extend_from_slice(&val);
    }
}

fn object(g: &mut Gen, version: u16, damaged: bool) -> Vec<u8> {
    let mut b = Vec::new();
    for _ in 0..3 {
        b.extend_from_slice(&g.mixed().to_bits().to_le_bytes());
    }
    let angle = if damaged && g.below(40) == 0 { f32::INFINITY } else { g.uniform(-30.0, 30.0) };
    b.extend_from_slice(&angle.to_bits().to_le_bytes());
    b.extend_from_slice(&g.i32().to_le_bytes());
    put_str(&mut b, NAMES[g.below(NAMES.len() as u32) as usize]);
    if version >= 2 {
        let waypoint = g.below(3) == 0;
        property_sheet(g, &mut b, waypoint);
    }
    b
}

/// The file: the table, then one to four `Object` chunks, then the damage.
fn gen_file(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut v = toc(g, clean);
    for _ in 0..1 + g.below(4) {
        let version = [1u16, 2, 3, 0, 4][g.below(5) as usize];
        let body = object(g, version, !clean);
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&version.to_le_bytes());
        v.extend_from_slice(&(body.len() as i32).to_le_bytes());
        v.extend_from_slice(&body);
    }
    if !clean {
        if g.below(6) == 0 && v.len() > 1 {
            let n = g.below(v.len() as u32) as usize;
            v.truncate(n.max(1));
        }
        if g.below(6) == 0 {
            for _ in 0..1 + g.below(3) {
                let p = g.below(v.len() as u32) as usize;
                v[p] = g.u32() as u8;
            }
        }
    }
    v
}

// ---------------------------------------------------------------------------
// The log both sides write, in the layout of wrap_map_object.cpp
// ---------------------------------------------------------------------------

fn put_cstr(v: &mut Vec<u8>, s: &[u8]) {
    v.extend_from_slice(&(s.len() as u32).to_le_bytes());
    v.extend_from_slice(s);
}

fn put_coord(v: &mut Vec<u8>, c: Coord3D) {
    for x in [c.x, c.y, c.z] {
        v.extend_from_slice(&x.to_bits().to_le_bytes());
    }
}

fn rust_log(ctx: &ObjectParse) -> Vec<u8> {
    let mut log = Vec::new();
    for obj in &ctx.objects {
        put_cstr(&mut log, obj.name());
        put_coord(&mut log, obj.location());
        log.extend_from_slice(&obj.angle().to_bits().to_le_bytes());
        log.extend_from_slice(&obj.flags().to_le_bytes());
        let d = obj.properties();
        let mut v: Vec<(Vec<u8>, i32)> =
            (0..d.get_pair_count()).map(|n| (ctx.names.key_to_name(d.get_nth_key(n)).to_vec(), n)).collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        log.extend_from_slice(&(v.len() as u32).to_le_bytes());
        for (name, n) in v {
            put_cstr(&mut log, &name);
            match d.get_nth_type(n) {
                DataType::Bool => log.extend_from_slice(&[0, u8::from(d.try_get_nth_bool(n).expect("in range"))]),
                DataType::Int => {
                    log.push(1);
                    log.extend_from_slice(&d.try_get_nth_int(n).expect("in range").to_le_bytes());
                }
                DataType::Real => {
                    log.push(2);
                    log.extend_from_slice(&d.try_get_nth_real(n).expect("in range").to_bits().to_le_bytes());
                }
                DataType::AsciiString => {
                    log.push(3);
                    put_cstr(&mut log, &d.try_get_nth_ascii_string(n).expect("in range"));
                }
                DataType::UnicodeString => {
                    log.push(4);
                    let s = d.try_get_nth_unicode_string(n).expect("in range");
                    log.extend_from_slice(&(s.len() as u32).to_le_bytes());
                    for u in s {
                        log.extend_from_slice(&u.to_le_bytes());
                    }
                }
                DataType::None => unreachable!("a pair always has a type"),
            }
        }
        log.push(u8::from(obj.is_waypoint()));
        put_cstr(&mut log, obj.thing_template().expect("looked up by name"));
    }
    log.extend_from_slice(&(ctx.waypoints.len() as u32).to_le_bytes());
    for (name, loc) in &ctx.waypoints {
        put_cstr(&mut log, name);
        put_coord(&mut log, *loc);
    }
    log
}

/// The C++ run: (status, thrown code, log), or `None` if the log does not fit.
fn cpp_run(data: &[u8]) -> (i32, u32, Vec<u8>) {
    let mut log = vec![0u8; 1 << 20];
    let mut log_len: i32 = 0;
    let mut code: u32 = 0;
    let status = unsafe {
        cpp::ref_map_object_run(data.as_ptr() as *const _, data.len() as i32, log.as_mut_ptr(), log.len() as i32, &mut log_len, &mut code)
    };
    assert!(status >= 0, "C++ log overflow");
    log.truncate(log_len as usize);
    (status, code, log)
}

#[derive(Default)]
struct Counts {
    parsed: u64,
    thrown: BTreeMap<u32, u64>,
    objects: u64,
    waypoints: u64,
    log_bytes: u64,
    excluded: BTreeMap<String, u64>,
}

#[test]
fn map_object_chunk_matches_upstream() {
    let mut g = Gen::new(0x0B7E);
    let mut check = Check::new("MapObject chunk reader (ParseObjectDataChunk)");
    let mut counts = Counts::default();
    for case in 0..CASES {
        let clean = case % 2 == 0;
        let data = gen_file(&mut g, clean);

        // The Rust reference. Undefined inputs are counted and not sent.
        let mut ctx = ObjectParse::new();
        let r: Result<(i32, u32), String> = (|| {
            let stream = CachedFileInputStream::new(data.clone()).ok_or("empty")?;
            let mut input = match DataChunkInput::new(stream) {
                Ok(i) => i,
                Err(ChunkError::Undefined(u)) => return Err(format!("{u:?}")),
                Err(e) => unreachable!("{e:?}"),
            };
            if input.contents().eof_mappings() > 100_000 {
                return Err("table of contents past the end of the file (over 10^5 entries)".to_string());
            }
            input.register_parser(b"Object", b"", parse_object_data_chunk);
            match input.parse(&mut ctx) {
                Ok(ok) => match ctx.undefined {
                    None => Ok((i32::from(ok), 0)),
                    Some(_) => Err("an angle that normalizeAngle never reduces".to_string()),
                },
                Err(ChunkError::Thrown(c)) => Ok((3, c)),
                Err(ChunkError::Undefined(u)) => Err(format!("{u:?}")),
            }
        })();
        let (rust_status, rust_code) = match r {
            Ok(x) => x,
            Err(kind) => {
                // a clean file is well formed and has a finite angle: it is never excluded
                assert!(!clean, "case {case}: clean file undefined for the Rust reader: {kind}");
                *counts.excluded.entry(kind).or_default() += 1;
                continue;
            }
        };
        // (status, code) as the C++ reports them: 1, 0, or 3 with the thrown ErrorCode
        let (cs, cc, log) = cpp_run(&data);
        let cpp_pair = (cs, if cs == 3 { cc } else { 0 });
        check.eq((rust_status, rust_code), cpp_pair, || format!("case {case} (clean {clean}): status/code of {:02x?}", data));
        let rust = rust_log(&ctx);
        counts.log_bytes += rust.len() as u64;
        check.eq(rust, log, || format!("case {case} (clean {clean}): {:02x?}", data));
        if rust_status == 3 {
            *counts.thrown.entry(rust_code).or_default() += 1;
        }
        if rust_status == 1 {
            counts.parsed += 1;
        }
        counts.objects += ctx.objects.len() as u64;
        counts.waypoints += ctx.waypoints.len() as u64;
    }
    println!(
        "MapObject: {CASES} files, {} with a successful parse, {} objects, {} waypoints, {} log bytes; \
         thrown (ErrorCode -> files): {:?}; excluded (undefined upstream, not sent): {:?}",
        counts.parsed, counts.objects, counts.waypoints, counts.log_bytes, counts.thrown, counts.excluded
    );
    check.finish();
}
