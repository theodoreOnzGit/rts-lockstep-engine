#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation drafts of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameFont.cpp (FontLibrary::linkFont, unlinkFont, deleteAllFonts)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/HeaderTemplate.cpp (findHeaderTemplate, newHeaderTemplate, getFirstHeader,
//   getNextHeader, populateGameFonts)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::{game_font, header_template}` against upstream's (#215).
//!
//! # Method
//!
//! The functions and the classes they belong to are cut verbatim (build.rs, drafts_guilogic3) into
//! `differential/wrap_draft_fonts.cpp`, on upstream's own AsciiString.cpp. 6,000 random programs for each class, each of up to 60
//! operations, run through both and the observable state (the library: count and the order of the list; the manager: each
//! operation's result and, per template, its font) compared after every dump.
//!
//! Fonts: make, link (only a font that is not linked: linking a linked font makes a cycle upstream, which no caller does), unlink
//! (a linked font, a font that is made but not linked, NULL), delete-all (a font it deletes is not used again), dump.
//! Header templates: new (names `h0`..`h5`, so that finds meet duplicates, and finds by `H0`..`H7`, which a case-sensitive compare never matches), find, first, next (a template at any place, the last, and
//! one outside the list), set the fields (font name `f<k>` or `n<k>`, which has no font; point sizes from -50 to 100 and, in a tenth
//! of them, up to 2^30, where `adjustFontSize`'s Int goes through a `Real` and back), populate, dump.
//!
//! Excluded: `releaseFontData`'s device work (`FontCharsClass::Release_Ref` of the font pages): the stand-in only clears `fontData`,
//! as the draft does, and the font is deleted at once, so nothing else is observable. `FontLibrary::getFont` and
//! `GlobalLanguage::adjustFontSize` are stand-ins (the wrapper's header says what they return).
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! 0 mismatches: list operations and a conversion. The one place I would look is the Int to Real to Int of the point size.
//!
//! # Results
//!
//! 2026-11-19: 6,000 programs per class, **0 mismatches** in both, first run (prediction met: the Int to Real to Int of the point size, tested up to 2^30, agrees).
//! Planted bugs, 14 of 14 caught (mismatches of 6,000): fonts 15669 4,096 (link at the tail); 15670 5,225 (count not incremented); 15671 4,367 (count not
//! decremented); 15672 4,929 (count decremented for a font not in the list); 15673 2,131 (delete-all leaves one); 15674 2,224 (unlink removes the next font);
//! header templates 15675 5,309 (new at the back); 15676 1,601 (find returns the last match); 15677 1,430 (find ignores case: **missed by the first generator**, which
//! only looked up lower-case names; finds by `H<k>` were added and it is caught); 15678 2,978 (next wraps round); 15679 4,102 (first is the last); 15680 5,049 (populate
//! ignores the language adjustment); 15681 5,049 (bold negated); 15682 2,221 (a missing font leaves the old one).

mod support;

