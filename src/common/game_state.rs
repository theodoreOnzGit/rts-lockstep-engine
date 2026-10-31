// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/GameState.h
//   GeneralsMD/Code/GameEngine/Source/Common/System/SaveGame/GameState.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `GameState`: the save file's blocks, the save-game information block, the post-process list, the next save file's
//! name, and the map paths a save stores portably (layer 12 batch S1).
//!
//! # Shape
//!
//! Upstream registers each subsystem's `Snapshot*` under a block name, per [`SnapshotType`]. Here a block is the
//! caller's id (`B`), and the host xfers it ([`GameStateHost::xfer_block`]); the registry, the save file's layout (each
//! block's name, then the block with its size, then `SG_EOF`; on load, blocks found by name case-blind, unknown ones
//! skipped by their size) and the post-process list are this file's. `GameState`'s own block (`CHUNK_GameState`) is the
//! [`SaveGameInfo`], xferred by [`GameState::xfer_info`].
//!
//! # Post-processing
//!
//! Upstream's `XferLoad::xferSnapshot` adds every snapshot it loads to `GameState`'s list, nested ones included, after
//! their data (so an inner snapshot is post-processed before the one that holds it). Here a block's load adds the
//! block ([`GameState::add_post_process`] is also open to the blocks for their inner snapshots); the order is upstream's
//! when each block adds its inner snapshots as it loads them.
//!
//! # Not translated
//!
//! The save and load listboxes and the available games list (client), `getUnicodeDateBuffer`/`getUnicodeTimeBuffer`
//! (Windows' locale formats, display only), `init`'s registration of the engine's subsystems (the world's), and
//! `saveGame`/`loadGame`'s file handling, error boxes and engine reset (the game loop's, through the host).

use crate::common::ini::{IniError, Undefined};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion, XO_NO_POST_PROCESSING};
use crate::logic::modules::plant;

/// `SAVE_FILE_EOF`.
pub const SAVE_FILE_EOF: &[u8] = b"SG_EOF";
/// `SAVE_GAME_EXTENSION`.
pub const SAVE_GAME_EXTENSION: &[u8] = b".sav";
/// `ZERO_NAME_ONLY`.
pub const ZERO_NAME_ONLY: &[u8] = b"00000000";
/// `MAX_SAVE_FILE_NUMBER`.
pub const MAX_SAVE_FILE_NUMBER: i32 = 99_999_999;
/// `GAME_STATE_BLOCK_STRING`.
pub const GAME_STATE_BLOCK_STRING: &[u8] = b"CHUNK_GameState";
/// `CAMPAIGN_BLOCK_STRING`.
pub const CAMPAIGN_BLOCK_STRING: &[u8] = b"CHUNK_Campaign";
/// `PORTABLE_SAVE`.
pub const PORTABLE_SAVE: &[u8] = b"Save\\";
/// `PORTABLE_MAPS`.
pub const PORTABLE_MAPS: &[u8] = b"Maps\\";
/// `PORTABLE_USER_MAPS`.
pub const PORTABLE_USER_MAPS: &[u8] = b"UserData\\Maps\\";
/// `MapCache::getMapDir()`.
pub const MAP_DIR: &[u8] = b"Maps";
/// `CampaignManager::INVALID_MISSION_NUMBER`.
pub const INVALID_MISSION_NUMBER: i32 = -1;

/// `enum SnapshotType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotType {
    SaveLoad = 0,
    DeepCrcLogicOnly = 1,
    DeepCrc = 2,
}

/// `SAVE_FILE_TYPE_NORMAL`.
pub const SAVE_FILE_TYPE_NORMAL: i32 = 0;
/// `SAVE_FILE_TYPE_MISSION`.
pub const SAVE_FILE_TYPE_MISSION: i32 = 1;

/// `SaveDate`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaveDate {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub day_of_week: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub milliseconds: u16,
}

