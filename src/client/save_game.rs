// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/System/SaveGame/GameState.cpp (`init`'s registration of the client's blocks,
//     `saveGame`'s and `loadGame`'s order of work)
//   GeneralsMD/Code/GameEngine/Source/Common/System/Radar.cpp (RadarObject::xfer, xferRadarObjectList, Radar::xfer,
//     Radar::loadPostProcess)
//   GeneralsMD/Code/GameEngine/Source/GameClient/View.cpp (View::xfer)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GameClient.cpp (GameClient::xferDrawableTOC, GameClient::xfer)
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (InGameUI::xfer)
//   GeneralsMD/Code/GameEngine/Source/GameClient/System/ParticleSys.cpp (ParticleSystemManager::xfer)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/GhostObject.cpp (GhostObjectManager::xfer)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameLogic/W3DGhostObject.cpp (W3DGhostObjectManager::xfer)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The whole save file, the client's blocks with the logic's (GitHub #4).
//!
//! Upstream's `GameState::init` registers the logic's and the client's snapshots in one list, and a save file holds them
//! all in that order. The logic's are the World's ([`crate::logic::world::game_state_world`]); this module adds the
//! client's, so a save written here has upstream's blocks in upstream's order, and an original game's save can be read.
//!
//! # The client's blocks, and what is done with each
//!
//! | Block | Upstream | Here |
//! |---|---|---|
//! | `CHUNK_Radar` | `Radar::xfer` v1 | **Translated whole**: the hidden and forced flags, both object lists (each object's id and colour; a load gives each object its radar data back, `friend_setRadarData`, the World's `on_radar`), the 64 events, the two indices. `loadPostProcess` refreshes the terrain. |
//! | `CHUNK_TacticalView` | `View::xfer` v1 | **Translated whole**: the angle and the position, into [`TacticalViewSave`]; the application turns its camera to them (upstream's `setAngle`, `lookAt`). |
//! | `CHUNK_GameClient` | `GameClient::xfer` v3 | **The frame, the drawable table of contents, each drawable's object id and the briefing list translated; each drawable's own data (`Drawable::xfer`: its modules, model condition, animation, tint, ambient sound) read past** by its block's size, as upstream itself skips a drawable of an unknown template. A loaded game's drawables are made new from their objects (the client makes one for every object), so their looks start from their objects' state, not the save's. A save writes the frame, an empty table, no drawables and no briefing lines (upstream's load then leaves each object's drawable as it made it). |
//! | `CHUNK_InGameUI` | `InGameUI::xfer` v3 | **Translated whole** into [`InGameUiSave`] (the named timers and the superweapon timers; a superweapon's special power must exist, as upstream). The port's in-game UI does not show these timers yet, so a load keeps what it read and a save writes it back. |
//! | `CHUNK_ParticleSystem` | `ParticleSystemManager::xfer` v1 | **The system id counter translated; the systems read past** (`ParticleSystem::xfer` and its particles are not translated): a loaded game's effects start empty. A save writes the counter and no systems. |
//! | `CHUNK_TerrainVisual` | `W3DTerrainVisual::xfer` v3 | **The World's** (the logic height map and the water grid live in the World: `World::xfer_terrain_visual_part`); a version 3 block's tree and prop buffers are read past here. A save writes version 2. |
//! | `CHUNK_GhostObject` | `W3DGhostObjectManager::xfer` v1 | **The local player translated; the ghost objects read past** (`W3DGhostObject::xfer`, the last seen look of a shrouded building, is not translated). A save writes no ghost objects. |
//!
//! ~~`CHUNK_Campaign` is the logic's and is not registered (see `game_state_world`): a load skips it by its size, as any
//! unknown block.~~ **Corrected 2026-11-06 (GitHub #58):** `CHUNK_Campaign` is registered, second as upstream's, and is the
//! World's (`CampaignManager::xfer` over the World's campaign stand-in): a save outside a campaign loads; a campaign's save
//! (a campaign or mission named, or a challenge) fails there, with `SC_INVALID_DATA`, as the port has no campaigns.
//!
//! # Starting the save's game (GitHub #58)
//!
//! Upstream's load starts the save's own game inside `CHUNK_GameStateMap` (`startNewGame(TRUE)`, with the map the block
//! extracts and the `TheSkirmishGameInfo` it reads). A World made ready for that (`SaveSide::new_game_on_load`: a World with
//! no game, upstream's `TheGameEngine->reset()`) starts it there; the host then makes the radar's new map (the
//! `TheRadar->newMap` of that `startNewGame`, which is the client's) before `CHUNK_Radar` is read. A block is "read past" with the save's own block size, which every block carries; so the game loads
//! without what was read past, as upstream does for blocks it does not know.
//!
//! # Loading an original game's save
//!
//! [`load_client_game`] reads through a [`TracingLoad`], which knows where each block ends (to read past the parts above)
//! and records the last reads, so a load that fails reports the block, the offset in it and the field it was reading
//! ([`LoadFailure`]). [`read_save_summary`] reads only the save's information and map (for the list of saves, and to
//! choose the map a load starts from).
//!
//! # Where this differs from upstream
//!
//! - **The events the load makes are dropped.** Upstream's load runs with the client attached: an object made by the load
//!   gets its drawable and its radar entry at once, and the object's xfer takes it off the radar again before the radar's
//!   block puts it back. Here the World's load emits those calls as client events; [`load_client_game`] drops them, since
//!   the radar's own block is what puts each object back (and the drawables are made from the objects).
//! - **The save's date** is the client's clock ([`ClientSaveState::local_time`]), where the World alone writes zero.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use crate::client::particle_sys::ParticleSystemManager;
use crate::client::radar::{Radar, RadarObject, MAX_RADAR_EVENTS};
use crate::common::base_type::Coord3D;
use crate::common::game_common::MAX_PLAYER_COUNT;
use crate::common::game_state::{GameState, GameStateHost, SaveError, SaveGameInfo, SnapshotType, SAVE_FILE_TYPE_NORMAL};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::ini::game_make_color;
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferState, XferStatus, XferVersion};
use crate::common::xfer_load::XferLoad;
use crate::common::xfer_save::XferSave;
use crate::logic::world::game_logic::World;
use crate::logic::world::game_state_world::WorldBlock;

/// A block of the whole save file: the World's, or the client's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveBlock {
    World(WorldBlock),
    Radar,
    TacticalView,
    GameClient,
    InGameUi,
    ParticleSystem,
    GhostObject,
}

/// `GameState::init`'s `SNAPSHOT_SAVELOAD` registration, in upstream's order: the World's blocks (as
/// `game_state_world::world_game_state` registers them) and the client's. `CHUNK_Campaign` is not registered (the
/// World's note).
pub fn client_game_state() -> GameState<SaveBlock> {
    use SaveBlock as S;
    use WorldBlock as W;
    let mut gs = GameState::new();
    for (name, b) in [
        (&b"CHUNK_GameState"[..], S::World(W::GameState)),
        (b"CHUNK_Campaign", S::World(W::Campaign)),
        (b"CHUNK_GameStateMap", S::World(W::GameStateMap)),
        (b"CHUNK_TerrainLogic", S::World(W::TerrainLogic)),
        (b"CHUNK_TeamFactory", S::World(W::TeamFactory)),
        (b"CHUNK_Players", S::World(W::Players)),
        (b"CHUNK_GameLogic", S::World(W::GameLogic)),
        (b"CHUNK_Radar", S::Radar),
        (b"CHUNK_ScriptEngine", S::World(W::ScriptEngine)),
        (b"CHUNK_SidesList", S::World(W::SidesList)),
        (b"CHUNK_TacticalView", S::TacticalView),
        (b"CHUNK_GameClient", S::GameClient),
        (b"CHUNK_InGameUI", S::InGameUi),
        (b"CHUNK_Partition", S::World(W::Partition)),
        (b"CHUNK_ParticleSystem", S::ParticleSystem),
        (b"CHUNK_TerrainVisual", S::World(W::TerrainVisual)),
        (b"CHUNK_GhostObject", S::GhostObject),
    ] {
        gs.add_snapshot_block(name, b, SnapshotType::SaveLoad);
    }
    gs
}

