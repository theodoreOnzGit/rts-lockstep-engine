// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::world::Object` (layer 8) against upstream's own text (commit 0a05454d).
//!
//! # What is compared against what
//!
//! `build.rs` cuts `Object.cpp` and the class `Object` of `Object.h` out of the unchanged upstream files (the
//! includes and the untranslated regions removed, every cut asserted to occur once), and compiles them with the
//! verbatim texts of `Thing.cpp`, `ExperienceTracker.cpp`, the functions of `Geometry.cpp` that `Object` reaches,
//! `Coord3D`/`ICoord3D`/`Region3D` of `BaseType.h`, the enums of the headers that define them, the weapon-set
//! model-condition map and `initKindOfMasks`, on the stand-ins of `differential/shim_object` (teams, players, the
//! partition manager, the radar, the audio, the AI, the script engine, the game logic, the module interfaces ...).
//! The stand-ins answer from the program's configuration and record every call that has an effect; the Rust side
//! is `World` with `ProbeModule` bodies and the `ObjectCuts` tables set from the same configuration.
//!
//! # Method
//!
//! A **program** is a header (the configuration: players, teams, relationships, globals, upgrades, the sets the
//! cut points answer from, the polygon triggers) and a list of operations (create an object from a random template
//! with random probe modules; status and script status; disabled states and the frame; weapon-set, weapon-bonus and
//! armor-set flags; team changes, defection; damage with and without a shock wave, healing, kill, estimate;
//! position and orientation; experience; the kill score; upgrades; containment; vision; partition maintenance;
//! selectability; special model conditions; `destroyObject` and `processDestroyList`; collisions; masking;
//! effectively dead; captured; the logic CRC). The same words run on both sides. Compared line for line:
//!
//! 1. **the trace**: every cut-point call (`cut ...`), client event (`evt ...`) and call a module receives
//!    (`mod Name`), in the order they happen, with every argument (reals as bit patterns);
//! 2. **the return values** (`ret ...`) of the operations that have one;
//! 3. **a snapshot of every live object after every operation**: the length and a hash of its `XferSave` bytes
//!    (the whole of `Object::xfer`, version 9: id, matrix, team, ids, status, geometry, sightings, ranges, disabled
//!    frames, experience, upgrades, trigger infos, layers, formation, the state of each module, the weapon set, the
//!    special power bits ...) and the fields `xfer` does not write (radar data, partition data, container, drawable,
//!    team, destroyed).
//!
//! The reads of the world (`getRelationship`, `getPlayersWithRelationship`, `getDisabledUntil`, ...) are covered by
//! the effects that depend on them.
//!
//! # Re-entrant module calls (fixed 2026-10-18, `p1w7-sonnet`)
//!
//! A call that reaches a module whose body is running (`attemptDamage` -> `Object::onDie` -> the same module's
//! `DieModuleInterface`) is upstream's plain recursion and runs **at once**. The Rust body lends itself to its
//! slot around the call (`World::lend_body`), so the nested call runs inside the outer one, on the live state, in
//! upstream's order; the `BehaviorModule` stand-in no longer has a `running` guard. Previously both sides skipped
//! such a call. Evidence: in the default run the Rust side delivered **5,292** re-entrant calls (counted by
//! `cuts.reentrant_delivered`; the run asserts the count is above 0 and that `cuts.reentrant_skips`, the tripwire
//! for a module that did not lend itself, stays empty), with every trace line and snapshot equal. **Planted bugs**
//! for this change (Rust side only, 1,200 programs each, then removed): (a) the lend does not take the body back
//! (the state the nested call changed is lost): 1,200 of 1,200 programs differ; (b) a deferred queue instead of the
//! synchronous call (the nested call runs after the outer one returns): 1,200 of 1,200 differ, first at the trace
//! line of the nested `mod OnDie`.
//!
//! # Known deviation, kept in the stand-ins
//!
//! - **Terrain.** The Rust world has no height map, so its extent is unknown and `OFF_MAP` is never touched; the
//!   stand-in answers an extent too large to leave.
//!
//! # Results
//!
//! Run 2026-10-18, before the re-entrancy fix (`OBJECT_CASES=15000 cargo test --release --features differential --test differential_object --
//! --nocapture`, 166 s): **15,000 programs of 110 operations, 28,303,130 lines compared, 0 programs differ.** The
//! default run is 1,200 programs (2,258,873 lines, 0 differ). Pass criterion: every line equal.
//!
//! Translation defects this comparison found, all fixed in the Rust: (1) the constructor's AI attitude and attack
//! info used the team asked for instead of the team after `setTeam`; (2) `findAnyShortcutSpecialPowerModuleInterface`
//! did not skip modules without a special power template; (3) `m_destinationLayer` started as ground, upstream's
//! zero-filled allocation makes it 0 (`LAYER_INVALID` here).
//!
//! **Planted bugs** (each put into the Rust translation alone, the 1,200-program run repeated, then removed):
//! 18 plants (zero-damage radar event, repulsor sleep 3 s, disabled-from-own-frame, UNMANNED exception, allies as
//! enemies in `scoreTheKill`, upgrade attempted when already upgraded, always-masked contained, second-spy refresh,
//! value map before shroud, no booby-trap check in `onDie`, xfer producer/builder swapped, position threshold 0.02,
//! upgrades before the created event, no relook on defect, no radar refresh on team change, unsellable not cleared
//! on capture, health not in the CRC, dozer not told of a disabled edge). **All 18 are caught** (3 to 1,199 of 1,200
//! programs differ; the rarest are the repulsor sleep, 3, and the disabled-from-own-frame, 5). The booby-trap plant
//! first survived (0 differ): the detonation was never reached (no `mod Detonate` line in 1,200 programs), so the
//! generator now builds the scenario (producer, `BOOBY_TRAPPED`, damage, kill) and over-picks the rare status bits;
//! with that the plant differs in 137 programs and `Detonate` appears 211 times.
//!
//! **One undefined-behaviour path avoided in the C++ driver:** an object queued twice for destruction (a module
//! destroying its own object from `onDestroy`) is deleted twice upstream; the driver skips the second entry, as the
//! Rust world documents (`game_logic.rs`).
//!
//! **Not covered by this test** (checked by unit tests or not at all): `OFF_MAP` (the Rust terrain has no extent),
//! loading into a fresh world, the weapon forwarders and `updateObjValuesFromMapProperties` (not translated),
//! command-button helpers, bone positions.
#![cfg(feature = "differential")]

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;

use rts_lockstep_engine::common::base_type::{Coord3D, ICoord3D};
use rts_lockstep_engine::common::game_common::{ObjectShroudStatus, Relationship, VeterancyLevel};
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::module::{
    MODULEINTERFACE_BODY, MODULEINTERFACE_COLLIDE, MODULEINTERFACE_CONTAIN, MODULEINTERFACE_CREATE, MODULEINTERFACE_DAMAGE, MODULEINTERFACE_DESTROY,
    MODULEINTERFACE_DIE, MODULEINTERFACE_SPECIAL_POWER, MODULEINTERFACE_UPGRADE,
};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_NAMES};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::upgrade::{UpgradeCenter, UpgradeMaskType};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::common::object_status_types::{OBJECT_STATUS_AIRBORNE_TARGET, OBJECT_STATUS_BOOBY_TRAPPED};
use rts_lockstep_engine::logic::world::object_damage::CrushSquishTestType;
use rts_lockstep_engine::logic::world::object_modules::SpecialPowerTarget;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::terrain_logic::TriggerArea;
use rts_lockstep_engine::logic::weapon_set_flags::WEAPON_SET_NAMES;
use rts_lockstep_engine::logic::world::ids::TeamId;
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::ObjectGlobals;
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::logic::weapon::WeaponStore;
use rts_lockstep_engine::logic::world::object_xfer::ObjectXferStores;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use rts_lockstep_engine::logic::world::World;
use support::Gen;

// ---- the program ---------------------------------------------------------------------------------------------

/// The words of a program.
#[derive(Default)]
struct Prog(Vec<u32>);

impl Prog {
    fn u(&mut self, v: u32) {
        self.0.push(v);
    }
    fn i(&mut self, v: i32) {
        self.0.push(v as u32);
    }
    fn f(&mut self, v: f32) {
        self.0.push(v.to_bits());
    }
    fn b(&mut self, v: bool) {
        self.0.push(u32::from(v));
    }
    fn u64(&mut self, v: u64) {
        self.0.push(v as u32);
        self.0.push((v >> 32) as u32);
    }
}

/// The reader of the interpreter (the C++ side reads the same words in the same order).
struct Rd<'a> {
    w: &'a [u32],
    at: usize,
}

impl Rd<'_> {
    fn u(&mut self) -> u32 {
        let v = self.w[self.at];
        self.at += 1;
        v
    }
    fn i(&mut self) -> i32 {
        self.u() as i32
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(self.u())
    }
    fn b(&mut self) -> bool {
        self.u() != 0
    }
    fn u64(&mut self) -> u64 {
        let lo = u64::from(self.u());
        let hi = u64::from(self.u());
        lo | (hi << 32)
    }
    fn more(&self) -> bool {
        self.at < self.w.len()
    }
}

// ---- the lines -------------------------------------------------------------------------------------------------

/// A trace line as the C++ side writes it: the tokens of the `{:?}` text, reals as `f:<bits>`.
fn norm(s: &str) -> String {
    let is_float = |t: &str| -> bool {
        let body = t.strip_prefix('-').unwrap_or(t);
        if body == "NaN" || body == "inf" {
            return true;
        }
        let first = body.chars().next().unwrap_or(' ');
        first.is_ascii_digit() && (body.contains('.') || body.contains('e')) && body.parse::<f32>().is_ok()
    };
    let mut out: Vec<String> = Vec::new();
    for t in s.split([' ', '{', '}', '(', ')', ',', ':', '[', ']']).filter(|t| !t.is_empty()) {
        if is_float(t) {
            out.push(format!("f:{:08x}", t.parse::<f32>().expect("float token").to_bits()));
        } else {
            out.push(t.to_string());
        }
    }
    out.join(" ")
}

/// A line the Rust side builds itself (a return value, a snapshot), in the format of the C++ `Rec`.
struct Line(String);

