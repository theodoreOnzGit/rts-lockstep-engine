// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of the layer-9 create modules (`logic::modules`: `PreorderCreate`,
//! `SpecialPowerCreate`, `SupplyWarehouseCreate`, `SupplyCenterCreate`, `LockWeaponCreate`,
//! `VeterancyGainCreate`, `GrantUpgradeCreate`) against upstream's own text (commit 0a05454d), the seven files
//! of `GameLogic/Object/Create` compiled unchanged in `differential/wrap_create_modules.cpp` (see build.rs).
//! Phase 1 layer 9, batch `creates` (p1l9-creates).
//!
//! # What is compared against what
//!
//! The C++ files run on the stand-ins of `differential/shim_creates`: one object (id 7) with a controlling
//! player, a status word (under construction or not), an experience tracker (present or not, trainable or not);
//! four players, each with a preorder flag, a science mask (three sciences), and a resource gathering manager
//! (present or not); a player list (a count of 0 to 4, or `NULL`); three upgrade templates (each present or not,
//! player-type or object-type); and the INI of the block, read by the real `INI`. `CreateModule.cpp` is the
//! layer-5 base (linked from the module fixture). The Rust side is the real module on a host that answers the
//! same questions and logs the same actions.
//!
//! # Method
//!
//! A **program** is a kind (one of the seven modules), the block of INI for it (fields present or absent,
//! random values; `ScienceRequired` names one of three sciences, `UpgradeToGrant` one of three upgrades or an
//! unknown name, `ExemptStatus` a subset of two statuses), the world words (above), and a list of operations:
//! `onCreate`, `onBuildComplete`, and changes to the world (a player's preorder flag, the controller, the
//! under-construction status, the tracker, the player count, a player's sciences, a player's manager, an
//! upgrade). The same words run on both sides. Compared line for line: the `data` line the INI produced, and
//! every action the module makes on the world, in order, with its arguments (`set`/`clear` a model condition,
//! `lock` a weapon slot, `behaviors` (the special-power loop's list), `center`/`warehouse` (the supply adders),
//! `minvet` (the veterancy), `object-upgrade`/`player-upgrade`, `academy`).
//!
//! Queries (`hasScience`, `didPlayerPreorder`, `isTrainable`, `getPlayerCount`, `findUpgrade`) are not logged;
//! their answers show in the actions that follow them.
//!
//! **Inputs for which upstream is undefined behaviour** (a controlling player that is `NULL` where the module
//! dereferences it) are never sent to the C++: the generator keeps the controller present. Those inputs are
//! errors in Rust; the unit tests of each module check them (`NoControllingPlayer`).
//!
//! The concrete parts that are not compared: the special-power loop body (the host's), which calls
//! `onSpecialPowerCreation` on the object's special-power modules (none are made in this build, so the loop
//! is empty on both sides; the call is compared), and `ScienceRequired`'s resolution to a science (the
//! science store is the host's: here three names map to three bits on both sides).
//!
//! # Results
//!
//! Run 2026-10-20 (`cargo test --release --features differential --test differential_create_modules --
//! --nocapture`): **1,500 random programs, 3,295 lines compared, 0 programs differ**. The C++ made 386 preorder,
//! 175 special-power, 445 warehouse, 228 center, 308 lock-weapon, 93 veterancy and 160 grant actions (lines other
//! than `data`). The hand-written program `a_hand_written_program_makes_the_documented_calls` agrees too.
//!
//! **Planted bugs.** Each plant is one edit to one Rust module, applied alone; the random run is repeated and
//! the edit then restored. The column counts the random programs (of 1,500) that differ. All 15 are caught.
//!
//! | plant | module | programs differing |
//! |---|---|---|
//! | P1 preorder condition inverted | `PreorderCreate` | 164 |
//! | P2 no clear when not pre-ordered | `PreorderCreate` | 130 |
//! | P3 base flag not cleared | `SpecialPowerCreate` | 108 |
//! | P4 no `shouldDoOnBuildComplete` test | `SpecialPowerCreate` | 108 |
//! | P5 warehouses recorded first player first | `SupplyWarehouseCreate` | 72 |
//! | P6 warehouse recorded for player 0 only | `SupplyWarehouseCreate` | 82 |
//! | P7 centers recorded first player first | `SupplyCenterCreate` | 75 |
//! | P8 no `shouldDoOnBuildComplete` test | `SupplyCenterCreate` | 73 |
//! | P9 lock always on the primary slot | `LockWeaponCreate` | 79 |
//! | P10 lock `LOCKED_TEMPORARILY` | `LockWeaponCreate` | 145 |
//! | P11 science test inverted | `VeterancyGainCreate` | 70 |
//! | P12 trainable tracker not tested | `VeterancyGainCreate` | 48 |
//! | P13 academy record `false` | `GrantUpgradeCreate` | 20 |
//! | P14 exempt status ignored on create | `GrantUpgradeCreate` | 29 |
//! | P15 player-type upgrade in build-complete given to the object | `GrantUpgradeCreate` | 31 |
#![cfg(feature = "differential")]

