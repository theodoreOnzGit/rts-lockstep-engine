// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/Audio/GameAudio.cpp (AudioManager::getAudioLengthMS)
//   GeneralsMD/Code/GameEngineDevice/Source/MilesAudioDevice/MilesAudioManager.cpp (MilesAudioManager::getFileLengthMS)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #52 (the owner's option 2): `TheAudio->getAudioLengthMS(&event)` as the logic reads it, from the game's own
//! files, made at load and the same on every machine.
//!
//! # As upstream does it
//!
//! `ScriptEngine::isSpeechComplete` and `isAudioComplete` (ScriptEngine.cpp:7292, 7327), the first time a script asks
//! about an event, store `frame + REAL_TO_UNSIGNEDINT(getAudioLengthMS(&event) / MSEC_PER_LOGICFRAME_REAL)` and answer
//! true from that frame on. `AudioManager::getAudioLengthMS` (GameAudio.cpp:944) copies a fresh, non-logical
//! `AudioEventRTS(name)`, calls `generateFilename()` and `generatePlayInfo()` on the copy, and returns
//! `getFileLengthMS(attack) + getFileLengthMS(sound) + getFileLengthMS(decay)` (0 for an event with no description).
//! `MilesAudioManager::getFileLengthMS` (MilesAudioManager.cpp:2595) is 0 for an empty name or a file Miles cannot open
//! as a stream, else `INT_TO_REAL` of the total that `AIL_stream_ms_position` writes, an integer of milliseconds.
//!
//! # The fixed choice: the first file of each list
//!
//! Upstream's copy is not logical, so `generateFilename` picks a `Control = RANDOM` sound effect's file, and
//! `generatePlayInfo` the attack and decay files, with **`GameAudioRandomValue`**, each machine's own audio stream: a
//! script waiting on such an event can end on different frames on different machines (upstream desyncs there). The logic
//! must not draw from the logic stream either, because upstream does not, and a draw would move every later logic value.
//! So the logic makes one fixed choice per list: **index 0**, the first file of `Sounds`, `Attack` and `Decay`. It is the
//! file upstream itself picks for a sound effect without `RANDOM` (`(++m_playingAudioIndex) % size` from the
//! constructor's -1 is 0), and for music and dialog there is only `Filename`; it needs no generator, is the same on every
//! machine, and is one of upstream's possible answers for every event. It is made by running the translated
//! `AudioEventRTS::generateFilename`/`generatePlayInfo` ([`crate::client::audio_event::AudioEventRts`]) with each draw's
//! value fixed at 0, so the paths (prefix, extension, `adjustForLocalization`) are the client's own.
//!
//! # The lengths: what Miles is known to do, and what is not
//!
//! Miles was never released; its documentation describes `AIL_stream_ms_position(stream, &total, &current)` as the
//! stream's total length and position in milliseconds, as `S32`s. What is taken here:
//!
//! - **WAV** (PCM and IMA ADPCM, the formats the game's sounds and speech use), from [`crate::formats::wav`]: the sample
//!   frames the `data` chunk holds (exactly what the decoder yields, the short last ADPCM block included) times 1000
//!   over the sample rate, **truncated** to whole milliseconds. Not known: whether Miles rounds rather than truncates
//!   (at most 1 ms per file), and whether it counts an ADPCM file by its samples or by `data bytes * 1000 /
//!   nAvgBytesPerSec` (for a 22050 Hz mono file in 1024-byte blocks those differ by under 1 ms per second of sound; the game's own
//!   files' formats were not checked here, no install being at hand).
//! - **MP3** (the music), from [`crate::formats::mp3`]: a Xing/`Info` or `VBRI` header's frame count when the first
//!   frame has one, else the audio bytes (after an ID3v2 tag, before an ID3v1 tag) over the first frame's bit rate,
//!   truncated. Upstream's comment says the stream is opened "to get the file info without actually opening the file",
//!   which reads as Miles answering from the stream's header rather than by decoding it, so a header-only length is
//!   the closer model; for a constant-bit-rate file, as the game's 2003 music is expected to be, it agrees with a count
//!   of every frame ([`crate::formats::mp3::mp3_walk`]) to within the padding bytes' share (11 ms in 7.8 s at 128
//!   kbit/s, in the test). Not known: whether Miles believes a Xing header, or estimates a VBR file from its first
//!   frame's bit rate.
//! - Anything else (a file not found, a format neither reader takes) is 0, as a failed `AIL_open_stream` is.
//!
//! A difference of a few milliseconds moves a wait by a frame only when the total crosses a multiple of 33.33 ms.
//!
//! # Reading only the headers
//!
//! The audio archives run to hundreds of megabytes and on the web are read by range. Each file's length needs only its
//! size and a few header bytes: [`file_length_ms`] reads a 512-byte prefix (enough for a WAV's RIFF, `fmt ` and `data`
//! headers, and usually an MP3's first frame), and only what lies beyond it (a long chunk list, an ID3v2 tag, an
//! ID3v1 tag at the end) by further small reads. No file is read whole: the frame walk is not used for the logic.

