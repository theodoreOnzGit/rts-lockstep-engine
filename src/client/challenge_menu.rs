// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/ChallengeMenu.cpp (the file's statics and
//     globals, findPositionButton, setEnabledButtons, setGeneralCampaign (the shell's half), setGeneralBio,
//     updateButtonSequence, updateBio, ChallengeMenuInit, ChallengeMenuUpdate, ChallengeMenuShutdown,
//     ChallengeMenuInput, ChallengeMenuSystem)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   ChallengeMenu.cpp: Steve Copeland, May 2003
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The Generals' Challenge's general select (GitHub #98): `Menus/ChallengeMenu.wnd`, which the main menu pushes once
//! a difficulty is chosen after *Challenge* (`setupGameStart` with `launchChallengeMenu`).
//!
//! # As upstream reads it
//!
//! **Init**: `TheChallengeGameInfo` made if there is none and set up (`init`, `clearSlotList`, `reset`, `enterGame`);
//! the shell map shown; the windows found under `ParentChallengeMenu` (*Play*, *Back*, the bio's portrait and its four
//! lines, `MainBackdrop`, `GeneralsBioParent`) and the twelve general buttons `GeneralPosition0` .. `11` (check boxes),
//! each hidden; the bio and *Play* hidden; `setEnabledButtons`: each general's button enabled and shown when the
//! general starts enabled (`ChallengeMode.ini`), and given its player template's medallions (the normal one as its
//! image, sized to the image's width both ways; the selected one as the hilited and disabled unchecked box; the
//! hilite one as the hilited image). The layout shown, the focus on the parent, `GadgetParent` hidden.
//!
//! **Update**: a frame later `ChallengeMenuFade` set; ten updates after the transitions are done the announcer's
//! `Taunts_GCAnnouncer01` (once); the bio typed out two characters an update (`updateBio(TELETYPE_SKIP)`); a shutdown
//! completes once the animations and transitions are done.
//!
//! **System**: the mouse entering a general's button (other than the selected one) shows its bio
//! (`setGeneralBio`: the bio window shown, the small portrait, the four lines `getBioName`, `getBioRank`,
//! `getBioBranch`, `getBioStrategy` from the game text, typed out from empty) and plays `GUILogoMouseOver`; leaving it
//! shows the selected general's bio again. A general's button pressed (`GBM_SELECTED`) un-checks the one selected
//! before (a toggle whose own `GBM_SELECTED` is swallowed: `isAutoSelecting`), plays the general's preview sound,
//! remembers it and shows *Play*. *Play*: unless `TheChallengeGameInfo` is gone, `setGeneralCampaign` (the general's
//! campaign, `TheChallengeGenerals`' player template, `TheChallengeGameInfo`'s slot 0 the local player with it), the
//! pending file and `TheChallengeGameInfo`'s map the campaign's map; the selected button un-checked; a game in progress
//! cleared; the challenge's difficulty for the campaign and the script engine; `MSG_NEW_GAME` (`GAME_SINGLE_PLAYER`,
//! the difficulty, the rank points, the FPS limit 30); `InitRandom(0)`. *Back* pops the menu. **Input**: Escape's
//! release presses *Back*. **Shutdown**: the selection and the button sequence forgotten; immediate, done at once;
//! else `ChallengeMenuFade` reversed, `TheChallengeGameInfo` deleted, the sounds stopped, the announcer's count reset.
//!
//! # Upstream quirks that are kept
//!
//! - The general buttons are check boxes driven as a radio group by hand: the previous one is toggled (not
//!   un-checked), and the toggle's own `GBM_SELECTED` is swallowed. Pressing the selected general again toggles it back
//!   on.
//! - *Play* stays shown after a start: pressed again (no general selected, `lastButtonIndex` -1),
//!   `setGeneralCampaign(-1)` returns at once and `buttonGeneralPositionID[-1]` is read out of the array (recorded as
//!   undefined).
//! - `winSetSize(width, width)`: the medallion's width both ways.
//! - An immediate shutdown does not reset the announcer's count: after one, a count already at 10 goes past it and the
//!   announcer is never heard again (until a shutdown that is not immediate).
//! - `lastButtonIndex` is reset by the shutdown, not by the init.
//! - The windows used without a check (the general buttons, the bio's parent and portrait, *Play*) are recorded as
//!   undefined when missing ([`GameWindowManager::set_undefined`]); `DEBUG_ASSERTCRASH` is compiled out of the release.
//! - `updateButtonSequence` (the unfinished intro) is translated, and as upstream never called (its call is commented
//!   out).
//!
//! # Where this differs from upstream
//!
//! - **The start** (`setGeneralCampaign`, the pending file, `TheChallengeGameInfo`'s map, `clearGameData`, the
//!   difficulty, `MSG_NEW_GAME`, `InitRandom(0)`) is the logic's: a [`ShellRequest::StartChallenge`] with the general
//!   and the challenge's difficulty, which the client runs as the World's `start_challenge` (`campaign_world.rs`, the
//!   same calls in the same order, `TheChallengeGameInfo` made there) through the campaign panel's flow. The shell's
//!   `TheChallengeGameInfo` is only whether it exists ([`ChallengeMenu::challenge_game_info`]), which *Play* checks.
//! - **`TheChallengeGenerals->getCurrentDifficulty()`** is the main menu's difficulty (`setupGameStart` gave it to
//!   both; [`crate::client::main_menu::MainMenu::difficulty`]).
//! - `TheChallengeGenerals` and `ThePlayerTemplateStore` are the owner's copy of the World's
//!   ([`crate::client::gui_util::SkirmishData`]: `challenge`, `templates` with the medallions' names).
//! - `WindowVideoManager` (the portraits' Bink movies) is not translated: Bink was never released. Its `init`,
//!   `update` and `delete` do nothing. `TheAudio->removeAudioEvent` of the handles is the host's audio (not recorded).
//! - `bioLine1Readout` .. `4` and `bioLine` globals other files never read are not kept but for the four lines.
//!
//! # Representation
//!
//! [`ChallengeMenu`]: the file's statics and globals. The window ids are the host's name keys.

use crate::client::challenge_generals::NUM_GENERALS;
use crate::client::gadget;
use crate::client::gui_util::GuiUtilHost;
use crate::client::shell::{self, ShellRequest};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::{win_status, WinId};
use crate::client::window_transitions as wt;

/// `TELETYPE_SKIP`.
pub const TELETYPE_SKIP: i32 = 2;
/// `DEFAULT_GENERAL` (unused upstream).
pub const DEFAULT_GENERAL: i32 = 0;

/// What the challenge menu needs besides the skirmish screens' host (the World's copy, the images).
pub trait ChallengeHost: GuiUtilHost {
    /// ChallengeMenu.cpp's statics.
    fn challenge_menu(&mut self) -> &mut ChallengeMenu;
}

/// ChallengeMenu.cpp's statics and globals (`NAMEKEY_INVALID` is 0).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChallengeMenu {
    pub parent_id: i32,
    pub button_play_id: i32,
    pub button_back_id: i32,
    pub bio_portrait_id: i32,
    pub bio_name_entry_id: i32,
    pub bio_dob_entry_id: i32,
    pub bio_birthplace_entry_id: i32,
    pub bio_strategy_entry_id: i32,
    pub button_general_position_id: [i32; NUM_GENERALS],
    pub backdrop_id: i32,
    pub bio_parent_id: i32,
    pub parent_menu: Option<WinId>,
    pub button_play: Option<WinId>,
    pub button_back: Option<WinId>,
    pub bio_portrait: Option<WinId>,
    pub bio_line1_entry: Option<WinId>,
    pub bio_line2_entry: Option<WinId>,
    pub bio_line3_entry: Option<WinId>,
    pub bio_line4_entry: Option<WinId>,
    pub button_general_position: [Option<WinId>; NUM_GENERALS],
    pub backdrop: Option<WinId>,
    pub bio_parent: Option<WinId>,
    pub initial_gadget_delay: i32,
    pub just_entered: bool,
    pub is_shutting_down: bool,
    pub last_button_index: i32,
    /// `lastHilitedIndex` (a global).
    pub last_hilited_index: i32,
    /// `isAutoSelecting` (a global).
    pub is_auto_selecting: bool,
    pub bio_line1: Vec<u16>,
    pub bio_line2: Vec<u16>,
    pub bio_line3: Vec<u16>,
    pub bio_line4: Vec<u16>,
    pub bio_text_position: i32,
    pub bio_total_length: i32,
    pub button_sequence_step: i32,
    pub intro_audio_magic_number: i32,
    pub has_played_intro_audio: bool,
    /// `TheChallengeGameInfo != NULL` (its contents are the World's: see the module docs).
    pub challenge_game_info: bool,
}

