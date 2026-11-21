// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui3a) of the six small WOL screens' drafts (`src/drafts/ui/`: `wol_message_window.rs`,
//! `wol_status_menu.rs`, `wol_custom_score_screen.rs`, `wol_qm_score_screen.rs`, `wol_ladder_screen.rs`,
//! `wol_locale_select_popup.rs`) against the upstream `WOLMessageWindow.cpp`, `WOLStatusMenu.cpp`, `WOLCustomScoreScreen.cpp`,
//! `WOLQMScoreScreen.cpp`, `WOLLadderScreen.cpp`, `WOLLocaleSelectPopup.cpp`, each cut verbatim from its first static to the end of
//! the file by build.rs (`drafts_ui3a`; `differential/wrap_draft_wol_small.cpp` lists the stand-ins, `differential/ui3a_host.h` the
//! window system: `winGetWindowFromId`, `winSendSystemMsg`, `winSendInputMsg`, `winSetFocus`, `windowHiding`, `winSetModal`,
//! `GameWindow::winHide` and `WindowLayout::hide` are upstream's text too).
//!
//! # Method
//!
//! A case is one screen, a subset of its windows (a parent, which may be missing, and its two children), whether the parent takes
//! focus, the layout's first hidden state, whether a push is pending in the shell, whether a web browser exists, the GameSpy layer's
//! constants, and 1 to 24 operations: the layout's `Init`, `Shutdown` and `Update`, a key or another message to the parent's input
//! callback, a system message to the parent (`CREATE`, `DESTROY`, `INPUT_FOCUS` with 0, 1 or other, `GBM_SELECTED` and
//! `GLM_SELECTED` with a control and an id, `GEM_EDIT_DONE`, others), and `winSetFocus(parent)` / `winSetFocus(NULL)`. The Rust side runs
//! the draft on the REAL Rust window manager and gadgets (windows from a synthetic WND of the screen's names, the list box a real
//! gadget) with a host that logs every call outside the draft; the C++ side runs upstream's text on the stand-ins. After each op
//! both print the log (every callback entered, `sys <window> <msg> <d1> <d2>` with the control named, its result, the shell, web,
//! GameSpy and preference calls) and the state (each window's hidden and enabled flag, the keyboard focus, the modal head, the
//! layout's hidden flag, the locale list's rows, colours and selection), and the two texts are compared.
//!
//! Where the Rust cannot log a call of the verified shell (`shutdownComplete`, `showShellMap`, `pop` run the real
//! `client::shell`), the host reads its effect: `showShellMap(TRUE)` is the `NewShellGame` request, `showShellMap(FALSE)` the creation of
//! `Menus/BlankWindow.wnd`, a pending push the creation of its layout, `pop` the top layout's shutdown callback, and the number of
//! `shutdownComplete` calls in a `Shutdown` is the host's `shell()` accessor count divided by the count one call costs (measured at the
//! start of the run); that count is compared only when no push was pending at the start of the op.
//!
//! Excluded inputs: `GBM_SELECTED` with a NULL control to the ladder or the locale popup (upstream dereferences it), and the locale
//! popup's OK with no list box (upstream's `int selected` is then unset).
//!
//! # Predictions (written before the first run)
//!
//! Written before the first run, from reading `PersistentStorageDefs.h`: `LOC_MAX` is 37 upstream and the draft says 20, so every locale
//! `Init` with a list box must mismatch (the list's rows); nothing else was expected to. (A second expectation, written from a misreading
//! of `WOLQMScoreScreenShutdown` as calling `shutdownComplete` twice, was wrong: upstream calls it once, and the run agreed with that.)
//!
mod support_ui3a;

use rts_lockstep_engine::client::gadget_list_box as lb;
use rts_lockstep_engine::client::shell::{self, KeyedFiles, LayoutFiles, NameKeys, Shell, ShellEnv, ShellHost, ShellRequest, WndFiles};
use rts_lockstep_engine::client::window_manager::{data_win, ggm, gwm, win_data, GameWindowManager, GuiHost, MsgHandled, WindowMsgData};
use rts_lockstep_engine::client::window_script::{win_status, WinId, WndHost};
use rts_lockstep_engine::drafts::ui::wol_custom_score_screen as cust;
use rts_lockstep_engine::drafts::ui::wol_ladder_screen as lad;
use rts_lockstep_engine::drafts::ui::wol_locale_select_popup as loc;
use rts_lockstep_engine::drafts::ui::wol_message_window as msgw;
use rts_lockstep_engine::drafts::ui::wol_qm_score_screen as qm;
use rts_lockstep_engine::drafts::ui::wol_status_menu as stat;
use std::collections::BTreeMap;
use support_ui3a::Rng;

