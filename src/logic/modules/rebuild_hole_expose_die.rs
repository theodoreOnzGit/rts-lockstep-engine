// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/RebuildHoleExposeDie.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/RebuildHoleExposeDie.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `RebuildHoleExposeDie`: the hole a GLA building leaves when it dies. Unless the building belongs to the neutral player or an inactive one, or was
//! still under construction, a `HoleName` object of the building's team takes its place (position, orientation, geometry, script name; the
//! pathfinder learns it), with `HoleMaxHealth`; the hole's `RebuildHoleBehavior` is told what to rebuild and who died; with `TransferAttackers` the
//! building's attackers move to the hole.
//!
//! # Upstream facts kept
//!
//! - The controlling player is read unchecked once it is not the neutral player ([`Undefined::NullControllingPlayer`]); a missing `HoleName`
//!   template is `ThingFactory::newObject(NULL)`'s throw; the hole's body is read unchecked ([`Undefined::NullBodyModule`]).
//! - The debug build's hand-of-god test is not in the shipped build.

use std::sync::Arc;

use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData,
};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::DamageInfo;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::die_module::{DieModule, DieModuleData, DIE_MODULE_DATA_FIELD_PARSE};
use crate::logic::modules::die_body::{die_module_call, DieBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::update_body::UpdateBody;

/// `class RebuildHoleExposeDieModuleData : public DieModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct RebuildHoleExposeDieModuleData {
    /// The `DieModuleData` base (the die mux and the behaviour base).
    pub die: DieModuleData,
    /// `m_holeName`.
    pub hole_name: Vec<u8>,
    /// `m_holeMaxHealth`.
    pub hole_max_health: f32,
    /// `m_transferAttackers`.
    pub transfer_attackers: bool,
}

impl Default for RebuildHoleExposeDieModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl RebuildHoleExposeDieModuleData {
    /// `RebuildHoleExposeDieModuleData()`: no hole name, no health, attackers transferred.
    pub fn new() -> Self {
        RebuildHoleExposeDieModuleData {
            die: DieModuleData::new(),
            hole_name: Vec::new(),
            hole_max_health: 0.0,
            transfer_attackers: true,
        }
    }

    /// `buildFieldParse(p)`: the behaviour base, the four die mux fields (read from the layer-5 table into `die`), then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, RebuildHoleExposeDieModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<RebuildHoleExposeDieModuleData> {
        let mut data = RebuildHoleExposeDieModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                RebuildHoleExposeDieModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// Lifts entry `k` of the layer-5 die field table onto `RebuildHoleExposeDieModuleData::die`.
fn lift_die_field(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut RebuildHoleExposeDieModuleData,
    u: &UserData,
) -> IniResult<()> {
    match DIE_MODULE_DATA_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.die, u),
        None => Ok(()),
    }
}

