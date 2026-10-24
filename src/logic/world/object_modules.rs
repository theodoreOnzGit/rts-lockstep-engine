// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp
//   (`findModule`, the interface lookups `getProductionUpdateInterface` ..
//   `findSpecialAbilityUpdate`, `onCollide`, `isSalvageCrate`,
//   `updateUpgradeModules`, `forceRefreshSubObjectUpgradeStatus`, the
//   upgrade functions, `getSpecialPowerModule` and `doSpecialPower`,
//   `pauseAllSpecialPowers`)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `Object`'s module list: finding a module, dispatching an interface call to
//! it, and the loops over `m_behaviors` that ask each module's interface
//! (collide, upgrade, special power, ...). See `object.rs` for the data and
//! `module_call.rs` for the calls.
//!
//! # Re-entrancy
//!
//! A call that reaches a module already running (upstream: a plain recursive
//! call) runs at once on the live state of the module, because a running body
//! *lends* itself to the slot around every call that can reach its own object
//! ([`World::lend_body`]). See `module_call.rs`.

use crate::common::base_type::Coord3D;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::module::{
    ModuleInterfaceType, MODULEINTERFACE_COLLIDE, MODULEINTERFACE_CREATE, MODULEINTERFACE_DESTROY,
    MODULEINTERFACE_DIE, MODULEINTERFACE_SPECIAL_POWER, MODULEINTERFACE_UPGRADE,
};
use crate::common::object_status_types::OBJECT_STATUS_NO_COLLISIONS;
use crate::common::special_power_type::{SpecialPowerType, SPECIAL_INVALID};
use crate::common::upgrade::UpgradeMaskType;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object::{ModuleIndex, ModuleSlot};
use crate::logic::world::update_body::UpdateBody;

impl World {
    // ---- checkout of a module's body ---------------------------------------

    /// Takes the body of module `h` out of its slot (the slot of the
    /// `BehaviorSlot` for a module with an update interface). `None` if the
    /// module is missing, has no body, or its body is already out.
    pub(crate) fn take_module_body(&mut self, h: ModuleHandle) -> Option<UpdateBody> {
        let obj = self.objects.find_mut(h.object)?;
        let slot = obj.modules.get_mut(h.index as usize)?;
        match slot.update_index {
            Some(u) => obj.behaviors.get_mut(u as usize)?.body.take(),
            None => slot.body.take(),
        }
    }

    // --- p1w13-sonnet ---
    /// The concrete body of module `h` where it is (the module's slot, or the update module's entry of `Object::behaviors`), for a test that reads a module's state
    /// without taking it out; `None` if the module is missing, has no body, or its body is out.
    pub fn peek_module_body(&self, h: ModuleHandle) -> Option<&UpdateBody> {
        let obj = self.objects.find(h.object)?;
        let slot = obj.modules.get(h.index as usize)?;
        match slot.update_index {
            Some(u) => obj.behaviors.get(u as usize)?.body.as_ref(),
            None => slot.body.as_ref(),
        }
    }
    // --- end p1w13-sonnet ---

    /// Puts a checked-out body back.
    pub(crate) fn put_module_body(&mut self, h: ModuleHandle, body: UpdateBody) {
        if let Some(obj) = self.objects.find_mut(h.object) {
            if let Some(slot) = obj.modules.get_mut(h.index as usize) {
                match slot.update_index {
                    Some(u) => {
                        if let Some(b) = obj.behaviors.get_mut(u as usize) {
                            b.body = Some(body);
                        }
                    }
                    None => slot.body = Some(body),
                }
            }
        }
    }