/// `CHUNK_TacticalView`: `View::xfer`'s angle and look-at position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TacticalViewSave {
    pub angle: f32,
    pub position: Coord3D,
}

/// A named timer as `InGameUI::xfer` writes it (what `addNamedTimer` needs).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NamedTimerSave {
    pub name: Vec<u8>,
    pub text: Vec<u16>,
    pub is_countdown: bool,
}

/// A superweapon timer as `InGameUI::xfer` writes it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SuperweaponSave {
    pub player_index: i32,
    /// The special power template's name.
    pub template_name: Vec<u8>,
    pub power_name: Vec<u8>,
    pub id: ObjectId,
    pub timestamp: u32,
    pub hidden_by_script: bool,
    pub hidden_by_science: bool,
    pub ready: bool,
    pub eva_ready_played: bool,
}

/// `CHUNK_InGameUI`'s data (see the module docs: kept as read, written back).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InGameUiSave {
    pub named_timer_last_flash_frame: i32,
    pub named_timer_used_flash_color: bool,
    pub show_named_timers: bool,
    pub named_timers: Vec<NamedTimerSave>,
    pub superweapon_hidden_by_script: bool,
    pub superweapons: Vec<SuperweaponSave>,
}

impl Default for InGameUiSave {
    /// `InGameUI::reset`'s values: no timers, the flash frame 0, the flash colour used ("so next one is false"), the named
    /// timers shown, superweapons not hidden.
    fn default() -> Self {
        InGameUiSave {
            named_timer_last_flash_frame: 0,
            named_timer_used_flash_color: true,
            show_named_timers: true,
            named_timers: Vec::new(),
            superweapon_hidden_by_script: false,
            superweapons: Vec::new(),
        }
    }
}

/// `CHUNK_GameClient`'s translated part.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameClientSave {
    /// `m_frame`, the client's frame.
    pub frame: u32,
    /// The drawable table of contents a load read: (template name, id).
    pub toc: Vec<(Vec<u8>, u16)>,
    /// The drawables a load read past: their template and object (`INVALID_ID` for one with none).
    pub drawables_read_past: Vec<(Vec<u8>, ObjectId)>,
    /// The mission briefing history (`GetBriefingTextList`); a save writes none (the port keeps no briefing list).
    pub briefing: Vec<Vec<u8>>,
}

/// The client's state a save holds besides the radar and the particle systems' id counter.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientSaveState {
    pub view: TacticalViewSave,
    pub in_game_ui: InGameUiSave,
    pub game_client: GameClientSave,
    /// `GhostObjectManager::m_localPlayer` a load read (a save writes the World's local player).
    pub ghost_local_player: i32,
    /// `GetLocalTime` for the save's date: year, month, day of week, day, hour, minute, second, milliseconds.
    pub local_time: [u16; 8],
    /// What a load read past (see the module docs), in words.
    pub read_past: Vec<String>,
    /// A block's error that is not an `XferStatus` (the xfer sees `XFER_UNKNOWN_ERROR`), in words.
    pub errors: Vec<String>,
}

/// One top-level block a load met: its name, the offset of its data and its size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockSeen {
    pub name: Vec<u8>,
    pub offset: usize,
    pub size: i32,
}

/// What a [`TracingLoad`] has seen.
#[derive(Clone, Debug, Default)]
pub struct LoadTrace {
    /// The read position after the last call.
    pub pos: usize,
    /// The open blocks' (start, end), outermost first.
    pub open: Vec<(usize, usize)>,
    /// The last string read outside any block (a block's name).
    pub last_token: Vec<u8>,
    /// Every top-level block met.
    pub blocks: Vec<BlockSeen>,
    /// The last reads, with their offsets.
    pub reads: VecDeque<String>,
}

impl LoadTrace {
    fn note(&mut self, at: usize, what: String) {
        if self.reads.len() == 16 {
            self.reads.pop_front();
        }
        self.reads.push_back(format!("@{at}: {what}"));
    }

    /// The top-level block being read: its name, the offset of its data and its end.
    pub fn current_block(&self) -> Option<(Vec<u8>, usize, usize)> {
        let (start, end) = *self.open.first()?;
        let name = self.blocks.last().map(|b| b.name.clone()).unwrap_or_default();
        Some((name, start, end))
    }
}

/// New (not upstream's): an [`XferLoad`] that keeps a [`LoadTrace`]: where each block ends, and the last reads.
pub struct TracingLoad {
    inner: XferLoad,
    pub trace: Rc<RefCell<LoadTrace>>,
}

impl TracingLoad {
    pub fn new(bytes: Vec<u8>) -> Self {
        TracingLoad { inner: XferLoad::new(bytes), trace: Rc::new(RefCell::new(LoadTrace::default())) }
    }

    fn sync(&self) {
        self.trace.borrow_mut().pos = self.inner.position().unwrap_or(0);
    }

    fn at(&self) -> usize {
        self.inner.position().unwrap_or(0)
    }
}

impl Xfer for TracingLoad {
    fn state(&self) -> &XferState {
        self.inner.state()
    }
    fn state_mut(&mut self) -> &mut XferState {
        self.inner.state_mut()
    }
    fn open(&mut self, identifier: &[u8]) -> XferResult<()> {
        let r = self.inner.open(identifier);
        self.sync();
        r
    }
    fn close(&mut self) -> XferResult<()> {
        self.inner.close()
    }
    fn begin_block(&mut self) -> XferResult<i32> {
        let at = self.at();
        let r = self.inner.begin_block();
        self.sync();
        let pos = self.at();
        if let Ok(size) = r {
            let mut t = self.trace.borrow_mut();
            if t.open.is_empty() {
                let name = t.last_token.clone();
                t.blocks.push(BlockSeen { name, offset: pos, size });
            }
            t.open.push((pos, pos.saturating_add(size.max(0) as usize)));
            t.note(at, format!("block of {size} bytes"));
        }
        r
    }
    fn end_block(&mut self) -> XferResult<()> {
        self.trace.borrow_mut().open.pop();
        self.inner.end_block()
    }
    fn skip(&mut self, data_size: i32) -> XferResult<()> {
        let at = self.at();
        let r = self.inner.skip(data_size);
        self.sync();
        let pos = self.at();
        let mut t = self.trace.borrow_mut();
        t.note(at, format!("skip {data_size}"));
        while t.open.last().is_some_and(|&(_, end)| end <= pos) {
            t.open.pop();
        }
        r
    }
    fn xfer_snapshot(&mut self, snapshot: &mut dyn Snapshot) -> XferResult<()> {
        // (`XferLoad::xferSnapshot`: the snapshot's xfer through this xfer, so its reads are traced)
        snapshot.xfer(self)
    }
    fn xfer_implementation(&mut self, data: &mut [u8]) -> XferResult<()> {
        let at = self.at();
        let r = self.inner.xfer_implementation(data);
        self.sync();
        let shown: Vec<String> = data.iter().take(8).map(|b| format!("{b:02x}")).collect();
        self.trace.borrow_mut().note(at, format!("{} bytes [{}]{}", data.len(), shown.join(" "), if r.is_err() { " FAILED" } else { "" }));
        r
    }
    fn xfer_version(&mut self, version_data: &mut XferVersion, current_version: XferVersion) -> XferResult<()> {
        let at = self.at();
        let mut b = [*version_data];
        let r = self.inner.xfer_implementation(&mut b);
        self.sync();
        *version_data = b[0];
        r?;
        let ok = *version_data <= current_version;
        self.trace.borrow_mut().note(at, format!("version {} (this port reads up to {current_version}){}", *version_data, if ok { "" } else { " TOO NEW" }));
        if !ok {
            return Err(XferStatus::InvalidVersion);
        }
        Ok(())
    }
    fn xfer_ascii_string(&mut self, s: &mut Vec<u8>) -> XferResult<()> {
        let at = self.at();
        let r = self.inner.xfer_ascii_string(s);
        self.sync();
        let mut t = self.trace.borrow_mut();
        t.note(at, format!("string {:?}{}", String::from_utf8_lossy(s), if r.is_err() { " FAILED" } else { "" }));
        if t.open.is_empty() {
            t.last_token = s.clone();
        }
        r
    }
    fn xfer_unicode_string(&mut self, s: &mut Vec<u16>) -> XferResult<()> {
        let at = self.at();
        let r = self.inner.xfer_unicode_string(s);
        self.sync();
        self.trace.borrow_mut().note(at, format!("unicode string {:?}{}", String::from_utf16_lossy(s), if r.is_err() { " FAILED" } else { "" }));
        r
    }
}

