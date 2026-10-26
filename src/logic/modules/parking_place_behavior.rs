// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ParkingPlaceBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/ParkingPlaceBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ParkingPlaceBehavior`: an airfield. Its `NumRows` x `NumCols` parking spaces (each with a hangar start, a parking spot and a runway
//! prep point, read off the art's bones `Runway<c>Park<r>Han`, `Runway<c>Parking<r>` and `Runway<c>Prep<r>`, and its own exit door) and,
//! with `HasRunways`, one runway per column (`RunwayStart<c>`, `RunwayEnd<c>`) are built the first time they are needed once the airfield is
//! finished. It is the exit of the planes it produces (a door is a space reserved for exit; a plane produced at the helipad needs no door and
//! comes out at the bone `HeliPark01`), it hands the jets their spaces and runways (`ParkingPlaceBehaviorInterface`: one jet in line for
//! take-off behind the one using a runway), heals the planes that ask (`HealAmountPerSecond`, every sixth of a second), and kills or
//! defects the parked planes with it.
//!
//! # Upstream facts kept
//!
//! - `update()` is always awake ("the most expedient way is to ensure our update is run every frame"): the constructor wakes it now and it
//!   never sleeps; it keeps the spaces built and the dead purged, and heals when the heal frame comes.
//! - A missing bone gives the object's position and transform (`getSingleLogicalBonePosition`'s fallback), so the space is there all the
//!   same.
//! - `calcPPInfo` reads the space's runway unchecked: an airfield without runways that hands out a space is undefined
//!   (`Undefined::ArrayIndexOutOfRange`), as is `reserveRunway` and `getRunwayReservation` of a runway it does not have. A healee with no
//!   body (`Undefined::NullBodyModule`), a reserved door for a `NULL` template (`Undefined::NullObject`), and an exit whose space could
//!   not be reserved (`exitObjectViaDoor` then places the plane at an uninitialised hangar position: `Undefined::UninitialisedLocal`)
//!   are undefined too.
//! - `buildInfo` reserves `NumRows * NumCols` spaces, then `NumCols` runways: a negative count throws `std::length_error`
//!   (`IniErrorKind::LengthError`), the spaces' before anything is built, the runways' after the spaces are, and the call that built aborts.
//! - The xfer is version 3: the base, the spaces' occupants (a load builds the spaces after the version and the base, and drops the
//!   entries beyond them), the runways', the healees, the helicopter rally point and the next heal frame. Before version 3 a load wakes the
//!   heal at once. The world stages the load's build before the xfer (it reads the bones) and releases its doors after.
//!
//! The parked jets' `JetAIUpdate` (found by name) is layer 10's: its two answers are calls. The art's bones are the world's
//! ([`World::object_get_single_logical_bone_transform`]).
//!
//! `p1w13-sonnet`, batch R74, 2026-10-26.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::KINDOF_PRODUCED_AT_HELIPAD;
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DIE};
use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_DECK_HEIGHT_OFFSET, OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_HEALING, DAMAGE_UNRESISTABLE, DEATH_NONE, DEATH_NORMAL};
use crate::logic::module::update_module::{
    ExitDoorType, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, DOOR_NONE_AVAILABLE, DOOR_NONE_NEEDED, UPDATE_SLEEP_NONE,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::TeamId;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply, PPInfo};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `FOREVER`.
const FOREVER: u32 = 0x3fff_ffff;
/// `HEAL_RATE_FRAMES` ("don't really need to autoheal every frame...").
const HEAL_RATE_FRAMES: u32 = (LOGICFRAMES_PER_SECOND / 5) as u32;
/// `SECONDS_PER_LOGICFRAME_REAL`.
const SECONDS_PER_LOGICFRAME_REAL: f32 = 1.0 / LOGICFRAMES_PER_SECOND as f32;
/// `CMD_FROM_AI`.
const CMD_FROM_AI: i32 = 2;

/// `class ParkingPlaceBehaviorModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct ParkingPlaceBehaviorModuleData {
    pub update: UpdateModuleData,
    /// `m_healAmount` (per second).
    pub heal_amount: f32,
    /// `m_numRows`, `m_numCols`.
    pub num_rows: i32,
    pub num_cols: i32,
    /// `m_approachHeight`, `m_landingDeckHeightOffset`.
    pub approach_height: f32,
    pub landing_deck_height_offset: f32,
    /// `m_hasRunways` ("if true, each col has a runway in front of it"), `m_parkInHangars` ("park at the hangar production spot").
    pub has_runways: bool,
    pub park_in_hangars: bool,
}

impl Default for ParkingPlaceBehaviorModuleData {
    /// `ParkingPlaceBehaviorModuleData()`.
    fn default() -> Self {
        ParkingPlaceBehaviorModuleData {
            update: UpdateModuleData::new(),
            heal_amount: 0.0,
            num_rows: 0,
            num_cols: 0,
            approach_height: 0.0,
            landing_deck_height_offset: 0.0,
            has_runways: false,
            park_in_hangars: false,
        }
    }
}

