#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the drafts src/drafts/wwlib/chunk_save.rs, twiddler.rs (Save_Variables, Load_Variables),
// saveload_ids.rs and saveloadstatus.rs against (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/chunkio.h, chunkio.cpp, iostruct.h
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/twiddler.cpp, saveloadids.h, saveloadstatus.h, saveloadstatus.cpp
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/wwmathids.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with the additional terms in NOTICE.md.

//! `ChunkSaveClass`, the twiddler's variable chunks, the save/load chunk ids and `SaveLoadStatus` against the C++ (#215, group wwlib2).
//!
//! # Method
//!
//! * `ChunkSaveClass` (differential/wrap_draft_chunksave.cpp): upstream's chunkio.cpp (compiled unchanged into upstream_w3d) writes
//!   into an in-memory `FileClass` stand-in. A case is a PROGRAM of 1 to 60 operations from a fixed seed, run on both sides:
//!   `Begin_Chunk`, `End_Chunk`, `Begin_Micro_Chunk`, `End_Micro_Chunk`, `Write(buf, n)` (0 to 300 bytes: a micro chunk that outgrows
//!   255 bytes wraps its `uint8` size, as upstream), the four vector writes (`IOVector2/3/4Struct`, `IOQuaternionStruct`) and
//!   `Cur_Chunk_Depth`. Every return value and the final file image are compared word for word. Programs mix data and chunks in one
//!   chunk, nest micro chunks and end a chunk inside a micro chunk, which upstream only `assert`s against (compiled out), and one
//!   case in 200 nests 256 chunks (the full stack).
//! * `TwiddlerClass::Save_Variables` / `Load_Variables`: upstream's two functions and enums are cut VERBATIM from twiddler.cpp
//!   (build.rs `drafts_wwlib2`) onto a stand-in class holding the two members they use. Save: 0 to 40 definition ids and a random
//!   indirect class id, written inside a `CHUNKID_VARIABLES` chunk, bytes compared. Load: well-formed streams (micro chunks with
//!   ids 1 and 2 of 4 bytes, unknown ids 3 to 9 with 0 to 8 bytes, repeated ids; a second top-level chunk of another id) are loaded
//!   by both over a list pre-filled with 777 and an indirect id of 0xA5A5A5A5 (to show "start fresh" and an unset variable); the
//!   resulting id and list are compared.
//! * `saveload_ids.rs`: the 23 constants against the enums of saveloadids.h and wwmathids.h. `saveloadstatus.rs`: programs of
//!   `Set_Status_Text` (ids 0 and 1: setting id 0 clears id 1), `Get_Status_Text`, `Reset_Status_Count`, `Inc_Status_Count`,
//!   `Get_Status_Count`, the C++ with stand-in `StringClass` and mutex headers (see differential/wrap_draft_slmisc.cpp).
//!
//! # Excluded inputs
//!
//! `Write`, `Begin_Micro_Chunk` and `Cur_Chunk_Depth`-dependent state with no chunk open (upstream reads `HeaderStack[-1]`);
//! `End_Chunk` with none open (`StackIndex` becomes -1); a 257th nested chunk (writes past the 256-entry stacks); chunk sizes
//! of 2 GB (the draft's `add_size` was corrected by reading, see chunk_save.rs; not reachable here); status ids other than 0 and 1.
//! In the load streams: micro chunks whose size is not what the variable needs (the reader would run into the next header).
//!
//! # Predictions (written before the first run, 2026-11-19)
//!
//! 0 mismatches for the writer; I expect the chunk-size accumulation (`Get_Size() + sizeof(header)`) and the micro-chunk header
//! tracking (the header goes through `Write`, so it counts in the wrapping chunk) to be right because the draft's comments say so.
//!
//! # Results and planted bugs
//!
//! See the `DIFF` lines. `L9_PLANT=<id>`: 14894 `Begin_Chunk` does not set the parent's sub-chunk flag; 14895 `End_Chunk` adds the
//! child's size without its 8-byte header; 14896 `End_Chunk` does not seek back to the end; 14897 the micro header written past
//! `Write` (not counted in the chunk); 14898 micro-chunk size not accumulated; 14899 micro header written (size, type); 14900
//! the chunk's recorded start position is after its header; 14901 the 4-float vector writes 12 bytes; 14902 twiddler Save writes the indirect id last; 14903
//! Load does not clear the list; 14904 Load stops at an unknown micro chunk; 14905 Save writes the ids as 2 bytes; 14907 the
//! status text of id 1 is not cleared by setting id 0; 14908 `Reset_Status_Count` does nothing. (The id constants are `const`s: a
//! wrong value does not compile into a runtime plant; the test compares all 23.)

mod support;

