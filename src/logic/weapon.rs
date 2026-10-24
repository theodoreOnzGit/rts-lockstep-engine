// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Weapon.h
//     (WeaponReloadType, WeaponPrefireType, WeaponAntiMaskType, WeaponAffectsMaskType, WeaponCollideMaskType,
//      their name lists, HistoricWeaponDamageInfo, WeaponTemplate, WeaponStore)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Weapon.cpp
//     (the field parsers, TheWeaponTemplateFieldParseTable, WeaponTemplate's constructor, reset, parsers,
//      postProcessLoad and getters, and WeaponStore without its firing)
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INIWeapon.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Weapon.cpp: Steven Johnson, February 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `WeaponTemplate` (what a kind of weapon is: damage, ranges, rates, the effects it plays, what it can aim at),
//! `WeaponStore` (every template read from the `Weapon` INI blocks, in the order they were read) and the
//! `Weapon` INI block handler. This is the data half of `Weapon.h`/`Weapon.cpp`.
//!
//! # What is translated, and what is not (wave 7, slice A)
//!
//! Translated here: the template's data and its INI table (all 80 fields), the getters that fold in a
//! [`WeaponBonus`], `postProcessLoad`, the store (`findWeaponTemplate`, `newWeaponTemplate`, `reset`, the delayed
//! damage list, the block handler). ~~**Not yet translated** (they need `Object`, the partition manager, the
//! drawable and the damage application, and belong to the next slices): `fireWeaponTemplate`, `dealDamageInternal`
//! and with it `WeaponStore::update`, `estimateWeaponTemplateDamage`, `shouldProjectileCollideWith`,
//! `handleProjectileDetonation`, `createAndFireTempWeapon`, and the `Weapon` instance (ammo, reload, status,
//! `xfer`).~~ **CORRECTED 2026-10-20 (slice B, `p1w8-sonnet`)** — all of those are translated in
//! `logic::weapon_fire` (the `Weapon` instance and the firing half of the template and the store, over a
//! `WeaponHost`) and `logic::weapon_set_state` (the owned `WeaponSet`), and verified by
//! `tests/differential_weapon_fire.rs`.
//!
//! # Translation choices
//!
//! - **Pointers become names or ids.** `FXList*`, `ParticleSystemTemplate*` and `AudioEventRTS` are the client's;
//!   they stay **names** ([`FxName`]), exactly as `DamageFX` keeps its `FXList`s: the INI parsers store `NULL` for
//!   `None` (any case), and a name for anything else (the client's store would answer `NULL` for a name it does
//!   not know, which is not observable here). The fire sound is the name given to `setEventName`; the client
//!   fills the rest of the event from its audio tables. `m_projectileTmpl` is the thing template's id and the
//!   `ObjectCreationList*`s are ids, both found by the closures given to [`WeaponTemplate::post_process_load`]
//!   (the thing factory and the OCL store are other units); `m_historicBonusWeapon` is the template's name key.
//! - **`WeaponStore::m_weaponTemplateVector` is a `Vec`** in creation order, and a lookup is a linear search by
//!   name key, as upstream.
//! - `findWeaponTemplate` calls `nameToKey`, which **assigns a key to a name it has not seen**, and `None` (any
//!   case) returns before it does; so the lookups take the [`NameKeyGenerator`].
//! - `getDelayBetweenShots` draws from `GameLogicRandomValue` when the delay is a range: it takes the logic
//!   [`RandomValues`].
//!
//! # Upstream defects kept
//!
//! - **`newOverride` never puts the override in the store.** `parseWeaponTemplateDefinition` of a name that exists,
//!   when loading with `INI_LOAD_CREATE_OVERRIDES` (a map's `Weapon` block), copies the template, parses the block
//!   into the copy, and drops it: `m_weaponTemplateVector` is not changed (the other stores replace the entry).
//!   So a map cannot override a weapon, `WeaponStore::reset`'s override cleanup (`isOverride()`) never finds
//!   anything, and the copy is leaked upstream. The block is still parsed (its errors still throw, its name keys
//!   are still assigned). The translation does the same: [`WeaponStore::parse_weapon_template_definition`] parses
//!   into a copy and drops it. `m_nextTemplate` is therefore not translated: nothing in the store is ever linked.
//! - **The override copy shares the original's `WeaponBonusSet`.** `(*wt) = (*weaponTemplate)` copies the
//!   `m_extraBonus` pointer, not the set, so `WeaponTemplate::parseWeaponBonusSet` on the override (a `WeaponBonus =`
//!   line of the map's block) writes into the original's set whenever the original has one: that one field of an
//!   override block does take effect, by accident, and stays even when the block fails. Kept
//!   ([`WeaponStore::parse_weapon_template_definition`] writes the set back).
//! - **A `Weapon` block for a name that exists, when not loading overrides, returns without reading the block**
//!   (the `DEBUG_CRASH` is compiled out), so the loader then meets the block's field lines as unknown top-level
//!   tokens.
//! - **`DelayBetweenShots`** reads `stricmp(token, ...)` of a missing token (`NULL`): undefined behaviour,
//!   [`Undefined::NullTokenString`] here. It also stores `ceilf(...)` into an `Int` (the values it can reach fit).
//! - `getPreAttackDelay` returns an `Int` computed as `m_preAttackDelay * bonus` (a `Real`), truncating, which is
//!   undefined for a product outside `Int`: an error here.
//! - `WeaponStore::update` has `const isProjectileDetonation = false;` (an implicit `int`, MSVC only); translated in
//!   `weapon_fire::weapon_store_update` (the differential build adds the `Bool` it lacks, the only edit of the text).
//!
//! # Verified
//!
//! Against upstream's own `Weapon.cpp` (the template and store regions, cut out by `build.rs`, every cut asserted
//! to occur once) with the real `INI::load`, by `tests/differential_weapon.rs`.
//!
//! **Results (2026-10-18, synthetic INI, no game files):** 4,000 cases (829 loads ok, 4,060 blocks threw, 5,052
//! templates, 1,267 override files); INI exception text, `XferCRC`, all 80 fields of every template, name keys,
//! `postProcessLoad`, 10,097 getter queries, 30,321 finds, delayed/historic damage and `reset`: **0 mismatches**.
//! Excluded as undefined upstream and counted (Rust returns an error, the C++ is never given them): 307 loads
//! (`DelayBetweenShots` with a missing token) and 7 getter queries (pre-attack delay leaving `Int`).
//! **18 of 18 planted bugs caught.** The differential found two fidelity bugs in the first translation, both
//! fixed: `ProjectileObject = None` is not applied when the field parse threw, and an override copy shares the
//! original's extra `WeaponBonusSet` (the dropped copy's `WeaponBonus =` lines land on the original).

use std::collections::BTreeSet;

use petir::Real;

use crate::common::base_type::{Coord2D, PI};
use crate::common::game_common::{convert_duration_from_msecs_to_frames, VETERANCY_NAMES};
use crate::common::ini::{
    FieldParse, Ini, IniError, IniLoadType, IniResult, Undefined, UserData, SEPS_COLON,
};
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_NONE};
use crate::common::random_value::RandomValues;
use crate::logic::damage::{DamageType, DeathType};
use crate::logic::damage::{DamageTypeFlags, DAMAGE_EXPLOSION, DEATH_NAMES, DEATH_NORMAL};
use crate::logic::weapon_bonus::{
    WeaponBonus, WeaponBonusSet, FIELD_DAMAGE, FIELD_PRE_ATTACK, FIELD_RADIUS, FIELD_RANGE,
    FIELD_RATE_OF_FIRE,
};

/// `LEVEL_COUNT` as a `usize`.
pub const LEVELS: usize = 4;

/// `PATHFIND_CELL_SIZE_F` (`AIPathfind.h`).
pub const PATHFIND_CELL_SIZE_F: f32 = 10.0;

/// `NO_MAX_SHOTS_LIMIT`.
pub const NO_MAX_SHOTS_LIMIT: i32 = 0x7fff_ffff;

/// `enum WeaponReloadType`.
pub type WeaponReloadType = i32;
/// `AUTO_RELOAD`.
pub const AUTO_RELOAD: WeaponReloadType = 0;
/// `NO_RELOAD`.
pub const NO_RELOAD: WeaponReloadType = 1;
/// `RETURN_TO_BASE_TO_RELOAD`.
pub const RETURN_TO_BASE_TO_RELOAD: WeaponReloadType = 2;
/// `TheWeaponReloadNames`.
pub const WEAPON_RELOAD_NAMES: [&str; 3] = ["YES", "NO", "RETURN_TO_BASE"];

