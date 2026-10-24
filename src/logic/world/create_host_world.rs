// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file (this repository's own design, not upstream code):
// the world's answers to the calls the layer-9 create modules make.

//! [`CreateHost`] for the [`World`]: what the seven create modules
//! (`PreorderCreate`, `SpecialPowerCreate`, `SupplyWarehouseCreate`,
//! `SupplyCenterCreate`, `LockWeaponCreate`, `VeterancyGainCreate`,
//! `GrantUpgradeCreate`) ask of the object, its player and the game logic.
//!
//! Each method is the world's own data where the world has it (the player's
//! preorder flag, sciences, resource gathering manager and academy stats; the
//! object's weapon set and experience record; the upgrade center) and a cut
//! point where the Player operation belongs to layer 12
//! ([`CutCall::PlayerAddUpgradeComplete`]).
//!
//! `ScienceRequired` is held as a name by the module data and resolved here
//! with the science store ([`GameStatics::science_store`](crate::logic::world::statics::GameStatics)),
//! at the time of use; upstream resolves it when the INI is read.

use crate::common::game_common::VeterancyLevel;
use crate::common::game_type::ObjectId;
use crate::common::model_state::ModelConditionFlagType;
use crate::common::module::MODULEINTERFACE_SPECIAL_POWER;
use crate::common::name_key_generator::NameKeyType;
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::common::upgrade::UpgradeType;
use crate::logic::modules::create_host::{CreateHost, UpgradeRef};
use crate::logic::weapon_set_state::WeaponLockType;
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::PlayerIndex;
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::object_cuts::CutCall;

impl World {
    /// `ExperienceTracker::setMinVeterancyLevel(newLevel)`: raises the level
    /// (with feedback) if it is below `new_level`; never lowers it. It does
    /// not test trainability ("for explicit setting").
    pub fn cut_experience_set_min_veterancy_level(
        &mut self,
        id: ObjectId,
        new_level: VeterancyLevel,
    ) {
        let Some(o) = self.objects.find_mut(id) else {
            return;
        };
        if (o.experience.level as i32) < new_level as i32 {
            let old_level = o.experience.level;
            o.experience.level = new_level;
            o.experience.points = o.template().get_experience_required(new_level); // minimum for this level
            self.on_veterancy_level_changed(id, old_level, new_level, true);
        }
    }
}

impl CreateHost for World {
    fn controlling_player(&self, obj: ObjectId) -> Option<PlayerIndex> {
        self.get_controlling_player(obj)
    }

    fn player_did_preorder(&self, player: PlayerIndex) -> bool {
        self.players.get_nth(player).is_some_and(|p| p.is_preorder)
    }

    fn player_has_science(&mut self, player: PlayerIndex, science: &[u8]) -> bool {
        // (the science store resolves the name, and the name key generator interns it)
        let statics = self.statics.clone();
        let Ok(t) = statics
            .science_store
            .get_science_from_internal_name(&mut self.names, science)
        else {
            return false;
        };
        self.players
            .get_nth(player)
            .is_some_and(|p| p.sciences.contains(&t))
    }

    fn set_model_condition_state(&mut self, obj: ObjectId, flag: ModelConditionFlagType) {
        World::set_model_condition_state(self, obj, flag);
    }

    fn clear_model_condition_state(&mut self, obj: ObjectId, flag: ModelConditionFlagType) {
        World::clear_model_condition_state(self, obj, flag);
    }

    fn set_weapon_lock(&mut self, obj: ObjectId, slot: i32, lock: WeaponLockType) {
        if let Some(o) = self.objects.find_mut(obj) {
            o.weapon_set.set_weapon_lock(slot, lock);
        }
    }

    fn special_power_creation(&mut self, obj: ObjectId) {
        for idx in self.modules_with(obj, MODULEINTERFACE_SPECIAL_POWER) {
            self.module_call(obj, idx, ModuleCall::OnSpecialPowerCreation);
        }
    }