impl Default for ChallengeMenu {
    fn default() -> Self {
        ChallengeMenu {
            parent_id: 0,
            button_play_id: 0,
            button_back_id: 0,
            bio_portrait_id: 0,
            bio_name_entry_id: 0,
            bio_dob_entry_id: 0,
            bio_birthplace_entry_id: 0,
            bio_strategy_entry_id: 0,
            button_general_position_id: [0; NUM_GENERALS],
            backdrop_id: 0,
            bio_parent_id: 0,
            parent_menu: None,
            button_play: None,
            button_back: None,
            bio_portrait: None,
            bio_line1_entry: None,
            bio_line2_entry: None,
            bio_line3_entry: None,
            bio_line4_entry: None,
            button_general_position: [None; NUM_GENERALS],
            backdrop: None,
            bio_parent: None,
            initial_gadget_delay: 2,
            just_entered: false,
            is_shutting_down: false,
            last_button_index: -1,
            last_hilited_index: -1,
            is_auto_selecting: false,
            bio_line1: Vec::new(),
            bio_line2: Vec::new(),
            bio_line3: Vec::new(),
            bio_line4: Vec::new(),
            bio_text_position: 0,
            bio_total_length: 0,
            button_sequence_step: 0,
            intro_audio_magic_number: 0,
            has_played_intro_audio: false,
            challenge_game_info: false,
        }
    }
}

/// A window upstream uses without a check: `None` records the `NULL` call as undefined.
fn must(wm: &mut GameWindowManager, w: Option<WinId>, what: &'static str) -> Option<WinId> {
    if w.is_none() {
        wm.set_undefined(what);
    }
    w
}

