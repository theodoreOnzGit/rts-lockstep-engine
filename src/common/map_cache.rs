// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MapUtil.cpp, Include/GameClient/MapUtil.h
//   (calcCRC, ParseObjectDataChunk's classification, ParseWorldDictDataChunk, ParseSizeOnly, loadMap, resetMap,
//   getExtent, WaypointMap::update, MapCache::writeCacheINI, updateCache, clearUnseenMaps, loadStandardMaps,
//   loadUserMaps, addMap, findMap)
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INIMapCache.cpp (MapMetaDataReader, INI::parseMapCacheDefinition)
//   GeneralsMD/Code/GameEngine/Source/Common/System/QuotedPrintable.cpp (AsciiStringToQuotedPrintable,
//   QuotedPrintableToAsciiString, hexDigitToInt, intToHexDigit; #89: UnicodeStringToQuotedPrintable,
//   QuotedPrintableToUnicodeString)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `TheMapCache`: what the game knows of each map without loading it (its player count, start positions, CRC, size).
//! Integration batch I56, GitHub issue #15.
//!
//! Upstream builds it once, in `GameEngine::init` (`TheMapCache->updateCache()`): the user's maps are scanned and each
//! one that changed is parsed for its size, start waypoints and the tech and supply positions; then the shipped maps'
//! metadata is read from `Maps\MapCache.ini`, which `MapsZH.big` carries (the shipped maps are not parsed in a release
//! build). The logic reads it in `startNewGame` (the start positions), in `GameInfo`'s slot checks (the player count)
//! and in a replay or save header (the map's CRC).
//!
//! - [`MapCacheHost`] is what upstream reaches through globals: `TheFileSystem`, `TheGameText`, `TheGlobalData`'s user
//!   data path and `m_buildMapCache`, `TheThingFactory` (a template's kind of), and the `fopen` of `writeCacheINI`.
//! - The statics of `MapUtil.cpp` that `loadMap` fills (`m_width`, `m_height`, `m_borderSize`, `worldDict`,
//!   `m_waypoints`, the tech and supply lists) are [`MapUtilStatics`], kept in the cache: upstream never clears the
//!   sizes or `worldDict` between maps, so a map with no `WorldInfo` chunk takes the previous map's name tag.
//! - **Not translated:** the `_DEBUG`/`_INTERNAL` block of `updateCache` (a debug build rebuilds the shipped cache), ~~the
//!   map list boxes and preview images (GUI), `WouldMapTransfer`,~~ `copyFromBigToDir`. **Corrected 2026-11-10 (#89):**
//!   the map list boxes, `WouldMapTransfer`, `isValidMap`, `getDefaultMap`, `getDefaultOfficialMap`, `isOfficialMap`,
//!   `findDrawPositions` and the preview image's lookup are translated in `crate::client::map_util` (the preview's copy
//!   out of the archive, `copyFromBigToDir`, is not); QuotedPrintable.cpp's Unicode pair is here too
//!   ([`unicode_string_to_quoted_printable`], [`quoted_printable_to_unicode_string`]).
//! - **Deviation:** `FileInfo`'s timestamps are 0 ([`crate::common::file_system::FileSystem::get_file_info`] returns
//!   only the size). Upstream reads them only to write them into `MapCache.ini`; `addMap` compares size and CRC.
//! - **Deviation:** `parseMapCacheDefinition` reads `mdr.m_waypoints[i]` for `i < m_numPlayers` from an eight-entry
//!   array; a `numPlayers` above eight reads past it upstream. Here the loop stops at eight.

use std::collections::{BTreeMap, BTreeSet};

use crate::common::base_type::{Coord3D, Region3D};
use crate::common::crc::Crc;
use crate::common::dict::Dict;
use crate::common::ini::{BlockParse, FieldParse, Ini, IniError, IniLoadType, IniResult, UserData};
#[cfg(test)]
use crate::common::ini::IniErrorKind;
use crate::common::name_key_generator::NameKeyGenerator;
use crate::common::unicode_string::UnicodeString;
use crate::formats::big::FilenameList;
use crate::formats::data_chunk::{CachedFileInputStream, ChunkResult, DataChunkInfo, DataChunkInput};
use crate::logic::map_object::{parse_object_data_chunk, ObjectParse};
use crate::logic::terrain_visual::{HeightMapParse, LogicHeightMap, MAP_XY_FACTOR};

/// `MAX_SLOTS` (GameNetwork/NetworkDefs.h).
const MAX_SLOTS: usize = 8;

/// `MapCache::m_mapCacheName`.
pub const MAP_CACHE_NAME: &[u8] = b"MapCache.ini";

/// `mapExtension` (MapUtil.cpp:72).
const MAP_EXTENSION: &[u8] = b".map";

/// `struct WinTimeStamp`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WinTimeStamp {
    /// `m_lowTimeStamp`.
    pub low: u32,
    /// `m_highTimeStamp`.
    pub high: u32,
}

/// `class WaypointMap : public std::map<AsciiString, Coord3D>`: a map's start waypoints (and its initial camera
/// position), by name, in `AsciiString` order (byte order, as `strcmp`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaypointMap {
    /// The map's entries.
    pub map: BTreeMap<Vec<u8>, Coord3D>,
    /// `m_numStartSpots`.
    pub num_start_spots: i32,
}

/// `class MapMetaData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapMetaData {
    /// `m_displayName`.
    pub display_name: UnicodeString,
    /// `m_nameLookupTag`.
    pub name_lookup_tag: Vec<u8>,
    /// `m_extent`.
    pub extent: Region3D,
    /// `m_numPlayers`.
    pub num_players: i32,
    /// `m_isMultiplayer`.
    pub is_multiplayer: bool,
    /// `m_isOfficial`.
    pub is_official: bool,
    /// `m_filesize`.
    pub filesize: u32,
    /// `m_CRC`.
    pub crc: u32,
    /// `m_timestamp`.
    pub timestamp: WinTimeStamp,
    /// `m_waypoints`.
    pub waypoints: WaypointMap,
    /// `m_supplyPositions`.
    pub supply_positions: Vec<Coord3D>,
    /// `m_techPositions`.
    pub tech_positions: Vec<Coord3D>,
    /// `m_fileName`.
    pub file_name: Vec<u8>,
}

/// What a map file gives `getFileInfo` (`struct FileInfo`): its size (`sizeLow`) and its timestamps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MapFileInfo {
    /// `sizeLow`.
    pub size_low: u32,
    /// `timestampLow`.
    pub timestamp_low: u32,
    /// `timestampHigh`.
    pub timestamp_high: u32,
}

