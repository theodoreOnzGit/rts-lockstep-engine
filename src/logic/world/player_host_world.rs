// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/Team.cpp, Player.cpp, PlayerList.cpp
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/ProductionPrerequisite.cpp (`isSatisfied`, `calcNumPrereqUnitsOwned`)
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/Science.cpp (`playerHasPrereqsForScience`)
//   (the calls those files make out of the team and player lists, answered by the world)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Integration batch I3: the team and player hosts (`TeamHost`, `PlayerHost`, `PlayerObjectHost`, `PlayerSquadHost`,
//! `PlayerXferHost`, `PlayerInitHost`) for [`World`], so that the team and player code of `team_ops.rs` and
//! `player_*.rs` runs on the world's own arenas, objects, templates and stores.
//!
//! # What answers what
//!
//! - **The world itself** answers every call into a unit it holds: the two arenas (`world.teams`, `world.players`), the
//!   name keys (`world.names`, `world.keys`), the frame, the objects and their modules (a special power, a container, a
//!   body, a production queue, a stealth module, an AI module is asked with a [`ModuleCall`]), the thing templates, the
//!   player templates, the ranks, the sciences and the special powers (`world.statics`), the waypoints
//!   (`world.terrain`), the sides list (`world.sides`), the client events (`world.events`) and the global data.
//! - **The partition manager** answers its three questions live when `cuts.live_partition` is on (I1), and from
//!   [`PlayerHostCuts`] otherwise.
//! - **A unit the world does not hold yet** (the script engine and its scripts, the AI players, the victory conditions,
//!   the message stream, the in-game UI, the network, the game text) is a cut point: the call is logged as
//!   [`CutCall::Host`] with upstream's name, and the answer comes from [`PlayerHostCuts`]. Integration batches I5 to I9
//!   replace each with the unit.
//!
//! # Handles
//!
//! - A `TemplateRef` is the thing template's id (`ThingTemplate::getTemplateID`), a `PlayerTemplateRef` the index in
//!   the player template store, a `SpecialPowerTemplateRef` the special power template's id.
//! - A `SpecialPowerModuleRef` is the object id and the module's index, `(id << 8) | index`; upstream's pointer has no
//!   such limit, and an object id at or above `1 << 24` or a module index at or above 256 is not given out (the module
//!   is skipped). No game reaches either.
//! - A `ScriptRef` is a 1-based place in [`PlayerHostCuts::scripts`], a `GroupRef` and a `MessageRef` keys of
//!   [`PlayerHostCuts::groups`] and [`PlayerHostCuts::messages`], a `ScriptListRef` a number this file gives out.
//!
//! # Side effects kept
//!
//! - `getPercentReady()` and `getReadyFrame()` of a shared-synced power start the player's timer when it has none
//!   (`getOrStartSpecialPowerReadyFrame`); they are asked through the module, which does the same. This is why the host
//!   methods that reach a module take `&mut self`.
//! - `findPlayerTemplate` makes its fifteen legacy faction keys on the first call.
//!
//! `p1w13-sonnet`, integration batch I3, 2026-10-31.

use std::collections::{BTreeMap, BTreeSet};

use crate::common::base_type::Coord3D;
use crate::common::dict::Dict;
use crate::common::disabled_types::{DISABLED_HELD, DISABLED_UNDERPOWERED};
use crate::common::game_common::{CommandSourceType, GameDifficulty, PlayerType, Relationship, VeterancyLevel};
use crate::common::game_type::TimeOfDay;
use crate::common::kind_of::{KindOfMaskType, KindOfType, KINDOF_STRUCTURE};
use crate::common::money::Money;
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use crate::common::object_status_types::ObjectStatusTypes;
use crate::common::production_prerequisite::{MAX_PREREQ, UNIT_OR_WITH_PREV};
use crate::common::science::ScienceType;
use crate::common::special_power_type::SpecialPowerType;
use crate::common::thing_template::ThingTemplate;
use crate::common::unicode_string::UnicodeString;
use crate::common::upgrade::{UpgradeCenter, UpgradeMaskType};
use crate::common::well_known_keys::WellKnownKeys;
use crate::common::xfer::Xfer;
use crate::logic::client_event::ClientEvent;
use crate::logic::damage::DamageInfo;
use crate::logic::sides_list::BuildListInfo;
use crate::logic::world::academy_stats::AcademyObject;
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::{ObjectId, PlayerIndex, TeamId, TeamPrototypeId, INVALID_ID};
use crate::logic::world::module_call::{ModuleCall, ModuleReply};
use crate::logic::world::object::ModuleIndex;
use crate::logic::world::object_cuts::CutCall;
use crate::logic::object_iter::IterOrderType;
use crate::logic::world::partition::filters::ALLOW_ENEMIES;
use crate::logic::world::partition::{DistanceCalculationType, PartitionFilter, ValueOrThreat};
use crate::logic::world::player::{PlayerArena, PlayerTemplateRef};
use crate::logic::world::player_init::{PlayerInitHost, ScriptListRef};
use crate::logic::world::player_objects::{CommandSource, PlayerObjectHost};
use crate::logic::world::player_ops::{
    player_count_objects_by_thing_template, player_get_player_difficulty, player_get_relationship, player_has_science, player_add_team_to_list,
    player_pre_team_destroy, player_remove_team_from_list, PResult, PlayerGlobals, PlayerHost, PlayerTemplateView, RankInfoView,
    SpecialPowerModuleRef, SpecialPowerTemplateRef, SpecialPowerView,
};
use crate::logic::world::player_list::player_list_team_about_to_be_deleted;
use crate::logic::world::player_squads::{MessageRef, PlayerSquadHost};
use crate::logic::world::player_xfer::PlayerXferHost;
use crate::logic::world::score_keeper::ScoredObject;
use crate::logic::world::team::{ScriptRef, TeamArena, WaypointId, WaypointInfo};
use crate::logic::world::team_ops::{GroupRef, TeamHost, TemplateRef, TriggerRef};

/// A script as the team code reads it (`Script`'s name, evaluation frame, delay, difficulty and one-shot flags), and the
/// answers its condition gives, one per evaluation (the last repeats; none: false).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScriptStandIn {
    /// `getName()`.
    pub name: Vec<u8>,
    /// `getFrameToEvaluate()`.
    pub frame_to_evaluate: u32,
    /// `getDelayEvalSeconds()`.
    pub delay_eval_seconds: i32,
    /// `isEasy()`, `isNormal()`, `isHard()`.
    pub easy: bool,
    /// See `easy`.
    pub normal: bool,
    /// See `easy`.
    pub hard: bool,
    /// `isOneShot()`.
    pub one_shot: bool,
    /// What `evaluateConditions` answers, in order.
    pub conditions: Vec<bool>,
    /// How many times its conditions were evaluated.
    pub evaluations: usize,
}

/// What an AI player answers (`AIPlayer`), until batch I9 brings the AI player in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AiPlayerStandIn {
    /// `getAIDifficulty()`.
    pub difficulty: GameDifficulty,
    /// `isSupplySourceSafe`.
    pub supply_source_safe: bool,
    /// `isSupplySourceAttacked`.
    pub supply_source_attacked: bool,
    /// `checkBridges`.
    pub bridges: bool,
    /// `getBaseCenter`: `None` for false.
    pub base_center: Option<Coord3D>,
    /// `computeSuperweaponTarget`: `None` for false.
    pub superweapon_target: Option<Coord3D>,
    /// `calcClosestConstructionZoneLocation`: `None` for false.
    pub construction_zone: Option<Coord3D>,
    /// `getAiEnemy()`.
    pub enemy: Option<PlayerIndex>,
}

impl Default for AiPlayerStandIn {
    fn default() -> Self {
        AiPlayerStandIn {
            difficulty: GameDifficulty::Normal,
            supply_source_safe: true,
            supply_source_attacked: false,
            bridges: false,
            base_center: None,
            superweapon_target: None,
            construction_zone: None,
            enemy: None,
        }
    }
}

/// The answers of the units the player and team hosts reach that the world does not hold yet. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerHostCuts {
    /// `TheScriptEngine`'s scripts, by [`ScriptRef`] (1-based).
    pub scripts: Vec<ScriptStandIn>,
    /// The script lists given out (`duplicateAndQualify`), for the next handle.
    pub script_lists_given: ScriptListRef,
    /// The script list of each side and skirmish side (`getScriptList()`): `None` is `NULL`.
    pub side_scripts: BTreeMap<i32, ScriptListRef>,
    /// See `side_scripts`.
    pub skirmish_side_scripts: BTreeMap<i32, ScriptListRef>,
    /// `TheScriptEngine->getGlobalDifficulty()`.
    pub global_difficulty: GameDifficulty,
    /// The AI players, by player index.
    pub ai: BTreeMap<PlayerIndex, AiPlayerStandIn>,
    /// `TheAI->getAiData()->m_forceSkirmishAI`.
    pub force_skirmish_ai: bool,
    /// `TheGameInfo` (`None`: `NULL`): its starting cash.
    pub game_info_starting_cash: Option<Money>,
    /// `TheGameLogic->isInSinglePlayerGame()`, `isInShellGame()`, `getRankLevelLimit()`.
    pub in_single_player_game: bool,
    /// See `in_single_player_game`.
    pub in_shell_game: bool,
    /// See `in_single_player_game`.
    pub rank_level_limit: i32,
    /// `TheInGameUI->isClientQuiet()`.
    pub client_quiet: bool,
    /// `TheNetwork != NULL`.
    pub network_present: bool,
    /// `TheGameText->fetch("SCIENCE:GeneralName")`.
    pub general_name: Vec<u16>,
    /// The AI groups' members (`AIGroup::getAllIDs`).
    pub groups: BTreeMap<GroupRef, Vec<ObjectId>>,
    /// The object id arguments of the messages (`GameMessage::getArgument(i)->objectID`).
    pub messages: BTreeMap<MessageRef, Vec<ObjectId>>,
    /// The objects with an enemy within their vision range, while the partition manager is not live.
    pub enemy_near: BTreeSet<ObjectId>,
}

impl Default for PlayerHostCuts {
    fn default() -> Self {
        PlayerHostCuts {
            scripts: Vec::new(),
            script_lists_given: 0,
            side_scripts: BTreeMap::new(),
            skirmish_side_scripts: BTreeMap::new(),
            global_difficulty: GameDifficulty::Normal,
            ai: BTreeMap::new(),
            force_skirmish_ai: false,
            game_info_starting_cash: None,
            in_single_player_game: false,
            in_shell_game: false,
            rank_level_limit: 1000,
            client_quiet: false,
            network_present: false,
            general_name: "General".encode_utf16().collect(),
            groups: BTreeMap::new(),
            messages: BTreeMap::new(),
            enemy_near: BTreeSet::new(),
        }
    }
}

