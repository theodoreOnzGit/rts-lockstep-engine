// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of Phase 1 layer 4's `GlobalData`
//! (`common::global_data`, with `logic::weapon_bonus::WeaponBonusSet`) against
//! upstream `GlobalData.cpp`, `INIGameData.cpp` and the `WeaponBonusSet` parts
//! of `Weapon.h`/`Weapon.cpp` (commit 0a05454d), compiled as
//! `differential/wrap_globaldata.cpp` and build.rs describe (the one edit:
//! the tail of `parseGameDataDefinition` that reads the player's
//! `OptionPreferences` is cut), with the real `INI::load`, `Money` and
//! `XferCRC` underneath, in the retail configuration (`NDEBUG`).
//!
//! No game INI files are available, so every file is **synthetic**: `GameData`
//! blocks (1 to 3 per file, 1 to 3 files per case) naming the 336 fields of
//! the retail table in random order and in every form their parsers take
//! (booleans, ints in and out of the member's range, reals, percentages,
//! durations, angles, colours, coordinates, names, quoted names, name lists,
//! index lists, money, `WeaponBonus` triples), with malformed values,
//! missing tokens, unknown fields (including the debug-only and commented-out
//! ones the retail table lacks, and the wrong case), a missing `End`,
//! comments, `CRLF`, `GameData` followed by other block types, and in a share
//! of cases random byte damage to the whole file. Each file is loaded with
//! `INI_LOAD_OVERWRITE`, `INI_LOAD_CREATE_OVERRIDES` or `INI_LOAD_MULTIFILE`;
//! between loads a case may `reset()`, call `setTimeOfDay`, or find the
//! original already created (as the engine does before it reads
//! `Default\GameData`).
//!
//! 1. **The table**: the 336 tokens, in order, against upstream's.
//! 2. **The constructor**: every member of a new `GlobalData`.
//! 3. **INI blocks**: 6,000 cases, about 11,700 loads. After every action, compared:
//!    what `INI::load` throws (nothing, an `INIException` and its whole
//!    message, an `INI_*` status code, an `ErrorCode`, `std::out_of_range`),
//!    the INI `XferCRC`, the length of the `m_next` chain, and **every data
//!    member of every instance of the chain**, floats bitwise (about 280
//!    members each; the user data directory included).
//! 4. `setTimeOfDay` (also out of range) and `WeaponBonusSet::appendBonuses`
//!    (all 32 flag bits, 27 of them conditions) on loaded data.
//!
//! Inputs for which the Rust reports undefined behaviour upstream (a number
//! out of `Int` range, `inf` or `nan` as a real, ...: the INI reader's own, see
//! its module docs) are **not** given to the C++; their cases are counted and
//! printed. `GlobalData` adds none.
//!
//! **Planted bugs.** Each comparison also runs a deliberately wrong variant,
//! built from the public API: an override that copies the previous instance
//! (what the comment of `newOverride` promises, and `operator=` does not
//! do), `DrawSkyBox` stored as a whole `Real` instead of one byte, a
//! saturating `FirewallPortAllocationDelta`, `StandardPublicBone` that
//! replaces instead of appending, a wrong default; all must be caught.
//!
//! # Results
//!
//! Recorded on 2026-10-18 by `cargo test --release --features differential
//! --test differential_globaldata -- --nocapture` (the `DIFF`, `PLANTED` and
//! `GlobalData INI` lines), against the C++ built as described above:
//!
//! - **Table**: 336 of 336 tokens, in the same order.
//! - **Constructor**: 2,854 bytes (every member of one instance), no
//!   difference.
//! - **INI blocks**: 6,000 cases generated, 158 left out (undefined upstream:
//!   106 `Int` out of range, 4 `Real` out of range, 48 `inf`/`nan`/hex
//!   reals), 5,842 run: 11,682 loads (5,721 loaded, 5,961 threw, 3,459 made
//!   an override), 1,747 resets, 1,138 `setTimeOfDay`, 516,323 field lines,
//!   chains of up to 8 instances. 16,865 outcome comparisons (throw, whole
//!   message, CRC) and 33,730 whole-chain comparisons (length and every
//!   member of every instance): **0 mismatches**. Every one of the 336
//!   tokens was accepted in a file that loaded at least 655 times (mean 727).
//! - **`setTimeOfDay` and `appendBonuses`**: 12,000 queries each (4,388 times
//!   in range, 7,612 outside), 36,000 comparisons: 0 mismatches.
//! - **Planted bugs, caught**: the copying override in 2,841 of 9,038 steps
//!   (a case needs an override with data in front of it to tell); `DrawSkyBox`
//!   as a whole `Real` 901 times; the saturating `FirewallPortAllocationDelta`
//!   121; the replacing `StandardPublicBone` 105; the wrong default 1 byte;
//!   `appendBonuses` with the flag bits reversed 9,670 of 12,000.
//! - **Bugs planted in the translation itself, run, and removed**: the default
//!   `m_maxFieldParticleCount` 31 instead of 30 (15,573 chain mismatches and
//!   the constructor test); `WaterPositionY` stored in `m_waterPositionZ`
//!   (1,435); `RefundPercent` parsed as a plain real, not a percentage (1,360).
//!
//! # Found by this test
//!
//! `INI::parseCoord3D` and `parseCoord2D` store each component as it is
//! scanned, so `X:1 Y:oops` leaves `x` stored when the load throws; the Rust
//! `Ini::parse_coord3d` returns the whole value only when all components
//! parse. The first run differed in `m_terrainLighting` after such lines; the
//! `GameData` table now reads the components itself (`common::global_data`).
//! No other translated table uses those two yet; `differential_ini.rs` logs
//! the value only after a successful parse, so it could not see the
//! difference. A later table that stores a coordinate should do the same.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::base_type::{Coord2D, Coord3D, RGBColor};
use rts_lockstep_engine::common::global_data::{
    parse_game_data_definition, GlobalData, GlobalDataStore, TerrainLighting, BODY_DAMAGE_TYPE_NAMES, GLOBAL_DATA_FIELDS, TERRAIN_LOD_NAMES,
};
use rts_lockstep_engine::common::game_type::{TIME_OF_DAY_NAMES, WEATHER_NAMES};
use rts_lockstep_engine::common::ini::{BlockParse, FieldParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Thrown, UserData, TYPE_TABLE};
use rts_lockstep_engine::common::money::Money;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::weapon_bonus::{WeaponBonus, WeaponBonusSet, WEAPONBONUSCONDITION_COUNT, WEAPON_BONUS_FIELD_COUNT, WEAPON_BONUS_FIELD_NAMES, WEAPON_BONUS_NAMES};
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::OnceLock;
use support::{Check, Gen};

