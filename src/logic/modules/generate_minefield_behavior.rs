// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/GenerateMinefieldBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/GenerateMinefieldBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `GenerateMinefieldBehavior`: lays a minefield around its object (or a target point) when upgraded or, if so configured, when it dies:
//! rings of mines along the footprint's border (a rectangle for a box, a circle otherwise), several rings out to a distance ("smart
//! border"), or mines scattered at random through the footprint at a density. Each mine is placed unless the ground there is water or a
//! cliff or a structure would cover most of it; it scoots there from the generator. An upgradable field is replaced by its upgraded mines
//! once the object has `Upgrade_ChinaEMPMines`.
//!
//! [`ModuleCall::GenerateMinefieldSetTarget`] is `setMinefieldTarget`; the upgrade calls are the `UpgradeMux`'s.
//!
//! # Upstream facts kept
//!
//! - The data is a `BehaviorModuleData` (not an `UpdateModuleData`) with an `UpgradeMuxData`; `DistanceAroundObject` and
//!   `MinesPerSquareFoot` default to `TheGlobalData`'s standard minefield distance (40) and density (0.01). This port takes `GlobalData`'s
//!   constructor values (a map's `GameData.ini` override of them is not seen: Open, until the data constructor can read the store).
//! - The xfer is the version (1), **`BehaviorModule::xfer`** (not `UpdateModule`'s: the scheduling is not saved), the mux, the three flags,
//!   the target and the mine list (a count byte, then the ids); the CRC is `BehaviorModule`'s and the mux's.
//! - `placeMines` runs once (`m_generated`); the mine template is the upgraded one once upgraded; with none found nothing is placed (a debug
//!   crash). The generation FX is played on the object.
//! - The rings: along a line, `ceil(len / mineDiameter)` mines (at least one), every `len / n` from 0 (or the first step, for the rectangle's
//!   sides) to `len` inclusive; around a circle, `ceil(2 PI r / mineDiameter)` mines at angles below `2 PI - step / 2`; each point at
//!   ground height and jittered by `mineRadius * RandomJitter` in x then y. Rectangle corners are the object's transform applied to
//!   (+/-major, +/-minor) (the target position is not used for them).
//! - `placeMineAt`: the highest layer at z 99999; on the ground, water or a cliff skips the mine; the orientation is a random angle in
//!   [-PI, PI]; the mine's geometry shrunk by `radius * (1 - SkipIfThisMuchUnderStructure)` is tested against the potential collisions, and a
//!   structure among them skips it. The new mine is placed, oriented, given the generator as its producer, and told to scoot from the
//!   generator's position (the first module with a `LandMineInterface`); an upgradable field remembers its id.
//! - The footprint scatter tries up to 100 random points per mine for one at least a mine diameter from those placed, and skips a point
//!   inside the object's own footprint (centred on the target).
//!
//! # Where upstream is undefined
//!
//! Placing mines for an object with no controlling player (`getControllingPlayer()->getDefaultTeam()`: [`Undefined::NullControllingPlayer`]);
//! a line of length zero laid with its first point skipped (an increment of 0: the loop never ends, [`Undefined::EndlessLoop`]). The case
//! is recorded in `world.cuts.module_errors` and the call returns.
//!
//! `p1w13-sonnet`, batch R41, 2026-10-24.

use std::sync::Arc;

use crate::common::base_type::{real_to_int_ceil, Coord3D, PI};
use crate::common::game_type::{INVALID_ID, LAYER_GROUND};
use crate::common::geometry::{GeometryInfo, GeometryType};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::KINDOF_STRUCTURE;
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DIE, MODULEINTERFACE_UPGRADE};
use crate::logic::world::ids::TeamId;
use crate::common::trig;
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::module::upgrade_module::{UpgradeModuleData, UpgradeMux};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::modules::upgrade_mux_user::{upgrade_mux_call, UpgradeMuxUser};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