    /// Lends a running module's body to its slot for the duration of `f`, so
    /// that a call reaching the module from inside `f` is delivered **at
    /// once**, as upstream's plain recursion delivers it, and finds the live
    /// state (every change the module made before the lend, none lost).
    /// Returns `f`'s result and the body as `f` left it (nested calls may have
    /// changed it); the caller puts it back into its own state.
    ///
    /// `None` for the body only if the module is gone (its object was
    /// deleted inside `f`, which upstream defers to the end of the frame).
    ///
    /// A module must lend itself around every call into the world that can
    /// reach its own object. One that does not is a defect, and the call is
    /// then recorded in `cuts.reentrant_skips` instead of being lost silently.
    pub fn lend_body<R>(
        &mut self,
        me: ModuleHandle,
        body: UpdateBody,
        f: impl FnOnce(&mut World) -> R,
    ) -> (R, Option<UpdateBody>) {
        self.put_module_body(me, body);
        let r = f(self);
        (r, self.take_module_body(me))
    }

    /// Sends `call` to module `idx` of object `id` and returns its answer.
    /// [`ModuleReply::Unhandled`] if the object or module is missing, or the
    /// module has no body. A call to a module that is running delivers at once
    /// if the module lent itself ([`World::lend_body`]); if it did not, the
    /// call is recorded in `cuts.reentrant_skips` (a defect of that module).
    pub fn module_call(&mut self, id: ObjectId, idx: ModuleIndex, call: ModuleCall) -> ModuleReply {
        let h = ModuleHandle {
            object: id,
            index: idx,
        };
        let had_body = match self
            .objects
            .find(id)
            .and_then(|o| o.modules.get(idx as usize))
        {
            Some(s) => s.has_body,
            None => return ModuleReply::Unhandled,
        };
        let Some(mut body) = self.take_module_body(h) else {
            if had_body {
                self.cuts.reentrant_skips.push((h, call));
            }
            return ModuleReply::Unhandled;
        };
        if self.cuts.module_stack.contains(&h) {
            self.cuts.reentrant_delivered += 1;
        }
        self.cuts.module_stack.push(h);
        let reply = body.module_call(self, h, call);
        self.cuts.module_stack.pop();
        self.put_module_body(h, body);
        reply
    }

    /// The modules of `id` that have the interface `mask`, in module order,
    /// as upstream's `for (b = m_behaviors; *b; ++b) { x = (*b)->getX(); if (!x) continue; .. }`
    /// sees them. A snapshot: the list of an object does not change after it
    /// is made, so it is the same list at every step of the loop.
    pub fn modules_with(&self, id: ObjectId, mask: ModuleInterfaceType) -> Vec<ModuleIndex> {
        match self.objects.find(id) {
            Some(o) => o
                .modules
                .iter()
                .enumerate()
                .filter(|(_, m)| m.has_interface(mask))
                .map(|(i, _)| i as ModuleIndex)
                .collect(),
            None => Vec::new(),
        }
    }

    /// Every module of `id`, in module order (`for (b = m_behaviors; *b; ++b)`).
    pub fn all_modules(&self, id: ObjectId) -> Vec<ModuleIndex> {
        match self.objects.find(id) {
            Some(o) => (0..o.modules.len()).map(|i| i as ModuleIndex).collect(),
            None => Vec::new(),
        }
    }

    /// `ModuleSlot` of module `idx`.
    pub fn module_slot(&self, id: ObjectId, idx: ModuleIndex) -> Option<&ModuleSlot> {
        self.objects.find(id)?.modules.get(idx as usize)
    }

    /// Sends `call` to every module of `id` (`for (b = m_behaviors; *b; ++b) (*b)->x()`).
    pub fn call_all_modules(&mut self, id: ObjectId, call: ModuleCall) {
        for idx in self.all_modules(id) {
            self.module_call(id, idx, call.clone());
        }
    }

    // --- p1w11-sonnet ---
    /// The world-level post-load pass for the modules of `id`: `loadPostProcess()` of each module whose `loadPostProcess` needs the world
    /// (`PowerPlantUpgrade` re-adds its power bonus to the player), in module order, as [`ModuleCall::LoadPostProcess`]. It runs after the
    /// world-free `Snapshot::load_post_process` of every module has (that is `World::load_post_process`'s and the xfer chain's); see
    /// `docs/world-design.md`, "A World-aware `loadPostProcess`".
    pub fn modules_load_post_process(&mut self, id: ObjectId) {
        self.call_all_modules(id, ModuleCall::LoadPostProcess);
    }
    // --- end p1w11-sonnet ---

