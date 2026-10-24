// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::object_creation_list` (`ObjectCreationList`, its store, the five nugget classes and their INI
//! blocks), `logic::radius_decal::RadiusDecalTemplate` and `logic::modules::deliver_payload_data::DeliverPayloadData` against upstream's own
//! `ObjectCreationList.h` / `.cpp` (commit 0a05454d). Phase 1 layer 8, p1w10-sonnet.
//!
//! # What is compared against what
//!
//! `build.rs` takes `ObjectCreationList.h` and `RadiusDecal.h` unchanged and `ObjectCreationList.cpp` whole, with two mechanical edits made in
//! memory (the `PreRTS.h` include and the include block are replaced; asserted to apply once), and three verbatim pieces of other units' files
//! (`RadiusDecalTemplate`'s constructor and parser from `RadiusDecal.cpp`; the `DeliverPayloadData` class and its `getFieldParse` from
//! `DeliverPayloadAIUpdate.h`/`.cpp`; `INI::parseStaticGameLODLevel` from `GameLOD.cpp`); `differential/wrap_ocl.cpp` compiles them under
//! clang++ on the stand-ins of `differential/shim_ocl`: the objects and the modules `ObjectCreationList.cpp` finds in them (physics, body,
//! contain, drawable and its debris interface, AI, `DeliverPayloadAIUpdate`, lifetime, float, radius decal, special-power completion die,
//! slaved update, experience), the thing templates and factory, the players, the terrain, the pathfinder, the partition manager, the weapon
//! store, the particle systems, the audio, the script engine, the game logic and LOD, and the global data. The real `INI` (`INI.cpp`, with the
//! parsers it holds), the real name key generator, `AsciiString`, the game-logic random value generator, WWMath's `Matrix3D` and `Vector3`,
//! `Trig.cpp`'s `Sin`/`Cos`, and the headers `KindOf.h`, `GameCommon.h`, `Damage.h` are the real code. The Rust side runs the same program on
//! `Sim`, an [`OclHost`] that answers from the same configuration and writes the same lines.
//!
//! Every call that has an effect writes a line, formatted alike on both sides (reals as their bit patterns); calls that only answer a question
//! write none. After every operation both sides write the state of every object (position, orientation, layer, veterancy, mass, hidden,
//! transform). The store is compared as a text dump (every nugget, every field; every list with the indices of its nuggets), also when the
//! INI load threw.
//!
//! # Method
//!
//! 1. **INI.** A synthetic `ObjectCreationList` file (no game INI is available) of 1 to 5 blocks with 1 to 5 nuggets of every class, every field
//!    in its several forms, duplicate and case-different names (a redefinition clears the list and keeps the old nuggets), `None` and unknown
//!    weapons, FX lists and LOD levels, unknown fields, missing `End`, malformed values and, in a share of cases, random byte damage. Both sides
//!    load it as `INI_LOAD_OVERWRITE` (with the INI `XferCRC` in a share). Compared: what the load throws (nothing, an `INIException` and its whole
//!    message, an `INI_*` status, an `ErrorCode`) and the CRC.
//! 2. **The store dump.**
//! 3. **A program** (a world and 8 to 14 operations) on the store as parsed, even after a failed load: `create(ocl, primary, secondary, life)`,
//!    `create(ocl, primaryObj, primary, secondary, createOwner, life)`, `create(ocl, primaryObj, primary, secondary, angle, life)` and
//!    `findObjectCreationList`, the primary and secondary objects chosen from the objects of the world so far (including those the earlier
//!    operations made), positions given or `NULL`, an angle given or `INVALID_ANGLE`.
//!
//! **Inputs for which upstream is undefined** (see the module docs of `object_creation_list.rs`) make the C++ crash; the driver catches that
//! (`SIGSEGV`, `sigsetjmp`), drops the lines of the operation and ends the program with `UNDEF op=<k>`; the Rust, which returns an
//! `is_undefined()` error there, writes the same line. The two must agree on where upstream is undefined. Float-to-`Int` conversions out of range
//! are not generated.
//!
//! # Results (2026-10-21, upstream `0a05454d`)
//!
//! `OCL_CASES=3000 cargo test --release --features differential --test differential_object_creation_list -- --nocapture` (the default is 1,000):
//! **3,000 cases, 41 of them excluded before the C++ saw them (a `Real` field whose text `sscanf` reads in a platform-dependent way, as in the
//! other INI tests), so 2,959 files loaded on both sides: 1,463 without error, 1,496 with the same `INIException` message, `ErrorCode` or `INI_*`
//! status and the same INI `XferCRC`, 0 mismatches; 12,409 nuggets parsed (860 `Attack`, 1,737 `DeliverPayload`, 912 `FireWeapon`, 967
//! `ApplyRandomForce`, 7,933 `CreateObject`/`CreateDebris`), every field of every nugget and every list compared in the store dump, 0
//! mismatches; 2,959 programs, 384,399 trace lines, 0 mismatches, of which 484 programs end in `UNDEF` where upstream dereferences a null
//! pointer (the two sides stop at the same operation).** The programs make 19,262 objects, 799 temporary weapons, 1,215 payload deliveries,
//! 238 starting forces at maximum speed, 6,543 forces and 5,382 spin rates on debris, 2,138 fade audio events, 6,052 lifetime overrides, 1,227
//! inherited veterancies with their object names, 427 floating objects, 2,353 containments, 232 water damages and 911 kills on bad land, 656
//! attack orders with 736 radius decals, and 2,204 special-power creators.
//!
//! **Planted bugs.** `OCL_PLANT=<n>` switches one fault into the Rust alone (all in one build; ids 1 to 136, of which 68 was dropped while the
//! set was written and has no code). **135 faults: 134 are caught by mismatching lines or dump lines within 3,000 cases, and 1 is an
//! equivalent mutant**: 95 (the `orientation = atan2(force.y, force.x)` of the `SEND_IT_OUT` branch is a dead store upstream: the branch for
//! `SEND_IT_FLYING | SEND_IT_UP | RANDOM_FORCE` that reads `orientation` assigns it again under the same flag). The faults cover the defaults of
//! the constructors (13), the parsers (the every-other-token name list, the preload list, the error path, redefinition, `None`, the friction
//! scale, the payload count default, the bit lists and the lookups: 16), `adjustVector` and `calcRandomForce` (3), `FireWeapon` and `Attack` (4),
//! the formation of `DeliverPayload` (the rotation, the truncation of the offset multiplier, the convergence, the error radius, the heading,
//! the slop, the producer, the targetable flag, the delay, the special-power creator, the maximum-speed force, the decal radius, the preferred
//! height, the payload loops, the containers: 31), and `ApplyRandomForce`, `CreateObject`/`CreateDebris` and the lists (the live-player gate,
//! the owner, the pick range, the LOD gate, the layer, the spread formation, the lifetime override, the colour, the anim sets, every
//! disposition branch, the slaved update, veterancy inheritance, containment, the bad-land test, the fades, the container, the first object
//! returned: 68).
//!
//! Rounds of the generator that the plants forced: a lowercase and a padded weapon name (plant 26); a `WeaponErrorRadius` of 1 or less (47); a
//! negative `DeliveryDistance` (52); explicit zero rates (101); the objects made get an indicator colour of their own, so that `OkToChangeModelColor`
//! shows (76); the layer the terrain answers depends on the height asked (92); positions near the terrain, so that the water test, the layers
//! and the `+ 10` of the bad-land test come into play (113); a list named `None` or `none`, which `findObjectCreationList` never returns (21).
#![cfg(feature = "differential")]

mod support;

use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::damage_fx::FxListName;
use rts_lockstep_engine::common::disabled_types::DisabledType;
use rts_lockstep_engine::common::game_common::{CommandSourceType, VeterancyLevel};
use rts_lockstep_engine::common::game_type::{ObjectId, PathfindLayer, INVALID_ANGLE};
use rts_lockstep_engine::common::ini::{
    BlockParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Thrown, TYPE_TABLE,
};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::deliver_payload_data::{
    DeliverPayloadData, WeaponTemplateRef,
};
use rts_lockstep_engine::logic::object_creation_list::{
    FindPositionOptions, Nugget, ObjectCreationList, ObjectCreationListStore, OclError, OclHost,
    OclParseEnv, UpdateModuleKind,
};
use rts_lockstep_engine::logic::radius_decal::RadiusDecalTemplate;
use rts_lockstep_engine::logic::world::build_assistant::PathfindCellType;
use rts_lockstep_engine::logic::world::ids::{PlayerIndex, TeamId};
use rts_lockstep_engine::logic::world::team_ops::TemplateRef;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use support::{Check, Gen};

/// The C++ side has process-global state (the name keys, the store, `strtok`, the random generator), so the tests take turns.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

const N_TEMPLATES: usize = 10;
const N_WEAPONS: usize = 6;
const N_PSYS: usize = 4;

// template flags (ocl_driver.h)
const TF_CREATABLE: u32 = 1 << 0;
const TF_PHYSICS: u32 = 1 << 1;
const TF_BODY: u32 = 1 << 2;
const TF_CONTAIN: u32 = 1 << 3;
const TF_ACCEPTS: u32 = 1 << 4;
const TF_DRAWABLE: u32 = 1 << 5;
const TF_DEBRIS0: u32 = 1 << 6;
const TF_DEBRIS1: u32 = 1 << 7;
const TF_AI: u32 = 1 << 8;
const TF_DPAI: u32 = 1 << 9;
const TF_LIFE: u32 = 1 << 10;
const TF_FLOAT: u32 = 1 << 11;
const TF_RDU: u32 = 1 << 12;
const TF_SPCD: u32 = 1 << 13;
const TF_SLAVED0: u32 = 1 << 14;
const TF_SLAVED1: u32 = 1 << 15;
const TF_TRAINABLE: u32 = 1 << 16;
const TF_STRUCTURE: u32 = 1 << 17;
const TF_HIDDEN: u32 = 1 << 18;
// program flags
const PF_DEBRIS_SKIPPED: u32 = 1;
const PF_NEUTRAL_PLAYER: u32 = 2;
const PF_NEUTRAL_TEAM: u32 = 4;
const PF_SRC_PLAYER: u32 = 8;
const PF_SRC_ACTIVE: u32 = 16;
const PF_SRC_TEAM: u32 = 32;

fn fb(f: f32) -> String {
    format!("{:08x}", f.to_bits())
}
fn v3(c: &Coord3D) -> String {
    format!(
        "{:08x},{:08x},{:08x}",
        c.x.to_bits(),
        c.y.to_bits(),
        c.z.to_bits()
    )
}
fn m12(m: &Matrix3D) -> String {
    m.to_array()
        .iter()
        .map(|f| format!("{:08x}", f.to_bits()))
        .collect::<Vec<_>>()
        .join(",")
}
/// `qs`: an `AsciiString` as `<length>:<text>`.
fn qs(s: &[u8]) -> String {
    format!("{}:{}", s.len(), String::from_utf8_lossy(s))
}
fn fx_name(fx: &FxListName) -> Vec<u8> {
    fx.clone().unwrap_or_default()
}

