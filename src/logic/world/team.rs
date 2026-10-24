// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Team.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/Team.cpp
//   (the data of `TeamRelationMap`, `TeamTemplateInfo`, `Team`, `TeamPrototype` and `TeamFactory`;
//   the operations are in `team_ops.rs`, the save/load in `team_xfer.rs`)
//   GeneralsMD/Code/GameEngine/Include/Common/GameCommon.h (`MAKE_DLINK_HEAD`, `MAKE_DLINK`, `DLINK_ITERATOR`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The team arena: `TeamFactory`'s id counters and prototype map, every `TeamPrototype`, every `Team`, and the
//! two intrusive lists (a team's members, a prototype's team instances). Wave 7, `p1w9-sonnet`.
//!
//! # The shape
//!
//! `Team`, `TeamPrototype` and `TeamFactory` live in one [`TeamArena`] (`World::teams`); a team is its [`TeamId`],
//! a prototype its [`TeamPrototypeId`], a player its [`PlayerIndex`] and an object its [`ObjectId`]. The
//! operations that reach anything outside the arena (the player list, the script engine, the objects of a team,
//! the partition manager, the terrain's waypoints, the AI) are the free functions of `team_ops.rs`, generic over a
//! [`TeamHost`](super::team_ops::TeamHost) that owns the arena; the lead's decision of 2026-10-20 (see
//! `docs/world-design.md`, "Player and Team shape").
//!
//! # What is kept from upstream
//!
//! - **Ids are pre-incremented counters that start at 0**, so the first team is 1 and the first prototype is 1
//!   (0 is the invalid id).
//! - **`loadPostProcess` recovers each counter as "highest id in use, plus one"**, after which the next allocation
//!   is `highest + 2`: the counter is incremented before use, so one id is skipped after every load. Kept as
//!   shipped; a replay that crosses a save depends on it.
//! - **`TeamFactory::m_prototypes` is keyed by name key**, not by prototype id: `std::map<NameKeyType, TeamPrototype*>`.
//!   Its order is the order in which the names were first seen, which `xfer` writes and `findTeamByID` searches
//!   in. [`TeamArena::by_key`] is that map; [`TeamArena::prototypes`] is the storage the pointers point into.
//!   A prototype whose name key is already taken is never entered in the map (upstream's `addTeamPrototypeToList`
//!   returns early) but stays alive, and its destructor then removes **by name key**, i.e. the other prototype's
//!   entry. Kept as shipped.
//! - **The lists are `DLINK`s**, not `Vec`s: [`DLinks`] is the macro's algorithm, quirks included. `isInList` is
//!   `head == this || prev || next`, so an object that is in **another** team's list counts as "in the list";
//!   `prependTo` of such an object does nothing; `removeFrom` of it unlinks it from **that** other list and, if it
//!   was that list's head, sets **this** list's head instead. A `DLINK_ITERATOR` reads `next` when it advances,
//!   after the loop body ran, so a body that moves the current object to another team makes the loop continue
//!   in that team's list (upstream's comment in `Team::deleteTeam` describes the bug this causes).
//! - **The relation maps** are `hash_map`s that only `xfer` iterates; the Rust uses ordered maps (the determinism
//!   rule of `docs/phase1-porting-graph.md`).
//!
//! # Where upstream is undefined
//!
//! A load that reads a relationship outside `0..=2` (upstream stores the int in an `enum`) is
//! [`TeamError::InvalidData`]; the other undefined inputs are listed with the operation in `team_ops.rs`.

use std::collections::BTreeMap;

use crate::common::base_type::Coord3D;
use crate::common::dict::Dict;
use crate::common::game_common::Relationship;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use crate::common::well_known_keys::*;
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus};

use super::ids::{PlayerIndex, TeamId, TeamPrototypeId, TEAM_ID_INVALID, TEAM_PROTOTYPE_ID_INVALID};
use super::player::PlayerRelationMap;

/// `MAX_GENERIC_SCRIPTS`.
pub const MAX_GENERIC_SCRIPTS: usize = 16;
/// `TeamTemplateInfo::MAX_UNIT_TYPES`.
pub const MAX_UNIT_TYPES: usize = 7;
/// `TeamPrototype::TEAM_SINGLETON`.
pub const TEAM_SINGLETON: i32 = 0x01;
/// `AI_NORMAL` (`AI.h`, `AttitudeType`).
pub const AI_NORMAL: i32 = 0;

/// A `Waypoint*` is its id (`Waypoint::getID()`).
pub type WaypointId = u32;

/// A `Script*` the script engine owns or lent: an opaque handle the [`TeamHost`](super::team_ops::TeamHost) gives
/// out (`findScriptByName`, `duplicate`) and takes back (`deleteInstance`).
pub type ScriptRef = u32;

/// What the team code throws or cannot continue after (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamError {
    /// `throw ERROR_BAD_ARG`: `createInactiveTeam` of a name with no prototype, `createTeamOnPrototype(NULL)`.
    BadArg,
    /// `throw SC_INVALID_DATA`, or a value upstream stores in an `enum` and cannot hold.
    InvalidData,
    /// An id names nothing where upstream dereferences a pointer (undefined behaviour there).
    Null,
    /// A team or prototype id that is already in use by a team or prototype that is alive (possible upstream only after
    /// `reset()` while a prototype outside the name-key map survives): the arena is keyed by id and cannot hold both.
    IdCollision,
    /// A loop that upstream would never leave (`while (first member) setTeam(...)` with a `setTeam` that does not
    /// remove the member).
    Hang,
    /// What the xfer itself returned.
    Xfer(XferStatus),
}

impl From<XferStatus> for TeamError {
    fn from(s: XferStatus) -> TeamError {
        TeamError::Xfer(s)
    }
}

/// `Relationship` read back from a save (`xferUser( &r, sizeof( Relationship ) )`).
pub(crate) fn relationship_from_i32(v: i32) -> Option<Relationship> {
    match v {
        0 => Some(Relationship::Enemies),
        1 => Some(Relationship::Neutral),
        2 => Some(Relationship::Allies),
        _ => None,
    }
}

