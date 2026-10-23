// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/GarrisonContain.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/GarrisonContain.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `GarrisonContain`: a building infantry can garrison. The riders that have a target are moved to the garrison point (a `FIREPOINT` bone of the art,
//! for the building's damage state) closest to it, with a muzzle-flash drawable (`GarrisonGun`) there; a non-enclosing garrison (a fire base) keeps its
//! riders at fixed `STATION` bones instead. The building takes its riders' side (and hides it while every rider is an undetected stealth unit), heals
//! them if the data says so, and lets them out by a door to the left, the right or from the centre.
//!
//! # Design
//!
//! A garrison point's object and effect are a union upstream with the ids a load reads, until `loadPostProcess` finds them again ([`PointRef`]). A
//! load's original team is checked against the team factory after the xfer ([`GarrisonContain::check_loaded_team`], called by
//! `World::xfer_module_slot`); upstream throws in the middle of the xfer, so the fields after the team are put back as they were when the check fails.
//! The muzzle flashes are client drawables: [`World::contain_effect_new`] and the other `contain_effect_*` cut points.
//!
//! # Upstream facts kept
//!
//! - `findConditionIndex` reads the body unchecked; an unknown damage state indexes the points with -1: both are undefined.
//! - `removeInvalidObjectsFromGarrisonPoints` reads the rider's AI unchecked (a rider at a point has one unless a load put it there).
//! - `loadStationGarrisonPoints` reads the drawable unchecked (`loadGarrisonPoints` only runs with one).
//! - `trackTargets` orients the effect of the point the rider was at, after it may have moved: a moved rider's old point has no effect any more, and
//!   the direction is from its new place.
//! - `recalcApparentControllingPlayer` reads the first of the module's own list when the count is not 0.
//! - `exitObjectViaDoor`'s locomotor test tries the door, then the far side, then gives up.
//! - The evacuation side, the station points and their flag are not saved.
//! - `onObjectCreated` makes the initial roster with the controlling player's default team, unchecked; a roster name that is no template throws
//!   `ERROR_BAD_ARG` (recorded).
//!
//! `p1w13-sonnet`, batch R12, 2026-10-23.

use std::sync::Arc;

use crate::common::base_type::{Coord2D, Coord3D};
use crate::common::disabled_types::{DISABLED_HELD, DISABLED_SUBDUED};
use crate::common::game_common::{Relationship, LOGICFRAMES_PER_SECOND};
use crate::common::game_type::{ObjectId, INVALID_DRAWABLE_ID, INVALID_ID};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::{KindOfMaskType, KINDOF_GARRISONABLE_UNTIL_DESTROYED, KINDOF_INFANTRY, KINDOF_NO_GARRISON};
use crate::common::model_state::{MODELCONDITION_DAMAGED, MODELCONDITION_FIRING_A, MODELCONDITION_GARRISONED, MODELCONDITION_REALLY_DAMAGED, MODELCONDITION_RUBBLE, MODELCONDITION_SPECIAL_DAMAGED};
use crate::common::module::ModuleDataKind;
use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_CAN_ATTACK, OBJECT_STATUS_DETECTED, OBJECT_STATUS_IS_ATTACKING};
use crate::common::trig;
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_HEALING, DAMAGE_POISON, DEATH_NONE};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::body_module::{BODY_DAMAGED, BODY_PRISTINE, BODY_REALLYDAMAGED, BODY_RUBBLE};
use crate::logic::module::contain_module::EvacDisposition;
use crate::logic::module::update_module::{UpdateModule, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::open_contain::{self, out, OpenContain, OpenContainModuleData, OpenContainVirtuals, CMD_FROM_AI};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::{PlayerIndex, TeamId};
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

/// `TEAM_ID_INVALID`.
const TEAM_ID_INVALID: TeamId = 0;
/// `MAX_GARRISON_POINTS`.
pub const MAX_GARRISON_POINTS: usize = 40;
/// `MAX_GARRISON_POINT_CONDITIONS`: pristine, damaged, really damaged.
pub const MAX_GARRISON_POINT_CONDITIONS: usize = 3;
/// `GARRISON_INDEX_INVALID`.
const GARRISON_INDEX_INVALID: i32 = -1;
/// `MUZZLE_FLASH_LIFETIME`.
const MUZZLE_FLASH_LIFETIME: u32 = (LOGICFRAMES_PER_SECOND / 7) as u32;
/// `HUGE_FRAME_IN_FUTURE`.
const HUGE_FRAME_IN_FUTURE: u32 = (LOGICFRAMES_PER_SECOND * 1000) as u32;
/// `FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS`.
const FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS: i32 = 0x8;

/// `class GarrisonContainModuleData : public OpenContainModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct GarrisonContainModuleData {
    /// The `OpenContainModuleData` base.
    pub open: OpenContainModuleData,
    /// `m_doIHealObjects` (`HealObjects`).
    pub do_i_heal_objects: bool,
    /// `m_framesForFullHeal` (`TimeForFullHeal`, a duration as a real).
    pub frames_for_full_heal: f32,
    /// `m_mobileGarrison` (`MobileGarrison`).
    pub mobile_garrison: bool,
    /// `m_immuneToClearBuildingAttacks`.
    pub immune_to_clear_building_attacks: bool,
    /// `m_isEnclosingContainer`: "a sensible default for a garrison container... few exceptions, firebase is one".
    pub is_enclosing_container: bool,
    /// `m_initialRoster.templateName`.
    pub initial_roster_template_name: Vec<u8>,
    /// `m_initialRoster.count`.
    pub initial_roster_count: i32,
}

impl Default for GarrisonContainModuleData {
    /// `GarrisonContainModuleData()`: infantry inside, a heal over one frame, enclosing.
    fn default() -> Self {
        let mut open = OpenContainModuleData::default();
        let mut allow = KindOfMaskType::new();
        allow.set(KINDOF_INFANTRY, 1);
        open.allow_inside_kind_of = allow;
        GarrisonContainModuleData {
            open,
            do_i_heal_objects: false,
            frames_for_full_heal: 1.0,
            mobile_garrison: false,
            immune_to_clear_building_attacks: false,
            is_enclosing_container: true,
            initial_roster_template_name: Vec::new(),
            initial_roster_count: 0,
        }
    }
}

/// Lifts entry `k` of `OpenContainModuleData`'s table onto the `open` base.
fn lift_open(k: usize, i: &mut Ini<'_>, d: &mut GarrisonContainModuleData, u: &UserData) -> IniResult<()> {
    match open_contain::FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.open, u),
        None => Ok(()),
    }
}

/// Lifts entry `k` of the die mux table onto the `open` base.
fn lift_die(k: usize, i: &mut Ini<'_>, d: &mut GarrisonContainModuleData, u: &UserData) -> IniResult<()> {
    match open_contain::DIE_MUX_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.open, u),
        None => Ok(()),
    }
}

static LIFTED_OPEN: [FieldParse<GarrisonContainModuleData>; 16] = [
    FieldParse::new("ContainMax", |i, d, u| lift_open(0, i, d, u), UserData::None),
    FieldParse::new("EnterSound", |i, d, u| lift_open(1, i, d, u), UserData::None),
    FieldParse::new("ExitSound", |i, d, u| lift_open(2, i, d, u), UserData::None),
    FieldParse::new("DamagePercentToUnits", |i, d, u| lift_open(3, i, d, u), UserData::None),
    FieldParse::new("BurnedDeathToUnits", |i, d, u| lift_open(4, i, d, u), UserData::None),
    FieldParse::new("AllowInsideKindOf", |i, d, u| lift_open(5, i, d, u), UserData::None),
    FieldParse::new("ForbidInsideKindOf", |i, d, u| lift_open(6, i, d, u), UserData::None),
    FieldParse::new("PassengersAllowedToFire", |i, d, u| lift_open(7, i, d, u), UserData::None),
    FieldParse::new("PassengersInTurret", |i, d, u| lift_open(8, i, d, u), UserData::None),
    FieldParse::new("NumberOfExitPaths", |i, d, u| lift_open(9, i, d, u), UserData::None),
    FieldParse::new("DoorOpenTime", |i, d, u| lift_open(10, i, d, u), UserData::None),
    FieldParse::new("WeaponBonusPassedToPassengers", |i, d, u| lift_open(11, i, d, u), UserData::None),
    FieldParse::new("AllowAlliesInside", |i, d, u| lift_open(12, i, d, u), UserData::None),
    FieldParse::new("AllowEnemiesInside", |i, d, u| lift_open(13, i, d, u), UserData::None),
    FieldParse::new("AllowNeutralInside", |i, d, u| lift_open(14, i, d, u), UserData::None),
    FieldParse::end(),
];

static LIFTED_DIE: [FieldParse<GarrisonContainModuleData>; 5] = [
    FieldParse::new("DeathTypes", |i, d, u| lift_die(0, i, d, u), UserData::None),
    FieldParse::new("VeterancyLevels", |i, d, u| lift_die(1, i, d, u), UserData::None),
    FieldParse::new("ExemptStatus", |i, d, u| lift_die(2, i, d, u), UserData::None),
    FieldParse::new("RequiredStatus", |i, d, u| lift_die(3, i, d, u), UserData::None),
    FieldParse::end(),
];

