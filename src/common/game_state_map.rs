// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/GameStateMap.h
//   GeneralsMD/Code/GameEngine/Source/Common/System/SaveGame/GameStateMap.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `GameStateMap`: the save file's `CHUNK_GameStateMap` block, which carries the map file itself, the game mode, the
//! object and drawable id counters and the skirmish game's options, and starts the loaded game; and the clean-up of
//! the maps a load leaves in the save directory (layer 12 batch M2).
//!
//! # Shape
//!
//! [`xfer_game_state_map`] is `GameStateMap::xfer`; a world registers the block with [`GameState::add_snapshot_block`]
//! and calls it from [`GameStateHost::xfer_block`], which is handed the `GameState`. Upstream throws
//! `SC_INVALID_DATA` (a `SaveCode`) where an `XferStatus` cannot say it, so the function returns a [`MapXferError`];
//! a block's xfer can only return an `XferStatus`, so the world maps the others to one and keeps the whole error, as
//! [`crate::common::state_machine`] does.
//!
//! The file system (`TheFileSystem`'s file for the pristine map, the C library's `FILE` for the map in the save
//! directory, Win32's directory search for the clean-up) is the host's, one call per upstream call, so a failure can
//! fall where upstream's does (a file left open, a file created and left empty).
//!
//! # Upstream quirks kept
//!
//! - On a load, `TheGameLogic->setLoadingSave(TRUE)` comes first; a throw after it leaves it set.
//! - A failed read leaves the file open (the `throw` skips the close); so does a failed xfer of a map read from the
//!   save directory.
//! - `TheSkirmishGameInfo` is deleted on the xfer of a game that is not a skirmish, even if a load's post-process list
//!   still holds it: the list then dangles (a world's `load_post_process` of that block is a use after free).
//! - `clearScratchPadMaps` returns without restoring the current directory when the save directory has no entries.

use crate::common::game_info::{GameInfo, GameInfoHost, SkirmishXfer};
use crate::common::game_state::{map_leaf_name, GameState, GameStateHost};
use crate::common::game_type::{DrawableId, ObjectId};
use crate::common::ini::{IniError, Undefined};
use crate::common::recorder::game_mode;
use crate::common::xfer::{Xfer, XferMode, XferStatus, XferVersion, XO_NO_POST_PROCESSING};
use crate::logic::modules::plant;

/// What `GameStateMap::xfer` throws.
#[derive(Clone, Debug, PartialEq)]
pub enum MapXferError {
    /// An `XferStatus` (the xfer's own).
    Xfer(XferStatus),
    /// `SC_INVALID_DATA`.
    InvalidData,
    /// Undefined upstream.
    Undefined(IniError),
}

impl From<XferStatus> for MapXferError {
    fn from(s: XferStatus) -> Self {
        MapXferError::Xfer(s)
    }
}

impl From<IniError> for MapXferError {
    fn from(e: IniError) -> Self {
        MapXferError::Undefined(e)
    }
}

type MapResult<T> = Result<T, MapXferError>;

/// One item of a Win32 directory search (`WIN32_FIND_DATA`'s `cFileName` and whether `dwFileAttributes` has
/// `FILE_ATTRIBUTE_DIRECTORY`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindItem {
    pub name: Vec<u8>,
    pub is_directory: bool,
}