/// A load that failed: where, and what it was reading (the first unreadable or refused field).
#[derive(Clone, Debug, PartialEq)]
pub struct LoadFailure {
    pub error: XferStatus,
    /// The block being read (empty: between blocks).
    pub block: String,
    /// The offset in the file, and in the block's data.
    pub file_offset: usize,
    pub block_offset: usize,
    /// The last reads before the failure, oldest first, the failing one last.
    pub last_reads: Vec<String>,
    /// The errors the blocks recorded (the World's and the client's), in words.
    pub errors: Vec<String>,
    /// The blocks met before it.
    pub blocks: Vec<BlockSeen>,
}

impl std::fmt::Display for LoadFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} in {} at byte {} of the block ({} of the file)", self.error, if self.block.is_empty() { "the block list" } else { &self.block }, self.block_offset, self.file_offset)?;
        if let Some(last) = self.last_reads.last() {
            write!(f, "; reading {last}")?;
        }
        if !self.errors.is_empty() {
            write!(f, "; {}", self.errors.join("; "))?;
        }
        Ok(())
    }
}

fn failure(trace: &LoadTrace, error: XferStatus, errors: Vec<String>) -> LoadFailure {
    let (block, block_offset) = match trace.current_block() {
        Some((name, start, _)) => (String::from_utf8_lossy(&name).into_owned(), trace.pos.saturating_sub(start)),
        None => (String::new(), 0),
    };
    LoadFailure { error, block, file_offset: trace.pos, block_offset, last_reads: trace.reads.iter().cloned().collect(), errors, blocks: trace.blocks.clone() }
}

/// The game a save holds, as `GameState` reaches it here: the World, the client's radar, the particle systems' manager
/// and the rest of the client's saved state.
pub struct SaveHost<'a> {
    pub world: &'a mut World,
    pub radar: &'a mut Radar,
    pub particles: &'a mut ParticleSystemManager,
    pub client: &'a mut ClientSaveState,
    /// The World's blocks see a `GameState<WorldBlock>`: this one, carrying the save's information to and from the whole
    /// file's.
    shadow: GameState<WorldBlock>,
    /// While loading: where the current block ends.
    trace: Option<Rc<RefCell<LoadTrace>>>,
}

impl<'a> SaveHost<'a> {
    pub fn new(world: &'a mut World, radar: &'a mut Radar, particles: &'a mut ParticleSystemManager, client: &'a mut ClientSaveState) -> Self {
        SaveHost { world, radar, particles, client, shadow: GameState::new(), trace: None }
    }

    /// The rest of the current top-level block read past (see the module docs); `what` is recorded.
    fn read_past_rest(&mut self, xfer: &mut dyn Xfer, what: String) -> XferResult<()> {
        let Some(t) = &self.trace else { return Ok(()) };
        let (pos, end) = {
            let t = t.borrow();
            (t.pos, t.open.first().map_or(t.pos, |b| b.1))
        };
        self.client.read_past.push(what);
        if end > pos {
            xfer.skip((end - pos) as i32)?;
        }
        Ok(())
    }

    fn block_error(&mut self, call: &str, e: SaveError) -> XferStatus {
        match e {
            SaveError::Xfer(s) => s,
            other => {
                self.client.errors.push(format!("{call}: {other:?}"));
                XferStatus::ErrorUnknown
            }
        }
    }

