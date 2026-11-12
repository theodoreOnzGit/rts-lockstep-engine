// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of the window manager and the gadgets (#70: `client::window_manager`, `client::gadget`,
//! `client::gadget_list_box`, `client::gadget_slider`) against upstream's own GameWindowManager.cpp, GameWindow.cpp,
//! WinInstanceData.cpp, GameWindowGlobal.cpp and Gadget*.cpp (commit 0a05454d), copied verbatim by `build.rs` into a
//! program, `wm_ref`, built with `differential/wrap_wm.cpp` (which lists every stand-in).
//!
//! # Method
//!
//! Each case is a random window tree and a random sequence of input, made as the Rust runs it (so that the targets
//! are windows that exist): one or two user windows (the host's `TestSystem` / `TestInput`, which record every
//! message), and in them one to four gadgets made with `gogoGadget*` (list boxes most often, with or without a scroll
//! bar, one to three columns, multi-select, auto-scroll, auto-purge, force-select, scroll-if-at-end, `ONE_LINE`, a
//! title; vertical and horizontal sliders; push buttons, check boxes, radio buttons), then 10 to 70 operations:
//! mouse events (moves, presses and releases left and right, drags with deltas, the wheel) aimed mostly at the
//! gadgets and their inner windows, keys (arrows, tab, enter, space, letters), the clock, list box entries added (end
//! or row, overwrite or not, empty texts), selections set, the list length changed, `GLM_DEL_ENTRY`,
//! `GLM_SCROLL_BUFFER`, `GLM_UPDATE_DISPLAY`, `GLM_TOGGLE_MULTI_SELECTION`, the text read back, the top/bottom entry
//! set, slider positions and ranges, hiding, enabling, focus, capture, modal windows, resizing, destruction.
//!
//! The Rust runs the case first. If it meets upstream's undefined behaviour ([`GameWindowManager::undefined`]), the
//! case stops before that operation and the C++ runs only the operations before it; the reasons are counted. After
//! each operation both sides write the messages the host recorded and a dump: every window (freed, status, state,
//! style, position, size, parent, owner, children, text), every list box's data (length, columns and their widths,
//! the end, insert and selection positions, the display height and position, the total height, the inner windows,
//! the selections, and per row its `listHeight`, `height`, whether it has cells, and the used rows' cells: type,
//! colour, size, text and word wrap), every slider's data (min, max, `numTicks`' bits, position), and the manager
//! (focus, capture, grab window, the window under the mouse, the lone window, the modal stack, the window list, the
//! mouse's tooltip). The dumps must be identical.
//!
//! # Predictions (written before the first run)
//!
//! A handful of mismatches on the first run, in the list box: the hit test and scroll loops (`row_at`, `top_entry`,
//! `adjust_display`) and the row heights after an overwrite; the sliders' float positions; then 0. Undefined cases
//! (skipped): mostly `GLM_DEL_ENTRY` on a row with cells, the full auto-purge + auto-scroll list (`listData[listLength]`),
//! and the empty list's `getListboxBottomEntry` with `scrollIfAtEnd`.
//!
//! # Results (2026-11-10, `WM_CASES=2000 cargo test --release --features differential --test differential_wm`)
//!
//! - **2000 cases, 67,343 operations compared, 0 mismatches** on the first run (the prediction of a few missed).
//! - Stopped where upstream is undefined, counted by reason in the output: mostly `getListboxBottomEntry` reading
//!   `listData[-1]` (with `scrollIfAtEnd`), `GLM_DEL_ENTRY` writing one past the row's cells, list boxes of length 0
//!   and shrunk lists' rows without cells, a destroyed slider still named by its list box.
//! - Planted bugs (`L9_PLANT`, 2000 cases each): 4701 (`adjustDisplay`'s `+ 1`) 103 cases differ, 4702 (the wrap
//!   offset) 879, 4703 (the vertical slider's thumb from the minimum) 291, 4704 (`GLM_SCROLL_BUFFER`'s selection at
//!   0) 19. An earlier 4702 (force-select ignored on the second click) was an equivalent mutant: never caught, since
//!   upstream's next line restores the selection.
//! - ~~Not compared: drawing; the combo box, text entry and tab control (compiled, not driven: not translated yet);
//!   images in list box cells.~~ **Corrected 2026-11-10 (#72):** the combo box, text entry and tab control are driven
//!   (see below); not compared: drawing, images in list box cells.
//!
//! # #72: the text entry, the combo box, the tab control, the default visual
//!
//! Every gadget is now made with `defaultVisual` TRUE or FALSE (a third of them TRUE), and the cases also make combo
//! boxes (editable or not, `maxDisplay` 0..6, `maxChars` -2..300, the filters, the list's length 0..12 with or without
//! a scroll bar, a font given or not), tab controls (any orientation, edges 0..6, tab sizes including 0, 0..9 tabs,
//! disabled tabs), text entries (`maxTextLen` -1..12 or ~300, the four flags, a label or none), progress bars and
//! static texts. The new operations: `GWM_IME_CHAR` sent to an entry (letters, digits, space, `-`, return, 0xE9,
//! 0x4E2D, 0xFF10, 0), `GadgetTextEntrySetText` / `GetText`, `GadgetComboBoxAddEntry`, `Reset`, `SetSelectedPos`,
//! `GetSelectedPos`, `GetText`, `SetText`, `HideList`, the five setters, `SetItemData` / `GetItemData`, clicks on the
//! drop-down button, presses on the tabs, `GadgetTabControlShowSubPane` (-1..8), `CreateSubPanes`,
//! `FixupSubPaneList`; the keys add backspace, delete, escape, home, F1 and control. The dump adds every window's
//! text colours, the draw data (27 images and colours) of the windows each `W` made, and the entries' (text, secret
//! text, `charPos`, `maxTextLen`, flags), combo boxes' and tab controls' data.
//!
//! **Predictions (written before the first run):** a few mismatches on the first run (2 to 5 causes), most likely in
//! the combo box's resizing chain (`GGM_RESIZED` from `winSetSize` reaching the list box and the edit box), in the
//! order of the default visual's writes on the inner windows, and in `charPos` for long texts; glibc's `iswalnum` in
//! the C locale classifies only ASCII, as the translation does, so the characters above 0x7F agree. Then 0
//! mismatches. Stopped as undefined: tab controls with no tabs (`subPanes[-1]` at creation), a zero tab size pressed,
//! `ShowSubPane(-1)` / `(8)`, combo boxes whose children were destroyed, list lengths of 0.
//! Planted bugs (2000 cases each): 4705 (typing allowed up to `maxTextLen` instead of `maxTextLen - 1`) 100 to 200
//! cases; 4706 (`GCM_ADD_ENTRY`'s short list not 16 wider) 400 to 500; 4707 (the tabs' right edge exclusive) 10 to
//! 30; 4708 (the default visual's hilite text border dark blue) 1500 or more, since every scroll bar button and slider
//! thumb gets the visual.
//!
//! **Results (2026-11-10, `WM_CASES=2000`):**
//! - First run (50 cases): 16 cases differed, all from the harness: wrap_wm.cpp wrote `ret ` before making the call
//!   (`g_log << "ret " << call(...)`), so the messages the call sent were logged after it. Fixed by making each call
//!   before logging its result. Then 200 cases: 0 mismatches.
//! - 2000 cases: **1 mismatch**, not in the new gadgets: a window destroyed below the top of the modal stack stayed in
//!   it (`winDestroy` unsets only the top), was freed, became the top when the window above was unset, and the mouse
//!   was routed into it. Upstream reads freed memory there (the compiled upstream found no window, the port found
//!   one): recorded as undefined in `winProcessMouseEvent` (a new unit test pins it). Not predicted.
//! - Then **2000 cases, 70,035 operations compared, 0 mismatches.** Compared, by operation: 28,421 mouse events, 6,319
//!   keys, 1,785 `GWM_IME_CHAR`, 239 entry texts set and 500 read, 544 combo box entries added, 89 resets, 199
//!   selections set and 95 read, 90 + 89 texts read and set, 102 lists hidden, 221 setters, 81 + 119 item data,
//!   217 + 132 + 141 tab control calls, and the gadgets made (by `W` type): 550 combo boxes, 274 tab controls, 593
//!   entries, 297 progress bars, 316 static texts, 1,331 list boxes, 614 sliders, 854 buttons.
//! - Stopped where upstream is undefined, new reasons: 56 `ShowSubPane` past the array, 3 with no tabs, 3 a zero tab
//!   size pressed, 5 a freed pane, 9 a freed combo box child, 1 the freed modal window.
//! - The prediction of a few mismatch causes in the gadgets missed: none was in the translation of the gadgets; the
//!   one cause was in the manager, and one in the harness. The prediction that glibc agrees above 0x7F held as far as
//!   the cases reach (no mismatch; 3 in 10 of the characters typed are 0xE9, 0x4E2D or 0xFF10, into entries of which
//!   about two in five filter); MSVC's classification there stays unverified.
//! - Planted bugs (2000 cases each): 4705 **26** cases differ (predicted 100 to 200: missed, typing up to the limit is
//!   rarer than guessed: entries made with a label already near or past a small `maxTextLen`); 4706 **254**
//!   (predicted 400 to 500: missed, low); 4707 **9** (predicted 10 to 30: just under; in 3 of them the compiled
//!   upstream crashed past the point the planted Rust ate the press); 4708 **1500** (predicted 1500 or more: held).

