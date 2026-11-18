//! UNTESTED DRAFT (#202, #205): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/GUICallbacks/W3DMainMenu.cpp
//!
//! The main menu's draw callbacks, in the port's style (`crate::client::window_draw`): each appends
//! [`DrawItem`]s for the client to render; the names are for `DrawFunc::Host(name)`. Reused: `win_draw_border`
//! (`window.winDrawBorder()`), `GuiHost::{text_size, image_size, has_image}`, `game_make_color`.
//!
//! Upstream statics are kept as [`MainMenuDrawState`]: `advancePosition`'s function statics (`goingForward`, `Width`,
//! `x`, `y`, `m_startTime`, each set ONCE, on the first call: `Width` and `y` from the first call's size and image),
//! and `clipRegion` (a file static, zero: its only assignment is inside `W3DMainMenuInit`'s block comment).
//! `timeGetTime()` and the wall clock (`localtime`) come in through [`MainMenuEnv`].
//! `DrawItem::Line` has no width in the port (upstream passes 2, 1 or 3 pixels): the widths are noted per call.
//! The commented-out second half of the file (`LogoGlow`) is dead upstream and not translated.

use rts_lockstep_engine::client::window_draw::win_draw_border;
use rts_lockstep_engine::client::window_manager::{DrawItem, GameWindowManager, GuiHost, WIN_STATE_HILITED, WIN_STATE_SELECTED};
use rts_lockstep_engine::client::window_script::{win_status, WinId, WIN_COLOR_UNDEFINED};
use rts_lockstep_engine::common::ini::game_make_color;

/// `static Color BrownishColor = GameMakeColor(167,134,94,255)`.
pub fn brownish_color() -> i32 {
    game_make_color(167, 134, 94, 255)
}

/// What the callbacks read of the running game.
#[derive(Clone, Copy, Debug, Default)]
pub struct MainMenuEnv {
    /// `timeGetTime()`.
    pub now_ms: u32,
    /// `TheDisplay->getHeight()`.
    pub display_height: i32,
    /// `localtime`: hour, minute, second.
    pub local_time: (u32, u32, u32),
}

/// The file's statics.
#[derive(Clone, Debug)]
pub struct MainMenuDrawState {
    /// `advancePosition`'s `goingForward`.
    pub going_forward: bool,
    /// `advancePosition`'s `Width`, `x`, `y`, `m_startTime`: None until the first call sets them.
    pub width: Option<i32>,
    pub x: i32,
    pub y: i32,
    pub start_time: u32,
    /// `static IRegion2D clipRegion` (lo.x, lo.y, hi.x, hi.y): zero.
    pub clip_region: (i32, i32, i32, i32),
}

impl Default for MainMenuDrawState {
    fn default() -> Self {
        MainMenuDrawState { going_forward: true, width: None, x: -800, y: 0, start_time: 0, clip_region: (0, 0, 0, 0) }
    }
}

fn line(out: &mut Vec<DrawItem>, r: (i32, i32, i32, i32), color: i32) {
    out.push(DrawItem::Line { x0: r.0, y0: r.1, x1: r.2, y1: r.3, color });
}

/// `advancePosition(window = NULL, image, posX, posY, sizeX, sizeY)` (W3DMainMenu.cpp:89): the pulse image sweeps
/// across the menu's bar in 10 seconds, then back along the bottom. Only the `window == NULL` form is called.
pub fn advance_position<H: GuiHost>(wm: &GameWindowManager, h: &H, st: &mut MainMenuDrawState, env: &MainMenuEnv, image: Option<&[u8]>, pos_x: i32, pos_y: i32, size_x: i32, size_y: i32, out: &mut Vec<DrawItem>) {
    let Some(image) = image else { return };
    let (iw, ih) = h.image_size(image).unwrap_or((0, 0));
    let (pos, size) = ((pos_x, pos_y), (size_x, size_y));
    // the statics are set once, on the first call
    if st.width.is_none() {
        st.width = Some(size.0 + iw);
        st.x = -800;
        st.y = pos.1 - (ih / 2);
        st.start_time = env.now_ms;
    }
    let width = st.width.unwrap();
    let time = env.now_ms.wrapping_sub(st.start_time) as i32;
    let percent_done = time as f32 / 10000.0;
    if st.going_forward {
        if percent_done >= 1.0 {
            st.y = pos.1 + size.1 - (ih / 2);
            st.start_time = env.now_ms;
            st.going_forward = false;
        } else {
            st.y = pos.1 - (ih / 2);
            st.x = (percent_done * width as f32) as i32 - iw;
        }
    } else if percent_done >= 1.0 {
        st.y = pos.1 - (ih / 2);
        st.start_time = env.now_ms;
        st.going_forward = true;
    } else {
        st.y = pos.1 + size.1 - (ih / 2);
        st.x = size.0 - (percent_done * width as f32) as i32;
    }
    let _ = wm;
    out.push(DrawItem::Image { image: image.to_vec(), x0: st.x, y0: st.y, x1: st.x + iw, y1: st.y + ih, color: 0xFFFF_FFFF, grayscale: false, clip: None });
}