/// `findPositionButton(controlID)`: the general button's index, or -1.
pub fn find_position_button<H: ChallengeHost>(h: &mut H, control_id: i32) -> i32 {
    let ids = h.challenge_menu().button_general_position_id;
    ids.iter().position(|&id| id == control_id).map_or(-1, |i| i as i32)
}

/// `TheMappedImageCollection->findImageByName(name)` (`None`: `NULL`).
fn find_image<H: ChallengeHost>(h: &H, name: &[u8]) -> Option<Vec<u8>> {
    h.find_mapped_image(name)
}

/// The medallion names of general `i`'s player template (`getTemplateNumByName`, `getNthPlayerTemplate`): normal,
/// selected, hilite; `None` without the template.
fn medallions<H: ChallengeHost>(h: &H, i: usize) -> Option<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let d = h.util_data();
    let g = &d.challenge.get_challenge_generals()[i];
    let n = d.template_num_by_name(g.get_player_template_name());
    let t = d.template(n)?;
    Some((t.medallion_normal.clone(), t.medallion_selected.clone(), t.medallion_hilite.clone()))
}

/// `setEnabledButtons()`.
pub fn set_enabled_buttons<H: ChallengeHost>(wm: &mut GameWindowManager, h: &mut H) {
    for i in 0..NUM_GENERALS {
        let enabled = h.util_data().challenge.get_challenge_generals()[i].is_starting_enabled();
        let b = h.challenge_menu().button_general_position[i];
        let Some(b) = must(wm, b, "setEnabledButtons: no GeneralPosition button (buttonGeneralPosition[i]->winEnable on NULL)") else { return };
        wm.win_enable(b, enabled);
        wm.win_hide(b, !enabled);
        if let Some((normal, selected, hilite)) = medallions(h, i) {
            let enabled_image = find_image(h, &normal);
            wm.tree.windows[b].inst.enabled_draw_data[0].image = enabled_image.clone();
            if let Some(img) = enabled_image {
                // image size keeps changing, so it'll drive the window size directly
                let w = h.image_size(&img).map_or(0, |s| s.0);
                wm.win_set_size(h, b, w, w);
            }
            let selected_image = find_image(h, &selected);
            wm.tree.windows[b].inst.hilite_draw_data[1].image = selected_image.clone();
            wm.tree.windows[b].inst.disabled_draw_data[1].image = selected_image;
            let hilited_image = find_image(h, &hilite);
            wm.tree.windows[b].inst.hilite_draw_data[0].image = hilited_image;
        }
    }
}

/// `setGeneralBio(buttonIndex)`.
pub fn set_general_bio<H: ChallengeHost>(wm: &mut GameWindowManager, h: &mut H, button_index: i32) {
    if button_index < 0 || button_index >= NUM_GENERALS as i32 {
        return;
    }
    let m = h.challenge_menu().clone();
    let Some(bio_parent) = must(wm, m.bio_parent, "setGeneralBio: no GeneralsBioParent (bioParent->winHide on NULL)") else { return };
    wm.win_hide(bio_parent, false);
    let g = h.util_data().challenge.get_challenge_generals()[button_index as usize].clone();
    let image = find_image(h, &g.bio_portrait_small);
    let Some(portrait) = must(wm, m.bio_portrait, "setGeneralBio: no BioPortrait (bioPortrait->winSetEnabledImage on NULL)") else { return };
    wm.tree.windows[portrait].inst.enabled_draw_data[0].image = image;
    wm.win_set_status(portrait, win_status::IMAGE);
    let lines = [h.fetch_text(&g.bio_name), h.fetch_text(&g.bio_rank), h.fetch_text(&g.bio_branch), h.fetch_text(&g.bio_strategy)];
    let c = h.challenge_menu();
    c.bio_text_position = 0;
    c.bio_total_length = lines.iter().map(|l| l.len() as i32).sum();
    [c.bio_line1, c.bio_line2, c.bio_line3, c.bio_line4] = lines;
    // clear the bio readout text, because updateBio likes it that way
    for e in [m.bio_line1_entry, m.bio_line2_entry, m.bio_line3_entry, m.bio_line4_entry] {
        gadget::static_text_set_text(wm, h, e, &[]);
    }
}

/// `updateButtonSequence(stepsPerUpdate)` (the unfinished intro; never called upstream).
pub fn update_button_sequence<H: ChallengeHost>(wm: &mut GameWindowManager, h: &mut H, steps_per_update: i32) {
    const CLEANUP_STATES: i32 = 2;
    if h.challenge_menu().button_sequence_step > NUM_GENERALS as i32 + CLEANUP_STATES {
        return;
    }
    for _ in 0..steps_per_update {
        let mut pos = h.challenge_menu().button_sequence_step;
        // selected look, mouseover look, regular look
        for (k, which) in [0, 1, 2].into_iter().enumerate() {
            if k > 0 {
                pos -= 1;
            }
            let in_range = if k == 0 { pos < NUM_GENERALS as i32 } else { pos > 0 && pos < NUM_GENERALS as i32 };
            if !in_range {
                continue;
            }
            let b = h.challenge_menu().button_general_position[pos as usize];
            let Some(b) = must(wm, b, "updateButtonSequence: no GeneralPosition button (winIsHidden on NULL)") else { return };
            if wm.is_hidden(Some(b)) {
                continue;
            }
            if let Some((normal, selected, hilite)) = medallions(h, pos as usize) {
                let name = match which {
                    0 => selected,
                    1 => hilite,
                    _ => normal,
                };
                wm.tree.windows[b].inst.enabled_draw_data[0].image = find_image(h, &name);
            }
        }
        h.challenge_menu().button_sequence_step += 1;
    }
}

