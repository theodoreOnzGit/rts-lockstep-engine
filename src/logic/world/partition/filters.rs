// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/PartitionManager.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/PartitionManager.cpp
//   (`PartitionFilter` and its 33 subclasses, `filtersAllow`)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Line2D.cpp
//   (`IntersectLine2D`, `PointInsideArea2D`, the two functions the irregular
//   area filter calls)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Steven Johnson, September 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `PartitionFilter`: the predicates a query passes to the partition manager
//! to drop objects it does not want.
//!
//! Upstream is an abstract class with 33 subclasses and a null-terminated
//! `PartitionFilter **` array; here it is one enum ([`PartitionFilter`]) and
//! a slice, and `allow` is a `match`. A filter holds *ids* where upstream
//! holds `Object *`, `Player *`, `Team *` and so on (players are indices,
//! teams `TeamId`s). What a filter asks of an object is the
//! [`PartitionFilterHost`]'s answer; the decision logic of each filter is
//! upstream's, branch for branch.
//!
//! Filters are tried in array order and the first refusal wins
//! ([`filters_allow`]); the order is part of the behaviour only through side
//! effects (`PartitionFilterFreeOfFog` recomputes a shroudedness).

use super::collide::{self, DistanceCalculationType, HUGE_DIST_SQR};
use super::host::{CanAttackResult, PartitionFilterHost};
use super::manager::PartitionManager;
use crate::common::base_type::{Coord2D, Coord3D};
use crate::common::game_common::{
    AbleToAttackType, CommandSourceType, ObjectShroudStatus, Relationship,
};
use crate::common::geometry::GeometryInfo;
use crate::common::kind_of::{
    KindOfMaskType, KINDOF_DISGUISER, KINDOF_FS_BASE_DEFENSE, KINDOF_INERT, KINDOF_INFANTRY,
    KINDOF_STRUCTURE, KINDOF_VEHICLE,
};
use crate::common::object_status_types::{
    ObjectStatusMaskType, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_REPULSOR,
    OBJECT_STATUS_STEALTHED,
};
use crate::logic::world::ids::{ObjectId, TeamId};
use crate::ww::vector3::Vector3;

/// `PartitionFilterRelationship::ALLOW_ALLIES`: `1 << ALLIES`.
pub const ALLOW_ALLIES: i32 = 1 << (Relationship::Allies as i32);
/// `PartitionFilterRelationship::ALLOW_ENEMIES`: `1 << ENEMIES`.
pub const ALLOW_ENEMIES: i32 = 1 << (Relationship::Enemies as i32);
/// `PartitionFilterRelationship::ALLOW_NEUTRAL`: `1 << NEUTRAL`.
pub const ALLOW_NEUTRAL: i32 = 1 << (Relationship::Neutral as i32);

/// `AllowPlayerRelationship` (`PlayerList.h`): the flags of
/// [`PartitionFilter::PlayerAffiliation`] and `getPlayersWithRelationship`. They are *not* the
/// `PartitionFilterRelationship::ALLOW_*` values above (those are `1 << Relationship`).
pub mod allow_player {
    /// `ALLOW_SAME_PLAYER`: allow only objects of the same player.
    pub const SAME_PLAYER: u32 = 0x01;
    /// `ALLOW_ALLIES`.
    pub const ALLIES: u32 = 0x02;
    /// `ALLOW_ENEMIES`.
    pub const ENEMIES: u32 = 0x04;
    /// `ALLOW_NEUTRAL`.
    pub const NEUTRAL: u32 = 0x08;
}

/// `reallyFarPoint` of `Line2D.cpp`.
const REALLY_FAR_POINT: Coord2D = Coord2D {
    x: 1_000_000.0,
    y: 1_000_000.0,
};

/// `IntersectLine2D(a, b, c, d, NULL)`: whether segment ab crosses segment cd
/// (Line2D.cpp; the FAQ 1.03 solution). Parallel lines do not intersect.
pub fn intersect_line_2d(a: &Coord2D, b: &Coord2D, c: &Coord2D, d: &Coord2D) -> bool {
    let denom = (b.x - a.x) * (d.y - c.y) - (b.y - a.y) * (d.x - c.x);
    if denom == 0.0 {
        // the lines are parallel.
        return false;
    }

    let r = ((a.y - c.y) * (d.x - c.x) - (a.x - c.x) * (d.y - c.y)) / denom;
    let s = ((a.y - c.y) * (b.x - a.x) - (a.x - c.x) * (b.y - a.y)) / denom;

    (0.0..=1.0).contains(&r) && (0.0..=1.0).contains(&s)
}

