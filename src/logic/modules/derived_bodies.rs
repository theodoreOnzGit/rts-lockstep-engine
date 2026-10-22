// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/{StructureBody,HighlanderBody,ImmortalBody}.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Body/{StructureBody,HighlanderBody,ImmortalBody}.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The three bodies derived from `ActiveBody` that change nothing but one small thing: `HighlanderBody` ("takes damage according to armor, but can't die from
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
//! `p1w13-sonnet`, 2026-10-22.

use std::sync::Arc;

use crate::common::base_type::{max, min};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::ini::IniResult;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_UNRESISTABLE};
use crate::logic::modules::active_body::{self, ActiveBody, ActiveBodyModuleData, ActiveBodyVirtuals};
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
    fn attempt_damage_override(&mut self, world: &mut World, me: ModuleHandle, info: &mut DamageInfo) {
        if (info.input.damage_type != DAMAGE_UNRESISTABLE) != crate::logic::modules::plant(1261) {
            info.input.amount = if crate::logic::modules::plant(1262) { min(info.input.amount, self.ab.get_health()) } else if crate::logic::modules::plant(1263) { max(info.input.amount, self.ab.get_health() - 1.0) } else { min(info.input.amount, self.ab.get_health() - 1.0) };
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

    fn attempt_damage_override(&mut self, world: &mut World, me: ModuleHandle, info: &mut DamageInfo) {
        active_body::attempt_damage(self, world, me, info);
    }

    /// `ImmortalBody::internalChangeHealth`: "don't let anything change us to below one hit point", then the base.
    fn internal_change_health_override(&mut self, world: &mut World, me: ModuleHandle, delta: f32) {
        let delta = if crate::logic::modules::plant(1264) { max(delta, -self.ab.get_health()) } else if crate::logic::modules::plant(1265) { min(delta, -self.ab.get_health() + 1.0) } else { max(delta, -self.ab.get_health() + 1.0) };
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
            ModuleCall::GetConstructorObjectId => Some(ModuleReply::Id(self.get_constructor_object_id())),
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

    fn attempt_damage_override(&mut self, world: &mut World, me: ModuleHandle, info: &mut DamageInfo) {
        active_body::attempt_damage(self, world, me, info);
    }

    fn internal_change_health_override(&mut self, world: &mut World, me: ModuleHandle, delta: f32) {
        active_body::internal_change_health(self, world, me, delta);
    }
}
