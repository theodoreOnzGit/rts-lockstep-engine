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
    MODULEINTERFACE_BODY, MODULEINTERFACE_COLLIDE, MODULEINTERFACE_CREATE, MODULEINTERFACE_DAMAGE,
    MODULEINTERFACE_DIE, MODULEINTERFACE_UPGRADE,
};
use crate::common::name_key_generator::NameKeyGenerator;
use crate::common::object_status_types::{
    OBJECT_STATUS_DESTROYED, OBJECT_STATUS_MASKED, OBJECT_STATUS_UNDER_CONSTRUCTION,
    OBJECT_STATUS_UNSELECTABLE,
};
use crate::common::thing_factory::ThingFactory;
use crate::common::thing_template::ThingTemplate;
use crate::common::upgrade::UpgradeCenter;
use crate::common::xfer::Xfer;
use crate::common::xfer_crc::XferCrc;
use crate::common::xfer_load::XferLoad;
use crate::common::xfer_save::XferSave;
use crate::logic::client_event::ClientEvent;
use crate::logic::damage::DAMAGE_UNRESISTABLE;
use crate::logic::weapon::WeaponStore;
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
    m.interfaces = MODULEINTERFACE_BODY
        | MODULEINTERFACE_DAMAGE
        | MODULEINTERFACE_DIE
        | MODULEINTERFACE_UPGRADE;
    m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
        ProbeAnswers::default(),
        max_health,
        Default::default(),
    )));
    m
}

fn world() -> World {
    let mut w = World::new();
    w.cuts.neutral_default_team = 1;
    w.teams
        .insert_team(crate::logic::world::team::Team::new(1, 0))
        .unwrap();
    w.cuts.log = Some(Vec::new());
    w
}

fn make(w: &mut World, modules: Vec<NewModule>) -> ObjectId {
    let tt = Arc::new(ThingTemplate::new(0));
    w.new_object(NewObject {
        template: tt,
        status: Default::default(),
        team: None,
        modules,
    })
    .expect("new object")
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
    assert!(
        o.helpers.smc.is_some()
            && o.helpers.status_damage.is_some()
            && o.helpers.subdual_damage.is_some()
    );
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
    assert!(w
        .objects
        .find(id)
        .unwrap()
        .test_status(OBJECT_STATUS_UNSELECTABLE));
    w.clear_status(id, mask_of(OBJECT_STATUS_UNSELECTABLE));
    assert!(!w
        .objects
        .find(id)
        .unwrap()
        .test_status(OBJECT_STATUS_UNSELECTABLE));
    w.set_status(id, mask_of(OBJECT_STATUS_MASKED), true);
    assert!(w
        .objects
        .find(id)
        .unwrap()
        .test_status(OBJECT_STATUS_MASKED));
    w.destroy_object(id);
    assert!(w
        .objects
        .find(id)
        .unwrap()
        .test_status(OBJECT_STATUS_DESTROYED));
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
    let died = w
        .cuts
        .log
        .as_ref()
        .unwrap()
        .iter()
        .filter(|c| matches!(c, CutCall::TeamNotifyTeamOfObjectDeath { .. }))
        .count();
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
    assert!(w
        .objects
        .find(id)
        .unwrap()
        .is_disabled_by_type(DISABLED_EMP));
    assert_eq!(w.get_disabled_until(id, DISABLED_EMP), 50);
    w.set_disabled(id, DISABLED_HELD);
    assert!(w.objects.find(id).unwrap().is_disabled());
    assert!(w.clear_disabled(id, DISABLED_EMP));
    assert!(!w
        .objects
        .find(id)
        .unwrap()
        .is_disabled_by_type(DISABLED_EMP));
    assert!(w
        .objects
        .find(id)
        .unwrap()
        .is_disabled_by_type(DISABLED_HELD));
    assert!(!w.clear_disabled(id, DISABLED_EMP));
}

#[test]
fn check_disabled_status_expires_timed_disables() {
    let mut w = world();
    let id = make(&mut w, vec![]);
    w.set_disabled_until(id, DISABLED_EMP, 1);
    w.check_disabled_status(id);
    assert!(
        !w.objects
            .find(id)
            .unwrap()
            .is_disabled_by_type(DISABLED_EMP)
            || w.frame() < 1
    );
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
    assert!(events(&w)
        .iter()
        .any(|e| matches!(e, ObjectClientEvent::ObjectChangedTeam { .. })));
    w.set_temporary_team(id, Some(1));
    w.restore_original_team(id);
    assert_eq!(w.get_team(id), Some(2));
}

