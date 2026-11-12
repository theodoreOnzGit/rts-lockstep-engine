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
//! (`populate_build_tooltip_layout`): [`BarState`] carries them, ~~[`TooltipTimer`] decides when the popup shows (new: the
//! hover delay, and on a phone a long press, with the last button touched kept as a visible fallback).~~ **Corrected
//! 2026-11-11, #138:** the client's `TooltipTimer` is gone: the popup shows and hides as `TheControlBar`'s does
//! (`rts_lockstep_engine::client::build_tooltip`, differential-tested): the command buttons get `commandButtonTooltip`
//! (`setControlCommand`), the bar's window manager calls it from `winProcessMouseEvent` with the one mouse lent (whose
//! own tooltip it clears over the bar, as upstream's), the mouse's position goes to the manager every frame (upstream's
//! `MSG_RAW_MOUSE_POSITION`) and `ControlBar::update`'s part runs every frame ([`GameUi::popup_frame`]). Two things are
//! new, for a phone, which has no hover: the pointer stays where a finger lifted (as a mouse's would), so the popup of the
//! last button touched shows once its delay has passed and stays; and a touch held [`LONG_PRESS_MS`] or longer (a long
//! press) is released away from the button, so it does not press it, and its popup then shows. (While a button is held,
//! upstream's window manager calls no tooltip callback: its grab. So the popup cannot show during the press itself.)
//!
//! Since #34, the bar's windows live in the window manager (`rts_lockstep_engine::client::window_manager`, a translation
//! of `GameWindowManager`): [`GameUi::sync`] hides, shows, enables and disables them as `switchToContext` and
//! `setControlCommand` would from the [`BarState`], and [`GameUi::pointer`] sends the pointer through
//! `winProcessMouseEvent`, so a command button is pressed by the push button's own `GBM_SELECTED` reaching
//! `ControlBarSystem`. Touch is a mouse: a press moves the pointer there first. The drawing is still the quads above (moving
//! it onto the manager's draw list is #34's remaining work).
//!
//! Since #74, the keyboard reaches the manager ([`keyboard_into`], [`GameUi::keyboard`]): each key as DirectInput's
//! code through `winProcessKey`, and the text it typed as `WM_CHAR`s through the translated IME manager, so a text entry
//! with the focus is typed into, natively and in the browser (`app::keyboard_ui` feeds Bevy's keyboard events). While a
//! window has the focus the camera's keys are the UI's.
//!
//! Since #78, [`draw_list_quads`] turns a window manager's draw list (`winRepaint`'s `DrawItem`s) into the same quads, so
//! the shell (`crate::shell`) is drawn by the app's one quad drawing; the mapped images are read alone
//! ([`load_mapped_images`]) and shared between the shell and the bar ([`load_game_ui_with`]). The bar itself still draws
//! from [`GameUi::frame`].
//!
//! Since #109, the bar's options button is ControlBarCallback.cpp's `ToggleQuitMenu` (and the general's and the idle
//! worker's buttons its `HideQuitMenu`): [`GameUi::pointer`] names the call ([`QuitMenuCall`]) and the app makes it on the
//! shell's quit menu (`crate::shell::ShellUi::bar_quit_menu_call`): the phone's way into the quit menu, which has no
//! Escape key. The general's `togglePurchaseScience` and the idle worker's `selectNextIdleWorker` are not translated.
//!
//! Since #136, the bar hides, shows, lowers and raises as `TheControlBar`'s stages do
//! (`rts_lockstep_engine::client::control_bar_stage`, differential-tested): [`GameUi::toggle_control_bar`] is
//! `ToggleControlBar`, `ButtonLarge`'s `GBM_SELECTED` is `toggleControlBarStage` (the low stage leaves the bar's top tenth
//! of the layout showing; the scheme's layers move with `BackgroundMarker`). ~~Its windows' tooltips go to the one mouse~~
//! **Corrected 2026-11-11, #144:** the tooltip text of its windows without `commandButtonTooltip` goes to the one mouse
//! ([`GameUi::with_mouse`]), and their click sounds to the audio ([`GameUi::take_gui_sounds`]).
//!
//! Since #138, the bar's other `HideControlBar`/`ShowControlBar` callers: its load makes `createControlBar`'s, the end of
//! `GameEngine::init`'s and the end of `startNewGame`'s calls ([`load_game_ui_with`]), a script's letterbox reaches it as
//! the logic's event ([`GameUi::script_call`]), and the World's game cleared hides it ([`GameUi::clear_game_data`]). The
//! view's height the stages ask for is applied: [`tactical_view_height`] maps it onto the window.
//!
//! Since #144, `ControlBar::init`'s tooltip callbacks ([`GameUi::init_tooltips`],
//! `rts_lockstep_engine::client::build_tooltip::control_bar_init_tooltips`, differential-tested): the non-command push
//! buttons (`PopupCommunicator`, `ButtonOptions`, `ButtonIdleWorker`, `ButtonPlaceBeacon`, `ButtonGeneral`, `ButtonLarge`)
//! get their `NonCommand_*` commands and `commandButtonTooltip`, and the power, money and general's experience windows
//! `commandButtonTooltip`, so their popups show (the windows' ones through [`GameUi::window_popup`]) and their own tooltip
//! text no longer goes to `TheMouse`, as upstream's.
//!
//! The client still never changes the simulation: everything here reads the `World`.

use std::collections::BTreeMap;
use std::sync::Arc;

use rts_lockstep_engine::client::control_bar_command::{populate_build_queue, populate_command_with_hot_keys, CommandSlot, ControlBarStateHost, QueueSlot};
use rts_lockstep_engine::client::control_bar_popup::{populate_build_tooltip_layout, populate_window_tooltip, set_portrait_by_object, tooltip_window_of, PopupHost, PopupWindowHost, Portrait, PortraitHost, Tooltip, MAX_RIGHT_HUD_UPGRADE_CAMEOS};
use rts_lockstep_engine::client::control_bar_scheme::{ControlBarSchemeManager, SchemeDraw};
use rts_lockstep_engine::client::build_tooltip::{command_button_tooltip, command_gets_tooltip, control_bar_init_tooltips, BuildTooltip, BuildTooltipHost, ControlBarInitHost, COMMAND_BUTTON_TOOLTIP, GAME_COLOR_UNDEFINED, NON_COMMAND_BUTTONS};
use rts_lockstep_engine::logic::command_button::CommandButton;
use rts_lockstep_engine::client::control_bar_stage::{
    clear_game_data_control_bar, create_control_bar, game_engine_init_control_bar, script_control_bar, set_toggle_images, start_new_game_control_bar, toggle_control_bar,
    ControlBarStage, ControlBarStageHost, ControlBarStages, ToggleImage, BUTTON_LARGE,
};
use rts_lockstep_engine::client::image::ImageCollection;
use rts_lockstep_engine::client::gadget;
use rts_lockstep_engine::client::window_manager::{data_win, ggm, gwm, key, DrawItem, GameWindowManager, GuiHost, MsgHandled, NoGuiHost, WinInputReturnCode, WindowMsgData};
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
    /// (#78) Shared with the shell ([`load_game_ui_with`]).
    pub images: Arc<ImageCollection>,
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
    /// (#109) `ControlBar.wnd:ButtonOptions` (`ControlBarSystem`'s `buttonOptions`: `ToggleQuitMenu`).
    pub options_button: Option<WinId>,
    /// (#109) `ControlBar.wnd:ButtonGeneral` and `ButtonIdleWorker`, whose `GBM_SELECTED` first calls `HideQuitMenu`.
    pub hide_quit_menu_buttons: Vec<WinId>,
    /// (#109) The quit menu call the last pointer event's `GBM_SELECTED` made ([`GameUi::take_quit_menu_call`]).
    quit_menu_call: Option<QuitMenuCall>,
    /// (#148) `ControlBarSystem`'s `GBM_SELECTED` on `ControlBar.wnd:PopupCommunicator` (`ToggleDiplomacy(FALSE)`, the
    /// briefing history outside a multiplayer game): made by the app on the shell ([`GameUi::take_diplomacy_toggle`]).
    diplomacy_toggle: bool,
    /// (#136) `TheControlBar`'s stage: `m_currentControlBarStage`, `CP_MASTER` (`ControlBarParent`) and its default
    /// position (`rts_lockstep_engine::client::control_bar_stage`).
    pub stage: ControlBarStage,
    /// (#136) `ControlBar.wnd:ButtonLarge` (`ControlBarSystem`'s `buttonLargeID`: `toggleControlBarStage()`).
    pub button_large: Option<WinId>,
    /// (#136) `ControlBar.wnd:BackgroundMarker` and its screen position at load (`m_controlBarBackgroundMarkerPos`, which
    /// is also the foreground's): the scheme's layers move with it (`W3DCommandBarBackgroundDraw`'s offset).
    pub background_marker: Option<(WinId, (i32, i32))>,
    /// (#136) `TheRecorder`'s playback, as the application last set it: the stages do not switch during one.
    pub recorder_playback: bool,
    /// (#136) The tactical view's height the stages last asked for (`TheTacticalView->setHeight`), in the layout's pixels.
    /// ~~The client's view keeps the whole window, the bar drawn over it, so nothing reads it but the tests.~~ **Corrected
    /// 2026-11-11, #138:** the client's view takes it ([`tactical_view_height`] maps it onto the window).
    pub view_height: Option<i32>,
    /// (#138) `TheControlBar`'s popup state (`m_showBuildToolTipLayout`, `prevWindow`, ...).
    pub build_tooltip: BuildTooltip,
    /// (#138) `m_buildToolTipLayout`'s hidden bit (`ControlBar::init` hides it).
    pub popup_hidden: bool,
    /// (#138) The last `populateBuildTooltipLayout(commandButton, tooltipWin)`: the command's button (its place in the
    /// World's list), the window.
    pub popup: Option<(Option<usize>, Option<WinId>)>,
    /// (#138) Each command slot's button (`GadgetButtonGetData`), as the last [`GameUi::sync`] set them.
    slot_buttons: Vec<Option<usize>>,
    /// (#138) When the touch now held went down (`PopupWorld::now_ms`), for the long press ([`LONG_PRESS_MS`]).
    touch_press_ms: Option<u32>,
    /// (#136) The window code's sounds (`GUIClick` ...) since [`GameUi::take_gui_sounds`].
    gui_sounds: Vec<Vec<u8>>,
    /// (#144) The non-command push buttons' commands (`ControlBar::init`'s `setControlCommand`: `GadgetButtonSetData`),
    /// by window: each `NonCommand_*` button's place in the World's list.
    pub non_command_buttons: Vec<(WinId, usize)>,
    /// (#144) How many times `populateBuildTooltipLayout` was called: the application refills the popup's text when it
    /// changes (upstream fills it at that call, and the text then stays as it was filled).
    pub popup_fills: u32,
}

/// (#136) [`ControlBarStageHost`] on the bar's window manager and scheme. The display is the bar's 800x600 layout; the
/// client has no replay controls window (`ReplayControl.wnd` is not loaded), no special power shortcut bar and no
/// animate window manager~~, and rebuilds the popup and the buttons every frame ([`GameUi::bar_state`])~~: those calls do
/// nothing. **Corrected 2026-11-11, #138:** `repopulateBuildTooltipLayout` is made (on [`GameUi::build_tooltip`]).
struct StageHost<'a> {
    wm: &'a mut GameWindowManager,
    schemes: &'a ControlBarSchemeManager,
    images: &'a ImageCollection,
    playback: bool,
    view_height: &'a mut Option<i32>,
    /// (#138) `repopulateBuildTooltipLayout()` was asked for (made after the stage's call: [`GameUi::with_stage`]).
    repopulate: &'a mut bool,
}