use rts_lockstep_engine::drafts::wwlib::chunk_save::ChunkSaveClass;
use rts_lockstep_engine::drafts::wwlib::saveload_ids as ids;
use rts_lockstep_engine::drafts::wwlib::saveloadstatus as status;
use rts_lockstep_engine::drafts::wwlib::twiddler::TwiddlerClass;
use rts_lockstep_engine::ww::chunkio::{ChunkLoad, GameFile};
use support::{Check, Gen};

extern "C" {
    fn ref_cs_run(prog: *const u32, n: i32, bytes: *mut u8, cap: i32, res: *mut u32) -> i32;
    fn ref_tw_save(indirect: u32, list: *const i32, n: i32, bytes: *mut u8, cap: i32) -> i32;
    fn ref_tw_load(bytes: *const u8, n: i32, out: *mut u32) -> i32;
    fn ref_sl_ids(out: *mut u32);
    fn ref_sl_status(prog: *const u32, n: i32, out: *mut u32) -> i32;
}

const CASES: usize = 40_000;

fn cases(default: usize) -> usize {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(default)
}

fn plant_id() -> u32 {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0)
}

/// Prints the DIFF line (or, in a planted run, the PLANTED line) and asserts accordingly; `mine` are the plants that this check
/// is meant to catch.
fn conclude(c: Check, bad_programs: u64, mine: &[u32]) {
    let id = plant_id();
    if mine.contains(&id) {
        let bad = c.report();
        println!("PLANTED {id} : {bad} mismatching words in {bad_programs} programs");
        assert!(bad > 0, "plant {id} not caught");
    } else if id != 0 {
        let bad = c.report();
        println!("(plant {id} is not for this check: {bad} mismatching words)");
    } else {
        c.finish();
    }
}

fn gen_program(g: &mut Gen) -> Vec<u32> {
    let mut p: Vec<u32> = Vec::new();
    if g.below(200) == 0 {
        // the full stack: 256 nested chunks
        for i in 0..256 {
            p.extend_from_slice(&[0, 0x1000 + i]);
        }
        p.extend_from_slice(&[4, 3, 1, 2, 3, 6]);
        for _ in 0..256 {
            p.push(1);
        }
        return p;
    }
    let mut depth = 0u32;
    let nops = 1 + g.below(60);
    for _ in 0..nops {
        match g.below(14) {
            0..=2 => {
                if depth < 255 {
                    p.extend_from_slice(&[0, g.u32() & if g.below(2) == 0 { 0xFF } else { u32::MAX }]);
                    depth += 1;
                }
            }
            3 | 4 => {
                if depth > 0 {
                    p.push(1);
                    depth -= 1;
                }
            }
            5 | 6 => {
                if depth > 0 {
                    p.extend_from_slice(&[2, g.below(256)]);
                }
            }
            7 => p.push(3),
            8..=10 => {
                if depth > 0 {
                    let len = match g.below(6) {
                        0 => 0,
                        1 => 200 + g.below(101),
                        _ => g.below(12),
                    };
                    p.extend_from_slice(&[4, len]);
                    for _ in 0..len {
                        p.push(g.below(256));
                    }
                }
            }
            11 | 12 => {
                if depth > 0 {
                    let k = 2 + g.below(4);
                    p.extend_from_slice(&[5, k]);
                    for _ in 0..(if k == 5 { 4 } else { k }) {
                        p.push(g.mixed().to_bits());
                    }
                }
            }
            _ => p.push(6),
        }
    }
    p
}

fn run_rust(prog: &[u32]) -> (Vec<u32>, Vec<u8>) {
    let mut cs = ChunkSaveClass::new();
    let mut res: Vec<u32> = Vec::new();
    let mut i = 0;
    while i < prog.len() {
        match prog[i] {
            0 => {
                res.push(cs.begin_chunk(prog[i + 1]) as u32);
                i += 2;
            }
            1 => {
                res.push(cs.end_chunk() as u32);
                i += 1;
            }
            2 => {
                res.push(cs.begin_micro_chunk(prog[i + 1]) as u32);
                i += 2;
            }
            3 => {
                res.push(cs.end_micro_chunk() as u32);
                i += 1;
            }
            4 => {
                let len = prog[i + 1] as usize;
                let b: Vec<u8> = prog[i + 2..i + 2 + len].iter().map(|&w| w as u8).collect();
                res.push(cs.write(&b));
                i += 2 + len;
            }
            5 => {
                let k = prog[i + 1] as usize;
                let n = if k == 5 { 4 } else { k };
                let f: Vec<f32> = prog[i + 2..i + 2 + n].iter().map(|&w| f32::from_bits(w)).collect();
                res.push(cs.write_floats(&f));
                i += 2 + n;
            }
            _ => {
                res.push(cs.cur_chunk_depth() as u32);
                i += 1;
            }
        }
    }
    (res, cs.file.clone())
}