// ---- the intrusive lists ------------------------------------------------------------------------------------------

/// One `DLINK_<list>` of an item: its `m_prev` and `m_next`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Link<K> {
    prev: Option<K>,
    next: Option<K>,
}

impl<K> Default for Link<K> {
    fn default() -> Self {
        Link { prev: None, next: None }
    }
}

/// The `MAKE_DLINK` half of a list kind (`DLINK_TeamMemberList`, `DLINK_TeamInstanceList`): the links of every item
/// that has ever been in a list of that kind, by item. The heads are held by the list owners (`MAKE_DLINK_HEAD`).
#[derive(Clone, Debug)]
pub struct DLinks<K: Ord + Copy> {
    links: BTreeMap<K, Link<K>>,
}

impl<K: Ord + Copy> Default for DLinks<K> {
    fn default() -> Self {
        DLinks { links: BTreeMap::new() }
    }
}

impl<K: Ord + Copy> DLinks<K> {
    fn link(&self, o: K) -> Link<K> {
        self.links.get(&o).copied().unwrap_or_default()
    }

    /// `dlink_next_<list>()`.
    pub fn next(&self, o: K) -> Option<K> {
        self.link(o).next
    }

    /// `dlink_prev_<list>()`.
    pub fn prev(&self, o: K) -> Option<K> {
        self.link(o).prev
    }

    /// `dlink_isInList_<list>(&head)`: `*pListHead == this || m_prev || m_next`.
    pub fn is_in_list(&self, head: Option<K>, o: K) -> bool {
        let l = self.link(o);
        head == Some(o) || l.prev.is_some() || l.next.is_some()
    }

    /// `prependTo_<list>(o)` of the head's owner: nothing if `isInList`, else `dlink_prependTo`.
    pub fn prepend(&mut self, head: &mut Option<K>, o: K) {
        if self.is_in_list(*head, o) {
            return;
        }
        // m_next = *pListHead; if (*pListHead) (*pListHead)->m_prev = this; *pListHead = this;
        self.links.entry(o).or_default().next = *head;
        if let Some(h) = *head {
            self.links.entry(h).or_default().prev = Some(o);
        }
        *head = Some(o);
    }

    /// `removeFrom_<list>(o)` of the head's owner: nothing unless `isInList`, else `dlink_removeFrom`.
    pub fn remove_from(&mut self, head: &mut Option<K>, o: K) {
        if !self.is_in_list(*head, o) {
            return;
        }
        let l = self.link(o);
        if let Some(n) = l.next {
            self.links.entry(n).or_default().prev = l.prev;
        }
        if let Some(p) = l.prev {
            self.links.entry(p).or_default().next = l.next;
        } else {
            *head = l.next;
        }
        self.links.remove(&o);
    }

    /// Renames an item (`Team::setID` on a team that is in a list): its entry, its neighbours' references to it.
    /// `None` if the new name is already an item of the list kind (two items of one name cannot be told apart).
    fn rekey(&mut self, head: &mut Option<K>, old: K, new: K) -> bool {
        if old == new {
            return true;
        }
        if self.links.contains_key(&new) || *head == Some(new) {
            return false;
        }
        let l = self.link(old);
        if let Some(n) = l.next {
            self.links.entry(n).or_default().prev = Some(new);
        }
        if let Some(p) = l.prev {
            self.links.entry(p).or_default().next = Some(new);
        }
        if *head == Some(old) {
            *head = Some(new);
        }
        if self.links.remove(&old).is_some() {
            self.links.insert(new, l);
        }
        true
    }

    /// `reverse_<list>()` is not used by this unit (its only call is in a comment).
    #[cfg(test)]
    fn items_from(&self, mut cur: Option<K>) -> Vec<K> {
        let mut v = Vec::new();
        while let Some(c) = cur {
            v.push(c);
            cur = self.next(c);
        }
        v
    }
}

// ---- TeamRelationMap ----------------------------------------------------------------------------------------------

/// `class TeamRelationMap`: the teams this team has a special relationship with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TeamRelationMap {
    /// `m_map` (upstream: `hash_map`; ordered here).
    pub map: BTreeMap<TeamId, Relationship>,
}

impl Snapshot for TeamRelationMap {
    /// `crc`: nothing.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `xfer`, version 1: the count (`UnsignedShort`), then each team id (`xferUser`, 4 bytes) and relationship
    /// (`xferUser`, 4 bytes); a load assigns `m_map[teamID] = r` (an existing entry is overwritten).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current_version = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        let mut count = self.map.len() as u16;
        xfer.xfer_unsigned_short(&mut count)?;

        if xfer.get_xfer_mode() == XferMode::Save {
            for (team_id, r) in self.map.iter() {
                let mut b = team_id.to_le_bytes();
                xfer.xfer_user(&mut b)?;
                let mut rb = (*r as i32).to_le_bytes();
                xfer.xfer_user(&mut rb)?;
            }
        } else {
            for _ in 0..count {
                let mut b = [0u8; 4];
                xfer.xfer_user(&mut b)?;
                let team_id = u32::from_le_bytes(b);
                let mut rb = [0u8; 4];
                xfer.xfer_user(&mut rb)?;
                let r = relationship_from_i32(i32::from_le_bytes(rb)).ok_or(XferStatus::InvalidParameters)?;
                self.map.insert(team_id, r);
            }
        }
        Ok(())
    }

    /// `loadPostProcess`: nothing.
    fn load_post_process(&mut self) {}
}

// ---- TeamTemplateInfo ---------------------------------------------------------------------------------------------

/// `TCreateUnitsInfo`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TCreateUnitsInfo {
    /// `minUnits`.
    pub min_units: i32,
    /// `maxUnits`.
    pub max_units: i32,
    /// `unitThingName`.
    pub unit_thing_name: Vec<u8>,
}

