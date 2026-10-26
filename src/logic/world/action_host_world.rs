// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file (this repository's own design, not upstream code):
// the world's answers to what `ActionManager` asks of objects beyond the weapon code.

//! [`ActionHost`] for the [`World`] (`p1w13-sonnet`, batch R79): what `ActionManager`'s queries ask of an object, its modules and the
//! world, so that they run on the game's own objects. Each answer is the world's own object state, one of its object-system functions,
//! or a [`ModuleCall`] to the module that owns the interface (asked in module order, as upstream walks `getBehaviorModules()`); the
//! AI layer's interfaces (the supply truck's and the dozer's, `SpecialAbilityUpdate`) are module calls that the AI modules answer once
//! they are translated (a probe in the tests). The weapon questions are [`ObjectWeaponHost`]'s, which the world answers already
//! (`weapon_host_world`).
//!
//! The queries themselves (`logic::action_manager`) are verified against upstream's `ActionManager.cpp` by
//! `tests/differential_weapon_fire.rs`; this file is the cut point between them and the world.

use crate::common::base_type::Coord3D;
use crate::common::disabled_types::DisabledType;
use crate::common::game_common::{CellShroudStatus, ObjectShroudStatus, PlayerType, Relationship};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::module::MODULEINTERFACE_COLLIDE;
use crate::common::special_power_type::SpecialPowerType;
use crate::logic::action_manager::{
    ActionHost, CollideFacts, DozerFacts, NamedUpdateModule, ParkingFacts, SpecialAbilityFacts, SpecialPowerTemplateRef, SupplyTruckFacts,
    DOZER_TASK_BUILD,
};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleReply};

impl World {
    /// The id the module call replies with (`INVALID_ID` for anything else).
    fn reply_id(r: ModuleReply) -> ObjectId {
        match r {
            ModuleReply::Id(id) => id,
            _ => INVALID_ID,
        }
    }

    /// The modules of `o` that answer `has` with yes, in module order.
    fn modules_answering(&mut self, o: ObjectId, has: ModuleCall) -> Vec<crate::logic::world::object::ModuleIndex> {
        self.all_modules(o).into_iter().filter(|&idx| self.module_call(o, idx, has.clone()).as_bool()).collect()
    }
}

impl ActionHost for World {
    fn is_mobile(&mut self, o: ObjectId) -> bool {
        World::is_mobile(self, o)
    }
    fn carrier_deck_height(&mut self, o: ObjectId) -> f32 {
        self.get_carrier_deck_height(o)
    }
    fn is_disabled_by_type(&mut self, o: ObjectId, t: DisabledType) -> bool {
        self.objects.find(o).is_some_and(|x| x.is_disabled_by_type(t))
    }
    fn body_health(&mut self, o: ObjectId) -> Option<(f32, f32)> {
        let body = self.get_body_module(o)?;
        let health = self.module_call(o, body, ModuleCall::GetHealth).as_real(0.0);
        let max = self.module_call(o, body, ModuleCall::GetMaxHealth).as_real(0.0);
        Some((health, max))
    }
    fn transport_slot_count(&mut self, o: ObjectId) -> i32 {
        self.get_transport_slot_count(o)
    }
    fn is_faction_structure(&mut self, o: ObjectId) -> bool {
        World::is_faction_structure(self, o)
    }
    fn object_shrouded_status(&mut self, o: ObjectId, player_index: i32) -> ObjectShroudStatus {
        self.get_shrouded_status(o, player_index)
    }
    fn builder_id(&mut self, o: ObjectId) -> ObjectId {
        self.objects.find(o).map_or(INVALID_ID, |x| x.builder_id)
    }
    fn controlling_player_is_human(&mut self, o: ObjectId) -> Option<bool> {
        let p = self.get_controlling_player(o)?;
        Some(self.players.get_nth(p).is_some_and(|pl| pl.player_type == PlayerType::Human))
    }
    fn player_relationship_to_team_of(&mut self, player_index: i32, o: ObjectId) -> Relationship {
        let team = self.get_team(o);
        self.cuts.player_relationship_to_team(player_index, team)
    }