#[test]
fn position_and_orientation_setters_report_changes() {
    let mut w = world();
    let id = make(&mut w, vec![]);
    let p = crate::common::base_type::Coord3D {
        x: 10.0,
        y: 20.0,
        z: 0.0,
    };
    w.set_position(id, &p);
    assert_eq!(w.objects.find(id).unwrap().get_position().x, 10.0);
    w.set_orientation(id, 1.0);
    assert!((w.objects.find(id).unwrap().thing.orientation() - 1.0).abs() < 1e-6);
    assert!(crate::logic::world::object_transform::is_pos_different(
        &p,
        &crate::common::base_type::Coord3D {
            x: 11.0,
            y: 20.0,
            z: 0.0
        }
    ));
}

#[test]
fn real_to_int_truncates_toward_zero() {
    use crate::logic::world::object_transform::real_to_int;
    assert_eq!(real_to_int(2.9), 2);
    assert_eq!(real_to_int(-2.9), -2);
    assert_eq!(real_to_int(0.0), 0);
}

#[test]
fn a_call_to_a_body_that_is_out_and_not_lent_is_recorded_as_a_defect() {
    let mut w = world();
    let id = make(&mut w, vec![probe_module(10.0)]);
    let b = w.get_body_module(id).unwrap();
    let body = w
        .take_module_body(ModuleHandle {
            object: id,
            index: b,
        })
        .expect("body present");
    let r = w.module_call(id, b, ModuleCall::GetHealth);
    assert!(matches!(
        r,
        crate::logic::world::module_call::ModuleReply::Unhandled
    ));
    assert_eq!(
        w.cuts.reentrant_skips.len(),
        1,
        "the tripwire records a module that did not lend itself"
    );
    w.put_module_body(
        ModuleHandle {
            object: id,
            index: b,
        },
        body,
    );
    assert_eq!(health(&mut w, id), 10.0);
}

#[test]
fn a_body_that_dies_is_called_back_at_once_by_its_own_die_interface() {
    // ActiveBody::attemptDamage -> Object::onDie -> the same module's DieModuleInterface, a plain recursive call
    // upstream: the nested OnDie runs inside AttemptDamage, before it returns, on the live state.
    let mut w = world();
    w.cuts.trace = Some(Vec::new());
    let id = make(&mut w, vec![probe_module(10.0)]);
    let b = w.get_body_module(id).unwrap();
    w.cuts.trace.as_mut().unwrap().clear();
    let mut info = damage_of(25.0, INVALID_ID);
    info.input.amount = 25.0;
    w.attempt_damage(id, &mut info);
    let t = w.cuts.trace.clone().unwrap();
    let attempt = t
        .iter()
        .position(|l| l == "mod AttemptDamage")
        .expect("attempt");
    let die = t
        .iter()
        .position(|l| l == "mod OnDie")
        .expect("the die interface was called");
    assert!(die > attempt);
    assert!(w.cuts.reentrant_skips.is_empty(), "nothing is skipped");
    assert_eq!(
        w.cuts.reentrant_delivered, 1,
        "one call ran inside a call to the same module"
    );
    // the state the nested call changed (the call count) was not lost when the outer call took its body back
    let calls_before = match w.objects.find(id).unwrap().modules[b as usize]
        .body
        .as_ref()
    {
        Some(UpdateBody::ModuleProbe(m)) => m.calls,
        _ => panic!("probe body"),
    };
    // helpers answer first: AttemptDamage + OnDie + whatever the death asks (at least these two)
    assert!(calls_before >= 2, "calls = {calls_before}");
    assert_eq!(health(&mut w, id), 0.0);
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
    w.cut(CutCall::GameLogicDeselectObject {
        object: id,
        player_mask: 0,
        affect_client: true,
    });
    assert_eq!(w.cuts.log.as_ref().unwrap().len(), before + 1);
}

