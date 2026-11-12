// SPDX-License-Identifier: GPL-3.0-only
//! The game's cursor shown as the system's (GitHub #117): what the translated `TheMouse`
//! (`rts_lockstep_engine::client::mouse`) decides to show is shown with a system cursor icon chosen for it.
//!
//! # Why a system icon
//!
//! The game runs its mouse in `RM_WINDOWS` (see the translated module): the cursor is a Windows cursor loaded from
//! `data\cursors\*.ANI`, animated, with eight directions for the scroll cursor. Those files are EA's, a browser cannot
//! show a Windows `.ANI` cursor, and a phone has no cursor at all. So the client keeps everything that decides the
//! cursor (which one, which direction, visible or not: translated and differential-tested in the logic crate) and only
//! the last step, the picture, is its own: [`cursor_look`] maps each cursor to the nearest system icon. On a touch
//! screen the icon and the hiding have no effect, and nothing else depends on them.
//!
//! The mapping is new (the client's choice, not upstream's): the arrow and the normal cursor are the default arrow;
//! the scroll cursor is the resize arrow for its direction frame (frame 0 points right, then clockwise, as
//! `W3DMouse::setCursorDirection` assumes), or the move cross when the scroll cursor has one direction; the attack
//! cursors a crosshair; the invalid ones "not allowed"; the placing ones a cell; selecting, moving and the other
//! commands on an object a pointer; entering, capturing and hacking a grab.
//!
//! Until `Mouse.ini` is read ([`load_mouse`]) the client leaves the window's cursor alone (the demo has no game files).

use rts_lockstep_engine::client::mouse::{cursor, Mouse};

/// A system cursor icon (the names of Bevy's `SystemCursorIcon`, which the application maps one to one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Default,
    Pointer,
    Crosshair,
    Move,
    NotAllowed,
    Cell,
    Grab,
    Help,
    EResize,
    SeResize,
    SResize,
    SwResize,
    WResize,
    NwResize,
    NResize,
    NeResize,
}

/// What the window shows: no cursor, or an icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorLook {
    Hidden,
    Icon(Icon),
}

/// The look of what `mouse` shows ([`Mouse::shown`]).
pub fn cursor_look(mouse: &Mouse) -> CursorLook {
    let Some((c, direction)) = mouse.shown() else { return CursorLook::Hidden };
    CursorLook::Icon(match c {
        cursor::NORMAL | cursor::ARROW => Icon::Default,
        cursor::SCROLL => {
            let n = mouse.cursor_info[cursor::SCROLL as usize].num_directions;
            if n > 1 {
                // the frame's angle, clockwise from the right, to the nearest of eight
                const EIGHT: [Icon; 8] = [Icon::EResize, Icon::SeResize, Icon::SResize, Icon::SwResize, Icon::WResize, Icon::NwResize, Icon::NResize, Icon::NeResize];
                EIGHT[((direction * 8 + n / 2) / n).rem_euclid(8) as usize]
            } else {
                Icon::Move
            }
        }
        cursor::CROSS
        | cursor::ATTACK_OBJECT
        | cursor::FORCE_ATTACK_OBJECT
        | cursor::FORCE_ATTACK_GROUND
        | cursor::ATTACKMOVETO
        | cursor::OUTRANGE
        | cursor::SNIPE_VEHICLE
        | cursor::LASER_GUIDED_MISSILES
        | cursor::TANKHUNTER_TNT_ATTACK
        | cursor::STAB_ATTACK
        | cursor::FIRE_FLAME
        | cursor::FIRE_BOMB
        | cursor::PARTICLE_UPLINK_CANNON => Icon::Crosshair,
        cursor::INVALID_BUILD_PLACEMENT | cursor::GENERIC_INVALID | cursor::STAB_ATTACK_INVALID | cursor::PLACE_CHARGE_INVALID => Icon::NotAllowed,
        cursor::BUILD_PLACEMENT | cursor::PLACE_REMOTE_CHARGE | cursor::PLACE_TIMED_CHARGE | cursor::PLACE_BEACON => Icon::Cell,
        cursor::ENTER_FRIENDLY | cursor::ENTER_AGGRESSIVELY | cursor::CAPTUREBUILDING | cursor::HACK | cursor::DEFECTOR | cursor::DISGUISE_AS_VEHICLE => Icon::Grab,
        cursor::GET_REPAIRED | cursor::GET_HEALED | cursor::DO_REPAIR | cursor::RESUME_CONSTRUCTION => Icon::Help,
        _ => Icon::Pointer,
    })
}

