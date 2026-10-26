// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::weapon` (`WeaponTemplate`, `WeaponStore`, the `Weapon` INI block)
//! against upstream's own `Weapon.cpp` (commit 0a05454d), the template and store regions of it, compiled by
//! `differential/wrap_weapon.cpp` (see build.rs: the regions that need an `Object`, the partition manager, the
//! drawable or the damage application are cut out, in memory, every cut asserted to occur once) with the REAL
//! `INI::load`, `NameKeyGenerator` and `XferCRC` underneath. Phase 1 layer 8, p1w7-sonnet, slice A.
//!
//! No game INI files are available, so every file is **synthetic**: `Weapon` blocks (repeated names, names
//! differing only in case, every one of the 80 fields in its several forms, `None` and named FX lists,
//! particle systems and OCLs, the per-veterancy forms, the one- and two-number forms of `DelayBetweenShots`,
//! bit lists with `NONE`/`+`/`-`, `WeaponBonus` lines, `HistoricBonusWeapon` naming this template, another,
//! `None` and a stranger, malformed values, missing tokens, unknown fields, a missing `End`, comments, `CRLF`,
//! and in a share of cases random byte damage to the whole file), loaded as `INI_LOAD_OVERWRITE` and, in a share
//! of cases, a second file as `INI_LOAD_CREATE_OVERRIDES` (which upstream parses into a copy it drops).
//!
//! # Method
//!
//! 1. **INI blocks.** Both sides load the same files. Compared: what `INI::load` throws (nothing, an
//!    `INIException` and its whole message, an `INI_*` status code, an `ErrorCode`), the INI `XferCRC`, **every
//!    field of every template, in store order** (80 fields, the reals bitwise, strings, the FX and particle names,
//!    the bonus set, the scatter targets), also when the load threw, and the keys `nameToKey` then gives to a
//!    list of names (the key assignment order is lockstep state).
//! 2. `postProcessLoad` (the projectile and OCL lookups; the factory and the store answer for the names that
//!    start with `Proj` and `OCL_`), then the dump again.
//! 3. The getters under a random bonus (attack range, minimum range, delay between shots with its logic random
//!    draw, clip reload time, pre-attack delay, damage and radius, contact weapon, auto-reload), bitwise.
//! 4. `findWeaponTemplate` (known, unknown, `None`, other case), delayed damage (`setDelayedDamage`,
//!    `deleteAllDelayedDamage`), the historic damage list and the store's `reset`, with the dump after each.
//!
//! Inputs for which upstream is undefined behaviour (`DelayBetweenShots` with a missing token; a pre-attack
//! delay whose product with the bonus leaves `Int`) are **not** given to the C++; their count is printed.
//!
//! Each comparison also runs a planted bug, which must be caught.
//!
//! # Results
//!
//! See the `RESULT` lines of `cargo test --release --features differential --test differential_weapon --
//! --nocapture`. Recorded 2026-10-18: 4,000 cases, 10,097 getter queries, 30,321 finds, **0 mismatches**; 307 loads
//! and 7 getter queries excluded as undefined upstream; **18 of 18 planted bugs caught**. Detail in the
//! `weapon.rs` module docs.
#![cfg(feature = "differential")]

mod support;

use std::collections::BTreeMap;
use std::ffi::CString;

use rts_lockstep_engine::common::game_common::VETERANCY_NAMES;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Thrown, TYPE_TABLE};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::damage::{DAMAGE_TYPE_NAMES, DEATH_NAMES};
use rts_lockstep_engine::logic::weapon::{
    WeaponStore, WeaponTemplate, LEVELS, WEAPON_AFFECTS_MASK_NAMES, WEAPON_COLLIDE_MASK_NAMES, WEAPON_PREFIRE_NAMES, WEAPON_RELOAD_NAMES,
};
use rts_lockstep_engine::logic::weapon_bonus::{WeaponBonus, WEAPON_BONUS_FIELD_COUNT, WEAPON_BONUS_FIELD_NAMES, WEAPON_BONUS_NAMES};
use support::{Check, Gen};

/// The C++ side has process-global state (the name keys, the stores, `strtok`, `XferLoad`'s static buffer), so
/// the tests take turns.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// ---------------------------------------------------------------------------
// The Rust side of the INI blocks
// ---------------------------------------------------------------------------

struct Ctx {
    store: WeaponStore,
    names: NameKeyGenerator,
}

impl Ctx {
    fn new() -> Self {
        let mut names = NameKeyGenerator::new();
        names.reset();
        Ctx { store: WeaponStore::new(), names }
    }
}

fn h_weapon(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
    c.store.parse_weapon_template_definition(ini, &mut c.names)
}
fn h_stub(_: &mut Ini<'_>, _: &mut Ctx) -> IniResult<()> {
    Err(IniError::new(IniErrorKind::Bug))
}