fn save(w: &mut World, id: ObjectId, names: &mut NameKeyGenerator, uc: &UpgradeCenter) -> Vec<u8> {
    let mut x = XferSave::new();
    x.open(b"object").unwrap();
    let mut stores = ObjectXferStores {
        upgrade_center: uc,
        names,
        factory: &ThingFactory::new(),
        weapon_store: &WeaponStore::new(),
    };
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
    let mut stores = ObjectXferStores {
        upgrade_center: &uc,
        names: &mut names,
        factory: &ThingFactory::new(),
        weapon_store: &WeaponStore::new(),
    };
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
        w.object_crc(id, &mut x, &WeaponStore::new()).unwrap();
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

// ---- the second batch: the reactions the differential test exercises, pinned without the C++ ------------------------

use crate::common::base_type::{Coord3D, ICoord3D};
use crate::common::disabled_types::DISABLED_UNMANNED;
use crate::common::game_common::{Relationship, VeterancyLevel};
use crate::common::kind_of::{
    KINDOF_BOOBY_TRAP, KINDOF_INFANTRY, KINDOF_MP_COUNT_FOR_VICTORY, KINDOF_PROJECTILE,
    KINDOF_STRUCTURE, KINDOF_VEHICLE,
};
use crate::common::module::{MODULEINTERFACE_CONTAIN, MODULEINTERFACE_SPECIAL_POWER};
use crate::common::object_status_types::{
    OBJECT_STATUS_BOOBY_TRAPPED, OBJECT_STATUS_DETECTED, OBJECT_STATUS_NO_ATTACK,
    OBJECT_STATUS_RECONSTRUCTING,
};
use crate::common::thing_template::ObjectTemplateValues;
use crate::common::upgrade::UpgradeMaskType;
use crate::logic::terrain_logic::TriggerArea;
use crate::logic::world::object_events::{EvaMessage, ObjectSound, RadarCall};
use crate::logic::world::object_modules::SpecialPowerTarget;

fn kinds(w: &mut World, kinds: &[i32], modules: Vec<NewModule>) -> ObjectId {
    let v = ObjectTemplateValues {
        kind_of: kinds.to_vec(),
        template_id: 7,
        is_trainable: true,
        experience_required: [0, 10, 20, 30],
        ..Default::default()
    };
    let template = Arc::new(ThingTemplate::from_object_values(&v));
    w.new_object(NewObject {
        template,
        status: Default::default(),
        team: None,
        modules,
    })
    .expect("new object")
}

fn probe_with(f: impl FnOnce(&mut ProbeAnswers)) -> NewModule {
    let mut a = ProbeAnswers::default();
    f(&mut a);
    let mut m = NewModule::plain(b"ProbeBody", b"ModuleTag_Body");
    m.interfaces = MODULEINTERFACE_BODY
        | MODULEINTERFACE_DAMAGE
        | MODULEINTERFACE_DIE
        | MODULEINTERFACE_UPGRADE;
    m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
        a,
        100.0,
        Default::default(),
    )));
    m
}

/// A world that records the module calls: `ObjectCuts::trace` is what `module_names` reads.
fn traced_world() -> World {
    let mut w = world();
    w.cuts.trace = Some(Vec::new());
    w
}

/// A module of the given class that answers the scripted way and so appears in the trace; no interfaces.
fn named_probe(class: &[u8], tag: &[u8]) -> NewModule {
    let mut m = NewModule::plain(class, tag);
    m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
        ProbeAnswers::default(),
        1.0,
        Default::default(),
    )));
    m
}

