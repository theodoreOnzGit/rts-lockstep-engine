// SPDX-License-Identifier: GPL-3.0-only
//
// Part of the translation of GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590); see
// `ai_attack_states`. Licensed under the GNU General Public License version 3 (GPL-3.0-only for this translation), with the
// additional terms in NOTICE.md. Translated to Rust and modified; not the original program.
//! [`FireHost`] for every [`AiStateHost`] (`p1w13-sonnet`, layer 10, batch A10a): the attack machine's fire state and range
//! condition asked of the AI's owner, its AI (lent: the innermost of [`AiStateHost::ai_stack`]), its weapons and the world.
//!
//! This module is the only one where both traits are in scope (their method names overlap: `position`, `is_kind_of`, ...), so
//! every call here names its trait.

use crate::common::base_type::Coord3D;
use crate::common::game_common::CommandSourceType;
use crate::common::game_type::{ObjectId, WeaponSlotType, INVALID_ID};
use crate::common::kind_of::KindOfType;
use crate::common::object_status_types::ObjectStatusTypes;
use crate::common::state_machine::StateContext;
use crate::logic::ai::ai_attack_states::frame;
use crate::logic::ai::ai_states::{with_ai, AiStateHost};
use crate::logic::ai::ai_update::AiHost;
use crate::logic::ai::fire_state::FireHost;
use crate::logic::locomotor::LocomotorHost;
use crate::logic::weapon_types::WeaponStatus;
use crate::logic::world::ids::PlayerIndex;

/// The AI's command source (`CommandSourceType` as an `int`) as the fire state's enum.
fn command_source(src: i32) -> CommandSourceType {
    match src {
        0 => CommandSourceType::FromPlayer,
        1 => CommandSourceType::FromScript,
        2 => CommandSourceType::FromAi,
        3 => CommandSourceType::FromDozer,
        4 => CommandSourceType::DefaultSwitchWeapon,
        _ => unreachable!("a command source"),
    }
}

