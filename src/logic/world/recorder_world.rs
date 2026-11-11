// SPDX-License-Identifier: GPL-3.0-only
//! Integration batch I55 (GitHub #14): `TheRecorder` in the `World`. `common::recorder` translates `Recorder.cpp`; this is
//! the `RecorderHost` it runs on, and the engine frame that moves the message stream into the command list.
//!
//! - **`TheRecorder`** is [`GameLoopCuts::recorder`](crate::logic::world::game_logic_host_world::GameLoopCuts::recorder).
//!   It is `None` until [`World::recorder_init`] makes it (upstream's `GameEngine::init` makes it for every game), and the
//!   frame then calls it live: `update` (recording `TheCommandList`, or appending the next frame's recorded commands
//!   during a playback), `isMultiplayer`, `getMode`, `handleCRCMessage` from the dispatcher, `REPLAY_CRC_INTERVAL`, and
//!   the replay's game options as `TheGameInfo` when a replay starts. Without it the World's earlier stand-ins answer, so
//!   the tests that build a World without a recorder are unchanged.
//! - **Files** are on the World's disk ([`SaveSide::disk`](crate::logic::world::game_state_world::SaveSide::disk)), under
//!   the user data path (empty: the recorder's `Replays\` folder).
//! - **[`World::engine_update`]** is `GameEngine::update`'s logic half: `TheMessageStream->propagateMessages()` (the logic
//!   attaches no translators, so the stream moves to the command list) and then `GameLogic::update`.
//!
//! # What the host answers that upstream's process does not have
//!
//! The version strings, version number, build time, local time and `time()` are fixed (empty and 0): a replay records
//! them, and upstream's playback reads them only for its replay menu. The exe and INI CRCs are the global data's.

use crate::common::game_info::GameInfo;
use crate::common::message_stream::Messages;
use crate::common::recorder::{game_mode, Recorder, RecorderHost, RecorderMode};
use crate::logic::world::game_logic::World;
use crate::logic::world::game_logic_new_game::GameSource;
use crate::logic::world::object_cuts::CutCall;

type R<T> = Result<T, crate::common::ini::IniError>;

impl World {
    /// `TheRecorder = createRecorder()` and its `init()` (`GameEngine::init`).
    pub fn recorder_init(&mut self) {
        let r = Recorder::new(self);
        self.cuts.game_loop.recorder = Some(r);
    }

    /// Runs `f` on `TheRecorder` with the World as its host (the recorder is taken out of the World while `f` runs).
    /// `None` without a recorder.
    pub fn with_recorder<T>(&mut self, f: impl FnOnce(&mut Recorder, &mut World) -> T) -> Option<T> {
        let mut r = self.cuts.game_loop.recorder.take()?;
        let out = f(&mut r, self);
        if self.cuts.game_loop.recorder.is_none() {
            self.cuts.game_loop.recorder = Some(r);
        }
        Some(out)
    }

    /// `TheRecorder->isMultiplayer()` on a shared World (`RecorderClass::isMultiplayer`): a playback with an occupied
    /// slot; otherwise not in a single-player game or the shell, and a network or `TheSkirmishGameInfo`.
    pub fn recorder_is_multiplayer(&self) -> Option<bool> {
        let r = self.cuts.game_loop.recorder.as_ref()?;
        if r.mode == RecorderMode::Playback && r.game_info.slots.iter().any(|s| s.is_occupied()) {
            return Some(true);
        }
        let mode = self.cuts.game_loop.game_mode;
        if mode == game_mode::SINGLE_PLAYER || mode == game_mode::SHELL {
            return Some(false);
        }
        Some(self.cuts.game_loop.has_network || self.cuts.new_game.games.iter().any(|(s, _)| *s == GameSource::Skirmish))
    }

    /// `GameEngine::update`'s logic half: `TheMessageStream->propagateMessages()`, then `TheGameLogic->update()`.
    pub fn engine_update(&mut self) -> R<()> {
        self.cuts.game_loop.messages.transfer_stream_to_command_list()?;
        self.logic_update()
    }

    /// `TheRecorder->playbackFile(filename)` (the replay menu's call): the replay's header read, its game options kept as
    /// the recorder's `TheGameInfo`, and `MSG_NEW_GAME` (`GAME_REPLAY`) appended to the message stream. Whether the
    /// file was read.
    pub fn play_replay(&mut self, filename: &[u8]) -> R<bool> {
        if self.cuts.game_loop.recorder.is_none() {
            self.recorder_init();
        }
        self.with_recorder(|r, w| r.playback_file(filename, w)).unwrap_or(Ok(false))
    }

    /// (#98, new) The file `filename` put in the recorder's replay folder on the World's disk (`getReplayDir()` and the
    /// name), as a replay the replay menu plays back is there for upstream's `playbackFile`.
    pub fn put_replay(&mut self, filename: &[u8], bytes: &[u8]) {
        let mut path = RecorderHost::user_data_path(self);
        path.extend_from_slice(b"Replays\\");
        path.extend_from_slice(filename);
        self.save_side.disk.insert(path.to_ascii_lowercase(), bytes.to_vec());
    }

    /// (#98, new) The last replay as the recorder wrote it to the World's disk (`getReplayDir()`,
    /// `getLastReplayFileName()` and `.rep`, `00000000.rep`): what the score screen's *Save Replay* copies. `None`
    /// when no game was recorded. Read only.
    pub fn last_replay(&self) -> Option<&[u8]> {
        let mut path = RecorderHost::user_data_path(self);
        path.extend_from_slice(b"Replays\\");
        path.extend_from_slice(crate::common::recorder::LAST_REPLAY_FILE_NAME);
        path.extend_from_slice(crate::common::recorder::REPLAY_EXTENSION);
        self.save_side.disk.get(&path.to_ascii_lowercase()).map(|b| b.as_slice())
    }

    /// The replay CRC check's outcome so far, during a playback: `(mismatch seen, CRCs still queued)`.
    pub fn replay_crc_state(&self) -> Option<(bool, usize)> {
        let info = self.cuts.game_loop.recorder.as_ref()?.crc_info.as_ref()?;
        Some((info.saw_crc_mismatch, info.data.len()))
    }
}

