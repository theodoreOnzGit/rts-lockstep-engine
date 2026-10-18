// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp
//   (`Object::Object`, `initObject`, `~Object`, `onDestroy`,
//   `friend_bindToDrawable`)
//   GeneralsMD/Code/GameEngine/Source/Common/Thing/ThingFactory.cpp
//   (`newObject` after the template is chosen)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp
//   (`friend_createObject`, `sendObjectCreated`, `sendObjectDestroyed`,
//   `bindObjectAndDrawable`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Making and unmaking an `Object`: [`World::new_object`] is
//! `ThingFactory::newObject` after the template side (build variation) —
//! `GameLogic::friend_createObject` (the `Object` constructor), every create
//! module's `onCreate()`, the partition manager, `initObject()` — and
//! [`World::delete_instance`] is `~Object`.
//!
//! # Differences from upstream
//!
//! - **The modules arrive made.** The constructor asks `TheModuleFactory` for
//!   each module of the template, in order; here the caller makes them (with
//!   the `ModuleFactory`, which fails for a class that is not translated
//!   yet) and hands them in [`NewObject::modules`]. They are therefore built
//!   before the object's team is set rather than after; module constructors
//!   cannot observe the difference through the id they are given.
//! - **The helpers are made by a cut point.** `Object::Object` makes up to
//!   eight sleepy helper modules (`ObjectSMCHelper`, ...) before the
//!   template's. Which ones is decided here, exactly as upstream; making
//!   them is `cuts.make_helper`, which until layer 9 makes a stand-in with no
//!   behaviour and no update interface.
//! - **An object under construction is findable by id from the start**, as
//!   upstream (`setID` adds it to the lookup table), but is not in the object
//!   list until the end of the constructor (`registerObject`).
//! - **During `~Object` the object is findable by id.** Upstream's table
//!   entry is already `NULL` when the destructor runs; the Rust destructor
//!   runs the same steps through the id-based operations, so it puts the
//!   object back (out of the list) for their duration.

use std::sync::Arc;

use crate::common::game_common::PlayerMaskType;
use crate::common::game_type::{DrawableId, ObjectId, INVALID_DRAWABLE_ID, INVALID_ID};
use crate::common::kind_of::{
    KINDOF_BOOBY_TRAP, KINDOF_CAN_BE_REPULSED, KINDOF_DEMOTRAP, KINDOF_INERT, KINDOF_MINE, KINDOF_PROJECTILE, KINDOF_SHRUBBERY,
};
use crate::common::model_state::{MODELCONDITION_INVALID, MODELCONDITION_NIGHT, MODELCONDITION_SNOW};
use crate::common::module::{
    Module, ModuleInterfaceType, MODULEINTERFACE_BODY, MODULEINTERFACE_CONTAIN, MODULEINTERFACE_SPECIAL_POWER, MODULEINTERFACE_UPDATE,
};
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::common::thing_template::ThingTemplate;
use crate::logic::module::update_module::UpdateModule;
use crate::logic::world::game_logic::{World, WorldError};
use crate::logic::world::ids::TeamId;
use crate::logic::world::module_call::{ModuleCall, ModuleReply};
use crate::logic::world::object::{BehaviorSlot, HelperKind, ModuleIndex, ModuleSlot, Object, CONSTRUCTION_COMPLETE};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use crate::logic::world::object_state::WEAPON_SET_TYPE_TO_MODEL_CONDITION;
use crate::logic::world::update_body::UpdateBody;

/// A module of a new object, as `TheModuleFactory->newModule` made it.
#[derive(Clone, Debug)]
pub struct NewModule {
    /// The class name (`getModuleNameKey()`).
    pub name: Vec<u8>,
    /// The module tag (`TheNameKeyGenerator->keyToName(getModuleTagNameKey())`).
    pub tag: Vec<u8>,
    /// Which interfaces answer non-null (`MODULEINTERFACE_*`).
    pub interfaces: ModuleInterfaceType,
    /// The scheduling half, if the module has an update interface. The module
    /// sets its own wake frame on it, as upstream's constructors do.
    pub update: Option<UpdateModule>,
    /// The concrete behaviour.
    pub body: Option<UpdateBody>,
    /// The base-class state the factory made.
    pub module: Option<Module>,
    /// `getStealth()` is non-null (`StealthUpdate`).
    pub is_stealth: bool,
    /// `getAIUpdateInterface()` is non-null.
    pub is_ai: bool,
}

