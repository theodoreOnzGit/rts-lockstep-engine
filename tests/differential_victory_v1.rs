#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/VictoryConditions.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `VictoryConditions` (`logic::world::victory_conditions`) against the unchanged C++, `p1w13-sonnet`, layer 11 batch V1.
//!
//! # Method
//!
//! The C++ side is `VictoryConditions.cpp` and its header, unchanged, compiled by `differential/wrap_victory.cpp` on the stand-ins of
//! `differential/shim_victory` (`vc_world.h`): the recorder (multiplayer or not), the frame, the player list (sixteen slots, a neutral
//! player), the player templates (a civilian one or none), and players that answer from the configuration: their template, observer,
//! local, whether they have objects, units and buildings that count, their name key, their relationship to each other player's default
//! team; the game info's slots (an AI's or not); every call with an effect (the map revealed, the client's fake drawables, the defeat
//! message and sound, an AI slot's last frame, `killPlayer`, the diplomacy popup, the radar forced on, the chat type) writes a line.
//! The players' questions are `Player`'s, verified in `tests/differential_player.rs`.
//!
//! The Rust side is the module on [`VictoryHost`], a host that answers from the same configuration and writes the same lines.
//!
//! A program is the configuration and 1 to 40 operations: `update`, `cachePlayerPtrs`, `init`, `setVictoryConditions`, a new frame, a
//! change to the configuration, and each of the nine questions (about a player or `NULL`). Compared after each: the result, the lines
//! and the whole state (the conditions, the cached players, the local slot, the end frame, the eliminations, the three flags). A
//! program for which upstream is undefined (the local player's slot read at -1, before `cachePlayerPtrs` found one) is not given to the
//! C++.
//!
//! # Results (2026-10-30, `VICTORY_CASES=20000`, the default)
//!
//! 20,000 programs (5,671 excluded as undefined upstream: `isLocalAlliedVictory` or `isLocalAlliedDefeat` asked with no local player,
//! which reads `m_players[-1]`), 14,329 compared, 264,727 ops: **0 mismatches**. The compared programs' effects: 13,551 players
//! eliminated (each `killPlayer` and the diplomacy popup), 6,978 after frame 1 (the map revealed, the client's fake drawables, the
//! message), 3,611 AI slots' last frame, 15,902 radars forced on, 2,103 local defeats (chat to everyone).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`, 20,000 programs: 5 of 5 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 3831 | allies one way only count as allies | 543 |
//! | 3832 | the defeat shown from frame 1, not after it | 2,002 |
//! | 3833 | the local player's defeat noticed again every frame | 2,017 |
//! | 3834 | `NoBuildings` alone never defeats | 1,245 |
//! | 3835 | the civilians' player cached | 2,944 |

use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::ini::IniError;
use rts_lockstep_engine::common::name_key_generator::NameKeyType;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::ids::PlayerIndex;
use rts_lockstep_engine::logic::world::victory_conditions::{VictoryConditions, VictoryEffect, VictoryHost};

mod support;
use support::Gen;

/// The configuration a program runs on (the driver's `cfg`).
struct Sim {
    cfg: [u32; 40],
    log: Vec<String>,
}

impl Sim {
    fn word(&self, p: PlayerIndex) -> u32 {
        self.cfg[5 + p as usize]
    }
    /// The name key the stand-ins give a name ("playerN" 1000 + N, "FactionCivilian" 500, else 1).
    fn key_of(name: &[u8]) -> i32 {
        let s = String::from_utf8_lossy(name);
        if let Some(n) = s.strip_prefix("player").and_then(|t| t.parse::<i32>().ok()) {
            return 1000 + n;
        }
        if s == "FactionCivilian" {
            500
        } else {
            1
        }
    }
}

fn key(k: i32) -> NameKeyType {
    NameKeyType(k as u32)
}

