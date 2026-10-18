// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file (this repository's own design, not upstream code):
// a scripted module that answers every [`ModuleCall`] of `Object`, for the
// tests of the dispatch and for the differential test, which runs the same
// module on the C++ side. It is part of the library (not `#[cfg(test)]`)
// because the integration tests use it. Production code never constructs one.

//! [`ProbeModule`]: a module with every interface, whose answers are data.
//!
//! It stands for the concrete modules of layer 9 the way `ProbeUpdate` stands
//! for the update modules: it makes the dispatch of `Object` testable before
//! any real module exists. Its behaviour is a few lines per call, written
//! down here and mirrored by the C++ stand-in of
//! `differential/wrap_object.cpp`, so that both sides answer alike. What it
//! does beyond answering:
//!
//! - as a **body** it keeps a health, takes damage and healing, and when the
//!   health reaches zero calls `Object::onDie` once, as `ActiveBody` does
//!   (the re-entrant path `attemptDamage` -> body -> `onDie` -> die modules);
//! - as an **upgrade** module it keeps whether it has run, and runs when the
//!   mask it is given intersects its trigger;
//! - as a **collide** module it can set `OBJECT_STATUS_NO_COLLISIONS` on its
//!   object ("check each time thru the loop, in case a collide module sets it").

use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::object_status_types::OBJECT_STATUS_NO_COLLISIONS;
use crate::common::upgrade::UpgradeMaskType;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_HEALING};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_state::mask_of;

/// What a probe answers to the yes/no questions. Bit `k` of `has` answers the
/// `k`-th `Has...Interface` call (in the order of [`ModuleCall`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProbeAnswers {
    /// The answer to every other yes/no question.
    pub yes: bool,
    /// The `Has...Interface` answers.
    pub has: u32,
    /// `isSalvageCrateCollide`.
    pub salvage: bool,
    /// `isSubObjectsUpgrade`.
    pub sub_objects: bool,
    /// `isEnclosingContainerFor`.
    pub enclosing: bool,
    /// `isGarrisonable`.
    pub garrisonable: bool,
    /// `isKickOutOnCapture`.
    pub kick_out: bool,
    /// `isPassengerAllowedToFire`.
    pub passenger_may_fire: bool,
    /// `isSpecialZeroSlotContainer`.
    pub zero_slot: bool,
    /// `isHeroInside`.
    pub hero_inside: bool,
    /// The special power type (0 = none: the reply is `Unhandled`).
    pub special_power_type: i32,
    /// `isShortcutPower`.
    pub shortcut: bool,
    /// The template `isModuleForPower` accepts (0 = none).
    pub power_template: u32,
    /// The rider (`friend_getRider`).
    pub rider: ObjectId,
    /// The contained items.
    pub contained: Vec<ObjectId>,
    /// The slaver (`getSlaverID`, 0 = no slaved interface: `Unhandled`).
    pub slaver: ObjectId,
    /// The landing deck height offset (NaN = no parking place: `Unhandled`).
    pub deck_height: f32,
    /// The apparent controlling player.
    pub apparent_player: i32,
    /// The countermeasure to divert to.
    pub countermeasure: ObjectId,
    /// A collide call sets `OBJECT_STATUS_NO_COLLISIONS` on the module's object.
    pub collide_sets_no_collisions: bool,
}

/// The scripted module. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeModule {
    /// The answers.
    pub answers: ProbeAnswers,
    /// The body's health.
    pub health: f32,
    /// The body's maximum health.
    pub max_health: f32,
    /// The body's damage scalar.
    pub damage_scalar: f32,
    /// The body has called `onDie` already.
    pub died: bool,
    /// The source of the last damage the body took.
    pub last_damage_source: ObjectId,
    /// The armor set flags the body has set (bit per flag).
    pub armor_flags: u32,
    /// The upgrade this module runs on.
    pub upgrade_trigger: UpgradeMaskType,
    /// The upgrade module has run.
    pub upgraded: bool,
    /// How many calls the module has received (part of its state, saved).
    pub calls: u32,
}

impl ProbeModule {
    /// A probe with these answers, full health `max_health`, and an upgrade
    /// trigger.
    pub fn new(answers: ProbeAnswers, max_health: f32, upgrade_trigger: UpgradeMaskType) -> ProbeModule {
        ProbeModule {
            answers,
            health: max_health,
            max_health,
            damage_scalar: 1.0,
            died: false,
            last_damage_source: INVALID_ID,
            armor_flags: 0,
            upgrade_trigger,
            upgraded: false,
            calls: 0,
        }
    }

