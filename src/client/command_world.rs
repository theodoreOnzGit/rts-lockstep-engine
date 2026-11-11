// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (canSelectedObjectsDoAction's per-object switch)
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/CommandXlat.cpp (canObjectForceAttack; the command set walk of
//     the capture and hack branches)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The World's answers to the command translator's per-object questions (C2b, GitHub #6): the action manager's checks for one
//! selected object, `canObjectForceAttack`, and a command set's special power buttons. A client host backed by a World calls
//! these from its [`crate::client::command_xlat::CommandXlatHost`] methods.
//!
//! An error from the logic's translation (`WeaponError`) answers "not possible" and is recorded in `cuts.team_live_errors`,
//! where the tests look for it.

use crate::client::command_xlat::{ActionType, GuiButton, TargetMode};
use crate::common::base_type::Coord3D;
use crate::common::game_common::{CommandSourceType, ATTACK_NEW_TARGET, ATTACK_NEW_TARGET_FORCED};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::kind_of::{KINDOF_IMMOBILE, KINDOF_SPAWNS_ARE_THE_WEAPONS};
use crate::common::special_power_type::SpecialPowerType;
use crate::logic::action_manager as am;
use crate::logic::action_manager::{ActionHost, CanEnterType};
use crate::logic::ai::ai_update_types::{
    CanAttackResultValue, ATTACKRESULT_NOT_POSSIBLE, ATTACKRESULT_POSSIBLE, ATTACKRESULT_POSSIBLE_AFTER_MOVING,
};
use crate::logic::world::game_logic::World;
use crate::logic::world::object_command_button::gui;
use crate::logic::world::object_weapons::{get_able_to_attack_specific_object, get_able_to_use_weapon_against_target};

const FROM_PLAYER: CommandSourceType = CommandSourceType::FromPlayer;

fn ok_or_record<T: Default, E: std::fmt::Debug>(w: &mut World, what: &str, r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| {
        w.cuts.team_live_errors.push(format!("{what}: {e:?}"));
        T::default()
    })
}

/// `canSelectedObjectsDoAction`'s switch for one selected object `src` (`other->getObject()`): the action manager's check,
/// with the heal-only container refused for `GET_HEALED_AT` and another repairer refused for `REPAIR_OBJECT`. `None`,
/// `AttackObject` and `SetRallyPoint` are answered by the caller and are false here.
pub fn object_can_do_action(w: &mut World, src: Option<ObjectId>, action: ActionType, target: Option<ObjectId>, additional: bool) -> bool {
    use ActionType as A;
    match action {
        A::GetRepairedAt => {
            let r = am::can_get_repaired_at(w, src, target, FROM_PLAYER);
            ok_or_record(w, "canGetRepairedAt", r)
        }
        // (`canDockAt` dereferences both; a missing one cannot dock)
        A::DockAt => match (src, target) {
            (Some(s), Some(t)) => am::can_dock_at(w, s, t, FROM_PLAYER),
            _ => false,
        },
        A::GetHealedAt => {
            // "This container is only used for the purposes of healing and we cannot enter it normally -- this is NOT a transport!"
            am::can_get_healed_at(w, src, target, FROM_PLAYER) && !target.is_some_and(|t| w.contain_is_heal_contain(t))
        }
        A::RepairObject => {
            let current = target.map_or(INVALID_ID, |t| w.get_sole_healing_benefactor(t));
            let r = am::can_repair_object(w, src, target, FROM_PLAYER);
            // "unless someone else is already healing it"
            ok_or_record(w, "canRepairObject", r) && (current == INVALID_ID || Some(current) == src)
        }
        A::ResumeConstruction => am::can_resume_construction_of(w, src, target, FROM_PLAYER),
        A::CombatdropInto => {
            let r = am::can_enter_object(w, src, target, FROM_PLAYER, CanEnterType::CombatDropInto);
            ok_or_record(w, "canEnterObject", r)
        }
        A::EnterObject => {
            // `additional`: check the transport is not full first
            let mode = if additional { CanEnterType::CheckCapacity } else { CanEnterType::DontCheckCapacity };
            let r = am::can_enter_object(w, src, target, FROM_PLAYER, mode);
            ok_or_record(w, "canEnterObject", r)
        }
        A::HijackVehicle => am::can_hijack_vehicle(w, src, target, FROM_PLAYER),
        A::SabotageBuilding => am::can_sabotage_building(w, src, target, FROM_PLAYER),
        A::ConvertObjectToCarbomb => am::can_convert_object_to_car_bomb(w, src, target, FROM_PLAYER),
        A::CaptureBuilding => {
            let r = am::can_capture_building(w, src, target, FROM_PLAYER);
            ok_or_record(w, "canCaptureBuilding", r)
        }
        A::DisableVehicleViaHacking => {
            // (`checkSourceRequirements` takes its default, true)
            let r = am::can_disable_vehicle_via_hacking(w, src, target, FROM_PLAYER, true);
            ok_or_record(w, "canDisableVehicleViaHacking", r)
        }
        A::StealCashViaHacking => {
            let r = am::can_steal_cash_via_hacking(w, src, target, FROM_PLAYER);
            ok_or_record(w, "canStealCashViaHacking", r)
        }
        A::DisableBuildingViaHacking => {
            let r = am::can_disable_building_via_hacking(w, src, target, FROM_PLAYER);
            ok_or_record(w, "canDisableBuildingViaHacking", r)
        }
        A::MakeDefector => am::can_make_object_defector(w, src, target, FROM_PLAYER),
        A::None | A::AttackObject | A::SetRallyPoint => false,
    }
}

