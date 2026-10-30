// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/FlightDeckBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/FlightDeckBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `FlightDeckBehavior`: an aircraft carrier's deck. Its `NumSpacesPerRunway` x `NumRunways` parking spaces (sorted runway 1 space 1,
//! runway 2 space 1, runway 1 space 2, ...; each at the bone the data names) and its runways (take-off and landing strips, taxi and
//! creation points, a catapult's steam) are built the first time they are needed, the payload's jets with them. It hands the jets their
//! spaces and runways (`ParkingPlaceBehaviorInterface`: only a front space may take off), heals the jets that ask, keeps the idle jets
//! moving up to the front spaces (every `ParkingCleanupPeriod`, one jet per runway at a time), builds a replacement for a lost jet
//! through its production update, launches the front jets when it has orders (raising the ramp, the catapult's steam, lowering the ramp),
//! passes its orders on to its jets (the carrier itself never moves: "a dumb stump"), and kills or defects the parked jets with it.
//!
//! # Upstream facts kept
//!
//! - `update()` is always awake; it keeps the deck built and the dead purged every frame.
//! - `buildInfo`'s walk to a space's bone name stops on the counter alone (`it != end, counter < row`: the comma drops the end test), so
//!   a row with no bone name reads past the list (undefined: `Undefined::EndIteratorDereferenced`). A runway past the data's two
//!   (`NumRunways` above 2) is read past its array (`Undefined::ArrayIndexOutOfRange`).
//! - The runway built for each column is one `RunwayInfo` reused: a column without creation bones keeps the previous column's start
//!   orientation and transform, and the first column's are then uninitialised (read by an exit or the catapult's steam:
//!   `Undefined::UninitialisedLocal`).
//! - `reserveRunway` for take-off looks only at the first `NumRunways` spaces, unchecked (fewer spaces: `Undefined::ArrayIndexOutOfRange`);
//!   `calcBestParkingAssignment` gives up a found spot when a plane is in the way only if it was asked for the position.
//! - The heal loop and the purge never advance past a healee of no id; none is ever added.
//! - `xfer` moves the command through a local `Int commandType` it never initialises, and assigns the local to the command in a save
//!   as in a load: upstream's save writes an undefined word and leaves the carrier's orders undefined. This translation saves the
//!   command and keeps it (the test gives upstream's local the command, the one value that leaves a save defined).
//! - `xfer` (version 1) saves `m_rampUp[MAX_RUNWAYS]` twice: the byte past the two flags, never the flags themselves ([`Self::ramp_pad`]).
//!
//! # Deviations
//!
//! Upstream's class is an `AIUpdateInterface` (its data an `AIUpdateModuleData`), but replaces the base's `update` and `aiDoCommand`
//! outright. This translation is the update module those leave: its base is `UpdateModule`, so its save holds `UpdateModule`'s members, not
//! the AI's between them and this class's; its data parses `AIUpdateModuleData`'s own fields (into [`FlightDeckBehaviorModuleData::ai`]),
//! which nothing here reads. (The AI updates are not yet modules of the world.)
//! `aiDoCommand` is [`ModuleCall::AiDoCommand`]; the jets' `JetAIUpdate` questions are calls of their AI.
//!
//! Differential-verified in `tests/differential_flight_deck_r82.rs`: 9,000 cases, 307,931 outputs, 0 mismatches; plants 3821-3828
//! caught.
//!
//! `p1w13-sonnet`, batch R82, 2026-10-30.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::model_state::{MODELCONDITION_DOOR_2_CLOSING, MODELCONDITION_DOOR_2_OPENING};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DIE};
use crate::common::object_status_types::{
    ObjectStatusMaskType, OBJECT_STATUS_AIRBORNE_TARGET, OBJECT_STATUS_DECK_HEIGHT_OFFSET, OBJECT_STATUS_NO_ATTACK, OBJECT_STATUS_REASSIGN_PARKING,
    OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_HEALING, DAMAGE_UNRESISTABLE, DEATH_NONE, DEATH_NORMAL};
use crate::logic::ai::ai_commands::{
    AICMD_ATTACKMOVE_TO_POSITION, AICMD_ATTACK_OBJECT, AICMD_ATTACK_POSITION, AICMD_ENTER, AICMD_FORCE_ATTACK_OBJECT, AICMD_GUARD_POSITION, AICMD_IDLE,
    AICMD_NO_COMMAND,
};
use crate::logic::ai::ai_update_data::{self, AiUpdateModuleData};
use crate::logic::ai::ai_update_types::{CMD_FROM_AI, CMD_FROM_PLAYER, GUARDMODE_NORMAL};
use crate::logic::weapon::NO_MAX_SHOTS_LIMIT;
use crate::logic::module::update_module::{ExitDoorType, SleepyUpdatePhase, UpdateModule, UpdateSleepTime, DOOR_1, DOOR_NONE_NEEDED, UPDATE_SLEEP_NONE};
use crate::logic::modules::plant;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::TeamId;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply, PPInfo};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::ww::matrix3d::Matrix3D;

/// `MAX_RUNWAYS`.
pub const MAX_RUNWAYS: usize = 2;
/// `FOREVER`.
const FOREVER: u32 = 0x3fff_ffff;
/// `HEAL_RATE_FRAMES`.
const HEAL_RATE_FRAMES: u32 = (LOGICFRAMES_PER_SECOND / 5) as u32;
/// `SECONDS_PER_LOGICFRAME_REAL`.
const SECONDS_PER_LOGICFRAME_REAL: f32 = 1.0 / LOGICFRAMES_PER_SECOND as f32;
/// `NUM_MODELCONDITION_DOOR_STATES`.
const NUM_MODELCONDITION_DOOR_STATES: i32 = 4;
/// `FROM_CENTER_2D`.
const FROM_CENTER_2D: crate::logic::world::partition::DistanceCalculationType = crate::logic::world::partition::DistanceCalculationType::FromCenter2D;

/// `struct RunwayDefinition`: a runway's bones (`RUNWAY_START_BONE`, `RUNWAY_END_BONE`) and its catapult's particle system (by name).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunwayDefinition {
    pub spaces_bone_names: Vec<Vec<u8>>,
    pub taxi_bone_names: Vec<Vec<u8>>,
    pub creation_bone_names: Vec<Vec<u8>>,
    pub takeoff_bone_names: [Vec<u8>; 2],
    pub landing_bone_names: [Vec<u8>; 2],
    pub catapult_particle_system: Option<Vec<u8>>,
}

/// `class FlightDeckBehaviorModuleData : public AIUpdateModuleData` (this class's fields: see the module's deviations).
#[derive(Clone, Debug, Default)]
pub struct FlightDeckBehaviorModuleData {
    /// The `AIUpdateModuleData` base (parsed; the module reads its `UpdateModuleData` only).
    pub ai: AiUpdateModuleData,
    pub runway_info: [RunwayDefinition; MAX_RUNWAYS],
    pub thing_template_name: Vec<u8>,
    pub heal_amount: f32,
    pub approach_height: f32,
    pub landing_deck_height_offset: f32,
    pub num_rows: i32,
    pub num_cols: i32,
    pub cleanup_frames: u32,
    pub human_follow_frames: u32,
    pub replacement_frames: u32,
    pub dock_animation_frames: u32,
    pub launch_wave_frames: u32,
    pub launch_ramp_frames: u32,
    pub lower_ramp_frames: u32,
    pub catapult_fire_frames: u32,
}

/// `parseRunwayStrip`: up to two tokens (none: nothing), the start's then the end's (`AsciiString::format(token)`: the token as written).
fn parse_runway_strip(ini: &mut Ini<'_>, store: &mut [Vec<u8>; 2]) -> IniResult<()> {
    if let Some(t) = ini.get_next_token_or_null(None) {
        store[0] = t;
        if let Some(t) = ini.get_next_token_or_null(None) {
            store[1] = t;
        }
    }
    Ok(())
}

/// `INI::parseParticleSystemTemplate`: the name (`None`: `NULL`), looked up when used.
fn parse_particle_system(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let token = ini.get_next_token(None)?;
    Ok((!token.eq_ignore_ascii_case(b"None")).then_some(token))
}