    /// `Radar::xfer` (version 1).
    fn xfer_radar(&mut self, xfer: &mut dyn Xfer) -> Result<(), SaveError> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)?;
        xfer.xfer_bool(&mut self.radar.radar_hidden)?;
        xfer.xfer_bool(&mut self.radar.radar_force_on)?;
        xfer_radar_object_list(xfer, &mut self.radar.local_object_list, self.world)?;
        xfer_radar_object_list(xfer, &mut self.radar.object_list, self.world)?;
        let verify = MAX_RADAR_EVENTS as u16;
        let mut count = verify;
        xfer.xfer_unsigned_short(&mut count)?;
        if count != verify {
            return Err(SaveError::InvalidData);
        }
        for e in self.radar.event.iter_mut() {
            // (`xferUser(&type, sizeof(RadarEventType))`: the enum's four bytes)
            let mut t = e.event_type.to_le_bytes();
            xfer.xfer_user(&mut t)?;
            e.event_type = i32::from_le_bytes(t);
            xfer.xfer_bool(&mut e.active)?;
            xfer.xfer_unsigned_int(&mut e.create_frame)?;
            xfer.xfer_unsigned_int(&mut e.die_frame)?;
            xfer.xfer_unsigned_int(&mut e.fade_frame)?;
            xfer.xfer_rgba_color_int(&mut e.color1)?;
            xfer.xfer_rgba_color_int(&mut e.color2)?;
            xfer.xfer_coord3d(&mut e.world_loc)?;
            xfer.xfer_icoord2d(&mut e.radar_loc)?;
            xfer.xfer_bool(&mut e.sound_played)?;
        }
        xfer.xfer_int(&mut self.radar.next_free_radar_event)?;
        xfer.xfer_int(&mut self.radar.last_radar_event)?;
        Ok(())
    }

    /// `View::xfer` (version 1): the angle, then the position's x, y and z.
    fn xfer_view(&mut self, xfer: &mut dyn Xfer) -> Result<(), SaveError> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)?;
        let v = &mut self.client.view;
        xfer.xfer_real(&mut v.angle)?;
        xfer.xfer_real(&mut v.position.x)?;
        xfer.xfer_real(&mut v.position.y)?;
        xfer.xfer_real(&mut v.position.z)?;
        Ok(())
    }

    /// `GameClient::xfer` (version 3) with `xferDrawableTOC` (version 1); each drawable's own data is read past (module
    /// docs).
    fn xfer_game_client(&mut self, xfer: &mut dyn Xfer) -> Result<(), SaveError> {
        let current: XferVersion = 3;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        xfer.xfer_unsigned_int(&mut self.client.game_client.frame)?;
        // xferDrawableTOC
        let mut toc_version: XferVersion = 1;
        xfer.xfer_version(&mut toc_version, 1)?;
        let saving = xfer.get_xfer_mode() == XferMode::Save;
        let gc = &mut self.client.game_client;
        if saving {
            // (no drawable is saved: an empty table)
            let mut toc_count = 0u32;
            xfer.xfer_unsigned_int(&mut toc_count)?;
        } else {
            gc.toc.clear();
            let mut toc_count = 0u32;
            xfer.xfer_unsigned_int(&mut toc_count)?;
            for _ in 0..toc_count {
                let mut name = Vec::new();
                xfer.xfer_ascii_string(&mut name)?;
                let mut id = 0u16;
                xfer.xfer_unsigned_short(&mut id)?;
                gc.toc.push((name, id));
            }
        }
        let mut drawable_count = 0u16;
        xfer.xfer_unsigned_short(&mut drawable_count)?;
        if !saving {
            gc.drawables_read_past.clear();
            for _ in 0..drawable_count {
                let mut toc_id = 0u16;
                xfer.xfer_unsigned_short(&mut toc_id)?;
                let Some(name) = gc.toc.iter().find(|(_, id)| *id == toc_id).map(|(n, _)| n.clone()) else {
                    return Err(SaveError::InvalidData);
                };
                let size = xfer.begin_block()?;
                if self.world.statics.thing_factory.find_template(&name).is_none() {
                    // ("Unrecognized thing template ..., skipping")
                    xfer.skip(size)?;
                    continue;
                }
                let mut object = INVALID_ID;
                xfer.xfer_object_id(&mut object)?;
                if object != INVALID_ID && self.world.find_object(object).is_none() {
                    return Err(SaveError::InvalidData);
                }
                // `xferSnapshot(draw)`: `Drawable::xfer` is not translated; its data is read past
                xfer.skip(size - 4)?;
                gc.drawables_read_past.push((name, object));
            }
            if drawable_count > 0 {
                self.client.read_past.push(format!("CHUNK_GameClient: {drawable_count} drawables' own data (Drawable::xfer)"));
            }
        }
        let gc = &mut self.client.game_client;
        if version >= 2 {
            if saving {
                let mut n = gc.briefing.len() as i32;
                xfer.xfer_int(&mut n)?;
                for b in gc.briefing.iter_mut() {
                    xfer.xfer_ascii_string(b)?;
                }
            } else {
                let mut n = 0i32;
                xfer.xfer_int(&mut n)?;
                gc.briefing.clear();
                while n > 0 {
                    n -= 1;
                    let mut s = Vec::new();
                    xfer.xfer_ascii_string(&mut s)?;
                    gc.briefing.push(s);
                }
            }
        }
        Ok(())
    }

    /// `InGameUI::xfer` (version 3).
    fn xfer_in_game_ui(&mut self, xfer: &mut dyn Xfer) -> Result<(), SaveError> {
        let current: XferVersion = 3;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        let saving = xfer.get_xfer_mode() == XferMode::Save;
        let ui = &mut self.client.in_game_ui;
        if version >= 2 {
            xfer.xfer_int(&mut ui.named_timer_last_flash_frame)?;
            xfer.xfer_bool(&mut ui.named_timer_used_flash_color)?;
            xfer.xfer_bool(&mut ui.show_named_timers)?;
            if saving {
                let mut count = ui.named_timers.len() as i32;
                xfer.xfer_int(&mut count)?;
                for t in ui.named_timers.iter_mut() {
                    xfer.xfer_ascii_string(&mut t.name)?;
                    xfer.xfer_unicode_string(&mut t.text)?;
                    xfer.xfer_bool(&mut t.is_countdown)?;
                }
            } else {
                let mut count = 0i32;
                xfer.xfer_int(&mut count)?;
                ui.named_timers.clear();
                for _ in 0..count {
                    let mut t = NamedTimerSave::default();
                    xfer.xfer_ascii_string(&mut t.name)?;
                    xfer.xfer_unicode_string(&mut t.text)?;
                    xfer.xfer_bool(&mut t.is_countdown)?;
                    // (`addNamedTimer`: a map by name, a later one of the same name replacing it)
                    ui.named_timers.retain(|o| o.name != t.name);
                    ui.named_timers.push(t);
                }
            }
        }
        xfer.xfer_bool(&mut ui.superweapon_hidden_by_script)?;
        if saving {
            for s in ui.superweapons.iter_mut() {
                xfer.xfer_int(&mut s.player_index)?;
                xfer.xfer_ascii_string(&mut s.template_name)?;
                xfer.xfer_ascii_string(&mut s.power_name)?;
                xfer.xfer_object_id(&mut s.id)?;
                xfer.xfer_unsigned_int(&mut s.timestamp)?;
                xfer.xfer_bool(&mut s.hidden_by_script)?;
                xfer.xfer_bool(&mut s.hidden_by_science)?;
                xfer.xfer_bool(&mut s.ready)?;
                // (`currentVersion >= 3`, always)
                xfer.xfer_bool(&mut s.eva_ready_played)?;
            }
            let mut no_more_players = -1i32;
            xfer.xfer_int(&mut no_more_players)?;
        } else {
            ui.superweapons.clear();
            loop {
                let mut player_index = 0i32;
                xfer.xfer_int(&mut player_index)?;
                if player_index == -1 {
                    break;
                }
                if !(0..MAX_PLAYER_COUNT).contains(&player_index) {
                    // ("SWInfo bad plyrindex", INI_INVALID_DATA)
                    return Err(SaveError::InvalidData);
                }
                let mut s = SuperweaponSave { player_index, ..Default::default() };
                xfer.xfer_ascii_string(&mut s.template_name)?;
                if self.world.statics.special_power_store.find_special_power_template(&s.template_name).is_none() {
                    // ("power %s not found", INI_INVALID_DATA)
                    return Err(SaveError::InvalidData);
                }
                let ui = &mut self.client.in_game_ui;
                xfer.xfer_ascii_string(&mut s.power_name)?;
                xfer.xfer_object_id(&mut s.id)?;
                xfer.xfer_unsigned_int(&mut s.timestamp)?;
                xfer.xfer_bool(&mut s.hidden_by_script)?;
                xfer.xfer_bool(&mut s.hidden_by_science)?;
                xfer.xfer_bool(&mut s.ready)?;
                // (upstream tests `currentVersion >= 3`, not the save's version: a version 2 save's flag is read too)
                xfer.xfer_bool(&mut s.eva_ready_played)?;
                ui.superweapons.push(s);
            }
        }
        Ok(())
    }

    /// `ParticleSystemManager::xfer` (version 1): the unique system id counter, the system count; the systems are read
    /// past (module docs), and a save writes none.
    fn xfer_particle_systems(&mut self, xfer: &mut dyn Xfer) -> Result<(), SaveError> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)?;
        let mut id = self.particles.unique_system_id.to_le_bytes();
        xfer.xfer_user(&mut id)?;
        self.particles.unique_system_id = u32::from_le_bytes(id);
        let mut count = 0u32;
        xfer.xfer_unsigned_int(&mut count)?;
        if xfer.get_xfer_mode() == XferMode::Load && count > 0 {
            self.read_past_rest(xfer, format!("CHUNK_ParticleSystem: {count} particle systems (ParticleSystem::xfer)"))?;
        }
        Ok(())
    }

    /// `W3DGhostObjectManager::xfer` (version 1) with `GhostObjectManager::xfer` (version 1): the local player and the
    /// ghost object count; the ghost objects are read past (module docs), and a save writes none.
    fn xfer_ghost_objects(&mut self, xfer: &mut dyn Xfer) -> Result<(), SaveError> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)?;
        let mut base: XferVersion = 1;
        xfer.xfer_version(&mut base, 1)?;
        if xfer.get_xfer_mode() == XferMode::Save {
            self.client.ghost_local_player = self.world.players.local_index().unwrap_or(0);
        }
        xfer.xfer_int(&mut self.client.ghost_local_player)?;
        let mut count = 0u16;
        xfer.xfer_unsigned_short(&mut count)?;
        if xfer.get_xfer_mode() == XferMode::Load && count > 0 {
            self.read_past_rest(xfer, format!("CHUNK_GhostObject: {count} ghost objects (W3DGhostObject::xfer)"))?;
        }
        Ok(())
    }
}

/// `xferRadarObjectList(xfer, head)` (version 1) with `RadarObject::xfer` (version 1) for each.
fn xfer_radar_object_list(xfer: &mut dyn Xfer, list: &mut Vec<RadarObject>, world: &mut World) -> Result<(), SaveError> {
    let mut version: XferVersion = 1;
    xfer.xfer_version(&mut version, 1)?;
    let mut count = list.len() as u16;
    xfer.xfer_unsigned_short(&mut count)?;
    if xfer.get_xfer_mode() == XferMode::Save {
        for o in list.iter_mut() {
            radar_object_xfer(xfer, o, world)?;
        }
    } else {
        // "the list should be empty at this point", or hold only destroyed objects (an id the World no longer has counts
        // as one: the port's entries are ids, upstream's pointers)
        if list.iter().any(|o| world.find_object(o.object).is_some_and(|x| !x.is_destroyed())) {
            return Err(SaveError::InvalidData);
        }
        for _ in 0..count {
            // (`newInstance(RadarObject)`, linked at the end)
            let mut o = RadarObject { object: INVALID_ID, color: game_make_color(255, 255, 255, 255) };
            radar_object_xfer(xfer, &mut o, world)?;
            list.push(o);
        }
    }
    Ok(())
}

