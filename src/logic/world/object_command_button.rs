// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp
//   (`doCommandButton`, `doCommandButtonAtObject`, `doCommandButtonAtPosition`,
//   `doCommandButtonUsingWaypoints`)
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/SpecialPower.cpp
//   (`SpecialPowerStore::canUseSpecialPower`)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp
//   (`CommandButton::isValidObjectTarget(Object*, Object*)`, `isValidRelationshipTarget`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Integration batch I23: an object carrying out a command button (a script's or a module's: `CommandButtonHuntUpdate`, the group's
//! `groupDoCommandButton*`), and the special power store's `canUseSpecialPower`.
//!
//! A button is its index in the world's table (`cuts.command_buttons`, which `World::install_command_buttons` fills from the INI). The
//! `DEBUG_CRASH` warnings of the unhandled cases do nothing in a release build, so those cases do nothing here.

use crate::common::base_type::Coord3D;
use crate::common::game_common::Relationship;
use crate::common::game_type::{ObjectId, INVALID_ANGLE};
use crate::common::name_key_generator::NameKeyType;
use crate::common::science::SCIENCE_INVALID;
use crate::common::upgrade::UpgradeType;
use crate::logic::ai::ai_commands::{AiCommandParms, AICMD_COMBATDROP};
use crate::logic::ai::ai_update_types::CMD_FROM_SCRIPT;
use crate::logic::weapon_set_state::{LOCKED_PERMANENTLY, LOCKED_TEMPORARILY};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::object_cuts::CommandButtonFacts;
use crate::logic::world::object_modules::SpecialPowerTarget;

/// `GUICommandType` values (the index in `GUI_COMMAND_NAMES`).
pub mod gui {
    pub const DOZER_CONSTRUCT: i32 = 1;
    pub const UNIT_BUILD: i32 = 3;
    pub const PLAYER_UPGRADE: i32 = 5;
    pub const OBJECT_UPGRADE: i32 = 6;
    pub const ATTACK_MOVE: i32 = 8;
    pub const STOP: i32 = 12;
    pub const SELL: i32 = 19;
    pub const FIRE_WEAPON: i32 = 20;
    pub const SPECIAL_POWER: i32 = 21;
    pub const HACK_INTERNET: i32 = 23;
    pub const COMBATDROP: i32 = 25;
    pub const SWITCH_WEAPON: i32 = 26;
    pub const HIJACK_VEHICLE: i32 = 27;
    pub const CONVERT_TO_CARBOMB: i32 = 28;
    pub const SABOTAGE_BUILDING: i32 = 29;
}

/// `CommandOption` bits.
pub mod opt {
    pub const NEED_TARGET_ENEMY_OBJECT: u32 = 0x0000_0001;
    pub const NEED_TARGET_NEUTRAL_OBJECT: u32 = 0x0000_0002;
    pub const NEED_TARGET_ALLY_OBJECT: u32 = 0x0000_0004;
    pub const NEED_TARGET_POS: u32 = 0x0000_0020;
    pub const ATTACK_OBJECTS_POSITION: u32 = 0x0000_1000;
    pub const COMMAND_FIRED_BY_SCRIPT: u32 = 0x0004_0000;
    pub const CAN_USE_WAYPOINTS: u32 = 0x0040_0000;
    /// `COMMAND_OPTION_NEED_OBJECT_TARGET`.
    pub const NEED_OBJECT_TARGET: u32 = NEED_TARGET_ENEMY_OBJECT | NEED_TARGET_NEUTRAL_OBJECT | NEED_TARGET_ALLY_OBJECT;
}

/// `CommandButton::isValidRelationshipTarget(r)`.
pub fn is_valid_relationship_target(options: u32, r: Relationship) -> bool {
    let mask = match r {
        Relationship::Enemies => opt::NEED_TARGET_ENEMY_OBJECT,
        Relationship::Allies => opt::NEED_TARGET_ALLY_OBJECT,
        Relationship::Neutral => opt::NEED_TARGET_NEUTRAL_OBJECT,
    };
    options & mask != 0
}

impl World {
    /// `TheSpecialPowerStore->canUseSpecialPower(obj, template)`: the object has a module for the power, and its controlling player the
    /// power's required science, if any. Without `cuts.live_special_powers` the cut table (`special_power_denied`).
    pub fn can_use_special_power(&mut self, obj: ObjectId, template: Option<u32>) -> bool {
        if !self.cuts.live_special_powers {
            return self.cuts.special_power_can_use(obj, template);
        }
        let Some(t) = template else { return false };
        if self.objects.find(obj).is_none() {
            return false;
        }
        if self.get_special_power_module(obj, Some(t)).is_none() {
            return false;
        }
        let statics = self.statics.clone();
        let Some(power) = statics.special_power_store.find_special_power_template_by_id(t) else { return false };
        let science = power.required_science();
        if science != SCIENCE_INVALID {
            // `player->hasScience(..)` through the controlling player, which upstream dereferences
            let Some(p) = self.get_controlling_player(obj) else { return false };
            if !crate::logic::world::player_ops::player_has_science(self, p, science).unwrap_or(false) {
                return false;
            }
        }
        true
    }