//!
//! # #74: typed characters (the IME manager's `WM_CHAR`)
//!
//! The text entry's own calls on `TheIMEManager` (`attach(window)` when it takes the focus, `attach(NULL)` when it
//! loses it while attached; GadgetTextEntry.cpp, compiled verbatim) now act on a stand-in whose `attach`, `detatch`
//! and `isAttachedTo` are transcribed from IMEManager.cpp (which needs Win32's IME to compile), and the dump adds the
//! attached window. A new operation, `w`, is `serviceIMEMessage`'s `WM_CHAR` case (transcribed: a unit of 32 or more,
//! or return, goes to the attached window as `GWM_IME_CHAR`): letters, a digit, space (32), 31, return, backspace (8),
//! tab, 0xE9, 0x4E2D, and the low unit of a surrogate pair. Three in ten of the entry operations are now `w` (the
//! random stream changed, so the cases are not those of #72).
//!
//! **Predictions (written before the first run):** 0 mismatches (the attach chain is three lines on each side). A
//! character typed after its entry was destroyed and freed is undefined (a few cases). Planted (2000 cases each): 4709
//! (`WM_CHAR` takes 33 and above: a space is dropped) 20 to 60 cases, since a space must be typed while an entry that
//! takes it is attached; 4710 (the entry does not detach when it loses the focus) 100 to 300.
//!
//! **Results (2026-11-10, `WM_CASES=2000`):** **2000 cases, 70,324 operations compared, 0 mismatches** (798 `w`,
//! 1,000 `I`). Stopped as undefined at `w`: 2 cases (the attached entry freed). Plants: 4709 **14** cases differ
//! (predicted 20 to 60: missed, low: a space typed while an entry is attached is rarer than guessed); 4710 **143**
//! (predicted 100 to 300: held).

//!
//! # #151: `TheGlobalLanguageData`'s `DefaultWindowFont`
//!
//! `winCreate` and `assignDefaultGadgetLook` take the language's `m_defaultWindowFont` when there is language data and
//! its name is not empty, else Times New Roman 14 (GameWindowManager.cpp:1447, 2915). Each case now starts with an `l`
//! operation from its own stream (so the operations after it are #74's): `TheGlobalLanguageData` NULL in a quarter of
//! the cases (the NULL case kept, the shim's), else language data whose font is `Arial Unicode MS`, `Arial`, `Courier
//! New` or `Times New Roman` (the empty name one time in five), 6 to 20 points, bold or not. The dump adds every
//! window's font (name, size, bold).
//!
//! **Predictions (written before the first run, 2026-11-12):** 0 mismatches over 20,000 cases (the fonts' heights now
//! vary, which moves list box rows, wraps and the combo box's list; the arithmetic is #70's). Plants: 9421 (`winCreate`
//! ignores the language) caught in most cases with language data, every window being made by `winCreate`; 9422 (the
//! gadget look ignores it) caught wherever a gadget is made without a font given, nearly as many.
//!
//! **Results (2026-11-12, `WM_CASES=20000`):** **20,000 cases, 719,964 operations compared, 0 mismatches** (20,000 `l`),
//! as predicted. Plants (20,000 cases each): 9421 **11,958** cases differ, 9422 **11,720** (predicted: most of the
//! cases with language data, about 12,000: held). The #74 sequences after the `l` are unchanged, so the undefined
//! stops are #74's kinds.

#![cfg(feature = "differential")]

use rts_lockstep_engine::client::gadget_combo_box::{self as cbx, combo_rt};
use rts_lockstep_engine::client::gadget_list_box::{self as lbx, glm, listbox_rt, CellData, ListboxRt, LISTBOX_IMAGE, LISTBOX_TEXT};
use rts_lockstep_engine::client::gadget_slider::{slider_rt, slider_set_position};
use rts_lockstep_engine::client::gadget_tab_control::{self as tbx, tab_rt};
use rts_lockstep_engine::client::gadget_text_entry::{self as tex, EntryRt};
use rts_lockstep_engine::client::global_language::FontDesc;
use rts_lockstep_engine::client::window_manager::*;
use rts_lockstep_engine::client::window_script::*;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn range(&mut self, a: i32, b: i32) -> i32 {
        a + self.below((b - a + 1) as u64) as i32
    }
    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }
}

/// The host: wrap_wm.cpp's `TestSystem` / `TestInput` and stand-ins, recording what they record.
#[derive(Default)]
struct Host {
    log: String,
}

