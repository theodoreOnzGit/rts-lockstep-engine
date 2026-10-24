// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/ExperienceTracker.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/ExperienceTracker.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Graham Smallwood, February 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ExperienceTracker`: the experience points and the veterancy level of one
//! object (a unit gains levels by killing things, or by being set to one).
//!
//! # State and cut points
//!
//! The tracker owns its state: the level, the experience points, the sink
//! (the object that receives this one's experience) and the scalar. It does
//! not hold its owning `Object`. Everything upstream reads from the object
//! or its template is passed in, and everything upstream calls on the object
//! is returned:
//!
//! - **The template** (`m_parent->getTemplate()`: `getExperienceValue`,
//!   `getExperienceRequired`, `isTrainable`) is an [`ExperienceRules`]
//!   argument, copied from the template by the caller.
//! - **`killer->getRelationship(m_parent) == ALLIES`** (`getExperienceValue`)
//!   is the boolean `killer_is_ally`, computed by the caller.
//! - **`m_parent->onVeterancyLevelChanged(old, new[, feedback])`** is a
//!   [`LevelChange`] pushed to the caller's vector, in the order upstream makes
//!   the calls. The owner runs the effects (sounds, the model, the UI).
//! - **`TheGameLogic->findObjectByID(sink)` and the sink's own tracker** are
//!   the [`ExperienceArena`] trait: upstream's `addExperiencePoints` and
//!   `setExperienceAndLevel` forward to the sink's tracker when the sink
//!   object exists, so the routing functions ([`add_experience_points`],
//!   [`set_experience_and_level`], [`gain_exp_for_level`]) take the arena. The
//!   arena is the world's object table in the full game.
//!
//! # Semantics kept from upstream
//!
//! - **Level from points.** A gain recomputes the level from the new total:
//!   the highest level whose requirement the total meets (upstream's loop:
//!   while there is a next level and the total reaches its requirement, step
//!   up). So a total below the current level's requirement lowers the level,
//!   as upstream does.
//! - **Scaling.** With `canScaleForBonus` the gain is multiplied by the
//!   scalar in `Real` and truncated to `Int` (`amountToGain *= scalar`); NaN
//!   or an out-of-range product gives `i32::MIN`, as x86 does (see
//!   `real_to_int_trunc`). The product is one `f32` rounding, no fused
//!   multiply-add, and the differential build is SSE (the shipped x87 build
//!   is not covered, `SCOPE.md` section 3).
//!   The sink path multiplies by the *sender's* scalar, then the sink
//!   applies its own, as upstream.
//! - **Trainability.** `addExperiencePoints` and `setExperienceAndLevel`
//!   return early for a non-trainable owner, after the sink check. The
//!   explicit setters (`setVeterancyLevel`, `setMinVeterancyLevel`) do not
//!   check it, as upstream.
//! - **Wrapping.** Experience is an `Int` and upstream builds with `-fwrapv`
//!   (the MSVC x86 behaviour), so the additions and subtractions wrap. The
//!   checked Rust release profile would otherwise panic on the same input.
//! - **`gainExpForLevel`** clamps the target level to `LEVEL_LAST` and gains
//!   `required(target) - current` points through `addExperiencePoints`, so a
//!   sink receives them.
//!
//! # Deliberate differences
//!
//! - **`xfer` of the level.** Upstream transfers the level's four bytes and
//!   accepts any integer on load (an out-of-range level then indexes past the
//!   tables). [`VeterancyLevel`] cannot hold such a value, so a load of one is
//!   refused with [`XferStatus::InvalidParameters`].
//! - **Cycles.** A sink chain that loops (an object whose sink leads back to
//!   itself) recurses without end upstream. Here it does too; nothing guards
//!   it, and the game never builds one.
//! - **No `Snapshot` impl.** The trait's methods take `&mut dyn Xfer`; this
//!   crate uses generics, so [`ExperienceTracker::xfer`] and
//!   [`ExperienceTracker::crc`] are inherent and generic over the transfer.

