// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `common::geometry` (`GeometryInfo`) against
//! upstream `Geometry.cpp` (commit 0a05454d), compiled unchanged in
//! `differential/wrap_geometry.cpp` (see build.rs). Phase 1 layer 4.
//!
//! # Methodology
//!
//! Four comparisons, each on seeded random inputs from `support::Gen`
//! (ordinary magnitudes, signs, zeros, subnormals, infinities and NaN):
//!
//! 1. **INI.** Random `GEO` blocks (the Geometry fields in random order, with
//!    valid and broken values, wrong case, unknown fields, comments, missing
//!    `End`) are parsed by the Rust `Ini::load` with `GEOMETRY_FIELDS`, and by
//!    the upstream field parsers through the upstream INI reader. Compared:
//!    whether parsing failed, and the whole geometry (type, isSmall, stored
//!    height, both radii, both bounding radii) bit for bit, including the
//!    partly written object after a failure. The exception *text* is not
//!    compared: the harness calls `initFromINI` directly, while the Rust goes
//!    through `INI::load`, which wraps every error with the file and line.
//!    Inputs where the Rust reports `Undefined` behaviour upstream (`inf`,
//!    `nan`, hexadecimal or overflowing reals) are not sent to the C++; they
//!    are counted per kind.
//! 2. **Methods.** Each non-random method (footprint area, heights, centre,
//!    pitches, expand, 2D bounds, clipping, footprint tests, line
//!    intersection, the setters) on random shapes and arguments.
//! 3. **Random offsets.** `makeRandomOffsetWithinFootprint` and
//!    `makeRandomOffsetOnPerimeter` from the same logic seed, several calls in
//!    a row (the generator state carries over). Shapes with a negative radius
//!    are not sent: upstream's rejection loop never ends for a disc.
//! 4. **Xfer.** `XferSave` bytes against the file upstream writes, `XferLoad`
//!    of the same bytes (and of the upstream file) on both sides, a corrupt
//!    shape type (upstream stores it without complaint; Rust reports
//!    `InvalidParameters`, so it is counted, not compared), and the
//!    `XferDeepCRC`.
//!
//! **Planted bugs.** Each comparison is run a second time against a
//! deliberately wrong variant of the Rust (a plausible slip), which must be
//! caught (`Check::finish_planted`).
//!
//! # Results
//!
//! Recorded by running `cargo test --release --features differential --test
//! differential_geometry -- --nocapture` (the `DIFF` and `PLANTED` lines).
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType, GEOMETRY_FIELDS};
use rts_lockstep_engine::common::ini::{
    BlockParse, FieldParse, Ini, IniLoadType, IniResult, UserData,
};
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferDeepCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use std::collections::BTreeMap;
use std::ffi::CString;
use support::{Check, Gen};

/// The state the comparisons check: type, isSmall, stored height, major
/// radius, minor radius, bounding circle and sphere radii, as bit patterns.
type Bits = Vec<u32>;

/// The bit pattern of a float, with every NaN replaced by one canonical NaN.
/// The sign and payload of a NaN are not compared, as `support::Check::f32`
/// does: the NaN's sign comes from which operation produced it (x86 SSE's
/// default NaN is negative; the Rust `f64` library path gives a positive
/// one), and every NaN is a non-number either way. Non-NaN bits are exact.
fn canon(f: f32) -> u32 {
    if f.is_nan() {
        0x7FC0_0000
    } else {
        f.to_bits()
    }
}

fn state(g: &GeometryInfo) -> Bits {
    vec![
        g.get_geom_type() as i32 as u32,
        u32::from(g.get_is_small()),
        canon(g.get_height()),
        canon(g.get_major_radius()),
        canon(g.get_minor_radius()),
        canon(g.get_bounding_circle_radius()),
        canon(g.get_bounding_sphere_radius()),
    ]
}