extern "C" {
    // (`--cfg u3a_dev`: the fast loop of the author links a hand-built copy of the wrapper under another name, see the notes of the PR)
    #[cfg_attr(u3a_dev, link_name = "ref_u3a_small_run_dev")]
    fn ref_u3a_small_run(input: *const i32, n: i32, out: *mut u8, cap: i32) -> i32;
}

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A screen: the WND's file, its windows' short names with the kinds the synthetic WND makes them, the callbacks' names.
struct Menu {
    file: &'static str,
    wins: &'static [(&'static str, &'static str)],
    system: &'static [u8],
    input: &'static [u8],
    init: &'static str,
    shutdown: &'static str,
}

const MENUS: [Menu; 6] = [
    Menu { file: "WOLMessageWindow.wnd", wins: &[("USER", "WOLMessageWindowParent"), ("PUSHBUTTON", "ButtonCancel")], system: b"WOLMessageWindowSystem", input: b"WOLMessageWindowInput", init: "WOLMessageWindowInit", shutdown: "WOLMessageWindowShutdown" },
    Menu { file: "WOLStatusMenu.wnd", wins: &[("USER", "WOLStatusMenuParent"), ("PUSHBUTTON", "ButtonDisconnect"), ("SCROLLLISTBOX", "ListboxStatus")], system: b"WOLStatusMenuSystem", input: b"WOLStatusMenuInput", init: "WOLStatusMenuInit", shutdown: "WOLStatusMenuShutdown" },
    Menu { file: "WOLCustomScoreScreen.wnd", wins: &[("USER", "WOLCustomScoreScreenParent"), ("PUSHBUTTON", "ButtonDisconnect"), ("PUSHBUTTON", "ButtonLobby")], system: b"WOLCustomScoreScreenSystem", input: b"WOLCustomScoreScreenInput", init: "WOLCustomScoreScreenInit", shutdown: "WOLCustomScoreScreenShutdown" },
    Menu { file: "WOLQMScoreScreen.wnd", wins: &[("USER", "WOLQMScoreScreenParent"), ("PUSHBUTTON", "ButtonDisconnect"), ("PUSHBUTTON", "ButtonQuickMatch")], system: b"WOLQMScoreScreenSystem", input: b"WOLQMScoreScreenInput", init: "WOLQMScoreScreenInit", shutdown: "WOLQMScoreScreenShutdown" },
    Menu { file: "WOLLadderScreen.wnd", wins: &[("USER", "LadderParent"), ("PUSHBUTTON", "ButtonBack"), ("USER", "WindowLadder")], system: b"WOLLadderScreenSystem", input: b"WOLLadderScreenInput", init: "WOLLadderScreenInit", shutdown: "WOLLadderScreenShutdown" },
    Menu { file: "PopupLocaleSelect.wnd", wins: &[("USER", "ParentLocaleSelect"), ("PUSHBUTTON", "ButtonOk"), ("SCROLLLISTBOX", "ListBoxLocaleSelect")], system: b"WOLLocaleSelectSystem", input: b"WOLLocaleSelectInput", init: "WOLLocaleSelectInit", shutdown: "WOLLocaleSelectShutdown" },
];

#[derive(Clone, Debug)]
struct Case {
    menu: usize,
    mask: i32,
    no_focus: bool,
    layout_hidden0: bool,
    pending_push: bool,
    has_web: bool,
    color: i32,
    local_id: i32,
    stats_kind: i32,
    stats_locale: i32,
    stack_top: bool,
    ops: Vec<[i32; 5]>,
}

const LIST_LENGTH: i32 = 100;

fn wnd(m: &Menu, mask: i32, no_focus: bool) -> Vec<u8> {
    let child = |kind: &str, name: &str| {
        let (style, extra) = match kind {
            "SCROLLLISTBOX" => ("SCROLLLISTBOX", format!("    LISTBOXDATA = LENGTH: {LIST_LENGTH}, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 0, MULTISELECT: 0, COLUMNS: 1, COLUMNSWIDTH%: 100, FORCESELECT: 0;\n")),
            "PUSHBUTTON" => ("PUSHBUTTON", "    TEXT = \"GUI:Back\";\n".to_string()),
            _ => ("USER", String::new()),
        };
        format!(
            "  WINDOW\n    WINDOWTYPE = {kind};\n    SCREENRECT = UPPERLEFT: 10 10, BOTTOMRIGHT: 110 40, CREATIONRESOLUTION: 800 600;\n    NAME = \"{}:{name}\";\n    STATUS = ENABLED;\n    STYLE = {style};\n{extra}  END\n",
            m.file
        )
    };
    let mut children = String::new();
    for (i, (kind, name)) in m.wins.iter().enumerate().skip(1) {
        if mask & (1 << i) != 0 {
            children += &child(kind, name);
        }
    }
    let status = if no_focus { "ENABLED+HIDDEN+NOFOCUS" } else { "ENABLED+HIDDEN" };
    let mut s = format!("FILE_VERSION = 2;\nSTARTLAYOUTBLOCK\n  LAYOUTINIT = {};\n  LAYOUTUPDATE = [None];\n  LAYOUTSHUTDOWN = {};\nENDLAYOUTBLOCK\n", m.init, m.shutdown);
    if mask & 1 != 0 {
        s += &format!(
            "WINDOW\n  WINDOWTYPE = USER;\n  SCREENRECT = UPPERLEFT: 0 0, BOTTOMRIGHT: 800 600, CREATIONRESOLUTION: 800 600;\n  NAME = \"{}:{}\";\n  STATUS = {status};\n  STYLE = USER;\n  SYSTEMCALLBACK = \"{}\";\n  INPUTCALLBACK = \"{}\";\n  CHILD\n{children}  ENDALLCHILDREN\nEND\n",
            m.file,
            m.wins[0].1,
            String::from_utf8_lossy(m.system),
            String::from_utf8_lossy(m.input)
        );
    } else {
        // no parent: the children are windows of their own
        let roots = children.replace("  WINDOW\n", "WINDOW\n").replace("\n  END\n", "\nEND\n");
        s += &roots;
    }
    s.into_bytes()
}