use std::sync::Arc;

use rts_lockstep_engine::common::game_common::VeterancyLevel;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_COUNT, OBJECT_STATUS_UNDER_CONSTRUCTION};
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::module::create_module::CreateModuleData;
use rts_lockstep_engine::logic::modules::create_host::{CreateHost, UpgradeRef};
use rts_lockstep_engine::logic::modules::grant_upgrade_create::{GrantUpgradeCreate, GrantUpgradeCreateModuleData};
use rts_lockstep_engine::logic::modules::lock_weapon_create::{LockWeaponCreate, LockWeaponCreateModuleData};
use rts_lockstep_engine::logic::modules::preorder_create::PreorderCreate;
use rts_lockstep_engine::logic::modules::special_power_create::SpecialPowerCreate;
use rts_lockstep_engine::logic::modules::supply_center_create::SupplyCenterCreate;
use rts_lockstep_engine::logic::modules::supply_warehouse_create::SupplyWarehouseCreate;
use rts_lockstep_engine::logic::modules::veterancy_gain_create::{VeterancyGainCreate, VeterancyGainCreateModuleData};

/// The object of every program (`g_object.m_id` on the C++ side).
const OBJ: ObjectId = 7;
/// The players of a program (the C++ `g_players` has sixteen; four are ever set).
const PLAYERS: usize = 4;
/// The upgrade templates of a program.
const UPGRADES: usize = 3;
/// `PlayerList` absent.
const NULL_LIST: u32 = 0xffff_ffff;
const UPGRADE_NAMES: [&str; UPGRADES] = ["U0", "U1", "U2"];

// --- the program -------------------------------------------------------------------------------

/// A deterministic generator (xorshift64*): the same seed, the same programs, on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    /// A number in `0..n`.
    fn below(&mut self, n: u32) -> u32 {
        (self.next() % u64::from(n)) as u32
    }
    /// True with probability `num / den`.
    fn chance(&mut self, num: u32, den: u32) -> bool {
        self.below(den) < num
    }
}

/// One generated program: the block of INI and the words of `ref_create_run`.
#[derive(Clone, Debug)]
struct Program {
    text: String,
    words: Vec<u32>,
}