/// The block table: `Weapon` is translated; the other handlers throw `ERROR_BUG`, as the stubs of wrap_ini.cpp do.
fn blocks() -> Vec<BlockParse<Ctx>> {
    TYPE_TABLE.iter().map(|&(token, _, _)| BlockParse { token, parse: if token == "Weapon" { h_weapon } else { h_stub } }).collect()
}

fn put_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_real(v: &mut Vec<u8>, x: f32) {
    put_u32(v, x.to_bits());
}
fn put_str(v: &mut Vec<u8>, s: &[u8]) {
    put_u32(v, s.len() as u32);
    v.extend_from_slice(s);
}
fn put_fx(v: &mut Vec<u8>, fx: &Option<Vec<u8>>) {
    match fx {
        None => v.push(0),
        Some(n) => {
            v.push(1);
            put_str(v, n);
        }
    }
}
fn put_bonus(v: &mut Vec<u8>, b: &WeaponBonus) {
    for f in 0..WEAPON_BONUS_FIELD_COUNT {
        put_real(v, b.get_field(f));
    }
}

/// The id the stand-in thing factory and OCL store compute from a name (wrap_weapon.cpp `idOf`).
fn id_of(name: &[u8]) -> u32 {
    1 + name.iter().map(|b| u32::from(*b)).sum::<u32>() % 1000
}

fn find_template(name: &[u8]) -> Option<u16> {
    name.starts_with(b"Proj").then(|| id_of(name) as u16)
}

fn find_ocl(name: &[u8]) -> Option<u32> {
    name.starts_with(b"OCL_").then(|| id_of(name))
}

/// `dumpTemplate` of wrap_weapon.cpp.
fn dump_template(v: &mut Vec<u8>, t: &WeaponTemplate) {
    put_str(v, &t.name);
    put_u32(v, t.name_key.0);
    put_str(v, &t.projectile_stream_name);
    put_str(v, &t.laser_name);
    put_str(v, &t.laser_bone_name);
    for r in [
        t.primary_damage,
        t.primary_damage_radius,
        t.secondary_damage,
        t.secondary_damage_radius,
        t.shock_wave_amount,
        t.shock_wave_radius,
        t.shock_wave_taper_off,
        t.attack_range,
        t.minimum_attack_range,
        t.request_assist_range,
        t.aim_delta,
        t.scatter_radius,
        t.scatter_target_scalar,
    ] {
        put_real(v, r);
    }
    put_u32(v, t.scatter_targets.len() as u32);
    for c in &t.scatter_targets {
        put_real(v, c.x);
        put_real(v, c.y);
    }
    put_u32(v, t.damage_type as u32);
    put_u32(v, t.death_type as u32);
    put_real(v, t.weapon_speed);
    put_real(v, t.min_weapon_speed);
    put_u32(v, u32::from(t.is_scale_weapon_speed));
    put_real(v, t.weapon_recoil);
    put_real(v, t.min_target_pitch);
    put_real(v, t.max_target_pitch);
    put_real(v, t.radius_damage_angle);
    put_str(v, &t.projectile_name);
    put_u32(v, t.projectile_tmpl.map_or(0, u32::from));
    for i in 0..LEVELS {
        put_str(v, &t.fire_ocl_names[i]);
        put_str(v, &t.projectile_detonation_ocl_names[i]);
        put_fx(v, &t.projectile_exhausts[i]);
        put_u32(v, t.fire_ocls[i].unwrap_or(0));
        put_u32(v, t.projectile_detonation_ocls[i].unwrap_or(0));
        put_fx(v, &t.fire_fxs[i]);
        put_fx(v, &t.projectile_detonate_fxs[i]);
    }
    put_str(v, &t.fire_sound);
    put_u32(v, t.fire_sound_loop_time);
    match &t.extra_bonus {
        Some(set) => {
            v.push(1);
            for i in 0..WEAPON_BONUS_NAMES.len() {
                put_bonus(v, set.bonus(i));
            }
        }
        None => v.push(0),
    }
    for x in [
        t.clip_size as u32,
        t.clip_reload_time as u32,
        t.min_delay_between_shots as u32,
        t.max_delay_between_shots as u32,
        t.continuous_fire_one_shots_needed as u32,
        t.continuous_fire_two_shots_needed as u32,
        t.continuous_fire_coast_frames,
        t.auto_reload_when_idle_frames,
        t.shots_per_barrel as u32,
        t.anti_mask,
        t.affects_mask,
        t.collide_mask,
        u32::from(t.damage_dealt_at_self_position),
        t.reload_type as u32,
        t.prefire_type as u32,
        t.historic_bonus_time,
    ] {
        put_u32(v, x);
    }
    put_real(v, t.historic_bonus_radius);
    put_u32(v, t.historic_bonus_count as u32);
    put_u32(v, t.historic_bonus_weapon.map_or(0, |k| k.0));
    for b in [t.leech_range_weapon, t.capable_of_following_waypoint, t.is_shows_ammo_pips, t.allow_attack_garrisoned_bldgs, t.play_fx_when_stealthed] {
        put_u32(v, u32::from(b));
    }
    put_u32(v, t.pre_attack_delay as u32);
    put_real(v, t.continue_attack_range);
    put_real(v, t.infantry_inaccuracy_dist);
    put_u32(v, t.damage_status_type as u32);
    put_u32(v, t.suspend_fx_delay);
    put_u32(v, u32::from(t.die_on_detonate));
    put_u32(v, t.historic_damage.len() as u32);
    for h in &t.historic_damage {
        put_u32(v, h.frame);
        put_real(v, h.location.x);
        put_real(v, h.location.y);
        put_real(v, h.location.z);
    }
}