    // ---- special powers -------------------------------------------------------------------------------------------
    fn has_special_power(&mut self, o: ObjectId, t: SpecialPowerType) -> bool {
        self.objects.find(o).is_some_and(|x| x.has_special_power(t))
    }
    fn special_power_percent_ready_by_type(&mut self, o: ObjectId, t: SpecialPowerType) -> Option<f32> {
        let idx = self.find_special_power_module_interface(o, t)?;
        Some(self.module_call(o, idx, ModuleCall::GetPercentReady).as_real(0.0))
    }
    fn special_power_percent_ready_by_template(&mut self, o: ObjectId, template: &SpecialPowerTemplateRef) -> Option<f32> {
        let idx = self.get_special_power_module(o, Some(template.id))?;
        Some(self.module_call(o, idx, ModuleCall::GetPercentReady).as_real(0.0))
    }
    fn special_ability_update(&mut self, o: ObjectId, t: SpecialPowerType) -> Option<SpecialAbilityFacts> {
        let idx = self.find_special_ability_update(o, t)?;
        let object_count = self.module_call(o, idx, ModuleCall::SpecialAbilityGetSpecialObjectCount).as_int(0);
        let object_max = self.module_call(o, idx, ModuleCall::SpecialAbilityGetSpecialObjectMax).as_int(0);
        Some(SpecialAbilityFacts { object_count, object_max })
    }
    fn special_ability_has_object_with_producer(&mut self, o: ObjectId, t: SpecialPowerType, target: ObjectId) -> bool {
        let Some(idx) = self.find_special_ability_update(o, t) else { return false };
        self.module_call(o, idx, ModuleCall::SpecialAbilityFindSpecialObjectWithProducerId(target)).as_bool()
    }
    fn special_power_overridable_destination_active(&mut self, o: ObjectId, _t: SpecialPowerType) -> bool {
        self.find_special_power_with_overridable_destination_active(o).is_some()
    }