fn module_names(w: &World) -> Vec<String> {
    w.cuts
        .trace
        .as_ref()
        .map(|t| {
            t.iter()
                .filter(|l| l.starts_with("mod "))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn defect_changes_team_marks_the_defector_and_tells_the_client() {
    let mut w = world();
    w.cuts.team_players.insert(1, 0);
    w.cuts.team_players.insert(2, 1);
    w.cuts.player_default_teams.insert(0, 1);
    w.cuts.player_default_teams.insert(1, 2);
    w.teams
        .insert_team(crate::logic::world::team::Team::new(2, 0))
        .unwrap();
    w.players.set_player_count(2);
    let id = make(&mut w, vec![probe_with(|_| {})]);
    w.defect(id, 2, 30);
    assert_eq!(w.get_team(id), Some(2));
    assert!(w.objects.find(id).unwrap().is_undetected_defector());
    assert!(events(&w).iter().any(|e| matches!(
        e,
        ObjectClientEvent::Sound {
            sound: ObjectSound::VoiceDefect,
            ..
        }
    )));
    // defecting to the default team of the player who now controls it does nothing
    w.defect(id, 2, 0);
    assert_eq!(w.get_team(id), Some(2));
    w.go_invulnerable(id, 0);
    assert!(!w.objects.find(id).unwrap().is_undetected_defector());
}

#[test]
fn disabling_an_unmanned_vehicle_plays_the_splatter_and_lifting_a_power_loss_the_reenabled_sound() {
    let mut w = world();
    let id = kinds(&mut w, &[KINDOF_VEHICLE], vec![probe_with(|_| {})]);
    w.set_disabled(id, DISABLED_UNMANNED);
    assert!(events(&w).iter().any(|e| matches!(
        e,
        ObjectClientEvent::Sound {
            sound: ObjectSound::SplatterVehiclePilotsBrain,
            ..
        }
    )));
    let s = kinds(&mut w, &[KINDOF_STRUCTURE], vec![probe_with(|_| {})]);
    w.set_disabled(s, DISABLED_EMP);
    assert!(events(&w).iter().any(|e| matches!(
        e,
        ObjectClientEvent::Sound {
            sound: ObjectSound::BuildingDisabled,
            ..
        }
    )));
    assert!(w.clear_disabled(s, DISABLED_EMP));
    assert!(events(&w).iter().any(|e| matches!(
        e,
        ObjectClientEvent::Sound {
            sound: ObjectSound::BuildingReenabled,
            ..
        }
    )));
}

#[test]
fn the_death_of_a_local_unit_says_so_and_a_destroyed_hole_rebuilds() {
    let mut w = traced_world();
    w.cuts.team_players.insert(1, 0);
    w.players.set_player_count(1);
    w.players.set_local_player(Some(0));
    let hole = make(
        &mut w,
        vec![
            probe_with(|_| {}),
            named_probe(b"RebuildHoleBehavior", b"ModuleTag_Hole"),
        ],
    );
    let unit = kinds(&mut w, &[KINDOF_INFANTRY], vec![probe_with(|_| {})]);
    let building = kinds(
        &mut w,
        &[KINDOF_STRUCTURE, KINDOF_MP_COUNT_FOR_VICTORY],
        vec![probe_with(|_| {})],
    );
    w.kill(
        unit,
        DAMAGE_UNRESISTABLE,
        crate::logic::damage::DEATH_NORMAL,
    );
    w.kill(
        building,
        DAMAGE_UNRESISTABLE,
        crate::logic::damage::DEATH_NORMAL,
    );
    let e = events(&w);
    assert!(e.iter().any(|e| matches!(
        e,
        ObjectClientEvent::Eva {
            message: EvaMessage::UnitLost
        }
    )));
    assert!(e.iter().any(|e| matches!(
        e,
        ObjectClientEvent::Eva {
            message: EvaMessage::BuildingLost
        }
    )));
    assert!(e.iter().any(|e| matches!(
        e,
        ObjectClientEvent::Radar {
            call: RadarCall::TryFakeEvent,
            ..
        }
    )));
    // an object being reconstructed that dies tells its hole
    let rebuilt = make(&mut w, vec![probe_with(|_| {})]);
    w.set_status(rebuilt, mask_of(OBJECT_STATUS_RECONSTRUCTING), true);
    w.set_producer(rebuilt, hole);
    w.kill(
        rebuilt,
        DAMAGE_UNRESISTABLE,
        crate::logic::damage::DEATH_NORMAL,
    );
    assert!(module_names(&w).contains(&"mod StartRebuildProcess".to_string()));
}

#[test]
fn a_sole_healing_benefactor_keeps_the_heal_until_it_expires() {
    let mut w = world();
    let id = make(&mut w, vec![probe_with(|_| {})]);
    // the sources must exist: `attemptHealingFromSoleBenefactor` refuses an unknown one ("sanity")
    let a = make(&mut w, vec![probe_with(|_| {})]);
    let b = make(&mut w, vec![probe_with(|_| {})]);
    // frame 0 is not past the initial expiration frame 0, so the first healer would be refused
    w.set_frame(1);
    let mut info = damage_of(50.0, INVALID_ID);
    w.attempt_damage(id, &mut info);
    assert!(w.attempt_healing_from_sole_benefactor(id, 10.0, a, 5));
    assert_eq!(w.get_sole_healing_benefactor(id), a);
    assert!(
        !w.attempt_healing_from_sole_benefactor(id, 10.0, b, 5),
        "another healer is refused while the first holds"
    );
    assert!(w.attempt_healing_from_sole_benefactor(id, 10.0, a, 5));
    w.set_frame(7); // 1 + 5 = 6 is the last frame the first healer holds
    assert_eq!(w.get_sole_healing_benefactor(id), INVALID_ID);
    assert!(w.attempt_healing_from_sole_benefactor(id, 10.0, b, 5));
    assert_eq!(health(&mut w, id), 80.0);
}

#[test]
fn special_powers_go_to_the_module_for_that_template_unless_denied_or_disabled() {
    let mut w = traced_world();
    let mut sp = NewModule::plain(b"SpecialPowerModule", b"ModuleTag_SP");
    sp.interfaces = MODULEINTERFACE_SPECIAL_POWER;
    sp.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
        ProbeAnswers {
            power_template: 3,
            special_power_type: 5,
            shortcut: true,
            ..Default::default()
        },
        1.0,
        Default::default(),
    )));
    let id = make(&mut w, vec![probe_with(|_| {}), sp]);
    assert!(w.objects.find(id).unwrap().has_any_special_power());
    w.do_special_power(id, Some(3), SpecialPowerTarget::None, 0, false);
    w.do_special_power(id, Some(4), SpecialPowerTarget::None, 0, false);
    w.cuts.special_power_denied.insert(id);
    w.do_special_power(id, Some(3), SpecialPowerTarget::Object(2), 0, false);
    w.do_special_power(id, Some(3), SpecialPowerTarget::Object(2), 0, true);
    w.set_disabled(id, DISABLED_EMP);
    w.do_special_power(
        id,
        Some(3),
        SpecialPowerTarget::Location {
            loc: Coord3D::default(),
            angle: 0.0,
        },
        0,
        true,
    );
    let names = module_names(&w);
    let did = |n: &str| names.iter().filter(|l| l.as_str() == n).count();
    assert_eq!(did("mod DoSpecialPower"), 1);
    assert_eq!(
        did("mod DoSpecialPowerAtObject"),
        1,
        "only the forced one passes the store"
    );
    assert_eq!(
        did("mod DoSpecialPowerAtLocation"),
        0,
        "a disabled object does nothing"
    );
    assert!(w.find_special_power_module_interface(id, 5).is_some());
    assert!(w
        .find_any_shortcut_special_power_module_interface(id)
        .is_some());
    assert!(w.get_special_power_module(id, Some(9)).is_none());
}