/// The engine as `MapUtil.cpp` and `INIMapCache.cpp` reach it.
pub trait MapCacheHost {
    /// `TheFileSystem->getFileListInDirectory(dir, pattern, list, recursive)`.
    fn get_file_list_in_directory(&self, dir: &[u8], pattern: &[u8], list: &mut FilenameList, recursive: bool);
    /// `TheFileSystem->getFileInfo(path, &info)`.
    fn get_file_info(&self, path: &[u8]) -> Option<MapFileInfo>;
    /// `TheFileSystem->openFile(path, File::READ)`, the whole file.
    fn open_file(&self, path: &[u8]) -> Option<Vec<u8>>;
    /// `TheGlobalData->getPath_UserData()`.
    fn user_data_path(&self) -> Vec<u8>;
    /// `TheGlobalData->m_buildMapCache`.
    fn build_map_cache(&self) -> bool;
    /// `TheGameText->fetch(label)`.
    fn fetch(&self, label: &[u8]) -> UnicodeString;
    /// `TheGameText->initMapStringFile(path)`.
    fn init_map_string_file(&mut self, path: &[u8]);
    /// `TheGameText->reset()`.
    fn reset_game_text(&mut self);
    /// `TheThingFactory->findTemplate(name, FALSE)`'s `isKindOf(KINDOF_TECH_BUILDING)` and
    /// `isKindOf(KINDOF_SUPPLY_SOURCE_ON_PREVIEW)` (both false with no such template).
    fn tech_or_supply(&self, template: &[u8]) -> (bool, bool);
    /// `writeCacheINI`'s `fopen(path, "w")` and `fprintf`s: the whole file.
    fn write_file(&mut self, path: &[u8], text: Vec<u8>);
    /// `TheNameKeyGenerator` (the map's dictionaries are read with it).
    fn names(&mut self) -> &mut NameKeyGenerator;
}

/// The statics of `MapUtil.cpp` that `loadMap` fills and `addMap` reads.
#[derive(Clone, Debug, Default)]
pub struct MapUtilStatics {
    /// `m_width`.
    pub width: i32,
    /// `m_height`.
    pub height: i32,
    /// `m_borderSize`.
    pub border_size: i32,
    /// `worldDict`.
    pub world_dict: Dict,
    /// `m_waypoints` (`None`: `NULL`, before a `loadMap` and after `resetMap`).
    pub waypoints: Option<BTreeMap<Vec<u8>, Coord3D>>,
    /// `m_supplyPositions`.
    pub supply_positions: Vec<Coord3D>,
    /// `m_techPositions`.
    pub tech_positions: Vec<Coord3D>,
    /// `m_mapDX`.
    pub map_dx: i32,
    /// `m_mapDY`.
    pub map_dy: i32,
}

/// `class MapCache : public std::map<AsciiString, MapMetaData>`.
#[derive(Clone, Debug, Default)]
pub struct MapCache {
    /// The cache's entries, by lower-cased file name.
    pub maps: BTreeMap<Vec<u8>, MapMetaData>,
    /// `m_seen`.
    seen: BTreeMap<Vec<u8>, bool>,
    /// `m_allowedMaps`.
    allowed_maps: BTreeSet<Vec<u8>>,
    /// See [`MapUtilStatics`].
    pub statics: MapUtilStatics,
}

/// `str.reverseFind('\\') + 1`: what follows the last backslash (a name with none is undefined upstream: `NULL + 1`;
/// here the whole name).
fn after_last_backslash(name: &[u8]) -> &[u8] {
    name.iter().rposition(|&c| c == b'\\').map_or(name, |i| &name[i + 1..])
}

/// `m_displayName.translate(name)` and, for two players or more, `L" (%d)"` after it.
fn display_name_from(name: &[u8], num_players: i32) -> UnicodeString {
    let mut units: Vec<u16> = name.iter().map(|&b| u16::from(b)).collect();
    if num_players >= 2 {
        units.extend(format!(" ({num_players})").encode_utf16());
    }
    UnicodeString::try_from_chars(&units).unwrap_or_default()
}

/// `TheGameText->fetch(tag)` and, for two players or more, `L" (%d)"` after it.
fn display_name_fetched(host: &impl MapCacheHost, tag: &[u8], num_players: i32) -> UnicodeString {
    let mut units = host.fetch(tag).str().to_vec();
    if num_players >= 2 {
        units.extend(format!(" ({num_players})").encode_utf16());
    }
    UnicodeString::try_from_chars(&units).unwrap_or_default()
}

/// `hexDigitToInt(c)`.
fn hex_digit_to_int(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    }
}

/// `QuotedPrintableToAsciiString(original)`: `_XY` is the byte `0xXY`. A `_` at the end ends the string; a `_X` at the
/// end is the byte `X`. (Upstream's `i<1023` bound never moves, as `i` is never incremented.)
pub fn quoted_printable_to_ascii_string(original: &[u8]) -> Vec<u8> {
    let mut dest = Vec::with_capacity(original.len());
    let mut i = 0;
    while i < original.len() && original[i] != 0 {
        if original[i] == b'_' {
            let Some(&d1) = original.get(i + 1).filter(|&&c| c != 0) else { break };
            let mut c = hex_digit_to_int(d1);
            i += 1;
            if let Some(&d2) = original.get(i + 1).filter(|&&c| c != 0) {
                c = (c << 4) | hex_digit_to_int(d2);
                i += 1;
            }
            dest.push(c);
        } else {
            dest.push(original[i]);
        }
        i += 1;
    }
    // the string ends at the first 0 a `_00` writes
    if let Some(n) = dest.iter().position(|&c| c == 0) {
        dest.truncate(n);
    }
    dest
}

/// `intToHexDigit(num)`: `'\0'` outside 0 to 15.
fn int_to_hex_digit(num: i32) -> u8 {
    match num {
        0..=9 => b'0' + num as u8,
        10..=15 => b'A' + (num - 10) as u8,
        _ => 0,
    }
}

/// `AsciiStringToQuotedPrintable(original)`: each byte that is not a letter or digit is `_` and two hex digits. A byte
/// above 0x7F is a negative `char`, whose `>> 4` is negative: its first digit is `'\0'`, which ends the string there.
pub fn ascii_string_to_quoted_printable(original: &[u8]) -> Vec<u8> {
    let mut dest = Vec::with_capacity(original.len() * 3);
    for &b in original.iter().take_while(|&&b| b != 0) {
        if dest.len() >= 1021 {
            break;
        }
        if b.is_ascii_alphanumeric() {
            dest.push(b);
        } else {
            let signed = i32::from(b as i8);
            dest.push(b'_');
            dest.push(int_to_hex_digit(signed >> 4));
            dest.push(int_to_hex_digit(signed & 0xf));
        }
    }
    if let Some(n) = dest.iter().position(|&c| c == 0) {
        dest.truncate(n);
    }
    dest
}