    fn player_count(&self) -> Option<i32> {
        Some(self.players.player_count())
    }

    fn resource_add_supply_center(&mut self, index: PlayerIndex, obj: ObjectId) {
        if let Some(rgm) = self
            .players
            .get_nth_mut(index)
            .and_then(|p| p.resource_gathering_manager.as_mut())
        {
            rgm.add_supply_center(Some(obj));
        }
    }

    fn resource_add_supply_warehouse(&mut self, index: PlayerIndex, obj: ObjectId) {
        if let Some(rgm) = self
            .players
            .get_nth_mut(index)
            .and_then(|p| p.resource_gathering_manager.as_mut())
        {
            rgm.add_supply_warehouse(Some(obj));
        }
    }

    fn experience_is_trainable(&self, obj: ObjectId) -> Option<bool> {
        // every object owns a tracker for its whole life
        self.objects.find(obj).map(|o| o.template().is_trainable())
    }

    fn experience_set_min_veterancy_level(&mut self, obj: ObjectId, level: VeterancyLevel) {
        self.cut_experience_set_min_veterancy_level(obj, level);
    }

    fn find_upgrade(&mut self, name: &[u8]) -> Option<UpgradeRef> {
        let statics = self.statics.clone();
        match statics.upgrade_center.find_upgrade(&mut self.names, name) {
            Ok(Some(t)) => Some(UpgradeRef {
                id: t.upgrade_name_key().0,
                player_type: t.upgrade_type() == UpgradeType::Player,
            }),
            _ => None,
        }
    }

    fn player_add_upgrade_complete(&mut self, player: PlayerIndex, upgrade: UpgradeRef) {
        // `Player::addUpgrade` is layer 12's; its visible effect on the completed-upgrade mask is kept here
        let mask = self
            .statics
            .upgrade_center
            .find_upgrade_by_key(NameKeyType(upgrade.id))
            .map(|t| t.upgrade_mask());
        if let Some(mask) = mask {
            self.cuts
                .player_upgrades
                .entry(player)
                .or_default()
                .set_mask(&mask);
        }
        self.cut(CutCall::PlayerAddUpgradeComplete {
            player,
            upgrade: upgrade.id,
        });
    }

    fn object_give_upgrade(&mut self, obj: ObjectId, upgrade: UpgradeRef) {
        let mask = self
            .statics
            .upgrade_center
            .find_upgrade_by_key(NameKeyType(upgrade.id))
            .map(|t| t.upgrade_mask());
        self.give_upgrade(obj, mask.as_ref());
    }

    fn academy_record_upgrade(&mut self, player: PlayerIndex, upgrade: UpgradeRef, granted: bool) {
        let class = self
            .statics
            .upgrade_center
            .find_upgrade_by_key(NameKeyType(upgrade.id))
            .map(|t| t.academy_classification_type());
        if let (Some(class), Some(p)) = (class, self.players.get_nth_mut(player)) {
            p.academy_stats.record_upgrade(class, granted);
        }
        self.cut(CutCall::PlayerAcademyRecordUpgrade {
            player,
            upgrade: upgrade.id,
            granted,
        });
    }