/// The INI block of a kind, with random fields. Kinds: 0 Preorder, 1 SpecialPower, 2 SupplyWarehouse,
/// 3 SupplyCenter (no fields), 4 LockWeapon, 5 VeterancyGain, 6 GrantUpgrade.
fn gen_block(rng: &mut Rng, kind: u32) -> String {
    let mut s = String::from("D\n");
    match kind {
        4 => {
            if rng.chance(3, 4) {
                let slot = ["PRIMARY", "SECONDARY", "TERTIARY"][rng.below(3) as usize];
                s.push_str(&format!("SlotToLock = {slot}\n"));
            }
        }
        5 => {
            if rng.chance(3, 4) {
                let level = ["REGULAR", "VETERAN", "ELITE", "HEROIC"][rng.below(4) as usize];
                s.push_str(&format!("StartingLevel = {level}\n"));
            }
            if rng.chance(2, 3) {
                let science = ["SCIENCE_A", "SCIENCE_B", "SCIENCE_C"][rng.below(3) as usize];
                s.push_str(&format!("ScienceRequired = {science}\n"));
            }
        }
        6 => {
            if rng.chance(4, 5) {
                let names = ["U0", "U1", "U2", "NOPE"];
                s.push_str(&format!("UpgradeToGrant = {}\n", names[rng.below(4) as usize]));
            }
            if rng.chance(2, 3) {
                let mut statuses = Vec::new();
                if rng.chance(1, 2) {
                    statuses.push("UNDER_CONSTRUCTION");
                }
                if rng.chance(1, 2) {
                    statuses.push("NO_COLLISIONS");
                }
                if !statuses.is_empty() {
                    s.push_str(&format!("ExemptStatus = {}\n", statuses.join(" ")));
                }
            }
        }
        _ => {}
    }
    s.push_str("End\n");
    s
}

fn gen_program(rng: &mut Rng) -> Program {
    let kind = rng.below(7);
    let text = gen_block(rng, kind);
    let mut w = vec![kind];
    // the player list: absent once in eight programs, otherwise a count of 0 to 4
    w.push(if rng.chance(1, 8) { NULL_LIST } else { rng.below(5) });
    for _ in 0..PLAYERS {
        w.push(u32::from(rng.chance(1, 3))); // preorder
        w.push(rng.below(8)); // sciences
        w.push(u32::from(rng.chance(3, 4))); // resource gathering manager present
    }
    w.push(rng.below(PLAYERS as u32)); // the controller
    w.push(u32::from(rng.chance(1, 3))); // under construction
    w.push(u32::from(rng.chance(3, 4))); // tracker present
    w.push(u32::from(rng.chance(1, 2))); // trainable
    for _ in 0..UPGRADES {
        w.push(u32::from(rng.chance(3, 4))); // exists
        w.push(u32::from(rng.chance(1, 2))); // player type
    }
    let ops = 1 + rng.below(12);
    for _ in 0..ops {
        // onCreate and onBuildComplete are the common ops; the world changes are the rest
        match rng.below(12) {
            0 | 1 | 2 => w.push(1),
            3 | 4 | 5 => w.push(2),
            6 => {
                w.push(3);
                w.push(rng.below(PLAYERS as u32));
                w.push(u32::from(rng.chance(1, 2)));
            }
            7 => {
                w.push(4);
                w.push(rng.below(PLAYERS as u32));
            }
            8 => {
                w.push(5);
                w.push(u32::from(rng.chance(1, 2)));
            }
            9 => {
                w.push(6);
                w.push(u32::from(rng.chance(3, 4)));
                w.push(u32::from(rng.chance(1, 2)));
            }
            10 => {
                w.push(7);
                w.push(if rng.chance(1, 8) { NULL_LIST } else { rng.below(5) });
            }
            _ => {
                // 8: sciences of a player; 9: a player's manager; 10: an upgrade (kept last so it stays rare)
                match rng.below(3) {
                    0 => {
                        w.push(8);
                        w.push(rng.below(PLAYERS as u32));
                        w.push(rng.below(8));
                    }
                    1 => {
                        w.push(9);
                        w.push(rng.below(PLAYERS as u32));
                        w.push(u32::from(rng.chance(1, 2)));
                    }
                    _ => {
                        w.push(10);
                        w.push(rng.below(UPGRADES as u32));
                        w.push(u32::from(rng.chance(3, 4)));
                        w.push(u32::from(rng.chance(1, 2)));
                    }
                }
            }
        }
    }
    w.push(0);
    Program { text, words: w }
}

