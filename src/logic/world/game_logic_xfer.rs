// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/GameLogic.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp
//   (`xfer`, `xferObjectTOC`, `addTOCEntry`, `findTOCEntryByName`, `findTOCEntryById`, `prepareLogicForObjectLoad`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `GameLogic`'s save block (`CHUNK_GameLogic`): the frame, the objects with their table of contents, the campaign,
//! the caves, the polygon triggers, the build assistant's sell list, and the members scripts set (layer 12 batch L2).
//!
//! # Shape
//!
//! [`xfer_game_logic`] is `GameLogic::xfer` (version 10) over [`GameLogicState`] (the members it saves) and
//! [`GameLogicSaveHost`]: the objects, templates, bridges, polygon triggers, the campaign manager, the cave system,
//! the build assistant and the control bar are the world's, each snapshot xferred by the host. `crc` is empty
//! upstream, and `loadPostProcess` is the world's ([`crate::logic::world::World::load_post_process`]).
//!
//! # The override tables' order (a deliberate difference)
//!
//! Upstream keeps the buildable and control-bar overrides in `hash_map`s and saves them in the order STLport's
//! `hash_map` iterates them, which depends on STLport's string hash and bucket sizes. STLport is not in the
//! repository, and this translation keeps no hash order (see `common::stl_typedefs`): the tables are saved in key
//! order. A save with two or more entries in a table is therefore not byte for byte the shipped game's; loading
//! either order gives the same tables.
//!
//! # Upstream facts kept
//!
//! - The table of contents is rebuilt on a save (each template's name once, in object order, numbered from 1 as an
//!   `UnsignedShort`), and on a load is read as it is (names and numbers may repeat; a lookup takes the first).
//! - A load first destroys every bridge (with its four towers) and every object that walks on top of walls, since
//!   the save brings them back: the bridge found at the object's position is dereferenced unchecked.
//! - An object whose template is unknown is skipped by its block's size; an unknown table-of-contents number is
//!   `SC_INVALID_DATA`.
//! - The neutral player is dereferenced before the objects are read, even when there are none.
//! - A polygon-trigger count that differs from the map's is `SC_INVALID_DATA`, as is an id the map does not have;
//!   a load then rebuilds the pathfinder's map.
//! - A load into a table that is not empty is `SC_INVALID_DATA`; a control-bar override names its button, and a name
//!   the control bar does not know loads as a NULL button.
//! - Before version 10, a load sets the superweapon restriction to 0.
//! - The members are xferred in place, so a load that runs out of bytes in one leaves the bytes it found there
//!   (`fread`'s partial read; the frame and the rank limit go through the host and are stored before the error).

use crate::common::game_state::SaveError;
use crate::common::ini::{IniError, Undefined};
use crate::common::xfer::{Xfer, XferMode, XferVersion};
use crate::logic::modules::plant;
use crate::logic::world::game_logic_update::GameLogicState;

type SaveResult<T> = Result<T, SaveError>;

/// `BRIDGE_MAX_TOWERS`.
pub const BRIDGE_MAX_TOWERS: usize = 4;

/// One entry of the object table of contents (`ObjectTOCEntry`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TocEntry {
    pub name: Vec<u8>,
    pub id: u16,
}

