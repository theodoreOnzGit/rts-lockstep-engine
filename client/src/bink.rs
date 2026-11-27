//! The Bink movie player (#241), behind the client's `bink` feature (off by default).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/VideoDevice/Bink/BinkVideoPlayer.h
//!   GeneralsMD/Code/GameEngineDevice/Source/VideoDevice/Bink/BinkVideoPlayer.cpp
//!
//! # What is translated and what is not
//!
//! `BinkVideoPlayer.cpp` is EA's glue between the game's `VideoPlayer` and RAD's Bink SDK. That SDK (`bink.h`,
//! `binkw32.dll`) was never released (`upstream/README.md`), so every `Bink*` call has nothing to translate. It is
//! answered here by the `infinitier_bik_decoder` crate (=0.1.0, GPL-3.0-or-later), a port of FFmpeg's open Bink
//! decoder (`libavcodec/bink.c`, `binkaudio.c`). **The decoding is the crate's, not a translation of EA's code.**
//!
//! Translated: `open`'s three-path lookup ([`open_paths`]), `createStream`'s speech volume ([`bink_volume`]),
//! `frameRender`'s `VideoBuffer` format → `BINKSURFACE*` switch ([`surface_for`]), and every `BinkVideoStream` member
//! ([`BinkVideoStream`]).
//!
//! Not translated, as they are Miles and DirectSound plumbing (also never released): `initializeBinkWithMiles`,
//! `notifyVideoPlayerOfNewProvider`, `BinkSoundUseDirectSound`, `BinkSetSoundTrack`, `releaseHandleForBink`. The
//! decoded audio is kept per frame ([`BinkVideoStream::take_audio`]) for `src/audio.rs` to play (a later step, #241).
//!
//! # The RAD calls, as modelled
//!
//! RAD's documentation is not in the repository either, so the behaviour below is the model this file assumes, not a
//! measurement:
//! * `BinkOpen(path, BINKPRELOADALL)`: the whole file is in memory ([`BinkVideoStream::open`] takes its bytes).
//! * `HBINK::FrameNum` is 1-based: 1 after the open, the frame the next `BinkDoFrame` decompresses.
//!   `frameIndex()` is `FrameNum - 1`.
//! * `BinkDoFrame` decompresses frame `FrameNum`. `BinkNextFrame` advances `FrameNum`, and after the last frame it
//!   wraps to 1 (Bink movies loop; callers stop at `frameIndex() >= frameCount() - 1`).
//! * `BinkGoto(index)` makes `index` (1-based) the next frame. The decoder only decodes forward, so a backward goto
//!   restarts it and decodes up to `index - 1` again.
//! * `BinkWait` is nonzero ("wait") until the current frame's display time has come; the time is the caller's clock
//!   ([`BinkVideoStream::set_clock_us`]), as this crate has no wall clock of its own (wasm has none in `std`).
//! * `BinkCopyToBuffer(mem, pitch, height, x, y, flags)`: the decompressed frame is written at `(x, y)` of the
//!   destination, clipped to `height` rows and to `pitch` bytes per row. `BINKSURFACE32` is X8R8G8B8 (bytes B, G, R,
//!   X; X written 0xFF), `24` is R8G8B8 (B, G, R), `565` and `555` are little-endian 16-bit words.
//!
//! # Determinism
//!
//! The movie never reaches the simulation, which is why the decoder's own `f32` maths is acceptable here: a frame's
//! colours may differ in the last bit between machines without changing any CRC.

use std::io::Cursor;

use infinitier_bik_decoder::{BikAudioChunk, BikError, BikOutputFormat, BikPixels, BikStreamingDecoder};

/// `VIDEO_LANG_PATH_FORMAT` (`"Data/%s/Movies/%s.%s"`).
fn video_lang_path(language: &str, filename: &str) -> String {
    format!("Data/{language}/Movies/{filename}.{VIDEO_EXT}")
}

/// `VIDEO_PATH`.
pub const VIDEO_PATH: &str = "Data\\Movies";
/// `VIDEO_EXT`.
pub const VIDEO_EXT: &str = "bik";

