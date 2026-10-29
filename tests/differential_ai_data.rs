// SPDX-License-Identifier: GPL-3.0-only
//! Differential test of the AI's data (`logic::ai::ai_data` and `ai_system::TAiData`, Phase 1 layer 10, `p1w13-sonnet`, batch A27)
//! against AI.cpp's `TheAIFieldParseTable` and its parsers (`parseSideInfo`, `parseSkillSet`, `parseScience`,
//! `parseSkirmishBuildList`), `newOverride`, `TAiData`'s constructor, `crc` and `xfer`, and SidesList.cpp's
//! `BuildListInfo::parseStructure`, cut verbatim and compiled with the real AI.h and SidesList.h (`differential/wrap_ai_data.cpp`),
//! under the real `INI::load` and `ScienceStore` (wrap_science.cpp's archive).
//!
//! A **session** is a fresh science store and AI data, a `Science` file (the sciences the skill sets name, some free, so not
//! purchasable), then 1 to 5 `AIData` files, each loaded with `INI_LOAD_OVERWRITE`, `INI_LOAD_MULTIFILE` or
//! `INI_LOAD_CREATE_OVERRIDES` (which makes a new override first). The files are synthetic: every field of the table with
//! well-formed and malformed values, `SideInfo` blocks (new and repeated sides, every field, skill sets with known, unknown, free
//! and too many sciences), `SkirmishBuildList` blocks (new and repeated factions, `Structure` entries with every field, none),
//! unknown fields, missing `End`s. After every file, compared: what `INI::load` throws, the load's `XferCRC`, the **whole chain**
//! (every `TAiData`'s 43 scalars, its side info and its build lists with every `BuildListInfo` field), and the current data's
//! `crc` and `xfer` CRCs.
//!
//! Not run: a `INI_LOAD_CREATE_OVERRIDES` load when a faction's build list is empty (upstream duplicates a null list): the
//! session stops there before the C++ runs (counted).
//!
//! # Results (2026-10-29, batch A27; upstream commit 0a05454d, seeds `0xA27_0000 + session`, 20,000 sessions)
//!
//! - 73,606 loads (12,882 of which threw, 17,279 with overrides), **0 mismatches** in the load outcome and the INI CRC, the whole
//!   chain after each load, and the current data's `crc` and `xfer` CRCs. At the sessions' ends: 37,691 sides, 172,065 counted
//!   skills, 58,464 build list entries. 2,900 sessions stopped at the undefined override of an empty build list.
//! - No port defect found.
//! - Planted bugs at 5,000 sessions: 3570 (the override's side info not reversed), 3571 (the 20-skill limit: a panic), 3572 (a free
//!   science counted), 3573 (a structure prepended) each caught.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Thrown, TYPE_TABLE};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::science::{GameTextFetch, ScienceStore};
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::ai::ai_data::{self, AiDataChain, AiDataEntry, MAX_AI_UPGRADES};
use rts_lockstep_engine::logic::sides_list::MAX_RESOURCE_GATHERERS;
use std::ffi::CString;
use support::{Check, Gen};

const SESSIONS: u64 = 20_000;

struct StandInText;
impl GameTextFetch for StandInText {
    fn fetch(&self, label: &[u8]) -> UnicodeString {
        let mut units: Vec<u16> = vec![u16::from(b'T'), u16::from(b':')];
        units.extend(label.iter().map(|&b| u16::from(b)));
        UnicodeString::try_from_chars(&units).unwrap()
    }
}

struct Ctx {
    sciences: ScienceStore,
    names: NameKeyGenerator,
    ai: AiDataChain,
    undefined: bool,
}

fn h_science(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.sciences.parse_science_definition(ini, &mut c.names, &StandInText)
}
fn h_ai_data(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    match ai_data::parse_ai_data_definition(ini, &mut c.ai, &c.sciences, &mut c.names) {
        Ok(r) => r,
        Err(_) => {
            c.undefined = true;
            Err(IniError::new(IniErrorKind::Bug))
        }
    }
}
fn h_stub(_: &mut Ini<'_>, _: &mut Ctx) -> IniResult<()> {
    Err(IniError::new(IniErrorKind::Bug))
}
fn blocks() -> Vec<BlockParse<Ctx>> {
    TYPE_TABLE
        .iter()
        .map(|&(token, _, _)| BlockParse {
            token,
            parse: match token {
                "Science" => h_science,
                "AIData" => h_ai_data,
                _ => h_stub,
            },
        })
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    status: i32,
    detail: Vec<u8>,
    crc: u32,
}

