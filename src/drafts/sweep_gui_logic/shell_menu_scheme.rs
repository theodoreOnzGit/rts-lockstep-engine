//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
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
        ShellMenuSchemeLine { start_pos: ICoord2D::default(), end_pos: ICoord2D::default(), width: 1, color: crate::client::window_script::WIN_COLOR_UNDEFINED }
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
    /// deletes the scheme it points to (a dangling pointer); here the index is kept as it was.
    pub fn new_shell_menu_scheme(&mut self, name: &[u8]) -> usize {
        let name = name.to_ascii_lowercase();
        if let Some(i) = self.scheme_list.iter().position(|s| s.name == name) {
            self.scheme_list.remove(i);
        }
        self.scheme_list.push(ShellMenuScheme { name, line_list: Vec::new() });
        self.scheme_list.len() - 1
    }

    /// `setShellMenuScheme(name)` (:290): an empty name clears the current scheme; else the (lower-cased) name selects
    /// the scheme of that name, and an unknown name leaves the current one as it was.
    pub fn set_shell_menu_scheme(&mut self, name: &[u8]) {
        if name.is_empty() {
            self.current_scheme = None;
            return;
        }
        let name = name.to_ascii_lowercase();
        if let Some(i) = self.scheme_list.iter().position(|s| s.name == name) {
            self.current_scheme = Some(i);
        }
    }
}