/// The paths `BinkVideoPlayer::open` tries, in its order, for a `Video` whose `m_filename` is `filename`:
/// * with a mod folder, `"%s%s\\%s.%s"` of `m_modDir`, [`VIDEO_PATH`], the name and [`VIDEO_EXT`];
/// * then the localized `"Data/<language>/Movies/<name>.bik"` (`GetRegistryLanguage()`);
/// * then `"Data\\Movies\\<name>.bik"`.
///
/// The first one that opens is the movie. Upstream's `getVideo(movieTitle)` (the INI's `Video` entry) is the caller's,
/// as is the file lookup: the player's files are the host's (`src/import.rs`).
pub fn open_paths(mod_dir: &str, language: &str, filename: &str) -> Vec<String> {
    let mut paths = Vec::new();
    if !mod_dir.is_empty() {
        paths.push(format!("{mod_dir}{VIDEO_PATH}\\{filename}.{VIDEO_EXT}"));
    }
    paths.push(video_lang_path(language, filename));
    paths.push(format!("{VIDEO_PATH}\\{filename}.{VIDEO_EXT}"));
    paths
}

/// `BinkVideoPlayer::createStream`'s volume, from `TheAudio->getVolume(AudioAffect_Speech)`: "never let volume go to 0,
/// as Bink will interpret that as "play at full volume"". `mod = (Int)((speech * 0.8f) * 100) + 1`, then
/// `(32768 * mod) / 100`, the value given to `BinkSetVolume(handle, 0, volume)`, where 32768 is Bink's full volume.
pub fn bink_volume(speech_volume: f32) -> i32 {
    let m = ((speech_volume * 0.8f32) * 100.0f32) as i32 + 1;
    (32768 * m) / 100
}

/// `VideoBuffer::Type` (`TYPE_UNKNOWN`, `TYPE_R8G8B8`, `TYPE_X8R8G8B8`, `TYPE_R5G6B5`, `TYPE_X1R5G5B5`), as this file
/// needs it. The same order as the draft's `VideoBufferType` (`src/drafts/logic_common/video_player.rs`); this module
/// does not use that one so that the `bink` feature does not need `untested-drafts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoBufferFormat {
    /// `TYPE_UNKNOWN`.
    Unknown,
    /// `TYPE_R8G8B8`.
    R8G8B8,
    /// `TYPE_X8R8G8B8`.
    X8R8G8B8,
    /// `TYPE_R5G6B5`.
    R5G6B5,
    /// `TYPE_X1R5G5B5`.
    X1R5G5B5,
}

/// The `BINKSURFACE*` flags `frameRender` passes to `BinkCopyToBuffer`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinkSurface {
    /// `BINKSURFACE32`.
    Surface32,
    /// `BINKSURFACE24`.
    Surface24,
    /// `BINKSURFACE565`.
    Surface565,
    /// `BINKSURFACE555`.
    Surface555,
}

impl BinkSurface {
    /// Bytes per pixel of the surface.
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            BinkSurface::Surface32 => 4,
            BinkSurface::Surface24 => 3,
            BinkSurface::Surface565 | BinkSurface::Surface555 => 2,
        }
    }
}

/// `frameRender`'s `switch (buffer->format())`: the surface for each format; `default: return;` (no copy) is `None`.
pub fn surface_for(format: VideoBufferFormat) -> Option<BinkSurface> {
    match format {
        VideoBufferFormat::X8R8G8B8 => Some(BinkSurface::Surface32),
        VideoBufferFormat::R8G8B8 => Some(BinkSurface::Surface24),
        VideoBufferFormat::R5G6B5 => Some(BinkSurface::Surface565),
        VideoBufferFormat::X1R5G5B5 => Some(BinkSurface::Surface555),
        VideoBufferFormat::Unknown => None,
    }
}

/// `BinkCopyToBuffer(handle, mem, pitch, height, x, y, flags)` from a tightly packed RGBA8 frame of `width` ×
/// `frame_height`, under the model in the module doc. Rows below `dest_height`, and pixels past `pitch`, are clipped.
pub fn copy_to_buffer(
    rgba: &[u8],
    width: usize,
    frame_height: usize,
    mem: &mut [u8],
    pitch: usize,
    dest_height: usize,
    x: usize,
    y: usize,
    surface: BinkSurface,
) {
    let bpp = surface.bytes_per_pixel();
    for row in 0..frame_height {
        let dy = y + row;
        if dy >= dest_height {
            break;
        }
        for col in 0..width {
            let dx = (x + col) * bpp;
            if dx + bpp > pitch {
                break;
            }
            let at = dy * pitch + dx;
            let Some(dst) = mem.get_mut(at..at + bpp) else { return };
            let s = (row * width + col) * 4;
            let (r, g, b) = (rgba[s], rgba[s + 1], rgba[s + 2]);
            match surface {
                BinkSurface::Surface32 => dst.copy_from_slice(&[b, g, r, 0xFF]),
                BinkSurface::Surface24 => dst.copy_from_slice(&[b, g, r]),
                BinkSurface::Surface565 => {
                    let w = ((r as u16 >> 3) << 11) | ((g as u16 >> 2) << 5) | (b as u16 >> 3);
                    dst.copy_from_slice(&w.to_le_bytes());
                }
                BinkSurface::Surface555 => {
                    let w = ((r as u16 >> 3) << 10) | ((g as u16 >> 3) << 5) | (b as u16 >> 3);
                    dst.copy_from_slice(&w.to_le_bytes());
                }
            }
        }
    }
}

