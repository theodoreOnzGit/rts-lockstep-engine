// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Recorder.h
//   GeneralsMD/Code/GameEngine/Source/Common/Recorder.cpp
//   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameMessageParser.h
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameMessageParser.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `RecorderClass`: the replay file, written while a game is recorded and read back while it is played (layer 12 batch
//! R1), and `GameMessageParser`, the run-length list of argument types each recorded command carries.
//!
//! # The file
//!
//! Upstream writes the file with `stdio` on Windows; the format is what that build writes, byte for byte:
//!
//! | bytes | field |
//! |---|---|
//! | 6 | `GENREP` |
//! | 4 + 4 | start and end time, `time_t` (32 bits in the shipped build; the start written by `logGameStart`, the end by `logGameEnd`) |
//! | 4 | the duration in frames (`logGameEnd`) |
//! | 1 + 1 + 8 | desync, quit early, and one disconnect flag per slot (`Bool`) |
//! | UTF-16 + 0 | the replay's name (`fwprintf(L"%ws")` and `fputwc(0)` on a binary stream: two bytes a unit) |
//! | 16 | the local time (`SYSTEMTIME`) |
//! | UTF-16 + 0, UTF-16 + 0, 4, 4, 4 | the version string, its build time, the version number, the exe CRC, the INI CRC |
//! | ASCII + 0 | the game options (`GameInfoToAsciiString`) |
//! | ASCII + 0 | the local slot, `%d` |
//! | 4 x 4 | difficulty, original game mode, rank points, the frame rate limit |
//!
//! then, for each recorded command, its frame (4), type (4), player index (4), the parser's run count (1), each run's
//! type and count (1 + 1), and the arguments (an integer, real, object, drawable, team or timestamp 4 bytes, a `Bool` 1,
//! a location 12, a pixel 8, a pixel region 16, a wide character 2).
//!
//! The file is a [`ReplayFile`] (bytes, a position and a direction, as a `FILE*`: a write to a stream opened for
//! reading fails and moves nothing, and so does a read from one opened for writing); the files themselves are the host's
//! ([`RecorderHost::read_file`], [`RecorderHost::write_file`]), written when upstream's stream would reach the disk
//! (`fflush`, `fclose`).
//!
//! # Reading, as upstream reads
//!
//! - `fread` of an item the file has only part of copies those bytes and fails (both Windows' and glibc's `fread` do):
//!   `readNextFrame`'s frame keeps the low bytes it read, and a failed read is reported as a failure, not a value.
//! - A command argument read short is an uninitialised local appended to the message ([`Undefined::UninitialisedLocal`]);
//!   a `Bool` argument byte other than 0 or 1 is [`Undefined::InvalidBoolLoaded`].
//! - `readAsciiString` at the end of the file stores `(char)EOF`, 0xFF, before it sees the end; a string of 1,024
//!   characters or more writes `str[1024]` ([`Undefined::ArrayIndexOutOfRange`]).
//! - `readUnicodeString` tests `fgetwc`'s result against `EOF`; Windows' `fgetwc` returns `WEOF`, 0xFFFF (its `wint_t` is
//!   an `unsigned short`), which is never `EOF`, so a wide string that meets the end of the file runs on to `str[1024]`
//!   ([`Undefined::ArrayIndexOutOfRange`]).
//! - A type run of count 0 makes `appendNextCommand` read every remaining argument as that type (its count goes to -1
//!   and never back to 0).
//!
//! # Not translated
//!
//! The debug-build statistics files (`m_saveStats`), `cleanUpReplayFile` (debug only), `analyzeReplay` and
//! `isAnalysisInProgress` (debug only; `m_doingAnalysis` is therefore always false, and kept), `initControls` (the
//! replay control window: client), and the network's slot lists (`TheLAN`, `TheGameSpyGame`): with a network the host
//! gives the slot list and local slot ([`RecorderHost::network_slot_list`]).

use crate::common::game_info::{game_info_to_ascii_string, parse_ascii_string_to_game_info, GameInfo, GameInfoHost, MAX_SLOTS};
use crate::common::ini::{IniError, Undefined};
use crate::common::message_stream::*;
use crate::logic::modules::plant;
use std::collections::VecDeque;

type R<T> = Result<T, IniError>;

fn undefined(u: Undefined) -> IniError {
    IniError::undefined(u)
}

/// `GameMode` (GameLogic.h).
pub mod game_mode {
    pub const SINGLE_PLAYER: i32 = 0;
    pub const LAN: i32 = 1;
    pub const SKIRMISH: i32 = 2;
    pub const REPLAY: i32 = 3;
    pub const SHELL: i32 = 4;
    pub const INTERNET: i32 = 5;
    pub const NONE: i32 = 6;
}

/// `DIFFICULTY_NORMAL`.
pub const DIFFICULTY_NORMAL: i32 = 1;
/// `replayExtention`.
pub const REPLAY_EXTENSION: &[u8] = b".rep";
/// `lastReplayFileName` (what `getLastReplayFileName` returns in a release build).
pub const LAST_REPLAY_FILE_NAME: &[u8] = b"00000000";
/// `sizeof(time_t)` in the shipped build.
const TIME_T: u64 = 4;
/// `startTimeOffset`.
pub const START_TIME_OFFSET: u64 = 6;
/// `endTimeOffset`.
pub const END_TIME_OFFSET: u64 = START_TIME_OFFSET + TIME_T;
/// `framesOffset`.
pub const FRAMES_OFFSET: u64 = END_TIME_OFFSET + TIME_T;
/// `desyncOffset`.
pub const DESYNC_OFFSET: u64 = FRAMES_OFFSET + 4;
/// `quitEarlyOffset`.
pub const QUIT_EARLY_OFFSET: u64 = DESYNC_OFFSET + 1;
/// `disconOffset`.
pub const DISCON_OFFSET: u64 = QUIT_EARLY_OFFSET + 1;

/// `enum RecorderModeType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum RecorderMode {
    Record = 0,
    Playback = 1,
    None = 2,
}