impl ControlBarStageHost for StageHost<'_> {
    fn win_from_name(&self, name: &[u8]) -> Option<WinId> {
        self.wm.tree.find_by_name(name)
    }
    fn win_is_hidden(&self, w: WinId) -> bool {
        self.wm.tree.windows[w].is_hidden()
    }
    fn win_hide(&mut self, w: WinId, hide: bool) {
        self.wm.win_hide(w, hide);
    }
    fn win_get_position(&self, w: WinId) -> (i32, i32) {
        let r = self.wm.tree.windows[w].region;
        (r.0, r.1)
    }
    fn win_set_position(&mut self, w: WinId, x: i32, y: i32) {
        self.wm.win_set_position(w, x, y);
    }
    /// `updateUpDownImages`' images are the side's scheme's (`ToggleButtonUpIn` ...), by name.
    fn gadget_button_set_images(&mut self, w: WinId, enabled: ToggleImage, hilite: ToggleImage, hilite_selected: ToggleImage) {
        let scheme = self.schemes.current_scheme();
        let img = |t: ToggleImage| scheme.and_then(|c| c.images.get(t.scheme_field())).and_then(|n| self.images.find_image_by_name(n)).map(|i| i.name.clone());
        let images = [img(enabled), img(hilite), img(hilite_selected)];
        set_toggle_images(&mut self.wm.tree, w, images);
    }
    fn display_height(&self) -> u32 {
        LAYOUT.1 as u32
    }
    fn set_tactical_view_height(&mut self, h: i32) {
        *self.view_height = Some(h);
    }
    fn recorder_playback(&self) -> bool {
        self.playback
    }
    fn is_in_replay_game(&self) -> bool {
        false
    }
    fn replay_window(&self) -> Option<WinId> {
        None
    }
    fn show_special_power_shortcut(&mut self) {}
    fn hide_special_power_shortcut(&mut self) {}
    fn animate_special_power_shortcut(&mut self, _on: bool) {}
    fn repopulate_build_tooltip_layout(&mut self) {
        *self.repopulate = true;
    }
    fn mark_ui_dirty(&mut self) {}
    fn has_animate_window_manager(&self) -> bool {
        false
    }
    fn animate_reset(&mut self) {}
    fn animate_register_slide_bottom(&mut self, _w: WinId) {}
    fn animate_reverse(&mut self) {}
}

/// (#109) What `ControlBarSystem`'s `GBM_SELECTED` asks of the quit menu (ControlBarCallback.cpp): `ToggleQuitMenu()` for
/// the options button, `HideQuitMenu()` before the general's (`togglePurchaseScience`) and the idle worker's
/// (`selectNextIdleWorker`) own calls. The quit menu is the shell's (`crate::shell::ShellUi`): another window manager,
/// so the bar names the call and the app makes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuitMenuCall {
    Toggle,
    Hide,
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
    load_game_ui_with(fs, text, side, None)
}

/// (#78) What [`load_mapped_images`] found.
pub enum ImagesBuild {
    Ready(Arc<ImageCollection>),
    /// A file's bytes are still being fetched (the web's ranged reads): ask again later.
    Waiting,
    Missing(String),
}

/// (#78) The mapped images alone (`TheMappedImageCollection->load(512)`, `Data\INI\MappedImages`), shared: upstream has
/// one collection for every window, the shell's and the control bar's (`TheMappedImageCollection`). Reads only.
pub fn load_mapped_images<D: LocalDevice>(fs: &FileSystem<D>) -> ImagesBuild {
    let reads = Reads { fs, waiting: std::cell::Cell::new(false) };
    let mut images = ImageCollection::new();
    let loaded = images.load(512, &reads);
    if reads.waiting.get() {
        return ImagesBuild::Waiting;
    }
    match loaded {
        Ok(_) => ImagesBuild::Ready(Arc::new(images)),
        Err(e) => ImagesBuild::Missing(format!("the mapped images did not load: {e:?}")),
    }
}

/// [`load_game_ui`] with (#78) the mapped images already read (the shell's, [`load_mapped_images`]) when given, so the
/// shell and the bar share one collection, as upstream's windows share `TheMappedImageCollection`.
pub fn load_game_ui_with<D: LocalDevice>(fs: &FileSystem<D>, text: &impl GameTextFetch, side: &[u8], shared: Option<Arc<ImageCollection>>) -> UiBuild {
    let reads = Reads { fs, waiting: std::cell::Cell::new(false) };
    let images = match shared {
        Some(i) => i,
        None => match load_mapped_images(fs) {
            ImagesBuild::Ready(i) => i,
            ImagesBuild::Waiting => return UiBuild::Waiting,
            ImagesBuild::Missing(why) => return UiBuild::Missing(why),
        },
    };
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
        options_button: find("ControlBar.wnd:ButtonOptions".into()),
        hide_quit_menu_buttons: ["ButtonGeneral", "ButtonIdleWorker"].iter().filter_map(|n| find(format!("ControlBar.wnd:{n}"))).collect(),
        quit_menu_call: None,
        diplomacy_toggle: false,
        stage: ControlBarStage::default(),
        button_large: find(String::from_utf8_lossy(BUTTON_LARGE).into_owned()),
        background_marker: find("ControlBar.wnd:BackgroundMarker".into()).map(|w| (w, wm.tree.screen_position(w))),
        recorder_playback: false,
        view_height: None,
        build_tooltip: BuildTooltip::default(),
        popup_hidden: true,
        popup: None,
        slot_buttons: Vec::new(),
        touch_press_ms: None,
        gui_sounds: Vec::new(),
        non_command_buttons: Vec::new(),
        popup_fills: 0,
        images,
        wm,
        schemes,
    };
    let mut ui = ui;
    // (#136) `ControlBar::init`'s `CP_MASTER` and its default position; (#138) `InGameUI::createControlBar`'s
    // `HideControlBar()` and the end of `GameEngine::init`'s; then `ControlBarScheme::init`'s
    // `switchControlBarStage(CONTROL_BAR_STAGE_DEFAULT)` when the side's scheme is set (the "down" images on `ButtonLarge`);
    // (#138) then the end of `GameLogic::startNewGame`: a game, not the shell's, so `ShowControlBar(FALSE)`
    ui.with_stage(|stage, h| {
        *stage = ControlBarStage::new(h);
        create_control_bar(stage, h);
        game_engine_init_control_bar(stage, h);
        stage.switch_control_bar_stage(ControlBarStages::Default as i32, h);
        start_new_game_control_bar(stage, false, h);
    });
    // (#144) `ControlBar::init`'s tooltip callbacks (ControlBar.cpp:1190-1240): the bar's windows get
    // `commandButtonTooltip`; the `NonCommand_*` buttons are the World's, given by the application once it has the World
    // ([`GameUi::init_tooltips`] again): none here
    ui.init_tooltips(|_| None);
    UiBuild::Ready(Box::new(ui))
}

/// (#34) The bar's own callbacks for the manager (new: upstream's `ControlBarSystem` calls
/// `processContextSensitiveButtonClick`; here the button `GBM_SELECTED` names is kept for the caller to press).
#[derive(Default)]
struct BarHost {
    selected: Option<WinId>,
    /// (#136) The window code's sounds (`TheAudio->addAudioEvent`: the push button's `GUIClick`, `GUIClickDisabled`).
    sounds: Vec<Vec<u8>>,
    /// (#138) The windows whose `commandButtonTooltip` the manager called, in order.
    tooltips: Vec<WinId>,
}

impl GuiHost for BarHost {
    fn add_audio_event(&mut self, name: &[u8]) {
        self.sounds.push(name.to_vec());
    }
    /// (#138) `winProcessMouseEvent`'s tooltip callback: `commandButtonTooltip` ([`GameUi::pointer`] makes it).
    fn tooltip(&mut self, _wm: &mut GameWindowManager, func: &[u8], win: WinId, _mouse: u32) {
        if func == COMMAND_BUTTON_TOOLTIP {
            self.tooltips.push(win);
        }
    }
    fn system(&mut self, _wm: &mut GameWindowManager, func: &[u8], _win: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
        if func == b"ControlBarSystem" && msg == ggm::GBM_SELECTED {
            self.selected = data_win(d1);
            return MsgHandled::Handled;
        }
        MsgHandled::Ignored
    }
}

/// (#74) A key of the keyboard into a window manager, as the game receives it: the key itself as DirectInput reports it
/// (`scan`, a `KeyDefs.h` code; 0 for none) through `winProcessKey` (down or up), then, for a press, the text it typed
/// as `WM_CHAR`s through the IME manager (`client::ime_manager::type_text`: one per UTF-16 unit; the manager takes 32
/// and above, and return, for the text entry it is attached to). New glue: upstream gets the two from DirectInput and
/// from `WndProc`; here both come from one Bevy keyboard event, natively and in the browser. Whether either was used
/// (`WIN_INPUT_USED`, or a character eaten).
pub fn keyboard_into<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, scan: u8, down: bool, text: Option<&str>) -> bool {
    let state = if down { key::STATE_DOWN } else { key::STATE_UP };
    let mut used = scan != key::NONE && wm.win_process_key(h, scan, state) == WinInputReturnCode::Used;
    if down {
        if let Some(t) = text {
            used |= rts_lockstep_engine::client::ime_manager::type_text(wm, h, t) > 0;
        }
    }
    used
}

/// One thing to draw, in the layout's space.
#[derive(Clone, Debug, Default, PartialEq)]
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
    /// (#78) The image's colour (`drawImage`'s `color`, as RGBA) when it is not opaque white.
    pub tint: Option<[u8; 4]>,
    /// (#78) The part of the image shown, as fractions of it `(u0, v0, u1, v1)`: an image cut by its clip region.
    pub uv: Option<[f32; 4]>,
    /// (#78) The text's colour (white when `None`).
    pub text_color: Option<[u8; 4]>,
    /// (#78) The text's height in layout pixels (the bar's own size when `None`).
    pub font_px: Option<f32>,
    /// (#78) The text starts at the quad's top left, where the window's draw placed it (`DisplayString::draw(x, y)`),
    /// rather than centred; it wraps at the quad's width only when `text_wrap`.
    pub text_top_left: bool,
    pub text_wrap: bool,
    /// (#78) A line not along an axis (`drawLine`): the quad is the line laid flat about its centre, turned clockwise by
    /// the angle whose `(sin, cos)` this is.
    pub rotation: Option<(f32, f32)>,
}

/// `WIN_DRAW_LINE_WIDTH` (W3DGadget.h, 1.0): the width every outline and line of the windows' draws is given upstream
/// (each `winOpenRect` and `drawOpenRect` caller passes it or a literal 1, checked 2026-11-10), so a [`DrawItem`] does not
/// carry one (#82).
pub const WIN_DRAW_LINE_WIDTH: f32 = 1.0;

/// An `ARGB` colour (`GameMakeColor`) as RGBA.
fn rgba(c: u32) -> [u8; 4] {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8]
}

/// `(x0, y0, x1, y1)` cut by a clip region `(x0, y0, x1, y1)`, with the fractions of the original kept: `None` when
/// nothing is left.
fn clipped(r: [f32; 4], clip: Option<(i32, i32, i32, i32)>) -> Option<([f32; 4], Option<[f32; 4]>)> {
    let Some((cx0, cy0, cx1, cy1)) = clip else { return Some((r, None)) };
    let c = [r[0].max(cx0 as f32), r[1].max(cy0 as f32), r[2].min(cx1 as f32), r[3].min(cy1 as f32)];
    if c[2] <= c[0] || c[3] <= c[1] {
        return None;
    }
    if c == r {
        return Some((r, None));
    }
    let (w, h) = (r[2] - r[0], r[3] - r[1]);
    Some((c, Some([(c[0] - r[0]) / w, (c[1] - r[1]) / h, (c[2] - r[0]) / w, (c[3] - r[1]) / h])))
}

