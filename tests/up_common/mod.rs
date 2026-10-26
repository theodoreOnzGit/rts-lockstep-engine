// SPDX-License-Identifier: GPL-3.0-only
//! Shared by `differential_upgrade.rs` and `differential_special_power.rs`
//! (p1w2-upgrade): the Rust side of the INI blocks (both handlers are always
//! in the table, because one wrapper, `differential/wrap_upgrade.cpp`,
//! provides both in the C++ `INI::load`), the dumps of both stores in the
//! layout of the wrapper's `ref_up_dump` and `ref_sp_dump`, the C++ load, and
//! the generators of INI text.
#![allow(dead_code)]

use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Thrown, TYPE_TABLE};
use rts_lockstep_engine::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use rts_lockstep_engine::common::special_power::SpecialPowerStore;
use rts_lockstep_engine::common::upgrade::UpgradeCenter;
use rts_lockstep_engine::differential as cpp;
use std::collections::BTreeMap;
use std::ffi::CString;

use crate::support::Gen;

/// The sciences the tests define (`SCIENCE_*`): INI names that
/// `RequiredScience` accepts.
pub const SCIENCES: [&str; 5] = ["SCIENCE_Paradrop", "SCIENCE_Rank3", "SCIENCE_NapalmStrike", "science_lower", "SCIENCE_X"];

// ---------------------------------------------------------------------------
// The Rust side of the INI blocks
// ---------------------------------------------------------------------------

pub struct Ctx {
    pub center: UpgradeCenter,
    pub store: SpecialPowerStore,
    pub names: NameKeyGenerator,
    pub sciences: Vec<NameKeyType>,
}

impl Ctx {
    /// What `ref_up_reset` does on the C++ side: a name key generator (reset,
    /// and `init()`ed if asked), the sciences' keys assigned in order, an
    /// empty store, and an `UpgradeCenter` (`init()`ed if asked).
    pub fn new(init_names: bool, init_center: bool) -> Self {
        let mut names = NameKeyGenerator::new();
        names.reset();
        if init_names {
            names.init();
        }
        let sciences = SCIENCES.iter().map(|s| names.name_to_key(s.as_bytes())).collect();
        let mut center = UpgradeCenter::new();
        if init_center {
            center.init(&mut names).expect("init");
        }
        Ctx { center, store: SpecialPowerStore::new(), names, sciences }
    }
}

fn h_upgrade(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.center.parse_upgrade_definition(ini, &mut c.names)
}
fn h_special_power(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    let sciences = c.sciences.clone();
    c.store.parse_special_power_definition(ini, &mut c.names, &move |k| sciences.contains(&k))
}
fn h_stub(_: &mut Ini<'_>, _: &mut Ctx) -> IniResult<()> {
    Err(IniError::new(IniErrorKind::Bug))
}