#[test]
fn chunk_save_matches_the_cpp() {
    let mut g = Gen::new(0x215_C5A7);
    let mut c = Check::new("ChunkSaveClass programs (results and file image)");
    let mut bad_programs = 0u64;
    let mut bytes_total = 0u64;
    for _ in 0..cases(CASES) {
        let prog = gen_program(&mut g);
        let (rres, rbytes) = run_rust(&prog);
        let mut cres = vec![0u32; prog.len() + 4];
        let mut cbytes = vec![0u8; 1 << 20];
        let clen = unsafe { ref_cs_run(prog.as_ptr(), prog.len() as i32, cbytes.as_mut_ptr(), cbytes.len() as i32, cres.as_mut_ptr()) } as usize;
        bytes_total += clen as u64;
        let m0 = c.mismatches;
        c.eq(rbytes.len(), clen, || format!("file length, program {prog:?}"));
        if rbytes.len() == clen {
            c.eq(rbytes.clone(), cbytes[..clen].to_vec(), || format!("file image, program {prog:?}"));
        }
        c.eq(rres.clone(), cres[..rres.len()].to_vec(), || format!("results, program {prog:?}"));
        if c.mismatches > m0 {
            bad_programs += 1;
        }
    }
    println!("bytes written by the C++: {bytes_total}");
    conclude(c, bad_programs, &[14894, 14895, 14896, 14897, 14898, 14899, 14900, 14901]);
}

fn gen_list(g: &mut Gen) -> Vec<i32> {
    let n = match g.below(6) {
        0 => 0,
        1 => 40,
        _ => g.below(8) as usize,
    };
    (0..n).map(|_| if g.below(4) == 0 { g.i32() } else { g.below(500) as i32 - 100 }).collect()
}

#[test]
fn twiddler_variables_match_the_cpp() {
    let mut g = Gen::new(0x215_7D1E);
    let mut save = Check::new("TwiddlerClass::Save_Variables (chunk bytes)");
    let mut load = Check::new("TwiddlerClass::Load_Variables (indirect id, list)");
    let (mut save_bad, mut load_bad) = (0u64, 0u64);
    for _ in 0..cases(CASES) {
        // Save
        let list = gen_list(&mut g);
        let indirect = if g.below(3) == 0 { 0 } else { g.u32() };
        let mut cs = ChunkSaveClass::new();
        let mut t = TwiddlerClass::new();
        t.set_indirect_class_id(indirect);
        t.definition_list = list.clone();
        cs.begin_chunk(0x100);
        t.save_variables(&mut cs);
        cs.end_chunk();
        let mut cb = vec![0u8; 1 << 16];
        let clen = unsafe { ref_tw_save(indirect, list.as_ptr(), list.len() as i32, cb.as_mut_ptr(), cb.len() as i32) } as usize;
        let m0 = save.mismatches;
        save.eq(cs.file.clone(), cb[..clen].to_vec(), || format!("indirect {indirect:#x} list {list:?}"));
        if save.mismatches > m0 {
            save_bad += 1;
        }

        // Load: a stream built here, or the C++'s own saved image
        let stream: Vec<u8> = if g.below(4) == 0 {
            cb[..clen].to_vec()
        } else {
            let mut body: Vec<u8> = Vec::new();
            for _ in 0..g.below(12) {
                match g.below(5) {
                    0 | 1 => {
                        body.extend_from_slice(&[1, 4]);
                        body.extend_from_slice(&g.i32().to_le_bytes());
                    }
                    2 => {
                        body.extend_from_slice(&[2, 4]);
                        body.extend_from_slice(&g.u32().to_le_bytes());
                    }
                    _ => {
                        let id = 3 + g.below(7) as u8;
                        let len = g.below(9) as u8;
                        body.extend_from_slice(&[id, len]);
                        for _ in 0..len {
                            body.push(g.below(256) as u8);
                        }
                    }
                }
            }
            let mut v: Vec<u8> = Vec::new();
            v.extend_from_slice(&0x100u32.to_le_bytes());
            v.extend_from_slice(&(body.len() as u32).to_le_bytes());
            v.extend_from_slice(&body);
            if g.below(3) == 0 {
                // another top-level chunk, of data
                v.extend_from_slice(&0x200u32.to_le_bytes());
                v.extend_from_slice(&3u32.to_le_bytes());
                v.extend_from_slice(&[9, 9, 9]);
            }
            v
        };
        let mut t = TwiddlerClass::new();
        t.set_indirect_class_id(0xA5A5A5A5);
        t.definition_list = vec![777];
        {
            let mut cl = ChunkLoad::new(GameFile::new(&stream).unwrap());
            while cl.open_chunk().unwrap_or(false) {
                if cl.cur_chunk_id().unwrap_or(0) == 0x100 {
                    t.load_variables(&mut cl);
                }
                let _ = cl.close_chunk();
            }
        }
        let mut rust: Vec<u32> = vec![t.get_indirect_class_id(), t.definition_list.len() as u32];
        rust.extend(t.definition_list.iter().map(|&v| v as u32));
        let mut out = vec![0u32; stream.len() + 8];
        let n = unsafe { ref_tw_load(stream.as_ptr(), stream.len() as i32, out.as_mut_ptr()) } as usize;
        let m0 = load.mismatches;
        load.eq(rust, out[..n].to_vec(), || format!("stream {stream:?}"));
        if load.mismatches > m0 {
            load_bad += 1;
        }
    }
    conclude(save, save_bad, &[14902, 14905]);
    conclude(load, load_bad, &[14903, 14904]);
}

