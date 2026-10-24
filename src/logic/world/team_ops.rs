// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Team.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/Team.cpp
//   (`Team`, `TeamPrototype` and `TeamFactory`: every member function except `xfer`/`loadPostProcess`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The operations of `Team`, `TeamPrototype` and `TeamFactory`, as free functions over a [`TeamHost`] (the world
//! that owns the [`TeamArena`] and answers for everything the team code reaches outside it). Wave 7, `p1w9-sonnet`.
//! The data and the lists are in `team.rs`; the save and load in `team_xfer.rs`.
//!
//! # The cut points (the host)
//!
//! Each [`TeamHost`] method is called where upstream makes the call, in upstream's order, with the same
//! short-circuiting; the differential test writes one line for every call that has an effect and compares the
//! sequences. The groups:
//!
//! - **objects**: `Object`'s queries and the few things a team does to its members (`kill`, `attemptDamage`,
//!   `healCompletely`, `setTeam`, `handlePartitionCellMaintenance`, the contain module's `removeAllContained`,
//!   `TheGameLogic->destroyObject`). `setTeam` is the host's, because it changes the member lists: the host calls
//!   [`TeamArena::remove_member`]/[`TeamArena::prepend_member`] (and the object's own team), as `Object::setTeam`
//!   does with the `DLINK`s.
//! - **players and the player list**: `addTeamToList`, `removeTeamFromList`, `preTeamDestroy`, `getDefaultTeam`,
//!   `getRelationship(team)`, the type and difficulty, the beacon template, `findPlayerWithNameKey`, the neutral
//!   player, `getNthPlayer`, `teamAboutToBeDeleted`. (`Player` and `PlayerList` are the next units of this wave.)
//! - **the script engine**: `findScriptByName`, `duplicate`, `deleteInstance`, the script's getters,
//!   `evaluateConditions`, `friend_executeAction`, `runScript`, `notifyOfTeamDestruction`, `AppendDebugMessage`.
//!   A `Script*` is an opaque [`ScriptRef`](super::team::ScriptRef) (layer 11 owns scripts).
//! - **the partition manager** (the enemy sighted test), **the AI** (`isIdle`, `getValidSurfaces`,
//!   `isRecruitable`, `AIGroup::add`), **polygon triggers** (`didEnter`, `didExit`, `isInside`), **the terrain's
//!   waypoints**, **the thing templates** (identity, kind-of, build variations).
//!
//! # Where upstream is undefined
//!
//! The Rust returns [`TeamError`] (the tests count these and do not give them to the C++; an id collision is
//! [`TeamError::IdCollision`], see `team.rs`): a `NULL` controlling
//! player dereferenced (`getRelationship`, `setTeamTargetObject`, `deleteTeam`, `tryToRecruit`, `killTeam`,
//! `evaluateProductionCondition`); a `Team*` that names no team; a member whose template is `NULL` where it is
//! dereferenced; `createInactiveTeam` of a name with no prototype and `createTeamOnPrototype(NULL)` (these two
//! `throw ERROR_BAD_ARG`: [`TeamError::BadArg`]); and a loop that never ends (`~Team` and `transferUnitsTo` loop
//! "while the first member is not NULL" and rely on `setTeam` removing it: [`TeamError::Hang`] after 10,000
//! rounds).
//!
//! # Upstream defects kept as shipped
//!
//! - `Team::hasAnyBuildings(kindOf)` tests the objects, not the templates, and sets `KINDOF_STRUCTURE` in its copy
//!   of the mask on every pass (harmless).
//! - `Team::deleteTeam`, `killTeam` and `transferUnitsTo` iterate lists that their own body edits; see
//!   `team.rs` ("The lists are `DLINK`s") for what the iterator does then.
//! - `locoSetMatches` maps `whichToConsider` bit 1 (air) to `LOCOMOTORSURFACE_AIR` by `<< 2` of the *water* bit
//!   (value 2): `(bits & 1) | ((bits & 2) << 2)`, so bit 0 is ground and bit 1 is air; the water and cliff
//!   surfaces can never match.
//! - `TeamPrototype::updateState` deletes an empty team inside the loop that walks the list and starts over.
//! - `TeamFactory::loadPostProcess` skips one team id and one prototype id (`team.rs`).
//! - `Team::moveTeamTo` and `TeamPrototype::moveTeamTo` do nothing but test each member for being dead
//!   (upstream's own `@todo`).
//! - `TeamPrototype::removeTeamPrototypeFromList` erases by name key, whatever prototype holds it.