/// The world as `GameLogic::xfer` reaches it. Objects are named by id, buttons by the world's handles.
pub trait GameLogicSaveHost {
    /// `m_frame`.
    fn x_frame(&self) -> u32;
    /// The objects in `m_objList`'s order.
    fn x_objects(&self) -> Vec<u32>;
    /// `processDestroyList()`.
    fn x_process_destroy_list(&mut self);
    /// `m_frame = frame` (a load).
    fn x_set_frame(&mut self, frame: u32);
    /// `obj->getTemplate()->getName()`.
    fn x_object_template_name(&self, id: u32) -> Vec<u8>;
    /// `obj->isKindOf(KINDOF_BRIDGE)`.
    fn x_object_is_bridge(&self, id: u32) -> bool;
    /// `obj->isKindOf(KINDOF_WALK_ON_TOP_OF_WALL)`.
    fn x_object_walks_on_walls(&self, id: u32) -> bool;
    /// `TheTerrainLogic->findBridgeAt(obj->getPosition())`'s `peekBridgeInfo()`: the bridge object's id and its
    /// towers' ids; `None` when there is no bridge there.
    fn x_bridge_at_object(&self, id: u32) -> Option<(u32, [u32; BRIDGE_MAX_TOWERS])>;
    /// `findObjectByID(id) != NULL`.
    fn x_object_exists(&self, id: u32) -> bool;
    /// `destroyObject(obj)`.
    fn x_destroy_object(&mut self, id: u32);
    /// `xfer->xferSnapshot(obj)`.
    fn x_xfer_object(&mut self, id: u32, xfer: &mut dyn Xfer) -> SaveResult<()>;
    /// `ThePlayerList->getNeutralPlayer() != NULL`.
    fn x_has_neutral_player(&self) -> bool;
    /// `TheThingFactory->findTemplate(name) != NULL`.
    fn x_template_exists(&self, name: &[u8]) -> bool;
    /// `TheThingFactory->newObject(template, the neutral player's default team)`: the new object's id.
    fn x_new_object(&mut self, template_name: &[u8]) -> SaveResult<u32>;
    /// `TheAI->pathfinder()->addWallPiece(obj)`.
    fn x_add_wall_piece(&mut self, id: u32);
    /// `TheAI->pathfinder()->newMap()`.
    fn x_pathfinder_new_map(&mut self);
    /// `xfer->xferSnapshot(TheCampaignManager)`.
    fn x_xfer_campaign_manager(&mut self, xfer: &mut dyn Xfer) -> SaveResult<()>;
    /// `xfer->xferSnapshot(TheCaveSystem)`.
    fn x_xfer_cave_system(&mut self, xfer: &mut dyn Xfer) -> SaveResult<()>;
    /// The polygon triggers' ids, in `PolygonTrigger::getFirstPolygonTrigger`'s order.
    fn x_polygon_triggers(&self) -> Vec<i32>;
    /// `xfer->xferSnapshot(poly)` of the trigger at that place in the list (ids may repeat).
    fn x_xfer_polygon_trigger(&mut self, at: usize, xfer: &mut dyn Xfer) -> SaveResult<()>;
    /// `m_rankLevelLimit`.
    fn x_rank_level_limit(&self) -> i32;
    /// `m_rankLevelLimit = limit` (as the xfer writes it: no clamp).
    fn x_set_rank_level_limit(&mut self, limit: i32);
    /// `TheBuildAssistant->xferTheSellList(xfer)`.
    fn x_xfer_sell_list(&mut self, xfer: &mut dyn Xfer) -> SaveResult<()>;
    /// `TheControlBar->findCommandButton(name)`.
    fn x_find_command_button(&self, name: &[u8]) -> Option<u32>;
    /// The button's `getName()`.
    fn x_command_button_name(&self, button: u32) -> Vec<u8>;
}

/// `findTOCEntryByName(name)`: the first entry of that name.
pub fn find_toc_by_name<'a>(toc: &'a [TocEntry], name: &[u8]) -> Option<&'a TocEntry> {
    toc.iter().find(|e| e.name == name)
}

/// `findTOCEntryById(id)`: the first entry of that number.
pub fn find_toc_by_id(toc: &[TocEntry], id: u16) -> Option<&TocEntry> {
    toc.iter().find(|e| e.id == id)
}

/// `xferObjectTOC(xfer)` (version 1): saving, the table rebuilt from the objects and written; loading, read.
pub fn xfer_object_toc(state: &mut GameLogicState, xfer: &mut dyn Xfer, h: &mut impl GameLogicSaveHost) -> SaveResult<()> {
    let current: XferVersion = 1;
    let mut version = current;
    xfer.xfer_version(&mut version, current)?;
    state.object_toc.clear();
    let mut count: u32 = 0;
    if xfer.get_xfer_mode() == XferMode::Save {
        for id in h.x_objects() {
            let name = h.x_object_template_name(id);
            if find_toc_by_name(&state.object_toc, &name).is_some() && !plant(4011) {
                continue;
            }
            count = count.wrapping_add(1);
            state.object_toc.push(TocEntry { name, id: count as u16 });
        }
        xfer.xfer_unsigned_int(&mut count)?;
        for e in state.object_toc.iter_mut() {
            xfer.xfer_ascii_string(&mut e.name)?;
            xfer.xfer_unsigned_short(&mut e.id)?;
        }
    } else {
        xfer.xfer_unsigned_int(&mut count)?;
        for _ in 0..count {
            let mut name = Vec::new();
            xfer.xfer_ascii_string(&mut name)?;
            let mut id = 0u16;
            xfer.xfer_unsigned_short(&mut id)?;
            state.object_toc.push(TocEntry { name, id });
        }
    }
    Ok(())
}

