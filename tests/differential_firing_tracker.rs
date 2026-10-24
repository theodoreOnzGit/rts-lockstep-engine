// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::firing_tracker` (`FiringTracker`)
//! against upstream `GameLogic/Object/FiringTracker.cpp` (commit 0a05454d),
//! compiled unchanged in `differential/wrap_firing_tracker.cpp` (see
//! build.rs). Phase 1 layer 8, p1w6-trackers.
//!
//! # Methodology
//!
//! A **program** is a random sequence of operations on one tracker owned by
//! object 1, with one to four weapons (auto-reload frames, coast frames, the
//! two shots-needed counts, the loop time, a fire sound), run on both sides
//! from a fresh tracker: `shotFired` (any weapon, any victim 0..4, after a
//! random advance of the frame), `update` (after a random advance, including
//! jumps of thousands of frames), the owner's bonus word set directly, the
//! tracker destroyed and rebuilt, `getNumConsecutiveShotsAtVictim`,
//! `getLastShotVictim`, the client's "is the looping sound playing" answer,
//! `XferSave` (the bytes), `XferLoad` of a blob (the status and a save of the
//! tracker afterwards), a weapon's reloading state and next-shot frame, and
//! a victim's faerie-fire status. Auto-reload and coast values near
//! `u32::MAX` are drawn now and then so that the unsigned frame sums wrap.
//!
//! The stand-ins on both sides model what upstream's `Object` does when the
//! bonus word changes: `setWeaponBonusCondition` / `clearWeaponBonusCondition`
//! call `weaponSetOnWeaponBonusChange`, which recomputes the next-shot frame
//! of every weapon that is reloading (`frame + delay(word)`, `delay` any fixed
//! function of the word). That is the cut point the module docs of
//! `firing_tracker.rs` call "the decision": it makes the tracker's read of
//! `getPossibleNextShotFrame` depend on its order relative to the faerie-fire
//! step.
//!
//! **Compared, word for word:** the log of every program. After each
//! operation the log has the operation's own results, the owner's weapon
//! bonus word, and then **every call the tracker made on its owner, weapon
//! and audio, in call order**: `setWeaponBonusCondition`,
//! `clearWeaponBonusCondition`, `clearAndSetModelConditionFlags` (the three
//! continuous-fire bits of each mask), `reloadAllAmmo`, `addAudioEvent` (the
//! object and the name), `removeAudioEvent` and `setWakeFrame`. The
//! looping-sound restart is the client's answer (`isCurrentlyPlaying`) on
//! both sides, so a restart that upstream skips shows as a missing add.
//!
//! The module base's part of the transfer is empty on the C++ side (its
//! stand-in `UpdateModule::xfer` writes nothing) and an empty closure on the
//! Rust side, so the bytes compared are the tracker's own: the version, the
//! count, the victim and the cooldown frame. The blob a program loads is
//! random bytes, a valid save, that save with one bit flipped, or that save
//! cut at a field boundary (never inside a field: a load that runs out of
//! bytes part-way through a field leaves the bytes `XferLoad` had copied in
//! upstream and not in this crate, a property of the Xfer layer outside this
//! unit).
//!
//! **Planted bugs** (each run on the same programs, each must differ):
//! 1. the weapon's next-shot frame is read *before* the faerie-fire step (the
//!    snapshot design of the previous draft), and
//! 2. the looping sound is restarted whether or not it is playing.
//!
//! # Results
//!
//! Recorded by `cargo test --release --features differential --test
//! differential_firing_tracker -- --nocapture` (the `DIFF`, `PLANTED` and
//! `COVERAGE` lines).

#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::game_type::{ObjectId, INVALID_ID};
use rts_lockstep_engine::common::model_state::{
    ModelConditionFlags, MODELCONDITION_CONTINUOUS_FIRE_FAST, MODELCONDITION_CONTINUOUS_FIRE_MEAN,
    MODELCONDITION_CONTINUOUS_FIRE_SLOW,
};
use rts_lockstep_engine::common::xfer::{Xfer, XferStatus};
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::firing_tracker::{FiringHost, FiringTracker, WeaponRef};
use rts_lockstep_engine::logic::module::update_module::UpdateSleepTime;
use std::ffi::CString;
use support::{Check, Gen};

