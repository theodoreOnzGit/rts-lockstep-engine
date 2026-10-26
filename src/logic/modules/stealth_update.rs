// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/StealthUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StealthUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `StealthUpdate`: the module that makes an object stealthy (invisible to non-allies) when its rules allow it, detected for a while when a
//! detector or a reveal marks it, and, for a disguising unit (the bomb truck), disguised as another object's template.
//!
//! # Design
//!
//! The client's half (the drawable's opacity, its second material pass, its stealth look, the drawable swap of a disguise, the control bar) is
//! events ([`ObjectClientEvent::Stealth`]): the decisions that only change what is drawn (is the drawable selected, is it hidden) move to the
//! client with the event, as the world's design asks. The logic's half runs for real: the status bits, the detection and stealth timers, the wake
//! frames, the rebinding of the object to a new drawable (`friend_bindToDrawable`, which tells every module), the physics reset, the radar, the
//! academy stats, the sounds and the FX.
//!
//! Another object's `getStealth()` (the rider's rules, a disguise target's disguise) is read from that object's slot
//! ([`stealth_update_of`]): those reads are of the data and of the disguise, which no running module of that object changes underneath.
//!
//! # Upstream facts kept
//!
//! - `update` and `markAsDetected` read the rider's rules through a local that stays uninitialised when the rider has no stealth update
//!   (`Undefined::UninitialisedLocal` where it is read).
//! - `changeVisualDisguise` reads the drawable unchecked (`isSelected`), and its reveal branch destroys the drawable, discards the new one
//!   (`TheThingFactory->newDrawable(tTemplate)` is not assigned) and goes on with the destroyed pointer. The memory pool's free list is last in,
//!   first out, so the new drawable takes the freed block and the pointer names it: the translation binds the new drawable, as the game does.
//! - `update`'s own `draw` pointer outlives a `changeVisualDisguise` in the transition the same way (it names the new drawable).
//! - Unstealthing on its own and losing detection both play `SoundStealthOn` (the latter only for the local player's object); gaining detection
//!   plays `SoundStealthOff`.
//! - `getPhysics()` is read unchecked by the drawable swap (`Undefined::NullPhysics`), the controlling player by the black market check, the
//!   colour of a disguise and the academy stats, the local player by the stealth look and the disguise colour.
//!
//! `p1w13-sonnet`, batch R26, 2026-10-23.

use std::sync::Arc;

