#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Body/ActiveBody.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `ActiveBody` (`logic::modules::active_body`) against the unchanged C++, `p1w13-sonnet`, batch P.
//!
//! # Method
//!
//! The C++ side is `ActiveBody.cpp` with one mechanical edit made by `build.rs` into `mod_gen` (the seven function-local `static` particle system templates lose their
//! `static`; asserted seven times), compiled into `wrap_module.cpp`'s unit (`differential/l9_active_body_p.inc`) on the module build's stand-ins, with the REAL
//! `Armor` and `DamageInfo::xfer` (the Armor archive), the REAL `DamageFX` functions (cut verbatim from `DamageFX.cpp`), the REAL `ArmorTemplateSetFinder`
//! (`SparseMatchFinder.h`), the REAL `Object::getRelationship` and `Object::kill` (cut from `Object.cpp`), the REAL `BridgeBehavior::getBridgeBehaviorInterfaceFromObject`
//! (cut from `BridgeBehavior.cpp`), `INI`, `BitFlags` and `Xfer`. A scene of up to four objects is built, each with an `ActiveBody`, a template with armor sets, a team, a
//! player, optionally a drawable, an AI, a contain module and up to two probe behaviours (damage module, die module, bridge behaviour, projectile interface). Every call the
//! body makes on the rest of the world is a line of the effect log, in the order it is made.
//!
//! The Rust side is the real thing: a `World` with the same scene (`World::new_object` with the body from `modules::new_module` and `ProbeModule`s), the cut points of
//! `body_cuts` answered from the same tables, `cuts.body.stub_object_calls` set (the Object-level calls that cascade are records only; see `body_cuts`), and the ops run
//! as `Object` runs them: `World::attempt_damage`, `World::module_call`. The scene is one list of words (`cfg`); the Rust side parses it with a reader of its own.
//!
//! Each case reads a `D` block (`MaxHealth`, `InitialHealth`, `SubdualDamageCap`, `SubdualDamageHealRate`, `SubdualDamageHealAmount`; mostly well formed) for the
//! first object and runs 1 to about 70 ops: damage, healing and kill hits of every type from every source, estimates, direct health, state, maximum, initial health,
//! indestructibility, aflame, veterancy, armor flags, crushing, the last attacker, the visual condition and its particle systems, deletion, the environment (the frame,
//! status bits, players, the tables the cut points answer, the probes' answers, the selection, the container) and the three xfers.
//!
//! Compared, bit for bit: the read's throw; the data; per op the throw status, a result word, the effects in order, the xfer status, bytes and CRC, and the state of every
//! object (the body's fields, its last damage, the status bits, the effectively-dead flag, the geometry height) and every player (who attacked it and when).
//!
//! # Deviations the test cannot see
//!
//! The Object-level calls that cascade are stubbed on the Rust side (`setDisabled`, `clearDisabled`, `doStatusDamage`, `notifySubdualDamage`, `scoreTheKill`, `onDie`,
//! `setTeam`, `setStatus`, the neutral player's academy statistics): the C++ side stands in for them with a log line, and each has its own differential test. The retaliation
//! scan, `getAbleToAttackSpecificObject`, `getDistanceSquared`, the bones of the drawable and the particle system manager are stand-ins on both sides (the partition manager,
//! the weapon layer, the drawable and the particle system manager are verified by their own tests): the body's use of them is what is under test.
//!
//! Measured 2026-10-22 (fourth run, after `UndeadBody` and `HiveStructureBody` joined the scene: the C++ side also runs the unchanged `UndeadBody.cpp` (one mechanical edit
//! by `build.rs`: its second loop declares its own `update`, as Visual C++ 6 let the first loop's outlive it; asserted once) and `HiveStructureBody.cpp`), 9,000 cases
//! (`L9AB_CASES`, seed `0x00AB_0D1E`), the unchanged `ActiveBody.cpp` (edited only by `build.rs`) against the Rust module:
//!
//! - data reads: 8,465 compared, 0 mismatches (583 data blocks that upstream's parser throws on, the same message on both sides);
//! - op outputs: 1,343,857 compared, 0 mismatches, max ULP 0, no NaN-payload-only differences;
//! - 258,170 ops, 90,050 damage hits, 6,054 deaths, 4,592 kills scored, 8,478 xfers (save, load, CRC), 897,087 effect lines, 8,688 fx, 204,862 particle systems,
//!   722 retaliation checks, 8,452 subdual hits; among the effect lines 629 slow deaths begun (`UndeadBody`'s second life), 3,577 closest-slave and 2,282 closest-rider
//!   questions (`HiveStructureBody`);
//! - 535 cases excluded: upstream-undefined input (a null AI, an empty contained list, an out-of-range array index, a `Real` outside `Int`), counted Rust-only.
//!
//! ~~Measured 2026-10-22 (third run): 8,446 data reads, 1,352,101 op outputs, 554 excluded~~ **CORRECTED 2026-10-22**: the numbers above are the run after the two classes
//! were added; the third run's figures were for the scene without them.
//!
//! Interpretation: every line the body sends to the rest of the game (and every value it returns or saves) agrees bit for bit with upstream over this input space.
//! The retaliation scan, the able-to-attack and distance answers, the bones and the particle manager are the test's stand-ins on both sides, so this does not verify them.
//!
//! # The derived classes
//!
//! Each object's body is one of `ActiveBody`, `HighlanderBody`, `ImmortalBody`, `StructureBody`, `InactiveBody`, `UndeadBody`, `HiveStructureBody` (the scene's last word per
//! object, then the class's own words; class 4 is the `InactiveBody`, whose state the driver and the test read through the body's virtuals: the same 18 words, the Rust side
//! asking the module with `ModuleCall`s), with op 36 `setConstructorObject` and, as the last word of the object's state, the structure's (and hive's) constructor object id or
//! the undead body's `m_isSecondLife`. The block of the INI file names the class of the first object's data (`D` `ActiveBodyModuleData`, `U` `UndeadBodyModuleData` with
//! `SecondLifeMaxHealth`, `H` `HiveStructureBodyModuleData` with the two `DamageTypeFlags` fields), and the first object's body follows it. A load only takes bytes that a
//! body of the same class saved: another class's bytes are shifted fields, and a flag read from a byte that is not 0 or 1 is not a `bool` in C++ (the C++ side prints `0x3c`
//! where Rust, which normalises, has `true`).
//!
//! The probes grew what the two classes ask of the object's other modules: a probe may be a `SlowDeathBehaviorInterface` (which damage types it is applicable to, its
//! probability modifier; `beginSlowDeath` is a log line), and one probe may be the object's spawn behaviour (`getClosestSlave` logs its position and answers a scene object);
//! the contain module's `getClosestRider` does the same. A hive's slave or rider is always a LATER object (a hive that passed damage on to itself, or in a ring, would recurse
//! for ever upstream). The result word of a damage hit (op 0) is the caller's view afterwards (the amount the body limited, the damage dealt and clipped, `noEffect`), and each
//! damage info starts with scene-chosen output fields, so a body that leaves them alone (a hive swallowing damage) is told from one that zeroes them.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` flips one branch of `active_body.rs`; each must make the test fail at 9,000 cases (1212-1253, 1261-1267, 1271-1282, 1331-1347 and 1351-1360 all caught except as noted; re-run 2026-10-22 on the final binary):
//!
//! | id | planted bug |
//! |---|---|
//! | 1212 | armor set without an armor template keeps the old armor |
//! | 1213 | damage FX not refreshed with the armor set |
//! | 1214 | rubble does not set NO_COLLISIONS |
//! | 1215 | damage FX override of UNRESISTABLE not skipped |
//! | 1216 | damage FX throttle comparison inverted |
//! | 1217-1223 | indestructible, onDie, kill, moving guard, retaliation friend loop, kill count cap, dead check |
//! | 1224-1229 | subdual frames floor not ceil, cannot-be-subdued early return, unresistable branches, kill, actual damage clipped |
//! | 1230-1241 | last-damage timestamp window, faction-structure test, last attacker, damage-state change test, particle cap, died test, subdual/garrison/sniper branches |
//! | 1242-1248 | setDamageState REALLYDAMAGED ratio, setInitialHealth base, setMaxHealth ratio / add / full-heal / cap |
//! | 1261-1263 | Highlander: the limit applied to UNRESISTABLE instead of the rest; limit by health not health - 1; `max` for `min` |
//! | 1264-1265 | Immortal: limit by -health (not -health + 1); `min` for `max` |
//! | 1266-1267 | Structure: `setConstructorObject(NULL)` overwrites the id; the id is not saved |
//! | 1271-1282 | InactiveBody: the object not made effectively dead; the unresistable estimate; the damage outputs not zeroed; `noEffect` kept on the kill; the die modules run on every kill; `m_dieCalled` never set; a damage-type healing call not sent on to damage; a damage state other than pristine; not indestructible; the version/flag saved (`m_dieCalled` is not); the damage scalar ignored |
//! | 1331-1340 | Undead: the unresistable test inverted; the first-life test dropped; `>` for `>=`; the health-damaging test dropped; the limit by health not health - 1; the second life started before the damage; `m_isSecondLife` never set; the first life's maximum health kept; `PRESERVE_RATIO` for `FULLY_HEAL`; the armor flag not set |
//! | 1341-1347 | Undead: non-applicable behaviours counted in the total; the roll from 0; the modifier replaced by 1; `<` for `<=` on the roll; the first applicable behaviour always begun; the loop not left after the first begin; `m_isSecondLife` not saved |
//! | 1351-1360 | Hive: the swallow does not zero the clipped damage; `noEffect` not set; the propagation test inverted; the swallow test inverted (spawn branch); the swallow set used for propagation; the contain branch taken before the spawn branch; the origin used for the shooter's position; a missing shooter not stopping the propagation; the damage also taken by the hive after passing it on; the swallow test inverted (contain branch) |
//! | 1249-1253 | veterancy bonus ratio inverted, promoted-hero sound, UI dirty (selected, contained-by), promotion feedback |
//!
//! Not planted, equivalent mutants (1273 survived, removed): in `InactiveBody::attemptDamage`, sending a `DAMAGE_HEALING` call on to `attemptHealing` (the fall-through zeroes the same outputs, `noEffect` true); and the `max(desiredHealth, 0)` clamp in `setDamageState` (1243 survived): `internalChangeHealth` clamps the result to 0 anyway, so removing it
//! cannot be seen from outside. Its plant was removed and the clamp stays as upstream has it.

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::damage_fx::{Dfx, DamageFx, DamageFxStore};
use rts_lockstep_engine::common::game_common::{PlayerType, Relationship, VeterancyLevel};
use rts_lockstep_engine::common::game_type::{ObjectId, INVALID_DRAWABLE_ID, INVALID_ID};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, MODULEINTERFACE_CONTAIN, MODULEINTERFACE_DAMAGE, MODULEINTERFACE_DIE};
use rts_lockstep_engine::common::object_status_types::*;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate, ThingTemplateAudioType};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::armor::{ArmorStore, ArmorTemplate};
use rts_lockstep_engine::logic::armor_set::ArmorSetFlags;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::damage::*;
use rts_lockstep_engine::logic::modules::active_body::ActiveBodyModuleData;
use rts_lockstep_engine::logic::modules::derived_bodies::{HiveStructureBodyModuleData, UndeadBodyModuleData};
use rts_lockstep_engine::logic::world::body_cuts::{info_line, AutoParticle, BodyCall};
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleReply};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9ab_p.bin");
const XFER_FILE: &str = "l9ab_p.bin";

