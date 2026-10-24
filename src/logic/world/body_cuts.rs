// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file (this repository's own design, not upstream code):
// the cut points of `ActiveBody` and the bodies built on it, so that one
// module (`logic::modules::active_body`) can be translated line by line while
// the units it calls are layers 10 to 12. Each method names the upstream call
// it replaces.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this file), with the additional terms in NOTICE.md.

//! The calls `ActiveBody` makes on units that are not translated, or that cascade, as [`World`] methods and the tables that answer them.
//!
//! # What is cut and why
//!
//! `ActiveBody.cpp` reaches the game globals (`TheGlobalData`, `TheAI->getAiData()`), the particle system manager, the drawable's bones, the pathfinder,
//! the partition manager, `Object::getAbleToAttackSpecificObject`, the audio manager, the control bar and the in-game UI. Each is a method below, one line
//! of the upstream call, with an answer from [`BodyCutTables`] where upstream reads something back. The `ControlBar` and `InGameUI` reads are state of the
//! world already (`World::reads`, `ObjectClientEvent::ControlBarMarkUiDirty`).
//!
//! # The Object-level calls, and the explicit ablation
//!
//! `setDisabled`, `clearDisabled`, `doStatusDamage`, `notifySubdualDamage`, `scoreTheKill`, `onDie`, `setTeam`, `setStatus` and the neutral player's
//! academy statistics are translated (`object_state`, `object_damage`, ...) and **run for real by default** (`body_*` below call them). Each of them
//! cascades (sounds, special power pauses, die modules, the radar, the team's members), and every cascade has its own differential test; the test of
//! `ActiveBody` therefore sets [`BodyCutTables::stub_object_calls`], which replaces each of these calls by a [`BodyCall`] record alone. That is an
//! explicit ablation, off by default, in the spirit of `ObjectCuts::stub_helpers`; the object's state that `ActiveBody` itself reads back
//! (the effectively dead flag, the status bits, the geometry) is not stubbed.
//!
//! `p1w13-sonnet`, 2026-10-21.

use std::collections::{BTreeMap, BTreeSet};

use crate::common::base_type::Coord3D;
use crate::common::game_type::ObjectId;
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::logic::damage::DamageInfo;
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::{PlayerIndex, TeamId};
use crate::logic::world::object_cuts::CutCall;

/// One auto particle system class of `TheGlobalData` (`m_autoFireParticleSmallPrefix`, `...System`, `...Max`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AutoParticle {
    /// The bone name prefix.
    pub prefix: Vec<u8>,
    /// The particle system template's name.
    pub system: Vec<u8>,
    /// The most systems made.
    pub max: i32,
}

/// The values of `TheGlobalData` and `TheAI->getAiData()` that `ActiveBody` reads. The defaults are the constructors' (`GlobalData::GlobalData`,
/// `TAiData::TAiData`).
#[derive(Clone, Debug, PartialEq)]
pub struct BodyGlobals {
    /// `m_unitDamagedThresh`.
    pub unit_damaged_thresh: f32,
    /// `m_unitReallyDamagedThresh`.
    pub unit_really_damaged_thresh: f32,
    /// `m_defaultStructureRubbleHeight`.
    pub default_structure_rubble_height: f32,
    /// `m_healthBonus[LEVEL_COUNT]`.
    pub health_bonus: [f32; 4],
    /// `m_autoFireParticle{Small,Medium,Large}*`.
    pub auto_fire: [AutoParticle; 3],
    /// `m_autoSmokeParticle{Small,Medium,Large}*`.
    pub auto_smoke: [AutoParticle; 3],
    /// `m_autoAflameParticle*`.
    pub auto_aflame: AutoParticle,
    /// `TheAI->getAiData()->m_retaliateFriendsRadius`.
    pub retaliate_friends_radius: f32,
    /// `TheAI->getAiData()->m_maxRetaliateDistance`.
    pub max_retaliate_distance: f32,
}

