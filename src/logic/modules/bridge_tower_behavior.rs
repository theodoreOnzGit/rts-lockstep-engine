// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/BridgeTowerBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BridgeTowerBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `BridgeTowerBehavior`: one of the four towers at the corners of a bridge. Damage or healing done to a tower is passed on, as the same
//! fraction of each one's maximum health, to the other towers and to the bridge (unless it came from the bridge or a tower: that is how the
//! bridge passes its own on, and the towers do not pass it back); the tower's death kills the bridge, which kills the towers.
//!
//! The interface `TerrainLogic`'s `Bridge::createTower` uses (`BridgeTowerBehaviorInterface`) is the module calls
//! [`ModuleCall::BridgeTowerSetBridge`], `BridgeTowerGetBridgeId` and `BridgeTowerSetTowerType`;
//! `getBridgeTowerBehaviorInterfaceFromObject` is [`World::bridge_tower_interface`].
//!
//! # Upstream facts kept
//!
//! - The module data is the plain `ModuleData` (`MAKE_STANDARD_MODULE_MACRO`, no data class); the interfaces are damage and die only.
//! - The bridge's interface is the first behaviour module of the bridge that has one; with none, nothing is passed on (a debug assertion).
//! - The damage passed on is a new `DamageInfo` (its defaults) with the amount, this tower as the source, and the damage and death types;
//!   the healing is `attemptHealing(amount, thisTower)`. A tower that is this one is skipped; the bridge is damaged or healed last.
//! - `onBodyDamageStateChange` does nothing; `onDie` is `bridge->kill()` (`DAMAGE_UNRESISTABLE`, `DEATH_NORMAL`).
//! - The xfer is the version (1), the `BehaviorModule` base, the bridge id and the tower type as the four bytes of the enum (`xferUser`); the
//!   CRC is the base's.
//! - The constructor leaves `m_type` uninitialised; `Bridge::createTower`, the only maker of towers, sets it at once. This port starts it at
//!   `BRIDGE_TOWER_FROM_LEFT` (0).
//!
//! # Where upstream is undefined
//!
//! `onDamage` or `onHealing` with a bridge, on an object without a body (`getBodyModule()->getMaxHealth()`), or passing it on to a tower or
//! a bridge without one; `onDie` with a bridge without a body (`Object::kill` reads it unchecked): [`Undefined::NullBodyModule`]. The case
//! is recorded in `world.cuts.module_errors` and the call returns.
//!
//! `p1w13-sonnet`, batch R38, 2026-10-24.

use std::sync::Arc;

use crate::common::game_type::INVALID_ID;
use crate::common::ini::{IniError, IniErrorKind, IniResult, Undefined};
use crate::common::kind_of::{KINDOF_BRIDGE, KINDOF_BRIDGE_TOWER};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DAMAGE, MODULEINTERFACE_DIE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_UNRESISTABLE, DEATH_NONE, DEATH_NORMAL};
use crate::logic::module::behavior_module::{BehaviorModule, BehaviorModuleData};
use crate::logic::modules::lend_clone;
use crate::logic::terrain_logic::BRIDGE_MAX_TOWERS;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object::ModuleIndex;
use crate::logic::world::update_body::UpdateBody;

/// `class BridgeTowerBehavior : public BehaviorModule, public BridgeTowerBehaviorInterface, public DieModuleInterface, public
/// DamageModuleInterface`.
#[derive(Clone, Debug)]
pub struct BridgeTowerBehavior {
    /// The `BehaviorModule` base.
    behavior: BehaviorModule,
    /// `m_bridgeID`: "the bridge we're a part of".
    bridge_id: ObjectId,
    /// `m_type`: "type of tower (positioning) we are" (`BridgeTowerType` as its value).
    tower_type: i32,
}

impl PartialEq for BridgeTowerBehavior {
    /// The same object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.behavior.object() == other.behavior.object() && self.bridge_id == other.bridge_id && self.tower_type == other.tower_type
    }
}

impl BridgeTowerBehavior {
    /// `getInterfaceMask()`: `MODULEINTERFACE_DAMAGE | MODULEINTERFACE_DIE`.
    pub const fn get_interface_mask() -> i32 {
        MODULEINTERFACE_DAMAGE | MODULEINTERFACE_DIE
    }

    /// `BridgeTowerBehavior(thing, moduleData)`: the base (`INI_INVALID_DATA` for null data), no bridge.
    pub fn new(object: ObjectId, module_data: Option<Arc<BehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let behavior = BehaviorModule::new(object, Some(ModuleDataKind::Behavior(data)))?;
        Ok(BridgeTowerBehavior { behavior, bridge_id: INVALID_ID, tower_type: 0 })
    }