/// The world as `GameStateMap.cpp` reaches it, beyond `GameState` and the skirmish game's `GameInfo`.
pub trait GameStateMapHost<B>: GameStateHost<B> + GameInfoHost {
    /// `TheGameLogic->setLoadingSave(on)`.
    fn set_loading_save(&mut self, on: bool);
    /// `TheGameLogic->getGameMode()`.
    fn logic_game_mode(&self) -> i32;
    /// `TheGameLogic->setGameMode(mode)`.
    fn set_logic_game_mode(&mut self, mode: i32);
    /// `TheWritableGlobalData->m_mapName = name`.
    fn set_map_name(&mut self, name: &[u8]);
    /// `TheGameLogic->getObjectIDCounter()`.
    fn object_id_counter(&self) -> ObjectId;
    /// `TheGameLogic->setObjectIDCounter(id)`.
    fn set_object_id_counter(&mut self, id: ObjectId);
    /// `TheGameClient->getDrawableIDCounter()`.
    fn drawable_id_counter(&self) -> DrawableId;
    /// `TheGameClient->setDrawableIDCounter(id)`.
    fn set_drawable_id_counter(&mut self, id: DrawableId);
    /// `TheGameLogic->startNewGame(loadingSaveGame)`.
    fn start_new_game(&mut self, loading_save_game: bool);
    /// `TheSkirmishGameInfo` (`None` is `NULL`).
    fn skirmish_game_info(&mut self) -> &mut Option<GameInfo>;
    /// The block a load's post-process list holds for `TheSkirmishGameInfo` (its `loadPostProcess` is empty
    /// upstream; the block names that object, not any later one).
    fn skirmish_block(&self) -> B;
    /// The block a load's post-process list holds for that object's starting cash (a `Money`, xferred as a snapshot
    /// inside it; its `loadPostProcess` is empty upstream).
    fn skirmish_cash_block(&self) -> B;

    /// `TheFileSystem->openFile(path, File::READ | File::BINARY) != NULL` (the file then open).
    fn fs_open(&mut self, path: &[u8]) -> bool;
    /// `file->seek(0, File::END)`: the size.
    fn fs_seek_end(&mut self) -> i32;
    /// `file->seek(0, File::START)`.
    fn fs_seek_start(&mut self);
    /// `file->read(buffer, n)`: the bytes read (fewer than `n` on a failure).
    fn fs_read(&mut self, n: usize) -> Vec<u8>;
    /// `file->close()`.
    fn fs_close(&mut self);

    /// `fopen(path, mode) != NULL` (`"rb"` or `"w+b"`; the stream then open).
    fn c_fopen(&mut self, path: &[u8], mode: &str) -> bool;
    /// `fseek(fp, 0, SEEK_END); ftell(fp); fseek(fp, 0, SEEK_SET)`: the size.
    fn c_size(&mut self) -> i32;
    /// `fread(buffer, 1, n, fp)`: the bytes read.
    fn c_fread(&mut self, n: usize) -> Vec<u8>;
    /// `fwrite(data, 1, n, fp)`: the count written.
    fn c_fwrite(&mut self, data: &[u8]) -> usize;
    /// `fclose(fp)`.
    fn c_fclose(&mut self);

    /// `GetCurrentDirectory(_MAX_PATH, buffer)` (a directory of fewer than 260 bytes).
    fn current_directory(&mut self) -> Vec<u8>;
    /// `SetCurrentDirectory(dir)`.
    fn set_current_directory(&mut self, dir: &[u8]);
    /// `FindFirstFile(pattern, &item)`: `None` is `INVALID_HANDLE_VALUE`.
    fn find_first_file(&mut self, pattern: &[u8]) -> Option<FindItem>;
    /// `FindNextFile(handle, &item)`: `None` when it returns 0.
    fn find_next_file(&mut self) -> Option<FindItem>;
    /// `FindClose(handle)`.
    fn find_close(&mut self);
    /// `DeleteFile(name)` (relative to the current directory).
    fn delete_file(&mut self, name: &[u8]);
}

/// `new char[n]` of an `Int` size: a negative one is `std::bad_alloc` on the shipped build.
fn buffer_of(n: i32) -> MapResult<usize> {
    usize::try_from(n).map_err(|_| MapXferError::Undefined(IniError::undefined(Undefined::AllocationFailed)))
}

/// `embedPristineMap(map, xfer)`: the map read whole through the file system, then embedded as a block.
fn embed_pristine_map<B>(map: &[u8], xfer: &mut dyn Xfer, h: &mut impl GameStateMapHost<B>) -> MapResult<()> {
    if !h.fs_open(map) {
        return Err(MapXferError::InvalidData);
    }
    let size = h.fs_seek_end();
    h.fs_seek_start();
    let n = buffer_of(size)?;
    let mut buffer = h.fs_read(n);
    if buffer.len() != n {
        // the throw skips the close
        return Err(MapXferError::InvalidData);
    }
    h.fs_close();
    xfer.begin_block()?;
    xfer.xfer_user(&mut buffer)?;
    xfer.end_block()?;
    Ok(())
}