impl Default for BodyGlobals {
    fn default() -> Self {
        BodyGlobals {
            unit_damaged_thresh: 0.5,
            unit_really_damaged_thresh: 0.1,
            default_structure_rubble_height: 1.0,
            health_bonus: [1.0; 4],
            auto_fire: Default::default(),
            auto_smoke: Default::default(),
            auto_aflame: AutoParticle::default(),
            retaliate_friends_radius: 120.0,
            max_retaliate_distance: 210.0,
        }
    }
}

/// The tables behind the cut points of `ActiveBody`, and the switches of its test. See the module docs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BodyCutTables {
    /// A probe module logs the damage module, contain, AI and projectile calls it receives as `bt ...` lines of `ObjectCuts::trace`.
    pub probe_trace: bool,
    /// The explicit ablation of the module docs: the Object-level calls are records only.
    pub stub_object_calls: bool,
    /// `TheGlobalData` and `TheAI->getAiData()`.
    pub globals: BodyGlobals,
    /// The bones of a drawable: `getPristineBonePositions(prefix, 1, ..)` of object and bone name prefix.
    pub bones: BTreeMap<(ObjectId, Vec<u8>), Vec<Coord3D>>,
    /// The particle system templates `TheParticleSystemManager->findTemplate(name)` finds (by name).
    pub particle_templates: BTreeSet<Vec<u8>>,
    /// The particle system ids `findParticleSystem(id)` no longer finds: those with `id % modulus == remainder` (a modulus of 0: none).
    pub dead_particle_modulus: (u32, u32),
    /// `getAbleToAttackSpecificObject(ATTACK_NEW_TARGET, victim, CMD_FROM_AI)`, by `(object, victim)`, as the `CanAttackResult` value (default: not possible).
    pub able_to_attack: BTreeMap<(ObjectId, ObjectId), i32>,
    /// `getDistanceSquared(object, other, FROM_BOUNDINGSPHERE_2D)`, by `(object, other)` (default 0).
    pub distance_squared: BTreeMap<(ObjectId, ObjectId), f32>,
    /// The objects `iterateObjectsInRange(position, range, FROM_CENTER_2D, {allied to the player, on the map}, ITER_FASTEST)` visits, in order.
    pub retaliation_friends: Vec<ObjectId>,
}