/// An open replay file (`FILE*`): its path, its bytes, the position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayFile {
    pub path: Vec<u8>,
    pub bytes: Vec<u8>,
    pub pos: usize,
    writing: bool,
}

impl ReplayFile {
    /// `fwrite`: at the position, over the bytes there and past the end (a gap left by a seek is zeros). On a stream
    /// opened for reading it fails and nothing moves (upstream writes the recording into the stream a playback left
    /// open, after `testVersionPlayback`).
    pub fn write(&mut self, data: &[u8]) {
        if !self.writing {
            return;
        }
        if self.bytes.len() < self.pos {
            self.bytes.resize(self.pos, 0);
        }
        let end = self.pos + data.len();
        if self.bytes.len() < end {
            self.bytes.resize(end, 0);
        }
        self.bytes[self.pos..end].copy_from_slice(data);
        self.pos = end;
    }

    /// `fread` of one item of `buf.len()` bytes: the bytes there are (copied into the start of `buf`), true when all were.
    pub fn read_into(&mut self, buf: &mut [u8]) -> bool {
        // a stream opened for writing reads nothing
        if self.writing {
            return buf.is_empty();
        }
        let avail = self.bytes.len().saturating_sub(self.pos).min(buf.len());
        buf[..avail].copy_from_slice(&self.bytes[self.pos..self.pos + avail]);
        self.pos += avail;
        avail == buf.len()
    }

    /// Whether the stream was opened for writing (`"wb"`).
    pub fn is_writing(&self) -> bool {
        self.writing
    }

    /// `fgetc`: the byte, or `None` at the end.
    pub fn getc(&mut self) -> Option<u8> {
        if self.writing {
            return None;
        }
        let b = self.bytes.get(self.pos).copied()?;
        self.pos += 1;
        Some(b)
    }

    /// Windows' `fgetwc` on a binary stream: two bytes, little-endian, or `WEOF` (0xFFFF) when fewer are left.
    pub fn getwc(&mut self) -> u16 {
        let Some(a) = self.getc() else { return 0xFFFF };
        let Some(b) = self.getc() else { return 0xFFFF };
        u16::from_le_bytes([a, b])
    }
}

/// `CRCInfo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrcInfo {
    pub saw_crc_mismatch: bool,
    pub skipped_one: bool,
    pub data: VecDeque<u32>,
    pub local_player: u32,
}

impl Default for CrcInfo {
    fn default() -> Self {
        CrcInfo { saw_crc_mismatch: false, skipped_one: false, data: VecDeque::new(), local_player: !0 }
    }
}

impl CrcInfo {
    /// `addCRC(val)`.
    pub fn add_crc(&mut self, val: u32) {
        self.data.push_back(val);
    }
    /// `readCRC()`: the oldest, or 0 when there is none.
    pub fn read_crc(&mut self) -> u32 {
        if plant(3966) {
            return self.data.pop_back().unwrap_or(0);
        }
        self.data.pop_front().unwrap_or(0)
    }
}

/// `ReplayHeader` (the fields a read reached; `None` where the file ended first, as upstream's uninitialised members).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReplayHeader {
    pub filename: Vec<u8>,
    pub for_playback: bool,
    pub replay_name: Vec<u16>,
    pub time_val: Option<[u8; 16]>,
    pub version_string: Vec<u16>,
    pub version_time_string: Vec<u16>,
    pub version_number: Option<u32>,
    pub exe_crc: Option<u32>,
    pub ini_crc: Option<u32>,
    pub start_time: Option<u32>,
    pub end_time: Option<u32>,
    pub frame_duration: Option<u32>,
    pub quit_early: Option<u8>,
    pub desync_game: Option<u8>,
    pub player_discons: [Option<u8>; MAX_SLOTS],
    pub game_options: Vec<u8>,
    pub local_player_index: i32,
}

/// `GameMessageParser(msg)`: the message's arguments as runs of one type, `(type, count)` (over the first
/// `getArgumentCount()` arguments, an `UnsignedByte`).
pub fn message_arg_type_runs(msg: &GameMessage) -> Vec<(i32, i32)> {
    let mut runs = Vec::new();
    let mut last = ArgumentDataType::Unknown as i32;
    let mut this_count = 0;
    for i in 0..i32::from(msg.get_argument_count()) {
        let t = msg.get_argument_data_type(i) as i32;
        if t != last {
            if this_count > 0 {
                runs.push((last, this_count));
            }
            last = t;
            this_count = 0;
        }
        this_count += 1;
    }
    if this_count > 0 {
        runs.push((last, this_count));
    }
    runs
}