#[derive(Clone, Default)]
struct Files {
    wnd: Vec<u8>,
}

impl WndHost for Files {
    fn name_to_key(&mut self, _name: &[u8]) -> u32 {
        unreachable!("the keys are the host's (KeyedFiles)")
    }
    fn find_image(&self, _name: &[u8]) -> Option<Vec<u8>> {
        None
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        b"T:".iter().chain(label).map(|&b| u16::from(b)).collect()
    }
    fn display_size(&self) -> (i32, i32) {
        (800, 600)
    }
}

impl WndFiles for Files {
    fn open_wnd(&mut self, _path: &[u8]) -> Option<Vec<u8>> {
        Some(self.wnd.clone())
    }
}

struct Host {
    shell: Shell,
    shell_calls: usize,
    files: Option<LayoutFiles<Files>>,
    keys: NameKeys,
    names: BTreeMap<u32, String>,
    log: String,
    menu: usize,
    msgw: msgw::WolMessageWindow,
    stat: stat::WolStatusMenu,
    cust: cust::WolCustomScoreScreen,
    qm: qm::WolQmScoreScreen,
    lad: lad::WolLadderScreen,
    loc: loc::WolLocaleSelect,
    has_web: bool,
    color: i32,
    local_id: i32,
    stats_kind: i32,
    stats_locale: i32,
}

fn short(name: &[u8]) -> String {
    let s = String::from_utf8_lossy(name).into_owned();
    s.split_once(':').map_or(s.clone(), |(_, b)| b.to_string())
}

fn win_short(wm: &GameWindowManager, w: Option<WinId>) -> String {
    w.map_or("-".to_string(), |w| short(wm.tree.windows[w].name()))
}

fn msg_name(m: u32) -> &'static str {
    match m {
        gwm::CREATE => "CREATE",
        gwm::DESTROY => "DESTROY",
        gwm::CHAR => "CHAR",
        gwm::INPUT_FOCUS => "INPUT_FOCUS",
        gwm::LEFT_DOWN => "LEFT_DOWN",
        ggm::GBM_SELECTED => "GBM_SELECTED",
        ggm::GLM_SELECTED => "GLM_SELECTED",
        ggm::GEM_EDIT_DONE => "GEM_EDIT_DONE",
        _ => "OTHER",
    }
}

fn d1_text(wm: &GameWindowManager, msg: u32, d1: WindowMsgData) -> String {
    if msg == ggm::GBM_SELECTED || msg == ggm::GLM_SELECTED {
        win_short(wm, data_win(d1))
    } else {
        d1.to_string()
    }
}

impl Host {
    fn d2_text(&self, msg: u32, d2: WindowMsgData) -> String {
        if msg == ggm::GBM_SELECTED {
            self.names.get(&d2).map_or(format!("?{d2}"), |n| short(n.as_bytes()))
        } else if msg == ggm::GLM_SELECTED {
            (d2 as i32).to_string()
        } else if msg == gwm::INPUT_FOCUS {
            "-".to_string()
        } else {
            d2.to_string()
        }
    }
    fn key_of(&mut self, name: &[u8]) -> u32 {
        let k = self.keys.name_to_key(name);
        self.names.insert(k, String::from_utf8_lossy(name).into_owned());
        k
    }
}