/// The dump of wrap_weapon.cpp's `ref_weapon_dump`.
fn rust_dump(s: &WeaponStore) -> Vec<u8> {
    let mut v = Vec::new();
    put_u32(&mut v, s.templates().len() as u32);
    for t in s.templates() {
        dump_template(&mut v, t);
    }
    put_u32(&mut v, s.delayed_damage().len() as u32);
    for d in s.delayed_damage() {
        put_u32(&mut v, d.delayed_weapon.0);
        put_real(&mut v, d.delay_damage_pos.x);
        put_real(&mut v, d.delay_damage_pos.y);
        put_real(&mut v, d.delay_damage_pos.z);
        put_u32(&mut v, d.delay_damage_frame);
        put_u32(&mut v, d.delay_source_id as u32);
        put_u32(&mut v, d.delay_intended_victim_id as u32);
        put_bonus(&mut v, &d.bonus);
    }
    v
}

/// Compares two dumps; a difference is reported by its offset, with the bytes around it.
fn cmp_dump(check: &mut Check, rust: Vec<u8>, cpp: Vec<u8>, input: impl FnOnce() -> String) {
    if rust == cpp {
        check.eq(0, 0, String::new);
        return;
    }
    let at = rust.iter().zip(cpp.iter()).position(|(a, b)| a != b).unwrap_or(rust.len().min(cpp.len()));
    let (lo, hi) = (at.saturating_sub(24), at + 24);
    let slice = |v: &[u8]| v[lo.min(v.len())..hi.min(v.len())].to_vec();
    check.eq(0, 1, || format!("{}: lengths {} / {}, first difference at byte {at}: rust {:?} cpp {:?}", input(), rust.len(), cpp.len(), slice(&rust), slice(&cpp)));
}

fn cpp_dump() -> Vec<u8> {
    let mut buf = vec![0u8; 1 << 22];
    let n = unsafe { cpp::ref_weapon_dump(buf.as_mut_ptr(), buf.len() as i32) };
    assert!((n as usize) <= buf.len(), "dump overflow");
    buf.truncate(n as usize);
    buf
}

// ---------------------------------------------------------------------------
// The generator
// ---------------------------------------------------------------------------

fn pick<'a, T: ?Sized>(g: &mut Gen, xs: &[&'a T]) -> &'a T {
    xs[g.below(xs.len() as u32) as usize]
}

fn case_mix(g: &mut Gen, s: &str) -> String {
    match g.below(6) {
        0 => s.to_ascii_lowercase(),
        1 => s.to_ascii_uppercase(),
        _ => s.to_string(),
    }
}

/// A random entry of `list`, in a random case.
fn pick_mix(g: &mut Gen, list: &[&str]) -> String {
    let s = list[g.below(list.len() as u32) as usize];
    case_mix(g, s)
}

const BLOCK_NAMES: [&str; 8] = ["Rifle", "rifle", "TankGun", "Missile", "ProjBomb", "A", "Cannon_2", "X1"];
const FX_NAMES: [&str; 6] = ["FX_Small", "FX_Big", "fx_small", "FX_Explosion", "None", "none"];
const PSYS_NAMES: [&str; 4] = ["PS_Smoke", "PS_Trail", "None", "ps_smoke"];
const OCL_NAMES: [&str; 6] = ["OCL_Debris", "OCL_Crater", "ocl_debris", "SomethingElse", "None", ""];
const PROJ_NAMES: [&str; 5] = ["ProjBullet", "ProjMissile", "None", "NONE", "Rocket"];
const SOUND_NAMES: [&str; 5] = ["RifleFire", "CannonShot", "NoSound", "nosound", "None"];

fn gen_real(g: &mut Gen, clean: bool) -> String {
    match g.below(if clean { 12 } else { 16 }) {
        0 => "0".to_string(),
        1 => "1".to_string(),
        2 => "-3.5".to_string(),
        3..=8 => format!("{}", g.below(2000) as f32 / 8.0),
        9 => "1e3".to_string(),
        10 => "100000".to_string(),
        11 => format!("{:.3}", g.uniform(-50.0, 500.0)),
        12 => pick(g, &["abc", "1.2.3", "", "0x10", "nan", "1,5"]).to_string(),
        13 => "9999999999999".to_string(),
        14 => "-0".to_string(),
        _ => "3.4e38".to_string(),
    }
}

