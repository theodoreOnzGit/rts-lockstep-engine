// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/Sabotage{CommandCenter,FakeBuilding,InternetCenter,MilitaryFactory,PowerPlant,Superweapon,
//     SupplyCenter,SupplyDropzone}CrateCollide.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/Sabotage{CommandCenter,InternetCenter,MilitaryFactory,PowerPlant,
//     Superweapon,SupplyCenter,SupplyDropzone}CrateCollide.cpp, SabotageFakeBuilding.cpp
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/CrateCollide.cpp (doSabotageFeedbackFX)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The eight sabotage "crates": the collide modules a saboteur (Black Lotus, Hijacker-style infiltrators) carries, which act when it reaches an enemy
//! building of their kind: a command centre or a superweapon has its special powers restarted, a power plant browns its owner out, a factory or an
//! internet centre is hacked (the internet centre's hackers and its owner's spy vision with it), a supply centre or drop zone has cash stolen (the
//! drop zone's drop timer reset too), and a fake building is destroyed.
//!
//! # Design
//!
//! The eight classes differ only in the kind they accept and what they do, so they are one module class with the victim as an enum
//! ([`SabotageVictim`]); the factory makes each upstream name's data with its own victim and field table. Each answers `isSabotageBuildingCrateCollide`
//! TRUE.
//!
//! The calls on the victim's player and object that cascade (`setDisabledUntil`, `attemptDamage`, the money, the energy) run for real unless
//! `cuts.crate_stub_object_calls` is set (the explicit ablation of the crate tests, as `ContainCutTables::stub_object_calls` for the containers),
//! which makes them records. The money and the brown-out are records either way (the player's money and energy machinery is not on the world yet),
//! as `MoneyCrateCollide`'s deposit is.
//!
//! # Upstream facts kept
//!
//! - The saboteur acts only on its AI's goal object (none without an AI).
//! - The victim's and the saboteur's controlling players are read unchecked by the money thieves and the internet centre (undefined for none), and
//!   the internet centre's contain is read unchecked; the fake building's body is read unchecked.
//! - `Player::getMoney()` is never `NULL`: the thieves always act.
//! - `SabotageFakeBuilding` (the file) holds `SabotageFakeBuildingCrateCollide`; its feedback plays nothing (`SAB_VICTIM_FAKE_BUILDING`) and its Eva
//!   line is the sabotage one.
//!
//! `p1w13-sonnet`, batch R13, 2026-10-23.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::disabled_types::DISABLED_HACKED;
use crate::common::game_common::Relationship;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::{
    KindOfType, KINDOF_AIRCRAFT_CARRIER, KINDOF_COMMANDCENTER, KINDOF_FS_AIRFIELD, KINDOF_FS_BARRACKS, KINDOF_FS_FAKE, KINDOF_FS_INTERNET_CENTER, KINDOF_FS_POWER,
    KINDOF_FS_STRATEGY_CENTER, KINDOF_FS_SUPERWEAPON, KINDOF_FS_SUPPLY_CENTER, KINDOF_FS_SUPPLY_DROPZONE, KINDOF_FS_WARFACTORY,
};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_SPECIAL_POWER};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::crate_collide_data_field_parse;
use crate::logic::damage::{DamageInfo, DAMAGE_UNRESISTABLE};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::collide_module::CollideModule;
use crate::logic::modules::crate_collide::{crate_is_valid_to_execute, crate_module_call, CrateBehavior, CrateCollideModuleData};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::{EvaMessage, ObjectClientEvent, ObjectSound};
use crate::logic::world::update_body::UpdateBody;

/// `DEATH_DETONATED` (`DeathType`).
const DEATH_DETONATED: i32 = 10;

