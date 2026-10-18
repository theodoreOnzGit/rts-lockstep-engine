// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `common::money` (`Money`) against upstream
//! `RTS/Money.cpp` (commit 0a05454d), compiled unchanged in
//! `differential/wrap_money.cpp` (see build.rs). Phase 1 layer 4.
//!
//! # Methodology
//!
//! 1. **Withdraw and deposit**, on random balances (ordinary, and near
//!    `UnsignedInt`'s top, where the deposit wraps), amounts (zero, ordinary,
//!    more than the balance, huge), player indices 0..8 and the `playSound`
//!    flag. Compared: the amount returned, the balance after, the number of
//!    income records (upstream's `ThePlayerList->getNthPlayer(i)` is non-null
//!    for these indices; the Rust returns the flag and the owning player makes
//!    the call), and the client events (sound id and player index) that
//!    `TheAudio->addAudioEvent` received.
//! 2. **`parseMoneyAmount`**, through the INI reader from one `MONEY` block
//!    with random values (signs, partial numbers, overflowing numbers). Where
//!    the Rust reports `Undefined` behaviour upstream (an overflowing
//!    `UnsignedInt`), the input is not sent to the C++ and is counted.
//! 3. **Xfer**: `XferSave` bytes against upstream's file, `XferLoad` of the
//!    same bytes on both sides, and the `XferDeepCRC`.
//!
//! The exception text of the INI comparison is not compared (see the geometry
//! test); the status and the amount are.
//!
//! **Planted bugs.** Each comparison also runs a deliberately wrong variant
//! (no clamp on the withdrawal, a saturating deposit, a parse that adds one,
//! a missing sound), which must be caught.
//!
//! # Results
//!
//! Recorded by `cargo test --release --features differential --test
//! differential_money -- --nocapture` (the `DIFF` and `PLANTED` lines).
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::ini::{BlockParse, FieldParse, Ini, IniLoadType, IniResult, UserData};
use rts_lockstep_engine::common::money::{parse_money_amount, Money};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferDeepCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::{ClientEvent, MoneySound};
use std::collections::BTreeMap;
use std::ffi::CString;
use support::{Check, Gen};

/// The client event as the C++ stand-in logs it: (MiscAudio sound id, player).
fn event_pair(e: &ClientEvent) -> (u32, u32) {
    match e {
        ClientEvent::MoneySound { sound: MoneySound::Deposit, player_index } => (1, *player_index as u32),
        ClientEvent::MoneySound { sound: MoneySound::Withdraw, player_index } => (2, *player_index as u32),
        // p1w4-thingtemplate: the event of `ThingFactory::newObject`'s drawable; the money code makes none
        ClientEvent::ObjectCreated { .. } => unreachable!("the money code makes no ObjectCreated event"),
        // p1w5-world: the other events of the unified client interface; the money code makes none
        ClientEvent::ObjectDestroyed { .. } | ClientEvent::Audio { .. } | ClientEvent::Terrain(_) => {
            unreachable!("the money code makes only MoneySound events")
        }
    }
}

fn gen_balance(g: &mut Gen) -> u32 {
    match g.below(10) {
        0 => u32::MAX - g.below(1000),
        1 => 0,
        2..=4 => g.below(100),
        _ => g.u32() >> g.below(31),
    }
}

fn gen_amount(g: &mut Gen) -> u32 {
    match g.below(10) {
        0 => 0,
        1 => u32::MAX,
        2..=5 => g.below(1000),
        _ => g.u32() >> g.below(31),
    }
}

/// The Rust side of `ref_money_op`: [returned, balance, income records, n,
/// then (sound id, player) for each event]. Planted: the withdrawal is not
/// clamped to the balance, and the deposit saturates instead of wrapping.
fn rust_op(op: u32, money0: u32, player0: i32, amount: u32, play_sound: bool, planted: bool) -> Vec<u32> {
    let mut m = Money::new();
    m.set_player_index(player0);
    if money0 != 0 {
        m.deposit(money0, false, &mut Vec::new());
    }
    let before = m.count_money();
    let mut events = Vec::new();
    let (mut returned, income) = if op == 0 {
        (m.withdraw(amount, play_sound, &mut events), 0)
    } else {
        (0, u32::from(m.deposit(amount, play_sound, &mut events)))
    };
    let mut balance = m.count_money();
    if planted {
        if op == 0 && amount > before {
            returned = amount;
        }
        if op == 1 && before.checked_add(amount).is_none() {
            balance = u32::MAX;
        }
    }
    let mut out = vec![returned, balance, income, events.len() as u32];
    out.extend(events.iter().flat_map(|e| {
        let (a, b) = event_pair(e);
        [a, b]
    }));
    out
}

