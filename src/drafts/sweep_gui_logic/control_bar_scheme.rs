//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarScheme.cpp (ControlBarScheme::updateAnim,
//!   ControlBarSchemeManager::setControlBarScheme)
//!
//! The verified `crate::client::control_bar_scheme` does not animate (its docs: "the animation (`update`,
//! `animSlideRight`) is not translated") and chooses a scheme by side or player; `setControlBarScheme(name)` is the
//! by-name one `ControlBar::setControlBarSchemeByName` calls.

use crate::client::control_bar_scheme::{ControlBarSchemeAnimation, ControlBarSchemeManager};

/// `ControlBarSchemeAnimation::CB_ANIM_SLIDE_RIGHT` (ControlBarScheme.h:103).
pub const CB_ANIM_SLIDE_RIGHT: i32 = 0;

/// `ControlBarScheme::updateAnim(anim)` (ControlBarScheme.cpp:726): dispatches on the animation type; only
/// `CB_ANIM_SLIDE_RIGHT` exists (`animSlideRight(anim)`), anything else is a `DEBUG_ASSERTCRASH`. `anim_type` is `m_animType`;
/// returns whether `animSlideRight` was called.
pub fn update_anim(_anim: &ControlBarSchemeAnimation, anim_type: i32, anim_slide_right: impl FnOnce()) -> bool {
    match anim_type {
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
/// `m_multiplyer` to, for the differential to settle which the game uses.
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
            let mx = if res.x != 0 { (display.0 / res.x as u32) as f32 } else { 0.0 };
            let my = if res.y != 0 { (display.1 / res.y as u32) as f32 } else { 0.0 };
            mgr.multiplier = (mx, my);
            mgr.current = Some(i);
        }
        None => {
            // DEBUG_ASSERTCRASH(FALSE,("There's no ControlBarScheme in the ControlBarSchemeList:m_schemeList"));
            mgr.current = None;
        }
    }
    if mgr.current.is_some() {
        init_current(mgr);
    }
}
