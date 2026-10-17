// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `common::ini` against upstream `INI.cpp`
//! (commit 0a05454d), compiled by g++ from a copy with two mechanical edits
//! for MSVC's `for` scope and `const` binding (see `build.rs`), with
//! `GameCommon.cpp` unchanged (see `differential/wrap_ini.cpp`).
//!
//! No game INI files are available, so every file is **synthetic**: blocks,
//! field lines and `End` lines built from the format as the upstream reader
//! reads it, with every field parser's value syntax (numbers at and beyond
//! their range, partial numbers such as `12abc` and `1e`, signs, names in
//! any case, `+`/`-` flag lists, colours and coordinates with missing or
//! wrong tags, quoted strings in all their broken forms), nested blocks,
//! unknown and handler-less fields, unknown blocks, missing `End`, comments,
//! tabs, CRs, NUL and high bytes, over-long lines (> 1028 bytes), no final
//! newline, and in a share of cases random byte mutations of the whole file.
//!
//! The block table of `INI.cpp` is fixed, so five of its block types have
//! test handlers on both sides (`Object`: name, one table; `Armor`:
//! `getNextAsciiString` name, `initFromINIMulti` with a catch-all table
//! second; `Weapon`: `getNextQuotedAsciiString` name, catch-all table first
//! via `initFromINIMultiProc`; `Credits`: the rest of its line; `Science`:
//! no name), and the other 57 throw `ERROR_BUG`. Each field parse appends
//! the stored value's bytes to a log. Compared per file: what `INI::load`
//! throws (nothing, an `INIException` and its whole message, an `INI_*`
//! status code, an `ErrorCode`, `std::out_of_range`), the log byte for byte,
//! and the `XferCRC` of the lines read (the multiplayer INI CRC).
//! `INI::loadDirectory` is compared the same way on generated listings, and
//! the static scanners (`scanInt`, `scanUnsignedInt`, `scanReal`,
//! `scanPercentToReal`, `scanBool`, `scanIndexList`, `scanLookupList`,
//! `isValidINIFilename`, `isEndOfBlock`, `isDeclarationOfType`) on random
//! tokens.
//!
//! Inputs for which the Rust reports [`Undefined`] behaviour upstream (see
//! the module docs of `common::ini`) are **not** given to the C++; their
//! count is printed per kind.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::base_type::{Coord2D, Coord3D, ICoord2D, RGBAColorInt, RGBColor};
use rts_lockstep_engine::common::ini::{
    BlockParse, FieldParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, LookupListRec,
    MultiIniFieldParse, Thrown, UserData, TYPE_TABLE,
};
use rts_lockstep_engine::common::random_value::{DistributionType, GameClientRandomVariable};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::big::FilenameList;
use rts_lockstep_engine::logic::damage::{DamageTypeFlags, DAMAGE_TYPE_NAMES, DEATH_NAMES};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::CString;
use support::{Check, Gen};

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