/// The C++ state words (`fieldsOf` in wrap_geometry.cpp), the same layout,
/// with the NaNs canonicalised as [`state`] does.
fn cpp_state(words: &[u32]) -> Bits {
    let mut v = words[..7].to_vec();
    for w in &mut v[2..] {
        let f = f32::from_bits(*w);
        *w = canon(f);
    }
    v
}

fn bits(v: &[f32]) -> Bits {
    v.iter().map(|&x| canon(x)).collect()
}

fn shape_of(t: i32, small: i32, h: f32, maj: f32, min: f32) -> GeometryInfo {
    GeometryInfo::new(
        GeometryType::from_index(t).expect("type 0..=2"),
        small != 0,
        h,
        maj,
        min,
    )
}

fn gen_real(g: &mut Gen) -> f32 {
    if g.below(10) < 8 {
        g.mixed()
    } else {
        g.finite()
    }
}

fn gen_shape(g: &mut Gen) -> (i32, i32, f32, f32, f32) {
    (
        g.below(3) as i32,
        g.below(2) as i32,
        gen_real(g),
        gen_real(g),
        gen_real(g),
    )
}

// ---------------------------------------------------------------------------
// 1. INI
// ---------------------------------------------------------------------------

/// The Rust `GEO` block handler: the Geometry fields into the context.
fn parse_geo_block(ini: &mut Ini<'_>, g: &mut GeometryInfo) -> IniResult<()> {
    ini.init_from_ini(g, &GEOMETRY_FIELDS)
}

const BLOCKS: [BlockParse<GeometryInfo>; 1] = [BlockParse {
    token: "GEO",
    parse: parse_geo_block,
}];

/// Planted slip: `GeometryMinorRadius` stores into the major radius.
fn planted_minor(ini: &mut Ini<'_>, g: &mut GeometryInfo, _: &UserData) -> IniResult<()> {
    let v = ini.parse_real()?;
    g.set_major_radius(v);
    Ok(())
}

static PLANTED_FIELDS: [FieldParse<GeometryInfo>; 6] = [
    FieldParse::new(
        "Geometry",
        rts_lockstep_engine::common::geometry::parse_geometry_type,
        UserData::None,
    ),
    FieldParse::new(
        "GeometryMajorRadius",
        rts_lockstep_engine::common::geometry::parse_geometry_major_radius,
        UserData::None,
    ),
    FieldParse::new("GeometryMinorRadius", planted_minor, UserData::None),
    FieldParse::new(
        "GeometryHeight",
        rts_lockstep_engine::common::geometry::parse_geometry_height,
        UserData::None,
    ),
    FieldParse::new(
        "GeometryIsSmall",
        rts_lockstep_engine::common::geometry::parse_geometry_is_small,
        UserData::None,
    ),
    FieldParse::end(),
];

fn planted_geo_block(ini: &mut Ini<'_>, g: &mut GeometryInfo) -> IniResult<()> {
    ini.init_from_ini(g, &PLANTED_FIELDS)
}

const PLANTED_BLOCKS: [BlockParse<GeometryInfo>; 1] = [BlockParse {
    token: "GEO",
    parse: planted_geo_block,
}];

const TYPE_TOKENS: &[&str] = &[
    "SPHERE", "CYLINDER", "BOX", "sphere", "Box", "cylinder", "BOXES", "NONE", "1",
];
const BOOL_TOKENS: &[&str] = &["Yes", "No", "TRUE", "FALSE", "1", "0", "maybe", "y", "yes"];
const ODD_REAL_TOKENS: &[&str] = &[
    "12abc", "1e", "1.5.3", "-0", "+3", ".5", "5.", "1e39", "inf", "nan", "0x1p3", "  7", "-1e-45",
    "3.4e38",
];
const FIELD_NAMES: &[&str] = &[
    "Geometry",
    "GeometryMajorRadius",
    "GeometryMinorRadius",
    "GeometryHeight",
    "GeometryIsSmall",
];
const WRONG_NAMES: &[&str] = &[
    "geometry",
    "GeometryRadius",
    "GeometryIsSmal",
    "GeometryHeightX",
    "Foo",
];