use crate::common::base_type::Coord3D;
use crate::common::dict::Dict;
use crate::common::game_common::{GameDifficulty, PlayerType, Relationship, LOGICFRAMES_PER_SECOND};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::kind_of::{
    KindOfMaskType, KindOfType, KINDOFMASK_NONE, KINDOF_AIRCRAFT, KINDOF_INERT, KINDOF_MINE, KINDOF_PROJECTILE, KINDOF_STRUCTURE, KINDOF_TECH_BUILDING,
};
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use crate::common::object_status_types::{
    ObjectStatusTypes, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_STEALTHED, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use crate::common::well_known_keys::{WellKnownKeys, TEAM_IS_SINGLETON, TEAM_NAME, TEAM_OWNER};
use crate::logic::locomotor::LOCOMOTORSURFACE_GROUND;
use crate::logic::sides_list::SidesList;

use super::ids::{PlayerIndex, TeamId, TeamPrototypeId, TEAM_ID_INVALID};
use super::team::*;

/// A `ThingTemplate*`: an opaque handle the host gives out (`Object::getTemplate()`, `findTemplate`).
pub type TemplateRef = u32;
/// A `PolygonTrigger*`.
pub type TriggerRef = u32;
/// An `AIGroup*`.
pub type GroupRef = u32;

/// Everything the team code reaches outside the team arena. See the module docs.
pub trait TeamHost {
    /// The arena (`TheTeamFactory`'s state).
    fn teams(&self) -> &TeamArena;
    /// The arena, mutable.
    fn teams_mut(&mut self) -> &mut TeamArena;

    // ---- name keys ----
    /// `TheNameKeyGenerator` and the well-known keys (`TheKey_*`).
    fn names(&mut self) -> (&mut NameKeyGenerator, &WellKnownKeys);
    /// `NAMEKEY(name)`.
    fn name_to_key(&mut self, name: &[u8]) -> NameKeyType {
        self.names().0.name_to_key(name)
    }

    // ---- TheGameLogic ----
    /// `TheGameLogic->getFrame()`.
    fn frame(&self) -> u32;
    /// `getFirstObject()` / `getNextObject()`: every object, in the game logic's list order.
    fn game_logic_objects(&self) -> Vec<ObjectId>;
    /// `TheGameLogic->findObjectByID(id) != NULL`.
    fn game_logic_find_object(&self, id: ObjectId) -> bool;
    /// `TheGameLogic->destroyObject(obj)`.
    fn game_logic_destroy_object(&mut self, o: ObjectId);

    // ---- ThePlayerList ----
    /// `findPlayerWithNameKey(key)`.
    fn player_list_find_with_name_key(&mut self, key: NameKeyType) -> Option<PlayerIndex>;
    /// `getNeutralPlayer()`.
    fn player_list_neutral(&self) -> Option<PlayerIndex>;
    /// `getPlayerCount()`.
    fn player_list_count(&self) -> i32;
    /// `getNthPlayer(i)`.
    fn player_list_nth(&self, i: i32) -> Option<PlayerIndex>;
    /// `ThePlayerList->teamAboutToBeDeleted(team)`.
    fn player_list_team_about_to_be_deleted(&mut self, team: TeamId);

    // ---- Player ----
    /// `player->addTeamToList(proto)`.
    fn player_add_team_to_list(&mut self, p: PlayerIndex, proto: TeamPrototypeId);
    /// `player->removeTeamFromList(proto)`.
    fn player_remove_team_from_list(&mut self, p: PlayerIndex, proto: TeamPrototypeId);
    /// `player->preTeamDestroy(team)`.
    fn player_pre_team_destroy(&mut self, p: PlayerIndex, team: TeamId);
    /// `player->getDefaultTeam()`.
    fn player_default_team(&self, p: PlayerIndex) -> Option<TeamId>;
    /// `player->getRelationship(that)`.
    fn player_relationship(&self, p: PlayerIndex, that: Option<TeamId>) -> Relationship;
    /// `player->getPlayerType()`.
    fn player_type(&self, p: PlayerIndex) -> PlayerType;
    /// `player->getPlayerDifficulty()`.
    fn player_difficulty(&self, p: PlayerIndex) -> GameDifficulty;
    /// `TheThingFactory->findTemplate(player->getPlayerTemplate()->getBeaconTemplate())`.
    fn player_beacon_template(&self, p: PlayerIndex) -> Option<TemplateRef>;

    // ---- the script engine ----
    /// `TheScriptEngine->findScriptByName(name)`.
    fn script_find_by_name(&mut self, name: &[u8]) -> Option<ScriptRef>;
    /// `script->duplicate()`.
    fn script_duplicate(&mut self, s: ScriptRef) -> ScriptRef;
    /// `script->deleteInstance()`.
    fn script_delete(&mut self, s: ScriptRef);
    /// `script->getFrameToEvaluate()`.
    fn script_frame_to_evaluate(&self, s: ScriptRef) -> u32;
    /// `script->setFrameToEvaluate(frame)`.
    fn script_set_frame_to_evaluate(&mut self, s: ScriptRef, frame: u32);
    /// `script->getDelayEvalSeconds()`.
    fn script_delay_eval_seconds(&self, s: ScriptRef) -> i32;
    /// `script->isEasy()`.
    fn script_is_easy(&self, s: ScriptRef) -> bool;
    /// `script->isNormal()`.
    fn script_is_normal(&self, s: ScriptRef) -> bool;
    /// `script->isHard()`.
    fn script_is_hard(&self, s: ScriptRef) -> bool;
    /// `script->isOneShot()`.
    fn script_is_one_shot(&self, s: ScriptRef) -> bool;
    /// `script->getName()`.
    fn script_name(&self, s: ScriptRef) -> Vec<u8>;
    /// `TheScriptEngine->evaluateConditions(script, team, player)`.
    fn script_evaluate_conditions(&mut self, s: ScriptRef, team: Option<TeamId>, player: Option<PlayerIndex>) -> bool;
    /// `TheScriptEngine->friend_executeAction(script->getAction(), team)`.
    fn script_execute_action(&mut self, s: ScriptRef, team: Option<TeamId>);
    /// `TheScriptEngine->runScript(name, team)`.
    fn script_run(&mut self, name: &[u8], team: TeamId);
    /// `TheScriptEngine->notifyOfTeamDestruction(team)`.
    fn script_notify_of_team_destruction(&mut self, team: TeamId);
    /// `TheScriptEngine->AppendDebugMessage(msg, forcePause)`.
    fn script_append_debug_message(&mut self, msg: &[u8], force_pause: bool);

    // ---- the terrain ----
    /// `TheTerrainLogic->getFirstWaypoint()` and `getNext()`: the waypoints, in list order.
    fn terrain_waypoints(&self) -> Vec<WaypointInfo>;
    /// `TheTerrainLogic->getWaypointByID(id) != NULL`: the id back, or `None`.
    fn terrain_waypoint_by_id(&self, id: WaypointId) -> Option<WaypointId>;

    // ---- thing templates ----
    /// `Object::getTemplate()`.
    fn obj_template(&self, o: ObjectId) -> Option<TemplateRef>;
    /// `a->isEquivalentTo(b)` (false when either is `NULL`).
    fn tmpl_is_equivalent_to(&self, a: TemplateRef, b: Option<TemplateRef>) -> bool;
    /// `t->isKindOf(kind)`.
    fn tmpl_is_kind_of(&self, t: TemplateRef, kind: KindOfType) -> bool;
    /// `t->isKindOfMulti(set, clear)`.
    fn tmpl_is_kind_of_multi(&self, t: TemplateRef, set: &KindOfMaskType, clear: &KindOfMaskType) -> bool;
    /// `t->isBuildFacility()`.
    fn tmpl_is_build_facility(&self, t: TemplateRef) -> bool;
    /// `t->getName()`.
    fn tmpl_name(&self, t: TemplateRef) -> Vec<u8>;
    /// `t->getBuildVariations()`.
    fn tmpl_build_variations(&self, t: TemplateRef) -> Vec<Vec<u8>>;

    // ---- objects ----
    /// `obj->isEffectivelyDead()`.
    fn obj_is_effectively_dead(&self, o: ObjectId) -> bool;
    /// `obj->isDestroyed()`.
    fn obj_is_destroyed(&self, o: ObjectId) -> bool;
    /// `obj->isKindOf(kind)`.
    fn obj_is_kind_of(&self, o: ObjectId, kind: KindOfType) -> bool;
    /// `obj->isKindOfMulti(set, clear)`.
    fn obj_is_kind_of_multi(&self, o: ObjectId, set: &KindOfMaskType, clear: &KindOfMaskType) -> bool;
    /// `obj->testStatus(status)` / `obj->getStatusBits().test(status)`.
    fn obj_test_status(&self, o: ObjectId, status: ObjectStatusTypes) -> bool;
    /// `obj->isDisabledByType(DISABLED_HELD)`.
    fn obj_is_disabled_held(&self, o: ObjectId) -> bool;
    /// `obj->getContainedBy() != NULL`.
    fn obj_is_contained(&self, o: ObjectId) -> bool;
    /// `obj->getControllingPlayer()`.
    fn obj_controlling_player(&self, o: ObjectId) -> Option<PlayerIndex>;
    /// `obj->getTeam()`.
    fn obj_team(&self, o: ObjectId) -> Option<TeamId>;
    /// `*obj->getPosition()`.
    fn obj_position(&self, o: ObjectId) -> Coord3D;
    /// `obj->getVisionRange()`.
    fn obj_vision_range(&self, o: ObjectId) -> f32;
    /// `obj->healCompletely()`.
    fn obj_heal_completely(&mut self, o: ObjectId);
    /// `obj->kill()`.
    fn obj_kill(&mut self, o: ObjectId);
    /// `obj->attemptDamage(&info)` with `m_damageType = DAMAGE_UNRESISTABLE`, `m_deathType = DEATH_NORMAL`,
    /// `m_sourceID = INVALID_ID` and `m_amount = amount` (`Team::damageTeamMembers`).
    fn obj_attempt_damage(&mut self, o: ObjectId, amount: f32);
    /// `obj->setTeam(team)`: leaves the old team's member list, joins the new one's (`None` = `NULL`).
    fn obj_set_team(&mut self, o: ObjectId, team: Option<TeamId>);
    /// `obj->handlePartitionCellMaintenance()`.
    fn obj_handle_partition_cell_maintenance(&mut self, o: ObjectId);
    /// `obj->getContain()` and `->getContainCount()`: `None` without a contain module.
    fn obj_contain_count(&self, o: ObjectId) -> Option<i32>;
    /// `obj->getContain()->removeAllContained()`.
    fn obj_remove_all_contained(&mut self, o: ObjectId);
    /// `obj->didEnter(trigger)`.
    fn obj_did_enter(&self, o: ObjectId, t: TriggerRef) -> bool;
    /// `obj->didExit(trigger)`.
    fn obj_did_exit(&self, o: ObjectId, t: TriggerRef) -> bool;
    /// `obj->isInside(trigger)`.
    fn obj_is_inside(&self, o: ObjectId, t: TriggerRef) -> bool;

    // ---- the AI ----
    /// `obj->getAIUpdateInterface() != NULL`.
    fn obj_has_ai(&self, o: ObjectId) -> bool;
    /// `ai->isIdle()`.
    fn ai_is_idle(&self, o: ObjectId) -> bool;
    /// `ai->getLocomotorSet().getValidSurfaces()`.
    fn ai_valid_surfaces(&self, o: ObjectId) -> u32;
    /// `ai->isRecruitable()`.
    fn ai_is_recruitable(&self, o: ObjectId) -> bool;
    /// `pAIGroup->add(obj)`.
    fn ai_group_add(&mut self, group: GroupRef, o: ObjectId);
    /// A team changed its id (`Team::setID`): whatever the host keeps by team id (the members' teams, a player's
    /// default team) follows. Upstream's pointers do not need it; ids do.
    fn team_renamed(&mut self, old: TeamId, new: TeamId);

    // ---- the partition manager ----
    /// `ThePartitionManager->getClosestObject(obj, range, FROM_CENTER_2D, filters) != NULL` for the filters
    /// `PartitionFilterRelationship(obj, ALLOW_ENEMIES)`, `PartitionFilterAlive` and
    /// `PartitionFilterSameMapStatus(obj)`.
    fn partition_closest_enemy(&mut self, o: ObjectId, range: f32) -> bool;
}

type TResult<T> = Result<T, TeamError>;

/// The number of rounds after which a "while the first member is not NULL" loop is reported as endless.
const HANG_LIMIT: u32 = 10_000;

/// C's `(Int)f` for a `Real` (see `experience_tracker.rs`): `0x80000000` for NaN and out of range.
fn real_to_int_trunc(f: f32) -> i32 {
    if f.is_nan() || f64::from(f) < -2_147_483_648.0 || f64::from(f) >= 2_147_483_648.0 {
        return i32::MIN;
    }
    f as i32
}

/// `locoSetMatches(lstm, surfaceBitFlags)`.
pub fn loco_set_matches(lstm: u32, surface_bit_flags: u32) -> bool {
    let s = surface_bit_flags & 0x01 | ((surface_bit_flags & 0x02) << 2);
    (s & lstm) != 0
}

fn team_ref<H: TeamHost>(h: &H, t: TeamId) -> TResult<&Team> {
    h.teams().team(t).ok_or(TeamError::Null)
}

fn proto_of<H: TeamHost>(h: &H, t: TeamId) -> TResult<&TeamPrototype> {
    let team = team_ref(h, t)?;
    h.teams().prototype(team.proto).ok_or(TeamError::Null)
}

fn proto_ref<H: TeamHost>(h: &H, p: TeamPrototypeId) -> TResult<&TeamPrototype> {
    h.teams().prototype(p).ok_or(TeamError::Null)
}

// ======================================================================================================================
// Team
// ======================================================================================================================

/// `Team::Team(proto, id)` for a new instance: stores the team, prepends it to its prototype's instance list, and,
/// when the prototype has scripts for it, keeps the enemy-sighted check; the script engine is told in the debug log.
pub fn create_team_instance<H: TeamHost>(h: &mut H, proto: TeamPrototypeId, id: TeamId) -> TResult<TeamId> {
    h.teams_mut().insert_team(Team::new(id, proto))?;
    let (check, name) = match h.teams().prototype(proto) {
        Some(p) => {
            let i = &p.team_template;
            (!i.script_on_all_clear.is_empty() || !i.script_on_enemy_sighted.is_empty(), Some(p.name.clone()))
        }
        None => (false, None),
    };
    if let Some(mut team_name) = name {
        if check {
            if let Some(t) = h.teams_mut().team_mut(id) {
                t.check_enemy_sighted = true; // Only keep track of enemy sighted if there is a script that cares.
            }
        }
        team_name.extend_from_slice(b" - creating team instance.");
        h.script_append_debug_message(&team_name, false);
    }
    Ok(id)
}

/// `Team::~Team()` and the pool's `deleteInstance`: the script engine and every player are told, each member is
/// taken off the team, the team leaves its prototype's list, and the storage is dropped.
pub fn delete_team_instance<H: TeamHost>(h: &mut H, id: TeamId) -> TResult<()> {
    h.script_notify_of_team_destruction(id);

    // Tell the players a team is going away.
    let count = h.player_list_count();
    for i in 0..count {
        if let Some(plyr) = h.player_list_nth(i) {
            h.player_pre_team_destroy(plyr, id);
        }
    }

    let mut rounds = 0u32;
    while let Some(tm) = h.teams().first_member(id) {
        h.obj_set_team(tm, None);
        rounds += 1;
        if rounds > HANG_LIMIT {
            return Err(TeamError::Hang);
        }
    }

    let proto = team_ref(h, id)?.proto;
    if h.teams().prototype(proto).is_some() && h.teams().is_in_instance_list(proto, id) {
        h.teams_mut().remove_instance(proto, id);
    }
    h.teams_mut().drop_team(id);
    Ok(())
}

/// `Team::getControllingPlayer()`: the prototype's.
pub fn team_get_controlling_player<H: TeamHost>(h: &H, team: TeamId) -> TResult<Option<PlayerIndex>> {
    Ok(proto_of(h, team)?.owning_player)
}

/// `Team::setControllingPlayer(newController)`: the prototype changes hands, and every member relooks.
pub fn team_set_controlling_player<H: TeamHost>(h: &mut H, team: TeamId, new_controller: Option<PlayerIndex>) -> TResult<()> {
    let proto = team_ref(h, team)?.proto;
    // NULL is not allowed, but is caught by TeamPrototype::setControllingPlayer()
    proto_set_controlling_player(h, proto, new_controller)?;

    // Tell all members to redo their looking status, as their Player has changed, but they don't know.
    let mut cur = h.teams().first_member(team);
    while let Some(obj) = cur {
        h.obj_handle_partition_cell_maintenance(obj);
        cur = h.teams().next_member(obj);
    }
    Ok(())
}

/// `Team::setAttackPriorityName(name)`.
pub fn team_set_attack_priority_name<H: TeamHost>(h: &mut H, team: TeamId, name: &[u8]) -> TResult<()> {
    let proto = team_ref(h, team)?.proto;
    if let Some(p) = h.teams_mut().prototype_mut(proto) {
        p.attack_priority_name = name.to_vec();
    }
    Ok(())
}

/// `Team::getTeamAsAIGroup(pAIGroup)`: the members, newest first, added to the group.
pub fn team_get_team_as_ai_group<H: TeamHost>(h: &mut H, team: TeamId, group: Option<GroupRef>) {
    let Some(g) = group else { return };
    let mut cur = h.teams().first_member(team);
    while let Some(o) = cur {
        h.ai_group_add(g, o);
        cur = h.teams().next_member(o);
    }
}

/// `Team::getTargetableCount()`.
pub fn team_get_targetable_count<H: TeamHost>(h: &H, team: TeamId) -> i32 {
    let mut ret_val = 0;
    let mut cur = h.teams().first_member(team);
    while let Some(obj) = cur {
        if !(h.obj_is_effectively_dead(obj) || (!h.obj_has_ai(obj) && !h.obj_is_kind_of(obj, KINDOF_STRUCTURE))) {
            ret_val += 1;
        }
        cur = h.teams().next_member(obj);
    }
    ret_val
}

/// `Team::getRelationship(that)`: this team's override for that team, else for that team's player, else the
/// controlling player's.
pub fn team_get_relationship<H: TeamHost>(h: &H, team: TeamId, that: Option<TeamId>) -> TResult<Relationship> {
    let t = team_ref(h, team)?;
    // do we have an override for that particular team? if so, return it.
    if !t.team_relations.map.is_empty() {
        if let Some(that_id) = that {
            let that_team = team_ref(h, that_id)?;
            if let Some(r) = t.team_relations.map.get(&that_team.id) {
                return Ok(*r);
            }
        }
    }
    // hummm... well, do we have an override for that team's player?
    if !t.player_relations.map.is_empty() {
        if let Some(that_id) = that {
            if let Some(that_player) = team_get_controlling_player(h, that_id)? {
                if let Some(r) = t.player_relations.map.get(&that_player) {
                    return Ok(*r);
                }
            }
        }
    }
    // nope -- go with our Player's view on the matter.
    let me = team_get_controlling_player(h, team)?.ok_or(TeamError::Null)?;
    Ok(h.player_relationship(me, that))
}

/// `Team::setTeamTargetObject(target)`: only computer players, and not on easy, keep a common target.
pub fn team_set_team_target_object<H: TeamHost>(h: &mut H, team: TeamId, target: Option<ObjectId>) -> TResult<()> {
    let Some(target) = target else {
        team_mut(h, team)?.common_attack_target = INVALID_ID;
        return Ok(());
    };
    // Only ai players do common attack.
    let p = team_get_controlling_player(h, team)?.ok_or(TeamError::Null)?;
    if h.player_type(p) == PlayerType::Computer {
        if h.player_difficulty(p) == GameDifficulty::Easy {
            return Ok(()); // we don't do this for easy.  jba.
        }
        team_mut(h, team)?.common_attack_target = target;
    }
    Ok(())
}

fn team_mut<H: TeamHost>(h: &mut H, t: TeamId) -> TResult<&mut Team> {
    h.teams_mut().team_mut(t).ok_or(TeamError::Null)
}

/// `Team::getTeamTargetObject()`: the common target if it is still a fair target, else it is forgotten.
pub fn team_get_team_target_object<H: TeamHost>(h: &mut H, team: TeamId) -> TResult<Option<ObjectId>> {
    let id = team_ref(h, team)?.common_attack_target;
    if id == INVALID_ID {
        return Ok(None);
    }
    let mut target = if h.game_logic_find_object(id) { Some(id) } else { None };
    if let Some(t) = target {
        // If the enemy unit is stealthed and not detected, then we can't attack it!
        if h.obj_test_status(t, OBJECT_STATUS_STEALTHED) && !h.obj_test_status(t, OBJECT_STATUS_DETECTED) && !h.obj_test_status(t, OBJECT_STATUS_DISGUISED) {
            target = None;
        }
    }
    if let Some(t) = target {
        if h.obj_is_effectively_dead(t) {
            target = None;
        }
    }
    if let Some(t) = target {
        if h.obj_is_contained(t) {
            target = None; // target entered a building or vehicle, so stop targeting.
        }
    }
    if let Some(t) = target {
        if h.obj_is_kind_of(t, KINDOF_AIRCRAFT) {
            // It is just generally bad to have an aircraft as the team target.
            target = None;
        }
    }
    if target.is_none() {
        team_mut(h, team)?.common_attack_target = INVALID_ID;
    }
    Ok(target)
}

/// `Team::setOverrideTeamRelationship(teamID, r)`.
pub fn team_set_override_team_relationship<H: TeamHost>(h: &mut H, team: TeamId, team_id: TeamId, r: Relationship) -> TResult<()> {
    if team_id != TEAM_ID_INVALID {
        // note that this creates the entry if it doesn't exist.
        team_mut(h, team)?.team_relations.map.insert(team_id, r);
    }
    Ok(())
}

/// `Team::removeOverrideTeamRelationship(teamID)`: `TEAM_ID_INVALID` removes them all.
pub fn team_remove_override_team_relationship<H: TeamHost>(h: &mut H, team: TeamId, team_id: TeamId) -> TResult<bool> {
    let t = team_mut(h, team)?;
    if !t.team_relations.map.is_empty() {
        if team_id == TEAM_ID_INVALID {
            t.team_relations.map.clear();
            return Ok(true);
        }
        return Ok(t.team_relations.map.remove(&team_id).is_some());
    }
    Ok(false)
}

/// `PLAYER_INDEX_INVALID`.
pub const PLAYER_INDEX_INVALID: PlayerIndex = -1;

/// `Team::setOverridePlayerRelationship(playerIndex, r)`.
pub fn team_set_override_player_relationship<H: TeamHost>(h: &mut H, team: TeamId, player_index: PlayerIndex, r: Relationship) -> TResult<()> {
    if player_index != PLAYER_INDEX_INVALID {
        // note that this creates the entry if it doesn't exist.
        team_mut(h, team)?.player_relations.map.insert(player_index, r);
    }
    Ok(())
}

/// `Team::removeOverridePlayerRelationship(playerIndex)`: `PLAYER_INDEX_INVALID` removes them all.
pub fn team_remove_override_player_relationship<H: TeamHost>(h: &mut H, team: TeamId, player_index: PlayerIndex) -> TResult<bool> {
    let t = team_mut(h, team)?;
    if !t.player_relations.map.is_empty() {
        if player_index == PLAYER_INDEX_INVALID {
            t.player_relations.map.clear();
            return Ok(true);
        }
        return Ok(t.player_relations.map.remove(&player_index).is_some());
    }
    Ok(false)
}

/// `Team::countObjectsByThingTemplate(...)`: one pass over the members; each member counts for the first
/// template it is equivalent to that it is not excluded for.
pub fn team_count_objects_by_thing_template<H: TeamHost>(
    h: &H,
    team: TeamId,
    things: &[Option<TemplateRef>],
    ignore_dead: bool,
    counts: &mut [i32],
    ignore_under_construction: bool,
) {
    let mut cur = h.teams().first_member(team);
    while let Some(o) = cur {
        let objtmpl = h.obj_template(o);
        for (i, thing) in things.iter().enumerate() {
            // Kris: Compare
            let equivalent = match objtmpl {
                Some(t) => h.tmpl_is_equivalent_to(t, *thing),
                None => false,
            };
            if !equivalent {
                continue;
            }
            if ignore_dead && h.obj_is_effectively_dead(o) {
                continue;
            }
            if ignore_under_construction && h.obj_test_status(o, OBJECT_STATUS_UNDER_CONSTRUCTION) {
                continue;
            }
            counts[i] += 1;
            break; // from 'next i', NOT 'next object'
        }
        cur = h.teams().next_member(o);
    }
}

/// `Team::countBuildings()`.
pub fn team_count_buildings<H: TeamHost>(h: &H, team: TeamId) -> i32 {
    let mut ret_val = 0;
    let mut cur = h.teams().first_member(team);
    while let Some(o) = cur {
        if let Some(t) = h.obj_template(o) {
            if h.tmpl_is_kind_of(t, KINDOF_STRUCTURE) {
                ret_val += 1;
            }
        }
        cur = h.teams().next_member(o);
    }
    ret_val
}

/// `Team::countObjects(setMask, clearMask)`.
pub fn team_count_objects<H: TeamHost>(h: &H, team: TeamId, set_mask: &KindOfMaskType, clear_mask: &KindOfMaskType) -> i32 {
    let mut ret_val = 0;
    let mut cur = h.teams().first_member(team);
    while let Some(o) = cur {
        if let Some(t) = h.obj_template(o) {
            if h.tmpl_is_kind_of_multi(t, set_mask, clear_mask) {
                ret_val += 1;
            }
        }
        cur = h.teams().next_member(o);
    }
    ret_val
}

/// `Team::healAllObjects()`.
pub fn team_heal_all_objects<H: TeamHost>(h: &mut H, team: TeamId) {
    let mut cur = h.teams().first_member(team);
    while let Some(o) = cur {
        h.obj_heal_completely(o);
        cur = h.teams().next_member(o);
    }
}

/// `Team::iterateObjects(func, userData)`.
pub fn team_iterate_objects<H: TeamHost>(h: &mut H, team: TeamId, mut func: impl FnMut(&mut H, ObjectId)) {
    let mut cur = h.teams().first_member(team);
    while let Some(o) = cur {
        func(h, o);
        cur = h.teams().next_member(o);
    }
}

/// `Team::hasAnyBuildings()`.
pub fn team_has_any_buildings<H: TeamHost>(h: &H, team: TeamId) -> bool {
    let mut cur = h.teams().first_member(team);
    while let Some(o) = cur {
        if !h.obj_is_effectively_dead(o) && !h.obj_is_destroyed(o) && h.obj_is_kind_of(o, KINDOF_STRUCTURE) {
            return true;
        }
        cur = h.teams().next_member(o);
    }
    false
}

/// `Team::hasAnyBuildings(kindOf)`: the mask is the caller's copy, and gets `KINDOF_STRUCTURE` set.
pub fn team_has_any_buildings_kind_of<H: TeamHost>(h: &H, team: TeamId, mut kind_of: KindOfMaskType) -> bool {
    let mut cur = h.teams().first_member(team);
    while let Some(o) = cur {
        if !h.obj_is_effectively_dead(o) && !h.obj_is_destroyed(o) {
            kind_of.set(KINDOF_STRUCTURE, 1);
            if h.obj_is_kind_of_multi(o, &kind_of, &KINDOFMASK_NONE) {
                return true;
            }
        }
        cur = h.teams().next_member(o);
    }
    false
}

/// `Team::hasAnyUnits()`: alive, not a structure, projectile or mine.
pub fn team_has_any_units<H: TeamHost>(h: &H, team: TeamId) -> bool {
    let mut cur = h.teams().first_member(team);
    while let Some(o) = cur {
        if !h.obj_is_effectively_dead(o)
            && !h.obj_is_destroyed(o)
            && !h.obj_is_kind_of(o, KINDOF_STRUCTURE)
            && !h.obj_is_kind_of(o, KINDOF_PROJECTILE)
            && !h.obj_is_kind_of(o, KINDOF_MINE)
        {
            return true;
        }
        cur = h.teams().next_member(o);
    }
    false
}

/// `Team::isIdle()`.
pub fn team_is_idle<H: TeamHost>(h: &H, team: TeamId) -> bool {
    let mut idle = true; // assume idle.
    let mut cur = h.teams().first_member(team);
    while let Some(o) = cur {
        if h.obj_has_ai(o) && !h.obj_is_effectively_dead(o) && !h.ai_is_idle(o) {
            idle = false;
            break;
        }
        cur = h.teams().next_member(o);
    }
    idle
}

/// `Team::hasAnyObjects()`: alive, and not a projectile, an inert object (radiation fields) or a mine.
pub fn team_has_any_objects<H: TeamHost>(h: &H, team: TeamId) -> bool {
    let mut cur = h.teams().first_member(team);
    while let Some(o) = cur {
        if !h.obj_is_effectively_dead(o)
            && !h.obj_is_destroyed(o)
            && !h.obj_is_kind_of(o, KINDOF_PROJECTILE)
            && !h.obj_is_kind_of(o, KINDOF_INERT)
            && !h.obj_is_kind_of(o, KINDOF_MINE)
        {
            return true;
        }
        cur = h.teams().next_member(o);
    }
    false
}

/// `Team::updateState()`: clears `m_enteredOrExited`; an active team runs its create script and sets up the
/// destroyed threshold once, checks for an enemy in sight, for the destroyed threshold and for idleness.
pub fn team_update_state<H: TeamHost>(h: &mut H, team: TeamId) -> TResult<()> {
    team_mut(h, team)?.entered_or_exited = false;
    if !team_ref(h, team)?.active {
        return Ok(());
    }
    let proto = team_ref(h, team)?.proto;
    let info = proto_ref(h, proto)?.team_template.clone();
    if team_ref(h, team)?.created {
        team_mut(h, team)?.created = false;
        // Run the on create script, if any.
        if !info.script_on_create.is_empty() {
            h.script_run(&info.script_on_create, team);
        }

        // Set up info for the onDestroyed script, if needed.
        if !info.script_on_destroyed.is_empty() {
            let mut cur = h.teams().first_member(team);
            while let Some(o) = cur {
                team_mut(h, team)?.cur_units += 1;
                cur = h.teams().next_member(o);
            }
            let t = team_mut(h, team)?;
            // m_curUnits - (m_curUnits * pInfo->m_destroyedThreshold): an Int and a Real, in Real
            t.destroy_threshold = real_to_int_trunc(t.cur_units as f32 - (t.cur_units as f32 * info.destroyed_threshold));
            if t.destroy_threshold > t.cur_units - 1 {
                t.destroy_threshold = t.cur_units - 1;
            }
            if t.destroy_threshold < 0 {
                t.destroy_threshold = 0;
            }
        }
    }

    // Do enemy sighted/on clear checks.
    if team_ref(h, team)?.check_enemy_sighted {
        {
            let t = team_mut(h, team)?;
            t.prev_see_enemy = t.see_enemy;
            t.see_enemy = false;
        }
        let mut any_alive_in_team = false; // If we're all dead, don't do all clear.
        // only consider enemies.
        let mut cur = h.teams().first_member(team);
        while let Some(o) = cur {
            if !h.obj_is_effectively_dead(o) {
                let vision_range = h.obj_vision_range(o);
                any_alive_in_team = true;
                if h.partition_closest_enemy(o, vision_range) {
                    team_mut(h, team)?.see_enemy = true;
                    break;
                }
            }
            cur = h.teams().next_member(o);
        }
        if any_alive_in_team {
            let t = team_ref(h, team)?;
            let (prev, now) = (t.prev_see_enemy, t.see_enemy);
            if prev != now {
                if now {
                    // fire onEnemySighted
                    h.script_run(&info.script_on_enemy_sighted, team);
                } else {
                    // fire on all clear.
                    h.script_run(&info.script_on_all_clear, team);
                }
            }
        }
    }

    // Do onDestroyed checks.
    if !info.script_on_destroyed.is_empty() {
        let prev_units = team_ref(h, team)?.cur_units;
        team_mut(h, team)?.cur_units = 0;
        let mut cur = h.teams().first_member(team);
        while let Some(o) = cur {
            if !h.obj_is_effectively_dead(o) {
                team_mut(h, team)?.cur_units += 1;
            }
            cur = h.teams().next_member(o);
        }
        let t = team_ref(h, team)?;
        if t.cur_units != prev_units && t.cur_units <= t.destroy_threshold {
            h.script_run(&info.script_on_destroyed, team);
            team_mut(h, team)?.destroy_threshold = -1; // don't trigger again.
        }
    }

    // Do onIdle checks.
    if !info.script_on_idle.is_empty() {
        let mut is_idle = true;
        let mut any_alive_in_team = false; // If we're all dead, don't do all clear.
        let mut cur = h.teams().first_member(team);
        while let Some(o) = cur {
            if !h.obj_is_effectively_dead(o) && h.obj_has_ai(o) {
                any_alive_in_team = true;
                if !h.ai_is_idle(o) {
                    is_idle = false;
                }
            }
            cur = h.teams().next_member(o);
        }
        if any_alive_in_team && is_idle && team_ref(h, team)?.was_idle {
            h.script_run(&info.script_on_idle, team);
        }
        team_mut(h, team)?.was_idle = is_idle;
    }
    Ok(())
}

/// `Team::notifyTeamOfObjectDeath()`.
pub fn team_notify_team_of_object_death<H: TeamHost>(h: &mut H, team: TeamId) -> TResult<()> {
    let name = proto_of(h, team)?.team_template.script_on_unit_destroyed.clone();
    if name.is_empty() {
        return Ok(());
    }
    h.script_run(&name, team);
    Ok(())
}

/// Which of the six trigger questions a member loop asks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TriggerQuery {
    DidAllEnter,
    DidPartialEnter,
    DidPartialExit,
    DidAllExit,
    AllInside,
    NoneInside,
    SomeInsideSomeOutside,
}

