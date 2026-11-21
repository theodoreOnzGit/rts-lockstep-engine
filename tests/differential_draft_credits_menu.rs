// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui3b) of the draft `src/drafts/ui/credits_menu.rs` (`credits_menu_init`, `_shutdown`, `_update`, `_input`, `_system`) against
//! upstream `CreditsMenu.cpp`, cut verbatim by build.rs (`drafts_ui3b`; `differential/wrap_draft_credits_menu.cpp`, whose stand-ins, the window manager's functions cut
//! verbatim from GameWindowManager.cpp / GameWindow.cpp among them, and exclusion are listed there). This is not `credits.rs` (the credits manager's lines, tested by
//! `differential_draft_credits.rs`).
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides). A tree of 2 to 7 windows (ids: the credits parent window, decoys; duplicates; random hidden, enabled, no-focus bits; a
//! third with the menu's own system callback), a layout of a random subset of them, `TheCredits` present or not, finished or not; 1 to 12 steps of `CreditsMenuInit`,
//! `CreditsMenuShutdown` (only while `TheCredits` exists: upstream dereferences it), `CreditsMenuUpdate`, `CreditsMenuInput` (Escape with and without the key-up state, other keys,
//! other messages), `CreditsMenuSystem` (create, destroy, `GWM_INPUT_FOCUS` with a Bool, `GBM_SELECTED`, others), a change of the credits' finished flag.
//! The shell is the verified one on the Rust side; to see which of its functions the draft called, the host is set up so that each is observable as one word: the shell map is on and
//! the shell active with no background, so `showShellMap(TRUE)` requests a new shell game (word 21) and `showShellMap(FALSE)` makes the blank background (word 20); the credits layout is
//! the top of the shell's stack with a named shutdown, so `pop()` runs it (word 22); a push is pending before each step, so `shutdownComplete()` makes the pending screen (word 23). The C++
//! stand-in logs the same words at the call. After every step those words, the credits manager's own calls, the audio calls, the result, the Bool of `GWM_INPUT_FOCUS`, every window's
//! status word, the focus, the modal stack, the statics and the layout's hidden flag are compared.
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! 0 mismatches.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 21201 | `CreditsMenuInit` shows the shell map (`showShellMap(TRUE)`) |
//! | 21202 | the menu does not take the keyboard focus |
//! | 21203 | the layout is hidden at init |
//! | 21204 | `CreditsMenuShutdown` does not complete the shutdown |
//! | 21205 | the update pops when the credits are NOT finished |
//! | 21206 | the update does not pop when the credits are gone |
//! | 21207 | Escape pops on the key-down |
//! | 21208 | the menu says it does not want the keyboard focus |
//! | 21209 | an unknown message is reported handled |
//! | 21210 | the credits music is not started |
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=20000` (the default): **0 mismatches over 20,000 sequences** after the fixes noted here, max ULP 0 (nothing is floating point in the comparison, which is of integer words). No draft change was needed. Prediction held.
//! Planted bugs (of 20,000): 21201 14,581; 21202 11,754; 21203 14,581; 21204 9,440; 21205 7,870; 21206 5,223; 21207 8,169; 21208 5,385; 21209 8,503; 21210 14,581. **10 of 10 caught.**
//! The `DIFF` line is printed by the run.

mod support;
mod ui3b_common;

use rts_lockstep_engine::client::shell::{Shell, ShellEnv, ShellHost, ShellRequest};
use rts_lockstep_engine::client::window_manager::{gwm, win_data, GameWindowManager, GuiHost, MsgHandled, WindowMsgData};
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::drafts::ui::credits_menu::*;
use std::sync::Mutex;
use support::{Check, Gen};
use ui3b_common::*;

extern "C" {
    fn ref_ui3b_cr_run(input: *const u32, n: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

struct Host {
    log: Vec<u32>,
    shell: Shell,
    menu: CreditsMenu,
    has_credits: bool,
    finished: bool,
}

impl GuiHost for Host {
    fn system(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == b"credits" {
            credits_menu_system(wm, self, win, msg, d1, d2)
        } else {
            focus_system(&mut self.log, wm, win, msg, d1)
        }
    }
}

impl ShellHost for Host {
    fn shell(&mut self) -> &mut Shell {
        &mut self.shell
    }
    fn create_layout(&mut self, _wm: &mut GameWindowManager, filename: &[u8]) -> Option<usize> {
        match filename {
            b"Menus/BlankWindow.wnd" => self.log.push(20),
            b"pushed" => self.log.push(23),
            _ => panic!("an unexpected layout"),
        }
        None
    }
    fn create_from_script(&mut self, _wm: &mut GameWindowManager, _filename: &[u8]) -> Option<WinId> {
        None
    }
    fn layout_callback(&mut self, _wm: &mut GameWindowManager, func: &[u8], _layout: usize, _user: Option<bool>) {
        assert_eq!(func, b"shutdown");
        self.log.push(22);
    }
    fn time_ms(&self) -> u32 {
        0
    }
    fn env(&self) -> ShellEnv {
        ShellEnv { shell_map_on: true, ..ShellEnv::default() }
    }
    fn request(&mut self, r: ShellRequest) {
        assert_eq!(r, ShellRequest::NewShellGame);
        self.log.push(21);
    }
    fn name_to_key(&mut self, name: &[u8]) -> u32 {
        key_of(name) as u32
    }
}

impl CreditsHost for Host {
    fn credits_menu(&mut self) -> &mut CreditsMenu {
        &mut self.menu
    }
    fn has_credits(&self) -> bool {
        self.has_credits
    }
    fn credits_create_load_init(&mut self) {
        if self.has_credits {
            self.log.push(35);
        }
        self.has_credits = true;
        self.log.extend([30, 31, 32]);
    }
    fn credits_update(&mut self) {
        self.log.push(33);
    }
    fn credits_is_finished(&self) -> bool {
        self.finished
    }
    fn credits_reset_and_delete(&mut self) {
        self.log.extend([34, 35]);
        self.has_credits = false;
    }
    fn audio_remove_stop_the_music_fade(&mut self) {
        self.log.push(40);
    }
    fn audio_add_credits_music_with_fade(&mut self) {
        self.log.extend([41, 1]);
    }
}

#[test]
fn credits_menu_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_B3_0A);
    let mut chk = Check::new("CreditsMenu callbacks");
    let parent_key = key_of(b"CreditsMenu.wnd:ParentCreditsWindow");
    let ids = [parent_key, parent_key, 7001, 7002];
    for _ in 0..cases(20_000) {
        let n = 2 + g.below(6) as usize;
        let ts = random_tree(&mut g, n, &ids, 3);
        let (mut wm, scene) = make_tree_scene(&ts, b"credits");
        let members: Vec<usize> = (0..n).filter(|_| g.below(3) != 0).collect();
        let layout = make_layout(&mut wm, &members);
        wm.layouts[layout].shutdown = Some(b"shutdown".to_vec());
        let mut input = scene;
        input.push(members.len() as u32);
        input.extend(members.iter().map(|&m| m as u32));
        let has_credits = g.below(2) == 0;
        let finished = g.below(2) == 0;
        input.extend([u32::from(has_credits), u32::from(finished)]);
        let mut host = Host { log: Vec::new(), shell: Shell::new(), menu: CreditsMenu::default(), has_credits, finished };
        host.shell.screen_stack = vec![layout];
        host.shell.is_shell_active = true;
        let steps = 1 + g.below(12);
        input.push(steps);
        let mut rust = Vec::new();
        let mut model_credits = has_credits;
        for _ in 0..steps {
            let mut kind = g.below(7);
            if kind == 1 && !model_credits {
                kind = 0;
            }
            let (kind, a, b, c, d) = match kind {
                0 => (0, 0, 0, 0, 0),
                1 => (1, 0, 0, 0, 0),
                2 => (2, 0, 0, 0, 0),
                3 => {
                    let key = match g.below(3) {
                        0 | 1 => 1u32,
                        _ => g.below(256),
                    };
                    let state = if g.below(5) == 0 { g.below(256) } else { [1u32, 2][g.below(2) as usize] | if g.below(4) == 0 { 0x100 } else { 0 } };
                    let k = if g.below(8) == 0 { key | g.below(0x1000) << 8 } else { key };
                    let msg = if g.below(8) == 0 { g.below(30) } else { gwm::CHAR };
                    (3, g.below(n as u32), msg, k, state)
                }
                4 | 5 => {
                    let msg = match g.below(6) {
                        0 => gwm::CREATE,
                        1 => gwm::DESTROY,
                        2 => gwm::INPUT_FOCUS,
                        3 => 16392,
                        _ => g.below(40),
                    };
                    let c = if msg == 16392 { 1 + g.below(n as u32) } else if msg == gwm::INPUT_FOCUS { g.below(3) } else { g.below(5) };
                    (4, g.below(n as u32), msg, c, g.below(2))
                }
                _ => (5, g.below(2), 0, 0, 0),
            };
            input.extend([kind, a, b, c, d]);
            host.log.clear();
            host.shell.pending_push = true;
            host.shell.pending_push_name = b"pushed".to_vec();
            host.shell.pending_pop = false;
            host.shell.clear_background = false;
            let w = a as usize;
            let mut res: i32 = -1;
            match kind {
                0 => {
                    credits_menu_init(&mut wm, &mut host, layout);
                    model_credits = true;
                }
                1 => {
                    credits_menu_shutdown(&mut wm, &mut host, layout);
                    model_credits = false;
                }
                2 => credits_menu_update(&mut wm, &mut host, layout),
                3 => res = i32::from(credits_menu_input(&mut wm, &mut host, w, b, c, d) == MsgHandled::Handled),
                4 => {
                    let d1 = if b == 16392 { win_data(Some(c as usize - 1)) } else { c };
                    wm.focus_reply = d != 0;
                    res = i32::from(credits_menu_system(&mut wm, &mut host, w, b, d1, d) == MsgHandled::Handled);
                    if b == gwm::INPUT_FOCUS {
                        res = res * 2 + i32::from(wm.focus_reply);
                    }
                }
                _ => host.finished = a != 0,
            }
            rust.extend(host.log.iter().copied());
            rust.extend([100, res as u32]);
            put_state_tree(&wm, n, &mut rust);
            rust.extend([host.menu.parent_main_menu_id as u32, host.menu.parent_main_menu.map_or(0, |p| p as u32 + 1), u32::from(host.has_credits), u32::from(host.finished), u32::from(wm.layouts[layout].hidden)]);
        }
        trace(&input);
        let mut buf = vec![0u32; 1 << 15];
        let len = unsafe { ref_ui3b_cr_run(input.as_ptr(), input.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(len >= 0, "output buffer too small");
        let cpp = buf[..len as usize].to_vec();
        chk.eq(rust.clone(), cpp.clone(), || format!("{} input {input:?}", diff_note(&rust, &cpp)));
    }
    finish(chk);
}