/// A `Waypoint` as `TeamTemplateInfo` reads it: `getName()` and `*getLocation()`.
#[derive(Clone, Debug, PartialEq)]
pub struct WaypointInfo {
    /// `getName()`.
    pub name: Vec<u8>,
    /// `*getLocation()`.
    pub location: Coord3D,
}

/// `class TeamTemplateInfo`: the properties of a team's map `Dict`.
///
/// Members that upstream leaves unset when the dictionary lacks them (the units after `m_numUnitsInfo`) are the
/// zero value here, and nothing reads them.
#[derive(Clone, Debug, PartialEq)]
pub struct TeamTemplateInfo {
    /// `m_unitsInfo`.
    pub units_info: [TCreateUnitsInfo; MAX_UNIT_TYPES],
    /// `m_numUnitsInfo`.
    pub num_units_info: i32,
    /// `m_homeLocation`.
    pub home_location: Coord3D,
    /// `m_hasHomeLocation`.
    pub has_home_location: bool,
    /// `m_scriptOnCreate`.
    pub script_on_create: Vec<u8>,
    /// `m_scriptOnIdle`.
    pub script_on_idle: Vec<u8>,
    /// `m_initialIdleFrames`.
    pub initial_idle_frames: i32,
    /// `m_scriptOnEnemySighted`.
    pub script_on_enemy_sighted: Vec<u8>,
    /// `m_scriptOnAllClear`.
    pub script_on_all_clear: Vec<u8>,
    /// `m_scriptOnUnitDestroyed`.
    pub script_on_unit_destroyed: Vec<u8>,
    /// `m_scriptOnDestroyed`.
    pub script_on_destroyed: Vec<u8>,
    /// `m_destroyedThreshold`.
    pub destroyed_threshold: f32,
    /// `m_isAIRecruitable`.
    pub is_ai_recruitable: bool,
    /// `m_isBaseDefense`.
    pub is_base_defense: bool,
    /// `m_isPerimeterDefense`.
    pub is_perimeter_defense: bool,
    /// `m_automaticallyReinforce`.
    pub automatically_reinforce: bool,
    /// `m_transportsReturn`.
    pub transports_return: bool,
    /// `m_avoidThreats`.
    pub avoid_threats: bool,
    /// `m_attackCommonTarget`.
    pub attack_common_target: bool,
    /// `m_maxInstances`.
    pub max_instances: i32,
    /// `m_productionPriority` (`mutable`).
    pub production_priority: i32,
    /// `m_productionPrioritySuccessIncrease`.
    pub production_priority_success_increase: i32,
    /// `m_productionPriorityFailureDecrease`.
    pub production_priority_failure_decrease: i32,
    /// `m_initialTeamAttitude` (`AttitudeType`, written to saves and maps as an int).
    pub initial_team_attitude: i32,
    /// `m_transportUnitType`.
    pub transport_unit_type: Vec<u8>,
    /// `m_startReinforceWaypoint`.
    pub start_reinforce_waypoint: Vec<u8>,
    /// `m_teamStartsFull`.
    pub team_starts_full: bool,
    /// `m_transportsExit`.
    pub transports_exit: bool,
    /// `m_veterancy` (`VeterancyLevel`).
    pub veterancy: i32,
    /// `m_productionCondition`.
    pub production_condition: Vec<u8>,
    /// `m_executeActions`.
    pub execute_actions: bool,
    /// `m_teamGenericScripts`.
    pub team_generic_scripts: [Vec<u8>; MAX_GENERIC_SCRIPTS],
}

