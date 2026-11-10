// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MapUtil.cpp (WouldMapTransfer, populateMapListboxNoReset,
//     populateMapListbox, isValidMap, getDefaultMap, getDefaultOfficialMap, isOfficialMap, getMapPreviewImage,
//     findDrawPositions)
//   GeneralsMD/Code/GameEngine/Include/GameClient/MapUtil.h (SUPPLY_TECH_SIZE)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! MapUtil.cpp's GUI half (GitHub #89): the map list boxes the skirmish and multiplayer screens fill from
//! `TheMapCache`, the default and valid maps, the map preview's image and where a map is drawn in a window. The cache
//! itself (`updateCache`, `addMap`, `findMap`) is [`crate::common::map_cache`].
//!
//! # As upstream reads it
//!
//! **`populateMapListboxNoReset`** lists the cache's maps by player count (0, 1, 2, ... until every map of the cache
//! has been counted), and within a count by display name without regard to case (a `std::set` with
//! `less_than_nocase`): a map is listed when its file name starts with the map directory asked for (the shipped `Maps`
//! or the user's), its multiplayer flag is the one asked for and it has a display name. With more than one column, the
//! first holds the skirmish medal's star (bronze, silver, gold, red-yellow: `SkirmishBattleHonors`' endurance medals,
//! for a multiplayer map) and its item data in column 1 is the star's level (0 to 4); the name is in the last column,
//! the file name the row's item data. The row of `mapToSelect` is selected (else row 0), and scrolled to when it is at
//! or below the bottom row shown. **`populateMapListbox`** empties the list first.
//!
//! # Upstream quirks that are kept
//!
//! - Two maps of one player count whose display names differ only in case are **one row**: the set keeps the first
//!   met (in the cache's file-name order), and the name-to-file map (exact names) gives that name's file; the other map
//!   is not listed (it is still counted, so the loop ends).
//! - `mapToSelect` is compared with the file name exactly (the cache's are lower case; a name in another case selects
//!   row 0).
//! - The two Generals maps patch 1.03 filters out (`maps\armored fury\armored fury.map`,
//!   `maps\scorched earth\scorched earth.map`, compared exactly) are never listed.
//! - With nothing listed, row 0 is still "selected" and the scroll's `getListboxBottomEntry` reads `listData[-1]`:
//!   recorded as undefined by the list box.
//! - `numMapsListed` counts every cache entry once, and `curNumPlayersInMap` is unsigned: a map with a negative player
//!   count is listed when the count wraps to its value (after about 4 billion empty passes).
//!
//! # Where this differs from upstream
//!
//! - The empty passes of the player-count loop (no map of that count) are skipped: they change nothing, so the result
//!   is the same, without the 4-billion-pass wait for a negative count.
//! - The item data is a `char *` into the map cache's key; here it is a number the host gives the name
//!   ([`MapUtilHost::map_list_item`], 0 for `NULL`), turned back by [`MapUtilHost::map_list_name`].
//! - `TheMapCache->updateCache()` is the host's ([`MapUtilHost::update_map_cache`]).
//! - `SkirmishBattleHonors` (the `SkirmishStats.ini` file) is the host's ([`MapUtilHost::endurance_medal`]).
//! - `getMapPreviewImage`: the image's name and the collection's lookup are translated; making a new image from the
//!   map's `.tga` (`doesFileExist`, `copyFromBigToDir` into the user's `MapPreviews` folder, `newInstance(Image)`) is
//!   the host's ([`MapUtilHost::map_preview_from_file`], recorded as untranslated by the shell's host).
//! - `findDrawPositions`: a float converted to `Int` out of range (an empty extent divides by zero) is undefined in C;
//!   Rust's `as` saturates (NaN to 0).
//!
//! # Representation
//!
//! Free functions over a [`MapUtilHost`]; the list box is the translated gadget
//! ([`crate::client::gadget_list_box`]).

use crate::client::gadget_list_box as lb;
use crate::client::window_manager::{GameWindowManager, GuiHost};
use crate::client::window_script::WinId;
use crate::common::base_type::{ICoord2D, Region3D};
use crate::common::game_info::slot_state;
use crate::common::ini::game_make_color;
use crate::common::map_cache::MapCache;
use crate::common::unicode_string::UnicodeString;
use crate::logic::modules::plant;
use std::cmp::Ordering;