thread_local! {
    static LOG: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    /// How often each field parser (by log id) succeeded in the Rust.
    static SEEN: RefCell<[u64; 64]> = const { RefCell::new([0; 64]) };
    /// Whether the generator draws only well-formed values.
    static CLEAN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// A successful field parse: its id into the log, and the tally.
fn tag(id: u8) {
    put8(id);
    SEEN.with(|s| s.borrow_mut()[id as usize] += 1);
}

fn put(b: &[u8]) {
    LOG.with(|l| l.borrow_mut().extend_from_slice(b));
}
fn put8(v: u8) {
    put(&[v]);
}
fn put_str(s: &[u8]) {
    put(&(s.len() as u32).to_le_bytes());
    put(s);
}
fn put_vec(v: &[Vec<u8>]) {
    put(&(v.len() as u32).to_le_bytes());
    for s in v {
        put_str(s);
    }
}
fn put_f32s(v: &[f32]) {
    for x in v {
        put(&x.to_le_bytes());
    }
}

/// The C++ test record (`Rec` in wrap_ini.cpp); the plain fields start as
/// 0x5A bytes.
struct Rec {
    u8_: u8,
    i16_: i16,
    u16_: u16,
    i32_: i32,
    u32_: u32,
    r: f32,
    bit: u32,
    bits32: u32,
    s: Vec<u8>,
    sv: Vec<Vec<u8>>,
}

impl Rec {
    fn new() -> Self {
        Rec {
            u8_: 0x5A,
            i16_: 0x5A5A,
            u16_: 0x5A5A,
            i32_: 0x5A5A_5A5A,
            u32_: 0x5A5A_5A5A,
            r: f32::from_bits(0x5A5A_5A5A),
            bit: 0x5A5A_5A5A,
            bits32: 0x5A5A_5A5A,
            s: Vec::new(),
            sv: Vec::new(),
        }
    }
}

static TEST_NAMES: [&str; 40] = [
    "N00", "N01", "N02", "N03", "N04", "N05", "N06", "N07", "N08", "N09", "N10", "N11", "N12", "N13",
    "N14", "N15", "N16", "N17", "N18", "N19", "N20", "N21", "N22", "N23", "N24", "N25", "N26", "N27",
    "N28", "N29", "N30", "N31", "N32", "N33", "N34", "N35", "N36", "N37", "N38", "N39",
];
static TEST_LOOKUP: [LookupListRec; 4] = [
    LookupListRec { name: "Low", value: 1 },
    LookupListRec { name: "Mid", value: 5 },
    LookupListRec { name: "High", value: -7 },
    LookupListRec { name: "Max", value: 0x7fff_ffff },
];

fn rgb(c: RGBColor) -> [f32; 3] {
    [c.red, c.green, c.blue]
}
fn rgba(c: RGBAColorInt) -> Vec<u8> {
    [c.red, c.green, c.blue, c.alpha].iter().flat_map(|v| v.to_le_bytes()).collect()
}
fn c3(c: Coord3D) -> [f32; 3] {
    [c.x, c.y, c.z]
}
fn c2(c: Coord2D) -> [f32; 2] {
    [c.x, c.y]
}
fn ic2(c: ICoord2D) -> Vec<u8> {
    [c.x, c.y].iter().flat_map(|v| v.to_le_bytes()).collect()
}
fn dmg(f: DamageTypeFlags) -> [u8; 8] {
    f.0.to_le_bytes()
}
fn rv(v: &GameClientRandomVariable) -> Vec<u8> {
    let mut b = v.low.to_le_bytes().to_vec();
    b.extend_from_slice(&v.high.to_le_bytes());
    b.extend_from_slice(&(v.distribution as i32).to_le_bytes());
    b
}

/// `TheFieldTable` of wrap_ini.cpp: each entry runs the translated parser,
/// stores like the C++ entry, and logs `id` and the stored bytes.
static FIELD_TABLE: [FieldParse<Rec>; 40] = [
    FieldParse::new("Byte", |ini, r, _| { r.u8_ = ini.parse_unsigned_byte()?; tag(1); put(&[r.u8_]); Ok(()) }, UserData::None),
    FieldParse::new("Short", |ini, r, _| { r.i16_ = ini.parse_short()?; tag(2); put(&r.i16_.to_le_bytes()); Ok(()) }, UserData::None),
    FieldParse::new("UShort", |ini, r, _| { r.u16_ = ini.parse_unsigned_short()?; tag(3); put(&r.u16_.to_le_bytes()); Ok(()) }, UserData::None),
    FieldParse::new("Int", |ini, r, _| { r.i32_ = ini.parse_int()?; tag(4); put(&r.i32_.to_le_bytes()); Ok(()) }, UserData::None),
    FieldParse::new("UInt", |ini, r, _| { r.u32_ = ini.parse_unsigned_int()?; tag(5); put(&r.u32_.to_le_bytes()); Ok(()) }, UserData::None),
    FieldParse::new("Real", |ini, r, _| { r.r = ini.parse_real()?; tag(6); put_f32s(&[r.r]); Ok(()) }, UserData::None),
    FieldParse::new("PosReal", |ini, r, _| { r.r = ini.parse_positive_non_zero_real()?; tag(7); put_f32s(&[r.r]); Ok(()) }, UserData::None),
    FieldParse::new("Bool", |ini, _, _| { let b = ini.parse_bool()?; tag(8); put8(u8::from(b)); Ok(()) }, UserData::None),
    FieldParse::new("Bit", |ini, r, ud| { ini.parse_bit_in_int32(&mut r.bit, ud.mask())?; tag(9); put(&r.bit.to_le_bytes()); Ok(()) }, UserData::Mask(0x0001_0010)),
    FieldParse::new("Str", |ini, r, _| { r.s = ini.parse_ascii_string()?; tag(10); put_str(&r.s); Ok(()) }, UserData::None),
    FieldParse::new("QStr", |ini, r, _| { r.s = ini.parse_quoted_ascii_string()?; tag(11); put_str(&r.s); Ok(()) }, UserData::None),
    FieldParse::new("StrVec", |ini, r, _| { ini.parse_ascii_string_vector(&mut r.sv)?; tag(12); put_vec(&r.sv); Ok(()) }, UserData::None),
    FieldParse::new("StrVecAdd", |ini, r, _| { ini.parse_ascii_string_vector_append(&mut r.sv)?; tag(13); put_vec(&r.sv); Ok(()) }, UserData::None),
    FieldParse::new("Percent", |ini, r, _| { r.r = ini.parse_percent_to_real()?; tag(14); put_f32s(&[r.r]); Ok(()) }, UserData::None),
    FieldParse::new("Bits8", |ini, _, ud| { let v = ini.parse_bit_string8(ud.names())?; tag(15); put8(v); Ok(()) }, UserData::Names(&TEST_NAMES)),
    FieldParse::new("Bits32", |ini, r, ud| { ini.parse_bit_string32(&mut r.bits32, ud.names())?; tag(16); put(&r.bits32.to_le_bytes()); Ok(()) }, UserData::Names(&TEST_NAMES)),
    FieldParse::new("RGB", |ini, _, _| { let v = ini.parse_rgb_color()?; tag(17); put_f32s(&rgb(v)); Ok(()) }, UserData::None),
    FieldParse::new("RGBA", |ini, _, _| { let v = ini.parse_rgba_color_int()?; tag(18); put(&rgba(v)); Ok(()) }, UserData::None),
    FieldParse::new("Color", |ini, _, _| { let v = ini.parse_color_int()?; tag(19); put(&v.to_le_bytes()); Ok(()) }, UserData::None),
    FieldParse::new("Coord3D", |ini, _, _| { let v = ini.parse_coord3d()?; tag(20); put_f32s(&c3(v)); Ok(()) }, UserData::None),
    FieldParse::new("Coord2D", |ini, _, _| { let v = ini.parse_coord2d()?; tag(21); put_f32s(&c2(v)); Ok(()) }, UserData::None),
    FieldParse::new("ICoord2D", |ini, _, _| { let v = ini.parse_icoord2d()?; tag(22); put(&ic2(v)); Ok(()) }, UserData::None),
    FieldParse::new("Index", |ini, _, ud| { let v = ini.parse_index_list(ud.names())?; tag(23); put(&v.to_le_bytes()); Ok(()) }, UserData::Names(&TEST_NAMES)),
    FieldParse::new("ByteIndex", |ini, _, ud| { let v = ini.parse_byte_sized_index_list(ud.names())?; tag(24); put8(v); Ok(()) }, UserData::Names(&TEST_NAMES)),
    FieldParse::new("Lookup", |ini, _, ud| { let v = ini.parse_lookup_list(ud.lookup())?; tag(25); put(&v.to_le_bytes()); Ok(()) }, UserData::Lookup(&TEST_LOOKUP)),
    FieldParse::new("Angle", |ini, r, _| { r.r = ini.parse_angle_real()?; tag(26); put_f32s(&[r.r]); Ok(()) }, UserData::None),
    FieldParse::new("AngVel", |ini, r, _| { r.r = ini.parse_angular_velocity_real()?; tag(27); put_f32s(&[r.r]); Ok(()) }, UserData::None),
    FieldParse::new("DurReal", |ini, r, _| { r.r = ini.parse_duration_real()?; tag(28); put_f32s(&[r.r]); Ok(()) }, UserData::None),
    FieldParse::new("DurUInt", |ini, r, _| { r.u32_ = ini.parse_duration_unsigned_int()?; tag(29); put(&r.u32_.to_le_bytes()); Ok(()) }, UserData::None),
    FieldParse::new("DurUShort", |ini, _, _| { let v = ini.parse_duration_unsigned_short()?; tag(30); put(&v.to_le_bytes()); Ok(()) }, UserData::None),
    FieldParse::new("Vel", |ini, r, _| { r.r = ini.parse_velocity_real()?; tag(31); put_f32s(&[r.r]); Ok(()) }, UserData::None),
    FieldParse::new("Accel", |ini, r, _| { r.r = ini.parse_acceleration_real()?; tag(32); put_f32s(&[r.r]); Ok(()) }, UserData::None),
    FieldParse::new("Vet", |ini, _, _| { let v = ini.parse_veterancy_level_flags()?; tag(33); put(&v.to_le_bytes()); Ok(()) }, UserData::None),
    FieldParse::new("Death", |ini, _, _| { let v = ini.parse_death_type_flags()?; tag(34); put(&v.to_le_bytes()); Ok(()) }, UserData::None),
    FieldParse::new("Damage", |ini, _, _| { let v = ini.parse_damage_type_flags()?; tag(35); put(&dmg(v)); Ok(()) }, UserData::None),
    FieldParse::new("Sounds", |ini, r, _| { ini.parse_sounds_list(&mut r.sv)?; tag(36); put_vec(&r.sv); Ok(()) }, UserData::None),
    FieldParse::new("RandVar", |ini, _, _| {
        let mut v = GameClientRandomVariable::new(0.0, 0.0, DistributionType::Constant);
        ini.parse_game_client_random_variable(&mut v)?;
        tag(37);
        put(&rv(&v));
        Ok(())
    }, UserData::None),
    FieldParse::new("Nested", |ini, r, _| { tag(38); ini.init_from_ini(r, &FIELD_TABLE)?; tag(39); Ok(()) }, UserData::None),
    FieldParse::without_parse("Dead"),
    FieldParse::end(),
];

/// `TheCatchTable` of wrap_ini.cpp.
static CATCH_TABLE: [FieldParse<Rec>; 2] = [
    FieldParse::new("Extra", |ini, r, _| { r.i32_ = ini.parse_int()?; tag(40); put(&r.i32_.to_le_bytes()); Ok(()) }, UserData::None),
    FieldParse::catch_all(|ini, _, ud| {
        put8(41);
        if let UserData::Token(t) = ud {
            put_str(t);
        }
        while let Some(t) = ini.get_next_token_or_null(None) {
            put8(42);
            put_str(&t);
        }
        put8(43);
        Ok(())
    }),
];

fn h_object(ini: &mut Ini<'_>, _: &mut ()) -> IniResult<()> {
    put(&[0xF0, 1]);
    put_str(&ini.get_next_token(None)?);
    ini.init_from_ini(&mut Rec::new(), &FIELD_TABLE)?;
    put8(0xF1);
    Ok(())
}
fn h_armor(ini: &mut Ini<'_>, _: &mut ()) -> IniResult<()> {
    put(&[0xF0, 2]);
    put_str(&ini.get_next_ascii_string());
    let mut p = MultiIniFieldParse::new();
    p.add(&FIELD_TABLE)?;
    p.add(&CATCH_TABLE)?;
    ini.init_from_ini_multi(&mut Rec::new(), &p)?;
    put8(0xF1);
    Ok(())
}
fn h_weapon(ini: &mut Ini<'_>, _: &mut ()) -> IniResult<()> {
    put(&[0xF0, 3]);
    put_str(&ini.get_next_quoted_ascii_string()?);
    ini.init_from_ini_multi_proc(&mut Rec::new(), |p| {
        p.add(&CATCH_TABLE)?;
        p.add(&FIELD_TABLE)
    })?;
    put8(0xF1);
    Ok(())
}
fn h_credits(ini: &mut Ini<'_>, _: &mut ()) -> IniResult<()> {
    put(&[0xF0, 4]);
    while let Some(t) = ini.get_next_token_or_null(None) {
        put8(42);
        put_str(&t);
    }
    put8(0xF1);
    Ok(())
}
fn h_science(ini: &mut Ini<'_>, _: &mut ()) -> IniResult<()> {
    put(&[0xF0, 5]);
    ini.init_from_ini(&mut Rec::new(), &FIELD_TABLE)?;
    put8(0xF1);
    Ok(())
}
fn h_stub(_: &mut Ini<'_>, _: &mut ()) -> IniResult<()> {
    Err(IniError::new(IniErrorKind::Bug))
}

fn blocks() -> Vec<BlockParse<()>> {
    TYPE_TABLE
        .iter()
        .map(|&(token, _, _)| BlockParse {
            token,
            parse: match token {
                "Object" => h_object,
                "Armor" => h_armor,
                "Weapon" => h_weapon,
                "Credits" => h_credits,
                "Science" => h_science,
                _ => h_stub,
            },
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The generator
// ---------------------------------------------------------------------------

fn pick<'a, T: ?Sized>(g: &mut Gen, xs: &[&'a T]) -> &'a T {
    xs[g.below(xs.len() as u32) as usize]
}

fn case_mix(g: &mut Gen, s: &str) -> String {
    match g.below(6) {
        0 => s.to_ascii_lowercase(),
        1 => s.to_ascii_uppercase(),
        _ => s.to_string(),
    }
}

fn gen_int(g: &mut Gen) -> String {
    match g.below(20) {
        0..=5 => format!("{}", g.i32() % 1000),
        6 => format!("{}", g.i32()),
        7 => pick(g, &["2147483647", "-2147483648", "2147483648", "-2147483649", "4294967295", "4294967296", "-4294967295", "99999999999999999999", "0", "-0", "+0"]).to_string(),
        8 => format!("{}abc", g.below(300)),
        9 => pick(g, &["-", "+", "x", "", "0x10", "+-1", "1.5", "--2", "255", "256", "-1", "65535", "65536", "32767", "-32768", "32768"]).to_string(),
        10 => format!("{}", g.u32()),
        11 => format!("-{}", g.u32()),
        12..=15 => format!("{}", g.below(256)),
        _ => format!("{}", g.below(70000) as i32 - 35000),
    }
}

fn gen_real(g: &mut Gen) -> String {
    match g.below(24) {
        0..=6 => format!("{}", g.uniform(-1000.0, 1000.0)),
        7 => format!("{:e}", g.finite()),
        8 => format!("{}", g.finite()),
        9 => pick(g, &["1e", "1e+", "1.", ".", "-.5", ".e1", "1e5x", "+.25", "1.5.3", "00012", "1,5", "5e-", ".5e", "1d", "1E2", "-0", "0", "-0.0"]).to_string(),
        10 => pick(g, &["inf", "-INF", "nan", "NaN(1)", "Infantry", "0x10", "0X1p3", "-0x", "infinity"]).to_string(),
        11 => pick(g, &["1e39", "3.4028236e38", "3.4028235e38", "-3.5e38", "1e-50", "1e-45", "1.4e-45", "7e-46", "1e999999999", "1e-999999999"]).to_string(),
        12 => format!("{}%", g.below(200)),
        13 => format!("{}.{}", g.below(100), g.below(1_000_000)),
        14 => {
            // long digit strings (rounding at many digits)
            let n = 1 + g.below(60);
            let mut s: String = (0..n).map(|_| char::from(b'0' + g.below(10) as u8)).collect();
            if g.below(2) == 0 {
                s.insert(g.below(n) as usize, '.');
            }
            s
        }
        15 => gen_int(g),
        16 => pick(g, &["x", "", "-", "+", "e5", "abc"]).to_string(),
        _ => format!("{}", g.uniform(-10.0, 10.0)),
    }
}

fn gen_name(g: &mut Gen, names: &[&str]) -> String {
    match g.below(12) {
        0 => pick(g, &["Bogus", "", "N40", "N-1", "NO"]).to_string(),
        _ => {
            let n = names[g.below(names.len() as u32) as usize];
            case_mix(g, n)
        }
    }
}

fn gen_flag_list(g: &mut Gen, names: &[&str], plain: bool) -> String {
    let n = g.below(5);
    let mut out = Vec::new();
    let mode = g.below(4);
    for _ in 0..n {
        let t = match g.below(14) {
            0 => case_mix(g, "NONE"),
            1 => case_mix(g, "ALL"),
            _ => {
                let p = match mode {
                    0 if plain => "",
                    1 => "+",
                    2 => "-",
                    _ => pick(g, &["", "+", "-"]),
                };
                format!("{p}{}", gen_name(g, names))
            }
        };
        out.push(t);
    }
    out.join(" ")
}

fn gen_quoted(g: &mut Gen) -> String {
    let w = |g: &mut Gen| pick(g, &["Hello", "World", "a", "b c", "x=y", "", "q\"r", "Zero Hour"]).to_string();
    match g.below(12) {
        0 => format!("\"{}\"", w(g)),
        1 => format!("\"{} {}\"", w(g), w(g)),
        2 => format!("\"{}  {}\" tail", w(g), w(g)),
        3 => "\"".to_string(),
        4 => "\"\"".to_string(),
        5 => format!("\"{}", w(g)),
        6 => format!("{} {}", w(g), w(g)),
        7 => format!("\"{}\" {}", w(g), w(g)),
        8 => format!("\" {}\"", w(g)),
        9 => format!("\"{} \"x", w(g)),
        10 => format!("\"{}\"=\"{}\"", w(g), w(g)),
        _ => String::new(),
    }
}

fn gen_color(g: &mut Gen, alpha: bool) -> String {
    let tags = ["R", "G", "B", "A"];
    let n = if alpha { 3 + g.below(2) as usize } else { 3 };
    let mut parts = Vec::new();
    for t in tags.iter().take(n) {
        let tag = match g.below(25) {
            0 => "X".to_string(),
            1 => String::new(),
            _ => case_mix(g, t),
        };
        let v = match g.below(10) {
            0 => gen_int(g),
            _ => format!("{}", g.below(260)),
        };
        let sep = pick(g, &[":", ":", ": ", " : ", ":", "="]);
        parts.push(format!("{tag}{sep}{v}"));
    }
    if g.below(15) == 0 {
        parts.truncate(g.below(parts.len() as u32 + 1) as usize);
    }
    parts.join(" ")
}

fn gen_coord(g: &mut Gen, n: usize, int: bool) -> String {
    let tags = ["X", "Y", "Z"];
    let mut parts = Vec::new();
    for t in tags.iter().take(n) {
        let tag = if g.below(25) == 0 { "W".to_string() } else { case_mix(g, t) };
        let v = if int { gen_int(g) } else { gen_real(g) };
        parts.push(format!("{tag}:{v}"));
    }
    if g.below(15) == 0 {
        parts.truncate(g.below(parts.len() as u32 + 1) as usize);
    }
    parts.join(" ")
}

/// A well-formed value for the field (the "clean" files).
fn gen_clean(g: &mut Gen, field: &str) -> String {
    let real = |g: &mut Gen| format!("{}", g.uniform(-500.0, 500.0));
    let flags = |g: &mut Gen, names: &[&str], skip_first: bool| -> String {
        let lo = u32::from(skip_first);
        let n = names.len() as u32 - lo;
        let p = pick(g, &["+", "-"]);
        (0..1 + g.below(3)).map(|_| format!("{p}{}", names[(lo + g.below(n)) as usize])).collect::<Vec<_>>().join(" ")
    };
    match field {
        "Byte" => format!("{}", g.below(256)),
        "Short" => format!("{}", g.below(65536) as i32 - 32768),
        "UShort" => format!("{}", g.below(65536)),
        "Int" | "Extra" => format!("{}", g.i32()),
        "UInt" => format!("{}", g.u32()),
        "DurUInt" => format!("{}", g.below(1_000_000)),
        "DurUShort" => format!("{}", g.below(2_000_000)),
        "Real" | "Angle" | "AngVel" | "DurReal" | "Vel" | "Accel" => real(g),
        "PosReal" => format!("{}", g.uniform(0.001, 500.0)),
        "Percent" => format!("{}%", g.uniform(0.0, 100.0)),
        "Bool" | "Bit" => pick(g, &["Yes", "No"]).to_string(),
        "Str" | "QStr" => pick(g, &["Plain", "\"Two Words\"", "\"One\""]).to_string(),
        "Bits8" => (0..1 + g.below(3)).map(|_| TEST_NAMES[g.below(8) as usize]).collect::<Vec<_>>().join(" "),
        "Bits32" => (0..1 + g.below(3)).map(|_| TEST_NAMES[g.below(32) as usize]).collect::<Vec<_>>().join(" "),
        "Index" => TEST_NAMES[g.below(40) as usize].to_string(),
        "ByteIndex" => TEST_NAMES[g.below(40) as usize].to_string(),
        "Lookup" => pick(g, &["Low", "Mid", "High", "Max"]).to_string(),
        "RGB" => format!("R:{} G:{} B:{}", g.below(256), g.below(256), g.below(256)),
        "RGBA" | "Color" => format!("R:{} G:{} B:{} A:{}", g.below(256), g.below(256), g.below(256), g.below(256)),
        "Coord3D" => format!("X:{} Y:{} Z:{}", real(g), real(g), real(g)),
        "Coord2D" => format!("X:{} Y:{}", real(g), real(g)),
        "ICoord2D" => format!("X:{} Y:{}", g.i32() % 5000, g.i32() % 5000),
        "Vet" => flags(g, &["REGULAR", "VETERAN", "ELITE", "HEROIC"], true),
        "Death" => flags(g, &DEATH_NAMES, true),
        "Damage" => flags(g, &DAMAGE_TYPE_NAMES, false),
        "RandVar" => format!("{} {} {}", real(g), real(g), pick(g, &["UNIFORM", "CONSTANT", "GAUSSIAN"])),
        _ => gen_value_any(g, field),
    }
}

fn gen_value(g: &mut Gen, field: &str) -> String {
    if CLEAN.with(|c| c.get()) {
        gen_clean(g, field)
    } else {
        gen_value_any(g, field)
    }
}

fn gen_value_any(g: &mut Gen, field: &str) -> String {
    match field {
        "Byte" | "Short" | "UShort" | "Int" | "UInt" | "Extra" | "DurUInt" | "DurUShort" => gen_int(g),
        "Real" | "PosReal" | "Angle" | "AngVel" | "DurReal" | "Vel" | "Accel" => gen_real(g),
        "Percent" => {
            let r = gen_real(g);
            if g.below(3) == 0 { r } else { format!("{r}%") }
        }
        "Bool" | "Bit" => pick(g, &["Yes", "No", "yes", "NO", "yES", "maybe", "1", ""]).to_string(),
        "Str" | "QStr" => gen_quoted(g),
        "StrVec" | "StrVecAdd" | "Sounds" => {
            let n = g.below(5);
            (0..n).map(|_| pick(g, &["a", "Bee", "c,d", "e=f", "g\th", "\"q\""]).to_string()).collect::<Vec<_>>().join(pick(g, &[" ", ",", " , ", "="]))
        }
        "Bits8" | "Bits32" => gen_flag_list(g, &TEST_NAMES, true),
        "Index" | "ByteIndex" => gen_name(g, &TEST_NAMES),
        "Lookup" => gen_name(g, &["Low", "Mid", "High", "Max"]),
        "RGB" => gen_color(g, false),
        "RGBA" | "Color" => gen_color(g, true),
        "Coord3D" => gen_coord(g, 3, false),
        "Coord2D" => gen_coord(g, 2, false),
        "ICoord2D" => gen_coord(g, 2, true),
        "Vet" => gen_flag_list(g, &["REGULAR", "VETERAN", "ELITE", "HEROIC"], false),
        "Death" => gen_flag_list(g, &DEATH_NAMES, false),
        "Damage" => gen_flag_list(g, &DAMAGE_TYPE_NAMES, false),
        "RandVar" => {
            let mut s = format!("{} {}", gen_real(g), gen_real(g));
            if g.below(2) == 0 {
                s.push(' ');
                s.push_str(&gen_name(g, &["CONSTANT", "UNIFORM", "GAUSSIAN", "TRIANGULAR", "LOW_BIAS", "HIGH_BIAS"]));
            }
            if g.below(10) == 0 {
                s = gen_real(g);
            }
            s
        }
        _ => gen_int(g),
    }
}

const FIELDS: &[&str] = &[
    "Byte", "Short", "UShort", "Int", "UInt", "Real", "PosReal", "Bool", "Bit", "Str", "QStr", "StrVec",
    "StrVecAdd", "Percent", "Bits8", "Bits32", "RGB", "RGBA", "Color", "Coord3D", "Coord2D", "ICoord2D",
    "Index", "ByteIndex", "Lookup", "Angle", "AngVel", "DurReal", "DurUInt", "DurUShort", "Vel", "Accel",
    "Vet", "Death", "Damage", "Sounds", "RandVar",
];

fn gen_end(g: &mut Gen) -> String {
    pick(g, &["End", "END", "end", "  End  ", "End ; done", "End=", "Ends", "=End"]).to_string()
}

fn gen_field_line(g: &mut Gen, depth: u32, lines: &mut Vec<String>) {
    let indent = pick(g, &["", "  ", "\t", " \t "]);
    let r = g.below(40);
    if r == 0 && depth < 3 {
        lines.push(format!("{indent}Nested"));
        for _ in 0..g.below(4) {
            gen_field_line(g, depth + 1, lines);
        }
        if g.below(8) != 0 {
            lines.push(gen_end(g));
        }
        return;
    }
    let field = match r {
        1 => pick(g, &["Dead", "Extra", "Unknown", "int", "BYTE", "Nested2"]).to_string(),
        _ => pick(g, FIELDS).to_string(),
    };
    let eq = pick(g, &[" = ", "=", " ", " =", "= ", " == "]);
    let value = gen_value(g, &field);
    let comment = if g.below(8) == 0 { pick(g, &[" ; comment", ";", " ;x=1 2"]) } else { "" };
    lines.push(format!("{indent}{field}{eq}{value}{comment}"));
}

fn gen_block(g: &mut Gen, lines: &mut Vec<String>) {
    let kind = match g.below(20) {
        0..=6 => "Object".to_string(),
        7..=9 => "Armor".to_string(),
        10..=12 => "Weapon".to_string(),
        13 => "Credits".to_string(),
        14 => "Science".to_string(),
        15 => TYPE_TABLE[g.below(TYPE_TABLE.len() as u32) as usize].0.to_string(),
        16 => pick(g, &["object", "OBJECT", "Thing", "End", "Objec"]).to_string(),
        _ => "Object".to_string(),
    };
    let name = match g.below(8) {
        0 => String::new(),
        1 => gen_quoted(g),
        _ => format!("Name{}", g.below(100)),
    };
    lines.push(format!("{kind} {name}").trim_end().to_string());
    if kind == "Credits" {
        return;
    }
    for _ in 0..g.below(10) {
        gen_field_line(g, 0, lines);
    }
    if g.below(15) != 0 {
        lines.push(gen_end(g));
    }
}

/// A synthetic INI file.
fn gen_file(g: &mut Gen) -> Vec<u8> {
    let mut lines = Vec::new();
    for _ in 0..1 + g.below(5) {
        match g.below(10) {
            0 => lines.push(pick(g, &["; a comment", "", "   ", ";", "\t"]).to_string()),
            1 => lines.push(pick(g, &["Unknown block", "=", "  =  ", "End"]).to_string()),
            _ => gen_block(g, &mut lines),
        }
    }
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        out.extend_from_slice(l.as_bytes());
        let last = i + 1 == lines.len();
        if !(last && g.below(4) == 0) {
            out.extend_from_slice(if g.below(6) == 0 { b"\r\n" } else { b"\n" });
        }
    }
    // occasional byte-level damage: NUL and high bytes, an over-long line,
    // random flips, insertions and deletions
    match g.below(14) {
        0 if !out.is_empty() => {
            let p = g.below(out.len() as u32) as usize;
            out.insert(p, if g.below(2) == 0 { 0 } else { 0x80 + g.below(0x80) as u8 });
        }
        1 if !out.is_empty() => {
            let p = g.below(out.len() as u32) as usize;
            let n = 1000 + g.below(100) as usize;
            let filler: Vec<u8> = (0..n).map(|_| b"ab =1"[g.below(5) as usize]).collect();
            out.splice(p..p, filler);
        }
        2 if !out.is_empty() => {
            for _ in 0..1 + g.below(4) {
                let p = g.below(out.len() as u32) as usize;
                match g.below(3) {
                    0 => out[p] = g.u32() as u8,
                    1 => out.insert(p, g.u32() as u8),
                    _ => {
                        out.remove(p);
                        if out.is_empty() {
                            break;
                        }
                    }
                }
            }
        }
        _ => {}
    }
    out
}

