// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/NeutronMissileSlowDeathUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/NeutronMissileSlowDeathUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `NeutronMissileSlowDeathBehavior`: a [`SlowDeathBehavior`] for the neutron missile superweapon. Each frame of its slow death, after the
//! base's update: on the first, its FX list at the ground under it (the frame recorded); then each enabled blast of nine whose delay has passed
//! since that frame goes off once (the objects in its outer radius toppled and damaged: the full damage inside the inner radius, falling off to
//! the minimum outside it; the first damage places a scorch mark), and each whose scorch delay has passed scorches once (the objects in range
//! take `MODELCONDITION_BURNED`, a tree's shadows are turned off).
//!
//! The module is a [`SlowDeathBehavior`] (the base's state and methods, [`slow_death_field_parse!`](crate::slow_death_field_parse) for its data)
//! plus this class's update; every call is the base's.
//!
//! # Upstream facts kept
//!
//! - A blast's time is `now - activationFrame > delay` (an unsigned difference against a real); the activation frame is the first frame of
//!   the slow death the update sees, and a slow death first seen at frame 0 is seen again as new on the next frame.
//! - The scan is `iterateObjectsInRange(position, outerRadius, FROM_CENTER_2D)`, the missile included if it is in the list; a blast with no
//!   outer radius does nothing, and is marked done.
//! - The damage (`DAMAGE_EXPLOSION`, `DEATH_EXPLODED`, the missile the source) is the maximum within the inner radius, else the maximum times
//!   `1 - (dist - inner) / (outer - inner + 0.01)`, at least the minimum; no damage when it comes to 0. The toppling comes first, for every
//!   object in range.
//! - The push force is never applied (upstream's code is commented out).
//! - The xfer is version 1: the version, `SlowDeathBehavior`'s, the activation frame, the blast count (9: another is `SC_INVALID_DATA`, here
//!   [`XferStatus::ListNotEmpty`]), the two flag arrays and whether the scorch is placed.
//!
//! # Where upstream is undefined
//!
//! The base's (`SlowDeathBehavior`).
//!
//! `p1w13-sonnet`, batch R49, 2026-10-25.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::kind_of::KINDOF_SHRUBBERY;
use crate::common::model_state::MODELCONDITION_BURNED;
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DIE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferStatus, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_EXPLOSION, DEATH_EXPLODED};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{UpdateModule, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::slow_death_behavior::{SlowDeathBehavior, SlowDeathBehaviorModuleData};
use crate::logic::modules::topple_update::{TOPPLE_OPTIONS_NO_BOUNCE, TOPPLE_OPTIONS_NO_FX};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `MAX_NEUTRON_BLASTS`.
pub const MAX_NEUTRON_BLASTS: usize = 9;
/// `FROM_CENTER_2D` (`PartitionManager.h`'s `DistanceCalculationType`).
const FROM_CENTER_2D: i32 = 0;
/// `SCORCH_1`.
const SCORCH_1: i32 = 0;

/// `struct BlastInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BlastInfo {
    /// `enabled`, `delay` and `scorchDelay` (frames), `innerRadius`, `outerRadius`, `maxDamage`, `minDamage`, `toppleSpeed`, `pushForceMag`.
    pub enabled: bool,
    pub delay: f32,
    pub scorch_delay: f32,
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub max_damage: f32,
    pub min_damage: f32,
    pub topple_speed: f32,
    pub push_force_mag: f32,
}

/// `class NeutronMissileSlowDeathBehaviorModuleData : public SlowDeathBehaviorModuleData`.
#[derive(Clone, Debug, Default)]
pub struct NeutronMissileSlowDeathBehaviorModuleData {
    /// The base.
    pub slow: SlowDeathBehaviorModuleData,
    /// `m_blastInfo`.
    pub blast_info: [BlastInfo; MAX_NEUTRON_BLASTS],
    /// `m_scorchSize`.
    pub scorch_size: f32,
    /// `m_fxList`, by name (`None` for `NULL`).
    pub fx_list: Option<Vec<u8>>,
}

fn slow_of(d: &mut NeutronMissileSlowDeathBehaviorModuleData) -> &mut SlowDeathBehaviorModuleData {
    &mut d.slow
}

/// `SlowDeathBehaviorModuleData::buildFieldParse`'s table.
static SLOW_FIELD_PARSE: [FieldParse<NeutronMissileSlowDeathBehaviorModuleData>; 20] =
    crate::slow_death_field_parse!(NeutronMissileSlowDeathBehaviorModuleData, slow_of);