/// (#78) A window manager's draw list (`winRepaint`'s [`DrawItem`]s, the transitions' draws last) as the quads the app
/// draws the control bar with, in the same order (the first underneath). New glue in place of `W3DDisplay`'s 2D
/// renderer:
///
/// - `drawFillRect`: a quad of its colour;
/// - `drawOpenRect`: its four edges, [`WIN_DRAW_LINE_WIDTH`] wide, inside the rectangle (`Render2DClass::Add_Outline`
///   on `RectClass(x, y, x + w, y + h)`);
/// - `drawLine`: a quad one line wide along it (turned when it is not along an axis);
/// - `drawImage`: the mapped image over its rectangle, cut by the clip region it was drawn under, tinted by its colour;
///   a grayscale draw is drawn dimmed (the app has no grayscale shader);
/// - `DisplayString::draw`: the text from its top left, in its colour, its box measured by the host
///   (`GuiHost::text_size`), cut by its clip region; the drop shadow is not drawn;
/// - `drawRectClock`: the quad's clock (the bar's darkened part), the remaining part for an inverse clock.
pub fn draw_list_quads<H: GuiHost>(items: &[DrawItem], host: &H) -> Vec<UiQuad> {
    let mut out = Vec::new();
    let lw = WIN_DRAW_LINE_WIDTH;
    for d in items {
        match d {
            DrawItem::FillRect { x, y, w, h, color: c } => {
                out.push(UiQuad { rect: [*x as f32, *y as f32, (x + w) as f32, (y + h) as f32], color: color(*c), ..Default::default() });
            }
            DrawItem::OpenRect { x, y, w, h, color: c } => {
                let (x0, y0, x1, y1) = (*x as f32, *y as f32, (x + w) as f32, (y + h) as f32);
                for rect in [[x0, y0, x1, y0 + lw], [x0, y1 - lw, x1, y1], [x0, y0, x0 + lw, y1], [x1 - lw, y0, x1, y1]] {
                    out.push(UiQuad { rect, color: color(*c), ..Default::default() });
                }
            }
            DrawItem::Line { x0, y0, x1, y1, color: c } => {
                let (dx, dy) = ((x1 - x0) as f32, (y1 - y0) as f32);
                let (mx, my) = ((x0 + x1) as f32 / 2.0, (y0 + y1) as f32 / 2.0);
                let q = if dy == 0.0 {
                    UiQuad { rect: [mx - dx.abs() / 2.0, my - lw / 2.0, mx + dx.abs() / 2.0, my + lw / 2.0], ..Default::default() }
                } else if dx == 0.0 {
                    UiQuad { rect: [mx - lw / 2.0, my - dy.abs() / 2.0, mx + lw / 2.0, my + dy.abs() / 2.0], ..Default::default() }
                } else {
                    let l = rts_lockstep_engine::ww::wwmath::sqrt(dx * dx + dy * dy);
                    UiQuad { rect: [mx - l / 2.0, my - lw / 2.0, mx + l / 2.0, my + lw / 2.0], rotation: Some((dy / l, dx / l)), ..Default::default() }
                };
                out.push(UiQuad { color: color(*c), ..q });
            }
            DrawItem::Image { image, x0, y0, x1, y1, color: c, grayscale, clip } => {
                let Some((rect, uv)) = clipped([*x0 as f32, *y0 as f32, *x1 as f32, *y1 as f32], *clip) else { continue };
                let tint = (*c != 0xFFFF_FFFF).then(|| rgba(*c));
                out.push(UiQuad { rect, image: Some(image.clone()), uv, tint, dim: *grayscale, ..Default::default() });
            }
            DrawItem::Text { text, x, y, color: c, font, word_wrap, clip, .. } => {
                if text.is_empty() {
                    continue;
                }
                let (w, h) = host.text_size(font.as_ref(), text, *word_wrap);
                let Some((rect, _)) = clipped([*x as f32, *y as f32, (x + w) as f32, (y + h) as f32], *clip) else { continue };
                out.push(UiQuad {
                    rect,
                    text: Some(String::from_utf16_lossy(text)),
                    text_color: Some(rgba(*c as u32)),
                    font_px: Some(host.font_height(font.as_ref()) as f32),
                    text_top_left: true,
                    text_wrap: word_wrap.is_some_and(|w| w > 0),
                    ..Default::default()
                });
            }
            DrawItem::RectClock { x, y, w, h, percent, inverse, .. } => {
                let pct = if *inverse { 100 - percent } else { *percent };
                out.push(UiQuad { rect: [*x as f32, *y as f32, (x + w) as f32, (y + h) as f32], clock: Some(pct), ..Default::default() });
            }
        }
    }
    out
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
    /// (#131) The hot keys `setControlCommand` adds, `(slot, key)` in order
    /// (`control_bar_command::populate_command_with_hot_keys`).
    pub hot_keys: Vec<(usize, Vec<u8>)>,
    /// (#138) Whether each command window gets `commandButtonTooltip` (`setControlCommand`: a text label or sciences).
    pub tooltip_funcs: Vec<bool>,
}

/// ~~`TooltipTimer`: when the popup of a command button shows (new: the hover delay, a phone's long press and the last
/// button touched kept as a visible fallback).~~ **Corrected 2026-11-11, #138:** removed; the popup's timing is
/// `TheControlBar`'s (`rts_lockstep_engine::client::build_tooltip`, [`GameUi::popup_frame`]). What the popup's timing reads
/// outside the bar this frame (#138).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PopupWorld {
    /// `timeGetTime()`: the client's clock in milliseconds.
    pub now_ms: u32,
    /// `TheInGameUI->areTooltipsDisabled()` (the client never disables them: #138's gaps).
    pub tooltips_disabled: bool,
    /// `TheScriptEngine->isGameEnding()`, the World's.
    pub game_ending: bool,
    /// `TheGameLogic->isInReplayGame()`.
    pub replay_game: bool,
    /// `TheInGameUI->isQuitMenuVisible()`.
    pub quit_menu_visible: bool,
}

/// (#138, new: a phone has no hover) A touch held this long on a button is a long press: its release does not press the
/// button ([`GameUi::pointer`]); the popup shows after it, the pointer staying there.
pub const LONG_PRESS_MS: u32 = 500;

/// (#138) [`BuildTooltipHost`] on the bar's windows, its popup layout and the frame's [`PopupWorld`].
struct PopupBar<'a> {
    wm: &'a GameWindowManager,
    command_windows: &'a [Option<WinId>],
    slot_buttons: &'a [Option<usize>],
    /// (#144) [`GameUi::non_command_buttons`].
    non_command_buttons: &'a [(WinId, usize)],
    /// (#144) [`GameUi::popup_fills`].
    fills: &'a mut u32,
    hidden: &'a mut bool,
    popup: &'a mut Option<(Option<usize>, Option<WinId>)>,
    world: PopupWorld,
}

impl BuildTooltipHost for PopupBar<'_> {
    fn tooltips_disabled(&self) -> bool {
        self.world.tooltips_disabled
    }
    fn is_game_ending(&self) -> bool {
        self.world.game_ending
    }
    fn time_get_time(&self) -> u32 {
        self.world.now_ms
    }
    fn win_style(&self, w: WinId) -> u32 {
        self.wm.tree.windows[w].inst.style
    }
    fn tooltip_delay(&self, w: WinId) -> i32 {
        self.wm.tree.windows[w].inst.tooltip_delay
    }
    fn gadget_button_command(&self, w: WinId) -> Option<usize> {
        // (#144) a non-command button's, set by `ControlBar::init`
        if let Some(&(_, c)) = self.non_command_buttons.iter().find(|(nw, _)| *nw == w) {
            return Some(c);
        }
        let slot = self.command_windows.iter().position(|&c| c == Some(w))?;
        self.slot_buttons.get(slot).copied().flatten()
    }
    fn is_in_replay_game(&self) -> bool {
        self.world.replay_game
    }
    fn is_quit_menu_visible(&self) -> bool {
        self.world.quit_menu_visible
    }
    /// (the client has no disconnect screen)
    fn disconnect_menu_visible(&self) -> bool {
        false
    }
    fn populate_build_tooltip_layout(&mut self, command: Option<usize>, tooltip_win: Option<WinId>) {
        *self.popup = Some((command, tooltip_win));
        *self.fills = self.fills.wrapping_add(1);
    }
    fn layout_is_hidden(&self) -> bool {
        *self.hidden
    }
    fn layout_hide(&mut self, hide: bool) {
        *self.hidden = hide;
    }
}

/// (#144) [`ControlBarInitHost`] on the bar's window manager: `ControlBar::init`'s tooltip part and `setControlCommand`
/// for the non-command buttons. The border colours are `ControlBar::ControlBar`'s (`GAME_COLOR_UNDEFINED`: a scheme's
/// `init` sets them later, not modelled); there is no `TheHotKeyManager` here (the bar's hot keys are the
/// application's, and upstream's `switchToContext` resets the manager before any key is used: #144's gaps).
struct BarInitHost<'a> {
    wm: &'a mut GameWindowManager,
    images: &'a ImageCollection,
    /// `findCommandButton`'s answers for the `NonCommand_*` names: the name, the button's place, the button.
    found: Vec<(&'static [u8], usize, CommandButton)>,
    data: &'a mut Vec<(WinId, usize)>,
}

impl BarInitHost<'_> {
    fn button(&self, c: usize) -> Option<&CommandButton> {
        self.found.iter().find(|(_, i, _)| *i == c).map(|(_, _, b)| b)
    }
}

impl ControlBarInitHost for BarInitHost<'_> {
    fn win_get_window_from_id(&self, name: &[u8]) -> Option<WinId> {
        self.wm.tree.find_by_name(name)
    }
    fn find_command_button(&self, name: &[u8]) -> Option<usize> {
        self.found.iter().find(|(n, _, _)| *n == name).map(|(_, i, _)| *i)
    }
    fn win_set_tooltip_func(&mut self, w: WinId) {
        self.wm.ext[w].tooltip = Some(COMMAND_BUTTON_TOOLTIP.to_vec());
    }
    fn is_push_button_input(&self, w: WinId) -> bool {
        self.wm.ext[w].input == rts_lockstep_engine::client::window_manager::InputFunc::PushButton
    }
    fn command_options(&self, c: usize) -> u32 {
        self.button(c).map_or(0, |b| b.options)
    }
    /// `getButtonImage()`: the `ButtonImage` found in the mapped images (`cacheButtonImage`).
    fn command_button_image(&self, c: usize) -> Option<Vec<u8>> {
        let name = &self.button(c)?.images[0];
        (!name.is_empty() && self.images.find_image_by_name(name).is_some()).then(|| name.clone())
    }
    fn command_text_label(&self, c: usize) -> Vec<u8> {
        self.button(c).map(|b| b.labels[0].clone()).unwrap_or_default()
    }
    fn command_has_sciences(&self, c: usize) -> bool {
        self.button(c).is_some_and(|b| !b.science.is_empty())
    }
    fn command_border(&self, c: usize) -> i32 {
        self.button(c).map_or(0, |b| b.border)
    }
    fn gadget_button_enable_check_like(&mut self, w: WinId, check_like: bool, checked: bool) {
        rts_lockstep_engine::client::gadget::button_enable_check_like(self.wm, w, check_like, checked);
    }
    fn gadget_button_set_enabled_image(&mut self, w: WinId, image: Vec<u8>) {
        rts_lockstep_engine::client::control_bar_scheme::gadget_button_set_enabled_image(&mut self.wm.tree, w, Some(image));
    }
    fn gadget_button_clear_text(&mut self, w: WinId) {
        rts_lockstep_engine::client::gadget::button_set_text(self.wm, &mut NoGuiHost, Some(w), &[]);
    }
    fn gadget_button_set_data(&mut self, w: WinId, c: usize) {
        self.data.retain(|(dw, _)| *dw != w);
        self.data.push((w, c));
    }
    fn gadget_button_set_border(&mut self, w: WinId, color: i32, draw: bool) {
        rts_lockstep_engine::client::gadget::button_set_border(self.wm, w, color, draw);
    }
    fn border_colors(&self) -> [i32; 4] {
        [GAME_COLOR_UNDEFINED; 4]
    }
    fn search_hot_key(&mut self, _label: &[u8]) -> Option<Vec<u8>> {
        None
    }
    fn add_hot_key(&mut self, _w: WinId, _key: Vec<u8>) {}
    fn gadget_button_set_alt_sound(&mut self, w: WinId, sound: &[u8]) {
        rts_lockstep_engine::client::gadget::button_set_alt_sound(self.wm, w, sound);
    }
}

