// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/InactiveBody.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Body/InactiveBody.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `InactiveBody`: the body of an object that has no health (a prop, a tech marker, a prerequisite-free structure piece). "Inactive bodies have no health so no
//! damage can really be done": damage and healing do nothing and report `noEffect`, the health is 0, the damage state is pristine; the one exception is
//! `DAMAGE_UNRESISTABLE`, which "always wipes us out" (the die modules run, once).
//!
//! # Module data
//!
//! `InactiveBody` declares `MAKE_STANDARD_MODULE_MACRO` without module data of its own, so the factory uses `Module::friend_newModuleData`: a plain `ModuleData`
//! whose block needs only its `End` ([`new_module_data`]). The `BodyModule` base wants a `BodyModuleData`, which here is that `ModuleData` and nothing more.
//!
//! # Upstream facts kept
//!
//! - The constructor makes the object effectively dead (`setEffectivelyDead(true)`): the world does it in [`InactiveBody::world_construct`].
//! - `m_dieCalled` is set after `Object::onDie` returns, and is not saved: a body that is loaded can wipe its object out a second time. During `onDie` it is still
//!   false, so a die module that damages the object again with `DAMAGE_UNRESISTABLE` reaches `onDie` again (the nested call runs on the lent state, see
//!   `active_body::out`); the same recursion happens upstream.
//! - `DEBUG_ASSERTCRASH(!isPrerequisite())` on the unresistable path is compiled out of a release build and is not translated.
//!
//! `p1w13-sonnet`, 2026-10-22.

use std::sync::Arc;

use crate::common::game_type::ObjectId;
use crate::common::ini::{Ini, IniResult};
use crate::common::module::{ModuleData, ModuleDataKind};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DamageInfoInput, DAMAGE_HEALING, DAMAGE_UNRESISTABLE};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::body_module::{BodyModule, BodyModuleData, BODY_PRISTINE};
use crate::logic::modules::lend_clone;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;

/// `class InactiveBody : public BodyModule`.
#[derive(Clone, Debug)]
pub struct InactiveBody {
    /// The `BodyModule` base.
    body: BodyModule,
    /// `m_dieCalled`.
    die_called: bool,
}

impl PartialEq for InactiveBody {
    /// The same object: the lent copy of [`InactiveBody::attempt_damage`] is always the state afterwards.
    fn eq(&self, other: &Self) -> bool {
        self.body.behavior().object() == other.body.behavior().object()
    }
}

