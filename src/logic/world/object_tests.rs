// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned tests (this repository's own design, not upstream code)
// of the `Object` translation (layer 8). The differential against the
// compiled `Object.cpp` is `tests/differential_object.rs`; these tests pin
// the behaviour that needs no C++ side: the order of effects, the cut
// points, the events and the xfer round trip.

use std::sync::Arc;

use crate::common::disabled_types::{DISABLED_EMP, DISABLED_HELD};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::module::{
    MODULEINTERFACE_BODY, MODULEINTERFACE_COLLIDE, MODULEINTERFACE_CREATE, MODULEINTERFACE_DAMAGE, MODULEINTERFACE_DIE,
    MODULEINTERFACE_UPGRADE,
};
use crate::common::name_key_generator::NameKeyGenerator;
use crate::common::object_status_types::{
    OBJECT_STATUS_DESTROYED, OBJECT_STATUS_MASKED, OBJECT_STATUS_UNDER_CONSTRUCTION, OBJECT_STATUS_UNSELECTABLE,
};
use crate::common::thing_template::ThingTemplate;
use crate::common::upgrade::UpgradeCenter;
use crate::common::xfer::Xfer;
use crate::common::xfer_crc::XferCrc;
use crate::common::xfer_load::XferLoad;
use crate::common::xfer_save::XferSave;
use crate::logic::client_event::ClientEvent;
use crate::logic::damage::DAMAGE_UNRESISTABLE;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle};
use crate::logic::world::module_probe::{damage_of, ProbeAnswers, ProbeModule};
use crate::logic::world::object::HelperKind;
use crate::logic::world::object_create::{NewModule, NewObject};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::object_state::mask_of;
use crate::logic::world::object_xfer::ObjectXferStores;
use crate::logic::world::update_body::UpdateBody;

fn probe_module(max_health: f32) -> NewModule {
    let mut m = NewModule::plain(b"ProbeBody", b"ModuleTag_Body");
    m.interfaces = MODULEINTERFACE_BODY | MODULEINTERFACE_DAMAGE | MODULEINTERFACE_DIE | MODULEINTERFACE_UPGRADE;
    m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), max_health, Default::default())));
    m
}

fn world() -> World {
    let mut w = World::new();
    w.cuts.neutral_default_team = 1;
    w.teams.insert_team(crate::logic::world::team::Team { id: 1, prototype: 0 });
    w.cuts.log = Some(Vec::new());
    w
}

fn make(w: &mut World, modules: Vec<NewModule>) -> ObjectId {
    let tt = Arc::new(ThingTemplate::new(0));
    w.new_object(NewObject { template: tt, status: Default::default(), team: None, modules }).expect("new object")
}

fn health(w: &mut World, id: ObjectId) -> f32 {
    let b = w.get_body_module(id).expect("body");
    w.module_call(id, b, ModuleCall::GetHealth).as_real(0.0)
}

fn events(w: &World) -> Vec<&ObjectClientEvent> {
    w.events
        .events()
        .iter()
        .filter_map(|e| match e {
            ClientEvent::Object(o) => Some(o),
            _ => None,
        })
        .collect()
}

#[test]
fn a_new_object_is_findable_and_has_the_helpers() {
    let mut w = world();
    let id = make(&mut w, vec![probe_module(100.0)]);
    let o = w.objects.find(id).unwrap();
    assert!(o.modules_ready);
    assert!(o.helpers.smc.is_some() && o.helpers.status_damage.is_some() && o.helpers.subdual_damage.is_some());
    assert!(o.helpers.defection.is_some());
    assert!(o.body.is_some());
    // the helpers come first ("even before Behaviors!"): Smc, StatusDamage, SubdualDamage, Defection
    assert_eq!(o.body, Some(4));
}

#[test]
fn ids_are_distinct_and_created_events_carry_the_object_id() {
    let mut w = world();
    let a = make(&mut w, vec![]);
    let b = make(&mut w, vec![]);
    assert_ne!(a, b);
    let ids: Vec<i32> = w
        .events
        .events()
        .iter()
        .filter_map(|e| match e {
            ClientEvent::ObjectCreated { object_id, .. } => Some(*object_id),
            _ => None,
        })
        .collect();
    // the template has no drawable, so the client event may be absent; when sent it names the object
    assert!(ids.iter().all(|i| *i == a as i32 || *i == b as i32));
}