    // ---- modules --------------------------------------------------------------------------------------------------
    fn has_named_update_module(&mut self, o: ObjectId, which: NamedUpdateModule) -> bool {
        let name: &[u8] = match which {
            NamedUpdateModule::SupplyWarehouseDock => b"SupplyWarehouseDockUpdate",
            NamedUpdateModule::SupplyCenterDock => b"SupplyCenterDockUpdate",
            NamedUpdateModule::RailedTransportDock => b"RailedTransportDockUpdate",
            NamedUpdateModule::Railroad => b"RailroadBehavior",
        };
        self.find_module(o, name).is_some()
    }
    fn warehouse_boxes_stored(&mut self, o: ObjectId) -> i32 {
        let Some(idx) = self.find_module(o, b"SupplyWarehouseDockUpdate") else { return 0 };
        self.module_call(o, idx, ModuleCall::SupplyWarehouseGetBoxesStored).as_int(0)
    }
    fn has_ai_interface(&mut self, o: ObjectId) -> bool {
        self.objects.find(o).is_some_and(|x| x.ai.is_some())
    }
    fn supply_truck_ai(&mut self, o: ObjectId) -> Option<SupplyTruckFacts> {
        let ai = self.objects.find(o)?.ai?;
        if !self.module_call(o, ai, ModuleCall::HasSupplyTruckAiInterface).as_bool() {
            return None;
        }
        let number_boxes = self.module_call(o, ai, ModuleCall::SupplyTruckGetNumberBoxes).as_int(0);
        let available_for_supplying = self.module_call(o, ai, ModuleCall::SupplyTruckIsAvailableForSupplying).as_bool();
        Some(SupplyTruckFacts { number_boxes, available_for_supplying })
    }
    fn dozer_ai(&mut self, o: ObjectId) -> Option<DozerFacts> {
        let ai = self.objects.find(o)?.ai?;
        if !self.module_call(o, ai, ModuleCall::AiGetDozerAIInterface).as_bool() {
            return None;
        }
        let current_task = self.module_call(o, ai, ModuleCall::DozerGetCurrentTask).as_int(0);
        let build_task_target = World::reply_id(self.module_call(o, ai, ModuleCall::DozerGetTaskTarget(DOZER_TASK_BUILD)));
        Some(DozerFacts { current_task, build_task_target })
    }
    fn has_dock_update_interface(&mut self, o: ObjectId) -> bool {
        !self.modules_answering(o, ModuleCall::HasDockUpdateInterface).is_empty()
    }
    fn collide_facts(&mut self, o: ObjectId, other: ObjectId) -> Vec<CollideFacts> {
        let mut facts = Vec::new();
        for idx in self.modules_with(o, MODULEINTERFACE_COLLIDE) {
            facts.push(CollideFacts {
                would_like: self.module_call(o, idx, ModuleCall::WouldLikeToCollideWith { other }).as_bool(),
                car_bomb: self.module_call(o, idx, ModuleCall::IsCarBombCrateCollide).as_bool(),
                hijack: self.module_call(o, idx, ModuleCall::IsHijackedVehicleCrateCollide).as_bool(),
                sabotage: self.module_call(o, idx, ModuleCall::IsSabotageBuildingCrateCollide).as_bool(),
            });
        }
        facts
    }
    fn parking_facts(&mut self, dest: ObjectId, obj: ObjectId) -> Vec<ParkingFacts> {
        let template = self.objects.find(obj).map(|x| x.template().get_template_id());
        let mut facts = Vec::new();
        for idx in self.modules_answering(dest, ModuleCall::HasParkingPlaceInterface) {
            let has_reserved_space = self.module_call(dest, idx, ModuleCall::ParkingPlaceHasReservedSpace { intended: obj }).as_bool();
            let (should_reserve_door, has_available_space) = match template {
                Some(template) => (
                    self.module_call(dest, idx, ModuleCall::ParkingPlaceShouldReserveDoorWhenQueued { template }).as_bool(),
                    self.module_call(dest, idx, ModuleCall::ParkingPlaceHasAvailableSpaceFor { template }).as_bool(),
                ),
                None => (false, false),
            };
            facts.push(ParkingFacts { has_reserved_space, should_reserve_door, has_available_space });
        }
        facts
    }
    fn contain_is_heal_contain(&mut self, o: ObjectId) -> bool {
        let Some(c) = self.objects.find(o).and_then(|x| x.contain) else { return false };
        self.module_call(o, c, ModuleCall::IsHealContain).as_bool()
    }
    fn contain_stealth_units_contained(&mut self, o: ObjectId) -> i32 {
        let Some(c) = self.objects.find(o).and_then(|x| x.contain) else { return 0 };
        self.module_call(o, c, ModuleCall::GetStealthUnitsContained).as_int(0)
    }
    fn contain_is_valid_container_for(&mut self, container: ObjectId, obj: ObjectId, check_capacity: bool) -> bool {
        let Some(c) = self.objects.find(container).and_then(|x| x.contain) else { return false };
        self.module_call(container, c, ModuleCall::IsValidContainerFor { obj, check_capacity }).as_bool()
    }
    fn contain_closest_rider(&mut self, container: ObjectId, pos: &Coord3D) -> Option<ObjectId> {
        let c = self.objects.find(container)?.contain?;
        Some(World::reply_id(self.module_call(container, c, ModuleCall::GetClosestRider(*pos)))).filter(|&id| id != INVALID_ID)
    }
    fn spawn_closest_slave(&mut self, o: ObjectId, pos: &Coord3D) -> Option<Option<ObjectId>> {
        let idx = *self.modules_answering(o, ModuleCall::HasSpawnBehaviorInterface).first()?;
        let slave = World::reply_id(self.module_call(o, idx, ModuleCall::GetClosestSlave(*pos)));
        Some((slave != INVALID_ID).then_some(slave))
    }