/// (#138) The tactical view's height on the window (logical pixels) for the height the stages asked for in the bar's
/// layout (`TheTacticalView->setHeight`, out of the layout's 600): where that line of the layout is on the window
/// (`placement`, the bar as drawn), so the view ends where upstream's ends above the bar (80 % of a 4:3 window, and on a
/// phone, whose bar is scaled to its width, the same line of the bar); without a bar on the window, that share of the
/// window; the whole window when nothing was asked. New glue: upstream's display is the layout's own size.
pub fn tactical_view_height(view_height: Option<i32>, placement: Option<LayoutPlacement>, window_height: f32) -> f32 {
    let h = match (view_height, placement) {
        (None, _) => window_height,
        (Some(v), Some(p)) => p.top + (v as f32 - p.y0) * p.scale,
        (Some(v), None) => window_height * v as f32 / LAYOUT.1 as f32,
    };
    // (`WWMath::Clamp`, function syntax)
    let top = if window_height > 1.0 { window_height } else { 1.0 };
    rts_lockstep_engine::ww::wwmath::clamp(h, 1.0, top)
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
        let (commands, hot_keys) = selected.map(|o| populate_command_with_hot_keys(o, h)).map(|(c, k)| (c.to_vec(), k)).unwrap_or_default();
        let tooltips = commands.iter().map(|c| c.as_ref().and_then(|c| h.button(c.button)).map(|b| populate_build_tooltip_layout(&b, h))).collect();
        let tooltip_funcs = commands.iter().map(|c| c.as_ref().and_then(|c| h.button(c.button)).is_some_and(|b| command_gets_tooltip(b.labels[0].is_empty(), !b.science.is_empty()))).collect();
        let portrait = set_portrait_by_object(selected, h);
        let (queue, queue_clock) = populate_build_queue(&queue);
        BarState { selected, commands, queue, queue_clock, money, power, portrait, tooltips, hot_keys, tooltip_funcs }
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
        // (#138) `setControlCommand`: `GadgetButtonSetData` and, with a text label or sciences, `commandButtonTooltip`
        // (never taken away again, as upstream's)
        self.slot_buttons = s.commands.iter().map(|c| c.as_ref().map(|c| c.button)).collect();
        for (slot, w) in self.command_windows.clone().into_iter().enumerate() {
            if let (Some(w), Some(true)) = (w, s.tooltip_funcs.get(slot).copied()) {
                self.wm.ext[w].tooltip = Some(COMMAND_BUTTON_TOOLTIP.to_vec());
            }
        }
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
    /// `GWM_LEFT_DOWN` or `GWM_LEFT_UP` (a touch is a mouse; its press moves the pointer there first). ~~The popup's timer
    /// follows the window under the pointer and the press~~ **Corrected 2026-11-11, #138:** the manager's
    /// `commandButtonTooltip` calls go to `showBuildTooltipLayout` ([`GameUi::build_tooltip`], read with `world`); a touch
    /// held [`LONG_PRESS_MS`] or longer is released away from the button (new: a phone's long press presses nothing, and
    /// the pointer staying there then shows the popup). Returns the command slot whose button the manager selected (`GBM_SELECTED` reaching
    /// `ControlBarSystem`).
    pub fn pointer(&mut self, msg: u32, pos: (f32, f32), touch: bool, world: &PopupWorld) -> Option<usize> {
        let mut p = (pos.0.floor() as i32, pos.1.floor() as i32);
        let mut host = BarHost::default();
        if msg == gwm::LEFT_DOWN && touch {
            self.wm.win_process_mouse_event(&mut host, gwm::MOUSE_POS, p, (0, 0));
        }
        if msg == gwm::LEFT_DOWN && touch {
            self.touch_press_ms = Some(world.now_ms);
        }
        if msg == gwm::LEFT_UP && touch {
            if self.touch_press_ms.take().is_some_and(|t| world.now_ms.wrapping_sub(t) >= LONG_PRESS_MS) {
                p = (-1, -1);
            }
        }
        self.wm.win_process_mouse_event(&mut host, msg, p, (0, 0));
        for w in std::mem::take(&mut host.tooltips) {
            self.with_popup(*world, |b, h| command_button_tooltip(b, w, h));
        }
        // (#109) `ControlBarSystem`'s `GBM_SELECTED` for the options button (`ToggleQuitMenu`), and the `HideQuitMenu` of
        // the general's and the idle worker's buttons (their own calls, `togglePurchaseScience` and
        // `selectNextIdleWorker`, are not translated)
        if let Some(w) = host.selected {
            if Some(w) == self.options_button {
                self.quit_menu_call = Some(QuitMenuCall::Toggle);
            } else if self.hide_quit_menu_buttons.contains(&w) {
                self.quit_menu_call = Some(QuitMenuCall::Hide);
            } else if Some(w) == self.wm.tree.find_by_name(b"ControlBar.wnd:PopupCommunicator") {
                // (#148) `controlID == buttonCommunicator`: `ToggleDiplomacy(FALSE)`, whichever command the button holds
                self.diplomacy_toggle = true;
            } else if Some(w) == self.button_large {
                // (#136) `buttonLargeID`: `TheControlBar->toggleControlBarStage()`
                self.with_stage(|stage, h| stage.toggle_control_bar_stage(h));
            }
        }
        self.gui_sounds.append(&mut host.sounds);
        self.slot_of(host.selected)
    }

    /// (#138) `f` on the popup's state with its host ([`PopupBar`]).
    fn with_popup(&mut self, world: PopupWorld, f: impl FnOnce(&mut BuildTooltip, &mut PopupBar)) {
        let mut h = PopupBar {
            wm: &self.wm,
            command_windows: &self.command_windows,
            slot_buttons: &self.slot_buttons,
            non_command_buttons: &self.non_command_buttons,
            fills: &mut self.popup_fills,
            hidden: &mut self.popup_hidden,
            popup: &mut self.popup,
            world,
        };
        f(&mut self.build_tooltip, &mut h);
    }

    /// (#138) `ControlBar::update`'s popup part, once a client frame, before the frame's mouse event.
    pub fn popup_frame(&mut self, world: &PopupWorld) {
        self.with_popup(*world, |b, h| b.update(h));
    }

    /// (#138) What the popup shows (`populateBuildTooltipLayout`'s command button and window), while it shows.
    pub fn shown_popup(&self) -> Option<(Option<usize>, Option<WinId>)> {
        if self.popup_hidden {
            None
        } else {
            self.popup
        }
    }

    /// (#144) `ControlBar::init`'s tooltip callbacks (ControlBar.cpp:1190-1240,
    /// `rts_lockstep_engine::client::build_tooltip::control_bar_init_tooltips`) on the bar's windows: the non-command
    /// push buttons get their `NonCommand_*` command (`find`: a name's button in the World's list and the button;
    /// `None` for `findCommandButton`'s `NULL`) and `commandButtonTooltip`; the power, money and general's experience
    /// windows get `commandButtonTooltip`. The window manager then calls the popup's timing over them, and their own
    /// tooltip text no longer goes to `TheMouse`. New glue: upstream runs it once, with its command buttons loaded; the
    /// bar is read before the World's buttons are at hand, so [`load_game_ui_with`] runs it with none and the
    /// application again with the World's (a second run sets the same callbacks again and the commands found).
    pub fn init_tooltips(&mut self, find: impl Fn(&[u8]) -> Option<(usize, CommandButton)>) {
        let found = NON_COMMAND_BUTTONS.iter().filter_map(|(_, name)| find(name).map(|(i, b)| (*name, i, b))).collect();
        let mut h = BarInitHost { wm: &mut self.wm, images: &self.images, found, data: &mut self.non_command_buttons };
        control_bar_init_tooltips(&mut h);
    }

    /// (#146) `ControlBarSchemeManager::setControlBarSchemeByPlayer`'s communicator switch (ControlBarScheme.cpp:1165,
    /// `rts_lockstep_engine::client::control_bar_scheme::set_control_bar_scheme_by_player_communicator`,
    /// differential-tested): at each game's start (`GameLogic::startNewGame`'s `setControlBarSchemeByPlayer`), the
    /// communicator button gets `NonCommand_Communicator` in a multiplayer game (`TheRecorder->isMultiplayer()`) and
    /// `NonCommand_BriefingHistory` outside one, in place of `ControlBar::init`'s `NonCommand_Communicator`. `find` is as
    /// [`Self::init_tooltips`]'s. The rest of the function (the scheme of the player's side) is the application's
    /// [`load_game_ui_with`] (`set_control_bar_scheme_by_side`, the same choice).
    pub fn set_communicator(&mut self, multiplayer: bool, find: impl Fn(&[u8]) -> Option<(usize, CommandButton)>) {
        const NAMES: [&[u8]; 2] = [b"NonCommand_Communicator", b"NonCommand_BriefingHistory"];
        let found = NAMES.iter().filter_map(|name| find(name).map(|(i, b)| (*name, i, b))).collect();
        let mut h = BarInitHost { wm: &mut self.wm, images: &self.images, found, data: &mut self.non_command_buttons };
        rts_lockstep_engine::client::control_bar_scheme::set_control_bar_scheme_by_player_communicator(multiplayer, &mut h);
    }

    /// (#146) The command button the bar's communicator holds (its place in the World's list), as `setControlCommand`
    /// gave it.
    pub fn communicator_command(&self) -> Option<usize> {
        let w = self.wm.tree.find_by_name(b"ControlBar.wnd:PopupCommunicator")?;
        self.non_command_buttons.iter().find(|(win, _)| *win == w).map(|(_, c)| *c)
    }

    /// (#144) `populateBuildTooltipLayout(NULL, tooltipWin)` for a window: which of the money, power and general's
    /// experience windows it is (`tooltip_window_of`, searching the bar's windows), and that window's popup; `None` for
    /// another window (upstream's `DEBUG_ASSERTCRASH` and `return`: the popup keeps what it had).
    pub fn window_popup(&self, win: WinId, h: &impl PopupWindowHost) -> Option<Tooltip> {
        let t = tooltip_window_of(win, |n| self.wm.tree.find_by_name(n))?;
        Some(populate_window_tooltip(t, h))
    }

    /// (#144) The non-command buttons' popups (`populateBuildTooltipLayout(commandButton)` of each `NonCommand_*` button
    /// `ControlBar::init` gave a window), by the button's place in the World's list, as [`tooltip_lines`] shows them.
    pub fn non_command_tooltips<H: PopupHost>(&self, h: &mut H) -> Vec<(usize, Vec<String>)> {
        self.non_command_buttons.iter().filter_map(|&(_, c)| h.button(c).map(|b| (c, tooltip_lines(&populate_build_tooltip_layout(&b, h))))).collect()
    }

    /// (#138) A logic event's call on the bar, as the application takes it from the frame's events: a script's
    /// `HideControlBar`/`ShowControlBar` (`script_control_bar`) or `deleteBuildTooltipLayout`.
    pub fn script_call(&mut self, call: &rts_lockstep_engine::logic::client_event::ScriptInputCall) {
        use rts_lockstep_engine::logic::client_event::ScriptInputCall;
        if *call == ScriptInputCall::DeleteBuildTooltipLayout {
            self.with_popup(PopupWorld::default(), |b, h| b.delete_build_tooltip_layout(h));
            return;
        }
        self.with_stage(|stage, h| {
            script_control_bar(call, stage, h);
        });
    }

    /// (#138) `GameLogic::clearGameData`'s `HideControlBar()`, when the World's game is cleared.
    pub fn clear_game_data(&mut self) {
        self.with_stage(|stage, h| clear_game_data_control_bar(stage, h));
    }

    /// (#131) Each command window's own `WIN_STATUS_HIDDEN` and `WIN_STATUS_ENABLED` bits as the manager has them (after
    /// [`GameUi::sync`]), by slot; `None` for a slot the layout has no window for. `executeHotKey` reads these.
    pub fn command_window_status(&self) -> Vec<Option<(bool, bool)>> {
        self.command_windows
            .iter()
            .map(|w| w.map(|w| {
                let win = &self.wm.tree.windows[w];
                (win.is_hidden(), win.status & win_status::ENABLED != 0)
            }))
            .collect()
    }

    /// (#136) `f` with the one `TheMouse`'s tooltip lent to the bar's window manager
    /// (`GameWindowManager::swap_mouse_tooltip`): moving over the bar clears the world's tooltip, as upstream's
    /// `winProcessMouseEvent` does, and a bar window's tooltip text shows in it.
    pub fn with_mouse<R>(&mut self, the_mouse: &mut rts_lockstep_engine::client::mouse::MouseTooltip, f: impl FnOnce(&mut Self) -> R) -> R {
        self.wm.swap_mouse_tooltip(the_mouse);
        let r = f(self);
        self.wm.swap_mouse_tooltip(the_mouse);
        r
    }

    /// (#136) The stage functions on the bar's windows ([`StageHost`]).
    fn with_stage(&mut self, f: impl FnOnce(&mut ControlBarStage, &mut StageHost)) {
        let mut repopulate = false;
        let mut h = StageHost { wm: &mut self.wm, schemes: &self.schemes, images: &self.images, playback: self.recorder_playback, view_height: &mut self.view_height, repopulate: &mut repopulate };
        f(&mut self.stage, &mut h);
        if repopulate {
            self.with_popup(PopupWorld::default(), |b, h| b.repopulate_build_tooltip_layout(h));
        }
    }

    /// (#136) `ToggleControlBar(immediate)` (`MSG_META_TOGGLE_CONTROL_BAR`, through the command translator's case:
    /// `rts_lockstep_engine::client::control_bar_stage::toggle_control_bar_meta`).
    pub fn toggle_control_bar(&mut self, immediate: bool) {
        self.with_stage(|stage, h| toggle_control_bar(stage, immediate, h));
    }

    /// (#136) Whether `ControlBarParent` (`CP_MASTER`) is hidden: the bar draws nothing.
    pub fn is_bar_hidden(&self) -> bool {
        self.stage.master.is_some_and(|m| self.wm.tree.windows[m].is_hidden())
    }

    /// (#109) The quit menu call a button of the bar made since the last call ([`QuitMenuCall`]), for the app to make
    /// on the shell's quit menu.
    pub fn take_quit_menu_call(&mut self) -> Option<QuitMenuCall> {
        self.quit_menu_call.take()
    }

    /// (#148) Whether the communicator button was pressed since the last call (`ToggleDiplomacy(FALSE)`), for the app to
    /// make on the shell's briefing history.
    pub fn take_diplomacy_toggle(&mut self) -> bool {
        std::mem::take(&mut self.diplomacy_toggle)
    }

    /// (#74) A key through the bar's window manager ([`keyboard_into`]); whether the UI used it.
    pub fn keyboard(&mut self, scan: u8, down: bool, text: Option<&str>) -> bool {
        let mut host = BarHost::default();
        let used = keyboard_into(&mut self.wm, &mut host, scan, down, text);
        self.gui_sounds.append(&mut host.sounds);
        used
    }

    /// (#136) The window code's sounds the bar's windows asked for since the last call, for the application's audio.
    pub fn take_gui_sounds(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.gui_sounds)
    }

    /// (#74) Whether a window of the bar has the keyboard's focus (the camera's keys then go to it, not the view).
    pub fn has_keyboard_focus(&self) -> bool {
        self.wm.keyboard_focus.is_some()
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
        // (#136) the layers are drawn by windows inside `ControlBarParent` (`W3DCommandBarBackgroundDraw`,
        // `W3DCommandBarForegroundDraw`), offset by how far `BackgroundMarker` has moved since the load: not at all
        // while the bar is hidden; lowered with it. Without a marker (the synthetic bar), `CP_MASTER` stands for it.
        let marker = match (self.background_marker, self.stage.master) {
            (Some((w, base)), _) => Some((w, base)),
            (None, Some(m)) => Some((m, self.stage.default_position)),
            _ => None,
        };
        let (layers_shown, offset) = match marker {
            Some((w, base)) => {
                let (x, y) = self.wm.tree.screen_position(w);
                (self.shown(w, s), rts_lockstep_engine::common::base_type::ICoord2D { x: x - base.0, y: y - base.1 })
            }
            None => (true, Default::default()),
        };
        let draws: Vec<SchemeDraw> = if layers_shown { scheme.map(|c| c.draw_list(self.schemes.multiplier, offset, &self.images)).unwrap_or_default() } else { Vec::new() };
        let quad = |d: &SchemeDraw| UiQuad { rect: [d.start.0, d.start.1, d.end.0, d.end.1], image: Some(d.image.clone()), color: None, text: None, command: None, dim: false, clock: None, overlay: None, ..Default::default() };
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
        let mut q = UiQuad { rect, image: None, color: None, text: None, command: None, dim: false, clock: None, overlay: None, ..Default::default() };
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
            out.push(UiQuad { rect: [rect[0], rect[1], rect[0] + (rect[2] - rect[0]) * frac.max(0.05), rect[3]], image: None, color: Some(colr), text: None, command: None, dim: false, clock: None, overlay: None, ..Default::default() });
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

    /// (#78) A screen point (logical pixels) in the layout: [`LayoutPlacement::to_screen`] inverted.
    pub fn to_layout(&self, p: (f32, f32)) -> (f32, f32) {
        ((p.0 - self.left) / self.scale, (p.1 - self.top) / self.scale + self.y0)
    }
}

