// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::sides_list`, `logic::scripts` and
//! `formats::data_chunk_output` against upstream `SidesList.cpp` and
//! `Scripts.cpp` (commit 0a05454d), compiled by g++ under the shims of
//! `differential/shim_sides` with the Phase 0 `DataChunk.cpp` /
//! `NameKeyGenerator.cpp` (see `differential/wrap_sides.cpp`, which says what
//! is stood in and what the one mechanical edit of `SidesList.cpp` is).
//!
//! # Methodology
//!
//! No game maps are available, so every chunk file is **synthetic**, built
//! byte by byte in the layout the upstream parsers read (a table of contents,
//! then nested chunks), and fed to **both** implementations. Per file the
//! comparison is of a **log** that both sides produce in the same format:
//!
//! 1. the result of the parse (`parse` returned true / false, or the
//!    `ErrorCode` thrown);
//! 2. a **dump of every field** of everything parsed: for each side its dict
//!    (pair by pair, in key order, by name), every `BuildListInfo` field, its
//!    script list (groups, scripts, OR clauses, conditions, actions, and every
//!    parameter: type, `m_initialized`, int, real bits, string, position,
//!    object status mask), the skirmish copies, the teams;
//! 3. after each of up to 8 **operations** on the list (`removeSide`,
//!    `removeTeam`, `addPlayerByTemplate`, `validateSides`, the finders,
//!    `prepareForMP_or_Skirmish` reading a generated skirmish scripts file,
//!    `duplicateAndQualify` / `duplicate` of script lists, `SidesInfo::operator=`,
//!    build list reordering, the script list editing functions
//!    `deleteScript`, `deleteGroup`, `deleteAction`, `deleteOrCondition`,
//!    `removeCondition`, `updateFrom`, `addScript`, `updateDefaults`, `reset`,
//!    `findPrevious*`; the table is at `runOp` in `wrap_sides.cpp`): its
//!    result and a new dump;
//! 4. **every byte** of the file `SidesList::WriteSidesDataChunk` (or
//!    `ScriptList::WriteScriptsDataChunk` for a `PlayerScriptsList` file)
//!    writes through a real `DataChunkOutput`, and a dump after it
//!    (`validateSides` runs after the write; `WriteParameter` replaces the
//!    string of a `KIND_OF_PARAM`);
//! 5. the next number of the group counter `ScriptList::m_curId`.
//!
//! The script templates (what `ScriptEngine::init` builds, layer 11) are a
//! synthetic table, the same in both: every condition and action has a name
//! of its own and 0 to 5 parameters of pseudo-random types, except those the
//! parser repairs (`TEAM_INSIDE_AREA_*`, `SKIRMISH_SPECIAL_POWER_READY`,
//! `TEAM_FOLLOW_WAYPOINTS`, the camera and map actions, ...), which get the
//! parameter lists the repairs lead to. The generator makes files whose
//! conditions and actions mostly follow their templates and sometimes do not
//! (wrong count, wrong name key, unknown name, retired chunk versions); half
//! of the files are well-formed and the rest also get truncations and byte
//! mutations. Process-wide state (the group counter, the read lists
//! `s_readLists`/`s_numInReadList`, `static_readPlayerNames`, the name keys)
//! is kept **across cases**, in both implementations, as the originals keep
//! it; a case the Rust reports as undefined is run on a copy and not given
//! to the C++, so both stay in step.
//!
//! **Undefined upstream inputs** are those for which [`MapUndefined`] (or
//! the chunk reader's [`ChunkUndefined`]) is returned: the Rust errors, the
//! case is not given to the C++ and is counted by kind. The test prints the
//! counts.
//!
//! **Limits of the C++ stand-ins.** The `AsciiString` of the differential
//! build is a `std::string` shim whose `getLength()` counts bytes past an
//! embedded NUL, where the engine's is `strlen`; strings the generator makes
//! contain no NUL, and none is longer than 32,767 characters (the engine's
//! `AsciiString` throws past that, the shim does not). The stand-in `Dict`
//! has no pair limit (`Dict::setPrep` throws `ERROR_OUT_OF_MEMORY` past
//! 32,767), and no dict here is anywhere near it.
//!
//! # Results
//!
//! See the line the test prints (counts of files, parses that succeeded and
//! failed, thrown codes, undefined cases by kind, log bytes compared,
//! mismatches) and the record in `docs/phase1-porting-graph.md`. The
//! **planted bugs** (a deliberate error put into the translation, one per
//! comparison, to see the comparison fail, then removed) are listed there
//! with how many files caught each.
#![cfg(feature = "differential")]
#![allow(clippy::too_many_arguments)]

mod support;

use rts_lockstep_engine::common::dict::{DataType, Dict};
use rts_lockstep_engine::common::kind_of::KindOfMaskType;
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType;
use rts_lockstep_engine::common::well_known_keys::{self as wk, WELL_KNOWN_KEY_NAMES};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::data_chunk_output::DataChunkOutput;
use rts_lockstep_engine::logic::scripts::{
    action_type, condition_type, parameter_type, version as sv, Condition, MapDataError, Parameter,
    Script, ScriptAction, ScriptList, ScriptTemplate, ScriptTemplates, MAX_PARMS, MAX_PLAYER_COUNT,
};
use rts_lockstep_engine::logic::sides_list::{BuildListInfo, SidesCtx, SidesInfo, SidesList};
use std::collections::BTreeMap;
use support::{Check, Gen};

// ---------------------------------------------------------------------------
// The synthetic script templates
// ---------------------------------------------------------------------------

/// The name and parameter types of each template: conditions, then actions.
fn template_defs() -> Vec<(String, Vec<u8>)> {
    use action_type as at;
    use condition_type as ct;
    use parameter_type as pt;
    let mut g = Gen::new(0x7E57);
    let random_types = |g: &mut Gen| -> Vec<u8> {
        let n = match g.below(10) {
            0..=2 => 0,
            3..=5 => 1,
            6..=7 => 2,
            8 => 3,
            _ => 5,
        };
        (0..n)
            .map(|_| match g.below(8) {
                0 => pt::INT as u8,
                1 => pt::SIDE as u8,
                2 => pt::TEAM as u8,
                3 => pt::OBJECT_TYPE as u8,
                4 => pt::COORD3D as u8,
                _ => g.below(pt::NUM_ITEMS as u32) as u8,
            })
            .collect()
    };
    let mut defs = Vec::new();
    for i in 0..ct::NUM_ITEMS {
        let types: Vec<u8> = match i {
            ct::TEAM_INSIDE_AREA_PARTIALLY
            | ct::TEAM_INSIDE_AREA_ENTIRELY
            | ct::TEAM_OUTSIDE_AREA_ENTIRELY
            | ct::TEAM_ENTERED_AREA_PARTIALLY
            | ct::TEAM_ENTERED_AREA_ENTIRELY
            | ct::TEAM_EXITED_AREA_ENTIRELY
            | ct::TEAM_EXITED_AREA_PARTIALLY => vec![
                pt::TEAM as u8,
                pt::TRIGGER_AREA as u8,
                pt::SURFACES_ALLOWED as u8,
            ],
            ct::SKIRMISH_SPECIAL_POWER_READY => vec![pt::SIDE as u8, pt::SPECIAL_POWER as u8],
            ct::CONDITION_FALSE => vec![],
            _ => random_types(&mut g),
        };
        defs.push((format!("Cond{i}"), types));
    }
    for i in 0..at::NUM_ITEMS {
        let types: Vec<u8> = match i {
            at::TEAM_FOLLOW_WAYPOINTS => {
                vec![pt::TEAM as u8, pt::WAYPOINT_PATH as u8, pt::BOOLEAN as u8]
            }
            at::NAMED_SET_ATTITUDE | at::TEAM_SET_ATTITUDE => {
                vec![pt::UNIT as u8, pt::AI_MOOD as u8]
            }
            at::MAP_REVEAL_AT_WAYPOINT | at::MAP_SHROUD_AT_WAYPOINT => {
                vec![pt::WAYPOINT as u8, pt::REAL as u8, pt::SIDE as u8]
            }
            at::MAP_REVEAL_ALL
            | at::MAP_REVEAL_ALL_PERM
            | at::MAP_REVEAL_ALL_UNDO_PERM
            | at::MAP_SHROUD_ALL => vec![pt::SIDE as u8],
            at::SPEECH_PLAY => vec![pt::DIALOG as u8, pt::BOOLEAN as u8],
            at::CAMERA_MOD_SET_FINAL_ZOOM | at::CAMERA_MOD_SET_FINAL_PITCH => {
                vec![pt::REAL as u8, pt::PERCENT as u8, pt::PERCENT as u8]
            }
            at::MOVE_CAMERA_TO
            | at::MOVE_CAMERA_ALONG_WAYPOINT_PATH
            | at::CAMERA_LOOK_TOWARD_OBJECT => {
                vec![
                    pt::WAYPOINT as u8,
                    pt::REAL as u8,
                    pt::REAL as u8,
                    pt::REAL as u8,
                    pt::REAL as u8,
                ]
            }
            at::RESET_CAMERA | at::ZOOM_CAMERA | at::PITCH_CAMERA | at::ROTATE_CAMERA => {
                vec![
                    pt::REAL as u8,
                    pt::REAL as u8,
                    pt::REAL as u8,
                    pt::REAL as u8,
                ]
            }
            at::CAMERA_LOOK_TOWARD_WAYPOINT => vec![
                pt::WAYPOINT as u8,
                pt::REAL as u8,
                pt::REAL as u8,
                pt::REAL as u8,
                pt::BOOLEAN as u8,
            ],
            at::SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST => {
                vec![pt::SIDE as u8, pt::SPECIAL_POWER as u8]
            }
            at::SKIRMISH_BUILD_BASE_DEFENSE_FRONT | at::SKIRMISH_BUILD_BASE_DEFENSE_FLANK => vec![],
            _ => random_types(&mut g),
        };
        defs.push((format!("Act{i}"), types));
    }
    defs
}