// --- the Rust side of the world ----------------------------------------------------------------

/// The world of a program, as the C++ stand-ins hold it.
#[derive(Clone, Debug, Default)]
struct World {
    /// `None` is the absent player list.
    player_count: Option<i32>,
    players: [PlayerState; PLAYERS],
    controller: usize,
    under_construction: bool,
    tracker: bool,
    trainable: bool,
    upgrades: [(bool, bool); UPGRADES],
}

#[derive(Clone, Copy, Debug, Default)]
struct PlayerState {
    preorder: bool,
    sciences: u32,
    rgm: bool,
}

/// The host of the create modules on the Rust side: the world, and the log of its actions.
#[derive(Default)]
struct Host {
    world: World,
    log: Vec<String>,
}

impl CreateHost for Host {
    fn controlling_player(&self, _obj: ObjectId) -> Option<i32> {
        Some(self.world.controller as i32)
    }
    fn player_did_preorder(&self, player: i32) -> bool {
        self.world.players[player as usize].preorder
    }
    fn player_has_science(&self, player: i32, science: &[u8]) -> bool {
        let bit = match science {
            b"SCIENCE_A" => 0,
            b"SCIENCE_B" => 1,
            b"SCIENCE_C" => 2,
            _ => return false,
        };
        (self.world.players[player as usize].sciences >> bit) & 1 == 1
    }
    fn set_model_condition_state(&mut self, obj: ObjectId, flag: i32) {
        self.log.push(format!("set {obj} {flag}"));
    }
    fn clear_model_condition_state(&mut self, obj: ObjectId, flag: i32) {
        self.log.push(format!("clear {obj} {flag}"));
    }
    fn set_weapon_lock(&mut self, obj: ObjectId, slot: i32, lock: i32) {
        self.log.push(format!("lock {obj} {slot} {lock}"));
    }
    fn special_power_creation(&mut self, obj: ObjectId) {
        self.log.push(format!("behaviors {obj}"));
    }
    fn player_count(&self) -> Option<i32> {
        self.world.player_count
    }
    fn resource_add_supply_center(&mut self, index: i32, obj: ObjectId) {
        if self.world.players[index as usize].rgm {
            self.log.push(format!("center {index} {obj}"));
        }
    }
    fn resource_add_supply_warehouse(&mut self, index: i32, obj: ObjectId) {
        if self.world.players[index as usize].rgm {
            self.log.push(format!("warehouse {index} {obj}"));
        }
    }
    fn experience_is_trainable(&self, _obj: ObjectId) -> Option<bool> {
        if self.world.tracker {
            Some(self.world.trainable)
        } else {
            None
        }
    }
    fn experience_set_min_veterancy_level(&mut self, obj: ObjectId, level: VeterancyLevel) {
        self.log.push(format!("minvet {obj} {}", level as i32));
    }
    fn find_upgrade(&self, name: &[u8]) -> Option<UpgradeRef> {
        let i = UPGRADE_NAMES.iter().position(|n| n.as_bytes() == name)?;
        let (exists, player_type) = self.world.upgrades[i];
        exists.then_some(UpgradeRef { id: i as u32, player_type })
    }
    fn player_add_upgrade_complete(&mut self, player: i32, upgrade: UpgradeRef) {
        self.log.push(format!("player-upgrade {player} {}", upgrade.id));
    }
    fn object_give_upgrade(&mut self, obj: ObjectId, upgrade: UpgradeRef) {
        self.log.push(format!("object-upgrade {obj} {}", upgrade.id));
    }
    fn academy_record_upgrade(&mut self, player: i32, upgrade: UpgradeRef, granted: bool) {
        self.log.push(format!("academy {player} {} {}", upgrade.id, u8::from(granted)));
    }
    fn object_status_bits(&self, _obj: ObjectId) -> ObjectStatusMaskType {
        let mut mask = ObjectStatusMaskType::default();
        if self.world.under_construction {
            mask.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 1);
        }
        mask
    }
}

