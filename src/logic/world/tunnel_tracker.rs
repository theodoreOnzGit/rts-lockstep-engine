// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/TunnelTracker.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/TunnelTracker.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   TunnelTracker.cpp: Graham Smallwood, March 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `TunnelTracker`: "the part of a Player's brain that holds the communal
//! passenger list of all tunnels". A `ContainModule` look-alike for a network
//! of tunnels, which belongs to a player (or, for a cave, to the cave system).
//!
//! # Translation choices
//!
//! - **Pointers are ids.** `m_containList` (`std::list<Object*>`) and
//!   `m_tunnelIDs` are `Vec<ObjectId>` in upstream's order. `getContainedBy()
//!   == deadTunnel` compares ids; `NULL` is [`INVALID_ID`].
//! - **The tracker is not in the world; its operations take the world.** A
//!   tracker lives in a `Player` or in the [`CaveSystem`](crate::logic::world::cave_system::CaveSystem);
//!   the caller checks it out of its slot for the call (`std::mem::take`),
//!   which is the checkout of `docs/world-design.md`. The operations that
//!   touch objects are methods `(&mut self, world: &mut World, ..)`.
//! - **`iterateContained(func, userData, reverse)`** is
//!   [`TunnelTracker::iterate_contained`] with a closure. Upstream saves the
//!   next element before calling `func` so that `func` may remove the current
//!   one; here the ids are copied first, so `func` sees every id that was in
//!   the list when the iteration began (removing another element from inside
//!   `func` is undefined behaviour upstream and is not supported).
//! - **Undefined behaviour becomes an error** ([`TunnelError`]): destroying
//!   a tunnel when no tunnel id is left to be the "valid tunnel" (`m_tunnelIDs.front()`
//!   of an empty list is read; a tracker that counts none wraps
//!   `m_tunnelCount--` to 4294967295, which is defined, and goes to that
//!   branch), a
//!   contained id that names no object (a dangling pointer upstream), and
//!   healing an object with no body module (`NULL->getMaxHealth()`).
//! - `loadPostProcess`'s partition, pathfinder and drawable calls are the
//!   world's cut points (`CutCall::PartitionUnregisterObject`,
//!   `PathfinderRemoveObjectFromPathfindMap`, `ObjectClientEvent::DrawableSetHidden`).
//!   Being a `Snapshot` with no world, the tracker's `load_post_process` is
//!   the inherent [`TunnelTracker::load_post_process_world`].
//! - `TheGlobalData->m_maxTunnelCapacity` is
//!   `world.cuts.globals.max_tunnel_capacity`.
//!
//! # Upstream defects kept
//!
//! - `removeFromContain` ignores its `exposeStealthUnits` argument.
//! - `xfer` on a load counts the ids it reads with an `UnsignedShort`
//!   (`for (UnsignedShort i = 0; i < m_containListSize; ++i)`) while the size
//!   is an `Int`, so a size above 65,535 never ends upstream (the file runs
//!   out first and the read throws end-of-file); the translation reports the
//!   counter's wrap as an error, the same outcome.
//! - `getCurNemesis` forgets a nemesis that is stealthed, undetected and not
//!   disguised, and one that is effectively dead; both clear the id.
//!
//! # Verified
//!
//! Against upstream's own `TunnelTracker.cpp` and `CaveSystem.cpp`, compiled
//! unchanged, by `tests/differential_tunnel_tracker.rs`. Result, 2026-10-18:
//! 1,500 random programs (878,064 lines: return values, every call the
//! tracker makes on the world with its arguments, the bytes of every save, and
//! the full state after every operation) and 20,000 in a longer run
//! (11,688,151 lines), **0 differ**; 12 planted bugs, all caught. The
//! undefined-behaviour inputs (six classes) are errors here and are never sent
//! to the C++. The `trace` lines (`tt ...`) this file writes into
//! `World::cuts.trace` are the test's record of the calls it makes.

use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::kind_of::{KINDOF_AIRCRAFT, KINDOF_INFANTRY, KINDOF_STRUCTURE, KINDOF_VEHICLE};
use crate::common::object_status_types::{OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_STEALTHED};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_HEALING, DEATH_NONE};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::object_cuts::CutCall;