impl SaveDate {
    /// `isNewerThan(other)`: year, month, day, hour, minute, second, milliseconds in turn (not the day of the week).
    pub fn is_newer_than(&self, other: &SaveDate) -> bool {
        let a = [self.year, self.month, self.day, self.hour, self.minute, self.second, self.milliseconds];
        let b = [other.year, other.month, other.day, other.hour, other.minute, other.second, other.milliseconds];
        if plant(3981) {
            return a >= b;
        }
        a > b
    }
}

/// `SaveGameInfo`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SaveGameInfo {
    pub map_label: Vec<u8>,
    pub date: SaveDate,
    pub campaign_side: Vec<u8>,
    pub mission_number: i32,
    pub description: Vec<u16>,
    /// `saveFileType` (an enum xferred as its four bytes: any value can be loaded).
    pub save_file_type: i32,
    pub mission_map_name: Vec<u8>,
    pub pristine_map_name: Vec<u8>,
    /// `saveGameMapName`: the map in the save directory (set by `GameStateMap`'s xfer; not in this block's data).
    pub save_game_map_name: Vec<u8>,
}

/// The world as `GameState.cpp` reaches it.
pub trait GameStateHost<B> {
    /// `xfer->xferSnapshot(block's snapshot)`'s `snapshot->xfer(xfer)`; `gs` is `TheGameState` (a block such as
    /// `GameStateMap`'s reaches it).
    fn xfer_block(&mut self, block: B, gs: &mut GameState<B>, xfer: &mut dyn Xfer) -> XferResult<()>;
    /// `snapshot->loadPostProcess()`.
    fn load_post_process(&mut self, block: B);
    /// `ThePartitionManager->update()`.
    fn partition_update(&mut self);
    /// `TheGlobalData->getPath_UserData()`.
    fn user_data_path(&self) -> Vec<u8>;
    /// `TheGlobalData->m_mapName`.
    fn map_name(&self) -> Vec<u8>;
    /// `GetLocalTime`: year, month, day of week, day, hour, minute, second, milliseconds (`SYSTEMTIME`'s order).
    fn local_time(&mut self) -> [u16; 8];
    /// `MapObject::getWorldDict()->getAsciiString(TheKey_mapName, &exists)`: `None` with no world dict or no key.
    fn world_dict_map_name(&self) -> Option<Vec<u8>>;
    /// `TheCampaignManager->getCurrentCampaign()`'s name and `getCurrentMissionNumber()`, when there is a campaign.
    fn current_campaign(&self) -> Option<(Vec<u8>, i32)>;
    /// `_access(path, 0) == -1` is false: the file exists.
    fn file_exists(&self, path: &[u8]) -> bool;
}

/// One of a [`SaveDate`]'s fields.
type DateField = fn(&mut SaveDate) -> &mut u16;

/// A snapshot block of the registry (`SnapshotBlock`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotBlock<B> {
    pub block_name: Vec<u8>,
    pub block: B,
}

/// `GameState`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameState<B> {
    pub blocks: [Vec<SnapshotBlock<B>>; 3],
    pub post_process: Vec<B>,
    pub info: SaveGameInfo,
    pub is_in_load_game: bool,
}

impl<B> Default for GameState<B> {
    fn default() -> Self {
        GameState { blocks: [Vec::new(), Vec::new(), Vec::new()], post_process: Vec::new(), info: SaveGameInfo::default(), is_in_load_game: false }
    }
}

type R<T> = Result<T, IniError>;

/// `AsciiString::toLower` (a byte of 0x80 or more is undefined; a string of 2,048 or more overflows its buffer).
fn to_lower(s: &[u8]) -> R<Vec<u8>> {
    if s.len() >= crate::common::ascii_string::MAX_FORMAT_BUF_LEN {
        return Err(IniError::undefined(Undefined::MessageOverflow));
    }
    if s.iter().any(|&b| b >= 0x80) {
        return Err(IniError::undefined(Undefined::TolowerNonAscii));
    }
    Ok(s.to_ascii_lowercase())
}