/// `parseInitialRoster`: the template's name, then a count (1 when there is none).
fn parse_initial_roster(ini: &mut Ini<'_>, d: &mut GarrisonContainModuleData, _u: &UserData) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    let count = match ini.get_next_token_or_null(None) {
        Some(c) => Ini::scan_int(&c)?,
        None => 1,
    };
    d.initial_roster_template_name = name;
    d.initial_roster_count = count;
    Ok(())
}

/// `GarrisonContainModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<GarrisonContainModuleData>; 7] = [
    FieldParse::new("MobileGarrison", |i, d, _| { d.mobile_garrison = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("HealObjects", |i, d, _| { d.do_i_heal_objects = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("TimeForFullHeal", |i, d, _| { d.frames_for_full_heal = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("InitialRoster", parse_initial_roster, UserData::None),
    FieldParse::new("ImmuneToClearBuildingAttacks", |i, d, _| { d.immune_to_clear_building_attacks = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("IsEnclosingContainer", |i, d, _| { d.is_enclosing_container = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl GarrisonContainModuleData {
    /// `GarrisonContainModuleData::buildFieldParse(p)`: `OpenContainModuleData::buildFieldParse(p)`, then this class's table.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, GarrisonContainModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&LIFTED_OPEN)?;
        p.add(&LIFTED_DIE)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<GarrisonContainModuleData> {
        let mut data = GarrisonContainModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, GarrisonContainModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `GarrisonContain`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::GarrisonContain(Arc::new(GarrisonContainModuleData::friend_new_module_data(ini)?)))
}

/// One half of a garrison point's unions: the object (or effect) pointer, or the id a load wrote over it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointRef {
    /// `NULL`.
    Null,
    /// A pointer to the object (or effect drawable) of this id.
    Live(i32),
    /// The id a load wrote (`objectID`/`effectID`) and not yet found again by `loadPostProcess`. On the shipped (32-bit) build the pointer is the id
    /// itself; `stale` says the pointer before the load was not `NULL` (a 64-bit build keeps its upper half: the two builds disagree for id 0).
    Loaded { id: i32, stale: bool },
}

impl PointRef {
    /// The id the union holds as an id (the low half of the pointer for a live one).
    pub fn word(self) -> u32 {
        match self {
            PointRef::Null => 0,
            PointRef::Live(id) | PointRef::Loaded { id, .. } => id as u32,
        }
    }

    /// `ptr == NULL`; `None` when the builds disagree (undefined).
    fn is_null(self) -> Option<bool> {
        match self {
            PointRef::Null => Some(true),
            PointRef::Live(_) => Some(false),
            PointRef::Loaded { id: 0, stale: false } => Some(true),
            PointRef::Loaded { id: 0, stale: true } => None,
            PointRef::Loaded { .. } => Some(false),
        }
    }

    /// The pointer, dereferenced: `Ok(None)` for `NULL`, `Err` for a pointer that is a loaded id (undefined).
    fn deref(self) -> Result<Option<i32>, ()> {
        match self {
            PointRef::Null | PointRef::Loaded { id: 0, stale: false } => Ok(None),
            PointRef::Live(id) => Ok(Some(id)),
            PointRef::Loaded { .. } => Err(()),
        }
    }

    /// `ptr == obj` for a real object.
    fn is(self, id: i32) -> bool {
        self == PointRef::Live(id)
    }

    /// The union after a load wrote `id` over it.
    fn loaded(self, id: i32) -> PointRef {
        let stale = match self {
            PointRef::Null => false,
            PointRef::Live(_) => true,
            PointRef::Loaded { id: old, stale } => stale || old != 0,
        };
        PointRef::Loaded { id, stale }
    }
}

/// `struct GarrisonPointData`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GarrisonPointData {
    /// `object` / `objectID`.
    pub object: PointRef,
    /// `targetID`.
    pub target_id: ObjectId,
    /// `placeFrame`.
    pub place_frame: u32,
    /// `lastEffectFrame`.
    pub last_effect_frame: u32,
    /// `effect` / `effectID`.
    pub effect: PointRef,
}

impl Default for GarrisonPointData {
    fn default() -> Self {
        GarrisonPointData { object: PointRef::Null, target_id: INVALID_ID, place_frame: 0, last_effect_frame: 0, effect: PointRef::Null }
    }
}

/// `struct StationPointData`.
#[derive(Clone, Copy, Debug)]
pub struct StationPointData {
    /// `occupantID`.
    pub occupant_id: ObjectId,
    /// `position`.
    pub position: Coord3D,
}

/// The bits of a position (the equality the lending of the module needs: a NaN is the same NaN).
fn bits(c: &Coord3D) -> [u32; 3] {
    [c.x.to_bits(), c.y.to_bits(), c.z.to_bits()]
}

/// `calcDistSqr(a, b)`.
fn calc_dist_sqr(a: &Coord3D, b: &Coord3D) -> f32 {
    let (dx, dy, dz) = (a.x - b.x, a.y - b.y, a.z - b.z);
    dx * dx + dy * dy + dz * dz
}

/// `MAKE_OBJECT_STATUS_MASK(bit)`.
fn status_mask(bit: i32) -> ObjectStatusMaskType {
    let mut m = ObjectStatusMaskType::new();
    m.set(bit, 1);
    m
}

/// Records that upstream is undefined here.
fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
    world.cuts.module_errors.push((me, IniError::undefined(u)));
}

/// `class GarrisonContain : public OpenContain`.
#[derive(Clone, Debug)]
pub struct GarrisonContain {
    oc: OpenContain,
    /// `getGarrisonContainModuleData()`.
    data: Arc<GarrisonContainModuleData>,
    /// `m_originalTeam`: "our original team before we were garrisoned" (`None` for `NULL`).
    original_team: Option<TeamId>,
    /// `m_garrisonPointData` (boxed, as the positions: the module would otherwise make every update body, which the world moves about by value,
    /// several kilobytes).
    garrison_point_data: Box<[GarrisonPointData; MAX_GARRISON_POINTS]>,
    /// `m_garrisonPointsInUse`.
    garrison_points_in_use: i32,
    /// `m_garrisonPoint`: "the garrison point positions (in world coords) for pristine, damaged, and really damaged".
    garrison_point: Box<[[Coord3D; MAX_GARRISON_POINTS]; MAX_GARRISON_POINT_CONDITIONS]>,
    /// `m_exitRallyPoint`.
    exit_rally_point: Coord3D,
    /// `m_stationPointList`.
    station_point_list: Vec<StationPointData>,
    /// `m_stationGarrisonPointsInitialized` ("DO NOT XFER THIS!!!").
    station_garrison_points_initialized: bool,
    /// `m_garrisonPointsInitialized`.
    garrison_points_initialized: bool,
    /// `m_hideGarrisonedStateFromNonallies`.
    hide_garrisoned_state_from_nonallies: bool,
    /// `m_rallyValid`.
    rally_valid: bool,
    /// `m_evacDisposition`.
    evac_disposition: EvacDisposition,
    /// A load's original team the factory has not been asked about yet, with the fields after it as they were before the load (see the module docs).
    team_check_pending: Option<Box<GarrisonContain>>,
}

impl PartialEq for GarrisonContain {
    fn eq(&self, other: &Self) -> bool {
        self.oc == other.oc
            && Arc::ptr_eq(&self.data, &other.data)
            && self.original_team == other.original_team
            && self.garrison_point_data == other.garrison_point_data
            && self.garrison_points_in_use == other.garrison_points_in_use
            && self.garrison_point.iter().flatten().zip(other.garrison_point.iter().flatten()).all(|(a, b)| bits(a) == bits(b))
            && bits(&self.exit_rally_point) == bits(&other.exit_rally_point)
            && self.station_point_list.len() == other.station_point_list.len()
            && self.station_point_list.iter().zip(other.station_point_list.iter()).all(|(a, b)| a.occupant_id == b.occupant_id && bits(&a.position) == bits(&b.position))
            && self.station_garrison_points_initialized == other.station_garrison_points_initialized
            && self.garrison_points_initialized == other.garrison_points_initialized
            && self.hide_garrisoned_state_from_nonallies == other.hide_garrisoned_state_from_nonallies
            && self.rally_valid == other.rally_valid
            && self.evac_disposition == other.evac_disposition
            && self.team_check_pending == other.team_check_pending
    }
}

impl GarrisonContain {
    /// `getInterfaceMask()`: `OpenContain`'s.
    pub const fn get_interface_mask() -> i32 {
        OpenContain::get_interface_mask()
    }

    /// The constructor's first half: the base's (on a copy of its part of the data); no team, no points in use, everything zero.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<GarrisonContainModuleData>>) -> IniResult<GarrisonContain> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let oc = OpenContain::unconstructed(object, Some(Arc::new(data.open.clone())))?;
        let zero = Coord3D { x: 0.0, y: 0.0, z: 0.0 };
        Ok(GarrisonContain {
            oc,
            data,
            original_team: None,
            garrison_point_data: Box::new([GarrisonPointData::default(); MAX_GARRISON_POINTS]),
            garrison_points_in_use: 0,
            garrison_point: Box::new([[zero; MAX_GARRISON_POINTS]; MAX_GARRISON_POINT_CONDITIONS]),
            exit_rally_point: zero,
            station_point_list: Vec::new(),
            station_garrison_points_initialized: false,
            garrison_points_initialized: false,
            hide_garrisoned_state_from_nonallies: false,
            rally_valid: false,
            evac_disposition: EvacDisposition::BurstFromCenter,
            team_check_pending: None,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        self.oc.update_module()
    }

    /// The module data.
    pub fn module_data(&self) -> &Arc<GarrisonContainModuleData> {
        &self.data
    }

    /// The state, as words: the base's; the original team; per point the object, target, frames and effect (as the unions' low words); the points
    /// in use; every position; the rally point; the station points; the flags and the evacuation side.
    pub fn state_words(&self) -> Vec<u32> {
        let mut v = self.oc.state_words();
        v.push(self.original_team.unwrap_or(TEAM_ID_INVALID));
        for p in self.garrison_point_data.iter() {
            v.extend([p.object.word(), p.target_id as u32, p.place_frame, p.last_effect_frame, p.effect.word()]);
        }
        v.push(self.garrison_points_in_use as u32);
        for c in self.garrison_point.iter().flatten() {
            v.extend(bits(c));
        }
        v.extend(bits(&self.exit_rally_point));
        v.push(self.station_point_list.len() as u32);
        for s in &self.station_point_list {
            v.push(s.occupant_id as u32);
            v.extend(bits(&s.position));
        }
        v.push(u32::from(self.station_garrison_points_initialized));
        v.push(u32::from(self.garrison_points_initialized));
        v.push(u32::from(self.hide_garrisoned_state_from_nonallies));
        v.push(u32::from(self.rally_valid));
        v.push(self.evac_disposition as u32);
        v
    }

    /// The constructor's second half: the base's.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.oc.world_construct(world, me);
    }

    /// `update()` as the scheduler runs it.
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The module's answer to one of `Object`'s calls; `loadPostProcess` finds the points' objects and effects after the base's has run.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        if matches!(call, ModuleCall::LoadPostProcess) {
            let before = world.cuts.module_errors.len();
            let r = open_contain::contain_module_call(self, world, me, call);
            // `OpenContain::loadPostProcess` threw: this class's part does not run
            if world.cuts.module_errors[before..].iter().any(|(_, e)| !e.is_undefined()) {
                return r;
            }
            if self.load_post_process_points(world).is_err() {
                world.cuts.module_errors.push((me, IniError::new(IniErrorKind::InvalidData)));
            }
            return r;
        }
        open_contain::contain_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: the version (1), the base's, then this class's fields.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.oc.xfer_with(update, xfer)?;
        self.xfer_own_fields(xfer)
    }

    /// `crc` with the scheduling half: "extend base class".
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.oc.crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half: the base's (the points are found by [`GarrisonContain::module_call`]'s, which has the world).
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        self.oc.load_post_process_with(update);
    }

    /// The fields of `xfer`, after the base's.
    fn xfer_own_fields(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut team_id = self.original_team.unwrap_or(TEAM_ID_INVALID);
        let mut b = team_id.to_le_bytes();
        let r = xfer.xfer_user(&mut b);
        team_id = u32::from_le_bytes(b);
        r?;
        if xfer.get_xfer_mode() == XferMode::Load {
            if team_id != TEAM_ID_INVALID {
                // `findTeamByID` is the world's: the check is made after the xfer, on the fields as they are now (see the module docs)
                self.original_team = Some(team_id);
                let mut before = self.clone();
                before.original_team = None;
                before.team_check_pending = None;
                self.team_check_pending = Some(Box::new(before));
            } else {
                self.original_team = None;
            }
        }
        xfer.xfer_bool(&mut self.hide_garrisoned_state_from_nonallies)?;
        let mut point_data_count = MAX_GARRISON_POINTS as u16;
        xfer.xfer_unsigned_short(&mut point_data_count)?;
        let load = xfer.get_xfer_mode() == XferMode::Load;
        for i in 0..point_data_count as usize {
            if !load {
                let p = self.garrison_point_data[i];
                // `obj ? obj->getID() : INVALID_ID`, `draw ? draw->getID() : INVALID_DRAWABLE_ID`: a loaded pointer is not one (`check_save` has refused it)
                let mut object_id = p.object.deref().ok().flatten().unwrap_or(INVALID_ID);
                xfer.xfer_object_id(&mut object_id)?;
                xfer.xfer_object_id(&mut self.garrison_point_data[i].target_id)?;
                xfer.xfer_unsigned_int(&mut self.garrison_point_data[i].place_frame)?;
                xfer.xfer_unsigned_int(&mut self.garrison_point_data[i].last_effect_frame)?;
                let mut drawable_id = p.effect.deref().ok().flatten().unwrap_or(INVALID_DRAWABLE_ID);
                xfer.xfer_drawable_id(&mut drawable_id)?;
            } else {
                let (mut object_id, mut target_id, mut place_frame, mut last_effect_frame, mut drawable_id) = (0, 0, 0u32, 0u32, 0);
                xfer.xfer_object_id(&mut object_id)?;
                xfer.xfer_object_id(&mut target_id)?;
                xfer.xfer_unsigned_int(&mut place_frame)?;
                xfer.xfer_unsigned_int(&mut last_effect_frame)?;
                xfer.xfer_drawable_id(&mut drawable_id)?;
                if i < MAX_GARRISON_POINTS {
                    let p = &mut self.garrison_point_data[i];
                    p.object = p.object.loaded(object_id);
                    p.target_id = target_id;
                    p.place_frame = if crate::logic::modules::plant(1631) { 0 } else { place_frame };
                    p.last_effect_frame = last_effect_frame;
                    p.effect = p.effect.loaded(drawable_id);
                }
            }
        }
        xfer.xfer_int(&mut self.garrison_points_in_use)?;
        let mut raw = [0u8; 12 * MAX_GARRISON_POINT_CONDITIONS * MAX_GARRISON_POINTS];
        for (k, c) in self.garrison_point.iter().flatten().enumerate() {
            raw[k * 12..k * 12 + 4].copy_from_slice(&c.x.to_le_bytes());
            raw[k * 12 + 4..k * 12 + 8].copy_from_slice(&c.y.to_le_bytes());
            raw[k * 12 + 8..k * 12 + 12].copy_from_slice(&c.z.to_le_bytes());
        }
        let r = xfer.xfer_user(&mut raw);
        for (k, c) in self.garrison_point.iter_mut().flatten().enumerate() {
            let f = |o: usize| f32::from_le_bytes([raw[o], raw[o + 1], raw[o + 2], raw[o + 3]]);
            *c = Coord3D { x: f(k * 12), y: f(k * 12 + 4), z: f(k * 12 + 8) };
        }
        r?;
        xfer.xfer_bool(&mut self.garrison_points_initialized)?;
        xfer.xfer_bool(&mut self.rally_valid)?;
        xfer.xfer_coord3d(&mut self.exit_rally_point)?;
        Ok(())
    }

    /// A save writes each point's object and effect id through the pointer: a pointer a load left (not yet found again) is undefined.
    pub fn check_save(&self) -> bool {
        self.garrison_point_data.iter().all(|p| p.object.deref().is_ok() && p.effect.deref().is_ok())
    }

    /// The load's `TheTeamFactory->findTeamByID(teamID)`, made after the xfer: an id it does not know is `SC_INVALID_DATA` (the test's
    /// `XFER_LIST_NOT_EMPTY`), with the original team `NULL` and the fields after it as they were. `r` is the rest of the xfer's result.
    pub fn check_loaded_team(&mut self, world: &World, r: XferResult<()>) -> XferResult<()> {
        let Some(before) = self.team_check_pending.take() else { return r };
        let id = self.original_team.unwrap_or(TEAM_ID_INVALID);
        if world.contain_find_team_by_id(id) {
            return r;
        }
        let oc = std::mem::replace(&mut self.oc, before.oc.clone());
        *self = *before;
        self.oc = oc;
        self.original_team = None;
        Err(XferStatus::ListNotEmpty)
    }

    /// This class's `loadPostProcess`: each point's object and effect found by id (`SC_INVALID_DATA` for one that is not there, the points after it
    /// left as the load wrote them).
    fn load_post_process_points(&mut self, world: &World) -> Result<(), ()> {
        for i in 0..MAX_GARRISON_POINTS {
            let p = self.garrison_point_data[i];
            let object_id = p.object.word() as i32;
            if object_id != INVALID_ID {
                if world.objects.find(object_id).is_none() {
                    return Err(());
                }
                self.garrison_point_data[i].object = PointRef::Live(object_id);
            } else {
                self.garrison_point_data[i].object = PointRef::Null;
            }
            let effect_id = p.effect.word() as i32;
            if effect_id != INVALID_ID && !crate::logic::modules::plant(1632) {
                if !world.contain_effect_exists(effect_id) {
                    return Err(());
                }
                self.garrison_point_data[i].effect = PointRef::Live(effect_id);
            } else {
                self.garrison_point_data[i].effect = PointRef::Null;
            }
        }
        Ok(())
    }

    // ---- the garrison points ------------------------------------------------------------------------------------------------------------------------

    /// `isEnclosingContainerFor(obj)`: the data's, for any object.
    fn enclosing(&self) -> bool {
        self.data.is_enclosing_container
    }

    /// `findClosestFreeGarrisonPointIndex(conditionIndex, targetPos)`; `Err` for a point the builds see differently (undefined).
    fn find_closest_free_garrison_point_index(&self, condition_index: usize, target_pos: Option<&Coord3D>) -> Result<i32, ()> {
        let Some(target_pos) = target_pos else { return Ok(GARRISON_INDEX_INVALID) };
        if self.garrison_points_in_use == MAX_GARRISON_POINTS as i32 {
            return Ok(GARRISON_INDEX_INVALID);
        }
        let mut closest_index = GARRISON_INDEX_INVALID;
        let mut closest_dist_sq = -1.0f32;
        for i in 0..MAX_GARRISON_POINTS {
            let free = self.garrison_point_data[i].object.is_null().ok_or(())?;
            if free || crate::logic::modules::plant(1601) {
                let dist_sq = calc_dist_sqr(target_pos, &self.garrison_point[condition_index][i]);
                if dist_sq < closest_dist_sq || closest_dist_sq == -1.0 {
                    closest_dist_sq = dist_sq;
                    closest_index = i as i32;
                }
            }
        }
        Ok(closest_index)
    }

    /// `getObjectGarrisonPointIndex(obj)`.
    fn get_object_garrison_point_index(&self, obj: ObjectId) -> i32 {
        if obj == INVALID_ID {
            return GARRISON_INDEX_INVALID;
        }
        self.garrison_point_data.iter().position(|p| p.object.is(obj)).map_or(GARRISON_INDEX_INVALID, |i| i as i32)
    }

    /// `putObjectAtGarrisonPoint(obj, targetID, conditionIndex, pointIndex)`.
    fn put_object_at_garrison_point(&mut self, world: &mut World, me: ModuleHandle, obj: ObjectId, target_id: ObjectId, condition_index: i32, point_index: i32) {
        if obj == INVALID_ID || point_index < 0 || point_index >= MAX_GARRISON_POINTS as i32 || condition_index < 0 || condition_index >= MAX_GARRISON_POINT_CONDITIONS as i32 {
            // DEBUG_CRASH: "Invalid arguments"
            return;
        }
        let pi = point_index as usize;
        match self.garrison_point_data[pi].object.is_null() {
            None => return undefined(world, me, Undefined::NullObject),
            Some(false) => return, // DEBUG_CRASH: "Garrison Point is not empty"
            Some(true) => {}
        }
        let pos = self.garrison_point[condition_index as usize][pi];
        out(self, world, me, |w| w.contain_set_position(obj, &pos));
        let p = &mut self.garrison_point_data[pi];
        p.object = PointRef::Live(obj);
        p.target_id = target_id;
        p.place_frame = if crate::logic::modules::plant(1603) { 0 } else { world.frame() };
        self.garrison_points_in_use += 1;

        // `static const ThingTemplate *muzzle = TheThingFactory->findTemplate( "GarrisonGun" )`
        if world.contain_garrison_gun_exists() && self.enclosing() {
            // "If we are showing the contained, we need no gun barrel drawable added"
            let draw = world.contain_effect_new();
            world.contain_effect_set_position(draw, &pos);
            let p = &mut self.garrison_point_data[pi];
            p.effect = PointRef::Live(draw);
            p.last_effect_frame = 0;
            if world.has_drawable(me.object) {
                let obscured = world.contain_drawable_fully_obscured_by_shroud(me.object);
                world.contain_effect_set_fully_obscured_by_shroud(draw, obscured);
            }
        }
    }

    /// `findConditionIndex()`: the points for the body's damage state; `None` (undefined recorded) without a body or for a state it does not know.
    fn find_condition_index(&mut self, world: &mut World, me: ModuleHandle) -> Option<usize> {
        let us = me.object;
        let Some(body) = world.get_body_module(us) else {
            undefined(world, me, Undefined::NullBodyModule);
            return None;
        };
        let state = out(self, world, me, |w| w.module_call(us, body, ModuleCall::GetDamageState)).as_int(BODY_PRISTINE);
        match state {
            BODY_PRISTINE => Some(0),
            BODY_DAMAGED => Some(1),
            BODY_REALLYDAMAGED => Some(2),
            BODY_RUBBLE => Some(if crate::logic::modules::plant(1604) { 1 } else { 2 }),
            _ => {
                // DEBUG_CRASH, then m_garrisonPoint[ GARRISON_INDEX_INVALID ]
                undefined(world, me, Undefined::NullObject);
                None
            }
        }
    }

    /// `putObjectAtBestGarrisonPoint(obj, target, targetPos)`.
    fn put_object_at_best_garrison_point(&mut self, world: &mut World, me: ModuleHandle, obj: ObjectId, target: ObjectId, target_pos: Option<Coord3D>) {
        if obj == INVALID_ID || (target == INVALID_ID && target_pos.is_none()) {
            return;
        }
        let target_pos = if target != INVALID_ID { world.objects.find(target).map(|o| o.get_position()) } else { target_pos };
        if self.get_object_garrison_point_index(obj) != GARRISON_INDEX_INVALID {
            return;
        }
        let Some(condition_index) = self.find_condition_index(world, me) else { return };
        let Ok(place_index) = self.find_closest_free_garrison_point_index(condition_index, target_pos.as_ref()) else {
            return undefined(world, me, Undefined::NullObject);
        };
        self.put_object_at_garrison_point(world, me, obj, target, condition_index as i32, place_index);
    }

    /// `removeObjectFromGarrisonPoint(obj, index)` (`None` is `SEARCH_FOR_REMOVE`): back to the centre of the building.
    fn remove_object_from_garrison_point(&mut self, world: &mut World, me: ModuleHandle, obj: ObjectId, index: Option<usize>) {
        if !self.enclosing() {
            return; // "since I am not enclosed, I am not at a garrison point!"
        }
        if obj == INVALID_ID {
            return;
        }
        let remove_index = match index {
            Some(i) => i,
            None => match self.garrison_point_data.iter().position(|p| p.object.is(obj)) {
                Some(i) => i,
                None => return,
            },
        };
        let effect = self.garrison_point_data[remove_index].effect;
        let p = &mut self.garrison_point_data[remove_index];
        p.object = PointRef::Null;
        p.target_id = INVALID_ID;
        p.place_frame = 0;
        p.last_effect_frame = 0;
        if !crate::logic::modules::plant(1602) {
            self.garrison_points_in_use -= 1;
        }
        match effect.deref() {
            Ok(Some(e)) => world.contain_effect_destroy(e),
            Ok(None) => {}
            Err(()) => return undefined(world, me, Undefined::NullObject),
        }
        self.garrison_point_data[remove_index].effect = PointRef::Null;
        let pos = world.objects.find(me.object).map_or(Coord3D { x: 0.0, y: 0.0, z: 0.0 }, |o| o.get_position());
        out(self, world, me, |w| w.contain_set_position(obj, &pos));
    }

    /// `removeInvalidObjectsFromGarrisonPoints()`: the riders at points that no longer have a target in range go back to the centre.
    fn remove_invalid_objects_from_garrison_points(&mut self, world: &mut World, me: ModuleHandle) -> Option<()> {
        if self.garrison_points_in_use == 0 {
            return Some(()); // "my, that was easy"
        }
        for i in 0..MAX_GARRISON_POINTS {
            let obj = match self.garrison_point_data[i].object.deref() {
                Ok(Some(o)) => o,
                Ok(None) => continue,
                Err(()) => {
                    undefined(world, me, Undefined::NullObject);
                    return None;
                }
            };
            let Some(ai) = world.objects.find(obj).and_then(|o| o.ai) else {
                undefined(world, me, Undefined::NullObject);
                return None;
            };
            let mut target_is_valid = true;
            let goal = out(self, world, me, |w| w.module_call(obj, ai, ModuleCall::AiGetGoalObject)).as_id();
            if goal != INVALID_ID && world.objects.find(goal).is_some() && !crate::logic::modules::plant(1608) {
                let in_range = world.contain_current_weapon(obj).is_some() && out(self, world, me, |w| w.contain_weapon_within_attack_range(obj, goal, None));
                if !in_range {
                    target_is_valid = false;
                }
            }
            let attacking = world.objects.find(obj).is_some_and(|o| o.test_status(OBJECT_STATUS_IS_ATTACKING));
            if !attacking || !target_is_valid {
                self.remove_object_from_garrison_point(world, me, obj, Some(i));
            }
        }
        Some(())
    }

    /// `addValidObjectsToGarrisonPoints()`: the riders in the centre that have a target now go to a point.
    fn add_valid_objects_to_garrison_points(&mut self, world: &mut World, me: ModuleHandle) {
        let list = self.oc.contain_list().to_vec();
        for obj in list {
            let Some(ai) = world.objects.find(obj).and_then(|o| o.ai) else { continue };
            let victim = out(self, world, me, |w| w.module_call(obj, ai, ModuleCall::AiGetCurrentVictim)).as_id();
            let victim_pos = out(self, world, me, |w| w.module_call(obj, ai, ModuleCall::AiGetCurrentVictimPos)).as_coord_opt();
            if victim != INVALID_ID {
                self.put_object_at_best_garrison_point(world, me, obj, victim, None);
            } else if let Some(pos) = victim_pos.filter(|_| !crate::logic::modules::plant(1609)) {
                self.put_object_at_best_garrison_point(world, me, obj, INVALID_ID, Some(pos));
            }
        }
    }

    /// `trackTargets()`: "keeps any of the attacking units at any of the fire points closest to their active target", and turns the gun barrels.
    fn track_targets(&mut self, world: &mut World, me: ModuleHandle) {
        if !self.enclosing() {
            return; // "objects fire from their station points, instead of being juggled around between garrison firepoints"
        }
        let Some(condition_index) = self.find_condition_index(world, me) else { return };
        for obj in self.oc.contain_list().to_vec() {
            let our_index = self.get_object_garrison_point_index(obj);
            if our_index == GARRISON_INDEX_INVALID {
                continue;
            }
            let our_index = our_index as usize;
            let Some(ai) = world.objects.find(obj).and_then(|o| o.ai) else { continue };
            let victim = out(self, world, me, |w| w.module_call(obj, ai, ModuleCall::AiGetCurrentVictim)).as_id();
            let mut victim_pos = out(self, world, me, |w| w.module_call(obj, ai, ModuleCall::AiGetCurrentVictimPos)).as_coord_opt();
            if victim == INVALID_ID && victim_pos.is_none() {
                continue;
            }
            if victim != INVALID_ID {
                victim_pos = world.objects.find(victim).map(|o| o.get_position());
            }
            let Some(vp) = victim_pos else { continue };
            let our_pos = world.objects.find(obj).map_or(Coord3D { x: 0.0, y: 0.0, z: 0.0 }, |o| o.get_position());
            let Ok(new_index) = self.find_closest_free_garrison_point_index(condition_index, Some(&vp)) else {
                return undefined(world, me, Undefined::NullObject);
            };
            if new_index != GARRISON_INDEX_INVALID {
                let current_dist_sq = calc_dist_sqr(&vp, &our_pos);
                let new_dist_sq = calc_dist_sqr(&vp, &self.garrison_point[condition_index][new_index as usize]);
                if (new_dist_sq < current_dist_sq) != crate::logic::modules::plant(1610) {
                    self.remove_object_from_garrison_point(world, me, obj, Some(our_index));
                    self.put_object_at_garrison_point(world, me, obj, victim, condition_index as i32, new_index);
                }
            }
            // the point the rider was at, and where it is now (`ourPos` points at the object's position)
            match self.garrison_point_data[our_index].effect.deref() {
                Ok(Some(effect)) => {
                    let our_pos = world.objects.find(obj).map_or(our_pos, |o| o.get_position());
                    let v = Coord2D { x: vp.x - our_pos.x, y: vp.y - our_pos.y };
                    world.contain_effect_set_orientation(effect, v.to_angle());
                }
                Ok(None) => {}
                Err(()) => return undefined(world, me, Undefined::NullObject),
            }
        }
    }

    /// `updateEffects()`: a muzzle flash at the point of a rider that shot last frame, off again after `MUZZLE_FLASH_LIFETIME`.
    fn update_effects(&mut self, world: &mut World, me: ModuleHandle) {
        let current_frame = world.frame();
        for obj in self.oc.contain_list().to_vec() {
            if world.contain_last_shot_fired_frame(obj) != current_frame.wrapping_sub(1) {
                continue;
            }
            let garrison_index = self.get_object_garrison_point_index(obj);
            if garrison_index == GARRISON_INDEX_INVALID {
                continue;
            }
            let gi = garrison_index as usize;
            let effect = match self.garrison_point_data[gi].effect.deref() {
                Ok(Some(e)) => e,
                Ok(None) => continue,
                Err(()) => return undefined(world, me, Undefined::NullObject),
            };
            // "No muzzle flash with poison weapon"
            if let Some(damage_type) = world.contain_current_weapon(obj) {
                if damage_type != DAMAGE_POISON || crate::logic::modules::plant(1612) {
                    world.contain_effect_model_condition(effect, MODELCONDITION_FIRING_A, true);
                    self.garrison_point_data[gi].last_effect_frame = current_frame;
                }
            }
        }
        let lifetime = if crate::logic::modules::plant(1613) { MUZZLE_FLASH_LIFETIME - 1 } else { MUZZLE_FLASH_LIFETIME };
        for i in 0..MAX_GARRISON_POINTS {
            let p = self.garrison_point_data[i];
            let effect = match p.effect.deref() {
                Ok(Some(e)) => e,
                Ok(None) => continue,
                Err(()) => return undefined(world, me, Undefined::NullObject),
            };
            if p.last_effect_frame != 0 && current_frame.wrapping_sub(p.last_effect_frame) > lifetime {
                world.contain_effect_model_condition(effect, MODELCONDITION_FIRING_A, false);
                self.garrison_point_data[i].last_effect_frame = 0;
            }
        }
    }

    /// `matchObjectsToGarrisonPoints()`: "every frame, and whenever anyone enters or leaves".
    fn match_objects_to_garrison_points(&mut self, world: &mut World, me: ModuleHandle) {
        if !self.enclosing() {
            self.position_objects_at_station_garrison_points(world, me);
        } else {
            if self.remove_invalid_objects_from_garrison_points(world, me).is_none() {
                return;
            }
            self.add_valid_objects_to_garrison_points(world, me);
            self.update_effects(world, me);
            self.track_targets(world, me);
        }
    }

    /// `positionObjectsAtStationGarrisonPoints()`: "enforce that everybody stays at their pre-assigned space".
    fn position_objects_at_station_garrison_points(&mut self, world: &mut World, me: ModuleHandle) {
        if !self.station_garrison_points_initialized && !self.load_station_garrison_points(world, me) {
            return;
        }
        for contained in self.oc.contain_list().to_vec() {
            let spot = self.station_point_list.iter().find(|s| s.occupant_id == contained).map(|s| s.position);
            match spot {
                Some(pos) => out(self, world, me, |w| w.contain_set_position(contained, &pos)),
                None => {
                    // DEBUG_ASSERTCRASH when there is no vacancy
                    self.pick_a_station_for_me(contained);
                }
            }
        }
    }

    /// `pickAStationForMe(obj)`: the first vacant station.
    fn pick_a_station_for_me(&mut self, obj: ObjectId) -> bool {
        let found = if crate::logic::modules::plant(1633) {
            self.station_point_list.first_mut()
        } else {
            self.station_point_list.iter_mut().find(|s| s.occupant_id == INVALID_ID)
        };
        match found {
            Some(s) => {
                s.occupant_id = obj;
                true
            }
            None => false,
        }
    }

    /// `removeObjectFromStationPoint(obj)`: "give up your space".
    fn remove_object_from_station_point(&mut self, obj: ObjectId) {
        if obj == INVALID_ID {
            return;
        }
        if let Some(s) = self.station_point_list.iter_mut().find(|s| s.occupant_id == obj) {
            s.occupant_id = INVALID_ID;
        }
    }

    /// `healObjects()`.
    fn heal_objects(&mut self, world: &mut World, me: ModuleHandle) {
        if !self.data.do_i_heal_objects && !crate::logic::modules::plant(1615) {
            return;
        }
        let frames = self.data.frames_for_full_heal;
        for obj in self.oc.contain_list().to_vec() {
            if !self.heal_single_object(world, me, obj, frames) {
                return;
            }
        }
    }

    /// `healSingleObject(obj, framesForFullHeal)`; `false` for a rider without a body (undefined).
    fn heal_single_object(&mut self, world: &mut World, me: ModuleHandle, obj: ObjectId, frames_for_full_heal: f32) -> bool {
        let mut info = DamageInfo::default();
        info.input.damage_type = DAMAGE_HEALING;
        info.input.death_type = DEATH_NONE;
        let Some(body) = world.objects.find(obj).and_then(|o| o.body) else {
            undefined(world, me, Undefined::NullBodyModule);
            return false;
        };
        let max = out(self, world, me, |w| w.module_call(obj, body, ModuleCall::GetMaxHealth)).as_real(0.0);
        let since = world.frame().wrapping_sub(world.objects.find(obj).map_or(0, |o| o.contained_by_frame));
        if since as f32 >= frames_for_full_heal || crate::logic::modules::plant(1616) {
            info.input.amount = max;
        } else {
            info.input.amount = max / frames_for_full_heal;
        }
        out(self, world, me, |w| w.module_call(obj, body, ModuleCall::AttemptHealing(info)));
        true
    }

    /// `getApparentControllingPlayer(observingPlayer)`: the original team's player for a non-ally while the garrison is hidden, else the object's.
    fn get_apparent_controlling_player(&self, world: &World, me: ModuleHandle, observing_player: Option<PlayerIndex>) -> Option<PlayerIndex> {
        let my_player = world.get_controlling_player(me.object);
        if let (true, Some(team), Some(mine), Some(observer)) = (self.hide_garrisoned_state_from_nonallies, self.original_team, my_player, observing_player) {
            let r = world.cuts.player_relationship_to_player_default_team(mine, observer);
            if (r != Relationship::Allies) != crate::logic::modules::plant(1617) {
                return world.contain_team_controlling_player(team);
            }
        }
        my_player
    }

    /// `recalcApparentControllingPlayer()`: the first rider's player owns the building (hidden while every rider is an undetected stealth unit); none
    /// gives it back; the drawable shows it, and the points are loaded once someone is in.
    fn recalc_apparent_controlling_player(&mut self, world: &mut World, me: ModuleHandle) {
        let us = me.object;
        if self.original_team.is_none() {
            self.original_team = world.get_team(us);
        }
        if world.get_team(us).is_none() {
            self.original_team = None;
        }
        let count = self.get_contain_count(world);
        if count > 0 {
            let Some(&rider) = self.oc.contain_list().first() else {
                // `*begin()` of an empty list: the end node's garbage, dereferenced (not the NULL the edited loops of other classes read)
                undefined(world, me, Undefined::EmptyContainedList);
                return;
            };
            let detected = world.objects.find(rider).is_some_and(|o| o.test_status(OBJECT_STATUS_DETECTED));
            self.hide_garrisoned_state_from_nonallies = !detected && (self.oc.stealth_units_contained() == count || crate::logic::modules::plant(1618));
            let controller = world.get_controlling_player(rider);
            if let Some(p) = controller {
                let team = world.cuts.player_default_team(p);
                if !crate::logic::modules::plant(1619) {
                    out(self, world, me, |w| w.contain_set_team(us, team));
                }
            }
        } else {
            let original = self.original_team;
            out(self, world, me, |w| w.contain_set_team_opt(us, original));
            self.hide_garrisoned_state_from_nonallies = false;
        }

        if !world.has_drawable(us) {
            return;
        }
        let local = world.players.local_index();
        let mut set_model_garrisoned = false;
        if count > 0 {
            let Some(&occupant) = self.oc.contain_list().first() else {
                // `*begin()` of an empty list: the end node's garbage, dereferenced (not the NULL the edited loops of other classes read)
                undefined(world, me, Undefined::EmptyContainedList);
                return;
            };
            let detected = world.objects.find(occupant).is_some_and(|o| o.test_status(OBJECT_STATUS_DETECTED));
            if detected || self.get_apparent_controlling_player(world, me, local) == world.get_controlling_player(us) {
                set_model_garrisoned = true;
            }
        }
        out(self, world, me, |w| w.contain_drawable_model_condition(us, MODELCONDITION_GARRISONED, set_model_garrisoned));
        if let Some(controller) = self.get_apparent_controlling_player(world, me, local) {
            let color = if world.cuts.globals.time_of_day_is_night { world.cuts.player_night_color(controller) } else { world.cuts.player_color(controller) };
            world.contain_drawable_set_indicator_color(us, color);
        }
        if count > 0 {
            if self.enclosing() {
                if !self.garrison_points_initialized {
                    self.load_garrison_points(world, me);
                }
            } else if !self.station_garrison_points_initialized {
                self.load_station_garrison_points(world, me);
            }
        }
    }

    /// The bones `prefix` of the art in the condition `clear`/`set` puts it in, written over `positions` (the count found).
    fn bones_in_condition(&mut self, world: &mut World, me: ModuleHandle, (clear, set): (&[i32], &[i32]), prefix: &[u8], max: i32, positions: &mut [Coord3D]) -> i32 {
        let us = me.object;
        out(self, world, me, |w| w.contain_clear_and_set_model_condition_flags(us, clear, set));
        let found: Vec<Matrix3D> = out(self, world, me, |w| w.contain_get_multi_logical_bone_position(us, prefix, max));
        for (k, m) in found.iter().enumerate().take(positions.len()) {
            let t = m.get_translation();
            positions[k] = Coord3D { x: t.x, y: t.y, z: t.z };
        }
        found.len() as i32
    }

    /// `loadGarrisonPoints()`: "load the garrison point position data and save for use later" (with a drawable: the caller checks).
    fn load_garrison_points(&mut self, world: &mut World, me: ModuleHandle) {
        let us = me.object;
        let here = world.objects.find(us).map_or(Coord3D { x: 0.0, y: 0.0, z: 0.0 }, |o| o.get_position());
        for c in self.garrison_point.iter_mut().flatten() {
            *c = here;
        }
        let original = world.reads.drawable_model_condition_flags(us);
        let conditions: [([i32; 4], i32); 3] = [
            ([MODELCONDITION_REALLY_DAMAGED, MODELCONDITION_RUBBLE, MODELCONDITION_SPECIAL_DAMAGED, MODELCONDITION_DAMAGED], MODELCONDITION_GARRISONED),
            ([MODELCONDITION_REALLY_DAMAGED, MODELCONDITION_RUBBLE, MODELCONDITION_SPECIAL_DAMAGED, -1], MODELCONDITION_DAMAGED),
            ([MODELCONDITION_RUBBLE, MODELCONDITION_SPECIAL_DAMAGED, MODELCONDITION_DAMAGED, -1], MODELCONDITION_REALLY_DAMAGED),
        ];
        for (index, (clear, set)) in conditions.iter().enumerate() {
            let clear: Vec<i32> = clear.iter().copied().filter(|&c| c >= 0).collect();
            let into = if crate::logic::modules::plant(1620) && index == 2 { 1 } else { index };
            let mut points = self.garrison_point[into];
            self.bones_in_condition(world, me, (&clear, &[*set]), b"FIREPOINT", MAX_GARRISON_POINTS as i32, &mut points);
            self.garrison_point[into] = points;
        }
        world.contain_drawable_replace_model_condition_flags(us, &original);
        self.garrison_points_initialized = true;
    }

    /// `loadStationGarrisonPoints()`; `false` for a building without a drawable (read unchecked: undefined).
    fn load_station_garrison_points(&mut self, world: &mut World, me: ModuleHandle) -> bool {
        let us = me.object;
        if !world.has_drawable(us) {
            undefined(world, me, Undefined::NullObject);
            return false;
        }
        let original = world.reads.drawable_model_condition_flags(us);
        let here = world.objects.find(us).map_or(Coord3D { x: 0.0, y: 0.0, z: 0.0 }, |o| o.get_position());
        let mut temp = [here; MAX_GARRISON_POINTS];
        let clear = [MODELCONDITION_REALLY_DAMAGED, MODELCONDITION_RUBBLE, MODELCONDITION_SPECIAL_DAMAGED, MODELCONDITION_DAMAGED];
        let max = self.data.open.contain_max;
        let count = self.bones_in_condition(world, me, (&clear, &[MODELCONDITION_GARRISONED]), b"STATION", max, &mut temp);
        // "we are starting over... forget everything"
        self.station_point_list.clear();
        for t in temp.iter().take(count.max(0) as usize) {
            self.station_point_list.push(StationPointData { occupant_id: INVALID_ID, position: *t });
        }
        world.contain_drawable_replace_model_condition_flags(us, &original);
        self.station_garrison_points_initialized = true;
        true
    }

    /// `validateRallyPoint()`: "validate any exit rally point that has been chosen (if any). If it's not valid, try to find a new one".
    fn validate_rally_point(&mut self, world: &mut World, me: ModuleHandle) {
        let us = me.object;
        if self.rally_valid {
            let mut result = self.exit_rally_point;
            if !out(self, world, me, |w| w.contain_partition_find_position_around(&mut result, 0.0, 0.0, FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS)) {
                self.rally_valid = false;
            }
        }
        if !self.rally_valid {
            let Some(o) = world.objects.find(us) else { return };
            let min_radius = o.geometry_info.get_bounding_circle_radius();
            // "arbitrary max distance away, change as needed"
            let max_radius = min_radius * if crate::logic::modules::plant(1621) { 3.6 } else { 1.8 };
            let mut pos = o.get_position();
            self.rally_valid = out(self, world, me, |w| w.contain_partition_find_position_around(&mut pos, min_radius, max_radius, FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS));
            if self.rally_valid {
                self.exit_rally_point = pos;
            }
        }
    }

    /// `moveObjectsWithMe()`: "a Mobile garrison keeps its occupants with it when it moves".
    fn move_objects_with_me(&mut self, world: &mut World, me: ModuleHandle) {
        if !self.data.mobile_garrison {
            return;
        }
        let pos = world.objects.find(me.object).map_or(Coord3D { x: 0.0, y: 0.0, z: 0.0 }, |o| o.get_position());
        for obj in self.oc.contain_list().to_vec() {
            out(self, world, me, |w| w.contain_set_position(obj, &pos));
        }
    }

    /// The end of the exit: the pathfinder, the AI's path to `end` and its goal.
    fn exit_walk(&mut self, world: &mut World, me: ModuleHandle, exit_obj: ObjectId, ai: Option<crate::logic::world::object::ModuleIndex>, mut end: Coord3D) {
        out(self, world, me, |w| w.contain_pathfind_add_object(exit_obj));
        if let Some(ai) = ai {
            out(self, world, me, |w| w.contain_pathfind_adjust_to_possible_destination(exit_obj, &mut end));
            let us = me.object;
            out(self, world, me, |w| w.module_call(exit_obj, ai, ModuleCall::AiFollowPath { path: vec![end], ignore_object: us, command_source: CMD_FROM_AI }));
            let layer = out(self, world, me, |w| w.contain_terrain_get_layer_for_destination(&end));
            out(self, world, me, |w| w.contain_pathfind_update_goal(exit_obj, &end, layer));
        }
    }
}

impl OpenContainVirtuals for GarrisonContain {
    fn oc(&self) -> &OpenContain {
        &self.oc
    }
    fn oc_mut(&mut self) -> &mut OpenContain {
        &mut self.oc
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::GarrisonContain(self)
    }
    fn from_body(b: UpdateBody) -> Option<Self> {
        match b {
            UpdateBody::GarrisonContain(m) => Some(m),
            _ => None,
        }
    }

    /// `GarrisonContain::update()`.
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        open_contain::base_update(self, world, me);
        // the iterator is advanced before the rider is removed
        for contained in self.oc.contain_list().to_vec() {
            if world.objects.find(contained).is_some_and(|o| o.is_effectively_dead()) && !crate::logic::modules::plant(1614) {
                self.remove_from_contain(world, me, contained, false);
                let frame = world.frame().wrapping_add(HUGE_FRAME_IN_FUTURE);
                out(self, world, me, |w| w.contain_set_safe_occlusion_frame(contained, frame));
            }
        }
        self.match_objects_to_garrison_points(world, me);
        self.heal_objects(world, me);
        if self.data.mobile_garrison && (world.is_mobile(me.object) || crate::logic::modules::plant(1634)) {
            self.move_objects_with_me(world, me);
        }
        UPDATE_SLEEP_NONE
    }

    /// `isValidContainerFor(obj, checkCapacity)`: "Garrison has an extra check forbidding any containment if ReallyDamaged".
    fn is_valid_container_for(&mut self, world: &mut World, me: ModuleHandle, obj: ObjectId, check_capacity: bool) -> bool {
        if !open_contain::base_is_valid_container_for(self, world, me, obj, check_capacity) {
            return false;
        }
        let us = me.object;
        let Some(body) = world.get_body_module(us) else {
            undefined(world, me, Undefined::NullBodyModule);
            return false;
        };
        let health = out(self, world, me, |w| w.module_call(us, body, ModuleCall::GetHealth)).as_real(0.0);
        if health <= 0.0 {
            return false;
        }
        let state = out(self, world, me, |w| w.module_call(us, body, ModuleCall::GetDamageState)).as_int(BODY_PRISTINE);
        if state == BODY_REALLYDAMAGED && !crate::logic::modules::plant(1605) && !world.objects.find(us).is_some_and(|o| o.is_kind_of(KINDOF_GARRISONABLE_UNTIL_DESTROYED)) {
            return false;
        }
        if world.objects.find(obj).is_some_and(|o| !o.is_kind_of(KINDOF_NO_GARRISON) || crate::logic::modules::plant(1606)) {
            if check_capacity {
                let garrison_max = self.get_contain_max(world);
                let contain_count = self.get_contain_count(world) as i32;
                return if crate::logic::modules::plant(1607) { contain_count <= garrison_max } else { contain_count < garrison_max };
            }
            return true;
        }
        false
    }

    /// `isEnclosingContainerFor(obj)`: the data's.
    fn is_enclosing_container_for(&mut self, _world: &mut World, _me: ModuleHandle, _obj: ObjectId) -> bool {
        self.enclosing()
    }

    /// `isPassengerAllowedToFire(id)`: "a GarrisonContain always lets people shoot out", unless it is subdued.
    fn is_passenger_allowed_to_fire(&mut self, world: &mut World, me: ModuleHandle, _id: ObjectId) -> bool {
        !(world.objects.find(me.object).is_some_and(|o| o.is_disabled_by_type(DISABLED_SUBDUED)) && !crate::logic::modules::plant(1629))
    }

    /// `isGarrisonable()`: true.
    fn is_garrisonable(&mut self, _world: &mut World, _me: ModuleHandle) -> bool {
        true
    }
    /// `isBustable()`: TRUE.
    fn is_bustable(&self) -> bool {
        true
    }
    /// `isImmuneToClearBuildingAttacks()`: the data's.
    fn is_immune_to_clear_building_attacks(&self) -> bool {
        self.data.immune_to_clear_building_attacks != crate::logic::modules::plant(1635)
    }
    /// `isDisplayedOnControlBar()`: TRUE.
    fn is_displayed_on_control_bar(&self, _world: &World) -> bool {
        true
    }

    /// `removeAllContained(exposeStealthUnits)`: a rally point first, then everyone out, then the building's side worked out again.
    fn remove_all_contained(&mut self, world: &mut World, me: ModuleHandle, expose_stealth_units: bool) {
        if self.get_contain_count(world) > 0 && !crate::logic::modules::plant(1622) {
            self.validate_rally_point(world, me);
        }
        open_contain::base_remove_all_contained(self, world, me, expose_stealth_units);
        self.recalc_apparent_controlling_player(world, me);
    }

    /// `onSelling()`.
    fn on_selling(&mut self, world: &mut World, me: ModuleHandle) {
        if !crate::logic::modules::plant(1630) {
            self.remove_all_contained(world, me, false);
        }
        // `OpenContain::onSelling()`: "an OpenContain tells everyone to leave"
        self.order_all_passengers_to_exit(world, me, CMD_FROM_AI, false);
    }

    /// `exitObjectViaDoor(exitObj, exitDoor)`: out by the door the evacuation side picks, or from the centre.
    fn exit_object_via_door(&mut self, world: &mut World, me: ModuleHandle, exit_obj: ObjectId, _exit_door: crate::logic::module::update_module::ExitDoorType) {
        // (DEBUG_ASSERTCRASH(exitDoor == DOOR_1): compiled out)
        self.remove_from_contain(world, me, exit_obj, false);
        let us = me.object;
        let Some(o) = world.objects.find(us) else { return };
        let exit_angle = o.thing.orientation();
        let here = o.get_position();
        let major = o.geometry_info.get_major_radius();
        let minor = o.geometry_info.get_minor_radius();
        let mtx = *o.thing.transform_matrix();
        let mut start_position = here;
        let ai = world.objects.find(exit_obj).and_then(|o| o.ai);
        if let Some(ai) = ai {
            let has_loco = out(self, world, me, |w| w.module_call(exit_obj, ai, ModuleCall::AiHasCurLocomotor)).as_bool();
            if has_loco && !out(self, world, me, |w| w.contain_valid_movement_terrain(us, exit_obj, &start_position)) {
                let offset = major;
                start_position.x -= offset * trig::cos(exit_angle);
                start_position.y -= offset * trig::sin(exit_angle);
                if !out(self, world, me, |w| w.contain_valid_movement_terrain(us, exit_obj, &start_position)) {
                    let k = if crate::logic::modules::plant(1624) { 1.0 } else { 2.0 };
                    start_position.x += k * offset * trig::cos(exit_angle);
                    start_position.y += k * offset * trig::sin(exit_angle);
                    if !out(self, world, me, |w| w.contain_valid_movement_terrain(us, exit_obj, &start_position)) {
                        start_position = here;
                    }
                }
            }
        }

        if matches!(self.evac_disposition, EvacDisposition::ToLeft | EvacDisposition::ToRight) {
            let left = (self.evac_disposition == EvacDisposition::ToLeft) != crate::logic::modules::plant(1623);
            let evac_scalar = if left { 1.0 } else { -1.0 };
            let container_half_length = major;
            let container_half_width = minor;
            // "a rectangular pocket to act as the 'doorway'"
            let dx = world.random.game_logic_random_value_real(-container_half_length / 4.0, container_half_length / 4.0);
            let dy = world.random.game_logic_random_value_real(container_half_width / 2.0, container_half_width * 2.0) * evac_scalar;
            let door_position = Matrix3D::transform_vector(&mtx, &Vector3 { x: dx, y: dy, z: 0.0 });
            let wx = world.random.game_logic_random_value_real(-container_half_length, container_half_length);
            // "spread-out!"
            let walk_to_position = Matrix3D::transform_vector(&mtx, &Vector3 { x: wx, y: container_half_width * 10.0 * evac_scalar, z: 0.0 });
            start_position = Coord3D { x: door_position.x, y: door_position.y, z: door_position.z };
            let end_position = Coord3D { x: walk_to_position.x, y: walk_to_position.y, z: walk_to_position.z };
            out(self, world, me, |w| w.contain_set_position(exit_obj, &start_position));
            out(self, world, me, |w| w.contain_set_orientation(exit_obj, exit_angle));
            self.exit_walk(world, me, exit_obj, ai, end_position);
        } else {
            // "must be EVAC_BURST_FROM_CENTER. then!"
            if self.enclosing() {
                // "correct for non-ground-level station points"
                out(self, world, me, |w| w.contain_set_position(exit_obj, &start_position));
                let z = out(self, world, me, |w| w.terrain_get_ground_height(start_position.x, start_position.y));
                out(self, world, me, |w| w.contain_set_position_z(exit_obj, z));
            }
            out(self, world, me, |w| w.contain_set_orientation(exit_obj, exit_angle));
            self.exit_walk(world, me, exit_obj, ai, start_position);
        }
        self.recalc_apparent_controlling_player(world, me);
    }

    /// `onContaining(obj, wasSelected)`: held, the building can attack, the rider fights garrisoned.
    fn on_containing(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId, was_selected: bool) {
        open_contain::base_on_containing(self, world, me, rider, was_selected);
        let us = me.object;
        out(self, world, me, |w| w.contain_set_disabled(rider, DISABLED_HELD));
        out(self, world, me, |w| w.set_status(us, status_mask(OBJECT_STATUS_CAN_ATTACK), true));
        if !crate::logic::modules::plant(1625) {
            out(self, world, me, |w| w.set_weapon_bonus_condition(rider, crate::logic::weapon_bonus::condition::GARRISONED));
        }
        if self.enclosing() {
            let pos = world.objects.find(us).map_or(Coord3D { x: 0.0, y: 0.0, z: 0.0 }, |o| o.get_position());
            out(self, world, me, |w| w.contain_set_position(rider, &pos));
        }
        let Some(player) = world.get_controlling_player(rider) else {
            undefined(world, me, Undefined::NullControllingPlayer);
            return;
        };
        if !crate::logic::modules::plant(1626) {
            out(self, world, me, |w| w.contain_record_building_garrisoned(player));
        }
        self.recalc_apparent_controlling_player(world, me);
        if world.has_drawable(rider) && world.reads.local_selection.contains(&rider) {
            world.emit(ObjectClientEvent::UiDeselectDrawable { object_id: rider });
        }
    }

    /// `onRemoving(obj)`: off its point (or station), let go; the last one out gives the building back.
    fn on_removing(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        open_contain::base_on_removing(self, world, me, rider);
        let us = me.object;
        if self.enclosing() {
            self.remove_object_from_garrison_point(world, me, rider, None);
        } else {
            self.remove_object_from_station_point(rider);
            let pos = world.objects.find(rider).map_or(Coord3D { x: 0.0, y: 0.0, z: 0.0 }, |o| o.get_position());
            let z = out(self, world, me, |w| w.terrain_get_ground_height(pos.x, pos.y));
            out(self, world, me, |w| w.contain_set_position_z(rider, z));
        }
        out(self, world, me, |w| w.clear_weapon_bonus_condition(rider, crate::logic::weapon_bonus::condition::GARRISONED));
        out(self, world, me, |w| w.contain_clear_disabled(rider, DISABLED_HELD));

        let count = self.get_contain_count(world);
        if count == 0 {
            if world.get_team(us).is_some() && !crate::logic::modules::plant(1627) {
                let original = self.original_team;
                out(self, world, me, |w| w.contain_set_team_opt(us, original));
                self.original_team = None;
            }
            out(self, world, me, |w| w.clear_status(us, status_mask(OBJECT_STATUS_CAN_ATTACK)));
            self.hide_garrisoned_state_from_nonallies = false;
            if world.has_drawable(us) {
                out(self, world, me, |w| w.contain_drawable_model_condition(us, MODELCONDITION_GARRISONED, false));
            }
        } else if self.oc.stealth_units_contained() != count {
            self.hide_garrisoned_state_from_nonallies = false;
        }
        let delay = if crate::logic::modules::plant(1628) { 0 } else { world.objects.find(rider).map_or(0, |o| o.template().get_occlusion_delay()) };
        let frame = world.frame().wrapping_add(delay);
        out(self, world, me, |w| w.contain_set_safe_occlusion_frame(rider, frame));
        self.recalc_apparent_controlling_player(world, me);
    }

    /// `onBodyDamageStateChange(info, oldState, newState)`: everyone out when the building is really damaged.
    fn on_body_damage_state_change(&mut self, world: &mut World, me: ModuleHandle, _info: &DamageInfo, _old_state: i32, new_state: i32) {
        if new_state == BODY_REALLYDAMAGED && (crate::logic::modules::plant(1636) || !world.objects.find(me.object).is_some_and(|o| o.is_kind_of(KINDOF_GARRISONABLE_UNTIL_DESTROYED))) && self.get_contain_count(world) > 0 {
            self.order_all_passengers_to_exit(world, me, CMD_FROM_AI, false);
        }
    }

    /// `redeployOccupants()`: everyone placed again, keeping the frame each was placed at.
    fn redeploy_occupants(&mut self, world: &mut World, me: ModuleHandle) {
        let copy = *self.garrison_point_data;
        self.station_garrison_points_initialized = false;
        self.match_objects_to_garrison_points(world, me);
        for p in copy.iter() {
            match p.object.deref() {
                Ok(Some(obj)) => {
                    let index = self.get_object_garrison_point_index(obj);
                    if index != GARRISON_INDEX_INVALID && !crate::logic::modules::plant(1611) {
                        self.garrison_point_data[index as usize].place_frame = p.place_frame;
                    }
                }
                Ok(None) => {}
                // a loaded id: no object is at it (the pointer is compared, never read)
                Err(()) => {}
            }
        }
    }

    /// `onObjectCreated()`: the initial roster, made with the controlling player's default team.
    fn on_object_created(&mut self, world: &mut World, me: ModuleHandle) {
        let count = self.data.initial_roster_count;
        let name = self.data.initial_roster_template_name.clone();
        let roster_template = world.thing_factory_find_template(&name);
        let us = me.object;
        for _ in 0..count {
            let Some(player) = world.get_controlling_player(us) else {
                undefined(world, me, Undefined::NullControllingPlayer);
                return;
            };
            let team = world.cuts.player_default_team(player);
            let Some(template) = roster_template else {
                // `newObject(NULL)` throws ERROR_BAD_ARG
                world.cuts.module_errors.push((me, IniError::new(IniErrorKind::BadArg)));
                return;
            };
            let payload = out(self, world, me, |w| w.contain_new_payload_object(template, team, ObjectStatusMaskType::new()));
            if self.is_valid_container_for(world, me, payload, true) {
                self.add_to_contain(world, me, payload);
            }
            // else DEBUG_CRASH: "PutInContainer is full, or not valid for the payload" (nothing in a release build)
        }
    }

    /// `setEvacDisposition(disp)`, `recalcApparentControllingPlayer()` and `getApparentControllingPlayer(observer)`.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::SetEvacDisposition(d) => {
                self.evac_disposition = match d {
                    1 => EvacDisposition::ToLeft,
                    2 => EvacDisposition::ToRight,
                    3 => EvacDisposition::BurstFromCenter,
                    _ => EvacDisposition::Invalid,
                };
                ModuleReply::Done
            }
            ModuleCall::RecalcApparentControllingPlayer => {
                self.recalc_apparent_controlling_player(world, me);
                ModuleReply::Done
            }
            ModuleCall::GetApparentControllingPlayer { observer } => {
                let observer = (observer >= 0).then_some(observer as PlayerIndex);
                ModuleReply::Int(self.get_apparent_controlling_player(world, me, observer).map_or(-1, |p| p))
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl UpdateBehavior for GarrisonContain {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        self.oc.update_module()
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        OpenContainVirtuals::update(self, world, me)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::GarrisonContain(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::GarrisonContain(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, _world: &mut World, _me: ModuleHandle, _call: ModuleCall) -> ModuleReply {
        ModuleReply::Unhandled
    }
}

impl Snapshot for GarrisonContain {
    /// `crc`: "extend base class".
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        Snapshot::crc(&mut self.oc, xfer)
    }

    /// `xfer` (version 1): the version, `OpenContain::xfer`, then this class's fields.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        Snapshot::xfer(&mut self.oc, xfer)?;
        self.xfer_own_fields(xfer)
    }

    /// `loadPostProcess`: the base's (see [`GarrisonContain::load_post_process_with`]).
    fn load_post_process(&mut self) {
        Snapshot::load_post_process(&mut self.oc);
    }
}
