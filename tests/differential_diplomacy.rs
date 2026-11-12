#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Diplomacy.cpp (grabWindowPointers,
//     releaseWindowPointers, updateFunc, UpdateDiplomacyBriefingText, ShowDiplomacy, ResetDiplomacy, HideDiplomacy,
//     ToggleDiplomacy, DiplomacyInput, DiplomacySystem)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::diplomacy` (GitHub #148) against Diplomacy.cpp.
//!
//! # Method
//!
//! The C++ side is Diplomacy.cpp from its statics to `PopulateInGameDiplomacyPopup`, cut verbatim by build.rs, on
//! stand-ins that record each call (`differential/wrap_diplomacy.cpp` documents the model and the words); the Rust side is
//! the same model as a [`DiplomacyHost`]. Both run the same random sequence: briefings (eight labels, among them a repeat
//! of another's text in other case or with a trailing space, and the empty one) with and without the clear; show, hide
//! and toggle, immediate or not; reset; the layout's update (the slide's frames); Escape and other keys through
//! `DiplomacyInput`; `DiplomacySystem`'s focus messages and its buttons (`ButtonHide`, the two radio buttons, the other
//! windows, a window of no name); and the game's flags (input off, the intro movie, the map loading, the quit menu, the
//! disconnect screen, a multiplayer game, `m_animateWindows` off). A sequence's layout has the solo list box or not.
//! Compared: every call in order (the windows hidden and enabled, the list box reset and its entries' text and colour,
//! the radio selection, the layout made, hidden and destroyed, the animation manager's life and calls, the layout
//! registered, the quit menu hidden, the multiplayer rows), and after each step the handled result, the focus flag,
//! whether the window exists, and the briefing list.
//!
//! # Predictions (written before the first run, 2026-11-12)
//!
//! - 0 mismatches over 20,000 sequences: no arithmetic, the same calls in the same order.
//! - Plants 9411-9418 all caught: 9411 (repeats added) by any repeated label; 9412 (clear leaves the box) by a clear with
//!   the window up and entries in it; 9413 (always the first colour) by a second entry; 9414 (the toggle inverted) by any
//!   toggle with the window made; 9415 (shown with the input off) by a show after the input flag goes off; 9416 (the
//!   immediate hide does not disable) by any immediate hide of a made window; 9417 (solo and in-game swapped) by every
//!   show; 9418 (the empty label recorded) by any empty briefing.
//!
//! # Results (2026-11-12, `DIPLOMACY_CASES=20000`, the default)
//!
//! - **First run: a miss in the harness, not the port.** The C++ crashed (SIGSEGV in `DiplomacySystem`): a radio button's
//!   click before the window existed (or after `ResetDiplomacy`) calls `winInGame->winHide` on `NULL`, as upstream would
//!   (its control IDs are statics kept from an earlier sequence's `ShowDiplomacy`). No click can reach the window then,
//!   so both sides now give `GBM_SELECTED` only while the window exists; the port skips the `NULL` call.
//! - Second run: 20,000 sequences, 509,378 steps (76,333 toggles, 178,314 briefings): **0 mismatches**, as predicted.
//! - Plants, mismatching sequences of 20,000: 9411 13,636; 9412 6,723; 9413 11,918; 9414 13,597; 9415 1,414; 9416
//!   13,455; 9417 17,256; 9418 11,425. **8 of 8 caught**, as predicted.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 9411 | a label already in the history is added again |
//! | 9412 | the clear empties the history but not the solo list box |
//! | 9413 | every entry gets the first message colour |
//! | 9414 | `ToggleDiplomacy` hides a hidden window and shows a shown one |
//! | 9415 | `ShowDiplomacy` does not stop when the input is off |
//! | 9416 | `HideDiplomacy(TRUE)` does not disable the window |
//! | 9417 | the solo and in-game parents are swapped |
//! | 9418 | the empty label is recorded |

mod support;

use rts_lockstep_engine::client::diplomacy::*;
use rts_lockstep_engine::client::window_manager::{ggm, gwm, MsgHandled};
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

