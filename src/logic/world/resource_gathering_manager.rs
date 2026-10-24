// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/ResourceGatheringManager.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/ResourceGatheringManager.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   ResourceGatheringManager.cpp: Graham Smallwood, January 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ResourceGatheringManager`: a player's record of its supply centers and
//! of the supply warehouses on the map, and the two questions a supply truck
//! asks it: "what warehouse should I go to?" and "what center should I
//! return to?".
//!
//! # Translation choices
//!
//! - **Pointers are ids.** The two `std::list<ObjectID>` are `Vec<ObjectId>`
//!   in upstream's order. A `NULL` object argument is `Option::None`.
//! - **The units it calls are cut points.** `queryObject->getAI()` is the
//!   object's AI module ([`Object::ai`](crate::logic::world::object::Object));
//!   `getSupplyTruckAIInterface()`, `getPreferredDockID()` and
//!   `getWarehouseScanDistance()` are the [`ModuleCall`]s
//!   `GetSupplyTruckPreferredDockId` and `GetSupplyTruckWarehouseScanDistance`
//!   (an AI module that does not answer the first has no such interface, as
//!   upstream's `NULL`); `isClearToApproach(docker)` is
//!   `ModuleCall::IsClearToApproach`; `findUpdateModule(key)` is
//!   [`World::find_module`] by class name;
//!   `TheActionManager->canTransferSuppliesAt(a, b)` is
//!   [`World::can_transfer_supplies_at`], a table of the world until
//!   `ActionManager` is translated;
//!   `ThePartitionManager->getDistanceSquared(a, b, FROM_CENTER_3D)` is
//!   [`dist_calc`](crate::logic::world::partition::dist_calc) on the two
//!   positions (the partition manager is verified by
//!   `differential_partition.rs`).
//! - **Undefined behaviour is an error** ([`ResourceError`]): a destination
//!   with no dock interface that `canTransferSuppliesAt` accepted
//!   (`dockInterface->isClearToApproach` on `NULL`), and a supply center
//!   that no longer exists, for which `findBestSupplyCenter` erases from
//!   **`m_supplyWarehouses`** with an iterator of **`m_supplyCenters`**.
//!
//! # Upstream defects kept
//!
//! - **`findBestSupplyCenter` erases a vanished center from the wrong
//!   list** (see above): undefined behaviour, reported as
//!   [`ResourceError::EraseFromWrongList`], not reproduced.
//! - `computeRelativeCost` reads `FLT_MAX` as "no": a real cost of exactly
//!   `FLT_MAX` is never chosen.
//! - A warehouse is chosen only if its distance is under the truck's scan
//!   distance squared, but a *preferred dock* is chosen at any distance
//!   ("the PreferedDock will override this").
//!
//! # Verified
//!
//! Against upstream's own `ResourceGatheringManager.cpp`, compiled
//! unchanged, by `tests/differential_resource_gathering.rs`. Result,
//! 2026-10-18: 1,500 random programs (215,917 lines: returns, every call the
//! manager makes with its arguments, the bytes of every save, both lists after
//! every operation), **0 differ**; 12 planted bugs, all caught; two classes of
//! undefined input are errors here and are never sent to the C++.

use crate::common::game_type::ObjectId;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::partition::{dist_calc, DistanceCalculationType, HUGE_DIST_SQR};

/// An input for which upstream is undefined behaviour (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceError {
    /// `destObject->getDockUpdateInterface()` is `NULL` and `isClearToApproach` is called on it.
    NoDockInterface(ObjectId),
    /// A supply center in the list no longer exists: `m_supplyWarehouses.erase(iterator)` with an iterator of
    /// `m_supplyCenters`.
    EraseFromWrongList(ObjectId),
}

/// Appends an `rgm ...` line to the test trace (`World::cuts.trace`), when it is on: the record of a call the
/// manager makes, which the differential test compares with the stand-ins' records.
fn trace(world: &mut World, line: impl FnOnce() -> String) {
    if let Some(t) = &mut world.cuts.trace {
        t.push(line());
    }
}