/// `class BinkVideoStream`: one open movie (`m_handle`, the `HBINK`). `m_next` and `m_player` (the player's stream
/// list) are the caller's, as in the draft's `VideoPlayer` arena.
pub struct BinkVideoStream {
    /// The file's bytes (`BINKPRELOADALL`), kept to restart the decoder for a backward `BinkGoto`.
    bytes: Vec<u8>,
    name: String,
    decoder: BikStreamingDecoder<Cursor<Vec<u8>>>,
    /// Frames the decoder has delivered (the 1-based number of the last one decoded; 0 before any).
    decoded: u32,
    /// `HBINK::FrameNum`.
    frame_num: u32,
    /// `HBINK::Frames`.
    frames: u32,
    width: u32,
    height: u32,
    /// The last decompressed frame, RGBA8.
    rgba: Vec<u8>,
    /// The audio decoded with the frames since the last [`BinkVideoStream::take_audio`].
    audio: Vec<BikAudioChunk>,
    /// The caller's clock, microseconds.
    clock_us: u64,
    /// When frame `frame_num` may be shown (the clock of the open plus its frames' durations).
    due_us: u64,
    frame_duration_us: u32,
    /// `BinkSetVolume`'s value ([`bink_volume`]), for the audio's output.
    pub volume: i32,
}

/// Why a movie could not be opened or decoded (the crate's error: a bad header, a truncated packet...).
pub type BinkStreamError = BikError;

fn new_decoder(bytes: &[u8], name: &str) -> Result<BikStreamingDecoder<Cursor<Vec<u8>>>, BinkStreamError> {
    Ok(BikStreamingDecoder::new(Cursor::new(bytes.to_vec()), name)?.with_output_format(BikOutputFormat::Rgba))
}

impl BinkVideoStream {
    /// `BinkOpen(path, BINKPRELOADALL)` then `BinkVideoPlayer::createStream(handle)`: `NULL` (an `Err`) when the file
    /// does not open. `speech_volume` is `TheAudio->getVolume(AudioAffect_Speech)`; `now_us` is the caller's clock.
    pub fn open(bytes: Vec<u8>, name: &str, speech_volume: f32, now_us: u64) -> Result<BinkVideoStream, BinkStreamError> {
        let decoder = new_decoder(&bytes, name)?;
        let (frames, width, height, frame_duration_us) =
            (decoder.frame_count(), decoder.width(), decoder.height(), decoder.frame_duration_us());
        Ok(BinkVideoStream {
            bytes,
            name: name.to_string(),
            decoder,
            decoded: 0,
            frame_num: 1,
            frames,
            width,
            height,
            rgba: vec![0; width as usize * height as usize * 4],
            audio: Vec::new(),
            clock_us: now_us,
            due_us: now_us,
            frame_duration_us,
            volume: bink_volume(speech_volume),
        })
    }

    /// The caller's clock, in microseconds, which `BinkWait` reads.
    pub fn set_clock_us(&mut self, now_us: u64) {
        self.clock_us = now_us;
    }

    /// `BinkWait(handle)`: true ("wait") until frame `FrameNum`'s time has come.
    fn bink_wait(&self) -> bool {
        self.clock_us < self.due_us
    }

    /// `BinkVideoStream::update`: `BinkWait( m_handle );` (its result is dropped upstream).
    pub fn update(&mut self) {
        let _ = self.bink_wait();
    }

    /// `BinkVideoStream::isFrameReady`: `!BinkWait( m_handle )`.
    pub fn is_frame_ready(&self) -> bool {
        !self.bink_wait()
    }

    /// Decode forward until frame `n` (1-based) is the last one decoded.
    fn decode_to(&mut self, n: u32) -> Result<(), BinkStreamError> {
        if n < self.decoded {
            self.decoder = new_decoder(&self.bytes, &self.name)?;
            self.decoded = 0;
        }
        while self.decoded < n {
            let Some(frame) = self.decoder.next_frame()? else { break };
            self.decoded += 1;
            self.audio.extend(frame.audio);
            if let BikPixels::Rgba(p) = frame.video.pixels {
                self.rgba = p;
            }
        }
        Ok(())
    }