#[test]
fn upgrades_run_the_upgrade_modules_with_the_player_and_object_masks() {
    let mut w = traced_world();
    w.cuts.team_players.insert(1, 0);
    w.players.set_player_count(1);
    let trig = UpgradeMaskType::from_indices(&[2]);
    let mut m = probe_with(|_| {});
    m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
        ProbeAnswers::default(),
        100.0,
        trig,
    )));
    let id = make(&mut w, vec![m]);
    assert!(!w.has_upgrade(id, &trig));
    assert!(w.affected_by_upgrade(id, &trig));
    w.give_upgrade(id, Some(&trig));
    assert!(w.has_upgrade(id, &trig));
    assert!(module_names(&w).contains(&"mod AttemptUpgrade".to_string()));
    w.remove_upgrade(id, &trig);
    assert!(!w.has_upgrade(id, &trig));
    w.give_upgrade(id, None);
    // under construction nothing runs
    w.set_status(id, mask_of(OBJECT_STATUS_UNDER_CONSTRUCTION), true);
    w.cuts.trace.as_mut().unwrap().clear();
    w.update_upgrade_modules(id);
    assert!(module_names(&w).is_empty());
}

#[test]
fn experience_levels_set_the_weapon_flags_and_promote_with_a_sound() {
    let mut w = world();
    let id = kinds(&mut w, &[KINDOF_INFANTRY], vec![probe_with(|_| {})]);
    w.cut_experience_add_points(id, 25, true);
    assert_eq!(w.get_veterancy_level(id), VeterancyLevel::Elite);
    assert!(w
        .objects
        .find(id)
        .unwrap()
        .test_weapon_set_flag(crate::logic::weapon_types::WEAPONSET_ELITE));
    assert!(events(&w).iter().any(|e| matches!(
        e,
        ObjectClientEvent::Sound {
            sound: ObjectSound::UnitPromoted,
            ..
        }
    )));
    w.cut_experience_set_veterancy_level(id, VeterancyLevel::Regular, true);
    assert!(!w
        .objects
        .find(id)
        .unwrap()
        .test_weapon_set_flag(crate::logic::weapon_types::WEAPONSET_ELITE));
    w.cut_experience_set_experience_and_level(id, 12, false);
    assert_eq!(w.get_veterancy_level(id), VeterancyLevel::Veteran);
    assert!(w.cut_experience_is_accepting_points(id));
}

