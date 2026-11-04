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
        Vec::new()
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
    use crate::common::dict::Dict;
    use crate::common::game_info::slot_state;
    use crate::common::message_types::{MSG_CLEAR_GAME_DATA, MSG_NEW_GAME};
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
        w.cuts.new_game.colors = vec![(0x00ff_0000, 0x0080_0000)];
        w.sides = crate::logic::sides_list::SidesList::new();
        w.cuts.new_game.map_objects = Vec::new();
        let file = skirmish_map(&mut w);
        w.save_side.disk.insert(MAP.to_ascii_lowercase(), file);
        // `TheMapCache`'s entry for the map (MapUtil.cpp scans the map files for it; not translated yet, so the World's
        // table holds it): two players, and the first one's start
        w.cuts.map_cache.insert(
            MAP.to_ascii_lowercase(),
            crate::logic::world::game_logic_start::MapStartInfo {
                num_players: 2,
                waypoints: vec![(b"Player_1_Start".to_vec(), crate::common::base_type::Coord3D { x: 100.0, y: 100.0, z: 0.0 })],
            },
        );
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

        let mut play = skirmish_world(false);
        play.save_side.disk.insert(replays[0].clone(), rec.save_side.disk[&replays[0]].clone());
        // (`playbackFile` takes the name within the replay folder, as the replay menu passes it)
        let name = replays[0].strip_prefix(b"replays\\".as_slice()).expect("in the replay folder").to_vec();
        assert!(play.play_replay(&name).expect("the replay read"));
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
}