/// Which upstream class a sabotage crate is (`CrateCollide::SabotageVictimType` names the same set).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SabotageVictim {
    /// `SabotageCommandCenterCrateCollide`.
    #[default]
    CommandCenter,
    /// `SabotageFakeBuildingCrateCollide`.
    FakeBuilding,
    /// `SabotageInternetCenterCrateCollide`.
    InternetCenter,
    /// `SabotageMilitaryFactoryCrateCollide`.
    MilitaryFactory,
    /// `SabotagePowerPlantCrateCollide`.
    PowerPlant,
    /// `SabotageSuperweaponCrateCollide`.
    Superweapon,
    /// `SabotageSupplyCenterCrateCollide`.
    SupplyCenter,
    /// `SabotageSupplyDropzoneCrateCollide`.
    SupplyDropzone,
}

impl SabotageVictim {
    /// The eight, with their module names.
    pub const ALL: [(SabotageVictim, &'static str); 8] = [
        (SabotageVictim::CommandCenter, "SabotageCommandCenterCrateCollide"),
        (SabotageVictim::FakeBuilding, "SabotageFakeBuildingCrateCollide"),
        (SabotageVictim::InternetCenter, "SabotageInternetCenterCrateCollide"),
        (SabotageVictim::MilitaryFactory, "SabotageMilitaryFactoryCrateCollide"),
        (SabotageVictim::PowerPlant, "SabotagePowerPlantCrateCollide"),
        (SabotageVictim::Superweapon, "SabotageSuperweaponCrateCollide"),
        (SabotageVictim::SupplyCenter, "SabotageSupplyCenterCrateCollide"),
        (SabotageVictim::SupplyDropzone, "SabotageSupplyDropzoneCrateCollide"),
    ];

    /// The victim of a module name.
    pub fn from_name(name: &str) -> Option<SabotageVictim> {
        SabotageVictim::ALL.iter().find(|(_, n)| *n == name).map(|(v, _)| *v)
    }
}

/// `class Sabotage<Victim>CrateCollideModuleData : public CrateCollideModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SabotageCrateCollideModuleData {
    /// The `CrateCollideModuleData` base.
    pub crate_data: CrateCollideModuleData,
    /// Which class.
    pub victim: SabotageVictim,
    /// `m_sabotageFrames` (`SabotageDuration`: factory, internet centre) or `m_powerSabotageFrames` (`SabotagePowerDuration`: power plant).
    pub sabotage_frames: u32,
    /// `m_stealCashAmount` (`StealCashAmount`: supply centre, drop zone).
    pub steal_cash_amount: u32,
}

static BASE_FIELD_PARSE: [FieldParse<SabotageCrateCollideModuleData>; 12] = crate_collide_data_field_parse!(SabotageCrateCollideModuleData, crate_data);