/// The names given name keys first, in this order, on both sides: the
/// well-known keys, then the templates' internal names.
fn pre_interned() -> Vec<String> {
    WELL_KNOWN_KEY_NAMES.iter().map(|s| s.to_string()).collect()
}

fn setup_blob(defs: &[(String, Vec<u8>)]) -> Vec<u8> {
    let names = pre_interned();
    let mut b = Vec::new();
    b.extend_from_slice(&(names.len() as u32).to_le_bytes());
    for n in &names {
        b.extend_from_slice(&(n.len() as u16).to_le_bytes());
        b.extend_from_slice(n.as_bytes());
    }
    for (n, types) in defs {
        b.extend_from_slice(&(n.len() as u16).to_le_bytes());
        b.extend_from_slice(n.as_bytes());
        b.push(types.len() as u8);
        b.extend_from_slice(types);
    }
    b
}

/// The Rust context, with the names interned in the order the C++ setup
/// interns them.
fn new_world(defs: &[(String, Vec<u8>)]) -> SidesCtx {
    let mut g = NameKeyGenerator::new();
    g.init();
    for n in pre_interned() {
        g.name_to_key(n.as_bytes());
    }
    let mut conds = Vec::new();
    let mut acts = Vec::new();
    for (i, (n, types)) in defs.iter().enumerate() {
        let t = ScriptTemplate {
            internal_name_key: g.name_to_key(n.as_bytes()),
            parameters: types.iter().map(|&t| i32::from(t)).collect(),
        };
        if i < condition_type::NUM_ITEMS as usize {
            conds.push(t);
        } else {
            acts.push(t);
        }
    }
    SidesCtx::new(g, ScriptTemplates::new(conds, acts).expect("table sizes"))
}

// ---------------------------------------------------------------------------
// The log and the dumps (the format of wrap_sides.cpp)
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Log(Vec<u8>);

impl Log {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn s(&mut self, v: &[u8]) {
        self.u32(v.len() as u32);
        self.0.extend_from_slice(v);
    }
    fn w(&mut self, v: &[u16]) {
        self.u32(v.len() as u32);
        for u in v {
            self.0.extend_from_slice(&u.to_le_bytes());
        }
    }
    fn bytes(&mut self, v: &[u8]) {
        self.0.extend_from_slice(v);
    }
}

fn dump_dict(l: &mut Log, d: &Dict, names: &NameKeyGenerator) {
    l.u32(d.get_pair_count() as u32);
    for n in 0..d.get_pair_count() {
        l.s(names.key_to_name(d.get_nth_key(n)));
        let t = d.get_nth_type(n);
        l.u8(t as i32 as u8);
        match t {
            DataType::Bool => l.u8(u8::from(d.try_get_nth_bool(n).expect("in range"))),
            DataType::Int => l.u32(d.try_get_nth_int(n).expect("in range") as u32),
            DataType::Real => l.f(d.try_get_nth_real(n).expect("in range")),
            DataType::AsciiString => l.s(&d.try_get_nth_ascii_string(n).expect("in range")),
            DataType::UnicodeString => l.w(&d.try_get_nth_unicode_string(n).expect("in range")),
            DataType::None => unreachable!("a pair always has a type"),
        }
    }
}

fn dump_param(l: &mut Log, p: &Parameter) {
    l.u32(p.param_type as u32);
    l.u8(u8::from(p.initialized));
    l.u32(p.int as u32);
    l.f(p.real);
    l.s(&p.string);
    l.f(p.coord.x);
    l.f(p.coord.y);
    l.f(p.coord.z);
    let set: Vec<u32> = (0..ObjectStatusMaskType::get_bit_names().len() as i32)
        .filter(|&i| p.object_status.test(i))
        .map(|i| i as u32)
        .collect();
    l.u32(set.len() as u32);
    for i in set {
        l.u32(i);
    }
}

fn dump_parms(l: &mut Log, parms: &[Option<Parameter>; MAX_PARMS], num: i32) {
    for p in parms.iter().take(num.clamp(0, MAX_PARMS as i32) as usize) {
        match p {
            Some(p) => {
                l.u8(1);
                dump_param(l, p);
            }
            None => l.u8(0),
        }
    }
}

fn dump_condition(l: &mut Log, c: &Condition) {
    l.u32(c.condition_type as u32);
    l.u32(c.num_parms as u32);
    l.u32(c.has_warnings as u32);
    l.u32(c.custom_data as u32);
    l.u32(c.custom_frame);
    dump_parms(l, &c.parms, c.num_parms);
}

fn dump_actions(l: &mut Log, v: &[ScriptAction]) {
    l.u32(v.len() as u32);
    for a in v {
        l.u32(a.action_type as u32);
        l.u32(a.num_parms as u32);
        l.u8(u8::from(a.has_warnings));
        dump_parms(l, &a.parms, a.num_parms);
    }
}

fn dump_script(l: &mut Log, s: &Script) {
    l.s(&s.script_name);
    l.s(&s.comment);
    l.s(&s.condition_comment);
    l.s(&s.action_comment);
    l.u32(s.delay_evaluation_seconds as u32);
    for b in [
        s.is_active,
        s.is_one_shot,
        s.is_subroutine,
        s.easy,
        s.normal,
        s.hard,
    ] {
        l.u8(u8::from(b));
    }
    l.u32(s.conditions.len() as u32);
    for o in &s.conditions {
        l.u32(o.conditions.len() as u32);
        for c in &o.conditions {
            dump_condition(l, c);
        }
    }
    dump_actions(l, &s.actions);
    dump_actions(l, &s.actions_false);
    l.u32(s.frame_to_evaluate_at);
    l.u8(u8::from(s.has_warnings));
    l.s(&s.condition_team_name);
    l.f(s.condition_time);
    l.f(s.cur_time);
    l.u32(s.condition_executed_count as u32);
}

fn dump_scripts(l: &mut Log, v: &[Script]) {
    l.u32(v.len() as u32);
    for s in v {
        dump_script(l, s);
    }
}

fn dump_script_list(l: &mut Log, sl: &ScriptList) {
    l.u32(sl.groups.len() as u32);
    for g in &sl.groups {
        l.s(&g.group_name);
        l.u8(u8::from(g.is_group_active));
        l.u8(u8::from(g.is_group_subroutine));
        l.u8(u8::from(g.has_warnings));
        dump_scripts(l, &g.scripts);
    }
    dump_scripts(l, &sl.scripts);
}

fn dump_build(l: &mut Log, b: &BuildListInfo) {
    l.s(&b.building_name);
    l.s(&b.template_name);
    l.f(b.location.x);
    l.f(b.location.y);
    l.f(b.location.z);
    l.f(b.rally_point_offset.x);
    l.f(b.rally_point_offset.y);
    l.f(b.angle);
    l.u8(u8::from(b.is_initially_built));
    l.u32(b.num_rebuilds);
    l.s(&b.script);
    l.u32(b.health as u32);
    for f in [
        b.whiner,
        b.unsellable,
        b.repairable,
        b.automatically_build,
        b.selected,
    ] {
        l.u8(u8::from(f));
    }
    l.u32(b.object_id as u32);
    l.u32(b.object_timestamp);
    l.u8(u8::from(b.under_construction));
    for g in b.resource_gatherers {
        l.u32(g as u32);
    }
    l.u8(u8::from(b.is_supply_building));
    l.u32(b.desired_gatherers as u32);
    l.u32(b.current_gatherers as u32);
    l.u8(u8::from(b.priority_build));
}

fn dump_side(l: &mut Log, s: &SidesInfo, names: &NameKeyGenerator) {
    dump_dict(l, &s.dict, names);
    l.u32(s.build_list.len() as u32);
    for b in &s.build_list {
        dump_build(l, b);
    }
    match &s.scripts {
        Some(sl) => {
            l.u8(1);
            dump_script_list(l, sl);
        }
        None => l.u8(0),
    }
}

fn dump_state(l: &mut Log, c: &SidesCtx) {
    l.u8(b'S');
    let n = &c.names.generator;
    l.u32(c.sides.get_num_sides() as u32);
    for i in 0..c.sides.get_num_sides() {
        dump_side(l, c.sides.get_side_info(i).expect("in range"), n);
    }
    l.u32(c.sides.get_num_skirmish_sides() as u32);
    for i in 0..c.sides.get_num_skirmish_sides() {
        dump_side(l, c.sides.get_skirmish_side_info(i).expect("in range"), n);
    }
    l.u32(c.sides.get_num_teams() as u32);
    for i in 0..c.sides.get_num_teams() {
        dump_dict(l, &c.sides.get_team_info(i).expect("in range").dict, n);
    }
    l.u32(c.sides.get_num_skirmish_teams() as u32);
    for i in 0..c.sides.get_num_skirmish_teams() {
        dump_dict(
            l,
            &c.sides.get_skirmish_team_info(i).expect("in range").dict,
            n,
        );
    }
}

// ---------------------------------------------------------------------------
// The Rust side of ref_sides_run
// ---------------------------------------------------------------------------

struct Rd<'a> {
    p: &'a [u8],
    i: usize,
}

impl Rd<'_> {
    fn u8(&mut self) -> usize {
        let v = self.p.get(self.i).copied().unwrap_or(0);
        self.i = (self.i + 1).min(self.p.len() + 1);
        usize::from(v)
    }
    fn i8(&mut self) -> i32 {
        i32::from(self.u8() as u8 as i8)
    }
    fn str(&mut self) -> Vec<u8> {
        let l = self.u8();
        (0..l).map(|_| self.u8() as u8).collect()
    }
}