impl Line {
    fn new(tag: &str, name: &str) -> Line {
        Line(format!("{tag} {name}"))
    }
    fn i(mut self, k: &str, v: i64) -> Line {
        self.0.push_str(&format!(" {k} {v}"));
        self
    }
    fn b(mut self, k: &str, v: bool) -> Line {
        self.0.push_str(&format!(" {k} {v}"));
        self
    }
    fn f(mut self, k: &str, v: f32) -> Line {
        self.0.push_str(&format!(" {k} f:{:08x}", v.to_bits()));
        self
    }
}

// ---- the interpreter -------------------------------------------------------------------------------------------

/// Names of the module classes of a program (`g_moduleNames` of the driver).
const MODULE_NAMES: [&str; 13] = [
    "ProbeBody",
    "PhysicsBehavior",
    "InactiveBody",
    "RadarUpgrade",
    "PowerPlantUpgrade",
    "OverchargeBehavior",
    "AutoHealBehavior",
    "ToppleUpdate",
    "StickyBombUpdate",
    "RebuildHoleBehavior",
    "SquishCollide",
    "SpecialPowerCompletionDie",
    "AnyModule",
];

struct Run {
    w: World,
    ids: Vec<ObjectId>,
    lines: Vec<String>,
    uc: UpgradeCenter,
    names: NameKeyGenerator,
    upgrade_masks: Vec<UpgradeMaskType>,
    production_levels: Vec<VeterancyLevel>,
    template_count: u16,
    /// Also write the bytes of every snapshot (to find where two of them differ).
    verbose: bool,
    /// The bytes `SAVE` took, by object index.
    saved: BTreeMap<i32, Vec<u8>>,
    /// The number of polygon triggers.
    n_triggers: usize,
}

fn level(i: i32) -> VeterancyLevel {
    match i {
        0 => VeterancyLevel::Regular,
        1 => VeterancyLevel::Veteran,
        2 => VeterancyLevel::Elite,
        _ => VeterancyLevel::Heroic,
    }
}

fn relationship(i: i32) -> Relationship {
    match i {
        0 => Relationship::Enemies,
        1 => Relationship::Neutral,
        _ => Relationship::Allies,
    }
}

fn shrouded(i: i32) -> ObjectShroudStatus {
    match i {
        1 => ObjectShroudStatus::Clear,
        2 => ObjectShroudStatus::PartialClear,
        3 => ObjectShroudStatus::Fogged,
        4 => ObjectShroudStatus::Shrouded,
        _ => ObjectShroudStatus::Invalid,
    }
}

fn mask64(m: u64) -> ObjectStatusMaskType {
    let mut mask = ObjectStatusMaskType::default();
    for i in 0..64 {
        if (m >> i) & 1 != 0 {
            mask.set(i, 1);
        }
    }
    mask
}

fn upgrade_mask(lo: u64, hi: u64) -> UpgradeMaskType {
    UpgradeMaskType::from_words([lo, hi, 0, 0])
}

impl Run {
    /// The configuration (the header of the program): `readHeader` of the driver.
    fn new(r: &mut Rd) -> Run {
        let mut w = World::new();
        w.cuts.log = Some(Vec::new());
        w.cuts.trace = Some(Vec::new());
        assert_eq!(r.u(), 0x4F42_4A31);
        let np = r.i();
        let mut production_levels = Vec::new();
        for p in 0..np {
            let default_team = r.i();
            let color = r.u();
            let night = r.u();
            let active = r.b();
            let playable = r.b();
            let skirmish = r.b();
            let lo = r.u64();
            let hi = r.u64();
            let plans = r.i();
            let prod = r.i();
            w.cuts.player_default_teams.insert(p, default_team as TeamId);
            w.cuts.player_colors.insert(p, color as i32);
            w.cuts.player_night_colors.insert(p, night as i32);
            if !active {
                w.cuts.inactive_players.insert(p);
            }
            if !playable {
                w.cuts.unplayable_players.insert(p);
            }
            if skirmish {
                w.cuts.skirmish_ai_players.insert(p);
            }
            w.cuts.player_upgrades.insert(p, upgrade_mask(lo, hi));
            w.cuts.player_battle_plans.insert(p, plans);
            production_levels.push(level(prod));
        }
        w.players.set_player_count(np);
        let nt = r.i();
        for _ in 0..nt {
            let id = r.i() as TeamId;
            let player = r.i();
            let template_info = r.b();
            let attack_info = r.b();
            w.teams.insert_team(Team { id, prototype: 0 });
            w.cuts.team_players.insert(id, player);
            w.cuts.team_names.insert(id, format!("team{id}").into_bytes());
            if template_info {
                w.cuts.teams_with_template_info.insert(id);
            }
            if attack_info {
                w.cuts.teams_with_attack_info.insert(id);
            }
        }
        let n = r.i();
        for _ in 0..n {
            let (a, b, rel) = (r.i() as TeamId, r.i() as TeamId, r.i());
            w.cuts.team_relationships.insert((a, b), relationship(rel));
        }
        let n = r.i();
        for _ in 0..n {
            let (p, t, rel) = (r.i(), r.i() as TeamId, r.i());
            w.cuts.player_relationships.insert((p, t), relationship(rel));
        }
        w.cuts.neutral_default_team = r.i() as TeamId;
        w.players.set_local_player(Some(r.i()));
        let g = ObjectGlobals {
            gravity: r.f(),
            force_models_to_follow_time_of_day: r.b(),
            time_of_day_is_night: r.b(),
            force_models_to_follow_weather: r.b(),
            weather_is_snowy: r.b(),
            level_gain_animation_name_set: r.b(),
            enable_repulsors: r.b(),
            draw_icon_ui: r.b(),
            objects_should_receive_difficulty_bonus: r.b(),
            max_tunnel_capacity: 0, // not read by Object.cpp (TunnelTracker.cpp, tested in differential_tunnel_tracker.rs)
        };
        w.cuts.globals = g;
        let _extent_known = r.b();
        let (_hx, _hy) = (r.f(), r.f());
        w.cuts.wall_height = r.f();
        for l in 0..4 {
            let b = r.i();
            if b > 0 {
                w.cuts.veterancy_upgrades[l] = Some(UpgradeMaskType::from_indices(&[b - 1]));
            }
        }
        let n = r.i();
        for _ in 0..n {
            w.cuts.special_power_denied.insert(r.i());
        }
        let n = r.i();
        for _ in 0..n {
            let (o, s) = (r.i(), r.i());
            w.cuts.weapons.insert((o, s));
        }
        let n = r.i();
        for _ in 0..n {
            w.cuts.weapon_sets_with_weapons.insert(r.i());
        }
        let n = r.i();
        for _ in 0..n {
            let (o, s, t) = (r.i(), r.i(), r.i());
            w.cuts.ai_turrets.insert((o, s), t);
        }
        let n = r.i();
        for _ in 0..n {
            let (o, t) = (r.i(), r.i());
            w.cuts.ai_turrets_disabled.insert((o, t));
        }
        let n = r.i();
        for _ in 0..n {
            w.cuts.ai_airborne.insert(r.i());
        }
        let n = r.i();
        for _ in 0..n {
            w.cuts.dozer_ais.insert(r.i());
        }
        let n = r.i();
        for _ in 0..n {
            let (o, t, f) = (r.i(), r.i(), r.f());
            w.cuts.ai_turret_rotation.insert((o, t), f);
        }
        let n = r.i();
        for _ in 0..n {
            let (o, p, s) = (r.i(), r.i(), r.i());
            w.cuts.shrouded_status.insert((o, p), shrouded(s));
        }
        let n = r.i();
        for _ in 0..n {
            w.cuts.potential_collisions.push(r.i());
        }
        let n = r.i();
        for _ in 0..n {
            w.cuts.booby_traps_in_range.push(r.i());
        }
        let k = r.i();
        let mut uc = UpgradeCenter::new();
        let mut names = NameKeyGenerator::new();
        let mut upgrade_masks = Vec::new();
        for i in 0..k {
            uc.new_upgrade(&mut names, format!("Upgrade_{i}").as_bytes()).expect("new upgrade");
            upgrade_masks.push(UpgradeMaskType::from_indices(&[i]));
        }
        let n = r.i();
        let mut triggers = Vec::new();
        for i in 0..n {
            let np = r.i();
            let mut points = Vec::new();
            for _ in 0..np {
                let (x, y) = (r.i(), r.i());
                points.push(ICoord3D { x, y, z: 0 });
            }
            let name = rts_lockstep_engine::common::ascii_string::AsciiString::try_from_bytes(format!("trigger{i}").as_bytes()).expect("name");
            triggers.push(TriggerArea::new(name, i + 1, false, points).expect("trigger"));
        }
        w.terrain.set_triggers(triggers);
        Run { w, ids: Vec::new(), lines: Vec::new(), uc, names, upgrade_masks, production_levels, template_count: 0, verbose: false, saved: BTreeMap::new(), n_triggers: n as usize }
    }

    /// Moves what the world traced into the lines.
    fn flush(&mut self) {
        let t = self.w.cuts.trace.as_mut().expect("trace is on");
        for s in t.drain(..) {
            self.lines.push(norm(&s));
        }
        self.w.cuts.log.as_mut().expect("log is on").clear();
        self.w.events.clear();
    }

    fn ret(&mut self, l: Line) {
        self.flush();
        self.lines.push(l.0);
    }

    fn live(&self, k: i32) -> Option<ObjectId> {
        let id = *self.ids.get(usize::try_from(k).ok()?)?;
        self.w.objects.find(id).map(|_| id)
    }