/// `LOGICFRAMES_PER_SECOND`.
const LOGICFRAMES_PER_SECOND: u32 = 30;

/// Appends a `tt ...` line to the test trace (`World::cuts.trace`), when it is on. The
/// differential test compares these lines with the stand-ins' records of the calls
/// `TunnelTracker.cpp` makes (`onContainedBy`, `onRemovedFrom`, `destroyObject`,
/// `attemptHealing`).
fn trace(world: &mut World, line: impl FnOnce() -> String) {
    if let Some(t) = &mut world.cuts.trace {
        t.push(line());
    }
}

/// An input for which upstream is undefined behaviour (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TunnelError {
    /// `onTunnelDestroyed` left a non-zero count (a tracker that counted none
    /// wraps `m_tunnelCount--` to 4294967295) and an empty `m_tunnelIDs`, whose
    /// `front()` upstream then reads. The count and the id have already
    /// changed when this is returned.
    NoTunnelIds,
    /// A contained id names no object: upstream holds a dangling pointer.
    ContainedObjectGone(ObjectId),
    /// `getBodyModule()` is `NULL` for an object being healed.
    NoBodyModule(ObjectId),
    /// `throw SC_INVALID_DATA` (`loadPostProcess`: the list was not empty, or a
    /// saved id names no object).
    InvalidData,
}

/// `class TunnelTracker`.
#[derive(Clone, Debug, PartialEq)]
pub struct TunnelTracker {
    /// `m_tunnelIDs`: "I have to try to keep track of these because Caves need to iterate on them".
    tunnel_ids: Vec<ObjectId>,
    /// `m_containList`.
    contain_list: Vec<ObjectId>,
    /// `m_xferContainList`: "for loading of m_containList during post processing".
    xfer_contain_list: Vec<ObjectId>,
    /// `m_containListSize`.
    contain_list_size: i32,
    /// `m_tunnelCount`: how many tunnels have registered.
    tunnel_count: u32,
    /// `m_curNemesisID`.
    cur_nemesis_id: ObjectId,
    /// `m_nemesisTimestamp`.
    nemesis_timestamp: u32,
}

impl Default for TunnelTracker {
    fn default() -> Self {
        TunnelTracker::new()
    }
}

impl TunnelTracker {
    /// `TunnelTracker::TunnelTracker()`.
    pub fn new() -> TunnelTracker {
        TunnelTracker {
            tunnel_ids: Vec::new(),
            contain_list: Vec::new(),
            xfer_contain_list: Vec::new(),
            contain_list_size: 0,
            tunnel_count: 0,
            cur_nemesis_id: INVALID_ID,
            nemesis_timestamp: 0,
        }
    }

    /// `getContainCount()`: `m_containListSize` (an `Int` converted to `UnsignedInt`).
    pub fn get_contain_count(&self) -> u32 {
        self.contain_list_size as u32
    }

    /// `getContainedItemsList()`, as ids.
    pub fn get_contained_items_list(&self) -> &[ObjectId] {
        &self.contain_list
    }

    /// `friend_getTunnelCount()`.
    pub fn friend_get_tunnel_count(&self) -> u32 {
        self.tunnel_count
    }

    /// `m_xferContainList`: the ids read by a load and not yet post-processed.
    pub fn get_xfer_contain_list(&self) -> &[ObjectId] {
        &self.xfer_contain_list
    }

    /// `m_containListSize`, as the signed value that is saved.
    pub fn get_contain_list_size(&self) -> i32 {
        self.contain_list_size
    }

    /// `m_curNemesisID` and `m_nemesisTimestamp`, as stored (`getCurNemesis` may still forget it).
    pub fn get_nemesis_state(&self) -> (ObjectId, u32) {
        (self.cur_nemesis_id, self.nemesis_timestamp)
    }

    /// `getContainerList()`: the tunnels, as ids.
    pub fn get_container_list(&self) -> &[ObjectId] {
        &self.tunnel_ids
    }

    /// `iterateContained(func, userData, reverse)`. See the module docs for
    /// how a callback that removes objects is treated.
    pub fn iterate_contained<F>(&self, world: &mut World, reverse: bool, mut func: F) -> Result<(), TunnelError>
    where
        F: FnMut(&mut World, ObjectId) -> Result<(), TunnelError>,
    {
        let mut ids = self.contain_list.clone();
        if reverse {
            ids.reverse();
        }
        for id in ids {
            func(world, id)?;
        }
        Ok(())
    }

