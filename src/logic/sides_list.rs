// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/SidesList.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Map/SidesList.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Created: John Ahlquist, November 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The players and teams of a map and the build lists of its non-player
//! sides: [`SidesList`] (`TheSidesList`), its parse from and write to the
//! `SidesList` chunk of a map file, and the validation that makes the list
//! consistent (a neutral player, a default team for every player, allies
//! and enemies that exist, no team named like a player).
//!
//! A *side* corresponds to a `Player` of the game: the dictionary of its
//! properties (`playerName`, `playerFaction`, `playerAllies`, ...), the
//! buildings its AI is to build ([`BuildListInfo`]) and its
//! [`ScriptList`](super::scripts::ScriptList). A *team* is a dictionary
//! (`teamName`, `teamOwner`, `teamIsSingleton`, ...) of a team prototype.
//!
//! # How the C++ shapes became Rust
//!
//! - **`TheSidesList` and the other globals travel in [`SidesCtx`]**: the
//!   list, the state of `Scripts.cpp`, the name keys, the script templates
//!   and `static_readPlayerNames`. It is the context type of the chunk
//!   parsers ([`SidesCtx::read_sides_chunk_file`]).
//! - **Linked lists are vectors** (`BuildListInfo::m_nextBuildList`), and
//!   `SidesInfo *` / `TeamsInfo *` results are indices.
//! - **Template and player pointers are names**: a `BuildListInfo` holds
//!   the building's and its `ThingTemplate`'s names, a team its owner's name
//!   in its dictionary, as in the file; nothing here resolves them.
//! - **`getSideInfo` and `getTeamInfo` out of range** are upstream's
//!   `throw ERROR_BAD_ARG`: `Err(MapDataError::Thrown(ERROR_BAD_ARG))`.
//!
//! # Cut points (what is not here)
//!
//! - **The game's files and players.** `prepareForMP_or_Skirmish` reads
//!   `data\Scripts\SkirmishScripts.scb` through `TheFileSystem`; here the
//!   bytes of that file are [`SidesCtx::skirmish_scripts_file`] (`None` when
//!   it does not exist). Nothing creates a `Player` from a side
//!   (layer 7: `PlayerList`).
//! - **INI:** `BuildListInfo::parseStructure` (the `BuildList` block of the
//!   AI INI files, which hands each entry to the AI's `AISideBuildList`)
//!   belongs with the AI layer.
//! - **Save games:** the `Snapshot` methods (`crc`, `xfer`,
//!   `loadPostProcess`) of `SidesList`. ~~and `BuildListInfo`~~ **CORRECTED 2026-10-20**
//!   (`p1w10-sonnet`): `BuildListInfo`'s are translated (`impl Snapshot for BuildListInfo`), verified through
//!   `Player::xfer` in `tests/differential_player.rs`.
//! - **The editor and the renderer:** `BuildListInfo`'s `m_renderObj`,
//!   `m_shadowObj` (pointers into the 3D scene) are not kept;
//!   `m_selected` is.
//! - **Memory pools:** `MemoryPoolObject`'s `newInstance`/`deleteInstance`.
//!
//! # Where upstream is undefined
//!
//! As in [`super::scripts`]: [`MapUndefined`] where the C++ would
//! dereference `NULL` or index out of bounds (copying a side whose
//! scripts hold such a condition included, since the copy goes through
//! `ScriptList::duplicate`).
//!
//! # Upstream quirks that are kept
//!
//! - `ParseSidesDataChunk` stops reading sides at 16 but reads **no more
//!   data for the rest**: the following fields are read from the bytes of
//!   the sides it skipped (the file is then misread, not rejected).
//! - `removeSide` and `removeTeam` refuse to remove the last one.
//! - `validateSides` runs again after the list is written, and it can
//!   change the list.
//! - `prepareForMP_or_Skirmish` copies every side into the skirmish list and
//!   removes the non-civilian sides from the real list (keeping the last
//!   side whatever it is); the standard skirmish scripts are read only if
//!   no skirmish side has any script.
//! - A `BuildListInfo`'s rally point and automatic build flag are not in
//!   the map file (they come from the AI INI).

use super::scripts::{
    parse_scripts_data_chunk, to_chunk_err, MapDataError, MapResult, MapUndefined, ScriptList,
    ScriptTemplates, ScriptsContext, ScriptsState, MAX_PLAYER_COUNT,
};
use crate::common::ascii_string::{AsciiString, AsciiStringError};
use crate::common::base_type::{Coord2D, Coord3D};
use crate::common::dict::{Dict, DictError};
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use crate::common::well_known_keys as wk;
use crate::common::well_known_keys::WellKnownKeys;
use crate::formats::data_chunk::{
    CachedFileInputStream, ChunkError, ChunkResult, DataChunkInfo, DataChunkInput,
    ERROR_CORRUPT_FILE_FORMAT, ERROR_OUT_OF_MEMORY,
};
use crate::formats::data_chunk_output::DataChunkOutput;

/// `ERROR_BAD_ARG` (Common/Errors.h).
pub const ERROR_BAD_ARG: u32 = 0xdead_0003;
/// `K_SIDES_DATA_VERSION_3`: the version written (3 adds the build list's
/// script, health and flags; 2 adds the team list).
pub const K_SIDES_DATA_VERSION_3: u16 = 3;
/// `BuildListInfo::UNLIMITED_REBUILDS`.
pub const UNLIMITED_REBUILDS: u32 = 0xFFFF_FFFF;
/// `BuildListInfo::MAX_RESOURCE_GATHERERS`.
pub const MAX_RESOURCE_GATHERERS: usize = 10;
/// `SidesList::MAX_TEAM_DEPTH`.
pub const MAX_TEAM_DEPTH: i32 = 3;
/// `INVALID_ID`, the `ObjectID` of no object.
pub const INVALID_ID: i32 = 0;

fn dict_err(_: DictError) -> MapDataError {
    // Dict::setPrep: ERROR_OUT_OF_MEMORY past 32,767 pairs
    MapDataError::Thrown(ERROR_OUT_OF_MEMORY)
}

fn ascii_err(_: AsciiStringError) -> MapDataError {
    // AsciiString's buffer limit: ERROR_OUT_OF_MEMORY
    MapDataError::Thrown(ERROR_OUT_OF_MEMORY)
}

/// The name keys of the map code: `TheNameKeyGenerator` and the
/// `TheKey_*` statics (which cache the key they first got).
#[derive(Clone, Debug, Default)]
pub struct MapNames {
    /// `TheNameKeyGenerator`.
    pub generator: NameKeyGenerator,
    keys: WellKnownKeys,
}

impl MapNames {
    /// A generator and the well-known keys, none looked up yet.
    pub fn new(generator: NameKeyGenerator) -> Self {
        MapNames {
            generator,
            keys: WellKnownKeys::new(),
        }
    }

    /// `TheKey_<name>` (one of the constants of [`wk`]).
    pub fn key(&mut self, which: usize) -> NameKeyType {
        self.keys.key(which, &mut self.generator)
    }

    /// `d.getAsciiString(TheKey_<name>)`: the value, empty if the key is
    /// missing or not an ASCII string.
    pub fn string(&mut self, d: &Dict, which: usize) -> Vec<u8> {
        let k = self.key(which);
        d.get_ascii_string(k).0
    }

    /// `d.setAsciiString(TheKey_<name>, v)`.
    pub fn set_string(&mut self, d: &mut Dict, which: usize, v: &[u8]) -> MapResult<()> {
        let k = self.key(which);
        d.set_ascii_string(k, v).map_err(dict_err)
    }

    /// `d.setBool(TheKey_<name>, v)`.
    pub fn set_bool(&mut self, d: &mut Dict, which: usize, v: bool) -> MapResult<()> {
        let k = self.key(which);
        d.set_bool(k, v).map_err(dict_err)
    }
}

// ---------------------------------------------------------------------------
// BuildListInfo
// ---------------------------------------------------------------------------

/// `class BuildListInfo`: one building of a side's build list.
#[derive(Clone, Debug)]
pub struct BuildListInfo {
    /// `m_buildingName`: the name of this building on the map.
    pub building_name: Vec<u8>,
    /// `m_templateName`: the thing template's name.
    pub template_name: Vec<u8>,
    /// `m_location`.
    pub location: Coord3D,
    /// `m_rallyPointOffset` (from the AI INI, not the map).
    pub rally_point_offset: Coord2D,
    /// `m_angle`.
    pub angle: f32,
    /// `m_isInitiallyBuilt`.
    pub is_initially_built: bool,
    /// `m_numRebuilds` ([`UNLIMITED_REBUILDS`] for no limit).
    pub num_rebuilds: u32,
    /// `m_script`.
    pub script: Vec<u8>,
    /// `m_health`.
    pub health: i32,
    /// `m_whiner`.
    pub whiner: bool,
    /// `m_unsellable`.
    pub unsellable: bool,
    /// `m_repairable`.
    pub repairable: bool,
    /// `m_automaticallyBuild`: whether the AI builds it (else a script must).
    pub automatically_build: bool,
    /// `m_selected` (editor).
    pub selected: bool,
    /// `m_objectID`: the object built from this entry, for the solo AI.
    pub object_id: i32,
    /// `m_objectTimestamp`.
    pub object_timestamp: u32,
    /// `m_underConstruction`.
    pub under_construction: bool,
    /// `m_resourceGatherers`.
    pub resource_gatherers: [i32; MAX_RESOURCE_GATHERERS],
    /// `m_isSupplyBuilding`.
    pub is_supply_building: bool,
    /// `m_desiredGatherers`.
    pub desired_gatherers: i32,
    /// `m_currentGatherers`.
    pub current_gatherers: i32,
    /// `m_priorityBuild`.
    pub priority_build: bool,
}