/// The highest object id a [`SpecialPowerModuleRef`] holds (exclusive).
const SP_REF_ID_LIMIT: u32 = 1 << 24;

/// The handle of a module: see the module docs.
fn sp_ref(o: ObjectId, idx: ModuleIndex) -> Option<SpecialPowerModuleRef> {
    let id = o as u32;
    (id < SP_REF_ID_LIMIT && idx < 256).then_some((id << 8) | u32::from(idx))
}

/// The object and module a handle names.
fn sp_unref(m: SpecialPowerModuleRef) -> (ObjectId, ModuleIndex) {
    ((m >> 8) as ObjectId, (m & 0xff) as ModuleIndex)
}

/// The objects of an iterator, in its order.
pub(crate) fn iterator_ids(mut it: crate::logic::object_iter::SimpleObjectIterator<ObjectId>) -> Vec<ObjectId> {
    let mut v = Vec::new();
    let mut cur = it.first();
    while let Some(o) = cur {
        v.push(o);
        cur = it.next();
    }
    v
}

impl World {
    /// Logs a call of the player and team hosts into a unit the world does not hold (see the module docs).
    fn host_cut(&mut self, call: &'static str, args: &[i64], text: &[u8]) {
        self.cut(CutCall::Host { call, args: args.to_vec(), text: text.to_vec() });
    }

    /// The thing template a handle names (its final override).
    fn tmpl_ref(&self, t: TemplateRef) -> Option<&ThingTemplate> {
        u16::try_from(t).ok().and_then(|id| self.statics.thing_factory.find_by_template_id(id)).map(|o| &o.get_final_override().data)
    }

    /// The template id of an object as a handle.
    fn object_template_ref(&self, o: ObjectId) -> Option<TemplateRef> {
        self.find_object(o).map(|obj| TemplateRef::from(obj.template().get_template_id()))
    }

    /// The special power modules of an object, in module order.
    fn special_power_module_indices(&self, o: ObjectId) -> Vec<ModuleIndex> {
        self.modules_with(o, crate::common::module::MODULEINTERFACE_SPECIAL_POWER)
    }

    /// A question to a special power module.
    fn sp_ask(&mut self, m: SpecialPowerModuleRef, call: ModuleCall) -> ModuleReply {
        let (o, idx) = sp_unref(m);
        self.module_call(o, idx, call)
    }

    /// A question to the container of an object (`getContain()`); `None` when it has none.
    fn contain_ask_opt(&mut self, o: ObjectId, call: ModuleCall) -> Option<ModuleReply> {
        let idx = self.get_contain(o)?;
        Some(self.module_call(o, idx, call))
    }

    /// The scored view of an object (`ScoreKeeper`'s reads of it).
    fn scored_object(&self, o: ObjectId) -> Option<ScoredObject> {
        let obj = self.find_object(o)?;
        let t = obj.template();
        Some(ScoredObject {
            template_name: t.get_name().to_vec(),
            kind_of: *t.kindof(),
            controller: self.get_controlling_player(o).unwrap_or(-1),
            under_construction: obj.test_status(crate::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION),
        })
    }

    /// `ProductionPrerequisite::isSatisfied(player)`, of the template's prerequisite `i`.
    fn prereq_is_satisfied(&self, t: TemplateRef, i: i32, p: PlayerIndex) -> bool {
        let Some(pre) = self.tmpl_ref(t).and_then(|tt| usize::try_from(i).ok().and_then(|i| tt.get_nth_prereq(i))) else {
            return false;
        };
        if self.players.get_nth(p).is_none() {
            return false;
        }
        // gotta have all the prereq sciences.
        for s in pre.prereq_sciences() {
            if !player_has_science(self, p, *s).unwrap_or(false) {
                return false;
            }
        }
        // the player must have at least one instance of each prereq unit.
        let units = pre.prereq_units();
        let cnt = units.len().min(MAX_PREREQ);
        let things: Vec<Option<TemplateRef>> = units[..cnt].iter().map(|u| u.unit.map(TemplateRef::from)).collect();
        let mut own = [0i32; MAX_PREREQ];
        if player_count_objects_by_thing_template(self, p, &things, false, &mut own[..cnt], true).is_err() {
            return false;
        }
        // fix up the "or" cases. (start at 1!)
        for i in 1..cnt {
            if units[i].flags & UNIT_OR_WITH_PREV != 0 {
                own[i] += own[i - 1]; // lump 'em together for prereq purposes
                own[i - 1] = -1; // flag for "ignore me"
            }
        }
        own[..cnt].iter().all(|c| *c != 0)
    }
}

