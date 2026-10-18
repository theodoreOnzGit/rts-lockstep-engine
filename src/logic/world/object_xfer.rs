// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp
//   (`crc`, `xfer`, `loadPostProcess`, `setID`)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `Object::crc`, `Object::xfer` (version 9) and `Object::loadPostProcess`.
//!
//! # What is cut
//!
//! - **The weapons** (`xferSnapshot(thisWeapon)` in `crc`, and the slots of
//!   `WeaponSet::xfer`): `Weapon` is not translated, so the weapon set is
//!   [`WeaponSetRecord`](crate::logic::world::object::WeaponSetRecord) with
//!   every slot empty.
//! - **The drawable's id** is written from `Object::drawable`; on a load the
//!   client is told the id the drawable has to take (`DrawableSetId`).
//! - **`xferUpgradeMask`** needs the upgrade centre and the name keys:
//!   [`ObjectXferStores`] carries them (the `World` has neither).
//! - **A module's own state** is its body's `Snapshot` (layer 9); the base
//!   chain is `ModuleSlot::module`, the scheduling half
//!   `BehaviorSlot::update`.
//!
//! # Errors
//!
//! `throw SC_INVALID_DATA` is [`ObjectXferError::InvalidData`] (an unknown
//! team on a load, or a trigger count outside 0..=5).

use crate::common::game_common::MAX_PLAYER_COUNT;
use crate::common::game_type::{DrawableId, ObjectId, INVALID_ID, NO_FORMATION_ID};
use crate::common::module::Module;
use crate::common::name_key_generator::NameKeyGenerator;
use crate::common::upgrade::{xfer_upgrade_mask, UpgradeCenter, XferUpgradeMaskError};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::module::update_module::UpdateModule;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::update_body::UpdateBody;

/// The current version of `Object::xfer`.
pub const OBJECT_XFER_VERSION: XferVersion = 9;

/// Why an object's `xfer` failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjectXferError {
    /// The `Xfer` failed.
    Xfer(XferStatus),
    /// `throw SC_INVALID_DATA`.
    InvalidData(&'static str),
    /// `xferUpgradeMask` failed.
    UpgradeMask(XferUpgradeMaskError),
    /// `setID` found the id taken by another object.
    IdTaken(ObjectId),
}

impl From<XferStatus> for ObjectXferError {
    fn from(s: XferStatus) -> Self {
        ObjectXferError::Xfer(s)
    }
}

impl From<XferUpgradeMaskError> for ObjectXferError {
    fn from(e: XferUpgradeMaskError) -> Self {
        ObjectXferError::UpgradeMask(e)
    }
}

/// The global stores `Object::xfer` needs and the world does not own:
/// `TheUpgradeCenter` and `TheNameKeyGenerator`.
#[derive(Debug)]
pub struct ObjectXferStores<'a> {
    /// `TheUpgradeCenter`.
    pub upgrade_center: &'a UpgradeCenter,
    /// `TheNameKeyGenerator`.
    pub names: &'a mut NameKeyGenerator,
}

/// A module as `xferSnapshot(module)` sees it: the scheduling half, the
/// base-class state and the behaviour, checked out of the object for the
/// duration of the call.
struct SlotSnapshot {
    update: Option<UpdateModule>,
    module: Option<Module>,
    body: Option<UpdateBody>,
}

impl Snapshot for SlotSnapshot {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        if let Some(u) = &mut self.update {
            u.crc(xfer)?;
        }
        if let Some(m) = &mut self.module {
            m.crc(xfer)?;
        }
        if let Some(b) = &mut self.body {
            b.crc(xfer)?;
        }
        Ok(())
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        if let Some(u) = &mut self.update {
            u.xfer(xfer)?;
        }
        if let Some(m) = &mut self.module {
            m.xfer(xfer)?;
        }
        if let Some(b) = &mut self.body {
            b.xfer(xfer)?;
        }
        Ok(())
    }
    fn load_post_process(&mut self) {
        if let Some(u) = &mut self.update {
            u.load_post_process();
        }
        if let Some(m) = &mut self.module {
            m.load_post_process();
        }
        if let Some(b) = &mut self.body {
            b.load_post_process();
        }
    }
}

fn user_i32(xfer: &mut dyn Xfer, v: &mut i32) -> XferResult<()> {
    let mut b = v.to_le_bytes();
    xfer.xfer_user(&mut b)?;
    *v = i32::from_le_bytes(b);
    Ok(())
}