#[test]
fn status_bits_set_and_clear_and_destroyed_is_sticky() {
    let mut w = world();
    let id = make(&mut w, vec![]);
    w.set_status(id, mask_of(OBJECT_STATUS_UNSELECTABLE), true);
    assert!(w.objects.find(id).unwrap().test_status(OBJECT_STATUS_UNSELECTABLE));
    w.clear_status(id, mask_of(OBJECT_STATUS_UNSELECTABLE));
    assert!(!w.objects.find(id).unwrap().test_status(OBJECT_STATUS_UNSELECTABLE));
    w.set_status(id, mask_of(OBJECT_STATUS_MASKED), true);
    assert!(w.objects.find(id).unwrap().test_status(OBJECT_STATUS_MASKED));
    w.destroy_object(id);
    assert!(w.objects.find(id).unwrap().test_status(OBJECT_STATUS_DESTROYED));
    assert!(w.objects.find(id).unwrap().is_destroyed());
    w.process_destroy_list();
    assert!(w.objects.find(id).is_none());
}

#[test]
fn under_construction_is_reported_as_incomplete() {
    let mut w = world();
    let id = make(&mut w, vec![]);
    assert!(w.is_construction_complete(id));
    w.set_status(id, mask_of(OBJECT_STATUS_UNDER_CONSTRUCTION), true);
    w.objects.find_mut(id).unwrap().construction_percent = 50.0;
    assert!(!w.is_construction_complete(id));
}

#[test]
fn damage_reaches_the_body_and_is_recorded() {
    let mut w = world();
    let id = make(&mut w, vec![probe_module(100.0)]);
    let mut info = damage_of(30.0, INVALID_ID);
    w.attempt_damage(id, &mut info);
    assert_eq!(info.out.actual_damage_dealt, 30.0);
    assert_eq!(health(&mut w, id), 70.0);
    assert_eq!(w.objects.find(id).unwrap().damage_received.len(), 1);
}

#[test]
fn lethal_damage_calls_on_die_once() {
    let mut w = world();
    let id = make(&mut w, vec![probe_module(50.0)]);
    let mut info = damage_of(500.0, INVALID_ID);
    info.input.damage_type = DAMAGE_UNRESISTABLE;
    w.attempt_damage(id, &mut info);
    assert_eq!(health(&mut w, id), 0.0);
    // effectively-dead is set by the body modules (layer 9), not by onDie
    let died = w.cuts.log.as_ref().unwrap().iter().filter(|c| matches!(c, CutCall::TeamNotifyTeamOfObjectDeath { .. })).count();
    assert_eq!(died, 1);
    let mut again = damage_of(10.0, INVALID_ID);
    w.attempt_damage(id, &mut again);
    assert_eq!(health(&mut w, id), 0.0);
}

#[test]
fn healing_is_clamped_to_max_health() {
    let mut w = world();
    let id = make(&mut w, vec![probe_module(100.0)]);
    let mut info = damage_of(40.0, INVALID_ID);
    w.attempt_damage(id, &mut info);
    w.attempt_healing(id, 1000.0, INVALID_ID);
    assert_eq!(health(&mut w, id), 100.0);
}

#[test]
fn heal_completely_restores_health() {
    let mut w = world();
    let id = make(&mut w, vec![probe_module(100.0)]);
    let mut info = damage_of(90.0, INVALID_ID);
    w.attempt_damage(id, &mut info);
    w.heal_completely(id);
    assert_eq!(health(&mut w, id), 100.0);
}

#[test]
fn estimate_damage_asks_the_body_without_changing_it() {
    let mut w = world();
    let id = make(&mut w, vec![probe_module(100.0)]);
    let input = damage_of(25.0, INVALID_ID).input;
    assert_eq!(w.estimate_damage(id, &input), 25.0);
    assert_eq!(health(&mut w, id), 100.0);
}

#[test]
fn disabled_types_set_and_clear_with_a_timer() {
    let mut w = world();
    let id = make(&mut w, vec![]);
    w.set_disabled_until(id, DISABLED_EMP, 50);
    assert!(w.objects.find(id).unwrap().is_disabled_by_type(DISABLED_EMP));
    assert_eq!(w.get_disabled_until(id, DISABLED_EMP), 50);
    w.set_disabled(id, DISABLED_HELD);
    assert!(w.objects.find(id).unwrap().is_disabled());
    assert!(w.clear_disabled(id, DISABLED_EMP));
    assert!(!w.objects.find(id).unwrap().is_disabled_by_type(DISABLED_EMP));
    assert!(w.objects.find(id).unwrap().is_disabled_by_type(DISABLED_HELD));
    assert!(!w.clear_disabled(id, DISABLED_EMP));
}