/// The C++ side has process-global state (the chain, the in-memory file system,
/// `strtok`), so the tests take turns.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// ---------------------------------------------------------------------------
// The dump
// ---------------------------------------------------------------------------

/// How wrap_globaldata.cpp's `putAny` writes a member.
trait Dump {
    fn dump(&self, d: &mut Vec<u8>);
}
impl Dump for bool {
    fn dump(&self, d: &mut Vec<u8>) {
        d.push(u8::from(*self));
    }
}
impl Dump for u8 {
    fn dump(&self, d: &mut Vec<u8>) {
        d.push(*self);
    }
}
impl Dump for i16 {
    fn dump(&self, d: &mut Vec<u8>) {
        d.extend_from_slice(&self.to_le_bytes());
    }
}
impl Dump for i32 {
    fn dump(&self, d: &mut Vec<u8>) {
        d.extend_from_slice(&self.to_le_bytes());
    }
}
impl Dump for u32 {
    fn dump(&self, d: &mut Vec<u8>) {
        d.extend_from_slice(&self.to_le_bytes());
    }
}
impl Dump for f32 {
    fn dump(&self, d: &mut Vec<u8>) {
        d.extend_from_slice(&self.to_bits().to_le_bytes());
    }
}
impl Dump for Vec<u8> {
    fn dump(&self, d: &mut Vec<u8>) {
        (self.len() as u32).dump(d);
        d.extend_from_slice(self);
    }
}
impl Dump for Vec<Vec<u8>> {
    fn dump(&self, d: &mut Vec<u8>) {
        (self.len() as u32).dump(d);
        for s in self {
            s.dump(d);
        }
    }
}
impl Dump for Coord3D {
    fn dump(&self, d: &mut Vec<u8>) {
        self.x.dump(d);
        self.y.dump(d);
        self.z.dump(d);
    }
}
impl Dump for Coord2D {
    fn dump(&self, d: &mut Vec<u8>) {
        self.x.dump(d);
        self.y.dump(d);
    }
}
impl Dump for RGBColor {
    fn dump(&self, d: &mut Vec<u8>) {
        self.red.dump(d);
        self.green.dump(d);
        self.blue.dump(d);
    }
}
impl Dump for Money {
    fn dump(&self, d: &mut Vec<u8>) {
        self.count_money().dump(d);
        self.player_index().dump(d);
    }
}
impl Dump for TerrainLighting {
    fn dump(&self, d: &mut Vec<u8>) {
        self.ambient.dump(d);
        self.diffuse.dump(d);
        self.light_pos.dump(d);
    }
}
impl Dump for WeaponBonusSet {
    fn dump(&self, d: &mut Vec<u8>) {
        for c in 0..WEAPONBONUSCONDITION_COUNT {
            for f in 0..WEAPON_BONUS_FIELD_COUNT {
                self.bonus(c).get_field(f).dump(d);
            }
        }
    }
}
impl<T: Dump, const N: usize> Dump for [T; N] {
    fn dump(&self, d: &mut Vec<u8>) {
        for x in self {
            x.dump(d);
        }
    }
}

// `dump_fields` and `FIELD_KINDS`, generated by scripts/gen_global_data.py
include!("support/global_data_dump.rs");

/// The dump of `ref_globaldata_dump`: the length of the chain, then every
/// instance's members, newest first. `marks` gets where each member begins.
fn rust_dump_marked(store: &GlobalDataStore) -> (Vec<u8>, Vec<(usize, String)>) {
    let mut d = Vec::new();
    (store.len() as u32).dump(&mut d);
    let mut all = Vec::new();
    for (i, g) in store.instances().enumerate() {
        let mut marks = Vec::new();
        dump_fields(g, &mut d, &mut marks);
        all.extend(marks.into_iter().map(|(at, name)| (at, format!("instance {i} (newest first): {name}"))));
    }
    (d, all)
}

fn rust_dump(store: &GlobalDataStore) -> Vec<u8> {
    rust_dump_marked(store).0
}

fn cpp_dump() -> Vec<u8> {
    let mut buf = vec![0u8; 1 << 22];
    let n = unsafe { cpp::ref_globaldata_dump(buf.as_mut_ptr(), buf.len() as i32) };
    assert!((n as usize) <= buf.len(), "dump overflow");
    buf.truncate(n as usize);
    buf
}

/// Names the first differing member of two dumps (the Rust one's `marks`).
fn first_difference(a: &[u8], b: &[u8], marks: &[(usize, String)]) -> String {
    match a.iter().zip(b.iter()).position(|(x, y)| x != y) {
        Some(i) => {
            let m = marks.iter().rev().find(|(at, _)| *at <= i);
            format!("first differing byte {i}: {}", m.map_or("the chain length".to_string(), |(_, n)| n.clone()))
        }
        None => format!("lengths {} / {}", a.len(), b.len()),
    }
}

// ---------------------------------------------------------------------------
// The Rust side of the blocks
// ---------------------------------------------------------------------------