    fn object_status_bits(&self, obj: ObjectId) -> ObjectStatusMaskType {
        self.objects.find(obj).map(|o| o.status).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
    use crate::common::model_state::MODELCONDITION_PREORDER;
    use crate::common::module::ModuleDataKind;
    use crate::common::name_key_generator::NameKeyGenerator;
    use crate::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION;
    use crate::common::science::ScienceType;
    use crate::common::thing_template::{ObjectTemplateValues, ThingTemplate};
    use crate::common::upgrade::UpgradeCenter;
    use crate::logic::client_event::ClientEvent;
    use crate::logic::module::create_module::CreateModuleData;
    use crate::logic::modules::grant_upgrade_create::GrantUpgradeCreateModuleData;
    use crate::logic::modules::new_module;
    use crate::logic::modules::veterancy_gain_create::VeterancyGainCreateModuleData;
    use crate::logic::world::object_create::NewObject;
    use crate::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
    use crate::logic::world::resource_gathering_manager::ResourceGatheringManager;
    use crate::logic::world::statics::GameStatics;
    use std::sync::Arc;

    fn block(ini: &mut Ini<'_>, c: &mut (UpgradeCenter, NameKeyGenerator)) -> IniResult<()> {
        c.0.parse_upgrade_definition(ini, &mut c.1)
    }

    /// A world with player 0 (team 1), two players, the upgrades `UpP` (a player upgrade) and `UpO` (an object upgrade), and
    /// one object made from `modules` (`(class, data)`), with a drawable if asked.
    fn world_with(
        modules: &[(&[u8], ModuleDataKind)],
        trainable: bool,
        drawable: bool,
    ) -> (World, ObjectId) {
        world_with_pre(modules, trainable, drawable, |_| {})
    }

    /// As [`world_with`], with `pre` run on the world before the object is made (its `onCreate` has run by the time the object is returned).
    fn world_with_pre(
        modules: &[(&[u8], ModuleDataKind)],
        trainable: bool,
        drawable: bool,
        pre: impl FnOnce(&mut World),
    ) -> (World, ObjectId) {
        let mut names = NameKeyGenerator::new();
        names.reset();
        let mut ctx = (UpgradeCenter::new(), names);
        let text = b"Upgrade UpP\n Type = PLAYER\nEnd\nUpgrade UpO\n Type = OBJECT\nEnd\n";
        let table = [BlockParse {
            token: "Upgrade",
            parse: block,
        }];
        Ini::load(
            b"u.ini",
            text,
            IniLoadType::Overwrite,
            None,
            &table,
            &mut ctx,
        )
        .unwrap();
        let mut w = World::new();
        w.statics = Arc::new(GameStatics {
            upgrade_center: ctx.0,
            ..Default::default()
        });
        w.names = ctx.1;
        w.cuts.neutral_default_team = 1;
        w.teams
            .insert_team(crate::logic::world::team::Team::new(1, 0))
            .unwrap();
        w.cuts.team_players.insert(1, 0);
        w.players.set_player_count(2);
        pre(&mut w);
        // a module that holds its object's id is made with the id the world is about to allocate
        let next = w.next_object_id();
        let built: Vec<_> = modules
            .iter()
            .enumerate()
            .map(|(i, (class, data))| {
                new_module(next, class, format!("ModuleTag_{i}").as_bytes(), data)
                    .unwrap()
                    .unwrap()
            })
            .collect();
        let v = ObjectTemplateValues {
            template_id: 7,
            is_trainable: trainable,
            experience_required: [0, 10, 20, 30],
            ..Default::default()
        };
        let mut status = ObjectStatusMaskType::default();
        status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 1);
        let id = w
            .new_object(NewObject {
                template: Arc::new(ThingTemplate::from_object_values(&v)),
                status,
                team: None,
                modules: built,
            })
            .unwrap();
        if drawable {
            w.objects.find_mut(id).unwrap().drawable = 1;
        }
        (w, id)
    }

    fn plain() -> ModuleDataKind {
        ModuleDataKind::Create(Arc::new(CreateModuleData::new()))
    }

    fn build_complete(w: &mut World, id: ObjectId) {
        for idx in w.modules_with(id, MODULEINTERFACE_CREATE) {
            w.module_call(id, idx, ModuleCall::OnBuildComplete);
        }
    }