/// Programs per run.
const PROGRAMS: usize = 3000;
/// The fire sounds (`soundNames` in wrap_firing_tracker.cpp, same order).
const SOUNDS: [&str; 4] = ["FIRE_ONE", "FIRE_LOOP", "FIRE_THREE", "FIRE_FOUR"];
/// The owner's rapid-fire voice.
const VOICE: &str = "VOICE_RAPID";
/// Field boundaries of a save: after the version, count, victim, cooldown frame.
const BOUNDARIES: [usize; 5] = [0, 1, 5, 9, 13];

/// One weapon of the program.
#[derive(Clone, Debug)]
struct WeaponSpec {
    auto_reload: u32,
    coast: u32,
    one: i32,
    two: i32,
    loop_time: u32,
    sound: usize,
}

/// A program: the weapons, the owner's initial bonus word, the operations, the blob.
struct Program {
    weapons: Vec<WeaponSpec>,
    word: u32,
    ops: Vec<[i32; 4]>,
    blob: Vec<u8>,
}

/// Which planted bug a run has (none for the real comparison).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Plant {
    None,
    /// The next-shot frame is read before the faerie-fire step.
    StaleNextShot,
    /// The looping sound restarts whether or not it is playing.
    AlwaysRestart,
}

/// One call the tracker made (the encoding of `putCalls` in the wrapper).
enum Call {
    Set(i32),
    Clear(i32),
    Model(u32, u32),
    Reload(bool),
    Add(&'static str),
    Remove,
    Wake(u32),
}

/// The delay `Weapon::onWeaponBonusChange` would compute, as in the wrapper.
fn bonus_delay(word: u32) -> u32 {
    1 + ((word ^ (word >> 2)) % 13)
}

/// A weapon of the host.
struct HostWeapon {
    spec: WeaponSpec,
    next_shot: u32,
    reloading: bool,
}

/// The Rust host: the owner, the world and the audio, recording every call.
struct Host {
    frame: u32,
    word: u32,
    faerie: [bool; 4],
    weapons: Vec<HostWeapon>,
    playing: bool,
    calls: Vec<Call>,
    plant: Plant,
    /// `StaleNextShot`: the next-shot frames as they were when the shot began.
    stale: Option<Vec<u32>>,
}

impl Host {
    fn changed(&mut self, old: u32) {
        if old != self.word {
            for w in &mut self.weapons {
                if w.reloading {
                    w.next_shot = self.frame.wrapping_add(bonus_delay(self.word));
                }
            }
        }
    }

    fn bits3(f: &ModelConditionFlags) -> u32 {
        let mut b = 0;
        for (k, c) in [
            MODELCONDITION_CONTINUOUS_FIRE_SLOW,
            MODELCONDITION_CONTINUOUS_FIRE_MEAN,
            MODELCONDITION_CONTINUOUS_FIRE_FAST,
        ]
        .into_iter()
        .enumerate()
        {
            if f.test(c) {
                b |= 1 << k;
            }
        }
        b
    }

