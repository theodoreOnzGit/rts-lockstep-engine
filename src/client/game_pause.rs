// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp (setGamePaused: its side effects, through the
//     logic's own translation, `crate::logic::world::game_logic_update::set_game_paused`)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/GameLogic.h (setGamePaused's `pauseMusic = TRUE` default)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GameClient.cpp (update: `freezeTime` while the game is paused)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplay.cpp (draw: `syncTime` held while frozen)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The client's side of `GameLogic::setGamePaused` (GitHub #108): the input, the mouse, the audio and the drawables'
//! time, which the quit menu's pause (`crate::client::quit_menu`, [`crate::client::shell::ShellRequest::SetGamePaused`])
//! reaches in the game.
//!
//! # As upstream reads it
//!
//! `setGamePaused(paused, pauseMusic)` does nothing when the game already is (or is not) paused. Pausing remembers
//! `TheInGameUI->getInputEnabled()` and `TheMouse->getVisibility()`, makes the mouse visible with the arrow cursor,
//! disables the input if it was enabled, and pauses the audio: all of it with `pauseMusic` (the default, which the quit
//! menu uses), else all but the music. Resuming restores the mouse's visibility, enables the input again if it was
//! enabled before, and resumes the same audio. The logic's translation is reused as it is
//! (`crate::logic::world::game_logic_update::set_game_paused`), over this module's [`GamePauseHost`].
//!
//! While the game is paused, `GameClient::update`'s and `W3DDisplay::draw`'s `freezeTime` is set: the drawables are
//! not updated and W3D's `syncTime` does not advance, so the models' animations and the W3D particles stand still
//! ([`ClientPause::sync_time`]).
//!
//! # Upstream quirks that are kept
//!
//! - The early return when the state does not change: an unpause while not paused restores nothing.
//! - The input is restored only if it was enabled when the game paused, whatever happened to it since.
//!
//! # Where this differs from upstream
//!
//! - `syncTime` advances by the wall clock's elapsed milliseconds while not frozen, not by a constant
//!   `TheW3DFrameLengthInMsec` a draw (the client's draw clock was already the wall clock).
//! - `TheMouse`: the client draws no cursor of its own (the system's arrow is always shown), so [`ClientMouse`] keeps
//!   the visibility and the cursor `setGamePaused` sets, for its memory and its tests, and nothing is drawn from it.
//! - `TheInGameUI->setInputEnabled` sets the client's InGameUI flag; the client's translators do not read it yet
//!   (upstream's `SelectionTranslator`, `CommandTranslator`, `LookAtTranslator` and `WindowTranslator` do): while the
//!   quit menu is up the app keeps the pointer from them instead.
//! - The paused state is the client's `Game::paused`, which the client's own panels (saves, campaigns, the shell over
//!   the install's game) also set without these side effects; [`ClientPause::set_game_paused`] starts from it.
//!
//! # Representation
//!
//! [`ClientPause`] holds `GameLogic`'s members `setGamePaused` keeps (a [`GameLogicState`]: `m_gamePaused`,
//! `m_inputEnabledMemory`, `m_mouseVisibleMemory`; its other members are not used here), the [`ClientMouse`] and the
//! drawables' clock. The host ([`GamePauseHost`]) is made for each call over the client's InGameUI, the mouse and, once
//! it is up, the audio manager and its device. The simulation is never touched: nothing here reads or writes the `World`.

use crate::client::game_audio::{AudioDevice, AudioManager};
use crate::client::in_game_ui::InGameUiSelection;
use crate::logic::world::game_logic_update::{set_game_paused, GameLogicState, GamePauseHost};

/// `TheMouse`'s state `setGamePaused` reads and sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientMouse {
    /// `getVisibility()` / `setVisibility(on)`.
    pub visible: bool,
    /// Whether `setCursor(Mouse::ARROW)` was the last cursor set.
    pub arrow: bool,
}

impl Default for ClientMouse {
    fn default() -> Self {
        ClientMouse { visible: true, arrow: false }
    }
}

