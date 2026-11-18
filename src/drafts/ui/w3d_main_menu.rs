//! UNTESTED DRAFT (#202, #203, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/GUICallbacks/W3DMainMenu.cpp
//!     (`advancePosition`, `W3DShellMenuSchemeDraw`, `W3DMainMenuDraw`, `W3DMainMenuFourDraw`,
//!      `W3DMetalBarMenuDraw`, `W3DClockDraw`, `W3DMainMenuMapBorder`, `W3DMainMenuButtonDropShadowDraw`,
//!      `drawText`, `W3DMainMenuRandomTextDraw`, `W3DThinBorderDraw`, `W3DMainMenuInit`, `W3DCreditsMenuDraw`)
//!
//! Each draw callback appends [`DrawItem`]s, as [`crate::client::window_draw`] does. `drawLine`'s width has no field in
//! [`DrawItem::Line`]: the width is kept in a comment at each call. The main menu's `static IRegion2D clipRegion` is
//! never assigned (its assigning code sits in a comment), so it is all zero, as in upstream.

use crate::client::window_manager::{DrawItem, GameWindowManager, GuiHost, UserData, WIN_STATE_HILITED, WIN_STATE_SELECTED};
use crate::client::window_script::{win_status, WinId, WIN_COLOR_UNDEFINED};

/// `BrownishColor = GameMakeColor(167,134,94,255)` (ARGB).
pub const BROWNISH_COLOR: i32 = 0xFFA7_865Eu32 as i32;
/// `GameMakeColor(38,30,21,255)`.
pub const COLOR_DROP: i32 = 0xFF26_1E15u32 as i32;
/// `GameMakeColor(255,255,255,255)`.
const WHITE: i32 = 0xFFFF_FFFFu32 as i32;
/// `GameMakeColor(0,0,0,255)`.
const BLACK: i32 = 0xFF00_0000u32 as i32;

/// `advancePosition`'s function statics: `goingForward`, `Width`, `x`, `y`, `m_startTime` (all set once, on the first
/// call that has an image).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdvancePositionState {
    pub going_forward: bool,
    /// `None` until the first call initialises `Width`, `x`, `y` and `m_startTime`.
    pub init: Option<AdvanceInit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdvanceInit {
    pub width: i32,
    pub x: i32,
    pub y: i32,
    pub start_time: u32,
}

impl Default for AdvancePositionState {
    fn default() -> Self {
        AdvancePositionState { going_forward: true, init: None }
    }
}

/// What the main menu draws reach outside the window manager.
pub trait W3DMainMenuHost: GuiHost {
    /// `timeGetTime()`.
    fn time_get_time(&self) -> u32;
    /// `TheDisplay->getHeight()`.
    fn display_height(&self) -> i32;
    fn advance_position_state(&mut self) -> &mut AdvancePositionState;
    /// `TheShell->isShellActive()` then `getShellMenuSchemeManager()->draw()`.
    fn shell_menu_scheme_draw(&mut self, out: &mut Vec<DrawItem>);
    /// `TheCredits->draw()`.
    fn credits_draw(&mut self, out: &mut Vec<DrawItem>);
    /// `strftime(datestr, 256, "%H:%M:%S", localtime(...))`.
    fn local_time_hh_mm_ss(&self) -> String;
    /// `MainMenuInit(layout, userData)` ([`crate::client::main_menu::main_menu_init`]).
    fn main_menu_init(&mut self, wm: &mut GameWindowManager, layout: usize);
}