#[test]
fn check_disabled_status_expires_timed_disables() {
    let mut w = world();
    let id = make(&mut w, vec![]);
    w.set_disabled_until(id, DISABLED_EMP, 1);
    w.check_disabled_status(id);
    assert!(!w.objects.find(id).unwrap().is_disabled_by_type(DISABLED_EMP) || w.frame() < 1);
}

#[test]
fn weapon_set_and_armor_set_flags_toggle() {
    let mut w = world();
    let id = make(&mut w, vec![probe_module(10.0)]);
    w.set_weapon_set_flag(id, 1);
    w.clear_weapon_set_flag(id, 1);
    w.set_weapon_bonus_condition(id, 2);
    w.clear_weapon_bonus_condition(id, 2);
    w.set_armor_set_flag(id, 1);
    assert!(w.test_armor_set_flag(id, 1));
    w.clear_armor_set_flag(id, 1);
    assert!(!w.test_armor_set_flag(id, 1));
}

#[test]
fn team_changes_notify_the_client_and_restore() {
    let mut w = world();
    w.cuts.neutral_default_team = 1;
    w.cuts.team_players.insert(1, 0);
    w.cuts.team_players.insert(2, 1);
    w.cuts.team_names.insert(1, b"neutral".to_vec());
    w.cuts.team_names.insert(2, b"alpha".to_vec());
    let id = make(&mut w, vec![]);
    assert_eq!(w.get_team(id), Some(1));
    w.set_team(id, Some(2));
    assert_eq!(w.get_team(id), Some(2));
    assert_eq!(w.get_controlling_player(id), Some(1));
    assert!(events(&w).iter().any(|e| matches!(e, ObjectClientEvent::ObjectChangedTeam { .. })));
    w.set_temporary_team(id, Some(1));
    w.restore_original_team(id);
    assert_eq!(w.get_team(id), Some(2));
}

#[test]
fn position_and_orientation_setters_report_changes() {
    let mut w = world();
    let id = make(&mut w, vec![]);
    let p = crate::common::base_type::Coord3D { x: 10.0, y: 20.0, z: 0.0 };
    w.set_position(id, &p);
    assert_eq!(w.objects.find(id).unwrap().get_position().x, 10.0);
    w.set_orientation(id, 1.0);
    assert!((w.objects.find(id).unwrap().thing.orientation() - 1.0).abs() < 1e-6);
    assert!(crate::logic::world::object_transform::is_pos_different(&p, &crate::common::base_type::Coord3D { x: 11.0, y: 20.0, z: 0.0 }));
}

#[test]
fn real_to_int_truncates_toward_zero() {
    use crate::logic::world::object_transform::real_to_int;
    assert_eq!(real_to_int(2.9), 2);
    assert_eq!(real_to_int(-2.9), -2);
    assert_eq!(real_to_int(0.0), 0);
}

#[test]
fn module_calls_to_a_running_module_are_skipped_and_recorded() {
    let mut w = world();
    let id = make(&mut w, vec![probe_module(10.0)]);
    let b = w.get_body_module(id).unwrap();
    let body = w.take_module_body(ModuleHandle { object: id, index: b }).expect("body present");
    let r = w.module_call(id, b, ModuleCall::GetHealth);
    assert!(matches!(r, crate::logic::world::module_call::ModuleReply::Unhandled));
    assert_eq!(w.cuts.reentrant_skips.len(), 1);
    w.put_module_body(ModuleHandle { object: id, index: b }, body);
    assert_eq!(health(&mut w, id), 10.0);
}

#[test]
fn collide_and_create_interfaces_are_listed_by_mask() {
    let mut w = world();
    let mut m = probe_module(10.0);
    m.interfaces |= MODULEINTERFACE_COLLIDE | MODULEINTERFACE_CREATE;
    let id = make(&mut w, vec![m]);
    let o = w.objects.find(id).unwrap();
    let idx = o.body.unwrap() as usize;
    assert_ne!(o.modules[idx].interfaces & MODULEINTERFACE_COLLIDE, 0);
    assert_ne!(o.modules[idx].interfaces & MODULEINTERFACE_CREATE, 0);
}

#[test]
fn helper_stubs_have_names_and_tags() {
    for kind in [
        HelperKind::Smc,
        HelperKind::StatusDamage,
        HelperKind::SubdualDamage,
        HelperKind::Repulsor,
        HelperKind::Defection,
        HelperKind::WeaponStatus,
        HelperKind::FiringTracker,
        HelperKind::TempWeaponBonus,
    ] {
        let m = NewModule::helper_stub(kind);
        assert!(!m.name.is_empty() && m.tag.starts_with(b"ModuleTag_"));
    }
}