fn starts_with_no_case(s: &[u8], p: &[u8]) -> bool {
    s.len() >= p.len() && s[..p.len()].eq_ignore_ascii_case(p)
}

/// `getMapLeafName(in)`: after the last '\\' (perhaps empty), or the whole.
pub fn map_leaf_name(s: &[u8]) -> Vec<u8> {
    match s.iter().rposition(|&b| b == b'\\') {
        Some(i) => s[i + 1..].to_vec(),
        None => s.to_vec(),
    }
}

/// `getMapLeafAndDirName(in)`: after the second-last '\\', or the whole when there are fewer than two.
pub fn map_leaf_and_dir_name(s: &[u8]) -> Vec<u8> {
    let Some(p) = s.iter().rposition(|&b| b == b'\\') else {
        return s.to_vec();
    };
    match s[..p].iter().rposition(|&b| b == b'\\') {
        Some(p2) => s[p2 + 1..].to_vec(),
        None => s.to_vec(),
    }
}

/// A block's snapshot as `xferSnapshot` sees it.
struct BlockSnapshot<'a, B: Copy, H: GameStateHost<B>> {
    block: B,
    gs: &'a mut GameState<B>,
    host: &'a mut H,
}

impl<B: Copy, H: GameStateHost<B>> Snapshot for BlockSnapshot<'_, B, H> {
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.host.xfer_block(self.block, self.gs, xfer)
    }
    fn load_post_process(&mut self) {}
}

impl<B: Copy> GameState<B> {
    /// `GameState()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `reset()`: the post-process list emptied, not in a load.
    pub fn reset(&mut self) {
        self.post_process.clear();
        self.is_in_load_game = false;
    }

    /// `addSnapshotBlock(name, snapshot, which)`: an empty name is refused.
    pub fn add_snapshot_block(&mut self, name: &[u8], block: B, which: SnapshotType) {
        if name.is_empty() {
            return;
        }
        self.blocks[which as usize].push(SnapshotBlock { block_name: name.to_vec(), block });
    }

    /// `findBlockInfoByToken(token, which)`: the first block of that name (exact case; an empty token finds none).
    pub fn find_block(&self, token: &[u8], which: SnapshotType) -> Option<B> {
        if token.is_empty() {
            return None;
        }
        self.blocks[which as usize].iter().find(|b| b.block_name == token).map(|b| b.block)
    }

    /// `addPostProcessSnapshot(snapshot)`.
    pub fn add_post_process(&mut self, block: B) {
        self.post_process.push(block);
    }

    /// `gameStatePostProcessLoad()`: each in order, the list emptied, then the partition manager's update.
    pub fn game_state_post_process_load(&mut self, h: &mut impl GameStateHost<B>) {
        let list = std::mem::take(&mut self.post_process);
        for b in list {
            h.load_post_process(b);
        }
        h.partition_update();
    }

    /// `xferSaveData(xfer, which)`: saving, each block of `which` (in a mission save, only `CHUNK_GameState` and
    /// `CHUNK_Campaign`) as its name and the block, then `SG_EOF`; loading, names read until `SG_EOF` (case-blind), each
    /// block found by name (exact case) loaded and added to the post-process list, an unknown one skipped by its size.
    pub fn xfer_save_data(&mut self, xfer: &mut dyn Xfer, which: SnapshotType, h: &mut impl GameStateHost<B>) -> XferResult<()> {
        if xfer.get_xfer_mode() == XferMode::Save {
            for i in 0..self.blocks[which as usize].len() {
                let b = self.blocks[which as usize][i].clone();
                let mission = self.info.save_file_type == SAVE_FILE_TYPE_MISSION;
                if !mission || b.block_name.eq_ignore_ascii_case(GAME_STATE_BLOCK_STRING) || b.block_name.eq_ignore_ascii_case(CAMPAIGN_BLOCK_STRING) {
                    let mut name = b.block_name.clone();
                    xfer.xfer_ascii_string(&mut name)?;
                    xfer.begin_block()?;
                    xfer.xfer_snapshot(&mut BlockSnapshot { block: b.block, gs: self, host: h })?;
                    xfer.end_block()?;
                }
            }
            let mut eof = SAVE_FILE_EOF.to_vec();
            xfer.xfer_ascii_string(&mut eof)
        } else {
            loop {
                let mut token = Vec::new();
                xfer.xfer_ascii_string(&mut token)?;
                if token.eq_ignore_ascii_case(SAVE_FILE_EOF) || (plant(3982) && token.is_empty()) {
                    return Ok(());
                }
                match self.find_block(&token, which) {
                    None => {
                        let size = xfer.begin_block()?;
                        xfer.skip(size)?;
                    }
                    Some(block) => {
                        xfer.begin_block()?;
                        xfer.xfer_snapshot(&mut BlockSnapshot { block, gs: self, host: h })?;
                        // XferLoad::xferSnapshot adds what it loaded, unless told not to
                        if xfer.get_xfer_mode() == XferMode::Load && xfer.get_options() & XO_NO_POST_PROCESSING == 0 && !plant(3983) {
                            self.add_post_process(block);
                        }
                        xfer.end_block()?;
                    }
                }
            }
        }
    }

