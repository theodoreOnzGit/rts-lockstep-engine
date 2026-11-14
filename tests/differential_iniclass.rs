// SPDX-License-Identifier: GPL-3.0-only
//! GitHub #181 item 3 (INIClass): WWLib's INI database (`ww::ini`) and `AnimatedSoundMgrClass::Initialize`'s table
//! read through it, against upstream's text compiled (`differential/wrap_w3d_eval.cpp`: ini.cpp, INI.H, inisup.h,
//! readline.cpp, crc.cpp, CRC.H, nstrdup.cpp cut verbatim by build.rs, each anchor asserted once; the real listnode.h,
//! index.h and bsearch.h; trim.cpp's strtrim; animatedsoundmgr.cpp whole).
//!
//! **Methodology.** Random INI texts (`ini_text`): section lines with padding, case variants, duplicates, text after the
//! `]`, `;` inside the brackets, `[` with no `]`; entries with padding around the name and the value, empty names
//! (`=v`), empty values (`k=`), several `=`, comments anywhere, comment-only and blank lines, lines with no `=`,
//! names whose CRC-32 collide (`WlZMkQ`/`ylvDlZ`, `3d3Fp1`/`KoElDA`), duplicate names, lines of 4,090 to 4,100 bytes
//! with `\r` inside, NUL, control and high bytes, `\n`, `\r\n` and lone `\r`, white space as long as the rest of the
//! line (strtrim's `*source` quirk), text before the first section, and a last line with or without a newline;
//! `Keep_Blank_Entries(true)` in 1 of 5. Each text is loaded by `INIClass(FileClass &)` on both sides, and every getter
//! the manager uses is logged and compared byte for byte (`probe_log`): `Section_Count`, `Is_Loaded`, the section list
//! in order, and for each listed section by its name `Entry_Count`, `Get_Entry(-1 ..= count)`, `Get_String` of each
//! entry and of 12 probe entries (default `"DEF"`); for 10 probe sections `Section_Present`, `Entry_Count`,
//! `Get_Entry(0)` and the probe entries' `Get_String`; and the `DuplicateCRCError` messages. Then `sound_text` (the
//! same line rules over sound-table sections) is read by the whole `Initialize()` on both sides (the file available in
//! 15 of 16) and the table compared (`table_log`): every list in `AnimSoundLists` order (bone, each sound's frame, name,
//! `2D`, `STOP`) and the list `AnimationNameHash.Get` answers for each section's upper-case name and some others. A
//! text on which the Rust says upstream is undefined (a value of one parameter, `atoi` outside `int`) is not given to
//! the C++ (counted). Pass: 0 mismatching cases. Cases: `INICLASS_CASES` (default 20,000).
//!
//! **Results:** see the test's doc comment.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::w3d::animatedsoundmgr as asm;
use rts_lockstep_engine::formats::w3d::W3dError;
use rts_lockstep_engine::ww::ini::IniClass;
use support::{Check, Gen};

const SECTIONS: [&[u8]; 14] =
    [b"A", b"a", b"Alpha", b"alpha", b"SND.Walk", b"snd.walk", b"x;y", b"", b"B C", b"WlZMkQ", b"ylvDlZ", b"Z]z", b"long_section_name_01", b"\xe9t\xe9"];
const KEYS: [&[u8]; 14] =
    [b"BoneName", b"bonename", b"S1", b"S2", b"k", b"K", b"WlZMkQ", b"ylvDlZ", b"3d3Fp1", b"KoElDA", b"a b", b"x=y", b"\xffkey", b"S1 "];
const VALUES: [&[u8]; 12] = [b"1", b"v", b"10,step", b" 3 , boom ,2D", b"x=y", b"", b"a ; b", b"  q  ", b"5,s,STOP", b"\t7,t\t", b"=", b"\x80\x81"];

fn pick<'a>(g: &mut Gen, v: &[&'a [u8]]) -> &'a [u8] {
    v[g.below(v.len() as u32) as usize]
}

fn pad(g: &mut Gen) -> Vec<u8> {
    match g.below(8) {
        0 => b" ".to_vec(),
        1 => b"\t".to_vec(),
        2 => b"   ".to_vec(),
        3 => vec![b' '; g.below(6) as usize],
        4 => vec![0x01, b' '],
        _ => Vec::new(),
    }
}

