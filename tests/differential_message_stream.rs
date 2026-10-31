#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/MessageStream.cpp
//   GeneralsMD/Code/GameEngine/Include/Common/MessageStream.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `GameMessage`, `GameMessageList`, `MessageStream` and `CommandList` (`common::message_stream`) against the C++, layer 12
//! batch M1.
//!
//! # Method
//!
//! The C++ side is `MessageStream.cpp` and its header, unchanged, compiled by `differential/wrap_message_stream.cpp` on the
//! stand-ins of `differential/shim_msg` (the local player's index; empty headers for what only the debug build's
//! `isInvalidDebugCommand` uses) and the shared shim's zero-filling pool glue. It runs a program of operations on
//! `TheMessageStream`, `TheCommandList` and a third `GameMessageList`, naming messages by the order they were made:
//! making, deleting, appending, inserting and removing messages on any list, `containsMessageOfType`, appending every kind
//! of argument (in runs that pass 256, to wrap `m_argCount`), reading the union's sixteen bytes and the argument types at
//! any index, the message's fields and links, `getCommandTypeAsAsciiString`, the stream's own append and insert, the
//! command list's `reset` and `appendMessageList`, a walk of each list with its last message, attaching (with a
//! translator or `NULL`), finding and removing translators, `propagateMessages` with test translators that destroy some
//! messages and append others while the stream is being walked, `buildRegion`, and the local player's index. Each
//! operation writes a line; the Rust side runs the same program on [`Messages`] and [`Translators`] and writes the same
//! lines.
//!
//! An operation the Rust side reports as undefined (a message used after it was freed, an endless walk round a cycle
//! of links, a handle that was never made, or a link printed that points at a freed message, whose identity is the
//! allocator's) ends the Rust run there, and the C++ runs only the operations before it.
//!
//! The message types are checked separately: each of the 274 names of the release build's enumeration
//! (`common::message_types`, generated from the header) has the value the compiled header gives it, and
//! `getCommandTypeAsAsciiString` matches over every value from -3 to 2,100.
//!
//! # Results (2026-10-31, `MSG_CASES=20000`, the default)
//!
//! 20,000 programs, 372,103 operations, 376,390 log lines: **0 mismatches**. 13,459 programs cut at an undefined operation
//! (8,629 a message used after it was freed, 3,577 an endless walk round a cycle, 1,128 a handle never made, 125 a freed link
//! printed). The 274 type values and the names of -3 to 2,100: identical.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 9 of 9 caught. A plant that changes the Rust state lets the C++ run operations the Rust calls defined but
//! the C++ state makes undefined (a cycle, a freed message); the C++ then crashes or loops, and an alarm in the wrapper
//! ends a loop after 20 seconds.
//!
//! | id | planted bug | caught by |
//! |---|---|---|
//! | 3931 | `insertMessage` after the last message leaves `m_lastMessage` | the C++ loops (the alarm ends it): the Rust state, not the C++, is the one without a cycle |
//! | 3932 | `removeMessage` leaves the message's `m_list` | mismatches, then the C++ crashes on a state the Rust calls defined |
//! | 3933 | `m_argCount` saturates at 255 instead of wrapping | 187 programs |
//! | 3934 | `getArgumentDataType` tests `>` for `>=` | 2 programs |
//! | 3935 | equal translator priorities attached before, not after | mismatches, then the alarm |
//! | 3936 | `propagateMessages` reads `next` before the translator runs | mismatches, then the alarm |
//! | 3937 | a type at or above `MSG_COUNT` named `"UnknownMessage"` | 559 programs, and the name check |
//! | 3938 | `buildRegion`'s y always `dest` low | 1,577 programs |
//! | 3939 | a location's `z` stored in the fourth word | 122 programs |

use rts_lockstep_engine::common::base_type::{Coord3D, ICoord2D, IRegion2D};
use rts_lockstep_engine::common::message_stream::*;
use rts_lockstep_engine::differential as cpp;
use std::fmt::Write as _;