/// `NeutronMissileSlowDeathBehaviorModuleData::buildFieldParse`'s table.
static FIELD_PARSE: [FieldParse<NeutronMissileSlowDeathBehaviorModuleData>; 84] = [
    FieldParse::new("ScorchMarkSize", |i, d, _| { d.scorch_size = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new(
        "FXList",
        |i, d, _| {
            // `INI::parseFXList`: the next token, `NULL` for `None`
            let name = i.get_next_token(None)?;
            d.fx_list = if name.eq_ignore_ascii_case(b"None") { None } else { Some(name) };
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new("Blast1Enabled", |i, d, _| { d.blast_info[0].enabled = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast1Delay", |i, d, _| { d.blast_info[0].delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast1ScorchDelay", |i, d, _| { d.blast_info[0].scorch_delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast1InnerRadius", |i, d, _| { d.blast_info[0].inner_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast1OuterRadius", |i, d, _| { d.blast_info[0].outer_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast1MaxDamage", |i, d, _| { d.blast_info[0].max_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast1MinDamage", |i, d, _| { d.blast_info[0].min_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast1ToppleSpeed", |i, d, _| { d.blast_info[0].topple_speed = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast1PushForce", |i, d, _| { d.blast_info[0].push_force_mag = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast2Enabled", |i, d, _| { d.blast_info[1].enabled = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast2Delay", |i, d, _| { d.blast_info[1].delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast2ScorchDelay", |i, d, _| { d.blast_info[1].scorch_delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast2InnerRadius", |i, d, _| { d.blast_info[1].inner_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast2OuterRadius", |i, d, _| { d.blast_info[1].outer_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast2MaxDamage", |i, d, _| { d.blast_info[1].max_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast2MinDamage", |i, d, _| { d.blast_info[1].min_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast2ToppleSpeed", |i, d, _| { d.blast_info[1].topple_speed = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast2PushForce", |i, d, _| { d.blast_info[1].push_force_mag = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast3Enabled", |i, d, _| { d.blast_info[2].enabled = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast3Delay", |i, d, _| { d.blast_info[2].delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast3ScorchDelay", |i, d, _| { d.blast_info[2].scorch_delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast3InnerRadius", |i, d, _| { d.blast_info[2].inner_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast3OuterRadius", |i, d, _| { d.blast_info[2].outer_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast3MaxDamage", |i, d, _| { d.blast_info[2].max_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast3MinDamage", |i, d, _| { d.blast_info[2].min_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast3ToppleSpeed", |i, d, _| { d.blast_info[2].topple_speed = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast3PushForce", |i, d, _| { d.blast_info[2].push_force_mag = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast4Enabled", |i, d, _| { d.blast_info[3].enabled = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast4Delay", |i, d, _| { d.blast_info[3].delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast4ScorchDelay", |i, d, _| { d.blast_info[3].scorch_delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast4InnerRadius", |i, d, _| { d.blast_info[3].inner_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast4OuterRadius", |i, d, _| { d.blast_info[3].outer_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast4MaxDamage", |i, d, _| { d.blast_info[3].max_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast4MinDamage", |i, d, _| { d.blast_info[3].min_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast4ToppleSpeed", |i, d, _| { d.blast_info[3].topple_speed = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast4PushForce", |i, d, _| { d.blast_info[3].push_force_mag = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast5Enabled", |i, d, _| { d.blast_info[4].enabled = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast5Delay", |i, d, _| { d.blast_info[4].delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast5ScorchDelay", |i, d, _| { d.blast_info[4].scorch_delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast5InnerRadius", |i, d, _| { d.blast_info[4].inner_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast5OuterRadius", |i, d, _| { d.blast_info[4].outer_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast5MaxDamage", |i, d, _| { d.blast_info[4].max_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast5MinDamage", |i, d, _| { d.blast_info[4].min_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast5ToppleSpeed", |i, d, _| { d.blast_info[4].topple_speed = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast5PushForce", |i, d, _| { d.blast_info[4].push_force_mag = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast6Enabled", |i, d, _| { d.blast_info[5].enabled = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast6Delay", |i, d, _| { d.blast_info[5].delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast6ScorchDelay", |i, d, _| { d.blast_info[5].scorch_delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast6InnerRadius", |i, d, _| { d.blast_info[5].inner_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast6OuterRadius", |i, d, _| { d.blast_info[5].outer_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast6MaxDamage", |i, d, _| { d.blast_info[5].max_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast6MinDamage", |i, d, _| { d.blast_info[5].min_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast6ToppleSpeed", |i, d, _| { d.blast_info[5].topple_speed = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast6PushForce", |i, d, _| { d.blast_info[5].push_force_mag = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast7Enabled", |i, d, _| { d.blast_info[6].enabled = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast7Delay", |i, d, _| { d.blast_info[6].delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast7ScorchDelay", |i, d, _| { d.blast_info[6].scorch_delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast7InnerRadius", |i, d, _| { d.blast_info[6].inner_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast7OuterRadius", |i, d, _| { d.blast_info[6].outer_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast7MaxDamage", |i, d, _| { d.blast_info[6].max_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast7MinDamage", |i, d, _| { d.blast_info[6].min_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast7ToppleSpeed", |i, d, _| { d.blast_info[6].topple_speed = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast7PushForce", |i, d, _| { d.blast_info[6].push_force_mag = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast8Enabled", |i, d, _| { d.blast_info[7].enabled = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast8Delay", |i, d, _| { d.blast_info[7].delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast8ScorchDelay", |i, d, _| { d.blast_info[7].scorch_delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast8InnerRadius", |i, d, _| { d.blast_info[7].inner_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast8OuterRadius", |i, d, _| { d.blast_info[7].outer_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast8MaxDamage", |i, d, _| { d.blast_info[7].max_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast8MinDamage", |i, d, _| { d.blast_info[7].min_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast8ToppleSpeed", |i, d, _| { d.blast_info[7].topple_speed = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast8PushForce", |i, d, _| { d.blast_info[7].push_force_mag = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast9Enabled", |i, d, _| { d.blast_info[8].enabled = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast9Delay", |i, d, _| { d.blast_info[8].delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast9ScorchDelay", |i, d, _| { d.blast_info[8].scorch_delay = i.parse_duration_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast9InnerRadius", |i, d, _| { d.blast_info[8].inner_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast9OuterRadius", |i, d, _| { d.blast_info[8].outer_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast9MaxDamage", |i, d, _| { d.blast_info[8].max_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast9MinDamage", |i, d, _| { d.blast_info[8].min_damage = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast9ToppleSpeed", |i, d, _| { d.blast_info[8].topple_speed = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("Blast9PushForce", |i, d, _| { d.blast_info[8].push_force_mag = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl NeutronMissileSlowDeathBehaviorModuleData {
    /// `buildFieldParse(p)`: `SlowDeathBehaviorModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, NeutronMissileSlowDeathBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&SLOW_FIELD_PARSE)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<NeutronMissileSlowDeathBehaviorModuleData> {
        let mut data = NeutronMissileSlowDeathBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, NeutronMissileSlowDeathBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The module factory's data proc.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::NeutronMissileSlowDeathBehavior(Arc::new(NeutronMissileSlowDeathBehaviorModuleData::friend_new_module_data(ini)?)))
}

/// `class NeutronMissileSlowDeathBehavior : public SlowDeathBehavior`.
#[derive(Clone, Debug)]
pub struct NeutronMissileSlowDeathBehavior {
    /// The base.
    base: SlowDeathBehavior,
    /// The data (this class's part; the base holds its own).
    data: Arc<NeutronMissileSlowDeathBehaviorModuleData>,
    /// `m_activationFrame`: "frame we were activated on".
    activation_frame: u32,
    /// `m_completedBlasts`, `m_completedScorchBlasts`: "blasts indexes we've already done".
    completed_blasts: [bool; MAX_NEUTRON_BLASTS],
    completed_scorch_blasts: [bool; MAX_NEUTRON_BLASTS],
    /// `m_scorchPlaced`: "TRUE once we've placed the scorch mark".
    scorch_placed: bool,
}

impl PartialEq for NeutronMissileSlowDeathBehavior {
    /// The same base, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl NeutronMissileSlowDeathBehavior {
    /// `getInterfaceMask()`: `SlowDeathBehavior`'s.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE
    }

    /// The constructor's first half: the base's (`INI_INVALID_DATA` for null data or a probability modifier under 1), then this class's.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<NeutronMissileSlowDeathBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = SlowDeathBehavior::unconstructed(object, Some(Arc::new(data.slow.clone())))?;
        Ok(NeutronMissileSlowDeathBehavior {
            base,
            data,
            activation_frame: 0,
            completed_blasts: [false; MAX_NEUTRON_BLASTS],
            completed_scorch_blasts: [false; MAX_NEUTRON_BLASTS],
            scorch_placed: false,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        self.base.update_module()
    }

    /// The state the test compares: the base's, then the activation frame, the two flag arrays (bits) and whether the scorch is placed.
    pub fn state_words(&self) -> Vec<u32> {
        let bits = |a: &[bool; MAX_NEUTRON_BLASTS]| a.iter().enumerate().fold(0u32, |m, (i, &b)| m | u32::from(b) << i);
        let mut w = self.base.state_words();
        w.extend([self.activation_frame, bits(&self.completed_blasts), bits(&self.completed_scorch_blasts), u32::from(self.scorch_placed)]);
        w
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half: the base's lists, then `setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER)`.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.base.find_effects(world);
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: this class's version, `SlowDeathBehavior::xfer`, then this class's fields.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)?;
        update_behavior::xfer_with(&mut self.base, update, xfer)?;
        xfer.xfer_unsigned_int(&mut self.activation_frame)?;
        // "size of MAX_NEUTRON_BLASTS since we use it for two arrays here"
        let mut max_neutron_blasts = MAX_NEUTRON_BLASTS as u8;
        xfer.xfer_unsigned_byte(&mut max_neutron_blasts)?;
        if max_neutron_blasts as usize != MAX_NEUTRON_BLASTS {
            // DEBUG_CRASH(( "... Size of MAX_NEUTRON_BLASTS has changed ..." )); throw SC_INVALID_DATA;
            return Err(XferStatus::ListNotEmpty);
        }
        for b in self.completed_blasts.iter_mut() {
            xfer.xfer_bool(b)?;
        }
        if !crate::logic::modules::plant(2191) {
            for b in self.completed_scorch_blasts.iter_mut() {
                xfer.xfer_bool(b)?;
            }
        }
        xfer.xfer_bool(&mut self.scorch_placed)?;
        Ok(())
    }

    /// `crc` with the scheduling half the slot holds: the base's.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds: the base's.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// The part of `update()` after the base's.
    fn update_blasts(&mut self, world: &mut World, me: ModuleHandle) {
        // "get out of here if we're not activated yet"
        if !self.base.is_slow_death_activated() {
            return;
        }
        let d = self.data.clone();
        let curr_frame = world.frame();
        // "when we become activated we want to do a few things"
        if self.activation_frame == 0 {
            let missile_pos = world.objects.find(me.object).map(|o| o.get_position()).unwrap_or_default();
            // "get the position to play the effect at on the ground"
            let mut pos = Coord3D { x: missile_pos.x, y: missile_pos.y, z: 0.0 };
            pos.z = world.terrain_get_ground_height(pos.x, pos.y);
            self.activation_frame = curr_frame;
            if let Some(fx) = d.fx_list.clone() {
                world.emit(ObjectClientEvent::FxListDoFxPos { fx, position: pos });
            }
        }
        // "see if it's time for any explosions"
        for i in 0..MAX_NEUTRON_BLASTS {
            let blast = &d.blast_info[i];
            if !blast.enabled {
                continue;
            }
            let since = curr_frame.wrapping_sub(self.activation_frame) as f32;
            if !self.completed_blasts[i] && since > blast.delay {
                self.do_blast(world, me, blast);
                self.completed_blasts[i] = !crate::logic::modules::plant(2192);
            }
            if !self.completed_scorch_blasts[i] && since > if crate::logic::modules::plant(2193) { blast.delay } else { blast.scorch_delay } {
                Self::do_scorch_blast(world, me, blast);
                self.completed_scorch_blasts[i] = true;
            }
        }
    }

    /// `doBlast(blastInfo)`: "do a single blast for the bomb".
    fn do_blast(&mut self, world: &mut World, me: ModuleHandle, blast: &BlastInfo) {
        let missile = me.object;
        let missile_pos = world.objects.find(missile).map(|o| o.get_position()).unwrap_or_default();
        let mut damage_info = DamageInfo::default();
        damage_info.input.damage_type = DAMAGE_EXPLOSION;
        damage_info.input.death_type = DEATH_EXPLODED;
        damage_info.input.source_id = if crate::logic::modules::plant(2199) { 0 } else { missile };
        damage_info.input.amount = blast.min_damage;
        // "scan objects around us and do damage to objects we have passed over and are behind us"
        if blast.outer_radius == 0.0 {
            return;
        }
        for other in world.partition_iterate_objects_in_range_at(missile_pos, blast.outer_radius, FROM_CENTER_2D) {
            let Some(other_pos) = world.objects.find(other).map(|o| o.get_position()) else { continue };
            // "compute vector from the missile to other object"
            let force_vector = Coord3D { x: other_pos.x - missile_pos.x, y: other_pos.y - missile_pos.y, z: other_pos.z - missile_pos.z };
            // "try to topple other object"
            world.topple(other, Some(force_vector), blast.topple_speed, TOPPLE_OPTIONS_NO_BOUNCE | TOPPLE_OPTIONS_NO_FX);
            let dist = force_vector.length();
            if dist <= blast.inner_radius {
                damage_info.input.amount = blast.max_damage;
            } else {
                let reach = if crate::logic::modules::plant(2194) { 0.0 } else { 0.01 };
                let percent = 1.0 - ((dist - blast.inner_radius) / (blast.outer_radius - blast.inner_radius + reach));
                damage_info.input.amount = blast.max_damage * percent;
                if damage_info.input.amount < blast.min_damage && !crate::logic::modules::plant(2195) {
                    damage_info.input.amount = blast.min_damage;
                }
            }
            // "do actual damage"
            if damage_info.input.amount != 0.0 {
                let mut each = damage_info.clone();
                world.attempt_damage(other, &mut each);
                // "place a scorch mark if we haven't already"
                if !self.scorch_placed {
                    world.emit(ObjectClientEvent::GameClientAddScorch { position: missile_pos, radius: self.data.scorch_size, scorch_type: SCORCH_1 });
                    self.scorch_placed = !crate::logic::modules::plant(2196);
                }
            }
        }
    }

    /// `doScorchBlast(blastInfo)`: "this doesn't do actual damage, but it scorches things".
    fn do_scorch_blast(world: &mut World, me: ModuleHandle, blast: &BlastInfo) {
        let missile_pos = world.objects.find(me.object).map(|o| o.get_position()).unwrap_or_default();
        if blast.outer_radius == 0.0 {
            return;
        }
        for other in world.partition_iterate_objects_in_range_at(missile_pos, blast.outer_radius, FROM_CENTER_2D) {
            // "set model condition as all burnt up"
            if !crate::logic::modules::plant(2197) {
                world.set_model_condition_state(other, MODELCONDITION_BURNED);
            }
            // "remove the shadows from trees"
            if world.objects.find(other).is_some_and(|o| o.is_kind_of(KINDOF_SHRUBBERY)) && world.has_drawable(other) && !crate::logic::modules::plant(2198) {
                world.emit(ObjectClientEvent::DrawableSetShadowsEnabled { object_id: other, enabled: false });
            }
        }
    }
}

impl UpdateBehavior for NeutronMissileSlowDeathBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        self.base.update_base()
    }
    /// `update()`: the base's, then the blasts (never sleeps).
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        UpdateBehavior::update(&mut self.base, world, me);
        self.update_blasts(world, me);
        UPDATE_SLEEP_NONE
    }
    fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.base.construct(world, me);
    }
    /// Every call is the base's (`OnDie` with this class lent to the slot: the base's `onDie` may choose this module).
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnDie(info) => {
                crate::logic::modules::lend_clone(world, me, self, UpdateBody::NeutronMissileSlowDeathBehavior, Self::from_body, |w, m| m.base.on_die(w, me, &info));
                ModuleReply::Done
            }
            ModuleCall::BeginSlowDeath(_) => {
                crate::logic::modules::lend_clone(world, me, self, UpdateBody::NeutronMissileSlowDeathBehavior, Self::from_body, |w, m| m.base.begin_slow_death(w, me));
                ModuleReply::Done
            }
            _ => self.base.other_call(world, me, call),
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::NeutronMissileSlowDeathBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::NeutronMissileSlowDeathBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for NeutronMissileSlowDeathBehavior {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.base.crc(xfer)
    }
    /// `xfer`: see [`NeutronMissileSlowDeathBehavior::xfer_with`].
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut update = self.base.update_module().clone();
        let r = self.xfer_with(&mut update, xfer);
        *self.base.update_module_mut() = update;
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
        let d = NeutronMissileSlowDeathBehaviorModuleData::default();
        assert_eq!((d.scorch_size, d.blast_info[8], d.fx_list.is_none(), d.slow.probability_modifier), (0.0, BlastInfo::default(), true, 10));
        let m = NeutronMissileSlowDeathBehavior::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words()[5..], [0, 0, 0, 0]);
    }
}
