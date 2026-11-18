//! UNTESTED DRAFT (#202, #203): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/WindowVideoManager.cpp
//!     (`WindowVideo`, `WindowVideoManager`; WindowVideoManager.h's enums and accessors)
//!
//! The movie itself is Bink video, never released and not translated (AGENTS.md): the stream and the buffer are the
//! host's ([`VideoHost`]), and every call on them is marked `// upstream: ... — device-only, not translated`.

use std::collections::BTreeMap;

use crate::client::window_manager::GameWindowManager;
use crate::client::window_script::WinId;

/// `enum WindowVideoPlayType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowVideoPlayType {
    /// `WINDOW_PLAY_MOVIE_ONCE`.
    Once,
    /// `WINDOW_PLAY_MOVIE_LOOP`.
    Loop,
    /// `WINDOW_PLAY_MOVIE_SHOW_LAST_FRAME`.
    ShowLastFrame,
}

/// `enum WindowVideoStates`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowVideoStates {
    Stop,
    Play,
    Pause,
    Hidden,
}

/// What the manager needs of the video player and the window's video buffer slot (all device-only upstream).
pub trait VideoHost {
    /// `TheVideoPlayer->open(movieName)` then `TheDisplay->createVideoBuffer()` and `allocate(width, height)`: the
    /// stream and buffer handles, or `None` when any step failed (the failed buffer and stream are released, as
    /// upstream's `delete videoBuffer; videoStream->close()`).
    fn open_stream_and_buffer(&mut self, movie_name: &[u8]) -> Option<(usize, usize)>;
    /// `win->winGetInstanceData()->setVideoBuffer(buffer)` (`None`: `NULL`).
    fn set_window_video_buffer(&mut self, wm: &mut GameWindowManager, win: WinId, buffer: Option<usize>);
    /// `delete m_videoBuffer`, `m_videoStream->close()`.
    fn release_stream_and_buffer(&mut self, stream: usize, buffer: usize);
    /// `videoStream->isFrameReady()`.
    fn is_frame_ready(&mut self, stream: usize) -> bool;
    /// `frameDecompress()`, `frameRender(videoBuffer)`, `frameNext()`.
    fn frame_decompress_render_next(&mut self, stream: usize, buffer: usize);
    /// `videoStream->frameIndex()`.
    fn frame_index(&self, stream: usize) -> i32;
}

/// `class WindowVideo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowVideo {
    pub play_type: WindowVideoPlayType,
    pub win: Option<WinId>,
    pub video_buffer: Option<usize>,
    pub video_stream: Option<usize>,
    pub movie_name: Vec<u8>,
    pub state: WindowVideoStates,
}

impl WindowVideo {
    /// `WindowVideo::WindowVideo()`.
    pub fn new() -> Self {
        WindowVideo { play_type: WindowVideoPlayType::Once, win: None, video_buffer: None, video_stream: None, movie_name: Vec::new(), state: WindowVideoStates::Stop }
    }

    /// `WindowVideo::~WindowVideo()`: "Don't Delete the window, only set it's video buffer to NULL", then the buffer
    /// and the stream are released.
    pub fn destroy<V: VideoHost>(mut self, wm: &mut GameWindowManager, v: &mut V) {
        if let Some(win) = self.win {
            v.set_window_video_buffer(wm, win, None);
        }
        self.win = None;
        if let (Some(stream), Some(buffer)) = (self.video_stream.take(), self.video_buffer.take()) {
            v.release_stream_and_buffer(stream, buffer);
        }
    }

    /// `WindowVideo::init(win, movieName, playType, videoBuffer, videoStream)`.
    pub fn init<V: VideoHost>(&mut self, wm: &mut GameWindowManager, v: &mut V, win: WinId, movie_name: &[u8], play_type: WindowVideoPlayType, video_buffer: usize, video_stream: usize) {
        self.win = Some(win);
        self.movie_name = movie_name.to_vec();
        self.play_type = play_type;
        self.video_buffer = Some(video_buffer);
        self.video_stream = Some(video_stream);
        self.state = WindowVideoStates::Play;
        v.set_window_video_buffer(wm, win, self.video_buffer);
    }