impl TeamHost for World {
    fn teams(&self) -> &TeamArena {
        &self.teams
    }
    fn teams_mut(&mut self) -> &mut TeamArena {
        &mut self.teams
    }
    fn names(&mut self) -> (&mut NameKeyGenerator, &WellKnownKeys) {
        (&mut self.names, &self.keys)
    }
    fn frame(&self) -> u32 {
        World::frame(self)
    }
    fn game_logic_objects(&self) -> Vec<ObjectId> {
        self.objects.ids_in_order()
    }
    fn game_logic_find_object(&self, id: ObjectId) -> bool {
        self.find_object(id).is_some()
    }
    fn game_logic_destroy_object(&mut self, o: ObjectId) {
        self.destroy_object(o);
    }
    fn player_list_find_with_name_key(&mut self, key: NameKeyType) -> Option<PlayerIndex> {
        self.players.find_with_name_key(key.0 as i32).map(|p| p.player_index)
    }
    fn player_list_neutral(&self) -> Option<PlayerIndex> {
        self.players.get_nth(0).map(|p| p.player_index)
    }
    fn player_list_count(&self) -> i32 {
        self.players.player_count()
    }
    fn player_list_nth(&self, i: i32) -> Option<PlayerIndex> {
        self.players.get_nth(i).map(|p| p.player_index)
    }
    fn player_list_team_about_to_be_deleted(&mut self, team: TeamId) {
        let _ = player_list_team_about_to_be_deleted(self, team);
    }
    fn player_add_team_to_list(&mut self, p: PlayerIndex, proto: TeamPrototypeId) {
        let _ = player_add_team_to_list(self, p, proto);
    }
    fn player_remove_team_from_list(&mut self, p: PlayerIndex, proto: TeamPrototypeId) {
        let _ = player_remove_team_from_list(self, p, proto);
    }
    fn player_pre_team_destroy(&mut self, p: PlayerIndex, team: TeamId) {
        let _ = player_pre_team_destroy(self, p, Some(team));
    }
    fn player_default_team(&self, p: PlayerIndex) -> Option<TeamId> {
        self.players.get_nth(p).and_then(|x| x.default_team)
    }
    fn player_relationship(&self, p: PlayerIndex, that: Option<TeamId>) -> Relationship {
        player_get_relationship(self, p, that).unwrap_or(Relationship::Neutral)
    }
    fn player_type(&self, p: PlayerIndex) -> PlayerType {
        self.players.get_nth(p).map_or(PlayerType::Human, |x| x.player_type)
    }
    fn player_difficulty(&self, p: PlayerIndex) -> GameDifficulty {
        player_get_player_difficulty(self, p).unwrap_or(GameDifficulty::Normal)
    }
    fn player_beacon_template(&self, p: PlayerIndex) -> Option<TemplateRef> {
        let t = self.players.get_nth(p)?.template?;
        let pt = self.statics.player_template_store.nth_player_template(t as i32)?;
        World::thing_factory_find_template(self, pt.beacon_template()).map(TemplateRef::from)
    }
    fn script_find_by_name(&mut self, name: &[u8]) -> Option<ScriptRef> {
        self.host_cut("ScriptEngine::findScriptByName", &[], name);
        self.cuts.player_host.scripts.iter().position(|s| s.name == name).map(|i| i as ScriptRef + 1)
    }
    fn script_duplicate(&mut self, s: ScriptRef) -> ScriptRef {
        self.host_cut("Script::duplicate", &[i64::from(s)], &[]);
        let scripts = &mut self.cuts.player_host.scripts;
        let mut copy = scripts.get(s as usize - 1).cloned().unwrap_or_default();
        copy.evaluations = 0;
        scripts.push(copy);
        scripts.len() as ScriptRef
    }
    fn script_delete(&mut self, s: ScriptRef) {
        self.host_cut("deleteInstance(Script)", &[i64::from(s)], &[]);
    }
    fn script_frame_to_evaluate(&self, s: ScriptRef) -> u32 {
        self.cuts.player_host.scripts.get(s as usize - 1).map_or(0, |x| x.frame_to_evaluate)
    }
    fn script_set_frame_to_evaluate(&mut self, s: ScriptRef, frame: u32) {
        self.host_cut("Script::setFrameToEvaluate", &[i64::from(s), i64::from(frame)], &[]);
        if let Some(x) = self.cuts.player_host.scripts.get_mut(s as usize - 1) {
            x.frame_to_evaluate = frame;
        }
    }
    fn script_delay_eval_seconds(&self, s: ScriptRef) -> i32 {
        self.cuts.player_host.scripts.get(s as usize - 1).map_or(0, |x| x.delay_eval_seconds)
    }
    fn script_is_easy(&self, s: ScriptRef) -> bool {
        self.cuts.player_host.scripts.get(s as usize - 1).is_some_and(|x| x.easy)
    }
    fn script_is_normal(&self, s: ScriptRef) -> bool {
        self.cuts.player_host.scripts.get(s as usize - 1).is_some_and(|x| x.normal)
    }
    fn script_is_hard(&self, s: ScriptRef) -> bool {
        self.cuts.player_host.scripts.get(s as usize - 1).is_some_and(|x| x.hard)
    }
    fn script_is_one_shot(&self, s: ScriptRef) -> bool {
        self.cuts.player_host.scripts.get(s as usize - 1).is_some_and(|x| x.one_shot)
    }
    fn script_name(&self, s: ScriptRef) -> Vec<u8> {
        self.cuts.player_host.scripts.get(s as usize - 1).map(|x| x.name.clone()).unwrap_or_default()
    }
    fn script_evaluate_conditions(&mut self, s: ScriptRef, team: Option<TeamId>, player: Option<PlayerIndex>) -> bool {
        self.host_cut("ScriptEngine::evaluateConditions", &[i64::from(s), i64::from(team.unwrap_or(0)), i64::from(player.unwrap_or(-1))], &[]);
        let Some(x) = self.cuts.player_host.scripts.get_mut(s as usize - 1) else { return false };
        let r = x.conditions.get(x.evaluations).or(x.conditions.last()).copied().unwrap_or(false);
        x.evaluations += 1;
        r
    }
    fn script_execute_action(&mut self, s: ScriptRef, team: Option<TeamId>) {
        self.host_cut("ScriptEngine::executeActions", &[i64::from(s), i64::from(team.unwrap_or(0))], &[]);
    }
    fn script_run(&mut self, name: &[u8], team: TeamId) {
        self.host_cut("ScriptEngine::runScript", &[i64::from(team)], name);
    }
    fn script_notify_of_team_destruction(&mut self, team: TeamId) {
        self.host_cut("ScriptEngine::notifyOfTeamDestruction", &[i64::from(team)], &[]);
    }
    fn script_append_debug_message(&mut self, msg: &[u8], force_pause: bool) {
        self.host_cut("ScriptEngine::AppendDebugMessage", &[i64::from(force_pause)], msg);
    }
    fn terrain_waypoints(&self) -> Vec<WaypointInfo> {
        self.terrain
            .waypoint_handles()
            .into_iter()
            .filter_map(|h| self.terrain.waypoint(h))
            .map(|w| WaypointInfo { name: w.name().str().to_vec(), location: *w.location() })
            .collect()
    }
    fn terrain_waypoint_by_id(&self, id: WaypointId) -> Option<WaypointId> {
        self.terrain.waypoint_by_id(id).and_then(|h| self.terrain.waypoint(h)).map(|w| w.id() as WaypointId)
    }
    fn obj_template(&self, o: ObjectId) -> Option<TemplateRef> {
        self.object_template_ref(o)
    }
    fn tmpl_is_equivalent_to(&self, a: TemplateRef, b: Option<TemplateRef>) -> bool {
        match (u16::try_from(a), b.map(u16::try_from)) {
            (Ok(a), Some(Ok(b))) => self.thing_template_is_equivalent_to(a, b),
            _ => false,
        }
    }
    fn tmpl_is_kind_of(&self, t: TemplateRef, kind: KindOfType) -> bool {
        self.tmpl_ref(t).is_some_and(|t| t.is_kind_of(kind))
    }
    fn tmpl_is_kind_of_multi(&self, t: TemplateRef, set: &KindOfMaskType, clear: &KindOfMaskType) -> bool {
        self.tmpl_ref(t).is_some_and(|t| t.is_kind_of_multi(set, clear))
    }
    fn tmpl_is_build_facility(&self, t: TemplateRef) -> bool {
        self.tmpl_ref(t).is_some_and(|t| t.is_build_facility())
    }
    fn tmpl_name(&self, t: TemplateRef) -> Vec<u8> {
        self.tmpl_ref(t).map(|t| t.get_name().to_vec()).unwrap_or_default()
    }
    fn tmpl_build_variations(&self, t: TemplateRef) -> Vec<Vec<u8>> {
        self.tmpl_ref(t).map(|t| t.get_build_variations().to_vec()).unwrap_or_default()
    }
    fn obj_is_effectively_dead(&self, o: ObjectId) -> bool {
        self.find_object(o).is_some_and(|x| x.is_effectively_dead())
    }
    fn obj_is_destroyed(&self, o: ObjectId) -> bool {
        self.find_object(o).is_some_and(|x| x.is_destroyed())
    }
    fn obj_is_kind_of(&self, o: ObjectId, kind: KindOfType) -> bool {
        self.find_object(o).is_some_and(|x| x.is_kind_of(kind))
    }
    fn obj_is_kind_of_multi(&self, o: ObjectId, set: &KindOfMaskType, clear: &KindOfMaskType) -> bool {
        self.find_object(o).is_some_and(|x| x.template().is_kind_of_multi(set, clear))
    }
    fn obj_test_status(&self, o: ObjectId, status: ObjectStatusTypes) -> bool {
        self.find_object(o).is_some_and(|x| x.test_status(status))
    }
    fn obj_is_disabled_held(&self, o: ObjectId) -> bool {
        self.find_object(o).is_some_and(|x| x.is_disabled_by_type(DISABLED_HELD))
    }
    fn obj_is_contained(&self, o: ObjectId) -> bool {
        self.is_contained(o)
    }
    fn obj_controlling_player(&self, o: ObjectId) -> Option<PlayerIndex> {
        self.get_controlling_player(o)
    }
    fn obj_team(&self, o: ObjectId) -> Option<TeamId> {
        self.get_team(o)
    }
    fn obj_position(&self, o: ObjectId) -> Coord3D {
        self.find_object(o).map(|x| x.get_position()).unwrap_or_default()
    }
    fn obj_vision_range(&self, o: ObjectId) -> f32 {
        self.get_vision_range(o)
    }
    fn obj_heal_completely(&mut self, o: ObjectId) {
        self.heal_completely(o);
    }
    fn obj_kill(&mut self, o: ObjectId) {
        // `kill()`'s defaults
        self.kill(o, crate::logic::damage::DAMAGE_UNRESISTABLE, crate::logic::damage::DEATH_NORMAL);
    }
    fn obj_attempt_damage(&mut self, o: ObjectId, amount: f32) {
        let mut info = DamageInfo::default();
        info.input.damage_type = crate::logic::damage::DAMAGE_UNRESISTABLE;
        info.input.death_type = crate::logic::damage::DEATH_NORMAL;
        info.input.source_id = INVALID_ID;
        info.input.amount = amount;
        self.attempt_damage(o, &mut info);
    }
    fn obj_set_team(&mut self, o: ObjectId, team: Option<TeamId>) {
        self.set_team(o, team);
    }
    fn obj_handle_partition_cell_maintenance(&mut self, o: ObjectId) {
        self.handle_partition_cell_maintenance(o);
    }
    fn obj_contain_count(&mut self, o: ObjectId) -> Option<i32> {
        self.contain_ask_opt(o, ModuleCall::GetContainCount).map(|r| r.as_int(0))
    }
    fn obj_remove_all_contained(&mut self, o: ObjectId) {
        self.contain_ask_opt(o, ModuleCall::RemoveAllContained { expose_stealth_units: false });
    }
    fn obj_did_enter(&self, o: ObjectId, t: TriggerRef) -> bool {
        self.did_enter(o, Some(t as i32))
    }
    fn obj_did_exit(&self, o: ObjectId, t: TriggerRef) -> bool {
        self.did_exit(o, Some(t as i32))
    }
    fn obj_is_inside(&self, o: ObjectId, t: TriggerRef) -> bool {
        self.is_inside(o, Some(t as i32))
    }
    fn obj_has_ai(&self, o: ObjectId) -> bool {
        self.find_object(o).is_some_and(|x| x.ai.is_some())
    }
    fn ai_is_idle(&self, o: ObjectId) -> bool {
        self.cuts.ai_idle.contains(&o)
    }
    fn ai_valid_surfaces(&self, o: ObjectId) -> u32 {
        self.cuts.ai_valid_surfaces.get(&o).copied().unwrap_or(0)
    }
    fn ai_is_recruitable(&self, o: ObjectId) -> bool {
        !self.cuts.ai_not_recruitable.contains(&o)
    }
    fn ai_group_add(&mut self, group: GroupRef, o: ObjectId) {
        self.host_cut("AIGroup::add", &[i64::from(group), i64::from(o)], &[]);
        self.cuts.player_host.groups.entry(group).or_default().push(o);
    }
    fn team_renamed(&mut self, old: TeamId, new: TeamId) {
        for id in self.objects.ids_in_order() {
            if let Some(obj) = self.objects.find_mut(id) {
                if obj.team == old {
                    obj.team = new;
                }
            }
        }
        for i in 0..16 {
            if let Some(p) = self.players.get_nth_mut(i) {
                if p.default_team == Some(old) {
                    p.default_team = Some(new);
                }
            }
        }
    }
    fn partition_closest_enemy(&mut self, o: ObjectId, range: f32) -> bool {
        if !self.cuts.live_partition {
            self.host_cut("PartitionManager::getClosestObject", &[i64::from(o), i64::from(range.to_bits())], &[]);
            return self.cuts.player_host.enemy_near.contains(&o);
        }
        let filters = [PartitionFilter::Relationship { obj: o, flags: ALLOW_ENEMIES }, PartitionFilter::Alive, PartitionFilter::SameMapStatus { obj: o }];
        self.with_partition(|pm, w| pm.get_closest_object(w, o, range, DistanceCalculationType::FromCenter2D, &filters).object.is_some())
    }
}

