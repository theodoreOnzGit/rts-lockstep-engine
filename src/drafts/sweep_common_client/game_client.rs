//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GameClient.cpp   (`addTextBearingDrawable`,
//!     `flushTextBearingDrawables`, `removeFromRayEffects`)
//!
//! Translated already (#213), no draft: `registerDrawable`, `addDrawableToLookupTable`, `removeDrawableFromLookupTable`
//! are `crate::client::drawable::ClientDrawables` (`alloc_drawable_id`, the `drawables` / `logic_drawables` maps keyed by
//! id, which are the lookup table and the master list). Excluded: `GameClient::unloadMap` is `assert( 0 ); // who
//! calls this?` and nothing else (never called); `assignSelectedDrawablesToGroup` and `selectDrawablesInGroup` have their
//! whole bodies commented out upstream (empty functions).

use crate::drafts::logic_common::ray_effect::RayEffectSystem;
use crate::common::game_type::ObjectId;

/// `GameClient::m_textBearingDrawableList`: the drawables that draw UI text this frame (their ids).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextBearingDrawables {
    /// The list, in push order.
    pub list: Vec<u32>,
}

impl TextBearingDrawables {
    /// `GameClient::addTextBearingDrawable(tbd)` (GameClient.cpp:989): NULL is ignored; else `push_back`.
    pub fn add_text_bearing_drawable(&mut self, tbd: Option<u32>) {
        if let Some(d) = tbd {
            self.list.push(d);
        }
    }

    /// `GameClient::flushTextBearingDrawables()` (GameClient.cpp:995): walk the list calling each drawable's
    /// `drawUIText()` (the closure; see `DrawableUi::draw_ui_text`), then clear it, so the group numbers draw last, in front.
    pub fn flush_text_bearing_drawables(&mut self, draw_ui_text: &mut dyn FnMut(u32)) {
        for &d in self.list.iter() {
            draw_ui_text(d);
        }
        self.list.clear();
    }
}

/// `GameClient::removeFromRayEffects(draw)` (GameClient.cpp:1032): `TheRayEffects->deleteRayEffect(draw)`.
pub fn remove_from_ray_effects(ray_effects: &mut RayEffectSystem, draw: Option<ObjectId>) {
    ray_effects.delete_ray_effect(draw);
}
