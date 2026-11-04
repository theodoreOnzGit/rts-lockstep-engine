// SPDX-License-Identifier: GPL-3.0-only
//
// Integration batch I57 (GitHub #3, #15), after (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/GameEngine.cpp (GameEngine::init: the file system, the game text, the
//   logic's subsystems with their INI, TheMapCache)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GameText.cpp (GameTextManager::init: g_csfFile)
//   GeneralsMD/Code/Main/WinMain.cpp (g_csfFile = "data\\%s\\Generals.csf")
//   GeneralsMD/Code/GameEngine/Source/Common/GlobalData.cpp (the user data directory's leaf name)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! A [`World`] made from a game install, as `GameEngine::init` makes the engine: the file system over the install's
//! folder and archives (batch F1), the game text from `Data\<language>\Generals.csf`, the logic's INI in
//! `initSubsystem`'s order (batch I12), and `TheMapCache` (batch I56).
//!
//! **The World's disk is its `TheFileSystem`.** The World reads its files (maps, `map.ini`, replays, saves) from
//! `save_side.disk`. An install's files reach it there: `Maps\MapCache.ini` at start, and a map's folder when the map
//! is chosen ([`World::mount_map`]), as upstream's file system would find them in `MapsZH.big`. Only what is mounted is
//! held in memory, not the install's archives.
//!
//! **Not done here:** the client's subsystems (art, audio, the GUI's INI), and every INI the logic does not read.

use std::sync::Arc;

use crate::common::file_system::{FileSystem, LocalDevice};
use crate::common::ini::IniError;
use crate::common::map_cache::MapCacheError;
use crate::formats::big::FilenameList;
use crate::formats::csf::{CsfError, GameText};
use crate::logic::world::game_data::{load_game_data, MissingFiles, TextTable};
use crate::logic::world::game_logic::World;

/// `GetRegistryLanguage()`'s answer for an English install (the folder name under `Data\`).
pub const DEFAULT_LANGUAGE: &[u8] = b"English";

/// The user data directory under the user's documents (`GlobalData`'s default `UserDataLeafName`, with its `\`). The
/// World's disk stands for the documents folder.
pub const USER_DATA_DIR: &[u8] = b"Command and Conquer Generals Zero Hour Data\\";

/// What stopped an install's World from being made.
#[derive(Clone, Debug, PartialEq)]
pub enum InstallError {
    /// The logic's INI load threw (a missing file, an unknown block, a bad value), with the files the load opens that
    /// the install does not have (upstream throws at the first; all of them are listed for the report).
    Ini { error: IniError, missing: Vec<Vec<u8>> },
    /// `updateCache` threw (`Maps\MapCache.ini` missing or bad, or a user map that does not parse).
    MapCache(MapCacheError),
    /// [`World::start_skirmish`]: the map is not in the map cache.
    MapNotInCache(Vec<u8>),
}

/// What the install gave, for the loader's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InstallReport {
    /// The archives that did not open (`FileSystem::archive_errors`).
    pub archive_errors: Vec<String>,
    /// The string file's labels, or why it did not load (`None`: not found).
    pub csf: Option<Result<usize, CsfError>>,
    /// The thing templates read.
    pub templates: usize,
    /// The maps in the map cache.
    pub maps: usize,
}