    /// `getContainMax()`: `TheGlobalData->m_maxTunnelCapacity`.
    pub fn get_contain_max(&self, world: &World) -> i32 {
        world.cuts.globals.max_tunnel_capacity
    }

    /// `updateNemesis(target)`.
    pub fn update_nemesis(&mut self, world: &mut World, target: Option<ObjectId>) {
        let cur = self.get_cur_nemesis(world);
        match cur {
            None => {
                if let Some(t) = target {
                    if let Some(o) = world.objects.find(t) {
                        if o.is_kind_of(KINDOF_VEHICLE)
                            || o.is_kind_of(KINDOF_STRUCTURE)
                            || o.is_kind_of(KINDOF_INFANTRY)
                            || o.is_kind_of(KINDOF_AIRCRAFT)
                        {
                            self.cur_nemesis_id = o.id;
                            self.nemesis_timestamp = world.frame();
                        }
                    }
                }
            }
            Some(c) => {
                if Some(c) == target {
                    self.nemesis_timestamp = world.frame();
                }
            }
        }
    }

    /// `getCurNemesis()`: the target the tunnel network is defending against,
    /// forgotten after four seconds, when it is gone, dead, or stealthed.
    pub fn get_cur_nemesis(&mut self, world: &World) -> Option<ObjectId> {
        if self.cur_nemesis_id == INVALID_ID {
            return None;
        }
        if self.nemesis_timestamp.wrapping_add(4u32.wrapping_mul(LOGICFRAMES_PER_SECOND)) < world.frame() {
            self.cur_nemesis_id = INVALID_ID;
            return None;
        }
        let mut target = world.objects.find(self.cur_nemesis_id).map(|o| o.id);
        if let Some(o) = world.objects.find(self.cur_nemesis_id) {
            // "If the enemy unit is stealthed and not detected, then we can't attack it!"
            if o.test_status(OBJECT_STATUS_STEALTHED) && !o.test_status(OBJECT_STATUS_DETECTED) && !o.test_status(OBJECT_STATUS_DISGUISED) {
                target = None;
            }
        }
        if let Some(o) = world.objects.find(self.cur_nemesis_id) {
            if target.is_some() && o.is_effectively_dead() {
                target = None;
            }
        }
        if target.is_none() {
            self.cur_nemesis_id = INVALID_ID;
        }
        target
    }

    /// `isValidContainerFor(obj, checkCapacity)`: "Dustin wants ALL units to
    /// be able to use tunnels! srj sez: um, except aircraft."
    pub fn is_valid_container_for(&self, world: &World, obj: Option<ObjectId>, check_capacity: bool) -> bool {
        if let Some(o) = obj.and_then(|id| world.objects.find(id)) {
            if !o.is_kind_of(KINDOF_AIRCRAFT) {
                if check_capacity {
                    let contain_max = self.get_contain_max(world);
                    // `Int containCount = getContainCount();`: the unsigned count converted back
                    let contain_count = self.get_contain_count() as i32;
                    return contain_count < contain_max;
                }
                return true;
            }
        }
        false
    }

    /// `addToContainList(obj)`.
    pub fn add_to_contain_list(&mut self, obj: ObjectId) {
        self.contain_list.push(obj);
        self.contain_list_size = self.contain_list_size.wrapping_add(1);
    }

    /// `removeFromContain(obj, exposeStealthUnits)`. Upstream ignores the
    /// second argument.
    pub fn remove_from_contain(&mut self, obj: ObjectId) {
        if let Some(at) = self.contain_list.iter().position(|o| *o == obj) {
            self.contain_list.remove(at);
            self.contain_list_size = self.contain_list_size.wrapping_sub(1);
        }
    }

    /// `isInContainer(obj)`.
    pub fn is_in_container(&self, obj: ObjectId) -> bool {
        self.contain_list.contains(&obj)
    }

    /// `onTunnelCreated(newTunnel)`.
    pub fn on_tunnel_created(&mut self, new_tunnel: ObjectId) {
        self.tunnel_count = self.tunnel_count.wrapping_add(1);
        self.tunnel_ids.push(new_tunnel);
    }