/// `SUPPLY_TECH_SIZE` (MapUtil.h).
pub const SUPPLY_TECH_SIZE: i32 = 15;

/// What MapUtil.cpp's GUI functions reach besides the window manager.
pub trait MapUtilHost: GuiHost {
    /// `TheMapCache` (`None`: `NULL`).
    fn map_cache(&self) -> Option<&MapCache>;
    /// `TheMapCache->updateCache()`.
    fn update_map_cache(&mut self);
    /// `TheGlobalData->getPath_UserData()`.
    fn user_data_path(&self) -> Vec<u8>;
    /// `TheMappedImageCollection->findImageByName(name)`: the image's name, or `None`.
    fn find_mapped_image(&self, name: &[u8]) -> Option<Vec<u8>>;
    /// `SkirmishBattleHonors::getEnduranceMedal(map, slotState)` (0: none).
    fn endurance_medal(&self, _map: &[u8], _state: i32) -> i32 {
        0
    }
    /// The item data that stands for the `char *` of the map name `name` (never 0).
    fn map_list_item(&mut self, name: &[u8]) -> u32;
    /// The map name an item data of [`MapUtilHost::map_list_item`] stands for (`None`: `NULL`).
    fn map_list_name(&self, data: u32) -> Option<Vec<u8>>;
    /// `TheGameState->realMapPathToPortableMapPath(path)`.
    fn real_map_path_to_portable(&self, path: &[u8]) -> Vec<u8>;
    /// `getMapPreviewImage`'s new image when the collection has none: `doesFileExist(tga)`, `copyFromBigToDir` into
    /// `<user data>MapPreviews\<name>`, and the `Image` added to the collection as `temp_name`. The image's name, or
    /// `None` (no file, or the copy threw).
    fn map_preview_from_file(&mut self, _tga: &[u8], _name: &[u8], _temp_name: &[u8]) -> Option<Vec<u8>> {
        None
    }
}

/// `UnicodeString::compareNoCase` of two display names (`_wcsicmp`).
fn no_case(a: &[u16], b: &[u16]) -> Ordering {
    UnicodeString::try_from_chars(a).unwrap_or_default().compare_no_case(b)
}

/// `WouldMapTransfer(mapName)`: the name starts with the user's map directory, without regard to case.
pub fn would_map_transfer<H: MapUtilHost>(h: &H, map_name: &[u8]) -> bool {
    let Some(c) = h.map_cache() else { return false };
    let mut dir = h.user_data_path();
    dir.extend_from_slice(&c.get_map_dir());
    map_name.len() >= dir.len() && map_name[..dir.len()].eq_ignore_ascii_case(&dir)
}

/// What the player-count loop reads of a cache entry.
struct Listed {
    file: Vec<u8>,
    display: Vec<u16>,
    num_players: i32,
    is_multiplayer: bool,
}

