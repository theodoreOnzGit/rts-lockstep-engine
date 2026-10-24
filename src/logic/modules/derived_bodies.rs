// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/{StructureBody,HighlanderBody,ImmortalBody,UndeadBody,HiveStructureBody}.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Body/{StructureBody,HighlanderBody,ImmortalBody,UndeadBody,HiveStructureBody}.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The bodies derived from `ActiveBody` (`UndeadBody` and `HiveStructureBody` below the first three, with their own module data). The first three change nothing but one small thing: `HighlanderBody` ("takes damage according to armor, but can't die from
//! normal damage"), `ImmortalBody` ("just like Active Body, but won't let health drop below 1") and `StructureBody` (an `ActiveBody` that remembers the object that
//! built it). Each holds its base by value and implements [`ActiveBodyVirtuals`], so the base's functions reach the override as the vtable sends them there.
//!
//! The module data of all three is the data of `ActiveBody`: `HighlanderBody` and `ImmortalBody` inherit `ActiveBody::friend_newModuleData` (they declare
//! `MAKE_STANDARD_MODULE_MACRO`, not the `_WITH_MODULE_DATA` form), and `StructureBodyModuleData` adds no field to `ActiveBodyModuleData` (its field parse is the
//! base's). The factory therefore makes the same [`ModuleDataKind::ActiveBody`] for the four names.
//!
//! # Upstream facts kept
//!
//! - `HighlanderBody::attemptDamage` writes the limited amount into the caller's `DamageInfo` (`damageInfo->in.m_amount = min(..)`), so the caller sees the
//!   limited amount afterwards. The `min` is the macro `(a < b) ? a : b`: a NaN amount is kept as it is.
//! - `HighlanderBody` limits the amount by `getHealth() - 1`: a body with less than one health point gets a negative amount, which `ActiveBody` takes as healing.
//! - `ImmortalBody::internalChangeHealth` limits the delta by `-getHealth() + 1` before the base sees it; its `DEBUG_ASSERTCRASH` ("Immortal objects should never get
//!   marked as dead!") is compiled out of a release build and is not translated.
//! - `StructureBody::setConstructorObject(NULL)` leaves the stored id as it was. Nothing in the game calls it (the whole tree was searched on 2026-10-22); it is
//!   reachable here as `ModuleCall::SetConstructorObject` and tested through the differential test.
//!
//! - `UndeadBody::attemptDamage` limits the amount to `getHealth() - 1` (written into the caller's `DamageInfo`, `min` as the macro, so a NaN amount is kept: but a NaN
//!   amount never passes the `>=` test either) and starts the second life AFTER the base took the damage. `startSecondLife` asks every behaviour of the object
//!   (`getBehaviorModules()`, the body included) for its `SlowDeathBehaviorInterface`: [`ModuleCall::IsDieApplicable`] answers `Unhandled` for a module that is none, and the
//!   probability modifiers are summed as `Int` (the `DEBUG_ASSERTCRASH(total > 0)` is compiled out of a release build, and `GameLogicRandomValue(1, total)` with `total < 1`
//!   is defined: `hi - lo + 1` wraps as unsigned, and a zero range returns `hi` without drawing). The roll walks the same behaviours again and the first whose running
//!   total reaches 0 begins its slow death; with no applicable behaviour nothing begins, and the object is left with its second life and no death.
//!   Upstream's second loop names the first loop's `update` (Visual C++ 6 let a `for` variable outlive its loop): the differential build declares its own, by one mechanical edit.
//! - `HiveStructureBody::attemptDamage` passes a damage type of `PropagateDamageTypesToSlavesWhenExisting` to the closest slave (the spawn behaviour) or, if there is no
//!   spawn behaviour, the closest rider of the contain module, measured from the SHOOTER's position; without a shooter it falls through to the structure body's damage. A
//!   hive with neither module reaches a `DEBUG_CRASH`, compiled out of a release build: the damage is taken by the hive. The swallow set eats the damage only when there is
//!   a shooter and no slave (zeroing the dealt and clipped damage and setting `noEffect`).
//!
//! `p1w13-sonnet`, 2026-10-22.