    /// `WindowVideo::setWindowState(state)`.
    pub fn set_window_state<V: VideoHost>(&mut self, wm: &mut GameWindowManager, v: &mut V, state: WindowVideoStates) {
        self.state = state;

        if self.state == WindowVideoStates::Stop {
            if let Some(win) = self.win {
                v.set_window_video_buffer(wm, win, None);
            }
        }

        if self.state == WindowVideoStates::Play || self.state == WindowVideoStates::Pause {
            if let Some(win) = self.win {
                v.set_window_video_buffer(wm, win, self.video_buffer);
            }
        }
    }
}

impl Default for WindowVideo {
    fn default() -> Self {
        Self::new()
    }
}

/// `class WindowVideoManager`. Upstream's `std::map<GameWindow *, WindowVideo *>` iterates in pointer order; the
/// window index order stands in for it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WindowVideoManager {
    pub playing_videos: BTreeMap<WinId, WindowVideo>,
    pub stop_all_movies: bool,
    pub pause_all_movies: bool,
}

impl WindowVideoManager {
    /// `WindowVideoManager::WindowVideoManager()`.
    pub fn new() -> Self {
        WindowVideoManager { playing_videos: BTreeMap::new(), stop_all_movies: false, pause_all_movies: false }
    }

    /// `WindowVideoManager::init()`.
    pub fn init(&mut self) {
        self.playing_videos.clear();
        self.stop_all_movies = false;
        self.pause_all_movies = false;
    }

    /// `WindowVideoManager::reset()` (and `~WindowVideoManager()`): every `WindowVideo` is deleted.
    pub fn reset<V: VideoHost>(&mut self, wm: &mut GameWindowManager, v: &mut V) {
        for (_, win_vid) in std::mem::take(&mut self.playing_videos) {
            win_vid.destroy(wm, v);
        }
        self.stop_all_movies = false;
        self.pause_all_movies = false;
    }

    /// `WindowVideoManager::update()`.
    pub fn update<V: VideoHost>(&mut self, wm: &mut GameWindowManager, v: &mut V) {
        if self.pause_all_movies || self.stop_all_movies {
            return;
        }

        // Iterate through the maps
        let keys: Vec<WinId> = self.playing_videos.keys().copied().collect();
        for key in keys {
            let Some(win_vid) = self.playing_videos.get(&key) else { continue };
            let Some(win) = win_vid.win else { continue };
            let state = win_vid.state;

            if state == WindowVideoStates::Hidden && !wm.is_hidden(Some(win)) {
                self.resume_movie(wm, v, win);
            }

            let state = self.playing_videos.get(&key).map_or(state, |w| w.state);
            if state == WindowVideoStates::Play && wm.is_hidden(Some(win)) {
                self.hide_movie(wm, v, win);
            }

            // Only advance the frame if we're playing
            let Some(win_vid) = self.playing_videos.get(&key) else { continue };
            if win_vid.state != WindowVideoStates::Play {
                continue;
            }

            // Get the Stream and the buffer to update for each animation
            let (stream, buffer, play_type) = (win_vid.video_stream, win_vid.video_buffer, win_vid.play_type);
            if let (Some(stream), Some(buffer)) = (stream, buffer) {
                if v.is_frame_ready(stream) {
                    v.frame_decompress_render_next(stream, buffer);

                    // If we reach frame Index of 0, we might have to pause, or loop.
                    if v.frame_index(stream) == 0 {
                        if play_type == WindowVideoPlayType::Once {
                            self.stop_movie(wm, v, win);
                        } else if play_type == WindowVideoPlayType::ShowLastFrame {
                            self.pause_movie(wm, v, win);
                        }
                    }
                }
            }
        }
    }

