// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/CaveContain.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/CaveContain.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `CaveContain`: a cave of a map's cave network (a "distributed garrison"). As a tunnel, its riders are a [`TunnelTracker`]'s, but the tracker is the
//! cave system's, by cave index (`CaveIndex`, settable by script through `tryToSetCaveIndex`), not a player's; and whoever is inside owns every connected
//! cave: the first rider in makes them all defect to its player's default team, the last one out gives them back.
//!
//! # Design
//!
//! As `tunnel_contain` (the tracker checked out of the cave system for an operation that touches the world: [`World::with_cave_tracker`]). The
//! original team is a team id (`None` for `NULL`).
//!
//! # Upstream facts kept
//!
//! - `getTunnelTrackerForCaveIndex` is read unchecked: a cave whose index has no tracker (not built yet, or never registered) is undefined in every entry
//!   point that uses it ([`Undefined::NullObject`]); the queries that read `&World` flag it in `ContainCutTables::null_player_read`.
//! - The cave index is the data's from `onCreate`; the cave registers in `onBuildComplete`, not when its object is made.
//! - `removeFromContain` asks the tracker first, and only then runs `onRemoving` and `onRemovedFrom` (a tunnel does it the other way round).
//! - `harmAndForceExitAllContained` and `killAllContained` are `OpenContain`'s, on the module's own list, which is always empty: a cave's riders are
//!   neither harmed nor killed by them.
//! - `onDie` does nothing for a cave under construction ("it never registered itself as a tunnel"), and does not extend `OpenContain::onDie`.
//! - `recalcApparentControllingPlayer`'s indicator colour is set only for an apparent controller, and `OpenContain::getApparentControllingPlayer` is
//!   always `NULL`: that part never runs.
//! - `xfer` writes the original team's id; on a load, an id the team factory does not know is `SC_INVALID_DATA`, after `m_originalTeam` was set to
//!   the `NULL` it found. The check needs the world, so the load leaves it pending and [`CaveContain::check_loaded_team`] (called by
//!   `World::xfer_module_slot`) makes it.
//!
//! `p1w13-sonnet`, batch R11, 2026-10-23.

use std::sync::Arc;

use crate::common::disabled_types::DISABLED_HELD;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::model_state::MODELCONDITION_GARRISONED;
use crate::common::module::ModuleDataKind;
use crate::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION;
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::damage::DamageInfo;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{UpdateModule, UpdateSleepTime};
use crate::logic::modules::open_contain::{self, out, OpenContain, OpenContainModuleData, OpenContainVirtuals};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::TeamId;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::tunnel_tracker::{TunnelError, TunnelTracker};
use crate::logic::world::update_body::UpdateBody;

/// `TEAM_ID_INVALID`.
const TEAM_ID_INVALID: TeamId = 0;

/// `class CaveContainModuleData : public OpenContainModuleData`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CaveContainModuleData {
    /// The `OpenContainModuleData` base.
    pub open: OpenContainModuleData,
    /// `m_caveIndexData` (`CaveIndex`): "by default, all Caves will be grouped together as number 0".
    pub cave_index_data: i32,
}

/// Lifts entry `k` of `OpenContainModuleData`'s table onto the `open` base.
fn lift_open(k: usize, i: &mut Ini<'_>, d: &mut CaveContainModuleData, u: &UserData) -> IniResult<()> {
    match open_contain::FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.open, u),
        None => Ok(()),
    }
}

/// Lifts entry `k` of the die mux table onto the `open` base.
fn lift_die(k: usize, i: &mut Ini<'_>, d: &mut CaveContainModuleData, u: &UserData) -> IniResult<()> {
    match open_contain::DIE_MUX_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.open, u),
        None => Ok(()),
    }
}