/// The test every trigger query starts each member with: the locomotor surfaces the member is judged by must match
/// `whichToConsider` (members without an AI are ground units).
fn considers<H: TeamHost>(h: &H, o: ObjectId, which_to_consider: u32) -> bool {
    if h.obj_has_ai(o) {
        loco_set_matches(h.ai_valid_surfaces(o), which_to_consider)
    } else {
        // things without ai should consider themselves ground units
        loco_set_matches(LOCOMOTORSURFACE_GROUND as u32, which_to_consider)
    }
}

fn trigger_query<H: TeamHost>(h: &H, team: TeamId, trigger: TriggerRef, which: u32, q: TriggerQuery) -> TResult<bool> {
    let t = team_ref(h, team)?;
    // If any units entered or exited, they set this flag.
    if matches!(q, TriggerQuery::DidAllEnter | TriggerQuery::DidPartialEnter | TriggerQuery::DidPartialExit | TriggerQuery::DidAllExit) && !t.entered_or_exited {
        return Ok(false);
    }
    // empty teams are not inside.
    if q == TriggerQuery::AllInside && !team_has_any_objects(h, team) {
        return Ok(false);
    }
    let (mut any_considered, mut entered, mut outside, mut exited, mut inside, mut any_outside, mut any_inside) = (false, false, false, false, false, false, false);
    let mut cur = h.teams().first_member(team);
    while let Some(o) = cur {
        'member: {
            if !considers(h, o, which) {
                break 'member;
            }
            if h.obj_is_effectively_dead(o) {
                break 'member;
            }
            if h.obj_is_kind_of(o, KINDOF_INERT) {
                break 'member;
            }
            match q {
                TriggerQuery::DidAllEnter => {
                    if h.obj_did_enter(o, trigger) {
                        entered = true;
                    } else if !h.obj_is_inside(o, trigger) {
                        outside = true;
                    }
                    // We need this in order to prevent this from returning a false positive
                    any_considered = true;
                }
                TriggerQuery::DidPartialEnter => {
                    if h.obj_did_enter(o, trigger) {
                        return Ok(true);
                    }
                }
                TriggerQuery::DidPartialExit => {
                    if h.obj_did_exit(o, trigger) {
                        return Ok(true);
                    }
                }
                TriggerQuery::DidAllExit => {
                    if h.obj_did_exit(o, trigger) {
                        exited = true;
                    } else if h.obj_is_inside(o, trigger) {
                        inside = true;
                    }
                    any_considered = true;
                }
                TriggerQuery::AllInside => {
                    if !h.obj_is_inside(o, trigger) {
                        any_outside = true;
                    }
                    any_considered = true;
                }
                TriggerQuery::NoneInside => {
                    if h.obj_is_inside(o, trigger) {
                        any_inside = true;
                    }
                    any_considered = true;
                }
                TriggerQuery::SomeInsideSomeOutside => {
                    if h.obj_is_inside(o, trigger) {
                        any_inside = true;
                    } else {
                        any_outside = true;
                    }
                    any_considered = true;
                }
            }
        }
        cur = h.teams().next_member(o);
    }
    Ok(match q {
        TriggerQuery::DidAllEnter => entered && !outside,
        TriggerQuery::DidPartialEnter | TriggerQuery::DidPartialExit => false,
        TriggerQuery::DidAllExit => any_considered && exited && !inside,
        TriggerQuery::AllInside => any_considered && !any_outside,
        TriggerQuery::NoneInside => any_considered && !any_inside,
        TriggerQuery::SomeInsideSomeOutside => any_considered && any_inside && any_outside,
    })
}

