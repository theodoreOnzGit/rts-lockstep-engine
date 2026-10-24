// SPDX-License-Identifier: GPL-3.0-only
// Translation-owned file (this repository's own tests, not upstream code).

//! Unit tests of the partition manager on the table-driven scenario host.
//! The behaviour that has to match upstream bit for bit (list orders, ties,
//! distances, shroud states) is checked against the compiled C++ in
//! `tests/differential_partition.rs`; these tests pin the hand-derived cases
//! and the translation's own choices.

use super::scenario::{sflag, ScenarioHost, ScenarioLog, ScenarioObject};
use super::*;
use crate::common::base_type::Coord3D;
use crate::common::game_common::{CellShroudStatus, ObjectShroudStatus, Relationship};
use crate::common::geometry::{GeometryInfo, GeometryType};
use crate::common::kind_of::{KindOfMaskType, KINDOF_IMMOBILE, KINDOF_INFANTRY, KINDOF_STRUCTURE};
use crate::common::object_status_types::OBJECT_STATUS_DESTROYED;
use crate::common::xfer::{Snapshot, Xfer};
use crate::common::xfer_crc::XferCrc;
use crate::common::xfer_load::XferLoad;
use crate::common::xfer_save::XferSave;
use crate::logic::object_iter::IterOrderType;

fn sphere(r: f32) -> GeometryInfo {
    GeometryInfo::new(GeometryType::Sphere, false, r, r, r)
}

fn at(x: f32, y: f32) -> Coord3D {
    Coord3D { x, y, z: 0.0 }
}

/// A 400 x 400 map with 40-unit cells, four players, the manager initialised. The grid is
/// 11 x 11, not 10 x 10: `REAL_TO_INT_CEIL(400 * 0.025f)` is `fast_float_ceil(10.0)`, which adds
/// 0.99999994 and truncates, and 10.99999994 rounds to 11.0 in `float`. (Upstream's own result.)
fn world() -> (ScenarioHost, PartitionManager) {
    let host = ScenarioHost::default();
    let mut pm = PartitionManager::new();
    pm.init(&host);
    (host, pm)
}

/// Adds a sphere to the scenario and registers it.
fn put(host: &mut ScenarioHost, pm: &mut PartitionManager, id: i32, x: f32, y: f32, r: f32) {
    host.add(ScenarioObject::new(id, at(x, y), sphere(r)));
    pm.register_object(host, id);
}

#[test]
fn init_builds_the_grid_and_the_ring_table() {
    let (host, pm) = world();
    assert_eq!(
        (pm.get_cell_count_x(), pm.get_cell_count_y()),
        (11, 11),
        "ceil(10.0) is 11 in fast_float_ceil"
    );
    assert_eq!(pm.get_cell_size(), 40.0);
    assert_eq!(pm.get_cell_size_inv(), 0.025);
    // sqrt((11*40)^2 * 2) / 40 = 15.56, rounded up
    assert_eq!(pm.max_gco_radius(), 16);
    // every offset (dx, dy) with |d| < 11 appears exactly once: (2*11 - 1)^2
    let total: usize = pm.radius_vec().iter().map(Vec::len).sum();
    assert_eq!(total, 441);
    // objects in cells at most one apart (even diagonally) can be arbitrarily close: ring 0
    assert_eq!(pm.radius_vec()[0].len(), 9);
    // the double loop runs rows outermost: the first entry of ring 0 is (-1,-1), the last (1,1)
    assert_eq!(
        pm.radius_vec()[0][0],
        crate::common::base_type::ICoord2D { x: -1, y: -1 }
    );
    assert_eq!(
        pm.radius_vec()[0][8],
        crate::common::base_type::ICoord2D { x: 1, y: 1 }
    );
    // each cell caches its terrain height range from the ground function
    let c = pm.get_cell_at(9, 9).unwrap();
    assert!(c.get_lo_terrain() < c.get_hi_terrain());
    assert_eq!(c.get_cell_x(), 9);
    assert!(pm.get_cell_at(11, 0).is_none() && pm.get_cell_at(-1, 3).is_none());
    assert_eq!(pm.get_cell_center_pos(2, 3), (100.0, 140.0));
    assert_eq!(pm.world_to_cell(100.0, 100.0), (2, 2));
    assert_eq!(pm.world_to_cell_dist(60.0), 2);
    let _ = host;
}

#[test]
fn no_terrain_leaves_the_manager_empty() {
    let mut host = ScenarioHost {
        extent: None,
        ..Default::default()
    };
    let mut pm = PartitionManager::new();
    pm.init(&host);
    assert_eq!((pm.get_cell_count_x(), pm.get_cell_size()), (0, 0.0));
    host.add(ScenarioObject::new(1, at(5.0, 5.0), sphere(1.0)));
    pm.register_object(&mut host, 1);
    pm.update(&mut host);
    let r = pm.get_closest_object_at(
        &host,
        &at(0.0, 0.0),
        100.0,
        DistanceCalculationType::FromCenter2D,
        &[],
    );
    assert_eq!(r.object, None, "an empty ring table finds nothing");
}