/// What the compared cases held and did (printed with the result).
#[derive(Default, Debug)]
struct Stats {
    sides: u64,
    build_entries: u64,
    teams: u64,
    script_lists: u64,
    scripts: u64,
    groups: u64,
    or_clauses: u64,
    conditions: u64,
    conditions_made_false: u64,
    actions: u64,
    actions_made_noop: u64,
    parameters: u64,
    ops_run: BTreeMap<usize, u64>,
    ops_skipped: BTreeMap<usize, u64>,
    skirmish_files_given: u64,
    skirmish_files_read: u64,
    skirmish_scripts_applied: u64,
    written_bytes: u64,
}

fn count_script_list(st: &mut Stats, sl: &ScriptList) {
    st.script_lists += 1;
    let mut scripts: Vec<&Script> = sl.scripts.iter().collect();
    for g in &sl.groups {
        st.groups += 1;
        scripts.extend(g.scripts.iter());
    }
    for s in scripts {
        st.scripts += 1;
        for o in &s.conditions {
            st.or_clauses += 1;
            for c in &o.conditions {
                st.conditions += 1;
                st.conditions_made_false += u64::from(
                    c.condition_type == condition_type::CONDITION_FALSE && c.num_parms == 0,
                );
                st.parameters += c.num_parms.max(0) as u64;
            }
        }
        for a in s.actions.iter().chain(&s.actions_false) {
            st.actions += 1;
            st.actions_made_noop +=
                u64::from(a.action_type == action_type::NO_OP && a.num_parms == 0);
            st.parameters += a.num_parms.max(0) as u64;
        }
    }
}

fn script_list(c: &mut SidesCtx, side: usize) -> Option<&mut ScriptList> {
    if (side as i32) < c.sides.get_num_sides() {
        c.sides
            .get_side_info_mut(side as i32)
            .expect("in range")
            .scripts
            .as_mut()
    } else {
        None
    }
}

fn thrown(l: &mut Log, r: Result<(), MapDataError>) -> Result<(), MapDataError> {
    match r {
        Err(MapDataError::Thrown(code)) => {
            l.u8(b'T');
            l.u32(code);
            Ok(())
        }
        other => other,
    }
}

/// `runOp` of wrap_sides.cpp.
fn run_op(
    c: &mut SidesCtx,
    rd: &mut Rd,
    l: &mut Log,
    st: &mut Stats,
    skirmish_given: bool,
) -> Result<(), MapDataError> {
    let op = rd.u8();
    l.u8(b'o');
    l.u8(op as u8);
    let before = l.0.len();
    let templates = c.templates.clone();
    let skip = |l: &mut Log| l.u8(b'x');
    match op {
        0 => {
            let i = rd.i8();
            c.sides.remove_side(i, &mut c.scripts.cur_id, &templates)?;
        }
        1 => {
            let i = rd.i8();
            c.sides.remove_team(i);
        }
        2 => {
            let n = rd.str();
            c.sides.add_player_by_template(&mut c.names, &n)?;
        }
        3 => {
            let m = c.sides.validate_sides(&mut c.names)?;
            l.u8(u8::from(m));
        }
        4 => {
            let n = rd.str();
            l.u32(
                c.sides
                    .find_side_info(&mut c.names, &n)
                    .map_or(-1i32, |i| i as i32) as u32,
            );
        }
        5 => {
            let n = rd.str();
            l.u32(
                c.sides
                    .find_team_info(&mut c.names, &n)
                    .map_or(-1i32, |i| i as i32) as u32,
            );
        }
        6 => {
            let i = rd.u8();
            if (i as i32) < c.sides.get_num_teams() {
                let t = c.sides.get_team_info(i as i32)?.clone();
                l.u8(u8::from(c.sides.is_player_default_team(&mut c.names, &t)));
            } else {
                skip(l);
            }
        }
        7 => {
            // whether the standard scripts file is read: no non-civilian side has any script
            let got_scripts = (0..c.sides.get_num_sides()).any(|i| {
                let side = c.sides.get_side_info(i).expect("in range");
                let faction = c.names.string(&side.dict, wk::PLAYER_FACTION);
                faction != b"FactionCivilian"
                    && side
                        .scripts
                        .as_ref()
                        .is_some_and(|sl| !sl.scripts.is_empty() || !sl.groups.is_empty())
            });
            thrown(l, c.prepare_for_mp_or_skirmish())?;
            if skirmish_given && !got_scripts {
                st.skirmish_files_read += 1;
                if (0..c.sides.get_num_skirmish_sides()).any(|i| {
                    c.sides
                        .get_skirmish_side_info(i)
                        .expect("in range")
                        .scripts
                        .as_ref()
                        .is_some_and(|sl| !sl.scripts.is_empty() || !sl.groups.is_empty())
                }) {
                    st.skirmish_scripts_applied += 1;
                }
            }
        }
        8 => {
            let (s, d) = (rd.u8(), rd.u8());
            let (q, tpl, nn) = (rd.str(), rd.str(), rd.str());
            let n = c.sides.get_num_sides() as usize;
            if s >= n || d >= n {
                skip(l);
            } else {
                let nl = match &c.sides.get_side_info(s as i32)?.scripts {
                    Some(src) => Some(src.duplicate_and_qualify(
                        &mut c.scripts.cur_id,
                        &templates,
                        &q,
                        &tpl,
                        &nn,
                    )?),
                    None => None,
                };
                c.sides.get_side_info_mut(d as i32)?.scripts = nl;
            }
        }
        9 => {
            let (s, b, pos) = (rd.u8(), rd.u8(), rd.i8());
            let ok = (s as i32) < c.sides.get_num_sides()
                && b < c.sides.get_side_info(s as i32)?.build_list.len();
            if ok {
                c.sides
                    .get_side_info_mut(s as i32)?
                    .reorder_in_build_list(b, pos)?;
            } else {
                skip(l);
            }
        }
        10 | 11 => {
            let (s, i) = (rd.u8(), rd.u8());
            match script_list(c, s) {
                Some(sl) => {
                    if op == 10 {
                        if i < sl.scripts.len() {
                            sl.delete_script(i)?;
                        } else {
                            skip(l);
                        }
                    } else if i < sl.groups.len() {
                        sl.delete_group(i)?;
                    } else {
                        skip(l);
                    }
                }
                None => skip(l),
            }
        }
        12 => {
            let (s, g, i) = (rd.u8(), rd.u8(), rd.u8());
            match script_list(c, s)
                .and_then(|sl| sl.groups.get_mut(g))
                .filter(|gr| i < gr.scripts.len())
            {
                Some(gr) => gr.delete_script(i)?,
                None => skip(l),
            }
        }
        13 | 14 | 16 => {
            let (s, i, a) = (rd.u8(), rd.u8(), rd.u8());
            match script_list(c, s).and_then(|sl| sl.scripts.get_mut(i)) {
                Some(sc) => {
                    let len = match op {
                        13 => sc.actions.len(),
                        16 => sc.actions_false.len(),
                        _ => sc.conditions.len(),
                    };
                    if a >= len {
                        skip(l);
                    } else {
                        match op {
                            13 => sc.delete_action(a)?,
                            16 => sc.delete_false_action(a)?,
                            _ => sc.delete_or_condition(a)?,
                        }
                    }
                }
                None => skip(l),
            }
        }
        15 => {
            let (s, i, o, cd) = (rd.u8(), rd.u8(), rd.u8(), rd.u8());
            match script_list(c, s)
                .and_then(|sl| sl.scripts.get_mut(i))
                .and_then(|sc| sc.conditions.get_mut(o))
                .filter(|or| cd < or.conditions.len())
            {
                Some(or) => or.delete_condition(cd)?,
                None => skip(l),
            }
        }
        17 => {
            let (s, i, pos) = (rd.u8(), rd.u8(), rd.i8());
            match script_list(c, s) {
                Some(sl) if i < sl.scripts.len() => {
                    let dup = sl.scripts[i].duplicate(&templates)?;
                    sl.add_script(dup, pos);
                }
                _ => skip(l),
            }
        }
        18 => c.sides.update_script_defaults(),
        19 => c.sides.reset_script_lists(),
        20 => {
            let (i, j) = (rd.u8(), rd.u8());
            let n = c.sides.get_num_sides() as usize;
            if i >= n || j >= n {
                skip(l);
            } else if i != j {
                // operator= ignores self-assignment
                let src = c.sides.get_side_info(i as i32)?.clone();
                c.sides.get_side_info_mut(j as i32)?.assign(
                    &src,
                    &mut c.scripts.cur_id,
                    &templates,
                )?;
            }
        }
        21 => {
            let (s, i, s2, i2) = (rd.u8(), rd.u8(), rd.u8(), rd.u8());
            let src = script_list(c, s2)
                .and_then(|sl| sl.scripts.get(i2))
                .map(|x| x.duplicate(&templates))
                .transpose()?;
            let dst_ok = script_list(c, s).is_some_and(|sl| i < sl.scripts.len());
            match (dst_ok, src) {
                (true, Some(mut tmp)) => {
                    script_list(c, s).expect("checked").scripts[i].update_from(&mut tmp)
                }
                _ => skip(l),
            }
        }
        22 => {
            let (s, g, s2, i2, pos) = (rd.u8(), rd.u8(), rd.u8(), rd.u8(), rd.i8());
            let src = script_list(c, s2)
                .and_then(|sl| sl.scripts.get(i2))
                .map(|x| x.duplicate(&templates))
                .transpose()?;
            let gr = script_list(c, s).and_then(|sl| sl.groups.get_mut(g));
            match (gr, src) {
                (Some(gr), Some(dup)) => gr.add_script(dup, pos),
                _ => skip(l),
            }
        }
        23 => {
            let (s, i, o) = (rd.u8(), rd.u8(), rd.u8());
            match script_list(c, s)
                .and_then(|sl| sl.scripts.get(i))
                .filter(|sc| o < sc.conditions.len())
            {
                Some(sc) => l.u32(
                    sc.find_previous_or_condition(o)?
                        .map_or(u32::MAX, |p| p as u32),
                ),
                None => skip(l),
            }
        }
        24 => {
            let (s, i, o, cd) = (rd.u8(), rd.u8(), rd.u8(), rd.u8());
            match script_list(c, s)
                .and_then(|sl| sl.scripts.get(i))
                .and_then(|sc| sc.conditions.get(o))
                .filter(|or| cd < or.conditions.len())
            {
                Some(or) => l.u32(
                    or.find_previous_condition(cd)?
                        .map_or(u32::MAX, |p| p as u32),
                ),
                None => skip(l),
            }
        }
        25 => {
            let (s, d) = (rd.u8(), rd.u8());
            let n = c.sides.get_num_sides() as usize;
            if s >= n || d >= n {
                skip(l);
            } else {
                let nl = match &c.sides.get_side_info(s as i32)?.scripts {
                    Some(src) => Some(src.duplicate(&mut c.scripts.cur_id, &templates)?),
                    None => None,
                };
                c.sides.get_side_info_mut(d as i32)?.scripts = nl;
            }
        }
        _ => l.u8(b'?'),
    }
    if l.0.len() > before && l.0[before..].contains(&b'x') && matches!(l.0.get(before), Some(b'x'))
    {
        *st.ops_skipped.entry(op).or_default() += 1;
    } else {
        *st.ops_run.entry(op).or_default() += 1;
    }
    Ok(())
}