    /// The module's answer to `call`.
    pub fn handle(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        self.calls = self.calls.wrapping_add(1);
        match call {
            // ---- the ones with an answer ----
            ModuleCall::IsSalvageCrateCollide => ModuleReply::Bool(self.answers.salvage),
            ModuleCall::IsAlreadyUpgraded => ModuleReply::Bool(self.upgraded),
            ModuleCall::AttemptUpgrade(mask) => {
                if mask.test_for_any(&self.upgrade_trigger) {
                    self.upgraded = true;
                    ModuleReply::Bool(true)
                } else {
                    ModuleReply::Bool(false)
                }
            }
            ModuleCall::WouldUpgrade(mask) => ModuleReply::Bool(!self.upgraded && mask.test_for_any(&self.upgrade_trigger)),
            ModuleCall::ResetUpgrade(mask) => {
                if mask.test_for_any(&self.upgrade_trigger) {
                    self.upgraded = false;
                    ModuleReply::Bool(true)
                } else {
                    ModuleReply::Bool(false)
                }
            }
            ModuleCall::IsSubObjectsUpgrade => ModuleReply::Bool(self.answers.sub_objects),
            ModuleCall::GetSpecialPowerType => {
                if self.answers.special_power_type != 0 {
                    ModuleReply::Int(self.answers.special_power_type)
                } else {
                    ModuleReply::Unhandled
                }
            }
            ModuleCall::IsShortcutPower => ModuleReply::Bool(self.answers.shortcut),
            ModuleCall::IsModuleForPower { template } => ModuleReply::Bool(self.answers.power_template == template),
            ModuleCall::AttemptDamage(mut info) => {
                // ActiveBody in miniature: the damage taken, then death once.
                let dealt = if info.input.kill {
                    self.health
                } else {
                    let amount = info.input.amount * self.damage_scalar;
                    if amount < self.health {
                        amount
                    } else {
                        self.health
                    }
                };
                self.health -= dealt;
                info.out.actual_damage_dealt = dealt;
                info.out.actual_damage_clipped = dealt;
                info.out.no_effect = false;
                self.last_damage_source = info.input.source_id;
                if self.health <= 0.0 && !self.died {
                    self.died = true;
                    world.on_die(me.object, &info);
                }
                ModuleReply::Damage(info)
            }
            ModuleCall::AttemptHealing(mut info) => {
                let room = self.max_health - self.health;
                let healed = if info.input.damage_type == DAMAGE_HEALING && info.input.amount < room { info.input.amount } else { room };
                self.health += healed;
                info.out.actual_damage_dealt = healed;
                ModuleReply::Damage(info)
            }
            ModuleCall::EstimateDamage(input) => ModuleReply::Real(input.amount * self.damage_scalar),
            ModuleCall::GetMaxHealth => ModuleReply::Real(self.max_health),
            ModuleCall::GetHealth => ModuleReply::Real(self.health),
            ModuleCall::GetDamageScalar => ModuleReply::Real(self.damage_scalar),
            ModuleCall::GetLastDamageSource => ModuleReply::Id(self.last_damage_source),
            ModuleCall::SetArmorSetFlag(f) => {
                self.armor_flags |= 1u32.wrapping_shl(f as u32);
                ModuleReply::Done
            }
            ModuleCall::ClearArmorSetFlag(f) => {
                self.armor_flags &= !1u32.wrapping_shl(f as u32);
                ModuleReply::Done
            }
            ModuleCall::TestArmorSetFlag(f) => ModuleReply::Bool(self.armor_flags & 1u32.wrapping_shl(f as u32) != 0),
            ModuleCall::SetMaxHealth(m) => {
                self.max_health = m;
                if self.health > m {
                    self.health = m;
                }
                ModuleReply::Done
            }
            ModuleCall::IsEnclosingContainerFor(_) => ModuleReply::Bool(self.answers.enclosing),
            ModuleCall::IsSpecialZeroSlotContainer => ModuleReply::Bool(self.answers.zero_slot),
            ModuleCall::GetContainedItems => ModuleReply::Ids(self.answers.contained.clone()),
            ModuleCall::IsPassengerAllowedToFire(_) => ModuleReply::Bool(self.answers.passenger_may_fire),
            ModuleCall::GetContainCount => ModuleReply::Int(self.answers.contained.len() as i32),
            ModuleCall::IsGarrisonable => ModuleReply::Bool(self.answers.garrisonable),
            ModuleCall::IsKickOutOnCapture => ModuleReply::Bool(self.answers.kick_out),
            ModuleCall::GetRider => ModuleReply::Id(self.answers.rider),
            ModuleCall::IsHeroInside => ModuleReply::Bool(self.answers.hero_inside),
            ModuleCall::GetApparentControllingPlayer { .. } => ModuleReply::Int(self.answers.apparent_player),
            ModuleCall::HasUpdateExitInterface => ModuleReply::Bool(self.answers.has & (1 << 0) != 0),
            ModuleCall::HasProductionUpdateInterface => ModuleReply::Bool(self.answers.has & (1 << 1) != 0),
            ModuleCall::HasDockUpdateInterface => ModuleReply::Bool(self.answers.has & (1 << 2) != 0),
            ModuleCall::HasSpawnBehaviorInterface => ModuleReply::Bool(self.answers.has & (1 << 3) != 0),
            ModuleCall::HasProjectileUpdateInterface => ModuleReply::Bool(self.answers.has & (1 << 4) != 0),
            ModuleCall::HasCountermeasuresInterface => ModuleReply::Bool(self.answers.has & (1 << 5) != 0),
            ModuleCall::HasParkingPlaceInterface => ModuleReply::Bool(self.answers.has & (1 << 6) != 0),
            ModuleCall::HasContainExitInterface => ModuleReply::Bool(self.answers.has & (1 << 7) != 0),
            ModuleCall::GetLandingDeckHeightOffset => {
                if self.answers.deck_height.is_nan() {
                    ModuleReply::Unhandled
                } else {
                    ModuleReply::Real(self.answers.deck_height)
                }
            }
            ModuleCall::GetSlaverId => {
                if self.answers.slaver != INVALID_ID {
                    ModuleReply::Id(self.answers.slaver)
                } else {
                    ModuleReply::Unhandled
                }
            }
            ModuleCall::CanAnySlavesAttack
            | ModuleCall::GetRadarDisableProof
            | ModuleCall::IsOverchargeActive
            | ModuleCall::IsAbleToBeToppled
            | ModuleCall::CountermeasuresIsActive => ModuleReply::Bool(self.answers.yes),
            ModuleCall::CalculateCountermeasureToDivertTo { .. } => ModuleReply::Id(self.answers.countermeasure),
            ModuleCall::DoesSpecialPowerHaveOverridableDestinationActive | ModuleCall::DoesSpecialPowerHaveOverridableDestination => {
                if self.answers.has & (1 << 8) != 0 {
                    ModuleReply::Bool(self.answers.yes)
                } else {
                    ModuleReply::Unhandled
                }
            }
            ModuleCall::GetSpecialAbilityType => {
                if self.answers.has & (1 << 9) != 0 {
                    ModuleReply::Int(self.answers.special_power_type)
                } else {
                    ModuleReply::Unhandled
                }
            }
            // ---- the ones with an effect ----
            ModuleCall::OnCollide { .. } => {
                if self.answers.collide_sets_no_collisions {
                    world.set_status(me.object, mask_of(OBJECT_STATUS_NO_COLLISIONS), true);
                }
                ModuleReply::Done
            }
            // ---- notifications with no answer ----
            ModuleCall::OnObjectCreated
            | ModuleCall::OnDrawableBoundToObject
            | ModuleCall::OnDelete
            | ModuleCall::OnDisabledEdge { .. }
            | ModuleCall::OnCapture { .. }
            | ModuleCall::OnCreate
            | ModuleCall::OnDestroy
            | ModuleCall::OnDie(_)
            | ModuleCall::ForceRefreshUpgrade
            | ModuleCall::PauseCountdown { .. }
            | ModuleCall::DoSpecialPower { .. }
            | ModuleCall::DoSpecialPowerAtObject { .. }
            | ModuleCall::DoSpecialPowerAtLocation { .. }
            | ModuleCall::DoSpecialPowerUsingWaypoints { .. }
            | ModuleCall::OnVeterancyLevelChanged { .. }
            | ModuleCall::SetInitialHealth(_)
            | ModuleCall::SetIndestructible(_)
            | ModuleCall::RemoveFromContain(_)
            | ModuleCall::ContainReactToTransformChange
            | ModuleCall::RemoveAllContained { .. }
            | ModuleCall::DefectAllParkedUnits { .. }
            | ModuleCall::OrderSlavesDisabledUntil { .. }
            | ModuleCall::OrderSlavesToClearDisabled { .. }
            | ModuleCall::OnSpawnDeath { .. }
            | ModuleCall::UndoUpgrade
            | ModuleCall::ApplyTopplingForce { .. }
            | ModuleCall::Detonate
            | ModuleCall::StartDefectionTimer { .. }
            | ModuleCall::CancelAndRefundAllProduction
            | ModuleCall::StartRebuildProcess { .. }
            | ModuleCall::ReportMissileForCountermeasures { .. }
            | ModuleCall::PhysicsApplyShock(_)
            | ModuleCall::PhysicsApplyRandomRotation
            | ModuleCall::PhysicsSetStunned(_)
            | ModuleCall::NotifySubdualDamage { .. }
            | ModuleCall::DoStatusDamage { .. }
            | ModuleCall::DoTempWeaponBonus { .. } => ModuleReply::Done,
        }
    }
}

impl Snapshot for ProbeModule {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        xfer.xfer_real(&mut self.health)
    }
    /// Version 1: health, max health, damage scalar, died, last damage source,
    /// armor flags, upgraded, calls.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        xfer.xfer_real(&mut self.health)?;
        xfer.xfer_real(&mut self.max_health)?;
        xfer.xfer_real(&mut self.damage_scalar)?;
        xfer.xfer_bool(&mut self.died)?;
        xfer.xfer_object_id(&mut self.last_damage_source)?;
        xfer.xfer_unsigned_int(&mut self.armor_flags)?;
        xfer.xfer_bool(&mut self.upgraded)?;
        xfer.xfer_unsigned_int(&mut self.calls)
    }
    fn load_post_process(&mut self) {}
}

/// A helper for tests: a probe damage of `amount` from `source`.
pub fn damage_of(amount: f32, source: ObjectId) -> DamageInfo {
    let mut info = DamageInfo::default();
    info.input.amount = amount;
    info.input.source_id = source;
    info
}