#[test]
fn a_small_sphere_touches_one_cell_and_a_big_one_a_block() {
    let (mut host, mut pm) = world();
    put(&mut host, &mut pm, 1, 100.0, 100.0, 5.0);
    put(&mut host, &mut pm, 2, 200.0, 200.0, 60.0);
    assert!(
        pm.cells_touched_by(1).is_empty(),
        "cells are computed by update()"
    );
    assert_eq!(
        pm.dirty_objects(),
        vec![Some(2), Some(1)],
        "the dirty list is prepended to"
    );
    pm.update(&mut host);
    assert!(pm.dirty_objects().is_empty());
    assert_eq!(pm.cells_touched_by(1), vec![(2, 2)]);
    let mut big = pm.cells_touched_by(2);
    big.sort();
    let expect: Vec<(i32, i32)> = (4..=6).flat_map(|x| (4..=6).map(move |y| (x, y))).collect();
    assert_eq!(
        big, expect,
        "a radius of two cells fills the 3x3 block around the centre cell (5,5)"
    );
    assert_eq!(pm.objects_in_cell(2, 2), vec![Some(1)]);
    assert_eq!(
        pm.registered_objects(),
        vec![Some(2), Some(1)],
        "newest registration first"
    );
    // the cell change callback fired once per object, in dirty-list order
    assert_eq!(
        host.log,
        vec![ScenarioLog::CellChange(2), ScenarioLog::CellChange(1)]
    );
}

#[test]
fn moving_an_object_updates_its_cells_and_unregistering_clears_them() {
    let (mut host, mut pm) = world();
    put(&mut host, &mut pm, 1, 100.0, 100.0, 5.0);
    pm.update(&mut host);
    host.objects.get_mut(&1).unwrap().position = at(300.0, 300.0);
    pm.make_object_dirty(1, true);
    pm.update(&mut host);
    assert_eq!(pm.cells_touched_by(1), vec![(7, 7)]);
    assert!(pm.objects_in_cell(2, 2).is_empty());
    pm.unregister_object(1);
    assert!(pm.objects_in_cell(7, 7).is_empty() && !pm.is_registered(1));
    assert!(pm.registered_objects().is_empty() && pm.dirty_objects().is_empty());
    pm.unregister_object(1); // a second time is a no-op
}

#[test]
fn the_closest_object_has_its_distance_and_vector_and_a_miss_returns_the_range() {
    let (mut host, mut pm) = world();
    put(&mut host, &mut pm, 1, 100.0, 100.0, 2.0);
    put(&mut host, &mut pm, 2, 180.0, 100.0, 2.0);
    put(&mut host, &mut pm, 3, 100.0, 250.0, 2.0);
    pm.update(&mut host);
    let r = pm.get_closest_object_at(
        &host,
        &at(110.0, 100.0),
        500.0,
        DistanceCalculationType::FromCenter2D,
        &[],
    );
    assert_eq!(r.object, Some(1));
    assert_eq!(r.dist, 10.0);
    assert_eq!(
        r.vec,
        Coord3D {
            x: -10.0,
            y: 0.0,
            z: 0.0
        }
    );
    let r = pm.get_closest_object(&host, 1, 500.0, DistanceCalculationType::FromCenter2D, &[]);
    assert_eq!(r.object, Some(2), "never the object itself");
    assert_eq!(r.dist, 80.0);
    // boundary distance: 80 - 2 - 2
    let r = pm.get_closest_object(
        &host,
        1,
        500.0,
        DistanceCalculationType::FromBoundingSphere2D,
        &[],
    );
    assert_eq!(r.dist, 76.0);
    let r = pm.get_closest_object(&host, 1, 50.0, DistanceCalculationType::FromCenter2D, &[]);
    assert_eq!(
        (r.object, r.dist),
        (None, 50.0),
        "nothing within range: the range back"
    );
}

#[test]
fn a_range_query_lists_the_reverse_of_the_visit_order_until_sorted() {
    let (mut host, mut pm) = world();
    // three objects on a line, each in its own cell so the visit order is the ring order
    put(&mut host, &mut pm, 1, 200.0, 200.0, 2.0);
    put(&mut host, &mut pm, 2, 250.0, 200.0, 2.0);
    put(&mut host, &mut pm, 3, 330.0, 200.0, 2.0);
    pm.update(&mut host);
    let origin = at(200.0, 200.0);
    let walk = |mut it: crate::logic::object_iter::SimpleObjectIterator<i32>| {
        let mut v = Vec::new();
        let (mut o, mut n) = it.first_with_numeric();
        while let Some(id) = o {
            v.push((id, n));
            (o, n) = it.next_with_numeric();
        }
        v
    };
    let near = walk(pm.iterate_objects_in_range_at(
        &host,
        &origin,
        500.0,
        DistanceCalculationType::FromCenter2D,
        &[],
        IterOrderType::SortedNearToFar,
    ));
    assert_eq!(near, vec![(1, 0.0), (2, 2500.0), (3, 16900.0)]);
    let far = walk(pm.iterate_objects_in_range_at(
        &host,
        &origin,
        500.0,
        DistanceCalculationType::FromCenter2D,
        &[],
        IterOrderType::SortedFarToNear,
    ));
    assert_eq!(far.iter().map(|p| p.0).collect::<Vec<_>>(), vec![3, 2, 1]);
    let fastest = walk(pm.iterate_objects_in_range_at(
        &host,
        &origin,
        500.0,
        DistanceCalculationType::FromCenter2D,
        &[],
        IterOrderType::Fastest,
    ));
    assert_eq!(
        fastest.iter().map(|p| p.0).collect::<Vec<_>>(),
        vec![3, 2, 1],
        "prepended while walking outwards: farthest first"
    );
    // by cost
    host.objects.get_mut(&1).unwrap().build_cost = 300;
    host.objects.get_mut(&2).unwrap().build_cost = 100;
    host.objects.get_mut(&3).unwrap().build_cost = 200;
    let cheap = walk(pm.iterate_objects_in_range_at(
        &host,
        &origin,
        500.0,
        DistanceCalculationType::FromCenter2D,
        &[],
        IterOrderType::SortedCheapToExpensive,
    ));
    assert_eq!(cheap.iter().map(|p| p.0).collect::<Vec<_>>(), vec![2, 3, 1]);
    // the range cuts
    let few = walk(pm.iterate_objects_in_range_at(
        &host,
        &origin,
        60.0,
        DistanceCalculationType::FromCenter2D,
        &[],
        IterOrderType::SortedNearToFar,
    ));
    assert_eq!(few.iter().map(|p| p.0).collect::<Vec<_>>(), vec![1, 2]);
    // all objects: the module list runs newest registration first and the iterator prepends each
    // insert, so the iterator lists the oldest registration first
    let mut all = pm.iterate_all_objects(&host, &[]);
    assert_eq!(
        (all.first(), all.next(), all.next(), all.next()),
        (Some(1), Some(2), Some(3), None)
    );
}