    /// `getBridgeID()`.
    pub fn get_bridge_id(&self) -> ObjectId {
        self.bridge_id
    }

    /// The tower type (`m_type`).
    pub fn tower_type(&self) -> i32 {
        self.tower_type
    }

    /// The bridge's `BridgeBehaviorInterface`: the first module of the bridge that has one.
    fn bridge_interface(world: &mut World, bridge: ObjectId) -> Option<ModuleIndex> {
        world.all_modules(bridge).into_iter().find(|&idx| world.module_call(bridge, idx, ModuleCall::HasBridgeBehaviorInterface).as_bool())
    }

    /// `getBodyModule()->getMaxHealth()` of `id`; [`Undefined::NullBodyModule`] without a body.
    fn max_health(world: &mut World, id: ObjectId) -> Result<f32, Undefined> {
        let body = world.get_body_module(id).ok_or(Undefined::NullBodyModule)?;
        Ok(world.module_call(id, body, ModuleCall::GetMaxHealth).as_real(0.0))
    }

    /// The fraction of this tower's maximum health `info` does, the bridge, and its interface, when the damage or healing is to be passed
    /// on: `None` without a bridge, without an interface, or when the source is the bridge or a tower.
    fn pass_on(&self, world: &mut World, me: ModuleHandle, info: &DamageInfo) -> Result<Option<(f32, ObjectId, ModuleIndex)>, Undefined> {
        let bridge = self.bridge_id;
        // sanity
        if world.objects.find(bridge).is_none() {
            return Ok(None);
        }
        // "get our body info so we now how much damage percent is being done to us"
        let percentage = info.input.amount / Self::max_health(world, me.object)?;
        // get the bridge behavior module for our bridge
        let Some(bbi) = Self::bridge_interface(world, bridge) else { return Ok(None) };
        // "damage each of the other towers if the source of this damage isn't from the bridge or other towers"
        let from_bridge = world.objects.find(info.input.source_id).is_some_and(|s| {
            s.is_kind_of(KINDOF_BRIDGE) || (s.is_kind_of(KINDOF_BRIDGE_TOWER) && !crate::logic::modules::plant(2085))
        });
        if from_bridge {
            return Ok(None);
        }
        Ok(Some((percentage, bridge, bbi)))
    }

    /// The other towers of the bridge, in tower order (a tower that is not there, or is this one, is skipped).
    fn other_towers(world: &mut World, me: ModuleHandle, bridge: ObjectId, bbi: ModuleIndex) -> Vec<ObjectId> {
        let mut towers = Vec::new();
        for i in 0..BRIDGE_MAX_TOWERS as i32 {
            let tower = world.module_call(bridge, bbi, ModuleCall::GetBridgeTowerId { tower: i }).as_id();
            if world.objects.find(tower).is_some() && (tower != me.object || crate::logic::modules::plant(2087)) {
                towers.push(tower);
            }
        }
        towers
    }

    /// The `DamageInfo` passed on: the amount, this tower as the source ("we're now the source"), the damage and death types.
    fn passed_damage(me: ModuleHandle, info: &DamageInfo, amount: f32) -> DamageInfo {
        let mut d = DamageInfo::default();
        d.input.amount = amount;
        d.input.source_id = if crate::logic::modules::plant(2082) { info.input.source_id } else { me.object };
        if !crate::logic::modules::plant(2083) {
            d.input.damage_type = info.input.damage_type;
        }
        if !crate::logic::modules::plant(2084) {
            d.input.death_type = info.input.death_type;
        }
        d
    }

    /// `onDamage(damageInfo)`.
    fn on_damage(&self, world: &mut World, me: ModuleHandle, info: &DamageInfo) -> Result<(), Undefined> {
        let Some((percentage, bridge, bbi)) = self.pass_on(world, me, info)? else { return Ok(()) };
        for tower in Self::other_towers(world, me, bridge, bbi) {
            let max = Self::max_health(world, tower)?;
            let amount = if crate::logic::modules::plant(2081) { percentage } else { percentage * max };
            let mut tower_damage = Self::passed_damage(me, info, amount);
            world.attempt_damage(tower, &mut tower_damage);
        }
        // "damage bridge object, but make sure it's done through the bridge interface so that it doesn't automatically propagate that
        // damage to the towers"
        let mut bridge_damage = Self::passed_damage(me, info, percentage * Self::max_health(world, bridge)?);
        world.attempt_damage(bridge, &mut bridge_damage);
        Ok(())
    }

