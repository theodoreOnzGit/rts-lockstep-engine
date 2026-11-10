// SPDX-License-Identifier: GPL-3.0-only
//! The game's own in-game UI from its files (#12): the control bar of `Window\ControlBar.wnd` (read by
//! `rts_lockstep_engine::client::window_script`), with the mapped images of `Data\INI\MappedImages`
//! (`client::image`), the side's control bar scheme (`client::control_bar_scheme`), the selected object's command
//! buttons and build queue (`client::control_bar_command`), the money (`InGameUI::update`'s `GUI:ControlBarMoneyDisplay`)
//! and the power.
//!
//! Plain Rust, no Bevy: [`load_game_ui`] reads the files, [`GameUi::frame`] says what to draw this frame as quads in
//! the layout's 800x600 space ([`UiQuad`]); `app` draws them with Bevy UI nodes. ~~Upstream's own drawing
//! (`W3DGameWinDefaultDraw`, the gadget draw functions, `W3DPowerDraw`) is not translated~~ **Corrected 2026-11-10
//! (#34):** `W3DGameWinDefaultDraw` and the push button, check box, radio button and static text draws are translated
//! (`client::window_draw`, a draw list), but the bar does not draw from them yet (`W3DPowerDraw` and the command bar's
//! own draws are not translated); here: a window with
//! `WIN_STATUS_IMAGE` is drawn as its state's first image stretched over its region, any other as its first colour,
//! text centred; the power bar as production against consumption. That part is new.
//!
//! Since #35, the selected object's portrait (`ControlBar.wnd:WinUnitSelected`, `CameoWindow`, `UnitUpgrade1..5`, from
//! `client::control_bar_popup::set_portrait_by_object`) and each command button's popup description
//! (`populate_build_tooltip_layout`): [`BarState`] carries them, [`TooltipTimer`] decides when the popup shows (new: the
//! hover delay, and on a phone a long press, with the last button touched kept as a visible fallback).
//!
//! Since #34, the bar's windows live in the window manager (`rts_lockstep_engine::client::window_manager`, a translation
//! of `GameWindowManager`): [`GameUi::sync`] hides, shows, enables and disables them as `switchToContext` and
//! `setControlCommand` would from the [`BarState`], and [`GameUi::pointer`] sends the pointer through
//! `winProcessMouseEvent`, so a command button is pressed by the push button's own `GBM_SELECTED` reaching
//! `ControlBarSystem`. Touch is a mouse: a press moves the pointer there first. The drawing is still the quads above (moving
//! it onto the manager's draw list is #34's remaining work).
//!
//! The client still never changes the simulation: everything here reads the `World`.

use std::collections::BTreeMap;

use rts_lockstep_engine::client::control_bar_command::{populate_build_queue, populate_command, CommandSlot, ControlBarStateHost, QueueSlot};
use rts_lockstep_engine::client::control_bar_popup::{populate_build_tooltip_layout, set_portrait_by_object, PopupHost, Portrait, PortraitHost, Tooltip, MAX_RIGHT_HUD_UPGRADE_CAMEOS};
use rts_lockstep_engine::client::control_bar_scheme::{ControlBarSchemeManager, SchemeDraw};
use rts_lockstep_engine::client::image::ImageCollection;
use rts_lockstep_engine::client::gadget;
use rts_lockstep_engine::client::window_manager::{data_win, ggm, gwm, GameWindowManager, GuiHost, MsgHandled, NoGuiHost, WindowMsgData};
use rts_lockstep_engine::client::window_script::{win_status, GameWindow, WinId, WindowScript, WndHost, WIN_COLOR_UNDEFINED};
use rts_lockstep_engine::common::file_system::{FileSystem, LocalDevice};
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::science::GameTextFetch;
use rts_lockstep_engine::logic::world::game_data::GameDataFiles;

/// The layout's size: the control bar is read at its creation resolution and scaled by the drawing.
pub const LAYOUT: (i32, i32) = (800, 600);

/// What [`load_game_ui`] found.
pub enum UiBuild {
    Ready(Box<GameUi>),
    /// A file's bytes are still being fetched (the web's ranged reads): ask again later.
    Waiting,
    /// The files are not there (no game files, or not the control bar's): the debug panel stays.
    Missing(String),
}

/// The control bar, read.
pub struct GameUi {
    pub images: ImageCollection,
    /// (#34) The window manager holding the bar's windows (`wm.tree`).
    pub wm: GameWindowManager,
    pub schemes: ControlBarSchemeManager,
    /// `ButtonCommand01..18`.
    pub command_windows: Vec<Option<WinId>>,
    /// `ButtonQueue01..09`.
    pub queue_windows: Vec<Option<WinId>>,
    pub money: Option<WinId>,
    pub power: Option<WinId>,
    /// The context parents `switchToContext` shows and hides.
    pub command_parent: Option<WinId>,
    pub queue_parent: Option<WinId>,
    /// The other context parents (`CP_BEACON`, `CP_UNDER_CONSTRUCTION`, `CP_OCL_TIMER`, `CP_OBSERVER_INFO`,
    /// `CP_OBSERVER_LIST`), which `switchToContext(CB_CONTEXT_NONE)` and the command context hide (their own contexts
    /// are not translated yet).
    pub hidden_parents: Vec<WinId>,
    /// `GUI:ControlBarMoneyDisplay`.
    pub money_format: Vec<u16>,
    /// (#35) `ControlBar.wnd:WinUnitSelected`, `CameoWindow` and `UnitUpgrade1..5`.
    pub unit_selected: Option<WinId>,
    pub cameo: Option<WinId>,
    pub upgrade_cameos: Vec<Option<WinId>>,
    /// (#35) `ControlBar.wnd:LeftHUD`, the radar's window (`Radar::newMap`'s `m_radarWindow`).
    pub radar: Option<WinId>,
}

/// The reads of the files, telling "not yet" (an unserved ranged read) from "not there".
struct Reads<'a, D: LocalDevice> {
    fs: &'a FileSystem<D>,
    waiting: std::cell::Cell<bool>,
}

impl<D: LocalDevice> Reads<'_, D> {
    fn open(&self, path: &[u8]) -> Option<Vec<u8>> {
        let before = self.fs.local.device.unserved_reads();
        let r = self.fs.open_file(path);
        if self.fs.local.device.unserved_reads() != before {
            self.waiting.set(true);
            return None;
        }
        r
    }
}

impl<D: LocalDevice> GameDataFiles for Reads<'_, D> {
    fn open(&self, path: &[u8]) -> Option<Vec<u8>> {
        Reads::open(self, path)
    }
    fn list_ini(&self, dir: &[u8]) -> rts_lockstep_engine::formats::big::FilenameList {
        self.fs.list_ini(dir)
    }
}

/// The window manager's services for the reader: name keys of its own, the collection's images, the game's text.
struct Host<'a, T: GameTextFetch> {
    keys: BTreeMap<Vec<u8>, u32>,
    images: &'a ImageCollection,
    text: &'a T,
}

impl<T: GameTextFetch> WndHost for Host<'_, T> {
    fn name_to_key(&mut self, name: &[u8]) -> u32 {
        let n = self.keys.len() as u32 + 1;
        *self.keys.entry(name.to_vec()).or_insert(n)
    }
    fn find_image(&self, name: &[u8]) -> Option<Vec<u8>> {
        self.images.find_image_by_name(name).map(|i| i.name.clone())
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        self.text.fetch(label).str().to_vec()
    }
    fn display_size(&self) -> (i32, i32) {
        LAYOUT
    }
}

