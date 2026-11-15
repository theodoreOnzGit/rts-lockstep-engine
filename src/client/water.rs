// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/Water.h (WaterSetting, WaterTransparencySetting)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Water.cpp (m_waterSettingFieldParseTable,
//     m_waterTransparencySettingFieldParseTable)
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INIWater.cpp (INI::parseWaterSettingDefinition,
//     INI::parseWaterTransparencyDefinition)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp (GameLogic::reset's water transparency lines)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The water's settings (GitHub #35, for the radar's water colour): `Data\INI\Default\Water.ini` and `Data\INI\Water.ini`
//! (`GameEngine::init`), their `WaterSet <time of day>` blocks (`WaterSettings[]`) and their `WaterTransparency` block
//! (`TheWaterTransparency`). Only `m_radarColor` is read by anything translated yet (`W3DRadar::buildTerrainTexture`);
//! the rest is kept as parsed.
//!
//! ~~**Where this differs from upstream:** a map's own `WaterTransparency` override (`INI_LOAD_CREATE_OVERRIDES`, from its
//! `map.ini`) is not read, and the skybox replacement it triggers is the renderer's.~~ **Corrected 2026-11-14 (GitHub
//! #57):** the override load is translated ([`WaterSettings::load_text_as`] with `INI_LOAD_CREATE_OVERRIDES`): an override
//! hangs off the final one, the setting reads the final override ([`WaterSettings::radar_color`]), and
//! [`WaterSettings::delete_overrides`] is `GameLogic::reset`'s clean-up. The skybox replacement it asks for
//! (`TheTerrainVisual->replaceSkyboxTextures`) is recorded in [`WaterSettings::skybox_replacements`] for the renderer,
//! which draws no skybox yet. Compared with upstream in `tests/differential_water.rs`.
//!
//! **Where this differs from upstream:** nothing reads a map's `map.ini` into these settings yet. A game's `map.ini` is
//! read by the logic (`World::load_ini_overrides`, `loadMapINI`), whose block table has no `WaterTransparency` row, so a
//! map whose `map.ini` holds one does not load in the port at all (`INI_UNKNOWN_TOKEN`; GitHub #57's gaps). Wiring it
//! needs the logic to pass such blocks on, which is a logic change.
//!
//! **Upstream's override copies the original, not the final override** (`*wtOverride = *wt`, `wt` the non-overloaded
//! pointer), and the copy takes `Overridable`'s members with it, `m_nextOverride` included. The first override of a map
//! is sound. A second before `GameLogic::reset` (a `map.ini` and a `solo.ini` that both hold the block, or two blocks)
//! copies the original's link to the first override, so the chain becomes a cycle (original, first, second, first, ...)
//! and `friend_getFinalOverride` recurses for ever. The port reports that as undefined ([`Undefined::EndlessLoop`]).

use crate::common::base_type::{RGBAColorInt, RGBColor};
use crate::common::game_type::{TIME_OF_DAY_COUNT, TIME_OF_DAY_NAMES};
use crate::common::ini::{BlockParse, FieldParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Undefined, UserData};
use crate::common::overridable::Overridable;
use crate::logic::modules::plant;

/// The files `GameEngine::init` loads, in order.
pub const WATER_INI_FILES: [&[u8]; 2] = [b"Data\\INI\\Default\\Water.ini", b"Data\\INI\\Water.ini"];

/// `class WaterSetting`: one time of day's water.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaterSetting {
    pub sky_texture_file: Vec<u8>,
    pub water_texture_file: Vec<u8>,
    pub vertex00_diffuse: RGBAColorInt,
    pub vertex10_diffuse: RGBAColorInt,
    pub vertex01_diffuse: RGBAColorInt,
    pub vertex11_diffuse: RGBAColorInt,
    pub water_diffuse_color: RGBAColorInt,
    pub transparent_water_diffuse: RGBAColorInt,
    pub u_scroll_per_ms: f32,
    pub v_scroll_per_ms: f32,
    pub sky_texels_per_unit: f32,
    pub water_repeat_count: i32,
}

