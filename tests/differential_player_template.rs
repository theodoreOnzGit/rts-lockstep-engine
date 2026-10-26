// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of Phase 1 layer 4's `PlayerTemplate` and
//! `PlayerTemplateStore` (`common::player_template`) against upstream
//! `PlayerTemplate.cpp` (commit 0a05454d), compiled unchanged as
//! `differential/wrap_playertemplate.cpp` describes, with the real
//! `INI::load`, `INI::parseAsciiString`/`parseBool`/`parseInt`/`parseRGBColor`/
//! `parseAndTranslateLabel`/`parseScienceVector`/`scanPercentToReal`/
//! `scanIndexList`, `NameKeyGenerator`, `Money::deposit`, `Handicap` and the
//! real `ScienceStore` underneath.
//!
//! No game INI files are available, so every file is **synthetic**:
//! `PlayerTemplate` blocks (all 45 fields in several forms, new names, names
//! seen before, names of the old maps' sub-factions, names differing only in
//! case, values well-formed and malformed: signs, partial and overflowing
//! numbers, colours out of range or out of order, percentages with and
//! without `%`, veterancy names in any case and unknown ones, quoted strings,
//! labels that translate and labels that translate to nothing, science lists
//! with known, unknown and `None` names), the `Science` blocks that define the
//! sciences those lists name, comments, `CRLF`, a missing `End`, unknown
//! fields and blocks, and in a share of files random byte damage. The game
//! text is a stand-in on both sides: a label translates to `T:` and its
//! bytes, and to nothing if it starts with `NOTEXT`.
//!
//! 1. `INI sessions`: each session is 2-5 steps against one pair of stores,
//!    each step a file loaded with `INI_LOAD_OVERWRITE` (or `MULTIFILE`,
//!    or `CREATE_OVERRIDES`, which the template block ignores and the science
//!    block does not), `PlayerTemplateStore::reset()` (which keeps the
//!    templates), `update()`, or `init()` (which clears them). A session
//!    starts from a name key generator that is reset: in a share of sessions
//!    the fifteen faction names are then looked up first, in the order of
//!    `findPlayerTemplate`'s statics, so that the old-map names are
//!    numbered as the statics hold them; in the others the numbering is
//!    whatever the file gives, and the statics (initialised once, by session
//!    0 and kept for the whole process in both programs, as upstream's are)
//!    are stale. Session 0's first file is fixed and starts with the block
//!    `FactionAmerica`: its first `findPlayerTemplate` is the one that
//!    initialises the statics, and the numbers they get are compared. After
//!    every step, compared: what `INI::load` throws (nothing, an
//!    `INIException` and its whole message, an `INI_*` status code, an
//!    `ErrorCode`), the INI `XferCRC`, **every field of every template** (name
//!    key and name, display name, sides, the four handicap multipliers, the
//!    money and its player index, the colour, the building, the ten starting
//!    units and the out-of-range ones, the cost, time and veterancy maps, the
//!    sciences, the command sets, the shortcut data, the music, the tooltip,
//!    the flags, the purchase points, the image names, the beacon, the
//!    features, the medallions), and the lookups (`getPlayerTemplateCount`,
//!    `getNthPlayerTemplate` for indices around the range, `findPlayerTemplate`
//!    for the keys of the old-map names, the template names, other names and raw
//!    numbers, `getTemplateNumByName` for the names with their case changed,
//!    `getAllSideStrings` onto lists that already hold sides). The name keys are
//!    lockstep state, so every lookup assigns them in the same order on both
//!    sides and the keys appear in the dumps.
//!
//! Inputs for which the Rust reports undefined behaviour upstream (an integer
//! out of range of `%d`, see `common::ini`) are **not** given to the C++;
//! their count is printed, and a session stops at such a step (the steps before
//! it are compared).
//!
//! Each comparison also runs a planted bug, which must be caught.
//!
//! `getAllSideStrings(NULL)` (returns at once) is run on the C++ side; it has
//! no Rust counterpart (no null lists). No method takes a `Player` or an
//! `Object`; the image getters are the names (a client cut point).
//!
//! # Results
//!
//! Recorded by `cargo test --release --features differential --test
//! differential_player_template -- --nocapture` (the `DIFF` and `PLANTED`
//! lines and the tally), 2026-10-18, upstream `0a05454d`, seed `0x97E5`:
//! 3,000 sessions (1,838 with the fifteen faction names numbered first), 8,317
//! file loads (1,517 with `INI_LOAD_CREATE_OVERRIDES`; 3,071 loaded, 5,246
//! threw), 605 `reset()`, 596 `update()` and 663 `init()` calls; **10,181 dumps
//! of the whole store** (9,791 template states, 2,702 with a non-zero
//! `StartMoney`, 5,508 with production maps, 1,216 with intrinsic sciences,
//! 1,275 observers, 4,179 with the name of an old map), **852,334 lookups**
//! (263,396 `findPlayerTemplate` by name, 17,233 of which found a template and
//! 6,148 of those by an old map's name; 711,360 finds with the raw keys; 40,334
//! `getNthPlayerTemplate`; 70,097 `getTemplateNumByName`; 30,543
//! `getAllSideStrings`) and the fifteen statics of session 0: **no
//! mismatch**. Each comparison also caught its planted bug (8,317 of 8,317
//! outcomes; 5,606, 3,286 and 5,606 of 10,181 dumps; 15,397 of 40,334 nth;
//! 25,817 of 711,360 finds; 20,012 of 70,097 names; 5,133 of 30,543 side
//! lists). 125 sessions stopped at a step the Rust reports as undefined
//! upstream (`IntOutOfRange`: a number outside `Int`, in `StartMoney`,
//! `SpecialPowerShortcutButtonCount` or `IntrinsicSciencePurchasePoints`).
//! Separately, bugs planted in `common::player_template` itself (no rename
//! by the block, no `Money::init()`, the old-map names all mapped to
//! America) each failed the store comparison and were removed.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::handicap::HandicapType;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Thrown, TYPE_TABLE};
use rts_lockstep_engine::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use rts_lockstep_engine::common::player_template::{
    PlayerTemplate, PlayerTemplateStore, LEGACY_FACTION_KEY_NAMES, MAX_MP_STARTING_UNITS,
};
use rts_lockstep_engine::common::science::{GameTextFetch, ScienceStore};
use rts_lockstep_engine::common::subsystem_interface::SubsystemInterface;
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::differential as cpp;
use std::collections::BTreeMap;
use std::ffi::CString;
use support::{Check, Gen};