fn pick<'a>(g: &mut Gen, xs: &[&'a str]) -> &'a str {
    xs[g.below(xs.len() as u32) as usize]
}

fn real_token(g: &mut Gen) -> String {
    match g.below(10) {
        0..=4 => format!("{}", g.finite()),
        5 => format!("{:e}", g.geom()),
        6 => pick(g, ODD_REAL_TOKENS).to_string(),
        7 => format!("{:.2}", g.uniform(-3.0, 3.0)),
        _ => format!("{}", g.geom()),
    }
}

/// A `GEO` block with random field lines (valid, broken, misspelled, and
/// sometimes without `End`).
fn gen_geo_text(g: &mut Gen) -> String {
    let mut s = String::from("GEO\n");
    let lines = g.below(8);
    for _ in 0..lines {
        let name = if g.below(10) < 8 {
            pick(g, FIELD_NAMES)
        } else {
            pick(g, WRONG_NAMES)
        };
        let value = match name {
            "Geometry" => {
                if g.below(10) < 8 {
                    pick(g, TYPE_TOKENS).to_string()
                } else {
                    real_token(g)
                }
            }
            "GeometryIsSmall" => pick(g, BOOL_TOKENS).to_string(),
            _ => real_token(g),
        };
        let sep = pick(g, &[" = ", " ", "=", "\t= ", "  =  "]);
        let trailer = pick(g, &["", " ; comment", "\r", "   ", " 99"]);
        s.push_str(&format!("  {name}{sep}{value}{trailer}\n"));
        if g.below(20) == 0 {
            s.push_str("\n; a comment line\n");
        }
    }
    if g.below(10) < 9 {
        s.push_str("End\n");
    }
    s
}

#[test]
fn geometry_ini_matches() {
    let _lock = cpp::GLOBAL_RNG_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    const CASES: u32 = 20_000;
    let mut g = Gen::new(0x9E0_0001);
    let mut ok = Check::new("GeometryInfo INI parse (fields)");
    let mut bug = Check::new("GeometryInfo INI parse (planted)");
    let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
    let (mut failed, mut parsed) = (0u64, 0u64);
    for case in 0..CASES {
        let text = gen_geo_text(&mut g);
        let (t0, s0, h0, m0, n0) = gen_shape(&mut g);
        let data = text.as_bytes();
        let input =
            || format!("case {case}: shape ({t0}, {s0}, {h0:e}, {m0:e}, {n0:e}), block {text:?}");

        let mut geo = shape_of(t0, s0, h0, m0, n0);
        let rust = rts_lockstep_engine::common::ini::Ini::load(
            b"geo.ini",
            data,
            IniLoadType::Overwrite,
            None,
            &BLOCKS,
            &mut geo,
        );
        if let Err(e) = &rust {
            if e.is_undefined() {
                *excluded.entry(format!("{:?}", e.kind)).or_default() += 1;
                continue;
            }
        }
        let cpp = cpp_geometry_parse(b"geo.ini", data, t0, s0, h0, m0, n0);
        let rust_outcome = (rust.is_err(), state(&geo));
        if rust.is_err() {
            failed += 1;
        } else {
            parsed += 1;
        }
        ok.eq(rust_outcome.clone(), (cpp.0 != 0, cpp_state(&cpp.1)), input);

        // The planted variant must disagree on some input.
        let mut geo_bug = shape_of(t0, s0, h0, m0, n0);
        let planted = rts_lockstep_engine::common::ini::Ini::load(
            b"geo.ini",
            data,
            IniLoadType::Overwrite,
            None,
            &PLANTED_BLOCKS,
            &mut geo_bug,
        );
        bug.eq(
            (planted.is_err(), state(&geo_bug)),
            (cpp.0 != 0, cpp_state(&cpp.1)),
            input,
        );
    }
    println!(
        "INI: {CASES} blocks, {parsed} parsed, {failed} failed as upstream does; excluded (Undefined upstream, not sent to C++): {} {:?}",
        excluded.values().sum::<u64>(),
        excluded
    );
    ok.finish();
    bug.finish_planted();
}

