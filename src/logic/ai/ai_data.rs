// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AI.cpp (lines 59-308: `TAiData::addSideInfo`, `addFactionBuildList`,
//     `AISideBuildList`, `TheAIFieldParseTable`, `AI::parseSideInfo`, `parseSkillSet`, `parseScience`, `parseSkirmishBuildList`;
//     lines 386-445: `AI::newOverride`, `addSideInfo`, `parseAiDataDefinition`; lines 953-998: `TAiData::crc`, `xfer`)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Map/SidesList.cpp (`BuildListInfo::parseStructure`, `duplicate`)
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INIAiData.cpp (`INI::parseAIDataDefinition`)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AI.h (`TSkillSet`, `AISideInfo`, `AISideBuildList`, `MAX_AI_UPGRADES`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The AI's data (`AIData.ini`; `p1w13-sonnet`, layer 10, batch A27): [`TAiData`]'s INI table, its per-side information (the
//! resource gatherers, the skill sets of sciences, the base defence), the skirmish build lists, the override chain the
//! `INI_LOAD_CREATE_OVERRIDES` loads make, and its CRC and save.
//!
//! The chain is [`AiDataChain`]: `m_aiData` and its `m_next` links, the current data first. The side info and build lists are
//! upstream's singly linked lists as vectors, head first (`addSideInfo` and a new faction's `addFactionBuildList` prepend).
//!
//! # Upstream facts kept
//!
//! - A side's skill sets are as the allocator left them until a `SkillSetN` line sets one (the pool zero-fills: no skills, every
//!   slot 0); a science that is not purchasable is written into the next slot and not counted, so the next one overwrites it; a
//!   skill past `MAX_AI_UPGRADES` is dropped without reading its token.
//! - `newOverride` copies the side info and the build lists head first, prepending each: the copies are in reverse order. A
//!   faction whose build list is empty (a `SkirmishBuildList` with no `Structure`, or one that replaced a list with none) is
//!   undefined there (`duplicate` of a null list): [`AiDataChain::new_override`] refuses it.
//! - `TAiData::crc` covers 24 of its scalars (from the structure seconds to the repulsors, but the wall height and the skirmish
//!   fudge value); its `xfer` is the version alone.

use crate::common::base_type::Coord3D;
use crate::common::ini::{FieldParse, Ini, IniLoadType, IniResult, UserData};
use crate::common::name_key_generator::NameKeyGenerator;
use crate::common::science::{ScienceStore, ScienceType, SCIENCE_INVALID};
use crate::common::xfer::{Xfer, XferResult, XferVersion};
use crate::logic::ai::ai_system::TAiData;
use crate::logic::modules::plant;
use crate::logic::sides_list::BuildListInfo;

/// `MAX_AI_UPGRADES`.
pub const MAX_AI_UPGRADES: usize = 20;

/// `TSkillSet`: the sciences an AI buys, in order.
#[derive(Clone, Debug, PartialEq)]
pub struct TSkillSet {
    pub num_skills: i32,
    pub skills: [ScienceType; MAX_AI_UPGRADES],
}

impl Default for TSkillSet {
    /// As the memory pool leaves it: zeroed.
    fn default() -> Self {
        TSkillSet { num_skills: 0, skills: [ScienceType(0); MAX_AI_UPGRADES] }
    }
}

/// `AISideInfo`: a side's gatherer counts per difficulty, skill sets and base defence.
#[derive(Clone, Debug, PartialEq)]
pub struct AiSideInfo {
    pub side: Vec<u8>,
    pub easy: i32,
    pub normal: i32,
    pub hard: i32,
    pub skill_sets: [TSkillSet; 5],
    pub base_defense_structure1: Vec<u8>,
}

impl Default for AiSideInfo {
    /// `AISideInfo()`: 0, 1 and 2 gatherers.
    fn default() -> Self {
        AiSideInfo { side: Vec::new(), easy: 0, normal: 1, hard: 2, skill_sets: Default::default(), base_defense_structure1: Vec::new() }
    }
}

/// `AISideBuildList`: a faction's skirmish build list.
#[derive(Clone, Debug)]
pub struct AiSideBuildList {
    pub side: Vec<u8>,
    /// `m_buildList` and its `m_nextBuildList` links, in order (empty: `NULL`).
    pub build_list: Vec<BuildListInfo>,
}

/// The side info and build lists of a [`TAiData`] (`m_sideInfo`, `m_sideBuildLists`), head first.
#[derive(Clone, Debug, Default)]
pub struct AiDataLists {
    pub side_info: Vec<AiSideInfo>,
    pub side_build_lists: Vec<AiSideBuildList>,
}

/// One `TAiData` of the chain: its scalars and its lists.
#[derive(Clone, Debug, Default)]
pub struct AiDataEntry {
    pub data: TAiData,
    pub lists: AiDataLists,
}

/// `TheAI->m_aiData` and its `m_next` chain: the current data first, the original last.
#[derive(Clone, Debug)]
pub struct AiDataChain {
    pub entries: Vec<AiDataEntry>,
}

impl Default for AiDataChain {
    /// `AI::AI()`: one `TAiData` with its constructor's values.
    fn default() -> Self {
        AiDataChain { entries: vec![AiDataEntry::default()] }
    }
}

impl AiDataLists {
    /// `TAiData::addSideInfo(info)`: at the head.
    pub fn add_side_info(&mut self, info: AiSideInfo) {
        self.side_info.insert(0, info);
    }

    /// `TAiData::addFactionBuildList(buildList)`: a faction already listed has its list replaced; a new one goes at the head.
    pub fn add_faction_build_list(&mut self, build: AiSideBuildList) {
        if let Some(info) = self.side_build_lists.iter_mut().find(|i| i.side == build.side) {
            info.build_list = build.build_list;
            return;
        }
        self.side_build_lists.insert(0, build);
    }
}

/// Why the data cannot be overridden (undefined upstream).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AiDataUndefined {
    /// `newOverride` duplicates a faction's empty build list (`NULL->duplicate()`).
    EmptyBuildListDuplicated,
}

impl AiDataChain {
    /// The current data (`m_aiData`).
    pub fn current(&self) -> &AiDataEntry {
        &self.entries[0]
    }

    /// `AI::newOverride()`: a copy of the current data becomes current, its side info and build lists copied in reverse.
    pub fn new_override(&mut self) -> Result<(), AiDataUndefined> {
        let cur = self.entries[0].clone();
        if cur.lists.side_build_lists.iter().any(|b| b.build_list.is_empty()) {
            return Err(AiDataUndefined::EmptyBuildListDuplicated);
        }
        let mut next = AiDataEntry { data: cur.data.clone(), lists: AiDataLists::default() };
        for info in &cur.lists.side_info {
            next.lists.add_side_info(info.clone());
        }
        for build in &cur.lists.side_build_lists {
            // (`duplicate()`: the whole list, in order)
            next.lists.add_faction_build_list(AiSideBuildList { side: build.side.clone(), build_list: build.build_list.clone() });
        }
        if plant(3570) {
            next.lists.side_info.reverse();
        }
        self.entries.insert(0, next);
        Ok(())
    }
}

/// What the parse functions write into: a target and the science store with the name keys (`TheScienceStore`,
/// `TheNameKeyGenerator`).
pub struct AiIniTarget<'a, T> {
    pub target: &'a mut T,
    pub sciences: &'a ScienceStore,
    pub names: &'a mut NameKeyGenerator,
}