/// `enum WeaponPrefireType`.
pub type WeaponPrefireType = i32;
/// `PREFIRE_PER_SHOT`.
pub const PREFIRE_PER_SHOT: WeaponPrefireType = 0;
/// `PREFIRE_PER_ATTACK`.
pub const PREFIRE_PER_ATTACK: WeaponPrefireType = 1;
/// `PREFIRE_PER_CLIP`.
pub const PREFIRE_PER_CLIP: WeaponPrefireType = 2;
/// `TheWeaponPrefireNames`.
pub const WEAPON_PREFIRE_NAMES: [&str; 3] = ["PER_SHOT", "PER_ATTACK", "PER_CLIP"];

/// `WEAPON_ANTI_AIRBORNE_VEHICLE`.
pub const WEAPON_ANTI_AIRBORNE_VEHICLE: u32 = 0x01;
/// `WEAPON_ANTI_GROUND`.
pub const WEAPON_ANTI_GROUND: u32 = 0x02;
/// `WEAPON_ANTI_PROJECTILE`.
pub const WEAPON_ANTI_PROJECTILE: u32 = 0x04;
/// `WEAPON_ANTI_SMALL_MISSILE`: "all missiles are also projectiles (but not all projectiles are missiles)".
pub const WEAPON_ANTI_SMALL_MISSILE: u32 = 0x08;
/// `WEAPON_ANTI_MINE`.
pub const WEAPON_ANTI_MINE: u32 = 0x10;
/// `WEAPON_ANTI_AIRBORNE_INFANTRY`: "when set, anti-air weapons will not target infantry paratroopers".
pub const WEAPON_ANTI_AIRBORNE_INFANTRY: u32 = 0x20;
/// `WEAPON_ANTI_BALLISTIC_MISSILE`.
pub const WEAPON_ANTI_BALLISTIC_MISSILE: u32 = 0x40;
/// `WEAPON_ANTI_PARACHUTE`.
pub const WEAPON_ANTI_PARACHUTE: u32 = 0x80;

/// `WEAPON_AFFECTS_SELF`.
pub const WEAPON_AFFECTS_SELF: u32 = 0x01;
/// `WEAPON_AFFECTS_ALLIES`.
pub const WEAPON_AFFECTS_ALLIES: u32 = 0x02;
/// `WEAPON_AFFECTS_ENEMIES`.
pub const WEAPON_AFFECTS_ENEMIES: u32 = 0x04;
/// `WEAPON_AFFECTS_NEUTRALS`.
pub const WEAPON_AFFECTS_NEUTRALS: u32 = 0x08;
/// `WEAPON_KILLS_SELF`.
pub const WEAPON_KILLS_SELF: u32 = 0x10;
/// `WEAPON_DOESNT_AFFECT_SIMILAR`.
pub const WEAPON_DOESNT_AFFECT_SIMILAR: u32 = 0x20;
/// `WEAPON_DOESNT_AFFECT_AIRBORNE`.
pub const WEAPON_DOESNT_AFFECT_AIRBORNE: u32 = 0x40;
/// `TheWeaponAffectsMaskNames`.
pub const WEAPON_AFFECTS_MASK_NAMES: [&str; 7] = [
    "SELF",
    "ALLIES",
    "ENEMIES",
    "NEUTRALS",
    "SUICIDE",
    "NOT_SIMILAR",
    "NOT_AIRBORNE",
];

/// `WEAPON_COLLIDE_ALLIES`.
pub const WEAPON_COLLIDE_ALLIES: u32 = 0x0001;
/// `WEAPON_COLLIDE_ENEMIES`.
pub const WEAPON_COLLIDE_ENEMIES: u32 = 0x0002;
/// `WEAPON_COLLIDE_STRUCTURES`: "all structures EXCEPT for structures belonging to the projectile's controller".
pub const WEAPON_COLLIDE_STRUCTURES: u32 = 0x0004;
/// `WEAPON_COLLIDE_SHRUBBERY`.
pub const WEAPON_COLLIDE_SHRUBBERY: u32 = 0x0008;
/// `WEAPON_COLLIDE_PROJECTILE`.
pub const WEAPON_COLLIDE_PROJECTILE: u32 = 0x0010;
/// `WEAPON_COLLIDE_WALLS`.
pub const WEAPON_COLLIDE_WALLS: u32 = 0x0020;
/// `WEAPON_COLLIDE_SMALL_MISSILES`.
pub const WEAPON_COLLIDE_SMALL_MISSILES: u32 = 0x0040;
/// `WEAPON_COLLIDE_BALLISTIC_MISSILES`.
pub const WEAPON_COLLIDE_BALLISTIC_MISSILES: u32 = 0x0080;
/// `WEAPON_COLLIDE_CONTROLLED_STRUCTURES`: "only structures belonging to the projectile's controller".
pub const WEAPON_COLLIDE_CONTROLLED_STRUCTURES: u32 = 0x0100;
/// `TheWeaponCollideMaskNames`.
pub const WEAPON_COLLIDE_MASK_NAMES: [&str; 9] = [
    "ALLIES",
    "ENEMIES",
    "STRUCTURES",
    "SHRUBBERY",
    "PROJECTILES",
    "WALLS",
    "SMALL_MISSILES",
    "BALLISTIC_MISSILES",
    "CONTROLLED_STRUCTURES",
];

/// A client effect (`FXList*`, `ParticleSystemTemplate*`) kept as its name: `None` is upstream's `NULL`.
pub type FxName = Option<Vec<u8>>;

/// An input for which upstream is undefined behaviour (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponError {
    /// `getPreAttackDelay`: a `Real` outside `Int` converted to `Int`.
    FloatToIntOutOfRange,
    /// A null `Object*` is dereferenced (`logic::weapon_fire`).
    NullObject,
    /// `ThingFactory::newObject(NULL, ..)` throws `ERROR_BAD_ARG`: `newProjectileFired` on a projectile stream name
    /// that names no template.
    NullTemplate,
    /// A value upstream reads without ever writing it (an out-parameter whose function returned false).
    UninitialisedRead,
}

/// `struct HistoricWeaponDamageInfo`: when and where this weapon was fired.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HistoricWeaponDamageInfo {
    /// `frame`.
    pub frame: u32,
    /// `location`.
    pub location: crate::common::base_type::Coord3D,
}