// ---------------------------------------------------------------------------
// Running both sides
// ---------------------------------------------------------------------------

/// One side's outcome: (status as wrap_ini.cpp's CATCH_ALL numbers it,
/// message or code, log, CRC).
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    status: i32,
    detail: Vec<u8>,
    log: Vec<u8>,
    crc: u32,
}

fn rust_outcome(r: IniResult<()>, crc: u32) -> Result<Outcome, IniError> {
    let log = LOG.with(|l| std::mem::take(&mut *l.borrow_mut()));
    let (status, detail) = match r {
        Ok(()) => (0, Vec::new()),
        Err(e) => match e.thrown() {
            None => return Err(e),
            Some(Thrown::Exception(m)) => (1, m),
            Some(Thrown::Status(c)) => (2, c.to_le_bytes().to_vec()),
            Some(Thrown::ErrorCode(c)) => (3, c.to_le_bytes().to_vec()),
            Some(Thrown::OutOfRange) => (4, Vec::new()),
        },
    };
    Ok(Outcome { status, detail, log, crc })
}

type CppLoad = unsafe extern "C" fn(
    *const std::os::raw::c_char,
    i32,
    *mut u8,
    i32,
    *mut i32,
    *mut u32,
    *mut std::os::raw::c_char,
    i32,
    *mut u32,
) -> i32;