    // ---- findModule and the interface lookups ----------------------------------

    // --- p1w13-sonnet --- (batch R13: the sabotage crates)
    /// `for (u = getBehaviorModules(); *u; ++u) if ((*u)->getSpyVisionUpdate())`: the `SpyVisionUpdate` modules of `id`, in module order.
    pub fn modules_with_spy_vision(&self, id: ObjectId) -> Vec<ModuleIndex> {
        match self.objects.find(id) {
            Some(o) => o
                .modules
                .iter()
                .enumerate()
                .filter(|(_, m)| m.name == b"SpyVisionUpdate")
                .map(|(i, _)| i as ModuleIndex)
                .collect(),
            None => Vec::new(),
        }
    }

    /// The objects `player->iterateObjects(func)` visits: the test's list when it gives one (`cuts.player_objects`), else the objects the player
    /// controls, in the order of the object list. (Upstream walks the player's teams' member lists; the teams are layer 10's.)
    pub fn player_iterate_objects(
        &self,
        p: crate::logic::world::ids::PlayerIndex,
    ) -> Vec<ObjectId> {
        if let Some(v) = self.cuts.player_objects.get(&p) {
            return v.clone();
        }
        self.objects
            .ids_in_order()
            .into_iter()
            .filter(|&id| self.get_controlling_player(id) == Some(p))
            .collect()
    }
    // --- end p1w13-sonnet ---

    /// `findModule(key)`: the first module with that class name.
    pub fn find_module(&self, id: ObjectId, class_name: &[u8]) -> Option<ModuleIndex> {
        self.objects.find(id)?.find_module(class_name)
    }

    /// `getBodyModule()`.
    pub fn get_body_module(&self, id: ObjectId) -> Option<ModuleIndex> {
        self.objects.find(id)?.body
    }

    /// `getContain()`.
    pub fn get_contain(&self, id: ObjectId) -> Option<ModuleIndex> {
        self.objects.find(id)?.contain
    }

    /// `getProductionUpdateInterface()`: the first module with one.
    pub fn get_production_update_interface(&mut self, id: ObjectId) -> Option<ModuleIndex> {
        self.first_module_answering(id, ModuleCall::HasProductionUpdateInterface)
    }

    /// `getDockUpdateInterface()`.
    pub fn get_dock_update_interface(&mut self, id: ObjectId) -> Option<ModuleIndex> {
        self.first_module_answering(id, ModuleCall::HasDockUpdateInterface)
    }

    /// `getSpawnBehaviorInterface()`.
    pub fn get_spawn_behavior_interface(&mut self, id: ObjectId) -> Option<ModuleIndex> {
        self.first_module_answering(id, ModuleCall::HasSpawnBehaviorInterface)
    }

    /// `getProjectileUpdateInterface()`.
    pub fn get_projectile_update_interface(&mut self, id: ObjectId) -> Option<ModuleIndex> {
        self.first_module_answering(id, ModuleCall::HasProjectileUpdateInterface)
    }

    /// `getCountermeasuresBehaviorInterface()`.
    pub fn get_countermeasures_behavior_interface(&mut self, id: ObjectId) -> Option<ModuleIndex> {
        self.first_module_answering(id, ModuleCall::HasCountermeasuresInterface)
    }

    /// `findSpecialPowerWithOverridableDestinationActive(type)`: the first
    /// special power update module that allows plotting positions right now.
    /// (The `type` argument is unused upstream.)
    pub fn find_special_power_with_overridable_destination_active(
        &mut self,
        id: ObjectId,
    ) -> Option<ModuleIndex> {
        self.all_modules(id).into_iter().find(|&idx| {
            self.module_call(
                id,
                idx,
                ModuleCall::DoesSpecialPowerHaveOverridableDestinationActive,
            )
            .as_bool()
        })
    }

