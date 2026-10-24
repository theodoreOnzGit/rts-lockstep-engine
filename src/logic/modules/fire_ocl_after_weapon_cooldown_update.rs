// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/FireOCLAfterWeaponCooldownUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FireOCLAfterWeaponCooldownUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `FireOCLAfterWeaponCooldownUpdate`: counts the consecutive shots of one weapon slot of the object while its upgrade conditions hold, and when the
//! weapon stops firing (or the slot is switched away from) after at least `MinShotsToCreateOCL` shots, creates an object creation list whose
//! lifetime is the time spent firing, scaled by `OCLLifetimePerSecond` and capped at `OCLLifetimeMaxCap`. It is an update module and an `UpgradeMux`
//! (`MODULEINTERFACE_UPGRADE`) that never sleeps.
//!
//! # Where the weapon comes from
//!
//! `getCurrentWeapon()` and `getWeaponInWeaponSlot(slot)` are the object's weapon set. The set of an object is [`Object::weapon_set`]; until the
//! full-game adapter (layer 12) fills it with weapons, the three facts the module reads of the current weapon (its slot, the frame of its last shot and
//! `getPossibleNextShotFrame()`) come from the table `ObjectCuts::cur_weapon`, and "a weapon in this slot" from `ObjectCuts::weapons`
//! ([`World::current_weapon_facts`], [`World::weapon_in_slot_exists`]). The weapon set wins when it has the weapon.
//!
//! `ObjectCreationList::create(ocl, obj, obj, frames)` is [`CutCall::OclCreateWithLifetime`] (the list is a name, found when it is used; a name that is
//! no list makes nothing).
//!
//! # Upstream facts kept
//!
//! - `update()` dereferences `getControllingPlayer()` (`getCompletedUpgradeMask`) before it looks at anything else but the weapon: an object with
//!   no controlling player is [`Undefined::NullControllingPlayer`].
//! - The lifetime is `(UnsignedInt)(seconds * 30)` for `seconds = (now - start) * (1/30) * (OCLLifetimePerSecond * 0.001)`, single precision; a value
//!   outside `UnsignedInt` converts undefinedly ([`Undefined::FloatToUnsignedOutOfRange`]).
//! - The first shot is only noticed when `getLastShotFrame() == now - 1`; the shot counter then restarts the clock at that frame.
//!
//! `p1w12-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::game_common::SECONDS_PER_LOGICFRAME_REAL;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_UPGRADE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::module::upgrade_module::{UpgradeModuleData, UpgradeMux};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::modules::upgrade_mux_user::{upgrade_mux_call, UpgradeMuxUser};
use crate::logic::weapon_fire::PRIMARY_WEAPON;
use crate::logic::weapon_set::WEAPON_SLOT_TYPE_NAMES;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;