fn gen_int(g: &mut Gen, clean: bool) -> String {
    match g.below(if clean { 10 } else { 13 }) {
        0 => "0".to_string(),
        1 => "-1".to_string(),
        2..=7 => format!("{}", g.below(50)),
        8 => "2147483647".to_string(),
        9 => "-2147483648".to_string(),
        10 => pick(g, &["x", "1.5", "", "99999999999"]).to_string(),
        _ => format!("{}", g.below(100000)),
    }
}

fn gen_duration(g: &mut Gen, clean: bool) -> String {
    match g.below(if clean { 10 } else { 13 }) {
        0 => "0".to_string(),
        1..=7 => format!("{}", g.below(5000)),
        8 => "4294967295".to_string(),
        9 => "33".to_string(),
        10 => pick(g, &["-5", "x", "", "1.5"]).to_string(),
        _ => "4294967296".to_string(),
    }
}

fn gen_bool(g: &mut Gen, clean: bool) -> String {
    if clean || g.below(10) != 0 {
        pick(g, &["Yes", "No", "yes", "TRUE", "false", "1", "0"]).to_string()
    } else {
        pick(g, &["maybe", "", "2", "yess"]).to_string()
    }
}

fn gen_bits(g: &mut Gen, names: &[&str], clean: bool) -> String {
    let mut parts = Vec::new();
    match g.below(8) {
        0 => parts.push("NONE".to_string()),
        1 => {
            for _ in 0..1 + g.below(3) {
                parts.push(format!("{}{}", if g.below(2) == 0 { "+" } else { "-" }, pick_mix(g, names)));
            }
        }
        _ => {
            for _ in 0..1 + g.below(4) {
                parts.push(pick_mix(g, names));
            }
        }
    }
    if !clean && g.below(12) == 0 {
        parts.push(pick(g, &["BOGUS", "+BOGUS", "-"]).to_string());
    }
    parts.join(" ")
}

fn gen_vet(g: &mut Gen) -> String {
    pick_mix(g, &VETERANCY_NAMES)
}

