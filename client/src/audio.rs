// SPDX-License-Identifier: GPL-3.0-only
//! The audio device (GitHub #13, C6): the sounds the translated audio manager starts
//! (`rts_lockstep_engine::client::game_audio`), played from the game's own files.
//!
//! **New code.** Upstream's device is Miles (`MilesAudioManager`), which was never released; the manager above it is
//! translated in the engine crate and decides what plays, on which channel, how loud and where. This module only turns
//! its voices into sound:
//!
//! - [`ClientDevice`] implements the manager's `AudioDevice`: a voice is started on a file name; the file is read from
//!   the art's file system (`AudioZH.big`, `SpeechZH.big`, `MusicZH.big`; on the web, read by range from the player's
//!   files as the art is), decoded, handed to the output and played; its end is reported back to the manager from the
//!   output's clock. A voice whose file is not there ends at once (upstream: the sample gets no file and is released).
//!   A voice whose file is still arriving waits up to [`MAX_WAIT_S`] seconds, then is dropped as ended.
//! - **Decoding is spread over frames** ([`ClientDevice::pump`]): WAV files (PCM and IMA ADPCM, the game's) by the
//!   engine's `formats::wav` decoder, at most [`DECODE_BUDGET`] bytes of input per frame; MP3 files (the music, some
//!   speech) are handed to the output whole, and the browser decodes them off the main thread (`decodeAudioData`).
//! - [`AudioOutput`] is what plays: [`NullOutput`] natively (silent: it keeps the clock and the lengths, so the manager
//!   runs exactly as with sound; no native backend is chosen yet, see the module's issue notes), and `WebOutput` in the
//!   browser (the Web Audio API through `web-sys`: one `AudioBufferSourceNode` -> `GainNode` -> `StereoPannerNode` per
//!   voice). The browser's `AudioContext` is the one the page made in the click that imported the files
//!   (`window.rtsAudioContext`), so it may start (browsers refuse audio before a user gesture).
//!
//! Positional gain and pan follow the camera: [`listener_view`] places the manager's microphone from the camera rig, and
//! the manager recomputes each 3-D voice's gain and pan every logic frame. Nothing here touches the `World`; the logic's
//! CRCs are unchanged with the audio attached (`drawables`' FX gun test).

use std::collections::BTreeMap;

use rts_lockstep_engine::client::audio_info::AudioTables;
use rts_lockstep_engine::client::game_audio::{AudioDevice, ListenerView, VoiceId, VoiceStart};
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::formats::wav::WavDecoder;
use rts_lockstep_engine::ww::wwmath;

/// Input bytes of WAV decoded per frame at most (about 0.2 ms of work in a release build).
pub const DECODE_BUDGET: usize = 256 * 1024;
/// How long a voice waits for its file, seconds.
pub const MAX_WAIT_S: f64 = 2.0;

/// What plays the sounds.
pub trait AudioOutput {
    /// A decoded sound: interleaved 16-bit samples.
    fn upload_pcm(&mut self, path: &[u8], channels: u16, rate: u32, samples: &[i16]);
    /// An encoded sound (MP3) the output decodes itself; `false` if it cannot.
    fn upload_encoded(&mut self, path: &[u8], bytes: Vec<u8>) -> bool;
    /// `None` while being decoded; `Some(None)` failed; `Some(Some(seconds))` ready, with its length.
    fn sound_ready(&mut self, path: &[u8]) -> Option<Option<f64>>;
    fn play(&mut self, voice: VoiceId, path: &[u8], gain: f32, pan: f32, rate: f32);
    fn set(&mut self, voice: VoiceId, gain: f32, pan: f32);
    fn stop(&mut self, voice: VoiceId);
    fn pause(&mut self, voice: VoiceId, paused: bool);
    /// The output's clock, seconds.
    fn now(&self) -> f64;
    /// The application's frame time (the null output's clock).
    fn advance(&mut self, _dt: f64) {}
}

/// No sound: lengths and a clock only.
#[derive(Default, Debug)]
pub struct NullOutput {
    pub clock: f64,
    pub lengths: BTreeMap<Vec<u8>, f64>,
    pub playing: BTreeMap<VoiceId, (Vec<u8>, f32, f32)>,
    pub plays: u64,
}