impl NewModule {
    /// A module with none of the interfaces and no behaviour.
    pub fn plain(name: &[u8], tag: &[u8]) -> NewModule {
        NewModule {
            name: name.to_vec(),
            tag: tag.to_vec(),
            interfaces: 0,
            update: None,
            body: None,
            module: None,
            is_stealth: false,
            is_ai: false,
        }
    }

    /// The stand-in for a helper no layer has translated: it has a name and a
    /// tag (so `xfer` finds it) and nothing else.
    pub fn helper_stub(kind: HelperKind) -> NewModule {
        let (name, tag): (&[u8], &[u8]) = match kind {
            HelperKind::Smc => (b"ObjectSMCHelper", b"ModuleTag_SMCHelper"),
            HelperKind::StatusDamage => (b"StatusDamageHelper", b"ModuleTag_StatusDamageHelper"),
            HelperKind::SubdualDamage => (b"SubdualDamageHelper", b"ModuleTag_SubdualDamageHelper"),
            HelperKind::Repulsor => (b"ObjectRepulsorHelper", b"ModuleTag_RepulsorHelper"),
            HelperKind::Defection => (b"ObjectDefectionHelper", b"ModuleTag_DefectionHelper"),
            HelperKind::WeaponStatus => (b"ObjectWeaponStatusHelper", b"ModuleTag_WeaponStatusHelper"),
            HelperKind::FiringTracker => (b"FiringTracker", b"ModuleTag_FiringTrackerHelper"),
            HelperKind::TempWeaponBonus => (b"TempWeaponBonusHelper", b"ModuleTag_TempWeaponBonusHelper"),
        };
        NewModule::plain(name, tag)
    }
}

/// The arguments of `newObject` after the template side.
#[derive(Clone, Debug)]
pub struct NewObject {
    /// The template, already the most overridden one (`getFinalOverride()`)
    /// and after the build variation.
    pub template: Arc<ThingTemplate>,
    /// `statusBits`: set before `onCreate`.
    pub status: ObjectStatusMaskType,
    /// The team; `None` is the neutral player's default team.
    pub team: Option<TeamId>,
    /// The template's behaviour modules, in module order, made by the module factory.
    pub modules: Vec<NewModule>,
}

impl World {
    /// `newObject` after the template side: `friend_createObject` (the
    /// `Object` constructor), then every create module's `onCreate()`, the
    /// partition manager's `registerObject`, and `initObject()`. Returns the
    /// new object's id.
    pub fn new_object(&mut self, p: NewObject) -> Result<ObjectId, WorldError> {
        let id = self.construct_object(p)?;
        // run the create function for the thing
        self.call_create_modules(id);
        // all objects are part of the partition manager system, add it to that system now
        self.partition_register_object(id);
        self.init_object(id);
        Ok(id)
    }

