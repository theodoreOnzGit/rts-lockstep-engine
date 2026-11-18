//! UNTESTED DRAFT (#202, #205): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplayStringManager.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DDisplayStringManager.h
//!
//! The base `DisplayStringManager` (GameClient/DisplayStringManager.cpp, not this area's) is represented by what this
//! class touches: the string list (`m_stringList`, `link` at the head, `unLink`) and `m_currentCheckpoint`. A
//! string is its id; the W3D string's text renderers (`m_textRenderer`, `m_textRendererHotKey`, device-only) are only
//! counted as released.

use std::collections::HashMap;

/// `MAX_GROUPS` (W3DDisplayStringManager.h:46; 20 under `KRIS_BRUTAL_HACK_FOR_AIRCRAFT_CARRIER_DEBUGGING`).
pub const MAX_GROUPS: usize = 10;

/// `W3DDisplayString`'s members the manager reads and writes.
#[derive(Clone, Debug, Default)]
pub struct W3DDisplayStringState {
    /// `m_lastResourceFrame`: 0 means no render resources held.
    pub last_resource_frame: u32,
    /// `m_textChanged`.
    pub text_changed: bool,
    /// The text, a font name (`GameFont`) and whether render resources are built (`m_textRenderer.Reset()` clears it).
    pub text: Vec<u16>,
    pub font: Option<(Vec<u8>, i32, bool)>,
    pub has_text_renderers: bool,
}

/// `DisplayString` handle: an id in the manager's list.
pub type DisplayStringId = u32;

/// What the manager asks of the rest of the game (`TheFontLibrary`, `TheGameText`, `TheDrawGroupInfo`,
/// `TheGlobalLanguageData`, `TheGameClient`).
pub trait DisplayStringHost {
    /// `TheGlobalLanguageData->m_defaultDisplayStringFont` (name, size, bold) when its name is not empty.
    fn default_display_string_font(&self) -> Option<(Vec<u8>, i32, bool)>;
    /// `TheDrawGroupInfo->m_fontName / m_fontSize / m_fontIsBold`.
    fn draw_group_font(&self) -> (Vec<u8>, i32, bool);
    /// `TheGameText->fetch(key)`.
    fn fetch_text(&mut self, key: &[u8]) -> Vec<u16>;
    /// `TheGameClient->getFrame()`.
    fn client_frame(&self) -> u32;
}

/// `class W3DDisplayStringManager`.
#[derive(Clone, Debug, Default)]
pub struct W3DDisplayStringManager {
    next_id: DisplayStringId,
    /// `m_stringList` front to back (`link` puts a new string at the head).
    pub string_list: Vec<DisplayStringId>,
    pub strings: HashMap<DisplayStringId, W3DDisplayStringState>,
    /// `m_currentCheckpoint`.
    pub current_checkpoint: Option<DisplayStringId>,
    /// `m_groupNumeralStrings[MAX_GROUPS]`.
    pub group_numeral_strings: [Option<DisplayStringId>; MAX_GROUPS],
    /// `m_formationLetterDisplayString`.
    pub formation_letter_display_string: Option<DisplayStringId>,
}

impl W3DDisplayStringManager {
    /// `W3DDisplayStringManager::W3DDisplayStringManager` (W3DDisplayStringManager.cpp:50).
    pub fn new() -> Self {
        W3DDisplayStringManager::default()
    }

    /// `W3DDisplayStringManager::~W3DDisplayStringManager` (W3DDisplayStringManager.cpp:63).
    pub fn destroy(&mut self) {
        for i in 0..MAX_GROUPS {
            if let Some(s) = self.group_numeral_strings[i] {
                self.free_display_string(Some(s));
            }
            self.group_numeral_strings[i] = None;
        }
        if let Some(s) = self.formation_letter_display_string {
            self.free_display_string(Some(s));
        }
        self.formation_letter_display_string = None;
    }

