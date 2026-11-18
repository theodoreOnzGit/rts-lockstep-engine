//! UNTESTED DRAFT (#202, #208): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameClient/DisplayStringManager.h
//!   GeneralsMD/Code/GameEngine/Source/GameClient/DisplayStringManager.cpp
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GameClientDispatch.cpp (see [`game_client_message_dispatcher`])
//!   GeneralsMD/Code/GameEngine/Source/GameClient/DrawableManager.cpp and Drawable/DrawableManager.cpp (no code: banner comments
//!   and `#include "PreRTS.h"` only; nothing to translate)
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GraphDraw.cpp: the whole file is under `#ifdef PERF_TIMERS` (not defined in the
//!   shipped build). Not translated.
//!
//! "Access for creating game managed display strings": the manager keeps a doubly linked list of every display string (it
//! tracks them, it does not own them). Upstream links through the string's `m_next`/`m_prev` pointers; here the links are the
//! string's index in the host's arena (`Option<usize>`), and the string's two link fields are [`DisplayStringLinks`].

use crate::common::message_stream::Disposition;
use crate::common::message_types::{MessageType, MSG_BEGIN_NETWORK_MESSAGES, MSG_CLEAR_GAME_DATA, MSG_END_NETWORK_MESSAGES, MSG_FRAME_TICK, MSG_NEW_GAME};

/// `DisplayString::m_next` / `m_prev`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DisplayStringLinks {
    /// `m_next`.
    pub next: Option<usize>,
    /// `m_prev`.
    pub prev: Option<usize>,
}

/// `class DisplayStringManager`'s list: `m_stringList` (the head) and `m_currentCheckpoint` (always NULL in this file).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisplayStringManager {
    /// `m_stringList`.
    pub string_list: Option<usize>,
    /// `m_currentCheckpoint`.
    pub current_checkpoint: Option<usize>,
}

impl DisplayStringManager {
    /// `DisplayStringManager::DisplayStringManager()`: `m_stringList = NULL; m_currentCheckpoint = NULL`.
    pub fn new() -> DisplayStringManager {
        DisplayStringManager { string_list: None, current_checkpoint: None }
    }

    /// `~DisplayStringManager()`: "we only keep track of the strings, we do NOT de-allocate them, our list better be cleaned out before
    /// we destroy ourselves": `assert(m_stringList == NULL)`.
    pub fn is_clean(&self) -> bool {
        self.string_list.is_none()
    }

    /// `DisplayStringManager::link(string)`: "Link a display string to the master list" at its head. The string must be unlinked
    /// (`assert(string->m_next == NULL)`, `assert(string->m_prev == NULL)`).
    pub fn link(&mut self, links: &mut [DisplayStringLinks], string: usize) {
        links[string].next = self.string_list;
        if let Some(head) = self.string_list {
            links[head].prev = Some(string);
        }
        self.string_list = Some(string);
    }

    /// `DisplayStringManager::unLink(string)`: "Unlink a display string from the master list"; the head moves on when the string was it
    /// (`assert(string == m_stringList)`).
    pub fn un_link(&mut self, links: &mut [DisplayStringLinks], string: usize) {
        if let Some(n) = links[string].next {
            links[n].prev = links[string].prev;
        }
        if let Some(p) = links[string].prev {
            links[p].next = links[string].next;
        } else {
            // assert( string == m_stringList );
            self.string_list = links[string].next;
        }
    }
}

/// `GameClientMessageDispatcher::translateGameMessage(msg)`: "the last translator on the message stream before the messages go to the
/// network for processing". Keeps the network messages (`MSG_BEGIN_NETWORK_MESSAGES..=MSG_END_NETWORK_MESSAGES`), `MSG_NEW_GAME`,
/// `MSG_CLEAR_GAME_DATA` and `MSG_FRAME_TICK`; destroys everything else.
pub fn game_client_message_dispatcher(msg_type: MessageType) -> Disposition {
    if msg_type >= MSG_BEGIN_NETWORK_MESSAGES && msg_type <= MSG_END_NETWORK_MESSAGES {
        return Disposition::Keep;
    }
    if msg_type == MSG_NEW_GAME || msg_type == MSG_CLEAR_GAME_DATA {
        return Disposition::Keep;
    }
    if msg_type == MSG_FRAME_TICK {
        return Disposition::Keep;
    }
    Disposition::Destroy
}