/// `class ResourceGatheringManager`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResourceGatheringManager {
    /// `m_supplyWarehouses`.
    supply_warehouses: Vec<ObjectId>,
    /// `m_supplyCenters`.
    supply_centers: Vec<ObjectId>,
}

impl ResourceGatheringManager {
    /// `ResourceGatheringManager::ResourceGatheringManager()`.
    pub fn new() -> ResourceGatheringManager {
        ResourceGatheringManager::default()
    }

    /// `m_supplyWarehouses`, in order.
    pub fn supply_warehouses(&self) -> &[ObjectId] {
        &self.supply_warehouses
    }

    /// `m_supplyCenters`, in order.
    pub fn supply_centers(&self) -> &[ObjectId] {
        &self.supply_centers
    }

    /// `addSupplyCenter(newCenter)`: "I captured or built a Supply Center, so record it".
    pub fn add_supply_center(&mut self, new_center: Option<ObjectId>) {
        if let Some(id) = new_center {
            self.supply_centers.push(id);
        }
    }

    /// `removeSupplyCenter(oldCenter)`: every entry for that object.
    pub fn remove_supply_center(&mut self, old_center: Option<ObjectId>) {
        if let Some(id) = old_center {
            self.supply_centers.retain(|c| *c != id);
        }
    }

    /// `addSupplyWarehouse(newWarehouse)`.
    pub fn add_supply_warehouse(&mut self, new_warehouse: Option<ObjectId>) {
        if let Some(id) = new_warehouse {
            self.supply_warehouses.push(id);
        }
    }

    /// `removeSupplyWarehouse(oldWarehouse)`: every entry for that object.
    pub fn remove_supply_warehouse(&mut self, old_warehouse: Option<ObjectId>) {
        if let Some(id) = old_warehouse {
            self.supply_warehouses.retain(|c| *c != id);
        }
    }

    /// `computeRelativeCost(queryObject, destObject, pureDistanceSquared)`: "a good score is a very small
    /// number"; `FLT_MAX` for anything that cannot be used. The distance is written only when it is computed.
    fn compute_relative_cost(world: &mut World, query: Option<ObjectId>, dest: Option<ObjectId>, pure_distance_squared: Option<&mut f32>) -> Result<f32, ResourceError> {
        let (Some(q), Some(d)) = (query, dest) else { return Ok(f32::MAX) };
        if world.objects.find(q).is_none() || world.objects.find(d).is_none() {
            return Ok(f32::MAX);
        }
        let can = world.can_transfer_supplies_at(q, d);
        trace(world, || format!("rgm canTransfer {q} {d} -> {}", u32::from(can)));
        if !can {
            return Ok(f32::MAX); // handles emptyness and alliances
        }
        let Some(dock) = world.get_dock_update_interface(d) else { return Err(ResourceError::NoDockInterface(d)) };
        let clear = world.module_call(d, dock, ModuleCall::IsClearToApproach { docker: q }).as_bool();
        trace(world, || format!("rgm clear {d} {q} -> {}", u32::from(clear)));
        if !clear {
            return Ok(f32::MAX);
        }
        // since we don't care about the distance as a distance per se, but rather as a goodness-factor, save some
        // time by getting the dist-sqr (srj)
        let (pa, pb) = (world.objects.find(q).map(|o| o.get_position()), world.objects.find(d).map(|o| o.get_position()));
        let (Some(pa), Some(pb)) = (pa, pb) else { return Ok(f32::MAX) };
        let dist_squared = dist_calc(DistanceCalculationType::FromCenter3D, &pa, None, &pb, None, HUGE_DIST_SQR).dist_sqr;
        trace(world, || format!("rgm dist {q} {d} -> {:08x}", dist_squared.to_bits()));
        if let Some(p) = pure_distance_squared {
            *p = dist_squared;
        }
        // I need the distance, but I don't want to count on the coincidence that the abstract 'cost' this
        // function returns happens to be just the distance, since it could become more complicated
        Ok(dist_squared)
    }