impl VictoryHost for Sim {
    fn victory_is_multiplayer(&self) -> bool {
        self.cfg[0] != 0
    }
    fn victory_frame(&self) -> u32 {
        self.cfg[1]
    }
    fn victory_nth_player(&self, i: i32) -> Option<PlayerIndex> {
        (0..16).contains(&i).then_some(i).filter(|&i| self.word(i) & 1 != 0)
    }
    fn victory_is_neutral_player(&self, p: PlayerIndex) -> bool {
        let n = self.cfg[3] as i32;
        (0..16).contains(&n) && self.word(n) & 1 != 0 && n == p
    }
    fn victory_player_template(&self, p: PlayerIndex) -> Option<NameKeyType> {
        match self.word(p) >> 1 & 3 {
            1 => Some(key(500)),
            2 => Some(key(7)),
            _ => None,
        }
    }
    fn victory_civilian_template(&mut self) -> Option<NameKeyType> {
        (self.cfg[2] != 0).then(|| key(500))
    }
    fn victory_is_player_observer(&self, p: PlayerIndex) -> bool {
        self.word(p) >> 3 & 1 != 0
    }
    fn victory_is_local_player(&self, p: PlayerIndex) -> bool {
        self.word(p) >> 4 & 1 != 0
    }
    fn victory_has_any_objects(&self, p: PlayerIndex) -> bool {
        self.word(p) >> 5 & 1 != 0
    }
    fn victory_has_any_units(&self, p: PlayerIndex) -> bool {
        self.word(p) >> 6 & 1 != 0
    }
    fn victory_has_any_buildings_counting_for_victory(&self, p: PlayerIndex) -> bool {
        self.word(p) >> 7 & 1 != 0
    }
    fn victory_relationship_to_default_team(&self, p: PlayerIndex, q: PlayerIndex) -> Relationship {
        if self.word(p) >> 16 >> q & 1 != 0 {
            Relationship::Allies
        } else if self.cfg[21 + p as usize] >> q & 1 != 0 {
            Relationship::Neutral
        } else {
            Relationship::Enemies
        }
    }
    fn victory_player_name_key(&self, p: PlayerIndex) -> NameKeyType {
        let nk = (self.word(p) >> 8 & 15) as i32;
        key(if nk < 8 { 1000 + nk } else if nk == 8 { 500 } else { 1 })
    }
    fn victory_name_key(&mut self, name: &[u8]) -> NameKeyType {
        key(Sim::key_of(name))
    }
    fn victory_slot_is_ai(&self, idx: i32) -> Option<bool> {
        (self.cfg[4] & 1 != 0 && (0..8).contains(&idx)).then(|| self.cfg[4] >> (8 + idx) & 1 != 0)
    }
    fn victory_kill_player(&mut self, p: PlayerIndex) -> Result<(), IniError> {
        self.log.push(format!("kill {p}"));
        Ok(())
    }
    fn victory_effect(&mut self, e: VictoryEffect) {
        let l = match e {
            VictoryEffect::RevealMapForPlayerPermanently(p) => format!("reveal {p}"),
            VictoryEffect::UpdateFakeDrawables => "fakedrawables".to_string(),
            VictoryEffect::PlayerHasBeenDefeatedMessage(p) => format!("message GUI:PlayerHasBeenDefeated P{p}"),
            VictoryEffect::LeftGameSound => "audio GUIMessageReceived".to_string(),
            VictoryEffect::SlotLastFrameInGame { slot, frame } => format!("slotlastframe {slot} {frame}"),
            VictoryEffect::PopulateInGameDiplomacyPopup => "diplomacy".to_string(),
            VictoryEffect::RadarForceOn => "radarforce 1".to_string(),
            VictoryEffect::InGameChatEveryone => "chat 1".to_string(),
        };
        self.log.push(l);
    }
}

fn put_str(out: &mut Vec<u32>, s: &str) {
    let b = s.as_bytes();
    out.push(b.len() as u32);
    for c in b.chunks(4) {
        out.push(c.iter().enumerate().fold(0u32, |w, (k, &x)| w | u32::from(x) << (8 * k)));
    }
}

/// The Rust run: the outputs, or `None` when upstream is undefined.
fn rust_run(cfg: &[u32; 40], ops: &[[u32; 3]]) -> Option<Vec<u32>> {
    let mut sim = Sim { cfg: *cfg, log: Vec::new() };
    let mut v = VictoryConditions::new();
    let mut out = v.state_words();
    for o in ops {
        if o[0] == 4 {
            sim.cfg[1] = o[1];
        }
        if o[0] == 5 && o[1] < 40 {
            sim.cfg[o[1] as usize] = o[2];
        }
        sim.log.clear();
        let who = (o[2] < 16).then_some(o[2] as PlayerIndex);
        let res: Result<u32, IniError> = match o[0] {
            0 => v.update(&mut sim).map(|_| 0),
            1 => {
                v.cache_player_ptrs(&mut sim);
                Ok(0)
            }
            2 => {
                v.init();
                Ok(0)
            }
            3 => {
                v.set_victory_conditions(o[1] as i32);
                Ok(0)
            }
            6 => match o[1] {
                0 => Ok(u32::from(v.has_achieved_victory(&sim, who))),
                1 => Ok(u32::from(v.has_been_defeated(&sim, who))),
                2 => Ok(u32::from(v.has_single_player_been_defeated(&sim, who))),
                3 => v.is_local_allied_victory(&sim).map(u32::from),
                4 => v.is_local_allied_defeat(&sim).map(u32::from),
                5 => Ok(u32::from(v.is_local_defeat())),
                6 => Ok(u32::from(v.am_i_observer())),
                7 => Ok(v.get_end_frame()),
                _ => Ok(v.get_victory_conditions() as u32),
            },
            _ => Ok(0),
        };
        let res = res.ok()?;
        out.push(res);
        out.push(sim.log.len() as u32);
        for l in &sim.log {
            put_str(&mut out, l);
        }
        out.extend(v.state_words());
    }
    Some(out)
}