    fn create(&mut self, r: &mut Rd) {
        let mut v = ObjectTemplateValues::default();
        let index = self.template_count;
        self.template_count += 1;
        v.name = format!("Template{index}").into_bytes();
        v.template_id = index + 1;
        let nk = r.i();
        for _ in 0..nk {
            v.kind_of.push(r.i());
        }
        v.vision_range = r.f();
        v.shroud_clearing_range = r.f();
        v.shroud_reveal_to_all_range = r.f();
        {
            let gt = r.i();
            let small = r.b();
            let (h, maj, mnr) = (r.f(), r.f(), r.f());
            let gt = match gt {
                0 => GeometryType::Sphere,
                1 => GeometryType::Cylinder,
                _ => GeometryType::Box,
            };
            v.geometry = Some(GeometryInfo::new(gt, small, h, maj, mnr));
        }
        v.energy_production = r.i();
        v.energy_bonus = r.i();
        v.build_cost = r.i() as u16;
        v.threat_value = r.i() as u16;
        v.crusher_level = r.i() as u8;
        v.crushable_level = r.i() as u8;
        v.transport_slot_count = r.i() as i8;
        for i in 0..4 {
            v.experience_values[i] = r.i();
        }
        for i in 0..4 {
            v.experience_required[i] = r.i();
        }
        v.is_trainable = r.b();
        v.enter_guard = r.b();
        v.occlusion_delay = r.u();
        v.radar_priority = r.i() as u8;
        let nws = r.i();
        for _ in 0..nws {
            let nf = r.i();
            let mut flags = Vec::new();
            for _ in 0..nf {
                flags.push(r.i());
            }
            let has = r.b();
            v.weapon_sets.push((flags, has));
        }
        let team = r.i();
        let st = r.u64();
        let nm = r.i();
        let mut modules = Vec::new();
        for m in 0..nm {
            let name_idx = r.i() as usize;
            let tag = format!("ModuleTag_{m}");
            let mut a = ProbeAnswers::default();
            let interfaces = r.i();
            let stealth = r.b();
            let ai = r.b();
            let max_health = r.f();
            let trig = r.i();
            a.yes = r.b();
            a.has = r.u();
            a.salvage = r.b();
            a.sub_objects = r.b();
            a.enclosing = r.b();
            a.garrisonable = r.b();
            a.kick_out = r.b();
            a.passenger_may_fire = r.b();
            a.zero_slot = r.b();
            a.hero_inside = r.b();
            a.special_power_type = r.i();
            a.shortcut = r.b();
            a.power_template = r.u();
            a.rider = r.i();
            let nc = r.i();
            for _ in 0..nc {
                a.contained.push(r.i());
            }
            a.slaver = r.i();
            a.deck_height = r.f();
            a.apparent_player = r.i();
            a.countermeasure = r.i();
            a.collide_sets_no_collisions = r.b();
            let trigger = if trig > 0 { UpgradeMaskType::from_indices(&[trig - 1]) } else { UpgradeMaskType::default() };
            v.behavior_modules.push((MODULE_NAMES[name_idx].as_bytes().to_vec(), tag.clone().into_bytes()));
            let mut nmod = NewModule::plain(MODULE_NAMES[name_idx].as_bytes(), tag.as_bytes());
            nmod.interfaces = interfaces;
            nmod.is_stealth = stealth;
            nmod.is_ai = ai;
            nmod.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(a, max_health, trigger)));
            modules.push(nmod);
        }
        // the production veterancy levels, by template name
        for (p, l) in self.production_levels.iter().enumerate() {
            self.w.cuts.player_production_levels.insert((p as i32, v.name.clone()), *l);
        }
        let template = Arc::new(ThingTemplate::from_object_values(&v));
        let id = self
            .w
            .new_object(NewObject { template, status: mask64(st), team: if team != 0 { Some(team as TeamId) } else { None }, modules })
            .expect("new object");
        self.ids.push(id);
        self.ret(Line::new("ret", "Created").i("idx", self.ids.len() as i64 - 1).i("id", i64::from(id)));
        self.snapshot_all();
    }

    fn snapshot_all(&mut self) {
        for k in 0..self.ids.len() {
            let Some(id) = self.live(k as i32) else { continue };
            let mut x = XferSave::new();
            x.open(b"object").expect("open");
            {
                let mut stores = ObjectXferStores { upgrade_center: &self.uc, names: &mut self.names, factory: &ThingFactory::new(), weapon_store: &WeaponStore::new() };
                self.w.object_xfer(id, &mut x, &mut stores).expect("save");
            }
            x.close().expect("close");
            let bytes = x.take_bytes();
            let mut h: u32 = 2_166_136_261;
            for b in &bytes {
                h = (h ^ u32::from(*b)).wrapping_mul(16_777_619);
            }
            let o = self.w.objects.find(id).expect("live");
            let (radar, partition, contained_by, drawable, team, destroyed) = (o.on_radar, o.in_partition, o.contained_by, o.drawable, o.team, o.is_destroyed());
            self.flush();
            if self.verbose {
                let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
                self.lines.push(format!("bytes Object idx {k} {hex}"));
            }
            self.lines.push(Line::new("snap", "Object").i("idx", k as i64).i("id", i64::from(id)).i("len", bytes.len() as i64).i("hash", i64::from(h as i32)).0);
            self.lines.push(
                Line::new("extra", "Object")
                    .i("idx", k as i64)
                    .b("radar", radar)
                    .b("partition", partition)
                    .i("contained_by", i64::from(contained_by))
                    .i("drawable", i64::from(drawable))
                    .i("team", i64::from(team))
                    .b("destroyed", destroyed)
                    .i("frame_vision", 0)
                    .0,
            );
        }
    }

    fn ops(&mut self, r: &mut Rd) {
        while r.more() {
            let op = r.i();
            if op == 1 {
                self.create(r);
                continue;
            }
            let k = r.i();
            let o = self.live(k);
            self.op(op, k, o, r);
            self.flush();
            self.lines.push(Line::new("end", "Op").i("op", i64::from(op)).i("object", i64::from(k)).0);
            self.snapshot_all();
        }
    }

    fn op(&mut self, op: i32, k: i32, o: Option<ObjectId>, r: &mut Rd) {
        match op {
            2 => {
                let m = r.u64();
                let set = r.b();
                if let Some(id) = o {
                    self.w.set_status(id, mask64(m), set);
                }
            }
            3 => {
                let bit = r.i();
                let set = r.b();
                if let Some(id) = o {
                    self.w.set_script_status(id, 1 << bit, set);
                }
            }
            4 => {
                let (t, f) = (r.i(), r.u());
                if let Some(id) = o {
                    self.w.set_disabled_until(id, t, f);
                }
            }
            5 => {
                let t = r.i();
                if let Some(id) = o {
                    self.w.set_disabled(id, t);
                }
            }
            6 => {
                let t = r.i();
                if let Some(id) = o {
                    let res = self.w.clear_disabled(id, t);
                    self.ret(Line::new("ret", "ClearDisabled").b("result", res));
                }
            }
            7 => {
                if let Some(id) = o {
                    self.w.check_disabled_status(id);
                }
            }
            8 => {
                let f = r.u();
                self.w.set_frame(f);
            }
            9 => {
                let (wst, set) = (r.i(), r.b());
                if let Some(id) = o {
                    if set {
                        self.w.set_weapon_set_flag(id, wst);
                    } else {
                        self.w.clear_weapon_set_flag(id, wst);
                    }
                }
            }
            10 => {
                let (wbc, set) = (r.i(), r.b());
                if let Some(id) = o {
                    if set {
                        self.w.set_weapon_bonus_condition(id, wbc);
                    } else {
                        self.w.clear_weapon_bonus_condition(id, wbc);
                    }
                }
            }
            11 => {
                let (ast, mode) = (r.i(), r.i());
                if let Some(id) = o {
                    match mode {
                        0 => self.w.set_armor_set_flag(id, ast),
                        1 => self.w.clear_armor_set_flag(id, ast),
                        _ => {
                            let res = self.w.test_armor_set_flag(id, ast);
                            self.ret(Line::new("ret", "TestArmorSetFlag").b("result", res));
                        }
                    }
                }
            }
            12 => {
                let (team, mode) = (r.i(), r.i());
                if let Some(id) = o {
                    let t = if team != 0 { Some(team as TeamId) } else { None };
                    match mode {
                        0 => self.w.set_team(id, t),
                        1 => self.w.set_temporary_team(id, t),
                        _ => self.w.restore_original_team(id),
                    }
                }
            }
            13 => {
                let (team, time) = (r.i(), r.u());
                if let Some(id) = o {
                    self.w.defect(id, team as TeamId, time);
                }
            }
            14 => {
                let time = r.u();
                if let Some(id) = o {
                    self.w.go_invulnerable(id, time);
                }
            }
            15 => {
                let amount = r.f();
                let dtype = r.i();
                let kill = r.b();
                let src = r.i();
                let (sw_amount, sw_radius, taper, vx, vy, vz) = (r.f(), r.f(), r.f(), r.f(), r.f(), r.f());
                let smask = r.u();
                if let Some(id) = o {
                    let mut info = DamageInfo::default();
                    info.input.amount = amount;
                    info.input.damage_type = dtype;
                    info.input.kill = kill;
                    info.input.source_id = src;
                    info.input.shock_wave_amount = sw_amount;
                    info.input.shock_wave_radius = sw_radius;
                    info.input.shock_wave_taper_off = taper;
                    info.input.shock_wave_vector = Coord3D { x: vx, y: vy, z: vz };
                    info.input.source_player_mask = smask as u16;
                    self.w.attempt_damage(id, &mut info);
                    self.ret(Line::new("ret", "Damage").f("dealt", info.out.actual_damage_dealt).f("clipped", info.out.actual_damage_clipped).b("no_effect", info.out.no_effect));
                }
            }
            16 => {
                let (amount, src) = (r.f(), r.i());
                if let Some(id) = o {
                    let s = self.live(src - 1);
                    self.w.attempt_healing(id, amount, s.unwrap_or(0));
                }
            }
            17 => {
                let (dt, death) = (r.i(), r.i());
                if let Some(id) = o {
                    self.w.kill(id, dt, death);
                }
            }
            18 => {
                if let Some(id) = o {
                    self.w.heal_completely(id);
                }
            }
            19 => {
                let (amount, dt) = (r.f(), r.i());
                if let Some(id) = o {
                    let input = rts_lockstep_engine::logic::damage::DamageInfoInput { amount, damage_type: dt, ..Default::default() };
                    let res = self.w.estimate_damage(id, &input);
                    self.ret(Line::new("ret", "EstimateDamage").f("result", res));
                }
            }
            20 => {
                let p = Coord3D { x: r.f(), y: r.f(), z: r.f() };
                if let Some(id) = o {
                    self.w.set_position(id, &p);
                }
            }
            21 => {
                let a = r.f();
                if let Some(id) = o {
                    self.w.set_orientation(id, a);
                }
            }
            22 => {
                let z = r.f();
                if let Some(id) = o {
                    self.w.set_position_z(id, z);
                }
            }
            23 => {
                let (kind, v) = (r.i(), r.i());
                if let Some(id) = o {
                    match kind {
                        0 => self.w.cut_experience_add_points(id, v, true),
                        1 => self.w.cut_experience_set_veterancy_level(id, level(v), true),
                        _ => self.w.cut_experience_set_experience_and_level(id, v, true),
                    }
                }
            }
            24 => {
                let victim = r.i();
                if let (Some(id), Some(v)) = (o, self.live(victim)) {
                    self.w.score_the_kill(id, v);
                }
            }
            25 => {
                let bit = r.i() as usize;
                if let Some(id) = o {
                    let m = self.upgrade_masks[bit];
                    self.w.give_upgrade(id, Some(&m));
                }
            }
            26 => {
                let bit = r.i() as usize;
                if let Some(id) = o {
                    let m = self.upgrade_masks[bit];
                    self.w.remove_upgrade(id, &m);
                }
            }
            27 => {
                if let Some(id) = o {
                    self.w.update_upgrade_modules(id);
                }
            }
            28 => {
                let bit = r.i() as usize;
                if let Some(id) = o {
                    let m = self.upgrade_masks[bit];
                    let h = self.w.has_upgrade(id, &m);
                    let a = self.w.affected_by_upgrade(id, &m);
                    self.ret(Line::new("ret", "Upgrade").b("has", h).b("affected", a));
                }
            }
            29 => {
                let by = r.i();
                if let (Some(id), Some(c)) = (o, self.live(by)) {
                    if self.w.get_contain(c).is_some() && !self.w.objects.find(c).is_some_and(|x| x.is_destroyed()) {
                        self.w.on_contained_by(id, c);
                    }
                }
            }
            30 => {
                if let Some(id) = o {
                    self.w.on_removed_from(id);
                }
            }
            31 => {
                let (kind, v) = (r.i(), r.f());
                if let Some(id) = o {
                    match kind {
                        0 => self.w.set_vision_range(id, v),
                        1 => self.w.set_shroud_clearing_range(id, v),
                        _ => self.w.set_shroud_range(id, v),
                    }
                }
            }
            32 => {
                let (setting, player) = (r.b(), r.i());
                if let Some(id) = o {
                    self.w.set_vision_spied(id, setting, player);
                }
            }
            33 => {
                if let Some(id) = o {
                    self.w.handle_partition_cell_maintenance(id);
                }
            }
            34 => {
                let s = r.b();
                if let Some(id) = o {
                    self.w.set_selectable(id, s);
                }
            }
            35 => {
                let (mc, frames) = (r.i(), r.u());
                if let Some(id) = o {
                    self.w.set_special_model_condition_state(id, mc, frames);
                }
            }
            36 => {
                if let Some(id) = o {
                    self.w.clear_special_model_condition_states(id);
                }
            }
            37 => {
                if let Some(id) = o {
                    // what a contain module does in onDelete: let go of the objects it holds
                    for j in 0..self.ids.len() {
                        if let Some(other) = self.live(j as i32) {
                            if self.w.objects.find(other).is_some_and(|x| x.contained_by == id) {
                                self.w.on_removed_from(other);
                            }
                        }
                    }
                    self.w.destroy_object(id);
                }
            }
            38 => self.w.process_destroy_list(),
            39 => {
                let other = r.i();
                if let (Some(id), Some(t)) = (o, self.live(other)) {
                    let loc = Coord3D { x: 1.0, y: 2.0, z: 3.0 };
                    self.w.on_collide(id, t, Some(loc), Some(loc));
                }
            }
            41 => {
                let m = r.b();
                if let Some(id) = o {
                    self.w.mask_object(id, m);
                }
            }
            42 => {
                let d = r.b();
                if let Some(id) = o {
                    self.w.set_effectively_dead(id, d);
                }
            }
            43 => {
                let c = r.b();
                if let Some(id) = o {
                    self.w.set_captured(id, c);
                }
            }
            44 => {
                if let Some(id) = o {
                    let mut x = XferCrc::new();
                    self.w.object_crc(id, &mut x, &WeaponStore::new()).expect("crc");
                    self.ret(Line::new("ret", "Crc").i("crc", i64::from(x.get_crc())));
                }
            }
            45 => {
                if let Some(id) = o {
                    let res = self.w.is_able_to_attack(id);
                    self.ret(Line::new("ret", "AbleToAttack").b("result", res));
                }
            }
            46 => {
                if let Some(id) = o {
                    let res = self.w.is_hero(id);
                    self.ret(Line::new("ret", "IsHero").b("result", res));
                }
            }
            47 => {
                if let Some(id) = o {
                    let res = self.w.get_transport_slot_count(id);
                    self.ret(Line::new("ret", "TransportSlots").i("result", i64::from(res)));
                }
            }
            48 => {
                let (kind, tmpl, forced, opts, target) = (r.i(), r.i(), r.b(), r.u(), r.i());
                let loc = Coord3D { x: r.f(), y: r.f(), z: r.f() };
                let angle = r.f();
                if let Some(id) = o {
                    let t = if tmpl != 0 { Some(tmpl as u32) } else { None };
                    let target = match kind {
                        0 => SpecialPowerTarget::None,
                        1 => SpecialPowerTarget::Object(self.live(target).unwrap_or(0)),
                        2 => SpecialPowerTarget::Location { loc, angle },
                        _ => SpecialPowerTarget::Waypoint(1),
                    };
                    self.w.do_special_power(id, t, target, opts, forced);
                }
            }
            49 => {
                let (kind, ty, tmpl) = (r.i(), r.i(), r.i());
                if let Some(id) = o {
                    let m = match kind {
                        0 => self.w.find_special_power_module_interface(id, ty),
                        1 => self.w.find_any_shortcut_special_power_module_interface(id),
                        2 => self.w.find_special_ability_update(id, ty),
                        3 => self.w.find_special_power_completion_die(id),
                        4 => self.w.find_special_power_with_overridable_destination_active(id),
                        5 => self.w.find_special_power_with_overridable_destination(id),
                        6 => self.w.get_production_update_interface(id),
                        7 => self.w.get_dock_update_interface(id),
                        8 => self.w.get_spawn_behavior_interface(id),
                        9 => self.w.get_projectile_update_interface(id),
                        10 => self.w.get_countermeasures_behavior_interface(id),
                        11 => {
                            if self.w.has_object_exit_interface(id) {
                                Some(0)
                            } else {
                                None
                            }
                        }
                        _ => self.w.get_special_power_module(id, if tmpl != 0 { Some(tmpl as u32) } else { None }),
                    };
                    self.ret(Line::new("ret", "Find").i("kind", i64::from(kind)).i("index", m.map_or(-1, i64::from)));
                }
            }
            50 => {
                let d = Coord3D { x: r.f(), y: r.f(), z: r.f() };
                let (speed, opts) = (r.f(), r.u());
                if let Some(id) = o {
                    self.w.topple(id, Some(d), speed, opts);
                }
            }
            51 => {
                let (other, tt) = (r.i(), r.i());
                if let Some(id) = o {
                    let t = match tt {
                        0 => CrushSquishTestType::CrushOnly,
                        1 => CrushSquishTestType::SquishOnly,
                        _ => CrushSquishTestType::CrushOrSquish,
                    };
                    let res = self.w.can_crush_or_squish(id, self.live(other), t);
                    self.ret(Line::new("ret", "Crush").b("result", res));
                }
            }
            52 => {
                if let Some(id) = o {
                    let pr = self.w.get_radar_priority(id);
                    let pos = self.w.get_health_box_position(id).expect("live");
                    let dims = self.w.get_health_box_dimensions(id);
                    let (h, wd) = dims.unwrap_or((0.0, 0.0));
                    self.ret(Line::new("ret", "HealthBox").i("priority", i64::from(pr)).f("x", pos.x).f("y", pos.y).f("z", pos.z).b("dims", dims.is_some()).f("height", h).f("width", wd));
                }
            }
            54 => {
                let (kind, gt, small) = (r.i(), r.i(), r.b());
                let (h, maj, mnr, z) = (r.f(), r.f(), r.f(), r.f());
                if let Some(id) = o {
                    if kind == 0 {
                        let gt = match gt {
                            0 => GeometryType::Sphere,
                            1 => GeometryType::Cylinder,
                            _ => GeometryType::Box,
                        };
                        self.w.set_geometry_info(id, &GeometryInfo::new(gt, small, h, maj, mnr));
                    } else {
                        self.w.set_geometry_info_z(id, z);
                    }
                }
            }
            55 => {
                let (kind, color) = (r.i(), r.u());
                if let Some(id) = o {
                    match kind {
                        0 => self.w.set_custom_indicator_color(id, color as i32),
                        1 => self.w.remove_custom_indicator_color(id),
                        _ => {
                            let (c, n) = (self.w.get_indicator_color(id), self.w.get_night_indicator_color(id));
                            let custom = self.w.objects.find(id).is_some_and(|o| o.indicator_color != 0);
                            self.ret(Line::new("ret", "Indicator").i("color", i64::from(c as u32)).i("night", i64::from(n as u32)).b("custom", custom));
                        }
                    }
                }
            }
            56 => {
                let (kind, g) = (r.i(), r.i());
                if let Some(id) = o {
                    if kind == 0 {
                        self.w.enter_group(id, g as u32);
                    } else {
                        self.w.leave_group(id);
                    }
                }
            }
            57 => {
                let (which, obj) = (r.i(), r.i());
                if let Some(id) = o {
                    let t = if obj != 0 { self.live(obj - 1).unwrap_or(0) } else { 0 };
                    if which == 0 {
                        self.w.set_producer(id, t);
                    } else {
                        self.w.set_builder(id, t);
                    }
                }
            }
            58 => {
                let (kind, layer) = (r.i(), r.i());
                if let Some(id) = o {
                    if kind == 0 {
                        self.w.set_layer(id, layer);
                    } else {
                        self.w.set_destination_layer(id, layer);
                    }
                }
            }
            59 => {
                let kind = r.i();
                if let Some(id) = o {
                    if kind == 0 {
                        self.w.friend_prepare_for_map_boundary_adjust(id);
                    } else {
                        self.w.friend_notify_of_new_map_boundary(id);
                    }
                }
            }
            60 => {
                let incoming = r.b();
                if let Some(id) = o {
                    self.w.friend_adjust_power_for_player(id, incoming);
                }
            }
            61 => {
                let receive = r.b();
                if let Some(id) = o {
                    self.w.set_receiving_difficulty_bonus(id, receive);
                }
            }
            62 => {
                let (old_p, new_p) = (r.i(), r.i());
                if let Some(id) = o {
                    self.w.on_capture(id, old_p, new_p);
                }
            }
            63 => {
                let (amount, src, duration) = (r.f(), r.i(), r.u());
                if let Some(id) = o {
                    let s = if src != 0 { self.live(src - 1).unwrap_or(0) } else { 0 };
                    let res = self.w.attempt_healing_from_sole_benefactor(id, amount, s, duration);
                    let ben = self.w.get_sole_healing_benefactor(id);
                    self.ret(Line::new("ret", "SoleHeal").b("result", res).i("benefactor", i64::from(ben)));
                }
            }
            64 => {
                let (kind, other) = (r.i(), r.i());
                if let Some(id) = o {
                    let t = self.live(other);
                    match kind {
                        0 => {
                            let res = self.w.has_countermeasures(id);
                            self.ret(Line::new("ret", "HasCountermeasures").b("result", res));
                        }
                        1 => {
                            if let Some(t) = t {
                                self.w.report_missile_for_countermeasures(id, t);
                            }
                        }
                        _ => {
                            if let Some(t) = t {
                                let d = self.w.calculate_countermeasure_to_divert_to(id, t);
                                self.ret(Line::new("ret", "Divert").i("id", i64::from(d)));
                            }
                        }
                    }
                }
            }
            65 => {
                let mut f = [0f32; 12];
                for v in f.iter_mut() {
                    *v = r.f();
                }
                if let Some(id) = o {
                    let m = Matrix3D::new(f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7], f[8], f[9], f[10], f[11]);
                    self.w.set_transform_matrix(id, &m);
                }
            }
            66 => {
                let (kind, trig) = (r.i(), r.i());
                if let Some(id) = o {
                    let t = if (trig as usize) < self.n_triggers { Some(trig + 1) } else { None };
                    let res = match kind {
                        0 => self.w.did_enter(id, t),
                        1 => self.w.did_exit(id, t),
                        _ => self.w.is_inside(id, t),
                    };
                    self.ret(Line::new("ret", "Trigger").i("kind", i64::from(kind)).b("result", res));
                }
            }
            67 => {
                if let Some(id) = o {
                    let pt = self.w.calc_natural_rally_point(id).expect("live");
                    let deck = self.w.get_carrier_deck_height(id);
                    self.ret(Line::new("ret", "Rally").f("x", pt.x).f("y", pt.y).f("deck", deck));
                }
            }
            68 => {
                let victim = r.i();
                if let Some(id) = o {
                    let v = if victim != 0 { self.live(victim - 1) } else { None };
                    let res = self.w.check_and_detonate_booby_trap(id, v);
                    self.ret(Line::new("ret", "Booby").b("result", res));
                }
            }
            69 => {
                let (kind, amount, status, dur) = (r.i(), r.f(), r.i(), r.u());
                if let Some(id) = o {
                    match kind {
                        0 => self.w.notify_subdual_damage(id, amount),
                        1 => self.w.do_status_damage(id, status, amount),
                        _ => self.w.do_temp_weapon_bonus(id, status, dur),
                    }
                }
            }
            70 => {
                let (turret, old_rot) = (r.i(), r.f());
                if let Some(id) = o {
                    self.w.react_to_turret_change(id, turret, old_rot, 0.0);
                }
            }
            71 => {
                let (other, player, dtype) = (r.i(), r.i(), r.i());
                if let Some(id) = o {
                    let t = self.live(other);
                    let l = Line::new("ret", "Queries");
                    let rel = t.map_or(-1, |t| self.w.get_relationship(id, t) as i64);
                    let l = l
                        .b("mobile", self.w.is_mobile(id))
                        .b("selectable", self.w.is_selectable(id))
                        .b("mass", self.w.is_mass_selectable(id))
                        .b("local", self.w.is_locally_controlled(id))
                        .b("neutral", self.w.is_neutral_controlled(id))
                        .i("relationship", rel)
                        .b("faction", self.w.is_faction_structure(id))
                        .b("nonfaction", self.w.is_non_faction_structure(id))
                        .i("shrouded", self.w.get_shrouded_status(id, player) as i64)
                        .b("airborne", self.w.is_using_airborne_locomotor(id))
                        .i("level", self.w.get_veterancy_level(id) as i64)
                        .f("vision", self.w.get_vision_range(id))
                        .f("clearing", self.w.get_shroud_clearing_range(id))
                        .f("shroud", self.w.get_shroud_range(id))
                        .i("disabled_until", i64::from(self.w.get_disabled_until(id, dtype)))
                        .i("disabled_any", i64::from(self.w.get_disabled_until(id, 65535)));
                    let salvage = self.w.is_salvage_crate(id);
                    let l = l.b("salvage", salvage).b("contained", self.w.is_contained(id));
                    let h = self.w.get_height_above_terrain(id);
                    let o = self.w.objects.find(id).expect("live");
                    let l = l
                        .b("construction_complete", self.w.is_construction_complete(id))
                        .b("effectively_dead", o.is_effectively_dead())
                        .b("captured", o.is_captured())
                        .b("off_map", o.is_off_map())
                        .b("undetected_defector", o.is_undetected_defector())
                        .b("has_special", o.has_any_special_power())
                        .f("height", h)
                        .b("hero", false)
                        .i("crusher", i64::from(self.w.get_crusher_level(id)))
                        .i("crushable", i64::from(self.w.get_crushable_level(id)))
                        .b("airborne_target", o.test_status(OBJECT_STATUS_AIRBORNE_TARGET))
                        .b("destroyed", o.is_destroyed())
                        .b("modules_ready", o.modules_ready)
                        .i("script_status", i64::from(o.test_script_status_bit(1)))
                        .i("weapon_set", o.cur_weapon_set_flags.words()[0] as i64)
                        .i("weapon_bonus", i64::from(o.weapon_bonus_condition))
                        .b("single_use", o.single_use_command_used);
                    self.ret(l);
                }
            }
            72 => {
                if let Some(id) = o {
                    let mut x = XferSave::new();
                    x.open(b"object").expect("open");
                    {
                        let mut stores = ObjectXferStores { upgrade_center: &self.uc, names: &mut self.names, factory: &ThingFactory::new(), weapon_store: &WeaponStore::new() };
                        self.w.object_xfer(id, &mut x, &mut stores).expect("save");
                    }
                    x.close().expect("close");
                    self.saved.insert(k, x.take_bytes());
                }
            }
            73 => {
                if let Some(id) = o {
                    if let Some(bytes) = self.saved.get(&k).cloned() {
                        let mut x = XferLoad::new(bytes);
                        x.open(b"object").expect("open");
                        {
                            let mut stores = ObjectXferStores { upgrade_center: &self.uc, names: &mut self.names, factory: &ThingFactory::new(), weapon_store: &WeaponStore::new() };
                            self.w.object_xfer(id, &mut x, &mut stores).expect("load");
                        }
                        x.close().expect("close");
                        self.w.object_load_post_process(id);
                    }
                }
            }
            _ => panic!("unknown op {op}"),
        }
    }
}

