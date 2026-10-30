// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/RailroadGuideAIUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/RailroadGuideAIUpdate.cpp
//   GeneralsMD/Code/GameEngine/Source/GameClient/Statistics.cpp   (`MuLaw`, `Normalize`, `NormalizeToRange`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `RailroadBehavior`: a train car that follows a track of waypoints (a `PhysicsBehavior` whose update and collision are its own). The
//! locomotive reads the track from the waypoint nearest it the first time it updates, hitches its carriages (cars the map placed behind it,
//! or new ones from its template list) and drives the train: it accelerates to its top speed, brakes at a waypoint named `...Station` or
//! `...Disembark` (whose passengers all get out), waits there, whistles, and leaves; a `...PingPong` waypoint turns it round. Each car pulls
//! the next along the track a hitch behind it (`getPulled`); a car nobody has pulled for three frames is the lead of what is left and coasts
//! to a stop. A moving train shoves what it hits aside, flattening or killing it, and kills a faction structure, a demo trap (which kills the
//! train car) and another train's end car.
//!
//! # Upstream facts kept
//!
//! - The class is in `RailroadGuideAIUpdate.cpp` but is a `PhysicsBehavior`, not an AI update; its name key is `RailroadBehavior`, so a train
//!   car's `getPhysics()` is `NULL` (`Object::Object` keeps only a module named `PhysicsBehavior` there).
//! - The track (`TrainTrack`) is shared by the cars of a train ("ALWAYS INCREFERENCE WHENEVER YOU INHERIT THIS POINTER"); it is written only
//!   while the locomotive builds it, and it is not saved: a load reads it again from the anchor waypoint, whose id is saved.
//! - A car that is not a locomotive never builds a track: it gets one only when a locomotive hitches it (`m_currentPointHandle` is set only
//!   by a locomotive, so a carriage counts a track joint, and plays its clickety-clack, every frame it is not in a tunnel).
//! - `getPulled(info)` sets the PULLER's `previousWaypoint` to its `currentWaypoint`, after copying it as its own conductor's info.
//! - The puller's position a collision reads is the object's own, read again after it was moved (`theirLoc` points at it).
//! - The template list a locomotive hitches by is used only if its first name finds a template; a car the map placed behind it is hitched
//!   (with its own followers, by proximity) only then too.
//! - A victim with physics never gets the data's default impact sound: `getBounceSound()` is never `NULL` (the audio manager's silent
//!   event, of no name, when the physics has no sound), so an impact with such a victim plays its bounce sound or nothing.
//! - `MAX(heft.z, getGroundHeight(...) + 2)` asks the terrain a second time when it picks the ground (the macro evaluates the argument
//!   again).
//! - `MuLaw`, `log` and the `fabs` of the impact sound are the C library's double functions on float arguments, rounded to `Real`.
//! - The three sounds are copies of the data's events (with the object's id) made by the constructor and again by `loadPostProcess`: the
//!   playing handles, the clickety-clack's position and volume (set after it plays, for the next time) start over then.
//!
//! # Where upstream is undefined
//!
//! A track whose waypoint path cycles without coming back to the anchor (`loadTrackData` never ends: `EndlessLoop`); a looping track of no
//! length (`FindPosByPathDistance`'s wrap never ends: `EndlessLoop`); a point after the last read as the next (`EndIteratorDereferenced`);
//! killing an object with no body (`NullBodyModule`); an impact sound for an object no player controls (`NullControllingPlayer`); a station
//! wait that counts below `INT_MIN` (`SignedIntOverflow`); a carriage the factory cannot make (`NewObjectFailed`). Recorded in
//! `world.cuts.module_errors`.
//!
//! `p1w13-sonnet`, batch R80, 2026-10-30.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_common::{std_angle_diff, Relationship};
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::{
    KINDOF_FS_BASE_DEFENSE, KINDOF_FS_FACTORY, KINDOF_FS_POWER, KINDOF_FS_TECHNOLOGY, KINDOF_HUGE_VEHICLE, KINDOF_INFANTRY, KINDOF_REBUILD_HOLE,
    KINDOF_STRUCTURE, KINDOF_VEHICLE,
};
use crate::common::model_state::MODELCONDITION_OVER_WATER;
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_COLLIDE};
use crate::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION;
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_CRUSH, DAMAGE_UNRESISTABLE, DEATH_CRUSHED, DEATH_NORMAL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{UpdateModule, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::physics_behavior::{physics_field, PhysicsBehavior, PhysicsBehaviorModuleData, PHYSICS_FIELD_TOKENS};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::ww::matrix3d::Matrix3D;

/// `NORMAL_VEL_Z`, `NORMAL_MASS`.
const NORMAL_VEL_Z: f32 = 0.25;
const NORMAL_MASS: f32 = 50.0;
/// `FRAMES_UNPULLED_LONG_ENOUGH_TO_UNHITCH`.
const FRAMES_UNPULLED_LONG_ENOUGH_TO_UNHITCH: i32 = 2;
/// `INVALID_WAYPOINT_ID`.
pub const INVALID_WAYPOINT_ID: u32 = 0x7FFF_FFFF;
/// `CMD_FROM_AI`.
const CMD_FROM_AI: i32 = 2;
/// The `0xfacade` the constructor marks the handles and waypoints with.
const FACADE: i32 = 0x00fa_cade;

/// `FROM_CENTER_2D` (`PartitionManager.h`'s `DistanceCalculationType`).
const FROM_CENTER_2D: i32 = 0;
/// `enum ConductorState`.
pub const APPLY_BRAKES: i32 = 0;
pub const WAIT_AT_STATION: i32 = 1;
pub const ACCELERATE: i32 = 2;
pub const COAST: i32 = 3;
/// `enum StationTask`: `DO_NOTHING`.
const DO_NOTHING: i32 = 0;

/// `MuLaw(valueToRun, maxValueForVal, mu)` (Statistics.cpp): `sign(testVal) * log(1 + mu * fabs(testVal)) / log(1 + mu)`, in double.
fn mu_law(value_to_run: f32, max_value_for_val: f32, mu: f32) -> f32 {
    let test_val = (value_to_run - max_value_for_val / 2.0) / (max_value_for_val / 2.0);
    let sign = if test_val > 0.0 {
        1.0
    } else if test_val < 0.0 {
        -1.0
    } else {
        0.0
    };
    (sign * (f64::from(1.0 + mu * test_val.abs())).ln() / f64::from(1.0 + mu).ln()) as f32
}

/// `NormalizeToRange(value, minRange, maxRange, outRangeMin, outRangeMax)` (Statistics.cpp, with `Normalize`).
fn normalize_to_range(value: f32, min_range: f32, max_range: f32, out_min: f32, out_max: f32) -> f32 {
    ((value - min_range) / (max_range - min_range)) * (out_max - out_min) + out_min
}

/// `class RailroadBehaviorModuleData : public PhysicsBehaviorModuleData`.
#[derive(Clone, Debug)]
pub struct RailroadBehaviorModuleData {
    /// The base.
    pub physics: PhysicsBehaviorModuleData,
    /// `m_carriageTemplateNameData`.
    pub carriage_template_name_data: Vec<Vec<u8>>,
    /// `m_pathPrefixName`, `m_CrashFXTemplateName` (read, not used).
    pub path_prefix_name: Vec<u8>,
    pub crash_fx_template_name: Vec<u8>,
    /// `m_isLocomotive`.
    pub is_locomotive: bool,
    /// `m_runningGarrisonSpeedMax`, `m_killSpeedMin`, `m_speedMax`, `m_acceleration`, `m_braking`, `m_friction`.
    pub running_garrison_speed_max: f32,
    pub kill_speed_min: f32,
    pub speed_max: f32,
    pub acceleration: f32,
    pub braking: f32,
    pub friction: f32,
    /// `m_waitAtStationTime`.
    pub wait_at_station_time: u32,
    /// The six events' names (empty: none).
    pub running_sound: Vec<u8>,
    pub clickety_clack_sound: Vec<u8>,
    pub big_metal_impact_default_sound: Vec<u8>,
    pub small_metal_impact_default_sound: Vec<u8>,
    pub meaty_impact_default_sound: Vec<u8>,
    pub whistle_sound: Vec<u8>,
}

impl Default for RailroadBehaviorModuleData {
    /// `RailroadBehaviorModuleData()`.
    fn default() -> Self {
        RailroadBehaviorModuleData {
            physics: PhysicsBehaviorModuleData::default(),
            carriage_template_name_data: Vec::new(),
            path_prefix_name: Vec::new(),
            crash_fx_template_name: Vec::new(),
            is_locomotive: false,
            running_garrison_speed_max: 1.0,
            kill_speed_min: 1.0,
            speed_max: 4.0,
            acceleration: 1.01,
            braking: 0.99,
            friction: 0.97,
            wait_at_station_time: 150,
            running_sound: Vec::new(),
            clickety_clack_sound: Vec::new(),
            big_metal_impact_default_sound: Vec::new(),
            small_metal_impact_default_sound: Vec::new(),
            meaty_impact_default_sound: Vec::new(),
            whistle_sound: Vec::new(),
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

macro_rules! physics_entry {
    ($k:expr) => {
        FieldParse::<RailroadBehaviorModuleData>::new(PHYSICS_FIELD_TOKENS[$k], |i, d, u| physics_field($k, i, &mut d.physics, u), UserData::None)
    };
}

/// `PhysicsBehaviorModuleData::buildFieldParse`'s table, on the base of this data.
static PHYSICS_FIELD_PARSE: [FieldParse<RailroadBehaviorModuleData>; 19] = [
    physics_entry!(0),
    physics_entry!(1),
    physics_entry!(2),
    physics_entry!(3),
    physics_entry!(4),
    physics_entry!(5),
    physics_entry!(6),
    physics_entry!(7),
    physics_entry!(8),
    physics_entry!(9),
    physics_entry!(10),
    physics_entry!(11),
    physics_entry!(12),
    physics_entry!(13),
    physics_entry!(14),
    physics_entry!(15),
    physics_entry!(16),
    physics_entry!(17),
    FieldParse::end(),
];

/// `RailroadBehaviorModuleData::buildFieldParse`'s table.
static FIELD_PARSE: [FieldParse<RailroadBehaviorModuleData>; 18] = [
    FieldParse::new("PathPrefixName", |i, d, _| { d.path_prefix_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("CrashFXTemplateName", |i, d, _| { d.crash_fx_template_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("IsLocomotive", |i, d, _| { d.is_locomotive = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("CarriageTemplateName", |i, d, _| i.parse_ascii_string_vector_append(&mut d.carriage_template_name_data), UserData::None),
    FieldParse::new("BigMetalBounceSound", |i, d, _| parse_sound(i, &mut d.big_metal_impact_default_sound), UserData::None),
    FieldParse::new("SmallMetalBounceSound", |i, d, _| parse_sound(i, &mut d.small_metal_impact_default_sound), UserData::None),
    FieldParse::new("MeatyBounceSound", |i, d, _| parse_sound(i, &mut d.meaty_impact_default_sound), UserData::None),
    FieldParse::new("RunningGarrisonSpeedMax", |i, d, _| { d.running_garrison_speed_max = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("KillSpeedMin", |i, d, _| { d.kill_speed_min = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("SpeedMax", |i, d, _| { d.speed_max = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Acceleration", |i, d, _| { d.acceleration = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Braking", |i, d, _| { d.braking = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("WaitAtStationTime", |i, d, _| { d.wait_at_station_time = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("RunningSound", |i, d, _| parse_sound(i, &mut d.running_sound), UserData::None),
    FieldParse::new("ClicketyClackSound", |i, d, _| parse_sound(i, &mut d.clickety_clack_sound), UserData::None),
    FieldParse::new("WhistleSound", |i, d, _| parse_sound(i, &mut d.whistle_sound), UserData::None),
    FieldParse::new("Friction", |i, d, _| { d.friction = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl RailroadBehaviorModuleData {
    /// `buildFieldParse(p)`: `PhysicsBehaviorModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, RailroadBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&PHYSICS_FIELD_PARSE)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<RailroadBehaviorModuleData> {
        let mut data = RailroadBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, RailroadBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The module factory's data proc.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::RailroadBehavior(Arc::new(RailroadBehaviorModuleData::friend_new_module_data(ini)?)))
}

/// `struct TrackPoint`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrackPoint {
    /// `m_position`, `m_distanceFromPrev`, `m_distanceFromFirst`, `m_handle` (the waypoint's id).
    pub position: Coord3D,
    pub distance_from_prev: f32,
    pub distance_from_first: f32,
    pub handle: i32,
    /// `m_isFirstPoint`, `m_isLastPoint`, `m_isTunnelOrBridge`, `m_isStation`, `m_isDisembark`, `m_isPingPong`.
    pub is_first_point: bool,
    pub is_last_point: bool,
    pub is_tunnel_or_bridge: bool,
    pub is_station: bool,
    pub is_disembark: bool,
    pub is_ping_pong: bool,
}

impl Default for TrackPoint {
    /// `TrackPoint()` (`clear()`).
    fn default() -> Self {
        TrackPoint {
            position: Coord3D::default(),
            distance_from_prev: 0.0,
            distance_from_first: 0.0,
            handle: FACADE,
            is_first_point: false,
            is_last_point: false,
            is_tunnel_or_bridge: false,
            is_station: false,
            is_disembark: false,
            is_ping_pong: false,
        }
    }
}

/// `struct TrainTrack`: the points, whether the track loops and its length (`m_isValid` is never read; the reference count is the `Arc`'s).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrainTrack {
    /// `m_isLooping`, `m_length`.
    pub is_looping: bool,
    pub length: f32,
    /// `m_pointList`.
    pub points: Vec<TrackPoint>,
}

/// `struct RailroadBehavior::PullInfo`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PullInfo {
    /// `m_direction` ("1 = forward, -1 = backward"), `speed`, `trackDistance`, `towHitchPosition`.
    pub direction: f32,
    pub speed: f32,
    pub track_distance: f32,
    pub tow_hitch_position: Coord3D,
    /// `m_mostRecentSpecialPointHandle`, `previousWaypoint`, `currentWaypoint`.
    pub most_recent_special_point_handle: i32,
    pub previous_waypoint: u32,
    pub current_waypoint: u32,
}

impl Default for PullInfo {
    /// The constructor's values.
    fn default() -> Self {
        PullInfo {
            direction: 1.0,
            speed: 0.0,
            track_distance: 0.0,
            tow_hitch_position: Coord3D::default(),
            most_recent_special_point_handle: FACADE,
            previous_waypoint: FACADE as u32,
            current_waypoint: FACADE as u32,
        }
    }
}

impl PullInfo {
    /// `xferPullInfo(xfer)`: version 1.
    fn xfer_pull_info(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)?;
        xfer.xfer_real(&mut self.direction)?;
        xfer.xfer_real(&mut self.speed)?;
        xfer.xfer_real(&mut self.track_distance)?;
        xfer.xfer_coord3d(&mut self.tow_hitch_position)?;
        xfer.xfer_int(&mut self.most_recent_special_point_handle)?;
        xfer.xfer_unsigned_int(&mut self.previous_waypoint)?;
        xfer.xfer_unsigned_int(&mut self.current_waypoint)
    }

    /// The seven fields as words (the state the test compares).
    pub fn words(&self) -> [u32; 9] {
        [
            self.direction.to_bits(),
            self.speed.to_bits(),
            self.track_distance.to_bits(),
            self.tow_hitch_position.x.to_bits(),
            self.tow_hitch_position.y.to_bits(),
            self.tow_hitch_position.z.to_bits(),
            self.most_recent_special_point_handle as u32,
            self.previous_waypoint,
            self.current_waypoint,
        ]
    }
}

/// An `AudioEventRTS` member: the event's name, the object it is attached to, the position and volume set on it, and its playing handle.
#[derive(Clone, Debug, Default, PartialEq)]
struct Sound {
    name: Vec<u8>,
    object_id: ObjectId,
    position: Option<Coord3D>,
    volume: Option<f32>,
    handle: u32,
}

impl Sound {
    /// `m_x = modData->m_x; m_x.setObjectID(getObject()->getID());`
    fn of(name: &[u8], object_id: ObjectId) -> Sound {
        Sound { name: name.to_vec(), object_id, ..Default::default() }
    }

    /// `TheAudio->addAudioEvent(&m_x)`: the handle.
    fn add(&self, world: &mut World) -> u32 {
        world.audio_add_event_full(self.name.clone(), self.object_id, self.position, None, self.volume)
    }
}

/// `class RailroadBehavior : public PhysicsBehavior`.
#[derive(Clone, Debug)]
pub struct RailroadBehavior {
    /// The base.
    base: PhysicsBehavior,
    /// The data (this class's part; the base holds its own).
    data: Arc<RailroadBehaviorModuleData>,
    /// `m_nextStationTask` (a `StationTask`, never changed but saved).
    next_station_task: i32,
    /// `m_trailerID`: "the ID of the object I am directly pulling".
    trailer_id: ObjectId,
    /// `conductorPullInfo`, `m_pullInfo`.
    conductor_pull_info: PullInfo,
    pull_info: PullInfo,
    /// `m_whistleSound`, `m_clicketyClackSound`, `m_runningSound`.
    whistle_sound: Sound,
    clickety_clack_sound: Sound,
    running_sound: Sound,
    /// `m_track` (shared by the train's cars).
    track: Option<Arc<TrainTrack>>,
    /// `m_currentPointHandle`, `m_waitAtStationTimer`.
    current_point_handle: i32,
    wait_at_station_timer: i32,
    /// `m_carriagesCreated`, `m_hasEverBeenHitched`, `m_trackDataLoaded`, `m_waitingInWings`, `m_endOfLine`, `m_isLocomotive`,
    /// `m_isLeadCarraige`.
    carriages_created: bool,
    has_ever_been_hitched: bool,
    track_data_loaded: bool,
    waiting_in_wings: bool,
    end_of_line: bool,
    is_locomotive: bool,
    is_lead_carraige: bool,
    /// `m_wantsToBeLeadCarraige`: the frames since the car was last pulled.
    wants_to_be_lead_carraige: i32,
    /// `m_disembark`, `m_inTunnel`, `m_held`.
    disembark: bool,
    in_tunnel: bool,
    held: bool,
    /// `m_conductorState`.
    conductor_state: i32,
    /// `m_anchorWaypointID`.
    anchor_waypoint_id: u32,
}

impl PartialEq for RailroadBehavior {
    /// The same base, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base
            && Arc::ptr_eq(&self.data, &other.data)
            && self.state_words() == other.state_words()
            && (self.whistle_sound == other.whistle_sound && self.clickety_clack_sound == other.clickety_clack_sound && self.running_sound == other.running_sound)
            && self.track == other.track
    }
}

impl RailroadBehavior {
    /// `getInterfaceMask()`: `PhysicsBehavior`'s.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_COLLIDE
    }

    /// The constructor's first half: the base's (`INI_INVALID_DATA` for null data), then this class's.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<RailroadBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = PhysicsBehavior::unconstructed(object, Some(Arc::new(data.physics.clone())))?;
        let is_locomotive = data.is_locomotive;
        Ok(RailroadBehavior {
            base,
            next_station_task: DO_NOTHING,
            trailer_id: INVALID_ID,
            conductor_pull_info: PullInfo::default(),
            pull_info: PullInfo::default(),
            whistle_sound: Sound::of(&data.whistle_sound, object),
            clickety_clack_sound: Sound::of(&data.clickety_clack_sound, object),
            running_sound: Sound::of(&data.running_sound, object),
            track: None,
            current_point_handle: FACADE,
            wait_at_station_timer: 0,
            carriages_created: false,
            has_ever_been_hitched: false,
            track_data_loaded: false,
            waiting_in_wings: true,
            end_of_line: false,
            is_locomotive,
            // "for now, I am the lead, only if I am the locomotive"
            is_lead_carraige: is_locomotive,
            wants_to_be_lead_carraige: 0,
            disembark: false,
            in_tunnel: false,
            held: false,
            conductor_state: if is_locomotive { ACCELERATE } else { COAST },
            anchor_waypoint_id: INVALID_WAYPOINT_ID,
            data,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        self.base.update_module()
    }

    /// The track the car has (`None`: `NULL`).
    pub fn track(&self) -> Option<&Arc<TrainTrack>> {
        self.track.as_ref()
    }

    /// The state the test compares: the base's 23 words, then this class's: the task, the trailer, the two pull infos, the handle and the
    /// timer, the flags (a word, bit 0 `m_carriagesCreated` .. bit 9 `m_held`), the lead count, the conductor state, the anchor, the three
    /// sounds' playing handles, the clickety-clack's position (x, y, z; 0 when unset), whether it has one, and its volume (bits; 0 unset).
    pub fn state_words(&self) -> Vec<u32> {
        let mut w = self.base.state_words();
        w.push(self.next_station_task as u32);
        w.push(self.trailer_id as u32);
        w.extend(self.conductor_pull_info.words());
        w.extend(self.pull_info.words());
        w.push(self.current_point_handle as u32);
        w.push(self.wait_at_station_timer as u32);
        let flags = [
            self.carriages_created,
            self.has_ever_been_hitched,
            self.track_data_loaded,
            self.waiting_in_wings,
            self.end_of_line,
            self.is_locomotive,
            self.is_lead_carraige,
            self.disembark,
            self.in_tunnel,
            self.held,
        ];
        w.push(flags.iter().enumerate().fold(0u32, |a, (i, &f)| a | (u32::from(f) << i)));
        w.push(self.wants_to_be_lead_carraige as u32);
        w.push(self.conductor_state as u32);
        w.push(self.anchor_waypoint_id);
        w.extend([self.whistle_sound.handle, self.clickety_clack_sound.handle, self.running_sound.handle]);
        let p = self.clickety_clack_sound.position.unwrap_or_default();
        w.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
        w.push(u32::from(self.clickety_clack_sound.position.is_some()));
        w.push(self.clickety_clack_sound.volume.map_or(0, f32::to_bits));
        w
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half: the base's (`setWakeFrame(getObject(), UPDATE_SLEEP_NONE)`).
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: version 3; from version 2 `PhysicsBehavior::xfer` and this class's fields (the track
    /// and `m_trackDataLoaded` are not saved); from version 3 `m_held`.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut version: XferVersion = 3;
        xfer.xfer_version(&mut version, 3)?;
        if version >= 2 {
            update_behavior::xfer_with(&mut self.base, update, xfer)?;
            xfer_user_i32(xfer, &mut self.next_station_task)?;
            xfer.xfer_object_id(&mut self.trailer_id)?;
            xfer.xfer_int(&mut self.current_point_handle)?;
            xfer.xfer_int(&mut self.wait_at_station_timer)?;
            xfer.xfer_bool(&mut self.carriages_created)?;
            xfer.xfer_bool(&mut self.has_ever_been_hitched)?;
            xfer.xfer_bool(&mut self.waiting_in_wings)?;
            xfer.xfer_bool(&mut self.end_of_line)?;
            xfer.xfer_bool(&mut self.is_locomotive)?;
            xfer.xfer_bool(&mut self.is_lead_carraige)?;
            xfer.xfer_int(&mut self.wants_to_be_lead_carraige)?;
            xfer.xfer_bool(&mut self.disembark)?;
            xfer.xfer_bool(&mut self.in_tunnel)?;
            xfer_user_i32(xfer, &mut self.conductor_state)?;
            let mut anchor = self.anchor_waypoint_id as i32;
            xfer_user_i32(xfer, &mut anchor)?;
            self.anchor_waypoint_id = anchor as u32;
            if !crate::logic::modules::plant(3779) {
                self.pull_info.xfer_pull_info(xfer)?;
            }
            self.conductor_pull_info.xfer_pull_info(xfer)?;
        }
        if version >= 3 {
            xfer.xfer_bool(&mut self.held)?;
        }
        Ok(())
    }

    /// `crc` with the scheduling half the slot holds: `UpdateModule::crc`.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds: the base's, then the track is to be read again and the three sounds are
    /// the data's again.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// `isRailroad()`: "I return that I am a railroad only if I am at one of the ends".
    fn is_railroad(&self) -> bool {
        if self.track.is_none() || self.waiting_in_wings || self.end_of_line {
            return false;
        }
        self.is_lead_carraige || self.trailer_id == INVALID_ID
    }

    /// `getObject()->kill()`: `Object::kill` reads `getBodyModule()->getMaxHealth()` unchecked.
    fn kill(world: &mut World, id: ObjectId) -> Result<(), Undefined> {
        if world.get_body_module(id).is_none() {
            return Err(Undefined::NullBodyModule);
        }
        world.kill(id, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
        Ok(())
    }

    /// The trailer's `RailroadBehavior`, if the trailer exists and has one (`findObjectByID(m_trailerID)`, `findUpdateModule(key)`).
    fn trailer_module(&self, world: &World) -> (bool, Option<ModuleHandle>) {
        if world.objects.find(self.trailer_id).is_none() {
            return (false, None);
        }
        let idx = world.find_module(self.trailer_id, b"RailroadBehavior");
        (true, idx.map(|index| ModuleHandle { object: self.trailer_id, index }))
    }

    /// `onCollide(other, loc, normal)`.
    fn on_collide(&mut self, world: &mut World, me: ModuleHandle, other: ObjectId, loc: Option<Coord3D>, normal: Option<Coord3D>) -> Result<(), Undefined> {
        let us = me.object;
        if world.objects.find(us).is_none() || world.objects.find(other).is_none() {
            return Ok(()); // sanity
        }
        let (Some(loc), Some(_normal)) = (loc, normal) else { return Ok(()) };
        if self.waiting_in_wings || self.end_of_line {
            return Ok(());
        }
        for idx in world.modules_with(other, MODULEINTERFACE_COLLIDE) {
            let railroad = if other == us && idx == me.index { self.is_railroad() } else { world.module_call(other, idx, ModuleCall::IsRailroad).as_bool() };
            if railroad {
                if self.is_locomotive {
                    // "yikes! I just rear ended a stranded train! Oh well..."
                    Self::kill(world, other)?;
                } else if self.is_lead_carraige {
                    // "yikes! I just coasted into some other carriage"
                    Self::kill(world, other)?;
                    Self::kill(world, us)?;
                }
                return Ok(()); // "its a train, folks"
            }
        }
        let is_kind = |w: &World, k| w.objects.find(other).is_some_and(|o| o.is_kind_of(k));
        if is_kind(world, KINDOF_STRUCTURE) {
            // "If it is a civilian building like a tunnel or a train station, let it be but if it is a faction building, kill it!"
            if [KINDOF_FS_POWER, KINDOF_FS_FACTORY, KINDOF_FS_BASE_DEFENSE, KINDOF_FS_TECHNOLOGY, KINDOF_REBUILD_HOLE].into_iter().any(|k| is_kind(world, k)) {
                let pos = world.objects.find(other).map(|o| o.get_position()).unwrap_or_default();
                self.play_impact_sound(world, other, pos)?;
                Self::kill(world, other)?;
                return Ok(());
            }
            if world.find_module(other, b"DemoTrapUpdate").is_some() {
                if !world.objects.find(other).is_some_and(|o| o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION)) {
                    // "it can only detonate on me if it is ready"
                    Self::kill(world, us)?;
                }
                let pos = world.objects.find(other).map(|o| o.get_position()).unwrap_or_default();
                self.play_impact_sound(world, other, pos)?;
                Self::kill(world, other)?;
                return Ok(());
            }
        }
        let Some(their_phys) = world.objects.find(other).and_then(|o| o.physics) else { return Ok(()) };
        // "maybe we don't want to trample this unit?"
        let d = self.data.clone();
        if self.conductor_state == WAIT_AT_STATION && self.pull_info.speed < d.running_garrison_speed_max {
            if let Some(ai) = world.objects.find(other).and_then(|o| o.ai) {
                if world.module_call(other, ai, ModuleCall::AiGetEnterTarget).as_id() == us {
                    return Ok(()); // "other intends to garrison me"
                }
            }
        }
        if world.objects.find(other).is_some_and(|o| o.contained_by == us) {
            return Ok(()); // "I dont mash my own sweet passengers"
        }
        let victim_is_infantry = is_kind(world, KINDOF_INFANTRY);
        let my_dir = world.objects.find_mut(us).map(|o| o.thing.unit_direction_vector_2d()).unwrap_or_default();
        let my_loc = world.objects.find(us).map(|o| o.get_position()).unwrap_or_default();
        let their_loc = |w: &World| w.objects.find(other).map(|o| o.get_position()).unwrap_or_default();
        let t = their_loc(world);
        let mut dlt = Coord3D { x: t.x - my_loc.x, y: t.y - my_loc.y, z: t.z - my_loc.z };
        // "Alert all the players of recent disaster"
        if !world.audio_is_currently_playing(self.whistle_sound.handle) {
            self.whistle_sound.handle = self.whistle_sound.add(world);
        }
        let dist = (dlt.x * dlt.x + dlt.y * dlt.y + dlt.z * dlt.z).sqrt();
        let us_radius = world.objects.find(us).map_or(0.0, |o| o.geometry_info.get_major_radius());
        let them_radius = world.objects.find(other).map_or(0.0, |o| o.geometry_info.get_major_radius());
        // "the plus 1 makes them go just outside of me."
        let mut overlap = ((us_radius + them_radius) - dist) + 1.0;
        dlt.normalize();
        if !victim_is_infantry {
            overlap /= 4.0;
            dlt.scale(overlap);
            let new_pos = Coord3D { x: t.x + dlt.x, y: t.y + dlt.y, z: t.z + dlt.z };
            world.set_position(other, &new_pos);
        }
        if self.conductor_state == WAIT_AT_STATION || (self.conductor_state == COAST && self.pull_info.speed < d.running_garrison_speed_max) || !self.is_locomotive {
            return Ok(()); // "let those trying to board pass through unhindered"
        }
        // "figure out the relative slope between them and me"
        let mut delta = their_loc(world);
        delta.sub(&my_loc);
        delta.normalize();
        let dot = delta.x * my_dir.x + delta.y * my_dir.y + delta.z * my_dir.z;
        let speed = self.pull_info.speed;
        if world.objects.find(other).is_some_and(|o| o.is_effectively_dead()) {
            // "we just run over debris, instead of shoving it around"
            delta.scale(min(0.3, speed * 0.66));
        } else {
            // "the faster I go, the harder I slam!"
            delta.scale(min(1.4, speed * 0.66));
            if speed >= d.kill_speed_min && !crate::logic::modules::plant(3772) {
                // "Absolute death to be hit by a train, no survival"
                Self::kill(world, other)?;
                let pitch = world.random.game_logic_random_value_real(-0.03, 0.03);
                world.module_call(other, their_phys, ModuleCall::PhysicsSetPitchRate(pitch));
                let roll = world.random.game_logic_random_value_real(-0.03, 0.03);
                world.module_call(other, their_phys, ModuleCall::PhysicsSetRollRate(roll));
            } else {
                self.play_impact_sound(world, other, loc)?;
                let mut info = DamageInfo::default();
                info.input.damage_type = DAMAGE_CRUSH;
                info.input.death_type = DEATH_CRUSHED;
                info.input.source_id = us;
                info.input.amount = speed * 10.0; // "hurt!"
                world.attempt_damage(other, &mut info);
            }
        }
        let mut heft = their_loc(world);
        // "lift them off the ground": `MAX(heft.z, getGroundHeight(x, y) + 2)`, the macro evaluating the height again when it picks it
        let lifted = world.terrain_get_ground_height(heft.x, heft.y) + 2.0;
        if heft.z.partial_cmp(&lifted) != Some(std::cmp::Ordering::Greater) {
            heft.z = world.terrain_get_ground_height(heft.x, heft.y) + 2.0;
        }
        world.set_position(other, &heft);
        // "for some fake heft"
        delta.z = world.random.game_logic_random_value_real(0.05, speed / 10.0);
        delta.scale(dot);
        // "This is a special check so that it wont hurl infantry clear across the map!"
        if !(victim_is_infantry && world.module_call(other, their_phys, ModuleCall::PhysicsGetVelocityMagnitude).as_real(0.0) > 5.0) {
            world.module_call(other, their_phys, ModuleCall::PhysicsAddVelocityTo(delta));
        }
        world.module_call(other, their_phys, ModuleCall::PhysicsSetAllowToFall(true));
        world.module_call(other, their_phys, ModuleCall::PhysicsSetAllowBouncing(true));
        world.module_call(other, their_phys, ModuleCall::PhysicsSetAllowAirborneFriction(true));
        let up = Coord3D { x: 0.0, y: 0.0, z: 1.0 };
        let cross = Coord3D::cross_product(&my_dir, &up);
        delta.normalize();
        let deviation_cog = cross.x * delta.x + cross.y * delta.y + cross.z * delta.z;
        if dot > 0.0 {
            world.module_call(other, their_phys, ModuleCall::PhysicsSetYawRate(deviation_cog * -0.06 * speed));
        }
        Ok(())
    }

    /// `playImpactSound(victim, impactPosition)`: the victim's physics' bounce sound, else the data's default for its kind, at a volume from
    /// its speed and mass.
    fn play_impact_sound(&mut self, world: &mut World, victim: ObjectId, impact_position: Coord3D) -> Result<(), Undefined> {
        let their_phys = world.objects.find(victim).and_then(|o| o.physics);
        let mut name: Vec<u8> = Vec::new();
        let mut has_bounce_sound = false;
        if let Some(p) = their_phys {
            // (`getBounceSound()` is never `NULL`: the audio manager's silent event when the physics has no sound)
            if let ModuleReply::Sound(Some(s)) = world.module_call(victim, p, ModuleCall::PhysicsGetBounceSound) {
                name = s; // "copy theirs"
            }
            has_bounce_sound = !crate::logic::modules::plant(3775);
        }
        if !has_bounce_sound {
            let d = &self.data;
            let is_kind = |k| world.objects.find(victim).is_some_and(|o| o.is_kind_of(k));
            if is_kind(KINDOF_INFANTRY) {
                name = d.meaty_impact_default_sound.clone();
            } else if is_kind(KINDOF_HUGE_VEHICLE) || is_kind(KINDOF_STRUCTURE) {
                name = d.big_metal_impact_default_sound.clone();
            } else if is_kind(KINDOF_VEHICLE) {
                name = d.small_metal_impact_default_sound.clone();
            }
        }
        if name.is_empty() {
            return Ok(());
        }
        let mut vel = NORMAL_VEL_Z; // "the max"
        let mut mass = NORMAL_MASS; // "the max"
        let mut position = impact_position;
        if let Some(p) = their_phys {
            let v = world.module_call(victim, p, ModuleCall::PhysicsGetVelocity);
            let len = match v {
                ModuleReply::Coord(c) => c.length(),
                _ => 0.0,
            };
            vel = (f64::from(vel) + f64::from(len).abs()) as f32;
            let m = world.module_call(victim, p, ModuleCall::PhysicsGetMass).as_real(0.0);
            mass = (f64::from(mass) + f64::from(m).abs()) as f32;
            vel /= 2.0;
            mass /= 2.0; // "average of him and me"
            position = world.objects.find(victim).map(|o| o.get_position()).unwrap_or_default();
        }
        vel = min(NORMAL_VEL_Z, max_zero(vel));
        mass = min(NORMAL_MASS, max_zero(mass));
        let mut vol_adjust = normalize_to_range(mu_law(vel, NORMAL_VEL_Z, 500.0), -1.0, 1.0, 0.25, 1.0);
        vol_adjust *= normalize_to_range(mu_law(mass, NORMAL_MASS, 500.0), -1.0, 1.0, 0.25, 1.0);
        // "set the player index of the sound to be the player index of the victim object"
        let Some(player) = world.get_controlling_player(victim) else { return Err(Undefined::NullControllingPlayer) };
        world.audio_add_event_full(name, INVALID_ID, Some(position), Some(player), Some(vol_adjust));
        Ok(())
    }

    /// `loadTrackData()`: the track from the waypoint nearest the object (or the saved anchor), once.
    fn load_track_data(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        if self.track.is_some() {
            return Ok(()); // "lets do this only once!"
        }
        let my_pos = world.objects.find(us).map(|o| o.get_position()).unwrap_or_default();
        let mut closest_distance = 99999.9f32;
        let terrain = &world.terrain;
        let mut anchor = None;
        if self.anchor_waypoint_id == INVALID_WAYPOINT_ID {
            for h in terrain.waypoint_handles() {
                let w = terrain.waypoint(h).expect("a listed waypoint");
                let l = w.location();
                let delta = Coord3D { x: my_pos.x - l.x, y: my_pos.y - l.y, z: my_pos.z - l.z };
                if closest_distance > delta.length() {
                    closest_distance = delta.length();
                    anchor = Some(h);
                    self.anchor_waypoint_id = w.id() as u32; // "save for XFER, later"
                }
            }
        } else {
            anchor = terrain.waypoint_by_id(self.anchor_waypoint_id);
        }
        let Some(anchor) = anchor else { return Ok(()) };
        let mut track = TrainTrack::default();
        let wp = |h| terrain.waypoint(h).expect("a waypoint");
        let a = wp(anchor);
        track.points.push(TrackPoint {
            distance_from_prev: 0.0,
            distance_from_first: 0.0,
            is_first_point: true,
            is_last_point: false,
            is_tunnel_or_bridge: a.name().ends_with(b"Tunnel"),
            is_station: a.name().ends_with(b"Station"),
            is_disembark: a.name().ends_with(b"Disembark"),
            is_ping_pong: false,
            position: *a.location(),
            handle: a.id(),
        });
        let mut scanner = anchor;
        let mut steps = 0usize;
        loop {
            let s = wp(scanner);
            if s.num_links() == 0 {
                break; // "since we have reached a non-linked waypoint"
            }
            let another = s.link(0).map_err(|_| Undefined::ArrayIndexOutOfRange)?;
            let Some(another) = another else { break };
            let an = wp(another);
            let (from_pos, to_pos) = (*s.location(), *an.location());
            let from_to_delta = Coord3D { x: from_pos.x - to_pos.x, y: from_pos.y - to_pos.y, z: from_pos.z - to_pos.z };
            let dist_from_to = from_to_delta.length();
            track.length += dist_from_to;
            track.points.push(TrackPoint {
                distance_from_prev: dist_from_to,
                distance_from_first: track.length,
                is_first_point: false,
                is_last_point: an.link(0).map_err(|_| Undefined::ArrayIndexOutOfRange)?.is_none(),
                is_tunnel_or_bridge: an.name().ends_with(b"Tunnel"),
                is_station: an.name().ends_with(b"Station"),
                is_ping_pong: s.name().ends_with(b"PingPong"),
                is_disembark: s.name().ends_with(b"Disembark"),
                position: *an.location(),
                handle: s.id(),
            });
            scanner = another;
            if scanner == anchor {
                track.is_looping = true; // "it must be a looping track. Cool."
                break;
            }
            steps += 1;
            if steps > terrain.waypoint_handles().len() {
                // a path that cycles without the anchor: upstream's loop never ends
                return Err(Undefined::EndlessLoop);
            }
        }
        self.track = Some(Arc::new(track));
        Ok(())
    }

    /// `makeAWallOutOfThisTrain(on)`: the pathfinder's wall on this car's footprint, then down the train.
    fn make_a_wall_out_of_this_train(&mut self, world: &mut World, us: ObjectId, on: bool) {
        if on {
            world.pathfinder_create_wall_from_footprint(us);
        } else {
            world.pathfinder_remove_wall_from_footprint(us);
        }
        if self.trailer_id != INVALID_ID {
            if let (_, Some(h)) = self.trailer_module(world) {
                world.module_call(h.object, h.index, ModuleCall::RailroadMakeAWallOutOfThisTrain(on));
            }
        }
    }

    /// `update()`.
    fn run_update(&mut self, world: &mut World, me: ModuleHandle) -> Result<UpdateSleepTime, Undefined> {
        let us = me.object;
        // "load the waypoint data if not loaded"
        if !self.track_data_loaded && self.is_locomotive {
            self.load_track_data(world, us)?;
            if self.track.is_some() {
                self.create_carriages(world, us)?;
            }
            self.track_data_loaded = true;
        }
        let Some(track) = self.track.clone() else { return Ok(UPDATE_SLEEP_NONE) };
        let d = self.data.clone();
        if self.is_locomotive {
            if self.conductor_state == APPLY_BRAKES {
                self.conductor_pull_info.speed *= d.braking;
                if self.conductor_pull_info.speed.abs() < 0.1 {
                    self.conductor_pull_info.speed = 0.0;
                    self.wait_at_station_timer = d.wait_at_station_time as i32;
                    self.conductor_state = WAIT_AT_STATION;
                    self.make_a_wall_out_of_this_train(world, us, true);
                    if self.disembark {
                        self.do_disembark(world, us);
                        self.disembark = false;
                    }
                }
            } else if self.conductor_state == WAIT_AT_STATION {
                self.wait_at_station_timer = self.wait_at_station_timer.checked_sub(1).ok_or(Undefined::SignedIntOverflow)?;
                if self.wait_at_station_timer <= 0 && !self.held {
                    self.conductor_state = ACCELERATE;
                    self.conductor_pull_info.speed = 0.05 * self.conductor_pull_info.direction;
                    self.running_sound.handle = self.running_sound.add(world);
                    self.make_a_wall_out_of_this_train(world, us, false);
                } else if self.wait_at_station_timer as u32 == d.wait_at_station_time / 4 {
                    self.whistle_sound.handle = self.whistle_sound.add(world);
                }
            } else if self.conductor_state == ACCELERATE {
                // "push start multiplier"
                self.conductor_pull_info.speed += 0.02 * self.conductor_pull_info.direction;
                self.conductor_pull_info.speed *= d.acceleration;
                if self.conductor_pull_info.speed > d.speed_max {
                    self.conductor_pull_info.speed = d.speed_max;
                } else if self.conductor_pull_info.speed < -d.speed_max {
                    self.conductor_pull_info.speed = -d.speed_max;
                }
                if !world.audio_is_currently_playing(self.running_sound.handle) {
                    self.running_sound.handle = self.running_sound.add(world);
                }
            }
        }
        if self.wants_to_be_lead_carraige > FRAMES_UNPULLED_LONG_ENOUGH_TO_UNHITCH && !crate::logic::modules::plant(3771) {
            // "if this flag survived until now, I have lost my puller"
            self.is_lead_carraige = true;
        }
        if self.is_lead_carraige {
            if self.conductor_state == COAST {
                self.conductor_pull_info.speed *= d.friction;
                world.audio_remove_event(self.running_sound.handle);
            }
            self.conductor_pull_info.track_distance += self.conductor_pull_info.speed;
            // "only normalize track position for a looping track, otherwise, let train exit by exceeding tracklength"
            if track.is_looping {
                self.conductor_pull_info.track_distance = wrap_down(self.conductor_pull_info.track_distance, track.length)?;
                self.conductor_pull_info.track_distance = wrap_up(self.conductor_pull_info.track_distance, track.length)?;
            }
            let td = self.conductor_pull_info.track_distance;
            if let Some(p) = self.find_pos_by_path_distance(world, us, td, track.length, false)? {
                self.conductor_pull_info.tow_hitch_position = p;
            }
            // "let the conductor pull "me" while reseting my info, then..."
            let conductor = self.conductor_pull_info;
            self.update_position_track_distance(world, us, &conductor)?;
            self.pull_trailer(world, us);
        } else if self.wants_to_be_lead_carraige <= FRAMES_UNPULLED_LONG_ENOUGH_TO_UNHITCH {
            // "like every young carriage, I aspire to be the lead carriage some day"
            self.wants_to_be_lead_carraige += 1;
        }
        if world.objects.find(us).is_some_and(|o| o.drawable != crate::common::game_type::INVALID_DRAWABLE_ID) {
            if !track.is_looping {
                world.emit(ObjectClientEvent::DrawableSetHidden { object_id: us, hidden: self.waiting_in_wings || self.end_of_line });
            }
            // "TURN OFF SMOKE EFFECTS IF WE ARE IN A TUNNEL"
            let draw_pos = world.objects.find(us).map(|o| o.get_position()).unwrap_or_default();
            let op = if draw_pos.z < world.terrain_get_ground_height(draw_pos.x, draw_pos.y) - 3.0 {
                ModelConditionOp::Set(vec![MODELCONDITION_OVER_WATER])
            } else {
                ModelConditionOp::Clear(vec![MODELCONDITION_OVER_WATER])
            };
            world.emit(ObjectClientEvent::DrawableModelCondition { object_id: us, op });
        }
        Ok(UPDATE_SLEEP_NONE)
    }

    /// The end of `update()` and `getPulled()`: the trailer pulled with this car's info, or forgotten ("designate myself as the caboose"),
    /// when this car leaves the end of the line.
    fn pull_trailer(&mut self, world: &mut World, us: ObjectId) {
        match self.trailer_module(world) {
            (true, Some(h)) => {
                if world.module_call(h.object, h.index, ModuleCall::RailroadGetPulled(self.pull_info)).as_bool() {
                    self.pull_info.previous_waypoint = self.pull_info.current_waypoint;
                }
            }
            (true, None) => {}
            (false, _) => {
                self.trailer_id = INVALID_ID;
                if self.end_of_line {
                    world.body_destroy_object(us);
                }
            }
        }
    }

    /// `disembark()`: every passenger out, down the train.
    fn do_disembark(&mut self, world: &mut World, us: ObjectId) {
        if let Some(contain) = world.get_contain(us) {
            world.module_call(us, contain, ModuleCall::OrderAllPassengersToExit { command_source: CMD_FROM_AI, instantly: false });
        }
        if let (_, Some(h)) = self.trailer_module(world) {
            world.module_call(h.object, h.index, ModuleCall::RailroadDisembark);
        }
    }

    /// `PartitionFilterIsValidCarriage::allow(other)`: another car with a `RailroadBehavior` nobody has hitched, an ally.
    fn is_valid_carriage(world: &mut World, us: ObjectId, other: ObjectId) -> bool {
        if world.objects.find(us).is_none() || world.objects.find(other).is_none() || us == other {
            return false;
        }
        let Some(idx) = world.find_module(other, b"RailroadBehavior") else { return false };
        if world.module_call(other, idx, ModuleCall::RailroadHasEverBeenHitched).as_bool() {
            return false; // "two of us can't pull you"
        }
        world.get_relationship(us, other) == Relationship::Allies
    }

    /// The car behind this one the map placed (`ThePartitionManager->getClosestObject(&myHitchLoc, maxRadius, FROM_CENTER_2D, { valid
    /// carriage })`), or the trailer it already has (after a load).
    fn close_carriage(&mut self, world: &mut World, us: ObjectId) -> Option<ObjectId> {
        let (pos, dir, radius) = {
            let o = world.objects.find_mut(us).expect("the module's object");
            (o.get_position(), o.thing.unit_direction_vector_2d(), o.geometry_info.get_major_radius())
        };
        let max_radius = radius * 2.0;
        let mut my_hitch_loc = pos;
        let mut hitch_offset = dir;
        hitch_offset.scale(-max_radius); // "negative, since I want the back, not the front"
        my_hitch_loc.add(&hitch_offset);
        if self.trailer_id != INVALID_ID && world.objects.find(self.trailer_id).is_some() {
            return Some(self.trailer_id);
        }
        let candidates = world.partition_closest_candidates_at(my_hitch_loc, max_radius, FROM_CENTER_2D);
        candidates.into_iter().find(|&c| Self::is_valid_carriage(world, us, c))
    }

    /// `createCarriages()`: the locomotive hitches the car behind it, or the first of its template list.
    fn create_carriages(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        if !self.is_locomotive {
            return Ok(()); // "a not locomotive attempted to create carriages"
        }
        let close_carriage = self.close_carriage(world, us);
        let list = self.data.carriage_template_name_data.clone();
        if let Some(first) = list.first() {
            if let Some(temp) = world.thing_factory_find_template(first) {
                let first_carriage = match close_carriage {
                    Some(c) => c,
                    None => {
                        // "or else let's use the defualt template list prvided in the INI"
                        let team = world.objects.find(us).map(|o| o.team).filter(|t| *t != 0);
                        world.thing_factory_new_object(temp, team, Default::default()).map_err(|_| Undefined::NewObjectFailed)?
                    }
                };
                world.set_producer(first_carriage, us);
                self.trailer_id = first_carriage;
                if let Some(idx) = world.find_module(first_carriage, b"RailroadBehavior") {
                    let track = self.track.clone().expect("the caller checked the track");
                    let call = match close_carriage {
                        Some(_) => ModuleCall::RailroadHitchByProximity { locomotive: us, track },
                        None => ModuleCall::RailroadHitchByTemplate { locomotive: us, names: list.clone(), next: 1, track },
                    };
                    world.module_call(first_carriage, idx, call);
                }
            }
        }
        self.carriages_created = true;
        Ok(())
    }

    /// The start of both hitches: whether this car is hitched (not a locomotive, "You can not hitch a locomotive in mid train, dude.", and
    /// the locomotive exists), when it takes the train's track and "so no other trains try to hitch me onto them" is set. (Done on the module
    /// before it is lent to its slot for the rest, so that the next car's filter, which asks this one, sees it hitched.)
    fn hitch_prelude(&mut self, world: &World, locomotive: ObjectId, track: &Arc<TrainTrack>) -> bool {
        if self.is_locomotive || world.objects.find(locomotive).is_none() {
            return false;
        }
        self.track = Some(track.clone()); // "ALWAYS INCREFERENCE WHENEVER YOU INHERIT THIS POINTER"
        self.has_ever_been_hitched = true;
        true
    }

    /// `hitchNewCarriagebyTemplate(locoID, list, iter, track)` after [`Self::hitch_prelude`]: makes and hitches the next car of the list
    /// (`names[next]`).
    fn hitch_by_template(&mut self, world: &mut World, locomotive: ObjectId, names: Vec<Vec<u8>>, next: usize, track: Arc<TrainTrack>) -> Result<(), Undefined> {
        if let Some(name) = names.get(next) {
            if let Some(temp) = world.thing_factory_find_template(name) {
                let team = world.objects.find(locomotive).map(|o| o.team).filter(|t| *t != 0);
                let new_carriage = world.thing_factory_new_object(temp, team, Default::default()).map_err(|_| Undefined::NewObjectFailed)?;
                if !crate::logic::modules::plant(3778) {
                    world.set_producer(new_carriage, locomotive);
                }
                self.trailer_id = new_carriage;
                if let Some(idx) = world.find_module(new_carriage, b"RailroadBehavior") {
                    world.module_call(new_carriage, idx, ModuleCall::RailroadHitchByTemplate { locomotive, names, next: next + 1, track });
                }
            }
        }
        Ok(())
    }

    /// `hitchNewCarriagebyProximity(locoID, track)` after [`Self::hitch_prelude`]: hitches the car behind this one.
    fn hitch_by_proximity(&mut self, world: &mut World, us: ObjectId, track: Arc<TrainTrack>) {
        if let Some(close) = self.close_carriage(world, us) {
            world.set_producer(close, us);
            self.trailer_id = close;
            if let Some(idx) = world.find_module(close, b"RailroadBehavior") {
                world.module_call(close, idx, ModuleCall::RailroadHitchByProximity { locomotive: us, track });
            }
        }
    }

    /// `getPulled(info)`: this car follows its puller (and pulls its own trailer); whether it read `info` (it has a track), when the
    /// puller's `previousWaypoint` becomes its `currentWaypoint`.
    fn get_pulled(&mut self, world: &mut World, us: ObjectId, info: PullInfo) -> Result<bool, Undefined> {
        // "ENFORCE MY STATUS AS A PULLEE, NOT A PULLER"
        self.wants_to_be_lead_carraige = 0;
        if self.track.is_none() {
            return Ok(false);
        }
        // "copy my puller to my conductor, so that if I ever get detached ... I will take over as the lead carriage!"
        self.conductor_pull_info = info;
        let mut info = info;
        info.previous_waypoint = info.current_waypoint;
        self.update_position_track_distance(world, us, &info)?;
        self.pull_trailer(world, us);
        Ok(true)
    }

    /// `destroyTheWholeTrainNow()`.
    fn destroy_the_whole_train_now(&mut self, world: &mut World, us: ObjectId) {
        world.body_destroy_object(us);
        if let (_, Some(h)) = self.trailer_module(world) {
            world.module_call(h.object, h.index, ModuleCall::RailroadDestroyTheWholeTrainNow);
        }
    }

    /// `updatePositionTrackDistance(pullerInfo, &m_pullInfo)`: this car a hitch behind its puller on the track, turned towards the puller's
    /// tow hitch and set on the ground (unless in a tunnel).
    fn update_position_track_distance(&mut self, world: &mut World, us: ObjectId, puller: &PullInfo) -> Result<(), Undefined> {
        let Some(track) = self.track.clone() else { return Ok(()) };
        if world.objects.find(us).is_none() {
            return Ok(()); // sanity
        }
        // "IF THIS HAS BEEN CALLED, AM AM GETTING PULLED BY SOMETHING, my conductor, or another car"
        let hitch_radius = world.objects.find(us).map_or(0.0, |o| o.geometry_info.get_major_radius());
        self.pull_info.track_distance = puller.track_distance - (hitch_radius * 2.0);
        self.pull_info.speed = puller.speed;
        self.pull_info.direction = puller.direction;
        let td = self.pull_info.track_distance;
        if let Some(p) = self.find_pos_by_path_distance(world, us, td, track.length, false)? {
            self.pull_info.tow_hitch_position = p;
        }
        let car_position = self.find_pos_by_path_distance(world, us, td, track.length, true)?.unwrap_or_default();
        let (mut turn_pos, dir, z_rotation, obj_mtx) = {
            let o = world.objects.find_mut(us).expect("checked above");
            let dir = o.thing.unit_direction_vector_2d();
            (o.get_position(), dir, o.thing.transform_matrix().get_z_rotation(), *o.thing.transform_matrix())
        };
        if !self.in_tunnel {
            turn_pos.z = world.terrain_get_ground_height(turn_pos.x, turn_pos.y);
        }
        turn_pos.x += dir.x * -hitch_radius;
        turn_pos.y += dir.y * -hitch_radius;
        let track_pos_delta = Coord3D { x: car_position.x - turn_pos.x, y: car_position.y - turn_pos.y, z: 0.0 };
        let dx = puller.tow_hitch_position.x - turn_pos.x;
        let dy = puller.tow_hitch_position.y - turn_pos.y;
        // (`atan2` on floats: the double function)
        let desired_angle = f64::from(dy).atan2(f64::from(dx)) as f32;
        let rel_angle = std_angle_diff(desired_angle, z_rotation);
        let mut tmp = Matrix3D::IDENTITY;
        tmp.translate(turn_pos.x, turn_pos.y, 0.0);
        tmp.translate(track_pos_delta.x, track_pos_delta.y, 0.0);
        if !crate::logic::modules::plant(3774) {
            tmp.in_place_pre_rotate_z(rel_angle);
        }
        tmp.translate(-turn_pos.x, -turn_pos.y, 0.0);
        let mtx = Matrix3D::multiply(&tmp, &obj_mtx);
        // "enforce ground elevation"
        let (enforce_elevation, _normal) = world.terrain_get_ground_height_and_normal(turn_pos.x, turn_pos.y);
        world.set_transform_matrix(us, &mtx);
        if !self.in_tunnel {
            world.set_position_z(us, enforce_elevation);
        }
        world.handle_partition_cell_maintenance(us);
        Ok(())
    }

    /// `FindPosByPathDistance(&pos, dist, length, setState)`: the point `dist` along the track (`None`: `pos` left at the origin, as the
    /// function leaves it when no segment holds the distance). With `setState`, the locomotive watches the track's joints for stations,
    /// disembark points and ping-pong points, and every car plays its clickety-clack at a joint outside a tunnel.
    fn find_pos_by_path_distance(&mut self, world: &mut World, us: ObjectId, dist: f32, length: f32, set_state: bool) -> Result<Option<Coord3D>, Undefined> {
        let Some(track) = self.track.clone() else { return Ok(None) };
        self.waiting_in_wings = false;
        let mut actual_distance = dist; // "WRAP TO NORMALIZE"
        if track.is_looping {
            actual_distance = wrap_up(actual_distance, length)?;
            actual_distance = wrap_down(actual_distance, length)?;
        } else {
            if dist < 0.0 {
                self.waiting_in_wings = true;
            } else if dist >= length && !crate::logic::modules::plant(3773) {
                self.end_of_line = true;
            }
            // "CLAMP TO NORMALIZE": MAX(MIN(length, dist), 0)
            actual_distance = max_zero(min(length, dist));
        }
        let mut pos = Coord3D::default();
        let points = &track.points;
        for (i, this_point) in points.iter().enumerate() {
            if this_point.distance_from_first < actual_distance {
                let mut this_point_pos = this_point.position;
                let Some(next_point) = points.get(i + 1) else {
                    // the point after the last is the list's end node, read as a point
                    return Err(Undefined::EndIteratorDereferenced);
                };
                if next_point.distance_from_first > actual_distance {
                    // "I am between this point and the next"
                    let handle_found = this_point.handle;
                    let edge = self.current_point_handle != handle_found;
                    if set_state {
                        self.in_tunnel = this_point.is_tunnel_or_bridge;
                        if self.is_locomotive && edge {
                            // "THE LOCOMOTIVE SNIFFS THE TRACK TO SMELL FOR THE NEXT STATION AND TUNNEL"
                            self.current_point_handle = handle_found;
                            if this_point.is_station {
                                self.conductor_state = APPLY_BRAKES;
                                self.disembark = false;
                                world.audio_remove_event(self.running_sound.handle);
                            } else if this_point.is_disembark {
                                self.conductor_state = APPLY_BRAKES;
                                self.disembark = !crate::logic::modules::plant(3776);
                                world.audio_remove_event(self.running_sound.handle);
                            } else if this_point.is_ping_pong && self.conductor_pull_info.most_recent_special_point_handle != handle_found {
                                self.conductor_pull_info.most_recent_special_point_handle = handle_found;
                                self.conductor_state = APPLY_BRAKES;
                                self.disembark = false;
                                world.audio_remove_event(self.running_sound.handle);
                                if !crate::logic::modules::plant(3777) {
                                    self.conductor_pull_info.direction = -self.conductor_pull_info.direction;
                                }
                            }
                        }
                        if edge && !self.in_tunnel {
                            // "play my clickety clack sound, `cause I just rode over a join IN the tracks"
                            self.clickety_clack_sound.add(world);
                            self.clickety_clack_sound.position = world.objects.find(us).map(|o| o.get_position());
                            // "assumed max speed"
                            self.clickety_clack_sound.volume = Some(self.conductor_pull_info.speed / 10.0);
                        }
                    }
                    let difference = actual_distance - this_point.distance_from_first;
                    let mut delta = next_point.position;
                    delta.sub(&this_point_pos);
                    delta.normalize();
                    delta.scale(difference);
                    this_point_pos.add(&delta);
                    return Ok(Some(this_point_pos)); // "copy out"
                }
                pos = this_point_pos; // "copy out"
            }
        }
        Ok(Some(pos))
    }
}

/// `MIN(a, b)`: `((a) < (b) ? (a) : (b))`.
fn min(a: f32, b: f32) -> f32 {
    if a < b {
        a
    } else {
        b
    }
}

/// `MAX(0, x)`: `0 > x ? 0 : x`.
fn max_zero(x: f32) -> f32 {
    if 0.0 > x {
        0.0
    } else {
        x
    }
}

/// `while (d > length) d -= length;`: `EndlessLoop` when a step does not lower `d` (a track of no length).
fn wrap_down(mut d: f32, length: f32) -> Result<f32, Undefined> {
    while d > length {
        let n = d - length;
        if n.partial_cmp(&d) != Some(std::cmp::Ordering::Less) {
            return Err(Undefined::EndlessLoop);
        }
        d = n;
    }
    Ok(d)
}

/// `while (d < 0.0f) d += length;`: `EndlessLoop` when a step does not raise `d`.
fn wrap_up(mut d: f32, length: f32) -> Result<f32, Undefined> {
    while d < 0.0 {
        let n = d + length;
        if n.partial_cmp(&d) != Some(std::cmp::Ordering::Greater) {
            return Err(Undefined::EndlessLoop);
        }
        d = n;
    }
    Ok(d)
}

/// `xferUser(&e, sizeof(enum))`: an enum's four bytes.
fn xfer_user_i32(xfer: &mut dyn Xfer, v: &mut i32) -> XferResult<()> {
    let mut b = v.to_le_bytes();
    xfer.xfer_user(&mut b)?;
    if xfer.get_xfer_mode() == XferMode::Load {
        *v = i32::from_le_bytes(b);
    }
    Ok(())
}

impl UpdateBehavior for RailroadBehavior {
    const XFER_VERSION: XferVersion = 3;
    fn update_base(&self) -> &UpdateModule {
        self.base.update_base()
    }
    /// `update()`: this class's (the base's integration does not run).
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        match self.run_update(world, me) {
            Ok(sleep) => sleep,
            Err(u) => {
                world.cuts.module_errors.push((me, IniError::undefined(u)));
                UPDATE_SLEEP_NONE
            }
        }
    }
    fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.base.construct(world, me);
    }
    /// `PhysicsBehavior::loadPostProcess`, then "have I TRIED to load track data, yet?" no, and the sounds the data's again.
    fn load_post_process_fields(&mut self) {
        self.base.load_post_process_fields();
        self.track_data_loaded = false;
        let us = self.base.update_module().behavior().object();
        let d = self.data.clone();
        self.running_sound = Sound::of(&d.running_sound, us);
        self.clickety_clack_sound = Sound::of(&d.clickety_clack_sound, us);
        self.whistle_sound = Sound::of(&d.whistle_sound, us);
    }
    /// The collision, `isRailroad` and the train's calls are this class's; the physics interface's other calls are the base's.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        let us = me.object;
        let lend = |world: &mut World, m: &mut Self, f: &mut dyn FnMut(&mut World, &mut Self) -> Result<ModuleReply, Undefined>| {
            let r = crate::logic::modules::lend_clone(world, me, m, UpdateBody::RailroadBehavior, Self::from_body, |w, m| f(w, m));
            match r {
                Ok(reply) => reply,
                Err(u) => {
                    world.cuts.module_errors.push((me, IniError::undefined(u)));
                    ModuleReply::Done
                }
            }
        };
        match call {
            ModuleCall::OnCollide { other, loc, normal } => lend(world, self, &mut |w, m| m.on_collide(w, me, other, loc, normal).map(|_| ModuleReply::Done)),
            ModuleCall::IsRailroad => ModuleReply::Bool(self.is_railroad()),
            ModuleCall::RailroadGetPulled(info) => lend(world, self, &mut |w, m| m.get_pulled(w, us, info).map(ModuleReply::Bool)),
            ModuleCall::RailroadHitchByTemplate { locomotive, ref names, next, ref track } => {
                if !self.hitch_prelude(world, locomotive, track) {
                    return ModuleReply::Done;
                }
                let (names, track) = (names.clone(), track.clone());
                lend(world, self, &mut |w, m| m.hitch_by_template(w, locomotive, names.clone(), next, track.clone()).map(|_| ModuleReply::Done))
            }
            ModuleCall::RailroadHitchByProximity { locomotive, ref track } => {
                if !self.hitch_prelude(world, locomotive, track) {
                    return ModuleReply::Done;
                }
                let track = track.clone();
                lend(world, self, &mut |w, m| {
                    m.hitch_by_proximity(w, us, track.clone());
                    Ok(ModuleReply::Done)
                })
            }
            ModuleCall::RailroadDisembark => {
                self.do_disembark(world, us);
                ModuleReply::Done
            }
            ModuleCall::RailroadMakeAWallOutOfThisTrain(on) => {
                self.make_a_wall_out_of_this_train(world, us, on);
                ModuleReply::Done
            }
            ModuleCall::RailroadDestroyTheWholeTrainNow => lend(world, self, &mut |w, m| {
                m.destroy_the_whole_train_now(w, us);
                Ok(ModuleReply::Done)
            }),
            ModuleCall::RailroadHasEverBeenHitched => ModuleReply::Bool(self.has_ever_been_hitched),
            ModuleCall::RailroadSetHeld(held) => {
                self.held = held;
                ModuleReply::Done
            }
            _ => self.base.other_call(world, me, call),
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::RailroadBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::RailroadBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for RailroadBehavior {
    /// `crc`: `UpdateModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.base.crc(xfer)
    }
    /// `xfer`: see [`RailroadBehavior::xfer_with`].
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut update = self.base.update_module().clone();
        let r = self.xfer_with(&mut update, xfer);
        *self.base.update_module_mut() = update;
        r
    }
    fn load_post_process(&mut self) {
        let mut update = self.base.update_module().clone();
        self.load_post_process_with(&mut update);
        *self.base.update_module_mut() = update;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = RailroadBehaviorModuleData::default();
        assert_eq!((d.speed_max, d.acceleration, d.braking, d.friction, d.wait_at_station_time), (4.0, 1.01, 0.99, 0.97, 150));
        let m = RailroadBehavior::unconstructed(1, Some(Arc::new(RailroadBehaviorModuleData { is_locomotive: true, ..d }))).unwrap();
        assert!(m.is_lead_carraige && m.waiting_in_wings && m.conductor_state == ACCELERATE && !m.is_railroad());
        assert_eq!(m.pull_info.current_waypoint, 0xfacade);
    }

    #[test]
    fn the_impact_volume_is_the_mu_law_of_speed_and_mass() {
        // no physics: the maxima, a volume of 1
        assert_eq!(normalize_to_range(mu_law(NORMAL_VEL_Z, NORMAL_VEL_Z, 500.0), -1.0, 1.0, 0.25, 1.0), 1.0);
        // at half, the law's zero: the middle of the range
        assert_eq!(normalize_to_range(mu_law(0.125, NORMAL_VEL_Z, 500.0), -1.0, 1.0, 0.25, 1.0), 0.625);
    }

    #[test]
    fn a_looping_track_of_no_length_never_wraps() {
        assert_eq!(wrap_down(3.0, 0.0), Err(Undefined::EndlessLoop));
        assert_eq!(wrap_up(-1.0, 10.0), Ok(9.0));
        assert_eq!(wrap_down(25.0, 10.0), Ok(5.0));
    }
}