    /// The supply-truck interface of the query object's AI: the preferred dock and, if asked, the scan distance.
    /// `None` if the object has no AI or its AI is not a supply truck's.
    fn supply_truck_of(world: &mut World, query: ObjectId) -> Option<(ObjectId, ModuleIndexOfAi)> {
        let ai = world.objects.find(query)?.ai?;
        match world.module_call(query, ai, ModuleCall::GetSupplyTruckPreferredDockId) {
            crate::logic::world::module_call::ModuleReply::Id(dock) => Some((dock, ai)),
            _ => None,
        }
    }

    /// `findBestSupplyWarehouse(queryObject)`: "what warehouse should this truck go to?". A truck's preferred
    /// dock wins if it is a warehouse and usable; otherwise the nearest usable warehouse within the truck's scan
    /// distance (100000 squared when the truck has none). Warehouses that no longer exist are dropped from the
    /// list as they are met.
    pub fn find_best_supply_warehouse(&mut self, world: &mut World, query: Option<ObjectId>) -> Result<Option<ObjectId>, ResourceError> {
        let mut best_warehouse = None;
        let mut max_distance_squared: f32 = 100000.0;

        let Some(q) = query else { return Ok(None) };
        let Some(qo) = world.objects.find(q) else { return Ok(None) };
        if qo.ai.is_none() {
            return Ok(None);
        }

        if let Some((dock_id, ai)) = Self::supply_truck_of(world, q) {
            // check for a dock override being set
            if let Some(dock) = world.find_object(dock_id).map(|o| o.id) {
                // if remotely okay, let user win
                if world.find_module(dock, b"SupplyWarehouseDockUpdate").is_some()
                    && Self::compute_relative_cost(world, Some(q), Some(dock), None)? != f32::MAX
                {
                    return Ok(Some(dock));
                }
            }
            // "Design wants a harvester to give up and return to base if it is 'too far' to the warehouse."
            let scan = world.module_call(q, ai, ModuleCall::GetSupplyTruckWarehouseScanDistance).as_real(0.0);
            max_distance_squared = scan * scan;
        }

        // otherwise, search for a good one
        let mut best_cost = f32::MAX;
        let mut i = 0;
        while i < self.supply_warehouses.len() {
            let current_id = self.supply_warehouses[i];
            match world.find_object(current_id).map(|o| o.id) {
                None => {
                    self.supply_warehouses.remove(i);
                }
                Some(current) => {
                    let mut distance_squared = 0.0f32;
                    let current_cost = Self::compute_relative_cost(world, Some(q), Some(current), Some(&mut distance_squared))?;
                    if current_cost < best_cost && distance_squared < max_distance_squared {
                        best_warehouse = Some(current);
                        best_cost = current_cost;
                    }
                    i += 1;
                }
            }
        }
        Ok(best_warehouse)
    }

    /// `findBestSupplyCenter(queryObject)`: "what center should this truck return to?". As for warehouses, with
    /// no distance limit; a center that no longer exists is undefined behaviour upstream (see the module docs).
    pub fn find_best_supply_center(&mut self, world: &mut World, query: Option<ObjectId>) -> Result<Option<ObjectId>, ResourceError> {
        let mut best_center = None;

        let Some(q) = query else { return Ok(None) };
        let Some(qo) = world.objects.find(q) else { return Ok(None) };
        if qo.ai.is_none() {
            return Ok(None);
        }

        if let Some((dock_id, _ai)) = Self::supply_truck_of(world, q) {
            if let Some(dock) = world.find_object(dock_id).map(|o| o.id) {
                if world.find_module(dock, b"SupplyCenterDockUpdate").is_some()
                    && Self::compute_relative_cost(world, Some(q), Some(dock), None)? != f32::MAX
                {
                    return Ok(Some(dock));
                }
            }
        }

        let mut best_cost = f32::MAX;
        for i in 0..self.supply_centers.len() {
            let current_id = self.supply_centers[i];
            match world.find_object(current_id).map(|o| o.id) {
                None => return Err(ResourceError::EraseFromWrongList(current_id)),
                Some(current) => {
                    let current_cost = Self::compute_relative_cost(world, Some(q), Some(current), None)?;
                    if current_cost < best_cost {
                        best_center = Some(current);
                        best_cost = current_cost;
                    }
                }
            }
        }
        Ok(best_center)
    }
}