impl GuiHost for Host {
    fn system(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        let f = func.to_vec();
        if !MENUS.iter().any(|m| m.system == f.as_slice()) {
            return MsgHandled::Ignored;
        }
        self.log += &format!("sys {} {} {} {}\n", win_short(wm, Some(win)), msg_name(msg), d1_text(wm, msg, d1), self.d2_text(msg, d2));
        let r = match f.as_slice() {
            b"WOLMessageWindowSystem" => msgw::wol_message_window_system(wm, self, win, msg, d1, d2),
            b"WOLStatusMenuSystem" => stat::wol_status_menu_system(wm, self, win, msg, d1, d2),
            b"WOLCustomScoreScreenSystem" => cust::wol_custom_score_screen_system(wm, self, win, msg, d1, d2),
            b"WOLQMScoreScreenSystem" => qm::wol_qm_score_screen_system(wm, self, win, msg, d1, d2),
            b"WOLLadderScreenSystem" => lad::wol_ladder_screen_system(wm, self, win, msg, d1, d2),
            _ => loc::wol_locale_select_system(wm, self, win, msg, d1, d2),
        };
        self.log += &format!("= {} {}\n", msg_name(msg), u8::from(r == MsgHandled::Handled));
        r
    }
    fn input(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        let f = func.to_vec();
        if !MENUS.iter().any(|m| m.input == f.as_slice()) {
            return MsgHandled::Ignored;
        }
        self.log += &format!("inp {} {} {} {}\n", win_short(wm, Some(win)), msg_name(msg), d1, d2);
        let r = match f.as_slice() {
            b"WOLMessageWindowInput" => msgw::wol_message_window_input(wm, self, win, msg, d1, d2),
            b"WOLStatusMenuInput" => stat::wol_status_menu_input(wm, self, win, msg, d1, d2),
            b"WOLCustomScoreScreenInput" => cust::wol_custom_score_screen_input(wm, self, win, msg, d1, d2),
            b"WOLQMScoreScreenInput" => qm::wol_qm_score_screen_input(wm, self, win, msg, d1, d2),
            b"WOLLadderScreenInput" => lad::wol_ladder_screen_input(wm, self, win, msg, d1, d2),
            _ => loc::wol_locale_select_input(msg),
        };
        self.log += &format!("= {} {}\n", msg_name(msg), u8::from(r == MsgHandled::Handled));
        r
    }
}

impl ShellHost for Host {
    fn shell(&mut self) -> &mut Shell {
        self.shell_calls += 1;
        &mut self.shell
    }
    fn create_layout(&mut self, _wm: &mut GameWindowManager, filename: &[u8]) -> Option<usize> {
        if filename == b"Menus/BlankWindow.wnd" {
            self.log += "showShellMap 0\n";
        } else {
            self.log += &format!("doPush {}\n", String::from_utf8_lossy(filename));
        }
        None
    }
    fn create_from_script(&mut self, _wm: &mut GameWindowManager, _filename: &[u8]) -> Option<WinId> {
        None
    }
    fn layout_callback(&mut self, _wm: &mut GameWindowManager, func: &[u8], _layout: usize, _user: Option<bool>) {
        // the top screen's shutdown, run by `Shell::pop`: logged as the pop; its own `shutdownComplete` is not run (the C++ stand-in
        // logs the pop only), so the pop stays pending
        if MENUS.iter().any(|m| m.shutdown.as_bytes() == func) {
            self.log += "pop\n";
            self.shell.pending_pop = false;
        }
    }
    fn time_ms(&self) -> u32 {
        0
    }
    fn env(&self) -> ShellEnv {
        ShellEnv { shell_map_on: true, ..ShellEnv::default() }
    }
    fn request(&mut self, r: ShellRequest) {
        if r == ShellRequest::NewShellGame {
            self.log += "showShellMap 1\n";
        }
    }
    fn name_to_key(&mut self, name: &[u8]) -> u32 {
        self.key_of(name)
    }
}

