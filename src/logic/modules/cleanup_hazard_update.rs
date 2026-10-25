// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/CleanupHazardUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/CleanupHazardUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `CleanupHazardUpdate`: a unit that cleans up hazards (`KINDOF_CLEANUP_HAZARD`: toxin puddles, radiation fields). Every `ScanRate` frames
//! it finds the closest hazard within `ScanRange` (of itself, or of the cleanup area plus its move range once `setCleanupAreaParameters` gave
//! it one); between scans it tracks the target (a target that left the weapon's range is dropped and a new scan forced in 0 to 3 frames) and,
//! when its AI is idle or busy, locks the weapon slot and attacks the target. Cleaning an area keeps an idle AI busy, gives the area up on a
//! new command from anyone but the AI, and once nothing is left walks back to the area's centre (or, within 25, gives up the area).
//!
//! # Upstream facts kept
//!
//! - `m_weaponTemplate` is the template of the weapon in `WeaponSlot` at `onObjectCreated` (after setting the veteran weapon set flag); with no
//!   weapon there it stays `NULL`, and tracking a target with no move range reads its attack range through it: `Undefined::NullWeaponTemplate`.
//!   It is not saved (`onObjectCreated` finds it again).
//! - `m_nextShotAvailableInFrames` is saved but nothing sets it above 0.
//! - The re-scan delay is `GameLogicRandomValue(0, 3)`; distances are `sqrt` of `getDistanceSquared(.., FROM_CENTER_2D)`.
//! - `xfer`: the version, the update base, the target, in range, the scan countdown, the shot countdown, the area's centre and its move range.
//!
//! `p1w13-sonnet`, batch R56, 2026-10-25.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::open_contain::CMD_FROM_AI;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::weapon::NO_MAX_SHOTS_LIMIT;
use crate::logic::weapon_bonus::WeaponBonus;
use crate::logic::weapon_set::WEAPON_SLOT_TYPE_NAMES;
use crate::logic::weapon_set_state::LOCKED_TEMPORARILY;
use crate::logic::weapon_types::WEAPONSET_VETERAN;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `PRIMARY_WEAPON`.
const PRIMARY_WEAPON: i32 = 0;

/// `class CleanupHazardUpdateModuleData : public ModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct CleanupHazardUpdateModuleData {
    /// The base the update module is made with (`UpdateModuleData`, which adds nothing).
    pub update: UpdateModuleData,
    /// `m_weaponSlot`.
    pub weapon_slot: i32,
    /// `m_scanFrames`.
    pub scan_frames: u32,
    /// `m_scanRange`.
    pub scan_range: f32,
}

impl Default for CleanupHazardUpdateModuleData {
    /// `CleanupHazardUpdateModuleData()`.
    fn default() -> Self {
        CleanupHazardUpdateModuleData { update: UpdateModuleData::new(), weapon_slot: PRIMARY_WEAPON, scan_frames: 0, scan_range: 0.0 }
    }
}