use crate::common::game_common::{VeterancyLevel, LEVEL_COUNT};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::xfer::{Xfer, XferMode, XferResult, XferStatus, XferVersion};

/// `LEVEL_COUNT` as an index.
pub const LEVELS: usize = LEVEL_COUNT as usize;
/// `LEVEL_LAST` (`GameCommon.h`): the highest level, `LEVEL_HEROIC`.
pub const LEVEL_LAST: i32 = LEVEL_COUNT - 1;

/// The template values the tracker reads, copied from the owner's
/// `ThingTemplate` by the caller (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExperienceRules {
    /// `getExperienceValue(level)`: the points an enemy gets for killing it.
    pub experience_values: [i32; LEVELS],
    /// `getExperienceRequired(level)`: the points that reach the level.
    pub experience_required: [i32; LEVELS],
    /// `isTrainable()`: whether the owner gains experience at all.
    pub trainable: bool,
}

/// A level change the owner must be told about: `onVeterancyLevelChanged(old,
/// new, provide_feedback)` on object `object`, in the order upstream calls it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelChange {
    /// The object whose level changed (the tracker's owner).
    pub object: ObjectId,
    /// `oldLevel`.
    pub old: VeterancyLevel,
    /// `newLevel`.
    pub new: VeterancyLevel,
    /// `provideFeedback`: `TRUE` for every call except the setters that pass
    /// `FALSE`.
    pub provide_feedback: bool,
}

/// What routing needs from the world: the tracker of an object, and its
/// template's rules, by id. `None` for an id no object has (upstream's
/// `findObjectByID` returning `NULL`).
pub trait ExperienceArena {
    /// The tracker of object `id`, with its template's rules.
    fn experience_of(&mut self, id: ObjectId) -> Option<(&mut ExperienceTracker, ExperienceRules)>;
}

/// `class ExperienceTracker`: the state of one owner's experience.
#[derive(Clone, Debug, PartialEq)]
pub struct ExperienceTracker {
    /// `m_parent`: the owner's id, the object the level changes are for.
    owner: ObjectId,
    /// `m_currentLevel`.
    current_level: VeterancyLevel,
    /// `m_currentExperience`.
    current_experience: i32,
    /// `m_experienceSink`: the object that receives this one's points, or
    /// `INVALID_ID`.
    experience_sink: ObjectId,
    /// `m_experienceScalar`: multiplies any experience gained.
    experience_scalar: f32,
}

/// C's `(Int)f` for a `Real`: truncation toward zero, and for NaN or a value
/// outside the `Int` range the x86 "integer indefinite" `0x80000000` (what
/// `cvttss2si` and the shipped build's `fistp` give; Rust's `as` would
/// saturate instead). Used for `experienceGain * m_experienceScalar`.
fn real_to_int_trunc(f: f32) -> i32 {
    if f.is_nan() || f64::from(f) < -2_147_483_648.0 || f64::from(f) >= 2_147_483_648.0 {
        return i32::MIN;
    }
    f as i32
}

/// `VeterancyLevel` from its index; `None` outside `0..LEVEL_COUNT`.
fn level_from_index(index: i32) -> Option<VeterancyLevel> {
    match index {
        0 => Some(VeterancyLevel::Regular),
        1 => Some(VeterancyLevel::Veteran),
        2 => Some(VeterancyLevel::Elite),
        3 => Some(VeterancyLevel::Heroic),
        _ => None,
    }
}

/// The level the points reach: the highest level index `i` such that every
/// level up to `i` has its requirement met (`levelIndex` of upstream's loop).
fn level_for_experience(experience: i32, rules: &ExperienceRules) -> i32 {
    let mut level_index = 0;
    while level_index + 1 < LEVEL_COUNT && experience >= rules.experience_required[(level_index + 1) as usize] {
        level_index += 1;
    }
    level_index
}

/// `(Int) m_currentLevel + levelsToGain`, clamped to `LEVEL_LAST`.
fn target_level(current: VeterancyLevel, levels_to_gain: i32) -> i32 {
    let new_level = (current as i32).wrapping_add(levels_to_gain);
    new_level.min(LEVEL_LAST)
}