/// The block table: `Upgrade` and `SpecialPower` are translated; the other 60
/// handlers throw `ERROR_BUG`, as the stubs of wrap_ini.cpp do.
pub fn blocks() -> Vec<BlockParse<Ctx>> {
    TYPE_TABLE
        .iter()
        .map(|&(token, _, _)| BlockParse {
            token,
            parse: match token {
                "Upgrade" => h_upgrade,
                "SpecialPower" => h_special_power,
                _ => h_stub,
            },
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The dumps
// ---------------------------------------------------------------------------

fn put_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_str(v: &mut Vec<u8>, s: &[u8]) {
    put_u32(v, s.len() as u32);
    v.extend_from_slice(s);
}

/// One template of `ref_up_dump`.
fn put_upgrade_template(v: &mut Vec<u8>, t: &rts_lockstep_engine::common::upgrade::UpgradeTemplate) {
    put_u32(v, t.upgrade_type() as i32 as u32);
    put_str(v, t.upgrade_name());
    put_u32(v, t.upgrade_name_key().0);
    put_str(v, t.display_name_label());
    put_u32(v, t.build_time().to_bits());
    put_u32(v, t.cost_to_build() as u32);
    for w in t.upgrade_mask().words() {
        v.extend_from_slice(&w.to_le_bytes());
    }
    put_str(v, &t.research_complete_sound().event_name);
    put_str(v, &t.unit_specific_sound().event_name);
    put_u32(v, t.academy_classification_type() as i32 as u32);
    put_str(v, t.button_image_name());
}

/// The dump of wrap_upgrade.cpp's `ref_up_dump`. `bug`: 0 none; the others
/// are the planted wrong variants (1: the oldest template first, 2: the
/// build time cut to a whole number, 3: the mask's two words swapped).
pub fn rust_upgrade_dump(center: &UpgradeCenter, bug: u32) -> Vec<u8> {
    let mut v = Vec::new();
    put_u32(&mut v, center.len() as u32);
    put_u32(&mut v, center.next_template_mask_bit() as u32);
    let mut ts: Vec<_> = center.templates().collect();
    if bug == 1 {
        ts.reverse();
    }
    for t in ts {
        match bug {
            2 => {
                let mut one = Vec::new();
                put_upgrade_template(&mut one, t);
                let at = 4 + 4 + t.upgrade_name().len() + 4 + 4 + t.display_name_label().len();
                let truncated = f32::from_bits(u32::from_le_bytes(one[at..at + 4].try_into().unwrap())).trunc();
                one[at..at + 4].copy_from_slice(&truncated.to_bits().to_le_bytes());
                v.extend_from_slice(&one);
            }
            3 => {
                let mut one = Vec::new();
                put_upgrade_template(&mut one, t);
                let at = 4 + 4 + t.upgrade_name().len() + 4 + 4 + t.display_name_label().len() + 4 + 4;
                let (a, b) = (one[at..at + 8].to_vec(), one[at + 8..at + 16].to_vec());
                one[at..at + 8].copy_from_slice(&b);
                one[at + 8..at + 16].copy_from_slice(&a);
                v.extend_from_slice(&one);
            }
            _ => put_upgrade_template(&mut v, t),
        }
    }
    v
}

/// The dump of wrap_upgrade.cpp's `ref_sp_dump`. `bug`: 0 none; the others
/// are planted wrong variants (1: the store's count and next id swapped,
/// 2: the nodes' override flags forgotten).
pub fn rust_special_power_dump(store: &SpecialPowerStore, bug: u32) -> Vec<u8> {
    let mut v = Vec::new();
    put_u32(&mut v, store.len() as u32);
    put_u32(&mut v, store.next_special_power_id());
    for t in store.templates() {
        let mut n = 0u32;
        let mut node = Some(t.chain());
        while let Some(o) = node {
            n += 1;
            node = o.get_next_override();
        }
        put_u32(&mut v, n);
        let mut node = Some(t.chain());
        while let Some(o) = node {
            v.push(if bug == 2 { 0 } else { u8::from(o.is_override()) });
            let d = &o.data;
            put_str(&mut v, &d.name);
            put_u32(&mut v, d.id);
            put_u32(&mut v, d.special_power_type as u32);
            put_u32(&mut v, d.reload_time);
            put_u32(&mut v, d.required_science.0 as u32);
            put_str(&mut v, &d.initiate_sound.event_name);
            put_str(&mut v, &d.initiate_at_location_sound.event_name);
            put_u32(&mut v, d.academy_classification_type as i32 as u32);
            put_u32(&mut v, d.detection_time);
            put_u32(&mut v, d.view_object_duration);
            put_u32(&mut v, d.view_object_range.to_bits());
            put_u32(&mut v, d.radius_cursor_radius.to_bits());
            v.push(u8::from(d.public_timer));
            v.push(u8::from(d.shared_n_sync));
            v.push(u8::from(d.shortcut_power));
            node = o.get_next_override();
        }
    }
    if bug == 1 {
        let (a, b) = (v[0..4].to_vec(), v[4..8].to_vec());
        v[0..4].copy_from_slice(&b);
        v[4..8].copy_from_slice(&a);
    }
    v
}

pub fn cpp_dump(f: unsafe extern "C" fn(*mut u8, i32) -> i32) -> Vec<u8> {
    let mut buf = vec![0u8; 1 << 22];
    let n = unsafe { f(buf.as_mut_ptr(), buf.len() as i32) };
    assert!((n as usize) <= buf.len(), "dump overflow");
    buf.truncate(n as usize);
    buf
}

/// `ref_up_reset`: the C++ twin of [`Ctx::new`].
pub fn cpp_reset(init_names: bool, init_center: bool) {
    let sc: Vec<CString> = SCIENCES.iter().map(|s| CString::new(*s).unwrap()).collect();
    let ptrs: Vec<*const std::os::raw::c_char> = sc.iter().map(|c| c.as_ptr()).collect();
    let st = unsafe { cpp::ref_up_reset(i32::from(init_names), i32::from(init_center), ptrs.len() as i32, ptrs.as_ptr()) };
    assert_eq!(st, 0);
}

// ---------------------------------------------------------------------------
// Running both sides
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    pub status: i32,
    pub detail: Vec<u8>,
    pub crc: u32,
}

pub fn rust_outcome(r: IniResult<()>, crc: u32) -> Result<Outcome, IniError> {
    let (status, detail) = match r {
        Ok(()) => (0, Vec::new()),
        Err(e) => match e.thrown() {
            None => return Err(e),
            Some(Thrown::Exception(m)) => (1, m),
            Some(Thrown::Status(c)) => (2, c.to_le_bytes().to_vec()),
            Some(Thrown::ErrorCode(c)) => (3, c.to_le_bytes().to_vec()),
            Some(Thrown::OutOfRange) => (4, Vec::new()),
            Some(Thrown::Other) => (5, Vec::new()),
        },
    };
    Ok(Outcome { status, detail, crc })
}

pub fn cpp_load(name: &[u8], data: &[u8], load_type: IniLoadType, with_xfer: bool) -> Outcome {
    let cname = CString::new(name.to_vec()).unwrap();
    let mut msg = vec![0u8; 4096];
    let (mut crc, mut code) = (0u32, 0u32);
    let status = unsafe {
        cpp::ref_ini_reset();
        cpp::ref_ini_add_file(cname.as_ptr(), data.as_ptr() as *const _, data.len() as i32);
        cpp::ref_up_load(
            cname.as_ptr(),
            load_type as i32,
            i32::from(with_xfer),
            &mut crc,
            msg.as_mut_ptr() as *mut _,
            msg.len() as i32,
            &mut code,
        )
    };
    let detail = match status {
        0 | 4 => Vec::new(),
        1 => msg[..code as usize].to_vec(),
        2 | 3 => code.to_le_bytes().to_vec(),
        _ => panic!("C++ threw something unexpected (status {status})"),
    };
    Outcome { status, detail, crc: if with_xfer { crc } else { 0 } }
}

pub fn show(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[derive(Default)]
pub struct Excluded(pub BTreeMap<String, u64>);
impl Excluded {
    pub fn add(&mut self, e: &IniError) {
        *self.0.entry(format!("{:?}", e.kind)).or_default() += 1;
    }
    pub fn total(&self) -> u64 {
        self.0.values().sum()
    }
}

// ---------------------------------------------------------------------------
// The generators
// ---------------------------------------------------------------------------

pub fn pick<'a, T: ?Sized>(g: &mut Gen, xs: &[&'a T]) -> &'a T {
    xs[g.below(xs.len() as u32) as usize]
}

