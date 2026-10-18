// SPDX-License-Identifier: GPL-3.0-only
//
// Shape of the events that game logic hands to the client (the cut point of
// docs/phase1-porting-graph.md, "Cut points: the client interface"). The
// client-facing singletons upstream (`TheAudio`, `TheInGameUI`, ...) are not
// reachable from logic; logic pushes one of these onto a queue it owns, and
// the client drains the queue after each logic frame.
//
// Translation-owned file (this repository's own design, not upstream code):
// it names the upstream calls each variant replaces, and nothing else.

//! Events that logic emits for the client. Only the variants needed by the
//! layer-4 static data are defined here (the others arrive with their
//! callers); each names the upstream call it replaces.

/// Which of `MiscAudio`'s money sounds an event asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MoneySound {
    /// `MiscAudio::m_moneyDepositSound`.
    Deposit,
    /// `MiscAudio::m_moneyWithdrawSound`.
    Withdraw,
}

/// One event for the client.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClientEvent {
    /// `TheAudio->addAudioEvent(&event)`, where `event` is the money sound
    /// of `MiscAudio` with `setPlayerIndex(player_index)`. The client looks
    /// up the sound's name in its own `MiscAudio`, so logic never holds a
    /// `AudioEventRTS`.
    MoneySound { sound: MoneySound, player_index: i32 },
}