    /// `onTunnelDestroyed(deadTunnel)`: with the last tunnel gone, everyone
    /// inside is destroyed ("Cave in!"); otherwise nobody inside may remember
    /// the dead tunnel as the one they entered.
    pub fn on_tunnel_destroyed(&mut self, world: &mut World, dead_tunnel: ObjectId) -> Result<(), TunnelError> {
        // `m_tunnelCount--`: unsigned, so a tracker that counts none wraps to 4294967295
        self.tunnel_count = self.tunnel_count.wrapping_sub(1);
        self.tunnel_ids.retain(|t| *t != dead_tunnel);

        if self.tunnel_count == 0 {
            // Kill everyone in our contain list.  Cave in!
            self.iterate_contained(world, false, TunnelTracker::destroy_object)?;
            self.contain_list.clear();
            self.contain_list_size = 0;
        } else {
            let Some(front) = self.tunnel_ids.first().copied() else {
                // `m_tunnelIDs.front()` of an empty list
                return Err(TunnelError::NoTunnelIds);
            };
            let valid_tunnel = world.objects.find(front).map_or(INVALID_ID, |o| o.id);
            // Otherwise, make sure nobody inside remembers the dead tunnel as the one they entered
            // (scripts need to use so there must be something valid here)
            for id in self.contain_list.clone() {
                let contained_by = match world.objects.find(id) {
                    Some(o) => o.contained_by,
                    None => return Err(TunnelError::ContainedObjectGone(id)),
                };
                if contained_by == dead_tunnel {
                    trace(world, || format!("tt onContainedBy {id} {valid_tunnel}"));
                    world.on_contained_by(id, valid_tunnel);
                }
            }
        }
        Ok(())
    }

    /// `destroyObject(obj, userData)`, the iterate callback: "Now that
    /// tunnels consider ContainedBy to be 'the tunnel you entered', I need to
    /// say goodbye like other contain types so they don't look us up on their
    /// deletion and crash".
    pub fn destroy_object(world: &mut World, obj: ObjectId) -> Result<(), TunnelError> {
        let Some(contained_by) = world.objects.find(obj).map(|o| o.contained_by) else {
            return Err(TunnelError::ContainedObjectGone(obj));
        };
        trace(world, || format!("tt onRemovedFrom {obj} {contained_by}"));
        world.on_removed_from(obj);
        trace(world, || format!("tt destroy {obj}"));
        world.destroy_object(obj);
        Ok(())
    }

    /// `healObjects(frames)`: heal all objects within the tunnel network.
    pub fn heal_objects(&self, world: &mut World, frames: f32) -> Result<(), TunnelError> {
        self.iterate_contained(world, false, |w, id| TunnelTracker::heal_object(w, id, frames))
    }

    /// `healObject(obj, frames)`: heal one object within the tunnel network
    /// system. In the first `frames` since it entered, it gets a sliver of its
    /// maximum health each call as if it were at zero and would be full at
    /// `frames`; after that, its maximum.
    pub fn heal_object(world: &mut World, obj: ObjectId, frames_for_full_heal: f32) -> Result<(), TunnelError> {
        // setup the healing damageInfo structure with all but the amount
        let mut heal_info = DamageInfo::default();
        heal_info.input.damage_type = DAMAGE_HEALING;
        heal_info.input.death_type = DEATH_NONE;
        // get body module of the thing to heal
        let contained_by_frame = match world.objects.find(obj) {
            Some(o) => o.contained_by_frame,
            None => return Err(TunnelError::ContainedObjectGone(obj)),
        };
        let Some(body) = world.get_body_module(obj) else { return Err(TunnelError::NoBodyModule(obj)) };
        let max_health = world.module_call(obj, body, ModuleCall::GetMaxHealth).as_real(0.0);
        // if we've been in here long enough ... set our health to max
        if world.frame().wrapping_sub(contained_by_frame) as f32 >= frames_for_full_heal {
            // set the amount to max just to be sure we're at the top
            heal_info.input.amount = max_health;
        } else {
            heal_info.input.amount = max_health / frames_for_full_heal;
        }
        trace(world, || {
            let i = &heal_info.input;
            format!(
                "tt heal {obj} src {} type {} fx {} death {} amount {:08x} kill {}",
                i.source_id,
                i.damage_type,
                i.damage_fx_override,
                i.death_type,
                i.amount.to_bits(),
                u32::from(i.kill)
            )
        });
        world.module_call(obj, body, ModuleCall::AttemptHealing(heal_info));
        Ok(())
    }

