// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SquishCollide.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/SquishCollide.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SquishCollide`: infantry that a crushing vehicle that drives into them (more than a 1.0 radius overlap, moving towards them) kills, unless the
//! vehicle is an ally of theirs or they are doing something to it (hijacking it, or placing a tank hunter's TNT charge on it).
//!
//! Its data is a plain `CollideModuleData` ([`ModuleDataKind::Collide`]). `self->getAI()->getGoalObject()` is [`ModuleCall::AiGetGoalObject`];
//! `findUpdateModule("HijackerUpdate")` is [`World::find_module`]; `findSpecialAbilityUpdate(SPECIAL_TANKHUNTER_TNT_ATTACK)->isActive()` is
//! [`World::find_special_ability_update`] and [`ModuleCall::SpecialAbilityIsActive`]; `other->getPhysics()->getVelocity()` is
//! [`ModuleCall::PhysicsGetVelocity`]; `ThePartitionManager->geomCollidesWithGeom` is the translated [`PartitionManager::geom_collides_with_geom`].
//!
//! # Upstream facts kept
//!
//! - The relationship is asked the other way round: whether the CRUSHER considers the infantry an ally.
//! - A crusher with no physics module is skipped after the relationship test (`otherPhysics == NULL`).
//! - Only the x and y components of the velocity count ("moving toward victim").
//!
//! `p1w13-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::game_common::Relationship;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{Ini, IniError, IniErrorKind, IniResult};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::special_power_type::SPECIAL_TANKHUNTER_TNT_ATTACK;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_CRUSH, DEATH_CRUSHED, HUGE_DAMAGE_AMOUNT};
use crate::logic::module::collide_module::{CollideModule, CollideModuleData};
use crate::logic::modules::lend_clone;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;

/// `class SquishCollide : public CollideModule`.
#[derive(Clone, Debug)]
pub struct SquishCollide {
    /// The `CollideModule` base.
    collide: CollideModule,
    /// The data (a `CollideModuleData`).
    data: Arc<CollideModuleData>,
}

impl PartialEq for SquishCollide {
    /// The same object and the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.collide.behavior().object() == other.collide.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl SquishCollide {
    /// `SquishCollide(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<CollideModuleData>>) -> IniResult<SquishCollide> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let collide = CollideModule::new(object, Some(data.clone()))?;
        Ok(SquishCollide { collide, data })
    }

    /// `CollideModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        CollideModule::get_interface_mask()
    }

    /// The data.
    pub fn module_data(&self) -> &Arc<CollideModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        match call {
            ModuleCall::OnCollide { other, .. } => {
                lend_clone(
                    world,
                    me,
                    self,
                    UpdateBody::SquishCollide,
                    |b| match b {
                        UpdateBody::SquishCollide(m) => Some(m),
                        _ => None,
                    },
                    |w, m| m.on_collide(w, me, other),
                );
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }

    /// `onCollide(other, loc, normal)`; `other` of `INVALID_ID` is `NULL`, "collide with ground".
    fn on_collide(&mut self, world: &mut World, me: ModuleHandle, other: ObjectId) {
        if other == INVALID_ID {
            return;
        }
        let Some(o) = world.objects.find(other) else {
            return;
        };
        let _ = o;
        let self_id = me.object;
        let ai = world.objects.find(self_id).and_then(|o| o.ai);
        if let Some(ai) = ai {
            if world
                .module_call(self_id, ai, ModuleCall::AiGetGoalObject)
                .as_id()
                == other
                && !crate::logic::modules::plant(1146)
            {
                // We are actually targeting the other object to do something to! Don't allow them to crush us in the following special circumstances:

                // Hijacking!
                if world.find_module(self_id, b"HijackerUpdate").is_some()
                    && !crate::logic::modules::plant(1144)
                {
                    return;
                }

                // TNT hunter placing a charge!
                if let Some(sa) =
                    world.find_special_ability_update(self_id, SPECIAL_TANKHUNTER_TNT_ATTACK)
                {
                    if world
                        .module_call(self_id, sa, ModuleCall::SpecialAbilityIsActive)
                        .as_bool()
                        || crate::logic::modules::plant(1145)
                    {
                        return;
                    }
                }
            }
        }

        // order matters: we want to know if IT considers ME to be an ally (a reversal of the usual situation)
        let relationship = if crate::logic::modules::plant(1147) {
            world.get_relationship(self_id, other)
        } else {
            world.get_relationship(other, self_id)
        };
        if (world.get_crusher_level(other) > 0 || crate::logic::modules::plant(1148))
            && relationship != Relationship::Allies
        {
            let Some(physics) = world.objects.find(other).and_then(|o| o.physics) else {
                return;
            };

            // use a 1.0 crush radius so the tank has to actually hit the infantry.
            let (Some(mine), Some(theirs)) =
                (world.objects.find(self_id), world.objects.find(other))
            else {
                return;
            };
            let mut my_geom = mine.geometry_info.clone();
            if !crate::logic::modules::plant(1149) {
                my_geom.set_major_radius(1.0);
                my_geom.set_minor_radius(1.0);
            }
            let (my_pos, my_angle) = (mine.get_position(), mine.thing.orientation());
            let (pos, angle) = (theirs.get_position(), theirs.thing.orientation());
            if !world.partition.geom_collides_with_geom(
                &pos,
                &theirs.geometry_info,
                angle,
                &my_pos,
                &my_geom,
                my_angle,
            ) {
                return;
            }

            // only squish if tank is moving toward victim
            let vel = world
                .module_call(other, physics, ModuleCall::PhysicsGetVelocity)
                .as_coord();
            let (to_x, to_y) = (my_pos.x - pos.x, my_pos.y - pos.y);

            if to_x * vel.x + to_y * vel.y > 0.0 || crate::logic::modules::plant(1141) {
                let mut damage_info = DamageInfo::default();
                damage_info.input.damage_type = DAMAGE_CRUSH;
                damage_info.input.death_type = if crate::logic::modules::plant(1142) {
                    0
                } else {
                    DEATH_CRUSHED
                };
                damage_info.input.source_id = other;
                damage_info.input.amount = HUGE_DAMAGE_AMOUNT; // make sure they die
                world.attempt_damage(self_id, &mut damage_info);
                if !crate::logic::modules::plant(1143) {
                    world.friend_set_undetected_defector(self_id, false); // My secret is out
                }
            }
        }
    }
}

impl Snapshot for SquishCollide {
    /// `crc`: `CollideModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.collide.crc(xfer)
    }
    /// `xfer` (version 1, "Initial version"): the version, then `CollideModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.collide.xfer(xfer)
    }
    /// `loadPostProcess`: `CollideModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.collide.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `SquishCollide`: a plain `CollideModuleData`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::Collide(Arc::new(
        CollideModuleData::friend_new_module_data(ini)?,
    )))
}
