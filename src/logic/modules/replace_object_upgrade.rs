// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ReplaceObjectUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/ReplaceObjectUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ReplaceObjectUpgrade`: the object is replaced by another, made from the template `ReplaceObject` names, in the same place and on the
//! same team (a structure that turns into its upgraded self).
//!
//! `ReplaceObject` is `INI::parseAsciiString`. Upstream `upgradeImplementation`, in this order: the object's transform matrix and team are
//! taken; `TheThingFactory->findTemplate(name)` (a name that finds none returns at once, after a `DEBUG_ASSERTCRASH` that release
//! compiles out: nothing else happens); `TheAI->pathfinder()->removeObjectFromPathfindMap(me)` ("Remove us first since occupation of cells is
//! apparently not a refcount, but a flag" -- [`CutCall::PathfinderRemoveObjectFromPathfindMap`]); `TheGameLogic->destroyObject(me)`
//! ([`World::destroy_object`], with this module lent to its slot around it, because the destroy sends `onDelete` and `onDestroy` to every module of
//! the object, this one included); `TheThingFactory->newObject(template, team)` ([`World::thing_factory_new_object`]); the new object gets the
//! matrix; `addObjectToPathfindMap(replacement)`; `onBuildComplete()` to each of the new object's modules that has a create interface, in module
//! order ("This magically created thing needs to be considered as Built for Game specific stuff"); and
//! `replacement->getControllingPlayer()->onStructureConstructionComplete(me, replacement, FALSE)` if the new object has a controlling player
//! ([`CutCall::PlayerOnStructureConstructionComplete`]). `setUpgradeExecuted(TRUE)` follows in the caller, on the destroyed object's module.
//! `isSubObjectsUpgrade` is false.
//!
//! The object that replaces is the new object's `NULL` upstream when the thing factory throws; here that is the undefined
//! [`Undefined::NewObjectFailed`]. A `forceRefreshUpgrade` of an upgraded module runs all this again on the destroyed object (`destroyObject`
//! ignores a destroyed object), as upstream does.
//!
//! The lines this module adds to `world.cuts.trace` (only when it is `Some`: the tests) are the order of its steps: `replace destroy`, `replace
//! newobject <id>`, `replace settransform`, `replace buildcomplete <module position>`.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::upgrade_module::{UpgradeModule, UpgradeModuleData};
use crate::logic::modules::upgrade_body::{upgrade_module_call, UpgradeBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;
use crate::common::ini::Undefined;
use crate::common::module::MODULEINTERFACE_CREATE;
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::logic::modules::upgrade_body::lend;
use crate::logic::world::object_cuts::CutCall;

/// `class ReplaceObjectUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct ReplaceObjectUpgradeModuleData {
    /// The `UpgradeModuleData` base, shared with the module's base.
    pub upgrade: Arc<UpgradeModuleData>,
    /// `m_replaceObjectName`.
    pub replace_object_name: Vec<u8>,
}

impl Default for ReplaceObjectUpgradeModuleData {
    /// `ReplaceObjectUpgradeModuleData()`.
    fn default() -> Self {
        ReplaceObjectUpgradeModuleData {
            upgrade: Arc::new(UpgradeModuleData::new()),
            replace_object_name: Vec::new(),
        }
    }
}

/// `UpgradeModuleData::buildFieldParse`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<ReplaceObjectUpgradeModuleData>; 6] = upgrade_module_data_field_parse!(ReplaceObjectUpgradeModuleData, upgrade);

/// `ReplaceObjectUpgradeModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<ReplaceObjectUpgradeModuleData>; 2] = [
    FieldParse::new("ReplaceObject", |ini, d, _| { d.replace_object_name = ini.parse_ascii_string()?; if crate::logic::modules::plant(688) { d.replace_object_name.make_ascii_uppercase(); } Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl ReplaceObjectUpgradeModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s, then `UpgradeModuleData`'s, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, ReplaceObjectUpgradeModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ReplaceObjectUpgradeModuleData> {
        let mut data = ReplaceObjectUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, ReplaceObjectUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class ReplaceObjectUpgrade : public UpgradeModule`.
#[derive(Clone, Debug)]
pub struct ReplaceObjectUpgrade {
    /// The `UpgradeModule` base (its `UpgradeMux` inside).
    base: UpgradeModule,
    /// `getReplaceObjectUpgradeModuleData()`.
    data: Arc<ReplaceObjectUpgradeModuleData>,
}

impl PartialEq for ReplaceObjectUpgrade {
    /// The same data object and the same `m_upgradeExecuted` (upstream compares the data pointer).
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data) && self.base.is_already_upgraded() == other.base.is_already_upgraded()
    }
}

impl ReplaceObjectUpgrade {
    /// `ReplaceObjectUpgrade(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<ReplaceObjectUpgradeModuleData>>) -> IniResult<ReplaceObjectUpgrade> {
        let data = data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let base = UpgradeModule::new(object, Some(data.upgrade.clone()))?;
        Ok(ReplaceObjectUpgrade { base, data })
    }

    /// `getInterfaceMask()`: the `UpgradeModule` one.
    pub const fn get_interface_mask() -> i32 {
        UpgradeModule::get_interface_mask()
    }

    /// The data, as `getReplaceObjectUpgradeModuleData()`.
    pub fn module_data(&self) -> &Arc<ReplaceObjectUpgradeModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        upgrade_module_call(self, world, me, call)
    }
}

impl UpgradeBehavior for ReplaceObjectUpgrade {
    fn base(&self) -> &UpgradeModule {
        &self.base
    }
    fn base_mut(&mut self) -> &mut UpgradeModule {
        &mut self.base
    }
    fn upgrade_implementation(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        let object = me.object;
        let trace = |world: &mut World, line: String| {
            if let Some(t) = &mut world.cuts.trace {
                t.push(line);
            }
        };
        // Matrix3D myMatrix = *me->getTransformMatrix(); Team *myTeam = me->getTeam();
        let Some(my_matrix) = world.objects.find(object).map(|o| *o.thing.transform_matrix()) else { return Ok(()) };
        let my_team = if crate::logic::modules::plant(687) { None } else { world.get_team(object) };
        let Some(replacement_template) = world.thing_factory_find_template(&self.data.replace_object_name) else {
            // DEBUG_ASSERTCRASH(replacementTemplate != NULL, ...); return;
            return Ok(());
        };
        // Remove us first since occupation of cells is apparently not a refcount, but a flag.
        if !crate::logic::modules::plant(681) {
            world.cut(CutCall::PathfinderRemoveObjectFromPathfindMap { object });
        }
        trace(world, "replace destroy".to_string());
        if !crate::logic::modules::plant(682) {
            lend(self, world, me, |w| w.destroy_object(object));
        }
        let replacement = world
            .thing_factory_new_object(replacement_template, my_team, ObjectStatusMaskType::new())
            .map_err(|_| IniError::undefined(Undefined::NewObjectFailed))?;
        trace(world, format!("replace newobject {replacement}"));
        trace(world, "replace settransform".to_string());
        if !crate::logic::modules::plant(683) {
            world.set_transform_matrix(replacement, &my_matrix);
        }
        world.cut(CutCall::PathfinderAddObjectToPathfindMap { object: replacement });
        // Now onCreates were called at the constructor. This magically created thing needs to be considered as Built for Game specific stuff.
        for idx in world.modules_with(replacement, MODULEINTERFACE_CREATE) {
            if crate::logic::modules::plant(684) {
                break;
            }
            trace(world, format!("replace buildcomplete {idx}"));
            world.module_call(replacement, idx, ModuleCall::OnBuildComplete);
        }
        if let Some(player) = world.get_controlling_player(replacement) {
            if !crate::logic::modules::plant(685) {
                world.cut(CutCall::PlayerOnStructureConstructionComplete { player, builder: object, structure: replacement, is_rebuild: crate::logic::modules::plant(686) });
            }
        }
        Ok(())
    }

    fn is_sub_objects_upgrade(&self) -> bool {
        false
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::ReplaceObjectUpgrade(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::ReplaceObjectUpgrade(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for ReplaceObjectUpgrade {
    /// `crc`: `UpgradeModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.base.crc(xfer)
    }
    /// `xfer` (version 1, "Initial version"): the version, then `UpgradeModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.base.xfer(xfer)
    }
    /// `loadPostProcess`: `UpgradeModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.base.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `ReplaceObjectUpgrade`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::ReplaceObjectUpgrade(Arc::new(ReplaceObjectUpgradeModuleData::friend_new_module_data(ini)?)))
}