/// What `ActiveBody` did to a unit that is not translated, or that is run for real but cascades. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub enum BodyCall {
    /// `TheGameLogic->destroyObject(obj)` (`SlowDeathBehavior`).
    DestroyObject { object: ObjectId },
    /// `obj->setDisabled(type)`.
    SetDisabled {
        object: ObjectId,
        disabled_type: i32,
    },
    /// batch R33: `obj->setDisabledUntil(type, frame)` (`EMPUpdate`, `LeafletDropBehavior`).
    SetDisabledUntil {
        object: ObjectId,
        disabled_type: i32,
        frame: u32,
    },
    /// `me->clearDisabled(type)`.
    ClearDisabled {
        object: ObjectId,
        disabled_type: i32,
    },
    /// `obj->doStatusDamage(status, frames)`.
    DoStatusDamage {
        object: ObjectId,
        status: i32,
        frames: i32,
    },
    /// `getObject()->notifySubdualDamage(amount)`.
    NotifySubdualDamage { object: ObjectId, amount: f32 },
    /// `damager->scoreTheKill(victim)`.
    ScoreTheKill { killer: ObjectId, victim: ObjectId },
    /// `obj->onDie(damageInfo)`.
    OnDie { object: ObjectId, info: DamageInfo },
    /// `obj->setTeam(team)`.
    SetTeam { object: ObjectId, team: TeamId },
    /// `obj->setStatus(mask)` (`set` true) or `clearStatus`.
    SetStatus {
        object: ObjectId,
        mask: ObjectStatusMaskType,
        set: bool,
    },
    /// `ThePlayerList->getNeutralPlayer()->getAcademyStats()->recordVehicleSniped()`.
    AcademyVehicleSniped,
    /// `thingToKill->getControllingPlayer()->getAcademyStats()->recordClearedGarrisonedBuilding()`.
    AcademyClearedGarrisonedBuilding { player: PlayerIndex },
    /// `obj->getMultiLogicalBonePosition(prefix, max, positions, NULL, FALSE)`.
    GetMultiLogicalBonePosition {
        object: ObjectId,
        prefix: Vec<u8>,
        max: i32,
    },
    /// `TheParticleSystemManager->findTemplate(name)`; `found` is the answer.
    ParticleFindTemplate { name: Vec<u8>, found: bool },
    /// `particleSystem->attachToObject(obj)`.
    ParticleAttachToObject { id: u32, object: ObjectId },
    /// `particleSystem = findParticleSystem(id); if (particleSystem) particleSystem->destroy()`; `found` is whether the manager still has it.
    ParticleDestroy { id: u32, found: bool },
    /// `object->getAbleToAttackSpecificObject(ATTACK_NEW_TARGET, victim, CMD_FROM_AI)`.
    GetAbleToAttackSpecificObject { object: ObjectId, victim: ObjectId },
    /// `ThePartitionManager->getDistanceSquared(object, other, FROM_BOUNDINGSPHERE_2D)`.
    GetDistanceSquared { object: ObjectId, other: ObjectId },
    /// `ThePartitionManager->iterateObjectsInRange(position, range, FROM_CENTER_2D, filters, ITER_FASTEST)` with `PartitionFilterPlayerAffiliation(player,
    /// ALLOW_ALLIES, true)` and `PartitionFilterOnMap`.
    IterateRetaliationFriends {
        position: Coord3D,
        range: f32,
        player: PlayerIndex,
    },
}

/// The summary of a `DamageInfo` in the lines of a probe module's trace and a differential test's log: source, damage type, death type, damage FX override,
/// amount, damage dealt, damage clipped (float bits), no-effect, kill, damage status type.
pub fn info_line(info: &DamageInfo) -> String {
    format!(
        "{} {} {} {} {:08x} {:08x} {:08x} {} {} {}",
        info.input.source_id,
        info.input.damage_type,
        info.input.death_type,
        info.input.damage_fx_override,
        info.input.amount.to_bits(),
        info.out.actual_damage_dealt.to_bits(),
        info.out.actual_damage_clipped.to_bits(),
        i32::from(info.out.no_effect),
        i32::from(info.input.kill),
        info.input.damage_status_type
    )
}

impl World {
    /// A line of the probe modules' effect trace (`bt ...`), when [`BodyCutTables::probe_trace`] asks.
    pub fn body_trace(&mut self, line: String) {
        if !self.cuts.body.probe_trace {
            return;
        }
        if let Some(t) = &mut self.cuts.trace {
            t.push(format!("bt {line}"));
        }
    }

    fn body_cut(&mut self, call: BodyCall) {
        self.cut(CutCall::Body(call));
    }

    /// `TheGameLogic->destroyObject(obj)`.
    pub fn body_destroy_object(&mut self, id: ObjectId) {
        self.body_cut(BodyCall::DestroyObject { object: id });
        if !self.cuts.body.stub_object_calls {
            self.destroy_object(id);
        }
    }

    /// `obj->setDisabled(type)`.
    pub fn body_set_disabled(&mut self, id: ObjectId, disabled_type: i32) {
        self.body_cut(BodyCall::SetDisabled {
            object: id,
            disabled_type,
        });
        if !self.cuts.body.stub_object_calls {
            self.set_disabled(id, disabled_type);
        }
    }

    /// batch R33: `obj->setDisabledUntil(type, frame)`.
    pub fn body_set_disabled_until(&mut self, id: ObjectId, disabled_type: i32, frame: u32) {
        self.body_cut(BodyCall::SetDisabledUntil {
            object: id,
            disabled_type,
            frame,
        });
        if !self.cuts.body.stub_object_calls {
            self.set_disabled_until(id, disabled_type, frame);
        }
    }