type Fp<'a, T> = FieldParse<AiIniTarget<'a, T>>;

macro_rules! field {
    ($tok:expr, $f:ident, $parse:ident) => {
        FieldParse::new(
            $tok,
            |ini, t, _| {
                t.target.$f = ini.$parse()?;
                Ok(())
            },
            UserData::None,
        )
    };
}

/// `TheAIFieldParseTable`.
fn ai_table<'a>() -> Vec<Fp<'a, AiDataEntry>> {
    vec![
        FieldParse::new("StructureSeconds", |ini, t, _| set(&mut t.target.data.structure_seconds, ini.parse_real()?), UserData::None),
        FieldParse::new("TeamSeconds", |ini, t, _| set(&mut t.target.data.team_seconds, ini.parse_real()?), UserData::None),
        FieldParse::new("Wealthy", |ini, t, _| set(&mut t.target.data.resources_wealthy, ini.parse_int()?), UserData::None),
        FieldParse::new("Poor", |ini, t, _| set(&mut t.target.data.resources_poor, ini.parse_int()?), UserData::None),
        FieldParse::new("ForceIdleMSEC", |ini, t, _| set(&mut t.target.data.force_idle_frames_count, ini.parse_duration_unsigned_int()?), UserData::None),
        FieldParse::new("StructuresWealthyRate", |ini, t, _| set(&mut t.target.data.structures_wealthy_mod, ini.parse_real()?), UserData::None),
        FieldParse::new("TeamsWealthyRate", |ini, t, _| set(&mut t.target.data.team_wealthy_mod, ini.parse_real()?), UserData::None),
        FieldParse::new("StructuresPoorRate", |ini, t, _| set(&mut t.target.data.structures_poor_mod, ini.parse_real()?), UserData::None),
        FieldParse::new("TeamsPoorRate", |ini, t, _| set(&mut t.target.data.team_poor_mod, ini.parse_real()?), UserData::None),
        FieldParse::new("TeamResourcesToStart", |ini, t, _| set(&mut t.target.data.team_resources_to_build, ini.parse_real()?), UserData::None),
        FieldParse::new("GuardInnerModifierAI", |ini, t, _| set(&mut t.target.data.guard_inner_modifier_ai, ini.parse_real()?), UserData::None),
        FieldParse::new("GuardOuterModifierAI", |ini, t, _| set(&mut t.target.data.guard_outer_modifier_ai, ini.parse_real()?), UserData::None),
        FieldParse::new("GuardInnerModifierHuman", |ini, t, _| set(&mut t.target.data.guard_inner_modifier_human, ini.parse_real()?), UserData::None),
        FieldParse::new("GuardOuterModifierHuman", |ini, t, _| set(&mut t.target.data.guard_outer_modifier_human, ini.parse_real()?), UserData::None),
        FieldParse::new("GuardChaseUnitsDuration", |ini, t, _| set(&mut t.target.data.guard_chase_unit_frames, ini.parse_duration_unsigned_int()?), UserData::None),
        FieldParse::new("GuardEnemyScanRate", |ini, t, _| set(&mut t.target.data.guard_enemy_scan_rate, ini.parse_duration_unsigned_int()?), UserData::None),
        FieldParse::new("GuardEnemyReturnScanRate", |ini, t, _| set(&mut t.target.data.guard_enemy_return_scan_rate, ini.parse_duration_unsigned_int()?), UserData::None),
        FieldParse::new("SkirmishGroupFudgeDistance", |ini, t, _| set(&mut t.target.data.skirmish_group_fudge_value, ini.parse_real()?), UserData::None),
        FieldParse::new("RepulsedDistance", |ini, t, _| set(&mut t.target.data.repulsed_distance, ini.parse_real()?), UserData::None),
        FieldParse::new("EnableRepulsors", |ini, t, _| set(&mut t.target.data.enable_repulsors, ini.parse_bool()?), UserData::None),
        FieldParse::new("AlertRangeModifier", |ini, t, _| set(&mut t.target.data.alert_range_modifier, ini.parse_real()?), UserData::None),
        FieldParse::new("AggressiveRangeModifier", |ini, t, _| set(&mut t.target.data.aggressive_range_modifier, ini.parse_real()?), UserData::None),
        FieldParse::new("ForceSkirmishAI", |ini, t, _| set(&mut t.target.data.force_skirmish_ai, ini.parse_bool()?), UserData::None),
        FieldParse::new("RotateSkirmishBases", |ini, t, _| set(&mut t.target.data.rotate_skirmish_bases, ini.parse_bool()?), UserData::None),
        FieldParse::new("AttackUsesLineOfSight", |ini, t, _| set(&mut t.target.data.attack_uses_line_of_sight, ini.parse_bool()?), UserData::None),
        FieldParse::new(
            "AttackIgnoreInsignificantBuildings",
            |ini, t, _| set(&mut t.target.data.attack_ignore_insignificant_buildings, ini.parse_bool()?),
            UserData::None,
        ),
        FieldParse::new(
            "AttackPriorityDistanceModifier",
            |ini, t, _| set(&mut t.target.data.attack_priority_distance_modifier, ini.parse_real()?),
            UserData::None,
        ),
        FieldParse::new("MaxRecruitRadius", |ini, t, _| set(&mut t.target.data.max_recruit_distance, ini.parse_real()?), UserData::None),
        FieldParse::new(
            "SkirmishBaseDefenseExtraDistance",
            |ini, t, _| set(&mut t.target.data.skirmish_base_defense_extra_distance, ini.parse_real()?),
            UserData::None,
        ),
        FieldParse::new("WallHeight", |ini, t, _| set(&mut t.target.data.wall_height, ini.parse_real()?), UserData::None),
        FieldParse::new("SideInfo", parse_side_info, UserData::None),
        FieldParse::new("SkirmishBuildList", parse_skirmish_build_list, UserData::None),
        FieldParse::new("MinInfantryForGroup", |ini, t, _| set(&mut t.target.data.min_infantry_for_group, ini.parse_int()?), UserData::None),
        FieldParse::new("MinVehiclesForGroup", |ini, t, _| set(&mut t.target.data.min_vehicles_for_group, ini.parse_int()?), UserData::None),
        FieldParse::new("MinDistanceForGroup", |ini, t, _| set(&mut t.target.data.min_distance_for_group, ini.parse_real()?), UserData::None),
        FieldParse::new("DistanceRequiresGroup", |ini, t, _| set(&mut t.target.data.distance_requires_group, ini.parse_real()?), UserData::None),
        FieldParse::new("MinClumpDensity", |ini, t, _| set(&mut t.target.data.min_clump_density, ini.parse_real()?), UserData::None),
        FieldParse::new("InfantryPathfindDiameter", |ini, t, _| set(&mut t.target.data.infantry_pathfind_diameter, ini.parse_int()?), UserData::None),
        FieldParse::new("VehiclePathfindDiameter", |ini, t, _| set(&mut t.target.data.vehicle_pathfind_diameter, ini.parse_int()?), UserData::None),
        FieldParse::new("RebuildDelayTimeSeconds", |ini, t, _| set(&mut t.target.data.rebuild_delay_seconds, ini.parse_int()?), UserData::None),
        FieldParse::new("SupplyCenterSafeRadius", |ini, t, _| set(&mut t.target.data.supply_center_safe_radius, ini.parse_real()?), UserData::None),
        FieldParse::new(
            "AIDozerBoredRadiusModifier",
            |ini, t, _| set(&mut t.target.data.ai_dozer_bored_radius_modifier, ini.parse_real()?),
            UserData::None,
        ),
        FieldParse::new("AICrushesInfantry", |ini, t, _| set(&mut t.target.data.ai_crushes_infantry, ini.parse_bool()?), UserData::None),
        FieldParse::new("MaxRetaliationDistance", |ini, t, _| set(&mut t.target.data.max_retaliate_distance, ini.parse_real()?), UserData::None),
        FieldParse::new("RetaliationFriendsRadius", |ini, t, _| set(&mut t.target.data.retaliate_friends_radius, ini.parse_real()?), UserData::None),
        FieldParse::end(),
    ]
}

