// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AI.cpp (lines 504-897: `findClosestEnemy`, `findClosestAlly`,
//     `findClosestRepulsor`, `getAdjustedVisionRangeForObject`, `priorityFunc`; lines 899-950: `TAiData::TAiData`)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AI.h (`TAiData`, the query qualifiers)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Michael S. Booth, November 2000
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The AI system's queries (`p1w13-sonnet`, layer 10, batch A26): the closest enemy (with an attack priority list), ally and
//! repulsor through the partition manager, and an object's vision range adjusted for its owner, guard ring and mood; and the AI's
//! tunable data, [`TAiData`], with upstream's constructor values.
//!
//! The queries take the partition manager and the AI data explicitly (upstream's `ThePartitionManager` and `TheAI->getAiData()`);
//! what they ask of objects is the [`AiQueryHost`]'s answer, a [`PartitionFilterHost`] with the AI's questions added. AI.cpp's two
//! private filters are [`PartitionFilter::LiveMapEnemies`] and [`PartitionFilter::WithinAttackRange`].
//!
//! Not here yet (batch A27): `TAiData`'s INI table, side info and build lists, `crc` and `xfer`; the AI's group list, formation ids,
//! `reset`, `newOverride`.

use crate::common::game_common::{CommandSourceType, ATTACK_NEW_TARGET};
use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_STEALTHED};
use crate::logic::ai::ai_update_types::{
    AI_VISIONFACTOR_GUARDINNER, AI_VISIONFACTOR_MOOD, AI_VISIONFACTOR_OWNERTYPE, MM_CONTROLLER_PLAYER, MM_MOOD_AGGRESSIVE, MM_MOOD_ALERT,
    MM_MOOD_BITMASK, MM_MOOD_SLEEP,
};
use crate::logic::modules::plant;
use crate::logic::object_iter::IterOrderType;
use crate::logic::world::ids::ObjectId;
use crate::logic::world::partition::{DistanceCalculationType, PartitionFilter, PartitionFilterHost, PartitionManager};

/// The qualifiers of `findClosestEnemy` and `findClosestAlly` (AI.h).
pub const CAN_SEE: u32 = 1 << 0;
pub const CAN_ATTACK: u32 = 1 << 1;
pub const IGNORE_INSIGNIFICANT_BUILDINGS: u32 = 1 << 2;
pub const ATTACK_BUILDINGS: u32 = 1 << 3;
pub const WITHIN_ATTACK_RANGE: u32 = 1 << 4;
pub const UNFOGGED: u32 = 1 << 5;

/// `LOGICFRAMES_PER_SECOND` (the guard scan rates' defaults).
const LOGICFRAMES_PER_SECOND: u32 = 30;

/// `TAiData`: the AI's tunable data (`AIData.ini`), here its scalar members with upstream's constructor values (the side info and
/// build lists are batch A27's).
#[derive(Clone, Debug, PartialEq)]
pub struct TAiData {
    pub structure_seconds: f32,
    pub team_seconds: f32,
    pub resources_wealthy: i32,
    pub resources_poor: i32,
    pub force_idle_frames_count: u32,
    pub structures_wealthy_mod: f32,
    pub team_wealthy_mod: f32,
    pub structures_poor_mod: f32,
    pub team_poor_mod: f32,
    pub team_resources_to_build: f32,
    pub guard_inner_modifier_ai: f32,
    pub guard_outer_modifier_ai: f32,
    pub guard_inner_modifier_human: f32,
    pub guard_outer_modifier_human: f32,
    pub guard_chase_unit_frames: u32,
    pub guard_enemy_scan_rate: u32,
    pub guard_enemy_return_scan_rate: u32,
    pub skirmish_group_fudge_value: f32,
    pub repulsed_distance: f32,
    pub enable_repulsors: bool,
    pub alert_range_modifier: f32,
    pub aggressive_range_modifier: f32,
    pub force_skirmish_ai: bool,
    pub rotate_skirmish_bases: bool,
    pub attack_uses_line_of_sight: bool,
    pub attack_ignore_insignificant_buildings: bool,
    pub attack_priority_distance_modifier: f32,
    pub max_recruit_distance: f32,
    pub skirmish_base_defense_extra_distance: f32,
    pub wall_height: f32,
    pub min_infantry_for_group: i32,
    pub min_vehicles_for_group: i32,
    pub min_distance_for_group: f32,
    pub distance_requires_group: f32,
    pub min_clump_density: f32,
    pub infantry_pathfind_diameter: i32,
    pub vehicle_pathfind_diameter: i32,
    pub rebuild_delay_seconds: i32,
    pub supply_center_safe_radius: f32,
    pub ai_dozer_bored_radius_modifier: f32,
    pub ai_crushes_infantry: bool,
    pub max_retaliate_distance: f32,
    pub retaliate_friends_radius: f32,
}