fn window_in_d1(msg: u32) -> bool {
    matches!(msg, ggm::LEFT_DRAG | ggm::GBM_MOUSE_ENTERING | ggm::GBM_MOUSE_LEAVING | ggm::GBM_SELECTED | ggm::GBM_SELECTED_RIGHT | ggm::GSM_SLIDER_TRACK | ggm::GSM_SLIDER_DONE | glm::SELECTED | glm::DOUBLE_CLICKED | glm::RIGHT_CLICKED | 16430 | 16431 | 16419 | 16422 | 16427)
}

impl GuiHost for Host {
    fn system(&mut self, wm: &mut GameWindowManager, _func: &[u8], win: WinId, msg: u32, d1: u32, d2: u32) -> MsgHandled {
        let _ = write!(self.log, "sys {win} {msg} {d1}");
        if msg == glm::RIGHT_CLICKED {
            let (p, x, y) = wm.list_msg.right_click;
            let _ = write!(self.log, " rc {p} {x} {y}");
        } else if matches!(msg, ggm::FOCUS_CHANGE | ggm::GSM_SLIDER_TRACK | ggm::GSM_SLIDER_DONE | glm::SELECTED | glm::DOUBLE_CLICKED) {
            let _ = write!(self.log, " {d2}");
        }
        self.log.push('\n');
        MsgHandled::Handled
    }
    fn input(&mut self, _wm: &mut GameWindowManager, _func: &[u8], win: WinId, msg: u32, d1: u32, _d2: u32) -> MsgHandled {
        if msg != gwm::MOUSE_POS {
            let _ = writeln!(self.log, "inp {win} {msg} {d1}");
        }
        MsgHandled::Ignored
    }
    fn add_audio_event(&mut self, name: &[u8]) {
        let _ = writeln!(self.log, "audio {}", String::from_utf8_lossy(name));
    }
    fn block_input_left_up(&mut self) {
        self.log.push_str("block\n");
    }
    fn font_height(&self, font: Option<&GameFont>) -> i32 {
        font.map_or(12, |f| f.point_size)
    }
    fn printable_key(&self, key: u8, state: i32) -> u16 {
        if state == 1 { u16::from(key) } else { 0 }
    }
    fn image_size(&self, image: &[u8]) -> Option<(i32, i32)> {
        Some(if image.ends_with(b"Mid") { (7, 10) } else { (5, 10) })
    }
}

fn text(o: &mut String, t: &[u16]) {
    o.push_str(" [");
    for (i, c) in t.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let _ = write!(o, "{c}");
    }
    o.push(']');
}

fn idx(w: Option<WinId>) -> i64 {
    w.map_or(-1, |w| w as i64)
}

fn draw_data(o: &mut String, set: &DrawDataSet) {
    for d in set {
        let name = d.image.as_ref().map_or("-".to_string(), |n| String::from_utf8_lossy(n).into_owned());
        let _ = write!(o, " {name}/{}/{}", d.color, d.border_color);
    }
}

/// The dump, in wrap_wm.cpp's format; the draw data of the windows from `dd_from` on (the ones a `W` made).
fn dump(wm: &GameWindowManager, dd_from: usize) -> String {
    let mut o = String::new();
    for (i, w) in wm.tree.windows.iter().enumerate() {
        if wm.ext[i].freed {
            let _ = writeln!(o, "w{i} freed");
            continue;
        }
        let _ = write!(o, "w{i} st {} state {} style {} at {} {} size {} {} parent {} owner {} kids", w.status, w.inst.state, w.inst.style, w.region.0, w.region.1, w.size.0, w.size.1, idx(w.parent), idx(wm.ext[i].owner));
        for c in &w.children {
            let _ = write!(o, " {c}");
        }
        text(&mut o, wm.win_get_text(i));
        let t = &w.inst;
        // (#151) the window's font
        match &t.font {
            Some(f) => {
                let _ = write!(o, " font {}/{}/{}", String::from_utf8_lossy(&f.name), f.point_size, u8::from(f.bold));
            }
            None => o.push_str(" font -"),
        }
        let _ = writeln!(o, " tx {}/{} {}/{} {}/{}", t.enabled_text.color, t.enabled_text.border_color, t.disabled_text.color, t.disabled_text.border_color, t.hilite_text.color, t.hilite_text.border_color);
        if i >= dd_from {
            o.push_str(" dd");
            draw_data(&mut o, &t.enabled_draw_data);
            draw_data(&mut o, &t.disabled_draw_data);
            draw_data(&mut o, &t.hilite_draw_data);
            o.push('\n');
        }
        match &wm.ext[i].user {
            UserData::Listbox(l) => dump_listbox(&mut o, l),
            UserData::Slider(s) => {
                let _ = writeln!(o, " sl {} {} {} {}", s.min_val, s.max_val, s.num_ticks.to_bits(), s.position);
            }
            UserData::Entry(e) => {
                o.push_str(" te");
                text(&mut o, &e.text.text);
                text(&mut o, &e.s_text.text);
                let b = |x: bool| u8::from(x);
                let _ = writeln!(o, " {} {} {}{}{}{}{}{} {}", e.char_pos, e.max_text_len, b(e.secret_text), b(e.numerical_only), b(e.alpha_numerical_only), b(e.ascii_only), b(e.draw_text_from_start), b(e.received_unichar), e.con_char_pos);
            }
            UserData::Combo(c) => {
                let b = |x: bool| u8::from(x);
                let _ = writeln!(o, " cb {} {} {} {}{}{} {} {} {} {}", b(c.is_editable), c.max_display, c.max_chars, b(c.ascii_only), b(c.letters_and_numbers_only), b(c.dont_hide), c.entry_count, idx(c.drop_down_button), idx(c.edit_box), idx(c.list_box));
            }
            UserData::Tab(t) => {
                let _ = write!(o, " tb {} {} {} {} {} {} {} {} {} {} {} panes", t.tab_orientation, t.tab_edge, t.tab_width, t.tab_height, t.tab_count, t.pane_border, t.active_tab, t.tabs_left_limit, t.tabs_right_limit, t.tabs_top_limit, t.tabs_bottom_limit);
                for p in &t.sub_panes {
                    let _ = write!(o, " {}", idx(*p));
                }
                o.push_str(" dis");
                for d in &t.sub_pane_disabled {
                    let _ = write!(o, " {}", u8::from(*d));
                }
                o.push('\n');
            }
            _ => {}
        }
    }
    let _ = write!(o, "mgr focus {} cap {} grab {} rgn {} lone {} modal", idx(wm.keyboard_focus), idx(wm.mouse_captor), idx(wm.grab_window), idx(wm.curr_mouse_rgn), idx(wm.lone_window));
    for m in &wm.modal {
        let _ = write!(o, " {m}");
    }
    o.push_str(" list");
    for w in &wm.window_list {
        let _ = write!(o, " {w}");
    }
    o.push_str(" tip");
    text(&mut o, if wm.mouse.is_empty { &[] } else { &wm.mouse.display_string });
    let _ = write!(o, " ime {}", idx(wm.ime.get_window()));
    o.push('\n');
    o
}