/// `populateMapListboxNoReset(listbox, useSystemMaps, isMultiplayer, mapToSelect)`: the maps added to the list (see the
/// module docs); the row selected, or -1 without a cache or a list box.
pub fn populate_map_listbox_no_reset<H: MapUtilHost>(wm: &mut GameWindowManager, h: &mut H, listbox: Option<WinId>, use_system_maps: bool, is_multiplayer: bool, map_to_select: &[u8]) -> i32 {
    let Some(cache) = h.map_cache() else { return -1 };
    let Some(listbox) = listbox else { return -1 };
    let entries: Vec<Listed> = cache.maps.iter().map(|(k, m)| Listed { file: k.clone(), display: m.display_name.str().to_vec(), num_players: m.num_players, is_multiplayer: m.is_multiplayer }).collect();
    let mut map_dir = if use_system_maps {
        cache.get_map_dir()
    } else {
        let mut d = h.user_data_path();
        d.extend_from_slice(&cache.get_map_dir());
        d
    };
    map_dir.make_ascii_lowercase();
    let num_columns = lb::get_num_columns(wm, listbox);
    let (mut easy_image, mut medium_image, mut brutal_image, mut max_brutal_image) = (None, None, None, None);
    let (mut w, mut hgt) = (10, 10);
    let battle_honors = num_columns > 1;
    if battle_honors {
        easy_image = h.find_mapped_image(b"Star-Bronze");
        medium_image = h.find_mapped_image(b"Star-Silver");
        brutal_image = h.find_mapped_image(b"Star-Gold");
        max_brutal_image = h.find_mapped_image(b"RedYell_Star");
        w = brutal_image.as_ref().map_or(10, |i| h.image_size(i).map_or(0, |s| s.0));
        w = w.min(lb::get_column_width(wm, listbox, 0));
        hgt = w;
    }
    let color = game_make_color(255, 255, 255, 255);
    let mut selection_index = 0;
    let mut num_maps_listed: u32 = 0;
    let mut cur_num_players_in_map: u32 = 0;
    // (the passes that find no map of their count change nothing: jump to the next count a map has)
    while (num_maps_listed as usize) < entries.len() {
        // `std::set<UnicodeString, less_than_nocase>` (the first of equal names kept) and `std::map<UnicodeString,
        // AsciiString>` (exact names; the last file assigned kept)
        let mut temp_cache: Vec<Vec<u16>> = Vec::new();
        let mut filename_map: Vec<(Vec<u16>, Vec<u8>)> = Vec::new();
        for e in &entries {
            if e.num_players as u32 == cur_num_players_in_map {
                // (plant 8901, differential builds only: the names in exact order, not without regard to case)
                let order = |x: &Vec<u16>| if plant(8901) { x.cmp(&e.display) } else { no_case(x, &e.display) };
                if let Err(at) = temp_cache.binary_search_by(order) {
                    temp_cache.insert(at, e.display.clone());
                }
                match filename_map.binary_search_by(|(x, _)| x.cmp(&e.display)) {
                    Ok(i) => filename_map[i].1 = e.file.clone(),
                    Err(at) => filename_map.insert(at, (e.display.clone(), e.file.clone())),
                }
                num_maps_listed += 1;
            }
        }
        for name in &temp_cache {
            let ascii_map_name = filename_map.iter().find(|(x, _)| x == name).map(|(_, f)| f.clone()).unwrap_or_default();
            // "Patch 1.03 -- Purposely filter out these broken maps that exist in Generals."
            if ascii_map_name == b"maps\\armored fury\\armored fury.map" || ascii_map_name == b"maps\\scorched earth\\scorched earth.map" {
                continue;
            }
            let Some(it) = entries.iter().find(|e| e.file == ascii_map_name) else { continue };
            let starts = it.file.len() >= map_dir.len() && it.file[..map_dir.len()].eq_ignore_ascii_case(&map_dir);
            if starts && is_multiplayer == it.is_multiplayer && !it.display.is_empty() {
                let mut index = -1;
                let mut image_item_data: i32 = -1;
                if num_columns > 1 && it.is_multiplayer {
                    let num_easy = h.endurance_medal(&it.file, slot_state::EASY_AI);
                    let num_medium = h.endurance_medal(&it.file, slot_state::MED_AI);
                    let num_brutal = h.endurance_medal(&it.file, slot_state::BRUTAL_AI);
                    let (image, level) = if num_brutal != 0 {
                        let max_brutal_slots = it.num_players - 1;
                        // (plant 8903, differential builds only: the red-yellow star for every slot, not one less)
                        if num_brutal == if plant(8903) { it.num_players } else { max_brutal_slots } {
                            (max_brutal_image.clone(), 4)
                        } else {
                            (brutal_image.clone(), 3)
                        }
                    } else if num_medium != 0 {
                        (medium_image.clone(), 2)
                    } else if num_easy != 0 {
                        (easy_image.clone(), 1)
                    } else {
                        (None, 0)
                    };
                    image_item_data = level;
                    index = lb::add_entry_image(wm, h, listbox, image, index, 0, w, hgt, true, -1);
                }
                index = lb::add_entry_text(wm, h, listbox, &it.display, color, index, num_columns - 1, true);
                // (plant 8902, differential builds only: compared without regard to case)
                if it.file == map_to_select || (plant(8902) && it.file.eq_ignore_ascii_case(map_to_select)) {
                    selection_index = index;
                }
                let data = h.map_list_item(&it.file);
                lb::set_item_data(wm, h, listbox, data, index, 0);
                if num_columns > 1 {
                    lb::set_item_data(wm, h, listbox, image_item_data as u32, index, 1);
                }
            }
        }
        // the next count a map not yet listed has (`++curNumPlayersInMap` until it is reached)
        let next = entries.iter().map(|e| e.num_players as u32).filter(|&n| n > cur_num_players_in_map).min();
        match next {
            Some(n) => cur_num_players_in_map = n,
            None => break,
        }
    }
    lb::set_selected(wm, h, listbox, &[selection_index]);
    if selection_index >= 0 {
        let top_index = lb::get_top_visible_entry(wm, listbox);
        let bottom_index = lb::get_bottom_visible_entry(wm, listbox);
        let rows_on_screen = bottom_index - top_index;
        // (plant 8904, differential builds only: scrolled only below the bottom row)
        if if plant(8904) { selection_index > bottom_index } else { selection_index >= bottom_index } {
            let new_top = 0.max(selection_index - 1.max(rows_on_screen / 2));
            lb::set_top_visible_entry(wm, h, listbox, new_top);
        }
    }
    selection_index
}

