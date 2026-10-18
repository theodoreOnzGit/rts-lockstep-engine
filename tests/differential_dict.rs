// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `common::dict` against upstream
//! `GameEngine/Source/Common/Dict.cpp` and `Include/Common/Dict.h` (commit
//! 0a05454d), compiled unchanged (`differential/wrap_dict.cpp`).
//!
//! Method: 4,000 random scripts of up to 120 operations on four `Dict`
//! slots (construct with preallocation, `clear`, the five setters,
//! `remove`, `copyPairFrom`, assignment and copy construction, full dumps
//! through `getNth*`, the five keyed getters, `getNthKey`/`getNthType` at
//! any index, the `getNth*` value getters where upstream is defined). Keys
//! are drawn mostly from a small set (so that pairs are replaced, retyped,
//! removed and copied), plus keys from 2^24 (stored truncated), the key
//! `0xffffff` (which ties with `remove`'s rename) and 0. Reals are raw bit
//! patterns (NaN included: they are stored, never computed). The logs must
//! be identical byte for byte. `getNth*` at an index upstream reads out of
//! bounds is not sent to the C++; the Rust must return an error there.
//!
//! Planted bugs, each of which must be caught: a stable sort in place of the
//! Shell sort, keys stored untruncated, and `remove` deleting the pair in
//! place.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::dict::{DataType, Dict, DictError, DictValue};
use rts_lockstep_engine::common::name_key_generator::NameKeyType;
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

#[derive(Clone, Debug)]
enum Op {
    New(u8, i32),
    Clear(u8),
    Set(u8, u32, DictValue),
    Remove(u8, u32),
    CopyPair(u8, u8, u32),
    Assign(u8, u8),
    CopyConstruct(u8, u8),
    Dump(u8),
    Get(u8, u32),
    NthKeyType(u8, i32),
    NthValues(u8, i32),
}

fn encode(ops: &[Op]) -> Vec<u8> {
    let mut s = Vec::new();
    for op in ops {
        match op {
            Op::New(d, n) => {
                s.extend_from_slice(&[0, *d]);
                s.extend_from_slice(&n.to_le_bytes());
            }
            Op::Clear(d) => s.extend_from_slice(&[1, *d]),
            Op::Set(d, k, v) => {
                let code = match v {
                    DictValue::Bool(_) => 2,
                    DictValue::Int(_) => 3,
                    DictValue::Real(_) => 4,
                    DictValue::AsciiString(_) => 5,
                    DictValue::UnicodeString(_) => 6,
                };
                s.extend_from_slice(&[code, *d]);
                s.extend_from_slice(&k.to_le_bytes());
                match v {
                    DictValue::Bool(b) => s.push(u8::from(*b)),
                    DictValue::Int(i) => s.extend_from_slice(&i.to_le_bytes()),
                    DictValue::Real(f) => s.extend_from_slice(&f.to_bits().to_le_bytes()),
                    DictValue::AsciiString(a) => {
                        s.extend_from_slice(&(a.len() as u16).to_le_bytes());
                        s.extend_from_slice(a);
                    }
                    DictValue::UnicodeString(u) => {
                        s.extend_from_slice(&(u.len() as u16).to_le_bytes());
                        for c in u {
                            s.extend_from_slice(&c.to_le_bytes());
                        }
                    }
                }
            }
            Op::Remove(d, k) => {
                s.extend_from_slice(&[7, *d]);
                s.extend_from_slice(&k.to_le_bytes());
            }
            Op::CopyPair(d, t, k) => {
                s.extend_from_slice(&[8, *d, *t]);
                s.extend_from_slice(&k.to_le_bytes());
            }
            Op::Assign(d, t) => s.extend_from_slice(&[9, *d, *t]),
            Op::CopyConstruct(d, t) => s.extend_from_slice(&[10, *d, *t]),
            Op::Dump(d) => s.extend_from_slice(&[11, *d]),
            Op::Get(d, k) => {
                s.extend_from_slice(&[12, *d]);
                s.extend_from_slice(&k.to_le_bytes());
            }
            Op::NthKeyType(d, n) => {
                s.extend_from_slice(&[13, *d]);
                s.extend_from_slice(&n.to_le_bytes());
            }
            Op::NthValues(d, n) => {
                s.extend_from_slice(&[14, *d]);
                s.extend_from_slice(&n.to_le_bytes());
            }
        }
    }
    s
}