/// Parses `data` with the upstream field table: (status, state words).
fn cpp_geometry_parse(
    file: &[u8],
    data: &[u8],
    t0: i32,
    s0: i32,
    h0: f32,
    m0: f32,
    n0: f32,
) -> (i32, Vec<u32>) {
    let cname = CString::new(file.to_vec()).expect("no NUL in name");
    let mut out = [0u32; 8];
    let mut msg = vec![0u8; 4096];
    let mut msg_len = 0i32;
    let rc = unsafe {
        cpp::ref_geometry_parse(
            cname.as_ptr(),
            data.as_ptr() as *const _,
            data.len() as i32,
            t0,
            s0,
            h0,
            m0,
            n0,
            out.as_mut_ptr(),
            msg.as_mut_ptr() as *mut _,
            msg.len() as i32,
            &mut msg_len,
        )
    };
    assert!(
        rc != -1,
        "the C++ layout check failed: the wrapper reads the wrong field"
    );
    (out[0] as i32, out[1..].to_vec())
}

// ---------------------------------------------------------------------------
// 2. Methods
// ---------------------------------------------------------------------------

fn pt(a: &[f32]) -> Coord3D {
    Coord3D {
        x: a[0],
        y: a[1],
        z: a[2],
    }
}

/// The Rust side of `ref_geometry_op`, with the same argument layout.
fn rust_op(op: i32, g: &mut GeometryInfo, a: &[f32]) -> Vec<f32> {
    match op {
        0 => vec![g.get_footprint_area()],
        1 => vec![g.get_max_height_above_position()],
        2 => vec![g.get_max_height_below_position()],
        3 => vec![g.get_z_delta_to_center_position()],
        4 => {
            let c = g.get_center_position(&pt(&a[0..3]));
            vec![c.x, c.y, c.z]
        }
        5 => {
            let that = shape_of(a[3] as i32, i32::from(a[4] != 0.0), a[5], a[6], a[7]);
            let (mn, mx) = g.calc_pitches(&pt(&a[0..3]), &that, &pt(&a[8..11]));
            vec![mn, mx]
        }
        6 => {
            g.expand_footprint(a[0]);
            state(g).iter().map(|&b| f32::from_bits(b)).collect()
        }
        7 => {
            let b = g.get_2d_bounds(&pt(&a[0..3]), a[3]);
            vec![b.lo.x, b.lo.y, b.hi.x, b.hi.y]
        }
        8 => {
            let mut p = pt(&a[3..6]);
            g.clip_point_to_footprint(&pt(&a[0..3]), &mut p);
            vec![p.x, p.y, p.z]
        }
        9 => vec![f32::from(u8::from(
            g.is_point_in_footprint(&pt(&a[0..3]), &pt(&a[3..6])),
        ))],
        10 => vec![f32::from(u8::from(g.is_intersected_by_line_segment(
            &pt(&a[0..3]),
            &pt(&a[3..6]),
            &pt(&a[6..9]),
        )))],
        11 => {
            g.set_max_height_above_position(a[0]);
            state(g).iter().map(|&b| f32::from_bits(b)).collect()
        }
        12 => {
            g.set_major_radius(a[0]);
            state(g).iter().map(|&b| f32::from_bits(b)).collect()
        }
        13 => {
            g.set_minor_radius(a[0]);
            state(g).iter().map(|&b| f32::from_bits(b)).collect()
        }
        _ => {
            g.set(
                GeometryType::from_index(a[0] as i32).expect("type"),
                a[1] != 0.0,
                a[2],
                a[3],
                a[4],
            );
            state(g).iter().map(|&b| f32::from_bits(b)).collect()
        }
    }
}