pub fn case_mix(g: &mut Gen, s: &str) -> String {
    match g.below(6) {
        0 => s.to_ascii_lowercase(),
        1 => s.to_ascii_uppercase(),
        _ => s.to_string(),
    }
}

pub fn gen_sep(g: &mut Gen) -> &'static str {
    pick(g, &[" = ", "=", " ", " =", "= ", " == ", " = "])
}

pub fn gen_end(g: &mut Gen) -> String {
    pick(g, &["End", "END", "end", "  End  ", "End ; done", "End=", "Ends", "=End"]).to_string()
}

pub fn gen_comment(g: &mut Gen) -> &'static str {
    if g.below(8) == 0 {
        pick(g, &[" ; comment", ";", " ;x=1 2"])
    } else {
        ""
    }
}

pub fn gen_indent(g: &mut Gen) -> &'static str {
    pick(g, &["", "  ", "\t", " \t "])
}

/// A real in the forms INI files use, with `clean` drawing only parseable ones.
pub fn gen_real(g: &mut Gen, clean: bool) -> String {
    match g.below(20) {
        0..=8 => format!("{}", g.uniform(-100.0, 1000.0)),
        9..=12 => format!("{}", g.below(2000)),
        13 => pick(g, &["0", "-0", "1e3", ".5", "5.", "1e-5", "100000000", "0.033", "33.3333"]).to_string(),
        _ if clean => "10".to_string(),
        14 => pick(g, &["x", "", "1e", ".", "-", "12abc", "1.5.3"]).to_string(),
        15 => format!("{}", g.mixed()),
        16 => pick(g, &["3e38", "1e39", "1e30", "-1e30", "7e8", "2e9", "7.2e7"]).to_string(),
        _ => format!("{}", g.finite()),
    }
}

