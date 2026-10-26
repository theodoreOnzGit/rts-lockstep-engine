// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SpectreGunshipUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SpectreGunshipUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SpectreGunshipUpdate`: the Spectre gunship itself, made by the command centre's `SpectreGunshipDeploymentUpdate`. Told to fire at a
//! target (`initiateIntentToDoSpecialPower`), it flies in from the map edge on its panic locomotor with its afterburners on, takes on a
//! gattling gun (`GattlingTemplateName`, made on its team and put in its contain, paralysed until the orbit) and draws two decals at
//! itself: the attack area and the targeting reticle. Each frame it steers for the point on its orbit ahead of the line to the target
//! (`OrbitInsertionSlope`, clamped to 0.5..0.8); once within its orbit radius it orbits (`OrbitTime`), un-paralyses the gattling and
//! turns its afterburners off. Every `HowitzerFiringRate` frames of the orbit it picks a target in the reticle (the player may move the
//! reticle: `setSpecialPowerOverridableDestination`, kept within the attack area) or, for a computer player, anywhere in the attack
//! area, not too near the ship; the gattling attacks it, or the strafing point. The strafing point walks towards the target by
//! `StrafingIncrement` each frame the gattling fires; once it has stayed on it for more than `HowitzerFollowLag` evaluations, the
//! howitzer (`HowitzerWeaponTemplate`) fires there with a random offset. When the orbit ends it heads off the map the way it faces
//! and is destroyed once it is off.
//!
//! # Upstream facts kept
//!
//! - The data class derives from `ModuleData`, not `UpdateModuleData`.
//! - `update()` is always awake (`UPDATE_SLEEP_NONE`) until the ship is effectively dead (then asleep for ever); it runs while the ship
//!   is subdued, underpowered, EMP'd or hacked (`getDisabledTypesToProcess`).
//! - The "gunship shot down" branch (`getObject()` `NULL`) cannot be reached.
//! - `m_satellitePosition` is not set by the constructor (the allocator's zero) and only its x and y are ever steered.
//! - The howitzer's random offset is `GameLogicRandomValue(-offs, offs)`, the `Int` draw of the offset truncated; the strafing smoke's is
//!   the client's `Real` draw, made only while the ship is at most partially clear to the local player.
//! - The decals are not saved (version 2: "can't flat-save decals"), and a load leaves them as they were; neither are the two sounds.
//! - Unchecked upstream, so undefined here: the special power module never found (`Undefined::NullSpecialPowerModule`), a `NULL`
//!   target position (`Undefined::NullLocation`), an AI with no current locomotor (`Undefined::NullLocomotor`), a ship with no
//!   controlling player when it picks a target (`Undefined::NullControllingPlayer`), no local player while it strafes
//!   (`Undefined::NullLocalPlayer`) and a zero `HowitzerFiringRate` (`Undefined::ModuloByZero`).
//!
//! The partition scans are cut points ([`World::partition_iterate_spectre_targets`]): the partition manager is layer 12's. The first of
//! their four filters, `PartitionFilterLiveMapEnemies`, is this file's own and is translated here ([`live_map_enemy`]).
//!
//! `p1w13-sonnet`, batch R72, 2026-10-26.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::disabled_types::{DisabledMaskType, DISABLED_EMP, DISABLED_HACKED, DISABLED_PARALYZED, DISABLED_SUBDUED, DISABLED_UNDERPOWERED};
use crate::common::game_common::{ObjectShroudStatus, PlayerType, Relationship};
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::model_state::{MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DOOR_1_OPENING, MODELCONDITION_JETAFTERBURNER};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::OBJECT_STATUS_IS_FIRING_WEAPON;
use crate::common::thing_template::ThingTemplateAudioType;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::special_power_update_module::SpecialPowerUpdateModule;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::spectre_gunship_deployment_update::COMMAND_FIRED_BY_SCRIPT;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::radius_decal::{RadiusDecal, RadiusDecalTemplate};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object::ModuleIndex;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `ORBIT_INSERTION_SLOPE_MAX`, `ORBIT_INSERTION_SLOPE_MIN`.
const ORBIT_INSERTION_SLOPE_MAX: f32 = 0.8;
const ORBIT_INSERTION_SLOPE_MIN: f32 = 0.5;
/// `LOTS_OF_SHOTS`.
pub const LOTS_OF_SHOTS: i32 = 9999;
/// `CMD_FROM_AI` (`CommandSourceType`).
const CMD_FROM_AI: i32 = 2;
/// `LOCOMOTORSET_NORMAL`, `LOCOMOTORSET_PANIC`.
const LOCOMOTORSET_NORMAL: i32 = 0;
const LOCOMOTORSET_PANIC: i32 = 4;

/// `GunshipStatus`.
pub const GUNSHIP_STATUS_INSERTING: i32 = 0;
pub const GUNSHIP_STATUS_ORBITING: i32 = 1;
pub const GUNSHIP_STATUS_DEPARTING: i32 = 2;
pub const GUNSHIP_STATUS_IDLE: i32 = 3;

