#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindow.cpp (the leaf finders, the draw data, callback and
//   instance data setters and getters)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::game_window` against upstream's (#215).
//!
//! # Method
//!
//! The upstream functions are cut verbatim (build.rs, `drafts_guilogic4`) into `differential/wrap_draft_gwin.cpp`; the stand-ins
//! (a small `GameWindow`, `WinInstanceData`, `DisplayString`, callback handles) are listed in its header.
//!
//! * **Leaf finders** (`findFirstLeaf`, `findLastLeaf`, `findPrevLeaf`, `findNextLeaf`): 20,000 random forests of 1 to 14 windows
//!   (several roots, parents among the earlier windows, the sibling order shuffled, each window a tab stop with probability about
//!   a third, other status bits random). The C++ gets the `m_parent`, `m_child`, `m_next`, `m_prev` pointers as indices computed
//!   from the draft's lists (`children`, `window_list`) by the test itself; every function is called from every window; the
//!   window found (or none) is compared.
//! * **Setters and getters**: 20,000 random sequences of 1 to 24 operations on one window, set up with random values on both sides
//!   (status, cursor, region, text colours, draw data of three states, instance data with text and tooltip text of 0 to 8
//!   characters or none). The operations: the six draw data setters (index from -3 to 11: the range check), `winSetDrawOffset`,
//!   `winGetDrawOffset` (each output pointer given or NULL), `winSetWindowId`, `winSetSystemFunc`, `winSetDrawFunc`,
//!   `winSetCallbacks` (a handle 0 is NULL: the manager's default), `winSetLayout`, `winSetInstanceData` (random data, with a text,
//!   tooltip text and a video buffer or none), `winDrawWindow` (how many times the draw callback ran), `winGetCursorPosition`,
//!   `winGetRegion`, `winGetTextLength`, the text colour getters and the callback getters. Every return value and output is
//!   compared, and the window's whole state after each sequence.
//!
//! Mapping: the draft's `DisplayString` is `GameWindow::text` (`Option<Vec<u16>>`, none when empty) and `inst.tooltip_text`; the data's
//! text of `winSetInstanceData` is the draft's `text_label_string` (bytes, widened), as the draft documents. Callback handles map to
//! the draft's enums: system 1000 `Default`, 1 `PushButton`, 2 `CheckBox`, 3 `StaticText`, 4 `ListBox`, 5 `Host("x")`; draw 2000 `W3DDefault`
//! (the W3D manager's default), 1.. `PushButton`, `CheckBox`, `GameWinDefault`, `NoDraw`, `Host("d")`; input 3000 `Default`, 1.. `Block`, ...
//!
//! Excluded, and why: a NULL `x` or `y` of `winGetCursorPosition` and a NULL `region` of `winGetRegion` (the draft returns both
//! values; it has no NULL), a NULL `m_draw` in `winDrawWindow` (the draft's draw callback is never absent: it defaults to the
//! manager's), `winGetEnabledTextBorderColor` (not in the draft; the verified code has it). `winGetWindowList` and the layout list
//! pointers are fields, not logic.
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! 0 mismatches in the leaf finders (a direct translation of pointer chasing); 0 in the setters, except that I expect to find
//! nothing wrong with the instance data copy but would not be surprised by a text or tooltip mix-up.
//!
//! # Results
//!
//! 2026-11-20: leaf finders, 20,000 forests, 149,842 calls of each finder, **0 mismatches** (prediction met). Setters and getters, 20,000 sequences, 250,371
//! operation comparisons and 40,000 state comparisons: the first run had **6,333 mismatches in the state comparisons**, not predicted: `winSetInstanceData` copies
//! the data's `m_id` into `m_instData` (`winGetWindowId()` then answers it), and the draft left the window's own `id` at its old value (fixed in `game_window.rs`);
//! then 0 mismatches. Two corrections to the test itself before that (a callback handle that mapped back wrongly; the IME composite colours are the host's in the
//! draft, and upstream's copy replaces them with the data's: the test sets them from the data after the call).
//! Planted bugs, 13 of 13 caught (mismatches; operations / states for the setters, calls for the finders): 15861 87,830 (`findLastLeaf`) and 33,056 (`findPrevLeaf`);
//! 15862 12,510; 15863 40,577; 15864 27,954; 15865 18,894; 15866 5,703 / 3,191; 15867 0 / 4,350; 15868 87 / 293; 15869 778 / 1,793; 15870 2,995 / 0; 15871 0 / 6,420;
//! 15872 0 / 1,315; 15873 11,862 / 0. (Two first versions were equivalent mutants and are replaced, as the brief says: 15863 skipped `findNextLeaf`'s tab stop
//! shortcut, which the loop below it repeats; 15872 skipped restoring the window's own text, which the draft never overwrites because it keeps the text outside `inst`.)
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 15861 `findLastLeaf` does not walk to the last sibling; 15862 `findPrevLeaf` descends past tab stops; 15863 `findNextLeaf`
//! stops at a leaf that is a tab stop only when it has no child (`&&` for `||`); 15864 `findNextLeaf` lacks the `leaf->m_parent` guard going up; 15865 `findPrevLeaf` going up uses the next
//! sibling; 15866 the draw data index 0 is out of range; 15867 `winSetHiliteColor` sets the border colour; 15868 `winSetSystemFunc(NULL)`
//! keeps the old callback; 15869 `winSetDrawFunc(NULL)` gives the engine's empty draw; 15870 `winGetDrawOffset` swaps x and y; 15871
//! `winSetWindowId` leaves the window's own id; 15872 `winSetInstanceData` sets the tooltip text even from an empty one; 15873 `winDrawWindow` draws a hidden window.