/// `W3DShellMenuSchemeDraw` (W3DMainMenu.cpp:150): `if (TheShell && TheShell->isShellActive())
/// TheShell->getShellMenuSchemeManager()->draw()`. `draw_scheme`: that call (the scheme manager is the shell's).
pub fn w3d_shell_menu_scheme_draw(shell_active: bool, draw_scheme: &mut dyn FnMut()) {
    if shell_active {
        draw_scheme();
    }
}

/// The frame both menu draws share; `verticals` are the x fractions of the full-height lines (each 3 pixels wide).
fn main_menu_frame<H: GuiHost>(wm: &GameWindowManager, h: &H, st: &mut MainMenuDrawState, env: &MainMenuEnv, w: WinId, verticals: &[f64], out: &mut Vec<DrawItem>) {
    //R:83 G:78 B:52
    let color = brownish_color();
    let color_drop = game_make_color(38, 30, 21, 255);
    let (pos, size) = (wm.tree.screen_position(w), wm.tree.windows[w].size);
    let height = env.display_height;

    // IRegion2D { lo.x, lo.y, hi.x, hi.y }; the products are `Int = pos.y + (size.y * .1)` (double, truncated)
    let top1 = (pos.0, pos.1, pos.0 + size.0, pos.1);
    let top1_drop = (pos.0, pos.1 + 1, pos.0 + size.0, pos.1 + 1);
    let y_t2 = pos.1 + (size.1 as f64 * 0.1) as i32;
    let top2 = (pos.0, y_t2, pos.0 + size.0, y_t2);
    let y_t2d = pos.1 + (size.1 as f64 * 0.12) as i32;
    let top2_drop = (pos.0, y_t2d, pos.0 + size.0, y_t2d);
    let y_b1 = pos.1 + (size.1 as f64 * 0.9) as i32;
    let bottom1 = (pos.0, y_b1, pos.0 + size.0, y_b1);
    let y_b1d = pos.1 + (size.1 as f64 * 0.92) as i32;
    let bottom1_drop = (pos.0, y_b1d, pos.0 + size.0, y_b1d);
    let bottom2 = (pos.0, pos.1 + size.1, pos.0 + size.0, pos.1 + size.1);
    let bottom2_drop = (pos.0, pos.1 + size.1 + 1, pos.0 + size.0, pos.1 + size.1 + 1);

    line(out, top1, color); // width 2
    line(out, top1_drop, color_drop); // width 2
    line(out, top2, color); // width 1
    line(out, top2_drop, color_drop); // width 1
    line(out, bottom1, color); // width 1
    line(out, bottom1_drop, color_drop); // width 1
    line(out, bottom2, color); // width 2
    line(out, bottom2_drop, color_drop); // width 2
    for f in verticals {
        let x = pos.0 + (size.0 as f64 * f) as i32;
        line(out, (x, pos.1, x, height), color); // width 3
    }
    advance_position(wm, h, st, env, Some(b"MainMenuPulse"), pos.0, pos.1, size.0, size.1, out);
}

