//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/Common/System/FunctionLexicon.cpp
//!   GeneralsMD/Code/GameEngine/Include/Common/FunctionLexicon.h
//!
//! `TheFunctionLexicon` maps names to window callbacks through tables of (name, key, function pointer). The port's
//! window code dispatches by enum (`crate::client::window_script`, "whether TheFunctionLexicon has a function of this
//! kind and name"), so the pointer is a generic `F` (an enum value or fn id) compared by equality. This is the table
//! engine: `loadTable`, `keyToFunc`, `findFunction`, `validate` and the two typed lookups with the `TABLE_ANY` fallback.
//!
//! Excluded (#213): `FunctionLexicon::funcToName` is inside `#ifdef NOT_IN_USE`; `functionToName` is commented out.

/// `NAMEKEY_INVALID`.
pub const NAMEKEY_INVALID: i32 = 0;

/// `TableIndex`: `TABLE_ANY` = -1, then the tables in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableIndex {
    /// `TABLE_ANY`.
    Any = -1,
    /// `TABLE_GAME_WIN_SYSTEM`.
    GameWinSystem = 0,
    /// `TABLE_GAME_WIN_INPUT`.
    GameWinInput,
    /// `TABLE_GAME_WIN_TOOLTIP`.
    GameWinTooltip,
    /// `TABLE_GAME_WIN_DEVICEDRAW`.
    GameWinDeviceDraw,
    /// `TABLE_GAME_WIN_DRAW`.
    GameWinDraw,
    /// `TABLE_WIN_LAYOUT_INIT`.
    WinLayoutInit,
    /// `TABLE_WIN_LAYOUT_DEVICEINIT`.
    WinLayoutDeviceInit,
    /// `TABLE_WIN_LAYOUT_UPDATE`.
    WinLayoutUpdate,
    /// `TABLE_WIN_LAYOUT_SHUTDOWN`.
    WinLayoutShutdown,
}

/// `MAX_FUNCTION_TABLES`.
pub const MAX_FUNCTION_TABLES: usize = 9;

/// `struct TableEntry`: `key` (set by `loadTable`), `name`, `func` (`None` for a NULL function).
#[derive(Clone, Debug, PartialEq)]
pub struct TableEntry<F> {
    /// `key`.
    pub key: i32,
    /// `name`.
    pub name: &'static str,
    /// `func`.
    pub func: Option<F>,
}

/// `class FunctionLexicon`.
#[derive(Clone, Debug)]
pub struct FunctionLexicon<F> {
    /// `m_tables[MAX_FUNCTION_TABLES]` (empty: NULL).
    pub tables: Vec<Vec<TableEntry<F>>>,
}

impl<F: Copy + PartialEq> FunctionLexicon<F> {
    /// `FunctionLexicon::FunctionLexicon()`: every table NULL.
    pub fn new() -> FunctionLexicon<F> {
        FunctionLexicon { tables: vec![Vec::new(); MAX_FUNCTION_TABLES] }
    }

    /// `FunctionLexicon::loadTable(table, tableIndex)` (FunctionLexicon.cpp:384): give every entry its name key
    /// (`name_to_key` is `TheNameKeyGenerator->nameToKey`) and install the table at the index.
    pub fn load_table(&mut self, mut table: Vec<TableEntry<F>>, table_index: TableIndex, name_to_key: &mut dyn FnMut(&str) -> i32) {
        for entry in table.iter_mut() {
            entry.key = name_to_key(entry.name);
        }
        self.tables[table_index as usize] = table;
    }

    /// `FunctionLexicon::keyToFunc(key, table)` (FunctionLexicon.cpp:413): the table is walked until an entry whose
    /// key is `NAMEKEY_INVALID` (the table's terminator).
    pub fn key_to_func(key: i32, table: &[TableEntry<F>]) -> Option<F> {
        if key == NAMEKEY_INVALID {
            return None;
        }
        for entry in table.iter() {
            if entry.key == NAMEKEY_INVALID {
                break;
            }
            if entry.key == key {
                return entry.func;
            }
        }
        None
    }

    /// `FunctionLexicon::findFunction(key, index)` (FunctionLexicon.cpp:440): `TABLE_ANY` searches every table in order.
    pub fn find_function(&self, key: i32, index: TableIndex) -> Option<F> {
        if key == NAMEKEY_INVALID {
            return None;
        }
        if index == TableIndex::Any {
            for i in 0..MAX_FUNCTION_TABLES {
                let func = Self::key_to_func(key, &self.tables[i]);
                if func.is_some() {
                    return func;
                }
            }
            None
        } else {
            Self::key_to_func(key, &self.tables[index as usize])
        }
    }

    /// `FunctionLexicon::validate()`: every non-NULL function appears once across all tables (a duplicate logs a
    /// warning and makes this FALSE).
    pub fn validate(&self) -> bool {
        let mut valid = true;
        for i in 0..MAX_FUNCTION_TABLES {
            for (si, source) in self.tables[i].iter().enumerate() {
                if source.key == NAMEKEY_INVALID {
                    break;
                }
                if source.func.is_some() {
                    for j in 0..MAX_FUNCTION_TABLES {
                        for (li, look) in self.tables[j].iter().enumerate() {
                            if look.key == NAMEKEY_INVALID {
                                break;
                            }
                            if !(i == j && si == li) && source.func == look.func {
                                // upstream: DEBUG_LOG("WARNING! Function lexicon entries match same address! '%s' and '%s'")
                                valid = false;
                            }
                        }
                    }
                }
            }
        }
        valid
    }

    /// `FunctionLexicon::gameWinDrawFunc(key, index)` (FunctionLexicon.cpp:683): for `TABLE_ANY` the device-draw table
    /// first, then the plain draw table.
    pub fn game_win_draw_func(&self, key: i32, index: TableIndex) -> Option<F> {
        if index == TableIndex::Any {
            let func = self.find_function(key, TableIndex::GameWinDeviceDraw);
            if func.is_none() {
                return self.find_function(key, TableIndex::GameWinDraw);
            }
            return func;
        }
        self.find_function(key, index)
    }

    /// `FunctionLexicon::winLayoutInitFunc(key, index)` (FunctionLexicon.cpp:701): the device-init table first.
    pub fn win_layout_init_func(&self, key: i32, index: TableIndex) -> Option<F> {
        if index == TableIndex::Any {
            let func = self.find_function(key, TableIndex::WinLayoutDeviceInit);
            if func.is_none() {
                return self.find_function(key, TableIndex::WinLayoutInit);
            }
            return func;
        }
        self.find_function(key, index)
    }
}
