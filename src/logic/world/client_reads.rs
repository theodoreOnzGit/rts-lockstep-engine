// SPDX-License-Identifier: GPL-3.0-only
//! The values logic reads from the client, as logic-owned state.
//!
//! Upstream logic is *almost* independent of the client, but a handful of
//! reads go the other way (`docs/phase1-porting-graph.md`, "Reads back").
//! Each becomes a field of [`ClientReads`], owned by the world and written
//! through the world, never fetched from a client at the point of use. A
//! headless run uses the defaults below, which are documented per field.
//!
//! **The rule:** a read from the client that can change logic is state of
//! the world. A client that exists *feeds* it, at a fixed point (the start of
//! the logic frame, with the commands), so that a replay or a peer can be fed
//! the same values; logic never calls out. A read that only decides whether to
//! play or draw something is not a read at all: the decision moves to the
//! client, and the event carries what it needs.
//!
//! `docs/world-design.md` lists every read from the porting doc with its
//! disposition. This struct holds the ones that are state.

use std::collections::BTreeMap;

use crate::common::base_type::Coord3D;
use crate::common::game_type::ObjectId;

/// See the module docs. `Default` is the headless run.
#[derive(Clone, Debug, PartialEq)]
pub struct ClientReads {
    /// `TheTacticalView->isTimeFrozen()` (`GameLogic::update` freezes logic
    /// when this holds and the camera is still moving). Headless: `false`.
    pub camera_time_frozen: bool,
    /// `TheTacticalView->isCameraMovementFinished()` (the freeze condition, a
    /// script condition, and `GameLogicDispatch`). Headless: `true`, so a
    /// script that waits for a camera move never waits.
    pub camera_movement_finished: bool,
    /// `TheTacticalView->getPosition()` (`ScriptActions`). Headless: the origin.
    pub camera_position: Coord3D,
    /// `TheTacticalView->getViewFilterType() == FT_VIEW_BW_FILTER`
    /// (`ScriptActions`). Headless: `false`.
    pub camera_filter_is_black_and_white: bool,
    /// `TheInGameUI->getAllSelectedDrawables()`, as the objects they belong to
    /// (`ScriptConditions`, `ActiveBody`, `ProductionUpdate` use the first).
    /// Headless: none.
    pub local_selection: Vec<ObjectId>,
    /// `TheInGameUI->getFrameSelectionChanged()` (`ScriptConditions`). Headless: 0.
    pub frame_selection_changed: u32,
    /// `TheAudio->getAudioLengthMS(&event)` (`ScriptEngine`: how long a
    /// speech takes), keyed by event name (bytes, as `AudioEventName`). It is a property of the audio
    /// files, so it is filled from game data at load, identically on every
    /// peer. Headless: empty, and [`ClientReads::audio_length_ms`] answers 0.
    pub audio_length_ms: BTreeMap<Vec<u8>, f32>,
    /// `TheAudio->hasMusicTrackCompleted(name, count)` (`ScriptConditions`):
    /// how many times each track has completed. Headless: empty (never).
    pub music_track_completions: BTreeMap<Vec<u8>, i32>,
    // --- p1w13-sonnet ---
    /// `Drawable::getModelConditionFlags()` of each object's drawable (`OpenContain::monitorConditionChanges` redeploys its occupants when it changes). Headless:
    /// empty, and [`ClientReads::drawable_model_condition_flags`] answers the empty set.
    pub drawable_model_condition:
        BTreeMap<ObjectId, crate::common::model_state::ModelConditionFlags>,
    // --- end p1w13-sonnet ---
}

impl Default for ClientReads {
    fn default() -> Self {
        ClientReads {
            camera_time_frozen: false,
            camera_movement_finished: true,
            camera_position: Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            camera_filter_is_black_and_white: false,
            local_selection: Vec::new(),
            frame_selection_changed: 0,
            audio_length_ms: BTreeMap::new(),
            music_track_completions: BTreeMap::new(),
            // --- p1w13-sonnet ---
            drawable_model_condition: BTreeMap::new(),
            // --- end p1w13-sonnet ---
        }
    }
}

impl ClientReads {
    // --- p1w13-sonnet ---
    /// `drawable->getModelConditionFlags()`: the flags of the object's drawable, the empty set for an object the client has not told about.
    pub fn drawable_model_condition_flags(
        &self,
        id: ObjectId,
    ) -> crate::common::model_state::ModelConditionFlags {
        self.drawable_model_condition
            .get(&id)
            .cloned()
            .unwrap_or_default()
    }
    // --- end p1w13-sonnet ---

    /// `GameLogic::update`'s `freezeTime` camera term:
    /// `isTimeFrozen() && !isCameraMovementFinished()`.
    pub fn camera_freezes_time(&self) -> bool {
        self.camera_time_frozen && !self.camera_movement_finished
    }

    /// `getAudioLengthMS`: the length in milliseconds, 0 for an unknown event.
    pub fn audio_length_ms(&self, event: &[u8]) -> f32 {
        self.audio_length_ms.get(event).copied().unwrap_or(0.0)
    }

    /// `hasMusicTrackCompleted(track, count)`: completed at least `count` times.
    pub fn has_music_track_completed(&self, track: &[u8], count: i32) -> bool {
        self.music_track_completions
            .get(track)
            .is_some_and(|&n| n >= count)
    }

    /// `getFirstSelectedDrawable()`'s object.
    pub fn first_selected(&self) -> Option<ObjectId> {
        self.local_selection.first().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_defaults_never_freeze_and_never_wait() {
        let r = ClientReads::default();
        assert!(!r.camera_freezes_time());
        assert!(r.camera_movement_finished);
        assert_eq!(r.first_selected(), None);
        assert_eq!(r.audio_length_ms(b"x"), 0.0);
        assert!(!r.has_music_track_completed(b"t", 1));
    }

    #[test]
    fn the_freeze_needs_both_terms() {
        let mut r = ClientReads {
            camera_time_frozen: true,
            ..ClientReads::default()
        };
        assert!(
            !r.camera_freezes_time(),
            "frozen but the camera is finished"
        );
        r.camera_movement_finished = false;
        assert!(r.camera_freezes_time());
        r.camera_time_frozen = false;
        assert!(!r.camera_freezes_time());
        r.music_track_completions.insert(b"t".to_vec(), 2);
        assert!(r.has_music_track_completed(b"t", 2));
        assert!(!r.has_music_track_completed(b"t", 3));
        r.local_selection = vec![9, 4];
        assert_eq!(r.first_selected(), Some(9));
    }
}
