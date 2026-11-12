// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/GlobalLanguage.h
//   GeneralsMD/Code/GameEngine/Source/GameClient/GlobalLanguage.cpp
//   GeneralsMD/Code/GameEngine/Include/GameClient/FontDesc.h (FontDesc)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #148: `TheGlobalLanguageData`, the language's fonts and font scaling (`Data\<language>\Language.ini`).
//!
//! # As upstream does it
//!
//! - `GlobalLanguage::GlobalLanguage` (GlobalLanguage.cpp:110) sets the members ([`GlobalLanguage::default`]):
//!   `m_resolutionFontSizeAdjustment = 0.7f`, `m_militaryCaptionDelayMS = 750`, the rest empty, 0 or `FALSE`, and each
//!   `FontDesc` its own constructor's `"Arial Unicode MS"`, 12, not bold (GlobalLanguage.cpp:202).
//! - `GlobalLanguage::init` (GlobalLanguage.cpp:136) loads `Data\<GetRegistryLanguage()>\Language.ini` with
//!   `INI_LOAD_OVERWRITE` (on Windows 9x `Language9x.ini` when it exists: never here) and adds each `LocalFontFile` as a
//!   font resource. Its one block, `Language`, is `INI::parseLanguageDefinition` (GlobalLanguage.cpp:99): the field
//!   table `TheGlobalLanguageDataFieldParseTable` (GlobalLanguage.cpp:65) on `TheGlobalLanguageData`
//!   ([`GlobalLanguage::parse_ini`]).
//! - `parseFontDesc` (GlobalLanguage.cpp:177): a quoted name, then `scanInt` and `scanBool` of the next two tokens.
//!   `parseFontFileName` (GlobalLanguage.cpp:185): `getNextAsciiString`, put at the **front** of `m_localFonts`.
//! - `adjustFontSize` (GlobalLanguage.cpp:192, [`crate::client::cinematic_text::adjust_font_size`]) scales a point size
//!   by the display's width over 800, damped by `m_resolutionFontSizeAdjustment`.
//!
//! # Where this differs from upstream
//!
//! - `AddFontResource` / `RemoveFontResource` (the local font files given to Windows) are not made: the client draws in
//!   its own font. The file names are kept ([`GlobalLanguage::local_fonts`]).
//! - `init` reads the file through the caller (`open`), so the file system and the language are the caller's.

use crate::common::ini::{BlockParse, FieldParse, Ini, IniLoadType, IniResult, UserData};
use crate::logic::modules::plant;

/// `class FontDesc` (FontDesc.h): a font's name, point size and bold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontDesc {
    pub name: Vec<u8>,
    pub size: i32,
    pub bold: bool,
}

impl Default for FontDesc {
    /// `FontDesc::FontDesc` (GlobalLanguage.cpp:202).
    fn default() -> Self {
        FontDesc { name: b"Arial Unicode MS".to_vec(), size: 12, bold: false }
    }
}

/// `class GlobalLanguage` (GlobalLanguage.h:69): its members, upstream's names.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobalLanguage {
    pub unicode_font_name: Vec<u8>,
    pub unicode_font_file_name: Vec<u8>,
    pub use_hard_wrap: bool,
    pub military_caption_speed: i32,
    pub military_caption_delay_ms: i32,
    pub copyright_font: FontDesc,
    pub message_font: FontDesc,
    pub military_caption_title_font: FontDesc,
    pub military_caption_font: FontDesc,
    pub superweapon_countdown_normal_font: FontDesc,
    pub superweapon_countdown_ready_font: FontDesc,
    pub named_timer_countdown_normal_font: FontDesc,
    pub named_timer_countdown_ready_font: FontDesc,
    pub drawable_caption_font: FontDesc,
    pub default_window_font: FontDesc,
    pub default_display_string_font: FontDesc,
    /// `m_tooltipFontName` (`TooltipFontName`): the tooltip's font, a whole `FontDesc` despite its name.
    pub tooltip_font_name: FontDesc,
    pub native_debug_display: FontDesc,
    pub draw_group_info_font: FontDesc,
    pub credits_title_font: FontDesc,
    /// `m_creditsPositionFont` (the field `CreditsMinorTitleFont`).
    pub credits_position_font: FontDesc,
    pub credits_normal_font: FontDesc,
    /// `m_resolutionFontSizeAdjustment` (`ResolutionFontAdjustment`).
    pub resolution_font_size_adjustment: f32,
    /// `m_localFonts` (`LocalFontFile`), in the list's order (each new one at the front).
    pub local_fonts: Vec<Vec<u8>>,
}