    fn button(&self, b: usize) -> Option<CommandButtonFacts> {
        self.cuts.command_buttons.get(b).cloned()
    }

    fn ai_order(&mut self, o: ObjectId, call: ModuleCall) {
        let _ = self.ai_ask(o, call);
    }

    /// The button's special power, fired by a script: its options with `COMMAND_FIRED_BY_SCRIPT`, forced when the source is a script.
    fn button_special_power(&mut self, o: ObjectId, b: &CommandButtonFacts, target: SpecialPowerTarget, src: i32) -> bool {
        let Some(sp) = b.special_power else { return false };
        self.do_special_power(o, Some(sp), target, b.options | opt::COMMAND_FIRED_BY_SCRIPT, src == CMD_FROM_SCRIPT);
        true
    }

    /// `doCommandButton(commandButton, cmdSource)`.
    pub fn do_command_button(&mut self, o: ObjectId, button: usize, src: i32) {
        let Some(obj) = self.objects.find(o) else { return };
        if obj.is_disabled() {
            return;
        }
        let has_ai = obj.ai.is_some();
        let Some(b) = self.button(button) else { return };
        match b.command_type {
            gui::SPECIAL_POWER => {
                self.button_special_power(o, &b, SpecialPowerTarget::None, src);
            }
            gui::STOP if has_ai => self.ai_order(o, ModuleCall::AiIdleFrom { command_source: src }),
            gui::SWITCH_WEAPON => {
                // "switches until un-switched, or switched to something else"
                self.set_weapon_lock(o, b.weapon_slot, LOCKED_PERMANENTLY);
            }
            gui::FIRE_WEAPON if has_ai => {
                if b.options & opt::NEED_OBJECT_TARGET == 0 && b.options & opt::NEED_TARGET_POS == 0 {
                    self.set_weapon_lock(o, b.weapon_slot, LOCKED_TEMPORARILY);
                    // location-based fire weapon
                    self.ai_order(o, ModuleCall::AiAttackPosition { pos: None, max_shots: b.max_shots_to_fire, command_source: src });
                }
                // (else a DEBUG_CRASH: no position to fire at)
            }
            gui::OBJECT_UPGRADE | gui::PLAYER_UPGRADE => {
                let statics = self.statics.clone();
                let Some(u) = b.upgrade.and_then(|k| statics.upgrade_center.find_upgrade_by_key(NameKeyType(k))) else { return };
                if u.upgrade_type() == UpgradeType::Object {
                    let mask = u.upgrade_mask();
                    if self.has_upgrade(o, &mask) || !self.affected_by_upgrade(o, &mask) {
                        return;
                    }
                }
                // the producer must have a production update; queue the upgrade "research"
                if let Some(pu) = self.get_production_update_interface(o) {
                    self.module_call(o, pu, ModuleCall::ProductionQueueUpgrade { upgrade: b.upgrade });
                }
            }
            gui::UNIT_BUILD | gui::DOZER_CONSTRUCT => {
                if let (Some(pu), Some(tt)) = (self.get_production_update_interface(o), b.thing_template) {
                    let id = self.module_call(o, pu, ModuleCall::ProductionRequestUniqueUnitId).as_int(0);
                    self.module_call(o, pu, ModuleCall::ProductionQueueCreateUnit { template: Some(tt), production_id: id });
                }
            }
            gui::HACK_INTERNET if has_ai => self.ai_order(o, ModuleCall::AiHackInternet { command_source: src }),
            gui::SELL => self.build_assistant_sell(o),
            // "Feel free to implement object based command buttons." (a DEBUG_CRASH otherwise)
            _ => {}
        }
    }