#[test]
fn filters_drop_objects_and_the_first_refusal_wins() {
    let (mut host, mut pm) = world();
    for (id, x) in [(1, 100.0), (2, 140.0), (3, 180.0), (4, 220.0)] {
        put(&mut host, &mut pm, id, x, 100.0, 2.0);
    }
    host.objects
        .get_mut(&2)
        .unwrap()
        .kind_of
        .set(KINDOF_INFANTRY, 1);
    host.objects
        .get_mut(&3)
        .unwrap()
        .kind_of
        .set(KINDOF_STRUCTURE, 1);
    host.objects.get_mut(&3).unwrap().flags |= sflag::DEAD;
    host.objects
        .get_mut(&4)
        .unwrap()
        .kind_of
        .set(KINDOF_INFANTRY, 1);
    host.objects.get_mut(&4).unwrap().player = Some(2);
    host.objects.get_mut(&1).unwrap().player = Some(1);
    host.rel[1][2] = Relationship::Enemies;
    pm.update(&mut host);
    let mut inf = KindOfMaskType::new();
    inf.set(KINDOF_INFANTRY, 1);
    let ids = |pm: &mut PartitionManager, host: &ScenarioHost, f: &[PartitionFilter]| {
        let mut it = pm.iterate_objects_in_range(
            host,
            1,
            500.0,
            DistanceCalculationType::FromCenter2D,
            f,
            IterOrderType::SortedNearToFar,
        );
        let mut w = Vec::new();
        let mut o = it.first();
        while let Some(id) = o {
            w.push(id);
            o = it.next();
        }
        w
    };
    let f_inf = [PartitionFilter::AcceptByKindOf {
        must_be_set: inf,
        must_be_clear: KindOfMaskType::new(),
    }];
    assert_eq!(ids(&mut pm, &host, &f_inf), vec![2, 4]);
    let f_inf_alive = [f_inf[0].clone(), PartitionFilter::Alive];
    assert_eq!(ids(&mut pm, &host, &f_inf_alive), vec![2, 4]);
    let f_alive = [PartitionFilter::Alive];
    assert_eq!(
        ids(&mut pm, &host, &f_alive),
        vec![2, 4],
        "3 is dead; 1 is the origin"
    );
    let f_enemy = [PartitionFilter::Relationship {
        obj: 1,
        flags: filters::ALLOW_ENEMIES,
    }];
    assert_eq!(ids(&mut pm, &host, &f_enemy), vec![4]);
    let f_player = [PartitionFilter::Player {
        player: Some(2),
        match_: true,
    }];
    assert_eq!(ids(&mut pm, &host, &f_player), vec![4]);
    let f_rej = [PartitionFilter::RejectByKindOf {
        must_be_set: inf,
        must_be_clear: KindOfMaskType::new(),
    }];
    assert_eq!(ids(&mut pm, &host, &f_rej), vec![3]);
    // a would-collide filter against a sphere that overlaps object 2 only
    let f_collide = [PartitionFilter::WouldCollide {
        position: at(141.0, 100.0),
        geom: sphere(3.0),
        angle: 0.0,
        desired: true,
    }];
    assert_eq!(ids(&mut pm, &host, &f_collide), vec![2]);
}

#[test]
fn potential_collisions_and_the_geometry_test_agree() {
    let (mut host, mut pm) = world();
    put(&mut host, &mut pm, 1, 100.0, 100.0, 5.0);
    put(&mut host, &mut pm, 2, 108.0, 100.0, 5.0);
    put(&mut host, &mut pm, 3, 130.0, 100.0, 5.0);
    pm.update(&mut host);
    let mut it =
        pm.iterate_potential_collisions(&host, &at(104.0, 100.0), &sphere(5.0), 0.0, false);
    let mut got = Vec::new();
    let mut o = it.first();
    while let Some(id) = o {
        got.push(id);
        o = it.next();
    }
    got.sort();
    assert_eq!(got, vec![1, 2]);
    assert!(pm.is_colliding(&host, 1, 2) && !pm.is_colliding(&host, 1, 3));
    assert!(pm.geom_collides_with_geom(
        &at(0.0, 0.0),
        &sphere(2.0),
        0.0,
        &at(3.0, 0.0),
        &sphere(2.0),
        0.0
    ));
}