use crate::common::base_type::PI;
use crate::common::disabled_types::DISABLED_HELD;
use crate::common::game_common::Relationship;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::{KINDOF_FS_BLACK_MARKET, KINDOF_MINE, KINDOF_SPAWNS_ARE_THE_WEAPONS};
use crate::common::model_state::MODELCONDITION_DISGUISED;
use crate::common::game_type::INVALID_ID;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::{
    ObjectStatusMaskType, ObjectStatusTypes, OBJECT_STATUS_CAN_STEALTH, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_IS_FIRING_WEAPON,
    OBJECT_STATUS_IS_USING_ABILITY, OBJECT_STATUS_SOLD, OBJECT_STATUS_STEALTHED, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use crate::common::thing_template::{ThingTemplate, ThingTemplateAudioType};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::damage::DAMAGE_HEALING;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::crate_collide::CrateCollideModuleData;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::object_script_status_bits::OBJECT_STATUS_SCRIPT_UNSTEALTHED;
use crate::logic::world::game_logic::World;
use crate::logic::world::statics::GameStatics;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::{ObjectClientEvent, StealthClientOp};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `STEALTH_NOT_WHILE_ATTACKING`.
pub const STEALTH_NOT_WHILE_ATTACKING: u32 = 0x0000_0001;
/// `STEALTH_NOT_WHILE_MOVING`.
pub const STEALTH_NOT_WHILE_MOVING: u32 = 0x0000_0002;
/// `STEALTH_NOT_WHILE_USING_ABILITY`.
pub const STEALTH_NOT_WHILE_USING_ABILITY: u32 = 0x0000_0004;
/// `STEALTH_NOT_WHILE_FIRING_PRIMARY`.
pub const STEALTH_NOT_WHILE_FIRING_PRIMARY: u32 = 0x0000_0008;
/// `STEALTH_NOT_WHILE_FIRING_SECONDARY`.
pub const STEALTH_NOT_WHILE_FIRING_SECONDARY: u32 = 0x0000_0010;
/// `STEALTH_NOT_WHILE_FIRING_TERTIARY`.
pub const STEALTH_NOT_WHILE_FIRING_TERTIARY: u32 = 0x0000_0020;
/// `STEALTH_ONLY_WITH_BLACK_MARKET`.
pub const STEALTH_ONLY_WITH_BLACK_MARKET: u32 = 0x0000_0040;
/// `STEALTH_NOT_WHILE_TAKING_DAMAGE`.
pub const STEALTH_NOT_WHILE_TAKING_DAMAGE: u32 = 0x0000_0080;
/// `STEALTH_NOT_WHILE_FIRING_WEAPON`: the three firing flags.
pub const STEALTH_NOT_WHILE_FIRING_WEAPON: u32 = STEALTH_NOT_WHILE_FIRING_PRIMARY | STEALTH_NOT_WHILE_FIRING_SECONDARY | STEALTH_NOT_WHILE_FIRING_TERTIARY;
/// `STEALTH_NOT_WHILE_RIDERS_ATTACKING`.
pub const STEALTH_NOT_WHILE_RIDERS_ATTACKING: u32 = 0x0000_0100;

/// `TheStealthLevelNames` (`StealthForbiddenConditions`).
pub const STEALTH_LEVEL_NAMES: [&str; 9] =
    ["ATTACKING", "MOVING", "USING_ABILITY", "FIRING_PRIMARY", "FIRING_SECONDARY", "FIRING_TERTIARY", "NO_BLACK_MARKET", "TAKING_DAMAGE", "RIDERS_ATTACKING"];

/// `TheEvaMessageNames` without the trailing `EVA_INVALID` (`Eva::nameToMessage` searches `EVA_FIRST` to `EVA_COUNT - 1`).
pub const EVA_MESSAGE_NAMES: [&str; 53] = [
    "LOWPOWER",
    "INSUFFICIENTFUNDS",
    "SUPERWEAPONDETECTED_OWN_PARTICLECANNON",
    "SUPERWEAPONDETECTED_OWN_NUKE",
    "SUPERWEAPONDETECTED_OWN_SCUDSTORM",
    "SUPERWEAPONDETECTED_ALLY_PARTICLECANNON",
    "SUPERWEAPONDETECTED_ALLY_NUKE",
    "SUPERWEAPONDETECTED_ALLY_SCUDSTORM",
    "SUPERWEAPONDETECTED_ENEMY_PARTICLECANNON",
    "SUPERWEAPONDETECTED_ENEMY_NUKE",
    "SUPERWEAPONDETECTED_ENEMY_SCUDSTORM",
    "SUPERWEAPONLAUNCHED_OWN_PARTICLECANNON",
    "SUPERWEAPONLAUNCHED_OWN_NUKE",
    "SUPERWEAPONLAUNCHED_OWN_SCUDSTORM",
    "SUPERWEAPONLAUNCHED_ALLY_PARTICLECANNON",
    "SUPERWEAPONLAUNCHED_ALLY_NUKE",
    "SUPERWEAPONLAUNCHED_ALLY_SCUDSTORM",
    "SUPERWEAPONLAUNCHED_ENEMY_PARTICLECANNON",
    "SUPERWEAPONLAUNCHED_ENEMY_NUKE",
    "SUPERWEAPONLAUNCHED_ENEMY_SCUDSTORM",
    "SUPERWEAPONREADY_OWN_PARTICLECANNON",
    "SUPERWEAPONREADY_OWN_NUKE",
    "SUPERWEAPONREADY_OWN_SCUDSTORM",
    "SUPERWEAPONREADY_ALLY_PARTICLECANNON",
    "SUPERWEAPONREADY_ALLY_NUKE",
    "SUPERWEAPONREADY_ALLY_SCUDSTORM",
    "SUPERWEAPONREADY_ENEMY_PARTICLECANNON",
    "SUPERWEAPONREADY_ENEMY_NUKE",
    "SUPERWEAPONREADY_ENEMY_SCUDSTORM",
    "BUILDINGLOST",
    "BASEUNDERATTACK",
    "ALLYUNDERATTACK",
    "BEACONDETECTED",
    "ENEMYBLACKLOTUSDETECTED",
    "ENEMYJARMENKELLDETECTED",
    "ENEMYCOLONELBURTONDETECTED",
    "OWNBLACKLOTUSDETECTED",
    "OWNJARMENKELLDETECTED",
    "OWNCOLONELBURTONDETECTED",
    "UNITLOST",
    "GENERALLEVELUP",
    "VEHICLESTOLEN",
    "BUILDINGSTOLEN",
    "CASHSTOLEN",
    "UPGRADECOMPLETE",
    "BUILDINGBEINGSTOLEN",
    "BUILDINGSABOTAGED",
    "SUPERWEAPONLAUNCHED_OWN_GPS_SCRAMBLER",
    "SUPERWEAPONLAUNCHED_ALLY_GPS_SCRAMBLER",
    "SUPERWEAPONLAUNCHED_ENEMY_GPS_SCRAMBLER",
    "SUPERWEAPONLAUNCHED_OWN_SNEAK_ATTACK",
    "SUPERWEAPONLAUNCHED_ALLY_SNEAK_ATTACK",
    "SUPERWEAPONLAUNCHED_ENEMY_SNEAK_ATTACK",
];

/// `EVA_Invalid`.
pub const EVA_INVALID: i32 = -1;

/// `StealthLookType` (`Drawable.h`).
pub mod stealth_look {
    /// `STEALTHLOOK_NONE`.
    pub const NONE: i32 = 0;
    /// `STEALTHLOOK_VISIBLE_FRIENDLY`.
    pub const VISIBLE_FRIENDLY: i32 = 1;
    /// `STEALTHLOOK_DISGUISED_ENEMY`.
    pub const DISGUISED_ENEMY: i32 = 2;
    /// `STEALTHLOOK_VISIBLE_DETECTED`.
    pub const VISIBLE_DETECTED: i32 = 3;
    /// `STEALTHLOOK_VISIBLE_FRIENDLY_DETECTED`.
    pub const VISIBLE_FRIENDLY_DETECTED: i32 = 4;
    /// `STEALTHLOOK_INVISIBLE`.
    pub const INVISIBLE: i32 = 5;
}

/// `CMD_FROM_PLAYER` (`CommandSourceType`).
const CMD_FROM_PLAYER: i32 = 0;
/// `FOREVER` (`GameCommon.h`).
const FOREVER: u32 = 0x3fff_ffff;
/// `PRIMARY_WEAPON`, `SECONDARY_WEAPON`, `TERTIARY_WEAPON`.
const WEAPON_SLOTS: [(u32, i32); 3] = [(STEALTH_NOT_WHILE_FIRING_PRIMARY, 0), (STEALTH_NOT_WHILE_FIRING_SECONDARY, 1), (STEALTH_NOT_WHILE_FIRING_TERTIARY, 2)];

fn status_mask(bits: &[ObjectStatusTypes]) -> ObjectStatusMaskType {
    let mut m = ObjectStatusMaskType::default();
    for &b in bits {
        m.set(b, 1);
    }
    m
}

/// `Eva::parseEvaMessageFromIni`: a name of `TheEvaMessageNames` (any case), `ERROR_BAD_INI` for any other.
pub fn parse_eva_message(ini: &mut Ini<'_>) -> IniResult<i32> {
    let token = ini.get_next_token(None)?;
    match EVA_MESSAGE_NAMES.iter().position(|n| n.as_bytes().eq_ignore_ascii_case(&token)) {
        Some(i) => Ok(i as i32),
        None => Err(IniError::new(IniErrorKind::InvalidData)),
    }
}

/// `class StealthUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct StealthUpdateModuleData {
    /// The base (`UpdateModuleData`, which adds nothing to `ModuleData`).
    pub update: UpdateModuleData,
    /// `m_hintDetectableStates`.
    pub hint_detectable_states: ObjectStatusMaskType,
    /// `m_requiredStatus`.
    pub required_status: ObjectStatusMaskType,
    /// `m_forbiddenStatus`.
    pub forbidden_status: ObjectStatusMaskType,
    /// `m_disguiseRevealFX`, by name (`None` for `NULL`).
    pub disguise_reveal_fx: Option<Vec<u8>>,
    /// `m_disguiseFX`.
    pub disguise_fx: Option<Vec<u8>>,
    /// `m_stealthSpeed`.
    pub stealth_speed: f32,
    /// `m_friendlyOpacityMin`.
    pub friendly_opacity_min: f32,
    /// `m_friendlyOpacityMax`.
    pub friendly_opacity_max: f32,
    /// `m_revealDistanceFromTarget`.
    pub reveal_distance_from_target: f32,
    /// `m_disguiseTransitionFrames`.
    pub disguise_transition_frames: u32,
    /// `m_disguiseRevealTransitionFrames`.
    pub disguise_reveal_transition_frames: u32,
    /// `m_pulseFrames` (parsed, never read).
    pub pulse_frames: u32,
    /// `m_stealthDelay`.
    pub stealth_delay: u32,
    /// `m_stealthLevel` (the `STEALTH_NOT_WHILE_*` flags).
    pub stealth_level: u32,
    /// `m_blackMarketCheckFrames`.
    pub black_market_check_frames: u32,
    /// `m_enemyDetectionEvaEvent` (`EVA_Invalid` = -1).
    pub enemy_detection_eva_event: i32,
    /// `m_ownDetectionEvaEvent`.
    pub own_detection_eva_event: i32,
    /// `m_innateStealth`.
    pub innate_stealth: bool,
    /// `m_orderIdleEnemiesToAttackMeUponReveal`.
    pub order_idle_enemies_to_attack_me_upon_reveal: bool,
    /// `m_teamDisguised`.
    pub team_disguised: bool,
    /// `m_useRiderStealth`: not set by the constructor; the game's `operator new` zeroes the data, so it starts `FALSE`.
    pub use_rider_stealth: bool,
    /// `m_grantedBySpecialPower`.
    pub granted_by_special_power: bool,
}