impl RecorderHost for World {
    fn frame(&self) -> u32 {
        World::frame(self)
    }
    fn is_in_game(&self) -> bool {
        self.cuts.game_loop.game_mode != game_mode::NONE
    }
    fn game_mode(&self) -> i32 {
        self.cuts.game_loop.game_mode
    }
    fn clear_game_data(&mut self) {
        self.cut(CutCall::Host { call: "GameLogic::clearGameData", args: Vec::new(), text: Vec::new() });
        // (~~a cut~~ GitHub #64: translated, `World::clear_game_data`)
        self.clear_game_data(true);
    }
    fn messages(&mut self) -> &mut Messages {
        &mut self.cuts.game_loop.messages
    }
    fn local_player_index(&self) -> i32 {
        // (no local player: upstream dereferences NULL; -1 here)
        self.players.local_index().map_or(-1, |p| p)
    }
    fn player_name_key_is(&mut self, player_index: i32, name: &[u8]) -> Option<bool> {
        let key = self.names.name_to_key(name);
        self.players.get_nth(player_index as _).map(|p| p.name_key as u32 == key.0)
    }
    fn pending_file(&self) -> Vec<u8> {
        self.cuts.new_game.pending_file.clone()
    }
    fn set_pending_file(&mut self, name: &[u8]) {
        self.cuts.new_game.pending_file = name.to_vec();
    }
    fn map_name(&self) -> Vec<u8> {
        self.cuts.new_game.map_name.clone()
    }
    fn user_data_path(&self) -> Vec<u8> {
        // ~~empty~~ (batch I57: `TheGlobalData->getPath_UserData()`, the global data's; empty until it is set, as in the
        // tests' worlds, which keep their replays in `Replays\`)
        self.statics.global_data.the_global_data().map(|g| g.get_path_user_data().to_vec()).unwrap_or_default()
    }
    fn exe_crc(&self) -> u32 {
        self.statics.global_data.the_global_data().map_or(0, |g| g.exe_crc)
    }
    fn ini_crc(&self) -> u32 {
        self.statics.global_data.the_global_data().map_or(0, |g| g.ini_crc)
    }
    fn version_string(&self) -> Vec<u16> {
        Vec::new()
    }
    fn version_time_string(&self) -> Vec<u16> {
        Vec::new()
    }
    fn version_number(&self) -> u32 {
        0
    }
    fn local_time(&mut self) -> [u8; 16] {
        [0; 16]
    }
    fn time(&mut self) -> u32 {
        0
    }
    fn game_logic_random_seed(&self) -> u32 {
        self.random.get_game_logic_random_seed()
    }
    fn init_random(&mut self, seed: u32) {
        self.random.init_random(seed);
    }
    fn create_directory(&mut self, _path: &[u8]) {}
    fn read_file(&mut self, path: &[u8]) -> Option<Vec<u8>> {
        self.save_side.disk.get(&path.to_ascii_lowercase()).cloned()
    }
    fn create_file(&mut self, path: &[u8]) -> bool {
        self.save_side.disk.insert(path.to_ascii_lowercase(), Vec::new());
        true
    }
    fn write_file(&mut self, path: &[u8], bytes: &[u8]) {
        self.save_side.disk.insert(path.to_ascii_lowercase(), bytes.to_vec());
    }
    fn has_network(&self) -> bool {
        self.cuts.game_loop.has_network
    }
    fn network_slot_list(&mut self) -> (Vec<u8>, i32) {
        (Vec::new(), 0)
    }
    fn skirmish_game_info(&mut self) -> Option<&mut GameInfo> {
        self.cuts.new_game.games.iter_mut().find(|(s, _)| *s == GameSource::Skirmish).map(|(_, g)| g)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::game_type::{DrawableId, ObjectId};
    use crate::common::dict::Dict;
    use crate::common::game_info::slot_state;
    use crate::common::message_types::{MSG_CLEAR_GAME_DATA, MSG_DO_MOVETO, MSG_DO_MOVETO_HINT, MSG_NEW_GAME};
    use crate::common::recorder::FRAMES_OFFSET;
    use crate::formats::data_chunk_output::DataChunkOutput;
    use crate::logic::world::player_host_world::tests::world;

    /// A skirmish map file: a flat 30 by 30 height map and the neutral side only (a skirmish's player sides are made from
    /// the game's slots).
    fn skirmish_map(w: &mut World) -> Vec<u8> {
        let mut out = DataChunkOutput::new();
        out.open_data_chunk(b"HeightMapData", crate::logic::terrain_visual::K_HEIGHT_MAP_VERSION_4);
        for v in [30, 30, 0, 1, 30, 30, 900] {
            out.write_int(v);
        }
        out.write_array_of_bytes(&[4u8; 900]);
        out.close_data_chunk();
        // the first player's start: a waypoint (a map object with a `waypointID`) named `Player_1_Start` at (100, 100)
        out.open_data_chunk(b"ObjectsList", crate::logic::map_object::K_OBJECTS_VERSION_3);
        out.open_data_chunk(b"Object", crate::logic::map_object::K_OBJECTS_VERSION_3);
        for r in [100.0, 100.0, 0.0, 0.0] {
            out.write_real(r);
        }
        out.write_int(0);
        out.write_ascii_string(b"*Waypoints/Waypoint");
        let mut props = Dict::new();
        props.set_int(w.names.name_to_key(b"waypointID"), 1).unwrap();
        props.set_ascii_string(w.names.name_to_key(b"waypointName"), b"Player_1_Start").unwrap();
        out.write_dict(&props, &w.names);
        out.close_data_chunk();
        out.close_data_chunk();
        let templates = crate::logic::script_templates::script_templates(&mut w.names);
        let mut sc = crate::logic::sides_list::SidesCtx::new(w.names.clone(), templates);
        let mut d = Dict::new();
        d.set_ascii_string(sc.names.generator.name_to_key(b"playerName"), b"").unwrap();
        d.set_bool(sc.names.generator.name_to_key(b"playerIsHuman"), false).unwrap();
        d.set_ascii_string(sc.names.generator.name_to_key(b"playerFaction"), b"").unwrap();
        sc.sides.add_side(&d);
        crate::logic::sides_list::SidesList::write_sides_data_chunk(&mut out, &mut sc).expect("the sides written");
        out.finish()
    }

    const MAP: &[u8] = b"Maps\\Test\\Test.map";

    /// The test world with the map on its disk, a recorder, and (`skirmish`) the skirmish's options: slot 0 a human of
    /// `FactionAmerica` (player template 0).
    fn skirmish_world(skirmish: bool) -> World {
        let (mut w, _) = world();
        w.cuts.live_teams = true;
        w.cuts.game_loop.game_mode = game_mode::SHELL;
        crate::logic::world::new_game_host_world::tests::multiplayer_colors(&mut w);
        w.sides = crate::logic::sides_list::SidesList::new();
        w.cuts.new_game.map_objects = Vec::new();
        let file = skirmish_map(&mut w);
        w.save_side.disk.insert(MAP.to_ascii_lowercase(), file);
        // `TheMapCache`'s entry for the map, as the engine makes it at start (~~a table: two players, the first one's
        // start~~ batch I56: `updateCache` over the disk; the map has one start waypoint, so one player). The shipped
        // `Maps\MapCache.ini` is empty here: the map is a user map (the user data path is empty, so `Maps\`)
        w.save_side.disk.insert(b"maps\\mapcache.ini".to_vec(), Vec::new());
        w.update_map_cache().expect("the map cache");
        assert_eq!(w.statics.map_cache.find_map(MAP).map(|m| m.num_players), Some(1));
        if skirmish {
            // (as the skirmish menu makes it: `enterGame()`, the slots, then `startGame(0)` when the game is launched)
            let mut g = GameInfo::new(&mut w);
            g.enter_game(&mut w);
            let name: Vec<u16> = "Me".encode_utf16().collect();
            g.slots[0].set_state(slot_state::PLAYER, &name, 0, &mut w);
            g.slots[0].set_player_template(0);
            g.slots[0].color = 0;
            g.slots[0].start_pos = 0;
            g.map_name = MAP.to_vec();
            g.seed = 1234;
            g.start_game(0, &mut w);
            w.cuts.new_game.games.push((GameSource::Skirmish, g));
        }
        w.recorder_init();
        // (GitHub #64: the engine's `init` of the player list and the script engine, as an engine has them; the replay's
        // `playbackFile` clears the shell game, which leaves them so)
        w.init_engine_subsystems().expect("the engine's init");
        w
    }

    /// Integration batch I55 (#14): a skirmish recorded and played back gives the same game. Predicted before the run:
    /// - recording: `MSG_NEW_GAME` (`GAME_SKIRMISH`) starts the recording (`updateRecord` -> `startRecording`), the game
    ///   starts on the next frame, and a CRC message is sent every 100 frames (`TheGameInfo`'s interval; a skirmish with a
    ///   recorder is multiplayer to the recorder). `MSG_CLEAR_GAME_DATA` ends the recording, and the replay file is on the
    ///   World's disk;
    /// - playback, in a World built alike but without the skirmish's options: the replay's header gives back the game
    ///   (its map and slot), `MSG_NEW_GAME` (`GAME_REPLAY`) starts it, and the recorded CRCs are compared with the
    ///   playback's own at every CRC frame: no mismatch, and the frames' CRCs equal the recording's frame by frame.
    ///
    /// (#98) Also, predicted before the run: the World's `last_replay` is the file; `replay_listings` reads it as a
    /// parsed skirmish on the map with local slot 0; the replay menu's path (`MissionChoice::Replay`) waits for that
    /// map and plays the recorded CRCs for 100 frames; bytes that are not a replay are refused. **Result
    /// (2026-11-10):** see the commit's entry in docs/phase2-client.md.
    ///
    /// The order at frame 60 (the tank selected, then `MSG_SELF_DESTRUCT`) is recorded and played back. A second playback,
    /// of the same file with its seed changed (`SD=1234` to `SD=1235`), must be caught as a mismatch: the check can fail.
    ///
    /// Results (2026-11-04): all held, after four corrections to the test's setup and three to the World:
    /// - the setup: the skirmish's options must be entered (`enterGame`) and started (`startGame(0)`), as the skirmish menu
    ///   makes them, or `getLocalSlotNum` is -1; the launch must seed the logic (`InitGameLogicRandom(seed)`, the menu's
    ///   call; without it the two runs differed from frame 0) and set the map name; the slot's start must be fixed (0), as
    ///   the map has only `Player_1_Start`; `playbackFile` takes the name within the replay folder;
    /// - the World: `TheMapCache` and `TheMultiplayerSettings` were answered as absent (a replay's header dereferences
    ///   both), and `ThePlayerTemplateStore`'s count was 0 (no slot list could parse). See the module's docs.
    ///
    /// The recording made the starting Barracks and Tank, sent its CRCs every 100 frames and wrote one replay; the
    /// playback made the same game, compared every CRC with no mismatch, and gave the same CRC as the recording at every one
    /// of the 350 frames; the tank destroyed itself in both. The tampered replay was caught (a mismatch). No live errors.
    #[test]
    fn a_recorded_skirmish_plays_back_in_sync() {
        use crate::logic::world::game_logic_update::CRC_RECALC;
        let frames = 350u32;
        let mut rec = skirmish_world(true);
        // the skirmish menu's launch (SkirmishGameOptionsMenu.cpp:432): `m_mapName`, `startGame(0)` (made above), then
        // `InitGameLogicRandom(TheSkirmishGameInfo->getSeed())`, then `MSG_NEW_GAME` (here put straight on the command list)
        // (GitHub #64: `reallyDoStart` first clears the game the shell is in, `clearGameData(FALSE)`, as the replay's
        // `playbackFile` clears the playing World's)
        assert!(rec.clear_game_data(false));
        rec.cuts.new_game.map_name = MAP.to_vec();
        rec.random.init_game_logic_random(1234);
        rec.push_command(MSG_NEW_GAME, 0, |m| {
            m.append_integer_argument(game_mode::SKIRMISH);
            m.append_integer_argument(1);
            m.append_integer_argument(0);
        });
        let mut recorded = Vec::new();
        let mut tank = None;
        for f in 0..frames {
            if f == 60 {
                // an order, as the client sends it: the tank selected (`MSG_CREATE_SELECTED_GROUP`), then told to destroy
                // itself (`MSG_SELF_DESTRUCT`; the test world's tank has no AI or locomotor, so it cannot be moved)
                let t = rec.objects.ids_in_order().into_iter().find(|o| rec.find_object(*o).is_some_and(|x| x.template().get_name() == b"Tank"));
                let t = t.expect("the starting tank");
                let p = rec.get_controlling_player(t).expect("its player");
                rec.push_command(crate::common::message_types::MSG_CREATE_SELECTED_GROUP, p, |m| {
                    m.append_boolean_argument(true);
                    m.append_object_id_argument(t as u32);
                });
                rec.push_command(crate::common::message_types::MSG_SELF_DESTRUCT, p, |m| m.append_boolean_argument(false));
                tank = Some((t, rec.find_object(t).unwrap().get_position()));
            }
            rec.engine_update().expect("a recorded frame");
            recorded.push((rec.frame(), rec.logic_crc(CRC_RECALC, None).expect("the CRC")));
        }
        rec.push_command(MSG_CLEAR_GAME_DATA, 0, |_| {});
        rec.engine_update().expect("the last frame");
        assert!(rec.cuts.team_live_errors.is_empty(), "{:?}", rec.cuts.team_live_errors);
        let replays: Vec<Vec<u8>> = rec.save_side.disk.keys().filter(|k| k.ends_with(b".rep")).cloned().collect();
        eprintln!("replays {:?}; players {}", replays.iter().map(|r| String::from_utf8_lossy(r).into_owned()).collect::<Vec<_>>(), rec.players.player_count());
        assert_eq!(replays.len(), 1, "one replay written");

        // (#98) the World's last replay is that file; the replay list reads its header and options on a copy of a World
        // (the client's `replay_listings`): a parsed skirmish (local slot 0) on the test map; the replay menu's
        // playback path (`MissionChoice::Replay`: the file put on the World's disk, the map from the options, then
        // `play_replay`) plays the same first 100 frames
        assert_eq!(rec.last_replay(), Some(&rec.save_side.disk[&replays[0]][..]));
        let bytes = rec.save_side.disk[&replays[0]].clone();
        let base = skirmish_world(false);
        let listing = crate::client::replay_menu::replay_listings(&base, &[(b"00000000.rep".to_vec(), bytes.clone())]);
        let l = listing[0].as_ref().expect("the header reads");
        assert!(l.parsed && l.map.eq_ignore_ascii_case(MAP) && l.header.local_player_index == 0, "{:?}", l.map);
        let choice = crate::logic::world::campaign_world::MissionChoice::Replay { filename: b"00000000.rep".to_vec(), bytes: bytes.clone() };
        let pending = crate::logic::world::campaign_world::PendingMission::new(&base, choice, 1).expect("the replay waits for its map");
        assert!(pending.map.eq_ignore_ascii_case(MAP));
        let (mut via_menu, map) = pending.start().expect("the playback starts");
        assert!(map.eq_ignore_ascii_case(MAP));
        for (i, want) in recorded.iter().take(100).enumerate() {
            via_menu.engine_update().expect("a played frame");
            assert_eq!((via_menu.frame(), via_menu.logic_crc(CRC_RECALC, None).expect("the CRC")), *want, "frame index {i}");
        }
        let bad = crate::logic::world::campaign_world::MissionChoice::Replay { filename: b"x.rep".to_vec(), bytes: b"NOTAREPLAY".to_vec() };
        assert!(crate::logic::world::campaign_world::PendingMission::new(&base, bad, 1).is_err());

        let mut play = skirmish_world(false);
        play.save_side.disk.insert(replays[0].clone(), rec.save_side.disk[&replays[0]].clone());
        // (`playbackFile` takes the name within the replay folder, as the replay menu passes it)
        let name = replays[0].strip_prefix(b"replays\\".as_slice()).expect("in the replay folder").to_vec();
        assert!(play.play_replay(&name).expect("the replay read"));
        // (batch I61: `isMultiplayerSession()` is the recorder's `isMultiplayer()`: a playback of a game with occupied slots is
        // one; the World answered the table's false before)
        assert!(crate::logic::world::game_logic_new_game::NewGameHost::sn_is_multiplayer_session(&play));
        let mut played = Vec::new();
        for _ in 0..frames {
            play.engine_update().expect("a played frame");
            played.push((play.frame(), play.logic_crc(CRC_RECALC, None).expect("the CRC")));
        }
        let first = recorded.iter().zip(&played).position(|(a, b)| a != b);
        eprintln!(
            "first difference at index {first:?}: recorded {:?} played {:?}; players {} vs {}; objects {} vs {}",
            first.map(|i| recorded[i]),
            first.map(|i| played[i]),
            rec.players.player_count(),
            play.players.player_count(),
            rec.objects.ids_in_order().len(),
            play.objects.ids_in_order().len()
        );
        let state = play.replay_crc_state();
        eprintln!("crc state {state:?}; errors {:?}", play.cuts.team_live_errors);
        assert!(play.cuts.team_live_errors.is_empty(), "{:?}", play.cuts.team_live_errors);
        assert_eq!(state.map(|s| s.0), Some(false), "no CRC mismatch");
        assert_eq!(played, recorded, "the same CRC every frame");
        // the order was played back: the tank is gone in both
        let (t, start) = tank.expect("the order was given");
        let gone = |w: &World| w.find_object(t).is_none_or(|o| o.is_effectively_dead());
        eprintln!("tank {t} at {start:?}: gone recorded {}, played {}; objects {} vs {}", gone(&rec), gone(&play), rec.objects.ids_in_order().len(), play.objects.ids_in_order().len());
        assert!(gone(&rec), "the recorded tank destroyed itself");
        assert!(gone(&play), "the played one did too");

        // the check can fail: the same replay with its random seed changed (`SD=1234` -> `SD=1235`) plays a different game,
        // and the recorded CRCs no longer match
        let mut bad = rec.save_side.disk[&replays[0]].clone();
        let at = bad.windows(7).position(|w| w == b"SD=1234").expect("the seed in the header");
        bad[at + 6] = b'5';
        let mut tampered = skirmish_world(false);
        tampered.save_side.disk.insert(replays[0].clone(), bad);
        assert!(tampered.play_replay(&name).expect("the tampered replay read"));
        for _ in 0..frames {
            tampered.engine_update().expect("a tampered frame");
        }
        assert_eq!(tampered.replay_crc_state().map(|s| s.0), Some(true), "a changed seed is caught");
    }

    /// GitHub #110: the quit menu's *Restart* (`restartMissionMenu`'s engine half, [`World::restart_mission`]) of a
    /// recorded skirmish and of a replay's playback, through the flow's choice (`MissionChoice::Restart`).
    ///
    /// **Predicted before the run** (the skirmish recorded as [`a_recorded_skirmish_plays_back_in_sync`] records it, seed
    /// 1234, 120 frames):
    /// - the skirmish restarted with what the menu reads (`GAME_SKIRMISH`, difficulty 1, no rank points, the map, no
    ///   replay, recording): the recording stopped and its file written (~~one `.rep` on the disk~~ **Corrected
    ///   2026-11-10 (#110), first run:** the file is on the disk before the stop, written as it is made; the stop writes
    ///   its header's frame count, 0 before and ~~120~~ 119 after: the logic's frame, which is 0 on the first of the 120
    ///   updates, as `recorded` shows), the game cleared and
    ///   started again as a skirmish (`TheSkirmishGameInfo` kept), recording again, no live error; its CRCs differ from the
    ///   first start's from the first frame, because the restart seeds with `InitRandom(0)` and the first start with the
    ///   game's seed (upstream's quirk, kept);
    /// - the replay of that game, played 50 frames, then restarted (`GAME_REPLAY`, the recorder's current file name): the
    ///   playback starts again from its header, and its first 100 frames give the recording's CRCs frame by frame.
    ///
    /// **Result (2026-11-10):** as predicted but for the corrected point above (two runs to get it right): the skirmish's
    /// restart differs from frame 0 (CRC `1350010189` against the first start's `894748852`), restarts as a recording
    /// skirmish with no live error; the replay's restart gives the recording's first 100 CRCs.
    #[test]
    fn a_restart_plays_a_replay_again_and_restarts_a_skirmish() {
        use crate::logic::world::campaign_world::{MissionChoice, PendingMission, Restart};
        use crate::logic::world::game_logic_update::CRC_RECALC;
        let mut rec = skirmish_world(true);
        assert!(rec.clear_game_data(false));
        rec.cuts.new_game.map_name = MAP.to_vec();
        rec.random.init_game_logic_random(1234);
        rec.push_command(MSG_NEW_GAME, 0, |m| {
            m.append_integer_argument(game_mode::SKIRMISH);
            m.append_integer_argument(1);
            m.append_integer_argument(0);
        });
        let run = |w: &mut World, n: usize| -> Vec<(u32, u32)> {
            (0..n)
                .map(|_| {
                    w.engine_update().expect("a frame");
                    (w.frame(), w.logic_crc(CRC_RECALC, None).expect("the CRC"))
                })
                .collect()
        };
        let recorded = run(&mut rec, 120);
        let reps = |w: &World| w.save_side.disk.keys().filter(|k| k.ends_with(b".rep")).count();
        // (1) the skirmish restarted
        let again = rec.clone();
        assert_eq!(again.cuts.game_loop.recorder.as_ref().map(|r| r.mode), Some(RecorderMode::Record));
        // (the recording is on the disk as it is made, `fflush` each frame: what the stop changes is its header's frame
        // count, `logGameEnd`)
        assert_eq!(reps(&again), 1);
        let rep = |w: &World| w.save_side.disk.iter().find(|(k, _)| k.ends_with(b".rep")).map(|(_, b)| b.clone()).expect("the replay");
        let frames_of = |b: &[u8]| u32::from_le_bytes(b[FRAMES_OFFSET as usize..FRAMES_OFFSET as usize + 4].try_into().unwrap());
        let before = frames_of(&rep(&again));
        let r = Restart { game_mode: game_mode::SKIRMISH, difficulty: 1, rank_points: 0, fps: 0, map_name: MAP.to_vec(), replay_file: Vec::new(), stop_recording: true };
        let (mut again, map) = PendingMission::new(&again, MissionChoice::Restart(r), 1).expect("the restart").start().expect("the start");
        assert_eq!(map, MAP);
        assert_eq!(reps(&again), 1, "the stopped recording's file");
        eprintln!("the recording's frame count: {before} before the stop, {} after", frames_of(&rep(&again)));
        assert_eq!((before, frames_of(&rep(&again))), (0, recorded[119].0), "the stop wrote the game's length");
        assert_eq!(recorded[119].0, 119);
        let restarted = run(&mut again, 120);
        assert_eq!(again.cuts.game_loop.game_mode, game_mode::SKIRMISH);
        assert_eq!(again.cuts.game_loop.recorder.as_ref().map(|r| r.mode), Some(RecorderMode::Record));
        assert!(again.cuts.team_live_errors.is_empty(), "{:?}", again.cuts.team_live_errors);
        let first = recorded.iter().zip(&restarted).position(|(a, b)| a != b);
        eprintln!("the skirmish's restart first differs at {first:?}: {:?} vs {:?}", first.map(|i| recorded[i]), first.map(|i| restarted[i]));
        assert_eq!(first, Some(0), "InitRandom(0), not the game's seed");
        // (2) the replay restarted
        rec.push_command(MSG_CLEAR_GAME_DATA, 0, |_| {});
        rec.engine_update().expect("the last frame");
        let path = rec.save_side.disk.keys().find(|k| k.ends_with(b".rep")).expect("the replay").clone();
        let name = path.strip_prefix(b"replays\\".as_slice()).expect("in the replay folder").to_vec();
        let mut play = skirmish_world(false);
        play.save_side.disk.insert(path.clone(), rec.save_side.disk[&path].clone());
        assert!(play.play_replay(&name).expect("the replay read"));
        run(&mut play, 50);
        let replay_file = play.cuts.game_loop.recorder.as_ref().map(|r| r.current_replay_filename()).unwrap_or_default();
        assert_eq!(replay_file, name);
        let r = Restart { game_mode: game_mode::REPLAY, difficulty: 1, rank_points: 0, fps: 0, map_name: MAP.to_vec(), replay_file, stop_recording: false };
        let (mut replayed, _) = PendingMission::new(&play, MissionChoice::Restart(r), 1).expect("the restart").start().expect("the playback");
        assert_eq!(run(&mut replayed, 100), recorded[..100], "the replay from its start again");
        assert!(replayed.cuts.team_live_errors.is_empty(), "{:?}", replayed.cuts.team_live_errors);
    }

    /// The client's selection facts read from a World, for the local player `p`; its messages go on the World's command list.
    struct ClientOfWorld {
        w: World,
        p: i32,
        /// The messages the client sent to the logic, which the command translator observes (C2b).
        sent: Vec<crate::common::message_stream::MessageType>,
        /// The hints the command translator made.
        hints: Vec<crate::common::message_stream::MessageType>,
    }

    impl crate::client::selection_info::SelectionInfoHost for ClientOfWorld {
        fn drawable_object(&self, draw: DrawableId) -> Option<ObjectId> {
            self.w.find_object(draw).map(|_| draw)
        }
        fn is_locally_controlled(&self, obj: ObjectId) -> bool {
            self.w.get_controlling_player(obj) == Some(self.p)
        }
        fn is_kind_of(&self, obj: ObjectId, k: crate::common::kind_of::KindOfType) -> bool {
            self.w.find_object(obj).is_some_and(|o| o.is_kind_of(k))
        }
        fn relationship_to_local_player(&self, obj: ObjectId) -> crate::common::game_common::Relationship {
            if self.is_locally_controlled(obj) {
                crate::common::game_common::Relationship::Allies
            } else {
                crate::common::game_common::Relationship::Enemies
            }
        }
        fn can_local_player_garrison(&mut self, _obj: ObjectId) -> bool {
            false
        }
        fn force_fire_or_move(&self) -> bool {
            false
        }
        fn prefer_selection(&self) -> bool {
            false
        }
        fn use_alternate_mouse(&self) -> bool {
            false
        }
    }

    impl crate::client::in_game_ui::SelectionUiHost for ClientOfWorld {
        fn logic_frame(&self) -> u32 {
            self.w.frame()
        }
        fn append_message_bool(&mut self, t: crate::common::message_stream::MessageType, b: bool) {
            self.w.push_command(t, self.p, |m| m.append_boolean_argument(b));
        }
        fn on_selection_changed(&mut self, _draw: DrawableId, _selected: bool) {}
    }

    impl crate::client::selection_xlat::SelectionXlatHost for ClientOfWorld {
        fn quit_menu_visible(&self) -> bool {
            false
        }
        fn gui_command_pending(&self) -> bool {
            false
        }
        // the fixture's templates carry no `SELECTABLE` (the shipped ones do): every object is picked; the pick filter is
        // `selection_xlat::tests`'
        fn matches_pick_kinds(&self, _draw: DrawableId) -> bool {
            true
        }
        fn is_selectable(&self, _draw: DrawableId) -> bool {
            true
        }
        fn contained_by(&self, _obj: ObjectId) -> Option<ObjectId> {
            None
        }
        fn is_enclosing_container_for(&mut self, _container: ObjectId, _obj: ObjectId) -> Option<bool> {
            None
        }
        fn object_drawable(&self, obj: ObjectId) -> Option<DrawableId> {
            Some(obj)
        }
        fn fogged_for_local_player(&self, _obj: ObjectId) -> bool {
            false
        }
        fn stealthed_and_undetected(&self, _obj: ObjectId) -> bool {
            false
        }
        fn append_message(&mut self, t: crate::common::message_stream::MessageType, create: Option<bool>, objects: &[ObjectId]) {
            self.sent.push(t);
            self.w.push_command(t, self.p, |m| {
                if let Some(b) = create {
                    m.append_boolean_argument(b);
                }
                for &o in objects {
                    m.append_object_id_argument(o as u32);
                }
            });
        }
        fn record_drag_selection(&mut self) {}
        fn clear_attack_move_to_mode(&mut self) {}
    }

    impl crate::client::selection_groups::SelectionGroupsHost for ClientOfWorld {
        fn all_drawables(&self) -> Vec<DrawableId> {
            self.w.objects.ids_in_order()
        }
        fn drawables_on_screen(&self) -> Vec<DrawableId> {
            self.w.objects.ids_in_order()
        }
        fn pick_selectable_at_click(&self) -> Option<DrawableId> {
            None
        }
        fn is_mass_selectable(&self, _obj: ObjectId) -> bool {
            true
        }
        fn object_template(&self, obj: ObjectId) -> u32 {
            self.w.find_object(obj).map_or(0, |o| u32::from(o.template().get_template_id()))
        }
        fn is_equivalent_to(&self, obj: ObjectId, template: u32) -> bool {
            self.object_template(obj) == template
        }
        fn is_car_bomb(&self, _obj: ObjectId) -> bool {
            false
        }
        fn is_off_map(&self, _obj: ObjectId) -> bool {
            false
        }
        fn hotkey_squad_live_objects(&mut self, _group: i32) -> Option<Vec<ObjectId>> {
            None
        }
        fn look_at(&mut self, _obj: ObjectId) {}
        fn ui_message(&mut self, _label: &[u8], _count: Option<i32>) {}
        fn append_message_drawable(&mut self, t: crate::common::message_stream::MessageType, draw: DrawableId) {
            self.sent.push(t);
            self.w.push_command(t, self.p, |m| m.append_drawable_id_argument(draw as u32));
        }
    }

    /// The command translator's questions, answered by the World (`client::command_world`).
    /// (#51) The recorder's client plays no voices.
    impl crate::client::unit_voice::UnitVoiceHost for ClientOfWorld {}

    impl crate::client::command_xlat::CommandXlatHost for ClientOfWorld {
        fn object_is_masked(&self, obj: ObjectId) -> bool {
            self.w.find_object(obj).is_some_and(|o| o.test_status(crate::common::object_status_types::OBJECT_STATUS_MASKED))
        }
        fn object_position(&self, obj: ObjectId) -> crate::common::base_type::Coord3D {
            self.w.find_object(obj).map(|o| o.get_position()).unwrap_or_default()
        }
        fn object_relationship(&self, src: ObjectId, target: ObjectId) -> crate::common::game_common::Relationship {
            if self.w.get_controlling_player(src) == self.w.get_controlling_player(target) {
                crate::common::game_common::Relationship::Allies
            } else {
                crate::common::game_common::Relationship::Enemies
            }
        }
        fn object_can_do_action(&mut self, src: Option<ObjectId>, action: crate::client::command_xlat::ActionType, target: Option<ObjectId>, additional: bool) -> bool {
            crate::client::command_world::object_can_do_action(&mut self.w, src, action, target, additional)
        }
        fn object_can_attack(&mut self, src: Option<ObjectId>, target: Option<ObjectId>, forced: bool) -> i32 {
            crate::client::command_world::object_can_attack(&mut self.w, src, target, forced)
        }
        fn object_can_force_attack(&mut self, src: ObjectId, victim: Option<ObjectId>, pos: Option<crate::common::base_type::Coord3D>) -> i32 {
            crate::client::command_world::object_can_force_attack(&mut self.w, src, victim, pos)
        }
        fn object_can_override_special_power_destination(&mut self, src: Option<ObjectId>, loc: &crate::common::base_type::Coord3D, sp: i32) -> bool {
            crate::client::command_world::object_can_override_special_power_destination(&mut self.w, src, loc, sp)
        }
        fn is_salvage_crate(&mut self, obj: ObjectId) -> bool {
            self.w.is_salvage_crate(obj)
        }
        fn shroud_clear_for_local_player(&mut self, pos: &crate::common::base_type::Coord3D) -> bool {
            self.w.partition.get_shroud_status_for_player(self.p, pos) == crate::common::game_common::CellShroudStatus::Clear
        }
        fn object_has_ai(&self, obj: ObjectId) -> bool {
            self.w.find_object(obj).is_some_and(|o| o.ai.is_some())
        }
        fn quick_path_available(&mut self, obj: ObjectId, pos: &crate::common::base_type::Coord3D) -> bool {
            crate::logic::world::build_assistant::BuildAssistantHost::ai_is_quick_path_available(&mut self.w, obj, *pos)
        }
        fn has_cliff_locomotor(&mut self, obj: ObjectId) -> bool {
            let call = crate::logic::world::module_call::ModuleCall::AiHasLocomotorForSurface(crate::logic::locomotor::LOCOMOTORSURFACE_CLIFF as u32);
            matches!(self.w.ai_ask(obj, call), Some(crate::logic::world::module_call::ModuleReply::Bool(true)))
        }
        fn is_cliff_cell(&mut self, x: f32, y: f32) -> bool {
            crate::logic::ai::pathfinder::PathfinderHost::is_cliff_cell(&self.w, x, y)
        }
        fn special_power_buttons(&mut self, obj: ObjectId) -> Vec<crate::client::command_xlat::GuiButton> {
            crate::client::command_world::special_power_buttons(&self.w, obj)
        }
        fn object_can_do_special_power(&mut self, src: Option<ObjectId>, button: &crate::client::command_xlat::GuiButton, mode: crate::client::command_xlat::TargetMode) -> bool {
            crate::client::command_world::object_can_do_special_power(&mut self.w, src, button, mode)
        }
        fn object_can_fire_weapon(&mut self, src: Option<ObjectId>, button: &crate::client::command_xlat::GuiButton, mode: crate::client::command_xlat::TargetMode) -> bool {
            crate::client::command_world::object_can_fire_weapon(&mut self.w, src, button, mode)
        }
        fn most_ready_shortcut_special_power(&mut self, sp: i32) -> Option<ObjectId> {
            crate::logic::world::player_objects::player_find_most_ready_shortcut_special_power_of_type(&mut self.w, self.p, sp).ok().flatten()
        }
        fn has_overridable_destination_special_power(&mut self, obj: ObjectId) -> bool {
            self.w.find_special_power_with_overridable_destination(obj).is_some()
        }
        fn append_command(&mut self, t: crate::common::message_stream::MessageType, args: &[crate::client::command_xlat::CmdArg]) {
            use crate::client::command_xlat::CmdArg;
            self.sent.push(t);
            self.w.push_command(t, self.p, |m| {
                for a in args {
                    match *a {
                        CmdArg::Int(i) => m.append_integer_argument(i),
                        CmdArg::Real(r) => m.append_real_argument(r),
                        CmdArg::Bool(b) => m.append_boolean_argument(b),
                        CmdArg::Object(o) => m.append_object_id_argument(o as u32),
                        CmdArg::Location(l) => m.append_location_argument(l),
                    }
                }
            });
        }
        fn append_hint(&mut self, t: crate::common::message_stream::MessageType, _args: &[crate::client::command_xlat::CmdArg]) {
            self.hints.push(t);
        }
        fn double_click_attack_move(&self) -> bool {
            false
        }
        fn record_double_click_attack_move_order_given(&mut self) {}
    }

    /// The control bar's questions (C2c), from the World where this file's tests ask them; the others answer as noted.
    impl crate::client::control_bar::ControlBarHost for ClientOfWorld {
        fn can_make_unit(&mut self, builder: Option<ObjectId>, what: Option<u16>) -> crate::logic::world::build_assistant::CanMakeType {
            use crate::logic::world::build_assistant::{build_assistant_can_make_unit, CanMakeType};
            match (builder, what) {
                (Some(b), Some(t)) => build_assistant_can_make_unit(&mut self.w, b, t.into()).unwrap_or(CanMakeType::NoPrereq),
                _ => CanMakeType::NoPrereq,
            }
        }
        fn eva_insufficient_funds(&mut self) {}
        fn request_unique_unit_id(&mut self, factory: ObjectId) -> Option<u32> {
            let pu = self.w.get_production_update_interface(factory)?;
            Some(self.w.module_call(factory, pu, crate::logic::world::module_call::ModuleCall::ProductionRequestUniqueUnitId).as_int(0) as u32)
        }
        // (the upgrade and science buttons are not pressed in these tests)
        fn can_afford_upgrade(&mut self, _upgrade: u32) -> bool {
            false
        }
        fn can_queue_upgrade(&mut self, _obj: ObjectId, _upgrade: u32) -> Option<crate::logic::world::build_assistant::CanMakeType> {
            None
        }
        fn has_upgrade(&mut self, _obj: ObjectId, _upgrade: u32) -> bool {
            false
        }
        fn affected_by_upgrade(&mut self, _obj: ObjectId, _upgrade: u32) -> bool {
            false
        }
        fn mark_single_use_command_used(&mut self, _obj: ObjectId) {}
        fn set_special_power_construction_button(&mut self, _obj: ObjectId, _button: usize) {}
        fn local_player_objects(&mut self) -> Vec<ObjectId> {
            let p = self.p;
            self.w.objects.ids_in_order().into_iter().filter(|&o| self.w.get_controlling_player(o) == Some(p)).collect()
        }
        fn object_exists(&self, obj: ObjectId) -> bool {
            self.w.find_object(obj).is_some()
        }
        fn has_science(&mut self, _st: i32) -> bool {
            false
        }
        fn has_prereqs_for_science(&mut self, _st: i32) -> bool {
            false
        }
        fn science_purchase_cost(&mut self, _st: i32) -> i32 {
            0
        }
        fn science_purchase_points(&mut self) -> i32 {
            0
        }
        fn switch_to_context_none(&mut self) {}
        fn placement_view_angle(&self, build: Option<u16>) -> f32 {
            build.and_then(|t| self.w.statics.thing_factory.find_by_template_id(t)).map_or(0.0, |t| t.data.get_placement_view_angle())
        }
    }

    /// Placement's questions (C2c part 2): this test view maps a pixel (x, y) to the ground at (x, y, 0).
    impl crate::client::place_event::PlaceHost for ClientOfWorld {
        fn screen_to_terrain(&mut self, pixel: crate::common::base_type::ICoord2D) -> crate::common::base_type::Coord3D {
            crate::common::base_type::Coord3D { x: pixel.x as f32, y: pixel.y as f32, z: 0.0 }
        }
        fn is_line_build_template(&self, build: u16) -> bool {
            crate::logic::world::build_assistant::build_assistant_is_line_build_template(&self.w, build.into())
        }
        fn is_location_legal_to_build(&mut self, world: crate::common::base_type::Coord3D, build: u16, angle: f32, builder: Option<ObjectId>) -> crate::logic::world::build_assistant::LegalBuildCode {
            use crate::logic::world::build_assistant as ba;
            let flags = ba::USE_QUICK_PATHFIND | ba::TERRAIN_RESTRICTIONS | ba::CLEAR_PATH | ba::NO_OBJECT_OVERLAP | ba::SHROUD_REVEALED | ba::IGNORE_STEALTHED | ba::FAIL_STEALTHED_WITHOUT_FEEDBACK;
            ba::build_assistant_is_location_legal_to_build(&mut self.w, world, build.into(), angle, flags, builder, None)
        }
        // (no construction special is placed in these tests)
        fn special_power_construction_button(&mut self, _builder: ObjectId) -> Option<crate::client::command_xlat::GuiButton> {
            None
        }
    }

    /// C2a (#6): a click in the client makes the player's selected group in the logic. Predicted from SelectionXlat.cpp and
    /// GameLogicDispatch.cpp, in the recorder's skirmish (the player starts with a Barracks and a Tank; drawable ids are the
    /// object ids here, as the client gives no picking yet):
    /// - a click on the tank: `MSG_DESTROY_SELECTED_GROUP`, then `MSG_CREATE_SELECTED_GROUP(true, tank)`; after the next
    ///   frame the player's current selection is `[tank]`;
    /// - a drag over both: the building is dropped (the tank is selectable), so the selection is `[tank]` again;
    /// - a click on the Barracks alone: the selection is `[barracks]`.
    ///
    /// Results (2026-11-04): ~~the first click selects the tank~~ **Corrected:** the first run picked nothing (`Keep`): the
    /// fixture's templates carry no `SELECTABLE` kind, so `addDrawableToList` refuses them, as upstream would. The host now
    /// picks every object (the filter is tested in `selection_xlat::tests`); with it, the three clicks gave the
    /// selections `[tank]`, `[tank]` and `[barracks]`, as predicted, with no live errors.
    #[test]
    fn a_click_in_the_client_makes_the_players_selected_group() {
        use crate::client::in_game_ui::InGameUiSelection;
        use crate::client::selection_xlat::{Disposition, SelectionTranslator};
        let mut w = skirmish_world(true);
        w.cuts.new_game.map_name = MAP.to_vec();
        w.random.init_game_logic_random(1234);
        w.push_command(MSG_NEW_GAME, 0, |m| {
            m.append_integer_argument(game_mode::SKIRMISH);
            m.append_integer_argument(1);
            m.append_integer_argument(0);
        });
        for _ in 0..5 {
            w.engine_update().expect("a frame");
        }
        let named = |w: &World, n: &[u8]| w.objects.ids_in_order().into_iter().find(|o| w.find_object(*o).is_some_and(|x| x.template().get_name() == n));
        let tank = named(&w, b"Tank").expect("the tank");
        let barracks = named(&w, b"Barracks").expect("the barracks");
        let p = w.get_controlling_player(tank).expect("its player");
        let mut c = ClientOfWorld { w, p, sent: Vec::new(), hints: Vec::new() };
        let (mut x, mut ui) = (SelectionTranslator::default(), InGameUiSelection::new());
        let cmd = crate::client::command_xlat::CommandTranslator::default();
        let selection = |c: &mut ClientOfWorld| {
            c.w.engine_update().expect("a frame");
            c.w.players.get_nth(c.p).and_then(|pl| pl.current_selection.as_ref()).map(|s| s.object_ids.clone()).unwrap_or_default()
        };
        assert_eq!(x.left_click(&[tank], true, &cmd, &mut ui, &mut c), Disposition::Destroy);
        assert_eq!(selection(&mut c), vec![tank]);
        assert_eq!(x.left_click(&[barracks, tank], false, &cmd, &mut ui, &mut c), Disposition::Destroy);
        assert_eq!(selection(&mut c), vec![tank]);
        assert_eq!(x.left_click(&[barracks], true, &cmd, &mut ui, &mut c), Disposition::Destroy);
        assert_eq!(selection(&mut c), vec![barracks]);
        assert!(c.w.cuts.team_live_errors.is_empty(), "{:?}", c.w.cuts.team_live_errors);
    }

    /// C2b (#6): clicks order the selected units through the command translator and the logic. In the gate World (every
    /// switch on; the recorder's skirmish Tank has no AI, so with the logic live `AIGroup::add` refuses it and it cannot be
    /// selected, as upstream refuses it), player 1 has a Runner at (40, 50) and a Soldier at (40, 300); player 2 a Soldier at
    /// (360, 300), 320 away, beyond the Soldiers' 150 vision. Predicted from CommandXlat.cpp, InGameUI.cpp, ActionManager.cpp
    /// and GameLogicDispatch.cpp:
    /// - a click on the Runner selects it (`MSG_DESTROY_SELECTED_GROUP`, `MSG_CREATE_SELECTED_GROUP`), so a group exists;
    /// - the mouse over the ground at (200, 50) hints `MSG_DO_MOVETO_HINT`; a click there is actioned, destroyed, and sends
    ///   `MSG_DO_MOVETO(200, 50)`; over 90 frames the Runner's distance to the point falls;
    /// - with the Soldier selected, the enemy is a context command (`EVALUATE_ONLY` gives `MSG_DO_ATTACK_OBJECT`, the action
    ///   manager's `getCanAttackObject` answering possible after moving), so the selection translator keeps the click rather
    ///   than selecting the enemy; the hint is `MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT`; the click sends
    ///   `MSG_DO_ATTACK_OBJECT(enemy)`; over 90 frames the Soldier closes on it (the enemy, beyond its vision, stays);
    /// - a click on the player's own Runner with the Soldier selected is no context command: it selects the Runner.
    ///
    /// Results (2026-11-05): all held, after three corrections to the test's World, none to the code:
    /// - ~~the recorder's skirmish~~: its Tank has no AI, so with the logic live `AIGroup::add` refuses it (the selection
    ///   was `[]`) and it cannot move; the test moved to the gate World;
    /// - the gate World was not a game in progress (`GAME_NONE`): `logicMessageDispatcher` makes the selection's group only
    ///   `isInGame()`, so the move reached no group (the selection messages are handled either way). A control, the same
    ///   move given to a group directly, drove the Runner 160 -> 75.2;
    /// - the gate's units are not `SELECTABLE`: `Squad::getLiveObjects` keeps only selectable objects, upstream and here, so
    ///   the order's group was empty. The test makes `SELECTABLE` copies of the Runner and the Soldier (the gate scenario,
    ///   and its CRCs, are unchanged).
    ///
    /// With them: the hints and messages were as predicted; the Runner's distance to its goal went 160 -> 75.2 in 90 frames
    /// (30/s for 3 s, less the start), and the Soldier's to the enemy 320 -> 235.1 while the enemy did not move; the click
    /// on the enemy was kept, the click on the Runner selected it; no live errors.
    #[test]
    fn clicks_order_the_selected_units() {
        use crate::client::command_xlat::{CommandEvaluateType, CommandTranslator};
        use crate::client::in_game_ui::InGameUiSelection;
        use crate::client::selection_xlat::{Disposition, SelectionTranslator};
        use crate::common::base_type::Coord3D;
        use crate::common::message_types::{MSG_DO_ATTACK_OBJECT, MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT};
        use crate::logic::world::gate_scenario::{gate_map_world_with, make, INI};
        use crate::logic::world::team_ops::factory_find_team;
        // the gate's Runner and Soldier, `SELECTABLE` as the shipped units are (`Squad::getLiveObjects` keeps only selectable
        // objects, so an order's group would be empty without it)
        let block = |name: &str| {
            let start = INI.find(&format!("Object {name}\n")).expect("the template");
            let end = start + INI[start..].find("\nEnd\n").expect("its end") + 5;
            INI[start..end].replacen(&format!("Object {name}"), &format!("Object Click{name}"), 1).replacen("KindOf = VEHICLE", "KindOf = VEHICLE SELECTABLE", 1)
        };
        let mut w = gate_map_world_with(&(block("Runner") + "\n" + &block("Soldier") + "\n"));
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let enemy_team = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
        let runner = make(&mut w, b"ClickRunner", team, Coord3D { x: 40.0, y: 50.0, z: 0.0 });
        let soldier = make(&mut w, b"ClickSoldier", team, Coord3D { x: 40.0, y: 300.0, z: 0.0 });
        let enemy = make(&mut w, b"ClickSoldier", enemy_team, Coord3D { x: 360.0, y: 300.0, z: 0.0 });
        w.pathfinder_op(crate::logic::world::pathfinder_host_world::PfOp::NewMap);
        // the gate World is made directly, not started by `startNewGame`: a game in progress (`isInGame()`), or the dispatcher
        // makes no group for a command
        w.cuts.game_loop.game_mode = game_mode::SKIRMISH;
        let p = w.get_controlling_player(runner).expect("its player");
        let mut c = ClientOfWorld { w, p, sent: Vec::new(), hints: Vec::new() };
        let (mut x, mut ui, mut cmd) = (SelectionTranslator::default(), InGameUiSelection::new(), CommandTranslator::default());
        let frame = |c: &mut ClientOfWorld, cmd: &mut CommandTranslator| {
            for t in c.sent.drain(..) {
                cmd.observe_message(t);
            }
            c.w.engine_update().expect("a frame");
        };
        let selection = |c: &ClientOfWorld| c.w.players.get_nth(c.p).and_then(|pl| pl.current_selection.as_ref()).map(|s| s.object_ids.clone()).unwrap_or_default();
        let dist = |c: &ClientOfWorld, a: ObjectId, b: Coord3D| {
            let at = c.w.find_object(a).expect("the unit").get_position();
            petir::Real::sqrt(petir::Real::powi(at.x - b.x, 2) + petir::Real::powi(at.y - b.y, 2))
        };

        assert_eq!(x.left_click(&[runner], true, &cmd, &mut ui, &mut c), Disposition::Destroy);
        frame(&mut c, &mut cmd);
        assert_eq!(selection(&c), vec![runner]);
        assert!(cmd.team_exists);
        let goal = Coord3D { x: 200.0, y: 50.0, z: 0.0 };
        assert_eq!(cmd.mouse_over(None, &goal, &mut ui, &mut c), MSG_DO_MOVETO_HINT, "hints {:?}", c.hints);
        assert!(cmd.click(false, true, &goal, None, &mut ui, &mut c));
        assert_eq!(c.sent, vec![MSG_DO_MOVETO]);
        let before = dist(&c, runner, goal);
        for _ in 0..90 {
            frame(&mut c, &mut cmd);
        }
        let after = dist(&c, runner, goal);
        eprintln!("C2b: the Runner's distance to its goal {before} -> {after} after 90 frames");
        assert!(after < before, "the Runner drove toward the point: {before} -> {after}");

        assert_eq!(x.left_click(&[soldier], true, &cmd, &mut ui, &mut c), Disposition::Destroy);
        frame(&mut c, &mut cmd);
        assert_eq!(selection(&c), vec![soldier]);
        let at = c.w.find_object(enemy).expect("the enemy").get_position();
        assert_eq!(cmd.evaluate_context_command(Some(enemy), &at, CommandEvaluateType::EvaluateOnly, &mut ui, &mut c), MSG_DO_ATTACK_OBJECT);
        assert_eq!(cmd.mouse_over(Some(enemy), &at, &mut ui, &mut c), MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT);
        assert_eq!(x.left_click(&[enemy], true, &cmd, &mut ui, &mut c), Disposition::Keep, "the enemy is not selected");
        assert!(cmd.click(false, true, &at, Some(enemy), &mut ui, &mut c));
        assert_eq!(c.sent, vec![MSG_DO_ATTACK_OBJECT]);
        let before = dist(&c, soldier, at);
        for _ in 0..90 {
            frame(&mut c, &mut cmd);
        }
        let after = dist(&c, soldier, at);
        let moved = dist(&c, enemy, at);
        eprintln!("C2b: the Soldier's distance to the enemy's start {before} -> {after}; the enemy moved {moved}");
        assert!(after < before, "the Soldier closed on the enemy: {before} -> {after}");

        let at = c.w.find_object(runner).expect("the runner").get_position();
        assert_eq!(cmd.evaluate_context_command(Some(runner), &at, CommandEvaluateType::EvaluateOnly, &mut ui, &mut c), crate::common::message_types::MSG_INVALID);
        assert_eq!(x.left_click(&[runner], true, &cmd, &mut ui, &mut c), Disposition::Destroy);
        frame(&mut c, &mut cmd);
        assert_eq!(selection(&c), vec![runner]);
        assert!(c.w.cuts.team_live_errors.is_empty(), "{:?}", c.w.cuts.team_live_errors);
    }

    /// C2c (#6): a control bar button orders through the logic. In the gate World (every switch on, a game in progress), player
    /// 1's Caster at (60, 60) (I23's: an `OCLSpecialPower` whose OCL drops a Pup at the target, reload 0; its set holds
    /// `Command_Drop`, a `SPECIAL_POWER` needing a position, and `Command_Halt`, a stop), made `SELECTABLE` and given a
    /// vision of 250, as shipped units have both. Predicted from
    /// ControlBarCommandProcessing.cpp, CommandXlat.cpp and the logic:
    /// - the Caster selected, pressing `Command_Drop` makes it the pending GUI command and sends nothing;
    /// - the mouse over the ground at (200, 200) hints `MSG_VALID_GUICOMMAND_HINT`; the click sends
    ///   `MSG_DO_SPECIAL_POWER_AT_LOCATION` and clears the GUI command; a frame later one Pup stands at (200, 200);
    /// - pressing `Command_Halt` sends `MSG_DO_STOP` at once.
    ///
    /// Results (2026-11-05): all held, after one correction to the test's Caster. ~~Without a vision~~ the first run hinted
    /// `MSG_INVALID_GUICOMMAND_HINT`: I23's Caster has no `VisionRange`, so the whole map was shrouded for player 1 (checked:
    /// `Shrouded` at the Caster and at the target), and `canDoSpecialPowerAtLocation` refuses a shrouded target for a damaging
    /// power (`SPECIAL_DAISY_CUTTER`), upstream and here. With a vision of 250 the hint was valid, the click sent the power,
    /// and one Pup stood at (200.0, 200.0) a frame later; the halt sent `MSG_DO_STOP`; no live errors.
    #[test]
    fn a_control_bar_button_orders_through_the_logic() {
        use crate::client::command_xlat::CommandTranslator;
        use crate::client::control_bar::{process_command_ui, CbCommandStatus, PressedControl};
        use crate::client::in_game_ui::InGameUiSelection;
        use crate::client::selection_xlat::{Disposition, SelectionTranslator};
        use crate::common::base_type::Coord3D;
        use crate::common::message_types::{MSG_DO_SPECIAL_POWER_AT_LOCATION, MSG_DO_STOP, MSG_VALID_GUICOMMAND_HINT};
        use crate::logic::world::gate_scenario::CASTER_INI as CASTER;
        use crate::logic::world::gate_scenario::{gate_map_world_with, make};
        use crate::logic::world::team_ops::factory_find_team;
        let ini = CASTER.replacen("Object Caster\n KindOf = VEHICLE", "Object Caster\n KindOf = VEHICLE SELECTABLE\n VisionRange = 250", 1);
        let mut w = gate_map_world_with(&ini);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let caster = make(&mut w, b"Caster", team, Coord3D { x: 60.0, y: 60.0, z: 0.0 });
        w.pathfinder_op(crate::logic::world::pathfinder_host_world::PfOp::NewMap);
        w.cuts.game_loop.game_mode = game_mode::SKIRMISH;
        let p = w.get_controlling_player(caster).expect("its player");
        let mut c = ClientOfWorld { w, p, sent: Vec::new(), hints: Vec::new() };
        let (mut x, mut ui, mut cmd) = (SelectionTranslator::default(), InGameUiSelection::new(), CommandTranslator::default());
        let frame = |c: &mut ClientOfWorld, cmd: &mut CommandTranslator| {
            for t in c.sent.drain(..) {
                cmd.observe_message(t);
            }
            c.w.engine_update().expect("a frame");
        };
        let button = |c: &ClientOfWorld, name: &[u8]| {
            let i = c.w.cuts.command_buttons.iter().position(|b| b.name == name).expect("the button");
            crate::client::command_world::gui_button(&c.w, i).expect("its facts")
        };
        let pups = |c: &ClientOfWorld| c.w.objects.ids_in_order().into_iter().filter_map(|o| c.w.find_object(o).filter(|x| x.template().get_name() == b"Pup").map(|x| x.get_position())).collect::<Vec<_>>();

        assert_eq!(x.left_click(&[caster], true, &cmd, &mut ui, &mut c), Disposition::Destroy);
        frame(&mut c, &mut cmd);
        c.sent.clear();
        let drop = button(&c, b"Command_Drop");
        assert_eq!(process_command_ui(&drop, PressedControl::Command, Some(caster), false, false, &mut ui, &mut c), CbCommandStatus::Used);
        assert_eq!(ui.gui_command(), Some(&drop));
        assert!(c.sent.is_empty());
        let at = Coord3D { x: 200.0, y: 200.0, z: 0.0 };
        assert_eq!(cmd.mouse_over(None, &at, &mut ui, &mut c), MSG_VALID_GUICOMMAND_HINT, "hints {:?}", c.hints);
        assert!(cmd.click(false, true, &at, None, &mut ui, &mut c));
        assert_eq!(c.sent, vec![MSG_DO_SPECIAL_POWER_AT_LOCATION]);
        assert_eq!(ui.gui_command(), None);
        assert!(pups(&c).is_empty());
        frame(&mut c, &mut cmd);
        let made = pups(&c);
        eprintln!("C2c: the pups after the drop: {made:?}");
        assert_eq!(made.len(), 1);
        assert!((made[0].x - 200.0).abs() < 1.0 && (made[0].y - 200.0).abs() < 1.0, "{made:?}");

        let halt = button(&c, b"Command_Halt");
        assert_eq!(process_command_ui(&halt, PressedControl::Command, Some(caster), false, false, &mut ui, &mut c), CbCommandStatus::Used);
        assert_eq!(c.sent, vec![MSG_DO_STOP]);
        assert!(c.w.cuts.team_live_errors.is_empty(), "{:?}", c.w.cuts.team_live_errors);
    }

    /// C2c part 2 (#6): a building is placed through the logic. In the gate World (every switch on, a game in progress),
    /// player 1's WorkDozer at (40, 40) (`a_computer_player_builds_its_base`'s, made `SELECTABLE` and given a vision of 250 so
    /// the site is not shrouded), with a construct-Depot button in its set. Predicted from ControlBarCommandProcessing.cpp,
    /// PlaceEventTranslator.cpp and the logic:
    /// - the dozer selected, its button starts placing the Depot (nothing sent);
    /// - the left button down at (150, 150) anchors it; the click sends `MSG_DOZER_CONSTRUCT(Depot, (150, 150), angle)` and
    ///   ends the placement;
    /// - the logic starts a Depot of player 1's at (150, 150), under construction, and the dozer finishes it within 600
    ///   frames (the depot's build time is 2 s, and the dozer must drive about 155 to it at 30/s).
    ///
    /// Results (2026-11-05): all held after one fix **to the logic**: the first run started no placement, because
    /// `BuildAssistant::canMakeUnit` answered `CANMAKE_NO_MONEY` for the free Depot with player 1's money at 0. The port compared
    /// `cost >= money`; upstream's is `cost > money` (BuildAssistant.cpp:1343), so a player with exactly the cost could not
    /// build either. Corrected to `>`. With it: the Depot started at (150.0, 150.0), player 1's, under construction, and the
    /// dozer finished it 203 frames later, standing at (115.6, 134.5); no live errors.
    #[test]
    fn a_building_is_placed_through_the_logic() {
        use crate::client::command_xlat::CommandTranslator;
        use crate::client::control_bar::{gui, process_command_ui, CbCommandStatus, PressedControl};
        use crate::client::in_game_ui::InGameUiSelection;
        use crate::client::place_event::PlaceEventTranslator;
        use crate::client::selection_xlat::{Disposition, SelectionTranslator};
        use crate::common::base_type::{Coord3D, ICoord2D};
        use crate::common::message_types::MSG_DOZER_CONSTRUCT;
        use crate::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION;
        use crate::logic::world::gate_scenario::{gate_map_world_with, make};
        use crate::logic::world::team_ops::factory_find_team;
        let extra = "\
Object WorkDozer\n KindOf = VEHICLE DOZER SELECTABLE\n VisionRange = 250\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = DozerAIUpdate ModuleTag_AI\n  RepairHealthPercentPerSecond = 2%\n  BoredTime = 5000\n  BoredRange = 150\n End\n\
 Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 50\n End\n Locomotor = SET_NORMAL GateTreads\nEnd\n";
        let mut w = gate_map_world_with(extra);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let dozer = make(&mut w, b"WorkDozer", team, Coord3D { x: 40.0, y: 40.0, z: 0.0 });
        let depot = w.statics.thing_factory.find_template(b"Depot").map(|t| t.data.get_template_id());
        w.cuts.command_buttons.push(crate::logic::world::object_cuts::CommandButtonFacts {
            name: b"GateDozerBuildDepot".to_vec(),
            command_type: gui::DOZER_CONSTRUCT,
            weapon_slot: 0,
            special_power: None,
            thing_template: depot,
            upgrade: None,
            ready_for: Default::default(),
            options: 0,
            max_shots_to_fire: 0,
        });
        let button_index = w.cuts.command_buttons.len() - 1;
        w.cuts.command_sets.push((b"GateDozerSet".to_vec(), vec![Some(button_index)]));
        w.objects.find_mut(dozer).expect("the dozer").command_set_string_override = b"GateDozerSet".to_vec();
        w.pathfinder_op(crate::logic::world::pathfinder_host_world::PfOp::NewMap);
        w.cuts.game_loop.game_mode = game_mode::SKIRMISH;
        let p = w.get_controlling_player(dozer).expect("its player");
        let mut c = ClientOfWorld { w, p, sent: Vec::new(), hints: Vec::new() };
        let (mut x, mut ui, cmd, mut place) = (SelectionTranslator::default(), InGameUiSelection::new(), CommandTranslator::default(), PlaceEventTranslator::default());
        assert_eq!(x.left_click(&[dozer], true, &cmd, &mut ui, &mut c), Disposition::Destroy);
        c.w.engine_update().expect("a frame");
        c.sent.clear();

        let button = crate::client::command_world::gui_button(&c.w, button_index).expect("the button");
        assert_eq!(process_command_ui(&button, PressedControl::Command, Some(dozer), false, false, &mut ui, &mut c), CbCommandStatus::Used);
        assert_eq!(ui.pending_place_type(), depot);
        assert!(c.sent.is_empty());
        assert!(place.left_button_down(ICoord2D { x: 150, y: 150 }, &mut ui, &mut c));
        assert!(place.left_click(c.w.frame(), &mut ui, &mut c), "messages {:?}", c.hints);
        assert_eq!(c.sent, vec![MSG_DOZER_CONSTRUCT]);
        assert_eq!(ui.pending_place_type(), None);
        let depots = |c: &ClientOfWorld| c.w.objects.ids_in_order().into_iter().filter(|&o| c.w.find_object(o).is_some_and(|x| x.template().get_name() == b"Depot")).collect::<Vec<_>>();
        c.w.engine_update().expect("a frame");
        let made = depots(&c);
        assert_eq!(made.len(), 1, "one depot started");
        let site = made[0];
        let at = c.w.find_object(site).expect("the site").get_position();
        eprintln!("C2c2: the depot started at {at:?}, owned by {:?}", c.w.get_controlling_player(site));
        assert!((at.x - 150.0).abs() < 1.0 && (at.y - 150.0).abs() < 1.0);
        assert_eq!(c.w.get_controlling_player(site), Some(p));
        let under = |c: &ClientOfWorld| c.w.find_object(site).is_some_and(|o| o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION));
        assert!(under(&c));
        let mut done = None;
        for f in 0..600 {
            c.w.engine_update().expect("a frame");
            if !under(&c) {
                done = Some(f);
                break;
            }
        }
        eprintln!("C2c2: finished after {done:?} frames, the dozer at {:?}", c.w.find_object(dozer).map(|o| o.get_position()));
        assert!(done.is_some(), "the dozer finishes the depot");
        assert!(c.w.cuts.team_live_errors.is_empty(), "{:?}", c.w.cuts.team_live_errors);
    }
}