/// `class WeaponTemplate`. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponTemplate {
    /// `m_name`.
    pub name: Vec<u8>,
    /// `m_nameKey`.
    pub name_key: NameKeyType,
    /// `m_projectileStreamName`.
    pub projectile_stream_name: Vec<u8>,
    /// `m_laserName`.
    pub laser_name: Vec<u8>,
    /// `m_laserBoneName`.
    pub laser_bone_name: Vec<u8>,
    /// `m_primaryDamage`.
    pub primary_damage: f32,
    /// `m_primaryDamageRadius`.
    pub primary_damage_radius: f32,
    /// `m_secondaryDamage`.
    pub secondary_damage: f32,
    /// `m_secondaryDamageRadius`.
    pub secondary_damage_radius: f32,
    /// `m_shockWaveAmount`.
    pub shock_wave_amount: f32,
    /// `m_shockWaveRadius`.
    pub shock_wave_radius: f32,
    /// `m_shockWaveTaperOff`.
    pub shock_wave_taper_off: f32,
    /// `m_attackRange`.
    pub attack_range: f32,
    /// `m_minimumAttackRange`.
    pub minimum_attack_range: f32,
    /// `m_requestAssistRange`.
    pub request_assist_range: f32,
    /// `m_aimDelta`.
    pub aim_delta: f32,
    /// `m_scatterRadius`.
    pub scatter_radius: f32,
    /// `m_scatterTargetScalar`.
    pub scatter_target_scalar: f32,
    /// `m_scatterTargets`.
    pub scatter_targets: Vec<Coord2D>,
    /// `m_damageType`.
    pub damage_type: DamageType,
    /// `m_deathType`.
    pub death_type: DeathType,
    /// `m_weaponSpeed`.
    pub weapon_speed: f32,
    /// `m_minWeaponSpeed`.
    pub min_weapon_speed: f32,
    /// `m_isScaleWeaponSpeed`.
    pub is_scale_weapon_speed: bool,
    /// `m_weaponRecoil`.
    pub weapon_recoil: f32,
    /// `m_minTargetPitch`.
    pub min_target_pitch: f32,
    /// `m_maxTargetPitch`.
    pub max_target_pitch: f32,
    /// `m_radiusDamageAngle`.
    pub radius_damage_angle: f32,
    /// `m_projectileName`.
    pub projectile_name: Vec<u8>,
    /// `m_projectileTmpl`: the id of the thing template, found by `postProcessLoad`.
    pub projectile_tmpl: Option<u16>,
    /// `m_fireOCLNames`.
    pub fire_ocl_names: [Vec<u8>; LEVELS],
    /// `m_projectileDetonationOCLNames`.
    pub projectile_detonation_ocl_names: [Vec<u8>; LEVELS],
    /// `m_projectileExhausts`.
    pub projectile_exhausts: [FxName; LEVELS],
    /// `m_fireOCLs`: found by `postProcessLoad`.
    pub fire_ocls: [Option<u32>; LEVELS],
    /// `m_projectileDetonationOCLs`: found by `postProcessLoad`.
    pub projectile_detonation_ocls: [Option<u32>; LEVELS],
    /// `m_fireFXs`.
    pub fire_fxs: [FxName; LEVELS],
    /// `m_projectileDetonateFXs`.
    pub projectile_detonate_fxs: [FxName; LEVELS],
    /// `m_fireSound`'s event name.
    pub fire_sound: Vec<u8>,
    /// `m_fireSoundLoopTime`.
    pub fire_sound_loop_time: u32,
    /// `m_extraBonus`.
    pub extra_bonus: Option<WeaponBonusSet>,
    /// `m_clipSize`.
    pub clip_size: i32,
    /// `m_clipReloadTime`.
    pub clip_reload_time: i32,
    /// `m_minDelayBetweenShots`.
    pub min_delay_between_shots: i32,
    /// `m_maxDelayBetweenShots`.
    pub max_delay_between_shots: i32,
    /// `m_continuousFireOneShotsNeeded`.
    pub continuous_fire_one_shots_needed: i32,
    /// `m_continuousFireTwoShotsNeeded`.
    pub continuous_fire_two_shots_needed: i32,
    /// `m_continuousFireCoastFrames`.
    pub continuous_fire_coast_frames: u32,
    /// `m_autoReloadWhenIdleFrames`.
    pub auto_reload_when_idle_frames: u32,
    /// `m_shotsPerBarrel`.
    pub shots_per_barrel: i32,
    /// `m_antiMask`.
    pub anti_mask: u32,
    /// `m_affectsMask`.
    pub affects_mask: u32,
    /// `m_collideMask`.
    pub collide_mask: u32,
    /// `m_damageDealtAtSelfPosition`.
    pub damage_dealt_at_self_position: bool,
    /// `m_reloadType`.
    pub reload_type: WeaponReloadType,
    /// `m_prefireType`.
    pub prefire_type: WeaponPrefireType,
    /// `m_historicBonusTime`.
    pub historic_bonus_time: u32,
    /// `m_historicBonusRadius`.
    pub historic_bonus_radius: f32,
    /// `m_historicBonusCount`.
    pub historic_bonus_count: i32,
    /// `m_historicBonusWeapon`: the name key of the template.
    pub historic_bonus_weapon: Option<NameKeyType>,
    /// `m_leechRangeWeapon`.
    pub leech_range_weapon: bool,
    /// `m_capableOfFollowingWaypoint`.
    pub capable_of_following_waypoint: bool,
    /// `m_isShowsAmmoPips`.
    pub is_shows_ammo_pips: bool,
    /// `m_allowAttackGarrisonedBldgs`.
    pub allow_attack_garrisoned_bldgs: bool,
    /// `m_playFXWhenStealthed`.
    pub play_fx_when_stealthed: bool,
    /// `m_preAttackDelay`.
    pub pre_attack_delay: i32,
    /// `m_continueAttackRange`.
    pub continue_attack_range: f32,
    /// `m_infantryInaccuracyDist`.
    pub infantry_inaccuracy_dist: f32,
    /// `m_damageStatusType`.
    pub damage_status_type: i32,
    /// `m_suspendFXDelay`.
    pub suspend_fx_delay: u32,
    /// `m_dieOnDetonate`.
    pub die_on_detonate: bool,
    /// `m_historicDamage`: the shots of the last `m_historicBonusTime` frames.
    pub historic_damage: Vec<HistoricWeaponDamageInfo>,
}

impl Default for WeaponTemplate {
    fn default() -> Self {
        WeaponTemplate::new()
    }
}

impl WeaponTemplate {
    /// `WeaponTemplate::WeaponTemplate()`.
    pub fn new() -> WeaponTemplate {
        WeaponTemplate {
            name: b"NoNameWeapon".to_vec(),
            name_key: NameKeyType::INVALID,
            projectile_stream_name: Vec::new(),
            laser_name: Vec::new(),
            laser_bone_name: Vec::new(),
            primary_damage: 0.0,
            primary_damage_radius: 0.0,
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            attack_range: 0.0,
            minimum_attack_range: 0.0,
            request_assist_range: 0.0,
            aim_delta: 0.0,
            scatter_radius: 0.0,
            scatter_target_scalar: 0.0,
            scatter_targets: Vec::new(),
            damage_type: DAMAGE_EXPLOSION,
            death_type: DEATH_NORMAL,
            weapon_speed: 999999.0, // effectively instant
            min_weapon_speed: 999999.0,
            is_scale_weapon_speed: false,
            weapon_recoil: 0.0, // no recoil
            min_target_pitch: -PI,
            max_target_pitch: PI,
            radius_damage_angle: PI, // PI each way, so full circle
            projectile_name: Vec::new(),
            projectile_tmpl: None,
            fire_ocl_names: Default::default(),
            projectile_detonation_ocl_names: Default::default(),
            projectile_exhausts: Default::default(),
            fire_ocls: [None; LEVELS],
            projectile_detonation_ocls: [None; LEVELS],
            fire_fxs: Default::default(),
            projectile_detonate_fxs: Default::default(),
            fire_sound: Vec::new(),
            fire_sound_loop_time: 0,
            extra_bonus: None,
            clip_size: 0,
            clip_reload_time: 0,
            min_delay_between_shots: 0,
            max_delay_between_shots: 0,
            continuous_fire_one_shots_needed: i32::MAX,
            continuous_fire_two_shots_needed: i32::MAX,
            continuous_fire_coast_frames: 0,
            auto_reload_when_idle_frames: 0,
            shots_per_barrel: 1,
            anti_mask: WEAPON_ANTI_GROUND, // but not air or projectile
            // most projectile weapons don't want to collide with nontargeted enemies/allies or trees...
            affects_mask: WEAPON_AFFECTS_ALLIES | WEAPON_AFFECTS_ENEMIES | WEAPON_AFFECTS_NEUTRALS,
            collide_mask: WEAPON_COLLIDE_STRUCTURES,
            damage_dealt_at_self_position: false,
            reload_type: AUTO_RELOAD,
            prefire_type: PREFIRE_PER_SHOT,
            historic_bonus_time: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_count: 0,
            historic_bonus_weapon: None,
            leech_range_weapon: false,
            capable_of_following_waypoint: false,
            is_shows_ammo_pips: false,
            allow_attack_garrisoned_bldgs: false,
            play_fx_when_stealthed: false,
            pre_attack_delay: 0,
            continue_attack_range: 0.0,
            infantry_inaccuracy_dist: 0.0,
            damage_status_type: OBJECT_STATUS_NONE,
            suspend_fx_delay: 0,
            die_on_detonate: false,
            historic_damage: Vec::new(),
        }
    }

    /// `reset()`: forget the shots remembered for the historic bonus.
    pub fn reset(&mut self) {
        self.historic_damage.clear();
    }

    /// `postProcessLoad()`: find the projectile's thing template and the OCLs by name, with the factory and the
    /// store given as lookups (`None` = not found, upstream's `NULL`); the OCL names are then cleared.
    pub fn post_process_load(
        &mut self,
        find_template: &impl Fn(&[u8]) -> Option<u16>,
        find_ocl: &impl Fn(&[u8]) -> Option<u32>,
    ) {
        self.projectile_tmpl = if self.projectile_name.is_empty() {
            None
        } else {
            find_template(&self.projectile_name)
        };
        for i in 0..LEVELS {
            // and the OCL if there is one
            self.fire_ocls[i] = if self.fire_ocl_names[i].is_empty() {
                None
            } else {
                find_ocl(&self.fire_ocl_names[i])
            };
            self.fire_ocl_names[i].clear();
            // and the other OCL if there is one
            self.projectile_detonation_ocls[i] =
                if self.projectile_detonation_ocl_names[i].is_empty() {
                    None
                } else {
                    find_ocl(&self.projectile_detonation_ocl_names[i])
                };
            self.projectile_detonation_ocl_names[i].clear();
        }
    }