#[test]
fn overlapping_objects_collide_once_each_way_and_a_dying_one_is_unregistered() {
    let (mut host, mut pm) = world();
    put(&mut host, &mut pm, 1, 100.0, 100.0, 6.0);
    put(&mut host, &mut pm, 2, 108.0, 100.0, 6.0);
    host.objects.get_mut(&2).unwrap().flags |= sflag::KILL_ON_COLLIDE;
    pm.update(&mut host);
    let collisions: Vec<_> = host
        .log
        .iter()
        .filter_map(|l| {
            if let ScenarioLog::Collide { id, other, .. } = l {
                Some((*id, *other))
            } else {
                None
            }
        })
        .collect();
    // the dirty list runs newest first: module 2 is brought up to date while module 1 has no cells yet,
    // so it finds no contact; module 1 then finds 2, and the pair is stored as (1, 2). Object 1 is told
    // first; then 2, which dies in its handler.
    assert_eq!(collisions, vec![(1, 2), (2, 1)]);
    assert!(!pm.is_registered(2) && pm.is_registered(1));
    assert!(host.objects[&2].status.test(OBJECT_STATUS_DESTROYED));
    // 2 left the cells when it was unregistered; 1 is still there and was re-dirtied for a collision check
    assert!(
        pm.objects_in_cell(2, 2).contains(&Some(1)) && !pm.objects_in_cell(2, 2).contains(&Some(2))
    );
    assert_eq!(pm.dirty_objects(), vec![Some(1)]);
}

