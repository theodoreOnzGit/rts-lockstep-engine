// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/DumbProjectileBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/DumbProjectileBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `DumbProjectileBehavior`: a shell that flies a fixed Bezier curve to its target (artillery, grenades, tank shells). Launched by a
//! weapon, it is placed at the launcher's barrel and computes its flight path: four control points (itself, two points above the highest
//! terrain along the way at `FirstPercentIndent` and `SecondPercentIndent` of the line, raised `FirstHeight` and `SecondHeight`, and the
//! target's centre), cut into as many segments as frames at the weapon's speed (scaled with the range for a `ScaleWeaponSpeed` weapon).
//! Each update moves it one step along the path (oriented to the path unless it tumbles), following a moving victim by up to
//! `FlightPathAdjustDistPerSecond`; it detonates at the end of the path, at the end of its lifespan, on the ground under a bridge it flew
//! over, or on a collision the detonation weapon accepts. A grenade (`GarrisonHitKillCount`) that hits a garrisoned building kills the
//! garrisoned units of the right kinds instead. It is an update module and the object's `ProjectileUpdateInterface`.
//!
//! # Upstream facts kept
//!
//! - `xfer` saves neither the flight path nor the step, the bonus flags or whether it has detonated.
//! - The garrison kill's loop reads the next item before it tests the count, so a loop that runs off the list reads `end()`
//!   ([`Undefined::EndIteratorDereferenced`]).
//! - A collision before any launch reads the detonation template through `NULL` ([`Undefined::NullWeaponTemplate`]); the detonation and the
//!   garrison kill read the body module unchecked ([`Undefined::NullBodyModule`]) and the controlling player ([`Undefined::NullControllingPlayer`]).
//! - The flight path's step 0 is oriented as step 1, so a path of one point reads past its end ([`Undefined::ArrayIndexOutOfRange`]); a
//!   segment count that is not an `Int` (a zero speed) is undefined too ([`Undefined::FloatToIntOutOfRange`]).
//!
//! The weapon calls go through the world ([`World::module_position_projectile_for_launch`], [`World::module_should_projectile_collide_with`],
//! [`World::module_handle_projectile_detonation`]).
//!
//! `p1w13-sonnet`, batch R68, 2026-10-26.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::bezier_segment::{BezierError, BezierSegment};
use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::game_type::{INVALID_ID, LAYER_GROUND};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::KindOfMaskType;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::OBJECT_STATUS_NO_COLLISIONS;
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_UNRESISTABLE, DEATH_NORMAL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::object_state::mask_of;
use crate::logic::world::partition::DistanceCalculationType;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

/// `DEFAULT_MAX_LIFESPAN`: `10 * LOGICFRAMES_PER_SECOND`.
const DEFAULT_MAX_LIFESPAN: u32 = 10 * LOGICFRAMES_PER_SECOND as u32;
/// `DEATH_DETONATED` (`DeathType`).
const DEATH_DETONATED: i32 = 10;
/// `PI` (`Real`).
const PI: f32 = std::f32::consts::PI;

/// An FX list by name (`INI::parseFXList`: `None` is `NULL`).
pub type Named = Option<Vec<u8>>;

/// `class DumbProjectileBehaviorModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct DumbProjectileBehaviorModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_maxLifespan`.
    pub max_lifespan: u32,
    /// `m_detonateCallsKill`, `m_orientToFlightPath` (TRUE), `m_tumbleRandomly`.
    pub detonate_calls_kill: bool,
    pub orient_to_flight_path: bool,
    pub tumble_randomly: bool,
    /// The Bezier curve's inner control points: `m_firstHeight`, `m_secondHeight`, `m_firstPercentIndent`, `m_secondPercentIndent`.
    pub first_height: f32,
    pub second_height: f32,
    pub first_percent_indent: f32,
    pub second_percent_indent: f32,
    /// `m_garrisonHitKillCount`, `m_garrisonHitKillKindof`, `m_garrisonHitKillKindofNot`, `m_garrisonHitKillFX`.
    pub garrison_hit_kill_count: i32,
    pub garrison_hit_kill_kind_of: KindOfMaskType,
    pub garrison_hit_kill_kind_of_not: KindOfMaskType,
    pub garrison_hit_kill_fx: Named,
    /// `m_flightPathAdjustDistPerFrame`.
    pub flight_path_adjust_dist_per_frame: f32,
}

