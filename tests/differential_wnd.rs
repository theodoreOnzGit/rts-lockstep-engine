// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of the WND layout reader (#12, `client::window_script`) against upstream's own
//! `GameWindowManagerScript.cpp` (commit 0a05454d), cut verbatim by `build.rs` and driven by
//! `differential/wrap_wnd.cpp` (which lists every stand-in).
//!
//! No game files are used: every layout is **synthetic**. Each case is one to three files read in order with the same
//! statics (the static buffer, the gadget parts' draw data and the gadget data statics persist across files, as in
//! the game), at one of five display sizes. A file is version 1 (no layout block) or 2 (a layout block, sometimes
//! without `ENDLAYOUTBLOCK` or with an unknown entry, sometimes missing: the read fails), default colours and fonts
//! between windows, and up to four top-level windows nested up to three deep, of every type upstream creates
//! (and, rarely, `MOUSETRACK`, which upstream has no branch for). Each window gets a random subset of the fields in a
//! random order: the name, status and style lists (with unknown, lower-case and `NULL` entries), the four callbacks
//! (resolved or `[None]`), a font, a header template, tooltip text (sometimes `""`), a tooltip delay, a text label
//! (sometimes 128 bytes or more), the text colours, the three states' draw data and the parts' draw data (images
//! `Img*`, `NoImage` or unknown), an image offset, `TOOLTIP`, its gadget data (sometimes another kind's), the obsolete
//! `DATA`, an unknown field, a second `CHILD`, fields after the children. One file in eight is then damaged: cut short,
//! a `;` removed, a value replaced by a word or a huge number, or random bytes put in.
//!
//! The Rust runs first, on a fresh [`WindowScript`] per case; the C++ in a forked child (fresh statics). Compared: if
//! the Rust read every file, the C++ must have too, and the dumps (every window: parent, status, style, id, region,
//! size, names, labels, delay, offset, font, text, tooltip, the callbacks, the gadget data, the 27 draw data, the text
//! colours and the parts' draw data; and per file the first window, the layout names, what resolved and the window
//! list) must be **identical**. If the Rust stopped where the C++ throws (a NULL token, a number out of range, a
//! `strcpy` overrun, the end of the file, a hang, a failed child block), the C++ must have stopped. The other
//! undefined cases (no quote in the buffer, an unknown window type, gadget data of another kind, an uninitialised
//! value, a position out of `Int`, more than 8 tab panes) are counted and not compared.
//!
//! # Predictions (written before the first run)
//!
//! 0 mismatches. If anything differs, most likely the stale-buffer quirks (a quote found past the NULs) and `strtok`'s
//! state after an empty token; then the `TEXTCOLOR`/draw data colour carry-over between failed scans.
//!
//! # Results (2026-11-05, `cargo test --release --features differential --test differential_wnd`)
//!
//! - **First run: 23 mismatches**, every one the C++ crashing where the Rust went on: the obsolete `DATA` field on a
//!   tab control, a combo box (or another gadget that reads its data) leaves the data pointer NULL, which the gadget
//!   then dereferences. Undefined upstream; the Rust now reports it (`GadgetDataMismatch`).
//! - Then 3,000 cases (the default): 1,782 files read and 6,012 windows compared in 1,072 identical cases, 1,425 stops
//!   agreed, 503 undefined cases not compared (`NoQuote` 106, `GadgetDataMismatch` 184, `UninitialisedRead` 62,
//!   `TabPaneOverrun` 44, `UnknownWindowType` 107): **0 mismatches**, as predicted.
//! - One planted bug (the `STATUS`/`STYLE` names compared with case in `parse_bit_string`): caught, 839 cases differ.

#![cfg(feature = "differential")]

use rts_lockstep_engine::client::window_script::*;
use rts_lockstep_engine::differential as cpp;
use std::collections::BTreeMap;
use std::fmt::Write as _;

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
    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[self.below(xs.len() as u64) as usize]
    }
}

/// The C++ stand-ins' rules (wrap_wnd.cpp).
#[derive(Default)]
struct Host {
    keys: BTreeMap<Vec<u8>, u32>,
    display: (i32, i32),
}