impl msgw::WolMessageWindowHost for Host {
    fn wol_message_window(&mut self) -> &mut msgw::WolMessageWindow {
        &mut self.msgw
    }
}
impl stat::WolStatusMenuHost for Host {
    fn wol_status_menu(&mut self) -> &mut stat::WolStatusMenu {
        &mut self.stat
    }
}
impl cust::WolCustomScoreScreenHost for Host {
    fn wol_custom_score_screen(&mut self) -> &mut cust::WolCustomScoreScreen {
        &mut self.cust
    }
}
impl qm::WolQmScoreScreenHost for Host {
    fn wol_qm_score_screen(&mut self) -> &mut qm::WolQmScoreScreen {
        &mut self.qm
    }
}
impl lad::WolLadderScreenHost for Host {
    fn wol_ladder_screen(&mut self) -> &mut lad::WolLadderScreen {
        &mut self.lad
    }
    fn has_web_browser(&self) -> bool {
        self.has_web
    }
    fn web_browser_create_window(&mut self, name: &[u8], window: Option<WinId>) {
        // (the window's name is read in `run`, which has the manager: the host keeps the id)
        self.log += &format!("web.create {} @{}\n", String::from_utf8_lossy(name), window.map_or(-1, |w| w as i64));
    }
    fn web_browser_close_window(&mut self, window: Option<WinId>) {
        self.log += &format!("web.close @{}\n", window.map_or(-1, |w| w as i64));
    }
}
impl loc::WolLocaleSelectHost for Host {
    fn wol_locale_select(&mut self) -> &mut loc::WolLocaleSelect {
        &mut self.loc
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        b"T:".iter().chain(label).map(|&b| u16::from(b)).collect()
    }
    fn gamespy_color_default(&self) -> i32 {
        self.color
    }
    fn win_set_modal(&mut self, wm: &mut GameWindowManager, w: Option<WinId>) {
        wm.win_set_modal(w);
    }
    fn ps_add_update_player_locale_request(&mut self, locale: i32) {
        self.log += &format!("ps.addRequest 1 locale {locale} id {} me@example.org Nick pw\n", self.local_id);
    }
    fn gamespy_close_overlay_locale_select(&mut self, _wm: &mut GameWindowManager) {
        self.log += "closeOverlay 7\n";
    }
    fn misc_prefs_set_locale_and_write(&mut self, locale: i32) {
        self.log += &format!("prefs.setLocale {locale}\nprefs.write\n");
    }
    fn local_profile_id(&self) -> i32 {
        self.local_id
    }
    fn ps_find_player_stats_by_id(&self, _id: i32) -> loc::LocaleStats {
        loc::LocaleStats { id: if self.stats_kind == 0 { 0 } else if self.stats_kind == 1 { self.local_id } else { 777 }, locale: self.stats_locale }
    }
    fn ps_track_player_stats_locale(&mut self, stats: &loc::LocaleStats) {
        self.log += &format!("ps.track {} {}\n", stats.id, stats.locale);
    }
    fn gamespy_set_cached_local_player_stats_locale(&mut self, locale: i32) {
        self.log += &format!("cached.set 0 {locale}\n");
    }
    fn ps_add_player_stats_response_for_local_player(&mut self) {
        let s = self.ps_find_player_stats_by_id(self.local_id);
        self.log += &format!("ps.addResponse 1 {} {}\n", s.id, s.locale);
    }
    fn check_re_open_player_info(&mut self, _wm: &mut GameWindowManager) {
        self.log += "checkReopen\n";
    }
}

fn new_host(menu: usize, c: Option<&Case>, wnd_bytes: Vec<u8>) -> Host {
    Host {
        shell: Shell::with_display_width(800),
        shell_calls: 0,
        files: Some(LayoutFiles::new(Files { wnd: wnd_bytes })),
        keys: NameKeys::default(),
        names: BTreeMap::new(),
        log: String::new(),
        menu,
        msgw: Default::default(),
        stat: Default::default(),
        cust: Default::default(),
        qm: Default::default(),
        lad: Default::default(),
        loc: Default::default(),
        has_web: c.is_some_and(|c| c.has_web),
        color: c.map_or(0, |c| c.color),
        local_id: c.map_or(0, |c| c.local_id),
        stats_kind: c.map_or(0, |c| c.stats_kind),
        stats_locale: c.map_or(0, |c| c.stats_locale),
    }
}

/// How many `h.shell()` calls one `shutdownComplete` with nothing pending costs (measured, not assumed).
fn calls_per_shutdown_complete() -> usize {
    let mut h = new_host(0, None, wnd(&MENUS[0], 3, false));
    let mut wm = GameWindowManager::new((800, 600));
    shell::shutdown_complete(&mut wm, &mut h, None, false);
    h.shell_calls
}

const MSGS: [u32; 8] = [gwm::CREATE, gwm::DESTROY, gwm::CHAR, gwm::INPUT_FOCUS, ggm::GBM_SELECTED, ggm::GEM_EDIT_DONE, ggm::GLM_SELECTED, gwm::LEFT_DOWN];