fn dump_listbox(o: &mut String, l: &ListboxRt) {
    let _ = write!(o, " lb len {} cols {} cw", l.list_length, l.columns);
    if let Some(cw) = &l.column_width {
        for c in cw.iter().take(usize::try_from(l.columns).unwrap_or(0)) {
            let _ = write!(o, " {c}");
        }
    }
    let _ = write!(o, " end {} ins {} sel {} dh {} dp {} th {} up {} down {} slider {} multi {} sels", l.end_pos, l.insert_pos, l.select_pos, l.display_height, l.display_pos, l.total_height, idx(l.up_button), idx(l.down_button), idx(l.slider), u8::from(l.multi_select));
    if l.multi_select {
        if let Some(s) = &l.selections {
            for v in s.iter().take(usize::try_from(l.list_length).unwrap_or(0)).take_while(|&&v| v >= 0) {
                let _ = write!(o, " {v}");
            }
        }
    }
    o.push('\n');
    let Some(rows) = &l.list_data else { return };
    for (r, row) in rows.iter().enumerate().take(usize::try_from(l.list_length).unwrap_or(0)) {
        let _ = write!(o, "  r{r} lh {} h {} c {}", row.list_height, row.height, u8::from(row.cell.is_some()));
        if let (Some(a), true) = (row.cell, (r as i32) < i32::from(l.end_pos)) {
            if let Some(Some(cells)) = l.arrays.get(a) {
                for c in cells.iter().take(usize::try_from(l.columns).unwrap_or(0)) {
                    let _ = write!(o, " {{{} {} {} {}", c.cell_type, c.color, c.width, c.height);
                    match &c.data {
                        CellData::Text(d) if c.cell_type == LISTBOX_TEXT => {
                            text(o, &d.text);
                            let _ = write!(o, " wrap {}", d.word_wrap);
                        }
                        CellData::Image(n) if c.cell_type == LISTBOX_IMAGE => {
                            let _ = write!(o, " img {}", String::from_utf8_lossy(n));
                        }
                        _ => {}
                    }
                    o.push('}');
                }
            }
        }
        o.push('\n');
    }
}

fn units(t: &str) -> Vec<u16> {
    t.bytes().filter(|&b| b != b'_').map(u16::from).collect()
}

