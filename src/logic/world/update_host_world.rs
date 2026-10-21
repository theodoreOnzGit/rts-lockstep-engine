// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file (this repository's own design, not upstream code):
// the world's answers to the calls the layer-9 update modules make.

//! [`UpdateHost`], [`BaseRegenHost`] and [`SmartBombHost`] for the [`World`]:
//! what `LifetimeUpdate`, `DeletionUpdate`, `BaseRegenerateUpdate` and
//! `SmartBombTargetHomingUpdate` ask of the game logic and of their object.
//!
//! A host call has no object argument (upstream's `getObject()` is the
//! module's own), so the world answers for **the module that is running**: the
//! top of `cuts.module_stack`. Every entry point of a wired module pushes its
//! handle there before it calls the module ([`World::in_update_module`] for the
//! scheduler's `update()`, `World::module_call` for an interface call, the
//! constructor hook for `construct`). A call made with the stack empty has no
//! object to act on and does nothing (it answers the harmless default).

use crate::common::base_type::Coord3D;
use crate::common::game_type::ObjectId;
use crate::common::kind_of::KINDOF_HULK;
use crate::common::object_status_types::{OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION};
use crate::logic::damage::{DAMAGE_UNRESISTABLE, DEATH_NORMAL};
use crate::logic::modules::base_regenerate_update::BaseRegenHost;
use crate::logic::modules::smart_bomb_target_homing_update::SmartBombHost;
use crate::logic::modules::UpdateHost;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle};
use crate::logic::world::sleepy::UpdateHandle;

impl World {
    /// The `UpdateHandle` (position in `Object::behaviors`) of the update module that is module `h` of its object.
    pub fn update_handle_of_module(&self, h: ModuleHandle) -> Option<UpdateHandle> {
        let slot = self.objects.find(h.object)?.modules.get(h.index as usize)?;
        slot.update_index.map(|index| UpdateHandle { object: h.object, index })
    }

    /// The module handle (position in `Object::modules`) of the update module `u`.
    pub fn module_handle_of_update(&self, u: UpdateHandle) -> Option<ModuleHandle> {
        let o = self.objects.find(u.object)?;
        o.modules.iter().position(|m| m.update_index == Some(u.index)).map(|i| ModuleHandle { object: u.object, index: i as u16 })
    }

    /// Runs `f` as the update module `me` (the scheduler's `update()`): `me` is the running module for the host calls
    /// ([`UpdateHost`] and the rest), and an interface call to it from inside `f` is a re-entrant one.
    pub fn in_update_module<R>(&mut self, me: UpdateHandle, f: impl FnOnce(&mut World) -> R) -> R {
        let h = self.module_handle_of_update(me);
        if let Some(h) = h {
            self.cuts.module_stack.push(h);
        }
        let r = f(self);
        if h.is_some() {
            self.cuts.module_stack.pop();
        }
        r
    }

    /// Runs the world half of the constructor of module `idx` of object `id` (`UpdateBody::construct`), as the module that is
    /// running. `add_module` calls it for every module in module order, as upstream's constructors run during `Object::Object`.
    pub(crate) fn construct_module(&mut self, id: ObjectId, idx: u16) {
        let h = ModuleHandle { object: id, index: idx };
        let Some(mut body) = self.take_module_body(h) else { return };
        self.cuts.module_stack.push(h);
        body.construct(self, h);
        self.cuts.module_stack.pop();
        self.put_module_body(h, body);
    }

    /// The module that is running (the top of the stack), if any.
    fn running_module(&self) -> Option<ModuleHandle> {
        self.cuts.module_stack.last().copied()
    }

    /// The object of the module that is running; `INVALID_ID` with none.
    pub(crate) fn running_object(&self) -> ObjectId {
        self.running_module().map_or(crate::common::game_type::INVALID_ID, |h| h.object)
    }
}

impl UpdateHost for World {
    fn frame(&self) -> u32 {
        World::frame(self)
    }

    fn is_kind_of_hulk(&self) -> bool {
        self.objects.find(self.running_object()).is_some_and(|o| o.template().is_kind_of(KINDOF_HULK))
    }

