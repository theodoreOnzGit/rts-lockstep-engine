// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameLogic/W3DTerrainLogic.cpp (`W3DTerrainLogic::loadMap`)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/WorldHeightMap.cpp (the constructor's chunk parsers)
//   GeneralsMD/Code/GameEngine/Source/GameClient/MapUtil.cpp (`ParseObjectsDataChunk`)
//   GeneralsMD/Code/GameEngine/Include/Common/MapObject.h (the `FLAG_*` values)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Integration batch I32: a map file read into the world, as `TheTerrainLogic->loadMap(filename)` reads it (the device's
//! `W3DTerrainLogic`, whose `WorldHeightMap(stream, logicalDataOnly)` registers the chunk parsers): the height map, the
//! boundaries and the waypoints (`TerrainLogic::load_map_w3d`), the map objects (`ObjectsList`, `ParseObjectsDataChunk` with
//! `ParseObjectDataChunk`), the polygon triggers (`PolygonTriggers`) and the sides with their teams and scripts (`SidesList`).
//!
//! The ported parsers each read their own chunks through their own context, so the file is read once for each, the others'
//! chunks skipped (upstream reads it once with all of them registered; the chunks are independent, so the result is the same).
//! The file is the world's disk's (`save_side.disk`, by its lower-cased name, the store a save also uses). A file that is not a
//! chunk file, or has no `HeightMapData` chunk, leaves the world as it was and answers false (upstream's constructor throws
//! `ERROR_CORRUPT_FILE_FORMAT`, which `loadMap` turns into `FALSE`); nothing is changed unless the height map was read.

use crate::common::ascii_string::AsciiString;
use crate::common::dict::Dict;
use crate::formats::data_chunk::{CachedFileInputStream, ChunkResult, DataChunkInfo, DataChunkInput};
use crate::logic::map_object::{parse_object_data_chunk, MapObject, ObjectParse};
use crate::logic::polygon_trigger::{parse_polygon_triggers_data_chunk, TriggerParse};
use crate::logic::terrain_logic::{TriggerArea, WaypointKeys, WaypointMapObject};
use crate::logic::terrain_visual::LogicHeightMap;
use crate::logic::world::game_logic::World;
use crate::logic::world::game_logic_new_game::MapObj;

/// `FLAG_DRAWS_IN_MIRROR` (MapObject.h).
pub const FLAG_DRAWS_IN_MIRROR: i32 = 0x0000_0001;
/// `FLAG_ROAD_FLAGS`: `FLAG_ROAD_POINT1 | FLAG_ROAD_POINT2`.
pub const FLAG_ROAD_FLAGS: i32 = 0x0000_0002 | 0x0000_0004;
/// `FLAG_BRIDGE_FLAGS`: `FLAG_BRIDGE_POINT1 | FLAG_BRIDGE_POINT2`.
pub const FLAG_BRIDGE_FLAGS: i32 = 0x0000_0010 | 0x0000_0020;

/// `ParseObjectsDataChunk(file, info, userData)`: the `Object` chunks inside, in file order.
fn parse_objects_data_chunk(file: &mut DataChunkInput<ObjectParse>, info: &DataChunkInfo, ctx: &mut ObjectParse) -> ChunkResult<bool> {
    file.register_parser(b"Object", &info.label, parse_object_data_chunk);
    file.parse(ctx)
}

/// What one reading of the map file gave, before any of it goes into the world.
struct MapRead {
    height_map: LogicHeightMap,
    objects: Vec<MapObject>,
    triggers: Vec<TriggerArea>,
    sides: Option<crate::logic::sides_list::SidesList>,
}