impl ExperienceTracker {
    /// `ExperienceTracker(Object *parent)`: regular, no points, no sink, scalar
    /// 1.0. `owner` is the id of the object it belongs to.
    pub fn new(owner: ObjectId) -> Self {
        ExperienceTracker {
            owner,
            current_level: VeterancyLevel::Regular,
            current_experience: 0,
            experience_sink: INVALID_ID,
            experience_scalar: 1.0,
        }
    }

    /// `getVeterancyLevel()`.
    pub fn veterancy_level(&self) -> VeterancyLevel {
        self.current_level
    }

    /// `getCurrentExperience()`.
    pub fn current_experience(&self) -> i32 {
        self.current_experience
    }

    /// The sink set by `setExperienceSink`. Upstream has no getter (the field
    /// is private); this is for the owner's save checks and the tests.
    pub fn experience_sink(&self) -> ObjectId {
        self.experience_sink
    }

    /// `setExperienceSink(sink)`.
    pub fn set_experience_sink(&mut self, sink: ObjectId) {
        self.experience_sink = sink;
    }

    /// `getExperienceScalar()`.
    pub fn experience_scalar(&self) -> f32 {
        self.experience_scalar
    }

    /// `setExperienceScalar(scalar)`.
    pub fn set_experience_scalar(&mut self, scalar: f32) {
        self.experience_scalar = scalar;
    }

    /// `getExperienceValue(killer)`: nothing for an ally (the caller passes
    /// `killer_is_ally`), otherwise the template's value for the current level.
    pub fn experience_value(&self, killer_is_ally: bool, rules: &ExperienceRules) -> i32 {
        if killer_is_ally {
            return 0;
        }
        rules.experience_values[self.current_level as usize]
    }

    /// `isTrainable()`.
    pub fn is_trainable(&self, rules: &ExperienceRules) -> bool {
        rules.trainable
    }

    /// `isAcceptingExperiencePoints()`: trainable, or a sink is set.
    pub fn is_accepting_experience_points(&self, rules: &ExperienceRules) -> bool {
        self.is_trainable(rules) || self.experience_sink != INVALID_ID
    }

    /// `setVeterancyLevel(newLevel, provideFeedback)`: sets the level and its
    /// minimum points. Does nothing for the level it already has. Not gated by
    /// trainability.
    pub fn set_veterancy_level(
        &mut self,
        new_level: VeterancyLevel,
        rules: &ExperienceRules,
        provide_feedback: bool,
        changes: &mut Vec<LevelChange>,
    ) {
        if self.current_level != new_level {
            let old = self.current_level;
            self.current_level = new_level;
            self.current_experience = rules.experience_required[new_level as usize];
            changes.push(LevelChange { object: self.owner, old, new: new_level, provide_feedback });
        }
    }

    /// `setMinVeterancyLevel(newLevel)`: raises the level to `newLevel` if it is
    /// below it (with feedback); never lowers it.
    pub fn set_min_veterancy_level(
        &mut self,
        new_level: VeterancyLevel,
        rules: &ExperienceRules,
        changes: &mut Vec<LevelChange>,
    ) {
        if (self.current_level as i32) < (new_level as i32) {
            let old = self.current_level;
            self.current_level = new_level;
            self.current_experience = rules.experience_required[new_level as usize];
            changes.push(LevelChange { object: self.owner, old, new: new_level, provide_feedback: true });
        }
    }

    /// The local part of `addExperiencePoints`: after the sink has been
    /// checked and found absent. Gated by trainability.
    fn add_local(
        &mut self,
        rules: &ExperienceRules,
        experience_gain: i32,
        can_scale_for_bonus: bool,
        changes: &mut Vec<LevelChange>,
    ) {
        if !rules.trainable {
            return;
        }
        let old = self.current_level;
        let mut amount_to_gain = experience_gain;
        if can_scale_for_bonus {
            amount_to_gain = real_to_int_trunc(amount_to_gain as f32 * self.experience_scalar);
        }
        self.current_experience = self.current_experience.wrapping_add(amount_to_gain);
        let new_level = level_for_experience(self.current_experience, rules);
        self.current_level = level_from_index(new_level).expect("level index is below LEVEL_COUNT");
        if old != self.current_level {
            changes.push(LevelChange {
                object: self.owner,
                old,
                new: self.current_level,
                provide_feedback: true,
            });
        }
    }