fn set<V>(slot: &mut V, v: V) -> IniResult<()> {
    *slot = v;
    Ok(())
}

/// `AI::parseSideInfo`: the side named by the next token (found, else a new one at the head) and its fields.
fn parse_side_info(ini: &mut Ini<'_>, t: &mut AiIniTarget<'_, AiDataEntry>, _: &UserData) -> IniResult<()> {
    let side = ini.get_next_token(None)?;
    let lists = &mut t.target.lists;
    let i = match lists.side_info.iter().position(|s| s.side == side) {
        Some(i) => i,
        None => {
            lists.add_side_info(AiSideInfo::default());
            0
        }
    };
    lists.side_info[i].side = side;
    let mut target = AiIniTarget { target: &mut lists.side_info[i], sciences: t.sciences, names: &mut *t.names };
    let table: Vec<Fp<'_, AiSideInfo>> = vec![
        field!("ResourceGatherersEasy", easy, parse_int),
        field!("ResourceGatherersNormal", normal, parse_int),
        field!("ResourceGatherersHard", hard, parse_int),
        field!("BaseDefenseStructure1", base_defense_structure1, parse_ascii_string),
        FieldParse::new("SkillSet1", |ini, t, _| parse_skill_set(ini, t, 0), UserData::None),
        FieldParse::new("SkillSet2", |ini, t, _| parse_skill_set(ini, t, 1), UserData::None),
        FieldParse::new("SkillSet3", |ini, t, _| parse_skill_set(ini, t, 2), UserData::None),
        FieldParse::new("SkillSet4", |ini, t, _| parse_skill_set(ini, t, 3), UserData::None),
        FieldParse::new("SkillSet5", |ini, t, _| parse_skill_set(ini, t, 4), UserData::None),
        FieldParse::end(),
    ];
    ini.init_from_ini(&mut target, &table)
}