impl WndHost for Host {
    fn name_to_key(&mut self, name: &[u8]) -> u32 {
        let n = self.keys.len() as u32 + 1;
        *self.keys.entry(name.to_vec()).or_insert(n)
    }
    fn find_image(&self, name: &[u8]) -> Option<Vec<u8>> {
        (name.len() >= 3 && name[..3].eq_ignore_ascii_case(b"img")).then(|| [b"Img".as_slice(), &name[3..]].concat())
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        [b"T:".as_slice(), label].concat().iter().map(|&b| u16::from(b)).collect()
    }
    fn font_from_template(&self, name: &[u8]) -> Option<GameFont> {
        (name == b"HT1").then(|| GameFont { name: b"HTFont".to_vec(), point_size: 12, bold: true })
    }
    fn display_size(&self) -> (i32, i32) {
        self.display
    }
}

const TYPES: &[&str] = &[
    "USER", "PUSHBUTTON", "RADIOBUTTON", "CHECKBOX", "VERTSLIDER", "HORZSLIDER", "SCROLLLISTBOX", "COMBOBOX", "ENTRYFIELD", "STATICTEXT",
    "PROGRESSBAR", "TABCONTROL", "TABPANE",
];

fn num(r: &mut Rng) -> String {
    match r.below(150) {
        0..=9 => "-5".into(),
        10 => "99999999999".into(),
        11 => "x".into(),
        _ => r.below(300).to_string(),
    }
}

fn rgba(r: &mut Rng) -> String {
    format!("{} {} {} {}", r.below(256), r.below(256), r.below(256), r.below(256))
}

fn draw_data(r: &mut Rng) -> String {
    let mut s = String::new();
    for i in 0..9 {
        let img = match r.below(4) {
            0 => "NoImage".to_string(),
            1 => "Other".to_string(),
            _ => format!("{}{}", r.pick(&["Img", "img", "IMG"]), r.below(5)),
        };
        if i > 0 {
            s.push_str(", ");
        }
        let _ = write!(s, "IMAGE: {img}, COLOR: {}, BORDERCOLOR: {}", rgba(r), rgba(r));
    }
    if r.chance(1, 150) {
        // cut short: a missing value
        s.truncate(s.len() / 2);
    }
    s
}

fn bits(r: &mut Rng, names: &[&str]) -> String {
    if r.chance(1, 15) {
        return "NULL".into();
    }
    let n = 1 + r.below(4);
    let mut v = Vec::new();
    for _ in 0..n {
        let s = match r.below(8) {
            0 => "bogus".to_string(),
            1 => r.pick(names).to_lowercase(),
            _ => r.pick(names).to_string(),
        };
        v.push(s);
    }
    v.join("+")
}