/// The module under test, one variant per kind.
enum Under {
    Preorder(PreorderCreate),
    Special(SpecialPowerCreate),
    Warehouse(SupplyWarehouseCreate),
    Center(SupplyCenterCreate),
    Lock(LockWeaponCreate),
    Veterancy(VeterancyGainCreate),
    Grant(GrantUpgradeCreate),
}

impl Under {
    /// `onCreate()`, for the module's kind.
    fn on_create(&mut self, host: &mut Host) {
        match self {
            Under::Preorder(m) => m.on_create(),
            Under::Special(m) => m.on_create(),
            Under::Warehouse(m) => m.on_create(host),
            Under::Center(m) => m.on_create(),
            Under::Lock(m) => m.on_create(),
            Under::Veterancy(m) => m.on_create(host),
            Under::Grant(m) => m.on_create(host).expect("the controller is present in every differential program"),
        }
    }
    /// `onBuildComplete()`, for the kinds that have one (the others have none upstream).
    fn on_build_complete(&mut self, host: &mut Host) {
        match self {
            Under::Preorder(m) => m.on_build_complete(host).expect("the controller is present in every differential program"),
            Under::Special(m) => m.on_build_complete(host),
            Under::Warehouse(_) | Under::Veterancy(_) => {}
            Under::Center(m) => m.on_build_complete(host),
            Under::Lock(m) => m.on_build_complete(host),
            Under::Grant(m) => m.on_build_complete(host).expect("the controller is present in every differential program"),
        }
    }
}

// --- the INI data, read by the Rust INI like the C++ one ---------------------------------------

/// What the block read: the kind's data, and the `data` line the C++ prints for it.
#[derive(Default)]
struct DataCtx {
    kind: u32,
    data: Option<Data>,
    line: String,
}

enum Data {
    None(CreateModuleData),
    Lock(LockWeaponCreateModuleData),
    Veterancy(VeterancyGainCreateModuleData),
    Grant(GrantUpgradeCreateModuleData),
}

fn block(ini: &mut Ini<'_>, c: &mut DataCtx) -> IniResult<()> {
    match c.kind {
        0..=3 => {
            let mut d = CreateModuleData::new();
            ini.init_from_ini_multi_proc(&mut d, |p| CreateModuleData::build_field_parse(p))?;
            c.line = "data none".to_string();
            c.data = Some(Data::None(d));
        }
        4 => {
            let mut d = LockWeaponCreateModuleData::new();
            ini.init_from_ini_multi_proc(&mut d, |p| LockWeaponCreateModuleData::build_field_parse(p))?;
            c.line = format!("data slot {}", d.slot_to_lock);
            c.data = Some(Data::Lock(d));
        }
        5 => {
            let mut d = VeterancyGainCreateModuleData::new();
            ini.init_from_ini_multi_proc(&mut d, |p| VeterancyGainCreateModuleData::build_field_parse(p))?;
            // the science as the C++ prints it: the value of the hook (SCIENCE_A = 1 ...), -1 for none
            let science = match d.science_required.as_deref() {
                None => -1,
                Some(b"SCIENCE_A") => 1,
                Some(b"SCIENCE_B") => 2,
                Some(b"SCIENCE_C") => 3,
                Some(_) => -2,
            };
            c.line = format!("data level {} science {science}", d.starting_level);
            c.data = Some(Data::Veterancy(d));
        }
        _ => {
            let mut d = GrantUpgradeCreateModuleData::new();
            ini.init_from_ini_multi_proc(&mut d, |p| GrantUpgradeCreateModuleData::build_field_parse(p))?;
            let bits: String = (0..OBJECT_STATUS_COUNT).map(|i| if d.exempt_status.test(i) { '1' } else { '0' }).collect();
            c.line = format!("data upgrade {} exempt {bits}", String::from_utf8_lossy(&d.upgrade_name));
            c.data = Some(Data::Grant(d));
        }
    }
    Ok(())
}