    fn hulk_max_lifetime_override(&self) -> i32 {
        self.cuts.globals.hulk_max_lifetime_override
    }

    fn awaken_update_module(&mut self, frame: u32) {
        let Some(u) = self.running_module().and_then(|h| self.update_handle_of_module(h)) else { return };
        // a defect of the sleepy list (an illegal index) is the list's own check; the module has nothing to do about it
        let _ = World::awaken_update_module(self, u.object, u, frame);
    }

    fn kill_object(&mut self) {
        let id = self.running_object();
        self.kill(id, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
    }

    fn destroy_object(&mut self) {
        let id = self.running_object();
        World::destroy_object(self, id);
    }
}

impl BaseRegenHost for World {
    fn base_regen_health_percent_per_second(&self) -> f32 {
        self.cuts.globals.base_regen_health_percent_per_second
    }

    fn base_regen_delay(&self) -> u32 {
        self.cuts.globals.base_regen_delay
    }

    fn is_under_construction(&self) -> bool {
        self.objects.find(self.running_object()).is_some_and(|o| o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION))
    }

    fn is_sold(&self) -> bool {
        self.objects.find(self.running_object()).is_some_and(|o| o.test_status(OBJECT_STATUS_SOLD))
    }

    fn max_health(&mut self) -> f32 {
        let id = self.running_object();
        match self.get_body_module(id) {
            Some(body) => self.module_call(id, body, ModuleCall::GetMaxHealth).as_real(0.0),
            None => 0.0,
        }
    }

    fn health(&mut self) -> f32 {
        let id = self.running_object();
        match self.get_body_module(id) {
            Some(body) => self.module_call(id, body, ModuleCall::GetHealth).as_real(0.0),
            None => 0.0,
        }
    }

    fn attempt_healing(&mut self, amount: f32) {
        let id = self.running_object();
        World::attempt_healing(self, id, amount, id);
    }
}

impl SmartBombHost for World {
    fn is_significantly_above_terrain(&mut self) -> bool {
        let id = self.running_object();
        // `-(3*3)*TheGlobalData->m_gravity`, as `Thing::isSignificantlyAboveTerrain`
        let threshold = (-(3 * 3)) as f32 * self.cuts.globals.gravity;
        self.get_height_above_terrain(id) > threshold
    }

    fn position(&self) -> Coord3D {
        self.objects.find(self.running_object()).map_or_else(Coord3D::default, |o| *o.thing.position())
    }