static DURATION_FIELD_PARSE: [FieldParse<SabotageCrateCollideModuleData>; 2] = [
    FieldParse::new("SabotageDuration", |ini, d, _| { d.sabotage_frames = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

static POWER_FIELD_PARSE: [FieldParse<SabotageCrateCollideModuleData>; 2] = [
    FieldParse::new("SabotagePowerDuration", |ini, d, _| { d.sabotage_frames = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

static CASH_FIELD_PARSE: [FieldParse<SabotageCrateCollideModuleData>; 2] = [
    FieldParse::new("StealCashAmount", |ini, d, _| { d.steal_cash_amount = ini.parse_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl SabotageCrateCollideModuleData {
    /// The constructor: the crate base's defaults, every own field 0.
    pub fn new(victim: SabotageVictim) -> Self {
        SabotageCrateCollideModuleData { crate_data: CrateCollideModuleData::new(), victim, sabotage_frames: 0, steal_cash_amount: 0 }
    }

    /// `buildFieldParse(p)`: the module data base, the crate fields, then the class's own (none for the command centre, superweapon and fake
    /// building).
    pub fn build_field_parse(victim: SabotageVictim, p: &mut MultiIniFieldParse<'_, SabotageCrateCollideModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        match victim {
            SabotageVictim::InternetCenter | SabotageVictim::MilitaryFactory => p.add(&DURATION_FIELD_PARSE)?,
            SabotageVictim::PowerPlant => p.add(&POWER_FIELD_PARSE)?,
            SabotageVictim::SupplyCenter | SabotageVictim::SupplyDropzone => p.add(&CASH_FIELD_PARSE)?,
            _ => {}
        }
        Ok(())
    }

    /// `friend_newModuleData(ini)` of the class `victim` names.
    pub fn friend_new_module_data(victim: SabotageVictim, ini: Option<&mut Ini<'_>>) -> IniResult<SabotageCrateCollideModuleData> {
        let mut data = SabotageCrateCollideModuleData::new(victim);
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, |p| SabotageCrateCollideModuleData::build_field_parse(victim, p))?;
        }
        Ok(data)
    }
}

macro_rules! sabotage_new_module_data {
    ($($f:ident => $v:ident),* $(,)?) => {
        $(
            #[doc = concat!("The factory's `NewModuleDataProc` for the `", stringify!($v), "` sabotage crate.")]
            pub fn $f(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
                Ok(ModuleDataKind::SabotageCrateCollide(Arc::new(SabotageCrateCollideModuleData::friend_new_module_data(SabotageVictim::$v, ini)?)))
            }
        )*
    };
}

sabotage_new_module_data!(
    new_command_center_data => CommandCenter,
    new_fake_building_data => FakeBuilding,
    new_internet_center_data => InternetCenter,
    new_military_factory_data => MilitaryFactory,
    new_power_plant_data => PowerPlant,
    new_superweapon_data => Superweapon,
    new_supply_center_data => SupplyCenter,
    new_supply_dropzone_data => SupplyDropzone,
);

/// The factory's `NewModuleDataProc` for the module name `name` (one of [`SabotageVictim::ALL`]).
pub fn new_module_data_proc(name: &str) -> Option<crate::common::module_factory::NewModuleDataProc> {
    Some(match SabotageVictim::from_name(name)? {
        SabotageVictim::CommandCenter => new_command_center_data,
        SabotageVictim::FakeBuilding => new_fake_building_data,
        SabotageVictim::InternetCenter => new_internet_center_data,
        SabotageVictim::MilitaryFactory => new_military_factory_data,
        SabotageVictim::PowerPlant => new_power_plant_data,
        SabotageVictim::Superweapon => new_superweapon_data,
        SabotageVictim::SupplyCenter => new_supply_center_data,
        SabotageVictim::SupplyDropzone => new_supply_dropzone_data,
    })
}

/// `class Sabotage<Victim>CrateCollide : public CrateCollide`.
#[derive(Clone, Debug)]
pub struct SabotageCrateCollide {
    /// The `CollideModule` base.
    collide: CollideModule,
    /// The module data.
    data: Arc<SabotageCrateCollideModuleData>,
}

impl PartialEq for SabotageCrateCollide {
    /// The same object and the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.collide.behavior().object() == other.collide.behavior().object() && Arc::ptr_eq(&self.data, &other.data)
    }
}

/// `MAKE_OBJECT_STATUS_MASK`-free `GameMakeColor(r, g, b, a)`: `(a << 24) | (r << 16) | (g << 8) | b`.
const fn game_make_color(r: u32, g: u32, b: u32, a: u32) -> i32 {
    ((a << 24) | (r << 16) | (g << 8) | b) as i32
}

impl SabotageCrateCollide {
    /// The constructor: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<SabotageCrateCollideModuleData>>) -> IniResult<SabotageCrateCollide> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let collide = CollideModule::new(object, Some(Arc::new(data.crate_data.collide.clone())))?;
        Ok(SabotageCrateCollide { collide, data })
    }

    /// `CollideModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        CollideModule::get_interface_mask()
    }

    /// The data.
    pub fn module_data(&self) -> &Arc<SabotageCrateCollideModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        crate_module_call(self, world, me, call)
    }

    /// The kinds the class accepts (any of them), and the one it refuses.
    fn kinds(&self) -> (&'static [KindOfType], Option<KindOfType>) {
        match self.data.victim {
            SabotageVictim::CommandCenter => (&[KINDOF_COMMANDCENTER], None),
            SabotageVictim::FakeBuilding => (&[KINDOF_FS_FAKE], None),
            SabotageVictim::InternetCenter => (&[KINDOF_FS_INTERNET_CENTER], None),
            // "Aircraft carriers are considered war factories, but cannot be sabotaged"
            SabotageVictim::MilitaryFactory => (&[KINDOF_FS_BARRACKS, KINDOF_FS_WARFACTORY, KINDOF_FS_AIRFIELD], Some(KINDOF_AIRCRAFT_CARRIER)),
            SabotageVictim::PowerPlant => (&[KINDOF_FS_POWER], None),
            SabotageVictim::Superweapon => (&[KINDOF_FS_SUPERWEAPON, KINDOF_FS_STRATEGY_CENTER], None),
            SabotageVictim::SupplyCenter => (&[KINDOF_FS_SUPPLY_CENTER], None),
            SabotageVictim::SupplyDropzone => (&[KINDOF_FS_SUPPLY_DROPZONE], None),
        }
    }

    /// `CrateCollide::doSabotageFeedbackFX(other, type)`: the victim's sound at its place, and its drawable flashes.
    fn do_sabotage_feedback_fx(&self, world: &mut World, other: ObjectId) {
        let sound = match self.data.victim {
            // "THIS NEEDS NO ADD'L FEEDBACK"
            SabotageVictim::FakeBuilding => return,
            SabotageVictim::CommandCenter | SabotageVictim::Superweapon => ObjectSound::SabotageResetTimerBuilding,
            SabotageVictim::SupplyDropzone | SabotageVictim::SupplyCenter => ObjectSound::MoneyWithdraw,
            _ => {
                if crate::logic::modules::plant(1641) {
                    ObjectSound::MoneyWithdraw
                } else {
                    ObjectSound::SabotageShutDownBuilding
                }
            }
        };
        let position = world.objects.find(other).map(|o| o.get_position());
        world.emit(ObjectClientEvent::Sound { sound, object_id: crate::common::game_type::INVALID_ID, position });
        if world.has_drawable(other) && !crate::logic::modules::plant(1642) {
            world.emit(ObjectClientEvent::DrawableFlashAsSelected { object_id: other });
        }
    }

    /// `TheEva->setShouldPlay(message)` when `other->isLocallyControlled()`: its player is the local one, which holds for an unowned victim when
    /// there is no local player (`NULL == NULL`).
    fn eva_if_local(world: &mut World, other: ObjectId, message: EvaMessage) {
        if world.is_locally_controlled(other) {
            world.emit(ObjectClientEvent::Eva { message });
        }
    }

    /// `obj->setDisabledUntil(type, frame)`: real, or a record under the crate tests' stub.
    fn set_disabled_until(world: &mut World, id: ObjectId, frame: u32) {
        if world.cuts.crate_stub_object_calls {
            world.cut(CutCall::ObjectSetDisabledUntil { object: id, disabled: DISABLED_HACKED, frame });
        } else {
            world.set_disabled_until(id, DISABLED_HACKED, frame);
        }
    }

    /// Every special power module of the victim restarts its recharge.
    fn restart_special_powers(world: &mut World, other: ObjectId) {
        for idx in world.modules_with(other, MODULEINTERFACE_SPECIAL_POWER) {
            if crate::logic::modules::plant(1643) {
                break;
            }
            world.module_call(other, idx, ModuleCall::StartPowerRecharge);
        }
    }

    /// The supply centre's and drop zone's theft: up to `StealCashAmount` of the victim's player's money to the saboteur's.
    fn steal_cash(&self, world: &mut World, me: ModuleHandle, other: ObjectId) -> Result<(), IniError> {
        let us = me.object;
        let target = world.get_controlling_player(other).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        let thief = world.get_controlling_player(us).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        let available = world.players.get_nth(target).map_or(0, |p| p.money.count_money());
        let desired = self.data.steal_cash_amount;
        let cash = if crate::logic::modules::plant(1644) { desired } else { desired.min(available) };
        if cash > 0 {
            world.cut(CutCall::PlayerMoneyWithdraw { player: target, amount: cash });
            world.cut(CutCall::PlayerMoneyDeposit { player: thief, amount: cash });
            world.cut(CutCall::PlayerScoreAddMoneyEarned { player: thief, amount: cash as i32 });
            Self::eva_if_local(world, other, EvaMessage::CashStolen);
            // "add a little z to make it show up above the unit"
            let mut pos = world.objects.find(us).map_or(Coord3D::default(), |o| o.get_position());
            pos.z += 20.0;
            world.cut(CutCall::InGameUiAddFloatingTextNamed { text: b"GUI:AddCash".to_vec(), amount: cash as i32, position: pos, color: game_make_color(0, 255, 0, 255) });
            let mut pos = world.objects.find(other).map_or(Coord3D::default(), |o| o.get_position());
            pos.z += if crate::logic::modules::plant(1645) { 20.0 } else { 30.0 };
            world.cut(CutCall::InGameUiAddFloatingTextNamed { text: b"GUI:LoseCash".to_vec(), amount: cash as i32, position: pos, color: game_make_color(255, 0, 0, 255) });
        } else {
            Self::eva_if_local(world, other, EvaMessage::BuildingSabotaged);
        }
        Ok(())
    }
}

impl CrateBehavior for SabotageCrateCollide {
    fn collide(&self) -> &CollideModule {
        &self.collide
    }
    fn crate_data(&self) -> &CrateCollideModuleData {
        &self.data.crate_data
    }

    /// `isValidToExecute(other)`: the base's, a living victim of the class's kind, an enemy of the saboteur.
    fn is_valid_to_execute(&self, world: &mut World, me: ModuleHandle, other: ObjectId) -> Result<bool, IniError> {
        if !crate_is_valid_to_execute(&self.data.crate_data, world, me, other)? {
            return Ok(false);
        }
        let Some(o) = world.objects.find(other) else { return Ok(false) };
        if o.is_effectively_dead() && !crate::logic::modules::plant(1646) {
            return Ok(false);
        }
        let (accepted, refused) = self.kinds();
        if refused.is_some_and(|k| o.is_kind_of(k)) && !crate::logic::modules::plant(1647) {
            return Ok(false);
        }
        if !accepted.iter().any(|&k| o.is_kind_of(k)) {
            return Ok(false);
        }
        let r = world.get_relationship(me.object, other);
        if (r != Relationship::Enemies) != crate::logic::modules::plant(1648) {
            return Ok(false);
        }
        Ok(true)
    }

    /// `executeCrateBehavior(other)`.
    fn execute_crate_behavior(&mut self, world: &mut World, me: ModuleHandle, other: ObjectId) -> Result<bool, IniError> {
        let us = me.object;
        // "only the object we are targeting"
        if let Some(ai) = world.objects.find(us).and_then(|o| o.ai) {
            if world.module_call(us, ai, ModuleCall::AiGetGoalObject).as_id() != other && !crate::logic::modules::plant(1649) {
                return Ok(false);
            }
        }
        world.emit(ObjectClientEvent::Radar { call: crate::logic::world::object_events::RadarCall::TryInfiltrationEvent, object_id: other, position: None });
        self.do_sabotage_feedback_fx(world, other);
        let frame = world.frame().wrapping_add(self.data.sabotage_frames);
        match self.data.victim {
            SabotageVictim::CommandCenter | SabotageVictim::Superweapon => {
                Self::eva_if_local(world, other, EvaMessage::BuildingSabotaged);
                Self::restart_special_powers(world, other);
            }
            SabotageVictim::PowerPlant => {
                Self::eva_if_local(world, other, EvaMessage::BuildingSabotaged);
                if let Some(player) = world.get_controlling_player(other) {
                    let frame = if crate::logic::modules::plant(1650) { world.frame() } else { frame };
                    world.cut(CutCall::PlayerSetPowerSabotagedTillFrame { player, frame });
                    world.cut(CutCall::PlayerOnPowerBrownOutChange { player, brown_out: true });
                }
            }
            SabotageVictim::MilitaryFactory => {
                Self::eva_if_local(world, other, EvaMessage::BuildingSabotaged);
                Self::set_disabled_until(world, other, frame);
            }
            SabotageVictim::InternetCenter => {
                Self::eva_if_local(world, other, EvaMessage::BuildingSabotaged);
                let player = world.get_controlling_player(other).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
                // `iterateObjects(disableInternetCenterSpyVision)`: the player's internet centres' spy vision
                for obj in world.player_iterate_objects(player) {
                    if world.objects.find(obj).is_some_and(|o| o.is_kind_of(KINDOF_FS_INTERNET_CENTER)) && !crate::logic::modules::plant(1651) {
                        for idx in world.modules_with_spy_vision(obj) {
                            world.module_call(obj, idx, ModuleCall::SetSpyVisionDisabledUntilFrame { frame });
                        }
                    }
                }
                Self::set_disabled_until(world, other, frame);
                // `getContain()->iterateContained(disableHacker)`: read unchecked
                let contain = world.get_contain(other).ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
                let hackers = match world.module_call(other, contain, ModuleCall::GetContainedItems) {
                    ModuleReply::Ids(v) => v,
                    _ => Vec::new(),
                };
                if !crate::logic::modules::plant(1652) {
                    for hacker in hackers {
                        Self::set_disabled_until(world, hacker, frame);
                    }
                }
            }
            SabotageVictim::SupplyCenter => self.steal_cash(world, me, other)?,
            SabotageVictim::SupplyDropzone => {
                // `findUpdateModule(NAMEKEY("OCLUpdate"))->resetTimer()`
                if let Some(idx) = world.find_module(other, b"OCLUpdate").filter(|_| !crate::logic::modules::plant(1653)) {
                    world.module_call(other, idx, ModuleCall::OclUpdateResetTimer);
                }
                self.steal_cash(world, me, other)?;
            }
            SabotageVictim::FakeBuilding => {
                Self::eva_if_local(world, other, EvaMessage::BuildingSabotaged);
                if world.get_controlling_player(other).is_some() {
                    let body = world.get_body_module(other).ok_or_else(|| IniError::undefined(Undefined::NullBodyModule))?;
                    let mut info = DamageInfo::default();
                    info.input.damage_type = DAMAGE_UNRESISTABLE;
                    info.input.death_type = DEATH_DETONATED;
                    info.input.source_id = us;
                    info.input.amount = world.module_call(other, body, ModuleCall::GetMaxHealth).as_real(0.0);
                    if crate::logic::modules::plant(1654) {
                        info.input.amount /= 2.0;
                    }
                    if world.cuts.crate_stub_object_calls {
                        world.cut(CutCall::ObjectAttemptDamage { object: other, info });
                    } else {
                        world.attempt_damage(other, &mut info);
                    }
                }
            }
        }
        Ok(true)
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::SabotageCrateCollide(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SabotageCrateCollide(m) => Some(m),
            _ => None,
        }
    }

    /// `isSabotageBuildingCrateCollide()`: TRUE.
    fn other_call(&mut self, _world: &mut World, _me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::IsSabotageBuildingCrateCollide => ModuleReply::Bool(!crate::logic::modules::plant(1655)),
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for SabotageCrateCollide {
    /// `crc`: `CrateCollide::crc`, which is `CollideModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.collide.crc(xfer)
    }
    /// `xfer` (version 1): the version, `CrateCollide::xfer` (its own version, then `CollideModule::xfer`).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        let mut crate_version = current;
        xfer.xfer_version(&mut crate_version, current)?;
        self.collide.xfer(xfer)
    }
    /// `loadPostProcess`: `CollideModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.collide.load_post_process();
    }
}
