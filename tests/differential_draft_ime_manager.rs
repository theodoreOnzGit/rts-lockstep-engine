#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/IMEManager.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::ime_manager` against `IMEManager` (#215, group lc3).
//!
//! # Method
//!
//! `getCompositionString`, `updateCompositionString`, `getResultsString`, `convertToUnicode`, `openCandidateList`, `closeCandidateList`,
//! `updateCandidateList`, `updateProperties`, `getIndexBase`, `resizeCandidateWindow`, `getCandidate`, `getCandidatePageStart`, `openStatusWindow` and
//! `closeStatusWindow` are cut verbatim by build.rs into `differential/wrap_draft_ime_manager.cpp` (its stand-ins, listed there: the Win32 IMM calls answering from
//! a fake block, `MultiByteToWideChar`, `_mbsnccnt`, a candidate list built as the bytes Windows writes, windows with the hide / modal rules of the window manager). A
//! case: a manager with or without a context, five windows (candidate window with its text area and two arrows as children, a status window) of random size and hidden
//! state, and a program of up to 14 operations, each with its own fake IMM answers (wide and ANSI composition and result strings, failures, cursor bits above 0xffff,
//! candidate lists of up to 7 strings in both widths, styles, pages, failing buffer sizes). The Rust runs on the verified `GameWindowManager` with the same windows. After every
//! operation the manager's state, the candidate strings, the five windows' hidden flag and region, the modal stack and the top-level order are compared. 20,000 cases from a fixed seed.
//! EXCLUDED (the draft deviates where upstream is undefined or limited by a buffer): a page size of 0 (upstream divides by zero; the draft guards), strings over
//! MAX_COMPSTRINGLEN (the draft's ImmApi does not carry the buffer size), a candidate window with no text area (upstream dereferences NULL), NUL bytes inside an ANSI string
//! (upstream's -1 length stops at the NUL).
//!
//! # Prediction (written before the first run, 2026-11-20)
//!
//! 0 mismatches expected, though the hide / modal stand-in is the likeliest place for a miss that is not the draft's.
//!
//! # Results
//!
//! 2026-11-20: **0 mismatches over 20,000 cases** (prediction held, including the hide / modal stand-in). Planted bugs, 14 of 14 caught (of 20,000 cases): 19891 2,350; 19892 5,315; 19893 221;
//! 19894 1,645; 19895 7,703; 19896 74; 19897 3,549; 19898 416; 19899 2,281; 19900 1,326; 19901 10,945; 19902 9,056; 19903 1,983; 19904 1,683.
//! (19892 first panicked the test's own slice, and 19894 was first an equivalent mutant, `string_len` = result, hidden by the draft's truncate: both replaced.)
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 19891 the wide cursor not masked with 0xffff; 19892 the wide length is the byte count; 19893 the ANSI cursor is not clamped; 19894 the ANSI result drops its last unit;
//! 19895 updateProperties reads the Unicode bit for the index base; 19896 getCandidate refuses index 0; 19897 the IME98 page-start correction is skipped; 19898 the down arrow hides with `<` for `<=`;
//! 19899 the candidate list is asked for index 0 whatever the flag; 19900 closeCandidateList keeps the count; 19901 the down arrow does not move on a resize; 19902 the window opens at x = 0;
//! 19903 no ANSI fallback when the wide list is unavailable; 19904 an ANSI candidate is widened by a cast, not converted.

mod support;

use rts_lockstep_engine::client::window_manager::{GameWindowManager, NoGuiHost, SystemFunc};
use rts_lockstep_engine::client::window_script::{win_status, WinId, WinInstanceData};
use rts_lockstep_engine::drafts::sweep_gui_logic::ime_manager::{CandidateList, CandidateString, ImeCandidateState, ImmApi, IME_CAND_CODE, IME_CAND_UNKNOWN};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_lc3_ime_run(prog: *const i32, nprog: i32, out: *mut i32) -> i32;
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

#[derive(Clone, Debug, Default)]
struct Fake {
    comp_w_res: i32,
    comp_w: Vec<u16>,
    comp_w_cursor: i32,
    comp_a_res: i32,
    comp_a: Vec<u8>,
    comp_a_cursor: i32,
    res_w_res: i32,
    res_w: Vec<u16>,
    res_a_res: i32,
    res_a: Vec<u8>,
    mb_fail: bool,
    propv: i32,
    cnt_w: i32,
    cnt_a: i32,
    style: u32,
    count: u32,
    sel: u32,
    pstart: u32,
    psize: u32,
    bytes_mode: i32,
    strs: Vec<Vec<u8>>,
}