/// Reads the control bar: the mapped images (`ImageCollection::load(512)`), `Window\ControlBar.wnd`, the schemes, and
/// the scheme of `side` (`setControlBarSchemeByPlayerTemplate`). Reads only.
pub fn load_game_ui<D: LocalDevice>(fs: &FileSystem<D>, text: &impl GameTextFetch, side: &[u8]) -> UiBuild {
    let reads = Reads { fs, waiting: std::cell::Cell::new(false) };
    let mut images = ImageCollection::new();
    let loaded = images.load(512, &reads);
    if reads.waiting.get() {
        return UiBuild::Waiting;
    }
    if let Err(e) = loaded {
        return UiBuild::Missing(format!("the mapped images did not load: {e:?}"));
    }
    let Some(wnd) = reads.open(b"Window\\ControlBar.wnd") else {
        return if reads.waiting.get() { UiBuild::Waiting } else { UiBuild::Missing("no Window\\ControlBar.wnd".into()) };
    };
    let mut schemes = ControlBarSchemeManager::new();
    let r = schemes.init(|p| reads.open(p));
    if reads.waiting.get() {
        return UiBuild::Waiting;
    }
    if let Err(e) = r {
        return UiBuild::Missing(format!("the control bar schemes did not load: {e:?}"));
    }
    let mut wm = GameWindowManager::new(LAYOUT);
    let mut host = Host { keys: BTreeMap::new(), images: &images, text };
    if let Err(e) = wm.win_create_from_script(&mut WindowScript::new(), b"ControlBar.wnd", &wnd, &mut host, &mut NoGuiHost) {
        return UiBuild::Missing(format!("ControlBar.wnd did not read: {e:?}"));
    }
    schemes.set_control_bar_scheme_by_side(side, false, &mut wm.tree, LAYOUT, &images);
    let find = |n: String| wm.tree.find_by_name(n.as_bytes());
    let ui = GameUi {
        command_windows: (1..=18).map(|i| find(format!("ControlBar.wnd:ButtonCommand{i:02}"))).collect(),
        queue_windows: (1..=9).map(|i| find(format!("ControlBar.wnd:ButtonQueue{i:02}"))).collect(),
        money: find("ControlBar.wnd:MoneyDisplay".into()),
        power: find("ControlBar.wnd:PowerWindow".into()),
        command_parent: find("ControlBar.wnd:CommandWindow".into()),
        queue_parent: find("ControlBar.wnd:ProductionQueueWindow".into()),
        hidden_parents: ["BeaconWindow", "UnderConstructionWindow", "OCLTimerWindow", "ObserverPlayerInfoWindow", "ObserverPlayerListWindow"]
            .iter()
            .filter_map(|n| find(format!("ControlBar.wnd:{n}")))
            .collect(),
        money_format: text.fetch(b"GUI:ControlBarMoneyDisplay").str().to_vec(),
        unit_selected: find("ControlBar.wnd:WinUnitSelected".into()),
        cameo: find("ControlBar.wnd:CameoWindow".into()),
        upgrade_cameos: (1..=MAX_RIGHT_HUD_UPGRADE_CAMEOS).map(|i| find(format!("ControlBar.wnd:UnitUpgrade{i}"))).collect(),
        radar: find("ControlBar.wnd:LeftHUD".into()),
        images,
        wm,
        schemes,
    };
    UiBuild::Ready(Box::new(ui))
}

/// (#34) The bar's own callbacks for the manager (new: upstream's `ControlBarSystem` calls
/// `processContextSensitiveButtonClick`; here the button `GBM_SELECTED` names is kept for the caller to press).
#[derive(Default)]
struct BarHost {
    selected: Option<WinId>,
}

impl GuiHost for BarHost {
    fn system(&mut self, _wm: &mut GameWindowManager, func: &[u8], _win: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
        if func == b"ControlBarSystem" && msg == ggm::GBM_SELECTED {
            self.selected = data_win(d1);
            return MsgHandled::Handled;
        }
        MsgHandled::Ignored
    }
}

/// One thing to draw, in the layout's space.
#[derive(Clone, Debug, PartialEq)]
pub struct UiQuad {
    /// (x0, y0, x1, y1).
    pub rect: [f32; 4],
    /// A mapped image's name.
    pub image: Option<Vec<u8>>,
    /// RGBA.
    pub color: Option<[u8; 4]>,
    pub text: Option<String>,
    /// A command window's slot (0..18): a click presses its button.
    pub command: Option<usize>,
    /// Drawn darker (a disabled button).
    pub dim: bool,
    /// A clock's percent over the quad.
    pub clock: Option<i32>,
    /// (#35) An image drawn over the quad (the portrait's veterancy chevron).
    pub overlay: Option<Vec<u8>>,
}

/// What the bar shows this frame, read from the World.
#[derive(Clone, Debug, Default)]
pub struct BarState {
    /// The selected object the bar is for (`m_currentSelectedDrawable`).
    pub selected: Option<ObjectId>,
    pub commands: Vec<Option<CommandSlot>>,
    pub queue: Vec<QueueSlot>,
    pub queue_clock: Option<i32>,
    pub money: u32,
    /// (production, consumption).
    pub power: (i32, i32),
    /// (#35) The portrait (`setPortraitByObject`), and each command window's popup (`populateBuildTooltipLayout`).
    pub portrait: Portrait,
    pub tooltips: Vec<Option<Tooltip>>,
}

/// When the popup of a command button shows (new: upstream's `showBuildTooltipLayout` waits `getTooltipDelay()` with the
/// mouse over the button; a phone has no hover). A pointer over a slot for [`TooltipTimer::DELAY`] seconds shows its popup,
/// whether a mouse hovers or a finger holds. A press held that long is a **long press**: its release shows the popup and
/// does not press the button. A touch's slot stays shown after the finger lifts (the **visible fallback**: the last button
/// touched keeps its description on screen) until another is touched or the selection changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TooltipTimer {
    /// The slot under the pointer, since when, and whether the pointer is a touch.
    over: Option<(usize, f64, bool)>,
    /// The slot pressed, and since when.
    pressed: Option<(usize, f64)>,
    /// The slot kept shown after a touch.
    pinned: Option<usize>,
}

impl TooltipTimer {
    /// Seconds over (or held on) a button before its popup shows.
    pub const DELAY: f64 = 0.5;

    /// The pointer came over `slot` at `now`.
    pub fn over(&mut self, slot: usize, now: f64, touch: bool) {
        if self.over.map(|o| o.0) != Some(slot) {
            self.over = Some((slot, now, touch));
        }
        if touch && self.pinned != Some(slot) {
            self.pinned = None;
        }
    }

    /// The pointer left `slot`.
    pub fn out(&mut self, slot: usize) {
        if self.over.is_some_and(|o| o.0 == slot) {
            self.over = None;
        }
    }

    /// `slot` was pressed at `now`.
    pub fn press(&mut self, slot: usize, now: f64) {
        self.pressed = Some((slot, now));
    }

    /// `slot` was clicked (released over it) at `now`: whether the click presses the button (`false` after a long press).
    pub fn click(&mut self, slot: usize, now: f64, touch: bool) -> bool {
        let long = self.pressed.is_some_and(|(s, t)| s == slot && now - t >= Self::DELAY);
        self.pressed = None;
        if touch || long {
            self.pinned = Some(slot);
        }
        !long
    }