impl Default for BuildListInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildListInfo {
    /// `BuildListInfo()`: not built, no rebuilds, 100 health, a whiner, not
    /// unsellable, repairable, built automatically.
    pub fn new() -> Self {
        BuildListInfo {
            building_name: Vec::new(),
            template_name: Vec::new(),
            location: Coord3D::default(),
            rally_point_offset: Coord2D::default(),
            angle: 0.0,
            is_initially_built: false,
            num_rebuilds: 0,
            script: Vec::new(),
            health: 100,
            whiner: true,
            unsellable: false,
            repairable: true,
            automatically_build: true,
            selected: false,
            object_id: INVALID_ID,
            object_timestamp: 0,
            under_construction: false,
            resource_gatherers: [INVALID_ID; MAX_RESOURCE_GATHERERS],
            is_supply_building: false,
            desired_gatherers: 0,
            current_gatherers: 0,
            priority_build: false,
        }
    }

    /// `decrementNumRebuilds`: nothing at 0 or unlimited.
    pub fn decrement_num_rebuilds(&mut self) {
        if self.num_rebuilds > 0 && self.num_rebuilds != UNLIMITED_REBUILDS {
            self.num_rebuilds -= 1;
        }
    }

    /// `incrementNumRebuilds`: nothing at unlimited.
    pub fn increment_num_rebuilds(&mut self) {
        if self.num_rebuilds != UNLIMITED_REBUILDS {
            self.num_rebuilds = self.num_rebuilds.wrapping_add(1);
        }
    }

    /// `getNumRebuilds`: an `Int`.
    pub fn get_num_rebuilds(&self) -> i32 {
        self.num_rebuilds as i32
    }

    /// `isBuildable`: rebuilds left (the test compares the `Int`).
    pub fn is_buildable(&self) -> bool {
        self.get_num_rebuilds() > 0 || self.num_rebuilds == UNLIMITED_REBUILDS
    }

    /// `getGathererID(ndx)`: `INVALID_ID` outside the table.
    pub fn get_gatherer_id(&self, ndx: i32) -> i32 {
        if ndx >= 0 && (ndx as usize) < MAX_RESOURCE_GATHERERS {
            self.resource_gatherers[ndx as usize]
        } else {
            INVALID_ID
        }
    }

    /// `setGathererID(ndx, id)`: nothing outside the table.
    pub fn set_gatherer_id(&mut self, ndx: i32, id: i32) {
        if ndx >= 0 && (ndx as usize) < MAX_RESOURCE_GATHERERS {
            self.resource_gatherers[ndx as usize] = id;
        }
    }

    /// `duplicate()` of the head of a build list: a copy of each entry.
    pub fn duplicate_chain(chain: &[BuildListInfo]) -> Vec<BuildListInfo> {
        chain.to_vec()
    }
}

// --- p1w10-sonnet ---
impl crate::common::xfer::Snapshot for BuildListInfo {
    /// `BuildListInfo::crc`: nothing.
    fn crc(
        &mut self,
        _xfer: &mut dyn crate::common::xfer::Xfer,
    ) -> crate::common::xfer::XferResult<()> {
        Ok(())
    }

    /// `BuildListInfo::xfer`, version 2 (2 adds `m_currentGatherers`). `m_renderObj`, `m_shadowObj` and `m_selected` are
    /// the editor's and are not saved; `m_resourceGatherers` is raw bytes (`xferUser`).
    fn xfer(
        &mut self,
        xfer: &mut dyn crate::common::xfer::Xfer,
    ) -> crate::common::xfer::XferResult<()> {
        let current_version: u8 = 2;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        if !crate::logic::world::player_ops::plant(390) {
            xfer.xfer_ascii_string(&mut self.building_name)?;
        }
        if !crate::logic::world::player_ops::plant(391) {
            xfer.xfer_ascii_string(&mut self.template_name)?;
        }
        if !crate::logic::world::player_ops::plant(392) {
            xfer.xfer_coord3d(&mut self.location)?;
        }
        if !crate::logic::world::player_ops::plant(393) {
            xfer.xfer_coord2d(&mut self.rally_point_offset)?;
        }
        if !crate::logic::world::player_ops::plant(394) {
            xfer.xfer_real(&mut self.angle)?;
        }
        if !crate::logic::world::player_ops::plant(395) {
            xfer.xfer_bool(&mut self.is_initially_built)?;
        }
        if !crate::logic::world::player_ops::plant(396) {
            xfer.xfer_unsigned_int(&mut self.num_rebuilds)?;
        }
        if !crate::logic::world::player_ops::plant(397) {
            xfer.xfer_ascii_string(&mut self.script)?;
        }
        if !crate::logic::world::player_ops::plant(398) {
            xfer.xfer_int(&mut self.health)?;
        }
        if !crate::logic::world::player_ops::plant(399) {
            xfer.xfer_bool(&mut self.whiner)?;
        }
        if !crate::logic::world::player_ops::plant(400) {
            xfer.xfer_bool(&mut self.unsellable)?;
        }
        if !crate::logic::world::player_ops::plant(401) {
            xfer.xfer_bool(&mut self.repairable)?;
        }
        if !crate::logic::world::player_ops::plant(402) {
            xfer.xfer_bool(&mut self.automatically_build)?;
        }
        if !crate::logic::world::player_ops::plant(403) {
            xfer.xfer_object_id(&mut self.object_id)?;
        }
        if !crate::logic::world::player_ops::plant(404) {
            xfer.xfer_unsigned_int(&mut self.object_timestamp)?;
        }
        if !crate::logic::world::player_ops::plant(405) {
            xfer.xfer_bool(&mut self.under_construction)?;
        }
        let mut bytes = [0u8; 4 * MAX_RESOURCE_GATHERERS];
        for (i, g) in self.resource_gatherers.iter().enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&g.to_le_bytes());
        }
        if !crate::logic::world::player_ops::plant(406) {
            xfer.xfer_user(&mut bytes)?;
        }
        for (i, g) in self.resource_gatherers.iter_mut().enumerate() {
            *g = i32::from_le_bytes([
                bytes[i * 4],
                bytes[i * 4 + 1],
                bytes[i * 4 + 2],
                if crate::logic::world::player_ops::plant(411) {
                    0
                } else {
                    bytes[i * 4 + 3]
                },
            ]);
        }
        if !crate::logic::world::player_ops::plant(407) {
            xfer.xfer_bool(&mut self.is_supply_building)?;
        }
        if !crate::logic::world::player_ops::plant(408) {
            xfer.xfer_int(&mut self.desired_gatherers)?;
        }
        if !crate::logic::world::player_ops::plant(409) {
            xfer.xfer_bool(&mut self.priority_build)?;
        }
        if (version >= 2) != crate::logic::world::player_ops::plant(389)
            && !crate::logic::world::player_ops::plant(410)
        {
            xfer.xfer_int(&mut self.current_gatherers)?;
        }
        Ok(())
    }

    /// `BuildListInfo::loadPostProcess`: nothing.
    fn load_post_process(&mut self) {}
}
// --- end p1w10-sonnet ---

// ---------------------------------------------------------------------------
// SidesInfo, TeamsInfo, TeamsInfoRec
// ---------------------------------------------------------------------------

/// `class SidesInfo`: a player's dictionary, build list and scripts.
#[derive(Clone, Debug, Default)]
pub struct SidesInfo {
    /// `m_pBuildList` and the chain after it.
    pub build_list: Vec<BuildListInfo>,
    /// `m_dict`: the general player dict.
    pub dict: Dict,
    /// `m_scripts` (`None` for `NULL`).
    pub scripts: Option<ScriptList>,
}

/// Where `addToBuildList(pBuildList, position)` puts the entry: after
/// `position` entries, at the front for 0, at the end for a longer or
/// negative position.
fn build_list_position(len: usize, position: i32) -> usize {
    match position {
        0 => 0,
        p if p > 0 => (p as usize).min(len),
        _ => len,
    }
}

impl SidesInfo {
    /// `SidesInfo()`: empty.
    pub fn new() -> Self {
        SidesInfo::default()
    }

    /// `init(d)`: drop the build list, dict and scripts; take a copy of `d`.
    pub fn init(&mut self, d: Option<&Dict>) {
        self.build_list.clear();
        self.dict.clear();
        self.scripts = None;
        if let Some(d) = d {
            self.dict = d.clone();
        }
    }

    /// `clear()`.
    pub fn clear(&mut self) {
        self.init(None);
    }

    /// `operator=`: a deep copy (the scripts through `ScriptList::duplicate`,
    /// which takes numbers of `cur_id`).
    pub fn assign(
        &mut self,
        that: &SidesInfo,
        cur_id: &mut i32,
        templates: &ScriptTemplates,
    ) -> MapResult<()> {
        self.clear();
        self.dict = that.dict.clone();
        self.build_list = BuildListInfo::duplicate_chain(&that.build_list);
        self.scripts = match &that.scripts {
            Some(s) => Some(s.duplicate(cur_id, templates)?),
            None => None,
        };
        Ok(())
    }

    /// `addToBuildList(pBuildList, position)`.
    pub fn add_to_build_list(&mut self, b: BuildListInfo, position: i32) {
        let at = build_list_position(self.build_list.len(), position);
        self.build_list.insert(at, b);
    }

    /// `removeFromBuildList(pBuildList)`: unlink the entry at `index`,
    /// returning its position (the same). An index outside the list is
    /// [`MapUndefined::NotInList`] (upstream returns a position past the
    /// end and removes nothing).
    pub fn remove_from_build_list(&mut self, index: usize) -> MapResult<i32> {
        if index >= self.build_list.len() {
            return Err(MapUndefined::NotInList.into());
        }
        self.build_list.remove(index);
        Ok(index as i32)
    }