use std::sync::Arc;

use crate::common::base_type::{max, min, Coord3D};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::ini::{FieldParse, Ini, IniResult, MultiIniFieldParse, UserData};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::armor_set::ARMORSET_SECOND_LIFE;
use crate::logic::damage::{
    is_health_damaging_damage, DamageInfo, DamageTypeFlags, DAMAGE_TYPE_FLAGS_NONE,
    DAMAGE_UNRESISTABLE,
};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::body_module::FULLY_HEAL;
use crate::logic::modules::active_body::{
    self, ActiveBody, ActiveBodyModuleData, ActiveBodyVirtuals,
};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;

/// Declares a class derived from `ActiveBody` that holds its base in `ab` and has the same `Snapshot` shape (`version 1`, then the base, then the class's own fields).
macro_rules! derived_body {
    ($(#[$doc:meta])* $name:ident, $variant:ident, $extra:ty, $init:expr) => {
        $(#[$doc])*
        #[derive(Clone, Debug)]
        pub struct $name {
            ab: ActiveBody,
            #[allow(dead_code)] // `()` for the two classes with no field of their own
            extra: $extra,
        }

        impl PartialEq for $name {
            /// The base's identity (see [`ActiveBody`]'s `PartialEq`): the lent copy of [`active_body::out`] is always the state afterwards.
            fn eq(&self, other: &Self) -> bool {
                self.ab == other.ab
            }
        }

        impl $name {
            /// `getInterfaceMask()`: the base's.
            pub const fn get_interface_mask() -> i32 {
                ActiveBody::get_interface_mask()
            }

            /// The constructor's first half (see [`ActiveBody::unconstructed`]).
            pub fn new(object: ObjectId, module_data: Option<Arc<ActiveBodyModuleData>>) -> IniResult<$name> {
                Ok($name { ab: ActiveBody::new(object, module_data)?, extra: $init })
            }

            /// The constructor's second half, run by the world.
            pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
                self.ab.world_construct(world, me);
            }

            /// The base.
            pub fn active_body(&self) -> &ActiveBody {
                &self.ab
            }

            /// The module's answer to one of `Object`'s calls.
            pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
                match self.derived_call(&call) {
                    Some(r) => r,
                    None => active_body::body_module_call(self, world, me, call),
                }
            }
        }

        impl ActiveBodyVirtuals for $name {
            fn ab(&self) -> &ActiveBody {
                &self.ab
            }
            fn ab_mut(&mut self) -> &mut ActiveBody {
                &mut self.ab
            }
            fn into_body(self) -> UpdateBody {
                UpdateBody::$variant(self)
            }
            fn from_body(b: UpdateBody) -> Option<Self> {
                match b {
                    UpdateBody::$variant(m) => Some(m),
                    _ => None,
                }
            }
            fn attempt_damage(&mut self, world: &mut World, me: ModuleHandle, info: &mut DamageInfo) {
                self.attempt_damage_override(world, me, info);
            }
            fn internal_change_health(&mut self, world: &mut World, me: ModuleHandle, delta: f32) {
                self.internal_change_health_override(world, me, delta);
            }
        }

        impl Snapshot for $name {
            /// `crc`: the base's.
            fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
                self.ab.crc(xfer)
            }

            /// `xfer` (version 1, "Initial version"): the version, the base, then the class's own fields.
            fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
                let current: XferVersion = 1;
                let mut version = current;
                xfer.xfer_version(&mut version, current)?;
                self.ab.xfer(xfer)?;
                self.xfer_extra(xfer)
            }

            /// `loadPostProcess`: the base's.
            fn load_post_process(&mut self) {
                self.ab.load_post_process();
            }
        }
    };
}