/// `W3DMainMenuDraw` (W3DMainMenu.cpp:156): four vertical lines.
pub fn w3d_main_menu_draw<H: GuiHost>(wm: &GameWindowManager, h: &H, st: &mut MainMenuDrawState, env: &MainMenuEnv, w: WinId, out: &mut Vec<DrawItem>) {
    main_menu_frame(wm, h, st, env, w, &[0.225, 0.445, 0.6662, 0.885], out);
}

/// `W3DMainMenuFourDraw` (W3DMainMenu.cpp:220): three vertical lines (the third of the four is commented out).
pub fn w3d_main_menu_four_draw<H: GuiHost>(wm: &GameWindowManager, h: &H, st: &mut MainMenuDrawState, env: &MainMenuEnv, w: WinId, out: &mut Vec<DrawItem>) {
    main_menu_frame(wm, h, st, env, w, &[0.295, 0.59, 0.885], out);
}

/// `W3DMetalBarMenuDraw` (W3DMainMenu.cpp:285): `window->winDrawBorder()`.
pub fn w3d_metal_bar_menu_draw<H: GuiHost>(wm: &GameWindowManager, h: &H, w: WinId, out: &mut Vec<DrawItem>) {
    win_draw_border(wm, h, w, out);
}

/// `W3DClockDraw` (W3DMainMenu.cpp:352): `W3DGameWinDefaultDraw`, then the time "%H:%M:%S" centred in Arial 16 with a
/// black drop, clipped one pixel inside the window. `set_window_text` receives the text upstream stores in the
/// window's instance data (`instData->setText`); `text_size` is `dString->getSize` of that text.
pub fn w3d_clock_draw<H: GuiHost>(wm: &GameWindowManager, h: &H, env: &MainMenuEnv, w: WinId, out: &mut Vec<DrawItem>, set_window_text: &mut dyn FnMut(&[u16])) {
    rts_lockstep_engine::client::window_draw::w3d_game_win_default_draw(wm, w, out);
    let (pos, size) = (wm.tree.screen_position(w), wm.tree.windows[w].size);

    let (hh, mm, ss) = env.local_time;
    let datestr = format!("{:02}:{:02}:{:02}", hh, mm, ss); // strftime(datestr, 256, "%H:%M:%S", curtime)
    let temp: Vec<u16> = datestr.bytes().map(|b| b as u16).collect();
    set_window_text(&temp);

    // dString->setFont(TheFontLibrary->getFont("Arial", 16, 0)): the font is the host's lookup
    let (text_width, text_height) = h.text_size(None, &temp, None);
    // Init the clip region
    let clock_clip = (pos.0 + 1, pos.1 + 1, pos.0 + size.0 - 1, pos.1 + size.1 - 1);
    let text_pos = (pos.0 + (size.0 / 2) - (text_width / 2), pos.1 + (size.1 / 2) - (text_height / 2));
    out.push(DrawItem::Text {
        text: temp,
        x: text_pos.0,
        y: text_pos.1,
        color: game_make_color(255, 255, 255, 255),
        drop_color: game_make_color(0, 0, 0, 255),
        font: None, // Arial 16
        word_wrap: None,
        wrap_centered: false,
        hotkey: false,
        clip: Some(clock_clip),
    });
}