const NAMES: [&str; 8] = [
    "Diplomacy.wnd:DiplomacyParent",
    "Diplomacy.wnd:ListboxSolo",
    "Diplomacy.wnd:RadioButtonInGame",
    "Diplomacy.wnd:RadioButtonBuddies",
    "Diplomacy.wnd:InGameParent",
    "Diplomacy.wnd:BuddiesParent",
    "Diplomacy.wnd:SoloParent",
    "Diplomacy.wnd:ButtonHide",
];
const LABELS: [&[u8]; 9] = [b"SCRIPT:Brief1", b"SCRIPT:Brief2", b"SCRIPT:Pop", b"DIALOGEVENT:Speech1Subtitle", b"SCRIPT:Brief1 ", b"script:brief1", b"MISSING", b"SCRIPT:Long briefing label", b""];

#[derive(Clone, Default)]
struct Win {
    hidden: bool,
    entries: Vec<(i32, i32)>,
}

/// The wrapper's model: the layout's windows, the name keys, the animation manager, the game's flags, the call log.
struct Model {
    d: Diplomacy,
    keys: Vec<Vec<u8>>,
    wins: Vec<Win>,
    made: bool,
    has_listbox: bool,
    anim_left: i32,
    anim_reverse: bool,
    out: Vec<i32>,
}

fn fnv(t: &[u16]) -> i32 {
    let mut h: u32 = 2166136261;
    for &u in t {
        h ^= u32::from(u);
        h = h.wrapping_mul(16777619);
    }
    h as i32
}

impl DiplomacyHost for Model {
    fn diplomacy(&mut self) -> &mut Diplomacy {
        &mut self.d
    }
    fn name_to_key(&mut self, name: &[u8]) -> i32 {
        if let Some(i) = self.keys.iter().position(|k| k == name) {
            return i as i32 + 1;
        }
        self.keys.push(name.to_vec());
        self.keys.len() as i32
    }
    fn win_get_window_from_id(&mut self, _parent: Option<WinId>, name: &[u8]) -> Option<WinId> {
        let key = self.name_to_key(name);
        if !self.made {
            return None;
        }
        let i = (0..8).find(|&i| key == i as i32 + 1)?;
        (i != 1 || self.has_listbox).then_some(i)
    }
    fn win_create_layout(&mut self, _file: &[u8]) -> Option<usize> {
        self.out.push(6);
        self.made = true;
        for w in &mut self.wins {
            *w = Win::default();
        }
        Some(0)
    }
    fn layout_first_window(&mut self, _layout: usize) -> Option<WinId> {
        Some(0)
    }
    fn layout_hide(&mut self, _layout: usize, hide: bool) {
        self.out.extend([7, i32::from(hide)]);
        for w in &mut self.wins {
            w.hidden = hide;
        }
    }
    fn layout_destroy(&mut self, _layout: usize) {
        self.out.push(8);
        self.made = false;
        for w in &mut self.wins {
            w.entries.clear();
        }
    }
    fn win_hide(&mut self, w: WinId, hide: bool) {
        self.wins[w].hidden = hide;
        self.out.extend([1, w as i32, i32::from(hide)]);
    }
    fn win_enable(&mut self, w: WinId, enable: bool) {
        self.out.extend([2, w as i32, i32::from(enable)]);
    }
    fn win_is_hidden(&mut self, w: WinId) -> bool {
        self.wins[w].hidden
    }
    fn list_box_reset(&mut self, w: WinId) {
        self.wins[w].entries.clear();
        self.out.extend([3, w as i32]);
    }
    fn list_box_get_num_entries(&mut self, w: WinId) -> i32 {
        self.wins[w].entries.len() as i32
    }
    fn list_box_add_entry_text(&mut self, w: WinId, text: &[u16], color: i32) {
        self.wins[w].entries.push((fnv(text), color));
        self.out.extend([4, w as i32, fnv(text), color]);
    }
    fn radio_set_selection(&mut self, w: WinId) {
        self.out.extend([5, w as i32]);
    }
    fn fetch(&mut self, label: &[u8]) -> Vec<u16> {
        // (`UnicodeString::translate`: each byte sign-extended)
        label.iter().map(|&b| (b as i8) as i16 as u16).collect()
    }
    fn animate_new(&mut self) {
        self.anim_left = 0;
        self.anim_reverse = false;
        self.out.push(9);
    }
    fn animate_delete(&mut self) {
        self.out.push(10);
    }
    fn animate_reset(&mut self) {
        self.anim_left = 0;
        self.anim_reverse = false;
        self.out.push(11);
    }
    fn animate_update(&mut self) {
        if self.anim_left > 0 {
            self.anim_left -= 1;
        }
        self.out.push(14);
    }
    fn animate_is_finished(&mut self) -> bool {
        self.anim_left == 0
    }
    fn animate_is_reversed(&mut self) -> bool {
        self.anim_reverse
    }
    fn animate_reverse(&mut self) {
        self.anim_reverse = true;
        self.anim_left = 3;
        self.out.push(13);
    }
    fn animate_register_slide_top(&mut self, w: WinId) {
        self.anim_left = 3;
        self.out.extend([12, w as i32]);
    }
    fn register_window_layout(&mut self, _layout: usize, register: bool) {
        self.out.extend([15, i32::from(register)]);
    }
    fn hide_quit_menu(&mut self) {
        self.out.push(16);
    }
    fn populate_in_game_diplomacy_popup(&mut self) {
        self.out.push(17);
    }
}