impl TeamTemplateInfo {
    /// `TeamTemplateInfo::TeamTemplateInfo(Dict *d)`. Every `TheKey_*` is looked up where upstream looks it up (the
    /// first use of a `StaticNameKey` allocates its name key, so the order is observable in every later key).
    pub fn from_dict(d: &Dict, gen: &mut NameKeyGenerator, keys: &WellKnownKeys, waypoints: &[WaypointInfo]) -> TeamTemplateInfo {
        let mut t = TeamTemplateInfo {
            units_info: Default::default(),
            num_units_info: 0,
            home_location: Coord3D { x: 0.0, y: 0.0, z: 0.0 },
            has_home_location: false,
            script_on_create: Vec::new(),
            script_on_idle: Vec::new(),
            initial_idle_frames: 0,
            script_on_enemy_sighted: Vec::new(),
            script_on_all_clear: Vec::new(),
            script_on_unit_destroyed: Vec::new(),
            script_on_destroyed: Vec::new(),
            destroyed_threshold: 0.0,
            is_ai_recruitable: false,
            is_base_defense: false,
            is_perimeter_defense: false,
            automatically_reinforce: false,
            transports_return: false,
            avoid_threats: false,
            attack_common_target: false,
            max_instances: 0,
            production_priority: 0,
            production_priority_success_increase: 0,
            production_priority_failure_decrease: 0,
            initial_team_attitude: AI_NORMAL,
            transport_unit_type: Vec::new(),
            start_reinforce_waypoint: Vec::new(),
            team_starts_full: false,
            transports_exit: false,
            veterancy: 0,
            production_condition: Vec::new(),
            execute_actions: false,
            team_generic_scripts: Default::default(),
        };
        // the seven (min, max, type) triples, in the order the constructor reads them
        let triples = [
            (TEAM_UNIT_MIN_COUNT1, TEAM_UNIT_MAX_COUNT1, TEAM_UNIT_TYPE1),
            (TEAM_UNIT_MIN_COUNT2, TEAM_UNIT_MAX_COUNT2, TEAM_UNIT_TYPE2),
            (TEAM_UNIT_MIN_COUNT3, TEAM_UNIT_MAX_COUNT3, TEAM_UNIT_TYPE3),
            (TEAM_UNIT_MIN_COUNT4, TEAM_UNIT_MAX_COUNT4, TEAM_UNIT_TYPE4),
            (TEAM_UNIT_MIN_COUNT5, TEAM_UNIT_MAX_COUNT5, TEAM_UNIT_TYPE5),
            (TEAM_UNIT_MIN_COUNT6, TEAM_UNIT_MAX_COUNT6, TEAM_UNIT_TYPE6),
            (TEAM_UNIT_MIN_COUNT7, TEAM_UNIT_MAX_COUNT7, TEAM_UNIT_TYPE7),
        ];
        for (kmin, kmax, ktype) in triples {
            let (min, _) = d.get_int(keys.key(kmin, gen));
            let (max, _) = d.get_int(keys.key(kmax, gen));
            let (name, exists) = d.get_ascii_string(keys.key(ktype, gen));
            if max > 0 && exists {
                let n = t.num_units_info as usize;
                t.units_info[n] = TCreateUnitsInfo { min_units: min, max_units: max, unit_thing_name: name };
                t.num_units_info += 1;
            }
        }

        let (waypoint, exists) = d.get_ascii_string(keys.key(TEAM_HOME, gen));
        t.home_location = Coord3D { x: 0.0, y: 0.0, z: 0.0 };
        t.has_home_location = false;
        if exists {
            // no `break`: the last waypoint of that name wins
            for way in waypoints {
                if way.name == waypoint {
                    t.home_location = way.location;
                    t.has_home_location = true;
                }
            }
        }

        t.script_on_create = d.get_ascii_string(keys.key(TEAM_ON_CREATE_SCRIPT, gen)).0;
        let (v, exists) = d.get_bool(keys.key(TEAM_IS_AIRECRUITABLE, gen));
        t.is_ai_recruitable = v;
        if !exists {
            t.is_ai_recruitable = false;
        }
        t.is_base_defense = d.get_bool(keys.key(TEAM_IS_BASE_DEFENSE, gen)).0;
        t.is_perimeter_defense = d.get_bool(keys.key(TEAM_IS_PERIMETER_DEFENSE, gen)).0;
        t.automatically_reinforce = d.get_bool(keys.key(TEAM_AUTO_REINFORCE, gen)).0;

        let (interact, exists) = d.get_int(keys.key(TEAM_AGGRESSIVENESS, gen));
        t.initial_team_attitude = AI_NORMAL;
        if exists {
            t.initial_team_attitude = interact;
        }

        t.transports_return = d.get_bool(keys.key(TEAM_TRANSPORTS_RETURN, gen)).0;
        t.avoid_threats = d.get_bool(keys.key(TEAM_AVOID_THREATS, gen)).0;
        t.attack_common_target = d.get_bool(keys.key(TEAM_ATTACK_COMMON_TARGET, gen)).0;
        t.max_instances = d.get_int(keys.key(TEAM_MAX_INSTANCES, gen)).0;

        t.script_on_idle = d.get_ascii_string(keys.key(TEAM_ON_IDLE_SCRIPT, gen)).0;
        t.initial_idle_frames = d.get_int(keys.key(TEAM_INITIAL_IDLE_FRAMES, gen)).0;
        t.script_on_enemy_sighted = d.get_ascii_string(keys.key(TEAM_ENEMY_SIGHTED_SCRIPT, gen)).0;
        t.script_on_all_clear = d.get_ascii_string(keys.key(TEAM_ALL_CLEAR_SCRIPT, gen)).0;
        t.script_on_destroyed = d.get_ascii_string(keys.key(TEAM_ON_DESTROYED_SCRIPT, gen)).0;
        t.destroyed_threshold = d.get_real(keys.key(TEAM_DESTROYED_THRESHOLD, gen)).0;
        t.script_on_unit_destroyed = d.get_ascii_string(keys.key(TEAM_ON_UNIT_DESTROYED_SCRIPT, gen)).0;

        t.production_priority = d.get_int(keys.key(TEAM_PRODUCTION_PRIORITY, gen)).0;
        t.production_priority_success_increase = d.get_int(keys.key(TEAM_PRODUCTION_PRIORITY_SUCCESS_INCREASE, gen)).0;
        t.production_priority_failure_decrease = d.get_int(keys.key(TEAM_PRODUCTION_PRIORITY_FAILURE_DECREASE, gen)).0;

        // Production scripts stuff
        t.production_condition = d.get_ascii_string(keys.key(TEAM_PRODUCTION_CONDITION, gen)).0;
        t.execute_actions = d.get_bool(keys.key(TEAM_EXECUTES_ACTIONS_ON_CREATE, gen)).0;

        // Which scripts to attempt during run?
        for i in 0..MAX_GENERIC_SCRIPTS {
            // keyName.format("%s%d", TheNameKeyGenerator->keyToName(TheKey_teamGenericScriptHook).str(), i)
            let hook_key = keys.key(TEAM_GENERIC_SCRIPT_HOOK, gen);
            let mut key_name = gen.key_to_name(hook_key).to_vec();
            key_name.extend_from_slice(i.to_string().as_bytes());
            let (s, exists) = d.get_ascii_string(gen.name_to_key(&key_name));
            t.team_generic_scripts[i] = s;
            if !exists {
                t.team_generic_scripts[i].clear();
            }
        }

        // reinforcement team info.
        t.transport_unit_type = d.get_ascii_string(keys.key(TEAM_TRANSPORT, gen)).0;
        t.transports_exit = d.get_bool(keys.key(TEAM_TRANSPORTS_EXIT, gen)).0;
        t.team_starts_full = d.get_bool(keys.key(TEAM_STARTS_FULL, gen)).0;
        t.start_reinforce_waypoint = d.get_ascii_string(keys.key(TEAM_REINFORCEMENT_ORIGIN, gen)).0;
        t.veterancy = d.get_int(keys.key(TEAM_VETERANCY, gen)).0;
        t
    }
}

impl Snapshot for TeamTemplateInfo {
    /// `crc`: nothing.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `xfer`, version 1: only `m_productionPriority`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current_version = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        xfer.xfer_int(&mut self.production_priority)
    }

    /// `loadPostProcess`: nothing.
    fn load_post_process(&mut self) {}
}