    // ---- the world ------------------------------------------------------------------------------------------------
    fn cell_shroud_status_for_player(&mut self, player_index: i32, loc: &Coord3D) -> CellShroudStatus {
        self.partition.get_shroud_status_for_player(player_index, loc)
    }
    fn is_underwater(&mut self, x: f32, y: f32) -> bool {
        self.terrain_is_underwater_water_z(x, y).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::kind_of::{KINDOF_HEAL_PAD, KINDOF_INFANTRY};
    use crate::common::module::MODULEINTERFACE_BODY;
    use crate::common::thing_template::{ObjectTemplateValues, ThingTemplate};
    use crate::logic::weapon_host::CommandSourceType;
    use crate::logic::world::module_probe::{ProbeAnswers, ProbeModule};
    use crate::logic::world::object_create::{NewModule, NewObject};
    use crate::logic::world::team::Team;
    use crate::logic::world::update_body::UpdateBody;
    use std::sync::Arc;

    fn world() -> World {
        let mut w = World::new();
        w.cuts.neutral_default_team = 1;
        w.teams.insert_team(Team::new(1, 0)).unwrap();
        w.cuts.team_players.insert(1, 0);
        w.cuts.team_relationships.insert((1, 1), Relationship::Allies);
        w
    }

    fn probe(name: &[u8], interfaces: i32, max: f32) -> NewModule {
        let mut m = NewModule::plain(name, b"ModuleTag_Probe");
        m.interfaces = interfaces;
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers { no_death: true, ..Default::default() }, max, Default::default())));
        m
    }

    fn make(w: &mut World, kinds: Vec<i32>, modules: Vec<NewModule>) -> ObjectId {
        let v = ObjectTemplateValues { kind_of: kinds, ..Default::default() };
        let id = w.new_object(NewObject { template: Arc::new(ThingTemplate::from_object_values(&v)), status: Default::default(), team: None, modules }).unwrap();
        w.objects.find_mut(id).unwrap().team = 1;
        id
    }

    fn set_health(w: &mut World, id: ObjectId, health: f32) {
        let idx = w.find_module(id, b"Body").unwrap();
        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(id).unwrap().modules[idx as usize].body.as_mut() {
            p.health = health;
        }
    }

    #[test]
    fn the_body_is_asked_through_its_module() {
        let mut w = world();
        let none = make(&mut w, vec![], vec![]);
        assert_eq!(w.body_health(none), None, "no body module");
        let id = make(&mut w, vec![], vec![probe(b"Body", MODULEINTERFACE_BODY, 100.0)]);
        set_health(&mut w, id, 40.0);
        assert_eq!(w.body_health(id), Some((40.0, 100.0)));
    }

    #[test]
    fn named_modules_and_missing_interfaces() {
        let mut w = world();
        let id = make(&mut w, vec![], vec![probe(b"RailroadBehavior", 0, 0.0)]);
        assert!(w.has_named_update_module(id, NamedUpdateModule::Railroad));
        assert!(!w.has_named_update_module(id, NamedUpdateModule::SupplyCenterDock));
        assert!(!w.has_ai_interface(id));
        assert_eq!(w.supply_truck_ai(id), None, "no AI: no supply truck interface");
        assert_eq!(w.dozer_ai(id), None);
        assert_eq!(w.special_ability_update(id, 1), None);
        assert!(!w.contain_is_heal_contain(id), "no contain module");
        assert_eq!(w.spawn_closest_slave(id, &Coord3D::default()), None, "no spawn behaviour");
        assert!(w.collide_facts(id, id).is_empty(), "no collide module");
    }

    #[test]
    fn an_action_manager_query_runs_on_the_world() {
        let mut w = world();
        let infantry = make(&mut w, vec![KINDOF_INFANTRY], vec![probe(b"Body", MODULEINTERFACE_BODY, 100.0)]);
        let pad = make(&mut w, vec![KINDOF_HEAL_PAD], vec![]);
        set_health(&mut w, infantry, 50.0);
        let cs = CommandSourceType::FromPlayer;
        assert!(crate::logic::action_manager::can_get_healed_at(&mut w, Some(infantry), Some(pad), cs), "hurt, at an allied heal pad");
        set_health(&mut w, infantry, 100.0);
        assert!(!crate::logic::action_manager::can_get_healed_at(&mut w, Some(infantry), Some(pad), cs), "no point at full health");
        assert!(!crate::logic::action_manager::can_get_healed_at(&mut w, Some(pad), Some(infantry), cs), "only infantry, at a heal pad");
    }
}
