// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/CreateCrateDie.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/CreateCrateDie.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `CreateCrateDie`: the crates a dying object may leave. Nothing for a killer allied to it ("no crate for killing ally at all"). For each
//! `CrateData` name that is a crate template (`TheCrateSystem`), in order: a creation chance (a logic random draw, always made), then the template's
//! optional conditions (the dying object's veterancy level, the killer's kinds, the killer's player's science); then one crate of the template's
//! weighted list is made where there is room (a tight search around the object, ignoring allied and neutral units, then a wide one; anywhere but on the
//! ground the object's own position), facing a random way, on the object's layer, with a crate decal. A crate the maker owns joins its player's
//! default team, and a computer killer's AI is told of the crate.
//!
//! Upstream facts kept: every passing template makes its own crate; the list pick may find nothing (chances summing below 1), and then nothing is
//! made; the owner's controlling player is read unchecked for an owned crate ([`Undefined::NullControllingPlayer`]).

use std::sync::Arc;

use crate::common::base_type::{Coord3D, PI};
use crate::common::game_common::Relationship;
use crate::common::game_type::{INVALID_ID, LAYER_GROUND};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::KindOfMaskType;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::DamageInfo;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::die_module::{DieModule, DieModuleData, DIE_MODULE_DATA_FIELD_PARSE};
use crate::logic::modules::die_body::{die_module_call, DieBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;

/// `FPF_NONE`.
const FPF_NONE: i32 = 0;
/// `FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS`.
const FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS: i32 = 0x8;
use crate::common::game_common::PlayerType;

/// `class CreateCrateDieModuleData : public DieModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CreateCrateDieModuleData {
    /// The `DieModuleData` base (the die mux and the behaviour base).
    pub die: DieModuleData,
    /// `m_crateNameList`: one name per `CrateData` line, in order.
    pub crate_name_list: Vec<Vec<u8>>,
}

impl CreateCrateDieModuleData {
    /// `CreateCrateDieModuleData()`.
    pub fn new() -> Self {
        CreateCrateDieModuleData { die: DieModuleData::new(), crate_name_list: Vec::new() }
    }

    /// `buildFieldParse(p)`: the behaviour base, the four die mux fields (read from the layer-5 table into `die`), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, CreateCrateDieModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<CreateCrateDieModuleData> {
        let mut data = CreateCrateDieModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, CreateCrateDieModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// Lifts entry `k` of the layer-5 die field table onto `CreateCrateDieModuleData::die`.
fn lift_die_field(k: usize, i: &mut Ini<'_>, d: &mut CreateCrateDieModuleData, u: &UserData) -> IniResult<()> {
    match DIE_MODULE_DATA_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.die, u),
        None => Ok(()),
    }
}