    /// `getAttackRange(bonus)`. "Undersize by 1/4 of a pathfind cell, so that the goal is not teetering on the
    /// edge of firing range" (`RATIONALIZE_ATTACK_RANGE`).
    pub fn get_attack_range(&self, bonus: &WeaponBonus) -> f32 {
        let undersize = PATHFIND_CELL_SIZE_F * 0.25;
        let mut r = self.attack_range * bonus.get_field(FIELD_RANGE) - undersize;
        if r < 0.0 {
            r = 0.0;
        }
        r
    }

    /// `getMinimumAttackRange()`.
    pub fn get_minimum_attack_range(&self) -> f32 {
        let undersize = PATHFIND_CELL_SIZE_F * 0.25;
        let mut r = self.minimum_attack_range - undersize;
        if r < 0.0 {
            r = 0.0;
        }
        r
    }

    /// `getUnmodifiedAttackRange()`.
    pub fn get_unmodified_attack_range(&self) -> f32 {
        self.attack_range
    }

    /// `getDelayBetweenShots(bonus)`: "yes, divide, not multiply; the larger the rate-of-fire bonus, the shorter
    /// we want the delay time to be". A range draws from the logic random numbers.
    pub fn get_delay_between_shots(&self, bonus: &WeaponBonus, rng: &mut RandomValues) -> i32 {
        let delay_to_use = if self.min_delay_between_shots == self.max_delay_between_shots {
            self.min_delay_between_shots // random number thing doesn't like this case
        } else {
            rng.game_logic_random_value(self.min_delay_between_shots, self.max_delay_between_shots)
        };
        let bonus_rof = bonus.get_field(FIELD_RATE_OF_FIRE);
        crate::common::base_type::real_to_int_floor(delay_to_use as f32 / bonus_rof)
    }

    /// `getClipReloadTime(bonus)`.
    pub fn get_clip_reload_time(&self, bonus: &WeaponBonus) -> i32 {
        crate::common::base_type::real_to_int_floor(
            self.clip_reload_time as f32 / bonus.get_field(FIELD_RATE_OF_FIRE),
        )
    }

    /// `getPreAttackDelay(bonus)`: `m_preAttackDelay * bonus` truncated to an `Int`.
    pub fn get_pre_attack_delay(&self, bonus: &WeaponBonus) -> Result<i32, WeaponError> {
        let v = self.pre_attack_delay as f32 * bonus.get_field(FIELD_PRE_ATTACK);
        if v.is_nan() || v >= 2147483648.0 || v <= -2147483649.0 {
            return Err(WeaponError::FloatToIntOutOfRange);
        }
        Ok(v as i32)
    }

    /// `getPrimaryDamage(bonus)`.
    pub fn get_primary_damage(&self, bonus: &WeaponBonus) -> f32 {
        self.primary_damage * bonus.get_field(FIELD_DAMAGE)
    }

    /// `getPrimaryDamageRadius(bonus)`.
    pub fn get_primary_damage_radius(&self, bonus: &WeaponBonus) -> f32 {
        self.primary_damage_radius * bonus.get_field(FIELD_RADIUS)
    }

    /// `getSecondaryDamage(bonus)`.
    pub fn get_secondary_damage(&self, bonus: &WeaponBonus) -> f32 {
        self.secondary_damage * bonus.get_field(FIELD_DAMAGE)
    }

    /// `getSecondaryDamageRadius(bonus)`.
    pub fn get_secondary_damage_radius(&self, bonus: &WeaponBonus) -> f32 {
        self.secondary_damage_radius * bonus.get_field(FIELD_RADIUS)
    }

    /// `isContactWeapon()`: a range under one pathfind cell.
    pub fn is_contact_weapon(&self) -> bool {
        let undersize = PATHFIND_CELL_SIZE_F * 0.25;
        (self.attack_range - undersize) < PATHFIND_CELL_SIZE_F
    }

    /// `getAutoReloadsClip()`.
    pub fn get_auto_reloads_clip(&self) -> bool {
        self.reload_type == AUTO_RELOAD
    }
}

/// What the fields of a `Weapon` block fill.
struct Fill<'a> {
    t: &'a mut WeaponTemplate,
    names: &'a mut NameKeyGenerator,
    /// The name keys of the templates in the store (including the one being filled).
    keys: &'a BTreeSet<NameKeyType>,
}

/// `INI::parseFXList`, as far as the token goes: `None` (any case) is `NULL`.
fn fx_name(ini: &mut Ini<'_>) -> IniResult<FxName> {
    let token = ini.get_next_token(None)?;
    Ok(if token.eq_ignore_ascii_case(b"None") {
        None
    } else {
        Some(token)
    })
}

/// `INI::scanIndexList(ini->getNextToken(), TheVeterancyNames)`.
fn vet_level(ini: &mut Ini<'_>) -> IniResult<usize> {
    let token = ini.get_next_token(None)?;
    Ok(Ini::scan_index_list(&token, &VETERANCY_NAMES)? as usize)
}

macro_rules! real_field {
    ($fname:ident, $field:ident, $parse:ident) => {
        fn $fname(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
            w.t.$field = ini.$parse()?;
            Ok(())
        }
    };
}

real_field!(f_primary_damage, primary_damage, parse_real);
real_field!(f_primary_damage_radius, primary_damage_radius, parse_real);
real_field!(f_secondary_damage, secondary_damage, parse_real);
real_field!(
    f_secondary_damage_radius,
    secondary_damage_radius,
    parse_real
);
real_field!(f_shock_wave_amount, shock_wave_amount, parse_real);
real_field!(f_shock_wave_radius, shock_wave_radius, parse_real);
real_field!(f_shock_wave_taper_off, shock_wave_taper_off, parse_real);
real_field!(f_attack_range, attack_range, parse_real);
real_field!(f_minimum_attack_range, minimum_attack_range, parse_real);
real_field!(f_request_assist_range, request_assist_range, parse_real);
real_field!(f_aim_delta, aim_delta, parse_angle_real);
real_field!(f_scatter_radius, scatter_radius, parse_real);
real_field!(f_scatter_target_scalar, scatter_target_scalar, parse_real);
real_field!(f_infantry_inaccuracy, infantry_inaccuracy_dist, parse_real);
real_field!(f_weapon_speed, weapon_speed, parse_velocity_real);
real_field!(f_min_weapon_speed, min_weapon_speed, parse_velocity_real);
real_field!(f_weapon_recoil, weapon_recoil, parse_angle_real);
real_field!(f_min_target_pitch, min_target_pitch, parse_angle_real);
real_field!(f_max_target_pitch, max_target_pitch, parse_angle_real);
real_field!(f_radius_damage_angle, radius_damage_angle, parse_angle_real);
real_field!(f_historic_bonus_radius, historic_bonus_radius, parse_real);
real_field!(f_continue_attack_range, continue_attack_range, parse_real);

macro_rules! bool_field {
    ($fname:ident, $field:ident) => {
        fn $fname(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
            w.t.$field = ini.parse_bool()?;
            Ok(())
        }
    };
}

bool_field!(f_scale_weapon_speed, is_scale_weapon_speed);
bool_field!(f_damage_dealt_at_self, damage_dealt_at_self_position);
bool_field!(f_leech_range, leech_range_weapon);
bool_field!(f_follow_waypoints, capable_of_following_waypoint);
bool_field!(f_ammo_pips, is_shows_ammo_pips);
bool_field!(f_attack_garrisoned, allow_attack_garrisoned_bldgs);
bool_field!(f_fx_when_stealthed, play_fx_when_stealthed);
bool_field!(f_die_on_detonate, die_on_detonate);

macro_rules! int_field {
    ($fname:ident, $field:ident) => {
        fn $fname(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
            w.t.$field = ini.parse_int()?;
            Ok(())
        }
    };
}

int_field!(f_clip_size, clip_size);
int_field!(f_continuous_one, continuous_fire_one_shots_needed);
int_field!(f_continuous_two, continuous_fire_two_shots_needed);
int_field!(f_shots_per_barrel, shots_per_barrel);
int_field!(f_historic_bonus_count, historic_bonus_count);

macro_rules! duration_u32_field {
    ($fname:ident, $field:ident) => {
        fn $fname(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
            w.t.$field = ini.parse_duration_unsigned_int()?;
            Ok(())
        }
    };
}