/// The C++ side of `ref_geometry_op`: the outputs as bit patterns (NaNs canonical).
fn cpp_op(op: i32, t0: i32, s0: i32, h0: f32, m0: f32, n0: f32, a: &[f32; 16]) -> Vec<u32> {
    let mut out = [0f32; 8];
    let mut count = 0i32;
    let rc = unsafe {
        cpp::ref_geometry_op(
            op,
            t0,
            s0,
            h0,
            m0,
            n0,
            a.as_ptr(),
            out.as_mut_ptr(),
            &mut count,
        )
    };
    assert_eq!(rc, 0, "ref_geometry_op {op}");
    out[..count as usize].iter().map(|&x| canon(x)).collect()
}

fn gen_args(g: &mut Gen) -> [f32; 16] {
    let mut a = [0f32; 16];
    for v in a.iter_mut() {
        *v = gen_real(g);
    }
    // the type and isSmall slots take small integers
    a[3] = g.below(3) as f32;
    a[4] = g.below(2) as f32;
    a
}

#[test]
fn geometry_methods_match() {
    const CASES: u32 = 20_000;
    let mut g = Gen::new(0x9E0_0002);
    let mut ok = Check::new("GeometryInfo methods (all ops)");
    let mut bug = Check::new("GeometryInfo methods (planted)");
    for case in 0..CASES {
        let op = (case % 15) as i32;
        let (t0, s0, h0, m0, n0) = gen_shape(&mut g);
        let mut a = gen_args(&mut g);
        // op 14 takes a type in slot 0 and an isSmall in slot 1
        if op == 14 {
            a[0] = g.below(3) as f32;
            a[1] = g.below(2) as f32;
        }
        let input = || {
            format!("case {case} op {op}: shape ({t0}, {s0}, {h0:e}, {m0:e}, {n0:e}), args {a:?}")
        };
        let mut geo = shape_of(t0, s0, h0, m0, n0);
        let rust = bits(&rust_op(op, &mut geo, &a));
        let cpp_bits = cpp_op(op, t0, s0, h0, m0, n0, &a);
        ok.eq(rust.clone(), cpp_bits.clone(), input);

        // Planted: a plausible slip in each op's formula (box footprint
        // area with 2 instead of 4, the pitch's bottom taken from the top
        // height, the centre lift applied to x instead of z, ...).
        let mut geo_b = shape_of(t0, s0, h0, m0, n0);
        let planted: Vec<f32> = match op {
            0 => vec![2.0 * rust_op(0, &mut geo_b, &a)[0]],
            5 => {
                let mut r = rust_op(5, &mut geo_b, &a);
                r[0] = r[1];
                r
            }
            4 => {
                let mut r = rust_op(4, &mut geo_b, &a);
                r.swap(0, 2);
                r
            }
            _ => rust_op(op, &mut geo_b, &a),
        };
        bug.eq(bits(&planted), cpp_bits, input);
    }
    ok.finish();
    bug.finish_planted();
}

// ---------------------------------------------------------------------------
// 3. Random offsets
// ---------------------------------------------------------------------------

fn cpp_random(op: i32, seed: u32, shape: (i32, i32, f32, f32, f32), count: usize) -> Vec<f32> {
    let (t0, s0, h0, m0, n0) = shape;
    let mut out = vec![0f32; 3 * count];
    let rc = unsafe {
        cpp::ref_geometry_random(op, seed, t0, s0, h0, m0, n0, count as i32, out.as_mut_ptr())
    };
    assert_eq!(rc, 0);
    out
}

fn rust_random(
    op: i32,
    seed: u32,
    shape: (i32, i32, f32, f32, f32),
    count: usize,
    planted: bool,
) -> Vec<f32> {
    let geo = shape_of(shape.0, shape.1, shape.2, shape.3, shape.4);
    let mut rng = RandomValues::default();
    rng.init_game_logic_random(seed);
    let mut out = Vec::with_capacity(3 * count);
    for _ in 0..count {
        let mut p = Coord3D {
            x: 0.0,
            y: 0.0,
            z: if op == 1 { 7.0 } else { 0.0 },
        };
        if op == 0 {
            geo.make_random_offset_within_footprint(&mut rng, &mut p);
            if planted {
                // a slip: the disc is sampled inside the bounding square's
                // corner quadrant only
                p.x = p.x.abs();
            }
        } else {
            geo.make_random_offset_on_perimeter(&mut rng, &mut p);
        }
        out.extend([p.x, p.y, p.z]);
    }
    out
}