/// The world as `Recorder.cpp` reaches it (besides `GameInfo.cpp`'s, the [`GameInfoHost`] supertrait).
pub trait RecorderHost: GameInfoHost {
    /// `TheGameLogic->getFrame()`.
    fn frame(&self) -> u32;
    /// `TheGameLogic->isInGame()`.
    fn is_in_game(&self) -> bool;
    /// `TheGameLogic->getGameMode()`.
    fn game_mode(&self) -> i32;
    /// `TheGameLogic->clearGameData()`.
    fn clear_game_data(&mut self);
    /// `TheCommandList` and `TheMessageStream`.
    fn messages(&mut self) -> &mut Messages;
    /// `ThePlayerList->getLocalPlayer()->getPlayerIndex()` (a new message's player).
    fn local_player_index(&self) -> i32;
    /// `ThePlayerList->getNthPlayer(i)`: `None` when there is none, else whether its name key is `NAMEKEY(name)`.
    fn player_name_key_is(&mut self, player_index: i32, name: &[u8]) -> Option<bool>;
    /// `TheGlobalData->m_pendingFile`.
    fn pending_file(&self) -> Vec<u8>;
    /// `TheWritableGlobalData->m_pendingFile = name`.
    fn set_pending_file(&mut self, name: &[u8]);
    /// `TheGlobalData->m_mapName`.
    fn map_name(&self) -> Vec<u8>;
    /// `TheGlobalData->getPath_UserData()`.
    fn user_data_path(&self) -> Vec<u8>;
    /// `TheGlobalData->m_exeCRC`.
    fn exe_crc(&self) -> u32;
    /// `TheGlobalData->m_iniCRC`.
    fn ini_crc(&self) -> u32;
    /// `TheVersion->getUnicodeVersion()`.
    fn version_string(&self) -> Vec<u16>;
    /// `TheVersion->getUnicodeBuildTime()`.
    fn version_time_string(&self) -> Vec<u16>;
    /// `TheVersion->getVersionNumber()`.
    fn version_number(&self) -> u32;
    /// `GetLocalTime(&systemTime)`: the `SYSTEMTIME`'s sixteen bytes.
    fn local_time(&mut self) -> [u8; 16];
    /// `time(&t)` (a 32-bit `time_t`).
    fn time(&mut self) -> u32;
    /// `GetGameLogicRandomSeed()`.
    fn game_logic_random_seed(&self) -> u32;
    /// `InitRandom(seed)`.
    fn init_random(&mut self, seed: u32);
    /// `TheFileSystem->createDirectory(path)`.
    fn create_directory(&mut self, path: &[u8]);
    /// `fopen(path, "rb")`'s contents, or `None`.
    fn read_file(&mut self, path: &[u8]) -> Option<Vec<u8>>;
    /// `fopen(path, "wb")`: whether it opens (the file is created empty).
    fn create_file(&mut self, path: &[u8]) -> bool;
    /// What `fflush`/`fclose` write to the file.
    fn write_file(&mut self, path: &[u8], bytes: &[u8]);
    /// `TheNetwork != NULL`.
    fn has_network(&self) -> bool;
    /// With a network: `GameInfoToAsciiString` of the LAN or GameSpy game and the local slot (not translated here).
    fn network_slot_list(&mut self) -> (Vec<u8>, i32);
    /// `TheSkirmishGameInfo`, when there is one.
    fn skirmish_game_info(&mut self) -> Option<&mut GameInfo>;
}

/// `RecorderClass` (with its `ReplayGameInfo`, a `GameInfo` with eight slots of its own).
#[derive(Clone, Debug, PartialEq)]
pub struct Recorder {
    pub crc_info: Option<CrcInfo>,
    pub file: Option<ReplayFile>,
    pub file_name: Vec<u8>,
    pub current_file_position: i32,
    pub mode: RecorderMode,
    pub current_replay_filename: Vec<u8>,
    pub game_info: GameInfo,
    pub was_desync: bool,
    pub doing_analysis: bool,
    pub original_game_mode: i32,
    pub next_frame: u32,
    /// `REPLAY_CRC_INTERVAL` (a global of `Recorder.cpp`, 100 at start; `playbackFile` sets it).
    pub replay_crc_interval: i32,
}

impl Recorder {
    /// `RecorderClass()`: `init()` (the members its initialiser list sets first are overwritten there, but for
    /// `m_nextFrame`, 0, and `m_crcInfo`, which the zero-filling `NEW` leaves `NULL`).
    pub fn new(host: &mut impl RecorderHost) -> Recorder {
        let game_info = GameInfo::new(host);
        let mut r = Recorder {
            crc_info: None,
            file: None,
            file_name: Vec::new(),
            current_file_position: 0,
            mode: RecorderMode::Record,
            current_replay_filename: Vec::new(),
            game_info,
            was_desync: false,
            doing_analysis: false,
            original_game_mode: game_mode::NONE,
            next_frame: 0,
            replay_crc_interval: 100,
        };
        r.init(host);
        r
    }

    /// `init()`: the open file, if any, is forgotten (not closed).
    pub fn init(&mut self, host: &mut impl RecorderHost) {
        self.original_game_mode = game_mode::NONE;
        self.mode = RecorderMode::None;
        self.file = None;
        self.file_name.clear();
        self.current_file_position = 0;
        self.game_info.clear_slot_list(host);
        self.game_info.reset(host);
        let pending = host.pending_file();
        let map = if pending.is_empty() { host.map_name() } else { pending };
        // not in a game: setMap only stores the name
        self.game_info.map_name = map;
        self.game_info.seed = host.game_logic_random_seed() as i32;
        self.was_desync = false;
        self.doing_analysis = false;
    }