// ---------------------------------------------------------------------------
// The dump of the store
// ---------------------------------------------------------------------------

fn decal_str(d: &RadiusDecalTemplate) -> String {
    format!(
        "{} shadow={} min={} max={} throb={} color={:08x} only={}",
        qs(&d.name),
        d.shadow_type as i32,
        fb(d.min_opacity),
        fb(d.max_opacity),
        d.opacity_throb_time,
        d.color as u32,
        i32::from(d.only_visible_to_owning_player)
    )
}

fn data_str(d: &DeliverPayloadData) -> String {
    format!(
        "{} {} {} fx={} dist={} preopen={} attempts={} dropOff={} dropVar={} dropDelay={} fire={} selfDestruct={} bones={} diveStart={} diveEnd={} slot={} perInterval={} \
         inherit={} parachute={} exitPitch={} strafeLen={} decalRadius={} weapon={} decal={}",
        qs(&d.visible_drop_bone_name),
        qs(&d.visible_sub_object_name),
        qs(&d.visible_payload_template_name),
        qs(&fx_name(&d.strafe_fx)),
        fb(d.dist_to_target),
        fb(d.pre_open_distance),
        d.max_attempts,
        v3(&d.drop_offset),
        v3(&d.drop_variance),
        d.drop_delay,
        i32::from(d.fire_weapon),
        i32::from(d.self_destruct_object),
        d.visible_num_bones,
        fb(d.dive_start_distance),
        fb(d.dive_end_distance),
        d.strafing_weapon_slot,
        d.visible_items_dropped_per_interval,
        i32::from(d.inherit_transport_velocity),
        i32::from(d.is_parachute_directly),
        fb(d.exit_pitch_rate),
        fb(d.strafe_length),
        fb(d.delivery_decal_radius),
        d.visible_payload_weapon_template.unwrap_or(0),
        decal_str(&d.delivery_decal_template)
    )
}

fn dump_nugget(n: &Nugget) -> String {
    match n {
        Nugget::FireWeapon(f) => format!("FireWeapon weapon={}", f.weapon.unwrap_or(0)),
        Nugget::Attack(a) => format!(
            "Attack radius={} shots={} slot={} decal={}",
            fb(a.delivery_decal_radius),
            a.number_of_shots,
            a.weapon_slot,
            decal_str(&a.delivery_decal_template)
        ),
        Nugget::ApplyRandomForce(a) => format!(
            "Force spin={} mag={},{} pitch={},{}",
            fb(a.spin_rate),
            fb(a.min_mag),
            fb(a.max_mag),
            fb(a.min_pitch),
            fb(a.max_pitch)
        ),
        Nugget::DeliverPayload(d) => {
            let mut s = format!(
                "Deliver spacing={} convergence={} error={} delay={} size={} startPref={} startMax={} payloads={} transport={} putIn={}",
                fb(d.formation_spacing),
                fb(d.convergence_factor),
                fb(d.error_radius),
                d.delay_delivery_frames_max,
                d.formation_size,
                i32::from(d.start_at_preferred_height),
                i32::from(d.start_at_max_speed),
                d.payload.len(),
                qs(&d.transport_name),
                qs(&d.put_in_container_name)
            );
            for p in &d.payload {
                s += &format!(" {} #{}", qs(&p.payload_name), p.payload_count);
            }
            s + " data=" + &data_str(&d.data)
        }
        Nugget::Generic(g) => {
            let mut s = format!(
                "Generic areObjects={} names={}",
                i32::from(g.name_are_objects),
                g.names.len()
            );
            for n in &g.names {
                s += &format!(" {}", qs(n));
            }
            s += &format!(
                " container={} anims={}",
                qs(&g.put_in_container),
                g.anim_sets.len()
            );
            for a in &g.anim_sets {
                s += &format!(
                    " {} {} {}",
                    qs(&a.anim_initial),
                    qs(&a.anim_flying),
                    qs(&a.anim_final)
                );
            }
            s += &format!(
                " fxFinal={} psys={} fadeSound={} bounce={}",
                qs(&fx_name(&g.fx_final)),
                qs(&g.particle_sys_name),
                qs(&g.fade_sound_name),
                qs(&g.bounce_sound)
            );
            s += &format!(
                " count={} mass={} bounciness={} friction={} offset={} disposition={} intensity={} spin={} yaw={} roll={} pitch={} mag={},{} pitchRange={},{} frames={},{} shadow={} lod={} \
                 invuln={} health={},{} fadeFrames={} dist={},{},{} objectCount={} flags={}{}{}{}{}{}{}{}{}{}{}{}",
                g.debris_to_generate,
                fb(g.mass),
                fb(g.extra_bounciness),
                fb(g.extra_friction),
                v3(&g.offset),
                g.disposition,
                fb(g.disposition_intensity),
                fb(g.spin_rate),
                fb(g.yaw_rate),
                fb(g.roll_rate),
                fb(g.pitch_rate),
                fb(g.min_mag),
                fb(g.max_mag),
                fb(g.min_pitch),
                fb(g.max_pitch),
                g.min_frames,
                g.max_frames,
                g.shadow_type as i32,
                g.min_lod_required,
                g.invulnerable_time,
                fb(g.min_health),
                fb(g.max_health),
                g.fade_frames,
                fb(g.min_distance_a_formation),
                fb(g.min_distance_b_formation),
                fb(g.max_distance_formation),
                g.object_count,
                i32::from(g.requires_live_player),
                i32::from(g.contain_inside_source_object),
                i32::from(g.preserve_layer),
                i32::from(g.ok_to_change_model_color),
                i32::from(g.orient_in_force_direction),
                i32::from(g.spread_formation),
                i32::from(g.fade_in),
                i32::from(g.fade_out),
                i32::from(g.ignore_primary_obstacle),
                i32::from(g.inherits_veterancy),
                i32::from(g.dies_on_bad_land),
                i32::from(g.skip_if_significantly_airborne)
            );
            s
        }
    }
}

/// `ref_ocl_dump` of ocl_driver.h.
fn rust_dump(store: &ObjectCreationListStore, names: &NameKeyGenerator) -> String {
    let mut out = format!(
        "store nuggets={} hack={}\n",
        store.nuggets.len(),
        store.debris_model_names_global_hack.len()
    );
    for h in &store.debris_model_names_global_hack {
        out += &format!("hack {}\n", qs(h));
    }
    for (i, n) in store.nuggets.iter().enumerate() {
        out += &format!("nugget {} {}\n", i, dump_nugget(n));
    }
    let mut lists: Vec<(Vec<u8>, &ObjectCreationList)> = store
        .ocls
        .iter()
        .map(|(k, o)| (names.key_to_name(*k).to_vec(), o))
        .collect();
    lists.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, o) in lists {
        let idx: String = o
            .nuggets
            .iter()
            .map(|n| {
                format!(
                    " {}",
                    store
                        .nuggets
                        .iter()
                        .rposition(|m| Arc::ptr_eq(m, n))
                        .map_or(-1, |i| i as i64)
                )
            })
            .collect();
        out += &format!("ocl {} n={}{}\n", qs(&name), o.nuggets.len(), idx);
    }
    out
}

fn cpp_dump() -> String {
    let mut buf = vec![0u8; 1 << 22];
    let n = unsafe { cpp::ref_ocl_dump(buf.as_mut_ptr() as *mut _, buf.len() as i32) };
    assert!((n as usize) <= buf.len(), "dump overflow");
    buf.truncate(n as usize);
    String::from_utf8_lossy(&buf).into_owned()
}

// ---------------------------------------------------------------------------
// The simulated world (the Rust twin of ocl_driver.h)
// ---------------------------------------------------------------------------

struct Tmpl {
    name: String,
    flags: u32,
    mass: f32,
    max_speed: f32,
    pref_height: f32,
    damage_state: i32,
}

#[derive(Clone)]
struct Obj {
    id: ObjectId,
    tmpl: usize,
    pos: Coord3D,
    orient: f32,
    xfrm: Matrix3D,
    layer: PathfindLayer,
    above: bool,
    sig_above: bool,
    off_map: bool,
    veterancy: i32,
    name: Vec<u8>,
    indicator_color: i32,
    dir2d: Coord3D,
    vel: Coord3D,
    mass: f32,
    hidden: bool,
    player: Option<PlayerIndex>,
}

struct Sim {
    flags: u32,
    frame: u32,
    tseed: u32,
    rng: RandomValues,
    tmpls: Vec<Tmpl>,
    objs: Vec<Obj>,
    next_id: ObjectId,
    lines: Vec<String>,
}

struct Reader<'a> {
    w: &'a [u32],
    p: usize,
}
impl Reader<'_> {
    fn u(&mut self) -> u32 {
        let v = self.w.get(self.p).copied().unwrap_or(0);
        self.p += 1;
        v
    }
    fn i(&mut self) -> i32 {
        self.u() as i32
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(self.u())
    }
}

fn h3(a: u32, b: u32, c: u32) -> u32 {
    let mut x = a.wrapping_mul(73856093) ^ b.wrapping_mul(19349663) ^ c.wrapping_mul(83492791);
    x ^= x >> 13;
    x = x.wrapping_mul(0x5bd1_e995);
    x ^= x >> 15;
    x
}

impl Sim {
    /// `setupWorld` of ocl_driver.h.
    fn new(r: &mut Reader) -> Sim {
        let seed = r.u();
        let frame = r.u();
        let flags = r.u();
        let tseed = r.u();
        let mut rng = RandomValues::new(0);
        rng.init_game_logic_random(seed);
        let mut tmpls = Vec::new();
        for i in 0..N_TEMPLATES {
            let name = if i == N_TEMPLATES - 1 {
                "GenericDebris".to_string()
            } else {
                format!("T{i}")
            };
            let flags = r.u();
            let mass = r.f();
            let max_speed = r.f();
            let pref_height = r.f();
            let damage_state = r.i();
            tmpls.push(Tmpl {
                name,
                flags,
                mass,
                max_speed,
                pref_height,
                damage_state,
            });
        }
        let mut s = Sim {
            flags,
            frame,
            tseed,
            rng,
            tmpls,
            objs: Vec::new(),
            next_id: 200,
            lines: Vec::new(),
        };
        let ti = (r.u() as usize) % N_TEMPLATES;
        let mut o = s.make_object(100, ti, Some(2));
        o.pos = Coord3D {
            x: r.f(),
            y: r.f(),
            z: r.f(),
        };
        o.orient = r.f();
        let mut m = [0.0f32; 12];
        for v in &mut m {
            *v = r.f();
        }
        o.xfrm = Matrix3D::from_array(m);
        o.dir2d = Coord3D {
            x: r.f(),
            y: r.f(),
            z: 0.0,
        };
        let (vx, vy, vz) = (r.f(), r.f(), r.f());
        o.vel = Coord3D {
            x: vx,
            y: vy,
            z: vz,
        };
        o.veterancy = (r.u() % 4) as i32;
        let name_no = r.u();
        if name_no != 0 {
            o.name = format!("N{name_no}").into_bytes();
        }
        o.layer = r.i();
        let st = r.u();
        o.above = st & 1 != 0;
        o.sig_above = st & 2 != 0;
        o.off_map = st & 4 != 0;
        o.indicator_color = r.u() as i32;
        o.player = if flags & PF_SRC_PLAYER != 0 {
            Some(1)
        } else {
            None
        };
        s.objs.push(o);
        s
    }