/// `WaterSetting::m_waterSettingFieldParseTable`.
pub static WATER_SETTING_FIELDS: [FieldParse<WaterSetting>; 13] = [
    FieldParse::new("SkyTexture", |i, s, _| i.parse_ascii_string().map(|v| s.sky_texture_file = v), UserData::None),
    FieldParse::new("WaterTexture", |i, s, _| i.parse_ascii_string().map(|v| s.water_texture_file = v), UserData::None),
    FieldParse::new("Vertex00Color", |i, s, _| i.parse_rgba_color_int().map(|v| s.vertex00_diffuse = v), UserData::None),
    FieldParse::new("Vertex10Color", |i, s, _| i.parse_rgba_color_int().map(|v| s.vertex10_diffuse = v), UserData::None),
    FieldParse::new("Vertex01Color", |i, s, _| i.parse_rgba_color_int().map(|v| s.vertex01_diffuse = v), UserData::None),
    FieldParse::new("Vertex11Color", |i, s, _| i.parse_rgba_color_int().map(|v| s.vertex11_diffuse = v), UserData::None),
    FieldParse::new("DiffuseColor", |i, s, _| i.parse_rgba_color_int().map(|v| s.water_diffuse_color = v), UserData::None),
    FieldParse::new("TransparentDiffuseColor", |i, s, _| i.parse_rgba_color_int().map(|v| s.transparent_water_diffuse = v), UserData::None),
    FieldParse::new("UScrollPerMS", |i, s, _| i.parse_real().map(|v| s.u_scroll_per_ms = v), UserData::None),
    FieldParse::new("VScrollPerMS", |i, s, _| i.parse_real().map(|v| s.v_scroll_per_ms = v), UserData::None),
    FieldParse::new("SkyTexelsPerUnit", |i, s, _| i.parse_real().map(|v| s.sky_texels_per_unit = v), UserData::None),
    FieldParse::new("WaterRepeatCount", |i, s, _| i.parse_int().map(|v| if !plant(10307) { s.water_repeat_count = v }), UserData::None),
    FieldParse::end(),
];

/// `class WaterTransparencySetting`.
#[derive(Clone, Debug, PartialEq)]
pub struct WaterTransparencySetting {
    pub transparent_water_depth: f32,
    pub min_water_opacity: f32,
    pub standing_water_color: RGBColor,
    /// `m_radarColor`, the radar's water.
    pub radar_color: RGBColor,
    pub additive_blend: bool,
    pub standing_water_texture: Vec<u8>,
    pub skybox_texture_n: Vec<u8>,
    pub skybox_texture_e: Vec<u8>,
    pub skybox_texture_s: Vec<u8>,
    pub skybox_texture_w: Vec<u8>,
    pub skybox_texture_t: Vec<u8>,
}

impl Default for WaterTransparencySetting {
    /// `WaterTransparencySetting()`. Its radar colour is `140, 140, 255` as `Real`s (upstream's: not divided by 255 as
    /// `parseRGBColor` divides, so a game whose INI does not set `RadarWaterColor` draws its water clamped near white).
    fn default() -> Self {
        WaterTransparencySetting {
            transparent_water_depth: 3.0,
            min_water_opacity: 1.0,
            standing_water_color: RGBColor { red: 1.0, green: 1.0, blue: 1.0 },
            radar_color: RGBColor { red: 140.0, green: 140.0, blue: 255.0 },
            standing_water_texture: b"TWWater01.tga".to_vec(),
            additive_blend: false,
            skybox_texture_n: b"TSMorningN.tga".to_vec(),
            skybox_texture_e: b"TSMorningE.tga".to_vec(),
            skybox_texture_s: b"TSMorningS.tga".to_vec(),
            skybox_texture_w: b"TSMorningW.tga".to_vec(),
            skybox_texture_t: b"TSMorningT.tga".to_vec(),
        }
    }
}

/// `WaterTransparencySetting::m_waterTransparencySettingFieldParseTable`.
pub static WATER_TRANSPARENCY_FIELDS: [FieldParse<WaterTransparencySetting>; 12] = [
    FieldParse::new("TransparentWaterDepth", |i, s, _| i.parse_real().map(|v| s.transparent_water_depth = v), UserData::None),
    FieldParse::new("TransparentWaterMinOpacity", |i, s, _| i.parse_real().map(|v| s.min_water_opacity = v), UserData::None),
    FieldParse::new("StandingWaterColor", |i, s, _| i.parse_rgb_color().map(|v| s.standing_water_color = v), UserData::None),
    FieldParse::new("StandingWaterTexture", |i, s, _| i.parse_ascii_string().map(|v| s.standing_water_texture = v), UserData::None),
    FieldParse::new("AdditiveBlending", |i, s, _| i.parse_bool().map(|v| s.additive_blend = v), UserData::None),
    FieldParse::new("RadarWaterColor", |i, s, _| i.parse_rgb_color().map(|v| s.radar_color = v), UserData::None),
    FieldParse::new("SkyboxTextureN", |i, s, _| i.parse_ascii_string().map(|v| s.skybox_texture_n = v), UserData::None),
    FieldParse::new("SkyboxTextureE", |i, s, _| i.parse_ascii_string().map(|v| s.skybox_texture_e = v), UserData::None),
    FieldParse::new("SkyboxTextureS", |i, s, _| i.parse_ascii_string().map(|v| s.skybox_texture_s = v), UserData::None),
    FieldParse::new("SkyboxTextureW", |i, s, _| i.parse_ascii_string().map(|v| s.skybox_texture_w = v), UserData::None),
    FieldParse::new("SkyboxTextureT", |i, s, _| i.parse_ascii_string().map(|v| s.skybox_texture_t = v), UserData::None),
    FieldParse::end(),
];

