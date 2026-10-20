// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::weapon_fire` (the `Weapon` instance, `fireWeaponTemplate`,
//! `dealDamageInternal`, `estimateWeaponTemplateDamage`, `shouldProjectileCollideWith`, `WeaponStore::update`,
//! `createAndFireTempWeapon`, `handleProjectileDetonation`) and `logic::weapon_set_state` (`WeaponSet`) against
//! upstream's own `Weapon.cpp` and `WeaponSet.cpp` (commit 0a05454d). Phase 1 layer 8, p1w8-sonnet, slice B.
//!
//! # What is compared against what
//!
//! `build.rs` takes the text of `Weapon.cpp` (every function: the template's firing half, the store, the class
//! `Weapon`) and of `WeaponSet.cpp` (the class `WeaponSet` and the members of `WeaponTemplateSet` that are not INI
//! parsers), with only the include lines replaced, the bit-name list of `WeaponSetFlags` and the four INI parsers of
//! `WeaponTemplateSet` cut (the sets are built without INI) and one `Bool` added to `const isProjectileDetonation =
//! false;` (an implicit `int` that only MSVC accepts); every cut is asserted to occur once. `differential/wrap_weapon.cpp`
//! compiles them under clang++ on the stand-ins of `differential/shim_weapon/fire_standins.h`: objects, players,
//! teams, the drawable, the AI, the contain module, the partition manager, the terrain, the game logic, the thing
//! factory, the client. An object's `WeaponSet` is upstream's own, so the shared-reload loop of `Weapon.cpp` acts on the
//! same weapons upstream's does. The Rust side runs the same program on `Sim`, a [`WeaponHost`] that answers from the same
//! configuration with the same arithmetic.
//!
//! The stand-ins that stand for other units (the distances, the line of sight, the terrain heights, the bone
//! transforms) are small formulas that both sides compute with the same operations; they are not under test, what
//! the weapon code does with them is. Every call that has an effect (a client effect, an object creation, damage, a
//! module call) writes a line, formatted alike on both sides; reals are written as bit patterns.
//!
//! # Method
//!
//! A **program** is a header (the world: the weapon templates with every field the code reads, the players and their
//! relations, the thing templates and their weapon sets, the objects with their fields and modules) and a list of
//! operations on it; the same words run on both sides. After the header and after every operation both sides write the
//! state: the logic random numbers' CRC, every standalone weapon, every object's weapon set and weapons, the store's
//! delayed damage and every template's historic damage. Compared line for line.
//!
//! The operations: advance the frame; create, copy and assign a weapon; load or reload it; its status; the clip
//! percentage; a bonus change; fire at an object or a position (also as a projectile detonation, and force fire);
//! pre-fire; every range and line-of-sight query; the approach target; the template-level estimate, collision test
//! and damage; temporary weapons and detonations; the store's update; the launch position and positioning a projectile;
//! the weapon's `crc` and `xfer` (saved, then loaded into a copy); and, for an object's weapon set, update, reload
//! all, bonus change, choose the best weapon, both "able to attack" questions, locks, the queries, `xfer`, clearing the
//! leech range, and a change to the object (its weapon-set flags, bonus word, a status bit, its position).
//!
//! Inputs for which upstream is undefined behaviour or throws (a null `Object*` dereferenced; `newObject(NULL)`,
//! which throws `ERROR_BAD_ARG`, for a projectile stream name that names no template) are **not** given to the C++:
//! the Rust returns [`WeaponError`] and the program is counted as excluded.
//!
//! One stand-in differs from upstream on purpose: `MemoryPoolObjectHolder`'s destructor tests its pointer for null.
//! Upstream's destructor calls `m_mpo->deleteInstance()` unconditionally and relies on the pool's `deleteInstance`
//! tolerating a null `this` (the zero-radius branch of `dealDamageInternal` holds a null iterator); clang removes a
//! `this` test, so the stand-in tests the pointer instead.
//!
//! # Planted bugs
//!
//! Each was applied to the Rust translation alone (one edit at a time, reverted after), and the comparison run
//! (600 programs unless noted). `mismatches` is the count of differing trace lines; the clean run has 0.
//!
//! | # | planted bug | mismatches |
//! |---|---|---|
//! | 1 | `0x7fff_ffff` (unlimited clip) -> `0x7fff_fffe` | 80181 |
//! | 2 | `reloadWithBonus` drops `&& !shared` | 3787 |
//! | 3 | `loadInstantly` reload time inverted | 220512 |
//! | 4 | `preFireWeapon` `delay > 0` -> `>= 0` | 8377 |
//! | 5 | `setClipPercentFull` status test un-inverted (upstream's defect fixed) | 1756 |
//! | 6 | `getStatus` `now < whenPreAttackFinished` -> `<=` | 1475 |
//! | 7 | `getStatus` `now >= whenWeCanFireAgain` -> `>` | 177848 |
//! | 8 | `DEATH_BURNED` 3 -> 2 | 178 |
//! | 9 | historic-damage count `>= n - 1` -> `>= n` | 16534 |
//! | 10 | historic-damage window `>=` -> `>` (edge of the window) | 0 at 600 programs, **1021 at 30000** |
//! | 11 | damage radius `max(primary, secondary)` -> primary | 42552 |
//! | 12 | `WEAPON_KILLS_SELF` test inverted | 31723 |
//! | 13 | `operator=` drops `m_suspendFXFrame` | 1519 |
//! | 14 | `m_pitchLimited` `||` -> `&&` | 0 at first, then **197** once the generator gave one-sided limits |
//! | 15 | `onWeaponBonusChange` ignores the between-shots case | 2080 |
//! | 16 | `onWeaponBonusChange` next-shot frame `+ 1` | 4963 |
//! | 17 | `getPreAttackDelay` compares the ammo as signed (a real defect found by the first run, kept here as a plant) | 7 |
//! | 18 | `setClipPercentFull` never reduces | 4017 |
//! | 19 | `chooseBestWeaponForTarget` drops the `CMD_DEFAULT_SWITCH_WEAPON` mask quirk | 5955 |
//! | 20 | `chooseBestWeaponForTarget` out-of-ammo test: auto-reload inverted | 0 at 600, **3493 at 30000** (needed op 36) |
//! | 21 | `chooseBestWeaponForTarget` anti-mask test inverted | 58488 |
//!
//! The weapon half of `Object` (`logic::world::object_weapons`, ops 37 to 47: Object.cpp's own text of those
//! functions is compiled on the stand-ins) has its own plants, run at 3000 programs:
//!
//! | # | planted bug | mismatches |
//! |---|---|---|
//! | O1 | `fireCurrentWeapon` fires a weapon that is not ready | 102326 |
//! | O2 | temporary locks released when the clip was **not** reloaded | 24 |
//! | O3 | `preFireCurrentWeapon` tests `frame` instead of `frame + 1` | 743 |
//! | O4 | `getLargestWeaponRange` starts at -2 instead of -1 | 1246 |
//! | O5 | `adjustModelConditionForWeaponStatus`: `lastShotFrame <= now` for `== now` | 228249 |
//! | O6 | the ready-to-fire "between" special case dropped | 8211 |
//! | O7 | the status to condition table with BETWEEN and RELOADING swapped | 30504 |
//! | O8 | `preAttackDone > now` -> `>=` | 0 at 3000 programs, **6 at 30000** |
//! | O9 | `friend_setUndetectedDefector(false)` dropped (after `fireCurrentWeapon`), or inverted (after `preFire`) | 142229, 336103 |
//! | O10 | `getAmmoPipShowingInfo` total and full swapped | 176 |
//! | O11 | `getLastShotFiredFrame` keeps the smallest | 39 |
//! | O12 | `setFiringConditionForCurrentWeapon` sets BETWEEN | 1346 |
//! | O13 | `getCurrentWeapon` always has a weapon | 543 |
//! | O14 | the firing tracker never told | 96869 |
//! | O15 | the shared reload of `fireCurrentWeapon` not applied to the other weapons | 1216 |
//! | O16 | the attacking test of the model condition inverted | 107977 |
//! | O17 | `fireCurrentWeapon(pos)` ignores the shared reload | 417 |
//! | B1 | bone: no drawable test (any object asked) | 49 |
//! | B2, B3 | the turret twist: a sign, a rotation offset | 175, 175 |
//! | B4, B5 | `convertToWorld` inverted; the start index | 423, 448 |
//! | B6 | `canProduceUpgrade` matches any upgrade | 143 |
//! | B7 | the command-set override ignored | the C++ crashes on the program the Rust no longer excludes |
//! | B8, B9 | the failed lookup's position and transform | 125, 117 |
//! | B10 | the turret function without its AI test | 80 |
//!
//! (One plant is equivalent: passing `i` for the weapon's own slot to `updateDrawableClipStatus`: the same value.)
//!
//! Plants 10, 14 and 20 first passed unnoticed; the generator was strengthened (short historic windows, one-sided
//! pitch limits, op 36 empties a set weapon's clip) and 10 and 20 were caught only at 30000 programs. One further plant
//! is **equivalent, not missed**: removing `m_projectileStreamID = INVALID_ID` in `newProjectileFired` (the reset is
//! overwritten by the very next statement unless creating the stream fails, and that failure is `ERROR_BAD_ARG`, an
//! excluded program).
//!
//! # Results
//!
//! Measured 2026-10-20 against upstream commit 0a05454d (clang++), generator seeds `0x57ea0000 + n`:
//!
//! * 3000 programs (the default; `WEAPON_FIRE_CASES`): about 2.15 million trace lines compared, **0 mismatches**,
//!   about 400 programs excluded (undefined upstream, by reason in the `RESULT` lines).
//! * **30000 programs: 21,426,969 trace lines compared, 0 mismatches**, 4,115 programs excluded: 1,087 dereference a
//!   null object, 1,870 read an out-parameter upstream never wrote (the turret bone function with no launch
//!   offset), 635 give `newObject` a NULL template (upstream throws), 523 position a projectile with no drawable.
//! * Coverage of the 3000-program run (events written): damage about 4,500, estimates 5,400, FX 2,900, object
//!   creations 1,100, OCLs 760, projectile launches 650, lasers 70, projectile streams 20, land-mine disarms 55,
//!   firing-tracker calls 690, model-condition changes 3,700, `xfer` round trips about 1,250, bone queries 2,100,
//!   `canProduceUpgrade` 680.
//! * Earlier runs of this test found: the unsigned comparison of `getPreAttackDelay` (program 370 of 600), the
//!   `newObject(NULL)` throw, the self-assignment guard of `operator=`, and (program 7910 of 30,000) that firing
//!   must take only the weapon, not the whole set, out of the object.
//!
//! Interpretation: the translation of the `Weapon` instance, the `WeaponSet` and the weapon half of `Object` agrees
//! bit for bit with upstream over this generator's inputs. Not covered: the host methods themselves (what the
//! layer-12 adapter will answer from the real `Object`, `Player`, AI and partition manager), the INI parsers, and the
//! cut points listed in `weapon_host`.
//!
#![cfg(feature = "differential")]

mod support;

use std::collections::BTreeSet;
use std::ffi::CString;

use rts_lockstep_engine::common::base_type::{Coord3D, Region3D, PI};
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::game_type::{ObjectId, PathfindLayer};
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::kind_of::{KindOfMaskType, KindOfType};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::object_status_types::ObjectStatusTypes;
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::sparse_match_finder::SparseMatchFinder;
use rts_lockstep_engine::common::xfer::{Xfer, XferStatus};
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::damage::{DamageInfo, DamageInfoInput, DAMAGE_EXPLOSION, DAMAGE_SMALL_ARMS};
use rts_lockstep_engine::logic::weapon::{FxName, WeaponError, WeaponStore};
use rts_lockstep_engine::logic::weapon_bonus::{WeaponBonus, WeaponBonusSet, WEAPON_BONUS_FIELD_COUNT};
use rts_lockstep_engine::logic::weapon_fire::{
    create_and_fire_temp_weapon, create_and_fire_temp_weapon_at_pos, deal_damage_internal,
    estimate_weapon_template_damage, handle_projectile_detonation, should_projectile_collide_with, weapon_store_update, Weapon, WeaponSnapshot,
    WeaponXferError,
};
use rts_lockstep_engine::logic::weapon_host::{
    AbleToAttackType, CanAttackResult, CommandSourceType, ContainFacts, DistanceCalculationType, LaunchOffset, WeaponFx, WeaponHost,
    WeaponTemplateSetRef, WeaponXferLookups,
};
use rts_lockstep_engine::logic::weapon_set::WeaponTemplateSet;
use rts_lockstep_engine::logic::weapon_set_flags::WeaponSetFlags;
use rts_lockstep_engine::logic::weapon_set_state::WeaponSet;
use rts_lockstep_engine::logic::weapon_types::WeaponStatus;
use rts_lockstep_engine::common::model_state::{ModelConditionFlags, MODELCONDITION_COUNT};
use rts_lockstep_engine::logic::world::object_weapons::{self as ow, FireAt, ObjectBoneHost, ObjectWeaponHost};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
use support::{Check, Gen};

/// The C++ side has process-global state (the name keys, the logic random numbers, the store, the objects), so the tests
/// take turns.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// ---------------------------------------------------------------------------------------------------------------------
// The words of a program
// ---------------------------------------------------------------------------------------------------------------------

struct Reader<'a> {
    w: &'a [u32],
    p: usize,
}

impl<'a> Reader<'a> {
    fn u(&mut self) -> u32 {
        let v = self.w[self.p];
        self.p += 1;
        v
    }
    fn i(&mut self) -> i32 {
        self.u() as i32
    }
    fn b(&mut self) -> bool {
        self.u() != 0
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(self.u())
    }
    fn coord(&mut self) -> Coord3D {
        let x = self.f();
        let y = self.f();
        let z = self.f();
        Coord3D { x, y, z }
    }
}

/// The writer of a program (the generator's side).
#[derive(Default)]
struct W(Vec<u32>);

impl W {
    fn u(&mut self, v: u32) {
        self.0.push(v);
    }
    fn i(&mut self, v: i32) {
        self.0.push(v as u32);
    }
    fn b(&mut self, v: bool) {
        self.0.push(v as u32);
    }
    fn f(&mut self, v: f32) {
        self.0.push(v.to_bits());
    }
    fn c(&mut self, v: [f32; 3]) {
        for x in v {
            self.f(x);
        }
    }
}

fn fb(f: f32) -> String {
    format!("{:08x}", f.to_bits())
}
fn fb3(c: &Coord3D) -> String {
    format!("{},{},{}", fb(c.x), fb(c.y), fb(c.z))
}

// ---------------------------------------------------------------------------------------------------------------------
// The world: objects, things, players
// ---------------------------------------------------------------------------------------------------------------------

/// One stand-in object (`WpnTestObject` in fire_standins.h): its configuration and the little state operations change.
#[derive(Clone)]
struct Obj {
    id: i32,
    tmpl: usize,
    player: i32,
    pos: Coord3D,
    orientation: f32,
    xform: Matrix3D,
    geom: GeometryInfo,
    kind: KindOfMaskType,
    status: u64,
    script_status: u32,
    veterancy: i32,
    locally: bool,
    bonus_cond: u32,
    contained_by: i32,
    producer: i32,
    eff_dead: bool,
    destroyed: bool,
    able_to_attack: bool,
    layer: i32,
    above: bool,
    signif: bool,
    consecutive: i32,
    estimate: f32,
    ws_flags: WeaponSetFlags,
    has_drawable: bool,
    barrels: [i32; 3],
    fx_handled: bool,
    has_launch: bool,
    launch_xform: Matrix3D,
    turret_rot_pos: Coord3D,
    turret_pitch_pos: Coord3D,
    has_laser: bool,
    has_ai: bool,
    sneaky: bool,
    sneaky_offset: Coord3D,
    loco: i32,
    adjusts: bool,
    assault: bool,
    turret: [i32; 3],
    turret_angle: [f32; 3],
    turret_pitch: [f32; 3],
    aiming: [bool; 3],
    has_contain: bool,
    contain_count: i32,
    garrisonable: bool,
    immune_clear: bool,
    passed_bonus: bool,
    passed_flags: u32,
    enclosing: BTreeSet<i32>,
    best_garrison: bool,
    passenger_fire: bool,
    has_items: bool,
    item_ids: Vec<i32>,
    items: Vec<i32>,
    apparent: i32,
    has_stealth: bool,
    disguised: bool,
    disguised_player: i32,
    has_spawn: bool,
    spawn_result: i32,
    has_land_mine: bool,
    has_pui: bool,
    has_parking: bool,
    has_spc_die: bool,
    has_stream: bool,
    has_assist: bool,
    assist_free: bool,
    has_cm: bool,
    has_phys: bool,
    launcher: i32,
    reserved: BTreeSet<i32>,
    collides: BTreeSet<i32>,
    cmd_override: i32,
    has_tracker: bool,
    last_cond: [i8; 3],
    weapon_set: WeaponSet,
}