/// The C++ side has process-global state (the name keys, the stores, `strtok`),
/// so the tests take turns.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// ---------------------------------------------------------------------------
// The Rust side of the INI blocks
// ---------------------------------------------------------------------------

/// The stand-in game text of `wrap_playertemplate.cpp`.
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

struct Ctx {
    /// The one store of the process: `init()` between sessions, never replaced
    /// (its fifteen statics live as long as the C++ ones).
    store: PlayerTemplateStore,
    sciences: ScienceStore,
    names: NameKeyGenerator,
}

impl Ctx {
    fn new() -> Self {
        let mut names = NameKeyGenerator::new();
        names.reset();
        Ctx { store: PlayerTemplateStore::new(), sciences: ScienceStore::new(), names }
    }
}

fn h_player_template(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.store.parse_player_template_definition(ini, &mut c.names, &c.sciences, &StandInText)
}
fn h_science(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.sciences.parse_science_definition(ini, &mut c.names, &StandInText)
}
fn h_stub(_: &mut Ini<'_>, _: &mut Ctx) -> IniResult<()> {
    Err(IniError::new(IniErrorKind::Bug))
}

/// The block table: `PlayerTemplate` and `Science` are the handlers under
/// test (the Science one is the translated `ScienceStore`, itself verified by
/// `differential_science`); the other 60 throw `ERROR_BUG`, as the stubs of
/// wrap_ini.cpp (the files never name them, except as noise).
fn blocks() -> Vec<BlockParse<Ctx>> {
    TYPE_TABLE
        .iter()
        .map(|&(token, _, _)| BlockParse {
            token,
            parse: match token {
                "PlayerTemplate" => h_player_template,
                "Science" => h_science,
                _ => h_stub,
            },
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The dumps (the format of wrap_playertemplate.cpp)
// ---------------------------------------------------------------------------

fn put_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_i32(v: &mut Vec<u8>, x: i32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_f32(v: &mut Vec<u8>, x: f32) {
    v.extend_from_slice(&x.to_bits().to_le_bytes());
}
fn put_ascii(v: &mut Vec<u8>, s: &[u8]) {
    put_u32(v, s.len() as u32);
    v.extend_from_slice(s);
}
fn put_uni(v: &mut Vec<u8>, s: &UnicodeString) {
    put_u32(v, s.str().len() as u32);
    for u in s.str() {
        v.extend_from_slice(&u.to_le_bytes());
    }
}

/// `dumpTemplate`. `bug` plants a wrong Rust: 1 flips `IsObserver`, 2 swaps the
/// cost and time maps, 3 numbers the template one higher.
fn rust_dump_template(v: &mut Vec<u8>, t: &PlayerTemplate, names: &NameKeyGenerator, bug: u32) {
    put_u32(v, t.name_key().0 + u32::from(bug == 3));
    put_ascii(v, t.name(names));
    put_uni(v, t.display_name());
    put_ascii(v, t.side());
    put_ascii(v, t.base_side());
    for ht in [HandicapType::BuildCost, HandicapType::BuildTime] {
        for is_structure in [false, true] {
            put_f32(v, t.handicap().get_handicap(ht, is_structure));
        }
    }
    put_u32(v, t.money().count_money());
    put_i32(v, t.money().player_index());
    put_f32(v, t.preferred_color().red);
    put_f32(v, t.preferred_color().green);
    put_f32(v, t.preferred_color().blue);
    put_ascii(v, t.starting_building());
    for i in -1..=MAX_MP_STARTING_UNITS as i32 {
        put_ascii(v, t.starting_unit(i));
    }
    let (cost, time) = if bug == 2 {
        (t.production_time_changes(), t.production_cost_changes())
    } else {
        (t.production_cost_changes(), t.production_time_changes())
    };
    for m in [cost, time] {
        put_u32(v, m.len() as u32);
        for (k, p) in m {
            put_u32(v, k.0);
            put_f32(v, *p);
        }
    }
    put_u32(v, t.production_veterancy_levels().len() as u32);
    for (k, level) in t.production_veterancy_levels() {
        put_u32(v, k.0);
        put_i32(v, *level as i32);
    }
    put_u32(v, t.intrinsic_sciences().len() as u32);
    for s in t.intrinsic_sciences() {
        put_i32(v, s.0);
    }
    put_ascii(v, t.purchase_science_command_set_rank1());
    put_ascii(v, t.purchase_science_command_set_rank3());
    put_ascii(v, t.purchase_science_command_set_rank8());
    put_ascii(v, t.special_power_shortcut_command_set());
    put_ascii(v, t.special_power_shortcut_win_name());
    put_i32(v, t.special_power_shortcut_button_count());
    put_ascii(v, t.load_screen_music());
    put_ascii(v, t.score_screen_music());
    put_ascii(v, t.tooltip());
    v.push(u8::from(t.is_observer() ^ (bug == 1)));
    v.push(u8::from(t.is_playable_side()));
    v.push(u8::from(t.is_old_faction()));
    put_i32(v, t.intrinsic_science_purchase_points());
    put_ascii(v, t.score_screen());
    put_ascii(v, t.load_screen());
    put_ascii(v, t.head_water_mark_image_name());
    put_ascii(v, t.flag_water_mark_image_name());
    put_ascii(v, t.enabled_image_name());
    put_ascii(v, t.side_icon_image_name());
    put_ascii(v, t.general_image_name());
    put_ascii(v, t.beacon_template());
    put_ascii(v, t.general_features());
    put_ascii(v, t.medallion_normal());
    put_ascii(v, t.medallion_hilite());
    put_ascii(v, t.medallion_selected());
}

/// `ref_pt_dump`.
fn rust_dump(c: &Ctx, bug: u32) -> Vec<u8> {
    let mut v = Vec::new();
    put_i32(&mut v, c.store.player_template_count());
    for i in 0..c.store.player_template_count() {
        rust_dump_template(&mut v, c.store.nth_player_template(i).unwrap(), &c.names, bug);
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

/// The index of `found` in the store, -1 for `None`.
fn index_of(store: &PlayerTemplateStore, found: Option<*const PlayerTemplate>) -> i32 {
    found.map_or(-1, |p| store.templates().iter().position(|t| std::ptr::eq(t, p)).expect("a template of the store") as i32)
}

/// `findPlayerTemplate(key)` on the Rust side, as an index. `bug` 1 leaves the
/// old-map names out: a plain search by key.
fn rust_find(c: &mut Ctx, key: NameKeyType, bug: u32) -> i32 {
    if bug == 1 {
        return c.store.templates().iter().position(|t| t.name_key() == key).map_or(-1, |i| i as i32);
    }
    let found = c.store.find_player_template(&mut c.names, key).unwrap().map(|p| p as *const PlayerTemplate);
    index_of(&c.store, found)
}

/// `getNthPlayerTemplate(i)` as an index. `bug` 1 counts from 1.
fn rust_nth(c: &Ctx, i: i32, bug: u32) -> i32 {
    let found = c.store.nth_player_template(i - i32::from(bug == 1)).map(|p| p as *const PlayerTemplate);
    index_of(&c.store, found)
}

/// `getTemplateNumByName(name)`. `bug` 1 compares with case.
fn rust_num_by_name(c: &Ctx, name: &[u8], bug: u32) -> i32 {
    if bug == 1 {
        return c.store.templates().iter().position(|t| t.name(&c.names) == name).map_or(-1, |i| i as i32);
    }
    c.store.template_num_by_name(&c.names, name)
}

/// `getAllSideStrings(&list)` onto `initial`. `bug` 1 does not skip a side
/// already seen.
fn rust_sides(c: &Ctx, initial: &[&str], bug: u32) -> Vec<u8> {
    let mut list: Vec<Vec<u8>> = initial.iter().map(|s| s.as_bytes().to_vec()).collect();
    if bug == 1 {
        list.extend(c.store.templates().iter().map(|t| t.side().to_vec()));
    } else {
        c.store.all_side_strings(&mut list);
    }
    let mut v = Vec::new();
    put_u32(&mut v, list.len() as u32);
    for s in &list {
        put_ascii(&mut v, s);
    }
    v
}

// ---------------------------------------------------------------------------
// The generator
// ---------------------------------------------------------------------------

fn pick<'a, T: ?Sized>(g: &mut Gen, xs: &[&'a T]) -> &'a T {
    xs[g.below(xs.len() as u32) as usize]
}

/// The names a block may carry that nothing treats specially. Names differing
/// only in case are different templates (`nameToKey` is `strcmp`).
const TPL_POOL: [&str; 9] = [
    "FactionAmerica", "FactionChina", "FactionGLA", "FactionCivilian", "FactionObserver", "Custom_A", "custom_a", "FACTIONAMERICA", "Skirmish_X",
];
/// The names of the old maps' sub-factions (`findPlayerTemplate`'s `a1`..`g4`).
const OLD_NAMES: [&str; 12] = [
    "FactionAmericaChooseAGeneral", "FactionAmericaTankCommand", "FactionAmericaSpecialForces", "FactionAmericaAirForce",
    "FactionChinaChooseAGeneral", "FactionChinaRedArmy", "FactionChinaSpecialWeapons", "FactionChinaSecretPolice",
    "FactionGLAChooseAGeneral", "FactionGLATerrorCell", "FactionGLABiowarCommand", "FactionGLAWarlordCommand",
];
const SCI_POOL: [&str; 8] = ["SCIENCE_A", "SCIENCE_B", "SCIENCE_C", "SCIENCE_D", "SCIENCE_E", "SCIENCE_F", "science_a", "SCIENCE_G"];
const GHOSTS: [&str; 3] = ["SCIENCE_Ghost", "Nope", "science_aa"];
const THINGS: [&str; 8] = ["AmericaTank", "ChinaTank", "Barracks", "AmericaBarracks", "americatank", "GLAWorker", "Upgrade_X", "Airfield"];
const SIDES: [&str; 6] = ["America", "China", "GLA", "Civilian", "america", ""];
const STRINGS: [&str; 12] = [
    "Alpha", "America_Dozer", "GUI:Tip", "Beta_2", "x=y", "\"quoted words\"", "\"one\"", "\"unterminated", "Foo Bar", "AmericaCommandCenter", "Menu.wnd", "a/b.tga",
];
const LABELS: [&str; 8] = ["NAME:America", "NAME:China", "Label_A", "X", "Rank:Colonel", "NOTEXT_Missing", "notext", "NoTextLabel"];
const LEVELS: [&str; 8] = ["REGULAR", "VETERAN", "ELITE", "HEROIC", "veteran", "Elite", "GOD", ""];

fn gen_sep(g: &mut Gen) -> &'static str {
    pick(g, &[" = ", "=", " ", " =", "= ", "\t= ", " = ", " = "])
}

/// What a file draws on: the templates and sciences defined so far, and
/// whether it may contain mistakes.
struct Known {
    templates: Vec<String>,
    sciences: Vec<String>,
    clean: bool,
}

impl Known {
    fn science(&self, g: &mut Gen) -> String {
        if !self.sciences.is_empty() && (self.clean || g.below(10) < 7) {
            self.sciences[g.below(self.sciences.len() as u32) as usize].clone()
        } else if self.clean {
            pick(g, &SCI_POOL).to_string()
        } else {
            pick(g, &GHOSTS).to_string()
        }
    }
}

fn gen_int(g: &mut Gen, clean: bool) -> String {
    match g.below(16) {
        0..=7 => format!("{}", g.below(20000)),
        8 => pick(g, &["0", "1", "007", "-3", "2147483647", "-2147483648", "10000", "4294967295"]).to_string(),
        _ if clean => format!("{}", g.below(5000)),
        9 => pick(g, &["1e3", "12abc", "+5", "-1"]).to_string(),
        10 => pick(g, &["abc", "", "x1", "-"]).to_string(),
        11 => pick(g, &["99999999999", "2147483648", "-2147483649"]).to_string(),
        _ => format!("{}", g.below(5000)),
    }
}

fn gen_bool(g: &mut Gen, clean: bool) -> String {
    if clean || g.below(11) < 6 {
        return pick(g, &["Yes", "No", "yes", "NO", "Yes", "No"]).to_string();
    }
    pick(g, &["1", "0", "True", "maybe", ""]).to_string()
}

fn gen_string(g: &mut Gen, clean: bool) -> String {
    if clean {
        return pick(g, &STRINGS[..5]).to_string();
    }
    if g.below(12) == 0 {
        return String::new();
    }
    pick(g, &STRINGS).to_string()
}

fn gen_color(g: &mut Gen, clean: bool) -> String {
    let c = |g: &mut Gen| g.below(256);
    if clean || g.below(4) != 0 {
        return format!("R:{} G:{} B:{}", c(g), c(g), c(g));
    }
    match g.below(8) {
        0 => format!("R:{} G:{} B:{}", 256 + g.below(50), c(g), c(g)),
        1 => format!("R:{} G:-{} B:{}", c(g), 1 + g.below(9), c(g)),
        2 => format!("G:{} R:{} B:{}", c(g), c(g), c(g)),
        3 => format!("R:{} G:{}", c(g), c(g)),
        4 => format!("R {} G {} B {}", c(g), c(g), c(g)),
        5 => pick(g, &["", "R:", "R:x G:1 B:2", "R:1.5 G:2 B:3"]).to_string(),
        6 => format!("r:{} g:{} b:{}", c(g), c(g), c(g)),
        _ => format!("R:{} G:{} B:{} A:{}", c(g), c(g), c(g), c(g)),
    }
}

fn gen_percent(g: &mut Gen, clean: bool) -> String {
    if clean || g.below(4) != 0 {
        return pick(g, &["10%", "-10%", "0%", "100%", "250%", "12.5%", "-50", "33", "0.1", "75%"]).to_string();
    }
    pick(g, &["abc", "", "5x", "1e1", ".5", "- 5", "%", "-0%"]).to_string()
}

fn gen_production(g: &mut Gen, clean: bool, veterancy: bool) -> String {
    let thing = pick(g, &THINGS);
    if !clean && g.below(12) == 0 {
        return thing.to_string();
    }
    if veterancy {
        let level = if clean { pick(g, &LEVELS[..4]) } else { pick(g, &LEVELS) };
        format!("{thing} {level}")
    } else {
        format!("{thing} {}", gen_percent(g, clean))
    }
}

fn gen_sci_list(g: &mut Gen, k: &Known) -> String {
    if !k.clean {
        match g.below(20) {
            0 => return pick(g, &["None", "none", "NONE", "nOnE"]).to_string(),
            1 => return format!("{} None", k.science(g)),
            2 => return format!("None {}", k.science(g)),
            3 => return pick(g, &GHOSTS).to_string(),
            4 => return format!("{} {}", k.science(g), pick(g, &GHOSTS)),
            5 => return String::new(),
            6 => return format!("{},{}", k.science(g), k.science(g)),
            _ => {}
        }
    }
    let n = 1 + g.below(3);
    let list: Vec<String> = (0..n).map(|_| k.science(g)).collect();
    list.join(pick(g, &[" ", " ", "\t", "  "]))
}

fn gen_label(g: &mut Gen, clean: bool) -> String {
    if clean {
        return pick(g, &LABELS[..5]).to_string();
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

fn gen_science_block(g: &mut Gen, lines: &mut Vec<String>, k: &mut Known) {
    let free: Vec<&str> = SCI_POOL.iter().copied().filter(|n| !k.sciences.iter().any(|s| s == n)).collect();
    if free.is_empty() {
        return;
    }
    let name = free[g.below(free.len() as u32) as usize];
    lines.push(format!("Science {name}"));
    for _ in 0..g.below(3) {
        let body = match g.below(3) {
            0 => format!("SciencePurchasePointCost = {}", g.below(9)),
            1 => format!("IsGrantable = {}", pick(g, &["Yes", "No"])),
            _ => format!("DisplayName = {}", pick(g, &LABELS[..5])),
        };
        lines.push(format!(" {body}"));
    }
    lines.push("End".to_string());
    k.sciences.push(name.to_string());
}

fn gen_template_name(g: &mut Gen, k: &Known) -> String {
    match g.below(20) {
        0..=7 if !k.templates.is_empty() => k.templates[g.below(k.templates.len() as u32) as usize].clone(),
        8..=13 => pick(g, &OLD_NAMES).to_string(),
        14..=15 => format!("Gen_{}", g.below(6)),
        _ => pick(g, &TPL_POOL).to_string(),
    }
}

fn gen_template_block(g: &mut Gen, lines: &mut Vec<String>, k: &mut Known) {
    let name = gen_template_name(g, k);
    lines.push(format!("PlayerTemplate {name}").trim_end().to_string());
    for _ in 0..g.below(14) {
        let sep = gen_sep(g);
        let c = k.clean;
        let body = match g.below(54) {
            0 if !c => pick(g, &["Foo = 1", "Side", "StartMoney =", "PreferredColor", "StartingUnit10 = X", "sidex = 1", "Dead = 1", "DisplayName"]).to_string(),
            0..=1 => format!("Side{sep}{}", pick(g, &SIDES)),
            2 => format!("BaseSide{sep}{}", pick(g, &SIDES)),
            3 => format!("PlayableSide{sep}{}", gen_bool(g, c)),
            4..=5 => format!("DisplayName{sep}{}", gen_label(g, c)),
            6..=8 => format!("StartMoney{sep}{}", gen_int(g, c)),
            9..=10 => format!("PreferredColor{sep}{}", gen_color(g, c)),
            11 => format!("StartingBuilding{sep}{}", gen_string(g, c)),
            12..=19 => format!("StartingUnit{}{sep}{}", g.below(10), gen_string(g, c)),
            20..=22 => format!("ProductionCostChange{sep}{}", gen_production(g, c, false)),
            23..=25 => format!("ProductionTimeChange{sep}{}", gen_production(g, c, false)),
            26..=28 => format!("ProductionVeterancyLevel{sep}{}", gen_production(g, c, true)),
            29..=31 => format!("IntrinsicSciences{sep}{}", gen_sci_list(g, k)),
            32 => format!("PurchaseScienceCommandSetRank1{sep}{}", gen_string(g, c)),
            33 => format!("PurchaseScienceCommandSetRank3{sep}{}", gen_string(g, c)),
            34 => format!("PurchaseScienceCommandSetRank8{sep}{}", gen_string(g, c)),
            35 => format!("SpecialPowerShortcutCommandSet{sep}{}", gen_string(g, c)),
            36 => format!("SpecialPowerShortcutWinName{sep}{}", gen_string(g, c)),
            37 => format!("SpecialPowerShortcutButtonCount{sep}{}", gen_int(g, c)),
            38 => format!("IsObserver{sep}{}", gen_bool(g, c)),
            39 => format!("OldFaction{sep}{}", gen_bool(g, c)),
            40 => format!("IntrinsicSciencePurchasePoints{sep}{}", gen_int(g, c)),
            41 => format!("ScoreScreenImage{sep}{}", gen_string(g, c)),
            42 => format!("LoadScreenImage{sep}{}", gen_string(g, c)),
            43 => format!("LoadScreenMusic{sep}{}", gen_string(g, c)),
            44 => format!("ScoreScreenMusic{sep}{}", gen_string(g, c)),
            45 => format!("{}{sep}{}", pick(g, &["HeadWaterMark", "FlagWaterMark"]), gen_string(g, c)),
            46 => format!("{}{sep}{}", pick(g, &["EnabledImage", "SideIconImage", "GeneralImage"]), gen_string(g, c)),
            47 => format!("BeaconName{sep}{}", gen_string(g, c)),
            48 => format!("ArmyTooltip{sep}{}", gen_string(g, c)),
            49 => format!("Features{sep}{}", gen_string(g, c)),
            50 => format!("{}{sep}{}", pick(g, &["MedallionRegular", "MedallionHilite", "MedallionSelect"]), gen_string(g, c)),
            _ => format!("{}{sep}{}", pick(g, &["Side", "StartMoney", "IsObserver"]), if c { "Yes" } else { "x" }),
        };
        lines.push(format!("{}{body}{}", gen_indent(g), gen_comment(g)));
    }
    gen_end(g, k.clean, lines);
    if !k.templates.contains(&name) {
        k.templates.push(name);
    }
}

/// One file; `templates` and `sciences` are the names known so far. A share of
/// the files (`clean`) holds only well-formed blocks.
fn gen_file(g: &mut Gen, templates: &[String], sciences: &[String]) -> Vec<u8> {
    let clean = g.below(10) < 4;
    let mut k = Known { templates: templates.to_vec(), sciences: sciences.to_vec(), clean };
    let mut lines = Vec::new();
    for _ in 0..1 + g.below(5) {
        match g.below(20) {
            0..=3 => gen_science_block(g, &mut lines, &mut k),
            4..=15 => gen_template_block(g, &mut lines, &mut k),
            16 => lines.push(pick(g, &["; a comment", "", "   ", ";", "\t"]).to_string()),
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

/// Session 0's first file: it starts with the block `FactionAmerica`, so the
/// first `findPlayerTemplate` of the process (which initialises the statics)
/// runs with that name already numbered 1, then the old maps' names in
/// `findPlayerTemplate`'s order, one of them a block of its own.
const FIRST_FILE: &[u8] = b"PlayerTemplate FactionAmerica\n Side = America\n BaseSide = America\n PlayableSide = Yes\n DisplayName = NAME:America\n\
 StartMoney = 10000\n PreferredColor = R:100 G:114 B:245\n StartingBuilding = AmericaCommandCenter\n StartingUnit0 = AmericaVehicleDozer\n\
 ProductionCostChange = AmericaTank -10%\n ProductionTimeChange = AmericaTank 25%\n ProductionVeterancyLevel = AmericaTank ELITE\n\
 OldFaction = Yes\n IsObserver = No\nEnd\n\
Science SCIENCE_A\n DisplayName = NAME:America\nEnd\n\
PlayerTemplate FactionChina\n Side = China\n IntrinsicSciences = SCIENCE_A\nEnd\n\
PlayerTemplate FactionAmericaTankCommand\n BaseSide = Tank\nEnd\n";

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
            Some(Thrown::Other) => (5, Vec::new()),
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
        cpp::ref_pt_load(cname.as_ptr(), load_type as i32, i32::from(with_xfer), &mut crc, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
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

/// The comparisons of one session.
struct Checks {
    outcome: Check,
    outcome_bug: Check,
    dump: Check,
    dump_bug_observer: Check,
    dump_bug_maps: Check,
    dump_bug_key: Check,
    nth: Check,
    nth_bug: Check,
    keys: Check,
    find: Check,
    find_bug: Check,
    num: Check,
    num_bug: Check,
    sides: Check,
    sides_bug: Check,
    first_keys: Check,
}

impl Checks {
    fn new() -> Self {
        Checks {
            outcome: Check::new("PlayerTemplate INI::load (throw, message, CRC)"),
            outcome_bug: Check::new("INI::load"),
            dump: Check::new("PlayerTemplateStore (every field of all)"),
            dump_bug_observer: Check::new("PlayerTemplate IsObserver"),
            dump_bug_maps: Check::new("PlayerTemplate cost/time maps"),
            dump_bug_key: Check::new("PlayerTemplate name key"),
            nth: Check::new("getNthPlayerTemplate"),
            nth_bug: Check::new("getNthPlayerTemplate"),
            keys: Check::new("NAMEKEY of the probe names"),
            find: Check::new("findPlayerTemplate (names and raw keys)"),
            find_bug: Check::new("findPlayerTemplate"),
            num: Check::new("getTemplateNumByName"),
            num_bug: Check::new("getTemplateNumByName"),
            sides: Check::new("getAllSideStrings"),
            sides_bug: Check::new("getAllSideStrings"),
            first_keys: Check::new("the statics of the first findPlayerTemplate"),
        }
    }
}

#[derive(Default)]
struct Tally {
    sessions: u64,
    primed_sessions: u64,
    loads: u64,
    loads_ok: u64,
    loads_failed: u64,
    overrides_loads: u64,
    resets: u64,
    updates: u64,
    inits: u64,
    templates: u64,
    templates_with_money: u64,
    templates_with_maps: u64,
    templates_with_sciences: u64,
    observers: u64,
    named_like_old_map: u64,
    finds: u64,
    finds_found: u64,
    finds_via_old_name: u64,
    probes: u64,
    stopped_undefined: u64,
}

/// The lookups after a step. They give unseen names their keys, in the same
/// order on both sides.
fn probe(ctx: &mut Ctx, ck: &mut Checks, t: &mut Tally, tag: &str, step_salt: u32) {
    let count = ctx.store.player_template_count();
    for i in -1..count + 2 {
        let r = rust_nth(ctx, i, 0);
        let cx = unsafe { cpp::ref_pt_nth(i) };
        ck.nth.eq(r, cx, || format!("{tag}: nth {i}"));
        ck.nth_bug.eq(rust_nth(ctx, i, 1), cx, || format!("{tag}: nth {i}"));
        t.probes += 1;
    }
    // names: the fifteen of the statics, the pool, the templates, some more
    let mut probes: Vec<String> = LEGACY_FACTION_KEY_NAMES.iter().map(|s| s.to_string()).collect();
    probes.extend(TPL_POOL.iter().map(|s| s.to_string()));
    for i in 0..count {
        probes.push(show(ctx.store.nth_player_template(i).unwrap().name(&ctx.names)));
    }
    probes.push("Never_Seen_Before".to_string());
    for (i, n) in probes.iter().enumerate() {
        if i >= 27 && !(i as u32 + step_salt).is_multiple_of(2) {
            continue;
        }
        let cn = CString::new(n.as_str()).unwrap();
        let rk = ctx.names.name_to_key(n.as_bytes());
        let ckey = unsafe { cpp::ref_pt_name_key(cn.as_ptr()) };
        ck.keys.eq(rk.0, ckey, || format!("{tag}: name {n:?}"));
        let r = rust_find(ctx, rk, 0);
        let cf = unsafe { cpp::ref_pt_find(rk.0) };
        ck.find.eq(r, cf, || format!("{tag}: find {n:?}"));
        ck.find_bug.eq(rust_find(ctx, rk, 1), cf, || format!("{tag}: find {n:?}"));
        t.finds += 1;
        t.probes += 1;
        if cf >= 0 {
            t.finds_found += 1;
            if OLD_NAMES.contains(&n.as_str()) {
                t.finds_via_old_name += 1;
            }
        }
    }
    for raw in (0..=40u32).chain([1000, 100_000, u32::MAX >> 1]) {
        let r = rust_find(ctx, NameKeyType(raw), 0);
        let cf = unsafe { cpp::ref_pt_find(raw) };
        ck.find.eq(r, cf, || format!("{tag}: find raw {raw}"));
        ck.find_bug.eq(rust_find(ctx, NameKeyType(raw), 1), cf, || format!("{tag}: find raw {raw}"));
        t.probes += 1;
    }
    // getTemplateNumByName: the names of the store as written, in other cases, and others
    let mut by_name: Vec<Vec<u8>> = vec![b"".to_vec(), b"Nobody".to_vec(), b"factionamerica".to_vec(), b"FACTIONCHINA".to_vec()];
    for i in 0..count {
        let n = ctx.store.nth_player_template(i).unwrap().name(&ctx.names).to_vec();
        by_name.push(n.to_ascii_lowercase());
        by_name.push(n.to_ascii_uppercase());
        by_name.push(n);
    }
    for n in &by_name {
        let cn = CString::new(n.clone()).unwrap();
        let cx = unsafe { cpp::ref_pt_num_by_name(cn.as_ptr()) };
        ck.num.eq(rust_num_by_name(ctx, n, 0), cx, || format!("{tag}: num {:?}", show(n)));
        ck.num_bug.eq(rust_num_by_name(ctx, n, 1), cx, || format!("{tag}: num {:?}", show(n)));
        t.probes += 1;
    }
    // getAllSideStrings onto lists that already hold some
    for initial in [&[][..], &["America"][..], &["Zed", "America", "China"][..]] {
        let mut blob = Vec::new();
        for s in initial {
            blob.extend_from_slice(s.as_bytes());
            blob.push(0);
        }
        blob.push(0);
        let cs = cpp_buf(|b, cap| unsafe { cpp::ref_pt_sides(blob.as_ptr() as *const _, initial.len() as i32, b, cap) });
        ck.sides.eq(rust_sides(ctx, initial, 0), cs.clone(), || format!("{tag}: sides onto {initial:?}"));
        ck.sides_bug.eq(rust_sides(ctx, initial, 1), cs, || format!("{tag}: sides onto {initial:?}"));
        t.probes += 1;
    }
}

#[test]
fn player_template_blocks_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let table = blocks();
    let mut g = Gen::new(0x97E5);
    let mut ck = Checks::new();
    let mut excluded = Excluded::default();
    let mut t = Tally::default();
    let mut ctx = Ctx::new();
    const SESSIONS: usize = 3_000;
    for session in 0..SESSIONS {
        ctx.sciences = ScienceStore::new();
        ctx.names.reset();
        ctx.store.init();
        unsafe { cpp::ref_pt_reset() };
        t.sessions += 1;
        // session 0: no priming, a fixed first file (see FIRST_FILE); later
        // sessions: the fifteen names first (so the statics' numbers mean what
        // they say) in 60 % of them
        let primed = session > 0 && g.below(10) < 6;
        if primed {
            t.primed_sessions += 1;
            for n in LEGACY_FACTION_KEY_NAMES {
                let cn = CString::new(n).unwrap();
                let rk = ctx.names.name_to_key(n.as_bytes());
                assert_eq!(rk.0, unsafe { cpp::ref_pt_name_key(cn.as_ptr()) });
            }
        }
        let steps = 2 + g.below(4);
        for step in 0..steps {
            // 0 overwrite load, 1 create-overrides load, 2 reset, 3 init, 4 update
            let kind = if step == 0 {
                0
            } else {
                match g.below(24) {
                    0..=12 => 0,
                    13..=17 => 1,
                    18..=19 => 2,
                    20..=21 => 4,
                    _ => 3,
                }
            };
            let tag = format!("session {session} step {step}");
            match kind {
                2..=4 => {
                    match kind {
                        2 => {
                            ctx.store.reset();
                            t.resets += 1;
                        }
                        3 => {
                            ctx.store.init();
                            t.inits += 1;
                        }
                        _ => {
                            ctx.store.update();
                            t.updates += 1;
                        }
                    }
                    unsafe { cpp::ref_pt_op(if kind == 2 { 0 } else if kind == 3 { 1 } else { 2 }) };
                }
                _ => {
                    let known: Vec<String> = ctx.store.templates().iter().map(|p| show(p.name(&ctx.names))).collect();
                    let known_sciences: Vec<String> = ctx.sciences.friend_get_science_names(&ctx.names).iter().map(|n| show(n)).collect();
                    let overrides = kind == 1;
                    let data = if session == 0 && step == 0 { FIRST_FILE.to_vec() } else { gen_file(&mut g, &known, &known_sciences) };
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
                    let r = Ini::load(&name, &data, load_type, if with_xfer { Some(&mut crc as &mut dyn Xfer) } else { None }, &table, &mut ctx);
                    let crc_v = if with_xfer { crc.get_crc() } else { 0 };
                    let rust = match rust_outcome(r, crc_v) {
                        Ok(o) => o,
                        Err(e) => {
                            assert!(session > 0, "session 0's fixed first file must not be undefined: {e:?}");
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
                    if session == 0 && step == 0 {
                        assert_eq!(rust.status, 0, "session 0's first file loads");
                        // the statics of findPlayerTemplate: initialised by that load, the
                        // same fifteen numbers on both sides (compared through the finds
                        // below as well)
                        let keys = ctx.store.legacy_faction_keys().expect("the first load ran findPlayerTemplate");
                        for (i, n) in LEGACY_FACTION_KEY_NAMES.iter().enumerate() {
                            let cn = CString::new(*n).unwrap();
                            ck.first_keys.eq(keys[i].0, unsafe { cpp::ref_pt_name_key(cn.as_ptr()) }, || format!("static {i} {n}"));
                        }
                    }
                }
            }

            // the store, after the step
            let cd = cpp_buf(|b, n| unsafe { cpp::ref_pt_dump(b, n) });
            let file = || tag.clone();
            ck.dump.eq(rust_dump(&ctx, 0), cd.clone(), file);
            ck.dump_bug_observer.eq(rust_dump(&ctx, 1), cd.clone(), file);
            ck.dump_bug_maps.eq(rust_dump(&ctx, 2), cd.clone(), file);
            ck.dump_bug_key.eq(rust_dump(&ctx, 3), cd, file);

            // tallies of what was compared
            for p in ctx.store.templates() {
                t.templates += 1;
                if p.money().count_money() != 0 {
                    t.templates_with_money += 1;
                }
                if !p.production_cost_changes().is_empty() || !p.production_time_changes().is_empty() || !p.production_veterancy_levels().is_empty() {
                    t.templates_with_maps += 1;
                }
                if !p.intrinsic_sciences().is_empty() {
                    t.templates_with_sciences += 1;
                }
                if p.is_observer() {
                    t.observers += 1;
                }
                if OLD_NAMES.iter().any(|o| o.as_bytes() == p.name(&ctx.names)) {
                    t.named_like_old_map += 1;
                }
            }
            probe(&mut ctx, &mut ck, &mut t, &tag, step + session as u32);
        }
    }
    unsafe { cpp::ref_pt_sides_null() };
    println!(
        "PlayerTemplate INI: {} sessions ({} with the fifteen names first), {} loads ({} overrides, {} ok, {} threw), {} resets, {} updates, {} inits; \
         compared {} templates ({} with money, {} with production maps, {} with sciences, {} observers, {} named like an old map), \
         {} lookups ({} finds, {} found, {} of those by an old map's name); sessions stopped at an undefined step: {} {:?}",
        t.sessions, t.primed_sessions, t.loads, t.overrides_loads, t.loads_ok, t.loads_failed, t.resets, t.updates, t.inits, t.templates,
        t.templates_with_money, t.templates_with_maps, t.templates_with_sciences, t.observers, t.named_like_old_map, t.probes, t.finds,
        t.finds_found, t.finds_via_old_name, excluded.total(), excluded.0
    );
    assert!(t.loads_ok > 1500 && t.loads_failed > 1500, "the generator must reach both outcomes: {} ok, {} threw", t.loads_ok, t.loads_failed);
    assert!(t.templates > 5000 && t.templates_with_money > 1500 && t.templates_with_maps > 1000 && t.templates_with_sciences > 500, "too few templates compared");
    assert!(t.observers > 100 && t.named_like_old_map > 100, "too few observers or renamed templates compared");
    assert!(t.finds_via_old_name > 200, "too few finds by an old map's name that found a template");
    ck.outcome.finish();
    ck.outcome_bug.finish_planted();
    ck.dump.finish();
    ck.dump_bug_observer.finish_planted();
    ck.dump_bug_maps.finish_planted();
    ck.dump_bug_key.finish_planted();
    ck.nth.finish();
    ck.nth_bug.finish_planted();
    ck.keys.finish();
    ck.find.finish();
    ck.find_bug.finish_planted();
    ck.num.finish();
    ck.num_bug.finish_planted();
    ck.sides.finish();
    ck.sides_bug.finish_planted();
    ck.first_keys.finish();
}
