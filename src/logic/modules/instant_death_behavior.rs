// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/InstantDeathBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/InstantDeathBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `InstantDeathBehavior`: a death with no delay. Unless another module's AI already handled it (the AI is in its dead state), the AI is marked
//! dead; then one FX list, one object creation list and one weapon are drawn at random from the module's lists (a logic random draw for each
//! list that is not empty) and played, made and fired at the object; then the object is destroyed.
//!
//! # Upstream facts kept
//!
//! - A list entry the store does not know is `NULL`, still drawn (the draw is made) and then does nothing; `FX = None` is `NULL` too.
//! - The FX names are kept as written (the FX store is the client's): a name is played as an FX list, as `SlowDeathBehavior`'s are.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::damage_fx::FxListName;
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData,
};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::DamageInfo;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::die_module::{DieModule, DieModuleData, DIE_MODULE_DATA_FIELD_PARSE};
use crate::logic::modules::crate_collide::crate_destroy_object;
use crate::logic::modules::die_body::{die_module_call, DieBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::update_body::UpdateBody;

/// `class InstantDeathBehaviorModuleData : public DieModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InstantDeathBehaviorModuleData {
    /// The `DieModuleData` base (the die mux and the behaviour base).
    pub die: DieModuleData,
    /// `m_fx` (`None`: `NULL`).
    pub fx: Vec<FxListName>,
    /// `m_ocls`, by name.
    pub ocls: Vec<Vec<u8>>,
    /// `m_weapons`, by name.
    pub weapons: Vec<Vec<u8>>,
}

impl InstantDeathBehaviorModuleData {
    /// `InstantDeathBehaviorModuleData()`: empty lists.
    pub fn new() -> Self {
        InstantDeathBehaviorModuleData {
            die: DieModuleData::new(),
            ..Default::default()
        }
    }

    /// `buildFieldParse(p)`: the behaviour base, the four die mux fields (read from the layer-5 table into `die`), then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, InstantDeathBehaviorModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<InstantDeathBehaviorModuleData> {
        let mut data = InstantDeathBehaviorModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                InstantDeathBehaviorModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// Lifts entry `k` of the layer-5 die field table onto `InstantDeathBehaviorModuleData::die`.
fn lift_die_field(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut InstantDeathBehaviorModuleData,
    u: &UserData,
) -> IniResult<()> {
    match DIE_MODULE_DATA_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.die, u),
        None => Ok(()),
    }
}

/// The tokens of a list line: `for (token = ini->getNextToken(); token != NULL; token = ini->getNextTokenOrNull())` (the first is required).
fn tokens(i: &mut Ini<'_>) -> IniResult<Vec<Vec<u8>>> {
    let mut out = vec![i.get_next_token(None)?];
    while let Some(t) = i.get_next_token_or_null(None) {
        out.push(t);
    }
    Ok(out)
}

/// `DieModuleData::buildFieldParse`'s fields, then `InstantDeathBehaviorModuleData`'s (`parseFX`, `parseOCL`, `parseWeapon`: each token appended).
static FIELD_PARSE: [FieldParse<InstantDeathBehaviorModuleData>; 8] = [
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
    // `findFXList(token)`: "could be null! this is OK!"
    FieldParse::new(
        "FX",
        |i, d, _| {
            d.fx.extend(tokens(i)?.into_iter().map(|t| {
                if t.eq_ignore_ascii_case(b"None") {
                    None
                } else {
                    Some(t)
                }
            }));
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "OCL",
        |i, d, _| {
            d.ocls.extend(tokens(i)?);
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "Weapon",
        |i, d, _| {
            d.weapons.extend(tokens(i)?);
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

/// `class InstantDeathBehavior : public DieModule`.
#[derive(Clone, Debug)]
pub struct InstantDeathBehavior {
    die: DieModule,
    /// `getInstantDeathBehaviorModuleData()`.
    data: Arc<InstantDeathBehaviorModuleData>,
}

impl PartialEq for InstantDeathBehavior {
    /// The same object, the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.die.behavior().object() == other.die.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl InstantDeathBehavior {
    /// `InstantDeathBehavior(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(
        object: ObjectId,
        data: Option<Arc<InstantDeathBehaviorModuleData>>,
    ) -> IniResult<InstantDeathBehavior> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let die = DieModule::new(object, Some(Arc::new(data.die.clone())))?;
        Ok(InstantDeathBehavior { die, data })
    }

    /// The data, as `getInstantDeathBehaviorModuleData()`.
    pub fn module_data(&self) -> &Arc<InstantDeathBehaviorModuleData> {
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

    /// `idx = GameLogicRandomValue(0, listSize-1)` for a list that is not empty.
    fn draw(world: &mut World, len: usize) -> Option<usize> {
        (len > 0).then(|| world.random.game_logic_random_value(0, len as i32 - 1) as usize)
    }
}

impl DieBehavior for InstantDeathBehavior {
    fn die(&self) -> &DieModule {
        &self.die
    }
    fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
        if !self.is_die_applicable(world, me, info) && !crate::logic::modules::plant(1801) {
            return;
        }
        let obj = me.object;
        if let Some(ai) = world.objects.find(obj).and_then(|o| o.ai) {
            if world
                .module_call(obj, ai, ModuleCall::AiIsAiInDeadState)
                .as_bool()
                && !crate::logic::modules::plant(1802)
            {
                return;
            }
            world.module_call(obj, ai, ModuleCall::AiMarkAsDead);
        }
        let d = self.data.clone();
        if let Some(idx) = Self::draw(world, d.fx.len()) {
            // `FXList::doFXObj(NULL, ..)` does nothing
            if let Some(fx) = &d.fx[idx] {
                if !crate::logic::modules::plant(1803) {
                    world.emit(ObjectClientEvent::FxListDoFxObj {
                        fx: fx.clone(),
                        object_id: obj,
                    });
                }
            }
        }
        if let Some(idx) = Self::draw(world, d.ocls.len()) {
            // `ObjectCreationList::create(NULL, ..)` makes nothing
            let ocl = &d.ocls[idx];
            if world
                .statics
                .clone()
                .object_creation_list_store
                .find_object_creation_list(&mut world.names, ocl)
                .is_some()
            {
                world.cut(CutCall::OclCreate {
                    object: obj,
                    ocl: ocl.clone(),
                });
            }
        }
        if let Some(idx) = Self::draw(
            world,
            d.weapons.len() + usize::from(crate::logic::modules::plant(1804)),
        ) {
            // `findWeaponTemplate(token)`: whatever the case; "could be null! this is OK!"
            let weapon = d.weapons.get(idx).and_then(|wt| {
                world
                    .statics
                    .weapon_store
                    .templates()
                    .iter()
                    .find(|t| t.name.eq_ignore_ascii_case(wt))
                    .map(|t| t.name.clone())
            });
            if let Some(weapon) = weapon {
                let position = world
                    .objects
                    .find(obj)
                    .map_or_else(Coord3D::default, |o| o.get_position());
                world.cut(CutCall::WeaponCreateAndFireTemp {
                    weapon,
                    source: obj,
                    position,
                });
            }
        }
        if !crate::logic::modules::plant(1805) {
            crate_destroy_object(world, obj);
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::InstantDeathBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::InstantDeathBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for InstantDeathBehavior {
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

/// The factory's `NewModuleDataProc` for `InstantDeathBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::InstantDeathBehavior(Arc::new(
        InstantDeathBehaviorModuleData::friend_new_module_data(ini)?,
    )))
}