/// The kinds and statuses the scene's masks name (local bit i is `KINDS[i]`, `STATUSES[i]`), as the driver's `kAbKinds` and `kAbStatus`.
const KINDS: [i32; 13] = [
    KINDOF_VEHICLE,
    KINDOF_STRUCTURE,
    KINDOF_PROJECTILE,
    KINDOF_BRIDGE,
    KINDOF_BRIDGE_TOWER,
    KINDOF_CAN_BE_REPULSED,
    KINDOF_DRONE,
    KINDOF_CANNOT_RETALIATE,
    KINDOF_IMMOBILE,
    KINDOF_FS_INTERNET_CENTER,
    KINDOF_INFANTRY,
    KINDOF_FS_FACTORY,
    KINDOF_SELECTABLE,
];
const STATUSES: [i32; 8] = [
    OBJECT_STATUS_AIRBORNE_TARGET,
    OBJECT_STATUS_STEALTHED,
    OBJECT_STATUS_DETECTED,
    OBJECT_STATUS_IS_USING_ABILITY,
    OBJECT_STATUS_UNDER_CONSTRUCTION,
    OBJECT_STATUS_AFLAME,
    OBJECT_STATUS_NO_COLLISIONS,
    OBJECT_STATUS_REPULSOR,
];
const NEUTRAL_TEAM: u32 = 9;
const PS_CLASSES: usize = 7;
const ARMORS: usize = 3;
const FXS: usize = 2;
const TEMPLATES: usize = 3;
const INFOS: usize = 12;

// ---- the scene, as words -------------------------------------------------------------------------------------------------------------------------------------------

fn bits(x: f32) -> u32 {
    x.to_bits()
}
fn f(x: u32) -> f32 {
    f32::from_bits(x)
}

/// Reads the words of a scene, in the order the driver reads them (`l9abRunInner`).
struct R<'a> {
    w: &'a [u32],
    i: usize,
}
impl R<'_> {
    fn u(&mut self) -> u32 {
        let v = self.w[self.i];
        self.i += 1;
        v
    }
    fn s(&mut self) -> i32 {
        self.u() as i32
    }
    fn f(&mut self) -> f32 {
        f(self.u())
    }
}

struct Obj {
    tmpl: usize,
    kinds: u32,
    status: u32,
    player: i32,
    vet: i32,
    pos: [f32; 3],
    contained_by: i32,
    drawable: bool,
    has_ai: bool,
    moving: bool,
    idle: bool,
    has_contain: bool,
    garrisonable: bool,
    immune: bool,
    rider_change: bool,
    n_probes: usize,
    items: Vec<i32>,
    probe_bridge: [bool; 2],
    probe_proj: [bool; 2],
    towers: [i32; 4],
    body: [u32; 5],
    /// 0 `ActiveBody`, 1 `HighlanderBody`, 2 `ImmortalBody`, 3 `StructureBody`, 4 `InactiveBody`, 5 `UndeadBody`, 6 `HiveStructureBody`
    cls: u32,
    /// `SecondLifeMaxHealth` of an `UndeadBody`'s data (float bits).
    second_life: u32,
    /// The damage types a `HiveStructureBody` propagates / swallows, as bit sets (`DamageTypeFlags`).
    propagate: u64,
    swallow: u64,
    /// The probe (1-based; 0 none) that is the object's spawn behaviour, and the objects it and the contain module answer as the closest slave and rider.
    spawn: u32,
    closest_slave: i32,
    closest_rider: i32,
    /// Per probe: a slow death behaviour, the damage types it is applicable to (bit set), its probability modifier.
    probe_slow: [bool; 2],
    slow_mask: [u64; 2],
    slow_prob: [i32; 2],
}

struct Tmpl {
    rubble: i8,
    audio: [u32; 6],
    sets: Vec<(u32, u32, u32)>,
}

struct FxCfg {
    default: (f32, u32, u32, u32),
    overrides: Vec<(i32, i32, f32, u32, u32, u32)>,
}

/// A damage info of the scene: source, type, death type, FX override, amount, kill, status type, and the dealt and clipped outputs it starts with.
type InfoCfg = (u32, i32, i32, i32, f32, bool, i32, f32, f32);

struct Scene {
    frame: u32,
    logic_seed: u32,
    client_seed: u32,
    unit_damaged: f32,
    unit_really_damaged: f32,
    rubble_default: f32,
    health_bonus: [f32; 4],
    repulsors: bool,
    friends_radius: f32,
    max_retaliate: f32,
    ps: [(i32, u32); PS_CLASSES],
    neutral_team: i32,
    armors: Vec<[f32; DAMAGE_NUM_TYPES]>,
    fxs: Vec<FxCfg>,
    tmpls: Vec<Tmpl>,
    players: [(u32, bool); 4],
    objs: Vec<Obj>,
    rel: Vec<Vec<i32>>,
    infos: Vec<InfoCfg>,
    able: Vec<Vec<i32>>,
    dist: Vec<Vec<f32>>,
    bones: Vec<(i32, i32, Vec<[f32; 3]>)>,
    dead_mod: u32,
    dead_rem: u32,
}

fn parse_scene(cfg: &[u32]) -> Scene {
    let mut r = R { w: cfg, i: 0 };
    let frame = r.u();
    let (logic_seed, client_seed) = (r.u(), r.u());
    let (unit_damaged, unit_really_damaged, rubble_default) = (r.f(), r.f(), r.f());
    let health_bonus = [r.f(), r.f(), r.f(), r.f()];
    let repulsors = r.u() != 0;
    let (friends_radius, max_retaliate) = (r.f(), r.f());
    let mut ps = [(0, 0); PS_CLASSES];
    for p in ps.iter_mut() {
        *p = (r.s(), r.u());
    }
    let neutral_team = r.s();
    let mut armors = Vec::new();
    for _ in 0..r.u() {
        let mut a = [0.0; DAMAGE_NUM_TYPES];
        for c in a.iter_mut() {
            *c = r.f();
        }
        armors.push(a);
    }
    let mut fxs = Vec::new();
    for _ in 0..r.u() {
        let default = (r.f(), r.u(), r.u(), r.u());
        let mut overrides = Vec::new();
        for _ in 0..r.u() {
            overrides.push((r.s(), r.s(), r.f(), r.u(), r.u(), r.u()));
        }
        fxs.push(FxCfg { default, overrides });
    }
    let mut tmpls = Vec::new();
    for _ in 0..r.u() {
        let rubble = r.s() as i8;
        let mut audio = [0; 6];
        for a in audio.iter_mut() {
            *a = r.u();
        }
        let mut sets = Vec::new();
        for _ in 0..r.u() {
            sets.push((r.u(), r.u(), r.u()));
        }
        tmpls.push(Tmpl { rubble, audio, sets });
    }
    let mut players = [(0, false); 4];
    for p in players.iter_mut() {
        *p = (r.u(), r.u() != 0);
    }
    let n = r.u() as usize;
    let mut objs = Vec::new();
    for _ in 0..n {
        let (tmpl, kinds, status, player, vet) = (r.u() as usize, r.u(), r.u(), r.s(), r.s());
        let pos = [r.f(), r.f(), r.f()];
        let (contained_by, drawable, has_ai, moving, idle, has_contain, garrisonable, immune, rider_change) =
            (r.u() as i32, r.u() != 0, r.u() != 0, r.u() != 0, r.u() != 0, r.u() != 0, r.u() != 0, r.u() != 0, r.u() != 0);
        let n_probes = r.u() as usize;
        let n_items = r.u() as usize;
        let items_all = [r.u(), r.u(), r.u()];
        let items = items_all[..n_items].iter().map(|&i| i as i32).filter(|&i| i != 0).collect();
        let (b0, p0, b1, p1) = (r.u() != 0, r.u() != 0, r.u() != 0, r.u() != 0);
        let towers = [r.u() as i32, r.u() as i32, r.u() as i32, r.u() as i32];
        let body = [r.u(), r.u(), r.u(), r.u(), r.u()];
        let cls = r.u();
        let second_life = r.u();
        let (p_lo, p_hi, s_lo, s_hi) = (r.u(), r.u(), r.u(), r.u());
        let (spawn, closest_slave, closest_rider) = (r.u(), r.u() as i32, r.u() as i32);
        let (mut probe_slow, mut slow_mask, mut slow_prob) = ([false; 2], [0u64; 2], [0i32; 2]);
        for i in 0..2 {
            probe_slow[i] = r.u() != 0;
            let (lo, hi) = (r.u(), r.u());
            slow_mask[i] = (u64::from(hi) << 32) | u64::from(lo);
            slow_prob[i] = r.s();
        }
        objs.push(Obj {
            tmpl,
            kinds,
            status,
            player,
            vet,
            pos,
            contained_by,
            drawable,
            has_ai,
            moving,
            idle,
            has_contain,
            garrisonable,
            immune,
            rider_change,
            n_probes,
            items,
            probe_bridge: [b0, b1],
            probe_proj: [p0, p1],
            towers,
            body,
            cls,
            second_life,
            propagate: (u64::from(p_hi) << 32) | u64::from(p_lo),
            swallow: (u64::from(s_hi) << 32) | u64::from(s_lo),
            spawn,
            closest_slave,
            closest_rider,
            probe_slow,
            slow_mask,
            slow_prob,
        });
    }
    let mut rel = vec![vec![0; n]; n];
    for row in rel.iter_mut() {
        for c in row.iter_mut() {
            *c = r.s();
        }
    }
    let mut infos = Vec::new();
    for _ in 0..r.u() {
        infos.push((r.u(), r.s(), r.s(), r.s(), r.f(), r.u() != 0, r.s(), r.f(), r.f()));
    }
    let mut able = vec![vec![0; n]; n];
    for row in able.iter_mut() {
        for c in row.iter_mut() {
            *c = r.s();
        }
    }
    let mut dist = vec![vec![0.0; n]; n];
    for row in dist.iter_mut() {
        for c in row.iter_mut() {
            *c = r.f();
        }
    }
    let mut bones = Vec::new();
    for _ in 0..r.u() {
        let (ob, cls) = (r.s(), r.s());
        let cnt = r.u();
        let v = (0..cnt).map(|_| [r.f(), r.f(), r.f()]).collect();
        bones.push((ob, cls, v));
    }
    let (dead_mod, dead_rem) = (r.u(), r.u());
    Scene {
        frame,
        logic_seed,
        client_seed,
        unit_damaged,
        unit_really_damaged,
        rubble_default,
        health_bonus,
        repulsors,
        friends_radius,
        max_retaliate,
        ps,
        neutral_team,
        armors,
        fxs,
        tmpls,
        players,
        objs,
        rel,
        infos,
        able,
        dist,
        bones,
        dead_mod,
        dead_rem,
    }
}

