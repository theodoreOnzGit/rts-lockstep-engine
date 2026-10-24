// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/PropagandaTowerBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/PropagandaTowerBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `PropagandaTowerBehavior`: a tower (or a speaker on a vehicle) that, every so often, pulses an FX and makes the allied units around it
//! enthusiastic (a weapon bonus; subliminal too once its upgrade is done) and heals them, as their sole benefactor; units that leave the
//! radius, or every unit when the tower is sold, disabled, carried, captured by the neutral player, dies or is deleted, lose the bonus.
//!
//! # Where the world comes in
//!
//! - The scan is [`World::partition_iterate_allies_alive_same_map_not_structures_at`] (`FROM_CENTER_2D` with `PartitionFilterRelationship
//!   (ALLOW_ALLIES)`, `PartitionFilterAlive`, `PartitionFilterSameMapStatus` and `PartitionFilterAcceptByKindOf(none, STRUCTURE)`, which the
//!   partition manager applies: layer 12's).
//! - The upgrade is the upgrade center's ([`crate::common::upgrade::UpgradeCenter::find_upgrade`], resolved at `onObjectCreated`); the
//!   player's completed upgrades are `cuts.player_completed_upgrade_mask`.
//! - The bonus is [`World::set_weapon_bonus_condition`] / [`World::clear_weapon_bonus_condition`] (when the unit has a damage weapon:
//!   `WeaponSet::has_any_damage_weapon`); the heal is [`World::attempt_healing_from_sole_benefactor`]; the pulse is
//!   [`ObjectClientEvent::FxListDoFxObj`].
//!
//! # Upstream facts kept
//!
//! - The tracked list is built by pushing at its head: it is kept in the reverse of the scan's order, and saved so.
//! - The scan runs when `now - m_lastScanFrame >= m_scanDelayInFrames` (unsigned: a frame before the last scan wraps and scans).
//! - A tower inside a container that is inside another, or a vehicle tower inside anything, does not scan; a carried tower is undone by
//!   the update first (`getContainedBy()->getContainedBy()`).
//! - The pulse is hidden from other players when the tower (or its stealthed, undetected container) is stealthed and undetected.
//! - `HealPercentEachSecond / LOGICFRAMES_PER_SECOND * maxHealth`, in `float`.
//! - A load into a non-empty list throws `SC_INVALID_DATA`.
//!
//! # Where upstream is undefined
//!
//! Giving or taking the effect with no controlling player, or with no `UpgradeRequired` found (`hasUpgradeComplete(NULL)` dereferences
//! it: [`Undefined::NullControllingPlayer`], [`Undefined::NullUpgradeTemplate`]); a scan with the upgrade a player one and no controlling
//! player. The case is recorded in `world.cuts.module_errors` and the call returns.
//!
//! `p1w13-sonnet`, batch R37, 2026-10-24.

use std::sync::Arc;

use crate::common::disabled_types::{DISABLED_COUNT, DISABLED_HELD};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::{KINDOF_MP_COUNT_FOR_VICTORY, KINDOF_PORTABLE_STRUCTURE, KINDOF_SCORE, KINDOF_SCORE_CREATE, KINDOF_SCORE_DESTROY, KINDOF_VEHICLE};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DIE};
use crate::common::object_status_types::{OBJECT_STATUS_DETECTED, OBJECT_STATUS_SOLD, OBJECT_STATUS_STEALTHED, OBJECT_STATUS_UNDER_CONSTRUCTION};
use crate::common::upgrade::{UpgradeMaskType, UpgradeType};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::weapon_bonus::condition as weapon_bonus;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `LOGICFRAMES_PER_SECOND`.
const LOGICFRAMES_PER_SECOND: f32 = 30.0;
/// The neutral player's index (`ThePlayerList->getNeutralPlayer()`).
const NEUTRAL_PLAYER: i32 = 0;

/// `class PropagandaTowerBehaviorModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct PropagandaTowerBehaviorModuleData {
    /// The base (`UpdateModuleData`, which adds nothing to `ModuleData`).
    pub update: UpdateModuleData,
    /// `m_scanRadius`: "radius of our scan".
    pub scan_radius: f32,
    /// `m_scanDelayInFrames`: "how frequently we do an update scan".
    pub scan_delay_in_frames: u32,
    /// `m_autoHealPercentPerSecond`.
    pub auto_heal_percent_per_second: f32,
    /// `m_pulseFX`, by name (`None` for `NULL`).
    pub pulse_fx: Option<Vec<u8>>,
    /// `m_upgradeRequired`: the upgrade's name (`NULL` in the constructor: the empty string).
    pub upgrade_required: Vec<u8>,
    /// `m_upgradedAutoHealPercentPerSecond`.
    pub upgraded_auto_heal_percent_per_second: f32,
    /// `m_upgradedPulseFX`.
    pub upgraded_pulse_fx: Option<Vec<u8>>,
    /// `m_affectsSelf`.
    pub affects_self: bool,
}

impl Default for PropagandaTowerBehaviorModuleData {
    /// `PropagandaTowerBehaviorModuleData()`.
    fn default() -> Self {
        PropagandaTowerBehaviorModuleData {
            update: UpdateModuleData::new(),
            scan_radius: 1.0,
            scan_delay_in_frames: 100,
            auto_heal_percent_per_second: 0.01,
            pulse_fx: None,
            upgrade_required: Vec::new(),
            upgraded_auto_heal_percent_per_second: 0.02,
            upgraded_pulse_fx: None,
            affects_self: false,
        }
    }
}

/// `INI::parseFXList`: the next token, `None` for `NULL`.
fn parse_fx(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let name = ini.get_next_token(None)?;
    Ok(if name.eq_ignore_ascii_case(b"None") { None } else { Some(name) })
}

