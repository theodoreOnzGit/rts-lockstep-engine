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
}
