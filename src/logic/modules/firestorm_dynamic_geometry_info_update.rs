// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/FirestormDynamicGeometryInfoUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FirestormDynamicGeometryInfoUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `FirestormDynamicGeometryInfoUpdate`: a [`DynamicGeometryInfoUpdate`] that is a firestorm. Once its starting delay is over it starts its
//! particle systems (up to sixteen) on the ground under it and plays its FX list, and the academy counts a firestorm for its player; every
//! frame its living particle systems take its radius; when it starts going back it leaves a scorch mark; and every so many frames it burns what
//! is in its bounding circle and not too far above it.
//!
//! # Where the world comes in
//!
//! - The base's growth is [`DynamicGeometryInfoUpdate`]'s own update, run first (its sleep time unused).
//! - The ground is `TheTerrainLogic->getGroundHeight` ([`World::terrain_get_ground_height`]); the particle systems are
//!   [`World::create_particle_system`]; whether the manager still has one is [`World::body_particle_exists`] (the client's), and its emission
//!   radius is [`ObjectClientEvent::ParticleSystemSetEmissionRadius`] (its volume type is the client's); the scorch is
//!   [`ObjectClientEvent::GameClientAddScorch`]; `FXList::doFXObj` is [`ObjectClientEvent::FxListDoFxObj`].
//! - The damage scan is `ThePartitionManager->iterateObjectsInRange(pos, radius, FROM_BOUNDINGSPHERE_2D, NULL)`
//!   ([`World::partition_iterate_objects_in_range_at`]); the damage is [`World::attempt_damage`].
//!
//! # Upstream facts kept
//!
//! - The scan has no filter: the firestorm is in its own range and burns itself.
//! - The damage interval is a `Real` compared with the `UnsignedInt` frames since the last scan (converted to `Real`).
//! - The save nests the base's (its version, its `UpdateModule`, its fields) inside this class's version; the ids are `xferUser` of the array.
//!
//! # Where upstream is undefined
//!
//! The academy's count of a firestorm with no controlling player (`getControllingPlayer()->getAcademyStats()`,
//! [`Undefined::NullControllingPlayer`]). The case is recorded in `world.cuts.module_errors` and the call returns.
//!
//! `p1w13-sonnet`, batch R32, 2026-10-24.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData,
};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_FLAME, DEATH_BURNED};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{UpdateModule, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::dynamic_geometry_info_update::{
    self as dgiu, DynamicGeometryInfoUpdate, DynamicGeometryInfoUpdateModuleData,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `MAX_FIRESTORM_SYSTEMS`.
pub const MAX_FIRESTORM_SYSTEMS: usize = 16;
/// `INVALID_PARTICLE_SYSTEM_ID`.
const INVALID_PARTICLE_SYSTEM_ID: u32 = 0;
/// `FROM_BOUNDINGSPHERE_2D` (`DistanceCalculationType`).
const FROM_BOUNDINGSPHERE_2D: i32 = 2;
/// `SCORCH_1`.
const SCORCH_1: i32 = 0;

/// `class FirestormDynamicGeometryInfoUpdateModuleData : public DynamicGeometryInfoUpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct FirestormDynamicGeometryInfoUpdateModuleData {
    /// The `DynamicGeometryInfoUpdateModuleData` base.
    pub geom: DynamicGeometryInfoUpdateModuleData,
    /// `m_particleSystem[i]`, by name (`None` for `NULL`).
    pub particle_systems: [Option<Vec<u8>>; MAX_FIRESTORM_SYSTEMS],
    /// `m_fxList`, by name (`None` for `NULL`).
    pub fx_list: Option<Vec<u8>>,
    /// `m_particleOffsetZ`.
    pub particle_offset_z: f32,
    /// `m_scorchSize`.
    pub scorch_size: f32,
    /// `m_delayBetweenDamageFrames`.
    pub delay_between_damage_frames: f32,
    /// `m_damageAmount`.
    pub damage_amount: f32,
    /// `m_maxHeightForDamage`.
    pub max_height_for_damage: f32,
}

impl Default for FirestormDynamicGeometryInfoUpdateModuleData {
    /// `FirestormDynamicGeometryInfoUpdateModuleData()`.
    fn default() -> Self {
        FirestormDynamicGeometryInfoUpdateModuleData {
            geom: DynamicGeometryInfoUpdateModuleData::default(),
            particle_systems: Default::default(),
            fx_list: None,
            particle_offset_z: 0.0,
            scorch_size: 0.0,
            delay_between_damage_frames: 0.0,
            damage_amount: 0.0,
            max_height_for_damage: 20.0,
        }
    }
}

/// Lifts entry `k` of the base's table onto `FirestormDynamicGeometryInfoUpdateModuleData::geom`.
fn lift(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut FirestormDynamicGeometryInfoUpdateModuleData,
    u: &UserData,
) -> IniResult<()> {
    match dgiu::FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.geom, u),
        None => Ok(()),
    }
}