/// The decals' indices in the client's events: `m_attackAreaDecal`, `m_targetingReticleDecal`.
const ATTACK_AREA_DECAL: u32 = 0;
const TARGETING_RETICLE_DECAL: u32 = 1;

/// `class SpectreGunshipUpdateModuleData : public ModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct SpectreGunshipUpdateModuleData {
    pub update: UpdateModuleData,
    /// `m_specialPowerTemplate`, by name (found in the store when used; an unknown name, and `None`, is `NULL`).
    pub special_power_template: Option<Vec<u8>>,
    /// `m_howitzerWeaponTemplate`, by name (found in the weapon store when used; a name it does not hold, and `None`, is `NULL`).
    pub howitzer_weapon_template: Option<Vec<u8>>,
    /// `m_gattlingTemplateName`.
    pub gattling_template_name: Vec<u8>,
    /// `m_attackAreaDecalTemplate`, `m_targetingReticleDecalTemplate`.
    pub attack_area_decal_template: RadiusDecalTemplate,
    pub targeting_reticle_decal_template: RadiusDecalTemplate,
    /// `m_orbitFrames`, `m_howitzerFiringRate`, `m_howitzerFollowLag`.
    pub orbit_frames: u32,
    pub howitzer_firing_rate: u32,
    pub howitzer_follow_lag: u32,
    /// `m_attackAreaRadius`, `m_targetingReticleRadius`, `m_gunshipOrbitRadius`, `m_strafingIncrement`, `m_orbitInsertionSlope`,
    /// `m_randomOffsetForHowitzer`.
    pub attack_area_radius: f32,
    pub targeting_reticle_radius: f32,
    pub gunship_orbit_radius: f32,
    pub strafing_increment: f32,
    pub orbit_insertion_slope: f32,
    pub random_offset_for_howitzer: f32,
    /// `m_gattlingStrafeFXParticleSystem`: the template's name, `None` for `NULL`.
    pub gattling_strafe_fx_particle_system: Option<Vec<u8>>,
}

impl Default for SpectreGunshipUpdateModuleData {
    /// `SpectreGunshipUpdateModuleData()`.
    fn default() -> Self {
        SpectreGunshipUpdateModuleData {
            update: UpdateModuleData::new(),
            special_power_template: None,
            howitzer_weapon_template: None,
            gattling_template_name: Vec::new(),
            attack_area_decal_template: RadiusDecalTemplate::default(),
            targeting_reticle_decal_template: RadiusDecalTemplate::default(),
            orbit_frames: 0,
            howitzer_firing_rate: 10,
            howitzer_follow_lag: 0,
            attack_area_radius: 200.0,
            targeting_reticle_radius: 25.0,
            gunship_orbit_radius: 250.0,
            strafing_increment: 20.0,
            orbit_insertion_slope: 0.7,
            random_offset_for_howitzer: 20.0,
            gattling_strafe_fx_particle_system: None,
        }
    }
}

/// `INI::parseParticleSystemTemplate`: `None` is `NULL`; any other name is looked up when it is used.
fn parse_particle_system(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let token = ini.get_next_token(None)?;
    Ok((!token.eq_ignore_ascii_case(b"None")).then_some(token))
}

