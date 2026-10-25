// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/DemoTrapUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DemoTrapUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `DemoTrapUpdate`: the demolition trap. Its mode is its object's current weapon slot: the detonation slot (a command button switched
//! to it) detonates at once; the manual slot waits; the proximity slot scans every `ScanRate` frames and detonates when an enemy on the
//! ground (not an ignored kind, not dead, not a dozer disarming it) is within `TriggerDetonationRange` (a friend in range stops the scan
//! unless `AutoDetonationWithFriendsInvolved`). Detonating fires `DetonationWeapon` (unless the trap is being built or sold), kills the
//! trap and marks it detonated; a dead trap detonates if `DetonateWhenKilled`.
//!
//! # Upstream facts kept
//!
//! - `onObjectCreated` sets the veteran weapon set flag and locks the proximity or the manual slot (`LOCKED_TEMPORARILY`); the three slots
//!   being the same is only a debug crash.
//! - The current weapon is dereferenced unchecked: `Undefined::NullCurrentWeapon`.
//! - The scan is `iterateObjectsInRange(pos, range, FROM_CENTER_2D)` with no filters; the distance is `getDistanceSquared(.., FROM_CENTER_2D)`
//!   against the range squared. `ScanRate` (an unsigned duration) is kept in the signed `m_nextScanFrames`.
//! - `xfer`: the version, the update base, the scan countdown and the detonated flag.
//!
//! `p1w13-sonnet`, batch R55, 2026-10-25.

use std::sync::Arc;

use crate::common::kind_of::KindOfMaskType;
use crate::common::base_type::Coord3D;
use crate::common::game_common::Relationship;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::KINDOF_DOZER;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::{OBJECT_STATUS_IS_ATTACKING, OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DAMAGE_DISARM, DAMAGE_UNRESISTABLE, DEATH_NORMAL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::weapon_set::WEAPON_SLOT_TYPE_NAMES;
use crate::logic::weapon_types::WEAPONSET_VETERAN;
use crate::logic::weapon_set_state::LOCKED_TEMPORARILY;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `PRIMARY_WEAPON`.
const PRIMARY_WEAPON: i32 = 0;

/// `class DemoTrapUpdateModuleData : public ModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct DemoTrapUpdateModuleData {
    /// The base the update module is made with (`UpdateModuleData`, which adds nothing).
    pub update: UpdateModuleData,
    /// `m_ignoreKindOf`.
    pub ignore_kind_of: KindOfMaskType,
    /// `m_manualModeWeaponSlot`, `m_detonationWeaponSlot`, `m_proximityModeWeaponSlot`.
    pub manual_mode_weapon_slot: i32,
    pub detonation_weapon_slot: i32,
    pub proximity_mode_weapon_slot: i32,
    /// `m_triggerDetonationRange`.
    pub trigger_detonation_range: f32,
    /// `m_detonationWeaponTemplate`, by name (found in the weapon store when used; a name it does not have, `None` among them, is `NULL`).
    pub detonation_weapon_template: Option<Vec<u8>>,
    /// `m_scanFrames`.
    pub scan_frames: u32,
    /// `m_defaultsToProximityMode`, `m_friendlyDetonation`, `m_detonateWhenKilled`.
    pub defaults_to_proximity_mode: bool,
    pub friendly_detonation: bool,
    pub detonate_when_killed: bool,
}

impl Default for DemoTrapUpdateModuleData {
    /// `DemoTrapUpdateModuleData()`.
    fn default() -> Self {
        DemoTrapUpdateModuleData {
            update: UpdateModuleData::new(),
            ignore_kind_of: KindOfMaskType::default(),
            manual_mode_weapon_slot: PRIMARY_WEAPON,
            detonation_weapon_slot: PRIMARY_WEAPON,
            proximity_mode_weapon_slot: PRIMARY_WEAPON,
            trigger_detonation_range: 0.0,
            detonation_weapon_template: None,
            scan_frames: 0,
            defaults_to_proximity_mode: false,
            friendly_detonation: false,
            detonate_when_killed: false,
        }
    }
}

/// `INI::parseLookupList` over `TheWeaponSlotTypeNamesLookupList`.
fn parse_slot(ini: &mut Ini<'_>) -> IniResult<i32> {
    ini.parse_index_list(&WEAPON_SLOT_TYPE_NAMES)
}