// ---- Team ---------------------------------------------------------------------------------------------------------

/// `class Team`.
#[derive(Clone, Debug, PartialEq)]
pub struct Team {
    /// `m_proto`, by id.
    pub proto: TeamPrototypeId,
    /// `m_id`.
    pub id: TeamId,
    /// `m_dlinkhead_TeamMemberList`: the first member (newest first).
    pub member_head: Option<ObjectId>,
    /// `m_state`: the name of the current AI state.
    pub state: Vec<u8>,
    /// `m_enteredOrExited`.
    pub entered_or_exited: bool,
    /// `m_active`.
    pub active: bool,
    /// `m_created`.
    pub created: bool,
    /// `m_checkEnemySighted`.
    pub check_enemy_sighted: bool,
    /// `m_seeEnemy`.
    pub see_enemy: bool,
    /// `m_prevSeeEnemy`.
    pub prev_see_enemy: bool,
    /// `m_wasIdle`.
    pub was_idle: bool,
    /// `m_destroyThreshold`.
    pub destroy_threshold: i32,
    /// `m_curUnits`.
    pub cur_units: i32,
    /// `m_currentWaypoint`.
    pub current_waypoint: Option<WaypointId>,
    /// `m_shouldAttemptGenericScript`.
    pub should_attempt_generic_script: [bool; MAX_GENERIC_SCRIPTS],
    /// `m_isRecruitablitySet`.
    pub is_recruitability_set: bool,
    /// `m_isRecruitable`.
    pub is_recruitable: bool,
    /// `m_commonAttackTarget`.
    pub common_attack_target: ObjectId,
    /// `m_teamRelations`.
    pub team_relations: TeamRelationMap,
    /// `m_playerRelations`.
    pub player_relations: PlayerRelationMap,
    /// `m_xferMemberIDList`: the ids a load read and `loadPostProcess` has not checked yet.
    pub xfer_member_id_list: Vec<ObjectId>,
}

impl Team {
    /// `getState()`.
    pub fn get_state(&self) -> &[u8] {
        &self.state
    }

    /// `setState(state)`.
    pub fn set_state(&mut self, state: &[u8]) {
        self.state = state.to_vec();
    }

    /// `setRecruitable(recruitable)`.
    pub fn set_recruitable(&mut self, recruitable: bool) {
        self.is_recruitability_set = true;
        self.is_recruitable = recruitable;
    }

    /// `setEnteredExited()`.
    pub fn set_entered_exited(&mut self) {
        self.entered_or_exited = true;
    }

    /// `getCurrentWaypoint()`.
    pub fn get_current_waypoint(&self) -> Option<WaypointId> {
        self.current_waypoint
    }

    /// `setCurrentWaypoint(way)`.
    pub fn set_current_waypoint(&mut self, way: Option<WaypointId>) {
        self.current_waypoint = way;
    }

    /// The members of `Team::Team(proto, id)` that do not reach anything else: every flag false, no members, empty
    /// relation maps, every generic script to be attempted. (`m_created` is set `FALSE` in the body, the other
    /// flags in the initialiser list; the prototype's list and the script engine's debug message are the
    /// constructor's other effects, done by `team_ops::create_team_instance`.)
    pub fn new(id: TeamId, proto: TeamPrototypeId) -> Team {
        Team {
            proto,
            id,
            member_head: None,
            state: Vec::new(),
            entered_or_exited: false,
            active: false,
            created: false,
            check_enemy_sighted: false,
            see_enemy: false,
            prev_see_enemy: false,
            was_idle: false,
            destroy_threshold: 0,
            cur_units: 0,
            current_waypoint: None,
            should_attempt_generic_script: [true; MAX_GENERIC_SCRIPTS],
            is_recruitability_set: false,
            is_recruitable: false,
            common_attack_target: INVALID_ID,
            team_relations: TeamRelationMap::default(),
            player_relations: PlayerRelationMap::default(),
            xfer_member_id_list: Vec::new(),
        }
    }
}

// ---- TeamPrototype ------------------------------------------------------------------------------------------------

/// `class TeamPrototype`.
#[derive(Clone, Debug, PartialEq)]
pub struct TeamPrototype {
    /// `m_id`.
    pub id: TeamPrototypeId,
    /// `m_owningPlayer`, by index (`None` = NULL, not a top-level team).
    pub owning_player: Option<PlayerIndex>,
    /// `m_name`.
    pub name: Vec<u8>,
    /// `m_flags`.
    pub flags: i32,
    /// `m_productionConditionAlwaysFalse`.
    pub production_condition_always_false: bool,
    /// `m_productionConditionScript`: a duplicate the prototype owns.
    pub production_condition_script: Option<ScriptRef>,
    /// `m_retrievedGenericScripts`.
    pub retrieved_generic_scripts: bool,
    /// `m_genericScriptsToRun`: duplicates the prototype owns.
    pub generic_scripts_to_run: [Option<ScriptRef>; MAX_GENERIC_SCRIPTS],
    /// `m_teamTemplate`.
    pub team_template: TeamTemplateInfo,
    /// `m_attackPriorityName`.
    pub attack_priority_name: Vec<u8>,
    /// `m_dlinkhead_TeamInstanceList`: the newest team instance.
    pub instance_head: Option<TeamId>,
}

impl TeamPrototype {
    /// `getIsSingleton()`.
    pub fn get_is_singleton(&self) -> bool {
        self.flags & TEAM_SINGLETON != 0
    }
}

impl Snapshot for TeamPrototype {
    /// `crc`: nothing. (The save/load of a prototype needs the arena and the player list: `team_xfer.rs`.)
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// Not available without the arena: `team_xfer::xfer_team_prototype`. Reports `XFER_INVALID_PARAMETERS`.
    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Err(XferStatus::InvalidParameters)
    }

    /// `loadPostProcess`: nothing.
    fn load_post_process(&mut self) {}
}

