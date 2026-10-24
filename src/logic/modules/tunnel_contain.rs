// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/TunnelContain.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/TunnelContain.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `TunnelContain`: one entrance of a player's tunnel network (a GLA tunnel). The riders are not this module's: they are the player's
//! [`TunnelTracker`]'s, the communal list of every tunnel the player owns, and the module answers the contain queries from it. A tunnel registers with the
//! tracker when its object is created and leaves it when it dies, is sold or deleted; the last one to go caves in on everyone inside.
//!
//! # Design
//!
//! As [`OpenContain`] for a class derived from it (see `heal_contain`). The tracker lives in the player ([`Player::tunnel_system`]); an operation that
//! touches the world checks it out of its slot for the call (`with_tracker`), as `docs/world-design.md` describes; the queries that read the world
//! (`getContainCount`, `getContainedItemsList`) read it in place.
//!
//! # Upstream facts kept
//!
//! - Most entry points read `getControllingPlayer()->getTunnelSystem()` unchecked: an object with no controlling player is undefined there
//!   ([`Undefined::NullControllingPlayer`]); `getContainCount`, `onSelling`, `onDie`, `onDelete` and `onObjectCreated` check it. The queries that cannot record
//!   (they read `&World`) flag it in `ContainCutTables::null_player_read`, which the module's entry points turn into the record.
//! - `removeFromContain` runs `onRemoving` (through the object's contain module, this one) and `onRemovedFrom` before it asks whether the tracker holds
//!   the rider at all, and does not touch this module's own list.
//! - `onDie` checks the die mux and does not extend `OpenContain::onDie` (nobody is let out: the riders stay in the network).
//! - The registration is `onObjectCreated`'s (`onBuildComplete`'s body is commented out upstream).
//! - `update` heals everyone in the network, then remembers a recent enemy attacker as the network's nemesis, reading the tracker of the player
//!   unchecked there (a player with no tracker is undefined).
//!
//! [`Player::tunnel_system`]: crate::logic::world::player::Player
//!
//! `p1w13-sonnet`, batch R10, 2026-10-22.

use std::sync::Arc;

use crate::common::disabled_types::DISABLED_HELD;
use crate::common::game_common::Relationship;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData,
};
use crate::common::kind_of::KINDOF_INFANTRY;
use crate::common::module::ModuleDataKind;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::DamageInfo;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{UpdateModule, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::open_contain::{
    self, out, OpenContain, OpenContainModuleData, OpenContainVirtuals,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::PlayerIndex;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::tunnel_tracker::{TunnelError, TunnelTracker};
use crate::logic::world::update_body::UpdateBody;

/// `LOGICFRAMES_PER_SECOND`.
const LOGICFRAMES_PER_SECOND: u32 = 30;

/// `class TunnelContainModuleData : public OpenContainModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct TunnelContainModuleData {
    /// The `OpenContainModuleData` base.
    pub open: OpenContainModuleData,
    /// `m_framesForFullHeal` (`TimeForFullHeal`, a duration as a real): "time (in frames) something becomes fully healed".
    pub frames_for_full_heal: f32,
}

impl Default for TunnelContainModuleData {
    /// `TunnelContainModuleData()`: "by default, takes no time to heal ppl" (1 frame), and infantry inside.
    fn default() -> Self {
        let mut open = OpenContainModuleData::default();
        let mut infantry = crate::common::kind_of::KindOfMaskType::new();
        infantry.set(KINDOF_INFANTRY, 1);
        open.allow_inside_kind_of = infantry;
        TunnelContainModuleData {
            open,
            frames_for_full_heal: 1.0,
        }
    }
}

/// Lifts entry `k` of `OpenContainModuleData`'s table onto the `open` base.
fn lift_open(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut TunnelContainModuleData,
    u: &UserData,
) -> IniResult<()> {
    match open_contain::FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.open, u),
        None => Ok(()),
    }
}

/// Lifts entry `k` of the die mux table onto the `open` base.
fn lift_die(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut TunnelContainModuleData,
    u: &UserData,
) -> IniResult<()> {
    match open_contain::DIE_MUX_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.open, u),
        None => Ok(()),
    }
}