fn handled(m: MsgHandled) -> i32 {
    match m {
        MsgHandled::Ignored => 0,
        MsgHandled::Handled => 1,
    }
}

fn rust_seq(input: &[i32]) -> Vec<i32> {
    let mut m = Model {
        d: Diplomacy::default(),
        keys: NAMES.iter().map(|n| n.as_bytes().to_vec()).collect(),
        wins: vec![Win::default(); 8],
        made: false,
        has_listbox: input[0] != 0,
        anim_left: 0,
        anim_reverse: false,
        out: Vec::new(),
    };
    m.d.game.message_colors = [0xFFFF_FFFFu32 as i32, 0xFFB4_B4B4u32 as i32];
    for s in input[1..].chunks(8) {
        if s.len() < 8 {
            break;
        }
        let w = &s[1..];
        let (mut h, mut focus) = (-1, 1);
        match s[0] {
            0 => update_diplomacy_briefing_text(&mut m, LABELS[w[0] as usize % 9], w[1] != 0),
            1 => show_diplomacy(&mut m, w[0] != 0),
            2 => hide_diplomacy(&mut m, w[0] != 0),
            3 => toggle_diplomacy(&mut m, w[0] != 0),
            4 => reset_diplomacy(&mut m),
            5 => {
                if m.d.the_layout.is_some() {
                    update_func(&mut m);
                }
            }
            6 => h = handled(diplomacy_input(&mut m, if w[0] == 0 { gwm::CHAR } else { 7 }, w[1] as u8)),
            7 => {
                let msg = match w[0] {
                    0 => ggm::FOCUS_CHANGE,
                    1 => gwm::INPUT_FOCUS,
                    2 => ggm::GBM_SELECTED,
                    _ => 7,
                };
                let control_id = if (0..8).contains(&w[1]) { m.name_to_key(NAMES[w[1] as usize].as_bytes()) } else { 999 };
                let mut f = true;
                // (a click needs the window, as the wrapper's)
                if msg != ggm::GBM_SELECTED || m.d.the_window.is_some() {
                    h = handled(diplomacy_system(&mut m, msg, control_id, (msg == gwm::INPUT_FOCUS).then_some((w[2] != 0, &mut f))));
                }
                focus = i32::from(f);
            }
            _ => {
                let g = &mut m.d.game;
                (g.input_enabled, g.intro_movie_playing, g.loading_map, g.quit_menu_visible, g.disconnect_screen_visible, g.multiplayer, g.animate_windows) =
                    (w[0] != 0, w[1] != 0, w[2] != 0, w[3] != 0, w[4] != 0, w[5] != 0, w[6] != 0);
            }
        }
        m.out.extend([100, h, focus, i32::from(m.d.the_window.is_some()), m.d.briefing_list.len() as i32]);
        let list: Vec<i32> = m.d.briefing_list.iter().map(|l| LABELS.iter().rposition(|x| x == l).map_or(-1, |i| i as i32)).collect();
        m.out.extend(list);
    }
    m.out
}

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 200_000];
    let n = unsafe { cpp::ref_diplomacy_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    let mut v = vec![i32::from(g.below(5) != 0)];
    for _ in 0..1 + g.below(50) {
        let mut s = [0i32; 8];
        let kind = match g.below(20) {
            0..=6 => 0,
            7 | 8 => 1,
            9 | 10 => 2,
            11..=13 => 3,
            14 => 4,
            15 | 16 => 5,
            17 => 6,
            18 => 7,
            _ => 8,
        };
        s[0] = kind;
        match kind {
            0 => {
                s[1] = g.below(9) as i32;
                s[2] = i32::from(g.below(8) == 0);
            }
            1..=3 => s[1] = g.below(2) as i32,
            6 => {
                s[1] = g.below(3).min(1) as i32;
                s[2] = if g.below(2) == 0 { 0x01 } else { g.below(256) as i32 };
            }
            7 => {
                s[1] = g.below(4) as i32;
                s[2] = g.below(10) as i32 - 1;
                s[3] = g.below(2) as i32;
            }
            8 => {
                // (mostly an ordinary game: each flag set one time in five)
                s[1] = i32::from(g.below(5) != 0);
                for i in 2..7 {
                    s[i] = i32::from(g.below(5) == 0);
                }
                s[7] = i32::from(g.below(5) != 0);
            }
            _ => {}
        }
        v.extend(s);
    }
    v
}

