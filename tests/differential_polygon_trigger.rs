// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::polygon_trigger` against upstream
//! `PolygonTrigger.cpp` (commit 0a05454d), compiled unchanged (see
//! `differential/wrap_polygon_trigger.cpp`; its one edit, a declaration moved
//! before a loop, is made in a build.rs copy, as for INI.cpp). The chunk
//! reader under it is upstream's `DataChunk.cpp` (the chunk build), and the
//! writer's output is upstream's `WritePolygonTriggersDataChunk` against a
//! stand-in `DataChunkOutput` that writes upstream's byte layout (the real
//! one goes through a temporary file in the user data directory).
//!
//! **Methodology.** Each case is a synthetic `PolygonTriggers` chunk file: a
//! table of contents (sometimes with a wrong tag, so the table is not read),
//! then one chunk of version 0 to 5 (version 1 gets the auto-added water area)
//! holding a count (sometimes wrong) and triggers with random names and layer
//! names, ids (small, duplicated, or any `Int`), water and river bytes (any
//! byte: non-zero is true), river starts, point counts (negative, zero, one,
//! many, or huge so the read runs past the end), and point coordinates (mostly
//! small, sometimes any `Int`). Half the cases are clean, half are damaged
//! (truncated, bytes overwritten). Both sides run the same bytes with the same
//! water extents. Compared, per case:
//!
//! - the parse's result and the whole chain, in order (names, layer names,
//!   ids, flags, river start, points, validity, export flag, water-handle
//!   flag), `s_currentID` after it, and the bytes of
//!   `WritePolygonTriggersDataChunk` (the chunk id and version, the size and
//!   the body);
//! - for every trigger and 12 to 23 query points (engine vertices, random
//!   points inside and outside the box, any `Int` pairs): `pointInTrigger`,
//!   the centre's x and y, and `getRadius`, the reals on their bits.
//!
//! Undefined upstream, and not sent to the C++ (which would read garbage or
//! overflow an `Int`): a chunk read that runs past the end of the file or of
//! the table of contents; a version-1 id at `Int` maximum; a query or a
//! trigger whose `Int` arithmetic overflows (the Rust returns
//! `PolygonUndefined::IntOverflow` and the answer is counted, not compared).
//!
//! **Planted bugs.** The comparisons were run against deliberately wrong
//! translations (see the results in the doc comment below), each edit put by
//! hand into `src/logic/polygon_trigger.rs`, observed to fail, and removed.
//!
//! # Results
//!
//! See the `DIFF` lines of `cargo test --release --features differential
//! --test differential_polygon_trigger -- --nocapture`, and the summary in
//! `docs/phase1-porting-graph.md`.

#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::base_type::ICoord3D;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::data_chunk::{CachedFileInputStream, ChunkError, DataChunkInput};
use rts_lockstep_engine::logic::polygon_trigger::{
    parse_polygon_triggers_data_chunk, write_polygon_triggers_chunk, PolygonTriggerList, PolygonUndefined, TriggerParse,
};
use std::collections::BTreeMap;
use support::{Check, Gen};

/// The chunk id the writer is given (what upstream's `allocateID` would answer).
const WRITE_ID: u32 = 7;
/// Capacity of the C++ result arrays, in triggers.
const TRIG_CAP: usize = 512;
/// Cases per run.
const CASES: usize = 6000;

// ---------------------------------------------------------------------------
// The generator
// ---------------------------------------------------------------------------

fn put_str(v: &mut Vec<u8>, s: &[u8]) {
    v.extend_from_slice(&(s.len() as u16).to_le_bytes());
    v.extend_from_slice(s);
}

const NAMES: &[&[u8]] = &[b"", b"Lake", b"Road", b"Zone 1", b"x\x00y", b"AutoAddedWaterAreaTrigger"];
const LAYERS: &[&[u8]] = &[b"", b"Water", b"Default", b"Layer2"];

