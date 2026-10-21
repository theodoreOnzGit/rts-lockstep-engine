// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/AutoHealBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/AutoHealBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `AutoHealBehavior`: heals its own object, or every ally in a radius, or every object of its player, by a fixed amount at an interval, while its
//! upgrade is active (`StartsActive`, or an upgrade). It is an update module, an `UpgradeMux` and a `DamageModuleInterface` (`onDamage` wakes a
//! self-healer when the object is hurt), and a sleepy one: it sleeps for ever when nothing is hurt.
//!
//! # Where the world comes from
//!
//! - The three scans are `ThePartitionManager->iterateObjectsInRange(pos, radius, FROM_CENTER_2D, {allies, alive, same map status})` and
//!   `Player::iterateObjects`: both are the lists the test (the full-game adapter, later) puts in `ObjectCuts::range_scan` and
//!   `ObjectCuts::player_object_list`, in the order upstream visits them. The filters are the partition manager's, verified with it.
//! - `getBodyModule()->getHealth()/getMaxHealth()` are `GetHealth`/`GetMaxHealth` calls on the object's body; `attemptHealing` and
//!   `attemptHealingFromSoleBenefactor` are the world's.
//! - The particle systems (`TheParticleSystemManager->createParticleSystem(tmpl)` and `setPosition`) are [`CutCall::ParticleSystemCreate`]; the
//!   template is a name (`None` is `NULL`), the id the world allocates.
//! - The "get healed" icon is [`CutCall::InGameUiAddWorldAnimation`] with `TheGlobalData->m_getHealedAnimation*` (`ObjectGlobals`).
//!
//! # Upstream facts kept
//!
//! - `HealingDelay` defaults to `UINT_MAX`; the constructor's phase is `GameLogicRandomValue(1, delay)` with the delay converted to `Int` (so `-1`
//!   for the default), which `getRandomValue` turns into a draw modulo `UINT_MAX`.
//! - A scanned object without a body module is dereferenced: [`Undefined::NullBodyModule`].
//! - `checkForAutoHeal` compares the two objects' controlling players as pointers (two objects with no controlling player are on the same side).
//! - `m_radiusParticleSystemID` is saved as raw bytes (`xferUser`).
//! - **Open (deviation):** `~AutoHealBehavior` (`destroyParticleSystemByID`) is not sent when the object goes, because the model has no hook for a
//!   module's destruction (as `~SpecialPowerModule`).
//!
//! `p1w12-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::KindOfMaskType;
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DAMAGE, MODULEINTERFACE_UPGRADE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE,
};
use crate::logic::module::upgrade_module::{UpgradeModuleData, UpgradeMux};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::modules::upgrade_mux_user::{give_self_upgrade, upgrade_mux_call, UpgradeMuxUser};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;

/// `INVALID_PARTICLE_SYSTEM_ID`.
pub const INVALID_PARTICLE_SYSTEM_ID: u32 = 0;

