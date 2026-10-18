// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of Phase 1 layer 4's `Science` and `RankInfo`
//! (`common::science`, `logic::rank_info`) against upstream `Science.cpp` and
//! `RankInfo.cpp` (commit 0a05454d), compiled as `differential/wrap_science.cpp`
//! describes, with the real `INI::load`, `INI::parseScience`,
//! `INI::parseScienceVector`, `INI::parseAndTranslateLabel`,
//! `NameKeyGenerator` and `Xfer::xferScienceType`/`xferScienceVec` underneath.
//!
//! No game INI files are available, so every file is **synthetic**: `Science`
//! blocks (every field in several forms, repeated names, names differing only
//! in case, prerequisites that are known, unknown, `None`, repeated, empty
//! lists, costs and booleans well-formed and malformed, labels that translate
//! and labels that translate to nothing), `Rank` blocks (monotonic and
//! out-of-order levels, every field), and `WebpageURL` blocks (an unused block
//! name that the C++ wrapper uses to reach `INI::parseScience` and
//! `INI::parseScienceVector` on their own), with comments, `CRLF`, a missing
//! `End`, unknown fields and blocks, and in a share of files random byte
//! damage. The game text is a stand-in on both sides: a label translates to
//! `T:` and its bytes, and to nothing if it starts with `NOTEXT`.
//!
//! 1. `INI sessions`: each session is 2-5 steps against one pair of stores,
//!    each step a file loaded with `INI_LOAD_OVERWRITE` (or, rarely,
//!    `INI_LOAD_MULTIFILE`), a file loaded with `INI_LOAD_CREATE_OVERRIDES`
//!    (which chains overrides onto the sciences and ranks already there, and
//!    adds sciences marked as overrides), `ScienceStore::reset()` and
//!    `RankInfoStore::reset()` (which delete exactly those), or `init()`.
//!    After every step, compared: what `INI::load` throws (nothing, an
//!    `INIException` and its whole message, an `INI_*` status code, an
//!    `ErrorCode`), the INI `XferCRC`, **every field of every node of every
//!    override chain** of both stores (the override flag, the science key, the
//!    name and description, the runtime-computed root sciences, the
//!    prerequisites, the cost, the grantable flag; the rank name, the points
//!    and the granted sciences), the `WebpageURL` records, also when the load
//!    threw, and the lookups (`isValidScience`, `isScienceGrantable`,
//!    `getNameAndDescription`, `getSciencePurchaseCost`,
//!    `getInternalNameForScience` of a range of ids, `getScienceFromInternalName`
//!    and `friend_lookupScience` of names, `friend_getScienceNames`,
//!    `getRankLevelCount`, `getRankInfo` of every level and beyond). The name
//!    keys are lockstep state, so every lookup assigns them in the same order
//!    on both sides and the keys appear in the dumps.
//! 2. `xferScienceType` / `xferScienceVec`: random science vectors (known
//!    sciences, `SCIENCE_INVALID`, ids no name has) saved (bytes and status
//!    compared), loaded back, loaded from mutated files (version byte, count,
//!    a name byte, truncation, extra bytes) into random starting vectors
//!    (the result and the status compared), and run through `XferCRC`.
//!
//! Inputs for which the Rust reports undefined behaviour upstream (a
//! prerequisite cycle created by overrides, which recurses for ever in
//! `addRootSciences`; an integer out of range of `%d`/`%u`, see
//! `common::ini`) are **not** given to the C++; their count is printed, and a
//! session stops at such a step (the steps before it are compared).
//!
//! Each comparison also runs a planted bug, which must be caught.
//!
//! The methods that take a `Player` (`playerHasPrereqsForScience`,
//! `playerHasRootPrereqsForScience`, `getPurchasableSciences`) are layer 7:
//! not translated, not tested.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::ini::{BlockParse, FieldParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Thrown, UserData, TYPE_TABLE};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::science::{
    xfer_science_type, xfer_science_vec, GameTextFetch, ScienceStore, ScienceType, ScienceVec, SCIENCE_INVALID,
};
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::rank_info::RankInfoStore;
use std::collections::BTreeMap;
use std::ffi::CString;
use support::{Check, Gen};

/// The C++ side has process-global state (the name keys, the stores, `strtok`,
/// `XferLoad`'s static buffer), so the tests take turns.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// ---------------------------------------------------------------------------
// The Rust side of the INI blocks
// ---------------------------------------------------------------------------

/// The stand-in game text of `wrap_science.cpp`.
struct StandInText;

impl GameTextFetch for StandInText {
    fn fetch(&self, label: &[u8]) -> UnicodeString {
        if label.len() >= 6 && label[..6].eq_ignore_ascii_case(b"NOTEXT") {
            return UnicodeString::new();
        }
        let mut units: Vec<u16> = vec![u16::from(b'T'), u16::from(b':')];
        units.extend(label.iter().map(|&b| u16::from(b)));
        UnicodeString::try_from_chars(&units).unwrap()
    }
}

/// One `WebpageURL` block: `Science =` and `Sciences =`.
#[derive(Clone, Debug)]
struct WebRec {
    one: ScienceType,
    vec: ScienceVec,
}

struct Ctx {
    sciences: ScienceStore,
    ranks: RankInfoStore,
    names: NameKeyGenerator,
    web: Vec<WebRec>,
}

impl Ctx {
    fn new() -> Self {
        let mut names = NameKeyGenerator::new();
        names.reset();
        Ctx { sciences: ScienceStore::new(), ranks: RankInfoStore::new(), names, web: Vec::new() }
    }
}