fn user_u32(xfer: &mut dyn Xfer, v: &mut u32) -> XferResult<()> {
    let mut b = v.to_le_bytes();
    xfer.xfer_user(&mut b)?;
    *v = u32::from_le_bytes(b);
    Ok(())
}

impl World {
    /// `Object::crc(xfer)`: what the logic CRC of a frame takes of one
    /// object: the private status, the transform (48 bytes), the id, the
    /// first 64 bits of the completed upgrades, the experience, the health,
    /// the weapon bonus conditions, the damage scalar and the weapons.
    pub fn object_crc(&mut self, id: ObjectId, xfer: &mut dyn Xfer) -> XferResult<()> {
        let health;
        let scalar;
        {
            let Some(_) = self.objects.find(id) else { return Ok(()) };
        }
        match self.get_body_module(id) {
            Some(b) => {
                health = self.module_call(id, b, ModuleCall::GetHealth).as_real(0.0);
                scalar = self.module_call(id, b, ModuleCall::GetDamageScalar).as_real(0.0);
            }
            None => {
                health = 0.0;
                scalar = 0.0;
            }
        }
        let o = self.objects.find_mut(id).expect("checked above");
        xfer.xfer_unsigned_byte(&mut o.private_status)?;
        let mut mtx = [0u8; 48];
        for (i, f) in o.thing.transform_matrix().to_array().iter().enumerate() {
            mtx[i * 4..i * 4 + 4].copy_from_slice(&f.to_le_bytes());
        }
        xfer.xfer_user(&mut mtx)?;
        let mut oid = o.id.to_le_bytes();
        xfer.xfer_user(&mut oid)?;
        let mut upgrades = o.object_upgrades_completed.words()[0].to_le_bytes();
        xfer.xfer_user(&mut upgrades)?;
        xfer.xfer_snapshot(&mut o.experience)?;
        xfer.xfer_user(&mut health.to_le_bytes())?;
        xfer.xfer_unsigned_int(&mut o.weapon_bonus_condition)?;
        xfer.xfer_user(&mut scalar.to_le_bytes())?;
        // for each weapon slot with a weapon: xferSnapshot(weapon) (cut: no weapon is ever present)
        Ok(())
    }

    /// `Object::xfer(xfer)`, version 9. See the module docs.
    pub fn object_xfer(&mut self, id: ObjectId, xfer: &mut dyn Xfer, stores: &mut ObjectXferStores<'_>) -> Result<(), ObjectXferError> {
        if self.objects.find(id).is_none() {
            return Ok(());
        }
        // version
        let current = OBJECT_XFER_VERSION;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;

        // object ID
        let mut oid = id;
        xfer.xfer_object_id(&mut oid)?;
        let id = self.set_object_id(id, oid)?;

        if version >= 7 {
            let mut mtx = *self.objects.find(id).expect("set_object_id kept it").thing.transform_matrix();
            xfer.xfer_matrix3d(&mut mtx)?;
            self.set_transform_matrix(id, &mtx);
        } else {
            // object position
            let o = self.objects.find(id).expect("kept");
            let mut pos = *o.thing.position();
            let mut orientation = o.thing.orientation();
            xfer.xfer_coord3d(&mut pos)?;
            self.set_position(id, &pos);
            // orientation
            xfer.xfer_real(&mut orientation)?;
            self.set_orientation(id, orientation);
        }

        // team
        let mut team_id = self.objects.find(id).expect("kept").team;
        user_u32(xfer, &mut team_id)?;
        // DON'T set the team yet; must wait till we read our status bits, since setTeam can affect the
        // player's power usage, but that could be done incorrectly if our status bits aren't accurate yet... (srj)

        let o = self.objects.find_mut(id).expect("kept");
        // producer id
        xfer.xfer_object_id(&mut o.producer_id)?;
        // builder id
        xfer.xfer_object_id(&mut o.builder_id)?;

        // drawable id
        let mut drawable_id: DrawableId = o.drawable;
        xfer.xfer_drawable_id(&mut drawable_id)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            // change the ID of the drawable attached to be the same ID as it was when it was saved
            o.drawable = drawable_id;
            self.emit(ObjectClientEvent::DrawableSetId { object_id: id, drawable_id });
        }

        let o = self.objects.find_mut(id).expect("kept");
        // internal name
        xfer.xfer_ascii_string(&mut o.name)?;