derived_body!(
    /// `class HighlanderBody : public ActiveBody`.
    HighlanderBody,
    HighlanderBody,
    (),
    ()
);
derived_body!(
    /// `class ImmortalBody : public ActiveBody`.
    ImmortalBody,
    ImmortalBody,
    (),
    ()
);
derived_body!(
    /// `class StructureBody : public ActiveBody`.
    StructureBody,
    StructureBody,
    // `StructureBody::m_constructorObjectID`
    ObjectId,
    INVALID_ID
);

impl HighlanderBody {
    fn derived_call(&self, _call: &ModuleCall) -> Option<ModuleReply> {
        None
    }

    fn xfer_extra(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `HighlanderBody::attemptDamage`: "bind to one hitpoint remaining afterwards, unless it is Unresistable damage".
    fn attempt_damage_override(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        info: &mut DamageInfo,
    ) {
        if (info.input.damage_type != DAMAGE_UNRESISTABLE) != crate::logic::modules::plant(1261) {
            info.input.amount = if crate::logic::modules::plant(1262) {
                min(info.input.amount, self.ab.get_health())
            } else if crate::logic::modules::plant(1263) {
                max(info.input.amount, self.ab.get_health() - 1.0)
            } else {
                min(info.input.amount, self.ab.get_health() - 1.0)
            };
        }
        active_body::attempt_damage(self, world, me, info);
    }

    fn internal_change_health_override(&mut self, world: &mut World, me: ModuleHandle, delta: f32) {
        active_body::internal_change_health(self, world, me, delta);
    }
}

impl ImmortalBody {
    fn derived_call(&self, _call: &ModuleCall) -> Option<ModuleReply> {
        None
    }

    fn xfer_extra(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    fn attempt_damage_override(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        info: &mut DamageInfo,
    ) {
        active_body::attempt_damage(self, world, me, info);
    }

    /// `ImmortalBody::internalChangeHealth`: "don't let anything change us to below one hit point", then the base.
    fn internal_change_health_override(&mut self, world: &mut World, me: ModuleHandle, delta: f32) {
        let delta = if crate::logic::modules::plant(1264) {
            max(delta, -self.ab.get_health())
        } else if crate::logic::modules::plant(1265) {
            min(delta, -self.ab.get_health() + 1.0)
        } else {
            max(delta, -self.ab.get_health() + 1.0)
        };
        active_body::internal_change_health(self, world, me, delta);
    }
}

impl StructureBody {
    /// `setConstructorObject(obj)`: "if( obj ) m_constructorObjectID = obj->getID();" (`INVALID_ID` stands for a null `obj`, which changes nothing).
    pub fn set_constructor_object(&mut self, obj: ObjectId) {
        if obj != INVALID_ID || crate::logic::modules::plant(1266) {
            self.extra = obj;
        }
    }

    /// `getConstructorObjectID()`.
    pub fn get_constructor_object_id(&self) -> ObjectId {
        self.extra
    }

    fn derived_call(&mut self, call: &ModuleCall) -> Option<ModuleReply> {
        match call {
            ModuleCall::SetConstructorObject(obj) => {
                self.set_constructor_object(*obj);
                Some(ModuleReply::Done)
            }
            ModuleCall::GetConstructorObjectId => {
                Some(ModuleReply::Id(self.get_constructor_object_id()))
            }
            _ => None,
        }
    }

    /// The constructor object id (`xferObjectID`).
    fn xfer_extra(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        if crate::logic::modules::plant(1267) {
            return Ok(());
        }
        xfer.xfer_object_id(&mut self.extra)
    }

    fn attempt_damage_override(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        info: &mut DamageInfo,
    ) {
        active_body::attempt_damage(self, world, me, info);
    }