/// `RadarObject::xfer` (version 1): the object's id (on a load the object must exist, and is given its radar data back:
/// `friend_setRadarData`, the World's `on_radar`) and the colour.
fn radar_object_xfer(xfer: &mut dyn Xfer, o: &mut RadarObject, world: &mut World) -> Result<(), SaveError> {
    let mut version: XferVersion = 1;
    xfer.xfer_version(&mut version, 1)?;
    let mut id = if world.find_object(o.object).is_some() { o.object } else { INVALID_ID };
    xfer.xfer_object_id(&mut id)?;
    if xfer.get_xfer_mode() == XferMode::Load {
        let Some(obj) = world.objects.find_mut(id) else {
            return Err(SaveError::InvalidData);
        };
        obj.on_radar = true;
        o.object = id;
    }
    xfer.xfer_color(&mut o.color)?;
    Ok(())
}

impl GameStateHost<SaveBlock> for SaveHost<'_> {
    fn xfer_block(&mut self, block: SaveBlock, gs: &mut GameState<SaveBlock>, xfer: &mut dyn Xfer) -> XferResult<()> {
        let r = match block {
            // (the save information with the client's clock, see the module docs)
            SaveBlock::World(WorldBlock::GameState) => return gs.xfer_info(xfer, self),
            SaveBlock::World(WorldBlock::TerrainVisual) => match self.world.xfer_terrain_visual_part(xfer) {
                Ok(true) if xfer.get_xfer_mode() == XferMode::Load => {
                    return self.read_past_rest(xfer, "CHUNK_TerrainVisual: the trees and props (BaseHeightMapRenderObjClass::xfer)".to_string());
                }
                Ok(_) => Ok(()),
                Err(e) => Err(("W3DTerrainVisual::xfer", e)),
            },
            SaveBlock::World(b) => {
                std::mem::swap(&mut self.shadow.info, &mut gs.info);
                self.shadow.is_in_load_game = gs.is_in_load_game;
                let starting = self.world.save_side.new_game_on_load;
                let r = <World as GameStateHost<WorldBlock>>::xfer_block(self.world, b, &mut self.shadow, xfer);
                std::mem::swap(&mut self.shadow.info, &mut gs.info);
                for p in std::mem::take(&mut self.shadow.post_process) {
                    gs.add_post_process(SaveBlock::World(p));
                }
                if r.is_ok() && starting && !self.world.save_side.new_game_on_load {
                    // GitHub #58: `CHUNK_GameStateMap` started the save's game (`startNewGame(TRUE)`), whose
                    // `TheRadar->newMap(TheTerrainLogic)` is the client's: the radar takes the new map's extent and
                    // heights before its own block is read (its terrain texture is rebuilt by its `loadPostProcess`)
                    let host = crate::client::radar_world::WorldRadarHost {
                        world: &mut *self.world,
                        drawables: None,
                        colors: None,
                        water_color: crate::client::radar_world::UNTEXTURED_TERRAIN_COLOR,
                    };
                    self.radar.new_map(&host);
                }
                return r;
            }
            SaveBlock::Radar => self.xfer_radar(xfer).map_err(|e| ("Radar::xfer", e)),
            SaveBlock::TacticalView => self.xfer_view(xfer).map_err(|e| ("View::xfer", e)),
            SaveBlock::GameClient => self.xfer_game_client(xfer).map_err(|e| ("GameClient::xfer", e)),
            SaveBlock::InGameUi => self.xfer_in_game_ui(xfer).map_err(|e| ("InGameUI::xfer", e)),
            SaveBlock::ParticleSystem => self.xfer_particle_systems(xfer).map_err(|e| ("ParticleSystemManager::xfer", e)),
            SaveBlock::GhostObject => self.xfer_ghost_objects(xfer).map_err(|e| ("W3DGhostObjectManager::xfer", e)),
        };
        r.map_err(|(call, e)| self.block_error(call, e))
    }

    fn load_post_process(&mut self, block: SaveBlock) {
        match block {
            SaveBlock::World(b) => <World as GameStateHost<WorldBlock>>::load_post_process(self.world, b),
            // `Radar::loadPostProcess`: `refreshTerrain(TheTerrainLogic)`
            SaveBlock::Radar => self.radar.refresh_terrain(),
            // (`GameClient::loadPostProcess` pulls the drawable id counter down to the drawables loaded: none are loaded here,
            // so the counter `CHUNK_GameStateMap` read stays; the other blocks' post-processes are empty)
            SaveBlock::GameClient | SaveBlock::TacticalView | SaveBlock::InGameUi | SaveBlock::ParticleSystem | SaveBlock::GhostObject => {}
        }
    }
    fn partition_update(&mut self) {
        <World as GameStateHost<WorldBlock>>::partition_update(self.world)
    }
    fn user_data_path(&self) -> Vec<u8> {
        <World as GameStateHost<WorldBlock>>::user_data_path(self.world)
    }
    fn map_name(&self) -> Vec<u8> {
        <World as GameStateHost<WorldBlock>>::map_name(self.world)
    }
    fn local_time(&mut self) -> [u16; 8] {
        self.client.local_time
    }
    fn world_dict_map_name(&self) -> Option<Vec<u8>> {
        <World as GameStateHost<WorldBlock>>::world_dict_map_name(self.world)
    }
    fn current_campaign(&self) -> Option<(Vec<u8>, i32)> {
        <World as GameStateHost<WorldBlock>>::current_campaign(self.world)
    }
    fn file_exists(&self, path: &[u8]) -> bool {
        <World as GameStateHost<WorldBlock>>::file_exists(self.world, path)
    }
}

/// `GameState::saveGame(filename, desc, SAVE_FILE_TYPE_NORMAL)`'s data: the description set, the file type normal (no
/// mission map), every block of the whole file written. The file's name and where it goes are the caller's.
pub fn save_client_game(host: &mut SaveHost<'_>, description: &[u16]) -> XferResult<Vec<u8>> {
    let mut gs = client_game_state();
    gs.info.description = description.to_vec();
    gs.info.save_file_type = SAVE_FILE_TYPE_NORMAL;
    gs.info.mission_map_name.clear();
    let mut x = XferSave::new();
    x.open(b"save game")?;
    gs.xfer_save_data(&mut x, SnapshotType::SaveLoad, host)?;
    x.close()?;
    Ok(x.take_bytes())
}

/// What a load read.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadOutcome {
    pub info: SaveGameInfo,
    pub blocks: Vec<BlockSeen>,
}

/// `GameState::loadGame`'s data: every block of the file loaded into a World made from the save's map (the caller's, as
/// for `World::load_game`), the radar, the particle systems' manager and the client's state, then post-processed in load
/// order. The client events the load made are dropped (module docs). A failure says where ([`LoadFailure`]).
pub fn load_client_game(host: &mut SaveHost<'_>, bytes: Vec<u8>) -> Result<LoadOutcome, LoadFailure> {
    let mut gs = client_game_state();
    let mut x = TracingLoad::new(bytes);
    host.trace = Some(x.trace.clone());
    let fail = |x: &TracingLoad, host: &SaveHost<'_>, e: XferStatus| {
        let mut errors = host.client.errors.clone();
        errors.extend(host.world.cuts.team_live_errors.iter().cloned());
        failure(&x.trace.borrow(), e, errors)
    };
    if let Err(e) = x.open(b"save game") {
        return Err(fail(&x, host, e));
    }
    // (`LatchRestore<Bool> inLoadGame(m_isInLoadGame, TRUE)`)
    gs.is_in_load_game = true;
    let r = gs.xfer_save_data(&mut x, SnapshotType::SaveLoad, host);
    gs.is_in_load_game = false;
    host.trace = None;
    if let Err(e) = r {
        return Err(fail(&x, host, e));
    }
    let _ = x.close();
    gs.game_state_post_process_load(host);
    // (the radar and drawable calls of the objects the load made: see the module docs)
    host.world.take_client_events();
    if !host.client.errors.is_empty() || !host.world.cuts.team_live_errors.is_empty() {
        return Err(fail(&x, host, XferStatus::ErrorUnknown));
    }
    let blocks = x.trace.borrow().blocks.clone();
    Ok(LoadOutcome { info: gs.info, blocks })
}