        // status
        if version >= 8 {
            o.status.xfer(xfer)?;
        } else {
            // We are loading an old version, so we must convert it from a 32-bit int to a bitflag
            let mut old_status = 0u32;
            xfer.xfer_unsigned_int(&mut old_status)?;
            // Clear our status
            o.status.clear();
            for i in 0..32 {
                let bit = 1u32 << i;
                if old_status & bit != 0 {
                    o.status.set(i + 1, 1);
                }
            }
        }

        // script status
        xfer.xfer_unsigned_byte(&mut o.script_status)?;
        // private status
        xfer.xfer_unsigned_byte(&mut o.private_status)?;

        // OK, now that we have xferred our status bits, it's safe to set the team...
        if xfer.get_xfer_mode() == XferMode::Load {
            if self.teams.find_team_by_id(team_id).is_none() {
                return Err(ObjectXferError::InvalidData("Object::xfer - Unable to load team"));
            }
            self.set_or_restore_team(id, Some(team_id), true);
        }

        let o = self.objects.find_mut(id).expect("kept");
        // geometry info
        xfer.xfer_snapshot(&mut o.geometry_info)?;

        // sighting info, last look - must be saved cause we save PartitionCell::m_shroudLevel
        xfer.xfer_snapshot(&mut o.partition_last_look)?;
        if version >= 9 {
            xfer.xfer_snapshot(&mut o.partition_reveal_all_last_look)?;
        }
        // sighting info, last shroud - must be saved cause we save PartitionCell::m_shroudLevel
        xfer.xfer_snapshot(&mut o.partition_last_shroud)?;

        // vision spied by
        let mut spied = Vec::with_capacity(4 * MAX_PLAYER_COUNT as usize);
        for v in &o.vision_spied_by {
            spied.extend_from_slice(&v.to_le_bytes());
        }
        xfer.xfer_user(&mut spied)?;
        for (i, v) in o.vision_spied_by.iter_mut().enumerate() {
            *v = i32::from_le_bytes([spied[4 * i], spied[4 * i + 1], spied[4 * i + 2], spied[4 * i + 3]]);
        }
        // vision spied by mask
        let mut mask = o.vision_spied_mask.to_le_bytes();
        xfer.xfer_user(&mut mask)?;
        o.vision_spied_mask = u16::from_le_bytes(mask);

        // (the threat and value sightings are not saved: "John M says we don't need to save this")

        // vision range, shroud clearing range, shroud range
        xfer.xfer_real(&mut o.vision_range)?;
        xfer.xfer_real(&mut o.shroud_clearing_range)?;
        xfer.xfer_real(&mut o.shroud_range)?;

        // disabled mask
        o.disabled.xfer(xfer)?;

        // New var added for version 2. Determines if the single use command button has been used or not.
        if xfer.get_xfer_mode() == XferMode::Save || version >= 2 {
            xfer.xfer_bool(&mut o.single_use_command_used)?;
        } else {
            o.single_use_command_used = false;
        }

        // disabled till frame
        let mut till = Vec::with_capacity(4 * o.disabled_till_frame.len());
        for v in &o.disabled_till_frame {
            till.extend_from_slice(&v.to_le_bytes());
        }
        xfer.xfer_user(&mut till)?;
        for (i, v) in o.disabled_till_frame.iter_mut().enumerate() {
            *v = u32::from_le_bytes([till[4 * i], till[4 * i + 1], till[4 * i + 2], till[4 * i + 3]]);
        }

        // special model condition until
        xfer.xfer_unsigned_int(&mut o.smc_until)?;

        // radar data ... when loading, we will remove all objects from the radar and let the radar system
        // load itself as a separate chunk of data from the save file
        if xfer.get_xfer_mode() == XferMode::Load && o.on_radar {
            self.radar_remove_object(id);
        }
        let o = self.objects.find_mut(id).expect("kept");

        // experience tracker
        xfer.xfer_snapshot(&mut o.experience)?;

        // we do not need to do anything with our m_containedBy pointer, the post process of that objects
        // contain module will actually re-do the contain process again
        if version >= 6 {
            // No, the contain module is just going to friend_ reach in and set this for us. Containers more
            // complicated than Open (like Tunnel) can't do that. Our variable, our responsibility.
            if xfer.get_xfer_mode() == XferMode::Save {
                o.xfer_contained_by_id = o.contained_by;
            }
            xfer.xfer_object_id(&mut o.xfer_contained_by_id)?;
        }