mod support;

use rts_lockstep_engine::client::window_manager::{DrawFunc, GameWindowManager, InputFunc, NoGuiHost, SystemFunc};
use rts_lockstep_engine::client::window_script::{win_status, WinDrawData, WinInstanceData, MAX_DRAW_DATA};
use rts_lockstep_engine::drafts::sweep_gui_logic::game_window as gw;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_gwin_leaf(n: i32, parent: *const i32, child: *const i32, next: *const i32, prev: *const i32, status: *const u32, which: i32, start: i32) -> i32;
    fn ref_gwin_setup(v: *const i64);
    fn ref_gwin_set_data(v: *const i64);
    fn ref_gwin_op(op: i32, a: i64, b: i64, c: i64, out: *mut i64) -> i32;
    fn ref_gwin_dump(o: *mut i64) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish(mut checks: Vec<Check>) {
    if planted() {
        for c in checks.drain(..) {
            c.report();
        }
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    }
    for c in checks {
        c.finish();
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// the leaf finders
// ---------------------------------------------------------------------------------------------------------------------

fn shuffle<T>(g: &mut Gen, v: &mut [T]) {
    for i in (1..v.len()).rev() {
        v.swap(i, g.below(i as u32 + 1) as usize);
    }
}

#[test]
fn leaf_finders() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_6A17);
    let names = ["findFirstLeaf", "findLastLeaf", "findPrevLeaf", "findNextLeaf"];
    let mut checks: Vec<Check> = names.iter().map(|n| Check::new(&format!("GameWindow::{n}"))).collect();
    let mut host = NoGuiHost;
    for _ in 0..20_000 {
        let n = 1 + g.below(14) as usize;
        let mut wm = GameWindowManager::new((800, 600));
        for i in 0..n {
            let parent = if i > 0 && g.below(10) < 7 { Some(g.below(i as u32) as usize) } else { None };
            let tab = g.below(3) == 0;
            let status = (g.u32() & !win_status::TAB_STOP) | if tab { win_status::TAB_STOP } else { 0 };
            wm.win_create(&mut host, parent, status, 0, 0, 10, 10, SystemFunc::Default, None);
        }
        // the sibling order is random
        let mut lists: Vec<Vec<usize>> = (0..n).map(|i| wm.tree.windows[i].children.clone()).collect();
        for l in lists.iter_mut() {
            shuffle(&mut g, l);
        }
        for (i, l) in lists.into_iter().enumerate() {
            wm.tree.windows[i].children = l;
        }
        let mut roots = wm.window_list.clone();
        shuffle(&mut g, &mut roots);
        wm.window_list = roots;
        // the pointers of upstream's GameWindow, from the lists
        let (mut parent, mut child, mut next, mut prev) = (vec![-1i32; n], vec![-1i32; n], vec![-1i32; n], vec![-1i32; n]);
        let mut status = vec![0u32; n];
        for i in 0..n {
            status[i] = wm.tree.windows[i].status;
            parent[i] = wm.tree.windows[i].parent.map_or(-1, |p| p as i32);
            child[i] = wm.tree.windows[i].children.first().map_or(-1, |&c| c as i32);
            let list = match wm.tree.windows[i].parent {
                Some(p) => &wm.tree.windows[p].children,
                None => &wm.window_list,
            };
            let at = list.iter().position(|&x| x == i).expect("in its list");
            if at > 0 {
                prev[i] = list[at - 1] as i32;
            }
            if at + 1 < list.len() {
                next[i] = list[at + 1] as i32;
            }
        }
        for start in 0..n {
            let rust = [
                Some(gw::find_first_leaf(&wm, start)),
                Some(gw::find_last_leaf(&wm, start)),
                gw::find_prev_leaf(&wm, start),
                gw::find_next_leaf(&wm, start),
            ];
            for which in 0..4 {
                let cpp = unsafe { ref_gwin_leaf(n as i32, parent.as_ptr(), child.as_ptr(), next.as_ptr(), prev.as_ptr(), status.as_ptr(), which as i32, start as i32) };
                let r = rust[which].map_or(-1, |x| x as i32);
                checks[which].eq(r, cpp, || format!("forest parent {parent:?} next {next:?} status {status:x?} from {start}"));
            }
        }
    }
    finish(checks);
}