/// `class AutoHealBehaviorModuleData : public UpdateModuleData`.
#[derive(Debug)]
pub struct AutoHealBehaviorModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_upgradeMuxData`, in an `UpgradeModuleData` (shared, with the mutable members).
    pub upgrade: Arc<UpgradeModuleData>,
    /// `m_initiallyActive`.
    pub initially_active: bool,
    /// `m_singleBurst`.
    pub single_burst: bool,
    /// `m_healingAmount`.
    pub healing_amount: i32,
    /// `m_healingDelay` (frames; `UINT_MAX` by default).
    pub healing_delay: u32,
    /// `m_startHealingDelay` (frames): how long since the last damage till auto-heal starts.
    pub start_healing_delay: u32,
    /// `m_radius`: if non-zero, an area effect.
    pub radius: f32,
    /// `m_affectsWholePlayer`.
    pub affects_whole_player: bool,
    /// `m_skipSelfForHealing`.
    pub skip_self_for_healing: bool,
    /// `m_kindOf`: only these kinds are healed (all by default).
    pub kind_of: KindOfMaskType,
    /// `m_forbiddenKindOf`.
    pub forbidden_kind_of: KindOfMaskType,
    /// `m_radiusParticleSystemTmpl`, by name (`None` for `NULL`).
    pub radius_particle_system: Option<Vec<u8>>,
    /// `m_unitHealPulseParticleSystemTmpl`, by name (`None` for `NULL`).
    pub unit_heal_pulse_particle_system: Option<Vec<u8>>,
}

impl Default for AutoHealBehaviorModuleData {
    /// `AutoHealBehaviorModuleData()`.
    fn default() -> Self {
        let mut kind_of = KindOfMaskType::new();
        // SET_ALL_KINDOFMASK_BITS: clear, then flip
        kind_of.flip();
        AutoHealBehaviorModuleData {
            update: UpdateModuleData::new(),
            upgrade: Arc::new(UpgradeModuleData::new()),
            initially_active: false,
            single_burst: false,
            healing_amount: 0,
            healing_delay: u32::MAX,
            start_healing_delay: 0,
            radius: 0.0,
            affects_whole_player: false,
            skip_self_for_healing: false,
            kind_of,
            forbidden_kind_of: KindOfMaskType::new(),
            radius_particle_system: None,
            unit_heal_pulse_particle_system: None,
        }
    }
}

/// `INI::parseParticleSystemTemplate`: `None` (or a template the manager does not have) is `NULL`; any other name is looked up when it is used.
fn parse_particle_system(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let token = ini.get_next_token(None)?;
    Ok((!token.eq_ignore_ascii_case(b"None")).then_some(token))
}

/// `UpgradeMuxData::getFieldParse()`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<AutoHealBehaviorModuleData>; 6] = upgrade_module_data_field_parse!(AutoHealBehaviorModuleData, upgrade);

/// `AutoHealBehaviorModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<AutoHealBehaviorModuleData>; 13] = [
    FieldParse::new("StartsActive", |ini, d, _| { d.initially_active = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("SingleBurst", |ini, d, _| { d.single_burst = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("HealingAmount", |ini, d, _| { d.healing_amount = ini.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("HealingDelay", |ini, d, _| { d.healing_delay = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("Radius", |ini, d, _| { d.radius = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("KindOf", |ini, d, _| d.kind_of.parse_from_ini(ini), UserData::None),
    FieldParse::new("ForbiddenKindOf", |ini, d, _| d.forbidden_kind_of.parse_from_ini(ini), UserData::None),
    FieldParse::new("RadiusParticleSystemName", |ini, d, _| { d.radius_particle_system = parse_particle_system(ini)?; Ok(()) }, UserData::None),
    FieldParse::new("UnitHealPulseParticleSystemName", |ini, d, _| { d.unit_heal_pulse_particle_system = parse_particle_system(ini)?; Ok(()) }, UserData::None),
    FieldParse::new("StartHealingDelay", |ini, d, _| { d.start_healing_delay = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("AffectsWholePlayer", |ini, d, _| { d.affects_whole_player = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("SkipSelfForHealing", |ini, d, _| { d.skip_self_for_healing = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl AutoHealBehaviorModuleData {
    /// `buildFieldParse(p)`: the update base, this class's own fields, then the mux's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, AutoHealBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&OWN_FIELD_PARSE)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<AutoHealBehaviorModuleData> {
        let mut data = AutoHealBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, AutoHealBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class AutoHealBehavior : public UpdateModule, public UpgradeMux, public DamageModuleInterface`.
#[derive(Clone, Debug)]
pub struct AutoHealBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getAutoHealBehaviorModuleData()`.
    data: Arc<AutoHealBehaviorModuleData>,
    /// The `UpgradeMux` base.
    mux: UpgradeMux,
    /// `m_radiusParticleSystemID`.
    radius_particle_system_id: u32,
    /// `m_soonestHealFrame`: "I need to record this, because with multiple wake up sources, I can't rely solely on my sleeping."
    soonest_heal_frame: u32,
    /// `m_stopped`.
    stopped: bool,
}

impl PartialEq for AutoHealBehavior {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.mux == other.mux
            && self.radius_particle_system_id == other.radius_particle_system_id
            && self.soonest_heal_frame == other.soonest_heal_frame
            && self.stopped == other.stopped
    }
}

impl AutoHealBehavior {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_UPGRADE | MODULEINTERFACE_DAMAGE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_UPGRADE | MODULEINTERFACE_DAMAGE
    }

    /// The first half of the constructor, which touches no world: the base (processing `DISABLED_HELD` too), the data (`INI_INVALID_DATA` for null
    /// data) and the zeroed state.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<AutoHealBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let mut update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        // getDisabledTypesToProcess(): MAKE_DISABLED_MASK( DISABLED_HELD )
        let mut mask = crate::common::disabled_types::DisabledMaskType::default();
        mask.set(crate::common::disabled_types::DISABLED_HELD, 1);
        update.set_disabled_types_to_process(mask);
        Ok(AutoHealBehavior { update, data, mux: UpgradeMux::new(), radius_particle_system_id: INVALID_PARTICLE_SYSTEM_ID, soonest_heal_frame: 0, stopped: false })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getAutoHealBehaviorModuleData()`).
    pub fn module_data(&self) -> &Arc<AutoHealBehaviorModuleData> {
        &self.data
    }

    /// The module's own state as words, for the tests that compare it with upstream's: the particle system id, the soonest heal frame, the stopped
    /// flag, and whether the upgrade ran.
    pub fn state_words(&self) -> Vec<u32> {
        vec![self.radius_particle_system_id, self.soonest_heal_frame, u32::from(self.stopped), u32::from(self.mux.is_already_upgraded())]
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

    /// `stopHealing()`.
    pub fn stop_healing(&mut self, world: &mut World) {
        self.stopped = true;
        self.soonest_heal_frame = crate::logic::module::update_module::FOREVER;
        if !crate::logic::modules::plant(953) {
            update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
        }
    }

    /// `undoUpgrade()`: "pretend like we have not been activated yet, so we can be reactivated later".
    pub fn undo_upgrade(&mut self) {
        if !crate::logic::modules::plant(954) {
            self.soonest_heal_frame = 0;
        }
        self.mux.set_upgrade_executed(false);
    }

    /// `onDamage(damageInfo)`: "damage has been dealt, this is an opportunity to react to that damage".
    pub fn on_damage(&mut self, world: &mut World) {
        if self.stopped {
            return;
        }
        let d = self.data.clone();
        if self.mux.is_already_upgraded() && d.radius == 0.0 {
            // if this is nonzero, getting damaged resets our healing process. so go to sleep for this long.
            if d.start_healing_delay > 0 {
                update_behavior::set_wake_frame(world, update_sleep(d.start_healing_delay));
            } else if world.frame() > self.soonest_heal_frame || crate::logic::modules::plant(941) {
                // We can only force an immediate wake if we are ready to heal. Otherwise we will heal on a timer AND at every damage input.
                update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
            }
        }
    }

    /// `pulseHealObject(obj)`.
    fn pulse_heal_object(&mut self, world: &mut World, me: ObjectId, target: ObjectId) {
        if self.stopped {
            return;
        }
        let data = self.data.clone();
        let amount = if crate::logic::modules::plant(944) { (data.healing_amount + 1) as f32 } else { data.healing_amount as f32 };
        if data.radius == 0.0 {
            world.attempt_healing(target, amount, me);
        } else if !crate::logic::modules::plant(942) {
            world.attempt_healing_from_sole_benefactor(target, amount, me, data.healing_delay);
        }
        if let Some(tmpl) = data.unit_heal_pulse_particle_system.clone() {
            let position = world.objects.find(target).map_or_else(Default::default, |o| o.get_position());
            world.create_particle_system(tmpl, position);
        }
        self.soonest_heal_frame = world.frame().wrapping_add(if crate::logic::modules::plant(951) { 0 } else { data.healing_delay }); // In case onDamage tries to wake us up early
    }

    /// `checkForAutoHeal(testObj, helper)`: whether the object joins the list of the whole-player scan.
    fn check_for_auto_heal(&self, world: &mut World, me: ObjectId, test_obj: ObjectId) -> Result<bool, IniError> {
        let d = &self.data;
        let Some(o) = world.objects.find(test_obj) else { return Ok(false) };
        if o.is_effectively_dead() {
            return Ok(false);
        }
        if world.get_controlling_player(test_obj) != world.get_controlling_player(me) && !crate::logic::modules::plant(945) {
            return Ok(false);
        }
        if o.is_off_map() && !crate::logic::modules::plant(950) {
            return Ok(false);
        }
        if d.skip_self_for_healing && test_obj == me {
            return Ok(false);
        }
        if !o.is_any_kind_of(&d.kind_of) {
            return Ok(false);
        }
        if o.is_any_kind_of(&d.forbidden_kind_of) && !crate::logic::modules::plant(949) {
            return Ok(false);
        }
        let (health, max_health) = world.body_health(test_obj).ok_or_else(|| IniError::undefined(Undefined::NullBodyModule))?;
        if health >= max_health {
            return Ok(false);
        }
        Ok(true)
    }

    /// The body of `update()` after the guards: the three healing modes.
    fn heal(&mut self, world: &mut World, me: ModuleHandle) -> Result<UpdateSleepTime, IniError> {
        let d = self.data.clone();
        if d.affects_whole_player {
            // Even newer system, I can ignore radius and iterate objects on the owning player. Faster than scanning range 10,000,000
            if let Some(player) = world.get_controlling_player(me.object) {
                let mut objects_to_heal = Vec::new();
                // Smack all objects with this function, and we will end up with a list of Objects deserving of pulseHealObject
                for obj in world.cuts.player_object_list.get(&player).cloned().unwrap_or_default() {
                    if self.check_for_auto_heal(world, me.object, obj)? {
                        objects_to_heal.push(obj);
                    }
                }
                for obj in objects_to_heal {
                    self.pulse_heal_object(world, me.object, obj);
                }
            }
            return Ok(update_sleep(d.healing_delay));
        }
        if d.radius == 0.0 {
            // ORIGINAL SYSTEM -- JUST HEAL SELF!
            // do not heal if we are at max health already
            let (health, max_health) = world.body_health(me.object).ok_or_else(|| IniError::undefined(Undefined::NullBodyModule))?;
            if health < max_health {
                self.pulse_heal_object(world, me.object, me.object);
                return Ok(update_sleep(d.healing_delay));
            }
            // go to sleep forever -- we'll wake back up when we are damaged again
            return Ok(if crate::logic::modules::plant(948) { update_sleep(d.healing_delay) } else { UPDATE_SLEEP_FOREVER });
        }
        // EXPANDED SYSTEM -- HEAL FRIENDLIES IN RADIUS
        for obj in world.cuts.range_scan.clone() {
            // do not heal if we are at max health already
            let (health, max_health) = world.body_health(obj).ok_or_else(|| IniError::undefined(Undefined::NullBodyModule))?;
            if health < max_health {
                let Some(o) = world.objects.find(obj) else { continue };
                if o.is_any_kind_of(&d.kind_of) && !o.is_any_kind_of(&d.forbidden_kind_of) && (!d.skip_self_for_healing || obj != me.object || crate::logic::modules::plant(946)) {
                    self.pulse_heal_object(world, me.object, obj);
                    if d.single_burst && world.cuts.globals.draw_icon_ui {
                        let g = world.cuts.globals;
                        if g.anim2d_collection_present && !world.cuts.get_healed_animation_name.is_empty() {
                            let template = world.cuts.get_healed_animation_name.clone();
                            let (time, rise) = (g.get_healed_animation_display_time_in_seconds, g.get_healed_animation_z_rise_per_second);
                            if let Some(o) = world.objects.find(obj) {
                                let mut position = o.get_position();
                                position.z += o.geometry_info.get_max_height_above_position();
                                world.cut(CutCall::InGameUiAddWorldAnimation { template, position, display_time_in_seconds: time, z_rise_per_second: rise });
                            }
                        }
                    }
                }
            }
        }
        Ok(update_sleep(if d.single_burst && !crate::logic::modules::plant(947) { UPDATE_SLEEP_FOREVER.0 } else { d.healing_delay }))
    }
}

impl UpgradeMuxUser for AutoHealBehavior {
    fn mux(&self) -> &UpgradeMux {
        &self.mux
    }
    fn mux_mut(&mut self) -> &mut UpgradeMux {
        &mut self.mux
    }
    fn mux_data(&self) -> &Arc<UpgradeModuleData> {
        &self.data.upgrade
    }
    /// `upgradeImplementation()`: `setWakeFrame(getObject(), UPDATE_SLEEP_NONE)`.
    fn upgrade_implementation(&mut self, world: &mut World, _me: ModuleHandle) -> Result<(), IniError> {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
        Ok(())
    }
    fn mux_into_body(self) -> UpdateBody {
        UpdateBody::AutoHealBehavior(self)
    }
    fn mux_from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::AutoHealBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl UpdateBehavior for AutoHealBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if self.stopped {
            return UPDATE_SLEEP_FOREVER;
        }
        // do not heal if our status bit is not on. do not heal if our status is effectively dead. There ain't no coming back, man!
        let dead = world.objects.find(me.object).is_some_and(|o| o.is_effectively_dead());
        if !self.mux.is_already_upgraded() || dead {
            return UPDATE_SLEEP_FOREVER;
        }
        match self.heal(world, me) {
            Ok(t) => t,
            Err(e) => {
                world.cuts.module_errors.push((me, e));
                UPDATE_SLEEP_NONE
            }
        }
    }
    fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        let d = self.data.clone();
        if let Some(tmpl) = d.radius_particle_system.clone() {
            let position = world.objects.find(me.object).map_or_else(Default::default, |o| o.get_position());
            self.radius_particle_system_id = world.create_particle_system(tmpl, position);
        }
        if d.initially_active {
            if let Err(e) = give_self_upgrade(self, world, me) {
                world.cuts.module_errors.push((me, e));
                return;
            }
            // start these guys with random phasings so that we don't have all of 'em check on the same frame.
            let delay = d.healing_delay;
            let phase = world.random.game_logic_random_value(if crate::logic::modules::plant(952) { 0 } else { 1 }, delay as i32);
            update_behavior::set_wake_frame(world, update_sleep(phase as u32));
        } else {
            update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
        }
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        // `UpgradeMux::upgradeMuxXfer` (the executed flag) comes before the fields, after the update base
        self.mux.upgrade_mux_xfer(xfer)?;
        // particle system id (`xferUser`: the raw bytes of the id)
        let mut bytes = self.radius_particle_system_id.to_le_bytes();
        xfer.xfer_user(&mut bytes)?;
        self.radius_particle_system_id = u32::from_le_bytes(bytes);
        // Timer safety
        xfer.xfer_unsigned_int(&mut self.soonest_heal_frame)?;
        if crate::logic::modules::plant(943) {
            return Ok(());
        }
        xfer.xfer_bool(&mut self.stopped)
    }
    fn load_post_process_fields(&mut self) {
        self.mux.upgrade_mux_load_post_process();
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::AutoHealBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::AutoHealBehavior(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        if let Some(r) = upgrade_mux_call(self, world, me, call.clone()) {
            return r;
        }
        match call {
            ModuleCall::OnDamage(_) => {
                self.on_damage(world);
                ModuleReply::Done
            }
            ModuleCall::UndoUpgrade => {
                self.undo_upgrade();
                ModuleReply::Done
            }
            ModuleCall::StopHealing => {
                self.stop_healing(world);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for AutoHealBehavior {
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

/// The factory's `NewModuleDataProc` for `AutoHealBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::AutoHealBehavior(Arc::new(AutoHealBehaviorModuleData::friend_new_module_data(ini)?)))
}