/// `DieModuleData::buildFieldParse`'s fields, then `CreateCrateDieModuleData`'s (`parseCrateData`: the next token, appended).
static FIELD_PARSE: [FieldParse<CreateCrateDieModuleData>; 6] = [
    FieldParse::new("DeathTypes", |i, d, u| lift_die_field(0, i, d, u), UserData::None),
    FieldParse::new("VeterancyLevels", |i, d, u| lift_die_field(1, i, d, u), UserData::None),
    FieldParse::new("ExemptStatus", |i, d, u| lift_die_field(2, i, d, u), UserData::None),
    FieldParse::new("RequiredStatus", |i, d, u| lift_die_field(3, i, d, u), UserData::None),
    FieldParse::new("CrateData", |ini, d, _| { let t = ini.get_next_token(None)?; d.crate_name_list.push(t); Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `class CreateCrateDie : public DieModule`.
#[derive(Clone, Debug)]
pub struct CreateCrateDie {
    die: DieModule,
    /// `getCreateCrateDieModuleData()`.
    data: Arc<CreateCrateDieModuleData>,
}

impl PartialEq for CreateCrateDie {
    /// The same object, the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.die.behavior().object() == other.die.behavior().object() && Arc::ptr_eq(&self.data, &other.data)
    }
}

/// What `onDie` needs of a crate template (a copy, so that the world can be borrowed while it is used).
struct CrateFacts {
    creation_chance: f32,
    veterancy_level: Option<crate::common::game_common::VeterancyLevel>,
    killed_by_type_kindof: KindOfMaskType,
    killer_science: Option<crate::common::name_key_generator::NameKeyType>,
    possible_crates: Vec<(Vec<u8>, f32)>,
    is_owned_by_maker: bool,
}

impl CreateCrateDie {
    /// `CreateCrateDie(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<CreateCrateDieModuleData>>) -> IniResult<CreateCrateDie> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let die = DieModule::new(object, Some(Arc::new(data.die.clone())))?;
        Ok(CreateCrateDie { die, data })
    }

    /// The data, as `getCreateCrateDieModuleData()`.
    pub fn module_data(&self) -> &Arc<CreateCrateDieModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        die_module_call(self, world, me, call)
    }

    /// `testKillerType(crate, killer)`: "Must match the whole group of bits set in the KilledBy description".
    fn test_killer_type(world: &World, c: &CrateFacts, killer: ObjectId) -> bool {
        world.objects.find(killer).is_some_and(|k| k.template().is_kind_of_multi(&c.killed_by_type_kindof, &KindOfMaskType::new()))
    }

    /// `testKillerScience(crate, killer)`: the killer's player has the science.
    fn test_killer_science(world: &World, c: &CrateFacts, killer: ObjectId) -> bool {
        if world.objects.find(killer).is_none() {
            return false;
        }
        let Some(p) = world.get_controlling_player(killer) else { return false };
        let science = c.killer_science.expect("tested by the caller");
        world.players.get_nth(p).is_some_and(|pl| pl.sciences.contains(&crate::common::science::ScienceType(science.0 as i32)))
    }

    /// `createCrate(crate)`: the crate of the weighted list, where there is room; `INVALID_ID` for none.
    fn create_crate(world: &mut World, me: ObjectId, c: &CrateFacts) -> ObjectId {
        let (center, layer) = world.objects.find(me).map_or((Coord3D::default(), LAYER_GROUND), |o| (o.get_position(), o.layer));
        // "CreationChance is used for the success of this block, but this block can have any number of potential actual crates"
        let pick = world.random.game_logic_random_value_real(0.0, 1.0);
        let mut running_total = 0.0f32;
        let mut crate_name: Vec<u8> = Vec::new();
        for (name, chance) in &c.possible_crates {
            running_total += *chance;
            if running_total > pick && !crate::logic::modules::plant(1771) {
                crate_name = name.clone();
                break;
            }
        }
        // "At this point, I could very well have a "" for the type, if the Designer didn't make the sum of chances 1"
        let Some(crate_type) = world.thing_factory_find_template(&crate_name) else { return INVALID_ID };
        let mut creation_point = center;
        let spot_found = if layer != LAYER_GROUND || world.contain_partition_find_position_around(&mut creation_point, 0.0, 5.0, FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS) {
            true
        } else {
            // "try a great big scan so we appear on the edge of the large dead thing (building rubble)"
            creation_point = center;
            !crate::logic::modules::plant(1772) && world.contain_partition_find_position_around(&mut creation_point, 0.0, 125.0, FPF_NONE)
        };
        if !spot_found {
            return INVALID_ID;
        }
        let new_crate = world.crate_new_object(crate_type, None);
        world.set_position(new_crate, &creation_point);
        let angle = world.random.game_logic_random_value_real(0.0, 2.0 * PI);
        world.crate_set_orientation(new_crate, angle);
        world.crate_set_layer(new_crate, layer);
        if world.has_drawable(new_crate) {
            world.crate_drawable_crate_decal(new_crate);
        }
        new_crate
    }

    /// The body of `onDie` after the applicability test. An error is upstream's undefined behaviour.
    fn create_crates(&self, world: &mut World, me: ObjectId, info: &DamageInfo) -> Result<(), IniError> {
        let killer = if world.objects.find(info.input.source_id).is_some() { info.input.source_id } else { INVALID_ID };
        if killer != INVALID_ID && world.get_relationship(killer, me) == Relationship::Allies && !crate::logic::modules::plant(1773) {
            return Ok(()); // Nope, no crate for killing ally at all.
        }
        let statics = world.statics.clone();
        for name in &self.data.crate_name_list {
            let Some(t) = statics.crate_system.find_crate_template(name) else { continue };
            let c = CrateFacts {
                creation_chance: t.creation_chance(),
                veterancy_level: t.veterancy_level(),
                killed_by_type_kindof: *t.killed_by_type_kindof(),
                killer_science: t.killer_science(),
                possible_crates: t.possible_crates().iter().map(|e| (e.crate_name.clone(), e.crate_chance)).collect(),
                is_owned_by_maker: t.is_owned_by_maker(),
            };
            // `testCreationChance`: "always test this"
            let test_with = world.random.game_logic_random_value_real(0.0, 1.0);
            // `testWith < testAgainst` (a NaN chance never passes)
            let passes = test_with < c.creation_chance;
            if !passes {
                continue;
            }
            if let Some(level) = c.veterancy_level {
                if world.objects.find(me).map(|o| o.experience.level) != Some(level) && !crate::logic::modules::plant(1774) {
                    continue;
                }
            }
            if c.killed_by_type_kindof.any() && !Self::test_killer_type(world, &c, killer) {
                continue;
            }
            if c.killer_science.is_some() && !Self::test_killer_science(world, &c, killer) && !crate::logic::modules::plant(1775) {
                continue;
            }
            let new_crate = Self::create_crate(world, me, &c);
            if new_crate == INVALID_ID {
                continue;
            }
            // "Design needs to set ownership of crates sometimes"
            if c.is_owned_by_maker {
                let player = world.get_controlling_player(me).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
                let team = world.players.get_nth(player).and_then(|p| p.default_team);
                world.crate_set_team(new_crate, team);
            }
            // "If the killer is a computer controlled player, notify that the crate exists."
            if killer != INVALID_ID {
                let computer = world.get_controlling_player(killer).and_then(|p| world.players.get_nth(p)).is_some_and(|p| p.player_type == PlayerType::Computer);
                if computer && !crate::logic::modules::plant(1776) {
                    if let Some(ai) = world.objects.find(killer).and_then(|o| o.ai) {
                        world.module_call(killer, ai, ModuleCall::AiNotifyCrate(new_crate));
                    }
                }
            }
        }
        Ok(())
    }
}

impl DieBehavior for CreateCrateDie {
    fn die(&self) -> &DieModule {
        &self.die
    }
    fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
        if !self.is_die_applicable(world, me, info) {
            return;
        }
        if let Err(e) = self.create_crates(world, me.object, info) {
            world.cuts.module_errors.push((me, e));
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::CreateCrateDie(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::CreateCrateDie(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for CreateCrateDie {
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

/// The factory's `NewModuleDataProc` for `CreateCrateDie`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::CreateCrateDie(Arc::new(CreateCrateDieModuleData::friend_new_module_data(ini)?)))
}
