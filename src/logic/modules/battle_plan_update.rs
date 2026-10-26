// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/BattlePlanUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/BattlePlanUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `BattlePlanUpdate`: the strategy centre's three battle plans. Choosing a plan (`initiateIntentToDoSpecialPower` with command option one,
//! two or three: bombardment, hold the line, search and destroy) sets the desired plan; the update walks the building through its states,
//! each ready at `m_nextReadyFrame`: idle (no plan; it waits `TransitionIdleTime` after packing), unpacking (the door opens, its sound, the
//! radar event, the message to all, the announcement), active (the plan's bonuses for the player's army and the building) and packing (the
//! bonuses taken back, the door closes). A bombardment plan's turret is enabled while active, and centred before the building packs.
//!
//! `setBattlePlan` takes the old plan's bonuses away (`Player::changeBattlePlan(plan, -1, bonuses)`, the building's own: its health for
//! hold the line, its sight and stealth detection for search and destroy) and gives the new plan's; the empty plan paralyses the player's
//! troops (`ValidMemberKindOf` and not `InvalidMemberKindOf`) for `BattlePlanChangeParalyzeTime`.
//!
//! `Player::changeBattlePlan` is a record ([`CutCall::PlayerChangeBattlePlan`]) until the player's object operations are on `World`
//! (`player_objects::player_change_battle_plan` is translated); the troops are `World::player_iterate_objects`'s. It rewrites the
//! bonuses when the last plan of a kind goes, and every caller here resets them right after, so the record loses nothing the module reads.
//! The turret is the AI's ([`ModuleCall::AiGetWhichTurretForCurWeapon`] and its three calls); the sounds, the radar, the message and the
//! drawable are the client's.
//!
//! # Upstream facts kept
//!
//! - Packing a bombardment or hold-the-line plan plays the pack sound only if the *unpack* sound has a name (and sets the pack sound's
//!   object then).
//! - `TransitionIdleTime` has no initialiser: the allocator's zero.
//! - The strategy centre's hold-the-line health is raised whenever the scalar is not zero, and lowered whenever it is not one.
//! - Unchecked upstream, so undefined here: the special power module never found (`NullSpecialPowerModule`), a drawable missing when a door
//!   moves (`NullDrawable`), a body missing when the health changes (`NullBodyModule`), and a player missing when a plan is chosen
//!   (`NullControllingPlayer`).
//!
//! [`CutCall::PlayerChangeBattlePlan`]: crate::logic::world::object_cuts::CutCall::PlayerChangeBattlePlan
//!
//! `p1w13-sonnet`, batch R71, 2026-10-26.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::disabled_types::DISABLED_PARALYZED;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::KindOfMaskType;
use crate::common::model_state::*;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::body_module::{MAX_HEALTH_CHANGE_TYPE_NAMES, PRESERVE_RATIO};
use crate::logic::module::special_power_update_module::SpecialPowerUpdateModule;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::open_contain::CMD_FROM_AI;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::weapon_set_state::LOCKED_TEMPORARILY;
use crate::logic::weapon_types::WEAPONSET_VETERAN;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object::ModuleIndex;
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::player::BattlePlanBonuses;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `TransitionStatus`.
pub const TRANSITIONSTATUS_IDLE: i32 = 0;
pub const TRANSITIONSTATUS_UNPACKING: i32 = 1;
pub const TRANSITIONSTATUS_ACTIVE: i32 = 2;
pub const TRANSITIONSTATUS_PACKING: i32 = 3;
/// `BattlePlanStatus`.
pub const PLANSTATUS_NONE: i32 = 0;
pub const PLANSTATUS_BOMBARDMENT: i32 = 1;
pub const PLANSTATUS_HOLDTHELINE: i32 = 2;
pub const PLANSTATUS_SEARCHANDDESTROY: i32 = 3;
/// `CommandOption`'s `OPTION_ONE`, `OPTION_TWO`, `OPTION_THREE` (`ControlBar.h`).
pub const OPTION_ONE: u32 = 0x0000_2000;
pub const OPTION_TWO: u32 = 0x0000_4000;
pub const OPTION_THREE: u32 = 0x0000_8000;
/// `RADAR_EVENT_BATTLE_PLAN`.
pub const RADAR_EVENT_BATTLE_PLAN: i32 = 7;
/// `TURRET_INVALID`.
const TURRET_INVALID: i32 = -1;
/// `PRIMARY_WEAPON`.
const PRIMARY_WEAPON: i32 = 0;

