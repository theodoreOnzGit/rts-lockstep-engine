// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SpecialAbilityUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SpecialAbilityUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SpecialAbilityUpdate`: a unit's special ability (a special power update module), started by its special power module
//! (`initiateIntentToDoSpecialPower`, at an object, a position or neither) and run by `update`: approach the target until within
//! `StartAbilityRange` (in line of sight if asked, touching it for a range under a quarter cell), face it, unpack, prepare for
//! `PreparationTime` (a laser or a hacker's stream as a special object, a capture's flag and the target's flashing), trigger the effect, then pack
//! and finish (flee `FleeRangeAfterCompletion`, away from the unit's own mines). A persistent ability prepares and triggers again and again. The
//! effects: the laser-guided missiles' attack, the napalm bomb, a timed charge or booby trap or TNT on the target, the remote charges placed or
//! all detonated, a hack that disables the target (with its particle effect), a capture (the target defects, or a garrison is emptied), the
//! cash steal (up to 1000), and the disguise. A command from anyone else, the unit's death, a dead or changed target or no special power module
//! ends it.
//!
//! # Upstream facts kept
//!
//! - `isPowerCurrentlyInUse` is asked by `update` with no command button (the control bar's question, with one, is the client's).
//! - The target is checked through fall-throughs: a capture or hack also gives up on a stealthed, undetected target before its preparation is
//!   complete; the laser-guided missiles give up on a structure and on any stealthed, undetected target.
//! - `initiateIntentToDoSpecialPower` resets the ability's state and clears the model conditions before it learns the unit has no AI (and then
//!   refuses); a special ability of another of the seven exclusive types on the unit is ended (`onExit(FALSE)`): `findSpecialAbilityUpdate`
//!   answers the first module of the type, so when that is this one no other is ended.
//! - The pack and unpack times are `time * GameLogicRandomValueReal(1 - f, 1 + f)` truncated to frames.
//! - The capture's flashing phase grows by a third of the preparation's completed fraction each frame, and flashes (and ticks) when it leaves an
//!   odd whole number.
//! - The cash steal takes `min(1000, cash)`, as unsigned.
//! - `xfer` extends `UpdateModule`'s (version 1); the prep sound's playing handle is not saved.
//!
//! # Deviations
//!
//! The destructor (`onExit(TRUE)`) is not run: the world has no hook for a module's destruction.
//!
//! # Where upstream is undefined
//!
//! A `NULL` special power template read for its type (`NullSpecialPowerTemplate`); a cash steal from a target or by a unit no player controls
//! (`NullControllingPlayer`); a capture by a unit no player controls (`NullControllingPlayer`). Recorded in `world.cuts.module_errors`.
//!
//! `p1w13-sonnet`, batch R81, 2026-10-30.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_common::Relationship;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::{KINDOF_MINE, KINDOF_STRUCTURE};
use crate::common::model_state::{MODELCONDITION_FIRING_A, MODELCONDITION_PACKING, MODELCONDITION_RAISING_FLAG, MODELCONDITION_UNPACKING};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::{OBJECT_STATUS_BOOBY_TRAPPED, OBJECT_STATUS_DETECTED, OBJECT_STATUS_IS_USING_ABILITY, OBJECT_STATUS_STEALTHED};
use crate::common::special_power_type::*;
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::special_power_update_module::SpecialPowerUpdateModule;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object::ModuleIndex;
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::{EvaMessage, ObjectClientEvent, ObjectSound, RadarCall};
use crate::logic::world::object_state::mask_of;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `SPECIAL_ABILITY_HUGE_DISTANCE`.
pub const SPECIAL_ABILITY_HUGE_DISTANCE: f32 = 10_000_000.0;
/// `CMD_FROM_AI`.
const CMD_FROM_AI: i32 = 2;
/// `PATHFIND_CELL_SIZE_F`.
const PATHFIND_CELL_SIZE_F: f32 = 10.0;
/// `NO_MAX_SHOTS_LIMIT`.
const NO_MAX_SHOTS_LIMIT: i32 = 0x7FFF_FFFF;
/// `PRIMARY_WEAPON`, `SECONDARY_WEAPON`; `LOCKED_TEMPORARILY`.
const PRIMARY_WEAPON: i32 = 0;
const SECONDARY_WEAPON: i32 = 1;
const LOCKED_TEMPORARILY: i32 = 1;
/// `FROM_CENTER_2D`, `FROM_BOUNDINGSPHERE_2D`.
const FROM_CENTER_2D: i32 = 0;

/// `enum PackingState`.
pub const STATE_NONE: i32 = 0;
pub const STATE_PACKING: i32 = 1;
pub const STATE_UNPACKING: i32 = 2;
pub const STATE_PACKED: i32 = 3;
pub const STATE_UNPACKED: i32 = 4;

/// `class SpecialAbilityUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug)]
pub struct SpecialAbilityUpdateModuleData {
    pub update: UpdateModuleData,
    /// `m_specialObjectName`, `m_specialObjectAttachToBoneName`.
    pub special_object_name: Vec<u8>,
    pub special_object_attach_to_bone_name: Vec<u8>,
    /// `m_specialPowerTemplate`, by name (found in the store when used; an unknown name, and none, is `NULL`).
    pub special_power_template: Option<Vec<u8>>,
    pub start_ability_range: f32,
    pub ability_abort_range: f32,
    pub pack_unpack_variation_factor: f32,
    pub flee_range_after_completion: f32,
    pub effect_value: i32,
    pub award_xp_for_triggering: i32,
    pub skill_points_for_triggering: i32,
    pub preparation_frames: u32,
    pub persistent_prep_frames: u32,
    pub effect_duration: u32,
    pub max_special_objects: u32,
    pub pack_time: u32,
    pub unpack_time: u32,
    pub pre_trigger_unstealth_frames: u32,
    pub skip_packing_with_no_target: bool,
    pub special_objects_persistent: bool,
    pub unique_special_object_targets: bool,
    pub special_objects_persist_when_owner_dies: bool,
    pub flip_object_after_packing: bool,
    pub flip_object_after_unpacking: bool,
    pub always_validate_special_objects: bool,
    pub do_capture_fx: bool,
    pub lose_stealth_on_trigger: bool,
    pub approach_requires_los: bool,
    pub need_to_face_target: bool,
    pub persistence_requires_recharge: bool,
    /// `m_disableFXParticleSystem`, by name (`None`: `NULL`).
    pub disable_fx_particle_system: Option<Vec<u8>>,
    /// The four events' names (empty: none).
    pub pack_sound: Vec<u8>,
    pub unpack_sound: Vec<u8>,
    pub prep_sound_loop: Vec<u8>,
    pub trigger_sound: Vec<u8>,
}

impl Default for SpecialAbilityUpdateModuleData {
    /// `SpecialAbilityUpdateModuleData()`.
    fn default() -> Self {
        SpecialAbilityUpdateModuleData {
            update: UpdateModuleData::default(),
            special_object_name: Vec::new(),
            special_object_attach_to_bone_name: Vec::new(),
            special_power_template: None,
            start_ability_range: SPECIAL_ABILITY_HUGE_DISTANCE,
            ability_abort_range: SPECIAL_ABILITY_HUGE_DISTANCE,
            pack_unpack_variation_factor: 0.0,
            flee_range_after_completion: 0.0,
            effect_value: 1,
            award_xp_for_triggering: 0,
            skill_points_for_triggering: -1,
            preparation_frames: 0,
            persistent_prep_frames: 0,
            effect_duration: 0,
            max_special_objects: 1,
            pack_time: 0,
            unpack_time: 0,
            pre_trigger_unstealth_frames: 0,
            skip_packing_with_no_target: false,
            special_objects_persistent: false,
            unique_special_object_targets: false,
            special_objects_persist_when_owner_dies: false,
            flip_object_after_packing: false,
            flip_object_after_unpacking: false,
            always_validate_special_objects: false,
            do_capture_fx: false,
            lose_stealth_on_trigger: false,
            approach_requires_los: true,
            need_to_face_target: true,
            persistence_requires_recharge: false,
            disable_fx_particle_system: None,
            pack_sound: Vec::new(),
            unpack_sound: Vec::new(),
            prep_sound_loop: Vec::new(),
            trigger_sound: Vec::new(),
        }
    }
}

/// `INI::parseAudioEventRTS`: the event's name, unless the token is `NoSound` (the event is left as it was).
fn parse_sound(ini: &mut Ini<'_>, store: &mut Vec<u8>) -> IniResult<()> {
    let token = ini.get_next_token(None)?;
    if !token.eq_ignore_ascii_case(b"NoSound") {
        *store = token;
    }
    Ok(())
}