impl<H: AiStateHost> FireHost for H {
    fn owner(&mut self) -> ObjectId {
        AiHost::object_id(self)
    }
    /// The AI's mood matrix, with the AI's machine as the attack frame saw it.
    fn ai_mood_action_adjustment(&mut self, _obj: ObjectId, action: i32) -> u32 {
        let outer = frame(self).outer.clone();
        with_ai(self, |a, h| a.get_mood_matrix_action_adjustment(h, &outer, action))
    }
    fn ai_are_turrets_linked(&mut self, _obj: ObjectId) -> bool {
        with_ai(self, |a, _| a.data.turrets_linked)
    }
    fn ai_current_victim(&mut self, _obj: ObjectId) -> Option<ObjectId> {
        let v = with_ai(self, |a, _| a.current_victim_id);
        (v != INVALID_ID).then_some(v)
    }
    fn ai_last_command_source(&mut self, _obj: ObjectId) -> Option<CommandSourceType> {
        Some(command_source(with_ai(self, |a, _| a.last_command_source)))
    }
    fn ai_is_doing_ground_movement(&mut self, _obj: ObjectId) -> Option<bool> {
        Some(with_ai(self, |a, h| a.is_doing_ground_movement(h)))
    }
    fn is_kind_of(&mut self, obj: ObjectId, kind: KindOfType) -> bool {
        if obj == AiHost::object_id(self) { LocomotorHost::is_kind_of(self, kind) } else { AiStateHost::object_is_kind_of(self, obj, kind) }
    }
    fn test_status(&mut self, obj: ObjectId, bit: ObjectStatusTypes) -> bool {
        if obj == AiHost::object_id(self) { LocomotorHost::status_test(self, bit) } else { AiStateHost::object_test_status(self, obj, bit) }
    }
    /// (the fire state sets the owner's statuses only)
    fn set_status(&mut self, _obj: ObjectId, bits: &[ObjectStatusTypes], set: bool) {
        AiStateHost::set_status_bits(self, bits, set)
    }
    fn is_effectively_dead(&mut self, obj: ObjectId) -> bool {
        if obj == AiHost::object_id(self) { AiHost::is_effectively_dead(self) } else { AiStateHost::object_is_effectively_dead(self, obj) }
    }
    fn is_destroyed(&mut self, obj: ObjectId) -> bool {
        AiStateHost::object_is_destroyed(self, obj)
    }
    fn contained_by(&mut self, obj: ObjectId) -> Option<ObjectId> {
        let c = if obj == AiHost::object_id(self) { AiHost::contained_by(self) } else { AiStateHost::object_contained_by(self, obj) };
        (c != INVALID_ID).then_some(c)
    }
    fn is_airborne_target(&mut self, obj: ObjectId) -> bool {
        AiStateHost::object_is_airborne_target(self, obj)
    }
    fn is_significantly_above_terrain(&mut self, obj: ObjectId) -> bool {
        if obj == AiHost::object_id(self) {
            AiStateHost::is_significantly_above_terrain(self)
        } else {
            AiStateHost::object_is_significantly_above_terrain(self, obj)
        }
    }
    fn position(&mut self, obj: ObjectId) -> Coord3D {
        if obj == AiHost::object_id(self) {
            LocomotorHost::position(self)
        } else {
            StateContext::find_object(self, obj).expect("the object").position
        }
    }
    /// (the scene's one player, or none: every object's)
    fn controlling_player(&mut self, _obj: ObjectId) -> Option<PlayerIndex> {
        AiHost::controlling_player_type(self).map(|_| PlayerIndex::default())
    }
    fn team_attacks_common_target(&mut self, _obj: ObjectId) -> bool {
        AiHost::team_attack_common_target(self)
    }
    fn team_target_object(&mut self, _obj: ObjectId) -> Option<ObjectId> {
        let t = AiHost::team_target_object(self);
        (t != INVALID_ID).then_some(t)
    }
    fn set_team_target_object(&mut self, _obj: ObjectId, target: ObjectId) {
        AiStateHost::set_team_target_object(self, target)
    }
    fn current_weapon(&mut self, _obj: ObjectId) -> Option<WeaponSlotType> {
        AiStateHost::current_weapon_slot(self)
    }
    fn has_weapon_in_slot(&mut self, _obj: ObjectId, slot: WeaponSlotType) -> bool {
        AiStateHost::weapon_slot_present(self, slot)
    }
    fn weapon_status(&mut self, _obj: ObjectId, slot: WeaponSlotType) -> WeaponStatus {
        AiStateHost::weapon_status_now(self, slot)
    }
    fn weapon_is_contact(&mut self, _obj: ObjectId, _slot: WeaponSlotType) -> bool {
        AiStateHost::weapon_is_contact(self)
    }
    fn weapon_has_leech_range(&mut self, _obj: ObjectId, slot: WeaponSlotType) -> bool {
        AiStateHost::weapon_has_leech_range_now(self, slot)
    }
    fn weapon_within_range_of_object(&mut self, _obj: ObjectId, _slot: WeaponSlotType, victim: ObjectId) -> bool {
        AiStateHost::weapon_is_within_attack_range_of_object(self, victim)
    }
    fn weapon_within_range_of_position(&mut self, _obj: ObjectId, _slot: WeaponSlotType, pos: &Coord3D) -> bool {
        AiStateHost::weapon_is_within_attack_range_of_position(self, pos)
    }
    fn weapon_continue_attack_range(&mut self, _obj: ObjectId, _slot: WeaponSlotType) -> f32 {
        AiStateHost::weapon_continue_attack_range(self)
    }
    /// (asked by the turret's machine only)
    fn weapon_attack_range(&mut self, _obj: ObjectId, _slot: WeaponSlotType) -> f32 {
        unreachable!("the attack machine does not ask a weapon's attack range")
    }
    fn weapon_set_pre_attack_finished_frame(&mut self, _obj: ObjectId, slot: WeaponSlotType, frame: u32) {
        AiStateHost::weapon_set_pre_attack_finished(self, slot, frame)
    }
    fn weapon_fire_at_position(&mut self, _obj: ObjectId, slot: WeaponSlotType, pos: &Coord3D) -> bool {
        AiStateHost::weapon_fire_at(self, slot, pos)
    }
    fn release_weapon_lock(&mut self, _obj: ObjectId, lock: i32) {
        AiStateHost::release_weapon_lock(self, lock)
    }
    fn notify_firing_tracker_shot_fired(&mut self, _obj: ObjectId, slot: WeaponSlotType, victim: ObjectId) {
        AiStateHost::notify_firing_tracker_shot_fired(self, slot, victim)
    }
    fn pre_fire_current_weapon(&mut self, _obj: ObjectId, victim: Option<ObjectId>) {
        AiStateHost::pre_fire_current_weapon(self, victim)
    }
    fn set_firing_condition_for_current_weapon(&mut self, _obj: ObjectId) {
        AiStateHost::set_firing_condition_for_current_weapon(self)
    }
    fn fire_current_weapon_at_object(&mut self, _obj: ObjectId, victim: Option<ObjectId>) {
        AiStateHost::fire_current_weapon_at_object(self, victim)
    }
    fn fire_current_weapon_at_position(&mut self, _obj: ObjectId, pos: &Coord3D) {
        AiStateHost::fire_current_weapon_at_position(self, pos)
    }
    fn is_attack_view_blocked(&mut self, _obj: ObjectId, _obj_pos: &Coord3D, victim: ObjectId, victim_pos: &Coord3D) -> bool {
        AiStateHost::pf_is_attack_view_blocked(self, victim, victim_pos)
    }
    /// (the scene's closest attackable object: the stand-in's search reads its first filter only)
    fn closest_attackable_of_player(&mut self, pos: &Coord3D, range: f32, _obj: ObjectId, source: CommandSourceType, _player: Option<PlayerIndex>) -> Option<ObjectId> {
        let c = AiStateHost::partition_closest_attack_target(self, pos, range, source as i32);
        (c != INVALID_ID).then_some(c)
    }
}