/// `Team::didAllEnter(pTrigger, whichToConsider)`: all members entered the area.
pub fn team_did_all_enter<H: TeamHost>(h: &H, team: TeamId, trigger: TriggerRef, which: u32) -> TResult<bool> {
    trigger_query(h, team, trigger, which, TriggerQuery::DidAllEnter)
}

/// `Team::didPartialEnter(...)`: one member entered the area.
pub fn team_did_partial_enter<H: TeamHost>(h: &H, team: TeamId, trigger: TriggerRef, which: u32) -> TResult<bool> {
    trigger_query(h, team, trigger, which, TriggerQuery::DidPartialEnter)
}

/// `Team::didPartialExit(...)`: one member exited the area.
pub fn team_did_partial_exit<H: TeamHost>(h: &H, team: TeamId, trigger: TriggerRef, which: u32) -> TResult<bool> {
    trigger_query(h, team, trigger, which, TriggerQuery::DidPartialExit)
}

/// `Team::didAllExit(...)`: all members exited the area.
pub fn team_did_all_exit<H: TeamHost>(h: &H, team: TeamId, trigger: TriggerRef, which: u32) -> TResult<bool> {
    trigger_query(h, team, trigger, which, TriggerQuery::DidAllExit)
}

/// `Team::allInside(...)`: all members are inside the area.
pub fn team_all_inside<H: TeamHost>(h: &H, team: TeamId, trigger: TriggerRef, which: u32) -> TResult<bool> {
    trigger_query(h, team, trigger, which, TriggerQuery::AllInside)
}

