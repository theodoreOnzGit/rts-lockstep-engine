// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/HordeUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/HordeUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `HordeUpdate`: the horde bonus of infantry and tanks. Every `UpdateRate` frames (every frame for vehicles, which also wait for the rate) the
//! object counts the horde members in `Radius` (from the bounding spheres, 3D): enough (`Count` with itself) and it is a true member; too few,
//! and it still is in the horde if a true member is within `RubOffRadius` (2D, from the centre). Either way its AI re-evaluates the morale
//! bonus. A member's drawable (alive) shows the horde decal while icons are drawn (the nationalism or fanaticism decal with those bonuses; a
//! vehicle's sized to 3.5 times its radius; none for an Overlord's portable structure), and fades it in or out when the membership changes.
//!
//! A horde member is ([`HordeUpdate::is_horde_member`], upstream's `PartitionFilterHordeMember`): of the same template if `ExactMatch`, with a
//! horde interface, of all the `KindOf` kinds, an ally if `AlliesOnly`, and as off the map as the object. The scan is
//! [`World::partition_iterate_objects_in_range_of`], which returns the candidates; the filter is the module's own, as upstream's is.
//!
//! The module data is `ModuleData`-derived upstream; the `UpdateModuleData` base here has no fields and parses none, so it is the same.
//! `joinOrLeaveHorde` and `showHideFlag` are never called by the module itself (the flag sub-objects are hidden when a drawable is bound);
//! `m_trueHordeMember` is not saved. The decal reads the drawable's own type ([`Object::terrain_decal_type`](crate::logic::world::object::Object)).

use std::sync::Arc;

use crate::common::game_common::{Relationship, LOGICFRAMES_PER_SECOND};
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData,
};
use crate::common::kind_of::{KindOfMaskType, KINDOF_INFANTRY, KINDOF_PORTABLE_STRUCTURE};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime,
    UPDATE_SLEEP_NONE,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::weapon_bonus::condition;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `TheHordeActionTypeNames`.
pub const HORDE_ACTION_TYPE_NAMES: [&str; 1] = ["HORDE"];
/// `TERRAIN_DECAL_HORDE` (`Drawable.h`).
const TERRAIN_DECAL_HORDE: i32 = 1;
/// `TERRAIN_DECAL_HORDE_WITH_NATIONALISM_UPGRADE`.
const TERRAIN_DECAL_HORDE_WITH_NATIONALISM_UPGRADE: i32 = 2;
/// `TERRAIN_DECAL_HORDE_VEHICLE`.
const TERRAIN_DECAL_HORDE_VEHICLE: i32 = 3;
/// `TERRAIN_DECAL_HORDE_WITH_FANATICISM_UPGRADE`.
const TERRAIN_DECAL_HORDE_WITH_FANATICISM_UPGRADE: i32 = 6;
/// `TERRAIN_DECAL_NONE`.
const TERRAIN_DECAL_NONE: i32 = 8;
/// `FROM_BOUNDINGSPHERE_3D` (`PartitionManager.h`'s `DistanceCalculationType`).
const FROM_BOUNDINGSPHERE_3D: i32 = 3;

/// `class HordeUpdateModuleData : public ModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct HordeUpdateModuleData {
    /// The base (`UpdateModuleData`, which adds nothing to `ModuleData`).
    pub update: UpdateModuleData,
    /// `m_updateRate`.
    pub update_rate: u32,
    /// `m_kindof`.
    pub kindof: KindOfMaskType,
    /// `m_minCount`.
    pub min_count: i32,
    /// `m_minDist`.
    pub min_dist: f32,
    /// `m_rubOffRadius`.
    pub rub_off_radius: f32,
    /// `m_alliesOnly`.
    pub allies_only: bool,
    /// `m_exactMatch`.
    pub exact_match: bool,
    /// `m_action` (an index into [`HORDE_ACTION_TYPE_NAMES`]).
    pub action: i32,
    /// `m_flagSubObjNames`.
    pub flag_sub_obj_names: Vec<Vec<u8>>,
    /// `m_allowedNationalism`.
    pub allowed_nationalism: bool,
}

