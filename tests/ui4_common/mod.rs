// SPDX-License-Identifier: GPL-3.0-only
//! Shared pieces of the ui4 differential tests (#215): the scene of named gadgets the menus run on, its operations (the same text lines
//! differential/wrap_draft_ui4.cpp reads), the dump of the windows (tests/differential_wm.rs's, copied: wrap_wm.cpp's format) and the run of the
//! reference program `ui4_ref` (upstream's own window manager and gadgets, see differential/wrap_draft_ui4.cpp).
#![allow(dead_code)]

use rts_lockstep_engine::client::gadget_list_box::{CellData, ListboxRt, LISTBOX_IMAGE, LISTBOX_TEXT};
use rts_lockstep_engine::client::gadget_text_entry::EntryRt;
use rts_lockstep_engine::client::gadget_text_entry as tex;
use rts_lockstep_engine::client::window_manager::*;
use rts_lockstep_engine::client::window_script::*;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;

pub fn text(o: &mut String, t: &[u16]) {
    o.push_str(" [");
    for (i, c) in t.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let _ = write!(o, "{c}");
    }
    o.push(']');
}

pub fn idx(w: Option<WinId>) -> i64 {
    w.map_or(-1, |w| w as i64)
}

pub fn dump(wm: &GameWindowManager) -> String {
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

pub fn dump_listbox(o: &mut String, l: &ListboxRt) {
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

pub fn units(t: &str) -> Vec<u16> {
    t.bytes().filter(|&b| b != b'_').map(u16::from).collect()
}

/// One operation on the Rust side (wrap_wm.cpp's `run_op`).

// ---------------------------------------------------------------------------------------------------------------------
// The scene's operations, the C++ run and the comparison
// ---------------------------------------------------------------------------------------------------------------------

/// A host that records what the stand-ins record (differential/wrap_draft_ui4.cpp's `g_log`).
pub trait HasLog {
    fn log(&mut self) -> &mut String;
}

/// `NameKeyGenerator::nameToKey`: keys 1, 2, ... in the order first asked (wrap_wm.cpp's).
#[derive(Default, Clone, Debug)]
pub struct KeyTable {
    pub map: BTreeMap<Vec<u8>, u32>,
}
impl KeyTable {
    pub fn key(&mut self, name: &[u8]) -> u32 {
        let n = self.map.len() as u32 + 1;
        *self.map.entry(name.to_vec()).or_insert(n)
    }
}

/// A text as the C++ reads it: its UTF-16 units in hex, comma separated; `-` for the empty text.
pub fn hex(t: &[u16]) -> String {
    if t.is_empty() {
        return "-".into();
    }
    t.iter().map(|c| format!("{c:x}")).collect::<Vec<_>>().join(",")
}

/// The operations every ui4 scene shares (wrap_wm.cpp's `W` for the gadgets, `M`, `K`, `H`, `E`, `F`, `T`, `t`, `e`, `I`, and the `@ nm` and `@ wu`
/// of wrap_draft_ui4.cpp). `menu_funcs` are the system / input callback names `@ wu`'s menu number picks. False when `op` is none of them.
pub fn scene_op<H: GuiHost + HasLog>(wm: &mut GameWindowManager, h: &mut H, keys: &mut KeyTable, menu_funcs: &[(&[u8], &[u8])], op: &str) -> bool {
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
                1 | 10 => {
                    wm.gogo_gadget_look(h, parent, status, x, y, ww, hh, inst, GadgetData::None, visual);
                }
                11 => {
                    wm.gogo_gadget_look(h, parent, status, x, y, ww, hh, inst, GadgetData::Text(TextData::default()), visual);
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
                _ => panic!("scene_op: gadget type {ty}"),
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
        "T" => {
            let x = n(&mut v) as usize;
            let t = units(v.remove(0));
            wm.win_set_text(x, &t);
        }
        "e" => {
            let x = n(&mut v) as usize;
            let t = units(v.remove(0));
            tex::set_text(wm, h, Some(x), &t);
        }
        "I" => {
            let (x, ch) = (n(&mut v) as usize, n(&mut v));
            let r = wm.win_send_input_msg(h, Some(x), gwm::IME_CHAR, ch as u32, 0);
            let _ = writeln!(h.log(), "ret {}", u8::from(r == MsgHandled::Handled));
        }
        "@" => match v[0] {
            "nm" => {
                let k = keys.key(v[2].as_bytes());
                wm.tree.windows[v[1].parse::<usize>().expect("window")].id = k as i32;
            }
            // (the layout's members: the Rust host's `create_layout` has them)
            "lay" => {}
            "tx" => {
                let t: Vec<u16> = if v[2] == "-" { Vec::new() } else { v[2].split(',').map(|x| u16::from_str_radix(x, 16).expect("hex")).collect() };
                tex::set_text(wm, h, Some(v[1].parse::<usize>().expect("window")), &t);
            }
            "wu" => {
                let mut a = v[1..].iter();
                let mut num = || -> i32 { a.next().expect("arg").parse().expect("number") };
                let (parent, menu, x, y, ww, hh) = (num(), num(), num(), num(), num(), num());
                let (status, style) = (num() as u32, num() as u32);
                let inst = WinInstanceData { style, ..WinInstanceData::new() };
                let parent = w(parent);
                let (sys, inp) = menu_funcs[menu as usize];
                let u = wm.win_create(h, parent, status, x, y, ww, hh, SystemFunc::Host(sys.to_vec()), Some(inst));
                wm.ext[u].input = InputFunc::Host(inp.to_vec());
                wm.ext[u].owner = parent;
            }
            _ => return false,
        },
        _ => return false,
    }
    true
}

/// The reference program `exe`'s output for `ops`, split into one chunk per operation (`#<n>` first), and its exit code.
pub fn run_cpp(exe: &str, ops: &[String]) -> (Vec<String>, Option<i32>) {
    let mut child = std::process::Command::new(exe).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().expect("run ui4_ref");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        for op in ops {
            let _ = writeln!(stdin, "{op}");
        }
    }
    let mut stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut v);
        v
    });
    let start = std::time::Instant::now();
    let code = loop {
        if let Some(st) = child.try_wait().expect("wait ui4_ref") {
            break st.code();
        }
        if start.elapsed() > std::time::Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    };
    let out = String::from_utf8_lossy(&reader.join().expect("reader")).into_owned();
    let mut cpp: Vec<String> = Vec::new();
    for part in out.split_inclusive('\n') {
        if part.starts_with('#') {
            cpp.push(String::new());
        }
        if let Some(last) = cpp.last_mut() {
            last.push_str(part);
        }
    }
    (cpp, code)
}