/// `Team::noneInside(...)`: no member is in the area.
pub fn team_none_inside<H: TeamHost>(h: &H, team: TeamId, trigger: TriggerRef, which: u32) -> TResult<bool> {
    trigger_query(h, team, trigger, which, TriggerQuery::NoneInside)
}

/// `Team::someInsideSomeOutside(...)`: one or more in, one or more out.
pub fn team_some_inside_some_outside<H: TeamHost>(h: &H, team: TeamId, trigger: TriggerRef, which: u32) -> TResult<bool> {
    trigger_query(h, team, trigger, which, TriggerQuery::SomeInsideSomeOutside)
}

/// `Team::unitsEntered(...)` is declared in `Team.h` and defined nowhere.
/// `Team::getEstimateTeamPosition()`: the position of the first member.
pub fn team_get_estimate_team_position<H: TeamHost>(h: &H, team: TeamId) -> Option<Coord3D> {
    h.teams().first_member(team).map(|o| h.obj_position(o))
}

/// `Team::deleteTeam(ignoreDead)`: the default team first evacuates its containers (taking a list of them
/// before it empties any, because emptying one changes team lists); then every member is destroyed.
pub fn team_delete_team<H: TeamHost>(h: &mut H, team: TeamId, ignore_dead: bool) -> TResult<()> {
    let me = team_get_controlling_player(h, team)?.ok_or(TeamError::Null)?;
    if Some(team) == h.player_default_team(me) {
        let mut guys_to_make_evacuate = Vec::new();
        let mut cur = h.teams().first_member(team);
        while let Some(obj) = cur {
            if let Some(n) = h.obj_contain_count(obj) {
                if n > 0 {
                    // Write them all down, so the DLINK track jumping doesn't screw me up here as well.
                    guys_to_make_evacuate.push(obj);
                }
            }
            cur = h.teams().next_member(obj);
        }
        for obj in guys_to_make_evacuate {
            if h.obj_contain_count(obj).is_some() {
                h.obj_remove_all_contained(obj);
            }
        }
    }

    // this doesn't actually delete the team, it deletes the members of the team.
    let mut cur = h.teams().first_member(team);
    while let Some(obj) = cur {
        if !(ignore_dead && h.obj_is_effectively_dead(obj)) {
            h.game_logic_destroy_object(obj);
        }
        cur = h.teams().next_member(obj);
    }
    Ok(())
}

/// `Team::transferUnitsTo(newTeam)`: every member moves to the new team.
pub fn team_transfer_units_to<H: TeamHost>(h: &mut H, team: TeamId, new_team: Option<TeamId>) -> TResult<()> {
    let Some(new_team) = new_team else { return Ok(()) };
    if team == new_team {
        return Ok(());
    }
    let mut rounds = 0u32;
    while let Some(obj) = h.teams().first_member(team) {
        h.obj_set_team(obj, Some(new_team));
        rounds += 1;
        if rounds > HANG_LIMIT {
            return Err(TeamError::Hang);
        }
    }
    Ok(())
}

/// `isInBuildVariations(ttWithVariations, b)`.
fn is_in_build_variations<H: TeamHost>(h: &H, tt_with_variations: TemplateRef, b: Option<TemplateRef>) -> TResult<bool> {
    let bv = h.tmpl_build_variations(tt_with_variations);
    if bv.is_empty() {
        return Ok(false);
    }
    let b = b.ok_or(TeamError::Null)?;
    let name = h.tmpl_name(b);
    Ok(bv.contains(&name))
}

/// `Team::tryToRecruit(tTemplate, teamHome, maxDist)`: the closest suitable unit of this player's other teams.
pub fn team_try_to_recruit<H: TeamHost>(h: &H, team: TeamId, t_template: TemplateRef, team_home: Coord3D, max_dist: f32) -> TResult<Option<ObjectId>> {
    let my_player = team_get_controlling_player(h, team)?.ok_or(TeamError::Null)?;
    let mut dist_sqr = max_dist * max_dist;
    let mut recruit: Option<ObjectId> = None;
    let my_priority = proto_of(h, team)?.team_template.production_priority;
    for obj in h.game_logic_objects() {
        let objtmpl = h.obj_template(obj);
        let equivalent = match objtmpl {
            Some(t) => h.tmpl_is_equivalent_to(t, Some(t_template)),
            None => false,
        };
        if !equivalent {
            // it might be ok, if tTemplate is really just a "build-variations" template...
            if !is_in_build_variations(h, t_template, objtmpl)? {
                continue;
            }
        }
        if h.obj_controlling_player(obj) != Some(my_player) {
            continue;
        }
        let their_team = h.obj_team(obj).ok_or(TeamError::Null)?;
        let is_default_team = Some(their_team) == h.player_default_team(my_player);
        if !team_ref(h, their_team)?.active {
            continue; // Team is building, so don't steal it's members yet.
        }
        let their = team_ref(h, their_team)?;
        let their_info = &proto_of(h, their_team)?.team_template;
        if their_info.production_priority >= my_priority {
            continue;
        }
        let mut team_is_recruitable = is_default_team; // Default team always recruitable.
        if their_info.is_ai_recruitable {
            team_is_recruitable = true;
        }
        // Check & see if individual team has been marked for recruitability.
        if their.is_recruitability_set {
            team_is_recruitable = their.is_recruitable;
        }
        if !team_is_recruitable {
            continue;
        }
        if h.obj_has_ai(obj) && !h.ai_is_recruitable(obj) {
            continue; // can't recruit this unit.
        }
        if h.obj_is_disabled_held(obj) {
            continue; // Don't recruit held units.
        }
        let pos = h.obj_position(obj);
        let dx = team_home.x - pos.x;
        let dy = team_home.y - pos.y;

        if is_default_team && recruit.is_none() {
            recruit = Some(obj);
            dist_sqr = dx * dx + dy * dy;
        }

        if dx * dx + dy * dy > dist_sqr {
            continue;
        }
        dist_sqr = dx * dx + dy * dy;
        recruit = Some(obj);
    }
    Ok(recruit)
}

