//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/Display.cpp   (`deleteViews`, `attachView`, `drawViews`,
//!     `updateViews`, `playLogoMovie`, `setDebugDisplayCallback`, `getDebugDisplayCallback`)
//!   GeneralsMD/Code/GameEngine/Source/GameClient/View.cpp      (`View::prependViewToList`)
//!
//! Upstream's views are an intrusive singly linked list (`m_viewList`, `View::m_next`); here a vector of view ids with
//! the head first (`attachView` prepends). A view's own `drawView()` / `updateView()` are the closures. The Bink video
//! stream calls of `playLogoMovie` are device-only and are not translated.

/// A `View*` as an id.
pub type ViewId = u32;

/// `View::prependViewToList(list)` (View.cpp:122): `m_next = list; return this;` — the new list with this view at the head.
pub fn prepend_view_to_list(this: ViewId, list: &[ViewId]) -> Vec<ViewId> {
    let mut v = Vec::with_capacity(list.len() + 1);
    v.push(this);
    v.extend_from_slice(list);
    v
}

/// The Display's movie and view members.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisplayViews {
    /// `m_viewList`, head first.
    pub view_list: Vec<ViewId>,
    /// `m_videoStream != NULL`.
    pub has_video_stream: bool,
    /// `m_currentlyPlayingMovie`.
    pub currently_playing_movie: Vec<u8>,
    /// `m_movieHoldTime`.
    pub movie_hold_time: i32,
    /// `m_copyrightHoldTime`.
    pub copyright_hold_time: i32,
    /// `m_elapsedMovieTime`.
    pub elapsed_movie_time: u32,
    /// `m_debugDisplayCallback` as an opaque id (`None` for NULL) and `m_debugDisplayUserData`.
    pub debug_display_callback: Option<u32>,
    /// `m_debugDisplayUserData`.
    pub debug_display_user_data: Option<u32>,
}

impl DisplayViews {
    /// `Display::deleteViews()` (Display.cpp:89): delete every view, head to tail, and null the list.
    pub fn delete_views(&mut self, delete_view: &mut dyn FnMut(ViewId)) {
        for &v in self.view_list.iter() {
            delete_view(v);
        }
        self.view_list.clear();
    }

    /// `Display::attachView(view)` (Display.cpp:105): prepend to the head of the list.
    pub fn attach_view(&mut self, view: ViewId) {
        self.view_list = prepend_view_to_list(view, &self.view_list);
    }

    /// `Display::drawViews()` (Display.cpp:114): every view's `drawView()`, head to tail.
    pub fn draw_views(&self, draw_view: &mut dyn FnMut(ViewId)) {
        for &v in self.view_list.iter() {
            draw_view(v);
        }
    }

    /// `Display::updateViews()` (Display.cpp:126): every view's `updateView()`.
    pub fn update_views(&self, update_view: &mut dyn FnMut(ViewId)) {
        for &v in self.view_list.iter() {
            update_view(v);
        }
    }

    /// `Display::playLogoMovie(movieName, minMovieLength, minCopyrightLength)` (Display.cpp:203). `open` is
    /// `TheVideoPlayer->open(movieName)`; `time_get_time` is `timeGetTime()` ("legal want's actual Seconds").
    pub fn play_logo_movie(&mut self, stop_movie: &mut dyn FnMut(&mut DisplayViews), open: &mut dyn FnMut(&[u8]) -> bool, time_get_time: u32, movie_name: &[u8], min_movie_length: i32, min_copyright_length: i32) {
        stop_movie(self);
        self.has_video_stream = open(movie_name);
        if !self.has_video_stream {
            return;
        }
        self.currently_playing_movie = movie_name.to_vec();
        self.movie_hold_time = min_movie_length;
        self.copyright_hold_time = min_copyright_length;
        self.elapsed_movie_time = time_get_time;
        // upstream: m_videoBuffer = createVideoBuffer(); m_videoBuffer->allocate(width, height) — device-only, not
        // translated (a failed allocation calls stopMovie() and returns)
    }

    /// `Display::setDebugDisplayCallback(callback, userData)` (Display.cpp:372).
    pub fn set_debug_display_callback(&mut self, callback: Option<u32>, user_data: Option<u32>) {
        self.debug_display_callback = callback;
        self.debug_display_user_data = user_data;
    }

    /// `Display::getDebugDisplayCallback()` (Display.cpp:382).
    pub fn get_debug_display_callback(&self) -> Option<u32> {
        self.debug_display_callback
    }
}