/// Run the case; the operations are asked for one at a time, with the state
/// they will apply to (`next_op` returns `None` when there are no more), and
/// returned with the log for the C++ run.
fn rust_run(
    c: &mut SidesCtx,
    mode: i32,
    data: &[u8],
    skirmish: &[u8],
    st: &mut Stats,
    mut next_op: impl FnMut(&SidesCtx) -> Option<Vec<u8>>,
) -> Result<(Vec<u8>, Vec<u8>), MapDataError> {
    let mut all_ops = Vec::new();
    let mut l = Log::default();
    c.sides = SidesList::new();
    c.skirmish_scripts_file = if skirmish.is_empty() {
        None
    } else {
        Some(skirmish.to_vec())
    };
    let parsed = if mode == 0 {
        c.read_sides_chunk_file(data.to_vec())
    } else {
        c.read_scripts_chunk_file(data.to_vec())
    };
    match parsed {
        Ok(r) => {
            l.u8(b'R');
            l.u8(u8::from(r));
        }
        Err(MapDataError::Thrown(code)) => {
            l.u8(b'T');
            l.u32(code);
        }
        Err(e) => return Err(e),
    }
    if mode == 0 {
        dump_state(&mut l, c);
        st.sides += c.sides.get_num_sides() as u64;
        st.teams += c.sides.get_num_teams() as u64;
        for i in 0..c.sides.get_num_sides() {
            let side = c.sides.get_side_info(i).expect("in range");
            st.build_entries += side.build_list.len() as u64;
            if let Some(sl) = &side.scripts {
                count_script_list(st, sl);
            }
        }
        while let Some(op) = next_op(c) {
            all_ops.extend_from_slice(&op);
            let mut rd = Rd { p: &op, i: 0 };
            run_op(c, &mut rd, &mut l, st, !skirmish.is_empty())?;
            dump_state(&mut l, c);
        }
        let mut out = DataChunkOutput::new();
        thrown(&mut l, SidesList::write_sides_data_chunk(&mut out, c))?;
        let bytes = out.finish();
        st.written_bytes += bytes.len() as u64;
        l.u8(b'W');
        l.u32(bytes.len() as u32);
        l.bytes(&bytes);
        dump_state(&mut l, c);
    } else {
        let mut lists = ScriptList::get_read_scripts(&mut c.scripts);
        for x in lists.iter().flatten() {
            count_script_list(st, x);
        }
        let dump_lists = |l: &mut Log, lists: &[Option<ScriptList>]| {
            l.u8(b'L');
            l.u8(lists.len() as u8);
            for x in lists {
                match x {
                    Some(sl) => {
                        l.u8(1);
                        dump_script_list(l, sl);
                    }
                    None => l.u8(0),
                }
            }
        };
        dump_lists(&mut l, &lists);
        let mut out = DataChunkOutput::new();
        {
            let mut refs: Vec<Option<&mut ScriptList>> =
                lists.iter_mut().map(Option::as_mut).collect();
            thrown(
                &mut l,
                ScriptList::write_scripts_data_chunk(
                    &mut out,
                    &mut refs,
                    &c.templates,
                    &c.names.generator,
                ),
            )?;
        }
        let bytes = out.finish();
        st.written_bytes += bytes.len() as u64;
        l.u8(b'W');
        l.u32(bytes.len() as u32);
        l.bytes(&bytes);
        dump_lists(&mut l, &lists);
    }
    l.u8(b'I');
    let id = ScriptList::get_next_id(&mut c.scripts.cur_id);
    l.u32(id as u32);
    Ok((l.0, all_ops))
}

const LOG_CAP: usize = 1 << 27;

fn cpp_run(mode: i32, data: &[u8], ops: &[u8], skirmish: &[u8], buf: &mut Vec<u8>) -> Vec<u8> {
    buf.resize(LOG_CAP, 0);
    let mut n = 0i32;
    let st = unsafe {
        cpp::ref_sides_run(
            mode,
            data.as_ptr() as *const _,
            data.len() as i32,
            ops.as_ptr(),
            ops.len() as i32,
            skirmish.as_ptr() as *const _,
            skirmish.len() as i32,
            buf.as_mut_ptr(),
            LOG_CAP as i32,
            &mut n,
        )
    };
    assert_eq!(st, 0);
    assert!(n as usize <= LOG_CAP, "C++ log overflow ({n} bytes)");
    buf[..n as usize].to_vec()
}

// ---------------------------------------------------------------------------
// The generator
// ---------------------------------------------------------------------------

/// What the generator made, counted (printed at the end).
#[derive(Default, Debug)]
struct Cov {
    conditions: u64,
    actions: u64,
    old_condition_versions: u64,
    unmatched_keys: u64,
    rematched_keys: u64,
    wrong_counts: u64,
    kind_of_names: u64,
    kind_of_retired: u64,
    kind_of_by_index: u64,
    status_names: u64,
    fundamentalist: u64,
    obsolete_upgrades: u64,
    heals: BTreeMap<&'static str, u64>,
}

struct Toc {
    names: Vec<String>,
}

impl Toc {
    fn id(&mut self, n: &str) -> u32 {
        if let Some(p) = self.names.iter().position(|x| x == n) {
            return p as u32 + 1;
        }
        self.names.push(n.to_string());
        self.names.len() as u32
    }
}

struct Gn {
    g: Gen,
    clean: bool,
    toc: Toc,
    defs: Vec<(String, Vec<u8>)>,
    cov: Cov,
}

fn w32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn w16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn wstr(v: &mut Vec<u8>, s: &[u8]) {
    w16(v, s.len() as u16);
    v.extend_from_slice(s);
}

const PLAYERS: &[&str] = &["", "PlyrA", "PlyrB", "PlyrC", "Civ", "teamPlyrA", "x y"];
const FACTIONS: &[&str] = &[
    "",
    "FactionAmerica",
    "FactionChina",
    "FactionCivilian",
    "FactionGLA",
    "Other",
];
const SOME_STRINGS: &[&str] = &[
    "",
    "a",
    "Alpha",
    "script 1",
    "<This Team>",
    "<This Player>",
    "PlyrA",
    "teamPlyrA",
    "Beta_2",
    "x",
    "long name with spaces",
    "\u{e9}t\u{e9}",
];
const EXTRA_KEYS: &[&str] = &[
    "customA",
    "customB",
    "customC",
    "objectName",
    "weather",
    "initialCameraPosition",
];