    /// `BinkVideoStream::frameDecompress`: `BinkDoFrame( m_handle );`, frame `FrameNum`.
    pub fn frame_decompress(&mut self) -> Result<(), BinkStreamError> {
        let n = self.frame_num;
        if n != self.decoded {
            self.decode_to(n)?;
        }
        Ok(())
    }

    /// `BinkVideoStream::frameRender(buffer)`: the format switch ([`surface_for`]; an unknown format copies nothing),
    /// then `BinkCopyToBuffer(m_handle, mem, pitch, height, xPos, yPos, flags)` when `mem` (the locked buffer) exists.
    pub fn frame_render(&self, mem: Option<&mut [u8]>, format: VideoBufferFormat, pitch: usize, height: usize, x: usize, y: usize) {
        let Some(surface) = surface_for(format) else { return };
        if let Some(mem) = mem {
            copy_to_buffer(&self.rgba, self.width as usize, self.height as usize, mem, pitch, height, x, y, surface);
        }
    }

    /// `BinkVideoStream::frameNext`: `BinkNextFrame( m_handle );` (wraps to frame 1 after the last).
    pub fn frame_next(&mut self) {
        self.frame_num = if self.frame_num >= self.frames { 1 } else { self.frame_num + 1 };
        self.due_us += self.frame_duration_us as u64;
    }

    /// `BinkVideoStream::frameIndex`: `m_handle->FrameNum - 1`.
    pub fn frame_index(&self) -> i32 {
        self.frame_num as i32 - 1
    }

    /// `BinkVideoStream::frameCount`: `m_handle->Frames`.
    pub fn frame_count(&self) -> i32 {
        self.frames as i32
    }

    /// `BinkVideoStream::frameGoto(index)`: `BinkGoto(m_handle, index, NULL)`; `index` is 1-based, clamped to the
    /// movie's frames.
    pub fn frame_goto(&mut self, index: i32) {
        self.frame_num = (index.max(1) as u32).min(self.frames.max(1));
        self.due_us = self.clock_us;
    }

    /// `BinkVideoStream::height`: `m_handle->Height`.
    pub fn height(&self) -> i32 {
        self.height as i32
    }

    /// `BinkVideoStream::width`: `m_handle->Width`.
    pub fn width(&self) -> i32 {
        self.width as i32
    }