/// `GlobalData::m_standardMinefieldDistance`'s constructor value.
const STANDARD_MINEFIELD_DISTANCE: f32 = 40.0;
/// `GlobalData::m_standardMinefieldDensity`'s constructor value.
const STANDARD_MINEFIELD_DENSITY: f32 = 0.01;
/// The upgrade that replaces an upgradable field's mines.
const CHINA_EMP_MINES: &[u8] = b"Upgrade_ChinaEMPMines";

/// `class GenerateMinefieldBehaviorModuleData : public BehaviorModuleData`.
#[derive(Clone, Debug)]
pub struct GenerateMinefieldBehaviorModuleData {
    /// The `BehaviorModuleData` base.
    pub behavior: BehaviorModuleData,
    /// `m_upgradeMuxData`.
    pub upgrade: Arc<UpgradeModuleData>,
    /// `m_mineName`.
    pub mine_name: Vec<u8>,
    /// `m_mineNameUpgraded`.
    pub mine_name_upgraded: Vec<u8>,
    /// `m_mineUpgradeTrigger` (parsed, never read).
    pub mine_upgrade_trigger: Vec<u8>,
    /// `m_genFX` (`None` for `NULL`).
    pub gen_fx: Option<Vec<u8>>,
    /// `m_distanceAroundObject`.
    pub distance_around_object: f32,
    /// `m_minesPerSquareFoot`.
    pub mines_per_square_foot: f32,
    /// `m_randomJitter`.
    pub random_jitter: f32,
    /// `m_skipIfThisMuchUnderStructure`.
    pub skip_if_this_much_under_structure: f32,
    /// `m_onDeath`.
    pub on_death: bool,
    /// `m_borderOnly`.
    pub border_only: bool,
    /// `m_alwaysCircular`.
    pub always_circular: bool,
    /// `m_upgradable`.
    pub upgradable: bool,
    /// `m_smartBorder`.
    pub smart_border: bool,
    /// `m_smartBorderSkipInterior`.
    pub smart_border_skip_interior: bool,
}

impl Default for GenerateMinefieldBehaviorModuleData {
    /// `GenerateMinefieldBehaviorModuleData()`.
    fn default() -> Self {
        GenerateMinefieldBehaviorModuleData {
            behavior: BehaviorModuleData::new(),
            upgrade: Arc::new(UpgradeModuleData::new()),
            mine_name: Vec::new(),
            mine_name_upgraded: Vec::new(),
            mine_upgrade_trigger: Vec::new(),
            gen_fx: None,
            distance_around_object: STANDARD_MINEFIELD_DISTANCE,
            mines_per_square_foot: STANDARD_MINEFIELD_DENSITY,
            on_death: false,
            border_only: true,
            always_circular: false,
            upgradable: false,
            smart_border: false,
            smart_border_skip_interior: true,
            random_jitter: 0.0,
            skip_if_this_much_under_structure: 0.33,
        }
    }
}

/// `INI::parseFXList`: the next token, `None` for `None`.
fn parse_fx(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let name = ini.get_next_token(None)?;
    Ok(if name.eq_ignore_ascii_case(b"None") { None } else { Some(name) })
}