fn eol(g: &mut Gen) -> &'static [u8] {
    match g.below(10) {
        0..=4 => b"\n",
        5..=7 => b"\r\n",
        8 => b"\r\r\n",
        _ => b"\n\n",
    }
}

/// A random INI text (module docs).
fn ini_text(g: &mut Gen) -> Vec<u8> {
    let mut t = Vec::new();
    if g.below(3) == 0 {
        t.extend_from_slice(b"before=1\n; header\n");
    }
    for _ in 0..g.below(40) {
        let mut line = Vec::new();
        match g.below(20) {
            0..=3 => {
                line.extend(pad(g));
                line.push(b'[');
                line.extend(pad(g));
                line.extend_from_slice(pick(g, &SECTIONS));
                line.extend(pad(g));
                line.push(b']');
                match g.below(6) {
                    0 => line.extend_from_slice(b" tail"),
                    1 => line.extend_from_slice(b" ; c"),
                    2 => line.push(b']'),
                    _ => {}
                }
            }
            4..=11 => {
                line.extend(pad(g));
                line.extend_from_slice(pick(g, &KEYS));
                line.extend(pad(g));
                line.push(b'=');
                line.extend(pad(g));
                line.extend_from_slice(pick(g, &VALUES));
                line.extend(pad(g));
                if g.below(5) == 0 {
                    line.extend_from_slice(b";comment=x");
                }
            }
            12 => line.extend_from_slice(b"; just a comment"),
            13 => line.extend(pad(g)),
            14 => {
                line.extend_from_slice(b"=");
                line.extend_from_slice(pick(g, &VALUES));
            }
            15 => {
                line.extend_from_slice(pick(g, &KEYS));
                line.push(b'=');
                line.extend(pad(g));
            }
            16 => line.extend_from_slice(pick(g, &[b"no equals", b"[no close", b"]", b"[", b"a;=b"])),
            17 => {
                // a line about MAX_LINE_LENGTH long, with '\r' inside now and then
                line.extend_from_slice(pick(g, &KEYS));
                line.push(b'=');
                let n = 4085 + g.below(16) as usize;
                for i in 0..n {
                    line.push(if i % 997 == 3 && g.below(2) == 0 { b'\r' } else { b'a' + (i % 26) as u8 });
                }
            }
            18 => {
                // odd bytes: NUL, control, high
                line.extend_from_slice(pick(g, &KEYS));
                line.push(b'=');
                for _ in 0..1 + g.below(6) {
                    line.push(pick(g, &[b"\x00", b"\x1f", b"\x7f", b"\xa0", b"z", b" "])[0]);
                }
                if g.below(3) == 0 {
                    // a NUL, then what would be an entry
                    line.extend_from_slice(b"\x00k=after");
                }
            }
            _ => {
                // white space as long as the text after it (strtrim's `*source` quirk), on either kind of line
                let body: Vec<u8> = if g.below(2) == 0 { [b"k=", pick(g, &VALUES)].concat() } else { [b"[", pick(g, &SECTIONS), b"]"].concat() };
                let tail = vec![b' '; g.below(3) as usize];
                let lead = body.len() + tail.len();
                line.extend(vec![b' '; lead]);
                line.extend(body);
                line.extend(tail);
            }
        }
        t.extend(line);
        t.extend_from_slice(eol(g));
    }
    if g.below(4) == 0 {
        // a last line with no newline
        t.extend_from_slice(if g.below(2) == 0 { &b"[Last]"[..] } else { b"last=1" });
    }
    t
}