impl Gn {
    fn below(&mut self, n: u32) -> u32 {
        self.g.below(n)
    }
    fn chance(&mut self, one_in: u32) -> bool {
        !self.clean && self.g.below(one_in) == 0
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.g.below(v.len() as u32) as usize]
    }

    fn string(&mut self) -> Vec<u8> {
        if self.below(6) == 0 {
            let n = self.below(20) as usize;
            return (0..n)
                .map(|_| {
                    [b' ', b'a', b'Z', b'_', b'0', b'<', 0xe9, b'\t'][self.g.below(8) as usize]
                })
                .collect();
        }
        let s = *self.pick(SOME_STRINGS);
        s.as_bytes().to_vec()
    }

    fn mixed_f32(&mut self) -> f32 {
        match self.below(10) {
            0 => self.g.special(),
            1 => f32::from_bits(self.g.u32()),
            _ => self.g.uniform(-1000.0, 1000.0),
        }
    }

    // ---- dicts ----
    fn dict(&mut self, v: &mut Vec<u8>, pairs: &[(&str, Vec<u8>, u8)]) {
        w16(v, pairs.len() as u16);
        for (name, val, t) in pairs {
            let id = self.toc.id(name);
            w32(v, (id << 8) | u32::from(*t));
            v.extend_from_slice(val);
        }
    }

    fn ascii_val(s: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        wstr(&mut v, s);
        v
    }

    fn player_dict(&mut self, v: &mut Vec<u8>, name: &str) {
        let allies = self.list_of_players();
        let enemies = self.list_of_players();
        let faction = *self.pick(FACTIONS);
        let mut pairs: Vec<(&str, Vec<u8>, u8)> = vec![
            ("playerName", Self::ascii_val(name.as_bytes()), 3),
            ("playerFaction", Self::ascii_val(faction.as_bytes()), 3),
            ("playerAllies", Self::ascii_val(allies.as_bytes()), 3),
            ("playerEnemies", Self::ascii_val(enemies.as_bytes()), 3),
        ];
        if self.below(3) == 0 {
            pairs.push(("playerIsHuman", vec![self.below(2) as u8], 0));
        }
        if self.below(4) == 0 {
            let units: Vec<u16> = (0..self.below(5))
                .map(|_| 0x41 + self.below(30) as u16)
                .collect();
            let mut u = Vec::new();
            w16(&mut u, units.len() as u16);
            for x in units {
                w16(&mut u, x);
            }
            pairs.push(("playerDisplayName", u, 4));
        }
        self.extra_pairs(&mut pairs);
        if self.chance(40) {
            // a wrong type for a key the code reads as an ASCII string
            pairs[0] = ("playerName", 7u32.to_le_bytes().to_vec(), 1);
        }
        self.dict(v, &pairs);
    }

    fn extra_pairs(&mut self, pairs: &mut Vec<(&'static str, Vec<u8>, u8)>) {
        for _ in 0..self.below(3) {
            let k = *self.pick(EXTRA_KEYS);
            if pairs.iter().any(|p| p.0 == k) {
                continue;
            }
            let t = self.below(5) as u8;
            let val = match t {
                0 => vec![self.below(2) as u8],
                1 => self.g.u32().to_le_bytes().to_vec(),
                2 => self.mixed_f32().to_le_bytes().to_vec(),
                3 => {
                    let s = self.string();
                    Self::ascii_val(&s)
                }
                _ => {
                    let mut u = Vec::new();
                    let n = self.below(4);
                    w16(&mut u, n as u16);
                    for _ in 0..n {
                        w16(&mut u, 0x30 + self.below(40) as u16);
                    }
                    u
                }
            };
            pairs.push((k, val, t));
        }
    }

    fn list_of_players(&mut self) -> String {
        let n = self.below(4);
        let mut s = String::new();
        for k in 0..n {
            if k > 0 {
                s.push_str(["", " ", "  ", "\t"][self.below(4) as usize]);
                s.push(' ');
            }
            s.push_str(self.pick(PLAYERS));
        }
        s
    }

    fn team_dict(&mut self, v: &mut Vec<u8>) {
        let name = *self.pick(&[
            "teamPlyrA",
            "teamPlyrB",
            "team",
            "PlyrA",
            "TeamX",
            "t1",
            "t2",
            "Civ",
        ]);
        let owner = *self.pick(PLAYERS);
        let mut pairs: Vec<(&str, Vec<u8>, u8)> = vec![
            ("teamName", Self::ascii_val(name.as_bytes()), 3),
            ("teamOwner", Self::ascii_val(owner.as_bytes()), 3),
        ];
        if self.below(2) == 0 {
            pairs.push(("teamIsSingleton", vec![self.below(2) as u8], 0));
        }
        self.extra_pairs(&mut pairs);
        self.dict(v, &pairs);
    }

    // ---- build lists ----
    fn build_entry(&mut self, v: &mut Vec<u8>, version: u16) {
        let n = self.string();
        wstr(v, &n);
        let n = self.string();
        wstr(v, &n);
        for _ in 0..3 {
            let f = self.mixed_f32();
            v.extend_from_slice(&f.to_le_bytes());
        }
        let f = self.mixed_f32();
        v.extend_from_slice(&f.to_le_bytes());
        v.push(self.below(3) as u8);
        w32(
            v,
            [0, 1, 3, 0xFFFF_FFFF, self.g.u32()][self.below(5) as usize],
        );
        if version >= 3 {
            let s = self.string();
            wstr(v, &s);
            w32(v, [100, 0, 1, 55, self.g.u32()][self.below(5) as usize]);
            for _ in 0..3 {
                v.push(self.below(3) as u8);
            }
        }
    }

    // ---- parameters ----
    fn param(&mut self, t: i32, v: &mut Vec<u8>) {
        use parameter_type as pt;
        w32(v, t as u32);
        if t == pt::COORD3D {
            for _ in 0..3 {
                let f = self.mixed_f32();
                v.extend_from_slice(&f.to_le_bytes());
            }
            return;
        }
        let mut int = match self.below(6) {
            0 => 0,
            1 => 1,
            2 => -1,
            3 => self.below(200) as i32,
            _ => self.g.i32(),
        };
        let real = self.mixed_f32();
        let s: Vec<u8> = match t {
            pt::OBJECT_TYPE if self.below(4) == 0 => {
                self.cov.fundamentalist += 1;
                let tail = if self.chance(60) {
                    "x".repeat(260)
                } else {
                    ["Infantry", "", "RocketBuggy"][self.below(3) as usize].to_string()
                };
                format!("Fundamentalist{tail}").into_bytes()
            }
            pt::UPGRADE if self.below(3) == 0 => {
                self.cov.obsolete_upgrades += 1;
                [
                    "Upgrade_AmericaRangerCaptureBuilding",
                    "Upgrade_ChinaRedguardCaptureBuilding",
                    "Upgrade_GLARebelCaptureBuilding",
                    "Upgrade_Other",
                ][self.below(4) as usize]
                    .as_bytes()
                    .to_vec()
            }
            pt::OBJECT_STATUS => {
                self.cov.status_names += 1;
                let names = ObjectStatusMaskType::get_bit_names();
                if self.below(5) == 0 {
                    b"NOT_A_STATUS".to_vec()
                } else {
                    let n = names[self.below(names.len() as u32) as usize];
                    if self.below(2) == 0 {
                        n.to_lowercase().into_bytes()
                    } else {
                        n.as_bytes().to_vec()
                    }
                }
            }
            pt::KIND_OF_PARAM => {
                let names = KindOfMaskType::get_bit_names();
                match self.below(10) {
                    0 | 1 => {
                        self.cov.kind_of_retired += 1;
                        ["CRUSHER", "Crushable", "OVERLAPPABLE", "MISSILE", "missile"]
                            [self.below(5) as usize]
                            .as_bytes()
                            .to_vec()
                    }
                    2 => {
                        self.cov.kind_of_by_index += 1;
                        int = [0, 1, 50, 115, 116, 117, -1, 5000][self.below(8) as usize];
                        if self.below(3) != 0 {
                            int = int.clamp(0, 116);
                        }
                        Vec::new()
                    }
                    3 => b"NOT_A_KIND".to_vec(),
                    _ => {
                        self.cov.kind_of_names += 1;
                        let n = names[self.below(names.len() as u32) as usize];
                        if self.below(2) == 0 {
                            n.to_lowercase().into_bytes()
                        } else {
                            n.as_bytes().to_vec()
                        }
                    }
                }
            }
            _ => self.string(),
        };
        w32(v, int as u32);
        v.extend_from_slice(&real.to_le_bytes());
        wstr(v, &s);
    }

    /// The parameters of a condition or an action: `n` of them, typed as the
    /// template says, sometimes not.
    fn params(&mut self, types: &[u8], n: i32, v: &mut Vec<u8>) {
        for i in 0..n.max(0) {
            let t = match types.get(i as usize) {
                Some(&t) if self.below(8) != 0 => i32::from(t),
                _ => self.below(52) as i32,
            };
            self.param(t, v);
        }
    }

    fn count_for(&mut self, template_count: usize) -> i32 {
        let n = template_count as i32;
        if self.below(10) == 0 {
            self.cov.wrong_counts += 1;
            return match self.below(6) {
                0 => n + 1,
                1 => (n - 1).max(0),
                2 => 2,
                3 => 3,
                4 => 1,
                _ => 0,
            };
        }
        if self.chance(60) {
            return [-1, 13, 40, i32::MIN][self.below(4) as usize];
        }
        n
    }

    // ---- scripts ----
    fn condition(&mut self, v: &mut Vec<u8>) -> u16 {
        self.cov.conditions += 1;
        let ver: u16 = match self.below(10) {
            0..=5 => 4,
            6 => 3,
            7 => 2,
            8 => 1,
            _ => 5,
        };
        if ver < 4 {
            self.cov.old_condition_versions += 1;
        }
        let special = [7, 17, 18, 39, 40, 41, 42, 84];
        let ty = match self.below(20) {
            0..=3 => *self.pick(&special),
            4 => 109 + self.below(19) as i32,
            5 if self.chance(30) => [128, -1, 5000][self.below(3) as usize],
            _ => self.below(109) as i32,
        };
        w32(v, ty as u32);
        let tmpl_idx = if (0..109).contains(&ty) {
            ty as usize
        } else {
            0
        };
        if ver >= 4 {
            let name = match self.below(10) {
                0 => {
                    self.cov.rematched_keys += 1;
                    format!("Cond{}", self.below(109))
                }
                1 => {
                    self.cov.unmatched_keys += 1;
                    "NotACondition".to_string()
                }
                _ => format!("Cond{tmpl_idx}"),
            };
            let id = self.toc.id(&name);
            w32(v, (id << 8) | 3);
        }
        let types = self.defs[tmpl_idx].1.clone();
        // the repairs of old files: the count the old file had
        let n = if ver < 2 && special[..7].contains(&ty) && self.below(4) != 0 {
            *self.cov.heals.entry("condition v1 surfaces").or_default() += 1;
            types.len() as i32 - 1
        } else if ver < 4 && ty == 84 && self.below(4) != 0 {
            *self
                .cov
                .heals
                .entry("skirmish special power ready")
                .or_default() += 1;
            1
        } else {
            self.count_for(types.len())
        };
        w32(v, n as u32);
        self.params(&types, n, v);
        ver
    }

    fn action(&mut self, v: &mut Vec<u8>) -> u16 {
        self.cov.actions += 1;
        let ver: u16 = if self.below(5) == 0 { 1 } else { 2 };
        let healed = [
            36, 45, 46, 90, 145, 102, 229, 230, 146, 83, 26, 27, 14, 17, 181, 19, 117, 118, 18,
            206, 251, 250,
        ];
        let ty = match self.below(20) {
            0..=5 => *self.pick(&healed),
            6 => 344 + self.below(168) as i32,
            7 if self.chance(30) => [512, -1, 100_000][self.below(3) as usize],
            _ => self.below(344) as i32,
        };
        w32(v, ty as u32);
        let tmpl_idx = if (0..344).contains(&ty) {
            ty as usize
        } else {
            0
        };
        if ver >= 2 {
            let name = match self.below(10) {
                0 => {
                    self.cov.rematched_keys += 1;
                    format!("Act{}", self.below(344))
                }
                1 => {
                    self.cov.unmatched_keys += 1;
                    "NotAnAction".to_string()
                }
                _ => format!("Act{tmpl_idx}"),
            };
            let id = self.toc.id(&name);
            w32(v, (id << 8) | 3);
        }
        let types = self.defs[109 + tmpl_idx].1.clone();
        let tc = types.len() as i32;
        // the count an old file had, for the types the parser repairs
        let old = match ty {
            36 => Some(2),
            45 | 46 => Some(2),
            90 | 145 => Some(2),
            102 | 229 | 230 | 146 => Some(0),
            83 => Some(1),
            26 | 27 => Some(1),
            14 | 17 | 181 => Some(3),
            19 | 117 | 118 | 18 => Some(2),
            206 => Some([2, 4][self.below(2) as usize]),
            251 => Some(1),
            250 => Some(1),
            _ => None,
        };
        let n = match old {
            Some(o) if self.below(4) != 0 => {
                *self.cov.heals.entry("action repairs").or_default() += 1;
                o
            }
            _ => self.count_for(tc as usize),
        };
        w32(v, n as u32);
        // an old SET_ATTITUDE has an Int where the new one has the mood
        if (ty == 45 || ty == 46) && n >= 2 && self.below(2) == 0 {
            self.param(parameter_type::UNIT, v);
            self.param(parameter_type::INT, v);
            self.params(&types, n - 2, v);
        } else {
            self.params(&types, n, v);
        }
        ver
    }

    fn chunk(&mut self, name: &str, ver: u16, body: &[u8]) -> Vec<u8> {
        let id = if self.chance(60) {
            self.below(300)
        } else {
            self.toc.id(name)
        };
        let mut v = Vec::new();
        w32(&mut v, id);
        w16(&mut v, ver);
        let size = if self.chance(40) {
            match self.below(4) {
                0 => 0,
                1 => body.len() as i32 + 1 + self.below(20) as i32,
                2 => (body.len() as i32 - 1 - self.below(10) as i32).max(0),
                _ => -1,
            }
        } else {
            body.len() as i32
        };
        w32(&mut v, size as u32);
        v.extend_from_slice(body);
        v
    }

    fn script(&mut self) -> Vec<u8> {
        let ver: u16 = if self.below(8) == 0 { 1 } else { 2 };
        let mut b = Vec::new();
        for _ in 0..4 {
            let s = self.string();
            wstr(&mut b, &s);
        }
        for _ in 0..6 {
            b.push([0, 1, 1, 2][self.below(4) as usize]);
        }
        if ver >= 2 {
            w32(&mut b, [0, 5, 100, 0xFFFF_FFFF][self.below(4) as usize]);
        }
        for _ in 0..self.below(3) {
            let mut o = Vec::new();
            for _ in 0..self.below(4) {
                let mut cb = Vec::new();
                let v = self.condition(&mut cb);
                let c = self.chunk("Condition", v, &cb);
                o.extend(c);
            }
            let c = self.chunk("OrCondition", 1, &o);
            b.extend(c);
        }
        for (name, count) in [
            ("ScriptAction", self.below(4)),
            ("ScriptActionFalse", self.below(3)),
        ] {
            for _ in 0..count {
                let mut ab = Vec::new();
                let v = self.action(&mut ab);
                let c = self.chunk(name, v, &ab);
                b.extend(c);
            }
        }
        self.chunk("Script", ver, &b)
    }

    fn group(&mut self) -> Vec<u8> {
        let ver: u16 = if self.below(6) == 0 { 1 } else { 2 };
        let mut b = Vec::new();
        let s = self.string();
        wstr(&mut b, &s);
        b.push(self.below(2) as u8);
        if ver == 2 {
            b.push(self.below(2) as u8);
        }
        for _ in 0..self.below(3) {
            let s = self.script();
            b.extend(s);
        }
        self.chunk("ScriptGroup", ver, &b)
    }

    fn script_list(&mut self) -> Vec<u8> {
        let mut b = Vec::new();
        let n_scripts = if self.below(5) == 0 {
            0
        } else {
            1 + self.below(3)
        };
        for _ in 0..n_scripts {
            let s = self.script();
            b.extend(s);
        }
        for _ in 0..self.below(3) {
            let s = self.group();
            b.extend(s);
        }
        self.chunk("ScriptList", 1, &b)
    }

    /// A `PlayerScriptsList` chunk of `lists` script lists (or, now and then, any number).
    fn player_scripts_list(&mut self, lists: u32) -> Vec<u8> {
        let mut b = Vec::new();
        let n = match self.below(30) {
            0 => 15 + self.below(4),
            1..=4 => self.below(lists + 2),
            _ => lists,
        };
        for _ in 0..n {
            let s = self.script_list();
            b.extend(s);
        }
        self.chunk("PlayerScriptsList", 5, &b)
    }

    fn sides_file(&mut self) -> Vec<u8> {
        let ver: u16 = [3, 3, 3, 2, 1][self.below(5) as usize];
        let mut b = Vec::new();
        let nsides = if self.below(40) == 0 {
            17 + self.below(3)
        } else {
            self.below(6)
        };
        w32(&mut b, nsides);
        for k in 0..nsides.min(16) {
            let name = if self.below(3) == 0 {
                *self.pick(PLAYERS)
            } else {
                PLAYERS[(k as usize) % PLAYERS.len()]
            };
            self.player_dict(&mut b, name);
            let nb = self.below(4);
            w32(&mut b, nb);
            for _ in 0..nb {
                self.build_entry(&mut b, ver);
            }
        }
        if ver >= 2 {
            let nt = self.below(6);
            w32(&mut b, nt);
            for _ in 0..nt {
                self.team_dict(&mut b);
            }
        }
        if self.below(12) != 0 {
            let s = self.player_scripts_list(nsides.min(16));
            b.extend(s);
            if self.below(10) == 0 {
                // a second one: the lists of the first are dropped when it is read
                let s = self.player_scripts_list(nsides.min(16));
                b.extend(s);
            }
        }
        let c = self.chunk("SidesList", ver, &b);
        self.finish_file(c)
    }

    fn scripts_file(&mut self) -> Vec<u8> {
        let lists = self.below(18);
        let mut c = self.player_scripts_list(lists);
        if self.below(8) == 0 {
            let lists = self.below(18);
            let more = self.player_scripts_list(lists);
            c.extend(more);
        }
        self.finish_file(c)
    }

    /// The file for the skirmish scripts: players, teams, scripts.
    fn skirmish_file(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut b = Vec::new();
        let ver: u16 = 1 + self.below(2) as u16;
        let readdicts = self.below(2);
        if ver >= 2 {
            w32(&mut b, readdicts);
        }
        let n = self.below(5);
        w32(&mut b, n);
        for _ in 0..n {
            let s = *self.pick(PLAYERS);
            wstr(&mut b, s.as_bytes());
            if ver >= 2 && readdicts != 0 {
                let mut d = Vec::new();
                let pairs = vec![("playerName", Self::ascii_val(b"q"), 3u8)];
                self.dict(&mut d, &pairs);
                b.extend(d);
            }
        }
        let c = self.chunk("ScriptsPlayers", ver, &b);
        out.extend(c);
        let mut b = Vec::new();
        for _ in 0..self.below(4) {
            self.team_dict(&mut b);
        }
        let c = self.chunk("ScriptTeams", 1, &b);
        out.extend(c);
        let lists = self.below(5);
        let s = self.player_scripts_list(lists);
        out.extend(s);
        self.finish_file(out)
    }

    /// Prepend the table of contents and apply the mutations.
    fn finish_file(&mut self, chunks: Vec<u8>) -> Vec<u8> {
        let mut v = if self.chance(30) {
            b"CkMq".to_vec()
        } else {
            b"CkMp".to_vec()
        };
        let count = if self.chance(25) {
            self.toc.names.len() as i32 + 1 + self.below(3) as i32
        } else {
            self.toc.names.len() as i32
        };
        w32(&mut v, count as u32);
        for (i, n) in self.toc.names.iter().enumerate() {
            v.push(n.len() as u8);
            v.extend_from_slice(n.as_bytes());
            w32(&mut v, i as u32 + 1);
        }
        v.extend(chunks);
        if self.chance(12) {
            let at = self.below(v.len() as u32) as usize;
            v.truncate(at);
        }
        if !v.is_empty() && self.chance(8) {
            for _ in 0..1 + self.below(3) {
                let p = self.below(v.len() as u32) as usize;
                v[p] = self.g.u32() as u8;
            }
        }
        if v.is_empty() {
            v.push(b'C');
        }
        v
    }

    /// The next operation, with indices that exist in `c` (nine times in
    /// ten) or not (the operation is then skipped, in both).
    fn smart_op(&mut self, c: &SidesCtx) -> Vec<u8> {
        let nsides = c.sides.get_num_sides() as u32;
        // (op, weight)
        const W: &[(u8, u32)] = &[
            (0, 2),
            (1, 3),
            (2, 3),
            (3, 4),
            (4, 3),
            (5, 3),
            (6, 3),
            (7, 3),
            (8, 6),
            (9, 5),
            (10, 6),
            (11, 6),
            (12, 6),
            (13, 6),
            (14, 6),
            (15, 6),
            (16, 5),
            (17, 6),
            (18, 1),
            (19, 1),
            (20, 4),
            (21, 6),
            (22, 6),
            (23, 4),
            (24, 4),
            (25, 5),
        ];
        let total: u32 = W.iter().map(|w| w.1).sum();
        let mut pick = self.below(total);
        let mut op = 0u8;
        for (o, w) in W {
            if pick < *w {
                op = *o;
                break;
            }
            pick -= w;
        }
        let mut v = vec![op];
        // a side index; `want` says what the operation needs of the side (mostly given)
        let sl_of = |s: u8| {
            c.sides
                .get_side_info(i32::from(s))
                .ok()
                .and_then(|x| x.scripts.as_ref())
        };
        let side_where = |g: &mut Gn, want: &dyn Fn(&ScriptList) -> bool| -> u8 {
            let ok: Vec<u8> = (0..nsides as u8)
                .filter(|&s| sl_of(s).is_some_and(want))
                .collect();
            if !ok.is_empty() && g.below(10) != 0 {
                ok[g.below(ok.len() as u32) as usize]
            } else if nsides > 0 && g.below(2) == 0 {
                g.below(nsides) as u8
            } else {
                g.below(20) as u8
            }
        };
        let side = |g: &mut Gn| -> u8 { side_where(g, &|_| true) };
        let idx = |g: &mut Gn, n: usize| -> u8 {
            if n > 0 && g.below(10) != 0 {
                g.below(n as u32) as u8
            } else {
                g.below(6) as u8
            }
        };
        let pos = |g: &mut Gn| (g.below(8) as i8 - 2) as u8;
        match op {
            0 | 1 => v.push((self.below(6) as i8 - 1) as u8),
            2 | 4 | 5 => {
                let s = *self.pick(&[
                    "",
                    "FactionAmerica",
                    "FactionCivilian",
                    "Other",
                    "PlyrA",
                    "teamPlyrA",
                    "Plyr",
                    "FactionX\u{e9}",
                    "PlyrB",
                    "Civ",
                ]);
                v.push(s.len() as u8);
                v.extend_from_slice(s.as_bytes());
            }
            3 | 7 | 18 | 19 => {}
            6 => {
                let n = c.sides.get_num_teams() as usize;
                v.push(idx(self, n));
            }
            8 | 25 => {
                let (s, d) = (side(self), side(self));
                v.push(s);
                v.push(d);
                if op == 8 {
                    for x in [
                        *self.pick(&["_q", "2", ""]),
                        *self.pick(&["PlyrA", "Plyr", "", "PlyrB"]),
                        *self.pick(&["PlyrB", "Z", "PlyrC"]),
                    ] {
                        v.push(x.len() as u8);
                        v.extend_from_slice(x.as_bytes());
                    }
                }
            }
            9 => {
                let s = side(self);
                v.push(s);
                let n = c
                    .sides
                    .get_side_info(i32::from(s))
                    .map_or(0, |x| x.build_list.len());
                v.push(idx(self, n));
                v.push(pos(self));
            }
            10 | 11 => {
                let s = if op == 10 {
                    side_where(self, &|l| !l.scripts.is_empty())
                } else {
                    side_where(self, &|l| !l.groups.is_empty())
                };
                v.push(s);
                let n = sl_of(s).map_or(0, |l| {
                    if op == 10 {
                        l.scripts.len()
                    } else {
                        l.groups.len()
                    }
                });
                v.push(idx(self, n));
            }
            12 | 22 => {
                let s = if op == 12 {
                    side_where(self, &|l| l.groups.iter().any(|g| !g.scripts.is_empty()))
                } else {
                    side_where(self, &|l| !l.groups.is_empty())
                };
                v.push(s);
                let ng = sl_of(s).map_or(0, |l| l.groups.len());
                let g = idx(self, ng);
                v.push(g);
                if op == 12 {
                    let n = sl_of(s)
                        .and_then(|l| l.groups.get(usize::from(g)))
                        .map_or(0, |gr| gr.scripts.len());
                    v.push(idx(self, n));
                } else {
                    let s2 = side_where(self, &|l| !l.scripts.is_empty());
                    v.push(s2);
                    let n = sl_of(s2).map_or(0, |l| l.scripts.len());
                    v.push(idx(self, n));
                    v.push(pos(self));
                }
            }
            13..=16 | 23 | 24 => {
                let s = match op {
                    13 => side_where(self, &|l| l.scripts.iter().any(|x| !x.actions.is_empty())),
                    16 => side_where(self, &|l| {
                        l.scripts.iter().any(|x| !x.actions_false.is_empty())
                    }),
                    14 | 23 => side_where(self, &|l| {
                        l.scripts.iter().any(|x| !x.conditions.is_empty())
                    }),
                    _ => side_where(self, &|l| {
                        l.scripts
                            .iter()
                            .any(|x| x.conditions.iter().any(|o| !o.conditions.is_empty()))
                    }),
                };
                v.push(s);
                let ns = sl_of(s).map_or(0, |l| l.scripts.len());
                let i = idx(self, ns);
                v.push(i);
                let sc = sl_of(s).and_then(|l| l.scripts.get(usize::from(i)));
                match op {
                    13 => v.push(idx(self, sc.map_or(0, |x| x.actions.len()))),
                    16 => v.push(idx(self, sc.map_or(0, |x| x.actions_false.len()))),
                    14 | 23 => v.push(idx(self, sc.map_or(0, |x| x.conditions.len()))),
                    _ => {
                        let no = sc.map_or(0, |x| x.conditions.len());
                        let o = idx(self, no);
                        v.push(o);
                        let nc = sc
                            .and_then(|x| x.conditions.get(usize::from(o)))
                            .map_or(0, |x| x.conditions.len());
                        v.push(idx(self, nc));
                    }
                }
            }
            17 => {
                let s = side_where(self, &|l| !l.scripts.is_empty());
                v.push(s);
                let n = sl_of(s).map_or(0, |l| l.scripts.len());
                v.push(idx(self, n));
                v.push(pos(self));
            }
            20 => {
                v.push(side(self));
                v.push(side(self));
            }
            _ => {
                // 21: s i s2 i2
                let s = side_where(self, &|l| !l.scripts.is_empty());
                v.push(s);
                let n = sl_of(s).map_or(0, |l| l.scripts.len());
                v.push(idx(self, n));
                let s2 = side_where(self, &|l| !l.scripts.is_empty());
                v.push(s2);
                let n2 = sl_of(s2).map_or(0, |l| l.scripts.len());
                v.push(idx(self, n2));
            }
        }
        v
    }
}