    /// The post-process phase after a load: the ids read by [`TunnelTracker::xfer`]
    /// become the contain list, and each contained object leaves its group,
    /// the partition manager, the pathfind map and the screen.
    pub fn load_post_process_world(&mut self, world: &mut World) -> Result<(), TunnelError> {
        // sanity, the contain list should be empty until we post process the id list
        if !self.contain_list.is_empty() {
            return Err(TunnelError::InvalidData);
        }
        // translate each object ids on the xferContainList into real object pointers in the contain list
        for id in self.xfer_contain_list.clone() {
            if world.objects.find(id).is_none() {
                return Err(TunnelError::InvalidData);
            }
            // push on the back of the contain list
            self.contain_list.push(id);
            // "This is in OpenContain as a fix, but not here."
            // remove object from its group (if any)
            world.leave_group(id);
            // remove rider from partition manager
            world.partition_unregister_object(id);
            // hide the drawable associated with rider
            if world.objects.find(id).is_some_and(|o| o.drawable != crate::common::game_type::INVALID_DRAWABLE_ID) {
                world.emit(ObjectClientEvent::DrawableSetHidden { object_id: id, hidden: true });
            }
            // remove object from pathfind map
            world.cut(CutCall::PathfinderRemoveObjectFromPathfindMap { object: id });
        }
        // we're done with the xfer contain list now
        self.xfer_contain_list.clear();
        Ok(())
    }
}

