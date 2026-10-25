// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SpectreGunshipDeploymentUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SpectreGunshipDeploymentUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SpectreGunshipDeploymentUpdate`: the command centre's Spectre gunship power. `initiateIntentToDoSpecialPower` for its own special power
//! makes a gunship (`GunshipTemplateName`, on the building's team, produced by it) at a map edge (`CreateLocation`: the edge nearest or
//! farthest from the building or the target), moved out along the line to the target by the orbit radius and up to its locomotor's preferred
//! height, facing the target; tells the gunship's own module for the power to fire at the target, selects it for the building's player, and
//! marks the building's power triggered. A script-fired power is also made ready at once (`setReadyFrame(now)`).
//!
//! # Upstream facts kept
//!
//! - The data class derives from `ModuleData`, not `UpdateModuleData` (the update base reads nothing from either).
//! - `m_gunshipOrbitRadius` is in the data class but no field parses it: the allocator's zero. `AttackAreaRadius` is read and unused here.
//! - A gunship already made is forgotten, not destroyed (`m_gunshipID = INVALID_ID`), and a new one made every time.
//! - The target position is not saved (`xfer` is the version, the update base and the gunship's id); it is not reset by a load.
//! - Unchecked upstream, so undefined here: the special power module never found (`Undefined::NullSpecialPowerModule`), a `NULL` target
//!   position (`Undefined::NullLocation`), a gunship with no AI or no current locomotor (`Undefined::NullAi`, `Undefined::NullLocomotor`),
//!   and the building with no controlling player when the gunship is selected (`Undefined::NullControllingPlayer`). An unknown gunship
//!   template is `ThingFactory::newObject(NULL)`'s `throw ERROR_BAD_ARG`.
//!
//! `p1w13-sonnet`, batch R60, 2026-10-25.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_type::{INVALID_ANGLE, INVALID_ID};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::{OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::special_power_update_module::SpecialPowerUpdateModule;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object::ModuleIndex;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `COMMAND_FIRED_BY_SCRIPT` (`CommandOption`, `ControlBar.h`).
pub const COMMAND_FIRED_BY_SCRIPT: u32 = 0x0004_0000;

/// `GunshipCreateLocType`.
pub const CREATE_GUNSHIP_AT_EDGE_NEAR_SOURCE: i32 = 0;
pub const CREATE_GUNSHIP_AT_EDGE_FARTHEST_FROM_SOURCE: i32 = 1;
pub const CREATE_GUNSHIP_AT_EDGE_NEAR_TARGET: i32 = 2;
pub const CREATE_GUNSHIP_AT_EDGE_FARTHEST_FROM_TARGET: i32 = 3;

/// `TheGunshipCreateLocTypeNames`.
pub static GUNSHIP_CREATE_LOC_TYPE_NAMES: [&str; 4] =
    ["CREATE_AT_EDGE_NEAR_SOURCE", "CREATE_AT_EDGE_FARTHEST_FROM_SOURCE", "CREATE_AT_EDGE_NEAR_TARGET", "CREATE_AT_EDGE_FARTHEST_FROM_TARGET"];

/// `class SpectreGunshipDeploymentUpdateModuleData : public ModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct SpectreGunshipDeploymentUpdateModuleData {
    pub update: UpdateModuleData,
    /// `m_specialPowerTemplate`, by name (found in the store when used; an unknown name, and `None`, is `NULL`).
    pub special_power_template: Option<Vec<u8>>,
    /// `m_extraRequiredScience`, by name (`INI::parseScience` needs the store a module's data proc does not have: looked up when used;
    /// `None` is `SCIENCE_INVALID`).
    pub extra_required_science: Option<Vec<u8>>,
    /// `m_gunshipTemplateName`.
    pub gunship_template_name: Vec<u8>,
    /// `m_attackAreaRadius`.
    pub attack_area_radius: f32,
    /// `m_gunshipOrbitRadius` (never parsed: 0).
    pub gunship_orbit_radius: f32,
    /// `m_createLoc` (`GunshipCreateLocType`).
    pub create_loc: i32,
}