impl AudioOutput for NullOutput {
    fn upload_pcm(&mut self, path: &[u8], channels: u16, rate: u32, samples: &[i16]) {
        let frames = samples.len() / usize::from(channels.max(1));
        self.lengths.insert(path.to_vec(), frames as f64 / f64::from(rate.max(1)));
    }
    fn upload_encoded(&mut self, _: &[u8], _: Vec<u8>) -> bool {
        false
    }
    fn sound_ready(&mut self, path: &[u8]) -> Option<Option<f64>> {
        Some(self.lengths.get(path).copied())
    }
    fn play(&mut self, voice: VoiceId, path: &[u8], gain: f32, pan: f32, _: f32) {
        self.plays += 1;
        self.playing.insert(voice, (path.to_vec(), gain, pan));
    }
    fn set(&mut self, voice: VoiceId, gain: f32, pan: f32) {
        if let Some(v) = self.playing.get_mut(&voice) {
            (v.1, v.2) = (gain, pan);
        }
    }
    fn stop(&mut self, voice: VoiceId) {
        self.playing.remove(&voice);
    }
    fn pause(&mut self, _: VoiceId, _: bool) {}
    fn now(&self) -> f64 {
        self.clock
    }
    fn advance(&mut self, dt: f64) {
        self.clock += dt;
    }
}

/// A file read by [`ClientDevice::pump`]'s reader.
pub enum Fetch {
    /// Still arriving (asked for).
    Waiting,
    /// Not in the game's files.
    Missing,
    Bytes(Vec<u8>),
}

enum FileState {
    Wanted,
    Decoding(Box<WavDecoder>, Vec<u8>),
    Uploaded,
    Ready(f64),
    Missing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum VoiceState {
    Waiting { since: f64 },
    Playing { ends: f64 },
    Paused { left: f64 },
}

struct Voice {
    file: Vec<u8>,
    start: VoiceStart,
    gain: f32,
    pan: f32,
    state: VoiceState,
    paused: bool,
}

/// What the device has done, for the status line and checks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceStats {
    pub voices_started: u64,
    pub voices_played: u64,
    pub voices_missing: u64,
    pub voices_timed_out: u64,
    pub files_decoded: u64,
    pub bytes_decoded: u64,
}

/// The manager's device: voices, the files they need, and an output.
pub struct ClientDevice<O: AudioOutput> {
    pub out: O,
    next: VoiceId,
    voices: BTreeMap<VoiceId, Voice>,
    finished: Vec<VoiceId>,
    files: BTreeMap<Vec<u8>, FileState>,
    pub stats: DeviceStats,
}

impl<O: AudioOutput> ClientDevice<O> {
    pub fn new(out: O) -> Self {
        ClientDevice { out, next: 0, voices: BTreeMap::new(), finished: Vec::new(), files: BTreeMap::new(), stats: DeviceStats::default() }
    }

    /// One frame: the wanted files read (`fetch`), the WAV decoding continued within the budget, the waiting voices
    /// started when their sound is ready (or dropped), and the voices past their end reported.
    pub fn pump(&mut self, mut fetch: impl FnMut(&[u8]) -> Fetch) {
        let mut budget = DECODE_BUDGET;
        let names: Vec<Vec<u8>> = self.files.iter().filter(|(_, s)| !matches!(s, FileState::Ready(_) | FileState::Missing)).map(|(k, _)| k.clone()).collect();
        for name in names {
            // each file goes as far as it can this frame: read, decoded (within the budget), handed over, ready
            let mut state = self.files.remove(&name).expect("listed");
            loop {
                let kind = std::mem::discriminant(&state);
                state = self.step_file(&name, state, &mut fetch, &mut budget);
                if std::mem::discriminant(&state) == kind || matches!(state, FileState::Ready(_) | FileState::Missing) {
                    break;
                }
            }
            self.files.insert(name, state);
        }
        let now = self.out.now();
        let ids: Vec<VoiceId> = self.voices.keys().copied().collect();
        for id in ids {
            let v = self.voices.get_mut(&id).expect("listed");
            match v.state {
                VoiceState::Waiting { since } => match self.files.get(&v.file) {
                    Some(FileState::Missing) => {
                        self.stats.voices_missing += 1;
                        self.finished.push(id);
                    }
                    Some(FileState::Ready(len)) => {
                        let rate = if v.start.rate > 0.0 { v.start.rate } else { 1.0 };
                        self.out.play(id, &v.file, v.gain, v.pan, rate);
                        self.stats.voices_played += 1;
                        v.state = VoiceState::Playing { ends: now + len / f64::from(rate) };
                        if v.paused {
                            self.out.pause(id, true);
                            v.state = VoiceState::Paused { left: len / f64::from(rate) };
                        }
                    }
                    _ if now - since > MAX_WAIT_S => {
                        self.stats.voices_timed_out += 1;
                        self.finished.push(id);
                    }
                    _ => {}
                },
                VoiceState::Playing { ends } if now >= ends => self.finished.push(id),
                _ => {}
            }
        }
        for id in &self.finished {
            if let Some(v) = self.voices.get_mut(id) {
                // ended: no further output calls for it (the manager may still stop it)
                v.state = VoiceState::Paused { left: -1.0 };
            }
        }
    }