/// `W3DMainMenuMapBorder` (W3DMainMenu.cpp:399): the frame around the map with the `FrameCorner*` pieces.
pub fn w3d_main_menu_map_border<H: GuiHost>(wm: &GameWindowManager, h: &H, w: WinId, out: &mut Vec<DrawItem>) {
    const BORDER_CORNER_SIZE: i32 = 10;
    const BORDER_LINE_SIZE: i32 = 20;
    let img = |out: &mut Vec<DrawItem>, name: &[u8], x0: i32, y0: i32, x1: i32, y1: i32| {
        // Display::drawImage of a NULL image draws nothing
        if h.has_image(name) {
            out.push(DrawItem::Image { image: name.to_vec(), x0, y0, x1, y1, color: 0xFFFF_FFFF, grayscale: false, clip: wm.clip });
        }
    };

    // save original x, y
    let (mut x, mut y);
    let (pos, sz) = (wm.tree.screen_position(w), wm.tree.windows[w].size);
    let (width, height) = sz;
    let original_x = pos.0;
    let original_y = pos.1;
    let max_x = pos.0 + width;
    let max_y = pos.1 + height;
    let mut x2;
    let mut y2;
    let size = 20;
    let half_size = size / 2;

    // Draw Horizontal Lines
    // All border pieces are based on a 10 pixel offset from the centerline
    y = original_y - 10;
    y2 = max_y - 10;
    x2 = max_x - (10 + BORDER_LINE_SIZE);
    if h.has_image(b"FrameCornerHorizontal") {
        let name: &[u8] = b"FrameCornerHorizontal";
        x = original_x + 10;
        while x <= x2 {
            img(out, name, x, y, x + size, y + size);
            img(out, name, x, y2, x + size, y2 + size);
            x += BORDER_LINE_SIZE;
        }
        x2 = max_x - BORDER_CORNER_SIZE;

        // x == place to draw remainder if any
        if (x2 - x) >= (BORDER_LINE_SIZE / 2) {
            //Blit Half piece
            img(out, name, x, y, x + half_size, y + size);
            img(out, name, x, y2, x + half_size, y2 + size);
            x += BORDER_LINE_SIZE / 2;
        }

        // x2 - x ... must now be less than a half piece
        // check for equals and if not blit an adjusted half piece border pieces have
        // a two pixel repeat so we will blit one pixel over if necessary to line up
        // the art, but we'll cover-up the overlap with the corners
        if x < x2 {
            x -= (BORDER_LINE_SIZE / 2) - (((x2 - x) + 1) & !1);
            //Blit Half piece
            img(out, name, x, y, x + half_size, y + size);
            img(out, name, x, y2, x + half_size, y2 + size);
        }
    }

    if h.has_image(b"FrameCornerVertical") {
        let name: &[u8] = b"FrameCornerVertical";
        // Draw Vertical Lines
        // All border pieces are based on a 10 pixel offset from the centerline
        x = original_x - 10;
        x2 = max_x - 10;
        y2 = max_y - (10 + BORDER_LINE_SIZE);
        y = original_y + 10;
        while y <= y2 {
            img(out, name, x, y, x + size, y + size);
            img(out, name, x2, y, x2 + size, y + size);
            y += BORDER_LINE_SIZE;
        }
        y2 = max_y - BORDER_CORNER_SIZE;

        // y == place to draw remainder if any
        if (y2 - y) >= (BORDER_LINE_SIZE / 2) {
            //Blit Half piece
            img(out, name, x, y, x + size, y + half_size);
            img(out, name, x2, y, x2 + size, y + half_size);
            y += BORDER_LINE_SIZE / 2;
        }

        // y2 - y ... must now be less than a half piece
        if y < y2 {
            y -= (BORDER_LINE_SIZE / 2) - (((y2 - y) + 1) & !1);
            //Blit Half piece
            img(out, name, x, y, x + size, y + half_size);
            img(out, name, x2, y, x2 + size, y + half_size);
        }
    }

    // Draw Corners
    x = original_x - BORDER_CORNER_SIZE;
    y = original_y - BORDER_CORNER_SIZE;
    img(out, b"FrameCornerUL", x, y, x + size, y + size);
    x = max_x - BORDER_CORNER_SIZE;
    y = original_y - BORDER_CORNER_SIZE;
    img(out, b"FrameCornerUR", x, y, x + size, y + size);
    x = original_x - BORDER_CORNER_SIZE;
    y = max_y - BORDER_CORNER_SIZE;
    img(out, b"FrameCornerLL", x, y, x + size, y + size);
    x = max_x - BORDER_CORNER_SIZE;
    y = max_y - BORDER_CORNER_SIZE;
    img(out, b"FrameCornerLR", x, y, x + size, y + size);

    // TheDisplay->enableClipping(FALSE): the clip is cleared by the caller
}