    /// `initObject`.
    fn make_object(&self, id: ObjectId, tmpl: usize, team: Option<TeamId>) -> Obj {
        let t = &self.tmpls[tmpl];
        Obj {
            id,
            tmpl,
            pos: Coord3D::default(),
            orient: 0.0,
            xfrm: Matrix3D::IDENTITY,
            layer: 1,
            above: false,
            sig_above: false,
            off_map: false,
            veterancy: 0,
            name: Vec::new(),
            indicator_color: 0x1100_0000u32.wrapping_add(id as u32 * 257) as i32,
            dir2d: Coord3D {
                x: 0.6,
                y: 0.8,
                z: 0.25,
            },
            vel: Coord3D::default(),
            mass: t.mass,
            hidden: t.flags & TF_HIDDEN != 0,
            // the player of the team: team 1 is player 0, team 2 is player 1
            player: team.map(|t| t as i32 - 1),
        }
    }

    fn log(&mut self, s: String) {
        self.lines.push(s);
    }
    fn ix(&self, id: ObjectId) -> usize {
        self.objs
            .iter()
            .position(|o| o.id == id)
            .expect("object exists")
    }
    fn o(&self, id: ObjectId) -> &Obj {
        &self.objs[self.ix(id)]
    }
    fn om(&mut self, id: ObjectId) -> &mut Obj {
        let i = self.ix(id);
        &mut self.objs[i]
    }
    fn tf(&self, id: ObjectId) -> u32 {
        self.tmpls[self.o(id).tmpl].flags
    }
    fn ground(&self, x: f32, y: f32) -> f32 {
        (h3(x.to_bits(), y.to_bits(), self.tseed) % 400) as f32 * 0.5
    }
    fn state_dump(&mut self) {
        let lines: Vec<String> = self
            .objs
            .iter()
            .map(|o| {
                format!(
                    "st {} pos={} or={} layer={} vet={} mass={} hidden={} xfrm={}",
                    o.id,
                    v3(&o.pos),
                    fb(o.orient),
                    o.layer,
                    o.veterancy,
                    fb(o.mass),
                    i32::from(o.hidden),
                    m12(&o.xfrm)
                )
            })
            .collect();
        self.lines.extend(lines);
    }
}