impl Default for GlobalLanguage {
    /// `GlobalLanguage::GlobalLanguage()` (GlobalLanguage.cpp:110).
    fn default() -> Self {
        GlobalLanguage {
            unicode_font_name: Vec::new(),
            unicode_font_file_name: Vec::new(),
            use_hard_wrap: false,
            military_caption_speed: 0,
            military_caption_delay_ms: 750,
            copyright_font: FontDesc::default(),
            message_font: FontDesc::default(),
            military_caption_title_font: FontDesc::default(),
            military_caption_font: FontDesc::default(),
            superweapon_countdown_normal_font: FontDesc::default(),
            superweapon_countdown_ready_font: FontDesc::default(),
            named_timer_countdown_normal_font: FontDesc::default(),
            named_timer_countdown_ready_font: FontDesc::default(),
            drawable_caption_font: FontDesc::default(),
            default_window_font: FontDesc::default(),
            default_display_string_font: FontDesc::default(),
            tooltip_font_name: FontDesc::default(),
            native_debug_display: FontDesc::default(),
            draw_group_info_font: FontDesc::default(),
            credits_title_font: FontDesc::default(),
            credits_position_font: FontDesc::default(),
            credits_normal_font: FontDesc::default(),
            resolution_font_size_adjustment: if plant(9401) { 1.0 } else { crate::client::cinematic_text::DEFAULT_RESOLUTION_FONT_SIZE_ADJUSTMENT },
            local_fonts: Vec::new(),
        }
    }
}

/// `GlobalLanguage::parseFontDesc(ini, instance, store, userData)` (GlobalLanguage.cpp:177): each member stored as it is
/// read, so a token that fails leaves the ones before it set (`fontDesc->name = ...` before `scanInt` throws).
fn parse_font_desc(ini: &mut Ini<'_>, font_desc: &mut FontDesc) -> IniResult<()> {
    font_desc.name = ini.get_next_quoted_ascii_string()?;
    font_desc.size = Ini::scan_int(&ini.get_next_token(None)?)?;
    if !plant(9403) {
        font_desc.bold = Ini::scan_bool(&ini.get_next_token(None)?)?;
    }
    Ok(())
}

/// A `FontDesc` field of the table: `parseFontDesc` into the member.
macro_rules! font_field {
    ($token:literal, $member:ident) => {
        FieldParse::new($token, |ini, g: &mut GlobalLanguage, _| parse_font_desc(ini, &mut g.$member), UserData::None)
    };
}