/// A sound table as an INI text: sections named like animations (case varied, sometimes repeated), with `BoneName`
/// and sound entries, under the same line rules.
fn sound_text(g: &mut Gen) -> (Vec<u8>, bool) {
    const ANIMS: [&[u8]; 6] = [b"SND.Walk", b"snd.walk", b"SND.Run", b"Other.X", b"A", b"a"];
    const NAMES: [&[u8]; 8] = [b"step", b" boom ", b"clank", b"x2D", b"STOPPER", b"  ab  ", b"LongerName_01", b"q"];
    const MORE: [&[u8]; 8] = [b"2D", b"STOP", b"x2Dy", b"stop", b"3", b"2d", b" STOP ", b""];
    let mut t = Vec::new();
    // whether a sound entry has no name parameter, or `bonename` makes one of a bone value: upstream reads past
    // the parameter list (the guard for planted runs, whose Rust may not see it)
    let mut risky = false;
    for _ in 0..1 + g.below(6) {
        t.extend(pad(g));
        t.push(b'[');
        t.extend_from_slice(pick(g, &ANIMS));
        t.push(b']');
        t.extend_from_slice(eol(g));
        for k in 0..g.below(6) {
            t.extend(pad(g));
            match g.below(8) {
                0 => {
                    let key = pick(g, &[b"BoneName", b"bonename", b"BoneName "]);
                    risky |= key == b"bonename";
                    t.extend_from_slice(key);
                    t.push(b'=');
                    t.extend_from_slice(pick(g, &[b"root", b"B1", b"foot ", b" x"]));
                }
                _ => {
                    t.extend_from_slice(format!("S{}", if g.below(5) == 0 { 0 } else { k }).as_bytes());
                    t.extend(pad(g));
                    t.push(b'=');
                    let frame = g.below(40) as i32 - 3;
                    t.extend_from_slice(match g.below(8) {
                        0 => format!(" {frame}"),
                        1 => format!("+{frame}"),
                        2 => format!("{frame}x"),
                        _ => format!("{frame}"),
                    }
                    .as_bytes());
                    if g.below(30) != 0 {
                        t.push(b',');
                        t.extend_from_slice(pick(g, &NAMES));
                    } else {
                        risky = true;
                    }
                    for _ in 0..g.below(3) {
                        t.push(b',');
                        t.extend_from_slice(pick(g, &MORE));
                    }
                    if g.below(6) == 0 {
                        t.extend_from_slice(b" ; why");
                    }
                }
            }
            t.extend_from_slice(eol(g));
        }
        if g.below(10) == 0 {
            t.extend_from_slice(b"; comment\n\n");
        }
    }
    (t, risky)
}

fn put32(log: &mut Vec<u8>, v: u32) {
    log.extend_from_slice(&v.to_le_bytes());
}

fn put_str(log: &mut Vec<u8>, v: Option<&[u8]>) {
    match v {
        None => put32(log, u32::MAX),
        Some(v) => {
            let v = &v[..v.iter().position(|&c| c == 0).unwrap_or(v.len())];
            put32(log, v.len() as u32);
            log.extend_from_slice(v);
        }
    }
}

fn strings(v: &[Vec<u8>]) -> Vec<u8> {
    let mut b = Vec::new();
    put32(&mut b, v.len() as u32);
    for s in v {
        put32(&mut b, s.len() as u32);
        b.extend_from_slice(s);
    }
    b
}

/// The Rust's log of `ref_iniclass_probe` (see wrap_w3d_eval.cpp).
fn probe_log(text: &[u8], keep_blank: bool, psec: &[Vec<u8>], pent: &[Vec<u8>]) -> Vec<u8> {
    let mut ini = IniClass::new();
    ini.set_keep_blank_entries(keep_blank);
    ini.load_file(b"probe.ini", Some(text)).expect("one Load");
    let mut log = Vec::new();
    // Load's answer, on a database made empty (INIClass() then Load(file))
    log.push(u8::from(IniClass::new().load_file(b"probe.ini", Some(text)).expect("one Load")));
    put32(&mut log, ini.section_count() as u32);
    log.push(u8::from(ini.is_loaded()));
    let list = ini.get_section_list();
    put32(&mut log, list.len() as u32);
    for section in list {
        let name = &section.section;
        put_str(&mut log, Some(name));
        let n = ini.entry_count(name);
        put32(&mut log, n as u32);
        for i in -1..=n {
            put_str(&mut log, ini.get_entry(name, i));
        }
        for i in 0..n {
            let e = ini.get_entry(name, i).unwrap_or_default().to_vec();
            put_str(&mut log, Some(&ini.get_string(name, &e, b"DEF")));
        }
        for p in pent {
            put_str(&mut log, Some(&ini.get_string(name, p, b"DEF")));
        }
    }
    for sn in psec {
        log.push(u8::from(ini.section_present(sn)));
        put32(&mut log, ini.entry_count(sn) as u32);
        put_str(&mut log, ini.get_entry(sn, 0));
        for p in pent {
            put_str(&mut log, Some(&ini.get_string(sn, p, b"DEF")));
        }
    }
    let dups = ini.duplicate_crc_errors();
    put32(&mut log, dups.len() as u32);
    for (section, entry) in dups {
        // DuplicateCRCError's message, as _snprintf writes it into 512 bytes
        let mut m = b"INIClass::Load - Duplicate Entry \"".to_vec();
        m.extend_from_slice(entry);
        m.extend_from_slice(b"\" in section \"");
        m.extend_from_slice(section);
        m.extend_from_slice(b"\" (");
        m.extend_from_slice(ini.get_filename());
        m.extend_from_slice(b")\n");
        m.truncate(511);
        put_str(&mut log, Some(&m));
    }
    log
}