    /// `WindowVideoManager::playMovie(win, movieName, playType)`.
    pub fn play_movie<V: VideoHost>(&mut self, wm: &mut GameWindowManager, v: &mut V, win: WinId, movie_name: &[u8], play_type: WindowVideoPlayType) {
        // if we already have a movie playing for that window, kill it.
        self.stop_and_remove_movie(wm, v, win);

        // create the new stream and the new buffer (upstream: TheVideoPlayer->open, TheDisplay->createVideoBuffer,
        // allocate — device-only, not translated)
        let Some((stream, buffer)) = v.open_stream_and_buffer(movie_name) else { return };

        // now that we have everything, create the new WindowVideo Structure
        let mut win_vid = WindowVideo::new();

        // init it.
        win_vid.init(wm, v, win, movie_name, play_type, buffer, stream);

        // add it to our map.
        self.playing_videos.insert(win, win_vid);

        self.pause_all_movies = false;
        self.stop_all_movies = false;
    }

    /// `WindowVideoManager::pauseMovie(win)`.
    pub fn pause_movie<V: VideoHost>(&mut self, wm: &mut GameWindowManager, v: &mut V, win: WinId) {
        if let Some(w) = self.playing_videos.get_mut(&win) {
            w.set_window_state(wm, v, WindowVideoStates::Pause);
        }
    }

    /// `WindowVideoManager::hideMovie(win)`.
    pub fn hide_movie<V: VideoHost>(&mut self, wm: &mut GameWindowManager, v: &mut V, win: WinId) {
        if let Some(w) = self.playing_videos.get_mut(&win) {
            w.set_window_state(wm, v, WindowVideoStates::Hidden);
        }
    }

    /// `WindowVideoManager::resumeMovie(win)`.
    pub fn resume_movie<V: VideoHost>(&mut self, wm: &mut GameWindowManager, v: &mut V, win: WinId) {
        if let Some(w) = self.playing_videos.get_mut(&win) {
            w.set_window_state(wm, v, WindowVideoStates::Play);
        }
        self.pause_all_movies = false;
        self.stop_all_movies = false;
    }

    /// `WindowVideoManager::stopMovie(win)`.
    pub fn stop_movie<V: VideoHost>(&mut self, wm: &mut GameWindowManager, v: &mut V, win: WinId) {
        if let Some(w) = self.playing_videos.get_mut(&win) {
            w.set_window_state(wm, v, WindowVideoStates::Stop);
        }
    }

    /// `WindowVideoManager::stopAndRemoveMovie(win)`.
    pub fn stop_and_remove_movie<V: VideoHost>(&mut self, wm: &mut GameWindowManager, v: &mut V, win: WinId) {
        if let Some(w) = self.playing_videos.remove(&win) {
            w.destroy(wm, v);
        }
    }

    /// `WindowVideoManager::stopAllMovies()`.
    pub fn stop_all_movies<V: VideoHost>(&mut self, wm: &mut GameWindowManager, v: &mut V) {
        for w in self.playing_videos.values_mut() {
            w.set_window_state(wm, v, WindowVideoStates::Stop);
        }
        self.stop_all_movies = true;
        self.pause_all_movies = false;
    }

    /// `WindowVideoManager::pauseAllMovies()`.
    pub fn pause_all_movies<V: VideoHost>(&mut self, wm: &mut GameWindowManager, v: &mut V) {
        for w in self.playing_videos.values_mut() {
            w.set_window_state(wm, v, WindowVideoStates::Pause);
        }
        self.pause_all_movies = true;
        self.stop_all_movies = false;
    }

    /// `WindowVideoManager::resumeAllMovies()`.
    pub fn resume_all_movies<V: VideoHost>(&mut self, wm: &mut GameWindowManager, v: &mut V) {
        for w in self.playing_videos.values_mut() {
            w.set_window_state(wm, v, WindowVideoStates::Play);
        }
        self.stop_all_movies = false;
        self.pause_all_movies = false;
    }

    /// `WindowVideoManager::getWinState(win)`: `WINDOW_VIDEO_STATE_STOP` for a window with no movie.
    pub fn get_win_state(&self, win: WinId) -> WindowVideoStates {
        self.playing_videos.get(&win).map_or(WindowVideoStates::Stop, |w| w.state)
    }
}