/// `advancePosition(window, image, posX, posY, sizeX, sizeY)` (W3DMainMenu.cpp:62), with `window == NULL` (both callers).
fn advance_position<H: W3DMainMenuHost>(wm: &GameWindowManager, h: &mut H, image: &[u8], pos: (i32, i32), size: (i32, i32), out: &mut Vec<DrawItem>) {
    let Some((iw, ih)) = h.image_size(image) else { return };
    let now = h.time_get_time();

    let st = h.advance_position_state();
    let init = st.init.get_or_insert_with(|| AdvanceInit { width: size.0 + iw, x: -800, y: pos.1 - (ih / 2), start_time: now });
    let time = now.wrapping_sub(init.start_time) as i32;
    let percent_done = time as f32 / 10000.0;

    if st.going_forward {
        if percent_done >= 1.0 {
            init.y = pos.1 + size.1 - (ih / 2);
            init.start_time = now;
            st.going_forward = false;
        } else {
            init.y = pos.1 - (ih / 2);
            init.x = (percent_done * init.width as f32) as i32 - iw;
        }
    } else if percent_done >= 1.0 {
        init.y = pos.1 - (ih / 2);
        init.start_time = now;
        st.going_forward = true;
    } else {
        init.y = pos.1 + size.1 - (ih / 2);
        init.x = size.0 - (percent_done * init.width as f32) as i32;
    }
    let (x, y) = (init.x, init.y);
    out.push(DrawItem::Image { image: image.to_vec(), x0: x, y0: y, x1: x + iw, y1: y + ih, color: 0xFFFF_FFFF, grayscale: false, clip: wm.clip });
}

/// `W3DShellMenuSchemeDraw(window, instData)`.
pub fn w3d_shell_menu_scheme_draw<H: W3DMainMenuHost>(h: &mut H, out: &mut Vec<DrawItem>) {
    h.shell_menu_scheme_draw(out);
}

fn line(out: &mut Vec<DrawItem>, x0: i32, y0: i32, x1: i32, y1: i32, _width: i32, color: i32) {
    out.push(DrawItem::Line { x0, y0, x1, y1, color });
}

/// The frame both `W3DMainMenuDraw` and `W3DMainMenuFourDraw` draw; `verticals` are the fractions of the width that
/// get a 3-wide vertical line from `pos.y` to the display height.
fn main_menu_frame<H: W3DMainMenuHost>(wm: &GameWindowManager, h: &mut H, window: WinId, verticals: &[f32], out: &mut Vec<DrawItem>) {
    let color = BROWNISH_COLOR;
    let color_drop = COLOR_DROP;
    let height = h.display_height();

    let (px, py) = wm.tree.screen_position(window);
    let (sx, sy) = wm.tree.windows[window].size;
    let f = |v: f32, k: f32| (v * k) as i32; // `pos.y + (size.y * .1)`: the product is a double, truncated on the store

    let top1 = (px, py, px + sx, py);
    let top1_drop = (px, py + 1, px + sx, py + 1);
    let top2 = (px, py + f(sy as f32, 0.1), px + sx, py + f(sy as f32, 0.1));
    let top2_drop = (px, py + f(sy as f32, 0.12), px + sx, py + f(sy as f32, 0.12));
    let bottom1 = (px, py + f(sy as f32, 0.9), px + sx, py + f(sy as f32, 0.9));
    let bottom1_drop = (px, py + f(sy as f32, 0.92), px + sx, py + f(sy as f32, 0.92));
    let bottom2 = (px, py + sy, px + sx, py + sy);
    let bottom2_drop = (px, py + sy + 1, px + sx, py + sy + 1);

    line(out, top1.0, top1.1, top1.2, top1.3, 2, color);
    line(out, top1_drop.0, top1_drop.1, top1_drop.2, top1_drop.3, 2, color_drop);
    line(out, top2.0, top2.1, top2.2, top2.3, 1, color);
    line(out, top2_drop.0, top2_drop.1, top2_drop.2, top2_drop.3, 1, color_drop);
    line(out, bottom1.0, bottom1.1, bottom1.2, bottom1.3, 1, color);
    line(out, bottom1_drop.0, bottom1_drop.1, bottom1_drop.2, bottom1_drop.3, 1, color_drop);
    line(out, bottom2.0, bottom2.1, bottom2.2, bottom2.3, 2, color);
    line(out, bottom2_drop.0, bottom2_drop.1, bottom2_drop.2, bottom2_drop.3, 2, color_drop);

    for v in verticals {
        let x = px + f(sx as f32, *v);
        line(out, x, py, x, height, 3, color);
    }

    advance_position(wm, h, b"MainMenuPulse", (px, py), (sx, sy), out);
}

