//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/W3DGameFont.cpp
//!
//! Sweep 2 (#213): `W3DFontLibrary::releaseFontData`.

/// A `GameFont`'s `fontData` (a `FontCharsClass *`) and its alternate unicode font, as the references held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameFontData {
    /// `fontData != NULL`.
    pub has_font_data: bool,
    /// `((FontCharsClass *)fontData)->AlternateUnicodeFont != NULL`.
    pub has_alternate_unicode_font: bool,
}

/// What `releaseFontData` releases, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontRelease {
    /// `AlternateUnicodeFont->Release_Ref()`.
    AlternateUnicodeFont,
    /// `FontCharsClass::Release_Ref()`.
    FontChars,
}

/// `W3DFontLibrary::releaseFontData` (W3DGameFont.cpp:130): releases the alternate unicode font first, then the font chars,
/// and sets `fontData` to NULL (also when there was none). Upstream dereferences `font` unconditionally in the last line.
pub fn release_font_data(font: &mut GameFontData) -> Vec<FontRelease> {
    let mut out = Vec::new();
    if font.has_font_data {
        if font.has_alternate_unicode_font {
            out.push(FontRelease::AlternateUnicodeFont);
        }
        out.push(FontRelease::FontChars);
    }
    font.has_font_data = false;
    out
}