fn cpp_outcome(f: CppLoad, name: &[u8], with_xfer: bool) -> Outcome {
    let cname = CString::new(name.to_vec()).unwrap();
    let mut log = vec![0u8; 1 << 20];
    let mut msg = vec![0u8; 4096];
    let (mut log_len, mut crc, mut code) = (0i32, 0u32, 0u32);
    let status = unsafe {
        f(
            cname.as_ptr(),
            i32::from(with_xfer),
            log.as_mut_ptr(),
            log.len() as i32,
            &mut log_len,
            &mut crc,
            msg.as_mut_ptr() as *mut _,
            msg.len() as i32,
            &mut code,
        )
    };
    assert!((log_len as usize) <= log.len(), "C++ log overflow");
    log.truncate(log_len as usize);
    let detail = match status {
        0 | 4 => Vec::new(),
        1 => msg[..code as usize].to_vec(),
        2 | 3 => code.to_le_bytes().to_vec(),
        _ => panic!("C++ threw something unexpected (status {status})"),
    };
    Outcome { status, detail, log, crc }
}

fn show(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[derive(Default)]
struct Excluded(BTreeMap<String, u64>);
impl Excluded {
    fn add(&mut self, e: &IniError) {
        *self.0.entry(format!("{:?}", e.kind)).or_default() += 1;
    }
    fn total(&self) -> u64 {
        self.0.values().sum()
    }
}

#[test]
fn ini_load_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let table = blocks();
    let mut g = Gen::new(0x1A1);
    let mut check = Check::new("INI::load (throw, log, CRC)");
    let mut excluded = Excluded::default();
    let (mut ok, mut failed, mut log_bytes) = (0u64, 0u64, 0u64);
    const CASES: usize = 30_000;
    for case in 0..CASES {
        CLEAN.with(|c| c.set(case % 5 < 2));
        let data = gen_file(&mut g);
        let name = format!("Data\\INI\\Case{}.ini", case % 7).into_bytes();
        let with_xfer = g.below(4) != 0;
        let mut crc = XferCrc::new_without_nan_guard();
        let r = Ini::load(
            &name,
            &data,
            IniLoadType::Overwrite,
            if with_xfer { Some(&mut crc as &mut dyn Xfer) } else { None },
            &table,
            &mut (),
        );
        let crc_v = if with_xfer { crc.get_crc() } else { 0 };
        let rust = match rust_outcome(r, crc_v) {
            Ok(o) => o,
            Err(e) => {
                excluded.add(&e);
                continue;
            }
        };
        if rust.status == 0 { ok += 1 } else { failed += 1 }
        log_bytes += rust.log.len() as u64;
        unsafe {
            cpp::ref_ini_reset();
            let cname = CString::new(name.clone()).unwrap();
            cpp::ref_ini_add_file(cname.as_ptr(), data.as_ptr() as *const _, data.len() as i32);
        }
        let mut c = cpp_outcome(cpp::ref_ini_load, &name, with_xfer);
        if !with_xfer {
            c.crc = 0;
        }
        check.eq(&rust, &c, || format!("case {case}: file {:?}", show(&data)));
    }
    println!(
        "INI::load: {CASES} files, {ok} loaded, {failed} threw, {log_bytes} log bytes; excluded (undefined upstream): {} {:?}",
        excluded.total(),
        excluded.0
    );
    let seen = SEEN.with(|s| *s.borrow());
    println!("    successful parses per field id 1..=40: {:?}", &seen[1..=40]);
    for (id, &n) in seen.iter().enumerate().take(41).skip(1) {
        assert!(n >= 100, "field id {id} succeeded only {n} times: the generator does not reach it");
    }
    check.finish();
}