/// `W3DMainMenuButtonDropShadowDraw` (W3DMainMenu.cpp:558): a three-piece push button whose centre clip uses the file's
/// `clipRegion` (zero) for its y range. The image set per state (disabled / hilite / enabled, selected or not) is
/// `push_button_image_draw_three`'s (left 0|1, right 6|4, middle 5|3 of the state's set).
pub fn w3d_main_menu_button_drop_shadow_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, st: &MainMenuDrawState, w: WinId, out: &mut Vec<DrawItem>) {
    let (origin, size) = (wm.tree.screen_position(w), wm.tree.windows[w].size);
    let win = &wm.tree.windows[w];
    // get image offset
    let (x_offset, y_offset) = win.inst.image_offset;
    let selected = win.inst.state as u32 & WIN_STATE_SELECTED as u32 != 0;
    let enabled = win.status & win_status::ENABLED != 0;
    let hilited = win.inst.state as u32 & WIN_STATE_HILITED as u32 != 0;
    let data = if !enabled { &win.inst.disabled_draw_data } else if hilited { &win.inst.hilite_draw_data } else { &win.inst.enabled_draw_data };
    let (l, r, m) = if selected { (1, 4, 3) } else { (0, 6, 5) };
    let (left_image, right_image, center_image) = (data[l].image.clone(), data[r].image.clone(), data[m].image.clone());

    // sanity, we need to have these images to make it look right
    let (Some(left_image), Some(right_image), Some(center_image)) = (left_image, right_image, center_image) else { return };

    // get image sizes for the ends
    let left_size = h.image_size(&left_image).unwrap_or((0, 0));
    let right_size = h.image_size(&right_image).unwrap_or((0, 0));
    let center_w = h.image_size(&center_image).unwrap_or((0, 0)).0;

    // get two key points used in the end drawing
    let left_end = (origin.0 + left_size.0 + x_offset, origin.1 + size.1 + y_offset);
    let right_start = (origin.0 + size.0 - right_size.0 + x_offset, origin.1 + y_offset);

    let mut push = |out: &mut Vec<DrawItem>, name: &Vec<u8>, x0: i32, y0: i32, x1: i32, y1: i32, clip: Option<(i32, i32, i32, i32)>| {
        out.push(DrawItem::Image { image: name.clone(), x0, y0, x1, y1, color: 0xFFFF_FFFF, grayscale: false, clip });
    };

    // get width we have to draw our repeating center in
    let center_width = right_start.0 - left_end.0;
    if center_width <= 0 {
        // draw left end
        let start = (origin.0 + x_offset, origin.1 + y_offset);
        let end = (origin.0 + x_offset + size.0 / 2, left_end.1);
        push(out, &left_image, start.0, start.1, end.0, end.1, None);

        // draw right end
        let start = (end.0, right_start.1);
        let end = (origin.0 + size.0, start.1 + size.1);
        push(out, &right_image, start.0, start.1, end.0, end.1, None);
    } else {
        // how many whole repeating pieces will fit in that width
        let pieces = if center_w != 0 { center_width / center_w } else { 0 };

        // draw the pieces
        let mut start = (left_end.0, origin.1 + y_offset);
        let end_y = start.1 + size.1 + y_offset;
        for _ in 0..pieces {
            let end_x = start.0 + center_w;
            push(out, &center_image, start.0, start.1, end_x, end_y, None);
            start.0 += center_w;
        }

        // we will draw the image but clip the parts we don't want to show
        let reg = (start.0, st.clip_region.1, right_start.0, st.clip_region.3);
        let center_width = right_start.0 - start.0;
        if center_width > 0 {
            let end_x = start.0 + center_w;
            push(out, &center_image, start.0, start.1, end_x, end_y, Some(reg));
        }

        // draw left end
        let start = (origin.0 + x_offset, origin.1 + y_offset);
        push(out, &left_image, start.0, start.1, left_end.0, left_end.1, None);

        // draw right end
        let start = right_start;
        push(out, &right_image, start.0, start.1, start.0 + right_size.0, start.1 + size.1, None);
    }

    // draw the button text
    draw_text(wm, h, w, out);

    // (the video buffer, overlay image, clock and special border that follow are the push button's extras:
    // `window_draw::push_button_extras`, drawn by the caller after this)
}