/// `AI::parseSkillSet`: emptied, then its `Science` lines.
fn parse_skill_set(ini: &mut Ini<'_>, t: &mut AiIniTarget<'_, AiSideInfo>, n: usize) -> IniResult<()> {
    let set = &mut t.target.skill_sets[n];
    set.num_skills = 0;
    let mut target = AiIniTarget { target: set, sciences: t.sciences, names: &mut *t.names };
    let table: Vec<Fp<'_, TSkillSet>> = vec![FieldParse::new("Science", parse_science, UserData::None), FieldParse::end()];
    ini.init_from_ini(&mut target, &table)
}

/// `AI::parseScience`: the next purchasable science into the next slot (a full set reads nothing; a science that costs nothing
/// is written but not counted).
fn parse_science(ini: &mut Ini<'_>, t: &mut AiIniTarget<'_, TSkillSet>, _: &UserData) -> IniResult<()> {
    let set = &mut *t.target;
    if set.num_skills >= MAX_AI_UPGRADES as i32 && !plant(3571) {
        return Ok(());
    }
    let n = set.num_skills as usize;
    set.skills[n] = SCIENCE_INVALID;
    let science = t.sciences.parse_science(ini, t.names)?;
    set.skills[n] = science;
    if science != SCIENCE_INVALID {
        if t.sciences.get_science_purchase_cost(science) == 0 && !plant(3572) {
            return Ok(()); // ("Science %s is not purchaseable, can't be bought.")
        }
        set.num_skills += 1;
    }
    Ok(())
}