impl World {
    /// `TheTerrainLogic->loadMap(filename, FALSE)` with the map's logical chunks: true when the map was read into the world. An
    /// error inside a chunk that upstream leaves undefined is recorded (`cuts.team_live_errors`) and the world is left as it was.
    pub fn load_map_file(&mut self, filename: &[u8]) -> bool {
        let Some(bytes) = self.save_side.disk.get(&filename.to_ascii_lowercase()).cloned() else { return false };
        let read = match self.read_map(&bytes) {
            Ok(Some(r)) => r,
            Ok(None) => return false,
            Err(e) => {
                self.cuts.team_live_errors.push(format!("TerrainLogic::loadMap: {e}"));
                return false;
            }
        };
        // the waypoints of the objects, for the terrain's `addWaypoint` loop
        let keys = WaypointKeys {
            id: self.names.name_to_key(b"waypointID"),
            name: self.names.name_to_key(b"waypointName"),
            label1: self.names.name_to_key(b"waypointPathLabel1"),
            label2: self.names.name_to_key(b"waypointPathLabel2"),
            label3: self.names.name_to_key(b"waypointPathLabel3"),
            bi_directional: self.names.name_to_key(b"waypointPathBiDirectional"),
        };
        let waypoints: Vec<WaypointMapObject> = read
            .objects
            .iter()
            .map(|o| WaypointMapObject { is_waypoint: o.is_waypoint(), location: o.location(), properties: o.properties().clone() })
            .collect();
        self.terrain.set_logic_height_map(Some(read.height_map));
        let name = AsciiString::try_from_bytes(filename).unwrap_or_default();
        if let Err(e) = self.terrain.load_map_w3d(&name, false, Some(&bytes), &waypoints, &keys) {
            self.cuts.team_live_errors.push(format!("TerrainLogic::loadMap: {e:?}"));
        }
        self.terrain.set_triggers(read.triggers);
        // the map objects, as `startNewGame` walks `MapObject::getFirstMapObject()`, each with its template found
        let owner_key = self.names.name_to_key(b"originalOwner");
        let objects: Vec<(MapObj, Dict)> = read
            .objects
            .iter()
            .map(|o| {
                let template = o.thing_template().and_then(|n| World::thing_factory_find_template(self, n)).map(u32::from);
                let mo = MapObj {
                    name: o.thing_template().unwrap_or_default().to_vec(),
                    template,
                    bridge_or_road: o.flags() & (FLAG_BRIDGE_FLAGS | FLAG_ROAD_FLAGS) != 0,
                    draws_in_mirror: o.flags() & FLAG_DRAWS_IN_MIRROR != 0,
                    location: o.location(),
                    angle: o.angle(),
                    original_owner: o.properties().get_ascii_string(owner_key).0,
                };
                (mo, o.properties().clone())
            })
            .collect();
        self.cuts.new_game.map_objects = objects;
        if let Some(sides) = read.sides {
            self.sides = sides;
        }
        true
    }

    /// The map file's chunks, each parse with the world's name keys lent to it. `Ok(None)`: not a map the logic can read.
    fn read_map(&mut self, bytes: &[u8]) -> Result<Option<MapRead>, String> {
        // the height map first: without it, nothing is read (`WorldHeightMap` throws)
        let Some(stream) = CachedFileInputStream::new(bytes.to_vec()) else { return Ok(None) };
        let height_map = match LogicHeightMap::from_map_file(stream, false) {
            Ok(Some(m)) => m,
            Ok(None) | Err(crate::formats::data_chunk::ChunkError::Thrown(_)) => return Ok(None),
            Err(e) => return Err(format!("HeightMapData: {e:?}")),
        };
        // the objects
        let mut op = ObjectParse::new();
        op.names = std::mem::take(&mut self.names);
        let r = Self::parse_with(bytes, &mut op, b"ObjectsList", parse_objects_data_chunk);
        self.names = std::mem::take(&mut op.names);
        r.map_err(|e| format!("ObjectsList: {e:?}"))?;
        if let Some(u) = op.undefined {
            return Err(format!("ObjectsList: {u:?}"));
        }
        // the polygon triggers (the version-1 water area needs `TheGlobalData`'s extents)
        let (wx, wy) = self.statics.global_data.the_global_data().map_or((0.0, 0.0), |g| (g.water_extent_x, g.water_extent_y));
        let mut tp = TriggerParse::new(wx, wy).map_err(|e| format!("PolygonTriggers: {e:?}"))?;
        Self::parse_with(bytes, &mut tp, b"PolygonTriggers", parse_polygon_triggers_data_chunk).map_err(|e| format!("PolygonTriggers: {e:?}"))?;
        if let Some(u) = tp.undefined {
            return Err(format!("PolygonTriggers: {u:?}"));
        }
        let triggers = tp.list.iter().map(TriggerArea::from_polygon_trigger).collect::<Result<Vec<_>, _>>().map_err(|e| format!("PolygonTriggers: {e:?}"))?;
        // the sides, their teams and scripts (a map with no `SidesList` chunk keeps the world's)
        let templates = crate::logic::script_templates::script_templates(&mut self.names);
        let mut sc = crate::logic::sides_list::SidesCtx::new(std::mem::take(&mut self.names), templates);
        let read = sc.read_sides_chunk_file(bytes.to_vec());
        self.names = std::mem::take(&mut sc.names.generator);
        let sides = match read {
            Ok(true) => Some(sc.sides),
            Ok(false) => None,
            Err(e) => return Err(format!("SidesList: {e:?}")),
        };
        Ok(Some(MapRead { height_map, objects: op.objects, triggers, sides }))
    }

    /// One parse of the file with only `label`'s parser registered (the other chunks are skipped).
    fn parse_with<C>(bytes: &[u8], ctx: &mut C, label: &[u8], parser: crate::formats::data_chunk::DataChunkParser<C>) -> ChunkResult<bool> {
        let Some(stream) = CachedFileInputStream::new(bytes.to_vec()) else { return Ok(false) };
        let mut input = DataChunkInput::<C>::new(stream)?;
        input.register_parser(label, b"", parser);
        input.parse(ctx)
    }
}