/// `W3DMainMenuDraw(window, instData)` (W3DMainMenu.cpp:129): four verticals.
pub fn w3d_main_menu_draw<H: W3DMainMenuHost>(wm: &GameWindowManager, h: &mut H, window: WinId, out: &mut Vec<DrawItem>) {
    main_menu_frame(wm, h, window, &[0.225, 0.445, 0.6662, 0.885], out);
}

/// `W3DMainMenuFourDraw(window, instData)` (W3DMainMenu.cpp:193): three verticals (`.295`, `.59`, `.885`).
pub fn w3d_main_menu_four_draw<H: W3DMainMenuHost>(wm: &GameWindowManager, h: &mut H, window: WinId, out: &mut Vec<DrawItem>) {
    main_menu_frame(wm, h, window, &[0.295, 0.59, 0.885], out);
}

/// `W3DMetalBarMenuDraw(window, instData)`: `window->winDrawBorder()`.
pub fn w3d_metal_bar_menu_draw<H: GuiHost>(wm: &GameWindowManager, h: &H, window: WinId, out: &mut Vec<DrawItem>) {
    crate::client::window_draw::win_draw_border(wm, h, window, out);
}

/// `W3DClockDraw(window, instData)` (W3DMainMenu.cpp:325): the default draw, then the local time as "%H:%M:%S" in Arial 16.
pub fn w3d_clock_draw<H: W3DMainMenuHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, out: &mut Vec<DrawItem>) {
    crate::client::window_draw::w3d_game_win_default_draw(wm, window, out);
    let (px, py) = wm.tree.screen_position(window);
    let (sx, sy) = wm.tree.windows[window].size;

    let text: Vec<u16> = h.local_time_hh_mm_ss().encode_utf16().collect();
    // `instData->setText(temp)`
    wm.win_set_text(window, &text);

    // `dString->setFont(TheFontLibrary->getFont("Arial",16,0))`: the host's text measure answers for that font
    let font = None;
    let (text_width, text_height) = h.text_size(font, &text, None);

    // Init the clip region
    let clip = (px + 1, py + 1, px + sx - 1, py + sy - 1);

    let text_x = px + (sx / 2) - (text_width / 2);
    let text_y = py + (sy / 2) - (text_height / 2);
    out.push(DrawItem::Text { text, x: text_x, y: text_y, color: WHITE, drop_color: BLACK, font: None, word_wrap: None, wrap_centered: false, hotkey: false, clip: Some(clip) });
}

