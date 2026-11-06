// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/Audio/GameAudio.cpp (AudioManager::addAudioEvent: its early returns, the
//     order of generateFilename and generatePlayInfo)
//   GeneralsMD/Code/GameEngine/Source/Common/Audio/AudioEventRTS.cpp (generateFilename, generatePlayInfo: their
//     GameLogicRandomValue draws when m_isLogicalAudio is set)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #50: the logic's random draws of a **logical** audio event, made by the logic itself.
//!
//! # As upstream does it
//!
//! Only four script actions set `setIsLogicalAudio(true)` (ScriptActions.cpp: `doPlaySoundEffect`,
//! `doPlaySoundEffectAt`, `doSoundPlayFromNamed`, `doSpeechPlay`), each on an `AudioEventRTS` that is a **local** of the
//! action, so its `m_playingAudioIndex` is always the constructor's -1 (upstream's `setPlayingAudioIndex` copies the
//! pick back into that local, which is then destroyed: nothing persists between calls). `TheAudio->addAudioEvent` then:
//!
//! 1. returns at once for an empty name or `NoSound`, and for a name with no description (`AHSV_Error`): no draw;
//! 2. returns when the event's category is off (music: `isOn(Music)`; a sound effect: `isOn(Sound) && isOn(Sound3D)`;
//!    streaming: `isOn(Speech)`), or for streaming while `getDisallowSpeech()`: no draw;
//! 3. else calls `generateFilename()`, which for a sound effect with a non-empty `Sounds` and `Control = RANDOM` draws
//!    `GameLogicRandomValue(0, Sounds.size() - 1)` (**one draw even for a single file**: `GetGameLogicRandomValue`
//!    advances the state when `hi - lo + 1` is 1; it returns without a draw only for 0) and nothing otherwise (music and
//!    streaming name their `Filename`; an empty `Sounds` returns before the pick; without `RANDOM` the pick is
//!    `++m_playingAudioIndex`);
//! 4. then `generatePlayInfo()`, which, whatever the type and even after an empty `Sounds`, draws
//!    `GameLogicRandomValue(0, Attack.size() - 1)` if `Attack` is not empty, then `GameLogicRandomValue(0,
//!    Decay.size() - 1)` if `Decay` is not empty. The delay, pitch and volume shifts are always the audio stream's.
//!
//! The draws come before `shouldPlayLocally`, so every machine makes them, whoever the sound is for.
//!
//! # What this does
//!
//! [`logical_audio_draws`] makes exactly those logic draws, in that order, from the World's own generator, at the
//! script action (`World::sa_audio_add_event`). The values drawn travel with the event
//! ([`crate::logic::client_event::ScriptAudioEvent::AddEvent`]) and the client's `AudioEventRTS` uses each in place of
//! the draw it would make (`client::audio_event`): the client plays the logic's picks and never draws from a logic
//! stream of its own.
//!
//! # Where this differs from upstream (the client state the logic cannot read)
//!
//! - **Every audio category is on.** Upstream's `isOn` is each player's own setting (`-noaudio`, the options menu's
//!   sound and speech switches, `GameEngine::init`'s `setOn` calls, the debug `MSG_META_DEMO_TOGGLE_SOUND`). A player
//!   with a category off makes **none** of these draws and, upstream, desyncs from a player with it on the first time a
//!   script plays a random sound of that category. The logic here draws as an audio-on upstream does, on every machine.
//! - **`getDisallowSpeech()` is false.** Upstream sets it when an uninterruptable speech stream *opens* on the device
//!   (`MilesAudioManager::playAudioEvent`, only `if (stream)`) and clears it when the stream completes
//!   (`notifyOfAudioCompletion`): it depends on the device, the file being there and wall-clock playback, none of which
//!   is the same on every machine. The logic takes the state with no uninterruptable speech playing. Only a streaming
//!   (`DialogEvent`) description with an `Attack` or `Decay` list draws at all (`generateFilename` makes no draw for
//!   streaming), so the choice changes nothing for a dialog event without them.
//! - The descriptions are the logic's own read of the audio INI ([`crate::logic::world::statics::GameStatics::audio`]),
//!   the same files the client reads.

use crate::client::audio_info::{AudioTables, AudioType, AC_RANDOM};
use crate::common::random_value::RandomValues;

/// The values of a logical event's `GameLogicRandomValue` draws, in the order upstream makes them; `None` for a draw
/// not made. Each is the raw value the draw returned, before `generateFilename`'s "not the one played last" step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogicalAudioPicks {
    /// `generateFilename`'s `GameLogicRandomValue(0, m_sounds.size() - 1)`.
    pub sound: Option<i32>,
    /// `generatePlayInfo`'s `GameLogicRandomValue(0, attackSize - 1)`.
    pub attack: Option<i32>,
    /// `generatePlayInfo`'s `GameLogicRandomValue(0, decaySize - 1)`.
    pub decay: Option<i32>,
}

impl LogicalAudioPicks {
    /// The number of draws made.
    pub fn count(&self) -> usize {
        [self.sound, self.attack, self.decay].iter().filter(|p| p.is_some()).count()
    }
}

