// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/GrantStealthBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/GrantStealthBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `GrantStealthBehavior`: a pulse that grants stealth (the GLA camouflage netting). Made with a particle system at the object; every frame the
//! scan radius grows by `RadiusGrowRate` (from `StartRadius`, up to `FinalRadius`), and every allied, living object on the same map in range
//! (2D, from the centre), not the object itself, of the `KindOf` kinds and with a stealth update receives a grant and flashes as selected. At the
//! final radius the object destroys itself. A dead object stops.
//!
//! The scan is [`World::partition_iterate_allies_in_range`] (the same three filters as `WeaponBonusUpdate`); the grant is
//! [`World::contain_stealth_receive_grant`] (`receiveGrant()`'s defaults: active, no frames); the self-destruction
//! [`World::contain_destroy_object`]. The `m_initiallyActive`, `m_singleBurst` and `m_healingAmount` of the data class are never set or parsed
//! upstream; they are not here. The destructor's `destroyParticleSystemByID` has no Rust counterpart (no destruction hook).
use std::sync::Arc;

use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData,
};
use crate::common::kind_of::KindOfMaskType;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER,
    UPDATE_SLEEP_NONE,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class GrantStealthBehaviorModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct GrantStealthBehaviorModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_startRadius`.
    pub start_radius: f32,
    /// `m_finalRadius`.
    pub final_radius: f32,
    /// `m_radiusGrowRate`.
    pub radius_grow_rate: f32,
    /// `m_kindOf` (all kinds by default).
    pub kind_of: KindOfMaskType,
    /// `m_radiusParticleSystemTmpl`, by name (`None` for `NULL`).
    pub radius_particle_system: Option<Vec<u8>>,
}

impl Default for GrantStealthBehaviorModuleData {
    /// `GrantStealthBehaviorModuleData()`: radii 0 to 200, growing 10 a frame, every kind, no particle system.
    fn default() -> Self {
        let mut kind_of = KindOfMaskType::new();
        kind_of.flip();
        GrantStealthBehaviorModuleData {
            update: UpdateModuleData::default(),
            start_radius: 0.0,
            final_radius: 200.0,
            radius_grow_rate: 10.0,
            kind_of,
            radius_particle_system: None,
        }
    }
}

/// `INI::parseParticleSystemTemplate`: `None` is `NULL`; any other name is looked up when it is used.
fn parse_particle_system(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let token = ini.get_next_token(None)?;
    Ok((!token.eq_ignore_ascii_case(b"None")).then_some(token))
}

/// `GrantStealthBehaviorModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<GrantStealthBehaviorModuleData>; 6] = [
    FieldParse::new(
        "StartRadius",
        |ini, d, _| {
            d.start_radius = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "FinalRadius",
        |ini, d, _| {
            d.final_radius = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "RadiusGrowRate",
        |ini, d, _| {
            d.radius_grow_rate = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "KindOf",
        |ini, d, _| d.kind_of.parse_from_ini(ini),
        UserData::None,
    ),
    FieldParse::new(
        "RadiusParticleSystemName",
        |ini, d, _| {
            d.radius_particle_system = parse_particle_system(ini)?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl GrantStealthBehaviorModuleData {
    /// `buildFieldParse(p)`: the base's (no fields), then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, GrantStealthBehaviorModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<GrantStealthBehaviorModuleData> {
        let mut data = GrantStealthBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                GrantStealthBehaviorModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// `class GrantStealthBehavior : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct GrantStealthBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getGrantStealthBehaviorModuleData()`.
    data: Arc<GrantStealthBehaviorModuleData>,
    /// `m_radiusParticleSystemID` (0: `INVALID_PARTICLE_SYSTEM_ID`).
    radius_particle_system_id: u32,
    /// `m_currentScanRadius`.
    current_scan_radius: f32,
}

impl PartialEq for GrantStealthBehavior {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.radius_particle_system_id == other.radius_particle_system_id
            && self.current_scan_radius.to_bits() == other.current_scan_radius.to_bits()
    }
}

impl GrantStealthBehavior {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and
    /// no particle system yet, the scan radius `StartRadius`.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<GrantStealthBehaviorModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(GrantStealthBehavior {
            update,
            radius_particle_system_id: 0,
            current_scan_radius: data.start_radius,
            data,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getGrantStealthBehaviorModuleData()`).
    pub fn module_data(&self) -> &Arc<GrantStealthBehaviorModuleData> {
        &self.data
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object: awake now.
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

    /// The state the test compares: the particle system id, the scan radius (bits).
    pub fn state_words(&self) -> [u32; 2] {
        [
            self.radius_particle_system_id,
            self.current_scan_radius.to_bits(),
        ]
    }

    /// `grantStealthToObject(obj)`.
    fn grant_stealth_to_object(&self, world: &mut World, me: ObjectId, obj: ObjectId) {
        if obj == me && !crate::logic::modules::plant(1933) {
            return;
        }
        let kinds = world
            .objects
            .find(obj)
            .is_some_and(|o| o.template().is_any_kind_of(&self.data.kind_of));
        if !kinds && !crate::logic::modules::plant(1934) {
            return;
        }
        if world.objects.find(obj).and_then(|o| o.stealth).is_some() {
            world.contain_stealth_receive_grant(obj, true, 0);
            if world.has_drawable(obj) && !crate::logic::modules::plant(1935) {
                world.emit(ObjectClientEvent::DrawableFlashAsSelected { object_id: obj });
            }
        }
    }
}

impl UpdateBehavior for GrantStealthBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if world
            .objects
            .find(me.object)
            .is_some_and(|o| o.is_effectively_dead())
        {
            return UPDATE_SLEEP_FOREVER;
        }
        let d = self.data.clone();
        if !crate::logic::modules::plant(1931) {
            self.current_scan_radius += d.radius_grow_rate;
        }
        let mut this_is_final_scan = false;
        if self.current_scan_radius >= d.final_radius {
            self.current_scan_radius = d.final_radius;
            this_is_final_scan = true;
        }
        for obj in world.partition_iterate_allies_in_range(me.object, self.current_scan_radius) {
            self.grant_stealth_to_object(world, me.object, obj);
        }
        if this_is_final_scan && !crate::logic::modules::plant(1932) {
            world.contain_destroy_object(me.object);
            return UPDATE_SLEEP_FOREVER;
        }
        UPDATE_SLEEP_NONE
    }
    /// The constructor: the particle system at the object, then awake now.
    fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        if let Some(name) = self.data.radius_particle_system.clone() {
            let pos = world
                .objects
                .find(me.object)
                .map(|o| o.get_position())
                .unwrap_or_default();
            self.radius_particle_system_id = world.create_particle_system(name, pos);
        }
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
    }
    /// The fields after the base: the particle system id (4 bytes of user data), the scan radius.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        let mut bytes = self.radius_particle_system_id.to_le_bytes();
        xfer.xfer_user(&mut bytes)?;
        self.radius_particle_system_id = u32::from_le_bytes(bytes);
        xfer.xfer_real(&mut self.current_scan_radius)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::GrantStealthBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::GrantStealthBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for GrantStealthBehavior {
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

/// The factory's `NewModuleDataProc` for `GrantStealthBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::GrantStealthBehavior(Arc::new(
        GrantStealthBehaviorModuleData::friend_new_module_data(ini)?,
    )))
}