/// (#89) `UnicodeStringToQuotedPrintable(original)`: the string's UTF-16 units as their two bytes, low byte first (the
/// `char *` walk over `WideChar`s), each byte that is not a letter or digit written `_` and two hex digits, until a unit
/// whose two bytes are both 0. A byte above 0x7F is a negative `char`, whose `>> 4` is negative: its first digit is
/// `'\0'`, which ends the string there (as [`ascii_string_to_quoted_printable`]; upstream's `isalnum` of a negative
/// `char` is undefined in C, MSVC's table read before its start: taken as not a letter or digit, **not re-checked**).
/// At most 1021 bytes are looked at before the check (`i<1021`, tested once per unit).
pub fn unicode_string_to_quoted_printable(original: &[u16]) -> Vec<u8> {
    let mut dest = Vec::with_capacity(original.len() * 6);
    let push = |dest: &mut Vec<u8>, b: u8| {
        if b.is_ascii_alphanumeric() {
            dest.push(b);
        } else {
            let signed = i32::from(b as i8);
            dest.push(b'_');
            dest.push(int_to_hex_digit(signed >> 4));
            dest.push(int_to_hex_digit(signed & 0xf));
        }
    };
    for &u in original.iter().take_while(|&&u| u != 0) {
        if dest.len() >= 1021 {
            break;
        }
        push(&mut dest, (u & 0xff) as u8);
        push(&mut dest, (u >> 8) as u8);
    }
    if let Some(n) = dest.iter().position(|&c| c == 0) {
        dest.truncate(n);
    }
    dest
}

/// (#89) `QuotedPrintableToUnicodeString(original)`: the bytes as [`quoted_printable_to_ascii_string`] decodes them,
/// paired into UTF-16 units low byte first; the string ends at the first 0 unit. An odd number of bytes leaves the last
/// unit's high byte 0 (upstream's `*c = 0`) and **does not end the string** in upstream's static buffer: it reads on
/// into what an earlier, longer call left there (from the first call, zeros). Here it ends after that unit (the earlier
/// calls' leftovers are not modelled; recorded in the module docs of `client::skirmish_game_options`). Upstream's
/// `i<1023` bound never moves (`i` is never incremented), so input past 2,047 decoded bytes overruns its buffer: here
/// the units go on.
pub fn quoted_printable_to_unicode_string(original: &[u8]) -> Vec<u16> {
    let bytes = {
        let mut dest = Vec::with_capacity(original.len());
        let mut i = 0;
        while i < original.len() && original[i] != 0 {
            if original[i] == b'_' {
                let Some(&d1) = original.get(i + 1).filter(|&&c| c != 0) else { break };
                let mut c = hex_digit_to_int(d1);
                i += 1;
                if let Some(&d2) = original.get(i + 1).filter(|&&c| c != 0) {
                    c = (c << 4) | hex_digit_to_int(d2);
                    i += 1;
                }
                dest.push(c);
            } else {
                dest.push(original[i]);
            }
            i += 1;
        }
        dest
    };
    let mut out: Vec<u16> = bytes.chunks(2).map(|p| u16::from(p[0]) | (u16::from(*p.get(1).unwrap_or(&0)) << 8)).collect();
    if let Some(n) = out.iter().position(|&u| u == 0) {
        out.truncate(n);
    }
    out
}

/// What stops `updateCache`: a throw out of an INI read, or `loadMap`'s `throw(ERROR_CORRUPT_FILE_FORMAT)` (a map
/// file whose chunks do not parse). Upstream catches neither here.
#[derive(Clone, Debug, PartialEq)]
pub enum MapCacheError {
    /// An INI read threw.
    Ini(IniError),
    /// `ERROR_CORRUPT_FILE_FORMAT`.
    CorruptFileFormat,
}

impl From<IniError> for MapCacheError {
    fn from(e: IniError) -> Self {
        MapCacheError::Ini(e)
    }
}

/// `updateCache`'s result.
pub type MapCacheResult<T> = Result<T, MapCacheError>;

/// `class MapMetaDataReader`: one `MapCache` block's fields before they become a [`MapMetaData`].
#[derive(Clone, Debug, Default)]
pub struct MapMetaDataReader {
    extent: Region3D,
    num_players: i32,
    is_multiplayer: bool,
    ascii_display_name: Vec<u8>,
    ascii_name_lookup_tag: Vec<u8>,
    is_official: bool,
    timestamp: WinTimeStamp,
    filesize: u32,
    crc: u32,
    waypoints: [Coord3D; MAX_SLOTS],
    initial_camera_position: Coord3D,
    supply_positions: Vec<Coord3D>,
    tech_positions: Vec<Coord3D>,
}