duration_u32_field!(f_fire_sound_loop_time, fire_sound_loop_time);
duration_u32_field!(f_continuous_coast, continuous_fire_coast_frames);
duration_u32_field!(f_auto_reload_idle, auto_reload_when_idle_frames);
duration_u32_field!(f_historic_bonus_time, historic_bonus_time);
duration_u32_field!(f_suspend_fx_delay, suspend_fx_delay);

/// `ClipReloadTime`, `PreAttackDelay`: `parseDurationUnsignedInt` into an `Int` (the same four bytes).
fn f_clip_reload_time(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    w.t.clip_reload_time = ini.parse_duration_unsigned_int()? as i32;
    Ok(())
}

fn f_pre_attack_delay(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    w.t.pre_attack_delay = ini.parse_duration_unsigned_int()? as i32;
    Ok(())
}

/// `DamageTypeFlags::parseSingleBitFromINI`.
fn f_damage_type(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    w.t.damage_type = DamageTypeFlags::parse_single_bit_from_ini(ini)?;
    Ok(())
}

/// `ObjectStatusMaskType::parseSingleBitFromINI`.
fn f_damage_status_type(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    w.t.damage_status_type = ObjectStatusMaskType::parse_single_bit_from_ini(ini)?;
    Ok(())
}

/// `INI::parseIndexList` with `TheDeathNames`.
fn f_death_type(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    w.t.death_type = ini.parse_index_list(&DEATH_NAMES)?;
    Ok(())
}

fn f_projectile_object(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    w.t.projectile_name = ini.parse_ascii_string()?;
    Ok(())
}

/// `INI::parseAudioEventRTS`: `NoSound` leaves the event alone, any other name is the event's.
fn f_fire_sound(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    let token = ini.get_next_token(None)?;
    if !token.eq_ignore_ascii_case(b"NoSound") {
        w.t.fire_sound = token;
    }
    Ok(())
}

fn f_fire_fx_all(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    let fx = fx_name(ini)?;
    w.t.fire_fxs = std::array::from_fn(|_| fx.clone());
    Ok(())
}

fn f_detonate_fx_all(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    let fx = fx_name(ini)?;
    w.t.projectile_detonate_fxs = std::array::from_fn(|_| fx.clone());
    Ok(())
}

fn f_fire_ocl_all(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    let a = ini.get_next_ascii_string();
    w.t.fire_ocl_names = std::array::from_fn(|_| a.clone());
    Ok(())
}

fn f_detonate_ocl_all(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    let a = ini.get_next_ascii_string();
    w.t.projectile_detonation_ocl_names = std::array::from_fn(|_| a.clone());
    Ok(())
}

fn f_exhaust_all(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    let pst = fx_name(ini)?;
    w.t.projectile_exhausts = std::array::from_fn(|_| pst.clone());
    Ok(())
}

fn f_fire_fx_vet(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    let v = vet_level(ini)?;
    w.t.fire_fxs[v] = fx_name(ini)?;
    Ok(())
}

fn f_detonate_fx_vet(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    let v = vet_level(ini)?;
    w.t.projectile_detonate_fxs[v] = fx_name(ini)?;
    Ok(())
}

fn f_fire_ocl_vet(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    let v = vet_level(ini)?;
    w.t.fire_ocl_names[v] = ini.get_next_ascii_string();
    Ok(())
}

fn f_detonate_ocl_vet(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    let v = vet_level(ini)?;
    w.t.projectile_detonation_ocl_names[v] = ini.get_next_ascii_string();
    Ok(())
}

fn f_exhaust_vet(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    let v = vet_level(ini)?;
    w.t.projectile_exhausts[v] = fx_name(ini)?;
    Ok(())
}

/// `DelayBetweenShots`: `WeaponTemplate::parseShotDelay`. A single number, or `Min:n` and optionally `Max:n`.
fn f_shot_delay(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    let token = ini.get_next_token_or_null(Some(SEPS_COLON));
    // `stricmp(token, ...)` of a missing token is a read through NULL
    let Some(token) = token else {
        return Err(IniError::undefined(Undefined::NullTokenString));
    };
    let (min, max);
    if token.eq_ignore_ascii_case(b"Min") {
        // two entry min/max
        min = Ini::scan_int(&ini.get_next_token(Some(SEPS_COLON))?)?;
        let token = ini.get_next_token_or_null(Some(SEPS_COLON));
        let Some(token) = token else {
            return Err(IniError::undefined(Undefined::NullTokenString));
        };
        if !token.eq_ignore_ascii_case(b"Max") {
            // messed up double entry
            max = min;
        } else {
            max = Ini::scan_int(&ini.get_next_token(Some(SEPS_COLON))?)?;
        }
    } else {
        // single entry, as in no label so the first token is just a number
        min = Ini::scan_int(&token)?;
        max = min;
    }
    // no matter what we have now, we want to convert it to frames from msec
    // `m_minDelayBetweenShots = ceilf(...)`: a float stored into an `Int` (the values it can reach fit)
    w.t.min_delay_between_shots =
        Real::ceil(convert_duration_from_msecs_to_frames(min as f32)) as i32;
    w.t.max_delay_between_shots =
        Real::ceil(convert_duration_from_msecs_to_frames(max as f32)) as i32;
    Ok(())
}

fn f_radius_damage_affects(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    ini.parse_bit_string32(&mut w.t.affects_mask, &WEAPON_AFFECTS_MASK_NAMES)
}

fn f_projectile_collides_with(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    ini.parse_bit_string32(&mut w.t.collide_mask, &WEAPON_COLLIDE_MASK_NAMES)
}

/// `INI::parseBitInInt32` with the mask of the entry.
fn f_anti(ini: &mut Ini<'_>, w: &mut Fill<'_>, user: &UserData) -> IniResult<()> {
    let mask = match user {
        UserData::Mask(m) => *m,
        _ => 0,
    };
    ini.parse_bit_in_int32(&mut w.t.anti_mask, mask)
}

fn f_auto_reloads_clip(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    w.t.reload_type = ini.parse_index_list(&WEAPON_RELOAD_NAMES)?;
    Ok(())
}

fn f_prefire_type(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    w.t.prefire_type = ini.parse_index_list(&WEAPON_PREFIRE_NAMES)?;
    Ok(())
}

fn f_projectile_stream_name(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    w.t.projectile_stream_name = ini.parse_ascii_string()?;
    Ok(())
}

fn f_laser_name(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    w.t.laser_name = ini.parse_ascii_string()?;
    Ok(())
}

fn f_laser_bone_name(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    w.t.laser_bone_name = ini.parse_ascii_string()?;
    Ok(())
}

/// `WeaponTemplate::parseWeaponBonusSet`: makes the extra bonus on first use.
fn f_weapon_bonus(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    let set = w.t.extra_bonus.get_or_insert_with(WeaponBonusSet::new);
    set.parse_weapon_bonus_set(ini)
}

/// `INI::parseWeaponTemplate`: `findWeaponTemplate` (`None` is `NULL` without touching the name keys; a
/// name the store does not have is `NULL`).
fn f_historic_bonus_weapon(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    let token = ini.get_next_token(None)?;
    w.t.historic_bonus_weapon = if token.eq_ignore_ascii_case(b"None") {
        None
    } else {
        let key = w
            .names
            .try_name_to_key(&token)
            .map_err(|_| IniError::undefined(Undefined::NameKeysExhausted))?;
        w.keys.contains(&key).then_some(key)
    };
    Ok(())
}

/// `WeaponTemplate::parseScatterTarget`: "accept multiple listings of Coord2D's".
fn f_scatter_target(ini: &mut Ini<'_>, w: &mut Fill<'_>, _: &UserData) -> IniResult<()> {
    let target = ini.parse_coord2d()?;
    w.t.scatter_targets.push(target);
    Ok(())
}