/// `W3DMainMenuMapBorder(window, instData)` (W3DMainMenu.cpp:372): the frame art around the map.
pub fn w3d_main_menu_map_border<H: GuiHost>(wm: &GameWindowManager, h: &H, window: WinId, out: &mut Vec<DrawItem>) {
    const BORDER_CORNER_SIZE: i32 = 10;
    const BORDER_LINE_SIZE: i32 = 20;

    // save original x, y
    let (original_x, original_y) = wm.tree.screen_position(window);
    let (width, height) = wm.tree.windows[window].size;
    let max_x = original_x + width;
    let max_y = original_y + height;
    let size = 20;
    let half_size = size / 2;
    let draw = |out: &mut Vec<DrawItem>, name: &[u8], x0: i32, y0: i32, x1: i32, y1: i32| {
        // `Display::drawImage` of a `NULL` image draws nothing
        if h.has_image(name) {
            out.push(DrawItem::Image { image: name.to_vec(), x0, y0, x1, y1, color: 0xFFFF_FFFF, grayscale: false, clip: wm.clip });
        }
    };

    // Draw Horizontal Lines: all border pieces are based on a 10 pixel offset from the centerline
    let mut y = original_y - 10;
    let y2 = max_y - 10;
    let mut x2 = max_x - (10 + BORDER_LINE_SIZE);
    let mut x;

    if h.has_image(b"FrameCornerHorizontal") {
        let img = b"FrameCornerHorizontal";
        x = original_x + 10;
        while x <= x2 {
            draw(out, img, x, y, x + size, y + size);
            draw(out, img, x, y2, x + size, y2 + size);
            x += BORDER_LINE_SIZE;
        }

        x2 = max_x - BORDER_CORNER_SIZE;

        // x == place to draw remainder if any
        if (x2 - x) >= (BORDER_LINE_SIZE / 2) {
            // Blit Half piece
            draw(out, img, x, y, x + half_size, y + size);
            draw(out, img, x, y2, x + half_size, y2 + size);
            x += BORDER_LINE_SIZE / 2;
        }

        // x2 - x must now be less than a half piece; border pieces have a two pixel repeat so we blit one pixel over
        // if necessary to line up the art, but we'll cover-up the overlap with the corners
        if x < x2 {
            x -= (BORDER_LINE_SIZE / 2) - (((x2 - x) + 1) & !1);
            draw(out, img, x, y, x + half_size, y + size);
            draw(out, img, x, y2, x + half_size, y2 + size);
        }
    }

    if h.has_image(b"FrameCornerVertical") {
        let img = b"FrameCornerVertical";
        // Draw Vertical Lines: all border pieces are based on a 10 pixel offset from the centerline
        x = original_x - 10;
        x2 = max_x - 10;
        let mut y2v = max_y - (10 + BORDER_LINE_SIZE);

        y = original_y + 10;
        while y <= y2v {
            draw(out, img, x, y, x + size, y + size);
            draw(out, img, x2, y, x2 + size, y + size);
            y += BORDER_LINE_SIZE;
        }

        y2v = max_y - BORDER_CORNER_SIZE;

        // y == place to draw remainder if any
        if (y2v - y) >= (BORDER_LINE_SIZE / 2) {
            draw(out, img, x, y, x + size, y + half_size);
            draw(out, img, x2, y, x2 + size, y + half_size);
            y += BORDER_LINE_SIZE / 2;
        }

        if y < y2v {
            y -= (BORDER_LINE_SIZE / 2) - (((y2v - y) + 1) & !1);
            draw(out, img, x, y, x + size, y + half_size);
            draw(out, img, x2, y, x2 + size, y + half_size);
        }
    }

    // Draw Corners
    draw(out, b"FrameCornerUL", original_x - BORDER_CORNER_SIZE, original_y - BORDER_CORNER_SIZE, original_x - BORDER_CORNER_SIZE + size, original_y - BORDER_CORNER_SIZE + size);
    draw(out, b"FrameCornerUR", max_x - BORDER_CORNER_SIZE, original_y - BORDER_CORNER_SIZE, max_x - BORDER_CORNER_SIZE + size, original_y - BORDER_CORNER_SIZE + size);
    draw(out, b"FrameCornerLL", original_x - BORDER_CORNER_SIZE, max_y - BORDER_CORNER_SIZE, original_x - BORDER_CORNER_SIZE + size, max_y - BORDER_CORNER_SIZE + size);
    draw(out, b"FrameCornerLR", max_x - BORDER_CORNER_SIZE, max_y - BORDER_CORNER_SIZE, max_x - BORDER_CORNER_SIZE + size, max_y - BORDER_CORNER_SIZE + size);

    // TheDisplay->enableClipping(FALSE): the clip is the caller's to clear
}

/// `drawText(window, instData)` (W3DMainMenu.cpp:754): the button text, wrapped at the width, centred in the window.
fn draw_text<H: GuiHost>(wm: &GameWindowManager, h: &H, window: WinId, out: &mut Vec<DrawItem>) {
    let win = &wm.tree.windows[window];
    let Some(text) = win.text.as_ref().filter(|t| !t.is_empty()) else { return };
    let (ox, oy) = wm.tree.screen_position(window);
    let (sx, sy) = win.size;
    let centered = win.inst.status & win_status::WRAP_CENTERED != 0;

    // get the right text color
    let (text_color, drop_color) = if win.status & win_status::ENABLED == 0 {
        (win.inst.disabled_text.color, win.inst.disabled_text.border_color)
    } else if win.inst.state & WIN_STATE_HILITED != 0 {
        (win.inst.hilite_text.color, win.inst.hilite_text.border_color)
    } else {
        (win.inst.enabled_text.color, win.inst.enabled_text.border_color)
    };

    let (width, height) = h.text_size(win.inst.font.as_ref(), text, Some(sx));
    let text_x = ox + (sx / 2) - (width / 2);
    let text_y = oy + (sy / 2) - (height / 2);
    out.push(DrawItem::Text { text: text.clone(), x: text_x, y: text_y, color: text_color, drop_color, font: win.inst.font.clone(), word_wrap: Some(sx), wrap_centered: centered, hotkey: false, clip: None });
}