    /// `Object::Object(tt, objectStatusMask, team)`.
    fn construct_object(&mut self, p: NewObject) -> Result<ObjectId, WorldError> {
        let tt = p.template.clone();
        // assign unique object id
        let id = self.allocate_object_id();
        let mut obj = Object::with_template(id, tt.clone());
        obj.status = p.status;
        // like upstream's setID: findable by id from now on, in no list yet
        self.objects.insert_unlisted(obj)?;

        // set m_team to null before the first call, to avoid naughtiness...
        // If no team is specified in the constructor, then assign the object to the neutral team.
        let team = p.team.unwrap_or(self.cuts.neutral_default_team);
        self.set_team(id, Some(team));

        // the helpers are done first -- even before Behaviors! -- in case a module needs to call something that uses them.
        let mut is_inactive_body = false; // Inactive bodies can't take special damage since they can't take damage
        let info = tt.get_behavior_module_info();
        for i in 0..info.get_count() {
            let name = info.get_nth_name(i);
            if name.is_empty() {
                continue;
            }
            if name == b"InactiveBody" {
                is_inactive_body = true;
                break;
            }
        }
        let mut helpers = vec![HelperKind::Smc];
        if !is_inactive_body {
            helpers.push(HelperKind::StatusDamage);
            helpers.push(HelperKind::SubdualDamage);
        }
        if self.cuts.globals.enable_repulsors && tt.is_kind_of(KINDOF_CAN_BE_REPULSED) {
            // if we can ever be a temporary-repulsor, make a repulsor helper. (srj)
            helpers.push(HelperKind::Repulsor);
        }
        // shrubbery cannot defect. no, really.
        if !tt.is_kind_of(KINDOF_SHRUBBERY) {
            helpers.push(HelperKind::Defection);
        }
        if tt.can_possibly_have_any_weapon() {
            // we only need a firingtracker and wshelper if we can possibly have a weapon.
            helpers.push(HelperKind::WeaponStatus);
            helpers.push(HelperKind::FiringTracker);
            helpers.push(HelperKind::TempWeaponBonus);
        }
        for kind in helpers {
            let m = self.cuts.make_helper(kind);
            self.add_module(id, m, Some(kind));
        }

        // behaviors are always done first, so they get into the publicModule arrays before anything else.
        for m in p.modules {
            self.add_module(id, m, None);
        }

        if self.objects.find(id).is_some_and(|o| o.ai.is_some()) {
            if self.cuts.team_has_template_info(team) {
                self.cut(CutCall::AiSetAttitude { object: id, team });
            }
            if self.cuts.team_attack_info(team) {
                self.cut(CutCall::AiSetAttackInfo { object: id, team });
            }
        }

        // allocate experience tracker (the record exists from `with_template`).
        // If a valid team has been assigned me, then I have a Player I can ask about my starting level
        if let Some(controller) = self.get_controlling_player(id) {
            let level = self.cuts.player_production_veterancy_level(controller, tt.get_name());
            self.cut_experience_set_veterancy_level(id, level, true);
        }

        // allow for inter-Module resolution
        self.call_all_modules(id, ModuleCall::OnObjectCreated);

        {
            let o = self.objects.find_mut(id).expect("inserted above");
            o.num_trigger_areas_active = 0;
            o.entered_or_exited_frame = 0;
            o.is_selectable = tt.is_kind_of(crate::common::kind_of::KINDOF_SELECTABLE);
            o.health_box_offset.zero();
            // Modules have now been completely created!
            o.modules_ready = true;
        }

        self.radar_add_object(id);

        // register the object with the GameLogic
        self.link_registered_object(id)?;

        // disable occlusion for some time after object is created to allow them to exit the factory/building.
        let safe = self.frame().wrapping_add(tt.get_occlusion_delay());
        let o = self.objects.find_mut(id).expect("inserted above");
        o.safe_occlusion_frame = safe;
        o.sole_healing_benefactor_id = INVALID_ID; // who is the only other object that can give me this non-stacking heal benefit?
        o.sole_healing_benefactor_expiration_frame = 0; // on what frame can I accept healing (thus to switch) from a new benefactor
        Ok(id)
    }

    /// Appends a module to the object's `m_behaviors` and caches what it is
    /// (`m_body`, `m_contain`, `m_stealth`, `m_ai`, `m_physics`, the helpers).
    /// Returns its position.
    fn add_module(&mut self, id: ObjectId, m: NewModule, helper: Option<HelperKind>) -> ModuleIndex {
        let Some(o) = self.objects.find_mut(id) else { return 0 };
        let idx = o.modules.len() as ModuleIndex;
        let has_body = m.body.is_some();
        let mut interfaces = m.interfaces;
        let (update_index, body) = match m.update {
            Some(update) => {
                interfaces |= MODULEINTERFACE_UPDATE;
                o.behaviors.push(BehaviorSlot { update, body: m.body });
                (Some((o.behaviors.len() - 1) as u16), None)
            }
            None => (None, m.body),
        };
        o.modules.push(ModuleSlot { name: m.name.clone(), tag: m.tag, interfaces, update_index, body, module: m.module, has_body, helper });
        if interfaces & MODULEINTERFACE_BODY != 0 {
            o.body = Some(idx); // "Duplicate bodies" is a debug assert
        }
        if interfaces & MODULEINTERFACE_CONTAIN != 0 {
            o.contain = Some(idx);
        }
        if m.is_stealth {
            o.stealth = Some(idx);
        }
        if m.is_ai {
            o.ai = Some(idx);
        }
        if m.name == b"PhysicsBehavior" {
            o.physics = Some(idx);
        }
        if let Some(kind) = helper {
            match kind {
                HelperKind::Smc => o.helpers.smc = Some(idx),
                HelperKind::StatusDamage => o.helpers.status_damage = Some(idx),
                HelperKind::SubdualDamage => o.helpers.subdual_damage = Some(idx),
                HelperKind::Repulsor => o.helpers.repulsor = Some(idx),
                HelperKind::Defection => o.helpers.defection = Some(idx),
                HelperKind::WeaponStatus => o.helpers.weapon_status = Some(idx),
                HelperKind::FiringTracker => o.helpers.firing_tracker = Some(idx),
                HelperKind::TempWeaponBonus => o.helpers.temp_weapon_bonus = Some(idx),
            }
        }
        idx
    }