impl PlayerHost for World {
    fn players(&self) -> &PlayerArena {
        &self.players
    }
    fn players_mut(&mut self) -> &mut PlayerArena {
        &mut self.players
    }
    fn globals(&self) -> PlayerGlobals {
        let c = &self.cuts.player_host;
        let g = self.statics.global_data.the_global_data();
        PlayerGlobals {
            client_retaliation_mode_enabled: g.is_some_and(|g| g.client_retaliation_mode_enabled),
            base_value_per_supply_box: g.map_or(0, |g| g.base_value_per_supply_box as u32),
            solo_player_health_bonus: g.map_or([[1.0; 3]; 2], |g| g.solo_player_health_bonus_for_difficulty),
            default_starting_cash: g.map_or_else(Money::new, |g| g.default_starting_cash),
            time_of_day_is_night: g.is_some_and(|g| g.time_of_day == TimeOfDay::Night as i32),
            game_info_starting_cash: c.game_info_starting_cash,
            force_skirmish_ai: c.force_skirmish_ai,
            in_single_player_game: c.in_single_player_game,
            in_shell_game: c.in_shell_game,
            in_game_logic_update: self.is_in_game_logic_update(),
            rank_level_limit: c.rank_level_limit,
            client_quiet: c.client_quiet,
            global_difficulty: c.global_difficulty,
        }
    }
    fn player_template(&self, t: PlayerTemplateRef) -> PlayerTemplateView {
        let pt = self.statics.player_template_store.nth_player_template(t as i32).expect("a player template the store gave out");
        PlayerTemplateView::from_template(pt)
    }
    fn client_event(&mut self, e: ClientEvent) {
        self.events.push(e);
    }
    fn rank_level_count(&self) -> i32 {
        self.statics.rank_info_store.get_rank_level_count()
    }
    fn rank_info(&self, level: i32) -> Option<RankInfoView> {
        self.statics.rank_info_store.get_rank_info(level).map(|r| RankInfoView {
            skill_points_needed: r.skill_points_needed,
            science_purchase_points_granted: r.science_purchase_points_granted,
            sciences_granted: r.sciences_granted.clone(),
        })
    }
    fn science_is_grantable(&self, science: ScienceType) -> bool {
        self.statics.science_store.is_science_grantable(science)
    }
    fn science_purchase_cost(&self, science: ScienceType) -> i32 {
        self.statics.science_store.get_science_purchase_cost(science)
    }
    fn science_has_prereqs(&self, p: PlayerIndex, science: ScienceType) -> bool {
        // `ScienceStore::playerHasPrereqsForScience`
        let Some(info) = self.statics.science_store.find_science_info(science) else { return false };
        info.prereq_sciences().iter().all(|s| player_has_science(self, p, *s).unwrap_or(false))
    }
    fn game_text_general_name(&mut self) -> UnicodeString {
        self.host_cut("GameTextInterface::fetch", &[], b"SCIENCE:GeneralName");
        UnicodeString::try_from_chars(&self.cuts.player_host.general_name).unwrap_or_default()
    }
    fn script_notify_of_object_creation_or_destruction(&mut self) {
        self.host_cut("ScriptEngine::notifyOfObjectCreationOrDestruction", &[], &[]);
    }
    fn script_notify_of_acquired_science(&mut self, player_index: PlayerIndex, science: ScienceType) {
        self.host_cut("ScriptEngine::notifyOfAcquiredScience", &[i64::from(player_index), i64::from(science.0)], &[]);
    }
    fn victory_has_single_player_been_defeated(&self, p: PlayerIndex) -> bool {
        // batch I5: the world's own conditions
        self.victory.has_single_player_been_defeated(self, Some(p))
    }
    fn message_enable_retaliation_mode(&mut self, player_index: PlayerIndex, enabled: bool) {
        self.host_cut("MessageStream::appendMessage(MSG_ENABLE_RETALIATION_MODE)", &[i64::from(player_index), i64::from(enabled)], &[]);
    }
    fn academy_init(&mut self, p: PlayerIndex) {
        let frame = World::frame(self);
        let side = self.players.get_nth(p).and_then(|x| x.template).and_then(|t| self.statics.player_template_store.nth_player_template(t as i32)).map(|t| t.side().to_vec());
        if let Some(pl) = self.players.get_nth_mut(p) {
            pl.academy_stats.init(frame, side.as_deref());
        }
    }
    fn academy_update(&mut self, p: PlayerIndex) {
        let frame = World::frame(self);
        let mut objects = Vec::new();
        for o in self.player_iterate_objects(p) {
            let Some(kind_of) = self.find_object(o).map(|x| *x.template().kindof()) else { continue };
            let tunnel_contain = self.contain_ask_opt(o, ModuleCall::IsTunnelContain).is_some_and(|r| r.as_bool());
            objects.push(AcademyObject { kind_of, tunnel_contain });
        }
        let Some(pl) = self.players.get_nth_mut(p) else { return };
        let money = Some(pl.money.count_money());
        let has_power = Some(pl.energy.has_sufficient_power(frame));
        pl.academy_stats.update(frame, money, has_power, &objects);
    }
    fn academy_record_income(&mut self, p: PlayerIndex) {
        let frame = World::frame(self);
        if let Some(pl) = self.players.get_nth_mut(p) {
            pl.academy_stats.record_income(frame);
        }
    }
    fn academy_record_generals_points_spent(&mut self, p: PlayerIndex, points: i32) {
        if let Some(pl) = self.players.get_nth_mut(p) {
            pl.academy_stats.record_generals_points_spent(points);
        }
    }
    fn score_reset(&mut self, p: PlayerIndex, player_idx: PlayerIndex) {
        if let Some(pl) = self.players.get_nth_mut(p) {
            pl.score_keeper.reset(player_idx);
        }
    }
    fn score_add_object_built(&mut self, p: PlayerIndex, o: Option<ObjectId>) {
        let scoring = self.game_logic.scoring_enabled;
        let Some(so) = o.and_then(|o| self.scored_object(o)) else { return };
        if let Some(pl) = self.players.get_nth_mut(p) {
            pl.score_keeper.add_object_built(&so, scoring);
        }
    }
    fn score_remove_object_built(&mut self, p: PlayerIndex, o: Option<ObjectId>) {
        let scoring = self.game_logic.scoring_enabled;
        let Some(so) = o.and_then(|o| self.scored_object(o)) else { return };
        if let Some(pl) = self.players.get_nth_mut(p) {
            pl.score_keeper.remove_object_built(&so, scoring);
        }
    }
    fn ai_create(&mut self, p: PlayerIndex, skirmish: bool) {
        self.host_cut("newInstance(AIPlayer)", &[i64::from(p), i64::from(skirmish)], &[]);
        self.cuts.player_host.ai.insert(p, AiPlayerStandIn::default());
    }
    fn ai_delete(&mut self, p: PlayerIndex) {
        self.host_cut("deleteInstance(AIPlayer)", &[i64::from(p)], &[]);
        self.cuts.player_host.ai.remove(&p);
    }
    fn ai_update(&mut self, p: PlayerIndex) {
        self.host_cut("AIPlayer::update", &[i64::from(p)], &[]);
    }
    fn ai_new_map(&mut self, p: PlayerIndex) {
        self.host_cut("AIPlayer::newMap", &[i64::from(p)], &[]);
    }
    fn ai_difficulty(&self, p: PlayerIndex) -> GameDifficulty {
        self.cuts.player_host.ai.get(&p).map_or(GameDifficulty::Normal, |a| a.difficulty)
    }
    fn ai_is_skirmish(&self, p: PlayerIndex) -> bool {
        self.players.get_nth(p).and_then(|x| x.ai).is_some_and(|a| a.skirmish)
    }
    fn ai_compute_superweapon_target(&mut self, p: PlayerIndex, power: SpecialPowerTemplateRef, pos: &mut Coord3D, player_ndx: i32, weapon_radius: f32) -> bool {
        self.host_cut("AIPlayer::computeSuperweaponTarget", &[i64::from(p), i64::from(power), i64::from(player_ndx), i64::from(weapon_radius.to_bits())], &[]);
        match self.cuts.player_host.ai.get(&p).and_then(|a| a.superweapon_target) {
            Some(t) => {
                *pos = t;
                true
            }
            None => false,
        }
    }
    fn ai_enemy(&self, p: PlayerIndex) -> Option<PlayerIndex> {
        self.cuts.player_host.ai.get(&p).and_then(|a| a.enemy)
    }
    fn ai_check_bridges(&mut self, p: PlayerIndex, unit: Option<ObjectId>, way: Option<WaypointId>) -> bool {
        self.host_cut("AIPlayer::checkBridges", &[i64::from(p), i64::from(unit.unwrap_or(0)), i64::from(way.unwrap_or(0))], &[]);
        self.cuts.player_host.ai.get(&p).is_some_and(|a| a.bridges)
    }
    fn ai_base_center(&mut self, p: PlayerIndex, pos: &mut Coord3D) -> bool {
        self.host_cut("AIPlayer::getBaseCenter", &[i64::from(p)], &[]);
        match self.cuts.player_host.ai.get(&p).and_then(|a| a.base_center) {
            Some(c) => {
                *pos = c;
                true
            }
            None => false,
        }
    }
    fn ai_repair_structure(&mut self, p: PlayerIndex, id: ObjectId) {
        self.host_cut("AIPlayer::repairStructure", &[i64::from(p), i64::from(id)], &[]);
    }
    fn ai_on_unit_produced(&mut self, p: PlayerIndex, factory: Option<ObjectId>, unit: Option<ObjectId>) {
        self.host_cut("AIPlayer::onUnitProduced", &[i64::from(p), i64::from(factory.unwrap_or(0)), i64::from(unit.unwrap_or(0))], &[]);
    }
    fn ai_on_structure_produced(&mut self, p: PlayerIndex, builder: Option<ObjectId>, structure: Option<ObjectId>) {
        self.host_cut("AIPlayer::onStructureProduced", &[i64::from(p), i64::from(builder.unwrap_or(0)), i64::from(structure.unwrap_or(0))], &[]);
    }
    fn ai_is_supply_source_safe(&mut self, p: PlayerIndex, min_supplies: i32) -> bool {
        self.host_cut("AIPlayer::isSupplySourceSafe", &[i64::from(p), i64::from(min_supplies)], &[]);
        self.cuts.player_host.ai.get(&p).is_none_or(|a| a.supply_source_safe)
    }
    fn ai_is_supply_source_attacked(&mut self, p: PlayerIndex) -> bool {
        self.host_cut("AIPlayer::isSupplySourceAttacked", &[i64::from(p)], &[]);
        self.cuts.player_host.ai.get(&p).is_some_and(|a| a.supply_source_attacked)
    }
    fn ai_set_team_delay_seconds(&mut self, p: PlayerIndex, delay: i32) {
        self.host_cut("AIPlayer::setTeamDelaySeconds", &[i64::from(p), i64::from(delay)], &[]);
    }
    fn ai_guard_supply_center(&mut self, p: PlayerIndex, team: Option<TeamId>, min_supplies: i32) {
        self.host_cut("AIPlayer::guardSupplyCenter", &[i64::from(p), i64::from(team.unwrap_or(0)), i64::from(min_supplies)], &[]);
    }
    fn ai_pre_team_destroy(&mut self, p: PlayerIndex, team: Option<TeamId>) {
        self.host_cut("AIPlayer::aiPreTeamDestroy", &[i64::from(p), i64::from(team.unwrap_or(0))], &[]);
    }
    fn ai_select_skillset(&mut self, p: PlayerIndex, skill_set: i32) {
        self.host_cut("AIPlayer::selectSkillset", &[i64::from(p), i64::from(skill_set)], &[]);
    }
    fn ai_build_specific_team(&mut self, p: PlayerIndex, proto: Option<TeamPrototypeId>, priority: bool) {
        self.host_cut("AIPlayer::buildSpecificAITeam", &[i64::from(p), i64::from(proto.unwrap_or(0)), i64::from(priority)], &[]);
    }
    fn ai_build_base_defense(&mut self, p: PlayerIndex, flank: bool) {
        self.host_cut("AIPlayer::buildAIBaseDefense", &[i64::from(p), i64::from(flank)], &[]);
    }
    fn ai_build_base_defense_structure(&mut self, p: PlayerIndex, name: &[u8], flank: bool) {
        self.host_cut("AIPlayer::buildAIBaseDefenseStructure", &[i64::from(p), i64::from(flank)], name);
    }
    fn ai_build_specific_building(&mut self, p: PlayerIndex, name: &[u8]) {
        self.host_cut("AIPlayer::buildSpecificAIBuilding", &[i64::from(p)], name);
    }
    fn ai_build_by_supplies(&mut self, p: PlayerIndex, minimum_cash: i32, name: &[u8]) {
        self.host_cut("AIPlayer::buildBySupplies", &[i64::from(p), i64::from(minimum_cash)], name);
    }
    fn ai_build_specific_building_nearest_team(&mut self, p: PlayerIndex, name: &[u8], team: Option<TeamId>) {
        self.host_cut("AIPlayer::buildSpecificBuildingNearestTeam", &[i64::from(p), i64::from(team.unwrap_or(0))], name);
    }
    fn ai_build_upgrade(&mut self, p: PlayerIndex, name: &[u8]) {
        self.host_cut("AIPlayer::buildUpgrade", &[i64::from(p)], name);
    }
    fn ai_recruit_specific_team(&mut self, p: PlayerIndex, proto: Option<TeamPrototypeId>, radius: f32) {
        self.host_cut("AIPlayer::recruitSpecificAITeam", &[i64::from(p), i64::from(proto.unwrap_or(0)), i64::from(radius.to_bits())], &[]);
    }
    fn ai_calc_closest_construction_zone_location(&mut self, p: PlayerIndex, template: TemplateRef, location: &mut Coord3D) -> bool {
        self.host_cut("AIPlayer::calcClosestConstructionZoneLocation", &[i64::from(p), i64::from(template)], &[]);
        match self.cuts.player_host.ai.get(&p).and_then(|a| a.construction_zone) {
            Some(c) => {
                *location = c;
                true
            }
            None => false,
        }
    }
    fn obj_veterancy_level(&self, o: ObjectId) -> i32 {
        self.get_veterancy_level(o) as i32
    }
    fn tmpl_skill_point_value(&self, t: TemplateRef, level: i32) -> i32 {
        let level = match level {
            1 => VeterancyLevel::Veteran,
            2 => VeterancyLevel::Elite,
            3 => VeterancyLevel::Heroic,
            _ => VeterancyLevel::Regular,
        };
        self.tmpl_ref(t).map_or(0, |t| t.get_skill_point_value(level))
    }
    fn obj_orientation(&self, o: ObjectId) -> f32 {
        self.find_object(o).map_or(0.0, |x| x.thing.orientation())
    }
    fn obj_set_script_status_disabled(&mut self, o: ObjectId, set: bool) {
        self.set_script_status(o, crate::logic::object_script_status_bits::OBJECT_STATUS_SCRIPT_DISABLED, set);
    }
    fn obj_set_disabled_underpowered(&mut self, o: ObjectId) {
        self.set_disabled(o, DISABLED_UNDERPOWERED);
    }
    fn obj_clear_disabled_underpowered(&mut self, o: ObjectId) {
        self.clear_disabled(o, DISABLED_UNDERPOWERED);
    }
    fn obj_special_power_modules(&mut self, o: ObjectId) -> Vec<SpecialPowerModuleRef> {
        self.special_power_module_indices(o).into_iter().filter_map(|i| sp_ref(o, i)).collect()
    }
    fn sp_required_science(&mut self, m: SpecialPowerModuleRef) -> ScienceType {
        ScienceType(self.sp_ask(m, ModuleCall::GetRequiredScience).as_int(-1))
    }
    fn sp_on_special_power_creation(&mut self, m: SpecialPowerModuleRef) {
        self.sp_ask(m, ModuleCall::OnSpecialPowerCreation);
    }
    fn sp_set_ready_frame(&mut self, m: SpecialPowerModuleRef, frame: u32) {
        self.sp_ask(m, ModuleCall::SetReadyFrame { frame });
    }
    fn special_power_template(&self, t: SpecialPowerTemplateRef) -> SpecialPowerView {
        let s = self.statics.special_power_store.find_special_power_template_by_id(t).expect("a special power template the store gave out");
        SpecialPowerView { id: s.id(), reload_time: s.reload_time(), shared_n_sync: s.is_shared_n_sync() }
    }
    fn partition_distance_squared_2d(&self, a: ObjectId, b: ObjectId) -> f32 {
        self.partition.get_distance_squared(self, a, b, DistanceCalculationType::FromCenter2D).map_or(0.0, |(d, _)| d)
    }
}