    /// `W3DDisplayStringManager::postProcessLoad` (W3DDisplayStringManager.cpp:84).
    pub fn post_process_load(&mut self, host: &mut dyn DisplayStringHost) {
        // Get the font.
        let font = host.draw_group_font();
        for i in 0..MAX_GROUPS {
            let id = self.new_display_string(host);
            self.strings.get_mut(&id).unwrap().font = Some(font.clone());
            let text = host.fetch_text(format!("NUMBER:{}", i).as_bytes());
            self.strings.get_mut(&id).unwrap().text = text;
            self.group_numeral_strings[i] = Some(id);
        }
        let id = self.new_display_string(host);
        self.strings.get_mut(&id).unwrap().font = Some(font);
        let text = host.fetch_text(b"LABEL:FORMATION");
        self.strings.get_mut(&id).unwrap().text = text;
        self.formation_letter_display_string = Some(id);
    }

    /// `W3DDisplayStringManager::newDisplayString` (W3DDisplayStringManager.cpp:119): a new string with the default
    /// font, linked at the head of the list.
    pub fn new_display_string(&mut self, host: &mut dyn DisplayStringHost) -> DisplayStringId {
        self.next_id += 1;
        let id = self.next_id;
        let mut state = W3DDisplayStringState::default();
        // assign a default font
        state.font = Some(match host.default_display_string_font() {
            Some(f) => f,
            None => (b"Times New Roman".to_vec(), 12, false),
        });
        self.strings.insert(id, state);
        // link string to list
        self.string_list.insert(0, id);
        id
    }

    /// `W3DDisplayStringManager::freeDisplayString` (W3DDisplayStringManager.cpp:155).
    pub fn free_display_string(&mut self, string: Option<DisplayStringId>) {
        // sanity
        let Some(string) = string else {
            return;
        };
        // unlink
        self.string_list.retain(|s| *s != string);
        // if the string happens to fall where our current checkpoint was, set the checkpoint to null
        if self.current_checkpoint == Some(string) {
            self.current_checkpoint = None;
        }
        // free data
        self.strings.remove(&string);
    }

    /// `W3DDisplayStringManager::update` (W3DDisplayStringManager.cpp:181): frees the render resources of up to ten
    /// strings not drawn for 60 frames, continuing from the last checkpoint.
    pub fn update(&mut self, host: &dyn DisplayStringHost) {
        // if the m_currentCheckpoint is valid, use it for the starting point for the search
        let mut pos: Option<usize> = if let Some(c) = self.current_checkpoint {
            self.string_list.iter().position(|s| *s == c)
        } else if self.string_list.is_empty() {
            None
        } else {
            Some(0)
        };

        let curr_frame = host.client_frame();
        const W3D_CLEANUP_TIME: u32 = 60; // any string not rendered after this many frames will have its render resources freed

        let mut num_strings = 10;
        // looping through all the strings eats up a lot of ambient time. Instead,
        // loop through 10 (arbitrarily chosen) or till the end is hit.
        while num_strings > 0 && pos.is_some() {
            num_strings -= 1;
            let idx = pos.unwrap();
            let id = self.string_list[idx];
            let s = self.strings.get_mut(&id).unwrap();
            // has this string "expired" in terms of using resources, a string with a resource frame of zero isn't
            // using any resources at all
            if s.last_resource_frame != 0 && curr_frame.wrapping_sub(s.last_resource_frame) > W3D_CLEANUP_TIME {
                // free the resources
                s.has_text_renderers = false; // m_textRenderer.Reset(); m_textRendererHotKey.Reset()
                // mark data in the string as changed so that if it needs to be drawn again it will know to reconstruct
                s.text_changed = true;
                // set the last resource frame to zero, this allows us to ignore it in future cleanup passes
                s.last_resource_frame = 0;
            }
            // move to next string
            pos = if idx + 1 < self.string_list.len() { Some(idx + 1) } else { None };
        }

        // reset the starting point for our next search
        self.current_checkpoint = pos.map(|i| self.string_list[i]);
    }

    /// `W3DDisplayStringManager::getGroupNumeralString` (W3DDisplayStringManager.cpp:244): out of range asserts, and
    /// gives string 0.
    pub fn get_group_numeral_string(&self, numeral: i32) -> Option<DisplayStringId> {
        if numeral < 0 || numeral > MAX_GROUPS as i32 - 1 {
            // DEBUG_CRASH(("Numeral '%d' out of range.\n", numeral));
            return self.group_numeral_strings[0];
        }
        self.group_numeral_strings[numeral as usize]
    }
}
