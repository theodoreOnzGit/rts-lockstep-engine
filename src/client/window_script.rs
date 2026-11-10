// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowManagerScript.cpp (the whole WND reader: parseBitFlag,
//     parseBitString, readUntilSemicolon, scanBool, scanShort, scanInt, scanUnsignedInt, the window stack, the
//     default colours, parseColor, parseDefaultColor, parseDefaultFont, every field parser of gameWindowFieldList,
//     getDataTemplate, parseData, setWindowText, createGadget, createWindow, parseChildWindows, parseWindow,
//     parseInit, parseUpdate, parseShutdown, parseLayoutBlock, WindowLayoutInfo's constructor, winCreateFromScript)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowManager.cpp (winCreate, addWindowToParent: the
//     window's place, region and size), GameWindow.cpp (normalizeWindowRegion, winGetScreenPosition)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/WinInstanceData.cpp (WinInstanceData::init)
//   GeneralsMD/Code/GameEngine/Source/Common/System/RAMFile.cpp (read, scanInt, scanString, nextLine: the WND file is
//     read as a RAMFile)
//   GeneralsMD/Code/GameEngine/Include/GameClient/{GameWindow.h, Gadget.h, WinInstanceData.h} (the status and style
//     bits, the gadget data, WinDrawData, TextDrawData)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   GameWindowManagerScript.cpp: Colin Day, 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The game's window layouts (`.wnd`, GitHub #12): `GameWindowManager::winCreateFromScript`, which reads a layout file
//! and creates its windows: the control bar, the shell menus, every screen of the UI.
//!
//! # As upstream reads it
//!
//! The file is a `RAMFile` (it comes from `WindowZH.big`). The first line is `FILE_VERSION = n` (15 bytes skipped, then
//! `scanInt`, then the rest of the line). From version 2 a layout block must follow:
//!
//! ```text
//! STARTLAYOUTBLOCK
//!   LAYOUTINIT = ControlBarInit;
//!   LAYOUTUPDATE = [None];
//!   LAYOUTSHUTDOWN = [None];
//! ENDLAYOUTBLOCK
//! WINDOW
//!   WINDOWTYPE = USER;
//!   SCREENRECT = UPPERLEFT: 0 448, BOTTOMRIGHT: 800 600, CREATIONRESOLUTION: 800 600;
//!   NAME = "ControlBar.wnd:ControlBarParent";
//!   STATUS = ENABLED+IMAGE;
//!   STYLE = USER;
//!   SYSTEMCALLBACK = "ControlBarSystem";
//!   ...
//!   ENABLEDDRAWDATA = IMAGE: InGameUIAmericaBase, COLOR: 255 255 255 255, BORDERCOLOR: 0 0 0 255,
//!                     IMAGE: NoImage, COLOR: ... (nine of them);
//!   ...
//!   CHILD
//!   WINDOW
//!     ...
//!   END
//!   ENDALLCHILDREN
//! END
//! ```
//!
//! Words are read with `RAMFile::scanString` (white space separated); a field's value with `readUntilSemicolon` into
//! a static 2048-byte buffer (leading white space dropped, the rest of the white space turned into spaces, up to the
//! `;`), and then cut with `strtok`. The first two fields of a window must be `WINDOWTYPE` and `SCREENRECT`, read
//! without looking at their names; the rest are looked up in `gameWindowFieldList` (exact case). A window is created
//! at its `END`, or at `CHILD`, before its children are read; its position is relative to its parent, scaled by the
//! display's size over the creation resolution. Outside windows, `ENABLEDCOLOR`, `DISABLEDCOLOR`, `HILITECOLOR`,
//! `SELECTEDCOLOR`, `TEXTCOLOR`, `BACKGROUNDCOLOR` and `FONT` set defaults (only `TEXTCOLOR` is ever used: it
//! initialises the next windows' text colours), and any other word is skipped.
//!
//! # Upstream quirks that are kept
//!
//! - **The statics.** The 2048-byte buffer is static and never cleared, and `parseFont`, `parseName`, the callback
//!   parsers, `parseHeaderTemplate`, `parseTooltipText` and `parseText` look for the opening `"` from the start of the
//!   buffer, past the NULs `strtok` and earlier reads left: a field without a quote can take one from an older, longer
//!   value. The buffer is modelled byte for byte ([`WindowScript`] keeps it across files, as the game does).
//! - The draw data of the gadgets' inner parts (list box buttons and slider, slider thumbs, combo box parts) are
//!   globals, never reset, not even between files: a list box without its `LISTBOX...DRAWDATA` fields gets the last
//!   ones read anywhere.
//! - The gadget data (`LISTBOXDATA`, `SLIDERDATA`, ...) is read into one static per kind, cleared when a window's
//!   `WINDOWTYPE` asks for it (`getDataTemplate`). The obsolete `DATA` field (`parseData`, "never reached") is
//!   translated too: it reads into a second set of statics, of which the static text's is never cleared.
//! - A window is created again at a second `CHILD` (two windows); fields after `CHILD` (or after a child block) change
//!   nothing, since the window was created; a window whose field parser fails (a text label of 128 bytes or more) is
//!   not created, and a NULL is kept in the layout's window list (or, inside a child block, the parent is destroyed:
//!   see below).
//! - `STATUS` and `STYLE` are `A+B+C` lists compared without case; an unknown name is skipped, and a list starting
//!   with `NULL` sets nothing. The gadget's own style bit is added when it is created.
//! - Text labels and tooltips are looked up in the game text when the window is created; an entry field shows the
//!   label itself, not its translation (`UnicodeString::translate`: each byte becomes a unit, sign-extended as MSVC's
//!   signed `char` converts to its 16-bit `WideChar`).
//! - `parseScreenRect` truncates `position * (display / creation resolution)` toward zero, per corner; the size is the
//!   difference of the scaled corners. A negative size is kept, and the region normalised (`normalizeWindowRegion`).
//! - The layout's `version` is never stored into the info upstream returns (it stays 0).
//! - A layout block without `ENDLAYOUTBLOCK` makes upstream loop forever; a window not closed by `END` too
//!   ([`WndError::Hangs`]).
//!
//! # Where this differs from upstream
//!
//! Where upstream is undefined or does not return, the Rust returns a [`WndError`] and stops: a `strtok` that finds
//! no token where the parser uses one (`NULL` given to `sscanf`, `atoi`, `strcmp`, `strcpy`), a field without any `"`
//! in the buffer, a `strcpy` past its buffer (`type[64]`, `fontName[256]`, `parseBitString`'s 256 bytes, a radio
//! button's 64-byte file name), a number out of `Int`'s range, a scaled position that does not fit an `Int`, a value
//! never read used (a failed `scanInt` into an uninitialised variable), a window type upstream has no branch for (an
//! uninitialised `window`), gadget data of the wrong kind or none, a tab control with more than 8 panes, the end of
//! the file inside `readUntilSemicolon` (upstream then reads stale buffer bytes, and loops forever if the buffer
//! holds white space there), more than 9 nested windows (the stack overflows), and a child block that fails (the
//! stack keeps a destroyed window). White space is the C locale's; a byte of 0x80 or more is not white space (glibc's
//! answer for a negative `char`; MSVC's `isspace` is undefined there).
//!
//! The windows are not the game's `GameWindow`s: the gadgets' own inner windows (list box buttons, slider thumbs,
//! combo box parts, tab panes made by default) are not created, and their draw data is kept on the gadget
//! ([`GameWindow::parts`]). ~~The parent's `GWM_SCRIPT_CREATE` message, the edit data, the system message `GWM_CREATE`
//! and the default font of `winCreate` (the language's, later replaced by the window's own) are not modelled.~~
//! **Corrected 2026-11-10 (#34):** the reader itself still sends no message, but
//! [`crate::client::window_manager::GameWindowManager::win_create_from_script`] runs it and then replays, window by
//! window, `winCreate`'s `GWM_CREATE`, the gadget's callbacks and owner, the file's callbacks, the text and the parent's
//! `GWM_SCRIPT_CREATE`; the edit data and the language's default font are still not modelled. What the
//! window manager supplies (name keys, images, game text, fonts, header templates, the function lexicon and the
//! display's size) is asked of a [`WndHost`].
//!
//! # Representation
//!
//! [`WindowScript`] holds the file's statics; [`WindowScript::win_create_from_script`] returns a
//! [`WindowLayoutInfo`] and adds the windows to a [`WindowTree`], indexed by [`WinId`] (creation order). A window's
//! `children` are in upstream's list order: the newest first (`addWindowToParent` puts a child at the head).

/// `WIN_BUFFER_LENGTH`.
pub const WIN_BUFFER_LENGTH: usize = 2048;
/// `WIN_STACK_DEPTH`.
pub const WIN_STACK_DEPTH: usize = 10;
/// `MAX_DRAW_DATA`.
pub const MAX_DRAW_DATA: usize = 9;
/// `MAX_TEXT_LABEL`.
pub const MAX_TEXT_LABEL: usize = 128;
/// `MAX_WINDOW_NAME_LEN`.
pub const MAX_WINDOW_NAME_LEN: usize = 64;
/// `NUM_TAB_PANES`.
pub const NUM_TAB_PANES: usize = 8;
/// `WIN_COLOR_UNDEFINED` (`GAME_COLOR_UNDEFINED`: white with zero alpha).
pub const WIN_COLOR_UNDEFINED: i32 = 0x00FF_FFFF;

/// `WindowStatusNames`, in bit order.
pub const WINDOW_STATUS_NAMES: &[&str] = &[
    "ACTIVE", "TOGGLE", "DRAGABLE", "ENABLED", "HIDDEN", "ABOVE", "BELOW", "IMAGE", "TABSTOP", "NOINPUT", "NOFOCUS", "DESTROYED", "BORDER",
    "SMOOTH_TEXT", "ONE_LINE", "NO_FLUSH", "SEE_THRU", "RIGHT_CLICK", "WRAP_CENTERED", "CHECK_LIKE", "HOTKEY_TEXT", "USE_OVERLAY_STATES",
    "NOT_READY", "FLASHING", "ALWAYS_COLOR", "ON_MOUSE_DOWN",
];
/// `WindowStyleNames`, in bit order.
pub const WINDOW_STYLE_NAMES: &[&str] = &[
    "PUSHBUTTON", "RADIOBUTTON", "CHECKBOX", "VERTSLIDER", "HORZSLIDER", "SCROLLLISTBOX", "ENTRYFIELD", "STATICTEXT", "PROGRESSBAR", "USER",
    "MOUSETRACK", "ANIMATED", "TABSTOP", "TABCONTROL", "TABPANE", "COMBOBOX",
];