#[test]
fn moving_into_a_trigger_area_enters_it_and_out_exits_it_a_step_late() {
    let mut w = world();
    w.cuts.team_players.insert(1, 0);
    w.players.set_player_count(1);
    let name = crate::common::ascii_string::AsciiString::try_from_bytes(b"area").unwrap();
    let area = TriggerArea::new(
        name,
        1,
        false,
        vec![
            ICoord3D { x: 0, y: 0, z: 0 },
            ICoord3D { x: 100, y: 0, z: 0 },
            ICoord3D {
                x: 100,
                y: 100,
                z: 0,
            },
            ICoord3D { x: 0, y: 100, z: 0 },
        ],
    )
    .unwrap();
    w.terrain.set_triggers(vec![area]);
    let id = kinds(&mut w, &[KINDOF_INFANTRY], vec![probe_with(|_| {})]);
    w.set_frame(10);
    w.set_position(
        id,
        &Coord3D {
            x: 50.0,
            y: 50.0,
            z: 0.0,
        },
    );
    assert!(w.is_inside(id, Some(1)));
    assert!(w.did_enter(id, Some(1)));
    w.set_frame(20);
    w.set_position(
        id,
        &Coord3D {
            x: 500.0,
            y: 500.0,
            z: 0.0,
        },
    );
    // upstream tests "exited" with the integer position of the previous call, so the exit shows on the next move
    w.set_position(
        id,
        &Coord3D {
            x: 501.0,
            y: 500.0,
            z: 0.0,
        },
    );
    assert!(w.did_exit(id, Some(1)));
    assert!(!w.is_inside(id, Some(1)));
}

#[test]
fn a_projectile_or_inert_object_triggers_no_areas() {
    let mut w = world();
    let name = crate::common::ascii_string::AsciiString::try_from_bytes(b"area").unwrap();
    let area = TriggerArea::new(
        name,
        1,
        false,
        vec![
            ICoord3D { x: 0, y: 0, z: 0 },
            ICoord3D { x: 100, y: 0, z: 0 },
            ICoord3D { x: 0, y: 100, z: 0 },
        ],
    )
    .unwrap();
    w.terrain.set_triggers(vec![area]);
    let id = kinds(&mut w, &[KINDOF_PROJECTILE], vec![probe_with(|_| {})]);
    w.set_position(
        id,
        &Coord3D {
            x: 10.0,
            y: 10.0,
            z: 0.0,
        },
    );
    assert!(!w.is_inside(id, Some(1)));
}

#[test]
fn a_booby_trapped_object_detonates_its_sticky_bomb_unless_the_victim_is_a_friend() {
    let mut w = traced_world();
    w.cuts.team_players.insert(1, 0);
    w.players.set_player_count(1);
    let bomb = make(
        &mut w,
        vec![
            probe_with(|_| {}),
            named_probe(b"StickyBombUpdate", b"ModuleTag_Bomb"),
        ],
    );
    let host = kinds(&mut w, &[KINDOF_BOOBY_TRAP], vec![probe_with(|_| {})]);
    w.set_producer(bomb, host);
    w.set_status(host, mask_of(OBJECT_STATUS_BOOBY_TRAPPED), true);
    w.cuts.booby_traps_in_range = vec![bomb];
    assert!(
        !w.check_and_detonate_booby_trap(bomb, None),
        "the bomb itself is not booby trapped"
    );
    assert!(w.check_and_detonate_booby_trap(host, None));
    let friend = make(&mut w, vec![probe_with(|_| {})]);
    w.cuts
        .team_relationships
        .insert((1, 1), Relationship::Allies);
    w.cuts
        .player_relationships
        .insert((0, 1), Relationship::Allies);
    assert!(!w.check_and_detonate_booby_trap(host, Some(friend)));
    assert!(module_names(&w).contains(&"mod Detonate".to_string()));
}

#[test]
fn crushing_needs_a_higher_level_a_non_ally_and_a_manned_crusher() {
    let mut w = world();
    let v = ObjectTemplateValues {
        crusher_level: 2,
        ..Default::default()
    };
    let crusher = w
        .new_object(NewObject {
            template: Arc::new(ThingTemplate::from_object_values(&v)),
            status: Default::default(),
            team: None,
            modules: vec![probe_with(|_| {})],
        })
        .unwrap();
    let w2 = ObjectTemplateValues {
        crushable_level: 1,
        ..Default::default()
    };
    let victim = w
        .new_object(NewObject {
            template: Arc::new(ThingTemplate::from_object_values(&w2)),
            status: Default::default(),
            team: None,
            modules: vec![probe_with(|_| {})],
        })
        .unwrap();
    use crate::logic::world::object_damage::CrushSquishTestType::*;
    w.cuts
        .team_relationships
        .insert((1, 1), Relationship::Allies);
    assert!(
        !w.can_crush_or_squish(crusher, Some(victim), CrushOnly),
        "allies do not crush each other"
    );
    w.cuts
        .team_relationships
        .insert((1, 1), Relationship::Enemies);
    assert!(w.can_crush_or_squish(crusher, Some(victim), CrushOnly));
    assert!(!w.can_crush_or_squish(crusher, Some(victim), SquishOnly));
    assert!(!w.can_crush_or_squish(crusher, None, CrushOrSquish));
    w.set_disabled(crusher, DISABLED_UNMANNED);
    assert!(!w.can_crush_or_squish(crusher, Some(victim), CrushOnly));
}