/// Compares the Rust chunks with the C++'s, one `Check` comparison per operation; the first mismatch's operations are in the message.
pub fn compare_case(exe: &str, chk: &mut super::support::Check, case: u64, ops: &[String], rust: &[String]) {
    let (cpp, code) = run_cpp(exe, ops);
    for (i, d) in rust.iter().enumerate() {
        let x = cpp.get(i).cloned().unwrap_or_else(|| format!("(the C++ stopped, exit {code:?})"));
        let same = *d == x;
        chk.eq(d.clone(), x, || format!("case {case}, op {i} `{}`\nops:\n{}", ops[i], ops.join("\n")));
        if !same {
            static SHOWN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            if SHOWN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 3 && !planted() {
                let cpp_chunk = cpp.get(i).cloned().unwrap_or_default();
                let (rl, cl): (Vec<&str>, Vec<&str>) = (d.lines().collect(), cpp_chunk.lines().collect());
                let at = (0..rl.len().max(cl.len())).find(|&k| rl.get(k) != cl.get(k)).unwrap_or(0);
                eprintln!("MISMATCH case {case} op {i} `{}`, first differing line {at}:\n  rust: {}\n  c++:  {}", ops[i], rl.get(at).unwrap_or(&"(none)"), cl.get(at).unwrap_or(&"(none)"));
            }
            break; // later operations differ in their consequences: one mismatch per case
        }
    }
}

pub fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

pub fn cases(default: usize) -> usize {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Ends a test: a planted run prints the `MEASURED` line (the mismatch count N is the plant caught) and fails on purpose; a normal run prints the `DIFF` line
/// and fails on any mismatch.
pub fn finish(c: super::support::Check) {
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}

/// A layout holding `members` (head first); returns its index (the C++ `winCreateLayout` stand-in adds them in reverse the same way).
pub fn make_layout(wm: &mut GameWindowManager, members: &[usize]) -> usize {
    let l = wm.layouts.len();
    wm.layouts.push(WindowLayout::default());
    for &m in members.iter().rev() {
        wm.layout_add_window(l, m);
    }
    l
}

/// The layouts in wrap_draft_ui4.cpp's dump format.
pub fn dump_layouts(wm: &GameWindowManager) -> String {
    let mut o = String::new();
    for (i, l) in wm.layouts.iter().enumerate() {
        let _ = write!(o, "lay{i} hidden {} wins", u8::from(l.hidden));
        for w in &l.windows {
            let _ = write!(o, " {w}");
        }
        o.push('\n');
    }
    o
}

/// Every static text's text (`GadgetStaticTextGetText`; the window manager's dump does not carry it), in wrap_draft_ui4.cpp's format.
pub fn dump_static_texts(wm: &GameWindowManager) -> String {
    let mut o = String::new();
    for (i, w) in wm.tree.windows.iter().enumerate() {
        if !wm.ext[i].freed && w.inst.style & gws::STATIC_TEXT != 0 {
            let _ = write!(o, "stt{i}");
            text(&mut o, &rts_lockstep_engine::client::gadget::static_text_get_text(wm, i));
            o.push('\n');
        }
    }
    o
}