    /// `reorderInBuildList(pBuildList, newPosition)`: remove, then add at
    /// the new position.
    pub fn reorder_in_build_list(&mut self, index: usize, new_position: i32) -> MapResult<()> {
        if index >= self.build_list.len() {
            return Err(MapUndefined::NotInList.into());
        }
        let b = self.build_list.remove(index);
        self.add_to_build_list(b, new_position);
        Ok(())
    }
}

/// `class TeamsInfo`: a team prototype's dictionary.
#[derive(Clone, Debug, Default)]
pub struct TeamsInfo {
    /// `m_dict`.
    pub dict: Dict,
}

impl TeamsInfo {
    /// `init(d)`: clear, then copy `d`.
    pub fn init(&mut self, d: Option<&Dict>) {
        self.dict.clear();
        if let Some(d) = d {
            self.dict = d.clone();
        }
    }

    /// `clear()`.
    pub fn clear(&mut self) {
        self.init(None);
    }
}

/// `class TeamsInfoRec`: a list of teams.
#[derive(Clone, Debug, Default)]
pub struct TeamsInfoRec {
    /// `m_teams[0..m_numTeams]` (upstream allocates them 8 at a time; the
    /// spare ones are always clear).
    pub teams: Vec<TeamsInfo>,
}

impl TeamsInfoRec {
    /// `clear()`.
    pub fn clear(&mut self) {
        self.teams.clear();
    }

    /// `getNumTeams()`.
    pub fn get_num_teams(&self) -> i32 {
        self.teams.len() as i32
    }

    /// `getTeamInfo(team)`: `ERROR_BAD_ARG` outside the list.
    pub fn get_team_info(&self, team: i32) -> MapResult<&TeamsInfo> {
        if team >= 0 && (team as usize) < self.teams.len() {
            Ok(&self.teams[team as usize])
        } else {
            Err(MapDataError::Thrown(ERROR_BAD_ARG))
        }
    }

    /// `getTeamInfo(team)`, to change it.
    pub fn get_team_info_mut(&mut self, team: i32) -> MapResult<&mut TeamsInfo> {
        if team >= 0 && (team as usize) < self.teams.len() {
            Ok(&mut self.teams[team as usize])
        } else {
            Err(MapDataError::Thrown(ERROR_BAD_ARG))
        }
    }

    /// `findTeamInfo(name, &index)`: the index of the first team whose
    /// `teamName` is `name`.
    pub fn find_team_info(&self, names: &mut MapNames, name: &[u8]) -> Option<usize> {
        let teams = &self.teams;
        teams
            .iter()
            .position(|t| names.string(&t.dict, wk::TEAM_NAME) == name)
    }

    /// `addTeam(d)`.
    pub fn add_team(&mut self, d: &Dict) {
        let mut t = TeamsInfo::default();
        t.init(Some(d));
        self.teams.push(t);
    }

    /// `removeTeam(i)`: the later teams move up; nothing if `i` is outside
    /// the list or it holds one team or none.
    pub fn remove_team(&mut self, i: i32) {
        if i < 0 || i as usize >= self.teams.len() || self.teams.len() <= 1 {
            return;
        }
        self.teams.remove(i as usize);
    }
}

// ---------------------------------------------------------------------------
// SidesList
// ---------------------------------------------------------------------------

/// `class SidesList`: the sides (players) and teams of a map, and the
/// copies kept for skirmish and multiplayer.
#[derive(Clone, Debug)]
pub struct SidesList {
    num_sides: i32,
    sides: Vec<SidesInfo>,
    num_skirmish_sides: i32,
    skirmish_sides: Vec<SidesInfo>,
    teamrec: TeamsInfoRec,
    skirmish_teamrec: TeamsInfoRec,
}

impl Default for SidesList {
    fn default() -> Self {
        Self::new()
    }
}

impl SidesList {
    /// `SidesList()`: no sides, no teams.
    pub fn new() -> Self {
        SidesList {
            num_sides: 0,
            sides: vec![SidesInfo::new(); MAX_PLAYER_COUNT],
            num_skirmish_sides: 0,
            skirmish_sides: vec![SidesInfo::new(); MAX_PLAYER_COUNT],
            teamrec: TeamsInfoRec::default(),
            skirmish_teamrec: TeamsInfoRec::default(),
        }
    }

    /// `reset()`.
    pub fn reset(&mut self) {
        self.clear();
    }

    /// `clear()`: no sides and no teams.
    pub fn clear(&mut self) {
        self.empty_sides();
        self.empty_teams();
    }

    /// `getNumSides()`.
    pub fn get_num_sides(&self) -> i32 {
        self.num_sides
    }

    /// `getNumSkirmishSides()`.
    pub fn get_num_skirmish_sides(&self) -> i32 {
        self.num_skirmish_sides
    }

    /// `getNumTeams()`.
    pub fn get_num_teams(&self) -> i32 {
        self.teamrec.get_num_teams()
    }

    /// `getNumSkirmishTeams()`.
    pub fn get_num_skirmish_teams(&self) -> i32 {
        self.skirmish_teamrec.get_num_teams()
    }

    /// `getSideInfo(side)`: `ERROR_BAD_ARG` outside the sides.
    pub fn get_side_info(&self, side: i32) -> MapResult<&SidesInfo> {
        if side >= 0 && side < self.num_sides {
            Ok(&self.sides[side as usize])
        } else {
            Err(MapDataError::Thrown(ERROR_BAD_ARG))
        }
    }

    /// `getSideInfo(side)`, to change it.
    pub fn get_side_info_mut(&mut self, side: i32) -> MapResult<&mut SidesInfo> {
        if side >= 0 && side < self.num_sides {
            Ok(&mut self.sides[side as usize])
        } else {
            Err(MapDataError::Thrown(ERROR_BAD_ARG))
        }
    }

    /// `getSkirmishSideInfo(side)`.
    pub fn get_skirmish_side_info(&self, side: i32) -> MapResult<&SidesInfo> {
        if side >= 0 && side < self.num_skirmish_sides {
            Ok(&self.skirmish_sides[side as usize])
        } else {
            Err(MapDataError::Thrown(ERROR_BAD_ARG))
        }
    }

    /// `getSkirmishSideInfo(side)`, to change it.
    pub fn get_skirmish_side_info_mut(&mut self, side: i32) -> MapResult<&mut SidesInfo> {
        if side >= 0 && side < self.num_skirmish_sides {
            Ok(&mut self.skirmish_sides[side as usize])
        } else {
            Err(MapDataError::Thrown(ERROR_BAD_ARG))
        }
    }

    /// `getTeamInfo(team)`.
    pub fn get_team_info(&self, team: i32) -> MapResult<&TeamsInfo> {
        self.teamrec.get_team_info(team)
    }

    /// `getTeamInfo(team)`, to change it.
    pub fn get_team_info_mut(&mut self, team: i32) -> MapResult<&mut TeamsInfo> {
        self.teamrec.get_team_info_mut(team)
    }

    /// `getSkirmishTeamInfo(team)`.
    pub fn get_skirmish_team_info(&self, team: i32) -> MapResult<&TeamsInfo> {
        self.skirmish_teamrec.get_team_info(team)
    }

    /// `emptySides()`: no sides and no skirmish sides.
    pub fn empty_sides(&mut self) {
        self.num_sides = 0;
        self.num_skirmish_sides = 0;
        for i in 0..MAX_PLAYER_COUNT {
            self.sides[i].clear();
            self.skirmish_sides[i].clear();
        }
    }

    /// `emptyTeams()`: no teams and no skirmish teams.
    pub fn empty_teams(&mut self) {
        self.teamrec.clear();
        self.skirmish_teamrec.clear();
    }

    /// `addSide(d)`: nothing if there are 16 already.
    pub fn add_side(&mut self, d: &Dict) {
        if (self.num_sides as usize) < MAX_PLAYER_COUNT {
            self.sides[self.num_sides as usize].init(Some(d));
            self.num_sides += 1;
        }
    }

    /// `removeSide(i)`: the later sides move up (copied with
    /// `SidesInfo::operator=`); nothing if `i` is outside the sides or there
    /// is one side or none.
    pub fn remove_side(
        &mut self,
        i: i32,
        cur_id: &mut i32,
        templates: &ScriptTemplates,
    ) -> MapResult<()> {
        if i < 0 || i >= self.num_sides || self.num_sides <= 1 {
            return Ok(());
        }
        let mut i = i as usize;
        while i < (self.num_sides - 1) as usize {
            let (head, tail) = self.sides.split_at_mut(i + 1);
            head[i].assign(&tail[0], cur_id, templates)?;
            i += 1;
        }
        while i < MAX_PLAYER_COUNT {
            self.sides[i].clear();
            i += 1;
        }
        self.num_sides -= 1;
        Ok(())
    }

    /// `addTeam(d)`.
    pub fn add_team(&mut self, d: &Dict) {
        self.teamrec.add_team(d);
    }

    /// `addSkirmishTeam(d)`.
    pub fn add_skirmish_team(&mut self, d: &Dict) {
        self.skirmish_teamrec.add_team(d);
    }

    /// `removeTeam(i)`.
    pub fn remove_team(&mut self, i: i32) {
        self.teamrec.remove_team(i);
    }

    /// `findTeamInfo(name, &index)`: the index of the team.
    pub fn find_team_info(&self, names: &mut MapNames, name: &[u8]) -> Option<usize> {
        self.teamrec.find_team_info(names, name)
    }

    /// `findSideInfo(name, &index)`: the index of the first side whose
    /// `playerName` is `name`.
    pub fn find_side_info(&self, names: &mut MapNames, name: &[u8]) -> Option<usize> {
        (0..self.num_sides as usize)
            .find(|&i| names.string(&self.sides[i].dict, wk::PLAYER_NAME) == name)
    }

    /// `findSkirmishSideInfo(name, &index)`.
    pub fn find_skirmish_side_info(&self, names: &mut MapNames, name: &[u8]) -> Option<usize> {
        (0..self.num_skirmish_sides as usize)
            .find(|&i| names.string(&self.skirmish_sides[i].dict, wk::PLAYER_NAME) == name)
    }