// ---------------------------------------------------------------------------
// The tests
// ---------------------------------------------------------------------------

/// The constants the translation names, in the order of
/// `ref_sides_constants`.
fn rust_constants() -> Vec<i32> {
    use action_type as at;
    use condition_type as ct;
    use parameter_type as pt;
    let mut v = vec![
        ct::NUM_ITEMS,
        at::NUM_ITEMS,
        pt::NUM_ITEMS,
        MAX_PARMS as i32,
        MAX_PLAYER_COUNT as i32,
        rts_lockstep_engine::common::kind_of::KINDOF_COUNT,
        rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_COUNT,
    ];
    v.extend([
        at::NO_OP,
        at::MOVE_CAMERA_TO,
        at::MOVE_CAMERA_ALONG_WAYPOINT_PATH,
        at::ROTATE_CAMERA,
        at::RESET_CAMERA,
        at::CAMERA_MOD_SET_FINAL_ZOOM,
        at::CAMERA_MOD_SET_FINAL_PITCH,
        at::TEAM_FOLLOW_WAYPOINTS,
        at::NAMED_SET_ATTITUDE,
        at::TEAM_SET_ATTITUDE,
        at::SPEECH_PLAY,
        at::MAP_REVEAL_AT_WAYPOINT,
        at::MAP_REVEAL_ALL,
        at::ZOOM_CAMERA,
        at::PITCH_CAMERA,
        at::MAP_SHROUD_AT_WAYPOINT,
        at::MAP_SHROUD_ALL,
        at::CAMERA_LOOK_TOWARD_OBJECT,
        at::CAMERA_LOOK_TOWARD_WAYPOINT,
        at::MAP_REVEAL_ALL_PERM,
        at::MAP_REVEAL_ALL_UNDO_PERM,
        at::SKIRMISH_BUILD_BASE_DEFENSE_FRONT,
        at::SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST,
        at::SKIRMISH_BUILD_BASE_DEFENSE_FLANK,
        ct::CONDITION_FALSE,
        ct::TEAM_INSIDE_AREA_PARTIALLY,
        ct::TEAM_INSIDE_AREA_ENTIRELY,
        ct::TEAM_OUTSIDE_AREA_ENTIRELY,
        ct::TEAM_ENTERED_AREA_ENTIRELY,
        ct::TEAM_ENTERED_AREA_PARTIALLY,
        ct::TEAM_EXITED_AREA_ENTIRELY,
        ct::TEAM_EXITED_AREA_PARTIALLY,
        ct::SKIRMISH_SPECIAL_POWER_READY,
        pt::INT,
        pt::REAL,
        pt::SCRIPT,
        pt::TEAM,
        pt::COUNTER,
        pt::FLAG,
        pt::COMPARISON,
        pt::WAYPOINT,
        pt::BOOLEAN,
        pt::TRIGGER_AREA,
        pt::TEXT_STRING,
        pt::SIDE,
        pt::SOUND,
        pt::SCRIPT_SUBROUTINE,
        pt::UNIT,
        pt::OBJECT_TYPE,
        pt::COORD3D,
        pt::ANGLE,
        pt::TEAM_STATE,
        pt::RELATION,
        pt::AI_MOOD,
        pt::DIALOG,
        pt::MUSIC,
        pt::MOVIE,
        pt::WAYPOINT_PATH,
        pt::LOCALIZED_TEXT,
        pt::BRIDGE,
        pt::KIND_OF_PARAM,
        pt::ATTACK_PRIORITY_SET,
        pt::RADAR_EVENT_TYPE,
        pt::SPECIAL_POWER,
        pt::SCIENCE,
        pt::UPGRADE,
        pt::COMMANDBUTTON_ABILITY,
        pt::BOUNDARY,
        pt::BUILDABLE,
        pt::SURFACES_ALLOWED,
        pt::SHAKE_INTENSITY,
        pt::COMMAND_BUTTON,
        pt::FONT_NAME,
        pt::OBJECT_STATUS,
        pt::COMMANDBUTTON_ALL_ABILITIES,
        pt::SKIRMISH_WAYPOINT_PATH,
        pt::COLOR,
        pt::EMOTICON,
        pt::OBJECT_PANEL_FLAG,
        pt::FACTION_NAME,
        pt::OBJECT_TYPE_LIST,
        pt::REVEALNAME,
        pt::SCIENCE_AVAILABILITY,
        pt::LEFT_OR_RIGHT,
        pt::PERCENT,
    ]);
    v
}