/// `updateBio(frames)`: whether a character was typed.
pub fn update_bio<H: ChallengeHost>(wm: &mut GameWindowManager, h: &mut H, frames: i32) -> bool {
    let mut ret = false;
    for _ in 0..frames {
        let m = h.challenge_menu().clone();
        if m.bio_text_position >= m.bio_total_length {
            continue;
        }
        let (l1, l2, l3) = (m.bio_line1.len() as i32, m.bio_line2.len() as i32, m.bio_line3.len() as i32);
        let p = m.bio_text_position;
        let (window, line, at) = if p < l1 {
            (m.bio_line1_entry, &m.bio_line1, p)
        } else if p < l1 + l2 {
            (m.bio_line2_entry, &m.bio_line2, p - l1)
        } else if p < l1 + l2 + l3 {
            (m.bio_line3_entry, &m.bio_line3, p - l1 - l2)
        } else {
            (m.bio_line4_entry, &m.bio_line4, p - l1 - l2 - l3)
        };
        let mut text = window.map(|w| gadget::static_text_get_text(wm, w)).unwrap_or_default();
        let w_char = line[at as usize];
        // (`concat(WideChar)`: a 0 appends nothing)
        if w_char != 0 {
            text.push(w_char);
        }
        gadget::static_text_set_text(wm, h, window, &text);
        h.challenge_menu().bio_text_position += 1;
        ret = true;
    }
    ret
}

/// `ChallengeMenuInit(layout, userData)`.
pub fn challenge_menu_init<H: ChallengeHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    // (`TheChallengeGameInfo` made if needed, `init`, `clearSlotList`, `reset`, `enterGame`: its contents are the
    // World's, made again by `start_challenge`)
    h.challenge_menu().challenge_game_info = true;
    shell::show_shell_map(wm, h, true);
    let mut k = |n: &str| h.name_to_key(format!("ChallengeMenu.wnd:{n}").as_bytes()) as i32;
    let keys = [
        k("ParentChallengeMenu"),
        k("ButtonPlay"),
        k("ButtonBack"),
        k("BioPortrait"),
        k("BioNameEntry"),
        k("BioDOBEntry"),
        k("BioBirthplaceEntry"),
        k("BioStrategyEntry"),
        k("MainBackdrop"),
        k("GeneralsBioParent"),
    ];
    let mut gen_ids = [0; NUM_GENERALS];
    for (i, id) in gen_ids.iter_mut().enumerate() {
        *id = k(&format!("GeneralPosition{i}"));
    }
    let gp = k("GadgetParent");
    let parent = wm.win_get_window_from_id(None, keys[0]);
    let w = |id: i32| wm.win_get_window_from_id(parent, id);
    let wins: Vec<Option<WinId>> = keys[1..].iter().map(|&id| w(id)).collect();
    let gens: Vec<Option<WinId>> = gen_ids.iter().map(|&id| w(id)).collect();
    let c = h.challenge_menu();
    [c.parent_id, c.button_play_id, c.button_back_id, c.bio_portrait_id, c.bio_name_entry_id, c.bio_dob_entry_id, c.bio_birthplace_entry_id, c.bio_strategy_entry_id, c.backdrop_id, c.bio_parent_id] = keys;
    c.button_general_position_id = gen_ids;
    c.parent_menu = parent;
    c.button_play = wins[0];
    c.button_back = wins[1];
    c.bio_portrait = wins[2];
    c.bio_line1_entry = wins[3];
    c.bio_line2_entry = wins[4];
    c.bio_line3_entry = wins[5];
    c.bio_line4_entry = wins[6];
    c.backdrop = wins[7];
    c.bio_parent = wins[8];
    for (i, g) in gens.iter().enumerate() {
        c.button_general_position[i] = *g;
    }
    for g in gens {
        // start all buttons hidden, then expose them later if there is a general for this spot
        let Some(g) = must(wm, g, "ChallengeMenuInit: no GeneralPosition button (buttonGeneralPosition[i]->winHide on NULL)") else { return };
        wm.win_hide(g, true);
    }
    let Some(bio) = must(wm, wins[8], "ChallengeMenuInit: no GeneralsBioParent (bioParent->winHide on NULL)") else { return };
    wm.win_hide(bio, true);
    let Some(play) = must(wm, wins[0], "ChallengeMenuInit: no ButtonPlay (buttonPlay->winHide on NULL)") else { return };
    wm.win_hide(play, true);
    let c = h.challenge_menu();
    c.is_auto_selecting = false;
    c.button_sequence_step = 0;
    set_enabled_buttons(wm, h);
    if wm.undefined.is_some() {
        return;
    }
    wm.layout_hide(layout, false);
    wm.win_set_focus(h, parent);
    let c = h.challenge_menu();
    c.just_entered = true;
    c.initial_gadget_delay = 2;
    if let Some(win) = wm.win_get_window_from_id(None, gp) {
        wm.win_hide(win, true);
    }
    let c = h.challenge_menu();
    c.is_shutting_down = false;
    // (`wndVideoManager->init()`: not translated; `lastSelectionSound`, `lastPreviewSound` cleared)
    c.has_played_intro_audio = false;
}