/// The window status bits (`WIN_STATUS_*`).
pub mod win_status {
    pub const ACTIVE: u32 = 0x0000_0001;
    pub const TOGGLE: u32 = 0x0000_0002;
    pub const DRAGABLE: u32 = 0x0000_0004;
    pub const ENABLED: u32 = 0x0000_0008;
    pub const HIDDEN: u32 = 0x0000_0010;
    pub const ABOVE: u32 = 0x0000_0020;
    pub const BELOW: u32 = 0x0000_0040;
    pub const IMAGE: u32 = 0x0000_0080;
    pub const TAB_STOP: u32 = 0x0000_0100;
    pub const NO_INPUT: u32 = 0x0000_0200;
    pub const NO_FOCUS: u32 = 0x0000_0400;
    pub const DESTROYED: u32 = 0x0000_0800;
    pub const BORDER: u32 = 0x0000_1000;
    pub const SMOOTH_TEXT: u32 = 0x0000_2000;
    pub const ONE_LINE: u32 = 0x0000_4000;
    pub const NO_FLUSH: u32 = 0x0000_8000;
    pub const SEE_THRU: u32 = 0x0001_0000;
    pub const RIGHT_CLICK: u32 = 0x0002_0000;
    pub const WRAP_CENTERED: u32 = 0x0004_0000;
    pub const CHECK_LIKE: u32 = 0x0008_0000;
    pub const HOTKEY_TEXT: u32 = 0x0010_0000;
    pub const USE_OVERLAY_STATES: u32 = 0x0020_0000;
    pub const NOT_READY: u32 = 0x0040_0000;
    pub const FLASHING: u32 = 0x0080_0000;
    pub const ALWAYS_COLOR: u32 = 0x0100_0000;
    pub const ON_MOUSE_DOWN: u32 = 0x0200_0000;
}

/// The gadget style bits (`GWS_*`, Gadget.h).
pub mod gws {
    pub const PUSH_BUTTON: u32 = 0x0000_0001;
    pub const RADIO_BUTTON: u32 = 0x0000_0002;
    pub const CHECK_BOX: u32 = 0x0000_0004;
    pub const VERT_SLIDER: u32 = 0x0000_0008;
    pub const HORZ_SLIDER: u32 = 0x0000_0010;
    pub const SCROLL_LISTBOX: u32 = 0x0000_0020;
    pub const ENTRY_FIELD: u32 = 0x0000_0040;
    pub const STATIC_TEXT: u32 = 0x0000_0080;
    pub const PROGRESS_BAR: u32 = 0x0000_0100;
    pub const USER_WINDOW: u32 = 0x0000_0200;
    pub const MOUSE_TRACK: u32 = 0x0000_0400;
    pub const ANIMATED: u32 = 0x0000_0800;
    pub const TAB_STOP: u32 = 0x0000_1000;
    pub const TAB_CONTROL: u32 = 0x0000_2000;
    pub const TAB_PANE: u32 = 0x0000_4000;
    pub const COMBO_BOX: u32 = 0x0000_8000;
}

/// Where upstream is undefined or never returns (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WndError {
    /// `strtok` found no token where one is used.
    NullToken,
    /// No `"` in the whole buffer, where a parser scans for one.
    NoQuote,
    /// A `strcpy` past its destination.
    BufferOverrun,
    /// A number out of `Int`'s range (`sscanf("%d")`, `atoi`).
    IntOutOfRange,
    /// A scaled position that does not fit an `Int`.
    FloatToInt,
    /// A variable used before any value was read into it.
    UninitialisedRead,
    /// A window type upstream has no branch for.
    UnknownWindowType,
    /// Gadget data of the wrong kind, or none, for a gadget data field.
    GadgetDataMismatch,
    /// More than `NUM_TAB_PANES` panes in `TABCONTROLDATA`.
    TabPaneOverrun,
    /// The end of the file inside `readUntilSemicolon`.
    ReadPastEnd,
    /// Upstream loops forever.
    Hangs,
    /// The window stack overflows.
    StackOverflow,
    /// A child block failed (the stack keeps a destroyed window).
    ChildBlockFailed,
    /// `strtok(NULL)` with no string to continue (never in upstream's own flow).
    StrtokState,
}

/// What the window manager supplies (upstream's singletons).
pub trait WndHost {
    /// `TheNameKeyGenerator->nameToKey(name)`.
    fn name_to_key(&mut self, name: &[u8]) -> u32;
    /// `TheMappedImageCollection->findImageByName(name)`: the found image's name.
    fn find_image(&self, name: &[u8]) -> Option<Vec<u8>>;
    /// `TheGameText->fetch(label)`.
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    /// `TheFontLibrary->getFont(name, size, bold)`.
    fn get_font(&self, name: &[u8], size: i32, bold: bool) -> Option<GameFont> {
        Some(GameFont { name: name.to_vec(), point_size: size, bold })
    }
    /// `TheHeaderTemplateManager->getFontFromTemplate(name)`.
    fn font_from_template(&self, _name: &[u8]) -> Option<GameFont> {
        None
    }
    /// Whether `TheFunctionLexicon` has a function of this kind and name (else the pointer is `NULL`).
    fn lexicon_has(&self, _kind: Callback, name: &[u8]) -> bool {
        !name.is_empty() && name != b"[None]"
    }
    /// `TheDisplay->getWidth()`, `getHeight()`.
    fn display_size(&self) -> (i32, i32);
}

/// The callbacks a window or a layout names (`TheFunctionLexicon`'s tables).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Callback {
    System,
    Input,
    Tooltip,
    Draw,
    LayoutInit,
    LayoutUpdate,
    LayoutShutdown,
}

/// A `GameFont`, by what it was asked for with.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GameFont {
    pub name: Vec<u8>,
    pub point_size: i32,
    pub bold: bool,
}

/// `struct WinDrawData`: the image by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WinDrawData {
    pub image: Option<Vec<u8>>,
    pub color: i32,
    pub border_color: i32,
}

impl Default for WinDrawData {
    /// `WinInstanceData::init`'s value (also the globals' zero: see [`WindowScript`]).
    fn default() -> Self {
        WinDrawData { image: None, color: WIN_COLOR_UNDEFINED, border_color: WIN_COLOR_UNDEFINED }
    }
}

/// `struct TextDrawData`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextDrawData {
    pub color: i32,
    pub border_color: i32,
}

/// The nine draw data of each state.
pub type DrawDataSet = [WinDrawData; MAX_DRAW_DATA];

/// `class WinInstanceData`, as the reader fills it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WinInstanceData {
    pub id: i32,
    pub state: i32,
    pub style: u32,
    pub status: u32,
    pub enabled_draw_data: DrawDataSet,
    pub disabled_draw_data: DrawDataSet,
    pub hilite_draw_data: DrawDataSet,
    pub enabled_text: TextDrawData,
    pub disabled_text: TextDrawData,
    pub hilite_text: TextDrawData,
    pub image_offset: (i32, i32),
    pub font: Option<GameFont>,
    pub text_label_string: Vec<u8>,
    pub decorated_name_string: Vec<u8>,
    pub tooltip_string: Vec<u8>,
    pub header_template_name: Vec<u8>,
    pub tooltip_delay: i32,
    /// `m_tooltip`'s text (`setTooltipText`), `None` if never set.
    pub tooltip_text: Option<Vec<u16>>,
}

impl WinInstanceData {
    /// `WinInstanceData::init()` (the header template name is not cleared by `init`; a new one is empty).
    pub fn new() -> Self {
        let u = TextDrawData { color: WIN_COLOR_UNDEFINED, border_color: WIN_COLOR_UNDEFINED };
        WinInstanceData {
            id: 0,
            state: 0,
            style: 0,
            status: 0,
            enabled_draw_data: Default::default(),
            disabled_draw_data: Default::default(),
            hilite_draw_data: Default::default(),
            enabled_text: u,
            disabled_text: u,
            hilite_text: u,
            image_offset: (0, 0),
            font: None,
            text_label_string: Vec::new(),
            decorated_name_string: Vec::new(),
            tooltip_string: Vec::new(),
            header_template_name: Vec::new(),
            tooltip_delay: -1,
            tooltip_text: None,
        }
    }
}

impl Default for WinInstanceData {
    fn default() -> Self {
        Self::new()
    }
}

/// `SliderData`'s fields the file sets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SliderData {
    pub min_val: i32,
    pub max_val: i32,
}

/// `ListboxData`'s fields the file sets.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListboxData {
    pub list_length: i16,
    pub columns: i16,
    /// `columnWidthPercentage` (`NULL`: empty).
    pub column_width_percentage: Vec<i32>,
    pub auto_scroll: bool,
    pub auto_purge: bool,
    pub scroll_bar: bool,
    pub multi_select: bool,
    pub force_select: bool,
    pub scroll_if_at_end: bool,
}

/// `ComboBoxData`'s fields the file sets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ComboBoxData {
    pub is_editable: bool,
    pub max_display: i32,
    pub max_chars: i32,
    pub ascii_only: bool,
    pub letters_and_numbers_only: bool,
}

/// `EntryData`'s fields the file sets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntryData {
    pub secret_text: bool,
    pub numerical_only: bool,
    pub alpha_numerical_only: bool,
    pub ascii_only: bool,
    pub max_text_len: i16,
}

/// `TextData`'s fields the file sets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextData {
    pub centered: bool,
    pub centered_vertically: bool,
    pub left_margin: i32,
    pub top_margin: i32,
}

/// `RadioButtonData`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RadioButtonData {
    pub screen: i32,
    pub group: i32,
}

/// `TabControlData`'s fields the file sets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TabControlData {
    pub tab_orientation: i32,
    pub tab_edge: i32,
    pub tab_width: i32,
    pub tab_height: i32,
    pub tab_count: i32,
    pub sub_pane_disabled: [bool; NUM_TAB_PANES],
    pub pane_border: i32,
}

/// A gadget's data, as it was when the gadget was created.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GadgetData {
    None,
    Slider(SliderData),
    Listbox(ListboxData),
    ComboBox(ComboBoxData),
    Entry(EntryData),
    Text(TextData),
    Radio(RadioButtonData),
    TabControl(TabControlData),
}

/// The kinds of gadget data (the statics of `getDataTemplate` and `parseData`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DataKind {
    Slider,
    Listbox,
    TabControl,
    Entry,
    Text,
    Radio,
    ComboBox,
}

/// One set of gadget data statics.
#[derive(Clone, Debug, Default)]
struct DataStatics {
    e: EntryData,
    s: SliderData,
    l: ListboxData,
    t: TextData,
    r: RadioButtonData,
    tc: TabControlData,
    c: ComboBoxData,
}