macro_rules! runway_fields {
    ($k:expr, $spaces:expr, $takeoff:expr, $landing:expr, $taxi:expr, $creation:expr, $catapult:expr) => {
        [
            FieldParse::new($spaces, |i, d: &mut FlightDeckBehaviorModuleData, _| i.parse_ascii_string_vector(&mut d.runway_info[$k].spaces_bone_names), UserData::None),
            FieldParse::new($takeoff, |i, d: &mut FlightDeckBehaviorModuleData, _| parse_runway_strip(i, &mut d.runway_info[$k].takeoff_bone_names), UserData::None),
            FieldParse::new($landing, |i, d: &mut FlightDeckBehaviorModuleData, _| parse_runway_strip(i, &mut d.runway_info[$k].landing_bone_names), UserData::None),
            FieldParse::new($taxi, |i, d: &mut FlightDeckBehaviorModuleData, _| i.parse_ascii_string_vector(&mut d.runway_info[$k].taxi_bone_names), UserData::None),
            FieldParse::new($creation, |i, d: &mut FlightDeckBehaviorModuleData, _| i.parse_ascii_string_vector(&mut d.runway_info[$k].creation_bone_names), UserData::None),
            FieldParse::new(
                $catapult,
                |i, d: &mut FlightDeckBehaviorModuleData, _| {
                    d.runway_info[$k].catapult_particle_system = parse_particle_system(i)?;
                    Ok(())
                },
                UserData::None,
            ),
        ]
    };
}

static RUNWAY1: [FieldParse<FlightDeckBehaviorModuleData>; 6] =
    runway_fields!(0, "Runway1Spaces", "Runway1Takeoff", "Runway1Landing", "Runway1Taxi", "Runway1Creation", "Runway1CatapultSystem");
static RUNWAY2: [FieldParse<FlightDeckBehaviorModuleData>; 6] =
    runway_fields!(1, "Runway2Spaces", "Runway2Takeoff", "Runway2Landing", "Runway2Taxi", "Runway2Creation", "Runway2CatapultSystem");

/// `AIUpdateModuleData::buildFieldParse`'s own entries, forwarded to the base.
fn ai_field(ini: &mut Ini<'_>, d: &mut FlightDeckBehaviorModuleData, k: usize, u: &UserData) -> IniResult<()> {
    match ai_update_data::OWN_FIELD_PARSE[k].parse {
        Some(f) => f(ini, &mut d.ai, u),
        None => Ok(()),
    }
}

static AI_FIELDS: [FieldParse<FlightDeckBehaviorModuleData>; 7] = [
    FieldParse::new("Turret", |i, d, u| ai_field(i, d, 0, u), UserData::None),
    FieldParse::new("AltTurret", |i, d, u| ai_field(i, d, 1, u), UserData::None),
    FieldParse::new("AutoAcquireEnemiesWhenIdle", |i, d, u| ai_field(i, d, 2, u), UserData::None),
    FieldParse::new("MoodAttackCheckRate", |i, d, u| ai_field(i, d, 3, u), UserData::None),
    FieldParse::new("ForbidPlayerCommands", |i, d, u| ai_field(i, d, 4, u), UserData::None),
    FieldParse::new("TurretsLinked", |i, d, u| ai_field(i, d, 5, u), UserData::None),
    FieldParse::end(),
];