fn run_rust(prog: &[u32], verbose: bool) -> Vec<String> {
    run_rust_counted(prog, verbose).0
}

/// The lines, the number of module calls that ran at once inside a call to the same module, and the number that
/// were skipped (must be 0).
fn run_rust_counted(prog: &[u32], verbose: bool) -> (Vec<String>, u32, usize) {
    let mut r = Rd { w: prog, at: 0 };
    let mut run = Run::new(&mut r);
    run.verbose = verbose;
    run.ops(&mut r);
    (run.lines, run.w.cuts.reentrant_delivered, run.w.cuts.reentrant_skips.len())
}

// ---- the C++ side ------------------------------------------------------------------------------------------------

fn flag_names() -> Vec<u8> {
    let mut out = String::new();
    let tables: [&[&str]; 4] = [
        &OBJECT_STATUS_NAMES,
        &rts_lockstep_engine::common::disabled_types::DISABLED_NAMES,
        &WEAPON_SET_NAMES,
        &rts_lockstep_engine::common::special_power_mask_type::SPECIAL_POWER_NAMES,
    ];
    for t in tables {
        out.push_str(&format!("{}\n", t.len()));
        for n in t {
            out.push_str(n);
            out.push('\n');
        }
    }
    out.into_bytes()
}

fn run_cpp(prog: &[u32], verbose: bool) -> Vec<String> {
    let names = flag_names();
    let mut cap = 1 << 20;
    loop {
        let mut buf = vec![0u8; cap];
        // SAFETY: the buffers are valid for the lengths passed; the wrapper writes at most `cap` bytes.
        let n = unsafe { cpp::ref_object_run(prog.as_ptr(), prog.len() as i32, names.as_ptr() as *const _, names.len() as i32, buf.as_mut_ptr() as *mut _, cap as i32, i32::from(verbose)) };
        if n < 0 {
            cap = (-n) as usize + 16;
            continue;
        }
        buf.truncate(n as usize);
        let text = String::from_utf8(buf).expect("utf-8 lines");
        return text.lines().map(str::to_string).collect();
    }
}