    /// The selection changed: nothing shown.
    pub fn reset(&mut self) {
        *self = TooltipTimer::default();
    }

    /// The slot whose popup shows at `now`.
    pub fn shown(&self, now: f64) -> Option<usize> {
        if let Some((s, t, _)) = self.over {
            if now - t >= Self::DELAY {
                return Some(s);
            }
        }
        if let Some((s, t)) = self.pressed {
            if now - t >= Self::DELAY {
                return Some(s);
            }
        }
        self.pinned
    }
}

/// A popup's text as the drawing shows it: the name, the cost (if any) and the description, one line each (the
/// description keeps its own line breaks).
pub fn tooltip_lines(t: &Tooltip) -> Vec<String> {
    let mut v = vec![String::from_utf16_lossy(&t.name)];
    if let Some(c) = &t.cost {
        v.push(String::from_utf16_lossy(c));
    }
    if !t.description.is_empty() {
        v.push(String::from_utf16_lossy(&t.description));
    }
    v
}

/// `UnicodeString::format(fmt, money)` for the money's one `%d` (the string as it is when it has none).
pub fn format_money(fmt: &[u16], money: u32) -> String {
    let f = String::from_utf16_lossy(fmt);
    if f.contains("%d") {
        f.replacen("%d", &money.to_string(), 1)
    } else if f.is_empty() || f.starts_with("MISSING") {
        format!("$ {money}")
    } else {
        f
    }
}

fn color(c: i32) -> Option<[u8; 4]> {
    if c == WIN_COLOR_UNDEFINED {
        return None;
    }
    let u = c as u32;
    Some([(u >> 16) as u8, (u >> 8) as u8, u as u8, (u >> 24) as u8])
}

impl GameUi {
    /// The bar's state for `selected`: `populateCommand` and the build queue when an object is selected, else nothing; the
    /// portrait (`setPortraitByObject`) and each shown button's popup (`populateBuildTooltipLayout`).
    pub fn bar_state<H: ControlBarStateHost + PopupHost + PortraitHost>(selected: Option<ObjectId>, h: &mut H, queue: Vec<(bool, u32, Option<Vec<u8>>, f32)>, money: u32, power: (i32, i32)) -> BarState {
        let commands: Vec<Option<CommandSlot>> = selected.map(|o| populate_command(o, h).to_vec()).unwrap_or_default();
        let tooltips = commands.iter().map(|c| c.as_ref().and_then(|c| h.button(c.button)).map(|b| populate_build_tooltip_layout(&b, h))).collect();
        let portrait = set_portrait_by_object(selected, h);
        let (queue, queue_clock) = populate_build_queue(&queue);
        BarState { selected, commands, queue, queue_clock, money, power, portrait, tooltips }
    }

    /// Whether a window is drawn: it and every ancestor not hidden, with the context parents shown as
    /// `switchToContext` would (the command window with a selection; the queue window with a queue).
    fn shown(&self, w: WinId, s: &BarState) -> bool {
        let mut cur = Some(w);
        while let Some(i) = cur {
            let win = &self.wm.tree.windows[i];
            if self.hidden_rule(i, s).unwrap_or_else(|| win.is_hidden()) {
                return false;
            }
            cur = win.parent;
        }
        true
    }

    /// Whether the bar's state hides window `i` (`switchToContext`, `setControlCommand`, the portrait); `None` for a
    /// window the state does not decide.
    fn hidden_rule(&self, i: WinId, s: &BarState) -> Option<bool> {
        let hidden = if Some(i) == self.command_parent {
            s.selected.is_none()
        } else if Some(i) == self.queue_parent {
            s.queue.is_empty()
        } else if let Some(slot) = self.command_windows.iter().position(|&c| c == Some(i)) {
            s.commands.get(slot).is_none_or(|c| c.is_none())
        } else if let Some(q) = self.queue_windows.iter().position(|&c| c == Some(i)) {
            q >= s.queue.len()
        } else if self.hidden_parents.contains(&i) {
            true
        } else if Some(i) == self.money || Some(i) == self.power {
            false
        } else if Some(i) == self.unit_selected {
            !s.portrait.shown
        } else if let Some(u) = self.upgrade_cameos.iter().position(|&c| c == Some(i)) {
            s.portrait.upgrade_cameos.get(u).is_none_or(|c| c.is_none())
        } else {
            return None;
        };
        Some(hidden)
    }

    /// (#34) The bar's state into the manager's windows, as upstream sets them (`winHide`, `winEnable`): the context
    /// windows hidden or shown by [`GameUi::hidden_rule`], each command button enabled when its command is available.
    /// Only changes are applied, so a window that stays hidden is not told again (`windowHiding`).
    pub fn sync(&mut self, s: &BarState) {
        for i in 0..self.wm.tree.windows.len() {
            if let Some(h) = self.hidden_rule(i, s) {
                if h != self.wm.tree.windows[i].is_hidden() {
                    self.wm.win_hide(i, h);
                }
            }
        }
        for (slot, w) in self.command_windows.clone().into_iter().enumerate() {
            let (Some(w), Some(Some(c))) = (w, s.commands.get(slot)) else { continue };
            if c.enabled != (self.wm.tree.windows[w].status & win_status::ENABLED != 0) {
                self.wm.win_enable(w, c.enabled);
            }
        }
    }

    /// (#34) A pointer event at `pos` (the layout's pixels) through `winProcessMouseEvent`: `msg` is `GWM_MOUSE_POS`,
    /// `GWM_LEFT_DOWN` or `GWM_LEFT_UP` (a touch is a mouse; its press moves the pointer there first). The popup's timer
    /// follows the window under the pointer and the press; returns the command slot whose button the manager selected
    /// (`GBM_SELECTED` reaching `ControlBarSystem`), unless the release ends a long press ([`TooltipTimer::click`]).
    pub fn pointer(&mut self, msg: u32, pos: (f32, f32), now: f64, touch: bool, tip: &mut TooltipTimer) -> Option<usize> {
        let p = (pos.0.floor() as i32, pos.1.floor() as i32);
        let mut host = BarHost::default();
        if msg == gwm::LEFT_DOWN && touch {
            self.wm.win_process_mouse_event(&mut host, gwm::MOUSE_POS, p, (0, 0));
        }
        let before = self.slot_of(self.wm.curr_mouse_rgn);
        self.wm.win_process_mouse_event(&mut host, msg, p, (0, 0));
        let after = self.slot_of(self.wm.curr_mouse_rgn);
        if before != after {
            if let Some(b) = before {
                tip.out(b);
            }
        }
        if let Some(a) = after {
            tip.over(a, now, touch);
        }
        if msg == gwm::LEFT_DOWN {
            if let Some(g) = self.slot_of(self.wm.grab_window) {
                tip.press(g, now);
            }
        }
        let slot = self.slot_of(host.selected)?;
        tip.click(slot, now, touch).then_some(slot)
    }

    /// The command slot of a window.
    fn slot_of(&self, w: Option<WinId>) -> Option<usize> {
        let w = w?;
        self.command_windows.iter().position(|&c| c == Some(w))
    }