#[test]
fn geometry_random_matches() {
    let _lock = cpp::GLOBAL_RNG_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    const SEEDS: u32 = 2_000;
    const COUNT: usize = 8;
    let mut g = Gen::new(0x9E0_0003);
    let mut ok = Check::new("GeometryInfo random offsets");
    let mut bug = Check::new("GeometryInfo random offsets (planted)");
    let (mut excluded, mut sent) = (0u64, 0u64);
    for case in 0..SEEDS {
        let op = (case % 2) as i32;
        let (t0, s0, h0, m0, n0) = gen_shape(&mut g);
        // a disc with a negative radius never accepts a point (upstream loops forever)
        let disc = t0 != 2;
        if op == 0 && disc && m0 < 0.0 {
            excluded += 1;
            continue;
        }
        let seed = g.u32();
        let input = || format!("seed {seed}: op {op}, shape ({t0}, {s0}, {h0:e}, {m0:e}, {n0:e})");
        let shape = (t0, s0, h0, m0, n0);
        let rust = bits(&rust_random(op, seed, shape, COUNT, false));
        let cpp_out = bits(&cpp_random(op, seed, shape, COUNT));
        ok.eq(rust, cpp_out.clone(), input);
        sent += 1;
        bug.eq(
            bits(&rust_random(op, seed, shape, COUNT, true)),
            cpp_out,
            input,
        );
    }
    println!("random: {sent} seeds sent; excluded (negative disc radius, upstream never returns): {excluded}");
    ok.finish();
    bug.finish_planted();
}

// ---------------------------------------------------------------------------
// 4. Xfer
// ---------------------------------------------------------------------------

/// A saved geometry with every NaN in its five float words replaced by the
/// canonical NaN (see [`canon`]). Layout: version (1 byte), type (4), isSmall
/// (1), then the five reals (4 bytes each, from offset 6).
fn canon_saved(bytes: &[u8]) -> Vec<u8> {
    let mut v = bytes.to_vec();
    for off in (6..26).step_by(4) {
        if let Some(w) = v.get(off..off + 4) {
            let f = f32::from_le_bytes([w[0], w[1], w[2], w[3]]);
            if f.is_nan() {
                v[off..off + 4].copy_from_slice(&0x7FC0_0000u32.to_le_bytes());
            }
        }
    }
    v
}

fn has_nan(t0: i32, s0: i32, h0: f32, m0: f32, n0: f32) -> bool {
    let g = shape_of(t0, s0, h0, m0, n0);
    [
        g.get_height(),
        g.get_major_radius(),
        g.get_minor_radius(),
        g.get_bounding_circle_radius(),
        g.get_bounding_sphere_radius(),
    ]
    .iter()
    .any(|f| f.is_nan())
}

fn temp_path(tag: &str, n: u32) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "p1w1-geometry-{}-{tag}-{n}.bin",
        std::process::id()
    ))
}