fn rust_outcome(r: IniResult<()>, crc: u32) -> Option<Outcome> {
    let (status, detail) = match r {
        Ok(()) => (0, Vec::new()),
        Err(e) => match e.thrown()? {
            Thrown::Exception(m) => (1, m),
            Thrown::Status(c) => (2, c.to_le_bytes().to_vec()),
            Thrown::ErrorCode(c) => (3, c.to_le_bytes().to_vec()),
            Thrown::OutOfRange => (4, Vec::new()),
            Thrown::Other => (5, Vec::new()),
        },
    };
    Some(Outcome { status, detail, crc })
}

fn cpp_load(name: &[u8], data: &[u8], load_type: IniLoadType) -> Outcome {
    let cname = CString::new(name.to_vec()).unwrap();
    let mut msg = vec![0u8; 4096];
    let (mut crc, mut code) = (0u32, 0u32);
    let status = unsafe {
        cpp::ref_ini_reset();
        cpp::ref_ini_add_file(cname.as_ptr(), data.as_ptr() as *const _, data.len() as i32);
        cpp::ref_science_load(cname.as_ptr(), load_type as i32, 1, &mut crc, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code)
    };
    let detail = match status {
        0 | 4 => Vec::new(),
        1 => msg[..code as usize].to_vec(),
        2 | 3 => code.to_le_bytes().to_vec(),
        _ => panic!("C++ threw something unexpected (status {status})"),
    };
    Outcome { status, detail, crc }
}

// ---------------------------------------------------------------------------
// The dump (wrap_ai_data.cpp's layout)
// ---------------------------------------------------------------------------

fn u(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn f(v: &mut Vec<u8>, x: f32) {
    u(v, x.to_bits());
}
fn s(v: &mut Vec<u8>, x: &[u8]) {
    u(v, x.len() as u32);
    v.extend_from_slice(x);
}

fn dump_entry(v: &mut Vec<u8>, e: &AiDataEntry) {
    let d = &e.data;
    f(v, d.structure_seconds);
    f(v, d.team_seconds);
    u(v, d.resources_wealthy as u32);
    u(v, d.resources_poor as u32);
    u(v, d.force_idle_frames_count);
    f(v, d.structures_wealthy_mod);
    f(v, d.team_wealthy_mod);
    f(v, d.structures_poor_mod);
    f(v, d.team_poor_mod);
    f(v, d.team_resources_to_build);
    f(v, d.guard_inner_modifier_ai);
    f(v, d.guard_outer_modifier_ai);
    f(v, d.guard_inner_modifier_human);
    f(v, d.guard_outer_modifier_human);
    u(v, d.guard_chase_unit_frames);
    u(v, d.guard_enemy_scan_rate);
    u(v, d.guard_enemy_return_scan_rate);
    f(v, d.skirmish_group_fudge_value);
    f(v, d.repulsed_distance);
    u(v, u32::from(d.enable_repulsors));
    f(v, d.alert_range_modifier);
    f(v, d.aggressive_range_modifier);
    u(v, u32::from(d.force_skirmish_ai));
    u(v, u32::from(d.rotate_skirmish_bases));
    u(v, u32::from(d.attack_uses_line_of_sight));
    u(v, u32::from(d.attack_ignore_insignificant_buildings));
    f(v, d.attack_priority_distance_modifier);
    f(v, d.max_recruit_distance);
    f(v, d.skirmish_base_defense_extra_distance);
    f(v, d.wall_height);
    u(v, d.min_infantry_for_group as u32);
    u(v, d.min_vehicles_for_group as u32);
    f(v, d.min_distance_for_group);
    f(v, d.distance_requires_group);
    f(v, d.min_clump_density);
    u(v, d.infantry_pathfind_diameter as u32);
    u(v, d.vehicle_pathfind_diameter as u32);
    u(v, d.rebuild_delay_seconds as u32);
    f(v, d.supply_center_safe_radius);
    f(v, d.ai_dozer_bored_radius_modifier);
    u(v, u32::from(d.ai_crushes_infantry));
    f(v, d.max_retaliate_distance);
    f(v, d.retaliate_friends_radius);
    u(v, e.lists.side_info.len() as u32);
    for i in &e.lists.side_info {
        s(v, &i.side);
        u(v, i.easy as u32);
        u(v, i.normal as u32);
        u(v, i.hard as u32);
        for set in &i.skill_sets {
            u(v, set.num_skills as u32);
            for k in 0..MAX_AI_UPGRADES {
                u(v, set.skills[k].0 as u32);
            }
        }
        s(v, &i.base_defense_structure1);
    }
    u(v, e.lists.side_build_lists.len() as u32);
    for b in &e.lists.side_build_lists {
        s(v, &b.side);
        u(v, b.build_list.len() as u32);
        for x in &b.build_list {
            s(v, &x.building_name);
            s(v, &x.template_name);
            f(v, x.location.x);
            f(v, x.location.y);
            f(v, x.location.z);
            f(v, x.rally_point_offset.x);
            f(v, x.rally_point_offset.y);
            f(v, x.angle);
            u(v, u32::from(x.is_initially_built));
            u(v, x.num_rebuilds);
            s(v, &x.script);
            u(v, x.health as u32);
            u(v, u32::from(x.whiner));
            u(v, u32::from(x.unsellable));
            u(v, u32::from(x.repairable));
            u(v, u32::from(x.automatically_build));
            u(v, u32::from(x.selected));
            u(v, x.object_id as u32);
            u(v, x.object_timestamp);
            u(v, u32::from(x.under_construction));
            for k in 0..MAX_RESOURCE_GATHERERS {
                u(v, x.resource_gatherers[k] as u32);
            }
            u(v, u32::from(x.is_supply_building));
            u(v, x.desired_gatherers as u32);
            u(v, x.current_gatherers as u32);
            u(v, u32::from(x.priority_build));
        }
    }
}

fn rust_dump(c: &AiDataChain) -> Vec<u8> {
    let mut v = Vec::new();
    u(&mut v, c.entries.len() as u32);
    for e in &c.entries {
        dump_entry(&mut v, e);
    }
    v
}

fn cpp_dump() -> Vec<u8> {
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = unsafe { cpp::ref_ai_data_dump(buf.as_mut_ptr(), buf.len() as i32) } as usize;
        if n <= buf.len() {
            buf.truncate(n);
            return buf;
        }
        buf = vec![0u8; n];
    }
}