/// `PointInsideArea2D(pt, area, n)` for `Coord3D`s (Line2D.cpp): the even-odd
/// rule against a ray to a really far point, over the closed polygon.
pub fn point_inside_area_2d(pt_to_test: &Coord3D, area: &[Coord3D]) -> bool {
    let mut num_intersections = 0;
    let pt2d = Coord2D {
        x: pt_to_test.x,
        y: pt_to_test.y,
    };
    let n = area.len();

    for i in 0..n {
        let area2d1 = Coord2D {
            x: area[i].x,
            y: area[i].y,
        };
        let area2d2 = Coord2D {
            x: area[(i + 1) % n].x,
            y: area[(i + 1) % n].y,
        };
        if intersect_line_2d(&pt2d, &REALLY_FAR_POINT, &area2d1, &area2d2) {
            num_intersections += 1;
        }
    }
    num_intersections % 2 == 1
}

/// One `PartitionFilter`. See the module docs.
#[derive(Clone, Debug)]
pub enum PartitionFilter {
    /// `PartitionFilterIsFlying`: reject any objects that aren't currently flying.
    IsFlying,
    /// `PartitionFilterWouldCollide`: the geometry's collision with the object must equal `desired`.
    WouldCollide {
        position: Coord3D,
        geom: GeometryInfo,
        angle: f32,
        desired: bool,
    },
    /// `PartitionFilterSamePlayer`: reject objects not controlled by the same player.
    SamePlayer { player: Option<i32> },
    /// `PartitionFilterRelationship`: accept the relationships in `flags` (`ALLOW_*`) with respect to `obj`.
    Relationship { obj: ObjectId, flags: i32 },
    /// `PartitionFilterAcceptOnTeam`.
    AcceptOnTeam { team: TeamId },
    /// `PartitionFilterAcceptOnSquad` (`squad` is the host's squad key; `None` is a null squad).
    AcceptOnSquad { squad: Option<i32> },
    /// `PartitionFilterLineOfSight`: terrain and obstacles.
    LineOfSight { obj: ObjectId },
    /// `PartitionFilterPossibleToAttack`.
    PossibleToAttack {
        attack_type: AbleToAttackType,
        obj: Option<ObjectId>,
        command_source: CommandSourceType,
    },
    /// `PartitionFilterPossibleToEnter`.
    PossibleToEnter {
        obj: Option<ObjectId>,
        command_source: CommandSourceType,
    },
    /// `PartitionFilterPossibleToHijack`.
    PossibleToHijack {
        obj: Option<ObjectId>,
        command_source: CommandSourceType,
    },
    /// `PartitionFilterLastAttackedBy`: accept only the last object who attacked `obj`.
    LastAttackedBy { last_attacked_by: ObjectId },
    /// `PartitionFilterAcceptByObjectStatus`.
    AcceptByObjectStatus {
        must_be_set: ObjectStatusMaskType,
        must_be_clear: ObjectStatusMaskType,
    },
    /// `PartitionFilterRejectByObjectStatus`.
    RejectByObjectStatus {
        must_be_set: ObjectStatusMaskType,
        must_be_clear: ObjectStatusMaskType,
    },
    /// `PartitionFilterStealthedAndUndetected`.
    StealthedAndUndetected { obj: ObjectId, allow: bool },
    /// `PartitionFilterAcceptByKindOf`.
    AcceptByKindOf {
        must_be_set: KindOfMaskType,
        must_be_clear: KindOfMaskType,
    },
    /// `PartitionFilterRejectByKindOf`.
    RejectByKindOf {
        must_be_set: KindOfMaskType,
        must_be_clear: KindOfMaskType,
    },
    /// `PartitionFilterRejectBehind`: reject any objects "behind" `obj` (a 3D check).
    RejectBehind { obj: ObjectId },
    /// `PartitionFilterAlive`.
    Alive,
    /// `PartitionFilterSameMapStatus`.
    SameMapStatus { obj: ObjectId },
    /// `PartitionFilterOnMap`.
    OnMap,
    /// `PartitionFilterRejectBuildings`; `acquire_enemies` is set by [`PartitionFilter::reject_buildings`].
    RejectBuildings {
        self_obj: ObjectId,
        acquire_enemies: bool,
    },
    /// `PartitionFilterInsignificantBuildings`.
    InsignificantBuildings {
        allow_non_buildings: bool,
        allow_insignificant: bool,
    },
    /// `PartitionFilterFreeOfFog`: accept if clear, not fogged or shrouded, to the player.
    FreeOfFog { comparison_index: i32 },
    /// `PartitionFilterRepulsor`.
    Repulsor { self_obj: ObjectId },
    /// `PartitionFilterIrregularArea`: reject everything outside the closed polygon.
    IrregularArea { area: Vec<Coord3D> },
    /// `PartitionFilterPolygonTrigger` (the host names the trigger).
    PolygonTrigger { trigger: i32 },
    /// `PartitionFilterPlayer`.
    Player { player: Option<i32>, match_: bool },
    /// `PartitionFilterPlayerAffiliation`; `affiliation` uses the `PlayerList.h` flags ([`allow_player`]).
    PlayerAffiliation {
        player: i32,
        affiliation: u32,
        match_: bool,
    },
    /// `PartitionFilterThing` (the host names the template).
    Thing { template_key: i32, match_: bool },
    /// `PartitionFilterGarrisonable`.
    Garrisonable { match_: bool },
    /// `PartitionFilterGarrisonableByPlayer`.
    GarrisonableByPlayer {
        player: Option<i32>,
        match_: bool,
        command_source: CommandSourceType,
    },
    /// `PartitionFilterUnmannedObject`.
    UnmannedObject { match_: bool },
    /// `PartitionFilterValidCommandButtonTarget`.
    ValidCommandButtonTarget {
        source: ObjectId,
        command_button: i32,
        match_: bool,
        command_source: CommandSourceType,
    },
}