// ---- the generators ------------------------------------------------------------------------------------------------------------------------------------------------

fn health_str(g: &mut Gen, clean: bool) -> String {
    match g.below(14) {
        0..=7 => format!("{}", *pick(g, &[&100u32, &200, &300, &500, &1000, &50, &120, &1]))
            + if g.below(5) == 0 { ".5" } else { "" },
        8 => "0".to_string(),
        9 => "-10".to_string(),
        _ if clean => "400".to_string(),
        _ => gen_real(g, clean),
    }
}

/// The value of a `DamageTypeFlags` field: `NONE` or `ALL`, then `+TYPE` / `-TYPE`s (an unknown type, and the empty value, sometimes).
fn gen_flags(g: &mut Gen, clean: bool) -> String {
    let mut t = vec![if g.below(3) == 0 { "ALL".to_string() } else { "NONE".to_string() }];
    for _ in 0..g.below(6) {
        let name = DAMAGE_TYPE_NAMES[g.below(DAMAGE_NUM_TYPES as u32) as usize];
        t.push(format!("{}{name}", if g.below(4) == 0 { '-' } else { '+' }));
    }
    if !clean && g.below(10) == 0 {
        t.push("+NOT_A_TYPE".to_string());
    }
    t.join(" ")
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    if g.below(100) < 92 {
        lines.push(format!("MaxHealth{}{}", gen_sep(g), health_str(g, clean)));
    }
    if g.below(100) < 85 {
        lines.push(format!("InitialHealth{}{}", gen_sep(g), health_str(g, clean)));
    }
    if g.below(100) < 50 {
        let v = match g.below(8) {
            0 => "0".to_string(),
            _ => health_str(g, clean),
        };
        lines.push(format!("SubdualDamageCap{}{v}", gen_sep(g)));
    }
    if g.below(100) < 40 {
        lines.push(format!("SubdualDamageHealRate{}{}", gen_sep(g), gen_int(g, clean)));
    }
    if g.below(100) < 40 {
        lines.push(format!("SubdualDamageHealAmount{}{}", gen_sep(g), gen_real(g, clean)));
    }
    if !clean && g.below(12) == 0 {
        lines.push(format!("Bogus{}1", gen_sep(g)));
    }
    // the class of the data: the block's name (D `ActiveBody`, U `UndeadBody`, H `HiveStructureBody`)
    let kind: &str = ["D", "D", "D", "U", "U", "H", "H"][g.below(7) as usize];
    if kind == "U" && g.below(100) < 80 {
        lines.push(format!("SecondLifeMaxHealth{}{}", gen_sep(g), health_str(g, clean)));
    }
    if kind == "H" {
        for key in ["PropagateDamageTypesToSlavesWhenExisting", "SwallowDamageTypesIfSlavesNotExisting"] {
            if g.below(100) < 85 {
                lines.push(format!("{key}{}{}", gen_sep(g), gen_flags(g, clean)));
            }
        }
    }
    let mut s = format!("{kind}\n");
    for l in &lines {
        s.push_str(gen_indent(g));
        s.push_str(l);
        s.push('\n');
    }
    if clean || g.below(12) != 0 {
        s.push_str("End\n");
    }
    s.into_bytes()
}

struct W(Vec<u32>);
impl W {
    fn u(&mut self, x: u32) {
        self.0.push(x);
    }
    fn s(&mut self, x: i32) {
        self.0.push(x as u32);
    }
    fn f(&mut self, x: f32) {
        self.0.push(x.to_bits());
    }
}

fn gen_amount(g: &mut Gen) -> f32 {
    match g.below(20) {
        0..=11 => *pick(g, &[&0.0f32, &1.0, &5.0, &10.0, &25.0, &50.0, &100.0, &150.0, &400.0, &33.3, &0.5, &75.0]),
        12 => -5.0,
        13 => 999_999.0,
        14 => 1e9,
        15 => 2.0,
        _ => g.uniform(0.0, 300.0),
    }
}

fn gen_damage_type(g: &mut Gen) -> i32 {
    match g.below(100) {
        0..=33 => g.below(DAMAGE_NUM_TYPES as u32) as i32,
        34..=44 => DAMAGE_HEALING,
        45..=52 => DAMAGE_UNRESISTABLE,
        53..=60 => DAMAGE_KILLPILOT,
        61..=68 => DAMAGE_KILL_GARRISONED,
        69..=76 => DAMAGE_STATUS,
        77..=82 => DAMAGE_SNIPER,
        83..=89 => *pick(g, &[&DAMAGE_SUBDUAL_MISSILE, &DAMAGE_SUBDUAL_VEHICLE, &DAMAGE_SUBDUAL_BUILDING, &DAMAGE_SUBDUAL_UNRESISTABLE]),
        90..=94 => DAMAGE_EXPLOSION,
        _ => *pick(g, &[&DAMAGE_FLAME, &DAMAGE_POISON, &DAMAGE_RADIATION, &DAMAGE_SMALL_ARMS]),
    }
}

/// A set of damage types, as a bit set: a few named ones, or most of them.
fn gen_type_mask(g: &mut Gen) -> u64 {
    let all = (1u64 << DAMAGE_NUM_TYPES) - 1;
    match g.below(6) {
        0 => 0,
        1 => all,
        2 => all & !(1 << DAMAGE_UNRESISTABLE),
        _ => (g.u32() as u64 | (g.u32() as u64) << 32) & (g.u32() as u64 | (g.u32() as u64) << 32) & all,
    }
}