fn rust_crcs(c: &mut AiDataChain) -> (u32, u32) {
    let d = &mut c.entries[0].data;
    let mut a = XferCrc::new();
    a.open(b"ai").unwrap();
    d.crc(&mut a).unwrap();
    let mut b = XferCrc::new();
    b.open(b"ai").unwrap();
    d.xfer(&mut b).unwrap();
    (a.get_crc(), b.get_crc())
}

// ---------------------------------------------------------------------------
// The files
// ---------------------------------------------------------------------------

const SCIENCES: [&str; 8] = ["SCIENCE_A", "SCIENCE_B", "SCIENCE_C", "SCIENCE_D", "SCIENCE_E", "SCIENCE_F", "SCIENCE_G", "SCIENCE_H"];
const SIDES: [&str; 4] = ["America", "China", "GLA", "america"];

fn pick<'a>(g: &mut Gen, v: &[&'a str]) -> &'a str {
    v[g.below(v.len() as u32) as usize]
}

/// The sciences: each a cost of 0 (not purchasable) or 1..4.
fn science_file(g: &mut Gen) -> Vec<u8> {
    let mut t = String::new();
    for name in SCIENCES {
        let cost = if g.below(4) == 0 { 0 } else { 1 + g.below(4) };
        t.push_str(&format!("Science {name}\n  PrerequisiteSciences = None\n  SciencePurchasePointCost = {cost}\n  IsGrantable = Yes\nEnd\n"));
    }
    t.into_bytes()
}

fn real(g: &mut Gen, clean: bool) -> String {
    if !clean && g.below(12) == 0 {
        return pick(g, &["", "abc", "1.5x", "-", "1e400", "nan"]).to_string();
    }
    match g.below(4) {
        0 => format!("{}", g.below(500)),
        1 => format!("{:.3}", g.uniform(-50.0, 500.0)),
        2 => format!("{:.2}", g.uniform(0.0, 3.0)),
        _ => pick(g, &["0", "0.5", "1.0", "250", "-3"]).to_string(),
    }
}
fn int(g: &mut Gen, clean: bool) -> String {
    if !clean && g.below(12) == 0 {
        return pick(g, &["", "x", "1.5", "99999999999", "-"]).to_string();
    }
    format!("{}", g.below(40) as i32 - 5)
}
fn boolean(g: &mut Gen, clean: bool) -> String {
    if !clean && g.below(12) == 0 {
        return pick(g, &["", "maybe", "1"]).to_string();
    }
    pick(g, &["Yes", "No", "yes", "NO"]).to_string()
}