/// `TheActionManager->getCanAttackObject(src, target, CMD_FROM_PLAYER, forced ? ATTACK_NEW_TARGET_FORCED : ATTACK_NEW_TARGET)`.
pub fn object_can_attack(w: &mut World, src: Option<ObjectId>, target: Option<ObjectId>, forced: bool) -> CanAttackResultValue {
    let t = if forced { ATTACK_NEW_TARGET_FORCED } else { ATTACK_NEW_TARGET };
    let r = am::get_can_attack_object(w, src, target, FROM_PLAYER, t).map(|r| r as CanAttackResultValue);
    ok_or_record(w, "getCanAttackObject", r)
}

fn able_to_attack(w: &mut World, o: ObjectId, victim: ObjectId) -> CanAttackResultValue {
    let r = get_able_to_attack_specific_object(w, o, ATTACK_NEW_TARGET_FORCED, victim, FROM_PLAYER, -1).map(|r| r as CanAttackResultValue);
    ok_or_record(w, "getAbleToAttackSpecificObject", r)
}

fn able_to_use_weapon_at(w: &mut World, o: ObjectId, pos: Coord3D) -> CanAttackResultValue {
    let r = get_able_to_use_weapon_against_target(w, o, ATTACK_NEW_TARGET, None, Some(pos), FROM_PLAYER, -1).map(|r| r as CanAttackResultValue);
    ok_or_record(w, "getAbleToUseWeaponAgainstTarget", r)
}

/// `canObjectForceAttack(obj, victim, pos)`: the object's own answer, or for one whose spawns are the weapons the closest
/// slave's (or, for a garrison like the Firebase, the closest rider's); against a position, the closest slave or rider of an
/// immobile one.
pub fn object_can_force_attack(w: &mut World, obj: ObjectId, victim: Option<ObjectId>, pos: Option<Coord3D>) -> CanAttackResultValue {
    if !w.is_able_to_attack(obj) {
        return ATTACKRESULT_NOT_POSSIBLE;
    }
    let kind = |w: &World, k| w.find_object(obj).is_some_and(|o| o.is_kind_of(k));
    if let Some(v) = victim {
        let mut r = able_to_attack(w, obj, v);
        if kind(w, KINDOF_SPAWNS_ARE_THE_WEAPONS) {
            let at = w.find_object(v).map(|x| x.get_position()).unwrap_or_default();
            if r != ATTACKRESULT_POSSIBLE && r != ATTACKRESULT_POSSIBLE_AFTER_MOVING {
                if let Some(Some(slave)) = w.spawn_closest_slave(obj, &at) {
                    r = able_to_attack(w, slave, v);
                }
            } else if let Some(rider) = w.contain_closest_rider(obj, &at) {
                // "The wierd case of a garrisoncontainer being a KINDOF_SPAWNS_ARE_THE_WEAPONS... the AmericaBuildingFirebase"
                let rr = able_to_attack(w, rider, v);
                if rr != ATTACKRESULT_NOT_POSSIBLE {
                    return rr;
                }
            }
        }
        return r;
    }
    let Some(p) = pos else { return ATTACKRESULT_NOT_POSSIBLE };
    let mut test = obj;
    if kind(w, KINDOF_IMMOBILE) || kind(w, KINDOF_SPAWNS_ARE_THE_WEAPONS) {
        match w.spawn_closest_slave(obj, &p) {
            Some(slave) => {
                if let Some(s) = slave {
                    test = s;
                }
            }
            None => {
                if able_to_use_weapon_at(w, obj, p) != ATTACKRESULT_POSSIBLE {
                    if let Some(rider) = w.contain_closest_rider(obj, &p) {
                        test = rider;
                    }
                }
            }
        }
    }
    able_to_use_weapon_at(w, test, p)
}