/// (#151) The two tests share wrap_diplomacy.cpp's statics (`theBriefingList`, the window's): one at a time.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn the_briefing_history_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: u64 = std::env::var("DIPLOMACY_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x148_0170);
    let mut whole = Check::new("the briefing history");
    let (mut steps, mut toggles, mut briefings) = (0u64, 0u64, 0u64);
    for i in 0..cases {
        let input = gen_case(&mut g);
        steps += ((input.len() - 1) / 8) as u64;
        let r = rust_seq(&input);
        let c = cpp_seq(&input);
        toggles += input[1..].chunks(8).filter(|s| s[0] == 3).count() as u64;
        briefings += input[1..].chunks(8).filter(|s| s[0] == 0).count() as u64;
        if r != c && !planted {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let k0 = k.saturating_sub(8);
            println!("sequence {i} first difference at word {k}: rust {:?} c++ {:?}", &r[k0.min(r.len())..(k + 8).min(r.len())], &c[k0.min(c.len())..(k + 8).min(c.len())]);
        }
        whole.eq(r, c, || format!("sequence {i}: {input:?}"));
    }
    println!("{cases} sequences, {steps} steps ({toggles} toggles, {briefings} briefings)");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}

// ---------------------------------------------------------------------------------------------------------------------
// #151: GameClient::xfer's briefing list
// ---------------------------------------------------------------------------------------------------------------------

/// wrap_diplomacy.cpp's `long_label`: Diplomacy's nine labels, then one of 255 bytes and one of 256.
fn long_label(i: i32) -> Vec<u8> {
    match i {
        0..=8 => LABELS[i as usize].to_vec(),
        9 => vec![b'L'; 255],
        _ => vec![b'L'; 256],
    }
}

fn fnv_ascii(a: &[u8]) -> i32 {
    let mut h: u32 = 2166136261;
    for &b in a {
        h ^= u32::from(b);
        h = h.wrapping_mul(16777619);
    }
    h as i32
}

/// A status as the wrapper gives it: 0, or the `XferStatus` + 1.
fn status(r: Result<(), rts_lockstep_engine::common::xfer::XferStatus>) -> i32 {
    match r {
        Ok(()) => 0,
        Err(e) => e as i32 + 1,
    }
}

fn xfer_path(kind: &str) -> String {
    format!("{}/briefing_xfer_{kind}_{}.bin", std::env::temp_dir().display(), std::process::id())
}