const REALS: [&str; 27] = [
    "StructureSeconds", "TeamSeconds", "StructuresWealthyRate", "TeamsWealthyRate", "StructuresPoorRate", "TeamsPoorRate",
    "TeamResourcesToStart", "GuardInnerModifierAI", "GuardOuterModifierAI", "GuardInnerModifierHuman", "GuardOuterModifierHuman",
    "SkirmishGroupFudgeDistance", "RepulsedDistance", "AlertRangeModifier", "AggressiveRangeModifier", "AttackPriorityDistanceModifier",
    "MaxRecruitRadius", "SkirmishBaseDefenseExtraDistance", "WallHeight", "MinDistanceForGroup", "DistanceRequiresGroup",
    "MinClumpDensity", "SupplyCenterSafeRadius", "AIDozerBoredRadiusModifier", "MaxRetaliationDistance", "RetaliationFriendsRadius",
    "StructureSeconds",
];
const INTS: [&str; 7] = ["Wealthy", "Poor", "MinInfantryForGroup", "MinVehiclesForGroup", "InfantryPathfindDiameter", "VehiclePathfindDiameter", "RebuildDelayTimeSeconds"];
const BOOLS: [&str; 7] = ["EnableRepulsors", "ForceSkirmishAI", "RotateSkirmishBases", "AttackUsesLineOfSight", "AttackIgnoreInsignificantBuildings", "AICrushesInfantry", "EnableRepulsors"];
const DURATIONS: [&str; 4] = ["ForceIdleMSEC", "GuardChaseUnitsDuration", "GuardEnemyScanRate", "GuardEnemyReturnScanRate"];

fn side_info(g: &mut Gen, t: &mut Vec<String>, clean: bool) {
    t.push(format!("  SideInfo = {}", pick(g, &SIDES)));
    for _ in 0..g.below(6) {
        match g.below(10) {
            0 => t.push(format!("    ResourceGatherersEasy = {}", int(g, clean))),
            1 => t.push(format!("    ResourceGatherersNormal = {}", int(g, clean))),
            2 => t.push(format!("    ResourceGatherersHard = {}", int(g, clean))),
            3 => t.push(format!("    BaseDefenseStructure1 = {}", pick(g, &["AmericaPatriotBattery", "ChinaGattlingCannon", "GLAStingerSite"]))),
            _ => {
                t.push(format!("    SkillSet{}", 1 + g.below(5)));
                // up to 23 sciences: past the 20 slots sometimes
                let n = if g.below(10) == 0 { 18 + g.below(6) } else { g.below(5) };
                for _ in 0..n {
                    let sci = if !clean && g.below(15) == 0 { pick(g, &["SCIENCE_UNKNOWN", "None", ""]) } else { pick(g, &SCIENCES) };
                    t.push(format!("      Science = {sci}"));
                }
                t.push("    End".to_string());
            }
        }
    }
    if clean || g.below(20) != 0 {
        t.push("  End".to_string());
    }
}

fn build_list(g: &mut Gen, t: &mut Vec<String>, clean: bool) {
    t.push(format!("  SkirmishBuildList = {}", pick(g, &["FactionAmerica", "FactionChina", "FactionGLA"])));
    // (an empty list one time in 15: an override load after one is undefined, and stops the session)
    let n = if g.below(15) == 0 { 0 } else { 1 + g.below(4) };
    for _ in 0..n {
        t.push(format!("    Structure {}", pick(g, &["AmericaCommandCenter", "ChinaBarracks", "GLAPalace", "GenericWall"])));
        for _ in 0..g.below(7) {
            match g.below(7) {
                0 => t.push(format!("      Name = {}", pick(g, &["Command", "Barracks_2", "x"]))),
                1 => t.push(format!("      Location = X:{} Y:{}", real(g, clean), real(g, clean))),
                2 => t.push(format!("      Rebuilds = {}", int(g, clean))),
                3 => t.push(format!("      Angle = {}", real(g, clean))),
                4 => t.push(format!("      InitiallyBuilt = {}", boolean(g, clean))),
                5 => t.push(format!("      RallyPointOffset = X:{} Y:{}", real(g, clean), real(g, clean))),
                _ => t.push(format!("      AutomaticallyBuild = {}", boolean(g, clean))),
            }
        }
        t.push("    End".to_string());
    }
    t.push("  End".to_string());
}

