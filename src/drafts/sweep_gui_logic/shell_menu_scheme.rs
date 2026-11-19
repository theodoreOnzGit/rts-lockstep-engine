//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 10,000 programs (the line default, new_shell_menu_scheme, set_shell_menu_scheme; `parse_line_part` and its field table are NOT covered: they need the INI reader) (test: tests/differential_draft_shellscheme.rs);
//! not yet wired into the game.
//! (After a draft fix, 2026-11-19: `new_shell_menu_scheme` moves the current index down when a scheme before it is replaced.)
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/Shell/ShellMenuScheme.cpp (ShellMenuSchemeManager::parseLinePart,
//!   newShellMenuScheme, setShellMenuScheme)
//!   GeneralsMD/Code/GameEngine/Include/GameClient/ShellMenuScheme.h (the line and scheme types)
//!
//! The shell's decoration (images and lines drawn behind the menus) per `ShellMenuScheme.ini` scheme. The verified
//! `client::shell` does not draw schemes (its module docs list them as not translated); this is the scheme list and the
//! line parser.

use crate::common::base_type::ICoord2D;
use crate::common::ini::{FieldParse, Ini, IniResult, UserData};
use crate::logic::modules::plant;

/// `class ShellMenuSchemeLine`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellMenuSchemeLine {
    /// `m_startPos`.
    pub start_pos: ICoord2D,
    /// `m_endPos`.
    pub end_pos: ICoord2D,
    /// `m_width`.
    pub width: i32,
    /// `m_color`.
    pub color: i32,
}

impl Default for ShellMenuSchemeLine {
    /// `ShellMenuSchemeLine::ShellMenuSchemeLine()` (ShellMenuScheme.cpp:103): positions 0, colour `GAME_COLOR_UNDEFINED`,
    /// width 1.
    fn default() -> Self {
        ShellMenuSchemeLine {
            start_pos: ICoord2D::default(),
            end_pos: ICoord2D::default(),
            width: if plant(15751) { 0 } else { 1 },
            color: if plant(15752) { 0 } else { crate::client::window_script::WIN_COLOR_UNDEFINED },
        }
    }
}

/// `class ShellMenuScheme`: the lines (the images are `ShellMenuSchemeImage`, parsed by `parseImagePart`, not in this
/// sweep's list).
#[derive(Clone, Debug, Default)]
pub struct ShellMenuScheme {
    /// `m_name` (lower case).
    pub name: Vec<u8>,
    /// `m_lineList`.
    pub line_list: Vec<ShellMenuSchemeLine>,
}

impl ShellMenuScheme {
    /// `addLine(schemeLine)`: pushed at the back.
    pub fn add_line(&mut self, scheme_line: ShellMenuSchemeLine) {
        self.line_list.push(scheme_line);
    }
}

/// `myFieldParse` of `parseLinePart` (ShellMenuScheme.cpp:243).
pub fn shell_menu_scheme_line_fields() -> Vec<FieldParse<ShellMenuSchemeLine>> {
    vec![
        FieldParse::new("StartPosition", |ini, l, _| ini.parse_icoord2d().map(|c| l.start_pos = c), UserData::None),
        FieldParse::new("EndPosition", |ini, l, _| ini.parse_icoord2d().map(|c| l.end_pos = c), UserData::None),
        FieldParse::new("Color", |ini, l, _| ini.parse_color_int().map(|c| l.color = c), UserData::None),
        FieldParse::new("Width", |ini, l, _| ini.parse_int().map(|w| l.width = w), UserData::None),
        FieldParse::end(),
    ]
}

/// `ShellMenuSchemeManager::parseLinePart(ini, instance, store, userData)` (ShellMenuScheme.cpp:240): a new line read
/// from the INI block and added to the scheme (`instance`).
pub fn parse_line_part(ini: &mut Ini<'_>, instance: &mut ShellMenuScheme) -> IniResult<()> {
    let mut scheme_line = ShellMenuSchemeLine::default();
    ini.init_from_ini(&mut scheme_line, &shell_menu_scheme_line_fields())?;
    instance.add_line(scheme_line);
    Ok(())
}

/// `class ShellMenuSchemeManager`'s list.
#[derive(Clone, Debug, Default)]
pub struct ShellMenuSchemeManager {
    /// `m_schemeList`, in order.
    pub scheme_list: Vec<ShellMenuScheme>,
    /// `m_currentScheme`, by its place in the list.
    pub current_scheme: Option<usize>,
}

impl ShellMenuSchemeManager {
    /// `newShellMenuScheme(name)` (:258): the name lower-cased; a scheme of that name already in the list is dropped (and
    /// deleted), and a new empty one appended. Its place (the last). Upstream does not fix `m_currentScheme` when it
    /// deletes the scheme it points to (a dangling pointer); here the index is kept as it was (and moves down when a scheme
    /// before the current one is removed, as upstream's pointer follows its scheme).
    pub fn new_shell_menu_scheme(&mut self, name: &[u8]) -> usize {
        let name = if plant(15745) { name.to_vec() } else { name.to_ascii_lowercase() };
        let mut at = None;
        if let Some(i) = self.scheme_list.iter().position(|s| s.name == name) {
            if !plant(15746) {
                self.scheme_list.remove(i);
                at = Some(i);
                // the current scheme is a pointer upstream and follows its scheme; here an index, which moves down when a scheme before it
                // goes. (A scheme that IS the current one is a dangling pointer upstream; the index is then kept as it was.)
                // ~~kept the index as it was in every case~~ **Corrected 2026-11-19 (differential, #215, tests/differential_draft_shellscheme.rs):**
                // an index kept after a removal before it names the next scheme; the draft mismatched upstream in 805 of 10,000 programs.
                if let Some(c) = self.current_scheme {
                    if i < c && !plant(15764) {
                        self.current_scheme = Some(c - 1);
                    }
                }
            }
        }
        if plant(15747) {
            if let Some(i) = at {
                self.scheme_list.insert(i, ShellMenuScheme { name, line_list: Vec::new() });
                return i;
            }
        }
        self.scheme_list.push(ShellMenuScheme { name, line_list: Vec::new() });
        self.scheme_list.len() - 1
    }

    /// `setShellMenuScheme(name)` (:290): an empty name clears the current scheme; else the (lower-cased) name selects
    /// the scheme of that name, and an unknown name leaves the current one as it was.
    pub fn set_shell_menu_scheme(&mut self, name: &[u8]) {
        if name.is_empty() && !plant(15749) {
            self.current_scheme = None;
            return;
        }
        let name = if plant(15748) { name.to_vec() } else { name.to_ascii_lowercase() };
        if let Some(i) = self.scheme_list.iter().position(|s| s.name == name) {
            self.current_scheme = Some(i);
        } else if plant(15750) {
            self.current_scheme = None;
        }
    }
}