/// A module index of an object (the AI's).
type ModuleIndexOfAi = crate::logic::world::object::ModuleIndex;

impl Snapshot for ResourceGatheringManager {
    /// `crc`: empty.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// Version 1: the warehouse ids, the center ids.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        xfer.xfer_stl_object_id_list(&mut self.supply_warehouses)?;
        xfer.xfer_stl_object_id_list(&mut self.supply_centers)
    }

    /// `loadPostProcess`: empty.
    fn load_post_process(&mut self) {}
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::base_type::Coord3D;
    use crate::common::module::MODULEINTERFACE_BODY;
    use crate::common::thing_template::ThingTemplate;
    use crate::common::xfer_load::XferLoad;
    use crate::common::xfer_save::XferSave;
    use crate::logic::world::module_probe::{ProbeAnswers, ProbeModule};
    use crate::logic::world::object_create::{NewModule, NewObject};
    use crate::logic::world::team::Team;
    use crate::logic::world::update_body::UpdateBody;
    use std::sync::Arc;

    fn world() -> World {
        let mut w = World::new();
        w.cuts.neutral_default_team = 1;
        w.teams.insert_team(Team::new(1, 0)).unwrap();
        w.cuts.trace = Some(Vec::new());
        w
    }

    /// An object at (x, 0, 0) with these modules: `truck` is a supply truck with `(preferred, scan)`; `dock` is a
    /// dock interface that is clear to approach if `clear`; `module` is the class name of an extra named module.
    fn make(w: &mut World, x: f32, truck: Option<(ObjectId, f32)>, dock: Option<bool>, module: Option<&[u8]>) -> ObjectId {
        let mut modules = Vec::new();
        if let Some((preferred_dock, scan_distance)) = truck {
            let mut m = NewModule::plain(b"AIUpdate", b"ModuleTag_AI");
            m.is_ai = true;
            m.interfaces = MODULEINTERFACE_BODY;
            let answers = ProbeAnswers { supply_truck: true, preferred_dock, scan_distance, ..Default::default() };
            m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 1.0, Default::default())));
            modules.push(m);
        }
        if let Some(clear) = dock {
            let mut m = NewModule::plain(b"DockProbe", b"ModuleTag_Dock");
            let answers = ProbeAnswers { has: 1 << 2, clear_to_approach: clear, ..Default::default() };
            m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 1.0, Default::default())));
            modules.push(m);
        }
        if let Some(name) = module {
            modules.push(NewModule::plain(name, b"ModuleTag_Named"));
        }
        let id = w.new_object(NewObject { template: Arc::new(ThingTemplate::new(0)), status: Default::default(), team: None, modules }).expect("object");
        w.set_position(id, &Coord3D { x, y: 0.0, z: 0.0 });
        id
    }

    fn allow(w: &mut World, q: ObjectId, d: ObjectId) {
        w.cuts.can_transfer_supplies.insert((q, d));
    }

    #[test]
    fn the_nearest_usable_warehouse_within_the_scan_distance_wins() {
        let mut w = world();
        let truck = make(&mut w, 0.0, Some((0, 100.0)), None, None);
        let (near, far, blocked, dead) = (
            make(&mut w, 50.0, None, Some(true), None),
            make(&mut w, 90.0, None, Some(true), None),
            make(&mut w, 10.0, None, Some(false), None),
            make(&mut w, 20.0, None, Some(true), None),
        );
        let out_of_reach = make(&mut w, 150.0, None, Some(true), None);
        for d in [near, far, blocked, dead, out_of_reach] {
            allow(&mut w, truck, d);
        }
        let mut m = ResourceGatheringManager::new();
        for d in [far, blocked, near, dead, out_of_reach] {
            m.add_supply_warehouse(Some(d));
        }
        m.add_supply_warehouse(None);
        let _ = w.objects.remove(dead);
        assert_eq!(m.find_best_supply_warehouse(&mut w, Some(truck)), Ok(Some(near)));
        assert_eq!(m.supply_warehouses(), &[far, blocked, near, out_of_reach], "the vanished one is dropped");
        let trace = w.cuts.trace.clone().unwrap().join("\n");
        assert!(trace.contains(&format!("rgm clear {blocked} {truck} -> 0")) && trace.contains("rgm dist"));
        m.remove_supply_warehouse(Some(far));
        m.remove_supply_warehouse(None);
        assert_eq!(m.supply_warehouses(), &[blocked, near, out_of_reach]);
        // nobody to ask for, or an object with no AI
        assert_eq!(m.find_best_supply_warehouse(&mut w, None), Ok(None));
        assert_eq!(m.find_best_supply_warehouse(&mut w, Some(near)), Ok(None));
        assert_eq!(m.find_best_supply_warehouse(&mut w, Some(999)), Ok(None));
    }

    #[test]
    fn a_preferred_dock_that_is_a_usable_warehouse_wins_at_any_distance() {
        let mut w = world();
        let wh = make(&mut w, 5000.0, None, Some(true), Some(b"SupplyWarehouseDockUpdate"));
        let truck = make(&mut w, 0.0, Some((wh, 10.0)), None, None);
        allow(&mut w, truck, wh);
        let mut m = ResourceGatheringManager::new();
        assert_eq!(m.find_best_supply_warehouse(&mut w, Some(truck)), Ok(Some(wh)));
        // not usable: the search finds nothing in reach
        w.cuts.can_transfer_supplies.clear();
        assert_eq!(m.find_best_supply_warehouse(&mut w, Some(truck)), Ok(None));
    }

    #[test]
    fn centers_have_no_distance_limit_and_a_vanished_one_is_undefined() {
        let mut w = world();
        let truck = make(&mut w, 0.0, Some((0, 1.0)), None, None);
        let (c1, c2) = (make(&mut w, 9000.0, None, Some(true), None), make(&mut w, 8000.0, None, Some(true), None));
        allow(&mut w, truck, c1);
        allow(&mut w, truck, c2);
        let mut m = ResourceGatheringManager::new();
        m.add_supply_center(Some(c1));
        m.add_supply_center(Some(c2));
        assert_eq!(m.find_best_supply_center(&mut w, Some(truck)), Ok(Some(c2)));
        // (the object leaves the sleepy list first, as `processDestroyList` does: its helpers are update modules now)
        w.sched.erase_object_updates(&mut w.objects, c1);
        let _ = w.objects.remove(c1);
        assert_eq!(m.find_best_supply_center(&mut w, Some(truck)), Err(ResourceError::EraseFromWrongList(c1)));
        m.remove_supply_center(Some(c1));
        m.remove_supply_center(None);
        assert_eq!(m.supply_centers(), &[c2]);
        assert_eq!(m.find_best_supply_center(&mut w, None), Ok(None));
        assert_eq!(m.find_best_supply_center(&mut w, Some(c2)), Ok(None), "no AI");
        assert_eq!(m.find_best_supply_center(&mut w, Some(999)), Ok(None));
        // a destination the table accepts but that has no dock interface: undefined
        let nodock = make(&mut w, 1.0, None, None, None);
        allow(&mut w, truck, nodock);
        m.add_supply_center(Some(nodock));
        assert_eq!(m.find_best_supply_center(&mut w, Some(truck)), Err(ResourceError::NoDockInterface(nodock)));
    }

    #[test]
    fn the_lists_round_trip_through_xfer() {
        let mut m = ResourceGatheringManager::new();
        m.add_supply_center(Some(4));
        m.add_supply_warehouse(Some(7));
        m.add_supply_warehouse(Some(8));
        let mut save = XferSave::new();
        save.open(b"r").unwrap();
        m.xfer(&mut save).unwrap();
        save.close().unwrap();
        let mut load = XferLoad::new(save.take_bytes());
        load.open(b"r").unwrap();
        let mut back = ResourceGatheringManager::new();
        back.xfer(&mut load).unwrap();
        assert_eq!(back, m);
        let mut crc = crate::common::xfer_crc::XferCrc::new();
        back.crc(&mut crc).unwrap();
        back.load_post_process();
    }
}
