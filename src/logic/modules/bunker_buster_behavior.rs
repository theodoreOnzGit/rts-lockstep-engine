// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/BunkerBusterBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BunkerBusterBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `BunkerBusterBehavior`: a bomb or missile that busts a garrisoned building. While it flies, a smart one (with an AI) remembers its AI's
//! first victim and plays `CrashThroughBunkerFX` every `CrashThroughBunkerFXFrequency` frames once it is killing itself; when it dies (with
//! the controlling player's `UpgradeRequired`, if any), the victim's bustable contain module kills its occupants (or harms them with
//! `OccupantDamageWeaponTemplate`'s damage and death types and forces them out), `DetonationFX` plays at the victim (or at the bomb, with no
//! victim) and `ShockwaveWeaponTemplate` is fired there.
//!
//! # Upstream facts kept
//!
//! - The module data is a `BehaviorModuleData` whose parse table extends `UpdateModuleData`'s (the same fields); `CrashThroughBunkerFXFrequency`
//!   is parsed as a duration but defaults to 4 *frames*. `SeismicEffectRadius` and `SeismicEffectMagnitude` are read and used only under
//!   `DO_SEISMIC_SIMULATIONS`, which the shipped build does not define.
//! - `TheGameLogic->getFrame() % m_crashThroughBunkerFXFrequency` with a frequency of 0 divides by zero: `Undefined::ModuloByZero`.
//! - The victim is not saved (`xfer` is the version and the update base), and is read only while it is `INVALID_ID`.
//! - The upgrade, and the occupant damage's source player mask, dereference the controlling player unchecked: `Undefined::NullControllingPlayer`.
//!
//! `p1w13-sonnet`, batch R54, 2026-10-25.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DIE};
use crate::common::object_status_types::OBJECT_STATUS_MISSILE_KILLING_SELF;
use crate::common::upgrade::UpgradeMaskType;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::DamageInfo;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class BunkerBusterBehaviorModuleData : public BehaviorModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct BunkerBusterBehaviorModuleData {
    /// The base the update module is made with (`UpdateModuleData`, which adds nothing).
    pub update: UpdateModuleData,
    /// `m_upgradeRequired`.
    pub upgrade_required: Vec<u8>,
    /// `m_detonationFX`, `m_crashThroughBunkerFX`, by name (`None`: no list).
    pub detonation_fx: Option<Vec<u8>>,
    pub crash_through_bunker_fx: Option<Vec<u8>>,
    /// `m_crashThroughBunkerFXFrequency`.
    pub crash_through_bunker_fx_frequency: u32,
    /// `m_seismicEffectRadius`, `m_seismicEffectMagnitude` (read, not used: no seismic simulations).
    pub seismic_effect_radius: f32,
    pub seismic_effect_magnitude: f32,
    /// `m_shockwaveWeaponTemplate`, `m_occupantDamageWeaponTemplate`, by name (found in the weapon store when used; a name it does not
    /// have, `None` among them, is `NULL`).
    pub shockwave_weapon_template: Option<Vec<u8>>,
    pub occupant_damage_weapon_template: Option<Vec<u8>>,
}

impl Default for BunkerBusterBehaviorModuleData {
    /// `BunkerBusterBehaviorModuleData()`.
    fn default() -> Self {
        BunkerBusterBehaviorModuleData {
            update: UpdateModuleData::new(),
            upgrade_required: Vec::new(),
            detonation_fx: None,
            crash_through_bunker_fx: None,
            crash_through_bunker_fx_frequency: 4,
            seismic_effect_radius: 140.0,
            seismic_effect_magnitude: 6.0,
            shockwave_weapon_template: None,
            occupant_damage_weapon_template: None,
        }
    }
}

/// `INI::parseFXList`: `None` is no list; any other name is looked up when used.
fn parse_fx(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let token = ini.get_next_token(None)?;
    Ok((!token.eq_ignore_ascii_case(b"None")).then_some(token))
}