/// `Team::evacuateTeam()`: every container with something in it empties.
pub fn team_evacuate_team<H: TeamHost>(h: &mut H, team: TeamId) {
    let mut objects_to_process = Vec::new();
    let mut cur = h.teams().first_member(team);
    while let Some(obj) = cur {
        if !(h.obj_is_destroyed(obj) || h.obj_is_effectively_dead(obj)) {
            let num_contained = h.obj_contain_count(obj).map_or(0u32, |n| n as u32);
            if num_contained > 0 {
                objects_to_process.push(obj);
            }
        }
        cur = h.teams().next_member(obj);
    }
    // evacuate any containers
    for obj in objects_to_process {
        if h.obj_contain_count(obj).is_some() {
            h.obj_remove_all_contained(obj);
        }
    }
}

/// `Team::killTeam()`: evacuate, then kill every member that is still on this team (tech buildings go to the
/// neutral player's default team instead).
pub fn team_kill_team<H: TeamHost>(h: &mut H, team: TeamId) -> TResult<()> {
    team_evacuate_team(h, team);

    // beacons are effectively dead, so we need to destroy via a non-kill() method
    let me = team_get_controlling_player(h, team)?.ok_or(TeamError::Null)?;
    let beacon_template = h.player_beacon_template(me);

    // now find objects to kill
    let mut objects_to_process = Vec::new();
    let mut cur = h.teams().first_member(team);
    while let Some(obj) = cur {
        let skip = h.obj_is_destroyed(obj) || (h.obj_is_effectively_dead(obj) && {
            let t = h.obj_template(obj).ok_or(TeamError::Null)?;
            !h.tmpl_is_equivalent_to(t, beacon_template)
        });
        if !skip {
            // the object's team could change after having all the passengers get out.
            if h.obj_team(obj) == Some(team) {
                objects_to_process.push(obj);
            }
        }
        cur = h.teams().next_member(obj);
    }

    // and finally, kill things
    for obj in objects_to_process {
        if h.obj_is_kind_of(obj, KINDOF_TECH_BUILDING) {
            let neutral = h.player_list_neutral().ok_or(TeamError::Null)?;
            let t = h.player_default_team(neutral).ok_or(TeamError::Null)?;
            h.obj_set_team(obj, Some(t));
        } else {
            h.obj_kill(obj);
        }
    }
    Ok(())
}

/// `Team::damageTeamMembers(amount)`: a negative amount kills; always returns `false`.
pub fn team_damage_team_members<H: TeamHost>(h: &mut H, team: TeamId, amount: f32) -> bool {
    let mut cur = h.teams().first_member(team);
    while let Some(o) = cur {
        if !h.obj_is_effectively_dead(o) && !h.obj_is_destroyed(o) {
            // do max amount of damage to object
            if amount < 0.0 {
                h.obj_kill(o);
            } else {
                h.obj_attempt_damage(o, amount);
            }
        }
        cur = h.teams().next_member(o);
    }
    false
}

/// `Team::moveTeamTo(destination)`: tests each member for being dead and does nothing else (upstream's `@todo`).
pub fn team_move_team_to<H: TeamHost>(h: &H, team: TeamId, _destination: Coord3D) {
    let mut cur = h.teams().first_member(team);
    while let Some(o) = cur {
        let _ = h.obj_is_effectively_dead(o) || h.obj_is_destroyed(o);
        cur = h.teams().next_member(o);
    }
}

/// `Team::hasAnyBuildFacility()`.
pub fn team_has_any_build_facility<H: TeamHost>(h: &H, team: TeamId) -> TResult<bool> {
    let mut cur = h.teams().first_member(team);
    while let Some(o) = cur {
        let t = h.obj_template(o).ok_or(TeamError::Null)?;
        if h.tmpl_is_build_facility(t) {
            return Ok(true);
        }
        cur = h.teams().next_member(o);
    }
    Ok(false)
}

/// `Team::updateGenericScripts()`: every generic script still to be attempted is evaluated, and runs when its
/// conditions hold; a one-shot never runs twice, and a missing script is never looked for again.
pub fn team_update_generic_scripts<H: TeamHost>(h: &mut H, team: TeamId) -> TResult<()> {
    for i in 0..MAX_GENERIC_SCRIPTS {
        if team_ref(h, team)?.should_attempt_generic_script[i] {
            // Does the condition succeed? If so, run it. If it is a run once script, also mark that we
            // shouldn't run it again.
            let proto = team_ref(h, team)?.proto;
            let script = proto_get_generic_script(h, proto, i as i32)?;
            if let Some(s) = script {
                if h.script_evaluate_conditions(s, Some(team), None) {
                    // It was successful.
                    if h.script_is_one_shot(s) {
                        team_mut(h, team)?.should_attempt_generic_script[i] = false;
                    }
                    h.script_execute_action(s, Some(team));
                    let mut msg = b"Generic script '".to_vec();
                    msg.extend_from_slice(&h.script_name(s));
                    msg.extend_from_slice(b"' run on team ");
                    msg.extend_from_slice(&proto_of(h, team)?.name);
                    h.script_append_debug_message(&msg, false);
                }
            } else {
                team_mut(h, team)?.should_attempt_generic_script[i] = false;
            }
        }
    }
    Ok(())
}

/// `Team::setID(id)`: the team keeps its place in its prototype's list under the new id, and the host is told.
pub fn set_team_id<H: TeamHost>(h: &mut H, team: TeamId, id: TeamId) -> TResult<()> {
    h.teams_mut().set_team_id(team, id)?;
    h.team_renamed(team, id);
    Ok(())
}

// ======================================================================================================================
// TeamPrototype
// ======================================================================================================================

/// `TeamPrototype::TeamPrototype(tf, name, ownerPlayer, isSingleton, d, id)`: the template info is read from the
/// dictionary (member-initialiser order: before the body), then the prototype enters the factory's map under its
/// name key and the owner's list.
pub fn create_team_prototype<H: TeamHost>(h: &mut H, name: &[u8], owner: Option<PlayerIndex>, is_singleton: bool, d: &Dict, id: TeamPrototypeId) -> TResult<TeamPrototypeId> {
    let waypoints = h.terrain_waypoints();
    let template = {
        let (gen, keys) = h.names();
        TeamTemplateInfo::from_dict(d, gen, keys, &waypoints)
    };
    let p = TeamPrototype {
        id,
        owning_player: owner,
        name: name.to_vec(),
        flags: if is_singleton { TEAM_SINGLETON } else { 0 },
        production_condition_always_false: false,
        production_condition_script: None,
        retrieved_generic_scripts: false,
        generic_scripts_to_run: [None; MAX_GENERIC_SCRIPTS],
        team_template: template,
        attack_priority_name: Vec::new(),
        instance_head: None,
    };
    h.teams_mut().insert_prototype(p)?;
    // m_factory->addTeamPrototypeToList(this)
    factory_add_team_prototype_to_list(h, id);
    if let Some(o) = owner {
        h.player_add_team_to_list(o, id);
    }
    Ok(id)
}

/// `TeamPrototype::~TeamPrototype()` and `deleteInstance`: every team instance goes (the factory and the player
/// list are told first), the owner and the factory forget the prototype, and the scripts it owns are deleted.
pub fn delete_team_prototype<H: TeamHost>(h: &mut H, id: TeamPrototypeId) -> TResult<()> {
    // removeAll_TeamInstanceList(deleteTeamCallback)
    while let Some(t) = h.teams().first_instance(id) {
        h.teams_mut().remove_instance(id, t);
        // deleteTeamCallback: TheTeamFactory->teamAboutToBeDeleted(o); o->deleteInstance();
        factory_team_about_to_be_deleted(h, Some(t))?;
        delete_team_instance(h, t)?;
    }
    let owner = proto_ref(h, id)?.owning_player;
    if let Some(o) = owner {
        h.player_remove_team_from_list(o, id);
    }
    factory_remove_team_prototype_from_list(h, id);

    let (cond, generics) = {
        let p = proto_ref(h, id)?;
        (p.production_condition_script, p.generic_scripts_to_run)
    };
    if let Some(s) = cond {
        h.script_delete(s);
    }
    for s in generics.into_iter().flatten() {
        h.script_delete(s);
    }
    h.teams_mut().drop_prototype(id);
    Ok(())
}

/// `TeamPrototype::getControllingPlayer()`.
pub fn proto_get_controlling_player<H: TeamHost>(h: &H, proto: TeamPrototypeId) -> TResult<Option<PlayerIndex>> {
    Ok(proto_ref(h, proto)?.owning_player)
}

/// `TeamPrototype::findTeamByID(teamID)`: the instance in this prototype's list with that id.
pub fn proto_find_team_by_id<H: TeamHost>(h: &H, proto: TeamPrototypeId, team_id: TeamId) -> Option<TeamId> {
    let mut cur = h.teams().first_instance(proto);
    while let Some(t) = cur {
        if t == team_id {
            return Some(t);
        }
        cur = h.teams().next_instance(t);
    }
    None
}

/// `TeamPrototype::setControllingPlayer(newController)`: `NULL` is ignored.
pub fn proto_set_controlling_player<H: TeamHost>(h: &mut H, proto: TeamPrototypeId, new_controller: Option<PlayerIndex>) -> TResult<()> {
    let Some(new_controller) = new_controller else { return Ok(()) };
    let old = proto_ref(h, proto)?.owning_player;
    if let Some(o) = old {
        h.player_remove_team_from_list(o, proto);
    }
    h.teams_mut().prototype_mut(proto).ok_or(TeamError::Null)?.owning_player = Some(new_controller);
    // impossible to get here with a NULL pointer.
    h.player_add_team_to_list(new_controller, proto);
    Ok(())
}

/// The instances of a prototype, newest first, read as a `DLINK_ITERATOR` reads them (`next` after each body):
/// the callers below walk with this and re-read `next` after their body.
fn first_instance<H: TeamHost>(h: &H, proto: TeamPrototypeId) -> Option<TeamId> {
    h.teams().first_instance(proto)
}

/// `TeamPrototype::countObjectsByThingTemplate(...)`.
pub fn proto_count_objects_by_thing_template<H: TeamHost>(
    h: &H,
    proto: TeamPrototypeId,
    things: &[Option<TemplateRef>],
    ignore_dead: bool,
    counts: &mut [i32],
    ignore_under_construction: bool,
) {
    let mut cur = first_instance(h, proto);
    while let Some(t) = cur {
        team_count_objects_by_thing_template(h, t, things, ignore_dead, counts, ignore_under_construction);
        cur = h.teams().next_instance(t);
    }
}