    /// The calls since the last drain, in the wrapper's `putCalls` encoding.
    fn drain(&mut self, out: &mut Vec<u32>) {
        out.push(self.word);
        out.push(self.calls.len() as u32);
        for c in self.calls.drain(..) {
            match c {
                Call::Set(c) => out.extend([1, c as u32]),
                Call::Clear(c) => out.extend([2, c as u32]),
                Call::Model(clr, set) => out.extend([3, clr, set]),
                Call::Reload(now) => out.extend([4, u32::from(now)]),
                Call::Add(name) => {
                    out.extend([5, 1, name.len() as u32]);
                    out.extend(name.bytes().map(u32::from));
                }
                Call::Remove => out.push(6),
                Call::Wake(w) => out.extend([7, w]),
            }
        }
    }
}

impl FiringHost for Host {
    fn frame(&self) -> u32 {
        self.frame
    }
    fn victim_has_faerie_fire(&self, victim_id: ObjectId) -> bool {
        (1..=4).contains(&victim_id) && self.faerie[(victim_id - 1) as usize]
    }
    fn test_weapon_bonus_condition(&self, condition: i32) -> bool {
        self.word & (1 << condition) != 0
    }
    fn set_weapon_bonus_condition(&mut self, condition: i32) {
        let old = self.word;
        self.word |= 1 << condition;
        self.calls.push(Call::Set(condition));
        self.changed(old);
    }
    fn clear_weapon_bonus_condition(&mut self, condition: i32) {
        let old = self.word;
        self.word &= !(1 << condition);
        self.calls.push(Call::Clear(condition));
        self.changed(old);
    }
    fn clear_and_set_model_condition_flags(&mut self, clr: &ModelConditionFlags, set: &ModelConditionFlags) {
        self.calls.push(Call::Model(Host::bits3(clr), Host::bits3(set)));
    }
    fn reload_all_ammo(&mut self, now: bool) {
        self.calls.push(Call::Reload(now));
    }
    fn weapon_auto_reload_when_idle_frames(&self, weapon: WeaponRef) -> u32 {
        self.weapons[weapon as usize].spec.auto_reload
    }
    fn weapon_continuous_fire_coast_frames(&self, weapon: WeaponRef) -> u32 {
        self.weapons[weapon as usize].spec.coast
    }
    fn weapon_possible_next_shot_frame(&self, weapon: WeaponRef) -> u32 {
        match &self.stale {
            Some(s) => s[weapon as usize],
            None => self.weapons[weapon as usize].next_shot,
        }
    }
    fn weapon_continuous_fire_one_shots_needed(&self, weapon: WeaponRef) -> i32 {
        self.weapons[weapon as usize].spec.one
    }
    fn weapon_continuous_fire_two_shots_needed(&self, weapon: WeaponRef) -> i32 {
        self.weapons[weapon as usize].spec.two
    }
    fn weapon_fire_sound_loop_time(&self, weapon: WeaponRef) -> u32 {
        self.weapons[weapon as usize].spec.loop_time
    }
    fn play_fire_sound(&mut self, weapon: WeaponRef) {
        self.calls.push(Call::Add(SOUNDS[self.weapons[weapon as usize].spec.sound]));
    }
    fn start_looping_fire_sound(&mut self, weapon: WeaponRef, only_if_not_playing: bool) {
        // the client's side of the cut point: add unless it is already playing
        if !only_if_not_playing || !self.playing || self.plant == Plant::AlwaysRestart {
            self.calls.push(Call::Add(SOUNDS[self.weapons[weapon as usize].spec.sound]));
        }
    }
    fn stop_looping_fire_sound(&mut self) {
        self.calls.push(Call::Remove);
    }
    fn play_rapid_fire_voice(&mut self) {
        self.calls.push(Call::Add(VOICE));
    }
    fn set_wake_frame(&mut self, wake_delay: UpdateSleepTime) {
        self.calls.push(Call::Wake(wake_delay.0));
    }
}

fn save_bytes(t: &mut FiringTracker) -> Vec<u8> {
    let mut save = XferSave::new();
    save.open(b"firing").unwrap();
    t.xfer(&mut save, |_| Ok(())).unwrap();
    save.close().unwrap();
    save.take_bytes()
}

fn push_bytes(out: &mut Vec<u32>, bytes: &[u8]) {
    out.push(bytes.len() as u32);
    out.extend(bytes.iter().map(|&b| u32::from(b)));
}

/// The Rust side of one program: the same log words as `ref_firing_run`.
/// `model_calls` counts the `clearAndSetModelConditionFlags` calls made (coverage).
fn rust_run(p: &Program, plant: Plant, model_calls: &mut usize) -> Vec<u32> {
    let mut host = Host {
        frame: 0,
        word: p.word,
        faerie: [false; 4],
        weapons: p.weapons.iter().map(|s| HostWeapon { spec: s.clone(), next_shot: 0, reloading: false }).collect(),
        playing: false,
        calls: Vec::new(),
        plant,
        stale: None,
    };
    let mut out = Vec::new();
    let mut t = FiringTracker::new(&mut host);
    host.drain(&mut out);
    for &[code, a, b, c] in &p.ops {
        match code {
            0 => {
                host.frame = host.frame.wrapping_add(c as u32);
                if plant == Plant::StaleNextShot {
                    host.stale = Some(host.weapons.iter().map(|w| w.next_shot).collect());
                }
                t.shot_fired(&mut host, a as WeaponRef, b);
                host.stale = None;
            }
            1 => {
                host.frame = host.frame.wrapping_add(a as u32);
                out.push(t.update(&mut host).0);
            }
            2 => host.word = a as u32,
            3 => {
                t.destroy(&mut host);
                t = FiringTracker::new(&mut host);
            }
            4 => out.push(t.get_num_consecutive_shots_at_victim((1..=4).contains(&b).then_some(b)) as u32),
            5 => out.push(t.get_last_shot_victim() as u32),
            6 => host.playing = a != 0,
            7 => push_bytes(&mut out, &save_bytes(&mut t)),
            8 => {
                let mut load = XferLoad::new(p.blob.clone());
                load.open(b"firing").unwrap();
                let status = match t.xfer(&mut load, |_| Ok(())) {
                    Ok(()) => 0,
                    Err(s) => 1 + s as u32,
                };
                load.close().unwrap();
                out.push(status);
                push_bytes(&mut out, &save_bytes(&mut t));
            }
            9 => {
                host.weapons[a as usize].next_shot = b as u32;
                host.weapons[a as usize].reloading = c != 0;
            }
            _ => host.faerie[(b - 1) as usize] = c != 0,
        }
        *model_calls += host.calls.iter().filter(|c| matches!(c, Call::Model(_, _))).count();
        host.drain(&mut out);
    }
    t.destroy(&mut host);
    host.drain(&mut out);
    out
}

/// Serialises the C++ side (its stand-in objects are process globals).
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The C++ side of one program.
fn cpp_run(p: &Program, path: &std::path::Path) -> Vec<u32> {
    let mut spec: Vec<i32> = vec![p.weapons.len() as i32];
    for w in &p.weapons {
        spec.extend([w.auto_reload as i32, w.coast as i32, w.one, w.two, w.loop_time as i32, w.sound as i32]);
    }
    spec.push(p.word as i32);
    let flat: Vec<i32> = p.ops.iter().flat_map(|o| o.iter().copied()).collect();
    let cpath = CString::new(path.to_string_lossy().as_bytes()).expect("path");
    let _serial = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = vec![0u32; 1 << 18];
    let r = unsafe {
        cpp::ref_firing_run(
            spec.as_ptr(),
            spec.len() as i32,
            flat.as_ptr(),
            p.ops.len() as i32,
            p.blob.as_ptr(),
            p.blob.len() as i32,
            cpath.as_ptr(),
            out.as_mut_ptr(),
            out.len() as i32,
        )
    };
    assert!(r >= 0, "ref_firing_run returned {r}");
    out.truncate(r as usize);
    out
}

fn random_weapon(g: &mut Gen) -> WeaponSpec {
    let auto_reload = match g.below(6) {
        0 => 0,
        1 => 0xffff_fff0 + g.below(16),
        _ => 5 + g.below(200),
    };
    let coast = match g.below(6) {
        0 => 0,
        1 => 0xffff_ff00 + g.below(256),
        _ => 1 + g.below(60),
    };
    WeaponSpec {
        auto_reload,
        coast,
        one: g.below(7) as i32 - 1,
        two: g.below(12) as i32 - 1,
        loop_time: if g.below(2) == 0 { 0 } else { 5 + g.below(100) },
        sound: g.below(4) as usize,
    }
}

/// The save of a random tracker state: version 1, the count, the victim, the cooldown frame.
fn random_save(g: &mut Gen) -> Vec<u8> {
    let mut b = vec![1u8];
    b.extend((g.below(12) as i32 - 2).to_le_bytes());
    b.extend((g.below(6) as i32).to_le_bytes());
    b.extend(g.u32().wrapping_shr(g.below(32)).to_le_bytes());
    b
}

/// The blob a program loads: random bytes, a valid save, that save with one
/// bit flipped, or that save cut at a field boundary.
fn random_blob(g: &mut Gen) -> Vec<u8> {
    match g.below(4) {
        0 => (0..BOUNDARIES[g.below(5) as usize]).map(|_| g.u32() as u8).collect(),
        1 => random_save(g),
        2 => {
            let mut b = random_save(g);
            let i = g.below(b.len() as u32) as usize;
            b[i] ^= 1 << g.below(8);
            b
        }
        _ => {
            let b = random_save(g);
            b[..BOUNDARIES[g.below(5) as usize]].to_vec()
        }
    }
}

fn random_program(g: &mut Gen) -> Program {
    let n_weapons = 1 + g.below(4) as usize;
    let weapons: Vec<WeaponSpec> = (0..n_weapons).map(|_| random_weapon(g)).collect();
    let word = if g.below(3) == 0 { g.u32() & 0x07ff_ffff } else { 0 };
    let blob = random_blob(g);
    let n_ops = 1 + g.below(60);
    let mut ops = Vec::new();
    for _ in 0..n_ops {
        let w = g.below(n_weapons as u32) as i32;
        let advance = match g.below(8) {
            0 => 0,
            1 => g.below(100_000) as i32,
            _ => g.below(40) as i32,
        };
        let op = match g.below(50) {
            0..=16 => [0, w, g.below(5) as i32, advance],
            17..=26 => [1, advance, 0, 0],
            27 => [2, (g.u32() & 0x07ff_ffff) as i32, 0, 0],
            28 => [3, 0, 0, 0],
            29..=30 => [4, 0, g.below(6) as i32, 0],
            31 => [5, 0, 0, 0],
            32..=34 => [6, g.below(2) as i32, 0, 0],
            35..=36 => [7, 0, 0, 0],
            37..=39 => [8, 0, 0, 0],
            40..=44 => [9, w, g.below(3000) as i32, g.below(2) as i32],
            _ => [10, 0, 1 + g.below(4) as i32, g.below(2) as i32],
        };
        ops.push(op);
    }
    Program { weapons, word, ops, blob }
}

fn temp_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("p1w6-firing-{}-{tag}.bin", std::process::id()))
}