/// One operation on the Rust side (wrap_wm.cpp's `run_op`).
fn run_op(wm: &mut GameWindowManager, h: &mut Host, op: &str) {
    let mut it = op.split(' ');
    let c = it.next().expect("op");
    let mut v: Vec<&str> = it.collect();
    let n = |v: &mut Vec<&str>| -> i32 { v.remove(0).parse().expect("number") };
    let w = |i: i32| -> Option<WinId> { (i >= 0).then_some(i as usize) };
    match c {
        "W" => {
            let parent = w(n(&mut v));
            let ty = n(&mut v);
            let (x, y, ww, hh) = (n(&mut v), n(&mut v), n(&mut v), n(&mut v));
            let status = n(&mut v) as u32;
            let style = n(&mut v) as u32;
            let visual = n(&mut v) != 0;
            let mut inst = WinInstanceData { style, ..WinInstanceData::new() };
            match ty {
                0 => {
                    let u = wm.win_create(h, parent, status, x, y, ww, hh, SystemFunc::Host(b"TestSystem".to_vec()), Some(inst));
                    wm.ext[u].input = InputFunc::Host(b"TestInput".to_vec());
                    wm.ext[u].owner = parent;
                }
                1 | 2 | 10 => {
                    wm.gogo_gadget_look(h, parent, status, x, y, ww, hh, inst, GadgetData::None, visual);
                }
                11 => {
                    wm.gogo_gadget_look(h, parent, status, x, y, ww, hh, inst, GadgetData::Text(TextData::default()), visual);
                }
                3 => {
                    let group = n(&mut v);
                    wm.gogo_gadget_look(h, parent, status, x, y, ww, hh, inst, GadgetData::Radio(RadioButtonData { screen: 1, group }), visual);
                }
                4 => {
                    let f: Vec<i32> = (0..8).map(|_| n(&mut v)).collect();
                    let d = ComboBoxData { is_editable: f[0] != 0, max_display: f[1], max_chars: f[2], ascii_only: f[3] != 0, letters_and_numbers_only: f[4] != 0 };
                    let mut t = cbx::combo_template(&d);
                    t.listbox.list_length = f[5] as i16;
                    t.listbox.scroll_bar = f[6] != 0;
                    inst.font = (f[7] != 0).then(|| GameFont { name: b"F".to_vec(), point_size: f[7], bold: false });
                    cbx::gogo_gadget_combo_box(wm, h, parent, status, x, y, ww, hh, inst, t, visual);
                }
                5 => {
                    let f: Vec<i32> = (0..7).map(|_| n(&mut v)).collect();
                    let mut d = TabControlData { tab_orientation: f[0], tab_edge: f[1], tab_width: f[2], tab_height: f[3], tab_count: f[4], pane_border: f[5], ..Default::default() };
                    for (k, x) in d.sub_pane_disabled.iter_mut().enumerate() {
                        *x = (f[6] >> k) & 1 != 0;
                    }
                    wm.gogo_gadget_look(h, parent, status, x, y, ww, hh, inst, GadgetData::TabControl(d), visual);
                }
                9 => {
                    let f: Vec<i32> = (0..5).map(|_| n(&mut v)).collect();
                    let label = v.remove(0);
                    if label != "_" {
                        inst.text_label_string = label.as_bytes().to_vec();
                    }
                    let t = EntryRt { max_text_len: f[0] as i16, secret_text: f[1] != 0, numerical_only: f[2] != 0, alpha_numerical_only: f[3] != 0, ascii_only: f[4] != 0, ..Default::default() };
                    tex::gogo_gadget_text_entry(wm, h, parent, status, x, y, ww, hh, inst, t, visual);
                }
                6 => {
                    let (len, cols) = (n(&mut v), n(&mut v));
                    let f: Vec<bool> = (0..6).map(|_| n(&mut v) != 0).collect();
                    let pct = if cols > 1 { (0..cols).map(|_| n(&mut v)).collect() } else { Vec::new() };
                    let d = ListboxData { list_length: len as i16, columns: cols as i16, column_width_percentage: pct, auto_scroll: f[0], auto_purge: f[1], scroll_bar: f[2], multi_select: f[3], force_select: f[4], scroll_if_at_end: f[5] };
                    wm.gogo_gadget_look(h, parent, status, x, y, ww, hh, inst, GadgetData::Listbox(d), visual);
                }
                _ => {
                    let (min_val, max_val) = (n(&mut v), n(&mut v));
                    wm.gogo_gadget_look(h, parent, status, x, y, ww, hh, inst, GadgetData::Slider(SliderData { min_val, max_val }), visual);
                }
            }
        }
        "M" => {
            let (m, x, y, dx, dy) = (n(&mut v), n(&mut v), n(&mut v), n(&mut v), n(&mut v));
            wm.win_process_mouse_event(h, m as u32, (x, y), (dx, dy));
        }
        "K" => {
            let (k, s) = (n(&mut v), n(&mut v));
            wm.win_process_key(h, k as u8, s as u32);
        }
        "t" => wm.time_ms = n(&mut v) as u32,
        "A" => {
            let l = n(&mut v) as usize;
            let t = units(v.remove(0));
            let (col, row, column, ow) = (n(&mut v), n(&mut v), n(&mut v), n(&mut v));
            let r = lbx::add_entry_text(wm, h, l, &t, col, row, column, ow != 0);
            if wm.undefined.is_none() {
                let _ = writeln!(h.log, "added {r}");
            }
        }
        "R" => lbx::reset(wm, h, n(&mut v) as usize),
        "S" => {
            let l = n(&mut v) as usize;
            let k = n(&mut v);
            let list: Vec<i32> = (0..k).map(|_| n(&mut v)).collect();
            lbx::set_selected(wm, h, l, &list);
        }
        "L" => {
            let l = n(&mut v) as usize;
            lbx::set_list_length(wm, h, l, n(&mut v));
        }
        "D" | "B" | "U" | "X" => {
            let l = n(&mut v) as usize;
            let d = n(&mut v) as u32;
            let m = match c {
                "D" => glm::DEL_ENTRY,
                "B" => glm::SCROLL_BUFFER,
                "U" => glm::UPDATE_DISPLAY,
                _ => glm::TOGGLE_MULTI_SELECTION,
            };
            wm.win_send_system_msg(h, Some(l), m, d, 0);
        }
        "G" => {
            let l = n(&mut v) as usize;
            let (r, col) = (n(&mut v), n(&mut v));
            let (t, color) = lbx::get_text_and_color(wm, h, l, r, col);
            if wm.undefined.is_none() {
                h.log.push_str("text");
                text(&mut h.log, &t);
                let _ = writeln!(h.log, " {color}");
            }
        }
        "V" => {
            let l = n(&mut v) as usize;
            let (top, p) = (n(&mut v), n(&mut v));
            if top != 0 {
                lbx::set_top_visible_entry(wm, h, l, p);
            } else {
                lbx::set_bottom_visible_entry(wm, h, l, p);
            }
        }
        "P" => {
            let s = n(&mut v) as usize;
            slider_set_position(wm, h, s, n(&mut v));
        }
        "Q" => {
            let s = n(&mut v) as usize;
            let (a, b) = (n(&mut v), n(&mut v));
            wm.win_send_system_msg(h, Some(s), ggm::GSM_SET_MIN_MAX, a as u32, b as u32);
        }
        "H" => {
            let x = n(&mut v) as usize;
            wm.win_hide(x, n(&mut v) != 0);
        }
        "E" => {
            let x = n(&mut v) as usize;
            wm.win_enable(x, n(&mut v) != 0);
        }
        "F" => {
            let x = w(n(&mut v));
            wm.win_set_focus(h, x);
        }
        "C" => {
            let x = w(n(&mut v));
            if n(&mut v) != 0 {
                wm.win_capture(x);
            } else {
                wm.win_release(x);
            }
        }
        "O" => {
            let x = w(n(&mut v));
            if n(&mut v) != 0 {
                wm.win_set_modal(x);
            } else {
                wm.win_unset_modal(x);
            }
        }
        "Z" => {
            let x = w(n(&mut v));
            wm.win_destroy(x);
            wm.update(h);
        }
        // (#151) `TheGlobalLanguageData`: NULL, or language data with this `m_defaultWindowFont`
        "l" => {
            let has = n(&mut v) != 0;
            let (name, size, bold) = (n(&mut v), n(&mut v), n(&mut v) != 0);
            wm.default_window_font = has.then(|| FontDesc { name: LANGUAGE_FONTS[name as usize].as_bytes().to_vec(), size, bold });
        }
        "Y" => {
            let x = n(&mut v) as usize;
            let (a, b) = (n(&mut v), n(&mut v));
            wm.win_set_size(h, x, a, b);
        }
        "T" => {
            let x = n(&mut v) as usize;
            let t = units(v.remove(0));
            wm.win_set_text(x, &t);
        }
        // #72: the text entry, the combo box, the tab control
        "w" => {
            let ch = n(&mut v);
            let r = rts_lockstep_engine::client::ime_manager::service_ime_message(wm, h, rts_lockstep_engine::client::ime_manager::WM_CHAR, ch as u32, 0);
            let _ = writeln!(h.log, "ime {}", u8::from(r));
        }
        "I" => {
            let (x, ch) = (n(&mut v) as usize, n(&mut v));
            let r = wm.win_send_input_msg(h, Some(x), gwm::IME_CHAR, ch as u32, 0);
            let _ = writeln!(h.log, "ret {}", u8::from(r == MsgHandled::Handled));
        }
        "e" | "y" | "a" => {
            let x = n(&mut v) as usize;
            let t = units(v.remove(0));
            match c {
                "e" => tex::set_text(wm, h, Some(x), &t),
                "y" => cbx::set_text(wm, h, x, &t),
                _ => {
                    let color = n(&mut v);
                    let r = cbx::add_entry(wm, h, x, &t, color);
                    if wm.undefined.is_none() {
                        let _ = writeln!(h.log, "cadded {r}");
                    }
                }
            }
        }
        "g" | "c" => {
            let x = n(&mut v) as usize;
            let t = if c == "g" { tex::get_text(wm, h, Some(x)) } else { cbx::get_text(wm, h, x) };
            if wm.undefined.is_none() {
                h.log.push_str(if c == "g" { "etext" } else { "ctext" });
                text(&mut h.log, &t);
                h.log.push('\n');
            }
        }
        "r" => cbx::reset(wm, h, n(&mut v) as usize),
        "s" => {
            let x = n(&mut v) as usize;
            let (i, d) = (n(&mut v), n(&mut v));
            cbx::set_selected_pos(wm, h, x, i, d != 0);
        }
        "p" => {
            let x = n(&mut v) as usize;
            let r = cbx::get_selected_pos(wm, h, x, -7);
            if wm.undefined.is_none() {
                let _ = writeln!(h.log, "csel {r}");
            }
        }
        "h" => cbx::hide_list(wm, h, n(&mut v) as usize),
        "k" => {
            let x = n(&mut v) as usize;
            let (which, val) = (n(&mut v), n(&mut v));
            match which {
                0 => cbx::set_is_editable(wm, x, val != 0),
                1 => cbx::set_max_chars(wm, x, val),
                2 => cbx::set_max_display(wm, x, val),
                3 => cbx::set_ascii_only(wm, x, val != 0),
                _ => cbx::set_letters_and_numbers_only(wm, x, val != 0),
            }
        }
        "j" => {
            let x = n(&mut v) as usize;
            let (i, d) = (n(&mut v), n(&mut v));
            cbx::set_item_data(wm, h, x, i, d as u32);
        }
        "J" => {
            let x = n(&mut v) as usize;
            let r = cbx::get_item_data(wm, h, x, n(&mut v));
            if wm.undefined.is_none() {
                let _ = writeln!(h.log, "cdata {r}");
            }
        }
        "n" => {
            let x = n(&mut v) as usize;
            tbx::show_sub_pane(wm, x, n(&mut v));
        }
        "q" => tbx::create_sub_panes(wm, h, n(&mut v) as usize),
        "u" => tbx::fixup_sub_pane_list(wm, n(&mut v) as usize),
        _ => panic!("unknown op {op}"),
    }
}