/// `TeamPrototype::teamAboutToBeDeleted(team)`: no instance keeps an override for the dying team.
pub fn proto_team_about_to_be_deleted<H: TeamHost>(h: &mut H, proto: TeamPrototypeId, team: Option<TeamId>) -> TResult<()> {
    let mut cur = first_instance(h, proto);
    while let Some(t) = cur {
        team_remove_override_team_relationship(h, t, team.unwrap_or(TEAM_ID_INVALID))?;
        cur = h.teams().next_instance(t);
    }
    Ok(())
}

/// `TeamPrototype::getGenericScript(scriptToRetrieve)`: the first call duplicates every named script.
pub fn proto_get_generic_script<H: TeamHost>(h: &mut H, proto: TeamPrototypeId, script_to_retrieve: i32) -> TResult<Option<ScriptRef>> {
    if !proto_ref(h, proto)?.retrieved_generic_scripts {
        // set this to true so we won't do the lookup again.
        h.teams_mut().prototype_mut(proto).ok_or(TeamError::Null)?.retrieved_generic_scripts = true;
        // Go get them from the script engine, and duplicate each one.
        for i in 0..MAX_GENERIC_SCRIPTS {
            let mut script_to_save = None;
            let name = proto_ref(h, proto)?.team_template.team_generic_scripts[i].clone();
            if !name.is_empty() {
                if let Some(tmp) = h.script_find_by_name(&name) {
                    script_to_save = Some(h.script_duplicate(tmp));
                }
            }
            // I now own this one. I'm responsible for cleaning it up on destruction.
            h.teams_mut().prototype_mut(proto).ok_or(TeamError::Null)?.generic_scripts_to_run[i] = script_to_save;
        }
    }
    let i = usize::try_from(script_to_retrieve).map_err(|_| TeamError::Null)?;
    Ok(*proto_ref(h, proto)?.generic_scripts_to_run.get(i).ok_or(TeamError::Null)?)
}

/// `TeamPrototype::increaseAIPriorityForSuccess()`.
pub fn proto_increase_ai_priority_for_success<H: TeamHost>(h: &mut H, proto: TeamPrototypeId) -> TResult<()> {
    let p = h.teams_mut().prototype_mut(proto).ok_or(TeamError::Null)?;
    p.team_template.production_priority = p.team_template.production_priority.wrapping_add(p.team_template.production_priority_success_increase);
    Ok(())
}

/// `TeamPrototype::decreaseAIPriorityForFailure()`.
pub fn proto_decrease_ai_priority_for_failure<H: TeamHost>(h: &mut H, proto: TeamPrototypeId) -> TResult<()> {
    let p = h.teams_mut().prototype_mut(proto).ok_or(TeamError::Null)?;
    p.team_template.production_priority = p.team_template.production_priority.wrapping_sub(p.team_template.production_priority_failure_decrease);
    Ok(())
}

/// `TeamPrototype::countBuildings()`.
pub fn proto_count_buildings<H: TeamHost>(h: &H, proto: TeamPrototypeId) -> i32 {
    let mut ret_val = 0;
    let mut cur = first_instance(h, proto);
    while let Some(t) = cur {
        ret_val += team_count_buildings(h, t);
        cur = h.teams().next_instance(t);
    }
    ret_val
}

/// `TeamPrototype::countObjects(setMask, clearMask)`.
pub fn proto_count_objects<H: TeamHost>(h: &H, proto: TeamPrototypeId, set_mask: &KindOfMaskType, clear_mask: &KindOfMaskType) -> i32 {
    let mut ret_val = 0;
    let mut cur = first_instance(h, proto);
    while let Some(t) = cur {
        ret_val += team_count_objects(h, t, set_mask, clear_mask);
        cur = h.teams().next_instance(t);
    }
    ret_val
}

/// `TeamPrototype::healAllObjects()`.
pub fn proto_heal_all_objects<H: TeamHost>(h: &mut H, proto: TeamPrototypeId) {
    let mut cur = first_instance(h, proto);
    while let Some(t) = cur {
        team_heal_all_objects(h, t);
        cur = h.teams().next_instance(t);
    }
}

/// `TeamPrototype::iterateObjects(func, userData)`.
pub fn proto_iterate_objects<H: TeamHost>(h: &mut H, proto: TeamPrototypeId, mut func: impl FnMut(&mut H, ObjectId)) {
    let mut cur = first_instance(h, proto);
    while let Some(t) = cur {
        team_iterate_objects(h, t, &mut func);
        cur = h.teams().next_instance(t);
    }
}

/// `TeamPrototype::countTeamInstances()`.
pub fn proto_count_team_instances<H: TeamHost>(h: &H, proto: TeamPrototypeId) -> i32 {
    h.teams().instances(proto).len() as i32
}

/// `TeamPrototype::hasAnyBuildings()`.
pub fn proto_has_any_buildings<H: TeamHost>(h: &H, proto: TeamPrototypeId) -> bool {
    let mut cur = first_instance(h, proto);
    while let Some(t) = cur {
        if team_has_any_buildings(h, t) {
            return true;
        }
        cur = h.teams().next_instance(t);
    }
    false
}

/// `TeamPrototype::hasAnyBuildings(kindOf)`.
pub fn proto_has_any_buildings_kind_of<H: TeamHost>(h: &H, proto: TeamPrototypeId, kind_of: &KindOfMaskType) -> bool {
    let mut cur = first_instance(h, proto);
    while let Some(t) = cur {
        if team_has_any_buildings_kind_of(h, t, *kind_of) {
            return true;
        }
        cur = h.teams().next_instance(t);
    }
    false
}

/// `TeamPrototype::hasAnyUnits()`.
pub fn proto_has_any_units<H: TeamHost>(h: &H, proto: TeamPrototypeId) -> bool {
    let mut cur = first_instance(h, proto);
    while let Some(t) = cur {
        if team_has_any_units(h, t) {
            return true;
        }
        cur = h.teams().next_instance(t);
    }
    false
}

/// `TeamPrototype::hasAnyObjects()`.
pub fn proto_has_any_objects<H: TeamHost>(h: &H, proto: TeamPrototypeId) -> bool {
    let mut cur = first_instance(h, proto);
    while let Some(t) = cur {
        if team_has_any_objects(h, t) {
            return true;
        }
        cur = h.teams().next_instance(t);
    }
    false
}

/// `TeamPrototype::hasAnyBuildFacility()`.
pub fn proto_has_any_build_facility<H: TeamHost>(h: &H, proto: TeamPrototypeId) -> TResult<bool> {
    let mut cur = first_instance(h, proto);
    while let Some(t) = cur {
        if team_has_any_build_facility(h, t)? {
            return Ok(true);
        }
        cur = h.teams().next_instance(t);
    }
    Ok(false)
}

/// `TeamPrototype::damageTeamMembers(amount)`.
pub fn proto_damage_team_members<H: TeamHost>(h: &mut H, proto: TeamPrototypeId, amount: f32) {
    let mut cur = first_instance(h, proto);
    while let Some(t) = cur {
        team_damage_team_members(h, t, amount);
        cur = h.teams().next_instance(t);
    }
}

/// `TeamPrototype::moveTeamTo(destination)`.
pub fn proto_move_team_to<H: TeamHost>(h: &H, proto: TeamPrototypeId, destination: Coord3D) {
    let mut cur = first_instance(h, proto);
    while let Some(t) = cur {
        team_move_team_to(h, t, destination);
        cur = h.teams().next_instance(t);
    }
}

/// `TeamPrototype::updateState()`: every instance updates, then empty instances that are not worth keeping are
/// deleted (one at a time, the walk starting over after each).
pub fn proto_update_state<H: TeamHost>(h: &mut H, proto: TeamPrototypeId) -> TResult<()> {
    let mut cur = first_instance(h, proto);
    while let Some(t) = cur {
        team_update_state(h, t)?;
        cur = h.teams().next_instance(t);
    }
    // remove empty teams.
    let mut done = false;
    while !done {
        done = true;
        let mut cur = first_instance(h, proto);
        while let Some(t) = cur {
            if h.teams().first_member(t).is_none() {
                // Team has no members.
                if proto_ref(h, proto)?.get_is_singleton() {
                    cur = h.teams().next_instance(t);
                    continue; // Don't delete singleton teams, even if they are empty.
                }
                if let Some(p) = team_get_controlling_player(h, t)? {
                    if h.player_default_team(p) == Some(t) {
                        // This is the player's default team, so don't remove it.
                        cur = h.teams().next_instance(t);
                        continue;
                    }
                }
                // don't delete inactive teams - they are under construction
                if !team_ref(h, t)?.active {
                    cur = h.teams().next_instance(t);
                    continue;
                }
                // So remove it
                factory_team_about_to_be_deleted(h, Some(t))?;
                delete_team_instance(h, t)?;
                done = false;
                break; // Not sure what state the iterator is in after deleting a member of the list. jba
            }
            cur = h.teams().next_instance(t);
        }
    }
    Ok(())
}