    fn set_position(&mut self, pos: Coord3D) {
        let id = self.running_object();
        World::set_position(self, id, &pos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::base_type::Coord3D;
    use crate::common::game_type::INVALID_ID;
    use crate::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
    use crate::common::module::{ModuleDataKind, MODULEINTERFACE_BODY, MODULEINTERFACE_DAMAGE};
    use crate::common::thing_factory::ThingFactory;
    use crate::common::thing_template::{ObjectTemplateValues, ThingTemplate};
    use crate::common::xfer_load::XferLoad;
    use crate::common::xfer_save::XferSave;
    use crate::common::xfer::Xfer;
    use crate::logic::damage::{DamageInfo, DAMAGE_ARMOR_PIERCING, DAMAGE_HEALING};
    use crate::logic::module::update_module::UPDATE_SLEEP_FOREVER;
    use crate::logic::modules::new_module;
    use crate::logic::weapon::WeaponStore;
    use crate::logic::world::module_probe::{ProbeAnswers, ProbeModule};
    use crate::logic::world::object_create::{NewModule, NewObject};
    use crate::logic::world::object_xfer::ObjectXferStores;
    use crate::logic::world::update_body::UpdateBody;
    use std::sync::Arc;

    /// The data of `class` read from the INI text `text` through the factory's data proc.
    fn read(class: &str, text: &str) -> ModuleDataKind {
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

    fn base_world() -> World {
        let mut w = World::new();
        w.cuts.neutral_default_team = 1;
        w.teams.insert_team(crate::logic::world::team::Team::new(1, 0)).unwrap();
        w.random.init_game_logic_random(1234);
        w
    }

    fn probe_body(max: f32) -> NewModule {
        let mut m = NewModule::plain(b"ProbeBody", b"ModuleTag_Body");
        m.interfaces = MODULEINTERFACE_BODY | MODULEINTERFACE_DAMAGE;
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), max, Default::default())));
        m
    }

    /// An object made from `modules` (`(class, ini text)`), after `body` (a probe body of that health), with the given kind-of.
    fn make(w: &mut World, modules: &[(&str, &str)], body: Option<f32>, kinds: &[i32]) -> ObjectId {
        let next = w.next_object_id();
        let mut built: Vec<NewModule> = body.map(probe_body).into_iter().collect();
        for (i, (class, text)) in modules.iter().enumerate() {
            built.push(new_module(next, class.as_bytes(), format!("ModuleTag_U{i}").as_bytes(), &read(class, text)).unwrap().unwrap());
        }
        let v = ObjectTemplateValues { kind_of: kinds.to_vec(), template_id: 3, ..Default::default() };
        w.new_object(NewObject { template: Arc::new(ThingTemplate::from_object_values(&v)), status: Default::default(), team: None, modules: built }).unwrap()
    }

    fn next_call(w: &World, id: ObjectId, class: &[u8]) -> u32 {
        let idx = w.find_module(id, class).unwrap();
        let u = w.update_handle_of_module(ModuleHandle { object: id, index: idx }).unwrap();
        w.objects.update_module(u).unwrap().friend_get_next_call_frame()
    }

    fn health(w: &mut World, id: ObjectId) -> f32 {
        let b = w.get_body_module(id).unwrap();
        w.module_call(id, b, ModuleCall::GetHealth).as_real(0.0)
    }

    #[test]
    fn deletion_update_destroys_the_object_on_its_die_frame() {
        let mut w = base_world();
        let before = w.random.clone();
        // 500 ms is 15 frames
        let id = make(&mut w, &[("DeletionUpdate", "MinLifetime = 500\n MaxLifetime = 500")], None, &[]);
        assert_ne!(w.random, before, "the constructor drew the lifetime");
        assert_eq!(next_call(&w, id, b"DeletionUpdate"), 15);
        for _ in 0..15 {
            assert!(w.run_logic_frame());
            assert!(!w.objects.find(id).unwrap().is_destroyed());
        }
        assert!(w.run_logic_frame()); // frame 15
        assert!(w.objects.find(id).is_none_or(|o| o.is_destroyed()));
        assert!(w.cuts.module_errors.is_empty(), "{:?}", w.cuts.module_errors);
        assert!(w.cuts.reentrant_skips.is_empty(), "{:?}", w.cuts.reentrant_skips);
        assert_eq!(w.cuts.reentrant_delivered, 1, "destroyObject's onDelete reached the running module and was delivered");
    }

    #[test]
    fn lifetime_update_kills_through_the_body_and_a_hulk_uses_the_override() {
        let mut w = base_world();
        let id = make(&mut w, &[("LifetimeUpdate", "MinLifetime = 300\n MaxLifetime = 300")], Some(100.0), &[]);
        assert_eq!(next_call(&w, id, b"LifetimeUpdate"), 9);
        while w.frame() <= 9 {
            w.run_logic_frame();
        }
        assert_eq!(health(&mut w, id), 0.0, "kill() does the body all its health in damage");
        // a hulk with an override of 600 ms: 18 frames from now, whatever its own range says
        let mut w = base_world();
        w.cuts.globals.hulk_max_lifetime_override = 18;
        let hulk = make(&mut w, &[("LifetimeUpdate", "MinLifetime = 300\n MaxLifetime = 300")], Some(100.0), &[crate::common::kind_of::KINDOF_HULK]);
        assert_eq!(next_call(&w, hulk, b"LifetimeUpdate"), 18);
        let other = make(&mut w, &[("LifetimeUpdate", "MinLifetime = 300\n MaxLifetime = 300")], Some(100.0), &[]);
        assert_eq!(next_call(&w, other, b"LifetimeUpdate"), 9);
    }

    #[test]
    fn set_lifetime_range_redraws_and_rewakes() {
        let mut w = base_world();
        let id = make(&mut w, &[("LifetimeUpdate", "MinLifetime = 3000\n MaxLifetime = 3000")], Some(10.0), &[]);
        assert_eq!(next_call(&w, id, b"LifetimeUpdate"), 90);
        let idx = w.find_module(id, b"LifetimeUpdate").unwrap();
        w.module_call(id, idx, ModuleCall::SetLifetimeRange { min_frames: 5, max_frames: 5 });
        assert_eq!(next_call(&w, id, b"LifetimeUpdate"), 5);
        // the other module of the same shape
        let id = make(&mut w, &[("DeletionUpdate", "MinLifetime = 3000\n MaxLifetime = 3000")], None, &[]);
        let idx = w.find_module(id, b"DeletionUpdate").unwrap();
        w.module_call(id, idx, ModuleCall::SetLifetimeRange { min_frames: 7, max_frames: 7 });
        assert_eq!(next_call(&w, id, b"DeletionUpdate"), w.frame() + 7);
    }

    #[test]
    fn base_regenerate_heals_after_damage_and_sleeps_when_full() {
        let mut w = base_world();
        w.cuts.globals.base_regen_health_percent_per_second = 0.1; // 3 * (100 * 0.1) / 30 = 1 per tick
        w.cuts.globals.base_regen_delay = 4;
        let id = make(&mut w, &[("BaseRegenerateUpdate", "")], Some(100.0), &[]);
        let b = w.get_body_module(id).unwrap();
        // 50 damage; the damage reaches the regeneration module as `onDamage`
        let mut info = DamageInfo::default();
        info.input.amount = 50.0;
        info.input.damage_type = DAMAGE_ARMOR_PIERCING;
        w.module_call(id, b, ModuleCall::AttemptDamage(info.clone()));
        let idx = w.find_module(id, b"BaseRegenerateUpdate").unwrap();
        w.module_call(id, idx, ModuleCall::OnDamage(info));
        assert_eq!(next_call(&w, id, b"BaseRegenerateUpdate"), w.frame() + 4, "the delay after a damage");
        for _ in 0..4 {
            w.run_logic_frame();
        }
        assert_eq!(health(&mut w, id), 50.0, "nothing healed yet");
        for _ in 0..13 {
            w.run_logic_frame();
        }
        // the update on frame 4 heals 1, then every third frame: 4, 7, 10, 13, 16
        assert_eq!(health(&mut w, id), 55.0);
        // healing damage does not restart the delay
        let mut heal = DamageInfo::default();
        heal.input.damage_type = DAMAGE_HEALING;
        let at = w.frame();
        w.module_call(id, idx, ModuleCall::OnDamage(heal));
        assert_eq!(next_call(&w, id, b"BaseRegenerateUpdate"), UPDATE_SLEEP_FOREVER.0, "forever (a frame past forever is forever), from frame {at}");
        assert!(w.cuts.module_errors.is_empty() && w.cuts.reentrant_skips.is_empty());
    }

    #[test]
    fn smart_bomb_homes_on_its_target_only_while_high_above_the_ground() {
        let mut w = base_world();
        w.cuts.globals.gravity = -1.0;
        let id = make(&mut w, &[("SmartBombTargetHomingUpdate", "CourseCorrectionScalar = 0.5")], None, &[]);
        let idx = w.find_module(id, b"SmartBombTargetHomingUpdate").unwrap();
        w.set_position(id, &Coord3D { x: 10.0, y: 20.0, z: 100.0 });
        // no target: nothing moves
        w.run_logic_frame();
        assert_eq!(*w.objects.find(id).unwrap().thing.position(), Coord3D { x: 10.0, y: 20.0, z: 100.0 });
        // a refused target (zero length) is ignored; a real one is taken
        w.module_call(id, idx, ModuleCall::SetSmartBombTargetPosition(Coord3D { x: 0.0, y: 0.0, z: 0.0 }));
        w.run_logic_frame();
        assert_eq!(w.objects.find(id).unwrap().thing.position().x, 10.0);
        w.module_call(id, idx, ModuleCall::SetSmartBombTargetPosition(Coord3D { x: 50.0, y: 60.0, z: 0.0 }));
        w.run_logic_frame();
        let p = *w.objects.find(id).unwrap().thing.position();
        assert_eq!((p.x, p.y, p.z), (30.0, 40.0, 100.0), "half way to the target in x and y, height kept");
        // on the ground it does not steer
        w.set_position(id, &Coord3D { x: 10.0, y: 20.0, z: 1.0 });
        w.run_logic_frame();
        assert_eq!(w.objects.find(id).unwrap().thing.position().x, 10.0);
    }

    #[test]
    fn an_object_with_the_update_modules_saves_and_loads() {
        let mut names = crate::common::name_key_generator::NameKeyGenerator::new();
        let uc = crate::common::upgrade::UpgradeCenter::new();
        let make_world = || {
            let mut w = base_world();
            let id = make(
                &mut w,
                &[("LifetimeUpdate", "MinLifetime = 3000\n MaxLifetime = 3000"), ("BaseRegenerateUpdate", ""), ("SmartBombTargetHomingUpdate", "")],
                Some(100.0),
                &[],
            );
            (w, id)
        };
        let (mut w, id) = make_world();
        w.run_logic_frame();
        let mut x = XferSave::new();
        x.open(b"object").unwrap();
        let mut stores = ObjectXferStores { upgrade_center: &uc, names: &mut names, factory: &ThingFactory::new(), weapon_store: &WeaponStore::new() };
        w.object_xfer(id, &mut x, &mut stores).expect("save");
        x.close().unwrap();
        let bytes = x.take_bytes();

        // a fresh world, whose lifetime was drawn from another seed, is brought to the saved state by the load
        let (mut w2, id2) = make_world();
        w2.objects.update_module_mut(w2.update_handle_of_module(ModuleHandle { object: id2, index: w2.find_module(id2, b"LifetimeUpdate").unwrap() }).unwrap())
            .unwrap()
            .friend_set_next_call_frame(77);
        let mut x = XferLoad::new(bytes.clone());
        x.open(b"object").unwrap();
        let mut stores = ObjectXferStores { upgrade_center: &uc, names: &mut names, factory: &ThingFactory::new(), weapon_store: &WeaponStore::new() };
        w2.object_xfer(id2, &mut x, &mut stores).expect("load");
        x.close().unwrap();
        w2.object_load_post_process(id2);
        assert_eq!(next_call(&w2, id2, b"LifetimeUpdate"), next_call(&w, id, b"LifetimeUpdate"));
        let mut x = XferSave::new();
        x.open(b"object").unwrap();
        let mut stores = ObjectXferStores { upgrade_center: &uc, names: &mut names, factory: &ThingFactory::new(), weapon_store: &WeaponStore::new() };
        w2.object_xfer(id2, &mut x, &mut stores).expect("save again");
        x.close().unwrap();
        assert_eq!(x.take_bytes().len(), bytes.len());
        let _ = INVALID_ID;
    }

    #[test]
    fn the_factory_knows_the_four_and_the_data_is_read() {
        use crate::common::module::MODULEINTERFACE_UPDATE;
        use crate::common::module_factory::translated_module;
        for (name, mask) in [
            ("LifetimeUpdate", MODULEINTERFACE_UPDATE),
            ("DeletionUpdate", MODULEINTERFACE_UPDATE),
            ("BaseRegenerateUpdate", MODULEINTERFACE_UPDATE | MODULEINTERFACE_DAMAGE),
            ("SmartBombTargetHomingUpdate", MODULEINTERFACE_UPDATE),
        ] {
            let t = translated_module(name).unwrap();
            assert_eq!(t.which_interfaces, Some(mask), "{name}");
            let data = (t.create_data_proc.unwrap())(None).unwrap();
            assert!(matches!((t.create_proc.unwrap())(4, Some(data)).unwrap(), crate::common::module::Module::Update(_)));
        }
        match read("LifetimeUpdate", "MinLifetime = 1000\n MaxLifetime = 2000") {
            ModuleDataKind::LifetimeUpdate(d) => assert_eq!((d.min_frames, d.max_frames), (30, 60)),
            _ => panic!(),
        }
    }
}