    /// `obj->clearDisabled(type)`.
    pub fn body_clear_disabled(&mut self, id: ObjectId, disabled_type: i32) {
        self.body_cut(BodyCall::ClearDisabled {
            object: id,
            disabled_type,
        });
        if !self.cuts.body.stub_object_calls {
            self.clear_disabled(id, disabled_type);
        }
    }

    /// `obj->doStatusDamage(status, frames)`; the duration reaches the helper as a `Real`.
    pub fn body_do_status_damage(&mut self, id: ObjectId, status: i32, frames: i32) {
        self.body_cut(BodyCall::DoStatusDamage {
            object: id,
            status,
            frames,
        });
        if !self.cuts.body.stub_object_calls {
            self.do_status_damage(id, status, frames as f32);
        }
    }

    /// `obj->notifySubdualDamage(amount)`.
    pub fn body_notify_subdual_damage(&mut self, id: ObjectId, amount: f32) {
        self.body_cut(BodyCall::NotifySubdualDamage { object: id, amount });
        if !self.cuts.body.stub_object_calls {
            self.notify_subdual_damage(id, amount);
        }
    }

    /// `killer->scoreTheKill(victim)`.
    pub fn body_score_the_kill(&mut self, killer: ObjectId, victim: ObjectId) {
        self.body_cut(BodyCall::ScoreTheKill { killer, victim });
        if !self.cuts.body.stub_object_calls {
            self.score_the_kill(killer, victim);
        }
    }

    /// `obj->onDie(damageInfo)`.
    pub fn body_on_die(&mut self, id: ObjectId, info: &DamageInfo) {
        self.body_cut(BodyCall::OnDie {
            object: id,
            info: info.clone(),
        });
        if !self.cuts.body.stub_object_calls {
            self.on_die(id, info);
        }
    }

    /// `obj->setTeam(ThePlayerList->getNeutralPlayer()->getDefaultTeam())`.
    pub fn body_set_team_to_neutral_default(&mut self, id: ObjectId) {
        let team = self.cuts.neutral_default_team;
        self.body_cut(BodyCall::SetTeam { object: id, team });
        if !self.cuts.body.stub_object_calls {
            self.set_team(id, Some(team));
        }
    }

    /// `obj->setStatus(mask)` (`set`) or `obj->clearStatus(mask)`.
    pub fn body_set_status(&mut self, id: ObjectId, mask: ObjectStatusMaskType, set: bool) {
        self.body_cut(BodyCall::SetStatus {
            object: id,
            mask,
            set,
        });
        if self.cuts.body.stub_object_calls {
            // the bits alone: the repulsor helper, the stealth relook and the construction mine sweep are `set_status`'s and have their own test
            if let Some(o) = self.objects.find_mut(id) {
                if set {
                    o.status.set_mask(&mask);
                } else {
                    o.status.clear_mask(&mask);
                }
            }
        } else {
            self.set_status(id, mask, set);
        }
    }

    /// `ThePlayerList->getNeutralPlayer()->getAcademyStats()->recordVehicleSniped()`.
    pub fn body_academy_vehicle_sniped(&mut self) {
        self.body_cut(BodyCall::AcademyVehicleSniped);
        if !self.cuts.body.stub_object_calls {
            if let Some(p) = self.players.get_nth_mut(0) {
                p.academy_stats.record_vehicle_sniped();
            }
        }
    }

    /// `player->getAcademyStats()->recordClearedGarrisonedBuilding()`.
    pub fn body_academy_cleared_garrisoned_building(&mut self, player: PlayerIndex) {
        self.body_cut(BodyCall::AcademyClearedGarrisonedBuilding { player });
        if !self.cuts.body.stub_object_calls {
            if let Some(p) = self.players.get_nth_mut(player) {
                p.academy_stats.record_cleared_garrisoned_building();
            }
        }
    }

