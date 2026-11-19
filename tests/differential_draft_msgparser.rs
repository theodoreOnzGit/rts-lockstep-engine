#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameMessageParser.cpp and Include/GameNetwork/GameMessageParser.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::game_message_parser` against `GameMessageParser` (#215).
//!
//! # Method
//!
//! Both classes (GameMessageParser.h), the constructors, destructors and `addArgType` (GameMessageParser.cpp) and the argument type
//! enum (MessageStream.h) are cut verbatim (build.rs) into `differential/wrap_draft_msgparser.cpp`; the wrapper stands in for
//! `GameMessage` (the type list; the count is an `UnsignedByte`). 40,000 messages of 0 to 300 arguments (runs of equal types of
//! length 1 to 12 in most; random types in some; the whole 11-type enum; 256 or more arguments now and then, so that the
//! `UnsignedByte` count wraps) go through `GameMessageParser(msg)`, and 20,000 sequences of 0 to 30 `addArgType(type, count)` calls
//! (counts of any `i32`, the type any of the 12 values, `UNKNOWN` included) through the default constructor; the run list and
//! `getNumTypes()` are compared.
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! 0 mismatches: the draft is the same loop over a `Vec`.
//!
//! # Results
//!
//! Run 2026-11-19: `GameMessageParser(msg)` 40,000 compared, 0 mismatches; `addArgType` 20,000 compared, 0 mismatches. Prediction hit.
//! Plants 14301..14306: all 6 caught (18,664 to 35,176 of the 40,000 / 20,000 each); baseline 0. Not covered:
//! `getFirstArgumentType` (accessor of the first run; not compared).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 14301 the run count is not incremented when a run closes; 14302 the last run is not added; 14303 a new run
//! starts at 1; 14304 the argument count is one short; 14305 a run closes even when empty; 14306 `addArgType` inserts at the head.

mod support;

use rts_lockstep_engine::common::base_type::{Coord3D, ICoord2D, IRegion2D};
use rts_lockstep_engine::common::message_stream::{ArgumentDataType, MessageType, Messages};
use rts_lockstep_engine::drafts::sweep_gui_logic::game_message_parser::GameMessageParser;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_mp_from_message(types: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
    fn ref_mp_manual(pairs: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish(c: Check) {
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}

fn types(g: &mut Gen) -> Vec<i32> {
    let n = match g.below(10) {
        0 => 0,
        1 => 250 + g.below(51) as usize,
        _ => g.below(40) as usize,
    };
    let mut v = Vec::new();
    let mode = g.below(3);
    while v.len() < n {
        if mode == 0 {
            v.push(g.below(11) as i32);
        } else {
            let t = g.below(11) as i32;
            for _ in 0..(1 + g.below(12)) {
                v.push(t);
            }
        }
    }
    v.truncate(n);
    v
}

fn dump(p: &GameMessageParser) -> Vec<i32> {
    let mut o = vec![p.get_num_types()];
    for t in &p.types {
        o.push(t.get_type() as i32);
        o.push(t.get_arg_count());
    }
    o
}

fn message(t: &[i32]) -> (Messages, rts_lockstep_engine::common::message_stream::MessageId) {
    let mut m = Messages::new();
    let id = m.new_message(MessageType(0), 0);
    {
        let msg = m.get_mut(id).unwrap();
        for &x in t {
            match x {
                0 => msg.append_integer_argument(1),
                1 => msg.append_real_argument(1.0),
                2 => msg.append_boolean_argument(true),
                3 => msg.append_object_id_argument(1),
                4 => msg.append_drawable_id_argument(1),
                5 => msg.append_team_id_argument(1),
                6 => msg.append_location_argument(Coord3D { x: 1.0, y: 2.0, z: 3.0 }),
                7 => msg.append_pixel_argument(ICoord2D { x: 1, y: 2 }),
                8 => msg.append_pixel_region_argument(IRegion2D { lo: ICoord2D { x: 1, y: 2 }, hi: ICoord2D { x: 3, y: 4 } }),
                9 => msg.append_timestamp_argument(1),
                _ => msg.append_wide_char_argument(1),
            }
        }
    }
    (m, id)
}

#[test]
fn parser_from_message() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_3E55);
    let mut c = Check::new("GameMessageParser(msg)");
    let mut buf = vec![0i32; 4000];
    for case in 0..40_000 {
        let t = types(&mut g);
        let (m, id) = message(&t);
        let rust = dump(&GameMessageParser::from_message(m.get(id).unwrap()));
        let n = unsafe { ref_mp_from_message(t.as_ptr(), t.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(n >= 0);
        c.eq(rust, buf[..n as usize].to_vec(), || format!("case {case}: types {t:?}"));
    }
    finish(c);
}

#[test]
fn parser_add_arg_type() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_3E56);
    let mut c = Check::new("GameMessageParser::addArgType");
    let mut buf = vec![0i32; 4000];
    for case in 0..20_000 {
        let n = g.below(31) as usize;
        let mut pairs = Vec::new();
        for _ in 0..n {
            pairs.push(g.below(12) as i32);
            pairs.push(if g.below(3) == 0 { g.i32() } else { g.below(10) as i32 });
        }
        let mut p = GameMessageParser::new();
        for i in 0..n {
            p.add_arg_type(ArgumentDataType::from_i32(pairs[2 * i]).unwrap(), pairs[2 * i + 1]);
        }
        let rust = dump(&p);
        let k = unsafe { ref_mp_manual(pairs.as_ptr(), n as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(k >= 0);
        c.eq(rust, buf[..k as usize].to_vec(), || format!("case {case}: pairs {pairs:?}"));
    }
    finish(c);
}