/// The scene of a case, as words (the one writer of the layout).
fn gen_scene(g: &mut Gen, data: &Option<ModuleDataKind>, focus: u32) -> Vec<u32> {
    let mut w = W(Vec::new());
    let n = 2 + g.below(3) as usize;
    w.u(g.below(300));
    w.u(g.u32());
    w.u(g.u32());
    // the globals
    let (a, b) = match g.below(10) {
        0 => (0.9, 0.3),
        1 => (0.5, 0.5),
        _ => (0.5, 0.1),
    };
    w.f(a);
    w.f(b);
    w.f(*pick(g, &[&1.0f32, &10.0, &0.0, &-1.0]));
    w.f(1.0);
    for _ in 0..3 {
        w.f(*pick(g, &[&1.0f32, &1.0, &1.1, &1.25, &2.0, &0.5]));
    }
    w.u(u32::from(g.below(4) != 0));
    w.f(*pick(g, &[&120.0f32, &120.0, &300.0, &10.0]));
    w.f(*pick(g, &[&210.0f32, &210.0, &500.0, &5.0]));
    for _ in 0..PS_CLASSES {
        w.s(match g.below(6) {
            0 => 0,
            1 => -1,
            2 => 1,
            3 => 2,
            4 => 5,
            _ => 16,
        });
        w.u(if g.below(6) == 0 { 0 } else { 1 + g.below(7) });
    }
    w.s(NEUTRAL_TEAM as i32);
    // the armors
    let n_armors = 1 + g.below(ARMORS as u32);
    w.u(n_armors);
    for _ in 0..n_armors {
        let base = *pick(g, &[&1.0f32, &1.0, &0.5, &0.0, &2.0, &-1.0]);
        for t in 0..DAMAGE_NUM_TYPES {
            w.f(if g.below(5) == 0 { *pick(g, &[&0.0f32, &0.25, &0.5, &1.5, &3.0, &-2.0, &1.0]) } else { base + (t as f32) * 0.0 });
        }
    }
    let n_fx = g.below(FXS as u32 + 1);
    w.u(n_fx);
    for _ in 0..n_fx {
        w.f(*pick(g, &[&0.0f32, &10.0, &50.0, &100.0, &1.0]));
        w.u(g.below(8));
        w.u(g.below(8));
        w.u(*pick(g, &[&0u32, &0, &5, &30, &1, &100]));
        let n_over = g.below(5);
        w.u(n_over);
        for _ in 0..n_over {
            w.s(gen_damage_type(g).clamp(0, DAMAGE_NUM_TYPES as i32 - 1));
            w.s(g.below(4) as i32);
            w.f(*pick(g, &[&0.0f32, &10.0, &50.0, &100.0, &1.0]));
            w.u(g.below(8));
            w.u(g.below(8));
            w.u(*pick(g, &[&0u32, &0, &5, &30, &1, &100]));
        }
    }
    let n_tmpl = 1 + g.below(TEMPLATES as u32);
    w.u(n_tmpl);
    for _ in 0..n_tmpl {
        w.s(*pick(g, &[&0i32, &0, &5, &-3, &20]));
        for _ in 0..6 {
            w.u(if g.below(4) == 0 { 0 } else { 1 + g.below(6) });
        }
        let n_sets = g.below(5);
        w.u(n_sets);
        for s in 0..n_sets {
            // the first set is mostly the default one; the others have a veteran, elite or hero flag or two
            let mask = if s == 0 && g.below(4) != 0 { 0 } else { g.below(256) & if g.below(2) == 0 { 0x07 } else { 0xff } };
            w.u(mask);
            w.u(if g.below(5) == 0 { 0 } else { 1 + g.below(n_armors) });
            w.u(if n_fx == 0 || g.below(3) == 0 { 0 } else { 1 + g.below(n_fx) });
        }
    }
    for p in 0..4u32 {
        w.u(u32::from(p != 0 && g.below(if focus == 3 { 20 } else { 5 }) == 0)); // computer?
        w.u(u32::from(g.below(if focus == 3 { 12 } else { 3 }) != 0));
    }
    w.u(n as u32);
    // the kinds first: the towers of a bridge must not be bridges (a bridge that lists itself or another bridge as a tower recurses for ever upstream, in setIndestructible)
    let kinds_all: Vec<u32> = (0..n)
        .map(|_| {
            let base: u32 = match g.below(8) {
                0..=2 => 1 << 0,
                3..=4 => (1 << 1) | if g.below(4) == 0 { 1 << 9 } else { 0 },
                5 => (1 << 10) | if g.below(2) == 0 { 1 << 5 } else { 0 },
                6 => (1 << 3) | (1 << 1),
                _ => (1 << 4) | (1 << 1),
            };
            base | (g.below(1 << 13) & g.below(1 << 13) & g.below(1 << 13))
        })
        .collect();
    for k in 0..n {
        w.u(g.below(n_tmpl)); // the template
        let kinds = kinds_all[k];
        w.u(kinds);
        w.u(if focus == 3 { g.below(256) & g.below(256) & g.below(256) } else { g.below(256) & g.below(256) });
        w.s(if g.below(40) == 0 { -1 } else { g.below(4) as i32 });
        w.s(g.below(4) as i32);
        for _ in 0..3 {
            w.f(*pick(g, &[&0.0f32, &10.0, &50.5, &-20.0]));
        }
        w.u(if g.below(6) == 0 { 1 + g.below(n as u32) } else { 0 }); // contained by
        w.u(u32::from(g.below(4) != 0)); // drawable
        let has_ai = g.below(10) < 7;
        w.u(u32::from(has_ai));
        w.u(u32::from(g.below(3) == 0));
        w.u(u32::from(g.below(2) == 0));
        let has_contain = g.below(10) < 6;
        w.u(u32::from(has_contain));
        w.u(u32::from(g.below(2) == 0));
        w.u(u32::from(g.below(4) == 0));
        w.u(u32::from((has_ai && has_contain && g.below(3) == 0) || g.below(40) == 0));
        let n_probes = if has_ai || has_contain { 1 + g.below(2) } else { g.below(3) };
        w.u(n_probes);
        let n_items = g.below(4);
        w.u(n_items);
        for i in 0..3 {
            // the first contained object is mostly a real one (the rider of a bike, the first to be killed in a garrison)
            w.u(if g.below(if i == 0 { 20 } else { 5 }) == 0 { 0 } else { 1 + g.below(n as u32) });
        }
        for _ in 0..2 {
            w.u(u32::from(g.below(3) == 0));
            w.u(u32::from(g.below(4) == 0));
        }
        for _ in 0..4 {
            let t = 1 + g.below(n as u32);
            w.u(if g.below(4) == 0 || kinds_all[t as usize - 1] & (1 << 3) != 0 { 0 } else { t });
        }
        // the body of an object other than the first (the first's is the data read from the INI)
        let _ = k;
        let max = *pick(g, &[&100.0f32, &200.0, &500.0, &50.0, &1000.0]);
        w.f(max);
        w.f(if g.below(4) == 0 { max * 0.3 } else { max });
        w.f(if focus == 4 { *pick(g, &[&100.0f32, &50.0, &80.0, &300.0, &0.0]) } else { *pick(g, &[&0.0f32, &0.0, &100.0, &50.0]) });
        w.u(*pick(g, &[&0u32, &30, &90]));
        w.f(*pick(g, &[&0.0f32, &5.0, &20.0]));
        // the class of the body; the first object's follows the class of the data read from the INI
        w.u(match (k, data) {
            (0, Some(ModuleDataKind::UndeadBody(_))) => 5,
            (0, Some(ModuleDataKind::HiveStructureBody(_))) => 6,
            (0, _) => *pick(g, &[&0u32, &0, &1, &2, &3, &3, &4, &4]),
            _ => *pick(g, &[&0u32, &0, &1, &2, &3, &3, &4, &4, &5, &5, &5, &6, &6, &6]),
        });
        // the second life, the damage types a hive passes on, its spawn behaviour, its closest slave and rider (always a later object: a hive that passed damage on
        // to itself, or in a ring, would recurse for ever upstream), and the slow death behaviours among the probes
        w.f(*pick(g, &[&1.0f32, &50.0, &200.0, &0.0, &-5.0, &1000.0, &30.0]));
        for _ in 0..2 {
            let m = gen_type_mask(g);
            w.u(m as u32);
            w.u((m >> 32) as u32);
        }
        w.u(if g.below(3) == 0 { 0 } else { 1 + g.below(2) });
        for _ in 0..2 {
            w.u(if k + 1 < n && g.below(4) != 0 { (k as u32 + 2) + g.below((n - k - 1) as u32) } else { 0 });
        }
        for _ in 0..2 {
            w.u(u32::from(g.below(5) < 3));
            let m = gen_type_mask(g);
            w.u(m as u32);
            w.u((m >> 32) as u32);
            w.s(*pick(g, &[&1i32, &2, &3, &5, &10, &0, &-2, &100]));
        }
    }
    for i in 0..n {
        for j in 0..n {
            w.s(if i == j {
                2
            } else if focus == 3 && j == 0 && g.below(10) != 0 {
                0
            } else {
                *pick(g, &[&0i32, &0, &1, &2])
            });
        }
    }
    // the damage infos: one of each kind that has a branch of its own, and some at random
    w.u(INFOS as u32);
    for i in 0..INFOS {
        w.u(match g.below(10) {
            0 => 0,
            1 => 9, // an object that does not exist
            _ => 1 + g.below(n as u32),
        });
        let t = match i {
            1 => DAMAGE_HEALING,
            2 => DAMAGE_UNRESISTABLE,
            3 => DAMAGE_KILLPILOT,
            4 => DAMAGE_KILL_GARRISONED,
            5 => DAMAGE_STATUS,
            6 => DAMAGE_SNIPER,
            7 => *pick(g, &[&DAMAGE_SUBDUAL_MISSILE, &DAMAGE_SUBDUAL_VEHICLE, &DAMAGE_SUBDUAL_BUILDING]),
            8 | 9 => DAMAGE_SUBDUAL_UNRESISTABLE,
            _ => g.below(DAMAGE_NUM_TYPES as u32) as i32,
        };
        w.s(t);
        w.s(*pick(g, &[&0i32, &0, &1, &2, &3, &4, &11, &7]));
        w.s(if g.below(4) == 0 { gen_damage_type(g).clamp(0, DAMAGE_NUM_TYPES as i32 - 1) } else { DAMAGE_UNRESISTABLE });
        w.f(match (i, g.below(3)) {
            (8, 0) => -30.0,
            (8, 1) => -5.0,
            (9, _) => *pick(g, &[&50.0f32, &120.0, &1000.0]),
            (4, _) => *pick(g, &[&1.0f32, &2.0, &3.0, &0.0, &5.0]),
            _ => gen_amount(g),
        });
        w.u(u32::from(i == 10 || g.below(12) == 0));
        w.s(g.below(OBJECT_STATUS_COUNT as u32) as i32);
        // the outputs the info holds before the body sees it
        w.f(*pick(g, &[&0.0f32, &0.0, &7.5, &-1.0]));
        w.f(*pick(g, &[&0.0f32, &0.0, &3.25, &-2.0]));
    }
    for _ in 0..n * n {
        w.s(g.below(4) as i32);
    }
    for _ in 0..n * n {
        w.f(if focus == 3 { *pick(g, &[&0.0f32, &100.0, &44_100.0, &1000.0, &90_000.0]) } else { *pick(g, &[&0.0f32, &100.0, &90_000.0, &1.0e6, &44_100.0]) });
    }
    // the bones: for some objects and classes
    let mut bones = Vec::new();
    for ob in 1..=n as i32 {
        for cls in 0..PS_CLASSES as i32 {
            if g.below(2) == 0 {
                bones.push((ob, cls));
            }
        }
    }
    w.u(bones.len() as u32);
    for (ob, cls) in bones {
        w.s(ob);
        w.s(cls);
        let cnt = g.below(20); // more than the sixteen the body asks for, sometimes
        w.u(cnt);
        for _ in 0..cnt {
            for _ in 0..3 {
                w.f(g.uniform(-30.0, 30.0));
            }
        }
    }
    // some of the particle systems are gone when the body looks for them
    if g.below(3) == 0 {
        w.u(2 + g.below(3));
        w.u(g.below(2));
    } else {
        w.u(0);
        w.u(0);
    }
    w.0
}

/// The index of a damage info whose type is one of `types` (any at random if the scene has none).
fn info_of(g: &mut Gen, sc: &Scene, types: &[i32]) -> u32 {
    let ok: Vec<u32> = (0..sc.infos.len() as u32).filter(|&i| types.contains(&sc.infos[i as usize].1)).collect();
    if ok.is_empty() {
        g.below(sc.infos.len() as u32)
    } else {
        ok[g.below(ok.len() as u32) as usize]
    }
}

