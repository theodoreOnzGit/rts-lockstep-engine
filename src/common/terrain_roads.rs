// SPDX-License-Identifier: GPL-3.0-or-later
//! `TerrainRoads.h` / `TerrainRoads.cpp` (`GameClient/Terrain`), `INITerrainRoad.cpp` and `INITerrainBridge.cpp`: the road and
//! bridge types the `Road` and `Bridge` blocks of `Roads.ini` define (`TheTerrainRoads`).
//!
//! Upstream keeps the collection in the client's tree, but the logic reads it: `BridgeBehavior` takes a bridge's tower objects,
//! scaffold objects and damage/repair transition effects from it, and `TerrainLogic` asks whether a map's bridge names one.
//! The fields a client draws with (models, textures, the radar colour) are kept too, so the type is whole.
//!
//! **Upstream's behaviour kept as it is:**
//! - A second `Road` block with a road's name, or a second `Bridge` block with a bridge's name, is `INI_INVALID_DATA`, whatever
//!   the load type: the parse functions throw whenever the find succeeds (the assertion beside it only names the case it meant).
//! - `newRoad` and `newBridge` link the new type at the head of the list, so a find meets the newest first.
//! - `newBridge` copies from `DefaultBridge` the texture, scale, models, damaged textures, transition height and effect count.
//!   Its loops over the sounds and the transition effects copy the new bridge's own (empty) strings onto itself, so those are
//!   not inherited; nor are the tower and scaffold names.
//! - `RepairedToSound` writes the `BODY_DAMAGED` slot, as `DamagedToSound` does.
//!
//! `p1w13-sonnet`, integration batch I50, 2026-11-03.

use crate::common::base_type::RGBColor;
use crate::common::global_data::BODY_DAMAGE_TYPE_NAMES;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, UserData};

/// `MAX_BRIDGE_BODY_FX`.
pub const MAX_BRIDGE_BODY_FX: usize = 3;
/// `BODYDAMAGETYPE_COUNT`.
const BODY_DAMAGE_TYPE_COUNT: usize = 4;
/// `BODY_DAMAGED`.
const BODY_DAMAGED: usize = 1;

/// `class TerrainRoadType`: one road or bridge type.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainRoadType {
    /// `m_name`.
    pub name: Vec<u8>,
    /// `m_isBridge`.
    pub is_bridge: bool,
    /// `m_id`: unique over roads and bridges, from 1.
    pub id: u32,
    /// `m_texture`.
    pub texture: Vec<u8>,
    /// `m_roadWidth`.
    pub road_width: f32,
    /// `m_roadWidthInTexture`.
    pub road_width_in_texture: f32,
    /// `m_bridgeScale`.
    pub bridge_scale: f32,
    /// `m_scaffoldObjectName`.
    pub scaffold_object_name: Vec<u8>,
    /// `m_scaffoldSupportObjectName`.
    pub scaffold_support_object_name: Vec<u8>,
    /// `m_radarColor`.
    pub radar_color: RGBColor,
    /// `m_bridgeModelName`.
    pub bridge_model_name: Vec<u8>,
    /// `m_bridgeModelNameDamaged`.
    pub bridge_model_name_damaged: Vec<u8>,
    /// `m_bridgeModelNameReallyDamaged`.
    pub bridge_model_name_really_damaged: Vec<u8>,
    /// `m_bridgeModelNameBroken`.
    pub bridge_model_name_broken: Vec<u8>,
    /// `m_textureDamaged`.
    pub texture_damaged: Vec<u8>,
    /// `m_textureReallyDamaged`.
    pub texture_really_damaged: Vec<u8>,
    /// `m_textureBroken`.
    pub texture_broken: Vec<u8>,
    /// `m_towerObjectName[BRIDGE_MAX_TOWERS]`, by `BridgeTowerType`.
    pub tower_object_name: [Vec<u8>; 4],
    /// `m_damageToOCLString[BODYDAMAGETYPE_COUNT][MAX_BRIDGE_BODY_FX]`.
    pub damage_to_ocl_string: [[Vec<u8>; MAX_BRIDGE_BODY_FX]; BODY_DAMAGE_TYPE_COUNT],
    /// `m_damageToFXString`.
    pub damage_to_fx_string: [[Vec<u8>; MAX_BRIDGE_BODY_FX]; BODY_DAMAGE_TYPE_COUNT],
    /// `m_repairedToOCLString`.
    pub repaired_to_ocl_string: [[Vec<u8>; MAX_BRIDGE_BODY_FX]; BODY_DAMAGE_TYPE_COUNT],
    /// `m_repairedToFXString`.
    pub repaired_to_fx_string: [[Vec<u8>; MAX_BRIDGE_BODY_FX]; BODY_DAMAGE_TYPE_COUNT],
    /// `m_damageToSoundString[BODYDAMAGETYPE_COUNT]`.
    pub damage_to_sound_string: [Vec<u8>; BODY_DAMAGE_TYPE_COUNT],
    /// `m_repairedToSoundString`.
    pub repaired_to_sound_string: [Vec<u8>; BODY_DAMAGE_TYPE_COUNT],
    /// `m_transitionEffectsHeight`.
    pub transition_effects_height: f32,
    /// `m_numFXPerType`.
    pub num_fx_per_type: i32,
}