static LIFTED_OPEN: [FieldParse<CaveContainModuleData>; 16] = [
    FieldParse::new("ContainMax", |i, d, u| lift_open(0, i, d, u), UserData::None),
    FieldParse::new("EnterSound", |i, d, u| lift_open(1, i, d, u), UserData::None),
    FieldParse::new("ExitSound", |i, d, u| lift_open(2, i, d, u), UserData::None),
    FieldParse::new("DamagePercentToUnits", |i, d, u| lift_open(3, i, d, u), UserData::None),
    FieldParse::new("BurnedDeathToUnits", |i, d, u| lift_open(4, i, d, u), UserData::None),
    FieldParse::new("AllowInsideKindOf", |i, d, u| lift_open(5, i, d, u), UserData::None),
    FieldParse::new("ForbidInsideKindOf", |i, d, u| lift_open(6, i, d, u), UserData::None),
    FieldParse::new("PassengersAllowedToFire", |i, d, u| lift_open(7, i, d, u), UserData::None),
    FieldParse::new("PassengersInTurret", |i, d, u| lift_open(8, i, d, u), UserData::None),
    FieldParse::new("NumberOfExitPaths", |i, d, u| lift_open(9, i, d, u), UserData::None),
    FieldParse::new("DoorOpenTime", |i, d, u| lift_open(10, i, d, u), UserData::None),
    FieldParse::new("WeaponBonusPassedToPassengers", |i, d, u| lift_open(11, i, d, u), UserData::None),
    FieldParse::new("AllowAlliesInside", |i, d, u| lift_open(12, i, d, u), UserData::None),
    FieldParse::new("AllowEnemiesInside", |i, d, u| lift_open(13, i, d, u), UserData::None),
    FieldParse::new("AllowNeutralInside", |i, d, u| lift_open(14, i, d, u), UserData::None),
    FieldParse::end(),
];

static LIFTED_DIE: [FieldParse<CaveContainModuleData>; 5] = [
    FieldParse::new("DeathTypes", |i, d, u| lift_die(0, i, d, u), UserData::None),
    FieldParse::new("VeterancyLevels", |i, d, u| lift_die(1, i, d, u), UserData::None),
    FieldParse::new("ExemptStatus", |i, d, u| lift_die(2, i, d, u), UserData::None),
    FieldParse::new("RequiredStatus", |i, d, u| lift_die(3, i, d, u), UserData::None),
    FieldParse::end(),
];

