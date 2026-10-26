// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file (this repository's own design, not upstream code):
// the world's answers to the calls the layer-9 helper and die modules make.

//! [`HelperHost`] and [`DieHost`] for the [`World`]: what the object's
//! helpers (`ObjectSMCHelper`, `ObjectRepulsorHelper`,
//! `ObjectWeaponStatusHelper`, `StatusDamageHelper`, `TempWeaponBonusHelper`)
//! and the die modules (`DestroyDie`, `KeepObjectDie`, `UpgradeDie`) ask of
//! the object and the game logic. As for the update modules
//! ([`update_host_world`](crate::logic::world::update_host_world)), the world
//! answers for **the module that is running**, the top of `cuts.module_stack`.

use std::sync::Arc;

use crate::common::game_type::ObjectId;
use crate::common::object_status_types::{ObjectStatusMaskType, ObjectStatusTypes};
use crate::logic::module::update_module::{UpdateModule, UpdateModuleData};
use crate::logic::modules::object_repulsor_helper::ObjectRepulsorHelper;
use crate::logic::modules::object_smc_helper::ObjectSMCHelper;
use crate::logic::modules::object_defection_helper::ObjectDefectionHelper;
use crate::logic::modules::subdual_damage_helper::SubdualDamageHelper;
use crate::logic::modules::object_weapon_status_helper::ObjectWeaponStatusHelper;
use crate::logic::modules::status_damage_helper::StatusDamageHelper;
use crate::logic::modules::temp_weapon_bonus_helper::TempWeaponBonusHelper;
use crate::logic::world::object::HelperKind;
use crate::logic::world::object_create::NewModule;
use crate::logic::world::update_body::UpdateBody;
use crate::common::upgrade::UpgradeMaskType;
use crate::logic::modules::object_helper::HelperHost;
use crate::logic::modules::{DieHost, UpdateHost};
use crate::logic::world::game_logic::World;
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::object_state::mask_of;

impl HelperHost for World {
    fn frame(&self) -> u32 {
        World::frame(self)
    }

    fn awaken_update_module(&mut self, frame: u32) {
        UpdateHost::awaken_update_module(self, frame);
    }

    fn is_destroyed(&self) -> bool {
        self.objects.find(self.running_object()).is_some_and(|o| o.is_destroyed())
    }

    fn set_status(&mut self, status: ObjectStatusTypes) {
        let id = self.running_object();
        World::set_status(self, id, mask_of(status), true);
    }

    fn clear_status(&mut self, status: ObjectStatusTypes) {
        let id = self.running_object();
        World::clear_status(self, id, mask_of(status));
    }

    fn clear_special_model_condition_states(&mut self) {
        let id = self.running_object();
        World::clear_special_model_condition_states(self, id);
    }

    fn adjust_model_condition_for_weapon_status(&mut self) {
        let object = self.running_object();
        self.cut(CutCall::AdjustModelConditionForWeaponStatus { object });
    }

    fn set_weapon_bonus_condition(&mut self, bonus: i32) {
        let id = self.running_object();
        World::set_weapon_bonus_condition(self, id, bonus);
    }

    fn clear_weapon_bonus_condition(&mut self, bonus: i32) {
        let id = self.running_object();
        World::clear_weapon_bonus_condition(self, id, bonus);
    }

    fn has_drawable(&self) -> bool {
        World::has_drawable(self, self.running_object())
    }

    fn set_tint_status(&mut self, tint: i32) {
        let object_id = self.running_object();
        self.emit(ObjectClientEvent::DrawableTint { object_id, status: tint, on: true });
    }

    fn clear_tint_status(&mut self, tint: i32) {
        let object_id = self.running_object();
        self.emit(ObjectClientEvent::DrawableTint { object_id, status: tint, on: false });
    }
}

