// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of Phase 1 layer 4's `TerrainTypes`,
//! `MultiplayerSettings` and `CrateSystem` with their INI blocks
//! (`common::terrain_types`, `common::multiplayer_settings`,
//! `logic::crate_system`) against upstream `TerrainTypes.cpp`,
//! `MultiplayerSettings.cpp`, `CrateSystem.cpp`, `INI/INITerrain.cpp`,
//! `INI/INIMultiplayer.cpp` and `INI/INICrate.cpp` (commit 0a05454d), compiled
//! as `differential/wrap_terrain.cpp` describes, with the real `INI::load`,
//! `NameKeyGenerator`, `Overridable` and `KindOfMaskType` underneath.
//!
//! No game INI files are available, so every file is **synthetic**: `Terrain`,
//! `MultiplayerSettings`, `MultiplayerColor`, `MultiplayerStartingMoneyChoice`
//! and `CrateData` blocks (repeated and case-varied names, every field in its
//! several forms, every class, veterancy level and KindOf name, colour
//! components at and beyond 0..=255, percentages, missing and unknown fields,
//! unknown blocks, a missing `End`, comments, CRLF, and in a share of files
//! random byte damage).
//!
//! 1. `INI blocks`: 6,000 files, each loaded by both sides, compared: what
//!    `INI::load` throws (nothing, the `INIException` and its whole message,
//!    an `INI_*` status, an `ErrorCode`), the INI `XferCRC`, **every field of
//!    the three stores** (every terrain in list order, the multiplayer
//!    settings, the colour list with both forms of each colour, the observer
//!    and random colours, the starting-money list and default; every crate
//!    template with its override chain and all of its fields, floats as bit
//!    patterns), also when the load threw, and the name keys that the probe
//!    names then get.
//! 2. `Queries`: after each load, `getColor(which)` for every key from -3 to
//!    the colour count plus one, `findMultiplayerColorDefinitionByName` and
//!    `findCrateTemplate` for a probe list of names.
//! 3. `Override sequences`: 2,000 sequences of up to six loads, each `Overwrite`
//!    or `CreateOverrides`, with `CrateSystem::reset` in between, the crate
//!    store compared (every chain node, its override flag included) after each
//!    step.
//! 4. `Planted bugs`: each comparison also runs a Rust dump with one plausible
//!    translation error (a class off by one, a blend flag inverted, the list in
//!    the wrong order, a colour without its alpha byte, an override flag not
//!    copied, a science key off by one, a name key off by one, ...). Each must be
//!    caught by the inputs.
//!
//! Inputs for which the Rust reports undefined behaviour upstream (a colour
//! component whose `getAsInt` shift is undefined, an out-of-range float, a
//! non-finite real, see the module docs of the three translations) are **not**
//! given to the C++; their count is printed. `ScienceStore`'s lookup is a
//! stand-in that accepts every name (see the wrapper), on both sides.
//!
//! # Results (2026-10-18, `cargo test --release --features differential`)
//!
//! - `ini_blocks_match_upstream`: 6,000 files, 3,307 loaded and 2,555 threw
//!   (both compared, the throw and its message included); 138 excluded as
//!   undefined upstream (86 integer out of range, 37 unsigned out of range, 15
//!   non-finite reals). Compared: 3,014 terrains, 2,472 colours, 2,573 crate
//!   templates (2,573 chain nodes, all fields), 134,826 colour and crate finds,
//!   5,862 name-key sets, 5,862 CRCs. **0 mismatches.**
//! - `override_sequences_match_upstream`: 2,000 sequences, 6,661 loads compared
//!   (3,135 override loads after a load), 1,589 resets, 25,298 chain nodes
//!   seen, 103 excluded (non-finite reals); every store after every step and
//!   reset, 153,203 finds. **0 mismatches.**
//! - Planted bugs, each caught: class off by one, blend inverted, list in
//!   insertion order (terrain); colour without its alpha byte, night colour
//!   equal to the day colour, default money the first amount listed, got-default
//!   flag inverted, `getColor(count)` answering the last colour (settings);
//!   science key off by one, base override flag dropped (crates, sequences
//!   only: the override flag needs override loads); name key off by one.
//!   The observer and random colours are not planted: no INI line sets them.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::game_common::VETERANCY_NAMES;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Thrown, TYPE_TABLE};
use rts_lockstep_engine::common::kind_of::KindOfMaskType;
use rts_lockstep_engine::common::money::Money;
use rts_lockstep_engine::common::multiplayer_settings::{
    MultiplayerColorDefinition, MultiplayerSettings, PLAYERTEMPLATE_OBSERVER, PLAYERTEMPLATE_RANDOM,
};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::terrain_types::{TerrainTypeCollection, TERRAIN_NUM_CLASSES, TERRAIN_TYPE_NAMES};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::crate_system::{CrateSystem, CrateTemplate};
use std::collections::BTreeMap;
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_uchar};
use support::{Check, Gen};

use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;