fn weapon_set_flags_of(mask: u32) -> WeaponSetFlags {
    let mut f = WeaponSetFlags::new();
    for b in 0..17 {
        if mask & (1 << b) != 0 {
            f.set(b, 1);
        }
    }
    f
}

/// One object record (the words of `readObject` in fire_driver.h).
fn read_object(r: &mut Reader<'_>, things: usize) -> Obj {
    let tmpl_id = r.i();
    let tmpl = if tmpl_id >= 1 && tmpl_id as usize <= things { tmpl_id as usize - 1 } else { 0 };
    let player = r.i();
    let pos = r.coord();
    let orientation = r.f();
    let mut xform = Matrix3D::identity();
    xform.rotate_z(orientation);
    xform.set_translation(&Vector3::new(pos.x, pos.y, pos.z));
    let gt = GeometryType::from_index(r.u() as i32).expect("geometry type");
    let small = r.b();
    let (h, ma, mi) = (r.f(), r.f(), r.f());
    let geom = GeometryInfo::new(gt, small, h, ma, mi);
    let mut kind = KindOfMaskType::new();
    for _ in 0..r.u() {
        kind.set(r.u() as i32, 1);
    }
    let (lo, hi) = (r.u(), r.u());
    let status = (u64::from(hi) << 32) | u64::from(lo);
    let script_status = r.u();
    let veterancy = r.i();
    let locally = r.b();
    let bonus_cond = r.u();
    let contained_by = r.i();
    let producer = r.i();
    let (eff_dead, destroyed, able_to_attack) = (r.b(), r.b(), r.b());
    let layer = r.i();
    let (above, signif) = (r.b(), r.b());
    let consecutive = r.i();
    let estimate = r.f();
    let ws_flags = weapon_set_flags_of(r.u());
    let has_drawable = r.b();
    let barrels = [r.i(), r.i(), r.i()];
    let fx_handled = r.b();
    let has_launch = r.b();
    let (rz, t) = (r.f(), r.coord());
    let mut launch_xform = Matrix3D::identity();
    launch_xform.rotate_z(rz);
    launch_xform.set_translation(&Vector3::new(t.x, t.y, t.z));
    let turret_rot_pos = r.coord();
    let turret_pitch_pos = r.coord();
    let has_laser = r.b();
    let has_ai = r.b();
    let sneaky = r.b();
    let sneaky_offset = r.coord();
    let loco = r.i();
    let adjusts = r.b();
    let assault = r.b();
    let turret = [r.i(), r.i(), r.i()];
    let turret_angle = [r.f(), r.f(), r.f()];
    let turret_pitch = [r.f(), r.f(), r.f()];
    let aiming = [r.b(), r.b(), r.b()];
    let has_contain = r.b();
    let contain_count = r.i();
    let (garrisonable, immune_clear, passed_bonus) = (r.b(), r.b(), r.b());
    let passed_flags = r.u();
    let mut enclosing = BTreeSet::new();
    for _ in 0..r.u() {
        enclosing.insert(r.i());
    }
    let best_garrison = r.b();
    let passenger_fire = r.b();
    let has_items = r.b();
    let mut item_ids = Vec::new();
    for _ in 0..r.u() {
        item_ids.push(r.i());
    }
    let apparent = r.i();
    let has_stealth = r.b();
    let disguised = r.b();
    let disguised_player = r.i();
    let has_spawn = r.b();
    let spawn_result = r.i();
    let (has_land_mine, has_pui, has_parking, has_spc_die, has_stream, has_assist, assist_free, has_cm, has_phys) =
        (r.b(), r.b(), r.b(), r.b(), r.b(), r.b(), r.b(), r.b(), r.b());
    let launcher = r.i();
    let mut reserved = BTreeSet::new();
    for _ in 0..r.u() {
        reserved.insert(r.i());
    }
    let mut collides = BTreeSet::new();
    for _ in 0..r.u() {
        collides.insert(r.i());
    }
    let cmd_override = r.i();
    let has_tracker = r.b();
    let mut last_cond = [0i8; 3];
    for c in last_cond.iter_mut() {
        *c = r.i() as i8;
    }
    Obj {
        id: 0,
        tmpl,
        player,
        pos,
        orientation,
        xform,
        geom,
        kind,
        status,
        script_status,
        veterancy,
        locally,
        bonus_cond,
        contained_by,
        producer,
        eff_dead,
        destroyed,
        able_to_attack,
        layer,
        above,
        signif,
        consecutive,
        estimate,
        ws_flags,
        has_drawable,
        barrels,
        fx_handled,
        has_launch,
        launch_xform,
        turret_rot_pos,
        turret_pitch_pos,
        has_laser,
        has_ai,
        sneaky,
        sneaky_offset,
        loco,
        adjusts,
        assault,
        turret,
        turret_angle,
        turret_pitch,
        aiming,
        has_contain,
        contain_count,
        garrisonable,
        immune_clear,
        passed_bonus,
        passed_flags,
        enclosing,
        best_garrison,
        passenger_fire,
        has_items,
        item_ids,
        items: Vec::new(),
        apparent,
        has_stealth,
        disguised,
        disguised_player,
        has_spawn,
        spawn_result,
        has_land_mine,
        has_pui,
        has_parking,
        has_spc_die,
        has_stream,
        has_assist,
        assist_free,
        has_cm,
        has_phys,
        launcher,
        reserved,
        collides,
        cmd_override,
        has_tracker,
        last_cond,
        weapon_set: WeaponSet::new(),
    }
}

struct Thing {
    group: i32,
    fence: f32,
    proto: Vec<u32>,
    sets: Vec<WeaponTemplateSet>,
    finder: SparseMatchFinder,
}

#[derive(Clone, Copy)]
struct PlayerCfg {
    battle_plans: i32,
    mask: u16,
    rel: [i32; 4],
}

/// The Rust host: `WpnTestObject`, `WpnTestPlayer`, ... of fire_standins.h, in the same arithmetic.
struct Sim {
    frame: u32,
    hist_limit: u32,
    extent: Region3D,
    global_bonus: Option<WeaponBonusSet>,
    store: WeaponStore,
    rng: RandomValues,
    players: Vec<PlayerCfg>,
    things: Vec<Thing>,
    objs: Vec<Obj>,
    lines: std::cell::RefCell<Vec<String>>,
    next_id: i32,
}

fn id_of(name: &[u8]) -> u32 {
    1 + name.iter().map(|&b| u32::from(b)).sum::<u32>() % 1000
}

impl Sim {
    /// A trace line (`Rec` in fire_standins.h). The queries that write one (`estimateDamage`) are `&self`, hence the cell.
    fn rec(&self, s: String) {
        if std::env::var_os("WPNFIRE_RTRACE").is_some() {
            eprintln!("R {s}");
        }
        self.lines.borrow_mut().push(s);
    }
    fn ob(&self, o: ObjectId) -> &Obj {
        &self.objs[(o - 1) as usize]
    }
    fn ob_mut(&mut self, o: ObjectId) -> &mut Obj {
        &mut self.objs[(o - 1) as usize]
    }
    fn exists(&self, o: i32) -> bool {
        o >= 1 && o as usize <= self.objs.len()
    }
    fn opt(&self, o: i32) -> Option<ObjectId> {
        self.exists(o).then_some(o)
    }
    fn rad(&self, o: ObjectId, t: DistanceCalculationType) -> f32 {
        if t == DistanceCalculationType::FromBoundingSphere3D {
            self.ob(o).geom.get_bounding_sphere_radius()
        } else {
            self.ob(o).geom.get_bounding_circle_radius()
        }
    }
}

/// `wpnDist` of fire_standins.h.
fn wpn_dist(a: &Coord3D, ra: f32, b: &Coord3D, rb: f32, t: DistanceCalculationType) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    let mut d = dx * dx + dy * dy;
    if t == DistanceCalculationType::FromCenter3D || t == DistanceCalculationType::FromBoundingSphere3D {
        d += dz * dz;
    }
    if t == DistanceCalculationType::FromBoundingSphere2D || t == DistanceCalculationType::FromBoundingSphere3D {
        let r = ra + rb;
        d -= r * r;
        if d < 0.0 {
            d = 0.0;
        }
    }
    d
}

fn fx_str(fx: &FxName) -> String {
    match fx {
        None => "-".into(),
        Some(n) => String::from_utf8_lossy(n).into_owned(),
    }
}

impl WeaponXferLookups for Sim {
    fn find_thing_template(&self, name: &[u8]) -> Option<u16> {
        // "T<n>" is the n-th stand-in thing template (what WeaponSet::xfer saves); "Proj..." names find a stand-in id
        if name.first() == Some(&b'T') && name.get(1).is_some_and(u8::is_ascii_digit) {
            let n: usize = String::from_utf8_lossy(&name[1..]).parse().unwrap_or(0);
            return (n >= 1 && n <= self.things.len()).then_some(n as u16);
        }
        if !name.starts_with(b"Proj") {
            return None;
        }
        Some(id_of(name) as u16)
    }
    fn thing_template_name(&self, id: u16) -> Vec<u8> {
        format!("T{id}").into_bytes()
    }
    fn find_weapon_template_set_by_flags(&mut self, thing_template: u16, flags: &WeaponSetFlags) -> Option<WeaponTemplateSetRef> {
        let t = &mut self.things[thing_template as usize - 1];
        let index = t.finder.find_best_info(&t.sets, flags)?;
        Some(WeaponTemplateSetRef { thing_template, index })
    }
    fn weapon_template_set(&self, r: WeaponTemplateSetRef) -> &WeaponTemplateSet {
        &self.things[r.thing_template as usize - 1].sets[r.index]
    }
    fn weapon_template_names(&self) -> Vec<(Vec<u8>, usize)> {
        self.store.templates().iter().enumerate().map(|(i, t)| (t.name.clone(), i)).collect()
    }
    fn weapon_template_index(&self, key: rts_lockstep_engine::common::name_key_generator::NameKeyType) -> Option<usize> {
        self.store.templates().iter().position(|t| t.name_key == key)
    }
    fn weapon_template_name(&self, index: usize) -> Vec<u8> {
        self.store.templates()[index].name.clone()
    }
}

impl WeaponHost for Sim {
    fn frame(&self) -> u32 {
        self.frame
    }
    fn object_exists(&self, o: ObjectId) -> bool {
        self.exists(o)
    }
    fn store(&self) -> &WeaponStore {
        &self.store
    }
    fn store_mut(&mut self) -> &mut WeaponStore {
        &mut self.store
    }
    fn random_int(&mut self, lo: i32, hi: i32) -> i32 {
        self.rng.game_logic_random_value(lo, hi)
    }
    fn random_real(&mut self, lo: f32, hi: f32) -> f32 {
        self.rng.game_logic_random_value_real(lo, hi)
    }
    fn delay_between_shots(&mut self, template: usize, bonus: &WeaponBonus) -> i32 {
        let (store, rng) = (&self.store, &mut self.rng);
        store.templates()[template].get_delay_between_shots(bonus, rng)
    }
    fn global_weapon_bonus_set(&self) -> Option<&WeaponBonusSet> {
        self.global_bonus.as_ref()
    }
    fn historic_damage_limit(&self) -> u32 {
        self.hist_limit
    }
    fn rng(&mut self) -> &mut RandomValues {
        &mut self.rng
    }

    fn position(&self, o: ObjectId) -> Coord3D {
        self.ob(o).pos
    }
    fn orientation(&self, o: ObjectId) -> f32 {
        self.ob(o).orientation
    }
    fn transform_matrix(&self, o: ObjectId) -> Matrix3D {
        self.ob(o).xform
    }
    fn geometry(&self, o: ObjectId) -> GeometryInfo {
        self.ob(o).geom.clone()
    }
    fn is_kind_of(&self, o: ObjectId, kind: KindOfType) -> bool {
        self.ob(o).kind.test(kind)
    }
    fn is_kind_of_multi(&self, o: ObjectId, must_be_set: &KindOfMaskType, must_be_clear: &KindOfMaskType) -> bool {
        self.ob(o).kind.test_set_and_clear(must_be_set, must_be_clear)
    }
    fn test_status(&self, o: ObjectId, status: ObjectStatusTypes) -> bool {
        (self.ob(o).status >> status) & 1 != 0
    }
    fn test_script_status_bit(&self, o: ObjectId, bit: u32) -> bool {
        self.ob(o).script_status & bit != 0
    }
    fn veterancy_level(&self, o: ObjectId) -> i32 {
        self.ob(o).veterancy
    }
    fn has_drawable(&self, o: ObjectId) -> bool {
        self.ob(o).has_drawable
    }
    fn is_locally_controlled(&self, o: ObjectId) -> bool {
        self.ob(o).locally
    }
    fn weapon_bonus_condition(&self, o: ObjectId) -> u32 {
        self.ob(o).bonus_cond
    }
    fn contained_by(&self, o: ObjectId) -> Option<ObjectId> {
        self.opt(self.ob(o).contained_by)
    }
    fn container_passed_weapon_bonus(&self, container: ObjectId) -> Option<u32> {
        let c = self.ob(container);
        (c.has_contain && c.passed_bonus).then_some(c.passed_flags)
    }
    fn is_reload_time_shared(&self, o: ObjectId) -> bool {
        match self.ob(o).weapon_set.cur_weapon_template_set() {
            Some(r) => self.weapon_template_set(r).is_shared_reload_time(),
            None => false,
        }
    }
    fn apply_shared_reload(&mut self, o: ObjectId, next_shot_frame: u32, status: WeaponStatus) {
        for w in self.ob_mut(o).weapon_set.slots_mut().iter_mut().flatten() {
            w.set_possible_next_shot_frame(next_shot_frame);
            w.set_status(status);
        }
    }
    fn num_consecutive_shots_fired_at_target(&self, o: ObjectId, _victim: Option<ObjectId>) -> i32 {
        self.ob(o).consecutive
    }
    fn is_above_terrain(&self, o: ObjectId) -> bool {
        self.ob(o).above
    }
    fn is_airborne_target(&self, o: ObjectId) -> bool {
        self.test_status(o, rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_AIRBORNE_TARGET)
    }
    fn is_significantly_above_terrain(&self, o: ObjectId) -> bool {
        self.ob(o).signif
    }
    fn layer(&self, o: ObjectId) -> PathfindLayer {
        self.ob(o).layer
    }
    fn producer_id(&self, o: ObjectId) -> ObjectId {
        self.ob(o).producer
    }
    fn relationship(&self, a: ObjectId, b: ObjectId) -> Relationship {
        let (pa, pb) = (self.ob(a).player, self.ob(b).player);
        if pa < 0 || pb < 0 {
            return Relationship::Neutral;
        }
        match self.players[pa as usize].rel[pb as usize] {
            0 => Relationship::Enemies,
            1 => Relationship::Neutral,
            _ => Relationship::Allies,
        }
    }
    fn is_equivalent_template(&self, a: ObjectId, b: ObjectId) -> bool {
        self.things[self.ob(a).tmpl].group == self.things[self.ob(b).tmpl].group
    }
    fn fence_width(&self, o: ObjectId) -> f32 {
        self.things[self.ob(o).tmpl].fence
    }
    fn is_effectively_dead(&self, o: ObjectId) -> bool {
        self.ob(o).eff_dead
    }
    fn is_destroyed(&self, o: ObjectId) -> bool {
        self.ob(o).destroyed
    }
    fn is_able_to_attack(&self, o: ObjectId) -> bool {
        self.ob(o).able_to_attack
    }
    fn team_relationship_to_player_index(&self, o: ObjectId, player_index: i32) -> Relationship {
        match self.players[self.ob(o).player as usize].rel[player_index as usize] {
            0 => Relationship::Enemies,
            1 => Relationship::Neutral,
            _ => Relationship::Allies,
        }
    }
    fn find_weapon_template_set(&mut self, o: ObjectId) -> Option<WeaponTemplateSetRef> {
        let (ti, flags) = (self.ob(o).tmpl, self.ob(o).ws_flags);
        let t = &mut self.things[ti];
        let index = t.finder.find_best_info(&t.sets, &flags)?;
        Some(WeaponTemplateSetRef { thing_template: ti as u16 + 1, index })
    }
    fn estimate_damage(&self, o: ObjectId, input: &DamageInfoInput) -> f32 {
        self.rec(format!("est victim={} src={} dt={} death={} amt={}", o, input.source_id, input.damage_type, input.death_type, fb(input.amount)));
        self.ob(o).estimate
    }
    fn attempt_damage(&mut self, o: ObjectId, info: &mut DamageInfo) {
        let i = &info.input;
        let line = format!(
            "dmg victim={} src={} mask={} dt={} dst={} fxo={} death={} amt={} kill={} sw={} swa={} swr={} swt={}",
            o,
            i.source_id,
            i.source_player_mask,
            i.damage_type,
            i.damage_status_type,
            i.damage_fx_override,
            i.death_type,
            fb(i.amount),
            i.kill as i32,
            fb3(&i.shock_wave_vector),
            fb(i.shock_wave_amount),
            fb(i.shock_wave_radius),
            fb(i.shock_wave_taper_off)
        );
        self.rec(line);
    }
    fn destroy_object(&mut self, o: ObjectId) {
        self.rec(format!("destroy {o}"));
    }