#[test]
fn ini_load_directory_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let table = blocks();
    let mut g = Gen::new(0xD1);
    let mut check = Check::new("INI::loadDirectory (throw, log, CRC)");
    let mut excluded = Excluded::default();
    const CASES: usize = 3_000;
    for case in 0..CASES {
        let dir: &[u8] = pick(&mut g, &[&b"Data\\INI"[..], b"Data\\INI", b"Data\\INI", b"", b"D"]);
        let mut list = FilenameList::new();
        let mut files: BTreeMap<Vec<u8>, Vec<u8>> = BTreeMap::new();
        let mut entries = Vec::new();
        for _ in 0..g.below(6) {
            let name: Vec<u8> = match g.below(12) {
                0 => b"x.ini".to_vec(),
                1 => format!("Data\\INI\\Sub{}\\F{}.ini", g.below(2), g.below(3)).into_bytes(),
                2 => format!("Data\\INI/Sub\\G{}.INI", g.below(3)).into_bytes(),
                3 => format!("data\\ini\\f{}.ini", g.below(3)).into_bytes(),
                _ => format!("Data\\INI\\F{}.ini", g.below(3)).into_bytes(),
            };
            if g.below(10) != 0 {
                files.insert(name.clone(), gen_file(&mut g));
            }
            list.insert(name.clone());
            entries.push(name);
        }
        let with_xfer = g.below(2) == 0;
        let mut crc = XferCrc::new_without_nan_guard();
        let r = Ini::load_directory(
            dir,
            true,
            IniLoadType::Overwrite,
            if with_xfer { Some(&mut crc as &mut dyn Xfer) } else { None },
            &list,
            |n| files.get(n).cloned(),
            &table,
            &mut (),
        );
        let crc_v = if with_xfer { crc.get_crc() } else { 0 };
        let rust = match rust_outcome(r, crc_v) {
            Ok(o) => o,
            Err(e) => {
                excluded.add(&e);
                continue;
            }
        };
        unsafe {
            cpp::ref_ini_reset();
            for (n, d) in &files {
                let cn = CString::new(n.clone()).unwrap();
                cpp::ref_ini_add_file(cn.as_ptr(), d.as_ptr() as *const _, d.len() as i32);
            }
            for n in &entries {
                let cn = CString::new(n.clone()).unwrap();
                cpp::ref_ini_add_dir_entry(cn.as_ptr());
            }
        }
        let mut c = cpp_outcome(cpp::ref_ini_load_dir, dir, with_xfer);
        if !with_xfer {
            c.crc = 0;
        }
        check.eq(&rust, &c, || format!("case {case}: dir {:?}, files {:?}", show(dir), entries.iter().map(|e| show(e)).collect::<Vec<_>>()));
    }
    println!("INI::loadDirectory: {CASES} listings; excluded (undefined upstream): {} {:?}", excluded.total(), excluded.0);
    check.finish();
}