fn cpp_probe(text: &[u8], keep_blank: bool, probes: &[u8]) -> Vec<u8> {
    let mut log = vec![0u8; 1 << 20];
    let mut n = 0;
    unsafe {
        cpp::ref_iniclass_probe(
            text.as_ptr().cast(),
            text.len() as i32,
            i32::from(keep_blank),
            probes.as_ptr(),
            probes.len() as i32,
            log.as_mut_ptr(),
            log.len() as i32,
            &mut n,
        );
    }
    assert!((n as usize) <= log.len(), "probe log overflow");
    log.truncate(n as usize);
    log
}

/// The Rust's `Initialize()` on `text` and its table (`ref_iniclass_table`'s log); `Err` where the Rust says upstream
/// is undefined.
fn table_log(text: &[u8], available: bool, names: &[Vec<u8>]) -> Result<Vec<u8>, W3dError> {
    // the statics as at the process's start on both sides (the hash's size would otherwise carry the Rust's undefined
    // cases, which the C++ never reads)
    asm::reset_statics();
    let ini = IniClass::from_file(b"w3danimsound.ini", Some(text)).expect("one Load");
    let r = asm::initialize(available.then_some(&ini));
    let lists = asm::anim_sound_lists();
    let found: Vec<Option<usize>> = names.iter().map(|n| asm::sound_list_index(n)).collect();
    asm::shutdown();
    r?;
    let mut log = Vec::new();
    put32(&mut log, lists.len() as u32);
    for l in &lists {
        put_str(&mut log, Some(&l.bone_name));
        put32(&mut log, l.list.len() as u32);
        for i in &l.list {
            put32(&mut log, i.frame as u32);
            put_str(&mut log, Some(&i.sound_name));
            log.push(u8::from(i.is_2d));
            log.push(u8::from(i.is_stop));
        }
    }
    for f in found {
        put32(&mut log, f.map_or(u32::MAX, |p| p as u32));
    }
    Ok(log)
}

fn cpp_table(text: &[u8], available: bool, names: &[u8]) -> Vec<u8> {
    let mut log = vec![0u8; 1 << 20];
    let mut n = 0;
    unsafe {
        cpp::ref_iniclass_table(text.as_ptr().cast(), text.len() as i32, i32::from(available), names.as_ptr(), log.as_mut_ptr(), log.len() as i32, &mut n);
    }
    assert!((n as usize) <= log.len(), "table log overflow");
    log.truncate(n as usize);
    log
}