    /// One step of a file's state (see [`ClientDevice::pump`]).
    fn step_file(&mut self, name: &[u8], state: FileState, fetch: &mut impl FnMut(&[u8]) -> Fetch, budget: &mut usize) -> FileState {
        match state {
            FileState::Wanted => match fetch(name) {
                Fetch::Waiting => FileState::Wanted,
                Fetch::Missing => FileState::Missing,
                Fetch::Bytes(b) if b.starts_with(b"RIFF") => match WavDecoder::new(&b) {
                    Ok(d) => FileState::Decoding(Box::new(d), b),
                    Err(_) => FileState::Missing,
                },
                Fetch::Bytes(b) => {
                    if self.out.upload_encoded(name, b) {
                        FileState::Uploaded
                    } else {
                        FileState::Missing
                    }
                }
            },
            FileState::Decoding(mut d, b) => {
                if *budget == 0 {
                    return FileState::Decoding(d, b);
                }
                let before = d.position();
                let done = d.decode_some(&b, *budget);
                *budget = budget.saturating_sub((d.position() - before).max(1));
                if done {
                    self.stats.files_decoded += 1;
                    self.stats.bytes_decoded += b.len() as u64;
                    self.out.upload_pcm(name, d.info.channels, d.info.sample_rate, &d.samples);
                    FileState::Uploaded
                } else {
                    FileState::Decoding(d, b)
                }
            }
            FileState::Uploaded => match self.out.sound_ready(name) {
                None => FileState::Uploaded,
                Some(None) => FileState::Missing,
                Some(Some(len)) => FileState::Ready(len),
            },
            s => s,
        }
    }

    /// The voices playing or waiting.
    pub fn voice_count(&self) -> usize {
        self.voices.len()
    }

    /// The device's state for the status line.
    pub fn status(&self) -> String {
        let s = &self.stats;
        format!(
            "voices={} started={} played={} missing={} timed_out={} files={} decoded={}",
            self.voices.len(),
            s.voices_started,
            s.voices_played,
            s.voices_missing,
            s.voices_timed_out,
            self.files.len(),
            s.files_decoded
        )
    }
}

