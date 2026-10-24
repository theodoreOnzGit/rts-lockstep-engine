// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `formats::csf` against upstream
//! `GameText.cpp` (commit 0a05454d), compiled unchanged by g++, the real
//! `GameTextManager` made by `CreateGameTextInterface` (see
//! `differential/wrap_csf.cpp`).
//!
//! No game string files are available, so every CSF is **synthetic**, built
//! byte by byte in the layout `parseCSF` reads: a header (right or wrong
//! identifier, versions below and above 2, label counts equal to, above,
//! below and far below the labels present, zero), labels (empty, with NUL
//! and high bytes, duplicates differing only in case, lengths that disagree
//! with the bytes that follow), strings (`STR `, `STRW`, bad tags, none,
//! several per label, texts with runs of spaces, tabs and newlines, raw 0 and
//! 0xFFFF units, negative lengths, which move the read position backwards),
//! speech names, files cut off anywhere, and random byte mutations.
//!
//! Compared per file, after `init()`: `fetch` (whether the label exists and,
//! when it does, the text unit by unit; when loading failed, the `m_failed`
//! text) for every label of the file, case-changed and cut-down versions,
//! unknown labels and `""`; and `getStringsWithLabelPrefix` for prefixes of
//! the labels and `""`.
//!
//! Files for which the Rust reports [`CsfError::Undefined`] are **not** given
//! to the C++ (counted per kind), nor are files declaring more than 10^5
//! labels (from byte mutations of the header): upstream allocates every
//! declared entry, and `new StringInfo[n]` fails for counts near 2^31 (the
//! Rust does not store the empty entries; counted separately), nor files
//! that end inside a label whose string count would make upstream spin more
//! than 10^5 times reading nothing (`GameText::eof_spin`; counted). A fetch whose result upstream leaves
//! unspecified (several labels equal under `stricmp` with different texts:
//! `qsort` and `bsearch` choose) is not compared (counted); a prefix list
//! whose order upstream leaves unspecified (labels equal under `stricmp` but
//! different bytes) is compared as a sorted list (counted).
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::csf::{
    CsfError, GameText, CSF_ID, CSF_LABEL, CSF_STRING, CSF_STRINGWITHWAVE, FAILED_TEXT,
};
use std::collections::BTreeMap;
use std::ffi::CString;
use support::{Check, Gen};

thread_local! {
    /// Whether the generator makes a well-formed file.
    static CLEAN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn clean() -> bool {
    CLEAN.with(|c| c.get())
}

fn w(v: &mut Vec<u8>, x: i32) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn pick<'a, T: ?Sized>(g: &mut Gen, xs: &[&'a T]) -> &'a T {
    xs[g.below(xs.len() as u32) as usize]
}

fn gen_label(g: &mut Gen) -> Vec<u8> {
    match g.below(12) {
        0 => Vec::new(),
        1 => (0..1 + g.below(12)).map(|_| g.u32() as u8).collect(),
        2 => {
            let mut l = pick(g, &[&b"GUI:Ok"[..], b"OBJECT:Tank"]).to_vec();
            l.insert(g.below(l.len() as u32) as usize, 0);
            l
        }
        _ => {
            let base = pick(
                g,
                &[
                    &b"GUI:Ok"[..],
                    b"GUI:Cancel",
                    b"OBJECT:Tank",
                    b"OBJECT:Ranger",
                    b"MAP:Desc",
                    b"gui:ok",
                    b"GUI:OK",
                    b"Gui:Ok2",
                    b"CONTROLBAR:Build",
                    b"a",
                    b"_x",
                ],
            );
            let mut l = base.to_vec();
            if g.below(4) == 0 {
                l.extend_from_slice(format!("{}", g.below(20)).as_bytes());
            }
            l
        }
    }
}

fn gen_text(g: &mut Gen) -> Vec<u16> {
    let n = g.below(30);
    (0..n)
        .map(|_| match g.below(20) {
            0..=3 => u16::from(b' '),
            4 => u16::from(b'\n'),
            5 => u16::from(b'\t'),
            6 => 0xFFFF, // decodes to 0: the text ends here
            7 => 0x00e9,
            8 => g.u32() as u16,
            _ => u16::from(b"abcdefXYZ0123:-"[g.below(15) as usize]),
        })
        .collect()
}

/// A length field: mostly the true one, sometimes off, negative or huge.
fn gen_len(g: &mut Gen, actual: usize) -> i32 {
    if clean() {
        return actual as i32;
    }
    match g.below(40) {
        0 => actual as i32 + 1 + g.below(4) as i32,
        1 => (actual as i32 - 1 - g.below(4) as i32).max(0),
        2 => -(1 + g.below(40) as i32),
        3 => pick(
            g,
            &[
                &10239,
                &10240,
                &20479,
                &20480,
                &0x4000_0000,
                &i32::MAX,
                &i32::MIN,
            ],
        )
        .to_owned(),
        _ => actual as i32,
    }
}