// ---------------------------------------------------------------------------------------------------------------------
// the setters and getters
// ---------------------------------------------------------------------------------------------------------------------

const SYS: [(i64, Option<SystemFunc>); 6] = [(0, None), (1000, Some(SystemFunc::Default)), (1, Some(SystemFunc::PushButton)), (2, Some(SystemFunc::CheckBox)), (3, Some(SystemFunc::StaticText)), (4, Some(SystemFunc::ListBox))];

fn sys_of(h: i64) -> Option<SystemFunc> {
    if h == 5 {
        return Some(SystemFunc::Host(b"x".to_vec()));
    }
    SYS.iter().find(|(k, _)| *k == h).and_then(|(_, v)| v.clone())
}
fn sys_h(s: &SystemFunc) -> i64 {
    match s {
        SystemFunc::Default => 1000,
        SystemFunc::PushButton => 1,
        SystemFunc::CheckBox => 2,
        SystemFunc::StaticText => 3,
        SystemFunc::ListBox => 4,
        SystemFunc::Host(_) => 5,
        _ => -1,
    }
}
fn draw_of(h: i64) -> Option<DrawFunc> {
    match h {
        0 => None,
        2000 => Some(DrawFunc::W3DDefault),
        1 => Some(DrawFunc::PushButton),
        2 => Some(DrawFunc::CheckBox),
        3 => Some(DrawFunc::GameWinDefault),
        4 => Some(DrawFunc::NoDraw),
        _ => Some(DrawFunc::Host(b"d".to_vec())),
    }
}
fn draw_h(d: &DrawFunc) -> i64 {
    match d {
        DrawFunc::W3DDefault => 2000,
        DrawFunc::PushButton => 1,
        DrawFunc::CheckBox => 2,
        DrawFunc::GameWinDefault => 3,
        DrawFunc::NoDraw => 4,
        DrawFunc::Host(_) => 5,
        _ => -1,
    }
}
fn input_of(h: i64) -> Option<InputFunc> {
    match h {
        0 => None,
        3000 => Some(InputFunc::Default),
        1 => Some(InputFunc::Block),
        2 => Some(InputFunc::PushButton),
        3 => Some(InputFunc::ListBox),
        _ => Some(InputFunc::Host(b"i".to_vec())),
    }
}
fn input_h(d: &InputFunc) -> i64 {
    match d {
        InputFunc::Default => 3000,
        InputFunc::Block => 1,
        InputFunc::PushButton => 2,
        InputFunc::ListBox => 3,
        InputFunc::Host(_) => 4,
        _ => -1,
    }
}
fn tip_of(h: i64) -> Option<Vec<u8>> {
    (h != 0).then(|| format!("tt{h}").into_bytes())
}
fn tip_h(t: &Option<Vec<u8>>) -> i64 {
    t.as_ref().map_or(0, |v| String::from_utf8_lossy(&v[2..]).parse().expect("handle"))
}
fn image_of(h: i64) -> Option<Vec<u8>> {
    (h != 0).then(|| h.to_string().into_bytes())
}
fn image_h(i: &Option<Vec<u8>>) -> i64 {
    i.as_ref().map_or(0, |v| String::from_utf8_lossy(v).parse().expect("handle"))
}