// ---- the generator -------------------------------------------------------------------------------------------------

const K_UPGRADES: i32 = 6;

struct Pg<'a> {
    g: &'a mut Gen,
    p: Prog,
    players: i32,
    teams: Vec<(i32, i32)>,
    created: i32,
}

impl Pg<'_> {
    fn chance(&mut self, pct: u32) -> bool {
        self.g.below(100) < pct
    }
    fn pick_team(&mut self) -> i32 {
        let i = self.g.below(self.teams.len() as u32) as usize;
        self.teams[i].0
    }
    fn small_obj(&mut self) -> i32 {
        1 + self.g.below(7) as i32
    }

    fn header(&mut self) {
        let np = 2 + self.g.below(3) as i32;
        self.players = np;
        self.p.u(0x4F42_4A31);
        self.p.i(np);
        for p in 0..np {
            self.p.i(10 + p); // default team
            let c = self.g.u32() | 0xff00_0000;
            self.p.u(c);
            let n = self.g.u32() | 0xff00_0000;
            self.p.u(n);
            let active = p == 0 || self.chance(85);
            self.p.b(active);
            let playable = self.chance(85);
            self.p.b(playable);
            let sk = self.chance(25);
            self.p.b(sk);
            let lo = u64::from(self.g.below(1 << K_UPGRADES));
            self.p.u64(lo);
            self.p.u64(0);
            let plans = if self.chance(25) { 1 + self.g.below(2) as i32 } else { 0 };
            self.p.i(plans);
            let prod = self.g.below(4) as i32;
            self.p.i(prod);
        }
        // the teams: each player's default team, and a few more
        for p in 0..np {
            self.teams.push((10 + p, p));
        }
        for t in 0..3 {
            let pl = self.g.below(np as u32) as i32;
            self.teams.push((20 + t, pl));
        }
        self.p.i(self.teams.len() as i32);
        for (id, pl) in self.teams.clone() {
            self.p.i(id);
            self.p.i(pl);
            // always: the constructor of Object reads the template info of its team without a null test
            self.p.b(true);
            let ai = self.chance(50);
            self.p.b(ai);
        }
        let ids: Vec<i32> = self.teams.iter().map(|t| t.0).collect();
        self.p.i((ids.len() * ids.len()) as i32);
        for a in &ids {
            for b in &ids {
                self.p.i(*a);
                self.p.i(*b);
                let rel = self.g.below(3) as i32;
                self.p.i(rel);
            }
        }
        self.p.i(np * ids.len() as i32);
        for pl in 0..np {
            for t in &ids {
                self.p.i(pl);
                self.p.i(*t);
                let rel = self.g.below(3) as i32;
                self.p.i(rel);
            }
        }
        self.p.i(10); // the neutral player's default team
        let local = self.g.below(np as u32) as i32;
        self.p.i(local);
        let gravity = if self.chance(70) { -1.0 } else { -self.g.uniform(0.1, 3.0) };
        self.p.f(gravity);
        for _ in 0..8 {
            let b = self.chance(50);
            self.p.b(b);
        }
        self.p.b(false); // extent known
        self.p.f(0.0);
        self.p.f(0.0);
        let wall = self.g.uniform(0.0, 20.0);
        self.p.f(wall);
        for _ in 0..4 {
            let b = if self.chance(60) { 1 + self.g.below(K_UPGRADES as u32) as i32 } else { 0 };
            self.p.i(b);
        }
        // sets keyed by object ids 1..7
        let n = self.g.below(3) as i32;
        self.p.i(n);
        for _ in 0..n {
            let o = self.small_obj();
            self.p.i(o);
        }
        let n = self.g.below(4) as i32;
        self.p.i(n);
        for _ in 0..n {
            let o = self.small_obj();
            let s = self.g.below(3) as i32;
            self.p.i(o);
            self.p.i(s);
        }
        let n = self.g.below(3) as i32;
        self.p.i(n);
        for _ in 0..n {
            let o = self.small_obj();
            self.p.i(o);
        }
        let n = self.g.below(3) as i32;
        self.p.i(n);
        for _ in 0..n {
            let o = self.small_obj();
            let s = self.g.below(3) as i32;
            let t = self.g.below(2) as i32;
            self.p.i(o);
            self.p.i(s);
            self.p.i(t);
        }
        let n = self.g.below(3) as i32;
        self.p.i(n);
        for _ in 0..n {
            let o = self.small_obj();
            let t = self.g.below(2) as i32;
            self.p.i(o);
            self.p.i(t);
        }
        for _ in 0..2 {
            let n = self.g.below(3) as i32;
            self.p.i(n);
            for _ in 0..n {
                let o = self.small_obj();
                self.p.i(o);
            }
        }
        let n = self.g.below(3) as i32;
        self.p.i(n);
        for _ in 0..n {
            let o = self.small_obj();
            let t = self.g.below(2) as i32;
            let f = self.g.uniform(-3.0, 3.0);
            self.p.i(o);
            self.p.i(t);
            self.p.f(f);
        }
        let n = self.g.below(4) as i32;
        self.p.i(n);
        for _ in 0..n {
            let o = self.small_obj();
            let pl = self.g.below(np as u32) as i32;
            let s = self.g.below(5) as i32;
            self.p.i(o);
            self.p.i(pl);
            self.p.i(s);
        }
        for list in 0..2 {
            // the second list is the booby traps in range: kept short and over the first ids, so that a trap, its
            // producer and the BOOBY_TRAPPED status meet often enough to reach the detonation (planted bug 9)
            let n = if list == 1 { 2 + self.g.below(2) as i32 } else { self.g.below(3) as i32 };
            self.p.i(n);
            for _ in 0..n {
                let o = if list == 1 { 1 + self.g.below(3) as i32 } else { self.small_obj() };
                self.p.i(o);
            }
        }
        self.p.i(K_UPGRADES);
        let nt = self.g.below(4) as i32;
        self.p.i(nt);
        for _ in 0..nt {
            let x0 = self.g.below(200) as i32 - 50;
            let y0 = self.g.below(200) as i32 - 50;
            let w = 20 + self.g.below(150) as i32;
            let h = 20 + self.g.below(150) as i32;
            if self.chance(50) {
                self.p.i(4);
                for (x, y) in [(x0, y0), (x0 + w, y0), (x0 + w, y0 + h), (x0, y0 + h)] {
                    self.p.i(x);
                    self.p.i(y);
                }
            } else {
                self.p.i(3);
                for (x, y) in [(x0, y0), (x0 + w, y0 + h / 2), (x0 + w / 3, y0 + h)] {
                    self.p.i(x);
                    self.p.i(y);
                }
            }
        }
    }

    fn create(&mut self) {
        use rts_lockstep_engine::common::kind_of::*;
        let kinds = [
            KINDOF_STRUCTURE,
            KINDOF_INFANTRY,
            KINDOF_VEHICLE,
            KINDOF_PROJECTILE,
            KINDOF_MINE,
            KINDOF_BOOBY_TRAP,
            KINDOF_IMMOBILE,
            KINDOF_SELECTABLE,
            KINDOF_SHRUBBERY,
            KINDOF_DRONE,
            KINDOF_CAPTURABLE,
            KINDOF_CAN_ATTACK,
            KINDOF_INERT,
            KINDOF_REVEAL_TO_ALL,
            KINDOF_HERO,
            KINDOF_MOB_NEXUS,
            KINDOF_IGNORED_IN_GUI,
            KINDOF_MP_COUNT_FOR_VICTORY,
            KINDOF_PORTABLE_STRUCTURE,
            KINDOF_SPAWNS_ARE_THE_WEAPONS,
            KINDOF_CAN_BE_REPULSED,
            KINDOF_DEMOTRAP,
            KINDOF_WALK_ON_TOP_OF_WALL,
            KINDOF_ALWAYS_VISIBLE,
            KINDOF_ALWAYS_SELECTABLE,
            KINDOF_FS_FACTORY,
            KINDOF_FS_BARRACKS,
        ];
        self.p.i(1);
        let nk = self.g.below(5) as i32;
        self.p.i(nk);
        for _ in 0..nk {
            let k = kinds[self.g.below(kinds.len() as u32) as usize];
            self.p.i(k);
        }
        let v = self.g.uniform(0.0, 200.0);
        self.p.f(v);
        let sc = if self.chance(50) { -1.0 } else { self.g.uniform(0.0, 150.0) };
        self.p.f(sc);
        let ra = if self.chance(30) { self.g.uniform(1.0, 80.0) } else { 0.0 };
        self.p.f(ra);
        let gt = self.g.below(3) as i32;
        self.p.i(gt);
        let small = self.chance(50);
        self.p.b(small);
        for _ in 0..3 {
            let f = self.g.uniform(1.0, 30.0);
            self.p.f(f);
        }
        let ep = if self.chance(40) { self.g.below(8) as i32 } else { 0 };
        self.p.i(ep);
        let eb = self.g.below(5) as i32;
        self.p.i(eb);
        for _ in 0..2 {
            let c = self.g.below(900) as i32;
            self.p.i(c);
        }
        for _ in 0..2 {
            let c = self.g.below(4) as i32;
            self.p.i(c);
        }
        let ts = self.g.below(6) as i32;
        self.p.i(ts);
        let mut x = 0;
        for _ in 0..4 {
            let v = self.g.below(100) as i32;
            self.p.i(v);
        }
        for _ in 0..4 {
            x += self.g.below(60) as i32;
            self.p.i(x);
        }
        let tr = self.chance(70);
        self.p.b(tr);
        let eg = self.chance(20);
        self.p.b(eg);
        let od = self.g.below(40);
        self.p.u(od);
        let rp = self.g.below(5) as i32;
        self.p.i(rp);
        let nws = self.g.below(3) as i32;
        self.p.i(nws);
        for _ in 0..nws {
            let nf = self.g.below(3) as i32;
            self.p.i(nf);
            for _ in 0..nf {
                let f = self.g.below(17) as i32;
                self.p.i(f);
            }
            let has = self.chance(50);
            self.p.b(has);
        }
        let team = if self.chance(70) { self.pick_team() } else { 0 };
        self.p.i(team);
        let mut st = 0u64;
        if self.chance(40) {
            st |= 1 << self.g.below(45);
        }
        if self.chance(15) {
            st |= 1 << 3; // under construction
        }
        self.p.u64(st);
        let nm = 1 + self.g.below(4) as i32;
        self.p.i(nm);
        for m in 0..nm {
            let name = if m == 0 { if self.chance(15) { 2 } else { 0 } } else { self.g.below(13) as i32 };
            self.p.i(name);
            let mut ifc = 0i32;
            if m == 0 {
                ifc |= MODULEINTERFACE_BODY | MODULEINTERFACE_DAMAGE | MODULEINTERFACE_DIE;
            }
            for bit in [MODULEINTERFACE_DIE, MODULEINTERFACE_DAMAGE, MODULEINTERFACE_CREATE, MODULEINTERFACE_COLLIDE, MODULEINTERFACE_CONTAIN, MODULEINTERFACE_UPGRADE, MODULEINTERFACE_SPECIAL_POWER, MODULEINTERFACE_DESTROY] {
                if m > 0 && self.chance(30) {
                    ifc |= bit;
                }
            }
            self.p.i(ifc);
            let stealth = m > 0 && self.chance(8);
            self.p.b(stealth);
            let ai = m > 0 && self.chance(25);
            self.p.b(ai);
            let health = self.g.uniform(10.0, 500.0);
            self.p.f(health);
            let trig = if self.chance(40) { 1 + self.g.below(K_UPGRADES as u32) as i32 } else { 0 };
            self.p.i(trig);
            for _ in 0..1 {
                let b = self.chance(30);
                self.p.b(b);
            }
            let has = self.g.below(1 << 10);
            self.p.u(has);
            for _ in 0..8 {
                let b = self.chance(30);
                self.p.b(b);
            }
            let spt = if self.chance(30) { 1 + self.g.below(10) as i32 } else { 0 };
            self.p.i(spt);
            let shortcut = self.chance(30);
            self.p.b(shortcut);
            let pt = self.g.below(3);
            self.p.u(pt);
            // containment and riding only point at objects created later (a cycle recurses without end upstream:
            // `getTransportSlotCount` through the contained items, `clearDisabled` through the rider)
            let own = self.created + 1;
            let later = |g: &mut Gen| -> i32 { own + 1 + g.below((8 - own).max(1) as u32) as i32 };
            let rider = if self.chance(15) && own < 7 { later(self.g) } else { 0 };
            self.p.i(rider);
            let nc = if own < 7 { self.g.below(3) as i32 } else { 0 };
            self.p.i(nc);
            for _ in 0..nc {
                let o = later(self.g);
                self.p.i(o);
            }
            let slaver = if self.chance(15) { self.small_obj() } else { 0 };
            self.p.i(slaver);
            let deck = self.g.uniform(0.0, 20.0);
            self.p.f(deck);
            let ap = if self.chance(30) { self.g.below(self.players as u32) as i32 } else { -1 };
            self.p.i(ap);
            let cm = if self.chance(20) { self.small_obj() } else { 0 };
            self.p.i(cm);
            let cn = self.chance(15);
            self.p.b(cn);
        }
        self.created += 1;
    }

    fn obj(&mut self) -> i32 {
        self.g.below(self.created as u32) as i32
    }

    fn op(&mut self) {
        let choice = self.g.below(100);
        if choice < 4 && self.created < 7 {
            self.create();
            return;
        }
        if choice < 8 && self.created >= 2 {
            // scenario (added after planted bug 9 survived): object `a` is made a sticky bomb produced by `b`, `b` is
            // booby trapped and listed traps include `a` often, then `b` is hurt and killed
            let a = self.g.below(self.created as u32) as i32;
            let b = (a + 1 + self.g.below(self.created as u32 - 1) as i32) % self.created;
            self.p.i(57);
            self.p.i(a);
            self.p.i(0);
            self.p.i(1 + b);
            self.p.i(2);
            self.p.i(b);
            self.p.u64(1 << OBJECT_STATUS_BOOBY_TRAPPED);
            self.p.b(true);
            self.emit_op(15, b);
            self.emit_op(15, b);
            self.emit_op(17, b);
            return;
        }
        let k = self.obj();
        const NEW_OPS: [i32; 34] = [45, 46, 47, 48, 48, 49, 49, 50, 51, 52, 54, 55, 56, 57, 57, 57, 68, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 71, 72, 73];
        let op = if self.chance(40) {
            NEW_OPS[self.g.below(NEW_OPS.len() as u32) as usize]
        } else {
            self.old_op()
        };
        self.emit_op(op, k);
    }

    fn old_op(&mut self) -> i32 {
        match self.g.below(42) {
            0 | 1 => 2,
            2 => 3,
            3 | 4 => 4,
            5 => 5,
            6 | 7 => 6,
            8 => 7,
            9 | 10 => 8,
            11 => 9,
            12 => 10,
            13 => 11,
            14 | 15 => 12,
            16 => 13,
            17 => 14,
            18..=20 => 15,
            21 => 16,
            22 => 17,
            23 => 18,
            24 => 19,
            25 | 26 => 20,
            27 => 21,
            28 => 22,
            29 => 23,
            30 => 24,
            31 => 25,
            32 => 26,
            33 => 27,
            34 => 28,
            35 => 29,
            36 => 30,
            37 => 31,
            38 => 32,
            39 => 33,
            40 => 34,
            _ => match self.g.below(8) {
                0 => 35,
                1 => 36,
                2 => 37,
                3 => 38,
                4 => 39,
                5 => 41,
                6 => 42,
                _ => 44,
            },
        }
    }

    fn emit_op(&mut self, op: i32, k: i32) {
        if op == 38 {
            // processDestroyList takes no object
            self.p.i(38);
            self.p.i(k);
            return;
        }
        self.p.i(op);
        self.p.i(k);
        match op {
            2 => {
                let mut m = 0u64;
                // a third of the bits come from the ones that gate rare branches (maintainer-visible bias, added after
                // planted bug 9 survived: booby trap, repulsor, stealth, disguise, construction, car bomb ...)
                const RARE: [u32; 14] = [3, 5, 7, 9, 16, 17, 19, 21, 22, 27, 28, 41, 43, 6];
                for _ in 0..1 + self.g.below(2) {
                    let bit = if self.chance(35) { RARE[self.g.below(RARE.len() as u32) as usize] } else { self.g.below(45) };
                    m |= 1 << bit;
                }
                m &= !1; // OBJECT_STATUS_NONE has no bit of its own
                let set = self.chance(60);
                self.p.u64(m);
                self.p.b(set);
            }
            3 => {
                let bit = self.g.below(5) as i32;
                let set = self.chance(60);
                self.p.i(bit);
                self.p.b(set);
            }
            4 => {
                let t = self.g.below(13) as i32;
                let f = self.g.below(300);
                self.p.i(t);
                self.p.u(f);
            }
            5 => {
                let t = self.g.below(13) as i32;
                self.p.i(t);
            }
            6 => {
                let t = self.g.below(13) as i32;
                self.p.i(t);
            }
            8 => {
                let f = self.g.below(400);
                self.p.u(f);
            }
            9 => {
                let wst = self.g.below(17) as i32;
                let set = self.chance(60);
                self.p.i(wst);
                self.p.b(set);
            }
            10 => {
                let wbc = self.g.below(27) as i32;
                let set = self.chance(60);
                self.p.i(wbc);
                self.p.b(set);
            }
            11 => {
                let ast = self.g.below(8) as i32;
                let mode = self.g.below(3) as i32;
                self.p.i(ast);
                self.p.i(mode);
            }
            12 => {
                let t = self.pick_team();
                let mode = self.g.below(3) as i32;
                self.p.i(t);
                self.p.i(mode);
            }
            13 => {
                let t = self.pick_team();
                let time = self.g.below(100);
                self.p.i(t);
                self.p.u(time);
            }
            14 => {
                let time = self.g.below(100);
                self.p.u(time);
            }
            15 => {
                let amount = if self.chance(15) { self.g.uniform(100.0, 600.0) } else { self.g.uniform(0.0, 120.0) };
                self.p.f(amount);
                let dt = match self.g.below(5) {
                    0 => 10,
                    1 => 17,
                    _ => self.g.below(38) as i32,
                };
                self.p.i(dt);
                let kill = self.chance(10);
                self.p.b(kill);
                let src = if self.chance(60) { self.small_obj() } else { 0 };
                self.p.i(src);
                let (a, r) = if self.chance(50) { (self.g.uniform(0.0, 5.0), self.g.uniform(1.0, 50.0)) } else { (0.0, 0.0) };
                self.p.f(a);
                self.p.f(r);
                let taper = self.g.uniform(0.0, 1.0);
                self.p.f(taper);
                for _ in 0..3 {
                    let v = self.g.uniform(-10.0, 10.0);
                    self.p.f(v);
                }
                let mask = self.g.below(16);
                self.p.u(mask);
            }
            16 => {
                let a = self.g.uniform(0.0, 100.0);
                let src = if self.chance(50) { 1 + self.obj() } else { 0 };
                self.p.f(a);
                self.p.i(src);
            }
            17 => {
                let dt = if self.chance(70) { 11 } else { self.g.below(38) as i32 };
                self.p.i(dt);
                self.p.i(0);
            }
            19 => {
                let a = self.g.uniform(0.0, 200.0);
                let dt = self.g.below(38) as i32;
                self.p.f(a);
                self.p.i(dt);
            }
            20 => {
                for _ in 0..2 {
                    let v = self.g.uniform(-60.0, 260.0);
                    self.p.f(v);
                }
                let z = self.g.uniform(-5.0, 30.0);
                self.p.f(z);
            }
            21 => {
                let a = self.g.uniform(-7.0, 7.0);
                self.p.f(a);
            }
            22 => {
                let z = self.g.uniform(-5.0, 30.0);
                self.p.f(z);
            }
            23 => {
                let kind = self.g.below(3) as i32;
                let v = match kind {
                    0 => self.g.below(120) as i32,
                    1 => self.g.below(4) as i32,
                    _ => self.g.below(200) as i32,
                };
                self.p.i(kind);
                self.p.i(v);
            }
            24 => {
                let v = self.obj();
                self.p.i(v);
            }
            25 | 26 | 28 => {
                let bit = self.g.below(K_UPGRADES as u32) as i32;
                self.p.i(bit);
            }
            29 => {
                let by = self.obj();
                self.p.i(by);
            }
            31 => {
                let kind = self.g.below(3) as i32;
                let v = self.g.uniform(0.0, 150.0);
                self.p.i(kind);
                self.p.f(v);
            }
            32 => {
                let setting = self.chance(60);
                let pl = self.g.below(self.players as u32) as i32;
                self.p.b(setting);
                self.p.i(pl);
            }
            34 => {
                let s = self.chance(50);
                self.p.b(s);
            }
            35 => {
                let mc = self.g.below(100) as i32;
                let frames = self.g.below(40);
                self.p.i(mc);
                self.p.u(frames);
            }
            39 => {
                let other = self.obj();
                self.p.i(other);
            }
            41..=43 => {
                let b = self.chance(50);
                self.p.b(b);
            }
            48 => {
                let kind = self.g.below(4) as i32;
                let tmpl = self.g.below(3) as i32;
                let forced = self.chance(30);
                let opts = self.g.below(4);
                let target = self.obj();
                self.p.i(kind);
                self.p.i(tmpl);
                self.p.b(forced);
                self.p.u(opts);
                self.p.i(target);
                for _ in 0..4 {
                    let v = self.g.uniform(-50.0, 50.0);
                    self.p.f(v);
                }
            }
            49 => {
                let kind = self.g.below(13) as i32;
                let ty = self.g.below(12) as i32;
                let tmpl = self.g.below(3) as i32;
                self.p.i(kind);
                self.p.i(ty);
                self.p.i(tmpl);
            }
            50 => {
                for _ in 0..3 {
                    let v = self.g.uniform(-1.0, 1.0);
                    self.p.f(v);
                }
                let speed = self.g.uniform(0.0, 5.0);
                let opts = self.g.below(4);
                self.p.f(speed);
                self.p.u(opts);
            }
            51 => {
                let other = self.obj();
                let tt = self.g.below(3) as i32;
                self.p.i(other);
                self.p.i(tt);
            }
            54 => {
                let kind = self.g.below(2) as i32;
                let gt = self.g.below(3) as i32;
                let small = self.chance(50);
                self.p.i(kind);
                self.p.i(gt);
                self.p.b(small);
                for _ in 0..4 {
                    let v = self.g.uniform(1.0, 40.0);
                    self.p.f(v);
                }
            }
            55 => {
                let kind = self.g.below(3) as i32;
                let color = if self.chance(70) { self.g.u32() | 0xff00_0000 } else { 0 };
                self.p.i(kind);
                self.p.u(color);
            }
            56 => {
                let kind = self.g.below(2) as i32;
                let g = 1 + self.g.below(3) as i32;
                self.p.i(kind);
                self.p.i(g);
            }
            57 => {
                let which = self.g.below(2) as i32;
                let obj = if self.chance(80) { 1 + if self.chance(50) { self.g.below(3) as i32 } else { self.obj() } } else { 0 };
                self.p.i(which);
                self.p.i(obj);
            }
            58 => {
                let kind = self.g.below(2) as i32;
                let layer = [0, 1, 2, 15][self.g.below(4) as usize];
                self.p.i(kind);
                self.p.i(layer);
            }
            59 => {
                let kind = self.g.below(2) as i32;
                self.p.i(kind);
            }
            60 | 61 => {
                let b = self.chance(50);
                self.p.b(b);
            }
            62 => {
                let a = self.g.below(self.players as u32) as i32;
                let b = self.g.below(self.players as u32) as i32;
                self.p.i(a);
                self.p.i(b);
            }
            63 => {
                let amount = self.g.uniform(0.0, 50.0);
                let src = if self.chance(80) { 1 + self.obj() } else { 0 };
                let duration = self.g.below(60);
                self.p.f(amount);
                self.p.i(src);
                self.p.u(duration);
            }
            64 => {
                let kind = self.g.below(3) as i32;
                let other = self.obj();
                self.p.i(kind);
                self.p.i(other);
            }
            65 => {
                // a rotation about Z and a translation (a transform nothing has to normalise)
                let a = self.g.uniform(-3.0, 3.0);
                let (c, s) = (a.cos(), a.sin());
                let (x, y, z) = (self.g.uniform(-60.0, 260.0), self.g.uniform(-60.0, 260.0), self.g.uniform(-5.0, 30.0));
                for v in [c, -s, 0.0, x, s, c, 0.0, y, 0.0, 0.0, 1.0, z] {
                    self.p.f(v);
                }
            }
            66 => {
                let kind = self.g.below(3) as i32;
                let trig = self.g.below(4) as i32;
                self.p.i(kind);
                self.p.i(trig);
            }
            68 => {
                let victim = if self.chance(70) { 1 + self.obj() } else { 0 };
                self.p.i(victim);
            }
            69 => {
                let kind = self.g.below(3) as i32;
                let amount = self.g.uniform(-5.0, 30.0);
                let status = self.g.below(27) as i32;
                let dur = self.g.below(60);
                self.p.i(kind);
                self.p.f(amount);
                self.p.i(status);
                self.p.u(dur);
            }
            70 => {
                let turret = self.g.below(2) as i32;
                let rot = self.g.uniform(-3.0, 3.0);
                self.p.i(turret);
                self.p.f(rot);
            }
            71 => {
                let other = self.obj();
                let player = self.g.below(self.players as u32) as i32;
                let dtype = self.g.below(13) as i32;
                self.p.i(other);
                self.p.i(player);
                self.p.i(dtype);
            }
            _ => {}
        }
    }
}