/// `TheWeaponTemplateFieldParseTable`. A table of function pointers over `Fill`, which has a lifetime, cannot be
// a `static`; it is built once per block.
fn field_table<'a>() -> Vec<FieldParse<Fill<'a>>> {
    let e = FieldParse::<Fill<'a>>::new;
    vec![
        e("PrimaryDamage", f_primary_damage, UserData::None),
        e(
            "PrimaryDamageRadius",
            f_primary_damage_radius,
            UserData::None,
        ),
        e("SecondaryDamage", f_secondary_damage, UserData::None),
        e(
            "SecondaryDamageRadius",
            f_secondary_damage_radius,
            UserData::None,
        ),
        e("ShockWaveAmount", f_shock_wave_amount, UserData::None),
        e("ShockWaveRadius", f_shock_wave_radius, UserData::None),
        e("ShockWaveTaperOff", f_shock_wave_taper_off, UserData::None),
        e("AttackRange", f_attack_range, UserData::None),
        e("MinimumAttackRange", f_minimum_attack_range, UserData::None),
        e("RequestAssistRange", f_request_assist_range, UserData::None),
        e("AcceptableAimDelta", f_aim_delta, UserData::None),
        e("ScatterRadius", f_scatter_radius, UserData::None),
        e(
            "ScatterTargetScalar",
            f_scatter_target_scalar,
            UserData::None,
        ),
        e(
            "ScatterRadiusVsInfantry",
            f_infantry_inaccuracy,
            UserData::None,
        ),
        e("DamageType", f_damage_type, UserData::None),
        e("DamageStatusType", f_damage_status_type, UserData::None),
        e("DeathType", f_death_type, UserData::None),
        e("WeaponSpeed", f_weapon_speed, UserData::None),
        e("MinWeaponSpeed", f_min_weapon_speed, UserData::None),
        e("ScaleWeaponSpeed", f_scale_weapon_speed, UserData::None),
        e("WeaponRecoil", f_weapon_recoil, UserData::None),
        e("MinTargetPitch", f_min_target_pitch, UserData::None),
        e("MaxTargetPitch", f_max_target_pitch, UserData::None),
        e("RadiusDamageAngle", f_radius_damage_angle, UserData::None),
        e("ProjectileObject", f_projectile_object, UserData::None),
        e("FireSound", f_fire_sound, UserData::None),
        e("FireSoundLoopTime", f_fire_sound_loop_time, UserData::None),
        e("FireFX", f_fire_fx_all, UserData::None),
        e("ProjectileDetonationFX", f_detonate_fx_all, UserData::None),
        e("FireOCL", f_fire_ocl_all, UserData::None),
        e(
            "ProjectileDetonationOCL",
            f_detonate_ocl_all,
            UserData::None,
        ),
        e("ProjectileExhaust", f_exhaust_all, UserData::None),
        e("VeterancyFireFX", f_fire_fx_vet, UserData::None),
        e(
            "VeterancyProjectileDetonationFX",
            f_detonate_fx_vet,
            UserData::None,
        ),
        e("VeterancyFireOCL", f_fire_ocl_vet, UserData::None),
        e(
            "VeterancyProjectileDetonationOCL",
            f_detonate_ocl_vet,
            UserData::None,
        ),
        e("VeterancyProjectileExhaust", f_exhaust_vet, UserData::None),
        e("ClipSize", f_clip_size, UserData::None),
        e("ContinuousFireOne", f_continuous_one, UserData::None),
        e("ContinuousFireTwo", f_continuous_two, UserData::None),
        e("ContinuousFireCoast", f_continuous_coast, UserData::None),
        e("AutoReloadWhenIdle", f_auto_reload_idle, UserData::None),
        e("ClipReloadTime", f_clip_reload_time, UserData::None),
        e("DelayBetweenShots", f_shot_delay, UserData::None),
        e("ShotsPerBarrel", f_shots_per_barrel, UserData::None),
        e(
            "DamageDealtAtSelfPosition",
            f_damage_dealt_at_self,
            UserData::None,
        ),
        e(
            "RadiusDamageAffects",
            f_radius_damage_affects,
            UserData::None,
        ),
        e(
            "ProjectileCollidesWith",
            f_projectile_collides_with,
            UserData::None,
        ),
        e(
            "AntiAirborneVehicle",
            f_anti,
            UserData::Mask(WEAPON_ANTI_AIRBORNE_VEHICLE),
        ),
        e("AntiGround", f_anti, UserData::Mask(WEAPON_ANTI_GROUND)),
        e(
            "AntiProjectile",
            f_anti,
            UserData::Mask(WEAPON_ANTI_PROJECTILE),
        ),
        e(
            "AntiSmallMissile",
            f_anti,
            UserData::Mask(WEAPON_ANTI_SMALL_MISSILE),
        ),
        e("AntiMine", f_anti, UserData::Mask(WEAPON_ANTI_MINE)),
        e(
            "AntiParachute",
            f_anti,
            UserData::Mask(WEAPON_ANTI_PARACHUTE),
        ),
        e(
            "AntiAirborneInfantry",
            f_anti,
            UserData::Mask(WEAPON_ANTI_AIRBORNE_INFANTRY),
        ),
        e(
            "AntiBallisticMissile",
            f_anti,
            UserData::Mask(WEAPON_ANTI_BALLISTIC_MISSILE),
        ),
        e("AutoReloadsClip", f_auto_reloads_clip, UserData::None),
        e(
            "ProjectileStreamName",
            f_projectile_stream_name,
            UserData::None,
        ),
        e("LaserName", f_laser_name, UserData::None),
        e("LaserBoneName", f_laser_bone_name, UserData::None),
        e("WeaponBonus", f_weapon_bonus, UserData::None),
        e("HistoricBonusTime", f_historic_bonus_time, UserData::None),
        e(
            "HistoricBonusRadius",
            f_historic_bonus_radius,
            UserData::None,
        ),
        e("HistoricBonusCount", f_historic_bonus_count, UserData::None),
        e(
            "HistoricBonusWeapon",
            f_historic_bonus_weapon,
            UserData::None,
        ),
        e("LeechRangeWeapon", f_leech_range, UserData::None),
        e("ScatterTarget", f_scatter_target, UserData::None),
        e(
            "CapableOfFollowingWaypoints",
            f_follow_waypoints,
            UserData::None,
        ),
        e("ShowsAmmoPips", f_ammo_pips, UserData::None),
        e(
            "AllowAttackGarrisonedBldgs",
            f_attack_garrisoned,
            UserData::None,
        ),
        e("PlayFXWhenStealthed", f_fx_when_stealthed, UserData::None),
        e("PreAttackDelay", f_pre_attack_delay, UserData::None),
        e("PreAttackType", f_prefire_type, UserData::None),
        e(
            "ContinueAttackRange",
            f_continue_attack_range,
            UserData::None,
        ),
        e("SuspendFXDelay", f_suspend_fx_delay, UserData::None),
        e("MissileCallsOnDie", f_die_on_detonate, UserData::None),
        FieldParse::end(),
    ]
}

/// `struct WeaponDelayedDamageInfo`: damage to deal on a later frame.
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponDelayedDamageInfo {
    /// `m_delayedWeapon`: the template's name key.
    pub delayed_weapon: NameKeyType,
    /// `m_delayDamagePos`.
    pub delay_damage_pos: crate::common::base_type::Coord3D,
    /// `m_delayDamageFrame`.
    pub delay_damage_frame: u32,
    /// `m_delaySourceID`.
    pub delay_source_id: i32,
    /// `m_delayIntendedVictimID`.
    pub delay_intended_victim_id: i32,
    /// `m_bonus`.
    pub bonus: WeaponBonus,
}

/// `class WeaponStore` (`TheWeaponStore`): every `WeaponTemplate`, in creation order. See the module docs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeaponStore {
    /// `m_weaponTemplateVector`.
    templates: Vec<WeaponTemplate>,
    /// `m_weaponDDI`.
    delayed_damage: Vec<WeaponDelayedDamageInfo>,
}

impl WeaponStore {
    /// `WeaponStore::WeaponStore()`.
    pub fn new() -> WeaponStore {
        WeaponStore::default()
    }

    /// The templates, in creation order.
    pub fn templates(&self) -> &[WeaponTemplate] {
        &self.templates
    }

    /// The templates, for a caller that changes one (the shots remembered for the historic bonus).
    pub fn templates_mut(&mut self) -> &mut [WeaponTemplate] {
        &mut self.templates
    }

    /// `m_weaponDDI`, in order.
    pub fn delayed_damage(&self) -> &[WeaponDelayedDamageInfo] {
        &self.delayed_damage
    }

    /// `findWeaponTemplate(name)`: `None` for `None` (any case) without touching the name keys; otherwise
    /// `nameToKey(name)` and the template under it, if any.
    pub fn find_weapon_template(
        &self,
        names: &mut NameKeyGenerator,
        name: &[u8],
    ) -> IniResult<Option<&WeaponTemplate>> {
        if name.eq_ignore_ascii_case(b"None") {
            return Ok(None);
        }
        let key = names
            .try_name_to_key(name)
            .map_err(|_| IniError::undefined(Undefined::NameKeysExhausted))?;
        Ok(self.find_weapon_template_private(key))
    }

