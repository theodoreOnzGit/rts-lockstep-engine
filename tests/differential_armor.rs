// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of Phase 1 layer 4's `Armor`, `Damage` and
//! `DamageFX` (`logic::armor`, `logic::damage`, `common::damage_fx`) against
//! upstream `Armor.cpp`, `Damage.cpp`, `DamageFX.cpp` and `INIDamageFX.cpp`
//! (commit 0a05454d), compiled as `differential/wrap_armor.cpp` describes,
//! with the real `INI::load`, `NameKeyGenerator` and `XferSave`/`XferLoad`/
//! `XferCRC` underneath.
//!
//! No game INI files are available, so every file is **synthetic**: `Armor`
//! and `DamageFX` blocks (repeated names, names differing only in case, every
//! field in its several forms, `Default` and single damage types, every
//! veterancy level, `None` and named `FXList`s, percentages and durations at
//! and beyond their usual range and malformed ones, missing tokens, unknown
//! fields, unknown blocks, a missing `End`, comments, `CRLF`, and in a share of
//! cases random byte damage to the whole file).
//!
//! 1. `INI blocks`: 8,000 files fed identically to both sides. Compared: what
//!    `INI::load` throws (nothing, an `INIException` and its whole message, an
//!    `INI_*` status code, an `ErrorCode`), the INI `XferCRC`, **every field
//!    of both stores** (all 38 coefficients of each `ArmorTemplate`, bitwise;
//!    for each `DamageFX` all 38 x 4 entries: threshold bitwise, throttle
//!    time, major and minor `FXList` name), also when the load threw, and the
//!    keys `nameToKey` then gives to a list of names (the key assignment order
//!    is lockstep state).
//! 2. `Armor::adjustDamage`, through both the `Armor` handle and the
//!    template: 40,000 queries on loaded templates, damage from the mixed
//!    generator (NaN, infinities, negatives, subnormals), floats bitwise.
//! 3. `DamageFX` getters (throttle time, `getDamageFXList`, and `doDamageFX`
//!    reaching the same effect): 40,000 queries on loaded `DamageFX`es, with
//!    and without a source object, at every veterancy level.
//! 4. `DamageInfo`, `DamageInfoInput` and `DamageInfoOutput` xfer: 6,000
//!    random infos saved (bytes and status compared), loaded back, loaded from
//!    mutated files (version byte, a random byte, short files, long files)
//!    into random starting infos (all fields and the status compared), and
//!    run through `XferCRC`.
//! 5. `IsSubdualDamage`, `IsHealthDamagingDamage`, `DAMAGE_TYPE_FLAGS_ALL`/
//!    `NONE` and the constructors' defaults.
//!
//! Inputs for which the Rust reports undefined behaviour upstream (an unknown
//! damage name used as an array index, a damage type out of range, see the
//! module docs of `logic::armor` and `common::damage_fx`) are **not** given
//! to the C++; their count is printed. The `FXList` stand-in of the C++ side
//! returns a distinct pointer for every name but `None` (see the wrapper).
//!
//! Each comparison also runs a planted bug, which must be caught.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::damage_fx::DamageFxStore;
use rts_lockstep_engine::common::game_common::{VeterancyLevel, VETERANCY_NAMES};
use rts_lockstep_engine::common::ini::{
    BlockParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Thrown, TYPE_TABLE,
};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::xfer::{Snapshot, Xfer, XferStatus};
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::armor::{ArmorStore, DamageTypeOutOfRange};
use rts_lockstep_engine::logic::damage::{
    is_health_damaging_damage, is_subdual_damage, DamageInfo, DamageInfoInput, DamageInfoOutput,
    DamageTypeFlags, DAMAGE_NUM_TYPES, DAMAGE_SUBDUAL_UNRESISTABLE, DAMAGE_TYPE_NAMES,
    DAMAGE_UNRESISTABLE,
};
use std::collections::BTreeMap;
use std::ffi::CString;
use support::{Check, Gen};

/// The C++ side has process-global state (the name keys, the stores, `strtok`,
/// `XferLoad`'s static buffer), so the tests take turns.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// ---------------------------------------------------------------------------
// The Rust side of the INI blocks
// ---------------------------------------------------------------------------

struct Ctx {
    armor: ArmorStore,
    fx: DamageFxStore,
    names: NameKeyGenerator,
}

impl Ctx {
    fn new() -> Self {
        let mut names = NameKeyGenerator::new();
        names.reset();
        Ctx {
            armor: ArmorStore::new(),
            fx: DamageFxStore::new(),
            names,
        }
    }
}

fn h_armor(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.armor.parse_armor_definition(ini, &mut c.names)
}
fn h_fx(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.fx.parse_damage_fx_definition(ini, &mut c.names)
}
fn h_stub(_: &mut Ini<'_>, _: &mut Ctx) -> IniResult<()> {
    Err(IniError::new(IniErrorKind::Bug))
}

/// The block table: `Armor` and `DamageFX` are translated; the other 60
/// handlers throw `ERROR_BUG`, as the stubs of wrap_ini.cpp do.
fn blocks() -> Vec<BlockParse<Ctx>> {
    TYPE_TABLE
        .iter()
        .map(|&(token, _, _)| BlockParse {
            token,
            parse: match token {
                "Armor" => h_armor,
                "DamageFX" => h_fx,
                _ => h_stub,
            },
        })
        .collect()
}