impl Default for DumbProjectileBehaviorModuleData {
    /// `DumbProjectileBehaviorModuleData()`.
    fn default() -> Self {
        DumbProjectileBehaviorModuleData {
            update: UpdateModuleData::new(),
            max_lifespan: DEFAULT_MAX_LIFESPAN,
            detonate_calls_kill: false,
            orient_to_flight_path: true,
            tumble_randomly: false,
            first_height: 0.0,
            second_height: 0.0,
            first_percent_indent: 0.0,
            second_percent_indent: 0.0,
            garrison_hit_kill_count: 0,
            garrison_hit_kill_kind_of: KindOfMaskType::default(),
            garrison_hit_kill_kind_of_not: KindOfMaskType::default(),
            garrison_hit_kill_fx: None,
            flight_path_adjust_dist_per_frame: 0.0,
        }
    }
}

/// `INI::parseFXList`: the next token, `None` for `NULL`.
fn parse_named(ini: &mut Ini<'_>) -> IniResult<Named> {
    let name = ini.get_next_token(None)?;
    Ok(if name.eq_ignore_ascii_case(b"None") { None } else { Some(name) })
}

static FIELD_PARSE: [FieldParse<DumbProjectileBehaviorModuleData>; 14] = [
    FieldParse::new("MaxLifespan", |i, d, _| { d.max_lifespan = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("TumbleRandomly", |i, d, _| { d.tumble_randomly = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("DetonateCallsKill", |i, d, _| { d.detonate_calls_kill = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("OrientToFlightPath", |i, d, _| { d.orient_to_flight_path = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("FirstHeight", |i, d, _| { d.first_height = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("SecondHeight", |i, d, _| { d.second_height = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("FirstPercentIndent", |i, d, _| { d.first_percent_indent = i.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("SecondPercentIndent", |i, d, _| { d.second_percent_indent = i.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("GarrisonHitKillRequiredKindOf", |i, d, _| d.garrison_hit_kill_kind_of.parse_from_ini(i), UserData::None),
    FieldParse::new("GarrisonHitKillForbiddenKindOf", |i, d, _| d.garrison_hit_kill_kind_of_not.parse_from_ini(i), UserData::None),
    FieldParse::new("GarrisonHitKillCount", |i, d, _| { d.garrison_hit_kill_count = i.parse_unsigned_int()? as i32; Ok(()) }, UserData::None),
    FieldParse::new("GarrisonHitKillFX", |i, d, _| { d.garrison_hit_kill_fx = parse_named(i)?; Ok(()) }, UserData::None),
    FieldParse::new("FlightPathAdjustDistPerSecond", |i, d, _| { d.flight_path_adjust_dist_per_frame = i.parse_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl DumbProjectileBehaviorModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, DumbProjectileBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<DumbProjectileBehaviorModuleData> {
        let mut data = DumbProjectileBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, DumbProjectileBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `DumbProjectileBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::DumbProjectileBehavior(Arc::new(DumbProjectileBehaviorModuleData::friend_new_module_data(ini)?)))
}

/// `class DumbProjectileBehavior : public UpdateModule, public ProjectileUpdateInterface`.
#[derive(Clone, Debug)]
pub struct DumbProjectileBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    data: Arc<DumbProjectileBehaviorModuleData>,
    /// `m_launcherID`, `m_victimID`.
    launcher_id: ObjectId,
    victim_id: ObjectId,
    /// `m_detonationWeaponTmpl` (the store's index; `None` for `NULL`).
    detonation_weapon_tmpl: Option<usize>,
    /// `m_lifespanFrame`.
    lifespan_frame: u32,
    /// `m_flightPath`.
    flight_path: Vec<Coord3D>,
    /// `m_flightPathStart`, `m_flightPathEnd`, `m_flightPathSpeed`, `m_flightPathSegments`, `m_currentFlightPathStep`.
    flight_path_start: Coord3D,
    flight_path_end: Coord3D,
    flight_path_speed: f32,
    flight_path_segments: i32,
    current_flight_path_step: i32,
    /// `m_extraBonusFlags`.
    extra_bonus_flags: u32,
    /// `m_hasDetonated`.
    has_detonated: bool,
    /// Every weapon template's name and store position, for `xfer` (taken from the store by the constructor's world half).
    template_names: Arc<Vec<(Vec<u8>, usize)>>,
}

impl PartialEq for DumbProjectileBehavior {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl DumbProjectileBehavior {
    /// The constructor (it touches no world): the base and the zeroed state (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, data: Option<Arc<DumbProjectileBehaviorModuleData>>) -> IniResult<Self> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        Ok(DumbProjectileBehavior {
            update,
            data,
            launcher_id: INVALID_ID,
            victim_id: INVALID_ID,
            detonation_weapon_tmpl: None,
            lifespan_frame: 0,
            flight_path: Vec::new(),
            flight_path_start: Coord3D::default(),
            flight_path_end: Coord3D::default(),
            flight_path_speed: 0.0,
            flight_path_segments: 0,
            current_flight_path_step: 0,
            extra_bonus_flags: 0,
            has_detonated: false,
            template_names: Arc::new(Vec::new()),
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The data.
    pub fn module_data(&self) -> &Arc<DumbProjectileBehaviorModuleData> {
        &self.data
    }

    /// The state as words: the launcher, the victim, the template (+1, 0 for `NULL`), the lifespan frame, the segments, the speed, the start
    /// and the end, the step, the bonus flags, whether it has detonated, then the flight path (its length, then each point).
    pub fn state_words(&self) -> Vec<u32> {
        let b = |c: &Coord3D| [c.x.to_bits(), c.y.to_bits(), c.z.to_bits()];
        let mut v = vec![
            self.launcher_id as u32,
            self.victim_id as u32,
            self.detonation_weapon_tmpl.map_or(0, |t| t as u32 + 1),
            self.lifespan_frame,
            self.flight_path_segments as u32,
            self.flight_path_speed.to_bits(),
        ];
        v.extend(b(&self.flight_path_start));
        v.extend(b(&self.flight_path_end));
        v.extend([self.current_flight_path_step as u32, self.extra_bonus_flags, u32::from(self.has_detonated), self.flight_path.len() as u32]);
        for p in &self.flight_path {
            v.extend(b(p));
        }
        v
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

    /// `xfer` with the scheduling half the slot holds: the version, that half, then the fields.
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

    fn position(world: &World, id: ObjectId) -> Coord3D {
        world.objects.find(id).map(|o| o.get_position()).unwrap_or_default()
    }

    /// `victim->getGeometryInfo().getCenterPosition(*victim->getPosition(), result)`.
    fn center(world: &World, id: ObjectId) -> Coord3D {
        world.objects.find(id).map(|o| o.geometry_info.get_center_position(&o.get_position())).unwrap_or_default()
    }

    /// `projectileLaunchAtObjectOrPosition(victim, victimPos, launcher, wslot, specificBarrelToUse, detWeap, exhaustSysOverride)`.
    #[allow(clippy::too_many_arguments)] // (upstream's seven parameters, with the world and the module)
    fn projectile_launch(&mut self, world: &mut World, me: ModuleHandle, victim: ObjectId, victim_pos: Coord3D, launcher: ObjectId, wslot: i32, barrel: i32, template: usize) -> Result<(), Undefined> {
        let launcher = world.objects.find(launcher).map(|_| launcher);
        let victim = world.objects.find(victim).map(|_| victim);
        self.launcher_id = launcher.unwrap_or(INVALID_ID);
        self.extra_bonus_flags = launcher.and_then(|l| world.objects.find(l)).map_or(0, |o| o.weapon_bonus_condition);
        self.victim_id = victim.unwrap_or(INVALID_ID);
        self.detonation_weapon_tmpl = Some(template);
        self.lifespan_frame = world.frame().wrapping_add(self.data.max_lifespan);
        world.module_position_projectile_for_launch(me.object, launcher, wslot, barrel);
        self.projectile_fire(world, me, victim, victim_pos, template)
    }

    /// `projectileFireAtObjectOrPosition(victim, victimPos, detWeap, exhaustSysOverride)`.
    fn projectile_fire(&mut self, world: &mut World, me: ModuleHandle, victim: Option<ObjectId>, victim_pos: Coord3D, template: usize) -> Result<(), Undefined> {
        let d = self.data.clone();
        let statics = world.statics.clone();
        let t = &statics.weapon_store.templates()[template];
        let (weapon_speed, min_weapon_speed) = (t.weapon_speed, t.min_weapon_speed);
        // "if an object, aim at the center, not the ground part"
        let victim_pos_to_use = match victim {
            Some(v) => Self::center(world, v),
            None => victim_pos,
        };
        if t.is_scale_weapon_speed && !crate::logic::modules::plant(2381) {
            // "Some weapons want to scale their start speed to the range"
            let min_range = t.get_minimum_attack_range();
            let max_range = t.get_unmodified_attack_range();
            let range = world.partition_distance_squared_to_pos(me.object, &victim_pos_to_use, DistanceCalculationType::FromCenter2D).sqrt();
            let range_ratio = (range - min_range) / (max_range - min_range);
            self.flight_path_speed = (range_ratio * (weapon_speed - min_weapon_speed)) + min_weapon_speed;
        } else {
            self.flight_path_speed = weapon_speed;
        }
        let physics = world.objects.find(me.object).and_then(|o| o.physics);
        if let (true, Some(p)) = (d.tumble_randomly, physics) {
            let r = 1.0 / PI;
            let pitch = world.random.game_logic_random_value_real(-r, r);
            world.module_call(me.object, p, ModuleCall::PhysicsSetPitchRate(pitch));
            let yaw = world.random.game_logic_random_value_real(-r, r);
            world.module_call(me.object, p, ModuleCall::PhysicsSetYawRate(yaw));
            let roll = world.random.game_logic_random_value_real(-r, r);
            world.module_call(me.object, p, ModuleCall::PhysicsSetRollRate(roll));
        }
        self.flight_path_start = Self::position(world, me.object);
        self.flight_path_end = victim_pos_to_use;
        if !self.calc_flight_path(world, true)? {
            // "Can only fail if wildly incorrect points"
            world.body_destroy_object(me.object);
            return Ok(());
        }
        self.current_flight_path_step = 0; // "We are at the first point, because the launching put us there"
        Ok(())
    }

    /// `calcFlightPath(recalcNumSegments)`: whether the line is on the map.
    fn calc_flight_path(&mut self, world: &mut World, recalc_num_segments: bool) -> Result<bool, Undefined> {
        let d = self.data.clone();
        // "First point is us, last point is them"
        let mut control_points = [self.flight_path_start, Coord3D::default(), Coord3D::default(), self.flight_path_end];
        let (on_map, mut highest_intervening_terrain) = world.partition_estimate_terrain_max_along_line(control_points[0], control_points[3]);
        if !on_map {
            return Ok(false);
        }
        // "X and Y for inner points are along the line between us"
        let mut target_vector = Vector3::new(
            control_points[3].x - control_points[0].x,
            control_points[3].y - control_points[0].y,
            control_points[3].z - control_points[0].z,
        );
        let target_distance = target_vector.length();
        target_vector.normalize();
        let first_point_along_line = target_vector * (target_distance * d.first_percent_indent);
        let second_point_along_line = target_vector * (target_distance * d.second_percent_indent);
        control_points[1].x = first_point_along_line.x + control_points[0].x;
        control_points[1].y = first_point_along_line.y + control_points[0].y;
        control_points[2].x = second_point_along_line.x + control_points[0].x;
        control_points[2].y = second_point_along_line.y + control_points[0].y;
        // "Z's are determined using the highest intervening height so they won't hit hills, low end bounded by current Zs"
        highest_intervening_terrain = highest_intervening_terrain.max(control_points[0].z);
        highest_intervening_terrain = highest_intervening_terrain.max(control_points[3].z);
        control_points[1].z = highest_intervening_terrain + d.first_height;
        control_points[2].z = highest_intervening_terrain + if crate::logic::modules::plant(2382) { d.first_height } else { d.second_height };
        let flight_curve = BezierSegment::from_points(control_points[0], control_points[1], control_points[2], control_points[3]);
        let bezier = |e: BezierError| match e {
            BezierError::NegativeSegmentCount => Undefined::VectorLengthError,
            BezierError::SubdivisionLimit => Undefined::StackOverflow,
        };
        if recalc_num_segments {
            let flight_distance = flight_curve.get_approximate_length(crate::common::bezier_segment::USUAL_TOLERANCE).map_err(bezier)?;
            let segments = (flight_distance / self.flight_path_speed).ceil();
            if !(segments >= i32::MIN as f32 && segments < 2_147_483_648.0) {
                return Err(Undefined::FloatToIntOutOfRange);
            }
            self.flight_path_segments = segments as i32;
        }
        flight_curve.get_segment_points(self.flight_path_segments, &mut self.flight_path).map_err(bezier)?;
        Ok(true)
    }

    /// `projectileHandleCollision(other)`.
    fn handle_collision(&mut self, world: &mut World, me: ModuleHandle, other: ObjectId) -> Result<bool, Undefined> {
        let d = self.data.clone();
        if world.objects.find(other).is_some() {
            let launcher = world.objects.find(self.launcher_id).map(|_| self.launcher_id);
            let template = self.detonation_weapon_tmpl.ok_or(Undefined::NullWeaponTemplate)?;
            // "if it's not the specific thing we were targeting, see if we should incidentally collide..."
            if !world.module_should_projectile_collide_with(template, launcher, me.object, other, self.victim_id) && !crate::logic::modules::plant(2383) {
                return Ok(true);
            }
            if d.garrison_hit_kill_count > 0 {
                if let Some(c) = world.objects.find(other).and_then(|o| o.contain) {
                    let count = world.module_call(other, c, ModuleCall::GetContainCount).as_int(0);
                    let garrisonable = world.module_call(other, c, ModuleCall::IsGarrisonable).as_bool();
                    if count > 0 && garrisonable && !world.module_call(other, c, ModuleCall::IsImmuneToClearBuildingAttacks).as_bool() {
                        // "garrisonable buildings subvert the normal process here."
                        let items = world.module_call(other, c, ModuleCall::GetContainedItems);
                        let items = match items { ModuleReply::Ids(v) => v, _ => Vec::new() };
                        let mut num_killed = 0;
                        // `for (it = items->begin(); *it != NULL && numKilled < count; )`: the item is read before the count is tested, so
                        // a loop that runs off the list's end reads `end()`
                        let mut it = 0;
                        loop {
                            let Some(&thing_to_kill) = items.get(it) else { return Err(Undefined::EndIteratorDereferenced) };
                            if num_killed >= d.garrison_hit_kill_count {
                                break;
                            }
                            it += 1;
                            let Some(t) = world.objects.find(thing_to_kill) else { continue };
                            if !t.is_effectively_dead() && t.thing.is_kind_of_multi(&d.garrison_hit_kill_kind_of, &d.garrison_hit_kill_kind_of_not) {
                                if let Some(l) = launcher {
                                    world.body_score_the_kill(l, thing_to_kill);
                                }
                                if world.get_body_module(thing_to_kill).is_none() {
                                    return Err(Undefined::NullBodyModule);
                                }
                                world.kill(thing_to_kill, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
                                num_killed += 1;
                            }
                        }
                        if num_killed > 0 && !crate::logic::modules::plant(2384) {
                            // "note, fx is played at center of building, not at grenade's location"
                            if let Some(fx) = d.garrison_hit_kill_fx.clone() {
                                world.emit(ObjectClientEvent::FxListDoFxObj { fx, object_id: other });
                            }
                            let player = world.get_controlling_player(me.object).ok_or(Undefined::NullControllingPlayer)?;
                            world.body_academy_cleared_garrisoned_building(player);
                            // "don't do the normal explosion; just destroy ourselves & return"
                            world.body_destroy_object(me.object);
                            return Ok(true);
                        }
                    }
                }
            }
        }
        // "collided with something... blow'd up!"
        self.detonate(world, me)?;
        // "mark ourself as 'no collisions' (since we might still exist in slow death mode)"
        world.set_status(me.object, mask_of(OBJECT_STATUS_NO_COLLISIONS), true);
        Ok(true)
    }

    /// `obj->attemptDamage(...)` of `DAMAGE_UNRESISTABLE`, `DEATH_DETONATED`, the body's maximum health: "don't call kill(); do it manually, so
    /// we can specify DEATH_DETONATED".
    fn detonation_damage(world: &mut World, me: ObjectId) -> Result<(), Undefined> {
        let body = world.get_body_module(me).ok_or(Undefined::NullBodyModule)?;
        let mut info = DamageInfo::default();
        info.input.damage_type = DAMAGE_UNRESISTABLE;
        info.input.death_type = DEATH_DETONATED;
        info.input.source_id = INVALID_ID;
        info.input.amount = world.module_call(me, body, ModuleCall::GetMaxHealth).as_real(0.0);
        world.attempt_damage(me, &mut info);
        Ok(())
    }

    /// `detonate()`.
    fn detonate(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        if self.has_detonated && !crate::logic::modules::plant(2385) {
            return Ok(());
        }
        let obj = me.object;
        match self.detonation_weapon_tmpl {
            Some(t) => {
                let pos = Self::position(world, obj);
                if let Err(e) = world.module_handle_projectile_detonation(t, obj, pos, self.extra_bonus_flags) {
                    world.cuts.module_errors.push((me, e));
                }
                if self.data.detonate_calls_kill {
                    Self::detonation_damage(world, obj)?;
                } else {
                    world.body_destroy_object(obj);
                }
            }
            None => Self::detonation_damage(world, obj)?,
        }
        if world.has_drawable(obj) {
            world.emit(ObjectClientEvent::DrawableSetHidden { object_id: obj, hidden: true });
        }
        self.has_detonated = true;
        Ok(())
    }

    /// The body of `update()`: "Simulate one frame of a missile's behavior".
    fn run(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        let d = self.data.clone();
        let obj = me.object;
        if self.lifespan_frame != 0 && world.frame() >= self.lifespan_frame {
            // "lifetime demands detonation"
            return self.detonate(world, me);
        }
        if self.current_flight_path_step as u32 as usize >= self.flight_path.len() {
            // "No more steps to use ... This way, it _will_ explode at the target point."
            return self.detonate(world, me);
        }
        if self.victim_id != INVALID_ID && d.flight_path_adjust_dist_per_frame > 0.0 && world.objects.find(self.victim_id).is_some() {
            let new_victim_pos = Self::center(world, self.victim_id);
            let mut delta = Coord3D {
                x: new_victim_pos.x - self.flight_path_end.x,
                y: new_victim_pos.y - self.flight_path_end.y,
                z: new_victim_pos.z - self.flight_path_end.z,
            };
            let dist_victim_moved_sqr = delta.x * delta.x + delta.y * delta.y + delta.z * delta.z;
            if dist_victim_moved_sqr > 0.1 {
                let mut dist_victim_moved = dist_victim_moved_sqr.sqrt();
                if dist_victim_moved > d.flight_path_adjust_dist_per_frame && !crate::logic::modules::plant(2386) {
                    dist_victim_moved = d.flight_path_adjust_dist_per_frame;
                }
                delta.normalize();
                self.flight_path_end.x += dist_victim_moved * delta.x;
                self.flight_path_end.y += dist_victim_moved * delta.y;
                self.flight_path_end.z += dist_victim_moved * delta.z;
                if !self.calc_flight_path(world, false)? {
                    // ("Hmm, recalc of flight path returned false... should this happen?")
                    return self.detonate(world, me);
                }
            }
        }
        // "Otherwise, continue to force the flight path"
        let step = self.current_flight_path_step as u32 as usize;
        let flight_step = *self.flight_path.get(step).ok_or(Undefined::ArrayIndexOutOfRange)?;
        if d.orient_to_flight_path && !d.tumble_randomly {
            // step 0 is oriented "the same as if it were on frame 1"
            let (prev_pos, cur_pos) = if step > 0 {
                (self.flight_path[step - 1], flight_step)
            } else {
                (self.flight_path[0], *self.flight_path.get(1).ok_or(Undefined::ArrayIndexOutOfRange)?)
            };
            let mut cur_dir = Vector3::new(cur_pos.x - prev_pos.x, cur_pos.y - prev_pos.y, cur_pos.z - prev_pos.z);
            cur_dir.normalize(); // "buildTransformMatrix wants it this way"
            let mut orient_mtx = Matrix3D::IDENTITY;
            orient_mtx.build_transform_matrix(&Vector3::new(flight_step.x, flight_step.y, flight_step.z), &cur_dir);
            world.set_transform_matrix(obj, &orient_mtx);
        } else {
            world.set_position(obj, &flight_step);
        }
        // "we want to use getHighestLayerForDestination() here, so that anything even slightly below the bridge translates into GROUND"
        let old_layer = world.objects.find(obj).map_or(LAYER_GROUND, |o| o.layer);
        let pos = Self::position(world, obj);
        let new_layer = world.terrain_get_highest_layer_for_destination(&pos);
        world.set_layer(obj, new_layer);
        if old_layer != LAYER_GROUND && new_layer == LAYER_GROUND {
            // "see if we' still in the bridge's xy area"
            let mut tmp = Self::position(world, obj);
            tmp.z = 9999.0;
            let test_layer = world.terrain_get_highest_layer_for_destination(&tmp);
            if test_layer == old_layer {
                // "ensure we are slightly above the bridge, to account for fudge & sloppy art"
                const FUDGE: f32 = 2.0;
                tmp.z = world.terrain_get_layer_height(tmp.x, tmp.y, test_layer) + FUDGE;
                world.set_position(obj, &tmp);
                // "blow'd up!"
                return self.detonate(world, me);
            }
        }
        if !crate::logic::modules::plant(2387) {
            self.current_flight_path_step += 1;
        }
        Ok(())
    }
}

impl UpdateBehavior for DumbProjectileBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if let Err(u) = self.run(world, me) {
            world.cuts.module_errors.push((me, IniError::undefined(u)));
        }
        UPDATE_SLEEP_NONE // "This no longer flys with physics, so it needs to not sleep"
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_object_id(&mut self.launcher_id)?;
        xfer.xfer_object_id(&mut self.victim_id)?;
        xfer.xfer_int(&mut self.flight_path_segments)?;
        xfer.xfer_real(&mut self.flight_path_speed)?;
        xfer.xfer_coord3d(&mut self.flight_path_start)?;
        xfer.xfer_coord3d(&mut self.flight_path_end)?;
        // "weapon template", by name (`xfer` has no world: the store's names were taken by the constructor's world half)
        let names = self.template_names.clone();
        let mut name = self.detonation_weapon_tmpl.and_then(|t| names.iter().find(|(_, i)| *i == t)).map(|(n, _)| n.clone()).unwrap_or_default();
        xfer.xfer_ascii_string(&mut name)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.detonation_weapon_tmpl = if name.is_empty() {
                None
            } else {
                // `TheWeaponStore->findWeaponTemplate(name)`; "Unknown weapon template": `throw SC_INVALID_DATA` (as `ActiveBody`'s, the status
                // `XferStatus::ListNotEmpty`)
                Some(names.iter().find(|(n, _)| n.eq_ignore_ascii_case(&name)).map(|(_, i)| *i).ok_or(XferStatus::ListNotEmpty)?)
            };
        }
        if crate::logic::modules::plant(2388) {
            return Ok(());
        }
        xfer.xfer_unsigned_int(&mut self.lifespan_frame)
    }
    /// The constructor's world half: the store's template names, for `xfer`.
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        let store = &world.statics.weapon_store;
        self.template_names = Arc::new(store.templates().iter().enumerate().map(|(i, t)| (t.name.to_vec(), i)).collect());
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::DumbProjectileBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::DumbProjectileBehavior(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::HasProjectileUpdateInterface => ModuleReply::Bool(true),
            ModuleCall::ProjectileGetLauncherId => ModuleReply::Id(self.launcher_id),
            ModuleCall::ProjectileLaunchAtObjectOrPosition { victim, destination, source, wslot, barrel, template, .. } => {
                if let Err(u) = self.projectile_launch(world, me, victim, destination, source, wslot, barrel, template) {
                    world.cuts.module_errors.push((me, IniError::undefined(u)));
                }
                ModuleReply::Done
            }
            ModuleCall::ProjectileHandleCollision { other } => match self.handle_collision(world, me, other) {
                Ok(b) => ModuleReply::Bool(b),
                Err(u) => {
                    world.cuts.module_errors.push((me, IniError::undefined(u)));
                    ModuleReply::Bool(true)
                }
            },
            // `setFramesTillCountermeasureDiversionOccurs` and `projectileNowJammed` are empty
            ModuleCall::ProjectileSetFramesTillCountermeasureDiversion(_) | ModuleCall::ProjectileNowJammed => ModuleReply::Done,
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for DumbProjectileBehavior {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = DumbProjectileBehaviorModuleData::default();
        assert_eq!(d.max_lifespan, 300);
        assert!(d.orient_to_flight_path && !d.tumble_randomly && !d.detonate_calls_kill);
        let m = DumbProjectileBehavior::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words().len(), 16);
    }
}