fn ai_file(g: &mut Gen) -> Vec<u8> {
    let clean = g.below(10) < 6;
    let mut t = vec!["AIData".to_string()];
    for _ in 0..g.below(14) {
        match g.below(20) {
            0..=5 => {
                let k = pick(g, &REALS);
                let v = real(g, clean);
                t.push(format!("  {k} = {v}"));
            }
            6..=7 => {
                let k = pick(g, &INTS);
                let v = int(g, clean);
                t.push(format!("  {k} = {v}"));
            }
            8..=9 => {
                let k = pick(g, &BOOLS);
                let v = boolean(g, clean);
                t.push(format!("  {k} = {v}"));
            }
            10 => {
                let k = pick(g, &DURATIONS);
                let v = int(g, clean);
                t.push(format!("  {k} = {v}"));
            }
            11..=14 => side_info(g, &mut t, clean),
            15..=17 => build_list(g, &mut t, clean),
            18 if !clean => t.push(pick(g, &["  Unknown = 3", "  =", "  SideInfo"]).to_string()),
            _ => t.push("  ; a comment".to_string()),
        }
    }
    if clean || g.below(15) != 0 {
        t.push("End".to_string());
    }
    let mut out = t.join("\n").into_bytes();
    out.push(b'\n');
    out
}

// ---------------------------------------------------------------------------
// The sessions
// ---------------------------------------------------------------------------

#[test]
fn ai_data_matches_upstream() {
    let sessions: u64 = std::env::var("AI_DATA_SESSIONS").ok().and_then(|s| s.parse().ok()).unwrap_or(SESSIONS);
    let table = blocks();
    let mut outcome = Check::new("AI data load outcome (what INI::load throws, the INI CRC)");
    let mut chain = Check::new("AI data chain (every TAiData's scalars, side info, build lists)");
    let mut crcs = Check::new("AI data crc and xfer CRCs");
    let (mut loads, mut threw, mut overrides, mut stopped, mut sides, mut skills, mut builds) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for session in 0..sessions {
        let mut g = Gen::new(0xA27_0000 + session);
        unsafe {
            cpp::ref_science_reset();
            cpp::ref_ai_data_reset();
        }
        let mut names = NameKeyGenerator::new();
        names.reset();
        let mut ctx = Ctx { sciences: ScienceStore::new(), names, ai: AiDataChain::default(), undefined: false };
        let steps = 2 + g.below(5);
        for step in 0..steps {
            let (data, load_type) = if step == 0 {
                (science_file(&mut g), IniLoadType::Overwrite)
            } else {
                let lt = match g.below(6) {
                    0 | 1 => IniLoadType::CreateOverrides,
                    2 => IniLoadType::Multifile,
                    _ => IniLoadType::Overwrite,
                };
                (ai_file(&mut g), lt)
            };
            let name = format!("Data\\INI\\AIData{}.ini", step).into_bytes();
            let mut crc = XferCrc::new_without_nan_guard();
            let r = Ini::load(&name, &data, load_type, Some(&mut crc as &mut dyn Xfer), &table, &mut ctx);
            if ctx.undefined {
                stopped += 1;
                break;
            }
            let Some(rust) = rust_outcome(r, crc.get_crc()) else {
                stopped += 1;
                break;
            };
            loads += 1;
            if rust.status != 0 {
                threw += 1;
            }
            if load_type == IniLoadType::CreateOverrides && step > 0 {
                overrides += 1;
            }
            let c = cpp_load(&name, &data, load_type);
            let tag = || format!("session {session} step {step} {load_type:?}: {}", String::from_utf8_lossy(&data));
            outcome.eq(&rust, &c, tag);
            chain.eq(rust_dump(&ctx.ai), cpp_dump(), tag);
            let (mut a, mut b) = (0u32, 0u32);
            unsafe { cpp::ref_ai_data_crc(&mut a, &mut b) };
            crcs.eq(rust_crcs(&mut ctx.ai), (a, b), tag);
        }
        let cur = ctx.ai.current();
        sides += cur.lists.side_info.len() as u64;
        skills += cur.lists.side_info.iter().flat_map(|i| i.skill_sets.iter()).map(|s| s.num_skills as u64).sum::<u64>();
        builds += cur.lists.side_build_lists.iter().map(|b| b.build_list.len() as u64).sum::<u64>();
    }
    println!(
        "COVERAGE: {sessions} sessions, {loads} loads ({threw} threw, {overrides} with overrides), {stopped} stopped (undefined); at the end: {sides} sides, {skills} skills, {builds} build list entries"
    );
    outcome.finish();
    chain.finish();
    crcs.finish();
}