/// The C++ side of `ref_money_op`, cut to the events it logged.
fn cpp_op(op: i32, money0: u32, player0: i32, amount: u32, play_sound: bool) -> Vec<u32> {
    let mut out = [0u32; 8];
    unsafe { cpp::ref_money_op(op, money0, player0, amount, i32::from(play_sound), out.as_mut_ptr()) };
    let n = out[3] as usize;
    let mut v = out[..4].to_vec();
    v.extend_from_slice(&out[4..4 + 2 * n]);
    v
}

/// The C++ stand-ins keep their log and counters in globals (wrap_money.cpp),
/// so the tests that call them run one at a time.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn money_withdraw_and_deposit_match() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    const CASES: u32 = 20_000;
    let mut g = Gen::new(0x3053_0001);
    let mut ok = Check::new("Money withdraw/deposit (+ client events)");
    let mut bug = Check::new("Money withdraw/deposit (planted)");
    for case in 0..CASES {
        let op = g.below(2);
        let money0 = gen_balance(&mut g);
        let player0 = g.below(8) as i32;
        let amount = gen_amount(&mut g);
        let play_sound = g.below(4) != 0;
        let input = || format!("case {case}: op {op}, balance {money0}, player {player0}, amount {amount}, sound {play_sound}");
        let rust = rust_op(op, money0, player0, amount, play_sound, false);
        let cpp_out = cpp_op(op as i32, money0, player0, amount, play_sound);
        ok.eq(rust, cpp_out.clone(), input);
        bug.eq(rust_op(op, money0, player0, amount, play_sound, true), cpp_out, input);
    }
    ok.finish();
    bug.finish_planted();
}

/// A `MONEY` block's handler: the one field, `parseMoneyAmount`.
fn parse_money_block(ini: &mut Ini<'_>, m: &mut Money) -> IniResult<()> {
    ini.init_from_ini(m, &MONEY_FIELDS)
}

static MONEY_FIELDS: [FieldParse<Money>; 2] = [
    FieldParse::new("DefaultStartingMoney", parse_money_amount, UserData::None),
    FieldParse::end(),
];

/// Planted slip: the parsed amount is one more than the file says.
fn planted_parse_money(ini: &mut Ini<'_>, m: &mut Money, _: &UserData) -> IniResult<()> {
    parse_money_amount(ini, m, &UserData::None)?;
    m.deposit(1, false, &mut Vec::new());
    Ok(())
}

static PLANTED_FIELDS: [FieldParse<Money>; 2] = [FieldParse::new("DefaultStartingMoney", planted_parse_money, UserData::None), FieldParse::end()];

fn planted_money_block(ini: &mut Ini<'_>, m: &mut Money) -> IniResult<()> {
    ini.init_from_ini(m, &PLANTED_FIELDS)
}

const BLOCKS: [BlockParse<Money>; 1] = [BlockParse { token: "MONEY", parse: parse_money_block }];
const PLANTED_BLOCKS: [BlockParse<Money>; 1] = [BlockParse { token: "MONEY", parse: planted_money_block }];

const VALUE_TOKENS: &[&str] = &["12abc", "1e", "1.5", "-1", "+5", "007", "0", "4294967295", "4294967296", "x", "", "99999999999"];

fn gen_money_text(g: &mut Gen) -> String {
    let mut s = String::from("MONEY\n");
    for _ in 0..g.below(4) {
        let name = if g.below(10) < 9 { "DefaultStartingMoney" } else { "DefaultStartingMony" };
        let value = match g.below(3) {
            0 => format!("{}", g.u32()),
            1 => format!("{}", g.below(100_000)),
            _ => VALUE_TOKENS[g.below(VALUE_TOKENS.len() as u32) as usize].to_string(),
        };
        s.push_str(&format!("  {name} = {value} ; comment\n"));
    }
    if g.below(10) < 9 {
        s.push_str("End\n");
    }
    s
}

