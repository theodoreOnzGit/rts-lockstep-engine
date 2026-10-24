// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/TerrainTypes.h
//   GeneralsMD/Code/GameEngine/Source/Common/TerrainTypes.cpp
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INITerrain.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, December 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `TerrainType` (one terrain entry of `Terrain.ini`: its texture, its class
//! and whether it has blend edges or forbids construction) and
//! `TerrainTypeCollection` (`TheTerrainTypes`, every entry), with the
//! `Terrain` INI block handler.
//!
//! # Translation choices
//!
//! - The collection's linked list (`m_terrainList`, prepended to) is a `Vec`
//!   in insertion order. [`TerrainTypeCollection::terrains`] yields the list
//!   order of upstream's `firstTerrain`/`nextTerrain`: newest first.
//! - `newTerrain` copies the `DefaultTerrain` entry (upstream's `*terrain =
//!   *defaultTerrain`, whose `m_next` is then overwritten). Only the fields
//!   the entry has are copied; the copy's name and position are its own.
//! - `TerrainClass` is an enum; upstream's `terrainTypeNames[]` is
//!   [`TERRAIN_TYPE_NAMES`], index for index.
//! - The `DEBUG_ASSERTCRASH` of `parseTerrainDefinition` compiles to nothing
//!   in the release configuration upstream (`Debug.h`), so it is not
//!   translated.
//!
//! # Left out: methods that take an `Object` or a `Player`
//!
//! None. Nothing in `TerrainTypes.h`, `TerrainTypes.cpp` or `INITerrain.cpp`
//! takes an `Object` or a `Player`.

use crate::common::ascii_string::AsciiString;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, UserData};
use crate::common::subsystem_interface::SubsystemInterface;

/// `TerrainClass`: the type classification of a terrain, "keep this in sync
/// with terrainTypeNames[]" (upstream). The order is the upstream enum's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum TerrainClass {
    #[default]
    None = 0,
    Desert1,
    Desert2,
    Desert3,
    EasternEurope1,
    EasternEurope2,
    EasternEurope3,
    Swiss1,
    Swiss2,
    Swiss3,
    Snow1,
    Snow2,
    Snow3,
    Dirt,
    Grass,
    Transition,
    Rock,
    Sand,
    Cliff,
    Wood,
    BlendEdges,
    LiveDesert,
    DryDesert,
    AccentSand,
    TropicalBeach,
    BeachPark,
    RuggedMountain,
    CobblestoneGrass,
    AccentGrass,
    Residential,
    RuggedSnow,
    FlatSnow,
    Field,
    Asphalt,
    Concrete,
    China,
    AccentRock,
    Urban,
}

/// `TERRAIN_NUM_CLASSES`: the number of classes.
pub const TERRAIN_NUM_CLASSES: i32 = 38;

/// `terrainTypeNames[]` (TerrainTypes.h, `DEFINE_TERRAIN_TYPE_NAMES`), without
/// the `NULL` terminator, in enum order. The INI names are the upstream
/// spellings (`BLEND_EDGE`, not `BLEND_EDGES`).
pub const TERRAIN_TYPE_NAMES: [&str; 38] = [
    "NONE",
    "DESERT_1",
    "DESERT_2",
    "DESERT_3",
    "EASTERN_EUROPE_1",
    "EASTERN_EUROPE_2",
    "EASTERN_EUROPE_3",
    "SWISS_1",
    "SWISS_2",
    "SWISS_3",
    "SNOW_1",
    "SNOW_2",
    "SNOW_3",
    "DIRT",
    "GRASS",
    "TRANSITION",
    "ROCK",
    "SAND",
    "CLIFF",
    "WOOD",
    "BLEND_EDGE",
    "DESERT_LIVE",
    "DESERT_DRY",
    "SAND_ACCENT",
    "BEACH_TROPICAL",
    "BEACH_PARK",
    "MOUNTAIN_RUGGED",
    "GRASS_COBBLESTONE",
    "GRASS_ACCENT",
    "RESIDENTIAL",
    "SNOW_RUGGED",
    "SNOW_FLAT",
    "FIELD",
    "ASPHALT",
    "CONCRETE",
    "CHINA",
    "ROCK_ACCENT",
    "URBAN",
];