    /// `ObjectHelper::sleepUntil(when)` for the helper of that kind: nothing
    /// for a destroyed object; the helper wakes at `when` (`NEVER` and
    /// `FOREVER` mean sleep forever: "the setWakeFrame(NEVER) actually
    /// awakens immediately, since NEVER==0"). A helper that is not
    /// translated yet has no update module, and the call is only recorded.
    pub fn helper_sleep_until(&mut self, id: ObjectId, kind: HelperKind, when: u32) {
        let Some(o) = self.objects.find(id) else { return };
        if o.is_destroyed() {
            return;
        }
        let idx = match kind {
            HelperKind::Smc => o.helpers.smc,
            HelperKind::StatusDamage => o.helpers.status_damage,
            HelperKind::SubdualDamage => o.helpers.subdual_damage,
            HelperKind::Repulsor => o.helpers.repulsor,
            HelperKind::Defection => o.helpers.defection,
            HelperKind::WeaponStatus => o.helpers.weapon_status,
            HelperKind::FiringTracker => o.helpers.firing_tracker,
            HelperKind::TempWeaponBonus => o.helpers.temp_weapon_bonus,
        };
        let update_index = idx.and_then(|i| o.modules.get(i as usize)).and_then(|m| m.update_index);
        self.cut(CutCall::HelperSleepUntil { object: id, kind, when });
        let Some(u) = update_index else { return };
        let now = self.frame();
        let delay = if when == crate::common::game_common::NEVER || when == crate::common::game_common::FOREVER {
            crate::logic::module::update_module::UPDATE_SLEEP_FOREVER.0
        } else {
            when.wrapping_sub(now)
        };
        let handle = crate::logic::world::sleepy::UpdateHandle { object: id, index: u };
        let _ = self.awaken_update_module(id, handle, now.wrapping_add(delay));
    }

    /// `Object::initObject()`: "emit message announcing object's creation".
    /// The drawable is made and bound; the upgrade modules run with the
    /// player's flags; difficulty and battle-plan bonuses apply; the special
    /// powers the modules implement are recorded; the scripts hear of a new
    /// unit; the weapon set is brought up to date; a mine is counted for the
    /// neutral player's academy statistics.
    pub fn init_object(&mut self, id: ObjectId) {
        let Some(o) = self.objects.find_mut(id) else { return };
        // Weapons & Damage ---------------------------------------------------------------------------------
        for i in 0..o.last_weapon_condition.len() {
            o.last_weapon_condition[i] = -1; // WSF_INVALID
        }

        // emit message announcing object's creation
        self.send_object_created(id);

        // If I have a valid team assigned, I can run through my Upgrade modules with his flags
        self.update_upgrade_modules(id);

        // If the player has battle plans (America Strategy Center), then apply those bonuses to this object if
        // applicable. Internally it validates certain kinds of objects.
        if let Some(controller) = self.get_controlling_player(id) {
            let receiving = self.objects.find(id).is_some_and(|o| o.is_receiving_difficulty_bonus);
            if !receiving && self.cuts.globals.objects_should_receive_difficulty_bonus {
                self.set_receiving_difficulty_bonus(id, true);
            }
            if self.cuts.player_num_battle_plans_active(controller) > 0 {
                self.cut(CutCall::PlayerApplyBattlePlanBonusesForObject { player: controller, object: id });
            }
        }

        // For each special power module that we have, add it's type to the specialpower bits. This is
        // for optimal access later.
        for idx in self.modules_with(id, MODULEINTERFACE_SPECIAL_POWER) {
            if let ModuleReply::Int(t) = self.module_call(id, idx, ModuleCall::GetSpecialPowerType) {
                if let Some(o) = self.objects.find_mut(id) {
                    o.special_power_bits.set(t, 1);
                }
            }
        }

        let o = self.objects.find(id).expect("checked above");
        if !o.is_kind_of(KINDOF_PROJECTILE) && !o.is_kind_of(KINDOF_INERT) {
            // Notify script conditions to update conditions that consider unit counts.
            // We ignore projectiles cause they are frequently created & destroyed, and are not of general interest.
            self.cut(CutCall::ScriptEngineNotifyOfObjectCreationOrDestruction);
            self.cut(CutCall::GameLogicUpdateObjectsChangedTriggerAreas);
        }

        // Everything (like weaponSet flags) is inited, so check if the WeaponSet needs to change.
        self.cut(CutCall::WeaponSetUpdate { object: id });

        let o = self.objects.find(id).expect("checked above");
        if o.is_kind_of(KINDOF_MINE) || o.is_kind_of(KINDOF_BOOBY_TRAP) || o.is_kind_of(KINDOF_DEMOTRAP) {
            self.cut(CutCall::AcademyStatsRecordMine);
        }
    }