/// `class FireOCLAfterWeaponCooldownUpdateModuleData : public UpdateModuleData`.
#[derive(Debug)]
pub struct FireOCLAfterWeaponCooldownUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_upgradeMuxData`, in an `UpgradeModuleData` (shared, with the mutable members).
    pub upgrade: Arc<UpgradeModuleData>,
    /// `m_ocl`, by name (`None` for `NULL`).
    pub ocl: Option<Vec<u8>>,
    /// `m_weaponSlot` (`PRIMARY_WEAPON` by default).
    pub weapon_slot: i32,
    /// `m_minShotsRequired` (1).
    pub min_shots_required: u32,
    /// `m_oclLifetimePerSecond` (1000).
    pub ocl_lifetime_per_second: u32,
    /// `m_oclMaxFrames` (1000).
    pub ocl_max_frames: u32,
}

impl Default for FireOCLAfterWeaponCooldownUpdateModuleData {
    /// `FireOCLAfterWeaponCooldownUpdateModuleData()`.
    fn default() -> Self {
        FireOCLAfterWeaponCooldownUpdateModuleData {
            update: UpdateModuleData::new(),
            upgrade: Arc::new(UpgradeModuleData::new()),
            ocl: None,
            weapon_slot: PRIMARY_WEAPON,
            min_shots_required: 1,
            ocl_lifetime_per_second: 1000,
            ocl_max_frames: 1000,
        }
    }
}

/// `UpgradeMuxData::getFieldParse()`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<FireOCLAfterWeaponCooldownUpdateModuleData>; 6] =
    upgrade_module_data_field_parse!(FireOCLAfterWeaponCooldownUpdateModuleData, upgrade);

/// `FireOCLAfterWeaponCooldownUpdateModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<FireOCLAfterWeaponCooldownUpdateModuleData>; 6] = [
    FieldParse::new("WeaponSlot", |ini, d, _| { d.weapon_slot = ini.parse_index_list(&WEAPON_SLOT_TYPE_NAMES)?; Ok(()) }, UserData::None),
    FieldParse::new(
        "OCL",
        |ini, d, _| {
            // INI::parseObjectCreationList: `None` is no list; any other name is looked up (here, when it is used)
            let token = ini.get_next_token(None)?;
            d.ocl = (!token.eq_ignore_ascii_case(b"None")).then_some(token);
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new("MinShotsToCreateOCL", |ini, d, _| { d.min_shots_required = ini.parse_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("OCLLifetimePerSecond", |ini, d, _| { d.ocl_lifetime_per_second = ini.parse_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("OCLLifetimeMaxCap", |ini, d, _| { d.ocl_max_frames = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl FireOCLAfterWeaponCooldownUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, this class's own fields, then the mux's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, FireOCLAfterWeaponCooldownUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&OWN_FIELD_PARSE)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<FireOCLAfterWeaponCooldownUpdateModuleData> {
        let mut data = FireOCLAfterWeaponCooldownUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, FireOCLAfterWeaponCooldownUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class FireOCLAfterWeaponCooldownUpdate : public UpdateModule, public UpgradeMux`.
#[derive(Clone, Debug)]
pub struct FireOCLAfterWeaponCooldownUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getFireOCLAfterWeaponCooldownUpdateModuleData()`.
    data: Arc<FireOCLAfterWeaponCooldownUpdateModuleData>,
    /// The `UpgradeMux` base.
    mux: UpgradeMux,
    /// `m_valid`.
    valid: bool,
    /// `m_consecutiveShots`.
    consecutive_shots: u32,
    /// `m_startFrame`.
    start_frame: u32,
}

impl PartialEq for FireOCLAfterWeaponCooldownUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.mux == other.mux
            && self.valid == other.valid
            && self.consecutive_shots == other.consecutive_shots
            && self.start_frame == other.start_frame
    }
}

impl FireOCLAfterWeaponCooldownUpdate {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_UPGRADE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_UPGRADE
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and `m_valid = false;
    /// resetStats()`.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<FireOCLAfterWeaponCooldownUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(FireOCLAfterWeaponCooldownUpdate { update, data, mux: UpgradeMux::new(), valid: false, consecutive_shots: 0, start_frame: 0 })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getFireOCLAfterWeaponCooldownUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<FireOCLAfterWeaponCooldownUpdateModuleData> {
        &self.data
    }

    /// The module's own state as words, for the tests that compare it with upstream's: the valid flag, the shot count, the start frame, and whether the
    /// upgrade ran.
    pub fn state_words(&self) -> Vec<u32> {
        vec![u32::from(self.valid), self.consecutive_shots, self.start_frame, u32::from(self.mux.is_already_upgraded())]
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

    /// `xfer` with the scheduling half the slot holds: the version, that half, then the fields.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    /// `crc` with the scheduling half the slot holds: `UpdateModule::crc`, then `upgradeMuxCRC`.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)?;
        self.mux.upgrade_mux_crc(xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// `resetStats()`.
    fn reset_stats(&mut self) {
        self.consecutive_shots = 0;
        self.start_frame = 0;
    }

    /// `fireOCL()`: the list, with a lifetime of the time spent firing.
    fn fire_ocl(&mut self, world: &mut World, me: ObjectId) -> Result<(), IniError> {
        let data = self.data.clone();
        // Calculate the lifetime of the OCL.
        let now = world.frame();
        let mut seconds = now.wrapping_sub(self.start_frame) as f32 * SECONDS_PER_LOGICFRAME_REAL;
        seconds *= data.ocl_lifetime_per_second as f32 * if crate::logic::modules::plant(921) { 0.01 } else { 0.001 };
        let frames = seconds * crate::common::game_common::LOGICFRAMES_PER_SECOND as f32;
        if !(0.0..4_294_967_296.0).contains(&frames) {
            return Err(IniError::undefined(Undefined::FloatToUnsignedOutOfRange));
        }
        let ocl_frames = if crate::logic::modules::plant(922) { frames as u32 } else { (frames as u32).min(data.ocl_max_frames) };

        if let Some(ocl) = data.ocl.clone() {
            world.cut(CutCall::OclCreateWithLifetime { primary: me, secondary: me, ocl, lifetime_frames: ocl_frames });
        }
        self.reset_stats();
        Ok(())
    }
}

impl UpgradeMuxUser for FireOCLAfterWeaponCooldownUpdate {
    fn mux(&self) -> &UpgradeMux {
        &self.mux
    }
    fn mux_mut(&mut self) -> &mut UpgradeMux {
        &mut self.mux
    }
    fn mux_data(&self) -> &Arc<UpgradeModuleData> {
        &self.data.upgrade
    }
    /// `upgradeImplementation()`: "nothing!".
    fn upgrade_implementation(&mut self, _world: &mut World, _me: ModuleHandle) -> Result<(), IniError> {
        Ok(())
    }
    fn mux_into_body(self) -> UpdateBody {
        UpdateBody::FireOCLAfterWeaponCooldownUpdate(self)
    }
    fn mux_from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::FireOCLAfterWeaponCooldownUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl FireOCLAfterWeaponCooldownUpdate {
    /// The body of `update()`, with the throws and the undefined behaviour as errors.
    fn update_checked(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        let data = self.data.clone();
        let statics = world.statics.clone();
        // getUpgradeActivationMasks( activation, conflicting );
        data.upgrade.mux_data_mut().get_upgrade_activation_masks(&statics.upgrade_center, &mut world.names)?;
        let mut valid_this_frame = true;
        let mut valid_to_fire_ocl = true;

        // Get current weapon
        let weapon = world.current_weapon_facts(me.object);
        match &weapon {
            Some(w) => {
                if w.slot != data.weapon_slot && !crate::logic::modules::plant(926) {
                    // Not the right weapon slot -- it's possible we switched weapons in which case it's possible to fire the OCL.
                    valid_this_frame = false;
                }
            }
            None => {
                // No weapon selected -- possible to switch off weapons, possible to have just finished firing our weapon.
                valid_this_frame = false;
            }
        }

        let object_mask = world.objects.find(me.object).map(|o| o.object_upgrades_completed).unwrap_or_default();
        let player = world.get_controlling_player(me.object).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        let player_mask = world.cuts.player_completed_upgrade_mask(player);
        let mut mask_to_check = player_mask;
        if !crate::logic::modules::plant(929) {
            mask_to_check.set_mask(&object_mask);
        }
        if valid_this_frame {
            let holder = data.upgrade.clone();
            let mut mux_data = holder.mux_data_mut();
            if !self.mux.test_upgrade_conditions(&mut mux_data, &statics.upgrade_center, &mut world.names, mask_to_check)? {
                // Can't use this period if this object doesn't have any of the upgrades
                valid_this_frame = false;
                valid_to_fire_ocl = crate::logic::modules::plant(930);
            }
        }

        let now = world.frame();
        if valid_this_frame {
            let w = weapon.expect("a valid frame has a weapon");
            if w.last_shot_frame == if crate::logic::modules::plant(927) { now } else { now.wrapping_sub(1) } {
                self.consecutive_shots += 1;
                if self.consecutive_shots == 1 && !crate::logic::modules::plant(928) {
                    // Our first shot -- so record the frame number so we can easily calculate the time we've been firing!
                    self.start_frame = now;
                }
            } else if w.possible_next_shot_frame < now {
                // Means we could have shot but didn't!
                if data.min_shots_required <= self.consecutive_shots || crate::logic::modules::plant(923) {
                    // We have fired enough shots to create the OCL.
                    self.fire_ocl(world, me.object)?;
                }
            }
        } else if valid_to_fire_ocl {
            // We've stopped being valid -- which means we may have stopped firing our weapon.
            if (world.weapon_in_slot_exists(me.object, data.weapon_slot) || crate::logic::modules::plant(931)) && data.min_shots_required <= self.consecutive_shots {
                // We switched weapons! Fire OCL!
                self.fire_ocl(world, me.object)?;
            }
        }

        if valid_this_frame != self.valid && !crate::logic::modules::plant(924) {
            // There has been a validity change, so reset all values!
            self.valid = valid_this_frame;
            self.reset_stats();
        }
        Ok(())
    }
}

impl UpdateBehavior for FireOCLAfterWeaponCooldownUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if let Err(e) = self.update_checked(world, me) {
            world.cuts.module_errors.push((me, e));
        }
        UPDATE_SLEEP_NONE
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        // `UpgradeMux::upgradeMuxXfer` (the executed flag) comes before the fields, after the update base
        self.mux.upgrade_mux_xfer(xfer)?;
        xfer.xfer_bool(&mut self.valid)?;
        xfer.xfer_unsigned_int(&mut self.consecutive_shots)?;
        if crate::logic::modules::plant(925) {
            return Ok(());
        }
        xfer.xfer_unsigned_int(&mut self.start_frame)
    }
    fn load_post_process_fields(&mut self) {
        self.mux.upgrade_mux_load_post_process();
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::FireOCLAfterWeaponCooldownUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::FireOCLAfterWeaponCooldownUpdate(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        if let Some(r) = upgrade_mux_call(self, world, me, call) {
            return r;
        }
        ModuleReply::Unhandled
    }
}

impl Snapshot for FireOCLAfterWeaponCooldownUpdate {
    /// `crc`: `UpdateModule::crc`, then `upgradeMuxCRC`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)?;
        self.mux.upgrade_mux_crc(xfer)
    }
    /// `xfer`: the version, the base, the mux, then the fields.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.update.clone();
        let r = update_behavior::xfer_own(self, &mut base, xfer);
        self.update = base;
        r
    }
    fn load_post_process(&mut self) {
        self.update.load_post_process();
        self.mux.upgrade_mux_load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `FireOCLAfterWeaponCooldownUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::FireOCLAfterWeaponCooldownUpdate(Arc::new(FireOCLAfterWeaponCooldownUpdateModuleData::friend_new_module_data(ini)?)))
}