static LIFTED_OPEN: [FieldParse<TunnelContainModuleData>; 16] = [
    FieldParse::new(
        "ContainMax",
        |i, d, u| lift_open(0, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "EnterSound",
        |i, d, u| lift_open(1, i, d, u),
        UserData::None,
    ),
    FieldParse::new("ExitSound", |i, d, u| lift_open(2, i, d, u), UserData::None),
    FieldParse::new(
        "DamagePercentToUnits",
        |i, d, u| lift_open(3, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "BurnedDeathToUnits",
        |i, d, u| lift_open(4, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "AllowInsideKindOf",
        |i, d, u| lift_open(5, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "ForbidInsideKindOf",
        |i, d, u| lift_open(6, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "PassengersAllowedToFire",
        |i, d, u| lift_open(7, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "PassengersInTurret",
        |i, d, u| lift_open(8, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "NumberOfExitPaths",
        |i, d, u| lift_open(9, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "DoorOpenTime",
        |i, d, u| lift_open(10, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "WeaponBonusPassedToPassengers",
        |i, d, u| lift_open(11, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "AllowAlliesInside",
        |i, d, u| lift_open(12, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "AllowEnemiesInside",
        |i, d, u| lift_open(13, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "AllowNeutralInside",
        |i, d, u| lift_open(14, i, d, u),
        UserData::None,
    ),
    FieldParse::end(),
];

static LIFTED_DIE: [FieldParse<TunnelContainModuleData>; 5] = [
    FieldParse::new("DeathTypes", |i, d, u| lift_die(0, i, d, u), UserData::None),
    FieldParse::new(
        "VeterancyLevels",
        |i, d, u| lift_die(1, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "ExemptStatus",
        |i, d, u| lift_die(2, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "RequiredStatus",
        |i, d, u| lift_die(3, i, d, u),
        UserData::None,
    ),
    FieldParse::end(),
];

/// `TunnelContainModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<TunnelContainModuleData>; 2] = [
    FieldParse::new(
        "TimeForFullHeal",
        |i, d, _| {
            d.frames_for_full_heal = i.parse_duration_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl TunnelContainModuleData {
    /// `TunnelContainModuleData::buildFieldParse(p)`: `OpenContainModuleData::buildFieldParse(p)`, then this class's table.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, TunnelContainModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&LIFTED_OPEN)?;
        p.add(&LIFTED_DIE)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<TunnelContainModuleData> {
        let mut data = TunnelContainModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, TunnelContainModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `TunnelContain`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::TunnelContain(Arc::new(
        TunnelContainModuleData::friend_new_module_data(ini)?,
    )))
}

/// `class TunnelContain : public OpenContain, public CreateModuleInterface`.
#[derive(Clone, Debug, PartialEq)]
pub struct TunnelContain {
    oc: OpenContain,
    /// `getTunnelContainModuleData()`.
    data: Arc<TunnelContainModuleData>,
    /// `m_needToRunOnBuildComplete`.
    need_to_run_on_build_complete: bool,
    /// `m_isCurrentlyRegistered`: "keeps track if this is registered with the player, so we don't double remove and mess up".
    is_currently_registered: bool,
    /// `getObject()`'s id, for the queries that have no handle.
    object: ObjectId,
}

/// Records that upstream is undefined here.
fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
    world.cuts.module_errors.push((me, IniError::undefined(u)));
}

/// The tracker of player `p`, in place (`None`: no player there, or no tracker).
fn tracker_of(world: &World, p: PlayerIndex) -> Option<&TunnelTracker> {
    world
        .players
        .get_nth(p)
        .and_then(|pl| pl.tunnel_system.as_ref())
}

/// Runs `f` with player `p`'s tracker checked out of its slot (`None`: no player there, or no tracker).
fn with_tracker<R>(
    world: &mut World,
    p: PlayerIndex,
    f: impl FnOnce(&mut TunnelTracker, &mut World) -> R,
) -> Option<R> {
    let mut t = world.players.get_nth_mut(p)?.tunnel_system.take()?;
    let r = f(&mut t, world);
    if let Some(pl) = world.players.get_nth_mut(p) {
        pl.tunnel_system = Some(t);
    }
    Some(r)
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

impl TunnelContain {
    /// `getInterfaceMask()`: `OpenContain::getInterfaceMask() | MODULEINTERFACE_CREATE`.
    pub const fn get_interface_mask() -> i32 {
        OpenContain::get_interface_mask() | crate::common::module::MODULEINTERFACE_CREATE
    }

    /// The constructor's first half: the base's (on a copy of its part of the data), then the two flags.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<TunnelContainModuleData>>,
    ) -> IniResult<TunnelContain> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let oc = OpenContain::unconstructed(object, Some(Arc::new(data.open.clone())))?;
        Ok(TunnelContain {
            oc,
            data,
            need_to_run_on_build_complete: true,
            is_currently_registered: false,
            object,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        self.oc.update_module()
    }

    /// The module data.
    pub fn module_data(&self) -> &Arc<TunnelContainModuleData> {
        &self.data
    }

    /// The state, as words: the base's, then the two flags.
    pub fn state_words(&self) -> Vec<u32> {
        let mut v = self.oc.state_words();
        v.push(u32::from(self.need_to_run_on_build_complete));
        v.push(u32::from(self.is_currently_registered));
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

    /// The module's answer to one of `Object`'s calls (a query that met an object with no controlling player is recorded: see the module docs).
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        let r = open_contain::contain_module_call(self, world, me, call);
        Self::check_null_player(world, me);
        r
    }

    /// A query read the tracker of an object with no controlling player (`getControllingPlayer()->getTunnelSystem()` through `NULL`).
    fn check_null_player(world: &mut World, me: ModuleHandle) {
        if world.cuts.contain.null_player_read.replace(false) {
            undefined(world, me, Undefined::NullControllingPlayer);
        }
    }

    /// `xfer` with the scheduling half the slot holds: the version (1), the base's, then the two flags.
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

    /// The two flags of `xfer`, after the base's.
    fn xfer_own_fields(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        xfer.xfer_bool(&mut self.need_to_run_on_build_complete)?;
        if crate::logic::modules::plant(1561) {
            return Ok(());
        }
        xfer.xfer_bool(&mut self.is_currently_registered)
    }

    /// The owning player (`getObject()->getControllingPlayer()`), recording undefined where upstream reads through it unchecked.
    fn owner_checked(world: &mut World, me: ModuleHandle) -> Option<PlayerIndex> {
        let p = world.get_controlling_player(me.object);
        if p.is_none() {
            undefined(world, me, Undefined::NullControllingPlayer);
        }
        p
    }

    /// The owner's tracker read in place, for the `&World` queries (`None`, and the flag set, for an object with no player or no tracker).
    fn tracker_read<'a>(&self, world: &'a World) -> Option<&'a TunnelTracker> {
        let t = world
            .get_controlling_player(self.object)
            .and_then(|p| tracker_of(world, p));
        if t.is_none() {
            world.cuts.contain.null_player_read.set(true);
        }
        t
    }

    /// `onTunnelDestroyed(getObject())` on `p`'s tracker.
    fn tunnel_destroyed(&mut self, world: &mut World, me: ModuleHandle, p: PlayerIndex) {
        let us = me.object;
        if let Some(r) = out(self, world, me, |w| {
            with_tracker(w, p, |t, w| t.on_tunnel_destroyed(w, us))
        }) {
            tracker_result(world, me, r);
        }
    }

    /// `onTunnelCreated(getObject())` on `p`'s tracker.
    fn tunnel_created(world: &mut World, me: ModuleHandle, p: PlayerIndex) {
        let us = me.object;
        with_tracker(world, p, |t, _| t.on_tunnel_created(us));
    }
}

impl OpenContainVirtuals for TunnelContain {
    fn oc(&self) -> &OpenContain {
        &self.oc
    }
    fn oc_mut(&mut self) -> &mut OpenContain {
        &mut self.oc
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::TunnelContain(self)
    }
    fn from_body(b: UpdateBody) -> Option<Self> {
        match b {
            UpdateBody::TunnelContain(m) => Some(m),
            _ => None,
        }
    }

    /// `isGarrisonable()`: `false`.
    fn is_garrisonable(&mut self, _world: &mut World, _me: ModuleHandle) -> bool {
        false
    }
    /// `isBustable()`: `TRUE` ("can this container get busted by a bunkerbuster").
    fn is_bustable(&self) -> bool {
        !crate::logic::modules::plant(1562)
    }
    /// `isTunnelContain()`: `TRUE`.
    fn is_tunnel_contain(&self) -> bool {
        true
    }
    /// `isImmuneToClearBuildingAttacks()`: `true`.
    fn is_immune_to_clear_building_attacks(&self) -> bool {
        true
    }
    /// `isDisplayedOnControlBar()`: `TRUE`.
    fn is_displayed_on_control_bar(&self, _world: &World) -> bool {
        true
    }
    /// `isKickOutOnCapture()`: "Caves and Tunnels don't kick out on capture."
    fn is_kick_out_on_capture(&self, _world: &World) -> bool {
        false
    }

    /// `addToContainList(obj)`: the rider joins the network's list.
    fn add_to_contain_list(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        let Some(p) = Self::owner_checked(world, me) else {
            return;
        };
        if with_tracker(world, p, |t, _| t.add_to_contain_list(rider)).is_none() {
            undefined(world, me, Undefined::NullObject); // no tracker: `getTunnelSystem()` is NULL
        }
    }

    /// `removeFromContain(obj, exposeStealthUnits)`: `onRemoving` and `onRemovedFrom` first, then out of the network's list if it is there.
    fn remove_from_contain(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        rider: ObjectId,
        expose_stealth_units: bool,
    ) {
        if rider == INVALID_ID {
            return;
        }
        // `getObject()->getContain()->onRemoving(obj)`: this module
        if !crate::logic::modules::plant(1563) {
            self.on_removing(world, me, rider);
        }
        let us = me.object;
        out(self, world, me, |w| w.contain_on_removed_from(rider, us));

        // "game tear down. We do the onRemove* stuff first because this is allowed to fail but that still needs to be done"
        let Some(p) = world.get_controlling_player(us) else {
            return;
        };
        let Some(t) = tracker_of(world, p) else {
            undefined(world, me, Undefined::NullObject);
            return;
        };
        if !t.is_in_container(rider) {
            return;
        }
        let _ = expose_stealth_units; // the tracker ignores it
        with_tracker(world, p, |t, _| t.remove_from_contain(rider));
    }

    /// `harmAndForceExitAllContained(info)`: the front of the network's list out and harmed, until the list is empty.
    fn harm_and_force_exit_all_contained(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        info: &mut DamageInfo,
    ) {
        let Some(p) = Self::owner_checked(world, me) else {
            return;
        };
        loop {
            let Some(t) = tracker_of(world, p) else {
                undefined(world, me, Undefined::NullObject);
                return;
            };
            let Some(&obj) = t.get_contained_items_list().first() else {
                break;
            };
            self.remove_from_contain(world, me, obj, true);
            let mut i = info.clone();
            out(self, world, me, |w| w.contain_attempt_damage(obj, &mut i));
            if tracker_of(world, p)
                .is_some_and(|t| t.get_contained_items_list().first() == Some(&obj))
            {
                // the front stayed: upstream loops for ever
                undefined(world, me, Undefined::EndlessLoop);
                return;
            }
        }
    }

    /// `killAllContained()`: everyone in the network out (the list as it was, the iterator advanced first) and killed.
    fn kill_all_contained(&mut self, world: &mut World, me: ModuleHandle) {
        let Some(p) = Self::owner_checked(world, me) else {
            return;
        };
        let Some(list) = tracker_of(world, p).map(|t| t.get_contained_items_list().to_vec()) else {
            undefined(world, me, Undefined::NullObject);
            return;
        };
        for obj in list {
            self.remove_from_contain(world, me, obj, true);
            if !crate::logic::modules::plant(1564) && !out(self, world, me, |w| w.contain_kill(obj))
            {
                undefined(world, me, Undefined::NullBodyModule); // `Object::kill` calls through `getBodyModule()`
                return;
            }
        }
    }

    /// `removeAllContained(exposeStealthUnits)`: everyone in the network out (the list as it was, the iterator advanced first).
    fn remove_all_contained(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        expose_stealth_units: bool,
    ) {
        let Some(p) = Self::owner_checked(world, me) else {
            return;
        };
        let Some(list) = tracker_of(world, p).map(|t| t.get_contained_items_list().to_vec()) else {
            undefined(world, me, Undefined::NullObject);
            return;
        };
        for obj in list {
            self.remove_from_contain(world, me, obj, expose_stealth_units);
        }
    }

    /// `iterateContained(func, userData, reverse)`: the network's list.
    fn iterate_contained(&self, world: &World, reverse: bool) -> Vec<ObjectId> {
        let mut v = self.get_contained_items_list_for_iteration(world);
        if reverse {
            v.reverse();
        }
        v
    }
    fn get_contained_items_list_for_iteration(&self, world: &World) -> Vec<ObjectId> {
        self.get_contained_items_list(world)
    }

    /// `getContainCount()`: the network's count (0 with no player or no tracker: upstream checks both here).
    fn get_contain_count(&self, world: &World) -> u32 {
        world
            .get_controlling_player(self.object)
            .and_then(|p| tracker_of(world, p))
            .map_or(0, |t| t.get_contain_count())
    }

    /// `getContainMax()`: the network's (`TheGlobalData->m_maxTunnelCapacity`).
    fn get_contain_max(&self, world: &World) -> i32 {
        match self.tracker_read(world) {
            Some(t) => t.get_contain_max(world),
            None => 0,
        }
    }

    /// `getContainedItemsList()`: the network's list.
    fn get_contained_items_list(&self, world: &World) -> Vec<ObjectId> {
        match self.tracker_read(world) {
            Some(t) => t.get_contained_items_list().to_vec(),
            None => Vec::new(),
        }
    }

    /// `TunnelContain::onContaining(obj, wasSelected)`: held, counted by the academy, and its partition cells looked after.
    fn on_containing(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        rider: ObjectId,
        was_selected: bool,
    ) {
        open_contain::base_on_containing(self, world, me, rider, was_selected);
        out(self, world, me, |w| {
            w.contain_set_disabled(rider, DISABLED_HELD)
        });
        // `obj->getControllingPlayer()->getAcademyStats()->recordUnitEnteredTunnelNetwork()`: the rider's player, unchecked
        let Some(rp) = world.get_controlling_player(rider) else {
            undefined(world, me, Undefined::NullControllingPlayer);
            return;
        };
        if !crate::logic::modules::plant(1565) {
            out(self, world, me, |w| {
                w.contain_record_unit_entered_tunnel_network(rp)
            });
        }
        out(self, world, me, |w| {
            w.contain_handle_partition_cell_maintenance(rider)
        });
    }

    /// `TunnelContain::onRemoving(obj)`: let go at the tunnel, registered and shown again, with the unload sound.
    fn on_removing(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        open_contain::base_on_removing(self, world, me, rider);
        let us = me.object;
        out(self, world, me, |w| {
            w.contain_clear_disabled(rider, DISABLED_HELD)
        });
        out(self, world, me, |w| {
            w.contain_partition_register_object(rider)
        });
        let pos = world
            .objects
            .find(us)
            .map(|o| o.get_position())
            .unwrap_or_default();
        out(self, world, me, |w| w.contain_set_position(rider, &pos));
        if world.has_drawable(rider) {
            let delay = world
                .objects
                .find(rider)
                .map_or(0, |o| o.template().get_occlusion_delay());
            let frame = world.frame().wrapping_add(delay);
            out(self, world, me, |w| {
                w.contain_set_safe_occlusion_frame(rider, frame)
            });
            if !crate::logic::modules::plant(1566) {
                out(self, world, me, |w| {
                    w.contain_drawable_set_hidden(rider, false)
                });
            }
        }
        open_contain::do_unload_sound(self, world, me);
    }

    /// `TunnelContain::onSelling()`: "Tunnel responds by kicking people out if this is the last tunnel", and leaves the network.
    fn on_selling(&mut self, world: &mut World, me: ModuleHandle) {
        let Some(p) = world.get_controlling_player(me.object) else {
            return;
        };
        let Some(count) = tracker_of(world, p).map(|t| t.friend_get_tunnel_count()) else {
            return;
        };
        if count == 1 && !crate::logic::modules::plant(1567) {
            // "Can't be order to exit, as I have no time to organize their exits."
            self.remove_all_contained(world, me, false);
        }
        if self.is_currently_registered {
            self.tunnel_destroyed(world, me, p);
            self.is_currently_registered = false;
        }
    }

    /// `TunnelContain::isValidContainerFor(obj, checkCapacity)`: the network's.
    fn is_valid_container_for(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        obj: ObjectId,
        check_capacity: bool,
    ) -> bool {
        let Some(p) = Self::owner_checked(world, me) else {
            return false;
        };
        let Some(t) = tracker_of(world, p) else {
            undefined(world, me, Undefined::NullObject);
            return false;
        };
        let obj = (obj != INVALID_ID).then_some(obj);
        t.is_valid_container_for(
            world,
            obj,
            check_capacity != crate::logic::modules::plant(1568),
        )
    }

    /// `TunnelContain::onDie(info)`: out of the network if it is in (the base's `onDie` is not run: nobody is let out).
    fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
        let applicable = {
            let Some(o) = world.objects.find(me.object) else {
                return;
            };
            self.data.open.die_mux_data.is_die_applicable(
                info.input.death_type,
                o.get_veterancy_level() as i32,
                &o.status,
            )
        };
        if !applicable || !self.is_currently_registered {
            return;
        }
        let Some(p) = world.get_controlling_player(me.object) else {
            return;
        };
        if tracker_of(world, p).is_none() {
            return;
        }
        self.tunnel_destroyed(world, me, p);
        self.is_currently_registered = crate::logic::modules::plant(1569);
    }

    /// `TunnelContain::onDelete()`: out of the network if it is in.
    fn on_delete(&mut self, world: &mut World, me: ModuleHandle) {
        if !self.is_currently_registered {
            return;
        }
        let Some(p) = world.get_controlling_player(me.object) else {
            return;
        };
        if tracker_of(world, p).is_none() {
            return;
        }
        self.tunnel_destroyed(world, me, p);
        self.is_currently_registered = false;
    }

    /// `TunnelContain::onObjectCreated()`: into the network, once.
    fn on_object_created(&mut self, world: &mut World, me: ModuleHandle) {
        if !self.need_to_run_on_build_complete {
            return;
        }
        self.need_to_run_on_build_complete = false;
        let Some(p) = world.get_controlling_player(me.object) else {
            return;
        };
        if tracker_of(world, p).is_none() {
            return;
        }
        Self::tunnel_created(world, me, p);
        self.is_currently_registered = !crate::logic::modules::plant(1570);
    }

    /// `TunnelContain::onCapture(old, new)`: from the old owner's network into the new one's, then the base's (nothing).
    fn on_capture(&mut self, world: &mut World, me: ModuleHandle, old_owner: i32, new_owner: i32) {
        if self.is_currently_registered {
            // `oldOwner->getTunnelSystem()`, `newOwner->getTunnelSystem()`: the players unchecked
            if old_owner < 0 || new_owner < 0 {
                undefined(world, me, Undefined::NullControllingPlayer);
                return;
            }
            if tracker_of(world, old_owner).is_some() {
                self.tunnel_destroyed(world, me, old_owner);
            }
            if tracker_of(world, new_owner).is_some() && !crate::logic::modules::plant(1571) {
                Self::tunnel_created(world, me, new_owner);
            }
        }
    }

    /// `TunnelContain::update()`: the base's, then the network healed and a recent enemy attacker remembered as its nemesis.
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let r = self.update_inner(world, me);
        Self::check_null_player(world, me);
        r
    }
}

impl TunnelContain {
    /// `TunnelContain::update()`'s body.
    fn update_inner(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        open_contain::base_update(self, world, me);
        let us = me.object;
        let frames = self.data.frames_for_full_heal;
        let Some(p) = world.get_controlling_player(us) else {
            return UPDATE_SLEEP_NONE;
        };
        let has_tracker = tracker_of(world, p).is_some();
        if has_tracker && !crate::logic::modules::plant(1572) {
            if let Some(r) = out(self, world, me, |w| {
                with_tracker(w, p, |t, w| t.heal_objects(w, frames))
            }) {
                tracker_result(world, me, r);
            }
        }
        if let Some(body) = world.get_body_module(us) {
            if let ModuleReply::Damage(info) = out(self, world, me, |w| {
                w.module_call(us, body, ModuleCall::GetLastDamageInfo)
            }) {
                let stamp = out(self, world, me, |w| {
                    w.module_call(us, body, ModuleCall::GetLastDamageTimestamp)
                })
                .as_int(0) as u32;
                if stamp.wrapping_add(LOGICFRAMES_PER_SECOND) > world.frame() {
                    let attacker = info.input.source_id;
                    if world.objects.find(attacker).is_some()
                        && world.get_relationship(us, attacker) == Relationship::Enemies
                    {
                        // `tunnelSystem->updateNemesis(attacker)`: unchecked
                        if !has_tracker {
                            undefined(world, me, Undefined::NullObject);
                            return UPDATE_SLEEP_NONE;
                        }
                        out(self, world, me, |w| {
                            with_tracker(w, p, |t, w| t.update_nemesis(w, Some(attacker)))
                        });
                    }
                }
            }
        }
        UPDATE_SLEEP_NONE
    }
}

impl UpdateBehavior for TunnelContain {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        self.oc.update_module()
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        OpenContainVirtuals::update(self, world, me)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::TunnelContain(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::TunnelContain(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(
        &mut self,
        _world: &mut World,
        _me: ModuleHandle,
        _call: ModuleCall,
    ) -> ModuleReply {
        ModuleReply::Unhandled
    }
}

impl Snapshot for TunnelContain {
    /// `crc`: "extend base class".
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        Snapshot::crc(&mut self.oc, xfer)
    }

    /// `xfer` (version 1): the version, `OpenContain::xfer`, then the two flags.
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