    /// `findSpecialPowerWithOverridableDestination(type)`: the first that may plot positions.
    pub fn find_special_power_with_overridable_destination(
        &mut self,
        id: ObjectId,
    ) -> Option<ModuleIndex> {
        self.all_modules(id).into_iter().find(|&idx| {
            self.module_call(
                id,
                idx,
                ModuleCall::DoesSpecialPowerHaveOverridableDestination,
            )
            .as_bool()
        })
    }

    /// `findSpecialAbilityUpdate(type)`: the first special ability update of that power type.
    pub fn find_special_ability_update(
        &mut self,
        id: ObjectId,
        t: SpecialPowerType,
    ) -> Option<ModuleIndex> {
        self.all_modules(id)
            .into_iter()
            .find(|&idx| matches!(self.module_call(id, idx, ModuleCall::GetSpecialAbilityType), ModuleReply::Int(ty) if ty == t))
    }

    /// `findSpecialPowerCompletionDie()`.
    pub fn find_special_power_completion_die(&self, id: ObjectId) -> Option<ModuleIndex> {
        self.find_module(id, b"SpecialPowerCompletionDie")
    }

    /// `hasCountermeasures()`: a countermeasures interface that is active.
    pub fn has_countermeasures(&mut self, id: ObjectId) -> bool {
        match self.get_countermeasures_behavior_interface(id) {
            Some(m) => self
                .module_call(id, m, ModuleCall::CountermeasuresIsActive)
                .as_bool(),
            None => false,
        }
    }

    /// `reportMissileForCountermeasures(missile)`: every countermeasures interface is told.
    pub fn report_missile_for_countermeasures(&mut self, id: ObjectId, missile: ObjectId) {
        for idx in self.all_modules(id) {
            if self
                .module_call(id, idx, ModuleCall::HasCountermeasuresInterface)
                .as_bool()
            {
                self.module_call(
                    id,
                    idx,
                    ModuleCall::ReportMissileForCountermeasures { missile },
                );
            }
        }
    }

    /// `calculateCountermeasureToDivertTo(victim)`: only an object with an AI
    /// asks; the victim's first countermeasures interface answers.
    pub fn calculate_countermeasure_to_divert_to(
        &mut self,
        id: ObjectId,
        victim: ObjectId,
    ) -> ObjectId {
        if self.objects.find(id).is_some_and(|o| o.ai.is_some()) {
            for idx in self.all_modules(victim) {
                if self
                    .module_call(victim, idx, ModuleCall::HasCountermeasuresInterface)
                    .as_bool()
                {
                    return self
                        .module_call(
                            victim,
                            idx,
                            ModuleCall::CalculateCountermeasureToDivertTo { victim },
                        )
                        .as_id();
                }
            }
        }
        INVALID_ID
    }

    /// `getObjectExitInterface()`: the first module with an update exit
    /// interface, else the contain module's exit interface.
    pub fn has_object_exit_interface(&mut self, id: ObjectId) -> bool {
        if self
            .first_module_answering(id, ModuleCall::HasUpdateExitInterface)
            .is_some()
        {
            return true;
        }
        // "If you don't have a fancy one, you may have one from your contain module"
        match self.get_contain(id) {
            Some(c) => self
                .module_call(id, c, ModuleCall::HasContainExitInterface)
                .as_bool(),
            None => false,
        }
    }

    /// batch R8: `getObjectExitInterface()` as the module that is it: the first module with an update exit interface, else the contain module if it
    /// has an exit interface.
    pub fn get_object_exit_interface(&mut self, id: ObjectId) -> Option<ModuleIndex> {
        if let Some(idx) = self.first_module_answering(id, ModuleCall::HasUpdateExitInterface) {
            return Some(idx);
        }
        // "If you don't have a fancy one, you may have one from your contain module"
        let c = self.get_contain(id)?;
        self.module_call(id, c, ModuleCall::HasContainExitInterface)
            .as_bool()
            .then_some(c)
    }