    fn controlling_player(&self, o: ObjectId) -> Option<i32> {
        let p = self.ob(o).player;
        (p >= 0).then_some(p)
    }
    fn controlling_player_mask(&self, o: ObjectId) -> Option<rts_lockstep_engine::common::game_common::PlayerMaskType> {
        self.controlling_player(o).map(|p| self.players[p as usize].mask)
    }
    fn controlling_player_battle_plans(&self, o: ObjectId) -> i32 {
        self.players[self.ob(o).player as usize].battle_plans
    }
    fn apply_battle_plan_bonuses(&mut self, source: ObjectId, projectile: ObjectId) {
        let p = self.ob(source).player;
        self.rec(format!("battleplan p={projectile} by={p}"));
    }
    fn record_mine_cleared(&mut self, o: ObjectId) {
        let p = self.ob(o).player;
        self.rec(format!("academy.mine p={p}"));
    }
    fn controlling_player_objects(&self, o: ObjectId) -> Vec<ObjectId> {
        let p = self.ob(o).player;
        if p < 0 {
            return Vec::new();
        }
        self.objs.iter().filter(|x| x.player == p && !x.destroyed).map(|x| x.id).collect()
    }

    fn contain_facts(&self, o: ObjectId) -> Option<ContainFacts> {
        let c = self.ob(o);
        c.has_contain.then_some(ContainFacts { count: c.contain_count, garrisonable: c.garrisonable, immune_to_clear_building_attacks: c.immune_clear })
    }
    fn contain_is_enclosing_container_for(&self, container: ObjectId, inner: ObjectId) -> bool {
        self.ob(container).enclosing.contains(&inner)
    }
    fn contain_is_garrisonable(&self, container: ObjectId) -> bool {
        self.ob(container).garrisonable
    }
    fn contain_calc_best_garrison_position(&self, container: ObjectId, target_pos: &Coord3D) -> Option<Coord3D> {
        self.ob(container).best_garrison.then_some(Coord3D { x: target_pos.x + 1.0, y: target_pos.y + 2.0, z: target_pos.z + 3.0 })
    }
    fn contain_is_passenger_allowed_to_fire(&self, o: ObjectId) -> bool {
        self.ob(o).passenger_fire
    }
    fn contain_items(&self, o: ObjectId) -> Option<Vec<ObjectId>> {
        self.ob(o).has_items.then(|| self.ob(o).items.clone())
    }
    fn contain_apparent_controlling_player(&self, o: ObjectId, _observer: Option<i32>) -> Option<i32> {
        let a = self.ob(o).apparent;
        (a >= 0).then_some(a)
    }
    fn has_contain(&self, o: ObjectId) -> bool {
        self.ob(o).has_contain
    }
    fn stealth_disguised_player_index(&self, o: ObjectId) -> Option<i32> {
        let s = self.ob(o);
        (s.has_stealth && s.disguised).then_some(s.disguised_player)
    }
    fn player_relationship_to_player(&self, observer: Option<i32>, other_index: i32) -> Option<Relationship> {
        let o = observer?;
        if other_index < 0 || other_index as usize >= self.players.len() {
            return None;
        }
        Some(match self.players[o as usize].rel[other_index as usize] {
            0 => Relationship::Enemies,
            1 => Relationship::Neutral,
            _ => Relationship::Allies,
        })
    }
    fn spawn_slaves_can_use_weapon_against_target(
        &self,
        o: ObjectId,
        _attack_type: AbleToAttackType,
        _victim: Option<ObjectId>,
        _pos: Option<Coord3D>,
        _command_source: CommandSourceType,
    ) -> Option<CanAttackResult> {
        let s = self.ob(o);
        s.has_spawn.then_some(match s.spawn_result {
            0 => CanAttackResult::NotPossible,
            1 => CanAttackResult::InvalidShot,
            2 => CanAttackResult::PossibleAfterMoving,
            _ => CanAttackResult::Possible,
        })
    }
    fn member_able_to_use_weapon_against_target(
        &self,
        member: ObjectId,
        attack_type: AbleToAttackType,
        victim: Option<ObjectId>,
        pos: Option<Coord3D>,
        command_source: CommandSourceType,
    ) -> Result<CanAttackResult, WeaponError> {
        self.ob(member).weapon_set.get_able_to_use_weapon_against_target(self, attack_type, member, victim, pos, command_source, -1)
    }

    fn ai_sneaky_targeting_offset(&self, o: ObjectId) -> Option<Coord3D> {
        let s = self.ob(o);
        (s.has_ai && s.sneaky).then_some(s.sneaky_offset)
    }
    fn ai_cur_locomotor_set_type(&self, o: ObjectId) -> Option<i32> {
        let s = self.ob(o);
        s.has_ai.then_some(s.loco)
    }
    fn ai_is_aircraft_that_adjusts_destination(&self, o: ObjectId) -> bool {
        let s = self.ob(o);
        s.has_ai && s.adjusts
    }
    fn ai_begin_assault(&mut self, o: ObjectId, victim: Option<ObjectId>) {
        let s = self.ob(o);
        if s.has_ai && s.assault {
            self.rec(format!("assault o={} victim={}", o, victim.unwrap_or(0)));
        }
    }
    fn ai_which_turret_for_weapon_slot(&self, o: ObjectId, wslot: i32) -> Option<(i32, f32, f32)> {
        let s = self.ob(o);
        s.has_ai.then(|| (s.turret[wslot as usize], s.turret_angle[wslot as usize], s.turret_pitch[wslot as usize]))
    }
    fn ai_is_weapon_slot_on_turret_and_aiming_at_target(&self, o: ObjectId, wslot: i32, _victim: ObjectId) -> bool {
        let s = self.ob(o);
        s.has_ai && s.aiming[wslot as usize]
    }
    fn adjust_target_destination(&mut self, source: ObjectId, target: Option<ObjectId>, _pos: Option<Coord3D>, template: usize, approach: &mut Coord3D) {
        self.rec(format!("adjust src={} target={} tpl={}", source, target.unwrap_or(0), template));
        approach.x += 1.0;
        approach.y += 2.0;
    }

    fn distance_squared_to_object(&self, a: ObjectId, b: ObjectId, calc: DistanceCalculationType) -> f32 {
        wpn_dist(&self.ob(a).pos, self.rad(a, calc), &self.ob(b).pos, self.rad(b, calc), calc)
    }
    fn distance_squared_to_pos(&self, a: ObjectId, pos: &Coord3D, calc: DistanceCalculationType) -> f32 {
        wpn_dist(&self.ob(a).pos, self.rad(a, calc), pos, 0.0, calc)
    }
    fn goal_distance_squared_to_object(&self, a: ObjectId, goal: &Coord3D, b: ObjectId, calc: DistanceCalculationType) -> f32 {
        wpn_dist(goal, self.rad(a, calc), &self.ob(b).pos, self.rad(b, calc), calc)
    }
    fn goal_distance_squared_to_pos(&self, a: ObjectId, goal: &Coord3D, pos: &Coord3D, calc: DistanceCalculationType) -> f32 {
        wpn_dist(goal, self.rad(a, calc), pos, 0.0, calc)
    }
    fn vector_to_object(&self, a: ObjectId, b: ObjectId, _calc: DistanceCalculationType) -> Coord3D {
        let (pa, pb) = (self.ob(a).pos, self.ob(b).pos);
        Coord3D { x: pb.x - pa.x, y: pb.y - pa.y, z: pb.z - pa.z }
    }
    fn vector_to_pos(&self, a: ObjectId, pos: &Coord3D, _calc: DistanceCalculationType) -> Coord3D {
        let pa = self.ob(a).pos;
        Coord3D { x: pos.x - pa.x, y: pos.y - pa.y, z: pos.z - pa.z }
    }
    fn objects_in_range(&self, pos: &Coord3D, radius: f32, calc: DistanceCalculationType) -> Vec<(ObjectId, f32)> {
        let mut v = Vec::new();
        for o in &self.objs {
            let d = wpn_dist(pos, 0.0, &o.pos, self.rad(o.id, calc), calc);
            if d <= radius * radius {
                v.push((o.id, d));
            }
        }
        v
    }
    fn potential_collisions(&self, source: ObjectId) -> Vec<ObjectId> {
        self.ob(source).collides.iter().copied().filter(|&c| self.exists(c)).collect()
    }
    fn is_clear_line_of_sight_terrain(&self, from: &Coord3D, to: &Coord3D) -> bool {
        from.z + 0.5 * to.z >= 3.0
    }
    fn bridge_attack_points(&self, bridge: ObjectId) -> (Coord3D, Coord3D) {
        let p = self.ob(bridge).pos;
        (Coord3D { x: p.x + 10.0, y: p.y, z: p.z }, Coord3D { x: p.x - 10.0, y: p.y + 5.0, z: p.z })
    }
    fn layer_height(&self, x: f32, y: f32, layer: PathfindLayer) -> f32 {
        x * 0.01 + y * 0.02 + (layer as f32) * 1.5
    }
    fn ground_height(&self, x: f32, y: f32) -> f32 {
        x * 0.01 + y * 0.02
    }
    fn terrain_extent(&self) -> Region3D {
        self.extent
    }

    fn new_object_for_player_of(&mut self, template: u16, source: ObjectId) -> ObjectId {
        let player = self.ob(source).player;
        let id = self.next_id;
        self.rec(format!("newobj tpl={template} team={player} -> {id}"));
        let proto = self.things[(template as usize - 1) % self.things.len()].proto.clone();
        let mut o = read_object(&mut Reader { w: &proto, p: 0 }, self.things.len());
        o.id = id;
        self.next_id += 1;
        o.player = player;
        o.pos = Coord3D { x: 0.0, y: 0.0, z: 0.0 };
        o.xform = Matrix3D::identity();
        self.objs.push(o);
        id
    }
    fn set_producer(&mut self, projectile: ObjectId, source: ObjectId) {
        self.ob_mut(projectile).producer = source;
        self.rec(format!("producer p={projectile} s={source}"));
    }
    fn has_special_power_completion_die(&self, o: ObjectId) -> bool {
        self.ob(o).has_spc_die
    }
    fn special_power_completion_die_notify_script_engine(&mut self, o: ObjectId) {
        self.rec(format!("spc.notify o={o}"));
    }
    fn special_power_completion_die_set_creator(&mut self, o: ObjectId, creator: ObjectId) {
        self.rec(format!("spc.creator o={o} c={creator}"));
    }
    fn has_projectile_update_interface(&self, o: ObjectId) -> bool {
        self.ob(o).has_pui
    }
    fn projectile_launch(
        &mut self,
        projectile: ObjectId,
        victim: Option<ObjectId>,
        destination: &Coord3D,
        source: ObjectId,
        wslot: i32,
        barrel: i32,
        template: usize,
        exhaust: &FxName,
    ) {
        self.rec(format!(
            "launch proj={} victim={} dest={} src={} slot={} bar={} tpl={} exhaust={}",
            projectile,
            victim.unwrap_or(0),
            fb3(destination),
            source,
            wslot,
            barrel,
            template,
            fx_str(exhaust)
        ));
        self.ob_mut(projectile).launcher = source;
    }
    fn projectile_launcher_id(&self, o: ObjectId) -> Option<ObjectId> {
        let s = self.ob(o);
        s.has_pui.then_some(s.launcher)
    }
    fn set_position(&mut self, o: ObjectId, pos: &Coord3D) {
        self.ob_mut(o).pos = *pos;
        self.rec(format!("setpos o={} {}", o, fb3(pos)));
    }
    fn has_countermeasures(&self, o: ObjectId) -> bool {
        self.ob(o).has_cm
    }
    fn report_missile_for_countermeasures(&mut self, o: ObjectId, projectile: ObjectId) {
        self.rec(format!("cm.report o={o} missile={projectile}"));
    }
    fn has_land_mine_interface(&self, o: ObjectId) -> bool {
        self.ob(o).has_land_mine
    }
    fn disarm_land_mine(&mut self, o: ObjectId) {
        self.rec(format!("lm.disarm o={o}"));
    }
    fn projectile_stream_add(&mut self, stream: ObjectId, source: ObjectId, projectile: ObjectId, victim: Option<ObjectId>, victim_pos: Option<Coord3D>) -> bool {
        if !self.ob(stream).has_stream {
            return false;
        }
        let sp = self.ob(source).pos;
        self.rec(format!("stream.pos o={} {}", stream, fb3(&sp)));
        let vp = victim_pos.unwrap_or(Coord3D { x: 0.0, y: 0.0, z: 0.0 });
        self.rec(format!("stream.add o={} src={} proj={} victim={} pos={}", stream, source, projectile, victim.unwrap_or(0), fb3(&vp)));
        true
    }
    fn assisted_targeting_is_free(&self, o: ObjectId) -> Option<bool> {
        let s = self.ob(o);
        s.has_assist.then_some(s.assist_free)
    }
    fn assisted_targeting_assist_attack(&mut self, o: ObjectId, requesting: ObjectId, victim: ObjectId) {
        self.rec(format!("assist o={o} req={requesting} victim={victim}"));
    }
    fn laser_init(&mut self, laser: ObjectId, source: ObjectId, victim: Option<ObjectId>, pos: &Coord3D, bone_name: &[u8]) {
        let l = self.ob(laser);
        if l.has_drawable && l.has_laser {
            let sp = self.ob(source).pos;
            self.rec(format!(
                "laser o={} src={} victim={} start={} end={} bone={}",
                laser,
                source,
                victim.unwrap_or(0),
                fb3(&sp),
                fb3(pos),
                String::from_utf8_lossy(bone_name)
            ));
        }
    }
    fn create_ocl(&mut self, ocl: u32, primary: ObjectId) {
        self.rec(format!("ocl id={ocl} primary={primary}"));
    }

    fn barrel_count(&self, o: ObjectId, wslot: i32) -> Option<i32> {
        let s = self.ob(o);
        s.has_drawable.then(|| s.barrels[wslot as usize])
    }
    fn projectile_launch_offset(&self, o: ObjectId, _wslot: i32, _barrel: i32, _turret: i32) -> Option<LaunchOffset> {
        let s = self.ob(o);
        (s.has_drawable && s.has_launch).then_some(LaunchOffset { attach_transform: s.launch_xform, turret_rot_pos: s.turret_rot_pos, turret_pitch_pos: s.turret_pitch_pos })
    }
    fn convert_bone_transform_to_world(&self, o: ObjectId, bone_transform: &Matrix3D) -> Matrix3D {
        let mut w = Matrix3D::identity();
        w.mul(&self.ob(o).xform, bone_transform);
        w
    }
    fn weapon_fx(&mut self, fx: WeaponFx) {
        match fx {
            WeaponFx::Fire { source, weapon_slot, barrel, fx, speed, recoil_amount, recoil_dir, target, radius, contact_weapon } => {
                self.rec(format!(
                    "fx.fire src={} slot={} bar={} fx={} spd={} ra={} rd={} t={} rad={}",
                    source,
                    weapon_slot,
                    barrel,
                    fx_str(&fx),
                    fb(speed),
                    fb(recoil_amount),
                    fb(recoil_dir),
                    fb3(&target),
                    fb(radius)
                ));
                if !self.ob(source).fx_handled && fx.is_some() {
                    // bah. just play it at the drawable's pos.
                    let at = if contact_weapon { target } else { self.ob(source).pos };
                    self.rec(format!("fx.pos fx={} at={} spd={} t={} rad={}", fx_str(&fx), fb3(&at), fb(speed), fb3(&target), fb(radius)));
                }
            }
            WeaponFx::Disarm { fx, at } => {
                let p = self.ob(at).pos;
                self.rec(format!("fx.pos fx={} at={} spd={} t={} rad={}", fx_str(&fx), fb3(&p), fb(0.0), fb3(&p), fb(0.0)));
            }
        }
    }