impl PartitionFilter {
    /// `PartitionFilterLastAttackedBy(obj)`: remembers who last damaged `obj`
    /// (`INVALID_ID` when it has no body).
    pub fn last_attacked_by<H: PartitionFilterHost>(
        host: &H,
        obj: Option<ObjectId>,
    ) -> PartitionFilter {
        let last = obj
            .and_then(|o| host.last_damage_source(o))
            .unwrap_or(crate::logic::world::ids::INVALID_ID);
        PartitionFilter::LastAttackedBy {
            last_attacked_by: last,
        }
    }

    /// `PartitionFilterRejectBuildings(o)`: a computer-controlled opponent auto-acquires
    /// enemy buildings.
    pub fn reject_buildings<H: PartitionFilterHost>(host: &H, o: ObjectId) -> PartitionFilter {
        PartitionFilter::RejectBuildings {
            self_obj: o,
            acquire_enemies: host.controlling_player_is_computer(o),
        }
    }

    /// `allow(objOther)`: whether `other` passes. Takes the manager mutably
    /// because `PartitionFilterFreeOfFog` recomputes a shroudedness.
    pub fn allow<H: PartitionFilterHost>(
        &self,
        pm: &mut PartitionManager,
        host: &H,
        other: ObjectId,
    ) -> bool {
        match self {
            PartitionFilter::IsFlying => host.is_using_airborne_locomotor(other),

            PartitionFilter::WouldCollide {
                position,
                geom,
                angle,
                desired,
            } => {
                let Some(og) = host.object_geometry(other) else {
                    return !*desired;
                };
                let does_collide = collide::geom_collides_with_geom(
                    position,
                    geom,
                    *angle,
                    &og.position,
                    &og.geometry,
                    og.orientation,
                );
                does_collide == *desired
            }

            PartitionFilter::SamePlayer { player } => *player == host.controlling_player(other),

            PartitionFilter::Relationship { obj, flags } => {
                let r = host.relationship(*obj, other);
                (flags & (1 << (r as i32))) != 0
            }

            PartitionFilter::AcceptOnTeam { team } => host.team(other) == *team,

            PartitionFilter::AcceptOnSquad { squad } => match squad {
                Some(s) => host.is_on_squad(*s, other) && !host.is_effectively_dead(other),
                None => false,
            },

            PartitionFilter::LineOfSight { obj } => {
                let (Some(a), Some(b)) = (host.object_geometry(*obj), host.object_geometry(other))
                else {
                    return false;
                };
                if !pm.is_clear_line_of_sight_terrain(
                    host,
                    Some(*obj),
                    &a.position,
                    Some(other),
                    &b.position,
                ) {
                    return false;
                }
                if host.is_view_blocked_by_obstacle(*obj, other) {
                    return false;
                }
                true
            }

            PartitionFilter::PossibleToAttack {
                attack_type,
                obj,
                command_source,
            } => {
                // (upstream dereferences a null `m_obj` here; refused instead)
                let Some(o) = obj else {
                    return false;
                };
                let result =
                    host.able_to_attack_specific_object(*o, *attack_type, other, *command_source);
                result == CanAttackResult::Possible
                    || result == CanAttackResult::PossibleAfterMoving
            }

            PartitionFilter::PossibleToEnter {
                obj,
                command_source,
            } => match obj {
                Some(o) => host.can_enter_object(*o, other, *command_source),
                None => false,
            },

            PartitionFilter::PossibleToHijack {
                obj,
                command_source,
            } => match obj {
                Some(o) => host.can_hijack_vehicle(*o, other, *command_source),
                None => false,
            },

            PartitionFilter::LastAttackedBy { last_attacked_by } => other == *last_attacked_by,

            PartitionFilter::AcceptByObjectStatus {
                must_be_set,
                must_be_clear,
            } => {
                let status = host.status_bits(other);
                status.test_for_all(must_be_set) && status.test_for_none(must_be_clear)
            }

            PartitionFilter::RejectByObjectStatus {
                must_be_set,
                must_be_clear,
            } => {
                let status = host.status_bits(other);
                !(status.test_for_all(must_be_set) && status.test_for_none(must_be_clear))
            }

            PartitionFilter::StealthedAndUndetected { obj, allow } => {
                stealthed_and_undetected(host, *obj, *allow, other)
            }

            PartitionFilter::AcceptByKindOf {
                must_be_set,
                must_be_clear,
            } => host
                .kind_of_mask(other)
                .test_set_and_clear(must_be_set, must_be_clear),

            PartitionFilter::RejectByKindOf {
                must_be_set,
                must_be_clear,
            } => !host
                .kind_of_mask(other)
                .test_set_and_clear(must_be_set, must_be_clear),

            PartitionFilter::RejectBehind { obj } => {
                let x = host.transform_x_vector(*obj);
                let mut dir = Vector3::new(x.x, x.y, x.z);
                dir.normalize();

                let (Some(a), Some(b)) = (host.object_geometry(*obj), host.object_geometry(other))
                else {
                    return false;
                };
                let v = collide::dist_calc(
                    DistanceCalculationType::FromCenter3D,
                    &a.position,
                    Some(&a.geometry),
                    &b.position,
                    Some(&b.geometry),
                    HUGE_DIST_SQR,
                )
                .vec;

                let dot = dir.x * v.x + dir.y * v.y + dir.z * v.z;
                dot > 0.0
            }

            PartitionFilter::Alive => !host.is_effectively_dead(other),

            PartitionFilter::SameMapStatus { obj } => {
                host.is_off_map(other) == host.is_off_map(*obj)
            }

            PartitionFilter::OnMap => !host.is_off_map(other),

            PartitionFilter::RejectBuildings {
                self_obj,
                acquire_enemies,
            } => reject_buildings_allow(host, *self_obj, *acquire_enemies, other),

            PartitionFilter::InsignificantBuildings {
                allow_non_buildings,
                allow_insignificant,
            } => {
                if host.is_structure(other) {
                    if host.is_non_faction_structure(other) && !allow_insignificant {
                        if let Some(cmi) = host.contain_facts(other) {
                            if !cmi.is_garrisonable || cmi.contain_count == 0 {
                                return false;
                            }
                        }
                    }
                    true
                } else {
                    *allow_non_buildings
                }
            }

            PartitionFilter::FreeOfFog { comparison_index } => {
                pm.get_object_shrouded_status(host, other, *comparison_index)
                    == ObjectShroudStatus::Clear
            }

            PartitionFilter::Repulsor { self_obj } => {
                if other == *self_obj {
                    // don't repulse yourself. :)
                    return false;
                }

                // If it's flagged, it's a repulsor.
                if host.status_bits(other).test(OBJECT_STATUS_REPULSOR) {
                    return true;
                }

                if host.is_effectively_dead(other) {
                    return false; // no dead enemies.
                }

                if host.relationship(*self_obj, other) != Relationship::Enemies {
                    return false; // only enemies auto repulse.
                }

                if host.is_kind_of(other, KINDOF_STRUCTURE) {
                    // always pay attention to buildings that can attack
                    return host.is_able_to_attack(other);
                }

                if host.is_kind_of(other, KINDOF_INERT) {
                    return false;
                }

                host.is_able_to_attack(other)
            }

            PartitionFilter::IrregularArea { area } => match host.object_geometry(other) {
                Some(og) => point_inside_area_2d(&og.position, area),
                None => false,
            },

            PartitionFilter::PolygonTrigger { trigger } => match host.object_geometry(other) {
                Some(og) => {
                    // Trigger areas compare on xy only; the float position is truncated to an Int.
                    host.point_in_trigger(*trigger, og.position.x as i32, og.position.y as i32)
                }
                None => false,
            },

            PartitionFilter::Player { player, match_ } => {
                (*player == host.controlling_player(other)) == *match_
            }

            PartitionFilter::PlayerAffiliation {
                player,
                affiliation,
                match_,
            } => {
                let rel = host.player_relationship_to_team_of(*player, other);
                match rel {
                    Relationship::Enemies => {
                        if affiliation & allow_player::ENEMIES != 0 {
                            return *match_;
                        }
                    }
                    Relationship::Neutral => {
                        if affiliation & allow_player::NEUTRAL != 0 {
                            return *match_;
                        }
                    }
                    Relationship::Allies => {
                        if affiliation & allow_player::ALLIES != 0 {
                            return *match_;
                        }
                    }
                }

                if host.controlling_player(other) == Some(*player) {
                    return *match_;
                }

                !*match_
            }

            PartitionFilter::Thing {
                template_key,
                match_,
            } => host.template_is_equivalent_to(*template_key, other) == *match_,

            PartitionFilter::Garrisonable { match_ } => match host.contain_facts(other) {
                None => !*match_,
                Some(cmi) => cmi.is_garrisonable == *match_,
            },

            PartitionFilter::GarrisonableByPlayer {
                player,
                match_,
                command_source,
            } => host.can_player_garrison(*player, other, *command_source) == *match_,

            PartitionFilter::UnmannedObject { match_ } => {
                host.is_disabled_unmanned(other) == *match_
            }

            PartitionFilter::ValidCommandButtonTarget {
                source,
                command_button,
                match_,
                command_source,
            } => {
                host.command_button_valid_target(*command_button, *source, other, *command_source)
                    == *match_
            }
        }
    }
}