/// `INI::parseSpecialPowerTemplate` / `parseParticleSystemTemplate`: the name (`None`: `NULL`), looked up when used.
fn parse_named(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let token = ini.get_next_token(None)?;
    Ok((!token.eq_ignore_ascii_case(b"None")).then_some(token))
}

/// `SpecialAbilityUpdateModuleData::buildFieldParse`'s table (`ApproachRequiresLOS` twice, as upstream's).
static FIELD_PARSE: [FieldParse<SpecialAbilityUpdateModuleData>; 36] = [
    FieldParse::new("SpecialPowerTemplate", |i, d, _| { d.special_power_template = parse_named(i)?; Ok(()) }, UserData::None),
    FieldParse::new("StartAbilityRange", |i, d, _| { d.start_ability_range = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("AbilityAbortRange", |i, d, _| { d.ability_abort_range = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("PreparationTime", |i, d, _| { d.preparation_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("PersistentPrepTime", |i, d, _| { d.persistent_prep_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("PackTime", |i, d, _| { d.pack_time = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("UnpackTime", |i, d, _| { d.unpack_time = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("PreTriggerUnstealthTime", |i, d, _| { d.pre_trigger_unstealth_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("SkipPackingWithNoTarget", |i, d, _| { d.skip_packing_with_no_target = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("PackUnpackVariationFactor", |i, d, _| { d.pack_unpack_variation_factor = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("SpecialObject", |i, d, _| { d.special_object_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("SpecialObjectAttachToBone", |i, d, _| { d.special_object_attach_to_bone_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("MaxSpecialObjects", |i, d, _| { d.max_special_objects = i.parse_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("SpecialObjectsPersistent", |i, d, _| { d.special_objects_persistent = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("EffectDuration", |i, d, _| { d.effect_duration = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("EffectValue", |i, d, _| { d.effect_value = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("UniqueSpecialObjectTargets", |i, d, _| { d.unique_special_object_targets = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("SpecialObjectsPersistWhenOwnerDies", |i, d, _| { d.special_objects_persist_when_owner_dies = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("AlwaysValidateSpecialObjects", |i, d, _| { d.always_validate_special_objects = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("FlipOwnerAfterPacking", |i, d, _| { d.flip_object_after_packing = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("FlipOwnerAfterUnpacking", |i, d, _| { d.flip_object_after_unpacking = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("FleeRangeAfterCompletion", |i, d, _| { d.flee_range_after_completion = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("DisableFXParticleSystem", |i, d, _| { d.disable_fx_particle_system = parse_named(i)?; Ok(()) }, UserData::None),
    FieldParse::new("DoCaptureFX", |i, d, _| { d.do_capture_fx = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("PackSound", |i, d, _| parse_sound(i, &mut d.pack_sound), UserData::None),
    FieldParse::new("UnpackSound", |i, d, _| parse_sound(i, &mut d.unpack_sound), UserData::None),
    FieldParse::new("PrepSoundLoop", |i, d, _| parse_sound(i, &mut d.prep_sound_loop), UserData::None),
    FieldParse::new("TriggerSound", |i, d, _| parse_sound(i, &mut d.trigger_sound), UserData::None),
    FieldParse::new("LoseStealthOnTrigger", |i, d, _| { d.lose_stealth_on_trigger = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("AwardXPForTriggering", |i, d, _| { d.award_xp_for_triggering = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("SkillPointsForTriggering", |i, d, _| { d.skill_points_for_triggering = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ApproachRequiresLOS", |i, d, _| { d.approach_requires_los = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("ApproachRequiresLOS", |i, d, _| { d.approach_requires_los = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("NeedToFaceTarget", |i, d, _| { d.need_to_face_target = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("PersistenceRequiresRecharge", |i, d, _| { d.persistence_requires_recharge = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl SpecialAbilityUpdateModuleData {
    /// `buildFieldParse(p)`: `UpdateModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, SpecialAbilityUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<SpecialAbilityUpdateModuleData> {
        let mut data = SpecialAbilityUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, SpecialAbilityUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The module factory's data proc.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::SpecialAbilityUpdate(Arc::new(SpecialAbilityUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `class SpecialAbilityUpdate : public SpecialPowerUpdateModule`.
#[derive(Clone, Debug)]
pub struct SpecialAbilityUpdate {
    base: SpecialPowerUpdateModule,
    data: Arc<SpecialAbilityUpdateModuleData>,
    /// `m_prepSoundLoop`'s playing handle (the event is the data's, with the object's id, once preparation starts).
    prep_sound_handle: u32,
    /// `m_prepFrames`, `m_animFrames`.
    prep_frames: u32,
    anim_frames: u32,
    /// `m_targetID`, `m_targetPos`, `m_locationCount`.
    target_id: ObjectId,
    target_pos: Coord3D,
    location_count: i32,
    /// `m_specialObjectIDList`, `m_specialObjectEntries`.
    special_object_ids: Vec<ObjectId>,
    special_object_entries: u32,
    /// `m_captureFlashPhase`.
    capture_flash_phase: f32,
    /// `m_packingState`.
    packing_state: i32,
    /// `m_active`, `m_noTargetCommand`, `m_facingInitiated`, `m_facingComplete`, `m_withinStartAbilityRange`, `m_doDisableFXParticles`.
    active: bool,
    no_target_command: bool,
    facing_initiated: bool,
    facing_complete: bool,
    within_start_ability_range: bool,
    do_disable_fx_particles: bool,
}

impl PartialEq for SpecialAbilityUpdate {
    fn eq(&self, other: &Self) -> bool {
        self.base.update().behavior().object() == other.base.update().behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

/// `target->testStatus(STEALTHED) && !target->testStatus(DETECTED)`.
fn stealthed_undetected(world: &World, id: ObjectId) -> bool {
    world.objects.find(id).is_some_and(|o| o.test_status(OBJECT_STATUS_STEALTHED) && !o.test_status(OBJECT_STATUS_DETECTED))
}

impl SpecialAbilityUpdate {
    /// `getInterfaceMask()`: `SpecialPowerUpdateModule`'s.
    pub const fn get_interface_mask() -> i32 {
        SpecialPowerUpdateModule::get_interface_mask()
    }

    /// The constructor's first half.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<SpecialAbilityUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = SpecialPowerUpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        Ok(SpecialAbilityUpdate {
            base,
            data,
            prep_sound_handle: 0,
            prep_frames: 0,
            anim_frames: 0,
            target_id: INVALID_ID,
            target_pos: Coord3D::default(),
            location_count: 0,
            special_object_ids: Vec::new(),
            special_object_entries: 0,
            capture_flash_phase: 0.0,
            packing_state: STATE_NONE,
            active: false,
            no_target_command: false,
            facing_initiated: false,
            facing_complete: false,
            within_start_ability_range: false,
            // "true always, unless small unit causes it to toggle on-off"
            do_disable_fx_particles: true,
        })
    }

    pub fn update_module(&self) -> &UpdateModule {
        self.base.update()
    }

    /// The state the test compares: the prep sound's handle, the frames, the target and its position, the location count, the special
    /// objects (their count, then the list's ids), the flash phase, the packing state, and the six flags (a word, bit 0 `m_active` .. bit 5
    /// `m_doDisableFXParticles`).
    pub fn state_words(&self) -> Vec<u32> {
        let mut w = vec![
            self.prep_sound_handle,
            self.prep_frames,
            self.anim_frames,
            self.target_id as u32,
            self.target_pos.x.to_bits(),
            self.target_pos.y.to_bits(),
            self.target_pos.z.to_bits(),
            self.location_count as u32,
            self.special_object_entries,
            self.special_object_ids.len() as u32,
        ];
        w.extend(self.special_object_ids.iter().map(|&i| i as u32));
        w.push(self.capture_flash_phase.to_bits());
        w.push(self.packing_state as u32);
        let flags = [self.active, self.no_target_command, self.facing_initiated, self.facing_complete, self.within_start_ability_range, self.do_disable_fx_particles];
        w.push(flags.iter().enumerate().fold(0u32, |a, (i, &f)| a | (u32::from(f) << i)));
        w
    }

    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half: `setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER)`.
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

    /// `calcSleepTime()`.
    fn calc_sleep_time(&self) -> UpdateSleepTime {
        if self.active || self.data.always_validate_special_objects {
            UPDATE_SLEEP_NONE
        } else {
            UPDATE_SLEEP_FOREVER
        }
    }

    /// `m_specialPowerTemplate`'s id in the store (`None`: `NULL`).
    fn template_id(&self, world: &World) -> Option<u32> {
        self.data.special_power_template.as_deref().and_then(|n| world.statics.special_power_store.find_special_power_template(n)).map(|t| t.id())
    }

    /// `m_specialPowerTemplate->getSpecialPowerType()` (`NULL`: undefined).
    fn power_type(&self, world: &World) -> Result<i32, Undefined> {
        self.data
            .special_power_template
            .as_deref()
            .and_then(|n| world.statics.special_power_store.find_special_power_template(n))
            .map(|t| t.special_power_type())
            .ok_or(Undefined::NullSpecialPowerTemplate)
    }

    /// `getMySPM()`: the object's special power module for the template.
    fn get_my_spm(&self, world: &mut World, us: ObjectId) -> Option<ModuleIndex> {
        let template = self.template_id(world);
        world.get_special_power_module(us, template)
    }

    fn ai_of(world: &World, id: ObjectId) -> Option<ModuleIndex> {
        world.objects.find(id).and_then(|o| o.ai)
    }

    fn ai_call(world: &mut World, us: ObjectId, call: ModuleCall) -> ModuleReply {
        match Self::ai_of(world, us) {
            Some(ai) => world.module_call(us, ai, call),
            None => ModuleReply::Unhandled,
        }
    }

    fn exists(world: &World, id: ObjectId) -> bool {
        world.objects.find(id).is_some()
    }

    fn has_target_pos(&self) -> bool {
        self.target_pos.x != 0.0 || self.target_pos.y != 0.0 || self.target_pos.z != 0.0
    }

    /// `isPowerCurrentlyInUse(NULL)`.
    fn is_power_currently_in_use(&self) -> bool {
        self.packing_state != STATE_NONE && self.within_start_ability_range
    }

    /// The four model conditions the ability uses.
    fn clear_ability_conditions(world: &mut World, us: ObjectId) {
        world.clear_model_condition_flags(us, &[MODELCONDITION_UNPACKING, MODELCONDITION_PACKING, MODELCONDITION_FIRING_A, MODELCONDITION_RAISING_FLAG]);
    }

    /// `onExit(cleanup)`.
    fn on_exit(&mut self, world: &mut World, us: ObjectId, cleanup: bool) -> Result<(), Undefined> {
        let d = self.data.clone();
        Self::clear_ability_conditions(world, us);
        world.set_status(us, mask_of(OBJECT_STATUS_IS_USING_ABILITY), false);
        world.audio_remove_event(self.prep_sound_handle);
        self.end_preparation(world, us)?;
        if !d.special_objects_persistent || cleanup && !d.special_objects_persist_when_owner_dies {
            // "Delete special objects that aren't considered persistent whenever we turn off"
            self.kill_special_objects(world, us)?;
        }
        self.active = false;
        self.within_start_ability_range = false;
        self.packing_state = STATE_NONE;
        Ok(())
    }

    /// `update()`.
    fn run_update(&mut self, world: &mut World, me: ModuleHandle) -> Result<UpdateSleepTime, Undefined> {
        let us = me.object;
        self.validate_special_objects(world);
        if world.objects.find(us).is_some_and(|o| o.is_effectively_dead()) {
            self.on_exit(world, us, true)?;
            return Ok(self.calc_sleep_time());
        }
        if !self.active {
            return Ok(self.calc_sleep_time());
        }
        if Self::ai_of(world, us).is_none() {
            self.on_exit(world, us, false)?;
            return Ok(self.calc_sleep_time());
        }
        if Self::ai_call(world, us, ModuleCall::AiGetLastCommandSource).as_int(0) != CMD_FROM_AI {
            self.on_exit(world, us, false)?;
            return Ok(self.calc_sleep_time());
        }
        if Self::ai_call(world, us, ModuleCall::AiIsMoving).as_bool() && self.is_power_currently_in_use() && !self.facing_initiated {
            // "Capture is broken by movement just as if we had been given a direct command"
            let t = self.power_type(world)?;
            if t == SPECIAL_INFANTRY_CAPTURE_BUILDING || t == SPECIAL_BLACKLOTUS_CAPTURE_BUILDING {
                self.on_exit(world, us, false)?;
                return Ok(self.calc_sleep_time());
            }
        }
        if self.handle_packing_processing(world, us)? {
            return Ok(self.calc_sleep_time());
        }
        let mut should_abort = false;
        if self.target_id != INVALID_ID && Self::exists(world, self.target_id) {
            let target = self.target_id;
            if world.objects.find(target).is_some_and(|o| o.is_effectively_dead()) {
                should_abort = true;
            } else {
                let t = self.power_type(world)?;
                let same_team = || world.objects.find(target).map(|o| o.team) == world.objects.find(us).map(|o| o.team);
                match t {
                    SPECIAL_INFANTRY_CAPTURE_BUILDING | SPECIAL_BLACKLOTUS_CAPTURE_BUILDING | SPECIAL_HACKER_DISABLE_BUILDING | SPECIAL_BLACKLOTUS_STEAL_CASH_HACK | SPECIAL_BOOBY_TRAP => {
                        let captures = t != SPECIAL_BLACKLOTUS_STEAL_CASH_HACK && t != SPECIAL_BOOBY_TRAP;
                        if captures && same_team() {
                            // "it's been captured by a colleague! we should stop." (then falls through)
                            should_abort = true;
                        }
                        if stealthed_undetected(world, target) && !self.is_preparation_complete() {
                            should_abort = true;
                        }
                    }
                    SPECIAL_REMOTE_CHARGES | SPECIAL_TIMED_CHARGES => {
                        if !self.need_to_unpack() && stealthed_undetected(world, target) && !self.is_preparation_complete() {
                            should_abort = true;
                        }
                    }
                    SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES | SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK => {
                        if t == SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES && world.objects.find(target).is_some_and(|o| o.is_kind_of(KINDOF_STRUCTURE)) {
                            should_abort = true;
                        }
                        if stealthed_undetected(world, target) {
                            // "where'd my target go? 'Twas here just a second ago."
                            should_abort = true;
                        }
                    }
                    _ => {}
                }
            }
        }
        let spm = self.get_my_spm(world, us);
        let Some(spm) = spm.filter(|_| !should_abort) else {
            // "doh, a colleague has already captured it. just stop."
            Self::ai_call(world, us, ModuleCall::AiIdle);
            self.on_exit(world, us, false)?;
            return Ok(self.calc_sleep_time());
        };
        let d = self.data.clone();
        if !self.is_preparation_complete() {
            let mut spm_ready = true;
            if self.is_persistent_ability() && d.persistence_requires_recharge {
                // "unless I intend to persist in this ability's effect, whereupon I must verify that power is recharged"
                spm_ready = world.module_call(us, spm, ModuleCall::IsReady).as_bool() && (world.module_call(us, spm, ModuleCall::GetReadyFrame).as_int(0) as u32) < world.frame();
            }
            if spm_ready && !crate::logic::modules::plant(3811) {
                self.prep_frames -= 1;
            }
            if self.is_preparation_complete() {
                // "STEP 4 -- TRIGGER (with preparation)"
                self.trigger_ability_effect(world, us)?;
                if self.is_persistent_ability() {
                    // "VARIATION -- PERSISTENCE (repeats preparation)"
                    self.reset_preparation();
                    if d.persistence_requires_recharge {
                        world.module_call(us, spm, ModuleCall::StartPowerRecharge);
                    }
                } else {
                    self.end_preparation(world, us)?;
                    if self.need_to_pack() {
                        self.start_packing(world, us, true)?;
                    } else {
                        self.finish_ability(world, us)?;
                    }
                }
            } else if !self.continue_preparation(world, us)? {
                // "We failed so abort!"
                self.end_preparation(world, us)?;
                if self.need_to_pack() {
                    self.start_packing(world, us, false)?;
                } else {
                    self.finish_ability(world, us)?;
                }
            }
        } else if self.is_within_start_ability_range(world, us) {
            self.within_start_ability_range = true;
            if !self.is_facing(world, us) && self.need_to_face(world, us) {
                self.start_facing(world, us);
                return Ok(self.calc_sleep_time());
            }
            if self.need_to_unpack() {
                self.start_unpacking(world, us);
                return Ok(self.calc_sleep_time());
            }
            if self.packing_state == STATE_UNPACKED {
                self.start_preparation(world, us)?;
                if self.is_preparation_complete() {
                    // "STEP 4 -- TRIGGER (skipping preparation)"
                    self.trigger_ability_effect(world, us)?;
                    if self.is_persistent_ability() && d.persistence_requires_recharge {
                        self.reset_preparation();
                        world.module_call(us, spm, ModuleCall::StartPowerRecharge);
                        return Ok(self.calc_sleep_time());
                    }
                    self.end_preparation(world, us)?;
                    if self.need_to_pack() {
                        self.start_packing(world, us, true)?;
                    } else {
                        self.finish_ability(world, us)?;
                    }
                }
            }
        } else if Self::ai_call(world, us, ModuleCall::AiIsIdle).as_bool() {
            // "STEP 1 -- APPROACH"
            self.approach_target(world, us);
        }
        Ok(self.calc_sleep_time())
    }

    /// `initiateIntentToDoSpecialPower(template, targetObj, targetPos, way, options)` (the template by id, 0 for `NULL`).
    fn initiate_intent(&mut self, world: &mut World, me: ModuleHandle, template: u32, target_object: ObjectId, target_position: Option<Coord3D>) -> Result<bool, Undefined> {
        let us = me.object;
        let d = self.data.clone();
        if self.template_id(world).unwrap_or(0) != template {
            // "Check to make sure our modules are connected."
            return Ok(false);
        }
        self.target_id = INVALID_ID;
        self.target_pos = Coord3D::default();
        self.location_count = 0;
        self.prep_frames = 0;
        self.anim_frames = 0;
        self.packing_state = STATE_PACKED;
        self.facing_initiated = false;
        self.facing_complete = false;
        self.within_start_ability_range = false;
        Self::clear_ability_conditions(world, us);
        let has_target_object = Self::exists(world, target_object);
        if has_target_object {
            self.target_id = target_object;
        } else if let Some(p) = target_position {
            self.target_pos = p;
        }
        // "Clear any old AI before starting this special ability."
        if Self::ai_of(world, us).is_none() {
            return Ok(false);
        }
        Self::ai_call(world, us, ModuleCall::AiIdle);
        self.no_target_command = !has_target_object && target_position.is_none();
        if d.unpack_time == 0 || self.no_target_command && d.skip_packing_with_no_target {
            // "Only unpack if we need to"
            self.packing_state = STATE_UNPACKED;
        }
        self.active = true;
        // "Prevent other mutually exclusive specials from running"
        for t in [
            SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK,
            SPECIAL_BLACKLOTUS_STEAL_CASH_HACK,
            SPECIAL_BLACKLOTUS_CAPTURE_BUILDING,
            SPECIAL_REMOTE_CHARGES,
            SPECIAL_TIMED_CHARGES,
            SPECIAL_INFANTRY_CAPTURE_BUILDING,
            SPECIAL_BOOBY_TRAP,
        ] {
            if let Some(idx) = self.find_special_ability_update(world, me, t)?.filter(|_| !crate::logic::modules::plant(3815)) {
                world.module_call(us, idx, ModuleCall::SpecialAbilityOnExit { cleanup: false });
            }
        }
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
        Ok(true)
    }

    /// `getObject()->findSpecialAbilityUpdate(type)`, for `initiateIntentToDoSpecialPower`: the first behavior module that is a special ability
    /// of the type (`None` also when that is this one, which the caller skips).
    fn find_special_ability_update(&self, world: &mut World, me: ModuleHandle, t: i32) -> Result<Option<ModuleIndex>, Undefined> {
        for idx in world.all_modules(me.object) {
            if idx == me.index {
                if self.power_type(world)? == t {
                    return Ok(None);
                }
            } else if matches!(world.module_call(me.object, idx, ModuleCall::GetSpecialAbilityType), ModuleReply::Int(ty) if ty == t) {
                return Ok(Some(idx));
            }
        }
        Ok(None)
    }

    /// `handlePackingProcessing()`: whether packing or unpacking is still going on.
    fn handle_packing_processing(&mut self, world: &mut World, us: ObjectId) -> Result<bool, Undefined> {
        let d = self.data.clone();
        if self.anim_frames == 0 {
            return Ok(false);
        }
        self.anim_frames -= 1;
        if self.anim_frames == 0 {
            world.clear_model_condition_flags(us, &[MODELCONDITION_UNPACKING, MODELCONDITION_PACKING]);
            if self.packing_state == STATE_UNPACKING {
                if d.flip_object_after_unpacking {
                    let a = world.objects.find(us).map_or(0.0, |o| o.thing.orientation());
                    world.set_orientation(us, a + std::f32::consts::PI);
                }
                self.packing_state = STATE_UNPACKED;
            } else if self.packing_state == STATE_PACKING {
                if d.flip_object_after_packing {
                    let a = world.objects.find(us).map_or(0.0, |o| o.thing.orientation());
                    world.set_orientation(us, a + std::f32::consts::PI);
                }
                // "We just finished packing up, therefore we have completed our special ability."
                self.packing_state = STATE_PACKED;
                self.finish_ability(world, us)?;
                return Ok(true);
            }
            return Ok(false);
        }
        // "the ability to disable stealth before triggering"
        if d.lose_stealth_on_trigger && self.anim_frames < d.pre_trigger_unstealth_frames {
            if let Some(stealth) = world.objects.find(us).and_then(|o| o.stealth) {
                world.module_call(us, stealth, ModuleCall::StealthMarkAsDetected);
            }
        }
        Ok(true)
    }

    fn need_to_pack(&self) -> bool {
        let d = &self.data;
        self.packing_state == STATE_UNPACKED && !(d.skip_packing_with_no_target && self.no_target_command) && d.pack_time != 0
    }

    fn need_to_unpack(&self) -> bool {
        let d = &self.data;
        self.packing_state == STATE_PACKED && !(d.skip_packing_with_no_target && self.no_target_command) && d.unpack_time != 0
    }

    fn is_preparation_complete(&self) -> bool {
        self.prep_frames == 0
    }

    fn is_persistent_ability(&self) -> bool {
        self.data.persistent_prep_frames > 0
    }

    fn reset_preparation(&mut self) {
        self.prep_frames = self.data.persistent_prep_frames;
    }

    /// `m_animFrames = time * GameLogicRandomValueReal(1 - f, 1 + f)`.
    fn anim_time(world: &mut World, time: u32, factor: f32) -> u32 {
        let variation = world.random.game_logic_random_value_real(1.0 - factor, 1.0 + factor);
        let frames = time as f32 * variation;
        // (the conversion of a `Real` to `UnsignedInt`, MSVC's for an out-of-range value aside)
        frames as u32
    }

    /// The voice the unit plays when the packing that follows a success starts.
    fn completion_voice(&self, world: &World, us: ObjectId) -> Result<Vec<u8>, Undefined> {
        let t = self.power_type(world)?;
        let Some(o) = world.objects.find(us) else { return Ok(Vec::new()) };
        let tmpl = o.template();
        let name = match t {
            SPECIAL_BLACKLOTUS_CAPTURE_BUILDING => tmpl.get_per_unit_sound(b"VoiceCaptureBuildingComplete").clone(),
            SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK => tmpl.get_per_unit_sound(b"VoiceDisableVehicleComplete").clone(),
            SPECIAL_BLACKLOTUS_STEAL_CASH_HACK => tmpl.get_per_unit_sound(b"VoiceStealCashComplete").clone(),
            _ => tmpl.get_audio(crate::common::thing_template::ThingTemplateAudioType::VoiceTaskComplete).clone(),
        };
        Ok(name.event_name)
    }

    /// `startPacking(success)`.
    fn start_packing(&mut self, world: &mut World, us: ObjectId, success: bool) -> Result<(), Undefined> {
        let d = self.data.clone();
        self.packing_state = STATE_PACKING;
        self.anim_frames = Self::anim_time(world, d.pack_time, d.pack_unpack_variation_factor);
        world.clear_and_set_model_condition_flags(us, &[MODELCONDITION_UNPACKING, MODELCONDITION_RAISING_FLAG], &[MODELCONDITION_PACKING]);
        world.audio_add_event(d.pack_sound.clone(), us);
        if world.has_drawable(us) {
            world.emit(ObjectClientEvent::DrawableSetAnimationCompletionTime { object_id: us, frames: self.anim_frames });
        }
        Self::ai_call(world, us, ModuleCall::AiBusy { command_source: CMD_FROM_AI });
        if success {
            let voice = self.completion_voice(world, us)?;
            world.audio_add_event(voice, us);
        }
        Ok(())
    }

    /// `startUnpacking()`.
    fn start_unpacking(&mut self, world: &mut World, us: ObjectId) {
        let d = self.data.clone();
        self.packing_state = STATE_UNPACKING;
        self.anim_frames = Self::anim_time(world, d.unpack_time, d.pack_unpack_variation_factor);
        world.clear_and_set_model_condition_flags(us, &[MODELCONDITION_PACKING], &[MODELCONDITION_UNPACKING]);
        world.audio_add_event(d.unpack_sound.clone(), us);
        if world.has_drawable(us) {
            world.emit(ObjectClientEvent::DrawableSetAnimationCompletionTime { object_id: us, frames: self.anim_frames });
        }
        Self::ai_call(world, us, ModuleCall::AiBusy { command_source: CMD_FROM_AI });
    }

    /// The squared distance to the target or the position (`None`: neither, so "this step is useless"), and the target.
    fn distance_to_target(&self, world: &mut World, us: ObjectId) -> Option<(f32, Option<ObjectId>)> {
        use crate::logic::world::partition::DistanceCalculationType;
        if self.target_id != INVALID_ID {
            let target = Self::exists(world, self.target_id).then_some(self.target_id);
            let d = match target {
                Some(t) => world.partition_distance_squared(us, t, DistanceCalculationType::FromBoundingSphere2D),
                None => 0.0,
            };
            Some((d, target))
        } else if self.has_target_pos() {
            let p = self.target_pos;
            Some((world.partition_distance_squared_to_pos(us, &p, DistanceCalculationType::FromBoundingSphere2D), None))
        } else {
            None
        }
    }

    /// Whether the unit touches the target (`iteratePotentialCollisions` at its position, of its geometry).
    fn collides_with(world: &mut World, us: ObjectId, target: Option<ObjectId>) -> bool {
        let Some(o) = world.objects.find(us) else { return false };
        let (pos, geom) = (o.get_position(), o.geometry_info.clone());
        let them = world.partition_iterate_potential_collisions_at(pos, &geom, 0.0);
        target.is_some_and(|t| them.contains(&t))
    }

    /// `range = __max(0, StartAbilityRange - PATHFIND_CELL_SIZE_F * 0.25)`.
    fn contact_range(&self) -> f32 {
        let range = self.data.start_ability_range - PATHFIND_CELL_SIZE_F * 0.25;
        if 0.0 > range {
            0.0
        } else {
            range
        }
    }

    /// `isWithinStartAbilityRange()`.
    fn is_within_start_ability_range(&self, world: &mut World, us: ObjectId) -> bool {
        let range = self.contact_range();
        if self.within_start_ability_range {
            return true; // "Only get within range once."
        }
        let Some((dist_sqr, target)) = self.distance_to_target(world, us) else { return true };
        let start = self.data.start_ability_range;
        if dist_sqr > start * start {
            return false;
        }
        if range == 0.0 && self.target_id != INVALID_ID {
            // "We want to ensure we collided with our target first!"
            return Self::collides_with(world, us, target);
        }
        if self.data.approach_requires_los {
            // "Make sure we can see the target!"
            let seen = world.partition_iterate_line_of_sight_near_to_far(us, range);
            return seen.iter().any(|&s| Some(s) == target);
        }
        true
    }

    /// `isWithinAbilityAbortRange()`.
    fn is_within_ability_abort_range(&self, world: &mut World, us: ObjectId) -> bool {
        let range = self.contact_range();
        let Some((dist_sqr, target)) = self.distance_to_target(world, us) else { return true };
        let abort = self.data.ability_abort_range;
        if dist_sqr > abort * abort {
            return false;
        }
        if range == 0.0 && self.target_id != INVALID_ID {
            return Self::collides_with(world, us, target);
        }
        true
    }

    /// `approachTarget()`.
    fn approach_target(&mut self, world: &mut World, us: ObjectId) -> bool {
        if self.target_id != INVALID_ID {
            if Self::exists(world, self.target_id) && Self::ai_of(world, us).is_some() {
                Self::ai_call(world, us, ModuleCall::AiIgnoreObstacle { obstacle: self.target_id });
                Self::ai_call(world, us, ModuleCall::AiMoveToObject { target: self.target_id, command_source: CMD_FROM_AI });
                return true;
            }
        } else if self.has_target_pos() && Self::ai_of(world, us).is_some() {
            Self::ai_call(world, us, ModuleCall::AiMoveToPosition { pos: self.target_pos, command_source: CMD_FROM_AI });
            return true;
        }
        false
    }

    /// `startPreparation()`.
    fn start_preparation(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        let d = self.data.clone();
        self.prep_frames = d.preparation_frames;
        let t = self.power_type(world)?;
        let target = Self::exists(world, self.target_id).then_some(self.target_id);
        match t {
            SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES => {
                if let Some(target) = target {
                    if let Some(special) = self.create_special_object(world, us)? {
                        if !self.init_laser(world, us, special, Some(target))? {
                            return Ok(());
                        }
                    }
                }
            }
            SPECIAL_INFANTRY_CAPTURE_BUILDING => {
                if let Some(target) = target {
                    if world.objects.find(target).map(|o| o.team) == world.objects.find(us).map(|o| o.team) {
                        return Ok(()); // "it's been captured by a colleague! we should stop."
                    }
                    if world.check_and_detonate_booby_trap(target, Some(us)) && world.objects.find(target).is_some_and(|o| o.is_effectively_dead()) {
                        return Ok(()); // "Whoops, it was mined. Cancel if it is now dead."
                    }
                }
                world.clear_and_set_model_condition_flags(us, &[MODELCONDITION_UNPACKING], &[MODELCONDITION_RAISING_FLAG]);
                if world.has_drawable(us) {
                    world.emit(ObjectClientEvent::DrawableSetAnimationCompletionTime { object_id: us, frames: d.preparation_frames });
                }
                // "Warn the victim so he might have a chance to react!"
                if target.is_some_and(|t| world.is_locally_controlled(t)) {
                    world.emit(ObjectClientEvent::Eva { message: EvaMessage::BuildingBeingStolen });
                }
                world.emit(ObjectClientEvent::Radar { call: RadarCall::TryInfiltrationEvent, object_id: target.unwrap_or(INVALID_ID), position: None });
            }
            SPECIAL_HACKER_DISABLE_BUILDING | SPECIAL_BLACKLOTUS_CAPTURE_BUILDING | SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK | SPECIAL_BLACKLOTUS_STEAL_CASH_HACK => {
                if let Some(target) = target {
                    if world.get_relationship(us, target) == Relationship::Allies {
                        return Ok(());
                    }
                    if let Some(special) = self.create_special_object(world, us)? {
                        if !self.init_laser(world, us, special, Some(target))? {
                            return Ok(());
                        }
                        // "For the hacker this sets up the looping typing animation."
                        world.clear_and_set_model_condition_flags(us, &[MODELCONDITION_UNPACKING], &[MODELCONDITION_FIRING_A]);
                    }
                    if t == SPECIAL_BLACKLOTUS_CAPTURE_BUILDING && world.is_locally_controlled(target) {
                        world.emit(ObjectClientEvent::Eva { message: EvaMessage::BuildingBeingStolen });
                    }
                    world.emit(ObjectClientEvent::Radar { call: RadarCall::TryInfiltrationEvent, object_id: target, position: None });
                }
            }
            _ => {}
        }
        // "Trigger the reset timer on the special power because we're officially starting it now!"
        if let Some(spm) = self.get_my_spm(world, us) {
            world.module_call(us, spm, ModuleCall::MarkSpecialPowerTriggered { location: None });
        }
        Self::ai_call(world, us, ModuleCall::AiIdle); // "just in case. jba."
        world.set_status(us, mask_of(OBJECT_STATUS_IS_USING_ABILITY), true);
        self.prep_sound_handle = world.audio_add_event(d.prep_sound_loop.clone(), us);
        Ok(())
    }

    /// `initLaser(specialObject, target)`: whether the special object has a laser (a `LaserUpdate` on its drawable); one without is killed with
    /// the others.
    fn init_laser(&mut self, world: &mut World, us: ObjectId, special: ObjectId, target: Option<ObjectId>) -> Result<bool, Undefined> {
        if !world.has_drawable(special) || !world.client_update_module_named(special, b"LaserUpdate") {
            self.kill_special_objects(world, us)?;
            return Ok(false);
        }
        let bone = self.data.special_object_attach_to_bone_name.clone();
        let start = world.contain_get_single_logical_bone_position(us, &bone);
        let end = match target.and_then(|t| world.objects.find(t)) {
            Some(o) => o.geometry_info.get_center_position(&o.get_position()),
            None => start,
        };
        world.emit(ObjectClientEvent::LaserInit { laser: special, parent: us, target: target.unwrap_or(INVALID_ID), start, end, bone });
        Ok(true)
    }

    /// `continuePreparation()`: whether to go on.
    fn continue_preparation(&mut self, world: &mut World, us: ObjectId) -> Result<bool, Undefined> {
        let d = self.data.clone();
        if d.ability_abort_range < SPECIAL_ABILITY_HUGE_DISTANCE && !self.is_within_ability_abort_range(world, us) {
            return Ok(false);
        }
        let t = self.power_type(world)?;
        match t {
            SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES | SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK => {
                let target = self.target_id;
                if !Self::exists(world, target) {
                    return Ok(false); // "Target is dead, stop."
                }
                if world.get_relationship(us, target) == Relationship::Allies {
                    return Ok(false); // "It's been captured by a colleague, so cancel!"
                }
                for special in self.special_object_ids.clone() {
                    if Self::exists(world, special) && !self.init_laser(world, us, special, Some(target))? {
                        return Ok(false);
                    }
                }
            }
            SPECIAL_INFANTRY_CAPTURE_BUILDING | SPECIAL_BLACKLOTUS_CAPTURE_BUILDING => {
                let target = self.target_id;
                if !Self::exists(world, target) {
                    return Ok(false);
                }
                if world.get_relationship(us, target) == Relationship::Allies {
                    return Ok(false);
                }
                if d.do_capture_fx && world.has_drawable(target) {
                    // "PROCESS THE FLASHING WHILE GETTING CAPTURED FX HERE"
                    let last_phase = (self.capture_flash_phase as i32) & 1 != 0;
                    let denominator = if 1 > d.preparation_frames { 1.0 } else { d.preparation_frames as f32 };
                    let increment = 1.0 - (self.prep_frames as f32 / denominator);
                    self.capture_flash_phase += increment / if crate::logic::modules::plant(3812) { 2.0 } else { 3.0 };
                    let this_phase = (self.capture_flash_phase as i32) & 1 != 0;
                    if last_phase && !this_phase {
                        let (r, g, b) = saturate_rgb(world.get_indicator_color(us), world.cuts.globals.selection_flash_saturation_factor);
                        world.emit(ObjectClientEvent::DrawableFlashAsSelectedColor { object_id: target, r, g, b });
                        world.emit(ObjectClientEvent::Sound { sound: ObjectSound::DefectorTimerTick, object_id: target, position: None });
                    }
                }
                if t == SPECIAL_INFANTRY_CAPTURE_BUILDING {
                    if let Some(spm) = self.get_my_spm(world, us) {
                        // "these keep resetting the recharge timer while in use"
                        world.module_call(us, spm, ModuleCall::StartPowerRecharge);
                    }
                }
            }
            _ => {}
        }
        Ok(true)
    }

    /// `triggerAbilityEffect()`.
    fn trigger_ability_effect(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        let d = self.data.clone();
        let t = self.power_type(world)?;
        if d.award_xp_for_triggering != 0 {
            world.experience_add_points(us, d.award_xp_for_triggering, true);
        }
        let skill_points = if d.skill_points_for_triggering != -1 { d.skill_points_for_triggering } else { d.award_xp_for_triggering };
        if skill_points > 0 {
            if let Some(player) = world.get_controlling_player(us) {
                world.cut(CutCall::PlayerAddSkillPoints { player, points: skill_points });
            }
        }
        world.audio_add_event(d.trigger_sound.clone(), us);
        let mut ok_to_lose_stealth = true;
        let target = Self::exists(world, self.target_id).then_some(self.target_id);
        // `checkAndDetonateBoobyTrap`: whether it went off and left the target or the unit dead
        let mined_dead = |world: &mut World, target: ObjectId| {
            world.check_and_detonate_booby_trap(target, Some(us))
                && (world.objects.find(target).is_some_and(|o| o.is_effectively_dead()) || world.objects.find(us).is_some_and(|o| o.is_effectively_dead()))
        };
        match t {
            SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES => {
                if let Some(target) = target {
                    if world.weapon_in_slot_exists(us, SECONDARY_WEAPON) {
                        // "lock it just till the weapon is empty or the attack is "done""
                        world.set_weapon_lock(us, SECONDARY_WEAPON, LOCKED_TEMPORARILY);
                        Self::ai_call(world, us, ModuleCall::AiAttackObject { victim: target, max_shots: NO_MAX_SHOTS_LIMIT, command_source: CMD_FROM_AI });
                    }
                }
            }
            SPECIAL_HELIX_NAPALM_BOMB => {
                // "Couldn't be simpler... the special object is the bomb"
                self.create_special_object(world, us)?;
            }
            SPECIAL_TANKHUNTER_TNT_ATTACK | SPECIAL_TIMED_CHARGES | SPECIAL_BOOBY_TRAP => {
                let Some(target) = target else { return Ok(()) };
                if mined_dead(world, target) {
                    return Ok(());
                }
                if t == SPECIAL_BOOBY_TRAP && world.objects.find(target).is_some_and(|o| o.test_status(OBJECT_STATUS_BOOBY_TRAPPED)) {
                    // "we can't double booby trap something."
                    return Ok(());
                }
                if let Some(charge) = self.create_special_object(world, us)? {
                    match world.find_module(charge, b"StickyBombUpdate") {
                        Some(idx) => {
                            world.module_call(charge, idx, ModuleCall::StickyBombInit { target, bomber: us, specific_pos: None });
                        }
                        None => {
                            self.kill_special_objects(world, us)?;
                            return Ok(());
                        }
                    }
                }
            }
            SPECIAL_HACKER_DISABLE_BUILDING | SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK => {
                let Some(target) = target else { return Ok(()) };
                if world.get_relationship(us, target) == Relationship::Allies {
                    return Ok(());
                }
                let until = world.frame().wrapping_add(d.effect_duration);
                world.cut(CutCall::ObjectSetDisabledUntil { object: target, disabled: crate::common::disabled_types::DISABLED_HACKED, frame: until });
                let mut duration_interleave_factor = 1u32;
                let (area, structure) = world.objects.find(target).map_or((0.0, false), |o| (o.geometry_info.get_footprint_area(), o.is_kind_of(KINDOF_STRUCTURE)));
                if area < 300.0 && structure {
                    // "toggle if small building"
                    self.do_disable_fx_particles = !self.do_disable_fx_particles;
                    duration_interleave_factor = 2;
                }
                if self.do_disable_fx_particles {
                    if let Some(tmp) = d.disable_fx_particle_system.clone() {
                        let sys = world.create_particle_system_unplaced(tmp);
                        let mut offs = Coord3D::default();
                        if let Some(geom) = world.objects.find(target).map(|o| o.geometry_info.clone()) {
                            geom.make_random_offset_within_footprint(&mut world.random, &mut offs);
                        }
                        world.body_particle_attach_to_object(sys, target);
                        world.particle_system_set_position(sys, offs);
                        world.particle_system_set_lifetime(sys, d.effect_duration.wrapping_mul(duration_interleave_factor));
                    }
                }
            }
            SPECIAL_INFANTRY_CAPTURE_BUILDING | SPECIAL_BLACKLOTUS_CAPTURE_BUILDING => {
                let Some(target) = target else { return Ok(()) };
                if mined_dead(world, target) {
                    return Ok(());
                }
                if world.objects.find(target).map(|o| o.team) == world.objects.find(us).map(|o| o.team) {
                    return Ok(()); // "it's been captured by a colleague! we should stop."
                }
                // "Just in case we are capturing a building which is already garrisoned by other"
                if let Some(contain) = world.get_contain(target) {
                    if world.module_call(target, contain, ModuleCall::IsGarrisonable).as_bool() {
                        world.module_call(target, contain, ModuleCall::RemoveAllContained { expose_stealth_units: true });
                        return self.after_effect(world, us, ok_to_lose_stealth);
                    }
                }
                if world.is_locally_controlled(target) {
                    world.emit(ObjectClientEvent::Eva { message: EvaMessage::BuildingStolen });
                }
                let Some(player) = world.get_controlling_player(us) else { return Err(Undefined::NullControllingPlayer) };
                let team = world.cuts.player_default_team(player);
                world.crate_defect(target, Some(team), 1); // "one frame of flash!"
                if t == SPECIAL_BLACKLOTUS_CAPTURE_BUILDING {
                    if let Some(spm) = self.get_my_spm(world, us) {
                        world.module_call(us, spm, ModuleCall::StartPowerRecharge);
                    }
                }
                world.cut(CutCall::PlayerAcademyRecordBuildingCapture { player });
            }
            SPECIAL_BLACKLOTUS_STEAL_CASH_HACK => {
                let Some(target) = target else { return Ok(()) };
                let (Some(victim), Some(thief)) = (world.get_controlling_player(target), world.get_controlling_player(us)) else {
                    return Err(Undefined::NullControllingPlayer);
                };
                let available = world.players.get_nth(victim).map_or(0, |p| p.money.count_money());
                let cash = if 1000 < available { 1000 } else { available };
                if cash > 0 && !crate::logic::modules::plant(3813) {
                    world.cut(CutCall::PlayerMoneyWithdraw { player: victim, amount: cash });
                    world.cut(CutCall::PlayerMoneyDeposit { player: thief, amount: cash });
                    world.cut(CutCall::PlayerScoreAddMoneyEarned { player: thief, amount: cash as i32 });
                    if world.is_locally_controlled(target) {
                        world.emit(ObjectClientEvent::Eva { message: EvaMessage::CashStolen });
                    }
                    let mut pos = world.objects.find(us).map(|o| o.get_position()).unwrap_or_default();
                    pos.z += 20.0;
                    world.cut(CutCall::InGameUiAddFloatingTextNamed { text: b"GUI:AddCash".to_vec(), amount: cash as i32, position: pos, color: game_make_color(0, 255, 0, 255) });
                    let mut pos = world.objects.find(target).map(|o| o.get_position()).unwrap_or_default();
                    pos.z += 30.0;
                    world.cut(CutCall::InGameUiAddFloatingTextNamed { text: b"GUI:LoseCash".to_vec(), amount: cash as i32, position: pos, color: game_make_color(255, 0, 0, 255) });
                }
            }
            SPECIAL_REMOTE_CHARGES => {
                if let Some(target) = target {
                    if mined_dead(world, target) {
                        return Ok(());
                    }
                }
                if self.target_id == INVALID_ID && !self.has_target_pos() {
                    // "If there is no target object nor position, then we are detonating the existing charges."
                    for special in self.special_object_ids.clone() {
                        if Self::exists(world, special) {
                            if let Some(idx) = world.find_module(special, b"StickyBombUpdate") {
                                world.module_call(special, idx, ModuleCall::Detonate);
                                ok_to_lose_stealth = false;
                            }
                        }
                    }
                } else {
                    let Some(target) = target else { return Ok(()) };
                    if let Some(charge) = self.create_special_object(world, us)? {
                        match world.find_module(charge, b"StickyBombUpdate") {
                            Some(idx) => {
                                world.module_call(charge, idx, ModuleCall::StickyBombInit { target, bomber: us, specific_pos: None });
                            }
                            None => {
                                self.kill_special_objects(world, us)?;
                                return Ok(());
                            }
                        }
                    }
                }
            }
            SPECIAL_DISGUISE_AS_VEHICLE => {
                if let Some(target) = target {
                    if let Some(stealth) = world.objects.find(us).and_then(|o| o.stealth) {
                        world.module_call(us, stealth, ModuleCall::StealthDisguiseAsObject(target));
                    }
                }
            }
            _ => {}
        }
        self.after_effect(world, us, ok_to_lose_stealth)
    }

    /// The end of `triggerAbilityEffect()` (after the switch's `break`s): lose stealth on trigger.
    fn after_effect(&mut self, world: &mut World, us: ObjectId, ok_to_lose_stealth: bool) -> Result<(), Undefined> {
        if self.data.lose_stealth_on_trigger && ok_to_lose_stealth {
            if let Some(stealth) = world.objects.find(us).and_then(|o| o.stealth) {
                world.module_call(us, stealth, ModuleCall::StealthMarkAsDetected);
            }
        }
        Ok(())
    }

    /// `createSpecialObject()`.
    fn create_special_object(&mut self, world: &mut World, us: ObjectId) -> Result<Option<ObjectId>, Undefined> {
        let d = self.data.clone();
        if self.special_object_entries == d.max_special_objects {
            if d.special_objects_persistent {
                // "don't allow any more to be created"
                return Ok(None);
            }
            // "We've reached our limit with non-persistent object(s), so remove them."
            self.kill_special_objects(world, us)?;
        }
        let Some(template) = world.thing_factory_find_template(&d.special_object_name) else { return Ok(None) };
        let team = world.objects.find(us).map(|o| o.team).filter(|t| *t != 0);
        let special = world.thing_factory_new_object(template, team, Default::default()).map_err(|_| Undefined::NewObjectFailed)?;
        self.special_object_ids.push(special);
        self.special_object_entries += 1;
        let (pos, angle) = world.objects.find(us).map_or((Coord3D::default(), 0.0), |o| (o.get_position(), o.thing.orientation()));
        world.set_position(special, &pos);
        world.set_orientation(special, angle);
        world.cut(CutCall::ObjectSetExperienceSink { object: special, sink: us });
        if let Some(o) = world.objects.find_mut(special) {
            o.experience.sink = us;
        }
        if let Some(phys) = world.objects.find(special).and_then(|o| o.physics) {
            let pitch_rate = world.module_call(special, phys, ModuleCall::PhysicsGetCenterOfMassOffset).as_real(0.0);
            world.module_call(special, phys, ModuleCall::PhysicsSetPitchRate(pitch_rate));
            world.module_call(special, phys, ModuleCall::PhysicsSetAllowAirborneFriction(false));
        }
        Ok(Some(special))
    }

    /// `killSpecialObjects()`.
    fn kill_special_objects(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        for special in std::mem::take(&mut self.special_object_ids) {
            if Self::exists(world, special) {
                world.body_destroy_object(special); // "Delete the old one!"
            }
        }
        self.special_object_entries = 0;
        if self.power_type(world)? == SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES && world.weapon_in_slot_exists(us, PRIMARY_WEAPON) && !crate::logic::modules::plant(3818) {
            // "The instant we remove the laser, reset the weaponset!"
            world.set_weapon_lock(us, PRIMARY_WEAPON, LOCKED_TEMPORARILY);
        }
        Ok(())
    }

    /// `validateSpecialObjects()`: the special objects that are gone leave the list.
    fn validate_special_objects(&mut self, world: &World) {
        let before = self.special_object_ids.len();
        self.special_object_ids.retain(|&id| world.objects.find(id).is_some());
        let removed = if crate::logic::modules::plant(3819) { 0 } else { (before - self.special_object_ids.len()) as u32 };
        self.special_object_entries = self.special_object_entries.wrapping_sub(removed);
    }

    /// `finishAbility()`.
    fn finish_ability(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        let d = self.data.clone();
        self.within_start_ability_range = false;
        self.packing_state = STATE_NONE;
        let valid_target = self.has_target_pos() || self.target_id != INVALID_ID;
        if d.flee_range_after_completion != 0.0 && valid_target {
            let mut pos = world.objects.find(us).map(|o| o.get_position()).unwrap_or_default();
            if Self::ai_of(world, us).is_some() {
                let mut dir = world.objects.find_mut(us).map(|o| o.thing.unit_direction_vector_2d()).unwrap_or_default();
                dir.scale(d.flee_range_after_completion);
                if (d.flip_object_after_unpacking || d.flip_object_after_packing) != crate::logic::modules::plant(3816) {
                    pos.add(&dir);
                } else {
                    pos.sub(&dir);
                }
                // "Now check for mines. Normally we are fleeing from a bomb we just planted."
                if let Some(player) = world.get_controlling_player(us) {
                    let candidates = world.partition_closest_candidates_at(pos, d.flee_range_after_completion, FROM_CENTER_2D);
                    let mine = candidates.into_iter().find(|&c| {
                        world.objects.find(c).is_some_and(|o| o.is_kind_of(KINDOF_MINE)) && world.get_controlling_player(c) == Some(player)
                    });
                    if let Some(mine) = mine {
                        let mpos = world.objects.find(mine).map(|o| o.get_position()).unwrap_or_default();
                        dir.set(pos.x - mpos.x, pos.y - mpos.y, 0.0);
                        dir.normalize();
                        dir.scale(d.flee_range_after_completion);
                        pos = mpos;
                        pos.add(&dir);
                    }
                }
                // "apply a motive force (even zero!) so that AI will think we were already "moving""
                if let Some(phys) = world.objects.find(us).and_then(|o| o.physics) {
                    world.module_call(us, phys, ModuleCall::PhysicsApplyMotiveForce(Coord3D::default()));
                }
                Self::ai_call(world, us, ModuleCall::AiMoveToPosition { pos, command_source: CMD_FROM_AI });
                if Self::exists(world, self.target_id) {
                    Self::ai_call(world, us, ModuleCall::AiIgnoreObstacle { obstacle: self.target_id });
                }
            }
        } else {
            // "Clear any old AI before ending this special ability."
            Self::ai_call(world, us, ModuleCall::AiIdle);
        }
        self.on_exit(world, us, false)
    }

    /// `isFacing()`.
    fn is_facing(&mut self, world: &mut World, us: ObjectId) -> bool {
        if Self::ai_of(world, us).is_none() {
            return true; // "Something without AI always faces target..."
        }
        if !self.facing_complete && self.facing_initiated {
            if Self::ai_call(world, us, ModuleCall::AiIsIdle).as_bool() {
                self.facing_complete = true; // "We finished facing the target"
                return false;
            }
            return true;
        }
        false
    }

    /// `needToFace()`.
    fn need_to_face(&self, world: &World, us: ObjectId) -> bool {
        if Self::ai_of(world, us).is_none() || !self.data.need_to_face_target && !crate::logic::modules::plant(3817) {
            return false;
        }
        !self.facing_initiated || !self.facing_complete
    }

    /// `startFacing()`.
    fn start_facing(&mut self, world: &mut World, us: ObjectId) {
        if Self::ai_of(world, us).is_none() {
            return;
        }
        // "First kill it's physics."
        Self::ai_call(world, us, ModuleCall::AiIdle);
        if let Some(phys) = world.objects.find(us).and_then(|o| o.physics) {
            world.module_call(us, phys, ModuleCall::PhysicsResetDynamicPhysics);
        }
        self.facing_initiated = true;
        if Self::exists(world, self.target_id) {
            Self::ai_call(world, us, ModuleCall::AiFaceObject { target: self.target_id, command_source: CMD_FROM_AI });
        } else if self.has_target_pos() {
            Self::ai_call(world, us, ModuleCall::AiFacePosition { pos: self.target_pos, command_source: CMD_FROM_AI });
        }
    }

    /// `findSpecialObjectWithProducerID(target)`: the first special object (in the list's order) that exists and whose producer is the target.
    pub fn special_object_with_producer_id(&self, world: &World, target: ObjectId) -> Option<ObjectId> {
        self.special_object_ids.iter().copied().find(|&s| world.objects.find(s).is_some_and(|o| o.producer_id == target))
    }

    /// `endPreparation()`.
    fn end_preparation(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        world.set_status(us, mask_of(OBJECT_STATUS_IS_USING_ABILITY), false);
        world.audio_remove_event(self.prep_sound_handle);
        match self.power_type(world)? {
            SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES
            | SPECIAL_HACKER_DISABLE_BUILDING
            | SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK
            | SPECIAL_BLACKLOTUS_CAPTURE_BUILDING
            | SPECIAL_BLACKLOTUS_STEAL_CASH_HACK
            | SPECIAL_INFANTRY_CAPTURE_BUILDING => self.kill_special_objects(world, us)?,
            // "No, don't delete placed charges. -OR- Not applicable"
            _ => {}
        }
        Ok(())
    }

    fn report<T>(world: &mut World, me: ModuleHandle, r: Result<T, Undefined>) -> Option<T> {
        match r {
            Ok(v) => Some(v),
            Err(u) => {
                world.cuts.module_errors.push((me, IniError::undefined(u)));
                None
            }
        }
    }
}

/// `RGBColor::setFromInt(color)` then `Drawable::saturateRGB(color, factor)`: the colour `flashAsSelected` is given.
fn saturate_rgb(color: i32, factor: f32) -> (f32, f32, f32) {
    let channel = |shift: i32| ((color >> shift) & 0xff) as f32 / 255.0;
    let half_factor = factor * 0.5;
    let s = |c: f32| c * factor - half_factor;
    (s(channel(16)), s(channel(8)), s(channel(0)))
}

/// `GameMakeColor(r, g, b, a)`.
fn game_make_color(r: i32, g: i32, b: i32, a: i32) -> i32 {
    (a << 24) | (r << 16) | (g << 8) | b
}

impl UpdateBehavior for SpecialAbilityUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        self.base.update()
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let r = self.run_update(world, me);
        Self::report(world, me, r).unwrap_or(UPDATE_SLEEP_NONE)
    }
    /// `setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER)`.
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_bool(&mut self.active)?;
        xfer.xfer_unsigned_int(&mut self.prep_frames)?;
        xfer.xfer_unsigned_int(&mut self.anim_frames)?;
        xfer.xfer_object_id(&mut self.target_id)?;
        xfer.xfer_coord3d(&mut self.target_pos)?;
        xfer.xfer_int(&mut self.location_count)?;
        xfer.xfer_stl_object_id_list(&mut self.special_object_ids)?;
        xfer.xfer_unsigned_int(&mut self.special_object_entries)?;
        xfer.xfer_bool(&mut self.no_target_command)?;
        let mut b = self.packing_state.to_le_bytes();
        xfer.xfer_user(&mut b)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.packing_state = i32::from_le_bytes(b);
        }
        xfer.xfer_bool(&mut self.facing_initiated)?;
        xfer.xfer_bool(&mut self.facing_complete)?;
        xfer.xfer_bool(&mut self.within_start_ability_range)?;
        xfer.xfer_bool(&mut self.do_disable_fx_particles)?;
        if !crate::logic::modules::plant(3814) {
            xfer.xfer_real(&mut self.capture_flash_phase)?;
        }
        Ok(())
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::SpecialAbilityUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SpecialAbilityUpdate(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        let us = me.object;
        match call {
            ModuleCall::InitiateIntentToDoSpecialPower { template, target_object, target_position, .. } => {
                {
                let r = self.initiate_intent(world, me, template, target_object, target_position);
                ModuleReply::Bool(Self::report(world, me, r).unwrap_or(false))
            }
            }
            ModuleCall::GetSpecialAbilityType => match self.power_type(world) {
                Ok(t) => ModuleReply::Int(t),
                Err(u) => {
                    world.cuts.module_errors.push((me, IniError::undefined(u)));
                    ModuleReply::Unhandled
                }
            },
            ModuleCall::SpecialAbilityOnExit { cleanup } => {
                let r = self.on_exit(world, us, cleanup);
                Self::report(world, me, r);
                ModuleReply::Done
            }
            ModuleCall::SpecialPowerUpdateIsActive => ModuleReply::Bool(self.active),
            ModuleCall::SpecialPowerUpdateGetCommandOption => ModuleReply::Int(0),
            ModuleCall::IsPowerCurrentlyInUse => ModuleReply::Bool(self.is_power_currently_in_use()),
            ModuleCall::SpecialAbilityGetSpecialObjectCount => ModuleReply::Int(self.special_object_entries as i32),
            ModuleCall::SpecialAbilityGetSpecialObjectMax => ModuleReply::Int(self.data.max_special_objects as i32),
            ModuleCall::SpecialAbilityFindSpecialObjectWithProducerId(target) => ModuleReply::Bool(self.special_object_with_producer_id(world, target).is_some()),
            ModuleCall::DoesSpecialPowerUpdatePassScienceTest => {
                ModuleReply::Bool(self.base.does_special_power_update_pass_science_test(self.base.get_extra_required_science(), |_| false))
            }
            ModuleCall::DoesSpecialPowerHaveOverridableDestinationActive | ModuleCall::DoesSpecialPowerHaveOverridableDestination => ModuleReply::Bool(false),
            ModuleCall::SetSpecialPowerOverridableDestination(_) => ModuleReply::Done,
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for SpecialAbilityUpdate {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.base.update_mut().crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.base.update().clone();
        let r = update_behavior::xfer_own(self, &mut base, xfer);
        *self.base.update_mut() = base;
        r
    }
    fn load_post_process(&mut self) {
        self.base.update_mut().load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = SpecialAbilityUpdateModuleData::default();
        assert_eq!((d.start_ability_range, d.max_special_objects, d.effect_value, d.skill_points_for_triggering), (SPECIAL_ABILITY_HUGE_DISTANCE, 1, 1, -1));
        assert!(d.approach_requires_los && d.need_to_face_target);
        let m = SpecialAbilityUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.packing_state, STATE_NONE);
        assert!(m.do_disable_fx_particles && !m.active);
        assert_eq!(game_make_color(0, 255, 0, 255), 0xFF00_FF00u32 as i32);
    }
}