fn gen_case(g: &mut Gen, data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    // the case's focus: 0 to 2 general; 3 retaliation; 4 subdual; 5 the pilot and garrison kills; 6 states and particle systems; 7 health, veterancy and armor
    let focus = g.below(8);
    let cfg = gen_scene(g, data, focus);
    let s = parse_scene(&cfg);
    let n = s.objs.len() as u32;
    let n_infos = INFOS as u32;
    let mut ops: Vec<[u32; 5]> = Vec::new();
    // most cases begin with a look at the visual condition (the particle systems)
    if g.below(100) < 40 || focus == 6 {
        ops.push([13, 0, 0, 0, 0]);
    }
    if focus == 3 {
        // the victim's player retaliates; its friends are the other objects; they are idle and can attack the aggressor
        let p = s.objs[0].player.max(0) as u32;
        ops.push([21, p, 1, 0, 0]);
        ops.push([24, n.min(3) - 1, 2, if n > 2 { 3 } else { 2 }, 0]);
        for them in 0..n {
            for victim in 0..n {
                ops.push([22, them, victim, 2 + g.below(2), 0]);
            }
            ops.push([26, them, 1, 1, 0]);
        }
    }
    let n_ops = 1 + g.below(60) as usize;
    let mut saved_cls: Option<u32> = None;
    for _ in 0..n_ops {
        let obj = if g.below(100) < 60 { 0 } else { g.below(n) };
        let mut r = g.below(1000);
        // the focused ops replace half of the rolls
        let focused = focus >= 3 && g.below(100) < 55;
        let op: [u32; 5] = if focused {
            match focus {
                3 => match g.below(10) {
                    0..=5 => [0, 0, info_of(g, &s, &[DAMAGE_EXPLOSION, DAMAGE_SMALL_ARMS, DAMAGE_ARMOR_PIERCING, DAMAGE_FLAME, DAMAGE_LASER]), 0, 0],
                    6 => [24, 1 + g.below(3), 1 + g.below(n), 1 + g.below(n), 1 + g.below(n)],
                    7 => [26, g.below(n), 1, g.below(2), 0],
                    8 => [18, g.below(n), 0b0000_1110, g.below(2), 0],
                    _ => [22, g.below(n), 0, 2 + g.below(2), 0],
                },
                4 => match g.below(10) {
                    0..=6 => [0, obj, info_of(g, &s, &[DAMAGE_SUBDUAL_MISSILE, DAMAGE_SUBDUAL_VEHICLE, DAMAGE_SUBDUAL_BUILDING, DAMAGE_SUBDUAL_UNRESISTABLE]), 0, 0],
                    7 => [2, obj, info_of(g, &s, &[DAMAGE_SUBDUAL_VEHICLE, DAMAGE_SUBDUAL_UNRESISTABLE]), 0, 0],
                    8 => [5, obj, bits(*pick(g, &[&10.0f32, &500.0, &50.0])), g.below(5), 0],
                    _ => [28, obj, 0, 0, 0],
                },
                5 => match g.below(10) {
                    0..=4 => [0, obj, info_of(g, &s, &[DAMAGE_KILLPILOT]), 0, 0],
                    5..=7 => [0, obj, info_of(g, &s, &[DAMAGE_KILL_GARRISONED]), 0, 0],
                    8 => [26, obj, 3 + g.below(2) * 2, g.below(2), 0],
                    _ => [26, obj, 6, 1 + g.below(3), g.u32()],
                },
                6 => match g.below(10) {
                    0..=2 => [3, obj, g.below(4), 0, 0],
                    3..=4 => [6, obj, bits(*pick(g, &[&-10.0f32, &-100.0, &20.0, &500.0, &-1000.0])), 0, 0],
                    5..=6 => [8, obj, g.below(2), 0, 0],
                    7 => [14, obj, 0, 0, 0],
                    8 => [13, obj, 0, 0, 0],
                    _ => [18, obj, 0b0001_0000, g.below(2), 0],
                },
                _ => match g.below(10) {
                    0..=2 => [9, obj, g.below(4), g.below(4), g.below(2)],
                    3..=4 => [5, obj, bits(*pick(g, &[&100.0f32, &200.0, &50.0, &1000.0, &0.0, &10.0])), g.below(5), 0],
                    5 => [4, obj, *pick(g, &[&0u32, &50, &100, &25, &150]), 0, 0],
                    6..=7 => [10, obj, g.below(3), g.below(8), 0],
                    8 => [1, obj, info_of(g, &s, &[DAMAGE_HEALING]), 0, 0],
                    _ => [0, obj, g.below(n_infos), 0, 0],
                },
            }
        } else {
            [0; 5]
        };
        if focused {
            ops.push(op);
            continue;
        }
        r = r.min(999);
        let op: [u32; 5] = match r {
            0..=299 => [0, obj, g.below(n_infos), 0, 0],
            300..=369 => [1, obj, g.below(n_infos), 0, 0],
            370..=409 => [2, obj, g.below(n_infos), 0, 0],
            410..=429 => [3, obj, g.below(4), 0, 0],
            430..=449 => [4, obj, *pick(g, &[&0u32, &50, &100, &25, &150, &u32::MAX]), 0, 0],
            450..=479 => [5, obj, bits(*pick(g, &[&100.0f32, &200.0, &50.0, &1000.0, &0.0, &10.0])), g.below(5), 0],
            480..=509 => [6, obj, bits(*pick(g, &[&-10.0f32, &-100.0, &20.0, &500.0, &-1000.0, &0.0, &-0.5])), 0, 0],
            510..=524 => [7, obj, g.below(2), 0, 0],
            525..=544 => [8, obj, g.below(2), 0, 0],
            545..=574 => [9, obj, g.below(4), g.below(4), g.below(2)],
            575..=599 => [10, obj, g.below(3), g.below(8), 0],
            600..=609 => [11, obj, g.below(2), g.below(2), 0],
            610..=619 => [12, obj, 0, 0, 0],
            620..=649 => [13, obj, 0, 0, 0],
            650..=669 => [14, obj, 0, 0, 0],
            670..=679 => [15, obj, 0, 0, 0],
            680..=689 => [16, obj, bits(*pick(g, &[&0.5f32, &2.0, &0.0, &1.0])), 0, 0],
            690..=719 => [17, g.below(400), 0, 0, 0],
            720..=769 => [18, obj, g.below(256), g.below(2), 0],
            770..=779 => [21, g.below(4), g.below(2), g.below(2), 0],
            780..=799 => [22, g.below(n), g.below(n), g.below(4), 0],
            800..=819 => [23, g.below(n), g.below(n), bits(*pick(g, &[&0.0f32, &100.0, &90_000.0, &1.0e6])), 0],
            820..=849 => [24, g.below(4), 1 + g.below(n), 1 + g.below(n), 1 + g.below(n)],
            850..=889 => [26, obj, g.below(6), g.below(2), g.u32()],
            890..=899 => [26, obj, 5, g.below(4), g.u32()],
            900..=909 => [27, g.below(4), g.u32(), 0, 0],
            910..=919 => [28, obj, if g.below(2) == 0 { 0 } else { 1 + g.below(n) }, 0, 0],
            920..=939 => {
                saved_cls = Some(s.objs[obj as usize].cls);
                [29, obj, 0, 0, 0]
            }
            // a load of the bytes another class saved is a load of shifted fields (a flag read from a byte that is not 0 or 1 is not a `bool` in C++, and is `true` here):
            // upstream-undefined, so only a body of the class that saved loads
            940..=959 => match (0..n).filter(|&k| Some(s.objs[k as usize].cls) == saved_cls).nth(g.below(2) as usize) {
                Some(k) => [30, k, 0, 0, 0],
                None => [31, obj, 0, 0, 0],
            },
            960..=969 => [31, obj, 0, 0, 0],
            970..=979 => [32, obj, g.below(2), 0, 0],
            980..=989 => [33, obj, g.below(2), 0, 0],
            990..=994 => [34, obj, g.below(4), 0, 0],
            _ => [36, obj, if g.below(3) == 0 { 0 } else { 1 + g.below(n) }, 0, 0],
        };
        ops.push(op);
    }
    (cfg, ops)
}

// ---- the dump ------------------------------------------------------------------------------------------------------------------------------------------------------

#[derive(Default)]
struct Tally {
    effects: u64,
    xfers: u64,
    damages: u64,
    deaths: u64,
    kills: u64,
    fxs: u64,
    particle_systems: u64,
    retaliations: u64,
    subdual: u64,
    /// how many log lines of each first word
    words: BTreeMap<String, u64>,
}

struct FxIds(BTreeMap<Vec<u8>, i32>);
impl FxIds {
    fn id(&mut self, name: &[u8]) -> i32 {
        let next = self.0.len() as i32 + 1;
        *self.0.entry(name.to_vec()).or_insert(next)
    }
}

fn put_info(v: &mut Vec<u8>, i: &DamageInfo) {
    put_u32(v, i.input.source_id as u32);
    put_u32(v, i.input.source_player_mask as u32);
    put_i32(v, i.input.damage_type);
    put_i32(v, i.input.damage_status_type);
    put_i32(v, i.input.damage_fx_override);
    put_i32(v, i.input.death_type);
    put_u32(v, i.input.amount.to_bits());
    put_u32(v, u32::from(i.input.kill));
    put_u32(v, i.input.shock_wave_vector.x.to_bits());
    put_u32(v, i.input.shock_wave_vector.y.to_bits());
    put_u32(v, i.input.shock_wave_vector.z.to_bits());
    put_u32(v, i.input.shock_wave_amount.to_bits());
    put_u32(v, i.input.shock_wave_radius.to_bits());
    put_u32(v, i.input.shock_wave_taper_off.to_bits());
    put_str(v, &i.input.source_template);
    put_u32(v, i.out.actual_damage_dealt.to_bits());
    put_u32(v, i.out.actual_damage_clipped.to_bits());
    put_u32(v, u32::from(i.out.no_effect));
}

fn get_info(r: &mut Reader) -> String {
    let src = r.u32();
    let mask = r.u32();
    let (dt, st, fxo, death) = (r.u32() as i32, r.u32() as i32, r.u32() as i32, r.u32() as i32);
    let amount = r.u32();
    let kill = r.u32();
    let shock: Vec<u32> = (0..6).map(|_| r.u32()).collect();
    let tn = r.str();
    let (dealt, clipped, noeff) = (r.u32(), r.u32(), r.u32());
    format!("info src {src} mask {mask:x} type {dt} status {st} fxov {fxo} death {death} amount {amount:08x} kill {kill} shock {shock:x?} tmpl {tn:?} dealt {dealt:08x} clipped {clipped:08x} noeffect {noeff}")
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    // the class of the data (0 `ActiveBody`, 1 `UndeadBody`: the second life, 2 `HiveStructureBody`: the two sets as four words), then the five fields
    let kind = r.u32();
    let n_extra = [0, 1, 4][kind as usize];
    let data: Vec<u32> = (0..5 + n_extra).map(|_| r.u32()).collect();
    out.push(format!("data {kind} {data:08x?}"));
    let n_obj = r.u32();
    for i in 0..=n_ops {
        if r.at_end() {
            break;
        }
        let st = r.u32() as i32;
        let result = r.u32();
        let nlog = r.u32();
        let log: Vec<String> = (0..nlog).map(|_| r.str()).collect();
        let xst = r.u32() as i32;
        let blen = r.u32();
        let bytes: Vec<u32> = (0..blen).map(|_| r.u32()).collect();
        let crc = r.u32();
        out.push(format!("op {i}: status {st} result {result:08x} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x}"));
        for k in 0..n_obj {
            let state: Vec<u32> = (0..18).map(|_| r.u32()).collect();
            let n_ps = state[17];
            let ids: Vec<u32> = (0..n_ps).map(|_| r.u32()).collect();
            let info = get_info(&mut r);
            let rest: Vec<u32> = (0..5).map(|_| r.u32()).collect();
            out.push(format!("op {i} object {k}: body {state:08x?} ps {ids:?} {info} status {:08x} {:08x} dead {} geometry {:08x} constructor {}", rest[0], rest[1], rest[2], rest[3], rest[4]));
        }
        let players: Vec<u32> = (0..8).map(|_| r.u32()).collect();
        out.push(format!("op {i} players {players:x?}"));
    }
    out
}

