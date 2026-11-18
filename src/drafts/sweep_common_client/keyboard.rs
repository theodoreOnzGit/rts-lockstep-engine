//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 30,000 op lists (test: tests/differential_draft_keyboard.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/Input/Keyboard.cpp   (`getFirstKey`, `getKeyStatusData`,
//!     `getKeyStateBit`, `getKeySequenceData`, `setKeyStatusData`, `setKeyStateData`, `isShift`, `isCtrl`, `isAlt`)
//!   GeneralsMD/Code/GameEngine/Include/GameClient/Keyboard.h        (`KeyboardIO`)
//!
//! The verified `crate::client::keyboard` holds the key names (`KeyNames`, `getPrintableKey`). The key arrays and the
//! modifier tests are [`KeyboardState`]. `KEY_STATE_*` are the bits of `crate::client::meta_event` (KeyDefs.h).

use crate::logic::modules::plant;

/// `KEY_STATE_*` bits (KeyDefs.h); the same values as `crate::client::meta_event::key_state` (`i32` there, `u16` here
/// for `KeyboardIO::state`).
pub const KEY_STATE_LCONTROL: u16 = 0x0004;
/// `KEY_STATE_RCONTROL`.
pub const KEY_STATE_RCONTROL: u16 = 0x0008;
/// `KEY_STATE_LSHIFT`.
pub const KEY_STATE_LSHIFT: u16 = 0x0010;
/// `KEY_STATE_RSHIFT`.
pub const KEY_STATE_RSHIFT: u16 = 0x0020;
/// `KEY_STATE_LALT`.
pub const KEY_STATE_LALT: u16 = 0x0040;
/// `KEY_STATE_RALT`.
pub const KEY_STATE_RALT: u16 = 0x0080;
/// `KEY_STATE_SHIFT2`.
pub const KEY_STATE_SHIFT2: u16 = 0x0400;
/// `NUM_KEYS` (256, one slot per scan code).
pub const NUM_KEYS: usize = 256;

/// `struct KeyboardIO`: `key`, `status` (`StatusType`: 0 unused, 1 used), `state` (`KEY_STATE_*`), `sequence`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyboardIO {
    /// `key`.
    pub key: u8,
    /// `status`.
    pub status: u8,
    /// `state`.
    pub state: u16,
    /// `sequence`.
    pub sequence: u32,
}

/// `Keyboard::m_keys`, `m_keyStatus` and `m_modifiers`.
#[derive(Clone, Debug)]
pub struct KeyboardState {
    /// `m_keys[KEY_BUFFER]`.
    pub keys: Vec<KeyboardIO>,
    /// `m_keyStatus[NUM_KEYS]`.
    pub key_status: Vec<KeyboardIO>,
    /// `m_modifiers`.
    pub modifiers: u16,
}

impl KeyboardState {
    /// A cleared keyboard.
    pub fn new(key_buffer: usize) -> KeyboardState {
        KeyboardState { keys: vec![KeyboardIO::default(); key_buffer.max(1)], key_status: vec![KeyboardIO::default(); NUM_KEYS], modifiers: 0 }
    }

    /// `Keyboard::getFirstKey()` (Keyboard.cpp:763): `&m_keys[0]`.
    pub fn get_first_key(&self) -> &KeyboardIO {
        &self.keys[if plant(12328) { 1 } else { 0 }]
    }

    /// `Keyboard::getKeyStatusData(key)` (Keyboard.cpp:771).
    pub fn get_key_status_data(&self, key: u8) -> u8 {
        if plant(12326) {
            self.keys[key as usize].status
        } else {
            self.key_status[key as usize].status
        }
    }

    /// `Keyboard::getKeyStateBit(key, bit)` (Keyboard.cpp:779): `(state & bit) ? 1 : 0`. Upstream's `state` is an
    /// `UnsignedShort` and `bit` an `Int`; the AND is done in `Int`.
    pub fn get_key_state_bit(&self, key: u8, bit: i32) -> bool {
        if plant(12321) {
            (self.key_status[key as usize].state as i32 & bit) == bit
        } else {
            (self.key_status[key as usize].state as i32 & bit) != 0
        }
    }

    /// `Keyboard::getKeySequenceData(key)` (Keyboard.cpp:787).
    pub fn get_key_sequence_data(&self, key: u8) -> u32 {
        if plant(12327) {
            self.key_status[key as usize].sequence as u16 as u32
        } else {
            self.key_status[key as usize].sequence
        }
    }

    /// `Keyboard::setKeyStatusData(key, data)` (Keyboard.cpp:795).
    pub fn set_key_status_data(&mut self, key: u8, data: u8) {
        self.key_status[if plant(12329) { key.wrapping_add(1) } else { key } as usize].status = data;
    }

    /// `Keyboard::setKeyStateData(key, data)` (Keyboard.cpp:803): `data` is an `UnsignedByte` stored in the `state` short.
    pub fn set_key_state_data(&mut self, key: u8, data: u8) {
        if plant(12322) {
            self.key_status[key as usize].state |= data as u16;
        } else {
            self.key_status[key as usize].state = data as u16;
        }
    }

    /// `Keyboard::isShift()` (Keyboard.cpp:942): any shift state is pressed.
    pub fn is_shift(&self) -> bool {
        self.modifiers & KEY_STATE_LSHIFT != 0 || self.modifiers & KEY_STATE_RSHIFT != 0 || (self.modifiers & KEY_STATE_SHIFT2 != 0 && !plant(12323))
    }

    /// `Keyboard::isCtrl()` (Keyboard.cpp:954).
    pub fn is_ctrl(&self) -> bool {
        self.modifiers & KEY_STATE_LCONTROL != 0 || (self.modifiers & KEY_STATE_RCONTROL != 0 && !plant(12324))
    }

    /// `Keyboard::isAlt()` (Keyboard.cpp:966).
    pub fn is_alt(&self) -> bool {
        self.modifiers & KEY_STATE_LALT != 0 || self.modifiers & if plant(12325) { KEY_STATE_RSHIFT } else { KEY_STATE_RALT } != 0
    }
}