impl InactiveBody {
    /// `getInterfaceMask()`: `BodyModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        BodyModule::get_interface_mask()
    }

    /// The constructor's first half: the base from the plain `ModuleData` (`INI_INVALID_DATA` for null data).
    pub fn new(object: ObjectId, module_data: Option<Arc<ModuleData>>) -> IniResult<InactiveBody> {
        let data = module_data.map(|d| Arc::new(BodyModuleData { behavior: BehaviorModuleData { module_data: (*d).clone() } }));
        Ok(InactiveBody { body: BodyModule::new(object, data)?, die_called: false })
    }

    /// The constructor's second half, run by the world: `getObject()->setEffectivelyDead(true)`.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        world.set_effectively_dead(me.object, !crate::logic::modules::plant(1271));
    }

    /// `estimateDamage(damageInfo)`: "inactive bodies have no health so no damage can really be done", except the unresistable.
    pub fn estimate_damage(&self, input: &DamageInfoInput) -> f32 {
        let mut amount = 0.0;
        // exception!
        if input.damage_type == DAMAGE_UNRESISTABLE && !crate::logic::modules::plant(1272) {
            amount = input.amount;
        }
        amount
    }

    /// `attemptDamage(damageInfo)`.
    pub fn attempt_damage(&mut self, world: &mut World, me: ModuleHandle, info: &mut DamageInfo) {
        if info.input.damage_type == DAMAGE_HEALING {
            // Healing and Damage are separate, so this shouldn't happen
            self.attempt_healing(info);
            return;
        }

        // Inactive bodies have no health so no damage can really be done
        info.out.actual_damage_dealt = 0.0;
        info.out.actual_damage_clipped = if crate::logic::modules::plant(1274) { info.input.amount } else { 0.0 };
        info.out.no_effect = true;

        // exception: damage type KILL always wipes us out
        if info.input.damage_type == DAMAGE_UNRESISTABLE {
            info.out.no_effect = crate::logic::modules::plant(1275);

            // since we have no Health, we do not call DamageModules, nor do DamageFX. however, we DO process DieModules.
            if !self.die_called || crate::logic::modules::plant(1276) {
                let i = info.clone();
                let obj = me.object;
                lend_clone(world, me, self, UpdateBody::InactiveBody, |b| if let UpdateBody::InactiveBody(m) = b { Some(m) } else { None }, |w, _s| w.body_on_die(obj, &i));
                self.die_called = !crate::logic::modules::plant(1277);
            }
        }
    }

    /// `attemptHealing(damageInfo)`.
    pub fn attempt_healing(&mut self, info: &mut DamageInfo) {
        if info.input.damage_type != DAMAGE_HEALING {
            // Healing and Damage are separate, so this shouldn't happen: upstream sends it to `attemptDamage`, which needs the world; a caller that has one
            // goes through `module_call`, which handles this case before reaching here.
            return;
        }

        // Inactive bodies have no health so no damage can really be done
        info.out.actual_damage_dealt = 0.0;
        info.out.actual_damage_clipped = 0.0;
        info.out.no_effect = true;
    }

    /// The module's answer to one of `Object`'s calls: its own virtuals, then `BodyModule`'s defaults.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::AttemptDamage(mut info) => {
                self.attempt_damage(world, me, &mut info);
                ModuleReply::Damage(info)
            }
            ModuleCall::AttemptHealing(mut info) => {
                if info.input.damage_type != DAMAGE_HEALING && !crate::logic::modules::plant(1278) {
                    // Healing and Damage are separate, so this shouldn't happen
                    self.attempt_damage(world, me, &mut info);
                } else {
                    self.attempt_healing(&mut info);
                }
                ModuleReply::Damage(info)
            }
            ModuleCall::EstimateDamage(input) => ModuleReply::Real(self.estimate_damage(&input)),
            // "Inactive bodies have no health to get"
            ModuleCall::GetHealth => ModuleReply::Real(0.0),
            ModuleCall::GetDamageState => ModuleReply::Int(if crate::logic::modules::plant(1279) { 1 } else { BODY_PRISTINE }),
            ModuleCall::SetDamageState(_) | ModuleCall::SetAflame(_) | ModuleCall::OnVeterancyLevelChanged { .. } => ModuleReply::Done,
            ModuleCall::SetArmorSetFlag(_) | ModuleCall::ClearArmorSetFlag(_) => ModuleReply::Done,
            ModuleCall::TestArmorSetFlag(_) => ModuleReply::Bool(false),
            // "Inactive bodies have no health to increase or decrease"
            ModuleCall::InternalChangeHealth(_) => ModuleReply::Done,
            // the defaults of `BodyModule`
            ModuleCall::GetMaxHealth | ModuleCall::GetInitialHealth | ModuleCall::GetPreviousHealth => ModuleReply::Real(0.0),
            ModuleCall::GetDamageScalar => ModuleReply::Real(if crate::logic::modules::plant(1282) { 1.0 } else { self.body.get_damage_scalar() }),
            ModuleCall::ApplyDamageScalar(s) => {
                self.body.apply_damage_scalar(s);
                ModuleReply::Done
            }
            ModuleCall::GetSubdualDamageHealRate => ModuleReply::Int(self.body.get_subdual_damage_heal_rate() as i32),
            ModuleCall::GetSubdualDamageHealAmount => ModuleReply::Real(self.body.get_subdual_damage_heal_amount()),
            ModuleCall::HasAnySubdualDamage => ModuleReply::Bool(self.body.has_any_subdual_damage()),
            ModuleCall::GetCurrentSubdualDamageAmount => ModuleReply::Real(self.body.get_current_subdual_damage_amount()),
            // `getLastDamageInfo()` is NULL
            ModuleCall::GetLastDamageInfo | ModuleCall::GetLastDamageSource => ModuleReply::Unhandled,
            ModuleCall::GetLastDamageTimestamp => ModuleReply::Int(self.body.get_last_damage_timestamp() as i32),
            ModuleCall::GetLastHealingTimestamp => ModuleReply::Int(self.body.get_last_healing_timestamp() as i32),
            ModuleCall::GetClearableLastAttacker => ModuleReply::Id(self.body.get_clearable_last_attacker()),
            ModuleCall::ClearLastAttacker => {
                self.body.clear_last_attacker();
                ModuleReply::Done
            }
            ModuleCall::GetFrontCrushed => ModuleReply::Bool(self.body.get_front_crushed()),
            ModuleCall::GetBackCrushed => ModuleReply::Bool(self.body.get_back_crushed()),
            ModuleCall::SetInitialHealth(p) => {
                self.body.set_initial_health(p);
                ModuleReply::Done
            }
            ModuleCall::SetMaxHealth(m) => {
                self.body.set_max_health(m, crate::logic::module::body_module::SAME_CURRENTHEALTH);
                ModuleReply::Done
            }
            ModuleCall::SetMaxHealthWithChange { max_health, change_type } => {
                self.body.set_max_health(max_health, change_type);
                ModuleReply::Done
            }
            ModuleCall::SetFrontCrushed(b) => {
                self.body.set_front_crushed(b);
                ModuleReply::Done
            }
            ModuleCall::SetBackCrushed(b) => {
                self.body.set_back_crushed(b);
                ModuleReply::Done
            }
            ModuleCall::SetIndestructible(b) => {
                self.body.set_indestructible(b);
                ModuleReply::Done
            }
            ModuleCall::IsIndestructible => ModuleReply::Bool(self.body.is_indestructible() != crate::logic::modules::plant(1280)),
            ModuleCall::EvaluateVisualCondition => {
                self.body.evaluate_visual_condition();
                ModuleReply::Done
            }
            ModuleCall::UpdateBodyParticleSystems => {
                self.body.update_body_particle_systems();
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for InactiveBody {
    /// `crc`: `BodyModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.body.crc(xfer)
    }

    /// `xfer` (version 1, "Initial version"): the version and the base; `m_dieCalled` is not saved.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        if crate::logic::modules::plant(1281) {
            let mut d = self.die_called;
            xfer.xfer_bool(&mut d)?;
        }
        self.body.xfer(xfer)
    }

    /// `loadPostProcess`: `BodyModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.body.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `InactiveBody`: `Module::friend_newModuleData`, a plain `ModuleData`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::Module(Arc::new(ModuleData::friend_new_module_data(ini)?)))
}