/// `populateMapListbox(listbox, useSystemMaps, isMultiplayer, mapToSelect)`: the list emptied, then
/// [`populate_map_listbox_no_reset`].
pub fn populate_map_listbox<H: MapUtilHost>(wm: &mut GameWindowManager, h: &mut H, listbox: Option<WinId>, use_system_maps: bool, is_multiplayer: bool, map_to_select: &[u8]) -> i32 {
    if h.map_cache().is_none() {
        return -1;
    }
    let Some(l) = listbox else { return -1 };
    lb::reset(wm, h, l);
    populate_map_listbox_no_reset(wm, h, listbox, use_system_maps, is_multiplayer, map_to_select)
}

/// `isValidMap(mapName, isMultiplayer)`: in the cache (after `updateCache`, the name lower-cased) with that
/// multiplayer flag.
pub fn is_valid_map<H: MapUtilHost>(h: &mut H, map_name: &[u8], is_multiplayer: bool) -> bool {
    if h.map_cache().is_none() || map_name.is_empty() {
        return false;
    }
    h.update_map_cache();
    let lower = map_name.to_ascii_lowercase();
    h.map_cache().and_then(|c| c.maps.get(&lower)).is_some_and(|m| m.is_multiplayer == is_multiplayer)
}

/// `getDefaultMap(isMultiplayer)`: the first map of the cache (after `updateCache`) with that multiplayer flag, or
/// empty.
pub fn get_default_map<H: MapUtilHost>(h: &mut H, is_multiplayer: bool) -> Vec<u8> {
    if h.map_cache().is_none() {
        return Vec::new();
    }
    h.update_map_cache();
    h.map_cache().and_then(|c| c.maps.iter().find(|(_, m)| m.is_multiplayer == is_multiplayer).map(|(k, _)| k.clone())).unwrap_or_default()
}

/// `getDefaultOfficialMap()`: the first official multiplayer map of the cache (after `updateCache`), or empty.
pub fn get_default_official_map<H: MapUtilHost>(h: &mut H) -> Vec<u8> {
    if h.map_cache().is_none() {
        return Vec::new();
    }
    h.update_map_cache();
    h.map_cache().and_then(|c| c.maps.iter().find(|(_, m)| m.is_multiplayer && m.is_official).map(|(k, _)| k.clone())).unwrap_or_default()
}

/// `isOfficialMap(mapName)`.
pub fn is_official_map<H: MapUtilHost>(h: &mut H, map_name: &[u8]) -> bool {
    if h.map_cache().is_none() || map_name.is_empty() {
        return false;
    }
    h.update_map_cache();
    let lower = map_name.to_ascii_lowercase();
    h.map_cache().and_then(|c| c.maps.get(&lower)).is_some_and(|m| m.is_official)
}

