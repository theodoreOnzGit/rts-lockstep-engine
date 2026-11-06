// SPDX-License-Identifier: GPL-3.0-only
//! The game's own in-game UI from its files (#12): the control bar of `Window\ControlBar.wnd` (read by
//! `rts_lockstep_engine::client::window_script`), with the mapped images of `Data\INI\MappedImages`
//! (`client::image`), the side's control bar scheme (`client::control_bar_scheme`), the selected object's command
//! buttons and build queue (`client::control_bar_command`), the money (`InGameUI::update`'s `GUI:ControlBarMoneyDisplay`)
//! and the power.
//!
//! Plain Rust, no Bevy: [`load_game_ui`] reads the files, [`GameUi::frame`] says what to draw this frame as quads in
//! the layout's 800x600 space ([`UiQuad`]); `app` draws them with Bevy UI nodes. Upstream's own drawing
//! (`W3DGameWinDefaultDraw`, the gadget draw functions, `W3DPowerDraw`) is not translated: a window with
//! `WIN_STATUS_IMAGE` is drawn as its state's first image stretched over its region, any other as its first colour,
//! text centred; the power bar as production against consumption. That part is new.
//!
//! The client still never changes the simulation: everything here reads the `World`.

use std::collections::BTreeMap;

use rts_lockstep_engine::client::control_bar_command::{populate_build_queue, populate_command, CommandSlot, ControlBarStateHost, QueueSlot};
use rts_lockstep_engine::client::control_bar_scheme::{ControlBarSchemeManager, SchemeDraw};
use rts_lockstep_engine::client::image::ImageCollection;
use rts_lockstep_engine::client::window_script::{win_status, GameWindow, WinId, WindowScript, WindowTree, WndHost, WIN_COLOR_UNDEFINED};
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
    pub tree: WindowTree,
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
    let mut tree = WindowTree::default();
    let mut host = Host { keys: BTreeMap::new(), images: &images, text };
    if let Err(e) = WindowScript::new().win_create_from_script(&wnd, &mut tree, &mut host) {
        return UiBuild::Missing(format!("ControlBar.wnd did not read: {e:?}"));
    }
    schemes.set_control_bar_scheme_by_side(side, false, &mut tree, LAYOUT, &images);
    let find = |n: String| tree.find_by_name(n.as_bytes());
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
        images,
        tree,
        schemes,
    };
    UiBuild::Ready(Box::new(ui))
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
    /// The bar's state for `selected`: `populateCommand` and the build queue when an object is selected, else nothing.
    pub fn bar_state(selected: Option<ObjectId>, h: &mut impl ControlBarStateHost, queue: Vec<(bool, u32, Option<Vec<u8>>, f32)>, money: u32, power: (i32, i32)) -> BarState {
        let commands = selected.map(|o| populate_command(o, h).to_vec()).unwrap_or_default();
        let (queue, queue_clock) = populate_build_queue(&queue);
        BarState { selected, commands, queue, queue_clock, money, power }
    }

    /// Whether a window is drawn: it and every ancestor not hidden, with the context parents shown as
    /// `switchToContext` would (the command window with a selection; the queue window with a queue).
    fn shown(&self, w: WinId, s: &BarState) -> bool {
        let mut cur = Some(w);
        while let Some(i) = cur {
            let win = &self.tree.windows[i];
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
            } else {
                win.is_hidden()
            };
            if hidden {
                return false;
            }
            cur = win.parent;
        }
        true
    }

    /// The quads of a frame: the scheme's background layers, the windows (parents before children; a parent's children
    /// oldest first, so the newest is on top, as the window list's head is), then the scheme's foreground.
    pub fn frame(&self, s: &BarState) -> Vec<UiQuad> {
        let mut out = Vec::new();
        let scheme = self.schemes.current_scheme();
        let draws: Vec<SchemeDraw> = scheme.map(|c| c.draw_list(self.schemes.multiplier, Default::default(), &self.images)).unwrap_or_default();
        let quad = |d: &SchemeDraw| UiQuad { rect: [d.start.0, d.start.1, d.end.0, d.end.1], image: Some(d.image.clone()), color: None, text: None, command: None, dim: false, clock: None };
        out.extend(draws.iter().filter(|d| d.layer >= 3).map(quad));
        let roots: Vec<WinId> = (0..self.tree.windows.len()).filter(|&i| self.tree.windows[i].parent.is_none()).collect();
        for r in roots {
            self.windows_into(r, s, &mut out);
        }
        out.extend(draws.iter().filter(|d| d.layer < 3).map(quad));
        out
    }

    fn windows_into(&self, w: WinId, s: &BarState, out: &mut Vec<UiQuad>) {
        if !self.shown(w, s) {
            return;
        }
        let win: &GameWindow = &self.tree.windows[w];
        let (x, y) = self.tree.screen_position(w);
        let rect = [x as f32, y as f32, (x + win.region.2 - win.region.0) as f32, (y + win.region.3 - win.region.1) as f32];
        let mut q = UiQuad { rect, image: None, color: None, text: None, command: None, dim: false, clock: None };
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
        out.push(q);
        if Some(w) == self.power {
            // the power bar (new drawing): consumption against production
            let (p, c) = s.power;
            let frac = if p <= 0 { 0.0 } else { (c as f32 / p as f32).min(1.0) };
            let colr = if c > p { [220, 40, 30, 255] } else { [40, 200, 60, 255] };
            out.push(UiQuad { rect: [rect[0], rect[1], rect[0] + (rect[2] - rect[0]) * frac.max(0.05), rect[3]], image: None, color: Some(colr), text: None, command: None, dim: false, clock: None });
        }
        for &c in win.children.iter().rev() {
            self.windows_into(c, s, out);
        }
    }
}