impl Snapshot for TunnelTracker {
    /// `crc`: upstream writes nothing.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// Version 1: the tunnel ids, the contain list size, the contained ids
    /// (saved from the list; on a load pushed to `m_xferContainList`, read
    /// as many times as `m_containListSize` says, with an `UnsignedShort`
    /// counter as upstream), the tunnel count.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        // tunnel object id list
        xfer.xfer_stl_object_id_list(&mut self.tunnel_ids)?;
        // contain list count
        xfer.xfer_int(&mut self.contain_list_size)?;
        // contain list data
        let mut object_id: ObjectId = INVALID_ID;
        if xfer.get_xfer_mode() == XferMode::Save {
            for id in &self.contain_list {
                object_id = *id;
                xfer.xfer_object_id(&mut object_id)?;
            }
        } else {
            // `for (UnsignedShort i = 0; i < m_containListSize; ++i)`: the counter is 16 bits, so a
            // size above 65535 never ends upstream; the translation stops at the counter's wrap and
            // reports it, which is the only way out of an endless loop.
            let mut i: u16 = 0;
            while (i as i32) < self.contain_list_size {
                xfer.xfer_object_id(&mut object_id)?;
                self.xfer_contain_list.push(object_id);
                i = match i.checked_add(1) {
                    Some(n) => n,
                    None => return Err(XferStatus::InvalidParameters),
                };
            }
        }
        // tunnel count
        xfer.xfer_unsigned_int(&mut self.tunnel_count)
    }

    /// Needs the world: see [`TunnelTracker::load_post_process_world`].
    fn load_post_process(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::kind_of::{KINDOF_INFANTRY, KINDOF_SELECTABLE};
    use crate::common::object_status_types::OBJECT_STATUS_STEALTHED as STEALTHED_BIT;
    use crate::common::module::{MODULEINTERFACE_BODY, MODULEINTERFACE_DAMAGE};
    use crate::common::thing_template::{ObjectTemplateValues, ThingTemplate};
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

    fn make(w: &mut World, kinds: &[i32], health: Option<f32>) -> ObjectId {
        let v = ObjectTemplateValues { kind_of: kinds.to_vec(), ..Default::default() };
        let mut modules = Vec::new();
        if let Some(h) = health {
            let mut m = NewModule::plain(b"ProbeBody", b"ModuleTag_Body");
            m.interfaces = MODULEINTERFACE_BODY | MODULEINTERFACE_DAMAGE;
            let mut p = ProbeModule::new(ProbeAnswers::default(), 100.0, Default::default());
            p.health = h;
            m.body = Some(UpdateBody::ModuleProbe(p));
            modules.push(m);
        }
        let tt = Arc::new(ThingTemplate::from_object_values(&v));
        w.new_object(NewObject { template: tt, status: Default::default(), team: None, modules }).expect("object")
    }

    fn health(w: &mut World, id: ObjectId) -> f32 {
        let b = w.get_body_module(id).unwrap();
        w.module_call(id, b, ModuleCall::GetHealth).as_real(0.0)
    }

    #[test]
    fn containing_is_a_list_with_a_count_and_a_capacity() {
        let mut w = world();
        w.cuts.globals.max_tunnel_capacity = 2;
        let (a, b) = (make(&mut w, &[KINDOF_INFANTRY], None), make(&mut w, &[KINDOF_AIRCRAFT], None));
        let mut t = TunnelTracker::new();
        assert!(t.is_valid_container_for(&w, Some(a), true));
        assert!(!t.is_valid_container_for(&w, Some(b), false), "aircraft never");
        assert!(!t.is_valid_container_for(&w, None, false));
        t.add_to_contain_list(a);
        t.add_to_contain_list(a);
        assert_eq!((t.get_contain_count(), t.is_in_container(a)), (2, true));
        assert!(!t.is_valid_container_for(&w, Some(a), true), "full");
        t.remove_from_contain(a);
        assert_eq!(t.get_contained_items_list(), &[a]);
        assert_eq!(t.get_contain_list_size(), 1);
    }

    #[test]
    fn the_last_tunnel_destroyed_destroys_everyone_inside() {
        let mut w = world();
        let (tunnel, a) = (make(&mut w, &[], None), make(&mut w, &[KINDOF_INFANTRY], Some(10.0)));
        let mut t = TunnelTracker::new();
        t.on_tunnel_created(tunnel);
        t.add_to_contain_list(a);
        w.on_contained_by(a, tunnel);
        assert_eq!(t.get_container_list(), &[tunnel]);
        assert!(t.on_tunnel_destroyed(&mut w, tunnel).is_ok());
        assert!(w.objects.find(a).unwrap().is_destroyed());
        assert_eq!((t.get_contain_count(), t.friend_get_tunnel_count()), (0, 0));
        let trace = w.cuts.trace.clone().unwrap();
        assert!(trace.contains(&format!("tt onRemovedFrom {a} {tunnel}")) && trace.contains(&format!("tt destroy {a}")));
    }

    #[test]
    fn another_tunnel_left_takes_over_the_containers_of_the_dead_one() {
        let mut w = world();
        let (t1, t2, a) = (make(&mut w, &[], None), make(&mut w, &[], None), make(&mut w, &[KINDOF_INFANTRY], None));
        let mut t = TunnelTracker::new();
        t.on_tunnel_created(t1);
        t.on_tunnel_created(t2);
        t.add_to_contain_list(a);
        w.on_contained_by(a, t1);
        assert!(t.on_tunnel_destroyed(&mut w, t1).is_ok());
        assert_eq!(w.objects.find(a).unwrap().contained_by, t2);
        assert!(!w.objects.find(a).unwrap().is_destroyed());
        // undefined upstream: nothing left to point at
        let mut empty = TunnelTracker::new();
        empty.on_tunnel_created(t1);
        empty.on_tunnel_created(t2);
        empty.tunnel_ids.clear();
        assert_eq!(empty.on_tunnel_destroyed(&mut w, t1), Err(TunnelError::NoTunnelIds));
    }

    #[test]
    fn healing_gives_a_sliver_until_the_time_is_up_then_everything() {
        let mut w = world();
        let a = make(&mut w, &[KINDOF_INFANTRY], Some(10.0));
        let mut t = TunnelTracker::new();
        t.add_to_contain_list(a);
        w.set_frame(5);
        w.on_contained_by(a, a); // entered at frame 5
        w.set_frame(10);
        assert!(t.heal_objects(&mut w, 100.0).is_ok());
        assert_eq!(health(&mut w, a), 11.0, "a hundredth of the maximum (100) after 5 of 100 frames");
        w.set_frame(105);
        assert!(t.heal_objects(&mut w, 100.0).is_ok());
        assert_eq!(health(&mut w, a), 100.0);
        let nobody = make(&mut w, &[KINDOF_INFANTRY], None);
        assert_eq!(TunnelTracker::heal_object(&mut w, nobody, 1.0), Err(TunnelError::NoBodyModule(nobody)));
        assert_eq!(TunnelTracker::heal_object(&mut w, 999, 1.0), Err(TunnelError::ContainedObjectGone(999)));
        assert_eq!(TunnelTracker::destroy_object(&mut w, 999), Err(TunnelError::ContainedObjectGone(999)));
    }

    #[test]
    fn the_nemesis_is_kept_for_four_seconds_and_forgotten_when_hidden_or_dead() {
        let mut w = world();
        let (a, b) = (make(&mut w, &[KINDOF_INFANTRY], None), make(&mut w, &[KINDOF_SELECTABLE], None));
        let mut t = TunnelTracker::new();
        t.update_nemesis(&mut w, Some(b));
        assert_eq!(t.get_cur_nemesis(&w), None, "not a vehicle, structure, infantry or aircraft");
        t.update_nemesis(&mut w, Some(a));
        assert_eq!(t.get_cur_nemesis(&w), Some(a));
        assert_eq!(t.get_nemesis_state(), (a, 0));
        w.set_frame(120);
        assert_eq!(t.get_cur_nemesis(&w), Some(a), "120 frames is still inside the window");
        t.update_nemesis(&mut w, Some(a));
        assert_eq!(t.get_nemesis_state().1, 120, "seeing it again renews it");
        w.set_frame(241);
        assert_eq!(t.get_cur_nemesis(&w), None);
        w.set_frame(0);
        t.update_nemesis(&mut w, Some(a));
        w.objects.find_mut(a).unwrap().status.set(STEALTHED_BIT, 1);
        assert_eq!(t.get_cur_nemesis(&w), None, "stealthed and undetected");
        w.objects.find_mut(a).unwrap().status.set(OBJECT_STATUS_STEALTHED, 0);
        t.update_nemesis(&mut w, Some(a));
        w.objects.find_mut(a).unwrap().private_status |= crate::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
        assert_eq!(t.get_cur_nemesis(&w), None, "effectively dead");
    }

    #[test]
    fn save_and_load_round_trip_and_post_process_hides_and_unregisters() {
        let mut w = world();
        let a = make(&mut w, &[KINDOF_INFANTRY], None);
        w.objects.find_mut(a).unwrap().drawable = a;
        w.objects.find_mut(a).unwrap().group = 7;
        let mut t = TunnelTracker::new();
        t.on_tunnel_created(a);
        t.add_to_contain_list(a);
        let mut save = XferSave::new();
        save.open(b"t").unwrap();
        t.xfer(&mut save).unwrap();
        save.close().unwrap();
        let mut load = XferLoad::new(save.take_bytes());
        load.open(b"t").unwrap();
        let mut back = TunnelTracker::new();
        back.xfer(&mut load).unwrap();
        assert_eq!(back.get_xfer_contain_list(), &[a]);
        assert_eq!(back.get_contain_count(), 1);
        assert!(back.get_contained_items_list().is_empty(), "until the post process");
        w.cuts.trace.as_mut().unwrap().clear();
        back.load_post_process_world(&mut w).unwrap();
        assert_eq!(back.get_contained_items_list(), &[a]);
        assert!(back.get_xfer_contain_list().is_empty());
        assert_eq!(w.objects.find(a).unwrap().group, 0);
        let trace = w.cuts.trace.clone().unwrap().join("\n");
        assert!(trace.contains("PartitionUnregisterObject") && trace.contains("PathfinderRemoveObjectFromPathfindMap") && trace.contains("DrawableSetHidden"));
        // a second post process finds the list full; an unknown id is invalid data
        assert_eq!(back.load_post_process_world(&mut w), Err(TunnelError::InvalidData));
        back.xfer_contain_list = vec![9999];
        back.contain_list.clear();
        assert_eq!(back.load_post_process_world(&mut w), Err(TunnelError::InvalidData));
        // crc writes nothing; the load hook is the world one
        let mut crc = crate::common::xfer_crc::XferCrc::new();
        back.crc(&mut crc).unwrap();
        back.load_post_process();
    }
}