        // contained by frame
        xfer.xfer_unsigned_int(&mut o.contained_by_frame)?;
        // construction percent
        xfer.xfer_real(&mut o.construction_percent)?;
        // upgrades completed
        xfer_upgrade_mask(xfer, &mut o.object_upgrades_completed, stores.upgrade_center, stores.names)?;
        // original team name
        xfer.xfer_ascii_string(&mut o.original_team_name)?;
        // indicator color
        xfer.xfer_color(&mut o.indicator_color)?;
        // health box offset
        xfer.xfer_coord3d(&mut o.health_box_offset)?;

        // Entered & exited housekeeping.
        xfer.xfer_byte(&mut o.num_trigger_areas_active)?;
        xfer.xfer_unsigned_int(&mut o.entered_or_exited_frame)?;
        xfer.xfer_icoord3d(&mut o.i_pos)?;
        if o.num_trigger_areas_active < 0 || o.num_trigger_areas_active as usize > crate::logic::world::object::MAX_TRIGGER_AREA_INFOS {
            return Err(ObjectXferError::InvalidData("Invalid m_numTriggerAreasActive"));
        }
        for i in 0..o.num_trigger_areas_active as usize {
            let o = self.objects.find(id).expect("kept");
            let mut trigger_name: Vec<u8> = match o.trigger_info[i].trigger {
                Some(t) => self.terrain.triggers().iter().find(|a| a.id() == t).map(|a| a.name().str().to_vec()).unwrap_or_default(),
                None => Vec::new(),
            };
            xfer.xfer_ascii_string(&mut trigger_name)?;
            let o = self.objects.find_mut(id).expect("kept");
            if xfer.get_xfer_mode() == XferMode::Load {
                o.trigger_info[i].trigger = self
                    .terrain
                    .trigger_area_by_name(&crate::common::ascii_string::AsciiString::try_from_bytes(&trigger_name).unwrap_or_default())
                    .map(|ix| self.terrain.triggers()[ix].id());
            }
            let o = self.objects.find_mut(id).expect("kept");
            let (mut entered, mut exited, mut inside) =
                (o.trigger_info[i].entered as i8, o.trigger_info[i].exited as i8, o.trigger_info[i].is_inside as i8);
            xfer.xfer_byte(&mut entered)?;
            xfer.xfer_byte(&mut exited)?;
            xfer.xfer_byte(&mut inside)?;
            o.trigger_info[i].entered = entered != 0;
            o.trigger_info[i].exited = exited != 0;
            o.trigger_info[i].is_inside = inside != 0;
        }
        let o = self.objects.find_mut(id).expect("kept");
        // Layer object is pathing on.
        user_i32(xfer, &mut o.layer)?;
        // Layer of current path goal.
        user_i32(xfer, &mut o.destination_layer)?;
        // Object selectability.
        xfer.xfer_bool(&mut o.is_selectable)?;
        xfer.xfer_unsigned_int(&mut o.safe_occlusion_frame)?;
        // User formations.
        user_i32(xfer, &mut o.formation_id)?;
        if o.formation_id != NO_FORMATION_ID {
            xfer.xfer_coord2d(&mut o.formation_offset)?;
        }

        // module count
        let mut module_count = o.modules.len() as u16;
        xfer.xfer_unsigned_short(&mut module_count)?;
        if xfer.get_xfer_mode() == XferMode::Save {
            // go through all modules
            for idx in 0..o.modules.len() {
                let mut snap = self.take_slot_snapshot(id, idx);
                let mut identifier = self.objects.find(id).expect("kept").modules[idx].tag.clone();
                // write module identifier
                xfer.xfer_ascii_string(&mut identifier)?;
                // begin a data block
                xfer.begin_block()?;
                // xfer data
                let r = xfer.xfer_snapshot(&mut snap);
                self.put_slot_snapshot(id, idx, snap);
                r?;
                // end a data block
                xfer.end_block()?;
            }
        } else {
            // read all module data
            for _ in 0..module_count {
                // read module name
                let mut identifier = Vec::new();
                xfer.xfer_ascii_string(&mut identifier)?;
                // find the module with this identifier in the module list
                let found = self.objects.find(id).expect("kept").modules.iter().position(|m| m.tag == identifier);
                // start of a new block
                let data_size = xfer.begin_block()?;
                // if we didn't find the module, it's quite possible that we have removed it from the object
                // definition in a future patch, if that is so, we need to skip the module data in the file
                match found {
                    None => xfer.skip(data_size)?,
                    Some(idx) => {
                        let mut snap = self.take_slot_snapshot(id, idx);
                        let r = xfer.xfer_snapshot(&mut snap);
                        self.put_slot_snapshot(id, idx, snap);
                        r?;
                    }
                }
                // end block
                xfer.end_block()?;
            }
        }