    /// The local part of `setExperienceAndLevel`: after the sink check. Gated by
    /// trainability; the points are set, not added.
    fn set_local(
        &mut self,
        rules: &ExperienceRules,
        experience_in: i32,
        provide_feedback: bool,
        changes: &mut Vec<LevelChange>,
    ) {
        if !rules.trainable {
            return;
        }
        let old = self.current_level;
        self.current_experience = experience_in;
        let new_level = level_for_experience(self.current_experience, rules);
        self.current_level = level_from_index(new_level).expect("level index is below LEVEL_COUNT");
        if old != self.current_level {
            changes.push(LevelChange { object: self.owner, old, new: self.current_level, provide_feedback });
        }
    }

    /// `crc(xfer)`: the points, then the level.
    pub fn crc<X: Xfer + ?Sized>(&mut self, xfer: &mut X) -> XferResult<()> {
        xfer.xfer_int(&mut self.current_experience)?;
        xfer_level(xfer, &mut self.current_level)
    }

    /// `xfer(xfer)`, version 1: the version, the level, the points, the sink
    /// and the scalar. The owner is not saved (it is set at allocation).
    pub fn xfer<X: Xfer + ?Sized>(&mut self, xfer: &mut X) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        xfer_level(xfer, &mut self.current_level)?;
        xfer.xfer_int(&mut self.current_experience)?;
        xfer.xfer_object_id(&mut self.experience_sink)?;
        xfer.xfer_real(&mut self.experience_scalar)
    }

    /// `loadPostProcess()`: empty upstream and here.
    pub fn load_post_process(&mut self) {}
}

/// `xferUser(&m_currentLevel, sizeof(VeterancyLevel))`: the four bytes of the
/// enum (little-endian, as the Xfer layer writes every integer). A load of a
/// value that is not a level is refused (see the module docs).
fn xfer_level<X: Xfer + ?Sized>(xfer: &mut X, level: &mut VeterancyLevel) -> XferResult<()> {
    let mut bytes = (*level as i32).to_le_bytes();
    xfer.xfer_user(&mut bytes)?;
    if xfer.get_xfer_mode() == XferMode::Load {
        let index = i32::from_le_bytes(bytes);
        *level = level_from_index(index).ok_or(XferStatus::InvalidParameters)?;
    }
    Ok(())
}

/// `addExperiencePoints(experienceGain, canScaleForBonus)` on object `owner`,
/// routed to the sink when the sink object exists: the sink gets
/// `experienceGain * scalar` (truncated to `Int`, scaled by this owner's
/// scalar), with the same `canScaleForBonus`, and this owner gets nothing.
pub fn add_experience_points<A: ExperienceArena>(
    arena: &mut A,
    owner: ObjectId,
    experience_gain: i32,
    can_scale_for_bonus: bool,
    changes: &mut Vec<LevelChange>,
) {
    let Some((tracker, _)) = arena.experience_of(owner) else {
        return;
    };
    let sink = tracker.experience_sink;
    let scalar = tracker.experience_scalar;
    if sink != INVALID_ID && arena.experience_of(sink).is_some() {
        let forwarded = real_to_int_trunc(experience_gain as f32 * scalar);
        add_experience_points(arena, sink, forwarded, can_scale_for_bonus, changes);
        return;
    }
    let Some((tracker, rules)) = arena.experience_of(owner) else {
        return;
    };
    tracker.add_local(&rules, experience_gain, can_scale_for_bonus, changes);
}