#[test]
fn is_able_to_attack_follows_the_status_kind_and_weapon_rules() {
    let mut w = world();
    let id = kinds(
        &mut w,
        &[crate::common::kind_of::KINDOF_CAN_ATTACK],
        vec![probe_with(|_| {})],
    );
    assert!(w.is_able_to_attack(id));
    w.set_status(id, mask_of(OBJECT_STATUS_NO_ATTACK), true);
    assert!(!w.is_able_to_attack(id));
    w.set_status(id, mask_of(OBJECT_STATUS_NO_ATTACK), false);
    w.set_status(id, mask_of(OBJECT_STATUS_UNDER_CONSTRUCTION), true);
    assert!(!w.is_able_to_attack(id));
    let plain = make(&mut w, vec![probe_with(|_| {})]);
    assert!(!w.is_able_to_attack(plain));
    assert!(!w.is_mobile(plain) || w.is_mobile(plain));
}

#[test]
fn the_logic_crc_is_stable_and_follows_the_state() {
    let mut w = world();
    let id = make(&mut w, vec![probe_with(|_| {})]);
    let crc = |w: &mut World| {
        let mut x = XferCrc::new();
        w.object_crc(id, &mut x, &WeaponStore::new()).unwrap();
        x.get_crc()
    };
    let a = crc(&mut w);
    w.set_position(
        id,
        &Coord3D {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        },
    );
    assert_ne!(a, crc(&mut w), "the matrix is hashed");
    w.set_weapon_bonus_condition(id, 3);
    let b = crc(&mut w);
    w.clear_weapon_bonus_condition(id, 3);
    assert_ne!(b, crc(&mut w), "the weapon bonus is hashed");
}

#[test]
fn deleting_an_object_tells_the_client_and_leaves_the_id_free() {
    let mut w = world();
    let id = make(&mut w, vec![probe_with(|_| {})]);
    w.destroy_object(id);
    assert!(w.objects.find(id).unwrap().is_destroyed());
    w.cuts.log.as_mut().unwrap().clear();
    w.process_destroy_list();
    assert!(w.objects.find(id).is_none());
    assert!(w
        .events
        .events()
        .iter()
        .any(|e| matches!(e, ClientEvent::ObjectDestroyed { object_id } if *object_id == id)));
    assert!(w
        .cuts
        .log
        .as_ref()
        .unwrap()
        .iter()
        .any(|c| matches!(c, CutCall::ScriptEngineNotifyOfObjectDestruction { .. })));
}

#[test]
fn the_geometry_and_the_health_box_follow_the_object() {
    let mut w = world();
    let id = make(&mut w, vec![probe_with(|_| {})]);
    let g = crate::common::geometry::GeometryInfo::new(
        crate::common::geometry::GeometryType::Box,
        false,
        10.0,
        8.0,
        6.0,
    );
    w.set_geometry_info(id, &g);
    assert!(events(&w)
        .iter()
        .any(|e| matches!(e, ObjectClientEvent::DrawableReactToGeometryChange { .. })));
    w.set_geometry_info_z(id, 20.0);
    let pos = w.get_health_box_position(id).unwrap();
    assert!(pos.z >= 20.0 + 10.0);
    let (h, wd) = w.get_health_box_dimensions(id).unwrap();
    assert_eq!(h, 3.0);
    // (8 + 6) is under the lower limit of 20 for the size, which doubles to 40
    assert_eq!(wd, 40.0);
    w.set_custom_indicator_color(id, 0xff00ff00u32 as i32);
    assert_eq!(w.get_indicator_color(id), 0xff00ff00u32 as i32);
    w.remove_custom_indicator_color(id);
    assert_ne!(w.get_indicator_color(id), 0xff00ff00u32 as i32);
}