struct Gen(u64);

impl Gen {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn chance(&mut self, n: u32) -> bool {
        self.below(n) == 0
    }
}

type Op = [u32; 5];

fn gen_type(g: &mut Gen) -> u32 {
    match g.below(6) {
        0 => g.below(30),
        1 => 1000 + g.below(100),
        2 => (-(g.below(4) as i32)) as u32,
        3 => 1995 + g.below(15),
        _ => g.below(2100),
    }
}

fn gen_program(g: &mut Gen) -> Vec<Op> {
    let n = 5 + g.below(60);
    let mut made = 0u32;
    let mut translators = 0u32;
    let mut ops = Vec::new();
    for i in 0..n {
        let h = |g: &mut Gen, made: u32| if made == 0 || g.chance(60) { g.below(made + 2) } else { made - 1 - g.below(made.min(6)) };
        let k = if made == 0 || i < 2 { [0, 11][g.below(2) as usize] } else { match g.below(100) {
            0..=9 => 0,
            10..=11 => 1,
            12..=23 => 2,
            24..=29 => 3,
            30..=33 => 4,
            34..=36 => 5,
            37..=45 => 6,
            46..=51 => 7,
            52..=55 => 8,
            56..=58 => 9,
            59..=61 => 10,
            62..=71 => 11,
            72..=74 => 12,
            75..=77 => 13,
            78..=79 => 14,
            80..=86 => 15,
            87..=89 => 16,
            90 => 17,
            91 => 18,
            92..=95 => 19,
            96 => 20,
            97 => 21,
            _ => 22,
        } };
        let op: Op = match k {
            0 | 11 => {
                made += 1;
                [k, gen_type(g), 0, 0, 0]
            }
            12 => {
                let c = h(g, made);
                made += 1;
                [k, gen_type(g), 0, c, 0]
            }
            1 => [k, h(g, made), 0, 0, 0],
            2 | 4 => [k, h(g, made), g.below(3), 0, 0],
            3 => [k, h(g, made), g.below(3), h(g, made), 0],
            5 => [k, gen_type(g), g.below(3), 0, 0],
            6 => {
                let reps = if g.chance(8) { 240 + g.below(40) } else { g.below(3) };
                let c = if g.chance(3) { g.next() } else { g.below(100) };
                [k, h(g, made), g.below(11), c, (reps << 24) | (g.next() & 0x00ff_ffff)]
            }
            7 | 8 => {
                let i = match g.below(5) {
                    0 => (-(g.below(3) as i32)) as u32,
                    1 => 250 + g.below(12),
                    2 => g.below(30),
                    _ => g.below(5),
                };
                [k, h(g, made), i, 0, 0]
            }
            9 => [k, h(g, made), 0, 0, 0],
            10 => [k, gen_type(g), 0, 0, 0],
            13 => [k, 0, 0, 0, 0],
            14 => [k, h(g, made), 0, 0, 0],
            15 => [k, g.below(3), 0, 0, 0],
            16 => {
                translators += 1;
                [k, g.below(5), u32::from(g.chance(6)), g.below(97), 0]
            }
            17 | 18 => [k, g.below(translators + 2), 0, 0, 0],
            19 => [k, g.below(4), 0, 0, 0],
            20 => {
                let mut v = || if g.chance(2) { g.below(10) } else { g.next() };
                [k, v(), v(), v(), v()]
            }
            21 => [k, h(g, made), g.below(16), 0, 0],
            _ => [k, g.below(16), 0, 0, 0],
        };
        ops.push(op);
    }
    ops
}

/// A Rust operation that is undefined upstream (or whose result is the allocator's): the run stops before it.
struct Cut(String);

fn e<T>(r: Result<T, rts_lockstep_engine::common::ini::IniError>) -> Result<T, Cut> {
    r.map_err(|e| Cut(format!("{e:?}")))
}

struct Sim {
    m: Messages,
    xl: Translators<i32>,
    handles: Vec<MessageId>,
    local: i32,
    log: String,
    budget: i32,
}