impl<O: AudioOutput> AudioDevice for ClientDevice<O> {
    fn start(&mut self, file: &[u8], start: VoiceStart) -> Option<VoiceId> {
        if file.is_empty() || matches!(self.files.get(file), Some(FileState::Missing)) {
            return None;
        }
        self.files.entry(file.to_vec()).or_insert(FileState::Wanted);
        self.next += 1;
        let id = self.next;
        let since = self.out.now();
        self.voices.insert(id, Voice { file: file.to_vec(), start, gain: start.gain, pan: start.pan, state: VoiceState::Waiting { since }, paused: false });
        self.stats.voices_started += 1;
        Some(id)
    }
    fn set_gain_pan(&mut self, voice: VoiceId, gain: f32, pan: f32) {
        if let Some(v) = self.voices.get_mut(&voice) {
            (v.gain, v.pan) = (gain, pan);
            if matches!(v.state, VoiceState::Playing { .. }) {
                self.out.set(voice, gain, pan);
            }
        }
    }
    fn stop(&mut self, voice: VoiceId) {
        if let Some(v) = self.voices.remove(&voice) {
            if !matches!(v.state, VoiceState::Waiting { .. }) {
                self.out.stop(voice);
            }
        }
        self.finished.retain(|&f| f != voice);
    }
    fn set_paused(&mut self, voice: VoiceId, paused: bool) {
        let now = self.out.now();
        if let Some(v) = self.voices.get_mut(&voice) {
            v.paused = paused;
            match (v.state, paused) {
                (VoiceState::Playing { ends }, true) => {
                    self.out.pause(voice, true);
                    v.state = VoiceState::Paused { left: ends - now };
                }
                (VoiceState::Paused { left }, false) if left >= 0.0 => {
                    self.out.pause(voice, false);
                    v.state = VoiceState::Playing { ends: now + left };
                }
                _ => {}
            }
        }
    }
    fn take_finished(&mut self) -> Vec<VoiceId> {
        let f = std::mem::take(&mut self.finished);
        for id in &f {
            if let Some(v) = self.voices.remove(id) {
                if !matches!(v.state, VoiceState::Waiting { .. }) {
                    self.out.stop(*id);
                }
            }
        }
        f
    }
    fn file_length_ms(&self, file: &[u8]) -> f32 {
        match self.files.get(file) {
            Some(FileState::Ready(len)) => (len * 1000.0) as f32,
            _ => 0.0,
        }
    }
}

/// The audio's tables from the game's INI files (`AudioManager::init`'s list), read with `open` (the art's file system):
/// `None` while a file is still arriving (try again on a later frame). The errors are the files that did not parse.
///
/// (#88) `prefs` is the user's `Options.ini` as `parseAudioSettingsDefinition` reads it (`OptionPreferences`).
pub fn load_tables(open: impl Fn(&[u8]) -> Option<Option<Vec<u8>>>, prefs: rts_lockstep_engine::client::audio_info::OptionPrefs) -> Option<(AudioTables, Vec<(Vec<u8>, String)>)> {
    let waiting = std::cell::Cell::new(false);
    let mut t = AudioTables::new();
    t.prefs = prefs;
    let errors = t.load(|p| match open(p) {
        None => {
            waiting.set(true);
            None
        }
        Some(b) => b,
    });
    (!waiting.get()).then_some((t, errors))
}

/// The camera rig as the manager's view (`TheTacticalView`): the ground point looked at, the heading, and the camera's
/// place, game coordinates; the rig's `yaw` is upstream's view angle (both look along `(-sin a, cos a)`).
pub fn listener_view(focus: [f32; 2], yaw: f32, pitch: f32, distance: f32) -> ListenerView {
    let (sy, cy) = (wwmath::sin(yaw), wwmath::cos(yaw));
    let (sp, cp) = (wwmath::sin(pitch), wwmath::cos(pitch));
    ListenerView {
        ground: Coord3D { x: focus[0], y: focus[1], z: 0.0 },
        angle: yaw,
        camera: Coord3D { x: focus[0] + sy * cp * distance, y: focus[1] - cy * cp * distance, z: sp * distance },
    }
}

/// The browser's output: the Web Audio API.
#[cfg(target_arch = "wasm32")]
pub mod web {
    use super::AudioOutput;
    use rts_lockstep_engine::client::game_audio::VoiceId;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;
    use wasm_bindgen::JsCast;
    use web_sys::{AudioBuffer, AudioBufferSourceNode, AudioContext, AudioContextState, GainNode, StereoPannerNode};

    /// The page's context (`window.rtsAudioContext`, made and resumed in the import's click), else a new one.
    fn context() -> Option<AudioContext> {
        let w = web_sys::window()?;
        if let Ok(v) = js_sys::Reflect::get(&w, &"rtsAudioContext".into()) {
            if let Ok(c) = v.dyn_into::<AudioContext>() {
                return Some(c);
            }
        }
        AudioContext::new().ok()
    }

    type Decoded = Rc<RefCell<HashMap<Vec<u8>, Option<AudioBuffer>>>>;

    /// One voice's nodes.
    struct Nodes {
        source: AudioBufferSourceNode,
        gain: GainNode,
        pan: StereoPannerNode,
        level: f32,
    }

    #[derive(Default)]
    pub struct WebOutput {
        ctx: Option<AudioContext>,
        buffers: HashMap<Vec<u8>, AudioBuffer>,
        decoded: Decoded,
        voices: HashMap<VoiceId, Nodes>,
    }