fn describe(p: &Program) -> String {
    format!("ops {:?} weapons {:?} word {} blob {:?}", p.ops, p.weapons, p.word, p.blob)
}

fn run(plant: Plant, tag: &str, name: &str) -> Check {
    let path = temp_path(tag);
    let mut g = Gen::new(0x0603);
    let mut check = Check::new(name);
    let mut model_calls = 0usize;
    let (mut ops_total, mut shots, mut loads, mut words) = (0usize, 0usize, 0usize, 0usize);
    for _ in 0..PROGRAMS {
        let p = random_program(&mut g);
        ops_total += p.ops.len();
        shots += p.ops.iter().filter(|o| o[0] == 0).count();
        loads += p.ops.iter().filter(|o| o[0] == 8).count();
        let rust = rust_run(&p, plant, &mut model_calls);
        let cpp = cpp_run(&p, &path);
        words += cpp.len();
        check.eq(rust, cpp, || describe(&p));
    }
    let _ = std::fs::remove_file(&path);
    if plant == Plant::None {
        println!(
            "COVERAGE FiringTracker: {ops_total} operations, {shots} shots, {loads} loads, {} model-condition calls, \
             {PROGRAMS} programs, {words} log words",
            model_calls
        );
        assert!(model_calls > 2000, "the rate changes were barely exercised ({model_calls})");
    }
    check
}

#[test]
fn firing_tracker_matches_upstream_call_for_call() {
    run(Plant::None, "main", "FiringTracker (programs, calls, saves, loads)").finish();
}

#[test]
fn planted_stale_next_shot_read_is_caught() {
    run(Plant::StaleNextShot, "stale", "FiringTracker (planted: next-shot read before faerie fire)")
        .finish_planted();
}

#[test]
fn planted_unconditional_loop_restart_is_caught() {
    run(Plant::AlwaysRestart, "restart", "FiringTracker (planted: loop restarts while playing)").finish_planted();
}

#[test]
fn a_wrong_version_is_refused_before_any_field() {
    let mut host = Host {
        frame: 0,
        word: 0,
        faerie: [false; 4],
        weapons: Vec::new(),
        playing: false,
        calls: Vec::new(),
        plant: Plant::None,
        stale: None,
    };
    let mut t = FiringTracker::new(&mut host);
    let mut load = XferLoad::new(vec![2, 5, 0, 0, 0]);
    load.open(b"x").unwrap();
    assert!(matches!(t.xfer(&mut load, |_| Ok(())), Err(XferStatus::InvalidVersion)));
    assert_eq!(t.get_last_shot_victim(), INVALID_ID);
}