/// The C++ side has process-global state (the stores, the name keys, `strtok`,
/// `XferLoad`'s static buffer), so the tests take turns.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// ---------------------------------------------------------------------------
// The Rust side of the INI blocks
// ---------------------------------------------------------------------------

struct Ctx {
    terrain: TerrainTypeCollection,
    mp: MultiplayerSettings,
    crates: CrateSystem,
    names: NameKeyGenerator,
}

impl Ctx {
    fn new() -> Self {
        let mut names = NameKeyGenerator::new();
        names.reset();
        Ctx { terrain: TerrainTypeCollection::new(), mp: MultiplayerSettings::new(), crates: CrateSystem::new(), names }
    }
}

fn h_terrain(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.terrain.parse_terrain_definition(ini)
}
fn h_settings(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.mp.parse_multiplayer_settings_definition(ini)
}
fn h_color(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.mp.parse_multiplayer_color_definition(ini)
}
fn h_money(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.mp.parse_multiplayer_starting_money_choice_definition(ini)
}
fn h_crate(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.crates.parse_crate_template_definition(ini, &mut c.names)
}
fn h_stub(_: &mut Ini<'_>, _: &mut Ctx) -> IniResult<()> {
    Err(IniError::new(IniErrorKind::Bug))
}

/// The block table: the five blocks of this layer are translated; the other
/// handlers throw `ERROR_BUG`, as the stubs of wrap_ini.cpp do. The generator
/// never writes a block whose upstream handler is a real one in this build.
fn blocks() -> Vec<BlockParse<Ctx>> {
    TYPE_TABLE
        .iter()
        .map(|&(token, _, _)| BlockParse {
            token,
            parse: match token {
                "Terrain" => h_terrain,
                "MultiplayerSettings" => h_settings,
                "MultiplayerColor" => h_color,
                "MultiplayerStartingMoneyChoice" => h_money,
                "CrateData" => h_crate,
                _ => h_stub,
            },
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The dumps (the byte format of wrap_terrain.cpp's dump functions)
// ---------------------------------------------------------------------------

fn put_u8(v: &mut Vec<u8>, x: u8) {
    v.push(x);
}
fn put_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_i32(v: &mut Vec<u8>, x: i32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_str(v: &mut Vec<u8>, s: &[u8]) {
    put_u32(v, s.len() as u32);
    v.extend_from_slice(s);
}
fn put_bits(v: &mut Vec<u8>, r: f32) {
    put_u32(v, r.to_bits());
}

/// Every terrain, in list order (newest first).
fn rust_terrain_dump(t: &TerrainTypeCollection, bug: u32) -> Vec<u8> {
    let mut v = Vec::new();
    let mut terrains: Vec<_> = t.terrains().collect();
    if bug == 3 {
        // planted: the list in the order it was added
        terrains.reverse();
    }
    put_u32(&mut v, terrains.len() as u32);
    for (i, terr) in terrains.iter().enumerate() {
        put_str(&mut v, terr.name());
        put_str(&mut v, terr.texture());
        // planted 2: the blend flag inverted, on the first terrain
        put_u8(&mut v, u8::from(terr.is_blend_edge() ^ (bug == 2 && i == 0)));
        // planted 1: the class's index off by one, on the first terrain
        let class = terr.class() as i32 + i32::from(bug == 1 && i == 0);
        put_i32(&mut v, class % TERRAIN_NUM_CLASSES);
        put_u8(&mut v, u8::from(terr.restrict_construction()));
    }
    v
}

fn put_color_def(v: &mut Vec<u8>, d: &MultiplayerColorDefinition, bug: u32) {
    put_str(v, d.tooltip_name());
    let rgb = d.rgb_value();
    put_bits(v, rgb.red);
    put_bits(v, rgb.green);
    put_bits(v, rgb.blue);
    let color = if bug == 4 { d.color() & 0x00FF_FFFF } else { d.color() };
    put_i32(v, color);
    // planted 5: the night value is the day value
    let night = if bug == 5 { rgb } else { d.rgb_night_value() };
    put_bits(v, night.red);
    put_bits(v, night.green);
    put_bits(v, night.blue);
    put_i32(v, d.night_color());
}

fn put_money(v: &mut Vec<u8>, m: &Money) {
    put_u32(v, m.count_money());
    put_i32(v, m.player_index());
}

/// The multiplayer settings: the fields, every colour by key, the observer and
/// random colours, the money list, the default money and its flag.
fn rust_mp_dump(s: &mut MultiplayerSettings, bug: u32) -> Vec<u8> {
    let mut v = Vec::new();
    put_i32(&mut v, s.get_start_countdown_timer_seconds());
    put_i32(&mut v, s.get_max_beacons_per_player());
    put_u8(&mut v, u8::from(s.is_shroud_in_multiplayer()));
    put_u8(&mut v, u8::from(s.show_random_player_template()));
    put_u8(&mut v, u8::from(s.show_random_start_pos()));
    put_u8(&mut v, u8::from(s.show_random_color()));
    let count = s.get_num_colors();
    put_u32(&mut v, count as u32);
    for k in 0..count {
        put_i32(&mut v, k);
        let d = s.get_color(k).expect("every key below the count exists").clone();
        put_color_def(&mut v, &d, bug);
    }
    // the observer's and random colours: no INI line sets them, so both are the
    // constructor's values on both sides
    let observer = s.get_color(PLAYERTEMPLATE_OBSERVER).expect("observer").clone();
    put_color_def(&mut v, &observer, bug);
    let random = s.get_color(PLAYERTEMPLATE_RANDOM).expect("random").clone();
    put_color_def(&mut v, &random, bug);
    let money: Vec<Money> = s.get_starting_money_list().to_vec();
    put_u32(&mut v, money.len() as u32);
    for m in &money {
        put_money(&mut v, m);
    }
    // planted 6: the default is the first amount listed, not the one flagged Default
    let default = if bug == 6 { money.first().copied().unwrap_or(*s.get_default_starting_money()) } else { *s.get_default_starting_money() };
    put_money(&mut v, &default);
    // planted 7: the got-default flag inverted
    put_u8(&mut v, u8::from(s.got_default_starting_money() ^ (bug == 7)));
    v
}

fn put_crate_fields(v: &mut Vec<u8>, ct: &CrateTemplate, bug: u32) {
    put_str(v, ct.name());
    put_bits(v, ct.creation_chance());
    let vet = match ct.veterancy_level() {
        None => -1,
        Some(l) => l as i32,
    };
    put_i32(v, vet);
    for b in 0..rts_lockstep_engine::common::kind_of::KINDOF_COUNT {
        put_u8(v, u8::from(ct.killed_by_type_kindof().test(b)));
    }
    // planted 8: the science key off by one, where there is one
    let key = match ct.killer_science() {
        None => -1,
        Some(k) => k.0 as i32 + i32::from(bug == 8),
    };
    put_i32(v, key);
    let crates = ct.possible_crates();
    put_u32(v, crates.len() as u32);
    for c in crates {
        put_str(v, &c.crate_name);
        put_bits(v, c.crate_chance);
    }
    put_u8(v, u8::from(ct.is_owned_by_maker()));
}

/// The crate store: each base template and its override chain, every node
/// with its override flag. Planted 9: the flag of a base template dropped
/// (what a whole-object copy of a flagged default would have carried).
fn rust_crate_dump(c: &CrateSystem, bug: u32) -> Vec<u8> {
    let mut v = Vec::new();
    put_u32(&mut v, c.chains().len() as u32);
    for base in c.chains() {
        let mut node = base;
        loop {
            put_u8(&mut v, 1);
            let flag = node.is_override() && !(bug == 9 && std::ptr::eq(node, base));
            put_u8(&mut v, u8::from(flag));
            put_crate_fields(&mut v, &node.data, bug);
            match node.get_next_override() {
                Some(next) => node = next,
                None => break,
            }
        }
        put_u8(&mut v, 0);
    }
    v
}

/// `findCrateTemplate(name)` as wrap_terrain.cpp's `ref_crate_find` writes it.
fn rust_crate_find(c: &CrateSystem, name: &[u8], bug: u32) -> Vec<u8> {
    let mut v = Vec::new();
    match c.find_crate_template(name) {
        None => put_u8(&mut v, 0),
        Some(ct) => {
            put_u8(&mut v, 1);
            put_crate_fields(&mut v, ct, bug);
        }
    }
    v
}

/// `getColor(which)` for each key, as `ref_mp_colors_by_query` writes it.
fn rust_colors(s: &mut MultiplayerSettings, from: i32, to: i32, bug: u32) -> Vec<u8> {
    let mut v = Vec::new();
    let count = s.get_num_colors();
    for which in from..=to {
        // planted 10: the key just past the last colour answers the last colour, not NULL
        let key = if bug == 10 && which == count && count > 0 { count - 1 } else { which };
        match s.get_color(key) {
            None => put_u8(&mut v, 0),
            Some(d) => {
                put_u8(&mut v, 1);
                let d = d.clone();
                put_color_def(&mut v, &d, 0);
            }
        }
    }
    v
}

/// `findMultiplayerColorDefinitionByName(name)` as `ref_mp_find_by_name` writes it.
fn rust_find_color(s: &MultiplayerSettings, name: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    match s.find_multiplayer_color_definition_by_name(name) {
        None => put_u8(&mut v, 0),
        Some(d) => {
            put_u8(&mut v, 1);
            put_color_def(&mut v, d, 0);
        }
    }
    v
}

/// Runs a dump function of wrap_terrain.cpp into a buffer of its own size.
fn cpp_dump(f: impl Fn(*mut c_uchar, c_int) -> c_int) -> Vec<u8> {
    let mut buf = vec![0u8; 1 << 20];
    let n = f(buf.as_mut_ptr(), buf.len() as c_int);
    assert!(n >= 0 && (n as usize) <= buf.len(), "dump of {n} bytes does not fit");
    buf.truncate(n as usize);
    buf
}

fn cstr(s: &[u8]) -> CString {
    CString::new(s.to_vec()).expect("no NUL in a generated name")
}

// ---------------------------------------------------------------------------
// The C++ load and the outcome
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    status: i32,
    detail: Vec<u8>,
    crc: u32,
}

fn rust_outcome(r: IniResult<()>, crc: u32) -> Result<Outcome, IniError> {
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
    Ok(Outcome { status, detail, crc })
}

/// `INI::load` of `data` as `load_type` on the C++ side (wrap_ini.cpp's hooks).
fn cpp_load(name: &[u8], data: &[u8], load_type: IniLoadType, with_xfer: bool) -> Outcome {
    let cname = cstr(name);
    let mut log = vec![0u8; 16];
    let mut msg = vec![0u8; 4096];
    let (mut log_len, mut crc, mut code) = (0i32, 0u32, 0u32);
    let status = unsafe {
        cpp::ref_ini_reset();
        cpp::ref_ini_add_file(cname.as_ptr(), data.as_ptr() as *const c_char, data.len() as c_int);
        cpp::ref_ini_load_typed(
            cname.as_ptr(),
            load_type as c_int,
            c_int::from(with_xfer),
            log.as_mut_ptr(),
            log.len() as c_int,
            &mut log_len,
            &mut crc,
            msg.as_mut_ptr() as *mut c_char,
            msg.len() as c_int,
            &mut code,
        )
    };
    assert_eq!(log_len, 0, "the real block handlers log nothing");
    let detail = match status {
        0 | 4 => Vec::new(),
        1 => msg[..code as usize].to_vec(),
        2 | 3 => code.to_le_bytes().to_vec(),
        _ => panic!("C++ threw something unexpected (status {status})"),
    };
    Outcome { status, detail, crc: if with_xfer { crc } else { 0 } }
}

fn show(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// Names the queries probe: the block names of the generator, the defaults,
/// the case variants, and some that are never written.
const PROBES: [&str; 22] = [
    "DefaultTerrain", "Dirt", "dirt", "Grass", "Water", "Rock", "Snow1", "Probe", "DefaultCrate", "CrateA", "CrateB",
    "cratea", "Salvage", "Red", "Blue", "Green", "X", "COLOR:Red", "COLOR:Blue", "SCIENCE_A", "SCIENCE_B", "zzz",
];

// ---------------------------------------------------------------------------
// The generator
// ---------------------------------------------------------------------------

const TERRAIN_NAMES: [&str; 7] = ["DefaultTerrain", "Dirt", "dirt", "Grass", "Water", "Rock", "Snow1"];
const TEXTURES: [&str; 3] = ["dirt.tga", "grass.tga", "w.tga"];
const CRATE_NAMES: [&str; 5] = ["DefaultCrate", "CrateA", "CrateB", "cratea", "Salvage"];
const COLOR_BLOCK_NAMES: [&str; 5] = ["Red", "Blue", "Green", "X", "Red"];
const TOOLTIPS: [&str; 5] = ["COLOR:Red", "COLOR:Blue", "Red", "COLOR:Green", "X"];
const SCIENCES: [&str; 3] = ["SCIENCE_A", "SCIENCE_B", "SCIENCE_C"];
const BOOL_OK: [&str; 6] = ["Yes", "No", "yes", "no", "YES", "nO"];
const BOOL_BAD: [&str; 5] = ["maybe", "1", "0", "true", "y"];
const REALS_OK: [&str; 6] = ["0.5", "1", "0.25", "1e-3", "0", "2.75"];
const REALS_BAD: [&str; 6] = ["x", "-0.5", "1.5e30", "1.5.3", "", "inf"];

fn pick<'a>(g: &mut Gen, xs: &[&'a str]) -> &'a str {
    xs[g.below(xs.len() as u32) as usize]
}

/// Upper, lower or mixed case: names are compared exactly.
fn case_mix(g: &mut Gen, s: &str) -> String {
    match g.below(4) {
        0 => s.to_ascii_lowercase(),
        1 => s.to_ascii_uppercase(),
        _ => s.to_string(),
    }
}

fn gen_sep(g: &mut Gen) -> &'static str {
    [" ", "  ", "\t", " = ", "=", " = ", "\t=\t"][g.below(7) as usize]
}

fn gen_int(g: &mut Gen, clean: bool) -> String {
    if clean {
        g.below(200).to_string()
    } else {
        ["-3", "x", "99999999999", "7.5", "-0"][g.below(5) as usize].to_string()
    }
}

fn gen_bool(g: &mut Gen, clean: bool) -> String {
    if clean { pick(g, &BOOL_OK) } else { pick(g, &BOOL_BAD) }.to_string()
}

fn gen_rgb(g: &mut Gen, clean: bool) -> String {
    let comp = |g: &mut Gen| -> String {
        if clean {
            g.below(256).to_string()
        } else {
            ["256", "-1", "x", "300"][g.below(4) as usize].to_string()
        }
    };
    match g.below(12) {
        0 => format!("{} {} {}", comp(g), comp(g), comp(g)),
        1 if !clean => format!("R:{} G:{}", comp(g), comp(g)),
        _ => format!("R:{} G:{} B:{}", comp(g), comp(g), comp(g)),
    }
}

/// One field generator: a value for `clean` (or dirty) input.
type FieldGen = fn(&mut Gen, bool) -> String;

fn write_fields(g: &mut Gen, out: &mut String, clean: bool, fields: &[(&str, FieldGen)]) {
    for (token, value) in fields {
        if g.below(10) < 7 {
            let indent = [" ", "  ", "\t"][g.below(3) as usize];
            out.push_str(&format!("{indent}{token}{}{}\n", gen_sep(g), value(g, clean)));
        }
    }
    if !clean && g.below(8) == 0 {
        out.push_str(" Foo = 1\n");
    }
}

fn gen_terrain_field_class(g: &mut Gen, clean: bool) -> String {
    if clean || g.below(4) != 0 {
        pick(g, &TERRAIN_TYPE_NAMES).to_string()
    } else {
        pick(g, &["NOPE", "grass", "BLEND_EDGES", ""]).to_string()
    }
}

fn gen_texture(g: &mut Gen, _clean: bool) -> String {
    pick(g, &TEXTURES).to_string()
}

fn gen_blend(g: &mut Gen, clean: bool) -> String {
    gen_bool(g, clean)
}

fn gen_restrict(g: &mut Gen, clean: bool) -> String {
    gen_bool(g, clean)
}

fn gen_countdown(g: &mut Gen, clean: bool) -> String {
    gen_int(g, clean)
}

fn gen_beacons(g: &mut Gen, clean: bool) -> String {
    gen_int(g, clean)
}

fn gen_shroud(g: &mut Gen, clean: bool) -> String {
    gen_bool(g, clean)
}

fn gen_tooltip(g: &mut Gen, _clean: bool) -> String {
    pick(g, &TOOLTIPS).to_string()
}

fn gen_rgb_day(g: &mut Gen, clean: bool) -> String {
    gen_rgb(g, clean)
}

fn gen_rgb_night(g: &mut Gen, clean: bool) -> String {
    gen_rgb(g, clean)
}

fn gen_money_value(g: &mut Gen, clean: bool) -> String {
    if clean {
        [0u32, 5000, 10000, 20000, 100000][g.below(5) as usize].to_string()
    } else {
        ["-1", "many", "99999999999"][g.below(3) as usize].to_string()
    }
}

fn gen_money_default(g: &mut Gen, clean: bool) -> String {
    gen_bool(g, clean)
}

fn gen_creation_chance(g: &mut Gen, clean: bool) -> String {
    if clean || g.below(3) != 0 {
        pick(g, &REALS_OK).to_string()
    } else {
        pick(g, &REALS_BAD).to_string()
    }
}

fn gen_veterancy(g: &mut Gen, clean: bool) -> String {
    if clean || g.below(4) != 0 {
        pick(g, &VETERANCY_NAMES).to_string()
    } else {
        "LEGEND".to_string()
    }
}

fn gen_killed_by(g: &mut Gen, clean: bool) -> String {
    let names = KindOfMaskType::get_bit_names();
    if g.below(10) == 0 {
        return "NONE".to_string();
    }
    let mut words: Vec<String> = Vec::new();
    for _ in 0..1 + g.below(3) {
        let w = names[g.below(names.len() as u32) as usize];
        words.push(if g.below(5) == 0 { format!("+{w}") } else { w.to_string() });
    }
    if !clean && g.below(3) == 0 {
        words.push("NOT_A_KIND".to_string());
    }
    words.join(" ")
}

fn gen_crate_object(g: &mut Gen, clean: bool) -> String {
    let chance = if clean || g.below(4) != 0 { pick(g, &REALS_OK) } else { pick(g, &REALS_BAD) };
    format!("{} {}", pick(g, &CRATE_NAMES), chance)
}

fn gen_science(g: &mut Gen, _clean: bool) -> String {
    pick(g, &SCIENCES).to_string()
}

fn gen_owned(g: &mut Gen, clean: bool) -> String {
    gen_bool(g, clean)
}

const TERRAIN_FIELDS: [(&str, FieldGen); 4] = [
    ("Texture", gen_texture),
    ("BlendEdges", gen_blend),
    ("Class", gen_terrain_field_class),
    ("RestrictConstruction", gen_restrict),
];
const SETTINGS_FIELDS: [(&str, FieldGen); 6] = [
    ("StartCountdownTimer", gen_countdown),
    ("MaxBeaconsPerPlayer", gen_beacons),
    ("UseShroud", gen_shroud),
    ("ShowRandomPlayerTemplate", gen_bool_field),
    ("ShowRandomStartPos", gen_bool_field),
    ("ShowRandomColor", gen_bool_field),
];
const COLOR_FIELDS: [(&str, FieldGen); 3] = [("TooltipName", gen_tooltip), ("RGBColor", gen_rgb_day), ("RGBNightColor", gen_rgb_night)];
const MONEY_FIELDS: [(&str, FieldGen); 2] = [("Value", gen_money_value), ("Default", gen_money_default)];
const CRATE_FIELDS: [(&str, FieldGen); 6] = [
    ("CreationChance", gen_creation_chance),
    ("VeterancyLevel", gen_veterancy),
    ("KilledByType", gen_killed_by),
    ("CrateObject", gen_crate_object),
    ("KillerScience", gen_science),
    ("OwnedByMaker", gen_owned),
];

fn gen_bool_field(g: &mut Gen, clean: bool) -> String {
    gen_bool(g, clean)
}

/// One block (its kind by weight), its `End` in one of three spellings.
/// `crates_only` restricts the kinds to `CrateData`.
fn gen_block(g: &mut Gen, out: &mut String, clean: bool, crates_only: bool) {
    let end = match g.below(10) {
        0 => "END",
        1 => "end",
        _ => "End",
    };
    let kind = if crates_only { 77 + g.below(21) } else { g.below(100) };
    match kind {
        0..=24 => {
            let base = pick(g, &TERRAIN_NAMES);
            let name = case_mix(g, base);
            out.push_str(&format!("Terrain {name}\n"));
            write_fields(g, out, clean, &TERRAIN_FIELDS);
        }
        25..=44 => {
            out.push_str("MultiplayerSettings\n");
            write_fields(g, out, clean, &SETTINGS_FIELDS);
        }
        45..=64 => {
            let base = pick(g, &COLOR_BLOCK_NAMES);
            let name = case_mix(g, base);
            out.push_str(&format!("MultiplayerColor {name}\n"));
            write_fields(g, out, clean, &COLOR_FIELDS);
        }
        65..=76 => {
            out.push_str("MultiplayerStartingMoneyChoice\n");
            write_fields(g, out, clean, &MONEY_FIELDS);
        }
        77..=97 => {
            let base = pick(g, &CRATE_NAMES);
            let name = case_mix(g, base);
            out.push_str(&format!("CrateData {name}\n"));
            write_fields(g, out, clean, &CRATE_FIELDS);
        }
        98 => out.push_str("FXList Probe\nEnd\n"),
        _ => out.push_str("Bogus Thing\nEnd\n"),
    }
    out.push_str(end);
    out.push('\n');
}

/// A file of 0..=6 blocks, with comments and CRLF line ends sometimes, a
/// missing final `End` and random damage in a share of files.
fn gen_file(g: &mut Gen, crates_only: bool) -> Vec<u8> {
    let clean = g.below(10) < 7;
    let mut text = String::new();
    if g.below(5) == 0 {
        text.push_str("; a comment line\n");
    }
    for _ in 0..g.below(7) {
        gen_block(g, &mut text, clean, crates_only);
    }
    if g.below(12) == 0 && text.ends_with("End\n") {
        let cut = text.len() - 4;
        text.truncate(cut);
    }
    if g.below(10) == 0 {
        text = text.replace('\n', " ; c\r\n");
    }
    let mut bytes = text.into_bytes();
    if !crates_only && g.below(20) == 0 {
        for _ in 0..1 + g.below(4) {
            if !bytes.is_empty() {
                let i = g.below(bytes.len() as u32) as usize;
                bytes[i] = (g.u32() & 0x7f) as u8 | 0x20;
            }
        }
    }
    bytes
}

// ---------------------------------------------------------------------------
// The comparisons
// ---------------------------------------------------------------------------

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

/// Every check of one comparison round: the stores, the queries, and the
/// planted variants of each.
struct Checks {
    outcome: Check,
    terrains: Check,
    settings: Check,
    crates: Check,
    keys: Check,
    colors: Check,
    find_color: Check,
    find_crate: Check,
    /// planted bugs, each caught by the inputs: (what, check)
    planted: Vec<Check>,
}

impl Checks {
    fn new() -> Self {
        let planted = [
            "P1 class off by one (terrain)",
            "P2 blend flag inverted (terrain)",
            "P3 list in insertion order (terrain)",
            "P4 colour without its alpha byte",
            "P5 night colour is the day colour",
            "P6 default money is the first amount listed",
            "P7 got-default flag inverted",
            "P8 science key off by one (crate)",
            "P9 base template's override flag dropped",
            "P10 getColor(count) answers the last colour",
            "P11 name key off by one",
        ];
        Checks {
            outcome: Check::new("Terrain/Multiplayer/Crate INI::load (throw, CRC)"),
            terrains: Check::new("TheTerrainTypes (every field, list order)"),
            settings: Check::new("TheMultiplayerSettings (fields, colours, money)"),
            crates: Check::new("TheCrateSystem (chains, every field, flags)"),
            keys: Check::new("name keys after the load"),
            colors: Check::new("getColor for every key (and the two others)"),
            find_color: Check::new("findMultiplayerColorDefinitionByName"),
            find_crate: Check::new("findCrateTemplate (every field, chain final)"),
            planted: planted.iter().map(|n| Check::new(n)).collect(),
        }
    }

    /// Finishes every check, and the planted bugs at `planted` (the others
    /// need inputs this round does not generate: e.g. terrains, in a crate-only run).
    fn finish_with_planted(self, planted: &[usize]) {
        self.outcome.finish();
        self.terrains.finish();
        self.settings.finish();
        self.crates.finish();
        self.keys.finish();
        self.colors.finish();
        self.find_color.finish();
        self.find_crate.finish();
        for (i, p) in self.planted.into_iter().enumerate() {
            if planted.contains(&i) {
                p.finish_planted();
            }
        }
    }
}

/// Compares the stores, the queries and the planted variants, after one load
/// on both sides (`data` names the input in a mismatch).
fn compare_stores(ctx: &mut Ctx, data: &[u8], checks: &mut Checks, what: &str) {
    let cterr = cpp_dump(|b, c| unsafe { cpp::ref_terrain_dump(b, c) });
    let cmp = cpp_dump(|b, c| unsafe { cpp::ref_mp_dump(b, c) });
    let ccr = cpp_dump(|b, c| unsafe { cpp::ref_crate_dump(b, c) });
    let input = || format!("{what}: file {:?}", show(data));
    checks.terrains.eq(rust_terrain_dump(&ctx.terrain, 0), cterr.clone(), input);
    checks.settings.eq(rust_mp_dump(&mut ctx.mp, 0), cmp.clone(), input);
    checks.crates.eq(rust_crate_dump(&ctx.crates, 0), ccr.clone(), input);
    // planted: the same comparisons with one translation error each
    checks.planted[0].eq(rust_terrain_dump(&ctx.terrain, 1), cterr.clone(), input);
    checks.planted[1].eq(rust_terrain_dump(&ctx.terrain, 2), cterr.clone(), input);
    checks.planted[2].eq(rust_terrain_dump(&ctx.terrain, 3), cterr.clone(), input);
    checks.planted[3].eq(rust_mp_dump(&mut ctx.mp, 4), cmp.clone(), input);
    checks.planted[4].eq(rust_mp_dump(&mut ctx.mp, 5), cmp.clone(), input);
    checks.planted[5].eq(rust_mp_dump(&mut ctx.mp, 6), cmp.clone(), input);
    checks.planted[6].eq(rust_mp_dump(&mut ctx.mp, 7), cmp.clone(), input);
    checks.planted[7].eq(rust_crate_dump(&ctx.crates, 8), ccr.clone(), input);
    checks.planted[8].eq(rust_crate_dump(&ctx.crates, 9), ccr.clone(), input);
}

/// The queries after a load: the name keys, `getColor` for every key from -3
/// to the colour count plus one, the colour and crate finds of the probes.
fn compare_queries(ctx: &mut Ctx, data: &[u8], checks: &mut Checks) {
    let probes = PROBES;
    let input = || format!("file {:?}", show(data));
    let rk: Vec<u32> = probes.iter().map(|n| ctx.names.name_to_key(n.as_bytes()).0).collect();
    let ck: Vec<u32> = probes
        .iter()
        .map(|n| {
            let c = cstr(n.as_bytes());
            unsafe { cpp::ref_terrain_name_key(c.as_ptr()) }
        })
        .collect();
    checks.keys.eq(rk.clone(), ck.clone(), input);
    checks.planted[10].eq(rk.iter().map(|k| k + 1).collect::<Vec<u32>>(), ck, input);
    let count = ctx.mp.get_num_colors();
    let cc = cpp_dump(|b, c| unsafe { cpp::ref_mp_colors_by_query(b, c, -3, count + 1) });
    checks.colors.eq(rust_colors(&mut ctx.mp, -3, count + 1, 0), cc.clone(), input);
    checks.planted[9].eq(rust_colors(&mut ctx.mp, -3, count + 1, 10), cc, input);
    for n in probes.iter().chain(std::iter::once(&"Probe")) {
        let c = cstr(n.as_bytes());
        let input_n = || format!("name {n:?}, file {:?}", show(data));
        let rf = rust_find_color(&ctx.mp, n.as_bytes());
        let cf = cpp_dump(|b, cap| unsafe { cpp::ref_mp_find_by_name(c.as_ptr(), b, cap) });
        checks.find_color.eq(rf, cf, input_n);
        let rq = rust_crate_find(&ctx.crates, n.as_bytes(), 0);
        let cq = cpp_dump(|b, cap| unsafe { cpp::ref_crate_find(c.as_ptr(), b, cap) });
        checks.find_crate.eq(rq, cq, input_n);
    }
}

fn chain_nodes(c: &CrateSystem) -> u64 {
    c.chains()
        .iter()
        .map(|b| {
            let mut n = 1u64;
            let mut node = b;
            while let Some(next) = node.get_next_override() {
                n += 1;
                node = next;
            }
            n
        })
        .sum()
}

// ---------------------------------------------------------------------------
// The tests
// ---------------------------------------------------------------------------

#[test]
fn ini_blocks_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let table = blocks();
    let mut g = Gen::new(0x007E_44A1);
    let mut checks = Checks::new();
    let mut excluded = Excluded::default();
    let (mut ok, mut failed, mut terrains, mut colours, mut templates, mut nodes) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    const CASES: usize = 6_000;
    for case in 0..CASES {
        let data = gen_file(&mut g, false);
        let name = format!("Data\\INI\\Case{}.ini", case % 7).into_bytes();
        let with_xfer = g.below(4) != 0;
        let mut ctx = Ctx::new();
        let mut crc = XferCrc::new_without_nan_guard();
        let r = Ini::load(
            &name,
            &data,
            IniLoadType::Overwrite,
            if with_xfer { Some(&mut crc as &mut dyn Xfer) } else { None },
            &table,
            &mut ctx,
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
        terrains += ctx.terrain.len() as u64;
        colours += ctx.mp.get_num_colors() as u64;
        templates += ctx.crates.chains().len() as u64;
        nodes += chain_nodes(&ctx.crates);
        unsafe { cpp::ref_terrain_reset() };
        let c = cpp_load(&name, &data, IniLoadType::Overwrite, with_xfer);
        checks.outcome.eq(rust, c, || format!("case {case}: file {:?}", show(&data)));
        let what = format!("case {case}");
        compare_stores(&mut ctx, &data, &mut checks, &what);
        compare_queries(&mut ctx, &data, &mut checks);
    }
    println!(
        "Terrain/Multiplayer/Crate INI: {CASES} files, {ok} loaded, {failed} threw; compared {terrains} terrains, {colours} colours, {templates} crate templates ({nodes} chain nodes); excluded (undefined upstream): {} {:?}",
        excluded.total(),
        excluded.0
    );
    assert!(ok > 1000 && failed > 1000, "the generator must reach both outcomes: {ok} loaded, {failed} threw");
    assert!(terrains > 1000 && colours > 1000 && templates > 1000, "too few stores compared");
    // the override plant (P9) needs override loads: the sequence test below runs it
    checks.finish_with_planted(&[0, 1, 2, 3, 4, 5, 6, 7, 9, 10]);
}

#[test]
fn override_sequences_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let table = blocks();
    let mut g = Gen::new(0x5EC0_0A1D);
    let mut checks = Checks::new();
    let mut excluded = Excluded::default();
    let (mut steps, mut resets, mut nodes, mut overrides_made) = (0u64, 0u64, 0u64, 0u64);
    const SEQUENCES: usize = 2_000;
    for seq in 0..SEQUENCES {
        let mut ctx = Ctx::new();
        unsafe { cpp::ref_terrain_reset() };
        let name = b"Data\\INI\\Sequence.ini".to_vec();
        for step in 0..1 + g.below(6) {
            let data = gen_file(&mut g, true);
            let load_type = if g.below(2) == 0 { IniLoadType::Overwrite } else { IniLoadType::CreateOverrides };
            let r = Ini::load(&name, &data, load_type, None, &table, &mut ctx);
            let rust = match rust_outcome(r, 0) {
                Ok(o) => o,
                Err(e) => {
                    // an undefined input ends the sequence (both sides would be undefined)
                    excluded.add(&e);
                    break;
                }
            };
            let c = cpp_load(&name, &data, load_type, false);
            checks.outcome.eq(rust, c, || format!("sequence {seq} step {step}: file {:?}", show(&data)));
            steps += 1;
            let before = chain_nodes(&ctx.crates);
            let what = format!("sequence {seq} step {step} ({load_type:?})");
            compare_stores(&mut ctx, &data, &mut checks, &what);
            compare_queries(&mut ctx, &data, &mut checks);
            nodes += before;
            overrides_made += u64::from(before > 0 && load_type == IniLoadType::CreateOverrides);
            if g.below(4) == 0 {
                ctx.crates.reset();
                unsafe { cpp::ref_crate_reset() };
                resets += 1;
                let what = format!("sequence {seq} step {step} after reset");
                compare_stores(&mut ctx, &data, &mut checks, &what);
            }
        }
    }
    println!(
        "Crate override sequences: {SEQUENCES} sequences, {steps} loads compared, {resets} resets, {nodes} chain nodes seen, {overrides_made} override loads after a load; excluded: {} {:?}",
        excluded.total(),
        excluded.0
    );
    assert!(steps > 3_000 && resets > 300, "too few steps: {steps} loads, {resets} resets");
    // the crate-only sequences reach the crate plants (P8, P9) and the key plant (P11) only
    checks.finish_with_planted(&[7, 8, 10]);
}