/// The logic draws of `TheAudio->addAudioEvent(&e)` for a logical event named `name` (see the module docs), from
/// `random`'s logic stream (`theGameLogicSeed`).
pub fn logical_audio_draws(tables: &AudioTables, name: &[u8], random: &mut RandomValues) -> LogicalAudioPicks {
    let mut picks = LogicalAudioPicks::default();
    // addAudioEvent: an empty name or "NoSound"
    if name.is_empty() || name == b"NoSound" {
        return picks;
    }
    // getInfoForAudioEvent: no description is AHSV_Error
    let Some(info) = tables.find_audio_event_info(name) else { return picks };
    // the category switches are all on, and getDisallowSpeech() is false (the module docs)
    // generateFilename()
    if info.sound_type == AudioType::SoundEffect && !info.sounds.is_empty() && info.control & AC_RANDOM != 0 {
        picks.sound = Some(random.game_logic_random_value(0, info.sounds.len() as i32 - 1));
    }
    // generatePlayInfo()
    if !info.attack_sounds.is_empty() {
        picks.attack = Some(random.game_logic_random_value(0, info.attack_sounds.len() as i32 - 1));
    }
    if !info.decay_sounds.is_empty() {
        picks.decay = Some(random.game_logic_random_value(0, info.decay_sounds.len() as i32 - 1));
    }
    picks
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &[u8] = b"\
AudioEvent Multi\n Sounds = a b c d\n Control = random\nEnd\n\
AudioEvent Single\n Sounds = solo\nEnd\n\
AudioEvent SingleRandom\n Sounds = solo\n Control = random\nEnd\n\
AudioEvent Seq\n Sounds = a b c\nEnd\n\
AudioEvent Full\n Sounds = a b\n Control = random\n Attack = x y z\n Decay = p q\nEnd\n\
AudioEvent Empty\n Control = random\n Decay = p q\nEnd\n\
DialogEvent Talk\n Filename = talk.wav\nEnd\n\
DialogEvent TalkAttack\n Filename = talk.wav\n Attack = x y\nEnd\n\
MusicTrack Song\n Filename = song.mp3\n Control = random\nEnd\n";

    fn tables() -> AudioTables {
        let mut t = AudioTables::new();
        t.load_text(b"t.ini", TABLE).expect("the table");
        t
    }

    /// *Predictions, written before the first run:* `Multi` (4 files, RANDOM) draws once, `GameLogicRandomValue(0, 3)`;
    /// `Single` (1 file, no RANDOM) and `Seq` (no RANDOM) draw nothing; `SingleRandom` (1 file, RANDOM) draws once (a
    /// range of one still advances the state upstream), the value 0; `Full` draws three times (sound 0..1, attack 0..2,
    /// decay 0..1) in that order; `Empty` (no Sounds) draws only its decay; `Talk` (streaming) and `Song` (music) draw
    /// nothing; `TalkAttack` draws its attack; an unknown name, `""` and `NoSound` draw nothing. Each value equals the
    /// one a second generator from the same state gives for the same ranges in the same order.
    #[test]
    fn a_logical_event_draws_upstreams_logic_values_in_upstreams_order() {
        let t = tables();
        let start = RandomValues::new(0x5eed);
        let draws = |name: &[u8]| {
            let mut r = start.clone();
            let p = logical_audio_draws(&t, name, &mut r);
            (p, r)
        };
        let expect = |ranges: &[i32]| {
            let mut r = start.clone();
            let v: Vec<i32> = ranges.iter().map(|&n| r.game_logic_random_value(0, n - 1)).collect();
            (v, r)
        };
        let (p, r) = draws(b"Multi");
        let (v, r2) = expect(&[4]);
        assert_eq!(p, LogicalAudioPicks { sound: Some(v[0]), attack: None, decay: None });
        assert_eq!(r, r2);
        for name in [&b"Single"[..], b"Seq", b"Talk", b"Song", b"Missing", b"", b"NoSound"] {
            let (p, r) = draws(name);
            assert_eq!(p.count(), 0, "{}", String::from_utf8_lossy(name));
            assert_eq!(r, start, "{}", String::from_utf8_lossy(name));
        }
        let (p, r) = draws(b"SingleRandom");
        assert_eq!(p.sound, Some(0));
        assert_ne!(r, start, "a range of one still draws");
        assert_eq!(r, expect(&[1]).1);
        let (p, r) = draws(b"Full");
        let (v, r2) = expect(&[2, 3, 2]);
        assert_eq!(p, LogicalAudioPicks { sound: Some(v[0]), attack: Some(v[1]), decay: Some(v[2]) });
        assert_eq!(r, r2);
        let (p, r) = draws(b"Empty");
        let (v, r2) = expect(&[2]);
        assert_eq!(p, LogicalAudioPicks { sound: None, attack: None, decay: Some(v[0]) });
        assert_eq!(r, r2);
        let (p, r) = draws(b"TalkAttack");
        let (v, r2) = expect(&[2]);
        assert_eq!(p, LogicalAudioPicks { sound: None, attack: Some(v[0]), decay: None });
        assert_eq!(r, r2);
    }
}