impl TerrainClass {
    /// The class with this enum value, or `None` outside `[0, TERRAIN_NUM_CLASSES)`.
    pub fn from_index(i: i32) -> Option<TerrainClass> {
        usize::try_from(i)
            .ok()
            .and_then(|i| ALL_CLASSES.get(i).copied())
    }
}

/// Every class, in enum order (the index of each is its value).
const ALL_CLASSES: [TerrainClass; 38] = [
    TerrainClass::None,
    TerrainClass::Desert1,
    TerrainClass::Desert2,
    TerrainClass::Desert3,
    TerrainClass::EasternEurope1,
    TerrainClass::EasternEurope2,
    TerrainClass::EasternEurope3,
    TerrainClass::Swiss1,
    TerrainClass::Swiss2,
    TerrainClass::Swiss3,
    TerrainClass::Snow1,
    TerrainClass::Snow2,
    TerrainClass::Snow3,
    TerrainClass::Dirt,
    TerrainClass::Grass,
    TerrainClass::Transition,
    TerrainClass::Rock,
    TerrainClass::Sand,
    TerrainClass::Cliff,
    TerrainClass::Wood,
    TerrainClass::BlendEdges,
    TerrainClass::LiveDesert,
    TerrainClass::DryDesert,
    TerrainClass::AccentSand,
    TerrainClass::TropicalBeach,
    TerrainClass::BeachPark,
    TerrainClass::RuggedMountain,
    TerrainClass::CobblestoneGrass,
    TerrainClass::AccentGrass,
    TerrainClass::Residential,
    TerrainClass::RuggedSnow,
    TerrainClass::FlatSnow,
    TerrainClass::Field,
    TerrainClass::Asphalt,
    TerrainClass::Concrete,
    TerrainClass::China,
    TerrainClass::AccentRock,
    TerrainClass::Urban,
];

/// `class TerrainType`: a terrain definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerrainType {
    /// `m_name`: the terrain entry's name.
    name: Vec<u8>,
    /// `m_texture`: the `.tga` file of the terrain (a client name).
    texture: Vec<u8>,
    /// `m_blendEdgeTexture`: "contains custom blend edges".
    blend_edge_texture: bool,
    /// `m_class`: the type classification.
    class: TerrainClass,
    /// `m_restrictConstruction`: "do not allow construction on this terrain tile".
    restrict_construction: bool,
}

impl Default for TerrainType {
    /// `TerrainType()`.
    fn default() -> Self {
        TerrainType {
            name: Vec::new(),
            texture: Vec::new(),
            blend_edge_texture: false,
            class: TerrainClass::None,
            restrict_construction: false,
        }
    }
}

impl TerrainType {
    /// `TerrainType()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `getName()`.
    pub fn name(&self) -> &[u8] {
        &self.name
    }

    /// `isBlendEdge()`.
    pub fn is_blend_edge(&self) -> bool {
        self.blend_edge_texture
    }

    /// `getClass()`.
    pub fn class(&self) -> TerrainClass {
        self.class
    }

    /// `getRestrictConstruction()`.
    pub fn restrict_construction(&self) -> bool {
        self.restrict_construction
    }

    /// `getTexture()`.
    pub fn texture(&self) -> &[u8] {
        &self.texture
    }
}

/// `class TerrainTypeCollection`: every terrain type (`TheTerrainTypes`).
#[derive(Clone, Debug)]
pub struct TerrainTypeCollection {
    /// `m_name` (the subsystem's name).
    name: AsciiString,
    /// `m_terrainList`, in insertion order (upstream prepends, so its list
    /// order is the reverse; see [`TerrainTypeCollection::terrains`]).
    terrain_list: Vec<TerrainType>,
}

impl Default for TerrainTypeCollection {
    /// `TerrainTypeCollection()`: an empty list.
    fn default() -> Self {
        TerrainTypeCollection {
            name: AsciiString::default(),
            terrain_list: Vec::new(),
        }
    }
}

impl SubsystemInterface for TerrainTypeCollection {
    fn name(&self) -> &AsciiString {
        &self.name
    }

    fn set_name(&mut self, name: AsciiString) {
        self.name = name;
    }

    /// `init() { }`.
    fn init(&mut self) {}