    /// `isPlayerDefaultTeam(t)`: whether the team is named `team` followed
    /// by the name of a player.
    pub fn is_player_default_team(&self, names: &mut MapNames, t: &TeamsInfo) -> bool {
        let tname = names.string(&t.dict, wk::TEAM_NAME);
        if let Some(rest) = tname.strip_prefix(b"team") {
            for j in 0..self.num_sides as usize {
                let pname = names.string(&self.sides[j].dict, wk::PLAYER_NAME);
                if pname == rest {
                    return true;
                }
            }
        }
        false
    }

    /// `ScriptList::updateDefaults()`: give every side without a script list
    /// an empty one.
    pub fn update_script_defaults(&mut self) {
        for i in 0..self.num_sides as usize {
            if self.sides[i].scripts.is_none() {
                self.sides[i].scripts = Some(ScriptList::new());
            }
        }
    }

    /// `ScriptList::reset()`: take every side's script list away.
    pub fn reset_script_lists(&mut self) {
        for i in 0..self.num_sides as usize {
            self.sides[i].scripts = None;
        }
    }

    /// `addPlayerByTemplate(playerTemplateName)`: a side for a faction (the
    /// neutral player for an empty name), with its default team.
    pub fn add_player_by_template(
        &mut self,
        names: &mut MapNames,
        player_template_name: &[u8],
    ) -> MapResult<()> {
        let (player_name, display, is_human): (Vec<u8>, Vec<u16>, bool);
        if player_template_name.is_empty() {
            player_name = Vec::new(); // magic code for "neutral"
            display = "Neutral".encode_utf16().collect();
            is_human = false;
        } else {
            let mut n = b"Plyr".to_vec();
            if player_template_name.starts_with(b"Faction") {
                n.extend_from_slice(&player_template_name[7..]);
            } else {
                n.extend_from_slice(player_template_name);
            }
            // UnicodeString::translate: each (signed) char widened
            display = n.iter().map(|&b| b as i8 as u16).collect();
            is_human = n != b"PlyrCivilian";
            player_name = n;
        }

        let mut d = Dict::new();
        d.clear();
        names.set_string(&mut d, wk::PLAYER_NAME, &player_name)?;
        names.set_bool(&mut d, wk::PLAYER_IS_HUMAN, is_human)?;
        let k = names.key(wk::PLAYER_DISPLAY_NAME);
        d.set_unicode_string(k, &display).map_err(dict_err)?;
        names.set_string(&mut d, wk::PLAYER_FACTION, player_template_name)?;
        names.set_string(&mut d, wk::PLAYER_ALLIES, b"")?;
        names.set_string(&mut d, wk::PLAYER_ENEMIES, b"")?;

        self.add_side(&d);

        let mut team_name = b"team".to_vec();
        team_name.extend_from_slice(&player_name);

        d.clear();
        names.set_string(&mut d, wk::TEAM_NAME, &team_name)?;
        names.set_string(&mut d, wk::TEAM_OWNER, &player_name)?;
        names.set_bool(&mut d, wk::TEAM_IS_SINGLETON, true)?;
        self.add_team(&d);
        Ok(())
    }

    /// `validateAllyEnemyList(tname, allies)`: drop the names in a
    /// space-separated list that are `tname` itself or no player. The
    /// result, and whether anything was dropped.
    fn validate_ally_enemy_list(
        &self,
        names: &mut MapNames,
        tname: &[u8],
        allies: &[u8],
    ) -> MapResult<(Vec<u8>, bool)> {
        let mut modified = false;
        let mut s = AsciiString::try_from_bytes(allies).map_err(ascii_err)?;
        let mut newstr: Vec<u8> = Vec::new();
        let mut token = AsciiString::new();
        while s.next_token(&mut token, None).map_err(ascii_err)? {
            if token.str() == tname {
                modified = true;
                continue; // no allies/enemies with self
            }
            if self.find_side_info(names, token.str()).is_none() {
                modified = true;
                continue; // player not found
            }
            if !newstr.is_empty() {
                newstr.push(b' ');
            }
            newstr.extend_from_slice(token.str());
        }
        Ok((newstr, modified))
    }

    /// `validateSides()`: make the sides and teams consistent; true if
    /// anything had to change. There is a neutral player (an empty
    /// `playerName`); every player has a default team `team<name>` owned by
    /// it and a singleton; a player's allies and enemies are players other
    /// than itself; no team is named like a player (the team goes); every
    /// team's owner is a player other than the team (else the neutral one).
    pub fn validate_sides(&mut self, names: &mut MapNames) -> MapResult<bool> {
        let mut modified = false;

        // ensure we have at least one player, and at least one neutral player
        let mut neutral = -1;
        let num = self.get_num_sides();
        for i in 0..num {
            let n = names.string(&self.get_side_info(i)?.dict, wk::PLAYER_NAME);
            if n.is_empty() {
                neutral = i;
                break;
            }
        }
        if neutral == -1 {
            self.add_player_by_template(names, b"")?;
            modified = true;
        }

        // now ensure that every player has a proper "default team"
        let mut i = 0;
        while i < self.get_num_sides() {
            let pdict = self.get_side_info(i)?.dict.clone();
            let pname = names.string(&pdict, wk::PLAYER_NAME);
            let mut tname = b"team".to_vec();
            tname.extend_from_slice(&pname);
            match self.find_team_info(names, &tname) {
                Some(ti) => {
                    // make sure the team owner points back to the player
                    let mut team = std::mem::take(&mut self.teamrec.teams[ti].dict);
                    if names.string(&team, wk::TEAM_OWNER) != pname {
                        names.set_string(&mut team, wk::TEAM_OWNER, &pname)?;
                        modified = true;
                    }
                    // default teams are always singletons
                    let k = names.key(wk::TEAM_IS_SINGLETON);
                    if !team.get_bool(k).0 {
                        names.set_bool(&mut team, wk::TEAM_IS_SINGLETON, true)?;
                        modified = true;
                    }
                    self.teamrec.teams[ti].dict = team;
                }
                None => {
                    let mut d = Dict::new();
                    names.set_string(&mut d, wk::TEAM_NAME, &tname)?;
                    names.set_string(&mut d, wk::TEAM_OWNER, &pname)?;
                    names.set_bool(&mut d, wk::TEAM_IS_SINGLETON, true)?;
                    self.add_team(&d);
                    modified = true;
                }
            }

            let allies = names.string(&pdict, wk::PLAYER_ALLIES);
            let enemies = names.string(&pdict, wk::PLAYER_ENEMIES);

            // ensure all teams have valid allies & enemies
            let (allies, bad) = self.validate_ally_enemy_list(names, &pname, &allies)?;
            if bad {
                let mut d = std::mem::take(&mut self.sides[i as usize].dict);
                names.set_string(&mut d, wk::PLAYER_ALLIES, &allies)?;
                self.sides[i as usize].dict = d;
                modified = true;
            }
            let (enemies, bad) = self.validate_ally_enemy_list(names, &pname, &enemies)?;
            if bad {
                let mut d = std::mem::take(&mut self.sides[i as usize].dict);
                names.set_string(&mut d, wk::PLAYER_ENEMIES, &enemies)?;
                self.sides[i as usize].dict = d;
                modified = true;
            }
            i += 1;
        }

        // ensure there's no overlap between team names and player names
        // (if there is, the player wins and the team is whacked)
        'validate_team_names: loop {
            for i in 0..self.get_num_teams() {
                let tname = names.string(&self.get_team_info(i)?.dict, wk::TEAM_NAME);
                if self.find_side_info(names, &tname).is_some() {
                    if self.teamrec.teams.len() <= 1 {
                        // removeTeam refuses to remove the last team, and the
                        // goto starts over: upstream loops forever
                        return Err(MapUndefined::NonTermination.into());
                    }
                    self.remove_team(i);
                    modified = true;
                    continue 'validate_team_names;
                }
            }
            break;
        }

        for i in 0..self.get_num_teams() {
            let tdict = self.get_team_info(i)?.dict.clone();
            let tname = names.string(&tdict, wk::TEAM_NAME);
            let towner = names.string(&tdict, wk::TEAM_OWNER);
            if self.find_side_info(names, &towner).is_none() || towner == tname {
                let mut d = tdict;
                names.set_string(&mut d, wk::TEAM_OWNER, b"")?;
                self.get_team_info_mut(i)?.dict = d;
                modified = true;
            }
        }

        Ok(modified)
    }
}

// ---------------------------------------------------------------------------
// The context, the parsers, the writer, skirmish
// ---------------------------------------------------------------------------

/// What the sides parsers read into and use: `TheSidesList` and the other
/// globals of `SidesList.cpp` and `Scripts.cpp`.
#[derive(Clone, Debug)]
pub struct SidesCtx {
    /// `TheSidesList`.
    pub sides: SidesList,
    /// The state of `Scripts.cpp`.
    pub scripts: ScriptsState,
    /// `TheNameKeyGenerator` and the `TheKey_*` keys.
    pub names: MapNames,
    /// `TheScriptEngine`'s templates.
    pub templates: ScriptTemplates,
    /// `static_readPlayerNames`.
    pub read_player_names: Vec<Vec<u8>>,
    /// The bytes of `data\Scripts\SkirmishScripts.scb`, if the file exists.
    pub skirmish_scripts_file: Option<Vec<u8>>,
    /// How many strings read by the parsers so far held a 0 before their
    /// end ([`DataChunkInput::embedded_nul_strings`]); the tests that compare
    /// with the C++ stand-in strings leave those files out.
    pub embedded_nul_strings: u64,
}