/// The live windows of a kind.
fn live(wm: &GameWindowManager, f: impl Fn(&GameWindow, &UserData) -> bool) -> Vec<WinId> {
    (0..wm.tree.windows.len()).filter(|&i| !wm.ext[i].freed && wm.tree.windows[i].status & win_status::DESTROYED == 0 && f(&wm.tree.windows[i], &wm.ext[i].user)).collect()
}

const TEXTS: &[&str] = &["A", "B", "C", "AB", "BA", "ABC", "CAB", "_", "ABCABCABCABCABCABCABCABCABCABCABCABC", "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"];

/// The next operation for the current state.
fn gen_op(r: &mut Rng, wm: &GameWindowManager) -> String {
    let lbs = live(wm, |_, u| matches!(u, UserData::Listbox(_)));
    let sls = live(wm, |_, u| matches!(u, UserData::Slider(_)));
    let ens = live(wm, |_, u| matches!(u, UserData::Entry(_)));
    let cbs = live(wm, |_, u| matches!(u, UserData::Combo(_)));
    let tbs = live(wm, |_, u| matches!(u, UserData::Tab(_)));
    let all = live(wm, |_, _| true);
    let pick = |r: &mut Rng, v: &[WinId]| v[r.below(v.len() as u64) as usize];
    // a point in a window (or anywhere)
    let point = |r: &mut Rng| -> (i32, i32) {
        if !all.is_empty() && r.chance(9, 10) {
            let w = pick(r, &all);
            let (x, y) = wm.tree.screen_position(w);
            let (sx, sy) = wm.tree.windows[w].size;
            (x + r.range(-2, sx.max(0) + 2), y + r.range(-2, sy.max(0) + 2))
        } else {
            (r.range(0, 799), r.range(0, 599))
        }
    };
    let k = r.below(100);
    if k < 36 {
        let (x, y) = point(r);
        let m = match r.below(12) {
            0..=2 => gwm::MOUSE_POS,
            3..=4 => gwm::LEFT_DOWN,
            5..=6 => gwm::LEFT_UP,
            7 => gwm::LEFT_DRAG,
            8 => gwm::RIGHT_DOWN,
            9 => gwm::RIGHT_UP,
            10 => gwm::WHEEL_UP,
            _ => gwm::WHEEL_DOWN,
        };
        let (dx, dy) = if m == gwm::LEFT_DRAG { (r.range(-30, 30), r.range(-30, 30)) } else { (0, 0) };
        return format!("M {m} {x} {y} {dx} {dy}");
    }
    if k < 46 {
        let keys = [key::UP, key::DOWN, key::LEFT, key::RIGHT, key::TAB, key::ENTER, key::SPACE, b'A', b'B', b'C', b'Z', key::BACKSPACE, key::BACKSPACE, key::DEL, key::ESC, key::HOME, 0x3B];
        let kk = keys[r.below(keys.len() as u64) as usize];
        let state = match r.below(8) {
            0..=4 => key::STATE_DOWN,
            5..=6 => key::STATE_UP,
            _ => key::STATE_DOWN | 0x0004,
        };
        return format!("K {kk} {state}");
    }
    if k < 49 {
        return format!("t {}", wm.time_ms + r.below(800) as u32);
    }
    if k < 58 && !ens.is_empty() {
        let e = pick(r, &ens);
        let chars = [u32::from(b'a'), u32::from(b'Z'), u32::from(b'5'), u32::from(b' '), u32::from(b'-'), 13, 0xE9, 0x4E2D, 0xFF10, 0];
        // (#74) `w`: a typed character through the IME manager, to whichever entry it is attached to
        let typed = [u32::from(b'a'), u32::from(b'Q'), u32::from(b'7'), 32, 31, 13, 8, 9, 0xE9, 0x4E2D, 0x1F600 & 0xFFFF];
        return match r.below(10) {
            0..=3 => format!("I {e} {}", chars[r.below(chars.len() as u64) as usize]),
            4..=6 => format!("w {}", typed[r.below(typed.len() as u64) as usize]),
            7 => format!("e {e} {}", TEXTS[r.below(TEXTS.len() as u64) as usize]),
            _ => format!("g {e}"),
        };
    }
    if k < 70 && !cbs.is_empty() {
        let c = pick(r, &cbs);
        let n = combo_rt(wm, c).map_or(0, |d| d.entry_count);
        return match r.below(20) {
            0..=5 => format!("a {c} {} {}", TEXTS[r.below(8) as usize], r.range(0, 9)),
            6 => format!("r {c}"),
            7..=8 => format!("s {c} {} {}", r.range(-1, n), r.below(2)),
            9 => format!("p {c}"),
            10 => format!("c {c}"),
            11 => format!("y {c} {}", TEXTS[r.below(8) as usize]),
            12 => format!("h {c}"),
            13..=14 => {
                let which = r.below(5);
                let val = match which {
                    1 => r.range(-2, 300),
                    2 => r.range(0, 8),
                    _ => r.range(0, 1),
                };
                format!("k {c} {which} {val}")
            }
            15 => format!("j {c} {} {}", r.range(-1, n), r.range(0, 1000)),
            16 => format!("J {c} {}", r.range(-1, n)),
            // the drop-down button's GBM_SELECTED, through a click on it
            _ => {
                let Some(dd) = combo_rt(wm, c).and_then(|d| d.drop_down_button).filter(|&d| !wm.ext[d].freed) else { return format!("c {c}") };
                let (x, y) = wm.tree.screen_position(dd);
                format!("M {} {} {} 0 0", if r.chance(1, 2) { gwm::LEFT_DOWN } else { gwm::LEFT_UP }, x + 3, y + 3)
            }
        };
    }
    if k < 76 && !tbs.is_empty() {
        let t = pick(r, &tbs);
        let d = tab_rt(wm, t).expect("a tab control").clone();
        return match r.below(10) {
            0..=5 => {
                // a press on the tabs (or near them)
                let (x, y) = wm.tree.screen_position(t);
                format!("M {} {} {} 0 0", gwm::LEFT_DOWN, x + r.range(d.tabs_left_limit - 2, d.tabs_right_limit + 1), y + r.range(d.tabs_top_limit - 2, d.tabs_bottom_limit + 1))
            }
            6..=7 => format!("n {t} {}", r.range(-1, 8)),
            8 => format!("q {t}"),
            _ => format!("u {t}"),
        };
    }
    if k < 82 && !lbs.is_empty() {
        let l = pick(r, &lbs);
        let lr = listbox_rt(wm, l).expect("a list box");
        let end = i32::from(lr.end_pos);
        let cols = i32::from(lr.columns);
        return match r.below(20) {
            0..=8 => {
                let row = if r.chance(2, 3) { -1 } else { r.range(-1, end + 1) };
                let col = if r.chance(3, 4) { -1 } else { r.range(0, cols - 1) };
                format!("A {l} {} {} {row} {col} {}", TEXTS[r.below(TEXTS.len() as u64) as usize], r.range(0, 9), u8::from(r.chance(1, 4)))
            }
            9 => format!("R {l}"),
            10..=11 => {
                let k = if lr.multi_select { r.range(1, 3) } else { 1 };
                let mut s = format!("S {l} {k}");
                for _ in 0..k {
                    let _ = write!(s, " {}", r.range(-1, end));
                }
                s
            }
            12 => format!("L {l} {}", r.range(1, 14)),
            13 => format!("D {l} {}", r.range(0, end)),
            14 => format!("B {l} {}", r.range(0, end.min(2))),
            15 => format!("U {l} {}", r.range(0, end)),
            16 => format!("X {l} {}", r.range(-1, end - 1)),
            17 => format!("G {l} {} {}", r.range(-1, end - 1), r.range(0, cols - 1)),
            _ if end > 0 || r.chance(1, 10) => format!("V {l} {} {}", r.below(2), r.range(0, end)),
            _ => format!("G {l} -1 0"),
        };
    }
    if k < 85 && !sls.is_empty() {
        let s = pick(r, &sls);
        return if r.chance(3, 4) { format!("P {s} {}", r.range(-5, 110)) } else { format!("Q {s} {} {}", r.range(-10, 20), r.range(-10, 120)) };
    }
    if all.is_empty() {
        return "t 0".to_string();
    }
    let w = pick(r, &all);
    match r.below(10) {
        0 => format!("H {w} {}", u8::from(r.chance(1, 3))),
        1 => format!("E {w} {}", u8::from(r.chance(2, 3))),
        2..=3 => format!("F {}", if r.chance(1, 5) { -1 } else { w as i64 }),
        4 => format!("C {w} {}", u8::from(r.chance(1, 2))),
        5 if wm.tree.windows[w].parent.is_none() => format!("O {w} {}", u8::from(r.chance(1, 2))),
        6 if r.chance(1, 4) => format!("Z {w}"),
        7 => {
            let (sx, sy) = wm.tree.windows[w].size;
            format!("Y {w} {} {}", (sx + r.range(-20, 20)).max(1), (sy + r.range(-20, 20)).max(1))
        }
        _ => format!("M {} {} {} 0 0", gwm::MOUSE_POS, r.range(0, 799), r.range(0, 599)),
    }
}