fn mbsnccnt(buf: &[u8], n: i32) -> i32 {
    let (mut i, mut c) = (0usize, 0);
    while (i as i32) < n && i < buf.len() {
        if buf[i] >= 0x81 && buf[i] <= 0xFE && i + 1 < buf.len() { i += 2 } else { i += 1 }
        c += 1;
    }
    c
}

impl Fake {
    fn needed(&self, wide: bool) -> i32 {
        let mut t = 24 + 4 * self.strs.len() as i32;
        for s in &self.strs {
            t += if wide { 2 * (s.len() as i32 + 1) } else { s.len() as i32 + 1 };
        }
        t
    }
    fn put(&self, out: &mut Vec<i32>) {
        out.push(self.comp_w_res);
        out.push(self.comp_w.len() as i32);
        out.extend(self.comp_w.iter().map(|&u| u as i32));
        out.push(self.comp_w_cursor);
        out.push(self.comp_a_res);
        out.push(self.comp_a.len() as i32);
        out.extend(self.comp_a.iter().map(|&u| u as i32));
        out.push(self.comp_a_cursor);
        out.push(self.res_w_res);
        out.push(self.res_w.len() as i32);
        out.extend(self.res_w.iter().map(|&u| u as i32));
        out.push(self.res_a_res);
        out.push(self.res_a.len() as i32);
        out.extend(self.res_a.iter().map(|&u| u as i32));
        out.extend([self.mb_fail as i32, self.propv, self.cnt_w, self.cnt_a, self.style as i32, self.count as i32, self.sel as i32, self.pstart as i32, self.psize as i32, self.bytes_mode]);
        out.push(self.strs.len() as i32);
        for s in &self.strs {
            out.push(s.len() as i32);
            out.extend(s.iter().map(|&b| b as i32));
        }
    }
}

impl ImmApi for Fake {
    fn get_composition_string_w(&self, _max_bytes: usize) -> (i32, Vec<u16>) {
        (self.comp_w_res, if self.comp_w_res >= 0 { self.comp_w.clone() } else { Vec::new() })
    }
    fn get_composition_cursor_pos_w(&self) -> i32 {
        self.comp_w_cursor
    }
    fn get_composition_string_a(&self, _max_bytes: usize) -> (i32, Vec<u8>) {
        (self.comp_a_res, self.comp_a.clone())
    }
    fn get_composition_cursor_pos_a(&self) -> i32 {
        self.comp_a_cursor
    }
    fn mbsnccnt(&self, buf: &[u8], n: i32) -> i32 {
        mbsnccnt(buf, n)
    }
    fn get_result_string_w(&self, _max_bytes: usize) -> (i32, Vec<u16>) {
        (self.res_w_res, if self.res_w_res >= 0 { self.res_w.clone() } else { Vec::new() })
    }
    fn get_result_string_a(&self, _max_bytes: usize) -> (i32, Vec<u8>) {
        (self.res_a_res, self.res_a.clone())
    }
    fn multi_byte_to_wide_char(&self, mbcs: &[u8]) -> Vec<u16> {
        if self.mb_fail {
            Vec::new()
        } else {
            mbcs.iter().map(|&b| b as u16 + 0x100).collect()
        }
    }
    fn get_candidate_list_count(&self, unicode: bool) -> i32 {
        if unicode { self.cnt_w } else { self.cnt_a }
    }
    fn get_candidate_list(&self, list_index: i32, unicode: bool, size: i32) -> (i32, CandidateList) {
        let strings = self
            .strs
            .iter()
            .enumerate()
            .map(|(k, s)| {
                let v: Vec<u16> = s.iter().enumerate().map(|(j, &b)| b as u16 + if k == 0 && j == 0 { list_index as u16 } else { 0 }).collect();
                if unicode {
                    CandidateString::Wide(v.iter().map(|&u| u + 0x300).collect())
                } else {
                    CandidateString::Ansi(v.iter().map(|&u| u as u8).collect())
                }
            })
            .collect();
        let list = CandidateList { dw_style: self.style, dw_count: self.count, dw_selection: self.sel, dw_page_start: self.pstart, dw_page_size: self.psize, strings };
        let bytes = match self.bytes_mode {
            1 => 0,
            2 => size + 1,
            _ => self.needed(unicode),
        };
        (bytes, list)
    }
    fn get_property(&self) -> i32 {
        self.propv
    }
}