/// `setExperienceAndLevel(experienceIn, provideFeedback)` on object `owner`,
/// routed to the sink when the sink object exists (the sink gets the same
/// points, unscaled).
pub fn set_experience_and_level<A: ExperienceArena>(
    arena: &mut A,
    owner: ObjectId,
    experience_in: i32,
    provide_feedback: bool,
    changes: &mut Vec<LevelChange>,
) {
    let Some((tracker, _)) = arena.experience_of(owner) else {
        return;
    };
    let sink = tracker.experience_sink;
    if sink != INVALID_ID && arena.experience_of(sink).is_some() {
        set_experience_and_level(arena, sink, experience_in, provide_feedback, changes);
        return;
    }
    let Some((tracker, rules)) = arena.experience_of(owner) else {
        return;
    };
    tracker.set_local(&rules, experience_in, provide_feedback, changes);
}

/// `canGainExpForLevel(levelsToGain)`: whether `gain_exp_for_level` would gain
/// anything, without changing it.
pub fn can_gain_exp_for_level(tracker: &ExperienceTracker, levels_to_gain: i32) -> bool {
    target_level(tracker.current_level, levels_to_gain) > tracker.current_level as i32
}

/// `gainExpForLevel(levelsToGain, canScaleForBonus)` on object `owner`: gains
/// the points that reach the target level (clamped to `LEVEL_LAST`) through
/// `addExperiencePoints`, so a sink receives them. Returns whether the target
/// is above the current level.
pub fn gain_exp_for_level<A: ExperienceArena>(
    arena: &mut A,
    owner: ObjectId,
    levels_to_gain: i32,
    can_scale_for_bonus: bool,
    changes: &mut Vec<LevelChange>,
) -> bool {
    let Some((tracker, rules)) = arena.experience_of(owner) else {
        return false;
    };
    let new_level = target_level(tracker.current_level, levels_to_gain);
    if new_level > tracker.current_level as i32 {
        let experience_needed =
            rules.experience_required[new_level as usize].wrapping_sub(tracker.current_experience);
        add_experience_points(arena, owner, experience_needed, can_scale_for_bonus, changes);
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::xfer_load::XferLoad;
    use crate::common::xfer_save::XferSave;
    use std::collections::BTreeMap;

    /// The rules of a template with requirements 0, 100, 250, 500 and values 1, 2, 3, 4.
    fn rules() -> ExperienceRules {
        ExperienceRules { experience_values: [1, 2, 3, 4], experience_required: [0, 100, 250, 500], trainable: true }
    }

    /// Objects by id, each with its tracker and rules: the arena of the tests.
    #[derive(Default)]
    struct Arena {
        objects: BTreeMap<ObjectId, (ExperienceTracker, ExperienceRules)>,
    }

    impl ExperienceArena for Arena {
        fn experience_of(&mut self, id: ObjectId) -> Option<(&mut ExperienceTracker, ExperienceRules)> {
            self.objects.get_mut(&id).map(|(t, r)| (t, *r))
        }
    }

    fn arena_with(ids: &[ObjectId]) -> Arena {
        let mut a = Arena::default();
        for &id in ids {
            a.objects.insert(id, (ExperienceTracker::new(id), rules()));
        }
        a
    }

    #[test]
    fn a_gain_climbs_to_the_highest_level_it_reaches() {
        let mut a = arena_with(&[1]);
        let mut changes = Vec::new();
        add_experience_points(&mut a, 1, 260, false, &mut changes);
        let (t, _) = a.objects.get(&1).unwrap();
        assert_eq!(t.veterancy_level(), VeterancyLevel::Elite);
        assert_eq!(t.current_experience(), 260);
        assert_eq!(
            changes,
            vec![LevelChange {
                object: 1,
                old: VeterancyLevel::Regular,
                new: VeterancyLevel::Elite,
                provide_feedback: true
            }]
        );
    }

    #[test]
    fn scaling_truncates_the_product_and_untrainable_owners_gain_nothing() {
        let mut a = arena_with(&[1]);
        a.objects.get_mut(&1).unwrap().0.set_experience_scalar(1.5);
        let mut changes = Vec::new();
        add_experience_points(&mut a, 1, 101, true, &mut changes);
        assert_eq!(a.objects.get(&1).unwrap().0.current_experience(), 151);
        add_experience_points(&mut a, 1, 101, false, &mut changes);
        assert_eq!(a.objects.get(&1).unwrap().0.current_experience(), 252);

        a.objects.get_mut(&1).unwrap().1.trainable = false;
        add_experience_points(&mut a, 1, 1000, false, &mut changes);
        assert_eq!(a.objects.get(&1).unwrap().0.current_experience(), 252);
    }

    #[test]
    fn a_sink_receives_the_gain_scaled_by_the_sender() {
        let mut a = arena_with(&[1, 2]);
        a.objects.get_mut(&1).unwrap().0.set_experience_sink(2);
        a.objects.get_mut(&1).unwrap().0.set_experience_scalar(2.0);
        a.objects.get_mut(&2).unwrap().0.set_experience_scalar(0.5);
        let mut changes = Vec::new();
        add_experience_points(&mut a, 1, 100, true, &mut changes);
        assert_eq!(a.objects.get(&1).unwrap().0.current_experience(), 0);
        // 100 * 2.0 = 200 at the sender; the sink scales its own 200 by 0.5 = 100.
        assert_eq!(a.objects.get(&2).unwrap().0.current_experience(), 100);
    }

    #[test]
    fn a_sink_that_does_not_exist_keeps_the_points_locally() {
        let mut a = arena_with(&[1]);
        a.objects.get_mut(&1).unwrap().0.set_experience_sink(9);
        let mut changes = Vec::new();
        add_experience_points(&mut a, 1, 30, false, &mut changes);
        assert_eq!(a.objects.get(&1).unwrap().0.current_experience(), 30);
    }

    #[test]
    fn gain_for_level_reaches_the_target_exactly_and_is_clamped() {
        let mut a = arena_with(&[1]);
        let mut changes = Vec::new();
        assert!(gain_exp_for_level(&mut a, 1, 1, false, &mut changes));
        assert_eq!(a.objects.get(&1).unwrap().0.current_experience(), 100);
        assert!(gain_exp_for_level(&mut a, 1, 9, false, &mut changes));
        let (t, _) = a.objects.get(&1).unwrap();
        assert_eq!(t.veterancy_level(), VeterancyLevel::Heroic);
        assert_eq!(t.current_experience(), 500);
        assert!(!gain_exp_for_level(&mut a, 1, 1, false, &mut changes));
        assert!(!can_gain_exp_for_level(&a.objects.get(&1).unwrap().0, 1));
    }

    #[test]
    fn set_experience_and_level_sets_the_points_and_routes_to_a_sink() {
        let mut a = arena_with(&[1, 2]);
        let mut changes = Vec::new();
        set_experience_and_level(&mut a, 1, 300, false, &mut changes);
        assert_eq!(a.objects.get(&1).unwrap().0.veterancy_level(), VeterancyLevel::Elite);
        assert!(!changes[0].provide_feedback);
        a.objects.get_mut(&1).unwrap().0.set_experience_sink(2);
        set_experience_and_level(&mut a, 1, 40, true, &mut changes);
        assert_eq!(a.objects.get(&1).unwrap().0.current_experience(), 300);
        assert_eq!(a.objects.get(&2).unwrap().0.current_experience(), 40);
    }

    #[test]
    fn explicit_level_setters_and_the_value_of_a_kill() {
        let mut t = ExperienceTracker::new(7);
        let r = rules();
        let mut changes = Vec::new();
        t.set_veterancy_level(VeterancyLevel::Veteran, &r, false, &mut changes);
        assert_eq!(t.current_experience(), 100);
        t.set_veterancy_level(VeterancyLevel::Veteran, &r, true, &mut changes);
        assert_eq!(changes.len(), 1);
        t.set_min_veterancy_level(VeterancyLevel::Regular, &r, &mut changes);
        assert_eq!(t.veterancy_level(), VeterancyLevel::Veteran);
        t.set_min_veterancy_level(VeterancyLevel::Elite, &r, &mut changes);
        assert_eq!(t.veterancy_level(), VeterancyLevel::Elite);
        assert_eq!(t.experience_value(false, &r), 3);
        assert_eq!(t.experience_value(true, &r), 0);
        assert!(t.is_accepting_experience_points(&r));
        t.set_experience_sink(INVALID_ID);
        assert!(t.is_trainable(&r));
    }

    #[test]
    fn save_then_load_and_crc_cover_every_field() {
        let mut t = ExperienceTracker::new(3);
        let r = rules();
        let mut changes = Vec::new();
        t.set_experience_scalar(1.25);
        t.set_experience_sink(5);
        t.set_min_veterancy_level(VeterancyLevel::Elite, &r, &mut changes);
        let mut save = XferSave::new();
        save.open(b"exp").unwrap();
        t.clone().xfer(&mut save).unwrap();
        save.close().unwrap();
        let bytes = save.take_bytes();
        let mut loaded = ExperienceTracker::new(3);
        let mut load = XferLoad::new(bytes);
        load.open(b"exp").unwrap();
        loaded.xfer(&mut load).unwrap();
        load.close().unwrap();
        assert_eq!(loaded.veterancy_level(), VeterancyLevel::Elite);
        assert_eq!(loaded.current_experience(), 250);
        assert_eq!(loaded.experience_sink(), 5);
        assert_eq!(loaded.experience_scalar(), 1.25);
        loaded.load_post_process();

        let mut crc = XferSave::new();
        crc.open(b"crc").unwrap();
        t.crc(&mut crc).unwrap();
        crc.close().unwrap();
        assert_eq!(crc.take_bytes().len(), 4 + 4);
    }

    #[test]
    fn an_out_of_range_level_is_refused_on_load() {
        // version 1, then the level as four little-endian bytes: 9 is no level
        let mut load = XferLoad::new(vec![1, 9, 0, 0, 0]);
        load.open(b"bad").unwrap();
        let mut t = ExperienceTracker::new(1);
        assert_eq!(t.xfer(&mut load), Err(XferStatus::InvalidParameters));
        load.close().unwrap();
    }

    #[test]
    fn the_scaled_product_truncates_like_c_and_overflows_to_the_indefinite_value() {
        assert_eq!(real_to_int_trunc(2.9), 2);
        assert_eq!(real_to_int_trunc(-2.9), -2);
        assert_eq!(real_to_int_trunc(f32::NAN), i32::MIN);
        assert_eq!(real_to_int_trunc(3.0e9), i32::MIN);
        assert_eq!(real_to_int_trunc(-3.0e9), i32::MIN);
        assert_eq!(real_to_int_trunc(-2147483648.0), i32::MIN);

        // a gain whose scaled product overflows Int adds the indefinite value, and the sum wraps
        let mut a = arena_with(&[1]);
        a.objects.get_mut(&1).unwrap().0.set_experience_scalar(1.0e9);
        let mut changes = Vec::new();
        add_experience_points(&mut a, 1, 100, true, &mut changes);
        assert_eq!(a.objects.get(&1).unwrap().0.current_experience(), i32::MIN);
        add_experience_points(&mut a, 1, i32::MIN, false, &mut changes);
        assert_eq!(a.objects.get(&1).unwrap().0.current_experience(), 0, "i32::MIN + i32::MIN wraps");
    }

    #[test]
    fn a_total_below_the_current_requirement_lowers_the_level() {
        let mut a = arena_with(&[1]);
        let mut changes = Vec::new();
        add_experience_points(&mut a, 1, 300, false, &mut changes);
        add_experience_points(&mut a, 1, -250, false, &mut changes);
        let (t, _) = a.objects.get(&1).unwrap();
        assert_eq!(t.veterancy_level(), VeterancyLevel::Regular);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[1].old, VeterancyLevel::Elite);
        assert_eq!(changes[1].new, VeterancyLevel::Regular);
    }
}
