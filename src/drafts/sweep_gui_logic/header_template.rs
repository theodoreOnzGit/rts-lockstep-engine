//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/HeaderTemplate.cpp (HeaderTemplateManager::findHeaderTemplate,
//!   newHeaderTemplate, getFirstHeader, getNextHeader, populateGameFonts)
//!
//! A header template names a font for the window headers; the manager holds them in a list, newest first
//! (`push_front`). The verified port asks the manager for a font through `WndHost::font_from_template`.

use crate::client::window_script::GameFont;

/// `class HeaderTemplate`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeaderTemplate {
    /// `m_name`.
    pub name: Vec<u8>,
    /// `m_fontName`.
    pub font_name: Vec<u8>,
    /// `m_point`.
    pub point: i32,
    /// `m_bold`.
    pub bold: bool,
    /// `m_font`.
    pub font: Option<GameFont>,
}

/// What `populateGameFonts` asks of the game.
pub trait HeaderTemplateFontHost {
    /// `TheGlobalLanguageData->adjustFontSize(point)`.
    fn adjust_font_size(&self, point: i32) -> f32;
    /// `TheFontLibrary->getFont(name, pointSize, bold)`.
    fn get_font(&self, name: &[u8], point_size: i32, bold: bool) -> Option<GameFont>;
}

/// `class HeaderTemplateManager`'s list.
#[derive(Clone, Debug, Default)]
pub struct HeaderTemplateManager {
    /// `m_headerTemplateList`, front first.
    pub header_template_list: Vec<HeaderTemplate>,
}

impl HeaderTemplateManager {
    /// `findHeaderTemplate(name)` (HeaderTemplate.cpp:163): the first with that name (a case-sensitive `compare`), by
    /// its place in the list.
    pub fn find_header_template(&self, name: &[u8]) -> Option<usize> {
        self.header_template_list.iter().position(|h| h.name.as_slice() == name)
    }

    /// `newHeaderTemplate(name)` (:176): a new template at the front of the list; its place (0).
    pub fn new_header_template(&mut self, name: &[u8]) -> usize {
        let new_h = HeaderTemplate { name: name.to_vec(), ..HeaderTemplate::default() };
        self.header_template_list.insert(0, new_h);
        0
    }

    /// `getFirstHeader()` (:201): the front of the list, if any.
    pub fn get_first_header(&self) -> Option<usize> {
        if self.header_template_list.is_empty() {
            None
        } else {
            Some(0)
        }
    }

    /// `getNextHeader(ht)` (:210): the template after `ht`; none for the last, or for one not in the list.
    pub fn get_next_header(&self, ht: usize) -> Option<usize> {
        if ht < self.header_template_list.len() && ht + 1 < self.header_template_list.len() {
            Some(ht + 1)
        } else {
            None
        }
    }

    /// `populateGameFonts()` (:236): each template's font is the library's for its (language adjusted) point size.
    /// A missing font is a `DEBUG_ASSERTCRASH` and leaves `m_font` `NULL`.
    pub fn populate_game_fonts<H: HeaderTemplateFontHost>(&mut self, h: &H) {
        for t in self.header_template_list.iter_mut() {
            // Real pointSize = TheGlobalLanguageData->adjustFontSize(hTemplate->m_point);
            let point_size = h.adjust_font_size(t.point);
            // getFont(name, Real pointSize, bold): the Real converts to the library's Int
            t.font = h.get_font(&t.font_name, point_size as i32, t.bold);
        }
    }
}
