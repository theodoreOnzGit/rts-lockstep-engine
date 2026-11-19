//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 6,000 programs (link_font, unlink_font, delete_all_fonts; release_font_data only clears the handle, the device release is not covered) (test: tests/differential_draft_fonts.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameFont.cpp (FontLibrary::linkFont, unlinkFont, deleteAllFonts)
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/W3DGameFont.cpp (W3DFontLibrary::releaseFontData)
//!
//! The library keeps its loaded fonts as a singly linked list through `GameFont::next`; here a `Vec`, head first, which
//! has the same order (`linkFont` puts at the head). `m_count` is its length.
//!
//! `W3DFontLibrary::releaseFontData` (W3DGameFont.cpp:130) releases the `FontCharsClass` (and its alternate Unicode font)
//! behind `font->fontData` with `Release_Ref`: a reference count of WW3D's font texture pages, which the client draws
//! with its own font cache (`client/src/fonts.rs`). The draft clears the data handle, as upstream does last.

use crate::client::window_script::GameFont;
use crate::logic::modules::plant;

/// A font in the library: the font and its device data handle (`fontData`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryFont {
    pub font: GameFont,
    /// `fontData`: the device's font handle (`None`: `NULL`).
    pub font_data: Option<u32>,
}

/// `class FontLibrary`'s list.
#[derive(Clone, Debug, Default)]
pub struct FontLibrary {
    /// `m_fontList` .. `next`, head first.
    pub font_list: Vec<LibraryFont>,
    /// `m_count`.
    pub count: i32,
}

impl FontLibrary {
    /// `FontLibrary::linkFont(font)` (GameFont.cpp:44): to the head of the list, counted. (A `NULL` font does nothing.)
    pub fn link_font(&mut self, font: Option<LibraryFont>) {
        let Some(font) = font else { return };
        if plant(15669) {
            self.font_list.push(font);
        } else {
            self.font_list.insert(0, font);
        }
        if !plant(15670) {
            self.count += 1;
        }
    }

    /// `FontLibrary::unlinkFont(font)` (GameFont.cpp:63): takes the font (by its place) out of the list and returns it,
    /// uncounted. A font not in the list is a `DEBUG_CRASH` and nothing changes. `font` is its place in `font_list`.
    pub fn unlink_font(&mut self, font: Option<usize>) -> Option<LibraryFont> {
        let i = font?;
        if i >= self.font_list.len() {
            // DEBUG_CRASH(( "Font '%s' not found in library\n", font->nameString.str() ));
            if plant(15672) {
                self.count -= 1;
            }
            return None;
        }
        if !plant(15671) {
            self.count -= 1;
        }
        if plant(15674) && i + 1 < self.font_list.len() {
            return Some(self.font_list.remove(i + 1));
        }
        Some(self.font_list.remove(i))
    }

    /// `FontLibrary::deleteAllFonts()` (GameFont.cpp:108): each font is unlinked, its device data released
    /// (`releaseFontData`) and the font deleted, from the head, until the list is empty.
    pub fn delete_all_fonts(&mut self) {
        while self.font_list.len() > if plant(15673) { 1 } else { 0 } {
            let mut font = self.unlink_font(Some(0));
            if let Some(f) = font.as_mut() {
                release_font_data(f);
            }
            // font->deleteInstance();
        }
    }
}

/// `W3DFontLibrary::releaseFontData(font)` (W3DGameFont.cpp:130): the device data is released and `fontData = NULL`.
pub fn release_font_data(font: &mut LibraryFont) {
    // if (font && font->fontData) { ((FontCharsClass *)font->fontData)->AlternateUnicodeFont->Release_Ref(); ->Release_Ref(); }
    // upstream: FontCharsClass::Release_Ref — device-only, not translated
    font.font_data = None;
}