    fn close_file(&mut self, host: &mut impl RecorderHost) {
        if let Some(f) = self.file.take() {
            if f.writing {
                host.write_file(&f.path, &f.bytes);
            }
        }
    }

    fn flush(&mut self, host: &mut impl RecorderHost) {
        if let Some(f) = &self.file {
            if f.writing {
                host.write_file(&f.path, &f.bytes);
            }
        }
    }

    /// `reset()`.
    pub fn reset(&mut self, host: &mut impl RecorderHost) {
        self.close_file(host);
        self.file_name.clear();
        self.init(host);
    }

    /// `update()`.
    pub fn update(&mut self, host: &mut impl RecorderHost) -> R<()> {
        match self.mode {
            RecorderMode::Record | RecorderMode::None => self.update_record(host),
            RecorderMode::Playback => self.update_playback(host),
        }
    }

    /// `updatePlayback()`.
    pub fn update_playback(&mut self, host: &mut impl RecorderHost) -> R<()> {
        self.cull_bad_commands(host)?;
        if self.next_frame == u32::MAX {
            return Ok(());
        }
        let mut cur = host.frame();
        if self.doing_analysis {
            cur = self.next_frame;
        }
        while self.next_frame == cur {
            self.append_next_command(host)?;
            self.read_next_frame(host)?;
        }
        Ok(())
    }

    /// `stopPlayback()`.
    pub fn stop_playback(&mut self, host: &mut impl RecorderHost) -> R<()> {
        self.close_file(host);
        self.file_name.clear();
        if !self.doing_analysis {
            let local = host.local_player_index();
            host.messages().append_new_message(Messages::STREAM, MSG_CLEAR_GAME_DATA, local)?;
        }
        Ok(())
    }

    /// `updateRecord()`.
    pub fn update_record(&mut self, host: &mut impl RecorderHost) -> R<()> {
        let mut need_flush = false;
        let mut at = host.messages().first_message(Messages::COMMAND_LIST);
        while let Some(id) = at {
            let msg = host.messages().message(id)?.clone();
            let t = msg.get_type();
            let mode = msg.get_argument(0).integer();
            if t == MSG_NEW_GAME && mode != game_mode::SHELL && mode != game_mode::SINGLE_PLAYER && mode != game_mode::NONE {
                self.original_game_mode = mode;
                let count = msg.get_argument_count();
                let diff = if count >= 2 { msg.get_argument(1).integer() } else { DIFFICULTY_NORMAL };
                let rank = if count >= 3 { msg.get_argument(2).integer() } else { 0 };
                let fps = if count >= 4 { msg.get_argument(3).integer() } else { 0 };
                self.start_recording(diff, self.original_game_mode, rank, fps, host)?;
            } else if t == MSG_CLEAR_GAME_DATA {
                if self.file.is_some() {
                    self.write_to_file(&msg, host)?;
                    self.stop_recording(host);
                }
                self.file_name.clear();
            } else if self.file.is_some() && t > MSG_BEGIN_NETWORK_MESSAGES && t < MSG_END_NETWORK_MESSAGES && !(plant(3961) && t == MSG_LOGIC_CRC) {
                self.write_to_file(&msg, host)?;
                need_flush = true;
            }
            at = host.messages().message(id)?.next();
        }
        if need_flush {
            // fflush(m_file): a stream closed above is NULL here (fflush(NULL) flushes every stream)
            self.flush(host);
        }
        Ok(())
    }

    /// `getReplayDir()`: the user data path and `Replays\`.
    pub fn replay_dir(&self, host: &impl RecorderHost) -> Vec<u8> {
        let mut d = host.user_data_path();
        d.extend_from_slice(b"Replays\\");
        d
    }

    fn put(&mut self, data: &[u8]) {
        if let Some(f) = &mut self.file {
            f.write(data);
        }
    }

    fn put_wide(&mut self, s: &[u16]) {
        let bytes: Vec<u8> = s.iter().take_while(|&&c| c != 0).flat_map(|c| c.to_le_bytes()).collect();
        self.put(&bytes);
        self.put(&[0, 0]);
    }