    /// `obj->getMultiLogicalBonePosition(prefix, max, positions, NULL, FALSE)`: no bones for an object without a drawable
    /// (`get_multi_logical_bone_position`), else the bones of the table, at most `max`.
    pub fn body_get_multi_logical_bone_position(
        &mut self,
        id: ObjectId,
        prefix: &[u8],
        max: i32,
    ) -> Vec<Coord3D> {
        self.body_cut(BodyCall::GetMultiLogicalBonePosition {
            object: id,
            prefix: prefix.to_vec(),
            max,
        });
        let has_drawable = self
            .objects
            .find(id)
            .is_some_and(|o| o.drawable != crate::common::game_type::INVALID_DRAWABLE_ID);
        if !has_drawable {
            return Vec::new();
        }
        let mut bones = self
            .cuts
            .body
            .bones
            .get(&(id, prefix.to_vec()))
            .cloned()
            .unwrap_or_default();
        bones.truncate(max.max(0) as usize);
        bones
    }

    /// `TheParticleSystemManager->findTemplate(name) != NULL`.
    pub fn body_particle_find_template(&mut self, name: &[u8]) -> bool {
        let found = self.cuts.body.particle_templates.contains(name);
        self.body_cut(BodyCall::ParticleFindTemplate {
            name: name.to_vec(),
            found,
        });
        found
    }

    /// `particleSystem->attachToObject(obj)`.
    pub fn body_particle_attach_to_object(&mut self, id: u32, object: ObjectId) {
        self.body_cut(BodyCall::ParticleAttachToObject { id, object });
    }

    /// batch R32: `TheParticleSystemManager->findParticleSystem(id) != NULL` (the client's: see `BodyCutTables::dead_particle_modulus`).
    pub fn body_particle_exists(&self, id: u32) -> bool {
        let (modulus, remainder) = self.cuts.body.dead_particle_modulus;
        !(modulus != 0 && id % modulus == remainder)
    }

    /// `findParticleSystem(id)` and, if the manager still has it, `destroy()`.
    pub fn body_particle_destroy(&mut self, id: u32) {
        let (modulus, remainder) = self.cuts.body.dead_particle_modulus;
        let found = !(modulus != 0 && id % modulus == remainder);
        self.body_cut(BodyCall::ParticleDestroy { id, found });
    }

    /// `object->getAbleToAttackSpecificObject(ATTACK_NEW_TARGET, victim, CMD_FROM_AI)`: a `CanAttackResult` value.
    pub fn body_get_able_to_attack_specific_object(
        &mut self,
        object: ObjectId,
        victim: ObjectId,
    ) -> i32 {
        self.body_cut(BodyCall::GetAbleToAttackSpecificObject { object, victim });
        self.cuts
            .body
            .able_to_attack
            .get(&(object, victim))
            .copied()
            .unwrap_or(0)
    }

    /// `ThePartitionManager->getDistanceSquared(object, other, FROM_BOUNDINGSPHERE_2D)`.
    pub fn body_get_distance_squared(&mut self, object: ObjectId, other: ObjectId) -> f32 {
        self.body_cut(BodyCall::GetDistanceSquared { object, other });
        self.cuts
            .body
            .distance_squared
            .get(&(object, other))
            .copied()
            .unwrap_or(0.0)
    }

    /// The retaliation scan of `ActiveBody::attemptDamage`: the objects in range allied to `player` and on the map, in iteration order.
    pub fn body_iterate_retaliation_friends(
        &mut self,
        position: Coord3D,
        range: f32,
        player: PlayerIndex,
    ) -> Vec<ObjectId> {
        self.body_cut(BodyCall::IterateRetaliationFriends {
            position,
            range,
            player,
        });
        self.cuts.body.retaliation_friends.clone()
    }
}