/// What a save file says about itself, read without a game: its information (`CHUNK_GameState`), its map
/// (`CHUNK_GameStateMap`'s names, game mode and the embedded map file) and its blocks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SaveSummary {
    pub info: SaveGameInfo,
    /// The map's portable name in the save directory (`save\<leaf>`) and the pristine map's (`maps\<dir>\<leaf>`).
    pub save_map: Vec<u8>,
    pub pristine_map: Vec<u8>,
    pub game_mode: i32,
    /// The embedded map file.
    pub map_file: Vec<u8>,
    pub blocks: Vec<BlockSeen>,
}

/// A host for reading the save information alone (its xfer takes the clock and the map label from the host before
/// reading them over).
struct NoGame;

impl GameStateHost<()> for NoGame {
    fn xfer_block(&mut self, _: (), _: &mut GameState<()>, _: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }
    fn load_post_process(&mut self, _: ()) {}
    fn partition_update(&mut self) {}
    fn user_data_path(&self) -> Vec<u8> {
        Vec::new()
    }
    fn map_name(&self) -> Vec<u8> {
        Vec::new()
    }
    fn local_time(&mut self) -> [u16; 8] {
        [0; 8]
    }
    fn world_dict_map_name(&self) -> Option<Vec<u8>> {
        None
    }
    fn current_campaign(&self) -> Option<(Vec<u8>, i32)> {
        None
    }
    fn file_exists(&self, _: &[u8]) -> bool {
        false
    }
}