/// `AI::parseSkirmishBuildList`: the faction named by the next token, its `Structure` lines, the list added (or replacing the
/// faction's).
fn parse_skirmish_build_list(ini: &mut Ini<'_>, t: &mut AiIniTarget<'_, AiDataEntry>, _: &UserData) -> IniResult<()> {
    let faction = ini.get_next_token(None)?;
    let mut build = AiSideBuildList { side: faction, build_list: Vec::new() };
    let mut target = AiIniTarget { target: &mut build, sciences: t.sciences, names: &mut *t.names };
    let table: Vec<Fp<'_, AiSideBuildList>> = vec![FieldParse::new("Structure", parse_structure, UserData::None), FieldParse::end()];
    ini.init_from_ini(&mut target, &table)?;
    t.target.lists.add_faction_build_list(build);
    Ok(())
}

/// `BuildListInfo::parseStructure` (SidesList.cpp): a build list entry for the template named by the next token, at the end of
/// the list (`AISideBuildList::addInfo`).
fn parse_structure(ini: &mut Ini<'_>, t: &mut AiIniTarget<'_, AiSideBuildList>, _: &UserData) -> IniResult<()> {
    let template_name = ini.get_next_token(None)?;
    let mut info = BuildListInfo::new();
    info.template_name = template_name;
    let mut target = AiIniTarget { target: &mut info, sciences: t.sciences, names: &mut *t.names };
    let table: Vec<Fp<'_, BuildListInfo>> = vec![
        field!("Name", building_name, parse_ascii_string),
        FieldParse::new(
            "Location",
            |ini, t, _| {
                // (a Coord2D written over the Coord3D's x and y)
                let c = ini.parse_coord2d()?;
                t.target.location = Coord3D { x: c.x, y: c.y, z: t.target.location.z };
                Ok(())
            },
            UserData::None,
        ),
        FieldParse::new("Rebuilds", |ini, t, _| set(&mut t.target.num_rebuilds, ini.parse_int()? as u32), UserData::None),
        field!("Angle", angle, parse_angle_real),
        field!("InitiallyBuilt", is_initially_built, parse_bool),
        field!("RallyPointOffset", rally_point_offset, parse_coord2d),
        field!("AutomaticallyBuild", automatically_build, parse_bool),
        FieldParse::end(),
    ];
    ini.init_from_ini(&mut target, &table)?;
    if plant(3573) {
        t.target.build_list.insert(0, info);
    } else {
        t.target.build_list.push(info);
    }
    Ok(())
}

