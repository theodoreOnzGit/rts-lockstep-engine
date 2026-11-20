//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_game_client.rs);
//! not yet wired into the game.
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
            if crate::logic::modules::plant(19881) {
                self.list.insert(0, d);
            } else if !(crate::logic::modules::plant(19885) && self.list.contains(&d)) {
                self.list.push(d);
            }
        }
    }

    /// `GameClient::flushTextBearingDrawables()` (GameClient.cpp:995): walk the list calling each drawable's
    /// `drawUIText()` (the closure; see `DrawableUi::draw_ui_text`), then clear it, so the group numbers draw last, in front.
    pub fn flush_text_bearing_drawables(&mut self, draw_ui_text: &mut dyn FnMut(u32)) {
        if crate::logic::modules::plant(19883) {
            for &d in self.list.iter().rev() {
                draw_ui_text(d);
            }
        } else {
            for &d in self.list.iter() {
                draw_ui_text(d);
            }
        }
        if !crate::logic::modules::plant(19882) {
            self.list.clear();
        }
    }
}

/// `GameClient::removeFromRayEffects(draw)` (GameClient.cpp:1032): `TheRayEffects->deleteRayEffect(draw)`.
pub fn remove_from_ray_effects(ray_effects: &mut RayEffectSystem, draw: Option<ObjectId>) {
    if !crate::logic::modules::plant(19884) {
        ray_effects.delete_ray_effect(draw);
    }
}