fn h_science(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.sciences.parse_science_definition(ini, &mut c.names, &StandInText)
}
fn h_rank(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.ranks.parse_rank_definition(ini, &mut c.names, &c.sciences, &StandInText)
}

struct WebFill<'a> {
    rec: &'a mut WebRec,
    names: &'a mut NameKeyGenerator,
    sciences: &'a ScienceStore,
}
fn w_one(ini: &mut Ini<'_>, w: &mut WebFill<'_>, _: &UserData) -> IniResult<()> {
    w.rec.one = w.sciences.parse_science(ini, w.names)?;
    Ok(())
}
fn w_vec(ini: &mut Ini<'_>, w: &mut WebFill<'_>, _: &UserData) -> IniResult<()> {
    w.sciences.parse_science_vector(ini, w.names, &mut w.rec.vec)
}
/// The `WebpageURL` handler of `wrap_science.cpp`.
fn h_web(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.web.push(WebRec { one: SCIENCE_INVALID, vec: Vec::new() });
    let rec = c.web.last_mut().unwrap();
    let mut fill = WebFill { rec, names: &mut c.names, sciences: &c.sciences };
    let table = [
        FieldParse::new("Science", w_one, UserData::None),
        FieldParse::new("Sciences", w_vec, UserData::None),
        FieldParse::end(),
    ];
    ini.init_from_ini(&mut fill, &table)
}
fn h_stub(_: &mut Ini<'_>, _: &mut Ctx) -> IniResult<()> {
    Err(IniError::new(IniErrorKind::Bug))
}