fn gadget_data(r: &mut Rng, ty: &str) -> String {
    let kind = if r.chance(1, 60) { r.pick(TYPES) } else { ty };
    match kind {
        "SCROLLLISTBOX" => {
            let cols = r.below(4);
            let mut s = format!("LISTBOXDATA = LENGTH: {}, AUTOSCROLL: {}, ", num(r), r.below(2));
            if r.chance(1, 2) {
                let _ = write!(s, "ScrollIfAtEnd: {}, ", r.below(2));
            }
            let _ = write!(s, "AUTOPURGE: {}, SCROLLBAR: {}, MULTISELECT: {}, COLUMNS: {cols}, ", r.below(2), r.below(2), r.below(2));
            if cols > 1 {
                for _ in 0..cols {
                    let _ = write!(s, "COLUMNSWIDTH%: {}, ", num(r));
                }
            }
            let _ = write!(s, "FORCESELECT: {};", r.below(2));
            s
        }
        "COMBOBOX" => format!(
            "COMBOBOXDATA = ISEDITABLE: {}, MAXCHARS: {}, MAXDISPLAY: {}, ASCIIONLY: {}, LETTERSANDNUMBERS: {};",
            r.below(2),
            num(r),
            num(r),
            r.below(2),
            r.below(2)
        ),
        "VERTSLIDER" | "HORZSLIDER" => format!("SLIDERDATA = MINVALUE: {}, MAXVALUE: {};", num(r), num(r)),
        "RADIOBUTTON" => format!("RADIOBUTTONDATA = GROUP: {};", num(r)),
        "STATICTEXT" => format!("STATICTEXTDATA = CENTERED: {};", r.below(3)),
        "ENTRYFIELD" => format!(
            "TEXTENTRYDATA = MAXLEN: {}, SECRETTEXT: {}, NUMERICALONLY: {}, ALPHANUMERICALONLY: {}, ASCIIONLY: {};",
            num(r),
            r.below(2),
            r.below(2),
            r.below(2),
            r.below(2)
        ),
        "TABCONTROL" => {
            let panes = if r.chance(1, 20) { 9 + r.below(3) } else { r.below(9) };
            let mut s = format!(
                "TABCONTROLDATA = TABORIENTATION: {}, TABEDGE: {}, TABWIDTH: {}, TABHEIGHT: {}, TABCOUNT: {}, PANEBORDER: {}, PANEDISABLED: {panes}",
                r.below(3),
                r.below(4),
                num(r),
                num(r),
                r.below(8),
                r.below(3)
            );
            for _ in 0..panes {
                let _ = write!(s, ", {}", r.below(2));
            }
            s.push(';');
            s
        }
        _ => String::new(),
    }
}