/// `CaveContainModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<CaveContainModuleData>; 2] = [
    FieldParse::new("CaveIndex", |i, d, _| { d.cave_index_data = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl CaveContainModuleData {
    /// `CaveContainModuleData::buildFieldParse(p)`: `OpenContainModuleData::buildFieldParse(p)`, then this class's table.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, CaveContainModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&LIFTED_OPEN)?;
        p.add(&LIFTED_DIE)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<CaveContainModuleData> {
        let mut data = CaveContainModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, CaveContainModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `CaveContain`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::CaveContain(Arc::new(CaveContainModuleData::friend_new_module_data(ini)?)))
}

/// `class CaveContain : public OpenContain, public CreateModuleInterface, public CaveInterface`.
#[derive(Clone, Debug, PartialEq)]
pub struct CaveContain {
    oc: OpenContain,
    /// `getCaveContainModuleData()`.
    data: Arc<CaveContainModuleData>,
    /// `m_needToRunOnBuildComplete`.
    need_to_run_on_build_complete: bool,
    /// `m_caveIndex`.
    cave_index: i32,
    /// `m_originalTeam`: "our original team before we were garrisoned" (`None` for `NULL`).
    original_team: Option<TeamId>,
    /// A loaded original team the team factory has not been asked about yet (see the module docs).
    team_check_pending: bool,
}

/// Records that upstream is undefined here.
fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
    world.cuts.module_errors.push((me, IniError::undefined(u)));
}

/// A tracker operation's undefined input, recorded.
fn tracker_result(world: &mut World, me: ModuleHandle, r: Result<(), TunnelError>) {
    let u = match r {
        Ok(()) => return,
        Err(TunnelError::NoBodyModule(_)) => Undefined::NullBodyModule,
        Err(_) => Undefined::NullObject,
    };
    undefined(world, me, u);
}

impl CaveContain {
    /// `getInterfaceMask()`: `OpenContain::getInterfaceMask() | MODULEINTERFACE_CREATE`.
    pub const fn get_interface_mask() -> i32 {
        OpenContain::get_interface_mask() | crate::common::module::MODULEINTERFACE_CREATE
    }

    /// The constructor's first half: the base's (on a copy of its part of the data); index 0, no original team.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<CaveContainModuleData>>) -> IniResult<CaveContain> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let oc = OpenContain::unconstructed(object, Some(Arc::new(data.open.clone())))?;
        Ok(CaveContain { oc, data, need_to_run_on_build_complete: true, cave_index: 0, original_team: None, team_check_pending: false })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        self.oc.update_module()
    }

    /// The module data.
    pub fn module_data(&self) -> &Arc<CaveContainModuleData> {
        &self.data
    }

    /// The state, as words: the base's, then the flag, the index and the original team (`TEAM_ID_INVALID` for none).
    pub fn state_words(&self) -> Vec<u32> {
        let mut v = self.oc.state_words();
        v.push(u32::from(self.need_to_run_on_build_complete));
        v.push(self.cave_index as u32);
        v.push(self.original_team.unwrap_or(TEAM_ID_INVALID));
        v
    }

    /// The constructor's second half: the base's.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.oc.world_construct(world, me);
    }

    /// `update()` as the scheduler runs it.
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The module's answer to one of `Object`'s calls (a query that met no tracker is recorded: see the module docs).
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        let r = open_contain::contain_module_call(self, world, me, call);
        if world.cuts.contain.null_player_read.replace(false) {
            undefined(world, me, Undefined::NullObject);
        }
        r
    }

    /// `xfer` with the scheduling half the slot holds: the version (1), the base's, then the flag, the index and the original team's id.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.oc.xfer_with(update, xfer)?;
        self.xfer_own_fields(xfer)
    }

    /// `crc` with the scheduling half: "extend base class".
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.oc.crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half: "extend base class".
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        self.oc.load_post_process_with(update);
    }

    /// The fields of `xfer`, after the base's. On a load, an original team other than `TEAM_ID_INVALID` is kept and left for
    /// [`CaveContain::check_loaded_team`].
    fn xfer_own_fields(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        xfer.xfer_bool(&mut self.need_to_run_on_build_complete)?;
        xfer.xfer_int(&mut self.cave_index)?;
        let mut team_id = self.original_team.unwrap_or(TEAM_ID_INVALID);
        let mut b = team_id.to_le_bytes();
        let r = xfer.xfer_user(&mut b);
        team_id = u32::from_le_bytes(b);
        r?;
        if xfer.get_xfer_mode() == XferMode::Load {
            if team_id != TEAM_ID_INVALID && !crate::logic::modules::plant(1581) {
                self.original_team = Some(team_id);
                self.team_check_pending = true;
            } else {
                self.original_team = None;
            }
        }
        Ok(())
    }

    /// The load's `TheTeamFactory->findTeamByID(teamID)`: an id it does not know is `SC_INVALID_DATA` (the test's `XFER_LIST_NOT_EMPTY`), after the
    /// original team was set to the `NULL` it found.
    pub fn check_loaded_team(&mut self, world: &World) -> XferResult<()> {
        if !std::mem::take(&mut self.team_check_pending) {
            return Ok(());
        }
        let id = self.original_team.unwrap_or(TEAM_ID_INVALID);
        if world.contain_find_team_by_id(id) {
            return Ok(());
        }
        self.original_team = None;
        Err(XferStatus::ListNotEmpty)
    }

    /// The tracker of this cave's index, read in place (`None`, and the flag set, when the index has none).
    fn tracker_read<'a>(&self, world: &'a World) -> Option<&'a TunnelTracker> {
        let t = world.cave_system.get_tunnel_tracker_for_cave_index(self.cave_index);
        if t.is_none() {
            world.cuts.contain.null_player_read.set(true);
        }
        t
    }

    /// The tracker of this cave's index, for an entry point that reads it unchecked (`None`, and undefined recorded, when there is none).
    fn tracker_checked<'a>(&self, world: &'a mut World, me: ModuleHandle) -> Option<&'a TunnelTracker> {
        if world.cave_system.get_tunnel_tracker_for_cave_index(self.cave_index).is_none() {
            undefined(world, me, Undefined::NullObject);
            return None;
        }
        world.cave_system.get_tunnel_tracker_for_cave_index(self.cave_index)
    }

    /// `tryToSetCaveIndex(newIndex)`: "Called by script as an alternative to instancing separate objects. 'Try', because can fail."
    pub fn try_to_set_cave_index(&mut self, world: &mut World, me: ModuleHandle, new_index: i32) {
        if !world.cave_system.can_switch_index_to_index(self.cave_index, new_index) {
            return;
        }
        let us = me.object;
        let old = self.cave_index;
        if world.cave_system.get_tunnel_tracker_for_cave_index(old).is_none() {
            undefined(world, me, Undefined::NullObject);
            return;
        }
        world.cave_system.unregister_cave(old);
        if let Some(r) = out(self, world, me, |w| w.with_cave_tracker(old, |t, w| t.on_tunnel_destroyed(w, us))) {
            tracker_result(world, me, r);
        }
        self.cave_index = new_index;
        if world.cave_system.register_new_cave(new_index).is_err() {
            undefined(world, me, Undefined::EndlessLoop); // "the while loop would push NULL forever"
            return;
        }
        if !crate::logic::modules::plant(1582) {
            world.with_cave_tracker(new_index, |t, _| t.on_tunnel_created(us));
        }
    }

    /// `changeTeamOnAllConnectedCaves(newTeam, setOriginalTeams)`: "when one gets captured, all connected ones get captured".
    fn change_team_on_all_connected_caves(&mut self, world: &mut World, me: ModuleHandle, new_team: Option<TeamId>, set_original_teams: bool) {
        let Some(all_caves) = self.tracker_checked(world, me).map(|t| t.get_container_list().to_vec()) else { return };
        for cave in all_caves {
            if world.objects.find(cave).is_none() {
                continue;
            }
            // `findCave(currentCave)`: the first behaviour module with a cave interface
            let Some(module) = out(self, world, me, |w| w.first_module_answering(cave, ModuleCall::HasCaveInterface)) else { continue };
            let team = if set_original_teams { world.get_team(cave) } else { None };
            if cave == me.object && module == me.index {
                // this module itself
                self.original_team = team;
            } else {
                out(self, world, me, |w| w.module_call(cave, module, ModuleCall::SetOriginalTeam(team)));
            }
            if !crate::logic::modules::plant(1583) {
                out(self, world, me, |w| w.contain_defect(cave, new_team, 0));
            }
        }
    }
}