    fn unhide_drawable(&mut self, o: ObjectId) {
        self.rec(format!("unhide o={o} hidden=0"));
    }
    fn set_transform_matrix(&mut self, o: ObjectId, m: &Matrix3D) {
        self.ob_mut(o).xform = *m;
        let words: Vec<String> = m.to_array().iter().map(|&x| fb(x)).collect();
        self.rec(format!("xform o={} {} ", o, words.join(" ")));
    }
    fn set_experience_sink(&mut self, o: ObjectId, sink: ObjectId) {
        self.rec(format!("xpsink o={o} sink={sink}"));
    }
    fn transfer_velocity_and_ignore_collisions(&mut self, launcher: ObjectId, projectile: ObjectId) {
        if self.ob(launcher).has_phys && self.ob(projectile).has_phys {
            self.rec(format!("phys.transfer from={launcher} to={projectile}"));
            self.rec(format!("phys.ignore o={projectile} with={launcher}"));
        }
    }
    fn parking_place_has_reserved_space(&self, o: ObjectId, intended: ObjectId) -> bool {
        let s = self.ob(o);
        s.has_parking && s.reserved.contains(&intended)
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Building the world from the header
// ---------------------------------------------------------------------------------------------------------------------

fn fx_name(prefix: &str, id: u32) -> FxName {
    (id != 0).then(|| format!("{prefix}{id}").into_bytes())
}

fn read_bonus_set(r: &mut Reader<'_>) -> WeaponBonusSet {
    let mut set = WeaponBonusSet::new();
    for _ in 0..r.u() {
        let (c, f, v) = (r.i(), r.i(), r.f());
        set.set_bonus_field(c as usize, f as usize, v);
    }
    set
}

/// `buildWorld` of fire_driver.h: the header of a program.
fn build_sim(r: &mut Reader<'_>, names: &mut NameKeyGenerator) -> Sim {
    let seed = r.u();
    let mut rng = RandomValues::new(0);
    rng.init_game_logic_random(seed);
    let frame = r.u();
    let hist_limit = r.u();
    let lo = r.coord();
    let hi = r.coord();
    let global_bonus = if r.b() { Some(read_bonus_set(r)) } else { None };
    let mut store = WeaponStore::new();
    let n_tpl = r.u();
    let mut hist_weapon = Vec::new();
    for k in 0..n_tpl {
        let idx = store.new_weapon_template(names, format!("W{k}").as_bytes()).expect("keys").expect("name");
        let t = &mut store.templates_mut()[idx];
        t.primary_damage = r.f();
        t.primary_damage_radius = r.f();
        t.secondary_damage = r.f();
        t.secondary_damage_radius = r.f();
        t.shock_wave_amount = r.f();
        t.shock_wave_radius = r.f();
        t.shock_wave_taper_off = r.f();
        t.attack_range = r.f();
        t.minimum_attack_range = r.f();
        t.request_assist_range = r.f();
        t.aim_delta = r.f();
        t.scatter_radius = r.f();
        t.scatter_target_scalar = r.f();
        for _ in 0..r.u() {
            let (x, y) = (r.f(), r.f());
            t.scatter_targets.push(rts_lockstep_engine::common::base_type::Coord2D { x, y });
        }
        t.damage_type = r.i();
        t.death_type = r.i();
        t.weapon_speed = r.f();
        t.weapon_recoil = r.f();
        t.min_target_pitch = r.f();
        t.max_target_pitch = r.f();
        t.radius_damage_angle = r.f();
        let proj = r.u();
        t.projectile_tmpl = (proj != 0).then_some(proj as u16);
        match r.u() {
            1 => t.laser_name = b"ProjLaser".to_vec(),
            2 => t.laser_name = b"Other".to_vec(),
            _ => {}
        }
        match r.u() {
            1 => t.projectile_stream_name = b"ProjStream".to_vec(),
            2 => t.projectile_stream_name = b"Other".to_vec(),
            _ => {}
        }
        t.laser_bone_name = b"Bone".to_vec();
        for v in 0..4 {
            let id = r.u();
            t.fire_ocls[v] = (id != 0).then_some(id);
        }
        for v in 0..4 {
            let id = r.u();
            t.projectile_detonation_ocls[v] = (id != 0).then_some(id);
        }
        for v in 0..4 {
            t.fire_fxs[v] = fx_name("fx", r.u());
        }
        for v in 0..4 {
            t.projectile_detonate_fxs[v] = fx_name("fx", r.u());
        }
        for v in 0..4 {
            t.projectile_exhausts[v] = fx_name("ps", r.u());
        }
        if r.b() {
            t.extra_bonus = Some(read_bonus_set(r));
        }
        t.clip_size = r.i();
        t.clip_reload_time = r.i();
        t.min_delay_between_shots = r.i();
        t.max_delay_between_shots = r.i();
        t.continuous_fire_one_shots_needed = r.i();
        t.continuous_fire_two_shots_needed = r.i();
        t.continuous_fire_coast_frames = r.u();
        t.auto_reload_when_idle_frames = r.u();
        t.shots_per_barrel = r.i();
        t.anti_mask = r.u();
        t.affects_mask = r.u();
        t.collide_mask = r.u();
        t.damage_dealt_at_self_position = r.b();
        t.reload_type = r.i();
        t.prefire_type = r.i();
        t.historic_bonus_time = r.u();
        t.historic_bonus_radius = r.f();
        t.historic_bonus_count = r.i();
        hist_weapon.push(r.i());
        t.leech_range_weapon = r.b();
        t.capable_of_following_waypoint = r.b();
        t.is_shows_ammo_pips = r.b();
        t.allow_attack_garrisoned_bldgs = r.b();
        t.play_fx_when_stealthed = r.b();
        t.pre_attack_delay = r.i();
        t.continue_attack_range = r.f();
        t.infantry_inaccuracy_dist = r.f();
        t.damage_status_type = r.i();
        t.suspend_fx_delay = r.u();
    }
    for (k, h) in hist_weapon.iter().enumerate() {
        if *h >= 0 {
            let key = store.templates()[*h as usize].name_key;
            store.templates_mut()[k].historic_bonus_weapon = Some(key);
        }
    }
    let n_players = r.u();
    let mut players = Vec::new();
    for _ in 0..n_players {
        let battle_plans = r.i();
        let mask = r.u() as u16;
        let rel = [r.i(), r.i(), r.i(), r.i()];
        players.push(PlayerCfg { battle_plans, mask, rel });
    }
    let n_thing = r.u();
    let mut things = Vec::new();
    for _ in 0..n_thing {
        let group = r.i();
        let fence = r.f();
        let len = r.u() as usize;
        let proto: Vec<u32> = (0..len).map(|_| r.u()).collect();
        things.push(Thing { group, fence, proto, sets: Vec::new(), finder: SparseMatchFinder::new() });
    }
    for _ in 0..r.u() {
        let tid = r.i();
        let flags = r.u();
        let mut template = [None; 3];
        for slot in template.iter_mut() {
            let idx = r.i();
            *slot = (idx >= 0).then(|| store.templates()[idx as usize].name_key);
        }
        let masks = [r.u(), r.u(), r.u()];
        let mut pref = [KindOfMaskType::new(); 3];
        for p in pref.iter_mut() {
            for _ in 0..r.u() {
                p.set(r.u() as i32, 1);
            }
        }
        let (shared, lock_shared) = (r.b(), r.b());
        things[tid as usize - 1].sets.push(WeaponTemplateSet::from_parts(Some(tid as u16), weapon_set_flags_of(flags), template, masks, pref, shared, lock_shared));
    }
    let mut sim = Sim {
        frame,
        hist_limit,
        extent: Region3D { lo, hi },
        global_bonus,
        store,
        rng,
        players,
        things,
        objs: Vec::new(),
        lines: Default::default(),
        next_id: 1,
    };
    let n_obj = r.u();
    for _ in 0..n_obj {
        let len = r.u() as usize;
        let mut sub = Reader { w: &r.w[..r.p + len], p: r.p };
        let mut o = read_object(&mut sub, sim.things.len());
        r.p += len;
        o.id = sim.next_id;
        sim.next_id += 1;
        sim.objs.push(o);
    }
    let count = sim.objs.len() as i32;
    for o in sim.objs.iter_mut() {
        o.items = o.item_ids.iter().copied().filter(|&i| i >= 1 && i <= count).collect();
    }
    sim
}

// ---------------------------------------------------------------------------------------------------------------------
// The dumps
// ---------------------------------------------------------------------------------------------------------------------

fn dump_weapon(w: &Weapon) -> String {
    let sc: String = w.scatter_targets_unused().iter().map(|x| format!("{x},")).collect();
    format!(
        "tpl={} slot={} st={} ammo={} next={} pre={} rel={} last={} sfx={} strm={} max={} bar={} nsb={} pl={} lr={} sc=[{}]",
        w.template(),
        w.weapon_slot(),
        w.raw_status() as i32,
        w.raw_ammo_in_clip(),
        w.possible_next_shot_frame(),
        w.pre_attack_finished_frame(),
        w.last_reload_started_frame(),
        w.last_shot_frame(),
        w.suspend_fx_frame(),
        w.projectile_stream_id(),
        w.max_shot_count(),
        w.cur_barrel(),
        w.num_shots_for_cur_barrel(),
        w.is_pitch_limited() as i32,
        w.has_leech_range() as i32,
        sc
    )
}

fn dump_snapshot(sim: &Sim, reg: &[Option<Weapon>]) {
    sim.rec(format!("frame={} rng={:08x}", sim.frame, sim.rng.get_game_logic_random_seed_crc()));
    for (i, w) in reg.iter().enumerate() {
        if let Some(w) = w {
            sim.rec(format!("W{} {}", i, dump_weapon(w)));
        }
    }
    for o in &sim.objs {
        let ws = &o.weapon_set;
        let tset = ws.cur_weapon_template_set().map_or(-1, |r| r.index as i32);
        sim.rec(format!(
            "O{} pos={} launcher={} bonus={} status={:016x} cur={} lock={} mask={} anti={} pl={} dw={} tset={} dmg={},{} lwc={},{},{}",
            o.id,
            fb3(&o.pos),
            o.launcher,
            o.bonus_cond,
            o.status,
            ws.get_cur_weapon_slot(),
            ws.cur_weapon_locked_status(),
            ws.filled_weapon_slot_mask(),
            ws.total_anti_mask(),
            ws.has_pitch_limit() as i32,
            ws.has_any_damage_weapon() as i32,
            tset,
            ws.total_damage_type_count(),
            ws.has_weapon_to_deal_damage_type(DAMAGE_EXPLOSION) as i32,
            o.last_cond[0],
            o.last_cond[1],
            o.last_cond[2]
        ));
        for k in 0..3 {
            if let Some(w) = ws.get_weapon_in_weapon_slot(k) {
                sim.rec(format!("O{}.{} {}", o.id, k, dump_weapon(w)));
            }
        }
    }
    for (i, t) in sim.store.templates().iter().enumerate() {
        if t.historic_damage.is_empty() {
            continue;
        }
        let s: String = t.historic_damage.iter().map(|h| format!("{}:{} ", h.frame, fb3(&h.location))).collect();
        sim.rec(format!("H{i} {s}"));
    }
    for d in sim.store.delayed_damage() {
        let tpl = sim.store.templates().iter().position(|t| t.name_key == d.delayed_weapon).map_or(-1, |i| i as i32);
        sim.rec(format!(
            "DD tpl={} pos={} frame={} src={} victim={} bonus={},{},{},{},{}",
            tpl,
            fb3(&d.delay_damage_pos),
            d.delay_damage_frame,
            d.delay_source_id,
            d.delay_intended_victim_id,
            fb(d.bonus.get_field(0)),
            fb(d.bonus.get_field(1)),
            fb(d.bonus.get_field(2)),
            fb(d.bonus.get_field(3)),
            fb(d.bonus.get_field(4))
        ));
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The operations
// ---------------------------------------------------------------------------------------------------------------------

/// Why a program was not given to the C++: upstream is undefined behaviour in it.
#[derive(Debug)]
struct Excluded(&'static str);

impl From<WeaponError> for Excluded {
    fn from(e: WeaponError) -> Self {
        Excluded(match e {
            WeaponError::NullTemplate => "newObject is given a NULL template (upstream throws)",
            WeaponError::UninitialisedRead => "an out-parameter that upstream never wrote is read",
            _ => "a null object is dereferenced",
        })
    }
}

const ST: [&str; 5] = ["ready", "oom", "between", "reload", "pre"];

fn status_name(s: WeaponStatus) -> &'static str {
    ST[s as usize]
}

fn rd_bonus(r: &mut Reader<'_>) -> WeaponBonus {
    let mut b = WeaponBonus::new();
    for f in 0..WEAPON_BONUS_FIELD_COUNT {
        b.set_field(f, r.f());
    }
    b
}

fn xfer_status(r: Result<(), WeaponXferError>) -> Result<u32, Excluded> {
    match r {
        Ok(()) => Ok(0),
        Err(WeaponXferError::Xfer(XferStatus::ErrorUnknown)) => Err(Excluded("a weapon without a template set is loaded")),
        Err(WeaponXferError::Xfer(s)) => Ok(1 + s as u32),
        Err(WeaponXferError::InvalidData) => Ok(100),
    }
}

/// Run the program on the Rust side: the trace lines, or why the program is excluded.
fn run_rust(words: &[u32], names: &mut NameKeyGenerator) -> Result<Vec<String>, Excluded> {
    let mut r = Reader { w: words, p: 0 };
    let mut sim = build_sim(&mut r, names);
    let mut reg: Vec<Option<Weapon>> = Vec::new();
    let n_ops = r.u();
    dump_snapshot(&sim, &reg);
    for _ in 0..n_ops {
        let op = r.u();
        sim.rec(format!("op {op}"));
        match op {
            0 => sim.frame = r.u(),
            1 => {
                let t = r.u() as usize;
                let slot = r.i();
                reg.push(Some(Weapon::new(&sim, t, slot)));
            }
            2 => {
                let s = reg[r.u() as usize].clone().expect("weapon");
                reg.push(Some(Weapon::copy_from(&s, &sim.store)));
            }
            3 => {
                let d = r.u() as usize;
                let s_idx = r.u() as usize;
                let s = reg[s_idx].clone().expect("weapon");
                // `if (this != &that)`: a self-assignment is a no-op
                if d != s_idx {
                    let mut w = reg[d].take().expect("weapon");
                    w.assign_from(&s, &sim.store);
                    reg[d] = Some(w);
                }
            }
            4 => {
                let wi = r.u() as usize;
                let src = r.i();
                let mode = r.u();
                let shared = sim.is_reload_time_shared(src);
                let w = reg[wi].as_mut().expect("weapon");
                let e = if mode == 0 { w.load_ammo_now(&sim, src, shared) } else { w.reload_ammo(&sim, src, shared) };
                if let Some(e) = e {
                    e.apply_via_host(&mut sim, src);
                }
            }
            5 => {
                let w = reg[r.u() as usize].as_ref().expect("weapon");
                let now = sim.frame;
                let st = w.get_status(now);
                let line = format!(
                    "ret status={} remaining={} pct={} next={}",
                    status_name(st),
                    w.remaining_ammo(now),
                    fb(w.get_percent_ready_to_fire(now)),
                    w.possible_next_shot_frame()
                );
                sim.rec(line);
            }
            6 => {
                let wi = r.u() as usize;
                let p = r.f();
                let a = r.b();
                reg[wi].as_mut().expect("weapon").set_clip_percent_full(&sim, p, a);
            }
            7 => {
                let wi = r.u() as usize;
                let src = r.i();
                let shared = sim.is_reload_time_shared(src);
                if let Some(e) = reg[wi].as_mut().expect("weapon").on_weapon_bonus_change(&mut sim, src, shared) {
                    e.apply_via_host(&mut sim, src);
                }
            }
            8 => {
                let wi = r.u() as usize;
                let src = r.i();
                let kind = r.u();
                let victim = r.i();
                let pos = r.coord();
                let extra = r.u();
                let inflict = r.b();
                let mut pid: ObjectId = 12345;
                let mut res = false;
                let mut forced = 0;
                let shared = sim.is_reload_time_shared(src);
                let w = reg[wi].as_mut().expect("weapon");
                let effect = match kind {
                    0 => {
                        let o = w.fire_weapon(&mut sim, src, victim, Some(&mut pid), shared)?;
                        res = o.reloaded;
                        o.shared
                    }
                    1 => {
                        let o = w.fire_weapon_at_pos(&mut sim, src, &pos, Some(&mut pid), shared)?;
                        res = o.reloaded;
                        o.shared
                    }
                    2 => w.fire_projectile_detonation_weapon(&mut sim, src, victim, extra, inflict, shared)?,
                    3 => w.fire_projectile_detonation_weapon_at_pos(&mut sim, src, &pos, extra, inflict, shared)?,
                    _ => {
                        let (f, o) = w.force_fire_weapon(&mut sim, src, &pos, shared)?;
                        forced = f.unwrap_or(0);
                        o.shared
                    }
                };
                if let Some(e) = effect {
                    e.apply_via_host(&mut sim, src);
                }
                sim.rec(format!("ret fire kind={kind} reloaded={} pid={pid} forced={forced}", res as i32));
            }
            9 => {
                let wi = r.u() as usize;
                let src = r.i();
                let victim = r.i();
                let v = sim.opt(victim);
                reg[wi].as_mut().expect("weapon").pre_fire_weapon(&mut sim, src, v)?;
            }
            10 => {
                let w = reg[r.u() as usize].as_ref().expect("weapon");
                let src = r.i();
                let victim = r.i();
                let pos = r.coord();
                let goal = r.coord();
                let v = sim.opt(victim);
                let b = |x: bool| x as i32;
                let inobj = v.map_or(-1, |v| b(w.is_within_attack_range(&sim, src, v)));
                let inpos = b(w.is_within_attack_range_pos(&sim, src, &pos));
                let closeobj = v.map_or(-1, |v| b(w.is_too_close(&sim, src, v)));
                let closepos = b(w.is_too_close_pos(&sim, src, &pos));
                let goalpos = b(w.is_goal_pos_within_attack_range(&sim, src, &goal, v, &pos));
                let sg1 = b(w.is_source_object_with_goal_position_within_attack_range(&sim, src, &goal, v, Some(&pos)));
                let sg2 = b(w.is_source_object_with_goal_position_within_attack_range(&sim, src, &goal, None, Some(&pos)));
                let range = fb(w.get_attack_range(&sim, src));
                let dist = fb(w.get_attack_distance(&sim, src, v));
                let pdr = fb(w.get_primary_damage_radius(&sim, src));
                let crt = w.get_clip_reload_time(&sim, src);
                let dmgw = b(w.is_damage_weapon(&sim.store));
                let pitch = v.map_or(-1, |v| b(w.is_within_target_pitch(&sim, src, v)));
                let est = match v {
                    Some(v) => w.estimate_weapon_damage(&sim, src, Some(v), None)?.to_bits(),
                    None => 0,
                };
                let los1 = v.map_or(-1, |v| b(w.is_clear_firing_line_of_sight_terrain(&sim, src, v)));
                let los2 = b(w.is_clear_firing_line_of_sight_terrain_pos(&sim, src, &pos));
                let los3 = v.map_or(-1, |v| b(w.is_clear_goal_firing_line_of_sight_terrain(&sim, src, &goal, v)));
                let los4 = b(w.is_clear_goal_firing_line_of_sight_terrain_pos(&sim, src, &goal, &pos));
                sim.rec(format!(
                    "ret ranges inobj={inobj} inpos={inpos} closeobj={closeobj} closepos={closepos} goalpos={goalpos} srcgoal={sg1},{sg2} range={range} dist={dist} pdr={pdr} crt={crt} dmgw={dmgw} pitch={pitch} est={est:08x} los={los1},{los2},{los3},{los4}"
                ));
            }
            11 => {
                let wi = r.u() as usize;
                let src = r.i();
                let target = r.i();
                let has_pos = r.b();
                let pos = r.coord();
                let ao = r.f();
                let t = sim.opt(target);
                let w = reg[wi].take().expect("weapon");
                let (close, out) = w.compute_approach_target(&mut sim, src, t, has_pos.then_some(pos), ao);
                reg[wi] = Some(w);
                sim.rec(format!("ret approach close={} out={}", close as i32, fb3(&out)));
            }
            12 => {
                let ti = r.u() as usize;
                let src = r.i();
                let victim = r.i();
                let has_pos = r.b();
                let pos = r.coord();
                let bonus = rd_bonus(&mut r);
                let v = sim.opt(victim);
                let e = estimate_weapon_template_damage(&sim, ti, Some(src), v, has_pos.then_some(pos), &bonus)?;
                sim.rec(format!("ret estimate {}", fb(e)));
            }
            13 => {
                let ti = r.u() as usize;
                let launcher = r.i();
                let proj = r.i();
                let thing = r.i();
                let intended = r.i();
                let c = should_projectile_collide_with(&sim, ti, sim.opt(launcher), sim.opt(proj), sim.opt(thing), intended);
                sim.rec(format!("ret collide {}", c as i32));
            }
            14 => {
                let ti = r.u() as usize;
                let src = r.i();
                let victim = r.i();
                let pos = r.coord();
                let bonus = rd_bonus(&mut r);
                let is_det = r.b();
                deal_damage_internal(&mut sim, ti, src, victim, pos, &bonus, is_det)?;
            }
            15 => {
                let ti = r.u() as usize;
                let src = r.i();
                let kind = r.u();
                let victim = r.i();
                let pos = r.coord();
                let extra = r.u();
                let inflict = r.b();
                match kind {
                    0 => create_and_fire_temp_weapon_at_pos(&mut sim, Some(ti), Some(src), &pos)?,
                    1 => create_and_fire_temp_weapon(&mut sim, Some(ti), Some(src), victim)?,
                    _ => handle_projectile_detonation(&mut sim, ti, src, &pos, extra, inflict)?,
                }
            }
            16 => weapon_store_update(&mut sim)?,
            17 => {
                let launcher = r.i();
                let wslot = r.i();
                let barrel = r.i();
                let (m, p) = Weapon::calc_projectile_launch_position(&sim, launcher, wslot, barrel);
                let words: Vec<String> = m.to_array().iter().map(|&x| fb(x)).collect();
                sim.rec(format!("ret launchpos pos={} m={} ", fb3(&p), words.join(" ")));
            }
            18 => {
                let proj = r.i();
                let launcher = r.i();
                let wslot = r.i();
                let barrel = r.i();
                let l = sim.opt(launcher);
                if l.is_some() && !sim.ob(proj).has_drawable {
                    // `projectile->getDrawable()->setDrawableHidden(false)` on a NULL drawable
                    return Err(Excluded("positionProjectileForLaunch of a projectile with no drawable"));
                }
                Weapon::position_projectile_for_launch(&mut sim, proj, l, wslot, barrel);
            }
            19 => {
                let wi = r.u() as usize;
                let mut w = reg[wi].take().expect("weapon");
                let name = sim.weapon_template_name(w.template());
                {
                    let mut xc = XferCrc::new();
                    xc.open(b"w").unwrap();
                    w.crc_with_name(&name, &mut xc).unwrap();
                    xc.close().unwrap();
                    sim.rec(format!("ret crc {:08x}", xc.get_crc()));
                }
                let mut save = XferSave::new();
                save.open(b"w").unwrap();
                w.xfer_with_names(&name, &[], &mut save).unwrap();
                save.close().unwrap();
                let bytes = save.take_bytes();
                sim.rec(format!("ret xfer-save len={} hash={:08x}", bytes.len(), hash_bytes(&bytes)));
                let c = Weapon::for_load(w.template(), w.weapon_slot());
                let mut snap = WeaponSnapshot { weapon: c, current_name: name.clone(), templates: sim.weapon_template_names(), error: None };
                let mut load = XferLoad::new(bytes);
                load.open(b"w").unwrap();
                let status = match load.xfer_snapshot(&mut snap) {
                    Ok(()) => 0,
                    Err(s) => match snap.error {
                        Some(_) => 100,
                        None => 1 + s as u32,
                    },
                };
                let _ = load.close();
                sim.rec(format!("ret xfer-load {} {}", status, dump_weapon(&snap.weapon)));
                reg[wi] = Some(w);
            }
            20 => {
                let o = r.i();
                let mut ws = std::mem::take(&mut sim.ob_mut(o).weapon_set);
                ws.update_weapon_set(&mut sim, o);
                sim.ob_mut(o).weapon_set = ws;
            }
            21 => {
                let o = r.i();
                let now = r.b();
                let mut ws = std::mem::take(&mut sim.ob_mut(o).weapon_set);
                ws.reload_all_ammo(&sim, o, now);
                sim.ob_mut(o).weapon_set = ws;
            }
            22 => {
                let o = r.i();
                let mut ws = std::mem::take(&mut sim.ob_mut(o).weapon_set);
                ws.weapon_set_on_weapon_bonus_change(&mut sim, o);
                sim.ob_mut(o).weapon_set = ws;
            }
            23 => {
                let o = r.i();
                let v = r.i();
                let crit = r.i();
                let cmd = r.i();
                let victim = sim.opt(v);
                let mut ws = std::mem::take(&mut sim.ob_mut(o).weapon_set);
                let res = ws.choose_best_weapon_for_target(&sim, o, victim, crit, command_source(cmd));
                sim.ob_mut(o).weapon_set = ws;
                sim.rec(format!("ret choose {}", res? as i32));
            }
            24 => {
                let o = r.i();
                let v = r.i();
                let at = r.i();
                let cmd = r.i();
                let slot = r.i();
                let res = sim.ob(o).weapon_set.get_able_to_attack_specific_object(&sim, at as AbleToAttackType, o, v, command_source(cmd), slot)?;
                sim.rec(format!("ret able {}", res as i32));
            }
            25 => {
                let o = r.i();
                let v = r.i();
                let has_pos = r.b();
                let pos = r.coord();
                let at = r.i();
                let cmd = r.i();
                let slot = r.i();
                let victim = sim.opt(v);
                let res = sim.ob(o).weapon_set.get_able_to_use_weapon_against_target(&sim, at, o, victim, has_pos.then_some(pos), command_source(cmd), slot)?;
                sim.rec(format!("ret ableuse {}", res as i32));
            }
            26 => {
                let o = r.i();
                let what = r.i();
                let slot = r.i();
                let ty = r.i();
                if what == 0 {
                    let res = sim.ob_mut(o).weapon_set.set_weapon_lock(slot, ty);
                    sim.rec(format!("ret lock {}", res as i32));
                } else {
                    sim.ob_mut(o).weapon_set.release_weapon_lock(ty);
                }
            }
            27 => {
                let o = r.i();
                let now = sim.frame;
                let mut ws = std::mem::take(&mut sim.ob_mut(o).weapon_set);
                let pip = ws.find_ammo_pip_showing_weapon(&sim).map_or(-1, |w| w.weapon_slot());
                let wp = ws.find_waypoint_following_capable_weapon(&sim).map_or(-1, |w| w.weapon_slot());
                let line = format!(
                    "ret setq oom={} any={} anydmg={} single={},{} locked={} slot={} shared={} pip={} wp={} most={} cmd={},{},{} cur={}",
                    ws.is_out_of_ammo(now) as i32,
                    ws.has_any_weapon() as i32,
                    ws.has_any_damage_weapon() as i32,
                    ws.has_single_damage_type(DAMAGE_EXPLOSION) as i32,
                    ws.has_weapon_to_deal_damage_type(DAMAGE_SMALL_ARMS) as i32,
                    ws.is_cur_weapon_locked() as i32,
                    ws.get_cur_weapon_slot(),
                    ws.is_shared_reload_time(&sim) as i32,
                    pip,
                    wp,
                    ws.get_most_percent_ready_to_fire_any_weapon(now),
                    ws.get_nth_command_source_mask(&sim, 0),
                    ws.get_nth_command_source_mask(&sim, 1),
                    ws.get_nth_command_source_mask(&sim, 2),
                    ws.get_cur_weapon().map_or(-1, |w| w.weapon_slot())
                );
                sim.ob_mut(o).weapon_set = ws;
                sim.rec(line);
            }
            28 => {
                let o = r.i();
                let mut ws = std::mem::take(&mut sim.ob_mut(o).weapon_set);
                let mut save = XferSave::new();
                save.open(b"s").unwrap();
                ws.xfer(&mut sim, &mut save).map_err(|_| Excluded("save of a weapon set failed"))?;
                save.close().unwrap();
                sim.ob_mut(o).weapon_set = ws;
                let bytes = save.take_bytes();
                sim.rec(format!("ret setxfer-save len={} hash={:08x}", bytes.len(), hash_bytes(&bytes)));
                let mut copy = WeaponSet::new();
                let mut load = XferLoad::new(bytes);
                load.open(b"s").unwrap();
                let status = xfer_status(copy.xfer(&mut sim, &mut load))?;
                let _ = load.close();
                sim.rec(format!(
                    "ret setxfer-load {} cur={} lock={} mask={} anti={} dw={}",
                    status,
                    copy.get_cur_weapon_slot(),
                    copy.cur_weapon_locked_status(),
                    copy.filled_weapon_slot_mask(),
                    copy.total_anti_mask(),
                    copy.has_any_damage_weapon() as i32
                ));
                for k in 0..3 {
                    if let Some(w) = copy.get_weapon_in_weapon_slot(k) {
                        sim.rec(format!("ret setxfer-w{} {}", k, dump_weapon(w)));
                    }
                }
            }
            29 => {
                let o = r.i();
                let m = r.u();
                sim.ob_mut(o).ws_flags = weapon_set_flags_of(m);
            }
            30 => {
                let o = r.i();
                sim.ob_mut(o).weapon_set.clear_leech_range_mode_for_all_weapons();
            }
            31 => {
                let slot = r.i();
                let cond = r.i();
                let f = WeaponSet::get_model_condition_for_weapon_slot(slot, cond);
                let mut s = String::new();
                for k in 0..rts_lockstep_engine::common::model_state::MODELCONDITION_COUNT {
                    if f.test(k) {
                        s.push_str(&format!("{k},"));
                    }
                }
                sim.rec(format!("ret modelcond {s}"));
            }
            32 => {
                let o = r.i();
                sim.ob_mut(o).bonus_cond = r.u();
            }
            33 => {
                let o = r.i();
                let bit = r.i();
                let on = r.b();
                let s = &mut sim.ob_mut(o).status;
                if on {
                    *s |= 1u64 << bit;
                } else {
                    *s &= !(1u64 << bit);
                }
            }
            34 => {
                let o = r.i();
                let p = r.coord();
                let ob = sim.ob_mut(o);
                ob.pos = p;
                ob.xform.set_translation(&Vector3::new(p.x, p.y, p.z));
            }
            35 => {
                let w = r.u() as usize;
                reg[w] = None;
            }
            36 => {
                let o = r.i();
                let slot = r.i() as usize;
                let p = r.f();
                let a = r.b();
                if o != 0 {
                    let taken = sim.ob_mut(o).weapon_set.slots_mut()[slot].take();
                    if let Some(mut w) = taken {
                        w.set_clip_percent_full(&sim, p, a);
                        sim.ob_mut(o).weapon_set.slots_mut()[slot] = Some(w);
                    }
                }
            }
            37 => {
                let o = r.i();
                let dt = r.i();
                let slot = r.i();
                let cur = ow::get_current_weapon(&sim, o);
                let wp = ow::find_waypoint_following_capable_weapon(&mut sim, o);
                let pip = ow::get_ammo_pip_showing_info(&sim, o);
                // (the const getCurrentWeapon is the same call)
                sim.rec(format!(
                    "ret objq any={} dmg={} ready={} dtype={} range={:08x} oom={} locked={} shared={} mask={:08x} lastshot={} cur={},{} ccur={},{} wp={} pip={},{},{}",
                    ow::has_any_weapon(&sim, o) as i32,
                    ow::has_any_damage_weapon(&sim, o) as i32,
                    ow::get_most_percent_ready_to_fire_any_weapon(&sim, o),
                    ow::has_weapon_to_deal_damage_type(&sim, o, dt) as i32,
                    ow::get_largest_weapon_range(&sim, o).to_bits(),
                    ow::is_out_of_ammo(&sim, o) as i32,
                    ow::is_cur_weapon_locked(&sim, o) as i32,
                    ow::is_reload_time_shared(&sim, o) as i32,
                    ow::get_weapon_in_weapon_slot_command_source_mask(&sim, o, slot as usize),
                    ow::get_last_shot_fired_frame(&sim, o),
                    cur.has_weapon as i32,
                    cur.slot.unwrap_or(-1),
                    cur.has_weapon as i32,
                    cur.slot.unwrap_or(-1),
                    wp.unwrap_or(-1),
                    pip.is_some() as i32,
                    pip.map_or(-7, |p| p.0),
                    pip.map_or(-7, |p| p.1)
                ));
            }
            38 => {
                let o = r.i();
                let now = r.b();
                ow::reload_all_ammo(&mut sim, o, now);
            }
            39 => {
                let o = r.i();
                ow::set_firing_condition_for_current_weapon(&mut sim, o);
            }
            40 => {
                let o = r.i();
                ow::adjust_model_condition_for_weapon_status(&mut sim, o);
            }
            41 => {
                let o = r.i();
                let v = r.i();
                let pos = r.coord();
                let has_pos = r.b();
                let at_pos = r.b();
                let at = if at_pos { FireAt::Position(has_pos.then_some(pos)) } else { FireAt::Object(sim.opt(v)) };
                ow::fire_current_weapon(&mut sim, o, at)?;
            }
            42 => {
                let o = r.i();
                let v = r.i();
                let victim = sim.opt(v);
                ow::pre_fire_current_weapon(&mut sim, o, victim)?;
            }
            43 => {
                let o = r.i();
                let v = r.i();
                let has_pos = r.b();
                let pos = r.coord();
                let at = r.i();
                let cmd = r.i();
                let slot = r.i();
                let crit = r.i();
                let kind = r.u();
                let victim = sim.opt(v);
                match kind {
                    0 => {
                        let res = ow::get_able_to_attack_specific_object(&sim, o, at as AbleToAttackType, v, command_source(cmd), slot)?;
                        sim.rec(format!("ret oable {}", res as i32));
                    }
                    1 => {
                        let res = ow::get_able_to_use_weapon_against_target(&sim, o, at, victim, has_pos.then_some(pos), command_source(cmd), slot)?;
                        sim.rec(format!("ret oableuse {}", res as i32));
                    }
                    _ => {
                        let res = ow::choose_best_weapon_for_target(&mut sim, o, victim, crit, command_source(cmd))?;
                        sim.rec(format!("ret ochoose {}", res as i32));
                    }
                }
            }
            44 => {
                let o = r.i();
                let what = r.i();
                let slot = r.i();
                let ty = r.i();
                match what {
                    0 => {
                        let res = ow::set_weapon_lock(&mut sim, o, slot, ty);
                        sim.rec(format!("ret olock {}", res as i32));
                    }
                    1 => ow::release_weapon_lock(&mut sim, o, ty),
                    _ => ow::clear_leech_range_mode_for_all_weapons(&mut sim, o),
                }
            }
            45 => {
                let o = r.i();
                let slot = r.i();
                let victim = r.i();
                let weapon = sim.ob(o).weapon_set.get_weapon_in_weapon_slot(slot).cloned();
                if let Some(w) = weapon {
                    ow::notify_firing_tracker_shot_fired(&mut sim, o, &w, victim);
                }
            }
            46 => {
                let o = r.i();
                let want_pos = r.b();
                let want_tr = r.b();
                let kind = r.u();
                let turret = r.i();
                let name_idx = r.i();
                let max_bones = r.i();
                let to_world = r.b();
                const NAMES: [&[u8]; 4] = [b"Bone", b"Muzzle", b"Turret01", b"FXBone"];
                let name = NAMES[(name_idx & 3) as usize];
                let fmt_pos = |p: Option<Coord3D>| {
                    let p = p.unwrap_or(Coord3D { x: -99.0, y: -99.0, z: -99.0 });
                    format!("{},{},{}", fb(p.x), fb(p.y), fb(p.z))
                };
                let untouched = {
                    let mut t = Matrix3D::identity();
                    t.set_translation(&Vector3::new(-77.0, -77.0, -77.0));
                    t
                };
                if kind < 2 {
                    let q = if kind == 0 {
                        ow::get_single_logical_bone_position(&sim, o, name, want_pos, want_tr)
                    } else {
                        ow::get_single_logical_bone_position_on_turret(&sim, o, turret, name, want_pos, want_tr)?
                    };
                    sim.rec(format!("ret bone{} found={} pos={} tr={}", kind, q.found as i32, fmt_pos(q.position), mtx_str(&q.transform.unwrap_or(untouched))));
                } else {
                    let bones = ow::get_multi_logical_bone_position(&sim, o, name, max_bones, want_pos, want_tr, to_world);
                    let mut s = String::new();
                    for i in 0..4 {
                        let (p, t) = bones.get(i).copied().unwrap_or((None, None));
                        s.push_str(&format!("[{} {}]", fmt_pos(p), mtx_str(&t.unwrap_or(untouched))));
                    }
                    sim.rec(format!("ret bonemulti n={} {}", bones.len(), s));
                }
            }
            47 => {
                let o = r.i();
                let up = r.u();
                let name = ow::get_command_set_string(&sim, o);
                sim.rec(format!("ret cmdset {}", String::from_utf8_lossy(&name)));
                let res = ow::can_produce_upgrade(&sim, o, up)?;
                sim.rec(format!("ret canproduce {}", res as i32));
            }
            _ => panic!("unknown op {op}"),
        }
        dump_snapshot(&sim, &reg);
    }
    let lines = sim.lines.take();
    Ok(lines)
}

fn mtx_str(m: &Matrix3D) -> String {
    m.to_array().iter().map(|&x| fb(x)).collect::<Vec<_>>().join(",")
}

/// The drawable, the AI and the control bar as `fire_driver.h` stands them in (the formulas are the same operations).
impl ObjectBoneHost for Sim {
    fn pristine_bone_positions(&self, _o: ObjectId, prefix: &[u8], start_index: i32, max_bones: i32) -> Vec<(Coord3D, Matrix3D)> {
        let mut h: u32 = 0;
        for &c in prefix {
            h = h.wrapping_mul(31).wrapping_add(u32::from(c));
        }
        let mut count = (h.wrapping_add(start_index as u32) % 4) as i32;
        if count > max_bones {
            count = max_bones;
        }
        (0..count)
            .map(|i| {
                let p = Coord3D { x: (h % 97) as f32 + i as f32, y: (i * 2) as f32, z: start_index as f32 + 0.5 };
                let mut t = Matrix3D::identity();
                t.rotate_z(0.25 * (i + 1) as f32);
                t.set_translation(&Vector3::new(p.x, p.y, p.z));
                (p, t)
            })
            .collect()
    }
    fn convert_bone_pos_to_world_pos(&self, o: ObjectId, bone_pos: Option<&Coord3D>, bone_transform: Option<&Matrix3D>) -> (Option<Coord3D>, Option<Matrix3D>) {
        let s = self.ob(o);
        let mut p = Coord3D { x: 0.0, y: 0.0, z: 0.0 };
        if let Some(b) = bone_pos {
            p = Coord3D { x: b.x + s.pos.x, y: b.y + s.pos.y, z: b.z + s.pos.z };
        }
        let mut t = Matrix3D::identity();
        if let Some(b) = bone_transform {
            t.mul(&s.xform, b);
        }
        (bone_pos.map(|_| p), bone_transform.map(|_| t))
    }
    fn has_ai(&self, o: ObjectId) -> bool {
        self.ob(o).has_ai
    }
    fn ai_turret_rot_and_pitch(&self, o: ObjectId, turret: i32) -> Option<(f32, f32)> {
        (0..=2).contains(&turret).then(|| (self.ob(o).turret_angle[turret as usize], self.ob(o).turret_pitch[turret as usize]))
    }
    fn command_set_upgrades(&self, name: &[u8]) -> Option<Vec<Option<u32>>> {
        let rest = name.strip_prefix(b"Cmd")?;
        let k: i32 = rest.iter().take_while(|c| c.is_ascii_digit()).fold(0i32, |a, &c| a * 10 + i32::from(c - b'0'));
        Some(
            (0..18)
                .map(|i| {
                    if (k + i) % 3 != 0 {
                        let up = (k * 7 + i * 3) % 5;
                        (up != 0).then_some(up as u32)
                    } else {
                        None
                    }
                })
                .collect(),
        )
    }
    fn command_set_string_override(&self, o: ObjectId) -> Vec<u8> {
        match self.ob(o).cmd_override {
            c if c < 0 => Vec::new(),
            c if c >= 100 => b"Other".to_vec(),
            c => format!("Cmd{c}").into_bytes(),
        }
    }
    fn template_command_set_string(&self, o: ObjectId) -> Vec<u8> {
        let k = self.ob(o).tmpl;
        if k % 4 == 3 {
            Vec::new()
        } else {
            format!("Cmd{k}").into_bytes()
        }
    }
}

fn mc_flags_str(f: &ModelConditionFlags) -> String {
    (0..MODELCONDITION_COUNT).filter(|&b| f.test(b)).map(|b| format!("{b},")).collect()
}

/// The weapon half of `Object` on the scenario host (`Drawable`, `FiringTracker` and the rest write the trace lines
/// that `fire_driver.h` writes).
impl ObjectWeaponHost for Sim {
    fn weapon_set(&self, o: ObjectId) -> &WeaponSet {
        &self.ob(o).weapon_set
    }
    fn take_weapon_set(&mut self, o: ObjectId) -> WeaponSet {
        std::mem::take(&mut self.ob_mut(o).weapon_set)
    }
    fn put_weapon_set(&mut self, o: ObjectId, set: WeaponSet) {
        self.ob_mut(o).weapon_set = set;
    }
    fn weapon_set_mut(&mut self, o: ObjectId) -> &mut WeaponSet {
        &mut self.ob_mut(o).weapon_set
    }
    fn take_weapon(&mut self, o: ObjectId, slot: i32) -> Option<Weapon> {
        self.ob_mut(o).weapon_set.slots_mut()[slot as usize].take()
    }
    fn put_weapon(&mut self, o: ObjectId, slot: i32, weapon: Weapon) {
        self.ob_mut(o).weapon_set.slots_mut()[slot as usize] = Some(weapon);
    }
    fn last_weapon_condition(&self, o: ObjectId, slot: usize) -> i8 {
        self.ob(o).last_cond[slot]
    }
    fn set_last_weapon_condition(&mut self, o: ObjectId, slot: usize, v: i8) {
        self.ob_mut(o).last_cond[slot] = v;
    }
    fn firing_tracker_shot_fired(&mut self, o: ObjectId, weapon: &Weapon, victim: ObjectId) -> bool {
        if !self.ob(o).has_tracker {
            return false;
        }
        self.rec(format!(
            "tracker o={} slot={} victim={} next={} last={}",
            o,
            weapon.weapon_slot(),
            victim,
            weapon.possible_next_shot_frame(),
            weapon.last_shot_frame()
        ));
        true
    }
    fn friend_set_undetected_defector(&mut self, o: ObjectId, undetected: bool) {
        self.rec(format!("undetected o={} v={}", o, undetected as i32));
    }
    fn drawable_update_clip_status(&mut self, o: ObjectId, ammo_in_clip: u32, max_clip_size: i32, slot: i32) {
        self.rec(format!("clipstatus o={} ammo={} clip={} slot={}", o, ammo_in_clip, max_clip_size as u32, slot));
    }
    fn drawable_clear_and_set_model_condition_flags(&mut self, o: ObjectId, clear: &ModelConditionFlags, set: &ModelConditionFlags) {
        self.rec(format!("mcflags o={} clear=[{}] set=[{}]", o, mc_flags_str(clear), mc_flags_str(set)));
    }
    fn drawable_set_animation_loop_duration(&mut self, o: ObjectId, frames: u32) {
        self.rec(format!("animloop o={} frames={}", o, frames));
    }
}

fn command_source(c: i32) -> CommandSourceType {
    match c {
        0 => CommandSourceType::FromPlayer,
        1 => CommandSourceType::FromScript,
        2 => CommandSourceType::FromAi,
        3 => CommandSourceType::FromDozer,
        _ => CommandSourceType::DefaultSwitchWeapon,
    }
}

fn hash_bytes(v: &[u8]) -> u32 {
    let mut h: u32 = 2166136261;
    for &b in v {
        h = (h ^ u32::from(b)).wrapping_mul(16777619);
    }
    h
}

// ---------------------------------------------------------------------------------------------------------------------
// The generator
// ---------------------------------------------------------------------------------------------------------------------

use rts_lockstep_engine::common::kind_of::{
    KINDOF_BALLISTIC_MISSILE, KINDOF_BOOBY_TRAP, KINDOF_BRIDGE, KINDOF_DEMOTRAP, KINDOF_DISGUISER, KINDOF_FS_AIRFIELD, KINDOF_HERO, KINDOF_IMMOBILE,
    KINDOF_INFANTRY, KINDOF_MINE, KINDOF_PARACHUTE, KINDOF_PROJECTILE, KINDOF_SHRUBBERY, KINDOF_SMALL_MISSILE, KINDOF_SPAWNS_ARE_THE_WEAPONS,
    KINDOF_STRUCTURE, KINDOF_UNATTACKABLE, KINDOF_VEHICLE,
};
use rts_lockstep_engine::common::object_status_types::{
    OBJECT_STATUS_AIRBORNE_TARGET, OBJECT_STATUS_BURNED, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_IGNORING_STEALTH,
    OBJECT_STATUS_IS_AIMING_WEAPON, OBJECT_STATUS_IS_ATTACKING, OBJECT_STATUS_IS_FIRING_WEAPON, OBJECT_STATUS_MASKED, OBJECT_STATUS_NO_ATTACK_FROM_AI,
    OBJECT_STATUS_STEALTHED,
};

const KIND_POOL: [KindOfType; 18] = [
    KINDOF_INFANTRY,
    KINDOF_VEHICLE,
    KINDOF_STRUCTURE,
    KINDOF_PROJECTILE,
    KINDOF_SMALL_MISSILE,
    KINDOF_BALLISTIC_MISSILE,
    KINDOF_MINE,
    KINDOF_BOOBY_TRAP,
    KINDOF_DEMOTRAP,
    KINDOF_SHRUBBERY,
    KINDOF_BRIDGE,
    KINDOF_UNATTACKABLE,
    KINDOF_DISGUISER,
    KINDOF_HERO,
    KINDOF_IMMOBILE,
    KINDOF_SPAWNS_ARE_THE_WEAPONS,
    KINDOF_FS_AIRFIELD,
    KINDOF_PARACHUTE,
];
const STATUS_POOL: [ObjectStatusTypes; 11] = [
    OBJECT_STATUS_AIRBORNE_TARGET,
    OBJECT_STATUS_BURNED,
    OBJECT_STATUS_STEALTHED,
    OBJECT_STATUS_DETECTED,
    OBJECT_STATUS_MASKED,
    OBJECT_STATUS_NO_ATTACK_FROM_AI,
    OBJECT_STATUS_IGNORING_STEALTH,
    OBJECT_STATUS_DISGUISED,
    OBJECT_STATUS_IS_ATTACKING,
    OBJECT_STATUS_IS_AIMING_WEAPON,
    OBJECT_STATUS_IS_FIRING_WEAPON,
];

fn chance(g: &mut Gen, percent: u32) -> bool {
    g.below(100) < percent
}

fn pick<T: Copy>(g: &mut Gen, xs: &[T]) -> T {
    xs[g.below(xs.len() as u32) as usize]
}

fn gen_coord(g: &mut Gen, spread: f32) -> [f32; 3] {
    [g.uniform(0.0, spread), g.uniform(0.0, spread), if chance(g, 30) { g.uniform(0.0, 40.0) } else { 0.0 }]
}

fn gen_bonus_entries(g: &mut Gen, w: &mut W) {
    let n = g.below(5);
    w.u(n);
    for _ in 0..n {
        w.u(g.below(27));
        w.u(g.below(5));
        w.f(pick(g, &[0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0]));
    }
}

/// The facts about the generated world that the operations need.
struct World {
    n_tpl: usize,
    n_obj: usize,
    /// the objects that can fire: a controlling player and a drawable
    fire: Vec<i32>,
    spread: f32,
}

fn gen_template(g: &mut Gen, w: &mut W, k: usize, n_tpl: usize, n_things: usize) -> bool {
    let radius_roll = g.below(10);
    let pr = if radius_roll < 4 { 0.0 } else { g.uniform(5.0, 60.0) };
    let sr = if pr == 0.0 || chance(g, 50) { if chance(g, 30) { g.uniform(5.0, 90.0) } else { 0.0 } } else { pr + g.uniform(0.0, 40.0) };
    w.f(if chance(g, 10) { 0.0 } else { g.uniform(5.0, 200.0) });
    w.f(pr);
    w.f(if chance(g, 30) { 0.0 } else { g.uniform(1.0, 100.0) });
    w.f(sr);
    let shock = chance(g, 25);
    w.f(if shock { g.uniform(5.0, 60.0) } else { 0.0 });
    w.f(g.uniform(5.0, 50.0));
    w.f(g.uniform(0.0, 1.0));
    w.f(if chance(g, 10) { g.uniform(1.0, 8.0) } else { g.uniform(20.0, 400.0) }); // attack range (some are contact weapons)
    w.f(if chance(g, 30) { g.uniform(5.0, 120.0) } else { 0.0 });
    w.f(if chance(g, 20) { g.uniform(40.0, 250.0) } else { 0.0 });
    w.f(g.uniform(0.0, 1.0));
    let scatter = chance(g, 30);
    w.f(if scatter { g.uniform(3.0, 40.0) } else { 0.0 });
    w.f(g.uniform(0.5, 3.0));
    let n_sc = if chance(g, 30) { g.below(5) } else { 0 };
    w.u(n_sc);
    for _ in 0..n_sc {
        w.f(g.uniform(-10.0, 10.0));
        w.f(g.uniform(-10.0, 10.0));
    }
    // damage type: the ones the code treats specially more often
    let dt = if chance(g, 55) { pick(g, &[13, 20, 15, 14, 8, 6, 22, 16, 11, 5]) } else { g.below(38) as i32 };
    w.i(dt);
    w.i(if chance(g, 30) { 3 } else { g.below(21) as i32 });
    w.f(match g.below(5) {
        0 => 0.0,
        1 => 999999.0,
        _ => g.uniform(20.0, 300.0),
    });
    w.f(if chance(g, 25) { g.uniform(0.05, 0.5) } else { 0.0 });
    let narrow = chance(g, 25);
    // a limit on one side only is its own case (`minTargetPitch > -PI || maxTargetPitch < PI`)
    let side = g.below(3);
    w.f(if narrow && side != 2 { g.uniform(-0.6, 0.0) } else { -PI });
    w.f(if narrow && side != 1 { g.uniform(0.0, 0.8) } else { PI });
    w.f(if chance(g, 20) { g.uniform(0.3, 3.0) } else { PI });
    let projectile = chance(g, 40);
    w.u(if projectile { 1 + g.below(n_things as u32) } else { 0 });
    w.u(if chance(g, 15) { 1 + g.below(2) } else { 0 }); // laser
    w.u(if chance(g, 20) { 1 + g.below(2) } else { 0 }); // stream
    for _ in 0..16 {
        w.u(if chance(g, 25) { 1 + g.below(9) } else { 0 }); // fire OCLs, detonation OCLs, fire FX, detonation FX
    }
    for _ in 0..4 {
        w.u(if chance(g, 30) { 1 + g.below(9) } else { 0 }); // exhausts
    }
    let extra = chance(g, 35);
    w.b(extra);
    if extra {
        gen_bonus_entries(g, w);
    }
    let clip = if chance(g, 40) { 0 } else { 1 + g.below(6) as i32 };
    w.i(clip);
    w.i(if chance(g, 30) { 0 } else { g.below(90) as i32 });
    let (dmin, dmax) = if chance(g, 40) {
        let d = g.below(20) as i32;
        (d, d)
    } else {
        let a = g.below(15) as i32;
        (a, a + 1 + g.below(15) as i32)
    };
    w.i(dmin);
    w.i(dmax);
    w.i(if chance(g, 40) { i32::MAX } else { g.below(6) as i32 });
    w.i(if chance(g, 40) { i32::MAX } else { g.below(12) as i32 });
    w.u(g.below(60));
    w.u(if chance(g, 30) { g.below(100) } else { 0 });
    w.i(1 + g.below(3) as i32);
    let mut anti = g.below(256);
    if chance(g, 70) {
        anti |= 2;
    }
    w.u(anti);
    let mut affects = g.below(128);
    if chance(g, 60) {
        affects |= 0x0e;
    }
    w.u(affects);
    w.u(g.below(512));
    w.b(chance(g, 10));
    w.i(g.below(3) as i32);
    w.i(g.below(3) as i32);
    let hist = chance(g, 40) && k + 1 < n_tpl;
    // short windows, so that a frame lands exactly on the edge of the historic-damage time window now and then
    w.u(if hist { g.below(24) } else { g.below(30) });
    w.f(g.uniform(5.0, 90.0));
    w.i(if hist { 1 + g.below(4) as i32 } else { 0 });
    w.i(if hist { (k + 1 + g.below((n_tpl - k - 1) as u32) as usize) as i32 } else { -1 });
    w.b(chance(g, 12)); // leech
    w.b(chance(g, 15));
    w.b(chance(g, 15));
    w.b(chance(g, 15));
    w.b(chance(g, 15));
    w.i(if chance(g, 40) { g.below(30) as i32 } else { 0 });
    w.f(g.uniform(0.0, 50.0));
    w.f(if chance(g, 20) { g.uniform(1.0, 20.0) } else { 0.0 });
    w.i(g.below(45) as i32);
    w.u(if chance(g, 25) { g.below(15) } else { 0 });
    projectile
}

/// One object record. `kind_bias`: kinds to include in addition to the random ones.
#[allow(clippy::too_many_arguments)]
fn gen_object(
    g: &mut Gen,
    n_things: usize,
    n_players: usize,
    n_obj: usize,
    index: usize,
    force_drawable: bool,
    player: i32,
    contains: &[bool],
    container: i32,
    spread: f32,
    projectile_proto: bool,
) -> Vec<u32> {
    let mut w = W::default();
    let id = index as i32 + 1;
    w.u(1 + g.below(n_things as u32));
    w.i(player);
    let pos = gen_coord(g, spread);
    w.c(pos);
    let orientation = g.uniform(-PI, PI);
    w.f(orientation);
    w.u(g.below(3));
    w.b(chance(g, 20));
    w.f(g.uniform(2.0, 30.0));
    w.f(g.uniform(2.0, 25.0));
    w.f(g.uniform(2.0, 25.0));
    let mut kinds: Vec<KindOfType> = KIND_POOL.iter().copied().filter(|_| chance(g, 12)).collect();
    if projectile_proto {
        if chance(g, 60) {
            kinds.push(KINDOF_PROJECTILE);
        }
        if chance(g, 40) {
            kinds.push(KINDOF_SMALL_MISSILE);
        }
    }
    w.u(kinds.len() as u32);
    for k in kinds {
        w.i(k);
    }
    let mut status: u64 = 0;
    for s in STATUS_POOL {
        if chance(g, 9) {
            status |= 1u64 << s;
        }
    }
    w.u(status as u32);
    w.u((status >> 32) as u32);
    w.u(if chance(g, 35) { 0x10 } else { 0 } | if chance(g, 20) { 0x01 } else { 0 });
    w.i(g.below(4) as i32);
    w.b(chance(g, 30));
    w.u(if chance(g, 40) { g.below(1 << 27) } else { 0 });
    w.i(container);
    w.i(if chance(g, 15) { 1 + g.below(n_obj as u32) as i32 } else { 0 });
    w.b(chance(g, 6));
    w.b(chance(g, 5));
    w.b(chance(g, 85));
    w.i(if chance(g, 90) { 1 } else if chance(g, 50) { 15 } else { 2 });
    w.b(chance(g, 25));
    w.b(chance(g, 25));
    w.i(g.below(4) as i32);
    w.f(if chance(g, 30) { 0.0 } else { g.uniform(1.0, 80.0) });
    let ws_mask = if chance(g, 45) { 0 } else { (1u32 << g.below(17)) | if chance(g, 30) { 1u32 << g.below(17) } else { 0 } };
    w.u(ws_mask);
    let has_drawable = force_drawable || chance(g, 90);
    w.b(has_drawable);
    for _ in 0..3 {
        w.i(1 + g.below(4) as i32);
    }
    w.b(chance(g, 50));
    let has_launch = chance(g, 70);
    w.b(has_launch);
    w.f(g.uniform(-1.0, 1.0));
    w.c([g.uniform(-3.0, 3.0), g.uniform(-3.0, 3.0), g.uniform(0.0, 8.0)]);
    w.c([g.uniform(-2.0, 2.0), g.uniform(-2.0, 2.0), g.uniform(0.0, 4.0)]);
    w.c([g.uniform(-2.0, 2.0), g.uniform(-2.0, 2.0), g.uniform(0.0, 4.0)]);
    w.b(chance(g, 50)); // laser update
    let has_ai = chance(g, 70);
    w.b(has_ai);
    w.b(chance(g, 15)); // sneaky
    w.c([g.uniform(-4.0, 4.0), g.uniform(-4.0, 4.0), g.uniform(0.0, 3.0)]);
    w.i(if chance(g, 25) { 6 } else { g.below(8) as i32 });
    w.b(chance(g, 20));
    w.b(chance(g, 30));
    for _ in 0..3 {
        w.i(if chance(g, 40) { g.below(2) as i32 } else { -1 });
    }
    for _ in 0..3 {
        w.f(g.uniform(-3.0, 3.0));
    }
    for _ in 0..3 {
        w.f(g.uniform(-0.5, 0.5));
    }
    for _ in 0..3 {
        w.b(chance(g, 25));
    }
    let has_contain = contains[index];
    w.b(has_contain);
    w.i(g.below(4) as i32);
    w.b(chance(g, 40));
    w.b(chance(g, 20));
    w.b(chance(g, 30));
    w.u(if chance(g, 50) { g.below(1 << 27) } else { 0 });
    let mut enclosing = Vec::new();
    for o in 1..=n_obj as i32 {
        if chance(g, 25) {
            enclosing.push(o);
        }
    }
    w.u(enclosing.len() as u32);
    for e in enclosing {
        w.i(e);
    }
    w.b(chance(g, 50));
    w.b(chance(g, 50));
    let has_items = has_contain && chance(g, 70);
    w.b(has_items);
    let mut items = Vec::new();
    if has_items {
        for o in id + 1..=n_obj as i32 {
            if chance(g, 40) {
                items.push(o);
            }
        }
    }
    w.u(items.len() as u32);
    for i in items {
        w.i(i);
    }
    w.i(if chance(g, 30) { g.below(n_players as u32) as i32 } else { -2 });
    w.b(chance(g, 15)); // stealth
    w.b(chance(g, 50)); // disguised
    w.i(if chance(g, 80) { g.below(n_players as u32 + 1) as i32 } else { -1 });
    w.b(chance(g, 15)); // spawn
    w.i(g.below(4) as i32);
    w.b(chance(g, 12)); // land mine
    w.b(if projectile_proto { chance(g, 75) } else { chance(g, 15) }); // projectile update
    w.b(chance(g, 12)); // parking
    w.b(chance(g, 15)); // special power completion die
    w.b(chance(g, 20)); // projectile stream
    w.b(chance(g, 25)); // assisted targeting
    w.b(chance(g, 60));
    w.b(chance(g, 15)); // countermeasures
    w.b(chance(g, 50)); // physics
    w.i(0);
    let mut reserved = Vec::new();
    for o in 1..=n_obj as i32 {
        if chance(g, 15) {
            reserved.push(o);
        }
    }
    w.u(reserved.len() as u32);
    for e in reserved {
        w.i(e);
    }
    let mut collides = Vec::new();
    for o in 1..=n_obj as i32 {
        if chance(g, 25) {
            collides.push(o);
        }
    }
    w.u(collides.len() as u32);
    for e in collides {
        w.i(e);
    }
    w.i(pick(g, &[-1, -1, 0, 1, 2, 3, 4, 5, 6, 7, 100])); // a command set override
    w.b(chance(g, 70)); // a firing tracker
    for _ in 0..3 {
        w.i(pick(g, &[-1, 0, 1, 2, 3, 4]));
    }
    let _ = has_ai;
    w.0
}

fn gen_header(g: &mut Gen, w: &mut W) -> World {
    w.u(g.u32()); // seed
    w.u(if chance(g, 6) { u32::MAX - g.below(50) } else { g.below(3000) }); // frame
    w.u(g.below(300)); // historic damage limit
    w.c([0.0, 0.0, 0.0]);
    let spread = pick(g, &[120.0f32, 300.0, 600.0]);
    w.c([spread * 2.0, spread * 2.0, 100.0]);
    let gb = chance(g, 50);
    w.b(gb);
    if gb {
        gen_bonus_entries(g, w);
    }
    let n_tpl = 3 + g.below(6) as usize;
    let n_things = 1 + g.below(3) as usize;
    let n_players = 2 + g.below(3) as usize;
    let n_obj = 5 + g.below(8) as usize;
    w.u(n_tpl as u32);
    let mut has_projectile = Vec::new();
    for k in 0..n_tpl {
        has_projectile.push(gen_template(g, w, k, n_tpl, n_things));
    }
    w.u(n_players as u32);
    for p in 0..n_players {
        w.i(g.below(3) as i32);
        w.u(1 << p);
        for q in 0..4 {
            w.i(if p == q { 2 } else { g.below(3) as i32 });
        }
    }
    let contains: Vec<bool> = (0..n_obj).map(|_| chance(g, 30)).collect();
    w.u(n_things as u32);
    for t in 0..n_things {
        w.i(g.below(2) as i32);
        w.f(if chance(g, 25) { 5.0 } else { 0.0 });
        // the prototype dynamic objects copy: it has a drawable
        let contains_none = vec![false; n_obj];
        let rec = gen_object(g, n_things, n_players, n_obj, t, true, -1, &contains_none, 0, spread, true);
        w.u(rec.len() as u32);
        w.0.extend(rec);
    }
    let n_sets = n_things + g.below(3) as usize;
    w.u(n_sets as u32);
    for s in 0..n_sets {
        let tid = if s < n_things { s + 1 } else { 1 + g.below(n_things as u32) as usize };
        w.i(tid as i32);
        w.u(if s < n_things { 0 } else { (1u32 << g.below(17)) | if chance(g, 40) { 1u32 << g.below(17) } else { 0 } });
        for _ in 0..3 {
            w.i(if chance(g, 55) { g.below(n_tpl as u32) as i32 } else { -1 });
        }
        for _ in 0..3 {
            w.u(if chance(g, 60) { 0xffff_ffff } else { g.below(32) });
        }
        for _ in 0..3 {
            let n = if chance(g, 30) { 1 + g.below(2) } else { 0 };
            w.u(n);
            for _ in 0..n {
                w.i(pick(g, &KIND_POOL));
            }
        }
        w.b(chance(g, 30));
        w.b(chance(g, 30));
    }
    w.u(n_obj as u32);
    let mut fire = Vec::new();
    for i in 0..n_obj {
        let player = if i < 3 || chance(g, 85) { g.below(n_players as u32) as i32 } else { -1 };
        let container = {
            let cands: Vec<usize> = (0..n_obj).filter(|&j| j != i && contains[j]).collect();
            if !cands.is_empty() && chance(g, 20) {
                cands[g.below(cands.len() as u32) as usize] as i32 + 1
            } else {
                0
            }
        };
        let rec = gen_object(g, n_things, n_players, n_obj, i, i < 3, player, &contains, container, spread, false);
        // can it fire: a controlling player and a drawable (word 1 is the player; the drawable flag is found by the caller of
        // gen_object, so recompute it from the record: see `is_fire_source`)
        if player >= 0 && is_drawable(&rec) {
            fire.push(i as i32 + 1);
        }
        w.u(rec.len() as u32);
        w.0.extend(rec);
    }
    World { n_tpl, n_obj, fire, spread }
}

/// Whether an object record has a drawable (parse the record with the reader).
fn is_drawable(rec: &[u32]) -> bool {
    read_object(&mut Reader { w: rec, p: 0 }, 1 << 20).has_drawable
}

fn gen_bonus(g: &mut Gen, w: &mut W) {
    for _ in 0..WEAPON_BONUS_FIELD_COUNT {
        w.f(pick(g, &[1.0, 1.0, 1.0, 0.5, 1.25, 1.5, 2.0]));
    }
}

fn gen_ops(g: &mut Gen, world: &World, w: &mut W) {
    let n_ops = 25 + g.below(30);
    let mut ops = W::default();
    let mut count = 0u32;
    let mut weapons: Vec<usize> = Vec::new(); // the registry slots alive
    let mut made = 0usize;
    let obj = |g: &mut Gen| 1 + g.below(world.n_obj as u32) as i32;
    let src = |g: &mut Gen| world.fire[g.below(world.fire.len() as u32) as usize];
    let victim_opt = |g: &mut Gen| if chance(g, 15) { 0 } else { 1 + g.below(world.n_obj as u32) as i32 };
    let pos = |g: &mut Gen| gen_coord(g, world.spread);
    macro_rules! op {
        ($($body:tt)*) => {{
            count += 1;
            $($body)*
        }};
    }
    // objects get their weapons first, most of them
    for &o in &world.fire {
        if chance(g, 60) {
            op!(ops.u(20); ops.i(o));
        }
    }
    // two or three weapons, loaded
    for _ in 0..2 + g.below(2) {
        op!(ops.u(1); ops.u(g.below(world.n_tpl as u32)); ops.i(g.below(3) as i32));
        weapons.push(made);
        made += 1;
        if chance(g, 85) {
            let wi = weapons[weapons.len() - 1];
            let s = src(g);
            op!(ops.u(4); ops.u(wi as u32); ops.i(s); ops.u(if chance(g, 70) { 0 } else { 1 }));
        }
    }
    while count < n_ops {
        let wi = weapons[g.below(weapons.len() as u32) as usize];
        if chance(g, 4) {
            // a weapon of an object's set loses its ammo (or part of it)
            let o = src(g);
            let p = pick(g, &[0.0f32, 0.0, 0.5, 1.0]);
            op!(ops.u(36); ops.i(o); ops.i(g.below(3) as i32); ops.f(p); ops.b(chance(g, 80)));
            continue;
        }
        if chance(g, 3) {
            // an object that starts an attack: attacking, pre-fire, then its model condition follows the weapon
            let o = src(g);
            let v = victim_opt(g);
            op!(ops.u(33); ops.i(o); ops.i(OBJECT_STATUS_IS_ATTACKING); ops.b(true));
            op!(ops.u(42); ops.i(o); ops.i(v));
            op!(ops.u(40); ops.i(o));
            op!(ops.u(0); ops.u(g.below(4)));
            op!(ops.u(40); ops.i(o));
            continue;
        }
        if chance(g, 22) {
            // the weapon half of `Object` (Object.cpp's text): questions, firing, the model condition
            let o = src(g);
            match g.below(12) {
                0 => op!(ops.u(37); ops.i(o); ops.i(g.below(38) as i32); ops.i(g.below(3) as i32)),
                1 => op!(ops.u(38); ops.i(o); ops.b(chance(g, 50))),
                2 => op!(ops.u(39); ops.i(o)),
                3 | 4 => op!(ops.u(40); ops.i(o)),
                5 | 6 => {
                    let v = victim_opt(g);
                    let p = pos(g);
                    op!(ops.u(41); ops.i(o); ops.i(v); ops.c(p); ops.b(chance(g, 85)); ops.b(chance(g, 30)))
                }
                7 => {
                    let v = victim_opt(g);
                    op!(ops.u(42); ops.i(o); ops.i(v))
                }
                8 => {
                    let kind = g.below(3);
                    let v = if kind == 0 { obj(g) } else { victim_opt(g) };
                    let p = pos(g);
                    let has_pos = v == 0 || chance(g, 50);
                    let at = pick(g, &[0, 1, 2, 3, 4]);
                    let cmd = g.below(5) as i32;
                    let slot = if chance(g, 70) { -1 } else { g.below(3) as i32 };
                    op!(ops.u(43); ops.i(o); ops.i(v); ops.b(has_pos); ops.c(p); ops.i(at); ops.i(cmd); ops.i(slot); ops.i(g.below(2) as i32); ops.u(kind))
                }
                9 => op!(ops.u(44); ops.i(o); ops.i(g.below(3) as i32); ops.i(g.below(3) as i32); ops.i(1 + g.below(2) as i32)),
                10 => {
                    let v = victim_opt(g);
                    op!(ops.u(45); ops.i(o); ops.i(g.below(3) as i32); ops.i(v))
                }
                _ => op!(ops.u(37); ops.i(o); ops.i(g.below(38) as i32); ops.i(g.below(3) as i32)),
            }
            continue;
        }
        if chance(g, 6) {
            // bone positions, the command set (any object: some have no drawable)
            let o = obj(g);
            if chance(g, 25) {
                op!(ops.u(47); ops.i(o); ops.u(g.below(5)));
            } else {
                op!(ops.u(46); ops.i(o); ops.b(chance(g, 70)); ops.b(chance(g, 70)); ops.u(g.below(3)); ops.i(g.below(4) as i32 - 1); ops.i(g.below(4) as i32); ops.i(g.below(5) as i32 - 1); ops.b(chance(g, 50)));
            }
            continue;
        }
        match g.below(100) {
            0..=5 => op!(ops.u(0); ops.u(if chance(g, 15) { 100 + g.below(500) } else { g.below(40) })),
            6..=7 => {
                op!(ops.u(1); ops.u(g.below(world.n_tpl as u32)); ops.i(g.below(3) as i32));
                weapons.push(made);
                made += 1;
            }
            8 => {
                op!(ops.u(2); ops.u(wi as u32));
                weapons.push(made);
                made += 1;
            }
            9 => {
                let s = weapons[g.below(weapons.len() as u32) as usize];
                op!(ops.u(3); ops.u(wi as u32); ops.u(s as u32))
            }
            10..=14 => {
                let s = src(g);
                op!(ops.u(4); ops.u(wi as u32); ops.i(s); ops.u(g.below(2)))
            }
            15..=18 => op!(ops.u(5); ops.u(wi as u32)),
            19..=20 => {
                let r = g.uniform(0.0, 1.0);
                let p = pick(g, &[0.0f32, 0.3, 0.5, 1.0, r]);
                op!(ops.u(6); ops.u(wi as u32); ops.f(p); ops.b(chance(g, 50)))
            }
            21..=23 => {
                let s = src(g);
                op!(ops.u(7); ops.u(wi as u32); ops.i(s))
            }
            24..=38 => {
                let s = src(g);
                let kind = pick(g, &[0u32, 0, 0, 1, 1, 2, 3, 4]);
                let mut v = obj(g);
                if kind == 0 || kind == 2 {
                    while v == s && world.n_obj > 1 && chance(g, 80) {
                        v = obj(g);
                    }
                }
                let p = pos(g);
                op!(ops.u(8); ops.u(wi as u32); ops.i(s); ops.u(kind); ops.i(v); ops.c(p); ops.u(if chance(g, 30) { g.below(1 << 27) } else { 0 }); ops.b(chance(g, 80)))
            }
            39..=41 => {
                let s = src(g);
                let v = victim_opt(g);
                op!(ops.u(9); ops.u(wi as u32); ops.i(s); ops.i(v))
            }
            42..=47 => {
                let s = src(g);
                let v = victim_opt(g);
                let p = pos(g);
                let goal = pos(g);
                op!(ops.u(10); ops.u(wi as u32); ops.i(s); ops.i(v); ops.c(p); ops.c(goal))
            }
            48..=50 => {
                let s = src(g);
                let t = victim_opt(g);
                let has_pos = chance(g, 50);
                let p = pos(g);
                let ao = if chance(g, 40) { g.uniform(-1.0, 1.0) } else { 0.0 };
                op!(ops.u(11); ops.u(wi as u32); ops.i(s); ops.i(t); ops.b(has_pos); ops.c(p); ops.f(ao))
            }
            51..=53 => {
                let s = src(g);
                // a victim object (a position-only estimate dereferences a null pointer upstream, which the Rust reports)
                let v = if chance(g, 4) { 0 } else { obj(g) };
                let has_pos = v == 0 && chance(g, 50);
                let p = pos(g);
                let t = g.below(world.n_tpl as u32);
                op!(ops.u(12); ops.u(t); ops.i(s); ops.i(v); ops.b(has_pos); ops.c(p); gen_bonus(g, &mut ops))
            }
            54..=56 => {
                let t = g.below(world.n_tpl as u32);
                let (l, p, th, intended) = (victim_opt(g), obj(g), victim_opt(g), if chance(g, 20) { 0 } else { obj(g) });
                op!(ops.u(13); ops.u(t); ops.i(l); ops.i(p); ops.i(th); ops.i(intended))
            }
            57..=61 => {
                let t = g.below(world.n_tpl as u32);
                let s = src(g);
                let v = victim_opt(g);
                let p = pos(g);
                let is_det = chance(g, 30);
                op!(ops.u(14); ops.u(t); ops.i(s); ops.i(v); ops.c(p); gen_bonus(g, &mut ops); ops.b(is_det))
            }
            62..=64 => {
                let t = g.below(world.n_tpl as u32);
                let s = src(g);
                let kind = g.below(3);
                let v = obj(g);
                let p = pos(g);
                op!(ops.u(15); ops.u(t); ops.i(s); ops.u(kind); ops.i(v); ops.c(p); ops.u(if chance(g, 30) { g.below(1 << 27) } else { 0 }); ops.b(chance(g, 80)))
            }
            65..=68 => op!(ops.u(16)),
            69..=70 => {
                let l = src(g);
                op!(ops.u(17); ops.i(l); ops.i(g.below(3) as i32); ops.i(g.below(3) as i32))
            }
            71..=72 => {
                let l = if chance(g, 10) { 0 } else { src(g) };
                let p = obj(g);
                op!(ops.u(18); ops.i(p); ops.i(l); ops.i(g.below(3) as i32); ops.i(g.below(3) as i32))
            }
            73..=74 => op!(ops.u(19); ops.u(wi as u32)),
            75 => op!(ops.u(20); ops.i(obj(g))),
            76 => {
                let o = obj(g);
                op!(ops.u(21); ops.i(o); ops.b(chance(g, 50)))
            }
            77 => {
                let o = src(g);
                op!(ops.u(22); ops.i(o))
            }
            78..=80 => {
                let o = src(g);
                let v = victim_opt(g);
                op!(ops.u(23); ops.i(o); ops.i(v); ops.i(g.below(2) as i32); ops.i(g.below(5) as i32))
            }
            81..=84 => {
                let o = src(g);
                let v = obj(g);
                op!(ops.u(24); ops.i(o); ops.i(v); ops.i(pick(g, &[0, 1, 2, 3, 4])); ops.i(g.below(5) as i32); ops.i(if chance(g, 70) { -1 } else { g.below(3) as i32 }))
            }
            85..=86 => {
                let o = src(g);
                let v = victim_opt(g);
                let p = pos(g);
                op!(ops.u(25); ops.i(o); ops.i(v); ops.b(v == 0 || chance(g, 50)); ops.c(p); ops.i(pick(g, &[0, 1, 2, 3, 4])); ops.i(g.below(5) as i32); ops.i(if chance(g, 70) { -1 } else { g.below(3) as i32 }))
            }
            87..=88 => {
                let o = src(g);
                op!(ops.u(26); ops.i(o); ops.i(g.below(2) as i32); ops.i(g.below(3) as i32); ops.i(1 + g.below(2) as i32))
            }
            89 => {
                let o = src(g);
                op!(ops.u(27); ops.i(o))
            }
            90..=91 => {
                let o = src(g);
                op!(ops.u(28); ops.i(o))
            }
            92 => {
                let o = obj(g);
                let m = if chance(g, 40) { 0 } else { (1u32 << g.below(17)) | if chance(g, 30) { 1u32 << g.below(17) } else { 0 } };
                op!(ops.u(29); ops.i(o); ops.u(m))
            }
            93 => {
                let o = src(g);
                op!(ops.u(30); ops.i(o))
            }
            94 => op!(ops.u(31); ops.i(g.below(3) as i32); ops.i(g.below(5) as i32)),
            95..=96 => {
                let o = obj(g);
                op!(ops.u(32); ops.i(o); ops.u(if chance(g, 50) { g.below(1 << 27) } else { 0 }))
            }
            97 => {
                let o = obj(g);
                let bit = pick(g, &STATUS_POOL);
                op!(ops.u(33); ops.i(o); ops.i(bit); ops.b(chance(g, 50)))
            }
            98 => {
                let o = obj(g);
                let p = pos(g);
                op!(ops.u(34); ops.i(o); ops.c(p))
            }
            _ => {
                if weapons.len() > 2 {
                    let at = g.below(weapons.len() as u32) as usize;
                    let w_gone = weapons.remove(at);
                    op!(ops.u(35); ops.u(w_gone as u32));
                }
            }
        }
    }
    w.u(count);
    w.0.extend(ops.0);
}

fn gen_program(seed: u64) -> Vec<u32> {
    let mut g = Gen::new(seed);
    let mut w = W::default();
    let world = gen_header(&mut g, &mut w);
    gen_ops(&mut g, &world, &mut w);
    w.0
}

// ---------------------------------------------------------------------------------------------------------------------
// The comparison
// ---------------------------------------------------------------------------------------------------------------------

fn cpp_run(words: &[u32]) -> Vec<String> {
    let path = CString::new(format!("{}/wpnfire_{}.bin", std::env::temp_dir().display(), std::process::id())).unwrap();
    let mut cap = 1 << 20;
    loop {
        let mut buf = vec![0u8; cap];
        let n = unsafe { cpp::ref_weapon_fire_run(words.as_ptr(), words.len() as i32, path.as_ptr(), buf.as_mut_ptr() as *mut std::os::raw::c_char, cap as i32) } as usize;
        if n <= cap {
            buf.truncate(n);
            return String::from_utf8_lossy(&buf).lines().map(str::to_owned).collect();
        }
        cap = n + 1;
    }
}

fn cases() -> u64 {
    std::env::var("WEAPON_FIRE_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(3000)
}

fn rust_run_fresh(words: &[u32]) -> Result<Vec<String>, Excluded> {
    let mut names = NameKeyGenerator::new();
    names.reset();
    run_rust(words, &mut names)
}

#[test]
fn weapon_firing_matches_upstream() {
    let _guard = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { cpp::ref_weapon_reset() };
    let mut check = Check::new("weapon firing, trace lines");
    let (mut programs, mut excluded, mut lines) = (0u64, 0u64, 0u64);
    let mut counts: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
    let mut why_counts: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
    let markers = [
        "fx.fire", "fx.pos", "ocl ", "dmg ", "est ", "newobj", "launch ", "stream.add", "laser ", "assist ", "destroy", "lm.disarm", "cm.report", "assault",
        "spc.", "battleplan", "academy.mine", "adjust", "xform", "phys.", "ret fire", "ret able", "ret choose", "ret xfer-load 0", "ret setxfer-load 0",
        "ret approach", "ret estimate", "ret collide 1", "ret collide 0", "ret launchpos", "tracker ", "undetected ", "clipstatus ", "mcflags ", "animloop ",
        "ret objq", "ret oable ", "ret oableuse", "ret ochoose", "ret olock", "ret bone0", "ret bone1", "ret bonemulti", "ret canproduce",
    ];
    // (`WEAPON_FIRE_FROM` starts at another program, to look at one: `WEAPON_FIRE_FROM=7910 WEAPON_FIRE_CASES=1`)
    let from: u64 = std::env::var("WEAPON_FIRE_FROM").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    for case in from..from + cases() {
        let words = gen_program(0x57ea_0000 + case);
        programs += 1;
        let rust = match rust_run_fresh(&words) {
            Ok(l) => l,
            Err(Excluded(why)) => {
                excluded += 1;
                *why_counts.entry(why).or_insert(0) += 1;
                continue;
            }
        };
        let cpp_lines = cpp_run(&words);
        lines += rust.len() as u64;
        for l in &rust {
            for m in markers {
                if l.starts_with(m) || l.contains(m) && m.starts_with("ret") {
                    *counts.entry(m).or_insert(0) += 1;
                }
            }
        }
        check.eq(rust.len(), cpp_lines.len(), || format!("program {case}: line count"));
        for (i, (a, b)) in rust.iter().zip(cpp_lines.iter()).enumerate() {
            check.eq(a.as_str(), b.as_str(), || format!("program {case} line {i}"));
        }
    }
    println!("RESULT programs {programs}, excluded (upstream undefined) {excluded}, lines compared {lines}");
    for (why, c) in &why_counts {
        println!("RESULT   excluded: {why}: {c}");
    }
    for (m, c) in &counts {
        println!("RESULT   {m:<20} {c}");
    }
    check.finish();
}