impl Default for SpectreGunshipDeploymentUpdateModuleData {
    /// `SpectreGunshipDeploymentUpdateModuleData()`, on the allocator's zeroes.
    fn default() -> Self {
        SpectreGunshipDeploymentUpdateModuleData {
            update: UpdateModuleData::new(),
            special_power_template: None,
            extra_required_science: None,
            gunship_template_name: Vec::new(),
            attack_area_radius: 200.0,
            gunship_orbit_radius: 0.0,
            create_loc: CREATE_GUNSHIP_AT_EDGE_FARTHEST_FROM_TARGET,
        }
    }
}

static FIELD_PARSE: [FieldParse<SpectreGunshipDeploymentUpdateModuleData>; 6] = [
    FieldParse::new("GunshipTemplateName", |i, d, _| { d.gunship_template_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("RequiredScience", |i, d, _| { d.extra_required_science = Some(i.get_next_token(None)?); Ok(()) }, UserData::None),
    FieldParse::new("SpecialPowerTemplate", |i, d, _| { d.special_power_template = Some(i.get_next_token(None)?); Ok(()) }, UserData::None),
    FieldParse::new("AttackAreaRadius", |i, d, _| { d.attack_area_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("CreateLocation", |i, d, _| { d.create_loc = i.parse_index_list(&GUNSHIP_CREATE_LOC_TYPE_NAMES)?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl SpectreGunshipDeploymentUpdateModuleData {
    /// `buildFieldParse(p)`: `ModuleData`'s (nothing), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, SpectreGunshipDeploymentUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<SpectreGunshipDeploymentUpdateModuleData> {
        let mut data = SpectreGunshipDeploymentUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, SpectreGunshipDeploymentUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `SpectreGunshipDeploymentUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::SpectreGunshipDeploymentUpdate(Arc::new(SpectreGunshipDeploymentUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `class SpectreGunshipDeploymentUpdate : public SpecialPowerUpdateModule`.
#[derive(Clone, Debug)]
pub struct SpectreGunshipDeploymentUpdate {
    base: SpecialPowerUpdateModule,
    data: Arc<SpectreGunshipDeploymentUpdateModuleData>,
    /// `m_specialPowerModule` (`NULL` until `onObjectCreated` finds it).
    special_power_module: Option<ModuleIndex>,
    /// `m_initialTargetPosition` (the allocator's zero until a power is initiated).
    initial_target_position: Coord3D,
    /// `m_gunshipID`.
    gunship_id: ObjectId,
}

impl PartialEq for SpectreGunshipDeploymentUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.base.update().behavior().object() == other.base.update().behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.special_power_module == other.special_power_module
            && self.state_words() == other.state_words()
    }
}

/// `atan2` of two `Real`s: the C `double` function, stored in a `Real`.
fn atan2_d(y: f32, x: f32) -> f32 {
    petir::Real::atan2(f64::from(y), f64::from(x)) as f32
}

impl SpectreGunshipDeploymentUpdate {
    pub const fn get_interface_mask() -> i32 {
        SpecialPowerUpdateModule::get_interface_mask()
    }

    /// The constructor (it touches no world): the base, no special power module, no gunship (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<SpectreGunshipDeploymentUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = SpecialPowerUpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        Ok(SpectreGunshipDeploymentUpdate { base, data, special_power_module: None, initial_target_position: Coord3D::default(), gunship_id: INVALID_ID })
    }

    pub fn update_module(&self) -> &UpdateModule {
        self.base.update()
    }

    pub fn module_data(&self) -> &Arc<SpectreGunshipDeploymentUpdateModuleData> {
        &self.data
    }

    /// The gunship's id and the target position (bits).
    pub fn state_words(&self) -> Vec<u32> {
        let p = self.initial_target_position;
        vec![self.gunship_id as u32, p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
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

    /// The special power template's id in the store (`None`: `NULL`).
    fn template_id(world: &World, name: &Option<Vec<u8>>) -> Option<u32> {
        name.as_deref().and_then(|n| world.statics.special_power_store.find_special_power_template(n)).map(|t| t.id())
    }

    /// `onObjectCreated()`: the object's module for the data's special power (none named: left `NULL`).
    fn on_object_created(&mut self, world: &mut World, us: ObjectId) {
        let Some(template) = Self::template_id(world, &self.data.special_power_template) else { return };
        self.special_power_module = world.get_special_power_module(us, Some(template));
    }

    /// `doesSpecialPowerUpdatePassScienceTest()` with this class's `getExtraRequiredScience()`.
    fn passes_science_test(&self, world: &mut World, us: ObjectId) -> Result<bool, Undefined> {
        let Some(science) = self.data.extra_required_science.clone() else { return Ok(true) };
        // (the name resolved as `INI::parseScience` would have; the generator names only known sciences)
        let statics = world.statics.clone();
        let st = statics.science_store.get_science_from_internal_name(&mut world.names, &science).map_err(|_| Undefined::UnknownScience)?;
        let player = world.get_controlling_player(us).ok_or(Undefined::NullControllingPlayer)?;
        let has = world.players.get_nth(player).is_some_and(|p| p.sciences.contains(&st));
        Ok(self.base.does_special_power_update_pass_science_test(st, |_| has))
    }

    /// `initiateIntentToDoSpecialPower(specialPowerTemplate, targetObj, targetPos, way, commandOptions)`.
    fn initiate_intent(&mut self, world: &mut World, us: ObjectId, template: u32, target_pos: Option<Coord3D>, command_options: u32) -> Result<bool, IniError> {
        let d = self.data.clone();
        let sp = self.special_power_module.ok_or_else(|| IniError::undefined(Undefined::NullSpecialPowerModule))?;
        let ours = match world.module_call(us, sp, ModuleCall::GetSpecialPowerTemplateId) {
            ModuleReply::Int(id) => id as u32,
            _ => 0,
        };
        if ours != template {
            return Ok(false);
        }
        let target = target_pos.ok_or_else(|| IniError::undefined(Undefined::NullLocation))?;
        if command_options & COMMAND_FIRED_BY_SCRIPT != 0 && !crate::logic::modules::plant(2297) {
            let now = world.frame();
            world.module_call(us, sp, ModuleCall::SetReadyFrame { frame: now });
        }
        self.initial_target_position = target;
        if world.objects.find(self.gunship_id).is_some() {
            self.gunship_id = INVALID_ID;
        }
        let gunship_template = world.statics.thing_factory.find_template(&d.gunship_template_name).map(|t| t.data.get_template_id());
        let gunship_template = gunship_template.ok_or_else(|| IniError::new(IniErrorKind::BadArg))?;
        let team = world.get_team(us);
        // (`newObject` throws on failure; the world's errors are not upstream's)
        let gunship = world.thing_factory_new_object(gunship_template, team, Default::default()).map_err(|_| IniError::new(IniErrorKind::BadArg))?;
        world.set_producer(gunship, us);
        let me_pos = world.objects.find(us).map(|o| o.get_position()).unwrap_or_default();
        let mut creation = match d.create_loc {
            CREATE_GUNSHIP_AT_EDGE_NEAR_SOURCE if crate::logic::modules::plant(2298) => world.terrain_find_farthest_edge_point(me_pos),
            CREATE_GUNSHIP_AT_EDGE_NEAR_SOURCE => world.terrain_find_closest_edge_point(me_pos),
            CREATE_GUNSHIP_AT_EDGE_FARTHEST_FROM_SOURCE => world.terrain_find_farthest_edge_point(me_pos),
            CREATE_GUNSHIP_AT_EDGE_NEAR_TARGET => world.terrain_find_closest_edge_point(target),
            _ => world.terrain_find_farthest_edge_point(target),
        };
        let initial = self.initial_target_position;
        let mut delta = initial;
        delta.sub(&creation);
        let distance_from_target = delta.length();
        if !crate::logic::modules::plant(2296) {
            delta.normalize();
        }
        delta.x *= distance_from_target + d.gunship_orbit_radius;
        delta.y *= distance_from_target + d.gunship_orbit_radius;
        creation.x = initial.x - delta.x;
        creation.y = initial.y - delta.y;
        creation.z = world.ai_cur_locomotor_preferred_height(gunship).map_err(IniError::undefined)?;
        world.set_position(gunship, &creation);
        let orient = atan2_d(initial.y - creation.y, initial.x - creation.x);
        world.set_orientation(gunship, orient);
        self.gunship_id = gunship;
        if let Some(ship_sp) = world.get_special_power_module(gunship, Some(template)) {
            world.module_call(gunship, ship_sp, ModuleCall::MarkSpecialPowerTriggered { location: Some(initial) });
            if !crate::logic::modules::plant(2300) {
                world.module_call(gunship, ship_sp, ModuleCall::DoSpecialPowerAtLocation { loc: initial, angle: INVALID_ANGLE, command_options });
            }
        }
        let player = world.get_controlling_player(us).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        let mask = world.cuts.player_mask(player);
        world.game_logic_select_object(gunship, !crate::logic::modules::plant(2299), mask, true);
        if let Some(own) = world.get_special_power_module(us, Some(template)) {
            world.module_call(us, own, ModuleCall::MarkSpecialPowerTriggered { location: Some(initial) });
        }
        Ok(true)
    }
}

impl UpdateBehavior for SpectreGunshipDeploymentUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        self.base.update()
    }
    /// `update()`: asleep for ever once sold, under construction or effectively dead; else awake.
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let Some(o) = world.objects.find(me.object) else { return UPDATE_SLEEP_NONE };
        if o.status.test(OBJECT_STATUS_SOLD) || o.status.test(OBJECT_STATUS_UNDER_CONSTRUCTION) || (o.is_effectively_dead() && !crate::logic::modules::plant(2302)) {
            return UPDATE_SLEEP_FOREVER;
        }
        UPDATE_SLEEP_NONE
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        if crate::logic::modules::plant(2301) {
            return Ok(());
        }
        xfer.xfer_object_id(&mut self.gunship_id)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::SpectreGunshipDeploymentUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SpectreGunshipDeploymentUpdate(m) => Some(m),
            _ => None,
        }
    }
    /// `onObjectCreated`, `initiateIntentToDoSpecialPower`, the science test and the overridable destination (none).
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnObjectCreated => {
                self.on_object_created(world, me.object);
                ModuleReply::Done
            }
            ModuleCall::InitiateIntentToDoSpecialPower { template, target_position, command_options, .. } => {
                match self.initiate_intent(world, me.object, template, target_position, command_options) {
                    Ok(b) => ModuleReply::Bool(b),
                    Err(e) => {
                        world.cuts.module_errors.push((me, e));
                        ModuleReply::Bool(false)
                    }
                }
            }
            ModuleCall::DoesSpecialPowerUpdatePassScienceTest => match self.passes_science_test(world, me.object) {
                Ok(b) => ModuleReply::Bool(b),
                Err(e) => {
                    world.cuts.module_errors.push((me, IniError::undefined(e)));
                    ModuleReply::Bool(false)
                }
            },
            ModuleCall::DoesSpecialPowerHaveOverridableDestinationActive | ModuleCall::DoesSpecialPowerHaveOverridableDestination => ModuleReply::Bool(false),
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for SpectreGunshipDeploymentUpdate {
    /// `crc`: `UpdateModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.base.update_mut().crc(xfer)
    }
    /// `xfer` (version 1): the version, `UpdateModule::xfer`, the gunship's id.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = SpectreGunshipDeploymentUpdateModuleData::default();
        assert_eq!((d.attack_area_radius, d.gunship_orbit_radius, d.create_loc), (200.0, 0.0, CREATE_GUNSHIP_AT_EDGE_FARTHEST_FROM_TARGET));
        let m = SpectreGunshipDeploymentUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words(), vec![INVALID_ID as u32, 0, 0, 0]);
        assert!(m.special_power_module.is_none());
    }
}