/// (#122) The InGameUI's queued calls on `TheMouse` (`rts_lockstep_engine::client::in_game_ui_cursor::UiMouseCall`) made
/// on `mouse` in order: its cursors; (#124) the tooltip (`setCursorTooltip(text, -1, &rgb)` with the colour as
/// `RGBColor::setFromInt` makes it, or `setCursorTooltip(TheEmptyString)`) and `resetTooltipDelay` at `now` (ms); the
/// radius cursor's `setRadiusCursor(type, power, slot)` handed to `radius`. The capture has nothing to act on in the
/// client (the browser and the window system keep the pointer). On a phone the cursor shows nothing; the calls change
/// only the translated mouse's state. GUI glue's helper, tested below.
pub fn apply_ui_mouse_calls(mouse: &mut Mouse, calls: Vec<rts_lockstep_engine::client::in_game_ui_cursor::UiMouseCall>, now: u32, mut radius: impl FnMut(i32, Option<u32>, i32)) {
    use rts_lockstep_engine::client::in_game_ui_cursor::UiMouseCall;
    use rts_lockstep_engine::common::base_type::RGBColor;
    for c in calls {
        match c {
            UiMouseCall::SetCursor(c) => mouse.set_cursor(c),
            UiMouseCall::Tooltip { text, color } => {
                let rgb = color.map(|c| {
                    let mut r = RGBColor::default();
                    r.set_from_int(c);
                    r
                });
                mouse.tooltip.set_cursor_tooltip_with(&text, -1, rgb.as_ref(), 1.0);
            }
            UiMouseCall::ResetTooltipDelay => mouse.tooltip.reset_tooltip_delay(now),
            UiMouseCall::RadiusCursor { kind, special_power, weapon_slot } => radius(kind, special_power, weapon_slot),
            UiMouseCall::Capture | UiMouseCall::ReleaseCapture => {}
        }
    }
}

/// (#124) `DisplayString::getSize` of the tooltip ~~as the client draws it (new: the font's own measure is Bevy's, which
/// is not known before it lays the text out): `font_px` high a line, half that wide a character, each of the text's
/// lines wrapped at `wrap` pixels when given~~. **Corrected 2026-11-12, #155:** as upstream measures it
/// (`m_tooltipDisplayString->getSize`, Mouse.cpp's `drawTooltip`): the tooltip's font (`font`, `None` for none: Arial 12)
/// through #153's translated measure (`crate::text_metrics::text_size`: render2dsentence.cpp's pass over Arial's widths),
/// word wrapped at `wrap` pixels (0: no wrap, `setWordWrap(0)`).
pub fn tooltip_text_size(font: Option<&rts_lockstep_engine::client::window_script::GameFont>, text: &[u16], wrap: i32) -> (i32, i32) {
    crate::text_metrics::text_size(font, text, Some(wrap))
}

/// (#155) The frame the mouse's tooltip is laid out in: the window (`screen`, logical pixels), or, while the shell's layout
/// is drawn turned a quarter clockwise on a portrait screen (#153's `LayoutPlacement::turned`), the window turned with it:
/// `screen.1` wide and `screen.0` high, a screen point `(sx, sy)` at `(sy, screen.0 - sx)`. The mouse's limits
/// (`setMouseLimits`, the tooltip's display width) and its position are given in this frame, so the translated
/// `drawTooltip` keeps the box beside the pointer and inside the frame's edges as upstream does on an upright display;
/// the app then draws the frame turned back ([`TooltipFrame::to_screen`]). New presentation code: upstream's display is
/// never turned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TooltipFrame {
    pub screen: (f32, f32),
    pub turned: bool,
}

impl TooltipFrame {
    /// The frame's size, whole pixels (the mouse's limits).
    pub fn size(&self) -> (i32, i32) {
        let (w, h) = (self.screen.0 as i32, self.screen.1 as i32);
        if self.turned {
            (h, w)
        } else {
            (w, h)
        }
    }

    /// A screen point in the frame.
    pub fn to_frame(&self, p: (f32, f32)) -> (f32, f32) {
        if self.turned {
            (p.1, self.screen.0 - p.0)
        } else {
            p
        }
    }

    /// A frame point on screen ([`TooltipFrame::to_frame`] inverted).
    pub fn to_screen(&self, p: (f32, f32)) -> (f32, f32) {
        if self.turned {
            (self.screen.0 - p.1, p.0)
        } else {
            p
        }
    }
}