/// `W3DMainMenuButtonDropShadowDraw(window, instData)` (W3DMainMenu.cpp:531): the push button with user supplied
/// images (left, middle, right by state).
pub fn w3d_main_menu_button_drop_shadow_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, out: &mut Vec<DrawItem>) {
    let (ox, oy) = wm.tree.screen_position(window);
    let (sx, sy) = wm.tree.windows[window].size;
    let (x_offset, y_offset) = wm.tree.windows[window].inst.image_offset;

    // get pointer to image we want to draw depending on our state, see GadgetPushButton.h for info:
    // draw data 0..6 are enabled, enabled-selected(1), (2 unused), middle selected 3, right selected 4, middle 5, right 6
    let (left, right, center) = {
        let win = &wm.tree.windows[window];
        let selected = win.inst.state & WIN_STATE_SELECTED != 0;
        let (l, r, m) = if selected { (1, 4, 3) } else { (0, 6, 5) };
        let data = if win.status & win_status::ENABLED == 0 {
            &win.inst.disabled_draw_data
        } else if win.inst.state & WIN_STATE_HILITED != 0 {
            &win.inst.hilite_draw_data
        } else {
            &win.inst.enabled_draw_data
        };
        (data[l].image.clone(), data[r].image.clone(), data[m].image.clone())
    };

    // sanity, we need to have these images to make it look right
    let (Some(left), Some(right), Some(center)) = (left, right, center) else { return };

    // get image sizes for the ends
    let (left_w, _left_h) = h.image_size(&left).unwrap_or((0, 0));
    let (right_w, _right_h) = h.image_size(&right).unwrap_or((0, 0));
    let (center_w, _) = h.image_size(&center).unwrap_or((0, 0));

    // get two key points used in the end drawing
    let left_end = (ox + left_w + x_offset, oy + sy + y_offset);
    let right_start = (ox + sx - right_w + x_offset, oy + y_offset);

    let clip = wm.clip;
    let mut img = |out: &mut Vec<DrawItem>, name: &Vec<u8>, x0: i32, y0: i32, x1: i32, y1: i32, clip: Option<(i32, i32, i32, i32)>| {
        out.push(DrawItem::Image { image: name.clone(), x0, y0, x1, y1, color: 0xFFFF_FFFF, grayscale: false, clip });
    };

    // get width we have to draw our repeating center in
    let mut center_width = right_start.0 - left_end.0;

    if center_width <= 0 {
        // draw left end
        let start = (ox + x_offset, oy + y_offset);
        let end = (ox + x_offset + sx / 2, left_end.1);
        img(out, &left, start.0, start.1, end.0, end.1, clip);

        // draw right end
        let start = (end.0, right_start.1);
        let end = (ox + sx, start.1 + sy);
        img(out, &right, start.0, start.1, end.0, end.1, clip);
    } else {
        // how many whole repeating pieces will fit in that width
        let pieces = if center_w != 0 { center_width / center_w } else { 0 };

        // draw the pieces
        let mut start = (left_end.0, oy + y_offset);
        let end_y = start.1 + sy + y_offset;
        for _ in 0..pieces {
            let end_x = start.0 + center_w;
            img(out, &center, start.0, start.1, end_x, end_y, clip);
            start.0 += center_w;
        }

        // we will draw the image but clip the parts we don't want to show; the region's y is the static
        // `clipRegion`'s, which upstream never assigns (0, 0)
        let reg = (start.0, 0, right_start.0, 0);
        center_width = right_start.0 - start.0;
        if center_width > 0 {
            let end_x = start.0 + center_w;
            img(out, &center, start.0, start.1, end_x, end_y, Some(reg));
        }

        // draw left end
        img(out, &left, ox + x_offset, oy + y_offset, left_end.0, left_end.1, clip);

        // draw right end
        img(out, &right, right_start.0, right_start.1, right_start.0 + right_w, right_start.1 + sy, clip);
    }

    // draw the button text
    if wm.tree.windows[window].text.as_ref().map_or(false, |t| !t.is_empty()) {
        draw_text(wm, h, window, out);
    }

    // (upstream: if the instance data has a video buffer, TheDisplay->drawVideoBuffer — device-only, not translated)
    if let UserData::PushButton(Some(p)) = &mut wm.ext[window].user {
        if let Some(o) = &p.overlay_image {
            out.push(DrawItem::Image { image: o.clone(), x0: ox, y0: oy, x1: ox + sx, y1: oy + sy, color: 0xFFFF_FFFF, grayscale: false, clip });
        }
        if p.draw_clock != crate::client::window_manager::ClockKind::None {
            out.push(DrawItem::RectClock { x: ox, y: oy, w: sx, h: sy, percent: p.percent_clock, color: p.color_clock, inverse: p.draw_clock == crate::client::window_manager::ClockKind::Inverse });
            p.draw_clock = crate::client::window_manager::ClockKind::None;
        }
        if p.draw_border && p.color_border != WIN_COLOR_UNDEFINED {
            out.push(DrawItem::OpenRect { x: ox - 1, y: oy - 1, w: sx + 2, h: sy + 2, color: p.color_border });
        }
    }
}