/// `dataFieldParse`.
static OWN_FIELD_PARSE: [FieldParse<GenerateMinefieldBehaviorModuleData>; 15] = [
    FieldParse::new("MineName", |i, d, _| { d.mine_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("UpgradedMineName", |i, d, _| { d.mine_name_upgraded = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("UpgradedTriggeredBy", |i, d, _| { d.mine_upgrade_trigger = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("GenerationFX", |i, d, _| { d.gen_fx = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("DistanceAroundObject", |i, d, _| { d.distance_around_object = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MinesPerSquareFoot", |i, d, _| { d.mines_per_square_foot = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("GenerateOnlyOnDeath", |i, d, _| { d.on_death = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("BorderOnly", |i, d, _| { d.border_only = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("SmartBorder", |i, d, _| { d.smart_border = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("SmartBorderSkipInterior", |i, d, _| { d.smart_border_skip_interior = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("AlwaysCircular", |i, d, _| { d.always_circular = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("Upgradable", |i, d, _| { d.upgradable = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("RandomJitter", |i, d, _| { d.random_jitter = i.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("SkipIfThisMuchUnderStructure", |i, d, _| { d.skip_if_this_much_under_structure = i.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `UpgradeMuxData::getFieldParse()`'s table, for this data class.
static UPGRADE_FIELD_PARSE: [FieldParse<GenerateMinefieldBehaviorModuleData>; 6] = upgrade_module_data_field_parse!(GenerateMinefieldBehaviorModuleData, upgrade);

impl GenerateMinefieldBehaviorModuleData {
    /// `buildFieldParse(p)`: the behaviour base, this class's fields, then the upgrade mux's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, GenerateMinefieldBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&OWN_FIELD_PARSE)?;
        p.add(&UPGRADE_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<GenerateMinefieldBehaviorModuleData> {
        let mut data = GenerateMinefieldBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, GenerateMinefieldBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The mine template `placeMines` lays: its id and geometry.
#[derive(Clone)]
struct MineTemplate {
    id: u16,
    geom: GeometryInfo,
}

/// `class GenerateMinefieldBehavior : public UpdateModule, public DieModuleInterface, public UpgradeMux`.
#[derive(Clone, Debug)]
pub struct GenerateMinefieldBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getGenerateMinefieldBehaviorModuleData()`.
    data: Arc<GenerateMinefieldBehaviorModuleData>,
    /// The `UpgradeMux` base.
    mux: UpgradeMux,
    /// `m_target`.
    target: Coord3D,
    /// `m_hasTarget`.
    has_target: bool,
    /// `m_generated`.
    generated: bool,
    /// `m_upgraded`.
    upgraded: bool,
    /// `m_mineList`.
    mine_list: Vec<ObjectId>,
}

impl PartialEq for GenerateMinefieldBehavior {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.mux == other.mux
            && self.state_words() == other.state_words()
    }
}

impl GenerateMinefieldBehavior {
    /// `getInterfaceMask()`: `UpdateModule`'s, die and upgrade.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE | MODULEINTERFACE_UPGRADE
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and the state.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<GenerateMinefieldBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(UpdateModuleData { behavior: data.behavior.clone() });
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(GenerateMinefieldBehavior {
            update,
            data,
            mux: UpgradeMux::new(),
            target: Coord3D::default(),
            has_target: false,
            generated: false,
            upgraded: false,
            mine_list: Vec::new(),
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getGenerateMinefieldBehaviorModuleData()`).
    pub fn module_data(&self) -> &Arc<GenerateMinefieldBehaviorModuleData> {
        &self.data
    }

    /// The state the test compares: the executed flag, the three flags, the target and the mine list.
    pub fn state_words(&self) -> Vec<u32> {
        let mut w = vec![
            u32::from(self.mux.is_already_upgraded()),
            u32::from(self.generated),
            u32::from(self.has_target),
            u32::from(self.upgraded),
            self.target.x.to_bits(),
            self.target.y.to_bits(),
            self.target.z.to_bits(),
            self.mine_list.len() as u32,
        ];
        w.extend(self.mine_list.iter().map(|&id| id as u32));
        w
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object (nothing).
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        if let Some(r) = upgrade_mux_call(self, world, me, call.clone()) {
            return r;
        }
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: the scheduling is not saved (`BehaviorModule::xfer`).
    pub fn xfer_with(&mut self, _update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.xfer(xfer)
    }

    /// `crc` with the scheduling half the slot holds: `BehaviorModule::crc` and the mux's.
    pub fn crc_with(&mut self, _update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.crc(xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, _update: &mut UpdateModule) {
        self.load_post_process();
    }

    /// `getMinefieldTarget()`: the target, or the object's position.
    fn minefield_target(&self, world: &World, id: ObjectId) -> Coord3D {
        if self.has_target {
            self.target
        } else {
            world.objects.find(id).map(|o| o.get_position()).unwrap_or_default()
        }
    }

    /// `obj->getControllingPlayer()->getDefaultTeam()`.
    fn team(world: &World, id: ObjectId) -> Result<TeamId, Undefined> {
        let player = world.get_controlling_player(id).ok_or(Undefined::NullControllingPlayer)?;
        Ok(world.cuts.player_default_team(player))
    }

    /// `offsetBySmallRandomAmount(pt, maxAmt)`.
    fn jitter(world: &mut World, pt: &mut Coord3D, max_amt: f32) {
        pt.x += world.random.game_logic_random_value_real(-max_amt, max_amt);
        if !crate::logic::modules::plant(2119) {
            pt.y += world.random.game_logic_random_value_real(-max_amt, max_amt);
        }
    }

    /// `placeMineAt(pt, mineTemplate, team, producer)`: the mine, or `None` where none is placed.
    fn place_mine_at(&mut self, world: &mut World, me: ModuleHandle, pt: &Coord3D, mine: &MineTemplate, team: TeamId) -> Option<ObjectId> {
        let id = me.object;
        let tmp = Coord3D { x: pt.x, y: pt.y, z: 99999.0 };
        let layer = world.terrain_get_highest_layer_for_destination(&tmp);
        if layer == LAYER_GROUND && world.terrain_is_underwater_water_z(pt.x, pt.y).is_some() {
            return None;
        }
        if layer == LAYER_GROUND && world.terrain_is_cliff_cell(pt.x, pt.y) && !crate::logic::modules::plant(2111) {
            return None;
        }
        let orient = world.random.game_logic_random_value_real(-PI, PI);
        // "if the mine will be "mostly" under a structure, don't place it. for now, "mostly" means "central third of radius would overlap""
        let mut geom = mine.geom.clone();
        let mine_radius = mine.geom.get_bounding_circle_radius();
        geom.expand_footprint(mine_radius * -(1.0 - self.data.skip_if_this_much_under_structure));
        for them in world.partition_iterate_potential_collisions_at(*pt, &geom, orient) {
            if world.objects.find(them).is_some_and(|o| o.is_kind_of(KINDOF_STRUCTURE)) && !crate::logic::modules::plant(2112) {
                return None;
            }
        }
        let new_mine = world.crate_new_object(mine.id, Some(team));
        if new_mine == INVALID_ID {
            return None;
        }
        world.set_position(new_mine, pt);
        world.crate_set_orientation(new_mine, orient);
        if let Some(o) = world.objects.find_mut(new_mine) {
            o.producer_id = id;
        }
        let producer_pos = world.objects.find(id).map(|o| o.get_position()).unwrap_or_default();
        let end = if crate::logic::modules::plant(2113) { producer_pos } else { *pt };
        for idx in world.all_modules(new_mine) {
            let r = world.module_call(new_mine, idx, ModuleCall::LandMineSetScootParms { start: producer_pos, end });
            if !matches!(r, ModuleReply::Unhandled) {
                break;
            }
        }
        // Keep track of the mines
        if self.data.upgradable {
            self.mine_list.push(new_mine);
        }
        Some(new_mine)
    }

    /// `placeMinesAlongLine(posStart, posEnd, mineTemplate, skipOneAtStart)`.
    fn place_mines_along_line(&mut self, world: &mut World, me: ModuleHandle, start: &Coord3D, end: &Coord3D, mine: &MineTemplate, skip_one_at_start: bool) -> Result<(), Undefined> {
        let team = Self::team(world, me.object)?;
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let len = (dx * dx + dy * dy).sqrt();
        let mine_radius = mine.geom.get_bounding_circle_radius();
        let mine_diameter = mine_radius * 2.0;
        let mine_jitter = mine_radius * self.data.random_jitter;
        let mut num_mines = real_to_int_ceil(len / mine_diameter);
        if num_mines < 1 {
            num_mines = 1;
        }
        let inc = len / num_mines as f32;
        let mut place = if skip_one_at_start { inc } else { 0.0 };
        if place <= len && inc == 0.0 {
            // the loop's increment is 0 and its condition holds: it never ends
            return Err(Undefined::EndlessLoop);
        }
        while place <= len {
            let mut pt = Coord3D { x: start.x + place * dx / len, y: start.y + place * dy / len, z: 0.0 };
            pt.z = world.terrain_get_ground_height(pt.x, pt.y);
            Self::jitter(world, &mut pt, mine_jitter);
            self.place_mine_at(world, me, &pt, mine, team);
            place += inc;
        }
        Ok(())
    }

    /// `placeMinesAroundRect(pos, majorRadius, minorRadius, mineTemplate)`: the corners are the object's transform of (+/-major, +/-minor).
    fn place_mines_around_rect(&mut self, world: &mut World, me: ModuleHandle, major: f32, minor: f32, mine: &MineTemplate) -> Result<(), Undefined> {
        let mtx: Matrix3D = world.objects.find(me.object).map(|o| *o.thing.transform_matrix()).unwrap_or_default();
        let corner = |a: f32, b: f32| {
            let v = Matrix3D::transform_vector(&mtx, &Vector3::new(a, b, 0.0));
            Coord3D { x: v.x, y: v.y, z: v.z }
        };
        let pt = [corner(major, minor), corner(-major, minor), corner(-major, -minor), corner(major, -minor)];
        let skip = !crate::logic::modules::plant(2114);
        self.place_mines_along_line(world, me, &pt[0], &pt[1], mine, skip)?;
        self.place_mines_along_line(world, me, &pt[1], &pt[2], mine, skip)?;
        self.place_mines_along_line(world, me, &pt[2], &pt[3], mine, skip)?;
        self.place_mines_along_line(world, me, &pt[3], &pt[0], mine, skip)
    }

    /// `placeMinesAroundCircle(pos, radius, mineTemplate)`.
    fn place_mines_around_circle(&mut self, world: &mut World, me: ModuleHandle, pos: &Coord3D, radius: f32, mine: &MineTemplate) -> Result<(), Undefined> {
        let team = Self::team(world, me.object)?;
        let circum = 2.0 * PI * radius;
        let mine_radius = mine.geom.get_bounding_circle_radius();
        let mine_diameter = mine_radius * 2.0;
        let mine_jitter = mine_radius * self.data.random_jitter;
        let mut num_mines = real_to_int_ceil(circum / mine_diameter);
        if num_mines < 1 {
            num_mines = 1;
        }
        let angle_inc = (2.0 * PI) / num_mines as f32;
        let angle_lim = (2.0 * PI) - angle_inc * 0.5;
        let mut angle = 0.0f32;
        while angle < angle_lim {
            let mut pt = Coord3D { x: pos.x + radius * trig::cos(angle), y: pos.y + radius * trig::sin(angle), z: 0.0 };
            pt.z = world.terrain_get_ground_height(pt.x, pt.y);
            Self::jitter(world, &mut pt, mine_jitter);
            self.place_mine_at(world, me, &pt, mine, team);
            angle += angle_inc;
        }
        Ok(())
    }

    /// `placeMinesInFootprint(geom, mineTemplate)`.
    fn place_mines_in_footprint(&mut self, world: &mut World, me: ModuleHandle, geom: &GeometryInfo, mine: &MineTemplate) -> Result<(), Undefined> {
        let id = me.object;
        let team = Self::team(world, id)?;
        let area = geom.get_footprint_area();
        let mut num_mines = real_to_int_ceil(self.data.mines_per_square_foot * area);
        if num_mines < 1 {
            num_mines = 1;
        }
        let target = self.minefield_target(world, id);
        let mut placed: Vec<ObjectId> = Vec::new();
        let d = mine.geom.get_bounding_circle_radius() * 2.0;
        let min_dist_sqr = d * d;
        let Some(own_geom) = world.objects.find(id).map(|o| o.geometry_info.clone()) else { return Ok(()) };
        for _ in 0..num_mines {
            let mut pt = Coord3D::default();
            let mut max_retry = 100;
            loop {
                let mut rng = std::mem::take(&mut world.random);
                geom.make_random_offset_within_footprint(&mut rng, &mut pt);
                world.random = rng;
                pt.x += target.x;
                pt.y += target.y;
                pt.z += target.z;
                max_retry -= 1;
                // isAnythingTooClose2D: the mines' positions now
                let too_close = placed.iter().any(|&m| {
                    let p = world.objects.find(m).map(|o| o.get_position()).unwrap_or_default();
                    let (dx, dy) = (p.x - pt.x, p.y - pt.y);
                    dx * dx + dy * dy < min_dist_sqr
                });
                if !(too_close && max_retry > 0 && !crate::logic::modules::plant(2115)) {
                    break;
                }
            }
            if own_geom.is_point_in_footprint(&target, &pt) {
                continue;
            }
            // can return null.
            if let Some(m) = self.place_mine_at(world, me, &pt, mine, team) {
                placed.push(m);
            }
        }
        Ok(())
    }

    /// `placeMines()`.
    fn place_mines(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        if self.generated {
            return Ok(());
        }
        self.generated = true;
        let id = me.object;
        let d = self.data.clone();
        let statics = world.statics.clone();
        let name = if self.upgraded { &d.mine_name_upgraded } else { &d.mine_name };
        let Some(t) = statics.thing_factory.find_template(name) else { return Ok(()) };
        let mine = MineTemplate { id: t.data.get_template_id(), geom: t.data.get_template_geometry_info().clone() };
        let target = self.minefield_target(world, id);
        let Some(obj_geom) = world.objects.find(id).map(|o| o.geometry_info.clone()) else { return Ok(()) };
        if d.smart_border {
            let mut geom = obj_geom;
            if !d.smart_border_skip_interior {
                geom = mine.geom.clone();
                let team = Self::team(world, id)?;
                self.place_mine_at(world, me, &target, &mine, team);
            }
            if d.always_circular {
                let r = geom.get_bounding_circle_radius();
                geom.set(GeometryType::Cylinder, false, 1.0, r, r);
            }
            let mine_radius = mine.geom.get_bounding_circle_radius();
            let mine_diameter = mine_radius * 2.0;
            geom.expand_footprint(mine_radius);
            loop {
                if geom.get_geom_type() == GeometryType::Box && !d.always_circular {
                    self.place_mines_around_rect(world, me, geom.get_major_radius(), geom.get_minor_radius(), &mine)?;
                } else {
                    self.place_mines_around_circle(world, me, &target, geom.get_major_radius(), &mine)?;
                }
                geom.expand_footprint(if crate::logic::modules::plant(2116) { mine_radius } else { mine_diameter });
                if geom.get_bounding_circle_radius() >= d.distance_around_object {
                    break;
                }
            }
        } else if d.border_only {
            let mut geom = obj_geom;
            geom.expand_footprint(d.distance_around_object);
            if geom.get_geom_type() == GeometryType::Box && !d.always_circular {
                self.place_mines_around_rect(world, me, geom.get_major_radius(), geom.get_minor_radius(), &mine)?;
            } else {
                self.place_mines_around_circle(world, me, &target, geom.get_major_radius(), &mine)?;
            }
        } else {
            let mut geom = obj_geom;
            geom.expand_footprint(d.distance_around_object);
            if d.always_circular {
                let r = geom.get_bounding_circle_radius();
                geom.set(GeometryType::Cylinder, false, 1.0, r, r);
            }
            self.place_mines_in_footprint(world, me, &geom, &mine)?;
        }
        if let Some(fx) = &d.gen_fx {
            world.emit(ObjectClientEvent::FxListDoFxObj { fx: fx.clone(), object_id: id });
        }
        Ok(())
    }

    /// `update()`: "test to see if we need to replace the current mines with upgraded ones".
    fn update_inner(&mut self, world: &mut World, me: ModuleHandle) -> Result<UpdateSleepTime, Undefined> {
        if !self.upgraded && self.data.upgradable {
            if self.generated {
                let statics = world.statics.clone();
                let upgrade = statics.upgrade_center.find_upgrade(&mut world.names, CHINA_EMP_MINES).ok().flatten();
                if let Some(u) = upgrade {
                    let mask = u.upgrade_mask();
                    let obj_mask = world.objects.find(me.object).map(|o| o.object_upgrades_completed).unwrap_or_default();
                    if obj_mask.test_for_any(&mask) {
                        self.upgraded = true;
                        // Remove all old mine objects if present
                        for mine in std::mem::take(&mut self.mine_list) {
                            if world.objects.find(mine).is_some() && !crate::logic::modules::plant(2117) {
                                world.body_destroy_object(mine);
                            }
                        }
                        // "Place new mines down (Replace old ones that we removed"
                        self.generated = false;
                        self.place_mines(world, me)?;
                    }
                }
            }
            return Ok(UPDATE_SLEEP_NONE);
        }
        Ok(UPDATE_SLEEP_FOREVER)
    }

    fn undefined(world: &mut World, me: ModuleHandle, r: Result<(), Undefined>) {
        if let Err(u) = r {
            world.cuts.module_errors.push((me, IniError::undefined(u)));
        }
    }

    fn mux_from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::GenerateMinefieldBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl UpgradeMuxUser for GenerateMinefieldBehavior {
    fn mux(&self) -> &UpgradeMux {
        &self.mux
    }
    fn mux_mut(&mut self) -> &mut UpgradeMux {
        &mut self.mux
    }
    fn mux_data(&self) -> &Arc<UpgradeModuleData> {
        &self.data.upgrade
    }
    /// `upgradeImplementation()`: `placeMines()`.
    fn upgrade_implementation(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        let r = self.place_mines(world, me);
        Self::undefined(world, me, r);
        Ok(())
    }
    fn mux_into_body(self) -> UpdateBody {
        UpdateBody::GenerateMinefieldBehavior(self)
    }
    fn mux_from_body(body: UpdateBody) -> Option<Self> {
        Self::mux_from_body(body)
    }
}

impl UpdateBehavior for GenerateMinefieldBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        match self.update_inner(world, me) {
            Ok(s) => s,
            Err(u) => {
                world.cuts.module_errors.push((me, IniError::undefined(u)));
                UPDATE_SLEEP_NONE
            }
        }
    }
    /// The object's calls: the die interface and `setMinefieldTarget`.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnDie(_) => {
                if self.data.on_death {
                    let r = self.place_mines(world, me);
                    Self::undefined(world, me, r);
                }
                ModuleReply::Done
            }
            ModuleCall::GenerateMinefieldSetTarget(pos) => {
                match pos {
                    Some(p) => {
                        self.has_target = true;
                        self.target = p;
                    }
                    None => {
                        self.has_target = false;
                        self.target = Coord3D::default();
                    }
                }
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::GenerateMinefieldBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        Self::mux_from_body(body)
    }
}

impl Snapshot for GenerateMinefieldBehavior {
    /// `crc`: `BehaviorModule::crc`, then `upgradeMuxCRC`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.behavior().clone().crc(xfer)?;
        self.mux.upgrade_mux_crc(xfer)
    }
    /// `xfer` (version 1): the version, `BehaviorModule::xfer`, `upgradeMuxXfer`, the flags, the target and the mine list.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.update.behavior().clone().xfer(xfer)?;
        self.mux.upgrade_mux_xfer(xfer)?;
        xfer.xfer_bool(&mut self.generated)?;
        xfer.xfer_bool(&mut self.has_target)?;
        if !crate::logic::modules::plant(2118) {
            xfer.xfer_bool(&mut self.upgraded)?;
        }
        xfer.xfer_coord3d(&mut self.target)?;
        // "spaces info count and objectID data"
        let mut count = self.mine_list.len() as u8;
        xfer.xfer_unsigned_byte(&mut count)?;
        if xfer.get_xfer_mode() == XferMode::Save {
            for id in self.mine_list.iter_mut() {
                xfer.xfer_object_id(id)?;
            }
        } else if xfer.get_xfer_mode() == XferMode::Load {
            self.mine_list.clear();
            for _ in 0..count {
                let mut id = INVALID_ID;
                xfer.xfer_object_id(&mut id)?;
                self.mine_list.push(id);
            }
        }
        Ok(())
    }
    /// `loadPostProcess`: `BehaviorModule::loadPostProcess`, then `upgradeMuxLoadPostProcess`.
    fn load_post_process(&mut self) {
        self.update.behavior().clone().load_post_process();
        self.mux.upgrade_mux_load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `GenerateMinefieldBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::GenerateMinefieldBehavior(Arc::new(GenerateMinefieldBehaviorModuleData::friend_new_module_data(ini)?)))
}