impl Default for HordeUpdateModuleData {
    /// `HordeUpdateModuleData()`: a rate of a second, no count or radius, a rub-off radius of 20, allies only, no exact match, nationalism allowed.
    fn default() -> Self {
        HordeUpdateModuleData {
            update: UpdateModuleData::default(),
            update_rate: LOGICFRAMES_PER_SECOND as u32,
            kindof: KindOfMaskType::new(),
            min_count: 0,
            min_dist: 0.0,
            rub_off_radius: 20.0,
            allies_only: true,
            exact_match: false,
            action: 0,
            flag_sub_obj_names: Vec::new(),
            allowed_nationalism: true,
        }
    }
}

/// `HordeUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<HordeUpdateModuleData>; 11] = [
    FieldParse::new(
        "UpdateRate",
        |ini, d, _| {
            d.update_rate = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "KindOf",
        |ini, d, _| d.kindof.parse_from_ini(ini),
        UserData::None,
    ),
    FieldParse::new(
        "Count",
        |ini, d, _| {
            d.min_count = ini.parse_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "Radius",
        |ini, d, _| {
            d.min_dist = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "RubOffRadius",
        |ini, d, _| {
            d.rub_off_radius = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "AlliesOnly",
        |ini, d, _| {
            d.allies_only = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "ExactMatch",
        |ini, d, _| {
            d.exact_match = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "Action",
        |ini, d, _| {
            d.action = ini.parse_index_list(&HORDE_ACTION_TYPE_NAMES)?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "FlagSubObjectNames",
        |ini, d, _| ini.parse_ascii_string_vector(&mut d.flag_sub_obj_names),
        UserData::None,
    ),
    FieldParse::new(
        "AllowedNationalism",
        |ini, d, _| {
            d.allowed_nationalism = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl HordeUpdateModuleData {
    /// `buildFieldParse(p)`: `ModuleData`'s (no fields of its own besides the behaviour base's), then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, HordeUpdateModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<HordeUpdateModuleData> {
        let mut data = HordeUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, HordeUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class HordeUpdate : public UpdateModule, public HordeUpdateInterface`.
#[derive(Clone, Debug)]
pub struct HordeUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getHordeUpdateModuleData()`.
    data: Arc<HordeUpdateModuleData>,
    /// `m_lastHordeRefreshFrame`.
    last_horde_refresh_frame: u32,
    /// `m_inHorde`.
    in_horde: bool,
    /// `m_trueHordeMember` (not saved).
    true_horde_member: bool,
    /// `m_hasFlag` (never set but by a load).
    has_flag: bool,
}

impl PartialEq for HordeUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.state_words() == other.state_words()
    }
}

/// `getHUI(obj)`'s answer to `isTrueHordeMember()`: the first module of `obj` with a horde interface (`None`: none has one).
pub fn horde_is_true_member(world: &mut World, obj: ObjectId) -> Option<bool> {
    for idx in world.all_modules(obj) {
        match world.module_call(obj, idx, ModuleCall::HordeIsTrueMember) {
            ModuleReply::Bool(b) => return Some(b),
            _ => continue,
        }
    }
    None
}

impl HordeUpdate {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and the flags
    /// false (the refresh frame is the world's, set by the constructor's second half).
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<HordeUpdateModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(HordeUpdate {
            update,
            data,
            last_horde_refresh_frame: 0,
            in_horde: false,
            true_horde_member: false,
            has_flag: false,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getHordeUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<HordeUpdateModuleData> {
        &self.data
    }

    /// The state the test compares: the last refresh frame, the three flags.
    pub fn state_words(&self) -> [u32; 4] {
        [
            self.last_horde_refresh_frame,
            u32::from(self.in_horde),
            u32::from(self.true_horde_member),
            u32::from(self.has_flag),
        ]
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object: the refresh frame, a random first wake.
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

    /// `xfer` with the scheduling half the slot holds: the version, that half, the field.
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

    /// `PartitionFilterHordeMember::allow(objOther)`.
    pub fn is_horde_member(&self, world: &mut World, me: ObjectId, other: ObjectId) -> bool {
        let d = &self.data;
        let (Some(mine), Some(theirs)) = (
            world.objects.find(me).map(|o| o.template().clone()),
            world.objects.find(other).map(|o| o.template().clone()),
        ) else {
            return false;
        };
        // must be exact same type as us (well, maybe)
        if d.exact_match && !Arc::ptr_eq(&mine, &theirs) && !crate::logic::modules::plant(1951) {
            return false;
        }
        // can only horde with other horde-able things
        if horde_is_true_member(world, other).is_none() {
            return false;
        }
        // must match the kindof flags (if any)
        if !theirs.is_kind_of_multi(&d.kindof, &KindOfMaskType::new()) {
            return false;
        }
        // must be our ally (well, maybe)
        if d.allies_only && world.get_relationship(me, other) != Relationship::Allies {
            return false;
        }
        // doh
        let off_map = |id: ObjectId| world.objects.find(id).is_some_and(|o| o.is_off_map());
        if off_map(me) != off_map(other) {
            return false;
        }
        true
    }

    /// `isTrueHordeMember()`: a true member that is in the horde.
    pub fn is_true_horde_member(&self) -> bool {
        self.true_horde_member && self.in_horde
    }

    /// `onDrawableBoundToObject()`: `showHideFlag(FALSE)`, the flag sub-objects hidden.
    fn show_hide_flag(&self, world: &mut World, me: ObjectId, show: bool) {
        if self.data.flag_sub_obj_names.is_empty() || !world.has_drawable(me) {
            return;
        }
        for name in &self.data.flag_sub_obj_names {
            world.emit(ObjectClientEvent::DrawableShowSubObject {
                object_id: me,
                name: name.clone(),
                show,
            });
        }
        world.emit(ObjectClientEvent::DrawableUpdateSubObjects { object_id: me });
    }

    /// The horde decal of a member: by the nationalism and fanaticism bonuses, else the infantry or the vehicle one.
    fn decal_type(world: &World, me: ObjectId, infantry_or_vehicle: i32) -> i32 {
        let bonus = world
            .objects
            .find(me)
            .map_or(0, |o| o.weapon_bonus_condition);
        let test = |c: i32| bonus & (1 << c) != 0;
        if test(condition::NATIONALISM) {
            if test(condition::FANATICISM) {
                TERRAIN_DECAL_HORDE_WITH_FANATICISM_UPGRADE
            } else {
                TERRAIN_DECAL_HORDE_WITH_NATIONALISM_UPGRADE
            }
        } else {
            infantry_or_vehicle
        }
    }
}

impl UpdateBehavior for HordeUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let id = me.object;
        let d = self.data.clone();
        let was_in_horde = self.in_horde;
        let Some(template) = world.objects.find(id).map(|o| o.template().clone()) else {
            return UPDATE_SLEEP_NONE;
        };
        let is_infantry = template.is_kind_of(KINDOF_INFANTRY);
        let now = world.frame();
        if is_infantry
            || now > self.last_horde_refresh_frame.wrapping_add(d.update_rate)
            || crate::logic::modules::plant(1952)
        {
            self.last_horde_refresh_frame = now;
            let candidates =
                world.partition_iterate_objects_in_range_of(id, d.min_dist, FROM_BOUNDINGSPHERE_3D);
            let members: Vec<ObjectId> = candidates
                .into_iter()
                .filter(|&o| self.is_horde_member(world, id, o))
                .collect();
            // "we really are in the thick part of the horde" (`getCount() >= m_minCount - 1`: an Int comparison)
            if members.len() as i64 >= i64::from(d.min_count) - 1 {
                self.in_horde = true;
                self.true_horde_member = true;
            } else {
                self.in_horde = false;
                self.true_horde_member = false; // unless...
                let rub_off_radius_sq = d.rub_off_radius * d.rub_off_radius;
                for other in members {
                    // `getHUI(other)` on the object itself is this module, lent out of its slot here: its flags are the ones just cleared
                    let true_member = if other == id {
                        Some(self.is_true_horde_member())
                    } else {
                        horde_is_true_member(world, other)
                    };
                    if true_member == Some(true) {
                        let dist = world.partition_get_distance_squared_center_2d(id, other);
                        if dist <= rub_off_radius_sq && !crate::logic::modules::plant(1953) {
                            self.in_horde = true;
                            break;
                        }
                    }
                }
            }
            if let Some(ai) = world.objects.find(id).and_then(|o| o.ai) {
                world.module_call(id, ai, ModuleCall::AiEvaluateMoraleBonus);
            }
        }
        let alive = world
            .objects
            .find(id)
            .is_some_and(|o| !o.is_effectively_dead());
        if world.has_drawable(id) && alive {
            if world.cuts.globals.draw_icon_ui {
                if self.in_horde && !template.is_kind_of(KINDOF_PORTABLE_STRUCTURE) {
                    let current = world
                        .objects
                        .find(id)
                        .map_or(TERRAIN_DECAL_NONE, |o| o.terrain_decal_type);
                    if is_infantry {
                        let nu_type = Self::decal_type(world, id, TERRAIN_DECAL_HORDE);
                        if nu_type != current {
                            world.drawable_set_terrain_decal(id, nu_type);
                        }
                    } else {
                        let size = 3.5
                            * world
                                .objects
                                .find(id)
                                .map_or(0.0, |o| o.geometry_info.get_major_radius());
                        world.emit(ObjectClientEvent::DrawableSetTerrainDecalSize {
                            object_id: id,
                            x: size,
                            y: size,
                        });
                        let nu_type = Self::decal_type(world, id, TERRAIN_DECAL_HORDE_VEHICLE);
                        if nu_type != current || crate::logic::modules::plant(1954) {
                            world.drawable_set_terrain_decal(id, nu_type);
                        }
                    }
                }
            } else {
                world.drawable_set_terrain_decal(id, TERRAIN_DECAL_NONE);
            }
            if !was_in_horde && self.in_horde {
                world.emit(ObjectClientEvent::DrawableTerrainDecalFadeTarget {
                    object_id: id,
                    target: 1.0,
                    rate: 0.03,
                });
            } else if was_in_horde && !self.in_horde {
                world.emit(ObjectClientEvent::DrawableTerrainDecalFadeTarget {
                    object_id: id,
                    target: 0.0,
                    rate: -0.03,
                });
            }
        }
        if is_infantry {
            update_sleep(d.update_rate)
        } else {
            UPDATE_SLEEP_NONE
        }
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        self.last_horde_refresh_frame = world.frame();
        let delay = self.data.update_rate;
        let wake = world.random.game_logic_random_value(1, delay as i32);
        update_behavior::set_wake_frame(world, update_sleep(wake as u32));
    }
    /// The fields after the base: `m_inHorde`, `m_hasFlag`.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_bool(&mut self.in_horde)?;
        if crate::logic::modules::plant(1955) {
            return Ok(());
        }
        xfer.xfer_bool(&mut self.has_flag)
    }
    /// The horde interface and `onDrawableBoundToObject`.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::HordeIsTrueMember => ModuleReply::Bool(self.is_true_horde_member()),
            ModuleCall::HordeIsInHorde => ModuleReply::Bool(self.in_horde),
            ModuleCall::HordeHasFlag => ModuleReply::Bool(self.has_flag),
            ModuleCall::HordeIsAllowedNationalism => {
                ModuleReply::Bool(self.data.allowed_nationalism)
            }
            ModuleCall::OnDrawableBoundToObject => {
                self.show_hide_flag(world, me.object, false);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::HordeUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::HordeUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for HordeUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base, then the field.
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

/// The factory's `NewModuleDataProc` for `HordeUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::HordeUpdate(Arc::new(
        HordeUpdateModuleData::friend_new_module_data(ini)?,
    )))
}