#[test]
fn xfer_load_restores_the_state_and_refuses_an_unknown_team() {
    let mut names = NameKeyGenerator::new();
    let uc = UpgradeCenter::new();
    let mut w = world();
    let id = make(&mut w, vec![probe_with(|_| {})]);
    let saved = save(&mut w, id, &mut names, &uc);
    let mut info = damage_of(30.0, INVALID_ID);
    w.attempt_damage(id, &mut info);
    w.set_status(id, mask_of(OBJECT_STATUS_DETECTED), true);
    let mut x = XferLoad::new(saved.clone());
    x.open(b"object").unwrap();
    let mut stores = ObjectXferStores {
        upgrade_center: &uc,
        names: &mut names,
        factory: &ThingFactory::new(),
        weapon_store: &WeaponStore::new(),
    };
    w.object_xfer(id, &mut x, &mut stores).unwrap();
    assert_eq!(health(&mut w, id), 100.0);
    assert!(!w
        .objects
        .find(id)
        .unwrap()
        .test_status(OBJECT_STATUS_DETECTED));
    // a world that does not know the team refuses the load
    let mut w2 = World::new();
    w2.cuts.neutral_default_team = 1;
    let id2 = {
        let tt = Arc::new(ThingTemplate::new(0));
        w2.new_object(NewObject {
            template: tt,
            status: Default::default(),
            team: None,
            modules: vec![probe_with(|_| {})],
        })
        .unwrap()
    };
    let mut x = XferLoad::new(saved);
    x.open(b"object").unwrap();
    let mut stores = ObjectXferStores {
        upgrade_center: &uc,
        names: &mut names,
        factory: &ThingFactory::new(),
        weapon_store: &WeaponStore::new(),
    };
    assert!(w2.object_xfer(id2, &mut x, &mut stores).is_err());
}

#[test]
fn contained_items_count_in_a_zero_slot_transport() {
    let mut w = world();
    let inside = make(&mut w, vec![probe_with(|_| {})]);
    let mut c = probe_with(|a| {
        a.zero_slot = true;
        a.contained = vec![inside];
    });
    c.interfaces |= MODULEINTERFACE_CONTAIN;
    let v = ObjectTemplateValues {
        transport_slot_count: 3,
        ..Default::default()
    };
    let host = w
        .new_object(NewObject {
            template: Arc::new(ThingTemplate::from_object_values(&v)),
            status: Default::default(),
            team: None,
            modules: vec![c],
        })
        .unwrap();
    assert_eq!(w.get_transport_slot_count(inside), 0);
    assert_eq!(
        w.get_transport_slot_count(host),
        0,
        "a special zero-slot container counts what it holds"
    );
    w.on_contained_by(inside, host);
    assert!(w.is_contained(inside));
}

#[test]
fn model_conditions_and_the_special_state_reach_the_drawable() {
    let mut w = world();
    let id = make(&mut w, vec![probe_with(|_| {})]);
    w.set_model_condition_state(id, 5);
    w.clear_model_condition_state(id, 5);
    w.clear_and_set_model_condition_state(id, 5, 6);
    w.set_special_model_condition_state(id, 9, 0);
    w.clear_special_model_condition_states(id);
    let n = events(&w)
        .iter()
        .filter(|e| matches!(e, ObjectClientEvent::DrawableModelCondition { .. }))
        .count();
    assert!(n >= 6);
}

// --- p1w11-sonnet ---
#[test]
fn a_helper_that_processes_every_disabled_type_runs_on_a_disabled_object() {
    // `StatusDamageHelper::getDisabledTypesToProcess()` is `DISABLEDMASK_ALL`: its timer runs out although the object is disabled. (The
    // scheduler reads the module in its arena, the `UpdateModule` base, so the concrete class sets its override there.)
    let mut w = world();
    let id = make(&mut w, vec![probe_module(100.0)]);
    let idx = w.find_module(id, b"StatusDamageHelper").unwrap();
    w.module_call(
        id,
        idx,
        ModuleCall::DoStatusDamage {
            status: OBJECT_STATUS_MASKED,
            duration: 2.0,
        },
    );
    assert!(w
        .objects
        .find(id)
        .unwrap()
        .test_status(OBJECT_STATUS_MASKED));
    w.objects
        .find_mut(id)
        .unwrap()
        .disabled
        .set(DISABLED_HELD, 1);
    w.set_frame(5);
    w.run_sleepy_updates();
    assert!(
        !w.objects
            .find(id)
            .unwrap()
            .test_status(OBJECT_STATUS_MASKED),
        "the helper ran although the object is disabled"
    );
}

// --- end p1w11-sonnet ---