impl OpenContainVirtuals for CaveContain {
    fn oc(&self) -> &OpenContain {
        &self.oc
    }
    fn oc_mut(&mut self) -> &mut OpenContain {
        &mut self.oc
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::CaveContain(self)
    }
    fn from_body(b: UpdateBody) -> Option<Self> {
        match b {
            UpdateBody::CaveContain(m) => Some(m),
            _ => None,
        }
    }

    /// `isGarrisonable()`: `false`.
    fn is_garrisonable(&mut self, _world: &mut World, _me: ModuleHandle) -> bool {
        false
    }
    /// `isBustable()`: `TRUE`.
    fn is_bustable(&self) -> bool {
        true
    }
    /// `isKickOutOnCapture()`: "Caves and Tunnels don't kick out on capture."
    fn is_kick_out_on_capture(&self, _world: &World) -> bool {
        crate::logic::modules::plant(1584)
    }

    /// `addToContainList(obj)`: the rider joins the cave network's list.
    fn add_to_contain_list(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        if self.tracker_checked(world, me).is_none() {
            return;
        }
        world.with_cave_tracker(self.cave_index, |t, _| t.add_to_contain_list(rider));
    }

    /// `removeFromContain(obj, exposeStealthUnits)`: out of the network's list if it is there, then `onRemoving` and `onRemovedFrom`.
    fn remove_from_contain(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId, _expose_stealth_units: bool) {
        if rider == INVALID_ID {
            return;
        }
        let Some(t) = self.tracker_checked(world, me) else { return };
        if !t.is_in_container(rider) {
            return;
        }
        world.with_cave_tracker(self.cave_index, |t, _| t.remove_from_contain(rider));
        // `getObject()->getContain()->onRemoving(obj)`: this module
        self.on_removing(world, me, rider);
        let us = me.object;
        if !crate::logic::modules::plant(1585) {
            out(self, world, me, |w| w.contain_on_removed_from(rider, us));
        }
    }