    /// `reset() { }`.
    fn reset(&mut self) {}

    /// `update() { }`.
    fn update(&mut self) {}
}

impl TerrainTypeCollection {
    /// `TerrainTypeCollection()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// The position of the terrain called `name` (`findTerrain`'s search, which
    /// compares names with `AsciiString ==`).
    fn index_of(&self, name: &[u8]) -> Option<usize> {
        self.terrain_list.iter().position(|t| t.name == name)
    }

    /// `findTerrain(name)`: "not found" is `None`.
    pub fn find_terrain(&self, name: &[u8]) -> Option<&TerrainType> {
        self.index_of(name).map(|i| &self.terrain_list[i])
    }

    /// `newTerrain(name)`: a new entry, with the `DefaultTerrain` entry's
    /// values if there is one, appended to the list. Returns its position.
    ///
    /// Upstream does not check that `name` is new: the INI handler calls it
    /// only after `findTerrain` returned `NULL`, and so does this.
    pub fn new_terrain(&mut self, name: &[u8]) -> usize {
        let mut terrain = TerrainType::new();
        if let Some(default) = self.find_terrain(b"DefaultTerrain") {
            terrain.texture = default.texture.clone();
            terrain.blend_edge_texture = default.blend_edge_texture;
            terrain.class = default.class;
            terrain.restrict_construction = default.restrict_construction;
        }
        terrain.name = name.to_vec();
        self.terrain_list.push(terrain);
        self.terrain_list.len() - 1
    }

    /// Every terrain, in upstream's list order (`firstTerrain`, then
    /// `nextTerrain`): the most recently added first.
    pub fn terrains(&self) -> impl Iterator<Item = &TerrainType> {
        self.terrain_list.iter().rev()
    }

    /// The number of terrains.
    pub fn len(&self) -> usize {
        self.terrain_list.len()
    }

    /// Whether there are no terrains.
    pub fn is_empty(&self) -> bool {
        self.terrain_list.is_empty()
    }

    /// `INI::parseTerrainDefinition`: the entry's name, the entry (found, or
    /// new), then its fields until `End`. A field that fails leaves the
    /// entry as far as it was filled, as upstream.
    pub fn parse_terrain_definition(&mut self, ini: &mut Ini<'_>) -> IniResult<()> {
        let name = ini.get_next_token(None)?;
        let index = match self.index_of(&name) {
            Some(i) => i,
            None => self.new_terrain(&name),
        };
        ini.init_from_ini(&mut self.terrain_list[index], &TERRAIN_TYPE_FIELDS)
    }
}

/// `TerrainType::m_terrainTypeFieldParseTable`.
static TERRAIN_TYPE_FIELDS: [FieldParse<TerrainType>; 5] = [
    FieldParse::new("Texture", parse_texture, UserData::None),
    FieldParse::new("BlendEdges", parse_blend_edges, UserData::None),
    FieldParse::new("Class", parse_class, UserData::Names(&TERRAIN_TYPE_NAMES)),
    FieldParse::new(
        "RestrictConstruction",
        parse_restrict_construction,
        UserData::None,
    ),
    FieldParse::end(),
];

fn parse_texture(ini: &mut Ini<'_>, t: &mut TerrainType, _ud: &UserData) -> IniResult<()> {
    t.texture = ini.parse_ascii_string()?;
    Ok(())
}

fn parse_blend_edges(ini: &mut Ini<'_>, t: &mut TerrainType, _ud: &UserData) -> IniResult<()> {
    t.blend_edge_texture = ini.parse_bool()?;
    Ok(())
}

/// `parseIndexList` with `terrainTypeNames`. The index is always one of the
/// names', so the class always exists.
fn parse_class(ini: &mut Ini<'_>, t: &mut TerrainType, ud: &UserData) -> IniResult<()> {
    let index = ini.parse_index_list(ud.names())?;
    t.class =
        TerrainClass::from_index(index).ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
    Ok(())
}