// ---- TeamFactory and the arena ------------------------------------------------------------------------------------

/// `TeamFactory`, with the storage of every team and prototype. See the module docs.
#[derive(Clone, Debug, Default)]
pub struct TeamArena {
    /// `m_uniqueTeamID`.
    unique_team_id: TeamId,
    /// `m_uniqueTeamPrototypeID`.
    unique_prototype_id: TeamPrototypeId,
    /// `m_prototypes`: name key -> prototype (see the module docs).
    by_key: BTreeMap<NameKeyType, TeamPrototypeId>,
    /// The prototypes alive, by id (what `TeamPrototype*` points at).
    prototypes: BTreeMap<TeamPrototypeId, TeamPrototype>,
    /// The teams alive, by id (what `Team*` points at).
    teams: BTreeMap<TeamId, Team>,
    /// `DLINK_TeamMemberList` of every object that is on a team.
    member_links: DLinks<ObjectId>,
    /// `DLINK_TeamInstanceList` of every team instance.
    instance_links: DLinks<TeamId>,
}

impl TeamArena {
    /// `TeamFactory::TeamFactory()`: both counters at the invalid id.
    pub fn new() -> TeamArena {
        TeamArena { unique_team_id: TEAM_ID_INVALID, unique_prototype_id: TEAM_PROTOTYPE_ID_INVALID, ..TeamArena::default() }
    }

    /// `++m_uniqueTeamID`.
    pub fn allocate_team_id(&mut self) -> TeamId {
        self.unique_team_id = self.unique_team_id.wrapping_add(1);
        self.unique_team_id
    }

    /// `++m_uniqueTeamPrototypeID`.
    pub fn allocate_prototype_id(&mut self) -> TeamPrototypeId {
        self.unique_prototype_id = self.unique_prototype_id.wrapping_add(1);
        self.unique_prototype_id
    }

    /// `m_uniqueTeamID` (the last id handed out).
    pub fn last_team_id(&self) -> TeamId {
        self.unique_team_id
    }

    /// `m_uniqueTeamPrototypeID`.
    pub fn last_prototype_id(&self) -> TeamPrototypeId {
        self.unique_prototype_id
    }

    /// The first half of `TeamFactory::reset()`: both counters to the invalid id. (`clear()`, the second half,
    /// deletes the prototypes through the host: `team_ops::clear`.)
    pub fn reset_counters(&mut self) {
        self.unique_team_id = TEAM_ID_INVALID;
        self.unique_prototype_id = TEAM_PROTOTYPE_ID_INVALID;
    }

    /// Adds a team under its id and puts it first in its prototype's instance list, as `Team::Team` does
    /// (`proto->prependTo_TeamInstanceList(this)`). A team whose prototype does not exist is stored alone.
    pub fn insert_team(&mut self, t: Team) -> Result<(), TeamError> {
        let (id, proto) = (t.id, t.proto);
        if self.teams.contains_key(&id) {
            return Err(TeamError::IdCollision);
        }
        self.teams.insert(id, t);
        if let Some(p) = self.prototypes.get_mut(&proto) {
            self.instance_links.prepend(&mut p.instance_head, id);
        }
        Ok(())
    }

    /// Stores a prototype under its id **without** entering it in the name-key map (`addTeamPrototypeToList` does).
    pub fn insert_prototype(&mut self, p: TeamPrototype) -> Result<(), TeamError> {
        if self.prototypes.contains_key(&p.id) {
            return Err(TeamError::IdCollision);
        }
        self.prototypes.insert(p.id, p);
        Ok(())
    }

    /// `m_prototypes[nk]` of `addTeamPrototypeToList`: nothing if the key is taken.
    pub fn add_prototype_to_list(&mut self, nk: NameKeyType, id: TeamPrototypeId) {
        self.by_key.entry(nk).or_insert(id);
    }

    /// `removeTeamPrototypeFromList`'s erase: whatever prototype holds `nk`.
    pub fn remove_prototype_from_list(&mut self, nk: NameKeyType) {
        self.by_key.remove(&nk);
    }

    /// `m_prototypes.find(nk)`.
    pub fn prototype_by_key(&self, nk: NameKeyType) -> Option<TeamPrototypeId> {
        self.by_key.get(&nk).copied()
    }

