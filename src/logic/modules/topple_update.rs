// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ToppleUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ToppleUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ToppleUpdate`: a tree, a streetlight or a fence that a crusher (or a script, or a weapon) topples. It turns about its own z axis to lie
//! across the fall, falls with a growing angular velocity until it is nearly flat, bounces at a fraction of that velocity until the bounce is too
//! slow, and then dies (`DEATH_TOPPLED`), leaving its stump if it has one.
//!
//! # Design
//!
//! The rotations are the object's transform (`World::set_transform_matrix` on the real `Thing`), with `Matrix3D`'s own `In_Place_Pre_Rotate_*`.
//! The drawable's part is client events: the sway stops ([`ObjectClientEvent::DrawableStopSway`]), the stump burns if the tree's drawable is
//! burned ([`ObjectClientEvent::DrawableBurnStumpIfBurned`]: the model condition flags are the client's), the shadow goes. The script engine's
//! topple directions are `cuts.script_topple_directions` (the script engine is layer 10's).
//!
//! # Upstream facts kept
//!
//! - `applyTopplingForce` reads the drawable unchecked (`getClientUpdateModules`, `setModelConditionState`): `Undefined::NullDrawable`; and
//!   the direction (`Undefined::NullLocation` for none).
//! - `onCollide` topples the object through `Object::topple`, which finds the object's first `ToppleUpdate`: this one (the module answers its
//!   own `isAbleToBeToppled` and `applyTopplingForce` without a round trip through the world, where it is out of its slot).
//! - `atan2` is the C `double` function; the topple state is saved as the four bytes of the enum.
//! - `m_doBounceFX` is saved but never set but by a load.
//!
//! `p1w13-sonnet`, batch R28, 2026-10-24.

use std::sync::Arc;

use crate::common::base_type::{real_to_int_floor, Coord3D, PI};
use crate::common::game_common::{normalize_angle, std_angle_diff};
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData,
};
use crate::common::model_state::MODELCONDITION_TOPPLED;
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_COLLIDE};
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{
    DamageInfo, DeathType, DAMAGE_UNRESISTABLE, DEATH_NORMAL, HUGE_DAMAGE_AMOUNT,
};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER,
    UPDATE_SLEEP_NONE,
};
use crate::logic::modules::crate_collide::CrateCollideModuleData;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::ww::vector3::Vector3;

/// `TOPPLE_OPTIONS_NO_BOUNCE`: "do not bounce when hit the ground".
pub const TOPPLE_OPTIONS_NO_BOUNCE: u32 = 0x0000_0001;
/// `TOPPLE_OPTIONS_NO_FX`: "do not play any FX when hit the ground".
pub const TOPPLE_OPTIONS_NO_FX: u32 = 0x0000_0002;
/// `DEATH_TOPPLED`.
const DEATH_TOPPLED: DeathType = 6;
/// `ANGULAR_LIMIT`: "this is our "bounce" limit -- slightly less that 90 degrees, to account for slop".
const ANGULAR_LIMIT: f32 = PI / 2.0 - PI / 64.0;

/// `ToppleState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum ToppleState {
    /// `TOPPLE_UPRIGHT`.
    #[default]
    Upright = 0,
    /// `TOPPLE_FALLING`.
    Falling = 1,
    /// `TOPPLE_DOWN`.
    Down = 2,
}

/// `class ToppleUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct ToppleUpdateModuleData {
    /// The base (`UpdateModuleData`, which adds nothing to `ModuleData`).
    pub update: UpdateModuleData,
    /// `m_toppleFX`, by name (`None` for `NULL`).
    pub topple_fx: Option<Vec<u8>>,
    /// `m_bounceFX`.
    pub bounce_fx: Option<Vec<u8>>,
    /// `m_stumpName`.
    pub stump_name: Vec<u8>,
    /// `m_initialVelocityPercent`.
    pub initial_velocity_percent: f32,
    /// `m_initialAccelPercent`.
    pub initial_accel_percent: f32,
    /// `m_bounceVelocityPercent`.
    pub bounce_velocity_percent: f32,
    /// `m_killWhenToppled`.
    pub kill_when_toppled: bool,
    /// `m_killWhenStartToppled`.
    pub kill_when_start_toppled: bool,
    /// `m_killStumpWhenToppled`.
    pub kill_stump_when_toppled: bool,
    /// `m_toppleLeftOrRightOnly`: "constrained to topple to my left or right only".
    pub topple_left_or_right_only: bool,
    /// `m_reorientToppledRubble`.
    pub reorient_toppled_rubble: bool,
}