impl World {
    /// Makes the helper of that kind for object `id` (`Object::Object`'s `newInstance(ObjectSMCHelper)(this, &data)` and the
    /// others): ~~the five helpers that are translated are real modules, the rest (`SubdualDamageHelper`, `ObjectDefectionHelper`,
    /// `FiringTracker`)~~ **CORRECTED 2026-10-21** (`p1w11-sonnet`): seven are real modules, only `FiringTracker` are the stand-in of [`ObjectCuts::make_helper`](crate::logic::world::object_cuts::ObjectCuts::make_helper)
    /// until they are. **CORRECTED 2026-10-26** (batch R78): all eight are real modules; the stand-ins are only for `cuts.stub_helpers`.
    pub(crate) fn make_helper_module(&self, id: ObjectId, kind: HelperKind) -> NewModule {
        let data = Some(Arc::new(UpdateModuleData::new()));
        if self.cuts.stub_helpers {
            return self.cuts.make_helper(kind);
        }
        let made: Option<(UpdateModule, UpdateBody)> = match kind {
            HelperKind::Smc => ObjectSMCHelper::unconstructed(id, data).ok().map(|h| (h.helper().update_module().clone(), UpdateBody::ObjectSMCHelper(h))),
            HelperKind::Repulsor => {
                ObjectRepulsorHelper::unconstructed(id, data).ok().map(|h| (h.helper().update_module().clone(), UpdateBody::ObjectRepulsorHelper(h)))
            }
            HelperKind::WeaponStatus => {
                ObjectWeaponStatusHelper::unconstructed(id, data).ok().map(|h| (h.helper().update_module().clone(), UpdateBody::ObjectWeaponStatusHelper(h)))
            }
            HelperKind::StatusDamage => {
                StatusDamageHelper::unconstructed(id, data).ok().map(|h| (h.helper().update_module().clone(), UpdateBody::StatusDamageHelper(h)))
            }
            HelperKind::TempWeaponBonus => {
                TempWeaponBonusHelper::unconstructed(id, data).ok().map(|h| (h.helper().update_module().clone(), UpdateBody::TempWeaponBonusHelper(h)))
            }
            // --- p1w11-sonnet ---
            HelperKind::SubdualDamage => {
                SubdualDamageHelper::unconstructed(id, data).ok().map(|h| (h.helper().update_module().clone(), UpdateBody::SubdualDamageHelper(h)))
            }
            HelperKind::Defection => {
                ObjectDefectionHelper::unconstructed(id, data).ok().map(|h| (h.helper().update_module().clone(), UpdateBody::ObjectDefectionHelper(h)))
            }
            // --- end p1w11-sonnet ---
            // batch R78
            HelperKind::FiringTracker => crate::logic::modules::firing_tracker_module::FiringTrackerModule::unconstructed(id, data)
                .ok()
                .map(|h| (h.update_module().clone(), UpdateBody::FiringTracker(h))),
        };
        let mut m = self.cuts.make_helper(kind);
        if let Some((update, body)) = made {
            m.update = Some(update);
            m.body = Some(body);
        }
        m
    }

    /// The completed-upgrade mask of the upgrade `name`, if the upgrade center has it.
    fn named_upgrade_mask(&mut self, name: &[u8]) -> Option<UpgradeMaskType> {
        let statics = self.statics.clone();
        match statics.upgrade_center.find_upgrade(&mut self.names, name) {
            Ok(Some(t)) => Some(t.upgrade_mask()),
            _ => None,
        }
    }
}

impl DieHost for World {
    fn veterancy_level(&self) -> i32 {
        self.objects.find(self.running_object()).map_or(0, |o| o.get_veterancy_level() as i32)
    }

    fn status_bits(&self) -> ObjectStatusMaskType {
        self.objects.find(self.running_object()).map(|o| o.status).unwrap_or_default()
    }

    fn destroy_object(&mut self) {
        let id = self.running_object();
        World::destroy_object(self, id);
    }

    fn producer_exists(&self) -> bool {
        self.objects.find(self.running_object()).is_some_and(|o| self.objects.find(o.producer_id).is_some())
    }

    fn find_upgrade(&mut self, name: &[u8]) -> bool {
        self.named_upgrade_mask(name).is_some()
    }

    fn producer_has_upgrade(&mut self, name: &[u8]) -> bool {
        // `hasUpgrade`: the producer's completed upgrades contain every bit of the template's mask
        let Some(mask) = self.named_upgrade_mask(name) else { return false };
        let Some(producer) = self.objects.find(self.running_object()).map(|o| o.producer_id) else { return false };
        self.objects.find(producer).is_some_and(|p| p.object_upgrades_completed.test_for_all(&mask))
    }