#[test]
fn geometry_xfer_matches() {
    let _lock = cpp::GLOBAL_RNG_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    const CASES: u32 = 2_000;
    let mut g = Gen::new(0x9E0_0004);
    let mut ok = Check::new("GeometryInfo xfer (save, load, deep CRC)");
    let mut bug = Check::new("GeometryInfo xfer (planted)");
    let mut corrupt = 0u64;
    let mut nan_crc_skipped = 0u64;
    for case in 0..CASES {
        let (t0, s0, h0, m0, n0) = gen_shape(&mut g);
        let input = || format!("case {case}: shape ({t0}, {s0}, {h0:e}, {m0:e}, {n0:e})");

        // Save: upstream's file against the Rust bytes.
        let mut geo = shape_of(t0, s0, h0, m0, n0);
        let mut save = XferSave::new();
        save.open(b"geo").expect("open");
        save.xfer_snapshot(&mut geo).expect("save");
        save.close().expect("close");
        let rust_bytes = save.take_bytes();

        let path = temp_path("save", case);
        let cpath = CString::new(path.to_string_lossy().as_bytes()).expect("path");
        let st = unsafe { cpp::ref_geometry_save(cpath.as_ptr(), t0, s0, h0, m0, n0) };
        assert_eq!(st, 0, "upstream XferSave threw");
        let cpp_bytes = std::fs::read(&path).expect("upstream wrote the file");
        let _ = std::fs::remove_file(&path);
        ok.eq(canon_saved(&rust_bytes), canon_saved(&cpp_bytes), input);

        // Load: the Rust bytes read by both sides.
        let path2 = temp_path("load", case);
        std::fs::write(&path2, &rust_bytes).expect("write");
        let cpath2 = CString::new(path2.to_string_lossy().as_bytes()).expect("path");
        let mut cout = [0u32; 8];
        let st = unsafe {
            cpp::ref_geometry_load(cpath2.as_ptr(), t0, s0, h0, m0, n0, cout.as_mut_ptr())
        };
        let _ = std::fs::remove_file(&path2);
        assert_eq!(st, 0, "upstream XferLoad threw");
        let mut back = shape_of(t0, s0, h0, m0, n0);
        let mut load = XferLoad::new(rust_bytes.clone());
        load.open(b"geo").expect("open");
        load.xfer_snapshot(&mut back).expect("load");
        load.close().expect("close");
        ok.eq(state(&back), cpp_state(&cout[1..]), input);
        ok.eq(state(&back), state(&geo), input);

        // Deep CRC: the CRC of the xfer bytes.
        let mut crc = XferDeepCrc::new();
        crc.open(b"deep").expect("open");
        crc.xfer_snapshot(&mut shape_of(t0, s0, h0, m0, n0))
            .expect("crc");
        crc.close().expect("close");
        // The CRC covers the raw bytes, so a NaN's sign (see `canon`) changes
        // it: such shapes are counted, not compared.
        if has_nan(t0, s0, h0, m0, n0) {
            nan_crc_skipped += 1;
        } else {
            let path = temp_path("crc", case);
            let cpath = CString::new(path.to_string_lossy().as_bytes()).expect("path");
            let cpp_crc = unsafe { cpp::ref_geometry_deep_crc(cpath.as_ptr(), t0, s0, h0, m0, n0) };
            let _ = std::fs::remove_file(&path);
            ok.eq(crc.get_crc(), cpp_crc, input);
        }

        // Planted: the load drops the stored height (a plausible slip in
        // the field order), which the comparison must see.
        let mut planted = state(&back);
        planted[2] = state(&shape_of(t0, s0, h0, m0, n0))[2].wrapping_add(1);
        bug.eq(planted, cpp_state(&cout[1..]), input);
    }
    // A type word outside the enum: upstream loads it; Rust reports an error.
    for case in 0..50u32 {
        let (t0, s0, h0, m0, n0) = gen_shape(&mut g);
        let mut geo = shape_of(t0, s0, h0, m0, n0);
        let mut save = XferSave::new();
        save.open(b"geo").expect("open");
        save.xfer_snapshot(&mut geo).expect("save");
        save.close().expect("close");
        let mut bytes = save.take_bytes();
        bytes[1..5].copy_from_slice(&(7 + case as i32).to_le_bytes());
        let mut back = shape_of(t0, s0, h0, m0, n0);
        let mut load = XferLoad::new(bytes);
        load.open(b"geo").expect("open");
        assert!(
            load.xfer_snapshot(&mut back).is_err(),
            "an out-of-range type must be an error in Rust"
        );
        corrupt += 1;
    }
    println!(
        "xfer: {CASES} shapes; deep CRC not compared for {nan_crc_skipped} shapes with a NaN field (sign only); {corrupt} corrupt type words (upstream stores them; Rust reports InvalidParameters, not compared)"
    );
    ok.finish();
    bug.finish_planted();
}