/// A synthetic CSF file and its labels.
fn gen_csf(g: &mut Gen) -> (Vec<u8>, Vec<Vec<u8>>) {
    let n_labels = g.below(16) as usize;
    let mut labels = Vec::new();
    let mut body = Vec::new();
    for _ in 0..n_labels {
        let label = gen_label(g);
        w(
            &mut body,
            if g.below(80) == 0 && !clean() {
                CSF_STRING
            } else {
                CSF_LABEL
            },
        );
        let n_strings = match g.below(30) {
            0 | 1 if clean() => 1,
            0 => -1,
            1 => 5 + g.below(3) as i32,
            2 => 0,
            _ => 1 + (g.below(4) == 0) as i32,
        };
        w(&mut body, n_strings);
        w(&mut body, gen_len(g, label.len()));
        body.extend_from_slice(&label);
        labels.push(label);
        for _ in 0..n_strings.clamp(0, 3) {
            let wave = g.below(3) == 0;
            let tag = match g.below(80) {
                0 if !clean() => CSF_LABEL,
                _ if wave => CSF_STRINGWITHWAVE,
                _ => CSF_STRING,
            };
            w(&mut body, tag);
            let text = gen_text(g);
            w(&mut body, gen_len(g, text.len()));
            let raw0 = g.below(15) == 0;
            for (k, u) in text.iter().enumerate() {
                // raw 0 (before inversion) ends the inversion loop early
                let raw = if raw0 && k == text.len() / 2 { 0 } else { !u };
                body.extend_from_slice(&raw.to_le_bytes());
            }
            if wave {
                let speech = pick(g, &[&b"speech.wav"[..], b"", b"vo\x00x", b"A"]).to_vec();
                w(&mut body, gen_len(g, speech.len()));
                body.extend_from_slice(&speech);
            }
        }
    }
    let declared = match g.below(12) {
        0..=3 if clean() => n_labels as i32 + g.below(2) as i32,
        0 => 0,
        1 => n_labels as i32 + 1 + g.below(5) as i32,
        2 => (n_labels as i32 - 1).max(0),
        3 => -1,
        4 => 50,
        _ => n_labels as i32,
    };
    let mut data = Vec::new();
    w(
        &mut data,
        if g.below(30) == 0 && !clean() {
            CSF_LABEL
        } else {
            CSF_ID
        },
    );
    w(
        &mut data,
        pick(g, &[&3, &3, &2, &1, &0, &4, &-1]).to_owned(),
    );
    w(&mut data, declared);
    w(&mut data, g.below(100) as i32);
    w(&mut data, g.i32());
    w(&mut data, g.below(10) as i32);
    data.extend_from_slice(&body);
    match if clean() { 9 } else { g.below(10) } {
        0 => data.truncate(g.below(data.len() as u32 + 1) as usize),
        1 if !data.is_empty() => {
            for _ in 0..1 + g.below(3) {
                let p = g.below(data.len() as u32) as usize;
                data[p] = g.u32() as u8;
            }
        }
        _ => {}
    }
    (data, labels)
}

fn cpp_fetch(label: &[u8]) -> (bool, Vec<u16>) {
    let c = CString::new(label.to_vec()).unwrap();
    let mut out = vec![0u16; 1 << 16];
    let mut exists = 0;
    let n =
        unsafe { cpp::ref_csf_fetch(c.as_ptr(), out.as_mut_ptr(), out.len() as i32, &mut exists) };
    out.truncate(n as usize);
    (exists == 1, out)
}

fn cpp_prefix(prefix: &[u8]) -> Vec<Vec<u8>> {
    let c = CString::new(prefix.to_vec()).unwrap();
    let mut out = vec![0u8; 1 << 20];
    let n = unsafe { cpp::ref_csf_prefix(c.as_ptr(), out.as_mut_ptr() as *mut _, out.len() as i32) }
        as usize;
    assert!(n <= out.len());
    let mut v = Vec::new();
    let mut cur = Vec::new();
    for &b in &out[..n] {
        if b == 0 {
            v.push(std::mem::take(&mut cur));
        } else {
            cur.push(b);
        }
    }
    v
}

fn show(b: &[u8]) -> String {
    format!("{:?}", String::from_utf8_lossy(b))
}