/// `drawText` (W3DMainMenu.cpp:781): the button's text, word wrapped at the width, centred in the window.
pub fn draw_text<H: GuiHost>(wm: &GameWindowManager, h: &H, w: WinId, out: &mut Vec<DrawItem>) {
    let win = &wm.tree.windows[w];
    let Some(text) = win.text.as_ref().filter(|t| !t.is_empty()) else { return };
    let (origin, size) = (wm.tree.screen_position(w), win.size);
    let centered = win.inst.status & win_status::WRAP_CENTERED != 0;

    // get the right text color
    let enabled = win.status & win_status::ENABLED != 0;
    let (text_color, drop_color) = if !enabled {
        (win.inst.disabled_text.color, win.inst.disabled_text.border_color)
    } else if win.inst.state as u32 & WIN_STATE_HILITED as u32 != 0 {
        (win.inst.hilite_text.color, win.inst.hilite_text.border_color)
    } else {
        (win.inst.enabled_text.color, win.inst.enabled_text.border_color)
    };

    // get text size
    let (width, height) = h.text_size(win.inst.font.as_ref(), text, Some(size.0));

    // where to draw
    let text_pos = (origin.0 + (size.0 / 2) - (width / 2), origin.1 + (size.1 / 2) - (height / 2));
    out.push(DrawItem::Text { text: text.clone(), x: text_pos.0, y: text_pos.1, color: text_color, drop_color, font: win.inst.font.clone(), word_wrap: Some(size.0), wrap_centered: centered, hotkey: false, clip: None });
}

/// `W3DMainMenuRandomTextDraw` (W3DMainMenu.cpp:835): the random text, vertically centred, clipped inside the window,
/// in the disabled colours. `text`: `tData->text` (None for `NULL`: upstream returns).
pub fn w3d_main_menu_random_text_draw<H: GuiHost>(wm: &GameWindowManager, h: &H, w: WinId, text: Option<&[u16]>, out: &mut Vec<DrawItem>) {
    let win = &wm.tree.windows[w];
    let (origin, size) = (wm.tree.screen_position(w), win.size);
    let text_color = win.inst.disabled_text.color;
    let text_outline_color = win.inst.disabled_text.border_color;

    // draw the text
    let Some(text) = text else { return };
    if text_color == WIN_COLOR_UNDEFINED {
        return;
    }
    // Init the clip region
    let textclip = (origin.0 + 1, origin.1 + 1, origin.0 + size.0 - 1, origin.1 + size.1 - 1);

    // how much space will this text take up
    let (_text_width, text_height) = h.text_size(win.inst.font.as_ref(), text, None);

    // draw the text
    let text_pos = (origin.0, origin.1 + (size.1 / 2) - (text_height / 2));
    out.push(DrawItem::Text { text: text.to_vec(), x: text_pos.0, y: text_pos.1, color: text_color, drop_color: text_outline_color, font: win.inst.font.clone(), word_wrap: None, wrap_centered: false, hotkey: false, clip: Some(textclip) });
}

/// `W3DThinBorderDraw` (W3DMainMenu.cpp:869): the enabled image 0 stretched over the window (plus the image offset).
pub fn w3d_thin_border_draw(wm: &GameWindowManager, w: WinId, out: &mut Vec<DrawItem>) {
    let win = &wm.tree.windows[w];
    let (start, size) = (wm.tree.screen_position(w), win.size);
    if let Some(image) = win.inst.enabled_draw_data[0].image.clone() {
        let begin = (start.0 + win.inst.image_offset.0, start.1 + win.inst.image_offset.1);
        let end = (begin.0 + size.0, begin.1 + size.1);
        out.push(DrawItem::Image { image, x0: begin.0, y0: begin.1, x1: end.0, y1: end.1, color: 0xFFFF_FFFF, grayscale: false, clip: wm.clip });
    }
    // TheDisplay->enableClipping(FALSE)
}

/// `W3DMainMenuInit` (W3DMainMenu.cpp:897): everything but `MainMenuInit(layout, userData)` is a block comment
/// upstream, so this is that call (the host's).
pub fn w3d_main_menu_init(main_menu_init: &mut dyn FnMut()) {
    main_menu_init();
}

/// `W3DCreditsMenuDraw` (W3DMainMenu.cpp:999): `TheCredits->draw()`.
pub fn w3d_credits_menu_draw(credits_draw: &mut dyn FnMut()) {
    credits_draw();
}