/// Which static a window's `data` pointer points to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DataRef {
    /// `parseData`'s statics rather than `getDataTemplate`'s.
    parsed: bool,
    kind: DataKind,
}

/// The inner parts of a gadget whose draw data the file gives (the globals of GameWindowManagerScript.cpp).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GadgetPart {
    ListBoxUpButton,
    ListBoxDownButton,
    ListBoxSlider,
    SliderThumb,
    ComboBoxDropDownButton,
    ComboBoxEditBox,
    ComboBoxListBox,
}

/// The enabled, disabled and hilite draw data of a part.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PartDrawData {
    pub enabled: DrawDataSet,
    pub disabled: DrawDataSet,
    pub hilite: DrawDataSet,
}

/// A window's index in its [`WindowTree`].
pub type WinId = usize;

/// A `GameWindow`, as the reader creates it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameWindow {
    /// `m_parent`.
    pub parent: Option<WinId>,
    /// `m_child` and its siblings: newest first.
    pub children: Vec<WinId>,
    /// The `WINDOWTYPE` it was created as.
    pub window_type: Vec<u8>,
    /// `m_status`.
    pub status: u32,
    /// `m_size`.
    pub size: (i32, i32),
    /// `m_region` (lo.x, lo.y, hi.x, hi.y), relative to the parent, normalised.
    pub region: (i32, i32, i32, i32),
    /// `winGetWindowId()`.
    pub id: i32,
    /// `m_instData`.
    pub inst: WinInstanceData,
    /// The gadget's data.
    pub gadget: GadgetData,
    /// The draw data given to the gadget's inner parts.
    pub parts: Vec<(GadgetPart, PartDrawData)>,
    /// The text `setWindowText` set (`None`: none).
    pub text: Option<Vec<u16>>,
    /// The callbacks the lexicon resolved, by name (`system`, `input`, `tooltip`, `draw`); a gadget's own
    /// (`GadgetPushButtonSystem`, ...) where `None`.
    pub callbacks: [Option<Vec<u8>>; 4],
}

impl GameWindow {
    /// `winGetStyle()`.
    pub fn style(&self) -> u32 {
        self.inst.style
    }
    /// The decorated name (`ControlBar.wnd:ButtonCommand01`).
    pub fn name(&self) -> &[u8] {
        &self.inst.decorated_name_string
    }
    /// `winIsHidden()`.
    pub fn is_hidden(&self) -> bool {
        self.status & win_status::HIDDEN != 0
    }
}

/// The windows created so far (the window manager's list).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WindowTree {
    pub windows: Vec<GameWindow>,
}

impl WindowTree {
    /// `winGetScreenPosition`: the region's corner plus every ancestor's.
    pub fn screen_position(&self, id: WinId) -> (i32, i32) {
        let w = &self.windows[id];
        let (mut x, mut y) = (w.region.0, w.region.1);
        let mut p = w.parent;
        while let Some(i) = p {
            x = x.wrapping_add(self.windows[i].region.0);
            y = y.wrapping_add(self.windows[i].region.1);
            p = self.windows[i].parent;
        }
        (x, y)
    }
    /// The first window with this decorated name (`TheWindowManager->winGetWindowFromId(NULL, nameToKey(name))`).
    pub fn find_by_name(&self, name: &[u8]) -> Option<WinId> {
        self.windows.iter().position(|w| w.inst.decorated_name_string == name)
    }
}

/// `WindowLayoutInfo`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WindowLayoutInfo {
    /// `version` (never set by `winCreateFromScript`: 0).
    pub version: u32,
    pub init_name_string: Vec<u8>,
    pub update_name_string: Vec<u8>,
    pub shutdown_name_string: Vec<u8>,
    /// Whether the lexicon resolved `init`, `update`, `shutdown`.
    pub resolved: [bool; 3],
    /// `windows`: each top-level `WINDOW`'s window, `None` for upstream's `NULL`.
    pub windows: Vec<Option<WinId>>,
}

/// `winCreateFromScript`'s result: the first window, and the info (`None` when the layout block failed: upstream
/// returns `NULL` and leaves the caller's info as it was).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptResult {
    pub first_window: Option<WinId>,
    pub info: Option<WindowLayoutInfo>,
}

/// `Window\<name>` unless the name has a `\` (`winCreateFromScript`'s path).
pub fn window_script_path(filename: &[u8]) -> Vec<u8> {
    if filename.contains(&b'\\') {
        filename.to_vec()
    } else {
        [b"Window\\".as_slice(), filename].concat()
    }
}

// ---------------------------------------------------------------------------
// C helpers
// ---------------------------------------------------------------------------

/// `isspace` in the C locale (a byte of 0x80 or more is not white space; see the module docs).
fn c_isspace(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// `strtol`-style decimal: white space, sign, digits; `None` without a digit. Out of `Int` is undefined.
fn c_decimal(s: &[u8]) -> Result<Option<i32>, WndError> {
    let mut i = 0;
    while i < s.len() && c_isspace(s[i]) {
        i += 1;
    }
    let mut neg = false;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        neg = s[i] == b'-';
        i += 1;
    }
    let start = i;
    let mut mag: i64 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        mag = mag.saturating_mul(10).saturating_add(i64::from(s[i] - b'0'));
        i += 1;
    }
    if i == start {
        return Ok(None);
    }
    let v = if neg { -mag } else { mag };
    i32::try_from(v).map(Some).map_err(|_| WndError::IntOutOfRange)
}

/// `atoi(c)`.
fn c_atoi(c: Option<&[u8]>) -> Result<i32, WndError> {
    Ok(c_decimal(c.ok_or(WndError::NullToken)?)?.unwrap_or(0))
}

/// `sscanf(c, "%d", &val)`: the value, `None` when nothing matched.
fn c_scan(c: Option<&[u8]>) -> Result<Option<i32>, WndError> {
    c_decimal(c.ok_or(WndError::NullToken)?)
}

/// `scanInt(c, val)` / `scanUnsignedInt`: `val` unchanged when nothing matched.
fn scan_into(c: Option<&[u8]>, val: &mut Option<i32>) -> Result<(), WndError> {
    if let Some(v) = c_scan(c)? {
        *val = Some(v);
    }
    Ok(())
}

/// `scanInt(c, val)` into an initialised variable.
fn scan_int_init(c: Option<&[u8]>, val: &mut i32) -> Result<(), WndError> {
    if let Some(v) = c_scan(c)? {
        *val = v;
    }
    Ok(())
}

/// `scanBool(c, val)`: `temp = 0`, `sscanf`, `val = (Bool)temp`.
fn scan_bool(c: Option<&[u8]>) -> Result<bool, WndError> {
    Ok(c_scan(c)?.unwrap_or(0) != 0)
}

/// `scanShort(c, val)`: `val = (Short)temp`.
fn scan_short(c: Option<&[u8]>) -> Result<i16, WndError> {
    Ok(c_scan(c)?.unwrap_or(0) as i16)
}

/// `GameMakeColor(r, g, b, a)` of `UnsignedInt`s (each cut to a byte).
fn make_color(r: Option<i32>, g: Option<i32>, b: Option<i32>, a: Option<i32>) -> Result<i32, WndError> {
    let u = |v: Option<i32>| v.map(|x| x as u8).ok_or(WndError::UninitialisedRead);
    Ok(crate::common::ini::game_make_color(u(r)?, u(g)?, u(b)?, u(a)?))
}

/// The bytes of a C string from `at` (to its NUL).
fn c_str_at(buf: &[u8], at: usize) -> &[u8] {
    let rest = &buf[at.min(buf.len())..];
    &rest[..rest.iter().position(|&c| c == 0).unwrap_or(rest.len())]
}

/// `(WideChar)c` of each byte (`UnicodeString::translate`).
fn translate(label: &[u8]) -> Vec<u16> {
    label.iter().map(|&b| b as i8 as i16 as u16).collect()
}

/// The file, read as `RAMFile` reads it.
struct RamFile<'a> {
    data: &'a [u8],
    pos: usize,
}