/// `TeamPrototype::evaluateProductionCondition()`.
pub fn proto_evaluate_production_condition<H: TeamHost>(h: &mut H, proto: TeamPrototypeId) -> TResult<bool> {
    if proto_ref(h, proto)?.production_condition_always_false {
        // Set if we don't have a script.
        return Ok(false);
    }
    if let Some(s) = proto_ref(h, proto)?.production_condition_script {
        // If we are doing peridic evaluation, check the frame.
        if h.frame() < h.script_frame_to_evaluate(s) {
            return Ok(false);
        }
        let delay_seconds = h.script_delay_eval_seconds(s);
        if delay_seconds > 0 {
            let f = h.frame().wrapping_add(delay_seconds.wrapping_mul(LOGICFRAMES_PER_SECOND) as u32);
            h.script_set_frame_to_evaluate(s, f);
        }
        let owner = proto_get_controlling_player(h, proto)?;
        return Ok(h.script_evaluate_conditions(s, None, owner));
    }
    // We don't have a script yet, so check for one.
    let cond = proto_ref(h, proto)?.team_template.production_condition.clone();
    if cond.is_empty() {
        // No script defined.
        h.teams_mut().prototype_mut(proto).ok_or(TeamError::Null)?.production_condition_always_false = true;
        return Ok(false);
    }
    if let Some(p_script) = h.script_find_by_name(&cond) {
        // Check difficulty.
        let owner = proto_get_controlling_player(h, proto)?.ok_or(TeamError::Null)?;
        let ok = match h.player_difficulty(owner) {
            GameDifficulty::Easy => h.script_is_easy(p_script),
            GameDifficulty::Normal => h.script_is_normal(p_script),
            GameDifficulty::Hard => h.script_is_hard(p_script),
        };
        if !ok {
            h.teams_mut().prototype_mut(proto).ok_or(TeamError::Null)?.production_condition_always_false = true;
            return Ok(false);
        }

        // Make a copy of the script locally, just for paranoia's sake.
        let copy = h.script_duplicate(p_script);
        h.teams_mut().prototype_mut(proto).ok_or(TeamError::Null)?.production_condition_script = Some(copy);
        let owner = proto_get_controlling_player(h, proto)?;
        return Ok(h.script_evaluate_conditions(copy, None, owner));
    }
    // Couldn't find a script.
    h.teams_mut().prototype_mut(proto).ok_or(TeamError::Null)?.production_condition_always_false = true;
    Ok(false)
}

// ======================================================================================================================
// TeamFactory
// ======================================================================================================================

/// `TeamFactory::init()`: `clear()`.
pub fn factory_init<H: TeamHost>(h: &mut H) -> TResult<()> {
    factory_clear(h)
}

/// `TeamFactory::reset()`: both counters back to the invalid id, then `clear()`.
pub fn factory_reset<H: TeamHost>(h: &mut H) -> TResult<()> {
    h.teams_mut().reset_counters();
    factory_clear(h)
}

/// `TeamFactory::update()`: nothing.
pub fn factory_update<H: TeamHost>(_h: &mut H) {}

/// `TeamFactory::clear()`: the map is emptied first (a prototype removes itself from it when it goes), then every
/// prototype that was in it is deleted, in map order.
pub fn factory_clear<H: TeamHost>(h: &mut H) -> TResult<()> {
    let tmp: Vec<TeamPrototypeId> = h.teams().by_key().map(|(_, p)| p).collect();
    h.teams_mut().clear_by_key();
    for p in tmp {
        delete_team_prototype(h, p)?;
    }
    Ok(())
}

/// `TeamFactory::initFromSides(sides)`: `clear()`, then a prototype for each team of the map.
pub fn factory_init_from_sides<H: TeamHost>(h: &mut H, sides: &SidesList) -> TResult<()> {
    factory_clear(h)?;

    // create the teams we need.
    for i in 0..sides.get_num_teams() {
        let d = &sides.get_team_info(i).map_err(|_| TeamError::Null)?.dict;
        let (tname, oname, singleton) = {
            let (gen, keys) = h.names();
            let tname = d.get_ascii_string(keys.key(TEAM_NAME, gen)).0;
            let oname = d.get_ascii_string(keys.key(TEAM_OWNER, gen)).0;
            let singleton = d.get_bool(keys.key(TEAM_IS_SINGLETON, gen)).0;
            (tname, oname, singleton)
        };
        factory_init_team(h, &tname, &oname, singleton, d)?;
    }
    Ok(())
}

/// `TeamFactory::initTeam(name, owner, isSingleton, d)`: a prototype owned by the named player (the neutral player
/// when there is none), and for a singleton its one team.
pub fn factory_init_team<H: TeamHost>(h: &mut H, name: &[u8], owner: &[u8], is_singleton: bool, d: &Dict) -> TResult<()> {
    let nk = h.name_to_key(owner);
    let mut p_owner = h.player_list_find_with_name_key(nk);
    if p_owner.is_none() {
        p_owner = h.player_list_neutral();
    }
    let id = h.teams_mut().allocate_prototype_id();
    create_team_prototype(h, name, p_owner, is_singleton, d, id)?;
    if is_singleton {
        // Create the singleton team.
        factory_create_inactive_team(h, name)?;
    }
    Ok(())
}

/// `TeamFactory::addTeamPrototypeToList(team)`: under its name key, unless the key is taken.
pub fn factory_add_team_prototype_to_list<H: TeamHost>(h: &mut H, proto: TeamPrototypeId) {
    let Some(name) = h.teams().prototype(proto).map(|p| p.name.clone()) else { return };
    let nk = h.name_to_key(&name);
    h.teams_mut().add_prototype_to_list(nk, proto);
}

/// `TeamFactory::removeTeamPrototypeFromList(team)`: by name key, whoever holds it.
pub fn factory_remove_team_prototype_from_list<H: TeamHost>(h: &mut H, proto: TeamPrototypeId) {
    let Some(name) = h.teams().prototype(proto).map(|p| p.name.clone()) else { return };
    let nk = h.name_to_key(&name);
    h.teams_mut().remove_prototype_from_list(nk);
}

/// `TeamFactory::findTeamPrototype(name)`.
pub fn factory_find_team_prototype<H: TeamHost>(h: &mut H, name: &[u8]) -> Option<TeamPrototypeId> {
    let nk = h.name_to_key(name);
    h.teams().prototype_by_key(nk)
}

/// `TeamFactory::findTeamPrototypeByID(id)`: the map walked in key order.
pub fn factory_find_team_prototype_by_id<H: TeamHost>(h: &H, id: TeamPrototypeId) -> Option<TeamPrototypeId> {
    h.teams().by_key().map(|(_, p)| p).find(|p| h.teams().prototype(*p).is_some_and(|x| x.id == id))
}

/// `TeamFactory::findTeamByID(teamID)`: every prototype of the map, in key order, searched for the id.
pub fn factory_find_team_by_id<H: TeamHost>(h: &H, team_id: TeamId) -> Option<TeamId> {
    // simple case
    if team_id == TEAM_ID_INVALID {
        return None;
    }
    for (_, p) in h.teams().by_key() {
        if let Some(t) = proto_find_team_by_id(h, p, team_id) {
            return Some(t);
        }
    }
    None
}

/// What `createInactiveTeam` does when the prototype asks for its production condition's actions to run.
fn execute_production_actions<H: TeamHost>(h: &mut H, proto: TeamPrototypeId) -> TResult<()> {
    let info = &proto_ref(h, proto)?.team_template;
    if info.execute_actions {
        let cond = info.production_condition.clone();
        if let Some(s) = h.script_find_by_name(&cond) {
            h.script_execute_action(s, None);
        }
    }
    Ok(())
}

/// `TeamFactory::createInactiveTeam(name)`: a team to add members to as they are built; a singleton prototype
/// gives its one team back. Throws `ERROR_BAD_ARG` for a name with no prototype.
pub fn factory_create_inactive_team<H: TeamHost>(h: &mut H, name: &[u8]) -> TResult<TeamId> {
    let tp = factory_find_team_prototype(h, name).ok_or(TeamError::BadArg)?;
    if proto_ref(h, tp)?.get_is_singleton() {
        if let Some(t) = h.teams().first_instance(tp) {
            execute_production_actions(h, tp)?;
            return Ok(t);
        }
    }
    let id = h.teams_mut().allocate_team_id();
    let t = create_team_instance(h, tp, id)?;
    execute_production_actions(h, tp)?;
    Ok(t)
}

/// `TeamFactory::createTeam(name)`: an inactive team, made active.
pub fn factory_create_team<H: TeamHost>(h: &mut H, name: &[u8]) -> TResult<TeamId> {
    let t = factory_create_inactive_team(h, name)?;
    team_set_active(h, t)?;
    Ok(t)
}

/// `Team::setActive()`: a team is created when it is first activated.
pub fn team_set_active<H: TeamHost>(h: &mut H, team: TeamId) -> TResult<()> {
    let t = team_mut(h, team)?;
    if !t.active {
        t.created = true;
        t.active = true;
    }
    Ok(())
}

/// `TeamFactory::createTeamOnPrototype(prototype)`: throws `ERROR_BAD_ARG` for `NULL`; a singleton gives its one
/// team back.
pub fn factory_create_team_on_prototype<H: TeamHost>(h: &mut H, prototype: Option<TeamPrototypeId>) -> TResult<TeamId> {
    let prototype = prototype.ok_or(TeamError::BadArg)?;
    if proto_ref(h, prototype)?.get_is_singleton() {
        if let Some(t) = h.teams().first_instance(prototype) {
            return Ok(t);
        }
    }
    let id = h.teams_mut().allocate_team_id();
    let t = create_team_instance(h, prototype, id)?;
    team_set_active(h, t)?;
    Ok(t)
}

/// `TeamFactory::findTeam(name)`: the newest team of the prototype, making one if it has none and is not a
/// singleton.
pub fn factory_find_team<H: TeamHost>(h: &mut H, name: &[u8]) -> TResult<Option<TeamId>> {
    let Some(tp) = factory_find_team_prototype(h, name) else { return Ok(None) };
    let mut t = h.teams().first_instance(tp);
    if t.is_none() && !proto_ref(h, tp)?.get_is_singleton() {
        t = Some(factory_create_inactive_team(h, name)?);
    }
    Ok(t)
}

/// `TeamFactory::teamAboutToBeDeleted(team)`: every prototype of the map drops its overrides for the team, then
/// the player list is told.
pub fn factory_team_about_to_be_deleted<H: TeamHost>(h: &mut H, team: Option<TeamId>) -> TResult<()> {
    let protos: Vec<TeamPrototypeId> = h.teams().by_key().map(|(_, p)| p).collect();
    for p in protos {
        proto_team_about_to_be_deleted(h, p, team)?;
    }
    // if (ThePlayerList): the player list always exists here
    h.player_list_team_about_to_be_deleted(team.unwrap_or(TEAM_ID_INVALID));
    Ok(())
}
