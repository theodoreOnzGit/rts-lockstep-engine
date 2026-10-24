// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ParachuteContain.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/ParachuteContain.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ParachuteContain`: the parachute a paradropped unit or an ejected pilot hangs from. It holds one rider, opens once it has fallen
//! `ParachuteOpenDist`, sways on a spring and damper while it falls, lets the rider go when it lands (or dies in the air), and kills riders that
//! land in water, off the map or on impassable ground.
//!
//! # Design
//!
//! As [`OpenContain`] for a class derived from it (see `heal_contain`): the module holds its `OpenContain` by value and implements
//! [`OpenContainVirtuals`]; the data holds `OpenContainModuleData` by value and lifts its two field tables.
//!
//! The calls on units that are not translated are cut points of `contain_cuts`: the drawable's pristine bones and instance matrix, the partition
//! manager's `findPositionAround`, the pathfinder's cell, the player's skirmish AI. The locomotor is the AI's (`ModuleCall::AiLocomotor*`).
//!
//! # Upstream facts kept
//!
//! - The constructor draws `GameLogicRandomValueReal` twice (the pitch and roll rates) and sets `OBJECT_STATUS_PARACHUTING` on its object.
//! - `update` reads the drawable unchecked (`draw->setDrawableHidden`): a parachute with no drawable is undefined there
//!   ([`Undefined::NullDrawable`]).
//! - `onDie` reads the rider's body unchecked for the free-fall damage ([`Undefined::NullBodyModule`]).
//! - The rider's bones fall back on its drawable's geometry height when the art has no `PARA_MAN` bone; the parachute's two bones are zero when
//!   the art lacks them.
//! - `getContainCount()` is the virtual; the rider is the front of the module's own list (`getContainList()`).
//!
//! `p1w13-sonnet`, batch R8, 2026-10-22.

use std::sync::Arc;

use crate::common::base_type::{real_to_int, Coord3D};
use crate::common::disabled_types::DISABLED_HELD;
use crate::common::game_type::{ObjectId, INVALID_ID, LAYER_GROUND};
use crate::common::ini::{
    FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, Undefined, UserData,
};
use crate::common::kind_of::{KINDOF_INFANTRY, KINDOF_PARACHUTABLE};
use crate::common::model_state::{MODELCONDITION_FREEFALL, MODELCONDITION_PARACHUTING};
use crate::common::module::ModuleDataKind;
use crate::common::object_status_types::{
    ObjectStatusMaskType, OBJECT_STATUS_NO_COLLISIONS, OBJECT_STATUS_PARACHUTING,
};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_FALLING, DAMAGE_WATER, HUGE_DAMAGE_AMOUNT};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    UpdateModule, UpdateSleepTime, DOOR_1, UPDATE_SLEEP_NONE,
};
use crate::logic::modules::open_contain::{
    self, out, OpenContain, OpenContainModuleData, OpenContainVirtuals, CMD_FROM_AI,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::object_creation_list::DEATH_FLOODED;
use crate::logic::terrain_logic::DEATH_SPLATTED;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::ww::matrix3d::Matrix3D;

/// `NO_START_Z`.
const NO_START_Z: f32 = 1e10;
/// `LOCOMOTORSET_NORMAL`.
const LOCOMOTORSET_NORMAL: i32 = 0;
/// `LOCOMOTORSET_FREEFALL`.
const LOCOMOTORSET_FREEFALL: i32 = 2;
/// `PATHFIND_CELL_SIZE`.
const PATHFIND_CELL_SIZE: i32 = 10;
/// `PathfindCell::CELL_WATER`, `CELL_CLIFF`, `CELL_IMPASSABLE`.
const CELL_WATER: i32 = 1;
const CELL_CLIFF: i32 = 2;
const CELL_IMPASSABLE: i32 = 6;
/// `FPF_NONE`.
const FPF_NONE: i32 = 0;

/// `class ParachuteContainModuleData : public OpenContainModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct ParachuteContainModuleData {
    /// The `OpenContainModuleData` base.
    pub open: OpenContainModuleData,
    /// `m_pitchRateMax` (`PitchRateMax`, an angular velocity).
    pub pitch_rate_max: f32,
    /// `m_rollRateMax` (`RollRateMax`).
    pub roll_rate_max: f32,
    /// `m_lowAltitudeDamping` (`LowAltitudeDamping`).
    pub low_altitude_damping: f32,
    /// `m_paraOpenDist` (`ParachuteOpenDist`): "deploy the parachute when we have traveled this far".
    pub para_open_dist: f32,
    /// `m_freeFallDamagePercent` (`FreeFallDamagePercent`).
    pub free_fall_damage_percent: f32,
    /// `m_killWhenLandingInWaterSlop` (`KillWhenLandingInWaterSlop`).
    pub kill_when_landing_in_water_slop: f32,
    /// `m_parachuteOpenSound` (`ParachuteOpenSound`).
    pub parachute_open_sound: crate::common::audio_event_name::AudioEventName,
}

impl Default for ParachuteContainModuleData {
    /// `ParachuteContainModuleData()`.
    fn default() -> Self {
        ParachuteContainModuleData {
            open: OpenContainModuleData::default(),
            pitch_rate_max: 0.0,
            roll_rate_max: 0.0,
            low_altitude_damping: 0.2,
            para_open_dist: 0.0,
            free_fall_damage_percent: 0.5,
            kill_when_landing_in_water_slop: 10.0,
            parachute_open_sound: Default::default(),
        }
    }
}