fn put_object(v: &mut Vec<u8>, w: &mut World, id: ObjectId) {
    let body_idx = w.objects.find(id).expect("the object").body.expect("the body");
    let inactive = matches!(w.objects.find(id).unwrap().modules[body_idx as usize].body.as_ref(), Some(UpdateBody::InactiveBody(_)));
    let (words, info, constructor) = if inactive {
        // an InactiveBody has no fields of the ActiveBody layout: what Object asks of it, in the same 18 words as the driver's (the module's answers to the calls)
        let mut ask = |c: ModuleCall| w.module_call(id, body_idx, c);
        let real = |r: ModuleReply| r.as_real(f32::NAN).to_bits();
        let int = |r: ModuleReply| if let ModuleReply::Int(i) = r { i as u32 } else { 0xdead_0001 };
        let flag = |r: ModuleReply| u32::from(r.as_bool());
        let mut words = vec![
            real(ask(ModuleCall::GetHealth)),
            real(ask(ModuleCall::GetPreviousHealth)),
            real(ask(ModuleCall::GetMaxHealth)),
            real(ask(ModuleCall::GetInitialHealth)),
            real(ask(ModuleCall::GetCurrentSubdualDamageAmount)),
            int(ask(ModuleCall::GetDamageState)),
            int(ask(ModuleCall::GetSubdualDamageHealRate)),
            flag(ask(ModuleCall::HasAnySubdualDamage)) ^ real(ask(ModuleCall::GetSubdualDamageHealAmount)),
            int(ask(ModuleCall::GetLastDamageTimestamp)),
            int(ask(ModuleCall::GetLastHealingTimestamp)),
            flag(ask(ModuleCall::GetFrontCrushed)),
            flag(ask(ModuleCall::GetBackCrushed)),
            ask(ModuleCall::GetClearableLastAttacker).as_id() as u32,
            flag(ask(ModuleCall::IsIndestructible)),
        ];
        let mut flags = 0u32;
        for i in 0..rts_lockstep_engine::logic::armor_set::ARMORSET_COUNT {
            if ask(ModuleCall::TestArmorSetFlag(i)).as_bool() {
                flags |= 1 << i;
            }
        }
        words.push(flags);
        words.push(real(ask(ModuleCall::GetDamageScalar)));
        words.push(u32::from(!matches!(ask(ModuleCall::GetLastDamageInfo), ModuleReply::Unhandled)));
        words.push(0);
        (words, DamageInfo::default(), None)
    } else {
        let o = w.objects.find(id).unwrap();
        let (m, constructor) = match o.modules[body_idx as usize].body.as_ref() {
            Some(UpdateBody::ActiveBody(m)) => (m, None),
            Some(UpdateBody::HighlanderBody(m)) => (m.active_body(), None),
            Some(UpdateBody::ImmortalBody(m)) => (m.active_body(), None),
            Some(UpdateBody::StructureBody(m)) => (m.active_body(), Some(m.get_constructor_object_id())),
            Some(UpdateBody::UndeadBody(m)) => (m.active_body(), Some(i32::from(m.is_second_life()))),
            Some(UpdateBody::HiveStructureBody(m)) => (m.active_body(), Some(m.structure_body().get_constructor_object_id())),
            _ => panic!("the body is in its slot"),
        };
        (m.state_words(), m.get_last_damage_info().clone(), constructor)
    };
    let o = w.objects.find(id).unwrap();
    for x in words {
        put_u32(v, x);
    }
    put_info(v, &info);
    let (mut w0, mut w1) = (0u32, 0u32);
    for i in 0..OBJECT_STATUS_COUNT.min(64) {
        if o.status.test(i) {
            if i < 32 {
                w0 |= 1 << i;
            } else {
                w1 |= 1 << (i - 32);
            }
        }
    }
    put_u32(v, w0);
    put_u32(v, w1);
    put_u32(v, u32::from(o.is_effectively_dead()));
    put_u32(v, o.geometry_info.get_max_height_above_position().to_bits());
    // the constructor object id of a StructureBody, else 0
    put_u32(v, constructor.unwrap_or(0) as u32);
}

/// A `DamageTypeFlags` as a bit set (bit `t` for damage type `t`).
fn flags_bits(f: &DamageTypeFlags) -> u64 {
    (0..DAMAGE_NUM_TYPES as i32).filter(|&t| f.test(t)).fold(0, |m, t| m | 1 << t)
}

/// The inverse of [`flags_bits`].
fn bits_flags(m: u64) -> DamageTypeFlags {
    let mut f = DamageTypeFlags::NONE;
    for t in 0..DAMAGE_NUM_TYPES as i32 {
        if m >> t & 1 != 0 {
            f.set(t, 1);
        }
    }
    f
}

fn veterancy(i: i32) -> VeterancyLevel {
    match i {
        1 => VeterancyLevel::Veteran,
        2 => VeterancyLevel::Elite,
        3 => VeterancyLevel::Heroic,
        _ => VeterancyLevel::Regular,
    }
}

fn fx_name(idx: u32) -> Option<Vec<u8>> {
    (idx > 0).then(|| format!("FX{idx}").into_bytes())
}

/// An xfer of the body in module slot `idx`: `how` 0 saves, 1 loads `saved` (cut to 2048 bytes), 2 computes the CRC.
fn run_xfer_body(w: &mut World, me: ObjectId, idx: usize, how: u32, saved: &[u8]) -> (i32, Vec<u8>, u32) {
    fn status<T>(r: Result<T, rts_lockstep_engine::common::xfer::XferStatus>) -> i32 {
        match r {
            Ok(_) => 0,
            Err(e) => e as i32,
        }
    }
    match how {
        0 => {
            let mut x = XferSave::new();
            let st = match x.open(XFER_FILE.as_bytes()) {
                Ok(()) => status(w.xfer_module_slot(me, idx, &mut x)),
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, x.bytes().to_vec(), 0)
        }
        1 => {
            let mut b = saved.to_vec();
            b.truncate(2048);
            let mut x = XferLoad::new(b);
            let st = match x.open(XFER_FILE.as_bytes()) {
                Ok(()) => status(w.xfer_module_slot(me, idx, &mut x)),
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, Vec::new(), 0)
        }
        _ => {
            let mut x = XferCrc::new_without_nan_guard();
            let st = match x.open(b"crc") {
                Ok(()) => status(w.xfer_module_slot(me, idx, &mut x)),
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, Vec::new(), x.get_crc())
        }
    }
}

