// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/Water.h (WaterSetting, WaterTransparencySetting)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Water.cpp (m_waterSettingFieldParseTable,
//     m_waterTransparencySettingFieldParseTable)
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INIWater.cpp (INI::parseWaterSettingDefinition,
//     INI::parseWaterTransparencyDefinition)
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
//! **Where this differs from upstream:** a map's own `WaterTransparency` override (`INI_LOAD_CREATE_OVERRIDES`, from its
//! `map.ini`) is not read, and the skybox replacement it triggers is the renderer's.

use crate::common::base_type::{RGBAColorInt, RGBColor};
use crate::common::game_type::{TIME_OF_DAY_COUNT, TIME_OF_DAY_NAMES};
use crate::common::ini::{BlockParse, FieldParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, UserData};

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
    FieldParse::new("WaterRepeatCount", |i, s, _| i.parse_int().map(|v| s.water_repeat_count = v), UserData::None),
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

/// `WaterSettings[TIME_OF_DAY_COUNT]` and `TheWaterTransparency`.
#[derive(Clone, Debug, PartialEq)]
pub struct WaterSettings {
    pub settings: Vec<WaterSetting>,
    /// `None` until a `WaterTransparency` block is read (upstream's `TheWaterTransparency` is `NULL` until then).
    pub transparency: Option<WaterTransparencySetting>,
}

impl Default for WaterSettings {
    fn default() -> Self {
        WaterSettings { settings: vec![WaterSetting::default(); TIME_OF_DAY_COUNT as usize], transparency: None }
    }
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

/// `INI::parseWaterTransparencyDefinition`: the first block makes the setting; a second outside an override load is
/// `INI_INVALID_DATA` (an override load is not translated: see the module docs).
fn parse_water_transparency_definition(ini: &mut Ini<'_>, w: &mut WaterSettings) -> IniResult<()> {
    if w.transparency.is_some() {
        return Err(IniError::new(IniErrorKind::InvalidData));
    }
    let mut t = WaterTransparencySetting::default();
    ini.init_from_ini(&mut t, &WATER_TRANSPARENCY_FIELDS)?;
    w.transparency = Some(t);
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
        Ini::load(path, text, IniLoadType::Overwrite, None, WATER_BLOCKS, self)
    }

    /// `TheWaterTransparency->m_radarColor`, the default setting's while none was read.
    pub fn radar_color(&self) -> RGBColor {
        self.transparency.as_ref().map_or_else(|| WaterTransparencySetting::default().radar_color, |t| t.radar_color)
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
}