/// `PartitionFilterRejectBuildings::allow`.
fn reject_buildings_allow<H: PartitionFilterHost>(
    host: &H,
    self_obj: ObjectId,
    acquire_enemies: bool,
    other: ObjectId,
) -> bool {
    // this filter allows all non-buildings (and, um, no longer does)
    if !host.is_kind_of(other, KINDOF_STRUCTURE) {
        return true;
    }

    let Some(my_player) = host.controlling_player(self_obj) else {
        return false;
    };

    // Get the controlling team of other.
    let contain = host.contain_facts(other);
    let mut other_player = if contain.is_some() {
        host.apparent_controlling_player(other, Some(my_player))
    } else {
        None
    };

    if other_player.is_none() {
        other_player = host.controlling_player(other);
    }

    let Some(other_player) = other_player else {
        return false;
    };

    let relationship = host.player_relationship_to_player(my_player, other_player);
    if relationship != Relationship::Enemies {
        return false;
    }

    // if I am a computer-controlled opponent, auto-aquire enemy buildings (if we can see them!)
    if acquire_enemies {
        return true;
    }

    if host.is_kind_of(other, KINDOF_FS_BASE_DEFENSE) {
        // Don't reject base defenses.
        return true;
    }

    if host.contain_facts(other).is_some() && host.is_able_to_attack(other) {
        // Don't reject garrisoned buildings that can attack
        return true;
    }

    false
}

