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

/// (#124) `DisplayString::getSize` of the tooltip as the client draws it (new: the font's own measure is Bevy's, which
/// is not known before it lays the text out): `font_px` high a line, half that wide a character, each of the text's
/// lines wrapped at `wrap` pixels when given.
pub fn tooltip_text_size(font_px: i32, text: &[u16], wrap: i32) -> (i32, i32) {
    let h = font_px.max(1);
    let cw = (h / 2).max(1);
    let (mut w, mut rows) = (0, 0);
    for line in text.split(|&c| c == u16::from(b'\n')) {
        let lw = line.len() as i32 * cw;
        if wrap > 0 && lw > wrap {
            rows += (lw + wrap - 1) / wrap;
            w = w.max(wrap);
        } else {
            rows += 1;
            w = w.max(lw);
        }
    }
    (w, h * rows)
}

/// (#124) One shape of the world's tooltip on screen, in window pixels.
#[derive(Clone, Debug, PartialEq)]
pub enum TooltipShape {
    /// A filled rectangle `(x0, y0, x1, y1)` (`drawFillRect`, and each edge of `drawOpenRect`), RGBA.
    Rect { rect: [f32; 4], color: [u8; 4] },
    /// `DisplayString::draw`: the text laid out in `rect` (its full size), shown only inside `clip`.
    Text { text: String, rect: [f32; 4], clip: [f32; 4], color: [u8; 4], font_px: f32 },
}

/// `Color` (`0xAARRGGBB`) as RGBA.
fn rgba(c: i32) -> [u8; 4] {
    let c = c as u32;
    [(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8]
}

/// (#124) `W3DMouse::draw`'s tooltip (`Mouse::drawTooltip`, translated in the logic crate) at `now` (ms) as shapes: the
/// box, its one-pixel border, the text revealed up to the highlight and the highlighted strip. The drop shadow is not
/// drawn (as the control bar's text, `crate::ui`). Nothing while a script fade is on (`fade`) or the mouse is hidden.
pub fn tooltip_shapes(mouse: &mut Mouse, now: u32, fade: bool) -> Vec<TooltipShape> {
    use rts_lockstep_engine::client::window_manager::DrawItem;
    let font_px = mouse.tooltip.font.as_ref().map_or(12, |f| f.point_size.max(1));
    let items = mouse.draw_tooltip(now, fade, |t, wrap| tooltip_text_size(font_px, t, wrap));
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
                let (w, h) = tooltip_text_size(font_px, &text, word_wrap.unwrap_or(0));
                let rect = [x as f32, y as f32, (x + w) as f32, (y + h) as f32];
                // the clip region where it meets the text (a strip left of the text shows nothing)
                let clip = clip.map_or(rect, |(a, b, c, d)| [(a as f32).max(rect[0]), (b as f32).max(rect[1]), (c as f32).min(rect[2]), (d as f32).min(rect[3])]);
                if clip[2] > clip[0] && clip[3] > clip[1] {
                    out.push(TooltipShape::Text { text: String::from_utf16_lossy(&text), rect, clip, color: rgba(color), font_px: font_px as f32 });
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
        assert_eq!(s[0], TooltipShape::Rect { rect: [120.0, 100.0, 122.0, 114.0], color: [0, 0, 0, 255] });
        assert!(!s.iter().any(|x| matches!(x, TooltipShape::Text { .. })), "nothing revealed yet: {s:?}");
        tooltip_shapes(&mut m, 1075, false);
        let s = tooltip_shapes(&mut m, 1076, false);
        let texts: Vec<_> = s.iter().filter_map(|x| if let TooltipShape::Text { clip, color, .. } = x { Some((*clip, *color)) } else { None }).collect();
        assert_eq!(texts, vec![([122.0, 101.0, 134.0, 113.0], [255, 255, 255, 255]), ([122.0, 101.0, 134.0, 113.0], [255, 255, 0, 255])]);
        assert!(tooltip_shapes(&mut m, 1080, true).is_empty(), "a script fade hides it");
        m.pos.x = 790;
        let s = tooltip_shapes(&mut m, 1100, false);
        assert!(matches!(s[0], TooltipShape::Rect { rect, .. } if rect[0] == 766.0), "{s:?}");
        m.tooltip.reset_tooltip_delay(1100);
        assert!(tooltip_shapes(&mut m, 1101, false).is_empty());
        apply_ui_mouse_calls(&mut m, vec![UiMouseCall::Tooltip { text: Vec::new(), color: None }], 1200, |_, _, _| {});
        m.tooltip.create_stream_messages(2000, false);
        assert!(tooltip_shapes(&mut m, 2000, false).is_empty());
    }

    /// #124, predicted before the run: two lines wrap separately (`"ab\ncdefgh"` at 12 px: 6 px a character, 36 pixels
    /// for the longer line, 24 high; wrapped at 24 pixels the longer takes two rows: 24 x 36); a decal of size 20 at
    /// (10, 20) with colour `0x00336699` and opacity 128 rings 10 units around it, half a unit over the ground, in
    /// (0x33, 0x66, 0x99, 128); an `InGameUI.ini` giving the guard area's texture loads it, and a missing file is reported.
    ///
    /// Results (2026-11-11): all held.
    #[test]
    fn the_tooltip_measure_the_decal_ring_and_in_game_ui_ini() {
        use rts_lockstep_engine::client::radius_cursor::{radius_cursor_type, DecalShadow, InGameUiRadiusCursor};
        let t: Vec<u16> = "ab\ncdefgh".encode_utf16().collect();
        assert_eq!(tooltip_text_size(12, &t, 0), (36, 24));
        assert_eq!(tooltip_text_size(12, &t, 24), (24, 36));
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

}