/// `TheGlobalLanguageDataFieldParseTable` (GlobalLanguage.cpp:65), in its order.
pub const GLOBAL_LANGUAGE_FIELDS: &[FieldParse<GlobalLanguage>] = &[
    FieldParse::new("UnicodeFontName", |ini, g, _| ini.parse_ascii_string().map(|v| g.unicode_font_name = v), UserData::None),
    // `GlobalLanguage::parseFontFileName`: `m_localFonts.push_front(ini->getNextAsciiString())`
    FieldParse::new(
        "LocalFontFile",
        |ini, g, _| {
            let f = ini.get_next_ascii_string();
            if plant(9404) {
                g.local_fonts.push(f);
            } else {
                g.local_fonts.insert(0, f);
            }
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new("MilitaryCaptionSpeed", |ini, g, _| ini.parse_int().map(|v| g.military_caption_speed = v), UserData::None),
    FieldParse::new("UseHardWordWrap", |ini, g, _| ini.parse_bool().map(|v| g.use_hard_wrap = v), UserData::None),
    FieldParse::new("ResolutionFontAdjustment", |ini, g, _| ini.parse_real().map(|v| if !plant(9402) { g.resolution_font_size_adjustment = v }), UserData::None),
    font_field!("CopyrightFont", copyright_font),
    font_field!("MessageFont", message_font),
    font_field!("MilitaryCaptionTitleFont", military_caption_title_font),
    FieldParse::new("MilitaryCaptionDelayMS", |ini, g, _| ini.parse_int().map(|v| g.military_caption_delay_ms = v), UserData::None),
    font_field!("MilitaryCaptionFont", military_caption_font),
    font_field!("SuperweaponCountdownNormalFont", superweapon_countdown_normal_font),
    font_field!("SuperweaponCountdownReadyFont", superweapon_countdown_ready_font),
    font_field!("NamedTimerCountdownNormalFont", named_timer_countdown_normal_font),
    font_field!("NamedTimerCountdownReadyFont", named_timer_countdown_ready_font),
    font_field!("DrawableCaptionFont", drawable_caption_font),
    font_field!("DefaultWindowFont", default_window_font),
    font_field!("DefaultDisplayStringFont", default_display_string_font),
    font_field!("TooltipFontName", tooltip_font_name),
    font_field!("NativeDebugDisplay", native_debug_display),
    font_field!("DrawGroupInfoFont", draw_group_info_font),
    font_field!("CreditsTitleFont", credits_title_font),
    font_field!("CreditsMinorTitleFont", credits_position_font),
    font_field!("CreditsNormalFont", credits_normal_font),
    FieldParse::end(),
];

/// `INI::parseLanguageDefinition(ini)` (GlobalLanguage.cpp:99): `initFromINI(TheGlobalLanguageData, the table)`.
fn parse_language_definition(ini: &mut Ini<'_>, g: &mut GlobalLanguage) -> IniResult<()> {
    ini.init_from_ini(g, GLOBAL_LANGUAGE_FIELDS)
}

/// The block `Language.ini` holds (upstream's `theTypeTable` has it among the others).
pub const LANGUAGE_BLOCKS: &[BlockParse<GlobalLanguage>] = &[BlockParse { token: "Language", parse: parse_language_definition }];

impl GlobalLanguage {
    /// `Data\<language>\Language.ini`, the file `init` loads (`fname.format("Data\\%s\\Language.ini", ...)`).
    pub fn file_name(language: &[u8]) -> Vec<u8> {
        let mut f = b"Data\\".to_vec();
        f.extend_from_slice(language);
        f.extend_from_slice(b"\\Language.ini");
        f
    }

    /// `init`'s `ini.load(fname, INI_LOAD_OVERWRITE, NULL)` of the file's bytes: the `Language` blocks' fields over the
    /// current members.
    pub fn parse_ini(&mut self, path: &[u8], text: &[u8]) -> IniResult<()> {
        Ini::load(path, text, IniLoadType::Overwrite, None, LANGUAGE_BLOCKS, self)
    }

    /// `adjustFontSize(theFontSize)` (GlobalLanguage.cpp:192) with `TheGlobalData->m_xResolution` given.
    pub fn adjust_font_size(&self, the_font_size: i32, x_resolution: i32) -> i32 {
        crate::client::cinematic_text::adjust_font_size(the_font_size, x_resolution, self.resolution_font_size_adjustment)
    }

    /// `m_tooltipFontName` as the mouse asks for it (`Mouse::mouseNotifyResolutionChange`, Mouse.cpp:612): the font when
    /// its name is not empty (`isNotEmpty()`), else `None` (the mouse then takes `Mouse.ini`'s). Its size is the
    /// `FontDesc`'s, before `adjustFontSize`.
    pub fn tooltip_font(&self) -> Option<crate::client::window_script::GameFont> {
        let f = &self.tooltip_font_name;
        (!f.name.is_empty()).then(|| crate::client::window_script::GameFont { name: f.name.clone(), point_size: f.size, bold: f.bold })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #148, predicted before the run: the constructor's values (0.7, 750, `Arial Unicode MS` 12); a file setting
    /// `ResolutionFontAdjustment = 0.5`, the tooltip font `"Arial" 10 Yes` and two local fonts reads them, the fonts in
    /// reverse order (`push_front`); `adjustFontSize(20)` at 1600 wide is then 20 * (1 + 1 * 0.5) = 30 (34 with 0.7); an
    /// unknown field fails the load.
    #[test]
    fn language_ini_reads_as_upstream() {
        let mut g = GlobalLanguage::default();
        assert_eq!((g.resolution_font_size_adjustment, g.military_caption_delay_ms, g.tooltip_font_name.clone()), (0.7, 750, FontDesc::default()));
        assert_eq!(g.adjust_font_size(20, 1600), 34);
        assert_eq!(g.tooltip_font(), Some(crate::client::window_script::GameFont { name: b"Arial Unicode MS".to_vec(), point_size: 12, bold: false }));
        let text = b"Language\n  ResolutionFontAdjustment = 0.5\n  TooltipFontName = \"Arial\" 10 Yes\n  LocalFontFile = a.ttf\n  LocalFontFile = b.ttf\n  UnicodeFontName = Arial Unicode MS\nEnd\n";
        g.parse_ini(b"Data\\English\\Language.ini", text).expect("reads");
        assert_eq!(g.resolution_font_size_adjustment, 0.5);
        assert_eq!(g.tooltip_font_name, FontDesc { name: b"Arial".to_vec(), size: 10, bold: true });
        assert_eq!(g.local_fonts, vec![b"b.ttf".to_vec(), b"a.ttf".to_vec()]);
        // (`parseAsciiString` takes one token: the rest of the line is not the name)
        assert_eq!(g.unicode_font_name, b"Arial".to_vec());
        assert_eq!(g.adjust_font_size(20, 1600), 30);
        assert!(GlobalLanguage::default().parse_ini(b"x", b"Language\n  NoSuchField = 1\nEnd\n").is_err());
        assert_eq!(GlobalLanguage::file_name(b"English"), b"Data\\English\\Language.ini".to_vec());
    }
}