        let o = self.objects.find_mut(id).expect("kept");
        if version >= 3 {
            xfer.xfer_object_id(&mut o.sole_healing_benefactor_id)?;
            xfer.xfer_unsigned_int(&mut o.sole_healing_benefactor_expiration_frame)?;
        } else if xfer.get_xfer_mode() == XferMode::Load {
            o.sole_healing_benefactor_id = INVALID_ID;
            o.sole_healing_benefactor_expiration_frame = 0;
        }

        // (m_group, m_partitionData and the cached pointers are not saved)

        if version >= 4 {
            // xfer the weaponSetFlags FIRST, since we need 'em to restore the weaponSet properly. (srj)
            o.cur_weapon_set_flags.xfer(xfer)?;
            xfer.xfer_unsigned_int(&mut o.weapon_bonus_condition)?;
            let mut last: Vec<u8> = o.last_weapon_condition.iter().map(|&v| v as u8).collect();
            xfer.xfer_user(&mut last)?;
            for (i, v) in last.iter().enumerate() {
                o.last_weapon_condition[i] = *v as i8;
            }
            // do the weaponSet itself after all the weapon-related stuff, just in case
            xfer.xfer_snapshot(&mut o.weapon_set)?;
            o.special_power_bits.xfer(xfer)?;
            xfer.xfer_ascii_string(&mut o.command_set_string_override)?;
            xfer.xfer_bool(&mut o.modules_ready)?;
        }

        if version >= 5 {
            xfer.xfer_bool(&mut o.is_receiving_difficulty_bonus)?;
        } else {
            o.is_receiving_difficulty_bonus = false;
        }
        Ok(())
    }

    /// `Object::loadPostProcess()`: the container pointer is found again.
    pub fn object_load_post_process(&mut self, id: ObjectId) {
        let Some(o) = self.objects.find(id) else { return };
        let contained_by = if o.xfer_contained_by_id != INVALID_ID && self.objects.find(o.xfer_contained_by_id).is_some() {
            o.xfer_contained_by_id
        } else {
            INVALID_ID
        };
        if let Some(o) = self.objects.find_mut(id) {
            o.contained_by = contained_by;
        }
    }

    /// `setID(id)` for an object that already has one: the lookup table
    /// entry and the list links move to the new id. Returns the id the
    /// object has afterwards. The sleepy heap and the destroy list are
    /// renamed too (upstream's hold pointers).
    pub fn set_object_id(&mut self, old: ObjectId, new: ObjectId) -> Result<ObjectId, ObjectXferError> {
        if old == new {
            return Ok(old);
        }
        self.objects.rekey(old, new).map_err(|_| ObjectXferError::IdTaken(new))?;
        self.sched.rename_object(old, new);
        self.rename_in_destroy_list(old, new);
        Ok(new)
    }

    fn take_slot_snapshot(&mut self, id: ObjectId, idx: usize) -> SlotSnapshot {
        let o = self.objects.find_mut(id).expect("kept");
        let slot = &mut o.modules[idx];
        let module = slot.module.take();
        let (update, body) = match slot.update_index {
            Some(u) => {
                let b = &mut o.behaviors[u as usize];
                (Some(b.update.clone()), b.body.take())
            }
            None => (None, o.modules[idx].body.take()),
        };
        SlotSnapshot { update, module, body }
    }

    fn put_slot_snapshot(&mut self, id: ObjectId, idx: usize, snap: SlotSnapshot) {
        let o = self.objects.find_mut(id).expect("kept");
        o.modules[idx].module = snap.module;
        match o.modules[idx].update_index {
            Some(u) => {
                let b = &mut o.behaviors[u as usize];
                if let Some(upd) = snap.update {
                    // the scheduling half is the arena's; a load changes only what the base xfers read
                    b.update = upd;
                }
                b.body = snap.body;
            }
            None => o.modules[idx].body = snap.body,
        }
    }
}