    /// `startRecording(diff, originalGameMode, rankPoints, maxFPS)`.
    pub fn start_recording(&mut self, diff: i32, original_game_mode: i32, rank_points: i32, max_fps: i32, host: &mut impl RecorderHost) -> R<()> {
        self.reset(host);
        self.mode = RecorderMode::Record;
        let mut filepath = self.replay_dir(host);
        host.create_directory(&filepath);
        self.file_name = LAST_REPLAY_FILE_NAME.to_vec();
        self.file_name.extend_from_slice(REPLAY_EXTENSION);
        filepath.extend_from_slice(&self.file_name);
        if !host.create_file(&filepath) {
            return Ok(());
        }
        self.file = Some(ReplayFile { path: filepath, bytes: Vec::new(), pos: 0, writing: true });
        self.put(b"GENREP");
        self.put(&0u32.to_le_bytes());
        self.put(&0u32.to_le_bytes());
        self.put(&0u32.to_le_bytes());
        self.put(&[0, 0]);
        self.put(&[0; MAX_SLOTS]);
        let name = host.game_text("GUI:LastReplay");
        self.put_wide(&name);
        let t = host.local_time();
        self.put(&t);
        let (vs, vt, vn) = (host.version_string(), host.version_time_string(), host.version_number());
        self.put_wide(&vs);
        self.put_wide(&vt);
        self.put(&vn.to_le_bytes());
        self.put(&host.exe_crc().to_le_bytes());
        self.put(&host.ini_crc().to_le_bytes());
        let (slot_list, local_index) = if host.has_network() {
            host.network_slot_list()
        } else if host.skirmish_game_info().is_some() {
            let interval = self.replay_crc_interval;
            let g = host.skirmish_game_info().expect("checked").clone();
            let mut g2 = g;
            g2.set_crc_interval(interval);
            let s = game_info_to_ascii_string(Some(&g2), host)?;
            *host.skirmish_game_info().expect("checked") = g2;
            (s, 0)
        } else {
            self.game_info.set_crc_interval(self.replay_crc_interval);
            (game_info_to_ascii_string(Some(&self.game_info), host)?, if plant(3967) { 0 } else { -1 })
        };
        self.log_game_start(host);
        // fwrite(theSlotList.str(), getLength() + 1): the string and its 0
        let mut sl = slot_list.clone();
        sl.push(0);
        self.put(&sl);
        let mut li = format!("{local_index}").into_bytes();
        li.push(0);
        self.put(&li);
        self.put(&diff.to_le_bytes());
        self.put(&original_game_mode.to_le_bytes());
        self.put(&rank_points.to_le_bytes());
        self.put(&max_fps.to_le_bytes());
        Ok(())
    }

    /// Seek to `offset`, write, seek back to where the stream was (`ftell`'s position). `fseek` flushes a stream's
    /// buffered output, so the file holds everything written so far afterwards.
    fn patch(&mut self, offset: u64, data: &[u8], host: &mut impl RecorderHost) {
        if let Some(f) = &mut self.file {
            let back = f.pos;
            f.pos = offset as usize;
            f.write(data);
            f.pos = back;
        }
        self.flush(host);
    }

    /// `logGameStart(options)`: the start time into the header.
    pub fn log_game_start(&mut self, host: &mut impl RecorderHost) {
        if self.file.is_none() {
            return;
        }
        let t = host.time();
        self.patch(START_TIME_OFFSET, &t.to_le_bytes(), host);
    }

    /// `logPlayerDisconnect(player, slot)`: the slot's flag set.
    pub fn log_player_disconnect(&mut self, slot: i32, host: &mut impl RecorderHost) {
        if self.file.is_none() || !(0..MAX_SLOTS as i32).contains(&slot) {
            return;
        }
        self.patch(DISCON_OFFSET + slot as u64, &[1], host);
    }

    /// `logCRCMismatch()`: the desync flag set.
    pub fn log_crc_mismatch(&mut self, host: &mut impl RecorderHost) {
        if self.file.is_none() {
            return;
        }
        self.patch(DESYNC_OFFSET, &[1], host);
    }

    /// `logGameEnd()`: the end time and the duration in frames into the header.
    pub fn log_game_end(&mut self, host: &mut impl RecorderHost) {
        if self.file.is_none() {
            return;
        }
        let t = host.time();
        let duration = host.frame();
        self.patch(END_TIME_OFFSET, &t.to_le_bytes(), host);
        self.patch(FRAMES_OFFSET, &duration.to_le_bytes(), host);
    }

    /// `stopRecording()`.
    pub fn stop_recording(&mut self, host: &mut impl RecorderHost) {
        self.log_game_end(host);
        if host.has_network() {
            // cleanUpReplayFile() does nothing outside debug builds
            self.was_desync = false;
        }
        self.close_file(host);
        self.file_name.clear();
    }

    /// `writeToFile(msg)`: the frame, type, player, the argument type runs, the arguments; then `fflush`.
    pub fn write_to_file(&mut self, msg: &GameMessage, host: &mut impl RecorderHost) -> R<()> {
        let frame = host.frame();
        self.put(&frame.to_le_bytes());
        self.put(&msg.get_type().0.to_le_bytes());
        self.put(&msg.get_player_index().to_le_bytes());
        let runs = message_arg_type_runs(msg);
        self.put(&[runs.len() as u8]);
        for (t, n) in &runs {
            self.put(&[*t as u8, *n as u8]);
        }
        for i in 0..i32::from(msg.get_argument_count()) {
            let t = msg.get_argument_data_type(i);
            let a = msg.get_argument(i);
            self.write_argument(t, a);
        }
        self.flush(host);
        Ok(())
    }

    /// `writeArgument(type, arg)`: the member's bytes (nothing for an unknown type).
    fn write_argument(&mut self, t: ArgumentDataType, a: ArgumentData) {
        let w = a.0;
        let words = |n: usize| w[..n].iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
        match t {
            ArgumentDataType::Integer
            | ArgumentDataType::Real
            | ArgumentDataType::ObjectId
            | ArgumentDataType::DrawableId
            | ArgumentDataType::TeamId
            | ArgumentDataType::Timestamp => self.put(&words(1)),
            ArgumentDataType::Boolean if plant(3968) => self.put(&words(1)),
            ArgumentDataType::Boolean => self.put(&[w[0] as u8]),
            ArgumentDataType::Location => self.put(&words(3)),
            ArgumentDataType::Pixel => self.put(&words(2)),
            ArgumentDataType::PixelRegion => self.put(&words(4)),
            ArgumentDataType::WideChar => self.put(&(w[0] as u16).to_le_bytes()),
            ArgumentDataType::Unknown => {}
        }
    }