impl PlayerObjectHost for World {
    fn find_object_by_id(&self, id: ObjectId) -> Option<ObjectId> {
        self.find_object(id).map(|o| o.id)
    }
    fn obj_friend_adjust_power_for_player(&mut self, o: ObjectId, yes: bool) {
        self.friend_adjust_power_for_player(o, yes);
    }
    fn obj_has_auto_deposit_update(&mut self, o: ObjectId, key: NameKeyType) -> bool {
        // `findUpdateModule(key)`: the module's name, as its key
        let name = self.names.key_to_name(key).to_vec();
        self.find_module(o, &name).is_some()
    }
    fn auto_deposit_award_initial_capture_bonus(&mut self, o: ObjectId, p: PlayerIndex) {
        if let Some(idx) = self.find_module(o, b"AutoDepositUpdate") {
            self.module_call(o, idx, ModuleCall::AwardInitialCaptureBonus { player: p });
        }
    }
    fn obj_modules_ready(&self, o: ObjectId) -> bool {
        self.find_object(o).is_some_and(|x| x.modules_ready)
    }
    fn ui_add_idle_worker(&mut self, o: ObjectId) {
        self.host_cut("InGameUI::addIdleWorker", &[i64::from(o)], &[]);
    }
    fn ui_remove_idle_worker(&mut self, o: ObjectId, player: PlayerIndex) {
        self.host_cut("InGameUI::removeIdleWorker", &[i64::from(o), i64::from(player)], &[]);
    }
    fn tmpl_default_owning_side(&self, t: TemplateRef) -> Vec<u8> {
        self.tmpl_ref(t).map(|t| t.get_default_owning_side().to_vec()).unwrap_or_default()
    }
    fn obj_has_drawable(&self, o: ObjectId) -> bool {
        self.has_drawable(o)
    }
    fn obj_stealth_disguised_player_index(&mut self, o: ObjectId) -> Option<PlayerIndex> {
        let idx = self.find_object(o)?.stealth?;
        match self.module_call(o, idx, ModuleCall::StealthGetDisguisedPlayerIndex) {
            ModuleReply::Int(p) => Some(p),
            _ => None,
        }
    }
    fn obj_night_indicator_color(&self, o: ObjectId) -> u32 {
        self.get_night_indicator_color(o) as u32
    }
    fn obj_indicator_color(&self, o: ObjectId) -> u32 {
        self.get_indicator_color(o) as u32
    }
    fn obj_is_disabled(&self, o: ObjectId) -> bool {
        self.find_object(o).is_some_and(|x| x.is_disabled())
    }
    fn obj_has_any_special_power(&mut self, o: ObjectId) -> bool {
        self.find_object(o).is_some_and(|x| x.has_any_special_power())
    }
    fn obj_find_any_shortcut_special_power(&mut self, o: ObjectId) -> Option<SpecialPowerModuleRef> {
        let idx = self.find_any_shortcut_special_power_module_interface(o)?;
        sp_ref(o, idx)
    }
    fn obj_has_special_power(&mut self, o: ObjectId, t: SpecialPowerType) -> bool {
        self.find_object(o).is_some_and(|x| x.has_special_power(t))
    }
    fn obj_find_special_power(&mut self, o: ObjectId, t: SpecialPowerType) -> Option<SpecialPowerModuleRef> {
        let idx = self.find_special_power_module_interface(o, t)?;
        sp_ref(o, idx)
    }
    fn sp_is_script_only(&mut self, m: SpecialPowerModuleRef) -> bool {
        self.sp_ask(m, ModuleCall::IsScriptOnly).as_bool()
    }
    fn sp_ready_frame(&mut self, m: SpecialPowerModuleRef) -> u32 {
        self.sp_ask(m, ModuleCall::GetReadyFrame).as_int(0) as u32
    }
    fn sp_percent_ready(&mut self, m: SpecialPowerModuleRef) -> u32 {
        // `UnsignedInt percentage = sp->getPercentReady();`: the fraction, truncated (0 until ready, then 1)
        self.sp_ask(m, ModuleCall::GetPercentReady).as_real(0.0) as u32
    }
    fn sp_is_shared_n_sync(&mut self, m: SpecialPowerModuleRef) -> bool {
        match self.sp_ask(m, ModuleCall::GetSpecialPowerTemplateId) {
            ModuleReply::Int(id) => self.statics.special_power_store.find_special_power_template_by_id(id as u32).is_some_and(|t| t.is_shared_n_sync()),
            _ => false,
        }
    }
    fn obj_has_any_weapon(&self, o: ObjectId) -> bool {
        crate::logic::world::object_weapons::has_any_weapon(self, o)
    }
    fn obj_most_percent_ready_to_fire_any_weapon(&mut self, o: ObjectId) -> u32 {
        crate::logic::world::object_weapons::get_most_percent_ready_to_fire_any_weapon(self, o)
    }
    fn obj_leave_group(&mut self, o: ObjectId) {
        self.leave_group(o);
    }
    fn obj_is_any_kind_of(&self, o: ObjectId, mask: &KindOfMaskType) -> bool {
        self.find_object(o).is_some_and(|x| x.is_any_kind_of(mask))
    }
    fn ai_hunt(&mut self, o: ObjectId, source: CommandSource) {
        self.ai_ask(o, ModuleCall::AiHunt { command_source: source });
    }
    fn ai_idle(&mut self, o: ObjectId, source: CommandSource) {
        self.ai_ask(o, ModuleCall::AiIdleFrom { command_source: source });
    }
    fn ai_enter(&mut self, o: ObjectId, building: ObjectId, source: CommandSource) {
        self.ai_ask(o, ModuleCall::AiEnter { target: building, command_source: source });
    }
    fn ai_evacuate(&mut self, o: ObjectId, expose_stealth_units: bool, source: CommandSource) {
        self.ai_ask(o, ModuleCall::AiEvacuate { expose_stealth_units, command_source: source });
    }
    fn ai_move_to_position(&mut self, o: ObjectId, pos: Coord3D, source: CommandSource) {
        self.ai_ask(o, ModuleCall::AiMoveToPosition { pos, command_source: source });
    }
    fn ai_has_supply_truck_interface(&self, o: ObjectId) -> bool {
        self.cuts.supply_truck_ais.contains(&o)
    }
    fn ai_supply_truck_set_force_wanting_state(&mut self, o: ObjectId, v: bool) {
        self.ai_ask(o, ModuleCall::SupplyTruckSetForceWantingState(v));
    }
    fn pathfinder_remove_object(&mut self, o: ObjectId) {
        self.contain_pathfind_remove_object(o);
    }
    fn pathfinder_add_object(&mut self, o: ObjectId) {
        self.contain_pathfind_add_object(o);
    }
    fn partition_get_most_valuable_location(&mut self, p: PlayerIndex) {
        // (the answer is not used: the call has no effect)
        self.host_cut("PartitionManager::getMostValuableLocation", &[i64::from(p)], &[]);
        if self.cuts.live_partition {
            let _ = self.partition.get_most_valuable_location(self, p, ALLOW_ENEMIES as u32, ValueOrThreat::CashValue);
        }
    }
    fn partition_structures_near_to_far(&mut self, pos: Coord3D) -> Vec<ObjectId> {
        if !self.cuts.live_partition {
            self.host_cut("PartitionManager::iterateObjectsInRange(STRUCTURE)", &[], &[]);
            return self.cuts.range_scan.clone();
        }
        let filters = [PartitionFilter::AcceptByKindOf { must_be_set: KindOfMaskType::from_indices(&[KINDOF_STRUCTURE]), must_be_clear: KindOfMaskType::new() }];
        let it = self.with_partition(|pm, w| pm.iterate_objects_in_range_at(w, &pos, 1e9, DistanceCalculationType::FromCenter3D, &filters, IterOrderType::SortedNearToFar));
        iterator_ids(it)
    }
    fn partition_all_objects(&mut self) -> Vec<ObjectId> {
        if !self.cuts.live_partition {
            // (the game logic's list order stands in for the partition manager's)
            self.host_cut("PartitionManager::iterateAllObjects", &[], &[]);
            return self.objects.ids_in_order();
        }
        let it = self.with_partition(|pm, w| pm.iterate_all_objects(w, &[]));
        iterator_ids(it)
    }
    fn contain_player_who_entered(&mut self, building: ObjectId) -> Option<u32> {
        self.contain_ask_opt(building, ModuleCall::GetPlayerWhoEntered).map(|r| r.as_int(0) as u32)
    }
    fn contain_recalc_apparent_controlling_player(&mut self, o: ObjectId) {
        self.contain_ask_opt(o, ModuleCall::RecalcApparentControllingPlayer);
    }
    fn obj_has_contain(&self, o: ObjectId) -> bool {
        self.get_contain(o).is_some()
    }
    fn action_can_enter_object(&mut self, o: ObjectId, building: ObjectId, source: CommandSource) -> bool {
        let source = CommandSourceType::from_i32(source);
        // (an error is upstream's undefined behaviour: recorded, and answered no)
        match crate::logic::action_manager::can_enter_object(self, Some(o), Some(building), source, crate::logic::action_manager::CanEnterType::CheckCapacity) {
            Ok(b) => b,
            Err(e) => {
                self.host_cut("ActionManager::canEnterObject (undefined)", &[i64::from(o), i64::from(building)], format!("{e:?}").as_bytes());
                false
            }
        }
    }
    fn build_assistant_sell_object(&mut self, o: ObjectId) {
        // batch I6: the world's own build assistant
        self.build_assistant_sell(o);
    }
    fn obj_is_faction_structure(&self, o: ObjectId) -> bool {
        self.is_faction_structure(o)
    }
    fn thing_factory_find_template(&mut self, name: &[u8]) -> Option<TemplateRef> {
        World::thing_factory_find_template(self, name).map(TemplateRef::from)
    }
    fn score_add_money_spent(&mut self, p: PlayerIndex, amount: i32) {
        if let Some(pl) = self.players.get_nth_mut(p) {
            pl.score_keeper.add_money_spent(amount);
        }
    }
    fn score_add_money_earned(&mut self, p: PlayerIndex, amount: i32) {
        if let Some(pl) = self.players.get_nth_mut(p) {
            pl.score_keeper.add_money_earned(amount);
        }
    }
    fn tmpl_build_cost(&self, t: TemplateRef) -> i32 {
        self.tmpl_ref(t).map_or(0, |t| t.friend_get_build_cost())
    }
    fn tmpl_kind_of_mask(&self, t: TemplateRef) -> KindOfMaskType {
        self.tmpl_ref(t).map(|t| *t.kindof()).unwrap_or_default()
    }
    fn obj_producer_id(&self, o: ObjectId) -> ObjectId {
        self.find_object(o).map_or(INVALID_ID, |x| x.producer_id)
    }
    fn obj_body_max_health(&mut self, o: ObjectId) -> f32 {
        let Some(b) = self.get_body_module(o) else { return 0.0 };
        self.module_call(o, b, ModuleCall::GetMaxHealth).as_real(0.0)
    }
    fn obj_body_set_max_health(&mut self, o: ObjectId, health: f32) {
        if let Some(b) = self.get_body_module(o) {
            self.module_call(o, b, ModuleCall::SetMaxHealth(health));
        }
    }
    fn obj_body_apply_damage_scalar(&mut self, o: ObjectId, scalar: f32) {
        if let Some(b) = self.get_body_module(o) {
            self.module_call(o, b, ModuleCall::ApplyDamageScalar(scalar));
        }
    }
    fn obj_set_weapon_bonus_condition(&mut self, o: ObjectId, c: i32) {
        self.set_weapon_bonus_condition(o, c);
    }
    fn obj_clear_weapon_bonus_condition(&mut self, o: ObjectId, c: i32) {
        self.clear_weapon_bonus_condition(o, c);
    }
    fn obj_shroud_clearing_range(&self, o: ObjectId) -> f32 {
        self.get_shroud_clearing_range(o)
    }
    fn obj_set_vision_range(&mut self, o: ObjectId, range: f32) {
        self.set_vision_range(o, range);
    }
    fn obj_set_shroud_clearing_range(&mut self, o: ObjectId, range: f32) {
        self.set_shroud_clearing_range(o, range);
    }
    fn obj_set_vision_spied(&mut self, o: ObjectId, setting: bool, by_whom: PlayerIndex) {
        self.set_vision_spied(o, setting, by_whom);
    }
    fn tmpl_buildable(&self, t: TemplateRef) -> u8 {
        self.tmpl_ref(t).map_or(0, |t| t.buildable())
    }
    fn tmpl_prereq_count(&self, t: TemplateRef) -> i32 {
        self.tmpl_ref(t).map_or(0, |t| t.get_prereq_count())
    }
    fn tmpl_prereq_is_satisfied(&self, t: TemplateRef, i: i32, p: PlayerIndex) -> bool {
        self.prereq_is_satisfied(t, i, p)
    }
    fn tmpl_max_simultaneous_of_type(&self, t: TemplateRef) -> u32 {
        let restriction = u32::from(self.game_logic.superweapon_restriction);
        self.tmpl_ref(t).map_or(0, |t| t.get_max_simultaneous_of_type(Some(restriction)))
    }
    fn tmpl_max_simultaneous_link_key(&self, t: TemplateRef) -> NameKeyType {
        self.tmpl_ref(t).map_or(NameKeyType(0), |t| t.get_max_simultaneous_link_key())
    }
    fn obj_production_queue_count(&mut self, o: ObjectId, t: TemplateRef) -> Option<u32> {
        let idx = self.get_production_update_interface(o)?;
        let template = u16::try_from(t).ok();
        Some(self.module_call(o, idx, ModuleCall::ProductionCountUnitTypeInQueue { template }).as_int(0) as u32)
    }
    fn player_template_find(&mut self, key: NameKeyType) -> Option<PlayerTemplateRef> {
        // `findPlayerTemplate`: its fifteen statics are looked up on the first call (making the keys); an old map's
        // sub-faction (a1..a4, c1..c4, g1..g4) finds its base faction (`PlayerTemplateStore::find_index`)
        let keys = crate::common::player_template::LEGACY_FACTION_KEY_NAMES.map(|n| self.names.name_to_key(n.as_bytes()));
        let mut key = key;
        for group in 0..3 {
            let base = group * 5;
            if (1..=4).any(|k| key == keys[base + k]) {
                key = keys[base];
                break;
            }
        }
        self.statics.player_template_store.templates().iter().position(|t| t.name_key() == key).map(|i| i as PlayerTemplateRef)
    }
}