/// A `GameData` block handler.
type GameDataHandler = fn(&mut Ini<'_>, &mut GlobalDataStore) -> IniResult<()>;

fn h_game_data(ini: &mut Ini<'_>, store: &mut GlobalDataStore) -> IniResult<()> {
    parse_game_data_definition(ini, store)
}
/// The handler of the first planted bug: an override that copies the
/// previous instance (see the module docs of `common::global_data`).
fn h_game_data_copying(ini: &mut Ini<'_>, store: &mut GlobalDataStore) -> IniResult<()> {
    if !store.is_empty() && ini.get_load_type() == IniLoadType::CreateOverrides {
        let previous = store.the_global_data().unwrap().clone();
        *store.new_override() = previous;
        let what = store.the_writable_global_data().unwrap();
        return ini.init_from_ini(what, &GLOBAL_DATA_FIELDS);
    }
    parse_game_data_definition(ini, store)
}

/// `parseGameDataDefinition` over another table: the planted-bug runs.
fn parse_with(ini: &mut Ini<'_>, store: &mut GlobalDataStore, table: &[FieldParse<GlobalData>]) -> IniResult<()> {
    if !store.is_empty() && ini.get_load_type() != IniLoadType::Multifile {
        if ini.get_load_type() == IniLoadType::CreateOverrides {
            store.new_override();
        }
    } else if store.is_empty() {
        store.create_original();
    }
    ini.init_from_ini(store.the_writable_global_data().unwrap(), table)
}

/// `GLOBAL_DATA_FIELDS` with the entry of `token` replaced.
fn planted_table(token: &str, parse: fn(&mut Ini<'_>, &mut GlobalData, &UserData) -> IniResult<()>) -> Vec<FieldParse<GlobalData>> {
    let mut t: Vec<FieldParse<GlobalData>> =
        GLOBAL_DATA_FIELDS.iter().map(|f| FieldParse { token: f.token, parse: f.parse, user_data: f.user_data.clone() }).collect();
    t.iter_mut().find(|f| f.token == Some(token)).expect("token").parse = Some(parse);
    t
}

static SKY_TABLE: OnceLock<Vec<FieldParse<GlobalData>>> = OnceLock::new();
static DELTA_TABLE: OnceLock<Vec<FieldParse<GlobalData>>> = OnceLock::new();
static BONES_TABLE: OnceLock<Vec<FieldParse<GlobalData>>> = OnceLock::new();

/// Planted: `DrawSkyBox` stored as a whole `Real` instead of one byte.
fn h_planted_sky(ini: &mut Ini<'_>, store: &mut GlobalDataStore) -> IniResult<()> {
    let t = SKY_TABLE.get_or_init(|| {
        planted_table("DrawSkyBox", |ini, g, _| {
            g.draw_sky_box = if ini.parse_bool()? { 1.0 } else { 0.0 };
            Ok(())
        })
    });
    parse_with(ini, store, t)
}
/// Planted: `FirewallPortAllocationDelta` saturates instead of keeping the low 16 bits.
fn h_planted_delta(ini: &mut Ini<'_>, store: &mut GlobalDataStore) -> IniResult<()> {
    let t = DELTA_TABLE.get_or_init(|| {
        planted_table("FirewallPortAllocationDelta", |ini, g, _| {
            g.firewall_port_allocation_delta = ini.parse_int()?.clamp(-32768, 32767) as i16;
            Ok(())
        })
    });
    parse_with(ini, store, t)
}
/// Planted: `StandardPublicBone` replaces its list instead of appending.
fn h_planted_bones(ini: &mut Ini<'_>, store: &mut GlobalDataStore) -> IniResult<()> {
    let t = BONES_TABLE.get_or_init(|| {
        planted_table("StandardPublicBone", |ini, g, _| {
            g.standard_public_bones.clear();
            ini.parse_ascii_string_vector_append(&mut g.standard_public_bones)
        })
    });
    parse_with(ini, store, t)
}

fn h_stub(_: &mut Ini<'_>, _: &mut GlobalDataStore) -> IniResult<()> {
    Err(IniError::new(IniErrorKind::Bug))
}

/// The block table: `GameData` is translated; the other 61 handlers throw
/// `ERROR_BUG`, as the stubs of wrap_ini.cpp do.
fn blocks(game_data: GameDataHandler) -> Vec<BlockParse<GlobalDataStore>> {
    TYPE_TABLE.iter().map(|&(token, _, _)| BlockParse { token, parse: if token == "GameData" { game_data } else { h_stub } }).collect()
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

fn gen_sep(g: &mut Gen) -> &'static str {
    pick(g, &[" = ", "=", " ", " =", "= ", " == ", " = ", " = "])
}

fn gen_int(g: &mut Gen, clean: bool) -> String {
    match g.below(40) {
        0..=17 => format!("{}", g.below(2000)),
        18..=24 => format!("{}", g.below(200) as i32 - 100),
        25..=27 => format!("{}", g.i32()),
        28 => pick(g, &["0", "-1", "1", "255", "256", "65535", "65536", "70000", "-70000", "2147483647", "-2147483648", "4294967295"]).to_string(),
        _ if clean => format!("{}", g.below(100)),
        29 => pick(g, &["x", "", "12abc", "1.5", "-", "+5", "0x10", " 7", "1e3"]).to_string(),
        30 if g.below(8) == 0 => pick(g, &["2147483648", "-2147483649", "99999999999", "1e99"]).to_string(),
        _ => format!("{}", g.below(100)),
    }
}

fn gen_real(g: &mut Gen, clean: bool) -> String {
    match g.below(40) {
        0..=14 => format!("{}", g.uniform(-1000.0, 1000.0)),
        15..=22 => format!("{}", g.below(1000)),
        23..=27 => format!("{}", g.uniform(-1.0, 1.0)),
        28 => pick(g, &["0", "-0", "1e3", ".5", "5.", "1e-5", "100000000", "-1.0", "0.5"]).to_string(),
        29..=31 => format!("{}", g.finite()),
        _ if clean => "2.5".to_string(),
        32 => pick(g, &["x", "", "1e", ".", "-", "12abc", "1.5.3"]).to_string(),
        33 if g.below(10) == 0 => pick(g, &["inf", "nan", "1e39", "0x1p3"]).to_string(),
        _ => format!("{}", g.uniform(-10.0, 10.0)),
    }
}

fn gen_percent(g: &mut Gen, clean: bool) -> String {
    match g.below(20) {
        0..=9 => format!("{}%", g.below(301)),
        10..=12 => format!("{}%", g.uniform(-50.0, 400.0)),
        13 => pick(g, &["0", "100", "50", "0%", "100%", "25.5%", "1e2"]).to_string(),
        _ if clean => "100%".to_string(),
        14 => pick(g, &["abc", "", "%", "5x", "100%x", ".", "--5%"]).to_string(),
        _ => format!("{}", g.below(200)),
    }
}

fn gen_bool(g: &mut Gen, clean: bool) -> String {
    match g.below(12) {
        0..=3 => case_mix(g, "Yes"),
        4..=7 => case_mix(g, "No"),
        _ if clean => "yes".to_string(),
        8 => pick(g, &["maybe", "", "1", "0", "true", "false", "yes!", "y"]).to_string(),
        _ => case_mix(g, "yes"),
    }
}

fn gen_name(g: &mut Gen) -> String {
    match g.below(12) {
        0..=5 => format!("Name{}", g.below(100)),
        6 => format!("Maps\\Dir{}\\File{}.map", g.below(9), g.below(9)),
        7 => format!("\"Quoted {}\"", g.below(9)),
        8 => format!("\"Quoted {}\" extra", g.below(9)),
        9 => pick(g, &["\"", "\"unterminated", "\"\"", "a\"b", "x y z", ""]).to_string(),
        _ => format!("{}Tok", pick(g, &["A", "b", "C_", "d.", "E-"])),
    }
}

fn gen_colour_part(g: &mut Gen, clean: bool, comp: &str) -> String {
    let v = match g.below(24) {
        0..=17 => g.below(256) as i32,
        18 => 0,
        19 => 255,
        _ if clean => 128,
        20 => 256,
        21 => -1,
        22 => 1000,
        _ => g.below(300) as i32,
    };
    format!("{comp}:{v}")
}

fn gen_rgb(g: &mut Gen, clean: bool) -> String {
    let mut parts = vec![gen_colour_part(g, clean, "R"), gen_colour_part(g, clean, "G"), gen_colour_part(g, clean, "B")];
    if !clean {
        match g.below(16) {
            0 => {
                parts.pop();
            }
            1 => parts.swap(0, 1),
            2 => parts[1] = "G:x".to_string(),
            3 => parts.push("A:5".to_string()),
            _ => {}
        }
    }
    parts.join(" ")
}

fn gen_color_int(g: &mut Gen, clean: bool) -> String {
    let mut s = gen_rgb(g, clean);
    if g.below(2) == 0 {
        s.push(' ');
        s.push_str(&gen_colour_part(g, clean, "A"));
    }
    s
}

fn gen_coord(g: &mut Gen, clean: bool, axes: &[&str]) -> String {
    let mut parts: Vec<String> = axes.iter().map(|a| format!("{a}:{}", gen_real(g, clean))).collect();
    if !clean {
        match g.below(16) {
            0 => {
                parts.pop();
            }
            1 => parts.swap(0, 1),
            2 => parts[0] = parts[0].replace(':', " "),
            _ => {}
        }
    }
    parts.join(" ")
}

fn gen_index(g: &mut Gen, names: &[&str], clean: bool) -> String {
    if !clean && g.below(10) == 0 {
        return pick(g, &["BOGUS", "", "0", "-1", "NONEE", "MORNING2"]).to_string();
    }
    let i = g.below(names.len() as u32) as usize;
    case_mix(g, names[i])
}

fn gen_weapon_bonus(g: &mut Gen, clean: bool) -> String {
    let ci = g.below(WEAPONBONUSCONDITION_COUNT as u32) as usize;
    let c = case_mix(g, WEAPON_BONUS_NAMES[ci]);
    let fi = g.below(WEAPON_BONUS_FIELD_COUNT as u32) as usize;
    let f = case_mix(g, WEAPON_BONUS_FIELD_NAMES[fi]);
    let p = gen_percent(g, clean);
    if !clean {
        match g.below(14) {
            0 => return format!("{c} {f}"),
            1 => return c,
            2 => return format!("NOPE {f} {p}"),
            3 => return format!("{c} NOPE {p}"),
            4 => return format!("{c} {f} {p} extra"),
            _ => {}
        }
    }
    format!("{c} {f} {p}")
}

fn index_names(list: &str) -> &'static [&'static str] {
    match list {
        "TerrainLODNames" => &TERRAIN_LOD_NAMES,
        "TimeOfDayNames" => &TIME_OF_DAY_NAMES,
        "WeatherNames" => &WEATHER_NAMES,
        "TheBodyDamageTypeNames" => &BODY_DAMAGE_TYPE_NAMES,
        _ => unreachable!("{list}"),
    }
}

fn gen_value(g: &mut Gen, kind: &str, clean: bool) -> String {
    match kind {
        "bool" => gen_bool(g, clean),
        "int" => gen_int(g, clean),
        "ubyte" => match g.below(12) {
            0..=7 => format!("{}", g.below(256)),
            8 if !clean => pick(g, &["256", "-1", "300", "x", ""]).to_string(),
            _ => format!("{}", g.below(256)),
        },
        "real" | "angle" | "accel" => gen_real(g, clean),
        "percent" => gen_percent(g, clean),
        "duration" | "money" => match g.below(14) {
            0..=7 => format!("{}", g.below(5000)),
            8..=9 => format!("{}", g.i32() as u32),
            10 if !clean => pick(g, &["-1", "x", "", "4294967295", "1e3", "-0"]).to_string(),
            _ => format!("{}", g.below(100000)),
        },
        "color" => gen_color_int(g, clean),
        "string" => gen_name(g),
        "strings" => {
            let n = g.below(4);
            (0..n).map(|_| format!("Bone{}", g.below(9))).collect::<Vec<_>>().join(" ")
        }
        "rgb" => gen_rgb(g, clean),
        "coord3d" => gen_coord(g, clean, &["X", "Y", "Z"]),
        "coord2d" => gen_coord(g, clean, &["X", "Y"]),
        "weaponbonus" => gen_weapon_bonus(g, clean),
        k if k.starts_with("index:") => gen_index(g, index_names(&k[6..]), clean),
        k => unreachable!("{k}"),
    }
}

/// Tokens that the retail table lacks: debug-only members, the commented-out
/// ones, the prison bounty (`ALLOW_SURRENDER`), and a lower-case spelling.
const NOT_IN_RETAIL: [&str; 10] = [
    "Wireframe", "ShroudOn", "DebugProjectileTileWidth", "SaveStats", "HealthBonus_Regular", "DoubleClickTimeMS", "PrisonBountyMultiplier",
    "windowed", "TerrainLightingNightAmbient4", "InGame",
];

/// One field line; `kinds` lists the tokens drawn. Returns the line and its token.
fn gen_field_line(g: &mut Gen, clean: bool, used: &mut Vec<&'static str>) -> String {
    let indent = pick(g, &["", "  ", "\t", " \t "]);
    let comment = if g.below(10) == 0 { pick(g, &[" ; comment", ";", " ;x=1 2"]) } else { "" };
    if !clean && g.below(30) == 0 {
        let t = pick(g, &NOT_IN_RETAIL);
        return format!("{indent}{t}{}yes{comment}", gen_sep(g));
    }
    let (token, kind) = FIELD_KINDS[g.below(FIELD_KINDS.len() as u32) as usize];
    used.push(token);
    let token = if !clean && g.below(60) == 0 { case_mix(g, token) } else { token.to_string() };
    let value = gen_value(g, kind, clean);
    format!("{indent}{token}{}{value}{comment}", gen_sep(g))
}

fn gen_game_data_block(g: &mut Gen, lines: &mut Vec<String>, clean: bool, used: &mut Vec<&'static str>) {
    let head = if !clean && g.below(12) == 0 { pick(g, &["GameData Extra", "GameData ; c", "GameData\t"]) } else { "GameData" };
    lines.push(head.to_string());
    let n = if g.below(4) == 0 { g.below(120) } else { 1 + g.below(28) };
    for _ in 0..n {
        lines.push(gen_field_line(g, clean, used));
    }
    if clean || g.below(18) != 0 {
        lines.push(if clean { "End".to_string() } else { pick(g, &["End", "END", "end", "  End  ", "End ; done", "Ends", "=End"]).to_string() });
    }
}

/// One file's bytes and the tokens it names (for the coverage count).
fn gen_file(g: &mut Gen) -> (Vec<u8>, Vec<&'static str>) {
    // 45% of the files draw only well-formed values and always end their blocks
    let clean = g.below(100) < 45;
    let mut lines = Vec::new();
    let mut used = Vec::new();
    for _ in 0..1 + g.below(3) {
        match g.below(14) {
            0 => lines.push(pick(g, &["; a comment", "", "   ", ";", "\t"]).to_string()),
            1 if !clean => lines.push(pick(g, &["Unknown block", "=", "  =  ", "End", "gamedata", "AIData X", "Video X"]).to_string()),
            _ => gen_game_data_block(g, &mut lines, clean, &mut used),
        }
    }
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        out.extend_from_slice(l.as_bytes());
        let last = i + 1 == lines.len();
        if !(last && g.below(4) == 0) {
            out.extend_from_slice(if g.below(8) == 0 { b"\r\n" } else { b"\n" });
        }
    }
    match if clean { 99 } else { g.below(16) } {
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
    (out, used)
}

// ---------------------------------------------------------------------------
// A case: steps run on both sides
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Action {
    /// `TheWritableGlobalData = NEW GlobalData` if there is none.
    Create,
    /// `reset()`.
    Reset,
    /// `setTimeOfDay(tod)`, with a chain present.
    SetTimeOfDay(i32),
    /// Load file `n` of the case with this load type.
    Load { file: usize, load_type: IniLoadType, with_xfer: bool },
}

struct Case {
    files: Vec<(Vec<u8>, Vec<&'static str>)>,
    actions: Vec<Action>,
}

fn gen_case(g: &mut Gen) -> Case {
    let n_files = 1 + g.below(3) as usize;
    let files: Vec<_> = (0..n_files).map(|_| gen_file(g)).collect();
    let mut actions = Vec::new();
    if g.below(10) < 4 {
        actions.push(Action::Create);
    }
    for file in 0..n_files {
        let load_type = match g.below(10) {
            0..=4 => IniLoadType::Overwrite,
            5..=7 => IniLoadType::CreateOverrides,
            _ => IniLoadType::Multifile,
        };
        actions.push(Action::Load { file, load_type, with_xfer: g.below(4) != 0 });
        match g.below(20) {
            0..=2 => actions.push(Action::Reset),
            3..=4 => actions.push(Action::SetTimeOfDay(g.below(8) as i32 - 2)),
            _ => {}
        }
    }
    Case { files, actions }
}

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

fn cpp_load(name: &[u8], load_type: IniLoadType, with_xfer: bool) -> Outcome {
    let cname = CString::new(name.to_vec()).unwrap();
    let mut msg = vec![0u8; 4096];
    let (mut crc, mut code) = (0u32, 0u32);
    let status = unsafe {
        cpp::ref_globaldata_load(cname.as_ptr(), load_type as i32, i32::from(with_xfer), &mut crc, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
    };
    let detail = match status {
        0 | 4 => Vec::new(),
        1 => msg[..code as usize].to_vec(),
        2 | 3 => code.to_le_bytes().to_vec(),
        _ => panic!("C++ threw something unexpected (status {status})"),
    };
    Outcome { status, detail, crc: if with_xfer { crc } else { 0 } }
}

/// What one action produced on one side.
#[derive(Debug)]
struct Step {
    /// The load's throw (None for the other actions) or setTimeOfDay's Bool.
    outcome: Option<Outcome>,
    flag: Option<bool>,
    dump: Vec<u8>,
    /// Where each member begins in a Rust dump (empty for the C++).
    marks: Vec<(usize, String)>,
}

fn file_name(i: usize) -> Vec<u8> {
    format!("Data\\INI\\GameData{i}.ini").into_bytes()
}

/// The Rust side of a whole case. `Err` is an input upstream leaves undefined.
fn run_rust(case: &Case, game_data: GameDataHandler) -> Result<Vec<Step>, IniError> {
    let table = blocks(game_data);
    let mut store = GlobalDataStore::new();
    let mut steps = Vec::new();
    for action in &case.actions {
        let (mut outcome, mut flag) = (None, None);
        match action {
            Action::Create => {
                store.create_original();
            }
            Action::Reset => store.reset(),
            Action::SetTimeOfDay(tod) => {
                if let Some(g) = store.the_writable_global_data() {
                    flag = Some(g.set_time_of_day(*tod));
                }
            }
            Action::Load { file, load_type, with_xfer } => {
                let mut crc = XferCrc::new_without_nan_guard();
                let r = Ini::load(
                    &file_name(*file),
                    &case.files[*file].0,
                    *load_type,
                    if *with_xfer { Some(&mut crc as &mut dyn Xfer) } else { None },
                    &table,
                    &mut store,
                );
                outcome = Some(rust_outcome(r, if *with_xfer { crc.get_crc() } else { 0 })?);
            }
        }
        let (dump, marks) = rust_dump_marked(&store);
        steps.push(Step { outcome, flag, dump, marks });
    }
    Ok(steps)
}

/// The C++ side of the same case.
fn run_cpp(case: &Case) -> Vec<Step> {
    unsafe {
        cpp::ref_globaldata_clear();
        cpp::ref_ini_reset();
    }
    for (i, (data, _)) in case.files.iter().enumerate() {
        let n = CString::new(file_name(i)).unwrap();
        unsafe { cpp::ref_ini_add_file(n.as_ptr(), data.as_ptr() as *const _, data.len() as i32) };
    }
    let mut steps = Vec::new();
    for action in &case.actions {
        let (mut outcome, mut flag) = (None, None);
        match action {
            Action::Create => unsafe { cpp::ref_globaldata_create() },
            Action::Reset => unsafe { cpp::ref_globaldata_reset() },
            Action::SetTimeOfDay(tod) => {
                if unsafe { cpp::ref_globaldata_len() } > 0 {
                    flag = Some(unsafe { cpp::ref_globaldata_set_time_of_day(*tod) } != 0);
                }
            }
            Action::Load { file, load_type, with_xfer } => outcome = Some(cpp_load(&file_name(*file), *load_type, *with_xfer)),
        }
        steps.push(Step { outcome, flag, dump: cpp_dump(), marks: Vec::new() });
    }
    unsafe { cpp::ref_globaldata_clear() };
    steps
}

fn show(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn describe(case: &Case, k: usize) -> String {
    let file = match &case.actions[k] {
        Action::Load { file, load_type, with_xfer } => format!("{:?} xfer {with_xfer} file {:?}", load_type, show(&case.files[*file].0)),
        a => format!("{a:?}"),
    };
    format!("action {k} ({file}) of {:?}", case.actions.iter().map(|a| match a {
        Action::Create => "create".to_string(),
        Action::Reset => "reset".to_string(),
        Action::SetTimeOfDay(t) => format!("tod{t}"),
        Action::Load { file, load_type, .. } => format!("load{file}:{load_type:?}"),
    }).collect::<Vec<_>>())
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

// ---------------------------------------------------------------------------
// The tests
// ---------------------------------------------------------------------------

#[test]
fn game_data_table_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let rust = GlobalData::field_tokens();
    let n = unsafe { cpp::ref_globaldata_field_count() } as usize;
    let cpp_tokens: Vec<String> = (0..n)
        .map(|i| unsafe { std::ffi::CStr::from_ptr(cpp::ref_globaldata_field_token(i as i32)) }.to_str().unwrap().to_string())
        .collect();
    let mut tokens = Check::new("GameData table (token and position)");
    tokens.eq(rust.len(), cpp_tokens.len(), || "length".to_string());
    for (i, (r, c)) in rust.iter().zip(cpp_tokens.iter()).enumerate() {
        tokens.eq(r.to_string(), c.clone(), || format!("entry {i}"));
    }
    println!("GlobalData table: {} fields (upstream {})", rust.len(), cpp_tokens.len());
    assert_eq!(n, 336);
    assert_eq!(FIELD_KINDS.len(), 336);
    for (i, (t, _)) in FIELD_KINDS.iter().enumerate() {
        assert_eq!(*t, rust[i], "the kinds list follows the table");
    }
    tokens.finish();
}

#[test]
fn constructor_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut defaults = Check::new("GlobalData() (every member)");
    let mut wrong_default = Check::new("GlobalData() (every member)");
    unsafe {
        cpp::ref_globaldata_clear();
        cpp::ref_ini_reset();
        cpp::ref_globaldata_create();
    }
    let cpp_bytes = cpp_dump();
    unsafe { cpp::ref_globaldata_clear() };
    let mut store = GlobalDataStore::new();
    store.create_original();
    let rust_bytes = rust_dump(&store);
    assert!(rust_bytes.len() > 2000, "{} bytes", rust_bytes.len());
    defaults.eq(rust_bytes.len(), cpp_bytes.len(), || "dump length".to_string());
    for (i, (r, c)) in rust_bytes.iter().zip(cpp_bytes.iter()).enumerate() {
        defaults.eq(*r, *c, || format!("byte {i}"));
    }
    // planted: one default is wrong (the sky box scale)
    let mut wrong = GlobalDataStore::new();
    wrong.create_original().sky_box_scale = 4.0;
    let wrong_bytes = rust_dump(&wrong);
    for (i, (r, c)) in wrong_bytes.iter().zip(cpp_bytes.iter()).enumerate() {
        wrong_default.eq(*r, *c, || format!("byte {i}"));
    }
    // the same through a second construction: every instance is the same
    unsafe {
        cpp::ref_ini_reset();
        cpp::ref_globaldata_create();
    }
    assert_eq!(cpp_dump(), cpp_bytes, "constructing twice gives the same members");
    unsafe { cpp::ref_globaldata_clear() };
    println!("GlobalData(): {} bytes (every member of one instance) compared", rust_bytes.len() - 4);
    defaults.finish();
    wrong_default.finish_planted();
}

#[test]
fn game_data_blocks_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x61DA_0001);
    let mut outcome = Check::new("GameData INI::load (throw, CRC)");
    let mut chain = Check::new("GlobalData chain (length, every member)");
    let mut flags = Check::new("setTimeOfDay (result)");
    let mut copying = Check::new("GlobalData chain (planted: override copies)");
    let mut planted_fields: Vec<(Check, GameDataHandler)> = vec![
        (Check::new("GlobalData chain (planted: DrawSkyBox whole Real)"), h_planted_sky),
        (Check::new("GlobalData chain (planted: delta saturates)"), h_planted_delta),
        (Check::new("GlobalData chain (planted: bones replace)"), h_planted_bones),
    ];
    let mut excluded = Excluded::default();
    let mut accepted: BTreeMap<&'static str, u64> = FIELD_KINDS.iter().map(|(t, _)| (*t, 0)).collect();
    let (mut loads, mut ok, mut failed, mut overrides, mut max_chain) = (0u64, 0u64, 0u64, 0u64, 0usize);
    let (mut resets, mut tods, mut field_lines) = (0u64, 0u64, 0u64);
    const CASES: usize = 6_000;
    for case_no in 0..CASES {
        let case = gen_case(&mut g);
        let rust = match run_rust(&case, h_game_data) {
            Ok(steps) => steps,
            Err(e) => {
                excluded.add(&e);
                continue;
            }
        };
        if std::env::var_os("GD_TRACE").is_some() {
            eprintln!("case {case_no}: {}", describe(&case, 0));
        }
        let cpp_steps = run_cpp(&case);
        assert_eq!(rust.len(), cpp_steps.len());
        let mut planted = None;
        for (k, (r, c)) in rust.iter().zip(cpp_steps.iter()).enumerate() {
            let input = || format!("case {case_no}: {}", describe(&case, k));
            outcome.eq(&r.outcome, &c.outcome, input);
            flags.eq(r.flag, c.flag, input);
            chain.eq(r.dump.len(), c.dump.len(), || format!("{} (dump length)", input()));
            if r.dump != c.dump {
                chain.eq(false, true, || format!("{} ({})", input(), first_difference(&r.dump, &c.dump, &r.marks)));
            } else {
                chain.eq(true, true, input);
            }
            if let Some(o) = &r.outcome {
                loads += 1;
                if o.status == 0 {
                    ok += 1;
                    if let Action::Load { file, .. } = &case.actions[k] {
                        for t in &case.files[*file].1 {
                            *accepted.get_mut(t).unwrap() += 1;
                        }
                    }
                } else {
                    failed += 1;
                }
                if let Action::Load { load_type: IniLoadType::CreateOverrides, .. } = &case.actions[k] {
                    overrides += 1;
                }
            }
            match &case.actions[k] {
                Action::Reset => resets += 1,
                Action::SetTimeOfDay(_) if r.flag.is_some() => tods += 1,
                _ => {}
            }
            max_chain = max_chain.max(u32::from_le_bytes(r.dump[..4].try_into().unwrap()) as usize);
        }
        for f in &case.files {
            field_lines += f.1.len() as u64;
        }
        // planted: an override that copies the previous instance. Only
        // cases that make an override with data in front of it can tell.
        if case.actions.iter().any(|a| matches!(a, Action::Load { load_type: IniLoadType::CreateOverrides, .. })) {
            if let Ok(wrong) = run_rust(&case, h_game_data_copying) {
                planted = Some(wrong);
            }
        }
        if let Some(wrong) = planted {
            for (k, (w, c)) in wrong.iter().zip(cpp_steps.iter()).enumerate() {
                copying.eq(&w.dump, &c.dump, || format!("case {case_no}: {}", describe(&case, k)));
            }
        }
        for (check, handler) in planted_fields.iter_mut() {
            if let Ok(wrong) = run_rust(&case, *handler) {
                for (k, (w, c)) in wrong.iter().zip(cpp_steps.iter()).enumerate() {
                    check.eq(&w.dump, &c.dump, || format!("case {case_no}: {}", describe(&case, k)));
                }
            }
        }
    }
    println!(
        "GlobalData INI: {CASES} cases, {loads} loads ({ok} loaded, {failed} threw, {overrides} created overrides), {resets} resets, {tods} setTimeOfDay, \
         {field_lines} field lines, longest chain {max_chain}; excluded cases (undefined upstream): {} {:?}",
        excluded.total(),
        excluded.0
    );
    let least = accepted.iter().min_by_key(|(_, n)| **n).unwrap();
    let mean = accepted.values().sum::<u64>() / accepted.len() as u64;
    println!("GlobalData INI: every one of the {} tokens named by a file that loaded: least {} ({}), mean {mean}", accepted.len(), least.1, least.0);
    assert!(ok > 4000 && failed > 2000, "the generator must reach both outcomes: {ok} loaded, {failed} threw");
    assert!(overrides > 1500 && resets > 500 && tods > 200 && max_chain >= 4);
    assert!(excluded.total() * 5 < CASES as u64, "too many cases excluded");
    assert!(*least.1 >= 100, "every field must be reached: {} was named {} times in files that loaded", least.0, least.1);
    outcome.finish();
    flags.finish();
    chain.finish();
    copying.finish_planted();
    for (check, _) in planted_fields {
        check.finish_planted();
    }
}

// ---------------------------------------------------------------------------
// setTimeOfDay and appendBonuses
// ---------------------------------------------------------------------------

/// A clean file that sets every light of every time of day, and many
/// `WeaponBonus` lines.
fn lighting_and_bonus_file(g: &mut Gen) -> Vec<u8> {
    let mut lines = vec!["GameData".to_string()];
    for set in ["TerrainLighting", "TerrainObjectsLighting"] {
        for tod in ["Morning", "Afternoon", "Evening", "Night"] {
            for n in ["", "2", "3"] {
                lines.push(format!("{set}{tod}Ambient{n} = {}", gen_rgb(g, true)));
                lines.push(format!("{set}{tod}Diffuse{n} = {}", gen_rgb(g, true)));
                lines.push(format!("{set}{tod}LightPos{n} = {}", gen_coord(g, true, &["X", "Y", "Z"])));
            }
        }
    }
    for _ in 0..200 {
        lines.push(format!("WeaponBonus = {}", gen_weapon_bonus(g, true)));
    }
    lines.push("End".to_string());
    (lines.join("\n") + "\n").into_bytes()
}

#[test]
fn set_time_of_day_and_append_bonuses_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x61DA_0003);
    let mut tod = Check::new("setTimeOfDay (result, members)");
    let mut bonuses = Check::new("WeaponBonusSet::appendBonuses");
    let mut no_zero_shortcut = Check::new("WeaponBonusSet::appendBonuses");
    let (mut valid, mut invalid, mut queries) = (0u64, 0u64, 0u64);
    for round in 0..40 {
        let data = lighting_and_bonus_file(&mut g);
        let case = Case {
            files: vec![(data, Vec::new())],
            actions: vec![Action::Load { file: 0, load_type: IniLoadType::Overwrite, with_xfer: false }],
        };
        let rust_steps = run_rust(&case, h_game_data).expect("a clean file loads");
        assert_eq!(rust_steps[0].outcome.as_ref().unwrap().status, 0, "a clean file loads");
        let cpp_steps = run_cpp(&case);
        assert_eq!((&rust_steps[0].outcome, &rust_steps[0].dump), (&cpp_steps[0].outcome, &cpp_steps[0].dump), "round {round}: load");
        // the same data on both sides, kept alive for the queries
        let mut store = GlobalDataStore::new();
        Ini::load(b"x.ini", &case.files[0].0, IniLoadType::Overwrite, None, &blocks(h_game_data), &mut store).unwrap();
        unsafe {
            cpp::ref_globaldata_clear();
            cpp::ref_ini_reset();
            let n = CString::new(file_name(0)).unwrap();
            cpp::ref_ini_add_file(n.as_ptr(), case.files[0].0.as_ptr() as *const _, case.files[0].0.len() as i32);
        }
        let c = cpp_load(&file_name(0), IniLoadType::Overwrite, false);
        assert_eq!(c.status, 0);
        for q in 0..300 {
            let t = g.below(11) as i32 - 3;
            let flag = store.the_writable_global_data().unwrap().set_time_of_day(t);
            let cflag = unsafe { cpp::ref_globaldata_set_time_of_day(t) } != 0;
            if flag { valid += 1 } else { invalid += 1 }
            tod.eq(flag, cflag, || format!("round {round} query {q}: setTimeOfDay({t})"));
            tod.eq(rust_dump(&store), cpp_dump(), || format!("round {round} query {q}: members after setTimeOfDay({t})"));
            let flags = match g.below(6) {
                0 => 0,
                1 => 1 << g.below(32),
                2 => g.u32() & ((1 << WEAPONBONUSCONDITION_COUNT) - 1),
                _ => g.u32(),
            };
            let set = store.the_global_data().unwrap().weapon_bonus_set;
            let mut b = WeaponBonus::new();
            set.append_bonuses(flags, &mut b);
            let mut out = [0u32; 5];
            unsafe { cpp::ref_globaldata_append_bonuses(flags, out.as_mut_ptr()) };
            let rust_bits: Vec<u32> = (0..5).map(|f| b.get_field(f).to_bits()).collect();
            bonuses.eq(rust_bits.clone(), out.to_vec(), || format!("round {round} query {q}: flags {flags:#010x}"));
            // planted: flags of 0 are not special, and the condition bits are taken from the top
            let mut w = WeaponBonus::new();
            for i in 0..WEAPONBONUSCONDITION_COUNT {
                if flags & (1 << (WEAPONBONUSCONDITION_COUNT - 1 - i)) != 0 {
                    set.bonus(i).append_bonuses(&mut w);
                }
            }
            no_zero_shortcut.eq((0..5).map(|f| w.get_field(f).to_bits()).collect::<Vec<_>>(), out.to_vec(), || format!("flags {flags:#010x}"));
            queries += 1;
        }
        unsafe { cpp::ref_globaldata_clear() };
    }
    println!("setTimeOfDay: {valid} valid and {invalid} out of range values; appendBonuses: {queries} queries");
    assert!(valid > 3000 && invalid > 3000);
    tod.finish();
    bonuses.finish();
    no_zero_shortcut.finish_planted();
}