/// `embedInUseMap(map, xfer)`: the map a load extracted to the save directory, read whole, then embedded as a block
/// (the stream closed after the block).
fn embed_in_use_map<B>(map: &[u8], xfer: &mut dyn Xfer, h: &mut impl GameStateMapHost<B>) -> MapResult<()> {
    if !h.c_fopen(map, "rb") {
        return Err(MapXferError::InvalidData);
    }
    let size = h.c_size();
    let n = buffer_of(size)?;
    let mut buffer = h.c_fread(n);
    if buffer.len() != n {
        if !plant(3991) {
            return Err(MapXferError::InvalidData);
        }
        buffer.resize(n, 0);
    }
    xfer.begin_block()?;
    xfer.xfer_user(&mut buffer)?;
    xfer.end_block()?;
    h.c_fclose();
    Ok(())
}

/// `extractAndSaveMap(mapToSave, xfer)`: the file created, the block's data read and written to it whole.
fn extract_and_save_map<B>(map: &[u8], xfer: &mut dyn Xfer, h: &mut impl GameStateMapHost<B>) -> MapResult<()> {
    if !h.c_fopen(map, "w+b") {
        return Err(MapXferError::InvalidData);
    }
    // `UnsignedInt dataSize = xfer->beginBlock()`, then `new char[dataSize]`
    let data_size = xfer.begin_block()? as u32;
    if data_size > i32::MAX as u32 {
        return Err(MapXferError::Undefined(IniError::undefined(Undefined::AllocationFailed)));
    }
    let mut buffer = vec![0u8; data_size as usize];
    xfer.xfer_user(&mut buffer)?;
    if h.c_fwrite(&buffer) != buffer.len() {
        return Err(MapXferError::InvalidData);
    }
    h.c_fclose();
    xfer.end_block()?;
    Ok(())
}

/// The skirmish game's options as `xferSnapshot` sees them.
fn xfer_skirmish<B: Copy>(
    gs: &mut GameState<B>,
    game: &mut GameInfo,
    xfer: &mut dyn Xfer,
    h: &mut impl GameStateMapHost<B>,
) -> MapResult<()> {
    xfer.xfer_snapshot(&mut SkirmishXfer { game, host: h })?;
    // XferLoad::xferSnapshot adds what it loaded to TheGameState's post-process list, unless told not to: the starting
    // cash (a snapshot inside the game's xfer) first, then the game
    if xfer.get_xfer_mode() == XferMode::Load && xfer.get_options() & XO_NO_POST_PROCESSING == 0 {
        gs.add_post_process(h.skirmish_cash_block());
        gs.add_post_process(h.skirmish_block());
    }
    Ok(())
}