/// `W3DMainMenuRandomTextDraw(window, instData)` (W3DMainMenu.cpp:808): the text drawn with the disabled colours,
/// vertically centred at the window's left edge.
pub fn w3d_main_menu_random_text_draw<H: GuiHost>(wm: &GameWindowManager, h: &H, window: WinId, out: &mut Vec<DrawItem>) {
    let win = &wm.tree.windows[window];
    let UserData::StaticText(_, text) = &wm.ext[window].user else { return };
    let (ox, oy) = wm.tree.screen_position(window);
    let (sx, sy) = win.size;
    let text_color = win.inst.disabled_text.color;
    let text_outline_color = win.inst.disabled_text.border_color;

    // draw the text
    if text.is_empty() || text_color == WIN_COLOR_UNDEFINED {
        return;
    }

    // Init the clip region
    let clip = (ox + 1, oy + 1, ox + sx - 1, oy + sy - 1);

    // how much space will this text take up
    let (_text_width, text_height) = h.text_size(win.inst.font.as_ref(), text, None);

    let text_x = ox;
    let text_y = oy + (sy / 2) - (text_height / 2);
    out.push(DrawItem::Text { text: text.clone(), x: text_x, y: text_y, color: text_color, drop_color: text_outline_color, font: win.inst.font.clone(), word_wrap: None, wrap_centered: false, hotkey: false, clip: Some(clip) });
}

/// `W3DThinBorderDraw(window, instData)` (W3DMainMenu.cpp:842): the enabled image 0 at the window's size.
pub fn w3d_thin_border_draw(wm: &GameWindowManager, window: WinId, out: &mut Vec<DrawItem>) {
    let (sx_pos, sy_pos) = wm.tree.screen_position(window);
    let win = &wm.tree.windows[window];
    let (sx, sy) = win.size;
    if let Some(image) = win.inst.enabled_draw_data[0].image.clone() {
        let begin = (sx_pos + win.inst.image_offset.0, sy_pos + win.inst.image_offset.1);
        let end = (begin.0 + sx, begin.1 + sy);
        out.push(DrawItem::Image { image, x0: begin.0, y0: begin.1, x1: end.0, y1: end.1, color: 0xFFFF_FFFF, grayscale: false, clip: wm.clip });
    }
    // TheDisplay->enableClipping(FALSE): the clip is the caller's to clear
}

/// `W3DMainMenuInit(layout, userData)` (W3DMainMenu.cpp:870): the draw function assignments are commented out upstream;
/// only `MainMenuInit(layout, userData)` runs.
pub fn w3d_main_menu_init<H: W3DMainMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    h.main_menu_init(wm, layout);
}

/// `W3DCreditsMenuDraw(window, instData)`: `TheCredits->draw()`.
pub fn w3d_credits_menu_draw<H: W3DMainMenuHost>(h: &mut H, out: &mut Vec<DrawItem>) {
    h.credits_draw(out);
}