    /// `GameLogic::sendObjectCreated(obj)`: the client makes the drawable
    /// (`ObjectCreated`), and `bindObjectAndDrawable` binds the two. The
    /// drawable's id is the next of the client's counter
    /// (`cuts.next_drawable_id`, the headless stand-in of
    /// `GameClient::allocateNewDrawableID`).
    pub fn send_object_created(&mut self, id: ObjectId) {
        let Some(o) = self.objects.find(id) else { return };
        let template_id = o.template().get_template_id();
        self.events.push(crate::logic::client_event::ClientEvent::ObjectCreated { template_id, drawable_status_bits: 0, object_id: id });
        let draw = self.cuts.allocate_drawable_id();
        self.friend_bind_to_drawable(id, draw);
    }

    /// `GameLogic::sendObjectDestroyed(obj)`: the client destroys the
    /// drawable (`ObjectDestroyed`), and the binding is erased.
    pub fn send_object_destroyed(&mut self, id: ObjectId) {
        self.events.push(crate::logic::client_event::ClientEvent::ObjectDestroyed { object_id: id });
        self.friend_bind_to_drawable(id, INVALID_DRAWABLE_ID);
    }

    /// `friend_bindToDrawable(draw)`: sets the drawable association (the
    /// client's id; `INVALID_DRAWABLE_ID` to unbind). A new drawable gets the
    /// model-condition flags of the object's weapon set (and, if the global
    /// data forces it, the time of day and the weather); then every module is
    /// told `onDrawableBoundToObject`.
    pub fn friend_bind_to_drawable(&mut self, id: ObjectId, draw: DrawableId) {
        let Some(o) = self.objects.find_mut(id) else { return };
        o.drawable = draw;
        if draw != INVALID_DRAWABLE_ID {
            let mut set = [false; 128];
            let mut clr = [false; 128];
            for (i, &mcs) in WEAPON_SET_TYPE_TO_MODEL_CONDITION.iter().enumerate() {
                if mcs != MODELCONDITION_INVALID {
                    if o.cur_weapon_set_flags.test(i as i32) {
                        set[mcs as usize] = true;
                    } else {
                        clr[mcs as usize] = true;
                    }
                }
            }
            let g = self.cuts.globals;
            if g.force_models_to_follow_time_of_day {
                set[MODELCONDITION_NIGHT as usize] = g.time_of_day_is_night;
            }
            if g.force_models_to_follow_weather {
                set[MODELCONDITION_SNOW as usize] = g.weather_is_snowy;
            }
            let list = |a: &[bool; 128]| -> Vec<i32> { (0..128).filter(|&i| a[i]).map(|i| i as i32).collect() };
            self.emit(ObjectClientEvent::DrawableModelCondition {
                object_id: id,
                op: ModelConditionOp::ClearAndSet { clear: list(&clr), set: list(&set) },
            });
        }
        self.call_all_modules(id, ModuleCall::OnDrawableBoundToObject);
    }