    /// `m_prototypes` in map order: (name key, prototype id).
    pub fn by_key(&self) -> impl Iterator<Item = (NameKeyType, TeamPrototypeId)> + '_ {
        self.by_key.iter().map(|(k, v)| (*k, *v))
    }

    /// `m_prototypes.size()`.
    pub fn prototype_count(&self) -> usize {
        self.by_key.len()
    }

    /// `m_prototypes = {}` (the map only; the prototypes stay alive until deleted).
    pub fn clear_by_key(&mut self) {
        self.by_key.clear();
    }

    /// The prototype with this id, alive.
    pub fn prototype(&self, id: TeamPrototypeId) -> Option<&TeamPrototype> {
        self.prototypes.get(&id)
    }

    /// The prototype with this id, alive, mutable.
    pub fn prototype_mut(&mut self, id: TeamPrototypeId) -> Option<&mut TeamPrototype> {
        self.prototypes.get_mut(&id)
    }

    /// The team with this id, alive.
    pub fn team(&self, id: TeamId) -> Option<&Team> {
        self.teams.get(&id)
    }

    /// The team with this id, alive, mutable.
    pub fn team_mut(&mut self, id: TeamId) -> Option<&mut Team> {
        self.teams.get_mut(&id)
    }

    /// `findTeamByID(id)` as the existing callers use it: the team alive with that id (the factory's own
    /// `findTeamByID` walks the prototypes; see `team_ops::find_team_by_id`).
    pub fn find_team_by_id(&self, id: TeamId) -> Option<&Team> {
        self.teams.get(&id)
    }

    /// `findTeamPrototypeByID(id)` as a lookup of the storage (the factory's own walks the map in key order;
    /// see `team_ops::find_team_prototype_by_id`).
    pub fn find_prototype_by_id(&self, id: TeamPrototypeId) -> Option<&TeamPrototype> {
        self.prototypes.get(&id)
    }

    /// The prototypes alive, in id order.
    pub fn prototypes(&self) -> impl Iterator<Item = &TeamPrototype> {
        self.prototypes.values()
    }

    /// The teams alive, in id order.
    pub fn teams(&self) -> impl Iterator<Item = &Team> {
        self.teams.values()
    }

    // ---- the member list of a team (MAKE_DLINK_HEAD(Object, TeamMemberList)) ----

    /// `getFirstItemIn_TeamMemberList()`.
    pub fn first_member(&self, team: TeamId) -> Option<ObjectId> {
        self.teams.get(&team).and_then(|t| t.member_head)
    }

    /// `dlink_next_TeamMemberList()` of an object: what a `DLINK_ITERATOR` advances to.
    pub fn next_member(&self, o: ObjectId) -> Option<ObjectId> {
        self.member_links.next(o)
    }

    /// `dlink_prev_TeamMemberList()` of an object.
    pub fn prev_member(&self, o: ObjectId) -> Option<ObjectId> {
        self.member_links.prev(o)
    }

    /// `isInList_TeamMemberList(o)` (see the module docs for the quirk).
    pub fn is_in_member_list(&self, team: TeamId, o: ObjectId) -> bool {
        match self.teams.get(&team) {
            Some(t) => self.member_links.is_in_list(t.member_head, o),
            None => false,
        }
    }

    /// `prependTo_TeamMemberList(o)`.
    pub fn prepend_member(&mut self, team: TeamId, o: ObjectId) {
        if let Some(t) = self.teams.get_mut(&team) {
            self.member_links.prepend(&mut t.member_head, o);
        }
    }

    /// `removeFrom_TeamMemberList(o)`.
    pub fn remove_member(&mut self, team: TeamId, o: ObjectId) {
        if let Some(t) = self.teams.get_mut(&team) {
            self.member_links.remove_from(&mut t.member_head, o);
        }
    }

    /// The members of a team, from its head along `next` (a snapshot: use `first_member` and `next_member` where
    /// the loop body can change the lists).
    pub fn members(&self, team: TeamId) -> Vec<ObjectId> {
        let mut v = Vec::new();
        let mut cur = self.first_member(team);
        while let Some(o) = cur {
            v.push(o);
            cur = self.next_member(o);
        }
        v
    }

    // ---- the instance list of a prototype (MAKE_DLINK_HEAD(Team, TeamInstanceList)) ----

    /// `getFirstItemIn_TeamInstanceList()`.
    pub fn first_instance(&self, proto: TeamPrototypeId) -> Option<TeamId> {
        self.prototypes.get(&proto).and_then(|p| p.instance_head)
    }

    /// `dlink_next_TeamInstanceList()` of a team.
    pub fn next_instance(&self, t: TeamId) -> Option<TeamId> {
        self.instance_links.next(t)
    }

    /// `isInList_TeamInstanceList(t)`.
    pub fn is_in_instance_list(&self, proto: TeamPrototypeId, t: TeamId) -> bool {
        match self.prototypes.get(&proto) {
            Some(p) => self.instance_links.is_in_list(p.instance_head, t),
            None => false,
        }
    }

    /// `removeFrom_TeamInstanceList(t)`.
    pub fn remove_instance(&mut self, proto: TeamPrototypeId, t: TeamId) {
        if let Some(p) = self.prototypes.get_mut(&proto) {
            self.instance_links.remove_from(&mut p.instance_head, t);
        }
    }

    /// The team instances of a prototype, from its head along `next`.
    pub fn instances(&self, proto: TeamPrototypeId) -> Vec<TeamId> {
        let mut v = Vec::new();
        let mut cur = self.first_instance(proto);
        while let Some(t) = cur {
            v.push(t);
            cur = self.next_instance(t);
        }
        v
    }

    /// `m_uniqueTeamID = id` (`TeamFactory::xfer` reads it).
    pub fn set_last_team_id(&mut self, id: TeamId) {
        self.unique_team_id = id;
    }

    /// `Team::setID(newId)`: the team keeps its place in its prototype's list under the new id.
    /// [`TeamError::InvalidData`] if another team already has that id (upstream would have two).
    pub fn set_team_id(&mut self, old: TeamId, new: TeamId) -> Result<(), TeamError> {
        if old == new {
            return Ok(());
        }
        if self.teams.contains_key(&new) {
            return Err(TeamError::InvalidData);
        }
        let mut t = self.teams.remove(&old).ok_or(TeamError::Null)?;
        t.id = new;
        let proto = t.proto;
        self.teams.insert(new, t);
        if let Some(p) = self.prototypes.get_mut(&proto) {
            if !self.instance_links.rekey(&mut p.instance_head, old, new) {
                return Err(TeamError::InvalidData);
            }
        }
        Ok(())
    }

    /// `Team::getName()`: the prototype's.
    pub fn team_name(&self, id: TeamId) -> Option<&[u8]> {
        let t = self.teams.get(&id)?;
        self.prototypes.get(&t.proto).map(|p| p.name.as_slice())
    }

    /// Drops a team's storage (the memory pool's `deleteInstance` after the destructor ran).
    pub fn drop_team(&mut self, id: TeamId) {
        self.teams.remove(&id);
    }

    /// Drops a prototype's storage.
    pub fn drop_prototype(&mut self, id: TeamPrototypeId) {
        self.prototypes.remove(&id);
    }

    /// `TeamFactory::loadPostProcess`'s first part: both counters become the highest id in use plus one (0 when
    /// none), over the prototypes **in the map** and the teams **in their instance lists** (as upstream walks).
    /// See the module docs for the skipped id that follows.
    pub fn recover_counters(&mut self) {
        self.unique_team_id = 0;
        self.unique_prototype_id = 0;
        let ids: Vec<TeamPrototypeId> = self.by_key.values().copied().collect();
        for pid in ids {
            if pid >= self.unique_prototype_id {
                self.unique_prototype_id = pid.wrapping_add(1);
            }
            for t in self.instances(pid) {
                if t >= self.unique_team_id {
                    self.unique_team_id = t.wrapping_add(1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proto(id: u32) -> TeamPrototype {
        let mut gen = NameKeyGenerator::new();
        gen.init();
        let keys = WellKnownKeys::new();
        TeamPrototype {
            id,
            owning_player: Some(1),
            name: b"p".to_vec(),
            flags: 0,
            production_condition_always_false: false,
            production_condition_script: None,
            retrieved_generic_scripts: false,
            generic_scripts_to_run: [None; MAX_GENERIC_SCRIPTS],
            team_template: TeamTemplateInfo::from_dict(&Dict::new(), &mut gen, &keys, &[]),
            attack_priority_name: Vec::new(),
            instance_head: None,
        }
    }

    #[test]
    fn ids_are_pre_incremented_from_zero() {
        let mut t = TeamArena::new();
        assert_eq!(t.allocate_team_id(), 1);
        assert_eq!(t.allocate_team_id(), 2);
        assert_eq!(t.allocate_prototype_id(), 1);
        assert_eq!((t.last_team_id(), t.last_prototype_id()), (2, 1));
        t.reset_counters();
        assert_eq!(t.allocate_team_id(), 1);
    }

    #[test]
    fn load_recovers_highest_plus_one_so_the_next_id_skips_one() {
        let mut t = TeamArena::new();
        t.insert_prototype(proto(4)).unwrap();
        t.insert_prototype(proto(2)).unwrap();
        t.add_prototype_to_list(NameKeyType(10), 4);
        t.add_prototype_to_list(NameKeyType(11), 2);
        t.insert_team(Team::new(9, 4)).unwrap();
        t.insert_team(Team::new(3, 2)).unwrap();
        t.recover_counters();
        assert_eq!((t.last_team_id(), t.last_prototype_id()), (10, 5));
        assert_eq!(t.allocate_team_id(), 11, "9 was the highest; 10 is skipped");
        assert_eq!(t.find_team_by_id(3).map(|x| x.proto), Some(2));
        assert_eq!(t.prototypes().map(|p| p.id).collect::<Vec<_>>(), [2, 4]);
        let mut empty = TeamArena::new();
        empty.allocate_team_id();
        empty.recover_counters();
        assert_eq!(empty.last_team_id(), 0);
        assert_eq!(empty.teams().count(), 0);
    }

    #[test]
    fn dlink_prepends_newest_first_and_removes_from_the_middle() {
        let mut l: DLinks<u32> = DLinks::default();
        let mut head = None;
        for o in [1, 2, 3] {
            l.prepend(&mut head, o);
        }
        assert_eq!(l.items_from(head), [3, 2, 1]);
        l.prepend(&mut head, 2); // already in the list: nothing
        assert_eq!(l.items_from(head), [3, 2, 1]);
        l.remove_from(&mut head, 2);
        assert_eq!(l.items_from(head), [3, 1]);
        assert_eq!((l.prev(1), l.next(3)), (Some(3), Some(1)));
        l.remove_from(&mut head, 3);
        assert_eq!(head, Some(1));
        l.remove_from(&mut head, 1);
        assert_eq!(head, None);
    }

    #[test]
    fn dlink_in_another_list_counts_as_in_list_and_removal_hits_the_wrong_head() {
        // upstream's quirk: `isInList` is `head == this || prev || next`
        let mut l: DLinks<u32> = DLinks::default();
        let (mut a, mut b) = (None, None);
        l.prepend(&mut a, 1);
        l.prepend(&mut a, 2); // a: 2 -> 1
        assert!(l.is_in_list(b, 1), "1 is in a's list, which b's check cannot tell");
        l.prepend(&mut b, 1);
        assert_eq!(b, None, "prependTo of an object already in some list does nothing");
        l.remove_from(&mut b, 2); // 2 is a's head: unlinked from a, and b's head is set to what followed it
        assert_eq!(b, Some(1));
        assert_eq!(a, Some(2), "a still points at the removed head");
    }

    #[test]
    fn team_instances_and_members_use_their_own_lists() {
        let mut t = TeamArena::new();
        t.insert_prototype(proto(1)).unwrap();
        t.insert_team(Team::new(1, 1)).unwrap();
        t.insert_team(Team::new(2, 1)).unwrap();
        assert_eq!(t.insert_team(Team::new(2, 1)), Err(TeamError::IdCollision));
        assert_eq!(t.insert_prototype(proto(1)), Err(TeamError::IdCollision));
        assert_eq!(t.instances(1), [2, 1]);
        assert!(t.is_in_instance_list(1, 2));
        t.remove_instance(1, 2);
        assert_eq!(t.instances(1), [1]);
        t.prepend_member(1, 40);
        t.prepend_member(1, 41);
        assert_eq!(t.members(1), [41, 40]);
        assert!(t.is_in_member_list(1, 40));
        t.remove_member(1, 41);
        assert_eq!(t.first_member(1), Some(40));
        t.drop_team(1);
        assert!(t.team(1).is_none());
        t.drop_prototype(1);
        assert!(t.prototype(1).is_none());
    }

    #[test]
    fn a_taken_name_key_keeps_the_first_prototype_and_a_remove_erases_whoever_holds_it() {
        let mut t = TeamArena::new();
        t.add_prototype_to_list(NameKeyType(5), 1);
        t.add_prototype_to_list(NameKeyType(5), 2);
        assert_eq!(t.prototype_by_key(NameKeyType(5)), Some(1));
        t.remove_prototype_from_list(NameKeyType(5));
        assert_eq!(t.prototype_count(), 0);
    }
}