    fn file_mut(&mut self) -> R<&mut ReplayFile> {
        // a stream used after it was closed is fread on NULL
        self.file.as_mut().ok_or_else(|| undefined(Undefined::NullObject))
    }

    /// `readNextFrame()`: the next command's frame, or -1 (and the playback stopped) when it cannot be read whole (the
    /// bytes a short read reached are written into the frame first, and overwritten).
    pub fn read_next_frame(&mut self, host: &mut impl RecorderHost) -> R<()> {
        let mut b = self.next_frame.to_le_bytes();
        let ok = self.file_mut()?.read_into(&mut b);
        self.next_frame = u32::from_le_bytes(b);
        if !ok {
            self.next_frame = u32::MAX;
            if !plant(3964) {
                self.stop_playback(host)?;
            }
        }
        Ok(())
    }

    /// `appendNextCommand()`.
    pub fn append_next_command(&mut self, host: &mut impl RecorderHost) -> R<()> {
        let mut tb = [0u8; 4];
        // `type` is uninitialised; a failed read returns before it is used
        if !self.file_mut()?.read_into(&mut tb) {
            return Ok(());
        }
        let ty = MessageType(i32::from_le_bytes(tb));
        let local = host.local_player_index();
        let msg = host.messages().new_message(ty, local);
        let kept = !(ty == MSG_BEGIN_NETWORK_MESSAGES || ty == MSG_CLEAR_GAME_DATA);
        if kept && !self.doing_analysis {
            host.messages().append_message(Messages::COMMAND_LIST, msg)?;
        }
        let mut pb = (-1i32).to_le_bytes();
        self.file_mut()?.read_into(&mut pb);
        host.messages().message_mut(msg)?.set_player_index(i32::from_le_bytes(pb));
        let mut nt = [0u8; 1];
        self.file_mut()?.read_into(&mut nt);
        let mut runs: Vec<(i32, i32)> = Vec::new();
        let mut total_args = 0i32;
        for _ in 0..nt[0] {
            let mut t = [ArgumentDataType::Unknown as u8];
            self.file_mut()?.read_into(&mut t);
            let mut n = [0u8];
            self.file_mut()?.read_into(&mut n);
            runs.push((i32::from(t[0]), i32::from(n[0])));
            total_args += i32::from(n[0]);
        }
        let mut run_at = 0usize;
        let mut last_type = ArgumentDataType::Unknown as i32;
        let mut left = 0i32;
        if let Some(&(t, n)) = runs.first() {
            last_type = t;
            left = n;
        }
        for _ in 0..total_args {
            self.read_argument(last_type, msg, host)?;
            left -= 1;
            if left == 0 && !plant(3962) {
                run_at += 1;
                if let Some(&(t, n)) = runs.get(run_at) {
                    left = n;
                    last_type = t;
                }
            }
        }
        if !kept || self.doing_analysis {
            host.messages().delete_message(msg)?;
        }
        Ok(())
    }