    /// The quads of a frame: the scheme's background layers, the windows (parents before children; a parent's children
    /// oldest first, so the newest is on top, as the window list's head is), then the scheme's foreground.
    pub fn frame(&self, s: &BarState) -> Vec<UiQuad> {
        let mut out = Vec::new();
        let scheme = self.schemes.current_scheme();
        let draws: Vec<SchemeDraw> = scheme.map(|c| c.draw_list(self.schemes.multiplier, Default::default(), &self.images)).unwrap_or_default();
        let quad = |d: &SchemeDraw| UiQuad { rect: [d.start.0, d.start.1, d.end.0, d.end.1], image: Some(d.image.clone()), color: None, text: None, command: None, dim: false, clock: None, overlay: None };
        out.extend(draws.iter().filter(|d| d.layer >= 3).map(quad));
        let roots: Vec<WinId> = (0..self.wm.tree.windows.len()).filter(|&i| self.wm.tree.windows[i].parent.is_none()).collect();
        for r in roots {
            self.windows_into(r, s, &mut out);
        }
        out.extend(draws.iter().filter(|d| d.layer < 3).map(quad));
        out
    }

    /// (#35) The radar window's rectangle in the layout (`winGetScreenPosition`, `winGetSize` of `LeftHUD`), if the bar has
    /// one and it is shown.
    pub fn radar_window(&self) -> Option<[f32; 4]> {
        let w = self.radar?;
        if !self.shown(w, &BarState::default()) {
            return None;
        }
        let win = &self.wm.tree.windows[w];
        let (x, y) = self.wm.tree.screen_position(w);
        Some([x as f32, y as f32, (x + win.region.2 - win.region.0) as f32, (y + win.region.3 - win.region.1) as f32])
    }

    fn windows_into(&self, w: WinId, s: &BarState, out: &mut Vec<UiQuad>) {
        if !self.shown(w, s) {
            return;
        }
        let win: &GameWindow = &self.wm.tree.windows[w];
        let (x, y) = self.wm.tree.screen_position(w);
        let rect = [x as f32, y as f32, (x + win.region.2 - win.region.0) as f32, (y + win.region.3 - win.region.1) as f32];
        let mut q = UiQuad { rect, image: None, color: None, text: None, command: None, dim: false, clock: None, overlay: None };
        let enabled = win.status & win_status::ENABLED != 0;
        let dd = if enabled { &win.inst.enabled_draw_data[0] } else { &win.inst.disabled_draw_data[0] };
        if win.status & win_status::IMAGE != 0 {
            q.image = dd.image.clone();
        } else if win.status & win_status::SEE_THRU == 0 {
            q.color = color(dd.color);
        }
        if let Some(t) = &win.text {
            q.text = Some(String::from_utf16_lossy(t));
        }
        // (#34) a static text keeps its text in its own `TextData`
        let st = gadget::static_text_get_text(&self.wm, w);
        if !st.is_empty() {
            q.text = Some(String::from_utf16_lossy(&st));
        }
        if let Some(slot) = self.command_windows.iter().position(|&c| c == Some(w)) {
            if let Some(Some(c)) = s.commands.get(slot) {
                q.image = c.image.clone().or(q.image);
                q.command = Some(slot);
                q.dim = !c.enabled;
                q.clock = c.clock;
                if q.image.is_none() || self.images.find_image_by_name(q.image.as_deref().unwrap_or_default()).is_none() {
                    q.text = Some(String::from_utf16_lossy(&c.label));
                }
            }
        }
        if let Some(i) = self.queue_windows.iter().position(|&c| c == Some(w)) {
            if let Some(e) = s.queue.get(i) {
                q.image = e.image.clone().or(q.image);
                if i == 0 {
                    q.clock = s.queue_clock;
                }
            }
        }
        if Some(w) == self.money {
            q.text = Some(format_money(&self.money_format, s.money));
        }
        // (#35) the portrait in the cameo window, its chevron over it; the upgrade cameos, dimmed when not had
        if Some(w) == self.cameo && s.portrait.shown {
            q.image = s.portrait.image.clone();
            q.color = None;
            q.overlay = s.portrait.overlay.clone();
        }
        if let Some(u) = self.upgrade_cameos.iter().position(|&c| c == Some(w)) {
            if let Some(Some((image, enabled))) = s.portrait.upgrade_cameos.get(u) {
                q.image = image.clone();
                q.dim = !enabled;
            }
        }
        out.push(q);
        if Some(w) == self.power {
            // the power bar (new drawing): consumption against production
            let (p, c) = s.power;
            let frac = if p <= 0 { 0.0 } else { (c as f32 / p as f32).min(1.0) };
            let colr = if c > p { [220, 40, 30, 255] } else { [40, 200, 60, 255] };
            out.push(UiQuad { rect: [rect[0], rect[1], rect[0] + (rect[2] - rect[0]) * frac.max(0.05), rect[3]], image: None, color: Some(colr), text: None, command: None, dim: false, clock: None, overlay: None });
        }
        for &c in win.children.iter().rev() {
            self.windows_into(c, s, out);
        }
    }
}

/// (#35) Where the bar's layout sits on screen, as `app::game_ui` places it: the screen point of the layout's
/// `(0, y0)` (`y0` the top of the bar's quads) and the scale (screen pixels per layout pixel).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutPlacement {
    pub left: f32,
    pub top: f32,
    pub y0: f32,
    pub scale: f32,
}

impl LayoutPlacement {
    /// A layout rectangle `(x0, y0, x1, y1)` on screen.
    pub fn to_screen(&self, r: [f32; 4]) -> [f32; 4] {
        [self.left + r[0] * self.scale, self.top + (r[1] - self.y0) * self.scale, self.left + r[2] * self.scale, self.top + (r[3] - self.y0) * self.scale]
    }
}

/// The zoom column's height (~~4 buttons of 48 and 3 gaps of 6~~ **corrected 2026-11-06 (#4):** ~~5 buttons of 48 and 4
/// gaps of 6, the saves button added~~ **corrected 2026-11-06 (#62):** 6 buttons of 48 and 5 gaps of 6, the campaigns
/// button added, `app::setup_ui`) and its bottom margin.
const ZOOM_COLUMN_HEIGHT: f32 = 6.0 * 48.0 + 5.0 * 6.0 + 8.0;

/// (#35) The radar's window on screen (logical pixels, `(x0, y0, x1, y1)`): the control bar's `LeftHUD` where the bar is
/// drawn; else (no game files, or no such window) a box of the project's own at the right, just above the zoom column
/// (mobile first: 30 % of the screen's short side, 96 to 180 pixels wide, 4:3, inside an 8-pixel margin; never above the
/// status line's first line, y 36). ~~At the top right, below the status line~~ **corrected 2026-11-06 (#35):** the status
/// line wraps to four lines on a phone and the box covered it (the phone check's first screenshot).
pub fn radar_screen_rect(window: Option<[f32; 4]>, placement: Option<LayoutPlacement>, screen: (f32, f32)) -> [f32; 4] {
    if let (Some(r), Some(p)) = (window, placement) {
        return p.to_screen(r);
    }
    let w = (screen.0.min(screen.1) * 0.3).clamp(96.0, 180.0).min(screen.0 - 16.0);
    let h = (w * 0.75).round();
    let x1 = screen.0 - 8.0;
    let y0 = (screen.1 - ZOOM_COLUMN_HEIGHT - 8.0 - h).max(36.0);
    [x1 - w, y0, x1, y0 + h]
}