#[test]
fn a_collision_reports_the_surface_point_and_both_normals_are_opposite() {
    let (mut host, mut pm) = world();
    put(&mut host, &mut pm, 1, 100.0, 100.0, 6.0);
    put(&mut host, &mut pm, 2, 108.0, 100.0, 6.0);
    pm.update(&mut host);
    let c: Vec<_> = host
        .log
        .iter()
        .filter_map(|l| {
            if let ScenarioLog::Collide {
                id,
                other,
                loc,
                normal,
            } = l
            {
                Some((*id, *other, *loc, *normal))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(c.len(), 2);
    assert_eq!(c[0].0, c[1].1);
    assert_eq!(c[0].1, c[1].0);
    assert_eq!(c[0].3.x, -c[1].3.x);
    assert_eq!(c[0].2, c[1].2, "the same collision point for both");
}

#[test]
fn immobile_objects_do_not_start_collisions() {
    let (mut host, mut pm) = world();
    put(&mut host, &mut pm, 1, 100.0, 100.0, 6.0);
    put(&mut host, &mut pm, 2, 108.0, 100.0, 6.0);
    host.objects
        .get_mut(&1)
        .unwrap()
        .kind_of
        .set(KINDOF_IMMOBILE, 1);
    host.objects
        .get_mut(&2)
        .unwrap()
        .kind_of
        .set(KINDOF_IMMOBILE, 1);
    pm.update(&mut host);
    assert!(host
        .log
        .iter()
        .all(|l| !matches!(l, ScenarioLog::Collide { .. })));
}

#[test]
fn revealing_clears_the_shroud_and_unlooking_fogs_it() {
    let (mut host, mut pm) = world();
    host.local_player = Some(1);
    let mask = 1u16 << 1;
    assert_eq!(
        pm.get_shroud_status_for_player(1, &at(100.0, 100.0)),
        CellShroudStatus::Shrouded
    );
    pm.do_shroud_reveal(&host, 100.0, 100.0, 50.0, mask)
        .unwrap();
    assert_eq!(
        pm.get_shroud_status_for_player(1, &at(100.0, 100.0)),
        CellShroudStatus::Clear
    );
    assert_eq!(
        pm.get_shroud_status_for_player(2, &at(100.0, 100.0)),
        CellShroudStatus::Shrouded,
        "per player"
    );
    assert_eq!(
        pm.get_shroud_status_for_player(1, &at(380.0, 380.0)),
        CellShroudStatus::Shrouded
    );
    assert_eq!(
        pm.get_shroud_status_for_player(-1, &at(100.0, 100.0)),
        CellShroudStatus::Shrouded
    );
    assert_eq!(
        pm.get_shroud_status_for_player(1, &at(-100.0, 100.0)),
        CellShroudStatus::Shrouded,
        "off the map"
    );
    let evs = pm.take_client_events();
    assert!(evs.iter().all(|e| matches!(
        e,
        PartitionClientEvent::SetShroudLevel {
            status: CellShroudStatus::Clear,
            ..
        }
    )));
    assert!(
        !evs.is_empty(),
        "the local player's cells are pushed to the client"
    );
    pm.undo_shroud_reveal(&host, 100.0, 100.0, 50.0, mask)
        .unwrap();
    assert_eq!(
        pm.get_shroud_status_for_player(1, &at(100.0, 100.0)),
        CellShroudStatus::Fogged
    );
    // covering makes it shrouded again, and removing the cover leaves it fogged-or-shrouded as counted
    pm.do_shroud_cover(&host, 100.0, 100.0, 50.0, mask).unwrap();
    assert_eq!(
        pm.get_shroud_status_for_player(1, &at(100.0, 100.0)),
        CellShroudStatus::Shrouded
    );
    pm.undo_shroud_cover(&host, 100.0, 100.0, 50.0, mask)
        .unwrap();
    // another player's reveal does not touch player 1's events
    pm.take_client_events();
    pm.do_shroud_reveal(&host, 100.0, 100.0, 50.0, 1u16 << 2)
        .unwrap();
    assert!(pm.take_client_events().is_empty());
}

#[test]
fn queued_unlooks_wait_for_their_frame() {
    let (mut host, mut pm) = world();
    let mask = 1u16 << 1;
    host.frame = 10;
    pm.do_shroud_reveal(&host, 100.0, 100.0, 50.0, mask)
        .unwrap();
    pm.queue_undo_shroud_reveal(&host, 100.0, 100.0, 50.0, mask);
    assert_eq!(pm.pending_undo_shroud_reveals().next().unwrap().data, 40);
    host.frame = 40;
    pm.update(&mut host);
    assert_eq!(
        pm.get_shroud_status_for_player(1, &at(100.0, 100.0)),
        CellShroudStatus::Clear,
        "due at 40 means after 40"
    );
    host.frame = 41;
    pm.update(&mut host);
    assert_eq!(
        pm.get_shroud_status_for_player(1, &at(100.0, 100.0)),
        CellShroudStatus::Fogged
    );
    assert_eq!(pm.pending_undo_shroud_reveals().count(), 0);
    // processing the entire queue ignores the clock
    pm.queue_undo_shroud_reveal(&host, 100.0, 100.0, 50.0, mask);
    pm.process_entire_pending_undo_shroud_reveal_queue(&host);
    assert_eq!(pm.pending_undo_shroud_reveals().count(), 0);
}

#[test]
fn an_objects_shroudedness_follows_its_cells_and_is_invalid_until_the_first_update() {
    let (mut host, mut pm) = world();
    put(&mut host, &mut pm, 1, 100.0, 100.0, 5.0);
    assert_eq!(
        pm.get_object_shrouded_status(&host, 1, 1),
        ObjectShroudStatus::Invalid,
        "no update() yet"
    );
    pm.update(&mut host);
    assert_eq!(
        pm.get_object_shrouded_status(&host, 1, 1),
        ObjectShroudStatus::Shrouded
    );
    pm.do_shroud_reveal(&host, 100.0, 100.0, 20.0, 1u16 << 1)
        .unwrap();
    assert_eq!(
        pm.get_object_shrouded_status(&host, 1, 1),
        ObjectShroudStatus::Clear
    );
    pm.undo_shroud_reveal(&host, 100.0, 100.0, 20.0, 1u16 << 1)
        .unwrap();
    assert_eq!(
        pm.get_object_shrouded_status(&host, 1, 1),
        ObjectShroudStatus::Fogged
    );
    assert_eq!(
        pm.get_object_shrouded_status(&host, 1, 2),
        ObjectShroudStatus::Shrouded
    );
    // an unregistered object is clear; so is an always-visible one
    assert_eq!(
        pm.get_object_shrouded_status(&host, 99, 1),
        ObjectShroudStatus::Clear
    );
}

#[test]
fn an_immobile_object_with_a_ghost_keeps_its_module_while_a_player_remembers_it() {
    let (mut host, mut pm) = world();
    host.add(ScenarioObject::new(1, at(100.0, 100.0), sphere(5.0)));
    {
        let o = host.objects.get_mut(&1).unwrap();
        o.kind_of.set(KINDOF_IMMOBILE, 1);
        o.flags |= sflag::WANTS_GHOST;
        o.player = Some(3);
    }
    host.rel[1][3] = Relationship::Enemies;
    pm.register_object(&mut host, 1);
    pm.update(&mut host);
    assert_eq!(host.log.last(), Some(&ScenarioLog::CellChange(1)));
    // seen by player 1, then fogged: the building is remembered
    pm.do_shroud_reveal(&host, 100.0, 100.0, 20.0, 1u16 << 1)
        .unwrap();
    assert_eq!(
        pm.get_object_shrouded_status(&host, 1, 1),
        ObjectShroudStatus::Clear
    );
    pm.undo_shroud_reveal(&host, 100.0, 100.0, 20.0, 1u16 << 1)
        .unwrap();
    assert_eq!(
        pm.get_object_shrouded_status(&host, 1, 1),
        ObjectShroudStatus::Fogged,
        "an immobile building seen before stays visible in the fog"
    );
    let evs = pm.take_client_events();
    assert!(evs.contains(&PartitionClientEvent::GhostSnapShot {
        ghost: 1,
        player: 1
    }));
    // the object goes away; the ghost module stays in the cell with no object
    pm.unregister_object(1);
    assert!(pm
        .take_client_events()
        .contains(&PartitionClientEvent::GhostParentGone { ghost: 1 }));
    assert!(!pm.is_registered(1));
    assert_eq!(pm.objects_in_cell(2, 2), vec![None]);
    // queries skip it
    let r = pm.get_closest_object_at(
        &host,
        &at(100.0, 100.0),
        100.0,
        DistanceCalculationType::FromCenter2D,
        &[],
    );
    assert_eq!(r.object, None);
    // and the ghost manager releases it later
    pm.unregister_ghost_object(1);
    assert!(pm.objects_in_cell(2, 2).is_empty());
}

#[test]
fn threat_and_value_maps_fall_off_with_distance_and_undo_exactly() {
    let (host, mut pm) = world();
    let mask = 1u16 << 2;
    pm.do_threat_affect(&host, 200.0, 200.0, 80.0, 1000, mask)
        .unwrap();
    pm.do_value_affect(&host, 200.0, 200.0, 80.0, 500, mask)
        .unwrap();
    let centre = pm.get_cell_at(5, 5).unwrap();
    let t0 = centre.get_threat_value(2);
    assert!(t0 > 0 && t0 <= 1000);
    // the value map uses the same weight at half the amount, truncated
    assert_eq!(
        centre.get_cash_value(2),
        crate::common::base_type::real_to_unsigned_int(500.0 * (t0 as f32 / 1000.0))
    );
    assert!(pm.get_cell_at(7, 5).unwrap().get_threat_value(2) < t0);
    assert_eq!(pm.get_cell_at(5, 5).unwrap().get_threat_value(1), 0);
    let loc = pm.get_most_valuable_location(
        &host,
        2,
        filters::allow_player::SAME_PLAYER,
        ValueOrThreat::ThreatValue,
    );
    assert_eq!(
        loc,
        Some(Coord3D {
            x: 200.0,
            y: 200.0,
            z: 0.0
        }),
        "the player's own cells (ALLOW_SAME_PLAYER)"
    );
    pm.undo_threat_affect(&host, 200.0, 200.0, 80.0, 1000, mask)
        .unwrap();
    pm.undo_value_affect(&host, 200.0, 200.0, 80.0, 500, mask)
        .unwrap();
    for y in 0..11 {
        for x in 0..11 {
            let c = pm.get_cell_at(x, y).unwrap();
            assert_eq!((c.get_threat_value(2), c.get_cash_value(2)), (0, 0));
        }
    }
}

#[test]
fn the_breadth_first_walk_finds_the_nearest_cell_and_flags_the_last_row() {
    let (mut host, mut pm) = world();
    host.rel[0][1] = Relationship::Neutral;
    let mask = 1u16 << 1;
    pm.do_threat_affect(&host, 100.0, 100.0, 1.0, 100, mask)
        .unwrap();
    let (found, undefined) = pm.get_nearest_group_with_value(
        &host,
        0,
        filters::allow_player::NEUTRAL,
        ValueOrThreat::ThreatValue,
        &at(220.0, 100.0),
        10,
        true,
    );
    assert_eq!(
        found,
        Some(Coord3D {
            x: 80.0,
            y: 80.0,
            z: 0.0
        }),
        "the nearest cell with threat above 10: the cell (2,2) is the centre, its corner is at 80"
    );
    assert!(
        !undefined,
        "found before any cell of the last row was expanded"
    );
    // a walk that never finds anything runs through the last row
    let (none, undefined) = pm.get_nearest_group_with_value(
        &host,
        0,
        filters::allow_player::NEUTRAL,
        ValueOrThreat::CashValue,
        &at(20.0, 20.0),
        5,
        true,
    );
    assert_eq!(none, None);
    assert!(undefined, "upstream reads out of bounds there");
    // the cell stops are the callers': index of the cell that said stop
    let (idx, _) = pm.iterate_cells_breadth_first(&at(20.0, 20.0), |c| {
        c.get_cell_x() == 2 && c.get_cell_y() == 1
    });
    assert_eq!(idx, 13, "row 1 of an 11-wide grid, column 2");
    assert_eq!(
        pm.iterate_cells_breadth_first(&at(-500.0, 20.0), |_| true),
        (-1, true)
    );
}

#[test]
fn bresenham_visits_the_cells_between_two_points_and_terrain_extremes_come_from_the_cache() {
    let (_host, pm) = world();
    let mut visited = Vec::new();
    let r = pm.iterate_cells_along_line(&at(20.0, 20.0), &at(180.0, 100.0), |c| {
        visited.push((c.get_cell_x(), c.get_cell_y()));
        0
    });
    assert_eq!(r, 0);
    assert_eq!(visited.first(), Some(&(0, 0)));
    assert_eq!(visited.last(), Some(&(4, 2)));
    assert_eq!(
        visited.len(),
        5,
        "one cell per x step when the line is flatter than 45 degrees"
    );
    // stops early with the proc's value
    let mut n = 0;
    assert_eq!(
        pm.iterate_cells_along_line(&at(20.0, 20.0), &at(380.0, 20.0), |_| {
            n += 1;
            if n == 3 {
                7
            } else {
                0
            }
        }),
        7
    );
    let e = pm.estimate_terrain_extremes_along_line(&at(20.0, 20.0), &at(380.0, 380.0));
    assert!(e.is_valid && e.min_z < e.max_z);
    assert!(e.min_z_pos.is_some() && e.max_z_pos.is_some());
}

#[test]
fn line_of_sight_and_the_angle_and_vector_queries() {
    let (mut host, mut pm) = world();
    put(&mut host, &mut pm, 1, 100.0, 100.0, 2.0);
    put(&mut host, &mut pm, 2, 100.0, 200.0, 2.0);
    host.objects.get_mut(&1).unwrap().dir = (1.0, 0.0);
    pm.update(&mut host);
    // object 2 is straight up (+y): the angle from facing +x is +90 degrees (counter-clockwise positive)
    let a = pm.get_relative_angle_2d(&host, 1, 2).unwrap();
    assert!((a - core::f32::consts::FRAC_PI_2).abs() < 1e-6);
    assert_eq!(
        pm.get_relative_angle_2d_pos(&host, 1, &at(100.0, 100.0)),
        Some(0.0),
        "at the same spot: 0"
    );
    let (d, v) = pm
        .get_distance_squared(&host, 1, 2, DistanceCalculationType::FromCenter3D)
        .unwrap();
    assert_eq!(
        (d, v),
        (
            10000.0,
            Coord3D {
                x: 0.0,
                y: 100.0,
                z: 0.0
            }
        )
    );
    assert_eq!(
        pm.get_vector_to_pos(
            &host,
            1,
            &at(0.0, 100.0),
            DistanceCalculationType::FromCenter2D
        ),
        Some(Coord3D {
            x: -100.0,
            y: 0.0,
            z: 0.0
        })
    );
    let (g, _) = pm
        .get_goal_distance_squared_pos(
            &host,
            1,
            &at(100.0, 150.0),
            &at(100.0, 200.0),
            DistanceCalculationType::FromCenter2D,
        )
        .unwrap();
    assert_eq!(g, 2500.0);
    assert!(pm.is_clear_line_of_sight_terrain(
        &host,
        Some(1),
        &at(0.0, 0.0),
        Some(2),
        &at(0.0, 0.0)
    ));
    // a position under the hill is not visible from a position far above the other side
    assert!(!pm.is_clear_line_of_sight_terrain(
        &host,
        None,
        &Coord3D {
            x: 0.0,
            y: 0.0,
            z: -10.0
        },
        None,
        &Coord3D {
            x: 390.0,
            y: 390.0,
            z: -10.0
        }
    ));
}

#[test]
fn ground_or_structure_height_adds_the_tallest_structure() {
    let (mut host, mut pm) = world();
    host.add(ScenarioObject::new(
        1,
        at(100.0, 100.0),
        GeometryInfo::new(GeometryType::Box, false, 12.0, 4.0, 4.0),
    ));
    host.objects
        .get_mut(&1)
        .unwrap()
        .kind_of
        .set(KINDOF_STRUCTURE, 1);
    pm.register_object(&mut host, 1);
    put(&mut host, &mut pm, 2, 100.0, 100.0, 30.0); // not a structure: ignored
    pm.update(&mut host);
    let ground = ScenarioHost::terrain_height(100.0, 100.0);
    assert_eq!(
        pm.get_ground_or_structure_height(&host, 100.0, 100.0),
        ground + 12.0
    );
    assert_eq!(
        pm.get_ground_or_structure_height(&host, 300.0, 300.0),
        ScenarioHost::terrain_height(300.0, 300.0)
    );
}

#[test]
fn find_position_around_returns_a_legal_spot_or_the_off_map_centre() {
    let (mut host, mut pm) = world();
    put(&mut host, &mut pm, 1, 200.0, 200.0, 8.0);
    pm.update(&mut host);
    let opts = FindPositionOptions {
        flags: fpf::IGNORE_WATER,
        min_radius: 0.0,
        max_radius: 60.0,
        start_angle: 0.0,
        ..Default::default()
    };
    let p = pm
        .find_position_around(&mut host, &at(200.0, 200.0), &opts)
        .expect("a position exists");
    let d = ((p.x - 200.0).powi(2) + (p.y - 200.0).powi(2)).sqrt();
    assert!((0.0..=60.5).contains(&d));
    // the spot is clear of object 1 (a 5-radius sphere at the spot must not touch the 8-radius sphere)
    assert!(!pm.geom_collides_with_geom(
        &p,
        &sphere(5.0),
        0.0,
        &at(200.0, 200.0),
        &sphere(8.0),
        0.0
    ));
    // a centre off the pathfind map is returned unchanged
    let off = at(-50.0, 0.0);
    assert_eq!(pm.find_position_around(&mut host, &off, &opts), Some(off));
    // no radius at all: only the centre ring of one sample, which may fail
    let none = FindPositionOptions {
        flags: fpf::IGNORE_WATER | fpf::CLEAR_CELLS_ONLY,
        min_radius: 1000.0,
        max_radius: 10.0,
        start_angle: 0.0,
        ..Default::default()
    };
    assert_eq!(
        pm.find_position_around(&mut host, &at(200.0, 200.0), &none),
        None
    );
    // a random start angle uses the host's generator
    let rnd = FindPositionOptions {
        flags: fpf::IGNORE_WATER | fpf::IGNORE_ALL_OBJECTS,
        min_radius: 0.0,
        max_radius: 30.0,
        ..Default::default()
    };
    let before = host.rng;
    let _ = pm.find_position_around(&mut host, &at(200.0, 200.0), &rnd);
    assert_ne!(host.rng, before);
}

#[test]
fn fogged_cells_store_and_restore() {
    let (host, mut pm) = world();
    let mask = 1u16 << 1;
    pm.do_shroud_reveal(&host, 100.0, 100.0, 50.0, mask)
        .unwrap();
    pm.undo_shroud_reveal(&host, 100.0, 100.0, 50.0, mask)
        .unwrap();
    let mut store = ShroudStatusStoreRestore::default();
    pm.store_fogged_cells(&mut store, true);
    assert_eq!(store.cells_wide, 11);
    let fogged: usize = store.fogged_or_revealed[1]
        .iter()
        .filter(|&&b| b == STORE_FOG)
        .count();
    assert!(fogged > 0);
    assert!(store.fogged_or_revealed[0]
        .iter()
        .all(|&b| b == STORE_DONTTOUCH));
    // a fresh manager restored from the store fogs the same cells
    let mut pm2 = PartitionManager::new();
    pm2.init(&host);
    pm2.restore_fogged_cells(&host, &store, true);
    for y in 0..11 {
        for x in 0..11 {
            assert_eq!(
                pm2.get_cell_at(x, y)
                    .unwrap()
                    .get_shroud_status_for_player(1),
                pm.get_cell_at(x, y)
                    .unwrap()
                    .get_shroud_status_for_player(1)
            );
        }
    }
    // permanently revealed cells: a second pass
    pm.reveal_map_for_player_permanently(&host, 2);
    pm.store_fogged_cells(&mut store, false);
    assert!(store.fogged_or_revealed[2]
        .iter()
        .all(|&b| b == STORE_PERMANENTLY_REVEALED));
}

#[test]
fn whole_map_operations() {
    let (host, mut pm) = world();
    pm.reveal_map_for_player(&host, 1);
    assert!((0..11).all(|y| (0..11).all(|x| pm
        .get_cell_at(x, y)
        .unwrap()
        .get_shroud_status_for_player(1)
        == CellShroudStatus::Fogged)));
    pm.reveal_map_for_player_permanently(&host, 1);
    assert!((0..11).all(|y| (0..11).all(|x| pm
        .get_cell_at(x, y)
        .unwrap()
        .get_shroud_status_for_player(1)
        == CellShroudStatus::Clear)));
    pm.undo_reveal_map_for_player_permanently(&host, 1);
    pm.shroud_map_for_player(&host, 1);
    assert_eq!(
        pm.get_cell_at(3, 3)
            .unwrap()
            .get_shroud_status_for_player(1),
        CellShroudStatus::Shrouded
    );
    pm.take_client_events();
    pm.refresh_shroud_for_local_player(&host);
    let evs = pm.take_client_events();
    assert_eq!(evs[0], PartitionClientEvent::ClearShroud);
    assert_eq!(
        evs.len(),
        122,
        "ClearShroud, then one SetShroudLevel per cell of the 11 x 11 grid"
    );
}

#[test]
fn the_crc_covers_the_shroud_and_the_save_round_trips() {
    let (host, mut pm) = world();
    let crc = |pm: &mut PartitionManager| {
        let mut x = XferCrc::new();
        pm.crc(&mut x).unwrap();
        x.get_crc()
    };
    let c0 = crc(&mut pm);
    pm.do_shroud_reveal(&host, 100.0, 100.0, 50.0, 1u16 << 1)
        .unwrap();
    let c1 = crc(&mut pm);
    assert_ne!(c0, c1);
    pm.queue_undo_shroud_reveal(&host, 100.0, 100.0, 50.0, 1u16 << 1);

    let mut save = XferSave::new();
    save.open(b"partition").unwrap();
    Snapshot::xfer(&mut pm, &mut save).unwrap();
    save.close().unwrap();
    let bytes = save.take_bytes();

    let mut pm2 = PartitionManager::new();
    pm2.init(&host);
    let mut load = XferLoad::new(bytes.clone());
    load.open(b"partition").unwrap();
    Snapshot::xfer(&mut pm2, &mut load).unwrap();
    load.close().unwrap();
    assert_eq!(crc(&mut pm2), c1);
    assert_eq!(pm2.pending_undo_shroud_reveals().count(), 1);
    assert!(pm2.needs_shroud_refresh() && !pm2.get_updated_since_last_reset());
    pm2.refresh_shroud_for_local_player(&host);
    assert!(!pm2.needs_shroud_refresh());

    // a save from a different map size is refused
    let host3 = ScenarioHost {
        extent: Some(crate::common::base_type::Region3D {
            lo: at(0.0, 0.0),
            hi: Coord3D {
                x: 200.0,
                y: 200.0,
                z: 0.0,
            },
        }),
        ..Default::default()
    };
    let mut pm3 = PartitionManager::new();
    pm3.init(&host3);
    let mut load = XferLoad::new(bytes);
    load.open(b"partition").unwrap();
    assert!(Snapshot::xfer(&mut pm3, &mut load).is_err());
}

#[test]
fn reset_forgets_the_grid_and_the_queue() {
    let (host, mut pm) = world();
    pm.queue_undo_shroud_reveal(&host, 100.0, 100.0, 50.0, 2);
    pm.reset();
    assert_eq!(
        (
            pm.get_cell_count_x(),
            pm.get_cell_size(),
            pm.get_updated_since_last_reset()
        ),
        (0, 0.0, false)
    );
    assert_eq!(pm.pending_undo_shroud_reveals().count(), 0);
}

#[test]
fn the_world_owns_a_partition_manager_and_forwards_its_client_events() {
    let mut w = crate::logic::world::World::new();
    let host = ScenarioHost::default();
    w.partition.init(&host);
    w.partition
        .do_shroud_reveal(&host, 100.0, 100.0, 50.0, 1u16 << 1)
        .unwrap();
    let evs = w.take_client_events();
    assert!(evs.iter().any(|e| matches!(
        e,
        crate::logic::client_event::ClientEvent::Partition(
            PartitionClientEvent::SetShroudLevel { .. }
        )
    )));
    assert!(w.take_client_events().is_empty());
}