impl Default for TAiData {
    /// `TAiData::TAiData()`.
    fn default() -> Self {
        TAiData {
            structure_seconds: 0.0,
            team_seconds: 0.0,
            resources_wealthy: 0,
            resources_poor: 0,
            force_idle_frames_count: 1,
            structures_wealthy_mod: 0.0,
            team_wealthy_mod: 0.0,
            structures_poor_mod: 0.0,
            team_poor_mod: 0.0,
            team_resources_to_build: 0.0,
            guard_inner_modifier_ai: 0.0,
            guard_outer_modifier_ai: 0.0,
            guard_inner_modifier_human: 0.0,
            guard_outer_modifier_human: 0.0,
            guard_chase_unit_frames: 0,
            guard_enemy_scan_rate: LOGICFRAMES_PER_SECOND / 2,
            guard_enemy_return_scan_rate: LOGICFRAMES_PER_SECOND,
            skirmish_group_fudge_value: 0.0,
            repulsed_distance: 0.0,
            enable_repulsors: false,
            alert_range_modifier: 0.0,
            aggressive_range_modifier: 0.0,
            force_skirmish_ai: false,
            rotate_skirmish_bases: false,
            attack_uses_line_of_sight: true,
            attack_ignore_insignificant_buildings: false,
            attack_priority_distance_modifier: 0.0,
            max_recruit_distance: 0.0,
            skirmish_base_defense_extra_distance: 0.0,
            wall_height: 0.0,
            min_infantry_for_group: 3,
            min_vehicles_for_group: 4,
            min_distance_for_group: 100.0,
            distance_requires_group: 0.0,
            min_clump_density: 0.5,
            infantry_pathfind_diameter: 6,
            vehicle_pathfind_diameter: 6,
            rebuild_delay_seconds: 10,
            supply_center_safe_radius: 250.0,
            ai_dozer_bored_radius_modifier: 2.0,
            ai_crushes_infantry: true,
            max_retaliate_distance: 210.0,
            retaliate_friends_radius: 120.0,
        }
    }
}

/// What the AI's queries ask of the objects beyond the partition's filters.
pub trait AiQueryHost: PartitionFilterHost {
    /// The object's template (an opaque key the attack priority lists are read with).
    fn template_of(&self, id: ObjectId) -> i32;
    /// `info->getPriority(template)` for the attack priority list `info`.
    fn attack_priority(&self, info: i32, template: i32) -> i32;
    /// Whether `info` is `TheScriptEngine->getDefaultAttackInfo()`.
    fn is_default_attack_info(&self, info: i32) -> bool;
    /// The templates of the objects the object's contain holds, in its order (`iterateContained` forward; none without a contain).
    fn contained_templates(&self, id: ObjectId) -> Vec<i32>;
    /// `getVisionRange()`.
    fn vision_range(&self, id: ObjectId) -> f32;
    /// `getAI()` (its existence) and `getMoodMatrixValue()`.
    fn has_ai(&self, id: ObjectId) -> bool;
    fn mood_matrix_value(&self, id: ObjectId) -> u32;
    /// `getContainedBy() != NULL`.
    fn is_contained(&self, id: ObjectId) -> bool;
    /// `getLargestWeaponRange()`.
    fn largest_weapon_range(&self, id: ObjectId) -> f32;
}

/// C's `(Int)f`: truncation, and the x86 "integer indefinite" `0x80000000` for NaN or a value outside `Int`.
fn c_real_to_int(f: f32) -> i32 {
    if f.is_nan() || f64::from(f) < -2_147_483_648.0 || f64::from(f) >= 2_147_483_648.0 {
        return i32::MIN;
    }
    f as i32
}