impl World {
    /// Integration batch I33: `INI ini; ini.load(path, INI_LOAD_CREATE_OVERRIDES, NULL)` of a file of the world's disk into the
    /// world's stores (`loadMapINI`'s `map.ini` and `solo.ini`): the stores taken out of the statics, the file read into them as
    /// overrides, the stores put back. No post-process follows (upstream runs none after these). False when the disk has no such
    /// file.
    pub fn load_ini_overrides(&mut self, path: &[u8]) -> crate::common::ini::IniResult<bool> {
        let Some(text) = self.save_side.disk.get(&path.to_ascii_lowercase()).cloned() else { return Ok(false) };
        let statics = (*self.statics).clone();
        let mut load = crate::logic::world::game_data::GameDataLoad::from_statics(statics, std::mem::take(&mut self.names))?;
        let r = load.load_text_as(path, &text, crate::common::ini::IniLoadType::CreateOverrides);
        let (statics, names) = load.into_statics();
        self.statics = std::sync::Arc::new(statics);
        self.names = names;
        r.map(|()| true)
    }
}

/// Batch I56: `MapUtil.cpp`'s globals as the World holds them: the files are the world's disk (lower-cased paths), the
/// templates the statics' thing factory, the name keys the world's.
/// - The game text is ~~not the World's (a table with no strings)~~ the statics' (batch I57: the install's string
///   file). `initMapStringFile` and `reset` do nothing: no `map.str` is read (a map's own strings are left out; its
///   name tag is then fetched from the string file only).
/// - `TheGlobalData->getPath_UserData()` and `m_buildMapCache` are the global data's. An empty user data path (the tests'
///   worlds) makes the user's map directory `Maps\`, the shipped one; an install's World sets it (batch I57).
impl crate::common::map_cache::MapCacheHost for World {
    fn get_file_list_in_directory(&self, dir: &[u8], pattern: &[u8], list: &mut crate::formats::big::FilenameList, recursive: bool) {
        // `getFileListInDirectory` over the disk: the files under the directory whose name matches the pattern (a
        // `*.ext` pattern: the extension), in subdirectories when `recursive`
        let d = dir.to_ascii_lowercase();
        let ext = pattern.strip_prefix(b"*").unwrap_or(pattern).to_ascii_lowercase();
        for k in self.save_side.disk.keys() {
            if let Some(rest) = k.strip_prefix(d.as_slice()) {
                if k.ends_with(&ext) && (recursive || !rest.contains(&b'\\')) {
                    list.insert(k.clone());
                }
            }
        }
    }
    fn get_file_info(&self, path: &[u8]) -> Option<crate::common::map_cache::MapFileInfo> {
        let f = self.save_side.disk.get(&path.to_ascii_lowercase())?;
        Some(crate::common::map_cache::MapFileInfo { size_low: u32::try_from(f.len()).unwrap_or(u32::MAX), ..Default::default() })
    }
    fn open_file(&self, path: &[u8]) -> Option<Vec<u8>> {
        self.save_side.disk.get(&path.to_ascii_lowercase()).cloned()
    }
    fn user_data_path(&self) -> Vec<u8> {
        crate::common::recorder::RecorderHost::user_data_path(self)
    }
    fn build_map_cache(&self) -> bool {
        self.statics.global_data.the_global_data().is_some_and(|g| g.build_map_cache)
    }
    fn fetch(&self, label: &[u8]) -> crate::common::unicode_string::UnicodeString {
        // (~~a table with no strings~~ batch I57: the statics' game text)
        crate::common::science::GameTextFetch::fetch(&self.statics.game_text, label)
    }
    fn init_map_string_file(&mut self, _path: &[u8]) {}
    fn reset_game_text(&mut self) {}
    fn tech_or_supply(&self, template: &[u8]) -> (bool, bool) {
        use crate::common::kind_of::{KINDOF_SUPPLY_SOURCE_ON_PREVIEW, KINDOF_TECH_BUILDING};
        self.statics
            .thing_factory
            .find_template(template)
            .map_or((false, false), |t| (t.data.is_kind_of(KINDOF_TECH_BUILDING), t.data.is_kind_of(KINDOF_SUPPLY_SOURCE_ON_PREVIEW)))
    }
    fn write_file(&mut self, path: &[u8], text: Vec<u8>) {
        self.save_side.disk.insert(path.to_ascii_lowercase(), text);
    }
    fn names(&mut self) -> &mut crate::common::name_key_generator::NameKeyGenerator {
        &mut self.names
    }
}

impl World {
    /// Batch I56: `TheMapCache = NEW MapCache; TheMapCache->updateCache();` (GameEngine.cpp:606) over the world's disk,
    /// into the statics.
    pub fn update_map_cache(&mut self) -> crate::common::map_cache::MapCacheResult<()> {
        let mut cache = std::mem::take(&mut std::sync::Arc::make_mut(&mut self.statics).map_cache);
        let r = cache.update_cache(self);
        std::sync::Arc::make_mut(&mut self.statics).map_cache = cache;
        r
    }
}