impl ScriptsContext for SidesCtx {
    fn scripts_state(&mut self) -> &mut ScriptsState {
        &mut self.scripts
    }
    fn names(&mut self) -> &mut NameKeyGenerator {
        &mut self.names.generator
    }
    fn templates(&self) -> &ScriptTemplates {
        &self.templates
    }
}

impl SidesCtx {
    /// A new list and a process that has read nothing.
    pub fn new(names: NameKeyGenerator, templates: ScriptTemplates) -> Self {
        SidesCtx {
            sides: SidesList::new(),
            scripts: ScriptsState::new(),
            names: MapNames::new(names),
            templates,
            read_player_names: vec![Vec::new(); MAX_PLAYER_COUNT],
            skirmish_scripts_file: None,
            embedded_nul_strings: 0,
        }
    }

    /// Read a map file whose top level holds a `SidesList` chunk, as the map
    /// loader registers it: `ParseSidesDataChunk`. False if `data` is empty
    /// (a file `open` refuses) or the parse stops early.
    pub fn read_sides_chunk_file(&mut self, data: Vec<u8>) -> MapResult<bool> {
        let Some(stream) = CachedFileInputStream::new(data) else {
            return Ok(false);
        };
        let mut input = DataChunkInput::<SidesCtx>::new(stream)?;
        input.register_parser(b"SidesList", b"", SidesList::parse_sides_data_chunk);
        let r = input.parse(self);
        self.embedded_nul_strings += input.embedded_nul_strings();
        self.scripts.finish(r)
    }

    /// Read a `.scb`-style file whose top level holds a `PlayerScriptsList`
    /// chunk (`ParseScriptsDataChunk`); the lists are left in the state for
    /// [`ScriptList::get_read_scripts`].
    pub fn read_scripts_chunk_file(&mut self, data: Vec<u8>) -> MapResult<bool> {
        let Some(stream) = CachedFileInputStream::new(data) else {
            return Ok(false);
        };
        let mut input = DataChunkInput::<SidesCtx>::new(stream)?;
        input.register_parser(
            b"PlayerScriptsList",
            b"",
            parse_scripts_data_chunk::<SidesCtx>,
        );
        let r = input.parse(self);
        self.embedded_nul_strings += input.embedded_nul_strings();
        self.scripts.finish(r)
    }

    /// `SidesList::prepareForMP_or_Skirmish()`: keep a copy of the sides and
    /// teams for skirmish and multiplayer, leave the real list with only the
    /// civilian sides (or the last side); if no skirmish side has any script,
    /// take the standard scripts, players and teams from
    /// [`SidesCtx::skirmish_scripts_file`].
    pub fn prepare_for_mp_or_skirmish(&mut self) -> MapResult<()> {
        let st = &mut self.sides;
        st.skirmish_teamrec.clear();
        for i in 0..st.get_num_teams() {
            let d = st.teamrec.get_team_info(i)?.dict.clone();
            st.skirmish_teamrec.add_team(&d);
        }
        st.teamrec.clear();

        for i in 0..MAX_PLAYER_COUNT {
            st.skirmish_sides[i].clear();
        }
        st.num_skirmish_sides = 0;

        let mut i = 0;
        while i < st.num_sides {
            // m_skirmishSides[m_numSkirmishSides] = m_sides[i]
            let (n, src) = (st.num_skirmish_sides as usize, st.sides[i as usize].clone());
            st.skirmish_sides[n].assign(&src, &mut self.scripts.cur_id, &self.templates)?;
            st.num_skirmish_sides += 1;
            if self
                .names
                .string(&st.sides[i as usize].dict, wk::PLAYER_FACTION)
                == b"FactionCivilian"
            {
                // Don't remove FactionCivilian.
                i += 1;
                continue;
            }
            if st.num_sides == 1 {
                break; // can't remove the last side
            }
            st.remove_side(i, &mut self.scripts.cur_id, &self.templates)?;
            // i-- then i++
        }
        let mut got_scripts = false;
        for i in 0..st.num_skirmish_sides as usize {
            if self
                .names
                .string(&st.skirmish_sides[i].dict, wk::PLAYER_FACTION)
                == b"FactionCivilian"
            {
                continue; // Don't consider FactionCivilian.
            }
            let Some(sl) = &st.skirmish_sides[i].scripts else {
                continue;
            };
            if !sl.scripts.is_empty() || !sl.groups.is_empty() {
                got_scripts = true;
            }
        }
        if !got_scripts {
            self.sides.skirmish_teamrec.clear();
            let Some(stream) = self
                .skirmish_scripts_file
                .clone()
                .and_then(CachedFileInputStream::new)
            else {
                return Ok(());
            };
            let mut file = DataChunkInput::<SidesCtx>::new(stream)?;
            file.register_parser(
                b"PlayerScriptsList",
                b"",
                parse_scripts_data_chunk::<SidesCtx>,
            );
            file.register_parser(b"ScriptsPlayers", b"", parse_players_data_chunk);
            file.register_parser(b"ScriptTeams", b"", parse_teams_data_chunk);
            let r = file.parse(self);
            self.embedded_nul_strings += file.embedded_nul_strings();
            if !self.scripts.finish(r)? {
                return Ok(()); // "Unable to read in skirmish scripts"
            }
            let scripts = ScriptList::get_read_scripts(&mut self.scripts);
            for (i, sl) in scripts.into_iter().enumerate() {
                let mut cur_side = -1;
                for j in 0..self.sides.num_skirmish_sides {
                    let name = self
                        .names
                        .string(&self.sides.get_skirmish_side_info(j)?.dict, wk::PLAYER_NAME);
                    if name == self.read_player_names[i] {
                        cur_side = j;
                        break;
                    }
                }
                if cur_side == -1 {
                    continue;
                }
                self.sides.get_skirmish_side_info_mut(cur_side)?.scripts = sl;
            }
            for n in &mut self.read_player_names {
                n.clear();
            }
        }
        Ok(())
    }
}

/// `ParsePlayersDataChunk` (the `ScriptsPlayers` chunk of the skirmish
/// scripts file): the player names, kept in `static_readPlayerNames`.
fn parse_players_data_chunk(
    file: &mut DataChunkInput<SidesCtx>,
    info: &DataChunkInfo,
    ctx: &mut SidesCtx,
) -> ChunkResult<bool> {
    let mut read_dicts = 0;
    if info.version >= 2 {
        read_dicts = file.read_int()?;
    }
    let num_names = file.read_int()?;
    let mut i = 0;
    while i < num_names {
        if i >= MAX_PLAYER_COUNT as i32 {
            break;
        }
        ctx.read_player_names[i as usize] = file.read_ascii_string()?;
        if read_dicts != 0 {
            file.read_dict(&mut ctx.names.generator)?;
        }
        i += 1;
    }
    Ok(true)
}

/// `ParseTeamsDataChunk` (the `ScriptTeams` chunk): each team whose owner is
/// a skirmish side is added to the skirmish teams.
fn parse_teams_data_chunk(
    file: &mut DataChunkInput<SidesCtx>,
    _info: &DataChunkInfo,
    ctx: &mut SidesCtx,
) -> ChunkResult<bool> {
    while !file.at_end_of_chunk() {
        let team_dict = file.read_dict(&mut ctx.names.generator)?;
        let _team_name = ctx.names.string(&team_dict, wk::TEAM_NAME);
        let player = ctx.names.string(&team_dict, wk::TEAM_OWNER);
        if ctx
            .sides
            .find_skirmish_side_info(&mut ctx.names, &player)
            .is_some()
        {
            // player exists, so just add it.
            ctx.sides.add_skirmish_team(&team_dict);
        }
    }
    Ok(true)
}

impl SidesList {
    /// `ParseSidesDataChunk`: the chunk parser of a `SidesList` chunk. Reads
    /// the sides (dict, build list), from version 2 the teams, then the
    /// `PlayerScriptsList` chunk inside (a failure to read it throws
    /// `ERROR_CORRUPT_FILE_FORMAT`), gives each side its script list, and
    /// validates.
    pub fn parse_sides_data_chunk(
        file: &mut DataChunkInput<SidesCtx>,
        info: &DataChunkInfo,
        ctx: &mut SidesCtx,
    ) -> ChunkResult<bool> {
        ctx.sides.clear();
        let count = file.read_int()?;
        ctx.sides.empty_sides();
        let mut i = 0;
        while i < count {
            if i >= MAX_PLAYER_COUNT as i32 {
                break;
            }
            let d = file.read_dict(&mut ctx.names.generator)?;
            ctx.sides.add_side(&d);
            let count = file.read_int()?;
            for j in 0..count {
                let mut b = BuildListInfo::new();
                b.building_name = file.read_ascii_string()?;
                b.template_name = file.read_ascii_string()?;
                let x = file.read_real()?;
                let y = file.read_real()?;
                let _z = file.read_real()?;
                b.location = Coord3D { x, y, z: 0.0 }; // force to ground level
                b.angle = file.read_real()?;
                b.is_initially_built = file.read_byte()? != 0;
                b.num_rebuilds = file.read_int()? as u32;
                if info.version >= 3 {
                    b.script = file.read_ascii_string()?;
                    b.health = file.read_int()?;
                    b.whiner = file.read_byte()? != 0;
                    b.unsellable = file.read_byte()? != 0;
                    b.repairable = file.read_byte()? != 0;
                }
                match ctx.sides.get_side_info_mut(i) {
                    Ok(s) => s.add_to_build_list(b, j),
                    Err(e) => return Err(to_chunk_err(e, &mut ctx.scripts)),
                }
            }
            i += 1;
        }
        if info.version >= 2 {
            let count = file.read_int()?;
            ctx.sides.empty_teams();
            for _ in 0..count {
                let d = file.read_dict(&mut ctx.names.generator)?;
                ctx.sides.add_team(&d);
            }
        }

        file.register_parser(
            b"PlayerScriptsList",
            &info.label,
            parse_scripts_data_chunk::<SidesCtx>,
        );
        if !file.parse(ctx)? {
            return Err(ChunkError::Thrown(ERROR_CORRUPT_FILE_FORMAT));
        }
        let scripts = ScriptList::get_read_scripts(&mut ctx.scripts);
        for (i, sl) in scripts.into_iter().enumerate() {
            if (i as i32) < ctx.sides.get_num_sides() {
                ctx.sides.sides[i].scripts = sl;
            }
            // else: read in more players worth than we have
        }
        if let Err(e) = ctx.sides.validate_sides(&mut ctx.names) {
            return Err(to_chunk_err(e, &mut ctx.scripts));
        }
        Ok(true)
    }