/// `DynamicGeometryInfoUpdateModuleData::buildFieldParse`'s table, on the base.
static BASE_FIELD_PARSE: [FieldParse<FirestormDynamicGeometryInfoUpdateModuleData>; 10] = [
    FieldParse::new("InitialDelay", |i, d, u| lift(0, i, d, u), UserData::None),
    FieldParse::new("InitialHeight", |i, d, u| lift(1, i, d, u), UserData::None),
    FieldParse::new(
        "InitialMajorRadius",
        |i, d, u| lift(2, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "InitialMinorRadius",
        |i, d, u| lift(3, i, d, u),
        UserData::None,
    ),
    FieldParse::new("FinalHeight", |i, d, u| lift(4, i, d, u), UserData::None),
    FieldParse::new(
        "FinalMajorRadius",
        |i, d, u| lift(5, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "FinalMinorRadius",
        |i, d, u| lift(6, i, d, u),
        UserData::None,
    ),
    FieldParse::new("TransitionTime", |i, d, u| lift(7, i, d, u), UserData::None),
    FieldParse::new(
        "ReverseAtTransitionTime",
        |i, d, u| lift(8, i, d, u),
        UserData::None,
    ),
    FieldParse::end(),
];

/// `INI::parseParticleSystemTemplate` into `m_particleSystem[k]` (the user data): the next token, `None` for `NULL`.
fn parse_particle_system(
    i: &mut Ini<'_>,
    d: &mut FirestormDynamicGeometryInfoUpdateModuleData,
    u: &UserData,
) -> IniResult<()> {
    let UserData::Mask(k) = u else {
        return Err(IniError::new(IniErrorKind::InvalidData));
    };
    let name = i.get_next_token(None)?;
    d.particle_systems[*k as usize] = if name.eq_ignore_ascii_case(b"None") {
        None
    } else {
        Some(name)
    };
    Ok(())
}

/// `dataFieldParse`.
static FIELD_PARSE: [FieldParse<FirestormDynamicGeometryInfoUpdateModuleData>; 23] = [
    FieldParse::new(
        "DelayBetweenDamageFrames",
        |i, d, _| {
            d.delay_between_damage_frames = i.parse_duration_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "DamageAmount",
        |i, d, _| {
            d.damage_amount = i.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "MaxHeightForDamage",
        |i, d, _| {
            d.max_height_for_damage = i.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new("ParticleSystem1", parse_particle_system, UserData::Mask(0)),
    FieldParse::new("ParticleSystem2", parse_particle_system, UserData::Mask(1)),
    FieldParse::new("ParticleSystem3", parse_particle_system, UserData::Mask(2)),
    FieldParse::new("ParticleSystem4", parse_particle_system, UserData::Mask(3)),
    FieldParse::new("ParticleSystem5", parse_particle_system, UserData::Mask(4)),
    FieldParse::new("ParticleSystem6", parse_particle_system, UserData::Mask(5)),
    FieldParse::new("ParticleSystem7", parse_particle_system, UserData::Mask(6)),
    FieldParse::new("ParticleSystem8", parse_particle_system, UserData::Mask(7)),
    FieldParse::new("ParticleSystem9", parse_particle_system, UserData::Mask(8)),
    FieldParse::new("ParticleSystem10", parse_particle_system, UserData::Mask(9)),
    FieldParse::new(
        "ParticleSystem11",
        parse_particle_system,
        UserData::Mask(10),
    ),
    FieldParse::new(
        "ParticleSystem12",
        parse_particle_system,
        UserData::Mask(11),
    ),
    FieldParse::new(
        "ParticleSystem13",
        parse_particle_system,
        UserData::Mask(12),
    ),
    FieldParse::new(
        "ParticleSystem14",
        parse_particle_system,
        UserData::Mask(13),
    ),
    FieldParse::new(
        "ParticleSystem15",
        parse_particle_system,
        UserData::Mask(14),
    ),
    FieldParse::new(
        "ParticleSystem16",
        parse_particle_system,
        UserData::Mask(15),
    ),
    FieldParse::new(
        "FXList",
        |i, d, _| {
            let name = i.get_next_token(None)?;
            d.fx_list = if name.eq_ignore_ascii_case(b"None") {
                None
            } else {
                Some(name)
            };
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "ParticleOffsetZ",
        |i, d, _| {
            d.particle_offset_z = i.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "ScorchSize",
        |i, d, _| {
            d.scorch_size = i.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl FirestormDynamicGeometryInfoUpdateModuleData {
    /// `buildFieldParse(p)`: the base's, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, FirestormDynamicGeometryInfoUpdateModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<FirestormDynamicGeometryInfoUpdateModuleData> {
        let mut data = FirestormDynamicGeometryInfoUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                FirestormDynamicGeometryInfoUpdateModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// `class FirestormDynamicGeometryInfoUpdate : public DynamicGeometryInfoUpdate`.
#[derive(Clone, Debug)]
pub struct FirestormDynamicGeometryInfoUpdate {
    /// The `DynamicGeometryInfoUpdate` base (with the `UpdateModule` under it).
    geom: DynamicGeometryInfoUpdate,
    /// `getFirestormDynamicGeometryInfoUpdateModuleData()`.
    data: Arc<FirestormDynamicGeometryInfoUpdateModuleData>,
    /// `m_myParticleSystemID`.
    my_particle_system_id: [u32; MAX_FIRESTORM_SYSTEMS],
    /// `m_effectsFired`.
    effects_fired: bool,
    /// `m_scorchPlaced`.
    scorch_placed: bool,
    /// `m_lastDamageFrame`.
    last_damage_frame: u32,
}

impl PartialEq for FirestormDynamicGeometryInfoUpdate {
    /// The same base, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.geom == other.geom
            && Arc::ptr_eq(&self.data, &other.data)
            && self.state_words() == other.state_words()
    }
}

impl FirestormDynamicGeometryInfoUpdate {
    /// `getInterfaceMask()`: the update module's.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The constructor, which touches no world: the base's, then no systems, nothing fired, no damage yet.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<FirestormDynamicGeometryInfoUpdateModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let geom =
            DynamicGeometryInfoUpdate::unconstructed(object, Some(Arc::new(data.geom.clone())))?;
        Ok(FirestormDynamicGeometryInfoUpdate {
            geom,
            data,
            my_particle_system_id: [INVALID_PARTICLE_SYSTEM_ID; MAX_FIRESTORM_SYSTEMS],
            effects_fired: false,
            scorch_placed: false,
            last_damage_frame: 0,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        self.geom.update_module()
    }

    /// The module data (`getFirestormDynamicGeometryInfoUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<FirestormDynamicGeometryInfoUpdateModuleData> {
        &self.data
    }

    /// The `DynamicGeometryInfoUpdate` base.
    pub fn geometry_update(&self) -> &DynamicGeometryInfoUpdate {
        &self.geom
    }

    /// This class's state, as words (the test compares it with the base's): the ids, the two flags and the last damage frame.
    pub fn state_words(&self) -> Vec<u32> {
        let mut w = self.my_particle_system_id.to_vec();
        w.extend([
            u32::from(self.effects_fired),
            u32::from(self.scorch_placed),
            self.last_damage_frame,
        ]);
        w
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half (the base sets no wake frame).
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

    /// `xfer` with the scheduling half the slot holds: this class's version, then the base's `xfer` (its version, that half, its fields), then
    /// this class's fields.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current = <Self as UpdateBehavior>::XFER_VERSION;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        update_behavior::xfer_with(&mut self.geom, update, xfer)?;
        self.xfer_fields(xfer, version)
    }

    /// `crc` with the scheduling half the slot holds (the base's, the `UpdateModule`'s).
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// `doDamageScan()`: "scan objects around us and do damage to objects we have "passed over"".
    fn do_damage_scan(&self, world: &mut World, id: ObjectId) {
        let d = self.data.clone();
        let Some((pos, bounding_circle)) = world.objects.find(id).map(|o| {
            (
                o.get_position(),
                o.geometry_info.get_bounding_circle_radius(),
            )
        }) else {
            return;
        };
        // setup a damage info structure to do some damage
        let mut info = DamageInfo::default();
        info.input.damage_type = DAMAGE_FLAME;
        info.input.death_type = if crate::logic::modules::plant(2027) {
            0
        } else {
            DEATH_BURNED
        };
        info.input.source_id = id;
        info.input.amount = d.damage_amount;
        if bounding_circle != 0.0 {
            for other in world.partition_iterate_objects_in_range_at(
                pos,
                bounding_circle,
                FROM_BOUNDINGSPHERE_2D,
            ) {
                // it's too high above us. skip it.
                let Some(z) = world.objects.find(other).map(|o| o.get_position().z) else {
                    continue;
                };
                if z > pos.z + d.max_height_for_damage && !crate::logic::modules::plant(2026) {
                    continue;
                }
                let mut each = info.clone();
                world.attempt_damage(other, &mut each);
            }
        }
    }

    fn update_inner(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        // extend functionality
        UpdateBehavior::update(&mut self.geom, world, me);
        // get out of here if we haven't fired yet
        if self.geom.starting_delay_countdown() > 0 {
            return Ok(());
        }
        let d = self.data.clone();
        let id = me.object;
        // start effects for the first time
        if !self.effects_fired {
            // "construct a position to play the particle effects ... that is on the ground"
            let obj_pos = world
                .objects
                .find(id)
                .map(|o| o.get_position())
                .unwrap_or_default();
            let ground = world.terrain_get_ground_height(obj_pos.x, obj_pos.y);
            let pos = Coord3D {
                x: obj_pos.x,
                y: obj_pos.y,
                z: if crate::logic::modules::plant(2022) {
                    ground
                } else {
                    d.particle_offset_z + ground
                },
            };
            for i in 0..MAX_FIRESTORM_SYSTEMS {
                // `createParticleSystem(NULL)` makes none
                if let Some(template) = &d.particle_systems[i] {
                    self.my_particle_system_id[i] =
                        world.create_particle_system(template.clone(), pos);
                }
            }
            if let Some(fx) = d
                .fx_list
                .as_ref()
                .filter(|_| !crate::logic::modules::plant(2021))
            {
                world.emit(ObjectClientEvent::FxListDoFxObj {
                    fx: fx.clone(),
                    object_id: id,
                });
            }
            self.effects_fired = true;
            let player = world
                .get_controlling_player(id)
                .ok_or(Undefined::NullControllingPlayer)?;
            if let Some(p) = world.players.get_nth_mut(player) {
                p.academy_stats.record_firestorm_created();
            }
        }
        // "Parent goes first, and we use part of what he did to alter our bound particle system"
        let radius = world.objects.find(id).map_or(0.0, |o| {
            if crate::logic::modules::plant(2029) {
                o.geometry_info.get_minor_radius()
            } else {
                o.geometry_info.get_major_radius()
            }
        });
        for i in 0..MAX_FIRESTORM_SYSTEMS {
            let sys = self.my_particle_system_id[i];
            if sys != INVALID_PARTICLE_SYSTEM_ID {
                if world.body_particle_exists(sys) || crate::logic::modules::plant(2023) {
                    world.emit(ObjectClientEvent::ParticleSystemSetEmissionRadius {
                        id: sys,
                        radius,
                    });
                } else {
                    // "this system not found (it probably died)... stop trying to find it in the future"
                    self.my_particle_system_id[i] = INVALID_PARTICLE_SYSTEM_ID;
                }
            }
        }
        // when we first start running backward ... make a scorch mark
        if self.geom.switched_directions()
            && (!self.scorch_placed || crate::logic::modules::plant(2024))
        {
            let position = world
                .objects
                .find(id)
                .map(|o| o.get_position())
                .unwrap_or_default();
            world.emit(ObjectClientEvent::GameClientAddScorch {
                position,
                radius: d.scorch_size,
                scorch_type: SCORCH_1,
            });
            self.scorch_placed = true;
        }
        // scan and do some damage every once in a while
        let now = world.frame();
        if now.wrapping_sub(self.last_damage_frame) as f32 > d.delay_between_damage_frames
            || (!crate::logic::modules::plant(2025)
                && now.wrapping_sub(self.last_damage_frame) as f32 == d.delay_between_damage_frames)
        {
            self.do_damage_scan(world, id);
            self.last_damage_frame = world.frame();
        }
        Ok(())
    }
}

impl UpdateBehavior for FirestormDynamicGeometryInfoUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        self.geom.update_module()
    }
    /// "The update callback."
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if let Err(u) = self.update_inner(world, me) {
            world.cuts.module_errors.push((me, IniError::undefined(u)));
        }
        UPDATE_SLEEP_NONE
    }
    /// The constructor sets no wake frame.
    fn construct(&mut self, _world: &mut World, _me: ModuleHandle) {}
    /// This class's fields, after the base's `xfer`: the ids (`xferUser` of the array), the two flags, the last damage frame.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        let mut b: Vec<u8> = self
            .my_particle_system_id
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect();
        xfer.xfer_user(&mut b)?;
        for (k, c) in b.chunks(4).enumerate() {
            self.my_particle_system_id[k] = u32::from_le_bytes(c.try_into().expect("four bytes"));
        }
        xfer.xfer_bool(&mut self.effects_fired)?;
        if !crate::logic::modules::plant(2028) {
            xfer.xfer_bool(&mut self.scorch_placed)?;
        }
        xfer.xfer_unsigned_int(&mut self.last_damage_frame)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::FirestormDynamicGeometryInfoUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::FirestormDynamicGeometryInfoUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for FirestormDynamicGeometryInfoUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.geom.update_module().clone();
        base.crc(xfer)
    }
    /// `xfer`: see [`FirestormDynamicGeometryInfoUpdate::xfer_with`].
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.geom.update_module().clone();
        let r = self.xfer_with(&mut base, xfer);
        *self.geom.update_module_mut() = base;
        r
    }
    fn load_post_process(&mut self) {
        Snapshot::load_post_process(&mut self.geom);
    }
}

/// The factory's `NewModuleDataProc` for `FirestormDynamicGeometryInfoUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::FirestormDynamicGeometryInfoUpdate(
        Arc::new(FirestormDynamicGeometryInfoUpdateModuleData::friend_new_module_data(ini)?),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = FirestormDynamicGeometryInfoUpdateModuleData::default();
        assert_eq!(
            (
                d.max_height_for_damage,
                d.delay_between_damage_frames,
                d.fx_list.is_none()
            ),
            (20.0, 0.0, true)
        );
        let m = FirestormDynamicGeometryInfoUpdate::unconstructed(3, Some(Arc::new(d))).unwrap();
        assert!(m.state_words().iter().all(|&w| w == 0));
        assert_eq!(
            FirestormDynamicGeometryInfoUpdate::unconstructed(3, None)
                .unwrap_err()
                .kind,
            IniErrorKind::InvalidData
        );
    }
}