static FIELD_PARSE: [FieldParse<CleanupHazardUpdateModuleData>; 4] = [
    FieldParse::new("WeaponSlot", |i, d, _| { d.weapon_slot = i.parse_index_list(&WEAPON_SLOT_TYPE_NAMES)?; Ok(()) }, UserData::None),
    FieldParse::new("ScanRate", |i, d, _| { d.scan_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ScanRange", |i, d, _| { d.scan_range = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl CleanupHazardUpdateModuleData {
    /// `buildFieldParse(p)`: `ModuleData`'s (nothing), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, CleanupHazardUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<CleanupHazardUpdateModuleData> {
        let mut data = CleanupHazardUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, CleanupHazardUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `CleanupHazardUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::CleanupHazardUpdate(Arc::new(CleanupHazardUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `class CleanupHazardUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct CleanupHazardUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getCleanupHazardUpdateModuleData()`.
    data: Arc<CleanupHazardUpdateModuleData>,
    /// `m_bestTargetID`.
    best_target_id: ObjectId,
    /// `m_inRange`.
    in_range: bool,
    /// `m_nextScanFrames`, `m_nextShotAvailableInFrames`.
    next_scan_frames: i32,
    next_shot_available_in_frames: i32,
    /// `m_weaponTemplate`: its position in the weapon store (`None`: `NULL`).
    weapon_template: Option<usize>,
    /// `m_pos`, `m_moveRange`: the cleanup area.
    pos: Coord3D,
    move_range: f32,
}

impl PartialEq for CleanupHazardUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl CleanupHazardUpdate {
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<CleanupHazardUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        Ok(CleanupHazardUpdate {
            update,
            data,
            best_target_id: INVALID_ID,
            in_range: false,
            next_scan_frames: 0,
            next_shot_available_in_frames: 0,
            weapon_template: None,
            pos: Coord3D::default(),
            move_range: 0.0,
        })
    }

    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    pub fn module_data(&self) -> &Arc<CleanupHazardUpdateModuleData> {
        &self.data
    }

    /// The target, in range, the two countdowns, whether the weapon template was found, the area's centre (bits) and move range (bits).
    pub fn state_words(&self) -> Vec<u32> {
        vec![
            self.best_target_id as u32,
            u32::from(self.in_range),
            self.next_scan_frames as u32,
            self.next_shot_available_in_frames as u32,
            u32::from(self.weapon_template.is_some()),
            self.pos.x.to_bits(),
            self.pos.y.to_bits(),
            self.pos.z.to_bits(),
            self.move_range.to_bits(),
        ]
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

    /// `onObjectCreated()`: "make sure we have a weapon template".
    fn on_object_created(&mut self, world: &mut World, us: ObjectId) {
        world.set_weapon_set_flag(us, WEAPONSET_VETERAN);
        if let Some(t) = world.weapon_in_slot_template(us, self.data.weapon_slot) {
            self.weapon_template = Some(t);
        }
        // (the scan range not larger than the attack range is only a debug crash)
    }

    /// `m_weaponTemplate->getAttackRange(bonus)` with a cleared bonus.
    fn attack_range(&self, world: &World) -> Result<f32, Undefined> {
        let t = self.weapon_template.ok_or(Undefined::NullWeaponTemplate)?;
        let bonus = WeaponBonus::default();
        Ok(world.statics.weapon_store.templates()[t].get_attack_range(&bonus))
    }

    fn ai_reply(world: &mut World, us: ObjectId, call: ModuleCall) -> Option<ModuleReply> {
        let ai = world.objects.find(us).and_then(|o| o.ai)?;
        Some(world.module_call(us, ai, call))
    }

    fn ai_idle_or_busy(world: &mut World, us: ObjectId) -> Option<bool> {
        let idle = Self::ai_reply(world, us, ModuleCall::AiIsIdle)?.as_bool();
        Some(idle || Self::ai_reply(world, us, ModuleCall::AiIsBusy)?.as_bool())
    }

    /// `update()`.
    fn run_update(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        let d = self.data.clone();
        // "Make sure we are "busy" for scripting purposes if the unit is cleaning up an area."
        if self.move_range > 0.0 {
            if let Some(idle) = Self::ai_reply(world, us, ModuleCall::AiIsIdle).map(|r| r.as_bool()) {
                if idle {
                    // "Keep him busy even though he's not moving -- he might be cleaning up."
                    if !crate::logic::modules::plant(2268) {
                        Self::ai_reply(world, us, ModuleCall::AiBusy { command_source: CMD_FROM_AI });
                    }
                } else if Self::ai_reply(world, us, ModuleCall::AiGetLastCommandSource).map_or(0, |r| r.as_int(0)) != CMD_FROM_AI && !crate::logic::modules::plant(2261) {
                    // "Either the player or a script gave a NEW order so abandon the cleanup area cause."
                    self.move_range = 0.0;
                    return Ok(());
                }
            }
        }
        // "Optimized firing at acquired target"
        if self.next_scan_frames > 0 {
            self.next_scan_frames -= 1;
            // "Only happens if something is tracked."
            return self.fire_when_ready(world, us);
        }
        self.next_scan_frames = d.scan_frames as i32;
        // "Periodic scanning (expensive)"
        if self.scan_closest_target(world, us).is_some() {
            // "1 frame can make a big difference so fire ASAP!"
            self.fire_when_ready(world, us)?;
        } else if self.move_range != 0.0 {
            // "There's nothing nearby, so ... set the AI to idle so it can advance to the next script!"
            if Self::ai_idle_or_busy(world, us) == Some(true) {
                let dist = world.contain_partition_get_distance_squared(us, &self.pos).sqrt();
                if dist < if crate::logic::modules::plant(2262) { 2.5 } else { 25.0 } {
                    // "Abort clean area because there's nothing left to clean!"
                    self.move_range = 0.0;
                } else {
                    // "Abort clean area AFTER we move back to our initial position!"
                    Self::ai_reply(world, us, ModuleCall::AiMoveToPosition { pos: self.pos, command_source: CMD_FROM_AI });
                }
            }
        }
        Ok(())
    }

    /// `fireWhenReady()`.
    fn fire_when_ready(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        let d = self.data.clone();
        // "Track our target (this code prevents the object from moving beyond range)"
        let mut target = (self.best_target_id != INVALID_ID && world.objects.find(self.best_target_id).is_some()).then_some(self.best_target_id);
        if let Some(t) = target.filter(|_| self.move_range == 0.0) {
            let fire_range = self.attack_range(world)?;
            let dist = world.partition_get_distance_squared_center_2d(us, t).sqrt();
            if dist < fire_range {
                // "We are currently in range!"
                self.in_range = true;
            } else if self.in_range && !crate::logic::modules::plant(2263) {
                // "the target has moved out of firing range, so re-evaluate by forcing a new scan."
                self.next_scan_frames = world.random.game_logic_random_value(0, if crate::logic::modules::plant(2267) { 1 } else { 3 });
                self.best_target_id = INVALID_ID;
                if self.next_scan_frames == 0 {
                    self.scan_closest_target(world, us);
                    self.next_scan_frames = d.scan_frames as i32;
                    // "Set target to NULL so we don't shoot at it (might be out of range)"
                    target = None;
                }
            } else {
                // "Not in range"
                self.in_range = false;
            }
        }
        if self.next_shot_available_in_frames > 0 {
            // "We can't fire this frame."
            self.next_shot_available_in_frames -= 1;
            return Ok(());
        }
        // "Fire control!"
        if let Some(t) = target {
            if Self::ai_idle_or_busy(world, us) == Some(true) || crate::logic::modules::plant(2264) && world.objects.find(us).is_some_and(|o| o.ai.is_some()) {
                // "lock it just till the weapon is empty or the attack is "done""
                world.set_weapon_lock(us, d.weapon_slot, LOCKED_TEMPORARILY);
                Self::ai_reply(world, us, ModuleCall::AiAttackObject { victim: t, max_shots: NO_MAX_SHOTS_LIMIT, command_source: CMD_FROM_AI });
            }
        }
        Ok(())
    }

    /// `scanClosestTarget()`: the closest hazard (filters `KINDOF_CLEANUP_HAZARD` and the same map status) around the area or the unit.
    fn scan_closest_target(&mut self, world: &mut World, us: ObjectId) -> Option<ObjectId> {
        let d = &self.data;
        self.best_target_id = INVALID_ID;
        let best = if self.move_range > 0.0 {
            // "Look for targets around the target position only (but add scan range and move range)."
            let range = if crate::logic::modules::plant(2265) { d.scan_range } else { d.scan_range + self.move_range };
            world.partition_get_closest_cleanup_hazard_at(us, self.pos, range)
        } else {
            // "Look for targets near me -- passive default."
            let pos = world.objects.find(us).map(|o| o.get_position()).unwrap_or_default();
            world.partition_get_closest_cleanup_hazard_at(us, pos, d.scan_range)
        };
        if let Some(b) = best {
            self.best_target_id = b;
        }
        best
    }

    /// `setCleanupAreaParameters(pos, range)`: "this allows the unit to cleanup an area until clean, then the AI goes idle".
    fn set_cleanup_area_parameters(&mut self, world: &mut World, us: ObjectId, pos: Coord3D, range: f32) {
        self.move_range = range;
        self.pos = pos;
        // "CMD_FROM_AI important because it'll abort when other types are used"
        Self::ai_reply(world, us, ModuleCall::AiMoveToPosition { pos, command_source: CMD_FROM_AI });
    }
}

impl UpdateBehavior for CleanupHazardUpdate {
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
        xfer.xfer_object_id(&mut self.best_target_id)?;
        xfer.xfer_bool(&mut self.in_range)?;
        xfer.xfer_int(&mut self.next_scan_frames)?;
        xfer.xfer_int(&mut self.next_shot_available_in_frames)?;
        if crate::logic::modules::plant(2266) {
            return xfer.xfer_real(&mut self.move_range);
        }
        xfer.xfer_coord3d(&mut self.pos)?;
        xfer.xfer_real(&mut self.move_range)
    }
    /// `onObjectCreated` and `setCleanupAreaParameters`.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnObjectCreated => {
                self.on_object_created(world, me.object);
                ModuleReply::Done
            }
            ModuleCall::SetCleanupAreaParameters { loc, range } => {
                self.set_cleanup_area_parameters(world, me.object, loc, range);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::CleanupHazardUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::CleanupHazardUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for CleanupHazardUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base and the fields.
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
        let d = CleanupHazardUpdateModuleData::default();
        assert_eq!((d.weapon_slot, d.scan_frames, d.scan_range), (0, 0, 0.0));
        let m = CleanupHazardUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words()[..5], [INVALID_ID as u32, 0, 0, 0, 0]);
    }
}