impl OclHost for Sim {
    fn frame(&self) -> u32 {
        self.frame
    }
    fn is_debris_skipped(&self) -> bool {
        self.flags & PF_DEBRIS_SKIPPED != 0
    }
    fn random_value(&mut self, lo: i32, hi: i32) -> i32 {
        self.rng.game_logic_random_value(lo, hi)
    }
    fn random_value_real(&mut self, lo: f32, hi: f32) -> f32 {
        self.rng.game_logic_random_value_real(lo, hi)
    }
    fn destroy_object(&mut self, o: ObjectId) {
        self.log(format!("destroy {o}"));
    }
    fn neutral_player_default_team(&self) -> Option<TeamId> {
        if self.flags & PF_NEUTRAL_PLAYER != 0 {
            self.player_default_team(0)
        } else {
            None
        }
    }
    fn player_default_team(&self, player: PlayerIndex) -> Option<TeamId> {
        match player {
            0 => (self.flags & PF_NEUTRAL_TEAM != 0).then_some(1),
            _ => (self.flags & PF_SRC_TEAM != 0).then_some(2),
        }
    }
    fn player_is_active(&self, player: PlayerIndex) -> bool {
        player == 0 || self.flags & PF_SRC_ACTIVE != 0
    }
    fn transfer_object_name(&mut self, name: &[u8], o: ObjectId) {
        self.log(format!("script.transfer {} {}", qs(name), o));
    }
    fn debris_template(&mut self) -> Option<TemplateRef> {
        Some(N_TEMPLATES as u32 - 1)
    }
    fn find_template(&mut self, name: &[u8]) -> Option<TemplateRef> {
        self.tmpls
            .iter()
            .position(|t| t.name.as_bytes() == name)
            .map(|i| i as u32)
    }
    fn new_object(
        &mut self,
        tmplate: Option<TemplateRef>,
        team: Option<TeamId>,
    ) -> Option<ObjectId> {
        let t = tmplate? as usize;
        if self.tmpls[t].flags & TF_CREATABLE == 0 {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        let o = self.make_object(id, t, team);
        self.objs.push(o);
        self.log(format!(
            "new {} tmpl={} team={}",
            id,
            self.tmpls[t].name,
            team.unwrap_or(0)
        ));
        Some(id)
    }
    fn obj_position(&self, o: ObjectId) -> Coord3D {
        self.o(o).pos
    }
    fn obj_orientation(&self, o: ObjectId) -> f32 {
        self.o(o).orient
    }
    fn obj_transform(&self, o: ObjectId) -> Matrix3D {
        self.o(o).xfrm
    }
    fn obj_controlling_player(&self, o: ObjectId) -> Option<PlayerIndex> {
        self.o(o).player
    }
    fn obj_layer(&self, o: ObjectId) -> PathfindLayer {
        self.o(o).layer
    }
    fn obj_is_above_terrain(&self, o: ObjectId) -> bool {
        self.o(o).above
    }
    fn obj_is_significantly_above_terrain(&self, o: ObjectId) -> bool {
        self.o(o).sig_above
    }
    fn obj_is_off_map(&self, o: ObjectId) -> bool {
        self.o(o).off_map
    }
    fn obj_is_structure(&self, o: ObjectId) -> bool {
        self.tf(o) & TF_STRUCTURE != 0
    }
    fn obj_veterancy_level(&self, o: ObjectId) -> VeterancyLevel {
        match self.o(o).veterancy {
            0 => VeterancyLevel::Regular,
            1 => VeterancyLevel::Veteran,
            2 => VeterancyLevel::Elite,
            _ => VeterancyLevel::Heroic,
        }
    }
    fn obj_indicator_color(&self, o: ObjectId) -> i32 {
        self.o(o).indicator_color
    }
    fn obj_name(&self, o: ObjectId) -> Vec<u8> {
        self.o(o).name.clone()
    }
    fn obj_unit_direction_vector_2d(&self, o: ObjectId) -> Coord3D {
        self.o(o).dir2d
    }
    fn obj_set_producer(&mut self, o: ObjectId, producer: Option<ObjectId>) {
        self.log(format!("o.producer {} {}", o, producer.unwrap_or(0)));
    }
    fn obj_set_position(&mut self, o: ObjectId, pos: &Coord3D) {
        self.log(format!("o.pos {} {}", o, v3(pos)));
        self.om(o).pos = *pos;
    }
    fn obj_set_orientation(&mut self, o: ObjectId, angle: f32) {
        self.log(format!("o.orient {} {}", o, fb(angle)));
        self.om(o).orient = angle;
    }
    fn obj_set_transform_matrix(&mut self, o: ObjectId, mtx: &Matrix3D) {
        self.log(format!("o.xfrm {} {}", o, m12(mtx)));
        self.om(o).xfrm = *mtx;
    }
    fn obj_set_layer(&mut self, o: ObjectId, layer: PathfindLayer) {
        self.log(format!("o.layer {o} {layer}"));
        self.om(o).layer = layer;
    }
    fn obj_set_script_status(&mut self, o: ObjectId, bit: u8) {
        self.log(format!("o.script {} {} 1", o, bit));
    }
    fn obj_set_disabled_until(&mut self, o: ObjectId, disabled: DisabledType, frame: u32) {
        self.log(format!("o.disabled {o} {disabled} {frame}"));
    }
    fn obj_set_weapon_lock(&mut self, o: ObjectId, slot: i32, lock: i32) {
        self.log(format!("o.weaponLock {o} {slot} {lock}"));
    }
    fn obj_go_invulnerable(&mut self, o: ObjectId, frames: u32) {
        self.log(format!("o.invulnerable {o} {frames}"));
    }
    fn obj_kill(&mut self, o: ObjectId) {
        self.log(format!("o.kill {o}"));
    }
    fn obj_attempt_damage(&mut self, o: ObjectId, info: &DamageInfo) {
        self.log(format!(
            "o.damage {} type={} death={} src={} amount={}",
            o,
            info.input.damage_type,
            info.input.death_type,
            info.input.source_id,
            fb(info.input.amount)
        ));
    }
    fn obj_has_update_module(&self, o: ObjectId, kind: UpdateModuleKind) -> bool {
        let bit = match kind {
            UpdateModuleKind::RadiusDecal => TF_RDU,
            UpdateModuleKind::DeliverPayloadAi => TF_DPAI,
            UpdateModuleKind::Lifetime => TF_LIFE,
            UpdateModuleKind::Float => TF_FLOAT,
        };
        self.tf(o) & bit != 0
    }
    fn obj_has_special_power_completion_die(&self, o: ObjectId) -> bool {
        self.tf(o) & TF_SPCD != 0
    }
    fn obj_has_ai(&self, o: ObjectId) -> bool {
        self.tf(o) & TF_AI != 0
    }
    fn obj_has_physics(&self, o: ObjectId) -> bool {
        self.tf(o) & TF_PHYSICS != 0
    }
    fn obj_has_body(&self, o: ObjectId) -> bool {
        self.tf(o) & TF_BODY != 0
    }
    fn obj_has_contain(&self, o: ObjectId) -> bool {
        self.tf(o) & TF_CONTAIN != 0
    }
    fn obj_has_drawable(&self, o: ObjectId) -> bool {
        self.tf(o) & TF_DRAWABLE != 0
    }
    fn obj_is_trainable(&self, o: ObjectId) -> bool {
        self.tf(o) & TF_TRAINABLE != 0
    }
    fn obj_set_veterancy_level(&mut self, o: ObjectId, level: VeterancyLevel) {
        self.log(format!("exp.vet {} {}", o, level as i32));
    }
    fn ai_attack_position(
        &mut self,
        o: ObjectId,
        pos: &Coord3D,
        max_shots: i32,
        cmd: CommandSourceType,
    ) {
        self.log(format!(
            "ai.attack {} {} {} {}",
            o,
            v3(pos),
            max_shots,
            cmd as i32
        ));
    }
    fn radius_decal_create(
        &mut self,
        o: ObjectId,
        tmpl: &RadiusDecalTemplate,
        radius: f32,
        pos: &Coord3D,
    ) {
        self.log(format!(
            "rdu.create {} {} r={} pos={}",
            o,
            decal_str(tmpl),
            fb(radius),
            v3(pos)
        ));
    }
    fn radius_decal_kill_when_no_longer_attacking(&mut self, o: ObjectId) {
        self.log(format!("rdu.kill {o} 1"));
    }
    fn deliver_payload(
        &mut self,
        o: ObjectId,
        move_to: &Coord3D,
        target: &Coord3D,
        data: &DeliverPayloadData,
    ) {
        self.log(format!(
            "dpai.deliver {} move={} target={} {}",
            o,
            v3(move_to),
            v3(target),
            data_str(data)
        ));
    }
    fn deliver_payload_max_speed_for_condition(&self, o: ObjectId, damage_state: i32) -> f32 {
        self.tmpls[self.o(o).tmpl].max_speed + damage_state as f32 * 0.25
    }
    fn deliver_payload_preferred_height(&self, o: ObjectId) -> f32 {
        self.tmpls[self.o(o).tmpl].pref_height
    }
    fn body_damage_state(&self, o: ObjectId) -> i32 {
        self.tmpls[self.o(o).tmpl].damage_state
    }
    fn body_set_initial_health(&mut self, o: ObjectId, percent: i32) {
        self.log(format!("body.health {o} {percent}"));
    }
    fn lifetime_set_range(&mut self, o: ObjectId, min_frames: u32, max_frames: u32) {
        self.log(format!("life.range {o} {min_frames} {max_frames}"));
    }
    fn float_set_enabled(&mut self, o: ObjectId) {
        self.log(format!("float.enable {o} 1"));
    }
    fn special_power_completion_die_set_creator(&mut self, o: ObjectId, creator: ObjectId) {
        self.log(format!("spcd.creator {o} {creator}"));
    }
    fn behavior_module_count(&self, _o: ObjectId) -> usize {
        2
    }
    fn behavior_module_has_slaved_update(&self, o: ObjectId, i: usize) -> bool {
        self.tf(o) & if i == 0 { TF_SLAVED0 } else { TF_SLAVED1 } != 0
    }
    fn behavior_module_on_enslave(&mut self, o: ObjectId, _i: usize, slaver: Option<ObjectId>) {
        self.log(format!("slaved.enslave {} {}", o, slaver.unwrap_or(0)));
    }
    fn physics_velocity(&self, o: ObjectId) -> Coord3D {
        self.o(o).vel
    }
    fn physics_mass(&self, o: ObjectId) -> f32 {
        self.o(o).mass
    }
    fn physics_apply_motive_force(&mut self, o: ObjectId, force: &Coord3D) {
        self.log(format!("phys.motive {} {}", o, v3(force)));
    }
    fn physics_apply_force(&mut self, o: ObjectId, force: &Coord3D) {
        self.log(format!("phys.force {} {}", o, v3(force)));
    }
    fn physics_set_mass(&mut self, o: ObjectId, mass: f32) {
        self.log(format!("phys.mass {} {}", o, fb(mass)));
        self.om(o).mass = mass;
    }
    fn physics_set_extra_friction(&mut self, o: ObjectId, f: f32) {
        self.log(format!("phys.friction {} {}", o, fb(f)));
    }
    fn physics_set_extra_bounciness(&mut self, o: ObjectId, b: f32) {
        self.log(format!("phys.bounciness {} {}", o, fb(b)));
    }
    fn physics_set_allow_bouncing(&mut self, o: ObjectId, b: bool) {
        self.log(format!("phys.allowBounce {} {}", o, i32::from(b)));
    }
    fn physics_set_allow_to_fall(&mut self, o: ObjectId, b: bool) {
        self.log(format!("phys.allowFall {} {}", o, i32::from(b)));
    }
    fn physics_set_bounce_sound(&mut self, o: ObjectId, name: &[u8]) {
        self.log(format!("phys.bounceSound {} {}", o, qs(name)));
    }
    fn physics_set_ignore_collisions_with(&mut self, o: ObjectId, other: Option<ObjectId>) {
        self.log(format!("phys.ignore {} {}", o, other.unwrap_or(0)));
    }
    fn physics_set_angles(&mut self, o: ObjectId, yaw: f32, pitch: f32, roll: f32) {
        self.log(format!(
            "phys.angles {} {} {} {}",
            o,
            fb(yaw),
            fb(pitch),
            fb(roll)
        ));
    }
    fn physics_set_yaw_rate(&mut self, o: ObjectId, r: f32) {
        self.log(format!("phys.yawRate {} {}", o, fb(r)));
    }
    fn physics_set_roll_rate(&mut self, o: ObjectId, r: f32) {
        self.log(format!("phys.rollRate {} {}", o, fb(r)));
    }
    fn physics_set_pitch_rate(&mut self, o: ObjectId, r: f32) {
        self.log(format!("phys.pitchRate {} {}", o, fb(r)));
    }
    fn contain_is_valid_container_for(
        &self,
        container: ObjectId,
        _obj: ObjectId,
        _check_capacity: bool,
    ) -> bool {
        self.tf(container) & TF_ACCEPTS != 0
    }
    fn contain_add(&mut self, container: ObjectId, obj: ObjectId) {
        self.log(format!("contain.add {container} {obj}"));
    }
    fn drawable_is_effectively_hidden(&self, o: ObjectId) -> bool {
        self.o(o).hidden
    }
    fn drawable_set_hidden(&mut self, o: ObjectId, hidden: bool) {
        self.log(format!("draw.hidden {} {}", o, i32::from(hidden)));
        self.om(o).hidden = hidden;
    }
    fn drawable_fade_in(&mut self, o: ObjectId, frames: u32) {
        self.log(format!("draw.fadeIn {o} {frames}"));
    }
    fn drawable_fade_out(&mut self, o: ObjectId, frames: u32) {
        self.log(format!("draw.fadeOut {o} {frames}"));
    }
    fn drawable_debris_interface_count(&self, o: ObjectId) -> usize {
        let f = self.tf(o);
        usize::from(f & TF_DEBRIS0 != 0) + usize::from(f & TF_DEBRIS1 != 0)
    }
    fn debris_set_model_name(
        &mut self,
        o: ObjectId,
        i: usize,
        name: &[u8],
        color: i32,
        shadow_type: u32,
    ) {
        // the i-th interface is the module that has one: module 1 when module 0 has none
        let idx = self.debris_module(o, i);
        self.log(format!(
            "debris.model {} {} {} color={:08x} shadow={}",
            o,
            idx,
            qs(name),
            color as u32,
            shadow_type as i32
        ));
    }
    fn debris_set_anim_names(
        &mut self,
        o: ObjectId,
        i: usize,
        initial: &[u8],
        flying: &[u8],
        fin: &[u8],
        final_fx: &FxListName,
    ) {
        let idx = self.debris_module(o, i);
        self.log(format!(
            "debris.anim {} {} {} {} {} fx={}",
            o,
            idx,
            qs(initial),
            qs(flying),
            qs(fin),
            qs(&fx_name(final_fx))
        ));
    }
    fn terrain_ground_height(&self, x: f32, y: f32) -> f32 {
        self.ground(x, y)
    }
    fn terrain_highest_layer_for_destination(&self, pos: &Coord3D) -> PathfindLayer {
        match h3(
            pos.x.to_bits(),
            pos.y.to_bits(),
            self.tseed ^ 0x55 ^ if pos.z > 1000.0 { 0x1000 } else { 0 },
        ) % 8
        {
            0..=4 => 1,
            5 => 2,
            6 => 3,
            _ => 15,
        }
    }
    fn terrain_layer_height(&self, x: f32, y: f32, layer: PathfindLayer) -> f32 {
        self.ground(x, y) + layer as f32 * 0.5
    }
    fn terrain_flatten(&mut self, o: ObjectId) {
        self.log(format!("terrain.flatten {o}"));
    }
    fn terrain_is_underwater(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        let h = h3(x.to_bits(), y.to_bits(), self.tseed ^ 0x77);
        if !h.is_multiple_of(5) {
            return None;
        }
        Some((
            self.ground(x, y) + (h / 5 % 16) as f32 * 0.5,
            self.ground(x, y),
        ))
    }
    fn pathfinder_cell_type(
        &self,
        layer: PathfindLayer,
        x: i32,
        y: i32,
    ) -> Option<PathfindCellType> {
        let h = h3(
            (layer as u32).wrapping_mul(7919).wrapping_add(x as u32),
            y as u32,
            self.tseed ^ 0x99,
        ) % 11;
        if h == 10 {
            return None;
        }
        Some(match h % 7 {
            0 => PathfindCellType::Clear,
            1 => PathfindCellType::Water,
            2 => PathfindCellType::Cliff,
            3 => PathfindCellType::Rubble,
            4 => PathfindCellType::Obstacle,
            5 => PathfindCellType::BridgeImpassable,
            _ => PathfindCellType::Impassable,
        })
    }
    fn pathfinder_add_object(&mut self, o: ObjectId) {
        self.log(format!("pathfind.add {o}"));
    }
    fn partition_find_position_around(&mut self, c: &Coord3D, o: &FindPositionOptions) -> Coord3D {
        Coord3D {
            x: c.x + o.min_radius * 0.5 + o.max_radius * 0.25,
            y: c.y - o.min_radius * 0.25,
            z: c.z + if o.flags & 0x80 != 0 { 1.0 } else { 0.0 },
        }
    }
    fn weapon_create_and_fire_temp(
        &mut self,
        weapon: WeaponTemplateRef,
        source: ObjectId,
        pos: &Coord3D,
    ) {
        self.log(format!(
            "weapon.fire w={} src={} pos={}",
            weapon,
            source,
            v3(pos)
        ));
    }
    fn particle_system_find_template(&self, name: &[u8]) -> Option<u32> {
        (0..N_PSYS as u32)
            .find(|i| name == format!("P{i}").as_bytes())
            .map(|i| i + 1)
    }
    fn particle_system_create_attached(&mut self, tmpl: u32, o: ObjectId) {
        self.log(format!("psys.attach {o} tmpl={tmpl}"));
    }
    fn audio_add_event(&mut self, name: &[u8], object: ObjectId) {
        self.log(format!("audio {} obj={}", qs(name), object));
    }
}

impl Sim {
    /// The module index (0 or 1) of the `i`-th draw module that has a debris interface.
    fn debris_module(&self, o: ObjectId, i: usize) -> usize {
        let f = self.tf(o);
        let has = [f & TF_DEBRIS0 != 0, f & TF_DEBRIS1 != 0];
        (0..2).filter(|m| has[*m]).nth(i).expect("interface exists")
    }
}

// ---------------------------------------------------------------------------
// The operations of a program (the Rust twin of doOp)
// ---------------------------------------------------------------------------

fn sorted_names(store: &ObjectCreationListStore, names: &NameKeyGenerator) -> Vec<Vec<u8>> {
    let mut v: Vec<Vec<u8>> = store
        .ocls
        .keys()
        .map(|k| names.key_to_name(*k).to_vec())
        .collect();
    v.sort();
    v
}

fn sel(sim: &Sim, k: u32) -> Option<ObjectId> {
    (k != 0).then(|| sim.objs[(k as usize - 1) % sim.objs.len()].id)
}

fn do_op(
    sim: &mut Sim,
    store: &ObjectCreationListStore,
    names: &mut NameKeyGenerator,
    op: u32,
    r: &mut Reader,
) -> Result<(), OclError> {
    match op {
        0 => {
            let (oi, ps, ss, life) = (r.u(), r.u(), r.u(), r.u());
            let v = sorted_names(store, names);
            let Some(ocl) = (!v.is_empty())
                .then(|| store.find_object_creation_list(names, &v[oi as usize % v.len()]))
                .flatten()
            else {
                sim.log("skip".to_string());
                return Ok(());
            };
            let (p, s) = (sel(sim, ps), sel(sim, ss));
            let first = ocl.create_object(sim, p, s, life)?;
            sim.log(format!("ret obj first={}", first.unwrap_or(0)));
        }
        1 | 2 => {
            let (oi, ps, has_p) = (r.u(), r.u(), r.u());
            let p = Coord3D {
                x: r.f(),
                y: r.f(),
                z: r.f(),
            };
            let has_s = r.u();
            let s = Coord3D {
                x: r.f(),
                y: r.f(),
                z: r.f(),
            };
            let flag_or_angle = r.u();
            let angle = r.f();
            let life = r.u();
            let v = sorted_names(store, names);
            let Some(ocl) = (!v.is_empty())
                .then(|| store.find_object_creation_list(names, &v[oi as usize % v.len()]))
                .flatten()
            else {
                sim.log("skip".to_string());
                return Ok(());
            };
            let pp = (has_p & 1 != 0).then_some(&p);
            let sp = (has_s & 1 != 0).then_some(&s);
            let po = sel(sim, ps);
            let first = if op == 1 {
                ocl.create_with_owner(sim, po, pp, sp, flag_or_angle & 1 != 0, life)?
            } else {
                ocl.create_at(sim, po, pp, sp, angle, life)?
            };
            sim.log(format!(
                "ret {} first={}",
                if op == 1 { "owner" } else { "angle" },
                first.unwrap_or(0)
            ));
        }
        3 => {
            let ni = r.u();
            let mut v = sorted_names(store, names);
            v.extend(["None", "none", "Nope", "t0"].map(|s| s.as_bytes().to_vec()));
            let name = v[ni as usize % v.len()].clone();
            let o = store.find_object_creation_list(names, &name);
            let line = format!(
                "ret find {} found={} nuggets={}",
                String::from_utf8_lossy(&name),
                i32::from(o.is_some()),
                o.map_or(0, |o| o.nuggets.len())
            );
            sim.log(line);
        }
        _ => sim.log("ret nop".to_string()),
    }
    Ok(())
}

/// `ref_ocl_run`: the lines of a program.
fn run_rust(
    store: &ObjectCreationListStore,
    names: &mut NameKeyGenerator,
    words: &[u32],
) -> Vec<String> {
    let mut r = Reader { w: words, p: 0 };
    let mut sim = Sim::new(&mut r);
    let n_ops = r.i();
    for k in 0..n_ops {
        let before = sim.lines.len();
        let op = r.u();
        match do_op(&mut sim, store, names, op, &mut r) {
            Ok(()) => sim.state_dump(),
            Err(e) => {
                assert!(e.is_undefined());
                if std::env::var_os("OCL_SHOW").is_some() {
                    eprintln!("(rust undefined at op {k}: {e:?})");
                }
                sim.lines.truncate(before);
                sim.lines.push(format!("UNDEF op={k}"));
                break;
            }
        }
    }
    sim.lines
}

fn run_cpp(words: &[u32]) -> Vec<String> {
    let mut cap = 1usize << 20;
    loop {
        let mut buf = vec![0u8; cap];
        let n = unsafe {
            cpp::ref_ocl_run(
                words.as_ptr(),
                words.len() as i32,
                buf.as_mut_ptr() as *mut _,
                cap as i32,
            )
        } as usize;
        if n <= cap {
            buf.truncate(n);
            return String::from_utf8_lossy(&buf)
                .lines()
                .map(String::from)
                .collect();
        }
        cap = n + 1;
    }
}

// ---------------------------------------------------------------------------
// The generators
// ---------------------------------------------------------------------------

fn pick<'a, T: ?Sized>(g: &mut Gen, xs: &[&'a T]) -> &'a T {
    xs[g.below(xs.len() as u32) as usize]
}
fn range(g: &mut Gen, lo: i32, hi: i32) -> i32 {
    lo + g.below((hi - lo + 1) as u32) as i32
}
/// A real with a few decimals, in `[lo, hi]`.
fn real_in(g: &mut Gen, lo: f32, hi: f32) -> String {
    let steps = ((hi - lo) * 4.0) as u32;
    format!("{}", lo + g.below(steps + 1) as f32 * 0.25)
}
fn junk(g: &mut Gen) -> &'static str {
    pick(
        g,
        &["abc", "", "1.5.2", "--3", "Yes please", "1,5", "0x", "e5"],
    )
}
fn gen_real(g: &mut Gen, clean: bool, lo: f32, hi: f32) -> String {
    if !clean && g.below(14) == 0 {
        return junk(g).to_string();
    }
    real_in(g, lo, hi)
}
fn gen_int(g: &mut Gen, clean: bool, lo: i32, hi: i32) -> String {
    if !clean && g.below(14) == 0 {
        return junk(g).to_string();
    }
    format!("{}", range(g, lo, hi))
}
fn gen_bool(g: &mut Gen, clean: bool) -> String {
    if !clean && g.below(14) == 0 {
        return junk(g).to_string();
    }
    if !clean && g.below(8) == 0 {
        return pick(g, &["TRUE", "false", "1", "0"]).to_string();
    }
    pick(g, &["Yes", "No", "yes", "NO"]).to_string()
}
fn gen_duration(g: &mut Gen, clean: bool) -> String {
    if !clean && g.below(14) == 0 {
        return junk(g).to_string();
    }
    format!("{}", g.below(4000))
}
fn gen_percent(g: &mut Gen, clean: bool) -> String {
    if !clean && g.below(14) == 0 {
        return junk(g).to_string();
    }
    match g.below(4) {
        0 => format!("{}%", g.below(151)),
        1 => format!("{}", g.below(151)),
        2 => "100%".to_string(),
        _ => format!("{}.5%", g.below(100)),
    }
}
fn gen_coord(g: &mut Gen, clean: bool) -> String {
    if !clean && g.below(14) == 0 {
        return pick(g, &["X:1 Y:2", "1 2 3", "X:a Y:b Z:c", ""]).to_string();
    }
    format!(
        "X:{} Y:{} Z:{}",
        real_in(g, -20.0, 20.0),
        real_in(g, -20.0, 20.0),
        real_in(g, -5.0, 20.0)
    )
}
const OCL_NAMES: [&str; 9] = [
    "OCL_A", "OCL_B", "OCL_C", "OCL_D", "ocl_a", "OCL_E", "OCL_F", "None", "none",
];
const TEMPLATE_POOL: [&str; 12] = [
    "T0",
    "T1",
    "T2",
    "T3",
    "T4",
    "T5",
    "T6",
    "T7",
    "T8",
    "GenericDebris",
    "Nope",
    "t0",
];
const WEAPON_POOL: [&str; 10] = [
    "W0", "W1", "W2", "W3", "W4", "W5", "None", "Wnope", "w2", "W3 ",
];
const FX_POOL: [&str; 5] = ["FX_A", "FX_B", "None", "none", "FX_C"];
const PSYS_POOL: [&str; 6] = ["P0", "P1", "P2", "P3", "Pnope", "None"];
const MODEL_POOL: [&str; 5] = ["M_a", "M_b", "M_c", "M_d", "M_e"];
const SOUND_POOL: [&str; 4] = ["Snd1", "Snd2", "NoSound", "nosound"];
const SHADOW_POOL: [&str; 9] = [
    "SHADOW_NONE",
    "SHADOW_DECAL",
    "SHADOW_VOLUME",
    "SHADOW_PROJECTION",
    "SHADOW_DYNAMIC_PROJECTION",
    "SHADOW_DIRECTIONAL_PROJECTION",
    "SHADOW_ALPHA_DECAL",
    "SHADOW_ADDITIVE_DECAL",
    "NONE",
];
const DISPOSITIONS: [&str; 9] = [
    "LIKE_EXISTING",
    "ON_GROUND_ALIGNED",
    "SEND_IT_FLYING",
    "SEND_IT_UP",
    "SEND_IT_OUT",
    "RANDOM_FORCE",
    "FLOATING",
    "INHERIT_VELOCITY",
    "WHIRLING",
];