/// `TheActionManager->canOverrideSpecialPowerDestination(src, loc, spType, CMD_FROM_PLAYER)` (a missing object cannot).
pub fn object_can_override_special_power_destination(w: &mut World, src: Option<ObjectId>, loc: &Coord3D, sp: SpecialPowerType) -> bool {
    let Some(s) = src else { return false };
    let r = am::can_override_special_power_destination(w, s, loc, sp, FROM_PLAYER);
    ok_or_record(w, "canOverrideSpecialPowerDestination", r)
}

/// The World's button `index` (`cuts.command_buttons`) as the client reads it, with its special power's type and its
/// sciences (from the command button store, by name).
pub fn gui_button(w: &World, index: usize) -> Option<GuiButton> {
    let b = w.cuts.command_buttons.get(index)?;
    let special_power = b.special_power.and_then(|id| w.statics.special_power_store.find_special_power_template_by_id(id).map(|t| (id, t.special_power_type())));
    let found = w.statics.command_buttons.find_command_button(&b.name);
    let sciences = found.map(|c| c.science.iter().map(|s| s.0).collect()).unwrap_or_default();
    // (#122) the client's fields of the button: its cursors and radius cursor
    let (cursor_name, invalid_cursor_name, radius_cursor) = found.map(|c| (c.images[1].clone(), c.images[2].clone(), c.radius_cursor)).unwrap_or_default();
    Some(GuiButton {
        index,
        command_type: b.command_type,
        options: b.options,
        weapon_slot: b.weapon_slot,
        max_shots_to_fire: b.max_shots_to_fire,
        special_power,
        thing_template: b.thing_template,
        upgrade: b.upgrade,
        sciences,
        cursor_name,
        invalid_cursor_name,
        radius_cursor,
    })
}

/// `TheActionManager->canDoSpecialPower(AtObject|AtLocation)(src, ..., CMD_FROM_PLAYER, template, options)` (the source's
/// requirements checked, the default), by `mode`; a missing source or template cannot.
pub fn object_can_do_special_power(w: &mut World, src: Option<ObjectId>, b: &GuiButton, mode: TargetMode) -> bool {
    let (Some(s), Some((id, sp_type))) = (src, b.special_power) else { return false };
    let sp = am::SpecialPowerTemplateRef { id, sp_type };
    let r = match mode {
        TargetMode::None => Ok(am::can_do_special_power(w, s, &sp, FROM_PLAYER, b.options, true)),
        TargetMode::Object(t) => am::can_do_special_power_at_object(w, s, t, FROM_PLAYER, &sp, b.options, true),
        TargetMode::Location(p, in_way) => am::can_do_special_power_at_location(w, s, &p, FROM_PLAYER, &sp, in_way, b.options, true),
    };
    ok_or_record(w, "canDoSpecialPower", r)
}

/// `TheActionManager->canFireWeapon(AtObject|AtLocation)(src, ..., CMD_FROM_PLAYER, slot)`, by `mode`.
pub fn object_can_fire_weapon(w: &mut World, src: Option<ObjectId>, b: &GuiButton, mode: TargetMode) -> bool {
    let r = match mode {
        TargetMode::None => Ok(am::can_fire_weapon(w, src, b.weapon_slot, FROM_PLAYER)),
        TargetMode::Object(t) => am::can_fire_weapon_at_object(w, src, Some(t), FROM_PLAYER, b.weapon_slot),
        TargetMode::Location(p, in_way) => Ok(am::can_fire_weapon_at_location(w, src, Some(&p), FROM_PLAYER, b.weapon_slot, in_way)),
    };
    ok_or_record(w, "canFireWeapon", r)
}

/// `TheControlBar->findCommandSet(obj->getCommandSetString())`'s buttons that are `GUI_COMMAND_SPECIAL_POWER` with a special
/// power template, in slot order.
pub fn special_power_buttons(w: &World, obj: ObjectId) -> Vec<GuiButton> {
    let name = w.object_get_command_set_string(obj);
    let Some(set) = w.control_bar_find_command_set(&name) else { return Vec::new() };
    (0..crate::logic::command_button::MAX_COMMANDS_PER_SET)
        .filter_map(|i| w.command_set_get_command_button(set, i))
        .filter_map(|b| gui_button(w, b))
        .filter(|b| b.command_type == gui::SPECIAL_POWER && b.special_power.is_some())
        .collect()
}
