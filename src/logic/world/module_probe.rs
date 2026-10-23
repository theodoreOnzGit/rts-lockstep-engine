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

use crate::common::base_type::Coord3D;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::object_status_types::OBJECT_STATUS_NO_COLLISIONS;
use crate::common::upgrade::UpgradeMaskType;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_HEALING};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_state::mask_of;
use crate::logic::world::update_body::UpdateBody;

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
    // --- p1w11-sonnet ---
    /// `getSubdualDamageHealRate()` of the body.
    pub subdual_rate: u32,
    /// `getSubdualDamageHealAmount()` of the body.
    pub subdual_amount: f32,
    /// `hasAnySubdualDamage()` of the body.
    pub subdual_has: bool,
    /// The module has a `SpecialPowerUpdateInterface` (otherwise it answers neither of its two calls).
    pub spu_present: bool,
    /// `doesSpecialPowerUpdatePassScienceTest()`.
    pub spu_pass: bool,
    /// `initiateIntentToDoSpecialPower(...)`: whether the update takes the intent.
    pub spu_take: bool,
    // --- end p1w11-sonnet ---
    // --- p1w13-sonnet ---
    /// `getGoalObject()` of the AI (`INVALID_ID`: none).
    pub goal_object: ObjectId,
    /// batch R13e: `getDozerAIInterface()` of the AI is not `NULL` (its `cancelTask` is a line of the trace).
    pub dozer: bool,
    /// batch R13e: the probe is a parachute's contain: `isValidContainerFor` answers `yes` and it and `addToContain` are lines of the trace.
    pub parachute: bool,
    /// batch R14a: the body's `getPreviousHealth()`, `getCurrentSubdualDamageAmount()` and the source of `getLastDamageInfo()`, when the test gives them
    /// (`None`: the probe leaves the question to a concrete body, as before).
    pub previous_health: Option<f32>,
    pub subdual_damage: Option<f32>,
    pub last_damage_source: Option<ObjectId>,
    /// batch R14b: the body's front and back crushed flags, when the test gives them (`getFrontCrushed`, `setFrontCrushed` and the back's use them).
    pub front_crushed: Option<bool>,
    /// batch R15: what `construct` returns (`INVALID_ID`: `NULL`); `construct` and `aiResumeConstruction` are lines of the trace.
    pub construct_result: ObjectId,
    /// batch R15: the probe is a hole's `RebuildHoleBehavior`: `startRebuildProcess` is a line of the trace.
    pub rebuild_hole: bool,
    /// batch R15: the probe is a `StickyBombUpdate` whose `getTargetObject` answers this (`None`: not a sticky bomb); `setTargetObject` is a line.
    pub sticky_target: Option<ObjectId>,
    /// batch R18: `isDoingGroundMovement()` of the AI.
    pub ground_movement: bool,
    /// batch R19: what `wouldLikeToCollideWith(other)` answers, as a collide module: `None` for `FALSE` and no trace (the probe's old
    /// answer), `Some(list)` for whether `other` is in the list, with a trace line.
    pub collide_with: Option<Vec<ObjectId>>,
    /// batch R20: `getTurning()` of the physics.
    pub turning: i32,
    /// batch R25: a horde interface and its `isTrueHordeMember()` (`None`: no interface).
    pub horde: Option<bool>,
    /// batch R26: `areAllSlavesStealthed()` of the spawn behaviour (`revealSlaves` is a line of the trace).
    pub slaves_stealthed: bool,
    /// batch R26: `getLastCommandSource()` of the AI.
    pub last_command_source: i32,
    /// batch R26: the contain module answers `isAnyRiderAttacking()` with `yes`... and traces `recalcApparentControllingPlayer` (`false`: both
    /// unhandled, as before).
    pub stealth_contain: bool,
    /// batch R26: the body's `getLastDamageTimestamp()` and the damage type of `getLastDamageInfo()`, when the test gives them.
    pub last_damage: Option<(u32, i32)>,
    /// batch R26: `forceRefreshUpgrade()` is a line of the trace.
    pub trace_refresh: bool,
    pub back_crushed: Option<bool>,
    /// `getVelocity()` of the physics.
    pub velocity: Coord3D,
    /// `isMoving()` of the AI.
    pub moving: bool,
    /// `isIdle()` of the AI.
    pub idle: bool,
    /// `isImmuneToClearBuildingAttacks()` of the contain module.
    pub immune: bool,
    /// `isRiderChangeContain()` of the contain module.
    pub rider_change: bool,
    /// `getTowerID(i)` of the bridge behaviour (`has` bit 10 says there is one).
    pub bridge_towers: [ObjectId; 4],
    /// `getLastCollidee()` of the physics.
    pub collidee: ObjectId,
    /// `isAiInDeadState()` of the AI (`markAsDead` sets it).
    pub ai_dead: bool,
    /// The module is a `SlowDeathBehaviorInterface` (otherwise it answers none of its three calls).
    pub slow_death: bool,
    /// `isDieApplicable(info)`: bit `damageType` of this mask.
    pub slow_applicable: u64,
    /// `getProbabilityModifier(info)`.
    pub slow_probability: i32,
    /// `getClosestSlave(pos)` of the spawn behaviour (`has` bit 3 says there is one).
    pub closest_slave: ObjectId,
    /// `getClosestRider(pos)` of the contain module.
    pub closest_rider: ObjectId,
    /// `PhysicsBehavior::getMass()`.
    pub mass: f32,
    /// `PhysicsBehavior::getAllowToFall()` (`setAllowToFall` sets it).
    pub can_fall: bool,
    /// `AIUpdateInterface::getEnterTarget()`.
    pub enter_target: ObjectId,
    // --- p1w13-sonnet --- (batch R2: TransportContain)
    /// `PhysicsBehavior::getCenterOfMassOffset()`.
    pub center_of_mass_offset: f32,
    /// `AIUpdateInterface::getAiFreeToExit(exiter)` (`AIFreeToExitType`).
    pub free_to_exit: i32,
    /// `AIUpdateInterface::getCurLocomotor()` is not `NULL`.
    pub has_locomotor: bool,
    /// `DockUpdateInterface::isDockOpen()` (batch R3); `setDockOpen` sets it.
    pub dock_open: bool,
    /// batch R8: the current locomotor's pitch stiffness, roll stiffness, pitch damping and roll damping.
    pub locomotor_sway: [f32; 4],
    /// `PhysicsBehavior::getIsInFreeFall()` (`setIsInFreeFall` sets it).
    pub free_fall: bool,
    /// batch R12 (GarrisonContain): the AI's `getCurrentVictim()`.
    pub current_victim: ObjectId,
    /// batch R12: the AI's `getCurrentVictimPos()` (`None` for `NULL`).
    pub current_victim_pos: Option<Coord3D>,
    /// batch R12: the body's `getDamageState()` (`None`: the probe does not answer, as before).
    pub damage_state: Option<i32>,
    // --- end p1w13-sonnet ---
    // --- end p1w13-sonnet ---
    // --- p1w7-sonnet ---
    /// `isClearToApproach`.
    pub clear_to_approach: bool,
    /// The AI is a supply truck's (`getSupplyTruckAIInterface()` is not `NULL`).
    pub supply_truck: bool,
    /// `getPreferredDockID`.
    pub preferred_dock: ObjectId,
    /// `getWarehouseScanDistance`.
    pub scan_distance: f32,
    // --- end p1w7-sonnet ---
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

    /// Runs `f` with this module lent to its slot ([`World::lend_body`]), so a
    /// call that reaches the module from inside `f` runs at once on the live
    /// state, and takes the state back afterwards. This is how the probe
    /// keeps upstream's plain recursion (`attemptDamage` -> `onDie` -> the
    /// same module's `onDie`).
    pub fn lend<R>(&mut self, world: &mut World, me: ModuleHandle, f: impl FnOnce(&mut World) -> R) -> R {
        let vacant = ProbeModule::new(ProbeAnswers::default(), 0.0, UpgradeMaskType::default());
        let mine = std::mem::replace(self, vacant);
        let (r, back) = world.lend_body(me, UpdateBody::ModuleProbe(mine), f);
        if let Some(UpdateBody::ModuleProbe(b)) = back {
            *self = b;
        }
        r
    }

    /// The module's answer to `call`.
    pub fn handle(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        self.calls = self.calls.wrapping_add(1);
        if let Some(t) = &mut world.cuts.trace {
            let text = format!("{call:?}");
            let end = text.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(text.len());
            t.push(format!("mod {}", &text[..end]));
        }
        match call {
            // ---- the ones with an answer ----
            ModuleCall::IsSalvageCrateCollide => ModuleReply::Bool(self.answers.salvage),
            // --- p1w11-sonnet ---
            ModuleCall::SetCleanupAreaParameters { loc, range } => {
                if world.cuts.probe_effect_trace {
                    if let Some(t) = &mut world.cuts.trace {
                        t.push(format!("cleanup {},{},{} {}", loc.x.to_bits(), loc.y.to_bits(), loc.z.to_bits(), range.to_bits()));
                    }
                }
                ModuleReply::Done
            }
            ModuleCall::SetSpyVisionDisabledUntilFrame { .. } | ModuleCall::AwardInitialCaptureBonus { .. } => ModuleReply::Done,
            ModuleCall::ActivateSpyVision { duration } => {
                if world.cuts.probe_effect_trace {
                    if let Some(t) = &mut world.cuts.trace {
                        t.push(format!("spyvision {duration}"));
                    }
                }
                ModuleReply::Done
            }
            ModuleCall::IsReady | ModuleCall::IsScriptOnly => ModuleReply::Bool(self.answers.yes),
            ModuleCall::GetPercentReady => ModuleReply::Real(1.0),
            ModuleCall::GetReadyFrame | ModuleCall::GetRequiredScience => ModuleReply::Int(0),
            // batch R13: a recharge is a line of the trace, when `probe_effect_trace` asks (the sabotage crates' test)
            ModuleCall::StartPowerRecharge => {
                if world.cuts.probe_effect_trace {
                    if let Some(t) = &mut world.cuts.trace {
                        t.push(format!("recharge {} {}", me.object, me.index));
                    }
                }
                ModuleReply::Done
            }
            ModuleCall::SetReadyFrame { .. } | ModuleCall::MarkSpecialPowerTriggered { .. } => ModuleReply::Done,
            ModuleCall::DoesSpecialPowerUpdatePassScienceTest if self.answers.spu_present => ModuleReply::Bool(self.answers.spu_pass),
            ModuleCall::InitiateIntentToDoSpecialPower { template, target_object, target_position, waypoint, command_options } if self.answers.spu_present => {
                if world.cuts.probe_effect_trace {
                    if let Some(t) = &mut world.cuts.trace {
                        t.push(format!(
                            "intent {template} {} {} {} {command_options}",
                            if target_object == INVALID_ID { 0 } else { target_object },
                            u8::from(target_position.is_some()),
                            u8::from(waypoint != 0)
                        ));
                    }
                }
                ModuleReply::Bool(self.answers.spu_take)
            }
            ModuleCall::DoesSpecialPowerUpdatePassScienceTest | ModuleCall::InitiateIntentToDoSpecialPower { .. } => ModuleReply::Unhandled,
            ModuleCall::GetSubdualDamageHealRate => ModuleReply::Int(self.answers.subdual_rate as i32),
            ModuleCall::GetSubdualDamageHealAmount => ModuleReply::Real(self.answers.subdual_amount),
            ModuleCall::HasAnySubdualDamage => ModuleReply::Bool(self.answers.subdual_has),
            ModuleCall::WouldLikeToCollideWith { other } if self.answers.collide_with.is_some() => {
                world.body_trace(format!("wouldcollide {} {other}", me.object));
                ModuleReply::Bool(self.answers.collide_with.as_ref().is_some_and(|l| l.contains(&other)))
            }
            ModuleCall::WouldLikeToCollideWith { .. }
            | ModuleCall::IsRailroad
            | ModuleCall::IsCarBombCrateCollide
            | ModuleCall::IsHijackedVehicleCrateCollide
            | ModuleCall::IsSabotageBuildingCrateCollide => ModuleReply::Bool(false),
            // --- end p1w11-sonnet ---
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
            // --- p1w11-sonnet --- (the effects are lines of the trace, tagged with the module's position, when `cuts.probe_effect_trace` asks)
            ModuleCall::GetSpecialPowerTemplateId => {
                if self.answers.power_template != 0 {
                    ModuleReply::Int(self.answers.power_template as i32)
                } else {
                    ModuleReply::Unhandled
                }
            }
            ModuleCall::PauseCountdown { disabling } => {
                if world.cuts.probe_effect_trace {
                    if let Some(t) = &mut world.cuts.trace {
                        t.push(format!("pause {} {}", me.index, i32::from(disabling)));
                    }
                }
                ModuleReply::Done
            }
            ModuleCall::ExtendRods { extend } => {
                if self.answers.has & (1 << 20) != 0 {
                    if world.cuts.probe_effect_trace {
                        if let Some(t) = &mut world.cuts.trace {
                            t.push(format!("extendrods {} {}", me.index, i32::from(extend)));
                        }
                    }
                    ModuleReply::Done
                } else {
                    ModuleReply::Unhandled
                }
            }
            ModuleCall::ExtendRadar => {
                if world.cuts.probe_effect_trace {
                    if let Some(t) = &mut world.cuts.trace {
                        t.push("extendradar".to_string());
                    }
                }
                ModuleReply::Done
            }
            ModuleCall::LoadPostProcess | ModuleCall::SetCreator { .. } => ModuleReply::Done,
            // --- end p1w11-sonnet ---
            // --- p1w11-sonnet --- (the subdual helper's test: the body only logs the subdual damage it is given, when `probe_effect_trace` asks)
            ModuleCall::AttemptDamage(info) if world.cuts.probe_effect_trace && info.input.damage_type == crate::logic::damage::DAMAGE_SUBDUAL_UNRESISTABLE => {
                if let Some(t) = &mut world.cuts.trace {
                    t.push(format!(
                        "attemptdamage {} {} {} {}",
                        info.input.damage_type,
                        info.input.amount.to_bits(),
                        info.input.source_id,
                        info.input.death_type
                    ));
                }
                ModuleReply::Damage(info)
            }
            // --- end p1w11-sonnet ---
            ModuleCall::AttemptDamage(mut info) => {
                // --- p1w12-sonnet --- (FlammableUpdate's test: the damage is a line of the trace, when `probe_effect_trace` asks)
                if world.cuts.probe_effect_trace {
                    if let Some(t) = &mut world.cuts.trace {
                        t.push(format!("damage {} {:08x} {} {} {}", me.object, info.input.amount.to_bits(), info.input.damage_type, info.input.death_type, info.input.source_id));
                    }
                }
                // --- end p1w12-sonnet ---
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
                    self.lend(world, me, |w| w.on_die(me.object, &info));
                }
                ModuleReply::Damage(info)
            }
            // --- p1w13-sonnet --- (ActiveBody's test: the calls its body makes on the other modules of its object are lines of the trace, when `cuts.body.probe_trace` asks)
            ModuleCall::OnDamage(info) if world.cuts.body.probe_trace => {
                world.body_trace(format!("ondamage {} {} {}", me.object, me.index, crate::logic::world::body_cuts::info_line(&info)));
                ModuleReply::Done
            }
            ModuleCall::OnHealing(info) => {
                if world.cuts.body.probe_trace {
                    world.body_trace(format!("onhealing {} {} {}", me.object, me.index, crate::logic::world::body_cuts::info_line(&info)));
                }
                ModuleReply::Done
            }
            ModuleCall::OnBodyDamageStateChange { info, old_state, new_state } => {
                if world.cuts.body.probe_trace {
                    world.body_trace(format!("bodystate {} {} {old_state} {new_state} {}", me.object, me.index, crate::logic::world::body_cuts::info_line(&info)));
                }
                ModuleReply::Done
            }
            ModuleCall::IsImmuneToClearBuildingAttacks => ModuleReply::Bool(self.answers.immune),
            ModuleCall::IsRiderChangeContain => ModuleReply::Bool(self.answers.rider_change),
            ModuleCall::OrderAllPassengersToIdle | ModuleCall::OrderAllPassengersToHackInternet | ModuleCall::AiIdle | ModuleCall::ProjectileNowJammed => {
                if world.cuts.body.probe_trace {
                    let name = match call {
                        ModuleCall::OrderAllPassengersToIdle => "idleall",
                        ModuleCall::OrderAllPassengersToHackInternet => "hackall",
                        ModuleCall::AiIdle => "aiidle",
                        _ => "jammed",
                    };
                    world.body_trace(format!("{name} {}", me.object));
                }
                ModuleReply::Done
            }
            ModuleCall::AiIsMoving => ModuleReply::Bool(self.answers.moving),
            ModuleCall::AiIsIdle => ModuleReply::Bool(self.answers.idle),
            ModuleCall::AiEvacuateInstantly { expose_stealth_units } => {
                if world.cuts.body.probe_trace {
                    world.body_trace(format!("aievac {} {}", me.object, i32::from(expose_stealth_units)));
                }
                ModuleReply::Done
            }
            ModuleCall::AiGuardRetaliate { victim, pos } => {
                if world.cuts.body.probe_trace {
                    world.body_trace(format!("guard {} {} {:08x} {:08x} {:08x}", me.object, victim, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()));
                }
                ModuleReply::Done
            }
            ModuleCall::HasBridgeBehaviorInterface => ModuleReply::Bool(self.answers.has & (1 << 10) != 0),
            ModuleCall::GetBridgeTowerId { tower } => {
                if self.answers.has & (1 << 10) != 0 {
                    ModuleReply::Id(self.answers.bridge_towers.get(tower as usize).copied().unwrap_or(INVALID_ID))
                } else {
                    ModuleReply::Unhandled
                }
            }
            // batch R14b: the body's crushed flags, when the test gives them
            ModuleCall::GetFrontCrushed if self.answers.front_crushed.is_some() => ModuleReply::Bool(self.answers.front_crushed.unwrap_or(false)),
            ModuleCall::GetBackCrushed if self.answers.back_crushed.is_some() => ModuleReply::Bool(self.answers.back_crushed.unwrap_or(false)),
            ModuleCall::SetFrontCrushed(v) if self.answers.front_crushed.is_some() => {
                self.answers.front_crushed = Some(v);
                ModuleReply::Done
            }
            ModuleCall::SetBackCrushed(v) if self.answers.back_crushed.is_some() => {
                self.answers.back_crushed = Some(v);
                ModuleReply::Done
            }
            // batch R14a: the body's previous health, subdual damage and last damage source, when the test gives them
            ModuleCall::GetPreviousHealth if self.answers.previous_health.is_some() => ModuleReply::Real(self.answers.previous_health.unwrap_or(0.0)),
            ModuleCall::GetCurrentSubdualDamageAmount if self.answers.subdual_damage.is_some() => ModuleReply::Real(self.answers.subdual_damage.unwrap_or(0.0)),
            ModuleCall::GetLastDamageTimestamp if self.answers.last_damage.is_some() => ModuleReply::Int(self.answers.last_damage.map_or(0, |d| d.0) as i32),
            ModuleCall::GetLastDamageInfo if self.answers.last_damage.is_some() => {
                let mut d = crate::logic::damage::DamageInfo::default();
                d.input.damage_type = self.answers.last_damage.map_or(0, |d| d.1);
                ModuleReply::Damage(d)
            }
            ModuleCall::ForceRefreshUpgrade if self.answers.trace_refresh => {
                world.body_trace(format!("refreshsub {}", me.object));
                ModuleReply::Done
            }
            ModuleCall::GetLastDamageInfo if self.answers.last_damage_source.is_some() => {
                let mut d = crate::logic::damage::DamageInfo::default();
                d.input.source_id = self.answers.last_damage_source.unwrap_or(INVALID_ID);
                ModuleReply::Damage(d)
            }
            // batch R12: the damage state, when the test gives one
            ModuleCall::GetDamageState if self.answers.damage_state.is_some() => ModuleReply::Int(self.answers.damage_state.unwrap_or(0)),
            // the body half of BodyModuleInterface belongs to the concrete bodies: the probe's body answers its own few
            ModuleCall::GetDamageState
            | ModuleCall::SetDamageState(_)
            | ModuleCall::GetInitialHealth
            | ModuleCall::GetPreviousHealth
            | ModuleCall::InternalChangeHealth(_)
            | ModuleCall::EvaluateVisualCondition
            | ModuleCall::UpdateBodyParticleSystems
            | ModuleCall::IsSubdued
            | ModuleCall::CanBeSubdued
            | ModuleCall::OnSubdualChange(_)
            | ModuleCall::GetCurrentSubdualDamageAmount
            | ModuleCall::GetLastDamageInfo
            | ModuleCall::GetLastDamageTimestamp
            | ModuleCall::GetLastHealingTimestamp
            | ModuleCall::GetClearableLastAttacker
            | ModuleCall::ClearLastAttacker
            | ModuleCall::GetFrontCrushed
            | ModuleCall::GetBackCrushed
            | ModuleCall::SetFrontCrushed(_)
            | ModuleCall::SetBackCrushed(_)
            | ModuleCall::IsIndestructible
            | ModuleCall::ApplyDamageScalar(_) => ModuleReply::Unhandled,
            ModuleCall::WouldIgnite => {
                if world.cuts.probe_effect_trace {
                    if let Some(t) = &mut world.cuts.trace {
                        t.push(format!("wouldignite {}", me.object));
                    }
                }
                ModuleReply::Bool(self.answers.yes)
            }
            ModuleCall::TryToIgnite => {
                if world.cuts.probe_effect_trace {
                    if let Some(t) = &mut world.cuts.trace {
                        t.push(format!("tryignite {}", me.object));
                    }
                }
                ModuleReply::Done
            }
            // SlowDeathBehavior's test: the physics, AI and slaved calls it makes are lines of the trace (`bt ...`), the module's answers are scripted
            ModuleCall::PhysicsSetAllowToFall(b) => {
                world.body_trace(format!("pfall {} {}", me.object, i32::from(b)));
                self.answers.can_fall = b;
                ModuleReply::Done
            }
            ModuleCall::PhysicsApplyForce(f) => {
                world.body_trace(format!("pforce {} {:08x} {:08x} {:08x}", me.object, f.x.to_bits(), f.y.to_bits(), f.z.to_bits()));
                ModuleReply::Done
            }
            ModuleCall::PhysicsSetExtraBounciness(v) => {
                world.body_trace(format!("pbounce {} {:08x}", me.object, v.to_bits()));
                ModuleReply::Done
            }
            ModuleCall::PhysicsSetExtraFriction(v) => {
                world.body_trace(format!("pfriction {} {:08x}", me.object, v.to_bits()));
                ModuleReply::Done
            }
            ModuleCall::PhysicsSetAllowBouncing(b) => {
                world.body_trace(format!("pallowbounce {} {}", me.object, i32::from(b)));
                ModuleReply::Done
            }
            ModuleCall::PhysicsSetAngles { yaw, pitch, roll } => {
                world.body_trace(format!("pangles {} {:08x} {:08x} {:08x}", me.object, yaw.to_bits(), pitch.to_bits(), roll.to_bits()));
                ModuleReply::Done
            }
            ModuleCall::PhysicsGetLastCollidee => ModuleReply::Id(self.answers.collidee),
            ModuleCall::AiIsAiInDeadState => ModuleReply::Bool(self.answers.ai_dead),
            ModuleCall::AiMarkAsDead => {
                world.body_trace(format!("markdead {}", me.object));
                self.answers.ai_dead = true;
                ModuleReply::Done
            }
            ModuleCall::SlavedOnSlaverDie => {
                world.body_trace(format!("slaverdie {}", me.object));
                ModuleReply::Done
            }
            // a probe is no slow death behaviour unless it is told to be one (the body test's UndeadBody)
            ModuleCall::BeginSlowDeath(info) => {
                if !self.answers.slow_death {
                    return ModuleReply::Unhandled;
                }
                world.body_trace(format!("slowbegin {} {}", me.object, crate::logic::world::body_cuts::info_line(&info)));
                ModuleReply::Done
            }
            ModuleCall::GetProbabilityModifier(_) => {
                if self.answers.slow_death {
                    ModuleReply::Int(self.answers.slow_probability)
                } else {
                    ModuleReply::Unhandled
                }
            }
            ModuleCall::IsDieApplicable(info) => {
                if self.answers.slow_death {
                    ModuleReply::Bool(self.answers.slow_applicable.checked_shr(info.input.damage_type as u32).unwrap_or(0) & 1 != 0)
                } else {
                    ModuleReply::Unhandled
                }
            }
            // HiveStructureBody's: the closest slave of the spawn behaviour and the closest rider of the contain module
            ModuleCall::GetClosestSlave(pos) => {
                world.body_trace(format!("closestslave {} {:08x} {:08x} {:08x}", me.object, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()));
                ModuleReply::Id(self.answers.closest_slave)
            }
            ModuleCall::GetClosestRider(pos) => {
                world.body_trace(format!("closestrider {} {:08x} {:08x} {:08x}", me.object, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()));
                ModuleReply::Id(self.answers.closest_rider)
            }
            ModuleCall::AiGetGoalObject => ModuleReply::Id(self.answers.goal_object),
            ModuleCall::AiGetCurrentVictim => ModuleReply::Id(self.answers.current_victim),
            ModuleCall::AiGetCurrentVictimPos => self.answers.current_victim_pos.map_or(ModuleReply::Unhandled, ModuleReply::Coord),
            ModuleCall::PhysicsGetVelocity => ModuleReply::Coord(self.answers.velocity),
            // --- p1w13-sonnet --- (batch R2: TransportContain; the lines of the C++ stand-ins)
            ModuleCall::PhysicsApplyMotiveForce(f) => {
                world.body_trace(format!("pmotive {} {:08x} {:08x} {:08x}", me.object, f.x.to_bits(), f.y.to_bits(), f.z.to_bits()));
                ModuleReply::Done
            }
            ModuleCall::PhysicsGetCenterOfMassOffset => ModuleReply::Real(self.answers.center_of_mass_offset),
            ModuleCall::PhysicsSetPitchRate(p) => {
                world.body_trace(format!("ppitch {} {:08x}", me.object, p.to_bits()));
                ModuleReply::Done
            }
            ModuleCall::AiSetAttitude(a) => {
                world.body_trace(format!("aiattitude {} {a}", me.object));
                ModuleReply::Done
            }
            ModuleCall::AiWakeUpAndAttemptToTarget => {
                world.body_trace(format!("aiwake {}", me.object));
                ModuleReply::Done
            }
            ModuleCall::AiGetFreeToExit { .. } => ModuleReply::Int(self.answers.free_to_exit),
            ModuleCall::AiHasCurLocomotor => ModuleReply::Bool(self.answers.has_locomotor),
            ModuleCall::ReserveDoorForExit { .. } | ModuleCall::KillRidersWhoAreNotFreeToExit | ModuleCall::IsSpecificRiderFreeToExit { .. } => ModuleReply::Unhandled,
            // --- end p1w13-sonnet ---
            // OpenContain's test: the calls the container makes on its riders are lines of the trace (`bt ...`)
            ModuleCall::AiIdleFrom { command_source } => {
                world.body_trace(format!("aiidlefrom {} {command_source}", me.object));
                ModuleReply::Done
            }
            ModuleCall::AiExit { container, command_source } => {
                world.body_trace(format!("aiexit {} {container} {command_source}", me.object));
                ModuleReply::Done
            }
            ModuleCall::AiExitInstantly { container, command_source } => {
                world.body_trace(format!("aiexitnow {} {container} {command_source}", me.object));
                ModuleReply::Done
            }
            ModuleCall::AiHackInternet { command_source } => {
                world.body_trace(format!("aihack {} {command_source}", me.object));
                ModuleReply::Done
            }
            // batch R13e: the hijacker's own module's calls (a probe is not one)
            ModuleCall::HijackerSetTargetObject(_) | ModuleCall::HijackerSetIsInVehicle(_) | ModuleCall::HijackerSetUpdate(_) => ModuleReply::Unhandled,
            ModuleCall::AiGetDozerAIInterface => ModuleReply::Bool(self.answers.dozer),
            ModuleCall::AiTransferAttack { from, to } => {
                world.body_trace(format!("transferattack {} {from} {to}", me.object));
                ModuleReply::Done
            }
            ModuleCall::AiIsDoingGroundMovement => ModuleReply::Bool(self.answers.ground_movement),
            ModuleCall::PhysicsGetTurning => ModuleReply::Int(self.answers.turning),
            ModuleCall::HordeIsTrueMember => self.answers.horde.map_or(ModuleReply::Unhandled, ModuleReply::Bool),
            ModuleCall::HordeIsInHorde | ModuleCall::HordeHasFlag | ModuleCall::HordeIsAllowedNationalism => ModuleReply::Unhandled,
            ModuleCall::AiEvaluateMoraleBonus => {
                world.body_trace(format!("morale {}", me.object));
                ModuleReply::Done
            }
            ModuleCall::RadiusDecalCreate { .. } | ModuleCall::RadiusDecalKill | ModuleCall::RadiusDecalKillWhenNoLongerAttacking(_) => ModuleReply::Unhandled,
            ModuleCall::AiGetHealed { target, command_source } => {
                world.body_trace(format!("aiheal {} {target} {command_source}", me.object));
                ModuleReply::Done
            }
            ModuleCall::AiEnter { target, command_source } => {
                world.body_trace(format!("aienter {} {target} {command_source}", me.object));
                ModuleReply::Done
            }
            ModuleCall::AiFollowExitProductionPath { path, ignore_object, command_source } => {
                let pts: Vec<String> = path.iter().map(|p| format!("{:08x},{:08x},{:08x}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits())).collect();
                world.body_trace(format!("aiexitpath {} {ignore_object} {command_source} {}", me.object, pts.join(";")));
                ModuleReply::Done
            }
            ModuleCall::UnreserveDoorForExit(_)
            | ModuleCall::ExitObjectByBudding { .. }
            | ModuleCall::GetExitPosition
            | ModuleCall::GetNaturalRallyPointWithOffset(_) => ModuleReply::Unhandled,
            ModuleCall::AiConstruct { template, pos, angle, owner, is_rebuild } => {
                world.body_trace(format!(
                    "construct {} {} {:08x} {:08x} {:08x} {:08x} {} {}",
                    me.object,
                    template.map_or(-1, i32::from),
                    pos.x.to_bits(),
                    pos.y.to_bits(),
                    pos.z.to_bits(),
                    angle.to_bits(),
                    owner.unwrap_or(-1),
                    i32::from(is_rebuild)
                ));
                ModuleReply::Id(self.answers.construct_result)
            }
            ModuleCall::AiResumeConstruction { object, command_source } => {
                world.body_trace(format!("resume {} {object} {command_source}", me.object));
                ModuleReply::Done
            }
            ModuleCall::StartRebuildProcess { template, spawner } if self.answers.rebuild_hole => {
                world.body_trace(format!("startrebuild {} {template} {spawner}", me.object));
                ModuleReply::Done
            }
            ModuleCall::StickyBombGetTargetObject if self.answers.sticky_target.is_some() => ModuleReply::Id(self.answers.sticky_target.unwrap_or(INVALID_ID)),
            ModuleCall::StickyBombSetTargetObject(t) if self.answers.sticky_target.is_some() => {
                world.body_trace(format!("bombtarget {} {t}", me.object));
                self.answers.sticky_target = Some(t);
                ModuleReply::Done
            }
            ModuleCall::StickyBombGetTargetObject | ModuleCall::StickyBombSetTargetObject(_) => ModuleReply::Unhandled,
            ModuleCall::AiNotifyCrate(c) => {
                world.body_trace(format!("notifycrate {} {c}", me.object));
                ModuleReply::Done
            }
            ModuleCall::IsValidContainerFor { obj, check_capacity } if self.answers.parachute => {
                world.body_trace(format!("validfor {} {obj} {}", me.object, i32::from(check_capacity)));
                ModuleReply::Bool(self.answers.yes)
            }
            ModuleCall::AddToContain(obj) if self.answers.parachute => {
                world.body_trace(format!("addtocontain {} {obj}", me.object));
                ModuleReply::Done
            }
            ModuleCall::DozerCancelTask(task) => {
                world.body_trace(format!("canceltask {} {task}", me.object));
                ModuleReply::Done
            }
            ModuleCall::AiMoveToPosition { pos, command_source } => {
                world.body_trace(format!("aimove {} {:08x} {:08x} {:08x} {command_source}", me.object, pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()));
                ModuleReply::Done
            }
            ModuleCall::AiIgnoreObstacle { obstacle } => {
                world.body_trace(format!("aiignore {} {obstacle}", me.object));
                ModuleReply::Done
            }
            ModuleCall::AiGetEnterTarget => ModuleReply::Id(self.answers.enter_target),
            ModuleCall::AiSetIgnoreCollisionTime { frames } => {
                world.body_trace(format!("aiignorecoll {} {frames}", me.object));
                ModuleReply::Done
            }
            ModuleCall::AiFollowPath { path, ignore_object, command_source } => {
                let pts: Vec<String> = path.iter().map(|p| format!("{:08x},{:08x},{:08x}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits())).collect();
                world.body_trace(format!("aipath {} {ignore_object} {command_source} {}", me.object, pts.join(";")));
                ModuleReply::Done
            }
            ModuleCall::AiDoQuickExit { path } => {
                let pts: Vec<String> = path.iter().map(|p| format!("{:08x},{:08x},{:08x}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits())).collect();
                world.body_trace(format!("aiquick {} {}", me.object, pts.join(";")));
                ModuleReply::Done
            }
            ModuleCall::PhysicsGetMass => ModuleReply::Real(self.answers.mass),
            ModuleCall::PhysicsGetAllowToFall => ModuleReply::Bool(self.answers.can_fall),
            ModuleCall::StealthMarkAsDetected => {
                world.body_trace(format!("markdetected {}", me.object));
                ModuleReply::Done
            }
            // batch R26: StealthUpdate's calls
            ModuleCall::StealthMarkAsDetectedFor(frames) => {
                world.body_trace(format!("markdetected {} {frames}", me.object));
                ModuleReply::Done
            }
            ModuleCall::StealthDisguiseAsObject(_) => ModuleReply::Unhandled,
            ModuleCall::SpawnAreAllSlavesStealthed => ModuleReply::Bool(self.answers.slaves_stealthed),
            ModuleCall::SpawnRevealSlaves => {
                world.body_trace(format!("revealslaves {}", me.object));
                ModuleReply::Done
            }
            ModuleCall::AiGetLastCommandSource => ModuleReply::Int(self.answers.last_command_source),
            ModuleCall::PhysicsGetVelocityMagnitude => {
                let v = self.answers.velocity;
                ModuleReply::Real((v.x * v.x + v.y * v.y + v.z * v.z).sqrt())
            }
            ModuleCall::PhysicsResetDynamicPhysics => {
                world.body_trace(format!("resetphysics {}", me.object));
                ModuleReply::Done
            }
            ModuleCall::IsAnyRiderAttacking if self.answers.stealth_contain => ModuleReply::Bool(self.answers.yes),
            ModuleCall::RecalcApparentControllingPlayer if self.answers.stealth_contain => {
                world.body_trace(format!("recalcapparent {}", me.object));
                ModuleReply::Done
            }
            // the contain module's own interface: a probe is a damage and die module, and answers the contain questions as it did before (the scripted answers)
            ModuleCall::AddToContain(_)
            | ModuleCall::AddToContainList(_)
            | ModuleCall::RemoveFromContainExposing { .. }
            | ModuleCall::KillAllContained
            | ModuleCall::HarmAndForceExitAllContained(_)
            | ModuleCall::IsValidContainerFor { .. }
            | ModuleCall::OnContaining { .. }
            | ModuleCall::OnRemoving(_)
            | ModuleCall::OnSelling
            | ModuleCall::GetContainMax
            | ModuleCall::OrderAllPassengersToExit { .. }
            | ModuleCall::OrderAllPassengersToIdleFrom { .. }
            | ModuleCall::OrderAllPassengersToHackInternetFrom { .. }
            | ModuleCall::MarkAllPassengersDetected
            | ModuleCall::SetPassengerAllowedToFire(_)
            | ModuleCall::SetOverrideDestination(_)
            | ModuleCall::IsDisplayedOnControlBar
            | ModuleCall::GetExtraSlotsInUse
            | ModuleCall::GetContainedItemsMass
            | ModuleCall::GetStealthUnitsContained
            | ModuleCall::GetPlayerWhoEntered
            | ModuleCall::ProcessDamageToContained(_)
            | ModuleCall::EnableLoadSounds(_)
            | ModuleCall::SetEvacDisposition(_)
            | ModuleCall::IsWeaponBonusPassedToPassengers
            | ModuleCall::GetWeaponBonusPassedToPassengers
            | ModuleCall::IsBustable
            | ModuleCall::IsHealContain
            | ModuleCall::IsTunnelContain
            | ModuleCall::IsSpecialOverlordStyleContainer
            | ModuleCall::IsAnyRiderAttacking
            | ModuleCall::ClientVisibleContainedFlashAsSelected
            | ModuleCall::RecalcApparentControllingPlayer
            | ModuleCall::OnObjectWantsToEnterOrExit { .. }
            | ModuleCall::HasObjectsWantingToEnterOrExit
            | ModuleCall::IsExitBusy
            | ModuleCall::ExitObjectViaDoor { .. }
            | ModuleCall::ExitObjectInAHurry(_)
            | ModuleCall::SetRallyPoint(_)
            | ModuleCall::GetRallyPoint
            | ModuleCall::ScatterToNearbyPosition(_)
            | ModuleCall::RedeployOccupants
            | ModuleCall::PutObjAtNextFirePoint(_)
            | ModuleCall::DrainXferDestroys
            | ModuleCall::GetNaturalRallyPoint => ModuleReply::Unhandled,
            // --- end p1w13-sonnet ---
            // --- p1w12-sonnet --- (FlammableUpdate's test: the body's setAflame is a line of the trace, when `probe_effect_trace` asks)
            ModuleCall::StartFireSpreading => {
                if world.cuts.probe_effect_trace {
                    if let Some(t) = &mut world.cuts.trace {
                        t.push("spread".to_string());
                    }
                }
                ModuleReply::Done
            }
            ModuleCall::SetAflame(on) => {
                if world.cuts.probe_effect_trace {
                    if let Some(t) = &mut world.cuts.trace {
                        t.push(format!("aflame {}", i32::from(on)));
                    }
                }
                ModuleReply::Done
            }
            // --- end p1w12-sonnet ---
            ModuleCall::AttemptHealing(mut info) => {
                // --- p1w12-sonnet --- (AutoHealBehavior's test: the heal is a line of the trace, when `probe_effect_trace` asks)
                if world.cuts.probe_effect_trace {
                    if let Some(t) = &mut world.cuts.trace {
                        t.push(format!("heal {} {:08x} {}", me.object, info.input.amount.to_bits(), info.input.source_id));
                    }
                }
                // --- end p1w12-sonnet ---
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
// --- p1l9-upgrades ---
            ModuleCall::SetMaxHealthWithChange { max_health, .. } => {
                // the probe has no change type: the answer is the same as `SetMaxHealth`
                self.max_health = max_health;
                if self.health > max_health {
                    self.health = max_health;
                }
                ModuleReply::Done
            }
            // --- end p1l9-upgrades ---
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
            // --- p1w13-sonnet --- (batch R3: the dock of a railed transport; the line of the C++ stand-in)
            ModuleCall::DockIsOpen => ModuleReply::Bool(self.answers.dock_open),
            ModuleCall::StealthReceiveGrant { .. } => ModuleReply::Done,
            ModuleCall::AiChooseLocomotorSet(set) => {
                world.body_trace(format!("locoset {} {set}", me.object));
                ModuleReply::Done
            }
            // batch R8 (ParachuteContain): the AI's locomotor and the physics' free fall; the lines of the C++ stand-ins
            ModuleCall::AiHunt { command_source } => {
                world.body_trace(format!("aihunt {} {command_source}", me.object));
                ModuleReply::Done
            }
            ModuleCall::UseSpawnRallyPoint => ModuleReply::Bool(false),
            ModuleCall::TryToEvacuate { .. } | ModuleCall::HasCaveInterface | ModuleCall::SetOriginalTeam(_) | ModuleCall::TryToSetCaveIndex(_) => ModuleReply::Unhandled,
            ModuleCall::AiLocomotorSway(k) => ModuleReply::Real(self.answers.locomotor_sway[usize::from(k) & 3]),
            ModuleCall::AiLocomotorSetUltraAccurate(on) => {
                world.body_trace(format!("locoultra {} {}", me.object, i32::from(on)));
                ModuleReply::Done
            }
            ModuleCall::AiLocomotorSetCloseEnoughDist(d) => {
                world.body_trace(format!("lococlose {} {:08x}", me.object, d.to_bits()));
                ModuleReply::Done
            }
            ModuleCall::AiLocomotorSetCloseEnoughDist3D(on) => {
                world.body_trace(format!("lococlose3d {} {}", me.object, i32::from(on)));
                ModuleReply::Done
            }
            ModuleCall::PhysicsSetIsInFreeFall(on) => {
                world.body_trace(format!("pfreefall {} {}", me.object, i32::from(on)));
                self.answers.free_fall = on;
                ModuleReply::Done
            }
            ModuleCall::DockSetDockCrippled(c) => {
                world.body_trace(format!("crippled {} {}", me.object, i32::from(c)));
                ModuleReply::Done
            }
            ModuleCall::DockSetOpen(open) => {
                world.body_trace(format!("docksetopen {} {}", me.object, i32::from(open)));
                self.answers.dock_open = open;
                ModuleReply::Done
            }
            // --- end p1w13-sonnet ---
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
            | ModuleCall::OverchargeToggle
            | ModuleCall::OverchargeEnable(_)
            | ModuleCall::IsAbleToBeToppled
            | ModuleCall::SpecialAbilityIsActive // p1w13-sonnet
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
            // --- p1w7-sonnet ---
            ModuleCall::IsClearToApproach { .. } => ModuleReply::Bool(self.answers.clear_to_approach),
            ModuleCall::GetSupplyTruckPreferredDockId => {
                if self.answers.supply_truck {
                    ModuleReply::Id(self.answers.preferred_dock)
                } else {
                    ModuleReply::Unhandled
                }
            }
            ModuleCall::GetSupplyTruckWarehouseScanDistance => ModuleReply::Real(self.answers.scan_distance),
            // --- end p1w7-sonnet ---
            // ---- the ones with an effect ----
            ModuleCall::OnCollide { .. } => {
                if self.answers.collide_sets_no_collisions {
                    self.lend(world, me, |w| w.set_status(me.object, mask_of(OBJECT_STATUS_NO_COLLISIONS), true));
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
            // --- p1w10-sonnet ---
            | ModuleCall::OnBuildComplete
            | ModuleCall::OnSpecialPowerCreation
            | ModuleCall::OnDamage(_)
            | ModuleCall::SetLifetimeRange { .. }
            | ModuleCall::SetSmartBombTargetPosition(_)
            // --- end p1w10-sonnet ---
            | ModuleCall::OnDestroy
            | ModuleCall::OnDie(_)
            | ModuleCall::ForceRefreshUpgrade
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
            | ModuleCall::StopHealing // p1w12-sonnet
            | ModuleCall::ProneGoProne(_) // p1w13-sonnet
            | ModuleCall::FloatSetEnabled(_) // p1w13-sonnet
            | ModuleCall::SetConstructorObject(_) // p1w13-sonnet
            | ModuleCall::GetConstructorObjectId // p1w13-sonnet
            | ModuleCall::OclUpdateResetTimer // p1w12-sonnet
            | ModuleCall::OclUpdateGetCountdownPercent // p1w12-sonnet
            | ModuleCall::OclUpdateGetRemainingFrames // p1w12-sonnet
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