fn gen_body(g: &mut Gen, version: u16, clean: bool) -> Vec<u8> {
    let mut v = Vec::new();
    let ntrig = g.below(6);
    let count = if !clean && g.below(10) == 0 { g.below(8) as i32 } else { ntrig as i32 };
    v.extend_from_slice(&count.to_le_bytes());
    for _ in 0..ntrig {
        put_str(&mut v, NAMES[g.below(NAMES.len() as u32) as usize]);
        if version >= 4 {
            put_str(&mut v, LAYERS[g.below(LAYERS.len() as u32) as usize]);
        }
        let id = if g.below(8) == 0 { g.i32() } else { 1 + g.below(12) as i32 };
        v.extend_from_slice(&id.to_le_bytes());
        if version >= 2 {
            v.push(g.u32() as u8);
        }
        if version >= 3 {
            v.push(g.u32() as u8);
            let start = g.below(5) as i32 - 1;
            v.extend_from_slice(&start.to_le_bytes());
        }
        let num_points: i32 = match g.below(40) {
            0 => -(g.below(3) as i32),
            1 => g.below(2) as i32,
            2 if !clean => 40_000, // runs past the end: only in damaged files
            _ => 2 + g.below(6) as i32,
        };
        v.extend_from_slice(&num_points.to_le_bytes());
        for _ in 0..num_points.clamp(0, 12) {
            for _ in 0..3 {
                let c = if g.below(20) == 0 { g.i32() } else { g.below(401) as i32 - 200 };
                v.extend_from_slice(&c.to_le_bytes());
            }
        }
    }
    v
}