/// `MapMetaDataReader::m_mapFieldParseTable`.
static MAP_FIELDS: [FieldParse<MapMetaDataReader>; 23] = [
    FieldParse::new("isOfficial", |ini, r, _| { r.is_official = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("isMultiplayer", |ini, r, _| { r.is_multiplayer = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("extentMin", |ini, r, _| { r.extent.lo = ini.parse_coord3d()?; Ok(()) }, UserData::None),
    FieldParse::new("extentMax", |ini, r, _| { r.extent.hi = ini.parse_coord3d()?; Ok(()) }, UserData::None),
    FieldParse::new("numPlayers", |ini, r, _| { r.num_players = ini.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("fileSize", |ini, r, _| { r.filesize = ini.parse_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("fileCRC", |ini, r, _| { r.crc = ini.parse_unsigned_int()?; Ok(()) }, UserData::None),
    // (`INI::parseInt` into the `UnsignedInt` members: the bits as stored)
    FieldParse::new("timestampLo", |ini, r, _| { r.timestamp.low = ini.parse_int()? as u32; Ok(()) }, UserData::None),
    FieldParse::new("timestampHi", |ini, r, _| { r.timestamp.high = ini.parse_int()? as u32; Ok(()) }, UserData::None),
    FieldParse::new("displayName", |ini, r, _| { r.ascii_display_name = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("nameLookupTag", |ini, r, _| { r.ascii_name_lookup_tag = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    // `parseSupplyPositionCoord3D` and `parseTechPositionsCoord3D`: `push_front`
    FieldParse::new("supplyPosition", |ini, r, _| { r.supply_positions.insert(0, ini.parse_coord3d()?); Ok(()) }, UserData::None),
    FieldParse::new("techPosition", |ini, r, _| { r.tech_positions.insert(0, ini.parse_coord3d()?); Ok(()) }, UserData::None),
    FieldParse::new("Player_1_Start", |ini, r, _| { r.waypoints[0] = ini.parse_coord3d()?; Ok(()) }, UserData::None),
    FieldParse::new("Player_2_Start", |ini, r, _| { r.waypoints[1] = ini.parse_coord3d()?; Ok(()) }, UserData::None),
    FieldParse::new("Player_3_Start", |ini, r, _| { r.waypoints[2] = ini.parse_coord3d()?; Ok(()) }, UserData::None),
    FieldParse::new("Player_4_Start", |ini, r, _| { r.waypoints[3] = ini.parse_coord3d()?; Ok(()) }, UserData::None),
    FieldParse::new("Player_5_Start", |ini, r, _| { r.waypoints[4] = ini.parse_coord3d()?; Ok(()) }, UserData::None),
    FieldParse::new("Player_6_Start", |ini, r, _| { r.waypoints[5] = ini.parse_coord3d()?; Ok(()) }, UserData::None),
    FieldParse::new("Player_7_Start", |ini, r, _| { r.waypoints[6] = ini.parse_coord3d()?; Ok(()) }, UserData::None),
    FieldParse::new("Player_8_Start", |ini, r, _| { r.waypoints[7] = ini.parse_coord3d()?; Ok(()) }, UserData::None),
    FieldParse::new("InitialCameraPosition", |ini, r, _| { r.initial_camera_position = ini.parse_coord3d()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// The blocks of one `MapCache.ini` as read, in file order; each becomes an entry once the file is read
/// ([`MapCache::load_cache_ini`]), as the entry's display name needs the game text.
#[derive(Default)]
struct MapCacheIni {
    blocks: Vec<(Vec<u8>, MapMetaDataReader)>,
}

/// `INI::parseMapCacheDefinition`'s reading half: the name (quoted-printable) and the fields.
fn map_cache_block(ini: &mut Ini<'_>, c: &mut MapCacheIni) -> IniResult<()> {
    let token = ini.get_next_token(Some(b" \n\r\t"))?;
    let name = quoted_printable_to_ascii_string(&token);
    let mut mdr = MapMetaDataReader::default();
    let r = ini.init_from_ini(&mut mdr, &MAP_FIELDS);
    // (a block whose fields throw is not added: upstream adds after `initFromINI` returns)
    r?;
    c.blocks.push((name, mdr));
    Ok(())
}

/// `INI::parseMapCacheDefinition` is the one block of a map cache file.
static BLOCKS: [BlockParse<MapCacheIni>; 1] = [BlockParse { token: "MapCache", parse: map_cache_block }];

/// `registerParser("HeightMapData", "", ParseSizeOnlyInChunk)`.
fn parse_size_only_chunk(file: &mut DataChunkInput<HeightMapParse>, info: &DataChunkInfo, user: &mut HeightMapParse) -> ChunkResult<bool> {
    *user = LogicHeightMap::parse_chunk(file, info, true)?;
    Ok(true)
}

/// `registerParser("ObjectsList", "", ParseObjectsDataChunk)`.
fn parse_objects_data_chunk(file: &mut DataChunkInput<ObjectParse>, info: &DataChunkInfo, ctx: &mut ObjectParse) -> ChunkResult<bool> {
    file.register_parser(b"Object", &info.label, parse_object_data_chunk);
    file.parse(ctx)
}

/// The `WorldInfo` chunk's dictionary, read with the name keys.
struct WorldInfoParse {
    names: NameKeyGenerator,
    dict: Option<Dict>,
}

/// `registerParser("WorldInfo", "", ParseWorldDictDataChunk)`: `worldDict = file.readDict()`.
fn parse_world_dict_data_chunk(file: &mut DataChunkInput<WorldInfoParse>, _info: &DataChunkInfo, ctx: &mut WorldInfoParse) -> ChunkResult<bool> {
    ctx.dict = Some(file.read_dict(&mut ctx.names)?);
    Ok(true)
}

/// One pass over the file with only `label`'s parser (upstream registers all three in one pass; each chunk is read
/// the same either way, as no parser reads another's results).
fn parse_with<C>(bytes: &[u8], ctx: &mut C, label: &[u8], parser: crate::formats::data_chunk::DataChunkParser<C>) -> ChunkResult<bool> {
    let Some(stream) = CachedFileInputStream::new(bytes.to_vec()) else { return Ok(false) };
    let mut input = DataChunkInput::<C>::new(stream)?;
    input.register_parser(label, b"", parser);
    input.parse(ctx)
}

impl MapCache {
    /// `MapCache()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `getMapDir()`.
    pub fn get_map_dir(&self) -> Vec<u8> {
        b"Maps".to_vec()
    }

    /// `getUserMapDir()`: the user data path, then `Maps`.
    pub fn get_user_map_dir(&self, host: &impl MapCacheHost) -> Vec<u8> {
        let mut tmp = host.user_data_path();
        tmp.extend_from_slice(&self.get_map_dir());
        tmp
    }

    /// `getMapExtension()`.
    pub fn get_map_extension(&self) -> Vec<u8> {
        b"map".to_vec()
    }

    /// `addShippingMap(name)`.
    pub fn add_shipping_map(&mut self, map_name: &[u8]) {
        self.allowed_maps.insert(map_name.to_ascii_lowercase());
    }

    /// `findMap(name)`: the entry of the lower-cased name.
    pub fn find_map(&self, map_name: &[u8]) -> Option<&MapMetaData> {
        self.maps.get(&map_name.to_ascii_lowercase())
    }

    /// `updateCache()` (a release build's: the debug rebuild of the shipped cache is not translated). An error is a
    /// throw out of the INI reads or `loadMap` (`ERROR_CORRUPT_FILE_FORMAT`), which upstream does not catch here.
    pub fn update_cache(&mut self, host: &mut impl MapCacheHost) -> MapCacheResult<()> {
        // TheFileSystem->createDirectory(getUserMapDir()): the host makes directories as it writes
        if self.load_user_maps(host)? {
            self.write_cache_ini(true, host);
        }
        // "we shall overwrite info from matching user maps to prevent munkees from getting rowdy :)"
        self.load_standard_maps(host).map_err(MapCacheError::Ini)
    }

    /// `clearUnseenMaps(dirName)`: every entry not seen in this scan whose name starts with the directory is erased.
    fn clear_unseen_maps(&mut self, dir_name: &[u8]) -> bool {
        let dir = dir_name.to_ascii_lowercase();
        let mut erased_something = false;
        for (name, seen) in &self.seen {
            if !*seen && name.len() >= dir.len() && name[..dir.len()].eq_ignore_ascii_case(&dir) {
                self.maps.remove(name);
                erased_something = true;
            }
        }
        erased_something
    }

    /// `loadStandardMaps()`: `ini.load("Maps\MapCache.ini", INI_LOAD_OVERWRITE)`, which throws when the file is not
    /// there (a release build does not look first).
    fn load_standard_maps(&mut self, host: &mut impl MapCacheHost) -> IniResult<()> {
        let fname = self.cache_ini_path(&self.get_map_dir());
        let text = host.open_file(&fname).ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::CantOpenFile))?;
        self.load_cache_ini(&fname, &text, host)
    }

    /// `"%s\\%s"` of a directory and [`MAP_CACHE_NAME`].
    fn cache_ini_path(&self, dir: &[u8]) -> Vec<u8> {
        let mut fname = dir.to_vec();
        fname.push(b'\\');
        fname.extend_from_slice(MAP_CACHE_NAME);
        fname
    }

    /// `ini.load(fname, INI_LOAD_OVERWRITE, NULL)` of a map cache file: each block made an entry, as
    /// `parseMapCacheDefinition` does (the blocks read before a throw are added, then the throw returns).
    pub fn load_cache_ini(&mut self, fname: &[u8], text: &[u8], host: &impl MapCacheHost) -> IniResult<()> {
        let mut c = MapCacheIni::default();
        let r = Ini::load(fname, text, IniLoadType::Overwrite, None, &BLOCKS, &mut c);
        for (name, mdr) in c.blocks {
            self.add_from_reader(&name, mdr, host);
        }
        r
    }

    /// `parseMapCacheDefinition` after `initFromINI`: the entry made from the reader and stored under the lower-cased
    /// name, unless its display name is empty.
    fn add_from_reader(&mut self, name: &[u8], mdr: MapMetaDataReader, host: &impl MapCacheHost) {
        let mut md = MapMetaData {
            extent: mdr.extent,
            is_official: mdr.is_official,
            is_multiplayer: mdr.is_multiplayer,
            num_players: mdr.num_players,
            filesize: mdr.filesize,
            crc: mdr.crc,
            timestamp: mdr.timestamp,
            ..Default::default()
        };
        md.waypoints.map.insert(b"InitialCameraPosition".to_vec(), mdr.initial_camera_position);
        md.name_lookup_tag = quoted_printable_to_ascii_string(&mdr.ascii_name_lookup_tag);
        md.display_name = if md.name_lookup_tag.is_empty() {
            // maps without localized name tags
            display_name_from(after_last_backslash(name), md.num_players)
        } else {
            // official maps with name tags
            display_name_fetched(host, &md.name_lookup_tag, md.num_players)
        };
        for i in 0..usize::try_from(md.num_players).unwrap_or(0).min(MAX_SLOTS) {
            md.waypoints.map.insert(format!("Player_{}_Start", i + 1).into_bytes(), mdr.waypoints[i]);
        }
        // the reader's lists were built with `push_front`, and are read back with `push_front`: the file's order
        md.supply_positions = mdr.supply_positions.iter().rev().copied().collect();
        md.tech_positions = mdr.tech_positions.iter().rev().copied().collect();
        if !md.display_name.is_empty() {
            let lower_name = name.to_ascii_lowercase();
            md.file_name = lower_name.clone();
            self.maps.insert(lower_name, md);
        }
    }

    /// `loadUserMaps()`: the user's map directory (the shipped one when building the cache) scanned for `*.map`, each
    /// map at `<dir>\<name>\<name>.map` added. True when a map was parsed or an entry erased (the cache file is then
    /// written again).
    fn load_user_maps(&mut self, host: &mut impl MapCacheHost) -> MapCacheResult<bool> {
        let build = host.build_map_cache();
        let map_dir = if build {
            self.get_map_dir()
        } else {
            let dir = self.get_user_map_dir(host);
            let fname = self.cache_ini_path(&dir);
            if let Some(text) = host.open_file(&fname) {
                self.load_cache_ini(&fname, &text, host)?;
            }
            dir
        };
        // mark all as unseen
        self.seen = self.maps.keys().map(|k| (k.clone(), false)).collect();
        let mut toplevel = map_dir.clone();
        toplevel.push(b'\\');
        let mut pattern = b"*.".to_vec();
        pattern.extend_from_slice(&self.get_map_extension());
        let mut list = FilenameList::new();
        host.get_file_list_in_directory(&toplevel, &pattern, &mut list, true);
        let mut parsed_a_map = false;
        let names: Vec<Vec<u8>> = list.iter().map(<[u8]>::to_vec).collect();
        for file in names {
            let temp = file.to_ascii_lowercase();
            if !temp.contains(&b'\\') {
                // DEBUG_CRASH(("Couldn't find \\ in map name!"))
                continue;
            }
            let mut fname = after_last_backslash(&temp).to_vec();
            fname.truncate(fname.len().saturating_sub(MAP_EXTENSION.len()));
            let mut ending = fname.clone();
            ending.push(b'\\');
            ending.extend_from_slice(&fname);
            ending.extend_from_slice(MAP_EXTENSION);
            let skip = build && !self.allowed_maps.is_empty() && !self.allowed_maps.contains(&fname);
            if skip || !temp.ends_with(&ending) {
                // (a map in the wrong spot is a DEBUG_CRASH, and skipped)
                continue;
            }
            if let Some(info) = host.get_file_info(&temp) {
                self.seen.insert(temp.clone(), true);
                parsed_a_map |= self.add_map(&map_dir, &file, Some(info), build, host)?;
            }
        }
        // clean out unseen maps
        if self.clear_unseen_maps(&map_dir) {
            return Ok(true);
        }
        Ok(parsed_a_map)
    }

    /// `addMap(dirName, fname, fileInfo, isOfficial)`: an entry of the same size with a CRC is kept (its display name
    /// fetched again); otherwise the map is loaded and its entry made. True when the map was parsed.
    pub fn add_map(&mut self, dir_name: &[u8], fname: &[u8], file_info: Option<MapFileInfo>, is_official: bool, host: &mut impl MapCacheHost) -> MapCacheResult<bool> {
        let Some(file_info) = file_info else { return Ok(false) };
        let lower_fname = fname.to_ascii_lowercase();
        let filesize = file_info.size_low;
        let mut md = MapMetaData::default();
        if let Some(found) = self.maps.get(&lower_fname) {
            md = found.clone();
            if md.filesize == filesize && md.crc != 0 {
                // force a lookup so that we don't display the English localization in all builds
                let name = if md.name_lookup_tag.is_empty() {
                    display_name_from(after_last_backslash(fname), md.num_players)
                } else {
                    display_name_fetched(host, &md.name_lookup_tag, md.num_players)
                };
                if let Some(e) = self.maps.get_mut(&lower_fname) {
                    e.display_name = name;
                }
                return Ok(false);
            }
        }
        // just load for querying the data, since we aren't playing this map
        self.load_map(fname, host)?;
        md.file_name = lower_fname.clone();
        md.filesize = filesize;
        md.is_official = is_official;
        self.waypoint_map_update(&mut md.waypoints);
        md.num_players = md.waypoints.num_start_spots;
        md.is_multiplayer = md.num_players >= 2;
        md.timestamp = WinTimeStamp { low: file_info.timestamp_low, high: file_info.timestamp_high };
        md.supply_positions = self.statics.supply_positions.clone();
        md.tech_positions = self.statics.tech_positions.clone();
        md.crc = Self::calc_crc(fname, host);
        let key = host.names().name_to_key(b"mapName");
        let (munkee, exists) = self.statics.world_dict.get_ascii_string(key);
        md.name_lookup_tag = munkee.clone();
        if !exists || munkee.is_empty() {
            md.display_name = display_name_from(after_last_backslash(fname), md.num_players);
            host.reset_game_text();
        } else {
            // "%s\\%s" of the directory and the file name (itself a path from the directory: upstream's), less
            // ".map", then "\map.str"
            let mut string_file_name = dir_name.to_vec();
            string_file_name.push(b'\\');
            string_file_name.extend_from_slice(fname);
            string_file_name.truncate(string_file_name.len().saturating_sub(4));
            string_file_name.extend_from_slice(b"\\map.str");
            host.init_map_string_file(&string_file_name);
            md.display_name = display_name_fetched(host, &munkee, md.num_players);
            host.reset_game_text();
        }
        md.extent = self.get_extent();
        self.maps.insert(lower_fname, md);
        self.reset_map();
        Ok(true)
    }

    /// `calcCRC(dirName, fname)`: the CRC of the file's bytes, 0 when it does not open.
    fn calc_crc(fname: &[u8], host: &impl MapCacheHost) -> u32 {
        let Some(bytes) = host.open_file(fname) else { return 0 };
        let mut crc = Crc::new();
        crc.clear();
        for chunk in bytes.chunks(4096) {
            crc.compute_crc(chunk);
        }
        crc.get()
    }

    /// `loadMap(filename)`: the height map's size, the world dictionary and the objects' waypoints, tech buildings
    /// and supply sources into the statics. False when the file does not open (the statics keep what they held).
    fn load_map(&mut self, filename: &[u8], host: &mut impl MapCacheHost) -> MapCacheResult<bool> {
        let corrupt = || MapCacheError::CorruptFileFormat;
        let Some(bytes) = host.open_file(filename) else { return Ok(false) };
        if CachedFileInputStream::new(bytes.clone()).is_none() {
            return Ok(false);
        }
        self.statics.waypoints = Some(BTreeMap::new());
        // HeightMapData: ParseSizeOnly
        let mut hm = HeightMapParse::default();
        if !parse_with(&bytes, &mut hm, b"HeightMapData", parse_size_only_chunk).map_err(|_| corrupt())? {
            return Err(corrupt());
        }
        // (a file with no `HeightMapData` chunk leaves the sizes of the map before)
        if hm != HeightMapParse::default() {
            (self.statics.width, self.statics.height, self.statics.border_size) = hm.size();
        }
        // WorldInfo
        let mut wi = WorldInfoParse { names: std::mem::take(host.names()), dict: None };
        let r = parse_with(&bytes, &mut wi, b"WorldInfo", parse_world_dict_data_chunk);
        *host.names() = std::mem::take(&mut wi.names);
        r.map_err(|_| corrupt())?;
        if let Some(d) = wi.dict {
            self.statics.world_dict = d;
        }
        // ObjectsList
        let mut op = ObjectParse::new();
        op.names = std::mem::take(host.names());
        let r = parse_with(&bytes, &mut op, b"ObjectsList", parse_objects_data_chunk);
        *host.names() = std::mem::take(&mut op.names);
        r.map_err(|_| corrupt())?;
        if op.undefined.is_some() {
            return Err(corrupt());
        }
        let waypoints = self.statics.waypoints.get_or_insert_with(BTreeMap::new);
        for o in &op.objects {
            if o.is_waypoint() {
                // (*m_waypoints)[getWaypointName()] = loc: the parse's own map (the later one wins)
                continue;
            }
            let (tech, supply) = o.thing_template().map_or((false, false), |n| host.tech_or_supply(n));
            if tech {
                self.statics.tech_positions.push(o.location());
            } else if supply {
                self.statics.supply_positions.push(o.location());
            }
        }
        waypoints.extend(op.waypoints);
        self.statics.map_dx = self.statics.width - 2 * self.statics.border_size;
        self.statics.map_dy = self.statics.height - 2 * self.statics.border_size;
        Ok(true)
    }

    /// `resetMap()`.
    fn reset_map(&mut self) {
        self.statics.waypoints = None;
        self.statics.tech_positions.clear();
        self.statics.supply_positions.clear();
    }

    /// `getExtent(&extent)`: from the origin to the playable size in world units, at height 0.
    fn get_extent(&self) -> Region3D {
        let mut e = Region3D::default();
        e.hi.x = self.statics.map_dx as f32 * MAP_XY_FACTOR;
        e.hi.y = self.statics.map_dy as f32 * MAP_XY_FACTOR;
        e
    }

    /// `WaypointMap::update()`: the initial camera position and the start waypoints `Player_1_Start` on, up to the
    /// first missing one, from the last loaded map's waypoints. With none loaded, one start spot and the map as it was.
    fn waypoint_map_update(&self, wm: &mut WaypointMap) {
        let Some(waypoints) = &self.statics.waypoints else {
            wm.num_start_spots = 1;
            return;
        };
        wm.map.clear();
        if let Some(p) = waypoints.get(&b"InitialCameraPosition"[..]) {
            wm.map.insert(b"InitialCameraPosition".to_vec(), *p);
        }
        wm.num_start_spots = 0;
        for i in 0..MAX_SLOTS {
            let name = format!("Player_{}_Start", i + 1).into_bytes();
            match waypoints.get(&name) {
                Some(p) => {
                    wm.map.insert(name, *p);
                    wm.num_start_spots += 1;
                }
                None => break,
            }
        }
        wm.num_start_spots = wm.num_start_spots.max(1);
    }

    /// `writeCacheINI(userDir)`: the entries under the directory written as `MapCache` blocks to
    /// `<dir>\MapCache.ini`.
    pub fn write_cache_ini(&self, user_dir: bool, host: &mut impl MapCacheHost) {
        let map_dir = if !user_dir || host.build_map_cache() { self.get_map_dir() } else { self.get_user_map_dir(host) };
        let filepath = self.cache_ini_path(&map_dir);
        let mut out = Vec::new();
        let f = |c: &Coord3D| format!("X:{:2.2} Y:{:2.2} Z:{:2.2}", c.x, c.y, c.z);
        out.extend_from_slice(b"; FILE: ");
        out.extend_from_slice(&filepath);
        out.extend_from_slice(b" /////////////////////////////////////////////////////////////\n");
        out.extend_from_slice(b"; This INI file is auto-generated - do not modify\n");
        out.extend_from_slice(b"; /////////////////////////////////////////////////////////////////////////////\n");
        let dir = map_dir.to_ascii_lowercase();
        for (name, md) in &self.maps {
            if !(name.len() >= dir.len() && name[..dir.len()].eq_ignore_ascii_case(&dir)) {
                continue;
            }
            out.extend_from_slice(b"\nMapCache ");
            out.extend_from_slice(&ascii_string_to_quoted_printable(name));
            out.push(b'\n');
            let mut s = String::new();
            s += &format!("  fileSize = {}\n", md.filesize);
            s += &format!("  fileCRC = {}\n", md.crc);
            s += &format!("  timestampLo = {}\n", md.timestamp.low as i32);
            s += &format!("  timestampHi = {}\n", md.timestamp.high as i32);
            s += &format!("  isOfficial = {}\n", if md.is_official { "yes" } else { "no" });
            s += &format!("  isMultiplayer = {}\n", if md.is_multiplayer { "yes" } else { "no" });
            s += &format!("  numPlayers = {}\n", md.num_players);
            s += &format!("  extentMin = {}\n", f(&md.extent.lo));
            s += &format!("  extentMax = {}\n", f(&md.extent.hi));
            out.extend_from_slice(s.as_bytes());
            out.extend_from_slice(b"  nameLookupTag = ");
            out.extend_from_slice(&md.name_lookup_tag);
            out.push(b'\n');
            for (wname, pos) in &md.waypoints.map {
                out.extend_from_slice(b"  ");
                out.extend_from_slice(wname);
                out.extend_from_slice(format!(" = {}\n", f(pos)).as_bytes());
            }
            for pos in &md.tech_positions {
                out.extend_from_slice(format!("  techPosition = {}\n", f(pos)).as_bytes());
            }
            for pos in &md.supply_positions {
                out.extend_from_slice(format!("  supplyPosition = {}\n", f(pos)).as_bytes());
            }
            out.extend_from_slice(b"END\n\n");
        }
        host.write_file(&filepath, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A host over files in memory (lower-cased paths), with no game text but the `MISSING:` string.
    #[derive(Default)]
    struct Host {
        files: BTreeMap<Vec<u8>, Vec<u8>>,
        user: Vec<u8>,
        written: Vec<(Vec<u8>, Vec<u8>)>,
        names: NameKeyGenerator,
        tech: Vec<Vec<u8>>,
    }

    impl MapCacheHost for Host {
        fn get_file_list_in_directory(&self, dir: &[u8], _pattern: &[u8], list: &mut FilenameList, _recursive: bool) {
            let d = dir.to_ascii_lowercase();
            for k in self.files.keys().filter(|k| k.starts_with(&d) && k.ends_with(b".map")) {
                list.insert(k.clone());
            }
        }
        fn get_file_info(&self, path: &[u8]) -> Option<MapFileInfo> {
            self.files.get(&path.to_ascii_lowercase()).map(|f| MapFileInfo { size_low: f.len() as u32, ..Default::default() })
        }
        fn open_file(&self, path: &[u8]) -> Option<Vec<u8>> {
            self.files.get(&path.to_ascii_lowercase()).cloned()
        }
        fn user_data_path(&self) -> Vec<u8> {
            self.user.clone()
        }
        fn build_map_cache(&self) -> bool {
            false
        }
        fn fetch(&self, label: &[u8]) -> UnicodeString {
            use crate::common::science::GameTextFetch;
            crate::logic::world::game_data::TextTable::default().fetch(label)
        }
        fn init_map_string_file(&mut self, _path: &[u8]) {}
        fn reset_game_text(&mut self) {}
        fn tech_or_supply(&self, template: &[u8]) -> (bool, bool) {
            (self.tech.iter().any(|t| t == template), false)
        }
        fn write_file(&mut self, path: &[u8], text: Vec<u8>) {
            self.written.push((path.to_vec(), text));
        }
        fn names(&mut self) -> &mut NameKeyGenerator {
            &mut self.names
        }
    }

    fn host() -> Host {
        let mut names = NameKeyGenerator::new();
        names.init();
        Host { names, user: b"User\\".to_vec(), ..Default::default() }
    }

    /// A map file: a 70 by 60 height map with a border of 5, and the objects of [`a_user_map_is_parsed_into_an_entry`]
    /// (four waypoints and a tech building), with no `WorldInfo` chunk.
    fn duel_map(names: &mut NameKeyGenerator) -> Vec<u8> {
        use crate::formats::data_chunk_output::DataChunkOutput;
        use crate::logic::map_object::K_OBJECTS_VERSION_3;
        let mut out = DataChunkOutput::new();
        out.open_data_chunk(b"HeightMapData", crate::logic::terrain_visual::K_HEIGHT_MAP_VERSION_4);
        for v in [70, 60, 5, 0, 4200] {
            out.write_int(v);
        }
        out.write_array_of_bytes(&[4u8; 4200]);
        out.close_data_chunk();
        out.open_data_chunk(b"ObjectsList", K_OBJECTS_VERSION_3);
        let waypoints = [(&b"Player_1_Start"[..], 100.0, 100.0), (b"Player_2_Start", 500.0, 400.0), (b"InitialCameraPosition", 50.0, 60.0), (b"Other", 1.0, 1.0)];
        for (i, (name, x, y)) in waypoints.into_iter().enumerate() {
            out.open_data_chunk(b"Object", K_OBJECTS_VERSION_3);
            for r in [x, y, 0.0, 0.0] {
                out.write_real(r);
            }
            out.write_int(0);
            out.write_ascii_string(b"*Waypoints/Waypoint");
            let mut props = Dict::new();
            props.set_int(names.name_to_key(b"waypointID"), i as i32 + 1).unwrap();
            props.set_ascii_string(names.name_to_key(b"waypointName"), name).unwrap();
            out.write_dict(&props, names);
            out.close_data_chunk();
        }
        out.open_data_chunk(b"Object", K_OBJECTS_VERSION_3);
        for r in [300.0, 250.0, 0.0, 0.0] {
            out.write_real(r);
        }
        out.write_int(0);
        out.write_ascii_string(b"TechOilDerrick");
        out.write_dict(&Dict::new(), names);
        out.close_data_chunk();
        out.close_data_chunk();
        out.finish()
    }

    /// Batch I56: the quoted-printable pair. Predicted from QuotedPrintable.cpp: a space and a backslash are `_20` and
    /// `_5C` (upper-case digits), letters and digits are kept, and decoding gives the name back; a trailing `_` ends it.
    #[test]
    fn map_names_round_trip_through_quoted_printable() {
        let name = b"maps\\alpine assault\\alpine assault.map";
        let q = ascii_string_to_quoted_printable(name);
        assert_eq!(q, b"maps_5Calpine_20assault_5Calpine_20assault_2Emap".to_vec());
        assert_eq!(quoted_printable_to_ascii_string(&q), name.to_vec());
        assert_eq!(quoted_printable_to_ascii_string(b"ab_"), b"ab".to_vec());
        assert_eq!(quoted_printable_to_ascii_string(b"ab_4"), b"ab\x04".to_vec());
        // a byte above 0x7F ends the encoding (a negative `char`'s first digit is '\0')
        assert_eq!(ascii_string_to_quoted_printable(b"a\xE9b"), b"a_".to_vec());
    }

    /// #89: the Unicode pair (`Skirmish.ini`'s `UserName`). Predicted from QuotedPrintable.cpp before the run: `Al 1`
    /// is the bytes `A 0 l 0 space 0 1 0`, each 0 and the space written `_XX`, the letters and the digit kept:
    /// `A_00l_00_20_001_00`; decoding gives `Al 1` back; `é` (0x00E9) is `_` then the negative byte's `'\0'`, which ends
    /// the encoding; an odd byte count leaves the last unit's high byte 0 (`_41` alone is `A`); a 0 unit ends it.
    ///
    /// **Result (2026-11-10):** as predicted.
    #[test]
    fn user_names_round_trip_through_quoted_printable() {
        let name: Vec<u16> = "Al 1".encode_utf16().collect();
        let q = unicode_string_to_quoted_printable(&name);
        assert_eq!(q, b"A_00l_00_20_001_00".to_vec());
        assert_eq!(quoted_printable_to_unicode_string(&q), name);
        assert_eq!(unicode_string_to_quoted_printable(&[u16::from(b'a'), 0xE9]), b"a_00_".to_vec());
        assert_eq!(quoted_printable_to_unicode_string(b"_41"), vec![0x41]);
        assert_eq!(quoted_printable_to_unicode_string(b"A_00_00_00B_00"), vec![0x41]);
    }

    /// Batch I56: a shipped `MapCache.ini` block (the form `writeCacheINI` writes and `MapsZH.big` carries). Predicted
    /// from `parseMapCacheDefinition`: the name decoded and lower-cased, the player count's start waypoints and the
    /// initial camera position kept, a fifth `Player_5_Start` beyond `numPlayers` dropped, the supply positions in file
    /// order, the CRC and size as written, and a display name of the fetched tag (here the `MISSING:` text) with
    /// " (4)".
    #[test]
    fn a_shipped_map_cache_block_becomes_an_entry() {
        let mut h = host();
        let ini = b"MapCache maps_5CTournament_20Desert_5CTournament_20Desert_2Emap\n\
            fileSize = 123456\n fileCRC = 3735928559\n isOfficial = yes\n isMultiplayer = yes\n numPlayers = 4\n\
            extentMin = X:0.00 Y:0.00 Z:0.00\n extentMax = X:4000.00 Y:3000.00 Z:0.00\n nameLookupTag = MAP:TournamentDesert\n\
            InitialCameraPosition = X:10.00 Y:20.00 Z:0.00\n Player_1_Start = X:100.00 Y:100.00 Z:0.00\n\
            Player_2_Start = X:200.00 Y:100.00 Z:0.00\n Player_3_Start = X:100.00 Y:200.00 Z:0.00\n\
            Player_4_Start = X:200.00 Y:200.00 Z:0.00\n Player_5_Start = X:300.00 Y:300.00 Z:0.00\n\
            supplyPosition = X:1.00 Y:1.00 Z:0.00\n supplyPosition = X:2.00 Y:2.00 Z:0.00\nEND\n";
        h.files.insert(b"maps\\mapcache.ini".to_vec(), ini.to_vec());
        let mut c = MapCache::new();
        c.update_cache(&mut h).expect("the cache");
        let md = c.find_map(b"Maps\\Tournament Desert\\Tournament Desert.map").expect("the entry");
        assert_eq!((md.num_players, md.crc, md.filesize, md.is_official), (4, 0xDEAD_BEEF, 123_456, true));
        assert_eq!(md.waypoints.map.len(), 5, "four start waypoints and the camera");
        assert_eq!(md.waypoints.map[&b"Player_4_Start"[..]], Coord3D { x: 200.0, y: 200.0, z: 0.0 });
        assert!(!md.waypoints.map.contains_key(&b"Player_5_Start"[..]));
        assert_eq!(md.supply_positions, vec![Coord3D { x: 1.0, y: 1.0, z: 0.0 }, Coord3D { x: 2.0, y: 2.0, z: 0.0 }]);
        let shown = String::from_utf16_lossy(md.display_name.str());
        assert_eq!(shown, "MISSING: 'MAP:TournamentDesert' (4)");
        assert_eq!(md.extent.hi.x, 4000.0);
        // no user map was parsed: nothing written
        assert!(h.written.is_empty());
    }

    /// Batch I56: `loadStandardMaps` in a release build loads `Maps\MapCache.ini` without looking for it first: with no
    /// such file the INI load throws. Predicted: `CantOpenFile`.
    #[test]
    fn the_shipped_cache_file_must_exist() {
        let mut h = host();
        let r = MapCache::new().update_cache(&mut h);
        assert_eq!(r.err(), Some(MapCacheError::Ini(IniError::new(IniErrorKind::CantOpenFile))));
    }

    /// Batch I56: a user map parsed by `addMap`. The map file is the gate's map writer's output with two start
    /// waypoints, the camera, and a tech building. Predicted from `addMap`: two players (multiplayer), the waypoints
    /// `Player_1_Start`, `Player_2_Start` and the camera (another waypoint dropped), the tech building's position, the
    /// CRC of the file's bytes, the extent of the playable cells times 10, a display name of the file's name with
    /// " (2)" (no `mapName` in the world dictionary), and the user cache file written. A second update finds the entry
    /// by its size and CRC and parses nothing.
    #[test]
    fn a_user_map_is_parsed_into_an_entry() {
        let mut h = host();
        h.files.insert(b"maps\\mapcache.ini".to_vec(), Vec::new());
        h.tech.push(b"TechOilDerrick".to_vec());
        let bytes = duel_map(&mut h.names);
        h.files.insert(b"user\\maps\\duel\\duel.map".to_vec(), bytes.clone());
        let mut c = MapCache::new();
        c.update_cache(&mut h).expect("the cache");
        let md = c.find_map(b"User\\Maps\\Duel\\Duel.map").expect("the entry").clone();
        assert_eq!((md.num_players, md.is_multiplayer, md.filesize), (2, true, bytes.len() as u32));
        let names: Vec<&[u8]> = md.waypoints.map.keys().map(Vec::as_slice).collect();
        assert_eq!(names, vec![&b"InitialCameraPosition"[..], b"Player_1_Start", b"Player_2_Start"]);
        assert_eq!(md.tech_positions, vec![Coord3D { x: 300.0, y: 250.0, z: 0.0 }]);
        let mut crc = Crc::new();
        crc.compute_crc(&bytes);
        assert_eq!(md.crc, crc.get());
        assert_eq!((md.extent.hi.x, md.extent.hi.y), (600.0, 500.0));
        assert_eq!(String::from_utf16_lossy(md.display_name.str()), "duel.map (2)");
        assert_eq!(h.written.len(), 1);
        assert_eq!(h.written[0].0, b"User\\Maps\\MapCache.ini".to_vec());
        // the written file reads back as the same entry
        let mut again = MapCache::new();
        again.load_cache_ini(b"x", &h.written[0].1, &h).expect("the written file");
        let back = again.find_map(b"user\\maps\\duel\\duel.map").expect("read back");
        assert_eq!((back.num_players, back.crc, back.filesize), (2, md.crc, md.filesize));
        // a second update: the entry checks out, nothing is parsed or written
        c.update_cache(&mut h).expect("again");
        assert_eq!(h.written.len(), 1);
    }
}