/// A synthetic control bar (no EA file): the mapped images, `ControlBar.wnd` and a scheme, as test files. `ui.tga` is a
/// 64x64 Targa in four colours. Since #35 it has the portrait windows (`WinUnitSelected` with `CameoWindow` and
/// `UnitUpgrade1`) and a portrait image, `ImgPortrait` (the green quadrant; ~~`ImgBack` the lower half~~ `ImgBack` is now the
/// lower-right quadrant).
pub fn synthetic_ui_files() -> Vec<(&'static [u8], Vec<u8>)> {
    let images = b"MappedImage ImgBar\n  Texture = ui.tga\n  TextureWidth = 64\n  TextureHeight = 64\n  Coords = Left:0 Top:0 Right:32 Bottom:32\n  Status = NONE\nEnd\n\
MappedImage ImgButton\n  Texture = ui.tga\n  TextureWidth = 64\n  TextureHeight = 64\n  Coords = Left:32 Top:0 Right:64 Bottom:32\n  Status = NONE\nEnd\n\
MappedImage ImgBack\n  Texture = ui.tga\n  TextureWidth = 64\n  TextureHeight = 64\n  Coords = Left:32 Top:32 Right:64 Bottom:64\n  Status = NONE\nEnd\n\
MappedImage ImgPortrait\n  Texture = ui.tga\n  TextureWidth = 64\n  TextureHeight = 64\n  Coords = Left:0 Top:32 Right:32 Bottom:64\n  Status = NONE\nEnd\n"
        .to_vec();
    let rect = |x0: i32, y0: i32, x1: i32, y1: i32| format!("SCREENRECT = UPPERLEFT: {x0} {y0}, BOTTOMRIGHT: {x1} {y1}, CREATIONRESOLUTION: 800 600;");
    let mut wnd = String::from("FILE_VERSION = 2;\nSTARTLAYOUTBLOCK\n  LAYOUTINIT = ControlBarInit;\n  LAYOUTUPDATE = ControlBarUpdate;\n  LAYOUTSHUTDOWN = [None];\nENDLAYOUTBLOCK\n");
    let dd = |img: &str| {
        let mut d = format!("IMAGE: {img}, COLOR: 40 40 60 255, BORDERCOLOR: 0 0 0 255");
        for _ in 0..8 {
            d.push_str(", IMAGE: NoImage, COLOR: 0 0 0 0, BORDERCOLOR: 0 0 0 0");
        }
        d
    };
    wnd += &format!("WINDOW\n WINDOWTYPE = USER;\n {}\n NAME = \"ControlBar.wnd:ControlBarParent\";\n STATUS = ENABLED+IMAGE;\n STYLE = USER;\n SYSTEMCALLBACK = \"ControlBarSystem\";\n ENABLEDDRAWDATA = {};\n CHILD\n", rect(0, 450, 800, 600), dd("ImgBar"));
    wnd += &format!("WINDOW\n WINDOWTYPE = STATICTEXT;\n {}\n NAME = \"ControlBar.wnd:MoneyDisplay\";\n STATUS = ENABLED;\n STATICTEXTDATA = CENTERED: 1;\nEND\n", rect(20, 460, 120, 480));
    // (#35) the radar's window
    wnd += &format!("WINDOW\n WINDOWTYPE = USER;\n {}\n NAME = \"ControlBar.wnd:LeftHUD\";\n STATUS = ENABLED;\n ENABLEDDRAWDATA = {};\nEND\n", rect(20, 500, 220, 590), dd("NoImage"));
    wnd += &format!("WINDOW\n WINDOWTYPE = USER;\n {}\n NAME = \"ControlBar.wnd:PowerWindow\";\n STATUS = ENABLED;\n ENABLEDDRAWDATA = {};\nEND\n", rect(20, 485, 220, 495), dd("NoImage"));
    wnd += &format!("WINDOW\n WINDOWTYPE = USER;\n {}\n NAME = \"ControlBar.wnd:CommandWindow\";\n STATUS = ENABLED+HIDDEN;\n SYSTEMCALLBACK = \"PassSelectedButtonsToParentSystem\";\n CHILD\n", rect(250, 470, 700, 590));
    for i in 0..3 {
        let x = 260 + i * 60;
        wnd += &format!("WINDOW\n WINDOWTYPE = PUSHBUTTON;\n {}\n NAME = \"ControlBar.wnd:ButtonCommand{:02}\";\n STATUS = ENABLED+IMAGE;\n STYLE = PUSHBUTTON+MOUSETRACK;\n ENABLEDDRAWDATA = {};\nEND\n", rect(x, 480, x + 50, 530), i + 1, dd("ImgButton"));
    }
    wnd += "ENDALLCHILDREN\nEND\n";
    // (#35) the portrait: `WinUnitSelected` holding `CameoWindow` and one upgrade cameo
    wnd += &format!("WINDOW\n WINDOWTYPE = USER;\n {}\n NAME = \"ControlBar.wnd:WinUnitSelected\";\n STATUS = ENABLED;\n CHILD\n", rect(705, 460, 795, 590));
    wnd += &format!("WINDOW\n WINDOWTYPE = USER;\n {}\n NAME = \"ControlBar.wnd:CameoWindow\";\n STATUS = ENABLED+IMAGE;\n ENABLEDDRAWDATA = {};\nEND\n", rect(710, 465, 790, 545), dd("NoImage"));
    wnd += &format!("WINDOW\n WINDOWTYPE = PUSHBUTTON;\n {}\n NAME = \"ControlBar.wnd:UnitUpgrade1\";\n STATUS = ENABLED+IMAGE;\n ENABLEDDRAWDATA = {};\nEND\n", rect(710, 550, 740, 580), dd("NoImage"));
    wnd += "ENDALLCHILDREN\nEND\nENDALLCHILDREN\nEND\n";
    let scheme = b"ControlBarScheme Default\n  ScreenCreationRes = X:800 Y:600\n  Side = Observer\n  MoneyUL = X:30 Y:455\n  MoneyLR = X:130 Y:475\n  PowerBarUL = X:20 Y:485\n  PowerBarLR = X:220 Y:495\n\
  ImagePart\n    Position = X:0 Y:440\n    Size = X:800 Y:160\n    ImageName = ImgBack\n    Layer = 5\n  End\nEnd\n"
        .to_vec();
    // a 64x64 32-bit uncompressed Targa, bottom-up: four 32x32 quadrants
    let mut tga = vec![0u8, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 64, 0, 32, 8];
    for y in (0..64).rev() {
        for x in 0..64 {
            let c: [u8; 4] = match (x < 32, y < 32) {
                (true, true) => [90, 60, 30, 255],
                (false, true) => [30, 120, 200, 255],
                (true, false) => [60, 170, 80, 255],
                _ => [40, 40, 40, 255],
            };
            tga.extend_from_slice(&c);
        }
    }
    vec![
        (b"Data\\INI\\MappedImages\\TextureSize_512\\ui.ini".as_slice(), images),
        (b"Window\\ControlBar.wnd".as_slice(), wnd.into_bytes()),
        (b"Data\\INI\\ControlBarScheme.ini".as_slice(), scheme),
        (b"Art\\Textures\\ui.tga".as_slice(), tga),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Controls;
    use rts_lockstep_engine::client::command_xlat::CommandXlatHost;
    use rts_lockstep_engine::common::base_type::ICoord2D;
    use rts_lockstep_engine::common::file_system::MemoryDevice;
    use rts_lockstep_engine::logic::world::game_data::TextTable;

    /// #12: the control bar from synthetic files, for the demo game's dozer. Predicted before the run:
    /// - it loads: ~~3 mapped images, ~~8 windows~~ **missed on the first run:** 7 windows (the parent, the money, the power,
    ///   the command window and its 3 buttons: a miscount in the prediction, not in the reader)~~ **since #35 (2026-11-06):**
    ///   4 mapped images and ~~10 windows~~ (with `WinUnitSelected`, `CameoWindow`, `UnitUpgrade1`) **since #35's radar
    ///   (2026-11-06): 11 windows, with `LeftHUD`**; the `Default` scheme is chosen for side `America` (none of that side);
    /// - nothing selected: the scheme's background (`ImgBack`, from y 440), the parent (`ImgBar`), the money
    ///   (`$ <money>`: no string file, so `GUI:ControlBarMoneyDisplay` is missing) and the power bar; no command button
    ///   (the command window is hidden without a selection);
    /// - the money display placed by the scheme's corners: (30, 455)..(130, 475);
    /// - the dozer selected: the command window and its first button, `Command_BuildDepot` in slot 0, clickable;
    ///   pressing it through `Controls::press` starts the Depot's placement;
    /// - (#35) nothing selected: no portrait quad (`WinUnitSelected` hidden); the dozer selected: the cameo window drawn with
    ///   `ImgPortrait`, no chevron, the upgrade cameo hidden (the dozer has none); the build button's popup "Build a Depot",
    ///   no cost (the gate's Depot costs nothing), and the demo's description;
    /// - the CRCs: building the bar's state every frame for 60 frames leaves the logic's CRCs as a plain run's.
    #[test]
    fn the_control_bar_draws_from_its_files() {
        let mut d = MemoryDevice::new();
        for (p, b) in synthetic_ui_files() {
            d.insert(p, b);
        }
        let mut fs = FileSystem::new(d);
        fs.init(None);
        let UiBuild::Ready(mut ui) = load_game_ui(&fs, &TextTable::default(), b"America") else { panic!("the bar did not load") };
        assert_eq!((ui.images.len(), ui.wm.tree.windows.len()), (4, 11));
        assert_eq!(ui.schemes.current_scheme().map(|s| s.name.clone()), Some(b"default".to_vec()));
        let money = ui.money.expect("the money window");
        assert_eq!(ui.wm.tree.screen_position(money), (30, 455));
        let (w, o) = crate::demo::demo_world();
        let mut c = Controls::new(w, 1);
        let (m, p) = c.client.money_and_power();
        let none = GameUi::bar_state(None, &mut c.client, Vec::new(), m, p);
        let q = ui.frame(&none);
        let images: Vec<Option<&[u8]>> = q.iter().map(|x| x.image.as_deref()).collect();
        assert_eq!(images[0], Some(&b"ImgBack"[..]));
        assert_eq!(images[1], Some(&b"ImgBar"[..]));
        assert!(q.iter().any(|x| x.text.as_deref() == Some(format!("$ {m}").as_str())), "{q:?}");
        assert!(q.iter().all(|x| x.command.is_none()));
        assert!(q.iter().all(|x| x.image.as_deref() != Some(&b"ImgPortrait"[..])), "no portrait without a selection");
        // select the dozer
        let dp = c.client.object_position(o.dozer);
        assert!(c.left_release(ICoord2D { x: 5, y: 5 }, ICoord2D { x: 5, y: 5 }, &[o.dozer], Some(o.dozer), dp));
        let sel = c.ui.first_selected_drawable();
        let st = GameUi::bar_state(sel, &mut c.client, Vec::new(), m, p);
        let slot = st.commands[0].clone().expect("the build button");
        assert_eq!(slot.button, o.build_depot);
        let q = ui.frame(&st);
        assert_eq!(q.iter().filter(|x| x.command.is_some()).count(), 1);
        let portrait = q.iter().find(|x| x.image.as_deref() == Some(&b"ImgPortrait"[..])).expect("the portrait");
        assert_eq!((portrait.rect, portrait.overlay.as_ref()), ([710.0, 465.0, 790.0, 545.0], None));
        assert!(st.portrait.upgrade_cameos.iter().all(Option::is_none));
        let tip = st.tooltips[0].as_ref().expect("the build button's popup");
        assert_eq!(tooltip_lines(tip), vec![crate::demo::DEMO_TEXT[0].1.to_string(), crate::demo::DEMO_TEXT[1].1.to_string()]);
        let b = rts_lockstep_engine::client::command_world::gui_button(&c.client.world, slot.button).expect("a button");
        c.press(&b);
        assert!(c.ui.pending_place_type().is_some());
        // the CRCs
        let (plain_w, _) = crate::demo::demo_world();
        let mut plain = Controls::new(plain_w, 1);
        let (w2, o2) = crate::demo::demo_world();
        let mut asked = Controls::new(w2, 1);
        let d2 = asked.client.object_position(o2.dozer);
        asked.left_release(ICoord2D { x: 5, y: 5 }, ICoord2D { x: 5, y: 5 }, &[o2.dozer], Some(o2.dozer), d2);
        plain.left_release(ICoord2D { x: 5, y: 5 }, ICoord2D { x: 5, y: 5 }, &[o2.dozer], Some(o2.dozer), d2);
        for f in 0..60 {
            for c in [&mut plain, &mut asked] {
                c.observe_sent();
                c.world().logic_update().expect("a frame");
            }
            let sel = asked.ui.first_selected_drawable();
            let q = asked.client.production_queue(o2.dozer);
            let (m, p) = asked.client.money_and_power();
            let st = GameUi::bar_state(sel, &mut asked.client, q, m, p);
            let _ = ui.frame(&st);
            // (#34) the manager's state and a pointer through it
            ui.sync(&st);
            let _ = ui.pointer(gwm::MOUSE_POS, (285.0, 505.0 + (f % 3) as f32), f as f64, false, &mut TooltipTimer::default());
            let crc = |c: &mut Controls| c.world().logic_crc(rts_lockstep_engine::logic::world::game_logic_update::CRC_RECALC, None).expect("a CRC");
            assert_eq!(crc(&mut asked), crc(&mut plain), "frame {f}");
        }
    }

    /// #34: the bar's input through the window manager, on the synthetic files with the demo's dozer selected. Predicted
    /// before the run from `winProcessMouseEvent` and `GadgetPushButtonInput`:
    /// - after `sync`, the command window and `ButtonCommand01` are shown (and enabled: the depot is available),
    ///   `ButtonCommand02` and `03` hidden;
    /// - the mouse over button 1 (285, 505): the popup's timer is over slot 0; a press and a release at 0.1 s select it:
    ///   `GBM_SELECTED` goes to the command window, which passes it to `ControlBarSystem`: slot 0 is pressed;
    /// - a touch held 0.7 s on it: no press (a long press), its popup pinned;
    /// - a press where the hidden button 2 is (345, 505): the command window takes nothing; no press;
    /// - a press dragged off the button before the release: no press;
    /// - with nothing selected after `sync`: the command window is hidden; the press on button 1's place presses
    ///   nothing.
    #[test]
    fn the_bar_is_pressed_through_the_window_manager() {
        let mut d = MemoryDevice::new();
        for (p, b) in synthetic_ui_files() {
            d.insert(p, b);
        }
        let mut fs = FileSystem::new(d);
        fs.init(None);
        let UiBuild::Ready(mut ui) = load_game_ui(&fs, &TextTable::default(), b"America") else { panic!("the bar did not load") };
        let (w, o) = crate::demo::demo_world();
        let mut c = Controls::new(w, 1);
        let (m, p) = c.client.money_and_power();
        let dp = c.client.object_position(o.dozer);
        assert!(c.left_release(ICoord2D { x: 5, y: 5 }, ICoord2D { x: 5, y: 5 }, &[o.dozer], Some(o.dozer), dp));
        let st = GameUi::bar_state(c.ui.first_selected_drawable(), &mut c.client, Vec::new(), m, p);
        ui.sync(&st);
        let win = |ui: &GameUi, i: usize| ui.command_windows[i].expect("a command window");
        assert!(!ui.wm.is_hidden(Some(win(&ui, 0))) && ui.wm.is_enabled(Some(win(&ui, 0))));
        assert!(ui.wm.is_hidden(Some(win(&ui, 1))) && ui.wm.is_hidden(Some(win(&ui, 2))));
        let mut tip = TooltipTimer::default();
        assert_eq!(ui.pointer(gwm::MOUSE_POS, (285.0, 505.0), 0.0, false, &mut tip), None);
        assert_eq!(tip.shown(0.6), Some(0));
        assert_eq!(ui.pointer(gwm::LEFT_DOWN, (285.0, 505.0), 0.0, false, &mut tip), None);
        assert_eq!(ui.pointer(gwm::LEFT_UP, (285.0, 505.0), 0.1, false, &mut tip), Some(0));
        assert_eq!(ui.pointer(gwm::LEFT_DOWN, (286.0, 506.0), 1.0, true, &mut tip), None);
        assert_eq!(ui.pointer(gwm::LEFT_UP, (286.0, 506.0), 1.7, true, &mut tip), None, "a long press");
        assert_eq!(tip.shown(1.8), Some(0));
        assert_eq!(ui.pointer(gwm::LEFT_DOWN, (345.0, 505.0), 2.0, false, &mut tip), None);
        assert_eq!(ui.pointer(gwm::LEFT_UP, (345.0, 505.0), 2.1, false, &mut tip), None);
        assert_eq!(ui.pointer(gwm::LEFT_DOWN, (285.0, 505.0), 3.0, false, &mut tip), None);
        assert_eq!(ui.pointer(gwm::MOUSE_POS, (400.0, 560.0), 3.05, false, &mut tip), None);
        assert_eq!(ui.pointer(gwm::LEFT_UP, (400.0, 560.0), 3.1, false, &mut tip), None, "released off the button");
        let none = GameUi::bar_state(None, &mut c.client, Vec::new(), m, p);
        ui.sync(&none);
        assert!(ui.wm.is_hidden(Some(win(&ui, 0))));
        assert_eq!(ui.pointer(gwm::LEFT_DOWN, (285.0, 505.0), 4.0, false, &mut tip), None);
        assert_eq!(ui.pointer(gwm::LEFT_UP, (285.0, 505.0), 4.1, false, &mut tip), None);
    }

    /// #12: the money's format. Predicted: `$%d` with 1500 is `$1500`; a missing label falls back to `$ 1500`.
    #[test]
    fn the_money_reads_as_the_game_formats_it() {
        let f: Vec<u16> = "$%d".encode_utf16().collect();
        assert_eq!(format_money(&f, 1500), "$1500");
        let m: Vec<u16> = "MISSING: 'GUI:ControlBarMoneyDisplay'".encode_utf16().collect();
        assert_eq!(format_money(&m, 1500), "$ 1500");
    }

    /// #12, the page's and the native client's check install (not a test of its own): with `RTS_UI_INSTALL_FROM` (a
    /// synthetic install, e.g. the terrain one) and `RTS_WRITE_UI_INSTALL` (a folder) set, writes that install with the
    /// synthetic control bar added: the mapped images and the scheme in `INIZH.big`, `Window\ControlBar.wnd` and the
    /// texture in a new `WindowZH.big`. Then checks the bar loads from it through the import's ranged art device.
    ///
    /// *Prediction:* ~~the bar waits at least once (the page's ranges)~~, then loads with ~~7~~ 10 windows (**since #35:**
    /// the portrait's three). **Missed on the first
    /// run (2026-11-05):** it did not wait: the synthetic `WindowZH.big` (20,556 bytes) and `INIZH.big` are smaller than
    /// the 64 KiB prefix the page sends, so every member was already there; it loaded with 7 windows and 3 images.
    #[test]
    fn write_the_ui_install_when_asked() {
        use crate::import::{start_from_files, Imported};
        use rts_lockstep_engine::formats::big::{build_big, ArchiveFile, FilenameList};
        let (Ok(from), Ok(to)) = (std::env::var("RTS_UI_INSTALL_FROM"), std::env::var("RTS_WRITE_UI_INSTALL")) else { return };
        let (from, to) = (std::path::Path::new(&from), std::path::Path::new(&to));
        std::fs::create_dir_all(to).expect("the folder");
        let ui = synthetic_ui_files();
        let (window, ini): (Vec<_>, Vec<_>) = ui.iter().partition(|(p, _)| p.starts_with(b"Window") || p.starts_with(b"Art"));
        let mut archives: Vec<(String, Vec<u8>)> = Vec::new();
        for e in std::fs::read_dir(from).expect("the install") {
            let n = e.expect("an entry").file_name().into_string().expect("a name");
            if !n.ends_with(".big") {
                continue;
            }
            let bytes = std::fs::read(from.join(&n)).expect("read");
            let bytes = if n == "INIZH.big" {
                let a = ArchiveFile::open_archive_file(n.as_bytes(), bytes).expect("an archive");
                let mut list = FilenameList::new();
                a.get_file_list_in_directory(b"", b"", b"*", &mut list, true);
                let mut members: Vec<(Vec<u8>, Vec<u8>)> = list.iter().map(|m| (m.to_vec(), a.open_file(m).expect("a member").to_vec())).collect();
                // (#35) the install's tank made selectable, with the synthetic portrait and a command set of two labelled
                // buttons, so the page can show a portrait and a popup (the labels are not in the install's string file: the
                // popup reads `MISSING: '...'`, as upstream shows a missing label)
                for (_, text) in members.iter_mut() {
                    let t = String::from_utf8_lossy(text).into_owned();
                    // (the tank's `KindOf` line, after its `Draw` block when the art install added one)
                    let kind = " KindOf = VEHICLE\n";
                    if let Some(i) = t.find("Object Tank\n").and_then(|o| t[o..].find(kind).map(|k| o + k)) {
                        let head = " KindOf = VEHICLE SELECTABLE\n SelectPortrait = ImgPortrait\n CommandSet = UiTankSet\n";
                        let mut patched = format!("{}{}{}", &t[..i], head, &t[i + kind.len()..]);
                        patched.push_str("CommandButton UiTank_Stop\n Command = STOP\n TextLabel = CONTROLBAR:Stop\n DescriptLabel = CONTROLBAR:TooltipStop\nEnd\n");
                        patched.push_str("CommandButton UiTank_Guard\n Command = GUARD\n TextLabel = CONTROLBAR:Guard\n DescriptLabel = CONTROLBAR:TooltipGuard\nEnd\n");
                        patched.push_str("CommandSet UiTankSet\n 1 = UiTank_Stop\n 2 = UiTank_Guard\nEnd\n");
                        *text = patched.into_bytes();
                    }
                }
                members.extend(ini.iter().map(|(p, b)| (p.to_vec(), b.clone())));
                let refs: Vec<(&[u8], &[u8])> = members.iter().map(|(p, b)| (p.as_slice(), b.as_slice())).collect();
                build_big(&refs)
            } else {
                bytes
            };
            archives.push((n, bytes));
        }
        let refs: Vec<(&[u8], &[u8])> = window.iter().map(|(p, b)| (*p, b.as_slice())).collect();
        archives.push(("WindowZH.big".into(), build_big(&refs)));
        for (n, b) in &archives {
            std::fs::write(to.join(n), b).expect("written");
        }
        let files = archives
            .iter()
            .enumerate()
            .map(|(key, (n, b))| {
                let mut q = b.len().min(64 * 1024);
                while ArchiveFile::directory_prefix_needed(b.len(), &b[..q]) > q {
                    q = ArchiveFile::directory_prefix_needed(b.len(), &b[..q]);
                }
                let ranged = ["W3D", "Textures", "Terrain", "Window"].iter().any(|p| n.starts_with(p));
                let f = if ranged { Imported::Ranged { key: key as u32, len: b.len(), prefix: b[..q].to_vec() } } else { Imported::Whole { key: key as u32, bytes: b.clone() } };
                (n.as_bytes().to_vec(), f)
            })
            .collect();
        let (s, mut fs) = start_from_files(files, None).expect("the game starts");
        let mut waits = 0;
        let ui = loop {
            match load_game_ui(&fs, &s.world.statics.game_text, b"America") {
                UiBuild::Ready(ui) => break ui,
                UiBuild::Missing(why) => panic!("{why}"),
                UiBuild::Waiting => {
                    waits += 1;
                    assert!(waits < 20, "still waiting");
                    for (key, o, l) in fs.local.device.take_wanted() {
                        let b = &archives[key as usize].1;
                        fs.local.device.provide(key, o, b[o..o + l].to_vec());
                    }
                }
            }
        };
        println!("ui install: waited {waits} times; {} windows, {} images", ui.wm.tree.windows.len(), ui.images.len());
        assert_eq!(ui.wm.tree.windows.len(), 10);
    }

    /// #35, when the popup shows. Predicted from [`TooltipTimer`]'s rules (new, not upstream's): a mouse over slot 2 shows
    /// nothing at 0.4 s and slot 2 at 0.5 s; leaving hides it; a quick click (0.1 s) presses the button and, from a mouse,
    /// pins nothing; a touch held 0.6 s shows the popup while held, its release does not press, and the popup stays pinned
    /// after the finger leaves; a quick tap on slot 0 presses it and pins slot 0 (the visible fallback); a reset clears it.
    #[test]
    fn a_popup_shows_on_hover_and_on_a_long_press() {
        let mut t = TooltipTimer::default();
        t.over(2, 10.0, false);
        assert_eq!((t.shown(10.4), t.shown(10.5)), (None, Some(2)));
        t.out(2);
        assert_eq!(t.shown(11.0), None);
        t.over(2, 20.0, false);
        t.press(2, 20.0);
        assert!(t.click(2, 20.1, false), "a quick click presses");
        t.out(2);
        assert_eq!(t.shown(21.0), None);
        t.over(1, 30.0, true);
        t.press(1, 30.0);
        assert_eq!(t.shown(30.6), Some(1), "held: shown");
        assert!(!t.click(1, 30.6, true), "a long press does not press");
        t.out(1);
        assert_eq!(t.shown(31.0), Some(1), "pinned after the finger lifts");
        t.over(0, 40.0, true);
        t.press(0, 40.0);
        assert_eq!(t.shown(40.05), None, "another touch unpins");
        assert!(t.click(0, 40.1, true));
        t.out(0);
        assert_eq!(t.shown(40.2), Some(0), "the tapped button's popup stays");
        t.reset();
        assert_eq!(t.shown(50.0), None);
    }

    /// #12, on the owner's copy of Zero Hour (`RTS_ZH_DIR`, a folder with its archives; skipped without it, never faked):
    /// the real `ControlBar.wnd`, mapped images and schemes load. *Prediction:* the bar loads, with the 18 command
    /// windows, the 9 queue windows, the money and the power found, and an `America` scheme chosen. *Result:* not run
    /// here (no copy of the game in this environment).
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_owners_control_bar_loads() {
        let Ok(dir) = std::env::var("RTS_ZH_DIR") else { return };
        let mut fs = FileSystem::new(rts_lockstep_engine::common::file_system::NativeDevice::new(std::path::Path::new(&dir)));
        fs.init(None);
        let UiBuild::Ready(ui) = load_game_ui(&fs, &TextTable::default(), b"America") else { panic!("the bar did not load") };
        println!("the owner's control bar: {} windows, {} images, scheme {:?}", ui.wm.tree.windows.len(), ui.images.len(), ui.schemes.current_scheme().map(|s| String::from_utf8_lossy(&s.name).into_owned()));
        assert!(ui.command_windows.iter().all(Option::is_some));
        assert!(ui.queue_windows.iter().all(Option::is_some));
        assert!(ui.money.is_some() && ui.power.is_some());
    }

    /// #35: where the radar is drawn. Predicted before the run: the synthetic bar's `LeftHUD` is (20, 500)..(220, 590) in
    /// the layout; on a 390 x 844 phone the bar is 390 wide (scale 0.4875) and its quads start at y 440, so with the bar at
    /// the bottom (its top at 844 - 160 * 0.4875 = 766) the radar is (9.75, 795.25)..(107.25, 839.125); without the bar the
    /// fallback box on the phone is 117 x 88 at the top right, (265, 36)..(382, 124), and on a 1280 x 720 screen 180 x 135.
    /// **Corrected (2026-11-06, after the phone check):** the box moved above the zoom column: on the phone (265, 530)..(382,
    /// 618) (844 - 218 - 8 - 88 = 530); on a screen 300 tall it stops at y 36.
    ///
    /// *Results (2026-11-06):* as predicted, first run; after the move, as predicted.
    #[test]
    fn the_radar_sits_in_left_hud_or_at_the_top_right() {
        let mut d = MemoryDevice::new();
        for (p, b) in synthetic_ui_files() {
            d.insert(p, b);
        }
        let mut fs = FileSystem::new(d);
        fs.init(None);
        let UiBuild::Ready(ui) = load_game_ui(&fs, &TextTable::default(), b"America") else { panic!("the bar did not load") };
        let r = ui.radar_window().expect("LeftHUD");
        assert_eq!(r, [20.0, 500.0, 220.0, 590.0]);
        let scale = 390.0 / 800.0;
        let p = LayoutPlacement { left: 0.0, top: 844.0 - 160.0 * scale, y0: 440.0, scale };
        assert_eq!(radar_screen_rect(Some(r), Some(p), (390.0, 844.0)), [9.75, 795.25, 107.25, 839.125]);
        // (~~[265, 530, 382, 618]~~ corrected 2026-11-06, #4: 54 higher, above the zoom column's fifth button; ~~[265, 476,
        // 382, 564]~~ corrected 2026-11-06, #62: 54 higher again, above the sixth, the campaigns button)
        assert_eq!(radar_screen_rect(None, None, (390.0, 844.0)), [265.0, 422.0, 382.0, 510.0]);
        assert_eq!(radar_screen_rect(None, None, (844.0, 300.0))[1], 36.0);
        let big = radar_screen_rect(None, None, (1280.0, 720.0));
        assert_eq!((big[2] - big[0], big[3] - big[1]), (180.0, 135.0));
    }
}