/// `PropagandaTowerBehaviorModuleData::buildFieldParse`'s table.
static FIELD_PARSE: [FieldParse<PropagandaTowerBehaviorModuleData>; 9] = [
    FieldParse::new("Radius", |i, d, _| { d.scan_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("DelayBetweenUpdates", |i, d, _| { d.scan_delay_in_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("HealPercentEachSecond", |i, d, _| { d.auto_heal_percent_per_second = i.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("UpgradedHealPercentEachSecond", |i, d, _| { d.upgraded_auto_heal_percent_per_second = i.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("PulseFX", |i, d, _| { d.pulse_fx = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("UpgradeRequired", |i, d, _| { d.upgrade_required = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("UpgradedPulseFX", |i, d, _| { d.upgraded_pulse_fx = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("AffectsSelf", |i, d, _| { d.affects_self = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl PropagandaTowerBehaviorModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's fields.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, PropagandaTowerBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<PropagandaTowerBehaviorModuleData> {
        let mut data = PropagandaTowerBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, PropagandaTowerBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The upgrade `onObjectCreated` resolved: its type and mask (`NULL`: none).
#[derive(Clone, Copy, Debug, PartialEq)]
struct RequiredUpgrade {
    upgrade_type: UpgradeType,
    mask: UpgradeMaskType,
}

/// `class PropagandaTowerBehavior : public UpdateModule, public DieModuleInterface`.
#[derive(Clone, Debug)]
pub struct PropagandaTowerBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getPropagandaTowerBehaviorModuleData()`.
    data: Arc<PropagandaTowerBehaviorModuleData>,
    /// `m_lastScanFrame`.
    last_scan_frame: u32,
    /// `m_insideList`, head first.
    inside_list: Vec<ObjectId>,
    /// `m_upgradeRequired`.
    upgrade_required: Option<RequiredUpgrade>,
}

impl PartialEq for PropagandaTowerBehavior {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.last_scan_frame == other.last_scan_frame
            && self.inside_list == other.inside_list
            && self.upgrade_required == other.upgrade_required
    }
}

impl PropagandaTowerBehavior {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_DIE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and the state.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<PropagandaTowerBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(PropagandaTowerBehavior { update, data, last_scan_frame: 0, inside_list: Vec::new(), upgrade_required: None })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getPropagandaTowerBehaviorModuleData()`).
    pub fn module_data(&self) -> &Arc<PropagandaTowerBehaviorModuleData> {
        &self.data
    }

    /// The state the test compares: the last scan frame, whether an upgrade was found, and the tracked list (head first).
    pub fn state_words(&self) -> Vec<u32> {
        let mut w = vec![self.last_scan_frame, u32::from(self.upgrade_required.is_some()), self.inside_list.len() as u32];
        w.extend(self.inside_list.iter().map(|&id| id as u32));
        w
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    /// `crc` with the scheduling half the slot holds: the base's.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds: the base's.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// `onObjectCreated()`: "resolve" the upgrade.
    fn on_object_created(&mut self, world: &mut World) {
        let statics = world.statics.clone();
        let found = statics.upgrade_center.find_upgrade(&mut world.names, &self.data.upgrade_required).ok().flatten();
        self.upgrade_required = found.map(|u| RequiredUpgrade { upgrade_type: u.upgrade_type(), mask: u.upgrade_mask() });
    }

    /// `effectLogic(obj, giving, modData)`: grant or remove the effect.
    fn effect_logic(&self, world: &mut World, us: ObjectId, obj: ObjectId, giving: bool) -> Result<(), Undefined> {
        // `getObject()->getControllingPlayer()->hasUpgradeComplete(m_upgradeRequired)`: both dereferenced
        let player = world.get_controlling_player(us).ok_or(Undefined::NullControllingPlayer)?;
        let upgrade = self.upgrade_required.ok_or(Undefined::NullUpgradeTemplate)?;
        let effect_upgraded = world.cuts.player_completed_upgrade_mask(player).test_for_all(&upgrade.mask);
        if giving {
            if world.objects.find(obj).is_some_and(|o| o.weapon_set.has_any_damage_weapon()) {
                let test = |world: &World, c| world.objects.find(obj).is_some_and(|o| o.test_weapon_bonus_condition(c));
                if !test(world, weapon_bonus::ENTHUSIASTIC) {
                    world.set_weapon_bonus_condition(obj, weapon_bonus::ENTHUSIASTIC);
                }
                if (effect_upgraded || crate::logic::modules::plant(2076)) && !test(world, weapon_bonus::SUBLIMINAL) {
                    world.set_weapon_bonus_condition(obj, weapon_bonus::SUBLIMINAL);
                }
            }
            if let Some(body) = world.get_body_module(obj) {
                let health_percent = if effect_upgraded != crate::logic::modules::plant(2071) { self.data.upgraded_auto_heal_percent_per_second } else { self.data.auto_heal_percent_per_second };
                let max = world.module_call(obj, body, ModuleCall::GetMaxHealth).as_real(0.0);
                let amount = health_percent / if crate::logic::modules::plant(2075) { 15.0 } else { LOGICFRAMES_PER_SECOND } * max;
                // the non-stacking way
                world.attempt_healing_from_sole_benefactor(obj, amount, us, self.data.scan_delay_in_frames);
            }
        } else {
            world.clear_weapon_bonus_condition(obj, weapon_bonus::ENTHUSIASTIC);
            world.clear_weapon_bonus_condition(obj, weapon_bonus::SUBLIMINAL);
        }
        Ok(())
    }

    /// `removeAllInfluence()`: "remove all influence from objects we've given bonuses to".
    fn remove_all_influence(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        for obj in self.inside_list.clone() {
            if world.objects.find(obj).is_some() {
                self.effect_logic(world, us, obj, false)?;
            }
        }
        self.inside_list.clear();
        Ok(())
    }

    /// `doScan()`.
    fn do_scan(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        let d = self.data.clone();
        let mut upgrade_present = false;
        if let Some(upgrade) = self.upgrade_required {
            upgrade_present = match upgrade.upgrade_type {
                UpgradeType::Player => {
                    let player = world.get_controlling_player(us).ok_or(Undefined::NullControllingPlayer)?;
                    world.cuts.player_completed_upgrade_mask(player).test_for_all(&upgrade.mask)
                }
                UpgradeType::Object => world.objects.find(us).is_some_and(|o| o.object_upgrades_completed.test_for_all(&upgrade.mask)),
            };
        }
        let mut do_fx = true;
        let local = world.players.local_index();
        let Some(o) = world.objects.find(us) else { return Ok(()) };
        let overlord = o.contained_by;
        let (is_vehicle_tower, stealthed_undetected) =
            (o.is_kind_of(KINDOF_VEHICLE) && !o.is_kind_of(KINDOF_PORTABLE_STRUCTURE), o.test_status(OBJECT_STATUS_STEALTHED) && !o.test_status(OBJECT_STATUS_DETECTED));
        let not_local = world.get_controlling_player(us) != local;
        if let Some(over) = world.objects.find(overlord) {
            // dealing with someone else's tower
            if not_local && over.test_status(OBJECT_STATUS_STEALTHED) && !over.test_status(OBJECT_STATUS_DETECTED) {
                // so they don't give up their position
                do_fx = false;
            }
            if is_vehicle_tower {
                // "proptowers that are riding IN things cannot do their scan!"
                return Ok(());
            }
            if world.objects.find(over.contained_by).is_some() {
                // "proptowers that are riding ON things that are in-turn riding IN things cannot do their scan!"
                return Ok(());
            }
        }
        // "Certainly don't play if we ourselves are stealthed."
        if not_local && stealthed_undetected && !crate::logic::modules::plant(2078) {
            do_fx = false;
        }
        if do_fx {
            let fx = if upgrade_present != crate::logic::modules::plant(2079) { &d.upgraded_pulse_fx } else { &d.pulse_fx };
            if let Some(fx) = fx {
                world.emit(ObjectClientEvent::FxListDoFxObj { fx: fx.clone(), object_id: us });
            }
        }
        let pos = world.objects.find(us).map(|o| o.get_position()).unwrap_or_default();
        let mut new_inside_list: Vec<ObjectId> = Vec::new();
        let radius = if crate::logic::modules::plant(2074) { d.scan_radius * 2.0 } else { d.scan_radius };
        for obj in world.partition_iterate_allies_alive_same_map_not_structures_at(us, pos, radius) {
            if obj == us && !d.affects_self && !crate::logic::modules::plant(2073) {
                continue;
            }
            // `newEntry->next = newInsideList; newInsideList = newEntry;`: pushed at the head
            if crate::logic::modules::plant(2072) {
                new_inside_list.push(obj);
            } else {
                new_inside_list.insert(0, obj);
            }
        }
        // those that were inside and are not now lose the effect
        for curr in self.inside_list.clone() {
            if !new_inside_list.contains(&curr) && world.objects.find(curr).is_some() {
                self.effect_logic(world, us, curr, false)?;
            }
        }
        self.inside_list = new_inside_list;
        Ok(())
    }

    fn update_inner(&mut self, world: &mut World, me: ModuleHandle) -> Result<UpdateSleepTime, Undefined> {
        let us = me.object;
        let Some(o) = world.objects.find(us) else { return Ok(UPDATE_SLEEP_NONE) };
        if o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION) {
            return Ok(UPDATE_SLEEP_NONE);
        }
        if o.test_status(OBJECT_STATUS_SOLD) {
            self.remove_all_influence(world, us)?;
            return Ok(UPDATE_SLEEP_FOREVER);
        }
        if o.is_effectively_dead() {
            return Ok(UPDATE_SLEEP_FOREVER);
        }
        // any disable but `DISABLED_HELD`
        if o.is_disabled() && (0..DISABLED_COUNT).any(|t| t != DISABLED_HELD && o.disabled.test(t)) {
            self.remove_all_influence(world, us)?;
            return Ok(UPDATE_SLEEP_NONE);
        }
        let container = o.contained_by;
        if world.objects.find(container).is_some_and(|c| world.objects.find(c.contained_by).is_some()) {
            self.remove_all_influence(world, us)?;
            return Ok(UPDATE_SLEEP_NONE);
        }
        let current_frame = world.frame();
        if current_frame.wrapping_sub(self.last_scan_frame) >= self.data.scan_delay_in_frames {
            self.do_scan(world, us)?;
            self.last_scan_frame = current_frame;
        }
        // those inside that still count get the effect; the others leave the list
        let mut kept = Vec::new();
        for curr in self.inside_list.clone() {
            let counts = world.objects.find(curr).is_some_and(|o| {
                o.is_kind_of(KINDOF_SCORE) || o.is_kind_of(KINDOF_SCORE_CREATE) || o.is_kind_of(KINDOF_SCORE_DESTROY) || o.is_kind_of(KINDOF_MP_COUNT_FOR_VICTORY)
            });
            if counts {
                self.effect_logic(world, us, curr, true)?;
                kept.push(curr);
            }
        }
        self.inside_list = kept;
        Ok(UPDATE_SLEEP_NONE)
    }

    fn undefined(world: &mut World, me: ModuleHandle, r: Result<(), Undefined>) {
        if let Err(u) = r {
            world.cuts.module_errors.push((me, IniError::undefined(u)));
        }
    }
}

impl UpdateBehavior for PropagandaTowerBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        match self.update_inner(world, me) {
            Ok(s) => s,
            Err(u) => {
                world.cuts.module_errors.push((me, IniError::undefined(u)));
                UPDATE_SLEEP_NONE
            }
        }
    }
    /// The constructor's world half: awake.
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
    }
    /// `xfer`'s fields after the base: the last scan frame and the tracked ids (a load pushes each at the head, into an empty list).
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_unsigned_int(&mut self.last_scan_frame)?;
        let mut inside_count = self.inside_list.len() as u16;
        xfer.xfer_unsigned_short(&mut inside_count)?;
        if xfer.get_xfer_mode() == XferMode::Save {
            for id in self.inside_list.iter_mut() {
                xfer.xfer_object_id(id)?;
            }
        } else {
            if !self.inside_list.is_empty() {
                // "m_insideList should be empty but is not"
                return Err(XferStatus::ListNotEmpty);
            }
            for _ in 0..inside_count {
                let mut id = 0;
                xfer.xfer_object_id(&mut id)?;
                self.inside_list.insert(0, id);
            }
        }
        Ok(())
    }
    /// The object's calls: `onObjectCreated`, `onDelete`, `onCapture` and the die interface (no die mux).
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnObjectCreated => {
                self.on_object_created(world);
                ModuleReply::Done
            }
            ModuleCall::OnDie(_) | ModuleCall::OnDelete => {
                let r = self.remove_all_influence(world, me.object);
                Self::undefined(world, me, r);
                ModuleReply::Done
            }
            ModuleCall::OnCapture { new_owner, .. } => {
                if new_owner == NEUTRAL_PLAYER {
                    let r = self.remove_all_influence(world, me.object);
                    Self::undefined(world, me, r);
                    update_behavior::set_wake_frame(world, if crate::logic::modules::plant(2077) { UPDATE_SLEEP_NONE } else { UPDATE_SLEEP_FOREVER });
                } else {
                    update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
                }
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::PropagandaTowerBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::PropagandaTowerBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for PropagandaTowerBehavior {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base, then the fields.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.update.clone();
        let r = update_behavior::xfer_own(self, &mut base, xfer);
        self.update = base;
        r
    }
    fn load_post_process(&mut self) {
        self.update.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `PropagandaTowerBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::PropagandaTowerBehavior(Arc::new(PropagandaTowerBehaviorModuleData::friend_new_module_data(ini)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = PropagandaTowerBehaviorModuleData::default();
        assert_eq!((d.scan_radius, d.scan_delay_in_frames), (1.0, 100));
        assert!(d.upgrade_required.is_empty() && !d.affects_self);
        let m = PropagandaTowerBehavior::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words(), vec![0, 0, 0]);
    }
}