/// `GameStateMap::xfer` (version 2).
///
/// Saving: the map's name in the save directory (`saveGameMapName`, made from the global map name's leaf) and the
/// pristine map's name, both portable, and the game mode; then the map file itself as a block, from the maps (the
/// pristine map, when the global map is not in the save directory: a first save) or from the save directory (the map
/// the last load extracted). Loading: the names read back (the save's map must be in the save directory, else
/// `SC_INVALID_DATA`), the global map name set to it, the game mode set; then the block written out to that file.
/// Then, both ways, the object and drawable id counters; the skirmish game's options in a skirmish (`TheSkirmishGameInfo`
/// made when there is none: `init`, `clearSlotList`, `reset`), else `TheSkirmishGameInfo` deleted; and, loading, the
/// new game started from the save.
pub fn xfer_game_state_map<B: Copy>(gs: &mut GameState<B>, xfer: &mut dyn Xfer, h: &mut impl GameStateMapHost<B>) -> MapResult<()> {
    if xfer.get_xfer_mode() == XferMode::Load {
        h.set_loading_save(true);
    }
    let current: XferVersion = 2;
    let mut version = current;
    xfer.xfer_version(&mut version, current)?;
    let mut first_save = false;
    if xfer.get_xfer_mode() == XferMode::Save {
        let leaf = map_leaf_name(&h.map_name());
        gs.info.save_game_map_name = gs.file_path_in_save_directory(&leaf, h);
        let mut tmp = gs.real_map_path_to_portable(&gs.info.save_game_map_name, h)?;
        xfer.xfer_ascii_string(&mut tmp)?;
        let global = h.map_name();
        if !gs.is_in_save_directory(&global, h) {
            gs.info.pristine_map_name = global;
            first_save = true;
        }
        let mut tmp = gs.real_map_path_to_portable(&gs.info.pristine_map_name, h)?;
        xfer.xfer_ascii_string(&mut tmp)?;
        let mut mode = h.logic_game_mode();
        xfer.xfer_int(&mut mode)?;
    } else {
        let mut tmp = Vec::new();
        xfer.xfer_ascii_string(&mut tmp)?;
        gs.info.save_game_map_name = gs.portable_map_path_to_real(&tmp, h)?;
        if !gs.is_in_save_directory(&gs.info.save_game_map_name, h) && !plant(3992) {
            return Err(MapXferError::InvalidData);
        }
        h.set_map_name(&gs.info.save_game_map_name);
        xfer.xfer_ascii_string(&mut gs.info.pristine_map_name)?;
        gs.info.pristine_map_name = gs.portable_map_path_to_real(&gs.info.pristine_map_name, h)?;
        let mut mode = 0;
        xfer.xfer_int(&mut mode)?;
        h.set_logic_game_mode(mode);
    }

    if xfer.get_xfer_mode() == XferMode::Save {
        if first_save && !plant(3993) {
            let map = gs.info.pristine_map_name.clone();
            embed_pristine_map(&map, xfer, h)?;
        } else {
            let map = gs.info.save_game_map_name.clone();
            embed_in_use_map(&map, xfer, h)?;
        }
    } else {
        let map = gs.info.save_game_map_name.clone();
        extract_and_save_map(&map, xfer, h)?;
    }

    let mut object_id = h.object_id_counter();
    xfer.xfer_object_id(&mut object_id)?;
    h.set_object_id_counter(object_id);
    let mut drawable_id = h.drawable_id_counter();
    if !plant(3994) {
        xfer.xfer_drawable_id(&mut drawable_id)?;
    }
    h.set_drawable_id_counter(drawable_id);

    if h.logic_game_mode() == game_mode::SKIRMISH {
        let mut game = match h.skirmish_game_info().take() {
            Some(g) => g,
            None => {
                // NEW SkirmishGameInfo; init(); clearSlotList(); reset()
                let mut g = GameInfo::new(h);
                g.reset(h);
                if !plant(3995) {
                    g.clear_slot_list(h);
                }
                g.reset(h);
                g
            }
        };
        let r = xfer_skirmish(gs, &mut game, xfer, h);
        *h.skirmish_game_info() = Some(game);
        r?;
    } else if !plant(3996) {
        *h.skirmish_game_info() = None;
    }

    if xfer.get_xfer_mode() == XferMode::Load {
        h.start_new_game(true);
        h.set_loading_save(plant(3997));
    }
    Ok(())
}

/// `GameStateMap::clearScratchPadMaps()`: every file in the save directory whose name ends in `.map` (any case)
/// deleted, each after the search has moved past it; the current directory restored (not when the search finds
/// nothing). Upstream runs it when `TheGameStateMap` is destroyed.
pub fn clear_scratch_pad_maps<B: Copy>(gs: &GameState<B>, h: &mut impl GameStateMapHost<B>) {
    let current = h.current_directory();
    let dir = gs.save_directory(h);
    h.set_current_directory(&dir);
    let Some(mut item) = h.find_first_file(b"*") else {
        return;
    };
    loop {
        let mut to_delete = None;
        if !item.is_directory {
            if let Some(p) = item.name.iter().rposition(|&c| c == b'.') {
                if item.name[p..].eq_ignore_ascii_case(b".map") && !plant(3998) {
                    to_delete = Some(item.name.clone());
                }
            }
        }
        let next = h.find_next_file();
        if let Some(name) = to_delete.filter(|n| !n.is_empty()) {
            h.delete_file(&name);
        }
        match next {
            Some(n) => item = n,
            None => break,
        }
    }
    h.find_close();
    h.set_current_directory(&current);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_negative_size_is_a_failed_allocation() {
        assert!(matches!(buffer_of(-1), Err(MapXferError::Undefined(_))));
        assert_eq!(buffer_of(5), Ok(5));
    }
}