/// The client's pause: `setGamePaused`'s members, the mouse, and the drawables' clock (`syncTime`).
#[derive(Clone, Debug)]
pub struct ClientPause {
    /// `GameLogic`'s `m_gamePaused`, `m_inputEnabledMemory`, `m_mouseVisibleMemory` (the other members unused).
    pub logic: GameLogicState,
    pub mouse: ClientMouse,
    /// The wall clock at the last [`ClientPause::sync_time`] and the drawables' time then (milliseconds).
    clock: Option<(u32, u32)>,
}

impl Default for ClientPause {
    fn default() -> Self {
        ClientPause::new()
    }
}

/// The host of one `setGamePaused` call: the client's InGameUI, mouse and audio.
struct PauseTarget<'a, D: AudioDevice> {
    ui: &'a mut InGameUiSelection,
    mouse: &'a mut ClientMouse,
    audio: Option<(&'a mut AudioManager, &'a mut D)>,
}

impl<D: AudioDevice> GamePauseHost for PauseTarget<'_, D> {
    fn u_ui_input_enabled(&self) -> bool {
        self.ui.input_enabled()
    }
    fn u_ui_set_input_enabled(&mut self, on: bool) {
        self.ui.set_input_enabled(on);
    }
    fn u_mouse_visible(&self) -> bool {
        self.mouse.visible
    }
    fn u_mouse_set_visible(&mut self, on: bool) {
        self.mouse.visible = on;
    }
    fn u_mouse_set_arrow_cursor(&mut self) {
        self.mouse.arrow = true;
    }
    fn u_audio_pause(&mut self, affect: i32) {
        if let Some((m, d)) = self.audio.as_mut() {
            m.pause_audio(&mut **d, affect as u32);
        }
    }
    fn u_audio_resume(&mut self, affect: i32) {
        if let Some((m, d)) = self.audio.as_mut() {
            m.resume_audio(&mut **d, affect as u32);
        }
    }
}

impl ClientPause {
    /// As `GameLogic::GameLogic()` leaves the members (not paused, input and mouse remembered as on).
    pub fn new() -> Self {
        ClientPause { logic: GameLogicState::new(), mouse: ClientMouse::default(), clock: None }
    }

    /// `isGamePaused()`.
    pub fn is_game_paused(&self) -> bool {
        self.logic.game_paused
    }

    /// `setGamePaused(paused, pauseMusic)` from the client's paused state `was_paused` (`Game::paused`), applied to its
    /// InGameUI, its mouse and (once it is up) its audio; returns the new paused state.
    pub fn set_game_paused<D: AudioDevice>(
        &mut self,
        was_paused: bool,
        paused: bool,
        pause_music: bool,
        ui: &mut InGameUiSelection,
        audio: Option<(&mut AudioManager, &mut D)>,
    ) -> bool {
        self.logic.game_paused = was_paused;
        let mut host = PauseTarget { ui, mouse: &mut self.mouse, audio };
        set_game_paused(&mut self.logic, paused, pause_music, &mut host);
        self.logic.game_paused
    }