use std::collections::BTreeMap;

use crate::client::audio_event::{AudioEventHost, AudioEventRts};
use crate::client::audio_info::AudioTables;
use crate::common::base_type::Coord3D;
use crate::common::random_value::RandomValues;
use crate::logic::logical_audio::LogicalAudioPicks;
use crate::logic::world::game_data::GameDataFiles;

/// The bytes read from each file's start before any other read.
const PREFIX: usize = 512;

/// The three files `getAudioLengthMS` sums for the event `name` under the fixed choice (see the module docs):
/// `(attack, sound, decay)`, each empty when the event has none; `None` for an event with no description.
pub fn event_files(tables: &AudioTables, name: &[u8], files: &impl GameDataFiles, language: &[u8]) -> Option<[Vec<u8>; 3]> {
    let info = tables.find_audio_event_info(name)?.clone();
    struct Host<'a, F: GameDataFiles> {
        files: &'a F,
        language: &'a [u8],
    }
    impl<F: GameDataFiles> AudioEventHost for Host<'_, F> {
        fn does_file_exist(&self, path: &[u8]) -> bool {
            self.files.file_exists(path)
        }
        fn registry_language(&self) -> Vec<u8> {
            self.language.to_vec()
        }
        fn object_position(&self, _: u32) -> Option<Coord3D> {
            None
        }
        fn drawable_position(&self, _: u32) -> Option<Coord3D> {
            None
        }
        fn object_player_index(&self, _: u32) -> Option<i32> {
            None
        }
        fn drawable_player_index(&self, _: u32) -> Option<i32> {
            None
        }
    }
    let host = Host { files, language };
    let mut e = AudioEventRts::new(name);
    e.event_info = Some(info);
    // every draw's value fixed at 0 (the client's logical-event path takes a value given; the generator below only
    // feeds the delay, pitch and volume, which a length does not read, and is thrown away)
    e.is_logical_audio = true;
    e.logic_picks = LogicalAudioPicks { sound: Some(0), attack: Some(0), decay: Some(0) };
    let mut scratch = RandomValues::new(0);
    e.generate_filename(&tables.settings, &host, &mut scratch);
    e.generate_play_info(&tables.settings, &host, &mut scratch);
    Some([e.attack_name, e.filename_to_load, e.decay_name])
}

/// `MilesAudioManager::getFileLengthMS(path)` (see the module docs): 0 for an empty path, a missing file or one neither
/// reader takes; else the whole milliseconds from the file's headers.
pub fn file_length_ms(files: &impl GameDataFiles, path: &[u8]) -> u32 {
    if path.is_empty() {
        return 0;
    }
    let Some(size) = files.file_size(path) else { return 0 };
    let prefix = files.read_file_range(path, 0, PREFIX.min(size));
    if prefix.len() < PREFIX.min(size) {
        // the prefix is not there (yet, on a ranged device): asking for the MP3 reader's ranges of what may be a WAV
        // would fetch 4 KiB for nothing (the browser's first pass did, before this check)
        return 0;
    }
    let read = |o: usize, l: usize| -> Vec<u8> {
        let end = o.saturating_add(l).min(size);
        if o >= end {
            return Vec::new();
        }
        if end <= prefix.len() {
            return prefix[o..end].to_vec();
        }
        files.read_file_range(path, o, end - o)
    };
    if prefix.len() >= 12 && &prefix[0..4] == b"RIFF" && &prefix[8..12] == b"WAVE" {
        return crate::formats::wav::parse_wav_ranged(size, read).map_or(0, |i| i.length_ms());
    }
    crate::formats::mp3::mp3_info_ranged(size, read).map_or(0, |i| i.header_length_ms())
}