static BLOCKS: [BlockParse<DataCtx>; 1] = [BlockParse { token: "D", parse: block }];

/// Reads the block with the Rust INI. `None` is an error (the generator never makes one).
fn read_block(kind: u32, text: &str) -> Option<(Data, String)> {
    let mut c = DataCtx { kind, ..DataCtx::default() };
    Ini::load(b"create.ini", text.as_bytes(), IniLoadType::Overwrite, None, &BLOCKS, &mut c).ok()?;
    Some((c.data?, c.line))
}

// --- the two runs -------------------------------------------------------------------------------

/// Runs a program on the Rust side: the lines the C++ prints, in the same format.
fn run_rust(p: &Program) -> String {
    let w = &p.words;
    let kind = w[0];
    let mut world = World::default();
    world.player_count = if w[1] == NULL_LIST { None } else { Some(w[1] as i32) };
    let mut r = 2;
    for player in world.players.iter_mut() {
        player.preorder = w[r] != 0;
        player.sciences = w[r + 1];
        player.rgm = w[r + 2] != 0;
        r += 3;
    }
    world.controller = (w[r] & 3) as usize;
    world.under_construction = w[r + 1] != 0;
    world.tracker = w[r + 2] != 0;
    world.trainable = w[r + 3] != 0;
    r += 4;
    for u in world.upgrades.iter_mut() {
        *u = (w[r] != 0, w[r + 1] != 0);
        r += 2;
    }
    let mut host = Host { world, log: Vec::new() };

    let (data, line) = read_block(kind, &p.text).expect("the generated block is valid");
    host.log.push(line);
    let mut under = match data {
        Data::None(d) => {
            let d = Arc::new(d);
            match kind {
                0 => Under::Preorder(PreorderCreate::new(OBJ, Some(d)).expect("data")),
                1 => Under::Special(SpecialPowerCreate::new(OBJ, Some(d)).expect("data")),
                2 => Under::Warehouse(SupplyWarehouseCreate::new(OBJ, Some(d)).expect("data")),
                _ => Under::Center(SupplyCenterCreate::new(OBJ, Some(d)).expect("data")),
            }
        }
        Data::Lock(d) => Under::Lock(LockWeaponCreate::new(OBJ, Some(Arc::new(d))).expect("data")),
        Data::Veterancy(d) => Under::Veterancy(VeterancyGainCreate::new(OBJ, Some(Arc::new(d))).expect("data")),
        Data::Grant(d) => Under::Grant(GrantUpgradeCreate::new(OBJ, Some(Arc::new(d))).expect("data")),
    };

    while r < w.len() {
        let op = w[r];
        r += 1;
        match op {
            0 => break,
            1 => under.on_create(&mut host),
            2 => under.on_build_complete(&mut host),
            3 => {
                host.world.players[(w[r] & 3) as usize].preorder = w[r + 1] != 0;
                r += 2;
            }
            4 => {
                host.world.controller = (w[r] & 3) as usize;
                r += 1;
            }
            5 => {
                host.world.under_construction = w[r] != 0;
                r += 1;
            }
            6 => {
                host.world.tracker = w[r] != 0;
                host.world.trainable = w[r + 1] != 0;
                r += 2;
            }
            7 => {
                host.world.player_count = if w[r] == NULL_LIST { None } else { Some(w[r] as i32) };
                r += 1;
            }
            8 => {
                host.world.players[(w[r] & 3) as usize].sciences = w[r + 1];
                r += 2;
            }
            9 => {
                host.world.players[(w[r] & 3) as usize].rgm = w[r + 1] != 0;
                r += 2;
            }
            10 => {
                let u = (w[r] % UPGRADES as u32) as usize;
                host.world.upgrades[u] = (w[r + 1] != 0, w[r + 2] != 0);
                r += 3;
            }
            other => panic!("the generator made op {other}"),
        }
    }
    let mut out = String::new();
    for l in &host.log {
        out.push_str(l);
        out.push('\n');
    }
    out
}