/// The Rust run: the log text and the first hidden state of the layout.
fn rust_run(c: &Case, k: usize) -> (String, bool, String) {
    let m = &MENUS[c.menu];
    let mut h = new_host(c.menu, Some(c), wnd(m, c.mask, c.no_focus));
    let mut wm = GameWindowManager::new((800, 600));
    let mut f = h.files.take().expect("files");
    let layout = {
        let mut kf = KeyedFiles { files: &mut f.files, keys: h.keys.clone() };
        shell::win_create_layout_with(&mut f.script, &mut kf, &mut wm, &mut h, b"Menus/x.wnd").expect("the layout")
    };
    h.files = Some(f);
    for (_, n) in m.wins {
        h.key_of(format!("{}:{n}", m.file).as_bytes());
    }
    // (the reader's own messages, `GWM_CREATE` of each window, are not the draft's)
    h.log.clear();
    wm.undefined = None;
    let layout_hidden0 = wm.layouts[layout].hidden;
    if c.stack_top {
        h.shell.screen_stack.push(layout);
    }
    if c.pending_push {
        h.shell.pending_push = true;
        h.shell.pending_push_name = b"Menus/Next.wnd".to_vec();
    }
    let parent = wm.tree.find_by_name(format!("{}:{}", m.file, m.wins[0].1).as_bytes());
    let ws: Vec<Option<WinId>> = m.wins.iter().map(|(_, n)| wm.tree.find_by_name(format!("{}:{n}", m.file).as_bytes())).collect();
    let mut out = String::new();
    for op in &c.ops {
        out += &format!("> {} {} {} {} {}\n", op[0], op[1], op[2], op[3], op[4]);
        let pending0 = h.shell.pending_push;
        let calls0 = h.shell_calls;
        let msg = MSGS[(op[1] & 7) as usize];
        match op[0] {
            1 => match c.menu {
                0 => msgw::wol_message_window_init(&mut wm, &mut h, layout),
                1 => stat::wol_status_menu_init(&mut wm, &mut h, layout),
                2 => cust::wol_custom_score_screen_init(&mut wm, &mut h, layout),
                3 => qm::wol_qm_score_screen_init(&mut wm, &mut h, layout),
                4 => lad::wol_ladder_screen_init(&mut wm, &mut h, layout),
                _ => loc::wol_locale_select_init(&mut wm, &mut h, layout),
            },
            2 => {
                match c.menu {
                    0 => msgw::wol_message_window_shutdown(&mut wm, &mut h, layout),
                    1 => stat::wol_status_menu_shutdown(&mut wm, &mut h, layout),
                    2 => cust::wol_custom_score_screen_shutdown(&mut wm, &mut h, layout),
                    3 => qm::wol_qm_score_screen_shutdown(&mut wm, &mut h, layout),
                    4 => lad::wol_ladder_screen_shutdown(&mut wm, &mut h, layout),
                    _ => loc::wol_locale_select_shutdown(&mut wm, &mut h, layout),
                }
                if !pending0 {
                    let n = (h.shell_calls - calls0) / k;
                    for _ in 0..n {
                        h.log += "shutdownComplete\n";
                    }
                }
            }
            3 => match c.menu {
                0 => msgw::wol_message_window_update(&mut wm, &mut h, layout),
                1 => stat::wol_status_menu_update(&mut wm, &mut h, layout),
                2 => cust::wol_custom_score_screen_update(&mut wm, &mut h, layout),
                3 => qm::wol_qm_score_screen_update(&mut wm, &mut h, layout),
                4 => lad::wol_ladder_screen_update(&mut wm, &mut h, layout),
                _ => loc::wol_locale_select_update(&mut wm, &mut h, layout),
            },
            4 => {
                let r = wm.win_send_input_msg(&mut h, parent, msg, op[2] as u32, op[3] as u32);
                h.log += &format!("ret {}\n", u8::from(r == MsgHandled::Handled));
            }
            5 => {
                let (mut d1, mut d2) = (op[2] as u32, op[3] as u32);
                if msg == ggm::GBM_SELECTED || msg == ggm::GLM_SELECTED {
                    let ctl = if (0..3).contains(&op[2]) { ws.get(op[2] as usize).copied().flatten() } else { None };
                    d1 = win_data(ctl);
                    if msg == ggm::GBM_SELECTED {
                        d2 = h.key_of(format!("{}:{}", m.file, m.wins[(op[3] % 3) as usize].1).as_bytes());
                    } else {
                        d2 = op[3] as u32;
                    }
                }
                wm.focus_reply = false;
                let r = wm.win_send_system_msg(&mut h, parent, msg, d1, d2);
                h.log += &format!("ret {}\n", u8::from(r == MsgHandled::Handled));
                if msg == gwm::INPUT_FOCUS {
                    h.log += &format!("reply {}\n", u8::from(wm.focus_reply));
                }
            }
            _ => {
                wm.win_set_focus(&mut h, if op[2] < 0 { None } else { parent });
            }
        }
        wm.undefined = None;
        // the ladder's window ids in the web log, named
        let log = std::mem::take(&mut h.log);
        let mut named = String::new();
        for line in log.lines() {
            if let Some(i) = line.rfind(" @") {
                let id: i64 = line[i + 2..].parse().expect("window id");
                let n = if id < 0 { "-".to_string() } else { win_short(&wm, Some(id as usize)) };
                named += &format!("{}{}\n", &line[..i], format!(" {n}"));
            } else {
                named += line;
                named += "\n";
            }
        }
        out += &named;
        // the state
        for (i, (_, n)) in m.wins.iter().enumerate() {
            match ws[i] {
                None => out += &format!("win {n} absent\n"),
                Some(w) => {
                    let st = wm.tree.windows[w].status;
                    out += &format!("win {n} hidden {} enabled {}\n", u8::from(st & win_status::HIDDEN != 0), u8::from(st & win_status::ENABLED != 0));
                }
            }
        }
        out += &format!("focus {} modal {} layout {}\n", win_short(&wm, wm.keyboard_focus), win_short(&wm, wm.modal.first().copied()), u8::from(wm.layouts[layout].hidden));
        if c.menu == 5 {
            let (mut rows, mut sel, mut cells) = (0, -1, String::new());
            if let Some(l) = ws[2] {
                rows = lb::get_num_entries(&wm, l);
                sel = lb::get_selected(&mut wm, &mut h, l).first().copied().unwrap_or(-1);
                for r in 0..rows {
                    let (t, col) = lb::get_text_and_color(&mut wm, &mut h, l, r, 0);
                    cells += &format!(" [{}]{{{col}}}", support_ui3a::narrow(&t));
                }
            }
            out += &format!("list {rows} sel {sel}{cells}\n");
            // (the list's GLM_SELECTED traffic reached the log above)
        }
        // the screen's statics, the ids by the name they were asked for ("?0": never asked)
        let id = |h: &Host, i: i32| h.names.get(&(i as u32)).map_or(format!("?{i}"), |n| short(n.as_bytes()));
        let w = |w: Option<WinId>| win_short(&wm, w);
        out += &match c.menu {
            0 => format!("state {} {} {} {}\n", id(&h, h.msgw.parent_wol_message_window_id), w(h.msgw.parent_wol_message_window), id(&h, h.msgw.button_cancel_id), w(h.msgw.button_cancel)),
            1 => format!("state {} {} {} {} {}\n", id(&h, h.stat.parent_wol_status_id), id(&h, h.stat.button_disconnect_id), w(h.stat.parent_wol_status), w(h.stat.button_disconnect), w(h.stat.progress_text_window)),
            2 => format!("state {} {} {} {} {} {}\n", id(&h, h.cust.parent_wol_custom_score_id), id(&h, h.cust.button_disconnect_id), id(&h, h.cust.button_lobby_id), w(h.cust.parent_wol_custom_score), w(h.cust.button_disconnect), w(h.cust.button_lobby)),
            3 => format!("state {} {} {} {} {} {}\n", id(&h, h.qm.parent_wol_qm_score_id), id(&h, h.qm.button_disconnect_id), id(&h, h.qm.button_quickmatch_id), w(h.qm.parent_wol_qm_score), w(h.qm.button_disconnect), w(h.qm.button_quickmatch)),
            4 => format!("state {} {} {} {} {} {}\n", id(&h, h.lad.parent_window_id), id(&h, h.lad.button_back_id), id(&h, h.lad.window_ladder_id), w(h.lad.parent_window), w(h.lad.button_back), w(h.lad.window_ladder)),
            _ => format!("state {} {} {} {} {} {}\n", id(&h, h.loc.parent_locale_select_id), id(&h, h.loc.button_ok_id), id(&h, h.loc.listbox_locale_id), w(h.loc.parent_locale_select), w(h.loc.button_ok), w(h.loc.listbox_locale)),
        };
        let _ = h.menu;
    }
    (out, layout_hidden0, String::new())
}