/// The windows of a case.
fn gen_tree(r: &mut Rng) -> Vec<String> {
    let mut ops = Vec::new();
    let roots = 1 + r.below(2) as i32;
    let mut parents = Vec::new();
    let mut next = 0i32;
    for i in 0..roots {
        let (x, y) = if i == 0 { (0, 0) } else { (r.range(200, 400), r.range(100, 300)) };
        ops.push(format!("W -1 0 {x} {y} 400 300 {} {} 0", win_status::ENABLED, gws::USER_WINDOW));
        parents.push(next);
        next += 1;
    }
    for _ in 0..1 + r.below(4) {
        let p = parents[r.below(parents.len() as u64) as usize];
        let mut status = win_status::ENABLED;
        if r.chance(1, 5) {
            status |= win_status::IMAGE;
        }
        if r.chance(1, 20) {
            status |= win_status::HIDDEN;
        }
        let track = if r.chance(1, 2) { gws::MOUSE_TRACK } else { 0 };
        let (x, y) = (r.range(0, 250), r.range(0, 200));
        let kind = r.below(17);
        let visual = u8::from(r.chance(1, 3));
        let (op, made) = match kind {
            0..=4 => {
                if r.chance(1, 4) {
                    status |= win_status::ONE_LINE;
                }
                let cols = if r.chance(2, 3) { 1 } else { r.range(2, 3) };
                let mut s = format!("W {p} 6 {x} {y} {} {} {status} {} {visual} {} {cols}", r.range(60, 250), r.range(30, 200), gws::SCROLL_LISTBOX | track, if r.chance(1, 20) { 0 } else { r.range(1, 12) });
                for f in [3, 3, 2, 4, 3, 8] {
                    let _ = write!(s, " {}", u8::from(r.chance(1, f)));
                }
                for _ in 0..if cols > 1 { cols } else { 0 } {
                    let _ = write!(s, " {}", r.range(10, 70));
                }
                let made = if s.split(' ').nth(14) == Some("1") { 5 } else { 1 };
                (s, made)
            }
            5 => (format!("W {p} 7 {x} {y} {} {} {status} {} {visual} {} {}", r.range(10, 30), r.range(30, 200), gws::VERT_SLIDER | track, r.range(-10, 20), r.range(-10, 120)), 2),
            6 => (format!("W {p} 8 {x} {y} {} {} {status} {} {visual} {} {}", r.range(30, 250), r.range(10, 30), gws::HORZ_SLIDER | track, r.range(-10, 20), r.range(-10, 120)), 2),
            7 => (format!("W {p} 1 {x} {y} {} {} {status} {} {visual}", r.range(20, 100), r.range(10, 40), gws::PUSH_BUTTON | track), 1),
            8 => (format!("W {p} 2 {x} {y} {} {} {status} {} {visual}", r.range(20, 100), r.range(10, 40), gws::CHECK_BOX | track), 1),
            9 => (format!("W {p} 3 {x} {y} {} {} {status} {} {visual} {}", r.range(20, 100), r.range(10, 40), gws::RADIO_BUTTON | track, r.range(0, 2)), 1),
            10..=11 => {
                // a combo box: editable, maxDisplay, maxChars, ascii, letters and numbers, the list's length and scroll bar,
                // the font's size (0: none given)
                let len = if r.chance(1, 20) { 0 } else { r.range(1, 12) };
                let sb = u8::from(r.chance(4, 5));
                let fs = if r.chance(1, 2) { 0 } else { r.range(8, 16) };
                let op = format!("W {p} 4 {x} {y} {} {} {status} {} {visual} {} {} {} {} {} {len} {sb} {fs}", r.range(40, 200), r.range(14, 30), gws::COMBO_BOX | track, u8::from(r.chance(1, 2)), r.range(0, 6), r.range(-2, 300), u8::from(r.chance(1, 4)), u8::from(r.chance(1, 4)));
                (op, 4 + if sb == 1 { 4 } else { 0 })
            }
            12 => {
                let count = if r.chance(1, 20) { r.range(0, 9) } else { r.range(1, 8) };
                let w = if r.chance(1, 20) { 0 } else { r.range(10, 40) };
                let op = format!("W {p} 5 {x} {y} {} {} {status} {} {visual} {} {} {w} {} {count} {} {}", r.range(100, 300), r.range(60, 200), gws::TAB_CONTROL | track, r.range(0, 2), if r.chance(1, 10) { r.range(0, 2) } else { r.range(3, 6) }, r.range(10, 30), r.range(0, 8), r.below(256));
                (op, 9)
            }
            13..=14 => {
                let label = ["_", "Ab1", "xyz"][r.below(3) as usize];
                let op = format!("W {p} 9 {x} {y} {} {} {status} {} {visual} {} {} {} {} {} {label}", r.range(10, 150), r.range(10, 30), gws::ENTRY_FIELD | track, r.range(-1, 12) + if r.chance(1, 10) { 290 } else { 0 }, u8::from(r.chance(1, 4)), u8::from(r.chance(1, 5)), u8::from(r.chance(1, 5)), u8::from(r.chance(1, 4)));
                (op, 1)
            }
            15 => (format!("W {p} 10 {x} {y} {} {} {status} {} {visual}", r.range(20, 100), r.range(10, 40), gws::PROGRESS_BAR | track), 1),
            _ => (format!("W {p} 11 {x} {y} {} {} {status} {} {visual}", r.range(20, 100), r.range(10, 40), gws::STATIC_TEXT | track), 1),
        };
        ops.push(op);
        if kind <= 4 && r.chance(1, 4) {
            ops.push(format!("T {next} {}", TEXTS[r.below(6) as usize]));
        }
        next += made;
    }
    ops
}