/// `ChallengeMenuUpdate(layout, userData)`.
pub fn challenge_menu_update<H: ChallengeHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    let c = h.challenge_menu();
    if c.just_entered {
        if c.initial_gadget_delay == 1 {
            wt::set_group(wm, h, b"ChallengeMenuFade", false);
            let c = h.challenge_menu();
            c.initial_gadget_delay = 2;
            c.just_entered = false;
        } else {
            c.initial_gadget_delay -= 1;
        }
    }
    // delay the voice for N updates after the transition is done
    if !h.challenge_menu().has_played_intro_audio && wt::is_finished(wm) {
        let c = h.challenge_menu();
        c.intro_audio_magic_number += 1;
        if c.intro_audio_magic_number == 10 {
            c.has_played_intro_audio = true;
            // "Choose your general."
            h.add_audio_event(b"Taunts_GCAnnouncer01");
        }
    }
    update_bio(wm, h, TELETYPE_SKIP);
    if h.challenge_menu().is_shutting_down && shell::is_anim_finished(wm, h) && wt::is_finished(wm) {
        shell::shutdown_complete(wm, h, Some(layout), false);
    }
    // (`wndVideoManager->update()`: not translated)
}

/// `ChallengeMenuShutdown(layout, userData)`: `immediate` is `*(Bool *)userData`.
pub fn challenge_menu_shutdown<H: ChallengeHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize, immediate: Option<bool>) {
    let c = h.challenge_menu();
    c.last_button_index = -1;
    c.button_sequence_step = 0;
    let Some(pop_immediate) = immediate else {
        wm.set_undefined("ChallengeMenuShutdown: no user data (*(Bool *)NULL)");
        return;
    };
    if pop_immediate {
        wm.layout_hide(layout, true);
        shell::shutdown_complete(wm, h, Some(layout), false);
        return;
    }
    wt::reverse(wm, h, b"ChallengeMenuFade");
    let c = h.challenge_menu();
    c.is_shutting_down = true;
    c.challenge_game_info = false;
    // (`TheAudio->removeAudioEvent` of the two sounds: the host's audio)
    c.intro_audio_magic_number = 0;
}

/// `ChallengeMenuInput(window, msg, mData1, mData2)`.
pub fn challenge_menu_input<H: ChallengeHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg != gwm::CHAR || (d1 & 0xFF) as u8 != key::ESC {
        return MsgHandled::Ignored;
    }
    if (d2 & 0xFF) & key::STATE_UP != 0 {
        let c = h.challenge_menu();
        let (back, id) = (c.button_back, c.button_back_id);
        wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(back), id as u32);
    }
    MsgHandled::Handled
}

/// The control of a button message and its id (`(GameWindow *)mData1`, `winGetWindowId()`).
fn control_id(wm: &mut GameWindowManager, d1: WindowMsgData, what: &'static str) -> Option<i32> {
    let Some(c) = data_win(d1) else {
        wm.set_undefined(what);
        return None;
    };
    Some(wm.tree.windows[c].id)
}