/// (#124) One shape of the world's tooltip on screen, in window pixels.
#[derive(Clone, Debug, PartialEq)]
pub enum TooltipShape {
    /// A filled rectangle `(x0, y0, x1, y1)` (`drawFillRect`, and each edge of `drawOpenRect`), RGBA.
    Rect { rect: [f32; 4], color: [u8; 4] },
    /// `DisplayString::draw`: the text laid out in `rect` (its full size), shown only inside `clip`. **(#155)** `text`
    /// is the measured lines joined by newlines (`crate::text_metrics::text_lines`), drawn without wrapping at the font's
    /// em `font_px` (`MulDiv(points, 96, 72)`), `line_px` apart (its height), in Liberation Sans `face`.
    Text { text: String, rect: [f32; 4], clip: [f32; 4], color: [u8; 4], font_px: f32, line_px: f32, face: crate::fonts::Face },
}

/// `Color` (`0xAARRGGBB`) as RGBA.
fn rgba(c: i32) -> [u8; 4] {
    let c = c as u32;
    [(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8]
}

/// (#124) `W3DMouse::draw`'s tooltip (`Mouse::drawTooltip`, translated in the logic crate) at `now` (ms) as shapes: the
/// box, its one-pixel border, the text revealed up to the highlight and the highlighted strip. The drop shadow is not
/// drawn (as the control bar's text, `crate::ui`). Nothing while a script fade is on (`fade`) or the mouse is hidden.
/// **(#155)** Measured as upstream measures it ([`tooltip_text_size`]) and drawn in the tooltip font's em, line height
/// and Liberation Sans face (~~the point size, Bevy's font~~); the shapes are in the mouse's frame ([`TooltipFrame`]:
/// the window, or the turned shell's).
pub fn tooltip_shapes(mouse: &mut Mouse, now: u32, fade: bool) -> Vec<TooltipShape> {
    use rts_lockstep_engine::client::window_manager::DrawItem;
    let font = mouse.tooltip.font.clone();
    let items = mouse.draw_tooltip(now, fade, |t, wrap| tooltip_text_size(font.as_ref(), t, wrap));
    let mut out = Vec::new();
    for d in items {
        match d {
            DrawItem::FillRect { x, y, w, h, color } => out.push(TooltipShape::Rect { rect: [x as f32, y as f32, (x + w) as f32, (y + h) as f32], color: rgba(color) }),
            DrawItem::OpenRect { x, y, w, h, color } => {
                let (x0, y0, x1, y1) = (x as f32, y as f32, (x + w) as f32, (y + h) as f32);
                for rect in [[x0, y0, x1, y0 + 1.0], [x0, y1 - 1.0, x1, y1], [x0, y0, x0 + 1.0, y1], [x1 - 1.0, y0, x1, y1]] {
                    out.push(TooltipShape::Rect { rect, color: rgba(color) });
                }
            }
            DrawItem::Text { text, x, y, color, word_wrap, clip, .. } => {
                let (w, h) = tooltip_text_size(font.as_ref(), &text, word_wrap.unwrap_or(0));
                let rect = [x as f32, y as f32, (x + w) as f32, (y + h) as f32];
                // the clip region where it meets the text (a strip left of the text shows nothing)
                let clip = clip.map_or(rect, |(a, b, c, d)| [(a as f32).max(rect[0]), (b as f32).max(rect[1]), (c as f32).min(rect[2]), (d as f32).min(rect[3])]);
                if clip[2] > clip[0] && clip[3] > clip[1] {
                    let f = font.as_ref();
                    out.push(TooltipShape::Text {
                        text: crate::text_metrics::text_lines_string(f, &text, word_wrap),
                        rect,
                        clip,
                        color: rgba(color),
                        font_px: crate::text_metrics::em_px(f),
                        line_px: crate::text_metrics::line_px(f),
                        face: crate::text_metrics::draw_face(f),
                    });
                }
            }
            _ => {}
        }
    }
    out
}

/// (#124) `InGameUI::init`'s `INI::load("Data\\INI\\InGameUI.ini")` into the radius cursor's templates (from `open`).
/// `None` while the file is still being fetched; `Some(Err)` if it is missing or does not read (the templates are left
/// as they were: no texture, so no radius cursor shows, as upstream without the file).
pub fn load_in_game_ui(radius: &mut rts_lockstep_engine::client::radius_cursor::InGameUiRadiusCursor, open: impl Fn(&[u8]) -> Option<Vec<u8>>, waiting: impl Fn() -> bool) -> Option<Result<(), String>> {
    let path = b"Data\\INI\\InGameUI.ini";
    let text = open(path);
    if waiting() {
        return None;
    }
    let Some(text) = text else { return Some(Err("no Data\\INI\\InGameUI.ini".into())) };
    let mut r = rts_lockstep_engine::client::radius_cursor::InGameUiRadiusCursor::default();
    if let Err(e) = r.parse_ini(path, &text) {
        return Some(Err(format!("InGameUI.ini did not read: {e:?}")));
    }
    *radius = r;
    Some(Ok(()))
}

/// (#124) The radius cursor's projected decal as the client draws it on the ground: the circle's points (game
/// coordinates, `segments` of them, each lifted half a unit above the ground's `height(x, y)` so it lies on the terrain
/// rather than cutting through it) and its RGBA colour (the decal's colour, its opacity as the alpha; an opacity never
/// set is opaque). New drawing (upstream projects the decal's texture onto the terrain; the texture is EA's art and the
/// client has no projected-texture pass): the circle's edge.
pub fn decal_ring(decal: &rts_lockstep_engine::client::radius_cursor::DecalShadow, segments: usize, height: impl Fn(f32, f32) -> f32) -> (Vec<[f32; 3]>, [u8; 4]) {
    let r = decal.size.0 / 2.0;
    let c = decal.position;
    let n = segments.max(3);
    let pts = (0..=n)
        .map(|i| {
            let a = i as f32 / n as f32 * 2.0 * core::f32::consts::PI;
            let (x, y) = (c.x + r * rts_lockstep_engine::ww::wwmath::cos(a), c.y + r * rts_lockstep_engine::ww::wwmath::sin(a));
            [x, y, height(x, y) + 0.5]
        })
        .collect();
    let [cr, cg, cb, _] = rgba(decal.color);
    let alpha = decal.opacity.map_or(255, |o| o.clamp(0, 255) as u8);
    (pts, [cr, cg, cb, alpha])
}

/// (#148) `GlobalLanguage::init`: `INI::load("Data\\<language>\\Language.ini", INI_LOAD_OVERWRITE)` (from `open`) over the
/// constructor's members. `None` while the file is still being fetched; `Some(Err)` if it is missing or does not read
/// (the language is left as it was: the constructor's values, as upstream without the file).
pub fn load_language(language: &mut rts_lockstep_engine::client::global_language::GlobalLanguage, name: &[u8], open: impl Fn(&[u8]) -> Option<Vec<u8>>, waiting: impl Fn() -> bool) -> Option<Result<(), String>> {
    use rts_lockstep_engine::client::global_language::GlobalLanguage;
    let path = GlobalLanguage::file_name(name);
    let text = open(&path);
    if waiting() {
        return None;
    }
    let Some(text) = text else { return Some(Err(format!("no {}", String::from_utf8_lossy(&path)))) };
    let mut g = language.clone();
    if let Err(e) = g.parse_ini(&path, &text) {
        return Some(Err(format!("Language.ini did not read: {e:?}")));
    }
    *language = g;
    Some(Ok(()))
}

/// `GameClient::init`'s mouse: `parseIni` of `Data\INI\Mouse.ini` (from `open`), `initCursorResources` (each named
/// texture's resource counts as loaded: the system icon stands in for the `.ANI` file, which is not read), then
/// `init`. `None` while the file is still being fetched; `Some(Err)` if it is missing or does not read (the mouse is
/// left as it was).
pub fn load_mouse(mouse: &mut Mouse, open: impl Fn(&[u8]) -> Option<Vec<u8>>, waiting: impl Fn() -> bool) -> Option<Result<(), String>> {
    let path = b"Data\\INI\\Mouse.ini";
    let text = open(path);
    if waiting() {
        return None;
    }
    let Some(text) = text else { return Some(Err("no Data\\INI\\Mouse.ini".into())) };
    let mut m = Mouse::new();
    if let Err(e) = m.parse_ini(path, &text) {
        return Some(Err(format!("Mouse.ini did not read: {e:?}")));
    }
    m.init_cursor_resources(|_| true);
    // (#148) `TheGlobalLanguageData`, made before the mouse: `init`'s tooltip font reads it
    m.language = mouse.language.clone();
    m.init();
    // what the mouse had been asked before the file came (visibility, the cursor, the focus) is kept
    m.visible = mouse.visible;
    m.lost_focus = mouse.lost_focus;
    m.current_cursor = mouse.current_cursor;
    m.ui_scrolling = mouse.ui_scrolling;
    m.ui_scroll_amount = mouse.ui_scroll_amount;
    *mouse = m;
    Some(Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_lockstep_engine::common::base_type::Coord2D;

    /// #148, predicted before the run: without the file the language is the constructor's and the missing file is reported;
    /// while fetched, `None`; a `Language.ini` with `ResolutionFontAdjustment = 0.25` reads it; one with an unknown field
    /// is reported and leaves the language alone.
    #[test]
    fn language_ini_is_read_from_the_files() {
        use rts_lockstep_engine::client::global_language::GlobalLanguage;
        let mut g = GlobalLanguage::default();
        assert_eq!(load_language(&mut g, b"English", |_| None, || false), Some(Err("no Data\\English\\Language.ini".into())));
        assert_eq!(load_language(&mut g, b"English", |_| None, || true), None);
        assert_eq!(g, GlobalLanguage::default());
        let bad = b"Language\n  Nope = 1\nEnd\n".to_vec();
        assert!(matches!(load_language(&mut g, b"English", |_| Some(bad.clone()), || false), Some(Err(_))));
        assert_eq!(g, GlobalLanguage::default());
        let ini = b"Language\n  ResolutionFontAdjustment = 0.25\nEnd\n".to_vec();
        assert_eq!(load_language(&mut g, b"English", |p| (p == b"Data\\English\\Language.ini").then(|| ini.clone()), || false), Some(Ok(())));
        assert_eq!(g.resolution_font_size_adjustment, 0.25);
        // 1600 wide: 20 * (1 + 1 * 0.25) = 25
        assert_eq!(g.adjust_font_size(20, 1600), 25);
    }

    /// #117, predicted before the run: with a `Mouse.ini` naming the arrow, the attack cursor and an eight-way scroll
    /// cursor, the mouse after `load_mouse` and `setVisibility(TRUE)` shows the default arrow; hidden it shows nothing;
    /// an attack cursor drawn is a crosshair; scrolling down and to the left (`(-1, 1)` on the screen) the scroll cursor
    /// drawn twice is frame 3 of 8 (`atan2(1, -1) = 3π/4`, `3π/4 / (π/4) + 0.5 = 3.5`), the south-west resize arrow;
    /// without the file, `load_mouse` reports it and leaves the mouse alone.
    ///
    /// Results (2026-11-11): all held.
    #[test]
    fn the_translated_mouse_is_shown_with_system_icons() {
        let ini = b"MouseCursor Arrow\n  Texture = SCCPointer\nEnd\nMouseCursor AttackObj\n  Texture = SCCAttack\nEnd\nMouseCursor Scroll\n  Texture = SCCScroll\n  Directions = 8\nEnd\n".to_vec();
        let mut m = Mouse::new();
        assert_eq!(load_mouse(&mut m, |_| None, || false), Some(Err("no Data\\INI\\Mouse.ini".into())));
        assert_eq!(load_mouse(&mut m, |_| None, || true), None);
        assert_eq!(load_mouse(&mut m, |_| Some(ini.clone()), || false), Some(Ok(())));
        m.set_visibility(true);
        m.draw();
        assert_eq!(cursor_look(&m), CursorLook::Icon(Icon::Default));
        m.set_visibility(false);
        assert_eq!(cursor_look(&m), CursorLook::Hidden);
        m.set_visibility(true);
        m.set_cursor(cursor::ATTACK_OBJECT);
        m.draw();
        assert_eq!(cursor_look(&m), CursorLook::Icon(Icon::Crosshair));
        m.ui_scrolling = true;
        m.ui_scroll_amount = Coord2D { x: -1.0, y: 1.0 };
        m.set_cursor(cursor::SCROLL);
        m.draw();
        m.draw();
        assert_eq!(m.direction_frame, 3);
        assert_eq!(cursor_look(&m), CursorLook::Icon(Icon::SwResize));
    }

    /// #124, predicted before the run: a moused-over object's tooltip (`UiMouseCall::Tooltip`, colour `0xFF0000`, red)
    /// set at 1000 ms, with the mouse still from then on at (100, 100) in an 800x600 window and `Mouse.ini`'s defaults
    /// (delay 50 ms, fill 50 ms, 15 % width: 120 pixels of wrap at 800, the 12 pixel font): at 1049 ms nothing shows; at
    /// 1050 ms the timer shows it and restarts the highlight, so the first draw (1050) is a box 2 pixels wide (width
    /// `min(24, 0) + 2`) at (120, 100) with no text revealed; at 1075 ms the highlight is at `24 · 25 / 50 = 12`, so the
    /// text "Tank" (4 characters, 24 pixels) shows cut at x 134 with the strip from 119 to 134 (its part over the text, 122
    /// to 134, is drawn); at the right edge (x 790)
    /// the box moves left of the mouse (`790 + 20 − 20 − 24 = 766`); `resetTooltipDelay` hides it; a radius cursor call
    /// reaches the closure; the empty tooltip hides it.
    ///
    /// Results (2026-11-11): all held but the wrap. **Miss:** the word wrap after the tooltip is set is the display's 800,
    /// not 120: without `Mouse.ini`'s `TooltipWidth` the constructor's `m_tooltipWidth = 15.0f` (1,500 %, not 15 %)
    /// applies, so `setCursorTooltip`'s width is 12,000 pixels, capped at the display's. With `TooltipWidth = 15%` it is
    /// 120 (checked below). **Second miss:** the box was predicted in `TooltipBackgroundColor` (20, 20, 0, 127) and the
    /// text in `TooltipTextColor` (220, 220, 220); they are the constructor's black (0, 0, 0, 255) and white. Given a colour
    /// (the object's), `setCursorTooltip` changes the text and back colours only with `UseTooltipAltTextColor` /
    /// `UseTooltipAltBackColor`; only a call without a colour copies the INI's colours. This test makes no colourless call first;
    /// in a game `createMouseoverHint` clears the tooltip with one before every drawable's (`UiMouseCall::Tooltip` with
    /// no colour, queued first), so the INI's colours apply there. Upstream's behaviour, kept. **Third miss (the test's
    /// own drawing):** at 1050 the highlighted strip's clip (107 to 122) lies wholly left of the text; the shapes now cut
    /// each clip to its text, so nothing is drawn for it. **Fourth miss:** the highlight a draw shows is the one the draw
    /// before it computed (`drawTooltip` moves it on after drawing), so the 12-pixel reveal of 1075 shows at the next draw
    /// (1076), not at 1075's.
    ///
    /// **#155, predicted before the run (the measure is now upstream's, `tooltip_text_size`):** the default tooltip font,
    /// Times New Roman 12, measures as Arial 12: 18 high, "Tank" 10 + 9 + 9 + 8 + 2 = 38 wide. So the box is 20 high
    /// (rect to y 120); at 1076 the reveal is 38 x 25 / 50 = 19: the text's clip 122 to 141, the strip 126 to 141 (y 101 to
    /// 119); the text drawn is "Tank" in its full box (122, 101) to (160, 119) at the em 16, 18 apart, regular; at the right
    /// edge the box is at 810 - 20 - 38 = 752.
    #[test]
    fn the_world_tooltip_is_drawn_as_the_mouse_decides() {
        use rts_lockstep_engine::client::in_game_ui_cursor::UiMouseCall;
        let mut m = Mouse::new();
        m.init();
        m.set_visibility(true);
        m.set_mouse_limits(800, 600);
        m.pos = rts_lockstep_engine::common::base_type::ICoord2D { x: 100, y: 100 };
        let mut radius = Vec::new();
        let calls = vec![UiMouseCall::Tooltip { text: "Tank".encode_utf16().collect(), color: Some(0xFF0000) }, UiMouseCall::RadiusCursor { kind: 4, special_power: None, weapon_slot: 0 }];
        apply_ui_mouse_calls(&mut m, calls, 1000, |k, sp, w| radius.push((k, sp, w)));
        assert_eq!(radius, vec![(4, None, 0)]);
        assert_eq!(m.tooltip.word_wrap, 800);
        let mut m15 = m.clone();
        m15.tooltip.settings.tooltip_width = 0.15;
        m15.tooltip.last_tooltip_width = 0.0;
        apply_ui_mouse_calls(&mut m15, vec![UiMouseCall::Tooltip { text: "Tank".encode_utf16().collect(), color: None }], 1000, |_, _, _| {});
        assert_eq!(m15.tooltip.word_wrap, 120);
        m.tooltip.reset_tooltip_delay(1000);
        m.tooltip.create_stream_messages(1049, false);
        assert!(tooltip_shapes(&mut m, 1049, false).is_empty());
        m.tooltip.create_stream_messages(1050, false);
        let s = tooltip_shapes(&mut m, 1050, false);
        // (#155: 18 high, the measured Times New Roman 12 as Arial: the box 20 high; was 14)
        assert_eq!(s[0], TooltipShape::Rect { rect: [120.0, 100.0, 122.0, 120.0], color: [0, 0, 0, 255] });
        assert!(!s.iter().any(|x| matches!(x, TooltipShape::Text { .. })), "nothing revealed yet: {s:?}");
        tooltip_shapes(&mut m, 1075, false);
        let s = tooltip_shapes(&mut m, 1076, false);
        let texts: Vec<_> = s.iter().filter_map(|x| if let TooltipShape::Text { clip, color, .. } = x { Some((*clip, *color)) } else { None }).collect();
        // (#155: "Tank" 38 wide, so the highlight at 1075 is 38 x 25 / 50 = 19: the text cut at 141, the strip 126 to 141)
        assert_eq!(texts, vec![([122.0, 101.0, 141.0, 119.0], [255, 255, 255, 255]), ([126.0, 101.0, 141.0, 119.0], [255, 255, 0, 255])]);
        let drawn: Vec<_> = s.iter().filter_map(|x| if let TooltipShape::Text { text, rect, font_px, line_px, face, .. } = x { Some((text.clone(), *rect, *font_px, *line_px, *face)) } else { None }).collect();
        assert_eq!(drawn[0], ("Tank".to_string(), [122.0, 101.0, 160.0, 119.0], 16.0, 18.0, crate::fonts::Face::Regular));
        assert!(tooltip_shapes(&mut m, 1080, true).is_empty(), "a script fade hides it");
        m.pos.x = 790;
        let s = tooltip_shapes(&mut m, 1100, false);
        // (#155: 810 - 20 - 38 = 752; was 766 with "Tank" 24 wide)
        assert!(matches!(s[0], TooltipShape::Rect { rect, .. } if rect[0] == 752.0), "{s:?}");
        m.tooltip.reset_tooltip_delay(1100);
        assert!(tooltip_shapes(&mut m, 1101, false).is_empty());
        apply_ui_mouse_calls(&mut m, vec![UiMouseCall::Tooltip { text: Vec::new(), color: None }], 1200, |_, _, _| {});
        m.tooltip.create_stream_messages(2000, false);
        assert!(tooltip_shapes(&mut m, 2000, false).is_empty());
    }

    /// #124, predicted before the run: two lines wrap separately (`"ab\ncdefgh"` at 12 px: 6 px a character, 36 pixels
    /// for the longer line, 24 high; wrapped at 24 pixels the longer takes two rows: 24 x 36). ~~(that measure)~~
    /// **Corrected 2026-11-12, #155:** upstream's measure (Arial 12, no font given): "ab" 9 + 9, "cdefgh" 8 + 9 + 9 + 4 +
    /// 9 + 9 = 48, plus the overlap 2: 50 x 36, and the same wrapped at 24 (upstream wraps only at a space); "ab cdefgh" 18
    /// + 4 + 48 + 2 = 72 x 18, and at 24 two lines, the second the space and "cdefgh" (52), 54 x 36,
    /// drawn as "ab" and " cdefgh". A decal of size 20 at
    /// (10, 20) with colour `0x00336699` and opacity 128 rings 10 units around it, half a unit over the ground, in
    /// (0x33, 0x66, 0x99, 128); an `InGameUI.ini` giving the guard area's texture loads it, and a missing file is reported.
    ///
    /// Results (2026-11-11): all held.
    #[test]
    fn the_tooltip_measure_the_decal_ring_and_in_game_ui_ini() {
        use rts_lockstep_engine::client::radius_cursor::{radius_cursor_type, DecalShadow, InGameUiRadiusCursor};
        let t: Vec<u16> = "ab\ncdefgh".encode_utf16().collect();
        // (#155: upstream's measure, no font: Arial 12)
        assert_eq!(tooltip_text_size(None, &t, 0), (50, 36));
        assert_eq!(tooltip_text_size(None, &t, 24), (50, 36));
        let t2: Vec<u16> = "ab cdefgh".encode_utf16().collect();
        assert_eq!(tooltip_text_size(None, &t2, 0), (72, 18));
        assert_eq!(tooltip_text_size(None, &t2, 24), (54, 36));
        assert_eq!(crate::text_metrics::text_lines_string(None, &t2, Some(24)), "ab\n cdefgh");
        assert_eq!(crate::text_metrics::text_lines_string(None, &t, Some(24)), "ab\ncdefgh");
        let d = DecalShadow { texture: b"T".to_vec(), shadow_type: 0x20, size: (20.0, 20.0), angle: 0.0, color: 0x0033_6699, position: rts_lockstep_engine::common::base_type::Coord3D { x: 10.0, y: 20.0, z: 0.0 }, opacity: Some(128) };
        let (pts, c) = decal_ring(&d, 8, |_, _| 4.0);
        assert_eq!(c, [0x33, 0x66, 0x99, 128]);
        assert_eq!(pts.len(), 9);
        for p in &pts {
            let r = rts_lockstep_engine::ww::wwmath::sqrt((p[0] - 10.0) * (p[0] - 10.0) + (p[1] - 20.0) * (p[1] - 20.0));
            assert!((r - 10.0).abs() < 1e-3 && p[2] == 4.5, "{p:?}");
        }
        let ini = b"InGameUI\n  MaxSelectionSize = 40\nEnd\n".to_vec();
        let mut r = InGameUiRadiusCursor::default();
        assert!(matches!(load_in_game_ui(&mut r, |_| Some(ini.clone()), || false), Some(Err(_))), "an unknown field fails the load, as upstream's");
        let ini = b"InGameUI\n  GuardAreaRadiusCursor\n    Texture = SCCGuard\n    OpacityMin = 25%\n  End\n  DrawRMBScrollAnchor = Yes\nEnd\n".to_vec();
        assert_eq!(load_in_game_ui(&mut r, |_| Some(ini.clone()), || false), Some(Ok(())));
        assert_eq!(r.radius_cursors[radius_cursor_type::GUARD_AREA as usize].name, b"SCCGuard");
        assert_eq!(r.radius_cursors[radius_cursor_type::GUARD_AREA as usize].min_opacity, 0.25);
        assert!(r.settings.draw_rmb_scroll_anchor);
        assert_eq!(load_in_game_ui(&mut r, |_| None, || false), Some(Err("no Data\\INI\\InGameUI.ini".into())));
        assert_eq!(load_in_game_ui(&mut r, |_| None, || true), None);
    }


    /// #155, predicted before the run: on a 390x844 screen with the shell turned, the tooltip's frame is 844 wide and 390
    /// high; the finger at (300, 100) is the frame's (100, 90), and back. The mouse given that frame shows "Tank" (38 by 18)
    /// at frame (120, 90): beside the pointer in the frame, which on screen is (390 - 90, 120) = (300, 120), 20 pixels below
    /// the finger, its box running down the screen. Upright (844x390, not turned) the frame is the screen.
    ///
    /// Results (2026-11-12): all held.
    #[test]
    fn the_tooltip_turns_with_the_shell() {
        let f = TooltipFrame { screen: (390.0, 844.0), turned: true };
        assert_eq!(f.size(), (844, 390));
        assert_eq!(f.to_frame((300.0, 100.0)), (100.0, 90.0));
        assert_eq!(f.to_screen((100.0, 90.0)), (300.0, 100.0));
        let mut m = Mouse::new();
        m.init();
        m.set_visibility(true);
        let (w, h) = f.size();
        m.set_mouse_limits(w, h);
        let p = f.to_frame((300.0, 100.0));
        m.pos = rts_lockstep_engine::common::base_type::ICoord2D { x: p.0 as i32, y: p.1 as i32 };
        apply_ui_mouse_calls(&mut m, vec![rts_lockstep_engine::client::in_game_ui_cursor::UiMouseCall::Tooltip { text: "Tank".encode_utf16().collect(), color: None }], 1000, |_, _, _| {});
        m.tooltip.reset_tooltip_delay(1000);
        m.tooltip.create_stream_messages(2000, false);
        let s = tooltip_shapes(&mut m, 2000, false);
        let TooltipShape::Rect { rect, .. } = &s[0] else { panic!("{s:?}") };
        assert_eq!((rect[0], rect[1]), (120.0, 90.0));
        assert_eq!(f.to_screen((rect[0], rect[1])), (300.0, 120.0));
        let up = TooltipFrame { screen: (844.0, 390.0), turned: false };
        assert_eq!((up.size(), up.to_frame((300.0, 100.0)), up.to_screen((300.0, 100.0))), ((844, 390), (300.0, 100.0), (300.0, 100.0)));
    }
}