/// The lines of the effect log the trace and the records of one op make, in order.
fn effect_lines(trace: Vec<String>, cuts: Vec<CutCall>, events: Vec<ObjectClientEvent>, fx: &mut FxIds, body_idx: &[u16], tally: &RefCell<Tally>) -> Vec<String> {
    let mut log = Vec::new();
    let (mut cut_iter, mut ev_iter) = (cuts.into_iter(), events.into_iter());
    for t in trace {
        if t.starts_with("cut ") {
            match cut_iter.next() {
                Some(CutCall::Body(b)) => match b {
                    BodyCall::SetDisabled { object, disabled_type } => log.push(format!("setdisabled {object} {disabled_type}")),
                    BodyCall::ClearDisabled { object, disabled_type } => log.push(format!("cleardisabled {object} {disabled_type}")),
                    // batch R33: ActiveBody never disables until a frame
                    BodyCall::SetDisabledUntil { object, disabled_type, frame } => log.push(format!("disableduntil {object} {disabled_type} {frame}")),
                    BodyCall::DoStatusDamage { object, status, frames } => log.push(format!("statusdamage {object} {status} {}", frames as f32 as i32)),
                    BodyCall::NotifySubdualDamage { object, amount } => {
                        tally.borrow_mut().subdual += 1;
                        log.push(format!("subdual {object} {:08x}", amount.to_bits()))
                    }
                    BodyCall::ScoreTheKill { killer, victim } => {
                        tally.borrow_mut().kills += 1;
                        log.push(format!("score {killer} {victim}"))
                    }
                    // (batch R73's: ActiveBody never makes it)
                    BodyCall::ExperienceSetVeterancyLevel { object, level } => log.push(format!("xplevel {object} {level}")),
                    BodyCall::OnDie { object, info } => {
                        tally.borrow_mut().deaths += 1;
                        log.push(format!("ondie {object} {}", info_line(&info)))
                    }
                    BodyCall::SetTeam { team, .. } => log.push(format!("setteam {team}")),
                    BodyCall::SetStatus { object, mask, set } => {
                        let (mut w0, mut w1) = (0u32, 0u32);
                        for i in 0..OBJECT_STATUS_COUNT.min(64) {
                            if mask.test(i) {
                                if i < 32 {
                                    w0 |= 1 << i;
                                } else {
                                    w1 |= 1 << (i - 32);
                                }
                            }
                        }
                        log.push(format!("setstatus {object} {w1:08x}{w0:08x} {}", i32::from(set)))
                    }
                    BodyCall::DestroyObject { object } => log.push(format!("destroy {object}")),
                    BodyCall::AcademyVehicleSniped => log.push("sniped".to_string()),
                    BodyCall::AcademyClearedGarrisonedBuilding { player } => log.push(format!("garrisoncleared {player}")),
                    BodyCall::GetMultiLogicalBonePosition { object, prefix, max } => log.push(format!("bones {object} {} {max}", String::from_utf8_lossy(&prefix))),
                    BodyCall::ParticleFindTemplate { .. } => {}
                    BodyCall::ParticleAttachToObject { id, object } => log.push(format!("psattach {id} {object}")),
                    BodyCall::ParticleDestroy { id, found } => {
                        if found {
                            log.push(format!("psdestroy {id}"))
                        }
                    }
                    BodyCall::GetAbleToAttackSpecificObject { object, victim } => {
                        tally.borrow_mut().retaliations += 1;
                        log.push(format!("canattack {object} {victim}"))
                    }
                    BodyCall::GetDistanceSquared { object, other } => log.push(format!("distsq {object} {other}")),
                    BodyCall::IterateRetaliationFriends { position, range, player } => log.push(format!(
                        "friends {:08x} {:08x} {:08x} {:08x} {player}",
                        position.x.to_bits(),
                        position.y.to_bits(),
                        position.z.to_bits(),
                        range.to_bits()
                    )),
                },
                Some(CutCall::PathfinderRemoveObjectFromPathfindMap { object }) => log.push(format!("pathremove {object}")),
                Some(CutCall::PathfinderAddObjectToPathfindMap { object }) => log.push(format!("pathadd {object}")),
                Some(CutCall::GameLogicDeselectObject { object, player_mask, affect_client }) => log.push(format!("deselect {object} {player_mask} {}", i32::from(affect_client))),
                Some(CutCall::ParticleSystemCreate { template, position, id }) => {
                    tally.borrow_mut().particle_systems += 1;
                    log.push(format!(
                        "psys {} {id} {:08x},{:08x},{:08x}",
                        String::from_utf8_lossy(&template),
                        position.x.to_bits(),
                        position.y.to_bits(),
                        position.z.to_bits()
                    ))
                }
                _ => {}
            }
        } else if t.starts_with("evt ") {
            match ev_iter.next() {
                Some(ObjectClientEvent::AudioAddEvent { name, object_id, position, player_index }) => {
                    let pos = match position {
                        Some(p) => format!("{},{},{}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits()),
                        None => "-".to_string(),
                    };
                    log.push(format!("audio {} {pos} {} {object_id}", String::from_utf8_lossy(&name), player_index.unwrap_or(-1)));
                }
                Some(ObjectClientEvent::FxListDoFxObjSecondary { fx: name, object_id, secondary }) => {
                    tally.borrow_mut().fxs += 1;
                    log.push(format!("fx {}", fx.id(&name)));
                    log.push(format!("fxobj {object_id} {secondary}"));
                }
                Some(ObjectClientEvent::DrawableReactToBodyDamageStateChange { object_id, state }) => log.push(format!("react {object_id} {state}")),
                Some(ObjectClientEvent::DrawableReactToGeometryChange { .. }) => log.push("react".to_string()),
                Some(ObjectClientEvent::ControlBarMarkUiDirty) => log.push("uidirty".to_string()),
                _ => {}
            }
        } else if let Some(line) = t.strip_prefix("bt ") {
            // a probe's line: the module index is relative to the body's (the C++ side numbers the body 0)
            let mut tok: Vec<String> = line.split(' ').map(str::to_string).collect();
            if matches!(tok[0].as_str(), "ondamage" | "onhealing" | "bodystate") {
                let (object, idx): (usize, u16) = (tok[1].parse().unwrap(), tok[2].parse().unwrap());
                tok[2] = (idx - body_idx[object - 1]).to_string();
            }
            log.push(tok.join(" "));
        }
    }
    log
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let mut v = Vec::new();
    {
        // the class of the data (0 `ActiveBody`, 1 `UndeadBody`, 2 `HiveStructureBody`) and what it adds, then the five fields of `ActiveBodyModuleData`
        let (d, extra): (&ActiveBodyModuleData, Vec<u32>) = match &data {
            ModuleDataKind::ActiveBody(d) => (d, vec![0]),
            ModuleDataKind::UndeadBody(d) => (&d.active, vec![1, d.second_life_max_health.to_bits()]),
            ModuleDataKind::HiveStructureBody(d) => {
                let (p, sw) = (flags_bits(&d.damage_types_to_propagate_to_slaves), flags_bits(&d.damage_types_to_swallow));
                (&d.active, vec![2, p as u32, (p >> 32) as u32, sw as u32, (sw >> 32) as u32])
            }
            _ => unreachable!("not a body's data"),
        };
        for x in extra {
            put_u32(&mut v, x);
        }
        put_u32(&mut v, d.max_health.to_bits());
        put_u32(&mut v, d.initial_health.to_bits());
        put_u32(&mut v, d.subdual_damage_cap.to_bits());
        put_u32(&mut v, d.subdual_damage_heal_rate);
        put_u32(&mut v, d.subdual_damage_heal_amount.to_bits());
    }
    let sc = parse_scene(cfg);
    let n = sc.objs.len();
    put_u32(&mut v, n as u32);

    let mut w = World::new();
    w.names = ctx.names;
    // the stores
    let mut armor_store = ArmorStore::new();
    for (i, c) in sc.armors.iter().enumerate() {
        let key = w.names.name_to_key(format!("Armor{i}").as_bytes());
        armor_store.insert_template(key, ArmorTemplate::from_coefficients(*c));
    }
    let mut fx_store = DamageFxStore::new();
    for (i, c) in sc.fxs.iter().enumerate() {
        let key = w.names.name_to_key(format!("DamageFX{i}").as_bytes());
        let mut dfx = DamageFx::new();
        let d = Dfx { amount_for_major_fx: c.default.0, major_damage_fx_list: fx_name(c.default.1), minor_damage_fx_list: fx_name(c.default.2), damage_fx_throttle_time: c.default.3 };
        for t in 0..DAMAGE_NUM_TYPES as i32 {
            for l in 0..4 {
                dfx.set_dfx(t, veterancy(l), d.clone()).unwrap();
            }
        }
        for o in &c.overrides {
            let x = Dfx { amount_for_major_fx: o.2, major_damage_fx_list: fx_name(o.3), minor_damage_fx_list: fx_name(o.4), damage_fx_throttle_time: o.5 };
            dfx.set_dfx(o.0, veterancy(o.1), x).unwrap();
        }
        fx_store.insert(key, dfx);
    }
    let mut templates = Vec::new();
    for (k, o) in sc.objs.iter().enumerate() {
        let t = &sc.tmpls[o.tmpl];
        let mut flags_sets = Vec::new();
        for &(mask, armor, fx) in &t.sets {
            let mut flags = ArmorSetFlags::new();
            for b in 0..8 {
                if mask >> b & 1 != 0 {
                    flags.set(b, 1);
                }
            }
            let armor_key = (armor > 0).then(|| w.names.name_to_key(format!("Armor{}", armor - 1).as_bytes()));
            let fx_key = (fx > 0).then(|| w.names.name_to_key(format!("DamageFX{}", fx - 1).as_bytes()));
            flags_sets.push(rts_lockstep_engine::common::thing_template::ArmorTemplateSet::from_parts(flags, armor_key, fx_key));
        }
        let slots = [
            ThingTemplateAudioType::SoundOnDamaged,
            ThingTemplateAudioType::SoundOnReallyDamaged,
            ThingTemplateAudioType::VoiceFear,
            ThingTemplateAudioType::SoundPromotedVeteran,
            ThingTemplateAudioType::SoundPromotedElite,
            ThingTemplateAudioType::SoundPromotedHero,
        ];
        let audio = (0..6).filter(|&i| t.audio[i] > 0).map(|i| (slots[i], format!("snd{}", t.audio[i]).into_bytes())).collect();
        let kind_of: Vec<i32> = (0..13).filter(|&i| o.kinds >> i & 1 != 0).map(|i| KINDS[i]).collect();
        templates.push(Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues {
            name: format!("T{}", k + 1).into_bytes(),
            kind_of,
            armor_sets: flags_sets,
            audio,
            structure_rubble_height: t.rubble,
            ..Default::default()
        })));
    }
    w.statics = Arc::new(rts_lockstep_engine::logic::world::statics::GameStatics {
        upgrade_center: ctx.center,
        special_power_store: ctx.store,
        armor_store,
        damage_fx_store: fx_store,
        ..Default::default()
    });
    // the cut points
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.body.probe_trace = true;
    w.cuts.body.stub_object_calls = true;
    w.cuts.neutral_default_team = sc.neutral_team as u32;
    w.cuts.globals.enable_repulsors = sc.repulsors;
    {
        let mut classes: Vec<AutoParticle> = Vec::new();
        for k in 0..PS_CLASSES {
            let system = if sc.ps[k].1 == 0 { b"None".to_vec() } else { format!("PS{}", sc.ps[k].1).into_bytes() };
            if sc.ps[k].1 != 0 {
                w.cuts.body.particle_templates.insert(system.clone());
            }
            classes.push(AutoParticle { prefix: format!("BONE{k}").into_bytes(), system, max: sc.ps[k].0 });
        }
        let g = &mut w.cuts.body.globals;
        g.unit_damaged_thresh = sc.unit_damaged;
        g.unit_really_damaged_thresh = sc.unit_really_damaged;
        g.default_structure_rubble_height = sc.rubble_default;
        g.health_bonus = sc.health_bonus;
        g.retaliate_friends_radius = sc.friends_radius;
        g.max_retaliate_distance = sc.max_retaliate;
        g.auto_fire = [classes[0].clone(), classes[1].clone(), classes[2].clone()];
        g.auto_smoke = [classes[3].clone(), classes[4].clone(), classes[5].clone()];
        g.auto_aflame = classes[6].clone();
    }
    w.cuts.body.dead_particle_modulus = (sc.dead_mod, sc.dead_rem);
    for (ob, cls, list) in &sc.bones {
        w.cuts.body.bones.insert((*ob, format!("BONE{cls}").into_bytes()), list.iter().map(|p| Coord3D { x: p[0], y: p[1], z: p[2] }).collect());
    }
    for i in 0..n {
        for j in 0..n {
            w.cuts.body.able_to_attack.insert((i as i32 + 1, j as i32 + 1), sc.able[i][j]);
            w.cuts.body.distance_squared.insert((i as i32 + 1, j as i32 + 1), sc.dist[i][j]);
            let rel = match sc.rel[i][j] {
                0 => Relationship::Enemies,
                1 => Relationship::Neutral,
                _ => Relationship::Allies,
            };
            w.cuts.team_relationships.insert((i as u32 + 1, j as u32 + 1), rel);
        }
    }
    for (k, o) in sc.objs.iter().enumerate() {
        if o.player >= 0 {
            w.cuts.team_players.insert(k as u32 + 1, o.player);
        }
    }
    w.set_frame(sc.frame);
    w.random.init_random(sc.client_seed);
    w.random.init_game_logic_random(sc.logic_seed);
    for (p, &(ty, retaliation)) in sc.players.iter().enumerate() {
        let pl = w.players.get_nth_mut(p as i32).unwrap();
        pl.player_type = if ty != 0 { PlayerType::Computer } else { PlayerType::Human };
        pl.logical_retaliation_mode_enabled = retaliation;
    }
    // the objects
    for (k, o) in sc.objs.iter().enumerate() {
        let body_data = if k == 0 {
            data.clone()
        } else {
            let active = ActiveBodyModuleData {
                body: Default::default(),
                max_health: f(o.body[0]),
                initial_health: f(o.body[1]),
                subdual_damage_cap: f(o.body[2]),
                subdual_damage_heal_rate: o.body[3],
                subdual_damage_heal_amount: f(o.body[4]),
            };
            match o.cls {
                5 => ModuleDataKind::UndeadBody(Arc::new(UndeadBodyModuleData { active, second_life_max_health: f(o.second_life) })),
                6 => ModuleDataKind::HiveStructureBody(Arc::new(HiveStructureBodyModuleData {
                    active,
                    damage_types_to_propagate_to_slaves: bits_flags(o.propagate),
                    damage_types_to_swallow: bits_flags(o.swallow),
                })),
                _ => ModuleDataKind::ActiveBody(Arc::new(active)),
            }
        };
        let class_name: &[u8] =
            [b"ActiveBody".as_slice(), b"HighlanderBody", b"ImmortalBody", b"StructureBody", b"InactiveBody", b"UndeadBody", b"HiveStructureBody"][o.cls as usize];
        // an InactiveBody has no module data of its own: `Module::friend_newModuleData`'s plain `ModuleData`
        let body_data = if o.cls == 4 { ModuleDataKind::Module(Arc::new(rts_lockstep_engine::common::module::ModuleData::new())) } else { body_data };
        let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), class_name, b"ModuleTag_Body", &body_data).unwrap().unwrap();
        let mut modules = vec![m];
        for i in 0..o.n_probes {
            let mut a = ProbeAnswers::default();
            if i == 0 {
                a.moving = o.moving;
                a.idle = o.idle;
                a.immune = o.immune;
                a.rider_change = o.rider_change;
                a.garrisonable = o.garrisonable;
                a.contained = o.items.clone();
            }
            if o.probe_bridge[i] {
                a.has |= 1 << 10;
            }
            if o.probe_proj[i] {
                a.has |= 1 << 4;
            }
            a.bridge_towers = o.towers;
            if o.probe_slow[i] {
                a.slow_death = true;
                a.slow_applicable = o.slow_mask[i];
                a.slow_probability = o.slow_prob[i];
            }
            if o.spawn == i as u32 + 1 {
                a.has |= 1 << 3;
                a.closest_slave = o.closest_slave;
            }
            if i == 0 {
                a.closest_rider = o.closest_rider;
            }
            let mut pm = NewModule::plain(b"AbProbe", b"ModuleTag_Probe");
            pm.interfaces = MODULEINTERFACE_DAMAGE | MODULEINTERFACE_DIE | if i == 0 && o.has_contain { MODULEINTERFACE_CONTAIN } else { 0 };
            pm.is_ai = i == 0 && o.has_ai;
            pm.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(a, 0.0, Default::default())));
            modules.push(pm);
        }
        let status = {
            let mut s = Default::default();
            for (b, st) in STATUSES.iter().enumerate() {
                if o.status >> b & 1 != 0 {
                    rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType::set(&mut s, *st, 1);
                }
            }
            s
        };
        let id = w.new_object(NewObject { template: templates[k].clone(), status, team: Some(k as u32 + 1), modules }).unwrap();
        assert_eq!(id, k as i32 + 1);
        w.set_position(id, &Coord3D { x: o.pos[0], y: o.pos[1], z: o.pos[2] });
        // the world made the object a drawable; the scene says whether it has one
        w.friend_bind_to_drawable(id, if o.drawable { 7 + k as i32 } else { INVALID_DRAWABLE_ID });
        let ob = w.objects.find_mut(id).unwrap();
        ob.experience.level = veterancy(o.vet);
        ob.contained_by = o.contained_by;
    }
    let ids: Vec<ObjectId> = (1..=n as i32).collect();
    let mut saved: Vec<u8> = Vec::new();
    let mut excluded = false;
    let mut fx = FxIds(BTreeMap::new());
    for phase in 0..=ops.len() {
        let mut events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let errors_before = w.cuts.module_errors.len();
        let (mut xst, mut xbytes, mut crc, mut result) = (-2i32, Vec::new(), 0u32, 0u32);
        if phase > 0 {
            let o = &ops[phase - 1];
            let id = ids.get(o[1] as usize).copied().unwrap_or(1);
            let body = w.objects.find(id).and_then(|ob| ob.body);
            let m = body.unwrap_or(0);
            match o[0] {
                0 => {
                    let mut info = make_info(&sc, o[2] as usize);
                    w.attempt_damage(id, &mut info);
                    // what the caller sees afterwards: the amount the body limited, the damage dealt and clipped, and whether it had no effect
                    result = info
                        .out
                        .actual_damage_dealt
                        .to_bits()
                        .wrapping_add(info.out.actual_damage_clipped.to_bits().wrapping_mul(31))
                        .wrapping_add(info.input.amount.to_bits().wrapping_mul(1009))
                        .wrapping_add(if info.out.no_effect { 77777 } else { 0 });
                    tally.borrow_mut().damages += 1;
                }
                1 => {
                    w.module_call(id, m, ModuleCall::AttemptHealing(make_info(&sc, o[2] as usize)));
                }
                2 => result = w.module_call(id, m, ModuleCall::EstimateDamage(make_info(&sc, o[2] as usize).input)).as_real(0.0).to_bits(),
                3 => {
                    w.module_call(id, m, ModuleCall::SetDamageState(o[2] as i32));
                }
                4 => {
                    w.module_call(id, m, ModuleCall::SetInitialHealth(o[2] as i32));
                }
                5 => {
                    if o[3] == 4 {
                        w.module_call(id, m, ModuleCall::SetMaxHealth(f(o[2])));
                    } else {
                        w.module_call(id, m, ModuleCall::SetMaxHealthWithChange { max_health: f(o[2]), change_type: o[3] as i32 });
                    }
                }
                6 => {
                    w.module_call(id, m, ModuleCall::InternalChangeHealth(f(o[2])));
                }
                7 => {
                    w.module_call(id, m, ModuleCall::SetIndestructible(o[2] != 0));
                }
                8 => {
                    w.objects.find_mut(id).unwrap().status.set(OBJECT_STATUS_AFLAME, i32::from(o[2] != 0));
                    w.module_call(id, m, ModuleCall::SetAflame(o[2] != 0));
                }
                9 => {
                    w.module_call(id, m, ModuleCall::OnVeterancyLevelChanged { old_level: o[2] as i32, new_level: o[3] as i32, provide_feedback: o[4] != 0 });
                }
                10 => match o[2] {
                    0 => {
                        w.module_call(id, m, ModuleCall::SetArmorSetFlag(o[3] as i32));
                    }
                    1 => {
                        w.module_call(id, m, ModuleCall::ClearArmorSetFlag(o[3] as i32));
                    }
                    _ => result = u32::from(w.module_call(id, m, ModuleCall::TestArmorSetFlag(o[3] as i32)).as_bool()),
                },
                11 => {
                    w.module_call(id, m, if o[2] == 0 { ModuleCall::SetFrontCrushed(o[3] != 0) } else { ModuleCall::SetBackCrushed(o[3] != 0) });
                }
                12 => {
                    w.module_call(id, m, ModuleCall::ClearLastAttacker);
                }
                13 => {
                    w.module_call(id, m, ModuleCall::EvaluateVisualCondition);
                }
                14 => {
                    w.module_call(id, m, ModuleCall::UpdateBodyParticleSystems);
                }
                15 => {
                    w.module_call(id, m, ModuleCall::OnDelete);
                }
                16 => {
                    w.module_call(id, m, ModuleCall::ApplyDamageScalar(f(o[2])));
                }
                17 => w.set_frame(o[1]),
                18 => {
                    let ob = w.objects.find_mut(id).unwrap();
                    for (b, st) in STATUSES.iter().enumerate() {
                        if o[2] >> b & 1 != 0 {
                            ob.status.set(*st, i32::from(o[3] != 0));
                        }
                    }
                }
                21 => {
                    let pl = w.players.get_nth_mut((o[1] & 3) as i32).unwrap();
                    pl.logical_retaliation_mode_enabled = o[2] != 0;
                    pl.player_type = if o[3] != 0 { PlayerType::Computer } else { PlayerType::Human };
                }
                22 => {
                    w.cuts.body.able_to_attack.insert((o[1] as i32 + 1, o[2] as i32 + 1), o[3] as i32);
                }
                23 => {
                    w.cuts.body.distance_squared.insert((o[1] as i32 + 1, o[2] as i32 + 1), f(o[3]));
                }
                24 => {
                    w.cuts.body.retaliation_friends = (0..o[1].min(3)).map(|i| o[2 + i as usize] as i32).filter(|&x| x >= 1 && x as usize <= n).collect();
                }
                26 => {
                    let ob = w.objects.find_mut((o[1] & 3).min(n as u32 - 1) as i32 + 1).unwrap();
                    if let Some(UpdateBody::ModuleProbe(p)) = ob.modules.iter_mut().filter_map(|s| s.body.as_mut()).find(|b| matches!(b, UpdateBody::ModuleProbe(_))) {
                        match o[2] {
                            0 => p.answers.moving = o[3] != 0,
                            1 => p.answers.idle = o[3] != 0,
                            2 => p.answers.immune = o[3] != 0,
                            3 => p.answers.rider_change = o[3] != 0,
                            4 => p.answers.garrisonable = o[3] != 0,
                            _ => p.answers.contained = (0..o[3].min(3)).map(|i| ((o[4] >> (4 * i)) & 15) as i32).filter(|&x| x >= 1 && x as usize <= n).collect(),
                        }
                    }
                }
                27 => w.reads.local_selection = (0..o[1].min(3)).map(|i| ((o[2] >> (4 * i)) & 3) as i32 + 1).filter(|&x| x as usize <= n).collect(),
                28 => w.objects.find_mut((o[1] & 3).min(n as u32 - 1) as i32 + 1).unwrap().contained_by = o[2] as i32,
                29..=31 => {
                    let r = run_xfer_body(&mut w, id, m as usize, o[0] - 29, &saved);
                    xst = r.0;
                    xbytes = r.1;
                    crc = r.2;
                    if o[0] == 29 {
                        saved = xbytes.clone();
                    }
                    tally.borrow_mut().xfers += 1;
                }
                32 => w.set_effectively_dead(id, o[2] != 0),
                33 => {
                    w.friend_bind_to_drawable(id, if o[2] != 0 { 7 + id } else { INVALID_DRAWABLE_ID });
                    events_before = w.events.events().len();
                }
                34 => w.objects.find_mut(id).unwrap().experience.level = veterancy(o[2] as i32),
                36 => {
                    w.module_call(id, m, ModuleCall::SetConstructorObject(o[2] as i32));
                }
                _ => {}
            }
        } else {
            events_before = w.events.events().len();
        }
        if w.cuts.module_errors.len() > errors_before {
            assert!(w.cuts.module_errors[errors_before..].iter().all(|(_, e)| e.is_undefined()), "{:?}", w.cuts.module_errors);
            excluded = true;
        }
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let trace = w.cuts.trace.take().unwrap_or_default();
        let cuts = w.cuts.log.take().unwrap_or_default();
        let body_idx: Vec<u16> = ids.iter().map(|&id| w.objects.find(id).and_then(|ob| ob.body).unwrap_or(0)).collect();
        let log = if phase == 0 { Vec::new() } else { effect_lines(trace, cuts, events, &mut fx, &body_idx, tally) };
        tally.borrow_mut().effects += log.len() as u64;
        for l in &log {
            *tally.borrow_mut().words.entry(l.split(' ').next().unwrap().to_string()).or_default() += 1;
        }
        put_i32(&mut v, 0);
        put_u32(&mut v, result);
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        put_i32(&mut v, xst);
        put_u32(&mut v, xbytes.len() as u32);
        for b in &xbytes {
            put_u32(&mut v, u32::from(*b));
        }
        put_u32(&mut v, crc);
        for &id in &ids {
            put_object(&mut v, &mut w, id);
        }
        for p in 0..4 {
            let pl = w.players.get_nth(p).unwrap();
            let mut m = 0u32;
            for i in 0..16 {
                if pl.attacked_by[i] {
                    m |= 1 << i;
                }
            }
            put_u32(&mut v, m);
            put_u32(&mut v, pl.attacked_frame);
        }
    }
    RunOut { dump: v, keys: Vec::new(), excluded }
}