fn color(g: &mut Gen) -> i32 {
    match g.below(8) {
        0 => 0x00FF_FFFF,
        1 => 0,
        2 => -1,
        _ => g.i32(),
    }
}

/// A text of 0 to 8 characters as the 9 numbers the wrapper takes (length, characters), length -1: none.
fn text(g: &mut Gen) -> (Vec<i64>, Option<Vec<u16>>) {
    let len = match g.below(4) {
        0 => -1,
        1 => 0,
        _ => 1 + g.below(8) as i64,
    };
    let mut v = vec![len];
    let mut chars = Vec::new();
    for _ in 0..8 {
        let c = 0x20 + g.below(0x5f) as u16; // ASCII: the draft's text_label_string is bytes
        v.push(c as i64);
        chars.push(c);
    }
    if len < 0 {
        return (v, None);
    }
    chars.truncate(len as usize);
    (v, Some(chars))
}

fn draw_set(g: &mut Gen) -> ([WinDrawData; MAX_DRAW_DATA], Vec<i64>) {
    let mut set: [WinDrawData; MAX_DRAW_DATA] = Default::default();
    let mut v = Vec::new();
    for item in set.iter_mut() {
        let img = g.below(4) as i64;
        let (c, b) = (color(g), color(g));
        *item = WinDrawData { image: image_of(img), color: c, border_color: b };
        v.extend([img, c as i64, b as i64]);
    }
    (set, v)
}

struct State {
    wm: GameWindowManager,
    cursor: (i32, i32),
    ime: (i32, i32),
}

fn setup(g: &mut Gen) -> (State, Vec<i64>) {
    let mut v: Vec<i64> = Vec::new();
    let mut wm = GameWindowManager::new((800, 600));
    let w = wm.win_create(&mut NoGuiHost, None, 0, 0, 0, 10, 10, SystemFunc::Default, None);
    assert_eq!(w, 0);
    let status = g.u32();
    let cursor = (g.i32(), g.i32());
    let region = (g.i32(), g.i32(), g.i32(), g.i32());
    v.push(status as i64);
    v.extend([cursor.0 as i64, cursor.1 as i64]);
    v.extend([region.0 as i64, region.1 as i64, region.2 as i64, region.3 as i64]);
    let tc: Vec<i32> = (0..8).map(|_| color(g)).collect();
    v.extend(tc.iter().map(|&c| c as i64));
    let (id, state, style, istatus) = (g.i32(), g.i32(), g.u32(), g.u32());
    let offset = (g.i32(), g.i32());
    let delay = g.i32();
    v.extend([id as i64, state as i64, style as i64, istatus as i64, offset.0 as i64, offset.1 as i64, delay as i64]);
    let (tv, ttext) = text(g);
    let (pv, ptext) = text(g);
    v.extend(tv);
    v.extend(pv);
    let (e, ev) = draw_set(g);
    let (d, dv) = draw_set(g);
    let (h, hv) = draw_set(g);
    v.extend(ev);
    v.extend(dv);
    v.extend(hv);
    let win = &mut wm.tree.windows[w];
    win.status = status;
    win.region = region;
    win.id = id;
    win.text = ttext.filter(|t| !t.is_empty());
    let inst = &mut win.inst;
    inst.enabled_text.color = tc[0];
    inst.enabled_text.border_color = tc[1];
    inst.disabled_text.color = tc[2];
    inst.disabled_text.border_color = tc[3];
    inst.hilite_text.color = tc[4];
    inst.hilite_text.border_color = tc[5];
    inst.id = id;
    inst.state = state;
    inst.style = style;
    inst.status = istatus;
    inst.image_offset = offset;
    inst.tooltip_delay = delay;
    inst.tooltip_text = ptext;
    inst.enabled_draw_data = e;
    inst.disabled_draw_data = d;
    inst.hilite_draw_data = h;
    (State { wm, cursor, ime: (tc[6], tc[7]) }, v)
}