/// The block table: `Science`, `Rank` and `WebpageURL` are the handlers
/// under test; the other 59 throw `ERROR_BUG`, as the stubs of wrap_ini.cpp.
fn blocks() -> Vec<BlockParse<Ctx>> {
    TYPE_TABLE
        .iter()
        .map(|&(token, _, _)| BlockParse {
            token,
            parse: match token {
                "Science" => h_science,
                "Rank" => h_rank,
                "WebpageURL" => h_web,
                _ => h_stub,
            },
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The dumps (the formats of wrap_science.cpp)
// ---------------------------------------------------------------------------

fn put_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_i32(v: &mut Vec<u8>, x: i32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_uni(v: &mut Vec<u8>, s: &UnicodeString) {
    put_u32(v, s.str().len() as u32);
    for u in s.str() {
        v.extend_from_slice(&u.to_le_bytes());
    }
}
fn put_vec(v: &mut Vec<u8>, s: &[ScienceType]) {
    put_u32(v, s.len() as u32);
    for x in s {
        put_i32(v, x.0);
    }
}

/// `ref_science_dump`. `bug` plants a wrong Rust: 1 leaves the root
/// sciences out, 2 flips `grantable` on override nodes.
fn rust_science_dump(s: &ScienceStore, bug: u32) -> Vec<u8> {
    let mut v = Vec::new();
    put_u32(&mut v, s.len() as u32);
    for head in s.chains() {
        let mut chain = vec![head];
        while let Some(n) = chain.last().unwrap().get_next_override() {
            chain.push(n);
        }
        put_u32(&mut v, chain.len() as u32);
        for node in chain {
            let d = &node.data;
            v.push(u8::from(node.is_override()));
            put_i32(&mut v, d.science().0);
            put_uni(&mut v, d.name());
            put_uni(&mut v, d.description());
            put_vec(&mut v, if bug == 1 { &[] } else { d.root_sciences() });
            put_vec(&mut v, d.prereq_sciences());
            put_i32(&mut v, d.science_purchase_point_cost());
            v.push(u8::from(d.grantable() ^ (bug == 2 && node.is_override())));
        }
    }
    v
}

/// `ref_rank_dump`. `bug` 1 swaps the two numbers, 2 reports no override flag.
fn rust_rank_dump(r: &RankInfoStore, bug: u32) -> Vec<u8> {
    let mut v = Vec::new();
    put_u32(&mut v, r.get_rank_level_count() as u32);
    for head in r.chains() {
        let mut chain = vec![head];
        while let Some(n) = chain.last().unwrap().get_next_override() {
            chain.push(n);
        }
        put_u32(&mut v, chain.len() as u32);
        for node in chain {
            let d = &node.data;
            v.push(u8::from(node.is_override() && bug != 2));
            put_uni(&mut v, &d.rank_name);
            let (a, b) = (d.skill_points_needed, d.science_purchase_points_granted);
            put_i32(&mut v, if bug == 1 { b } else { a });
            put_i32(&mut v, if bug == 1 { a } else { b });
            put_vec(&mut v, &d.sciences_granted);
        }
    }
    v
}

/// `ref_science_web_dump`. `bug` 1 drops the first science of each vector.
fn rust_web_dump(web: &[WebRec], bug: u32) -> Vec<u8> {
    let mut v = Vec::new();
    put_u32(&mut v, web.len() as u32);
    for w in web {
        put_i32(&mut v, w.one.0);
        put_vec(&mut v, if bug == 1 && !w.vec.is_empty() { &w.vec[1..] } else { &w.vec });
    }
    v
}

/// `ref_science_probe`. `bug` 1 reports every science as grantable.
fn rust_probe(c: &Ctx, st: i32, bug: u32) -> Vec<u8> {
    let s = ScienceType(st);
    let mut v = vec![u8::from(c.sciences.is_valid_science(s)), u8::from(c.sciences.is_science_grantable(s) || bug == 1)];
    let nd = c.sciences.get_name_and_description(s);
    v.push(u8::from(nd.is_some()));
    let (n, d) = nd.unwrap_or_default();
    put_uni(&mut v, &n);
    put_uni(&mut v, &d);
    put_i32(&mut v, c.sciences.get_science_purchase_cost(s));
    let name = c.sciences.get_internal_name_for_science(&c.names, s);
    put_u32(&mut v, name.len() as u32);
    v.extend_from_slice(&name);
    v
}

/// `ref_science_name_probe`: `getScienceFromInternalName` (a new name gets a
/// key), then `friend_lookupScience`.
fn rust_name_probe(c: &mut Ctx, name: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    put_i32(&mut v, c.sciences.get_science_from_internal_name(&mut c.names, name).unwrap().0);
    match c.sciences.friend_lookup_science(&mut c.names, name) {
        Ok(s) => {
            put_i32(&mut v, s.0);
            v.push(1);
        }
        Err(e) => {
            assert_eq!(e.kind, IniErrorKind::InvalidData);
            v.push(0);
        }
    }
    v
}

/// `ref_science_names`.
fn rust_names(c: &Ctx) -> Vec<u8> {
    let names = c.sciences.friend_get_science_names(&c.names);
    let mut v = Vec::new();
    put_u32(&mut v, names.len() as u32);
    for n in names {
        put_u32(&mut v, n.len() as u32);
        v.extend_from_slice(&n);
    }
    v
}

/// `ref_rank_probe`. `bug` 1 numbers levels from 0.
fn rust_rank_probe(c: &Ctx, level: i32, bug: u32) -> Vec<u8> {
    let mut v = Vec::new();
    put_i32(&mut v, c.ranks.get_rank_level_count());
    let level = if bug == 1 { level + 1 } else { level };
    match c.ranks.get_rank_info(level) {
        None => v.push(0),
        Some(r) => {
            v.push(1);
            put_uni(&mut v, &r.rank_name);
            put_i32(&mut v, r.skill_points_needed);
            put_i32(&mut v, r.science_purchase_points_granted);
            put_vec(&mut v, &r.sciences_granted);
        }
    }
    v
}

fn cpp_buf(f: impl FnOnce(*mut u8, i32) -> i32) -> Vec<u8> {
    let mut buf = vec![0u8; 1 << 20];
    let n = f(buf.as_mut_ptr(), buf.len() as i32);
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

/// Names differing only in case are different sciences (`nameToKey` is `strcmp`).
const SCI_POOL: [&str; 10] = [
    "Science_A", "Science_B", "Science_C", "Science_D", "Science_E", "Science_F", "SCIENCE_A", "science_b", "Science_G", "Science_H",
];
const GHOSTS: [&str; 4] = ["Science_Ghost", "Nope", "science_a", "Science_AA"];
const LABELS: [&str; 10] = [
    "SCIENCE:Alpha", "SCIENCE:Beta", "Label_A", "X", "SCIENCE:Gamma", "Rank:Recruit", "Rank:Colonel", "NOTEXT_Missing", "notext", "NoTextLabel",
];

fn gen_sep(g: &mut Gen) -> &'static str {
    pick(g, &[" = ", "=", " ", " =", "= ", "\t= ", " = ", " = "])
}

/// What a file draws on: the sciences defined so far (the store's, then those
/// of the blocks already in the file), and whether it may contain mistakes.
struct Known {
    names: Vec<String>,
    clean: bool,
}

impl Known {
    /// A science that exists, when there is one (else a pool name, which a
    /// non-clean file may use anyway).
    fn existing(&self, g: &mut Gen) -> Option<String> {
        if !self.names.is_empty() && (self.clean || g.below(10) < 7) {
            Some(self.names[g.below(self.names.len() as u32) as usize].clone())
        } else if self.clean {
            None
        } else {
            Some(pick(g, &SCI_POOL).to_string())
        }
    }

    /// A name for a new definition: one nothing defines yet, when there is one.
    fn fresh(&self, g: &mut Gen) -> String {
        let free: Vec<&str> = SCI_POOL.iter().copied().filter(|n| !self.names.iter().any(|k| k == n)).collect();
        if !free.is_empty() && (self.clean || g.below(10) < 8) {
            free[g.below(free.len() as u32) as usize].to_string()
        } else {
            pick(g, &SCI_POOL).to_string()
        }
    }

    fn add(&mut self, name: &str) {
        if !self.names.iter().any(|k| k == name) {
            self.names.push(name.to_string());
        }
    }
}

/// The value of a science list field; `not` is a name to leave out of a clean list.
fn gen_sci_list(g: &mut Gen, k: &Known, not: &str) -> String {
    let one = |g: &mut Gen| -> Option<String> {
        for _ in 0..6 {
            if let Some(n) = k.existing(g) {
                if !k.clean || n != not {
                    return Some(n);
                }
            }
        }
        None
    };
    if !k.clean {
        match g.below(20) {
            0 => return pick(g, &["None", "none", "NONE", "nOnE"]).to_string(),
            1 => return format!("{} None", one(g).unwrap_or_default()),
            2 => return format!("None {}", one(g).unwrap_or_default()),
            3 => return pick(g, &GHOSTS).to_string(),
            4 => return format!("{} {}", one(g).unwrap_or_default(), pick(g, &GHOSTS)),
            5 => return String::new(),
            6 => return format!("{},{}", one(g).unwrap_or_default(), one(g).unwrap_or_default()),
            7 => {
                let a = one(g).unwrap_or_default();
                return format!("{a} {a}");
            }
            8 => return format!("{}={}", one(g).unwrap_or_default(), one(g).unwrap_or_default()),
            _ => {}
        }
    }
    let n = 1 + g.below(3);
    let list: Vec<String> = (0..n).filter_map(|_| one(g)).collect();
    if list.is_empty() {
        return pick(g, &["None", "none"]).to_string();
    }
    list.join(pick(g, &[" ", " ", "\t", "  "]))
}

fn gen_int(g: &mut Gen, clean: bool) -> String {
    match g.below(16) {
        0..=7 => format!("{}", g.below(20)),
        8 => pick(g, &["0", "1", "007", "-3", "2147483647", "-2147483648"]).to_string(),
        _ if clean => format!("{}", g.below(5000)),
        9 => pick(g, &["1e3", "12abc", "+5"]).to_string(),
        10 => pick(g, &["abc", "", "x1", "-"]).to_string(),
        11 => pick(g, &["99999999999", "2147483648", "-2147483649"]).to_string(),
        _ => format!("{}", g.below(5000)),
    }
}

fn gen_uint(g: &mut Gen, clean: bool) -> String {
    match g.below(16) {
        0..=7 => format!("{}", g.below(10)),
        8 => pick(g, &["0", "4294967295", "-1", "2147483648", "4294967294"]).to_string(),
        _ if clean => format!("{}", g.below(100)),
        9 => pick(g, &["abc", "", "5x"]).to_string(),
        10 => pick(g, &["4294967296", "99999999999"]).to_string(),
        _ => format!("{}", g.below(100)),
    }
}

fn gen_bool(g: &mut Gen, clean: bool) -> String {
    if clean || g.below(11) < 6 {
        return pick(g, &["Yes", "No", "yes", "NO", "Yes", "No"]).to_string();
    }
    pick(g, &["1", "0", "True", "maybe", ""]).to_string()
}

fn gen_label(g: &mut Gen, clean: bool) -> String {
    if clean {
        return pick(g, &LABELS[..7]).to_string();
    }
    if g.below(15) == 0 {
        return String::new();
    }
    pick(g, &LABELS).to_string()
}

fn gen_comment(g: &mut Gen) -> &'static str {
    if g.below(8) == 0 {
        pick(g, &[" ; comment", ";", " ;x=1 2"])
    } else {
        ""
    }
}

fn gen_indent(g: &mut Gen) -> &'static str {
    pick(g, &["", "  ", "\t", " \t "])
}

fn gen_end(g: &mut Gen, clean: bool, lines: &mut Vec<String>) {
    if clean || g.below(15) != 0 {
        lines.push(pick(g, &["End", "End", "End", "END", "end", "  End  ", "End ; done"]).to_string());
    }
}

fn gen_science_block(g: &mut Gen, lines: &mut Vec<String>, k: &mut Known, overrides: bool) {
    let name = if overrides && g.below(10) < 7 {
        k.existing(g).unwrap_or_else(|| k.fresh(g))
    } else {
        k.fresh(g)
    };
    lines.push(format!("Science {name}").trim_end().to_string());
    for _ in 0..g.below(7) {
        let sep = gen_sep(g);
        let body = match g.below(30) {
            0 if !k.clean => pick(g, &["Foo = 1", "PrerequisiteSciences", "IsGrantable =", "DisplayName", "Description =", "prerequisitesciences = Science_A", "Dead = 1"]).to_string(),
            0..=7 => format!("PrerequisiteSciences{sep}{}", gen_sci_list(g, k, &name)),
            8..=12 => format!("SciencePurchasePointCost{sep}{}", gen_int(g, k.clean)),
            13..=16 => format!("IsGrantable{sep}{}", gen_bool(g, k.clean)),
            17..=22 => format!("DisplayName{sep}{}", gen_label(g, k.clean)),
            _ => format!("Description{sep}{}", gen_label(g, k.clean)),
        };
        lines.push(format!("{}{body}{}", gen_indent(g), gen_comment(g)));
    }
    gen_end(g, k.clean, lines);
    k.add(&name);
}

fn gen_rank_block(g: &mut Gen, lines: &mut Vec<String>, k: &Known, overrides: bool, n_ranks: i32, next_rank: &mut i32) {
    let level: String = if overrides {
        if n_ranks > 0 && (k.clean || g.below(10) < 8) {
            format!("{}", 1 + g.below(n_ranks as u32))
        } else {
            pick(g, &["0", "-1", "99", "1", "2"]).to_string()
        }
    } else if k.clean || g.below(10) < 8 {
        let l = *next_rank;
        *next_rank += 1;
        format!("{l}")
    } else {
        pick(g, &["0", "-1", "99", "1", "abc", "", "2", "99999999999"]).to_string()
    };
    lines.push(format!("Rank {level}").trim_end().to_string());
    for _ in 0..g.below(6) {
        let sep = gen_sep(g);
        let body = match g.below(26) {
            0 if !k.clean => pick(g, &["Foo = 1", "RankName", "SkillPointsNeeded =", "SciencesGranted"]).to_string(),
            0..=6 => format!("RankName{sep}{}", gen_label(g, k.clean)),
            7..=12 => format!("SkillPointsNeeded{sep}{}", gen_int(g, k.clean)),
            13..=19 => format!("SciencesGranted{sep}{}", gen_sci_list(g, k, "")),
            _ => format!("SciencePurchasePointsGranted{sep}{}", gen_uint(g, k.clean)),
        };
        lines.push(format!("{}{body}{}", gen_indent(g), gen_comment(g)));
    }
    gen_end(g, k.clean, lines);
}

fn gen_web_block(g: &mut Gen, lines: &mut Vec<String>, k: &Known) {
    lines.push("WebpageURL".to_string());
    for _ in 0..g.below(4) {
        let sep = gen_sep(g);
        let body = match g.below(12) {
            0 if !k.clean => pick(g, &["Foo = 1", "Science", "Sciences"]).to_string(),
            0..=5 => {
                let one = if k.clean { k.existing(g) } else { None };
                let v = match one {
                    Some(n) => n,
                    None => {
                        let n = k.existing(g).unwrap_or_default();
                        pick(g, &[n.as_str(), n.as_str(), "None", "Science_Ghost", ""]).to_string()
                    }
                };
                format!("Science{sep}{v}")
            }
            _ => format!("Sciences{sep}{}", gen_sci_list(g, k, "")),
        };
        lines.push(format!("{}{body}{}", gen_indent(g), gen_comment(g)));
    }
    gen_end(g, k.clean, lines);
}

/// One file; `known` are the sciences defined so far, `n_ranks` the ranks.
/// A share of the files (`clean`) holds only well-formed blocks.
fn gen_file(g: &mut Gen, known: &[String], n_ranks: i32, overrides: bool) -> Vec<u8> {
    let clean = g.below(10) < 4;
    let mut k = Known { names: known.to_vec(), clean };
    let mut lines = Vec::new();
    let mut next_rank = n_ranks + 1;
    for _ in 0..1 + g.below(5) {
        match g.below(20) {
            0..=9 => gen_science_block(g, &mut lines, &mut k, overrides),
            10..=14 => gen_rank_block(g, &mut lines, &k, overrides, n_ranks, &mut next_rank),
            15..=17 => gen_web_block(g, &mut lines, &k),
            18 => lines.push(pick(g, &["; a comment", "", "   ", ";", "\t"]).to_string()),
            _ if !clean => lines.push(pick(g, &["Unknown block", "=", "  =  ", "End", "AIData"]).to_string()),
            _ => lines.push("; noise".to_string()),
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
    if !clean && g.below(14) == 0 && !out.is_empty() {
        for _ in 0..1 + g.below(3) {
            if out.is_empty() {
                break;
            }
            let p = g.below(out.len() as u32) as usize;
            match g.below(4) {
                0 => out[p] = g.u32() as u8,
                1 => out.insert(p, g.u32() as u8),
                2 => {
                    out.remove(p);
                }
                _ => out.insert(p, 0x80 + g.below(0x80) as u8),
            }
        }
    }
    out
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
    Ok(Outcome { status, detail, crc })
}

fn cpp_load(name: &[u8], data: &[u8], load_type: IniLoadType, with_xfer: bool) -> Outcome {
    let cname = CString::new(name.to_vec()).unwrap();
    let mut msg = vec![0u8; 4096];
    let (mut crc, mut code) = (0u32, 0u32);
    let status = unsafe {
        cpp::ref_ini_reset();
        cpp::ref_ini_add_file(cname.as_ptr(), data.as_ptr() as *const _, data.len() as i32);
        cpp::ref_science_load(cname.as_ptr(), load_type as i32, i32::from(with_xfer), &mut crc, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
    };
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

/// Name probes: the pool, ghosts, the empty name, a long name.
fn name_probes() -> Vec<String> {
    let mut v: Vec<String> = SCI_POOL.iter().map(|s| s.to_string()).collect();
    v.extend(GHOSTS.iter().map(|s| s.to_string()));
    v.push(String::new());
    v.push("Probe_Only_Once".to_string());
    v
}

/// The comparisons of one session.
struct Checks {
    outcome: Check,
    outcome_bug: Check,
    sci: Check,
    sci_bug_roots: Check,
    sci_bug_flag: Check,
    rank: Check,
    rank_bug_swap: Check,
    rank_bug_flag: Check,
    web: Check,
    web_bug: Check,
    probe: Check,
    probe_bug: Check,
    names: Check,
    names_bug: Check,
    name_list: Check,
    rank_probe: Check,
    rank_probe_bug: Check,
}

impl Checks {
    fn new() -> Self {
        Checks {
            outcome: Check::new("Science/Rank INI::load (throw, message, CRC)"),
            outcome_bug: Check::new("INI::load"),
            sci: Check::new("ScienceStore (every node of every chain)"),
            sci_bug_roots: Check::new("ScienceStore"),
            sci_bug_flag: Check::new("ScienceStore"),
            rank: Check::new("RankInfoStore (every node of every chain)"),
            rank_bug_swap: Check::new("RankInfoStore"),
            rank_bug_flag: Check::new("RankInfoStore"),
            web: Check::new("INI::parseScience/parseScienceVector (records)"),
            web_bug: Check::new("parseScienceVector"),
            probe: Check::new("ScienceStore lookups of an id"),
            probe_bug: Check::new("ScienceStore lookups"),
            names: Check::new("getScienceFromInternalName, friend_lookupScience"),
            names_bug: Check::new("getScienceFromInternalName"),
            name_list: Check::new("friend_getScienceNames"),
            rank_probe: Check::new("getRankLevelCount, getRankInfo"),
            rank_probe_bug: Check::new("getRankInfo"),
        }
    }
}

#[derive(Default)]
struct Tally {
    sessions: u64,
    loads: u64,
    loads_ok: u64,
    loads_failed: u64,
    overrides_loads: u64,
    resets: u64,
    inits: u64,
    sciences: u64,
    chains_over_1: u64,
    science_nodes_with_roots: u64,
    multi_root_nodes: u64,
    ranks: u64,
    rank_overrides: u64,
    web_records: u64,
    probes: u64,
    stopped_undefined: u64,
}

#[test]
fn science_and_rank_blocks_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let table = blocks();
    let probes = name_probes();
    let mut g = Gen::new(0x5C1E);
    let mut ck = Checks::new();
    let mut excluded = Excluded::default();
    let mut t = Tally::default();
    let mut ctx = Ctx::new();
    const SESSIONS: usize = 3_000;
    for session in 0..SESSIONS {
        ctx.sciences = ScienceStore::new();
        ctx.ranks = RankInfoStore::new();
        ctx.names.reset();
        ctx.web.clear();
        unsafe { cpp::ref_science_reset() };
        t.sessions += 1;
        let steps = 2 + g.below(4);
        for step in 0..steps {
            // 0 overwrite load, 1 create-overrides load, 2 reset, 3 init
            let kind = if step == 0 {
                0
            } else {
                match g.below(20) {
                    0..=8 => 1,
                    9..=14 => 0,
                    15..=18 => 2,
                    _ => 3,
                }
            };
            let tag = format!("session {session} step {step}");
            match kind {
                2 | 3 => {
                    if kind == 2 {
                        ctx.sciences.reset();
                        ctx.ranks.reset();
                        t.resets += 1;
                    } else {
                        ctx.sciences.init();
                        ctx.ranks.init();
                        t.inits += 1;
                    }
                    unsafe { cpp::ref_science_op(if kind == 2 { 0 } else { 1 }) };
                }
                _ => {
                    let known: Vec<String> = ctx.sciences.friend_get_science_names(&ctx.names).iter().map(|n| show(n)).collect();
                    let overrides = kind == 1;
                    let data = gen_file(&mut g, &known, ctx.ranks.get_rank_level_count(), overrides);
                    let load_type = if overrides {
                        IniLoadType::CreateOverrides
                    } else if g.below(12) == 0 {
                        IniLoadType::Multifile
                    } else {
                        IniLoadType::Overwrite
                    };
                    let name = format!("Data\\INI\\Case{}.ini", session % 7).into_bytes();
                    let with_xfer = g.below(4) != 0;
                    let mut crc = XferCrc::new_without_nan_guard();
                    let r = Ini::load(
                        &name,
                        &data,
                        load_type,
                        if with_xfer { Some(&mut crc as &mut dyn Xfer) } else { None },
                        &table,
                        &mut ctx,
                    );
                    let crc_v = if with_xfer { crc.get_crc() } else { 0 };
                    let rust = match rust_outcome(r, crc_v) {
                        Ok(o) => o,
                        Err(e) => {
                            excluded.add(&e);
                            t.stopped_undefined += 1;
                            break;
                        }
                    };
                    t.loads += 1;
                    if overrides {
                        t.overrides_loads += 1;
                    }
                    if rust.status == 0 { t.loads_ok += 1 } else { t.loads_failed += 1 }
                    let c = cpp_load(&name, &data, load_type, with_xfer);
                    ck.outcome.eq(&rust, &c, || format!("{tag}: {load_type:?} file {:?}", show(&data)));
                    let wrong = Outcome { status: rust.status ^ 4, detail: rust.detail.clone(), crc: rust.crc };
                    ck.outcome_bug.eq(&wrong, &c, || tag.clone());
                }
            }

            // the stores, after the step
            let file = || tag.clone();
            let cs = cpp_buf(|b, n| unsafe { cpp::ref_science_dump(b, n) });
            ck.sci.eq(rust_science_dump(&ctx.sciences, 0), cs.clone(), file);
            ck.sci_bug_roots.eq(rust_science_dump(&ctx.sciences, 1), cs.clone(), file);
            ck.sci_bug_flag.eq(rust_science_dump(&ctx.sciences, 2), cs, file);
            let cr = cpp_buf(|b, n| unsafe { cpp::ref_rank_dump(b, n) });
            ck.rank.eq(rust_rank_dump(&ctx.ranks, 0), cr.clone(), file);
            ck.rank_bug_swap.eq(rust_rank_dump(&ctx.ranks, 1), cr.clone(), file);
            ck.rank_bug_flag.eq(rust_rank_dump(&ctx.ranks, 2), cr, file);
            let cw = cpp_buf(|b, n| unsafe { cpp::ref_science_web_dump(b, n) });
            ck.web.eq(rust_web_dump(&ctx.web, 0), cw.clone(), file);
            ck.web_bug.eq(rust_web_dump(&ctx.web, 1), cw, file);

            // tallies of what was compared
            t.web_records += ctx.web.len() as u64;
            t.sciences += ctx.sciences.len() as u64;
            for head in ctx.sciences.chains() {
                let mut n = 0;
                let mut cur = Some(head);
                while let Some(node) = cur {
                    n += 1;
                    if !node.data.root_sciences().is_empty() {
                        t.science_nodes_with_roots += 1;
                        if node.data.root_sciences().len() > 1 {
                            t.multi_root_nodes += 1;
                        }
                    }
                    cur = node.get_next_override();
                }
                if n > 1 {
                    t.chains_over_1 += 1;
                }
            }
            t.ranks += ctx.ranks.get_rank_level_count() as u64;
            t.rank_overrides += ctx.ranks.chains().filter(|h| h.get_next_override().is_some()).count() as u64;

            // the lookups, after the step (they give unseen names their keys)
            for st in (-3..16).chain([1000, i32::MAX, i32::MIN]) {
                let cp = cpp_buf(|b, n| unsafe { cpp::ref_science_probe(st, b, n) });
                ck.probe.eq(rust_probe(&ctx, st, 0), cp.clone(), || format!("{tag}: id {st}"));
                ck.probe_bug.eq(rust_probe(&ctx, st, 1), cp, || format!("{tag}: id {st}"));
                t.probes += 1;
            }
            for (i, n) in probes.iter().enumerate() {
                if !(i as u32 + step + session as u32).is_multiple_of(3) && n != "Probe_Only_Once" {
                    continue;
                }
                let cn = CString::new(n.as_str()).unwrap();
                let cp = cpp_buf(|b, cap| unsafe { cpp::ref_science_name_probe(cn.as_ptr(), b, cap) });
                let rp = rust_name_probe(&mut ctx, n.as_bytes());
                ck.names.eq(rp.clone(), cp.clone(), || format!("{tag}: name {n:?}"));
                let mut wrong = rp;
                wrong[0] ^= 1;
                ck.names_bug.eq(wrong, cp, || format!("{tag}: name {n:?}"));
                t.probes += 1;
            }
            let cn = cpp_buf(|b, cap| unsafe { cpp::ref_science_names(b, cap) });
            ck.name_list.eq(rust_names(&ctx), cn, || tag.clone());
            for level in -1..ctx.ranks.get_rank_level_count() + 3 {
                let cp = cpp_buf(|b, cap| unsafe { cpp::ref_rank_probe(level, b, cap) });
                ck.rank_probe.eq(rust_rank_probe(&ctx, level, 0), cp.clone(), || format!("{tag}: level {level}"));
                ck.rank_probe_bug.eq(rust_rank_probe(&ctx, level, 1), cp, || format!("{tag}: level {level}"));
                t.probes += 1;
            }
        }
    }
    println!(
        "Science/Rank INI: {} sessions, {} loads ({} overrides, {} ok, {} threw), {} resets, {} inits; \
         compared {} sciences, {} chains with overrides, {} nodes with roots ({} with several), {} ranks, {} ranks with overrides, {} WebpageURL records, {} lookups; \
         sessions stopped at an undefined step: {} {:?}",
        t.sessions, t.loads, t.overrides_loads, t.loads_ok, t.loads_failed, t.resets, t.inits, t.sciences, t.chains_over_1,
        t.science_nodes_with_roots, t.multi_root_nodes, t.ranks, t.rank_overrides, t.web_records, t.probes, excluded.total(), excluded.0
    );
    assert!(t.loads_ok > 2000 && t.loads_failed > 2000, "the generator must reach both outcomes: {} ok, {} threw", t.loads_ok, t.loads_failed);
    assert!(t.sciences > 5000 && t.chains_over_1 > 500 && t.multi_root_nodes > 200, "too few sciences, overrides or roots compared");
    assert!(t.ranks > 2000 && t.rank_overrides > 300 && t.web_records > 2000, "too few ranks or records compared");
    ck.outcome.finish();
    ck.outcome_bug.finish_planted();
    ck.sci.finish();
    ck.sci_bug_roots.finish_planted();
    ck.sci_bug_flag.finish_planted();
    ck.rank.finish();
    ck.rank_bug_swap.finish_planted();
    ck.rank_bug_flag.finish_planted();
    ck.web.finish();
    ck.web_bug.finish_planted();
    ck.probe.finish();
    ck.probe_bug.finish_planted();
    ck.names.finish();
    ck.names_bug.finish_planted();
    ck.name_list.finish();
    ck.rank_probe.finish();
    ck.rank_probe_bug.finish_planted();
}

// ---------------------------------------------------------------------------
// xferScienceType / xferScienceVec
// ---------------------------------------------------------------------------

fn status_of<E: std::fmt::Debug>(r: Result<(), E>, as_status: impl Fn(&E) -> i32) -> i32 {
    match r {
        Ok(()) => 0,
        Err(e) => as_status(&e),
    }
}

fn xfer_status(e: &rts_lockstep_engine::common::science::ScienceXferError) -> i32 {
    match e {
        rts_lockstep_engine::common::science::ScienceXferError::Xfer(s) => *s as i32,
        other => panic!("{other}"),
    }
}

fn tmp_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("rts-lockstep-science-{}-{tag}.bin", std::process::id()))
}

/// Runs `xferScienceType` (kind 0, on `v[0]`) or `xferScienceVec` (kind 1).
fn rust_xfer(x: &mut dyn Xfer, names: &mut NameKeyGenerator, kind: i32, v: &mut ScienceVec) -> i32 {
    if kind == 0 {
        let mut s = v.first().copied().unwrap_or(SCIENCE_INVALID);
        let r = xfer_science_type(x, names, &mut s);
        *v = vec![s];
        status_of(r, xfer_status)
    } else {
        status_of(xfer_science_vec(x, names, v), xfer_status)
    }
}

fn random_vec(g: &mut Gen, keys: &[i32], max: u32) -> Vec<i32> {
    (0..g.below(max + 1))
        .map(|_| match g.below(12) {
            0 => -1,
            1 => 5000 + g.below(10) as i32,
            2 => g.below(40) as i32 - 3,
            _ => keys[g.below(keys.len() as u32) as usize],
        })
        .collect()
}

#[test]
fn science_xfer_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x5C1F);
    let path = tmp_path("xfer");
    let cpath = CString::new(path.to_str().unwrap()).unwrap();
    let mut save = Check::new("xferScienceType/Vec save (bytes, status)");
    let mut save_bug = Check::new("xferScienceVec save");
    let mut load = Check::new("xferScienceType/Vec load (result, status)");
    let mut load_bug = Check::new("xferScienceVec load");
    let mut crc = Check::new("xferScienceType/Vec CRC");
    let mut crc_bug = Check::new("xferScienceVec CRC");
    let mut keys_after = Check::new("name keys after the xfers");
    let (mut saved_ok, mut save_failed, mut load_ok, mut load_failed) = (0u64, 0u64, 0u64, 0u64);
    let mut names = NameKeyGenerator::new();
    names.reset();
    unsafe { cpp::ref_science_reset() };
    // some names have keys (as if defined by INI); the rest are born in the xfers
    let mut keys = Vec::new();
    for n in SCI_POOL.iter().take(6) {
        let cn = CString::new(*n).unwrap();
        let rk = names.name_to_key(n.as_bytes()).0 as i32;
        let ck = unsafe { cpp::ref_science_name_key(cn.as_ptr()) } as i32;
        assert_eq!(rk, ck);
        keys.push(rk);
    }
    const CASES: usize = 6_000;
    for case in 0..CASES {
        let kind = (case % 2) as i32;
        let vals = random_vec(&mut g, &keys, if kind == 0 { 1 } else { 7 });
        let mut v: ScienceVec = vals.iter().map(|&x| ScienceType(x)).collect();

        // save
        let mut cbytes = vec![0u8; 4096];
        let mut clen = 0i32;
        let cst = unsafe { cpp::ref_science_save(kind, vals.as_ptr(), vals.len() as i32, cpath.as_ptr(), cbytes.as_mut_ptr(), 4096, &mut clen) };
        cbytes.truncate(clen as usize);
        let mut xs = XferSave::new();
        xs.open(b"rust").unwrap();
        let rst = rust_xfer(&mut xs, &mut names, kind, &mut v);
        let _ = xs.close();
        let rbytes = xs.bytes().to_vec();
        if rst == 0 { saved_ok += 1 } else { save_failed += 1 }
        save.eq((rst, &rbytes), (cst, &cbytes), || format!("case {case} kind {kind}: {vals:?}"));
        let mut planted = rbytes.clone();
        if planted.len() > 3 {
            planted[3] ^= 1; // planted: a byte of the count or a name changes
        } else {
            planted.push(0);
        }
        save_bug.eq((rst, &planted), (cst, &cbytes), || format!("case {case}"));

        // load: the saved file, or a mutated one, into a random starting vector
        let mut file = cbytes.clone();
        match g.below(9) {
            1 if !file.is_empty() => file[0] = g.below(4) as u8, // the version byte
            2 if file.len() > 1 => file[1] = g.below(10) as u8, // the count
            3 if !file.is_empty() => {
                let p = g.below(file.len() as u32) as usize;
                file[p] = g.u32() as u8;
            }
            4 => file.truncate(file.len().saturating_sub(1 + g.below(6) as usize)),
            5 => file.extend((0..g.below(5)).map(|_| g.u32() as u8)),
            6 if file.len() > 4 => {
                // a name that nothing defines: its key is assigned by the load
                let p = 4 + g.below(file.len() as u32 - 4) as usize;
                file[p] = b'a' + g.below(26) as u8;
            }
            _ => {}
        }
        let start = random_vec(&mut g, &keys, 4);
        let mut cvals = vec![0i32; 64];
        cvals[..start.len()].copy_from_slice(&start);
        let mut cn = start.len() as i32;
        let cst = unsafe { cpp::ref_science_load_xfer(kind, cvals.as_mut_ptr(), 64, &mut cn, file.as_ptr(), file.len() as i32, cpath.as_ptr()) };
        let cres: Vec<i32> = cvals[..cn as usize].to_vec();
        let mut r: ScienceVec = start.iter().map(|&x| ScienceType(x)).collect();
        let mut xl = XferLoad::new(file.clone());
        xl.open(b"rust").unwrap();
        let rst = rust_xfer(&mut xl, &mut names, kind, &mut r);
        let rres: Vec<i32> = r.iter().map(|s| s.0).collect();
        if rst == 0 { load_ok += 1 } else { load_failed += 1 }
        load.eq((rst, rres.clone()), (cst, cres.clone()), || format!("case {case} kind {kind}: file {file:?} start {start:?}"));
        let mut wrong = rres.clone();
        if let Some(x) = wrong.first_mut() {
            *x += 1; // planted: the first science is off by one
        } else {
            wrong.push(0);
        }
        load_bug.eq((rst, wrong), (cst, cres), || format!("case {case}"));
        // the keys the load assigned: the next unseen name gets the same key
        let probe = format!("Fresh_Name_{case}");
        let cprobe = CString::new(probe.as_str()).unwrap();
        keys_after.eq(names.name_to_key(probe.as_bytes()).0, unsafe { cpp::ref_science_name_key(cprobe.as_ptr()) }, || format!("case {case}"));

        // CRC
        let mut cc = 7u32;
        let cst = unsafe { cpp::ref_science_crc(kind, vals.as_ptr(), vals.len() as i32, &mut cc) };
        let mut xc = XferCrc::new_without_nan_guard();
        xc.open(b"crc").unwrap();
        let mut v2: ScienceVec = vals.iter().map(|&x| ScienceType(x)).collect();
        let rst = rust_xfer(&mut xc, &mut names, kind, &mut v2);
        crc.eq((rst, xc.get_crc()), (cst, cc), || format!("case {case} kind {kind}: {vals:?}"));
        crc_bug.eq((rst, xc.get_crc() ^ 1), (cst, cc), || format!("case {case}"));
    }
    let _ = std::fs::remove_file(&path);
    println!("Science xfer: {CASES} vectors; saves {saved_ok} ok, {save_failed} failed; loads {load_ok} ok, {load_failed} failed");
    assert!(saved_ok > 5000 && load_ok > 1500 && load_failed > 500);
    save.finish();
    save_bug.finish_planted();
    load.finish();
    load_bug.finish_planted();
    crc.finish();
    crc_bug.finish_planted();
    keys_after.finish();
}