static FIELD_PARSE: [FieldParse<SpectreGunshipUpdateModuleData>; 16] = [
    FieldParse::new("SpecialPowerTemplate", |i, d, _| { d.special_power_template = Some(i.get_next_token(None)?); Ok(()) }, UserData::None),
    FieldParse::new("GattlingTemplateName", |i, d, _| { d.gattling_template_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("HowitzerFiringRate", |i, d, _| { d.howitzer_firing_rate = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("OrbitTime", |i, d, _| { d.orbit_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("HowitzerFollowLag", |i, d, _| { d.howitzer_follow_lag = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("AttackAreaRadius", |i, d, _| { d.attack_area_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("StrafingIncrement", |i, d, _| { d.strafing_increment = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("OrbitInsertionSlope", |i, d, _| { d.orbit_insertion_slope = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("RandomOffsetForHowitzer", |i, d, _| { d.random_offset_for_howitzer = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("TargetingReticleRadius", |i, d, _| { d.targeting_reticle_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("GunshipOrbitRadius", |i, d, _| { d.gunship_orbit_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("HowitzerWeaponTemplate", |i, d, _| { d.howitzer_weapon_template = Some(i.get_next_token(None)?); Ok(()) }, UserData::None),
    FieldParse::new("GattlingStrafeFXParticleSystem", |i, d, _| { d.gattling_strafe_fx_particle_system = parse_particle_system(i)?; Ok(()) }, UserData::None),
    FieldParse::new("AttackAreaDecal", |i, d, _| RadiusDecalTemplate::parse_radius_decal_template(i, &mut d.attack_area_decal_template), UserData::None),
    FieldParse::new(
        "TargetingReticleDecal",
        |i, d, _| RadiusDecalTemplate::parse_radius_decal_template(i, &mut d.targeting_reticle_decal_template),
        UserData::None,
    ),
    FieldParse::end(),
];

impl SpectreGunshipUpdateModuleData {
    /// `buildFieldParse(p)`: `ModuleData`'s (nothing), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, SpectreGunshipUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<SpectreGunshipUpdateModuleData> {
        let mut data = SpectreGunshipUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, SpectreGunshipUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `SpectreGunshipUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::SpectreGunshipUpdate(Arc::new(SpectreGunshipUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `PartitionFilterLiveMapEnemies(obj).allow(other)`: alive, on or off the map as `obj` is, and its enemy.
pub fn live_map_enemy(world: &World, obj: ObjectId, other: ObjectId) -> bool {
    let (Some(a), Some(b)) = (world.objects.find(obj), world.objects.find(other)) else { return false };
    if b.is_effectively_dead() {
        return false;
    }
    if b.is_off_map() != a.is_off_map() && !crate::logic::modules::plant(2421) {
        return false;
    }
    world.get_relationship(obj, other) == Relationship::Enemies
}

/// `class SpectreGunshipUpdate : public SpecialPowerUpdateModule`.
#[derive(Clone, Debug)]
pub struct SpectreGunshipUpdate {
    base: SpecialPowerUpdateModule,
    data: Arc<SpectreGunshipUpdateModuleData>,
    /// `m_specialPowerModule` (`NULL` until `onObjectCreated` finds it).
    special_power_module: Option<ModuleIndex>,
    /// `m_initialTargetPosition`, `m_overrideTargetDestination`, `m_satellitePosition`, `m_gattlingTargetPosition`, `m_positionToShootAt`.
    initial_target_position: Coord3D,
    override_target_destination: Coord3D,
    satellite_position: Coord3D,
    gattling_target_position: Coord3D,
    position_to_shoot_at: Coord3D,
    /// `m_status` (`GunshipStatus`).
    status: i32,
    /// `m_okToFireHowitzerCounter`, `m_orbitEscapeFrame`.
    ok_to_fire_howitzer_counter: u32,
    orbit_escape_frame: u32,
    /// `m_gattlingID`.
    gattling_id: ObjectId,
    /// `m_attackAreaDecal`, `m_targetingReticleDecal`.
    attack_area_decal: RadiusDecal,
    targeting_reticle_decal: RadiusDecal,
    /// `m_afterburnerSound`'s playing handle (the sound is the template's `Afterburner`; `m_howitzerFireSound`'s is never kept).
    afterburner_handle: u32,
}

impl PartialEq for SpectreGunshipUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.base.update().behavior().object() == other.base.update().behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.special_power_module == other.special_power_module
            && self.state_words() == other.state_words()
    }
}

impl SpectreGunshipUpdate {
    pub const fn get_interface_mask() -> i32 {
        SpecialPowerUpdateModule::get_interface_mask()
    }

    /// The constructor (it touches no world): the base, idle, no gattling, the positions zero, the decals empty (`INI_INVALID_DATA` for
    /// null data). The sounds are the template's, read when they are played.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<SpectreGunshipUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let mut base = SpecialPowerUpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        // getDisabledTypesToProcess(): MAKE_DISABLED_MASK4( DISABLED_SUBDUED, DISABLED_UNDERPOWERED, DISABLED_EMP, DISABLED_HACKED )
        let mut mask = DisabledMaskType::default();
        for t in [DISABLED_SUBDUED, DISABLED_UNDERPOWERED, DISABLED_EMP, DISABLED_HACKED] {
            mask.set(t, 1);
        }
        base.update_mut().set_disabled_types_to_process(mask);
        Ok(SpectreGunshipUpdate {
            base,
            data,
            special_power_module: None,
            initial_target_position: Coord3D::default(),
            override_target_destination: Coord3D::default(),
            satellite_position: Coord3D::default(),
            gattling_target_position: Coord3D::default(),
            position_to_shoot_at: Coord3D::default(),
            status: GUNSHIP_STATUS_IDLE,
            ok_to_fire_howitzer_counter: 0,
            orbit_escape_frame: 0,
            gattling_id: INVALID_ID,
            attack_area_decal: RadiusDecal::default(),
            targeting_reticle_decal: RadiusDecal::default(),
            afterburner_handle: 0,
        })
    }

    pub fn update_module(&self) -> &UpdateModule {
        self.base.update()
    }

    pub fn module_data(&self) -> &Arc<SpectreGunshipUpdateModuleData> {
        &self.data
    }

    /// The status, the five positions (bits), the counter, the escape frame, the gattling's id, whether each decal is empty and the
    /// afterburner's playing handle.
    pub fn state_words(&self) -> Vec<u32> {
        let mut v = vec![self.status as u32];
        for p in [self.initial_target_position, self.override_target_destination, self.satellite_position, self.gattling_target_position, self.position_to_shoot_at] {
            v.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
        }
        v.extend([
            self.ok_to_fire_howitzer_counter,
            self.orbit_escape_frame,
            self.gattling_id as u32,
            u32::from(self.attack_area_decal.is_empty()),
            u32::from(self.targeting_reticle_decal.is_empty()),
            self.afterburner_handle,
        ]);
        v
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

    /// The special power template's id in the store (`None`: `NULL`).
    fn template_id(world: &World, name: &Option<Vec<u8>>) -> Option<u32> {
        name.as_deref().and_then(|n| world.statics.special_power_store.find_special_power_template(n)).map(|t| t.id())
    }

    fn position(world: &World, id: ObjectId) -> Coord3D {
        world.objects.find(id).map(|o| o.get_position()).unwrap_or_default()
    }

    /// `onObjectCreated()`: the object's module for the data's special power and the satellite at the object (no special power named:
    /// the `DEBUG_CRASH`, compiled out, and nothing more).
    fn on_object_created(&mut self, world: &mut World, us: ObjectId) {
        let Some(template) = Self::template_id(world, &self.data.special_power_template) else { return };
        self.special_power_module = world.get_special_power_module(us, Some(template));
        if !crate::logic::modules::plant(2422) {
            self.satellite_position = Self::position(world, us);
        }
    }

    /// `shipAI->chooseLocomotorSet(set); getCurLocomotor()->setAllowInvalidPosition(TRUE); ->setUltraAccurate(TRUE)` (the locomotor
    /// unchecked).
    fn ship_locomotor(world: &mut World, us: ObjectId, ai: ModuleIndex, set: i32) -> Result<(), IniError> {
        world.module_call(us, ai, ModuleCall::AiChooseLocomotorSet(set));
        if !world.module_call(us, ai, ModuleCall::AiHasCurLocomotor).as_bool() {
            return Err(IniError::undefined(Undefined::NullLocomotor));
        }
        world.module_call(us, ai, ModuleCall::AiLocomotorSetAllowInvalidPosition(true));
        world.module_call(us, ai, ModuleCall::AiLocomotorSetUltraAccurate(true));
        Ok(())
    }

    /// `initiateIntentToDoSpecialPower(specialPowerTemplate, targetObj, targetPos, way, commandOptions)`.
    fn initiate_intent(&mut self, world: &mut World, us: ObjectId, template: u32, target_pos: Option<Coord3D>, command_options: u32) -> Result<bool, IniError> {
        let d = self.data.clone();
        let sp = self.special_power_module.ok_or_else(|| IniError::undefined(Undefined::NullSpecialPowerModule))?;
        let ours = match world.module_call(us, sp, ModuleCall::GetSpecialPowerTemplateId) {
            ModuleReply::Int(id) => id as u32,
            _ => 0,
        };
        if ours != template {
            // "Check to make sure our modules are connected."
            return Ok(false);
        }
        let target = target_pos.ok_or_else(|| IniError::undefined(Undefined::NullLocation))?;
        if command_options & COMMAND_FIRED_BY_SCRIPT == 0 {
            self.initial_target_position = target;
            self.override_target_destination = target;
            if !crate::logic::modules::plant(2423) {
                self.gattling_target_position = target;
            }
        } else {
            let now = world.frame();
            world.module_call(us, sp, ModuleCall::SetReadyFrame { frame: now });
            self.initial_target_position = target;
            self.status = GUNSHIP_STATUS_INSERTING;
        }
        // "MAKE TWEAKS TO AI SO SHIP MOVES PRETTY"
        if let Some(ai) = world.objects.find(us).and_then(|o| o.ai) {
            Self::ship_locomotor(world, us, ai, LOCOMOTORSET_PANIC)?;
        }
        world.clear_and_set_model_condition_state(us, MODELCONDITION_DOOR_1_OPENING, MODELCONDITION_DOOR_1_CLOSING);
        self.enable_afterburners(world, us, true);
        // "The gunship is en route to the tharget area, from map-edge"
        self.status = GUNSHIP_STATUS_INSERTING;
        // "TELL THE GUNNERS ABOARD THE GUNSHIP TO HOLD THEIR FIRE UNTIL ORBIT INSERTION"
        if let Some(contain) = world.get_contain(us) {
            if world.objects.find(self.gattling_id).is_some() {
                self.gattling_id = INVALID_ID;
            }
            let gattling_template = world.statics.thing_factory.find_template(&d.gattling_template_name).map(|t| t.data.get_template_id());
            if let Some(gattling_template) = gattling_template {
                let team = world.get_team(us);
                // (`newObject` throws on failure; the world's errors are not upstream's)
                let gattling = world.thing_factory_new_object(gattling_template, team, Default::default()).map_err(|_| IniError::new(IniErrorKind::BadArg))?;
                world.module_call(us, contain, ModuleCall::AddToContain(gattling));
                self.gattling_id = gattling;
            }
            if world.objects.find(self.gattling_id).is_some() && !crate::logic::modules::plant(2424) {
                world.body_set_disabled(self.gattling_id, DISABLED_PARALYZED);
            }
        }
        let (pos, owner) = (Self::position(world, us), world.get_controlling_player(us));
        let reticle = if crate::logic::modules::plant(2425) { d.attack_area_radius } else { d.targeting_reticle_radius };
        d.attack_area_decal_template.create_radius_decal(world, (us, ATTACK_AREA_DECAL), pos, d.attack_area_radius, owner, &mut self.attack_area_decal);
        d.targeting_reticle_decal_template.create_radius_decal(world, (us, TARGETING_RETICLE_DECAL), pos, reticle, owner, &mut self.targeting_reticle_decal);
        if let Some(own) = world.get_special_power_module(us, Some(template)) {
            world.module_call(us, own, ModuleCall::MarkSpecialPowerTriggered { location: Some(self.initial_target_position) });
        }
        Ok(true)
    }

    /// `setSpecialPowerOverridableDestination(loc)`: the reticle moves, unless the ship is disabled; the attack voice for the local player.
    fn set_overridable_destination(&mut self, world: &mut World, us: ObjectId, loc: Coord3D) {
        if world.objects.find(us).is_some_and(|o| o.is_disabled()) {
            return;
        }
        self.override_target_destination = loc;
        let local = world.get_controlling_player(us).is_some_and(|p| world.players.local_index() == Some(p));
        if local && !crate::logic::modules::plant(2426) {
            let voice = world.objects.find(us).map(|o| o.template().get_audio(ThingTemplateAudioType::VoiceAttack).event_name.clone()).unwrap_or_default();
            world.audio_add_event(voice, us);
        }
    }

    /// `isPointOffMap(testPos)`: not strictly inside the map's extent with its border (x and y).
    fn is_point_off_map(world: &World, test_pos: &Coord3D) -> bool {
        let r = world.terrain_get_extent_including_border();
        !((r.lo.x < test_pos.x) && (test_pos.x < r.hi.x) && (r.lo.y < test_pos.y) && (test_pos.y < r.hi.y))
    }

    /// `isFairDistanceFromShip(target)`: farther than three quarters of the orbit radius from the ship (2D).
    fn is_fair_distance_from_ship(&self, world: &World, us: ObjectId, target: ObjectId) -> bool {
        let (t, s) = (Self::position(world, target), Self::position(world, us));
        let delta = Coord3D { x: s.x - t.x, y: s.y - t.y, z: 0.0 };
        let fraction = if crate::logic::modules::plant(2427) { 0.5 } else { 0.75 };
        delta.length() > self.data.gunship_orbit_radius * fraction
    }

    /// `friend_enableAfterburners(v)`: the model condition, and the looping sound started (if it is not playing) or stopped (if it is).
    fn enable_afterburners(&mut self, world: &mut World, us: ObjectId, v: bool) {
        if v {
            world.set_model_condition_state(us, MODELCONDITION_JETAFTERBURNER);
            if !world.audio_is_currently_playing(self.afterburner_handle) {
                let name = world.objects.find(us).map(|o| o.template().get_per_unit_sound(b"Afterburner").event_name.clone()).unwrap_or_default();
                self.afterburner_handle = world.audio_add_event(name, us);
            }
        } else {
            world.clear_model_condition_state(us, MODELCONDITION_JETAFTERBURNER);
            if world.audio_is_currently_playing(self.afterburner_handle) {
                world.audio_remove_event(self.afterburner_handle);
            }
        }
    }

    /// `cleanUp()`: the decals cleared and the gattling destroyed.
    fn clean_up(&mut self, world: &mut World, us: ObjectId) {
        self.attack_area_decal.clear(world, us, ATTACK_AREA_DECAL);
        self.targeting_reticle_decal.clear(world, us, TARGETING_RETICLE_DECAL);
        if world.objects.find(self.gattling_id).is_some() && !crate::logic::modules::plant(2428) {
            world.body_destroy_object(self.gattling_id);
        }
    }

    /// `disengageAndDepartAO(gunship)`: "head off the map in the direction you are facing", the gattling paralysed, the panic locomotor,
    /// the doors, the afterburners, and `cleanUp`.
    fn disengage_and_depart(&mut self, world: &mut World, us: ObjectId) -> Result<(), IniError> {
        let ai = world.objects.find(us).and_then(|o| o.ai);
        if let Some(ai) = ai {
            let o = world.objects.find(us).unwrap();
            let mut exit_point = o.thing.unit_direction_vector_3d();
            let map_size = 99999.0f32;
            exit_point.x *= map_size;
            exit_point.y *= map_size;
            exit_point.add(&o.get_position());
            world.module_call(us, ai, ModuleCall::AiMoveToPosition { pos: exit_point, command_source: CMD_FROM_AI });
        }
        if world.objects.find(self.gattling_id).is_some() {
            world.body_set_disabled(self.gattling_id, DISABLED_PARALYZED);
        }
        if let Some(ai) = ai {
            Self::ship_locomotor(world, us, ai, LOCOMOTORSET_PANIC)?;
        }
        world.clear_and_set_model_condition_state(us, MODELCONDITION_DOOR_1_OPENING, MODELCONDITION_DOOR_1_CLOSING);
        self.enable_afterburners(world, us, true);
        self.clean_up(world, us);
        Ok(())
    }

    /// The first enemy the scan of `range` about `pos` finds (the live-map-enemies filter, then the fair distance), nearest first.
    fn scan(&self, world: &mut World, us: ObjectId, pos: Coord3D, range: f32, player_index: i32) -> Option<ObjectId> {
        let found = world.partition_iterate_spectre_targets(us, pos, range, player_index);
        found.into_iter().filter(|&e| live_map_enemy(world, us, e)).find(|&e| self.is_fair_distance_from_ship(world, us, e))
    }

    /// `update()`'s body; an error is undefined upstream.
    fn do_update(&mut self, world: &mut World, us: ObjectId) -> Result<UpdateSleepTime, IniError> {
        let d = self.data.clone();
        if world.objects.find(us).is_some_and(|o| o.is_effectively_dead()) {
            return Ok(UPDATE_SLEEP_FOREVER);
        }
        self.attack_area_decal.update(world, us, ATTACK_AREA_DECAL);
        self.targeting_reticle_decal.update(world, us, TARGETING_RETICLE_DECAL);
        let ship_ai = world.objects.find(us).and_then(|o| o.ai);
        let gattling = world.objects.find(self.gattling_id).map(|o| (o.id, o.ai));
        let gattling_ai = gattling.and_then(|(_, ai)| ai);
        if self.status == GUNSHIP_STATUS_INSERTING || self.status == GUNSHIP_STATUS_ORBITING {
            // "perigee is the point in the orbital arc nearest the satellite being captured"
            let mut perigee = Self::position(world, us);
            perigee.sub(&self.initial_target_position);
            perigee.z = 0.0;
            let distance_to_target = perigee.length();
            perigee.normalize();
            // "apogee is the anteclockwise point fathest from the perigee line"
            let apogee = Coord3D { x: -perigee.y, y: perigee.x, z: 0.0 };
            // "declination intersects line [p][a], given an attack slope of n1/n2"
            let n1 = ORBIT_INSERTION_SLOPE_MAX.min(ORBIT_INSERTION_SLOPE_MIN.max(d.orbit_insertion_slope));
            let n2 = if crate::logic::modules::plant(2429) { n1 } else { 1.0 - n1 };
            let mut declination = Coord3D { x: (perigee.x * n1) + (apogee.x * n2), y: (perigee.y * n1) + (apogee.y * n2), z: 0.0 };
            // "scale out to the orbital radius"
            let orbital_radius = d.gunship_orbit_radius;
            declination.x *= orbital_radius;
            declination.y *= orbital_radius;
            self.satellite_position.x = self.initial_target_position.x + declination.x;
            self.satellite_position.y = self.initial_target_position.y + declination.y;
            if let Some(ai) = ship_ai {
                world.module_call(us, ai, ModuleCall::AiMoveToPosition { pos: self.satellite_position, command_source: CMD_FROM_AI });
            }
            let constraint_radius = d.attack_area_radius - d.targeting_reticle_radius;
            // "Constrain Target Override to the targeting radius"
            let mut delta = self.initial_target_position;
            delta.sub(&self.override_target_destination);
            if delta.length() > constraint_radius && !crate::logic::modules::plant(2430) {
                delta.normalize();
                delta.x *= constraint_radius;
                delta.y *= constraint_radius;
                self.override_target_destination.x = self.initial_target_position.x - delta.x;
                self.override_target_destination.y = self.initial_target_position.y - delta.y;
            }
            self.attack_area_decal.set_position(world, us, ATTACK_AREA_DECAL, self.initial_target_position);
            self.targeting_reticle_decal.set_position(world, us, TARGETING_RETICLE_DECAL, self.override_target_destination);
            // "close enough to shoot"
            if self.status == GUNSHIP_STATUS_INSERTING && distance_to_target < orbital_radius {
                self.status = GUNSHIP_STATUS_ORBITING;
                self.orbit_escape_frame = world.frame().wrapping_add(d.orbit_frames);
                if world.objects.find(self.gattling_id).is_some() && !crate::logic::modules::plant(2431) {
                    world.body_clear_disabled(self.gattling_id, DISABLED_PARALYZED);
                }
                if let Some(ai) = ship_ai {
                    Self::ship_locomotor(world, us, ai, LOCOMOTORSET_NORMAL)?;
                }
                world.clear_and_set_model_condition_state(us, MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DOOR_1_OPENING);
                self.enable_afterburners(world, us, false);
            }
        }
        if self.status == GUNSHIP_STATUS_ORBITING {
            if world.frame() >= self.orbit_escape_frame {
                self.clean_up(world, us);
                self.status = GUNSHIP_STATUS_DEPARTING;
                // "CEASE FIRE, RETURN TO BASE"
                self.disengage_and_depart(world, us)?;
            } else {
                // "ONLY EVERY FEW FRAMES DO WE RE_EVALUATE THE TARGET OBJECT"
                let rate = d.howitzer_firing_rate;
                if rate == 0 {
                    return Err(IniError::undefined(Undefined::ModuloByZero));
                }
                if world.frame().is_multiple_of(rate) {
                    // "unless we get a hit, below"
                    self.position_to_shoot_at = self.override_target_destination;
                    let player = world.get_controlling_player(us).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
                    // "THIS WILL FIND A VALID TARGET WITHIN THE TARGETING RETICLE"
                    let mut valid = self.scan(world, us, self.override_target_destination, d.targeting_reticle_radius, player);
                    // "WE WANT THE WIDE_RANGE AUTOACQUIRE POWER DISABLED FOR HUMAN PLAYERS"
                    let human = world.players.get_nth(player).is_some_and(|p| p.player_type == PlayerType::Human);
                    if !human && valid.is_none() && !crate::logic::modules::plant(2432) {
                        // "THIS WILL FIND A VALID TARGET ANYWHERE INSIDE THE TARGETING AREA (THE BIG CIRCLE)"
                        valid = self.scan(world, us, self.initial_target_position, d.attack_area_radius, player);
                        if let Some(e) = valid {
                            self.position_to_shoot_at = Self::position(world, e);
                        }
                    }
                    // "lets keep a constant barrage of gattling bullets on the current aim location"
                    if let (Some(ai), Some((g, _))) = (gattling_ai, gattling) {
                        match valid {
                            Some(victim) => world.module_call(g, ai, ModuleCall::AiAttackObject { victim, max_shots: LOTS_OF_SHOTS, command_source: CMD_FROM_AI }),
                            None => world.module_call(
                                g,
                                ai,
                                ModuleCall::AiAttackPosition { pos: Some(self.gattling_target_position), max_shots: LOTS_OF_SHOTS, command_source: CMD_FROM_AI },
                            ),
                        };
                    }
                    // "IF THE GATTLING GUN HAS BEEN PLINKING THE TARGET LONG ENOUGH, LETS FOLLOW WITH A VOLLEY OF HOWITZER FIRE"
                    if self.ok_to_fire_howitzer_counter > d.howitzer_follow_lag {
                        let weapon = d.howitzer_weapon_template.as_deref().and_then(|n| {
                            world.statics.weapon_store.templates().iter().find(|t| t.name.eq_ignore_ascii_case(n)).map(|t| t.name.clone())
                        });
                        if let Some(weapon) = weapon {
                            let offs = d.random_offset_for_howitzer;
                            // (`GameLogicRandomValue` takes `Int`s: the offset truncated)
                            let (lo, hi) = (-offs as i32, offs as i32);
                            let rx = world.random.game_logic_random_value(lo, hi);
                            let ry = if crate::logic::modules::plant(2433) { rx } else { world.random.game_logic_random_value(lo, hi) };
                            let g = self.gattling_target_position;
                            let position = Coord3D { x: g.x + rx as f32, y: g.y + ry as f32, z: g.z };
                            world.cut(crate::logic::world::object_cuts::CutCall::WeaponCreateAndFireTemp { weapon, source: us, position });
                            let sound = world.objects.find(us).map(|o| o.template().get_per_unit_sound(b"HowitzerFire").event_name.clone()).unwrap_or_default();
                            world.audio_add_event(sound, us);
                        }
                    }
                }
                // "GATTLING TARGETING LOGIC"
                let firing = gattling.is_some_and(|(g, _)| world.objects.find(g).is_some_and(|o| o.test_status(OBJECT_STATUS_IS_FIRING_WEAPON)));
                if let (Some(template), true) = (d.gattling_strafe_fx_particle_system.clone(), firing) {
                    // "I am going to wind my gattling gun toward the next good spot, whether an object's position or a cursor position"
                    let mut delta = self.position_to_shoot_at;
                    delta.sub(&self.gattling_target_position);
                    let dist = delta.length();
                    if dist < d.strafing_increment {
                        self.gattling_target_position = self.position_to_shoot_at;
                        if !crate::logic::modules::plant(2434) {
                            self.ok_to_fire_howitzer_counter = self.ok_to_fire_howitzer_counter.wrapping_add(1);
                        }
                    } else {
                        self.ok_to_fire_howitzer_counter = 0;
                        delta.normalize();
                        delta.scale(d.strafing_increment);
                        self.gattling_target_position.add(&delta);
                    }
                    let local = world.players.local_index().ok_or_else(|| IniError::undefined(Undefined::NullLocalPlayer))?;
                    // "Make sure the gunship is visible to the player before drawing effects."
                    if world.get_shrouded_status(us, local) <= ObjectShroudStatus::PartialClear {
                        // "This makes the client smoke effects of the gattling cannon strafing the ground toward the attack position"
                        let sys = world.create_particle_system_unplaced(template);
                        let g = self.gattling_target_position;
                        let x = g.x + world.random.game_client_random_value_real(-5.0, 5.0);
                        let y = g.y + world.random.game_client_random_value_real(-5.0, 5.0);
                        let z = world.terrain_get_ground_height(x, y);
                        world.particle_system_set_position(sys, Coord3D { x, y, z });
                    }
                }
            }
        } else if self.status == GUNSHIP_STATUS_DEPARTING {
            let pos = Self::position(world, us);
            if Self::is_point_off_map(world, &pos) && !crate::logic::modules::plant(2435) {
                world.body_destroy_object(us);
                self.status = GUNSHIP_STATUS_IDLE;
                self.clean_up(world, us);
            }
        }
        Ok(UPDATE_SLEEP_NONE)
    }
}

impl UpdateBehavior for SpectreGunshipUpdate {
    const XFER_VERSION: XferVersion = 2;
    fn update_base(&self) -> &UpdateModule {
        self.base.update()
    }
    /// `update()`.
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        match self.do_update(world, me.object) {
            Ok(t) => t,
            Err(e) => {
                world.cuts.module_errors.push((me, e));
                UPDATE_SLEEP_NONE
            }
        }
    }
    /// The positions, the status and the escape frame; from version 2 the strafing point, the aim, the counter and the gattling's id.
    /// (Version 1 flat-saved the decals, which cannot be read back: "that's a hella crash". A save is always version 2.)
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
        xfer.xfer_coord3d(&mut self.initial_target_position)?;
        xfer.xfer_coord3d(&mut self.override_target_destination)?;
        xfer.xfer_coord3d(&mut self.satellite_position)?;
        let mut b = self.status.to_le_bytes();
        xfer.xfer_user(&mut b)?;
        self.status = i32::from_le_bytes(b);
        xfer.xfer_unsigned_int(&mut self.orbit_escape_frame)?;
        if version >= 2 && !crate::logic::modules::plant(2436) {
            xfer.xfer_coord3d(&mut self.gattling_target_position)?;
            xfer.xfer_coord3d(&mut self.position_to_shoot_at)?;
            xfer.xfer_unsigned_int(&mut self.ok_to_fire_howitzer_counter)?;
            xfer.xfer_object_id(&mut self.gattling_id)?;
        }
        Ok(())
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::SpectreGunshipUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SpectreGunshipUpdate(m) => Some(m),
            _ => None,
        }
    }
    /// `onObjectCreated`, `initiateIntentToDoSpecialPower`, the overridable destination and the special power update's queries.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnObjectCreated => {
                self.on_object_created(world, me.object);
                ModuleReply::Done
            }
            ModuleCall::InitiateIntentToDoSpecialPower { template, target_position, command_options, .. } => {
                match self.initiate_intent(world, me.object, template, target_position, command_options) {
                    Ok(b) => ModuleReply::Bool(b),
                    Err(e) => {
                        world.cuts.module_errors.push((me, e));
                        ModuleReply::Bool(false)
                    }
                }
            }
            ModuleCall::SetSpecialPowerOverridableDestination(loc) => {
                self.set_overridable_destination(world, me.object, loc);
                ModuleReply::Done
            }
            ModuleCall::DoesSpecialPowerHaveOverridableDestinationActive => {
                ModuleReply::Bool(self.status < GUNSHIP_STATUS_DEPARTING || crate::logic::modules::plant(2437))
            }
            ModuleCall::DoesSpecialPowerHaveOverridableDestination => ModuleReply::Bool(true),
            ModuleCall::SpecialPowerUpdateIsActive => ModuleReply::Bool(self.status < GUNSHIP_STATUS_DEPARTING),
            ModuleCall::SpecialPowerUpdateGetCommandOption => ModuleReply::Int(0),
            ModuleCall::IsPowerCurrentlyInUse => ModuleReply::Bool(false),
            ModuleCall::DoesSpecialPowerUpdatePassScienceTest => ModuleReply::Bool(true),
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for SpectreGunshipUpdate {
    /// `crc`: `UpdateModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.base.update_mut().crc(xfer)
    }
    /// `xfer` (version 2): the version, `UpdateModule::xfer`, the fields.
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
        let d = SpectreGunshipUpdateModuleData::default();
        assert_eq!((d.attack_area_radius, d.targeting_reticle_radius, d.gunship_orbit_radius), (200.0, 25.0, 250.0));
        assert_eq!((d.strafing_increment, d.orbit_insertion_slope, d.howitzer_firing_rate, d.random_offset_for_howitzer), (20.0, 0.7, 10, 20.0));
        let m = SpectreGunshipUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.status, GUNSHIP_STATUS_IDLE);
        assert_eq!(m.gattling_id, INVALID_ID);
        assert!(m.attack_area_decal.is_empty() && m.targeting_reticle_decal.is_empty());
        assert!(m.update_module().get_disabled_types_to_process().test(DISABLED_HACKED));
    }
}