    /// The first module, in module order, that answers `true` to a
    /// `Has...Interface` call.
    pub(crate) fn first_module_answering(
        &mut self,
        id: ObjectId,
        call: ModuleCall,
    ) -> Option<ModuleIndex> {
        self.all_modules(id)
            .into_iter()
            .find(|&idx| self.module_call(id, idx, call.clone()).as_bool())
    }

    /// `findSpecialPowerModuleInterface(type)`: the first special power module
    /// whose template is of that type, or any when `type` is `SPECIAL_INVALID`.
    /// (Upstream's condition is `spTemplate && type == ... || type == SPECIAL_INVALID`:
    /// `&&` binds tighter, so `SPECIAL_INVALID` matches any special power
    /// module, template or not.)
    pub fn find_special_power_module_interface(
        &mut self,
        id: ObjectId,
        t: SpecialPowerType,
    ) -> Option<ModuleIndex> {
        for idx in self.modules_with(id, MODULEINTERFACE_SPECIAL_POWER) {
            let reply = self.module_call(id, idx, ModuleCall::GetSpecialPowerType);
            if matches!(reply, ModuleReply::Int(sp_type) if sp_type == t) || t == SPECIAL_INVALID {
                return Some(idx);
            }
        }
        None
    }

    /// `findAnyShortcutSpecialPowerModuleInterface()`.
    pub fn find_any_shortcut_special_power_module_interface(
        &mut self,
        id: ObjectId,
    ) -> Option<ModuleIndex> {
        // `spTemplate && spTemplate->isShortcutPower()`: a module with no template is skipped
        self.modules_with(id, MODULEINTERFACE_SPECIAL_POWER)
            .into_iter()
            .find(|&idx| {
                matches!(
                    self.module_call(id, idx, ModuleCall::GetSpecialPowerType),
                    ModuleReply::Int(_)
                ) && self
                    .module_call(id, idx, ModuleCall::IsShortcutPower)
                    .as_bool()
            })
    }

    /// `getSpecialPowerModule(template)`: the module that implements the
    /// special power with this template id; `None` for no template.
    pub fn get_special_power_module(
        &mut self,
        id: ObjectId,
        template: Option<u32>,
    ) -> Option<ModuleIndex> {
        let template = template?;
        self.modules_with(id, MODULEINTERFACE_SPECIAL_POWER)
            .into_iter()
            .find(|&idx| {
                self.module_call(id, idx, ModuleCall::IsModuleForPower { template })
                    .as_bool()
            })
    }

    /// `pauseAllSpecialPowers(disabling)`.
    pub fn pause_all_special_powers(&mut self, id: ObjectId, disabling: bool) {
        for idx in self.modules_with(id, MODULEINTERFACE_SPECIAL_POWER) {
            self.module_call(id, idx, ModuleCall::PauseCountdown { disabling });
        }
    }

    // ---- collide ------------------------------------------------------------------

    /// `onCollide(other, loc, normal)`: each collide module in turn, stopping
    /// as soon as one of them sets `OBJECT_STATUS_NO_COLLISIONS` on this
    /// object ("check each time thru the loop, in case a collide module sets it").
    pub fn on_collide(
        &mut self,
        id: ObjectId,
        other: ObjectId,
        loc: Option<Coord3D>,
        normal: Option<Coord3D>,
    ) {
        for idx in self.modules_with(id, MODULEINTERFACE_COLLIDE) {
            if self
                .objects
                .find(id)
                .is_some_and(|o| o.status.test(OBJECT_STATUS_NO_COLLISIONS))
            {
                break;
            }
            self.module_call(id, idx, ModuleCall::OnCollide { other, loc, normal });
        }
    }

    /// `isSalvageCrate()`.
    pub fn is_salvage_crate(&mut self, id: ObjectId) -> bool {
        for idx in self.modules_with(id, MODULEINTERFACE_COLLIDE) {
            if self
                .module_call(id, idx, ModuleCall::IsSalvageCrateCollide)
                .as_bool()
            {
                return true;
            }
        }
        false
    }