impl PlayerSquadHost for World {
    fn upgrade_mask(&self, t: NameKeyType) -> UpgradeMaskType {
        self.statics.upgrade_center.find_upgrade_by_key(t).map(|u| u.upgrade_mask()).unwrap_or_default()
    }
    fn obj_update_upgrade_modules(&mut self, o: ObjectId) {
        self.update_upgrade_modules(o);
    }
    fn obj_is_selectable(&self, o: ObjectId) -> bool {
        self.is_selectable(o)
    }
    fn ai_group_all_ids(&self, group: GroupRef) -> Vec<ObjectId> {
        self.cuts.player_host.groups.get(&group).cloned().unwrap_or_default()
    }
    fn message_argument_count(&self, msg: MessageRef) -> u8 {
        self.cuts.player_host.messages.get(&msg).map_or(0, |m| m.len().min(255) as u8)
    }
    fn message_argument_object_id(&self, msg: MessageRef, i: u8) -> ObjectId {
        self.cuts.player_host.messages.get(&msg).and_then(|m| m.get(usize::from(i)).copied()).unwrap_or(INVALID_ID)
    }
    fn academy_record_control_groups_used(&mut self, p: PlayerIndex) {
        if let Some(pl) = self.players.get_nth_mut(p) {
            pl.academy_stats.record_control_groups_used();
        }
    }
}

impl PlayerXferHost for World {
    fn upgrade_center_and_names(&mut self) -> (&UpgradeCenter, &mut NameKeyGenerator) {
        (&self.statics.upgrade_center, &mut self.names)
    }
    fn xfer_score_keeper(&mut self, p: PlayerIndex, xfer: &mut dyn Xfer) -> PResult<()> {
        let factory = &self.statics.thing_factory;
        let Some(pl) = self.players.get_nth_mut(p) else { return Ok(()) };
        pl.score_keeper.xfer(xfer, &mut |name: &[u8]| factory.find_template(name).is_some())?;
        Ok(())
    }
    fn xfer_ai(&mut self, p: PlayerIndex, xfer: &mut dyn Xfer) -> PResult<()> {
        // the AI player is not in the world yet (batch I9): its version only
        self.host_cut("AIPlayer::xfer", &[i64::from(p)], &[]);
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)?;
        Ok(())
    }
}