fn queries(g: &mut Gen, labels: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let mut q: Vec<Vec<u8>> = vec![
        Vec::new(),
        b"GUI:Command&ConquerGenerals".to_vec(),
        b"nope".to_vec(),
    ];
    for l in labels {
        let l: Vec<u8> = l.iter().take_while(|&&b| b != 0).copied().collect();
        q.push(l.clone());
        q.push(l.to_ascii_uppercase());
        q.push(l.to_ascii_lowercase());
        if !l.is_empty() {
            q.push(l[..g.below(l.len() as u32) as usize].to_vec());
        }
    }
    q
}

#[test]
fn csf_matches_upstream() {
    let mut g = Gen::new(0xC5F);
    let mut fetches = Check::new("GameText fetch (exists, text)");
    let mut prefixes = Check::new("GameText getStringsWithLabelPrefix");
    let mut failed_state = Check::new("GameText failed load (m_failed)");
    let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
    let (
        mut loaded,
        mut not_loaded,
        mut ambiguous,
        mut unordered,
        mut found,
        mut too_large,
        mut spin,
    ) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    const CASES: usize = 20_000;
    let failed: Vec<u16> = FAILED_TEXT.encode_utf16().collect();
    for case in 0..CASES {
        CLEAN.with(|c| c.set(case % 2 == 0));
        let (data, labels) = gen_csf(&mut g);
        let rust = GameText::load_csf(&data);
        if let Err(CsfError::Undefined(u)) = rust {
            *excluded.entry(format!("{u:?}")).or_default() += 1;
            continue;
        }
        let declared =
            rts_lockstep_engine::formats::csf::CsfHeader::read(&data).map_or(0, |h| h.num_labels);
        if let Ok(t) = &rust {
            if t.eof_spin() > 100_000 {
                spin += 1;
                continue;
            }
        }
        if declared > 100_000 {
            too_large += 1;
            continue;
        }
        let threw = unsafe { cpp::ref_csf_load(data.as_ptr() as *const _, data.len() as i32) };
        assert_eq!(threw, 0, "case {case}: GameTextManager::init threw");
        let d = || format!("case {case}: {:02x?}", data);
        match &rust {
            Err(_) => {
                not_loaded += 1;
                for q in queries(&mut g, &labels) {
                    failed_state.eq((false, failed.clone()), cpp_fetch(&q), || {
                        format!("{}, fetch {}", d(), show(&q))
                    });
                }
            }
            Ok(t) => {
                loaded += 1;
                for q in queries(&mut g, &labels) {
                    if t.is_ambiguous(&q) {
                        ambiguous += 1;
                        continue;
                    }
                    let (ce, ct) = cpp_fetch(&q);
                    match t.fetch(&q) {
                        Some(text) => {
                            found += 1;
                            fetches.eq((true, text.to_vec()), (ce, ct), || {
                                format!("{}, fetch {}", d(), show(&q))
                            });
                        }
                        None => fetches.eq(false, ce, || format!("{}, fetch {}", d(), show(&q))),
                    }
                }
                let mut ps: Vec<Vec<u8>> =
                    vec![Vec::new(), b"G".to_vec(), b"GUI:".to_vec(), b"x".to_vec()];
                for l in labels.iter().take(3) {
                    let l: Vec<u8> = l.iter().take_while(|&&b| b != 0).copied().collect();
                    ps.push(l[..g.below(l.len() as u32 + 1) as usize].to_vec());
                }
                for p in ps {
                    let mut r = t.get_strings_with_label_prefix(&p);
                    let mut c = cpp_prefix(&p);
                    // labels equal under stricmp but different bytes: their order is qsort's
                    let mut by_key: BTreeMap<Vec<u8>, &Vec<u8>> = BTreeMap::new();
                    let order_unspecified = r
                        .iter()
                        .any(|l| *by_key.entry(l.to_ascii_lowercase()).or_insert(l) != l);
                    if order_unspecified {
                        unordered += 1;
                        r.sort();
                        c.sort();
                    }
                    prefixes.eq(&r, &c, || format!("{}, prefix {}", d(), show(&p)));
                }
            }
        }
    }
    println!(
        "CSF: {CASES} files, {loaded} loaded, {not_loaded} rejected; {found} fetches found; \
         excluded files (undefined upstream): {} {excluded:?}; excluded files declaring > 10^5 labels: {too_large}; \
         excluded files where upstream spins > 10^5 empty string reads at the end: {spin}; fetches not compared (unspecified upstream): {ambiguous}; \
         prefix lists compared unordered (order unspecified upstream): {unordered}",
        excluded.values().sum::<u64>()
    );
    fetches.finish();
    prefixes.finish();
    failed_state.finish();
}