/// `TerrainVisual::replaceSkyboxTextures(oldTexName, newTexName)`'s arguments: the N, E, S, W and T textures.
pub type SkyboxTextures = [Vec<u8>; 5];

/// `WaterSettings[TIME_OF_DAY_COUNT]` and `TheWaterTransparency`.
#[derive(Clone, Debug, PartialEq)]
pub struct WaterSettings {
    pub settings: Vec<WaterSetting>,
    /// `TheWaterTransparency`'s original (`getNonOverloadedPointer`) and its chain of overrides; `None` until a
    /// `WaterTransparency` block is read (upstream's is `NULL` until then).
    pub transparency: Option<Box<Overridable<WaterTransparencySetting>>>,
    /// Each `TheTerrainVisual->replaceSkyboxTextures(old, new)` an override load asked for, in order (the renderer's).
    pub skybox_replacements: Vec<(SkyboxTextures, SkyboxTextures)>,
}

impl Default for WaterSettings {
    fn default() -> Self {
        WaterSettings { settings: vec![WaterSetting::default(); TIME_OF_DAY_COUNT as usize], transparency: None, skybox_replacements: Vec::new() }
    }
}

/// The five skybox texture names of a setting, in `parseWaterTransparencyDefinition`'s order.
fn skybox_textures(t: &WaterTransparencySetting) -> SkyboxTextures {
    [t.skybox_texture_n.clone(), t.skybox_texture_e.clone(), t.skybox_texture_s.clone(), t.skybox_texture_w.clone(), t.skybox_texture_t.clone()]
}

/// `INI::parseWaterSettingDefinition`: the block of the named time of day (ignoring case); an unknown name is
/// `INI_INVALID_DATA`. (`TimeOfDayNames` begins with `NONE`, so `WaterSet NONE` names index 0, as upstream.)
fn parse_water_setting_definition(ini: &mut Ini<'_>, w: &mut WaterSettings) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    let Some(i) = TIME_OF_DAY_NAMES.iter().position(|n| n.as_bytes().eq_ignore_ascii_case(&name)) else {
        return Err(IniError::new(IniErrorKind::InvalidData));
    };
    ini.init_from_ini(&mut w.settings[i], &WATER_SETTING_FIELDS)
}

/// `INI::parseWaterTransparencyDefinition`: the first block makes the setting; a later one in an override load
/// (`INI_LOAD_CREATE_OVERRIDES`, a map's `map.ini`) makes an override, a copy of the original marked as one and hung off
/// the final override (see the module docs for the second override's cycle); a later one in any other load is
/// `INI_INVALID_DATA`. The block is read into the final override. After an override load that made one, the skybox's
/// textures are replaced: the original's names by the final override's.
fn parse_water_transparency_definition(ini: &mut Ini<'_>, w: &mut WaterSettings) -> IniResult<()> {
    let overrides = ini.get_load_type() == IniLoadType::CreateOverrides;
    match w.transparency.as_deref_mut() {
        None => w.transparency = Some(Box::new(Overridable::new(WaterTransparencySetting::default()))),
        Some(wt) if overrides => {
            // `*wtOverride = *wt`: the copy takes the original's `m_nextOverride`; with an override already there, the
            // chain becomes a cycle and the next `friend_getFinalOverride` never returns
            if wt.get_next_override().is_some() && !plant(10303) {
                return Err(IniError::undefined(Undefined::EndlessLoop));
            }
            // (the original is the final override here: with any override, the branch above has returned)
            let mut wt_override = Overridable::new(wt.data.clone());
            if !plant(10304) {
                wt_override.mark_as_override();
            }
            let _ = wt.friend_get_final_override().set_next_override(Some(Box::new(wt_override)));
        }
        Some(_) => return Err(IniError::new(IniErrorKind::InvalidData)),
    }
    let wt = w.transparency.as_deref_mut().expect("made above");
    let wt_original = skybox_textures(&wt.data);
    let has_override = wt.get_next_override().is_some();
    let water_trans = &mut wt.friend_get_final_override().data;
    ini.init_from_ini(water_trans, &WATER_TRANSPARENCY_FIELDS)?;
    // "If we overrode any skybox textures, then call the W3D Water stuff." (`wtOriginal == wtOverride` when the original is
    // the final one: then nothing)
    if overrides && (has_override || plant(10305)) {
        w.skybox_replacements.push((wt_original, skybox_textures(water_trans)));
    }
    Ok(())
}