fn bytes(g: &mut Gen, max: u32, hi: u32) -> Vec<u8> {
    (0..g.below(max + 1)).map(|_| (1 + g.below(hi)) as u8).collect()
}

fn cursor(g: &mut Gen) -> i32 {
    g.below(10) as i32 | if g.below(3) == 0 { 0x30000 } else { 0 }
}

fn make_fake(g: &mut Gen) -> Fake {
    let mut f = Fake::default();
    f.comp_w = bytes(g, 5, 200).iter().map(|&b| b as u16 + 0x200).collect();
    f.comp_w_res = if g.below(4) == 0 { -1 } else { 2 * f.comp_w.len() as i32 };
    f.comp_w_cursor = cursor(g);
    f.comp_a = bytes(g, 6, 0xFE);
    f.comp_a_res = if !f.comp_a.is_empty() && g.below(8) != 0 { f.comp_a.len() as i32 } else { -(g.below(2) as i32) };
    f.comp_a_cursor = cursor(g);
    f.res_w = bytes(g, 5, 200).iter().map(|&b| b as u16 + 0x200).collect();
    f.res_w_res = if g.below(4) == 0 { -1 } else { 2 * f.res_w.len() as i32 };
    f.res_a = bytes(g, 6, 0xFE);
    f.res_a_res = if !f.res_a.is_empty() && g.below(8) != 0 { f.res_a.len() as i32 } else { -(g.below(2) as i32) };
    f.mb_fail = g.below(8) == 0;
    f.propv = [0, 0x10000, 0x80000, 0x90000, 0x6789][g.below(5) as usize] | g.below(4) as i32;
    let n = g.below(8) as usize;
    f.strs = (0..n).map(|_| bytes(g, 4, 120)).collect();
    f.style = [0, 1, 2, 3, 4][g.below(5) as usize];
    f.count = n as u32;
    f.psize = 1 + g.below(10);
    f.sel = g.below(n as u32 + 4);
    f.pstart = g.below(n as u32 + 3);
    f.bytes_mode = [0, 0, 0, 0, 0, 0, 1, 2][g.below(8) as usize];
    f.cnt_w = if g.below(3) == 0 { -(g.below(2) as i32) } else { f.needed(true) + g.below(5) as i32 };
    f.cnt_a = if g.below(4) == 0 { -(g.below(2) as i32) } else { f.needed(false) + g.below(5) as i32 };
    f
}

fn put_units(v: &[u16], out: &mut Vec<i32>) {
    out.push(v.len() as i32);
    out.extend(v.iter().map(|&u| u as i32));
}