#[test]
fn money_parse_amount_matches() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    const CASES: u32 = 10_000;
    let mut g = Gen::new(0x3053_0002);
    let mut ok = Check::new("Money::parseMoneyAmount (INI)");
    let mut bug = Check::new("Money::parseMoneyAmount (planted)");
    let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
    for case in 0..CASES {
        let text = gen_money_text(&mut g);
        let data = text.as_bytes();
        let input = || format!("case {case}: block {text:?}");
        let mut m = Money::new();
        let rust = Ini::load(b"money.ini", data, IniLoadType::Overwrite, None, &BLOCKS, &mut m);
        if let Err(e) = &rust {
            if e.is_undefined() {
                *excluded.entry(format!("{:?}", e.kind)).or_default() += 1;
                continue;
            }
        }
        let cname = CString::new("money.ini").expect("name");
        let mut out = [0u32; 2];
        let mut msg = vec![0u8; 4096];
        let mut msg_len = 0i32;
        unsafe {
            cpp::ref_money_parse(
                cname.as_ptr(),
                data.as_ptr() as *const _,
                data.len() as i32,
                out.as_mut_ptr(),
                msg.as_mut_ptr() as *mut _,
                msg.len() as i32,
                &mut msg_len,
            )
        };
        let cpp_outcome = (out[0] != 0, out[1]);
        ok.eq((rust.is_err(), m.count_money()), cpp_outcome, input);

        let mut mb = Money::new();
        let planted = Ini::load(b"money.ini", data, IniLoadType::Overwrite, None, &PLANTED_BLOCKS, &mut mb);
        bug.eq((planted.is_err(), mb.count_money()), cpp_outcome, input);
    }
    println!(
        "parseMoneyAmount: {CASES} blocks; excluded (Undefined upstream, not sent to C++): {} {:?}",
        excluded.values().sum::<u64>(),
        excluded
    );
    ok.finish();
    bug.finish_planted();
}

fn temp_path(tag: &str, n: u32) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("p1w1-money-{}-{tag}-{n}.bin", std::process::id()))
}

#[test]
fn money_xfer_matches() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    const CASES: u32 = 2_000;
    let mut g = Gen::new(0x3053_0003);
    let mut ok = Check::new("Money xfer (save, load, deep CRC)");
    let mut bug = Check::new("Money xfer (planted)");
    for case in 0..CASES {
        let money0 = gen_balance(&mut g);
        let player0 = g.below(8) as i32;
        let input = || format!("case {case}: balance {money0}, player {player0}");
        let mut m = Money::new();
        m.set_player_index(player0);
        if money0 != 0 {
            m.deposit(money0, false, &mut Vec::new());
        }
        let mut save = XferSave::new();
        save.open(b"money").expect("open");
        save.xfer_snapshot(&mut m).expect("save");
        save.close().expect("close");
        let rust_bytes = save.take_bytes();

        let path = temp_path("save", case);
        let cpath = CString::new(path.to_string_lossy().as_bytes()).expect("path");
        assert_eq!(unsafe { cpp::ref_money_save(cpath.as_ptr(), money0, player0) }, 0, "upstream XferSave threw");
        let cpp_bytes = std::fs::read(&path).expect("upstream wrote the file");
        let _ = std::fs::remove_file(&path);
        ok.eq(rust_bytes.clone(), cpp_bytes, input);

        let path2 = temp_path("load", case);
        std::fs::write(&path2, &rust_bytes).expect("write");
        let cpath2 = CString::new(path2.to_string_lossy().as_bytes()).expect("path");
        let mut out = [0u32; 2];
        unsafe { cpp::ref_money_load(cpath2.as_ptr(), money0, player0, out.as_mut_ptr()) };
        let _ = std::fs::remove_file(&path2);
        assert_eq!(out[0], 0, "upstream XferLoad threw");
        let mut back = Money::new();
        let mut load = XferLoad::new(rust_bytes.clone());
        load.open(b"money").expect("open");
        load.xfer_snapshot(&mut back).expect("load");
        load.close().expect("close");
        ok.eq(back.count_money(), out[1], input);
        bug.eq(back.count_money().wrapping_add(1), out[1], input);

        let mut crc = XferDeepCrc::new();
        crc.open(b"deep").expect("open");
        crc.xfer_snapshot(&mut m).expect("crc");
        crc.close().expect("close");
        let path3 = temp_path("crc", case);
        let cpath3 = CString::new(path3.to_string_lossy().as_bytes()).expect("path");
        let cpp_crc = unsafe { cpp::ref_money_deep_crc(cpath3.as_ptr(), money0, player0) };
        let _ = std::fs::remove_file(&path3);
        ok.eq(crc.get_crc(), cpp_crc, input);
    }
    ok.finish();
    bug.finish_planted();
}