/// `INI::parseAIDataDefinition` (`AI::parseAiDataDefinition`): an `INI_LOAD_CREATE_OVERRIDES` load makes an override first; then
/// the block's fields into the current data.
pub fn parse_ai_data_definition(
    ini: &mut Ini<'_>,
    chain: &mut AiDataChain,
    sciences: &ScienceStore,
    names: &mut NameKeyGenerator,
) -> Result<IniResult<()>, AiDataUndefined> {
    if ini.get_load_type() == IniLoadType::CreateOverrides {
        chain.new_override()?;
    }
    let mut target = AiIniTarget { target: &mut chain.entries[0], sciences, names };
    Ok(ini.init_from_ini(&mut target, &ai_table()))
}

impl TAiData {
    /// `TAiData::crc(xfer)`: 24 of its scalars, in declaration order (not the wall height or the skirmish fudge value).
    pub fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        xfer.xfer_real(&mut self.structure_seconds)?;
        xfer.xfer_real(&mut self.team_seconds)?;
        xfer.xfer_int(&mut self.resources_wealthy)?;
        xfer.xfer_int(&mut self.resources_poor)?;
        xfer.xfer_unsigned_int(&mut self.force_idle_frames_count)?;
        xfer.xfer_real(&mut self.structures_wealthy_mod)?;
        xfer.xfer_real(&mut self.team_wealthy_mod)?;
        xfer.xfer_real(&mut self.structures_poor_mod)?;
        xfer.xfer_real(&mut self.team_poor_mod)?;
        xfer.xfer_real(&mut self.team_resources_to_build)?;
        xfer.xfer_real(&mut self.guard_inner_modifier_ai)?;
        xfer.xfer_real(&mut self.guard_outer_modifier_ai)?;
        xfer.xfer_real(&mut self.guard_inner_modifier_human)?;
        xfer.xfer_real(&mut self.guard_outer_modifier_human)?;
        xfer.xfer_unsigned_int(&mut self.guard_chase_unit_frames)?;
        xfer.xfer_unsigned_int(&mut self.guard_enemy_scan_rate)?;
        xfer.xfer_unsigned_int(&mut self.guard_enemy_return_scan_rate)?;
        xfer.xfer_real(&mut self.alert_range_modifier)?;
        xfer.xfer_real(&mut self.aggressive_range_modifier)?;
        xfer.xfer_real(&mut self.attack_priority_distance_modifier)?;
        xfer.xfer_real(&mut self.max_recruit_distance)?;
        xfer.xfer_real(&mut self.skirmish_base_defense_extra_distance)?;
        xfer.xfer_real(&mut self.repulsed_distance)?;
        xfer.xfer_bool(&mut self.enable_repulsors)
    }

    /// `TAiData::xfer(xfer)` (version 1): the version only.
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)
    }
}