impl PlayerInitHost for World {
    fn sides_num_sides(&self) -> i32 {
        self.sides.get_num_sides()
    }
    fn sides_side_dict(&self, i: i32) -> Dict {
        self.sides.get_side_info(i).map(|s| s.dict.clone()).unwrap_or_default()
    }
    fn sides_side_scripts(&self, i: i32) -> Option<ScriptListRef> {
        self.cuts.player_host.side_scripts.get(&i).copied()
    }
    fn sides_set_side_scripts(&mut self, i: i32, l: Option<ScriptListRef>) {
        match l {
            Some(l) => self.cuts.player_host.side_scripts.insert(i, l),
            None => self.cuts.player_host.side_scripts.remove(&i),
        };
    }
    fn sides_take_side_build_list(&mut self, i: i32) -> Vec<BuildListInfo> {
        self.sides.get_side_info_mut(i).map(|s| std::mem::take(&mut s.build_list)).unwrap_or_default()
    }
    fn sides_num_skirmish_sides(&self) -> i32 {
        self.sides.get_num_skirmish_sides()
    }
    fn sides_skirmish_side_dict(&self, i: i32) -> Dict {
        self.sides.get_skirmish_side_info(i).map(|s| s.dict.clone()).unwrap_or_default()
    }
    fn sides_skirmish_side_scripts(&self, i: i32) -> Option<ScriptListRef> {
        self.cuts.player_host.skirmish_side_scripts.get(&i).copied()
    }
    fn sides_set_skirmish_side_scripts(&mut self, i: i32, l: Option<ScriptListRef>) {
        match l {
            Some(l) => self.cuts.player_host.skirmish_side_scripts.insert(i, l),
            None => self.cuts.player_host.skirmish_side_scripts.remove(&i),
        };
    }
    fn sides_num_teams(&self) -> i32 {
        self.sides.get_num_teams()
    }
    fn sides_team_dict(&self, i: i32) -> Dict {
        self.sides.get_team_info(i).map(|t| t.dict.clone()).unwrap_or_default()
    }
    fn sides_remove_team(&mut self, i: i32) {
        self.sides.remove_team(i);
    }
    fn sides_num_skirmish_teams(&self) -> i32 {
        self.sides.get_num_skirmish_teams()
    }
    fn sides_skirmish_team_dict(&self, i: i32) -> Dict {
        self.sides.get_skirmish_team_info(i).map(|t| t.dict.clone()).unwrap_or_default()
    }
    fn sides_find_team_info(&mut self, name: &[u8]) -> bool {
        let mut names = crate::logic::sides_list::MapNames::new(std::mem::take(&mut self.names));
        let found = self.sides.find_team_info(&mut names, name).is_some();
        self.names = names.generator;
        found
    }
    fn sides_add_team(&mut self, d: &Dict) {
        self.sides.add_team(d);
    }
    fn script_list_duplicate_and_qualify(&mut self, list: ScriptListRef, qualifier: &[u8], qual_template_player_name: &[u8], new_player_name: &[u8]) -> ScriptListRef {
        let mut text = qualifier.to_vec();
        for t in [qual_template_player_name, new_player_name] {
            text.push(b' ');
            text.extend_from_slice(t);
        }
        self.host_cut("ScriptList::duplicateAndQualify", &[i64::from(list)], &text);
        self.cuts.player_host.script_lists_given += 1;
        self.cuts.player_host.script_lists_given
    }
    fn script_list_delete(&mut self, list: ScriptListRef) {
        self.host_cut("deleteInstance(ScriptList)", &[i64::from(list)], &[]);
    }
    fn script_engine_global_difficulty(&self) -> GameDifficulty {
        self.cuts.player_host.global_difficulty
    }
    fn ai_set_difficulty(&mut self, p: PlayerIndex, d: GameDifficulty) {
        self.host_cut("AIPlayer::setAIDifficulty", &[i64::from(p), d as i64], &[]);
        if let Some(a) = self.cuts.player_host.ai.get_mut(&p) {
            a.difficulty = d;
        }
    }
    fn network_present(&self) -> bool {
        self.cuts.player_host.network_present
    }
    fn team_factory_init_from_sides(&mut self) -> PResult<()> {
        let sides = std::mem::take(&mut self.sides);
        let r = crate::logic::world::team_ops::factory_init_from_sides(self, &sides);
        self.sides = sides;
        r?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    //! The world as the team and player host: the team and player code itself is checked against the compiled C++ by
    //! `tests/differential_team.rs` and `tests/differential_player.rs` with stand-in hosts; these check that the world's
    //! answers are its own objects, templates, modules and stores, and that the calls into units it does not hold yet are
    //! logged and answered from [`PlayerHostCuts`].
    use super::*;
    use crate::common::base_type::ICoord2D;
    use crate::common::damage_fx::DamageFxStore;
    use crate::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
    use crate::common::module_factory::ModuleFactory;
    use crate::common::object_status_types::ObjectStatusMaskType;
    use crate::common::science::{GameTextFetch, ScienceStore};
    use crate::common::thing_factory::ThingFactory;
    use crate::common::thing_template::ThingParseEnv;
    use crate::common::kind_of::KINDOF_VEHICLE;
    use crate::logic::armor::ArmorStore;
    use crate::logic::terrain_logic::{GroundSource, TerrainLogic};
    use crate::logic::terrain_visual::LogicHeightMap;
    use crate::logic::weapon_set::WeaponTemplateTable;
    use crate::logic::world::statics::GameStatics;
    use crate::logic::world::team_ops::{factory_find_team, factory_init_team};
    use std::sync::Arc;

    struct Text;
    impl GameTextFetch for Text {
        fn fetch(&self, _label: &[u8]) -> UnicodeString {
            UnicodeString::new()
        }
    }
    struct Ctx {
        factory: ThingFactory,
        names: NameKeyGenerator,
        modules: ModuleFactory,
        armors: ArmorStore,
        dfx: DamageFxStore,
        weapons: WeaponTemplateTable,
        sciences: ScienceStore,
        player_templates: crate::common::player_template::PlayerTemplateStore,
        /// batch I9c-1: the fixture's locomotors (the `Locomotor` blocks, read before the objects that name them)
        locomotors: crate::logic::locomotor::LocomotorStore,
    }
    fn locomotor_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.locomotors.parse_locomotor_template_definition(ini, &mut c.names)
    }
    fn player_template_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.player_templates.parse_player_template_definition(ini, &mut c.names, &c.sciences, &Text)
    }
    fn object_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        let mut env = ThingParseEnv {
            names: &mut c.names,
            modules: &mut c.modules,
            armors: &c.armors,
            damage_fx: &c.dfx,
            weapons: &c.weapons,
            sciences: &c.sciences,
            text: &Text,
            locomotors: &c.locomotors,
        };
        c.factory.parse_object_block(ini, &mut env, 0)
    }

    /// A tank that needs a barracks, a barracks, a garrisonable bunker, a wall piece and an immobile depot (batch I9a), a scout with an `AIUpdateInterface`
    /// (batch I9b-3), and two player templates (America,
    /// the civilians).
    const INI: &str = "Object Tank\n KindOf = VEHICLE\n BuildCost = 900\n Prerequisites\n  Object = Barracks\n End\n\
                       \x20Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
                       \x20Body = ActiveBody ModuleTag_Body\n  MaxHealth = 200\n  InitialHealth = 200\n End\nEnd\n\
                       Object Barracks\n KindOf = STRUCTURE\n BuildCost = 600\n Geometry = BOX\n GeometryMajorRadius = 20\n GeometryMinorRadius = 20\n GeometryHeight = 10\n\
                       \x20Body = ActiveBody ModuleTag_Body\n  MaxHealth = 1000\n  InitialHealth = 1000\n End\nEnd\n\
                       Object Bunker\n KindOf = STRUCTURE\n Geometry = BOX\n GeometryMajorRadius = 10\n GeometryMinorRadius = 10\n GeometryHeight = 10\n\
                       \x20Body = ActiveBody ModuleTag_Body\n  MaxHealth = 500\n  InitialHealth = 500\n End\n\
                       \x20Behavior = GarrisonContain ModuleTag_Contain\n  ContainMax = 5\n End\nEnd\n\
                       Object Wall\n KindOf = STRUCTURE IMMOBILE WALK_ON_TOP_OF_WALL\n Geometry = BOX\n GeometryMajorRadius = 30\n GeometryMinorRadius = 25\n GeometryHeight = 10\n\
                       \x20Body = ActiveBody ModuleTag_Body\n  MaxHealth = 500\n  InitialHealth = 500\n End\nEnd\n\
                       Object Scout\n KindOf = VEHICLE\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
                       \x20Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
                       \x20Behavior = AIUpdateInterface ModuleTag_AI\n End\nEnd\n\
                       Object Dozer\n KindOf = VEHICLE DOZER\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
                       \x20Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
                       \x20Behavior = DozerAIUpdate ModuleTag_AI\n  RepairHealthPercentPerSecond = 2%\n  BoredTime = 5000\n  BoredRange = 150\n End\nEnd\n\
                       Locomotor RunnerLoco\n Surfaces = GROUND\n Speed = 30\n Acceleration = 100\n Braking = 100\n TurnRate = 360\n Appearance = TREADS\nEnd\n\
                       Object Runner\n KindOf = VEHICLE\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
                       \x20Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
                       \x20Behavior = AIUpdateInterface ModuleTag_AI\n End\n\
                       \x20Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 50\n End\n Locomotor = SET_NORMAL RunnerLoco\nEnd\n\
                       Object Gun\n KindOf = VEHICLE\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
                       \x20Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
                       \x20Behavior = AIUpdateInterface ModuleTag_AI\n  Turret\n   TurretTurnRate = 60\n   ControlledWeaponSlots = PRIMARY\n   NaturalTurretAngle = 0\n  End\n End\nEnd\n\
                       Object Infantry\n KindOf = INFANTRY\n Geometry = CYLINDER\n GeometryMajorRadius = 2\n GeometryHeight = 5\n\
                       \x20Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
                       \x20Behavior = AIUpdateInterface ModuleTag_AI\n End\n\
                       \x20Behavior = ProneUpdate ModuleTag_Prone\n  DamageToFramesRatio = 2\n End\nEnd\n\
                       Object Wanderer\n KindOf = INFANTRY\n Geometry = CYLINDER\n GeometryMajorRadius = 2\n GeometryHeight = 5\n\
                       \x20Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
                       \x20Behavior = WanderAIUpdate ModuleTag_AI\n End\nEnd\n\
                       Object Depot\n KindOf = STRUCTURE IMMOBILE\n Geometry = BOX\n GeometryMajorRadius = 20\n GeometryMinorRadius = 20\n GeometryHeight = 10\n\
                       \x20Body = ActiveBody ModuleTag_Body\n  MaxHealth = 1000\n  InitialHealth = 1000\n End\nEnd\n\
                       PlayerTemplate FactionAmerica\n Side = America\n StartingBuilding = Barracks\n StartingUnit0 = Tank\nEnd\nPlayerTemplate FactionCivilian\n Side = Civilian\nEnd\n";

    /// A world over a flat 40 x 40 height map (400 x 400) with the templates of [`INI`], two players (the neutral player and `Player1`), the singleton team `team1`
    /// of `Player1` made by the team factory on the world, the cut log on and the partition manager initialised and live.
    pub(crate) fn world() -> (World, TeamId) {
        let mut names = NameKeyGenerator::new();
        names.reset();
        let mut modules = ModuleFactory::new();
        modules.init(&mut names).unwrap();
        let mut c = Ctx {
            factory: ThingFactory::new(),
            names,
            modules,
            armors: ArmorStore::new(),
            dfx: DamageFxStore::new(),
            weapons: WeaponTemplateTable::new(),
            sciences: ScienceStore::new(),
            player_templates: Default::default(),
            locomotors: crate::logic::locomotor::LocomotorStore::new(),
        };
        static BLOCKS: [BlockParse<Ctx>; 3] = [
            BlockParse { token: "Object", parse: object_block },
            BlockParse { token: "PlayerTemplate", parse: player_template_block },
            BlockParse { token: "Locomotor", parse: locomotor_block },
        ];
        Ini::load(b"Data\\INI\\Test.ini", INI.as_bytes(), IniLoadType::Overwrite, None, &BLOCKS, &mut c).expect("load");
        c.factory.post_process_load();

        let mut w = World::new();
        w.names = c.names;
        let mut st =
            GameStatics { thing_factory: c.factory, player_template_store: c.player_templates, locomotor_store: c.locomotors, ..Default::default() };
        let g = st.global_data.create_original();
        g.partition_cell_size = 40.0;
        w.statics = Arc::new(st);
        let mut t = TerrainLogic::new(GroundSource::LogicHeightMap);
        let size = ICoord2D { x: 40, y: 40 };
        t.set_logic_height_map(Some(LogicHeightMap::new(40, 40, 0, vec![size], vec![0; 1600]).unwrap()));
        t.set_boundaries(vec![size], 0);
        w.terrain = t;
        w.players.set_player_count(2);
        let key = w.names.name_to_key(b"Player1");
        w.players.get_nth_mut(1).unwrap().name_key = key.0 as i32;
        factory_init_team(&mut w, b"team1", b"Player1", true, &Dict::new()).expect("team");
        let team = factory_find_team(&mut w, b"team1").expect("find").expect("team1");
        w.cuts.log = Some(Vec::new());
        w.partition_init();
        w.cuts.live_partition = true;
        (w, team)
    }

    /// A new object of the template `name` on `team` at `pos`, a member of the team in the team arena (`setTeam` does
    /// that with `cuts.live_teams`, batch I3b) and registered with the partition manager.
    pub(crate) fn make(w: &mut World, name: &[u8], team: TeamId, pos: Coord3D) -> ObjectId {
        let t = World::thing_factory_find_template(w, name).expect("template");
        let id = w.thing_factory_new_object(t, Some(team), ObjectStatusMaskType::default()).expect("object");
        w.set_position(id, &pos);
        if !w.teams.is_in_member_list(team, id) {
            w.teams.prepend_member(team, id);
        }
        w.partition_register_object(id);
        w.partition_update();
        id
    }

    fn hosts(w: &World) -> Vec<&'static str> {
        w.cuts.log.as_ref().unwrap().iter().filter_map(|c| if let CutCall::Host { call, .. } = c { Some(*call) } else { None }).collect()
    }

    const AT: Coord3D = Coord3D { x: 100.0, y: 100.0, z: 0.0 };

    /// The team factory runs on the world: `team1` is a prototype of `Player1`, whose team list has it; the team's
    /// controlling player is player 1.
    #[test]
    fn the_team_factory_makes_its_teams_on_the_world() {
        let (w, team) = world();
        let proto = w.teams.team(team).expect("the team").proto;
        assert_eq!(w.players.get_nth(1).unwrap().team_prototypes, vec![proto]);
        assert_eq!(crate::logic::world::team_ops::team_get_controlling_player(&w, team).unwrap(), Some(1));
    }

    /// A player's objects are its teams' members: `countObjectsByThingTemplate` counts the world's objects, and the
    /// tank's prerequisite (a barracks) is satisfied once the player has one.
    #[test]
    fn a_players_objects_are_counted_and_satisfy_prerequisites() {
        let (mut w, team) = world();
        let tank = World::thing_factory_find_template(&w, b"Tank").unwrap();
        let barracks = World::thing_factory_find_template(&w, b"Barracks").unwrap();
        make(&mut w, b"Tank", team, AT);
        make(&mut w, b"Tank", team, Coord3D { x: 150.0, ..AT });
        let mut counts = [0; 2];
        player_count_objects_by_thing_template(&w, 1, &[Some(TemplateRef::from(tank)), Some(TemplateRef::from(barracks))], false, &mut counts, true).unwrap();
        assert_eq!(counts, [2, 0]);
        assert_eq!(w.tmpl_prereq_count(TemplateRef::from(tank)), 1);
        assert!(!w.tmpl_prereq_is_satisfied(TemplateRef::from(tank), 0, 1));
        make(&mut w, b"Barracks", team, Coord3D { x: 200.0, ..AT });
        assert!(w.tmpl_prereq_is_satisfied(TemplateRef::from(tank), 0, 1));
        // the neutral player has none
        assert!(!w.tmpl_prereq_is_satisfied(TemplateRef::from(tank), 0, 0));
    }

    /// The object and template questions are the world's objects and templates; a body module answers the health, a
    /// contain module the count.
    #[test]
    fn object_and_template_questions_reach_the_objects_and_modules() {
        let (mut w, team) = world();
        let t = make(&mut w, b"Tank", team, AT);
        let b = make(&mut w, b"Bunker", team, Coord3D { x: 200.0, ..AT });
        let tank = w.obj_template(t).unwrap();
        assert_eq!(w.tmpl_name(tank), b"Tank");
        assert!(w.tmpl_is_kind_of(tank, KINDOF_VEHICLE) && w.obj_is_kind_of(t, KINDOF_VEHICLE) && !w.obj_is_kind_of(b, KINDOF_VEHICLE));
        assert_eq!(w.tmpl_build_cost(tank), 900);
        assert!(w.tmpl_is_equivalent_to(tank, Some(tank)) && !w.tmpl_is_equivalent_to(tank, w.obj_template(b)) && !w.tmpl_is_equivalent_to(tank, None));
        assert_eq!(w.obj_position(t), AT);
        assert_eq!(w.obj_team(t), Some(team));
        assert_eq!(w.obj_body_max_health(t), 200.0);
        assert_eq!(w.obj_contain_count(t), None);
        assert_eq!(w.obj_contain_count(b), Some(0));
        assert!(w.obj_has_contain(b) && !w.obj_has_contain(t));
    }

    /// With the partition manager live its three questions are its own: every registered object, and the structures from
    /// nearest to farthest; without it they are cut points.
    #[test]
    fn the_partition_questions_are_the_partition_managers_when_live() {
        let (mut w, team) = world();
        let near = make(&mut w, b"Barracks", team, Coord3D { x: 60.0, y: 60.0, z: 0.0 });
        let far = make(&mut w, b"Bunker", team, Coord3D { x: 300.0, y: 300.0, z: 0.0 });
        let tank = make(&mut w, b"Tank", team, AT);
        assert_eq!(w.partition_structures_near_to_far(Coord3D { x: 50.0, y: 50.0, z: 50.0 }), vec![near, far]);
        let mut all = w.partition_all_objects();
        all.sort();
        assert_eq!(all, vec![near, far, tank]);
        w.cuts.live_partition = false;
        w.cuts.range_scan = vec![far];
        assert_eq!(w.partition_structures_near_to_far(AT), vec![far]);
        assert!(hosts(&w).contains(&"PartitionManager::iterateObjectsInRange(STRUCTURE)"));
    }

    /// The score keeper and the academy statistics are the player's own.
    #[test]
    fn the_score_keeper_and_academy_are_the_players() {
        let (mut w, _) = world();
        w.score_add_money_spent(1, 300);
        w.score_add_money_earned(1, 50);
        let sk = &w.players.get_nth(1).unwrap().score_keeper;
        assert_eq!((sk.total_money_spent(), sk.total_money_earned()), (300, 50));
        assert_eq!(w.players.get_nth(0).unwrap().score_keeper.total_money_spent(), 0);
    }

    /// The script engine and the AI player are cut points: each call is logged with upstream's name, and answered from
    /// the tables (a script's conditions in order, the last repeating; an AI's base centre).
    #[test]
    fn calls_into_units_the_world_lacks_are_logged_and_answered_from_the_tables() {
        let (mut w, _) = world();
        w.cuts.player_host.scripts.push(ScriptStandIn { name: b"Attack".to_vec(), conditions: vec![false, true], ..Default::default() });
        let s = w.script_find_by_name(b"Attack").expect("the script");
        assert_eq!(w.script_find_by_name(b"Defend"), None);
        let answers: Vec<bool> = (0..3).map(|_| w.script_evaluate_conditions(s, None, Some(1))).collect();
        assert_eq!(answers, [false, true, true]);
        let copy = w.script_duplicate(s);
        assert_eq!((copy, w.script_name(copy)), (2, b"Attack".to_vec()));
        w.ai_create(1, true);
        let mut pos = Coord3D::default();
        assert!(!w.ai_base_center(1, &mut pos));
        w.cuts.player_host.ai.get_mut(&1).unwrap().base_center = Some(AT);
        assert!(w.ai_base_center(1, &mut pos));
        assert_eq!(pos, AT);
        assert_eq!(
            hosts(&w),
            [
                "ScriptEngine::findScriptByName",
                "ScriptEngine::findScriptByName",
                "ScriptEngine::evaluateConditions",
                "ScriptEngine::evaluateConditions",
                "ScriptEngine::evaluateConditions",
                "Script::duplicate",
                "newInstance(AIPlayer)",
                "AIPlayer::getBaseCenter",
                "AIPlayer::getBaseCenter",
            ]
        );
    }

    /// `findPlayerTemplate` finds a template by its name key, and an old map's sub-faction finds its base faction.
    #[test]
    fn finding_a_player_template_maps_an_old_sub_faction() {
        let (mut w, _) = world();
        let america = w.names.name_to_key(b"FactionAmerica");
        let tank_command = w.names.name_to_key(b"FactionAmericaTankCommand");
        let civilian = w.names.name_to_key(b"FactionCivilian");
        let nothing = w.names.name_to_key(b"FactionNothing");
        assert_eq!(w.player_template_find(america), Some(0));
        assert_eq!(w.player_template_find(tank_command), Some(0));
        assert_eq!(w.player_template_find(civilian), Some(1));
        assert_eq!(w.player_template_find(nothing), None);
        assert_eq!(w.player_template(1).side, b"Civilian");
    }

    /// A special power module's handle carries its object and module index, and is refused past the limits.
    #[test]
    fn a_special_power_handle_names_its_object_and_module() {
        assert_eq!(sp_ref(7, 3).map(sp_unref), Some((7, 3)));
        assert_eq!(sp_ref((1 << 24) - 1, 255).map(sp_unref), Some(((1 << 24) - 1, 255)));
        assert_eq!(sp_ref(1 << 24, 0), None);
        assert_eq!(sp_ref(1, 256), None);
    }
}