    // ---- create and destroy modules ---------------------------------------------

    /// The loop of `ThingFactory::newObject` after the object is made: every
    /// create module's `onCreate()`.
    pub fn call_create_modules(&mut self, id: ObjectId) {
        for idx in self.modules_with(id, MODULEINTERFACE_CREATE) {
            self.module_call(id, idx, ModuleCall::OnCreate);
        }
    }

    /// The loop of `GameLogic::destroyObject`: every destroy module's `onDestroy()`.
    pub fn call_destroy_modules(&mut self, id: ObjectId) {
        for idx in self.modules_with(id, MODULEINTERFACE_DESTROY) {
            self.module_call(id, idx, ModuleCall::OnDestroy);
        }
    }

    /// `Object::onDie`'s first loop: every die module's `onDie(damageInfo)`.
    pub(crate) fn call_die_modules(
        &mut self,
        id: ObjectId,
        info: &crate::logic::damage::DamageInfo,
    ) {
        for idx in self.modules_with(id, MODULEINTERFACE_DIE) {
            self.module_call(id, idx, ModuleCall::OnDie(info.clone()));
        }
    }

    // ---- upgrades -----------------------------------------------------------------

    /// `updateUpgradeModules()`: "our owning player is telling us to recheck
    /// our UpgradeModules, as an upgrade has completed". Nothing while under
    /// construction ("the three places that clear UnderConstruction will
    /// re-update us"), destroyed, or without a player. Each upgrade module
    /// that has not run yet is asked to `attemptUpgrade` with the union of
    /// the player's completed upgrades and the object's own ("we combine all
    /// the masks in case someone has a Object AND Player combination").
    pub fn update_upgrade_modules(&mut self, id: ObjectId) {
        let Some(o) = self.objects.find(id) else {
            return;
        };
        if o.status
            .test(crate::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION)
        {
            return;
        }
        if o.status
            .test(crate::common::object_status_types::OBJECT_STATUS_DESTROYED)
        {
            return; // Patch 1.03: fixes a crash when a fake GLA command center is upgraded to a real one
        }
        let Some(player) = self.get_controlling_player(id) else {
            return; // "This can only happen in game teardown."
        };
        let player_mask = self.cuts.player_completed_upgrade_mask(player);
        let object_mask = self
            .objects
            .find(id)
            .map_or(UpgradeMaskType::new(), |o| o.object_upgrades_completed);
        let mut mask_to_check = player_mask;
        mask_to_check.set_mask(&object_mask);
        for idx in self.modules_with(id, MODULEINTERFACE_UPGRADE) {
            if !self
                .module_call(id, idx, ModuleCall::IsAlreadyUpgraded)
                .as_bool()
            {
                self.module_call(id, idx, ModuleCall::AttemptUpgrade(mask_to_check));
            }
        }
    }

    /// `forceRefreshSubObjectUpgradeStatus()`.
    pub fn force_refresh_sub_object_upgrade_status(&mut self, id: ObjectId) {
        for idx in self.modules_with(id, MODULEINTERFACE_UPGRADE) {
            if self
                .module_call(id, idx, ModuleCall::IsSubObjectsUpgrade)
                .as_bool()
            {
                self.module_call(id, idx, ModuleCall::ForceRefreshUpgrade);
            }
        }
    }

    /// `hasUpgrade(upgradeT)`: the object's own mask contains all of the
    /// upgrade's bits.
    pub fn has_upgrade(&self, id: ObjectId, upgrade_mask: &UpgradeMaskType) -> bool {
        self.objects
            .find(id)
            .is_some_and(|o| o.object_upgrades_completed.test_for_all(upgrade_mask))
    }