fn make_info(sc: &Scene, i: usize) -> DamageInfo {
    let (src, t, death, fxo, amount, kill, status, dealt, clipped) = sc.infos[i];
    let mut info = DamageInfo::default();
    info.input.source_id = if src == 0 { INVALID_ID } else { src as i32 };
    info.input.damage_type = t;
    info.input.death_type = death;
    info.input.damage_fx_override = fxo;
    info.input.amount = amount;
    info.input.kill = kill;
    info.input.damage_status_type = status;
    info.out.actual_damage_dealt = dealt;
    info.out.actual_damage_clipped = clipped;
    info
}

#[test]
fn active_body_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "body module P1",
        seed: 0x00AB_0D1E,
        cases_var: "L9AB_CASES",
        ini_stem: "Ab",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9ab_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "body module P1: {} cases, {} data blocks threw, {} excluded (undefined upstream), {} ops, {} damage hits, {} deaths, {} kills scored, {} xfers, {} effect lines, {} fx, {} particle systems, {} retaliation checks, {} subdual hits",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.damages, t.deaths, t.kills, t.xfers, t.effects, t.fxs, t.particle_systems, t.retaliations, t.subdual
    );
    println!("log lines by kind: {:?}", t.words);
    assert!(c.cases > 0);
}

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::ActiveBody(Arc::new(ActiveBodyModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn read_undead_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::UndeadBody(Arc::new(UndeadBodyModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn read_hive_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::HiveStructureBody(Arc::new(HiveStructureBodyModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }, BlockParse { token: "U", parse: read_undead_block }, BlockParse { token: "H", parse: read_hive_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}