impl Default for ToppleUpdateModuleData {
    /// `ToppleUpdateModuleData()`.
    fn default() -> Self {
        ToppleUpdateModuleData {
            update: UpdateModuleData::default(),
            topple_fx: None,
            bounce_fx: None,
            stump_name: Vec::new(),
            initial_velocity_percent: 0.2,
            initial_accel_percent: 0.01,
            bounce_velocity_percent: 0.3,
            kill_when_toppled: true,
            kill_when_start_toppled: false,
            kill_stump_when_toppled: false,
            topple_left_or_right_only: false,
            reorient_toppled_rubble: false,
        }
    }
}

/// `ToppleUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<ToppleUpdateModuleData>; 12] = [
    FieldParse::new(
        "ToppleFX",
        |ini, d, _| {
            d.topple_fx = CrateCollideModuleData::parse_fx_list(ini)?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "BounceFX",
        |ini, d, _| {
            d.bounce_fx = CrateCollideModuleData::parse_fx_list(ini)?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "StumpName",
        |ini, d, _| {
            d.stump_name = ini.parse_ascii_string()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "KillWhenStartToppling",
        |ini, d, _| {
            d.kill_when_start_toppled = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "KillWhenFinishedToppling",
        |ini, d, _| {
            d.kill_when_toppled = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "KillStumpWhenToppled",
        |ini, d, _| {
            d.kill_stump_when_toppled = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "ToppleLeftOrRightOnly",
        |ini, d, _| {
            d.topple_left_or_right_only = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "ReorientToppledRubble",
        |ini, d, _| {
            d.reorient_toppled_rubble = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "InitialVelocityPercent",
        |ini, d, _| {
            d.initial_velocity_percent = ini.parse_percent_to_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "InitialAccelPercent",
        |ini, d, _| {
            d.initial_accel_percent = ini.parse_percent_to_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "BounceVelocityPercent",
        |ini, d, _| {
            d.bounce_velocity_percent = ini.parse_percent_to_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl ToppleUpdateModuleData {
    /// `buildFieldParse(p)`: `ModuleData`'s (no fields of its own besides the behaviour base's), then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, ToppleUpdateModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ToppleUpdateModuleData> {
        let mut data = ToppleUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, ToppleUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class ToppleUpdate : public UpdateModule, public CollideModuleInterface`.
#[derive(Clone, Debug)]
pub struct ToppleUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getToppleUpdateModuleData()`.
    data: Arc<ToppleUpdateModuleData>,
    /// `m_angularVelocity`.
    angular_velocity: f32,
    /// `m_angularAcceleration`.
    angular_acceleration: f32,
    /// `m_toppleDirection`: "Z-less direction we are toppling".
    topple_direction: Coord3D,
    /// `m_toppleState`.
    topple_state: ToppleState,
    /// `m_angularAccumulation`: "How much have I rotated so I know when to bounce".
    angular_accumulation: f32,
    /// `m_angleDeltaX`: "how much to modify x angle each frame".
    angle_delta_x: f32,
    /// `m_numAngleDeltaX`: "how many frames to tweak x angle".
    num_angle_delta_x: i32,
    /// `m_doBounceFX`.
    do_bounce_fx: bool,
    /// `m_options`.
    options: u32,
    /// `m_stumpID`.
    stump_id: ObjectId,
}

impl PartialEq for ToppleUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.state_words() == other.state_words()
    }
}

/// `angleClosestTo(a1, a2, desired)`: the normalised one of `a1` and `a2` closer to `desired` (`a2` on a tie).
fn angle_closest_to(a1: f32, a2: f32, desired: f32) -> f32 {
    let a1 = normalize_angle(a1);
    let a2 = normalize_angle(a2);
    if std_angle_diff(desired, a1).abs() < std_angle_diff(desired, a2).abs() {
        a1
    } else {
        a2
    }
}

/// `deathByToppling(obj)`: "a special "topppling" damage type here so that toppled stuff can have different damage/die modules".
fn death_by_toppling(world: &mut World, obj: ObjectId) {
    let mut info = DamageInfo::default();
    info.input.damage_type = DAMAGE_UNRESISTABLE;
    info.input.death_type = DEATH_TOPPLED;
    info.input.source_id = INVALID_ID;
    info.input.amount = HUGE_DAMAGE_AMOUNT;
    world.attempt_damage(obj, &mut info);
}

impl ToppleUpdate {
    /// `getInterfaceMask()`: the update module's and `MODULEINTERFACE_COLLIDE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_COLLIDE
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and the state.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<ToppleUpdateModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(ToppleUpdate {
            update,
            data,
            angular_velocity: 0.0,
            angular_acceleration: 0.0,
            topple_direction: Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            topple_state: ToppleState::Upright,
            angular_accumulation: 0.0,
            angle_delta_x: 0.0,
            num_angle_delta_x: 0,
            do_bounce_fx: false,
            options: 0,
            stump_id: INVALID_ID,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getToppleUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<ToppleUpdateModuleData> {
        &self.data
    }

    /// The state the test compares, as words.
    pub fn state_words(&self) -> [u32; 12] {
        [
            self.angular_velocity.to_bits(),
            self.angular_acceleration.to_bits(),
            self.topple_direction.x.to_bits(),
            self.topple_direction.y.to_bits(),
            self.topple_direction.z.to_bits(),
            self.topple_state as u32,
            self.angular_accumulation.to_bits(),
            self.angle_delta_x.to_bits(),
            self.num_angle_delta_x as u32,
            u32::from(self.do_bounce_fx),
            self.options,
            self.stump_id as u32,
        ]
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object: asleep until toppled.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
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

    /// `isAbleToBeToppled()`: upright.
    pub fn is_able_to_be_toppled(&self) -> bool {
        self.topple_state == ToppleState::Upright
    }

    fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
        world.cuts.module_errors.push((me, IniError::undefined(u)));
    }

    /// `applyTopplingForce(toppleDirection, toppleSpeed, options)`: "Start the toppling process by giving a force vector".
    pub fn apply_toppling_force(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        direction: Option<Coord3D>,
        speed: f32,
        options: u32,
    ) {
        if let Err(u) = self.apply_toppling_force_inner(world, me, direction, speed, options) {
            Self::undefined(world, me, u);
        }
    }

    fn apply_toppling_force_inner(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        direction: Option<Coord3D>,
        speed: f32,
        options: u32,
    ) -> Result<(), Undefined> {
        let id = me.object;
        if world
            .objects
            .find(id)
            .is_none_or(|o| o.is_effectively_dead())
        {
            return Ok(());
        }
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
        let d = self.data.clone();
        if d.kill_when_start_toppled && !crate::logic::modules::plant(1981) {
            update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
            world.kill(id, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
            return Ok(());
        }
        self.topple_direction = direction.ok_or(Undefined::NullLocation)?;
        if !crate::logic::modules::plant(1982) {
            self.topple_direction.normalize();
        }
        world.script_adjust_topple_direction(id, &mut self.topple_direction);
        self.angular_velocity = speed * d.initial_velocity_percent;
        self.angular_acceleration = speed
            * if crate::logic::modules::plant(1983) {
                d.initial_velocity_percent
            } else {
                d.initial_accel_percent
            };
        self.topple_state = ToppleState::Falling;
        self.options = options;
        // tell the drawable to stop swaying (`draw->getClientUpdateModules()`, unchecked)
        if !world.has_drawable(id) {
            return Err(Undefined::NullDrawable);
        }
        world.emit(ObjectClientEvent::DrawableStopSway { object_id: id });
        // rotate around the z-axis so that our x-axis is perpendicular to the topple direction.
        let cur_angle_x = normalize_angle(
            world
                .objects
                .find(id)
                .map_or(0.0, |o| o.thing.orientation()),
        );
        let mut topple_angle = normalize_angle(
            f64::from(self.topple_direction.y).atan2(f64::from(self.topple_direction.x)) as f32,
        );
        if d.topple_left_or_right_only {
            // it's a fence or such, and can only topple left or right, so pick the closest
            topple_angle =
                angle_closest_to(cur_angle_x + PI / 2.0, cur_angle_x - PI / 2.0, topple_angle);
            self.topple_direction.x = crate::common::trig::cos(topple_angle);
            self.topple_direction.y = crate::common::trig::sin(topple_angle);
            // go ahead and remove it from the pathfinder now, rather than waiting for the topple to finish
            if !crate::logic::modules::plant(1984) {
                world.cut(CutCall::PathfinderRemoveObjectFromPathfindMap { object: id });
            }
        }
        // desired angle is toppleAngle +/- pi/2, whichever is closer to curangle
        let desired_angle_x = angle_closest_to(
            topple_angle + PI / 2.0,
            topple_angle - PI / 2.0,
            cur_angle_x,
        );
        self.num_angle_delta_x = real_to_int_floor(ANGULAR_LIMIT / (self.angular_velocity * 2.0));
        if self.num_angle_delta_x < 1 && !crate::logic::modules::plant(1985) {
            self.num_angle_delta_x = 1;
        }
        self.angle_delta_x = (desired_angle_x - cur_angle_x) / self.num_angle_delta_x as f32;
        world.set_model_condition_state(id, MODELCONDITION_TOPPLED);
        if let Some(fx) = d.topple_fx.clone() {
            world.emit(ObjectClientEvent::FxListDoFxObj { fx, object_id: id });
        }
        // if this is a tree, create a stump
        if !d.stump_name.is_empty() {
            let template = world.thing_factory_find_template(&d.stump_name);
            let Some(template) = template else {
                // `TheThingFactory->newObject(NULL, NULL)` throws `ERROR_BAD_ARG`
                world
                    .cuts
                    .module_errors
                    .push((me, IniError::new(IniErrorKind::BadArg)));
                return Ok(());
            };
            if let Ok(stump) =
                world.thing_factory_new_object(template, None, ObjectStatusMaskType::default())
            {
                let (pos, angle) = world
                    .objects
                    .find(id)
                    .map(|o| (o.get_position(), o.thing.orientation()))
                    .unwrap_or_default();
                world.set_position(stump, &pos);
                world.set_orientation(stump, angle);
                self.stump_id = stump;
                // if we are "burned", then we will burn our stump (the drawable's flags are the client's)
                if world.has_drawable(id) {
                    world.emit(ObjectClientEvent::DrawableBurnStumpIfBurned {
                        object_id: id,
                        stump,
                    });
                }
            }
        }
        Ok(())
    }

    /// `onCollide(other, loc, normal)`: a crusher above level 1 topples the object away from it at its own speed (`Object::topple`, which finds
    /// this module).
    pub fn on_collide(&mut self, world: &mut World, me: ModuleHandle, other: ObjectId) {
        // Note that other == null means "collide with ground"
        let Some(o) = world.objects.find(other) else {
            return;
        };
        let other_pos = o.get_position();
        if world.get_crusher_level(other)
            > if crate::logic::modules::plant(1989) {
                0
            } else {
                1
            }
        {
            // Give a vector with direction to thing and my speed.
            let mut topple_vector = world
                .objects
                .find(me.object)
                .map(|o| o.get_position())
                .unwrap_or_default();
            topple_vector.x -= other_pos.x;
            topple_vector.y -= other_pos.y;
            topple_vector.z = 0.0;
            let vel = match world.objects.find(other).and_then(|o| o.physics) {
                Some(phys) => {
                    match world.module_call(other, phys, ModuleCall::PhysicsGetVelocity) {
                        ModuleReply::Coord(v) => v,
                        _ => Coord3D::default(),
                    }
                }
                None => Coord3D::default(),
            };
            // getObject()->topple( &toppleVector, vel.length(), TOPPLE_OPTIONS_NONE )
            if self.is_able_to_be_toppled() {
                self.apply_toppling_force(world, me, Some(topple_vector), vel.length(), 0);
            }
        }
    }
}

impl UpdateBehavior for ToppleUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    /// "Keep track of rotational fall distance, bounce and/or stop when needed."
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if self.topple_state == ToppleState::Upright || self.topple_state == ToppleState::Down {
            return UPDATE_SLEEP_FOREVER;
        }
        let d = self.data.clone();
        // if the velocity after a bounce will be this or lower, just stop at zero
        const VELOCITY_BOUNCE_LIMIT: f32 = 0.01;
        // and if this low, then skip the bounce sound
        const VELOCITY_BOUNCE_SOUND_LIMIT: f32 = 0.03;
        let id = me.object;
        if world.objects.find(id).is_none() {
            return UPDATE_SLEEP_FOREVER;
        }
        if self.num_angle_delta_x != 0 {
            let mut xfrm = *world
                .objects
                .find(id)
                .expect("checked above")
                .thing
                .transform_matrix();
            xfrm.in_place_pre_rotate_z(self.angle_delta_x);
            world.set_transform_matrix(id, &xfrm);
            self.num_angle_delta_x -= 1;
        }
        let mut cur_vel_to_use = self.angular_velocity;
        if self.angular_accumulation + cur_vel_to_use > ANGULAR_LIMIT
            && !crate::logic::modules::plant(1988)
        {
            cur_vel_to_use = ANGULAR_LIMIT - self.angular_accumulation;
        }
        let mut xfrm = *world
            .objects
            .find(id)
            .expect("checked above")
            .thing
            .transform_matrix();
        xfrm.in_place_pre_rotate_x(-cur_vel_to_use * self.topple_direction.y);
        xfrm.in_place_pre_rotate_y(cur_vel_to_use * self.topple_direction.x);
        world.set_transform_matrix(id, &xfrm);
        self.angular_accumulation += cur_vel_to_use;
        if self.angular_accumulation >= ANGULAR_LIMIT && self.angular_velocity > 0.0 {
            // Hit so either bounce or stop if too little remaining velocity.
            self.angular_velocity *= -d.bounce_velocity_percent;
            if (self.options & TOPPLE_OPTIONS_NO_BOUNCE != 0 && !crate::logic::modules::plant(1986))
                || self.angular_velocity.abs() < VELOCITY_BOUNCE_LIMIT
            {
                // too slow, just stop
                self.angular_velocity = 0.0;
                self.topple_state = ToppleState::Down;
                if d.kill_when_toppled {
                    death_by_toppling(world, id);
                    if d.reorient_toppled_rubble {
                        // we have a separate rubble state that needs to be upright, and centered on the new "center" pos...
                        if let Some(o) = world.objects.find(id) {
                            let pos = Vector3 {
                                x: 0.0,
                                y: 0.0,
                                z: o.geometry_info.get_max_height_above_position(),
                            };
                            let pos = crate::ww::matrix3d::Matrix3D::transform_vector(
                                o.thing.transform_matrix(),
                                &pos,
                            );
                            world.set_position(
                                id,
                                &Coord3D {
                                    x: pos.x,
                                    y: pos.y,
                                    z: pos.z,
                                },
                            );
                            // this relies on the fact that setOrientation always forces us straight up in the Z axis!
                            let angle = world
                                .objects
                                .find(id)
                                .map_or(0.0, |o| o.thing.orientation());
                            world.set_orientation(id, angle);
                        }
                    }
                }
                if d.kill_stump_when_toppled
                    && !crate::logic::modules::plant(1987)
                    && world.objects.find(self.stump_id).is_some()
                {
                    death_by_toppling(world, self.stump_id);
                }
            } else if self.angular_velocity.abs() >= VELOCITY_BOUNCE_SOUND_LIMIT {
                // fast enough bounce to warrant the bounce fx
                if self.options & TOPPLE_OPTIONS_NO_FX == 0 {
                    if let Some(fx) = d.bounce_fx.clone() {
                        world.emit(ObjectClientEvent::FxListDoFxObj { fx, object_id: id });
                    }
                }
            }
        } else {
            self.angular_velocity += self.angular_acceleration;
        }
        if world.has_drawable(id) {
            world.emit(ObjectClientEvent::DrawableSetShadowsEnabled {
                object_id: id,
                enabled: false,
            });
        }
        UPDATE_SLEEP_NONE
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
    }
    /// The fields after the base, in order; the state as the four bytes of the enum.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_real(&mut self.angular_velocity)?;
        xfer.xfer_real(&mut self.angular_acceleration)?;
        xfer.xfer_coord3d(&mut self.topple_direction)?;
        let mut state = (self.topple_state as i32).to_le_bytes();
        xfer.xfer_user(&mut state)?;
        self.topple_state = match i32::from_le_bytes(state) {
            0 => ToppleState::Upright,
            1 => ToppleState::Falling,
            _ => ToppleState::Down,
        };
        xfer.xfer_real(&mut self.angular_accumulation)?;
        xfer.xfer_real(&mut self.angle_delta_x)?;
        xfer.xfer_int(&mut self.num_angle_delta_x)?;
        xfer.xfer_bool(&mut self.do_bounce_fx)?;
        xfer.xfer_unsigned_int(&mut self.options)?;
        xfer.xfer_object_id(&mut self.stump_id)
    }
    /// The collide interface and the topple calls.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::IsAbleToBeToppled => ModuleReply::Bool(self.is_able_to_be_toppled()),
            ModuleCall::ApplyTopplingForce {
                direction,
                speed,
                options,
            } => {
                self.apply_toppling_force(world, me, direction, speed, options);
                ModuleReply::Done
            }
            ModuleCall::OnCollide { other, .. } => {
                self.on_collide(world, me, other);
                ModuleReply::Done
            }
            ModuleCall::WouldLikeToCollideWith { .. }
            | ModuleCall::IsHijackedVehicleCrateCollide
            | ModuleCall::IsCarBombCrateCollide
            | ModuleCall::IsRailroad
            | ModuleCall::IsSalvageCrateCollide
            | ModuleCall::IsSabotageBuildingCrateCollide => ModuleReply::Bool(false),
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::ToppleUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::ToppleUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for ToppleUpdate {
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
    }
}

/// The factory's `NewModuleDataProc` for `ToppleUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::ToppleUpdate(Arc::new(
        ToppleUpdateModuleData::friend_new_module_data(ini)?,
    )))
}