struct Case {
    ops: Vec<String>,
    dumps: Vec<String>,
    undefined: Option<&'static str>,
    /// The operation that met it.
    undefined_op: String,
}

/// (#151) The `m_defaultWindowFont` names an `l` operation gives (wrap_wm.cpp's `NAMES`; 0 the empty name, which takes
/// upstream's fallback as NULL does).
const LANGUAGE_FONTS: [&str; 5] = ["", "Arial Unicode MS", "Arial", "Courier New", "Times New Roman"];

/// (#151) The case's first operation, `TheGlobalLanguageData`, from its own stream (`case`'s), so that the operations
/// after it are #74's: NULL in a quarter of the cases (the NULL case kept), else language data whose
/// `DefaultWindowFont` is one of [`LANGUAGE_FONTS`] (the empty name one time in five), size 6 to 20, bold or not.
fn gen_language(case: u64) -> String {
    let mut r = Rng(case.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x151);
    r.next();
    if r.chance(1, 4) {
        return "l 0 0 0 0".into();
    }
    let name = if r.chance(1, 5) { 0 } else { 1 + r.below(4) };
    format!("l 1 {name} {} {}", r.range(6, 20), r.below(2))
}

fn run_rust(r: &mut Rng, len: u64, case: u64) -> Case {
    let mut wm = GameWindowManager::new((800, 600));
    let mut h = Host::default();
    let mut ops = vec![gen_language(case)];
    ops.extend(gen_tree(r));
    let mut dumps = Vec::new();
    let mut i = 0;
    let total = ops.len() as u64 + len;
    while (i as u64) < total {
        if i >= ops.len() {
            let op = gen_op(r, &wm);
            ops.push(op);
        }
        let before = wm.tree.windows.len();
        run_op(&mut wm, &mut h, &ops[i]);
        let dd_from = if ops[i].starts_with("W ") { before } else { usize::MAX };
        if let Some(u) = wm.undefined {
            let at = ops[i].split(' ').next().unwrap_or("").to_string();
            ops.truncate(i);
            return Case { ops, dumps, undefined: Some(u), undefined_op: at };
        }
        dumps.push(format!("#{i}\n{}{}", std::mem::take(&mut h.log), dump(&wm, dd_from)));
        i += 1;
    }
    Case { ops, dumps, undefined: None, undefined_op: String::new() }
}

fn run_cpp(ops: &[String]) -> (String, Option<i32>) {
    let mut child = std::process::Command::new(env!("WM_REF_EXE")).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().expect("run wm_ref");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        for op in ops {
            let _ = writeln!(stdin, "{op}");
        }
    }
    // a case upstream never finishes (a loop the Rust's run cut, or a planted bug's different case) is stopped
    let mut stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut v);
        v
    });
    let start = std::time::Instant::now();
    let code = loop {
        if let Some(st) = child.try_wait().expect("wait wm_ref") {
            break st.code();
        }
        if start.elapsed() > std::time::Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    };
    (String::from_utf8_lossy(&reader.join().expect("reader")).into_owned(), code)
}

#[test]
fn window_manager_and_gadgets_match_upstream() {
    let cases: u64 = std::env::var("WM_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(2000);
    let mut r = Rng(0x2545_F491_4F6C_DD1D);
    let (mut ops_compared, mut equal_cases, mut mismatched_cases, mut crashed) = (0u64, 0u64, 0u64, 0u64);
    let mut undefined: BTreeMap<String, u64> = BTreeMap::new();
    // what was exercised: the operations compared, by kind, and the gadgets made (`W` by type), over the compared ones
    let mut by_op: BTreeMap<String, u64> = BTreeMap::new();
    let mut first_bad: Option<String> = None;
    for case in 0..cases {
        let len = 10 + r.below(61);
        let c = run_rust(&mut r, len, case);
        if let Some(u) = c.undefined {
            *undefined.entry(format!("{u} (at `{}`)", c.undefined_op)).or_default() += 1;
        }
        let (out, code) = run_cpp(&c.ops);
        let mut cpp: Vec<String> = Vec::new();
        for part in out.split_inclusive('\n') {
            if part.starts_with('#') {
                cpp.push(String::new());
            }
            if let Some(last) = cpp.last_mut() {
                last.push_str(part);
            }
        }
        let mut bad = None;
        for (i, d) in c.dumps.iter().enumerate() {
            match cpp.get(i) {
                Some(x) if x == d => {
                    ops_compared += 1;
                    let mut t = c.ops[i].split(' ');
                    let k = t.next().unwrap_or("").to_string();
                    let k = if k == "W" { format!("W{}", t.nth(1).unwrap_or("")) } else { k };
                    *by_op.entry(k).or_default() += 1;
                }
                Some(x) => {
                    bad = Some(format!("case {case}, op {i} `{}`:\n--- rust\n{d}--- c++\n{x}", c.ops[i]));
                    break;
                }
                None => {
                    crashed += 1;
                    bad = Some(format!("case {case}, op {i} `{}`: the C++ stopped (exit {code:?}); rust:\n{d}", c.ops[i]));
                    break;
                }
            }
        }
        match bad {
            None => equal_cases += 1,
            Some(b) => {
                mismatched_cases += 1;
                if first_bad.is_none() {
                    let ops: String = c.ops.iter().map(|o| format!("{o}\n")).collect();
                    first_bad = Some(format!("{b}\nops:\n{ops}"));
                }
            }
        }
    }
    eprintln!("wm differential: {cases} cases, {ops_compared} operations compared, {equal_cases} cases identical, {mismatched_cases} with a mismatch ({crashed} where the C++ stopped)");
    eprintln!("  compared, by operation (W by gadget type): {by_op:?}");
    for (u, n) in &undefined {
        eprintln!("  stopped where upstream is undefined: {n} x {u}");
    }
    if let Some(b) = &first_bad {
        eprintln!("first mismatch: {b}");
    }
    assert_eq!(mismatched_cases, 0, "{}", first_bad.unwrap_or_default());
}