/// `"%s\\%s"`-style join of the parts.
fn join(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

impl World {
    /// `GameEngine::init` over an install's file system: the game text, the logic's INI (every file must open, as
    /// `INI::load` throws on one that does not), a World with those statics, the user data directory, the shipped map
    /// cache file mounted, and `TheMapCache->updateCache()`.
    pub fn from_install<D: LocalDevice>(fs: &FileSystem<D>, language: &[u8]) -> Result<(World, InstallReport), InstallError> {
        let mut report = InstallReport { archive_errors: fs.archive_errors.iter().map(|e| format!("{e:?}")).collect(), ..Default::default() };
        // TheGameText->init(): `g_csfFile` with the registry's language
        let csf_path = join(&[b"data\\", language, b"\\Generals.csf"]);
        let text = match fs.open_file(&csf_path) {
            Some(bytes) => match GameText::load_csf(&bytes) {
                Ok(t) => {
                    report.csf = Some(Ok(t.text_count()));
                    TextTable::from_csf(t)
                }
                Err(e) => {
                    report.csf = Some(Err(e));
                    TextTable::default()
                }
            },
            None => TextTable::default(),
        };
        // the logic's subsystems, each `init()` then its INI, then `postProcessLoadAll()`
        let load = load_game_data(fs, MissingFiles::Error, text).map_err(|error| {
            let missing = crate::logic::world::game_data::LOGIC_SUBSYSTEMS
                .iter()
                .flat_map(|sys| [sys.path1, sys.path2])
                .flatten()
                .filter(|p| !fs.does_file_exist(p))
                .map(<[u8]>::to_vec)
                .collect();
            InstallError::Ini { error, missing }
        })?;
        report.templates = load.factory.templates().count();
        let (mut statics, names) = load.into_statics();
        if statics.global_data.the_global_data().is_none() {
            statics.global_data.create_original();
        }
        if let Some(g) = statics.global_data.the_writable_global_data() {
            g.set_user_data_dir(USER_DATA_DIR);
        }
        let mut w = World::new();
        w.names = names;
        w.statics = Arc::new(statics);
        w.install_command_buttons();
        // a game: every integration switch on (the tables are the module tests' ablations)
        w.cuts.set_all_live();
        // TheMapCache = NEW MapCache; TheMapCache->updateCache(): the shipped cache file from the archives
        let cache = join(&[b"Maps\\", crate::common::map_cache::MAP_CACHE_NAME]);
        if let Some(bytes) = fs.open_file(&cache) {
            w.save_side.disk.insert(cache.to_ascii_lowercase(), bytes);
        }
        w.update_map_cache().map_err(InstallError::MapCache)?;
        report.maps = w.statics.map_cache.maps.len();
        Ok((w, report))
    }

    /// The skirmish menu's launch (SkirmishGameOptionsMenu.cpp): the options entered (`enterGame`), slot 0 the local
    /// human with player template `template` at the map's first start, `startGame(0)`; then the recorder, the map's
    /// name, `InitGameLogicRandom(seed)` and `MSG_NEW_GAME` (`GAME_SKIRMISH`). The game starts on the next frame. The map
    /// must be in the map cache (the menu lists only those).
    pub fn start_skirmish(&mut self, map: &[u8], template: i32, seed: i32) -> Result<(), InstallError> {
        use crate::common::game_info::{slot_state, GameInfo};
        use crate::common::recorder::game_mode;
        use crate::logic::world::game_logic_new_game::GameSource;
        if self.statics.map_cache.find_map(map).is_none() {
            return Err(InstallError::MapNotInCache(map.to_vec()));
        }
        self.cuts.game_loop.game_mode = game_mode::SHELL;
        let mut g = GameInfo::new(self);
        g.enter_game(self);
        let name: Vec<u16> = "Me".encode_utf16().collect();
        g.slots[0].set_state(slot_state::PLAYER, &name, 0, self);
        g.slots[0].set_player_template(template);
        g.slots[0].color = 0;
        g.slots[0].start_pos = 0;
        g.map_name = map.to_vec();
        g.seed = seed;
        g.start_game(0, self);
        self.cuts.new_game.games.push((GameSource::Skirmish, g));
        self.recorder_init();
        self.cuts.new_game.map_name = map.to_vec();
        self.random.init_game_logic_random(seed as u32);
        self.push_command(crate::common::message_types::MSG_NEW_GAME, 0, |m| {
            m.append_integer_argument(game_mode::SKIRMISH);
            m.append_integer_argument(1);
            m.append_integer_argument(0);
        });
        Ok(())
    }

    /// The files of a map's folder (`Maps\<name>\`: the map, its `map.ini`, `map.str`, `solo.ini`) put on the World's
    /// disk, as upstream's file system finds them when the map is loaded. The number of files mounted.
    pub fn mount_map<D: LocalDevice>(&mut self, fs: &FileSystem<D>, map_path: &[u8]) -> usize {
        let Some(slash) = map_path.iter().rposition(|&c| c == b'\\') else { return 0 };
        let dir = &map_path[..=slash];
        let mut list = FilenameList::new();
        fs.get_file_list_in_directory(dir, b"*", &mut list, false);
        let mut n = 0;
        for name in list.iter() {
            if let Some(bytes) = fs.open_file(name) {
                self.save_side.disk.insert(name.to_ascii_lowercase(), bytes);
                n += 1;
            }
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::file_system::MemoryDevice;
    use crate::formats::big::build_big;
    use crate::logic::world::game_data::LOGIC_SUBSYSTEMS;

    const MAP: &[u8] = b"Maps\\Duel\\Duel.map";

    /// A string file of `(label, text)` pairs (the format of `formats::csf`'s module docs).
    fn csf(labels: &[(&str, &str)]) -> Vec<u8> {
        let tag = |s: &[u8; 4]| i32::from_be_bytes(*s);
        let mut v = Vec::new();
        let w = |v: &mut Vec<u8>, x: i32| v.extend_from_slice(&x.to_le_bytes());
        for x in [tag(b"CSF "), 3, labels.len() as i32, labels.len() as i32, 0, 0] {
            w(&mut v, x);
        }
        for (l, t) in labels {
            w(&mut v, tag(b"LBL "));
            w(&mut v, 1);
            w(&mut v, l.len() as i32);
            v.extend_from_slice(l.as_bytes());
            w(&mut v, tag(b"STR "));
            let units: Vec<u16> = t.encode_utf16().collect();
            w(&mut v, units.len() as i32);
            for u in units {
                v.extend_from_slice(&(!u).to_le_bytes());
            }
        }
        v
    }

    /// A map with a flat 30 x 30 height map, the neutral side, and two start waypoints.
    fn duel_map(w: &mut World) -> Vec<u8> {
        use crate::common::dict::Dict;
        use crate::formats::data_chunk_output::DataChunkOutput;
        use crate::logic::map_object::K_OBJECTS_VERSION_3;
        let mut out = DataChunkOutput::new();
        out.open_data_chunk(b"HeightMapData", crate::logic::terrain_visual::K_HEIGHT_MAP_VERSION_4);
        for v in [30, 30, 0, 1, 30, 30, 900] {
            out.write_int(v);
        }
        out.write_array_of_bytes(&[4u8; 900]);
        out.close_data_chunk();
        out.open_data_chunk(b"ObjectsList", K_OBJECTS_VERSION_3);
        for (i, (name, x, y)) in [(&b"Player_1_Start"[..], 100.0, 100.0), (b"Player_2_Start", 200.0, 200.0)].into_iter().enumerate() {
            out.open_data_chunk(b"Object", K_OBJECTS_VERSION_3);
            for r in [x, y, 0.0, 0.0] {
                out.write_real(r);
            }
            out.write_int(0);
            out.write_ascii_string(b"*Waypoints/Waypoint");
            let mut props = Dict::new();
            props.set_int(w.names.name_to_key(b"waypointID"), i as i32 + 1).unwrap();
            props.set_ascii_string(w.names.name_to_key(b"waypointName"), name).unwrap();
            out.write_dict(&props, &w.names);
            out.close_data_chunk();
        }
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

    /// An install in memory, laid out as Zero Hour's: `INIZH.big` (every file the logic's load opens: the tests'
    /// templates in `Object\`, `Multiplayer.ini` with one colour, the others empty), `EnglishZH.big` (the string file),
    /// and `MapsZH.big` (the shipped map cache, listing the map with its name tag, and the map's folder).
    fn install(map: &[u8]) -> FileSystem<MemoryDevice> {
        let mut d = MemoryDevice::new();
        for (name, bytes) in install_archives(map) {
            d.insert(&[&b"ZH\\"[..], &name].concat(), bytes);
        }
        let mut fs = FileSystem::new(d);
        fs.local.game_dir = b"ZH\\".to_vec();
        fs.init(None);
        fs
    }

    /// The three archives of [`install`], by file name.
    fn install_archives(map: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
        let mut ini: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for sys in LOGIC_SUBSYSTEMS {
            for p in [sys.path1, sys.path2].into_iter().flatten() {
                ini.push((p.to_vec(), Vec::new()));
            }
        }
        let set = |ini: &mut Vec<(Vec<u8>, Vec<u8>)>, path: &[u8], text: &[u8]| {
            ini.retain(|(p, _)| p != path);
            ini.push((path.to_vec(), text.to_vec()));
        };
        set(&mut ini, b"Data\\INI\\Object\\Units.ini", crate::logic::world::player_host_world::tests::INI.as_bytes());
        set(
            &mut ini,
            b"Data\\INI\\Multiplayer.ini",
            b"MultiplayerColor Gold\n TooltipName = Color:Gold\n RGBColor = R:255 G:200 B:0\n RGBNightColor = R:128 G:100 B:0\nEnd\n",
        );
        let ini_refs: Vec<(&[u8], &[u8])> = ini.iter().map(|(p, t)| (p.as_slice(), t.as_slice())).collect();
        let text = csf(&[("MAP:Duel", "Desert Duel")]);
        let cache = b"MapCache maps_5Cduel_5Cduel_2Emap\n fileSize = 1\n fileCRC = 7\n isOfficial = yes\n isMultiplayer = yes\n\
            numPlayers = 2\n extentMin = X:0.00 Y:0.00 Z:0.00\n extentMax = X:300.00 Y:300.00 Z:0.00\n nameLookupTag = MAP_3ADuel\n\
            Player_1_Start = X:100.00 Y:100.00 Z:0.00\n Player_2_Start = X:200.00 Y:200.00 Z:0.00\nEND\n";
        vec![
            (b"INIZH.big".to_vec(), build_big(&ini_refs)),
            (b"EnglishZH.big".to_vec(), build_big(&[(b"Data\\English\\Generals.csf", &text)])),
            (b"MapsZH.big".to_vec(), build_big(&[(b"Maps\\MapCache.ini", cache), (MAP, map), (b"Maps\\Duel\\map.ini", b"")])),
        ]
    }

    /// Batch I57: the same install written to a folder on disk, with the archives' names in other cases (a Steam folder
    /// on Linux keeps Windows' mixed case), read through [`crate::common::file_system::NativeDevice`] as the loader
    /// (`examples/load_install.rs`) reads one. Predicted: the same report, and the map mounts and starts.
    #[test]
    fn an_install_on_disk_loads_through_the_native_device() {
        use crate::common::file_system::NativeDevice;
        let mut scratch = World::new();
        let map = duel_map(&mut scratch);
        let dir = std::env::temp_dir().join(format!("rts_install_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the folder");
        for (name, bytes) in install_archives(&map) {
            std::fs::write(dir.join(String::from_utf8_lossy(&name).to_uppercase()), bytes).expect("an archive");
        }
        let mut fs = FileSystem::new(NativeDevice::new(&dir));
        fs.init(None);
        let (mut w, report) = World::from_install(&fs, DEFAULT_LANGUAGE).expect("the install");
        assert!(report.archive_errors.is_empty(), "{:?}", report.archive_errors);
        assert_eq!((report.csf, report.templates, report.maps), (Some(Ok(1)), 11, 1));
        assert_eq!(w.mount_map(&fs, MAP), 2);
        w.start_skirmish(MAP, 0, 5).expect("the skirmish");
        for _ in 0..10 {
            w.engine_update().expect("a frame");
        }
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        assert!(w.objects.ids_in_order().len() >= 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Integration batch I57: a World made from an install, as `GameEngine::init` makes the engine, starts a skirmish on a
    /// map of the install's archives. Predicted before the run:
    /// - the install loads with no error: the string file's one label, the tests' ~~twelve~~ eleven templates (a miscount in the prediction: the tests' INI holds eleven objects), one map in the cache
    ///   (from the shipped `MapCache.ini`, not parsed: its CRC is the file's 7, and its display name is the string file's
    ///   "Desert Duel (2)");
    /// - the user data directory is `Command and Conquer Generals Zero Hour Data\`, so the user's map directory is not the
    ///   shipped one and no cache file is written;
    /// - mounting the map puts its folder's two files on the disk;
    /// - a skirmish of one human `FactionAmerica` slot at start 0 starts on the next frame: the player's starting
    ///   Barracks and Tank are made at `Player_1_Start`, and 100 frames run with no live error;
    /// - with only `GameData.ini` in the archive, the load stops with `CantOpenFile`, and the report lists the other files
    ///   (`Multiplayer.ini` among them) and not `GameData.ini`.
    #[test]
    fn a_world_from_an_install_starts_a_skirmish_on_its_map() {
        // the map is written with a World's name keys (the dictionary's keys are names in the file)
        let mut scratch = World::new();
        let map = duel_map(&mut scratch);
        let fs = install(&map);
        assert!(fs.archive_errors.is_empty());
        let (mut w, report) = World::from_install(&fs, DEFAULT_LANGUAGE).expect("the install");
        assert_eq!(report.csf, Some(Ok(1)));
        assert_eq!(report.templates, 11);
        assert_eq!(report.maps, 1);
        let md = w.statics.map_cache.find_map(MAP).expect("the shipped entry").clone();
        assert_eq!((md.num_players, md.crc), (2, 7));
        assert_eq!(String::from_utf16_lossy(md.display_name.str()), "Desert Duel (2)");
        assert!(!w.save_side.disk.keys().any(|k| k.starts_with(b"command and conquer") && k.ends_with(b".ini")), "no cache file written");
        assert_eq!(w.mount_map(&fs, MAP), 2);

        assert!(w.cuts.all_live());
        w.start_skirmish(MAP, 0, 99).expect("the skirmish");
        let nowhere = b"Maps\\Nowhere\\Nowhere.map";
        assert_eq!(w.start_skirmish(nowhere, 0, 99), Err(InstallError::MapNotInCache(nowhere.to_vec())));
        for _ in 0..100 {
            w.engine_update().expect("a frame");
        }
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        let made: Vec<(Vec<u8>, crate::common::base_type::Coord3D)> = w
            .objects
            .ids_in_order()
            .into_iter()
            .filter_map(|o| w.find_object(o).map(|x| (x.template().get_name().to_vec(), x.get_position())))
            .collect();
        let names: Vec<&[u8]> = made.iter().map(|(n, _)| n.as_slice()).collect();
        assert!(names.contains(&&b"Barracks"[..]) && names.contains(&&b"Tank"[..]), "{names:?}");
        let barracks = made.iter().find(|(n, _)| n == b"Barracks").map(|(_, p)| *p).expect("the barracks");
        assert!((barracks.x - 100.0).abs() < 1.0 && (barracks.y - 100.0).abs() < 1.0, "{barracks:?}");

        // the same install without `Multiplayer.ini`: `INI::load` throws on the file it cannot open
        let mut d = MemoryDevice::new();
        let empty: [(&[u8], &[u8]); 1] = [(b"Data\\INI\\GameData.ini", b"")];
        d.insert(b"ZH\\INIZH.big", build_big(&empty));
        let mut bare = FileSystem::new(d);
        bare.local.game_dir = b"ZH\\".to_vec();
        bare.init(None);
        let r = World::from_install(&bare, DEFAULT_LANGUAGE);
        let Err(InstallError::Ini { error, missing }) = r else { panic!("{r:?}") };
        assert_eq!(error.kind, crate::common::ini::IniErrorKind::CantOpenFile);
        assert!(missing.iter().any(|m| m == b"Data\\INI\\Multiplayer.ini") && !missing.iter().any(|m| m == b"Data\\INI\\GameData.ini"), "{missing:?}");
    }
}