/// The chunk file: a table of contents (`CkMp`, or `CkMq` for a file whose
/// table is not read), then the `PolygonTriggers` chunk and sometimes a
/// chunk nobody registered.
fn gen_file(g: &mut Gen, clean: bool) -> (Vec<u8>, f32, f32) {
    let mut v = if !clean && g.below(12) == 0 { b"CkMq".to_vec() } else { b"CkMp".to_vec() };
    v.extend_from_slice(&2i32.to_le_bytes());
    for (name, id) in [(&b"PolygonTriggers"[..], 1u32), (b"Other", 2)] {
        v.push(name.len() as u8);
        v.extend_from_slice(name);
        v.extend_from_slice(&id.to_le_bytes());
    }
    let version = [1u16, 2, 3, 4, 4, 4, 0, 5][g.below(8) as usize];
    let body = gen_body(g, version, clean);
    v.extend_from_slice(&1u32.to_le_bytes());
    v.extend_from_slice(&version.to_le_bytes());
    v.extend_from_slice(&(body.len() as i32).to_le_bytes());
    v.extend_from_slice(&body);
    if g.below(3) == 0 {
        v.extend_from_slice(&2u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&4i32.to_le_bytes());
        v.extend_from_slice(&[1, 2, 3, 4]);
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
    // the water extents of TheGlobalData: modest, or the special values
    let wx = if g.below(4) == 0 { g.special() } else { g.uniform(-1.0e4, 1.0e4) };
    let wy = if g.below(4) == 0 { g.special() } else { g.uniform(-1.0e4, 1.0e4) };
    (v, wx, wy)
}

fn queries(g: &mut Gen, list: &PolygonTriggerList) -> Vec<(i32, i32)> {
    let verts: Vec<ICoord3D> = list.iter().flat_map(|t| (0..t.num_points()).filter_map(|i| t.get_point(i))).collect();
    let nq = 12 + g.below(12) as usize;
    (0..nq)
        .map(|_| match g.below(6) {
            0 | 1 if !verts.is_empty() => {
                let p = verts[g.below(verts.len() as u32) as usize];
                (p.x, p.y)
            }
            2 => (g.i32(), g.i32()), // may overflow: undefined, counted
            _ => (g.below(461) as i32 - 230, g.below(461) as i32 - 230),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The log both sides write, in the layout of wrap_polygon_trigger.cpp
// ---------------------------------------------------------------------------

fn put_i32(v: &mut Vec<u8>, x: i32) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn put_cstr(v: &mut Vec<u8>, s: &[u8]) {
    v.extend_from_slice(&(s.len() as u32).to_le_bytes());
    v.extend_from_slice(s);
}

fn rust_log(list: &PolygonTriggerList, parse_ok: bool) -> Vec<u8> {
    let mut log = vec![u8::from(parse_ok)];
    for t in list.iter() {
        put_cstr(&mut log, t.trigger_name());
        put_cstr(&mut log, t.layer_name());
        put_i32(&mut log, t.id());
        log.push(u8::from(t.is_water_area()));
        log.push(u8::from(t.is_river()));
        put_i32(&mut log, t.river_start());
        put_i32(&mut log, t.num_points());
        for i in 0..t.num_points() {
            let p = t.get_point(i).expect("a point");
            put_i32(&mut log, p.x);
            put_i32(&mut log, p.y);
            put_i32(&mut log, p.z);
        }
        log.push(u8::from(t.is_valid()));
        log.push(u8::from(t.export_with_scripts()));
        log.push(u8::from(t.water_handle().is_some()));
    }
    log.push(b'E');
    put_i32(&mut log, list.next_id());
    let mut out = Vec::new();
    write_polygon_triggers_chunk(list, WRITE_ID, &mut out);
    log.extend_from_slice(&(out.len() as u32).to_le_bytes());
    log.extend_from_slice(&out);
    log
}

/// Run the C++ on one file: (status, log, per-trigger inside answers,
/// centres, radii, number of triggers), or `None` if the log or the trigger
/// count does not fit.
struct CppRun {
    log: Vec<u8>,
    nq: usize,
    inside: Vec<u8>,
    centers: Vec<u32>,
    radii: Vec<u32>,
    nt: usize,
}

fn cpp_run(data: &[u8], wx: f32, wy: f32, q: &[(i32, i32)]) -> Option<CppRun> {
    let flat: Vec<i32> = q.iter().flat_map(|&(x, y)| [x, y]).collect();
    let mut log = vec![0u8; 1 << 20];
    let mut log_len: i32 = 0;
    let mut inside = vec![0u8; TRIG_CAP * q.len().max(1)];
    let mut centers = vec![0u32; 2 * TRIG_CAP];
    let mut radii = vec![0u32; TRIG_CAP];
    let mut nt: i32 = 0;
    let n = unsafe {
        cpp::ref_polygon_run(
            data.as_ptr() as *const _,
            data.len() as i32,
            wx,
            wy,
            WRITE_ID,
            flat.as_ptr(),
            q.len() as i32,
            log.as_mut_ptr(),
            log.len() as i32,
            &mut log_len,
            inside.as_mut_ptr(),
            centers.as_mut_ptr(),
            radii.as_mut_ptr(),
            TRIG_CAP as i32,
            &mut nt,
        )
    };
    assert!(n >= 0, "C++ log overflow");
    if nt as usize > TRIG_CAP {
        return None;
    }
    log.truncate(log_len as usize);
    Some(CppRun { log, nq: q.len(), inside, centers, radii, nt: nt as usize })
}

// ---------------------------------------------------------------------------
// The comparison
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Counts {
    parsed: u64,
    triggers: u64,
    parse_undefined: BTreeMap<String, u64>,
    queries_compared: u64,
    queries_undefined: u64,
    trig_cap_skipped: u64,
    log_bytes: u64,
}

#[test]
fn polygon_trigger_matches_upstream() {
    let mut g = Gen::new(0x9017);
    let mut check = Check::new("PolygonTrigger (parse, chain, write, queries)");
    let mut counts = Counts::default();
    for case in 0..CASES {
        let clean = case % 2 == 0;
        let (data, wx, wy) = gen_file(&mut g, clean);

        // The Rust reference: parse, then the undefined inputs are counted and not sent.
        let parsed = (|| -> Result<(bool, TriggerParse), String> {
            let stream = CachedFileInputStream::new(data.clone()).ok_or("empty")?;
            let mut input = match DataChunkInput::new(stream) {
                Ok(i) => i,
                Err(ChunkError::Undefined(u)) => return Err(format!("{u:?}")),
                Err(e) => unreachable!("{e:?}"),
            };
            // a table of contents whose count runs past the end: upstream allocates each entry
            if input.contents().eof_mappings() > 100_000 {
                return Err("table of contents past the end of the file (over 10^5 entries)".to_string());
            }
            input.register_parser(b"PolygonTriggers", b"", parse_polygon_triggers_data_chunk);
            // an extent whose version-1 conversion is out of range: refused up front
            let mut ctx = TriggerParse::new(wx, wy).map_err(|u| format!("water extent: {u:?}"))?;
            match input.parse(&mut ctx) {
                Ok(ok) => match ctx.undefined {
                    None => Ok((ok, ctx)),
                    Some(u) => Err(format!("{u:?}")),
                },
                Err(ChunkError::Undefined(u)) => Err(format!("{u:?}")),
                Err(e) => unreachable!("{e:?}"),
            }
        })();
        let (ok, ctx) = match parsed {
            Ok(x) => x,
            Err(kind) => {
                // A clean file is well formed: the only undefined input it can have is a
                // water extent (its own input). Anything else means the reader went wrong.
                assert!(!clean || kind.starts_with("water extent"), "case {case}: clean file undefined for the Rust reader: {kind}");
                *counts.parse_undefined.entry(kind).or_default() += 1;
                continue;
            }
        };
        if ok {
            counts.parsed += 1;
        }
        counts.triggers += ctx.list.len() as u64;

        let q = queries(&mut g, &ctx.list);
        let Some(c) = cpp_run(&data, wx, wy, &q) else {
            counts.trig_cap_skipped += 1;
            continue;
        };
        let rust = rust_log(&ctx.list, ok);
        counts.log_bytes += rust.len() as u64;
        check.eq(rust, c.log.clone(), || format!("case {case} (clean {clean}, wx {wx}, wy {wy}): {:02x?}", data));

        for (k, t) in ctx.list.iter().enumerate().take(c.nt) {
            for (j, &(x, y)) in q.iter().enumerate() {
                match t.point_in_trigger(ICoord3D { x, y, z: 0 }) {
                    Ok(b) => {
                        counts.queries_compared += 1;
                        check.eq(u8::from(b), c.inside[k * c.nq + j], || format!("case {case}: trigger {k} point ({x}, {y})"));
                    }
                    Err(PolygonUndefined::IntOverflow) => counts.queries_undefined += 1,
                    Err(e) => unreachable!("{e:?}"),
                }
            }
            // an Int overflow in the box, the centre or the radius is undefined upstream: counted
            match (t.center_point(), t.radius()) {
                (Ok((cx, cy)), Ok(r)) => {
                    check.f32(cx, f32::from_bits(c.centers[2 * k]), || format!("case {case}: trigger {k} centre x"));
                    check.f32(cy, f32::from_bits(c.centers[2 * k + 1]), || format!("case {case}: trigger {k} centre y"));
                    check.f32(r, f32::from_bits(c.radii[k]), || format!("case {case}: trigger {k} radius"));
                }
                _ => counts.queries_undefined += 1,
            }
        }
    }
    println!(
        "PolygonTrigger: {CASES} files, {} with a successful parse, {} triggers; \
         queries compared {} (undefined, counted: {}); cases with more than {TRIG_CAP} triggers: {}; \
         log bytes {}; excluded (undefined upstream, not sent): {:?}",
        counts.parsed,
        counts.triggers,
        counts.queries_compared,
        counts.queries_undefined,
        counts.trig_cap_skipped,
        counts.log_bytes,
        counts.parse_undefined
    );
    check.finish();
}