fn gen_disposition(g: &mut Gen, clean: bool) -> String {
    if !clean && g.below(10) == 0 {
        return pick(
            g,
            &[
                "BOGUS",
                "",
                "NONE LIKE_EXISTING",
                "+SEND_IT_UP -ON_GROUND_ALIGNED",
                "SEND_IT_UP +FLOATING",
            ],
        )
        .to_string();
    }
    match g.below(8) {
        0 => "NONE".to_string(),
        1 => format!("+{}", pick(g, &DISPOSITIONS)),
        2 => format!("-{}", pick(g, &DISPOSITIONS)),
        _ => (0..1 + g.below(3))
            .map(|_| pick(g, &DISPOSITIONS))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// An angular velocity in degrees per second: negative (the "unset" default), zero, or a rate.
fn rate(g: &mut Gen, clean: bool) -> String {
    if g.below(6) == 0 {
        "0".to_string()
    } else {
        gen_real(g, clean, -30.0, 120.0)
    }
}

fn gen_names(g: &mut Gen, pool: &[&str]) -> String {
    (0..1 + g.below(6))
        .map(|_| pick(g, pool))
        .collect::<Vec<_>>()
        .join(" ")
}

fn gen_decal(g: &mut Gen, clean: bool, lines: &mut Vec<String>, key: &str) {
    lines.push(format!("    {key}"));
    for _ in 0..g.below(6) {
        let l = match g.below(7) {
            0 => format!(
                "      Texture = {}",
                pick(g, &["Ring", "Circle", "decal_1"])
            ),
            1 => format!(
                "      Style = {}",
                if clean || g.below(8) != 0 {
                    pick(g, &SHADOW_POOL).to_string()
                } else {
                    "BOGUS".to_string()
                }
            ),
            2 => format!("      OpacityMin = {}", gen_percent(g, clean)),
            3 => format!("      OpacityMax = {}", gen_percent(g, clean)),
            4 => format!("      OpacityThrobTime = {}", gen_duration(g, clean)),
            5 => format!(
                "      Color = R:{} G:{} B:{}",
                g.below(256),
                g.below(256),
                g.below(256)
            ),
            _ => format!("      OnlyVisibleToOwningPlayer = {}", gen_bool(g, clean)),
        };
        lines.push(l);
    }
    lines.push("    End".to_string());
}

fn gen_generic(g: &mut Gen, clean: bool, debris: bool, lines: &mut Vec<String>) {
    lines.push(format!(
        "  {}",
        if debris {
            "CreateDebris"
        } else {
            "CreateObject"
        }
    ));
    let f = |lines: &mut Vec<String>, _g: &mut Gen, s: String| lines.push(format!("    {s}"));
    // a name list first, most of the time
    if g.below(10) != 0 {
        let s = if debris {
            format!("ModelNames = {}", gen_names(g, &MODEL_POOL))
        } else {
            format!("ObjectNames = {}", gen_names(g, &TEMPLATE_POOL))
        };
        f(lines, g, s);
    }
    if !debris && g.below(4) == 0 {
        f(lines, g, "InheritsVeterancy = Yes".to_string());
    }
    if !debris && g.below(5) == 0 {
        f(lines, g, "ContainInsideSourceObject = Yes".to_string());
    }
    for _ in 0..2 + g.below(14) {
        let s = match g.below(40) {
            0 => format!("PutInContainer = {}", pick(g, &TEMPLATE_POOL)),
            1 => format!("ParticleSystem = {}", pick(g, &PSYS_POOL)),
            2..=3 => format!(
                "Count = {}",
                gen_int(g, clean, if clean { 0 } else { -1 }, 4)
            ),
            4 => format!("IgnorePrimaryObstacle = {}", gen_bool(g, clean)),
            5 => format!("OrientInForceDirection = {}", gen_bool(g, clean)),
            6 => format!("ExtraBounciness = {}", gen_real(g, clean, 0.0, 2.0)),
            7 => format!("ExtraFriction = {}", gen_real(g, clean, 0.0, 30.0)),
            8 => format!("Offset = {}", gen_coord(g, clean)),
            9..=12 => format!("Disposition = {}", gen_disposition(g, clean)),
            13..=14 => format!("DispositionIntensity = {}", gen_real(g, clean, 0.0, 12.0)),
            15 => format!("SpinRate = {}", rate(g, clean)),
            16 => format!("YawRate = {}", rate(g, clean)),
            17 => format!("RollRate = {}", rate(g, clean)),
            18 => format!("PitchRate = {}", rate(g, clean)),
            19 => format!("MinForceMagnitude = {}", gen_real(g, clean, 0.0, 10.0)),
            20 => format!("MaxForceMagnitude = {}", gen_real(g, clean, 0.0, 20.0)),
            21 => format!("MinForcePitch = {}", gen_real(g, clean, -30.0, 60.0)),
            22 => format!("MaxForcePitch = {}", gen_real(g, clean, 0.0, 90.0)),
            23 => format!("MinLifetime = {}", gen_duration(g, clean)),
            24 => format!("MaxLifetime = {}", gen_duration(g, clean)),
            25 => format!("SpreadFormation = {}", gen_bool(g, clean)),
            26 => format!("MinDistanceAFormation = {}", gen_real(g, clean, 0.0, 20.0)),
            27 => format!("MinDistanceBFormation = {}", gen_real(g, clean, 0.0, 40.0)),
            28 => format!("MaxDistanceFormation = {}", gen_real(g, clean, 0.0, 60.0)),
            29 => format!("FadeIn = {}", gen_bool(g, clean)),
            30 => format!("FadeOut = {}", gen_bool(g, clean)),
            31 => format!("FadeTime = {}", gen_duration(g, clean)),
            32 => format!("FadeSound = {}", pick(g, &SOUND_POOL)),
            33 => format!("PreserveLayer = {}", gen_bool(g, clean)),
            34 => format!("DiesOnBadLand = {}", gen_bool(g, clean)),
            35 if !clean => (if debris { "Bogus = 1" } else { "Bogus 2" }).to_string(),
            35 => format!("Count = {}", gen_int(g, clean, 0, 4)),
            _ => {
                if debris {
                    match g.below(8) {
                        0 => format!("Mass = {}", gen_real(g, clean, 0.25, 40.0)),
                        1 => format!(
                            "AnimationSet = {} {} {}",
                            pick(g, &MODEL_POOL),
                            pick(g, &["A_fly", "A_fly2"]),
                            pick(g, &["A_end", "\"A end\""])
                        ),
                        2 => format!("FXFinal = {}", pick(g, &FX_POOL)),
                        3 => format!("OkToChangeModelColor = {}", gen_bool(g, clean)),
                        4 => format!(
                            "MinLODRequired = {}",
                            if clean || g.below(8) != 0 {
                                pick(g, &["Low", "Medium", "high", "CUSTOM"])
                            } else {
                                "Ultra"
                            }
                        ),
                        5 => format!(
                            "Shadow = {}",
                            if g.below(3) == 0 {
                                format!("{} {}", pick(g, &SHADOW_POOL), pick(g, &SHADOW_POOL))
                            } else {
                                pick(g, &SHADOW_POOL).to_string()
                            }
                        ),
                        6 => format!("BounceSound = {}", pick(g, &SOUND_POOL)),
                        _ => format!("ModelNames = {}", gen_names(g, &MODEL_POOL)),
                    }
                } else {
                    match g.below(8) {
                        0 => format!("ContainInsideSourceObject = {}", gen_bool(g, clean)),
                        1 => format!("ObjectNames = {}", gen_names(g, &TEMPLATE_POOL)),
                        2 => format!("ObjectCount = {}", gen_int(g, clean, 0, 5)),
                        3 => format!("InheritsVeterancy = {}", gen_bool(g, clean)),
                        4 => format!("SkipIfSignificantlyAirborne = {}", gen_bool(g, clean)),
                        5 => format!("InvulnerableTime = {}", gen_duration(g, clean)),
                        6 => format!("MinHealth = {}", gen_percent(g, clean)),
                        _ => {
                            if g.below(2) == 0 {
                                format!("MaxHealth = {}", gen_percent(g, clean))
                            } else {
                                format!("RequiresLivePlayer = {}", gen_bool(g, clean))
                            }
                        }
                    }
                }
            }
        };
        f(lines, g, s);
    }
    lines.push("  End".to_string());
}

fn gen_force(g: &mut Gen, clean: bool, lines: &mut Vec<String>) {
    lines.push("  ApplyRandomForce".to_string());
    for _ in 0..g.below(7) {
        let s = match g.below(5) {
            0 => format!("SpinRate = {}", gen_real(g, clean, 0.0, 200.0)),
            1 => format!("MinForceMagnitude = {}", gen_real(g, clean, 0.0, 10.0)),
            2 => format!("MaxForceMagnitude = {}", gen_real(g, clean, 0.0, 20.0)),
            3 => format!("MinForcePitch = {}", gen_real(g, clean, -30.0, 60.0)),
            _ => format!("MaxForcePitch = {}", gen_real(g, clean, 0.0, 90.0)),
        };
        lines.push(format!("    {s}"));
    }
    lines.push("  End".to_string());
}

fn gen_deliver(g: &mut Gen, clean: bool, lines: &mut Vec<String>) {
    lines.push("  DeliverPayload".to_string());
    if g.below(10) != 0 {
        lines.push(format!("    Transport = {}", pick(g, &TEMPLATE_POOL)));
    }
    if g.below(3) == 0 {
        lines.push("    StartAtMaxSpeed = Yes".to_string());
    }
    for _ in 0..2 + g.below(16) {
        let s = match g.below(32) {
            0 => format!("Transport = {}", pick(g, &TEMPLATE_POOL)),
            1 => format!("StartAtPreferredHeight = {}", gen_bool(g, clean)),
            2 => format!("StartAtMaxSpeed = {}", gen_bool(g, clean)),
            3..=4 => format!("FormationSize = {}", gen_int(g, clean, 0, 4)),
            5 => format!("FormationSpacing = {}", gen_real(g, clean, 0.0, 70.0)),
            6 => format!(
                "WeaponConvergenceFactor = {}",
                gen_real(g, clean, -1.0, 2.0)
            ),
            7 => format!(
                "WeaponErrorRadius = {}",
                if g.below(4) == 0 {
                    pick(g, &["0.5", "1", "0.25", "1.25", "0"]).to_string()
                } else {
                    gen_real(g, clean, 0.0, 30.0)
                }
            ),
            8 => format!("DelayDeliveryMax = {}", gen_duration(g, clean)),
            9..=11 => {
                let n = if g.below(3) == 0 {
                    String::new()
                } else {
                    format!(" {}", gen_int(g, clean, 0, 3))
                };
                format!("Payload = {}{}", pick(g, &TEMPLATE_POOL), n)
            }
            12 => format!("PutInContainer = {}", pick(g, &TEMPLATE_POOL)),
            13 => format!("DeliveryDistance = {}", gen_real(g, clean, -12.0, 80.0)),
            14 => format!("PreOpenDistance = {}", gen_real(g, clean, 0.0, 80.0)),
            15 => format!("MaxAttempts = {}", gen_int(g, clean, 0, 5)),
            16 => format!("DropDelay = {}", gen_duration(g, clean)),
            17 => format!("DropOffset = {}", gen_coord(g, clean)),
            18 => format!("DropVariance = {}", gen_coord(g, clean)),
            19 => format!("InheritTransportVelocity = {}", gen_bool(g, clean)),
            20 => format!("ExitPitchRate = {}", gen_real(g, clean, 0.0, 90.0)),
            21 => format!("ParachuteDirectly = {}", gen_bool(g, clean)),
            22 => format!(
                "VisibleItemsDroppedPerInterval = {}",
                gen_int(g, clean, 0, 4)
            ),
            23 => format!(
                "VisibleDropBoneBaseName = {}",
                pick(g, &["Bone", "\"Bone Two\"", "B"])
            ),
            24 => format!("VisibleSubObjectBaseName = {}", pick(g, &["Sub", "S"])),
            25 => format!("VisibleNumBones = {}", gen_int(g, clean, 0, 8)),
            26 => format!("VisiblePayloadTemplateName = {}", pick(g, &TEMPLATE_POOL)),
            27 => format!("VisiblePayloadWeaponTemplate = {}", pick(g, &WEAPON_POOL)),
            28 => format!("SelfDestructObject = {}", gen_bool(g, clean)),
            29 => match g.below(7) {
                0 => format!("FireWeapon = {}", gen_bool(g, clean)),
                1 => format!("DiveStartDistance = {}", gen_real(g, clean, 0.0, 60.0)),
                2 => format!("DiveEndDistance = {}", gen_real(g, clean, 0.0, 60.0)),
                3 => format!(
                    "StrafingWeaponSlot = {}",
                    if clean || g.below(8) != 0 {
                        pick(g, &["PRIMARY", "SECONDARY", "TERTIARY", "primary"])
                    } else {
                        "QUATERNARY"
                    }
                ),
                4 => format!("StrafeWeaponFX = {}", pick(g, &FX_POOL)),
                5 => format!("StrafeLength = {}", gen_real(g, clean, 0.0, 200.0)),
                _ => format!("DeliveryDecalRadius = {}", gen_real(g, clean, 0.0, 120.0)),
            },
            30 => {
                gen_decal(g, clean, lines, "DeliveryDecal");
                continue;
            }
            _ if !clean => "Bogus = 1".to_string(),
            _ => format!("MaxAttempts = {}", gen_int(g, clean, 0, 5)),
        };
        lines.push(format!("    {s}"));
    }
    lines.push("  End".to_string());
}

fn gen_attack(g: &mut Gen, clean: bool, lines: &mut Vec<String>) {
    lines.push("  Attack".to_string());
    for _ in 0..g.below(7) {
        match g.below(5) {
            0 => lines.push(format!("    NumberOfShots = {}", gen_int(g, clean, 0, 6))),
            1 => lines.push(format!(
                "    WeaponSlot = {}",
                if clean || g.below(8) != 0 {
                    pick(g, &["PRIMARY", "SECONDARY", "TERTIARY"])
                } else {
                    "NONE"
                }
            )),
            2 => gen_decal(g, clean, lines, "DeliveryDecal"),
            3 => lines.push(format!(
                "    DeliveryDecalRadius = {}",
                gen_real(g, clean, 0.0, 120.0)
            )),
            _ if !clean => lines.push("    Bogus = 3".to_string()),
            _ => lines.push(format!("    NumberOfShots = {}", gen_int(g, clean, 0, 6))),
        }
    }
    lines.push("  End".to_string());
}

fn gen_fire_weapon(g: &mut Gen, clean: bool, lines: &mut Vec<String>) {
    lines.push("  FireWeapon".to_string());
    for _ in 0..g.below(3) {
        lines.push(format!(
            "    Weapon = {}",
            if !clean && g.below(12) == 0 {
                ""
            } else {
                pick(g, &WEAPON_POOL)
            }
        ));
    }
    lines.push("  End".to_string());
}

fn gen_block(g: &mut Gen, clean: bool, lines: &mut Vec<String>) {
    lines.push(format!("ObjectCreationList {}", pick(g, &OCL_NAMES)));
    for _ in 0..1 + g.below(5) {
        match g.below(14) {
            0..=3 => gen_generic(g, clean, false, lines),
            4..=6 => gen_generic(g, clean, true, lines),
            7 => gen_force(g, clean, lines),
            8..=9 => gen_deliver(g, clean, lines),
            10 => gen_fire_weapon(g, clean, lines),
            11 => gen_attack(g, clean, lines),
            12 if !clean => lines.push(
                pick(
                    g,
                    &[
                        "  Bogus = 1",
                        "  CreateThing",
                        "  ",
                        "  ; comment",
                        "  CreateObject = Yes",
                    ],
                )
                .to_string(),
            ),
            _ => {
                let debris = g.below(2) == 0;
                gen_generic(g, clean, debris, lines)
            }
        }
    }
    if clean || g.below(30) != 0 {
        lines.push("End".to_string());
    }
}

fn gen_file(g: &mut Gen) -> Vec<u8> {
    let clean = g.below(10) < 5;
    let mut lines = Vec::new();
    for _ in 0..1 + g.below(5) {
        match g.below(10) {
            0 => lines.push(pick(g, &["; a comment", "", "   ", ";", "\t"]).to_string()),
            1 if !clean => lines.push(
                pick(
                    g,
                    &["Unknown block", "=", "  =  ", "End", "ObjectCreationList"],
                )
                .to_string(),
            ),
            _ => gen_block(g, clean, &mut lines),
        }
    }
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        out.extend_from_slice(l.as_bytes());
        let last = i + 1 == lines.len();
        if !(last && g.below(4) == 0) {
            out.extend_from_slice(if g.below(6) == 0 { b"\r\n" } else { b"\n" });
        }
    }
    match if clean { 99 } else { g.below(14) } {
        0 if !out.is_empty() => {
            let p = g.below(out.len() as u32) as usize;
            out.insert(
                p,
                if g.below(2) == 0 {
                    0
                } else {
                    0x80 + g.below(0x80) as u8
                },
            );
        }
        1 if !out.is_empty() => {
            let p = g.below(out.len() as u32) as usize;
            let n = 1000 + g.below(100) as usize;
            let filler: Vec<u8> = (0..n).map(|_| b"ab =1"[g.below(5) as usize]).collect();
            out.splice(p..p, filler);
        }
        2 if !out.is_empty() => {
            for _ in 0..1 + g.below(4) {
                let p = g.below(out.len() as u32) as usize;
                match g.below(3) {
                    0 => out[p] = g.u32() as u8,
                    1 => out.insert(p, g.u32() as u8),
                    _ => {
                        out.remove(p);
                        if out.is_empty() {
                            break;
                        }
                    }
                }
            }
        }
        _ => {}
    }
    out
}

/// The words of a program: the header (see ocl_driver.h), then the operations.
fn gen_prog(g: &mut Gen) -> Vec<u32> {
    let mut w: Vec<u32> = Vec::new();
    let f = |x: f32| x.to_bits();
    let coord = |g: &mut Gen| (g.below(1200) as f32 - 600.0) * 0.25;
    w.push(g.u32()); // seed
    w.push(g.below(100_000)); // frame
    let mut flags = 0u32;
    for (bit, p) in [
        (PF_DEBRIS_SKIPPED, 6),
        (PF_NEUTRAL_PLAYER, 1),
        (PF_NEUTRAL_TEAM, 1),
        (PF_SRC_PLAYER, 1),
        (PF_SRC_ACTIVE, 1),
        (PF_SRC_TEAM, 1),
    ] {
        // `p` is the odds against: 1 in 7 of PF_SRC_PLAYER and PF_NEUTRAL_PLAYER absent, ...
        if g.below(8) > p || bit == PF_DEBRIS_SKIPPED && g.below(6) == 0 {
            flags |= bit;
        }
    }
    w.push(flags);
    let tseed = g.u32();
    w.push(tseed); // terrain seed
                   // a position: x and y on a grid, z near the terrain there (so that the water, the layers and the bridges all come into play)
    let position = |g: &mut Gen| -> [f32; 3] {
        let (x, y) = (
            (g.below(1200) as f32 - 600.0) * 0.25,
            (g.below(1200) as f32 - 600.0) * 0.25,
        );
        let ground = (h3(x.to_bits(), y.to_bits(), tseed) % 400) as f32 * 0.5;
        [
            x,
            y,
            if g.below(4) == 0 {
                (g.below(1200) as f32 - 600.0) * 0.25
            } else {
                ground + (g.below(48) as f32 - 12.0) * 0.5
            },
        ]
    };
    for i in 0..N_TEMPLATES {
        let mut tf = 0u32;
        for b in 0..19 {
            let on = match 1u32 << b {
                TF_CREATABLE => g.below(12) != 0,
                TF_PHYSICS | TF_DRAWABLE => g.below(6) != 0,
                TF_BODY | TF_CONTAIN | TF_ACCEPTS => g.below(3) != 0,
                TF_DPAI | TF_SPCD => g.below(3) != 0,
                TF_STRUCTURE | TF_HIDDEN | TF_SLAVED0 | TF_SLAVED1 | TF_FLOAT => g.below(4) == 0,
                _ => g.below(2) == 0,
            };
            if on {
                tf |= 1 << b;
            }
        }
        if i == N_TEMPLATES - 1 && g.below(8) != 0 {
            tf |= TF_CREATABLE
                | TF_DRAWABLE
                | if g.below(2) == 0 {
                    TF_DEBRIS0
                } else {
                    TF_DEBRIS1
                }
                | TF_PHYSICS;
        }
        w.push(tf);
        w.push(f(0.5 + g.below(80) as f32 * 0.25)); // mass
        w.push(f(g.below(200) as f32 * 0.5)); // max speed
        w.push(f(g.below(300) as f32 * 0.5)); // preferred height
        w.push(g.below(4)); // damage state
    }
    // the source object
    w.push(g.below(N_TEMPLATES as u32));
    for c in position(g) {
        w.push(f(c));
    }
    w.push(f(g.below(400) as f32 * 0.0157)); // orientation
                                             // a transform: rotation about z, then a translation
    let (c, s) = (
        g.below(200) as f32 / 100.0 - 1.0,
        g.below(200) as f32 / 100.0 - 1.0,
    );
    let m = [
        c,
        -s,
        0.0,
        coord(g),
        s,
        c,
        0.0,
        coord(g),
        0.0,
        0.0,
        1.0,
        coord(g),
    ];
    for x in m {
        w.push(f(x));
    }
    w.push(f(g.below(200) as f32 / 100.0 - 1.0)); // direction
    w.push(f(g.below(200) as f32 / 100.0 - 1.0));
    for _ in 0..3 {
        w.push(f(g.below(80) as f32 * 0.25 - 10.0)); // velocity
    }
    w.push(g.below(4)); // veterancy
    w.push(if g.below(3) == 0 { 0 } else { 1 + g.below(5) }); // name
    w.push(match g.below(5) {
        0 => 2,
        1 => 15,
        _ => 1,
    }); // layer
    w.push(g.below(8)); // above terrain, significantly above, off map
    w.push(g.u32()); // indicator colour
    let n_ops = 8 + g.below(7);
    w.push(n_ops);
    for _ in 0..n_ops {
        let kind = match g.below(12) {
            0..=3 => 0,
            4..=6 => 1,
            7..=9 => 2,
            _ => 3,
        };
        w.push(kind);
        let objs = 1 + g.below(6); // selectors reach the source object and the first few made
        match kind {
            0 => {
                w.push(g.below(20));
                w.push(if g.below(16) == 0 {
                    0
                } else if g.below(2) == 0 {
                    1
                } else {
                    1 + g.below(objs)
                });
                w.push(if g.below(3) == 0 {
                    0
                } else {
                    1 + g.below(objs)
                });
                w.push(if g.below(3) == 0 { 0 } else { 1 + g.below(300) });
            }
            1 | 2 => {
                w.push(g.below(20));
                w.push(if g.below(16) == 0 {
                    0
                } else if g.below(2) == 0 {
                    1
                } else {
                    1 + g.below(objs)
                });
                w.push(u32::from(g.below(10) != 0));
                for c in position(g) {
                    w.push(f(c));
                }
                w.push(u32::from(g.below(10) != 0));
                for c in position(g) {
                    w.push(f(c));
                }
                w.push(g.below(2));
                w.push(if g.below(2) == 0 {
                    f(INVALID_ANGLE)
                } else {
                    f(g.below(1400) as f32 * 0.01 - 7.0)
                });
                w.push(if g.below(3) == 0 { 0 } else { 1 + g.below(300) });
            }
            _ => w.push(g.below(40)),
        }
    }
    w
}

// ---------------------------------------------------------------------------
// Running both sides
// ---------------------------------------------------------------------------

struct Ctx {
    store: ObjectCreationListStore,
    names: NameKeyGenerator,
    env: Arc<OclParseEnv>,
}

impl Ctx {
    fn new(preload_assets: bool) -> Self {
        let mut names = NameKeyGenerator::new();
        names.reset();
        let weapon_templates = (0..N_WEAPONS)
            .map(|i| (format!("W{i}").into_bytes(), i as u32 + 1))
            .collect();
        Ctx {
            store: ObjectCreationListStore::new(),
            names,
            env: Arc::new(OclParseEnv {
                preload_assets,
                weapon_templates,
            }),
        }
    }
}

fn h_ocl(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.store
        .parse_object_creation_list_definition(ini, &mut c.names, &c.env)
}
fn h_stub(_: &mut Ini<'_>, _: &mut Ctx) -> IniResult<()> {
    Err(IniError::new(IniErrorKind::Bug))
}
/// The block table: `ObjectCreationList` is translated; the other handlers throw `ERROR_BUG`, as the stubs of wrap_ini.cpp do.
fn blocks() -> Vec<BlockParse<Ctx>> {
    TYPE_TABLE
        .iter()
        .map(|&(token, _, _)| BlockParse {
            token,
            parse: if token == "ObjectCreationList" {
                h_ocl
            } else {
                h_stub
            },
        })
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    status: i32,
    detail: Vec<u8>,
    crc: u32,
}

fn rust_outcome(r: IniResult<()>, crc: u32) -> Result<Outcome, IniError> {
    let (status, detail) = match r {
        Ok(()) => (0, Vec::new()),
        Err(e) => match e.thrown() {
            None => return Err(e),
            Some(Thrown::Exception(m)) => (1, m),
            Some(Thrown::Status(c)) => (2, c.to_le_bytes().to_vec()),
            Some(Thrown::ErrorCode(c)) => (3, c.to_le_bytes().to_vec()),
            Some(Thrown::OutOfRange) => (4, Vec::new()),
        },
    };
    Ok(Outcome {
        status,
        detail,
        crc,
    })
}

fn cpp_load(name: &[u8], data: &[u8], with_xfer: bool) -> Outcome {
    let cname = CString::new(name.to_vec()).unwrap();
    let mut log = vec![0u8; 16];
    let mut msg = vec![0u8; 4096];
    let (mut log_len, mut crc, mut code) = (0i32, 0u32, 0u32);
    let status = unsafe {
        cpp::ref_ini_reset();
        cpp::ref_ini_add_file(cname.as_ptr(), data.as_ptr() as *const _, data.len() as i32);
        cpp::ref_ini_load_typed(
            cname.as_ptr(),
            IniLoadType::Overwrite as i32,
            i32::from(with_xfer),
            log.as_mut_ptr(),
            log.len() as i32,
            &mut log_len,
            &mut crc,
            msg.as_mut_ptr() as *mut _,
            msg.len() as i32,
            &mut code,
        )
    };
    assert_eq!(log_len, 0, "the real block handlers log nothing");
    let detail = match status {
        0 | 4 => Vec::new(),
        1 => msg[..code as usize].to_vec(),
        2 | 3 => code.to_le_bytes().to_vec(),
        _ => panic!("C++ threw something unexpected (status {status})"),
    };
    Outcome {
        status,
        detail,
        crc: if with_xfer { crc } else { 0 },
    }
}

fn show(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn cases() -> u64 {
    std::env::var("OCL_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000)
}

/// Compares two texts line by line; a miss shows the first difference with the lines before it.
fn cmp_lines(check: &mut Check, rust: &[String], cpp: &[String], input: impl Fn() -> String) {
    check.eq(rust.len(), cpp.len(), || format!("{}: line count", input()));
    if let Some(i) = rust.iter().zip(cpp.iter()).position(|(a, b)| a != b) {
        check.eq(rust[i].as_str(), cpp[i].as_str(), || {
            let ctx: Vec<&str> = rust[i.saturating_sub(4)..i]
                .iter()
                .map(String::as_str)
                .collect();
            format!("{}: line {i}, after {ctx:?}", input())
        });
    } else {
        check.eq(0, 0, String::new);
    }
}

#[test]
fn object_creation_list_matches_upstream() {
    let _guard = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let table = blocks();
    let mut outcome = Check::new("ObjectCreationList INI::load (throw, CRC)");
    let mut dumps = Check::new("ObjectCreationListStore (every nugget, every list)");
    let mut runs = Check::new("programs, trace lines");
    let (mut ok, mut failed, mut excluded, mut programs, mut lines, mut undef, mut nuggets_total) =
        (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
    let mut effects: BTreeMap<String, u64> = BTreeMap::new();
    let mut excluded_kinds: BTreeMap<String, u64> = BTreeMap::new();
    let from: u64 = std::env::var("OCL_FROM")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    for case in from..from + cases() {
        let mut g = Gen::new(0x0c1d_0000 + case);
        let preload = g.below(4) == 0;
        let data = gen_file(&mut g);
        let with_xfer = g.below(4) != 0;
        let mut ctx = Ctx::new(preload);
        unsafe { cpp::ref_ocl_reset(i32::from(preload)) };
        let name = format!("Data\\INI\\ObjectCreationList{}.ini", case % 5).into_bytes();
        let mut crc = XferCrc::new_without_nan_guard();
        let r = Ini::load(
            &name,
            &data,
            IniLoadType::Overwrite,
            if with_xfer {
                Some(&mut crc as &mut dyn Xfer)
            } else {
                None
            },
            &table,
            &mut ctx,
        );
        let crc_v = if with_xfer { crc.get_crc() } else { 0 };
        if std::env::var_os("OCL_OUTCOMES").is_some() {
            eprintln!("case {case}: rust error {:?}", r.as_ref().err());
        }
        let ro = match rust_outcome(r, crc_v) {
            Ok(o) => o,
            Err(e) => {
                excluded += 1;
                *excluded_kinds.entry(format!("{:?}", e.kind)).or_default() += 1;
                continue;
            }
        };
        let co = cpp_load(&name, &data, with_xfer);
        if ro.status == 0 {
            ok += 1
        } else {
            failed += 1
        }
        if std::env::var_os("OCL_OUTCOMES").is_some() {
            eprintln!(
                "case {case}: status {} {:?}\n{}",
                ro.status,
                show(&ro.detail),
                show(&data)
            );
        }
        outcome.eq(&ro, &co, || format!("case {case}: {:?}", show(&data)));

        let rd = rust_dump(&ctx.store, &ctx.names);
        let cd = cpp_dump();
        let (rl, cl): (Vec<String>, Vec<String>) = (
            rd.lines().map(String::from).collect(),
            cd.lines().map(String::from).collect(),
        );
        cmp_lines(&mut dumps, &rl, &cl, || {
            format!("case {case}: file {:?}", show(&data))
        });
        nuggets_total += ctx.store.nuggets.len() as u64;
        for l in &rl {
            if let Some(rest) = l.strip_prefix("nugget ") {
                let kind = rest.split(' ').nth(1).unwrap_or("").to_string();
                *kinds.entry(kind).or_default() += 1;
            }
        }

        // a program on the store as parsed
        let words = gen_prog(&mut g);
        let rust = run_rust(&ctx.store, &mut ctx.names, &words);
        let cpp_lines = run_cpp(&words);
        programs += 1;
        lines += rust.len() as u64;
        if rust.last().is_some_and(|l| l.starts_with("UNDEF")) {
            undef += 1;
        }
        for l in &rust {
            let tag = l.split(' ').next().unwrap_or("");
            if tag != "st" {
                *effects.entry(tag.to_string()).or_default() += 1;
            }
        }
        cmp_lines(&mut runs, &rust, &cpp_lines, || {
            format!(
                "case {case}, store {:?}",
                rd.lines().take(3).collect::<Vec<_>>()
            )
        });
        if std::env::var_os("OCL_SHOW").is_some() {
            if let Some(i) = rust.iter().zip(cpp_lines.iter()).position(|(a, b)| a != b) {
                eprintln!("--- case {case}, first difference at line {i}");
                for k in i.saturating_sub(12)..=i {
                    let (a, b) = (&rust[k], &cpp_lines[k]);
                    if a == b {
                        eprintln!("  {k}: {a}");
                    } else {
                        eprintln!("  {k}: RUST {a}\n  {k}: C++  {b}");
                    }
                }
            }
        }
    }
    println!("RESULT files loaded ok {ok}, failed {failed} (same exception on both sides), excluded {excluded}; nuggets parsed {nuggets_total}");
    for (k, c) in &kinds {
        println!("RESULT   nugget {k:<16} {c}");
    }
    for (k, c) in &excluded_kinds {
        println!("RESULT   excluded (not given to the C++, upstream undefined) {k}: {c}");
    }
    println!("RESULT programs {programs}, ending in UNDEF (upstream undefined) {undef}, lines compared {lines}");
    for (k, c) in &effects {
        println!("RESULT   effect {k:<18} {c}");
    }
    outcome.finish();
    dumps.finish();
    runs.finish();
}

/// A scratch aid: `OCL_FILE=<path>` loads a file through the Rust parse and prints what it throws (nothing else is compared).
#[test]
fn scratch_load_file() {
    let Some(path) = std::env::var_os("OCL_FILE") else {
        return;
    };
    let data = std::fs::read(path).unwrap();
    let table = blocks();
    let mut ctx = Ctx::new(false);
    let r = Ini::load(
        b"scratch.ini",
        &data,
        IniLoadType::Overwrite,
        None,
        &table,
        &mut ctx,
    );
    println!("SCRATCH {:?}", r.as_ref().err().map(|e| e.kind.clone()));
    println!("{}", rust_dump(&ctx.store, &ctx.names));
}