/// `findClosestEnemy(me, range, qualifiers, info, optionalFilter)`: the closest live, on-map enemy that passes the qualifiers'
/// filters (in upstream's order) and the optional one; with an attack priority list other than the default, the one whose
/// priority (raised to its passengers' best), less a point per `m_attackPriorityDistanceModifier` of distance (at least 1), is
/// highest, ties to the higher raw priority, the nearest first. `None` for none.
///
/// `me` must have a controlling player (upstream reads its index for the fog filter before asking whether it is needed).
#[allow(clippy::too_many_arguments)]
pub fn find_closest_enemy<H: AiQueryHost>(
    pm: &mut PartitionManager,
    h: &H,
    data: &TAiData,
    me: ObjectId,
    range: f32,
    qualifiers: u32,
    info: Option<i32>,
    optional_filter: Option<PartitionFilter>,
) -> Option<ObjectId> {
    if qualifiers & CAN_ATTACK != 0 && !h.is_able_to_attack(me) {
        return None; // "PartitionFilterPossibleToAttack would filter out everything anyway"
    }
    let player_index = h.controlling_player(me).expect("findClosestEnemy: me has a controlling player");
    let mut filters = vec![PartitionFilter::LiveMapEnemies { obj: me }];
    if qualifiers & ATTACK_BUILDINGS == 0 {
        filters.push(PartitionFilter::reject_buildings(h, me));
    }
    if qualifiers & WITHIN_ATTACK_RANGE != 0 {
        filters.push(PartitionFilter::WithinAttackRange { obj: me });
    }
    if qualifiers & CAN_SEE != 0 {
        filters.push(PartitionFilter::LineOfSight { obj: me });
    }
    if qualifiers & CAN_ATTACK != 0 {
        filters.push(PartitionFilter::PossibleToAttack { attack_type: ATTACK_NEW_TARGET, obj: Some(me), command_source: CommandSourceType::FromAi });
    }
    if qualifiers & UNFOGGED != 0 {
        filters.push(PartitionFilter::FreeOfFog { comparison_index: player_index });
    }
    if qualifiers & IGNORE_INSIGNIFICANT_BUILDINGS != 0 {
        filters.push(PartitionFilter::InsignificantBuildings { allow_non_buildings: true, allow_insignificant: false });
    }
    if !plant(3560) {
        filters.push(PartitionFilter::StealthedAndUndetected { obj: me, allow: false });
    }
    if let Some(f) = optional_filter {
        filters.push(f);
    }
    let dc = DistanceCalculationType::FromBoundingSphere2D;
    let info = match info {
        Some(i) if !h.is_default_attack_info(i) => i,
        // "No additional attack info, so just return the closest one."
        _ => return pm.get_closest_object(h, me, range, dc, &filters).object,
    };
    let mut best_enemy = None;
    let mut effective_priority = 0i32;
    let mut actual_priority = 0i32;
    let mut iter = pm.iterate_objects_in_range(h, me, range, dc, &filters, IterOrderType::SortedNearToFar);
    let mut e = iter.first();
    while let Some(enemy) = e {
        e = iter.next();
        let mut cur_priority = h.attack_priority(info, h.template_of(enemy));
        if cur_priority == 0 {
            continue; // "don't attack 0 priority targets."
        }
        // "check for garrisoned buildings/vehicles & see if a higher priority unit is inside."
        let inside = h.contained_templates(enemy);
        if h.contain_facts(enemy).is_some() {
            let mut p = cur_priority;
            for t in inside {
                let q = h.attack_priority(info, t);
                if q > p && !plant(3561) {
                    p = q;
                }
            }
            if p > cur_priority {
                cur_priority = p;
            }
        }
        let (dist_sqr, _) = pm.get_distance_squared(h, me, enemy, dc).expect("both objects exist");
        let dist = petir_sqrt(dist_sqr);
        let modifier = c_real_to_int(dist / data.attack_priority_distance_modifier);
        let mut mod_priority = cur_priority.wrapping_sub(modifier);
        if mod_priority < 1 {
            mod_priority = 1;
        }
        if mod_priority > effective_priority {
            effective_priority = mod_priority;
            actual_priority = cur_priority;
            best_enemy = Some(enemy);
        }
        if mod_priority == effective_priority && cur_priority > actual_priority && !plant(3562) {
            effective_priority = mod_priority;
            actual_priority = cur_priority;
            best_enemy = Some(enemy);
        }
    }
    best_enemy
}