    fn condition_events(w: &World) -> Vec<ModelConditionOp> {
        w.events
            .events()
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(ObjectClientEvent::DrawableModelCondition { op, .. }) => {
                    Some(op.clone())
                }
                _ => None,
            })
            .collect()
    }

    use crate::common::module::MODULEINTERFACE_CREATE;

    #[test]
    fn preorder_follows_the_players_flag_when_the_build_completes() {
        let (mut w, id) = world_with(&[(b"PreorderCreate", plain())], false, true);
        w.players.get_nth_mut(0).unwrap().is_preorder = true;
        build_complete(&mut w, id);

        w.players.get_nth_mut(0).unwrap().is_preorder = false;
        build_complete(&mut w, id);
        let ev = condition_events(&w);
        assert_eq!(
            ev[ev.len() - 2..],
            [
                ModelConditionOp::Set(vec![MODELCONDITION_PREORDER]),
                ModelConditionOp::Clear(vec![MODELCONDITION_PREORDER])
            ]
        );
        assert!(w.cuts.module_errors.is_empty());
    }

    #[test]
    fn an_object_with_no_controlling_player_is_reported_not_a_crash() {
        let (mut w, id) = world_with(&[(b"PreorderCreate", plain())], false, true);
        w.cuts.team_players.clear();
        build_complete(&mut w, id);
        assert_eq!(w.cuts.module_errors.len(), 1);
        assert!(w.cuts.module_errors[0].1.is_undefined());
    }

    #[test]
    fn supply_centers_and_warehouses_are_recorded_by_every_players_manager() {
        let (mut w, c) = world_with(&[(b"SupplyCenterCreate", plain())], false, false);
        // the managers exist after the object was made; a center registers itself when the build completes
        w.players.get_nth_mut(0).unwrap().resource_gathering_manager =
            Some(ResourceGatheringManager::new());
        w.players.get_nth_mut(1).unwrap().resource_gathering_manager =
            Some(ResourceGatheringManager::new());
        build_complete(&mut w, c);
        assert_eq!(
            w.players
                .get_nth(0)
                .unwrap()
                .resource_gathering_manager
                .as_ref()
                .unwrap()
                .supply_centers(),
            &[c]
        );
        assert_eq!(
            w.players
                .get_nth(1)
                .unwrap()
                .resource_gathering_manager
                .as_ref()
                .unwrap()
                .supply_centers(),
            &[c]
        );
    }

    #[test]
    fn warehouse_create_registers_with_the_managers_there_at_creation() {
        let mut w = World::new();
        w.cuts.neutral_default_team = 1;
        w.teams
            .insert_team(crate::logic::world::team::Team::new(1, 0))
            .unwrap();
        w.cuts.team_players.insert(1, 0);
        w.players.set_player_count(3);
        for i in 0..3 {
            w.players.get_nth_mut(i).unwrap().resource_gathering_manager =
                Some(ResourceGatheringManager::new());
        }
        let m = new_module(
            w.next_object_id(),
            b"SupplyWarehouseCreate",
            b"ModuleTag_W",
            &plain(),
        )
        .unwrap()
        .unwrap();
        let id = w
            .new_object(NewObject {
                template: Arc::new(ThingTemplate::new(0)),
                status: Default::default(),
                team: None,
                modules: vec![m],
            })
            .unwrap();
        for i in 0..3 {
            assert_eq!(
                w.players
                    .get_nth(i)
                    .unwrap()
                    .resource_gathering_manager
                    .as_ref()
                    .unwrap()
                    .supply_warehouses(),
                &[id]
            );
        }
    }

    #[test]
    fn veterancy_gain_needs_the_science_and_a_trainable_object() {
        let mut d = VeterancyGainCreateModuleData::new();
        d.starting_level = 2;
        d.science_required = Some(b"SCIENCE_Elite".to_vec());
        let data = ModuleDataKind::VeterancyGainCreate(Arc::new(d));
        let known = |w: &mut World| {
            let statics = w.statics.clone();
            let sci: ScienceType = statics
                .science_store
                .get_science_from_internal_name(&mut w.names, b"SCIENCE_Elite")
                .unwrap();
            w.players.get_nth_mut(0).unwrap().sciences.push(sci);
        };
        // not trainable: nothing, though the science is known
        let (w, id) = world_with_pre(
            &[(b"VeterancyGainCreate", data.clone())],
            false,
            false,
            known,
        );
        assert_eq!(
            w.objects.find(id).unwrap().experience.level,
            VeterancyLevel::Regular
        );
        // trainable, the player lacks the science: nothing
        let (w, id) = world_with(&[(b"VeterancyGainCreate", data.clone())], true, false);
        assert_eq!(
            w.objects.find(id).unwrap().experience.level,
            VeterancyLevel::Regular
        );
        // trainable and the science known: the level and its minimum experience (the tracker's table 0, 10, 20, 30)
        let (mut w, id) = world_with_pre(&[(b"VeterancyGainCreate", data)], true, false, known);
        let o = w.objects.find(id).unwrap();
        assert_eq!(
            (o.experience.level, o.experience.points),
            (VeterancyLevel::Elite, 20)
        );
        assert!(
            w.player_has_science(0, b"SCIENCE_Elite") && !w.player_has_science(1, b"SCIENCE_Elite")
        );
        // `setMinVeterancyLevel` never lowers the level
        w.cut_experience_set_min_veterancy_level(id, VeterancyLevel::Veteran);
        assert_eq!(
            w.objects.find(id).unwrap().experience.level,
            VeterancyLevel::Elite
        );
    }

    #[test]
    fn grant_upgrade_gives_a_player_upgrade_to_the_player_and_an_object_upgrade_to_the_object() {
        let mut p = GrantUpgradeCreateModuleData::new();
        p.upgrade_name = b"UpP".to_vec();
        let mut o = GrantUpgradeCreateModuleData::new();
        o.upgrade_name = b"UpO".to_vec();
        o.exempt_status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 1);
        let (mut w, id) = world_with(
            &[
                (
                    b"GrantUpgradeCreate",
                    ModuleDataKind::GrantUpgradeCreate(Arc::new(p)),
                ),
                (
                    b"GrantUpgradeCreate",
                    ModuleDataKind::GrantUpgradeCreate(Arc::new(o)),
                ),
            ],
            false,
            false,
        );
        w.cuts.log = Some(Vec::new());
        // onCreate already ran (the object was under construction: the UpO module is exempt only when not under construction)
        build_complete(&mut w, id);
        let statics = w.statics.clone();
        let up_p = statics
            .upgrade_center
            .find_upgrade(&mut w.names, b"UpP")
            .unwrap()
            .unwrap()
            .upgrade_mask();
        let up_o = statics
            .upgrade_center
            .find_upgrade(&mut w.names, b"UpO")
            .unwrap()
            .unwrap()
            .upgrade_mask();
        assert!(
            w.cuts.player_completed_upgrade_mask(0).test_for_all(&up_p),
            "the player has UpP"
        );
        assert!(
            w.objects
                .find(id)
                .unwrap()
                .object_upgrades_completed
                .test_for_all(&up_o),
            "the object has UpO"
        );
        assert!(w
            .cuts
            .log
            .as_ref()
            .unwrap()
            .iter()
            .any(|c| matches!(c, CutCall::PlayerAddUpgradeComplete { player: 0, .. })));
        // build complete runs once
        let n = w.cuts.log.as_ref().unwrap().len();
        build_complete(&mut w, id);
        assert_eq!(w.cuts.log.as_ref().unwrap().len(), n);
    }

    #[test]
    fn special_power_create_asks_each_special_power_module_and_lock_weapon_runs() {
        let mut lock = crate::logic::modules::lock_weapon_create::LockWeaponCreateModuleData::new();
        lock.slot_to_lock = 0;
        let (mut w, id) = world_with(
            &[
                (b"SpecialPowerCreate", plain()),
                (
                    b"LockWeaponCreate",
                    ModuleDataKind::LockWeaponCreate(Arc::new(lock)),
                ),
            ],
            false,
            false,
        );
        build_complete(&mut w, id);
        // the object has no weapon in the slot, so nothing is locked; the calls were made without a fault
        assert!(!w
            .objects
            .find(id)
            .unwrap()
            .weapon_set
            .is_cur_weapon_locked());
        assert!(w.cuts.module_errors.is_empty() && w.cuts.reentrant_skips.is_empty());
        for idx in w.modules_with(id, MODULEINTERFACE_CREATE) {
            assert!(
                w.module_call(id, idx, ModuleCall::IsAlreadyUpgraded)
                    == crate::logic::world::module_call::ModuleReply::Unhandled
            );
        }
    }

    #[test]
    fn the_factory_knows_the_seven_and_each_save_is_the_whole_chain() {
        use crate::common::module::{Module, MODULEINTERFACE_CREATE as CREATE};
        use crate::common::module_factory::translated_module;
        for name in [
            "PreorderCreate",
            "SpecialPowerCreate",
            "SupplyCenterCreate",
            "SupplyWarehouseCreate",
            "LockWeaponCreate",
            "VeterancyGainCreate",
            "GrantUpgradeCreate",
        ] {
            let t = translated_module(name).unwrap_or_else(|| panic!("{name} is translated"));
            assert_eq!(t.which_interfaces, Some(CREATE));
            let data = (t.create_data_proc.unwrap())(None).unwrap();
            let mut base = match (t.create_proc.unwrap())(5, Some(data.clone())).unwrap() {
                Module::Create(c) => c,
                _ => panic!("{name}: a CreateModule base"),
            };
            // the module's xfer: its version (1), then CreateModule's chain (its version, the behaviour chain, the flag)
            let mut body = new_module(5, name.as_bytes(), b"ModuleTag_X", &data)
                .unwrap()
                .unwrap()
                .body
                .unwrap();
            assert!(body.owns_base_chain());
            let mut mine = crate::logic::module::test_support::save_bytes(&mut body).unwrap();
            let theirs = crate::logic::module::test_support::save_bytes(&mut base).unwrap();
            assert_eq!(mine.remove(0), 1);
            assert_eq!(mine, theirs, "{name}");
        }
    }

    #[test]
    fn the_data_classes_read_their_fields() {
        use crate::common::module::ModuleDataKind as K;
        fn read(class: &str, text: &str) -> K {
            let t = crate::common::module_factory::translated_module(class).unwrap();
            let src = format!("D\n{text}\nEnd\n");
            struct Slot(Option<K>, fn(Option<&mut Ini<'_>>) -> IniResult<K>);
            fn blk(ini: &mut Ini<'_>, s: &mut Slot) -> IniResult<()> {
                s.0 = Some((s.1)(Some(ini))?);
                Ok(())
            }
            let table = [BlockParse {
                token: "D",
                parse: blk,
            }];
            let mut slot = Slot(None, t.create_data_proc.unwrap());
            Ini::load(
                b"d.ini",
                src.as_bytes(),
                IniLoadType::Overwrite,
                None,
                &table,
                &mut slot,
            )
            .unwrap();
            slot.0.unwrap()
        }
        match read(
            "GrantUpgradeCreate",
            "UpgradeToGrant = UpP\n ExemptStatus = UNDER_CONSTRUCTION",
        ) {
            K::GrantUpgradeCreate(d) => {
                assert_eq!(d.upgrade_name, b"UpP");
                assert!(d.exempt_status.test(OBJECT_STATUS_UNDER_CONSTRUCTION));
            }
            _ => panic!(),
        }
        match read(
            "VeterancyGainCreate",
            "StartingLevel = ELITE\n ScienceRequired = SCIENCE_X",
        ) {
            K::VeterancyGainCreate(d) => assert_eq!(
                (d.starting_level, d.science_required.clone()),
                (2, Some(b"SCIENCE_X".to_vec()))
            ),
            _ => panic!(),
        }
        match read("LockWeaponCreate", "SlotToLock = SECONDARY") {
            K::LockWeaponCreate(d) => assert_eq!(d.slot_to_lock, 1),
            _ => panic!(),
        }
        assert!(matches!(read("PreorderCreate", ""), K::Create(_)));
    }
}