/// `class BattlePlanUpdateModuleData : public ModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct BattlePlanUpdateModuleData {
    pub update: UpdateModuleData,
    /// `m_specialPowerTemplate`, by name (found in the store when used; `None` and an unknown name are `NULL`).
    pub special_power_template: Option<Vec<u8>>,
    pub bombardment_plan_animation_frames: u32,
    pub hold_the_line_plan_animation_frames: u32,
    pub search_and_destroy_plan_animation_frames: u32,
    /// `m_transitionIdleFrames` (no initialiser: the allocator's zero).
    pub transition_idle_frames: u32,
    pub bombardment_unpack_name: Vec<u8>,
    pub bombardment_pack_name: Vec<u8>,
    pub bombardment_message_label: Vec<u8>,
    pub bombardment_announcement_name: Vec<u8>,
    pub search_and_destroy_unpack_name: Vec<u8>,
    pub search_and_destroy_idle_name: Vec<u8>,
    pub search_and_destroy_pack_name: Vec<u8>,
    pub search_and_destroy_message_label: Vec<u8>,
    pub search_and_destroy_announcement_name: Vec<u8>,
    pub hold_the_line_unpack_name: Vec<u8>,
    pub hold_the_line_pack_name: Vec<u8>,
    pub hold_the_line_message_label: Vec<u8>,
    pub hold_the_line_announcement_name: Vec<u8>,
    pub valid_member_kind_of: KindOfMaskType,
    pub invalid_member_kind_of: KindOfMaskType,
    pub battle_plan_paralyze_frames: u32,
    pub hold_the_line_armor_damage_scalar: f32,
    pub search_and_destroy_sight_range_scalar: f32,
    pub strategy_center_search_and_destroy_sight_range_scalar: f32,
    pub strategy_center_search_and_destroy_detects_stealth: bool,
    pub strategy_center_hold_the_line_max_health_scalar: f32,
    pub strategy_center_hold_the_line_max_health_change_type: i32,
    pub vision_object_name: Vec<u8>,
}

impl Default for BattlePlanUpdateModuleData {
    /// `BattlePlanUpdateModuleData()`, on the allocator's zeroes.
    fn default() -> Self {
        BattlePlanUpdateModuleData {
            update: UpdateModuleData::new(),
            special_power_template: None,
            bombardment_plan_animation_frames: 0,
            hold_the_line_plan_animation_frames: 0,
            search_and_destroy_plan_animation_frames: 0,
            transition_idle_frames: 0,
            bombardment_unpack_name: Vec::new(),
            bombardment_pack_name: Vec::new(),
            bombardment_message_label: Vec::new(),
            bombardment_announcement_name: Vec::new(),
            search_and_destroy_unpack_name: Vec::new(),
            search_and_destroy_idle_name: Vec::new(),
            search_and_destroy_pack_name: Vec::new(),
            search_and_destroy_message_label: Vec::new(),
            search_and_destroy_announcement_name: Vec::new(),
            hold_the_line_unpack_name: Vec::new(),
            hold_the_line_pack_name: Vec::new(),
            hold_the_line_message_label: Vec::new(),
            hold_the_line_announcement_name: Vec::new(),
            valid_member_kind_of: KindOfMaskType::default(),
            invalid_member_kind_of: KindOfMaskType::default(),
            battle_plan_paralyze_frames: 0,
            hold_the_line_armor_damage_scalar: 1.0,
            search_and_destroy_sight_range_scalar: 1.0,
            strategy_center_search_and_destroy_sight_range_scalar: 1.0,
            strategy_center_search_and_destroy_detects_stealth: true,
            strategy_center_hold_the_line_max_health_scalar: 1.0,
            strategy_center_hold_the_line_max_health_change_type: PRESERVE_RATIO,
            vision_object_name: Vec::new(),
        }
    }
}

macro_rules! string_field {
    ($name:literal, $field:ident) => {
        FieldParse::new($name, |i, d, _| { d.$field = i.parse_ascii_string()?; Ok(()) }, UserData::None)
    };
}