fn put_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// The dump of wrap_armor.cpp's `ref_armor_dump`.
fn rust_armor_dump(s: &ArmorStore) -> Vec<u8> {
    let mut v = Vec::new();
    put_u32(&mut v, s.len() as u32);
    for (k, t) in s.templates() {
        put_u32(&mut v, k.0);
        for c in t.coefficients() {
            put_u32(&mut v, c.to_bits());
        }
    }
    v
}

fn put_fx(v: &mut Vec<u8>, fx: &Option<Vec<u8>>) {
    match fx {
        None => v.push(0),
        Some(n) => {
            v.push(1);
            put_u32(v, n.len() as u32);
            v.extend_from_slice(n);
        }
    }
}

/// The dump of wrap_armor.cpp's `ref_dfx_dump`.
fn rust_dfx_dump(s: &DamageFxStore) -> Vec<u8> {
    let mut v = Vec::new();
    put_u32(&mut v, s.len() as u32);
    for (k, d) in s.damage_fxs() {
        put_u32(&mut v, k.0);
        for dt in 0..DAMAGE_NUM_TYPES as i32 {
            for level in LEVELS {
                let e = d.dfx(dt, level).unwrap();
                put_u32(&mut v, e.amount_for_major_fx.to_bits());
                put_u32(&mut v, e.damage_fx_throttle_time);
                put_fx(&mut v, &e.major_damage_fx_list);
                put_fx(&mut v, &e.minor_damage_fx_list);
            }
        }
    }
    v
}

const LEVELS: [VeterancyLevel; 4] = [
    VeterancyLevel::Regular,
    VeterancyLevel::Veteran,
    VeterancyLevel::Elite,
    VeterancyLevel::Heroic,
];

fn cpp_dump(f: unsafe extern "C" fn(*mut u8, i32) -> i32) -> Vec<u8> {
    let mut buf = vec![0u8; 1 << 21];
    let n = unsafe { f(buf.as_mut_ptr(), buf.len() as i32) };
    assert!((n as usize) <= buf.len(), "dump overflow");
    buf.truncate(n as usize);
    buf
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

const BLOCK_NAMES: [&str; 8] = [
    "Tank",
    "tank",
    "Infantry",
    "A",
    "B",
    "NoDamage",
    "Heavy_Armor",
    "X1",
];
const FX_NAMES: [&str; 7] = [
    "FX_Small",
    "FX_Big",
    "fx_small",
    "FX_Explosion",
    "FX_Fire",
    "FX_Spark",
    "FX_Dust",
];

fn gen_damage(g: &mut Gen, clean: bool) -> String {
    match g.below(20) {
        0..=13 => {
            let i = g.below(DAMAGE_NUM_TYPES as u32) as usize;
            case_mix(g, DAMAGE_TYPE_NAMES[i])
        }
        14..=17 => pick(g, &["Default", "DEFAULT", "default"]).to_string(),
        _ if clean => "Default".to_string(),
        18 => pick(
            g,
            &[
                "BOGUS",
                "KILLPILOT",
                "EXPLOSION2",
                "Defaults",
                "",
                "NONE",
                "0",
                "-1",
            ],
        )
        .to_string(),
        _ => {
            let i = g.below(DAMAGE_NUM_TYPES as u32) as usize;
            case_mix(g, DAMAGE_TYPE_NAMES[i])
        }
    }
}

fn gen_vet(g: &mut Gen, clean: bool) -> String {
    if !clean && g.below(12) == 0 {
        return pick(g, &["Heroes", "", "0", "ELITES", "REGULARS"]).to_string();
    }
    let i = g.below(4) as usize;
    case_mix(g, VETERANCY_NAMES[i])
}

fn gen_percent(g: &mut Gen, clean: bool) -> String {
    match g.below(24) {
        0..=7 => format!("{}%", g.below(301)),
        8..=11 => format!("{}%", g.uniform(-50.0, 400.0)),
        12 => pick(
            g,
            &[
                "0", "100", "50", "0%", "100%", "25.5%", "1e2", "-100%", "1e10%", "3.4e38",
            ],
        )
        .to_string(),
        13 => format!("{}", g.uniform(0.0, 300.0)),
        _ if clean => "100%".to_string(),
        14 => pick(g, &["abc", "", "%", "5x", "100%x", ".", "1e", "-", "--5%"]).to_string(),
        15 => format!("{}%", g.mixed()),
        _ => format!("{}%", g.below(150)),
    }
}

fn gen_real(g: &mut Gen, clean: bool) -> String {
    match g.below(20) {
        0..=9 => format!("{}", g.uniform(-100.0, 1000.0)),
        10..=13 => format!("{}", g.below(2000)),
        14 => pick(g, &["0", "-0", "1e3", ".5", "5.", "1e-5", "100000000"]).to_string(),
        _ if clean => "10".to_string(),
        15 => pick(g, &["x", "", "1e", ".", "-", "12abc"]).to_string(),
        _ => format!("{}", g.mixed()),
    }
}

fn gen_ms(g: &mut Gen, clean: bool) -> String {
    match g.below(20) {
        0..=9 => format!("{}", g.below(5000)),
        10..=12 => format!("{}", g.uniform(0.0, 3000.0)),
        13 => pick(
            g,
            &["0", "1", "33", "34", "16.7", "1000", "100000", "0.4", "0.5"],
        )
        .to_string(),
        _ if clean => "1000".to_string(),
        14 => pick(g, &["x", "", "-5", "1e", "12abc", "-0.2"]).to_string(),
        _ => format!("{}", g.mixed()),
    }
}

fn gen_fx(g: &mut Gen) -> String {
    match g.below(8) {
        0 => pick(g, &["None", "NONE", "none", "nOnE"]).to_string(),
        _ => pick(g, &FX_NAMES).to_string(),
    }
}

fn gen_sep(g: &mut Gen) -> &'static str {
    pick(g, &[" = ", "=", " ", " =", "= ", " == ", " = "])
}