/// A save: `labels` (indices of [`long_label`]) as the list, at `version`: the status and the bytes. Rust's, then C++'s.
fn save_both(labels: &[i32], version: i32) -> ((i32, Vec<u8>), (i32, Vec<u8>)) {
    use rts_lockstep_engine::common::xfer::Xfer;
    let mut d = Diplomacy { briefing_list: labels.iter().map(|&i| long_label(i % 11)).collect(), ..Default::default() };
    let mut x = rts_lockstep_engine::common::xfer_save::XferSave::new();
    let st = match x.open(b"briefing") {
        Ok(()) => status(xfer_briefing_list(&mut x, version as u8, &mut BriefingListOnly(&mut d))),
        Err(e) => e as i32 + 1,
    };
    let rust = (st, x.bytes().to_vec());
    let path = xfer_path("save");
    let cpath = std::ffi::CString::new(path.clone()).expect("path");
    let cst = unsafe { cpp::ref_briefing_xfer_save(labels.as_ptr(), labels.len() as i32, version, cpath.as_ptr()) };
    let bytes = std::fs::read(&path).unwrap_or_default();
    let _ = std::fs::remove_file(&path);
    (rust, (cst, bytes))
}

/// A load of `bytes` at `version` after `pre` ([the layout has ListboxSolo, ShowDiplomacy first, the count, the labels]):
/// the status, the calls the load made, and the list.
fn load_both(pre: &[i32], bytes: &[u8], version: i32) -> (Vec<i32>, Vec<i32>) {
    use rts_lockstep_engine::common::xfer::Xfer;
    let mut m = Model {
        d: Diplomacy::default(),
        keys: NAMES.iter().map(|n| n.as_bytes().to_vec()).collect(),
        wins: vec![Win::default(); 8],
        made: false,
        has_listbox: pre[0] != 0,
        anim_left: 0,
        anim_reverse: false,
        out: Vec::new(),
    };
    m.d.game.message_colors = [0xFFFF_FFFFu32 as i32, 0xFFB4_B4B4u32 as i32];
    if pre[1] != 0 {
        show_diplomacy(&mut m, true);
    }
    for &l in &pre[3..3 + pre[2] as usize] {
        update_diplomacy_briefing_text(&mut m, &long_label(l % 9), false);
    }
    m.out.clear();
    let mut x = rts_lockstep_engine::common::xfer_load::XferLoad::new(bytes.to_vec());
    let st = match x.open(b"briefing") {
        Ok(()) => status(xfer_briefing_list(&mut x, version as u8, &mut m)),
        Err(e) => e as i32 + 1,
    };
    let mut rust = vec![st];
    rust.append(&mut m.out);
    rust.extend([100, m.d.briefing_list.len() as i32]);
    rust.extend(m.d.briefing_list.iter().map(|l| fnv_ascii(l)));
    let path = xfer_path("load");
    std::fs::write(&path, bytes).expect("write the save");
    let cpath = std::ffi::CString::new(path.clone()).expect("path");
    let mut out = vec![0i32; 100_000];
    let n = unsafe { cpp::ref_briefing_xfer_load(pre.as_ptr(), pre.len() as i32, version, cpath.as_ptr(), out.as_mut_ptr(), out.len() as i32) };
    let _ = std::fs::remove_file(&path);
    out.truncate(n as usize);
    (rust, out)
}