static FIELD_PARSE: [FieldParse<BattlePlanUpdateModuleData>; 29] = [
    FieldParse::new("SpecialPowerTemplate", |i, d, _| { d.special_power_template = Some(i.get_next_token(None)?); Ok(()) }, UserData::None),
    FieldParse::new("BombardmentPlanAnimationTime", |i, d, _| { d.bombardment_plan_animation_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("HoldTheLinePlanAnimationTime", |i, d, _| { d.hold_the_line_plan_animation_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("SearchAndDestroyPlanAnimationTime", |i, d, _| { d.search_and_destroy_plan_animation_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("TransitionIdleTime", |i, d, _| { d.transition_idle_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    string_field!("BombardmentPlanUnpackSoundName", bombardment_unpack_name),
    string_field!("BombardmentPlanPackSoundName", bombardment_pack_name),
    string_field!("BombardmentMessageLabel", bombardment_message_label),
    string_field!("BombardmentAnnouncementName", bombardment_announcement_name),
    string_field!("SearchAndDestroyPlanUnpackSoundName", search_and_destroy_unpack_name),
    string_field!("SearchAndDestroyPlanIdleLoopSoundName", search_and_destroy_idle_name),
    string_field!("SearchAndDestroyPlanPackSoundName", search_and_destroy_pack_name),
    string_field!("SearchAndDestroyMessageLabel", search_and_destroy_message_label),
    string_field!("SearchAndDestroyAnnouncementName", search_and_destroy_announcement_name),
    string_field!("HoldTheLinePlanUnpackSoundName", hold_the_line_unpack_name),
    string_field!("HoldTheLinePlanPackSoundName", hold_the_line_pack_name),
    string_field!("HoldTheLineMessageLabel", hold_the_line_message_label),
    string_field!("HoldTheLineAnnouncementName", hold_the_line_announcement_name),
    FieldParse::new("ValidMemberKindOf", |i, d, _| d.valid_member_kind_of.parse_from_ini(i), UserData::None),
    FieldParse::new("InvalidMemberKindOf", |i, d, _| d.invalid_member_kind_of.parse_from_ini(i), UserData::None),
    FieldParse::new("BattlePlanChangeParalyzeTime", |i, d, _| { d.battle_plan_paralyze_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("HoldTheLinePlanArmorDamageScalar", |i, d, _| { d.hold_the_line_armor_damage_scalar = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("SearchAndDestroyPlanSightRangeScalar", |i, d, _| { d.search_and_destroy_sight_range_scalar = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new(
        "StrategyCenterSearchAndDestroySightRangeScalar",
        |i, d, _| { d.strategy_center_search_and_destroy_sight_range_scalar = i.parse_real()?; Ok(()) },
        UserData::None,
    ),
    FieldParse::new(
        "StrategyCenterSearchAndDestroyDetectsStealth",
        |i, d, _| { d.strategy_center_search_and_destroy_detects_stealth = i.parse_bool()?; Ok(()) },
        UserData::None,
    ),
    FieldParse::new(
        "StrategyCenterHoldTheLineMaxHealthScalar",
        |i, d, _| { d.strategy_center_hold_the_line_max_health_scalar = i.parse_real()?; Ok(()) },
        UserData::None,
    ),
    FieldParse::new(
        "StrategyCenterHoldTheLineMaxHealthChangeType",
        |i, d, _| { d.strategy_center_hold_the_line_max_health_change_type = i.parse_index_list(&MAX_HEALTH_CHANGE_TYPE_NAMES)?; Ok(()) },
        UserData::None,
    ),
    string_field!("VisionObjectName", vision_object_name),
    FieldParse::end(),
];

impl BattlePlanUpdateModuleData {
    /// `buildFieldParse(p)`: `ModuleData`'s (nothing), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, BattlePlanUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<BattlePlanUpdateModuleData> {
        let mut data = BattlePlanUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, BattlePlanUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `BattlePlanUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::BattlePlanUpdate(Arc::new(BattlePlanUpdateModuleData::friend_new_module_data(ini)?)))
}

/// The sounds whose playing handles the module keeps (`m_bombardmentUnpack` ... `m_holdTheLinePack`), by index.
const BOMBARDMENT_UNPACK: usize = 0;
const BOMBARDMENT_PACK: usize = 1;
const SEARCH_AND_DESTROY_UNPACK: usize = 2;
const SEARCH_AND_DESTROY_IDLE: usize = 3;
const SEARCH_AND_DESTROY_PACK: usize = 4;
const HOLD_THE_LINE_UNPACK: usize = 5;
const HOLD_THE_LINE_PACK: usize = 6;

/// `class BattlePlanUpdate : public SpecialPowerUpdateModule`.
#[derive(Clone, Debug)]
pub struct BattlePlanUpdate {
    base: SpecialPowerUpdateModule,
    data: Arc<BattlePlanUpdateModuleData>,
    /// The ten sounds' names (`setEventName` in `onObjectCreated`: empty until then), the seven looping ones first (by the indices above),
    /// then the three announcements (bombardment, search and destroy, hold the line).
    sound_names: [Vec<u8>; 10],
    /// The seven sounds' playing handles (0: none).
    playing_handles: [u32; 7],
    /// The three announcements' positions (`setPosition` before each is played; `None` until then).
    announcement_positions: [Option<Coord3D>; 3],
    /// `m_specialPowerModule`.
    special_power_module: Option<ModuleIndex>,
    /// `m_status` (`TransitionStatus`).
    status: i32,
    /// `m_currentPlan`, `m_desiredPlan`, `m_planAffectingArmy` (`BattlePlanStatus`).
    current_plan: i32,
    desired_plan: i32,
    plan_affecting_army: i32,
    /// `m_nextReadyFrame`.
    next_ready_frame: u32,
    /// `m_invalidSettings`.
    invalid_settings: bool,
    /// `m_centeringTurret`.
    centering_turret: bool,
    /// `m_bonuses`.
    bonuses: BattlePlanBonuses,
    /// `m_visionObjectID`.
    vision_object_id: ObjectId,
}

impl PartialEq for BattlePlanUpdate {
    fn eq(&self, other: &Self) -> bool {
        self.base.update().behavior().object() == other.base.update().behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.state_words() == other.state_words()
            && self.sound_names == other.sound_names
            && self.special_power_module == other.special_power_module
    }
}

impl BattlePlanUpdate {
    pub const fn get_interface_mask() -> i32 {
        SpecialPowerUpdateModule::get_interface_mask()
    }

    /// The constructor (it touches no world): the base, idle with no plan, the bonuses at no change with the data's kinds.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<BattlePlanUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = SpecialPowerUpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        let bonuses = BattlePlanBonuses {
            armor_scalar: 1.0,
            bombardment: 0,
            search_and_destroy: 0,
            hold_the_line: 0,
            sight_range_scalar: 1.0,
            valid_kind_of: data.valid_member_kind_of,
            invalid_kind_of: data.invalid_member_kind_of,
        };
        Ok(BattlePlanUpdate {
            base,
            data,
            sound_names: Default::default(),
            playing_handles: [0; 7],
            announcement_positions: [None; 3],
            special_power_module: None,
            status: TRANSITIONSTATUS_IDLE,
            current_plan: PLANSTATUS_NONE,
            desired_plan: PLANSTATUS_NONE,
            plan_affecting_army: PLANSTATUS_NONE,
            next_ready_frame: 0,
            invalid_settings: false,
            centering_turret: false,
            bonuses,
            vision_object_id: INVALID_ID,
        })
    }

    pub fn update_module(&self) -> &UpdateModule {
        self.base.update()
    }

    pub fn module_data(&self) -> &Arc<BattlePlanUpdateModuleData> {
        &self.data
    }

    /// The state as words, for the tests that compare it with upstream's: the status, the three plans, the ready frame, the two flags, the
    /// bonuses, the vision object and the seven playing handles.
    pub fn state_words(&self) -> Vec<u32> {
        let b = &self.bonuses;
        let mut v = vec![
            self.status as u32,
            self.current_plan as u32,
            self.desired_plan as u32,
            self.plan_affecting_army as u32,
            self.next_ready_frame,
            u32::from(self.invalid_settings),
            u32::from(self.centering_turret),
            b.armor_scalar.to_bits(),
            b.bombardment as u32,
            b.search_and_destroy as u32,
            b.hold_the_line as u32,
            b.sight_range_scalar.to_bits(),
            self.vision_object_id as u32,
        ];
        v.extend(self.playing_handles);
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

    /// `onObjectCreated()`: the special power module, the sounds' names, the veteran weapon set, the primary weapon locked (with an AI)
    /// and the turret off. No special power template: invalid settings.
    fn on_object_created(&mut self, world: &mut World, us: ObjectId) {
        let d = self.data.clone();
        let template = d.special_power_template.as_deref().and_then(|n| world.statics.special_power_store.find_special_power_template(n)).map(|t| t.id());
        let Some(template) = template else {
            self.invalid_settings = true;
            return;
        };
        self.special_power_module = world.get_special_power_module(us, Some(template));
        self.sound_names = [
            d.bombardment_unpack_name.clone(),
            d.bombardment_pack_name.clone(),
            d.search_and_destroy_unpack_name.clone(),
            d.search_and_destroy_idle_name.clone(),
            d.search_and_destroy_pack_name.clone(),
            d.hold_the_line_unpack_name.clone(),
            d.hold_the_line_pack_name.clone(),
            d.bombardment_announcement_name.clone(),
            d.search_and_destroy_announcement_name.clone(),
            d.hold_the_line_announcement_name.clone(),
        ];
        if !crate::logic::modules::plant(2411) {
            world.set_weapon_set_flag(us, WEAPONSET_VETERAN);
        }
        if world.objects.find(us).is_some_and(|o| o.ai.is_some()) {
            // "lock it just till the weapon is empty or the attack is "done""
            world.set_weapon_lock(us, PRIMARY_WEAPON, LOCKED_TEMPORARILY);
        }
        self.enable_turret(world, us, false);
    }

    /// `initiateIntentToDoSpecialPower(template, ...)`: the desired plan from the command option.
    fn initiate_intent(&mut self, world: &mut World, us: ObjectId, template: u32, command_options: u32) -> Result<bool, IniError> {
        let sp = self.special_power_module.ok_or_else(|| IniError::undefined(Undefined::NullSpecialPowerModule))?;
        let ours = match world.module_call(us, sp, ModuleCall::GetSpecialPowerTemplateId) {
            ModuleReply::Int(id) => id as u32,
            _ => 0,
        };
        if ours != template {
            // "Check to make sure our modules are connected."
            return Ok(false);
        }
        self.desired_plan = if command_options & OPTION_ONE != 0 {
            PLANSTATUS_BOMBARDMENT
        } else if command_options & OPTION_TWO != 0 {
            if crate::logic::modules::plant(2412) { PLANSTATUS_SEARCHANDDESTROY } else { PLANSTATUS_HOLDTHELINE }
        } else if command_options & OPTION_THREE != 0 {
            PLANSTATUS_SEARCHANDDESTROY
        } else {
            // ("Selected an unsupported strategy for strategy center.")
            return Ok(false);
        };
        let player = world.get_controlling_player(us).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        world.cut(CutCall::AcademyStatsRecordBattlePlanSelected { player });
        Ok(true)
    }

    /// `getCommandOption()`.
    fn get_command_option(&self) -> u32 {
        match self.desired_plan {
            PLANSTATUS_BOMBARDMENT => OPTION_ONE,
            PLANSTATUS_HOLDTHELINE => OPTION_TWO,
            PLANSTATUS_SEARCHANDDESTROY => OPTION_THREE,
            _ => 0,
        }
    }

    /// `getActiveBattlePlan()`: the plan affecting the army while active, else none.
    fn get_active_battle_plan(&self) -> i32 {
        if self.status == TRANSITIONSTATUS_ACTIVE {
            self.plan_affecting_army
        } else {
            PLANSTATUS_NONE
        }
    }

    /// The AI's `getWhichTurretForCurWeapon()`, `None` without an AI or for `TURRET_INVALID`.
    fn cur_turret(world: &mut World, us: ObjectId) -> Option<(ModuleIndex, i32)> {
        let ai = world.objects.find(us).and_then(|o| o.ai)?;
        match world.module_call(us, ai, ModuleCall::AiGetWhichTurretForCurWeapon) {
            ModuleReply::Int(t) if t != TURRET_INVALID => Some((ai, t)),
            _ => None,
        }
    }

    /// `enableTurret(enable)`.
    fn enable_turret(&self, world: &mut World, us: ObjectId, enable: bool) {
        if let Some((ai, turret)) = Self::cur_turret(world, us) {
            world.module_call(us, ai, ModuleCall::AiSetTurretEnabled { turret, enabled: enable });
        }
    }

    /// `recenterTurret()`.
    fn recenter_turret(&self, world: &mut World, us: ObjectId) {
        if let Some((ai, turret)) = Self::cur_turret(world, us) {
            world.module_call(us, ai, ModuleCall::AiRecenterTurret { turret });
        }
    }

    /// `isTurretInNaturalPosition()`.
    fn is_turret_in_natural_position(&self, world: &mut World, us: ObjectId) -> bool {
        match Self::cur_turret(world, us) {
            Some((ai, turret)) => world.module_call(us, ai, ModuleCall::AiIsTurretInNaturalPosition { turret }).as_bool(),
            None => false,
        }
    }

    /// `TheAudio->addAudioEvent(&sound)` of a looping sound with the object: its playing handle.
    fn play(&mut self, world: &mut World, us: ObjectId, sound: usize) {
        self.playing_handles[sound] = world.audio_add_event(self.sound_names[sound].clone(), us);
    }

    /// `TheAudio->removeAudioEvent(sound.getPlayingHandle())`.
    fn stop(&self, world: &mut World, sound: usize) {
        world.audio_remove_event(self.playing_handles[sound]);
    }

    /// An announcement (`setPosition(obj pos)`, then `addAudioEvent`), if it has a name.
    fn announce(&mut self, world: &mut World, us: ObjectId, which: usize) {
        if self.sound_names[7 + which].is_empty() {
            return;
        }
        let pos = world.objects.find(us).map(|o| o.get_position()).unwrap_or_default();
        self.announcement_positions[which] = Some(pos);
        world.emit(ObjectClientEvent::AudioAddEvent { name: self.sound_names[7 + which].clone(), object_id: INVALID_ID, position: Some(pos), player_index: None });
    }

    /// `obj->getDrawable()->setAnimationLoopDuration(frames)` (the drawable read unchecked).
    fn animation_loop(world: &mut World, us: ObjectId, frames: u32) -> Result<(), IniError> {
        if !world.has_drawable(us) {
            return Err(IniError::undefined(Undefined::NullDrawable));
        }
        world.emit(ObjectClientEvent::DrawableSetAnimationLoopDuration { object_id: us, frames });
        Ok(())
    }

    /// `setStatus(newStatus)`: the old status's model condition and sound off, then the new one's.
    fn set_status(&mut self, world: &mut World, us: ObjectId, new_status: i32) -> Result<(), IniError> {
        let d = self.data.clone();
        if self.status == new_status {
            return Ok(());
        }
        // "Turn off old defining states and sounds"
        match self.status {
            TRANSITIONSTATUS_UNPACKING => match self.current_plan {
                PLANSTATUS_BOMBARDMENT => {
                    world.clear_model_condition_state(us, MODELCONDITION_DOOR_1_OPENING);
                    self.stop(world, BOMBARDMENT_UNPACK);
                }
                PLANSTATUS_HOLDTHELINE => {
                    world.clear_model_condition_state(us, MODELCONDITION_DOOR_2_OPENING);
                    self.stop(world, HOLD_THE_LINE_UNPACK);
                }
                PLANSTATUS_SEARCHANDDESTROY => {
                    world.clear_model_condition_state(us, MODELCONDITION_DOOR_3_OPENING);
                    self.stop(world, SEARCH_AND_DESTROY_UNPACK);
                }
                _ => {}
            },
            TRANSITIONSTATUS_ACTIVE => match self.current_plan {
                PLANSTATUS_BOMBARDMENT => world.clear_model_condition_state(us, MODELCONDITION_DOOR_1_WAITING_TO_CLOSE),
                PLANSTATUS_HOLDTHELINE => world.clear_model_condition_state(us, MODELCONDITION_DOOR_2_WAITING_TO_CLOSE),
                PLANSTATUS_SEARCHANDDESTROY => {
                    world.clear_model_condition_state(us, MODELCONDITION_DOOR_3_WAITING_TO_CLOSE);
                    self.stop(world, SEARCH_AND_DESTROY_IDLE);
                }
                _ => {}
            },
            TRANSITIONSTATUS_PACKING => match self.current_plan {
                PLANSTATUS_BOMBARDMENT => {
                    world.clear_model_condition_state(us, MODELCONDITION_DOOR_1_CLOSING);
                    self.stop(world, BOMBARDMENT_PACK);
                }
                PLANSTATUS_HOLDTHELINE => {
                    world.clear_model_condition_state(us, MODELCONDITION_DOOR_2_CLOSING);
                    self.stop(world, HOLD_THE_LINE_PACK);
                }
                PLANSTATUS_SEARCHANDDESTROY => {
                    world.clear_model_condition_state(us, MODELCONDITION_DOOR_3_CLOSING);
                    self.stop(world, SEARCH_AND_DESTROY_PACK);
                }
                _ => {}
            },
            _ => {}
        }
        let now = world.frame();
        // "Handle entering new status"
        match new_status {
            TRANSITIONSTATUS_IDLE => {
                self.current_plan = PLANSTATUS_NONE;
                self.next_ready_frame = now.wrapping_add(if crate::logic::modules::plant(2413) { 0 } else { d.transition_idle_frames });
            }
            TRANSITIONSTATUS_UNPACKING => {
                // "play a radar blip showing the battle plan change in the color of the owner"
                let player = world.get_controlling_player(us);
                let position = world.objects.find(us).map(|o| o.get_position()).unwrap_or_default();
                world.emit(ObjectClientEvent::RadarCreatePlayerEvent { player, position, event_type: RADAR_EVENT_BATTLE_PLAN });
                let (condition, frames, unpack, label, announcement) = match self.current_plan {
                    PLANSTATUS_BOMBARDMENT => (MODELCONDITION_DOOR_1_OPENING, d.bombardment_plan_animation_frames, BOMBARDMENT_UNPACK, &d.bombardment_message_label, 0),
                    PLANSTATUS_HOLDTHELINE => (MODELCONDITION_DOOR_2_OPENING, d.hold_the_line_plan_animation_frames, HOLD_THE_LINE_UNPACK, &d.hold_the_line_message_label, 2),
                    PLANSTATUS_SEARCHANDDESTROY => {
                        (MODELCONDITION_DOOR_3_OPENING, d.search_and_destroy_plan_animation_frames, SEARCH_AND_DESTROY_UNPACK, &d.search_and_destroy_message_label, 1)
                    }
                    _ => {
                        self.status = new_status;
                        return Ok(());
                    }
                };
                world.set_model_condition_state(us, condition);
                Self::animation_loop(world, us, frames)?;
                self.next_ready_frame = now.wrapping_add(frames);
                if !self.sound_names[unpack].is_empty() {
                    self.play(world, us, unpack);
                }
                // "display a message to *all* users"
                world.emit(ObjectClientEvent::UiMessage { label: label.clone() });
                self.announce(world, us, announcement);
            }
            TRANSITIONSTATUS_ACTIVE => {
                self.set_battle_plan(world, us, self.current_plan)?;
                match self.current_plan {
                    PLANSTATUS_BOMBARDMENT => world.set_model_condition_state(us, MODELCONDITION_DOOR_1_WAITING_TO_CLOSE),
                    PLANSTATUS_HOLDTHELINE => world.set_model_condition_state(us, MODELCONDITION_DOOR_2_WAITING_TO_CLOSE),
                    PLANSTATUS_SEARCHANDDESTROY => {
                        world.set_model_condition_state(us, MODELCONDITION_DOOR_3_WAITING_TO_CLOSE);
                        if !self.sound_names[SEARCH_AND_DESTROY_IDLE].is_empty() {
                            self.play(world, us, SEARCH_AND_DESTROY_IDLE);
                        }
                    }
                    _ => {}
                }
            }
            TRANSITIONSTATUS_PACKING => {
                self.set_battle_plan(world, us, PLANSTATUS_NONE)?;
                // (each pack sound plays if its plan's *unpack* sound has a name)
                let (condition, frames, unpack, pack) = match self.current_plan {
                    PLANSTATUS_BOMBARDMENT => (MODELCONDITION_DOOR_1_CLOSING, d.bombardment_plan_animation_frames, BOMBARDMENT_UNPACK, BOMBARDMENT_PACK),
                    PLANSTATUS_HOLDTHELINE => (MODELCONDITION_DOOR_2_CLOSING, d.hold_the_line_plan_animation_frames, HOLD_THE_LINE_UNPACK, HOLD_THE_LINE_PACK),
                    PLANSTATUS_SEARCHANDDESTROY => {
                        (MODELCONDITION_DOOR_3_CLOSING, d.search_and_destroy_plan_animation_frames, SEARCH_AND_DESTROY_UNPACK, SEARCH_AND_DESTROY_PACK)
                    }
                    _ => {
                        self.status = new_status;
                        return Ok(());
                    }
                };
                world.set_model_condition_state(us, condition);
                Self::animation_loop(world, us, frames)?;
                self.next_ready_frame = now.wrapping_add(frames);
                let gate = if crate::logic::modules::plant(2414) { pack } else { unpack };
                if !self.sound_names[gate].is_empty() {
                    self.play(world, us, pack);
                }
            }
            _ => {}
        }
        // "Set new status."
        self.status = new_status;
        Ok(())
    }

    /// `Player::changeBattlePlan(plan, delta, m_bonuses)` (a record: see the module's doc).
    fn change_battle_plan(&self, world: &mut World, player: i32, plan: i32, delta: i32) {
        let b = &self.bonuses;
        world.cut(CutCall::PlayerChangeBattlePlan {
            player,
            plan,
            delta,
            armor_scalar: b.armor_scalar,
            bombardment: b.bombardment,
            search_and_destroy: b.search_and_destroy,
            hold_the_line: b.hold_the_line,
            sight_range_scalar: b.sight_range_scalar,
        });
    }

    /// The building's body's `setMaxHealth(getMaxHealth() * factor, changeType)` (the body read unchecked).
    fn scale_max_health(world: &mut World, us: ObjectId, scale: impl Fn(f32) -> f32, change_type: i32) -> Result<(), IniError> {
        let body = world.get_body_module(us).ok_or_else(|| IniError::undefined(Undefined::NullBodyModule))?;
        let max = match world.module_call(us, body, ModuleCall::GetMaxHealth) {
            ModuleReply::Real(m) => m,
            _ => 0.0,
        };
        world.module_call(us, body, ModuleCall::SetMaxHealthWithChange { max_health: scale(max), change_type });
        Ok(())
    }

    /// The building's stealth detector's `setSDEnabled(enabled)`, if it has one (`findUpdateModule("StealthDetectorUpdate")`).
    fn set_stealth_detection(world: &mut World, us: ObjectId, enabled: bool) {
        if let Some(sd) = world.find_module(us, b"StealthDetectorUpdate") {
            world.module_call(us, sd, ModuleCall::StealthDetectorSetEnabled(enabled));
        }
    }

    /// `setBattlePlan(plan)`: the old plan's bonuses taken back, the new plan's given (the empty plan paralyses the troops).
    fn set_battle_plan(&mut self, world: &mut World, us: ObjectId, plan: i32) -> Result<(), IniError> {
        let d = self.data.clone();
        let Some(player) = world.get_controlling_player(us) else { return Ok(()) };
        match self.plan_affecting_army {
            PLANSTATUS_BOMBARDMENT => {
                // "Remove the previous plan!" ("The only building bonus is actually the turret, and that's already handled!")
                self.change_battle_plan(world, player, PLANSTATUS_BOMBARDMENT, -1);
            }
            PLANSTATUS_HOLDTHELINE => {
                self.change_battle_plan(world, player, PLANSTATUS_HOLDTHELINE, -1);
                // "Remove building health bonuses"
                let s = d.strategy_center_hold_the_line_max_health_scalar;
                if s != 1.0 {
                    Self::scale_max_health(world, us, |m| m * 1.0 / s, d.strategy_center_hold_the_line_max_health_change_type)?;
                }
            }
            PLANSTATUS_SEARCHANDDESTROY => {
                self.change_battle_plan(world, player, PLANSTATUS_SEARCHANDDESTROY, -1);
                // "Remove sight range bonus"
                let s = d.strategy_center_search_and_destroy_sight_range_scalar;
                if s != 1.0 {
                    let v = world.get_vision_range(us);
                    world.set_vision_range(us, v * 1.0 / s);
                    let r = world.get_shroud_clearing_range(us);
                    world.set_shroud_clearing_range(us, r * 1.0 / s);
                }
                // "Remove stealth detection"
                if d.strategy_center_search_and_destroy_detects_stealth {
                    Self::set_stealth_detection(world, us, false);
                }
            }
            _ => {}
        }
        // "Revert to default no-bonuses!"
        self.bonuses.armor_scalar = 1.0;
        self.bonuses.sight_range_scalar = 1.0;
        self.bonuses.bombardment = 0;
        self.bonuses.search_and_destroy = 0;
        self.bonuses.hold_the_line = 0;
        // "Add new bonuses!"
        match plan {
            PLANSTATUS_NONE => {
                // "Paralyze troops!"
                let until = world.frame().wrapping_add(d.battle_plan_paralyze_frames);
                for obj in world.player_iterate_objects(player) {
                    let Some(o) = world.objects.find(obj) else { continue };
                    if o.is_any_kind_of(&d.valid_member_kind_of) && (!o.is_any_kind_of(&d.invalid_member_kind_of) || crate::logic::modules::plant(2415)) {
                        world.body_set_disabled_until(obj, DISABLED_PARALYZED, until);
                    }
                }
            }
            PLANSTATUS_BOMBARDMENT => {
                self.bonuses.bombardment = 1; // "for weapon bonuses"
                self.change_battle_plan(world, player, PLANSTATUS_BOMBARDMENT, 1);
            }
            PLANSTATUS_HOLDTHELINE => {
                // "Add building health bonuses"
                let s = d.strategy_center_hold_the_line_max_health_scalar;
                if s != 0.0 && !crate::logic::modules::plant(2416) {
                    Self::scale_max_health(world, us, |m| m * s, d.strategy_center_hold_the_line_max_health_change_type)?;
                }
                self.bonuses.armor_scalar = d.hold_the_line_armor_damage_scalar;
                self.bonuses.hold_the_line = 1;
                self.change_battle_plan(world, player, PLANSTATUS_HOLDTHELINE, 1);
            }
            PLANSTATUS_SEARCHANDDESTROY => {
                // "Add sight range bonus"
                let s = d.strategy_center_search_and_destroy_sight_range_scalar;
                if s != 1.0 {
                    let v = world.get_vision_range(us);
                    world.set_vision_range(us, v * s);
                    let r = world.get_shroud_clearing_range(us);
                    world.set_shroud_clearing_range(us, r * s);
                }
                // "Enable stealth detection"
                if d.strategy_center_search_and_destroy_detects_stealth && !crate::logic::modules::plant(2417) {
                    Self::set_stealth_detection(world, us, true);
                }
                self.bonuses.search_and_destroy = 1;
                self.bonuses.sight_range_scalar = d.search_and_destroy_sight_range_scalar;
                self.change_battle_plan(world, player, PLANSTATUS_SEARCHANDDESTROY, 1);
            }
            _ => {}
        }
        self.plan_affecting_army = plan;
        Ok(())
    }

    /// `onDelete()`: the vision object destroyed, and the plan affecting the army taken back.
    fn on_delete(&mut self, world: &mut World, us: ObjectId) {
        if self.vision_object_id != INVALID_ID && world.objects.find(self.vision_object_id).is_some() {
            world.body_destroy_object(self.vision_object_id);
        }
        if let Some(player) = world.get_controlling_player(us) {
            if self.plan_affecting_army != PLANSTATUS_NONE {
                self.change_battle_plan(world, player, self.plan_affecting_army, -1);
            }
        }
    }

    /// `update()`.
    fn run(&mut self, world: &mut World, us: ObjectId) -> Result<(), IniError> {
        if self.invalid_settings {
            return Ok(());
        }
        let now = world.frame();
        if self.next_ready_frame > now && !crate::logic::modules::plant(2418) {
            return Ok(());
        }
        match self.status {
            TRANSITIONSTATUS_IDLE => {
                if self.desired_plan != PLANSTATUS_NONE {
                    self.current_plan = self.desired_plan;
                    self.set_status(world, us, TRANSITIONSTATUS_UNPACKING)?;
                }
            }
            TRANSITIONSTATUS_UNPACKING => {
                self.set_status(world, us, TRANSITIONSTATUS_ACTIVE)?;
                if self.current_plan == PLANSTATUS_BOMBARDMENT {
                    self.enable_turret(world, us, true);
                }
            }
            TRANSITIONSTATUS_ACTIVE => {
                if self.current_plan != self.desired_plan {
                    if self.current_plan == PLANSTATUS_BOMBARDMENT {
                        // "in bombardment status, we need to center the turret prior to packing up"
                        if let Some(ai) = world.objects.find(us).and_then(|o| o.ai) {
                            if self.is_turret_in_natural_position(world, us) {
                                self.set_status(world, us, TRANSITIONSTATUS_PACKING)?;
                                self.centering_turret = false;
                                self.enable_turret(world, us, false);
                            } else if !self.centering_turret {
                                world.module_call(us, ai, ModuleCall::AiIdleFrom { command_source: CMD_FROM_AI });
                                self.recenter_turret(world, us);
                                self.centering_turret = !crate::logic::modules::plant(2419);
                            }
                        }
                    } else {
                        self.set_status(world, us, TRANSITIONSTATUS_PACKING)?;
                    }
                }
            }
            TRANSITIONSTATUS_PACKING => self.set_status(world, us, TRANSITIONSTATUS_IDLE)?,
            _ => {}
        }
        Ok(())
    }
}

impl UpdateBehavior for BattlePlanUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        self.base.update()
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if let Err(e) = self.run(world, me.object) {
            world.cuts.module_errors.push((me, e));
        }
        UPDATE_SLEEP_NONE
    }
    /// The fields after the base: the three plans and the status (`xferUser`, four bytes each), the ready frame, the two flags, the bonuses
    /// (their kinds as `KindOfMaskType::xfer`) and the vision object.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        for v in [&mut self.current_plan, &mut self.desired_plan, &mut self.plan_affecting_army, &mut self.status] {
            let mut b = v.to_le_bytes();
            xfer.xfer_user(&mut b)?;
            *v = i32::from_le_bytes(b);
        }
        xfer.xfer_unsigned_int(&mut self.next_ready_frame)?;
        if crate::logic::modules::plant(2420) {
            return Ok(());
        }
        xfer.xfer_bool(&mut self.invalid_settings)?;
        xfer.xfer_bool(&mut self.centering_turret)?;
        let b = &mut self.bonuses;
        xfer.xfer_real(&mut b.armor_scalar)?;
        xfer.xfer_int(&mut b.bombardment)?;
        xfer.xfer_int(&mut b.search_and_destroy)?;
        xfer.xfer_int(&mut b.hold_the_line)?;
        xfer.xfer_real(&mut b.sight_range_scalar)?;
        b.valid_kind_of.xfer(xfer)?;
        b.invalid_kind_of.xfer(xfer)?;
        xfer.xfer_object_id(&mut self.vision_object_id)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::BattlePlanUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::BattlePlanUpdate(m) => Some(m),
            _ => None,
        }
    }
    /// `onObjectCreated`, `onDelete`, `initiateIntentToDoSpecialPower`, the special power update's queries and `getActiveBattlePlan`.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnObjectCreated => {
                self.on_object_created(world, me.object);
                ModuleReply::Done
            }
            ModuleCall::OnDelete => {
                self.on_delete(world, me.object);
                ModuleReply::Done
            }
            ModuleCall::InitiateIntentToDoSpecialPower { template, command_options, .. } => match self.initiate_intent(world, me.object, template, command_options) {
                Ok(b) => ModuleReply::Bool(b),
                Err(e) => {
                    world.cuts.module_errors.push((me, e));
                    ModuleReply::Bool(false)
                }
            },
            ModuleCall::SpecialPowerUpdateIsActive => ModuleReply::Bool(self.status != TRANSITIONSTATUS_IDLE),
            ModuleCall::SpecialPowerUpdateGetCommandOption => ModuleReply::Int(self.get_command_option() as i32),
            ModuleCall::IsPowerCurrentlyInUse => ModuleReply::Bool(false),
            ModuleCall::BattlePlanGetActiveBattlePlan => ModuleReply::Int(self.get_active_battle_plan()),
            ModuleCall::DoesSpecialPowerHaveOverridableDestinationActive | ModuleCall::DoesSpecialPowerHaveOverridableDestination => ModuleReply::Bool(false),
            ModuleCall::DoesSpecialPowerUpdatePassScienceTest => ModuleReply::Bool(true),
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for BattlePlanUpdate {
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