/// `PartitionFilterStealthedAndUndetected::allow`.
fn stealthed_and_undetected<H: PartitionFilterHost>(
    host: &H,
    m_obj: ObjectId,
    m_allow: bool,
    obj_other: ObjectId,
) -> bool {
    let status = host.status_bits(obj_other);
    let stealthed = status.test(OBJECT_STATUS_STEALTHED);
    let detected = status.test(OBJECT_STATUS_DETECTED);
    let disguised = status.test(OBJECT_STATUS_DISGUISED);

    if stealthed && !detected {
        if !host.is_kind_of(obj_other, KINDOF_DISGUISER) {
            // We have a stealthed & undetected object.
            return m_allow;
        } else if disguised {
            // Exception case -- bomb trucks can't be considered stealthed units when they are disguised as the enemy.
            if let Some(update) = host.stealth_facts(obj_other) {
                if update.is_disguised {
                    let our_player = host.controlling_player(m_obj);
                    let idx = update.disguised_player_index;
                    // ThePlayerList->getNthPlayer(idx) is null outside the 16 slots
                    let other_player =
                        if (0..crate::common::game_common::MAX_PLAYER_COUNT).contains(&idx) {
                            Some(idx)
                        } else {
                            None
                        };
                    if let (Some(ours), Some(theirs)) = (our_player, other_player) {
                        if host.player_relationship_to_player(ours, theirs) == Relationship::Enemies
                        {
                            // Our stealthed & undetected object is disguised as a unit perceived to be our
                            // enemy, therefore it's not considered to be stealthed!
                            return !m_allow;
                        }
                        // Our object is disguised as a "friendly" object, therefore it's considered to be stealthed.
                        return m_allow;
                    }
                }
            }
            // Our bomb truck is not disguised therefore it's not stealthed.
            return !m_allow;
        }
    } else {
        // This handles neutral containers that hold stealth units. This specifically fixes a bug where
        // hunt scripts would ignore this case -- units would acquire the building Jarmen Kell occupied
        // even though it was not stealth detected.
        if let Some(contain) = host.contain_facts(obj_other) {
            let victim_apparent_controller =
                host.apparent_controlling_player(obj_other, host.controlling_player(m_obj));
            // Check if it's stealthed!
            if contain.stealth_units_contained == contain.contain_count {
                // Check if the first object inside is detected (if one is detected, all are detected).
                if let Some(first_detected) = contain.first_contained_detected {
                    if !first_detected {
                        // Finally check the relationship!
                        if let Some(victim) = victim_apparent_controller {
                            if host.team_relationship_to_player(m_obj, victim)
                                == Relationship::Enemies
                            {
                                // Our object is a neutral building garrisoned by enemy units we can't see,
                                // therefore it is stealthed.
                                return m_allow;
                            }
                        }
                    }
                }
            }
        }
    }

    // The unit is not stealthed!
    !m_allow
}

/// `filtersAllow(filters, objOther)`: every filter must accept, in order.
pub fn filters_allow<H: PartitionFilterHost>(
    pm: &mut PartitionManager,
    host: &H,
    filters: &[PartitionFilter],
    obj_other: ObjectId,
) -> bool {
    for f in filters {
        if !f.allow(pm, host, obj_other) {
            return false;
        }
    }
    // assume true if no filters rejected it
    true
}

/// The "infantry or vehicle" test `tryPosition` applies to a unit.
pub(super) fn is_unit<H: PartitionFilterHost>(host: &H, id: ObjectId) -> bool {
    host.is_kind_of(id, KINDOF_INFANTRY) || host.is_kind_of(id, KINDOF_VEHICLE)
}