/// The `WaterSet` and `WaterTransparency` rows of `theTypeTable`.
pub static WATER_BLOCKS: &[BlockParse<WaterSettings>] = &[
    BlockParse { token: "WaterSet", parse: parse_water_setting_definition },
    BlockParse { token: "WaterTransparency", parse: parse_water_transparency_definition },
];

impl WaterSettings {
    /// `ini.load(path, INI_LOAD_OVERWRITE, &xferCRC)` of one file's text.
    pub fn load_text(&mut self, path: &[u8], text: &[u8]) -> IniResult<()> {
        self.load_text_as(path, text, IniLoadType::Overwrite)
    }

    /// `INI ini; ini.load(path, loadType, NULL)` of one file's text: `INI_LOAD_CREATE_OVERRIDES` for a map's `map.ini`
    /// and `solo.ini` (`GameLogic::loadMapINI`).
    pub fn load_text_as(&mut self, path: &[u8], text: &[u8], load_type: IniLoadType) -> IniResult<()> {
        Ini::load(path, text, load_type, None, WATER_BLOCKS, self)
    }

    /// `GameLogic::reset`'s "Clean up any water transparency overrides that were generated for this map":
    /// `TheWaterTransparency = wt->deleteOverrides()`. Undefined upstream without a setting (`deleteOverrides` through
    /// `NULL`).
    pub fn delete_overrides(&mut self) -> IniResult<()> {
        let Some(wt) = self.transparency.take() else {
            return Err(IniError::undefined(Undefined::NullObject));
        };
        self.transparency = if plant(10306) { Some(wt) } else { wt.delete_overrides() };
        Ok(())
    }

    /// `TheWaterTransparency`: the final override's setting, `None` while none was read.
    pub fn transparency(&self) -> Option<&WaterTransparencySetting> {
        self.transparency.as_deref().map(|t| &t.get_final_override().data)
    }