/// The zoom column's height (~~4 buttons of 48 and 3 gaps of 6~~ **corrected 2026-11-06 (#4):** ~~5 buttons of 48 and 4
/// gaps of 6, the saves button added~~ **corrected 2026-11-06 (#62):** ~~6 buttons of 48 and 5 gaps of 6, the campaigns
/// button added~~ ~~**Corrected 2026-11-11 (#131):** 8 buttons of 48 and 7 gaps of 6, the two turn buttons added~~
/// ~~**Corrected 2026-11-11, #136:** 9 buttons of 48 and 8 gaps of 6, the control bar's `B` added~~
/// **Corrected 2026-11-11, #142:** 10 buttons of 48 and 9 gaps of 6, the phone's cancel button `X` added (#135),
/// `app::setup_ui`) and its bottom margin. (#136's first phone screenshot, with the constant still at 8, showed the radar
/// over the `+` button.)
pub(crate) const ZOOM_COLUMN_HEIGHT: f32 = 10.0 * 48.0 + 9.0 * 6.0 + 8.0;

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

    /// #78: a window manager's draw list as the app's quads ([`draw_list_quads`]). Predicted before the run, from the
    /// rules in its doc:
    ///
    /// - `FillRect(10, 20, 30, 40, 0x80FF0000)`: one quad (10, 20)..(40, 60), RGBA (255, 0, 0, 128);
    /// - `OpenRect(0, 0, 10, 5)`: four edges, one pixel wide, inside: top (0,0,10,1), bottom (0,4,10,5), left (0,0,1,5),
    ///   right (9,0,10,5);
    /// - a horizontal line (0, 5)..(10, 5): (0, 4.5, 10, 5.5), not turned; a line (0, 0)..(3, 4): 5 long about (1.5, 2),
    ///   (-1, 1.5, 4, 2.5), turned by sin 0.8, cos 0.6;
    /// - an image (0, 0)..(100, 50) clipped to (50, 0)..(200, 25): (50, 0, 100, 25), the image's (0.5, 0)..(1, 0.5),
    ///   no tint (opaque white); tinted 0x80FFFFFF: alpha 128; clipped away: no quad;
    /// - the text "AB" at (5, 6), no font (the default host: 12 high, 6 a character): (5, 6, 17, 18) from its top left,
    ///   green, 12 high, not wrapped;
    /// - an inverse clock at 30 %: 70.
    /// - a placement's `to_layout` inverts `to_screen`.
    #[test]
    fn the_draw_list_becomes_quads() {
        let q = |items: &[DrawItem]| draw_list_quads(items, &NoGuiHost);
        let f = q(&[DrawItem::FillRect { x: 10, y: 20, w: 30, h: 40, color: 0x80FF_0000_u32 as i32 }]);
        assert_eq!((f.len(), f[0].rect, f[0].color), (1, [10.0, 20.0, 40.0, 60.0], Some([255, 0, 0, 128])));
        let o: Vec<[f32; 4]> = q(&[DrawItem::OpenRect { x: 0, y: 0, w: 10, h: 5, color: -1 }]).iter().map(|q| q.rect).collect();
        assert_eq!(o, [[0.0, 0.0, 10.0, 1.0], [0.0, 4.0, 10.0, 5.0], [0.0, 0.0, 1.0, 5.0], [9.0, 0.0, 10.0, 5.0]]);
        let l = q(&[DrawItem::Line { x0: 0, y0: 5, x1: 10, y1: 5, color: -1 }, DrawItem::Line { x0: 0, y0: 0, x1: 3, y1: 4, color: -1 }]);
        assert_eq!((l[0].rect, l[0].rotation), ([0.0, 4.5, 10.0, 5.5], None));
        assert_eq!(l[1].rect, [-1.0, 1.5, 4.0, 2.5]);
        let (sin, cos) = l[1].rotation.expect("turned");
        assert!((sin - 0.8).abs() < 1e-6 && (cos - 0.6).abs() < 1e-6);
        let img = |color: u32, clip| DrawItem::Image { image: b"ImgBack".to_vec(), x0: 0, y0: 0, x1: 100, y1: 50, color, grayscale: false, clip };
        let i = q(&[img(0xFFFF_FFFF, Some((50, 0, 200, 25))), img(0x80FF_FFFF, None), img(0xFFFF_FFFF, Some((200, 200, 300, 300)))]);
        assert_eq!(i.len(), 2);
        assert_eq!((i[0].rect, i[0].uv, i[0].tint, i[0].image.as_deref()), ([50.0, 0.0, 100.0, 25.0], Some([0.5, 0.0, 1.0, 0.5]), None, Some(&b"ImgBack"[..])));
        assert_eq!((i[1].uv, i[1].tint), (None, Some([255, 255, 255, 128])));
        let t = q(&[DrawItem::Text { text: "AB".encode_utf16().collect(), x: 5, y: 6, color: 0xFF00_FF00_u32 as i32, drop_color: 0, font: None, word_wrap: None, wrap_centered: false, hotkey: false, clip: None }]);
        assert_eq!((t[0].rect, t[0].text.as_deref(), t[0].text_color, t[0].font_px, t[0].text_top_left, t[0].text_wrap), ([5.0, 6.0, 17.0, 18.0], Some("AB"), Some([0, 255, 0, 255]), Some(12.0), true, false));
        let c = q(&[DrawItem::RectClock { x: 0, y: 0, w: 4, h: 4, percent: 30, color: 0, inverse: true }]);
        assert_eq!(c[0].clock, Some(70));
        let p = LayoutPlacement { left: 7.0, top: 100.0, y0: 450.0, scale: 0.5 };
        let r = p.to_screen([40.0, 470.0, 0.0, 0.0]);
        assert_eq!(p.to_layout((r[0], r[1])), (40.0, 470.0));
    }

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
        assert!(c.click(ICoord2D { x: 5, y: 5 }, ICoord2D { x: 5, y: 5 }, &[o.dozer], Some(o.dozer), dp));
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
        asked.click(ICoord2D { x: 5, y: 5 }, ICoord2D { x: 5, y: 5 }, &[o2.dozer], Some(o2.dozer), d2);
        plain.click(ICoord2D { x: 5, y: 5 }, ICoord2D { x: 5, y: 5 }, &[o2.dozer], Some(o2.dozer), d2);
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
            ui.popup_frame(&PopupWorld { now_ms: f * 33, ..Default::default() });
            let _ = ui.pointer(gwm::MOUSE_POS, (285.0, 505.0 + (f % 3) as f32), false, &PopupWorld { now_ms: f * 33, ..Default::default() });
            let crc = |c: &mut Controls| c.world().logic_crc(rts_lockstep_engine::logic::world::game_logic_update::CRC_RECALC, None).expect("a CRC");
            assert_eq!(crc(&mut asked), crc(&mut plain), "frame {f}");
        }
    }

    /// #34: the bar's input through the window manager, on the synthetic files with the demo's dozer selected. Predicted
    /// before the run from `winProcessMouseEvent` and `GadgetPushButtonInput`:
    /// - after `sync`, the command window and `ButtonCommand01` are shown (and enabled: the depot is available),
    ///   `ButtonCommand02` and `03` hidden;
    /// - a press and a release over button 1 (285, 505) select it: `GBM_SELECTED` goes to the command window, which
    ///   passes it to `ControlBarSystem`: slot 0 is pressed;
    /// - a press where the hidden button 2 is (345, 505): the command window takes nothing; no press;
    /// - a press dragged off the button before the release: no press;
    /// - with nothing selected after `sync`: the command window is hidden; the press on button 1's place presses
    ///   nothing.
    ///
    /// **#138** (the popup through `commandButtonTooltip`; predicted before the run): `sync` gives button 1
    /// `commandButtonTooltip` (the depot's command has a label); the first frame over it shows no popup, the second does
    /// (the synthetic window's tooltip delay is -1: `beginWaitTime - 1 < now`), filled from the depot's button; it stays
    /// while the frames' events come and goes two frames after they stop; a quick touch tap presses slot 0; a touch held
    /// half a second does not press (its release goes away from the button) and the popup shows after it.
    ///
    /// *Results (2026-11-11):* one miss, of my prediction (written as "the popup shows while the touch is held, and
    /// stays"): while a button is held the manager has a grab and calls no tooltip callback (upstream's
    /// `winProcessMouseEvent` makes the tooltip only with no grab window), so the popup goes during the press and shows
    /// again after the release, the pointer staying there. The long press's rule was changed to a plain hold of
    /// [`LONG_PRESS_MS`] (it had also required the popup shown), and the test to look after the release. Then as
    /// predicted.
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
        assert!(c.click(ICoord2D { x: 5, y: 5 }, ICoord2D { x: 5, y: 5 }, &[o.dozer], Some(o.dozer), dp));
        let st = GameUi::bar_state(c.ui.first_selected_drawable(), &mut c.client, Vec::new(), m, p);
        ui.sync(&st);
        let win = |ui: &GameUi, i: usize| ui.command_windows[i].expect("a command window");
        assert!(!ui.wm.is_hidden(Some(win(&ui, 0))) && ui.wm.is_enabled(Some(win(&ui, 0))));
        assert!(ui.wm.is_hidden(Some(win(&ui, 1))) && ui.wm.is_hidden(Some(win(&ui, 2))));
        assert_eq!(ui.wm.ext[win(&ui, 0)].tooltip.as_deref(), Some(COMMAND_BUTTON_TOOLTIP));
        let depot = st.commands[0].as_ref().expect("the depot").button;
        let at = |t: u32| PopupWorld { now_ms: t, ..Default::default() };
        // (#138) the frames over button 1: `ControlBar::update`'s part, then the frame's mouse event
        let frame = |ui: &mut GameUi, t: u32, pos: Option<(f32, f32)>| {
            ui.popup_frame(&at(t));
            if let Some(pos) = pos {
                assert_eq!(ui.pointer(gwm::MOUSE_POS, pos, false, &at(t)), None);
            }
        };
        frame(&mut ui, 1000, Some((285.0, 505.0)));
        assert_eq!(ui.shown_popup(), None);
        frame(&mut ui, 1033, Some((285.0, 505.0)));
        assert_eq!(ui.shown_popup(), Some((Some(depot), None)));
        frame(&mut ui, 1066, Some((286.0, 505.0)));
        assert_eq!(ui.shown_popup(), Some((Some(depot), None)));
        assert_eq!(ui.pointer(gwm::LEFT_DOWN, (285.0, 505.0), false, &at(1070)), None);
        assert_eq!(ui.pointer(gwm::LEFT_UP, (285.0, 505.0), false, &at(1080)), Some(0));
        frame(&mut ui, 1100, None);
        assert!(ui.shown_popup().is_some(), "the last event's mark holds one frame");
        frame(&mut ui, 1133, None);
        assert_eq!((ui.shown_popup(), ui.build_tooltip.prev_window), (None, None));
        // a quick tap presses; a held touch shows the popup, and its release does not press
        assert_eq!(ui.pointer(gwm::LEFT_DOWN, (286.0, 506.0), true, &at(2000)), None);
        assert_eq!(ui.pointer(gwm::LEFT_UP, (286.0, 506.0), true, &at(2050)), Some(0), "a quick tap");
        frame(&mut ui, 3000, None);
        frame(&mut ui, 3033, None);
        assert_eq!(ui.pointer(gwm::LEFT_DOWN, (286.0, 506.0), true, &at(4000)), None);
        frame(&mut ui, 4033, Some((286.0, 506.0)));
        assert_eq!(ui.pointer(gwm::LEFT_UP, (286.0, 506.0), true, &at(4500)), None, "a long press");
        // (no tooltip call while the button is held: the manager's grab; the pointer then stays where the finger lifted)
        frame(&mut ui, 4533, Some((286.0, 506.0)));
        frame(&mut ui, 4566, Some((286.0, 506.0)));
        assert_eq!(ui.shown_popup(), Some((Some(depot), None)), "the pointer stays where the finger lifted");
        assert_eq!(ui.pointer(gwm::LEFT_DOWN, (345.0, 505.0), false, &at(5000)), None);
        assert_eq!(ui.pointer(gwm::LEFT_UP, (345.0, 505.0), false, &at(5100)), None);
        assert_eq!(ui.pointer(gwm::LEFT_DOWN, (285.0, 505.0), false, &at(6000)), None);
        assert_eq!(ui.pointer(gwm::MOUSE_POS, (400.0, 560.0), false, &at(6050)), None);
        assert_eq!(ui.pointer(gwm::LEFT_UP, (400.0, 560.0), false, &at(6100)), None, "released off the button");
        let none = GameUi::bar_state(None, &mut c.client, Vec::new(), m, p);
        ui.sync(&none);
        assert!(ui.wm.is_hidden(Some(win(&ui, 0))));
        assert_eq!(ui.pointer(gwm::LEFT_DOWN, (285.0, 505.0), false, &at(7000)), None);
        assert_eq!(ui.pointer(gwm::LEFT_UP, (285.0, 505.0), false, &at(7100)), None);
    }

    /// (#109) The synthetic bar with `ButtonOptions` and `ButtonIdleWorker` added (copies of `ButtonCommand01`, children
    /// of the parent, at (130, 455)-(180, 475) and (190, 455)-(240, 475), where the scheme's corners put them too).
    fn bar_with_options() -> GameUi {
        let mut d = MemoryDevice::new();
        for (p, b) in synthetic_ui_files() {
            let b = if p == b"Window\\ControlBar.wnd" {
                let wnd = String::from_utf8(b).expect("text");
                let start = wnd.find("WINDOW\n WINDOWTYPE = PUSHBUTTON;").expect("a button");
                let button = &wnd[start..start + wnd[start..].find("END\n").expect("its end") + 4];
                let make = |name: &str, x: i32| {
                    button
                        .replace("ControlBar.wnd:ButtonCommand01", &format!("ControlBar.wnd:{name}"))
                        .replace("UPPERLEFT: 260 480, BOTTOMRIGHT: 310 530", &format!("UPPERLEFT: {x} 455, BOTTOMRIGHT: {} 475", x + 50))
                };
                let at = wnd.find("WINDOW\n WINDOWTYPE = USER;\n SCREENRECT = UPPERLEFT: 250 470").expect("the command window");
                format!("{}{}{}{}", &wnd[..at], make("ButtonOptions", 130), make("ButtonIdleWorker", 190), &wnd[at..]).into_bytes()
            } else if p == b"Data\\INI\\ControlBarScheme.ini" {
                // (first run, a miss of the test's: `ControlBarScheme::init` places both buttons at the scheme's
                // `OptionsUL`/`LR` and `WorkerUL`/`LR`, which the synthetic scheme lacked, so they were put at 0 by 0)
                String::from_utf8(b).expect("text").replacen("  Side = Observer\n", "  Side = Observer\n  OptionsUL = X:130 Y:455\n  OptionsLR = X:180 Y:475\n  WorkerUL = X:190 Y:455\n  WorkerLR = X:240 Y:475\n", 1).into_bytes()
            } else {
                b
            };
            d.insert(p, b);
        }
        let mut fs = FileSystem::new(d);
        fs.init(None);
        let UiBuild::Ready(ui) = load_game_ui(&fs, &TextTable::default(), b"America") else { panic!("the bar did not load") };
        *ui
    }

    /// #109: the bar's options button (ControlBarCallback.cpp, `ControlBarSystem`'s `GBM_SELECTED`). Predicted before the
    /// run: the bar finds `ButtonOptions` and one of the two `HideQuitMenu` buttons (`ButtonIdleWorker`; the synthetic bar
    /// has no `ButtonGeneral`); no call before a press; a tap on the options button presses no command slot and makes
    /// `ToggleQuitMenu` ([`QuitMenuCall::Toggle`]), taken once; a tap on the idle worker's makes `HideQuitMenu`
    /// ([`QuitMenuCall::Hide`]); a press dragged off the options button makes none; a tap on the bar's background none.
    ///
    /// **Result (2026-11-10):** one miss of the test's own (the scheme places the two buttons, and the synthetic scheme
    /// had no corners for them: see [`bar_with_options`]), then as predicted.
    #[test]
    fn the_bars_options_button_toggles_the_quit_menu() {
        let mut ui = bar_with_options();
        assert!(ui.options_button.is_some() && ui.hide_quit_menu_buttons.len() == 1);
        let w = PopupWorld::default();
        let tap = |ui: &mut GameUi, at: (f32, f32), _t: f64, w: &PopupWorld| -> Option<usize> {
            assert_eq!(ui.pointer(gwm::LEFT_DOWN, at, true, w), None);
            ui.pointer(gwm::LEFT_UP, at, true, w)
        };
        assert_eq!(ui.take_quit_menu_call(), None);
        assert_eq!(tap(&mut ui, (155.0, 465.0), 0.0, &w), None);
        assert_eq!(ui.take_quit_menu_call(), Some(QuitMenuCall::Toggle));
        assert_eq!(ui.take_quit_menu_call(), None, "taken once");
        assert_eq!(tap(&mut ui, (215.0, 465.0), 1.0, &w), None);
        assert_eq!(ui.take_quit_menu_call(), Some(QuitMenuCall::Hide));
        assert_eq!(ui.pointer(gwm::LEFT_DOWN, (155.0, 465.0), false, &w), None);
        assert_eq!(ui.pointer(gwm::MOUSE_POS, (400.0, 560.0), false, &w), None);
        assert_eq!(ui.pointer(gwm::LEFT_UP, (400.0, 560.0), false, &w), None);
        assert_eq!(ui.take_quit_menu_call(), None, "released off the button");
        assert_eq!(tap(&mut ui, (400.0, 460.0), 3.0, &w), None);
        assert_eq!(ui.take_quit_menu_call(), None, "the background");
    }

    /// (#136) The synthetic bar with `ButtonLarge` added (a copy of `ButtonCommand01` with the tooltip text `TIP:Large`, a
    /// child of the parent, placed by the scheme's `MinMaxUL`/`LR` at (380, 452)-(420, 468)) and the scheme's toggle images
    /// (down: `ImgButton`, up: `ImgBack`).
    fn bar_with_button_large() -> GameUi {
        let mut d = MemoryDevice::new();
        for (p, b) in synthetic_ui_files() {
            let b = if p == b"Window\\ControlBar.wnd" {
                let wnd = String::from_utf8(b).expect("text");
                let start = wnd.find("WINDOW\n WINDOWTYPE = PUSHBUTTON;").expect("a button");
                let button = &wnd[start..start + wnd[start..].find("END\n").expect("its end") + 4];
                let large = button
                    .replace("NAME = \"ControlBar.wnd:ButtonCommand01\";\n", "NAME = \"ControlBar.wnd:ButtonLarge\";\n TOOLTIPTEXT = \"TIP:Large\";\n")
                    .replace("UPPERLEFT: 260 480, BOTTOMRIGHT: 310 530", "UPPERLEFT: 380 452, BOTTOMRIGHT: 420 468");
                let at = wnd.find("WINDOW\n WINDOWTYPE = USER;\n SCREENRECT = UPPERLEFT: 250 470").expect("the command window");
                format!("{}{}{}", &wnd[..at], large, &wnd[at..]).into_bytes()
            } else if p == b"Data\\INI\\ControlBarScheme.ini" {
                let fields = "  MinMaxUL = X:380 Y:452\n  MinMaxLR = X:420 Y:468\n  ToggleButtonUpIn = ImgBack\n  ToggleButtonUpOn = ImgBack\n  ToggleButtonUpPushed = ImgBack\n  ToggleButtonDownIn = ImgButton\n  ToggleButtonDownOn = ImgButton\n  ToggleButtonDownPushed = ImgButton\n";
                String::from_utf8(b).expect("text").replacen("  Side = Observer\n", &format!("  Side = Observer\n{fields}"), 1).into_bytes()
            } else {
                b
            };
            d.insert(p, b);
        }
        let mut fs = FileSystem::new(d);
        fs.init(None);
        let UiBuild::Ready(ui) = load_game_ui(&fs, &TextTable::default(), b"America") else { panic!("the bar did not load") };
        *ui
    }

    /// #146: `setControlBarSchemeByPlayer`'s communicator switch on the bar's own windows. Predicted before the run: a bar
    /// whose layout has `PopupCommunicator` (a push button): `ControlBar::init` gives it `NonCommand_Communicator` (button
    /// 5); a single player game's start gives it `NonCommand_BriefingHistory` (6); a multiplayer game's
    /// `NonCommand_Communicator` again; with no `BriefingHistory` button in the World's list it keeps the one it had
    /// (`setControlCommand` does nothing for `NULL`).
    #[test]
    fn the_communicator_is_the_briefing_history_outside_multiplayer() {
        let mut d = MemoryDevice::new();
        for (p, b) in synthetic_ui_files() {
            let b = if p == b"Window\\ControlBar.wnd" {
                let wnd = String::from_utf8(b).expect("text");
                let start = wnd.find("WINDOW\n WINDOWTYPE = PUSHBUTTON;").expect("a button");
                let button = &wnd[start..start + wnd[start..].find("END\n").expect("its end") + 4];
                let communicator = button.replace("NAME = \"ControlBar.wnd:ButtonCommand01\";\n", "NAME = \"ControlBar.wnd:PopupCommunicator\";\n").replace("UPPERLEFT: 260 480, BOTTOMRIGHT: 310 530", "UPPERLEFT: 380 452, BOTTOMRIGHT: 420 468");
                let at = wnd.find("WINDOW\n WINDOWTYPE = USER;\n SCREENRECT = UPPERLEFT: 250 470").expect("the command window");
                format!("{}{}{}", &wnd[..at], communicator, &wnd[at..]).into_bytes()
            } else {
                b
            };
            d.insert(p, b);
        }
        let mut fs = FileSystem::new(d);
        fs.init(None);
        let UiBuild::Ready(ui) = load_game_ui(&fs, &TextTable::default(), b"America") else { panic!("the bar did not load") };
        let mut ui = *ui;
        let comm = CommandButton::new(b"NonCommand_Communicator");
        let briefing = CommandButton::new(b"NonCommand_BriefingHistory");
        let find = |n: &[u8]| match n {
            b"NonCommand_Communicator" => Some((5, comm.clone())),
            b"NonCommand_BriefingHistory" => Some((6, briefing.clone())),
            _ => None,
        };
        ui.init_tooltips(find);
        assert_eq!(ui.communicator_command(), Some(5));
        ui.set_communicator(false, find);
        assert_eq!(ui.communicator_command(), Some(6));
        ui.set_communicator(true, find);
        assert_eq!(ui.communicator_command(), Some(5));
        ui.set_communicator(false, |n: &[u8]| (n == b"NonCommand_Communicator").then(|| (5, comm.clone())));
        assert_eq!(ui.communicator_command(), Some(5), "no BriefingHistory: setControlCommand(NULL) does nothing");
    }

    /// #136: the bar hidden, shown, lowered and raised (`ToggleControlBar`, `ButtonLarge`'s `toggleControlBarStage`).
    /// Predicted before the run: loaded at the default stage (the view 480 high, `ButtonLarge` with the "down" image
    /// `ImgButton`), the scheme's layer from y 440; a tap on `ButtonLarge` lowers the bar to y 540 (the layer moves the
    /// same 90 down to 530) and gives it the "up" image `ImgBack`; a tap there again raises it to 450; the meta key
    /// queues one immediate `ToggleControlBar`, which hides the bar (nothing drawn, the view the whole 600) and the next
    /// shows it at the default stage; during a replay's playback the stage stays.
    ///
    /// **Result (2026-11-11):** as predicted on the first run.
    #[test]
    fn the_bar_hides_shows_lowers_and_raises() {
        use rts_lockstep_engine::client::control_bar_stage::ControlBarStages;
        let mut ui = bar_with_button_large();
        let large = ui.button_large.expect("ButtonLarge");
        let image = |ui: &GameUi| ui.wm.tree.windows[large].inst.enabled_draw_data[0].image.clone();
        assert_eq!((ui.stage.current, ui.view_height), (ControlBarStages::Default, Some(480)));
        assert_eq!(image(&ui).as_deref(), Some(&b"ImgButton"[..]));
        let top = |ui: &GameUi| ui.frame(&BarState::default()).iter().fold(f32::MAX, |a, q| a.min(q.rect[1]));
        assert_eq!(top(&ui), 440.0);
        let w = PopupWorld::default();
        let tap = |ui: &mut GameUi, at: (f32, f32), _t: f64, w: &PopupWorld| {
            assert_eq!(ui.pointer(gwm::LEFT_DOWN, at, true, w), None);
            assert_eq!(ui.pointer(gwm::LEFT_UP, at, true, w), None);
        };
        tap(&mut ui, (400.0, 460.0), 0.0, &w);
        let master = ui.stage.master.expect("ControlBarParent");
        assert_eq!((ui.stage.current, ui.wm.tree.windows[master].region.1), (ControlBarStages::Low, 540));
        assert_eq!(top(&ui), 530.0);
        assert_eq!(image(&ui).as_deref(), Some(&b"ImgBack"[..]));
        tap(&mut ui, (400.0, 550.0), 1.0, &w);
        assert_eq!((ui.stage.current, ui.wm.tree.windows[master].region.1), (ControlBarStages::Default, 450));
        let (world, _) = crate::demo::demo_world();
        let mut c = Controls::new(world, 1);
        c.meta(rts_lockstep_engine::common::message_types::MSG_META_TOGGLE_CONTROL_BAR);
        assert_eq!(c.client.control_bar_toggles, vec![true]);
        ui.toggle_control_bar(true);
        assert!(ui.is_bar_hidden() && ui.frame(&BarState::default()).is_empty());
        assert_eq!(ui.view_height, Some(600));
        ui.toggle_control_bar(true);
        assert!(!ui.is_bar_hidden());
        assert_eq!((ui.stage.current, ui.view_height), (ControlBarStages::Default, Some(480)));
        ui.recorder_playback = true;
        tap(&mut ui, (400.0, 460.0), 2.0, &w);
        assert_eq!(ui.stage.current, ControlBarStages::Default, "no stage switch during a replay's playback");
    }

    /// #136: one `TheMouse`. ~~Predicted before the run: with the mouse lent, the pointer over `ButtonLarge` sets its tooltip
    /// text into the one mouse (`winProcessMouseEvent`'s window tooltip), and the bar's manager keeps none of its own;
    /// once the mouse has been still past its delay the tooltip is drawn by the world's drawing
    /// (`crate::cursor::tooltip_shapes`): a box and the text; the pointer over the bar's background clears it (the event's
    /// `setCursorTooltip(UnicodeString::TheEmptyString)`), and nothing is drawn.~~
    ///
    /// ~~**Result (2026-11-11):** one miss, the test's own: the first draw is the box alone, 2 pixels wide (the highlight
    /// opens the box and reveals the text over the following draws, as #124's test found), so the text is looked for
    /// over the next draws. Then as predicted.~~
    ///
    /// **Corrected 2026-11-11, #144:** that asserted the client's behaviour, not upstream's: upstream's `ControlBar::init`
    /// gives `ButtonLarge` (and the other non-command buttons, and the power, money and general's experience windows)
    /// `commandButtonTooltip`, so `winProcessMouseEvent` calls it **instead of** setting the window's tooltip text into
    /// `TheMouse` (GameWindowManager.cpp:1222). Revisited to assert upstream's behaviour. *Predictions, written before the
    /// run:* loaded, the bar's windows already have the callback ([`load_game_ui_with`] runs the init with no command
    /// buttons) and `ButtonLarge` no command; given the World's `NonCommand_UpDown` (place 7), `ButtonLarge` holds it.
    /// With the mouse lent, the pointer over `ButtonLarge` leaves the one mouse's tooltip **empty** (its `TIP:Large` is
    /// never set) and nothing is drawn for it; still over it for a second of frames, the bar's popup shows
    /// `NonCommand_UpDown` (`(Some(7), None)`); over the power window it goes, then shows the power window's own popup
    /// (`(None, Some(power))`, "Power" and its description formatted with the player's production and consumption),
    /// then the money window's ("Money"); over the bar's background it goes, and the mouse's tooltip stays empty.
    ///
    /// **Result (2026-11-11):** one miss, the test's own: the popup never showed over `ButtonLarge` with the clock
    /// starting at 0, because the synthetic windows' tooltip delay is -1 and upstream's unsigned wait
    /// (`beginWaitTime + getTooltipDelay() < timeGetTime()`) is then `0xFFFFFFFF < now`, never true (#138's
    /// differential test predicted exactly this). Started at 1 s, the rest held as predicted.
    #[test]
    fn the_bars_tooltip_is_the_one_mouses() {
        use rts_lockstep_engine::client::mouse::Mouse;
        struct Energy;
        impl PopupWindowHost for Energy {
            fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
                match label {
                    b"CONTROLBAR:Power" => "Power".encode_utf16().collect(),
                    b"CONTROLBAR:PowerDescription" => "%d made, %d used".encode_utf16().collect(),
                    b"CONTROLBAR:Money" => "Money".encode_utf16().collect(),
                    _ => label.iter().map(|&b| u16::from(b)).collect(),
                }
            }
            fn is_observer_control_bar_on(&self) -> bool {
                false
            }
            fn observer_look_at_player_energy(&self) -> Option<(i32, i32)> {
                None
            }
            fn local_energy(&self) -> Option<(i32, i32)> {
                Some((5, 3))
            }
        }
        let mut ui = bar_with_button_large();
        let large = ui.button_large.expect("ButtonLarge");
        let power = ui.power.expect("PowerWindow");
        let money = ui.money.expect("MoneyDisplay");
        for w in [large, power, money] {
            assert_eq!(ui.wm.ext[w].tooltip.as_deref(), Some(COMMAND_BUTTON_TOOLTIP));
        }
        assert!(ui.non_command_buttons.is_empty());
        let mut up_down = CommandButton::new(b"NonCommand_UpDown");
        up_down.labels[0] = b"CONTROLBAR:UpDown".to_vec();
        ui.init_tooltips(|n| (n == b"NonCommand_UpDown").then(|| (7, up_down.clone())));
        assert_eq!(ui.non_command_buttons, vec![(large, 7)]);
        let mut m = Mouse::new();
        m.init();
        m.set_visibility(true);
        m.set_mouse_limits(800, 600);
        m.pos = ICoord2D { x: 400, y: 460 };
        // (the clock starts at 1 s: the synthetic windows' tooltip delay is -1, and `beginWaitTime + delay` at a
        // first call at 0 would be `0xFFFFFFFF`, which no clock passes, #138's test's finding)
        let mut t = 1000u32;
        let mut frames = |ui: &mut GameUi, m: &mut Mouse, at: (f32, f32), n: u32| {
            for _ in 0..n {
                let w = PopupWorld { now_ms: t, ..Default::default() };
                ui.popup_frame(&w);
                ui.with_mouse(&mut m.tooltip, |ui| ui.pointer(gwm::MOUSE_POS, at, false, &w));
                t += 100;
            }
        };
        frames(&mut ui, &mut m, (400.0, 460.0), 1);
        assert!(m.tooltip.is_empty && m.tooltip.display_string.is_empty(), "{:?}", String::from_utf16_lossy(&m.tooltip.display_string));
        assert!(ui.wm.mouse.is_empty);
        m.tooltip.create_stream_messages(1000, true);
        m.tooltip.create_stream_messages(100_000, false);
        assert!(crate::cursor::tooltip_shapes(&mut m, 100_000, false).is_empty());
        frames(&mut ui, &mut m, (400.0, 460.0), 10);
        assert_eq!(ui.shown_popup(), Some((Some(7), None)));
        assert!(m.tooltip.is_empty);
        let fills = ui.popup_fills;
        frames(&mut ui, &mut m, (120.0, 490.0), 12);
        assert_eq!(ui.shown_popup(), Some((None, Some(power))));
        assert!(ui.popup_fills > fills);
        let tip = ui.window_popup(power, &Energy).expect("the power window's popup");
        assert_eq!(tooltip_lines(&tip), vec!["Power".to_string(), "5 made, 3 used".to_string()]);
        frames(&mut ui, &mut m, (60.0, 470.0), 12);
        assert_eq!(ui.shown_popup(), Some((None, Some(money))));
        assert_eq!(ui.window_popup(money, &Energy).map(|t| tooltip_lines(&t)[0].clone()).as_deref(), Some("Money"));
        assert!(ui.window_popup(large, &Energy).is_none(), "a window the popup does not know: upstream's return");
        frames(&mut ui, &mut m, (600.0, 455.0), 3);
        assert_eq!(ui.shown_popup(), None);
        assert!(m.tooltip.is_empty);
        m.tooltip.create_stream_messages(200_000, false);
        assert!(crate::cursor::tooltip_shapes(&mut m, 200_000, false).is_empty());
    }

    /// #74: typed characters reach a text entry in the bar's window manager (the bar has none of its own: one is made
    /// with `gogoGadgetTextEntry`, as a menu's would be). Predicted from IMEManager.cpp and GadgetTextEntry.cpp before
    /// the run: before it has the focus, typing `a` is not used; once focused (`winSetFocus`, which attaches the IME),
    /// the key `A` (scan 0x1E, not a key the entry handles: eaten, so used) with the text `Go 1!` types `Go 1!`; a
    /// backspace key (scan 0x0E, no text) erases the `!`; its release is used too (the entry eats every key it does not
    /// pass on); a key whose text is a tab (`\t`, below 32) adds nothing; the bar's keyboard focus is reported.
    #[test]
    fn typed_text_reaches_a_text_entry_in_the_ui() {
        use rts_lockstep_engine::client::gadget_text_entry::{entry_rt, gogo_gadget_text_entry, EntryRt};
        use rts_lockstep_engine::client::window_script::{gws, WinInstanceData};
        let mut d = MemoryDevice::new();
        for (p, b) in synthetic_ui_files() {
            d.insert(p, b);
        }
        let mut fs = FileSystem::new(d);
        fs.init(None);
        let UiBuild::Ready(mut ui) = load_game_ui(&fs, &TextTable::default(), b"America") else { panic!("the bar did not load") };
        let inst = WinInstanceData { style: gws::ENTRY_FIELD, ..WinInstanceData::new() };
        let e = gogo_gadget_text_entry(&mut ui.wm, &mut NoGuiHost, None, win_status::ENABLED, 10, 10, 200, 20, inst, EntryRt { max_text_len: 32, ..Default::default() }, true).expect("an entry");
        assert!(!ui.keyboard(0, true, Some("a")));
        assert!(!ui.has_keyboard_focus());
        ui.wm.win_set_focus(&mut NoGuiHost, Some(e));
        assert!(ui.has_keyboard_focus());
        assert!(ui.keyboard(0x1E, true, Some("Go 1!")));
        let text = |ui: &GameUi| String::from_utf16_lossy(&entry_rt(&ui.wm, e).expect("data").text.text);
        assert_eq!(text(&ui), "Go 1!");
        assert!(ui.keyboard(0x0E, true, None));
        assert!(ui.keyboard(0x0E, false, None));
        assert_eq!(text(&ui), "Go 1");
        ui.keyboard(0x0F, true, Some("\t"));
        assert_eq!(text(&ui), "Go 1");
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

    /// ~~#35, when the popup shows (`TooltipTimer`'s rules).~~ **Corrected 2026-11-11, #138:** `TooltipTimer` is gone (the
    /// popup's timing is upstream's: `the_bar_is_pressed_through_the_window_manager`, and the engine's differential
    /// test). #138: where the view ends on the window. *Predicted before the run:* no height asked: the whole window; the
    /// bar shown at the default stage (480 of the layout's 600) on an 800x600 window, the bar's layout at scale 1 from y
    /// 440: 480; on a phone (390x844, the bar scaled 0.4875 to its width, its top 440 at 844 - 160 x 0.4875 = 766):
    /// 785.5; the low stage or the bar hidden (600): the whole window; a height with no bar on the window: its share of
    /// the window (480: 675.2); never below 1 or past the window.
    ///
    /// *Results (2026-11-11):* as predicted, on the first run.
    #[test]
    fn the_view_ends_where_upstreams_does_above_the_bar() {
        assert_eq!(tactical_view_height(None, None, 600.0), 600.0);
        let desk = LayoutPlacement { left: 0.0, top: 440.0, y0: 440.0, scale: 1.0 };
        assert_eq!(tactical_view_height(Some(480), Some(desk), 600.0), 480.0);
        let s = 390.0 / 800.0;
        let phone = LayoutPlacement { left: 0.0, top: 844.0 - 160.0 * s, y0: 440.0, scale: s };
        assert_eq!(tactical_view_height(Some(480), Some(phone), 844.0), 785.5);
        assert_eq!(tactical_view_height(Some(600), Some(phone), 844.0), 844.0);
        assert_eq!(tactical_view_height(Some(600), None, 844.0), 844.0);
        assert_eq!(tactical_view_height(Some(480), None, 844.0), 675.2);
        assert_eq!(tactical_view_height(Some(-5), None, 844.0), 1.0);
        assert_eq!(tactical_view_height(Some(900), Some(desk), 600.0), 600.0);
    }

    /// #138: the bar's other `HideControlBar`/`ShowControlBar` callers, on the synthetic bar. *Predicted before the run:*
    /// loaded as a game's (`createControlBar`'s and `GameEngine::init`'s hides, then `startNewGame`'s
    /// `ShowControlBar(FALSE)`): shown at the default stage, the view 480; a script's letterbox (the logic's
    /// `HideControlBar(TRUE)` event) hides it, the view 600; its end (`ShowControlBar(FALSE)`) shows it, 480; a script's
    /// `deleteBuildTooltipLayout` hides the popup and forgets its window; the World's game cleared hides the bar, 600.
    ///
    /// *Results (2026-11-11):* as predicted, on the first run.
    #[test]
    fn the_scripts_and_the_game_hide_and_show_the_bar() {
        use rts_lockstep_engine::logic::client_event::ScriptInputCall;
        let mut d = MemoryDevice::new();
        for (p, b) in synthetic_ui_files() {
            d.insert(p, b);
        }
        let mut fs = FileSystem::new(d);
        fs.init(None);
        let UiBuild::Ready(mut ui) = load_game_ui(&fs, &TextTable::default(), b"America") else { panic!("the bar did not load") };
        assert!(!ui.is_bar_hidden());
        assert_eq!((ui.stage.current, ui.view_height), (ControlBarStages::Default, Some(480)));
        ui.script_call(&ScriptInputCall::HideControlBar(true));
        assert!(ui.is_bar_hidden() && ui.view_height == Some(600));
        ui.script_call(&ScriptInputCall::ShowControlBar(false));
        assert!(!ui.is_bar_hidden() && ui.view_height == Some(480));
        ui.popup_hidden = false;
        ui.build_tooltip.prev_window = Some(3);
        ui.script_call(&ScriptInputCall::DeleteBuildTooltipLayout);
        assert!(ui.popup_hidden && ui.build_tooltip.prev_window.is_none());
        ui.clear_game_data();
        assert!(ui.is_bar_hidden() && ui.view_height == Some(600));
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
        // 382, 564]~~ corrected 2026-11-06, #62: 54 higher again, above the sixth, the campaigns button; ~~[265, 422, 382,
        // 510]~~ corrected 2026-11-11, #131: 108 higher, above the two turn buttons; ~~[265, 314, 382, 402]~~ corrected
        // 2026-11-11, #136: 54 higher again, above the control bar's `B`; ~~[265, 260, 382, 348]~~ corrected 2026-11-11,
        // #142: 54 higher again, above the phone's cancel button `X`, #135)
        assert_eq!(radar_screen_rect(None, None, (390.0, 844.0)), [265.0, 206.0, 382.0, 294.0]);
        assert_eq!(radar_screen_rect(None, None, (844.0, 300.0))[1], 36.0);
        let big = radar_screen_rect(None, None, (1280.0, 720.0));
        assert_eq!((big[2] - big[0], big[3] - big[1]), (180.0, 135.0));
    }
}