fn window(r: &mut Rng, depth: u32, out: &mut String, file: &str, counter: &mut u32) {
    let ty = if r.chance(1, 150) { "MOUSETRACK" } else { r.pick(TYPES) };
    let ind = "  ".repeat(depth as usize);
    let _ = writeln!(out, "{ind}WINDOW");
    let _ = writeln!(out, "{ind}  WINDOWTYPE = {ty};");
    let res = if r.chance(1, 4) { (1024, 768) } else { (800, 600) };
    let (x, y) = (r.below(800) as i64 - 50, r.below(600) as i64 - 50);
    let (x2, y2) = (x + r.below(300) as i64 - 20, y + r.below(200) as i64 - 20);
    let _ = writeln!(out, "{ind}  SCREENRECT = UPPERLEFT: {x} {y}, BOTTOMRIGHT: {x2} {y2}, CREATIONRESOLUTION: {} {};", res.0, res.1);
    *counter += 1;
    let name = format!("{file}:Win{counter}");
    let mut fields: Vec<String> = Vec::new();
    let quote = |r: &mut Rng, s: &str| if r.chance(1, 400) { s.to_string() } else { format!("\"{s}\"") };
    fields.push(format!("NAME = {};", quote(r, &name)));
    fields.push(format!("STATUS = {};", bits(r, WINDOW_STATUS_NAMES)));
    fields.push(format!("STYLE = {};", bits(r, WINDOW_STYLE_NAMES)));
    for cb in ["SYSTEMCALLBACK", "INPUTCALLBACK", "TOOLTIPCALLBACK", "DRAWCALLBACK"] {
        if r.chance(1, 2) {
            let v = if r.chance(1, 3) { "[None]".to_string() } else { format!("Cb{}", r.below(5)) };
            fields.push(format!("{cb} = {};", quote(r, &v)));
        }
    }
    if r.chance(1, 2) {
        fields.push(format!("FONT = NAME: \"{}\", SIZE: {}, BOLD: {};", r.pick(&["Arial", "Times New Roman", "Courier"]), num(r), r.below(2)));
    }
    if r.chance(1, 3) {
        fields.push(format!("HEADERTEMPLATE = \"{}\";", r.pick(&["HT1", "[NONE]", "Other"])));
    }
    if r.chance(1, 2) {
        let t = match r.below(6) {
            0 => String::new(),
            1 => "L".repeat(120 + r.below(20) as usize),
            _ => format!("TOOLTIP:Tip{}", r.below(9)),
        };
        fields.push(format!("TOOLTIPTEXT = \"{t}\";"));
    }
    if r.chance(1, 3) {
        fields.push(format!("TOOLTIPDELAY = {};", num(r)));
    }
    if r.chance(2, 3) {
        let t = match r.below(8) {
            0 => "L".repeat(125 + r.below(10) as usize),
            1 => "with space and Ü".to_string(),
            _ => format!("GUI:Label{}", r.below(9)),
        };
        fields.push(format!("TEXT = \"{t}\";"));
    }
    if r.chance(1, 2) {
        fields.push(format!(
            "TEXTCOLOR = ENABLED: {}, ENABLEDBORDER: {}, DISABLED: {}, DISABLEDBORDER: {}, HILITE: {}, HILITEBORDER: {};",
            rgba(r),
            rgba(r),
            rgba(r),
            rgba(r),
            rgba(r),
            rgba(r)
        ));
    }
    for dd in ["ENABLEDDRAWDATA", "DISABLEDDRAWDATA", "HILITEDRAWDATA"] {
        if r.chance(2, 3) {
            fields.push(format!("{dd} = {};", draw_data(r)));
        }
    }
    for dd in [
        "LISTBOXENABLEDUPBUTTONDRAWDATA",
        "LISTBOXDISABLEDDOWNBUTTONDRAWDATA",
        "LISTBOXHILITESLIDERDRAWDATA",
        "SLIDERTHUMBENABLEDDRAWDATA",
        "COMBOBOXDROPDOWNBUTTONENABLEDDRAWDATA",
        "COMBOBOXEDITBOXHILITEDRAWDATA",
        "COMBOBOXLISTBOXDISABLEDDRAWDATA",
    ] {
        if r.chance(1, 6) {
            fields.push(format!("{dd} = {};", draw_data(r)));
        }
    }
    if r.chance(1, 3) {
        fields.push(format!("IMAGEOFFSET = {} {};", num(r), num(r)));
    }
    if r.chance(1, 10) {
        fields.push("TOOLTIP = something;".into());
    }
    if r.chance(3, 4) {
        let g = gadget_data(r, ty);
        if !g.is_empty() {
            fields.push(g);
        }
    }
    if r.chance(1, 40) {
        fields.push(format!("DATA = {} {} {} {} {} {};", r.below(3), r.below(3), r.below(3), r.below(3), r.below(3), r.below(3)));
    }
    if r.chance(1, 8) {
        fields.push("UNKNOWNFIELD = whatever 1 2;".into());
    }
    // shuffle
    for i in (1..fields.len()).rev() {
        let j = r.below(i as u64 + 1) as usize;
        fields.swap(i, j);
    }
    let split = if fields.is_empty() { 0 } else { r.below(fields.len() as u64 + 1) as usize };
    for f in &fields[..split] {
        let _ = writeln!(out, "{ind}  {f}");
    }
    if depth < 3 && r.chance(1, 3) {
        let _ = writeln!(out, "{ind}  CHILD");
        if r.chance(1, 6) {
            let _ = writeln!(out, "{ind}  TEXTCOLOR = {} {} {};", r.below(256), r.below(256), r.below(256));
        }
        for _ in 0..1 + r.below(3) {
            window(r, depth + 1, out, file, counter);
        }
        let _ = writeln!(out, "{ind}  ENDALLCHILDREN");
        if r.chance(1, 20) {
            let _ = writeln!(out, "{ind}  CHILD\n{ind}  ENDALLCHILDREN");
        }
    }
    for f in &fields[split..] {
        let _ = writeln!(out, "{ind}  {f}");
    }
    let _ = writeln!(out, "{ind}END");
}