    /// `TheWaterTransparency->m_radarColor` (the final override's), the default setting's while none was read.
    pub fn radar_color(&self) -> RGBColor {
        self.transparency().map_or_else(|| WaterTransparencySetting::default().radar_color, |t| t.radar_color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #35: the water INI. Predicted before the run: a `WaterSet AFTERNOON` block fills index 2; `RadarWaterColor = R:51
    /// G:102 B:204` reads as 0.2, 0.4, 0.8; a second `WaterTransparency` is invalid data; an unknown time of day too; with
    /// no file the radar colour is the constructor's (140, 140, 255).
    ///
    /// *Results (2026-11-06):* as predicted, first run.
    #[test]
    fn the_water_ini_gives_the_radar_colour() {
        let mut w = WaterSettings::default();
        assert_eq!(w.radar_color(), RGBColor { red: 140.0, green: 140.0, blue: 255.0 });
        let text = b"WaterSet AFTERNOON\n  SkyTexture = TSAfternoon.tga\n  WaterRepeatCount = 32\n  DiffuseColor = R:10 G:20 B:30 A:40\nEnd\n\
WaterTransparency\n  TransparentWaterDepth = 3.5\n  RadarWaterColor = R:51 G:102 B:204\n  AdditiveBlending = No\nEnd\n";
        w.load_text(b"Data\\INI\\Water.ini", text).expect("parses");
        assert_eq!(w.settings[2].water_repeat_count, 32);
        assert_eq!(w.settings[2].water_diffuse_color, RGBAColorInt { red: 10, green: 20, blue: 30, alpha: 40 });
        assert_eq!(w.radar_color(), RGBColor { red: 51.0 / 255.0, green: 102.0 / 255.0, blue: 204.0 / 255.0 });
        assert!(w.load_text(b"x", b"WaterTransparency\nEnd\n").is_err());
        assert!(WaterSettings::default().load_text(b"x", b"WaterSet NOON\nEnd\n").is_err());
    }

    /// GitHub #57: a map's override. Predicted before the run: after `Water.ini` (radar colour 0.2, 0.4, 0.8), a map's
    /// `WaterTransparency` read as an override changes the radar colour to the map's and keeps the original's other values
    /// it does not set; it asks once for the skybox to go from the original's names to the override's (`TSMorningN.tga`
    /// to `MapN.tga`, the other four unchanged); `GameLogic::reset`'s clean-up brings back the original; a second override
    /// before the reset is undefined (the cycle, see the module docs); a reset with no setting is undefined.
    ///
    /// *Results (2026-11-14):* as predicted, first run.
    #[test]
    fn a_maps_water_transparency_overrides_until_the_reset() {
        let mut w = WaterSettings::default();
        assert!(matches!(w.delete_overrides().unwrap_err().kind, IniErrorKind::Undefined(Undefined::NullObject)));
        w.load_text(b"Data\\INI\\Water.ini", b"WaterTransparency\n  TransparentWaterDepth = 3.5\n  RadarWaterColor = R:51 G:102 B:204\nEnd\n").expect("parses");
        assert!(w.skybox_replacements.is_empty());
        let map = b"WaterTransparency\n  RadarWaterColor = R:0 G:0 B:255\n  SkyboxTextureN = MapN.tga\nEnd\n";
        w.load_text_as(b"Maps\\M\\map.ini", map, IniLoadType::CreateOverrides).expect("the override");
        assert_eq!(w.radar_color(), RGBColor { red: 0.0, green: 0.0, blue: 1.0 });
        assert_eq!(w.transparency().map(|t| t.transparent_water_depth), Some(3.5));
        assert_eq!(w.skybox_replacements.len(), 1);
        let (old, new) = &w.skybox_replacements[0];
        assert_eq!((old[0].as_slice(), new[0].as_slice()), (&b"TSMorningN.tga"[..], &b"MapN.tga"[..]));
        assert_eq!(old[1..], new[1..]);
        let e = w.load_text_as(b"Maps\\M\\solo.ini", map, IniLoadType::CreateOverrides).unwrap_err();
        assert!(matches!(e.kind, IniErrorKind::Undefined(Undefined::EndlessLoop)), "{e:?}");
        w.delete_overrides().expect("the reset");
        assert_eq!(w.radar_color(), RGBColor { red: 51.0 / 255.0, green: 102.0 / 255.0, blue: 204.0 / 255.0 });
        assert!(w.transparency.as_ref().is_some_and(|t| t.get_next_override().is_none()));
        // after the reset, the next map's override is sound again
        w.load_text_as(b"Maps\\M\\map.ini", map, IniLoadType::CreateOverrides).expect("the next map's override");
        assert_eq!(w.radar_color(), RGBColor { red: 0.0, green: 0.0, blue: 1.0 });
    }

    /// GitHub #57, the gap left: the logic reads a map's `map.ini` (`World::load_ini_overrides`, `loadMapINI`'s load),
    /// and its block table has no `WaterTransparency` row. Predicted: a `map.ini` holding the block fails the logic's
    /// load with `INI_UNKNOWN_TOKEN`, so no map with one starts in the port; one without it loads. This test records the
    /// blocker; it is to change when the logic passes the client's blocks on.
    ///
    /// *Results (2026-11-14):* as predicted, first run.
    #[test]
    fn the_logic_does_not_yet_take_a_maps_water_block() {
        let mut world = crate::logic::world::game_logic::World::new();
        world.save_side.disk.insert(b"maps\\m\\map.ini".to_vec(), b"WaterTransparency\n  RadarWaterColor = R:0 G:0 B:255\nEnd\n".to_vec());
        let e = world.load_ini_overrides(b"Maps\\M\\map.ini").unwrap_err();
        assert_eq!(e.kind, IniErrorKind::UnknownToken);
        world.save_side.disk.insert(b"maps\\n\\map.ini".to_vec(), b"; nothing\n".to_vec());
        assert_eq!(world.load_ini_overrides(b"Maps\\N\\map.ini").map_err(|e| e.kind), Ok(true));
    }
}