/// The rest of `buildFieldParse`'s table.
static FIELD_PARSE: [FieldParse<FlightDeckBehaviorModuleData>; 15] = [
    FieldParse::new("NumRunways", |i, d, _| { d.num_cols = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("NumSpacesPerRunway", |i, d, _| { d.num_rows = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ApproachHeight", |i, d, _| { d.approach_height = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("LandingDeckHeightOffset", |i, d, _| { d.landing_deck_height_offset = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("HealAmountPerSecond", |i, d, _| { d.heal_amount = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("ParkingCleanupPeriod", |i, d, _| { d.cleanup_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("HumanFollowPeriod", |i, d, _| { d.human_follow_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("PayloadTemplate", |i, d, _| { d.thing_template_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("ReplacementDelay", |i, d, _| { d.replacement_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("DockAnimationDelay", |i, d, _| { d.dock_animation_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("LaunchWaveDelay", |i, d, _| { d.launch_wave_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("LaunchRampDelay", |i, d, _| { d.launch_ramp_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("LowerRampDelay", |i, d, _| { d.lower_ramp_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("CatapultFireDelay", |i, d, _| { d.catapult_fire_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl FlightDeckBehaviorModuleData {
    /// `buildFieldParse(p)`: the base's (`AIUpdateModuleData`'s), then this class's (one table upstream; the runways' entries before
    /// the rest).
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, FlightDeckBehaviorModuleData>) -> IniResult<()> {
        crate::common::module::ModuleData::build_field_parse(p)?;
        p.add(&AI_FIELDS)?;
        p.add(&RUNWAY1[..])?;
        p.add(&RUNWAY2[..])?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<FlightDeckBehaviorModuleData> {
        let mut data = FlightDeckBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, FlightDeckBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::FlightDeckBehavior(Arc::new(FlightDeckBehaviorModuleData::friend_new_module_data(ini)?)))
}

/// `struct FlightDeckInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct FlightDeckInfo {
    prep: Coord3D,
    orientation: f32,
    runway: i32,
    object_in_space: ObjectId,
}

/// `struct RunwayInfo` (`m_startOrient` and `m_startTransform` `None` while uninitialised).
#[derive(Clone, Debug, Default, PartialEq)]
struct RunwayInfo {
    start: Coord3D,
    start_transform: Option<Matrix3D>,
    end: Coord3D,
    landing_start: Coord3D,
    landing_end: Coord3D,
    taxi: Vec<Coord3D>,
    creation: Vec<Coord3D>,
    start_orient: Option<f32>,
    in_use_by_for_takeoff: ObjectId,
    in_use_by_for_landing: ObjectId,
}

/// `struct HealingInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct HealingInfo {
    getting_healed_id: ObjectId,
    heal_start_frame: u32,
}

/// What `buildInfo` reads off the bones (the jets are made after, in space order).
#[derive(Clone, Debug)]
struct Build {
    spaces: Vec<FlightDeckInfo>,
    runways: Vec<RunwayInfo>,
    /// Where it stopped: `reserve` threw (`std::length_error`) or upstream is undefined.
    error: Option<IniError>,
    /// The spaces made before it stopped (the runways are built after all spaces).
    complete: bool,
}

/// `class FlightDeckBehavior : public AIUpdateInterface, public ParkingPlaceBehaviorInterface, public DieModuleInterface, public ExitInterface`.
#[derive(Clone, Debug)]
pub struct FlightDeckBehavior {
    base: UpdateModule,
    data: Arc<FlightDeckBehaviorModuleData>,
    /// `m_thingTemplate` (by id).
    thing_template: Option<u16>,
    spaces: Vec<FlightDeckInfo>,
    runways: Vec<RunwayInfo>,
    healing: Vec<HealingInfo>,
    next_heal_frame: u32,
    next_cleanup_frame: u32,
    started_production_frame: u32,
    next_allowed_production_frame: u32,
    next_launch_wave_frame: [u32; MAX_RUNWAYS],
    ramp_up_frame: [u32; MAX_RUNWAYS],
    catapult_system_frame: [u32; MAX_RUNWAYS],
    lower_ramp_frame: [u32; MAX_RUNWAYS],
    designated_target: ObjectId,
    designated_command: i32,
    designated_position: Coord3D,
    got_info: bool,
    ramp_up: [bool; MAX_RUNWAYS],
    /// `m_rampUp[MAX_RUNWAYS]`: the byte after the flags (the class's padding, never initialised upstream: the test zeroes it), which
    /// `xfer` saves and loads as a `Bool`.
    ramp_pad: bool,
    /// The load's `buildInfo(FALSE)`, staged by the world before the xfer (it reads the bones), applied after the version and the base.
    staged_build: Option<Build>,
    staged_error: Option<IniError>,
}

impl PartialEq for FlightDeckBehavior {
    fn eq(&self, other: &Self) -> bool {
        self.base.behavior().object() == other.base.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

fn mask(bit: crate::common::object_status_types::ObjectStatusTypes) -> ObjectStatusMaskType {
    let mut m = ObjectStatusMaskType::default();
    m.set(bit, 1);
    m
}

impl FlightDeckBehavior {
    /// `getInterfaceMask()`: `UpdateModule`'s and the die interface.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE
    }

    /// The constructor's first half; the second wakes the update now.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<FlightDeckBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = UpdateModule::new(object, Some(Arc::new(data.ai.update.clone())), SleepyUpdatePhase::Normal)?;
        Ok(FlightDeckBehavior {
            base,
            data,
            thing_template: None,
            spaces: Vec::new(),
            runways: Vec::new(),
            healing: Vec::new(),
            next_heal_frame: FOREVER,
            next_cleanup_frame: 0,
            started_production_frame: FOREVER,
            next_allowed_production_frame: 0,
            next_launch_wave_frame: [0; MAX_RUNWAYS],
            ramp_up_frame: [0; MAX_RUNWAYS],
            catapult_system_frame: [0; MAX_RUNWAYS],
            lower_ramp_frame: [0; MAX_RUNWAYS],
            designated_target: INVALID_ID,
            designated_command: AICMD_NO_COMMAND,
            designated_position: Coord3D::default(),
            got_info: false,
            ramp_up: [false; MAX_RUNWAYS],
            ramp_pad: false,
            staged_build: None,
            staged_error: None,
        })
    }

    pub fn update_module(&self) -> &UpdateModule {
        &self.base
    }

    /// The members, then each space, runway and healee.
    pub fn state_words(&self) -> Vec<u32> {
        let b = |x: f32| x.to_bits();
        let c = |p: Coord3D| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()];
        let mut v = vec![
            self.thing_template.map_or(0xFFFF_FFFF, u32::from),
            self.next_heal_frame,
            self.next_cleanup_frame,
            self.started_production_frame,
            self.next_allowed_production_frame,
        ];
        for i in 0..MAX_RUNWAYS {
            v.extend([self.next_launch_wave_frame[i], self.ramp_up_frame[i], self.catapult_system_frame[i], self.lower_ramp_frame[i], u32::from(self.ramp_up[i])]);
        }
        v.extend([self.designated_target as u32, self.designated_command as u32]);
        v.extend(c(self.designated_position));
        v.extend([u32::from(self.got_info), u32::from(self.ramp_pad)]);
        v.push(self.spaces.len() as u32);
        for s in &self.spaces {
            v.extend(c(s.prep));
            v.extend([b(s.orientation), s.runway as u32, s.object_in_space as u32]);
        }
        v.push(self.runways.len() as u32);
        for r in &self.runways {
            v.extend(c(r.start));
            v.extend(c(r.end));
            v.extend(c(r.landing_start));
            v.extend(c(r.landing_end));
            v.push(r.start_orient.map_or(0xFFFF_FFFF, b));
            match &r.start_transform {
                Some(m) => {
                    for row in 0..3 {
                        v.extend([b(m[row].x), b(m[row].y), b(m[row].z), b(m[row].w)]);
                    }
                }
                None => v.push(0xFFFF_FFFF),
            }
            v.push(r.taxi.len() as u32);
            for p in &r.taxi {
                v.extend(c(*p));
            }
            v.push(r.creation.len() as u32);
            for p in &r.creation {
                v.extend(c(*p));
            }
            v.extend([r.in_use_by_for_takeoff as u32, r.in_use_by_for_landing as u32]);
        }
        v.push(self.healing.len() as u32);
        for h in &self.healing {
            v.extend([h.getting_healed_id as u32, h.heal_start_frame]);
        }
        v
    }

    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half: `setWakeFrame(getObject(), UPDATE_SLEEP_NONE)`.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
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

    /// The bone's position and transform (`getSingleLogicalBonePosition(name, &pos, &mtx)`).
    fn bone(world: &World, us: ObjectId, name: &[u8]) -> (Coord3D, Matrix3D) {
        let (_, pos, mtx) = world.object_get_single_logical_bone_transform(us, name);
        (pos, mtx)
    }

    /// The spaces and runways `buildInfo` reads (`None`: built already, or the carrier unfinished or sold).
    fn make_build(&self, world: &World, us: ObjectId) -> Option<Build> {
        if self.got_info {
            return None;
        }
        let o = world.objects.find(us)?;
        if o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION) || o.test_status(OBJECT_STATUS_SOLD) {
            return None;
        }
        let d = &self.data;
        let mut b = Build { spaces: Vec::new(), runways: Vec::new(), error: None, complete: false };
        // `m_spaces.reserve(d->m_numRows * d->m_numCols)`
        if d.num_rows.wrapping_mul(d.num_cols) < 0 {
            b.error = Some(IniError::new(IniErrorKind::LengthError));
            return Some(b);
        }
        // "We want to sort the spaces so that we have runway 1 space 1, runway 2 space 1, R1S2, R2S2, R1S3..."
        for row in 0..d.num_rows {
            for col in 0..d.num_cols {
                let Some(def) = d.runway_info.get(col as usize) else {
                    b.error = Some(IniError::undefined(Undefined::ArrayIndexOutOfRange));
                    return Some(b);
                };
                // (the walk stops on the counter alone)
                let Some(name) = def.spaces_bone_names.get(row as usize) else {
                    b.error = Some(IniError::undefined(Undefined::EndIteratorDereferenced));
                    return Some(b);
                };
                let (prep, mtx) = Self::bone(world, us, name);
                let runway = if plant(3821) { 0 } else { col };
                b.spaces.push(FlightDeckInfo { prep, orientation: mtx.get_z_rotation(), runway, object_in_space: INVALID_ID });
            }
        }
        // `m_runways.reserve(data->m_numCols)`
        if d.num_cols < 0 {
            b.error = Some(IniError::new(IniErrorKind::LengthError));
            b.complete = false;
            return Some(b);
        }
        // (one `RunwayInfo` for every column: what a column does not set is the previous one's)
        let mut info = RunwayInfo { in_use_by_for_takeoff: INVALID_ID, in_use_by_for_landing: INVALID_ID, ..Default::default() };
        for col in 0..d.num_cols {
            let Some(def) = d.runway_info.get(col as usize) else {
                b.error = Some(IniError::undefined(Undefined::ArrayIndexOutOfRange));
                return Some(b);
            };
            info.start = Self::bone(world, us, &def.takeoff_bone_names[0]).0;
            info.end = Self::bone(world, us, &def.takeoff_bone_names[1]).0;
            info.landing_start = Self::bone(world, us, &def.landing_bone_names[0]).0;
            info.landing_end = Self::bone(world, us, &def.landing_bone_names[1]).0;
            info.in_use_by_for_takeoff = INVALID_ID;
            info.in_use_by_for_landing = INVALID_ID;
            info.taxi = def.taxi_bone_names.iter().map(|n| Self::bone(world, us, n).0).collect();
            info.creation.clear();
            let mut first_time = true;
            for n in &def.creation_bone_names {
                let (pos, mtx) = Self::bone(world, us, n);
                if first_time {
                    first_time = false;
                    info.start_orient = Some(mtx.get_z_rotation());
                    info.start_transform = Some(mtx);
                }
                info.creation.push(pos);
            }
            b.runways.push(info.clone());
        }
        b.complete = true;
        Some(b)
    }

    /// Keep a build as far as it got (the spaces, then the runways once all spaces are made).
    fn apply_build(&mut self, b: Build) -> Result<(), IniError> {
        self.spaces.extend(b.spaces);
        self.runways.extend(b.runways);
        if b.complete {
            self.got_info = true;
        }
        match b.error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// `buildInfo(createUnits)`: the template found (every time), then the deck (once, once the carrier is finished), the payload's jets
    /// made in its spaces.
    fn build_info(&mut self, world: &mut World, us: ObjectId, create_units: bool) -> Result<(), IniError> {
        if self.got_info {
            return Ok(());
        }
        self.thing_template = world.thing_factory_find_template(&self.data.thing_template_name);
        let Some(b) = self.make_build(world, us) else { return Ok(()) };
        let first_new = self.spaces.len();
        let made = b.spaces.len();
        let r = self.apply_build(b);
        if let (Some(template), true) = (self.thing_template, create_units) {
            for i in first_new..first_new + made {
                let Some(player) = world.get_controlling_player(us) else { return Err(IniError::undefined(Undefined::NullControllingPlayer)) };
                let team = world.cuts.player_default_team(player);
                let Ok(jet) = world.thing_factory_new_object(template, Some(team).filter(|t| *t != 0), Default::default()) else { continue };
                world.set_producer(jet, us);
                world.body_set_status(jet, mask(OBJECT_STATUS_DECK_HEIGHT_OFFSET), true);
                let s = self.spaces[i];
                world.set_position(jet, &s.prep);
                world.set_orientation(jet, s.orientation);
                self.spaces[i].object_in_space = jet;
            }
        }
        r
    }

    /// The world's load, before the xfer: what `buildInfo(FALSE)` would read.
    pub fn stage_build_info(&mut self, world: &mut World, us: ObjectId) {
        self.staged_error = None;
        if !self.got_info {
            self.thing_template = world.thing_factory_find_template(&self.data.thing_template_name);
        }
        self.staged_build = self.make_build(world, us);
    }

    /// The world's load, after the xfer: what the staged build threw.
    pub fn finish_staged_build(&mut self) -> Option<IniError> {
        self.staged_build = None;
        self.staged_error.take()
    }

    fn dead_or_gone(world: &World, id: ObjectId) -> bool {
        world.objects.find(id).is_none_or(|o| o.is_effectively_dead())
    }

    /// `purgeDead()`.
    fn purge_dead(&mut self, world: &mut World, us: ObjectId) -> Result<(), IniError> {
        self.build_info(world, us, true)?;
        for s in self.spaces.iter_mut() {
            if s.object_in_space != INVALID_ID && Self::dead_or_gone(world, s.object_in_space) {
                s.object_in_space = INVALID_ID;
            }
        }
        for r in self.runways.iter_mut() {
            if r.in_use_by_for_takeoff != INVALID_ID && Self::dead_or_gone(world, r.in_use_by_for_takeoff) {
                r.in_use_by_for_takeoff = INVALID_ID;
            }
            if r.in_use_by_for_landing != INVALID_ID && Self::dead_or_gone(world, r.in_use_by_for_landing) && !plant(3822) {
                r.in_use_by_for_landing = INVALID_ID;
            }
        }
        let before = self.healing.len();
        self.healing.retain(|h| h.getting_healed_id == INVALID_ID || !Self::dead_or_gone(world, h.getting_healed_id));
        if self.healing.len() != before {
            self.reset_wake_frame(world);
        }
        Ok(())
    }

    fn has_reserved_space(&self, id: ObjectId) -> bool {
        if !self.got_info || id == INVALID_ID {
            return false;
        }
        self.spaces.iter().any(|s| s.object_in_space == id)
    }

    fn get_space_index(&self, id: ObjectId) -> i32 {
        if id == INVALID_ID {
            return -1;
        }
        self.spaces.iter().position(|s| s.object_in_space == id).map_or(-1, |i| i as i32)
    }

    fn find_ppi(&self, id: ObjectId) -> Option<usize> {
        if !self.got_info || id == INVALID_ID {
            return None;
        }
        self.spaces.iter().position(|s| s.object_in_space == id)
    }

    fn find_empty_ppi(&self) -> Option<usize> {
        if !self.got_info {
            return None;
        }
        self.spaces.iter().position(|s| s.object_in_space == INVALID_ID)
    }

    /// `hasAvailableSpaceFor(thing)` ("just peek and see if we have a dead thing").
    fn has_available_space_for(&self, world: &World) -> bool {
        if !self.got_info {
            return false;
        }
        self.spaces.iter().any(|s| s.object_in_space == INVALID_ID || Self::dead_or_gone(world, s.object_in_space))
    }

    /// `reserveSpace(id, parkingOffset, info)`.
    fn reserve_space(&mut self, world: &mut World, us: ObjectId, id: ObjectId, parking_offset: f32, want_info: bool) -> Result<(bool, Option<PPInfo>), IniError> {
        self.build_info(world, us, true)?;
        self.purge_dead(world, us)?;
        let Some(i) = self.find_ppi(id).or_else(|| self.find_empty_ppi()) else {
            return Ok((false, None)); // "No parking places!"
        };
        self.spaces[i].object_in_space = id;
        if self.data.landing_deck_height_offset != 0.0 && world.objects.find(id).is_some() {
            world.body_set_status(id, mask(OBJECT_STATUS_DECK_HEIGHT_OFFSET), true);
        }
        let mut out = None;
        if want_info {
            let mut info = self.calc_pp_info(id, PPInfo::default())?;
            if parking_offset != 0.0 {
                let orientation = self.spaces[i].orientation;
                info.parking_space.x += parking_offset * crate::common::trig::cos(orientation);
                info.parking_space.y += parking_offset * crate::common::trig::sin(orientation);
            }
            out = Some(info);
        }
        Ok((true, out))
    }

    /// `calcPPInfo(id, info)` (the space not found: "Utter failure", `info` as it was).
    fn calc_pp_info(&self, id: ObjectId, mut info: PPInfo) -> Result<PPInfo, IniError> {
        let Some(i) = self.find_ppi(id) else { return Ok(info) };
        let ppi = self.spaces[i];
        let rr = self.runways.get(ppi.runway as usize).ok_or_else(|| IniError::undefined(Undefined::ArrayIndexOutOfRange))?;
        let d = &self.data;
        let approach_dist = if plant(3823) { 0.5f32 } else { 0.75f32 };
        info.parking_space = ppi.prep;
        info.runway_prep = ppi.prep;
        info.parking_orientation = ppi.orientation;
        info.runway_start = rr.start;
        info.runway_end = rr.end;
        info.runway_exit = rr.end;
        info.runway_exit.x += (rr.end.x - rr.start.x) * approach_dist;
        info.runway_exit.y += (rr.end.y - rr.start.y) * approach_dist;
        info.runway_exit.z = rr.end.z + d.approach_height + d.landing_deck_height_offset;
        info.runway_landing_start = rr.landing_start;
        info.runway_landing_end = rr.landing_end;
        info.runway_approach = rr.landing_start;
        info.runway_approach.x += (rr.landing_start.x - rr.landing_end.x) * approach_dist;
        info.runway_approach.y += (rr.landing_start.y - rr.landing_end.y) * approach_dist;
        info.runway_approach.z = rr.landing_start.z + d.approach_height + d.landing_deck_height_offset;
        // "Cache the runway's takeoff distance used by JetAIUpdate for calculating lift."
        let mut vector = info.runway_start;
        vector.sub(&info.runway_end);
        info.runway_takeoff_dist = vector.length();
        for r in &self.runways {
            if r.in_use_by_for_takeoff == id {
                info.runway_start = info.runway_prep;
            }
        }
        Ok(info)
    }

    /// `releaseSpace(id)`.
    fn release_space(&mut self, world: &mut World, us: ObjectId, id: ObjectId) -> Result<(), IniError> {
        self.build_info(world, us, true)?;
        self.purge_dead(world, us)?;
        for s in self.spaces.iter_mut() {
            if s.object_in_space == id {
                s.object_in_space = INVALID_ID;
            }
        }
        if world.objects.find(id).is_some() {
            world.body_set_status(id, mask(OBJECT_STATUS_DECK_HEIGHT_OFFSET), false);
        }
        Ok(())
    }

    /// `getRunwayReservation(runway, type)`.
    fn get_runway_reservation(&mut self, world: &mut World, us: ObjectId, runway: i32, landing: Option<bool>) -> Result<ObjectId, IniError> {
        self.build_info(world, us, true)?;
        self.purge_dead(world, us)?;
        let Some(landing) = landing else { return Ok(INVALID_ID) };
        let r = usize::try_from(runway).ok().and_then(|r| self.runways.get(r)).ok_or_else(|| IniError::undefined(Undefined::ArrayIndexOutOfRange))?;
        Ok(if landing { r.in_use_by_for_landing } else { r.in_use_by_for_takeoff })
    }

    /// `reserveRunway(id, forLanding)`: take-off only from a front space.
    fn reserve_runway(&mut self, world: &mut World, us: ObjectId, id: ObjectId, for_landing: bool) -> Result<bool, IniError> {
        self.build_info(world, us, true)?;
        self.purge_dead(world, us)?;
        let mut runway = -1;
        if !for_landing {
            // "Only look at the front spaces for takeoff. You can't take off from the back!"
            for i in 0..self.data.num_cols.max(0) as usize {
                let s = self.spaces.get(i).ok_or_else(|| IniError::undefined(Undefined::ArrayIndexOutOfRange))?;
                if s.object_in_space == id {
                    runway = s.runway;
                    break;
                }
            }
        } else if let Some(s) = self.spaces.iter().find(|s| s.object_in_space == id) {
            runway = s.runway;
        }
        if runway == -1 {
            return Ok(false);
        }
        let info = self.runways.get_mut(runway as usize).ok_or_else(|| IniError::undefined(Undefined::ArrayIndexOutOfRange))?;
        if info.in_use_by_for_takeoff == id && !for_landing || info.in_use_by_for_landing == id && for_landing {
            Ok(true)
        } else if info.in_use_by_for_takeoff == INVALID_ID && !for_landing || info.in_use_by_for_landing == INVALID_ID && for_landing {
            if for_landing {
                info.in_use_by_for_landing = id;
            } else {
                info.in_use_by_for_takeoff = id;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// `releaseRunway(id)`.
    fn release_runway(&mut self, world: &mut World, us: ObjectId, id: ObjectId) -> Result<(), IniError> {
        self.build_info(world, us, true)?;
        self.purge_dead(world, us)?;
        for r in self.runways.iter_mut() {
            if r.in_use_by_for_takeoff == id {
                r.in_use_by_for_takeoff = INVALID_ID;
            }
            if r.in_use_by_for_landing == id {
                r.in_use_by_for_landing = INVALID_ID;
            }
        }
        Ok(())
    }

    /// The runway of the space the object has (`None`: "only planes with reserved spaces can reserve runways").
    fn runway_of(&self, id: ObjectId) -> Option<i32> {
        self.spaces.iter().find(|s| s.object_in_space == id).map(|s| s.runway)
    }

    /// Space `i`'s place (`m_spaces[i].m_prep`), `None` past the spaces.
    pub fn space_prep(&self, i: usize) -> Option<Coord3D> {
        self.spaces.get(i).map(|s| s.prep)
    }

    /// `getTaxiLocations(id)`.
    pub fn get_taxi_locations(&self, id: ObjectId) -> Option<Vec<Coord3D>> {
        let r = self.runway_of(id)?;
        self.runways.get(r as usize).map(|r| r.taxi.clone())
    }

    /// `getCreationLocations(id)`.
    pub fn get_creation_locations(&self, id: ObjectId) -> Option<Vec<Coord3D>> {
        let r = self.runway_of(id)?;
        self.runways.get(r as usize).map(|r| r.creation.clone())
    }

    fn jet_ai(world: &World, jet: ObjectId) -> Option<u16> {
        world.objects.find(jet).and_then(|o| o.ai)
    }

    fn airborne(world: &World, jet: ObjectId) -> bool {
        world.objects.find(jet).is_some_and(|o| o.status.test(OBJECT_STATUS_AIRBORNE_TARGET))
    }

    /// `isAbleToGiveUpParkingSpace(jet)`.
    fn is_able_to_give_up_parking_space(&self, world: &mut World, jet: ObjectId) -> bool {
        // "If we're airborne or non-existant, someone else can have my spot if they need it."
        if world.objects.find(jet).is_none() || Self::airborne(world, jet) {
            return true;
        }
        if let Some(ai) = Self::jet_ai(world, jet) {
            // "If I'm not idle, I'm certainly not parked. But I could be on route to park in my space!"
            if !world.module_call(jet, ai, ModuleCall::AiIsIdle).as_bool() && !world.module_call(jet, ai, ModuleCall::JetIsTaxiingToParking).as_bool() {
                let command = world.module_call(jet, ai, ModuleCall::JetGetPendingCommandType).as_int(AICMD_NO_COMMAND);
                if command == AICMD_ENTER || self.designated_command == AICMD_IDLE {
                    world.module_call(jet, ai, ModuleCall::JetPurgePendingCommand);
                    return false;
                }
                // "But make sure we're not waiting to takeoff!"
                if self.runways.iter().any(|r| r.in_use_by_for_takeoff == jet) && !plant(3824) {
                    return false;
                }
                return true;
            }
        }
        false
    }

    /// `isInPositionToTakeoff(jet)`: in a front space, within `sqrt(10)` of it.
    fn is_in_position_to_takeoff(&self, world: &World, jet: ObjectId) -> Result<bool, IniError> {
        if Self::jet_ai(world, jet).is_none() {
            return Ok(false);
        }
        for i in 0..self.data.num_cols.max(0) as usize {
            let s = self.spaces.get(i).ok_or_else(|| IniError::undefined(Undefined::ArrayIndexOutOfRange))?;
            if s.object_in_space == jet {
                let distance_sqr = world.partition_distance_squared_to_pos(jet, &s.prep, FROM_CENTER_2D);
                return Ok(distance_sqr < 10.0);
            }
        }
        Ok(false)
    }

    /// `isAbleToMoveForward(jet)`: on the deck, idle or reloading.
    fn is_able_to_move_forward(world: &mut World, jet: ObjectId) -> bool {
        let Some(ai) = Self::jet_ai(world, jet) else { return false };
        if Self::airborne(world, jet) {
            return false;
        }
        world.module_call(jet, ai, ModuleCall::AiIsIdle).as_bool() || world.module_call(jet, ai, ModuleCall::JetIsReloading).as_bool()
    }

    /// `calcBestParkingAssignment(id, pos, oldIndex, newIndex)`: whether the jet moved up; its position (if asked), the two indices
    /// (`newIndex` written only when it moved). A jet with no space asked for its position, with a plane in the way of a free space,
    /// reads its space past the end (`Undefined::EndIteratorDereferenced`).
    pub fn calc_best_parking_assignment(&mut self, world: &mut World, id: ObjectId, want_pos: bool) -> Result<(bool, Option<Coord3D>, i32, Option<i32>), IniError> {
        let my_index = self.spaces.iter().position(|s| s.object_in_space == id);
        let mut pos = None;
        let runway = match my_index {
            Some(i) => {
                if want_pos {
                    pos = Some(self.spaces[i].prep);
                }
                self.spaces[i].runway
            }
            None => -1,
        };
        let old_index = my_index.unwrap_or(self.spaces.len()) as i32;
        let mut check_for_plane_in_way = false;
        let mut best: Option<(usize, ObjectId)> = None;
        for that in 0..self.spaces.len() {
            let occupant = self.spaces[that].object_in_space;
            let non_idle_jet = world.objects.find(occupant).map(|_| occupant);
            if Some(that) == my_index {
                // "Done, don't look at my spot, nor spots behind me."
                if let Some((best_index, best_jet)) = best {
                    let mi = that;
                    self.spaces[mi].object_in_space = best_jet;
                    self.spaces[best_index].object_in_space = id;
                    return Ok((true, pos, old_index, Some(best_index as i32)));
                }
                return Ok((false, pos, old_index, None));
            }
            if self.spaces[that].runway == runway {
                if non_idle_jet.is_none_or(|j| self.is_able_to_give_up_parking_space(world, j)) {
                    if !check_for_plane_in_way {
                        best = Some((that, non_idle_jet.unwrap_or(INVALID_ID)));
                        check_for_plane_in_way = true;
                        if want_pos {
                            pos = Some(self.spaces[that].prep);
                        }
                    }
                } else if check_for_plane_in_way {
                    // "Ugh, there's a plane parked between us and the best spot!"
                    check_for_plane_in_way = false;
                    if want_pos && !plant(3825) {
                        let i = my_index.ok_or_else(|| IniError::undefined(Undefined::EndIteratorDereferenced))?;
                        pos = Some(self.spaces[i].prep); // "reset the original position."
                        best = None;
                    }
                }
            }
        }
        Ok((false, pos, old_index, None))
    }

    fn reset_wake_frame(&mut self, world: &World) {
        self.next_heal_frame = if self.healing.is_empty() { FOREVER } else { world.frame().wrapping_add(HEAL_RATE_FRAMES) };
    }

    /// `setHealee(healee, add)`.
    fn set_healee(&mut self, world: &World, healee: ObjectId, add: bool) {
        if add {
            if self.healing.iter().any(|h| h.getting_healed_id == healee) {
                return;
            }
            self.healing.push(HealingInfo { getting_healed_id: healee, heal_start_frame: world.frame() });
            self.reset_wake_frame(world);
        } else {
            let mut i = 0;
            while i < self.healing.len() {
                if self.healing[i].getting_healed_id == healee {
                    self.healing.remove(i);
                    self.reset_wake_frame(world);
                } else {
                    i += 1;
                }
            }
        }
    }

    fn takeoff_or_landing(world: &mut World, obj: ObjectId) -> bool {
        match world.find_module(obj, b"JetAIUpdate") {
            Some(ju) => world.module_call(obj, ju, ModuleCall::JetIsTakeoffOrLandingInProgress).as_bool(),
            None => false,
        }
    }

    /// `defectAllParkedUnits(newTeam, detectionTime)`.
    fn defect_all_parked_units(&mut self, world: &mut World, us: ObjectId, new_team: TeamId, detection_time: u32) -> Result<(), IniError> {
        self.build_info(world, us, true)?;
        self.purge_dead(world, us)?;
        for i in 0..self.spaces.len() {
            let id = self.spaces[i].object_in_space;
            if id == INVALID_ID || Self::dead_or_gone(world, id) {
                continue;
            }
            let takeoff_or_landing = Self::takeoff_or_landing(world, id);
            if world.is_above_terrain(id) && !takeoff_or_landing {
                let new_player = world.cuts.team_players.get(&new_team).copied();
                if new_player != world.get_controlling_player(id) {
                    self.release_space(world, us, id)?;
                    if world.objects.find(id).is_some_and(|o| o.producer_id == us) {
                        world.set_producer(id, INVALID_ID);
                    }
                }
            } else {
                world.body_defect(id, new_team, detection_time);
            }
        }
        self.purge_dead(world, us)
    }

    /// `killAllParkedUnits()`.
    fn kill_all_parked_units(&mut self, world: &mut World, us: ObjectId) -> Result<(), IniError> {
        self.build_info(world, us, true)?;
        self.purge_dead(world, us)?;
        for i in 0..self.spaces.len() {
            let id = self.spaces[i].object_in_space;
            if id == INVALID_ID || Self::dead_or_gone(world, id) {
                continue;
            }
            let takeoff_or_landing = Self::takeoff_or_landing(world, id);
            if world.is_above_terrain(id) && !takeoff_or_landing {
                continue;
            }
            if world.get_body_module(id).is_none() {
                return Err(IniError::undefined(Undefined::NullBodyModule));
            }
            world.kill(id, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
        }
        self.purge_dead(world, us)
    }

    fn door_flags(i: usize) -> (i32, i32) {
        let opening = MODELCONDITION_DOOR_2_OPENING + i as i32 * NUM_MODELCONDITION_DOOR_STATES;
        let closing = MODELCONDITION_DOOR_2_CLOSING + i as i32 * NUM_MODELCONDITION_DOOR_STATES;
        (opening, closing)
    }

    /// `update()`.
    fn do_update(&mut self, world: &mut World, us: ObjectId) -> Result<UpdateSleepTime, IniError> {
        self.build_info(world, us, true)?;
        self.purge_dead(world, us)?;
        let d = self.data.clone();
        let now = world.frame();
        if now >= self.next_heal_frame {
            self.next_heal_frame = now.wrapping_add(HEAL_RATE_FRAMES);
            let mut i = 0;
            while i < self.healing.len() {
                let id = self.healing[i].getting_healed_id;
                if id == INVALID_ID {
                    i += 1;
                    continue;
                }
                if Self::dead_or_gone(world, id) {
                    self.healing.remove(i);
                    continue;
                }
                let mut heal_info = DamageInfo::default();
                heal_info.input.damage_type = DAMAGE_HEALING;
                heal_info.input.death_type = DEATH_NONE;
                heal_info.input.source_id = us;
                heal_info.input.amount = HEAL_RATE_FRAMES as f32 * d.heal_amount * SECONDS_PER_LOGICFRAME_REAL;
                let body = world.get_body_module(id).ok_or_else(|| IniError::undefined(Undefined::NullBodyModule))?;
                world.module_call(id, body, ModuleCall::AttemptHealing(heal_info));
                i += 1;
            }
        }
        // "Periodically, we want to look at all the aircraft assignments and keep them near the front of the carrier at all times."
        if now >= self.next_cleanup_frame {
            self.next_cleanup_frame = now.wrapping_add(d.cleanup_frames);
            let mut complete = [false; MAX_RUNWAYS];
            for it in 0..self.spaces.len() {
                let occupant = self.spaces[it].object_in_space;
                let non_idle_jet = world.objects.find(occupant).map(|_| occupant);
                if non_idle_jet.is_none_or(|j| self.is_able_to_give_up_parking_space(world, j)) {
                    let mut runway_count = d.num_cols;
                    for temp in it..self.spaces.len() {
                        if runway_count > 0 {
                            runway_count -= 1;
                            continue;
                        }
                        let rw = self.spaces[it].runway as usize;
                        if *complete.get(rw).ok_or_else(|| IniError::undefined(Undefined::ArrayIndexOutOfRange))? {
                            continue;
                        }
                        // "Now we have the correct runway. Check if that spot has an idle plane in it!"
                        let parked = self.spaces[temp].object_in_space;
                        if world.objects.find(parked).is_some() && Self::is_able_to_move_forward(world, parked) && !plant(3826) {
                            self.spaces[it].object_in_space = parked;
                            self.spaces[temp].object_in_space = non_idle_jet.unwrap_or(INVALID_ID);
                            world.body_set_status(parked, mask(OBJECT_STATUS_REASSIGN_PARKING), true);
                            let prep = self.spaces[it].prep;
                            let ai = Self::jet_ai(world, parked).ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
                            world.module_call(parked, ai, ModuleCall::AiFollowExitProductionPath { path: vec![prep], ignore_object: us, command_source: CMD_FROM_AI });
                            complete[rw] = true;
                            self.next_cleanup_frame = now.wrapping_add(d.human_follow_frames);
                        }
                        // "Break through and advance to the next spot!"
                        break;
                    }
                }
            }
        }
        // "Handle automatic production of lost aircraft."
        if self.next_allowed_production_frame <= now {
            self.started_production_frame = FOREVER;
        }
        if self.spaces.iter().any(|s| s.object_in_space == INVALID_ID) {
            // "But are we already building one?"
            if let Some(pu) = world.get_production_update_interface(us) {
                let count = world.module_call(us, pu, ModuleCall::ProductionGetCount).as_int(0);
                if count == 0 && now >= self.next_allowed_production_frame && self.thing_template.is_some() {
                    let production_id = world.module_call(us, pu, ModuleCall::ProductionRequestUniqueUnitId).as_int(0);
                    world.module_call(us, pu, ModuleCall::ProductionQueueCreateUnit { template: self.thing_template, production_id });
                    self.started_production_frame = now;
                    self.next_allowed_production_frame = now.wrapping_add(d.replacement_frames).wrapping_add(d.dock_animation_frames);
                }
            }
        }
        // "If the carrier has at least one aircraft, then allow it to attack."
        let has_aircraft = self.spaces.iter().any(|s| s.object_in_space != INVALID_ID);
        world.body_set_status(us, mask(OBJECT_STATUS_NO_ATTACK), !has_aircraft);
        let has_drawable = world.has_drawable(us);
        for i in 0..d.num_cols.max(0) as usize {
            let s = self.spaces.get(i).ok_or_else(|| IniError::undefined(Undefined::ArrayIndexOutOfRange))?;
            let jet = s.object_in_space;
            if world.objects.find(jet).is_some()
                && !self.is_able_to_give_up_parking_space(world, jet)
                && self.is_in_position_to_takeoff(world, jet)?
                && self.has_takeoff_orders(world)
            {
                if i >= MAX_RUNWAYS {
                    return Err(IniError::undefined(Undefined::ArrayIndexOutOfRange));
                }
                if self.next_launch_wave_frame[i] <= now {
                    // "Handle making the ramp going up and holding the launch until it's completely up."
                    if !self.ramp_up[i] {
                        self.ramp_up[i] = true;
                        self.ramp_up_frame[i] = now.wrapping_add(d.launch_ramp_frames);
                        self.lower_ramp_frame[i] = FOREVER;
                        let (opening, closing) = Self::door_flags(i);
                        if !has_drawable {
                            return Err(IniError::undefined(Undefined::NullDrawable));
                        }
                        world.clear_and_set_model_condition_state(us, closing, opening);
                    }
                    // "Handle launching the wave of fighters."
                    if self.ramp_up[i] && self.ramp_up_frame[i] <= now && Self::jet_ai(world, jet).is_some() {
                        self.propagate_order_to_specific_plane(world, us, jet);
                        self.next_launch_wave_frame[i] = now.wrapping_add(d.launch_wave_frames);
                        self.catapult_system_frame[i] = now.wrapping_add(d.catapult_fire_frames);
                        self.lower_ramp_frame[i] = now.wrapping_add(d.lower_ramp_frames);
                    }
                }
            }
            if i >= MAX_RUNWAYS {
                return Err(IniError::undefined(Undefined::ArrayIndexOutOfRange));
            }
            // "Handle firing of the catapult steam effect upon launching planes."
            if self.catapult_system_frame[i] <= now {
                if let Some(ps) = d.runway_info[i].catapult_particle_system.clone() {
                    let r = self.runways.get(i).ok_or_else(|| IniError::undefined(Undefined::ArrayIndexOutOfRange))?;
                    let (transform, start) = (r.start_transform, r.start);
                    let id = world.create_particle_system_unplaced(ps);
                    self.catapult_system_frame[i] = FOREVER;
                    let transform = transform.ok_or_else(|| IniError::undefined(Undefined::UninitialisedLocal))?;
                    world.particle_system_set_local_transform(id, &transform);
                    world.particle_system_set_position(id, start);
                }
            }
            // "Handle lowering the ramp after the fighter has been launched."
            if self.ramp_up[i] && self.lower_ramp_frame[i] <= now {
                self.ramp_up[i] = false;
                let (opening, closing) = Self::door_flags(i);
                if !has_drawable {
                    return Err(IniError::undefined(Undefined::NullDrawable));
                }
                world.clear_and_set_model_condition_state(us, opening, closing);
            }
        }
        Ok(UPDATE_SLEEP_NONE)
    }

    /// `exitObjectViaDoor(newObj, exitDoor)`.
    fn exit_object_via_door(&mut self, world: &mut World, us: ObjectId, new_obj: ObjectId, exit_door: ExitDoorType) -> Result<(), IniError> {
        let mut ppi = None;
        if exit_door != DOOR_NONE_NEEDED {
            let Some(i) = self.spaces.iter().position(|s| s.object_in_space == INVALID_ID) else {
                return Ok(()); // "could not find the space. what?"
            };
            self.spaces[i].object_in_space = new_obj;
            ppi = Some(i);
        }
        let parking_offset = match world.find_module(new_obj, b"JetAIUpdate") {
            Some(ju) => world.module_call(new_obj, ju, ModuleCall::JetGetParkingOffset).as_real(0.0),
            None => 0.0,
        };
        if !self.reserve_space(world, us, new_obj, parking_offset, true)?.0 {
            // "no spaces available, how did we get here?" (the info's place and orientation are the carrier's: never read)
        }
        let Some(creation) = self.get_creation_locations(new_obj) else {
            return Ok(()); // "No creation locations specified for runway"
        };
        // (`ppi` is read unchecked from here: no door needed is undefined)
        let i = ppi.ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
        let first = *creation.first().ok_or_else(|| IniError::undefined(Undefined::EndIteratorDereferenced))?;
        world.set_position(new_obj, &first);
        let r = self.spaces[i].runway as usize;
        let orient = self.runways.get(r).and_then(|r| r.start_orient).ok_or_else(|| IniError::undefined(Undefined::UninitialisedLocal))?;
        world.set_orientation(new_obj, orient);
        world.cut(crate::logic::world::object_cuts::CutCall::PathfinderAddObjectToPathfindMap { object: new_obj });
        if let Some(ai) = Self::jet_ai(world, new_obj) {
            let prep = self.spaces[i].prep;
            world.module_call(new_obj, ai, ModuleCall::AiFollowExitProductionPath { path: vec![prep], ignore_object: us, command_source: CMD_FROM_AI });
        }
        Ok(())
    }

    /// `aiDoCommand(parms)`: a command not from the AI is taken as the carrier's orders and passed on (the base's is never run).
    fn ai_do_command(&mut self, world: &mut World, cmd: i32, command_source: i32, obj: ObjectId, pos: Coord3D) {
        if command_source == CMD_FROM_AI {
            return;
        }
        match cmd {
            AICMD_GUARD_POSITION | AICMD_ATTACK_POSITION | AICMD_ATTACKMOVE_TO_POSITION => {
                self.designated_target = INVALID_ID;
                self.designated_position = pos;
                self.designated_command = cmd;
                self.propagate_orders_to_planes(world);
            }
            AICMD_FORCE_ATTACK_OBJECT | AICMD_ATTACK_OBJECT => {
                self.designated_target = if world.objects.find(obj).is_some() { obj } else { INVALID_ID };
                self.designated_position = Coord3D::default();
                self.designated_command = cmd;
                self.propagate_orders_to_planes(world);
            }
            AICMD_IDLE => {
                self.designated_target = INVALID_ID;
                self.designated_position = Coord3D::default();
                self.designated_command = cmd;
                if !plant(3827) {
                    self.propagate_orders_to_planes(world);
                }
            }
            _ => self.designated_command = AICMD_NO_COMMAND,
        }
    }

    /// `propagateOrdersToPlanes()`.
    fn propagate_orders_to_planes(&mut self, world: &mut World) {
        let us = self.base.behavior().object();
        for i in 0..self.spaces.len() {
            let jet = self.spaces[i].object_in_space;
            if jet != INVALID_ID && world.objects.find(jet).is_some() && self.is_able_to_give_up_parking_space(world, jet) {
                self.propagate_order_to_specific_plane(world, us, jet);
            }
        }
    }

    /// `hasTakeoffOrders()`: an attack on a target that is gone is forgotten.
    fn has_takeoff_orders(&mut self, world: &World) -> bool {
        match self.designated_command {
            AICMD_GUARD_POSITION | AICMD_ATTACK_POSITION | AICMD_ATTACKMOVE_TO_POSITION => true,
            AICMD_FORCE_ATTACK_OBJECT | AICMD_ATTACK_OBJECT => {
                if world.objects.find(self.designated_target).is_some() {
                    return true;
                }
                self.designated_command = AICMD_NO_COMMAND;
                self.designated_target = INVALID_ID;
                false
            }
            _ => false,
        }
    }

    /// `propagateOrderToSpecificPlane(jet)`.
    fn propagate_order_to_specific_plane(&self, world: &mut World, us: ObjectId, jet: ObjectId) {
        let Some(ai) = Self::jet_ai(world, jet) else { return };
        let target = if world.objects.find(self.designated_target).is_some() { self.designated_target } else { INVALID_ID };
        let pos = self.designated_position;
        let call = match self.designated_command {
            AICMD_GUARD_POSITION => ModuleCall::AiGuardPosition { pos, guard_mode: GUARDMODE_NORMAL, command_source: CMD_FROM_AI },
            AICMD_ATTACK_POSITION => ModuleCall::AiAttackPosition { pos: Some(pos), max_shots: NO_MAX_SHOTS_LIMIT, command_source: CMD_FROM_AI },
            AICMD_FORCE_ATTACK_OBJECT | AICMD_ATTACK_OBJECT => ModuleCall::AiForceAttackObject { victim: target, max_shots: NO_MAX_SHOTS_LIMIT, command_source: CMD_FROM_PLAYER },
            AICMD_ATTACKMOVE_TO_POSITION => ModuleCall::AiAttackMoveToPosition { pos, max_shots: NO_MAX_SHOTS_LIMIT, command_source: CMD_FROM_AI },
            AICMD_IDLE => ModuleCall::AiEnter { target: us, command_source: CMD_FROM_AI },
            _ => return,
        };
        world.module_call(jet, ai, call);
    }

    fn record<T>(world: &mut World, me: ModuleHandle, r: Result<T, IniError>) -> Option<T> {
        match r {
            Ok(t) => Some(t),
            Err(e) => {
                world.cuts.module_errors.push((me, e));
                None
            }
        }
    }
}

impl UpdateBehavior for FlightDeckBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.base
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let r = self.do_update(world, me.object);
        Self::record(world, me, r).unwrap_or(UPDATE_SLEEP_NONE)
    }
    /// The spaces' occupants, the runways', the healees, the frames, the orders, the runways' launch state.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        let mode = xfer.get_xfer_mode();
        // "first, build our info, so it won't be overwritten later." (buildInfo(FALSE): "the planes are going to save themselves")
        if mode == XferMode::Load {
            if let Some(b) = self.staged_build.take() {
                if let Err(e) = self.apply_build(b) {
                    self.staged_error = Some(e);
                    return Err(XferStatus::ErrorUnknown);
                }
            }
        }
        let mut spaces_count = self.spaces.len() as u8;
        xfer.xfer_unsigned_byte(&mut spaces_count)?;
        if mode == XferMode::Save {
            for s in self.spaces.iter_mut() {
                xfer.xfer_object_id(&mut s.object_in_space)?;
            }
        } else if mode == XferMode::Load {
            for i in 0..spaces_count as usize {
                let mut id = INVALID_ID;
                xfer.xfer_object_id(&mut id)?;
                if let Some(s) = self.spaces.get_mut(i) {
                    s.object_in_space = id;
                }
            }
        }
        let mut runways_count = self.runways.len() as u8;
        xfer.xfer_unsigned_byte(&mut runways_count)?;
        if mode == XferMode::Save {
            for r in self.runways.iter_mut() {
                xfer.xfer_object_id(&mut r.in_use_by_for_takeoff)?;
                xfer.xfer_object_id(&mut r.in_use_by_for_landing)?;
            }
        } else if mode == XferMode::Load {
            for i in 0..runways_count as usize {
                let (mut takeoff, mut landing) = (INVALID_ID, INVALID_ID);
                xfer.xfer_object_id(&mut takeoff)?;
                xfer.xfer_object_id(&mut landing)?;
                if let Some(r) = self.runways.get_mut(i) {
                    r.in_use_by_for_takeoff = takeoff;
                    r.in_use_by_for_landing = landing;
                }
            }
        }
        let mut heal_count = self.healing.len() as u8;
        xfer.xfer_unsigned_byte(&mut heal_count)?;
        if mode == XferMode::Save {
            for h in self.healing.iter_mut() {
                xfer.xfer_object_id(&mut h.getting_healed_id)?;
                xfer.xfer_unsigned_int(&mut h.heal_start_frame)?;
            }
        } else if mode == XferMode::Load {
            self.healing.clear();
            for _ in 0..heal_count {
                let mut h = HealingInfo::default();
                xfer.xfer_object_id(&mut h.getting_healed_id)?;
                xfer.xfer_unsigned_int(&mut h.heal_start_frame)?;
                self.healing.push(h);
            }
        }
        xfer.xfer_unsigned_int(&mut self.next_heal_frame)?;
        xfer.xfer_unsigned_int(&mut self.next_cleanup_frame)?;
        xfer.xfer_unsigned_int(&mut self.started_production_frame)?;
        if !plant(3828) {
            xfer.xfer_unsigned_int(&mut self.next_allowed_production_frame)?;
        }
        xfer.xfer_object_id(&mut self.designated_target)?;
        xfer.xfer_int(&mut self.designated_command)?;
        xfer.xfer_coord3d(&mut self.designated_position)?;
        let mut max_runways = MAX_RUNWAYS as u32;
        xfer.xfer_unsigned_int(&mut max_runways)?;
        for i in 0..MAX_RUNWAYS {
            if max_runways as usize <= MAX_RUNWAYS {
                xfer.xfer_unsigned_int(&mut self.next_launch_wave_frame[i])?;
                xfer.xfer_unsigned_int(&mut self.ramp_up_frame[i])?;
                xfer.xfer_unsigned_int(&mut self.catapult_system_frame[i])?;
                xfer.xfer_unsigned_int(&mut self.lower_ramp_frame[i])?;
                // (`m_rampUp[MAX_RUNWAYS]`: the byte after the flags)
                xfer.xfer_bool(&mut self.ramp_pad)?;
            } else {
                // "We must be loading... so load filler but don't assign it."
                let (mut dummy_int, mut dummy_bool) = (0u32, false);
                xfer.xfer_unsigned_int(&mut dummy_int)?;
                xfer.xfer_unsigned_int(&mut dummy_int)?;
                xfer.xfer_unsigned_int(&mut dummy_int)?;
                xfer.xfer_unsigned_int(&mut dummy_int)?;
                xfer.xfer_bool(&mut dummy_bool)?;
            }
        }
        Ok(())
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::FlightDeckBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::FlightDeckBehavior(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        let us = me.object;
        match call {
            ModuleCall::OnDie(_) => {
                let r = self.kill_all_parked_units(world, us);
                Self::record(world, me, r);
                ModuleReply::Done
            }
            ModuleCall::HasParkingPlaceInterface | ModuleCall::HasUpdateExitInterface => ModuleReply::Bool(true),
            ModuleCall::AiDoCommand { cmd, command_source, obj, pos } => {
                self.ai_do_command(world, cmd, command_source, obj, pos);
                ModuleReply::Done
            }
            // ExitInterface
            ModuleCall::IsExitBusy => ModuleReply::Bool(false),
            ModuleCall::ReserveDoorForExit { .. } => ModuleReply::Int(DOOR_1),
            ModuleCall::ExitObjectViaDoor { obj, door } => {
                let r = self.exit_object_via_door(world, us, obj, door);
                Self::record(world, me, r);
                ModuleReply::Done
            }
            ModuleCall::UnreserveDoorForExit(_) | ModuleCall::ExitObjectByBudding { .. } | ModuleCall::SetRallyPoint(_) => ModuleReply::Done,
            ModuleCall::GetExitPosition | ModuleCall::GetNaturalRallyPoint | ModuleCall::GetNaturalRallyPointWithOffset(_) | ModuleCall::GetRallyPoint => {
                ModuleReply::Unhandled
            }
            // ParkingPlaceBehaviorInterface
            ModuleCall::ParkingPlaceShouldReserveDoorWhenQueued { .. } => ModuleReply::Bool(true),
            ModuleCall::ParkingPlaceHasAvailableSpaceFor { .. } => ModuleReply::Bool(self.has_available_space_for(world)),
            ModuleCall::ParkingPlaceHasReservedSpace { intended } => ModuleReply::Bool(self.has_reserved_space(intended)),
            ModuleCall::ParkingPlaceGetSpaceIndex { id } => ModuleReply::Int(self.get_space_index(id)),
            ModuleCall::ParkingPlaceReserveSpace { id, parking_offset, want_info } => {
                let r = self.reserve_space(world, us, id, parking_offset, want_info);
                match Self::record(world, me, r) {
                    Some((ok, info)) => ModuleReply::PPInfo { ok, info },
                    None => ModuleReply::PPInfo { ok: false, info: None },
                }
            }
            ModuleCall::ParkingPlaceReleaseSpace { id } => {
                let r = self.release_space(world, us, id);
                Self::record(world, me, r);
                ModuleReply::Done
            }
            ModuleCall::ParkingPlaceReserveRunway { id, for_landing } => {
                let r = self.reserve_runway(world, us, id, for_landing);
                ModuleReply::Bool(Self::record(world, me, r).unwrap_or(false))
            }
            ModuleCall::ParkingPlaceCalcPPInfo { id, info } => {
                let r = self.calc_pp_info(id, info);
                match Self::record(world, me, r) {
                    Some(info) => ModuleReply::PPInfo { ok: true, info: Some(info) },
                    None => ModuleReply::PPInfo { ok: false, info: None },
                }
            }
            ModuleCall::ParkingPlaceReleaseRunway { id } => {
                let r = self.release_runway(world, us, id);
                Self::record(world, me, r);
                ModuleReply::Done
            }
            ModuleCall::ParkingPlaceGetRunwayCount => ModuleReply::Int(self.runways.len() as i32),
            ModuleCall::ParkingPlaceGetRunwayReservation { runway, kind } => {
                let landing = match kind {
                    0 => Some(false),
                    1 => Some(true),
                    _ => None,
                };
                let r = self.get_runway_reservation(world, us, runway, landing);
                ModuleReply::Id(Self::record(world, me, r).unwrap_or(INVALID_ID))
            }
            // "Aircraft carrier controls this functionality with an iron fist."
            ModuleCall::ParkingPlaceTransferRunwayReservation { .. } => ModuleReply::Done,
            ModuleCall::ParkingPlaceGetApproachHeight => ModuleReply::Real(self.data.approach_height),
            ModuleCall::GetLandingDeckHeightOffset => ModuleReply::Real(self.data.landing_deck_height_offset),
            ModuleCall::ParkingPlaceSetHealee { id, add } => {
                self.set_healee(world, id, add);
                ModuleReply::Done
            }
            ModuleCall::ParkingPlaceKillAllParkedUnits => {
                let r = self.kill_all_parked_units(world, us);
                Self::record(world, me, r);
                ModuleReply::Done
            }
            ModuleCall::DefectAllParkedUnits { new_team, detection_time } => {
                let r = self.defect_all_parked_units(world, us, new_team, detection_time);
                Self::record(world, me, r);
                ModuleReply::Done
            }
            ModuleCall::ParkingPlaceCalcBestParkingAssignment => ModuleReply::Unhandled,
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for FlightDeckBehavior {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.base.crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.base.clone();
        let r = update_behavior::xfer_own(self, &mut base, xfer);
        self.base = base;
        r
    }
    fn load_post_process(&mut self) {
        self.base.load_post_process();
    }
}

impl FlightDeckBehavior {
    /// `loadPostProcess()`: the template found again, then the base's.
    pub fn world_load_post_process(&mut self, world: &World) {
        self.thing_template = world.thing_factory_find_template(&self.data.thing_template_name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let m = FlightDeckBehavior::unconstructed(1, Some(Arc::new(FlightDeckBehaviorModuleData::default()))).unwrap();
        assert_eq!((m.next_heal_frame, m.started_production_frame, m.designated_command), (FOREVER, FOREVER, AICMD_NO_COMMAND));
        assert_eq!(HEAL_RATE_FRAMES, 6);
        assert_eq!(FlightDeckBehavior::door_flags(1), (26, 27));
    }
}