/// `getAudioLengthMS` of every described event, at load: `INT_TO_REAL(attack) + INT_TO_REAL(sound) + INT_TO_REAL(decay)`
/// in upstream's order, under the fixed choice. Events of length 0 are left out (the lookup answers 0 for them, as for
/// a name with no description). Each file is read once.
pub fn audio_lengths_ms(tables: &AudioTables, files: &impl GameDataFiles, language: &[u8]) -> BTreeMap<Vec<u8>, f32> {
    let mut per_file: BTreeMap<Vec<u8>, u32> = BTreeMap::new();
    let mut out = BTreeMap::new();
    for name in tables.infos.keys() {
        let Some(paths) = event_files(tables, name, files, language) else { continue };
        let mut ms = [0f32; 3];
        for (m, p) in ms.iter_mut().zip(&paths) {
            let len = *per_file.entry(p.clone()).or_insert_with(|| file_length_ms(files, p));
            *m = len as f32;
        }
        let total = ms[0] + ms[1] + ms[2];
        if total != 0.0 {
            out.insert(name.clone(), total);
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::logic::world::game_data::MemoryFiles;

    pub(crate) const TABLE: &[u8] = b"\
AudioSettings\n AudioRoot = Data\\Audio\n SoundsFolder = Sounds\n MusicFolder = Tracks\n StreamingFolder = Speech\n SoundsExtension = wav\nEnd\n\
AudioEvent Boom\n Sounds = boomA boomB\n Control = random\n Attack = attA attB\n Decay = decA\nEnd\n\
AudioEvent Quiet\n Sounds = missing\nEnd\n\
DialogEvent Talk\n Filename = talk.wav\nEnd\n\
MusicTrack Song\n Filename = song.mp3\nEnd\n";

    fn pcm(ms: u32) -> Vec<u8> {
        // 8-bit mono at 8000 Hz: 8 samples per millisecond
        crate::formats::wav::tests::wav(1, 1, 8000, 1, 8, None, &vec![128u8; 8 * ms as usize])
    }

    pub(crate) fn files() -> MemoryFiles {
        let mut f = MemoryFiles::new();
        f.insert(b"Data\\Audio\\Sounds\\boomA.wav", pcm(400));
        f.insert(b"Data\\Audio\\Sounds\\boomB.wav", pcm(9000));
        f.insert(b"Data\\Audio\\Sounds\\attA.wav", pcm(100));
        f.insert(b"Data\\Audio\\Sounds\\attB.wav", pcm(9000));
        f.insert(b"Data\\Audio\\Sounds\\decA.wav", pcm(50));
        // the speech has a localized copy, which wins (adjustForLocalization)
        f.insert(b"Data\\Audio\\Speech\\talk.wav", pcm(10));
        f.insert(b"Data\\Audio\\Speech\\English\\\\talk.wav", pcm(1234));
        f.insert(b"Data\\Audio\\Tracks\\song.mp3", crate::formats::mp3::tests::cbr_file(300));
        f
    }

    pub(crate) fn tables() -> AudioTables {
        let mut t = AudioTables::new();
        t.load_text(b"t.ini", TABLE).expect("the table");
        t
    }

    /// *Predictions, written before the first run:* `Boom`'s files are the first of each list, `attA`, `boomA`, `decA`
    /// under `Data\Audio\Sounds\` with `.wav`; its length 100 + 400 + 50 = 550 ms whatever the generators hold.
    /// `Quiet`'s sound is missing: 0, left out of the table. `Talk` takes the localized file (with upstream's doubled
    /// separator, as the client's path is): 1234 ms. `Song` is 300 CBR frames at 128 kbit/s: 7825 ms (the header
    /// estimate of `formats::mp3`'s test). An event with no description is not in the table.
    #[test]
    fn each_event_is_the_first_file_of_each_list_summed() {
        let (t, f) = (tables(), files());
        let p = event_files(&t, b"Boom", &f, b"English").unwrap();
        assert_eq!(p, [b"Data\\Audio\\Sounds\\attA.wav".to_vec(), b"Data\\Audio\\Sounds\\boomA.wav".to_vec(), b"Data\\Audio\\Sounds\\decA.wav".to_vec()]);
        assert_eq!(event_files(&t, b"Nothing", &f, b"English"), None);
        let l = audio_lengths_ms(&t, &f, b"English");
        assert_eq!(l.get(&b"Boom"[..]), Some(&550.0));
        assert_eq!(l.get(&b"Quiet"[..]), None);
        assert_eq!(l.get(&b"Talk"[..]), Some(&1234.0));
        assert_eq!(l.get(&b"Song"[..]), Some(&7825.0));
        assert_eq!(l.len(), 3);
        assert_eq!(file_length_ms(&f, b""), 0);
        assert_eq!(file_length_ms(&f, b"Data\\Audio\\Sounds\\none.wav"), 0);
    }

    /// The browser's way (GitHub #52): the same files in an archive on a [`RangedDevice`](crate::common::file_system::RangedDevice)
    /// that holds only the archive's directory, the ranges asked for handed back between passes, as the page does.
    /// *Predictions, written before the first run:* the first pass serves no header and lists ranges; within four
    /// passes nothing more is wanted, and the lengths then equal those read from memory; the bytes fetched are a few
    /// kilobytes, far below the members' ~150 KB (each WAV's 512-byte prefix and the MP3's prefix, 4 KiB window and tail).
    #[test]
    fn the_headers_are_read_by_range_from_an_archive() {
        use crate::common::file_system::{FileSystem, LocalDevice, RangedDevice};
        use crate::formats::big::{build_big, ArchiveFile};
        let mem = files();
        let names: [&[u8]; 8] = [
            b"Data\\Audio\\Sounds\\boomA.wav",
            b"Data\\Audio\\Sounds\\boomB.wav",
            b"Data\\Audio\\Sounds\\attA.wav",
            b"Data\\Audio\\Sounds\\attB.wav",
            b"Data\\Audio\\Sounds\\decA.wav",
            b"Data\\Audio\\Speech\\talk.wav",
            b"Data\\Audio\\Speech\\English\\talk.wav",
            b"Data\\Audio\\Tracks\\song.mp3",
        ];
        let bodies: Vec<Vec<u8>> = names.iter().map(|n| mem.open(n).unwrap_or_else(|| mem.open(b"Data\\Audio\\Speech\\English\\\\talk.wav").unwrap())).collect();
        let members: Vec<(&[u8], &[u8])> = names.iter().zip(&bodies).map(|(n, b)| (*n, b.as_slice())).collect();
        let big = build_big(&members);
        let mut dev = RangedDevice::new();
        let keep = ArchiveFile::directory_prefix_needed(big.len(), &big);
        dev.insert_ranged(b"AudioZH.big", big.len(), 7, big[..keep].to_vec());
        let mut fs = FileSystem::new(dev);
        fs.init(None);
        let mut passes = 0;
        loop {
            passes += 1;
            let before = fs.local.device.unserved_reads();
            let lengths = audio_lengths_ms(&tables(), &fs, b"English");
            if fs.local.device.unserved_reads() == before {
                // (the archive holds the localized speech at its single-separator path: the archive's tokens must find
                // it for the client's doubled one, as for the memory files' doubled path, so `Talk` is 1234 both ways)
                assert_eq!(lengths, audio_lengths_ms(&tables(), &mem, b"English"));
                break;
            }
            assert!(passes < 4, "pass {passes}");
            for (k, o, l) in fs.local.device.take_wanted() {
                assert_eq!(k, 7);
                fs.local.device.provide(k, o, big[o..o + l].to_vec());
            }
        }
        let fetched = fs.local.device.stats().bytes_arrived;
        assert!(fetched < 16 * 1024, "{fetched} bytes fetched of {}", big.len());
    }
}