fn encode(c: &Case, layout_hidden0: bool) -> Vec<i32> {
    let mut v = vec![
        c.menu as i32,
        c.mask,
        i32::from(c.no_focus),
        i32::from(layout_hidden0),
        i32::from(c.pending_push),
        i32::from(c.has_web),
        c.color,
        c.local_id,
        c.stats_kind,
        c.stats_locale,
        i32::from(c.stack_top),
        LIST_LENGTH,
        c.ops.len() as i32,
    ];
    for op in &c.ops {
        v.extend_from_slice(op);
    }
    v
}

fn cpp_run(v: &[i32]) -> String {
    let mut out = vec![0u8; 1 << 22];
    let n = unsafe { ref_u3a_small_run(v.as_ptr(), v.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!((n as usize) <= out.len(), "the log overflowed");
    String::from_utf8_lossy(&out[..n as usize]).into_owned()
}

fn gen_case(r: &mut Rng) -> Case {
    let menu = r.below(6) as usize;
    let count = MENUS[menu].wins.len() as u64;
    let mut mask = if r.below(4) == 0 { r.below(1 << count) as i32 } else { (1 << count) - 1 };
    if r.below(12) == 0 {
        mask &= !1;
    }
    if mask == 0 {
        mask = 2;
    }
    if menu == 4 {
        // Escape's release to the ladder sends GBM_SELECTED with *Back*, which upstream dereferences (NULL if the window is missing)
        mask |= 2;
    }
    let present = |i: i32| mask & (1 << i) != 0;
    // the first op is `Init`: upstream's statics (the window pointers) are the cut's own, left by the previous case in this process, so every
    // case sets them first (callbacks before `Init`, on NULL statics, are not compared)
    let mut ops = vec![[1, 0, 0, 0, 0]];
    for _ in 0..1 + r.below(24) {
        let kind = r.below(100);
        let mut op = [0i32; 5];
        if kind < 16 {
            op[0] = 1;
        } else if kind < 28 {
            op[0] = 2;
        } else if kind < 34 {
            op[0] = 3;
        } else if kind < 54 {
            op[0] = 4;
            op[1] = if r.below(5) == 0 { [7, 3, 1, 4][r.below(4) as usize] } else { 2 };
            op[2] = match r.below(10) {
                0..=3 => 0x01,
                4 => 0x101,
                5 => 0x02,
                6 => 0x1C,
                _ => r.below(256) as i32,
            };
            op[3] = match r.below(8) {
                0 => 0x02,
                1 => 0x01,
                2 => 0x102,
                3 => 0x03,
                4 => 0,
                _ => r.below(8) as i32,
            };
        } else if kind < 90 {
            op[0] = 5;
            op[1] = r.below(8) as i32;
            match op[1] {
                4 | 6 => {
                    op[2] = r.below(4) as i32 - 1;
                    if (menu == 4 || menu == 5) && op[1] == 4 && !(op[2] >= 0 && present(op[2])) {
                        // a NULL control (or an absent window): excluded (upstream dereferences it)
                        op[2] = if present(1) { 1 } else if present(0) { 0 } else { 2 };
                        if !present(op[2]) {
                            op[1] = 0;
                        }
                    }
                    op[3] = if op[1] == 4 { r.below(count) as i32 } else { r.below(40) as i32 - 3 };
                    if menu == 5 && op[1] == 4 && !present(2) && op[2] == 1 {
                        // OK with no list box: excluded
                        op[1] = 5;
                    }
                }
                3 => {
                    op[2] = [0, 1, 1, 2][r.below(4) as usize];
                    op[3] = r.below(3) as i32;
                }
                _ => {
                    op[2] = r.below(3) as i32;
                    op[3] = r.below(3) as i32;
                }
            }
        } else {
            op[0] = 6;
            op[2] = if r.below(3) == 0 { -1 } else { 0 };
        }
        ops.push(op);
    }
    Case {
        menu,
        mask,
        no_focus: r.below(7) == 0,
        layout_hidden0: false,
        pending_push: r.below(5) == 0,
        has_web: r.below(10) < 7,
        color: if r.below(4) == 0 { -(r.below(1000) as i32) } else { 0x0100_0000 + r.below(0xFFFFFF) as i32 },
        local_id: [0, 5, 12][r.below(3) as usize],
        stats_kind: r.below(3) as i32,
        stats_locale: r.below(40) as i32,
        stack_top: r.below(10) < 7,
        ops,
    }
}

#[test]
fn the_small_wol_screens_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("DRAFT_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let k = calls_per_shutdown_complete();
    assert!(k > 0, "the host's shell() count per shutdownComplete");
    println!("host shell() calls per shutdownComplete: {k}");
    let mut r = Rng(0x215_0A_0001);
    let mut per_menu = [0usize; 6];
    let mut bad_menu = [0usize; 6];
    let mut mismatches = 0usize;
    let mut ops = 0usize;
    for n in 0..cases {
        let mut c = gen_case(&mut r);
        if std::env::var("DRAFT_TRACE").is_ok() {
            eprintln!("case {n}: {c:?}");
        }
        let (rust, hidden0, _) = rust_run(&c, k);
        c.layout_hidden0 = hidden0;
        let cpp = cpp_run(&encode(&c, hidden0));
        per_menu[c.menu] += 1;
        if std::env::var("DRAFT_SHOW").ok().and_then(|v| v.parse::<usize>().ok()) == Some(c.menu) && c.ops.iter().any(|o| o[0] == 2) && !c.pending_push && per_menu[c.menu] < 40 {
            println!("SHOW {c:?}\n--- rust\n{rust}--- c++\n{cpp}");
        }
        ops += c.ops.len();
        if cpp != rust {
            mismatches += 1;
            bad_menu[c.menu] += 1;
            if mismatches <= 2 {
                let at = rust.lines().zip(cpp.lines()).position(|(x, y)| x != y).unwrap_or(rust.lines().count().min(cpp.lines().count()));
                let (rl, cl) = (rust.lines().nth(at).unwrap_or(""), cpp.lines().nth(at).unwrap_or(""));
                let col = rl.chars().zip(cl.chars()).position(|(x, y)| x != y).unwrap_or(rl.len().min(cl.len()));
                let win = |s: &str| s.chars().skip(col.saturating_sub(60)).take(160).collect::<String>();
                println!(
                    "case {n}: {c:?}\n  first difference at line {at}, column {col}:\n  rust: {:?}\n  c++:  {:?}\n  before: {:?}",
                    win(rl),
                    win(cl),
                    rust.lines().skip(at.saturating_sub(5)).take(5).map(|l| l.chars().take(100).collect::<String>()).collect::<Vec<_>>().join(" | ")
                );
            }
        }
    }
    println!("DIFF WOL small screens compared {cases} mismatches {mismatches} ({ops} ops; cases per screen {per_menu:?}; mismatching per screen {bad_menu:?})");
    if planted {
        println!("MEASURED mismatches {mismatches}");
    }
    assert_eq!(mismatches, 0);
}