    /// `findWeaponTemplatePrivate(key)` / `findWeaponTemplateByNameKey`: a linear search by name key.
    pub fn find_weapon_template_private(&self, key: NameKeyType) -> Option<&WeaponTemplate> {
        self.templates.iter().find(|t| t.name_key == key)
    }

    /// `newWeaponTemplate(name)`: `None` for an empty name; otherwise a new template, last in the store.
    pub fn new_weapon_template(
        &mut self,
        names: &mut NameKeyGenerator,
        name: &[u8],
    ) -> IniResult<Option<usize>> {
        // sanity
        if name.is_empty() {
            return Ok(None);
        }
        let mut wt = WeaponTemplate::new();
        wt.name = name.to_vec();
        wt.name_key = names
            .try_name_to_key(name)
            .map_err(|_| IniError::undefined(Undefined::NameKeysExhausted))?;
        self.templates.push(wt);
        Ok(Some(self.templates.len() - 1))
    }

    /// `m_weaponDDI.erase(it)`: drop the delayed damage at position `index` (`WeaponStore::update`).
    pub fn remove_delayed_damage(&mut self, index: usize) {
        if index < self.delayed_damage.len() {
            self.delayed_damage.remove(index);
        }
    }

    /// `deleteAllDelayedDamage()`.
    pub fn delete_all_delayed_damage(&mut self) {
        self.delayed_damage.clear();
    }

    /// `resetWeaponTemplates()`.
    pub fn reset_weapon_templates(&mut self) {
        for wt in &mut self.templates {
            wt.reset();
        }
    }

    /// `reset()`: upstream first cleans up overrides (`isOverride()`), of which the store never holds one (see
    /// the module docs); then the delayed damage and the historic damage are cleared.
    pub fn reset(&mut self) {
        self.delete_all_delayed_damage();
        self.reset_weapon_templates();
    }

    /// `setDelayedDamage(weapon, pos, whichFrame, sourceID, victimID, bonus)`.
    pub fn set_delayed_damage(
        &mut self,
        weapon: NameKeyType,
        pos: &crate::common::base_type::Coord3D,
        which_frame: u32,
        source_id: i32,
        victim_id: i32,
        bonus: &WeaponBonus,
    ) {
        self.delayed_damage.push(WeaponDelayedDamageInfo {
            delayed_weapon: weapon,
            delay_damage_pos: *pos,
            delay_damage_frame: which_frame,
            delay_source_id: source_id,
            delay_intended_victim_id: victim_id,
            bonus: *bonus,
        });
    }

    /// `postProcessLoad()`: every template finds its projectile and OCLs.
    pub fn post_process_load(
        &mut self,
        find_template: &impl Fn(&[u8]) -> Option<u16>,
        find_ocl: &impl Fn(&[u8]) -> Option<u32>,
    ) {
        for wt in &mut self.templates {
            wt.post_process_load(find_template, find_ocl);
        }
    }

    /// `WeaponStore::parseWeaponTemplateDefinition(ini)` (the `Weapon` block, reached through
    /// `INI::parseWeaponTemplateDefinition`): the weapon's name, then its fields until `End`. A known name, when
    /// loading overrides, is parsed into a copy that is dropped (see the module docs); a known name otherwise
    /// returns without reading the block.
    pub fn parse_weapon_template_definition(
        &mut self,
        ini: &mut Ini<'_>,
        names: &mut NameKeyGenerator,
    ) -> IniResult<()> {
        // read the weapon name
        let name = ini.get_next_token(None)?;
        // find existing item if present
        let key = names
            .try_name_to_key(&name)
            .map_err(|_| IniError::undefined(Undefined::NameKeysExhausted))?;
        let existing = self.templates.iter().position(|t| t.name_key == key);
        let keys: BTreeSet<NameKeyType>;
        // `scratch` is the template being filled; `stored` is where it goes when the block ends (a new template),
        // `shared_with` the stored template whose bonus set an override copy shares
        let (mut scratch, stored, shared_with);
        match existing {
            Some(i) => {
                if ini.get_load_type() != IniLoadType::CreateOverrides {
                    // "Weapon '%s' already exists, but OVERRIDE not specified" (compiled out)
                    return Ok(());
                }
                // `newOverride`: `(*wt) = (*weaponTemplate)`, a copy that never joins the store
                keys = self.templates.iter().map(|t| t.name_key).collect();
                scratch = self.templates[i].clone();
                stored = None;
                shared_with = self.templates[i].extra_bonus.is_some().then_some(i);
            }
            None => {
                let Some(i) = self.new_weapon_template(names, &name)? else {
                    return Ok(());
                };
                keys = self.templates.iter().map(|t| t.name_key).collect();
                scratch = std::mem::take(&mut self.templates[i]);
                stored = Some(i);
                shared_with = None;
            }
        }
        // parse the ini weapon definition
        let table = field_table();
        let r = {
            let mut fill = Fill {
                t: &mut scratch,
                names,
                keys: &keys,
            };
            ini.init_from_ini(&mut fill, &table)
        };
        // `if (weapon->m_projectileName.isNone()) weapon->m_projectileName.clear();`: after `initFromINI`, which
        // a failing field leaves by throwing, so a block that fails keeps the name `None` as read
        if r.is_ok() && scratch.projectile_name.eq_ignore_ascii_case(b"None") {
            scratch.projectile_name.clear();
        }
        if let Some(i) = shared_with {
            // the copy's `m_extraBonus` is the original's pointer: a `WeaponBonus =` line of the override block
            // changed the original's set in place (see the module docs)
            self.templates[i].extra_bonus = scratch.extra_bonus;
        }
        if let Some(i) = stored {
            self.templates[i] = scratch;
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniErrorKind};

    struct Ctx {
        store: WeaponStore,
        names: NameKeyGenerator,
    }

    fn block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.store.parse_weapon_template_definition(ini, &mut c.names)
    }

    static BLOCKS: [BlockParse<Ctx>; 1] = [BlockParse {
        token: "Weapon",
        parse: block,
    }];

    fn ctx() -> Ctx {
        let mut c = Ctx {
            store: WeaponStore::new(),
            names: NameKeyGenerator::new(),
        };
        c.names.init();
        c
    }

    fn load(c: &mut Ctx, text: &[u8], t: IniLoadType) -> IniResult<()> {
        Ini::load(b"w.ini", text, t, None, &BLOCKS, c)
    }

    #[test]
    fn a_weapon_block_fills_the_template_and_converts_durations_and_speeds() {
        let mut c = ctx();
        load(
            &mut c,
            b"Weapon Rifle\n PrimaryDamage = 12.5\n AttackRange = 100\n WeaponSpeed = 300\n DelayBetweenShots = Min:100 Max:200\n ClipSize = 5\n ClipReloadTime = 1000\n \
              AutoReloadsClip = RETURN_TO_BASE\n AntiGround = No\n AntiAirborneVehicle = Yes\n DamageType = FLAME\n DeathType = BURNED\n ProjectileObject = None\n FireSound = RifleFire\n \
              FireFX = FX_A\n VeterancyFireFX = ELITE FX_B\n FireOCL = OCL_X\n RadiusDamageAffects = SELF ENEMIES\n ScatterTarget = X:1 Y:2\nEnd\n",
            IniLoadType::Overwrite,
        )
        .unwrap();
        let t = &c.store.templates()[0];
        assert_eq!(t.name, b"Rifle");
        assert_eq!(t.primary_damage, 12.5);
        assert!(
            (t.weapon_speed - 10.0).abs() < 1e-5,
            "per second to per frame"
        );
        assert_eq!(
            (t.min_delay_between_shots, t.max_delay_between_shots),
            (3, 6),
            "milliseconds to frames, rounded up"
        );
        assert_eq!(t.clip_reload_time, 30);
        assert_eq!(t.reload_type, RETURN_TO_BASE_TO_RELOAD);
        assert_eq!(t.anti_mask, WEAPON_ANTI_AIRBORNE_VEHICLE);
        assert_eq!(t.affects_mask, WEAPON_AFFECTS_SELF | WEAPON_AFFECTS_ENEMIES);
        assert_eq!(t.damage_type, crate::logic::damage::DAMAGE_FLAME);
        assert_eq!(t.fire_sound, b"RifleFire");
        assert_eq!(t.fire_fxs[0], Some(b"FX_A".to_vec()));
        assert_eq!(t.fire_fxs[2], Some(b"FX_B".to_vec()));
        assert!(
            t.projectile_name.is_empty(),
            "None is cleared after the block"
        );
        assert_eq!(t.scatter_targets, vec![Coord2D { x: 1.0, y: 2.0 }]);
        assert!(t.extra_bonus.is_none());
        assert!(!t.get_auto_reloads_clip());
    }