static FIELD_PARSE: [FieldParse<ParkingPlaceBehaviorModuleData>; 8] = [
    FieldParse::new("NumRows", |i, d, _| { d.num_rows = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("NumCols", |i, d, _| { d.num_cols = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ApproachHeight", |i, d, _| { d.approach_height = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("LandingDeckHeightOffset", |i, d, _| { d.landing_deck_height_offset = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("HasRunways", |i, d, _| { d.has_runways = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("ParkInHangars", |i, d, _| { d.park_in_hangars = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("HealAmountPerSecond", |i, d, _| { d.heal_amount = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl ParkingPlaceBehaviorModuleData {
    /// `buildFieldParse(p)`: `UpdateModuleData`'s (`BehaviorModuleData`'s: nothing), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, ParkingPlaceBehaviorModuleData>) -> IniResult<()> {
        crate::logic::module::behavior_module::BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ParkingPlaceBehaviorModuleData> {
        let mut data = ParkingPlaceBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, ParkingPlaceBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `ParkingPlaceBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::ParkingPlaceBehavior(Arc::new(ParkingPlaceBehaviorModuleData::friend_new_module_data(ini)?)))
}

/// `struct ParkingPlaceInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct ParkingPlaceInfo {
    hangar_start: Coord3D,
    hangar_start_orient: f32,
    location: Coord3D,
    prep: Coord3D,
    orientation: f32,
    runway: i32,
    door: ExitDoorType,
    object_in_space: ObjectId,
    reserved_for_exit: bool,
}

/// `struct RunwayInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct RunwayInfo {
    start: Coord3D,
    end: Coord3D,
    in_use_by: ObjectId,
    next_in_line_for_takeoff: ObjectId,
    was_in_line: bool,
}

/// `struct HealingInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct HealingInfo {
    getting_healed_id: ObjectId,
    heal_start_frame: u32,
}

/// `class ParkingPlaceBehavior : public UpdateModule, public DieModuleInterface, public ParkingPlaceBehaviorInterface, public ExitInterface`.
#[derive(Clone, Debug)]
pub struct ParkingPlaceBehavior {
    base: UpdateModule,
    data: Arc<ParkingPlaceBehaviorModuleData>,
    /// `m_spaces`, `m_runways`, `m_healing`.
    spaces: Vec<ParkingPlaceInfo>,
    runways: Vec<RunwayInfo>,
    healing: Vec<HealingInfo>,
    /// `m_nextHealFrame`, `m_gotInfo`.
    next_heal_frame: u32,
    got_info: bool,
    /// `m_heliRallyPoint`, `m_heliRallyPointExists`.
    heli_rally_point: Coord3D,
    heli_rally_point_exists: bool,
    /// The load's `buildInfo()`, staged by the world before the xfer (it reads the bones) and applied where upstream runs it, after the
    /// version and `UpdateModule::xfer`; then the doors whose hold the world releases after the xfer (`None`: the build was not applied).
    staged_build: Option<StagedBuild>,
    applied_holds: Option<(Option<u16>, Vec<ExitDoorType>)>,
    staged_error: Option<IniError>,
}

/// What `buildInfo()` makes: the spaces, the runways and the production update whose doors it releases.
#[derive(Clone, Debug)]
struct StagedBuild {
    spaces: Vec<ParkingPlaceInfo>,
    runways: Vec<RunwayInfo>,
    production: Option<u16>,
    /// The spaces' `reserve` throws (nothing is built), or the runways' (after the spaces).
    spaces_throw: bool,
    runways_throw: bool,
}

impl PartialEq for ParkingPlaceBehavior {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.base.behavior().object() == other.base.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl ParkingPlaceBehavior {
    /// `getInterfaceMask()`: `UpdateModule`'s and the die interface.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE
    }

    /// The constructor's first half, which touches no world (`INI_INVALID_DATA` for null data); the second wakes the update now.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<ParkingPlaceBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = UpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        Ok(ParkingPlaceBehavior {
            base,
            data,
            spaces: Vec::new(),
            runways: Vec::new(),
            healing: Vec::new(),
            next_heal_frame: FOREVER,
            got_info: false,
            heli_rally_point: Coord3D::default(),
            heli_rally_point_exists: false,
            staged_build: None,
            applied_holds: None,
            staged_error: None,
        })
    }

    pub fn update_module(&self) -> &UpdateModule {
        &self.base
    }

    pub fn module_data(&self) -> &Arc<ParkingPlaceBehaviorModuleData> {
        &self.data
    }

    /// The flags, the next heal frame, the rally point, then each space (its places, orientations, runway, door, occupant and exit
    /// reservation), each runway and each healee.
    pub fn state_words(&self) -> Vec<u32> {
        let b = |x: f32| x.to_bits();
        let c = |p: Coord3D| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()];
        let mut v = vec![u32::from(self.got_info), self.next_heal_frame, u32::from(self.heli_rally_point_exists)];
        v.extend(c(self.heli_rally_point));
        v.push(self.spaces.len() as u32);
        for s in &self.spaces {
            v.extend(c(s.hangar_start));
            v.push(b(s.hangar_start_orient));
            v.extend(c(s.location));
            v.extend(c(s.prep));
            v.extend([b(s.orientation), s.runway as u32, s.door as u32, s.object_in_space as u32, u32::from(s.reserved_for_exit)]);
        }
        v.push(self.runways.len() as u32);
        for r in &self.runways {
            v.extend(c(r.start));
            v.extend(c(r.end));
            v.extend([r.in_use_by as u32, r.next_in_line_for_takeoff as u32, u32::from(r.was_in_line)]);
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

    /// The bone's position and its transform's rotation about z (`getSingleLogicalBonePosition(name, &pos, &mtx)` and
    /// `mtx.Get_Z_Rotation()`).
    fn bone(world: &World, us: ObjectId, name: &str) -> (Coord3D, f32) {
        let (_, pos, mtx) = world.object_get_single_logical_bone_transform(us, name.as_bytes());
        (pos, mtx.get_z_rotation())
    }

    /// `buildInfo()`: the spaces and runways, once, when the airfield is finished. A negative count makes a `reserve` throw
    /// `std::length_error` (`IniErrorKind::LengthError`): the spaces' before anything is built, the runways' after the spaces are.
    pub fn build_info(&mut self, world: &mut World, us: ObjectId) -> Result<(), IniError> {
        if let Some(b) = self.make_build(world, us) {
            let production = b.production;
            let (doors, r) = self.apply_build(b);
            Self::release_holds(world, us, production, &doors);
            r?;
        }
        Ok(())
    }

    /// The world's load, before the xfer: stage what `buildInfo()` would make, for the xfer to apply after the version and the base
    /// ("first, build our info, so it won't be overwritten later"); a load that fails before then builds nothing.
    pub fn stage_build_info(&mut self, world: &mut World, us: ObjectId) {
        self.staged_build = self.make_build(world, us);
        self.applied_holds = None;
        self.staged_error = None;
    }

    /// The world's load, after the xfer: the doors a staged build released (`setHoldDoorOpen(door, false)`), if the xfer applied it, and
    /// what the build threw (the xfer stopped there, with `XferStatus::ErrorUnknown`).
    pub fn finish_staged_build(&mut self, world: &mut World, us: ObjectId) -> Option<IniError> {
        self.staged_build = None;
        if let Some((pu, doors)) = self.applied_holds.take() {
            Self::release_holds(world, us, pu, &doors);
        }
        self.staged_error.take()
    }

    fn release_holds(world: &mut World, us: ObjectId, pu: Option<u16>, doors: &[ExitDoorType]) {
        if let Some(pu) = pu {
            for &door in doors {
                world.module_call(us, pu, ModuleCall::SetHoldDoorOpen { door, hold: false });
            }
        }
    }

    /// The spaces and runways `buildInfo()` makes, reading the bones (`None`: built already, or the airfield unfinished or sold).
    fn make_build(&self, world: &mut World, us: ObjectId) -> Option<StagedBuild> {
        if self.got_info {
            return None;
        }
        let o = world.objects.find(us)?;
        if o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION) || o.test_status(OBJECT_STATUS_SOLD) {
            return None;
        }
        let d = self.data.clone();
        let production = world.get_production_update_interface(us);
        // `m_spaces.reserve(d->m_numRows * d->m_numCols)`, the `Int` converted to `size_t`
        let spaces_throw = d.num_rows.wrapping_mul(d.num_cols) < 0;
        let mut door: ExitDoorType = 0;
        let mut spaces = Vec::new();
        for row in 0..if spaces_throw { 0 } else { d.num_rows } {
            for col in 0..d.num_cols {
                let (hangar_start, hangar_start_orient) = Self::bone(world, us, &format!("Runway{}Park{}Han", col + 1, row + 1));
                let (location, orientation) = Self::bone(world, us, &format!("Runway{}Parking{}", col + 1, row + 1));
                let (prep, _) = Self::bone(world, us, &format!("Runway{}Prep{}", col + 1, row + 1));
                spaces.push(ParkingPlaceInfo {
                    hangar_start,
                    hangar_start_orient,
                    location,
                    prep,
                    orientation,
                    runway: if crate::logic::modules::plant(2450) { 0 } else { col },
                    door,
                    object_in_space: INVALID_ID,
                    reserved_for_exit: false,
                });
                door += 1;
            }
        }
        // `m_runways.reserve(d->m_numCols)`
        let runways_throw = !spaces_throw && d.has_runways && d.num_cols < 0;
        let mut runways = Vec::new();
        if d.has_runways && !spaces_throw {
            for col in 0..d.num_cols {
                let (start, _) = Self::bone(world, us, &format!("RunwayStart{}", col + 1));
                let (end, _) = Self::bone(world, us, &format!("RunwayEnd{}", col + 1));
                runways.push(RunwayInfo { start, end, in_use_by: INVALID_ID, next_in_line_for_takeoff: INVALID_ID, was_in_line: false });
            }
        }
        Some(StagedBuild { spaces, runways, production, spaces_throw, runways_throw })
    }

    /// Keep a build, as far as it gets: the doors whose hold it releases, and what it threw.
    fn apply_build(&mut self, b: StagedBuild) -> (Vec<ExitDoorType>, Result<(), IniError>) {
        let thrown = || Err(IniError::new(IniErrorKind::LengthError));
        if b.spaces_throw {
            return (Vec::new(), thrown());
        }
        let doors = b.spaces.iter().map(|s| s.door).collect();
        self.spaces.extend(b.spaces);
        if b.runways_throw {
            return (doors, thrown());
        }
        self.runways.extend(b.runways);
        self.got_info = true;
        (doors, Ok(()))
    }

    fn dead_or_gone(world: &World, id: ObjectId) -> bool {
        world.objects.find(id).is_none_or(|o| o.is_effectively_dead())
    }

    /// `purgeDead()`: the spaces, runways and healees of the dead let go.
    fn purge_dead(&mut self, world: &mut World, us: ObjectId) -> Result<(), IniError> {
        self.build_info(world, us)?;
        let pu = world.get_production_update_interface(us);
        for i in 0..self.spaces.len() {
            let s = self.spaces[i];
            if s.object_in_space != INVALID_ID && Self::dead_or_gone(world, s.object_in_space) {
                self.spaces[i].object_in_space = INVALID_ID;
                self.spaces[i].reserved_for_exit = false;
                if let Some(pu) = pu {
                    world.module_call(us, pu, ModuleCall::SetHoldDoorOpen { door: s.door, hold: false });
                }
            }
        }
        for r in self.runways.iter_mut() {
            if r.in_use_by != INVALID_ID && Self::dead_or_gone(world, r.in_use_by) {
                r.in_use_by = INVALID_ID;
                r.was_in_line = false;
            }
            if r.next_in_line_for_takeoff != INVALID_ID && Self::dead_or_gone(world, r.next_in_line_for_takeoff) && !crate::logic::modules::plant(2451) {
                r.next_in_line_for_takeoff = INVALID_ID;
            }
        }
        // (upstream's loop never advances past an entry whose id is INVALID_ID: none is ever added)
        let before = self.healing.len();
        self.healing.retain(|h| h.getting_healed_id == INVALID_ID || !Self::dead_or_gone(world, h.getting_healed_id));
        if self.healing.len() != before {
            self.reset_wake_frame(world);
        }
        Ok(())
    }

    /// `hasReservedSpace(id)` ("called from client, so MUST NOT modify self").
    fn has_reserved_space(&self, id: ObjectId) -> bool {
        if !self.got_info {
            return false;
        }
        // "shouldn't call this way, but Weapon mistakenly does sometimes, so check for it"
        if id == INVALID_ID {
            return false;
        }
        self.spaces.iter().any(|s| s.object_in_space == id)
    }

    /// `getSpaceIndex(id)`.
    fn get_space_index(&self, id: ObjectId) -> i32 {
        if id == INVALID_ID {
            return -1;
        }
        self.spaces.iter().position(|s| s.object_in_space == id).map_or(-1, |i| i as i32)
    }

    /// `findPPI(id)`.
    fn find_ppi(&self, id: ObjectId) -> Option<usize> {
        if !self.got_info || id == INVALID_ID {
            return None;
        }
        self.spaces.iter().position(|s| s.object_in_space == id)
    }

    /// `findEmptyPPI()`.
    fn find_empty_ppi(&self) -> Option<usize> {
        if !self.got_info {
            return None;
        }
        self.spaces.iter().position(|s| s.object_in_space == INVALID_ID && !s.reserved_for_exit)
    }

    fn produced_at_helipad(world: &World, template: u16) -> bool {
        world.statics.thing_factory.find_by_template_id(template).is_some_and(|t| t.data.is_kind_of(KINDOF_PRODUCED_AT_HELIPAD))
    }

    /// `hasAvailableSpaceFor(thing)` ("since this is const, and we can't purge the dead safely, just peek").
    fn has_available_space_for(&self, world: &World, template: u16) -> bool {
        // "degenerate case, shouldn't happen, but just in case..."
        if !self.got_info {
            return false;
        }
        if Self::produced_at_helipad(world, template) {
            return true;
        }
        self.spaces.iter().any(|s| {
            let id = if s.object_in_space != INVALID_ID && Self::dead_or_gone(world, s.object_in_space) { INVALID_ID } else { s.object_in_space };
            id == INVALID_ID && !s.reserved_for_exit
        })
    }

    fn deck_height_mask() -> ObjectStatusMaskType {
        let mut m = ObjectStatusMaskType::default();
        m.set(OBJECT_STATUS_DECK_HEIGHT_OFFSET, 1);
        m
    }

    /// `reserveSpace(id, parkingOffset, info)`: whether a space was had; the info, if asked for (`None`: the caller's left alone).
    fn reserve_space(&mut self, world: &mut World, us: ObjectId, id: ObjectId, parking_offset: f32, want_info: bool) -> Result<(bool, Option<PPInfo>), IniError> {
        self.build_info(world, us)?;
        self.purge_dead(world, us)?;
        let d = self.data.clone();
        let Some(i) = self.find_ppi(id).or_else(|| self.find_empty_ppi()) else {
            // "No parking places!"
            return Ok((false, None));
        };
        self.spaces[i].object_in_space = id;
        self.spaces[i].reserved_for_exit = false;
        if d.landing_deck_height_offset != 0.0 && world.objects.find(id).is_some() {
            world.body_set_status(id, Self::deck_height_mask(), true);
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
        if let Some(pu) = world.get_production_update_interface(us) {
            world.module_call(us, pu, ModuleCall::SetHoldDoorOpen { door: self.spaces[i].door, hold: true });
        }
        Ok((true, out))
    }

    /// `calcPPInfo(id, info)`: `info` filled from the space and its runway (the space not found: "Utter failure", `info` as it was).
    fn calc_pp_info(&self, id: ObjectId, mut info: PPInfo) -> Result<PPInfo, IniError> {
        let Some(i) = self.find_ppi(id) else { return Ok(info) };
        let ppi = self.spaces[i];
        let rr = *self.runways.get(ppi.runway as usize).ok_or_else(|| IniError::undefined(Undefined::ArrayIndexOutOfRange))?;
        let d = &self.data;
        info.parking_space = if d.park_in_hangars { ppi.hangar_start } else { ppi.location };
        info.runway_prep = ppi.prep;
        info.parking_orientation = if d.park_in_hangars { ppi.hangar_start_orient } else { ppi.orientation };
        info.runway_start = rr.start;
        info.runway_end = rr.end;
        info.runway_approach = rr.end;
        // "no, too short, planes may not have enough space to drop altitude"
        let approach_dist = if crate::logic::modules::plant(2452) { 0.5f32 } else { 0.75f32 };
        info.runway_approach.x += (rr.end.x - rr.start.x) * approach_dist;
        info.runway_approach.y += (rr.end.y - rr.start.y) * approach_dist;
        info.runway_approach.z = rr.end.z + d.approach_height + d.landing_deck_height_offset;
        info.runway_exit = info.runway_approach;
        info.hangar_internal = ppi.hangar_start;
        info.hangar_internal_orient = ppi.hangar_start_orient;
        // "Cache the runway's takeoff distance used by JetAIUpdate for calculating lift."
        let mut vector = info.runway_start;
        vector.sub(&info.runway_end);
        info.runway_takeoff_dist = vector.length();
        for r in &self.runways {
            if r.in_use_by == id && r.was_in_line {
                info.runway_start = info.runway_prep;
            }
        }
        Ok(info)
    }

    /// `releaseSpace(id)`.
    fn release_space(&mut self, world: &mut World, us: ObjectId, id: ObjectId) -> Result<(), IniError> {
        self.build_info(world, us)?;
        self.purge_dead(world, us)?;
        let pu = world.get_production_update_interface(us);
        for i in 0..self.spaces.len() {
            if self.spaces[i].object_in_space == id {
                self.spaces[i].object_in_space = INVALID_ID;
                self.spaces[i].reserved_for_exit = false;
                if let Some(pu) = pu {
                    world.module_call(us, pu, ModuleCall::SetHoldDoorOpen { door: self.spaces[i].door, hold: false });
                }
            }
        }
        if world.objects.find(id).is_some() {
            world.body_set_status(id, Self::deck_height_mask(), false);
        }
        Ok(())
    }

    /// `getRunwayReservation(runway, type)` ("We don't care about type").
    fn get_runway_reservation(&mut self, world: &mut World, us: ObjectId, runway: i32) -> Result<ObjectId, IniError> {
        self.build_info(world, us)?;
        self.purge_dead(world, us)?;
        let r = usize::try_from(runway).ok().and_then(|r| self.runways.get(r)).ok_or_else(|| IniError::undefined(Undefined::ArrayIndexOutOfRange))?;
        Ok(r.in_use_by)
    }

    /// `transferRunwayReservationToNextInLineForTakeoff(id)`.
    fn transfer_runway_reservation(&mut self, world: &mut World, us: ObjectId, id: ObjectId) -> Result<(), IniError> {
        self.build_info(world, us)?;
        self.purge_dead(world, us)?;
        for r in self.runways.iter_mut() {
            if r.in_use_by == id && r.next_in_line_for_takeoff != INVALID_ID {
                r.in_use_by = r.next_in_line_for_takeoff;
                r.was_in_line = true;
                r.next_in_line_for_takeoff = INVALID_ID;
            }
        }
        Ok(())
    }

    /// `reserveRunway(id, forLanding)`.
    fn reserve_runway(&mut self, world: &mut World, us: ObjectId, id: ObjectId, for_landing: bool) -> Result<bool, IniError> {
        self.build_info(world, us)?;
        self.purge_dead(world, us)?;
        let Some(runway) = self.spaces.iter().find(|s| s.object_in_space == id).map(|s| s.runway) else {
            // "only planes with reserved spaces can reserve runways"
            return Ok(false);
        };
        let info = usize::try_from(runway).ok().and_then(|r| self.runways.get_mut(r)).ok_or_else(|| IniError::undefined(Undefined::ArrayIndexOutOfRange))?;
        if info.in_use_by == id {
            Ok(true)
        } else if info.in_use_by == INVALID_ID {
            info.in_use_by = id;
            if info.next_in_line_for_takeoff == id {
                info.next_in_line_for_takeoff = INVALID_ID;
                info.was_in_line = true;
            } else {
                info.was_in_line = false;
            }
            Ok(true)
        } else if !for_landing && info.next_in_line_for_takeoff == INVALID_ID && !crate::logic::modules::plant(2453) {
            info.next_in_line_for_takeoff = id;
            // "yes, that's right"
            Ok(false)
        } else {
            Ok(false)
        }
    }

    /// `releaseRunway(id)`.
    fn release_runway(&mut self, world: &mut World, us: ObjectId, id: ObjectId) -> Result<(), IniError> {
        self.build_info(world, us)?;
        self.purge_dead(world, us)?;
        for r in self.runways.iter_mut() {
            if r.in_use_by == id {
                r.in_use_by = INVALID_ID;
                r.was_in_line = false;
            }
            if r.next_in_line_for_takeoff == id {
                r.next_in_line_for_takeoff = INVALID_ID;
            }
        }
        Ok(())
    }

    /// `resetWakeFrame()`: the next heal a sixth of a second away, or never with no one to heal.
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

    /// The parked jet's `JetAIUpdate` (`findUpdateModule("JetAIUpdate")`): whether a takeoff or a landing is in progress.
    fn takeoff_or_landing(world: &mut World, obj: ObjectId) -> bool {
        match world.find_module(obj, b"JetAIUpdate") {
            Some(ju) => world.module_call(obj, ju, ModuleCall::JetIsTakeoffOrLandingInProgress).as_bool(),
            None => false,
        }
    }

    /// `defectAllParkedUnits(newTeam, detectionTime)`: a plane in the air loses its space if the team is another player's; a parked one
    /// defects with the airfield.
    fn defect_all_parked_units(&mut self, world: &mut World, us: ObjectId, new_team: TeamId, detection_time: u32) -> Result<(), IniError> {
        self.build_info(world, us)?;
        self.purge_dead(world, us)?;
        for i in 0..self.spaces.len() {
            let id = self.spaces[i].object_in_space;
            if id == INVALID_ID || Self::dead_or_gone(world, id) {
                continue;
            }
            // "srj sez: evil. fix better someday."
            let takeoff_or_landing = Self::takeoff_or_landing(world, id);
            if world.is_above_terrain(id) && !takeoff_or_landing {
                // "if the new team is a different controlling player, this guys loses his space."
                let new_player = world.cuts.team_players.get(&new_team).copied();
                if new_player != world.get_controlling_player(id) {
                    self.release_space(world, us, id)?;
                    if world.objects.find(id).is_some_and(|o| o.producer_id == us) && !crate::logic::modules::plant(2454) {
                        world.set_producer(id, INVALID_ID);
                    }
                }
            } else {
                world.body_defect(id, new_team, detection_time);
            }
        }
        self.purge_dead(world, us)?;
        Ok(())
    }

    /// `killAllParkedUnits()`: the parked planes, not those in the air.
    fn kill_all_parked_units(&mut self, world: &mut World, us: ObjectId) -> Result<(), IniError> {
        self.build_info(world, us)?;
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
            // (`Object::kill` reads the body unchecked)
            if world.get_body_module(id).is_none() {
                return Err(IniError::undefined(Undefined::NullBodyModule));
            }
            world.kill(id, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
        }
        self.purge_dead(world, us)?;
        Ok(())
    }

    /// `update()`'s body: the spaces kept, and the healees healed when the heal frame comes.
    fn do_update(&mut self, world: &mut World, us: ObjectId) -> Result<UpdateSleepTime, IniError> {
        self.build_info(world, us)?;
        self.purge_dead(world, us)?;
        let now = world.frame();
        if now >= self.next_heal_frame {
            self.next_heal_frame = now.wrapping_add(HEAL_RATE_FRAMES);
            let d = self.data.clone();
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
        Ok(UPDATE_SLEEP_NONE)
    }

    /// `reserveDoorForExit(objType, specificObject)`: no door for a plane of the helipad; else a free space, reserved for exit.
    fn reserve_door_for_exit(&mut self, world: &mut World, us: ObjectId, template: Option<u16>) -> Result<ExitDoorType, IniError> {
        self.build_info(world, us)?;
        self.purge_dead(world, us)?;
        let template = template.ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
        if Self::produced_at_helipad(world, template) {
            return Ok(DOOR_NONE_NEEDED);
        }
        // "this never allocates a per-unit parking space, even if specificObject is null"
        if let Some(i) = self.find_empty_ppi() {
            self.spaces[i].object_in_space = INVALID_ID;
            self.spaces[i].reserved_for_exit = !crate::logic::modules::plant(2455);
            return Ok(self.spaces[i].door);
        }
        Ok(DOOR_NONE_AVAILABLE)
    }

    /// `exitObjectViaDoor(newObj, exitDoor)`: the plane takes its space and is placed in the hangar (or at the helipad), then heads for
    /// its parking spot (or the rally point).
    fn exit_object_via_door(&mut self, world: &mut World, us: ObjectId, new_obj: ObjectId, exit_door: ExitDoorType) -> Result<(), IniError> {
        if exit_door != DOOR_NONE_NEEDED {
            let Some(i) = self.spaces.iter().position(|s| s.object_in_space == INVALID_ID && s.reserved_for_exit && s.door == exit_door) else {
                // "could not find the space. what?"
                return Ok(());
            };
            self.spaces[i].object_in_space = new_obj;
            self.spaces[i].reserved_for_exit = false;
        }
        // "this is evil. fix."
        let parking_offset = match world.find_module(new_obj, b"JetAIUpdate") {
            Some(ju) => world.module_call(new_obj, ju, ModuleCall::JetGetParkingOffset).as_real(0.0),
            None => 0.0,
        };
        let produced_at_helipad = world.objects.find(new_obj).is_some_and(|o| o.is_kind_of(KINDOF_PRODUCED_AT_HELIPAD));
        let (hangar_internal, hangar_internal_orient, parking_space) = if produced_at_helipad {
            let (pos, orient) = Self::bone(world, us, "HeliPark01");
            (pos, orient, pos)
        } else {
            match self.reserve_space(world, us, new_obj, parking_offset, true)? {
                (true, Some(info)) => (info.hangar_internal, info.hangar_internal_orient, info.parking_space),
                // "no spaces available, how did we get here?" (the hangar position is then read uninitialised)
                _ => return Err(IniError::undefined(Undefined::UninitialisedLocal)),
            }
        };
        world.set_position(new_obj, &hangar_internal);
        world.set_orientation(new_obj, hangar_internal_orient);
        world.cut(crate::logic::world::object_cuts::CutCall::PathfinderAddObjectToPathfindMap { object: new_obj });
        if let Some(ai) = world.objects.find(new_obj).and_then(|o| o.ai) {
            let mut moved_to_rally_point = false;
            if produced_at_helipad && self.heli_rally_point_exists {
                world.module_call(new_obj, ai, ModuleCall::AiMoveToPosition { pos: self.heli_rally_point, command_source: CMD_FROM_AI });
                moved_to_rally_point = true;
            }
            if !moved_to_rally_point {
                if !produced_at_helipad {
                    world.module_call(new_obj, ai, ModuleCall::AiFollowExitProductionPath { path: vec![parking_space], ignore_object: us, command_source: CMD_FROM_AI });
                } else {
                    // "aiMoveToPosition has an added benefit. It invokes the pathfinder to find a vacant destination."
                    world.module_call(new_obj, ai, ModuleCall::AiMoveToPosition { pos: parking_space, command_source: CMD_FROM_AI });
                }
            }
        }
        Ok(())
    }

    /// `unreserveDoorForExit(exitDoor)`: the first space of that door freed.
    fn unreserve_door_for_exit(&mut self, exit_door: ExitDoorType) {
        if exit_door == DOOR_NONE_NEEDED {
            return;
        }
        if let Some(s) = self.spaces.iter_mut().find(|s| s.door == exit_door) {
            s.object_in_space = INVALID_ID;
            s.reserved_for_exit = false;
        }
    }

    /// `getExitPosition(pos)` and `getNaturalRallyPoint(pos, offset)`: the bone `HeliPark01` ("We only use this for the helipad").
    fn heli_park(world: &World, us: ObjectId) -> (bool, Coord3D) {
        let (found, pos, _) = world.object_get_single_logical_bone_transform(us, b"HeliPark01");
        (found, pos)
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

impl UpdateBehavior for ParkingPlaceBehavior {
    const XFER_VERSION: XferVersion = 3;
    fn update_base(&self) -> &UpdateModule {
        &self.base
    }
    /// `update()`.
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let r = self.do_update(world, me.object);
        Self::record(world, me, r).unwrap_or(UPDATE_SLEEP_NONE)
    }
    /// The spaces' occupants, the runways', the healees; from version 2 the rally point; from version 3 the next heal frame. (The world's
    /// load has built the spaces first.)
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
        let mode = xfer.get_xfer_mode();
        // "first, build our info, so it won't be overwritten later."
        if mode == XferMode::Load {
            if let Some(b) = self.staged_build.take() {
                let pu = b.production;
                let (doors, r) = self.apply_build(b);
                self.applied_holds = Some((pu, doors));
                if let Err(e) = r {
                    self.staged_error = Some(e);
                    return Err(XferStatus::ErrorUnknown);
                }
            }
        }
        // "spaces info count and data"
        let mut spaces_count = self.spaces.len() as u8;
        xfer.xfer_unsigned_byte(&mut spaces_count)?;
        if mode == XferMode::Save {
            for s in self.spaces.iter_mut() {
                xfer.xfer_object_id(&mut s.object_in_space)?;
                xfer.xfer_bool(&mut s.reserved_for_exit)?;
            }
        } else if mode == XferMode::Load {
            for i in 0..spaces_count as usize {
                let (mut id, mut reserved) = (INVALID_ID, false);
                xfer.xfer_object_id(&mut id)?;
                xfer.xfer_bool(&mut reserved)?;
                // "store in vector if the vector does indeed still have room for this entry"
                if let Some(s) = self.spaces.get_mut(i) {
                    s.object_in_space = id;
                    s.reserved_for_exit = reserved;
                }
            }
        }
        let mut runways_count = self.runways.len() as u8;
        xfer.xfer_unsigned_byte(&mut runways_count)?;
        if mode == XferMode::Save {
            for r in self.runways.iter_mut() {
                xfer.xfer_object_id(&mut r.in_use_by)?;
                xfer.xfer_object_id(&mut r.next_in_line_for_takeoff)?;
                xfer.xfer_bool(&mut r.was_in_line)?;
            }
        } else if mode == XferMode::Load {
            for i in 0..runways_count as usize {
                let (mut in_use_by, mut next, mut was) = (INVALID_ID, INVALID_ID, false);
                xfer.xfer_object_id(&mut in_use_by)?;
                xfer.xfer_object_id(&mut next)?;
                xfer.xfer_bool(&mut was)?;
                if let Some(r) = self.runways.get_mut(i) {
                    r.in_use_by = in_use_by;
                    r.next_in_line_for_takeoff = next;
                    r.was_in_line = was;
                }
            }
        }
        // "healees"
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
        if version >= 2 {
            xfer.xfer_coord3d(&mut self.heli_rally_point)?;
            xfer.xfer_bool(&mut self.heli_rally_point_exists)?;
        }
        if version >= 3 && !crate::logic::modules::plant(2456) {
            xfer.xfer_unsigned_int(&mut self.next_heal_frame)?;
        } else if mode == XferMode::Load {
            // "old file .. just wake it up right away, and it will fall into the correct pattern"
            self.next_heal_frame = 0;
        }
        Ok(())
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::ParkingPlaceBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::ParkingPlaceBehavior(m) => Some(m),
            _ => None,
        }
    }
    /// The die, exit and parking place interfaces.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        let us = me.object;
        match call {
            ModuleCall::OnDie(_) => {
                let r = self.kill_all_parked_units(world, us);
                Self::record(world, me, r);
                ModuleReply::Done
            }
            ModuleCall::HasParkingPlaceInterface | ModuleCall::HasUpdateExitInterface => ModuleReply::Bool(true),
            // ExitInterface
            ModuleCall::IsExitBusy => ModuleReply::Bool(false),
            ModuleCall::ReserveDoorForExit { template, .. } => {
                let r = self.reserve_door_for_exit(world, us, template);
                ModuleReply::Int(Self::record(world, me, r).unwrap_or(DOOR_NONE_AVAILABLE))
            }
            ModuleCall::ExitObjectViaDoor { obj, door } => {
                let r = self.exit_object_via_door(world, us, obj, door);
                Self::record(world, me, r);
                ModuleReply::Done
            }
            ModuleCall::UnreserveDoorForExit(door) => {
                self.unreserve_door_for_exit(door);
                ModuleReply::Done
            }
            ModuleCall::ExitObjectByBudding { .. } => ModuleReply::Done,
            ModuleCall::GetExitPosition | ModuleCall::GetNaturalRallyPoint | ModuleCall::GetNaturalRallyPointWithOffset(_) => {
                let (found, pos) = Self::heli_park(world, us);
                if found {
                    ModuleReply::Coord(pos)
                } else {
                    ModuleReply::Unhandled
                }
            }
            ModuleCall::SetRallyPoint(pos) => {
                self.heli_rally_point_exists = true;
                self.heli_rally_point = pos;
                ModuleReply::Done
            }
            ModuleCall::GetRallyPoint => {
                if self.heli_rally_point_exists {
                    ModuleReply::Coord(self.heli_rally_point)
                } else {
                    ModuleReply::Unhandled
                }
            }
            // ParkingPlaceBehaviorInterface
            ModuleCall::ParkingPlaceShouldReserveDoorWhenQueued { template } => ModuleReply::Bool(!Self::produced_at_helipad(world, template)),
            ModuleCall::ParkingPlaceHasAvailableSpaceFor { template } => ModuleReply::Bool(self.has_available_space_for(world, template)),
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
            ModuleCall::ParkingPlaceGetRunwayReservation { runway, .. } => {
                let r = self.get_runway_reservation(world, us, runway);
                ModuleReply::Id(Self::record(world, me, r).unwrap_or(INVALID_ID))
            }
            ModuleCall::ParkingPlaceTransferRunwayReservation { id } => {
                let r = self.transfer_runway_reservation(world, us, id);
                Self::record(world, me, r);
                ModuleReply::Done
            }
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
            ModuleCall::ParkingPlaceCalcBestParkingAssignment => ModuleReply::Bool(false),
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for ParkingPlaceBehavior {
    /// `crc`: `UpdateModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.base.crc(xfer)
    }
    /// `xfer` (version 3): the version, `UpdateModule::xfer`, the fields.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let m = ParkingPlaceBehavior::unconstructed(1, Some(Arc::new(ParkingPlaceBehaviorModuleData::default()))).unwrap();
        assert_eq!((m.next_heal_frame, m.got_info, m.heli_rally_point_exists), (FOREVER, false, false));
        assert_eq!(HEAL_RATE_FRAMES, 6);
        assert!(!m.has_reserved_space(5));
        assert_eq!(m.get_space_index(INVALID_ID), -1);
    }
}