    /// `Object::onDestroy()`, "run during `TheGameLogic::destroyObject`": the
    /// container is told to let go of this object ("this is the old
    /// cleanUpContain safeguard. Say goodbye so they don't try to look us
    /// up"), every module gets `onDelete`, and the object stops looking.
    pub fn on_destroy(&mut self, id: ObjectId) {
        let Some(o) = self.objects.find(id) else { return };
        let contained_by = o.contained_by;
        if contained_by != INVALID_ID {
            if let Some(c) = self.get_contain(contained_by) {
                self.module_call(contained_by, c, ModuleCall::RemoveFromContain(id));
            }
        }
        // run the onDelete on all modules present so they each have an opportunity to cleanup
        // anything they need to ... including talking to any other modules
        self.call_all_modules(id, ModuleCall::OnDelete);
        // Have to remove ourself from looking as well. RebuildHoleWorkers definately hit here.
        self.handle_partition_cell_maintenance(id);
    }

    /// `Object::~Object()`: the pathfinder and the scripts forget the object,
    /// it leaves the radar, the client destroys its drawable, it leaves its
    /// team, the partition manager and its AI group, and its modules are
    /// deleted. `obj` is already out of the list and the table (see the
    /// module docs).
    pub(crate) fn delete_instance(&mut self, obj: Object) {
        let id = obj.id;
        if self.objects.insert_unlisted(obj).is_err() {
            return;
        }
        // tell the AI the building is gone
        self.cut(CutCall::PathfinderRemoveObjectFromPathfindMap { object: id });

        let o = self.objects.find(id).expect("inserted above");
        if !o.is_kind_of(KINDOF_PROJECTILE) && !o.is_kind_of(KINDOF_INERT) {
            // Notify script conditions to update conditions that consider unit counts.
            self.cut(CutCall::GameLogicUpdateObjectsChangedTriggerAreas);
            self.cut(CutCall::ScriptEngineNotifyOfObjectCreationOrDestruction);
        }

        // remove from radar before we NULL out the team ... the order of ops are critical here
        // because the radar code will sometimes look at the team info
        if self.objects.find(id).is_some_and(|o| o.on_radar) {
            self.radar_remove_object(id);
        }

        // emit message announcing object's destruction. Again, order is important; we must do this
        // before wiping out the team.
        self.send_object_destroyed(id);

        // empty the team
        self.set_team(id, None);

        // Object's set of these persist for the life of the object.
        let o = self.objects.find_mut(id).expect("inserted above");
        o.partition_last_look.reset();
        o.partition_reveal_all_last_look.reset();
        o.partition_last_shroud.reset();
        o.partition_last_threat.reset();
        o.partition_last_value.reset();

        // remove the object from the partition system if present
        if self.objects.find(id).is_some_and(|o| o.in_partition) {
            self.partition_unregister_object(id);
        }

        // if we are in a group, remove us
        let group = self.objects.find(id).map_or(0, |o| o.group);
        if group != 0 {
            self.cut(CutCall::AiGroupRemove { group, object: id });
        }

        // delete any modules present (the borrow checker's `delete`: dropping them), the experience tracker,
        // the helper pointers; "reset id to zero so we never mistaken grab 'dead' objects"
        let _ = self.objects.remove(id);

        // Instead of removing it from the named cache, notify the script engine that it has died.
        self.cut(CutCall::ScriptEngineNotifyOfObjectDestruction { object: id });
    }

    /// `ThePartitionManager->registerObject(obj)`: the object has partition data.
    pub fn partition_register_object(&mut self, id: ObjectId) {
        if let Some(o) = self.objects.find_mut(id) {
            o.in_partition = true;
            self.cut(CutCall::PartitionRegisterObject { object: id });
        }
    }

    /// `ThePartitionManager->unRegisterObject(obj)`.
    pub fn partition_unregister_object(&mut self, id: ObjectId) {
        if let Some(o) = self.objects.find_mut(id) {
            o.in_partition = false;
            self.cut(CutCall::PartitionUnregisterObject { object: id });
        }
    }

    /// The player mask of the object's controlling player (0 without one).
    pub fn controlling_player_mask(&self, id: ObjectId) -> PlayerMaskType {
        self.get_controlling_player(id).map_or(0, |p| self.cuts.player_mask(p))
    }

    /// `m_constructionPercent` is complete (`CONSTRUCTION_COMPLETE`).
    pub fn is_construction_complete(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some_and(|o| o.construction_percent == CONSTRUCTION_COMPLETE)
    }
}