/// Removes the directory of `DataChunkOutput`'s temporary file when the test ends.
struct TempDir(std::path::PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn setup_cpp(defs: &[(String, Vec<u8>)]) -> TempDir {
    let dir = std::env::temp_dir().join(format!("rts_sides_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let mut d = dir.to_str().expect("UTF-8 path").to_string();
    d.push(std::path::MAIN_SEPARATOR);
    let c = std::ffi::CString::new(d).expect("no NUL");
    let blob = setup_blob(defs);
    unsafe { cpp::ref_sides_setup(c.as_ptr(), blob.as_ptr(), blob.len() as i32) };
    TempDir(dir)
}

#[test]
fn constants_match_upstream() {
    let mut out = [0i32; 256];
    let n = unsafe { cpp::ref_sides_constants(out.as_mut_ptr()) } as usize;
    let want = rust_constants();
    let mut check = Check::new("Scripts.h constants");
    assert_eq!(
        n,
        want.len(),
        "the C++ list and the Rust list differ in length"
    );
    for (i, (r, c)) in want.iter().zip(&out[..n]).enumerate() {
        check.eq(*r, *c, || format!("constant #{i}"));
    }
    // enumerators the C++ cannot name (they are in Scripts.cpp / SidesList.cpp, not their headers)
    assert_eq!(sv::SCRIPTS_1, 5);
    assert_eq!(rts_lockstep_engine::logic::scripts::AT_END, 0x00FF_FFFF);
    check.finish();
}

/// Excluded cases, by kind.
#[derive(Default)]
struct Tally {
    files: u64,
    compared: u64,
    parse_true: u64,
    parse_false: u64,
    thrown: BTreeMap<u32, u64>,
    undefined: BTreeMap<String, u64>,
    toc_too_large: u64,
    log_bytes: u64,
}

fn merge_stats(into: &mut Stats, s: Stats, skirmish_given: bool) {
    into.sides += s.sides;
    into.build_entries += s.build_entries;
    into.teams += s.teams;
    into.script_lists += s.script_lists;
    into.scripts += s.scripts;
    into.groups += s.groups;
    into.or_clauses += s.or_clauses;
    into.conditions += s.conditions;
    into.conditions_made_false += s.conditions_made_false;
    into.actions += s.actions;
    into.actions_made_noop += s.actions_made_noop;
    into.parameters += s.parameters;
    into.written_bytes += s.written_bytes;
    into.skirmish_files_given += u64::from(skirmish_given);
    into.skirmish_files_read += s.skirmish_files_read;
    into.skirmish_scripts_applied += s.skirmish_scripts_applied;
    for (k, v) in s.ops_run {
        *into.ops_run.entry(k).or_default() += v;
    }
    for (k, v) in s.ops_skipped {
        *into.ops_skipped.entry(k).or_default() += v;
    }
}

fn count_log_facts(rust: &[u8], tally: &mut Tally) {
    match rust.first() {
        Some(b'R') if rust.get(1) == Some(&1) => tally.parse_true += 1,
        Some(b'R') => tally.parse_false += 1,
        Some(b'T') => {
            *tally
                .thrown
                .entry(u32::from_le_bytes(rust[1..5].try_into().expect("4 bytes")))
                .or_default() += 1
        }
        _ => {}
    }
}

/// A table of contents that declares more than 10^5 names (upstream allocates every one, up to 2^31).
fn toc_too_large(data: &[u8]) -> bool {
    data.len() >= 8
        && &data[..4] == b"CkMp"
        && i32::from_le_bytes(data[4..8].try_into().expect("4 bytes")) > 100_000
}

fn run_cases(
    world: &mut SidesCtx,
    mode: i32,
    cases: usize,
    seed: u64,
    check: &mut Check,
    tally: &mut Tally,
    stats: &mut Stats,
    cov_out: &mut Cov,
) {
    let defs = template_defs();
    let mut gn = Gn {
        g: Gen::new(seed),
        clean: false,
        toc: Toc { names: Vec::new() },
        defs: defs.clone(),
        cov: Cov::default(),
    };
    let mut buf = Vec::new();
    let no_ops = std::env::var("SIDES_NOOPS").is_ok();
    for case in 0..cases {
        gn.clean = case % 2 == 0;
        gn.toc = Toc { names: Vec::new() };
        let data = if mode == 0 {
            gn.sides_file()
        } else {
            gn.scripts_file()
        };
        let n_ops = if mode == 0 && !no_ops { gn.below(9) } else { 0 };
        let skirmish = if mode == 0 && n_ops > 0 && gn.below(2) == 0 {
            gn.toc = Toc { names: Vec::new() };
            gn.skirmish_file()
        } else {
            Vec::new()
        };
        tally.files += 1;
        if toc_too_large(&data) || toc_too_large(&skirmish) {
            tally.toc_too_large += 1;
            continue;
        }
        // run the Rust on a copy: a case that is undefined upstream must leave no trace
        let mut trial = world.clone();
        let mut case_stats = Stats::default();
        let mut done = 0;
        let r = rust_run(&mut trial, mode, &data, &skirmish, &mut case_stats, |c| {
            if done == n_ops {
                return None;
            }
            done += 1;
            Some(gn.smart_op(c))
        });
        let (rust, ops) = match r {
            Ok(x) => x,
            Err(MapDataError::Undefined(u)) => {
                *tally.undefined.entry(format!("{u:?}")).or_default() += 1;
                continue;
            }
            Err(e) => unreachable!("{e:?}"),
        };
        if trial.embedded_nul_strings != world.embedded_nul_strings {
            // a string with a 0 inside: the std::string stand-in keeps what follows it
            *tally
                .undefined
                .entry("StandInStringWithNul".to_string())
                .or_default() += 1;
            continue;
        }
        *world = trial;
        merge_stats(stats, case_stats, !skirmish.is_empty() && ops.contains(&7));
        let c = cpp_run(mode, &data, &ops, &skirmish, &mut buf);
        tally.compared += 1;
        tally.log_bytes += rust.len() as u64;
        count_log_facts(&rust, tally);
        let ctx = || {
            let at = rust
                .iter()
                .zip(&c)
                .position(|(a, b)| a != b)
                .unwrap_or(rust.len().min(c.len()));
            let lo = at.saturating_sub(24);
            format!(
                "mode {mode} case {case} (file {} bytes, ops {:02x?})\nfirst difference at byte {at} (rust log {} bytes, c++ log {})\nrust {:02x?}\nc++  {:02x?}",
                data.len(),
                ops,
                rust.len(),
                c.len(),
                &rust[lo..(at + 24).min(rust.len())],
                &c[lo..(at + 24).min(c.len())]
            )
        };
        if rust != c {
            if let Ok(dir) = std::env::var("SIDES_DUMP_DIR") {
                let _ = std::fs::write(format!("{dir}/m{mode}_c{case}.rust"), &rust);
                let _ = std::fs::write(format!("{dir}/m{mode}_c{case}.cpp"), &c);
                let _ = std::fs::write(format!("{dir}/m{mode}_c{case}.file"), &data);
                let _ = std::fs::write(format!("{dir}/m{mode}_c{case}.ops"), &ops);
                let _ = std::fs::write(format!("{dir}/m{mode}_c{case}.skirmish"), &skirmish);
            }
        }
        check.eq(rust == c, true, ctx);
    }
    *cov_out = gn.cov;
}

#[test]
fn sides_and_scripts_match_upstream() {
    let defs = template_defs();
    let _tmp = setup_cpp(&defs);
    let mut check = Check::new("SidesList / Scripts parse, ops, write, dumps");
    let mut tally = Tally::default();
    let mut cov0 = Cov::default();
    let mut stats = Stats::default();
    let n0: usize = std::env::var("SIDES_CASES0")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(6_000);
    let n1: usize = std::env::var("SIDES_CASES1")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2_000);
    let mut world = new_world(&defs);
    run_cases(
        &mut world, 0, n0, 0x51DE5, &mut check, &mut tally, &mut stats, &mut cov0,
    );
    let mut cov1 = Cov::default();
    run_cases(
        &mut world, 1, n1, 0x5C41B, &mut check, &mut tally, &mut stats, &mut cov1,
    );
    println!(
        "Sides/Scripts: {} files generated, {} compared (log bytes {}), parses true {} false {}, thrown {:?}; \
         excluded as undefined upstream: {} {:?}; excluded (TOC > 10^5 names): {}",
        tally.files,
        tally.compared,
        tally.log_bytes,
        tally.parse_true,
        tally.parse_false,
        tally.thrown,
        tally.undefined.values().sum::<u64>(),
        tally.undefined,
        tally.toc_too_large
    );
    println!("  compared cases held (after the parse): {stats:?}");
    println!("  generated (sides files): {cov0:?}");
    println!("  generated (scripts files): {cov1:?}");
    check.finish();
}
