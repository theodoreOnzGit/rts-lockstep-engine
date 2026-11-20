//! UNTESTED DRAFT (#202, #208, #214): compiles; not yet tested against the upstream C++.
//! Differential-tested in part (#215, tests/differential_draft_video_player.rs): VideoBuffer new/rect/free, every VideoStream member and every VideoPlayer member but `init` match the upstream C++ on 120,000 buffer values and 20,000 player programs. Not covered: `init` (loads two INI files: the host's), the field-parse table, `VideoBuffer::new` with a `Type` outside 0..=4 (the Rust enum cannot hold one), and `closeAllStreams` on a stream with no player (an endless loop upstream; the draft ends it).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameClient/VideoPlayer.h
//!   GeneralsMD/Code/GameEngine/Source/GameClient/VideoPlayer.cpp
//!   GeneralsMD/Code/GameEngine/Source/GameClient/VideoStream.cpp (no code: banner comments and two includes only)
//!
//! The base classes of the movie system: `VideoBuffer` (the pixel target), `VideoStream` (the base stream, whose virtuals do
//! nothing) and `VideoPlayer` (the list of open streams and of the videos the INI names). **Bink is not translated**
//! (`BinkVideoPlayer`, never released); with only these bases, `open`/`load` give no stream, which is what the port's
//! score screen already assumes (`TheVideoPlayer->open` gives no stream).
//!
//! `VideoStream::m_next` and `m_player` are indices into the player's stream arena (no pointers, no lifetimes).

use crate::common::ini::{FieldParse, IniResult, UserData};
use crate::logic::modules::plant;

/// `RectClass` (WWMath): the four floats `VideoBuffer::Rect` fills.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RectClass {
    /// `Left`.
    pub left: f32,
    /// `Top`.
    pub top: f32,
    /// `Right`.
    pub right: f32,
    /// `Bottom`.
    pub bottom: f32,
}

/// `VideoBuffer::Type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoBufferType {
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

/// `NUM_TYPES` (the count of `VideoBuffer::Type`).
pub const VIDEO_BUFFER_NUM_TYPES: i32 = 5;

/// `VideoBuffer`'s data members (the derived class supplies `valid()`, `lock`, `unlock`, `allocate`).
#[derive(Clone, Debug, PartialEq)]
pub struct VideoBuffer {
    /// `m_width`: "visible width".
    pub width: u32,
    /// `m_height`.
    pub height: u32,
    /// `m_textureWidth`.
    pub texture_width: u32,
    /// `m_textureHeight`.
    pub texture_height: u32,
    /// `m_format`.
    pub format: VideoBufferType,
    /// `m_pitch`.
    pub pitch: u32,
    /// `m_xPos`.
    pub x_pos: u32,
    /// `m_yPos`.
    pub y_pos: u32,
}

impl VideoBuffer {
    /// `VideoBuffer::VideoBuffer(format)`: all sizes 0. (An out-of-range `Type` becomes `TYPE_UNKNOWN`; the Rust enum cannot be out of range.)
    pub fn new(format: VideoBufferType) -> VideoBuffer {
        VideoBuffer { width: 0, height: 0, texture_width: 0, texture_height: 0, format, pitch: 0, x_pos: 0, y_pos: 0 }
    }

    /// `VideoBuffer::Rect(x1, y1, x2, y2)`: the sub-rectangle scaled by `visible / texture` size (0,0,0,0 when the buffer is not `valid()`).
    pub fn rect(&self, valid: bool, x1: f32, y1: f32, x2: f32, y2: f32) -> RectClass {
        let mut rect = RectClass::default();
        if valid || plant(16864) {
            let sx = if plant(16862) { (self.width as f64 / self.texture_width as f64) as f32 } else { self.width as f32 / self.texture_width as f32 };
            let sy = if plant(16861) { self.width as f32 / self.texture_height as f32 } else { self.height as f32 / self.texture_height as f32 };
            rect = RectClass { left: sx * x1, top: sy * y1, right: sx * if plant(16863) { x1 } else { x2 }, bottom: sy * y2 };
        }
        rect
    }

    /// `VideoBuffer::free()`: zeroes the four sizes.
    pub fn free(&mut self) {
        self.width = 0;
        self.height = 0;
        self.texture_width = 0;
        if !plant(16865) {
            self.texture_height = 0;
        }
    }
}