/// `(Real)sqrt(distSqr)`: the `double` square root.
fn petir_sqrt(x: f32) -> f32 {
    use petir::Real;
    Real::sqrt(f64::from(x)) as f32
}

/// `findClosestAlly(me, range, qualifiers)`: the closest live ally, not a building (unless it may be acquired), on the map as `me`
/// is (and seen, for `CAN_SEE`).
pub fn find_closest_ally<H: AiQueryHost>(pm: &mut PartitionManager, h: &H, me: ObjectId, range: f32, qualifiers: u32) -> Option<ObjectId> {
    let mut filters = vec![
        PartitionFilter::reject_buildings(h, me),
        PartitionFilter::Relationship { obj: me, flags: crate::logic::world::partition::filters::ALLOW_ALLIES },
        PartitionFilter::Alive,
        PartitionFilter::SameMapStatus { obj: me },
    ];
    if qualifiers & CAN_SEE != 0 && !plant(3563) {
        filters.push(PartitionFilter::LineOfSight { obj: me });
    }
    pm.get_closest_object(h, me, range, DistanceCalculationType::FromBoundingSphere2D, &filters).object
}

/// `findClosestRepulsor(me, range)`: the closest repulsor not stealthed (or else detected or disguised); none when repulsors are
/// off.
pub fn find_closest_repulsor<H: AiQueryHost>(pm: &mut PartitionManager, h: &H, data: &TAiData, me: ObjectId, range: f32) -> Option<ObjectId> {
    if !data.enable_repulsors {
        return None;
    }
    let mut must_be_set = ObjectStatusMaskType::new();
    must_be_set.set(OBJECT_STATUS_STEALTHED, 1);
    let mut must_be_clear = ObjectStatusMaskType::new();
    must_be_clear.set(OBJECT_STATUS_DETECTED, 1);
    if !plant(3564) {
        must_be_clear.set(OBJECT_STATUS_DISGUISED, 1);
    }
    let filters = [PartitionFilter::Repulsor { self_obj: me }, PartitionFilter::RejectByObjectStatus { must_be_set, must_be_clear }];
    pm.get_closest_object(h, me, range, DistanceCalculationType::FromBoundingSphere2D, &filters).object
}

/// `getAdjustedVisionRangeForObject(object, factorsToConsider)`: the vision range scaled by the guard ring's modifier for a human's
/// or the AI's unit; a contained object's is its largest weapon range; an AI-controlled one's by its mood (none asleep). 0 without
/// an AI.
pub fn get_adjusted_vision_range_for_object<H: AiQueryHost>(h: &H, data: &TAiData, object: ObjectId, factors: i32) -> f32 {
    let mut original_range = h.vision_range(object);
    if !h.has_ai(object) {
        return 0.0;
    }
    let mood = h.mood_matrix_value(object);
    if factors & AI_VISIONFACTOR_OWNERTYPE != 0 {
        let player_is_human = mood & MM_CONTROLLER_PLAYER != 0;
        if player_is_human {
            if factors & AI_VISIONFACTOR_GUARDINNER != 0 {
                original_range *= data.guard_inner_modifier_human;
            } else {
                original_range *= data.guard_outer_modifier_human;
            }
        } else if factors & AI_VISIONFACTOR_GUARDINNER != 0 {
            original_range *= data.guard_inner_modifier_ai;
        } else {
            original_range *= data.guard_outer_modifier_ai;
        }
    }
    if h.is_contained(object) && !plant(3565) {
        original_range = h.largest_weapon_range(object);
    } else if factors & AI_VISIONFACTOR_MOOD != 0 && mood & MM_CONTROLLER_PLAYER == 0 {
        match mood & MM_MOOD_BITMASK {
            MM_MOOD_SLEEP => return 0.0,
            MM_MOOD_ALERT => original_range *= data.alert_range_modifier,
            MM_MOOD_AGGRESSIVE => original_range *= data.aggressive_range_modifier,
            // (passive, normal, and a value that is no mood: unchanged)
            _ => {}
        }
    }
    original_range
}