fn gen_armor_line(g: &mut Gen, clean: bool) -> String {
    let indent = pick(g, &["", "  ", "\t", " \t "]);
    let body = match g.below(30) {
        0 if !clean => pick(
            g,
            &[
                "Foo = 1",
                "Armor",
                "Armor =",
                "Armor = FLAME",
                "Armor = FLAME 50% 60%",
                "armor = FLAME 50%",
                "Dead = 1",
            ],
        )
        .to_string(),
        _ => format!(
            "Armor{}{} {}",
            gen_sep(g),
            gen_damage(g, clean),
            gen_percent(g, clean)
        ),
    };
    let comment = if g.below(8) == 0 {
        pick(g, &[" ; comment", ";", " ;x=1 2"])
    } else {
        ""
    };
    format!("{indent}{body}{comment}")
}

fn gen_fx_line(g: &mut Gen, clean: bool) -> String {
    let indent = pick(g, &["", "  ", "\t"]);
    let veteran = g.below(3) == 0;
    let (field, value) = match g.below(4) {
        0 => ("AmountForMajorFX", gen_real(g, clean)),
        1 => ("MajorFX", gen_fx(g)),
        2 => ("MinorFX", gen_fx(g)),
        _ => ("ThrottleTime", gen_ms(g, clean)),
    };
    let body = if !clean && g.below(40) == 0 {
        pick(
            g,
            &[
                "Foo = 1",
                "MajorFX",
                "MajorFX = EXPLOSION",
                "ThrottleTime =",
                "VeterancyMajorFX = ELITE",
                "majorfx = FLAME FX_Big",
                "Dead = 1",
            ],
        )
        .to_string()
    } else if veteran {
        format!(
            "Veterancy{field}{}{} {} {value}",
            gen_sep(g),
            gen_vet(g, clean),
            gen_damage(g, clean)
        )
    } else {
        format!("{field}{}{} {value}", gen_sep(g), gen_damage(g, clean))
    };
    let comment = if g.below(8) == 0 {
        pick(g, &[" ; comment", ";", " ;x=1 2"])
    } else {
        ""
    };
    format!("{indent}{body}{comment}")
}

fn gen_end(g: &mut Gen) -> String {
    pick(
        g,
        &[
            "End",
            "END",
            "end",
            "  End  ",
            "End ; done",
            "End=",
            "Ends",
            "=End",
        ],
    )
    .to_string()
}

fn gen_block_name(g: &mut Gen) -> String {
    match g.below(10) {
        0 => String::new(),
        1..=2 => format!("Name{}", g.below(100)),
        _ => pick(g, &BLOCK_NAMES).to_string(),
    }
}

/// One block; `clean` draws only well-formed values (and always an `End`).
fn gen_block(g: &mut Gen, lines: &mut Vec<String>, clean: bool, kind: u32) {
    let (word, armor) = match kind {
        0 => ("Armor", true),
        1 => ("DamageFX", false),
        _ if !clean && g.below(8) == 0 => {
            (pick(g, &["Foo", "Armour", "damagefx2", "Objects"]), true)
        }
        _ => {
            if g.below(2) == 0 {
                ("Armor", true)
            } else {
                ("DamageFX", false)
            }
        }
    };
    let word = if !clean && g.below(10) == 0 {
        case_mix(g, word)
    } else {
        word.to_string()
    };
    let name = if clean {
        format!("T{}", g.below(40))
    } else {
        gen_block_name(g)
    };
    lines.push(format!("{word} {name}").trim_end().to_string());
    for _ in 0..g.below(if clean { 14 } else { 9 }) {
        lines.push(if armor {
            gen_armor_line(g, clean)
        } else {
            gen_fx_line(g, clean)
        });
    }
    if clean || g.below(15) != 0 {
        lines.push(if clean { "End".to_string() } else { gen_end(g) });
    }
}