    /// W3D's `syncTime` for a draw at wall clock `wall_ms`: it advances by the wall's elapsed time unless `frozen`
    /// (`freezeTime`: the game paused), when it stands still. The first call starts it at the wall clock.
    pub fn sync_time(&mut self, wall_ms: u32, frozen: bool) -> u32 {
        let sync = match self.clock {
            None => wall_ms,
            Some((_, sync)) if frozen => sync,
            Some((last, sync)) => sync.wrapping_add(wall_ms.wrapping_sub(last)),
        };
        self.clock = Some((wall_ms, sync));
        sync
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::audio_event::AudioEventRts;
    use crate::client::audio_info::AudioTables;
    use crate::client::game_audio::{AudioRequest, PlayingAudio, PlayingKind, RequestType, VoiceId, VoiceStart};

    #[derive(Default)]
    struct Paused(Vec<(VoiceId, bool)>);
    impl AudioDevice for Paused {
        fn start(&mut self, _: &[u8], _: VoiceStart) -> Option<VoiceId> {
            None
        }
        fn set_gain_pan(&mut self, _: VoiceId, _: f32, _: f32) {}
        fn stop(&mut self, _: VoiceId) {}
        fn set_paused(&mut self, v: VoiceId, p: bool) {
            self.0.push((v, p));
        }
        fn take_finished(&mut self) -> Vec<VoiceId> {
            Vec::new()
        }
        fn file_length_ms(&self, _: &[u8]) -> f32 {
            0.0
        }
    }

    fn sound(voice: VoiceId) -> PlayingAudio {
        PlayingAudio {
            event: Box::new(AudioEventRts::default()),
            kind: PlayingKind::Sample(None),
            voice: Some(voice),
            stopped: false,
            request_stop: false,
            frames_faded: 0,
            cleanup: false,
            loops_completed: 0,
        }
    }

    /// #108, predicted from GameLogic.cpp's `setGamePaused` before the run:
    /// - pause (input on, mouse visible): paused; input off; mouse visible with the arrow; the playing sound's voice
    ///   paused and the play request dropped;
    /// - pause again: nothing (the early return: no second `set_paused` on the device);
    /// - resume: not paused; input on; the voice resumed;
    /// - pause with the input already off: it stays off, and the resume leaves it off (`m_inputEnabledMemory`);
    /// - without audio, the input and the mouse still change;
    /// - the drawables' clock: 1,000 at the first draw, 1,040 40 ms later, held at 1,040 while frozen (two draws, 1,100
    ///   and 1,500 on the wall), then 1,060 at wall 1,520 (only the unfrozen 20 ms count).
    ///
    /// Results (2026-11-10): all held.
    #[test]
    fn the_pause_reaches_the_input_the_mouse_the_audio_and_the_time() {
        let mut p = ClientPause::new();
        let mut ui = InGameUiSelection::new();
        let mut m = AudioManager::new(AudioTables::default());
        let mut d = Paused::default();
        m.playing_sounds.push(sound(7));
        m.requests.push(AudioRequest { request: RequestType::Play, pending: Err(3), requires_check_for_sample: false });
        assert!(p.set_game_paused(false, true, true, &mut ui, Some((&mut m, &mut d))));
        assert!(!ui.input_enabled());
        assert_eq!(p.mouse, ClientMouse { visible: true, arrow: true });
        assert_eq!(d.0, [(7, true)]);
        assert!(m.requests.is_empty());
        assert!(p.set_game_paused(true, true, true, &mut ui, Some((&mut m, &mut d))));
        assert_eq!(d.0, [(7, true)]);
        assert!(!p.set_game_paused(true, false, true, &mut ui, Some((&mut m, &mut d))));
        assert!(ui.input_enabled());
        assert_eq!(d.0, [(7, true), (7, false)]);
        ui.set_input_enabled(false);
        p.set_game_paused(false, true, true, &mut ui, None::<(&mut AudioManager, &mut Paused)>);
        p.set_game_paused(true, false, true, &mut ui, None::<(&mut AudioManager, &mut Paused)>);
        assert!(!ui.input_enabled());
        p.mouse.visible = false;
        ui.set_input_enabled(true);
        p.set_game_paused(false, true, false, &mut ui, None::<(&mut AudioManager, &mut Paused)>);
        assert!(p.mouse.visible && !ui.input_enabled() && p.is_game_paused());
        p.set_game_paused(true, false, false, &mut ui, None::<(&mut AudioManager, &mut Paused)>);
        assert!(!p.mouse.visible && ui.input_enabled());
        assert_eq!(p.sync_time(1000, false), 1000);
        assert_eq!(p.sync_time(1040, false), 1040);
        assert_eq!(p.sync_time(1100, true), 1040);
        assert_eq!(p.sync_time(1500, true), 1040);
        assert_eq!(p.sync_time(1520, false), 1060);
    }
}