    /// The last decompressed frame, RGBA8, `width × height`.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// The audio decoded since the last call (interleaved 16-bit PCM per chunk), for the audio's output.
    pub fn take_audio(&mut self) -> Vec<BikAudioChunk> {
        std::mem::take(&mut self.audio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `open`'s paths, in upstream's order and spelling (mixed separators as upstream writes them).
    #[test]
    fn open_tries_the_mod_then_the_language_then_the_plain_folder() {
        assert_eq!(open_paths("", "english", "EA_LOGO"), vec!["Data/english/Movies/EA_LOGO.bik", "Data\\Movies\\EA_LOGO.bik"]);
        assert_eq!(
            open_paths("Mods\\M\\", "german", "x"),
            vec!["Mods\\M\\Data\\Movies\\x.bik", "Data/german/Movies/x.bik", "Data\\Movies\\x.bik"]
        );
    }

    /// `createStream`'s volume, by hand from the formula: speech 0 → mod 1 → 327; speech 1 → (Int)80.0 + 1 = 81 →
    /// 26542; speech 0.5 → (Int)40.0 + 1 = 41 → 13434.
    #[test]
    fn the_volume_is_never_zero() {
        assert_eq!(bink_volume(0.0), 327);
        assert_eq!(bink_volume(1.0), 26542);
        assert_eq!(bink_volume(0.5), 13434);
    }

    /// The format switch, and one pixel in each surface: (r, g, b) = (0xF8, 0xFC, 0x08).
    #[test]
    fn each_buffer_format_gets_its_surface() {
        assert_eq!(surface_for(VideoBufferFormat::Unknown), None);
        let rgba = [0xF8, 0xFC, 0x08, 0xFF];
        let one = |s: BinkSurface| {
            let mut mem = vec![0u8; 2 * s.bytes_per_pixel() * 2];
            copy_to_buffer(&rgba, 1, 1, &mut mem, 2 * s.bytes_per_pixel(), 2, 1, 1, s);
            mem[3 * s.bytes_per_pixel()..].to_vec()
        };
        assert_eq!(one(surface_for(VideoBufferFormat::X8R8G8B8).unwrap()), [0x08, 0xFC, 0xF8, 0xFF]);
        assert_eq!(one(surface_for(VideoBufferFormat::R8G8B8).unwrap()), [0x08, 0xFC, 0xF8]);
        // 565: r 31, g 63, b 1 → 0xFFE1.
        assert_eq!(one(surface_for(VideoBufferFormat::R5G6B5).unwrap()), [0xE1, 0xFF]);
        // 555: r 31, g 31, b 1 → 0x7FE1.
        assert_eq!(one(surface_for(VideoBufferFormat::X1R5G5B5).unwrap()), [0xE1, 0x7F]);
    }

    /// Clipping: a 2×2 frame at (1, 1) of a 2×2 destination writes one pixel only.
    #[test]
    fn the_copy_is_clipped_to_the_destination() {
        let rgba = [1u8, 2, 3, 255].repeat(4);
        let mut mem = vec![0u8; 2 * 2 * 3];
        copy_to_buffer(&rgba, 2, 2, &mut mem, 6, 2, 1, 1, BinkSurface::Surface24);
        assert_eq!(mem, [0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 2, 1]);
    }

    /// A file that is not Bink does not open (`BinkOpen` gives `NULL`, so `createStream` gives no stream).
    #[test]
    fn a_file_that_is_not_bink_does_not_open() {
        assert!(BinkVideoStream::open(b"not a movie".to_vec(), "x.bik", 1.0, 0).is_err());
    }

    /// Every movie of the owner's install (`RTS_CAMPAIGN_INSTALL`, the folder of `.big` archives): each `.bik`, loose
    /// or inside an archive, opens, plays every frame through `frameDecompress`/`frameNext` at its own pace, renders
    /// into an X8R8G8B8 buffer, and comes back to frame 1. Skipped without the path; never faked.
    #[test]
    fn every_movie_of_the_install_plays_to_the_end() {
        use rts_lockstep_engine::formats::big::{ArchiveFile, FilenameList};
        let Ok(dir) = std::env::var("RTS_CAMPAIGN_INSTALL") else { return };
        let mut movies: Vec<(String, Vec<u8>)> = Vec::new();
        let mut stack = vec![std::path::PathBuf::from(&dir)];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).expect("the folder").filter_map(|e| e.ok()) {
                let p = e.path();
                let n = p.to_string_lossy().to_ascii_lowercase();
                if p.is_dir() {
                    stack.push(p);
                } else if n.ends_with(".bik") {
                    movies.push((n, std::fs::read(&p).expect("a movie")));
                } else if n.ends_with(".big") {
                    let a = ArchiveFile::open_archive_file(n.as_bytes(), std::fs::read(&p).expect("an archive")).expect("a .big");
                    let mut list = FilenameList::new();
                    a.get_file_list_in_directory(b"", b"", b"*.bik", &mut list, true);
                    for m in list.iter() {
                        movies.push((format!("{n}:{}", String::from_utf8_lossy(m)), a.open_file(m).expect("a member").to_vec()));
                    }
                }
            }
        }
        movies.sort();
        let mut frames_total = 0;
        for (name, bytes) in &movies {
            let mut s = BinkVideoStream::open(bytes.clone(), name, 1.0, 0).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(s.width() > 0 && s.height() > 0 && s.frame_count() > 0, "{name}");
            let pitch = s.width() as usize * 4;
            let mut mem = vec![0u8; pitch * s.height() as usize];
            let mut clock = 0u64;
            for k in 0..s.frame_count() {
                while !s.is_frame_ready() {
                    clock += 1000;
                    s.set_clock_us(clock);
                    s.update();
                }
                assert_eq!(s.frame_index(), k, "{name}");
                s.frame_decompress().unwrap_or_else(|e| panic!("{name} frame {k}: {e}"));
                s.frame_render(Some(&mut mem), VideoBufferFormat::X8R8G8B8, pitch, s.height() as usize, 0, 0);
                s.frame_next();
            }
            assert_eq!(s.frame_index(), 0, "{name}: wraps to frame 1");
            frames_total += s.frame_count();
            eprintln!("MEASURED {name}: {}x{}, {} frames", s.width(), s.height(), s.frame_count());
        }
        eprintln!("MEASURED {} movies, {frames_total} frames", movies.len());
    }
}