impl RamFile<'_> {
    /// `read(NULL, n)`.
    fn skip(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.data.len());
    }
    /// `read(&c, 1)`: `None` at the end.
    fn read_byte(&mut self) -> Option<u8> {
        let b = self.data.get(self.pos).copied();
        if b.is_some() {
            self.pos += 1;
        }
        b
    }
    /// `scanString`: `None` (FALSE, the string cleared) at the end. A NUL byte adds nothing to the string
    /// (`concat(char)` of `{ c, 0 }`).
    fn scan_string(&mut self) -> Option<Vec<u8>> {
        while self.pos < self.data.len() && c_isspace(self.data[self.pos]) {
            self.pos += 1;
        }
        if self.pos >= self.data.len() {
            return None;
        }
        let mut s = Vec::new();
        loop {
            let c = self.data[self.pos];
            if c != 0 {
                s.push(c);
            }
            self.pos += 1;
            if self.pos >= self.data.len() || c_isspace(self.data[self.pos]) {
                break;
            }
        }
        Some(s)
    }
    /// `scanInt`: 0 at the end; `atoi` of the `-` or digit and the digits after it.
    fn scan_int(&mut self) -> Result<i32, WndError> {
        let d = self.data;
        while self.pos < d.len() && !d[self.pos].is_ascii_digit() && d[self.pos] != b'-' {
            self.pos += 1;
        }
        if self.pos >= d.len() {
            return Ok(0);
        }
        let start = self.pos;
        self.pos += 1;
        while self.pos < d.len() && d[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        Ok(c_decimal(&d[start..self.pos])?.unwrap_or(0))
    }
    /// `nextLine()`.
    fn next_line(&mut self) {
        while self.pos < self.data.len() && self.data[self.pos] != b'\n' {
            self.pos += 1;
        }
        if self.pos < self.data.len() {
            self.pos += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// The reader
// ---------------------------------------------------------------------------

/// The statics of GameWindowManagerScript.cpp, which persist across files.
#[derive(Clone, Debug)]
pub struct WindowScript {
    /// `winCreateFromScript`'s `static char buffer[WIN_BUFFER_LENGTH]`.
    buffer: Vec<u8>,
    /// `strtok`'s saved position in the buffer (`None`: past its end; `Some(None)`: never set or in another buffer).
    strtok_next: Option<Option<usize>>,
    /// `windowStack`.
    stack: Vec<WinId>,
    /// `defEnabledColor` ... `defTextColor`.
    pub def_colors: DefaultColors,
    /// The gadget parts' draw data (`enabledUpButtonDrawData`, ...): never reset.
    parts: [PartDrawData; 7],
    /// `getDataTemplate`'s statics.
    template_data: DataStatics,
    /// `parseData`'s statics.
    parsed_data: DataStatics,
    /// `systemFunc` ... `drawFunc` (by name, when resolved) and `theSystemString` ... `theDrawString`.
    funcs: [Option<Vec<u8>>; 4],
}

/// The default colours of the file (`defEnabledColor`, ...).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DefaultColors {
    pub enabled: i32,
    pub disabled: i32,
    pub background: i32,
    pub hilite: i32,
    pub selected: i32,
    pub text: i32,
}

impl Default for WindowScript {
    fn default() -> Self {
        Self::new()
    }
}

/// The parts in `GadgetPart` order, for the tables below.
const PART_ORDER: [GadgetPart; 7] = [
    GadgetPart::ListBoxUpButton,
    GadgetPart::ListBoxDownButton,
    GadgetPart::ListBoxSlider,
    GadgetPart::SliderThumb,
    GadgetPart::ComboBoxDropDownButton,
    GadgetPart::ComboBoxEditBox,
    GadgetPart::ComboBoxListBox,
];

/// Which draw data a `...DRAWDATA` field fills: the window's own state (0 enabled, 1 disabled, 2 hilite), or a part's.
fn draw_data_target(token: &[u8]) -> Option<(Option<GadgetPart>, usize)> {
    use GadgetPart::*;
    Some(match token {
        b"ENABLEDDRAWDATA" => (None, 0),
        b"DISABLEDDRAWDATA" => (None, 1),
        b"HILITEDRAWDATA" => (None, 2),
        b"LISTBOXENABLEDUPBUTTONDRAWDATA" => (Some(ListBoxUpButton), 0),
        b"LISTBOXDISABLEDUPBUTTONDRAWDATA" => (Some(ListBoxUpButton), 1),
        b"LISTBOXHILITEUPBUTTONDRAWDATA" => (Some(ListBoxUpButton), 2),
        b"LISTBOXENABLEDDOWNBUTTONDRAWDATA" => (Some(ListBoxDownButton), 0),
        b"LISTBOXDISABLEDDOWNBUTTONDRAWDATA" => (Some(ListBoxDownButton), 1),
        b"LISTBOXHILITEDOWNBUTTONDRAWDATA" => (Some(ListBoxDownButton), 2),
        b"LISTBOXENABLEDSLIDERDRAWDATA" => (Some(ListBoxSlider), 0),
        b"LISTBOXDISABLEDSLIDERDRAWDATA" => (Some(ListBoxSlider), 1),
        b"LISTBOXHILITESLIDERDRAWDATA" => (Some(ListBoxSlider), 2),
        b"SLIDERTHUMBENABLEDDRAWDATA" => (Some(SliderThumb), 0),
        b"SLIDERTHUMBDISABLEDDRAWDATA" => (Some(SliderThumb), 1),
        b"SLIDERTHUMBHILITEDRAWDATA" => (Some(SliderThumb), 2),
        b"COMBOBOXDROPDOWNBUTTONENABLEDDRAWDATA" => (Some(ComboBoxDropDownButton), 0),
        b"COMBOBOXDROPDOWNBUTTONDISABLEDDRAWDATA" => (Some(ComboBoxDropDownButton), 1),
        b"COMBOBOXDROPDOWNBUTTONHILITEDRAWDATA" => (Some(ComboBoxDropDownButton), 2),
        b"COMBOBOXEDITBOXENABLEDDRAWDATA" => (Some(ComboBoxEditBox), 0),
        b"COMBOBOXEDITBOXDISABLEDDRAWDATA" => (Some(ComboBoxEditBox), 1),
        b"COMBOBOXEDITBOXHILITEDRAWDATA" => (Some(ComboBoxEditBox), 2),
        b"COMBOBOXLISTBOXENABLEDDRAWDATA" => (Some(ComboBoxListBox), 0),
        b"COMBOBOXLISTBOXDISABLEDDRAWDATA" => (Some(ComboBoxListBox), 1),
        b"COMBOBOXLISTBOXHILITEDRAWDATA" => (Some(ComboBoxListBox), 2),
        _ => return None,
    })
}

/// The names of `gameWindowFieldList` (the draw data fields are found by [`draw_data_target`]).
const FIELDS: &[&[u8]] = &[
    b"NAME", b"STATUS", b"STYLE", b"SYSTEMCALLBACK", b"INPUTCALLBACK", b"TOOLTIPCALLBACK", b"DRAWCALLBACK", b"FONT", b"HEADERTEMPLATE",
    b"LISTBOXDATA", b"COMBOBOXDATA", b"SLIDERDATA", b"RADIOBUTTONDATA", b"TOOLTIPTEXT", b"TOOLTIPDELAY", b"TEXT", b"TEXTCOLOR",
    b"STATICTEXTDATA", b"TEXTENTRYDATA", b"TABCONTROLDATA", b"IMAGEOFFSET", b"TOOLTIP",
];

/// The parser's view of one `parseWindow`: the window's type, its data and the instance data.
struct Pending {
    window_type: Vec<u8>,
    data: Option<DataRef>,
    inst: WinInstanceData,
}

impl WindowScript {
    /// The statics as the program starts (zeroed; the parts' draw data are zero: no image, colour 0).
    pub fn new() -> Self {
        let zero = WinDrawData { image: None, color: 0, border_color: 0 };
        let set: DrawDataSet = std::array::from_fn(|_| zero.clone());
        let part = PartDrawData { enabled: set.clone(), disabled: set.clone(), hilite: set };
        WindowScript {
            buffer: vec![0; WIN_BUFFER_LENGTH],
            strtok_next: Some(None),
            stack: Vec::new(),
            def_colors: DefaultColors::default(),
            parts: std::array::from_fn(|_| part.clone()),
            template_data: DataStatics::default(),
            parsed_data: DataStatics::default(),
            funcs: Default::default(),
        }
    }

    // -- strtok over the static buffer ---------------------------------------

    /// `strtok(start or NULL, seps)`: the token's start in the buffer.
    fn strtok(&mut self, start: Option<usize>, seps: &[u8]) -> Result<Option<usize>, WndError> {
        let mut s = match start {
            Some(s) => s,
            None => match self.strtok_next {
                Some(Some(s)) => s,
                Some(None) => return Err(WndError::StrtokState),
                None => return Ok(None),
            },
        };
        let b = &mut self.buffer;
        let n = b.len();
        while s < n && b[s] != 0 && seps.contains(&b[s]) {
            s += 1;
        }
        if s >= n || b[s] == 0 {
            self.strtok_next = None;
            return Ok(None);
        }
        let begin = s;
        while s < n && b[s] != 0 && !seps.contains(&b[s]) {
            s += 1;
        }
        if s < n && b[s] != 0 {
            b[s] = 0;
            self.strtok_next = Some(Some(s + 1));
        } else {
            self.strtok_next = None;
        }
        Ok(Some(begin))
    }

    /// `strtok`, and the token's bytes.
    fn tok(&mut self, start: Option<usize>, seps: &[u8]) -> Result<Option<Vec<u8>>, WndError> {
        Ok(self.strtok(start, seps)?.map(|at| c_str_at(&self.buffer, at).to_vec()))
    }

    /// `ptr = buffer; while (*ptr != '"') ptr++; ptr++;`.
    fn after_quote(&self) -> Result<usize, WndError> {
        self.buffer.iter().position(|&c| c == b'"').map(|p| p + 1).ok_or(WndError::NoQuote)
    }

    /// `readUntilSemicolon(fp, buffer, WIN_BUFFER_LENGTH)`.
    fn read_until_semicolon(&mut self, f: &mut RamFile<'_>) -> Result<(), WndError> {
        let mut i = 0;
        let mut start = true;
        while i < WIN_BUFFER_LENGTH {
            let c = f.read_byte().ok_or(WndError::ReadPastEnd)?;
            self.buffer[i] = c;
            if c_isspace(c) {
                if !start {
                    self.buffer[i] = b' ';
                    i += 1;
                }
            } else {
                start = false;
                if c == b';' {
                    self.buffer[i] = 0;
                    return Ok(());
                }
                i += 1;
            }
        }
        self.buffer[WIN_BUFFER_LENGTH - 1] = 0;
        Ok(())
    }

    // -- the field parsers ----------------------------------------------------

    /// `parseBitString(buffer, bits, flagList)`: sets the named bits (never clears).
    fn parse_bit_string(&mut self, bits: &mut u32, names: &[&str]) -> Result<(), WndError> {
        let input = c_str_at(&self.buffer, 0).to_vec();
        if input.len() >= 256 {
            return Err(WndError::BufferOverrun);
        }
        // (strtok now points into the local copy)
        self.strtok_next = Some(None);
        if input.len() >= 4 && &input[..4] == b"NULL" {
            return Ok(());
        }
        for tok in input.split(|&c| c == b'+').filter(|t| !t.is_empty()) {
            // parseBitFlag
            if let Some(i) = names.iter().position(|n| n.as_bytes().eq_ignore_ascii_case(tok)) {
                *bits |= 1 << i;
            }
        }
        Ok(())
    }

    /// `parseColor(color, buffer)`.
    fn parse_color(&mut self) -> Result<i32, WndError> {
        let seps = b" \t\n\r";
        let r = c_atoi(self.tok(Some(0), seps)?.as_deref())?;
        let g = c_atoi(self.tok(None, seps)?.as_deref())?;
        let b = c_atoi(self.tok(None, seps)?.as_deref())?;
        // (`Byte red = atoi(c)`, then `UnsignedByte`)
        Ok(crate::common::ini::game_make_color(r as u8, g as u8, b as u8, 255))
    }

    /// `parseDefaultColor(color, inFile, buffer)`.
    fn parse_default_color(&mut self, f: &mut RamFile<'_>) -> Result<i32, WndError> {
        f.scan_string(); // eat '='
        self.read_until_semicolon(f)?;
        if c_str_at(&self.buffer, 0) == b"TRANSPARENT" {
            Ok(WIN_COLOR_UNDEFINED)
        } else {
            self.parse_color()
        }
    }

    /// `parseDefaultFont(font, inFile, buffer)`: read and ignored (`@todo`).
    fn parse_default_font(&mut self, f: &mut RamFile<'_>) -> Result<(), WndError> {
        f.scan_string();
        self.read_until_semicolon(f)
    }

    /// `parseScreenRect`: the position relative to the parent, and the size.
    fn parse_screen_rect(&mut self, tree: &WindowTree, host: &impl WndHost) -> Result<(i32, i32, i32, i32), WndError> {
        let seps = b" ,:=\n\r\t";
        let mut v: [Option<i32>; 6] = [None; 6];
        self.strtok(None, seps)?; // UPPERLEFT
        scan_into(self.tok(None, seps)?.as_deref(), &mut v[0])?;
        scan_into(self.tok(None, seps)?.as_deref(), &mut v[1])?;
        self.strtok(None, seps)?; // BOTTOMRIGHT
        scan_into(self.tok(None, seps)?.as_deref(), &mut v[2])?;
        scan_into(self.tok(None, seps)?.as_deref(), &mut v[3])?;
        self.strtok(None, seps)?; // CREATIONRESOLUTION
        scan_into(self.tok(None, seps)?.as_deref(), &mut v[4])?;
        scan_into(self.tok(None, seps)?.as_deref(), &mut v[5])?;
        let g = |i: usize| v[i].ok_or(WndError::UninitialisedRead);
        let (dw, dh) = host.display_size();
        let x_scale = dw as f32 / g(4)? as f32;
        let y_scale = dh as f32 / g(5)? as f32;
        let to_int = |p: i32, s: f32| -> Result<i32, WndError> {
            let r = p as f32 * s;
            if (-2_147_483_648.0..2_147_483_648.0).contains(&r) {
                Ok(r as i32)
            } else {
                Err(WndError::FloatToInt)
            }
        };
        let lo = (to_int(g(0)?, x_scale)?, to_int(g(1)?, y_scale)?);
        let hi = (to_int(g(2)?, x_scale)?, to_int(g(3)?, y_scale)?);
        let (x, y) = match self.stack.last() {
            Some(&p) => {
                let (px, py) = tree.screen_position(p);
                (lo.0.wrapping_sub(px), lo.1.wrapping_sub(py))
            }
            None => lo,
        };
        Ok((x, y, hi.0.wrapping_sub(lo.0), hi.1.wrapping_sub(lo.1)))
    }

    /// The `"name"` of `parseName`, the callback parsers and `parseHeaderTemplate`: `strtok(ptr, "\"")` after the first
    /// quote (`NULL` sets the empty string).
    fn quoted(&mut self) -> Result<Vec<u8>, WndError> {
        let p = self.after_quote()?;
        Ok(self.tok(Some(p), b"\"")?.unwrap_or_default())
    }

    /// The gadget data static a data field writes, which must be of `kind`.
    fn data_mut(&mut self, data: Option<DataRef>, kind: DataKind) -> Result<&mut DataStatics, WndError> {
        match data {
            Some(d) if d.kind == kind => {
                Ok(if d.parsed { &mut self.parsed_data } else { &mut self.template_data })
            }
            _ => Err(WndError::GadgetDataMismatch),
        }
    }

    /// `parseDrawData(token, instData, buffer)`.
    fn parse_draw_data(&mut self, token: &[u8], inst: &mut WinInstanceData, host: &impl WndHost) -> Result<bool, WndError> {
        let Some((part, state)) = draw_data_target(token) else { return Ok(false) };
        let seps = b" :,\n\r\t";
        let (mut r, mut g, mut b, mut a): (Option<i32>, Option<i32>, Option<i32>, Option<i32>) = (None, None, None, None);
        for i in 0..MAX_DRAW_DATA {
            let first = i == 0;
            self.strtok(if first { Some(0) } else { None }, seps)?; // IMAGE label
            let c = self.tok(None, seps)?.ok_or(WndError::NullToken)?;
            let image = if c != b"NoImage" { host.find_image(&c) } else { None };
            self.strtok(None, seps)?; // COLOR
            scan_into(self.tok(None, seps)?.as_deref(), &mut r)?;
            scan_into(self.tok(None, seps)?.as_deref(), &mut g)?;
            scan_into(self.tok(None, seps)?.as_deref(), &mut b)?;
            scan_into(self.tok(None, seps)?.as_deref(), &mut a)?;
            let color = make_color(r, g, b, a)?;
            self.strtok(None, seps)?; // BORDERCOLOR
            scan_into(self.tok(None, seps)?.as_deref(), &mut r)?;
            scan_into(self.tok(None, seps)?.as_deref(), &mut g)?;
            scan_into(self.tok(None, seps)?.as_deref(), &mut b)?;
            scan_into(self.tok(None, seps)?.as_deref(), &mut a)?;
            let border_color = make_color(r, g, b, a)?;
            let dd = WinDrawData { image, color, border_color };
            let target = match part {
                None => match state {
                    0 => &mut inst.enabled_draw_data,
                    1 => &mut inst.disabled_draw_data,
                    _ => &mut inst.hilite_draw_data,
                },
                Some(p) => {
                    let pd = &mut self.parts[PART_ORDER.iter().position(|&q| q == p).expect("part")];
                    match state {
                        0 => &mut pd.enabled,
                        1 => &mut pd.disabled,
                        _ => &mut pd.hilite,
                    }
                }
            };
            target[i] = dd;
        }
        Ok(true)
    }

    /// One field of `gameWindowFieldList`, its value in the buffer: `Ok(false)` where the parser returns `FALSE`.
    fn parse_field(&mut self, token: &[u8], p: &mut Pending, host: &mut impl WndHost) -> Result<bool, WndError> {
        let seps = b" :,\n\r\t";
        match token {
            b"NAME" => {
                let c = self.quoted()?;
                p.inst.id = host.name_to_key(&c) as i32;
                p.inst.decorated_name_string = c;
            }
            b"STATUS" => {
                p.inst.status = 0;
                let mut s = 0;
                self.parse_bit_string(&mut s, WINDOW_STATUS_NAMES)?;
                p.inst.status = s;
            }
            b"STYLE" => {
                let mut s = 0;
                self.parse_bit_string(&mut s, WINDOW_STYLE_NAMES)?;
                p.inst.style = s;
            }
            b"SYSTEMCALLBACK" | b"INPUTCALLBACK" | b"TOOLTIPCALLBACK" | b"DRAWCALLBACK" => {
                let (slot, kind) = match token {
                    b"SYSTEMCALLBACK" => (0, Callback::System),
                    b"INPUTCALLBACK" => (1, Callback::Input),
                    b"TOOLTIPCALLBACK" => (2, Callback::Tooltip),
                    _ => (3, Callback::Draw),
                };
                let c = self.quoted()?;
                host.name_to_key(&c);
                self.funcs[slot] = host.lexicon_has(kind, &c).then_some(c);
            }
            b"FONT" => {
                self.strtok(Some(0), b" ,\n\r\t")?; // label
                let ptr = self.after_quote()?;
                let name = self.tok(Some(ptr), b":,\n\r\t\"")?.ok_or(WndError::NullToken)?;
                if name.len() >= 256 {
                    return Err(WndError::BufferOverrun);
                }
                let (mut size, mut bold) = (None, None);
                self.strtok(None, b" ,\n\r\t")?;
                scan_into(self.tok(None, b" ,\n\r\t")?.as_deref(), &mut size)?;
                self.strtok(None, b" ,\n\r\t")?;
                scan_into(self.tok(None, b" ,\n\r\t")?.as_deref(), &mut bold)?;
                let size = size.ok_or(WndError::UninitialisedRead)?;
                let bold = bold.ok_or(WndError::UninitialisedRead)? != 0;
                if let Some(font) = host.get_font(&name, size, bold) {
                    p.inst.font = Some(font);
                }
            }
            b"HEADERTEMPLATE" => p.inst.header_template_name = self.quoted()?,
            b"LISTBOXDATA" => {
                self.data_mut(p.data, DataKind::Listbox)?;
                self.strtok(Some(0), seps)?; // LENGTH
                let length = scan_short(self.tok(None, seps)?.as_deref())?;
                self.strtok(None, seps)?;
                let auto_scroll = scan_bool(self.tok(None, seps)?.as_deref())?;
                let label = self.tok(None, seps)?.ok_or(WndError::NullToken)?;
                let scroll_if_at_end = if label.eq_ignore_ascii_case(b"ScrollIfAtEnd") {
                    let v = scan_bool(self.tok(None, seps)?.as_deref())?;
                    self.strtok(None, seps)?;
                    v
                } else {
                    false
                };
                let auto_purge = scan_bool(self.tok(None, seps)?.as_deref())?;
                self.strtok(None, seps)?;
                let scroll_bar = scan_bool(self.tok(None, seps)?.as_deref())?;
                self.strtok(None, seps)?;
                let multi_select = scan_bool(self.tok(None, seps)?.as_deref())?;
                self.strtok(None, seps)?;
                let columns = scan_short(self.tok(None, seps)?.as_deref())?;
                let mut widths = Vec::new();
                if columns > 1 {
                    for _ in 0..columns {
                        self.strtok(None, seps)?;
                        let mut w = None;
                        scan_into(self.tok(None, seps)?.as_deref(), &mut w)?;
                        widths.push(w.ok_or(WndError::UninitialisedRead)?);
                    }
                }
                self.strtok(None, seps)?;
                let force_select = scan_bool(self.tok(None, seps)?.as_deref())?;
                // (each value is stored as it is read; an error part-way stops the parse: see the module docs)
                let l = &mut self.data_mut(p.data, DataKind::Listbox)?.l;
                l.list_length = length;
                l.auto_scroll = auto_scroll;
                l.scroll_if_at_end = scroll_if_at_end;
                l.auto_purge = auto_purge;
                l.scroll_bar = scroll_bar;
                l.multi_select = multi_select;
                l.columns = columns;
                l.column_width_percentage = widths;
                l.force_select = force_select;
            }
            b"COMBOBOXDATA" => {
                self.data_mut(p.data, DataKind::ComboBox)?;
                self.strtok(Some(0), seps)?;
                let is_editable = scan_bool(self.tok(None, seps)?.as_deref())?;
                let mut c = self.data_mut(p.data, DataKind::ComboBox)?.c;
                c.is_editable = is_editable;
                self.strtok(None, seps)?;
                scan_int_init(self.tok(None, seps)?.as_deref(), &mut c.max_chars)?;
                self.strtok(None, seps)?;
                scan_int_init(self.tok(None, seps)?.as_deref(), &mut c.max_display)?;
                self.strtok(None, seps)?;
                c.ascii_only = scan_bool(self.tok(None, seps)?.as_deref())?;
                self.strtok(None, seps)?;
                c.letters_and_numbers_only = scan_bool(self.tok(None, seps)?.as_deref())?;
                self.data_mut(p.data, DataKind::ComboBox)?.c = c;
            }
            b"SLIDERDATA" => {
                let mut s = self.data_mut(p.data, DataKind::Slider)?.s;
                self.strtok(Some(0), seps)?;
                scan_int_init(self.tok(None, seps)?.as_deref(), &mut s.min_val)?;
                self.strtok(None, seps)?;
                scan_int_init(self.tok(None, seps)?.as_deref(), &mut s.max_val)?;
                self.data_mut(p.data, DataKind::Slider)?.s = s;
            }
            b"RADIOBUTTONDATA" => {
                let mut r = self.data_mut(p.data, DataKind::Radio)?.r;
                self.strtok(Some(0), seps)?;
                scan_int_init(self.tok(None, seps)?.as_deref(), &mut r.group)?;
                self.data_mut(p.data, DataKind::Radio)?.r = r;
            }
            b"TOOLTIPTEXT" => {
                let ptr = self.after_quote()?;
                if c_str_at(&self.buffer, ptr).len() == 1 {
                    return Ok(true);
                }
                let c = self.tok(Some(ptr), b"\n\r\t\"")?.ok_or(WndError::NullToken)?;
                if c.len() >= MAX_TEXT_LABEL {
                    return Ok(false);
                }
                p.inst.tooltip_text = Some(host.fetch_text(&c));
                p.inst.tooltip_string = c;
            }
            b"TOOLTIPDELAY" => scan_int_init(self.tok(Some(0), seps)?.as_deref(), &mut p.inst.tooltip_delay)?,
            b"TEXT" => {
                let ptr = self.after_quote()?;
                let c = self.tok(Some(ptr), b"\n\r\t\"")?.ok_or(WndError::NullToken)?;
                if c.len() >= MAX_TEXT_LABEL {
                    return Ok(false);
                }
                p.inst.text_label_string = c;
            }
            b"TEXTCOLOR" => {
                let (mut r, mut g, mut b, mut a): (Option<i32>, Option<i32>, Option<i32>, Option<i32>) = (None, None, None, None);
                for i in 0..3 {
                    self.strtok(if i == 0 { Some(0) } else { None }, seps)?;
                    scan_into(self.tok(None, seps)?.as_deref(), &mut r)?;
                    scan_into(self.tok(None, seps)?.as_deref(), &mut g)?;
                    scan_into(self.tok(None, seps)?.as_deref(), &mut b)?;
                    scan_into(self.tok(None, seps)?.as_deref(), &mut a)?;
                    let color = make_color(r, g, b, a)?;
                    self.strtok(None, seps)?;
                    scan_into(self.tok(None, seps)?.as_deref(), &mut r)?;
                    scan_into(self.tok(None, seps)?.as_deref(), &mut g)?;
                    scan_into(self.tok(None, seps)?.as_deref(), &mut b)?;
                    scan_into(self.tok(None, seps)?.as_deref(), &mut a)?;
                    let border_color = make_color(r, g, b, a)?;
                    let t = TextDrawData { color, border_color };
                    match i {
                        0 => p.inst.enabled_text = t,
                        1 => p.inst.disabled_text = t,
                        _ => p.inst.hilite_text = t,
                    }
                }
            }
            b"STATICTEXTDATA" => {
                self.data_mut(p.data, DataKind::Text)?;
                self.strtok(Some(0), seps)?;
                let centered = scan_bool(self.tok(None, seps)?.as_deref())?;
                let t = &mut self.data_mut(p.data, DataKind::Text)?.t;
                t.centered = centered;
                t.centered_vertically = true;
                t.left_margin = 7;
                t.top_margin = 7;
            }
            b"TEXTENTRYDATA" => {
                let mut e = self.data_mut(p.data, DataKind::Entry)?.e;
                self.strtok(Some(0), seps)?;
                e.max_text_len = scan_short(self.tok(None, seps)?.as_deref())?;
                self.strtok(None, seps)?;
                e.secret_text = scan_bool(self.tok(None, seps)?.as_deref())?;
                self.strtok(None, seps)?;
                e.numerical_only = scan_bool(self.tok(None, seps)?.as_deref())?;
                self.strtok(None, seps)?;
                e.alpha_numerical_only = scan_bool(self.tok(None, seps)?.as_deref())?;
                self.strtok(None, seps)?;
                e.ascii_only = scan_bool(self.tok(None, seps)?.as_deref())?;
                self.data_mut(p.data, DataKind::Entry)?.e = e;
            }
            b"TABCONTROLDATA" => {
                let mut t = self.data_mut(p.data, DataKind::TabControl)?.tc;
                self.strtok(Some(0), seps)?;
                scan_int_init(self.tok(None, seps)?.as_deref(), &mut t.tab_orientation)?;
                self.strtok(None, seps)?;
                scan_int_init(self.tok(None, seps)?.as_deref(), &mut t.tab_edge)?;
                self.strtok(None, seps)?;
                scan_int_init(self.tok(None, seps)?.as_deref(), &mut t.tab_width)?;
                self.strtok(None, seps)?;
                scan_int_init(self.tok(None, seps)?.as_deref(), &mut t.tab_height)?;
                self.strtok(None, seps)?;
                scan_int_init(self.tok(None, seps)?.as_deref(), &mut t.tab_count)?;
                self.strtok(None, seps)?;
                scan_int_init(self.tok(None, seps)?.as_deref(), &mut t.pane_border)?;
                let mut entry_count = 0;
                self.strtok(None, seps)?;
                scan_int_init(self.tok(None, seps)?.as_deref(), &mut entry_count)?;
                for pane in 0..entry_count.max(0) as usize {
                    let v = scan_bool(self.tok(None, seps)?.as_deref())?;
                    if pane >= NUM_TAB_PANES {
                        return Err(WndError::TabPaneOverrun);
                    }
                    t.sub_pane_disabled[pane] = v;
                }
                self.data_mut(p.data, DataKind::TabControl)?.tc = t;
            }
            b"IMAGEOFFSET" => {
                let x = c_atoi(self.tok(Some(0), b" \t\n\r")?.as_deref())?;
                let y = c_atoi(self.tok(None, b" \t\n\r")?.as_deref())?;
                p.inst.image_offset = (x, y);
            }
            b"TOOLTIP" => {
                p.inst.tooltip_text = Some("Need tooltip translation".encode_utf16().collect());
            }
            _ => {
                let mut inst = std::mem::take(&mut p.inst);
                let r = self.parse_draw_data(token, &mut inst, host);
                p.inst = inst;
                if !r? {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    /// `getDataTemplate(type)`: clears the static for the type and points to it.
    fn get_data_template(&mut self, window_type: &[u8]) -> Option<DataRef> {
        let d = &mut self.template_data;
        let kind = match window_type {
            b"VERTSLIDER" | b"HORZSLIDER" => {
                d.s = SliderData::default();
                DataKind::Slider
            }
            b"SCROLLLISTBOX" => {
                d.l = ListboxData::default();
                DataKind::Listbox
            }
            b"TABCONTROL" => {
                d.tc = TabControlData::default();
                DataKind::TabControl
            }
            b"ENTRYFIELD" => {
                d.e = EntryData::default();
                DataKind::Entry
            }
            b"STATICTEXT" => {
                d.t = TextData::default();
                DataKind::Text
            }
            b"RADIOBUTTON" => {
                d.r = RadioButtonData::default();
                DataKind::Radio
            }
            b"COMBOBOX" => {
                d.c = ComboBoxData::default();
                DataKind::ComboBox
            }
            _ => return None,
        };
        Some(DataRef { parsed: false, kind })
    }

    /// `parseData(&data, type, buffer)` (the obsolete `DATA` field).
    fn parse_data(&mut self, window_type: &[u8]) -> Result<Option<DataRef>, WndError> {
        let ws = b" \t\n\r";
        let kind = match window_type {
            b"VERTSLIDER" | b"HORZSLIDER" => {
                let min = c_atoi(self.tok(Some(0), ws)?.as_deref())?;
                let max = c_atoi(self.tok(None, ws)?.as_deref())?;
                self.parsed_data.s = SliderData { min_val: min, max_val: max };
                DataKind::Slider
            }
            b"SCROLLLISTBOX" => {
                let mut l = ListboxData { list_length: c_atoi(self.tok(Some(0), ws)?.as_deref())? as i16, ..Default::default() };
                l.auto_scroll = c_atoi(self.tok(None, ws)?.as_deref())? != 0;
                l.auto_purge = c_atoi(self.tok(None, ws)?.as_deref())? != 0;
                l.scroll_bar = c_atoi(self.tok(None, ws)?.as_deref())? != 0;
                l.multi_select = c_atoi(self.tok(None, ws)?.as_deref())? != 0;
                l.force_select = c_atoi(self.tok(None, ws)?.as_deref())? != 0;
                self.parsed_data.l = l;
                DataKind::Listbox
            }
            b"ENTRYFIELD" => {
                let mut e = EntryData { max_text_len: c_atoi(self.tok(Some(0), ws)?.as_deref())? as i16, ..Default::default() };
                self.strtok(None, ws)?;
                if let Some(c) = self.tok(None, ws)? {
                    e.secret_text = c_atoi(Some(&c))? != 0;
                }
                if let Some(c) = self.tok(None, ws)? {
                    let v = c_atoi(Some(&c))?;
                    e.numerical_only = v == 1;
                    e.alpha_numerical_only = v == 2;
                    e.ascii_only = v == 3;
                }
                self.parsed_data.e = e;
                DataKind::Entry
            }
            b"STATICTEXT" => {
                // (not cleared: the other fields keep what an earlier DATA left)
                self.parsed_data.t.centered = c_atoi(self.tok(Some(0), ws)?.as_deref())? != 0;
                self.strtok(None, ws)?;
                DataKind::Text
            }
            b"RADIOBUTTON" => {
                self.parsed_data.r.group = c_atoi(self.tok(Some(0), ws)?.as_deref())?;
                DataKind::Radio
            }
            _ => return Ok(None),
        };
        Ok(Some(DataRef { parsed: true, kind }))
    }

    /// The gadget data a window gets: the static its data points to, as it is now.
    fn snapshot(&self, data: Option<DataRef>) -> GadgetData {
        let Some(d) = data else { return GadgetData::None };
        let s = if d.parsed { &self.parsed_data } else { &self.template_data };
        match d.kind {
            DataKind::Slider => GadgetData::Slider(s.s),
            DataKind::Listbox => GadgetData::Listbox(s.l.clone()),
            DataKind::TabControl => GadgetData::TabControl(s.tc),
            DataKind::Entry => GadgetData::Entry(s.e),
            DataKind::Text => GadgetData::Text(s.t),
            DataKind::Radio => GadgetData::Radio(s.r),
            DataKind::ComboBox => GadgetData::ComboBox(s.c),
        }
    }

    /// `createWindow(type, id, status, x, y, width, height, instData, data, system, input, tooltip, draw)`, with
    /// `createGadget` and `setWindowText`.
    #[allow(clippy::too_many_arguments)]
    fn create_window(&mut self, p: &Pending, rect: (i32, i32, i32, i32), tree: &mut WindowTree, host: &mut impl WndHost) -> Result<WinId, WndError> {
        let parent = self.stack.last().copied();
        let mut inst = p.inst.clone();
        let t = p.window_type.as_slice();
        let mut parts: Vec<GadgetPart> = Vec::new();
        use GadgetPart::*;
        let style_bit = match t {
            b"USER" => gws::USER_WINDOW,
            b"TABPANE" => gws::TAB_PANE,
            b"PUSHBUTTON" => gws::PUSH_BUTTON,
            b"RADIOBUTTON" => {
                let name = &inst.decorated_name_string;
                if name.len() >= MAX_WINDOW_NAME_LEN {
                    return Err(WndError::BufferOverrun);
                }
                let file = name.split(|&c| c == b':').next().unwrap_or_default().to_vec();
                let key = host.name_to_key(&file) as i32;
                let d = p.data.ok_or(WndError::GadgetDataMismatch)?;
                if d.kind != DataKind::Radio {
                    return Err(WndError::GadgetDataMismatch);
                }
                (if d.parsed { &mut self.parsed_data } else { &mut self.template_data }).r.screen = key;
                gws::RADIO_BUTTON
            }
            b"CHECKBOX" => gws::CHECK_BOX,
            b"TABCONTROL" => gws::TAB_CONTROL,
            b"VERTSLIDER" | b"HORZSLIDER" => {
                parts = vec![SliderThumb];
                if t == b"VERTSLIDER" {
                    gws::VERT_SLIDER
                } else {
                    gws::HORZ_SLIDER
                }
            }
            b"SCROLLLISTBOX" => {
                parts = vec![ListBoxUpButton, ListBoxDownButton, ListBoxSlider, SliderThumb];
                gws::SCROLL_LISTBOX
            }
            b"COMBOBOX" => {
                parts = vec![ComboBoxDropDownButton, ComboBoxEditBox, ComboBoxListBox, ListBoxUpButton, ListBoxDownButton, ListBoxSlider, SliderThumb];
                gws::COMBO_BOX
            }
            b"ENTRYFIELD" => gws::ENTRY_FIELD,
            b"STATICTEXT" => gws::STATIC_TEXT,
            b"PROGRESSBAR" => gws::PROGRESS_BAR,
            _ => return Err(WndError::UnknownWindowType),
        };
        // (the gadgets that read their data dereference it: `DATA` on a tab control or a combo box leaves it NULL)
        if p.data.is_none() && matches!(t, b"TABCONTROL" | b"VERTSLIDER" | b"HORZSLIDER" | b"SCROLLLISTBOX" | b"COMBOBOX" | b"ENTRYFIELD" | b"STATICTEXT") {
            return Err(WndError::GadgetDataMismatch);
        }
        inst.style |= style_bit;
        // (`winCreate`: the region from the position and size, normalised)
        let (x, y, w, h) = rect;
        let (mut lx, mut hx) = (x, x.wrapping_add(w));
        let (mut ly, mut hy) = (y, y.wrapping_add(h));
        if lx > hx {
            std::mem::swap(&mut lx, &mut hx);
        }
        if ly > hy {
            std::mem::swap(&mut ly, &mut hy);
        }
        // setWindowText
        let text = if inst.text_label_string.is_empty() {
            None
        } else if inst.style & (gws::PUSH_BUTTON | gws::RADIO_BUTTON | gws::CHECK_BOX | gws::STATIC_TEXT) == 0 && inst.style & gws::ENTRY_FIELD != 0 {
            Some(translate(&inst.text_label_string))
        } else {
            Some(host.fetch_text(&inst.text_label_string))
        };
        let id = tree.windows.len();
        tree.windows.push(GameWindow {
            parent,
            children: Vec::new(),
            window_type: p.window_type.clone(),
            status: inst.status,
            size: (w, h),
            region: (lx, ly, hx, hy),
            id: inst.id,
            gadget: self.snapshot(p.data),
            parts: parts.into_iter().map(|q| (q, self.parts[PART_ORDER.iter().position(|&r| r == q).expect("part")].clone())).collect(),
            text,
            callbacks: self.funcs.clone(),
            inst,
        });
        if let Some(pid) = parent {
            tree.windows[pid].children.insert(0, id);
        }
        Ok(id)
    }

    /// `parseChildWindows(window, inFile, buffer)`: `Ok(false)` where upstream returns `FALSE` (never, but for the
    /// cases that are errors here).
    fn parse_child_windows(&mut self, window: WinId, f: &mut RamFile<'_>, tree: &mut WindowTree, host: &mut impl WndHost) -> Result<(), WndError> {
        if self.stack.len() == WIN_STACK_DEPTH - 1 {
            return Err(WndError::StackOverflow);
        }
        self.stack.push(window);
        while let Some(tok) = f.scan_string() {
            match tok.as_slice() {
                b"ENDALLCHILDREN" | b"END" => break,
                b"ENABLEDCOLOR" => self.def_colors.enabled = self.parse_default_color(f)?,
                b"DISABLEDCOLOR" => self.def_colors.disabled = self.parse_default_color(f)?,
                b"HILITECOLOR" => self.def_colors.hilite = self.parse_default_color(f)?,
                b"SELECTEDCOLOR" => self.def_colors.selected = self.parse_default_color(f)?,
                b"TEXTCOLOR" => self.def_colors.text = self.parse_default_color(f)?,
                b"WINDOW" if self.parse_window(f, tree, host)?.is_none() => return Err(WndError::ChildBlockFailed),
                _ => {}
            }
        }
        self.stack.pop();
        Ok(())
    }

    /// `parseWindow(inFile, buffer)`: the window, `None` for upstream's `NULL`.
    fn parse_window(&mut self, f: &mut RamFile<'_>, tree: &mut WindowTree, host: &mut impl WndHost) -> Result<Option<WinId>, WndError> {
        self.funcs = Default::default();
        let mut inst = WinInstanceData::new();
        let t = self.def_colors.text;
        let td = TextDrawData { color: t, border_color: t };
        inst.enabled_text = td;
        inst.disabled_text = td;
        inst.hilite_text = td;
        // WINDOWTYPE
        self.read_until_semicolon(f)?;
        self.strtok(Some(0), b" =;\n\r\t")?;
        let window_type = self.tok(None, b" =;\n\r\t")?.ok_or(WndError::NullToken)?;
        if window_type.len() >= 64 {
            return Err(WndError::BufferOverrun);
        }
        let data = self.get_data_template(&window_type);
        let mut p = Pending { window_type, data, inst };
        // SCREENRECT
        self.read_until_semicolon(f)?;
        self.strtok(Some(0), b" =;\n\r\t")?;
        let rect = self.parse_screen_rect(tree, host)?;
        let mut window = None;
        loop {
            let tok = f.scan_string().unwrap_or_default();
            if FIELDS.contains(&tok.as_slice()) || draw_data_target(&tok).is_some() {
                f.scan_string(); // eat '='
                self.read_until_semicolon(f)?;
                if !self.parse_field(&tok, &mut p, host)? {
                    return Ok(window);
                }
                continue;
            }
            match tok.as_slice() {
                b"DATA" => {
                    f.scan_string();
                    self.read_until_semicolon(f)?;
                    p.data = self.parse_data(&p.window_type)?;
                }
                b"END" => {
                    if let Some(font) = host.font_from_template(&p.inst.header_template_name) {
                        p.inst.font = Some(font);
                    }
                    if window.is_none() {
                        window = Some(self.create_window(&p, rect, tree, host)?);
                    }
                    return Ok(window);
                }
                b"CHILD" => {
                    let w = self.create_window(&p, rect, tree, host)?;
                    window = Some(w);
                    self.parse_child_windows(w, f, tree, host)?;
                }
                _ => {
                    // (at the end of the file this is where upstream loops forever)
                    if f.pos >= f.data.len() {
                        return Err(WndError::Hangs);
                    }
                    self.read_until_semicolon(f)?;
                }
            }
        }
    }

    /// `parseLayoutBlock(inFile, buffer, version, info)`: `Ok(false)` for `FALSE`.
    fn parse_layout_block(&mut self, f: &mut RamFile<'_>, info: &mut WindowLayoutInfo, host: &mut impl WndHost) -> Result<bool, WndError> {
        match f.scan_string() {
            Some(t) if t == b"STARTLAYOUTBLOCK" => {}
            _ => return Ok(false),
        }
        loop {
            let tok = match f.scan_string() {
                Some(t) => t,
                None => return Err(WndError::Hangs),
            };
            if tok == b"ENDLAYOUTBLOCK" {
                return Ok(true);
            }
            let (slot, kind) = match tok.as_slice() {
                b"LAYOUTINIT" => (0, Callback::LayoutInit),
                b"LAYOUTUPDATE" => (1, Callback::LayoutUpdate),
                b"LAYOUTSHUTDOWN" => (2, Callback::LayoutShutdown),
                _ => continue,
            };
            self.read_until_semicolon(f)?;
            // `c = strtok(buffer, " =")`, then `strtok(c, " \n\r\t")` in the parser
            // (a NULL `c` makes the parser's `strtok(c, ...)` continue the ended `strtok`: NULL, the empty name)
            let c = self.strtok(Some(0), b" =")?;
            let name = self.tok(c, b" \n\r\t")?.unwrap_or_default();
            host.name_to_key(&name);
            info.resolved[slot] = host.lexicon_has(kind, &name);
            match slot {
                0 => info.init_name_string = name,
                1 => info.update_name_string = name,
                _ => info.shutdown_name_string = name,
            }
        }
    }

    /// `GameWindowManager::winCreateFromScript(filename, info)` on the file's bytes: the windows go into `tree`.
    pub fn win_create_from_script(&mut self, data: &[u8], tree: &mut WindowTree, host: &mut impl WndHost) -> Result<ScriptResult, WndError> {
        // resetWindowStack, resetWindowDefaults
        self.stack.clear();
        self.def_colors = DefaultColors::default();
        let mut f = RamFile { data, pos: 0 };
        let mut info = WindowLayoutInfo::default();
        f.skip(b"FILE_VERSION = ".len());
        let version = f.scan_int()?;
        f.next_line();
        if version >= 2 {
            if !self.parse_layout_block(&mut f, &mut info, host)? {
                return Ok(ScriptResult { first_window: None, info: None });
            }
        } else {
            info.init_name_string = b"[None]".to_vec();
            info.update_name_string = b"[None]".to_vec();
            info.shutdown_name_string = b"[None]".to_vec();
        }
        let mut first = None;
        while let Some(tok) = f.scan_string() {
            match tok.as_slice() {
                b"END" => continue,
                b"ENABLEDCOLOR" => self.def_colors.enabled = self.parse_default_color(&mut f)?,
                b"DISABLEDCOLOR" => self.def_colors.disabled = self.parse_default_color(&mut f)?,
                b"HILITECOLOR" => self.def_colors.hilite = self.parse_default_color(&mut f)?,
                b"SELECTEDCOLOR" => self.def_colors.selected = self.parse_default_color(&mut f)?,
                b"TEXTCOLOR" => self.def_colors.text = self.parse_default_color(&mut f)?,
                b"BACKGROUNDCOLOR" => self.def_colors.background = self.parse_default_color(&mut f)?,
                b"FONT" => self.parse_default_font(&mut f)?,
                b"WINDOW" => {
                    let w = self.parse_window(&mut f, tree, host)?;
                    if first.is_none() {
                        first = w;
                    }
                    info.windows.push(w);
                }
                _ => {}
            }
        }
        Ok(ScriptResult { first_window: first, info: Some(info) })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// A host with sequential name keys, the images `Img*`, and `T:` texts.
    #[derive(Default)]
    pub(crate) struct Host {
        pub keys: BTreeMap<Vec<u8>, u32>,
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
        fn display_size(&self) -> (i32, i32) {
            (1600, 1200)
        }
    }

    pub(crate) const CONTROL_BAR_LIKE: &[u8] = b"FILE_VERSION = 2;\n\
STARTLAYOUTBLOCK\n  LAYOUTINIT = ControlBarInit;\n  LAYOUTUPDATE = [None];\n  LAYOUTSHUTDOWN = [None];\nENDLAYOUTBLOCK\n\
TEXTCOLOR = 10 20 30;\n\
WINDOW\n  WINDOWTYPE = USER;\n  SCREENRECT = UPPERLEFT: 0 448, BOTTOMRIGHT: 800 600, CREATIONRESOLUTION: 800 600;\n\
  NAME = \"ControlBar.wnd:ControlBarParent\";\n  STATUS = ENABLED+IMAGE+bogus;\n  STYLE = USER;\n  SYSTEMCALLBACK = \"ControlBarSystem\";\n\
  INPUTCALLBACK = \"[None]\";\n  FONT = NAME: \"Arial\", SIZE: 10, BOLD: 1;\n\
  ENABLEDDRAWDATA = IMAGE: ImgBase, COLOR: 255 255 255 255, BORDERCOLOR: 1 2 3 4,\n\
    IMAGE: NoImage, COLOR: 0 0 0 0, BORDERCOLOR: 0 0 0 0, IMAGE: NoImage, COLOR: 0 0 0 0, BORDERCOLOR: 0 0 0 0,\n\
    IMAGE: NoImage, COLOR: 0 0 0 0, BORDERCOLOR: 0 0 0 0, IMAGE: NoImage, COLOR: 0 0 0 0, BORDERCOLOR: 0 0 0 0,\n\
    IMAGE: NoImage, COLOR: 0 0 0 0, BORDERCOLOR: 0 0 0 0, IMAGE: NoImage, COLOR: 0 0 0 0, BORDERCOLOR: 0 0 0 0,\n\
    IMAGE: Unknown, COLOR: 0 0 0 0, BORDERCOLOR: 0 0 0 0, IMAGE: NoImage, COLOR: 0 0 0 0, BORDERCOLOR: 0 0 0 0;\n\
  CHILD\n\
  WINDOW\n    WINDOWTYPE = PUSHBUTTON;\n    SCREENRECT = UPPERLEFT: 100 500, BOTTOMRIGHT: 150 540, CREATIONRESOLUTION: 800 600;\n\
    NAME = \"ControlBar.wnd:ButtonCommand01\";\n    STATUS = ENABLED;\n    TEXT = \"CONTROLBAR:Build\";\n    TOOLTIPTEXT = \"TOOLTIP:Build\";\n\
  END\n\
  WINDOW\n    WINDOWTYPE = STATICTEXT;\n    SCREENRECT = UPPERLEFT: 10 450, BOTTOMRIGHT: 60 470, CREATIONRESOLUTION: 800 600;\n\
    NAME = \"ControlBar.wnd:MoneyDisplay\";\n    STATICTEXTDATA = CENTERED: 1;\n    TEXT = \"GUI:Money\";\n\
  END\n\
  ENDALLCHILDREN\n\
END\n";

    /// #12: the WND reader on a control-bar-like layout. Predicted from GameWindowManagerScript.cpp before the run, at a
    /// display of 1600x1200 (scale 2):
    /// - the info: init `ControlBarInit` (resolved), update and shutdown `[None]` (not resolved); one top window;
    /// - three windows, in creation order parent (0), button (1), text (2); the parent's children newest first [2, 1];
    /// - the parent: region (0, 896, 1600, 1200), size (1600, 304); status ENABLED|IMAGE = 0x88 (the unknown name
    ///   skipped); style USER (0x200); ~~id 1 (its name the first key; `ControlBarSystem` 2, `[None]` 3 come after)~~
    ///   **missed on the first run:** id 3, because the layout block's names took keys 1 (`ControlBarInit`) and 2
    ///   (`[None]`) first (`parseInit` calls `nameToKey`);
    ///   system callback `ControlBarSystem`, input none; font Arial 10 bold; enabled draw data 0: `ImgBase`, colour
    ///   0xFFFFFFFF as i32 (-1), border 0x04010203; draw data 7 (`Unknown`): no image; the text colours 10 20 30 with
    ///   alpha 255 (0xFF0A141E);
    /// - the button: region relative to the parent (200, 104)..(300, 184); style PUSHBUTTON; text `T:CONTROLBAR:Build`;
    ///   tooltip `T:TOOLTIP:Build`; callbacks none (they were reset for the window);
    /// - the static text: centred, margins 7, vertically centred.
    #[test]
    fn a_control_bar_layout_reads_as_upstream() {
        let mut ws = WindowScript::new();
        let mut tree = WindowTree::default();
        let mut h = Host::default();
        let r = ws.win_create_from_script(CONTROL_BAR_LIKE, &mut tree, &mut h).expect("parse");
        let info = r.info.expect("info");
        assert_eq!(info.init_name_string, b"ControlBarInit");
        assert_eq!(info.resolved, [true, false, false]);
        assert_eq!(info.windows, vec![Some(0)]);
        assert_eq!(tree.windows.len(), 3);
        let p = &tree.windows[0];
        assert_eq!(p.children, vec![2, 1]);
        assert_eq!((p.region, p.size, p.status, p.style(), p.id), ((0, 896, 1600, 1200), (1600, 304), 0x88, 0x200, 3));
        assert_eq!(p.callbacks[0].as_deref(), Some(&b"ControlBarSystem"[..]));
        assert_eq!(p.callbacks[1], None);
        assert_eq!(p.inst.font, Some(GameFont { name: b"Arial".to_vec(), point_size: 10, bold: true }));
        assert_eq!(p.inst.enabled_draw_data[0], WinDrawData { image: Some(b"ImgBase".to_vec()), color: -1, border_color: 0x04010203 });
        assert_eq!(p.inst.enabled_draw_data[7].image, None);
        assert_eq!(p.inst.enabled_text.color, 0xFF0A141Eu32 as i32);
        let b = &tree.windows[1];
        assert_eq!((b.region, b.style(), b.parent), ((200, 104, 300, 184), gws::PUSH_BUTTON, Some(0)));
        let s = |t: &str| t.encode_utf16().collect::<Vec<u16>>();
        assert_eq!(b.text, Some(s("T:CONTROLBAR:Build")));
        assert_eq!(b.inst.tooltip_text, Some(s("T:TOOLTIP:Build")));
        assert_eq!(b.callbacks, [None, None, None, None]);
        let t = &tree.windows[2];
        assert_eq!(t.gadget, GadgetData::Text(TextData { centered: true, centered_vertically: true, left_margin: 7, top_margin: 7 }));
        assert_eq!(tree.find_by_name(b"ControlBar.wnd:MoneyDisplay"), Some(2));
        assert_eq!(tree.screen_position(1), (200, 1000));
    }

    /// #12: the reader's quirks. Predicted:
    /// - a window not closed by `END` hangs upstream: `Hangs`;
    /// - a field cut by the end of the file: `ReadPastEnd`;
    /// - a `TEXT` of 128 bytes fails the window: the top-level list holds `None`, no window is created, and the next
    ///   window still reads;
    /// - a `NAME` field with no quote takes the quote an earlier, longer value left in the static buffer: the name is
    ///   the text after that stale quote (`stale`).
    #[test]
    fn the_readers_quirks_are_kept() {
        let head = b"FILE_VERSION = 1;\n".to_vec();
        let rect = b"WINDOWTYPE = USER; SCREENRECT = UPPERLEFT: 0 0, BOTTOMRIGHT: 10 10, CREATIONRESOLUTION: 800 600;";
        let mut h = Host::default();
        let mut run = |body: &[u8]| {
            let mut ws = WindowScript::new();
            let mut tree = WindowTree::default();
            let r = ws.win_create_from_script(&[head.as_slice(), body].concat(), &mut tree, &mut h);
            (r, tree)
        };
        assert_eq!(run(&[b"WINDOW ".as_slice(), rect].concat()).0, Err(WndError::Hangs));
        assert_eq!(run(&[b"WINDOW ".as_slice(), rect, b" NAME = \"x\""].concat()).0, Err(WndError::ReadPastEnd));
        let long = vec![b'L'; 128];
        let body = [b"WINDOW ".as_slice(), rect, b" TEXT = \"", &long, b"\"; END WINDOW ", rect, b" END"].concat();
        let (r, tree) = run(&body);
        let r = r.expect("parse");
        assert_eq!(r.info.unwrap().windows, vec![None, Some(0)]);
        assert_eq!(tree.windows.len(), 1);
        // `1 2 "stale"` stays in the buffer past the NULs strtok wrote; `NAME = n` overwrites only `n\0`
        let body = [b"WINDOW ".as_slice(), rect, b" IMAGEOFFSET = 1 2 \"stale\"; NAME = n; END"].concat();
        let (r, tree) = run(&body);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(tree.windows[0].inst.decorated_name_string, b"stale");
        assert_eq!(tree.windows[0].inst.image_offset, (1, 2));
    }
}