fn gen_program(seed: u64, ops: u32) -> Vec<u32> {
    let mut g = Gen::new(seed);
    let mut pg = Pg { g: &mut g, p: Prog::default(), players: 0, teams: Vec::new(), created: 0 };
    pg.header();
    let initial = 2 + pg.g.below(3);
    for _ in 0..initial {
        pg.create();
    }
    for _ in 0..ops {
        pg.op();
    }
    pg.p.0
}

// ---- the comparison --------------------------------------------------------------------------------------------------

/// The first line where the two differ, with context.
fn first_difference(a: &[String], b: &[String]) -> Option<String> {
    let n = a.len().min(b.len());
    for i in 0..n {
        if a[i] != b[i] {
            let from = i.saturating_sub(6);
            let mut s = format!("line {i}:\n  rust: {}\n  cpp : {}\ncontext (cpp):\n", a[i], b[i]);
            for l in &b[from..i] {
                s.push_str(&format!("    {l}\n"));
            }
            s.push_str("the operations before it (cpp):\n");
            for l in b[..i].iter().filter(|l| l.starts_with("end Op")).rev().take(8).collect::<Vec<_>>().into_iter().rev() {
                s.push_str(&format!("    {l}\n"));
            }
            return Some(s);
        }
    }
    if a.len() != b.len() {
        return Some(format!("length: rust {} lines, cpp {} lines; next {:?} / {:?}", a.len(), b.len(), a.get(n), b.get(n)));
    }
    None
}