static FIELD_PARSE: [FieldParse<BunkerBusterBehaviorModuleData>; 9] = [
    FieldParse::new("UpgradeRequired", |i, d, _| { d.upgrade_required = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("DetonationFX", |i, d, _| { d.detonation_fx = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("CrashThroughBunkerFX", |i, d, _| { d.crash_through_bunker_fx = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new(
        "CrashThroughBunkerFXFrequency",
        |i, d, _| {
            d.crash_through_bunker_fx_frequency = i.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new("SeismicEffectRadius", |i, d, _| { d.seismic_effect_radius = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("SeismicEffectMagnitude", |i, d, _| { d.seismic_effect_magnitude = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("ShockwaveWeaponTemplate", |i, d, _| { d.shockwave_weapon_template = Some(i.get_next_token(None)?); Ok(()) }, UserData::None),
    FieldParse::new(
        "OccupantDamageWeaponTemplate",
        |i, d, _| {
            d.occupant_damage_weapon_template = Some(i.get_next_token(None)?);
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl BunkerBusterBehaviorModuleData {
    /// `buildFieldParse(p)`: `UpdateModuleData`'s (the behavior base's), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, BunkerBusterBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<BunkerBusterBehaviorModuleData> {
        let mut data = BunkerBusterBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, BunkerBusterBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `BunkerBusterBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::BunkerBusterBehavior(Arc::new(BunkerBusterBehaviorModuleData::friend_new_module_data(ini)?)))
}

/// `class BunkerBusterBehavior : public UpdateModule, public DieModuleInterface`.
#[derive(Clone, Debug)]
pub struct BunkerBusterBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getBunkerBusterBehaviorModuleData()`.
    data: Arc<BunkerBusterBehaviorModuleData>,
    /// `m_upgradeRequired` (resolved at `onObjectCreated`): its mask.
    upgrade_required: Option<UpgradeMaskType>,
    /// `m_victimID`.
    victim_id: ObjectId,
}

impl PartialEq for BunkerBusterBehavior {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl BunkerBusterBehavior {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_DIE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE
    }

    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<BunkerBusterBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        Ok(BunkerBusterBehavior { update, data, upgrade_required: None, victim_id: INVALID_ID })
    }

    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    pub fn module_data(&self) -> &Arc<BunkerBusterBehaviorModuleData> {
        &self.data
    }

    /// The victim, and whether the upgrade was found.
    pub fn state_words(&self) -> Vec<u32> {
        vec![self.victim_id as u32, u32::from(self.upgrade_required.is_some())]
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

    /// `onObjectCreated()`: "convert module upgrade name to a pointer".
    fn on_object_created(&mut self, world: &mut World) {
        let statics = world.statics.clone();
        let found = statics.upgrade_center.find_upgrade(&mut world.names, &self.data.upgrade_required).ok().flatten();
        self.upgrade_required = found.map(|u| u.upgrade_mask());
    }

    /// `update()`.
    fn run_update(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        let d = self.data.clone();
        // "is this a SMART bomb?"
        let Some(ai) = world.objects.find(us).and_then(|o| o.ai) else { return Ok(()) };
        if self.victim_id == INVALID_ID || crate::logic::modules::plant(2244) {
            let victim = world.module_call(us, ai, ModuleCall::AiGetCurrentVictim).as_id();
            if world.objects.find(victim).is_some() {
                self.victim_id = victim;
            }
        }
        let frequency = d.crash_through_bunker_fx_frequency;
        if frequency == 0 {
            return Err(Undefined::ModuloByZero);
        }
        // "not too much"
        if world.frame() % frequency == if crate::logic::modules::plant(2241) { 0 } else { 1 } {
            let killing_self = world.objects.find(us).is_some_and(|o| o.test_status(OBJECT_STATUS_MISSILE_KILLING_SELF));
            if let Some(fx) = d.crash_through_bunker_fx.clone().filter(|_| killing_self || crate::logic::modules::plant(2242)) {
                // "CrashFX done on the missile/bomb"
                world.emit(ObjectClientEvent::FxListDoFxObj { fx, object_id: us });
            }
        }
        Ok(())
    }

    /// The weapon store's template of `name` (`None`: `NULL`): its name, damage type and death type.
    fn weapon(world: &World, name: &Option<Vec<u8>>) -> Option<(Vec<u8>, i32, i32)> {
        let name = name.as_deref()?;
        world.statics.weapon_store.templates().iter().find(|t| t.name.eq_ignore_ascii_case(name)).map(|t| (t.name.clone(), t.damage_type, t.death_type))
    }

    /// `bustTheBunker()`, from `onDie`.
    fn bust_the_bunker(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        let d = self.data.clone();
        if let Some(mask) = self.upgrade_required {
            let player = world.get_controlling_player(us).ok_or(Undefined::NullControllingPlayer)?;
            let weapon_upgraded = world.cuts.player_completed_upgrade_mask(player).test_for_all(&mask);
            if weapon_upgraded == crate::logic::modules::plant(2243) {
                return Ok(());
            }
        }
        // "here is where we kill everyone inside any targeted garrisoned buildings"
        let target = (self.victim_id != INVALID_ID && world.objects.find(self.victim_id).is_some()).then_some(self.victim_id);
        let mut object_for_fx = us;
        if let Some(target) = target {
            // "Was the pilot aiming at an object?"
            object_for_fx = target;
            if let Some(contain) = world.get_contain(target) {
                // "Was that object something that bunkerbusters bust?"
                if world.module_call(target, contain, ModuleCall::IsBustable).as_bool() || crate::logic::modules::plant(2246) {
                    match Self::weapon(world, &d.occupant_damage_weapon_template) {
                        Some((_, damage_type, death_type)) => {
                            let (damage_type, death_type) = if crate::logic::modules::plant(2249) { (death_type, damage_type) } else { (damage_type, death_type) };
                            let mut info = DamageInfo::default();
                            info.input.damage_type = damage_type;
                            info.input.death_type = death_type;
                            info.input.source_id = us;
                            let player = world.get_controlling_player(us).ok_or(Undefined::NullControllingPlayer)?;
                            info.input.source_player_mask = world.cuts.player_mask(player);
                            info.input.amount = if crate::logic::modules::plant(2247) { 50.0 } else { 100.0 };
                            // "Ouch!"
                            world.module_call(target, contain, ModuleCall::HarmAndForceExitAllContained(info));
                        }
                        None => {
                            world.module_call(target, contain, ModuleCall::KillAllContained);
                        }
                    }
                }
            }
        }
        if let Some(fx) = d.detonation_fx.clone() {
            // "DetonationFX done on the building"
            let object_id = if crate::logic::modules::plant(2245) { us } else { object_for_fx };
            world.emit(ObjectClientEvent::FxListDoFxObj { fx, object_id });
        }
        if let Some((weapon, _, _)) = Self::weapon(world, &d.shockwave_weapon_template) {
            let source = if crate::logic::modules::plant(2248) { us } else { object_for_fx };
            let position = world.objects.find(source).map_or_else(Coord3D::default, |o| o.get_position());
            world.cut(CutCall::WeaponCreateAndFireTemp { weapon, source, position });
        }
        Ok(())
    }
}

impl UpdateBehavior for BunkerBusterBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if let Err(e) = self.run_update(world, me.object) {
            world.cuts.module_errors.push((me, IniError::undefined(e)));
        }
        UPDATE_SLEEP_NONE
    }
    /// The constructor's world half: `setWakeFrame(getObject(), UPDATE_SLEEP_NONE)`.
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
    }
    /// `onObjectCreated`, and the die interface: `onDie` (no die mux).
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnObjectCreated => {
                self.on_object_created(world);
                ModuleReply::Done
            }
            ModuleCall::OnDie(_) => {
                if let Err(e) = self.bust_the_bunker(world, me.object) {
                    world.cuts.module_errors.push((me, IniError::undefined(e)));
                }
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::BunkerBusterBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::BunkerBusterBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for BunkerBusterBehavior {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version and the base.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = BunkerBusterBehaviorModuleData::default();
        assert_eq!(d.crash_through_bunker_fx_frequency, 4);
        assert_eq!((d.seismic_effect_radius, d.seismic_effect_magnitude), (140.0, 6.0));
        let m = BunkerBusterBehavior::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words(), vec![INVALID_ID as u32, 0]);
    }
}