fn cases() -> usize {
    std::env::var("INICLASS_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000)
}

/// The INI getters and the manager's table against upstream (module docs).
///
/// **Predicted** (iniclass_predictions.md, before any run): 0-2 classes of mismatch at first, most likely duplicate
/// sections (the order `qsort` leaves equal IDs in); the table 0 mismatches.
///
/// **Results (2026-11-14):** the first run (2,000 cases) found two things, neither predicted: 61 getter mismatches from
/// the harness (glibc's `strcpy` over itself inside strtrim; fixed in wrap_w3d_eval.cpp, see there), and table mismatches
/// from a port defect (the name hash was a map that kept the later of two equal names; upstream's `HashTemplateClass`
/// answers the earlier after a `Re_Hash`; translated as `NameHash`: with the old behaviour 1,217 of 14,603 tables
/// mismatch). Final: 20,000 texts, 57,476 sections, 135,474 entries, 23,119 duplicates dropped, **0 getter mismatches**;
/// 14,632 tables compared (5,368 undefined, kept from the C++), 34,883 sound lists, **0 table mismatches**.
///
/// Planted bugs (mismatching cases of 20,000; `L9_PLANT=N`): 10236 7,973; 10237 4,761; 10238 2,193; 10239 8,589; 10240
/// 2,909; 10241 9,927; 10242 20,000; 10243 16,719; 10244 3,105; 10245 13,994; 10246 15,562; 10247 8,111; 10248 6,149;
/// 10249 16,874; 10250 14,533; 10251 13,950; 10252 3,397; 10253 4,003; 10254 12,296; 10255 18,318 (what each plants: the
/// comments at `plant(N)` in `ww::ini` and `animatedsoundmgr`). 20 of 20 caught. With a plant set, a sound table that
/// upstream would read past its parameter list (`risky`) is kept from the C++, since the planted Rust may not see it.
#[test]
fn iniclass_and_the_sound_table_match_upstream() {
    let mut probe = Check::new("INIClass getters (ini.cpp)");
    let mut table = Check::new("AnimatedSoundMgrClass table via INIClass");
    let mut g = Gen::new(0x181_31C1);
    let (mut sections, mut entries, mut dups, mut undefined, mut lists) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let n = cases();
    for case in 0..n {
        let text = ini_text(&mut g);
        let keep_blank = g.below(5) == 0;
        let mut psec: Vec<Vec<u8>> = (0..10).map(|_| pick(&mut g, &SECTIONS).to_vec()).collect();
        psec.push(b"nowhere".to_vec());
        let pent: Vec<Vec<u8>> = (0..12).map(|_| pick(&mut g, &KEYS).to_vec()).collect();
        let mut probes = strings(&psec);
        probes.extend(strings(&pent));
        let rust = probe_log(&text, keep_blank, &psec, &pent);
        let c = cpp_probe(&text, keep_blank, &probes);
        {
            let mut ini = IniClass::new();
            ini.set_keep_blank_entries(keep_blank);
            ini.load(&text).expect("one Load");
            sections += ini.get_section_list().len() as u64;
            entries += ini.get_section_list().iter().map(|s| s.entry_list.len() as u64).sum::<u64>();
            dups += ini.duplicate_crc_errors().len() as u64;
        }
        if rust != c && std::env::var_os("INICLASS_DEBUG").is_some() {
            let at = rust.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(rust.len().min(c.len()));
            let w = |v: &[u8]| String::from_utf8_lossy(&v[at.saturating_sub(40)..(at + 60).min(v.len())]).into_owned();
            eprintln!("case {case}: first difference at {at} of {}/{}\n  rust {:?}\n  c++  {:?}", rust.len(), c.len(), w(&rust), w(&c));
        }
        probe.eq(rust, c, || format!("case {case}: {:?}", String::from_utf8_lossy(&text[..text.len().min(300)])));

        let (stext, risky) = sound_text(&mut g);
        let planted = std::env::var("L9_PLANT").is_ok();
        let available = g.below(16) != 0;
        let names: Vec<Vec<u8>> = {
            let ini = IniClass::from_file(b"w3danimsound.ini", Some(&stext)).expect("one Load");
            let mut v: Vec<Vec<u8>> = ini.get_section_list().iter().map(|s| s.section.to_ascii_uppercase()).collect();
            v.extend([b"SND.WALK".to_vec(), b"snd.walk".to_vec(), b"NONE".to_vec()]);
            v
        };
        match table_log(&stext, available, &names) {
            Err(_) => undefined += 1,
            Ok(_) if planted && risky && available => undefined += 1,
            Ok(rust) => {
                lists += u64::from(u32::from_le_bytes(rust[..4].try_into().unwrap()));
                let c = cpp_table(&stext, available, &strings(&names));
                table.eq(rust, c, || format!("case {case}: {:?}", String::from_utf8_lossy(&stext)));
            }
        }
    }
    println!(
        "INICLASS {n} texts: {sections} sections, {entries} entries, {dups} duplicates dropped; tables: {} compared, {undefined} undefined (not given to the C++), {lists} sound lists",
        table.compared
    );
    if std::env::var("L9_PLANT").is_ok() {
        let caught = probe.mismatches + table.mismatches;
        println!("PLANT caught in {caught} cases (getters {}, table {})", probe.mismatches, table.mismatches);
        assert!(caught > 0, "the planted bug was not caught");
        return;
    }
    probe.finish();
    table.finish();
}