/// `prepareLogicForObjectLoad()`: each bridge destroyed with its towers, each wall walker destroyed, then the
/// destroy list processed.
pub fn prepare_logic_for_object_load(h: &mut impl GameLogicSaveHost) -> SaveResult<()> {
    for id in h.x_objects() {
        if h.x_object_is_bridge(id) {
            let (bridge_object, towers) = h.x_bridge_at_object(id).ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
            for t in towers {
                if h.x_object_exists(t) {
                    h.x_destroy_object(t);
                }
            }
            // findObjectByID(bridgeObjectID), destroyed even when NULL (destroyObject(NULL) does nothing)
            if h.x_object_exists(bridge_object) && !plant(4012) {
                h.x_destroy_object(bridge_object);
            }
        } else if h.x_object_walks_on_walls(id) {
            h.x_destroy_object(id);
        }
    }
    h.x_process_destroy_list();
    Ok(())
}

/// `GameLogic::xfer` (version 10).
pub fn xfer_game_logic(state: &mut GameLogicState, xfer: &mut dyn Xfer, h: &mut impl GameLogicSaveHost) -> SaveResult<()> {
    let current: XferVersion = 10;
    let mut version = current;
    xfer.xfer_version(&mut version, current)?;
    // upstream xfers into m_frame itself: a short read leaves the bytes it found there
    let mut frame = h.x_frame();
    let r = xfer.xfer_unsigned_int(&mut frame);
    h.x_set_frame(frame);
    r?;

    xfer_object_toc(state, xfer, h)?;
    let loading = xfer.get_xfer_mode() == XferMode::Load;
    if loading {
        prepare_logic_for_object_load(h)?;
    }
    let mut object_count = h.x_objects().len() as u32;
    xfer.xfer_unsigned_int(&mut object_count)?;

    if xfer.get_xfer_mode() == XferMode::Save {
        for id in h.x_objects() {
            let name = h.x_object_template_name(id);
            let Some(entry) = find_toc_by_name(&state.object_toc, &name) else {
                return Err(SaveError::InvalidData);
            };
            let mut toc_id = entry.id;
            xfer.xfer_unsigned_short(&mut toc_id)?;
            xfer.begin_block()?;
            h.x_xfer_object(id, xfer)?;
            xfer.end_block()?;
        }
    } else {
        // ThePlayerList->getNeutralPlayer()->getDefaultTeam(), before the loop
        if !h.x_has_neutral_player() {
            return Err(IniError::undefined(Undefined::NullObject).into());
        }
        for _ in 0..object_count {
            let mut toc_id = 0u16;
            xfer.xfer_unsigned_short(&mut toc_id)?;
            let Some(entry) = find_toc_by_id(&state.object_toc, toc_id) else {
                return Err(SaveError::InvalidData);
            };
            let name = entry.name.clone();
            let size = xfer.begin_block()?;
            if !h.x_template_exists(&name) {
                xfer.skip(size)?;
                continue;
            }
            let id = h.x_new_object(&name)?;
            h.x_xfer_object(id, xfer)?;
            xfer.end_block()?;
            if h.x_object_walks_on_walls(id) && !plant(4013) {
                h.x_add_wall_piece(id);
            }
        }
    }

    h.x_xfer_campaign_manager(xfer)?;
    h.x_xfer_cave_system(xfer)?;
    if version >= 2 {
        xfer.xfer_bool(&mut state.scoring_enabled)?;
    }
    if version >= 3 {
        let triggers = h.x_polygon_triggers();
        let sanity = triggers.len() as u32;
        let mut count = sanity;
        xfer.xfer_unsigned_int(&mut count)?;
        if sanity != count {
            return Err(SaveError::InvalidData);
        }
        if xfer.get_xfer_mode() == XferMode::Save {
            for (at, id) in triggers.into_iter().enumerate() {
                let mut tid = id;
                xfer.xfer_int(&mut tid)?;
                h.x_xfer_polygon_trigger(at, xfer)?;
            }
        } else {
            for _ in 0..count {
                let mut tid = 0;
                xfer.xfer_int(&mut tid)?;
                // getPolygonTriggerByID: the first of the id
                let Some(at) = h.x_polygon_triggers().iter().position(|&p| p == tid) else {
                    return Err(SaveError::InvalidData);
                };
                h.x_xfer_polygon_trigger(at, xfer)?;
            }
            h.x_pathfinder_new_map();
        }
    }
    if version >= 5 {
        let mut limit = h.x_rank_level_limit();
        let r = xfer.xfer_int(&mut limit);
        h.x_set_rank_level_limit(limit);
        r?;
    }
    if version >= 6 {
        h.x_xfer_sell_list(xfer)?;
    }
    if version >= 7 {
        match xfer.get_xfer_mode() {
            XferMode::Save => {
                for (name, status) in state.buildable_overrides.clone() {
                    let mut name = name;
                    xfer.xfer_ascii_string(&mut name)?;
                    let mut bs = status.to_le_bytes();
                    xfer.xfer_user(&mut bs)?;
                }
                let mut empty = Vec::new();
                xfer.xfer_ascii_string(&mut empty)?;
            }
            XferMode::Load => {
                if !state.buildable_overrides.is_empty() && !plant(4014) {
                    return Err(SaveError::InvalidData);
                }
                loop {
                    let mut name = Vec::new();
                    xfer.xfer_ascii_string(&mut name)?;
                    if name.is_empty() {
                        break;
                    }
                    let mut bs = [0u8; 4];
                    xfer.xfer_user(&mut bs)?;
                    state.buildable_overrides.insert(name, i32::from_le_bytes(bs));
                }
            }
            _ => {}
        }
    }
    if version >= 8 {
        xfer.xfer_bool(&mut state.show_behind_building_markers)?;
        xfer.xfer_bool(&mut state.draw_icon_ui)?;
        xfer.xfer_bool(&mut state.show_dynamic_lod)?;
        xfer.xfer_int(&mut state.script_hulk_max_lifetime_override)?;
        match xfer.get_xfer_mode() {
            XferMode::Save => {
                for (name, button) in state.control_bar_overrides.clone() {
                    let mut name = name;
                    let mut value = button.map(|b| h.x_command_button_name(b)).unwrap_or_default();
                    xfer.xfer_ascii_string(&mut name)?;
                    xfer.xfer_ascii_string(&mut value)?;
                }
                let mut empty = Vec::new();
                xfer.xfer_ascii_string(&mut empty)?;
            }
            XferMode::Load => {
                if !state.control_bar_overrides.is_empty() {
                    return Err(SaveError::InvalidData);
                }
                loop {
                    let mut name = Vec::new();
                    xfer.xfer_ascii_string(&mut name)?;
                    if name.is_empty() {
                        break;
                    }
                    let mut value = Vec::new();
                    xfer.xfer_ascii_string(&mut value)?;
                    let button = if value.is_empty() { None } else { h.x_find_command_button(&value) };
                    state.control_bar_overrides.insert(name, button);
                }
            }
            _ => {}
        }
    }
    if version >= 9 {
        xfer.xfer_int(&mut state.rank_points_to_add_at_game_start)?;
    }
    if version >= 10 && !plant(4015) {
        xfer.xfer_unsigned_short(&mut state.superweapon_restriction)?;
    } else if xfer.get_xfer_mode() == XferMode::Load {
        state.superweapon_restriction = 0;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toc_lookups_take_the_first() {
        let toc = vec![TocEntry { name: b"A".to_vec(), id: 1 }, TocEntry { name: b"A".to_vec(), id: 2 }, TocEntry { name: b"B".to_vec(), id: 1 }];
        assert_eq!(find_toc_by_name(&toc, b"A").map(|e| e.id), Some(1));
        assert_eq!(find_toc_by_id(&toc, 1).map(|e| e.name.clone()), Some(b"A".to_vec()));
        assert!(find_toc_by_id(&toc, 3).is_none());
    }
}