    /// `readArgument(type, msg)`.
    fn read_argument(&mut self, t: i32, msg: MessageId, host: &mut impl RecorderHost) -> R<()> {
        let Some(t) = ArgumentDataType::from_i32(t) else {
            return Ok(());
        };
        let size = match t {
            ArgumentDataType::Integer
            | ArgumentDataType::Real
            | ArgumentDataType::ObjectId
            | ArgumentDataType::DrawableId
            | ArgumentDataType::TeamId
            | ArgumentDataType::Timestamp => 4,
            ArgumentDataType::Boolean => 1,
            ArgumentDataType::Location => 12,
            ArgumentDataType::Pixel => 8,
            ArgumentDataType::PixelRegion => 16,
            ArgumentDataType::WideChar => 2,
            ArgumentDataType::Unknown => return Ok(()),
        };
        let mut b = vec![0u8; size];
        if !self.file_mut()?.read_into(&mut b) {
            return Err(undefined(Undefined::UninitialisedLocal));
        }
        let w = |i: usize| u32::from_le_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]);
        let m = host.messages().message_mut(msg)?;
        match t {
            ArgumentDataType::Integer => m.append_integer_argument(w(0) as i32),
            ArgumentDataType::Real => m.append_real_argument(f32::from_bits(w(0))),
            ArgumentDataType::Boolean => {
                if b[0] > 1 {
                    return Err(undefined(Undefined::InvalidBoolLoaded));
                }
                m.append_boolean_argument(b[0] == 1);
            }
            ArgumentDataType::ObjectId => m.append_object_id_argument(w(0)),
            ArgumentDataType::DrawableId => m.append_drawable_id_argument(w(0)),
            ArgumentDataType::TeamId => m.append_team_id_argument(w(0)),
            ArgumentDataType::Location => m.append_argument(GameMessageArgument { data: ArgumentData([w(0), w(1), w(2), 0]), ty: t }),
            ArgumentDataType::Pixel => m.append_argument(GameMessageArgument { data: ArgumentData([w(0), w(1), 0, 0]), ty: t }),
            ArgumentDataType::PixelRegion => m.append_argument(GameMessageArgument { data: ArgumentData([w(0), w(1), w(2), w(3)]), ty: t }),
            ArgumentDataType::Timestamp => m.append_timestamp_argument(w(0)),
            ArgumentDataType::WideChar => m.append_wide_char_argument(u16::from_le_bytes([b[0], b[1]])),
            ArgumentDataType::Unknown => {}
        }
        Ok(())
    }

    /// `cullBadCommands()`: the network commands (but the logic CRC) a player gave during a playback are deleted.
    pub fn cull_bad_commands(&mut self, host: &mut impl RecorderHost) -> R<()> {
        if self.doing_analysis {
            return Ok(());
        }
        let mut at = host.messages().first_message(Messages::COMMAND_LIST);
        while let Some(id) = at {
            let m = host.messages().message(id)?;
            let next = m.next();
            let t = m.get_type();
            if t > MSG_BEGIN_NETWORK_MESSAGES && t < MSG_END_NETWORK_MESSAGES && (t != MSG_LOGIC_CRC || plant(3965)) {
                host.messages().delete_message(id)?;
            }
            at = next;
        }
        Ok(())
    }

    /// `readAsciiString()` (see the module docs for the end of the file and the 1,024-character buffer).
    fn read_ascii_string(&mut self) -> R<Vec<u8>> {
        let f = self.file_mut()?;
        let mut s: Vec<u8> = Vec::new();
        // the first character: (char)EOF at the end
        let c = f.getc().unwrap_or(0xFF);
        s.push(c);
        let mut index = 0usize;
        while index < 1024 && s[index] != 0 {
            index += 1;
            if index >= 1024 {
                return Err(undefined(Undefined::ArrayIndexOutOfRange));
            }
            match f.getc() {
                None => {
                    s.push(0);
                    break;
                }
                Some(c) => s.push(c),
            }
        }
        Ok(s.into_iter().take_while(|&b| b != 0).collect())
    }

    /// `readUnicodeString()` (see the module docs: `WEOF` is never `EOF`).
    fn read_unicode_string(&mut self) -> R<Vec<u16>> {
        let f = self.file_mut()?;
        let mut s: Vec<u16> = vec![f.getwc()];
        let mut index = 0usize;
        while index < 1024 && s[index] != 0 {
            index += 1;
            if index >= 1024 {
                return Err(undefined(Undefined::ArrayIndexOutOfRange));
            }
            s.push(f.getwc());
        }
        Ok(s.into_iter().take_while(|&c| c != 0).collect())
    }

    /// `readReplayHeader(header)`: opens the file (left open for a playback) and reads the header; the game options
    /// go into this recorder's game, entered and started.
    pub fn read_replay_header(&mut self, header: &mut ReplayHeader, host: &mut impl RecorderHost) -> R<bool> {
        let mut path = self.replay_dir(host);
        path.extend_from_slice(&header.filename);
        let Some(bytes) = host.read_file(&path) else {
            return Ok(false);
        };
        self.file = Some(ReplayFile { path, bytes, pos: 0, writing: false });
        let mut genrep = [0u8; 6];
        let f = self.file_mut()?;
        let at = f.pos;
        let got = f.bytes.len().saturating_sub(at).min(6);
        f.read_into(&mut genrep);
        // `strncmp` stops at the first difference; past a short read it compares uninitialised bytes
        if got < 6 && genrep[..got] == b"GENREP"[..got] {
            return Err(undefined(Undefined::UninitialisedLocal));
        }
        // short of six bytes, the difference is within the bytes read
        let matches = got == 6 && &genrep == b"GENREP";
        if !matches && !plant(3963) {
            self.file = None;
            return Ok(false);
        }
        fn rd32(f: &mut ReplayFile) -> Option<u32> {
            let mut b = [0u8; 4];
            f.read_into(&mut b).then(|| u32::from_le_bytes(b))
        }
        fn rd8(f: &mut ReplayFile) -> Option<u8> {
            f.getc()
        }
        let f = self.file_mut()?;
        header.start_time = rd32(f);
        header.end_time = rd32(f);
        header.frame_duration = rd32(f);
        header.desync_game = rd8(f);
        header.quit_early = rd8(f);
        for d in header.player_discons.iter_mut() {
            *d = rd8(f);
        }
        header.replay_name = self.read_unicode_string()?;
        let mut t = [0u8; 16];
        header.time_val = self.file_mut()?.read_into(&mut t).then_some(t);
        header.version_string = self.read_unicode_string()?;
        header.version_time_string = self.read_unicode_string()?;
        let f = self.file_mut()?;
        header.version_number = rd32(f);
        header.exe_crc = rd32(f);
        header.ini_crc = rd32(f);
        header.game_options = self.read_ascii_string()?;
        self.game_info.reset(host);
        self.game_info.enter_game(host);
        if !parse_ascii_string_to_game_info(Some(&mut self.game_info), &header.game_options, host)? {
            self.file = None;
            return Ok(false);
        }
        self.game_info.start_game(0, host);
        let index = self.read_ascii_string()?;
        header.local_player_index = atoi(&index)?;
        if header.local_player_index < -1 || header.local_player_index >= MAX_SLOTS as i32 {
            self.game_info.end_game();
            self.game_info.reset(host);
            self.file = None;
            return Ok(false);
        }
        if header.local_player_index >= 0 {
            let ip = self.game_info.slots[header.local_player_index as usize].ip;
            self.game_info.local_ip = ip;
        }
        if !header.for_playback {
            self.game_info.end_game();
            self.game_info.reset(host);
            self.file = None;
        }
        Ok(true)
    }

    /// `testVersionPlayback(filename)`: true when the file's version or CRCs differ from this build's (the file is left
    /// open, as upstream leaves it).
    pub fn test_version_playback(&mut self, filename: &[u8], host: &mut impl RecorderHost) -> R<bool> {
        let mut header = ReplayHeader { for_playback: true, filename: filename.to_vec(), ..Default::default() };
        if !self.read_replay_header(&mut header, host)? {
            return Ok(false);
        }
        let (vn, exe, ini) = match (header.version_number, header.exe_crc, header.ini_crc) {
            (Some(a), Some(b), Some(c)) => (a, b, c),
            _ => return Err(undefined(Undefined::UninitialisedLocal)),
        };
        let exe_different = header.version_string != host.version_string()
            || header.version_time_string != host.version_time_string()
            || vn != host.version_number()
            || exe != host.exe_crc();
        Ok(exe_different || ini != host.ini_crc())
    }

    /// `playbackFile(filename)`.
    pub fn playback_file(&mut self, filename: &[u8], host: &mut impl RecorderHost) -> R<bool> {
        if !self.doing_analysis && host.is_in_game() {
            host.clear_game_data();
        }
        self.mode = RecorderMode::Playback;
        let mut header = ReplayHeader { for_playback: true, filename: filename.to_vec(), ..Default::default() };
        if !self.read_replay_header(&mut header, host)? {
            return Ok(false);
        }
        let map = self.game_info.map_name.clone();
        host.set_pending_file(&map);
        self.crc_info = Some(CrcInfo { local_player: header.local_player_index as u32, ..Default::default() });
        self.replay_crc_interval = self.game_info.crc_interval;
        let mut m = self.original_game_mode.to_le_bytes();
        let f = self.file_mut()?;
        let mut d = 0i32.to_le_bytes();
        f.read_into(&mut d);
        f.read_into(&mut m);
        let mut rp = 0i32.to_le_bytes();
        f.read_into(&mut rp);
        let mut fps = 0i32.to_le_bytes();
        f.read_into(&mut fps);
        self.original_game_mode = i32::from_le_bytes(m);
        let (difficulty, rank_points, max_fps) = (i32::from_le_bytes(d), i32::from_le_bytes(rp), i32::from_le_bytes(fps));
        self.read_next_frame(host)?;
        if !self.doing_analysis {
            let local = host.local_player_index();
            let msgs = host.messages();
            let id = msgs.append_new_message(Messages::STREAM, MSG_NEW_GAME, local)?;
            let m = msgs.message_mut(id)?;
            m.append_integer_argument(game_mode::REPLAY);
            m.append_integer_argument(difficulty);
            m.append_integer_argument(rank_points);
            if max_fps != 0 && !plant(3969) {
                m.append_integer_argument(max_fps);
            }
            host.init_random(self.game_info.seed as u32);
        }
        self.current_replay_filename = filename.to_vec();
        Ok(true)
    }

    /// `handleCRCMessage(newCRC, playerIndex, fromPlayback)`: a CRC the replay carries is queued; a CRC this game made
    /// (the local player's, or any when the replay had no local player) is compared with the next queued one, and a
    /// difference after frame 0 marks the replay as out of sync (once). Outside a playback `m_crcInfo` is `NULL`.
    pub fn handle_crc_message(&mut self, new_crc: u32, player_index: i32, from_playback: bool, host: &mut impl RecorderHost) -> R<()> {
        let frame = host.frame();
        let Some(info) = self.crc_info.as_mut() else {
            return Err(undefined(Undefined::NullObject));
        };
        if from_playback {
            info.add_crc(new_crc);
            return Ok(());
        }
        let local = info.local_player as i32;
        let name = format!("player{local}").into_bytes();
        let same = host.player_name_key_is(player_index, &name).is_none_or(|b| b);
        if same || local < 0 {
            let playback = info.read_crc();
            if frame > 0 && new_crc != playback && !info.saw_crc_mismatch {
                info.saw_crc_mismatch = true;
            }
        }
        Ok(())
    }

    /// `getCurrentReplayFilename()`.
    pub fn current_replay_filename(&self) -> Vec<u8> {
        if self.mode == RecorderMode::Playback {
            self.current_replay_filename.clone()
        } else {
            Vec::new()
        }
    }

    /// `isMultiplayer()`.
    pub fn is_multiplayer(&mut self, host: &mut impl RecorderHost) -> bool {
        if self.mode == RecorderMode::Playback && self.game_info.slots.iter().any(|s| s.is_occupied()) {
            return true;
        }
        let mode = host.game_mode();
        if mode == game_mode::SINGLE_PLAYER || mode == game_mode::SHELL {
            return false;
        }
        host.has_network() || host.skirmish_game_info().is_some()
    }
}

/// `atoi` (a value outside `Int` is undefined).
fn atoi(s: &[u8]) -> R<i32> {
    let mut i = 0;
    while i < s.len() && (s[i] == b' ' || (9..=13).contains(&s[i])) {
        i += 1;
    }
    let neg = i < s.len() && s[i] == b'-';
    if i < s.len() && (s[i] == b'-' || s[i] == b'+') {
        i += 1;
    }
    let mut v: i64 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        v = v * 10 + i64::from(s[i] - b'0');
        if v > i64::from(i32::MAX) + 1 {
            return Err(undefined(Undefined::IntOutOfRange));
        }
        i += 1;
    }
    let v = if neg { -v } else { v };
    i32::try_from(v).map_err(|_| undefined(Undefined::IntOutOfRange))
}