/// `DieModuleData::buildFieldParse`'s fields, then `RebuildHoleExposeDieModuleData`'s.
static FIELD_PARSE: [FieldParse<RebuildHoleExposeDieModuleData>; 8] = [
    FieldParse::new(
        "DeathTypes",
        |i, d, u| lift_die_field(0, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "VeterancyLevels",
        |i, d, u| lift_die_field(1, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "ExemptStatus",
        |i, d, u| lift_die_field(2, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "RequiredStatus",
        |i, d, u| lift_die_field(3, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "HoleName",
        |ini, d, _| {
            d.hole_name = ini.parse_ascii_string()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "HoleMaxHealth",
        |ini, d, _| {
            d.hole_max_health = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "TransferAttackers",
        |ini, d, _| {
            d.transfer_attackers = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

/// `class RebuildHoleExposeDie : public DieModule`.
#[derive(Clone, Debug)]
pub struct RebuildHoleExposeDie {
    die: DieModule,
    /// `getRebuildHoleExposeDieModuleData()`.
    data: Arc<RebuildHoleExposeDieModuleData>,
}

impl PartialEq for RebuildHoleExposeDie {
    /// The same object, the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.die.behavior().object() == other.die.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl RebuildHoleExposeDie {
    /// `RebuildHoleExposeDie(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(
        object: ObjectId,
        data: Option<Arc<RebuildHoleExposeDieModuleData>>,
    ) -> IniResult<RebuildHoleExposeDie> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let die = DieModule::new(object, Some(Arc::new(data.die.clone())))?;
        Ok(RebuildHoleExposeDie { die, data })
    }

    /// The data, as `getRebuildHoleExposeDieModuleData()`.
    pub fn module_data(&self) -> &Arc<RebuildHoleExposeDieModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        die_module_call(self, world, me, call)
    }

    /// The body of `onDie` after the applicability test. An error is upstream's undefined behaviour (or `newObject`'s throw).
    fn expose(&self, world: &mut World, us: ObjectId) -> Result<(), IniError> {
        let modd = &self.data;
        // "if we are not owned by the neutral player, we will create a hole"
        let player = world.get_controlling_player(us);
        if player == Some(0) && !crate::logic::modules::plant(1791) {
            return Ok(());
        }
        let player = player.ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        let active = world
            .players
            .get_nth(player)
            .is_some_and(|p| !p.observer && !p.is_player_dead);
        let under_construction = world
            .objects
            .find(us)
            .is_some_and(|o| o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION));
        if !active || (under_construction && !crate::logic::modules::plant(1792)) {
            return Ok(());
        }
        // "create the hole"; `ThingFactory::newObject(NULL)` throws
        let template = world
            .thing_factory_find_template(&modd.hole_name)
            .ok_or_else(|| IniError::new(IniErrorKind::BadArg))?;
        let team = world.get_team(us);
        let hole = world.crate_new_object(template, team);
        let (pos, angle, geom, name, us_template) = {
            let o = world.objects.find(us).expect("the dying object");
            (
                o.get_position(),
                o.thing.orientation(),
                o.geometry_info.clone(),
                o.name.clone(),
                o.template().get_template_id(),
            )
        };
        // "set the position of the hole to that of our position"
        world.set_position(hole, &pos);
        world.crate_set_orientation(hole, angle);
        // "set the geometry info of the hole to that of us"
        world.set_geometry_info(hole, &geom);
        // "Transfer the structure's name to the hole."
        world.cut(CutCall::ScriptEngineTransferObjectName { name, object: hole });
        // "add the hole to the pathfind map"
        world.cut(CutCall::PathfinderAddObjectToPathfindMap { object: hole });
        // "set the max health of the hole"
        let body = world
            .objects
            .find(hole)
            .and_then(|o| o.body)
            .ok_or_else(|| IniError::undefined(Undefined::NullBodyModule))?;
        world.module_call(hole, body, ModuleCall::SetMaxHealth(modd.hole_max_health));
        // "set the information in the hole about what to build"
        if let Some(rhbi) = world
            .find_module(hole, b"RebuildHoleBehavior")
            .filter(|_| !crate::logic::modules::plant(1793))
        {
            // "start the rebuild process"
            world.module_call(
                hole,
                rhbi,
                ModuleCall::StartRebuildProcess {
                    template: us_template,
                    spawner: us,
                },
            );
        }
        if modd.transfer_attackers && !crate::logic::modules::plant(1794) {
            for obj in world.objects.ids_in_order() {
                let Some(ai) = world.objects.find(obj).and_then(|o| o.ai) else {
                    continue;
                };
                world.module_call(obj, ai, ModuleCall::AiTransferAttack { from: us, to: hole });
            }
        }
        Ok(())
    }
}

impl DieBehavior for RebuildHoleExposeDie {
    fn die(&self) -> &DieModule {
        &self.die
    }
    fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
        if !self.is_die_applicable(world, me, info) {
            return;
        }
        if let Err(e) = self.expose(world, me.object) {
            world.cuts.module_errors.push((me, e));
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::RebuildHoleExposeDie(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::RebuildHoleExposeDie(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for RebuildHoleExposeDie {
    /// `crc`: `DieModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.die.crc(xfer)
    }
    /// `xfer` (version 1, "Initial version"): the version, `DieModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.die.xfer(xfer)?;
        Ok(())
    }
    /// `loadPostProcess`: `DieModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.die.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `RebuildHoleExposeDie`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::RebuildHoleExposeDie(Arc::new(
        RebuildHoleExposeDieModuleData::friend_new_module_data(ini)?,
    )))
}