static FIELD_PARSE: [FieldParse<DemoTrapUpdateModuleData>; 11] = [
    FieldParse::new("DefaultProximityMode", |i, d, _| { d.defaults_to_proximity_mode = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("DetonationWeaponSlot", |i, d, _| { d.detonation_weapon_slot = parse_slot(i)?; Ok(()) }, UserData::None),
    FieldParse::new("ProximityModeWeaponSlot", |i, d, _| { d.proximity_mode_weapon_slot = parse_slot(i)?; Ok(()) }, UserData::None),
    FieldParse::new("ManualModeWeaponSlot", |i, d, _| { d.manual_mode_weapon_slot = parse_slot(i)?; Ok(()) }, UserData::None),
    FieldParse::new("TriggerDetonationRange", |i, d, _| { d.trigger_detonation_range = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("IgnoreTargetTypes", |i, d, _| d.ignore_kind_of.parse_from_ini(i), UserData::None),
    FieldParse::new("ScanRate", |i, d, _| { d.scan_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("AutoDetonationWithFriendsInvolved", |i, d, _| { d.friendly_detonation = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("DetonationWeapon", |i, d, _| { d.detonation_weapon_template = Some(i.get_next_token(None)?); Ok(()) }, UserData::None),
    FieldParse::new("DetonateWhenKilled", |i, d, _| { d.detonate_when_killed = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl DemoTrapUpdateModuleData {
    /// `buildFieldParse(p)`: `ModuleData`'s (nothing), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, DemoTrapUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<DemoTrapUpdateModuleData> {
        let mut data = DemoTrapUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, DemoTrapUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `DemoTrapUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::DemoTrapUpdate(Arc::new(DemoTrapUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `class DemoTrapUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct DemoTrapUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getDemoTrapUpdateModuleData()`.
    data: Arc<DemoTrapUpdateModuleData>,
    /// `m_nextScanFrames`.
    next_scan_frames: i32,
    /// `m_detonated`.
    detonated: bool,
}

impl PartialEq for DemoTrapUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl DemoTrapUpdate {
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<DemoTrapUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        Ok(DemoTrapUpdate { update, data, next_scan_frames: 0, detonated: false })
    }

    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    pub fn module_data(&self) -> &Arc<DemoTrapUpdateModuleData> {
        &self.data
    }

    /// The scan countdown and the detonated flag.
    pub fn state_words(&self) -> Vec<u32> {
        vec![self.next_scan_frames as u32, u32::from(self.detonated)]
    }

    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// `onObjectCreated()`: "validate that we have the necessary data from the ini file".
    fn on_object_created(&mut self, world: &mut World, us: ObjectId) {
        let d = &self.data;
        world.set_weapon_set_flag(us, WEAPONSET_VETERAN);
        // "lock it just till the weapon is empty or the attack is "done""
        let slot = if d.defaults_to_proximity_mode != crate::logic::modules::plant(2251) { d.proximity_mode_weapon_slot } else { d.manual_mode_weapon_slot };
        world.set_weapon_lock(us, slot, LOCKED_TEMPORARILY);
    }

    fn test(world: &World, id: ObjectId, status: i32) -> bool {
        world.objects.find(id).is_some_and(|o| o.test_status(status))
    }

    fn effectively_dead(world: &World, id: ObjectId) -> bool {
        world.objects.find(id).is_some_and(|o| o.is_effectively_dead())
    }

    /// `update()`.
    fn run_update(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        let d = self.data.clone();
        if self.detonated {
            return Ok(());
        }
        if Self::test(world, us, OBJECT_STATUS_UNDER_CONSTRUCTION) || Self::test(world, us, OBJECT_STATUS_SOLD) {
            return Ok(());
        }
        if Self::effectively_dead(world, us) {
            if d.detonate_when_killed {
                self.detonate(world, us);
            }
            return Ok(());
        }
        // "Get the current weapon slot -- this determines what mode we're in."
        let weapon_slot = world.current_weapon_facts(us).ok_or(Undefined::NullCurrentWeapon)?.slot;
        if weapon_slot == d.detonation_weapon_slot {
            // "We've been externally triggered by the press of a command button."
            self.detonate(world, us);
            return Ok(());
        }
        // "Don't scan every frame for performance reasons."
        if self.next_scan_frames > 0 {
            self.next_scan_frames -= 1;
            return Ok(());
        }
        if weapon_slot == d.manual_mode_weapon_slot && !crate::logic::modules::plant(2252) {
            // "Don't scan!"
            return Ok(());
        }
        // "Reset timer here -- because if we are in manual mode, and switch, we want instant gratification (if possible)."
        self.next_scan_frames = if crate::logic::modules::plant(2253) { 0 } else { d.scan_frames as i32 };
        // "Scan for a valid enemy in proximity range."
        let range = d.trigger_detonation_range;
        let mut shall_detonate = false;
        for other in world.partition_iterate_objects_in_range(us, range) {
            let Some(o) = world.objects.find(other) else { continue };
            if o.is_any_kind_of(&d.ignore_kind_of) && !crate::logic::modules::plant(2254) {
                // "Skip specified types to ignore."
                continue;
            }
            if o.is_effectively_dead() {
                continue;
            }
            if o.is_kind_of(KINDOF_DOZER) {
                // "If we're dealing with a dozer... check if it's trying to disarm me. If so, don't blow up!"
                if world.contain_current_weapon(other) == Some(DAMAGE_DISARM) && Self::test(world, other, OBJECT_STATUS_IS_ATTACKING) && !crate::logic::modules::plant(2255) {
                    // "Also check if it's attacking, because it seems to stay in disarm mode."
                    continue;
                }
            }
            // "order matters: we want to know if I consider it to be an enemy, not vice versa"
            if world.get_relationship(us, other) != Relationship::Enemies {
                if !d.friendly_detonation && !crate::logic::modules::plant(2256) {
                    // "Not allowed to proximity detonate with friends nearby"
                    return Ok(());
                }
                // "Don't shoot our friends!"
                continue;
            }
            if world.is_above_terrain(other) && !crate::logic::modules::plant(2257) {
                // "Don't detonate on anything airborne."
                continue;
            }
            // "Anyone close enough?"
            let dist = world.partition_get_distance_squared_center_2d(us, other);
            let limit = if crate::logic::modules::plant(2258) { range } else { range * range };
            if dist <= limit {
                // "Yeehaw!"
                shall_detonate = true;
                if d.friendly_detonation {
                    // "Okay, no need to look for friends because we don't care."
                    break;
                }
            }
        }
        if shall_detonate {
            // "Enemy in proximity and we are in proximity detonation mode, so trigger the explosion and kill them!!!"
            self.detonate(world, us);
        }
        Ok(())
    }

    /// `detonate()`.
    fn detonate(&mut self, world: &mut World, us: ObjectId) {
        // "Only shoot the weapon if not being built or sold."
        if !Self::test(world, us, OBJECT_STATUS_UNDER_CONSTRUCTION) && !Self::test(world, us, OBJECT_STATUS_SOLD) {
            let name = self.data.detonation_weapon_template.as_deref();
            let found = name.and_then(|n| world.statics.weapon_store.templates().iter().find(|t| t.name.eq_ignore_ascii_case(n)).map(|t| t.name.clone()));
            if let Some(weapon) = found {
                let position = world.objects.find(us).map_or_else(Coord3D::default, |o| o.get_position());
                world.cut(CutCall::WeaponCreateAndFireTemp { weapon, source: us, position });
            }
        }
        if !crate::logic::modules::plant(2259) {
            world.kill(us, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
        }
        self.detonated = true;
    }
}

impl UpdateBehavior for DemoTrapUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if let Err(e) = self.run_update(world, me.object) {
            world.cuts.module_errors.push((me, IniError::undefined(e)));
        }
        UPDATE_SLEEP_NONE
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_int(&mut self.next_scan_frames)?;
        xfer.xfer_bool(&mut self.detonated)
    }
    /// `onObjectCreated`, and `detonate()` (called by `Object` and `SpecialAbilityUpdate`).
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnObjectCreated => {
                self.on_object_created(world, me.object);
                ModuleReply::Done
            }
            ModuleCall::Detonate => {
                self.detonate(world, me.object);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::DemoTrapUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::DemoTrapUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for DemoTrapUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base, the scan countdown, the detonated flag.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = DemoTrapUpdateModuleData::default();
        assert_eq!((d.manual_mode_weapon_slot, d.detonation_weapon_slot, d.proximity_mode_weapon_slot), (0, 0, 0));
        let m = DemoTrapUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words(), vec![0, 0]);
    }
}