fn cpp_run(cfg: &[u32; 40], ops: &[[u32; 3]]) -> Vec<u32> {
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let mut out = vec![0u32; 1 << 16];
    let mut len = 0i32;
    let st = unsafe { cpp::ref_victory_run(cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), out.as_mut_ptr(), out.len() as i32, &mut len) };
    assert_eq!(st, 0, "the output fits");
    out.truncate(len as usize);
    out
}

fn gen_program(g: &mut Gen) -> ([u32; 40], Vec<[u32; 3]>) {
    let mut cfg = [0u32; 40];
    cfg[0] = u32::from(g.below(8) != 0);
    cfg[1] = [0u32, 1, 2, 30][g.below(4) as usize];
    cfg[2] = u32::from(g.below(4) != 0);
    let n_players = 2 + g.below(6);
    cfg[3] = if g.below(3) == 0 { u32::MAX } else { g.below(n_players + 1) };
    cfg[4] = u32::from(g.below(3) != 0) | (g.u32() & 0xff) << 8;
    let local = g.below(n_players + 2);
    for i in 0..16u32 {
        let exists = i < n_players && g.below(12) != 0;
        let template = if g.below(10) == 0 { 0 } else if g.below(8) == 0 { 1 } else { 2 };
        let observer = u32::from(g.below(10) == 0);
        let objects = u32::from(g.below(4) != 0);
        let units = u32::from(g.below(3) != 0);
        let buildings = u32::from(g.below(3) != 0);
        let name = if g.below(6) == 0 { 8 + g.below(4) } else { i % 8 };
        // allies: mostly teams of two (0-1, 2-3, ...), sometimes one way only
        let mut allied = 0u32;
        for q in 0..16u32 {
            if q != i && (q / 2 == i / 2 && g.below(6) != 0 || g.below(15) == 0) {
                allied |= 1 << q;
            }
        }
        cfg[5 + i as usize] = u32::from(exists) | template << 1 | observer << 3 | u32::from(i == local) << 4 | objects << 5 | units << 6 | buildings << 7 | name << 8 | allied << 16;
        cfg[21 + i as usize] = g.u32() & 0xffff;
    }
    let n_ops = 1 + g.below(40) as usize;
    let mut ops = Vec::new();
    if g.below(5) != 0 {
        ops.push([1, 0, 0]);
    }
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=29 => [0, 0, 0],
            30..=33 => [1, 0, 0],
            34..=35 => [2, 0, 0],
            36..=40 => [3, g.below(4), 0],
            41..=46 => [4, g.below(5), 0],
            47..=64 => {
                // a player loses something (objects, units or buildings), or a word changes
                let p = g.below(n_players);
                let w = cfg[5 + p as usize];
                let w2 = if g.below(4) != 0 { w & !(1 << (5 + g.below(3))) } else { w ^ 1 << g.below(16) };
                cfg[5 + p as usize] = w2;
                [5, 5 + p, w2]
            }
            _ => [6, g.below(9), if g.below(10) == 0 { 16 } else { g.below(n_players) }],
        };
        ops.push(o);
    }
    (cfg, ops)
}

#[test]
fn victory_conditions_match_upstream() {
    let cases: usize = std::env::var("VICTORY_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut g = Gen::new(0x0071_C701);
    let (mut compared, mut excluded, mut ops_run, mut lines, mut mismatches) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut counts = std::collections::BTreeMap::<String, u64>::new();
    for case in 0..cases {
        let (cfg, ops) = gen_program(&mut g);
        let Some(r) = rust_run(&cfg, &ops) else {
            excluded += 1;
            continue;
        };
        let c = cpp_run(&cfg, &ops);
        compared += 1;
        ops_run += ops.len() as u64;
        if r != c {
            mismatches += 1;
            if mismatches <= 3 {
                let at = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
                println!("case {case}: cfg {cfg:x?} ops {ops:?}\n  first difference at word {at}: rust {:x?} c++ {:x?}", &r[at.saturating_sub(4)..(at + 8).min(r.len())], &c[at.saturating_sub(4)..(at + 8).min(c.len())]);
            }
        } else {
            // count the lines of the compared programs, by their first word
            let s = String::from_utf8_lossy(&r.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<u8>>()).to_string();
            for w in ["kill ", "reveal ", "message ", "slotlastframe ", "radarforce ", "chat ", "diplomacy"] {
                let n = s.matches(w).count() as u64;
                *counts.entry(w.trim().to_string()).or_insert(0) += n;
                lines += n;
            }
        }
    }
    println!(
        "V1: {cases} programs, {excluded} excluded as undefined upstream, {compared} compared ({ops_run} ops, {lines} effect lines: {counts:?}), {mismatches} mismatches"
    );
    assert_eq!(mismatches, 0);
    for w in ["kill", "reveal", "message", "slotlastframe", "radarforce", "chat", "diplomacy"] {
        assert!(counts.get(w).copied().unwrap_or(0) > 0, "the generator must reach `{w}`");
    }
}