fn list_of(l: u32) -> ListId {
    match l % 3 {
        0 => Messages::STREAM,
        1 => Messages::COMMAND_LIST,
        _ => ListId(2),
    }
}

fn handle_of(handles: &[MessageId], m: &Messages, id: Option<MessageId>) -> Result<i64, Cut> {
    match id {
        None => Ok(-1),
        Some(id) => {
            if m.get(id).is_none() {
                return Err(Cut("a link to a freed message printed".into()));
            }
            Ok(handles.iter().rposition(|h| *h == id).map_or(-2, |i| i as i64))
        }
    }
}

impl Sim {
    fn h(&self, a: u32) -> Result<MessageId, Cut> {
        self.handles.get(a as usize).copied().ok_or_else(|| Cut("a handle never made".into()))
    }

    fn line(&mut self, s: String) {
        self.log.push_str(&s);
        self.log.push('\n');
    }

    fn op(&mut self, w: &Op) -> Result<(), Cut> {
        let (a, b, c, d) = (w[1], w[2], w[3], w[4]);
        match w[0] {
            0 => {
                let id = self.m.new_message(MessageType(a as i32), self.local);
                self.handles.push(id);
                let p = self.m.get(id).expect("new").get_player_index();
                self.line(format!("new {} player {}", self.handles.len() - 1, p));
            }
            1 => {
                let id = self.h(a)?;
                e(self.m.delete_message(id))?;
                self.line(format!("delete {a}"));
            }
            2 => {
                let id = self.h(a)?;
                e(self.m.append_message(list_of(b), id))?;
                self.line(format!("append {a} to {}", b % 3));
            }
            3 => {
                let (id, after) = (self.h(a)?, self.h(c)?);
                e(self.m.insert_message(list_of(b), id, after))?;
                self.line(format!("insert {a} after {c} on {}", b % 3));
            }
            4 => {
                let id = self.h(a)?;
                e(self.m.remove_message(list_of(b), id))?;
                self.line(format!("remove {a} from {}", b % 3));
            }
            5 => {
                let r = e(self.m.contains_message_of_type(list_of(b), MessageType(a as i32)))?;
                self.line(format!("contains {} {}: {}", b % 3, a as i32, i32::from(r)));
            }
            6 => {
                let id = self.h(a)?;
                let msg = e(self.m.message_mut(id))?;
                let reps = (d >> 24) + 1;
                for r in 0..reps {
                    let x = c.wrapping_add(r);
                    let y = d ^ r.wrapping_mul(0x9e37_79b9);
                    let z = c.wrapping_mul(31).wrapping_add(d);
                    let q = d.wrapping_mul(17).wrapping_add(r);
                    let (rx, ry, rz) = (f32::from_bits(x), f32::from_bits(y), f32::from_bits(z));
                    match b % 11 {
                        0 => msg.append_integer_argument(x as i32),
                        1 => msg.append_real_argument(rx),
                        2 => msg.append_boolean_argument(x & 1 != 0),
                        3 => msg.append_object_id_argument(x),
                        4 => msg.append_drawable_id_argument(x),
                        5 => msg.append_team_id_argument(x),
                        6 => msg.append_location_argument(Coord3D { x: rx, y: ry, z: rz }),
                        7 => msg.append_pixel_argument(ICoord2D { x: x as i32, y: y as i32 }),
                        8 => msg.append_pixel_region_argument(IRegion2D {
                            lo: ICoord2D { x: x as i32, y: y as i32 },
                            hi: ICoord2D { x: z as i32, y: q as i32 },
                        }),
                        9 => msg.append_timestamp_argument(x),
                        _ => msg.append_wide_char_argument(x as u16),
                    }
                }
                let count = msg.get_argument_count();
                self.line(format!("args {a} kind {} x{reps} count {count}", b % 11));
            }
            7 => {
                let id = self.h(a)?;
                let v = e(self.m.message(id))?.get_argument(b as i32).0;
                self.line(format!("arg {a} {}: {:08x} {:08x} {:08x} {:08x}", b as i32, v[0], v[1], v[2], v[3]));
            }
            8 => {
                let id = self.h(a)?;
                let t = e(self.m.message(id))?.get_argument_data_type(b as i32);
                self.line(format!("argtype {a} {}: {}", b as i32, t as i32));
            }
            9 => {
                let id = self.h(a)?;
                let msg = e(self.m.message(id))?.clone();
                let next = handle_of(&self.handles, &self.m, msg.next())?;
                let prev = handle_of(&self.handles, &self.m, msg.prev())?;
                self.line(format!(
                    "msg {a} type {} player {} count {} next {next} prev {prev}",
                    msg.get_type().0,
                    msg.get_player_index(),
                    msg.get_argument_count()
                ));
            }
            10 => self.line(format!("name {} {}", a as i32, MessageType(a as i32).command_string())),
            11 => {
                let id = e(self.m.append_new_message(Messages::STREAM, MessageType(a as i32), self.local))?;
                self.handles.push(id);
                self.line(format!("stream append {}", self.handles.len() - 1));
            }
            12 => {
                let after = self.h(c)?;
                let id = e(self.m.insert_new_message(Messages::STREAM, MessageType(a as i32), after, self.local))?;
                self.handles.push(id);
                self.line(format!("stream insert {} after {c}", self.handles.len() - 1));
            }
            13 => {
                e(self.m.destroy_all_messages(Messages::COMMAND_LIST))?;
                self.line("command reset".into());
            }
            14 => {
                let id = self.h(a)?;
                e(self.m.append_message_list(Messages::COMMAND_LIST, Some(id)))?;
                self.line(format!("command appendlist {a}"));
            }
            15 => {
                let list = list_of(a);
                let ids = e(self.m.list_messages(list))?;
                let mut s = String::new();
                for id in ids {
                    let _ = write!(s, " {}", handle_of(&self.handles, &self.m, Some(id))?);
                }
                let last = handle_of(&self.handles, &self.m, self.m.last_message(list))?;
                self.line(format!("list {}:{s} last {last}", a % 3));
            }
            16 => {
                let id = self.xl.attach(if b & 1 == 1 { None } else { Some((c % 97) as i32) }, a);
                self.line(format!("attach {id}"));
            }
            17 => {
                let k = self.xl.find(a).copied().unwrap_or(-1);
                self.line(format!("find {a}: {k}"));
            }
            18 => {
                self.xl.remove(a);
                self.line(format!("detach {a}"));
            }
            19 => {
                self.budget = (a % 4) as i32;
                let (handles, log, budget, local) = (&mut self.handles, &mut self.log, &mut self.budget, self.local);
                let r = self.xl.propagate_messages(&mut self.m, |k, m, msg| {
                    let t = m.message(msg)?.get_type().0;
                    let me = handles.iter().rposition(|h| *h == msg).map_or(-2, |i| i as i64);
                    let _ = writeln!(log, "xlat {k} sees {me} type {t}");
                    if (t * 7 + *k).rem_euclid(5) == 0 && *budget > 0 {
                        *budget -= 1;
                        let ty = (t + *k + 1).rem_euclid(2010);
                        let id = m.append_new_message(Messages::STREAM, MessageType(ty), local)?;
                        handles.push(id);
                        let _ = writeln!(log, "xlat {k} appends {}", handles.len() - 1);
                    }
                    Ok(if (t + *k).rem_euclid(3) == 0 { Disposition::Destroy } else { Disposition::Keep })
                });
                e(r)?;
                self.line("propagated".into());
            }
            20 => {
                let r = build_region(ICoord2D { x: a as i32, y: b as i32 }, ICoord2D { x: c as i32, y: d as i32 });
                self.line(format!("region {} {} {} {}", r.lo.x, r.lo.y, r.hi.x, r.hi.y));
            }
            21 => {
                let id = self.h(a)?;
                e(self.m.message_mut(id))?.set_player_index(b as i32);
                self.line(format!("setplayer {a} {}", b as i32));
            }
            22 => {
                self.local = a as i32;
                self.line(format!("local {}", a as i32));
            }
            _ => self.line("op?".into()),
        }
        Ok(())
    }
}

