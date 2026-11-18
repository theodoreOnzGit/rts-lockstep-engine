//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/Credits.cpp   (`CreditsManager::addBlank`, `parseBlank`)

/// `CREDIT_STYLE_BLANK` ("Keep this second to last"): TITLE 0, POSITION 1, NORMAL 2, COLUMN 3, BLANK 4.
pub const CREDIT_STYLE_BLANK: i32 = 4;

/// The parts of `CreditsLine` the parse functions set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CreditsLine {
    /// `m_style`.
    pub style: i32,
}

/// `CreditsManager::m_creditLineList`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CreditsManagerLines {
    /// The list.
    pub credit_line_list: Vec<CreditsLine>,
}

impl CreditsManagerLines {
    /// `CreditsManager::addBlank()` (Credits.cpp:409): a new line of style `CREDIT_STYLE_BLANK` at the back.
    pub fn add_blank(&mut self) {
        self.credit_line_list.push(CreditsLine { style: CREDIT_STYLE_BLANK });
    }

    /// `CreditsManager::parseBlank(ini, instance, ...)` (Credits.cpp:418): the field handler for `Blank`; it reads
    /// nothing from the INI line.
    pub fn parse_blank(&mut self) {
        self.add_blank();
    }
}