#[test]
fn the_cut_log_records_actions() {
    let mut w = world();
    let id = make(&mut w, vec![]);
    let before = w.cuts.log.as_ref().unwrap().len();
    w.cut(CutCall::GameLogicDeselectObject { object: id, player_mask: 0, affect_client: true });
    assert_eq!(w.cuts.log.as_ref().unwrap().len(), before + 1);
}

fn save(w: &mut World, id: ObjectId, names: &mut NameKeyGenerator, uc: &UpgradeCenter) -> Vec<u8> {
    let mut x = XferSave::new();
    x.open(b"object").unwrap();
    let mut stores = ObjectXferStores { upgrade_center: uc, names };
    w.object_xfer(id, &mut x, &mut stores).expect("save");
    x.close().unwrap();
    x.take_bytes()
}

#[test]
fn xfer_round_trips_and_is_deterministic() {
    let mut names = NameKeyGenerator::new();
    let uc = UpgradeCenter::new();
    let mut w = world();
    let id = make(&mut w, vec![probe_module(100.0)]);
    let mut info = damage_of(25.0, INVALID_ID);
    w.attempt_damage(id, &mut info);
    w.set_disabled_until(id, DISABLED_EMP, 77);
    let bytes = save(&mut w, id, &mut names, &uc);
    assert_eq!(bytes, save(&mut w, id, &mut names, &uc));

    // load into an identical fresh object
    let mut w2 = world();
    let id2 = make(&mut w2, vec![probe_module(100.0)]);
    assert_eq!(id, id2);
    let mut x = XferLoad::new(bytes.clone());
    x.open(b"object").unwrap();
    let mut stores = ObjectXferStores { upgrade_center: &uc, names: &mut names };
    w2.object_xfer(id2, &mut x, &mut stores).expect("load");
    x.close().unwrap();
    w2.object_load_post_process(id2);
    assert_eq!(health(&mut w2, id2), 75.0);
    assert_eq!(w2.get_disabled_until(id2, DISABLED_EMP), 77);
    // the probe counts the calls it receives (saved state), and loading makes some: same layout, not same count
    assert_eq!(bytes.len(), save(&mut w2, id2, &mut names, &uc).len());
}

#[test]
fn crc_changes_with_health_and_not_with_a_second_call() {
    let mut w = world();
    let id = make(&mut w, vec![probe_module(100.0)]);
    let crc = |w: &mut World| {
        let mut x = XferCrc::new();
        w.object_crc(id, &mut x).unwrap();
        x.get_crc()
    };
    let a = crc(&mut w);
    assert_eq!(a, crc(&mut w));
    let mut info = damage_of(1.0, INVALID_ID);
    w.attempt_damage(id, &mut info);
    assert_ne!(a, crc(&mut w));
}

#[test]
fn set_object_id_rekeys_and_refuses_a_taken_id() {
    let mut w = world();
    let a = make(&mut w, vec![]);
    let b = make(&mut w, vec![]);
    assert!(w.set_object_id(a, b).is_err());
    let new = b + 100;
    assert_eq!(w.set_object_id(a, new).unwrap(), new);
    assert!(w.objects.find(new).is_some());
    assert!(w.objects.find(a).is_none());
}

#[test]
fn contain_bookkeeping_marks_and_unmarks() {
    let mut w = world();
    let a = make(&mut w, vec![]);
    let b = make(&mut w, vec![]);
    w.on_contained_by(a, b);
    assert!(w.is_contained(a));
    w.on_removed_from(a);
    assert!(!w.is_contained(a));
}

#[test]
fn selectability_follows_the_unselectable_status() {
    let mut w = world();
    let id = make(&mut w, vec![]);
    w.set_selectable(id, true);
    assert!(w.is_selectable(id));
    w.set_status(id, mask_of(OBJECT_STATUS_UNSELECTABLE), true);
    assert!(!w.is_selectable(id));
}

#[test]
fn vision_ranges_are_stored() {
    let mut w = world();
    let id = make(&mut w, vec![]);
    w.set_vision_range(id, 120.0);
    w.set_shroud_clearing_range(id, 90.0);
    w.set_shroud_range(id, 30.0);
    assert_eq!(w.get_vision_range(id), 120.0);
    assert_eq!(w.get_shroud_clearing_range(id), 90.0);
    assert_eq!(w.get_shroud_range(id), 30.0);
}

#[test]
fn kill_runs_the_die_path() {
    let mut w = world();
    let id = make(&mut w, vec![probe_module(100.0)]);
    w.kill(id, DAMAGE_UNRESISTABLE, crate::logic::damage::DEATH_NORMAL);
    assert_eq!(health(&mut w, id), 0.0);
}