/// The Rust log, and where (and why) it stopped at an undefined operation.
fn rust_run(ops: &[Op]) -> (String, Option<(usize, String)>) {
    let mut m = Messages::new();
    let extra = m.new_list();
    assert_eq!(extra, ListId(2));
    let mut s = Sim { m, xl: Translators::new(), handles: Vec::new(), local: 0, log: String::new(), budget: 0 };
    for (i, w) in ops.iter().enumerate() {
        // a translator's lines are written before an undefined use ends the propagation; the C++ does not run it
        let before = s.log.len();
        if let Err(Cut(why)) = s.op(w) {
            s.log.truncate(before);
            return (s.log, Some((i, why)));
        }
    }
    (s.log, None)
}

fn cpp_run(ops: &[Op]) -> String {
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let mut cap = 1 << 16;
    loop {
        let mut out = vec![0u8; cap];
        let len = unsafe { cpp::ref_msgs_run(flat.as_ptr(), ops.len() as i32, out.as_mut_ptr().cast(), cap as i32) } as usize;
        if len <= cap {
            out.truncate(len);
            return String::from_utf8(out).expect("UTF-8 log");
        }
        cap = len;
    }
}

#[test]
fn message_types_match_the_compiled_header() {
    for (i, (name, t)) in MESSAGE_TYPES.iter().enumerate() {
        let v = unsafe { cpp::ref_msgs_type_value(i as i32) };
        assert_eq!(v, t.0, "{name}");
    }
    assert_eq!(unsafe { cpp::ref_msgs_type_value(MESSAGE_TYPES.len() as i32) }, -1, "the list ends with the header's");
    let mut buf = [0u8; 64];
    for t in -3..2100 {
        let n = unsafe { cpp::ref_msgs_type_name(t, buf.as_mut_ptr().cast(), buf.len() as i32) } as usize;
        assert_eq!(MessageType(t).command_string().as_bytes(), &buf[..n], "type {t}");
    }
}