#[derive(Default)]
struct Log(Vec<u8>);
impl Log {
    fn u8(&mut self, v: bool) {
        self.0.push(u8::from(v));
    }
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    fn ascii(&mut self, s: &[u8]) {
        // the C++ AsciiString is a C string: up to the first NUL
        let s = &s[..s.iter().position(|&b| b == 0).unwrap_or(s.len())];
        self.u32(s.len() as u32);
        self.0.extend_from_slice(s);
    }
    fn uni(&mut self, s: &[u16]) {
        self.u32(s.len() as u32);
        for c in s {
            self.0.extend_from_slice(&c.to_le_bytes());
        }
    }
}

/// The Rust run, in the wrapper's log format; `None` if a `getNth*` value
/// read is undefined upstream (and the Rust returned the error).
fn run_rust(ops: &[Op]) -> Option<Vec<u8>> {
    let mut slot: [Dict; 4] = Default::default();
    let mut lg = Log::default();
    for op in ops {
        match op {
            Op::New(d, n) => slot[*d as usize] = Dict::with_preallocated(*n).expect("n <= MAX_LEN"),
            Op::Clear(d) => slot[*d as usize].clear(),
            Op::Set(d, k, v) => {
                let dict = &mut slot[*d as usize];
                let k = NameKeyType(*k);
                match v {
                    DictValue::Bool(b) => dict.set_bool(k, *b),
                    DictValue::Int(i) => dict.set_int(k, *i),
                    DictValue::Real(f) => dict.set_real(k, *f),
                    DictValue::AsciiString(a) => {
                        let a = &a[..a.iter().position(|&b| b == 0).unwrap_or(a.len())];
                        dict.set_ascii_string(k, a)
                    }
                    DictValue::UnicodeString(u) => dict.set_unicode_string(k, u),
                }
                .expect("within MAX_LEN");
            }
            Op::Remove(d, k) => {
                let r = slot[*d as usize].remove(NameKeyType(*k));
                lg.u8(r);
            }
            Op::CopyPair(d, t, k) => {
                let that = slot[*t as usize].clone();
                slot[*d as usize].copy_pair_from(&that, NameKeyType(*k)).expect("within MAX_LEN");
            }
            Op::Assign(d, t) | Op::CopyConstruct(d, t) => slot[*d as usize] = slot[*t as usize].clone(),
            Op::Dump(d) => {
                let dict = &slot[*d as usize];
                let c = dict.get_pair_count();
                lg.i32(c);
                for n in 0..c {
                    lg.u32(dict.get_nth_key(n).0);
                    let t = dict.get_nth_type(n);
                    lg.i32(t as i32);
                    match t {
                        DataType::Bool => lg.u8(dict.try_get_nth_bool(n).ok()?),
                        DataType::Int => lg.i32(dict.try_get_nth_int(n).ok()?),
                        DataType::Real => lg.f32(dict.try_get_nth_real(n).ok()?),
                        DataType::AsciiString => lg.ascii(&dict.try_get_nth_ascii_string(n).ok()?),
                        DataType::UnicodeString => lg.uni(&dict.try_get_nth_unicode_string(n).ok()?),
                        DataType::None => {}
                    }
                }
            }
            Op::Get(d, k) => {
                let dict = &slot[*d as usize];
                let k = NameKeyType(*k);
                lg.i32(dict.get_type(k) as i32);
                let (b, e) = dict.get_bool(k);
                lg.u8(b);
                lg.u8(e);
                let (i, e) = dict.get_int(k);
                lg.i32(i);
                lg.u8(e);
                let (f, e) = dict.get_real(k);
                lg.f32(f);
                lg.u8(e);
                let (a, e) = dict.get_ascii_string(k);
                lg.ascii(&a);
                lg.u8(e);
                let (u, e) = dict.get_unicode_string(k);
                lg.uni(&u);
                lg.u8(e);
            }
            Op::NthKeyType(d, n) => {
                let dict = &slot[*d as usize];
                lg.u32(dict.get_nth_key(*n).0);
                lg.i32(dict.get_nth_type(*n) as i32);
            }
            Op::NthValues(d, n) => {
                let dict = &slot[*d as usize];
                let b = match dict.try_get_nth_bool(*n) {
                    Ok(b) => b,
                    Err(DictError::IndexOutOfRange) => {
                        assert!(dict.has_storage() && (*n < 0 || *n >= dict.get_pair_count()));
                        return None;
                    }
                    Err(e) => panic!("{e}"),
                };
                lg.u8(b);
                lg.i32(dict.try_get_nth_int(*n).ok()?);
                lg.f32(dict.try_get_nth_real(*n).ok()?);
                lg.ascii(&dict.try_get_nth_ascii_string(*n).ok()?);
                lg.uni(&dict.try_get_nth_unicode_string(*n).ok()?);
            }
        }
    }
    Some(lg.0)
}