    fn producer_remove_upgrade(&mut self, name: &[u8]) {
        let Some(mask) = self.named_upgrade_mask(name) else { return };
        let Some(producer) = self.objects.find(self.running_object()).map(|o| o.producer_id) else { return };
        self.remove_upgrade(producer, &mask);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
    use crate::common::module::{ModuleDataKind, MODULEINTERFACE_DIE};
    use crate::common::name_key_generator::NameKeyGenerator;
    use crate::common::object_status_types::{OBJECT_STATUS_AFLAME, OBJECT_STATUS_WET};
    use crate::common::thing_template::{ObjectTemplateValues, ThingTemplate};
    use crate::common::upgrade::UpgradeCenter;
    use crate::common::xfer_load::XferLoad;
    use crate::common::xfer_save::XferSave;
    use crate::common::xfer::Xfer;
    use crate::logic::client_event::ClientEvent;
    use crate::logic::damage::{DamageInfo, DAMAGE_UNRESISTABLE};
    use crate::logic::module::update_module::UPDATE_SLEEP_FOREVER;
    use crate::logic::modules::new_module;
    use crate::logic::weapon::WeaponStore;
    use crate::logic::world::module_call::ModuleHandle;
    use crate::logic::world::module_probe::{ProbeAnswers, ProbeModule};
    use crate::logic::world::object_create::NewObject;
    use crate::logic::world::object_xfer::ObjectXferStores;
    use crate::common::module::MODULEINTERFACE_BODY;
    use crate::common::thing_factory::ThingFactory;

    fn base_world() -> World {
        let mut w = World::new();
        w.cuts.neutral_default_team = 1;
        w.teams.insert_team(crate::logic::world::team::Team::new(1, 0)).unwrap();
        w.cuts.team_players.insert(1, 0);
        w.cuts.log = Some(Vec::new());
        w
    }

    fn probe_body(max: f32) -> NewModule {
        let mut m = NewModule::plain(b"ProbeBody", b"ModuleTag_Body");
        m.interfaces = MODULEINTERFACE_BODY | MODULEINTERFACE_DIE;
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), max, Default::default())));
        m
    }

    fn make(w: &mut World, modules: Vec<NewModule>) -> ObjectId {
        let v = ObjectTemplateValues { template_id: 5, ..Default::default() };
        w.new_object(NewObject { template: Arc::new(ThingTemplate::from_object_values(&v)), status: Default::default(), team: None, modules }).unwrap()
    }

    /// An object whose template can have a weapon, so that it has the weapon helpers as well.
    fn make_armed(w: &mut World) -> ObjectId {
        let v = ObjectTemplateValues { template_id: 6, weapon_sets: vec![(vec![], true)], ..Default::default() };
        w.new_object(NewObject { template: Arc::new(ThingTemplate::from_object_values(&v)), status: Default::default(), team: None, modules: vec![] }).unwrap()
    }

    fn next_call(w: &World, id: ObjectId, class: &[u8]) -> u32 {
        let idx = w.find_module(id, class).unwrap();
        let u = w.update_handle_of_module(ModuleHandle { object: id, index: idx }).unwrap();
        w.objects.update_module(u).unwrap().friend_get_next_call_frame()
    }

    #[test]
    fn an_object_has_its_helpers_asleep_unless_stubbed() {
        let mut w = base_world();
        let id = make(&mut w, vec![]);
        let o = w.objects.find(id).unwrap();
        // no weapon in the template: the SMC, status damage, subdual damage and defection helpers
        // (p1w11-sonnet: `SubdualDamageHelper` and `ObjectDefectionHelper` are real modules now)
        for class in [&b"ObjectSMCHelper"[..], b"StatusDamageHelper", b"SubdualDamageHelper", b"ObjectDefectionHelper"] {
            let idx = w.find_module(id, class).unwrap();
            assert!(o.modules[idx as usize].update_index.is_some(), "{}", String::from_utf8_lossy(class));
            assert_eq!(next_call(&w, id, class), UPDATE_SLEEP_FOREVER.0, "asleep: {}", String::from_utf8_lossy(class));
        }
        // batch R78: `FiringTracker` is a real module too, asleep until a shot reaches it
        let armed = make_armed(&mut w);
        let idx = w.find_module(armed, b"FiringTracker").unwrap();
        assert!(w.objects.find(armed).unwrap().modules[idx as usize].update_index.is_some(), "FiringTracker");
        assert_eq!(next_call(&w, armed, b"FiringTracker"), UPDATE_SLEEP_FOREVER.0, "asleep: FiringTracker");
        assert!(crate::logic::world::object_weapons::ObjectWeaponHost::firing_tracker_shot_fired(&mut w, armed, 0, 5));
        assert_eq!(w.module_call(armed, idx, crate::logic::world::module_call::ModuleCall::FiringTrackerGetLastShotVictim).as_id(), 5);
        // two more at the same victim: the weapon code reads the count from the tracker
        for _ in 0..2 {
            crate::logic::world::object_weapons::ObjectWeaponHost::firing_tracker_shot_fired(&mut w, armed, 0, 5);
        }
        assert_eq!(crate::logic::weapon_host::WeaponHost::num_consecutive_shots_fired_at_target(&mut w, armed, Some(5)), 3);
        assert_eq!(crate::logic::weapon_host::WeaponHost::num_consecutive_shots_fired_at_target(&mut w, armed, Some(6)), 0);
        // the ablation is explicit
        let mut w = base_world();
        w.cuts.stub_helpers = true;
        let id = make(&mut w, vec![]);
        let idx = w.find_module(id, b"ObjectSMCHelper").unwrap();
        assert!(w.objects.find(id).unwrap().modules[idx as usize].update_index.is_none());
    }

    #[test]
    fn status_damage_sets_the_status_and_clears_it_when_the_time_is_up() {
        let mut w = base_world();
        let id = make(&mut w, vec![]);
        w.do_status_damage(id, OBJECT_STATUS_AFLAME, 4.9);
        assert!(w.objects.find(id).unwrap().test_status(OBJECT_STATUS_AFLAME));
        assert_eq!(next_call(&w, id, b"StatusDamageHelper"), 4, "4.9 frames is 4");
        for _ in 0..4 {
            w.run_logic_frame();
            assert!(w.objects.find(id).unwrap().test_status(OBJECT_STATUS_AFLAME));
        }
        w.run_logic_frame(); // frame 4
        assert!(!w.objects.find(id).unwrap().test_status(OBJECT_STATUS_AFLAME));
        assert_eq!(next_call(&w, id, b"StatusDamageHelper"), UPDATE_SLEEP_FOREVER.0);
        // a different status first clears the old one
        w.do_status_damage(id, OBJECT_STATUS_AFLAME, 50.0);
        w.do_status_damage(id, OBJECT_STATUS_WET, 50.0);
        let o = w.objects.find(id).unwrap();
        assert!(o.test_status(OBJECT_STATUS_WET) && !o.test_status(OBJECT_STATUS_AFLAME));
        assert!(w.cuts.module_errors.is_empty() && w.cuts.reentrant_skips.is_empty());
    }

    #[test]
    fn temp_weapon_bonus_sets_and_clears_the_condition_and_tints_a_drawable() {
        let mut w = base_world();
        let id = make_armed(&mut w);
        w.objects.find_mut(id).unwrap().drawable = 1;
        w.do_temp_weapon_bonus(id, 3, 6);
        assert!(w.objects.find(id).unwrap().test_weapon_bonus_condition(3));
        let tint = |w: &World| -> Vec<bool> {
            w.events
                .events()
                .iter()
                .filter_map(|e| match e {
                    ClientEvent::Object(ObjectClientEvent::DrawableTint { status: 0x10, on, .. }) => Some(*on),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(tint(&w), vec![true]);
        for _ in 0..7 {
            w.run_logic_frame();
        }
        assert!(!w.objects.find(id).unwrap().test_weapon_bonus_condition(3));
        assert_eq!(tint(&w), vec![true, false]);
    }

    #[test]
    fn the_smc_helper_clears_the_special_condition_when_its_time_comes() {
        let mut w = base_world();
        let id = make(&mut w, vec![]);
        w.objects.find_mut(id).unwrap().drawable = 1;
        w.set_special_model_condition_state(id, crate::common::model_state::MODELCONDITION_SPECIAL_CHEERING, 5);
        assert_eq!(w.objects.find(id).unwrap().smc_until, 5);
        assert_eq!(next_call(&w, id, b"ObjectSMCHelper"), 5);
        for _ in 0..6 {
            w.run_logic_frame();
        }
        assert_eq!(w.objects.find(id).unwrap().smc_until, crate::common::game_common::NEVER, "cleared by the helper's update");
        assert_eq!(next_call(&w, id, b"ObjectSMCHelper"), UPDATE_SLEEP_FOREVER.0);
    }

    #[test]
    fn the_weapon_status_helper_runs_every_frame_in_the_final_phase() {
        let mut w = base_world();
        let id = make_armed(&mut w);
        assert_eq!(next_call(&w, id, b"ObjectWeaponStatusHelper"), 1, "asleep it is not: due next frame");
        for _ in 0..4 {
            w.run_logic_frame(); // frames 0 to 3: the helper runs on 1, 2 and 3
        }
        let n = w.cuts.log.as_ref().unwrap().iter().filter(|c| matches!(c, CutCall::AdjustModelConditionForWeaponStatus { .. })).count();
        assert_eq!(n, 3, "one adjustment per frame");
        let idx = w.find_module(id, b"ObjectWeaponStatusHelper").unwrap();
        let u = w.update_handle_of_module(ModuleHandle { object: id, index: idx }).unwrap();
        assert_eq!(w.objects.update_module(u).unwrap().friend_get_next_call_phase(), crate::logic::module::update_module::SleepyUpdatePhase::Final);
    }

    fn die_data(class: &str, text: &str) -> ModuleDataKind {
        struct Slot(Option<ModuleDataKind>, fn(Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind>);
        fn blk(ini: &mut Ini<'_>, s: &mut Slot) -> IniResult<()> {
            s.0 = Some((s.1)(Some(ini))?);
            Ok(())
        }
        let t = crate::common::module_factory::translated_module(class).expect("translated");
        let src = format!("D\n{text}\nEnd\n");
        let table = [BlockParse { token: "D", parse: blk }];
        let mut slot = Slot(None, t.create_data_proc.unwrap());
        Ini::load(b"d.ini", src.as_bytes(), IniLoadType::Overwrite, None, &table, &mut slot).unwrap();
        slot.0.unwrap()
    }

    fn kill(w: &mut World, id: ObjectId) {
        w.kill(id, DAMAGE_UNRESISTABLE, 0);
    }

    #[test]
    fn destroy_die_destroys_the_object_when_it_dies_and_keep_object_die_does_not() {
        let mut w = base_world();
        let next = w.next_object_id();
        let destroy = new_module(next, b"DestroyDie", b"ModuleTag_D", &die_data("DestroyDie", "")).unwrap().unwrap();
        let a = make(&mut w, vec![probe_body(10.0), destroy]);
        kill(&mut w, a);
        assert!(w.objects.find(a).unwrap().is_destroyed());
        assert!(w.cuts.reentrant_skips.is_empty(), "{:?}", w.cuts.reentrant_skips);

        let next = w.next_object_id();
        let keep = new_module(next, b"KeepObjectDie", b"ModuleTag_K", &die_data("KeepObjectDie", "")).unwrap().unwrap();
        let b = make(&mut w, vec![probe_body(10.0), keep]);
        kill(&mut w, b);
        assert!(!w.objects.find(b).unwrap().is_destroyed());

        // the die mux decides: a death type the module is not for
        let next = w.next_object_id();
        let picky = new_module(next, b"DestroyDie", b"ModuleTag_D", &die_data("DestroyDie", "DeathTypes = NONE +CRUSHED")).unwrap().unwrap();
        let c = make(&mut w, vec![probe_body(10.0), picky]);
        kill(&mut w, c);
        assert!(!w.objects.find(c).unwrap().is_destroyed(), "DEATH_NORMAL is not CRUSHED");
    }

    #[test]
    fn upgrade_die_takes_the_upgrade_from_the_producer() {
        let mut names = NameKeyGenerator::new();
        names.reset();
        let mut ctx = (UpgradeCenter::new(), names);
        fn blk(ini: &mut Ini<'_>, c: &mut (UpgradeCenter, NameKeyGenerator)) -> IniResult<()> {
            c.0.parse_upgrade_definition(ini, &mut c.1)
        }
        Ini::load(b"u.ini", b"Upgrade UpA\nEnd\nUpgrade UpB\nEnd\n", IniLoadType::Overwrite, None, &[BlockParse { token: "Upgrade", parse: blk }], &mut ctx).unwrap();
        let mut w = base_world();
        w.statics = Arc::new(crate::logic::world::statics::GameStatics { upgrade_center: ctx.0, ..Default::default() });
        w.names = ctx.1;
        let producer = make(&mut w, vec![]);
        let statics = w.statics.clone();
        let up_a = statics.upgrade_center.find_upgrade(&mut w.names, b"UpA").unwrap().unwrap().upgrade_mask();
        let up_b = statics.upgrade_center.find_upgrade(&mut w.names, b"UpB").unwrap().unwrap().upgrade_mask();
        w.objects.find_mut(producer).unwrap().object_upgrades_completed = {
            let mut m = up_a;
            m.set_mask(&up_b);
            m
        };
        let next = w.next_object_id();
        let die = new_module(next, b"UpgradeDie", b"ModuleTag_U", &die_data("UpgradeDie", "UpgradeToRemove = UpA")).unwrap().unwrap();
        let child = make(&mut w, vec![probe_body(10.0), die]);
        w.objects.find_mut(child).unwrap().producer_id = producer;
        kill(&mut w, child);
        let left = w.objects.find(producer).unwrap().object_upgrades_completed;
        assert!(!left.test_for_all(&up_a) && left.test_for_all(&up_b), "UpA is gone, UpB stays");
        assert!(w.cuts.module_errors.is_empty() && w.cuts.reentrant_skips.is_empty());
        let _ = DamageInfo::default();
    }

    #[test]
    fn the_factory_knows_the_three_dies_and_an_object_with_helpers_saves_and_loads() {
        use crate::common::module::{Module, MODULEINTERFACE_DIE as DIE};
        use crate::common::module_factory::translated_module;
        for name in ["DestroyDie", "KeepObjectDie", "UpgradeDie"] {
            let t = translated_module(name).unwrap();
            assert_eq!(t.which_interfaces, Some(DIE));
            let data = (t.create_data_proc.unwrap())(None).unwrap();
            let mut base = match (t.create_proc.unwrap())(5, Some(data.clone())).unwrap() {
                Module::Die(d) => d,
                _ => panic!("{name}: a DieModule base"),
            };
            let mut body = new_module(5, name.as_bytes(), b"ModuleTag_X", &data).unwrap().unwrap().body.unwrap();
            let mut mine = crate::logic::module::test_support::save_bytes(&mut body).unwrap();
            let theirs = crate::logic::module::test_support::save_bytes(&mut base).unwrap();
            assert_eq!(mine.remove(0), 1);
            assert_eq!(mine, theirs, "{name}");
        }
        // an object with its helpers (and a status in progress) round trips through xfer
        let mut names = NameKeyGenerator::new();
        let uc = UpgradeCenter::new();
        let mut w = base_world();
        let id = make(&mut w, vec![]);
        w.do_status_damage(id, OBJECT_STATUS_AFLAME, 40.0);
        w.do_temp_weapon_bonus(id, 2, 33);
        w.run_logic_frame();
        let save = |w: &mut World, names: &mut NameKeyGenerator| {
            let mut x = XferSave::new();
            x.open(b"object").unwrap();
            let mut stores = ObjectXferStores { upgrade_center: &uc, names, factory: &ThingFactory::new(), weapon_store: &WeaponStore::new() };
            w.object_xfer(id, &mut x, &mut stores).expect("save");
            x.close().unwrap();
            x.take_bytes()
        };
        let bytes = save(&mut w, &mut names);
        let mut w2 = base_world();
        let id2 = make(&mut w2, vec![]);
        assert_eq!(id, id2);
        let mut x = XferLoad::new(bytes.clone());
        x.open(b"object").unwrap();
        let mut stores = ObjectXferStores { upgrade_center: &uc, names: &mut names, factory: &ThingFactory::new(), weapon_store: &WeaponStore::new() };
        w2.object_xfer(id2, &mut x, &mut stores).expect("load");
        x.close().unwrap();
        w2.object_load_post_process(id2);
        let sd = w2.find_module(id2, b"StatusDamageHelper").unwrap();
        match w2.take_module_body(ModuleHandle { object: id2, index: sd }) {
            Some(UpdateBody::StatusDamageHelper(h)) => assert_eq!((h.status_to_heal, h.frame_to_heal), (OBJECT_STATUS_AFLAME, 40)),
            other => panic!("{other:?}"),
        }
    }
}