fn parse_restrict_construction(
    ini: &mut Ini<'_>,
    t: &mut TerrainType,
    _ud: &UserData,
) -> IniResult<()> {
    t.restrict_construction = ini.parse_bool()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniLoadType};

    fn block(ini: &mut Ini<'_>, c: &mut TerrainTypeCollection) -> IniResult<()> {
        c.parse_terrain_definition(ini)
    }

    static BLOCKS: [BlockParse<TerrainTypeCollection>; 1] = [BlockParse {
        token: "Terrain",
        parse: block,
    }];

    fn load(text: &[u8]) -> (TerrainTypeCollection, IniResult<()>) {
        let mut c = TerrainTypeCollection::new();
        let r = Ini::load(
            b"t.ini",
            text,
            IniLoadType::Overwrite,
            None,
            &BLOCKS,
            &mut c,
        );
        (c, r)
    }

    #[test]
    fn class_names_and_enum_agree() {
        assert_eq!(ALL_CLASSES.len(), TERRAIN_NUM_CLASSES as usize);
        assert_eq!(TERRAIN_TYPE_NAMES.len(), TERRAIN_NUM_CLASSES as usize);
        for (i, c) in ALL_CLASSES.iter().enumerate() {
            assert_eq!(*c as i32, i as i32);
            assert_eq!(TerrainClass::from_index(i as i32), Some(*c));
        }
        assert_eq!(TerrainClass::from_index(TERRAIN_NUM_CLASSES), None);
        assert_eq!(TerrainClass::from_index(-1), None);
        assert_eq!(TERRAIN_TYPE_NAMES[20], "BLEND_EDGE");
        assert_eq!(TerrainClass::BlendEdges as i32, 20);
    }

    #[test]
    fn default_entry_is_copied_and_names_are_case_sensitive() {
        let (c, r) = load(
            b"Terrain DefaultTerrain\n Texture = default.tga\n BlendEdges = Yes\n Class = GRASS\n RestrictConstruction = Yes\nEnd\n\
              Terrain Dirt\n Texture = dirt.tga\nEnd\nTerrain dirt\nEnd\n",
        );
        r.unwrap();
        assert_eq!(c.len(), 3);
        let dirt = c.find_terrain(b"Dirt").unwrap();
        assert_eq!(dirt.texture(), b"dirt.tga");
        assert!(dirt.is_blend_edge());
        assert_eq!(dirt.class(), TerrainClass::Grass);
        assert!(dirt.restrict_construction());
        // a lower-case name is a new terrain, not a second Dirt
        let lower = c.find_terrain(b"dirt").unwrap();
        assert_eq!(lower.texture(), b"default.tga");
        assert!(!c.is_empty());
        // newest first, as upstream's list
        let names: Vec<&[u8]> = c.terrains().map(|t| t.name()).collect();
        assert_eq!(
            names,
            vec![&b"dirt"[..], &b"Dirt"[..], &b"DefaultTerrain"[..]]
        );
    }

    #[test]
    fn redefinition_keeps_fields_not_written() {
        let (c, r) = load(b"Terrain A\n Class = SNOW_2\n Texture = a.tga\nEnd\nTerrain A\n BlendEdges = No\nEnd\n");
        r.unwrap();
        let a = c.find_terrain(b"A").unwrap();
        assert_eq!(a.class(), TerrainClass::Snow2);
        assert_eq!(a.texture(), b"a.tga");
        assert!(!a.is_blend_edge());
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn subsystem_interface_and_lengths() {
        let mut c = TerrainTypeCollection::new();
        assert!(c.is_empty());
        c.set_name(AsciiString::default());
        assert!(c.name().is_empty());
        c.init();
        c.update();
        c.reset();
        c.new_terrain(b"Only");
        assert_eq!(c.len(), 1);
        assert!(!c.is_empty());
        assert_eq!(
            c.find_terrain(b"Only").map(|t| t.name()),
            Some(&b"Only"[..])
        );
    }

    #[test]
    fn errors_are_reported() {
        let (_, r) = load(b"Terrain A\n Class = NOPE\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        let (_, r) = load(b"Terrain A\n Foo = 1\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::UnknownToken);
        let (_, r) = load(b"Terrain A\n BlendEdges = maybe\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        // the name is taken before the fields: a block without fields is fine
        let (c, r) = load(b"Terrain Z\nEnd\n");
        r.unwrap();
        assert_eq!(c.find_terrain(b"Z").unwrap().class(), TerrainClass::None);
    }
}