    #[test]
    fn getters_fold_in_the_bonus_and_post_process_resolves_names() {
        let mut c = ctx();
        load(&mut c, b"Weapon W\n AttackRange = 20\n MinimumAttackRange = 5\n PrimaryDamage = 10\n PrimaryDamageRadius = 4\n SecondaryDamage = 2\n SecondaryDamageRadius = 3\n PreAttackDelay = 1000\n ClipReloadTime = 3000\n DelayBetweenShots = 300\n ProjectileObject = ProjA\n FireOCL = OCL_F\n VeterancyProjectileDetonationOCL = HEROIC OCL_D\nEnd\n", IniLoadType::Overwrite).unwrap();
        let mut b = WeaponBonus::new();
        b.set_field(FIELD_RANGE, 2.0);
        b.set_field(FIELD_RATE_OF_FIRE, 2.0);
        b.set_field(FIELD_DAMAGE, 3.0);
        b.set_field(FIELD_RADIUS, 0.5);
        b.set_field(FIELD_PRE_ATTACK, 0.5);
        {
            let t = &c.store.templates()[0];
            assert_eq!(t.get_attack_range(&b), 20.0 * 2.0 - 2.5);
            assert_eq!(t.get_minimum_attack_range(), 2.5);
            assert_eq!(t.get_unmodified_attack_range(), 20.0);
            let mut rng = RandomValues::new(1);
            assert_eq!(t.get_delay_between_shots(&b, &mut rng), 4, "9 frames / 2");
            assert_eq!(t.get_clip_reload_time(&b), 45);
            assert_eq!(t.get_pre_attack_delay(&b), Ok(15));
            assert_eq!(t.get_primary_damage(&b), 30.0);
            assert_eq!(t.get_primary_damage_radius(&b), 2.0);
            assert_eq!(t.get_secondary_damage(&b), 6.0);
            assert_eq!(t.get_secondary_damage_radius(&b), 1.5);
            assert!(!t.is_contact_weapon());
        }
        let find_t = |n: &[u8]| (n == b"ProjA").then_some(77u16);
        let find_o = |n: &[u8]| n.starts_with(b"OCL_").then_some(5u32);
        c.store.post_process_load(&find_t, &find_o);
        let t = &c.store.templates()[0];
        assert_eq!(t.projectile_tmpl, Some(77));
        assert_eq!(
            (
                t.fire_ocls[0],
                t.projectile_detonation_ocls[3],
                t.projectile_detonation_ocls[0]
            ),
            (Some(5), Some(5), None)
        );
        assert!(
            t.fire_ocl_names.iter().all(|n| n.is_empty()),
            "the names are cleared once resolved"
        );
    }

    #[test]
    fn a_range_of_shot_delays_draws_from_the_logic_random_numbers() {
        let mut t = WeaponTemplate::new();
        t.min_delay_between_shots = 10;
        t.max_delay_between_shots = 20;
        let mut rng = RandomValues::new(5);
        rng.init_game_logic_random(5);
        let d = t.get_delay_between_shots(&WeaponBonus::new(), &mut rng);
        assert!((10..=20).contains(&d));
        let draws: std::collections::BTreeSet<_> = (0..50)
            .map(|_| t.get_delay_between_shots(&WeaponBonus::new(), &mut rng))
            .collect();
        assert!(draws.len() > 1, "a range draws varying values");
        t.max_delay_between_shots = 10;
        let seed = rng.get_game_logic_random_seed();
        assert_eq!(t.get_delay_between_shots(&WeaponBonus::new(), &mut rng), 10);
        assert_eq!(
            rng.get_game_logic_random_seed(),
            seed,
            "a fixed delay does not"
        );
    }

    #[test]
    fn redefining_a_weapon_without_overrides_returns_and_overrides_never_join_the_store() {
        let mut c = ctx();
        load(
            &mut c,
            b"Weapon A\n PrimaryDamage = 1\n WeaponBonus = VETERAN DAMAGE 150%\nEnd\n",
            IniLoadType::Overwrite,
        )
        .unwrap();
        // the same name again: the block is not read, so its first line is an unknown top-level token
        let r = load(
            &mut c,
            b"Weapon A\n PrimaryDamage = 2\nEnd\n",
            IniLoadType::Overwrite,
        );
        assert_eq!(r.unwrap_err().kind, IniErrorKind::UnknownToken);
        assert_eq!(c.store.templates()[0].primary_damage, 1.0);
        // as an override: parsed into a copy that is dropped, except for the shared bonus set
        load(
            &mut c,
            b"Weapon A\n PrimaryDamage = 99\n WeaponBonus = VETERAN DAMAGE 200%\nEnd\n",
            IniLoadType::CreateOverrides,
        )
        .unwrap();
        assert_eq!(c.store.templates().len(), 1);
        assert_eq!(
            c.store.templates()[0].primary_damage,
            1.0,
            "the override never joins the store"
        );
        let set = c.store.templates()[0].extra_bonus.unwrap();
        assert_eq!(
            set.bonus(crate::logic::weapon_bonus::condition::VETERAN as usize)
                .get_field(FIELD_DAMAGE),
            2.0,
            "but its bonus line reached the original"
        );
        // a failing project name keeps `None` as read
        let mut c2 = ctx();
        let r = load(
            &mut c2,
            b"Weapon B\n ProjectileObject = None\n Bogus = 1\nEnd\n",
            IniLoadType::Overwrite,
        );
        assert!(r.is_err());
        assert_eq!(c2.store.templates()[0].projectile_name, b"None");
    }

    #[test]
    fn the_store_finds_by_name_and_keeps_delayed_and_historic_damage_until_reset() {
        let mut c = ctx();
        load(&mut c, b"Weapon A\n HistoricBonusWeapon = A\n HistoricBonusWeapon = None\nEnd\nWeapon B\n HistoricBonusWeapon = A\nEnd\n", IniLoadType::Overwrite).unwrap();
        assert!(c
            .store
            .find_weapon_template(&mut c.names, b"None")
            .unwrap()
            .is_none());
        let a = c
            .store
            .find_weapon_template(&mut c.names, b"A")
            .unwrap()
            .unwrap()
            .name_key;
        assert_eq!(
            c.store.templates()[0].historic_bonus_weapon,
            None,
            "the last line said None"
        );
        assert_eq!(c.store.templates()[1].historic_bonus_weapon, Some(a));
        assert!(c
            .store
            .find_weapon_template(&mut c.names, b"Nope")
            .unwrap()
            .is_none());
        assert!(c
            .store
            .new_weapon_template(&mut c.names, b"")
            .unwrap()
            .is_none());
        c.store.set_delayed_damage(
            a,
            &crate::common::base_type::Coord3D {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            9,
            4,
            5,
            &WeaponBonus::new(),
        );
        c.store.templates_mut()[0]
            .historic_damage
            .push(HistoricWeaponDamageInfo {
                frame: 3,
                location: Default::default(),
            });
        assert_eq!(c.store.delayed_damage().len(), 1);
        c.store.reset();
        assert!(c.store.delayed_damage().is_empty());
        assert!(c.store.templates()[0].historic_damage.is_empty());
        assert!(c.store.find_weapon_template_private(a).is_some());
    }

    #[test]
    fn undefined_inputs_are_errors() {
        let mut c = ctx();
        let r = load(
            &mut c,
            b"Weapon A\n DelayBetweenShots =\nEnd\n",
            IniLoadType::Overwrite,
        );
        assert!(r.unwrap_err().is_undefined());
        let mut t = WeaponTemplate::new();
        t.pre_attack_delay = i32::MAX;
        let mut b = WeaponBonus::new();
        b.set_field(FIELD_PRE_ATTACK, 2.0);
        assert_eq!(
            t.get_pre_attack_delay(&b),
            Err(WeaponError::FloatToIntOutOfRange)
        );
        let _ = (
            NO_MAX_SHOTS_LIMIT,
            PREFIRE_PER_ATTACK,
            PREFIRE_PER_CLIP,
            WEAPON_COLLIDE_ALLIES,
            WEAPON_KILLS_SELF,
        );
    }
}