fn gen_file(g: &mut Gen) -> Vec<u8> {
    // 40% of the files draw only well-formed values and always end their blocks
    let clean = g.below(10) < 4;
    let mut lines = Vec::new();
    for _ in 0..1 + g.below(4) {
        match g.below(10) {
            0 => lines.push(pick(g, &["; a comment", "", "   ", ";", "\t"]).to_string()),
            1 if !clean => lines.push(pick(g, &["Unknown block", "=", "  =  ", "End"]).to_string()),
            _ => gen_block(g, &mut lines, clean, 2),
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
    match if clean { 99 } else { g.below(14) } {
        0 if !out.is_empty() => {
            let p = g.below(out.len() as u32) as usize;
            out.insert(
                p,
                if g.below(2) == 0 {
                    0
                } else {
                    0x80 + g.below(0x80) as u8
                },
            );
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

/// A file of well-formed blocks only: `n` `Armor` (kind 0) or `DamageFX`
/// (kind 1) blocks.
fn gen_clean_file(g: &mut Gen, n: u32, kind: u32) -> Vec<u8> {
    let mut lines = Vec::new();
    for _ in 0..n {
        gen_block(g, &mut lines, true, kind);
    }
    let mut s = lines.join("\n");
    s.push('\n');
    s.into_bytes()
}

// ---------------------------------------------------------------------------
// Running both sides
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
    Ok(Outcome {
        status,
        detail,
        crc,
    })
}

fn cpp_load(name: &[u8], data: &[u8], with_xfer: bool) -> Outcome {
    let cname = CString::new(name.to_vec()).unwrap();
    let mut log = vec![0u8; 16];
    let mut msg = vec![0u8; 4096];
    let (mut log_len, mut crc, mut code) = (0i32, 0u32, 0u32);
    let status = unsafe {
        cpp::ref_ini_reset();
        cpp::ref_ini_add_file(cname.as_ptr(), data.as_ptr() as *const _, data.len() as i32);
        cpp::ref_ini_load(
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
    assert_eq!(log_len, 0, "the real block handlers log nothing");
    let detail = match status {
        0 | 4 => Vec::new(),
        1 => msg[..code as usize].to_vec(),
        2 | 3 => code.to_le_bytes().to_vec(),
        _ => panic!("C++ threw something unexpected (status {status})"),
    };
    Outcome {
        status,
        detail,
        crc: if with_xfer { crc } else { 0 },
    }
}

fn show(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn name_probe() -> Vec<String> {
    let mut v: Vec<String> = BLOCK_NAMES.iter().map(|s| s.to_string()).collect();
    v.extend((0..100).map(|i| format!("Name{i}")));
    v.extend((0..40).map(|i| format!("T{i}")));
    v.extend(["Probe", "zzz", "Tank "].map(String::from));
    v
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
fn ini_blocks_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let table = blocks();
    let probe = name_probe();
    let mut g = Gen::new(0xA2C4);
    let mut outcome = Check::new("Armor/DamageFX INI::load (throw, CRC)");
    let mut armors = Check::new("ArmorStore (every coefficient)");
    let mut fxs = Check::new("DamageFXStore (every entry)");
    let mut keys = Check::new("name keys after the load");
    let mut excluded = Excluded::default();
    let mut ctx = Ctx::new();
    let (mut ok, mut failed, mut templates, mut damage_fxs) = (0u64, 0u64, 0u64, 0u64);
    const CASES: usize = 8_000;
    for case in 0..CASES {
        let data = gen_file(&mut g);
        let name = format!("Data\\INI\\Case{}.ini", case % 7).into_bytes();
        let with_xfer = g.below(4) != 0;
        ctx.armor = ArmorStore::new();
        ctx.fx = DamageFxStore::new();
        ctx.names.reset();
        let mut crc = XferCrc::new_without_nan_guard();
        let r = Ini::load(
            &name,
            &data,
            IniLoadType::Overwrite,
            if with_xfer {
                Some(&mut crc as &mut dyn Xfer)
            } else {
                None
            },
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
        if rust.status == 0 {
            ok += 1
        } else {
            failed += 1
        }
        templates += ctx.armor.len() as u64;
        damage_fxs += ctx.fx.len() as u64;
        unsafe { cpp::ref_armor_reset() };
        let c = cpp_load(&name, &data, with_xfer);
        outcome.eq(&rust, &c, || format!("case {case}: file {:?}", show(&data)));
        armors.eq(
            rust_armor_dump(&ctx.armor),
            cpp_dump(cpp::ref_armor_dump),
            || format!("case {case}: file {:?}", show(&data)),
        );
        fxs.eq(rust_dfx_dump(&ctx.fx), cpp_dump(cpp::ref_dfx_dump), || {
            format!("case {case}: file {:?}", show(&data))
        });
        let rk: Vec<u32> = probe
            .iter()
            .map(|n| ctx.names.name_to_key(n.as_bytes()).0)
            .collect();
        let ck: Vec<u32> = probe
            .iter()
            .map(|n| {
                let c = CString::new(n.as_str()).unwrap();
                unsafe { cpp::ref_armor_name_key(c.as_ptr()) }
            })
            .collect();
        keys.eq(rk, ck, || format!("case {case}: file {:?}", show(&data)));
    }
    println!(
        "Armor/DamageFX INI: {CASES} files, {ok} loaded, {failed} threw, {templates} armor templates and {damage_fxs} DamageFXes compared; excluded (undefined upstream): {} {:?}",
        excluded.total(),
        excluded.0
    );
    assert!(
        ok > 1500 && failed > 1500,
        "the generator must reach both outcomes: {ok} loaded, {failed} threw"
    );
    assert!(
        templates > 2500 && damage_fxs > 2500,
        "too few stores compared: {templates} / {damage_fxs}"
    );
    outcome.finish();
    armors.finish();
    fxs.finish();
    keys.finish();
}

// ---------------------------------------------------------------------------
// Armor::adjustDamage
// ---------------------------------------------------------------------------

/// The planted wrong variants of `adjustDamage`, from the coefficients.
fn planted_adjust(coef: &[f32; DAMAGE_NUM_TYPES], t: i32, damage: f32, bug: u32) -> f32 {
    if (t == DAMAGE_UNRESISTABLE) || (bug != 2 && t == DAMAGE_SUBDUAL_UNRESISTABLE) {
        return damage;
    }
    let d = damage * coef[t as usize];
    if bug == 1 || d >= 0.0 || d.is_nan() {
        d
    } else {
        0.0
    }
}

#[test]
fn armor_adjust_damage_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let table = blocks();
    let mut g = Gen::new(0xA2C5);
    let mut ctx = Ctx::new();
    let data = gen_clean_file(&mut g, 120, 0);
    Ini::load(
        b"armor.ini",
        &data,
        IniLoadType::Overwrite,
        None,
        &table,
        &mut ctx,
    )
    .expect("clean armor file loads");
    unsafe { cpp::ref_armor_reset() };
    let c = cpp_load(b"armor.ini", &data, false);
    assert_eq!(c.status, 0);
    assert!(ctx.armor.len() > 20);
    let names: Vec<String> = (0..40).map(|i| format!("T{i}")).collect();

    let mut both = Check::new("Armor::adjustDamage / ArmorTemplate::adjustDamage");
    let mut no_floor = Check::new("Armor::adjustDamage");
    let mut one_unresistable = Check::new("Armor::adjustDamage");
    let (mut compared, mut out_of_range, mut missing) = (0u64, 0u64, 0u64);
    for case in 0..40_000 {
        let n = &names[g.below(names.len() as u32) as usize];
        let dt = if g.below(20) == 0 {
            g.below(50) as i32 - 6
        } else {
            g.below(DAMAGE_NUM_TYPES as u32) as i32
        };
        let damage = if g.below(4) == 0 {
            g.finite()
        } else {
            g.mixed()
        };
        let key = ctx
            .armor
            .find_armor_template_key(&mut ctx.names, n.as_bytes())
            .unwrap();
        let Some(key) = key else {
            // no such template: the C++ finds none either
            let cn = CString::new(n.as_str()).unwrap();
            let mut out = [0u32; 2];
            assert_eq!(
                unsafe { cpp::ref_armor_adjust(cn.as_ptr(), dt, damage, out.as_mut_ptr()) },
                1
            );
            missing += 1;
            continue;
        };
        let armor = ctx.armor.make_armor(Some(key));
        let tmpl = ctx.armor.get(key).unwrap();
        let ra = armor.adjust_damage(&ctx.armor, dt, damage);
        let rt = tmpl.adjust_damage(dt, damage);
        assert_eq!(ra.map(f32::to_bits), rt.map(f32::to_bits));
        let Ok(r) = ra else {
            assert_eq!(ra, Err(DamageTypeOutOfRange));
            out_of_range += 1;
            continue;
        };
        let cn = CString::new(n.as_str()).unwrap();
        let mut out = [0u32; 2];
        assert_eq!(
            unsafe { cpp::ref_armor_adjust(cn.as_ptr(), dt, damage, out.as_mut_ptr()) },
            0
        );
        compared += 1;
        both.f32(r, f32::from_bits(out[0]), || {
            format!("case {case}: {n} type {dt} damage {damage:e} (Armor)")
        });
        both.f32(r, f32::from_bits(out[1]), || {
            format!("case {case}: {n} type {dt} damage {damage:e} (template)")
        });
        let coef = tmpl.coefficients();
        no_floor.f32(
            planted_adjust(coef, dt, damage, 1),
            f32::from_bits(out[0]),
            || format!("case {case}"),
        );
        one_unresistable.f32(
            planted_adjust(coef, dt, damage, 2),
            f32::from_bits(out[0]),
            || format!("case {case}"),
        );
    }
    // Armor() and a cleared Armor pass the damage through
    for _ in 0..200 {
        let d = g.mixed();
        let mut out = [0u32; 2];
        unsafe { cpp::ref_armor_none(d, out.as_mut_ptr()) };
        let mut a = ctx.armor.make_armor(None);
        both.f32(
            a.adjust_damage(&ctx.armor, 5, d).unwrap(),
            f32::from_bits(out[0]),
            || format!("Armor() damage {d:e}"),
        );
        a = ctx
            .armor
            .make_armor(ctx.armor.templates().next().map(|(k, _)| k));
        a.clear();
        both.f32(
            a.adjust_damage(&ctx.armor, 5, d).unwrap(),
            f32::from_bits(out[1]),
            || format!("cleared Armor damage {d:e}"),
        );
    }
    println!("Armor::adjustDamage: {compared} queries; excluded: {out_of_range} damage types out of range (undefined upstream); {missing} unknown templates (C++ finds none too)");
    assert!(compared > 20_000);
    both.finish();
    no_floor.finish_planted();
    one_unresistable.finish_planted();
}

// ---------------------------------------------------------------------------
// DamageFX getters
// ---------------------------------------------------------------------------

#[test]
fn damage_fx_getters_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let table = blocks();
    let mut g = Gen::new(0xA2C6);
    let mut ctx = Ctx::new();
    let data = gen_clean_file(&mut g, 120, 1);
    Ini::load(
        b"fx.ini",
        &data,
        IniLoadType::Overwrite,
        None,
        &table,
        &mut ctx,
    )
    .expect("clean DamageFX file loads");
    unsafe { cpp::ref_armor_reset() };
    let c = cpp_load(b"fx.ini", &data, false);
    assert_eq!(c.status, 0);
    assert!(ctx.fx.len() > 20);
    let names: Vec<String> = (0..40).map(|i| format!("T{i}")).collect();

    let mut ok = Check::new("DamageFX getters (throttle, FXList, doDamageFX)");
    let mut major_when_equal = Check::new("DamageFX getDamageFXList");
    let mut zero_damage = Check::new("DamageFX getDamageFXList");
    let (mut compared, mut out_of_range, mut major, mut minor, mut none) =
        (0u64, 0u64, 0u64, 0u64, 0u64);
    for case in 0..40_000 {
        let n = &names[g.below(names.len() as u32) as usize];
        let dt = if g.below(20) == 0 {
            g.below(50) as i32 - 6
        } else {
            g.below(DAMAGE_NUM_TYPES as u32) as i32
        };
        let level = g.below(5) as i32 - 1; // -1: no source object
        let vet = if level < 0 {
            VeterancyLevel::Regular
        } else {
            LEVELS[level as usize]
        };
        // amounts around the thresholds the INI set, and the specials
        let amount = match g.below(6) {
            0 => 0.0,
            1 => -0.0,
            2 => {
                let thr = ctx
                    .fx
                    .find_damage_fx(&mut ctx.names, n.as_bytes())
                    .unwrap()
                    .and_then(|d| {
                        d.dfx(dt.clamp(0, 37), vet)
                            .ok()
                            .map(|e| e.amount_for_major_fx)
                    });
                thr.unwrap_or(10.0)
            }
            3 => g.mixed(),
            _ => g.uniform(-10.0, 1010.0),
        };
        let Some(fx) = ctx.fx.find_damage_fx(&mut ctx.names, n.as_bytes()).unwrap() else {
            continue;
        };
        let rt = fx.get_damage_fx_throttle_time(dt, vet);
        let rl = fx.get_damage_fx_list(dt, amount, vet);
        let (Ok(rt), Ok(rl)) = (rt, rl) else {
            out_of_range += 1;
            continue;
        };
        let cn = CString::new(n.as_str()).unwrap();
        let (mut throttle, mut fx_len, mut via_do) = (0u32, 0i32, 0i32);
        let mut fx_buf = vec![0u8; 256];
        let st = unsafe {
            cpp::ref_dfx_query(
                cn.as_ptr(),
                dt,
                amount,
                level,
                &mut throttle,
                fx_buf.as_mut_ptr() as *mut _,
                256,
                &mut fx_len,
                &mut via_do,
            )
        };
        assert_eq!(st, 0);
        compared += 1;
        let cl: Option<Vec<u8>> = if fx_len < 0 {
            None
        } else {
            Some(fx_buf[..fx_len as usize].to_vec())
        };
        ok.eq((rt, rl.map(|s| s.to_vec()), true), (throttle, cl.clone(), via_do == 1), || {
            format!("case {case}: {n} type {dt} amount {amount:e} level {level} (via doDamageFX: {via_do})")
        });
        match &cl {
            None => none += 1,
            Some(_) if amount >= fx.dfx(dt, vet).unwrap().amount_for_major_fx => major += 1,
            Some(_) => minor += 1,
        }
        // planted: the threshold is exclusive; zero damage is not special
        let e = fx.dfx(dt, vet).unwrap();
        let wrong1 = if amount == 0.0 {
            None
        } else if amount > e.amount_for_major_fx {
            e.major_damage_fx_list.clone()
        } else {
            e.minor_damage_fx_list.clone()
        };
        major_when_equal.eq(wrong1, cl.clone(), || format!("case {case}"));
        let wrong2 = if amount >= e.amount_for_major_fx {
            e.major_damage_fx_list.clone()
        } else {
            e.minor_damage_fx_list.clone()
        };
        zero_damage.eq(wrong2, cl, || format!("case {case}"));
    }
    println!("DamageFX getters: {compared} queries ({major} major, {minor} minor, {none} no effect); excluded: {out_of_range} damage types out of range (undefined upstream)");
    assert!(compared > 20_000 && major > 1000 && minor > 1000 && none > 1000);
    ok.finish();
    major_when_equal.finish_planted();
    zero_damage.finish_planted();
}

// ---------------------------------------------------------------------------
// DamageInfo xfer
// ---------------------------------------------------------------------------

const TEMPLATE_NAMES: [&str; 5] = [
    "AmericaTankCrusader",
    "ChinaInfantryRedguard",
    "GLAVehicleScud",
    "x",
    "Lazer\u{e9}",
];

fn random_info(g: &mut Gen) -> DamageInfo {
    let mut d = DamageInfo::default();
    d.input.source_id = if g.below(3) == 0 { 0 } else { g.i32() };
    d.input.source_template = match g.below(8) {
        0 => Vec::new(),
        1 => vec![b'n'; 200 + g.below(100) as usize], // 255 and under saves, above is an error
        _ => TEMPLATE_NAMES[g.below(5) as usize].as_bytes().to_vec(),
    };
    d.input.source_player_mask = g.u32() as u16;
    d.input.damage_type = g.below(38) as i32;
    d.input.damage_fx_override = g.below(38) as i32;
    d.input.death_type = g.below(21) as i32;
    d.input.amount = g.mixed();
    d.input.kill = g.below(2) == 0;
    d.input.damage_status_type = g.below(40) as i32;
    d.input.shock_wave_vector.x = g.mixed();
    d.input.shock_wave_vector.y = g.mixed();
    d.input.shock_wave_vector.z = g.mixed();
    d.input.shock_wave_amount = g.mixed();
    d.input.shock_wave_radius = g.mixed();
    d.input.shock_wave_taper_off = g.mixed();
    d.out.actual_damage_dealt = g.mixed();
    d.out.actual_damage_clipped = g.mixed();
    d.out.no_effect = g.below(2) == 0;
    d
}

/// The 17 words of wrap_armor.cpp's `fillInfo`.
fn to_words(d: &DamageInfo) -> [u32; 17] {
    let i = &d.input;
    [
        i.source_id as u32,
        u32::from(i.source_player_mask),
        i.damage_type as u32,
        i.damage_fx_override as u32,
        i.death_type as u32,
        i.amount.to_bits(),
        u32::from(i.kill),
        i.damage_status_type as u32,
        i.shock_wave_vector.x.to_bits(),
        i.shock_wave_vector.y.to_bits(),
        i.shock_wave_vector.z.to_bits(),
        i.shock_wave_amount.to_bits(),
        i.shock_wave_radius.to_bits(),
        i.shock_wave_taper_off.to_bits(),
        d.out.actual_damage_dealt.to_bits(),
        d.out.actual_damage_clipped.to_bits(),
        u32::from(d.out.no_effect),
    ]
}

fn snapshot_of(d: &mut DamageInfo, kind: i32) -> &mut dyn Snapshot {
    match kind {
        0 => d,
        1 => &mut d.input,
        _ => &mut d.out,
    }
}

fn status_of(r: Result<(), XferStatus>) -> i32 {
    match r {
        Ok(()) => 0,
        Err(e) => e as i32,
    }
}

fn tmp_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "rts-lockstep-damage-{}-{tag}.bin",
        std::process::id()
    ))
}

fn template_cstring(d: &DamageInfo) -> CString {
    // the C++ side takes a C string: the generator never puts a NUL in a name
    CString::new(d.input.source_template.clone()).unwrap()
}

#[test]
fn damage_info_xfer_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0xA2C7);
    let path = tmp_path("xfer");
    let cpath = CString::new(path.to_str().unwrap()).unwrap();
    let mut save = Check::new("DamageInfo xfer save (bytes, status)");
    let mut save_bug = Check::new("DamageInfo xfer save");
    let mut load = Check::new("DamageInfo xfer load (fields, status)");
    let mut load_bug = Check::new("DamageInfo xfer load");
    let mut crc = Check::new("DamageInfo xfer CRC");
    let mut crc_bug = Check::new("DamageInfo xfer CRC");
    let (mut saved_ok, mut save_failed, mut load_ok, mut load_failed) = (0u64, 0u64, 0u64, 0u64);
    const CASES: usize = 6_000;
    for case in 0..CASES {
        let kind = (case % 3) as i32;
        let mut info = random_info(&mut g);
        // save
        let w = to_words(&info);
        let tmpl = template_cstring(&info);
        let mut cbytes = vec![0u8; 4096];
        let mut clen = 0i32;
        let cst = unsafe {
            cpp::ref_damage_save(
                kind,
                w.as_ptr(),
                tmpl.as_ptr(),
                cpath.as_ptr(),
                cbytes.as_mut_ptr(),
                4096,
                &mut clen,
            )
        };
        cbytes.truncate(clen as usize);
        let mut xs = XferSave::new();
        xs.open(b"rust").unwrap();
        let rst = status_of(xs.xfer_snapshot(snapshot_of(&mut info, kind)));
        let _ = xs.close();
        let rbytes = xs.bytes().to_vec();
        if rst == 0 {
            saved_ok += 1
        } else {
            save_failed += 1
        }
        save.eq((rst, &rbytes), (cst, &cbytes), || {
            format!("case {case} kind {kind}: {info:?}")
        });
        let mut planted = rbytes.clone();
        if planted.len() > 3 {
            planted[2] ^= 1; // planted: a byte of the value changes
        } else {
            planted.push(0);
        }
        save_bug.eq((rst, &planted), (cst, &cbytes), || format!("case {case}"));

        // load: the saved file, or a mutated one, into a random starting info
        let mut file = cbytes.clone();
        match g.below(8) {
            1 if !file.is_empty() => file[0] = g.below(6) as u8, // the version byte
            2 if !file.is_empty() => {
                let p = g.below(file.len() as u32) as usize;
                file[p] = g.u32() as u8;
            }
            3 => file.truncate(file.len().saturating_sub(1 + g.below(6) as usize)),
            4 => file.extend((0..g.below(5)).map(|_| g.u32() as u8)),
            5 if file.len() > 1 => file[1] = g.below(6) as u8, // the second version byte, or a field
            _ => {}
        }
        let start = random_info(&mut g);
        let mut tmpl_start = start.input.source_template.clone();
        tmpl_start.truncate(250);
        let mut start = start;
        start.input.source_template = tmpl_start;
        let sw = to_words(&start);
        let mut cw = sw;
        let stmpl = template_cstring(&start);
        let mut ctmpl = vec![0u8; 512];
        ctmpl[..stmpl.as_bytes().len()].copy_from_slice(stmpl.as_bytes());
        let cst = unsafe {
            cpp::ref_damage_load(
                kind,
                cw.as_mut_ptr(),
                ctmpl.as_mut_ptr() as *mut _,
                512,
                file.as_ptr(),
                file.len() as i32,
                cpath.as_ptr(),
            )
        };
        let cname = unsafe { std::ffi::CStr::from_ptr(ctmpl.as_ptr() as *const _) }
            .to_bytes()
            .to_vec();
        let mut r = start.clone();
        let mut xl = XferLoad::new(file.clone());
        xl.open(b"rust").unwrap();
        let rst = status_of(xl.xfer_snapshot(snapshot_of(&mut r, kind)));
        if rst == 0 {
            load_ok += 1
        } else {
            load_failed += 1
        }
        let rw = to_words(&r);
        // After a failed load the field being read may hold a partial read
        // in C++ (fread copies the bytes it got into the field), which the
        // Rust XferLoad does not do: so a failed load compares the status
        // and that at most one value (the 17 words and the name) differs.
        let differing = rw.iter().zip(cw.iter()).filter(|(a, b)| a != b).count()
            + usize::from(r.input.source_template != cname);
        if rst == 0 {
            load.eq(
                (rst, rw, r.input.source_template.clone()),
                (cst, cw, cname.clone()),
                || format!("case {case} kind {kind}: file {file:?} start {start:?}"),
            );
        } else {
            load.eq((rst, differing <= 1), (cst, true), || format!("case {case} kind {kind}: file {file:?} start {start:?}: {differing} values differ"));
        }
        // planted: the amount's sign flips (a load that succeeded), or on a
        // version-1 input the m_kill byte is treated as absent
        if rst == 0 {
            let mut ww = rw;
            ww[5] ^= 0x8000_0000;
            if kind == 2 {
                ww[14] ^= 0x8000_0000;
            }
            load_bug.eq((rst, ww), (cst, cw), || format!("case {case}"));
        } else {
            load_bug.eq((rst, differing <= 1, 1u8), (cst, true, 0u8), || {
                format!("case {case}")
            });
        }

        // CRC
        let mut cc = 7u32;
        let tmpl = template_cstring(&info);
        let cst = unsafe { cpp::ref_damage_crc(kind, w.as_ptr(), tmpl.as_ptr(), &mut cc) };
        let mut xc = XferCrc::new_without_nan_guard();
        xc.open(b"crc").unwrap();
        let rst = status_of(xc.xfer_snapshot(snapshot_of(&mut info, kind)));
        crc.eq((rst, xc.get_crc()), (cst, cc), || {
            format!("case {case} kind {kind}")
        });
        crc_bug.eq((rst, xc.get_crc() ^ 1), (cst, cc), || {
            format!("case {case}")
        });
    }
    let _ = std::fs::remove_file(&path);
    println!("DamageInfo xfer: {CASES} infos; saves {saved_ok} ok, {save_failed} failed; loads {load_ok} ok, {load_failed} failed");
    assert!(saved_ok > 3000 && save_failed > 100 && load_ok > 1500 && load_failed > 500);
    save.finish();
    save_bug.finish_planted();
    load.finish();
    load_bug.finish_planted();
    crc.finish();
    crc_bug.finish_planted();
}

// ---------------------------------------------------------------------------
// Flags, inline helpers and defaults
// ---------------------------------------------------------------------------

#[test]
fn damage_helpers_and_defaults_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut sub = Check::new("IsSubdualDamage / IsHealthDamagingDamage");
    let mut sub_bug = Check::new("IsSubdualDamage");
    for dt in 0..64 {
        let c0 = unsafe { cpp::ref_damage_inline(0, dt) } != 0;
        let c1 = unsafe { cpp::ref_damage_inline(1, dt) } != 0;
        sub.eq(is_subdual_damage(dt), c0, || {
            format!("IsSubdualDamage({dt})")
        });
        sub.eq(is_health_damaging_damage(dt), c1, || {
            format!("IsHealthDamagingDamage({dt})")
        });
        sub_bug.eq(
            is_subdual_damage(dt) && dt != DAMAGE_SUBDUAL_UNRESISTABLE,
            c0,
            || format!("{dt}"),
        );
    }
    sub.finish();
    sub_bug.finish_planted();

    let (mut all, mut none) = (0u64, 1u64);
    unsafe { cpp::ref_damage_flags(&mut all, &mut none) };
    let mut flags = Check::new("DAMAGE_TYPE_FLAGS_ALL / NONE");
    let mut flags_bug = Check::new("DAMAGE_TYPE_FLAGS_ALL");
    flags.eq(DamageTypeFlags::ALL.words(), [all, 0, 0, 0], || {
        "ALL".into()
    });
    flags.eq(DamageTypeFlags::NONE.words(), [none, 0, 0, 0], || {
        "NONE".into()
    });
    flags_bug.eq(
        [DamageTypeFlags::ALL.words()[0] >> 1, 0, 0, 0],
        [all, 0, 0, 0],
        || "ALL".into(),
    );
    flags.finish();
    flags_bug.finish_planted();

    let mut w = [0u32; 17];
    unsafe { cpp::ref_damage_default(w.as_mut_ptr()) };
    let mut defaults = Check::new("DamageInfo() defaults");
    let mut defaults_bug = Check::new("DamageInfo() defaults");
    defaults.eq(to_words(&DamageInfo::default()), w, || {
        "DamageInfo()".into()
    });
    assert_eq!(DamageInfoInput::default(), DamageInfo::default().input);
    assert_eq!(DamageInfoOutput::default(), DamageInfo::default().out);
    let mut wrong = DamageInfo::default();
    wrong.input.damage_fx_override = 0; // planted: not DAMAGE_UNRESISTABLE
    defaults_bug.eq(to_words(&wrong), w, || "DamageInfo()".into());
    defaults.finish();
    defaults_bug.finish_planted();
}