/// The same numbers as the C++ `ref_gwin_dump`.
fn dump(s: &State) -> Vec<i64> {
    let w = &s.wm.tree.windows[0];
    let e = &s.wm.ext[0];
    let i = &w.inst;
    let mut o: Vec<i64> = vec![sys_h(&e.system), input_h(&e.input), draw_h(&e.draw), tip_h(&e.tooltip), e.layout.map_or(0, |l| l as i64 + 1)];
    o.extend([i.id as i64, i.state as i64, i.style as i64, i.status as i64, i.image_offset.0 as i64, i.image_offset.1 as i64, i.tooltip_delay as i64]);
    o.extend([i.enabled_text.color, i.enabled_text.border_color, i.disabled_text.color, i.disabled_text.border_color, i.hilite_text.color, i.hilite_text.border_color].map(|c| c as i64));
    for set in [&i.enabled_draw_data, &i.disabled_draw_data, &i.hilite_draw_data] {
        for d in set.iter() {
            o.extend([image_h(&d.image), d.color as i64, d.border_color as i64]);
        }
    }
    o.push(0); // the draft has no video buffer: always none
    let t = s.wm.win_get_text(0);
    o.push(t.len() as i64);
    for k in 0..8 {
        o.push(t.get(k).map_or(-1, |&c| c as i64));
    }
    let p = i.tooltip_text.as_deref().unwrap_or_default();
    o.push(p.len() as i64);
    for k in 0..8 {
        o.push(p.get(k).map_or(-1, |&c| c as i64));
    }
    o
}

fn random_data(g: &mut Gen) -> (WinInstanceData, Vec<i64>, (i32, i32)) {
    let mut v: Vec<i64> = Vec::new();
    let mut d = WinInstanceData::new();
    d.id = g.i32();
    d.state = g.i32();
    d.style = g.u32();
    d.status = g.u32();
    d.image_offset = (g.i32(), g.i32());
    d.tooltip_delay = g.i32();
    v.extend([d.id as i64, d.state as i64, d.style as i64, d.status as i64, d.image_offset.0 as i64, d.image_offset.1 as i64, d.tooltip_delay as i64]);
    let tc: Vec<i32> = (0..8).map(|_| color(g)).collect();
    v.extend(tc.iter().map(|&c| c as i64));
    d.enabled_text.color = tc[0];
    d.enabled_text.border_color = tc[1];
    d.disabled_text.color = tc[2];
    d.disabled_text.border_color = tc[3];
    d.hilite_text.color = tc[4];
    d.hilite_text.border_color = tc[5];
    let (tv, ttext) = text(g);
    let (pv, ptext) = text(g);
    v.extend(tv);
    v.extend(pv);
    d.text_label_string = ttext.unwrap_or_default().iter().map(|&c| c as u8).collect();
    d.tooltip_text = ptext;
    v.push(g.below(2) as i64); // a video buffer or none (the draft's data has none)
    let (e, ev) = draw_set(g);
    let (dd, dv) = draw_set(g);
    let (h, hv) = draw_set(g);
    v.extend(ev);
    v.extend(dv);
    v.extend(hv);
    d.enabled_draw_data = e;
    d.disabled_draw_data = dd;
    d.hilite_draw_data = h;
    (d, v, (tc[6], tc[7]))
}