    /// `affectedByUpgrade(upgradeT)`: whether any upgrade module would act on
    /// the union of the player's, the object's and this upgrade's masks.
    pub fn affected_by_upgrade(&mut self, id: ObjectId, upgrade_mask: &UpgradeMaskType) -> bool {
        let object_mask = self
            .objects
            .find(id)
            .map_or(UpgradeMaskType::new(), |o| o.object_upgrades_completed);
        let player_mask = match self.get_controlling_player(id) {
            Some(p) => self.cuts.player_completed_upgrade_mask(p),
            None => UpgradeMaskType::new(),
        };
        let mut mask_to_check = player_mask;
        mask_to_check.set_mask(&object_mask);
        mask_to_check.set_mask(upgrade_mask);
        for idx in self.modules_with(id, MODULEINTERFACE_UPGRADE) {
            if self
                .module_call(id, idx, ModuleCall::WouldUpgrade(mask_to_check))
                .as_bool()
            {
                // "if any of my many upgrade modules would execute in response to this flag, say yes."
                return true;
            }
        }
        false
    }

    /// `giveUpgrade(upgradeT)`: add the upgrade's bits to the object's mask,
    /// then re-run the upgrade modules. Nothing for no upgrade.
    pub fn give_upgrade(&mut self, id: ObjectId, upgrade_mask: Option<&UpgradeMaskType>) {
        if let Some(mask) = upgrade_mask {
            if let Some(o) = self.objects.find_mut(id) {
                o.object_upgrades_completed.set_mask(mask);
            }
            self.update_upgrade_modules(id);
        }
    }

    /// `removeUpgrade(upgradeT)`: clear the bits and reset the modules so the
    /// upgrade may run again ("it is not removing anything in the sense of
    /// undoing the effects").
    pub fn remove_upgrade(&mut self, id: ObjectId, upgrade_mask: &UpgradeMaskType) {
        if let Some(o) = self.objects.find_mut(id) {
            o.object_upgrades_completed.clear_mask(upgrade_mask);
        }
        for idx in self.modules_with(id, MODULEINTERFACE_UPGRADE) {
            self.module_call(id, idx, ModuleCall::ResetUpgrade(*upgrade_mask));
        }
    }

    // ---- special powers -----------------------------------------------------------

    /// `doSpecialPower(template, commandOptions, forced)` and its three
    /// variants (`AtObject`, `AtLocation`, `UsingWaypoints`): nothing while
    /// disabled; unless forced, the special power store must allow it (a cut
    /// point); then the module for that power does it.
    pub fn do_special_power(
        &mut self,
        id: ObjectId,
        template: Option<u32>,
        target: SpecialPowerTarget,
        command_options: u32,
        forced: bool,
    ) {
        if self.objects.find(id).is_some_and(|o| o.is_disabled()) {
            return;
        }
        if !forced && !self.cut_special_power_can_use(id, template) {
            return;
        }
        if let Some(m) = self.get_special_power_module(id, template) {
            let call = match target {
                SpecialPowerTarget::None => ModuleCall::DoSpecialPower { command_options },
                SpecialPowerTarget::Object(target) => ModuleCall::DoSpecialPowerAtObject {
                    target,
                    command_options,
                },
                SpecialPowerTarget::Location { loc, angle } => {
                    ModuleCall::DoSpecialPowerAtLocation {
                        loc,
                        angle,
                        command_options,
                    }
                }
                SpecialPowerTarget::Waypoint(waypoint) => {
                    ModuleCall::DoSpecialPowerUsingWaypoints {
                        waypoint,
                        command_options,
                    }
                }
            };
            self.module_call(id, m, call);
        }
    }
}

/// What a special power is aimed at (which of the four `doSpecialPower*`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpecialPowerTarget {
    /// `doSpecialPower`.
    None,
    /// `doSpecialPowerAtObject`.
    Object(ObjectId),
    /// `doSpecialPowerAtLocation`.
    Location { loc: Coord3D, angle: f32 },
    /// `doSpecialPowerUsingWaypoints`, the waypoint by id.
    Waypoint(u32),
}