    /// `onHealing(damageInfo)`.
    fn on_healing(&self, world: &mut World, me: ModuleHandle, info: &DamageInfo) -> Result<(), Undefined> {
        let Some((percentage, bridge, bbi)) = self.pass_on(world, me, info)? else { return Ok(()) };
        for tower in Self::other_towers(world, me, bridge, bbi) {
            let amount = percentage * Self::max_health(world, tower)?;
            world.attempt_healing(tower, amount, me.object);
        }
        // heal bridge object
        let max = Self::max_health(world, bridge)?;
        let amount = if crate::logic::modules::plant(2086) { info.input.amount } else { percentage * max };
        world.attempt_healing(bridge, amount, me.object);
        Ok(())
    }

    /// `onDie(damageInfo)`: "kill the bridge object, this will kill all the towers".
    fn on_die(&self, world: &mut World) -> Result<(), Undefined> {
        let bridge = self.bridge_id;
        if world.objects.find(bridge).is_some() {
            // Object::kill reads the body unchecked
            world.get_body_module(bridge).ok_or(Undefined::NullBodyModule)?;
            let death = if crate::logic::modules::plant(2088) { DEATH_NONE } else { DEATH_NORMAL };
            world.kill(bridge, DAMAGE_UNRESISTABLE, death);
        }
        Ok(())
    }

    /// The module's answer to one of `Object`'s calls: the damage and die interfaces, and `BridgeTowerBehaviorInterface`.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        let r = match call {
            ModuleCall::OnDamage(info) => lend_clone(world, me, self, UpdateBody::BridgeTowerBehavior, Self::from_body, |w, m| m.on_damage(w, me, &info)),
            ModuleCall::OnHealing(info) => lend_clone(world, me, self, UpdateBody::BridgeTowerBehavior, Self::from_body, |w, m| m.on_healing(w, me, &info)),
            ModuleCall::OnBodyDamageStateChange { .. } => Ok(()),
            ModuleCall::OnDie(_) => lend_clone(world, me, self, UpdateBody::BridgeTowerBehavior, Self::from_body, |w, m| m.on_die(w)),
            ModuleCall::BridgeTowerSetBridge(bridge) => {
                // setBridge(NULL) clears it
                self.bridge_id = if world.objects.find(bridge).is_some() { bridge } else { INVALID_ID };
                Ok(())
            }
            ModuleCall::BridgeTowerGetBridgeId => return ModuleReply::Id(self.bridge_id),
            ModuleCall::BridgeTowerSetTowerType(t) => {
                self.tower_type = t;
                Ok(())
            }
            _ => return ModuleReply::Unhandled,
        };
        if let Err(u) = r {
            world.cuts.module_errors.push((me, IniError::undefined(u)));
        }
        ModuleReply::Done
    }

    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::BridgeTowerBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl World {
    /// `BridgeTowerBehavior::getBridgeTowerBehaviorInterfaceFromObject(obj)`: `None` for no object or one that is not a
    /// `KINDOF_BRIDGE_TOWER`; otherwise the first behaviour module with the interface, which only this module has.
    pub fn bridge_tower_interface(&self, id: ObjectId) -> Option<ModuleIndex> {
        if !self.objects.find(id).is_some_and(|o| o.is_kind_of(KINDOF_BRIDGE_TOWER)) {
            return None;
        }
        self.find_module(id, b"BridgeTowerBehavior")
    }
}

impl Snapshot for BridgeTowerBehavior {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.behavior.crc(xfer)
    }
    /// `xfer` (version 1): the version, the base, the bridge id, the tower type (four bytes).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.behavior.xfer(xfer)?;
        let mut tower_type = self.tower_type.to_le_bytes();
        if crate::logic::modules::plant(2089) {
            xfer.xfer_user(&mut tower_type)?;
            xfer.xfer_object_id(&mut self.bridge_id)?;
        } else {
            xfer.xfer_object_id(&mut self.bridge_id)?;
            xfer.xfer_user(&mut tower_type)?;
        }
        self.tower_type = i32::from_le_bytes(tower_type);
        Ok(())
    }
    fn load_post_process(&mut self) {
        self.behavior.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `BridgeTowerBehavior` (`MAKE_STANDARD_MODULE_MACRO`: `Module::friend_newModuleData`, the plain
/// `ModuleData`, which `BehaviorModuleData` adds nothing to).
pub fn new_module_data(ini: Option<&mut crate::common::ini::Ini<'_>>) -> IniResult<ModuleDataKind> {
    let module_data = crate::common::module::ModuleData::friend_new_module_data(ini)?;
    Ok(ModuleDataKind::Behavior(Arc::new(BehaviorModuleData { module_data })))
}