#[test]
fn setters_and_getters() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_6A18);
    let mut ops_check = Check::new("GameWindow setters, getters (each op)");
    let mut state_check = Check::new("GameWindow state after a sequence");
    let mut out = [0i64; 16];
    let mut cpp_dump = vec![0i64; 256];
    for seq in 0..20_000 {
        let (mut s, setup_v) = setup(&mut g);
        unsafe { ref_gwin_setup(setup_v.as_ptr()) };
        let steps = 1 + g.below(24);
        let mut log: Vec<String> = Vec::new();
        for _ in 0..steps {
            let op = g.below(21) as i32;
            let idx = g.below(15) as i64 - 3;
            let (a, b, c): (i64, i64, i64);
            let mut rust_ret: i64 = 0;
            let mut rust_out = [0i64; 16];
            let w = 0usize;
            match op {
                0 | 2 | 3 | 5 | 6 => {
                    a = idx;
                    b = color(&mut g) as i64;
                    c = 0;
                    rust_ret = match op {
                        0 => gw::win_set_enabled_color(&mut s.wm, w, idx as i32, b as i32),
                        2 => gw::win_set_disabled_color(&mut s.wm, w, idx as i32, b as i32),
                        3 => gw::win_set_disabled_border_color(&mut s.wm, w, idx as i32, b as i32),
                        5 => gw::win_set_hilite_color(&mut s.wm, w, idx as i32, b as i32),
                        _ => gw::win_set_hilite_border_color(&mut s.wm, w, idx as i32, b as i32),
                    } as i64;
                }
                1 | 4 => {
                    a = idx;
                    b = g.below(4) as i64;
                    c = 0;
                    rust_ret = if op == 1 { gw::win_set_disabled_image(&mut s.wm, w, idx as i32, image_of(b)) } else { gw::win_set_hilite_image(&mut s.wm, w, idx as i32, image_of(b)) } as i64;
                }
                7 => {
                    a = g.i32() as i64;
                    b = g.i32() as i64;
                    c = 0;
                    gw::win_set_draw_offset(&mut s.wm, w, a as i32, b as i32);
                }
                8 => {
                    a = g.below(2) as i64;
                    b = g.below(2) as i64;
                    c = 0;
                    match gw::win_get_draw_offset(&s.wm, w, a != 0, b != 0) {
                        Some((x, y)) => rust_out[..2].copy_from_slice(&[x as i64, y as i64]),
                        None => rust_out[..2].copy_from_slice(&[0x7777, 0x7777]),
                    }
                }
                9 => {
                    a = g.i32() as i64;
                    b = 0;
                    c = 0;
                    rust_ret = gw::win_set_window_id(&mut s.wm, w, a as i32) as i64;
                }
                10 => {
                    a = [0, 1000, 1, 2, 3, 4, 5][g.below(7) as usize];
                    b = 0;
                    c = 0;
                    rust_ret = gw::win_set_system_func(&mut s.wm, w, sys_of(a)) as i64;
                }
                11 => {
                    a = [0, 2000, 1, 2, 3, 4, 5][g.below(7) as usize];
                    b = 0;
                    c = 0;
                    rust_ret = gw::win_set_draw_func(&mut s.wm, w, draw_of(a)) as i64;
                }
                12 => {
                    a = [0, 3000, 1, 2, 3, 4][g.below(6) as usize];
                    b = [0, 2000, 1, 2, 3, 4, 5][g.below(7) as usize];
                    c = g.below(4) as i64;
                    rust_ret = gw::win_set_callbacks(&mut s.wm, w, input_of(a), draw_of(b), tip_of(c)) as i64;
                }
                13 => {
                    a = g.below(4) as i64;
                    b = 0;
                    c = 0;
                    gw::win_set_layout(&mut s.wm, w, (a != 0).then(|| a as usize - 1));
                }
                14 => {
                    let (d, dv, ime) = random_data(&mut g);
                    unsafe { ref_gwin_set_data(dv.as_ptr()) };
                    // the IME composite colours are the host's in the draft (the verified `WinInstanceData` has none); upstream's copy replaces them with the data's
                    s.ime = ime;
                    a = 0;
                    b = 0;
                    c = 0;
                    rust_ret = gw::win_set_instance_data(&mut s.wm, w, &d) as i64;
                }
                15 => {
                    a = 0;
                    b = 0;
                    c = 0;
                    s.wm.tree.windows[w].status = g.u32();
                    // the same status on the C++ side: through its own setup is not possible, so the sequence feeds it as `a`
                    rust_ret = gw::win_draw_window_should_draw(&s.wm, w) as i64;
                }
                16 => {
                    a = 0;
                    b = 0;
                    c = 0;
                    let ((x, y), r) = gw::win_get_cursor_position(s.cursor);
                    rust_out[..2].copy_from_slice(&[x as i64, y as i64]);
                    rust_ret = r as i64;
                }
                17 => {
                    a = 0;
                    b = 0;
                    c = 0;
                    let ((x0, y0, x1, y1), r) = gw::win_get_region(&s.wm, w);
                    rust_out[..4].copy_from_slice(&[x0 as i64, y0 as i64, x1 as i64, y1 as i64]);
                    rust_ret = r as i64;
                }
                18 => {
                    a = 0;
                    b = 0;
                    c = 0;
                    rust_ret = gw::win_get_text_length(&s.wm, w) as i64;
                }
                19 => {
                    a = 0;
                    b = 0;
                    c = 0;
                    let r = [
                        gw::win_get_enabled_text_color(&s.wm, w),
                        gw::win_get_disabled_text_color(&s.wm, w),
                        gw::win_get_disabled_text_border_color(&s.wm, w),
                        gw::win_get_hilite_text_color(&s.wm, w),
                        gw::win_get_hilite_text_border_color(&s.wm, w),
                        gw::win_get_ime_composite_text_color(s.ime),
                        gw::win_get_ime_composite_border_color(s.ime),
                    ];
                    for (k, v) in r.iter().enumerate() {
                        rust_out[k] = *v as i64;
                    }
                }
                _ => {
                    a = 0;
                    b = 0;
                    c = 0;
                    rust_out[0] = sys_h(gw::win_get_system_func(&s.wm, w));
                    rust_out[1] = input_h(&s.wm.ext[w].input);
                    rust_out[2] = draw_h(gw::win_get_draw_func(&s.wm, w));
                    rust_out[3] = tip_h(&gw::win_get_tooltip_func(&s.wm, w).cloned());
                }
            }
            let mut cpp_out = [0i64; 16];
            // op 15: the window status is set on the C++ side through a dedicated op (21)
            if op == 15 {
                let st = s.wm.tree.windows[w].status as i64;
                unsafe { ref_gwin_op(21, st, 0, 0, out.as_mut_ptr()) };
            }
            let cpp_ret = unsafe { ref_gwin_op(op, a, b, c, cpp_out.as_mut_ptr()) } as i64;
            log.push(format!("op {op} ({a},{b},{c})"));
            ops_check.eq((rust_ret, rust_out), (cpp_ret, cpp_out), || format!("seq {seq} {log:?}"));
        }
        let n = unsafe { ref_gwin_dump(cpp_dump.as_mut_ptr()) } as usize;
        let r = dump(&s);
        assert_eq!(r.len(), n, "the dumps have the same length");
        state_check.eq(r, cpp_dump[..n].to_vec(), || format!("seq {seq} {log:?}"));
        state_check.eq(s.wm.tree.windows[0].id as i64, cpp_dump[5], || format!("seq {seq}: the window's own id vs m_id {log:?}"));
    }
    finish(vec![ops_check, state_check]);
}