/// An integer token (cost, duration in milliseconds, ...).
pub fn gen_int(g: &mut Gen, clean: bool) -> String {
    match g.below(20) {
        0..=9 => format!("{}", g.below(5000)),
        10..=12 => format!("{}", g.below(300_000)),
        13 => pick(g, &["0", "1", "33", "34", "-5", "1000", "4294967295", "2147483647", "-2147483648"]).to_string(),
        _ if clean => "100".to_string(),
        14 => pick(g, &["x", "", "-", "12abc", "1e3", "5.5", "0x10"]).to_string(),
        15 => pick(g, &["4294967296", "-4294967297", "99999999999"]).to_string(),
        _ => format!("{}", g.i32()),
    }
}

pub fn gen_bool(g: &mut Gen, clean: bool) -> String {
    match g.below(12) {
        0..=7 => pick(g, &["Yes", "No", "yes", "NO", "YES", "no"]).to_string(),
        _ if clean => "Yes".to_string(),
        8 => pick(g, &["True", "false", "1", "0", "Maybe", "", "Y", "Yes No"]).to_string(),
        _ => pick(g, &["Yes", "No"]).to_string(),
    }
}

/// Assemble lines into a file (LF or CRLF, a missing final newline) and, in a
/// share of the unclean files, damage it at random.
pub fn assemble(g: &mut Gen, lines: &[String], clean: bool) -> Vec<u8> {
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
// The generators of blocks
// ---------------------------------------------------------------------------

use rts_lockstep_engine::common::academy_classification::ACADEMY_CLASSIFICATION_TYPE_NAMES;
use rts_lockstep_engine::common::special_power_mask_type::SPECIAL_POWER_NAMES;

pub const UPGRADE_BLOCK_NAMES: [&str; 14] = [
    "Upgrade_Radar",
    "Upgrade_A",
    "upgrade_radar",
    "DefaultUpgrade",
    "Upgrade_Veterancy_VETERAN",
    "Upgrade_Veterancy_ELITE",
    "Upgrade_Veterancy_HEROIC",
    "Upgrade_Veterancy_REGULAR",
    "DEFAULTUPGRADE",
    "U1",
    "U2",
    "Upgrade_Capture",
    "Upgrade_Flash",
    "X",
];

pub const SP_BLOCK_NAMES: [&str; 10] = [
    "DefaultSpecialPower",
    "SuperweaponDaisyCutter",
    "SuperweaponParadrop",
    "superweapondaisycutter",
    "SpecialAbilityX",
    "SuperweaponEMP",
    "defaultspecialpower",
    "A",
    "B",
    "SuperweaponNapalm",
];

const SOUND_NAMES: [&str; 8] = ["Sound_Chime", "sfx_a", "NoSound", "nosound", "NOSOUND", "Boom", "UpgradeComplete", "x"];
const IMAGE_NAMES: [&str; 5] = ["SSRadar", "ssradar", "None", "SAUpgrade_Flash", "Button 1"];
const LABEL_NAMES: [&str; 6] = ["UPGRADE:Radar", "OBJECT:Foo", "Name", "A_B", "\"Quoted Name\"", "two words"];

fn gen_name_from(g: &mut Gen, pool: &[&str], max_extra: u32) -> String {
    if g.below(5) == 0 {
        format!("Name{}", g.below(max_extra))
    } else {
        pool[g.below(pool.len() as u32) as usize].to_string()
    }
}

pub fn gen_upgrade_name(g: &mut Gen, clean: bool) -> String {
    if !clean && g.below(12) == 0 {
        return String::new();
    }
    gen_name_from(g, &UPGRADE_BLOCK_NAMES, 150)
}

pub fn gen_sp_name(g: &mut Gen, clean: bool) -> String {
    if !clean && g.below(12) == 0 {
        return String::new();
    }
    gen_name_from(g, &SP_BLOCK_NAMES, 30)
}

fn gen_sound(g: &mut Gen, clean: bool) -> String {
    if !clean && g.below(15) == 0 {
        String::new()
    } else {
        pick(g, &SOUND_NAMES).to_string()
    }
}

fn gen_academy(g: &mut Gen, clean: bool) -> String {
    if !clean && g.below(8) == 0 {
        return pick(g, &["ACT_X", "", "0", "ACT_NONES"]).to_string();
    }
    let i = g.below(3) as usize;
    case_mix(g, ACADEMY_CLASSIFICATION_TYPE_NAMES[i])
}

pub fn gen_upgrade_line(g: &mut Gen, clean: bool) -> String {
    let (field, value) = match g.below(8) {
        0 => ("DisplayName", pick(g, &LABEL_NAMES).to_string()),
        1 => (
            "Type",
            if !clean && g.below(8) == 0 {
                pick(g, &["", "Both", "PLAYERS", "0"]).to_string()
            } else {
                let i = g.below(2) as usize;
                case_mix(g, ["PLAYER", "OBJECT"][i])
            },
        ),
        2 => ("BuildTime", gen_real(g, clean)),
        3 => ("BuildCost", gen_int(g, clean)),
        4 => ("ButtonImage", pick(g, &IMAGE_NAMES).to_string()),
        5 => ("ResearchSound", gen_sound(g, clean)),
        6 => ("UnitSpecificSound", gen_sound(g, clean)),
        _ => ("AcademyClassify", gen_academy(g, clean)),
    };
    let body = if !clean && g.below(40) == 0 {
        pick(g, &["Foo = 1", "BuildCost", "Cost = 5", "buildcost = 5", "Dead = 1", "Enum = SPECIAL_EMP_PULSE"]).to_string()
    } else {
        format!("{field}{}{value}", gen_sep(g))
    };
    format!("{}{body}{}", gen_indent(g), gen_comment(g))
}

pub fn gen_sp_line(g: &mut Gen, clean: bool) -> String {
    let (field, value) = match g.below(13) {
        0 => ("ReloadTime", gen_int(g, clean)),
        1 => ("DetectionTime", gen_int(g, clean)),
        2 => ("ViewObjectDuration", gen_int(g, clean)),
        3 => (
            "RequiredScience",
            if !clean && g.below(6) == 0 {
                pick(g, &["SCIENCE_Unknown", "", "science_paradrop", "SCIENCE_PARADROP"]).to_string()
            } else {
                pick(g, &SCIENCES).to_string()
            },
        ),
        4 => ("InitiateSound", gen_sound(g, clean)),
        5 => ("InitiateAtLocationSound", gen_sound(g, clean)),
        6 => ("PublicTimer", gen_bool(g, clean)),
        7 => ("SharedSyncedTimer", gen_bool(g, clean)),
        8 => ("ShortcutPower", gen_bool(g, clean)),
        9 => ("ViewObjectRange", gen_real(g, clean)),
        10 => ("RadiusCursorRadius", gen_real(g, clean)),
        11 => (
            "Enum",
            if !clean && g.below(10) == 0 {
                pick(g, &["SPECIAL_NOPE", "", "7", "SPECIAL_"]).to_string()
            } else {
                let i = g.below(SPECIAL_POWER_NAMES.len() as u32) as usize;
                case_mix(g, SPECIAL_POWER_NAMES[i])
            },
        ),
        _ => ("AcademyClassify", gen_academy(g, clean)),
    };
    let body = if !clean && g.below(40) == 0 {
        pick(g, &["Foo = 1", "ReloadTime", "Reload = 5", "reloadtime = 5", "Dead = 1", "BuildCost = 5"]).to_string()
    } else {
        format!("{field}{}{value}", gen_sep(g))
    };
    format!("{}{body}{}", gen_indent(g), gen_comment(g))
}

/// One block: `kind` 0 `Upgrade`, 1 `SpecialPower`, anything else a random
/// one of the two (or, unclean, a block type that is neither). `clean` draws
/// only well-formed values and always an `End`.
pub fn gen_block(g: &mut Gen, lines: &mut Vec<String>, clean: bool, kind: u32) {
    let upgrade = match kind {
        0 => true,
        1 => false,
        _ => g.below(10) < 7,
    };
    let word = if !clean && kind > 1 && g.below(10) == 0 {
        pick(g, &["Foo", "Upgrades", "specialpower2", "Armour"]).to_string()
    } else if upgrade {
        "Upgrade".to_string()
    } else {
        "SpecialPower".to_string()
    };
    let word = if !clean && g.below(10) == 0 { case_mix(g, &word) } else { word };
    let name = if upgrade { gen_upgrade_name(g, clean) } else { gen_sp_name(g, clean) };
    lines.push(format!("{word} {name}").trim_end().to_string());
    for _ in 0..g.below(if clean { 12 } else { 9 }) {
        lines.push(if upgrade { gen_upgrade_line(g, clean) } else { gen_sp_line(g, clean) });
    }
    if clean || g.below(15) != 0 {
        lines.push(if clean { "End".to_string() } else { gen_end(g) });
    }
}

/// A file of 1 to 4 blocks and stray lines; 40% draw only well-formed values.
pub fn gen_file(g: &mut Gen, kind: u32) -> Vec<u8> {
    let clean = g.below(10) < 4;
    let mut lines = Vec::new();
    for _ in 0..1 + g.below(4) {
        match g.below(10) {
            0 => lines.push(pick(g, &["; a comment", "", "   ", ";", "\t"]).to_string()),
            1 if !clean => lines.push(pick(g, &["Unknown block", "=", "  =  ", "End"]).to_string()),
            _ => gen_block(g, &mut lines, clean, kind),
        }
    }
    assemble(g, &lines, clean)
}

/// A file of `n` well-formed blocks of one kind.
pub fn gen_clean_file(g: &mut Gen, n: u32, kind: u32) -> Vec<u8> {
    let mut lines = Vec::new();
    for _ in 0..n {
        gen_block(g, &mut lines, true, kind);
    }
    let mut s = lines.join("\n");
    s.push('\n');
    s.into_bytes()
}

/// A file of `n` upgrade blocks with distinct names `P0`, `P1`, ... (to fill
/// the 128 mask bits).
pub fn gen_many_upgrades(g: &mut Gen, n: u32, prefix: &str) -> Vec<u8> {
    let mut lines = Vec::new();
    for i in 0..n {
        lines.push(format!("Upgrade {prefix}{i}"));
        for _ in 0..g.below(3) {
            lines.push(gen_upgrade_line(g, true));
        }
        lines.push("End".to_string());
    }
    let mut s = lines.join("\n");
    s.push('\n');
    s.into_bytes()
}

/// The names whose keys are probed after a load (key assignment order is
/// lockstep state): every name the generators use, plus a few unseen.
pub fn name_probe() -> Vec<String> {
    let mut v: Vec<String> = UPGRADE_BLOCK_NAMES.iter().map(|s| s.to_string()).collect();
    v.extend(SP_BLOCK_NAMES.iter().map(|s| s.to_string()));
    v.extend(SCIENCES.iter().map(|s| s.to_string()));
    v.extend((0..150).map(|i| format!("Name{i}")));
    v.extend((0..130).map(|i| format!("P{i}")));
    v.extend(["Probe", "zzz", "Tank "].map(String::from));
    v
}

pub fn cpp_name_key(n: &str) -> u32 {
    let c = CString::new(n).unwrap();
    unsafe { cpp::ref_up_name_key(c.as_ptr()) }
}