/// A deliberately wrong model for the planted bugs (keys and dumps only).
#[derive(Clone, Copy, PartialEq)]
enum Bug {
    StableSort,
    UntruncatedKeys,
    RemoveInPlace,
}

fn run_planted(ops: &[Op], bug: Bug) -> Vec<u8> {
    // pairs: (stored name, value), plus whether storage exists
    let mut slot: Vec<(bool, Vec<(u32, DictValue)>)> = vec![(false, Vec::new()); 4];
    let mut lg = Log::default();
    let store = |k: u32| if bug == Bug::UntruncatedKeys { k } else { k & 0xff_ffff };
    let find = |p: &[(u32, DictValue)], k: u32| -> Option<usize> {
        let (mut lo, mut hi) = (0i32, p.len() as i32);
        while lo < hi {
            let mid = ((lo + hi) - 1) >> 1;
            let m = p[mid as usize].0 as i32;
            if (k as i32) > m {
                lo = mid + 1;
            } else if (k as i32) < m {
                hi = mid;
            } else {
                return Some(mid as usize);
            }
        }
        None
    };
    let sort = |p: &mut Vec<(u32, DictValue)>| {
        if bug == Bug::StableSort {
            p.sort_by_key(|e| e.0);
            return;
        }
        let n = p.len() as i32;
        let mut gap = n >> 1;
        while gap > 0 {
            for i in gap..n {
                let mut j = i - gap;
                while j >= 0 && p[j as usize].0 > p[(j + gap) as usize].0 {
                    p.swap(j as usize, (j + gap) as usize);
                    j -= gap;
                }
            }
            gap >>= 1;
        }
    };
    let ty = |v: &DictValue| match v {
        DictValue::Bool(_) => 0,
        DictValue::Int(_) => 1,
        DictValue::Real(_) => 2,
        DictValue::AsciiString(_) => 3,
        DictValue::UnicodeString(_) => 4,
    };
    for op in ops {
        match op {
            Op::New(d, n) => slot[*d as usize] = (*n > 0, Vec::new()),
            Op::Clear(d) => slot[*d as usize] = (false, Vec::new()),
            Op::Set(d, k, v) => {
                let v = match v {
                    DictValue::AsciiString(a) => DictValue::AsciiString(a[..a.iter().position(|&b| b == 0).unwrap_or(a.len())].to_vec()),
                    v => v.clone(),
                };
                let s = &mut slot[*d as usize];
                s.0 = true;
                match find(&s.1, *k) {
                    Some(i) => s.1[i].1 = v,
                    None => s.1.push((store(*k), v)),
                }
                sort(&mut s.1);
            }
            Op::Remove(d, k) => {
                let s = &mut slot[*d as usize];
                match find(&s.1, *k) {
                    Some(i) => {
                        if bug == Bug::RemoveInPlace {
                            s.1.remove(i);
                        } else {
                            let kept = matches!(s.1[i].1, DictValue::Bool(true));
                            s.1[i] = (store(0x7fff_ffff), DictValue::Bool(kept));
                            sort(&mut s.1);
                            s.1.pop();
                        }
                        lg.u8(true);
                    }
                    None => lg.u8(false),
                }
            }
            Op::CopyPair(d, t, k) => {
                let that = slot[*t as usize].1.clone();
                let s = &mut slot[*d as usize];
                match find(&that, *k) {
                    Some(i) => {
                        s.0 = true;
                        let v = that[i].1.clone();
                        match find(&s.1, *k) {
                            Some(j) => s.1[j].1 = v,
                            None => s.1.push((store(*k), v)),
                        }
                        sort(&mut s.1);
                    }
                    None => {
                        if let Some(j) = find(&s.1, *k) {
                            let kept = matches!(s.1[j].1, DictValue::Bool(true));
                            s.1[j] = (store(0x7fff_ffff), DictValue::Bool(kept));
                            sort(&mut s.1);
                            s.1.pop();
                        }
                    }
                }
            }
            Op::Assign(d, t) | Op::CopyConstruct(d, t) => slot[*d as usize] = slot[*t as usize].clone(),
            Op::Dump(d) => {
                let s = &slot[*d as usize];
                lg.i32(s.1.len() as i32);
                for (k, v) in &s.1 {
                    lg.u32(*k & if bug == Bug::UntruncatedKeys { u32::MAX } else { 0xff_ffff });
                    lg.i32(ty(v));
                    match v {
                        DictValue::Bool(b) => lg.u8(*b),
                        DictValue::Int(i) => lg.i32(*i),
                        DictValue::Real(f) => lg.f32(*f),
                        DictValue::AsciiString(a) => lg.ascii(a),
                        DictValue::UnicodeString(u) => lg.uni(u),
                    }
                }
            }
            Op::Get(d, k) => {
                let s = &slot[*d as usize];
                let p = find(&s.1, *k).map(|i| &s.1[i].1);
                lg.i32(p.map_or(-1, ty));
                let b = matches!(p, Some(DictValue::Bool(true)));
                lg.u8(b);
                lg.u8(matches!(p, Some(DictValue::Bool(_))));
                lg.i32(if let Some(DictValue::Int(i)) = p { *i } else { 0 });
                lg.u8(matches!(p, Some(DictValue::Int(_))));
                lg.f32(if let Some(DictValue::Real(f)) = p { *f } else { 0.0 });
                lg.u8(matches!(p, Some(DictValue::Real(_))));
                lg.ascii(if let Some(DictValue::AsciiString(a)) = p { a } else { &[] });
                lg.u8(matches!(p, Some(DictValue::AsciiString(_))));
                lg.uni(if let Some(DictValue::UnicodeString(u)) = p { u } else { &[] });
                lg.u8(matches!(p, Some(DictValue::UnicodeString(_))));
            }
            Op::NthKeyType(d, n) => {
                let s = &slot[*d as usize];
                match (*n >= 0).then(|| s.1.get(*n as usize)).flatten() {
                    Some((k, v)) => {
                        lg.u32(*k);
                        lg.i32(ty(v));
                    }
                    None => {
                        lg.u32(0);
                        lg.i32(-1);
                    }
                }
            }
            Op::NthValues(d, n) => {
                let s = &slot[*d as usize];
                let p = (*n >= 0).then(|| s.1.get(*n as usize)).flatten().map(|e| &e.1);
                lg.u8(matches!(p, Some(DictValue::Bool(true))));
                lg.i32(if let Some(DictValue::Int(i)) = p { *i } else { 0 });
                lg.f32(if let Some(DictValue::Real(f)) = p { *f } else { 0.0 });
                lg.ascii(if let Some(DictValue::AsciiString(a)) = p { a } else { &[] });
                lg.uni(if let Some(DictValue::UnicodeString(u)) = p { u } else { &[] });
            }
        }
    }
    lg.0
}