impl Default for TerrainRoadType {
    /// `TerrainRoadType::TerrainRoadType`.
    fn default() -> Self {
        TerrainRoadType {
            name: Vec::new(),
            is_bridge: false,
            id: 0,
            texture: Vec::new(),
            road_width: 0.0,
            road_width_in_texture: 0.0,
            bridge_scale: 1.0,
            scaffold_object_name: Vec::new(),
            scaffold_support_object_name: Vec::new(),
            radar_color: RGBColor { red: 0.0, green: 0.0, blue: 0.0 },
            bridge_model_name: Vec::new(),
            bridge_model_name_damaged: Vec::new(),
            bridge_model_name_really_damaged: Vec::new(),
            bridge_model_name_broken: Vec::new(),
            texture_damaged: Vec::new(),
            texture_really_damaged: Vec::new(),
            texture_broken: Vec::new(),
            tower_object_name: Default::default(),
            damage_to_ocl_string: Default::default(),
            damage_to_fx_string: Default::default(),
            repaired_to_ocl_string: Default::default(),
            repaired_to_fx_string: Default::default(),
            damage_to_sound_string: Default::default(),
            repaired_to_sound_string: Default::default(),
            transition_effects_height: 0.0,
            num_fx_per_type: 0,
        }
    }
}

fn invalid() -> IniError {
    IniError::new(IniErrorKind::InvalidData)
}

/// `parseTransitionToOCL` and `parseTransitionToFX`: `Transition:<Damage|Repair> ToState:<BODYTYPE> EffectNum:<INT> <what>:<NAME>`.
fn parse_transition(ini: &mut Ini<'_>, r: &mut TerrainRoadType, what: &[u8], fx: bool) -> IniResult<()> {
    let token = ini.get_next_sub_token(b"Transition")?;
    let damage = if token.eq_ignore_ascii_case(b"Damage") {
        true
    } else if token.eq_ignore_ascii_case(b"Repair") {
        false
    } else {
        return Err(invalid());
    };
    let token = ini.get_next_sub_token(b"ToState")?;
    let state = Ini::scan_index_list(&token, &BODY_DAMAGE_TYPE_NAMES)?;
    let token = ini.get_next_sub_token(b"EffectNum")?;
    // (made zero based)
    let effect = i64::from(Ini::scan_int(&token)?) - 1;
    if effect < 0 || effect >= MAX_BRIDGE_BODY_FX as i64 {
        return Err(invalid());
    }
    let name = ini.get_next_sub_token(what)?;
    let (s, e) = (state as usize, effect as usize);
    let slot = match (damage, fx) {
        (true, false) => &mut r.damage_to_ocl_string[s][e],
        (true, true) => &mut r.damage_to_fx_string[s][e],
        (false, false) => &mut r.repaired_to_ocl_string[s][e],
        (false, true) => &mut r.repaired_to_fx_string[s][e],
    };
    *slot = name;
    Ok(())
}