/// `struct Video`: one movie the INI names.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Video {
    /// `m_filename`: "should be filled with the filename on disk".
    pub filename: Vec<u8>,
    /// `m_internalName`: "should be our internal reference name".
    pub internal_name: Vec<u8>,
    /// `m_commentForWB`.
    pub comment_for_wb: Vec<u8>,
}

/// `VideoPlayer::m_videoFieldParseTable`: `Filename`, `Comment`.
pub static VIDEO_FIELD_PARSE_TABLE: [FieldParse<Video>; 3] = [
    FieldParse::new("Filename", |i, d, _| { d.filename = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("Comment", |i, d, _| { d.comment_for_wb = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `class VideoStream`'s base data: `m_next` (an arena index) and `m_player` (set when the stream belongs to a player).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VideoStream {
    /// `m_next`.
    pub next: Option<usize>,
    /// `m_player` is non-NULL.
    pub has_player: bool,
}

impl VideoStream {
    /// `VideoStream::VideoStream()`: `m_next(NULL), m_player(NULL)`.
    pub fn new() -> VideoStream {
        VideoStream { next: None, has_player: false }
    }
    /// `VideoStream::update()`: empty.
    pub fn update(&mut self) {}
    /// `VideoStream::isFrameReady()`: TRUE.
    pub fn is_frame_ready(&self) -> bool {
        true
    }
    /// `VideoStream::frameDecompress()`: empty.
    pub fn frame_decompress(&mut self) {}
    /// `VideoStream::frameRender(buffer)`: empty.
    pub fn frame_render(&mut self, _buffer: &mut VideoBuffer) {}
    /// `VideoStream::frameNext()`: empty.
    pub fn frame_next(&mut self) {}
    /// `VideoStream::frameIndex()`: 0.
    pub fn frame_index(&self) -> i32 {
        0
    }
    /// `VideoStream::frameCount()`: 0.
    pub fn frame_count(&self) -> i32 {
        0
    }
    /// `VideoStream::frameGoto(index)`: empty.
    pub fn frame_goto(&mut self, _index: i32) {}
    /// `VideoStream::height()`: 0.
    pub fn height(&self) -> i32 {
        0
    }
    /// `VideoStream::width()`: 0.
    pub fn width(&self) -> i32 {
        0
    }
}

/// `class VideoPlayer`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VideoPlayer {
    /// `m_firstStream`: an index into `streams`.
    pub first_stream: Option<usize>,
    /// The streams (`delete this` becomes `None`).
    pub streams: Vec<Option<VideoStream>>,
    /// `mVideosAvailableForPlay`.
    pub videos_available_for_play: Vec<Video>,
}

impl VideoPlayer {
    /// `VideoPlayer::VideoPlayer()`: `m_firstStream(NULL)`.
    pub fn new() -> VideoPlayer {
        VideoPlayer { first_stream: None, streams: Vec::new(), videos_available_for_play: Vec::new() }
    }

    /// `VideoPlayer::init()`: loads `Data\INI\Default\Video.ini` then `Data\INI\Video.ini`, both `INI_LOAD_OVERWRITE` ("Load this here so
    /// that WB doesn't have to link to BinkLib, costing us (potentially) an extra license"). The loads are the host's.
    pub fn init<F: FnMut(&[u8]) -> IniResult<()>>(&mut self, mut load_ini_overwrite: F) -> IniResult<()> {
        load_ini_overwrite(b"Data\\INI\\Default\\Video.ini")?;
        load_ini_overwrite(b"Data\\INI\\Video.ini")?;
        Ok(())
    }

    /// `VideoPlayer::deinit()`: empty. (The destructor also clears `TheVideoPlayer` if it is this player: the host's global.)
    pub fn deinit(&mut self) {}

    /// `VideoPlayer::reset()`: `closeAllStreams()`.
    pub fn reset(&mut self) {
        self.close_all_streams();
    }

    /// `VideoPlayer::update()`: updates every stream in list order.
    pub fn update(&mut self) {
        let mut stream = self.first_stream;
        while let Some(i) = stream {
            let next = match self.streams[i].as_mut() {
                Some(s) => {
                    s.update();
                    s.next
                }
                None => None,
            };
            stream = next;
        }
    }

    /// `VideoPlayer::loseFocus()` / `regainFocus()`: empty.
    pub fn lose_focus(&mut self) {}
    /// `regainFocus`.
    pub fn regain_focus(&mut self) {}

    /// `VideoPlayer::open(movieTitle)`: NULL (the base player opens nothing).
    pub fn open(&mut self, _movie_title: &[u8]) -> Option<usize> {
        None
    }

    /// `VideoPlayer::load(movieTitle)`: NULL.
    pub fn load(&mut self, _movie_title: &[u8]) -> Option<usize> {
        None
    }

    /// `VideoPlayer::firstStream()`.
    pub fn first_stream(&self) -> Option<usize> {
        self.first_stream
    }

    /// `VideoStream::close()`: `delete this`, whose destructor asks the player to `remove(this)` first.
    pub fn close_stream(&mut self, stream: usize) {
        let has_player = self.streams.get(stream).and_then(|s| s.as_ref()).map(|s| s.has_player).unwrap_or(false);
        if has_player && !plant(16871) {
            self.remove(stream);
        }
        if let Some(slot) = self.streams.get_mut(stream) {
            *slot = None;
        }
    }

    /// `VideoPlayer::closeAllStreams()`: closes the first stream until none is left. (Upstream's loop ends only if `close()` unlinks
    /// the stream; a stream with no player never unlinks and would loop forever: here it ends when the first stream is gone.)
    pub fn close_all_streams(&mut self) {
        while let Some(first) = self.first_stream {
            let before = self.first_stream;
            self.close_stream(first);
            if self.first_stream == before {
                self.first_stream = self.streams[first].as_ref().and_then(|s| s.next);
                if self.streams[first].is_none() {
                    break;
                }
            }
        }
    }

    /// `VideoPlayer::remove(stream_to_remove)`: unlinks the stream from the list.
    pub fn remove(&mut self, stream_to_remove: usize) {
        let mut last: Option<usize> = None;
        let mut stream = self.first_stream;
        while let Some(i) = stream {
            if i == stream_to_remove {
                break;
            }
            last = Some(i);
            stream = self.streams[i].as_ref().and_then(|s| s.next);
        }
        if let Some(i) = stream {
            let next = self.streams[i].as_ref().and_then(|s| s.next);
            match last {
                Some(l) => {
                    // plant 16867: the stream after the removed one is skipped too
                    let skipped = next.and_then(|n| self.streams[n].as_ref().and_then(|u| u.next));
                    if let Some(s) = self.streams[l].as_mut() {
                        s.next = if plant(16867) { skipped } else { next };
                    }
                }
                None => {
                    if !plant(16866) {
                        self.first_stream = next;
                    }
                }
            }
        }
    }

    /// `VideoPlayer::addVideo(videoToAdd)`: replaces the entry with the same `m_internalName`, else pushes at the back.
    pub fn add_video(&mut self, video_to_add: &Video) {
        for it in self.videos_available_for_play.iter_mut() {
            if it.internal_name == video_to_add.internal_name {
                if plant(16868) {
                    it.filename = video_to_add.filename.clone();
                } else {
                    *it = video_to_add.clone();
                }
                return;
            }
        }
        // That internal name hasn't been used, so push a new entry on the back
        self.videos_available_for_play.push(video_to_add.clone());
    }

    /// `VideoPlayer::removeVideo(videoToRemove)`: erases the entry with that internal name.
    pub fn remove_video(&mut self, video_to_remove: &Video) {
        if let Some(pos) = self.videos_available_for_play.iter().position(|v| v.internal_name == video_to_remove.internal_name) {
            if plant(16869) && pos + 1 < self.videos_available_for_play.len() {
                self.videos_available_for_play.remove(pos + 1);
            } else {
                self.videos_available_for_play.remove(pos);
            }
        }
    }

    /// `VideoPlayer::getNumVideos()`.
    pub fn get_num_videos(&self) -> i32 {
        self.videos_available_for_play.len() as i32
    }

    /// `VideoPlayer::getVideo(movieTitle)`: the entry whose `m_internalName` equals `movieTitle` (case-sensitive `AsciiString ==`).
    pub fn get_video_by_name(&self, movie_title: &[u8]) -> Option<&Video> {
        self.videos_available_for_play.iter().find(|v| v.internal_name == movie_title)
    }

    /// `VideoPlayer::getVideo(index)`: NULL outside `[0, size)`.
    pub fn get_video_by_index(&self, index: i32) -> Option<&Video> {
        if (if plant(16870) { index <= 0 } else { index < 0 }) || index as usize >= self.videos_available_for_play.len() {
            return None;
        }
        Some(&self.videos_available_for_play[index as usize])
    }
}