use rts_lockstep_engine::client::window_script::GameFont;
use rts_lockstep_engine::drafts::sweep_gui_logic::game_font::{FontLibrary, LibraryFont};
use rts_lockstep_engine::drafts::sweep_gui_logic::header_template::{HeaderTemplateFontHost, HeaderTemplateManager};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_fonts_run(inp: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
    fn ref_ht_run(inp: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn run_cpp(f: unsafe extern "C" fn(*const i32, i32, *mut i32, i32) -> i32, prog: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 4096];
    let n = unsafe { f(prog.as_ptr(), prog.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0, "buffer too small");
    out.truncate(n as usize);
    out
}

#[derive(Clone, Copy, PartialEq)]
enum St {
    Fresh,
    Linked,
    Unlinked,
    Dead,
}

fn font_of(id: i32) -> LibraryFont {
    LibraryFont { font: GameFont { name: format!("f{id}").into_bytes(), point_size: id, bold: false }, font_data: Some(1) }
}

fn dump_lib(lib: &FontLibrary) -> Vec<i32> {
    let mut v = vec![lib.count];
    v.extend(lib.font_list.iter().map(|f| f.font.point_size));
    v.push(-1);
    v
}

fn fonts_program(g: &mut Gen) -> (Vec<i32>, Vec<i32>) {
    let mut prog = Vec::new();
    let mut want = Vec::new();
    let mut lib = FontLibrary::default();
    let mut st = vec![St::Dead; 512];
    let mut made = 0usize;
    for _ in 0..(1 + g.below(60)) {
        match g.below(10) {
            0..=2 if made < 500 => {
                st[made] = St::Fresh;
                prog.extend([0, made as i32]);
                made += 1;
            }
            3 | 4 => {
                let cands: Vec<usize> = (0..made).filter(|&i| st[i] == St::Fresh || st[i] == St::Unlinked).collect();
                if let Some(&i) = cands.get(g.below(cands.len().max(1) as u32) as usize) {
                    prog.extend([1, i as i32]);
                    lib.link_font(Some(font_of(i as i32)));
                    st[i] = St::Linked;
                }
            }
            5 | 6 => {
                let cands: Vec<usize> = (0..made).filter(|&i| st[i] != St::Dead).collect();
                if g.below(12) == 0 || cands.is_empty() {
                    prog.extend([2, -1]);
                    lib.unlink_font(None);
                } else {
                    let i = cands[g.below(cands.len() as u32) as usize];
                    prog.extend([2, i as i32]);
                    let place = lib.font_list.iter().position(|f| f.font.point_size == i as i32).unwrap_or(lib.font_list.len());
                    let _ = lib.unlink_font(Some(place));
                    if st[i] == St::Linked {
                        st[i] = St::Unlinked;
                    }
                }
            }
            7 if g.below(4) == 0 => {
                prog.push(3);
                lib.delete_all_fonts();
                for s in st.iter_mut() {
                    if *s == St::Linked {
                        *s = St::Dead;
                    }
                }
            }
            _ => {
                prog.push(4);
                want.extend(dump_lib(&lib));
            }
        }
    }
    prog.push(4);
    want.extend(dump_lib(&lib));
    (prog, want)
}

struct Host;

impl HeaderTemplateFontHost for Host {
    fn adjust_font_size(&self, point: i32) -> f32 {
        // the wrapper's GlobalLanguage::adjustFontSize: an Int, p * 2 - 7 (wrapping, as the Int does)
        point.wrapping_mul(2).wrapping_sub(7) as f32
    }
    fn get_font(&self, name: &[u8], point_size: i32, bold: bool) -> Option<GameFont> {
        if name.first() == Some(&b'n') {
            None
        } else {
            Some(GameFont { name: name.to_vec(), point_size, bold })
        }
    }
}

fn dump_ht(m: &HeaderTemplateManager) -> Vec<i32> {
    let mut v = vec![m.header_template_list.len() as i32];
    for t in &m.header_template_list {
        v.push(t.font.is_some() as i32);
        v.push(t.font.as_ref().map_or(0, |f| f.point_size));
        v.push(t.font.as_ref().map_or(0, |f| f.bold as i32));
    }
    v
}

fn ht_program(g: &mut Gen) -> (Vec<i32>, Vec<i32>) {
    let mut prog = Vec::new();
    let mut want = Vec::new();
    let mut m = HeaderTemplateManager::default();
    let opt = |p: Option<usize>| p.map_or(-1, |i| i as i32);
    for _ in 0..(1 + g.below(60)) {
        let len = m.header_template_list.len();
        match g.below(12) {
            0..=2 => {
                let k = g.below(6) as i32;
                prog.extend([0, k]);
                want.push(m.new_header_template(format!("h{k}").as_bytes()) as i32);
            }
            3 => {
                // names h0..h7, and the same with a capital H (a case-sensitive compare finds none of those)
                let (k, big) = (g.below(8) as i32, g.below(3) == 0);
                prog.extend([1, k + if big { 100 } else { 0 }]);
                want.push(opt(m.find_header_template(format!("{}{k}", if big { 'H' } else { 'h' }).as_bytes())));
            }
            4 => {
                prog.push(2);
                want.push(opt(m.get_first_header()));
            }
            5 | 6 => {
                let place = g.below(len as u32 + 2) as usize;
                prog.extend([3, place as i32]);
                want.push(opt(m.get_next_header(place)));
            }
            7 | 8 if len > 0 => {
                let place = g.below(len as u32) as usize;
                let name = g.below(5) as i32 + if g.below(3) == 0 { 100 } else { 0 };
                let point = if g.below(10) == 0 { g.below(1 << 30) as i32 } else { g.below(151) as i32 - 50 };
                let bold = g.below(2) as i32;
                prog.extend([4, place as i32, name, point, bold]);
                let t = &mut m.header_template_list[place];
                t.font_name = format!("{}{}", if name < 100 { 'f' } else { 'n' }, name % 100).into_bytes();
                t.point = point;
                t.bold = bold != 0;
            }
            9 | 10 => {
                prog.push(5);
                m.populate_game_fonts(&Host);
            }
            _ => {
                prog.push(6);
                want.extend(dump_ht(&m));
            }
        }
    }
    prog.push(6);
    want.extend(dump_ht(&m));
    (prog, want)
}

#[test]
fn fonts_and_header_templates() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_F047);
    let mut fl = Check::new("FontLibrary: link, unlink, deleteAll");
    let mut ht = Check::new("HeaderTemplateManager: find, new, first, next, populate");
    for i in 0..6_000 {
        let (prog, want) = fonts_program(&mut g);
        fl.eq(want, run_cpp(ref_fonts_run, &prog), || format!("program {i}: {prog:?}"));
        let (prog, want) = ht_program(&mut g);
        ht.eq(want, run_cpp(ref_ht_run, &prog), || format!("program {i}: {prog:?}"));
    }
    if planted() {
        fl.report();
        ht.report();
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    }
    fl.finish();
    ht.finish();
}