#[test]
fn ime_manager_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_1E3);
    let mut c = Check::new("IMEManager composition, candidates, windows");
    for case in 0..20_000 {
        let has_ctx = g.below(6) != 0;
        let display = 200 + g.below(1200) as i32;
        // five windows: x0, y0, width, height
        let mut rects = [[0i32; 4]; 5];
        for r in rects.iter_mut() {
            *r = [g.below(100) as i32, g.below(100) as i32, 20 + g.below(300) as i32, 400 + g.below(200) as i32];
        }
        rects[1][3] = g.below(200) as i32;
        let hidden: Vec<bool> = (0..5).map(|_| g.below(2) == 0).collect();
        let has_font = g.below(5) != 0;
        let font_h = 1 + g.below(30) as i32;
        let has_user = g.below(2) == 0;
        let nops = 1 + g.below(14);
        let mut prog: Vec<i32> = vec![has_ctx as i32, display];
        for r in &rects {
            prog.extend([r[0], r[1], r[0] + r[2], r[1] + r[3]]);
        }
        prog.extend(hidden.iter().map(|&h| h as i32));
        prog.extend([has_font as i32, font_h, has_user as i32, nops as i32]);
        let mut ops: Vec<(i32, i32, Fake)> = Vec::new();
        for _ in 0..nops {
            let kind = g.below(12) as i32;
            let arg = match kind {
                4 | 5 | 6 => [0, 1, 2, 3, 4, 5, 8, 12, 33, 63, 1 << 30, -1][g.below(12) as usize],
                7 => g.below(14) as i32,
                10 => g.below(12) as i32 - 2,
                _ => 0,
            };
            let f = make_fake(&mut g);
            prog.push(kind);
            prog.push(arg);
            f.put(&mut prog);
            ops.push((kind, arg, f));
        }
        let mut cpp = vec![0i32; 400_000];
        let len = unsafe { ref_lc3_ime_run(prog.as_ptr(), prog.len() as i32, cpp.as_mut_ptr()) } as usize;
        cpp.truncate(len);

        // the Rust side: the same five windows on the verified window manager
        let mut wm = GameWindowManager::new((display, 800));
        let mut mk = |wm: &mut GameWindowManager, parent: Option<WinId>, r: [i32; 4]| -> WinId {
            let id = wm.win_create(&mut NoGuiHost, parent, 0, r[0], r[1], r[2], r[3], SystemFunc::Default, Some(WinInstanceData::new()));
            let w = wm.win_mut(id);
            w.region = (r[0], r[1], r[0] + r[2], r[1] + r[3]);
            w.size = (r[2], r[3]);
            id
        };
        let stat = mk(&mut wm, None, rects[4]);
        let cand = mk(&mut wm, None, rects[0]);
        let text = mk(&mut wm, Some(cand), rects[1]);
        let up = mk(&mut wm, Some(cand), rects[2]);
        let down = mk(&mut wm, Some(cand), rects[3]);
        let ws = [cand, text, up, down, stat];
        for (i, &w) in ws.iter().enumerate() {
            wm.win_hide(w, hidden[i]);
        }
        let mut st = ImeCandidateState { has_context: has_ctx, candidate_window: Some(cand), candidate_text_area: Some(text), candidate_up_arrow: Some(up), candidate_down_arrow: Some(down), status_window: Some(stat), index_base: 1, ..Default::default() };
        let font = if has_font { Some(font_h) } else { None };
        let mut rust: Vec<i32> = Vec::new();
        let mut host = NoGuiHost;
        for (kind, arg, f) in &ops {
            match *kind {
                0 => st.update_composition_string(f),
                1 => st.get_results_string(f),
                2 => put_units(&ImeCandidateState::convert_to_unicode(f, &f.comp_a), &mut rust),
                3 => st.update_properties(f),
                4 => st.update_candidate_list(f, &mut wm, &mut host, *arg),
                5 => st.open_candidate_list(f, &mut wm, &mut host, *arg, font, 2),
                6 => st.close_candidate_list(&mut wm, *arg),
                7 => st.resize_candidate_window(&mut wm, &mut host, *arg, font, 2),
                8 => st.open_status_window(&mut wm),
                9 => st.close_status_window(&mut wm),
                10 => put_units(&st.get_candidate(*arg), &mut rust),
                _ => put_units(&st.get_composition_string(), &mut rust),
            }
            rust.push(st.composition_cursor_pos);
            rust.push(st.composition_string_length);
            put_units(&st.composition_string[..(st.composition_string_length.max(0) as usize).min(st.composition_string.len())], &mut rust);
            put_units(&st.results_string, &mut rust);
            rust.extend([st.page_size, st.page_start, st.selected_index, st.candidate_count]);
            rust.push(!st.candidate_string.is_empty() as i32);
            for s in &st.candidate_string {
                put_units(s, &mut rust);
            }
            rust.extend([st.get_index_base(), st.unicode_ime as i32, st.get_candidate_page_start()]);
            for &w in &ws {
                let win = wm.win(w);
                rust.extend([(win.status & win_status::HIDDEN != 0) as i32, win.region.0, win.region.1, win.region.2, win.region.3]);
            }
            rust.push(wm.modal.len() as i32);
            for m in &wm.modal {
                rust.push(ws.iter().position(|w| w == m).unwrap() as i32);
            }
            rust.push(wm.window_list.len() as i32);
            for m in &wm.window_list {
                rust.push(ws.iter().position(|w| w == m).unwrap() as i32);
            }
        }
        let _ = (IME_CAND_CODE, IME_CAND_UNKNOWN, has_user);
        c.eq(rust, cpp, || format!("case {case}: ctx {has_ctx} display {display} rects {rects:?} hidden {hidden:?} font {font:?} ops {ops:?}"));
    }
    finish(c);
}