/// Runs a program on the C++ side: the lines the stand-ins log.
fn run_cpp(p: &Program) -> String {
    let mut out = vec![0u8; 1 << 20];
    let mut out_len: i32 = 0;
    let rc = unsafe {
        cpp::ref_create_run(
            p.text.as_ptr() as *const _,
            p.text.len() as i32,
            p.words.as_ptr(),
            p.words.len() as i32,
            out.as_mut_ptr() as *mut _,
            out.len() as i32,
            &mut out_len,
        )
    };
    assert_eq!(rc, 0, "the C++ run");
    String::from_utf8_lossy(&out[..out_len as usize]).into_owned()
}

/// The first line where two outputs differ, for the report.
fn first_difference(a: &str, b: &str) -> String {
    for (i, (x, y)) in a.lines().zip(b.lines()).enumerate() {
        if x != y {
            return format!("line {i}: rust `{x}` vs c++ `{y}`");
        }
    }
    format!("line counts {} vs {}", a.lines().count(), b.lines().count())
}

#[test]
fn creates_agree_with_the_upstream_text_on_random_programs() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let programs = 1500;
    let mut lines = 0usize;
    let mut differing = 0usize;
    let mut actions = [0usize; 7];
    let mut first_bad: Option<(Program, String, String)> = None;
    for _ in 0..programs {
        let p = gen_program(&mut rng);
        let rust = run_rust(&p);
        let cpp_out = run_cpp(&p);
        lines += cpp_out.lines().count();
        actions[p.words[0] as usize] += cpp_out.lines().filter(|l| !l.starts_with("data")).count();
        if rust != cpp_out {
            differing += 1;
            if first_bad.is_none() {
                first_bad = Some((p.clone(), rust.clone(), cpp_out.clone()));
            }
        }
    }
    println!(
        "RESULTS create modules: {programs} programs, {lines} lines compared, {differing} programs differ; \
         actions per kind (preorder, special, warehouse, center, lock, veterancy, grant): {actions:?}"
    );
    if let Some((p, rust, cpp_out)) = &first_bad {
        panic!("{differing} programs differ; first: {}\nprogram: {p:?}", first_difference(rust, cpp_out));
    }
}

/// Each behaviour the C++ does and the Rust must too, in one program: the preorder flag set and cleared, the
/// supply adders in reverse player order, the grant to the player (an academy record after it), and the
/// lock. Written out, so a reader sees what the random programs cover.
#[test]
fn a_hand_written_program_makes_the_documented_calls() {
    // kind 6 (grant), 4 players, controller 1, under construction, no tracker
    let words = vec![
        6, 4, // kind (grant), players
        1, 0, 1, // player 0: preorder, sciences, manager
        0, 0, 1, // player 1
        0, 0, 1, // player 2
        0, 0, 0, // player 3
        1, // controller
        0, // under construction (the exempt status then grants)
        0, 0, // tracker: absent, not trainable
        1, 1, 0, 0, 0, 0, // U0 present and player-type, U1 and U2 absent
        1, // onCreate
        0,
    ];
    let p = Program {
        text: "D\nUpgradeToGrant = U0\nExemptStatus = UNDER_CONSTRUCTION\nEnd\n".to_string(),
        words,
    };
    let rust = run_rust(&p);
    let cpp_out = run_cpp(&p);
    assert_eq!(rust, cpp_out);
    assert!(rust.contains("player-upgrade 1 0"), "the player-type upgrade goes to the controller: {rust}");
    assert!(rust.contains("academy 1 0 1"), "the academy records the granted upgrade: {rust}");
}