/// A synthetic control bar (no EA file): the mapped images, `ControlBar.wnd` and a scheme, as test files. `ui.tga` is a
/// 64x64 Targa in four colours.
pub fn synthetic_ui_files() -> Vec<(&'static [u8], Vec<u8>)> {
    let images = b"MappedImage ImgBar\n  Texture = ui.tga\n  TextureWidth = 64\n  TextureHeight = 64\n  Coords = Left:0 Top:0 Right:32 Bottom:32\n  Status = NONE\nEnd\n\
MappedImage ImgButton\n  Texture = ui.tga\n  TextureWidth = 64\n  TextureHeight = 64\n  Coords = Left:32 Top:0 Right:64 Bottom:32\n  Status = NONE\nEnd\n\
MappedImage ImgBack\n  Texture = ui.tga\n  TextureWidth = 64\n  TextureHeight = 64\n  Coords = Left:0 Top:32 Right:64 Bottom:64\n  Status = NONE\nEnd\n"
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
    wnd += &format!("WINDOW\n WINDOWTYPE = USER;\n {}\n NAME = \"ControlBar.wnd:ControlBarParent\";\n STATUS = ENABLED+IMAGE;\n STYLE = USER;\n ENABLEDDRAWDATA = {};\n CHILD\n", rect(0, 450, 800, 600), dd("ImgBar"));
    wnd += &format!("WINDOW\n WINDOWTYPE = STATICTEXT;\n {}\n NAME = \"ControlBar.wnd:MoneyDisplay\";\n STATUS = ENABLED;\n STATICTEXTDATA = CENTERED: 1;\nEND\n", rect(20, 460, 120, 480));
    wnd += &format!("WINDOW\n WINDOWTYPE = USER;\n {}\n NAME = \"ControlBar.wnd:PowerWindow\";\n STATUS = ENABLED;\n ENABLEDDRAWDATA = {};\nEND\n", rect(20, 485, 220, 495), dd("NoImage"));
    wnd += &format!("WINDOW\n WINDOWTYPE = USER;\n {}\n NAME = \"ControlBar.wnd:CommandWindow\";\n STATUS = ENABLED+HIDDEN;\n CHILD\n", rect(250, 470, 700, 590));
    for i in 0..3 {
        let x = 260 + i * 60;
        wnd += &format!("WINDOW\n WINDOWTYPE = PUSHBUTTON;\n {}\n NAME = \"ControlBar.wnd:ButtonCommand{:02}\";\n STATUS = ENABLED+IMAGE;\n ENABLEDDRAWDATA = {};\nEND\n", rect(x, 480, x + 50, 530), i + 1, dd("ImgButton"));
    }
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
    /// - it loads: 3 mapped images, ~~8 windows~~ **missed on the first run:** 7 windows (the parent, the money, the power,
    ///   the command window and its 3 buttons: a miscount in the prediction, not in the reader); the `Default` scheme is chosen for side `America` (none of that side);
    /// - nothing selected: the scheme's background (`ImgBack`, from y 440), the parent (`ImgBar`), the money
    ///   (`$ <money>`: no string file, so `GUI:ControlBarMoneyDisplay` is missing) and the power bar; no command button
    ///   (the command window is hidden without a selection);
    /// - the money display placed by the scheme's corners: (30, 455)..(130, 475);
    /// - the dozer selected: the command window and its first button, `Command_BuildDepot` in slot 0, clickable;
    ///   pressing it through `Controls::press` starts the Depot's placement;
    /// - the CRCs: building the bar's state every frame for 60 frames leaves the logic's CRCs as a plain run's.
    #[test]
    fn the_control_bar_draws_from_its_files() {
        let mut d = MemoryDevice::new();
        for (p, b) in synthetic_ui_files() {
            d.insert(p, b);
        }
        let mut fs = FileSystem::new(d);
        fs.init(None);
        let UiBuild::Ready(ui) = load_game_ui(&fs, &TextTable::default(), b"America") else { panic!("the bar did not load") };
        assert_eq!((ui.images.len(), ui.tree.windows.len()), (3, 7));
        assert_eq!(ui.schemes.current_scheme().map(|s| s.name.clone()), Some(b"default".to_vec()));
        let money = ui.money.expect("the money window");
        assert_eq!(ui.tree.screen_position(money), (30, 455));
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
        // select the dozer
        let dp = c.client.object_position(o.dozer);
        assert!(c.left_release(ICoord2D { x: 5, y: 5 }, ICoord2D { x: 5, y: 5 }, &[o.dozer], Some(o.dozer), dp));
        let sel = c.ui.first_selected_drawable();
        let st = GameUi::bar_state(sel, &mut c.client, Vec::new(), m, p);
        let slot = st.commands[0].clone().expect("the build button");
        assert_eq!(slot.button, o.build_depot);
        let q = ui.frame(&st);
        assert_eq!(q.iter().filter(|x| x.command.is_some()).count(), 1);
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
            let crc = |c: &mut Controls| c.world().logic_crc(rts_lockstep_engine::logic::world::game_logic_update::CRC_RECALC, None).expect("a CRC");
            assert_eq!(crc(&mut asked), crc(&mut plain), "frame {f}");
        }
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
    /// *Prediction:* ~~the bar waits at least once (the page's ranges)~~, then loads with 7 windows. **Missed on the first
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
        println!("ui install: waited {waits} times; {} windows, {} images", ui.tree.windows.len(), ui.images.len());
        assert_eq!(ui.tree.windows.len(), 7);
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
        println!("the owner's control bar: {} windows, {} images, scheme {:?}", ui.tree.windows.len(), ui.images.len(), ui.schemes.current_scheme().map(|s| String::from_utf8_lossy(&s.name).into_owned()));
        assert!(ui.command_windows.iter().all(Option::is_some));
        assert!(ui.queue_windows.iter().all(Option::is_some));
        assert!(ui.money.is_some() && ui.power.is_some());
    }
}