#[test]
fn save_ids_and_status_match_the_cpp() {
    let rust_ids = [
        ids::CHUNKID_SAVELOAD_BEGIN, ids::CHUNKID_SAVELOAD_DEFMGR, ids::CHUNKID_TWIDDLER, ids::CHUNKID_WW3D_BEGIN, ids::CHUNKID_WWPHYS_BEGIN,
        ids::CHUNKID_WWAUDIO_BEGIN, ids::CHUNKID_COMBAT_BEGIN, ids::CHUNKID_COMMANDO_EDITOR_BEGIN, ids::CHUNKID_PHYSTEST_BEGIN, ids::CHUNKID_COMMANDO_BEGIN,
        ids::CHUNKID_WWMATH_BEGIN, ids::CHUNKID_WWTRANSLATEDB_BEGIN,
        ids::WWMATH_CHUNKID_LINEARCURVE1D, ids::WWMATH_CHUNKID_HERMITESPLINE1D, ids::WWMATH_CHUNKID_CATMULLROMSPLINE1D, ids::WWMATH_CHUNKID_CARDINALSPLINE1D,
        ids::WWMATH_CHUNKID_TCBSPLINE1D, ids::WWMATH_CHUNKID_LINEARCURVE3D, ids::WWMATH_CHUNKID_HERMITESPLINE3D, ids::WWMATH_CHUNKID_CATMULLROMSPLINE3D,
        ids::WWMATH_CHUNKID_CARDINALSPLINE3D, ids::WWMATH_CHUNKID_TCBSPLINE3D, ids::WWMATH_CHUNKID_VEHICLECURVE,
    ];
    let mut cpp = [0u32; 23];
    unsafe { ref_sl_ids(cpp.as_mut_ptr()) };
    let mut c = Check::new("saveloadids.h / wwmathids.h constants");
    for (i, (r, v)) in rust_ids.iter().zip(cpp.iter()).enumerate() {
        c.eq(*r, *v, || format!("constant {i}"));
    }
    c.finish();

    let mut g = Gen::new(0x215_5747);
    let mut c = Check::new("SaveLoadStatus programs");
    let mut bad_programs = 0u64;
    let texts = ["", "a", "hello world", "sub"];
    for _ in 0..cases(CASES) {
        let mut prog: Vec<u32> = Vec::new();
        for _ in 0..(1 + g.below(30)) {
            match g.below(7) {
                0 | 1 => prog.extend_from_slice(&[0, g.below(2), g.below(4)]),
                2 | 3 => prog.extend_from_slice(&[1, g.below(2)]),
                4 => prog.push(2),
                5 => prog.push(3),
                _ => prog.push(4),
            }
        }
        let mut rust: Vec<u32> = Vec::new();
        let mut i = 0;
        while i < prog.len() {
            match prog[i] {
                0 => {
                    status::set_status_text(texts[prog[i + 2] as usize], prog[i + 1] as usize);
                    i += 3;
                }
                1 => {
                    let s = status::get_status_text(prog[i + 1] as usize);
                    rust.push(s.len() as u32);
                    rust.extend(s.bytes().map(|b| b as u32));
                    i += 2;
                }
                2 => {
                    status::reset_status_count();
                    i += 1;
                }
                3 => {
                    status::inc_status_count();
                    i += 1;
                }
                _ => {
                    rust.push(status::get_status_count() as u32);
                    i += 1;
                }
            }
        }
        let mut out = vec![0u32; prog.len() * 12 + 4];
        let n = unsafe { ref_sl_status(prog.as_ptr(), prog.len() as i32, out.as_mut_ptr()) } as usize;
        let m0 = c.mismatches;
        c.eq(rust, out[..n].to_vec(), || format!("program {prog:?}"));
        if c.mismatches > m0 {
            bad_programs += 1;
        }
    }
    conclude(c, bad_programs, &[14907, 14908]);
}
