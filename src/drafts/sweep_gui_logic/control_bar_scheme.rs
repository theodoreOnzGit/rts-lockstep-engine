//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases per function (test: tests/differential_draft_cbscheme.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarScheme.cpp (ControlBarScheme::updateAnim,
//!   ControlBarSchemeManager::setControlBarScheme)
//!
//! The verified `crate::client::control_bar_scheme` does not animate (its docs: "the animation (`update`,
//! `animSlideRight`) is not translated") and chooses a scheme by side or player; `setControlBarScheme(name)` is the
//! by-name one `ControlBar::setControlBarSchemeByName` calls.

use crate::client::control_bar_scheme::{ControlBarSchemeAnimation, ControlBarSchemeManager};
use crate::logic::modules::plant;

/// `ControlBarSchemeAnimation::CB_ANIM_SLIDE_RIGHT` (ControlBarScheme.h:103).
pub const CB_ANIM_SLIDE_RIGHT: i32 = 0;

/// `ControlBarScheme::updateAnim(anim)` (ControlBarScheme.cpp:726): dispatches on the animation type; only
/// `CB_ANIM_SLIDE_RIGHT` exists (`animSlideRight(anim)`), anything else is a `DEBUG_ASSERTCRASH`. `anim_type` is `m_animType`;
/// returns whether `animSlideRight` was called.
pub fn update_anim(_anim: &ControlBarSchemeAnimation, anim_type: i32, anim_slide_right: impl FnOnce()) -> bool {
    match if plant(15738) && anim_type == 1 { CB_ANIM_SLIDE_RIGHT } else { anim_type } {
        _ if plant(15737) => {
            anim_slide_right();
            true
        }
        CB_ANIM_SLIDE_RIGHT => {
            anim_slide_right();
            true
        }
        _ => {
            // DEBUG_ASSERTCRASH(FALSE,("We tried to animate but not animate function was found %d", anim->m_animType));
            false
        }
    }
}

/// `ControlBarSchemeManager::setControlBarScheme(schemeName)` (ControlBarScheme.cpp:1060): the scheme of that name
/// becomes current and the multiplier is the display over the scheme's creation resolution; a name not in the list leaves
/// no current scheme. The current scheme is then `init`ed (`init_current`).
///
/// Upstream divides `TheDisplay->getWidth() / tempScheme->m_ScreenCreationRes.x` as integers (an unsigned width by an
/// `Int`) before storing in the `Real` multiplier, so 1024 / 800 is 1.0, not 1.28. The verified
/// `ControlBarScheme::init` (control_bar_scheme.rs:340) divides as floats: this is the integer quotient upstream sets
/// `m_multiplyer` to. **Settled 2026-11-19 (differential, #215, tests/differential_draft_cbscheme.rs, 20,000 cases, 0 mismatches):**
/// `setControlBarScheme(name)` (ControlBarScheme.cpp:1066) stores the integer quotient; `setControlBarSchemeByPlayer` (:1148) and
/// `...ByPlayerTemplate` (:1216) cast the divisor to `Real` and divide as floats, so the three upstream paths do not agree.
pub fn set_control_bar_scheme(
    mgr: &mut ControlBarSchemeManager,
    scheme_name: &[u8],
    display: (u32, u32),
    init_current: impl FnOnce(&ControlBarSchemeManager),
) {
    match mgr.find_control_bar_scheme(scheme_name) {
        Some(i) => {
            let res = mgr.schemes[i].screen_creation_res;
            // m_multiplyer.x = TheDisplay->getWidth() / tempScheme->m_ScreenCreationRes.x;  (UnsignedInt / Int)
            let mut mx = if res.x != 0 { (display.0 / res.x as u32) as f32 } else { 0.0 };
            let mut my = if res.y != 0 { (display.1 / res.y as u32) as f32 } else { 0.0 };
            if plant(15739) {
                mx = display.0 as f32 / res.x as f32;
                my = display.1 as f32 / res.y as f32;
            }
            if plant(15740) {
                std::mem::swap(&mut mx, &mut my);
            }
            mgr.multiplier = (mx, my);
            mgr.current = Some(i);
        }
        None => {
            // DEBUG_ASSERTCRASH(FALSE,("There's no ControlBarScheme in the ControlBarSchemeList:m_schemeList"));
            if !plant(15741) {
                mgr.current = None;
            }
        }
    }
    if mgr.current.is_some() || plant(15742) {
        init_current(mgr);
    }
}