fn gen_field(g: &mut Gen, clean: bool, names: &[String]) -> String {
    let real_fields = [
        "PrimaryDamage",
        "PrimaryDamageRadius",
        "SecondaryDamage",
        "SecondaryDamageRadius",
        "ShockWaveAmount",
        "ShockWaveRadius",
        "ShockWaveTaperOff",
        "AttackRange",
        "MinimumAttackRange",
        "RequestAssistRange",
        "AcceptableAimDelta",
        "ScatterRadius",
        "ScatterTargetScalar",
        "ScatterRadiusVsInfantry",
        "WeaponSpeed",
        "MinWeaponSpeed",
        "WeaponRecoil",
        "MinTargetPitch",
        "MaxTargetPitch",
        "RadiusDamageAngle",
        "HistoricBonusRadius",
        "ContinueAttackRange",
    ];
    let bool_fields = [
        "ScaleWeaponSpeed",
        "DamageDealtAtSelfPosition",
        "LeechRangeWeapon",
        "CapableOfFollowingWaypoints",
        "ShowsAmmoPips",
        "AllowAttackGarrisonedBldgs",
        "PlayFXWhenStealthed",
        "MissileCallsOnDie",
        "AntiAirborneVehicle",
        "AntiGround",
        "AntiProjectile",
        "AntiSmallMissile",
        "AntiMine",
        "AntiParachute",
        "AntiAirborneInfantry",
        "AntiBallisticMissile",
    ];
    let int_fields = ["ClipSize", "ContinuousFireOne", "ContinuousFireTwo", "ShotsPerBarrel", "HistoricBonusCount"];
    let duration_fields = [
        "FireSoundLoopTime",
        "ContinuousFireCoast",
        "AutoReloadWhenIdle",
        "ClipReloadTime",
        "HistoricBonusTime",
        "PreAttackDelay",
        "SuspendFXDelay",
    ];
    match g.below(40) {
        0..=7 => format!("{} = {}", pick(g, &real_fields), gen_real(g, clean)),
        8..=11 => format!("{} = {}", pick(g, &bool_fields), gen_bool(g, clean)),
        12..=13 => format!("{} = {}", pick(g, &int_fields), gen_int(g, clean)),
        14..=16 => format!("{} = {}", pick(g, &duration_fields), gen_duration(g, clean)),
        17 => format!("DamageType = {}", {
            if clean || g.below(10) != 0 {
                pick_mix(g, &DAMAGE_TYPE_NAMES)
            } else {
                pick(g, &["BOGUS", "", "KILLPILOT"]).to_string()
            }
        }),
        18 => format!("DamageStatusType = {}", {
            if clean || g.below(10) != 0 {
                pick(g, &["BURNED", "AFLAME", "WET", "STEALTHED", "NONE", "DISGUISED", "FAERIE_FIRE"]).to_string()
            } else {
                pick(g, &["BOGUS", ""]).to_string()
            }
        }),
        19 => format!("DeathType = {}", {
            if clean || g.below(10) != 0 {
                pick_mix(g, &DEATH_NAMES)
            } else {
                pick(g, &["BOGUS", ""]).to_string()
            }
        }),
        20 => format!("ProjectileObject = {}", pick(g, &PROJ_NAMES)),
        21 => format!("FireSound = {}", pick(g, &SOUND_NAMES)),
        22 => format!("FireFX = {}", pick(g, &FX_NAMES)),
        23 => format!("ProjectileDetonationFX = {}", pick(g, &FX_NAMES)),
        24 => format!("FireOCL = {}", pick(g, &OCL_NAMES)),
        25 => format!("ProjectileDetonationOCL = {}", pick(g, &OCL_NAMES)),
        26 => format!("ProjectileExhaust = {}", pick(g, &PSYS_NAMES)),
        27 => {
            let v = if clean || g.below(12) != 0 { gen_vet(g) } else { "ULTRA".to_string() };
            match g.below(5) {
                0 => format!("VeterancyFireFX = {} {}", v, pick(g, &FX_NAMES)),
                1 => format!("VeterancyProjectileDetonationFX = {} {}", v, pick(g, &FX_NAMES)),
                2 => format!("VeterancyFireOCL = {} {}", v, pick(g, &OCL_NAMES)),
                3 => format!("VeterancyProjectileDetonationOCL = {} {}", v, pick(g, &OCL_NAMES)),
                _ => format!("VeterancyProjectileExhaust = {} {}", v, pick(g, &PSYS_NAMES)),
            }
        }
        28 => {
            // the shot delay: one number, a Min/Max pair, a Min alone, or a messed up pair
            let n = |g: &mut Gen| g.below(3000);
            match g.below(if clean { 6 } else { 9 }) {
                0..=2 => format!("DelayBetweenShots = {}", n(g)),
                3 | 4 => format!("DelayBetweenShots = Min:{} Max:{}", n(g), n(g)),
                5 => format!("DelayBetweenShots = Min:{}", n(g)),
                6 => format!("DelayBetweenShots = Min:{} Foo:{}", n(g), n(g)),
                7 => "DelayBetweenShots =".to_string(),
                _ => pick(g, &["DelayBetweenShots = Min:", "DelayBetweenShots = Max:5", "DelayBetweenShots = x"]).to_string(),
            }
        }
        29 => format!("RadiusDamageAffects = {}", gen_bits(g, &WEAPON_AFFECTS_MASK_NAMES, clean)),
        30 => format!("ProjectileCollidesWith = {}", gen_bits(g, &WEAPON_COLLIDE_MASK_NAMES, clean)),
        31 => format!("AutoReloadsClip = {}", if clean || g.below(10) != 0 { pick_mix(g, &WEAPON_RELOAD_NAMES) } else { "NEVER".to_string() }),
        32 => format!("PreAttackType = {}", if clean || g.below(10) != 0 { pick_mix(g, &WEAPON_PREFIRE_NAMES) } else { "ALWAYS".to_string() }),
        33 => format!(
            "{} = {}",
            pick(g, &["ProjectileStreamName", "LaserName", "LaserBoneName"]),
            pick(g, &["Stream1", "Laser_A", "Bone01", "None", ""])
        ),
        34 => {
            let cond = if clean || g.below(10) != 0 { WEAPON_BONUS_NAMES[g.below(WEAPON_BONUS_NAMES.len() as u32) as usize] } else { "BOGUS" };
            let field = if clean || g.below(10) != 0 { WEAPON_BONUS_FIELD_NAMES[g.below(WEAPON_BONUS_FIELD_NAMES.len() as u32) as usize] } else { "BOGUS" };
            let pct = if clean || g.below(10) != 0 { format!("{}%", g.below(400)) } else { "x%".to_string() };
            format!("WeaponBonus = {cond} {field} {pct}")
        }
        35 => {
            let target = if names.is_empty() || g.below(4) == 0 { pick(g, &["None", "none", "Nope", "Rifle"]).to_string() } else { names[g.below(names.len() as u32) as usize].clone() };
            format!("HistoricBonusWeapon = {target}")
        }
        36 => format!("ScatterTarget = X:{} Y:{}", g.uniform(-9.0, 9.0), g.uniform(-9.0, 9.0)),
        37 => pick(g, &["ScatterTarget = X:1", "ScatterTarget =", "ScatterTarget = Y:2 X:1"]).to_string(),
        38 if !clean => pick(g, &["Bogus = 1", "AntiAir = Yes", "primarydamage", "= 5", "PrimaryDamage"]).to_string(),
        _ => format!("{} = {}", pick(g, &real_fields), gen_real(g, clean)),
    }
}