    /// `friend_xferSaveDataForCRC(xfer, which)`: the save information's description, file type and map names cleared,
    /// then `xferSaveData`.
    pub fn xfer_save_data_for_crc(&mut self, xfer: &mut dyn Xfer, which: SnapshotType, h: &mut impl GameStateHost<B>) -> XferResult<()> {
        self.info.description.clear();
        self.info.save_file_type = SAVE_FILE_TYPE_NORMAL;
        self.info.mission_map_name.clear();
        self.info.pristine_map_name.clear();
        self.xfer_save_data(xfer, which, h)
    }

    /// `GameState::xfer` (version 2), `CHUNK_GameState`'s data: the file type and mission map, the local time (taken
    /// now, on a load too, then overwritten by what is read), the description, the map label (the world dict's map
    /// name, else the global map name's leaf), the campaign and mission number.
    pub fn xfer_info(&mut self, xfer: &mut dyn Xfer, h: &mut impl GameStateHost<B>) -> XferResult<()> {
        let current: XferVersion = 2;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        let info = &mut self.info;
        if version >= 2 {
            let mut t = info.save_file_type.to_le_bytes();
            xfer.xfer_user(&mut t)?;
            info.save_file_type = i32::from_le_bytes(t);
            xfer.xfer_ascii_string(&mut info.mission_map_name)?;
        }
        let st = h.local_time();
        // SYSTEMTIME: wYear, wMonth, wDayOfWeek, wDay, wHour, wMinute, wSecond, wMilliseconds
        let fields: [(u16, DateField); 8] = [
            (st[0], |d| &mut d.year),
            (st[1], |d| &mut d.month),
            (st[3], |d| &mut d.day),
            (st[2], |d| &mut d.day_of_week),
            (st[4], |d| &mut d.hour),
            (st[5], |d| &mut d.minute),
            (st[6], |d| &mut d.second),
            (st[7], |d| &mut d.milliseconds),
        ];
        for (v, f) in fields {
            let slot = f(&mut info.date);
            *slot = v;
            xfer.xfer_unsigned_short(slot)?;
        }
        xfer.xfer_unicode_string(&mut info.description)?;
        let label = h.world_dict_map_name();
        match label {
            Some(l) if !l.is_empty() => info.map_label = l,
            _ => {
                let m = h.map_name();
                if m.len() >= 260 {
                    // strcpy into a _MAX_PATH buffer
                    return Err(XferStatus::ErrorUnknown);
                }
                info.map_label = map_leaf_name(&m);
            }
        }
        xfer.xfer_ascii_string(&mut info.map_label)?;
        match h.current_campaign() {
            Some((name, mission)) => {
                info.campaign_side = name;
                xfer.xfer_ascii_string(&mut info.campaign_side)?;
                info.mission_number = mission;
                xfer.xfer_int(&mut info.mission_number)
            }
            None => {
                info.campaign_side.clear();
                xfer.xfer_ascii_string(&mut info.campaign_side)?;
                info.mission_number = INVALID_MISSION_NUMBER;
                xfer.xfer_int(&mut info.mission_number)
            }
        }
    }