/// `TerrainRoadType::m_terrainRoadFieldParseTable`.
static ROAD_FIELDS: [FieldParse<TerrainRoadType>; 4] = [
    FieldParse::new("Texture", |ini, r, _| { r.texture = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("RoadWidth", |ini, r, _| { r.road_width = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("RoadWidthInTexture", |ini, r, _| { r.road_width_in_texture = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `TerrainRoadType::m_terrainBridgeFieldParseTable`.
static BRIDGE_FIELDS: [FieldParse<TerrainRoadType>; 23] = [
    FieldParse::new("BridgeScale", |ini, r, _| { r.bridge_scale = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("ScaffoldObjectName", |ini, r, _| { r.scaffold_object_name = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("ScaffoldSupportObjectName", |ini, r, _| { r.scaffold_support_object_name = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("RadarColor", |ini, r, _| { r.radar_color = ini.parse_rgb_color()?; Ok(()) }, UserData::None),
    FieldParse::new("TransitionEffectsHeight", |ini, r, _| { r.transition_effects_height = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("NumFXPerType", |ini, r, _| { r.num_fx_per_type = ini.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("BridgeModelName", |ini, r, _| { r.bridge_model_name = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("Texture", |ini, r, _| { r.texture = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("BridgeModelNameDamaged", |ini, r, _| { r.bridge_model_name_damaged = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("TextureDamaged", |ini, r, _| { r.texture_damaged = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("BridgeModelNameReallyDamaged", |ini, r, _| { r.bridge_model_name_really_damaged = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("TextureReallyDamaged", |ini, r, _| { r.texture_really_damaged = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("BridgeModelNameBroken", |ini, r, _| { r.bridge_model_name_broken = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("TextureBroken", |ini, r, _| { r.texture_broken = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("TowerObjectNameFromLeft", |ini, r, _| { r.tower_object_name[0] = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("TowerObjectNameFromRight", |ini, r, _| { r.tower_object_name[1] = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("TowerObjectNameToLeft", |ini, r, _| { r.tower_object_name[2] = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("TowerObjectNameToRight", |ini, r, _| { r.tower_object_name[3] = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("DamagedToSound", |ini, r, _| { r.damage_to_sound_string[BODY_DAMAGED] = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    // (upstream's slot: `m_repairedToSoundString[BODY_DAMAGED]`)
    FieldParse::new("RepairedToSound", |ini, r, _| { r.repaired_to_sound_string[BODY_DAMAGED] = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("TransitionToOCL", |ini, r, _| parse_transition(ini, r, b"OCL", false), UserData::None),
    FieldParse::new("TransitionToFX", |ini, r, _| parse_transition(ini, r, b"FX", true), UserData::None),
    FieldParse::end(),
];

/// `class TerrainRoadCollection` (`TheTerrainRoads`). Each list is kept oldest first; upstream's head is the last element.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainRoadCollection {
    /// `m_roadList`.
    pub roads: Vec<TerrainRoadType>,
    /// `m_bridgeList`.
    pub bridges: Vec<TerrainRoadType>,
    /// `m_idCounter`: "MUST start this at 1".
    pub id_counter: u32,
}

impl Default for TerrainRoadCollection {
    fn default() -> Self {
        TerrainRoadCollection { roads: Vec::new(), bridges: Vec::new(), id_counter: 1 }
    }
}

impl TerrainRoadCollection {
    /// `TerrainRoadCollection::TerrainRoadCollection`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `findRoad(name)`: the newest road of that name.
    pub fn find_road(&self, name: &[u8]) -> Option<&TerrainRoadType> {
        self.roads.iter().rev().find(|r| r.name == name)
    }

    /// `findBridge(name)`: the newest bridge of that name.
    pub fn find_bridge(&self, name: &[u8]) -> Option<&TerrainRoadType> {
        self.bridges.iter().rev().find(|r| r.name == name)
    }

    /// `findRoadOrBridge(name)`.
    pub fn find_road_or_bridge(&self, name: &[u8]) -> Option<&TerrainRoadType> {
        self.find_road(name).or_else(|| self.find_bridge(name))
    }

    /// `newRoad(name)`: named, numbered, with `DefaultRoad`'s texture and widths; its index in `roads`.
    pub fn new_road(&mut self, name: &[u8]) -> usize {
        let mut road = TerrainRoadType { name: name.to_vec(), id: self.id_counter, is_bridge: false, ..Default::default() };
        self.id_counter += 1;
        if let Some(d) = self.find_road(b"DefaultRoad") {
            road.texture = d.texture.clone();
            road.road_width = d.road_width;
            road.road_width_in_texture = d.road_width_in_texture;
        }
        self.roads.push(road);
        self.roads.len() - 1
    }

    /// `newBridge(name)`: named, numbered, with what upstream copies from `DefaultBridge` (see the module's documentation);
    /// its index in `bridges`.
    pub fn new_bridge(&mut self, name: &[u8]) -> usize {
        let mut bridge = TerrainRoadType { name: name.to_vec(), id: self.id_counter, is_bridge: true, ..Default::default() };
        self.id_counter += 1;
        if let Some(d) = self.find_bridge(b"DefaultBridge") {
            bridge.texture = d.texture.clone();
            bridge.bridge_scale = d.bridge_scale;
            bridge.bridge_model_name = d.bridge_model_name.clone();
            bridge.bridge_model_name_damaged = d.bridge_model_name_damaged.clone();
            bridge.bridge_model_name_really_damaged = d.bridge_model_name_really_damaged.clone();
            bridge.bridge_model_name_broken = d.bridge_model_name_broken.clone();
            bridge.texture_damaged = d.texture_damaged.clone();
            bridge.texture_really_damaged = d.texture_really_damaged.clone();
            bridge.texture_broken = d.texture_broken.clone();
            bridge.transition_effects_height = d.transition_effects_height;
            bridge.num_fx_per_type = d.num_fx_per_type;
            // (the sound and transition loops copy the new bridge's own strings onto themselves: nothing more is inherited)
        }
        self.bridges.push(bridge);
        self.bridges.len() - 1
    }

    /// `INI::parseTerrainRoadDefinition`.
    pub fn parse_terrain_road_definition(&mut self, ini: &mut Ini<'_>) -> IniResult<()> {
        let name = ini.get_next_token(None)?;
        if self.find_road(&name).is_some() {
            return Err(invalid());
        }
        let i = self.new_road(&name);
        ini.init_from_ini(&mut self.roads[i], &ROAD_FIELDS)
    }

    /// `INI::parseTerrainBridgeDefinition`.
    pub fn parse_terrain_bridge_definition(&mut self, ini: &mut Ini<'_>) -> IniResult<()> {
        let name = ini.get_next_token(None)?;
        if self.find_bridge(&name).is_some() {
            return Err(invalid());
        }
        let i = self.new_bridge(&name);
        ini.init_from_ini(&mut self.bridges[i], &BRIDGE_FIELDS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniLoadType};

    fn road(ini: &mut Ini<'_>, c: &mut TerrainRoadCollection) -> IniResult<()> {
        c.parse_terrain_road_definition(ini)
    }
    fn bridge(ini: &mut Ini<'_>, c: &mut TerrainRoadCollection) -> IniResult<()> {
        c.parse_terrain_bridge_definition(ini)
    }
    static BLOCKS: [BlockParse<TerrainRoadCollection>; 2] = [BlockParse { token: "Bridge", parse: bridge }, BlockParse { token: "Road", parse: road }];

    fn load(text: &[u8]) -> (TerrainRoadCollection, IniResult<()>) {
        let mut c = TerrainRoadCollection::new();
        let r = Ini::load(b"Roads.ini", text, IniLoadType::Overwrite, None, &BLOCKS, &mut c);
        (c, r)
    }

    /// Predicted before the run (batch I50): `DefaultBridge`'s scale, models, textures, height and count reach a later bridge;
    /// its towers, scaffolds, sounds and transitions do not; the four towers, the transitions (1-based effect numbers, by damage
    /// state) and both sounds (in the `DAMAGED` slot) are read; ids run 1, 2, 3 over roads and bridges together; `DefaultRoad`'s
    /// texture and widths reach a later road.
    #[test]
    fn bridges_and_roads_are_read_as_upstream_reads_them() {
        let text = b"Road DefaultRoad\n Texture = Road.tga\n RoadWidth = 30.0\n RoadWidthInTexture = 0.5\nEnd\n\
Bridge DefaultBridge\n BridgeScale = 2.0\n BridgeModelName = Bridge01\n Texture = B.tga\n TextureBroken = BB.tga\n TransitionEffectsHeight = 7.5\n NumFXPerType = 3\n TowerObjectNameFromLeft = DefTower\n ScaffoldObjectName = DefScaffold\n DamagedToSound = DefSound\nEnd\n\
Bridge Wood\n TowerObjectNameFromLeft = TowerA\n TowerObjectNameFromRight = TowerB\n TowerObjectNameToLeft = TowerC\n TowerObjectNameToRight = TowerD\n\
 ScaffoldObjectName = Scaffold\n ScaffoldSupportObjectName = Support\n DamagedToSound = Crack\n RepairedToSound = Fixed\n\
 TransitionToOCL = Transition:Damage ToState:RUBBLE EffectNum:3 OCL:OCL_Fall\n TransitionToFX = Transition:Repair ToState:DAMAGED EffectNum:1 FX:FX_Fix\nEnd\n\
Road Dirt\nEnd\n";
        let (c, r) = load(text);
        r.unwrap();
        let d = c.find_road(b"Dirt").unwrap();
        assert_eq!((d.id, d.is_bridge, d.texture.as_slice(), d.road_width, d.road_width_in_texture), (4, false, &b"Road.tga"[..], 30.0, 0.5));
        let w = c.find_bridge(b"Wood").unwrap();
        assert_eq!((w.id, w.is_bridge), (3, true));
        assert_eq!((w.bridge_scale, w.bridge_model_name.as_slice(), w.texture.as_slice(), w.texture_broken.as_slice()), (2.0, &b"Bridge01"[..], &b"B.tga"[..], &b"BB.tga"[..]));
        assert_eq!((w.transition_effects_height, w.num_fx_per_type), (7.5, 3));
        assert_eq!(w.tower_object_name, [b"TowerA".to_vec(), b"TowerB".to_vec(), b"TowerC".to_vec(), b"TowerD".to_vec()]);
        assert_eq!((w.scaffold_object_name.as_slice(), w.scaffold_support_object_name.as_slice()), (&b"Scaffold"[..], &b"Support"[..]));
        assert_eq!((w.damage_to_sound_string[1].as_slice(), w.repaired_to_sound_string[1].as_slice()), (&b"Crack"[..], &b"Fixed"[..]));
        assert!(w.repaired_to_sound_string[0].is_empty());
        assert_eq!(w.damage_to_ocl_string[3][2], b"OCL_Fall".to_vec());
        assert_eq!(w.repaired_to_fx_string[1][0], b"FX_Fix".to_vec());
        // what `newBridge` does not inherit
        c.find_bridge(b"DefaultBridge").map(|d| assert_eq!(d.tower_object_name[0], b"DefTower".to_vec())).unwrap();
        let (c2, r2) = load(b"Bridge DefaultBridge\n TowerObjectNameFromLeft = DefTower\n ScaffoldObjectName = S\n DamagedToSound = D\nEnd\nBridge Bare\nEnd\n");
        r2.unwrap();
        let b = c2.find_bridge(b"Bare").unwrap();
        assert!(b.tower_object_name[0].is_empty() && b.scaffold_object_name.is_empty() && b.damage_to_sound_string[1].is_empty());
        assert_eq!(c.find_road_or_bridge(b"Wood").map(|r| r.id), Some(3));
    }

    /// Predicted before the run (batch I50): a second block of a road's or a bridge's name is invalid data; a bridge may share a
    /// road's name; an effect number outside 1..=3 and a transition other than Damage or Repair are invalid data.
    #[test]
    fn redefinitions_and_bad_transitions_are_invalid() {
        let bad = |t: &[u8]| load(t).1.unwrap_err().kind;
        assert_eq!(bad(b"Road A\nEnd\nRoad A\nEnd\n"), IniErrorKind::InvalidData);
        assert_eq!(bad(b"Bridge A\nEnd\nBridge A\nEnd\n"), IniErrorKind::InvalidData);
        load(b"Road A\nEnd\nBridge A\nEnd\n").1.unwrap();
        assert_eq!(bad(b"Bridge A\n TransitionToOCL = Transition:Damage ToState:DAMAGED EffectNum:4 OCL:X\nEnd\n"), IniErrorKind::InvalidData);
        assert_eq!(bad(b"Bridge A\n TransitionToOCL = Transition:Damage ToState:DAMAGED EffectNum:0 OCL:X\nEnd\n"), IniErrorKind::InvalidData);
        assert_eq!(bad(b"Bridge A\n TransitionToFX = Transition:Break ToState:DAMAGED EffectNum:1 FX:X\nEnd\n"), IniErrorKind::InvalidData);
    }
}