fn gen_block(g: &mut Gen, lines: &mut Vec<String>, clean: bool, names: &mut Vec<String>) {
    let word = if !clean && g.below(12) == 0 { pick(g, &["Foo", "Weapons", "weapon", "WEAPON"]).to_string() } else { "Weapon".to_string() };
    let name = if g.below(8) == 0 && !names.is_empty() { names[g.below(names.len() as u32) as usize].clone() } else { pick(g, &BLOCK_NAMES).to_string() };
    let name = if !clean && g.below(25) == 0 { pick(g, &["", "\"quoted\"", "Wea pon"]).to_string() } else { name };
    if !names.contains(&name) {
        names.push(name.clone());
    }
    lines.push(format!("{word} {name}").trim_end().to_string());
    for _ in 0..g.below(if clean { 26 } else { 14 }) {
        lines.push(gen_field(g, clean, names));
    }
    if clean || g.below(15) != 0 {
        lines.push(if !clean && g.below(8) == 0 { pick(g, &["end", "END", "  End  ", "End;comment"]).to_string() } else { "End".to_string() });
    }
}

fn gen_file(g: &mut Gen, names: &mut Vec<String>) -> Vec<u8> {
    let clean = g.below(10) < 5;
    let mut lines = Vec::new();
    for _ in 0..1 + g.below(4) {
        match g.below(10) {
            0 => lines.push(pick(g, &["; a comment", "", "   ", ";", "\t"]).to_string()),
            1 if !clean => lines.push(pick(g, &["Unknown block", "=", "  =  ", "End"]).to_string()),
            _ => gen_block(g, &mut lines, clean, names),
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
            out.insert(p, if g.below(2) == 0 { 0 } else { 0x80 + g.below(0x80) as u8 });
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

// ---------------------------------------------------------------------------
// Running both sides
// ---------------------------------------------------------------------------

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
            Some(Thrown::Other) => (5, Vec::new()),
        },
    };
    Ok(Outcome { status, detail, crc })
}