/// [`SaveSummary`] of a save file: its blocks walked by their sizes, `CHUNK_GameState` read by `GameState::xfer`, and the
/// start of `CHUNK_GameStateMap` by `GameStateMap::xfer`'s reads (version, the two names, the game mode, the embedded
/// map's block).
pub fn read_save_summary(bytes: Vec<u8>) -> Result<SaveSummary, LoadFailure> {
    let mut x = TracingLoad::new(bytes);
    let mut out = SaveSummary::default();
    let r: XferResult<()> = (|| {
        x.open(b"save game")?;
        loop {
            let mut token = Vec::new();
            x.xfer_ascii_string(&mut token)?;
            if token.eq_ignore_ascii_case(crate::common::game_state::SAVE_FILE_EOF) {
                return Ok(());
            }
            let size = x.begin_block()?;
            let end = x.trace.borrow().open.first().map_or(0, |b| b.1);
            if token == b"CHUNK_GameState" {
                let mut gs = GameState::<()>::new();
                gs.xfer_info(&mut x, &mut NoGame)?;
                out.info = gs.info;
            } else if token == b"CHUNK_GameStateMap" {
                let mut version: XferVersion = 2;
                x.xfer_version(&mut version, 2)?;
                x.xfer_ascii_string(&mut out.save_map)?;
                x.xfer_ascii_string(&mut out.pristine_map)?;
                x.xfer_int(&mut out.game_mode)?;
                let n = x.begin_block()?;
                let mut map = vec![0u8; n.max(0) as usize];
                x.xfer_user(&mut map)?;
                x.end_block()?;
                out.map_file = map;
            } else {
                x.skip(size)?;
                continue;
            }
            let pos = x.trace.borrow().pos;
            if end > pos {
                x.skip((end - pos) as i32)?;
            }
            x.end_block()?;
        }
    })();
    out.blocks = x.trace.borrow().blocks.clone();
    match r {
        Ok(()) => Ok(out),
        Err(e) => Err(failure(&x.trace.borrow(), e, Vec::new())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::world::game_logic_update::CRC_RECALC;
    use crate::logic::world::gate_scenario::{gate_map_world, gate_world};

    fn step(w: &mut World) -> u32 {
        w.logic_update().expect("the frame");
        w.logic_crc(CRC_RECALC, None).expect("the CRC")
    }

    /// The client's parts of a game: a radar (new map, fed the frame's events), a particle manager and the saved state.
    struct Client {
        world: World,
        radar: crate::client::radar_world::ClientRadar,
        particles: ParticleSystemManager,
        state: ClientSaveState,
    }

    impl Client {
        fn new(mut world: World) -> Self {
            let mut radar = crate::client::radar_world::ClientRadar::default();
            radar.radar.force_on(true);
            radar.new_map(&mut world);
            Client { world, radar, particles: ParticleSystemManager::new(), state: ClientSaveState::default() }
        }
        fn step(&mut self) -> u32 {
            let c = step(&mut self.world);
            let ev = self.world.take_client_events();
            self.radar.take_events(&mut self.world, &ev);
            c
        }
        fn save(&mut self, description: &str) -> Vec<u8> {
            self.state.game_client.frame = self.world.frame();
            let d: Vec<u16> = description.encode_utf16().collect();
            let mut h = SaveHost::new(&mut self.world, &mut self.radar.radar, &mut self.particles, &mut self.state);
            let r = save_client_game(&mut h, &d);
            r.unwrap_or_else(|e| panic!("the save: {e:?} {:?} {:?}", self.world.cuts.team_live_errors, self.state.errors))
        }
        fn load(bytes: Vec<u8>) -> Result<(Client, LoadOutcome), LoadFailure> {
            Client::load_into(gate_map_world(), bytes)
        }
        fn load_into(world: World, bytes: Vec<u8>) -> Result<(Client, LoadOutcome), LoadFailure> {
            let mut c = Client::new(world);
            let mut h = SaveHost::new(&mut c.world, &mut c.radar.radar, &mut c.particles, &mut c.state);
            let o = load_client_game(&mut h, bytes)?;
            Ok((c, o))
        }
    }

    /// The blocks of a save file by name (top level).
    fn blocks(bytes: &[u8]) -> std::collections::BTreeMap<String, Vec<u8>> {
        let s = read_save_summary(bytes.to_vec()).expect("the summary");
        s.blocks.iter().map(|b| (String::from_utf8_lossy(&b.name).into_owned(), bytes[b.offset..b.offset + b.size as usize].to_vec())).collect()
    }

    /// GitHub #4: the whole save file through the client's path. The gate game (with radar-visible units of its own: a
    /// `RadarPriority` template, two of player 1's and one of player 2's) runs 120 frames with a client radar attached and
    /// is saved through [`save_client_game`]; the save is loaded into two fresh clients ([`load_client_game`]) and into a
    /// World alone (`World::load_game`, which skips the client's blocks by their size).
    ///
    /// **Predicted before the run:** (1) the save has upstream's sixteen registered blocks in `GameState::init`'s order,
    /// `CHUNK_Campaign` excepted (~~sixteen~~ **corrected 2026-11-06 (GitHub #58):** all seventeen, `CHUNK_Campaign` now
    /// registered); (2) the two client loads give the same logic CRC at each of the next 150 frames, and the
    /// World loaded alone gives the same CRCs as they do (the client's blocks change nothing the logic computes); (3) the
    /// client blocks round-trip: a loaded client saved again writes the same `CHUNK_Radar`, `CHUNK_TacticalView`,
    /// `CHUNK_GameClient`, `CHUNK_InGameUI`, `CHUNK_ParticleSystem`, `CHUNK_TerrainVisual` and `CHUNK_GhostObject` bytes,
    /// and its radar lists name the three units, with the World's `on_radar` set on each; (4) the World's logic blocks in
    /// the client's save equal the World's own save's at the same frame; (5) the summary reads the description back.
    ///
    /// **Results (2026-11-06, native, release):** all five held, on the third run. **Misses, both the fixture's:** the first
    /// load failed in `CHUNK_Radar` (`SC_INVALID_DATA`, object 15 not found): the loads were into `gate_map_world`, which
    /// lacks the test's `Blip` template, so the logic's load skipped the three blips (as upstream skips an object of an unknown
    /// template) and the radar named objects that were not there; the second run's `CHUNK_GhostObject` did not round-trip
    /// (local player 0, was 1): the map world had no local player, which the game start sets and no block saves. Both fixed
    /// in the fixture (the same data and the game start's local player). The radar listed objects 15, 14 and 16 before the
    /// save and after the load; the three World loads agreed at every one of the 150 frames.
    #[test]
    fn a_client_save_loads_and_runs_on() {
        use crate::logic::world::gate_scenario::make;
        use crate::logic::world::player_list::player_list_set_local_player;
        use crate::logic::world::team_ops::factory_find_team;
        let extra = "Object Blip\n KindOf = VEHICLE\n RadarPriority = UNIT\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\nEnd\n";
        let mk = || {
            let mut w = crate::logic::world::gate_scenario::gate_world_with(extra);
            player_list_set_local_player(&mut w, Some(1)).expect("the local player");
            w
        };
        let mut a = Client::new(mk());
        let t1 = factory_find_team(&mut a.world, b"team1").expect("the team").expect("team1");
        let t2 = factory_find_team(&mut a.world, b"team2").expect("the team").expect("team2");
        let blips = [make(&mut a.world, b"Blip", t1, Coord3D { x: 50.0, y: 50.0, z: 0.0 }), make(&mut a.world, b"Blip", t1, Coord3D { x: 70.0, y: 50.0, z: 0.0 }), make(&mut a.world, b"Blip", t2, Coord3D { x: 300.0, y: 300.0, z: 0.0 })];
        a.state.view = TacticalViewSave { angle: 0.5, position: Coord3D { x: 100.0, y: 120.0, z: 0.0 } };
        a.state.local_time = [2026, 11, 5, 6, 12, 30, 15, 250];
        for _ in 0..120 {
            a.step();
        }
        let listed: Vec<ObjectId> = a.radar.radar.local_object_list.iter().chain(a.radar.radar.object_list.iter()).map(|o| o.object).collect();
        eprintln!("radar lists before the save: {listed:?}");
        let bytes = a.save("gate at 120");

        // (1)
        let names: Vec<String> = read_save_summary(bytes.clone()).expect("summary").blocks.iter().map(|b| String::from_utf8_lossy(&b.name).into_owned()).collect();
        let expected: Vec<String> = client_game_state().blocks[0].iter().map(|b| String::from_utf8_lossy(&b.block_name).into_owned()).collect();
        assert_eq!(names, expected);
        assert_eq!(names.len(), 17);
        // (5)
        let s = read_save_summary(bytes.clone()).expect("summary");
        assert_eq!(String::from_utf16_lossy(&s.info.description), "gate at 120");
        assert_eq!(s.info.date.year, 2026);
        assert!(!s.map_file.is_empty());
        // (4)
        let own = blocks(&a.world.save_game().expect("the World's save"));
        let mine = blocks(&bytes);
        for name in ["CHUNK_Campaign", "CHUNK_GameStateMap", "CHUNK_TerrainLogic", "CHUNK_TeamFactory", "CHUNK_Players", "CHUNK_GameLogic", "CHUNK_ScriptEngine", "CHUNK_SidesList", "CHUNK_Partition", "CHUNK_TerrainVisual"] {
            assert!(own[name] == mine[name], "{name}: the client's save holds the World's block");
        }

        // (2)
        // (each into a World made from the same map and the same data: the test's own template too; a load skips an object
        // whose template the World does not have, as upstream;
        // and with the local player its game start set: the save does not hold it, `PlayerList::xfer` does not write it)
        let map_world = || {
            let mut w = crate::logic::world::gate_scenario::gate_map_world_with(extra);
            player_list_set_local_player(&mut w, Some(1)).expect("the local player");
            w
        };
        let (mut b, ob) = Client::load_into(map_world(), bytes.clone()).unwrap_or_else(|e| panic!("the load: {e}"));
        let (mut c, _) = Client::load_into(map_world(), bytes.clone()).unwrap_or_else(|e| panic!("the load: {e}"));
        let mut d = map_world();
        d.load_game(bytes.clone()).unwrap_or_else(|e| panic!("the World's load: {e:?} {:?}", d.cuts.team_live_errors));
        assert_eq!(String::from_utf16_lossy(&ob.info.description), "gate at 120");
        // (3)
        let again = b.save("again");
        let back = blocks(&again);
        for name in ["CHUNK_Radar", "CHUNK_TacticalView", "CHUNK_GameClient", "CHUNK_InGameUI", "CHUNK_ParticleSystem", "CHUNK_TerrainVisual", "CHUNK_GhostObject"] {
            assert!(back[name] == mine[name], "{name} round-trips");
        }
        assert_eq!(b.state.view, a.state.view);
        let relisted: Vec<ObjectId> = b.radar.radar.local_object_list.iter().chain(b.radar.radar.object_list.iter()).map(|o| o.object).collect();
        assert_eq!(relisted, listed);
        for id in blips {
            assert!(relisted.contains(&id), "{id} on the radar");
            assert!(b.world.find_object(id).is_some_and(|o| o.on_radar), "{id} has its radar data back");
        }
        assert!(b.radar.radar.terrain_refresh_wanted, "the radar's post-process refreshes its terrain");
        for f in 0..150 {
            let (cb, cc, cd) = (b.step(), c.step(), step(&mut d));
            assert_eq!(cb, cc, "two client loads part at frame {f} after the load");
            assert_eq!(cb, cd, "the client's load and the World's part at frame {f} after the load");
        }
        for w in [&a.world, &b.world, &c.world, &d] {
            assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        }
        assert!(b.state.read_past.is_empty(), "{:?}", b.state.read_past);
    }

    /// GitHub #4: a client block that does not load reports where. Predicted: a save whose `CHUNK_Radar` names an object
    /// the World does not have (the id overwritten) fails with `SC_INVALID_DATA` (`XFER_UNKNOWN_ERROR` to the xfer), in
    /// `CHUNK_Radar`, with the object id the last read. **Result (2026-11-06):** as predicted, first run (`ErrorUnknown in
    /// CHUNK_Radar at byte 11 of the block`, reading the four bytes `3f 42 0f 00`).
    #[test]
    fn a_bad_client_block_reports_its_place() {
        let mut a = Client::new(gate_world());
        for _ in 0..5 {
            a.step();
        }
        // one radar object made by hand: an object that exists
        let id = a.world.objects.ids_in_order()[0];
        a.radar.radar.local_object_list.push(RadarObject { object: id, color: 7 });
        let mut bytes = a.save("bad");
        let s = read_save_summary(bytes.clone()).expect("summary");
        let radar = s.blocks.iter().find(|b| b.name == b"CHUNK_Radar").expect("the radar's block");
        // version 1, two flags, the list's version and count, the object's version: the id follows
        let at = radar.offset + 1 + 2 + 1 + 2 + 1;
        assert_eq!(i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()), id);
        bytes[at..at + 4].copy_from_slice(&999_999i32.to_le_bytes());
        let e = Client::load(bytes).err().expect("the load fails");
        eprintln!("{e}");
        assert_eq!(e.block, "CHUNK_Radar");
        assert_eq!(e.error, XferStatus::ErrorUnknown);
        assert_eq!(e.block_offset, 1 + 2 + 1 + 2 + 1 + 4);
        assert!(e.last_reads.last().is_some_and(|r| r.contains("4 bytes [3f 42 0f 00]")), "{:?}", e.last_reads);
        assert!(e.errors.iter().any(|x| x.contains("Radar::xfer") && x.contains("InvalidData")), "{:?}", e.errors);
    }

    /// GitHub #4: an original game's save (`RTS_ZH_SAVE=<file.sav>`), with its install (`RTS_ZH_DIR=<Zero Hour folder>`).
    /// Skipped without them (no EA file is in this repository). The summary is read (its blocks, its information, its
    /// map); with the install, ~~a skirmish is started on the save's map and~~ the save loaded through the client's path
    /// into the install's World with no game, the save starting its own (GitHub #58); the
    /// first block, offset and field that does not load is reported, or what was read past. **Not run (2026-11-06):** no
    /// original save is in this repository or on this machine, so the test was skipped (it passes by skipping).
    #[test]
    fn an_original_save_loads_or_reports_its_first_difference() {
        let Some(path) = std::env::var_os("RTS_ZH_SAVE") else {
            eprintln!("skipped: RTS_ZH_SAVE is not set (an original game's .sav)");
            return;
        };
        let bytes = std::fs::read(&path).expect("the save file");
        let summary = match read_save_summary(bytes.clone()) {
            Ok(s) => s,
            Err(e) => panic!("the save's block list does not read: {e}\n{:#?}", e.last_reads),
        };
        eprintln!(
            "save: {:?} on {:?}, side {:?}, mode {}, map {:?} ({} bytes); blocks: {:?}",
            String::from_utf16_lossy(&summary.info.description),
            String::from_utf8_lossy(&summary.info.map_label),
            String::from_utf8_lossy(&summary.info.campaign_side),
            summary.game_mode,
            String::from_utf8_lossy(&summary.pristine_map),
            summary.map_file.len(),
            summary.blocks.iter().map(|b| format!("{}:{}", String::from_utf8_lossy(&b.name), b.size)).collect::<Vec<_>>()
        );
        let Some(dir) = std::env::var_os("RTS_ZH_DIR") else {
            eprintln!("the load skipped: RTS_ZH_DIR is not set (the install the save's game ran on)");
            return;
        };
        use crate::common::file_system::{FileSystem, NativeDevice};
        let mut fs = FileSystem::new(NativeDevice::new(std::path::Path::new(&dir)));
        fs.init(None);
        let (mut world, _) = World::from_install(&fs, crate::logic::world::install_world::DEFAULT_LANGUAGE).expect("the install");
        let map = summary.pristine_map.clone();
        let real = GameState::<WorldBlock>::new().portable_map_path_to_real(&map, &world).expect("the map's path");
        world.mount_map(&fs, &real);
        if !world.save_side.disk.contains_key(&real.to_ascii_lowercase()) {
            world.save_side.disk.insert(real.to_ascii_lowercase(), summary.map_file.clone());
        }
        world.save_side.new_game_on_load = true;
        let mut c = Client::new(world);
        let mut h = SaveHost::new(&mut c.world, &mut c.radar.radar, &mut c.particles, &mut c.state);
        match load_client_game(&mut h, bytes) {
            Ok(o) => {
                eprintln!("loaded; read past: {:#?}", c.state.read_past);
                for _ in 0..30 {
                    c.step();
                }
                eprintln!("ran 30 frames from {:?}", o.info.map_label);
            }
            Err(e) => panic!("the first difference: {e}\nlast reads:\n{}", e.last_reads.join("\n")),
        }
    }


    /// GitHub #58: a four-player game's save through the client's path. The install's four-player game
    /// (`install_world`'s `four_player_game`: the human with China at the third start, America, GLA and China computer
    /// players, two teams) runs 60 frames, then 60 with a client radar attached, and is saved through
    /// [`save_client_game`]; it is loaded through [`load_client_game`] into two copies of the install's World with no game
    /// (`SaveSide::new_game_on_load`), each in a fresh client whose radar was given the empty World's map.
    ///
    /// **Predicted before the run:** each load starts the save's game in `CHUNK_GameStateMap` and the host makes the radar's
    /// new map then: the loaded radar's map extent is the saving radar's (300 x 300), not the empty World's; the radar lists
    /// name the same objects; the player list, sides and local player are the original's; the two loads give the same logic
    /// CRC at each of the next 150 frames; nothing is read past and no error is recorded.
    ///
    /// **Results (2026-11-06, native, release):** all held, first run (extent (0, 0) to (300, 300) on both). **Ablation:**
    /// with the host's radar new map taken out, the loaded radar's extent is the empty World's ((0, 0) to (0, 0)) and the
    /// test fails, so it checks the call.
    #[test]
    fn a_four_player_client_save_starts_its_own_game() {
        use crate::logic::world::install_world::tests::{four_player_game, player_sides};
        let (base, w) = four_player_game(60);
        let mut a = Client::new(w);
        for _ in 0..60 {
            a.step();
        }
        let bytes = a.save("four at 120");
        let load = || {
            let mut w = base.clone();
            w.save_side.new_game_on_load = true;
            Client::load_into(w, bytes.clone()).unwrap_or_else(|e| panic!("the load: {e}\n{}", e.last_reads.join("\n")))
        };
        let ((mut b, _), (mut c, _)) = (load(), load());
        let ext = |c: &Client| c.radar.radar.map_extent;
        eprintln!("radar extent: saved {:?}, loaded {:?}", ext(&a), ext(&b));
        assert_eq!(ext(&b), ext(&a));
        assert!(ext(&a).hi.x > ext(&a).lo.x);
        let listed = |c: &Client| c.radar.radar.local_object_list.iter().chain(c.radar.radar.object_list.iter()).map(|o| o.object).collect::<Vec<_>>();
        assert_eq!(listed(&b), listed(&a));
        assert_eq!(player_sides(&b.world), player_sides(&a.world));
        assert_eq!(b.world.players.local_index(), a.world.players.local_index());
        for f in 0..150 {
            assert_eq!(b.step(), c.step(), "two client loads part at frame {f} after the load");
        }
        for w in [&a.world, &b.world, &c.world] {
            assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        }
        assert!(b.state.read_past.is_empty(), "{:?}", b.state.read_past);
        assert!(b.state.errors.is_empty(), "{:?}", b.state.errors);
    }


    /// GitHub #58: a campaign's save fails cleanly. The gate game's client save has its `CHUNK_Campaign` rewritten to name
    /// a campaign (`usa`, mission `m1`; the block's size fixed). **Predicted before the run:** the load fails in
    /// `CHUNK_Campaign` with `XFER_UNKNOWN_ERROR` and the recorded `CampaignManager::xfer: InvalidData` (the port has no
    /// campaigns for `setCampaignAndMission`), and no later block is read. **Result (2026-11-06):** as predicted, first run
    /// (`ErrorUnknown in CHUNK_Campaign at byte 17 of the block`; `CampaignManager::xfer: InvalidData`).
    #[test]
    fn a_campaign_save_fails_cleanly() {
        let mut a = Client::new(gate_world());
        for _ in 0..5 {
            a.step();
        }
        let bytes = a.save("campaign");
        let s = read_save_summary(bytes.clone()).expect("summary");
        let b = s.blocks.iter().find(|b| b.name == b"CHUNK_Campaign").expect("the campaign's block");
        let (start, end) = (b.offset, b.offset + b.size as usize);
        let old = &bytes[start..end];
        // version, then the two empty names (each a length byte of 0), then the rest as it was
        assert_eq!(&old[1..3], &[0, 0]);
        let mut data = vec![old[0], 3];
        data.extend_from_slice(b"usa");
        data.push(2);
        data.extend_from_slice(b"m1");
        data.extend_from_slice(&old[3..]);
        let mut edited = bytes[..start - 4].to_vec();
        edited.extend_from_slice(&(data.len() as i32).to_le_bytes());
        edited.extend_from_slice(&data);
        edited.extend_from_slice(&bytes[end..]);
        let e = Client::load(edited).err().expect("the load fails");
        eprintln!("{e}");
        assert_eq!(e.block, "CHUNK_Campaign");
        assert_eq!(e.error, XferStatus::ErrorUnknown);
        assert!(e.errors.iter().any(|x| x.contains("CampaignManager::xfer") && x.contains("InvalidData")), "{:?}", e.errors);
        assert_eq!(e.blocks.last().map(|b| b.name.clone()), Some(b"CHUNK_Campaign".to_vec()));
    }
}