/// `ChallengeMenuSystem(window, msg, mData1, mData2)`.
pub fn challenge_menu_system<H: ChallengeHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
            return MsgHandled::Handled;
        }
        ggm::GBM_MOUSE_ENTERING => {
            let Some(id) = control_id(wm, d1, "ChallengeMenuSystem: GBM_MOUSE_ENTERING without a control") else { return MsgHandled::Handled };
            let i = find_position_button(h, id);
            if i != -1 && i != h.challenge_menu().last_button_index {
                set_general_bio(wm, h, i);
                // special sound for Harvard
                h.add_audio_event(b"GUILogoMouseOver");
                h.challenge_menu().last_hilited_index = i;
            }
        }
        ggm::GBM_MOUSE_LEAVING => {
            let Some(id) = control_id(wm, d1, "ChallengeMenuSystem: GBM_MOUSE_LEAVING without a control") else { return MsgHandled::Handled };
            let i = find_position_button(h, id);
            let last = h.challenge_menu().last_button_index;
            if i != -1 && i != last {
                // (with no selection yet the bio's hiding is commented out upstream)
                set_general_bio(wm, h, last);
            }
        }
        ggm::GBM_SELECTED => {
            let Some(id) = control_id(wm, d1, "ChallengeMenuSystem: GBM_SELECTED without a control") else { return MsgHandled::Handled };
            // we don't need to handle these
            if h.challenge_menu().is_auto_selecting {
                h.challenge_menu().is_auto_selecting = false;
                return MsgHandled::Handled;
            }
            let i = find_position_button(h, id);
            let c = h.challenge_menu().clone();
            if i != -1 {
                if c.last_button_index != -1 {
                    h.challenge_menu().is_auto_selecting = true;
                    let last = wm.win_get_window_from_id(None, c.button_general_position_id[c.last_button_index as usize]);
                    let Some(last) = must(wm, last, "ChallengeMenuSystem: no previous general button (GadgetCheckBoxToggle of NULL)") else { return MsgHandled::Handled };
                    gadget::check_box_toggle(wm, h, last);
                }
                // play audio to indicate selection (the previous sounds removed: the host's audio)
                let sound = h.util_data().challenge.get_challenge_generals()[i as usize].preview_sound.clone();
                h.add_audio_event(&sound);
                h.challenge_menu().last_button_index = i;
                let Some(play) = must(wm, c.button_play, "ChallengeMenuSystem: no ButtonPlay (buttonPlay->winHide on NULL)") else { return MsgHandled::Handled };
                wm.win_hide(play, false);
            } else if id == c.button_play_id {
                if !c.challenge_game_info {
                    // If this is NULL, then we must be on the way back out of this menu.
                    return MsgHandled::Handled;
                }
                // (`setGeneralCampaign(lastButtonIndex)`, the pending file, `TheChallengeGameInfo->setMap`: the World's
                // `start_challenge`, asked below)
                if c.last_button_index < 0 || c.last_button_index >= NUM_GENERALS as i32 {
                    wm.set_undefined("ChallengeMenuSystem: Play with no general selected (buttonGeneralPositionID[-1] read out of the array)");
                    return MsgHandled::Handled;
                }
                // turn off the last button so the screen will be pristine when the user returns
                h.challenge_menu().is_auto_selecting = true;
                let last = wm.win_get_window_from_id(None, c.button_general_position_id[c.last_button_index as usize]);
                let Some(last) = must(wm, last, "ChallengeMenuSystem: no selected general button (GadgetCheckBoxSetChecked of NULL)") else { return MsgHandled::Handled };
                gadget::check_box_set_checked(wm, h, last, false);
                let cm = h.challenge_menu();
                cm.last_button_index = -1;
                cm.button_sequence_step = 0;
                // (`clearGameData` if in a game, the challenge's difficulty, `MSG_NEW_GAME`, `InitRandom(0)`: the World's)
                let difficulty = h.main_menu().difficulty;
                h.request(ShellRequest::StartChallenge { general: c.last_button_index, difficulty });
            } else if id == c.button_back_id {
                shell::pop(wm, h);
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::challenge_generals::ChallengeGenerals;
    use crate::client::gui_util::TemplateView;
    use crate::client::save_load_menu::tests::{booted, click, hidden, named, settle, tick, H};

    const F: &str = "ChallengeMenu.wnd";
    /// Three generals: 0 and 2 start enabled (`FactionAmerica`, `FactionChina`), 1 does not.
    const CHALLENGE: &[u8] = b"ChallengeGenerals\n GeneralPersona0\n  StartsEnabled = Yes\n  BioNameString = GUI:N0\n  BioRankString = GUI:R0\n  BioBranchString = GUI:B0\n  BioStrategyString = GUI:S0\n  BioPortraitSmall = imgPortrait0\n  Campaign = Challenge_0\n  PlayerTemplate = FactionAmerica\n  PreviewSound = Preview0\n End\n\
 GeneralPersona1\n  StartsEnabled = No\n  Campaign = Challenge_1\n  PlayerTemplate = FactionAmerica\n End\n\
 GeneralPersona2\n  StartsEnabled = Yes\n  BioNameString = GUI:N2\n  Campaign = Challenge_2\n  PlayerTemplate = factionchina\n  PreviewSound = Preview2\n End\nEnd\n";

    fn tpl(name: &str, m: &str) -> TemplateView {
        let s = |x: &str| format!("img{m}{x}").into_bytes();
        TemplateView { name: name.into(), medallion_normal: s("Normal"), medallion_selected: s("Selected"), medallion_hilite: s("Hilite"), ..Default::default() }
    }

    /// The shell with the challenge's generals and two player templates.
    fn shell() -> (GameWindowManager, H) {
        let (wm, mut h) = booted();
        let mut c = ChallengeGenerals::new();
        c.init(CHALLENGE).expect("ChallengeMode.ini");
        h.skirmish_data.challenge = c;
        h.skirmish_data.templates = vec![tpl("FactionAmerica", "Usa"), tpl("FactionChina", "Chn")];
        (wm, h)
    }

    fn general(wm: &GameWindowManager, i: usize) -> WinId {
        named(wm, F, &format!("GeneralPosition{i}"))
    }

    fn line(wm: &GameWindowManager, n: &str) -> String {
        String::from_utf16_lossy(&gadget::static_text_get_text(wm, named(wm, F, n)))
    }

    fn mouse_at(wm: &mut GameWindowManager, h: &mut H, w: WinId) {
        let (x, y) = wm.tree.screen_position(w);
        wm.win_process_mouse_event(h, gwm::MOUSE_POS, (x + 5, y + 5), (0, 0));
    }

    /// #98: the Generals' Challenge from the main menu, predicted from MainMenu.cpp and ChallengeMenu.cpp before the run:
    ///
    /// - *Single Player*, *Challenge*, *Medium*: `setupGameStart` pushes `Menus/ChallengeMenu.wnd` (no campaign start);
    ///   its init: `TheChallengeGameInfo` made; the general buttons 0 and 2 shown and enabled, 1 .. 11 hidden; 0's
    ///   image `ImgUsaNormal` sized 32x32 (the image's width both ways), its hilited images `ImgUsaHilite` (0) and
    ///   `ImgUsaSelected` (1), its disabled unchecked box `ImgUsaSelected`; 2's `ImgChnNormal` (the template's name
    ///   found without case); the bio and *Play* hidden; `GadgetParent` hidden; a frame later `ChallengeMenuFade`;
    ///   ten updates once the transitions are done, `Taunts_GCAnnouncer01`, once;
    /// - the mouse on general 0: the bio shown, the portrait `ImgPortrait0`, `GUILogoMouseOver`; the lines typed two
    ///   characters an update: after one update `T:`, after four `T:GUI:N0` (8 characters), then the rank's line;
    /// - general 0 pressed: `Preview0`, *Play* shown, the button checked; leaving it changes nothing;
    /// - general 2 pressed: 0's button toggled off (its own `GBM_SELECTED` swallowed), `Preview2`, 2 selected;
    /// - *Play*: `StartChallenge { general: 2, difficulty: 1 }`, 2's button un-checked, the selection forgotten;
    /// - *Play* again: no general: `buttonGeneralPositionID[-1]`, undefined;
    /// - Escape's release presses *Back*: the menu pops once `ChallengeMenuFade` is reversed; `TheChallengeGameInfo`
    ///   deleted.
    #[test]
    fn the_challenge_menu_picks_a_general_and_starts_its_campaign() {
        let (mut wm, mut h) = shell();
        wm.win_process_key(&mut h, crate::client::window_manager::key::ENTER, crate::client::window_manager::key::STATE_DOWN);
        settle(&mut wm, &mut h);
        for b in ["ButtonSinglePlayer", "ButtonChallenge", "ButtonMedium"] {
            let w = named(&wm, "MainMenu.wnd", b);
            click(&mut wm, &mut h, w);
            settle(&mut wm, &mut h);
        }
        settle(&mut wm, &mut h);
        assert_eq!(wm.layouts[h.shell.top().expect("top")].filename, b"Menus/ChallengeMenu.wnd");
        assert!(h.requests.is_empty(), "{:?}", h.requests);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        assert!(h.challenge.challenge_game_info);
        let (g0, g1, g2) = (general(&wm, 0), general(&wm, 1), general(&wm, 2));
        assert!(!hidden(&wm, g0) && !hidden(&wm, g2) && hidden(&wm, g1) && !wm.is_enabled(Some(g1)));
        for i in 3..12 {
            assert!(hidden(&wm, general(&wm, i)), "{i}");
        }
        let d = &wm.tree.windows[g0].inst;
        assert_eq!(d.enabled_draw_data[0].image.as_deref(), Some(&b"ImgUsaNormal"[..]));
        assert_eq!(d.hilite_draw_data[0].image.as_deref(), Some(&b"ImgUsaHilite"[..]));
        assert_eq!(d.hilite_draw_data[1].image.as_deref(), Some(&b"ImgUsaSelected"[..]));
        assert_eq!(d.disabled_draw_data[1].image.as_deref(), Some(&b"ImgUsaSelected"[..]));
        assert_eq!(wm.tree.windows[g0].size, (32, 32));
        assert_eq!(wm.tree.windows[g2].inst.enabled_draw_data[0].image.as_deref(), Some(&b"ImgChnNormal"[..]));
        assert!(hidden(&wm, named(&wm, F, "GeneralsBioParent")) && hidden(&wm, named(&wm, F, "ButtonPlay")));
        assert!(hidden(&wm, named(&wm, F, "GadgetParent")));
        // (prediction missed: the settles had run the menu's update once; the fade and the announcer's ten updates
        // need frames of their own)
        for _ in 0..30 {
            tick(&mut wm, &mut h);
        }
        assert!(!h.challenge.just_entered);
        assert_eq!(h.audio.iter().filter(|a| a.as_slice() == b"Taunts_GCAnnouncer01").count(), 1, "{:?}", h.audio);
        // the bio
        h.audio.clear();
        mouse_at(&mut wm, &mut h, g0);
        assert!(!hidden(&wm, named(&wm, F, "GeneralsBioParent")));
        assert_eq!(wm.tree.windows[named(&wm, F, "BioPortrait")].inst.enabled_draw_data[0].image.as_deref(), Some(&b"ImgPortrait0"[..]));
        assert!(h.audio.contains(&b"GUILogoMouseOver".to_vec()));
        assert_eq!(h.challenge.last_hilited_index, 0);
        tick(&mut wm, &mut h);
        assert_eq!(line(&wm, "BioNameEntry"), "T:");
        for _ in 0..3 {
            tick(&mut wm, &mut h);
        }
        assert_eq!((line(&wm, "BioNameEntry").as_str(), line(&wm, "BioDOBEntry").as_str()), ("T:GUI:N0", ""));
        tick(&mut wm, &mut h);
        assert_eq!(line(&wm, "BioDOBEntry"), "T:");
        // general 0, then 2
        click(&mut wm, &mut h, g0);
        assert!(h.audio.contains(&b"Preview0".to_vec()));
        assert_eq!(h.challenge.last_button_index, 0);
        assert!(!hidden(&wm, named(&wm, F, "ButtonPlay")) && gadget::check_box_is_checked(&wm, g0));
        wm.win_process_mouse_event(&mut h, gwm::MOUSE_POS, (790, 590), (0, 0));
        assert_eq!(h.challenge.last_button_index, 0);
        click(&mut wm, &mut h, g2);
        assert!(!gadget::check_box_is_checked(&wm, g0) && gadget::check_box_is_checked(&wm, g2));
        assert!(!h.challenge.is_auto_selecting);
        assert!(h.audio.contains(&b"Preview2".to_vec()));
        assert_eq!(h.challenge.last_button_index, 2);
        // Play
        let play = named(&wm, F, "ButtonPlay");
        click(&mut wm, &mut h, play);
        assert_eq!(h.requests, [ShellRequest::StartChallenge { general: 2, difficulty: 1 }]);
        assert!(!gadget::check_box_is_checked(&wm, g2) && !h.challenge.is_auto_selecting);
        assert_eq!(h.challenge.last_button_index, -1);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        click(&mut wm, &mut h, play);
        assert!(wm.undefined.take().is_some(), "Play with no general");
        // Escape: Back
        h.requests.clear();
        let p = h.challenge.parent_menu.expect("parent");
        wm.win_set_focus(&mut h, Some(p));
        wm.win_process_key(&mut h, key::ESC, key::STATE_UP);
        assert!(h.challenge.is_shutting_down && !h.challenge.challenge_game_info);
        settle(&mut wm, &mut h);
        tick(&mut wm, &mut h);
        assert_eq!(wm.layouts[h.shell.top().expect("top")].filename, b"Menus/MainMenu.wnd");
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        assert!(!h.untranslated.iter().any(|f| f.starts_with(b"Challenge")), "{:?}", h.untranslated);
    }

    /// #98: the pieces on their own, predicted from ChallengeMenu.cpp:
    /// - `findPositionButton` of an id that is no general's is -1;
    /// - `setGeneralBio(-1)` and `(12)` do nothing; `updateBio` returns false once the bio is typed out;
    /// - `updateButtonSequence(1)` from step 0: general 0's image becomes its selected medallion; step 1: 0's the
    ///   hilite one (`--pos > 0` fails at 0: unchanged, as upstream), step 2: 2's selected (and 1 hidden: skipped);
    ///   past `NUM_GENERALS + 2` steps it does nothing;
    /// - *Play* after the shutdown deleted `TheChallengeGameInfo` is eaten; a shutdown without user data is undefined.
    #[test]
    fn the_challenge_pieces_follow_upstream() {
        let (mut wm, mut h) = shell();
        shell::push(&mut wm, &mut h, b"Menus/ChallengeMenu.wnd", true);
        tick(&mut wm, &mut h);
        assert_eq!(find_position_button(&mut h, -5), -1);
        let id2 = h.challenge.button_general_position_id[2];
        assert_eq!(find_position_button(&mut h, id2), 2);
        set_general_bio(&mut wm, &mut h, -1);
        set_general_bio(&mut wm, &mut h, 12);
        assert!(hidden(&wm, named(&wm, F, "GeneralsBioParent")));
        set_general_bio(&mut wm, &mut h, 2);
        while update_bio(&mut wm, &mut h, 5) {}
        assert_eq!(line(&wm, "BioNameEntry"), "T:GUI:N2");
        assert!(!update_bio(&mut wm, &mut h, 1));
        let g0 = general(&wm, 0);
        update_button_sequence(&mut wm, &mut h, 1);
        assert_eq!(wm.tree.windows[g0].inst.enabled_draw_data[0].image.as_deref(), Some(&b"ImgUsaSelected"[..]));
        update_button_sequence(&mut wm, &mut h, 1);
        assert_eq!(wm.tree.windows[g0].inst.enabled_draw_data[0].image.as_deref(), Some(&b"ImgUsaSelected"[..]));
        update_button_sequence(&mut wm, &mut h, 1);
        assert_eq!(wm.tree.windows[general(&wm, 2)].inst.enabled_draw_data[0].image.as_deref(), Some(&b"ImgChnSelected"[..]));
        h.challenge.button_sequence_step = NUM_GENERALS as i32 + 3;
        update_button_sequence(&mut wm, &mut h, 1);
        assert_eq!(h.challenge.button_sequence_step, NUM_GENERALS as i32 + 3);
        h.challenge.challenge_game_info = false;
        h.challenge.last_button_index = 0;
        let play = named(&wm, F, "ButtonPlay");
        let p = h.challenge.parent_menu.expect("parent");
        assert_eq!(challenge_menu_system(&mut wm, &mut h, p, ggm::GBM_SELECTED, win_data(Some(play)), 0), MsgHandled::Handled);
        assert!(h.requests.is_empty() && wm.undefined.is_none());
        challenge_menu_shutdown(&mut wm, &mut h, 0, None);
        assert!(wm.undefined.is_some());
    }
}