#[test]
fn message_stream_matches_upstream() {
    let cases: usize = std::env::var("MSG_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut g = Gen(0x4D53_0001_2345_6789);
    let (mut mismatches, mut cut, mut ops_run, mut lines) = (0usize, 0usize, 0usize, 0usize);
    let mut by_op = [0usize; 23];
    let mut cut_why = std::collections::BTreeMap::<String, usize>::new();
    for case in 0..cases {
        let ops = gen_program(&mut g);
        let (r, undefined_at) = rust_run(&ops);
        let n = undefined_at.as_ref().map_or(ops.len(), |u| u.0);
        if let Some((_, why)) = &undefined_at {
            cut += 1;
            *cut_why.entry(why.clone()).or_insert(0) += 1;
        }
        let c = cpp_run(&ops[..n]);
        ops_run += n;
        for o in &ops[..n] {
            by_op[o[0] as usize] += 1;
        }
        if r != c {
            mismatches += 1;
            if mismatches <= 3 {
                let (rl, cl): (Vec<&str>, Vec<&str>) = (r.lines().collect(), c.lines().collect());
                let at = rl.iter().zip(&cl).position(|(a, b)| a != b).unwrap_or(rl.len().min(cl.len()));
                println!("case {case}: ops {:?}\n  first difference at line {at}:\n  rust {:?}\n  c++  {:?}", &ops[..n], rl.get(at), cl.get(at));
            }
        } else {
            lines += r.lines().count();
        }
    }
    println!("M1: {cases} programs ({cut} cut at an undefined operation), {ops_run} operations, {lines} lines, {mismatches} mismatches; by kind {by_op:?}");
    println!("cut at: {cut_why:?}");
    assert_eq!(mismatches, 0);
}