fn gen_token(g: &mut Gen, op: i32) -> Vec<u8> {
    let s = match (op, g.below(10)) {
        (0 | 1, 0..=6) => gen_int(g),
        (2 | 3, 0..=6) => gen_real(g),
        (4, 0..=6) => pick(g, &["yes", "no", "YES", "No", "yess", "n", "", " yes", "nO"]).to_string(),
        (5, 0..=6) => gen_name(g, &TEST_NAMES),
        (6, 0..=6) => gen_name(g, &["Low", "Mid", "High", "Max"]),
        (7, 0..=6) => pick(g, &["a.ini", "A.INI", "ini", "in", ".iNi", "x.inix", "", "i", "foo.Ini"]).to_string(),
        (8, 0..=6) => format!(
            "{}{}{}",
            pick(g, &["", " ", "  ", "\t", "\x0b", "\x0c\r"]),
            pick(g, &["End", "end", "END", "En", "Endx", "", "e nd"]),
            pick(g, &["", " ", "  ", "x", " x", "\n", "\t ", " \u{e9}"])
        ),
        (9, 0..=6) => format!(
            "{}{}{}{}{}",
            pick(g, &["", " ", "\t"]),
            pick(g, &["Object", "object", "OBJECT", "Objec", "ObjectX", ""]),
            pick(g, &["", " ", "  ", "\t"]),
            pick(g, &["Tank", "tank", "TANKS", "Tan", ""]),
            pick(g, &["", " ", "x", " x", "\n", " \u{e9}", "\u{e9}"])
        ),
        (_, 7) => {
            // leading C white space before numbers, and other bytes
            format!("{}{}", pick(g, &[" ", "\t", "\n", "\x0b", "\x0c", "\r"]), gen_real(g))
        }
        _ => {
            let n = g.below(12);
            return (0..n).map(|_| 1 + g.below(255) as u8).collect();
        }
    };
    s.into_bytes()
}