/// Lifts entry `k` of `OpenContainModuleData`'s table onto the `open` base.
fn lift_open(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut ParachuteContainModuleData,
    u: &UserData,
) -> IniResult<()> {
    match open_contain::FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.open, u),
        None => Ok(()),
    }
}

/// Lifts entry `k` of the die mux table onto the `open` base.
fn lift_die(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut ParachuteContainModuleData,
    u: &UserData,
) -> IniResult<()> {
    match open_contain::DIE_MUX_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.open, u),
        None => Ok(()),
    }
}

static LIFTED_OPEN: [FieldParse<ParachuteContainModuleData>; 16] = [
    FieldParse::new(
        "ContainMax",
        |i, d, u| lift_open(0, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "EnterSound",
        |i, d, u| lift_open(1, i, d, u),
        UserData::None,
    ),
    FieldParse::new("ExitSound", |i, d, u| lift_open(2, i, d, u), UserData::None),
    FieldParse::new(
        "DamagePercentToUnits",
        |i, d, u| lift_open(3, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "BurnedDeathToUnits",
        |i, d, u| lift_open(4, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "AllowInsideKindOf",
        |i, d, u| lift_open(5, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "ForbidInsideKindOf",
        |i, d, u| lift_open(6, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "PassengersAllowedToFire",
        |i, d, u| lift_open(7, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "PassengersInTurret",
        |i, d, u| lift_open(8, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "NumberOfExitPaths",
        |i, d, u| lift_open(9, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "DoorOpenTime",
        |i, d, u| lift_open(10, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "WeaponBonusPassedToPassengers",
        |i, d, u| lift_open(11, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "AllowAlliesInside",
        |i, d, u| lift_open(12, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "AllowEnemiesInside",
        |i, d, u| lift_open(13, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "AllowNeutralInside",
        |i, d, u| lift_open(14, i, d, u),
        UserData::None,
    ),
    FieldParse::end(),
];

static LIFTED_DIE: [FieldParse<ParachuteContainModuleData>; 5] = [
    FieldParse::new("DeathTypes", |i, d, u| lift_die(0, i, d, u), UserData::None),
    FieldParse::new(
        "VeterancyLevels",
        |i, d, u| lift_die(1, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "ExemptStatus",
        |i, d, u| lift_die(2, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "RequiredStatus",
        |i, d, u| lift_die(3, i, d, u),
        UserData::None,
    ),
    FieldParse::end(),
];

/// `ParachuteContainModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<ParachuteContainModuleData>; 8] = [
    FieldParse::new(
        "PitchRateMax",
        |i, d, _| {
            d.pitch_rate_max = i.parse_angular_velocity_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "RollRateMax",
        |i, d, _| {
            d.roll_rate_max = i.parse_angular_velocity_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "LowAltitudeDamping",
        |i, d, _| {
            d.low_altitude_damping = i.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "ParachuteOpenDist",
        |i, d, _| {
            d.para_open_dist = i.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "KillWhenLandingInWaterSlop",
        |i, d, _| {
            d.kill_when_landing_in_water_slop = i.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "FreeFallDamagePercent",
        |i, d, _| {
            d.free_fall_damage_percent = i.parse_percent_to_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "ParachuteOpenSound",
        |i, d, _| d.parachute_open_sound.parse(i),
        UserData::None,
    ),
    FieldParse::end(),
];

impl ParachuteContainModuleData {
    /// `ParachuteContainModuleData::buildFieldParse(p)`: `OpenContainModuleData::buildFieldParse(p)`, then this class's table.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, ParachuteContainModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&LIFTED_OPEN)?;
        p.add(&LIFTED_DIE)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<ParachuteContainModuleData> {
        let mut data = ParachuteContainModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, ParachuteContainModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `ParachuteContain`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::ParachuteContain(Arc::new(
        ParachuteContainModuleData::friend_new_module_data(ini)?,
    )))
}

/// `class ParachuteContain : public OpenContain`.
#[derive(Clone, Debug, PartialEq)]
pub struct ParachuteContain {
    oc: OpenContain,
    /// `getParachuteContainModuleData()`.
    data: Arc<ParachuteContainModuleData>,
    /// `m_pitch`, `m_roll`, `m_pitchRate`, `m_rollRate`: the sway.
    pitch: f32,
    roll: f32,
    pitch_rate: f32,
    roll_rate: f32,
    /// `m_startZ`: the height the fall started at (`NO_START_Z` until the first update).
    start_z: f32,
    /// `m_isLandingOverrideSet`, `m_landingOverride`: "instead of falling peacefully towards a clear spot, I will now aim here".
    is_landing_override_set: bool,
    landing_override: Coord3D,
    /// `m_riderAttachBone`, `m_riderSwayBone`, `m_paraAttachBone`, `m_paraSwayBone`: the bones, in the art's space.
    rider_attach_bone: Coord3D,
    rider_sway_bone: Coord3D,
    para_attach_bone: Coord3D,
    para_sway_bone: Coord3D,
    /// `m_riderAttachOffset`, `m_riderSwayOffset`, `m_paraAttachOffset`, `m_paraSwayOffset`: the bones' offsets from the objects, in the world.
    rider_attach_offset: Coord3D,
    rider_sway_offset: Coord3D,
    para_attach_offset: Coord3D,
    para_sway_offset: Coord3D,
    /// `m_needToUpdateRiderBones`, `m_needToUpdateParaBones`.
    need_to_update_rider_bones: bool,
    need_to_update_para_bones: bool,
    /// `m_opened`.
    opened: bool,
}

/// Records that upstream is undefined here.
fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
    world.cuts.module_errors.push((me, IniError::undefined(u)));
}

/// `MAKE_OBJECT_STATUS_MASK(bit)`.
fn status_mask(bit: i32) -> ObjectStatusMaskType {
    let mut m = ObjectStatusMaskType::new();
    m.set(bit, 1);
    m
}

const ZERO: Coord3D = Coord3D {
    x: 0.0,
    y: 0.0,
    z: 0.0,
};

impl ParachuteContain {
    /// `getInterfaceMask()`: the base's.
    pub const fn get_interface_mask() -> i32 {
        OpenContain::get_interface_mask()
    }

    /// The constructor's first half: the base's (on a copy of its part of the data) and the fields.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<ParachuteContainModuleData>>,
    ) -> IniResult<ParachuteContain> {
        let data = module_data
            .ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let oc = OpenContain::unconstructed(object, Some(Arc::new(data.open.clone())))?;
        Ok(ParachuteContain {
            oc,
            data,
            pitch: 0.0,
            roll: 0.0,
            pitch_rate: 0.0,
            roll_rate: 0.0,
            start_z: NO_START_Z,
            is_landing_override_set: false,
            landing_override: ZERO,
            rider_attach_bone: ZERO,
            rider_sway_bone: ZERO,
            para_attach_bone: ZERO,
            para_sway_bone: ZERO,
            rider_attach_offset: ZERO,
            rider_sway_offset: ZERO,
            para_attach_offset: ZERO,
            para_sway_offset: ZERO,
            need_to_update_rider_bones: true,
            need_to_update_para_bones: true,
            opened: false,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        self.oc.update_module()
    }

    /// The module data.
    pub fn module_data(&self) -> &Arc<ParachuteContainModuleData> {
        &self.data
    }

    /// The state, as words: the base's, then the fields in `xfer` order (reals as bits).
    pub fn state_words(&self) -> Vec<u32> {
        let mut v = self.oc.state_words();
        let c =
            |v: &mut Vec<u32>, p: &Coord3D| v.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
        v.extend([
            self.pitch.to_bits(),
            self.roll.to_bits(),
            self.pitch_rate.to_bits(),
            self.roll_rate.to_bits(),
            self.start_z.to_bits(),
        ]);
        v.push(u32::from(self.is_landing_override_set));
        for p in [
            &self.landing_override,
            &self.rider_attach_bone,
            &self.rider_sway_bone,
            &self.para_attach_bone,
            &self.para_sway_bone,
            &self.rider_attach_offset,
            &self.rider_sway_offset,
            &self.para_attach_offset,
            &self.para_sway_offset,
        ] {
            c(&mut v, p);
        }
        v.extend([
            u32::from(self.need_to_update_rider_bones),
            u32::from(self.need_to_update_para_bones),
            u32::from(self.opened),
        ]);
        v
    }

    /// The constructor's second half: the base's, then the two random rates and `OBJECT_STATUS_PARACHUTING` on the object.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.oc.world_construct(world, me);
        let d = self.data.clone();
        self.pitch_rate = world
            .random
            .game_logic_random_value_real(-d.pitch_rate_max, d.pitch_rate_max);
        if !crate::logic::modules::plant(1511) {
            self.roll_rate = world
                .random
                .game_logic_random_value_real(-d.roll_rate_max, d.roll_rate_max);
        }
        out(self, world, me, |w| {
            w.set_status(me.object, status_mask(OBJECT_STATUS_PARACHUTING), true)
        });
    }

    /// `update()` as the scheduler runs it.
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        open_contain::contain_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: the version (1), the base's, then the fields.
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

    /// `loadPostProcess` with the scheduling half: "extend base class".
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        self.oc.load_post_process_with(update);
    }

    /// The fields of `xfer`, after the base's.
    fn xfer_own_fields(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        xfer.xfer_real(&mut self.pitch)?;
        xfer.xfer_real(&mut self.roll)?;
        xfer.xfer_real(&mut self.pitch_rate)?;
        xfer.xfer_real(&mut self.roll_rate)?;
        xfer.xfer_real(&mut self.start_z)?;
        xfer.xfer_bool(&mut self.is_landing_override_set)?;
        xfer.xfer_coord3d(&mut self.landing_override)?;
        xfer.xfer_coord3d(&mut self.rider_attach_bone)?;
        xfer.xfer_coord3d(&mut self.rider_sway_bone)?;
        xfer.xfer_coord3d(&mut self.para_attach_bone)?;
        if !crate::logic::modules::plant(1512) {
            xfer.xfer_coord3d(&mut self.para_sway_bone)?;
        }
        xfer.xfer_coord3d(&mut self.rider_attach_offset)?;
        xfer.xfer_coord3d(&mut self.rider_sway_offset)?;
        xfer.xfer_coord3d(&mut self.para_attach_offset)?;
        xfer.xfer_coord3d(&mut self.para_sway_offset)?;
        xfer.xfer_bool(&mut self.need_to_update_rider_bones)?;
        xfer.xfer_bool(&mut self.need_to_update_para_bones)?;
        xfer.xfer_bool(&mut self.opened)
    }

    /// The rider: the front of the module's own list when `getContainCount()` (the virtual) says there is one. `front()` of an empty list (the count and
    /// the list disagree) is undefined ([`Undefined::NullObject`], recorded).
    fn rider(&self, world: &mut World, me: ModuleHandle) -> ObjectId {
        if self.get_contain_count(world) > 0 {
            match self.oc.contain_list().first() {
                Some(&r) => r,
                None => {
                    undefined(world, me, Undefined::NullObject);
                    INVALID_ID
                }
            }
        } else {
            INVALID_ID
        }
    }

    /// `calcSwayMtx(offset, &mtx)`: the sway about `offset` (roll about X, then pitch about Y).
    fn calc_sway_mtx(&self, offset: &Coord3D) -> Matrix3D {
        let mut mtx = Matrix3D::IDENTITY;
        mtx.make_identity();
        mtx.translate(offset.x, offset.y, offset.z);
        mtx.in_place_pre_rotate_x(self.roll);
        if !crate::logic::modules::plant(1513) {
            mtx.in_place_pre_rotate_y(self.pitch);
        }
        mtx.translate(-offset.x, -offset.y, -offset.z);
        mtx
    }

    /// `updateBonePositions()`: the bones, read from the art once each is asked for ("yeah, even if not found").
    fn update_bone_positions(&mut self, world: &mut World, me: ModuleHandle) {
        let us = me.object;
        if self.need_to_update_para_bones {
            self.need_to_update_para_bones = false;
            if world.has_drawable(us) {
                self.para_sway_bone = world
                    .contain_drawable_pristine_bone(us, b"PARA_COG")
                    .unwrap_or(ZERO);
                self.para_attach_bone = world
                    .contain_drawable_pristine_bone(us, b"PARA_ATTCH")
                    .unwrap_or(ZERO);
            }
        }

        if self.need_to_update_rider_bones {
            self.need_to_update_rider_bones = false;
            let rider = self.rider(world, me);
            if rider != INVALID_ID && world.has_drawable(rider) {
                match world.contain_drawable_pristine_bone(rider, b"PARA_MAN") {
                    Some(p) => self.rider_attach_bone = p,
                    None => {
                        // "No parachute-attach bone... using object height!"
                        self.rider_attach_bone = ZERO;
                        let h = world
                            .objects
                            .find(rider)
                            .map_or(0.0, |o| o.geometry_info.get_max_height_above_position());
                        if !crate::logic::modules::plant(1514) {
                            self.rider_attach_bone.z += h;
                        }
                    }
                }
            }
        }
    }

    /// `updateOffsetsFromBones()`: the bones in the world, as offsets from the objects.
    fn update_offsets_from_bones(&mut self, world: &mut World, me: ModuleHandle) {
        let us = me.object;
        let obj_pos = world.objects.find(us).map_or(ZERO, |o| o.get_position());
        let sub = |a: Coord3D, b: &Coord3D| Coord3D {
            x: a.x - b.x,
            y: a.y - b.y,
            z: a.z - b.z,
        };

        self.para_sway_offset = sub(
            world.contain_convert_bone_pos_to_world_pos(us, &self.para_sway_bone),
            &obj_pos,
        );
        self.para_attach_offset = sub(
            world.contain_convert_bone_pos_to_world_pos(us, &self.para_attach_bone),
            &obj_pos,
        );

        let rider = self.rider(world, me);
        if rider != INVALID_ID {
            let rider_pos = world.objects.find(rider).map_or(ZERO, |o| o.get_position());
            self.rider_attach_offset = sub(
                world.contain_convert_bone_pos_to_world_pos(rider, &self.rider_attach_bone),
                &rider_pos,
            );
            self.rider_attach_offset = sub(self.para_attach_offset, &self.rider_attach_offset);
            if !crate::logic::modules::plant(1515) {
                self.rider_sway_offset = sub(self.para_sway_offset, &self.rider_attach_offset);
            }
        }
    }

    /// `positionRider(rider)`: the rider hangs from the parachute's attach bone (never below the ground), faces as it does, and sways with it while held.
    fn position_rider(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        self.update_bone_positions(world, me);
        self.update_offsets_from_bones(world, me);

        let us = me.object;
        let mut pos = world.objects.find(us).map_or(ZERO, |o| o.get_position());
        pos.x += self.rider_attach_offset.x;
        pos.y += self.rider_attach_offset.y;
        pos.z += self.rider_attach_offset.z;
        out(self, world, me, |w| w.contain_set_position(rider, &pos));

        let alt = out(self, world, me, |w| w.get_height_above_terrain(rider));
        if alt < 0.0 && !crate::logic::modules::plant(1516) {
            // "don't let him go below ground."
            pos.z -= alt;
            out(self, world, me, |w| w.contain_set_position(rider, &pos));
        }

        let angle = world
            .objects
            .find(us)
            .map_or(0.0, |o| o.thing.orientation());
        out(self, world, me, |w| w.contain_set_orientation(rider, angle));

        if world.has_drawable(rider) {
            if world
                .objects
                .find(rider)
                .is_some_and(|o| o.is_disabled_by_type(DISABLED_HELD))
            {
                let tmp = self.calc_sway_mtx(&self.rider_sway_offset.clone());
                out(self, world, me, |w| {
                    w.contain_drawable_set_instance_matrix(rider, Some(&tmp))
                });
            } else {
                out(self, world, me, |w| {
                    w.contain_drawable_set_instance_matrix(rider, None)
                });
            }
        }
    }

    /// `obj->kill()`: `false`, and undefined recorded, for an object with no body (`Object::kill` calls through `getBodyModule()`).
    fn kill(&mut self, world: &mut World, me: ModuleHandle, id: ObjectId) -> bool {
        if out(self, world, me, |w| w.contain_kill(id)) {
            return true;
        }
        undefined(world, me, Undefined::NullBodyModule);
        false
    }

    /// The hidden flag of the parachute's drawable, if it has one (`onDrawableBoundToObject`, `containReactToTransformChange`).
    fn hide_unless_opened(&mut self, world: &mut World, me: ModuleHandle) {
        if world.has_drawable(me.object) {
            let hidden = !self.opened;
            out(self, world, me, |w| {
                w.contain_drawable_set_hidden(me.object, hidden)
            });
        }
    }
}

impl OpenContainVirtuals for ParachuteContain {
    fn oc(&self) -> &OpenContain {
        &self.oc
    }
    fn oc_mut(&mut self) -> &mut OpenContain {
        &mut self.oc
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::ParachuteContain(self)
    }
    fn from_body(b: UpdateBody) -> Option<Self> {
        match b {
            UpdateBody::ParachuteContain(m) => Some(m),
            _ => None,
        }
    }

    /// `ParachuteContain::isValidContainerFor(rider, checkCapacity)`: the base's, then a transportable rider (infantry and parachutable objects always
    /// are: "this allows us to paradrop pilots, but not transport them by other means"), and only one at a time.
    fn is_valid_container_for(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        rider: ObjectId,
        check_capacity: bool,
    ) -> bool {
        if rider == INVALID_ID || world.objects.find(rider).is_none() {
            return false;
        }
        // extend functionality
        if !open_contain::base_is_valid_container_for(self, world, me, rider, check_capacity) {
            return false;
        }
        let transport_slot_count = out(self, world, me, |w| w.get_transport_slot_count(rider));
        let (infantry, parachutable) = world.objects.find(rider).map_or((false, false), |o| {
            (
                o.is_kind_of(KINDOF_INFANTRY),
                o.is_kind_of(KINDOF_PARACHUTABLE),
            )
        });
        if transport_slot_count == 0
            && !infantry
            && (!parachutable || crate::logic::modules::plant(1517))
        {
            return false;
        }
        // "we can only 'hold' one item at a time."
        if self.get_contain_count(world) > 0 && !crate::logic::modules::plant(1518) {
            return false;
        }
        true
    }

    /// `isEnclosingContainerFor(obj)`: `FALSE`.
    fn is_enclosing_container_for(
        &mut self,
        _world: &mut World,
        _me: ModuleHandle,
        _obj: ObjectId,
    ) -> bool {
        crate::logic::modules::plant(1519)
    }

    /// `isSpecialZeroSlotContainer()`: `true`.
    fn is_special_zero_slot_container(&self) -> bool {
        true
    }

    /// `ParachuteContain::onContaining(rider, wasSelected)`: the rider is held, parachuting and in free fall, and hangs from the parachute.
    fn on_containing(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        rider: ObjectId,
        was_selected: bool,
    ) {
        open_contain::base_on_containing(self, world, me, rider, was_selected);

        // "objects inside a transport are held"
        out(self, world, me, |w| {
            w.contain_set_disabled(rider, DISABLED_HELD)
        });
        out(self, world, me, |w| {
            w.set_status(rider, status_mask(OBJECT_STATUS_PARACHUTING), true)
        });

        out(self, world, me, |w| {
            w.clear_and_set_model_condition_state(
                rider,
                MODELCONDITION_PARACHUTING,
                MODELCONDITION_FREEFALL,
            )
        });
        self.need_to_update_rider_bones = true;

        // "position him correctly."
        self.position_rider(world, me, rider);
    }

    /// `ParachuteContain::onRemoving(rider)`: the rider is let go to fall, given orders, and killed when he lands in water, off the map or on
    /// impassable ground.
    fn on_removing(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        open_contain::base_on_removing(self, world, me, rider);
        let d = self.data.clone();
        let us = me.object;

        // "object is no longer held inside a transport"
        out(self, world, me, |w| {
            w.contain_clear_disabled(rider, DISABLED_HELD)
        });
        out(self, world, me, |w| {
            w.clear_status(rider, status_mask(OBJECT_STATUS_PARACHUTING))
        });

        // "mark parachute as 'no-collisions'... it is just ephemeral at this point"
        if !crate::logic::modules::plant(1520) {
            out(self, world, me, |w| {
                w.set_status(us, status_mask(OBJECT_STATUS_NO_COLLISIONS), true)
            });
        }

        // "position him correctly."
        self.position_rider(world, me, rider);

        out(self, world, me, |w| {
            w.contain_clear_model_condition_flags(
                rider,
                &[MODELCONDITION_FREEFALL, MODELCONDITION_PARACHUTING],
            )
        });
        self.need_to_update_rider_bones = true;

        // "temporarily mark the guy as being allowed to fall (overriding his locomotor's stick-to-ground attribute)"
        if let Some(physics) = world.objects.find(rider).and_then(|o| o.physics) {
            out(self, world, me, |w| {
                w.module_call(rider, physics, ModuleCall::PhysicsSetAllowToFall(true))
            });
            out(self, world, me, |w| {
                w.module_call(rider, physics, ModuleCall::PhysicsApplyForce(ZERO))
            });
        }

        if let Some(rider_ai) = world.objects.find(rider).and_then(|o| o.ai) {
            let controller = world.get_controlling_player(rider);
            if controller.is_some_and(|p| world.contain_player_is_skirmish_ai(p)) {
                // "hunt, as per Dustin's request."
                out(self, world, me, |w| {
                    w.module_call(
                        rider,
                        rider_ai,
                        ModuleCall::AiHunt {
                            command_source: CMD_FROM_AI,
                        },
                    )
                });
            } else {
                let mut has_rally_point = false;
                // the transport of the rider, and the building that produced the transport
                let transport = world
                    .objects
                    .find(rider)
                    .map_or(INVALID_ID, |o| o.producer_id);
                if let Some(t) = world.objects.find(transport) {
                    let producer = t.producer_id;
                    if world.objects.find(producer).is_some() {
                        // "See if we need to set a rally point for the object being parachuted"
                        if let Some(exit) =
                            out(self, world, me, |w| w.get_object_exit_interface(producer))
                        {
                            if out(self, world, me, |w| {
                                w.module_call(producer, exit, ModuleCall::UseSpawnRallyPoint)
                            })
                            .as_bool()
                            {
                                out(self, world, me, |w| {
                                    w.module_call(
                                        producer,
                                        exit,
                                        ModuleCall::ExitObjectViaDoor {
                                            obj: rider,
                                            door: DOOR_1,
                                        },
                                    )
                                });
                                has_rally_point = true;
                            }
                        }
                    }
                }
                if !has_rally_point && !crate::logic::modules::plant(1521) {
                    out(self, world, me, |w| {
                        w.module_call(
                            rider,
                            rider_ai,
                            ModuleCall::AiIdleFrom {
                                command_source: CMD_FROM_AI,
                            },
                        )
                    });
                }
            }
        }

        // "if we land in the water, we die. alas."
        let rider_pos = world.objects.find(rider).map_or(ZERO, |o| o.get_position());
        let rider_layer = world.objects.find(rider).map_or(LAYER_GROUND, |o| o.layer);
        if let Some(water_z) = out(self, world, me, |w| {
            w.terrain_is_underwater_water_z(rider_pos.x, rider_pos.y)
        }) {
            if rider_pos.z <= water_z + d.kill_when_landing_in_water_slop
                && rider_layer == LAYER_GROUND
            {
                // "don't call kill(); do it manually, so we can specify DEATH_FLOODED"
                let mut info = DamageInfo::default();
                info.input.damage_type = DAMAGE_WATER;
                info.input.death_type = if crate::logic::modules::plant(1522) {
                    crate::logic::damage::DEATH_NORMAL
                } else {
                    DEATH_FLOODED
                };
                info.input.source_id = INVALID_ID;
                info.input.amount = HUGE_DAMAGE_AMOUNT;
                out(self, world, me, |w| {
                    w.contain_attempt_damage(rider, &mut info)
                });
            }
        }

        // "Kill if we landed on impassable ground"
        let rider_pos = world.objects.find(rider).map_or(ZERO, |o| o.get_position());
        let cell_x = real_to_int(rider_pos.x / PATHFIND_CELL_SIZE as f32);
        let cell_y = real_to_int(rider_pos.y / PATHFIND_CELL_SIZE as f32);
        let rider_layer = world.objects.find(rider).map_or(LAYER_GROUND, |o| o.layer);
        let cell_type = out(self, world, me, |w| {
            w.contain_pathfind_cell_type(rider_layer, cell_x, cell_y)
        })
        .unwrap_or(CELL_IMPASSABLE);

        // "If we land outside the map from a faulty parachute, we die too."
        let off_map = world.objects.find(rider).is_some_and(|o| o.is_off_map());
        if off_map
            || cell_type == CELL_CLIFF
            || (cell_type == CELL_WATER && !crate::logic::modules::plant(1523))
            || cell_type == CELL_IMPASSABLE
        {
            self.kill(world, me, rider);
        }
    }

    /// `ParachuteContain::onDie(damageInfo)`: killed in the air, the parachute drops its rider screaming, with some damage "for losing your parachute".
    fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
        let us = me.object;
        if out(self, world, me, |w| w.is_significantly_above_terrain(us))
            != crate::logic::modules::plant(1524)
        {
            let rider = self.rider(world, me);
            if rider != INVALID_ID {
                self.remove_all_contained(world, me, false);
                let d = self.data.clone();
                if d.free_fall_damage_percent > 0.0 {
                    let Some(body) = world.objects.find(rider).and_then(|o| o.body) else {
                        undefined(world, me, Undefined::NullBodyModule);
                        return;
                    };
                    let max = out(self, world, me, |w| {
                        w.module_call(rider, body, ModuleCall::GetMaxHealth)
                    })
                    .as_real(0.0);
                    let mut extra = DamageInfo::default();
                    extra.input.damage_type = DAMAGE_FALLING;
                    extra.input.death_type = DEATH_SPLATTED;
                    extra.input.source_id = info.input.source_id;
                    extra.input.amount = max * d.free_fall_damage_percent;
                    out(self, world, me, |w| {
                        w.contain_attempt_damage(rider, &mut extra)
                    });
                }
                if let Some(physics) = world.objects.find(rider).and_then(|o| o.physics) {
                    out(self, world, me, |w| {
                        w.module_call(rider, physics, ModuleCall::PhysicsSetAllowToFall(true))
                    });
                    // "bwah ha ha"
                    if !crate::logic::modules::plant(1525) {
                        out(self, world, me, |w| {
                            w.module_call(rider, physics, ModuleCall::PhysicsSetIsInFreeFall(true))
                        });
                    }
                    out(self, world, me, |w| {
                        w.module_call(rider, physics, ModuleCall::PhysicsApplyForce(ZERO))
                    });
                }
            }
        }

        open_contain::base_on_die(self, world, me, info);
    }

    /// `ParachuteContain::onCollide(other)`: hitting the ground (`other` is `NULL`) lets the rider go and kills the parachute "so that the chute's
    /// SlowDeath will trigger"; nothing else (the base's collision entry is not run).
    fn on_collide(&mut self, world: &mut World, me: ModuleHandle, other: ObjectId) {
        if other == INVALID_ID || world.objects.find(other).is_none() {
            // "if we're in a container (eg, a transport plane), just ignore this..."
            if world
                .objects
                .find(me.object)
                .is_some_and(|o| o.contained_by != INVALID_ID)
                && !crate::logic::modules::plant(1526)
            {
                return;
            }
            self.remove_all_contained(world, me, false);
            self.kill(world, me, me.object);
        }
    }

    /// `ParachuteContain::update()`.
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        open_contain::base_update(self, world, me);

        let us = me.object;
        if world
            .objects
            .find(us)
            .is_some_and(|o| o.is_disabled_by_type(DISABLED_HELD))
        {
            // "my, that was easy"
            return UPDATE_SLEEP_NONE;
        }

        let parachute_ai = world.objects.find(us).and_then(|o| o.ai);
        let d = self.data.clone();
        let rider = self.rider(world, me);
        let pos = |w: &World| w.objects.find(us).map_or(ZERO, |o| o.get_position());

        if self.start_z == NO_START_Z {
            let p = pos(world);
            self.start_z = p.z;
            let ground_height = out(self, world, me, |w| w.terrain_get_ground_height(p.x, p.y));
            if self.start_z - ground_height < 2.0 * d.para_open_dist
                && !crate::logic::modules::plant(1527)
            {
                // "we ejected too close to the ground, and there isn't enough room to open the chute ... we'll fudge a little"
                self.start_z = ground_height + 2.0 * d.para_open_dist;
            }
        }

        if !self.opened {
            // see if we need to open.
            if (self.start_z - pos(world).z).abs() >= d.para_open_dist {
                self.opened = !crate::logic::modules::plant(1528);
                out(self, world, me, |w| {
                    w.clear_and_set_model_condition_state(
                        us,
                        MODELCONDITION_FREEFALL,
                        MODELCONDITION_PARACHUTING,
                    )
                });
                self.need_to_update_para_bones = true;
                if rider != INVALID_ID {
                    out(self, world, me, |w| {
                        w.clear_and_set_model_condition_state(
                            rider,
                            MODELCONDITION_FREEFALL,
                            MODELCONDITION_PARACHUTING,
                        )
                    });
                    self.need_to_update_rider_bones = true;
                    let name = d.parachute_open_sound.event_name.clone();
                    out(self, world, me, |w| {
                        w.emit(ObjectClientEvent::AudioAddEvent {
                            name,
                            object_id: rider,
                            position: None,
                            player_index: None,
                        })
                    });
                }

                // "When a parachute opens, it should look for a good place to land. This could be explicitly set by a DeliverPayload, otherwise any
                // place clear is good."
                if let Some(ai) = parachute_ai {
                    let mut target = pos(world);
                    if self.is_landing_override_set {
                        target = self.landing_override;
                        if out(self, world, me, |w| {
                            w.module_call(us, ai, ModuleCall::AiHasCurLocomotor)
                        })
                        .as_bool()
                        {
                            out(self, world, me, |w| {
                                w.module_call(us, ai, ModuleCall::AiLocomotorSetUltraAccurate(true))
                            });
                        }
                    } else if !crate::logic::modules::plant(1529) {
                        out(self, world, me, |w| {
                            w.contain_partition_find_position_around(
                                &mut target,
                                0.0,
                                100.0,
                                FPF_NONE,
                            )
                        });
                    }
                    out(self, world, me, |w| {
                        w.module_call(
                            us,
                            ai,
                            ModuleCall::AiMoveToPosition {
                                pos: target,
                                command_source: CMD_FROM_AI,
                            },
                        )
                    });
                }
            } else if rider != INVALID_ID {
                out(self, world, me, |w| {
                    w.clear_and_set_model_condition_state(
                        rider,
                        MODELCONDITION_PARACHUTING,
                        MODELCONDITION_FREEFALL,
                    )
                });
            }
        }
        // the drawable, unchecked
        if !world.has_drawable(us) {
            undefined(world, me, Undefined::NullDrawable);
            return UPDATE_SLEEP_NONE;
        }
        let hidden = !self.opened;
        out(self, world, me, |w| {
            w.contain_drawable_set_hidden(us, hidden)
        });

        let no_collisions = status_mask(OBJECT_STATUS_NO_COLLISIONS);
        if !self.opened || self.get_contain_count(world) == 0 {
            // "unopened, or empty, chutes, don't collide with anything, to simplify ejections, paradrops, landings, etc..."
            out(self, world, me, |w| w.set_status(us, no_collisions, true));
            if rider != INVALID_ID {
                out(self, world, me, |w| {
                    w.set_status(rider, no_collisions, true)
                });
            }
        } else {
            // "opened/nonempty chutes DO collide..."
            out(self, world, me, |w| w.clear_status(us, no_collisions));
            if rider != INVALID_ID && !crate::logic::modules::plant(1530) {
                out(self, world, me, |w| w.clear_status(rider, no_collisions));
            }
        }

        if let Some(ai) = parachute_ai {
            if !world
                .objects
                .find(us)
                .is_some_and(|o| o.is_effectively_dead())
            {
                let set = if self.opened {
                    LOCOMOTORSET_NORMAL
                } else {
                    LOCOMOTORSET_FREEFALL
                };
                out(self, world, me, |w| {
                    w.module_call(us, ai, ModuleCall::AiChooseLocomotorSet(set))
                });

                if out(self, world, me, |w| {
                    w.module_call(us, ai, ModuleCall::AiHasCurLocomotor)
                })
                .as_bool()
                {
                    // "damp the swaying a bunch when we get close, so that things land vertically (or nearly so)"
                    let mut altitude_damping = 0.0;
                    if self.get_contain_count(world) > 0 {
                        let rider = self.rider(world, me);
                        const ALTITUDE_DAMP_START: f32 = 20.0;
                        if out(self, world, me, |w| w.get_height_above_terrain(rider))
                            <= ALTITUDE_DAMP_START
                        {
                            altitude_damping = d.low_altitude_damping;
                        }
                    }

                    if self.opened {
                        let sway = |t: &mut Self, w: &mut World, k: u8| {
                            out(t, w, me, |w| {
                                w.module_call(us, ai, ModuleCall::AiLocomotorSway(k))
                            })
                            .as_real(0.0)
                        };
                        let pitch_stiffness = sway(self, world, 0);
                        let roll_stiffness = sway(self, world, 1);
                        let pitch_damping = sway(self, world, 2) + altitude_damping;
                        let roll_damping = sway(self, world, 3) + altitude_damping;
                        // spring/damper
                        self.pitch_rate +=
                            (-pitch_stiffness * self.pitch) + (-pitch_damping * self.pitch_rate);
                        self.roll_rate +=
                            (-roll_stiffness * self.roll) + (-roll_damping * self.roll_rate);

                        self.pitch += self.pitch_rate;
                        if !crate::logic::modules::plant(1531) {
                            self.roll += self.roll_rate;
                        }

                        if self.is_landing_override_set {
                            // "Need to wait until after opening to do this, or else we are sending the message to the Freefall locomotor."
                            out(self, world, me, |w| {
                                w.module_call(
                                    us,
                                    ai,
                                    ModuleCall::AiLocomotorSetCloseEnoughDist(10.0),
                                )
                            });
                            out(self, world, me, |w| {
                                w.module_call(
                                    us,
                                    ai,
                                    ModuleCall::AiLocomotorSetCloseEnoughDist3D(false),
                                )
                            });
                        }
                    }

                    // (the drawable is there: upstream read it unchecked above)
                    self.update_bone_positions(world, me);
                    self.update_offsets_from_bones(world, me);
                    let tmp = self.calc_sway_mtx(&self.para_sway_offset.clone());
                    out(self, world, me, |w| {
                        w.contain_drawable_set_instance_matrix(us, Some(&tmp))
                    });

                    self.position_contained_objects_relative_to_container(world, me);
                }
            }
        }

        // "allow us to land on bridges!"
        let para_pos = pos(world);
        let new_layer = out(self, world, me, |w| {
            w.terrain_get_highest_layer_for_destination(&para_pos)
        });
        out(self, world, me, |w| w.contain_set_layer(us, new_layer));
        if rider != INVALID_ID {
            out(self, world, me, |w| w.contain_set_layer(rider, new_layer));
        }

        // "If we have lost our passenger for whatever reason, die early. Otherwise we just sit around forever."
        if self.get_contain_count(world) == 0
            && !crate::logic::modules::plant(1532)
            && !self.kill(world, me, us)
        {
            return UPDATE_SLEEP_NONE;
        }

        // "the collide system doesn't always collide us with the ground if we fall into water. so force the issue."
        let para_pos = pos(world);
        let (dead, layer) = world
            .objects
            .find(us)
            .map_or((true, LAYER_GROUND), |o| (o.is_effectively_dead(), o.layer));
        if !dead && layer == LAYER_GROUND {
            if let Some(water_z) = out(self, world, me, |w| {
                w.terrain_is_underwater_water_z(para_pos.x, para_pos.y)
            }) {
                if para_pos.z - water_z < d.kill_when_landing_in_water_slop {
                    self.kill(world, me, us);
                }
            }
        }

        UPDATE_SLEEP_NONE
    }

    /// `positionContainedObjectsRelativeToContainer()`: every rider of the module's own list is positioned.
    fn position_contained_objects_relative_to_container(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
    ) {
        for rider in self.oc.contain_list().to_vec() {
            self.position_rider(world, me, rider);
        }
    }

    /// `containReactToTransformChange()`: "hidden is a flag, not a counter, so when we are dumped from a plane, we might get drawn for a frame before our
    /// update is called ... put this here to ensure we stay hidden appropriately."
    fn contain_react_to_transform_change(&mut self, world: &mut World, me: ModuleHandle) {
        self.hide_unless_opened(world, me);
    }

    /// `setOverrideDestination(override)` and `onDrawableBoundToObject()`.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::SetOverrideDestination(p) => {
                // "Instead of trying to float straight down, I am going to nail this spot."
                self.landing_override = p;
                self.is_landing_override_set = !crate::logic::modules::plant(1533);
                ModuleReply::Done
            }
            ModuleCall::OnDrawableBoundToObject => {
                self.hide_unless_opened(world, me);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl UpdateBehavior for ParachuteContain {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        self.oc.update_module()
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        OpenContainVirtuals::update(self, world, me)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::ParachuteContain(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::ParachuteContain(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(
        &mut self,
        _world: &mut World,
        _me: ModuleHandle,
        _call: ModuleCall,
    ) -> ModuleReply {
        ModuleReply::Unhandled
    }
}

impl Snapshot for ParachuteContain {
    /// `crc`: "extend base class".
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        Snapshot::crc(&mut self.oc, xfer)
    }

    /// `xfer` (version 1): the version, `OpenContain::xfer`, then the fields.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        Snapshot::xfer(&mut self.oc, xfer)?;
        self.xfer_own_fields(xfer)
    }

    /// `loadPostProcess`: "extend base class".
    fn load_post_process(&mut self) {
        Snapshot::load_post_process(&mut self.oc);
    }
}