    /// `removeAllContained(exposeStealthUnits)`: everyone in the network out (the list as it was, the iterator advanced first).
    fn remove_all_contained(&mut self, world: &mut World, me: ModuleHandle, expose_stealth_units: bool) {
        let Some(list) = self.tracker_checked(world, me).map(|t| t.get_contained_items_list().to_vec()) else { return };
        for obj in list {
            self.remove_from_contain(world, me, obj, expose_stealth_units);
        }
    }

    /// `iterateContained(func, userData, reverse)`: the network's list.
    fn iterate_contained(&self, world: &World, reverse: bool) -> Vec<ObjectId> {
        let mut v = self.get_contained_items_list(world);
        if reverse {
            v.reverse();
        }
        v
    }
    fn get_contained_items_list_for_iteration(&self, world: &World) -> Vec<ObjectId> {
        self.get_contained_items_list(world)
    }
    /// `getContainCount()`: the network's.
    fn get_contain_count(&self, world: &World) -> u32 {
        self.tracker_read(world).map_or(0, |t| t.get_contain_count())
    }
    /// `getContainMax()`: the network's.
    fn get_contain_max(&self, world: &World) -> i32 {
        self.tracker_read(world).map_or(0, |t| t.get_contain_max(world))
    }
    /// `getContainedItemsList()`: the network's.
    fn get_contained_items_list(&self, world: &World) -> Vec<ObjectId> {
        self.tracker_read(world).map_or_else(Vec::new, |t| t.get_contained_items_list().to_vec())
    }

    /// `CaveContain::isValidContainerFor(obj, checkCapacity)`: the network's.
    fn is_valid_container_for(&mut self, world: &mut World, me: ModuleHandle, obj: ObjectId, check_capacity: bool) -> bool {
        if self.tracker_checked(world, me).is_none() {
            return false;
        }
        let obj = (obj != INVALID_ID).then_some(obj);
        let world: &World = world;
        world.cave_system.get_tunnel_tracker_for_cave_index(self.cave_index).is_some_and(|t| t.is_valid_container_for(world, obj, check_capacity))
    }

    /// `CaveContain::onContaining(obj, wasSelected)`: held, and the caves' owner worked out again.
    fn on_containing(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId, was_selected: bool) {
        open_contain::base_on_containing(self, world, me, rider, was_selected);
        out(self, world, me, |w| w.contain_set_disabled(rider, DISABLED_HELD));
        if !crate::logic::modules::plant(1586) {
            self.recalc_apparent_controlling_player(world, me);
        }
    }

    /// `CaveContain::onRemoving(obj)`: let go at the cave and shown again; the last one out gives the caves back.
    fn on_removing(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        open_contain::base_on_removing(self, world, me, rider);
        let us = me.object;
        out(self, world, me, |w| w.contain_clear_disabled(rider, DISABLED_HELD));
        out(self, world, me, |w| w.contain_partition_register_object(rider));
        let pos = world.objects.find(us).map(|o| o.get_position()).unwrap_or_default();
        out(self, world, me, |w| w.contain_set_position(rider, &pos));
        if world.has_drawable(rider) {
            out(self, world, me, |w| w.contain_drawable_set_hidden(rider, false));
        }
        open_contain::do_unload_sound(self, world, me);

        if self.get_contain_count(world) == 0 {
            if world.get_team(us).is_some() && !crate::logic::modules::plant(1587) {
                let original = self.original_team;
                self.change_team_on_all_connected_caves(world, me, original, false);
                self.original_team = None;
            }
            out(self, world, me, |w| w.contain_drawable_model_condition(us, MODELCONDITION_GARRISONED, false));
        }
    }