    impl WebOutput {
        fn ctx(&mut self) -> Option<&AudioContext> {
            if self.ctx.is_none() {
                self.ctx = context();
            }
            let c = self.ctx.as_ref()?;
            if c.state() == AudioContextState::Suspended {
                // a no-op until the page's user gesture has started it
                let _ = c.resume();
            }
            Some(c)
        }
    }

    impl AudioOutput for WebOutput {
        fn upload_pcm(&mut self, path: &[u8], channels: u16, rate: u32, samples: &[i16]) {
            let Some(c) = self.ctx() else { return };
            let ch = usize::from(channels.max(1));
            let frames = samples.len() / ch;
            if frames == 0 {
                self.decoded.borrow_mut().insert(path.to_vec(), None);
                return;
            }
            let Ok(buf) = c.create_buffer(ch as u32, frames as u32, rate as f32) else {
                self.decoded.borrow_mut().insert(path.to_vec(), None);
                return;
            };
            for k in 0..ch {
                let data: Vec<f32> = samples.iter().skip(k).step_by(ch).map(|&s| f32::from(s) / 32768.0).collect();
                let _ = buf.copy_to_channel(&data, k as i32);
            }
            self.buffers.insert(path.to_vec(), buf);
        }
        fn upload_encoded(&mut self, path: &[u8], bytes: Vec<u8>) -> bool {
            let Some(c) = self.ctx() else { return false };
            let array = js_sys::Uint8Array::from(bytes.as_slice());
            let Ok(promise) = c.decode_audio_data(&array.buffer()) else { return false };
            let decoded = self.decoded.clone();
            let key = path.to_vec();
            wasm_bindgen_futures::spawn_local(async move {
                let r = wasm_bindgen_futures::JsFuture::from(promise).await;
                let buf = r.ok().and_then(|v| v.dyn_into::<AudioBuffer>().ok());
                decoded.borrow_mut().insert(key, buf);
            });
            true
        }
        fn sound_ready(&mut self, path: &[u8]) -> Option<Option<f64>> {
            if let Some(b) = self.buffers.get(path) {
                return Some(Some(b.duration()));
            }
            let d = self.decoded.borrow_mut().remove(path)?;
            match d {
                Some(b) => {
                    let len = b.duration();
                    self.buffers.insert(path.to_vec(), b);
                    Some(Some(len))
                }
                None => Some(None),
            }
        }
        fn play(&mut self, voice: VoiceId, path: &[u8], gain: f32, pan: f32, rate: f32) {
            let Some(buf) = self.buffers.get(path).cloned() else { return };
            let Some(c) = self.ctx() else { return };
            let (Ok(source), Ok(g), Ok(p)) = (c.create_buffer_source(), c.create_gain(), c.create_stereo_panner()) else { return };
            source.set_buffer(Some(&buf));
            source.playback_rate().set_value(rate);
            g.gain().set_value(gain);
            p.pan().set_value(pan);
            let dest = c.destination();
            let ok = source.connect_with_audio_node(&g).is_ok() && g.connect_with_audio_node(&p).is_ok() && p.connect_with_audio_node(&dest).is_ok();
            if ok && source.start().is_ok() {
                self.voices.insert(voice, Nodes { source, gain: g, pan: p, level: gain });
            }
        }
        fn set(&mut self, voice: VoiceId, gain: f32, pan: f32) {
            if let Some(n) = self.voices.get_mut(&voice) {
                n.level = gain;
                n.gain.gain().set_value(gain);
                n.pan.pan().set_value(pan);
            }
        }
        fn stop(&mut self, voice: VoiceId) {
            if let Some(n) = self.voices.remove(&voice) {
                let _ = n.source.unchecked_ref::<web_sys::AudioScheduledSourceNode>().stop();
                let _ = n.pan.disconnect();
            }
        }
        /// A buffer source cannot pause: a paused voice is silenced (its clock runs on).
        fn pause(&mut self, voice: VoiceId, paused: bool) {
            if let Some(n) = self.voices.get(&voice) {
                n.gain.gain().set_value(if paused { 0.0 } else { n.level });
            }
        }
        fn now(&self) -> f64 {
            self.ctx.as_ref().map_or(0.0, |c| c.current_time())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_lockstep_engine::client::game_audio::VoiceKind;
    use rts_lockstep_engine::ww::wwmath;

    fn wav16(rate: u32, frames: usize) -> Vec<u8> {
        let data: Vec<u8> = (0..frames).flat_map(|i| ((i as i16).wrapping_mul(97)).to_le_bytes()).collect();
        let mut b = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0".to_vec();
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&(rate * 2).to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend_from_slice(&data);
        b
    }

    const S: VoiceStart = VoiceStart { gain: 0.5, pan: 0.0, rate: 1.0, kind: VoiceKind::Sample2D };

    /// Predicted: a voice on a file that arrives two frames later waits, then plays (the half-second WAV decoded within
    /// one frame's budget); it is reported ended once the clock passes half a second at rate 1 (and a quarter at rate
    /// 2); a voice on a missing file ends on the next pump, and a later start on it is refused at once; a voice whose
    /// file never arrives is dropped after two seconds. A 600 KiB file is read and decoded over three frames (256 KiB of
    /// input each), and plays on the third.
    #[test]
    fn voices_wait_for_their_files_play_and_end() {
        let mut d = ClientDevice::new(NullOutput::default());
        let a = d.start(b"a.wav", S).unwrap();
        let fast = d.start(b"a.wav", VoiceStart { rate: 2.0, ..S }).unwrap();
        let gone = d.start(b"gone.wav", S).unwrap();
        let never = d.start(b"never.wav", S).unwrap();
        let mut frame = 0;
        let file = wav16(22050, 11025);
        let pump = |d: &mut ClientDevice<NullOutput>, frame: u32| {
            d.pump(|p| match p {
                b"a.wav" if frame >= 2 => Fetch::Bytes(file.clone()),
                b"gone.wav" => Fetch::Missing,
                _ => Fetch::Waiting,
            });
            d.out.advance(0.1);
        };
        pump(&mut d, frame);
        assert_eq!(d.take_finished(), vec![gone]);
        assert!(d.start(b"gone.wav", S).is_none());
        assert_eq!(d.out.plays, 0);
        frame += 1;
        pump(&mut d, frame);
        frame += 1;
        pump(&mut d, frame);
        assert_eq!(d.out.plays, 2, "both voices of a.wav started");
        assert_eq!(d.file_length_ms(b"a.wav"), 500.0);
        let mut ended = Vec::new();
        for _ in 0..25 {
            frame += 1;
            pump(&mut d, frame);
            for v in d.take_finished() {
                ended.push((v, frame));
            }
        }
        assert_eq!(ended.iter().map(|e| e.0).collect::<Vec<_>>(), [fast, a, never]);
        assert_eq!(ended[0].1, 5, "the fast one after a quarter second");
        assert_eq!(ended[1].1, 7, "the other after half a second");
        assert!(ended[2].1 >= 20, "the missing one after two seconds");
        assert_eq!(d.stats.voices_timed_out, 1);

        let big = wav16(22050, 300 * 1024);
        let mut d = ClientDevice::new(NullOutput::default());
        d.start(b"big.wav", S).unwrap();
        let mut frames = 0;
        while d.out.plays == 0 {
            d.pump(|_| Fetch::Bytes(big.clone()));
            frames += 1;
            assert!(frames < 10);
        }
        assert_eq!(frames, 3, "read on the first, decoded over three");
    }

    /// Predicted: the rig at yaw 0 looking north from the south: the camera at y = focus - cos(pitch) * distance; at a
    /// quarter turn the camera is east of the focus... (yaw pi/2: forward (-1, 0), the camera behind it, +x).
    #[test]
    fn the_listener_follows_the_rig() {
        let v = listener_view([100.0, 200.0], 0.0, 0.5, 300.0);
        assert_eq!(v.ground, Coord3D { x: 100.0, y: 200.0, z: 0.0 });
        assert!((v.camera.y - (200.0 - wwmath::cos(0.5) * 300.0)).abs() < 1e-3);
        let q = listener_view([0.0, 0.0], std::f32::consts::FRAC_PI_2, 0.0, 100.0);
        assert!((q.camera.x - 100.0).abs() < 1e-3 && q.camera.y.abs() < 1e-3, "{:?}", q.camera);
    }
}