/// `getMapPreviewImage(mapName)`: the image named after the map's portable path (`\` and `:` made `_`, the `.map`
/// dropped) from the collection; else the host's new one from the map's `.tga` ([`MapUtilHost::map_preview_from_file`]).
/// `TheGlobalData` always exists here.
pub fn get_map_preview_image<H: MapUtilHost>(h: &mut H, map_name: &[u8]) -> Option<Vec<u8>> {
    let mut tga_name = map_name.to_vec();
    for _ in 0..4 {
        tga_name.pop();
    }
    let name = tga_name.clone();
    let mut filename = tga_name.iter().rposition(|&c| c == b'\\').map_or_else(Vec::new, |i| tga_name[i + 1..].to_vec());
    filename.extend_from_slice(b".tga");
    tga_name.extend_from_slice(b".tga");
    let portable_name = h.real_map_path_to_portable(&name);
    let temp_name: Vec<u8> = portable_name.iter().map(|&c| if c == b'\\' || c == b':' { b'_' } else { c }).collect();
    let mut name = temp_name.clone();
    name.extend_from_slice(b".tga");
    if let Some(image) = h.find_mapped_image(&temp_name) {
        return Some(image);
    }
    h.map_preview_from_file(&tga_name, &name, &temp_name)
}

/// `findDrawPositions(startX, startY, width, height, extent, &ul, &lr)`: where a map of `extent` sits in a
/// `width` x `height` area at (`startX`, `startY`) keeping its aspect ratio.
pub fn find_draw_positions(start_x: i32, start_y: i32, width: i32, height: i32, extent: &Region3D) -> (ICoord2D, ICoord2D) {
    let ew = extent.hi.x - extent.lo.x;
    let eh = extent.hi.y - extent.lo.y;
    let ratio_width = ew / (width as f32 * 1.0);
    let ratio_height = eh / (height as f32 * 1.0);
    let (mut ul, mut lr);
    if ratio_width >= ratio_height {
        let rx = ew / ratio_width;
        let ry = eh / ratio_width;
        ul = ICoord2D { x: 0, y: ((height as f32 - ry) / 2.0) as i32 };
        lr = ICoord2D { x: rx as i32, y: 0 };
        lr.y = height - ul.y;
    } else {
        let rx = ew / ratio_height;
        let ry = eh / ratio_height;
        ul = ICoord2D { x: ((width as f32 - rx) / 2.0) as i32, y: 0 };
        lr = ICoord2D { x: 0, y: ry as i32 };
        lr.x = width - ul.x;
    }
    ul.x += start_x;
    ul.y += start_y;
    lr.x += start_x;
    lr.y += start_y;
    (ul, lr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::base_type::Coord3D;

    /// #89: `findDrawPositions`, predicted by hand before the run: a 2:1 map in a 200x200 area sits in a 200x100 band
    /// centred (50 above and below); a 1:2 map in a 100x50 area at (10, 20) is 25 wide, centred: (47, 20)-(72, 70).
    ///
    /// **Result (2026-11-10):** the first as predicted; **the second missed**: (47, 20)-(73, 70). The prediction's
    /// arithmetic was wrong, not the code: `ul.x` is 37.5 truncated to 37, and `lr.x` is `width - ul.x` = 63, not
    /// 37 + 25, so the band is 26 wide after the truncation. The assertion now says 73.
    #[test]
    fn a_map_keeps_its_aspect_ratio() {
        let e = |x: f32, y: f32| Region3D { lo: Coord3D { x: 0.0, y: 0.0, z: 0.0 }, hi: Coord3D { x, y, z: 0.0 } };
        assert_eq!(find_draw_positions(0, 0, 200, 200, &e(400.0, 200.0)), (ICoord2D { x: 0, y: 50 }, ICoord2D { x: 200, y: 150 }));
        assert_eq!(find_draw_positions(10, 20, 100, 50, &e(100.0, 200.0)), (ICoord2D { x: 47, y: 20 }, ICoord2D { x: 73, y: 70 }));
    }
}