    fn internal_change_health_override(&mut self, world: &mut World, me: ModuleHandle, delta: f32) {
        active_body::internal_change_health(self, world, me, delta);
    }
}

// ---- UndeadBody -----------------------------------------------------------------------------------------------------------------------------------------------

/// `class UndeadBodyModuleData : public ActiveBodyModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct UndeadBodyModuleData {
    /// The `ActiveBodyModuleData` base.
    pub active: ActiveBodyModuleData,
    /// `m_secondLifeMaxHealth` ("SecondLifeMaxHealth"; `1` unless the INI says).
    pub second_life_max_health: f32,
}

impl Default for UndeadBodyModuleData {
    /// `UndeadBodyModuleData()`: "m_secondLifeMaxHealth = 1".
    fn default() -> Self {
        UndeadBodyModuleData {
            active: ActiveBodyModuleData::default(),
            second_life_max_health: 1.0,
        }
    }
}

/// Lifts entry `k` of `ActiveBodyModuleData`'s table onto the `active` base of a derived class's data.
fn lift_active_field(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut ActiveBodyModuleData,
    u: &UserData,
) -> IniResult<()> {
    match active_body::FIELD_PARSE[k].parse {
        Some(f) => f(i, d, u),
        None => Ok(()),
    }
}

/// `ActiveBodyModuleData::buildFieldParse`'s own table, on the `active` base of `UndeadBodyModuleData`.
static UNDEAD_BASE_FIELD_PARSE: [FieldParse<UndeadBodyModuleData>; 6] = [
    FieldParse::new(
        "MaxHealth",
        |i, d, u| lift_active_field(0, i, &mut d.active, u),
        UserData::None,
    ),
    FieldParse::new(
        "InitialHealth",
        |i, d, u| lift_active_field(1, i, &mut d.active, u),
        UserData::None,
    ),
    FieldParse::new(
        "SubdualDamageCap",
        |i, d, u| lift_active_field(2, i, &mut d.active, u),
        UserData::None,
    ),
    FieldParse::new(
        "SubdualDamageHealRate",
        |i, d, u| lift_active_field(3, i, &mut d.active, u),
        UserData::None,
    ),
    FieldParse::new(
        "SubdualDamageHealAmount",
        |i, d, u| lift_active_field(4, i, &mut d.active, u),
        UserData::None,
    ),
    FieldParse::end(),
];

/// `UndeadBodyModuleData::buildFieldParse`'s own table.
static UNDEAD_FIELD_PARSE: [FieldParse<UndeadBodyModuleData>; 2] = [
    FieldParse::new(
        "SecondLifeMaxHealth",
        |i, d, _| {
            d.second_life_max_health = i.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl UndeadBodyModuleData {
    /// `buildFieldParse(p)`: `ActiveBodyModuleData::buildFieldParse` (the module base, then the active table), then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, UndeadBodyModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UNDEAD_BASE_FIELD_PARSE)?;
        p.add(&UNDEAD_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<UndeadBodyModuleData> {
        let mut data = UndeadBodyModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, UndeadBodyModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `UndeadBody`.
pub fn new_undead_module_data(
    ini: Option<&mut Ini<'_>>,
) -> IniResult<crate::common::module::ModuleDataKind> {
    Ok(crate::common::module::ModuleDataKind::UndeadBody(Arc::new(
        UndeadBodyModuleData::friend_new_module_data(ini)?,
    )))
}

/// `class UndeadBody : public ActiveBody`: "a body that gets a second life": the first fatal hit leaves it with one health point and starts a slow death, then
/// its health is refilled to `SecondLifeMaxHealth` and its armor changes.
#[derive(Clone, Debug)]
pub struct UndeadBody {
    ab: ActiveBody,
    /// `getUndeadBodyModuleData()`.
    data: Arc<UndeadBodyModuleData>,
    /// `m_isSecondLife`.
    is_second_life: bool,
}

impl PartialEq for UndeadBody {
    /// The base's identity (see [`ActiveBody`]'s `PartialEq`): the lent copy of [`active_body::out`] is always the state afterwards.
    fn eq(&self, other: &Self) -> bool {
        self.ab == other.ab
    }
}

impl UndeadBody {
    /// `getInterfaceMask()`: the base's.
    pub const fn get_interface_mask() -> i32 {
        ActiveBody::get_interface_mask()
    }

    /// The constructor's first half (see [`ActiveBody::unconstructed`]): `m_isSecondLife = FALSE`.
    pub fn new(
        object: ObjectId,
        module_data: Option<Arc<UndeadBodyModuleData>>,
    ) -> IniResult<UndeadBody> {
        let base = module_data.as_ref().map(|d| Arc::new(d.active.clone()));
        let ab = ActiveBody::new(object, base)?;
        Ok(UndeadBody {
            ab,
            data: module_data.unwrap_or_default(),
            is_second_life: false,
        })
    }

    /// The constructor's second half, run by the world.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.ab.world_construct(world, me);
    }

    /// The base.
    pub fn active_body(&self) -> &ActiveBody {
        &self.ab
    }

    /// `m_isSecondLife`.
    pub fn is_second_life(&self) -> bool {
        self.is_second_life
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        active_body::body_module_call(self, world, me, call)
    }

    /// `UndeadBody::attemptDamage`: "if we are on our first life, see if this damage will kill us. If it will, bind it to one hitpoint remaining, then go ahead
    /// and take it."
    fn attempt_damage_override(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        info: &mut DamageInfo,
    ) {
        use crate::logic::modules::plant;
        let mut should_start_second_life = false;
        let unresistable = if plant(1331) {
            info.input.damage_type == DAMAGE_UNRESISTABLE
        } else {
            info.input.damage_type != DAMAGE_UNRESISTABLE
        };
        let first_life = plant(1332) || !self.is_second_life;
        let lethal = if plant(1333) {
            info.input.amount > self.ab.get_health()
        } else {
            info.input.amount >= self.ab.get_health()
        };
        if unresistable
            && first_life
            && lethal
            && (plant(1334) || is_health_damaging_damage(info.input.damage_type))
        {
            info.input.amount = min(
                info.input.amount,
                self.ab.get_health() - if plant(1335) { 0.0 } else { 1.0 },
            );
            should_start_second_life = true;
        }

        if plant(1336) && should_start_second_life {
            // the planted order: the second life starts before the damage is taken
            self.start_second_life(world, me, info);
            should_start_second_life = false;
        }
        active_body::attempt_damage(self, world, me, info);

        // after we take it (which allows for damaging special effects), we will do our modifications to the body module
        if should_start_second_life {
            self.start_second_life(world, me, info);
        }
    }

    /// `UndeadBody::startSecondLife`.
    fn start_second_life(&mut self, world: &mut World, me: ModuleHandle, info: &mut DamageInfo) {
        use crate::logic::modules::plant;
        // flag module as no longer intercepting damage
        self.is_second_life = !plant(1337);

        // modify ActiveBody's max health and initial health
        let max = if plant(1338) {
            self.data.active.max_health
        } else {
            self.data.second_life_max_health
        };
        let change = if plant(1339) {
            crate::logic::module::body_module::PRESERVE_RATIO
        } else {
            FULLY_HEAL
        };
        active_body::set_max_health(self, world, me, max, change);

        // set armor set flag to use second life armor
        if !plant(1340) {
            self.ab.set_armor_set_flag(ARMORSET_SECOND_LIFE);
        }

        // fire the slow death module. The fact that this is not the result of an onDie will cause the special behavior
        let obj = me.object;
        let modules = world.all_modules(obj);
        let mut total: i32 = 0;
        for &idx in &modules {
            let i = info.clone();
            let applicable = active_body::out(self, world, me, |w| {
                w.module_call(obj, idx, ModuleCall::IsDieApplicable(i))
            })
            .as_bool();
            if applicable || plant(1341) {
                let i = info.clone();
                if let ModuleReply::Int(m) = active_body::out(self, world, me, |w| {
                    w.module_call(obj, idx, ModuleCall::GetProbabilityModifier(i))
                }) {
                    total += m;
                }
            }
        }

        // this returns a value from 1...total, inclusive
        let mut roll = world
            .random
            .game_logic_random_value(if plant(1342) { 0 } else { 1 }, total);

        for &idx in &modules {
            let i = info.clone();
            let applicable = active_body::out(self, world, me, |w| {
                w.module_call(obj, idx, ModuleCall::IsDieApplicable(i))
            })
            .as_bool();
            if applicable {
                let i = info.clone();
                if let ModuleReply::Int(m) = active_body::out(self, world, me, |w| {
                    w.module_call(obj, idx, ModuleCall::GetProbabilityModifier(i))
                }) {
                    roll -= if plant(1343) { 1 } else { m };
                }
                if if plant(1344) { roll < 0 } else { roll <= 0 } || plant(1345) {
                    let i = info.clone();
                    active_body::out(self, world, me, |w| {
                        w.module_call(obj, idx, ModuleCall::BeginSlowDeath(i))
                    });
                    if !plant(1346) {
                        return;
                    }
                }
            }
        }
    }
}

impl ActiveBodyVirtuals for UndeadBody {
    fn ab(&self) -> &ActiveBody {
        &self.ab
    }
    fn ab_mut(&mut self) -> &mut ActiveBody {
        &mut self.ab
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::UndeadBody(self)
    }
    fn from_body(b: UpdateBody) -> Option<Self> {
        match b {
            UpdateBody::UndeadBody(m) => Some(m),
            _ => None,
        }
    }
    fn attempt_damage(&mut self, world: &mut World, me: ModuleHandle, info: &mut DamageInfo) {
        self.attempt_damage_override(world, me, info);
    }
}

impl Snapshot for UndeadBody {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.ab.crc(xfer)
    }

    /// `xfer` (version 1, "Initial version"): the version, the base, then `m_isSecondLife`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.ab.xfer(xfer)?;
        if crate::logic::modules::plant(1347) {
            return Ok(());
        }
        xfer.xfer_bool(&mut self.is_second_life)
    }

    /// `loadPostProcess`: the base's.
    fn load_post_process(&mut self) {
        self.ab.load_post_process();
    }
}

// ---- HiveStructureBody ------------------------------------------------------------------------------------------------------------------------------------------

/// `class HiveStructureBodyModuleData : public StructureBodyModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct HiveStructureBodyModuleData {
    /// The `StructureBodyModuleData` (that is, `ActiveBodyModuleData`) base.
    pub active: ActiveBodyModuleData,
    /// `m_damageTypesToPropagateToSlaves`.
    pub damage_types_to_propagate_to_slaves: DamageTypeFlags,
    /// `m_damageTypesToSwallow`: "a subset of the damage types to propagate. Do not take them ourselves".
    pub damage_types_to_swallow: DamageTypeFlags,
}

impl Default for HiveStructureBodyModuleData {
    /// `HiveStructureBodyModuleData()`: both sets `DAMAGE_TYPE_FLAGS_NONE`.
    fn default() -> Self {
        HiveStructureBodyModuleData {
            active: ActiveBodyModuleData::default(),
            damage_types_to_propagate_to_slaves: DAMAGE_TYPE_FLAGS_NONE,
            damage_types_to_swallow: DAMAGE_TYPE_FLAGS_NONE,
        }
    }
}

/// `ActiveBodyModuleData::buildFieldParse`'s own table, on the `active` base of `HiveStructureBodyModuleData`.
static HIVE_BASE_FIELD_PARSE: [FieldParse<HiveStructureBodyModuleData>; 6] = [
    FieldParse::new(
        "MaxHealth",
        |i, d, u| lift_active_field(0, i, &mut d.active, u),
        UserData::None,
    ),
    FieldParse::new(
        "InitialHealth",
        |i, d, u| lift_active_field(1, i, &mut d.active, u),
        UserData::None,
    ),
    FieldParse::new(
        "SubdualDamageCap",
        |i, d, u| lift_active_field(2, i, &mut d.active, u),
        UserData::None,
    ),
    FieldParse::new(
        "SubdualDamageHealRate",
        |i, d, u| lift_active_field(3, i, &mut d.active, u),
        UserData::None,
    ),
    FieldParse::new(
        "SubdualDamageHealAmount",
        |i, d, u| lift_active_field(4, i, &mut d.active, u),
        UserData::None,
    ),
    FieldParse::end(),
];

/// `HiveStructureBodyModuleData::buildFieldParse`'s own table.
static HIVE_FIELD_PARSE: [FieldParse<HiveStructureBodyModuleData>; 3] = [
    FieldParse::new(
        "PropagateDamageTypesToSlavesWhenExisting",
        |i, d, _| {
            d.damage_types_to_propagate_to_slaves = i.parse_damage_type_flags()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "SwallowDamageTypesIfSlavesNotExisting",
        |i, d, _| {
            d.damage_types_to_swallow = i.parse_damage_type_flags()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl HiveStructureBodyModuleData {
    /// `buildFieldParse(p)`: `StructureBodyModuleData::buildFieldParse` (which adds nothing to the active body's), then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, HiveStructureBodyModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&HIVE_BASE_FIELD_PARSE)?;
        p.add(&HIVE_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<HiveStructureBodyModuleData> {
        let mut data = HiveStructureBodyModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                HiveStructureBodyModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `HiveStructureBody`.
pub fn new_hive_module_data(
    ini: Option<&mut Ini<'_>>,
) -> IniResult<crate::common::module::ModuleDataKind> {
    Ok(crate::common::module::ModuleDataKind::HiveStructureBody(
        Arc::new(HiveStructureBodyModuleData::friend_new_module_data(ini)?),
    ))
}

/// `class HiveStructureBody : public StructureBody`: "a structure body that passes the damage it takes on to its spawn (or its riders), the closest to the
/// shooter taking it".
#[derive(Clone, Debug)]
pub struct HiveStructureBody {
    /// The `StructureBody` base.
    sb: StructureBody,
    /// `getHiveStructureBodyModuleData()`.
    data: Arc<HiveStructureBodyModuleData>,
}

impl PartialEq for HiveStructureBody {
    /// The base's identity.
    fn eq(&self, other: &Self) -> bool {
        self.sb == other.sb
    }
}

impl HiveStructureBody {
    /// `getInterfaceMask()`: the base's.
    pub const fn get_interface_mask() -> i32 {
        StructureBody::get_interface_mask()
    }

    /// The constructor's first half.
    pub fn new(
        object: ObjectId,
        module_data: Option<Arc<HiveStructureBodyModuleData>>,
    ) -> IniResult<HiveStructureBody> {
        let base = module_data.as_ref().map(|d| Arc::new(d.active.clone()));
        let sb = StructureBody::new(object, base)?;
        Ok(HiveStructureBody {
            sb,
            data: module_data.unwrap_or_default(),
        })
    }

    /// The constructor's second half, run by the world.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.sb.world_construct(world, me);
    }

    /// The base's base.
    pub fn active_body(&self) -> &ActiveBody {
        self.sb.active_body()
    }

    /// The structure base.
    pub fn structure_body(&self) -> &StructureBody {
        &self.sb
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        match self.sb.derived_call(&call) {
            Some(r) => r,
            None => active_body::body_module_call(self, world, me, call),
        }
    }

    /// Gives the damage to `target` (`target->attemptDamage(damageInfo)`), with the body lent out.
    fn pass_on(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        target: ObjectId,
        info: &mut DamageInfo,
    ) {
        active_body::out(self, world, me, |w| w.attempt_damage(target, info));
    }

    /// "no slave to give to, so eat it".
    fn swallow(info: &mut DamageInfo) {
        use crate::logic::modules::plant;
        info.out.actual_damage_dealt = 0.0;
        if !plant(1351) {
            info.out.actual_damage_clipped = 0.0;
        }
        info.out.no_effect = !plant(1352);
    }

    /// `HiveStructureBody::attemptDamage`.
    fn attempt_damage_override(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        info: &mut DamageInfo,
    ) {
        use crate::logic::modules::plant;
        let data = self.data.clone();
        let hive = me.object;
        let propagate_set = if plant(1355) {
            &data.damage_types_to_swallow
        } else {
            &data.damage_types_to_propagate_to_slaves
        };
        let propagate = propagate_set.test(info.input.damage_type) != plant(1353);
        if propagate {
            // we have the right type of damage types incoming to propagate to slaves. Do we have slaves?
            let spawn = active_body::out(self, world, me, |w| w.get_spawn_behavior_interface(hive));
            let contain = world.get_contain(hive);
            let (spawn, contain) = if plant(1356) && contain.is_some() {
                (None, contain)
            } else {
                (spawn, contain)
            };
            // `shooter->getPosition()`
            let shooter_pos = match world.objects.find(info.input.source_id) {
                Some(o) => Some(if plant(1357) {
                    Coord3D {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    }
                } else {
                    o.get_position()
                }),
                None => plant(1358).then_some(Coord3D {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                }),
            };
            if let Some(spawn) = spawn {
                // we found the spawn interface, now get some slaves!
                if let Some(pos) = shooter_pos {
                    let slave = active_body::out(self, world, me, |w| {
                        w.module_call(hive, spawn, ModuleCall::GetClosestSlave(pos))
                    })
                    .as_id();
                    if slave != INVALID_ID {
                        // propagate damage and return!
                        self.pass_on(world, me, slave, info);
                        if !plant(1359) {
                            return;
                        }
                    } else if data.damage_types_to_swallow.test(info.input.damage_type)
                        != plant(1354)
                    {
                        Self::swallow(info);
                        return;
                    }
                }
            } else if let Some(contain) = contain {
                if let Some(pos) = shooter_pos {
                    let rider = active_body::out(self, world, me, |w| {
                        w.module_call(hive, contain, ModuleCall::GetClosestRider(pos))
                    })
                    .as_id();
                    if rider != INVALID_ID {
                        // propagate damage and return!
                        self.pass_on(world, me, rider, info);
                        return;
                    } else if data.damage_types_to_swallow.test(info.input.damage_type)
                        != plant(1360)
                    {
                        Self::swallow(info);
                        return;
                    }
                }
            }
            // else: `DEBUG_CRASH`, compiled out of a release build
        }

        // nothing to propagate (either different damage type or no slaves), so damage me instead!
        active_body::attempt_damage(self, world, me, info);
    }
}

impl ActiveBodyVirtuals for HiveStructureBody {
    fn ab(&self) -> &ActiveBody {
        &self.sb.ab
    }
    fn ab_mut(&mut self) -> &mut ActiveBody {
        &mut self.sb.ab
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::HiveStructureBody(self)
    }
    fn from_body(b: UpdateBody) -> Option<Self> {
        match b {
            UpdateBody::HiveStructureBody(m) => Some(m),
            _ => None,
        }
    }
    fn attempt_damage(&mut self, world: &mut World, me: ModuleHandle, info: &mut DamageInfo) {
        self.attempt_damage_override(world, me, info);
    }
}

impl Snapshot for HiveStructureBody {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.sb.crc(xfer)
    }

    /// `xfer` (version 1, "Initial version"): the version, then the parent class's.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.sb.xfer(xfer)
    }

    /// `loadPostProcess`: the base's.
    fn load_post_process(&mut self) {
        self.sb.load_post_process();
    }
}