fn cpp_load(name: &[u8], data: &[u8], load_type: i32, with_xfer: bool) -> Outcome {
    let cname = CString::new(name.to_vec()).unwrap();
    let mut log = vec![0u8; 16];
    let mut msg = vec![0u8; 4096];
    let (mut log_len, mut crc, mut code) = (0i32, 0u32, 0u32);
    let status = unsafe {
        cpp::ref_ini_reset();
        cpp::ref_ini_add_file(cname.as_ptr(), data.as_ptr() as *const _, data.len() as i32);
        cpp::ref_ini_load_typed(
            cname.as_ptr(),
            load_type,
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
    Outcome { status, detail, crc: if with_xfer { crc } else { 0 } }
}

fn show(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn name_probe() -> Vec<String> {
    let mut v: Vec<String> = BLOCK_NAMES.iter().map(|s| s.to_string()).collect();
    v.extend((0..40).map(|i| format!("Name{i}")));
    v.extend(["Probe", "zzz", "Nope", "ProjBullet"].map(String::from));
    v
}

fn random_bonus(g: &mut Gen) -> [f32; 5] {
    let mut b = [1.0f32; 5];
    for x in &mut b {
        *x = match g.below(8) {
            0 => 1.0,
            1 => 0.5,
            2 => 2.0,
            3 => 0.0,
            4 => 1.0 + g.below(300) as f32 / 100.0,
            5 => 0.01,
            6 => 100.0,
            _ => g.uniform(0.1, 3.0),
        };
    }
    b
}

fn bonus_of(b: &[f32; 5]) -> WeaponBonus {
    let mut w = WeaponBonus::new();
    for (i, x) in b.iter().enumerate() {
        w.set_field(i, *x);
    }
    w
}

#[derive(Default)]
struct Excluded(BTreeMap<String, u64>);
impl Excluded {
    fn add(&mut self, e: &IniError) {
        *self.0.entry(format!("{:?}", e.kind)).or_default() += 1;
    }
    fn total(&self) -> u64 {
        self.0.values().sum()
    }
}

#[test]
fn weapon_store_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let table = blocks();
    let probe = name_probe();
    let mut g = Gen::new(0x3EA9_0D11);
    let mut outcome = Check::new("Weapon INI::load (throw, CRC)");
    let mut dumps = Check::new("WeaponStore (every field of every template)");
    let mut keys = Check::new("name keys after the load");
    let mut post = Check::new("WeaponStore after postProcessLoad");
    let mut getters = Check::new("the getters under a bonus");
    let mut finds = Check::new("findWeaponTemplate");
    let mut store_ops = Check::new("delayed damage, historic damage, reset");
    let mut excluded = Excluded::default();
    let mut ctx = Ctx::new();
    let (mut ok, mut failed, mut templates, mut overrides_dropped, mut getter_calls, mut excluded_getters) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    const CASES: usize = 4_000;
    for case in 0..CASES {
        let mut block_names: Vec<String> = Vec::new();
        let data = gen_file(&mut g, &mut block_names);
        let second = (g.below(3) == 0).then(|| gen_file(&mut g, &mut block_names));
        let name = format!("Data\\INI\\Weapon{}.ini", case % 7).into_bytes();
        let with_xfer = g.below(4) != 0;
        ctx.store = WeaponStore::new();
        ctx.names.reset();
        unsafe { cpp::ref_weapon_reset() };
        let mut rust_outcomes = Vec::new();
        let mut skip = false;
        let mut files: Vec<(&Vec<u8>, i32)> = vec![(&data, IniLoadType::Overwrite as i32)];
        if let Some(s) = &second {
            files.push((s, IniLoadType::CreateOverrides as i32));
        }
        for (d, lt) in &files {
            let mut crc = XferCrc::new_without_nan_guard();
            let load_type = if *lt == IniLoadType::CreateOverrides as i32 { IniLoadType::CreateOverrides } else { IniLoadType::Overwrite };
            let before = ctx.store.templates().len();
            let r = Ini::load(&name, d, load_type, if with_xfer { Some(&mut crc as &mut dyn Xfer) } else { None }, &table, &mut ctx);
            let crc_v = if with_xfer { crc.get_crc() } else { 0 };
            if load_type == IniLoadType::CreateOverrides {
                overrides_dropped += 1;
                assert!(ctx.store.templates().len() >= before);
            }
            match rust_outcome(r, crc_v) {
                Ok(o) => rust_outcomes.push(o),
                Err(e) => {
                    excluded.add(&e);
                    skip = true;
                    break;
                }
            }
        }
        if skip {
            continue;
        }
        for (i, (d, lt)) in files.iter().enumerate() {
            let c = cpp_load(&name, d, *lt, with_xfer);
            if rust_outcomes[i].status == 0 { ok += 1 } else { failed += 1 }
            outcome.eq(&rust_outcomes[i], &c, || format!("case {case} file {i}: {:?}", show(d)));
        }
        templates += ctx.store.templates().len() as u64;
        cmp_dump(&mut dumps, rust_dump(&ctx.store), cpp_dump(), || format!("case {case}: files {:?} / {:?}", show(&data), second.as_ref().map(|s| show(s))));
        let rk: Vec<u32> = probe.iter().map(|n| ctx.names.name_to_key(n.as_bytes()).0).collect();
        let ck: Vec<u32> = probe
            .iter()
            .map(|n| {
                let c = CString::new(n.as_str()).unwrap();
                unsafe { cpp::ref_weapon_name_key(c.as_ptr()) }
            })
            .collect();
        keys.eq(rk, ck, || format!("case {case}"));

        // postProcessLoad
        ctx.store.post_process_load(&find_template, &find_ocl);
        unsafe { cpp::ref_weapon_post_process() };
        cmp_dump(&mut post, rust_dump(&ctx.store), cpp_dump(), || format!("case {case}"));

        // the getters, under a few bonuses
        let seed = g.u32();
        for idx in 0..ctx.store.templates().len() {
            for _ in 0..2 {
                let b = random_bonus(&mut g);
                let wb = bonus_of(&b);
                let t = &ctx.store.templates()[idx];
                let pre = match t.get_pre_attack_delay(&wb) {
                    Ok(p) => p,
                    Err(_) => {
                        excluded_getters += 1;
                        continue;
                    }
                };
                unsafe { cpp::ref_init_game_logic_random(seed) };
                let mut rng = RandomValues::new(0);
                rng.init_game_logic_random(seed);
                let r = [
                    t.get_attack_range(&wb).to_bits(),
                    t.get_minimum_attack_range().to_bits(),
                    t.get_unmodified_attack_range().to_bits(),
                    t.get_delay_between_shots(&wb, &mut rng) as u32,
                    t.get_clip_reload_time(&wb) as u32,
                    pre as u32,
                    t.get_primary_damage(&wb).to_bits(),
                    t.get_primary_damage_radius(&wb).to_bits(),
                    t.get_secondary_damage(&wb).to_bits(),
                    t.get_secondary_damage_radius(&wb).to_bits(),
                    u32::from(t.is_contact_weapon()),
                    u32::from(t.get_auto_reloads_clip()),
                ];
                let mut c = [0u32; 12];
                unsafe { cpp::ref_weapon_getters(idx as i32, b.as_ptr(), c.as_mut_ptr()) };
                getter_calls += 1;
                getters.eq(r.to_vec(), c.to_vec(), || format!("case {case} template {idx} bonus {b:?}"));
            }
        }

        // findWeaponTemplate
        for n in block_names.iter().map(String::as_str).chain(["None", "none", "Nope", "Rifle", "rifle"]) {
            let cn = CString::new(n).unwrap();
            let c = unsafe { cpp::ref_weapon_find(cn.as_ptr()) };
            let r = ctx
                .store
                .find_weapon_template(&mut ctx.names, n.as_bytes())
                .unwrap()
                .map(|t| ctx.store.templates().iter().position(|x| x.name_key == t.name_key).unwrap() as i32)
                .unwrap_or(-1);
            finds.eq(r, c, || format!("case {case}: {n:?}"));
        }
        // names not seen before: their keys show whether a find assigned a key to "None" or to a stranger
        let fresh = [format!("Fresh{case}a"), "None".to_string(), format!("Fresh{case}b")];
        let rk: Vec<u32> = fresh.iter().map(|n| ctx.names.name_to_key(n.as_bytes()).0).collect();
        let ck: Vec<u32> = fresh
            .iter()
            .map(|n| {
                let c = CString::new(n.as_str()).unwrap();
                unsafe { cpp::ref_weapon_name_key(c.as_ptr()) }
            })
            .collect();
        finds.eq(rk, ck, || format!("case {case}: keys after the finds"));

        // delayed damage, historic damage, reset
        let n = ctx.store.templates().len();
        if n > 0 {
            for _ in 0..g.below(4) {
                let idx = g.below(n as u32) as usize;
                let (frame, src, vic) = (g.below(500), g.below(10) as i32, g.below(10) as i32);
                let (x, y, z) = (g.uniform(-9.0, 9.0), g.uniform(-9.0, 9.0), g.uniform(0.0, 5.0));
                let b = random_bonus(&mut g);
                let key = ctx.store.templates()[idx].name_key;
                ctx.store.set_delayed_damage(key, &rts_lockstep_engine::common::base_type::Coord3D { x, y, z }, frame, src, vic, &bonus_of(&b));
                unsafe { cpp::ref_weapon_delayed(0, idx as i32, frame, src, vic, x, y, z, b.as_ptr()) };
            }
            for _ in 0..g.below(4) {
                let idx = g.below(n as u32) as usize;
                let (frame, x, y, z) = (g.below(500), g.uniform(-9.0, 9.0), g.uniform(-9.0, 9.0), g.uniform(0.0, 5.0));
                ctx.store.templates_mut()[idx].historic_damage.push(rts_lockstep_engine::logic::weapon::HistoricWeaponDamageInfo {
                    frame,
                    location: rts_lockstep_engine::common::base_type::Coord3D { x, y, z },
                });
                unsafe { cpp::ref_weapon_historic(idx as i32, frame, x, y, z) };
            }
            cmp_dump(&mut store_ops, rust_dump(&ctx.store), cpp_dump(), || format!("case {case}: after the delayed and historic damage"));
            match g.below(3) {
                0 => {
                    ctx.store.delete_all_delayed_damage();
                    unsafe { cpp::ref_weapon_delayed(1, 0, 0, 0, 0, 0.0, 0.0, 0.0, [1.0f32; 5].as_ptr()) };
                }
                1 => {
                    ctx.store.reset();
                    unsafe { cpp::ref_weapon_store_reset() };
                }
                _ => {}
            }
            cmp_dump(&mut store_ops, rust_dump(&ctx.store), cpp_dump(), || format!("case {case}: after the clean-up"));
        }
    }
    println!(
        "RESULT {CASES} cases: {ok} loads ok, {failed} loads threw, {templates} templates compared, {overrides_dropped} override files, {getter_calls} getter queries; \
         excluded as undefined upstream: {} loads {:?}, {excluded_getters} getter queries",
        excluded.total(),
        excluded.0
    );
    let total = outcome.report() + dumps.report() + keys.report() + post.report() + getters.report() + finds.report() + store_ops.report();
    assert_eq!(total, 0);
}

/// A direct check of the pieces the random files rarely reach, and of the undefined inputs (Rust only).
#[test]
fn defaults_getters_and_undefined_inputs() {
    use rts_lockstep_engine::common::ini::Undefined;
    let t = WeaponTemplate::new();
    assert_eq!(t.name, b"NoNameWeapon");
    assert_eq!(t.weapon_speed, 999999.0);
    assert_eq!(t.anti_mask, 2);
    assert_eq!(t.affects_mask, 14);
    assert_eq!(t.collide_mask, 4);
    assert_eq!(t.continuous_fire_one_shots_needed, i32::MAX);
    assert!(t.get_auto_reloads_clip());
    let wb = WeaponBonus::new();
    assert_eq!(t.get_attack_range(&wb), 0.0, "the undersize cannot make a range negative");
    assert!(t.is_contact_weapon());

    // DelayBetweenShots with a missing token is undefined upstream
    let mut ctx = Ctx::new();
    let table = blocks();
    let r = Ini::load(b"a.ini", b"Weapon A\n DelayBetweenShots =\nEnd\n", IniLoadType::Overwrite, None, &table, &mut ctx);
    assert!(r.unwrap_err().is_undefined());
    let r = Ini::load(b"a.ini", b"Weapon B\n DelayBetweenShots = Min:5\nEnd\n", IniLoadType::Overwrite, None, &table, &mut ctx);
    assert!(r.unwrap_err().is_undefined(), "after Min: the Max label is read through NULL");
    let _ = Undefined::NullTokenString;

    // a pre-attack delay whose product leaves Int
    let mut big = WeaponTemplate::new();
    big.pre_attack_delay = 1_000_000_000;
    let mut b2 = WeaponBonus::new();
    b2.set_field(4, 4.0);
    assert!(big.get_pre_attack_delay(&b2).is_err());
    assert!(big.get_pre_attack_delay(&WeaponBonus::new()).is_ok());
    println!("RESULT 3 classes of undefined input are errors in Rust (and are never sent to the C++)");
}