/// #151: `GameClient::xfer`'s briefing list (`client::diplomacy::xfer_briefing_list`) against GameClient.cpp's own block,
/// cut verbatim, on Diplomacy.cpp's cut and the real `XferSave` / `XferLoad`.
///
/// # Method
///
/// Each sequence is a save and a load. The save: a list of 0 to 12 labels (Diplomacy's nine, repeats kept, and one time
/// in twenty a 255- or 256-byte one, which `XferSave` refuses: `XFER_STRING_ERROR`) at version 1, 2 or 3; compared, the
/// status and the file's bytes. The load: the bytes of a random list's save, or of a hand-made file (a count from -3 to
/// 15, labels of any bytes, NUL among them, of 0 to 40 bytes, the file sometimes cut short), into a briefing history
/// whose layout has the solo list box or not, shown first or not, holding 0 to 4 labels already; compared, the status,
/// every call the load made on the window (the list box's reset and entries, as `ref_diplomacy_seq` records them) and the
/// list after it.
///
/// # Predictions (written before the first run, 2026-11-12)
///
/// - 0 mismatches over 20,000 sequences: a count and strings on both sides, through code already compared (#148).
/// - Plants 9423-9425 all caught: 9423 (the save's count at most 1) by every version-2 save of two labels or more, about
///   half the sequences; 9424 (the load does not clear) by every load into a history holding labels, about 4 in 5 of
///   them; 9425 (the load adds without `UpdateDiplomacyBriefingText`) by every load of an empty or repeated label, and
///   by a shown window (no entry added), about half.
///
/// # Results (2026-11-12, `BRIEFING_CASES=20000`, the default)
///
/// - First run: 20,000 sequences, **0 mismatches** in the saves and in the loads, as predicted. 1,816 saves refused (a
///   256-byte label: `XFER_STRING_ERROR`), 4,298 loads that stopped (a file cut short: `XFER_READ_ERROR`), about 29,000
///   list box entries added by loads into a shown window.
/// - Plants, sequences that differ of 20,000: 9423 **11,294** saves (predicted about half: held); 9424 **12,586** loads
///   (predicted about 4 in 5: **missed, low**, 63 %: a load that stops on its count, or of version 1, reads before the
///   clear or not at all); 9425 **9,771** loads (predicted about half: held). **3 of 3 caught.**
///
/// # Planted bugs
///
/// | id | planted bug |
/// |---|---|
/// | 9423 | the save writes a count of at most 1 |
/// | 9424 | the load does not clear the list first |
/// | 9425 | the load pushes each label without `UpdateDiplomacyBriefingText` |
#[test]
fn the_briefing_list_xfer_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: u64 = std::env::var("BRIEFING_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x151_0002);
    let mut saves = Check::new("the briefing list's save");
    let mut loads = Check::new("the briefing list's load");
    let (mut errors, mut load_errors, mut entries) = (0u64, 0u64, 0u64);
    for i in 0..cases {
        let version = 1 + g.below(3) as i32;
        let n = g.below(13) as usize;
        let labels: Vec<i32> = (0..n).map(|_| if g.below(20) == 0 { 9 + g.below(2) as i32 } else { g.below(9) as i32 }).collect();
        let (r, c) = save_both(&labels, version);
        if r.0 != 0 {
            errors += 1;
        }
        if r != c && !planted {
            println!("sequence {i} save differs: rust {r:?} c++ {c:?}");
        }
        saves.eq(r.clone(), c, || format!("sequence {i}: save {labels:?} at version {version}"));
        // the load's file: the save's own bytes, or a hand-made one
        let bytes = if g.below(2) == 0 && r.0 == 0 {
            r.1
        } else {
            let mut b = Vec::new();
            let count = g.below(19) as i32 - 3;
            b.extend_from_slice(&count.to_le_bytes());
            for _ in 0..count.max(0) {
                let len = g.below(41) as usize;
                b.push(len as u8);
                for _ in 0..len {
                    b.push(match g.below(10) {
                        0 => 0,
                        1 => 0x80 + g.below(128) as u8,
                        _ => b'A' + g.below(4) as u8,
                    });
                }
            }
            if g.below(8) == 0 && !b.is_empty() {
                let cut = g.below(b.len() as u32) as usize;
                b.truncate(cut);
            }
            b
        };
        let pre_count = g.below(5) as i32;
        let mut pre = vec![i32::from(g.below(5) != 0), i32::from(g.below(2) == 0), pre_count];
        pre.extend((0..pre_count).map(|_| g.below(9) as i32));
        let load_version = if g.below(10) == 0 { 1 } else { version.max(2) };
        let (r, c) = load_both(&pre, &bytes, load_version);
        if r[0] != 0 {
            load_errors += 1;
        }
        entries += r.iter().filter(|&&w| w == 4).count() as u64;
        if r != c && !planted {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            println!("sequence {i} load first difference at word {k}: rust {:?} c++ {:?}", &r[k.saturating_sub(4)..(k + 8).min(r.len())], &c[k.saturating_sub(4).min(c.len())..(k + 8).min(c.len())]);
        }
        loads.eq(r, c, || format!("sequence {i}: load of {bytes:?} at version {load_version} after {pre:?}"));
    }
    println!("{cases} sequences: {errors} saves refused, {load_errors} loads that stopped, about {entries} list box entries added by loads");
    if planted {
        let a = saves.report();
        let b = loads.report();
        println!("planted: {a} saves and {b} loads differ");
    } else {
        saves.finish();
        loads.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