    /// `WriteSidesDataChunk`: a `SidesList` chunk of version 3 with the
    /// sides (dict and build list), the teams and the scripts; then
    /// `validateSides()` runs again (its result is only asserted upstream).
    pub fn write_sides_data_chunk(out: &mut DataChunkOutput, ctx: &mut SidesCtx) -> MapResult<()> {
        out.open_data_chunk(b"SidesList", K_SIDES_DATA_VERSION_3);
        let n = ctx.sides.get_num_sides();
        out.write_int(n);
        for i in 0..n {
            let side = ctx.sides.get_side_info(i)?;
            out.write_dict(&side.dict, &ctx.names.generator);
            out.write_int(side.build_list.len() as i32);
            for b in &side.build_list {
                out.write_ascii_string(&b.building_name);
                out.write_ascii_string(&b.template_name);
                out.write_real(b.location.x);
                out.write_real(b.location.y);
                out.write_real(b.location.z);
                out.write_real(b.angle);
                out.write_byte(i8::from(b.is_initially_built));
                out.write_int(b.get_num_rebuilds());
                // BEGIN stuff new to K_SIDES_DATA_VERSION_3
                out.write_ascii_string(&b.script);
                out.write_int(b.health);
                out.write_byte(i8::from(b.whiner));
                out.write_byte(i8::from(b.unsellable));
                out.write_byte(i8::from(b.repairable));
                // END stuff new to K_SIDES_DATA_VERSION_3
            }
        }

        // BEGIN stuff new to K_SIDES_DATA_VERSION_2
        out.write_int(ctx.sides.get_num_teams());
        for i in 0..ctx.sides.get_num_teams() {
            out.write_dict(&ctx.sides.get_team_info(i)?.dict, &ctx.names.generator);
        }
        // END stuff new to K_SIDES_DATA_VERSION_2

        let mut lists: Vec<Option<&mut ScriptList>> = ctx
            .sides
            .sides
            .iter_mut()
            .take(n as usize)
            .map(|s| s.scripts.as_mut())
            .collect();
        ScriptList::write_scripts_data_chunk(
            out,
            &mut lists,
            &ctx.templates,
            &ctx.names.generator,
        )?;
        out.close_data_chunk();

        ctx.sides.validate_sides(&mut ctx.names)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::scripts::{action_type, condition_type, ScriptList as SL, ScriptTemplate};

    fn templates(g: &mut NameKeyGenerator) -> ScriptTemplates {
        let conds = (0..condition_type::NUM_ITEMS)
            .map(|i| ScriptTemplate {
                internal_name_key: g.name_to_key(format!("C{i}").as_bytes()),
                parameters: vec![],
            })
            .collect();
        let acts = (0..action_type::NUM_ITEMS)
            .map(|i| ScriptTemplate {
                internal_name_key: g.name_to_key(format!("A{i}").as_bytes()),
                parameters: vec![],
            })
            .collect();
        ScriptTemplates::new(conds, acts).unwrap()
    }

    fn ctx() -> SidesCtx {
        let mut g = NameKeyGenerator::new();
        g.init();
        let t = templates(&mut g);
        SidesCtx::new(g, t)
    }

    fn player(c: &mut SidesCtx, name: &str, faction: &str, allies: &str, enemies: &str) -> Dict {
        let mut d = Dict::new();
        let n = &mut c.names;
        n.set_string(&mut d, wk::PLAYER_NAME, name.as_bytes())
            .unwrap();
        n.set_string(&mut d, wk::PLAYER_FACTION, faction.as_bytes())
            .unwrap();
        n.set_string(&mut d, wk::PLAYER_ALLIES, allies.as_bytes())
            .unwrap();
        n.set_string(&mut d, wk::PLAYER_ENEMIES, enemies.as_bytes())
            .unwrap();
        d
    }

    fn pname(c: &mut SidesCtx, i: i32) -> Vec<u8> {
        let d = c.sides.get_side_info(i).unwrap().dict.clone();
        c.names.string(&d, wk::PLAYER_NAME)
    }

    #[test]
    fn build_list_info_counts_and_gatherers() {
        let mut b = BuildListInfo::new();
        assert_eq!(
            (
                b.health,
                b.whiner,
                b.repairable,
                b.automatically_build,
                b.unsellable
            ),
            (100, true, true, true, false)
        );
        assert!(!b.is_buildable());
        b.increment_num_rebuilds();
        assert!(b.is_buildable());
        b.decrement_num_rebuilds();
        b.decrement_num_rebuilds();
        assert_eq!(b.num_rebuilds, 0);
        b.num_rebuilds = UNLIMITED_REBUILDS;
        b.decrement_num_rebuilds();
        b.increment_num_rebuilds();
        assert_eq!(
            (b.num_rebuilds, b.get_num_rebuilds(), b.is_buildable()),
            (UNLIMITED_REBUILDS, -1, true)
        );
        b.set_gatherer_id(3, 77);
        b.set_gatherer_id(10, 5);
        b.set_gatherer_id(-1, 5);
        assert_eq!(
            (
                b.get_gatherer_id(3),
                b.get_gatherer_id(10),
                b.get_gatherer_id(0)
            ),
            (77, INVALID_ID, INVALID_ID)
        );
    }

    #[test]
    fn build_list_order() {
        let mut s = SidesInfo::new();
        let named = |n: &str| BuildListInfo {
            building_name: n.as_bytes().to_vec(),
            ..BuildListInfo::new()
        };
        for (n, p) in [("a", 0), ("b", 5), ("c", 1), ("d", -1), ("e", 0)] {
            s.add_to_build_list(named(n), p);
        }
        let order = |s: &SidesInfo| {
            s.build_list
                .iter()
                .map(|b| b.building_name[0])
                .collect::<Vec<_>>()
        };
        assert_eq!(order(&s), b"eacbd".to_vec());
        assert_eq!(s.remove_from_build_list(2).unwrap(), 2);
        assert_eq!(order(&s), b"eabd".to_vec());
        s.reorder_in_build_list(0, 3).unwrap();
        assert_eq!(order(&s), b"abde".to_vec());
        assert!(s.remove_from_build_list(4).is_err() && s.reorder_in_build_list(9, 0).is_err());
    }

    #[test]
    fn sides_and_teams_bookkeeping() {
        let mut c = ctx();
        assert!(c.sides.get_side_info(0).is_err());
        assert_eq!(
            c.sides.get_side_info(0).unwrap_err(),
            MapDataError::Thrown(ERROR_BAD_ARG)
        );
        for i in 0..18 {
            let d = player(&mut c, &format!("P{i}"), "F", "", "");
            c.sides.add_side(&d);
        }
        assert_eq!(c.sides.get_num_sides(), 16, "no more than 16");
        let mut ids = 0;
        c.sides.remove_side(-1, &mut ids, &c.templates).unwrap();
        c.sides.remove_side(16, &mut ids, &c.templates).unwrap();
        assert_eq!(c.sides.get_num_sides(), 16);
        c.sides.remove_side(3, &mut ids, &c.templates).unwrap();
        assert_eq!(c.sides.get_num_sides(), 15);
        assert_eq!(pname(&mut c, 3), b"P4");
        assert_eq!(pname(&mut c, 14), b"P15");
        assert_eq!(c.sides.find_side_info(&mut c.names, b"P15"), Some(14));
        assert_eq!(c.sides.find_side_info(&mut c.names, b"P3"), None);
        assert!(c.sides.get_skirmish_side_info(0).is_err());
        c.sides.get_side_info_mut(0).unwrap().scripts = Some(SL::new());
        c.sides.update_script_defaults();
        assert!(c.sides.get_side_info(14).unwrap().scripts.is_some());
        c.sides.reset_script_lists();
        assert!(c.sides.get_side_info(0).unwrap().scripts.is_none());
        c.sides.empty_sides();
        assert_eq!(c.sides.get_num_sides(), 0);
        // a single side cannot be removed
        let d = player(&mut c, "Only", "F", "", "");
        c.sides.add_side(&d);
        c.sides.remove_side(0, &mut ids, &c.templates).unwrap();
        assert_eq!(c.sides.get_num_sides(), 1);

        // teams
        for n in ["a", "b", "c"] {
            let mut d = Dict::new();
            c.names
                .set_string(&mut d, wk::TEAM_NAME, n.as_bytes())
                .unwrap();
            c.sides.add_team(&d);
        }
        assert_eq!(c.sides.find_team_info(&mut c.names, b"b"), Some(1));
        assert!(c.sides.get_team_info(3).is_err() && c.sides.get_skirmish_team_info(0).is_err());
        c.sides.remove_team(0);
        assert_eq!(c.sides.find_team_info(&mut c.names, b"b"), Some(0));
        c.sides.remove_team(7);
        c.sides.remove_team(-1);
        c.sides.remove_team(0);
        c.sides.remove_team(0);
        assert_eq!(c.sides.get_num_teams(), 1, "the last team stays");
        c.sides.get_team_info_mut(0).unwrap().clear();
        assert_eq!(c.sides.get_team_info(0).unwrap().dict.get_pair_count(), 0);
        c.sides.add_skirmish_team(&Dict::new());
        assert_eq!(c.sides.get_num_skirmish_teams(), 1);
        c.sides.reset();
        assert_eq!(
            (
                c.sides.get_num_sides(),
                c.sides.get_num_teams(),
                c.sides.get_num_skirmish_teams()
            ),
            (0, 0, 0)
        );
    }

    #[test]
    fn add_player_by_template_names_and_flags() {
        let mut c = ctx();
        c.sides.add_player_by_template(&mut c.names, b"").unwrap();
        c.sides
            .add_player_by_template(&mut c.names, b"FactionAmerica")
            .unwrap();
        c.sides
            .add_player_by_template(&mut c.names, b"Other")
            .unwrap();
        c.sides
            .add_player_by_template(&mut c.names, b"FactionCivilian")
            .unwrap();
        assert_eq!(c.sides.get_num_sides(), 4);
        assert_eq!(pname(&mut c, 0), b"");
        assert_eq!(pname(&mut c, 1), b"PlyrAmerica");
        assert_eq!(pname(&mut c, 2), b"PlyrOther");
        let hum = |c: &mut SidesCtx, i: i32| {
            let d = c.sides.get_side_info(i).unwrap().dict.clone();
            let k = c.names.key(wk::PLAYER_IS_HUMAN);
            d.get_bool(k)
        };
        assert_eq!(hum(&mut c, 0), (false, true));
        assert_eq!(hum(&mut c, 1), (true, true));
        assert_eq!(hum(&mut c, 3), (false, true), "the civilian is not human");
        let d = c.sides.get_side_info(0).unwrap().dict.clone();
        let k = c.names.key(wk::PLAYER_DISPLAY_NAME);
        assert_eq!(
            d.get_unicode_string(k).0,
            "Neutral".encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(c.sides.get_num_teams(), 4);
        let t = c.sides.get_team_info(1).unwrap().clone();
        assert!(c.sides.is_player_default_team(&mut c.names, &t));
        let mut other = Dict::new();
        c.names
            .set_string(&mut other, wk::TEAM_NAME, b"teamNobody")
            .unwrap();
        assert!(!c
            .sides
            .is_player_default_team(&mut c.names, &TeamsInfo { dict: other }));
        let mut high = ctx();
        high.sides
            .add_player_by_template(&mut high.names, &[b'a', 0xe9])
            .unwrap();
        let d = high.sides.get_side_info(0).unwrap().dict.clone();
        let k = high.names.key(wk::PLAYER_DISPLAY_NAME);
        assert_eq!(
            d.get_unicode_string(k).0,
            vec![
                b'P' as u16,
                b'l' as u16,
                b'y' as u16,
                b'r' as u16,
                b'a' as u16,
                0xffe9
            ]
        );
    }

    #[test]
    fn validation_repairs_the_list() {
        let mut c = ctx();
        // two players, one with bad allies/enemies, no neutral, no teams
        let d = player(&mut c, "A", "FactionX", "B  Ghost A", "Ghost");
        c.sides.add_side(&d);
        let d = player(&mut c, "B", "FactionY", "A", "  ");
        c.sides.add_side(&d);
        // a team named like a player, one with an unknown owner, one with the wrong owner
        for (n, o) in [("A", "B"), ("teamA", "B"), ("teamB", "Ghost"), ("Tm", "Tm")] {
            let mut t = Dict::new();
            c.names
                .set_string(&mut t, wk::TEAM_NAME, n.as_bytes())
                .unwrap();
            c.names
                .set_string(&mut t, wk::TEAM_OWNER, o.as_bytes())
                .unwrap();
            c.sides.add_team(&t);
        }
        assert!(c.sides.validate_sides(&mut c.names).unwrap());
        assert_eq!(c.sides.get_num_sides(), 3, "the neutral player was added");
        assert_eq!(pname(&mut c, 2), b"");
        let get = |c: &mut SidesCtx, i: i32, which| {
            let d = c.sides.get_side_info(i).unwrap().dict.clone();
            c.names.string(&d, which)
        };
        assert_eq!(get(&mut c, 0, wk::PLAYER_ALLIES), b"B");
        assert_eq!(get(&mut c, 0, wk::PLAYER_ENEMIES), b"");
        assert_eq!(
            get(&mut c, 1, wk::PLAYER_ENEMIES),
            b"  ",
            "not modified, so not rewritten"
        );
        // teams: "A" removed (a player's name); teamA owned by A and singleton; teamB likewise;
        // Tm reparented to the neutral player; teamteam added for the neutral player is "team"
        let owner = |c: &mut SidesCtx, name: &[u8]| {
            let i = c
                .sides
                .find_team_info(&mut c.names, name)
                .expect("team exists") as i32;
            let d = c.sides.get_team_info(i).unwrap().dict.clone();
            let k = c.names.key(wk::TEAM_IS_SINGLETON);
            (c.names.string(&d, wk::TEAM_OWNER), d.get_bool(k).0)
        };
        assert_eq!(c.sides.find_team_info(&mut c.names, b"A"), None);
        assert_eq!(owner(&mut c, b"teamA"), (b"A".to_vec(), true));
        assert_eq!(owner(&mut c, b"teamB"), (b"B".to_vec(), true));
        assert_eq!(owner(&mut c, b"team"), (b"".to_vec(), true));
        assert_eq!(owner(&mut c, b"Tm").0, b"");
        assert!(
            !c.sides.validate_sides(&mut c.names).unwrap(),
            "a valid list is unchanged"
        );
    }

    /// A map file: TOC, then one top-level `SidesList` chunk.
    fn sides_file(
        c: &mut SidesCtx,
        version: u16,
        body: &dyn Fn(&mut Vec<u8>, &mut DataChunkOutputLike),
    ) -> Vec<u8> {
        let mut o = DataChunkOutputLike::new();
        let mut b = Vec::new();
        body(&mut b, &mut o);
        let _ = c;
        o.finish(version, b)
    }

    /// Builds the TOC of a hand-made file.
    struct DataChunkOutputLike {
        names: Vec<String>,
    }

    impl DataChunkOutputLike {
        fn new() -> Self {
            DataChunkOutputLike {
                names: vec![
                    "SidesList".into(),
                    "PlayerScriptsList".into(),
                    "ScriptList".into(),
                ],
            }
        }
        fn id(&mut self, n: &str) -> u32 {
            if let Some(p) = self.names.iter().position(|x| x == n) {
                return p as u32 + 1;
            }
            self.names.push(n.into());
            self.names.len() as u32
        }
        fn finish(&self, version: u16, body: Vec<u8>) -> Vec<u8> {
            let mut v = b"CkMp".to_vec();
            v.extend_from_slice(&(self.names.len() as i32).to_le_bytes());
            for (i, n) in self.names.iter().enumerate() {
                v.push(n.len() as u8);
                v.extend_from_slice(n.as_bytes());
                v.extend_from_slice(&(i as u32 + 1).to_le_bytes());
            }
            v.extend_from_slice(&1u32.to_le_bytes());
            v.extend_from_slice(&version.to_le_bytes());
            v.extend_from_slice(&(body.len() as i32).to_le_bytes());
            v.extend(body);
            v
        }
    }

    fn put_dict(b: &mut Vec<u8>, o: &mut DataChunkOutputLike, pairs: &[(&str, &str)]) {
        b.extend_from_slice(&(pairs.len() as u16).to_le_bytes());
        for (k, v) in pairs {
            let id = o.id(k);
            b.extend_from_slice(&((id << 8) | 3).to_le_bytes());
            b.extend_from_slice(&(v.len() as u16).to_le_bytes());
            b.extend_from_slice(v.as_bytes());
        }
    }

    fn put_str(b: &mut Vec<u8>, s: &str) {
        b.extend_from_slice(&(s.len() as u16).to_le_bytes());
        b.extend_from_slice(s.as_bytes());
    }

    fn put_build(b: &mut Vec<u8>, version: u16) {
        put_str(b, "Barracks1");
        put_str(b, "AmericaBarracks");
        for x in [10.0f32, 20.0, 30.0, 1.5] {
            b.extend_from_slice(&x.to_le_bytes());
        }
        b.push(1);
        b.extend_from_slice(&(-1i32).to_le_bytes());
        if version >= 3 {
            put_str(b, "OnBuilt");
            b.extend_from_slice(&55i32.to_le_bytes());
            b.extend_from_slice(&[0, 1, 0]);
        }
    }

    #[test]
    fn parse_and_write_a_sides_chunk() {
        let mut c = ctx();
        let file = sides_file(&mut c, 3, &|b, o| {
            b.extend_from_slice(&2i32.to_le_bytes());
            put_dict(
                b,
                o,
                &[
                    ("playerName", "A"),
                    ("playerFaction", "FactionX"),
                    ("playerAllies", "Zed"),
                ],
            );
            b.extend_from_slice(&2i32.to_le_bytes());
            put_build(b, 3);
            put_build(b, 3);
            put_dict(b, o, &[("playerName", "")]);
            b.extend_from_slice(&0i32.to_le_bytes());
            b.extend_from_slice(&1i32.to_le_bytes()); // teams
            put_dict(b, o, &[("teamName", "teamA"), ("teamOwner", "Q")]);
            // PlayerScriptsList with one empty ScriptList
            let mut inner = Vec::new();
            let sl = o.id("ScriptList");
            inner.extend_from_slice(&sl.to_le_bytes());
            inner.extend_from_slice(&1u16.to_le_bytes());
            inner.extend_from_slice(&3i32.to_le_bytes());
            inner.extend_from_slice(&[0, 0, 0]);
            let ps = o.id("PlayerScriptsList");
            b.extend_from_slice(&ps.to_le_bytes());
            b.extend_from_slice(&5u16.to_le_bytes());
            b.extend_from_slice(&(inner.len() as i32).to_le_bytes());
            b.extend(inner);
        });
        assert!(c.read_sides_chunk_file(file).unwrap());
        assert_eq!(c.sides.get_num_sides(), 2);
        let a = c.sides.get_side_info(0).unwrap();
        assert_eq!(a.build_list.len(), 2);
        let bl = &a.build_list[0];
        assert_eq!(
            (
                &bl.building_name[..],
                &bl.template_name[..],
                bl.location.z,
                bl.angle,
                bl.num_rebuilds
            ),
            (
                &b"Barracks1"[..],
                &b"AmericaBarracks"[..],
                0.0,
                1.5,
                UNLIMITED_REBUILDS
            )
        );
        assert_eq!(
            (
                &bl.script[..],
                bl.health,
                bl.whiner,
                bl.unsellable,
                bl.repairable,
                bl.is_initially_built
            ),
            (&b"OnBuilt"[..], 55, false, true, false, true)
        );
        assert!(
            a.scripts.is_some() && c.sides.get_side_info(1).unwrap().scripts.is_none(),
            "one script list for the first side only"
        );
        // validation ran: allies "Zed" dropped, default teams made
        assert_eq!(
            c.sides.get_num_teams(),
            2,
            "teamA kept (owner repaired), `team` added for the neutral player"
        );
        let d = c.sides.get_side_info(0).unwrap().dict.clone();
        assert_eq!(c.names.string(&d, wk::PLAYER_ALLIES), b"");
        // write it back and read that
        let mut out = DataChunkOutput::new();
        SidesList::write_sides_data_chunk(&mut out, &mut c).unwrap();
        let bytes = out.finish();
        // the written file has the SidesList chunk at the top level
        let mut c2 = ctx();
        c2.names = c.names.clone();
        assert!(c2.read_sides_chunk_file(bytes.clone()).unwrap());
        let mut out2 = DataChunkOutput::new();
        SidesList::write_sides_data_chunk(&mut out2, &mut c2).unwrap();
        assert_eq!(out2.finish(), bytes, "write(read(write(x))) == write(x)");
        assert_eq!(c2.sides.get_num_sides(), 2);
        assert_eq!(c2.sides.get_side_info(0).unwrap().build_list[1].health, 55);
    }

    #[test]
    fn version_1_has_no_teams_and_old_build_entries() {
        let mut c = ctx();
        let file = sides_file(&mut c, 1, &|b, o| {
            b.extend_from_slice(&1i32.to_le_bytes());
            put_dict(b, o, &[("playerName", "")]);
            b.extend_from_slice(&1i32.to_le_bytes());
            put_build(b, 1);
            let ps = o.id("PlayerScriptsList");
            b.extend_from_slice(&ps.to_le_bytes());
            b.extend_from_slice(&5u16.to_le_bytes());
            b.extend_from_slice(&0i32.to_le_bytes());
        });
        assert!(c.read_sides_chunk_file(file).unwrap());
        let bl = &c.sides.get_side_info(0).unwrap().build_list[0];
        assert_eq!(
            (bl.health, bl.whiner, bl.repairable, &bl.script[..]),
            (100, true, true, &b""[..])
        );
        assert_eq!(
            c.sides.get_num_teams(),
            1,
            "only the default team validation made"
        );
    }

    #[test]
    fn a_missing_scripts_chunk_is_not_an_error_and_an_oversized_one_is() {
        let mut c = ctx();
        let one_side = |b: &mut Vec<u8>, o: &mut DataChunkOutputLike| {
            b.extend_from_slice(&1i32.to_le_bytes());
            put_dict(b, o, &[("playerName", "")]);
            b.extend_from_slice(&0i32.to_le_bytes());
            b.extend_from_slice(&0i32.to_le_bytes());
            b.extend_from_slice(&[0, 0, 0]); // keeps the chunk from ending the file
        };
        let file = sides_file(&mut c, 2, &one_side);
        assert!(c.read_sides_chunk_file(file).unwrap());
        assert!(c.sides.get_side_info(0).unwrap().scripts.is_none());
        // 17 script lists make ParseScriptsDataChunk fail: ERROR_CORRUPT_FILE_FORMAT
        let file = sides_file(&mut c, 2, &|b, o| {
            b.extend_from_slice(&1i32.to_le_bytes());
            put_dict(b, o, &[("playerName", "")]);
            b.extend_from_slice(&0i32.to_le_bytes());
            b.extend_from_slice(&0i32.to_le_bytes());
            let mut inner = Vec::new();
            for _ in 0..17 {
                inner.extend_from_slice(&3u32.to_le_bytes());
                inner.extend_from_slice(&1u16.to_le_bytes());
                inner.extend_from_slice(&3i32.to_le_bytes());
                inner.extend_from_slice(&[0, 0, 0]);
            }
            b.extend_from_slice(&2u32.to_le_bytes());
            b.extend_from_slice(&5u16.to_le_bytes());
            b.extend_from_slice(&(inner.len() as i32).to_le_bytes());
            b.extend(inner);
        });
        assert_eq!(
            c.read_sides_chunk_file(file).unwrap_err(),
            MapDataError::Thrown(ERROR_CORRUPT_FILE_FORMAT)
        );
        assert!(!c.read_sides_chunk_file(Vec::new()).unwrap());
    }

    #[test]
    fn skirmish_preparation_keeps_civilians_and_reads_the_standard_scripts() {
        let mut c = ctx();
        for (n, f) in [
            ("PlyrA", "FactionA"),
            ("Civ", "FactionCivilian"),
            ("PlyrB", "FactionB"),
            ("", ""),
        ] {
            let d = player(&mut c, n, f, "", "");
            c.sides.add_side(&d);
        }
        c.sides.get_side_info_mut(0).unwrap().scripts = Some(SL::new());
        c.sides.validate_sides(&mut c.names).unwrap();
        let teams = c.sides.get_num_teams();
        c.prepare_for_mp_or_skirmish().unwrap();
        assert_eq!(c.sides.get_num_skirmish_sides(), 4);
        assert_eq!(
            c.sides
                .get_skirmish_side_info(2)
                .unwrap()
                .dict
                .get_pair_count(),
            4
        );
        assert_eq!(c.sides.get_num_sides(), 1, "only the civilian side is kept");
        assert_eq!(pname(&mut c, 0), b"Civ");
        assert_eq!(
            (c.sides.get_num_teams(), c.sides.get_num_skirmish_teams()),
            (0, 0),
            "no file: no skirmish teams"
        );
        let _ = teams;

        // with the standard scripts file: players, teams, and scripts for PlyrA only
        let mut c = ctx();
        for (n, f) in [("PlyrA", "FactionA"), ("PlyrB", "FactionB")] {
            let d = player(&mut c, n, f, "", "");
            c.sides.add_side(&d);
        }
        let mut o = DataChunkOutput::new();
        // ScriptsPlayers v2: readDicts=1, two names with a dict each
        o.open_data_chunk(b"ScriptsPlayers", 2);
        o.write_int(1);
        o.write_int(3);
        for n in [&b"PlyrB"[..], b"Nobody", b"PlyrA"] {
            o.write_ascii_string(n);
            o.write_dict(&Dict::new(), &c.names.generator);
        }
        o.close_data_chunk();
        o.open_data_chunk(b"ScriptTeams", 1);
        for (team, owner) in [("tB", "PlyrB"), ("tN", "Nobody")] {
            let mut d = Dict::new();
            c.names
                .set_string(&mut d, wk::TEAM_NAME, team.as_bytes())
                .unwrap();
            c.names
                .set_string(&mut d, wk::TEAM_OWNER, owner.as_bytes())
                .unwrap();
            o.write_dict(&d, &c.names.generator);
        }
        o.close_data_chunk();
        // three script lists; the second (for "Nobody") is dropped, the first goes to PlyrB
        let mut names = [SL::new(), SL::new(), SL::new()];
        names[0].scripts.push(crate::logic::scripts::Script {
            script_name: b"forB".to_vec(),
            ..Default::default()
        });
        names[2].scripts.push(crate::logic::scripts::Script {
            script_name: b"forA".to_vec(),
            ..Default::default()
        });
        let mut refs: Vec<Option<&mut SL>> = names.iter_mut().map(Some).collect();
        SL::write_scripts_data_chunk(&mut o, &mut refs, &c.templates, &c.names.generator).unwrap();
        c.skirmish_scripts_file = Some(o.finish());
        c.prepare_for_mp_or_skirmish().unwrap();
        assert_eq!(c.sides.get_num_sides(), 1, "the last side cannot go");
        assert_eq!(c.sides.get_num_skirmish_sides(), 2);
        assert_eq!(
            c.sides.get_num_skirmish_teams(),
            1,
            "only the team whose owner is a skirmish player"
        );
        let a = c
            .sides
            .get_skirmish_side_info(0)
            .unwrap()
            .scripts
            .as_ref()
            .expect("scripts for PlyrA");
        let b = c
            .sides
            .get_skirmish_side_info(1)
            .unwrap()
            .scripts
            .as_ref()
            .expect("scripts for PlyrB");
        assert_eq!(
            (&a.scripts[0].script_name[..], &b.scripts[0].script_name[..]),
            (&b"forA"[..], &b"forB"[..])
        );
        assert!(c.read_player_names.iter().all(|n| n.is_empty()));
        // a file that fails to parse leaves the names
        c.skirmish_scripts_file = Some(vec![1, 2, 3]);
        let d = player(&mut c, "PlyrC", "FactionC", "", "");
        c.sides.add_side(&d);
        c.prepare_for_mp_or_skirmish().unwrap();
    }

    #[test]
    fn validation_adds_the_neutral_player_and_default_teams() {
        let mut c = ctx();
        let d = player(&mut c, "x", "F", "", "");
        c.sides.add_side(&d);
        assert!(c.sides.validate_sides(&mut c.names).unwrap());
        assert_eq!(c.sides.get_num_teams(), 2);
    }
}