    /// `CaveContain::onDie(info)`: out of the cave system (not for a cave under construction: "it never registered itself as a tunnel").
    fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
        let us = me.object;
        let (applicable, building) = {
            let Some(o) = world.objects.find(us) else { return };
            (self.data.open.die_mux_data.is_die_applicable(info.input.death_type, o.get_veterancy_level() as i32, &o.status), o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION))
        };
        if !applicable || building {
            return;
        }
        if self.tracker_checked(world, me).is_none() {
            return;
        }
        world.cave_system.unregister_cave(self.cave_index);
        if !crate::logic::modules::plant(1588) {
            let index = self.cave_index;
            if let Some(r) = out(self, world, me, |w| w.with_cave_tracker(index, |t, w| t.on_tunnel_destroyed(w, us))) {
                tracker_result(world, me, r);
            }
        }
    }

    /// `onCreate()`, `onBuildComplete()`, `tryToSetCaveIndex(i)`, `setOriginalTeam(team)`, and the cave interface.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnCreate => {
                self.cave_index = if crate::logic::modules::plant(1589) { 0 } else { self.data.cave_index_data };
                ModuleReply::Done
            }
            ModuleCall::OnBuildComplete => {
                if !self.need_to_run_on_build_complete {
                    return ModuleReply::Done;
                }
                self.need_to_run_on_build_complete = false;
                if world.cave_system.register_new_cave(self.cave_index).is_err() {
                    undefined(world, me, Undefined::EndlessLoop); // "the while loop would push NULL forever"
                    return ModuleReply::Done;
                }
                let us = me.object;
                world.with_cave_tracker(self.cave_index, |t, _| t.on_tunnel_created(us));
                ModuleReply::Done
            }
            ModuleCall::TryToSetCaveIndex(i) => {
                self.try_to_set_cave_index(world, me, i);
                ModuleReply::Done
            }
            ModuleCall::SetOriginalTeam(team) => {
                self.original_team = team;
                ModuleReply::Done
            }
            ModuleCall::HasCaveInterface => ModuleReply::Bool(true),
            ModuleCall::RecalcApparentControllingPlayer => {
                self.recalc_apparent_controlling_player(world, me);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl CaveContain {
    /// `recalcApparentControllingPlayer()`: one rider makes every connected cave defect to its player's default team (remembering their teams); none
    /// gives them back.
    fn recalc_apparent_controlling_player(&mut self, world: &mut World, me: ModuleHandle) {
        let us = me.object;
        if self.original_team.is_none() {
            self.original_team = world.get_team(us);
        }
        if world.get_team(us).is_none() {
            self.original_team = None;
        }
        let count = self.get_contain_count(world);
        if count == 1 {
            let Some(&rider) = self.get_contained_items_list(world).first() else {
                undefined(world, me, Undefined::NullObject);
                return;
            };
            if let Some(p) = world.get_controlling_player(rider) {
                let team = world.cuts.player_default_team(p);
                self.change_team_on_all_connected_caves(world, me, Some(team), true);
            }
        } else if count == 0 && !crate::logic::modules::plant(1590) {
            let original = self.original_team;
            self.change_team_on_all_connected_caves(world, me, original, false);
        }
        // `getApparentControllingPlayer(localPlayer)`: `OpenContain`'s, always `NULL`; the indicator colour is never set
    }
}

impl UpdateBehavior for CaveContain {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        self.oc.update_module()
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        OpenContainVirtuals::update(self, world, me)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::CaveContain(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::CaveContain(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, _world: &mut World, _me: ModuleHandle, _call: ModuleCall) -> ModuleReply {
        ModuleReply::Unhandled
    }
}

impl Snapshot for CaveContain {
    /// `crc`: "extend base class".
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        Snapshot::crc(&mut self.oc, xfer)
    }

    /// `xfer` (version 1): the version, `OpenContain::xfer`, then the flag, the index and the original team's id.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        Snapshot::xfer(&mut self.oc, xfer)?;
        self.xfer_own_fields(xfer)
    }

    /// `loadPostProcess`: "extend base class".
    fn load_post_process(&mut self) {
        Snapshot::load_post_process(&mut self.oc);
    }
}