    /// `getSaveDirectory()`: the user data path and `Save\`.
    pub fn save_directory(&self, h: &impl GameStateHost<B>) -> Vec<u8> {
        let mut d = h.user_data_path();
        d.extend_from_slice(b"Save\\");
        d
    }

    /// `getFilePathInSaveDirectory(leaf)`.
    pub fn file_path_in_save_directory(&self, leaf: &[u8], h: &impl GameStateHost<B>) -> Vec<u8> {
        let mut d = self.save_directory(h);
        d.extend_from_slice(leaf);
        d
    }

    /// `isInSaveDirectory(path)`.
    pub fn is_in_save_directory(&self, path: &[u8], h: &impl GameStateHost<B>) -> bool {
        starts_with_no_case(path, &self.save_directory(h))
    }

    /// `findNextSaveFilename(desc)` (the build's `LOWEST_NUMBER` search): the first `%08d.sav` the save directory does
    /// not have, or empty past `MAX_SAVE_FILE_NUMBER`.
    pub fn find_next_save_filename(&self, h: &impl GameStateHost<B>) -> Vec<u8> {
        let mut i: i32 = 0;
        loop {
            let mut name = format!("{i:08}").into_bytes();
            name.extend_from_slice(SAVE_GAME_EXTENSION);
            if !h.file_exists(&self.file_path_in_save_directory(&name, h)) {
                return name;
            }
            i += 1;
            if i > MAX_SAVE_FILE_NUMBER {
                return Vec::new();
            }
        }
    }

    /// `realMapPathToPortableMapPath(in)`: a map in the save directory as `save\<leaf>`, in the map directory as
    /// `maps\<dir>\<leaf>`, in the user's as `userdata\maps\<dir>\<leaf>`, anything else as it is; lower case.
    pub fn real_map_path_to_portable(&self, s: &[u8], h: &impl GameStateHost<B>) -> R<Vec<u8>> {
        let user_maps = {
            let mut d = h.user_data_path();
            d.extend_from_slice(MAP_DIR);
            d
        };
        let mut out;
        if starts_with_no_case(s, &self.save_directory(h)) {
            out = PORTABLE_SAVE.to_vec();
            out.extend_from_slice(&map_leaf_name(s));
        } else if starts_with_no_case(s, MAP_DIR) && !plant(3984) {
            out = PORTABLE_MAPS.to_vec();
            out.extend_from_slice(&map_leaf_and_dir_name(s));
        } else if starts_with_no_case(s, &user_maps) {
            out = PORTABLE_USER_MAPS.to_vec();
            out.extend_from_slice(&map_leaf_and_dir_name(s));
        } else {
            out = s.to_vec();
        }
        to_lower(&out)
    }

    /// `portableMapPathToRealMapPath(in)`: the inverse (the map directories joined with a '\\'); lower case.
    pub fn portable_map_path_to_real(&self, s: &[u8], h: &impl GameStateHost<B>) -> R<Vec<u8>> {
        let mut out;
        if starts_with_no_case(s, PORTABLE_SAVE) {
            out = self.save_directory(h);
            out.extend_from_slice(&map_leaf_name(s));
        } else if starts_with_no_case(s, PORTABLE_MAPS) {
            out = MAP_DIR.to_vec();
            out.push(b'\\');
            out.extend_from_slice(&map_leaf_and_dir_name(s));
        } else if starts_with_no_case(s, PORTABLE_USER_MAPS) {
            out = h.user_data_path();
            out.extend_from_slice(MAP_DIR);
            out.push(b'\\');
            out.extend_from_slice(&map_leaf_and_dir_name(s));
        } else {
            out = s.to_vec();
        }
        to_lower(&out)
    }
}