/// Reruns a program with the snapshot bytes on, and says where the first pair of snapshots that differ differs.
fn explain_snapshot(prog: &[u32]) -> String {
    let (c, r) = (run_cpp(prog, true), run_rust(prog, true));
    let unhex = |l: &str| -> Vec<u8> {
        let hex = l.rsplit(' ').next().unwrap_or("");
        (0..hex.len() / 2).map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap_or(0)).collect()
    };
    let (cb, rb): (Vec<&String>, Vec<&String>) = (c.iter().filter(|l| l.starts_with("bytes")).collect(), r.iter().filter(|l| l.starts_with("bytes")).collect());
    for (i, (a, b)) in cb.iter().zip(rb.iter()).enumerate() {
        if a != b {
            let (a, b) = (unhex(a), unhex(b));
            let at = a.iter().zip(b.iter()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
            let from = at.saturating_sub(24);
            return format!("  snapshot #{i} differs at byte {at} of {}:\n    cpp : {:02x?}\n    rust: {:02x?}", a.len(), &a[from..(at + 24).min(a.len())], &b[from..(at + 24).min(b.len())]);
        }
    }
    String::from("  (the bytes do not differ)")
}

#[test]
fn object_matches_upstream() {
    let cases: u64 = std::env::var("OBJECT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(1200);
    let (mut lines, mut kinds): (usize, BTreeMap<String, usize>) = (0, BTreeMap::new());
    let mut bad = 0;
    let mut reentrant = 0u64;
    for case in 0..cases {
        let prog = gen_program(0x0B1E_C700 + case, 110);
        if let Ok(dir) = std::env::var("OBJECT_DUMP_ALL") {
            let bytes: Vec<u8> = prog.iter().flat_map(|w| w.to_le_bytes()).collect();
            std::fs::write(format!("{dir}/prog_{case}.bin"), bytes).expect("dump program");
            std::fs::write(format!("{dir}/names.bin"), flag_names()).expect("dump names");
        }
        if std::env::var("OBJECT_TRACE").is_ok() { eprintln!("CASE {case}"); }
        let c = run_cpp(&prog, false);
        let (r, reentered, skipped) = run_rust_counted(&prog, false);
        reentrant += u64::from(reentered);
        assert_eq!(skipped, 0, "case {case}: a module call was not delivered");
        lines += c.len();
        for l in &c {
            let tag: Vec<&str> = l.split(' ').take(2).collect();
            *kinds.entry(tag.join(" ")).or_default() += 1;
        }
        if let Some(d) = first_difference(&r, &c) {
            bad += 1;
            if let Ok(dir) = std::env::var("OBJECT_DUMP") {
                let bytes: Vec<u8> = prog.iter().flat_map(|w| w.to_le_bytes()).collect();
                std::fs::write(format!("{dir}/prog_{case}.bin"), bytes).expect("dump program");
                std::fs::write(format!("{dir}/names.bin"), flag_names()).expect("dump names");
            }
            if bad <= 3 {
                println!("DIFF case {case}: {d}");
                if d.contains("snap Object") {
                    println!("{}", explain_snapshot(&prog));
                }
            }
        }
    }
    println!("RESULT {cases} programs, {lines} lines compared, {bad} programs differ");
    println!("RESULT re-entrant module calls run at once (Rust side): {reentrant}");
    assert!(reentrant > 0, "the re-entrant path was never exercised");
    for (k, n) in &kinds {
        println!("RESULT   {k}: {n}");
    }
    assert_eq!(bad, 0);
}