fn run_cpp(ops: &[Op]) -> Vec<u8> {
    let script = encode(ops);
    let mut log = vec![0u8; 8 * 1024 * 1024];
    let n = unsafe { cpp::ref_dict_run(script.as_ptr(), script.len() as i32, log.as_mut_ptr(), log.len() as i32) };
    assert!(n >= 0, "C++ log overflow");
    log.truncate(n as usize);
    log
}

fn key(g: &mut Gen) -> u32 {
    match g.below(20) {
        0 => (1 << 24) + g.below(8),
        1 => 0xff_ffff,
        2 => 0,
        3 => g.u32() & 0x7fff_ffff,
        4 => (2 << 24) + g.below(4),
        _ => 1 + g.below(30),
    }
}

fn value(g: &mut Gen) -> DictValue {
    match g.below(5) {
        0 => DictValue::Bool(g.below(2) == 0),
        1 => DictValue::Int(g.i32()),
        2 => DictValue::Real(f32::from_bits(g.u32())),
        3 => DictValue::AsciiString((0..g.below(12)).map(|_| g.below(256) as u8).collect()),
        _ => DictValue::UnicodeString((0..g.below(8)).map(|_| g.u32() as u16).collect()),
    }
}

#[test]
fn dict_matches_cpp() {
    let mut g = Gen::new(401);
    let mut ok = Check::new("Dict script logs");
    let mut planted = [
        (Bug::StableSort, Check::new("Dict (stable sort)")),
        (Bug::UntruncatedKeys, Check::new("Dict (untruncated keys)")),
        (Bug::RemoveInPlace, Check::new("Dict (remove in place)")),
    ];
    let mut undefined = 0u32;
    let mut ops_total = 0usize;
    for script in 0..4000 {
        let mut ops = Vec::new();
        for _ in 0..g.below(120) {
            let d = g.below(4) as u8;
            let t = g.below(4) as u8;
            let op = match g.below(100) {
                0..=2 => Op::New(d, [0, 1, 5, -3][g.below(4) as usize]),
                3..=4 => Op::Clear(d),
                5..=49 => Op::Set(d, key(&mut g), value(&mut g)),
                50..=61 => Op::Remove(d, key(&mut g)),
                62..=67 => Op::CopyPair(d, t, key(&mut g)),
                68..=69 => Op::Assign(d, t),
                70 => Op::CopyConstruct(d, t),
                71..=80 => Op::Dump(d),
                81..=88 => Op::Get(d, key(&mut g)),
                89..=93 => Op::NthKeyType(d, g.below(40) as i32 - 3),
                _ => Op::NthValues(d, g.below(12) as i32 - 1),
            };
            ops.push(op);
        }
        ops_total += ops.len();
        // Keep only the defined getNth* reads for the C++ (the Rust is
        // checked to error on the others).
        let mut defined = Vec::new();
        for op in ops {
            let mut probe = defined.clone();
            probe.push(op.clone());
            if run_rust(&probe).is_none() {
                undefined += 1;
                continue;
            }
            defined.push(op);
        }
        let rust = run_rust(&defined).expect("defined script");
        let c = run_cpp(&defined);
        ok.eq(&rust, &c, || format!("script {script} ({} ops)", defined.len()));
        for (bug, check) in planted.iter_mut() {
            check.eq(run_planted(&defined, *bug), c.clone(), || format!("script {script}"));
        }
    }
    println!("     {ops_total} operations; {undefined} getNth* reads left out (out of range with storage, undefined upstream; Rust returned Err)");
    ok.finish();
    for (_, check) in planted {
        check.finish_planted();
    }
}