fn layout(r: &mut Rng, k: u32) -> Vec<u8> {
    let mut s = String::new();
    let version = if r.chance(1, 4) { 1 } else { 2 };
    let _ = writeln!(s, "FILE_VERSION = {version};");
    if version >= 2 && !r.chance(1, 40) {
        s.push_str("STARTLAYOUTBLOCK\n");
        for (t, v) in [("LAYOUTINIT", "Init"), ("LAYOUTUPDATE", "[None]"), ("LAYOUTSHUTDOWN", "Shut")] {
            if r.chance(5, 6) {
                let v = if r.chance(1, 10) { "=".to_string() } else { format!("{v}{}", r.below(3)) };
                let _ = writeln!(s, "  {t} = {v};");
            }
        }
        if r.chance(1, 10) {
            s.push_str("  SOMETHING = else;\n");
        }
        if !r.chance(1, 50) {
            s.push_str("ENDLAYOUTBLOCK\n");
        }
    }
    let file = format!("File{k}.wnd");
    let mut counter = 0;
    for _ in 0..1 + r.below(4) {
        if r.chance(1, 4) {
            let which = r.pick(&["ENABLEDCOLOR", "DISABLEDCOLOR", "HILITECOLOR", "SELECTEDCOLOR", "TEXTCOLOR", "BACKGROUNDCOLOR"]);
            let v = if r.chance(1, 4) { "TRANSPARENT".to_string() } else { format!("{} {} {}", r.below(256), r.below(256), r.below(256)) };
            let _ = writeln!(s, "{which} = {v};");
        }
        if r.chance(1, 8) {
            s.push_str("FONT = NAME: \"Arial\", SIZE: 10, BOLD: 0;\n");
        }
        if r.chance(1, 10) {
            s.push_str("STRAY\n");
        }
        window(r, 0, &mut s, &file, &mut counter);
    }
    let mut b = s.into_bytes();
    if r.chance(1, 12) {
        match r.below(4) {
            0 => {
                let n = r.below(b.len() as u64) as usize;
                b.truncate(n);
            }
            1 => {
                let semis: Vec<usize> = b.iter().enumerate().filter(|(_, &c)| c == b';').map(|(i, _)| i).collect();
                if !semis.is_empty() {
                    let i = semis[r.below(semis.len() as u64) as usize];
                    b.remove(i);
                }
            }
            2 => {
                for _ in 0..1 + r.below(4) {
                    let i = r.below(b.len() as u64) as usize;
                    b.insert(i, (r.below(256)) as u8);
                }
            }
            _ => {
                let digits: Vec<usize> = b.iter().enumerate().filter(|(_, c)| c.is_ascii_digit()).map(|(i, _)| i).collect();
                if !digits.is_empty() {
                    let i = digits[r.below(digits.len() as u64) as usize];
                    b.splice(i..i + 1, b"9999999999".iter().copied());
                }
            }
        }
    }
    b
}

fn units(o: &mut String, t: &Option<Vec<u16>>) {
    match t {
        None => o.push('-'),
        Some(u) => {
            o.push('[');
            for (i, c) in u.iter().enumerate() {
                let _ = write!(o, "{}{c:x}", if i > 0 { " " } else { "" });
            }
            o.push(']');
        }
    }
}

fn draw(o: &mut String, d: &DrawDataSet) {
    for e in d {
        let img = e.image.as_ref().map_or("-".to_string(), |i| String::from_utf8_lossy(i).into_owned());
        let _ = write!(o, " {img}/{:x}/{:x}", e.color as u32, e.border_color as u32);
    }
}