#[test]
fn ini_scanners_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x5C);
    let names = [
        "scanInt", "scanUnsignedInt", "scanReal", "scanPercentToReal", "scanBool", "scanIndexList",
        "scanLookupList", "isValidINIFilename", "isEndOfBlock", "isDeclarationOfType",
    ];
    for (op, name) in names.iter().enumerate() {
        let op = op as i32;
        let mut check = Check::new(&format!("INI::{name}"));
        let mut excluded = Excluded::default();
        let mut thrown = 0u64;
        for _ in 0..support::N / 2 {
            let tok = gen_token(&mut g, op);
            if tok.contains(&0) {
                continue;
            }
            let r: IniResult<u32> = match op {
                0 => Ini::scan_int(&tok).map(|v| v as u32),
                1 => Ini::scan_unsigned_int(&tok),
                2 => Ini::scan_real(&tok).map(f32::to_bits),
                3 => Ini::scan_percent_to_real(&tok).map(f32::to_bits),
                4 => Ini::scan_bool(&tok).map(u32::from),
                5 => Ini::scan_index_list(&tok, &TEST_NAMES).map(|v| v as u32),
                6 => Ini::scan_lookup_list(&tok, &TEST_LOOKUP).map(|v| v as u32),
                7 => Ok(u32::from(Ini::is_valid_ini_filename(&tok))),
                8 => Ini::is_end_of_block(&tok).map(u32::from),
                _ => Ini::is_declaration_of_type(b"Object", b"Tank", &tok).map(u32::from),
            };
            let rust = match r {
                Ok(v) => (0, v),
                Err(e) => match e.thrown() {
                    None => {
                        excluded.add(&e);
                        continue;
                    }
                    Some(Thrown::Status(c)) => (2, c),
                    Some(Thrown::ErrorCode(c)) => (3, c),
                    Some(t) => panic!("unexpected {t:?}"),
                },
            };
            if rust.0 != 0 {
                thrown += 1;
            }
            let ct = CString::new(tok.clone()).unwrap();
            let mut out = 0u32;
            let status = unsafe { cpp::ref_ini_scan(op, ct.as_ptr(), &mut out) };
            check.eq(rust, (status, out), || format!("token {:?} ({:02x?})", show(&tok), tok));
        }
        println!("    {name}: {thrown} threw; excluded (undefined upstream): {} {:?}", excluded.total(), excluded.0);
        check.finish();
    }
}