impl Default for StealthUpdateModuleData {
    /// `StealthUpdateModuleData()`.
    fn default() -> Self {
        StealthUpdateModuleData {
            update: UpdateModuleData::default(),
            hint_detectable_states: ObjectStatusMaskType::default(),
            required_status: ObjectStatusMaskType::default(),
            forbidden_status: ObjectStatusMaskType::default(),
            disguise_reveal_fx: None,
            disguise_fx: None,
            stealth_speed: 0.0,
            friendly_opacity_min: 0.5,
            friendly_opacity_max: 1.0,
            reveal_distance_from_target: 0.0,
            disguise_transition_frames: 0,
            disguise_reveal_transition_frames: 0,
            pulse_frames: 30,
            stealth_delay: u32::MAX,
            stealth_level: 0,
            black_market_check_frames: 0,
            enemy_detection_eva_event: EVA_INVALID,
            own_detection_eva_event: EVA_INVALID,
            innate_stealth: true,
            order_idle_enemies_to_attack_me_upon_reveal: false,
            team_disguised: false,
            use_rider_stealth: false,
            granted_by_special_power: false,
        }
    }
}

/// `StealthUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<StealthUpdateModuleData>; 23] = [
    FieldParse::new("StealthDelay", |ini, d, _| { d.stealth_delay = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("MoveThresholdSpeed", |ini, d, _| { d.stealth_speed = ini.parse_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::new("StealthForbiddenConditions", |ini, d, _| ini.parse_bit_string32(&mut d.stealth_level, &STEALTH_LEVEL_NAMES), UserData::None),
    FieldParse::new("HintDetectableConditions", |ini, d, _| d.hint_detectable_states.parse_from_ini(ini), UserData::None),
    FieldParse::new("RequiredStatus", |ini, d, _| d.required_status.parse_from_ini(ini), UserData::None),
    FieldParse::new("ForbiddenStatus", |ini, d, _| d.forbidden_status.parse_from_ini(ini), UserData::None),
    FieldParse::new("FriendlyOpacityMin", |ini, d, _| { d.friendly_opacity_min = ini.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("FriendlyOpacityMax", |ini, d, _| { d.friendly_opacity_max = ini.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("PulseFrequency", |ini, d, _| { d.pulse_frames = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("DisguisesAsTeam", |ini, d, _| { d.team_disguised = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("RevealDistanceFromTarget", |ini, d, _| { d.reveal_distance_from_target = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("OrderIdleEnemiesToAttackMeUponReveal", |ini, d, _| { d.order_idle_enemies_to_attack_me_upon_reveal = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("DisguiseFX", |ini, d, _| { d.disguise_fx = CrateCollideModuleData::parse_fx_list(ini)?; Ok(()) }, UserData::None),
    FieldParse::new("DisguiseRevealFX", |ini, d, _| { d.disguise_reveal_fx = CrateCollideModuleData::parse_fx_list(ini)?; Ok(()) }, UserData::None),
    FieldParse::new("DisguiseTransitionTime", |ini, d, _| { d.disguise_transition_frames = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("DisguiseRevealTransitionTime", |ini, d, _| { d.disguise_reveal_transition_frames = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("InnateStealth", |ini, d, _| { d.innate_stealth = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("UseRiderStealth", |ini, d, _| { d.use_rider_stealth = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("EnemyDetectionEvaEvent", |ini, d, _| { d.enemy_detection_eva_event = parse_eva_message(ini)?; Ok(()) }, UserData::None),
    FieldParse::new("OwnDetectionEvaEvent", |ini, d, _| { d.own_detection_eva_event = parse_eva_message(ini)?; Ok(()) }, UserData::None),
    FieldParse::new("BlackMarketCheckDelay", |ini, d, _| { d.black_market_check_frames = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("GrantedBySpecialPower", |ini, d, _| { d.granted_by_special_power = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl StealthUpdateModuleData {
    /// `buildFieldParse(p)`: `ModuleData`'s (no fields of its own besides the behaviour base's), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, StealthUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<StealthUpdateModuleData> {
        let mut data = StealthUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, StealthUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class StealthUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct StealthUpdate {
    /// The `UpdateModule` base (the scheduling half; it processes `DISABLED_HELD` too).
    update: UpdateModule,
    /// `getStealthUpdateModuleData()`.
    data: Arc<StealthUpdateModuleData>,
    /// `m_stealthAllowedFrame`.
    stealth_allowed_frame: u32,
    /// `m_detectionExpiresFrame`.
    detection_expires_frame: u32,
    /// `m_nextBlackMarketCheckFrame` (`mutable`).
    next_black_market_check_frame: u32,
    /// `m_enabled`.
    enabled: bool,
    /// `m_pulsePhaseRate`.
    pulse_phase_rate: f32,
    /// `m_pulsePhase`.
    pulse_phase: f32,
    /// `m_disguiseAsPlayerIndex`.
    disguise_as_player_index: i32,
    /// `m_disguiseAsTemplate`.
    disguise_as_template: Option<Arc<ThingTemplate>>,
    /// `m_disguiseTransitionFrames`.
    disguise_transition_frames: u32,
    /// `m_disguiseHalfpointReached`.
    disguise_halfpoint_reached: bool,
    /// `m_transitioningToDisguise`.
    transitioning_to_disguise: bool,
    /// `m_disguised`.
    disguised: bool,
    /// `m_framesGranted`.
    frames_granted: u32,
    /// `m_xferRestoreDisguise` (not saved).
    xfer_restore_disguise: bool,
    /// The world's statics (`TheThingFactory` for a load), kept by the constructor.
    statics: Option<Arc<GameStatics>>,
}

impl PartialEq for StealthUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.state_words() == other.state_words()
            && self.disguise_template_name() == other.disguise_template_name()
    }
}

/// `obj->getStealth()`, read from the object's slot (`None`: no stealth update, or its body is out).
pub fn stealth_update_of(world: &World, obj: ObjectId) -> Option<&StealthUpdate> {
    let idx = world.objects.find(obj)?.stealth?;
    match world.peek_module_body(ModuleHandle { object: obj, index: idx })? {
        UpdateBody::StealthUpdate(s) => Some(s),
        _ => None,
    }
}

impl StealthUpdate {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base (processing `DISABLED_HELD` too), the data (`INI_INVALID_DATA` for
    /// null data) and the fields the constructor sets without the world.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<StealthUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let mut update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        // getDisabledTypesToProcess(): MAKE_DISABLED_MASK( DISABLED_HELD ) "Still gets called, even if held -ML"
        let mut mask = crate::common::disabled_types::DisabledMaskType::default();
        mask.set(DISABLED_HELD, 1);
        update.set_disabled_types_to_process(mask);
        let enabled = !data.team_disguised;
        Ok(StealthUpdate {
            update,
            data,
            stealth_allowed_frame: 0,
            detection_expires_frame: 0,
            next_black_market_check_frame: 0,
            enabled,
            pulse_phase_rate: 0.2,
            pulse_phase: 0.0,
            disguise_as_player_index: -1,
            disguise_as_template: None,
            disguise_transition_frames: 0,
            disguise_halfpoint_reached: false,
            transitioning_to_disguise: false,
            disguised: false,
            frames_granted: 0,
            xfer_restore_disguise: false,
            statics: None,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getStealthUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<StealthUpdateModuleData> {
        &self.data
    }

    /// The state the test compares: the two frames, the black market frame, the flags, the pulse, the disguise, the grant.
    pub fn state_words(&self) -> [u32; 13] {
        [
            self.stealth_allowed_frame,
            self.detection_expires_frame,
            self.next_black_market_check_frame,
            u32::from(self.enabled),
            self.pulse_phase_rate.to_bits(),
            self.pulse_phase.to_bits(),
            self.disguise_as_player_index as u32,
            self.disguise_transition_frames,
            u32::from(self.disguise_halfpoint_reached),
            u32::from(self.transitioning_to_disguise),
            u32::from(self.disguised),
            self.frames_granted,
            u32::from(self.xfer_restore_disguise),
        ]
    }

    /// The disguise template's name (empty for none).
    pub fn disguise_template_name(&self) -> Vec<u8> {
        self.disguise_as_template.as_ref().map_or(Vec::new(), |t| t.get_name().to_vec())
    }

    /// `isDisguised()`.
    pub fn is_disguised(&self) -> bool {
        self.disguise_as_template.is_some()
    }

    /// `getDisguisedPlayerIndex()`.
    pub fn get_disguised_player_index(&self) -> i32 {
        self.disguise_as_player_index
    }

    /// `getDisguisedTemplate()`.
    pub fn get_disguised_template(&self) -> Option<&Arc<ThingTemplate>> {
        self.disguise_as_template.as_ref()
    }

    /// `getFriendlyOpacity()`: the minimum.
    pub fn get_friendly_opacity(&self) -> f32 {
        self.data.friendly_opacity_min
    }

    /// `getStealthDelay()`.
    pub fn get_stealth_delay(&self) -> u32 {
        self.data.stealth_delay
    }

    /// `getStealthLevel()`.
    pub fn get_stealth_level(&self) -> u32 {
        self.data.stealth_level
    }

    /// `getEnemyDetectionEvaEvent()`.
    pub fn get_enemy_detection_eva_event(&self) -> i32 {
        self.data.enemy_detection_eva_event
    }

    /// `getOwnDetectionEvaEvent()`.
    pub fn get_own_detection_eva_event(&self) -> i32 {
        self.data.own_detection_eva_event
    }

    /// `getOrderIdleEnemiesToAttackMeUponReveal()`.
    pub fn get_order_idle_enemies_to_attack_me_upon_reveal(&self) -> bool {
        self.data.order_idle_enemies_to_attack_me_upon_reveal
    }

    /// `isGrantedBySpecialPower()`.
    pub fn is_granted_by_special_power(&self) -> bool {
        self.data.granted_by_special_power
    }

    /// `isTemporaryGrant()`.
    pub fn is_temporary_grant(&self) -> bool {
        self.frames_granted > 0
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

    /// `crc` with the scheduling half the slot holds.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
        world.cuts.module_errors.push((me, IniError::undefined(u)));
    }

    fn calc_sleep_time(&self) -> UpdateSleepTime {
        if self.enabled {
            UPDATE_SLEEP_NONE
        } else {
            UPDATE_SLEEP_FOREVER
        }
    }

    fn can_disguise(&self) -> bool {
        self.data.team_disguised
    }

    fn emit(world: &mut World, object_id: ObjectId, op: StealthClientOp) {
        world.emit(ObjectClientEvent::Stealth { object_id, op });
    }

    /// `TheAudio->addAudioEvent(&sound)` for a copy of one of the template's sounds, with `setObjectID(self)`.
    fn template_sound(world: &mut World, id: ObjectId, sound: ThingTemplateAudioType) {
        let Some(name) = world.objects.find(id).map(|o| o.template().get_audio(sound).event_name.clone()) else { return };
        world.emit(ObjectClientEvent::AudioAddEvent { name, object_id: id, position: None, player_index: None });
    }

    /// `receiveGrant(active, frames)`: stealth on (now) or off, passed on to a rider change container's rider.
    pub fn receive_grant(&mut self, world: &mut World, me: ModuleHandle, active: bool, frames: u32) {
        let id = me.object;
        if world.objects.find(id).is_none() {
            return;
        }
        if self.can_disguise() {
            return; // so bombtrucks and stuff do not get foiled by this.
        }
        self.enabled = active;
        let mask = status_mask(&[OBJECT_STATUS_CAN_STEALTH, OBJECT_STATUS_STEALTHED]);
        if self.enabled {
            world.set_status(id, mask, true);
            self.stealth_allowed_frame = world.frame();
            update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
            if !crate::logic::modules::plant(1961) {
                self.frames_granted = frames;
            }
        } else {
            world.clear_status(id, mask);
            self.stealth_allowed_frame = FOREVER;
            self.frames_granted = 0;
            if world.has_drawable(id) {
                Self::emit(world, id, StealthClientOp::EffectiveOpacity { opacity: 1.0, explicit: -1.0 });
            }
        }
        if let Some(contain) = world.get_contain(id) {
            if world.module_call(id, contain, ModuleCall::IsRiderChangeContain).as_bool() {
                let rider = world.module_call(id, contain, ModuleCall::GetRider).as_id();
                if let Some(stealth) = world.objects.find(rider).and_then(|o| o.stealth) {
                    world.module_call(rider, stealth, ModuleCall::StealthReceiveGrant { active, frames });
                }
            }
        }
    }

    /// `allowedToStealth(stealthOwner)`.
    fn allowed_to_stealth(&mut self, world: &mut World, me: ModuleHandle, stealth_owner: ObjectId) -> bool {
        let id = me.object;
        let now = world.frame();
        let mut flags = self.data.stealth_level;
        if id != stealth_owner {
            // "the rider determines if the container can stealth or not"
            if let Some(s) = stealth_update_of(world, stealth_owner) {
                flags = s.get_stealth_level();
            }
        }
        let Some(o) = world.objects.find(id) else { return false };
        if o.is_kind_of(KINDOF_SPAWNS_ARE_THE_WEAPONS) {
            if let Some(sb) = world.get_spawn_behavior_interface(id) {
                if !world.module_call(id, sb, ModuleCall::SpawnAreAllSlavesStealthed).as_bool() {
                    world.module_call(id, sb, ModuleCall::SpawnRevealSlaves);
                    return false;
                }
            }
        }
        let status = |w: &World| w.objects.find(id).map(|o| o.status).unwrap_or_default();
        if flags & STEALTH_NOT_WHILE_ATTACKING != 0 && status(world).test(OBJECT_STATUS_IS_FIRING_WEAPON) {
            return false;
        }
        if flags & STEALTH_NOT_WHILE_USING_ABILITY != 0 && status(world).test(OBJECT_STATUS_IS_USING_ABILITY) {
            return false;
        }
        if flags & STEALTH_ONLY_WITH_BLACK_MARKET != 0 && self.next_black_market_check_frame < now {
            // randomize timer a little incase we have a whole bunch on the same frame.
            let r = world.random.game_logic_random_value(0, 10) as u32;
            self.next_black_market_check_frame = self.next_black_market_check_frame.wrapping_add(self.data.black_market_check_frames.wrapping_add(r));
            let Some(player) = world.get_controlling_player(id) else {
                Self::undefined(world, me, Undefined::NullControllingPlayer);
                return false;
            };
            let black_market = world.player_iterate_objects(player).into_iter().any(|o| {
                world.objects.find(o).is_some_and(|o| {
                    o.is_kind_of(KINDOF_FS_BLACK_MARKET) && !o.is_effectively_dead() && !o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION) && !o.test_status(OBJECT_STATUS_SOLD)
                })
            });
            if !black_market {
                return false;
            }
        }
        if !world.objects.find(stealth_owner).is_some_and(|o| o.test_status(OBJECT_STATUS_CAN_STEALTH)) {
            return false;
        }
        if flags & STEALTH_NOT_WHILE_TAKING_DAMAGE != 0 {
            let Some(body) = world.objects.find(id).and_then(|o| o.body) else {
                Self::undefined(world, me, Undefined::NullBodyModule);
                return false;
            };
            let stamp = |w: &mut World| match w.module_call(id, body, ModuleCall::GetLastDamageTimestamp) {
                ModuleReply::Int(t) => t as u32,
                _ => 0,
            };
            if stamp(world) >= now.wrapping_sub(1) {
                // Only if it's not healing damage.
                let damage_type = match world.module_call(id, body, ModuleCall::GetLastDamageInfo) {
                    ModuleReply::Damage(info) => info.input.damage_type,
                    _ => 0,
                };
                // "But it's initialized to 0xffffffff so we don't think we took damage on the first frame."
                if damage_type != DAMAGE_HEALING && stamp(world) != 0xffff_ffff {
                    return false;
                }
            }
        }
        let st = status(world);
        // We need all required status or else we fail
        if self.data.required_status.any() && !st.test_for_all(&self.data.required_status) {
            return false;
        }
        // If we have any forbidden statii, then fail
        if st.test_for_any(&self.data.forbidden_status) && !crate::logic::modules::plant(1962) {
            return false;
        }
        if flags & STEALTH_NOT_WHILE_FIRING_WEAPON != 0 && st.test(OBJECT_STATUS_IS_FIRING_WEAPON) {
            if flags & STEALTH_NOT_WHILE_FIRING_WEAPON == STEALTH_NOT_WHILE_FIRING_WEAPON {
                // Not allowed to stealth while firing ANY weapon!
                return false;
            }
            let last_frame = now.wrapping_sub(1);
            let o = world.objects.find(id).expect("checked above");
            for (flag, slot) in WEAPON_SLOTS {
                if flags & flag != 0 && o.weapon_set.get_weapon_in_weapon_slot(slot).is_some_and(|w| w.last_shot_frame() >= last_frame) {
                    return false;
                }
            }
        }
        let contained_by = world.objects.find(id).map_or(INVALID_ID, |o| o.contained_by);
        if world.objects.find(contained_by).is_some() {
            if let Some(contain) = world.get_contain(contained_by) {
                if !world.module_call(contained_by, contain, ModuleCall::IsGarrisonable).as_bool() {
                    return false;
                }
            }
        }
        // new past-alpha feature, grr...
        if flags & STEALTH_NOT_WHILE_RIDERS_ATTACKING != 0 {
            if let Some(contain) = world.get_contain(id) {
                if world.module_call(id, contain, ModuleCall::IsPassengerAllowedToFire(INVALID_ID)).as_bool()
                    && world.module_call(id, contain, ModuleCall::IsAnyRiderAttacking).as_bool()
                {
                    return false;
                }
            }
        }
        if flags & STEALTH_NOT_WHILE_MOVING != 0 {
            if let Some(physics) = world.objects.find(id).and_then(|o| o.physics) {
                if world.module_call(id, physics, ModuleCall::PhysicsGetVelocityMagnitude).as_real(0.0) > self.data.stealth_speed {
                    return false;
                }
            }
        }
        if world.objects.find(id).is_some_and(|o| o.test_script_status_bit(OBJECT_STATUS_SCRIPT_UNSTEALTHED)) {
            // We can't stealth because a script disabled this ability for this object!
            return false;
        }
        true
    }

    /// `hintDetectableWhileUnstealthed()`: the local player's own object in a hinted state shows its second material pass.
    fn hint_detectable_while_unstealthed(&self, world: &mut World, id: ObjectId) {
        let Some(st) = world.objects.find(id).map(|o| o.status) else { return };
        if self.data.hint_detectable_states.test_for_any(&st) && world.get_controlling_player(id) == world.players.local_index() && world.has_drawable(id) {
            Self::emit(world, id, StealthClientOp::SecondMaterialPassOpacity(1.0));
        }
    }

    /// `calcStealthedStatusForPlayer(obj, ThePlayerList->getLocalPlayer())`; `None` where upstream reads a `NULL` local player.
    fn calc_stealthed_status_for_local_player(&self, world: &World, id: ObjectId) -> Option<i32> {
        let o = world.objects.find(id)?;
        if o.is_effectively_dead() {
            return Some(stealth_look::NONE); // making sure he turns visible when he dies
        }
        if !o.test_status(OBJECT_STATUS_STEALTHED) {
            return Some(stealth_look::NONE);
        }
        let local = world.players.local_index();
        let team = o.team;
        let mut r = if team != 0 {
            let default_team = world.cuts.player_default_team(local?);
            world.cuts.team_relationship(team, Some(default_team).filter(|&t| t != 0))
        } else {
            Relationship::Neutral
        };
        if !world.cuts.player_is_active(local?) {
            // Observer players are friends to everyone!
            r = Relationship::Allies;
        }
        if self.can_disguise() {
            return Some(if r != Relationship::Allies && self.is_disguised() { stealth_look::DISGUISED_ENEMY } else { stealth_look::NONE });
        }
        Some(match (o.test_status(OBJECT_STATUS_DETECTED), r == Relationship::Allies) {
            (true, true) => stealth_look::VISIBLE_FRIENDLY_DETECTED,
            (true, false) => stealth_look::VISIBLE_DETECTED,
            (false, true) if !crate::logic::modules::plant(1967) => stealth_look::VISIBLE_FRIENDLY,
            (false, _) => stealth_look::INVISIBLE,
        })
    }

    /// `calcStealthOwner()`: the first rider for `UseRiderStealth` with a contain module that holds one, else the object.
    fn calc_stealth_owner(&self, world: &mut World, id: ObjectId) -> ObjectId {
        if self.data.use_rider_stealth {
            if let Some(contain) = world.get_contain(id) {
                if let ModuleReply::Ids(riders) = world.module_call(id, contain, ModuleCall::GetContainedItems) {
                    if let Some(&first) = riders.first() {
                        return first;
                    }
                }
            }
        }
        id
    }

    /// `markAsDetected(numFrames)`: detected for the stealth delay (0) or at least `num_frames` more; a disguise is dropped, and the idle enemies
    /// that can see the object are woken to attack it if the rules say so.
    pub fn mark_as_detected(&mut self, world: &mut World, me: ModuleHandle, num_frames: u32) {
        let id = me.object;
        let stealth_owner = self.calc_stealth_owner(world, id);
        let (stealth_delay, order_idles_to_attack) = if id == stealth_owner {
            (Some(self.data.stealth_delay), Some(self.data.order_idle_enemies_to_attack_me_upon_reveal))
        } else {
            match stealth_update_of(world, stealth_owner) {
                Some(s) => (Some(s.get_stealth_delay()), Some(s.get_order_idle_enemies_to_attack_me_upon_reveal())),
                None => (None, None),
            }
        };
        let this_player = world.get_controlling_player(id);
        // If we are disguised, remove the disguise permanently!
        if self.is_disguised() {
            self.disguise_as_object(world, me, INVALID_ID);
        }
        let now = world.frame();
        if num_frames == 0 {
            // "If numFrames is zero (the default value), use the stealth delay specified in the ini file."
            let Some(delay) = stealth_delay else {
                Self::undefined(world, me, Undefined::UninitialisedLocal);
                return;
            };
            self.detection_expires_frame = now.wrapping_add(delay).wrapping_add(u32::from(crate::logic::modules::plant(1964)));
        } else if self.detection_expires_frame < now.wrapping_add(num_frames) {
            self.detection_expires_frame = now.wrapping_add(num_frames);
        }
        let Some(order_idles_to_attack) = order_idles_to_attack else {
            Self::undefined(world, me, Undefined::UninitialisedLocal);
            return;
        };
        if order_idles_to_attack {
            let Some(this_player) = this_player else {
                Self::undefined(world, me, Undefined::NullControllingPlayer);
                return;
            };
            let my_pos = world.objects.find(id).map(|o| o.get_position()).unwrap_or_default();
            for n in 0..world.players.player_count() {
                if world.players.get_nth(n).is_none() {
                    continue;
                }
                if world.cuts.player_relationship_to_player_default_team(n, this_player) != Relationship::Enemies {
                    continue;
                }
                for obj in world.player_iterate_objects(n) {
                    // setWakeupIfInRange
                    let Some(ai) = world.objects.find(obj).and_then(|o| o.ai) else { continue };
                    let vision = world.get_vision_range(obj);
                    let mut src = world.objects.find(obj).map(|o| o.get_position()).unwrap_or_default();
                    src.sub(&my_pos);
                    if src.length() > vision {
                        continue;
                    }
                    world.module_call(obj, ai, ModuleCall::AiWakeUpAndAttemptToTarget);
                }
            }
        }
    }

    /// `disguiseAsObject(target)`: start disguising as the target (as what it is disguised as, if it is), or, with no target, start dropping a
    /// disguise.
    pub fn disguise_as_object(&mut self, world: &mut World, me: ModuleHandle, target: ObjectId) {
        let id = me.object;
        let target_player = world.objects.find(target).and_then(|_| world.get_controlling_player(target));
        if let Some(target_player) = target_player {
            // the target's getStealth() and its disguise; on the object itself that is this module, out of its slot while it is called
            let disguise = if target == id {
                self.disguise_as_template.clone().map(|t| (t, self.disguise_as_player_index))
            } else {
                stealth_update_of(world, target).and_then(|s| s.get_disguised_template().cloned().map(|t| (t, s.get_disguised_player_index())))
            };
            match disguise {
                Some((template, player)) => {
                    self.disguise_as_template = Some(template);
                    self.disguise_as_player_index = player;
                }
                None => {
                    self.disguise_as_template = world.objects.find(target).map(|o| o.template().clone());
                    self.disguise_as_player_index = target_player;
                }
            }
            self.enabled = true;
            self.transitioning_to_disguise = true; // Means we are gaining disguise over time.
            self.disguise_transition_frames = self.data.disguise_transition_frames;
            self.disguise_halfpoint_reached = false;
            // Wake up so I can process!
            update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
        } else if self.disguised {
            self.disguise_as_template = None;
            self.disguise_as_player_index = 0;
            self.disguise_transition_frames = self.data.disguise_reveal_transition_frames;
            self.transitioning_to_disguise = false; // Means we are losing the disguise over time.
            self.disguise_halfpoint_reached = false;
        }
        if world.has_drawable(id) {
            // `if( draw && draw->isSelected() ) TheControlBar->markUIDirty();`: the client knows the selection
            Self::emit(world, id, StealthClientOp::MarkUiDirtyIfSelected);
        }
    }

    /// The drawable swap of `changeVisualDisguise`: the client destroys the drawable and makes one of `template` (with the old one's position,
    /// orientation and model condition flags); the object is unbound and bound to the new one; the physics is reset. `Err` where upstream is
    /// undefined.
    fn swap_drawable(world: &mut World, id: ObjectId, template: Vec<u8>) -> Result<(), Undefined> {
        Self::emit(world, id, StealthClientOp::ReplaceDrawable { template });
        // TheGameClient->destroyDrawable( draw ): obj->friend_bindToDrawable( NULL )
        world.friend_bind_to_drawable(id, crate::common::game_type::INVALID_DRAWABLE_ID);
        // TheGameLogic->bindObjectAndDrawable(self, draw)
        let draw = world.cuts.allocate_drawable_id();
        world.friend_bind_to_drawable(id, draw);
        let physics = world.objects.find(id).and_then(|o| o.physics).ok_or(Undefined::NullPhysics)?;
        world.module_call(id, physics, ModuleCall::PhysicsResetDynamicPhysics);
        Ok(())
    }

    /// `changeVisualDisguise()`: put the disguise on (a drawable of the disguise template, in the disguise player's colour to non-allies) or take
    /// it off (a drawable of the object's own template), with the sound, the FX, the status and the model condition; the radar is reset.
    fn change_visual_disguise(&mut self, world: &mut World, me: ModuleHandle) {
        if let Err(u) = self.change_visual_disguise_inner(world, me) {
            Self::undefined(world, me, u);
        }
    }

    fn change_visual_disguise_inner(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        let id = me.object;
        // `Bool selected = draw->isSelected();` (unchecked)
        if !world.has_drawable(id) {
            return Err(Undefined::NullDrawable);
        }
        let night = world.cuts.globals.time_of_day_is_night;
        let position = world.objects.find(id).map(|o| o.get_position()).unwrap_or_default();
        if let Some(template) = self.disguise_as_template.clone() {
            // the colour: the disguise player's to neutrals and enemies of the local player (if active), the object's own otherwise
            let me_player = world.get_controlling_player(id).ok_or(Undefined::NullControllingPlayer)?;
            let local = world.players.local_index().ok_or(Undefined::NullLocalPlayer)?;
            let color = if world.cuts.player_relationship_to_player_default_team(me_player, local) != Relationship::Allies && world.cuts.player_is_active(local) {
                // Neutrals and enemies will see this disguised unit as the team it's disguised as.
                if world.players.get_nth(self.disguise_as_player_index).is_none() {
                    return Err(Undefined::NullControllingPlayer);
                }
                if night {
                    world.cuts.player_night_color(self.disguise_as_player_index)
                } else {
                    world.cuts.player_color(self.disguise_as_player_index)
                }
            } else if night {
                world.get_night_indicator_color(id)
            } else {
                world.get_indicator_color(id)
            };
            Self::swap_drawable(world, id, template.get_name().to_vec())?;
            Self::emit(world, id, StealthClientOp::SelectIfWasSelected);
            Self::emit(world, id, StealthClientOp::IndicatorColor(color));
            // Play a disguise sound!
            let sound = world.objects.find(id).map(|o| o.template().get_per_unit_sound(b"DisguiseStarted").event_name.clone()).unwrap_or_default();
            world.emit(ObjectClientEvent::AudioAddEvent { name: sound, object_id: id, position: None, player_index: None });
            if let Some(fx) = self.data.disguise_fx.clone() {
                world.emit(ObjectClientEvent::FxListDoFxPos { fx, position });
            }
            self.disguised = true;
            world.set_status(id, status_mask(&[OBJECT_STATUS_DISGUISED]), true);
            world.set_model_condition_state(id, MODELCONDITION_DISGUISED);
            // 33) Did the player ever build a "disguisable" unit and never used the disguise ability?
            let player = world.get_controlling_player(id).ok_or(Undefined::NullControllingPlayer)?;
            if let Some(p) = world.players.get_nth_mut(player) {
                p.academy_stats.record_vehicle_disguised();
            }
        } else if self.disguise_as_player_index != -1 {
            self.disguise_as_player_index = -1;
            let own = world.objects.find(id).map(|o| o.template().get_name().to_vec()).unwrap_or_default();
            let color = if night { world.get_night_indicator_color(id) } else { world.get_indicator_color(id) };
            // (the destroyed drawable's pointer, which the pool gives to the new drawable: see the module docs)
            Self::swap_drawable(world, id, own)?;
            Self::emit(world, id, StealthClientOp::IndicatorColor(color));
            Self::emit(world, id, StealthClientOp::SelectIfWasSelected);
            world.force_refresh_sub_object_upgrade_status(id);
            let successful_reveal = match world.objects.find(id).and_then(|o| o.ai) {
                Some(ai) => {
                    let victim = world.module_call(id, ai, ModuleCall::AiGetCurrentVictim).as_id();
                    world.objects.find(victim).is_some()
                }
                None => false,
            };
            let key: &[u8] = if successful_reveal { b"DisguiseRevealedSuccess" } else { b"DisguiseRevealedFailure" };
            let sound = world.objects.find(id).map(|o| o.template().get_per_unit_sound(key).event_name.clone()).unwrap_or_default();
            world.emit(ObjectClientEvent::AudioAddEvent { name: sound, object_id: id, position: None, player_index: None });
            if let Some(fx) = self.data.disguise_reveal_fx.clone() {
                world.emit(ObjectClientEvent::FxListDoFxPos { fx, position });
            }
            self.disguised = false;
            world.clear_status(id, status_mask(&[OBJECT_STATUS_DISGUISED]));
            world.clear_model_condition_state(id, MODELCONDITION_DISGUISED);
        }
        // Reset the radar (determines color on add)
        if !crate::logic::modules::plant(1968) {
            world.radar_remove_object(id);
            world.radar_add_object(id);
        }
        // couldn't possibly need to restore a disguise now :)
        self.xfer_restore_disguise = false;
        Ok(())
    }

    /// The disguise template by name; a load finds it with `TheThingFactory->findTemplate(name)` (the world's factory, kept by the constructor),
    /// `SC_INVALID_DATA` for an unknown one.
    fn xfer_disguise_template(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut name = self.disguise_template_name();
        xfer.xfer_ascii_string(&mut name)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.disguise_as_template = None;
            if !name.is_empty() {
                let found = self.statics.as_ref().and_then(|s| s.thing_factory.find_template(&name)).map(|t| Arc::new(t.get_final_override().data.clone()));
                match found {
                    Some(t) => self.disguise_as_template = Some(t),
                    // throw SC_INVALID_DATA (a `SaveCode`, not an `XferStatus`: `ErrorUnknown`, as `StateMachine` maps it)
                    None => return Err(XferStatus::ErrorUnknown),
                }
            }
        }
        Ok(())
    }
}

impl UpdateBehavior for StealthUpdate {
    const XFER_VERSION: XferVersion = 2;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let id = me.object;
        // restore disguise if we need to from a game load
        if self.xfer_restore_disguise {
            // (the hidden state of the old drawable is kept in the new one: the client's)
            self.change_visual_disguise(world, me);
            if world.has_drawable(id) {
                Self::emit(world, id, StealthClientOp::HideIfWasHidden);
            }
        }
        let stealth_owner = self.calc_stealth_owner(world, id);
        let stealth_delay = if id == stealth_owner { Some(self.data.stealth_delay) } else { stealth_update_of(world, stealth_owner).map(|s| s.get_stealth_delay()) };
        let now = world.frame();
        if !self.enabled {
            return self.calc_sleep_time();
        }
        let Some(o) = world.objects.find(id) else { return self.calc_sleep_time() };
        let is_mine = o.is_kind_of(KINDOF_MINE);
        let draw = world.has_drawable(id);
        if draw {
            if is_mine {
                // special case for mines
                Self::emit(world, id, StealthClientOp::EffectiveOpacity { opacity: 0.0, explicit: 0.0 });
            } else if self.disguise_transition_frames != 0 {
                self.disguise_transition_frames -= 1;
                let total = if self.transitioning_to_disguise { self.data.disguise_transition_frames } else { self.data.disguise_reveal_transition_frames };
                let factor = 1.0f32 - (self.disguise_transition_frames as f32 / total as f32);
                if factor >= 0.5 && !self.disguise_halfpoint_reached {
                    // Switch models at the halfway point
                    self.change_visual_disguise(world, me);
                    self.disguise_halfpoint_reached = true;
                }
                // Opacity ranges from full to none at midpoint and full again at the end
                let opacity = (1.0f32 - factor * 2.0).abs();
                let override_opacity = if opacity < 1.0 { 0.0 } else { 1.0 };
                Self::emit(world, id, StealthClientOp::EffectiveOpacity { opacity, explicit: override_opacity });
                if self.disguise_transition_frames == 0 && !self.transitioning_to_disguise {
                    // We're finished removing disguise so turn off stealth update.
                    self.enabled = false;
                    world.clear_status(id, status_mask(&[OBJECT_STATUS_STEALTHED]));
                    world.clear_status(id, status_mask(&[OBJECT_STATUS_DETECTED]));
                    return self.calc_sleep_time();
                }
            } else {
                // between one half and full opacity
                let opacity = 0.5 + crate::common::trig::sin(self.pulse_phase) * 0.5;
                Self::emit(world, id, StealthClientOp::EffectiveOpacity { opacity, explicit: -1.0 });
                self.pulse_phase += self.pulse_phase_rate * if crate::logic::modules::plant(1965) { 2.0 } else { 1.0 };
            }
        }
        let reveal_distance = self.data.reveal_distance_from_target;
        if reveal_distance > 0.0 {
            if let Some(ai) = world.objects.find(id).and_then(|o| o.ai) {
                let target = world.module_call(id, ai, ModuleCall::AiGetCurrentVictim).as_id();
                if world.objects.find(target).is_some() {
                    let dist_sqrd = world.partition_get_distance_squared_center_2d(id, target);
                    if dist_sqrd <= reveal_distance * reveal_distance {
                        // We're close enough to reveal ourselves
                        self.mark_as_detected(world, me, 0);
                        return self.calc_sleep_time();
                    }
                }
            }
        }
        // Deal with temporary stealth.
        if self.frames_granted > 0 {
            self.frames_granted -= 1;
            // If the last AI command given was by the player... then LOSE the stealth now!
            if let Some(ai) = world.objects.find(id).and_then(|o| o.ai) {
                if world.module_call(id, ai, ModuleCall::AiGetLastCommandSource) == ModuleReply::Int(CMD_FROM_PLAYER) {
                    // No exploits :)
                    self.receive_grant(world, me, false, 0);
                }
            }
            if self.frames_granted == 0 {
                // Disable it now that it has officially expired.
                self.receive_grant(world, me, false, 0);
            }
        }
        let stealthed = |w: &World| w.objects.find(id).is_some_and(|o| o.test_status(OBJECT_STATUS_STEALTHED));
        if self.allowed_to_stealth(world, me, stealth_owner) {
            // If I can stealth, don't attempt to Stealth until the timer is zero.
            if self.stealth_allowed_frame > now {
                return self.calc_sleep_time();
            }
            if !stealthed(world) {
                Self::template_sound(world, id, ThingTemplateAudioType::SoundStealthOn);
            }
            world.set_status(id, status_mask(&[OBJECT_STATUS_STEALTHED]), true);
        } else {
            let Some(delay) = stealth_delay else {
                Self::undefined(world, me, Undefined::UninitialisedLocal);
                return self.calc_sleep_time();
            };
            self.stealth_allowed_frame = now.wrapping_add(delay);
            // if you are destealthing on your own free will, play sound for all to hear
            if stealthed(world) {
                Self::template_sound(world, id, ThingTemplateAudioType::SoundStealthOn);
            }
            world.clear_status(id, status_mask(&[OBJECT_STATUS_STEALTHED]));
            self.hint_detectable_while_unstealthed(world, id);
        }
        let detected = |w: &World| w.objects.find(id).is_some_and(|o| o.test_status(OBJECT_STATUS_DETECTED));
        let mut detected_status_changed_this_frame = false;
        if self.detection_expires_frame > now || (crate::logic::modules::plant(1963) && self.detection_expires_frame == now) {
            // if this is the first time being detected, play stealth off sound
            if !detected(world) {
                detected_status_changed_this_frame = true;
                Self::template_sound(world, id, ThingTemplateAudioType::SoundStealthOff);
            }
            world.set_status(id, status_mask(&[OBJECT_STATUS_DETECTED]), true);
        } else {
            // if this is the first time your clearing the detected status, play the stealth on sound
            if detected(world) {
                detected_status_changed_this_frame = true;
                // Only play sound effect if the selected object is controllable.
                if world.is_locally_controlled(id) {
                    Self::template_sound(world, id, ThingTemplateAudioType::SoundStealthOn);
                }
            }
            world.clear_status(id, status_mask(&[OBJECT_STATUS_DETECTED]));
        }
        if detected_status_changed_this_frame && world.is_contained(id) {
            // tell our container to recalc his apparent controlling player
            let container = world.objects.find(id).map_or(INVALID_ID, |o| o.contained_by);
            if world.objects.find(container).is_some() {
                if let Some(contain) = world.get_contain(container) {
                    if world.module_call(container, contain, ModuleCall::IsGarrisonable).as_bool() {
                        world.module_call(container, contain, ModuleCall::RecalcApparentControllingPlayer);
                    }
                }
            }
        }
        if draw {
            match self.calc_stealthed_status_for_local_player(world, id) {
                Some(look) => Self::emit(world, id, StealthClientOp::StealthLook(look)),
                None => Self::undefined(world, me, Undefined::NullLocalPlayer),
            }
        }
        self.calc_sleep_time()
    }
    fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.statics = Some(world.statics.clone());
        let now = world.frame();
        self.stealth_allowed_frame = now.wrapping_add(self.data.stealth_delay);
        self.detection_expires_frame = 0;
        self.pulse_phase_rate = 0.2;
        self.pulse_phase = world.random.game_client_random_value_real(0.0, PI);
        if self.data.innate_stealth {
            // Giving innate stealth units this status bit allows other code to easily check the status bit.
            world.set_status(me.object, status_mask(&[OBJECT_STATUS_CAN_STEALTH]), true);
        }
        // start active, since some stealths start enabled from the get-go
        update_behavior::set_wake_frame(world, if self.data.granted_by_special_power { UPDATE_SLEEP_FOREVER } else { UPDATE_SLEEP_NONE });
        // we do not need to restore a disguise
        self.xfer_restore_disguise = false;
    }
    /// The fields after the base, in order; `m_framesGranted` from version 2.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
        xfer.xfer_unsigned_int(&mut self.stealth_allowed_frame)?;
        xfer.xfer_unsigned_int(&mut self.detection_expires_frame)?;
        xfer.xfer_bool(&mut self.enabled)?;
        xfer.xfer_real(&mut self.pulse_phase_rate)?;
        xfer.xfer_real(&mut self.pulse_phase)?;
        xfer.xfer_int(&mut self.disguise_as_player_index)?;
        self.xfer_disguise_template(xfer)?;
        xfer.xfer_unsigned_int(&mut self.disguise_transition_frames)?;
        xfer.xfer_bool(&mut self.disguise_halfpoint_reached)?;
        xfer.xfer_bool(&mut self.transitioning_to_disguise)?;
        xfer.xfer_bool(&mut self.disguised)?;
        if version >= 2 && !crate::logic::modules::plant(1966) {
            xfer.xfer_unsigned_int(&mut self.frames_granted)?;
        }
        Ok(())
    }
    /// "we will need to restore our disguise when the game is ready to run".
    fn load_post_process_fields(&mut self) {
        if self.is_disguised() {
            self.xfer_restore_disguise = true;
        }
    }
    /// The stealth interface (`getStealth()`).
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::StealthMarkAsDetected => {
                self.mark_as_detected(world, me, 0);
                ModuleReply::Done
            }
            ModuleCall::StealthMarkAsDetectedFor(frames) => {
                self.mark_as_detected(world, me, frames);
                ModuleReply::Done
            }
            ModuleCall::StealthReceiveGrant { active, frames } => {
                self.receive_grant(world, me, active, frames);
                ModuleReply::Done
            }
            ModuleCall::StealthIsTemporaryGrant => ModuleReply::Bool(self.is_temporary_grant()), // batch R44 (SupplyCenterDockUpdate)
            ModuleCall::StealthAllowedToStealth { owner } => ModuleReply::Bool(self.allowed_to_stealth(world, me, owner)), // batch R73 (SpawnBehavior)
            ModuleCall::StealthDisguiseAsObject(target) => {
                self.disguise_as_object(world, me, target);
                ModuleReply::Done
            }
            ModuleCall::StealthGetEnemyDetectionEvaEvent => ModuleReply::Int(self.get_enemy_detection_eva_event()),
            ModuleCall::StealthGetOwnDetectionEvaEvent => ModuleReply::Int(self.get_own_detection_eva_event()),
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::StealthUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::StealthUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for StealthUpdate {
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
        self.load_post_process_fields();
    }
}

/// The factory's `NewModuleDataProc` for `StealthUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::StealthUpdate(Arc::new(StealthUpdateModuleData::friend_new_module_data(ini)?)))
}