fn s(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// The dump wrap_wnd.cpp writes.
fn dump(results: &[ScriptResult], tree: &WindowTree) -> String {
    let mut o = String::new();
    for (f, r) in results.iter().enumerate() {
        let info = r.info.clone().unwrap_or_default();
        let _ = write!(o, "F {f} first={}", r.first_window.map_or(-1, |w| w as i64));
        let _ = write!(o, " init={} update={} shutdown={}", s(&info.init_name_string), s(&info.update_name_string), s(&info.shutdown_name_string));
        let _ = write!(o, " r={}{}{} v={} windows=", info.resolved[0] as u8, info.resolved[1] as u8, info.resolved[2] as u8, info.version);
        for w in &info.windows {
            let _ = write!(o, "{},", w.map_or(-1, |w| w as i64));
        }
        o.push('\n');
    }
    for (i, w) in tree.windows.iter().enumerate() {
        let d = &w.inst;
        let _ = write!(o, "W {i} p={} st={:x} sty={:x} id={}", w.parent.map_or(-1, |p| p as i64), w.status, d.style, w.id);
        let _ = write!(o, " rg={},{},{},{} sz={},{}", w.region.0, w.region.1, w.region.2, w.region.3, w.size.0, w.size.1);
        let _ = write!(o, " n={} lbl={} tip={} ht={}", s(&d.decorated_name_string), s(&d.text_label_string), s(&d.tooltip_string), s(&d.header_template_name));
        let _ = write!(o, " td={} off={},{} st2={:x}", d.tooltip_delay, d.image_offset.0, d.image_offset.1, d.status);
        o.push_str(" font=");
        match &d.font {
            Some(f) => {
                let _ = write!(o, "{},{},{}", s(&f.name), f.point_size, f.bold as u8);
            }
            None => o.push('-'),
        }
        o.push_str(" txt=");
        units(&mut o, &w.text);
        o.push_str(" tt=");
        units(&mut o, &d.tooltip_text);
        let cb = |i: usize| w.callbacks[i].as_deref().map(s).unwrap_or_default();
        let _ = write!(o, " cb={}|{}|{}|{}", cb(0), cb(1), cb(2), cb(3));
        let b = |x: bool| x as u8;
        let g = match &w.gadget {
            GadgetData::None => "-".to_string(),
            GadgetData::Radio(r) => format!("radio {} {}", r.screen, r.group),
            GadgetData::TabControl(t) => {
                let mut g = format!("tab {} {} {} {} {} {}", t.tab_orientation, t.tab_edge, t.tab_width, t.tab_height, t.tab_count, t.pane_border);
                for p in t.sub_pane_disabled {
                    let _ = write!(g, " {}", b(p));
                }
                g
            }
            GadgetData::Listbox(l) => {
                let mut g = format!(
                    "list {} {} {}{}{}{}{}{}",
                    l.list_length,
                    l.columns,
                    b(l.auto_scroll),
                    b(l.auto_purge),
                    b(l.scroll_bar),
                    b(l.multi_select),
                    b(l.force_select),
                    b(l.scroll_if_at_end)
                );
                for c in &l.column_width_percentage {
                    let _ = write!(g, " {c}");
                }
                g
            }
            GadgetData::Slider(x) => format!("slider {} {}", x.min_val, x.max_val),
            GadgetData::ComboBox(c) => format!("combo {} {} {} {} {}", b(c.is_editable), c.max_display, c.max_chars, b(c.ascii_only), b(c.letters_and_numbers_only)),
            GadgetData::Text(t) => format!("text {} {} {} {}", b(t.centered), b(t.centered_vertically), t.left_margin, t.top_margin),
            GadgetData::Entry(e) => format!("entry {} {} {} {} {}", b(e.secret_text), b(e.numerical_only), b(e.alpha_numerical_only), b(e.ascii_only), e.max_text_len),
        };
        let _ = writeln!(o, " g={g}");
        o.push_str(" E");
        draw(&mut o, &d.enabled_draw_data);
        o.push_str("\n D");
        draw(&mut o, &d.disabled_draw_data);
        o.push_str("\n H");
        draw(&mut o, &d.hilite_draw_data);
        let t = |x: &TextDrawData| format!("{:x}/{:x}", x.color as u32, x.border_color as u32);
        let _ = writeln!(o, "\n T {} {} {}", t(&d.enabled_text), t(&d.disabled_text), t(&d.hilite_text));
        for (part, pd) in &w.parts {
            let _ = write!(o, " P{}", *part as u8);
            draw(&mut o, &pd.enabled);
            draw(&mut o, &pd.disabled);
            draw(&mut o, &pd.hilite);
            o.push('\n');
        }
    }
    o
}

/// Errors the C++ stand-ins throw on (the Rust must stop where the C++ stops).
fn cpp_stops(e: WndError) -> bool {
    matches!(
        e,
        WndError::NullToken | WndError::IntOutOfRange | WndError::BufferOverrun | WndError::ReadPastEnd | WndError::Hangs | WndError::ChildBlockFailed
    )
}

#[test]
fn wnd_reader_matches_upstream() {
    let cases: u64 = std::env::var("WND_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(3000);
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    let displays = [(800, 600), (1024, 768), (1600, 1200), (640, 480), (1366, 768)];
    let (mut files_read, mut windows, mut equal, mut stops, mut skipped, mut mismatches) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut skipped_by: BTreeMap<String, u64> = BTreeMap::new();
    let mut first_bad: Option<String> = None;
    let mut bad_kinds: BTreeMap<String, u64> = BTreeMap::new();
    let dump_dir = std::env::var("WND_DUMP").ok();
    let mut out = vec![0u8; 1 << 22];
    for case in 0..cases {
        let n = 1 + r.below(3) as u32;
        let files: Vec<Vec<u8>> = (0..n).map(|k| layout(&mut r, k)).collect();
        let display = displays[r.below(5) as usize];
        // Rust
        let mut ws = WindowScript::new();
        let mut tree = WindowTree::default();
        let mut host = Host { display, ..Default::default() };
        let mut results = Vec::new();
        let mut err = None;
        for f in &files {
            match ws.win_create_from_script(f, &mut tree, &mut host) {
                Ok(res) => results.push(res),
                Err(e) => {
                    err = Some(e);
                    break;
                }
            }
        }
        if let Some(e) = err {
            if !cpp_stops(e) {
                skipped += 1;
                *skipped_by.entry(format!("{e:?}")).or_default() += 1;
                continue;
            }
        }
        // C++
        let data: Vec<u8> = files.concat();
        let lens: Vec<i32> = files.iter().map(|f| f.len() as i32).collect();
        let mut out_len = 0;
        let code = unsafe {
            cpp::ref_wnd_run(data.as_ptr() as *const _, lens.as_ptr(), n as i32, display.0, display.1, out.as_mut_ptr() as *mut _, out.len() as i32, &mut out_len)
        };
        match err {
            Some(e) => {
                if code == 1 {
                    stops += 1;
                } else {
                    mismatches += 1;
                    *bad_kinds.entry(format!("rust {e:?} / c++ {code}")).or_default() += 1;
                    if let Some(d) = &dump_dir {
                        let _ = std::fs::write(format!("{d}/case{case}.wnd"), files.concat());
                    }
                    first_bad.get_or_insert_with(|| format!("case {case}: Rust stopped ({e:?}), C++ code {code}\n{}", String::from_utf8_lossy(&files.concat())));
                }
            }
            None => {
                files_read += u64::from(n);
                windows += tree.windows.len() as u64;
                let want = dump(&results, &tree);
                let got = String::from_utf8_lossy(&out[..out_len as usize]).into_owned();
                if code == 0 && got == want {
                    equal += 1;
                } else {
                    mismatches += 1;
                    *bad_kinds.entry(format!("rust ok / c++ {code}")).or_default() += 1;
                    if let Some(d) = &dump_dir {
                        let _ = std::fs::write(format!("{d}/case{case}.wnd"), files.concat());
                    }
                    if first_bad.is_none() {
                        let line = want.lines().zip(got.lines()).position(|(a, b)| a != b);
                        let ctx = line.map(|l| format!("rust: {}\n c++: {}", want.lines().nth(l).unwrap_or(""), got.lines().nth(l).unwrap_or(""))).unwrap_or_default();
                        first_bad = Some(format!("case {case}: C++ code {code}\n{ctx}\n--- input\n{}", String::from_utf8_lossy(&files.concat())));
                    }
                }
            }
        }
    }
    eprintln!(
        "#12 WND differential: {cases} cases, {files_read} files read and {windows} windows compared in {equal} identical cases, \
{stops} stops agreed, {skipped} undefined skipped ({skipped_by:?}), {mismatches} mismatches {bad_kinds:?}"
    );
    if let Some(b) = first_bad {
        let cut: String = b.chars().take(6000).collect();
        eprintln!("first mismatch: {cut}");
    }
    assert_eq!(mismatches, 0);
    assert!(equal > cases / 3, "too few cases compared");
}