    /// `doCommandButtonAtObject(commandButton, obj, cmdSource)` (`None`: a `NULL` object).
    pub fn do_command_button_at_object(&mut self, o: ObjectId, button: usize, target: Option<ObjectId>, src: i32) {
        let Some(obj) = self.objects.find(o) else { return };
        if obj.is_disabled() {
            return;
        }
        let has_ai = obj.ai.is_some();
        let Some(b) = self.button(button) else { return };
        match b.command_type {
            gui::COMBATDROP if has_ai => {
                // `aiCombatDrop(obj, *obj->getPosition(), cmdSource)`: the target is dereferenced upstream
                let Some(t) = target else { return self.cuts.team_live_errors.push("Object::doCommandButtonAtObject: COMBATDROP at NULL".into()) };
                let pos = self.objects.find(t).map(|x| x.get_position()).unwrap_or_default();
                let mut parms = AiCommandParms::new(AICMD_COMBATDROP, src);
                parms.obj = t;
                parms.pos = pos;
                self.ai_order(o, ModuleCall::AiDoCommandParms(parms));
            }
            gui::SPECIAL_POWER => {
                // (a `NULL` object is passed on as is: the modules take `INVALID_ID` as no object)
                let t = target.unwrap_or(crate::common::game_type::INVALID_ID);
                self.button_special_power(o, &b, SpecialPowerTarget::Object(t), src);
            }
            gui::STOP if has_ai => self.ai_order(o, ModuleCall::AiIdleFrom { command_source: src }),
            gui::FIRE_WEAPON if has_ai => {
                if b.options & opt::NEED_OBJECT_TARGET != 0 {
                    let Some(t) = target else { return };
                    if self.objects.find(t).is_none() || !is_valid_relationship_target(b.options, self.get_relationship(o, t)) {
                        return;
                    }
                    self.set_weapon_lock(o, b.weapon_slot, LOCKED_TEMPORARILY);
                    if b.options & opt::ATTACK_OBJECTS_POSITION != 0 {
                        let pos = self.objects.find(t).map(|x| x.get_position());
                        self.ai_order(o, ModuleCall::AiAttackPosition { pos, max_shots: b.max_shots_to_fire, command_source: src });
                    } else {
                        self.ai_order(o, ModuleCall::AiAttackObject { victim: t, max_shots: b.max_shots_to_fire, command_source: src });
                    }
                }
                // (else a DEBUG_CRASH: the weapon cannot fire at an object)
            }
            gui::HIJACK_VEHICLE | gui::CONVERT_TO_CARBOMB | gui::SABOTAGE_BUILDING if has_ai => {
                if let Some(t) = target {
                    self.ai_order(o, ModuleCall::AiEnter { target: t, command_source: src });
                }
            }
            _ => {}
        }
    }

    /// `doCommandButtonAtPosition(commandButton, pos, cmdSource)`.
    pub fn do_command_button_at_position(&mut self, o: ObjectId, button: usize, pos: &Coord3D, src: i32) {
        let Some(obj) = self.objects.find(o) else { return };
        if obj.is_disabled() {
            return;
        }
        let has_ai = obj.ai.is_some();
        let Some(b) = self.button(button) else { return };
        match b.command_type {
            gui::SPECIAL_POWER => {
                self.button_special_power(o, &b, SpecialPowerTarget::Location { loc: *pos, angle: INVALID_ANGLE }, src);
            }
            gui::ATTACK_MOVE if has_ai => {
                self.ai_order(o, ModuleCall::AiAttackMoveToPosition { pos: *pos, max_shots: b.max_shots_to_fire, command_source: src });
            }
            gui::STOP if has_ai => self.ai_order(o, ModuleCall::AiIdleFrom { command_source: src }),
            gui::DOZER_CONSTRUCT => {
                // `TheBuildAssistant->buildObjectNow(this, getThingTemplate(), pos, 0.0f, getControllingPlayer())`
                // (`buildObjectNow` returns `NULL` at once for a `NULL` template or owner)
                let Some(what) = b.thing_template.map(crate::logic::world::team_ops::TemplateRef::from) else { return };
                let Some(owner) = self.get_controlling_player(o) else { return };
                if let Err(e) = crate::logic::world::build_assistant::build_assistant_build_object_now(self, Some(o), what, *pos, 0.0, owner) {
                    self.cuts.team_live_errors.push(format!("BuildAssistant::buildObjectNow: {e:?}"));
                }
            }
            // (without `NEED_TARGET_POS` a DEBUG_CRASH: the weapon cannot fire at a position)
            gui::FIRE_WEAPON if has_ai && b.options & opt::NEED_TARGET_POS != 0 => {
                self.set_weapon_lock(o, b.weapon_slot, LOCKED_TEMPORARILY);
                self.ai_order(o, ModuleCall::AiAttackPosition { pos: Some(*pos), max_shots: b.max_shots_to_fire, command_source: src });
            }
            _ => {}
        }
    }

    /// `doCommandButtonUsingWaypoints(commandButton, way, cmdSource)`, the waypoint by id: only a button with `CAN_USE_WAYPOINTS`, and
    /// of those only a special power.
    pub fn do_command_button_using_waypoints(&mut self, o: ObjectId, button: usize, waypoint: u32, src: i32) {
        let Some(obj) = self.objects.find(o) else { return };
        if obj.is_disabled() {
            return;
        }
        let Some(b) = self.button(button) else { return };
        if b.options & opt::CAN_USE_WAYPOINTS == 0 {
            return;
        }
        if b.command_type == gui::SPECIAL_POWER {
            self.button_special_power(o, &b, SpecialPowerTarget::Waypoint(waypoint), src);
        }
    }
}
