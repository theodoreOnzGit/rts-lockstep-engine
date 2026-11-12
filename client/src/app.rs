// SPDX-License-Identifier: GPL-3.0-only
//! The Bevy application, milestone C1: the running game logic drawn without art (`docs/phase2-client.md`).
//!
//! - **The logic** steps in `FixedUpdate` at 30 Hz (`LOGICFRAMES_PER_SECOND`). After each frame the view model takes a
//!   [`Snapshot`]. The logic is the only thing that changes the game (see the crate's docs).
//! - **Drawing** (`Update`, every rendered frame) interpolates each object between the last two snapshots, by how far
//!   the fixed clock has run past its last step. One entity per logic object, spawned when it appears and despawned when
//!   it goes. The shapes are its collision geometry, in its player's colour. A health bar is drawn above each object with
//!   a body.
//! - **The camera** ~~is an RTS camera: a focus point on the ground, a distance and a heading. Keys W/A/S/D or the
//!   arrows pan, Q/E turn, the wheel or +/- zoom, and a right- or middle-drag pans. On a touch screen one finger pans and
//!   two pinch.~~ **Corrected 2026-11-10 (#119):** is the game's: the translated `LookAtTranslator` on the translated
//!   tactical view ([`crate::camera`]). The arrows scroll, a right drag scrolls (the game's RMB scroll: the farther from
//!   where it went down, the faster), the middle button turns (a click resets), the wheel or +/- zoom and Q/E turn. On a
//!   touch screen one finger is the right button and two pinch. On-screen buttons zoom in, zoom out, reset the view and
//!   pause, so a phone needs no keyboard.
//!
//! - **The player's input** (C2d): a left click selects or orders, a left drag selects the player's units in the box; the
//!   pointer's hint (what a click would order) is in the status line; selected units have a ring; ~~the first selected
//!   unit's command buttons are along the bottom~~ **corrected 2026-11-05 (#12):** the first selected unit's command
//!   buttons are the game's control bar's when its files are loaded (below), else a debug panel along the bottom (also
//!   shown, above the bar, on a narrow screen); a construct button puts the building at the cursor until a click
//!   places it. Escape cancels a placement or a pending command (**corrected 2026-11-10 (#87):** and otherwise
//!   opens the game's quit menu, the shell's, when its files are loaded). Everything goes through [`crate::input::Controls`], the
//!   translated client: the window only says what is under the pointer. Objects are picked by their projected position
//!   (within 18 pixels), the ground by the camera ray meeting ~~the ground plane (the demo's ground is flat)~~
//!   **corrected 2026-11-05 (#11):** the logic's ground height (`TerrainLogic::ground_height`), the plane at 0 only
//!   when the ray meets no ground.
//!
//! - **The terrain** (#11): built from the map when a game starts (`crate::terrain`): the game's tile textures and
//!   blend layers when the art has them, else the height map shaded, else a flat plane; while a terrain texture is
//!   being fetched (the web) it is built again on a later frame.
//!
//! - **Models** (#25): after each logic frame the frame's client events go to the drawables (`crate::drawables`); every
//!   rendered frame the draw modules choose and time each object's model and animation, and an object whose model the
//!   art has is drawn as it (its parts at their bones, skins deformed), else as its shape. Natively with `--zh` the art
//!   is the player's folder; ~~the demo and the web page have none yet (#24), so they draw shapes~~ **corrected
//!   2026-11-05 (#24):** on the web it is the page's imported files, the art archives read by range as their members are
//!   needed (`crate::import`); the demo has none, so it draws shapes.
//!
//! - **The control bar** (#12): read from the art's files when a game starts (`crate::ui`, tested there) and drawn as
//!   Bevy UI nodes along the bottom, scaled to the screen's width (a phone's too); its command buttons press through
//!   `Controls::press`. Without the files the debug panel stays. **(#136)** It hides and shows: the CommandMap's
//!   `TOGGLE_CONTROL_BAR` key or the zoom column's `B` button make `MSG_META_TOGGLE_CONTROL_BAR`, whose case
//!   (`ToggleControlBar`) the bar carries out; its own `ButtonLarge` lowers it to a strip and raises it
//!   (`toggleControlBarStage`). The window code's click sounds play through the audio, and the shell's and the bar's
//!   windows set the one mouse's tooltip, drawn over the shell too.
//!
//! - **The shell** (#78): the game's menus (`crate::shell`, tested there), read once the owner's files are there and
//!   shown over the install's game (paused under it), drawn from the shell's window manager's draw list with the
//!   control bar's drawing, the 800x600 layout scaled whole over the 3D view and clear of the zoom column (a phone's
//!   too); a game's start hides it (`hideShell`). Without its files the plain panels (saves, campaigns) are the way in.
//!
//! GUI glue: the drawing systems need a window and a GPU, so no test reaches them. What they draw is the view model's,
//! which `view`'s tests check against the `World`; what the input does is `input`'s, which its tests drive.

use std::collections::HashMap;

use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::picking::events::{PointerLeave, PointerMove, PointerPress, PointerRelease};
use bevy::picking::pointer::PointerButton;
use bevy::ui_widgets::{observe, Activate, Button};

use rts_lockstep_engine::common::base_type::{Coord3D, ICoord2D};
use rts_lockstep_engine::common::game_type::ObjectId;

use rts_lockstep_engine::logic::world::game_logic::World as GameWorld;

use crate::demo::demo_world;
use crate::drawables::{Art, WorldDrawables};
use rts_lockstep_engine::common::file_system::LocalDevice;
use crate::input::Controls;
use crate::terrain::{spawn_terrain, terrain_look, Ground, TerrainBuild};
use crate::view::{interpolate, snapshot, to_bevy, Shape, Snapshot};

/// The logic's frame rate (`LOGICFRAMES_PER_SECOND`).
const LOGIC_HZ: f64 = 30.0;

/// The device the game's art is read from: the player's folder natively; in the browser the page's files, ~~(it imports no
/// art archives yet, #24, so there the art is absent and every object is its shape)~~ **corrected 2026-11-05 (#24):**
/// read by range from the player's files as they are needed (`crate::import`'s ranged reads).
#[cfg(not(target_arch = "wasm32"))]
type ArtDevice = rts_lockstep_engine::common::file_system::NativeDevice;
#[cfg(target_arch = "wasm32")]
type ArtDevice = rts_lockstep_engine::common::file_system::RangedDevice;

/// The game: the logic's `World` and the last two snapshots. A non-send resource: the `World` is the logic's, used only
/// from the main thread, as upstream's single-threaded logic is.
struct Game {
    /// The translated client, which owns the `World` (`controls.client.world`).
    controls: Controls,
    prev: Snapshot,
    cur: Snapshot,
    paused: bool,
    /// The logic's error, if a frame failed; the game stops there and the HUD shows it.
    error: Option<String>,
    /// What is being played: the demo, or the player's game (the load's report), or why the player's files did not load.
    playing: String,
    /// The objects' drawables and their models (#25): fed the logic's client events after each frame, drawn every
    /// rendered frame.
    models: WorldDrawables<ArtDevice>,
    /// Each model's textures as Bevy images, by model name (made once).
    images: HashMap<Vec<u8>, Vec<Vec<Option<Handle<Image>>>>>,
    /// The client's clock for the draw modules (`WW3D::Sync`), milliseconds.
    models_clock: u32,
    /// The game's control bar (#12), once read from the art's files.
    ui: Option<Box<crate::ui::GameUi>>,
    /// Whether the control bar is still to be read (`false` once read or found missing), and why it is not drawn.
    ui_pending: bool,
    ui_status: String,
    /// The UI's textures by file name (`None`: missing), with their sizes.
    ui_textures: UiTextures,
    /// (#35) ~~When a command button's popup shows, the popups' lines by command slot, the selection they were made for,
    /// and~~ whether the last pointer to press a button was a touch. **Corrected 2026-11-11, #138:** when the popup shows
    /// is the bar's (`GameUi::build_tooltip`); here, the popups' lines by command button (`GadgetButtonGetData`, the
    /// World's list), and where the pointer over the bar last was (the window's logical pixels; whether a touch), for
    /// the frame's mouse event (upstream's `MSG_RAW_MOUSE_POSITION`, every frame).
    tooltip_lines: Vec<(usize, Vec<String>)>,
    bar_pointer: Option<(Vec2, bool)>,
    touch_pressed: bool,
    /// (#34) The command buttons of the bar's slots this frame (`populateCommand`'s), for a press the manager selects.
    bar_buttons: Vec<Option<usize>>,
    /// The FX lists and particle systems still to be read from the art (#43, #44).
    fx_pending: bool,
    /// The audio's tables still to be read from the art (#13).
    audio_pending: bool,
    /// The audio (#13): the translated manager and the device, once its tables are read.
    audio: Option<Box<GameSound>>,
    /// (#136) The window code's sounds to play (`GUIClick`, `GUIClickDisabled`, the shell's buttons' and transitions'):
    /// the hot keys', the bar's and the shell's windows', played by `update_audio`.
    gui_sounds: Vec<Vec<u8>>,
    /// (#35) The radar's image, the logic frame and size it was drawn for, and the "RadarEvent" sounds it asked for
    /// (played with the next frame's sounds).
    radar_image: Option<Handle<Image>>,
    radar_drawn: Option<(u32, ICoord2D)>,
    radar_event_sounds: u32,
    /// (#35) Whether the water INI has been read for the radar (once the art is there).
    radar_water_pending: bool,
    /// (#4) The World as the install made it, before the game started: a save of this install's games is loaded into a
    /// copy of it (`crate::import::Started::base`); `None` for the demo.
    base: Option<Box<GameWorld>>,
    /// (#4) The client's state a save holds besides the World and the radar (the camera's view, the in-game UI's timers).
    save_state: crate::saves::ClientState,
    /// (#59) A loaded game's drawables' ambient sounds are still to be given to the audio (once it is up).
    ambient_restore_pending: bool,
    /// (#78) This game's number in the session (each new game, loaded save or mission is a new one): the shell is hidden
    /// when the game under it is no longer the one it was shown over.
    serial: u64,
    /// (#108) `setGamePaused`'s members, the mouse it sets and the drawables' clock (`syncTime`, held while paused).
    pause: rts_lockstep_engine::client::game_pause::ClientPause,
    /// (#108) Whether the World's game was already seen cleared (`clearGameData`), so `closeWindows` runs once.
    cleared_seen: bool,
    /// (#117) A script's `closeWindows` (`doQuickVictory`) still to be made on the shell's windows.
    close_windows_pending: bool,
    /// (#144) `TheDisplay`'s letterbox (`rts_lockstep_engine::client::letterbox`): a script's `EnableLetterBox`, drawn by
    /// [`letterbox_bars`].
    letterbox: rts_lockstep_engine::client::letterbox::LetterBox,
    /// (#144) The popup's text as the last `populateBuildTooltipLayout(NULL, window)` filled it (the money, power and
    /// general's experience windows), and the bar's fill count it was taken at (`GameUi::popup_fills`).
    window_popup_lines: Option<Vec<String>>,
    popup_fills_seen: u32,
    /// (#117) `TheInGameUI->isClientQuiet()` (the shell's quit menu flag), for the audio's radar edge sounds.
    client_quiet: bool,
    /// (#117) `Mouse.ini` still to be read for the mouse (`GameClient::init`'s `parseIni`); until it is, the window's
    /// cursor is left alone.
    mouse_pending: bool,
    /// (#124) The InGameUI's radius cursor (its templates from `InGameUI.ini`, the current decal), and whether
    /// `InGameUI.ini` is still to be read for it.
    radius: rts_lockstep_engine::client::radius_cursor::InGameUiRadiusCursor,
    in_game_ui_pending: bool,
    /// (#130) The client's halves of the logic's own radius decals (delivery decals, the shroud's grid, the gunship's
    /// reticles), fed the frame's `GridDecal` events. A new game or a loaded save starts with none (`xferRadiusDecal`'s
    /// load clears every decal).
    logic_decals: rts_lockstep_engine::client::radius_cursor::LogicRadiusDecals,
    // (#117's `mouse_mode_cursor` is ~~here~~ **Corrected 2026-11-11 (#122):** the InGameUI's, `controls.ui`'s, with the
    // mouse mode it depends on)
}

/// (#78) The next [`Game::serial`].
static GAME_SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The audio's output: the browser's Web Audio, or (natively, no backend chosen yet) the silent one.
#[cfg(target_arch = "wasm32")]
type SoundOutput = crate::audio::web::WebOutput;
#[cfg(not(target_arch = "wasm32"))]
type SoundOutput = crate::audio::NullOutput;

/// The audio of a game (#13).
struct GameSound {
    audio: rts_lockstep_engine::client::client_audio::ClientAudio,
    device: crate::audio::ClientDevice<SoundOutput>,
}

impl Game {
    fn new(world: GameWorld, player: i32, playing: String, art: Option<Art<ArtDevice>>) -> Self {
        Game::from_controls(Controls::new(world, player), player, playing, art)
    }

    /// (#4) A game over controls already made (a loaded save's).
    fn from_controls(controls: Controls, player: i32, playing: String, art: Option<Art<ArtDevice>>) -> Self {
        Game::from_loaded(controls, player, playing, WorldDrawables::new(art))
    }

    /// (#59) A game over controls and drawables already made (a loaded save's: its drawables are the save's).
    fn from_loaded(controls: Controls, player: i32, playing: String, models: WorldDrawables<ArtDevice>) -> Self {
        let mut controls = controls;
        let cur = snapshot(controls.world());
        let cleared_seen = controls.client.world.game_cleared.is_some();
        Game {
            controls,
            prev: cur.clone(),
            cur,
            paused: false,
            error: None,
            playing,
            models: {
                // the shroud's local player (`ClientDrawables::local_player_index`, GitHub #45)
                let mut m = models;
                m.drawables.local_player_index = Some(player);
                m
            },
            images: HashMap::new(),
            models_clock: 0,
            ui: None,
            ui_pending: true,
            ui_status: "no game files: debug panel".to_string(),
            ui_textures: HashMap::new(),
            tooltip_lines: Vec::new(),
            bar_pointer: None,
            touch_pressed: false,
            bar_buttons: Vec::new(),
            fx_pending: true,
            audio_pending: true,
            audio: None,
            gui_sounds: Vec::new(),
            radar_image: None,
            radar_drawn: None,
            radar_event_sounds: 0,
            radar_water_pending: true,
            base: None,
            save_state: Default::default(),
            ambient_restore_pending: false,
            serial: GAME_SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            pause: Default::default(),
            cleared_seen,
            close_windows_pending: false,
            letterbox: Default::default(),
            window_popup_lines: None,
            popup_fills_seen: 0,
            client_quiet: false,
            mouse_pending: true,
            radius: Default::default(),
            in_game_ui_pending: true,
            logic_decals: Default::default(),
        }
    }

    /// (#117) `self` replaced by the game `new`, with `TheMouse` kept (upstream's mouse outlives the games), and the
    /// mouse made visible as `GameLogic::startNewGame`'s load screen leaves it (`TheMouse->setVisibility(TRUE)`,
    /// GameLogic.cpp:2199): every game the client starts is a new game, a loaded save or a mission. GUI glue.
    fn replace(&mut self, mut new: Game) {
        new.pause.mouse = std::mem::take(&mut self.pause.mouse);
        new.mouse_pending = self.mouse_pending;
        // (#124) `TheInGameUI`'s templates outlive the games too (its decal does not: `InGameUI::reset` clears it)
        new.radius.radius_cursors = std::mem::take(&mut self.radius.radius_cursors);
        new.radius.settings = std::mem::take(&mut self.radius.settings);
        new.in_game_ui_pending = self.in_game_ui_pending;
        new.pause.mouse.set_visibility(true);
        *self = new;
    }

    /// (#108) `TheGameLogic->setGamePaused(paused)` (the quit menu's, `pauseMusic` at its default `TRUE`): the pause
    /// (`Game::paused`, the logic's frames not run) with its side effects on the control bar's InGameUI, the mouse and
    /// the audio (`ClientPause::set_game_paused`, tested in the logic crate). GUI glue: the `Game` needs the app's
    /// resources, so no test makes one.
    fn set_game_paused(&mut self, paused: bool) {
        let audio = self.audio.as_deref_mut().map(|s| (&mut s.audio.manager, &mut s.device));
        self.paused = self.pause.set_game_paused(self.paused, paused, true, &mut self.controls.ui, audio);
    }
}

impl Game {
    /// The demo game (`crate::demo`). #35: its player has no radar building, so the demo forces the radar on, as a map
    /// script's `RADAR_FORCE_ENABLE` would (the demo's own choice: the radar is to be seen without the game's files).
    fn demo(playing: String) -> Self {
        let (w, _) = demo_world();
        let mut g = Game::new(w, crate::demo::DEMO_LOCAL_PLAYER, playing, None);
        g.controls.radar.radar.force_on(true);
        g
    }
}

/// The drawn entity of each logic object, by `ObjectID`, and the model it was drawn with (empty: its shape).
#[derive(Resource, Default)]
struct Drawn(HashMap<i32, (Entity, Vec<u8>)>);

/// A drawn piece of an object's model: the object, the sub-object, and its mesh (rewritten every frame for a skin).
#[derive(Component)]
struct ModelPart {
    object: ObjectId,
    sub: usize,
    skin: bool,
    mesh: Handle<Mesh>,
}

/// The camera (#119): the translated `LookAtTranslator` on the translated tactical view
/// ([`crate::camera::Camera`], tested there), with the glue's own pointer state. ~~The RTS camera's state: what it looks
/// at, from how far, from which heading.~~ **Corrected 2026-11-10 (#119):** the camera is the game's.
#[derive(Resource)]
struct Rig {
    cam: crate::camera::Camera,
    /// Wheel lines not yet a whole notch.
    wheel: f32,
    /// The finger that drags as the right button.
    touch: Option<u64>,
    /// A two-finger pinch's distance at its last zoom step.
    pinch: Option<f32>,
    /// The pointer's last pixel given as `MSG_RAW_MOUSE_POSITION`.
    last_pos: Option<ICoord2D>,
    /// (#131) The keyboard camera's fallback keys and turn buttons held (rotate left, right, zoom in, out), for their
    /// meta messages' begin and end.
    camera_held: [bool; 4],
}

/// (#131) An on-screen turn button (0 left, 1 right): held, it turns the view as the camera's rotate meta keys do.
#[derive(Component)]
struct TurnButton(usize);

impl Rig {
    /// GUI glue (the app's start of a game; [`crate::camera::Camera::new`] is tested): the camera over the map's centre.
    fn new(world: &GameWorld, screen: (i32, i32)) -> Self {
        let (centre, size) = view_frame(world);
        Rig { cam: crate::camera::Camera::new(world, Coord3D { x: centre.x, y: centre.y, z: 0.0 }, size, screen), wheel: 0.0, touch: None, pinch: None, last_pos: None, camera_held: [false; 4] }
    }
}

#[derive(Component)]
struct Hud;

/// Runs the client: on the player's Zero Hour folder when `zh` names one (native), else on the demo game until the page
/// hands over the player's files ([`crate::import`]). (C2d ran the demo; C1 the gate game, whose units cannot be selected.)
pub fn run(zh: Option<std::path::PathBuf>) {
    let game = match zh {
        #[cfg(not(target_arch = "wasm32"))]
        Some(folder) => match crate::import::start_from_folder(&folder, None) {
            Ok(s) => {
                // the art: the same folder through the game's file search (its archives mounted), as the model viewer reads it
                let mut fs = rts_lockstep_engine::common::file_system::FileSystem::new(ArtDevice::new(&folder));
                fs.init(None);
                let mut g = Game::new(s.world, s.player, s.report, Some(Art::new(fs)));
                g.base = Some(Box::new(s.base));
                g
            }
            Err(e) => Game::demo(format!("demo ({e})")),
        },
        _ => Game::demo("demo game: load your Zero Hour folder to play".to_string()),
    };
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "rts-lockstep-engine".into(),
                // the web build draws into the page's `#rts` canvas and fills its parent
                canvas: Some("#rts".into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: true,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(Time::<Fixed>::from_hz(LOGIC_HZ))
        .insert_non_send(game)
        .insert_non_send(Saves::new())
        .init_resource::<SaveAction>()
        .insert_resource(KeyboardScroll(1.0))
        .insert_non_send(Campaigns::default())
        .init_resource::<CampaignAction>()
        .insert_resource(TerrainState { pending: true, look: "none" })
        .init_resource::<Drawn>()
        .init_resource::<Pointer>()
        .init_resource::<UiBar>()
        .init_resource::<UiKeyboard>()
        .init_resource::<RadarBox>()
        .init_resource::<ParticleDraw>()
        .add_systems(Startup, (setup_scene, setup_ui))
        .add_systems(FixedUpdate, step_logic)
        .insert_non_send(ShellState::default())
        .add_systems(Update, (poll_import, apply_save_action, saves_panel, pump_art, apply_campaign_action, campaign_flow, campaign_panel, update_terrain, load_shell, shell_ui).chain())
        .add_systems(
            Update,
            (load_fx, update_audio, sync_objects, draw_particles, health_bars, keyboard_ui, camera_input, apply_camera, pointer_input, selection_rings, drag_box, load_game_ui, game_ui, radar_view, tooltip_panel, lift_zoom, control_bar, update_hud, show_cursor).chain(),
        )
        .add_systems(Update, (radius_decal, logic_decals, world_tooltip).chain().after(show_cursor))
        .add_systems(Update, letterbox_bars.after(world_tooltip))
        .run();
}

/// The terrain (#11): built when a game starts, and again on a later frame while its textures are being fetched.
#[derive(Resource)]
struct TerrainState {
    /// The terrain is still to be built for the current game.
    pending: bool,
    /// What it was drawn as (`TerrainLook::name`), for the HUD.
    look: &'static str,
}

/// The view's centre and size: the map's extent, or a 400-unit square without a map.
fn view_frame(world: &GameWorld) -> (Vec2, f32) {
    match world.terrain.extent() {
        Ok(e) => {
            let (w, h) = (e.hi.x - e.lo.x, e.hi.y - e.lo.y);
            (Vec2::new(e.lo.x + w / 2.0, e.lo.y + h / 2.0), w.max(h))
        }
        Err(_) => (Vec2::new(200.0, 200.0), 400.0),
    }
}

/// GUI glue (needs the app): (re)builds the terrain while [`TerrainState::pending`]: the look from the world and the
/// art's files ([`terrain_look`], tested there), spawned with [`spawn_terrain`] in place of the last game's. While a
/// texture is being fetched (the web's ranged reads) it waits for a later frame.
#[allow(clippy::too_many_arguments)]
fn update_terrain(
    mut commands: Commands,
    mut game: NonSendMut<Game>,
    mut state: ResMut<TerrainState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    grounds: Query<Entity, With<Ground>>,
) {
    if !state.pending {
        return;
    }
    let world = &game.controls.client.world;
    let look = match terrain_look(world, game.models.art.as_ref().map(|a| &a.fs)) {
        TerrainBuild::Waiting => return,
        TerrainBuild::Ready(look) => look,
    };
    for e in &grounds {
        commands.entity(e).despawn();
    }
    let width = world.terrain.logic_height_map().map_or(0, |m| m.x_extent());
    spawn_terrain(&mut commands, &mut meshes, &mut materials, &mut images, &look, width);
    info!("terrain: {}", look.name());
    // #35: the radar's terrain from the tiles' colours (`W3DRadar::buildTerrainTexture`'s `getTerrainColorAt`)
    let g = &mut *game;
    g.controls.radar.set_terrain_colors(look.radar_colors().cloned(), &mut g.controls.client.world);
    g.radar_drawn = None;
    state.pending = false;
    state.look = look.name();
}

/// GUI glue (needs a window): the player's files, once the page has handed them over, replace the demo with their game.
#[allow(clippy::too_many_arguments)]
fn poll_import(
    mut commands: Commands,
    mut game: NonSendMut<Game>,
    mut drawn: ResMut<Drawn>,
    mut rig: ResMut<Rig>,
    mut terrain: ResMut<TerrainState>,
) {
    // (#52: the audio files' headers are fetched first, over as many frames as the page takes to read them)
    let Some(started) = crate::import::poll_start() else { return };
    game.playing = "loading your Zero Hour files...".to_string();
    match started {
        Ok((s, art)) => {
            info!("playing your game: {}", s.report);
            crate::import::set_outcome(Ok(&s.report));
            #[cfg(target_arch = "wasm32")]
            let art = Some(Art::new(art));
            #[cfg(not(target_arch = "wasm32"))]
            let art = {
                // (natively the files come from --zh, not from the page)
                drop(art);
                None
            };
            game.replace(Game::new(s.world, s.player, s.report, art));
            game.base = Some(Box::new(s.base));
            for (_, (e, _)) in drawn.0.drain() {
                commands.entity(e).despawn();
            }
            terrain.pending = true;
            *rig = Rig::new(&game.controls.client.world, rig.cam.screen);
        }
        Err(e) => {
            warn!("your files did not start a game: {e}");
            crate::import::set_outcome(Err(&e));
            game.playing = format!("demo ({e})");
        }
    }
}

/// GUI glue (needs the app): the art's exchange with the page each frame (#24): the ranges the page read go to the art's
/// device and the ranges it wants are listed for the page (`crate::import::pump_ranges`, tested there), and the art's
/// status is set for the page's `art_status()`.
fn pump_art(mut game: NonSendMut<Game>, terrain: Res<TerrainState>) {
    let g = &mut *game;
    let Some(art) = &mut g.models.art else { return };
    #[cfg(target_arch = "wasm32")]
    let device = {
        crate::import::pump_ranges(&mut art.fs.local.device);
        let s = art.fs.local.device.stats();
        format!(
            " held={} total={} arrived={} ranges={} outstanding={} wasm_memory={}",
            s.held,
            s.total_len,
            s.bytes_arrived,
            s.ranges_arrived,
            art.fs.local.device.outstanding(),
            core::arch::wasm32::memory_size(0) * 65536
        )
    };
    #[cfg(not(target_arch = "wasm32"))]
    let device = {
        let _ = &art;
        String::new()
    };
    let models: std::collections::BTreeSet<&[u8]> = g.cur.objects.iter().filter_map(|b| g.models.instance(b.id).map(|m| m.name.as_slice())).collect();
    let audio = g.audio.as_deref().map_or_else(|| "none".to_string(), |s| s.device.status());
    crate::import::set_art_status(format!(
        "models={} names={} remade={} missing={} terrain={} audio=[{audio}]{device}",
        g.models.instance_count(),
        models.into_iter().map(|n| String::from_utf8_lossy(n).into_owned()).collect::<Vec<_>>().join(","),
        g.models.remade,
        g.models.missing.len(),
        terrain.look
    ));
}

fn setup_scene(mut commands: Commands, game: NonSend<Game>) {
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(0.0, 1.0, 0.0).looking_at(Vec3::new(-0.4, 0.0, -0.3), Vec3::Y),
    ));
    let rig = Rig::new(&game.controls.client.world, (800, 600));
    commands.spawn((Camera3d::default(), Transform::default()));
    // (#138) the UI's own camera over the whole window, drawn after the world's: the world's camera keeps to the tactical
    // view's viewport (`W3DView::setHeight`), which the UI must not be cut to
    commands.spawn((Camera2d, Camera { order: 1, clear_color: ClearColorConfig::None, ..default() }, IsDefaultUiCamera));
    commands.insert_resource(rig);
}

fn step_logic(mut game: NonSendMut<Game>, mut rig: Option<ResMut<Rig>>, time: Res<Time>) {
    if game.paused || game.error.is_some() {
        return;
    }
    let g = &mut *game;
    g.controls.observe_sent();
    // (#130: the frame the modules run in, `TheGameLogic->getFrame()` during it; the World has moved past it when the
    // events are read)
    let frame_run = g.controls.client.world.frame();
    // (`GameEngine::update`'s logic half: the message stream's commands, then the frame)
    match g.controls.world().engine_update() {
        Ok(()) => {
            // (#4: the frame, for the page's checks)
            crate::saves::page::set_frame(g.controls.client.world.frame());
            g.prev = std::mem::take(&mut g.cur);
            g.cur = snapshot(g.controls.world());
            // the frame's client events, to the drawables (their flags; the draw itself is every rendered frame)
            let events = g.controls.world().take_client_events();
            // #130: the logic's radius decals (made, moved, throbbed, cleared; gone with their objects)
            g.logic_decals.take_events(&events, frame_run, &g.controls.client);
            // #116: the scripts' calls on the input (`DISABLE_INPUT`, `ENABLE_INPUT`), then the frame's tick through the
            // selection translator (its input gate ends a drag)
            // (#119: `LookAtResetModes` to the camera's translator)
            // (#117: `CloseWindows`, `doQuickVictory`'s `closeWindows`, to the shell's windows)
            let back = g.controls.take_script_input(&events, &mut g.pause.mouse);
            if back.reset_modes {
                if let Some(r) = rig.as_deref_mut() {
                    r.cam.reset_modes();
                }
            }
            g.close_windows_pending |= back.close_windows;
            // (#138: a script's letterbox `HideControlBar`/`ShowControlBar` and `deleteBuildTooltipLayout`, to the bar)
            // (#144: its `EnableLetterBox`, `TheDisplay->enableLetterBox(enable)` at `timeGetTime()`, to the display's
            // letterbox, and the zoom limit to the tactical view, `if( TheTacticalView )`)
            let now_ms = (time.elapsed_secs_f64() * 1000.0) as u32;
            for c in &back.bar_calls {
                if let rts_lockstep_engine::logic::client_event::ScriptInputCall::EnableLetterBox(enable) = *c {
                    if let (Some(limit), Some(r)) = (g.letterbox.enable_letter_box(enable, now_ms), rig.as_deref_mut()) {
                        r.cam.view.zoom_limited = limit;
                    }
                } else if let Some(ui) = g.ui.as_mut() {
                    ui.script_call(c);
                }
            }
            g.controls.frame_tick();
            // #56: the logic's messages to the local player (`TheInGameUI->message`)
            g.controls.client.take_logic_messages(&events);
            // #35: the radar's calls; a stealth detector's feedback the radar's `tryEvent` refused does not play
            let radar = g.controls.take_radar_events(&events);
            let heard: Vec<_> = events.iter().enumerate().filter(|(i, _)| !radar.refused.contains(i)).map(|(_, e)| e.clone()).collect();
            let radar_sounds = std::mem::take(&mut g.radar_event_sounds);
            let now = g.models_clock;
            g.models.apply_events(g.controls.world(), &events, now);
            // #13: the frame's sounds, from the logic's events and the FX lists' requests, then the manager's frame
            let requests: Vec<_> = g.models.drawables.fx.requests.drain(..).collect();
            let voices: Vec<_> = g.controls.client.voices.drain(..).collect();
            let cheers = std::mem::take(&mut g.controls.client.all_cheers);
            let insufficient_funds = g.controls.client.eva_insufficient_funds;
            if let (Some(sound), Some(art)) = (g.audio.as_deref_mut(), g.models.art.as_ref()) {
                let files = |p: &[u8]| art.fs.does_file_exist(p);
                let w = &g.controls.client.world;
                let host = rts_lockstep_engine::client::client_audio::WorldAudioHost {
                    world: w,
                    local_player_index: g.models.drawables.local_player_index,
                    files: &files,
                    language: rts_lockstep_engine::logic::world::install_world::DEFAULT_LANGUAGE,
                };
                // #51: a map's `map.ini` audio events, as the logic read them; the unit voices the translators played since
                // the last frame; then the frame's events
                sound.audio.sync_logic_ini_audio(&w.statics.audio);
                // #117: `TheInGameUI->isClientQuiet()` (the radar edge sounds' last term)
                sound.audio.client_quiet = g.client_quiet;
                // #59: a loaded game's drawables' ambient sounds, once (`Drawable::xfer`'s and `loadPostProcess`'s)
                if g.ambient_restore_pending {
                    g.ambient_restore_pending = false;
                    let saved: Vec<_> = g.models.drawables.drawables.iter().map(|(id, d)| (*id, d.saved.ambient.clone())).collect();
                    sound.audio.ambient.restore_saved(&saved, &mut sound.audio.manager, &mut sound.device, &host);
                }
                for v in &voices {
                    sound.audio.play_unit_voice(&host, v);
                }
                // #125: `MSG_META_ALL_CHEER`'s sound
                sound.audio.play_all_cheer_sound(&host, cheers);
                for e in &heard {
                    sound.audio.apply_event(&host, &mut sound.device, e);
                }
                // #35: the radar's sounds and EVA (`tryUnderAttackEvent`, `tryInfiltrationEvent`, `drawEvents`)
                sound.audio.apply_radar_feedback(&host, &radar.feedback);
                sound.audio.play_radar_event_sound(&host, radar_sounds);
                for r in &requests {
                    sound.audio.fx_request(&host, r);
                }
                sound.audio.update_ambient(&host, &mut sound.device);
                // #55: EVA, from the frame's events and the client's own requests
                sound.audio.update_eva(&host, &heard, insufficient_funds);
                let view = rig.as_deref().map_or_else(Default::default, |r| r.cam.listener());
                sound.audio.manager.update(&host, &mut sound.device, &view);
            }
        }
        Err(e) => g.error = Some(format!("logic frame {}: {e:?}", g.controls.world().frame())),
    }
}

/// The mesh and its offset above the object's position for a shape, in Bevy's axes.
fn shape_mesh(shape: Shape) -> (Mesh, f32) {
    match shape {
        Shape::Sphere { radius } => (Sphere::new(radius.max(0.5)).into(), 0.0),
        Shape::Cylinder { radius, height } => {
            let h = height.max(0.5);
            (Cylinder::new(radius.max(0.5), h).into(), h / 2.0)
        }
        Shape::Box { half_forward, half_side, height } => {
            let h = height.max(0.5);
            (Cuboid::new(2.0 * half_forward.max(0.25), h, 2.0 * half_side.max(0.25)).into(), h / 2.0)
        }
    }
}

/// The drawn shape's offset, kept on the entity's child.
#[derive(Component)]
struct Body;

/// GUI glue (needs a window): each object drawn at its interpolated place, as its model when its drawable chose one that
/// the art has (`crate::drawables`, which its tests check without a window), else as its shape.
#[allow(clippy::too_many_arguments)]
fn sync_objects(
    mut commands: Commands,
    mut game: NonSendMut<Game>,
    time: Res<Time>,
    fixed: Res<Time<Fixed>>,
    mut drawn: ResMut<Drawn>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut transforms: Query<&mut Transform, (Without<Body>, Without<ModelPart>)>,
    mut parts: Query<(&ModelPart, &mut Transform, &MeshMaterial3d<StandardMaterial>), Without<Body>>,
) {
    let t = if game.paused || game.error.is_some() { 1.0 } else { fixed.overstep_fraction() };
    let g = &mut *game;
    // the draw modules' clock: the client's own (upstream's sync time), not the logic's frames; (#108) it stands still
    // while the game is paused (`freezeTime`)
    let paused = g.paused;
    g.models_clock = g.pause.sync_time((time.elapsed_secs_f64() * 1000.0) as u32, paused);
    if g.error.is_none() {
        let now = g.models_clock;
        g.models.draw(g.controls.world(), now);
    }
    let mut alive = std::collections::HashSet::new();
    for b in &g.cur.objects {
        alive.insert(b.id);
        let o = match g.prev.find(b.id) {
            Some(a) => interpolate(a, b, t),
            None => b.clone(),
        };
        let mut transform = Transform::from_translation(Vec3::from(to_bevy(o.pos))).with_rotation(Quat::from_rotation_y(o.angle));
        // the drawable's client-side physics (#41, `applyPhysicsXform`): suspension, pitch and roll, in the model's frame
        if let Some(x) = g.models.physics_xform(b.id) {
            transform = Transform::from_matrix(transform.to_matrix() * crate::model::to_bevy_matrix(&x));
        }
        let mut instance = g.models.instance(b.id);
        // the model's textures, once per model; while one is being fetched (#24) the object stays its shape
        if let (Some(m), Some(art)) = (instance, &g.models.art) {
            if !g.images.contains_key(&m.name) {
                let unserved = art.fs.local.device.unserved_reads();
                let tex = crate::model::model_textures(&art.assets, &art.fs, m).unwrap_or_default();
                if art.fs.local.device.unserved_reads() == unserved {
                    let handles = tex
                        .iter()
                        .zip(&m.sub_objects)
                        .map(|(t, s)| t.iter().zip(&s.model.textures).map(|(r, x)| r.as_ref().ok().map(|img| images.add(crate::model::texture_image(img, x.clamp)))).collect())
                        .collect();
                    g.images.insert(m.name.clone(), handles);
                } else {
                    instance = None;
                }
            }
        }
        let model_name = instance.map(|m| m.name.clone()).unwrap_or_default();
        if let Some((e, name)) = drawn.0.get(&b.id) {
            if *name == model_name {
                if let Ok(mut tr) = transforms.get_mut(*e) {
                    *tr = transform;
                }
                continue;
            }
            // the model changed (or appeared, or went): drawn anew
            commands.entity(*e).despawn();
        }
        let e = commands.spawn((transform, Visibility::default(), Name::new(format!("{} #{}", o.template, o.id)))).id();
        let model_parts = instance.and_then(|m| crate::model::model_parts(m).ok().map(|p| (m, p)));
        match model_parts {
            Some((m, model_parts)) => {
                let handles = g.images.entry(model_name.clone()).or_default();
                commands.entity(e).with_children(|c| {
                    for part in &model_parts {
                        let sub = &m.sub_objects[part.sub];
                        let (v, n) = m.deformed(part.sub).unwrap_or_else(|_| (sub.model.vertices.clone(), sub.model.normals.clone()));
                        let mesh = meshes.add(crate::model::group_mesh(&sub.model, &part.group, &v, &n));
                        let no_textures = Vec::new();
                        let mut sm = part.material.to_standard_material(handles.get(part.sub).unwrap_or(&no_textures));
                        sm.uv_transform = crate::model::uv_offset_transform(sub.uv_offset);
                        let material = materials.add(sm);
                        let tm = m.sub_object_transform(part.sub).map(|x| crate::model::to_bevy_matrix(&x)).unwrap_or(Mat4::IDENTITY);
                        c.spawn((
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(material),
                            Transform::from_matrix(tm),
                            ModelPart { object: b.id, sub: part.sub, skin: sub.is_skin(), mesh },
                        ));
                    }
                });
            }
            None => {
                let (mesh, lift) = shape_mesh(o.shape);
                let [r, gr, bl] = o.color;
                let material = materials.add(StandardMaterial { base_color: Color::srgb(r, gr, bl), perceptual_roughness: 0.7, ..default() });
                commands.entity(e).with_children(|c| {
                    c.spawn((Body, Mesh3d(meshes.add(mesh)), MeshMaterial3d(material), Transform::from_xyz(0.0, lift, 0.0)));
                });
            }
        }
        drawn.0.insert(b.id, (e, model_name));
    }
    // the models' poses: each part at its bone, each skin deformed, each tread's UV offset (#41) on its own material
    for (part, mut transform, material) in &mut parts {
        let Some(m) = g.models.instance(part.object) else { continue };
        if let Ok(x) = m.sub_object_transform(part.sub) {
            *transform = Transform::from_matrix(crate::model::to_bevy_matrix(&x));
        }
        let uv = crate::model::uv_offset_transform(m.sub_objects.get(part.sub).and_then(|s| s.uv_offset));
        if materials.get(&material.0).is_some_and(|x| x.uv_transform != uv) {
            if let Some(mut x) = materials.get_mut(&material.0) {
                x.uv_transform = uv;
            }
        }
        if part.skin {
            if let (Ok((pos, nrm)), Some(mut mesh)) = (m.deformed(part.sub), meshes.get_mut(&part.mesh)) {
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos.iter().map(|p| to_bevy(*p)).collect::<Vec<_>>());
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm.iter().map(|p| to_bevy(*p)).collect::<Vec<_>>());
            }
        }
    }
    drawn.0.retain(|id, (e, _)| {
        let keep = alive.contains(id);
        if !keep {
            commands.entity(*e).despawn();
        }
        keep
    });
}

fn health_bars(game: NonSend<Game>, fixed: Res<Time<Fixed>>, rig: Res<Rig>, mut gizmos: Gizmos) {
    let t = if game.paused || game.error.is_some() { 1.0 } else { fixed.overstep_fraction() };
    // the bar lies across the screen: along the camera's right, in game coordinates (cos yaw, sin yaw)
    let yaw = rig.cam.view.get_angle();
    let right = Vec3::from(to_bevy([rts_lockstep_engine::ww::wwmath::cos(yaw), rts_lockstep_engine::ww::wwmath::sin(yaw), 0.0]));
    for b in &game.cur.objects {
        let Some(h) = b.health else { continue };
        let o = match game.prev.find(b.id) {
            Some(a) => interpolate(a, b, t),
            None => b.clone(),
        };
        let top = match o.shape {
            Shape::Sphere { radius } => radius,
            Shape::Cylinder { height, .. } | Shape::Box { height, .. } => height,
        };
        let half = match o.shape {
            Shape::Sphere { radius } | Shape::Cylinder { radius, .. } => radius,
            Shape::Box { half_forward, half_side, .. } => half_forward.max(half_side),
        }
        .max(3.0);
        let base = Vec3::from(to_bevy([o.pos[0], o.pos[1], o.pos[2] + top + 3.0]));
        let start = base - right * half;
        let end = base + right * half;
        // the filled part and the missing part, end to end (two lines over each other draw in no fixed order)
        let split = start + (end - start) * h;
        let colour = if h > 0.5 { Color::srgb(0.2, 0.9, 0.2) } else if h > 0.25 { Color::srgb(0.95, 0.8, 0.1) } else { Color::srgb(0.95, 0.15, 0.1) };
        if h > 0.0 {
            gizmos.line(start, split, colour);
        }
        if h < 1.0 {
            gizmos.line(split, end, Color::srgb(0.25, 0.05, 0.05));
        }
    }
}

/// (#74) Whether the game's UI has the keyboard (a window of it has the focus): the camera's keys are then the UI's,
/// as upstream's `WindowXlat` eats a key the window manager used.
#[derive(Resource, Default)]
struct UiKeyboard(bool);

/// (#74) The DirectInput code (`KeyDefs.h`'s `KEY_*`, the scan code) of a physical key, 0 for one the game has no code
/// for. GUI glue (a table of Bevy's key codes; the path it feeds is tested in `crate::ui::keyboard_into`).
fn dik_of(k: KeyCode) -> u8 {
    use KeyCode as K;
    let letters = [(K::KeyQ, 0x10), (K::KeyW, 0x11), (K::KeyE, 0x12), (K::KeyR, 0x13), (K::KeyT, 0x14), (K::KeyY, 0x15), (K::KeyU, 0x16), (K::KeyI, 0x17), (K::KeyO, 0x18), (K::KeyP, 0x19)];
    let row2 = [(K::KeyA, 0x1E), (K::KeyS, 0x1F), (K::KeyD, 0x20), (K::KeyF, 0x21), (K::KeyG, 0x22), (K::KeyH, 0x23), (K::KeyJ, 0x24), (K::KeyK, 0x25), (K::KeyL, 0x26)];
    let row3 = [(K::KeyZ, 0x2C), (K::KeyX, 0x2D), (K::KeyC, 0x2E), (K::KeyV, 0x2F), (K::KeyB, 0x30), (K::KeyN, 0x31), (K::KeyM, 0x32)];
    let digits = [(K::Digit1, 0x02), (K::Digit2, 0x03), (K::Digit3, 0x04), (K::Digit4, 0x05), (K::Digit5, 0x06), (K::Digit6, 0x07), (K::Digit7, 0x08), (K::Digit8, 0x09), (K::Digit9, 0x0A), (K::Digit0, 0x0B)];
    let fkeys = [(K::F1, 0x3B), (K::F2, 0x3C), (K::F3, 0x3D), (K::F4, 0x3E), (K::F5, 0x3F), (K::F6, 0x40), (K::F7, 0x41), (K::F8, 0x42), (K::F9, 0x43), (K::F10, 0x44), (K::F11, 0x57), (K::F12, 0x58)];
    let other = [
        (K::Escape, 0x01), (K::Minus, 0x0C), (K::Equal, 0x0D), (K::Backspace, 0x0E), (K::Tab, 0x0F), (K::BracketLeft, 0x1A), (K::BracketRight, 0x1B), (K::Enter, 0x1C), (K::ControlLeft, 0x1D),
        (K::Semicolon, 0x27), (K::Quote, 0x28), (K::Backquote, 0x29), (K::ShiftLeft, 0x2A), (K::Backslash, 0x2B), (K::Comma, 0x33), (K::Period, 0x34), (K::Slash, 0x35), (K::ShiftRight, 0x36),
        (K::AltLeft, 0x38), (K::Space, 0x39), (K::CapsLock, 0x3A), (K::NumpadEnter, 0x9C), (K::ControlRight, 0x9D), (K::AltRight, 0xB8), (K::Home, 0xC7), (K::ArrowUp, 0xC8), (K::PageUp, 0xC9),
        (K::ArrowLeft, 0xCB), (K::ArrowRight, 0xCD), (K::End, 0xCF), (K::ArrowDown, 0xD0), (K::PageDown, 0xD1), (K::Insert, 0xD2), (K::Delete, 0xD3),
    ];
    letters.iter().chain(&row2).chain(&row3).chain(&digits).chain(&fkeys).chain(&other).find(|(c, _)| *c == k).map_or(0, |&(_, d)| d)
}

/// (#74) GUI glue (needs a window's keyboard events): each of Bevy's keyboard events, natively and in the browser,
/// into the game's UI ([`crate::ui::GameUi::keyboard`], tested there): the key by its DirectInput code, and the text it
/// typed. ~~The modifier states (`KEY_STATE_CONTROL`, ...) are not passed yet.~~ **Corrected 2026-11-11 (#125):** the window
/// manager's keys still carry no modifier state; the keys the game's windows do not use (none has the focus) go on to
/// `MetaEventTranslator` with the keyboard's modifier bits ([`crate::input::Controls::key`]), which turns the
/// CommandMap's bindings into the meta messages the selection and command translators carry out. `CommandMap.ini`
/// (`Data\English\`, then `Data\INI\`) is read once the game's files are up. A phone has no keyboard: none of it runs.
///
/// (#87) Escape in a game (`MSG_META_OPTIONS`) cancels a placement or a pending command first (the client's, #6), else
/// toggles the quit menu ([`crate::shell::ShellUi::toggle_quit_menu`], tested there) when the shell is loaded; while the
/// in-game menus are up they have the keyboard, and an Escape their windows did not use toggles again (upstream's
/// window manager sees the key before the meta map).
fn keyboard_ui(mut events: MessageReader<KeyboardInput>, mut game: NonSendMut<Game>, mut taken: ResMut<UiKeyboard>, mut shell: NonSendMut<ShellState>) {
    let g = &mut *game;
    // (#78) the shell, while it is shown, has the keyboard (`crate::shell::ShellUi::keyboard`, tested there); (#87) so
    // do the in-game menus
    if let Some(s) = shell.ui.as_mut().filter(|s| s.takes_input()) {
        for k in events.read() {
            let down = k.state == ButtonState::Pressed;
            // (#136: with the one mouse's tooltip lent, `ShellUi::with_mouse`)
            let used = s.with_mouse(&mut g.pause.mouse.tooltip, |s| s.keyboard(dik_of(k.key_code), down, k.text.as_deref()));
            if !used && down && k.key_code == KeyCode::Escape && !s.is_active() {
                s.toggle_quit_menu(crate::shell::quit_menu_game(&g.controls.client.world, g.controls.ui.input_enabled()));
            }
        }
        taken.0 = true;
        return;
    }
    let panel = crate::saves::page::panel_open() || crate::campaigns::page::panel_open();
    let mut keys: Vec<KeyboardInput> = Vec::new();
    for k in events.read() {
        if k.key_code == KeyCode::Escape && k.state == ButtonState::Pressed && !panel {
            let c = &mut g.controls;
            if c.ui.pending_place_type().is_some() || c.ui.gui_command().is_some() {
                // (the client's cancel, #6)
                c.ui.place_build_available(None, None, 0.0, &c.client);
                c.ui.set_gui_command(None);
            } else if let Some(s) = shell.ui.as_mut() {
                s.toggle_quit_menu(crate::shell::quit_menu_game(&c.client.world, c.ui.input_enabled()));
            }
            continue;
        }
        keys.push(k.clone());
    }
    // (#125) `TheMetaMap`, once the game's files are up: `Data\English\CommandMap.ini`, then `Data\INI\CommandMap.ini`
    if !g.controls.meta_map_tried {
        if let Some(art) = g.models.art.as_ref() {
            let waiting = std::cell::Cell::new(false);
            let open = |p: &[u8]| {
                let before = art.fs.local.device.unserved_reads();
                let r = art.fs.open_file(p);
                if art.fs.local.device.unserved_reads() != before {
                    waiting.set(true);
                    return None;
                }
                r
            };
            let language = String::from_utf8_lossy(rts_lockstep_engine::logic::world::install_world::DEFAULT_LANGUAGE).into_owned();
            let mut map = rts_lockstep_engine::client::meta_event::MetaMap::new();
            let text = &g.controls.client.world.statics.game_text;
            let read = map.load_files(&language, open, Some(text));
            if !waiting.get() {
                g.controls.meta_map_tried = true;
                match read {
                    Ok(true) => g.controls.meta_map = map,
                    Ok(false) => warn!("no CommandMap.ini: the game's keys are not bound"),
                    Err(e) => warn!("the CommandMap did not read: {e:?}"),
                }
            }
        }
    }
    // the game's windows first (`WindowTranslator`), then (#125) the keys they did not use through the CommandMap
    let mut used = vec![false; keys.len()];
    match g.ui.as_mut() {
        Some(ui) => {
            for (k, u) in keys.iter().zip(used.iter_mut()) {
                *u = ui.with_mouse(&mut g.pause.mouse.tooltip, |ui| ui.keyboard(dik_of(k.key_code), k.state == ButtonState::Pressed, k.text.as_deref()));
            }
            taken.0 = ui.has_keyboard_focus();
        }
        None => taken.0 = false,
    }
    if !taken.0 {
        for (k, u) in keys.iter().zip(used) {
            let dik = dik_of(k.key_code);
            if !u && dik != 0 {
                g.controls.key(dik, k.state == ButtonState::Pressed, k.repeat);
            }
        }
    }
}

/// GUI glue (needs a window): the player's camera input, as raw input messages, through the camera's translated
/// `LookAtTranslator` ([`crate::camera::Camera::translate`], tested there), then the client's frames
/// ([`crate::camera::Camera::tick`]). ~~#116: with the InGameUI's input off the camera takes none of them ...~~
/// **Corrected 2026-11-10 (#119):** the translator's own input gates decide (LookAtXlat.cpp:76, :294), and the window
/// translator takes the raw mouse first. The arrows are the game's scroll keys (`KEY_UP` ...); Q and E turn and + and -
/// zoom (the InGameUI's keyboard camera; **#131:** only where the CommandMap binds no key to that camera meta message,
/// and then through the same `MSG_META_BEGIN/END_CAMERA_*` messages; the on-screen `<` and `>` buttons, held, turn the
/// view the same way, and `R` sends `MSG_META_CAMERA_RESET`) **(#125:** a key the CommandMap binds is left to it, as upstream's
/// `MetaEventTranslator` destroys a key it maps before `LookAtTranslator` sees it); the translators' `lookAt`s (the meta
/// keys, a control group's double press) and the camera tracking move the view here; the right button drags to scroll (the game's RMB scroll), the middle button
/// turns, the wheel zooms. On a touch screen one finger is the right button and two pinch (a wheel notch per 15 %).
#[allow(clippy::too_many_arguments)]
fn camera_input(
    mut game: NonSendMut<Game>,
    taken: Res<UiKeyboard>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    touches: Res<Touches>,
    time: Res<Time>,
    windows: Query<&Window>,
    bar: Res<UiBar>,
    shell: NonSend<ShellState>,
    mut rig: ResMut<Rig>,
    keyboard_scroll: Res<KeyboardScroll>,
    turn_buttons: Query<(Has<bevy::ui::Pressed>, &TurnButton)>,
) {
    use rts_lockstep_engine::client::window_xlat::RawMessage;
    use rts_lockstep_engine::common::message_types::*;
    let g = &mut *game;
    let r = &mut *rig;
    if let Ok(w) = windows.single() {
        r.cam.set_screen((w.width() as i32, w.height() as i32), &g.controls.client.world);
        // (#138) `TheTacticalView->setHeight` as the bar's stages last asked (80 % with the bar shown), on the window
        let asked = g.ui.as_ref().filter(|_| !shell.ui.as_ref().is_some_and(|s| s.is_active())).and_then(|u| u.view_height);
        r.cam.set_view_height(crate::ui::tactical_view_height(asked, bar.placement, w.height()) as i32);
    }
    r.cam.set_keyboard_scroll(keyboard_scroll.0);
    let shell_active = shell.ui.as_ref().is_some_and(|s| s.takes_input());
    let panel = crate::saves::page::panel_open() || crate::campaigns::page::panel_open();
    let input = !shell_active && !panel;
    let ui = g.controls.ui.clone();
    // (#117) the translator's `getMouseCursor` is the client's mouse's
    r.cam.cursor = g.pause.mouse.get_mouse_cursor();
    let mouse_msg = |r: &mut Rig, g: &mut Game, t: rts_lockstep_engine::common::message_stream::MessageType, p: ICoord2D, spin: i32| {
        if g.controls.window_keeps(t, p) {
            let m = RawMessage { t, pos: p, delta: ICoord2D::default(), arg0: 0, arg1: spin };
            r.cam.translate(&m, &ui, &g.controls.client.world, shell_active);
        }
    };
    if input {
        // the keys (a UI window with the focus has the keyboard, #74)
        let free = !taken.0;
        // (#125: a key the CommandMap binds is MetaEventTranslator's, which upstream runs before the look-at translator)
        let own = |k: KeyCode| !g.controls.meta_map_binds(dik_of(k));
        for k in [KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::ArrowRight].into_iter().filter(|&k| own(k)) {
            let state = if keys.just_pressed(k) && free {
                Some(rts_lockstep_engine::client::window_manager::key::STATE_DOWN)
            } else if keys.just_released(k) {
                Some(rts_lockstep_engine::client::window_manager::key::STATE_UP)
            } else {
                None
            };
            if let Some(st) = state {
                let t = if st == rts_lockstep_engine::client::window_manager::key::STATE_UP { MSG_RAW_KEY_UP } else { MSG_RAW_KEY_DOWN };
                let m = RawMessage { t, pos: ICoord2D::default(), delta: ICoord2D::default(), arg0: i32::from(dik_of(k)), arg1: st as i32 };
                r.cam.translate(&m, &ui, &g.controls.client.world, shell_active);
            }
        }
        // (#131) the keyboard camera: the CommandMap's camera meta keys set the InGameUI's flags (`Controls::key`); where
        // it binds none, Q and E turn and + and - zoom as the application's fallback, through the same meta messages; the
        // on-screen turn buttons (a phone's way) always do
        let fallbacks = [
            (&[KeyCode::KeyQ][..], MSG_META_BEGIN_CAMERA_ROTATE_LEFT, MSG_META_END_CAMERA_ROTATE_LEFT, Some(0usize)),
            (&[KeyCode::KeyE][..], MSG_META_BEGIN_CAMERA_ROTATE_RIGHT, MSG_META_END_CAMERA_ROTATE_RIGHT, Some(1)),
            (&[KeyCode::Equal, KeyCode::NumpadAdd][..], MSG_META_BEGIN_CAMERA_ZOOM_IN, MSG_META_END_CAMERA_ZOOM_IN, None),
            (&[KeyCode::Minus, KeyCode::NumpadSubtract][..], MSG_META_BEGIN_CAMERA_ZOOM_OUT, MSG_META_END_CAMERA_ZOOM_OUT, None),
        ];
        for (ks, begin, end, button) in fallbacks {
            let bound = g.controls.meta_map_binds_meta(begin);
            let key_held = !bound && free && ks.iter().any(|&k| !g.controls.meta_map_binds(dik_of(k)) && keys.pressed(k));
            let button_held = button.is_some_and(|b| turn_buttons.iter().any(|(pressed, t)| t.0 == b && pressed));
            let held = key_held || button_held;
            let i = match begin {
                x if x == MSG_META_BEGIN_CAMERA_ROTATE_LEFT => 0,
                x if x == MSG_META_BEGIN_CAMERA_ROTATE_RIGHT => 1,
                x if x == MSG_META_BEGIN_CAMERA_ZOOM_IN => 2,
                _ => 3,
            };
            if held != r.camera_held[i] {
                r.camera_held[i] = held;
                g.controls.meta(if held { begin } else { end });
            }
        }
        if let Ok(window) = windows.single() {
            let bar_h = bar.height + bar.panel;
            if let Some(cursor) = window.cursor_position() {
                let p = pixel(cursor);
                if r.last_pos != Some(p) {
                    r.last_pos = Some(p);
                    mouse_msg(r, g, MSG_RAW_MOUSE_POSITION, p, 0);
                }
                let free_pointer = !over_ui(cursor, window, bar_h);
                // (#132) the mouse's right button goes through `SelectionTranslator` (50) first: the press keeps its place,
                // time and camera; a release that is a click cancels a GUI command (destroyed: the camera never gets it)
                // or deselects everything
                let now_ms = (time.elapsed_secs_f64() * 1000.0) as u32;
                let selection_keeps = |r: &mut Rig, g: &mut Game, t, is_down: bool| -> bool {
                    if !g.controls.window_keeps(t, p) {
                        return true;
                    }
                    let mut stop = false;
                    let d = g.controls.selection_right_button(is_down, p, now_ms, r.cam.view.pos, |on| stop = !on);
                    if stop {
                        r.cam.set_ui_scrolling(false);
                    }
                    d == rts_lockstep_engine::client::selection_xlat::Disposition::Keep
                };
                for (b, down, up) in [(MouseButton::Right, MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN, MSG_RAW_MOUSE_RIGHT_BUTTON_UP), (MouseButton::Middle, MSG_RAW_MOUSE_MIDDLE_BUTTON_DOWN, MSG_RAW_MOUSE_MIDDLE_BUTTON_UP)] {
                    let right = b == MouseButton::Right;
                    if buttons.just_pressed(b) && free_pointer && (!right || selection_keeps(r, g, down, true)) {
                        mouse_msg(r, g, down, p, 0);
                    }
                    if buttons.just_released(b) && (!right || selection_keeps(r, g, up, false)) {
                        mouse_msg(r, g, up, p, 0);
                    }
                }
                // the wheel: whole notches (a line, or 100 pixels)
                let lines = match scroll.unit {
                    bevy::input::mouse::MouseScrollUnit::Line => scroll.delta.y,
                    bevy::input::mouse::MouseScrollUnit::Pixel => scroll.delta.y / 100.0,
                };
                if free_pointer {
                    r.wheel += lines;
                }
                let spin = r.wheel as i32;
                if spin != 0 {
                    r.wheel -= spin as f32;
                    mouse_msg(r, g, MSG_RAW_MOUSE_WHEEL, p, spin);
                }
            }
            // touch: one finger is the right button, two pinch
            let active: Vec<_> = touches.iter().collect();
            if let Some(id) = r.touch {
                match touches.get_pressed(id) {
                    Some(t) if active.len() == 1 => mouse_msg(r, g, MSG_RAW_MOUSE_POSITION, pixel(t.position()), 0),
                    _ => {
                        let at = touches.get_pressed(id).map(|t| t.position()).or_else(|| touches.iter_just_released().find(|t| t.id() == id).map(|t| t.position())).unwrap_or_default();
                        r.touch = None;
                        mouse_msg(r, g, MSG_RAW_MOUSE_RIGHT_BUTTON_UP, pixel(at), 0);
                    }
                }
            } else if let [t] = active.as_slice() {
                if touches.just_pressed(t.id()) && !over_ui(t.position(), window, bar_h) {
                    r.touch = Some(t.id());
                    mouse_msg(r, g, MSG_RAW_MOUSE_POSITION, pixel(t.position()), 0);
                    mouse_msg(r, g, MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN, pixel(t.position()), 0);
                }
            }
            match active.as_slice() {
                [a, b] => {
                    let now = a.position().distance(b.position());
                    let base = *r.pinch.get_or_insert(now);
                    if now > 1.0 && base > 1.0 {
                        let step = if now > base * 1.15 { 1 } else if now * 1.15 < base { -1 } else { 0 };
                        if step != 0 {
                            r.pinch = Some(now);
                            let mid = pixel((a.position() + b.position()) / 2.0);
                            mouse_msg(r, g, MSG_RAW_MOUSE_WHEEL, mid, step);
                        }
                    }
                }
                _ => r.pinch = None,
            }
        }
    }
    if !input {
        // (#131) a held fallback key or turn button ends while the shell or a panel has the input
        let ends = [MSG_META_END_CAMERA_ROTATE_LEFT, MSG_META_END_CAMERA_ROTATE_RIGHT, MSG_META_END_CAMERA_ZOOM_IN, MSG_META_END_CAMERA_ZOOM_OUT];
        for (i, end) in ends.into_iter().enumerate() {
            if std::mem::take(&mut r.camera_held[i]) {
                g.controls.meta(end);
            }
        }
    }
    // (#131) `MSG_META_CAMERA_RESET`'s `TheInGameUI->resetCamera()` (a key, or the R button)
    for _ in 0..std::mem::take(&mut g.controls.client.camera_resets) {
        r.cam.reset_camera(&g.controls.client.world);
    }
    // (#125) the tactical view's `lookAt`s the translators asked for (the meta keys, a control group's double press), and
    // `GameClient::update`'s camera tracking
    for at in g.controls.take_look_ats() {
        r.cam.look_at(at, &g.controls.client.world);
    }
    if let Some(at) = g.controls.camera_tracking() {
        r.cam.look_at(at, &g.controls.client.world);
    }
    // the client's frames
    for _ in 0..r.cam.ticks(time.delta_secs()) {
        let ui = g.controls.ui.clone();
        r.cam.tick(&ui, &g.controls.client.world, shell_active);
    }
    g.controls.view_mouse_locked = r.cam.view.is_mouse_locked();
    g.controls.ui_scrolling = r.cam.ui_scrolling;
    // (#117) the translator's calls on the cursor, made on the client's mouse; the InGameUI's scroll for its direction.
    // ~~`gui_command_mode` was a pending GUI command~~ **Corrected 2026-11-11 (#122):** the calls join the InGameUI's
    // queue in order (`setMouseCursor` reads the InGameUI's own mouse mode and mode cursor), which `show_cursor` makes
    // on the mouse; and `hasMouseMovedRecently` for the hints
    for c in r.cam.cursor_calls.drain(..) {
        match c {
            crate::camera::CursorCall::InGameUi(c) => g.controls.ui.set_mouse_cursor(c),
            crate::camera::CursorCall::Mouse(c) => g.controls.ui.mouse_set_cursor(c),
        }
    }
    let logic_frame = g.controls.client.world.frame();
    g.controls.client.view.mouse_moved_recently = r.cam.look.has_mouse_moved_recently(logic_frame);
    let playback = rts_lockstep_engine::client::in_game_ui_cursor::CursorHintHost::recorder_playback(&g.controls.client);
    g.controls.ui.set_recorder_playback(playback);
    g.pause.mouse.ui_scrolling = r.cam.ui_scrolling;
    g.pause.mouse.ui_scroll_amount = r.cam.scroll_amount;
}

/// GUI glue (needs a window; the decisions are the translated mouse's, tested in the logic crate, and the icons'
/// mapping is [`crate::cursor`]'s, tested there): `Mouse.ini` read once the art is up, the window's focus given to the
/// mouse (`WM_SETFOCUS`, `WM_KILLFOCUS`), the mouse drawn (`W3DMouse::draw`, once a frame), and the window's cursor set
/// to what it shows: hidden, or the system icon for it. A phone has no cursor: the settings change nothing there.
fn show_cursor(mut commands: Commands, mut game: NonSendMut<Game>, time: Res<Time>, touches: Res<Touches>, mut windows: Query<(Entity, &Window, &mut bevy::window::CursorOptions, Option<&bevy::window::CursorIcon>), With<bevy::window::PrimaryWindow>>) {
    use crate::cursor::{cursor_look, CursorLook, Icon};
    use bevy::window::{CursorIcon, SystemCursorIcon as S};
    use rts_lockstep_engine::client::radius_cursor::RadiusUiState;
    let g = &mut *game;
    let now = (time.elapsed_secs_f64() * 1000.0) as u32;
    // (#124) the mouse's position and limits (`m_currMouse.pos`, `setMouseLimits`), and `createStreamMessages`' tooltip
    // timer: the mouse moved this frame or not. A phone has no hover: the finger's place stands for it while it touches.
    if let Ok((_, window, _, _)) = windows.single() {
        g.pause.mouse.set_mouse_limits(window.width() as i32, window.height() as i32);
        if let Some(p) = window.cursor_position().or_else(|| touches.iter().next().map(|t| t.position())) {
            let p = pixel(p);
            let moved = p != g.pause.mouse.pos;
            g.pause.mouse.pos = p;
            g.pause.mouse.tooltip.create_stream_messages(now, moved);
        } else {
            g.pause.mouse.tooltip.create_stream_messages(now, false);
        }
    }
    // (#122) the InGameUI's calls on `TheMouse` this frame (setMouseCursor from the hints, the GUI command, the
    // placement and the camera's scroll), in order, before the mouse draws; (#124) the tooltip's, and the radius cursor's
    // `setRadiusCursor`
    {
        let calls = g.controls.ui.take_mouse_calls();
        let state = RadiusUiState::of(&g.controls.ui);
        let (radius, client) = (&mut g.radius, &mut g.controls.client);
        crate::cursor::apply_ui_mouse_calls(&mut g.pause.mouse, calls, now, |kind, sp, slot| radius.set_radius_cursor(&state, kind, sp, slot, client));
        // `InGameUI::preDraw`'s `handleRadiusCursor()`
        radius.handle_radius_cursor(&state, client);
    }
    if g.in_game_ui_pending {
        if let Some(art) = g.models.art.as_ref() {
            let waiting = std::cell::Cell::new(false);
            let open = |p: &[u8]| {
                let before = art.fs.local.device.unserved_reads();
                let r = art.fs.open_file(p);
                if art.fs.local.device.unserved_reads() != before {
                    waiting.set(true);
                    return None;
                }
                r
            };
            match crate::cursor::load_in_game_ui(&mut g.radius, open, || waiting.get()) {
                None => {}
                Some(Ok(())) => g.in_game_ui_pending = false,
                Some(Err(e)) => {
                    warn!("the radius cursors: {e}");
                    g.in_game_ui_pending = false;
                }
            }
        }
    }
    if g.mouse_pending {
        let Some(art) = g.models.art.as_ref() else { return };
        let waiting = std::cell::Cell::new(false);
        let open = |p: &[u8]| {
            let before = art.fs.local.device.unserved_reads();
            let r = art.fs.open_file(p);
            if art.fs.local.device.unserved_reads() != before {
                waiting.set(true);
                return None;
            }
            r
        };
        match crate::cursor::load_mouse(&mut g.pause.mouse, open, || waiting.get()) {
            None => return,
            Some(Ok(())) => {
                // (the main menu or the game's load screen makes it visible; the client's game is already up)
                g.pause.mouse.set_visibility(true);
                g.mouse_pending = false;
                // (#131) `TheMouse`'s drag tolerances, for the clicks (`MetaEventTranslator`, `Mouse::isClick`)
                let st = &g.pause.mouse.tooltip.settings;
                g.controls.client.view.mouse_drag_tolerance = Some((st.drag_tolerance, st.drag_tolerance_ms));
                // (#132) and the 3-D one, for the selection translator's right click
                g.controls.client.view.mouse_drag_tolerance_3d = Some(st.drag_tolerance_3d);
            }
            Some(Err(e)) => {
                warn!("the mouse's cursors: {e}");
                g.mouse_pending = false;
                return;
            }
        }
    }
    let Ok((e, window, mut options, icon)) = windows.single_mut() else { return };
    g.pause.mouse.lost_focus(!window.focused);
    g.pause.mouse.draw();
    let look = cursor_look(&g.pause.mouse);
    let visible = look != CursorLook::Hidden;
    if options.visible != visible {
        options.visible = visible;
    }
    if let CursorLook::Icon(i) = look {
        let s = match i {
            Icon::Default => S::Default,
            Icon::Pointer => S::Pointer,
            Icon::Crosshair => S::Crosshair,
            Icon::Move => S::Move,
            Icon::NotAllowed => S::NotAllowed,
            Icon::Cell => S::Cell,
            Icon::Grab => S::Grab,
            Icon::Help => S::Help,
            Icon::EResize => S::EResize,
            Icon::SeResize => S::SeResize,
            Icon::SResize => S::SResize,
            Icon::SwResize => S::SwResize,
            Icon::WResize => S::WResize,
            Icon::NwResize => S::NwResize,
            Icon::NResize => S::NResize,
            Icon::NeResize => S::NeResize,
        };
        let want = CursorIcon::System(s);
        if icon != Some(&want) {
            commands.entity(e).insert(want);
        }
    }
}

/// GUI glue (needs a window): the 3D camera where the translated view placed it ([`crate::camera::Camera::eye_target`]
/// and [`crate::camera::Camera::projection`], tested there); (#138) drawn into the view's viewport
/// ([`crate::camera::Camera::viewport`], tested there: `W3DView::setHeight`'s `Set_Viewport`), the whole window when the
/// view is.
fn apply_camera(rig: Res<Rig>, windows: Query<&Window>, mut cams: Query<(&mut Transform, &mut Projection, &mut Camera), With<Camera3d>>) {
    let (eye, target) = rig.cam.eye_target();
    let (fov, near, far) = rig.cam.projection();
    let viewport = windows.single().ok().and_then(|w| {
        let (vw, vh) = rig.cam.viewport();
        if vh as f32 >= w.height() && vw as f32 >= w.width() {
            return None;
        }
        let sf = w.scale_factor();
        let size = UVec2::new(((vw as f32 * sf) as u32).clamp(1, w.physical_width().max(1)), ((vh as f32 * sf) as u32).clamp(1, w.physical_height().max(1)));
        Some(bevy::camera::Viewport { physical_position: UVec2::ZERO, physical_size: size, ..default() })
    });
    for (mut t, mut proj, mut camera) in &mut cams {
        let key = |v: &Option<bevy::camera::Viewport>| v.as_ref().map(|v| (v.physical_position, v.physical_size));
        if key(&camera.viewport) != key(&viewport) {
            camera.viewport = viewport.clone();
        }
        *t = Transform::from_translation(Vec3::from(to_bevy(eye))).looking_at(Vec3::from(to_bevy(target)), Vec3::Y);
        if let Projection::Perspective(p) = &mut *proj {
            p.fov = fov;
            p.near = near;
            p.far = far;
        }
    }
}

fn update_hud(game: NonSend<Game>, mut hud: Query<&mut Text, With<Hud>>) {
    for mut text in &mut hud {
        let state = match (&game.error, game.paused) {
            (Some(e), _) => format!("stopped: {e}"),
            (None, true) => "paused".to_string(),
            (None, false) => "running".to_string(),
        };
        let hint = game.controls.hint.map_or(String::new(), |h| format!(" | {}", hint_name(h)));
        let placing = if game.controls.ui.pending_place_type().is_some() { " | placing: click to build, Esc to cancel" } else { "" };
        // the art's progress (#24): models drawn, and files still being read
        let art = match &game.models.art {
            Some(_) => {
                let waiting = game.models.waiting_models.len() + game.models.waiting_anims.len();
                let reading = if waiting > 0 { format!(", reading {waiting} files") } else { String::new() };
                format!(" | {} models{reading}", game.models.instance_count())
            }
            None => String::new(),
        };
        **text = format!(
            "{} | logic frame {} | {} objects{art} | {} selected{hint}{placing} | {} | {state}",
            game.playing,
            game.cur.frame,
            game.cur.objects.len(),
            game.controls.ui.select_count(),
            game.ui_status
        );
    }
}

/// The on-screen controls (zoom in, zoom out, reset, pause) and the status line. The buttons are the controls a phone
/// has; the keys and the mouse are extras.
fn setup_ui(mut commands: Commands) {
    commands.spawn((
        DragBox,
        Visibility::Hidden,
        Node { position_type: PositionType::Absolute, border: UiRect::all(px(1)), ..default() },
        BorderColor::all(Color::srgb(0.3, 1.0, 0.4)),
    ));
    commands.spawn((
        ControlBar { shown: u32::MAX },
        Node {
            position_type: PositionType::Absolute,
            left: px(8),
            bottom: px(8),
            column_gap: px(6),
            flex_direction: FlexDirection::Row,
            ..default()
        },
    ));
    commands.spawn((
        Hud,
        Text::new(""),
        TextColor(Color::WHITE),
        Node { position_type: PositionType::Absolute, top: px(8), left: px(8), ..default() },
    ));
    commands.spawn((
        ZoomColumn,
        Node {
            position_type: PositionType::Absolute,
            right: px(8),
            bottom: px(8),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            ..default()
        },
        children![
            // (#119: three wheel notches through the translator, and the reset of `Camera::reset`)
            (button("+"), observe(|_: On<Activate>, mut rig: ResMut<Rig>, game: NonSend<Game>| zoom_button(&mut rig, &game, 3))),
            (button("-"), observe(|_: On<Activate>, mut rig: ResMut<Rig>, game: NonSend<Game>| zoom_button(&mut rig, &game, -3))),
            // (#131) the reset is the game's: `MSG_META_CAMERA_RESET` through the command translator (`resetCamera`: the
            // default angle and zoom where the view looks; ~~the start's point~~); the turn buttons, held, turn the view
            (button("R"), observe(|_: On<Activate>, mut game: NonSendMut<Game>| {
                game.controls.meta(rts_lockstep_engine::common::message_types::MSG_META_CAMERA_RESET);
            })),
            (button("<"), TurnButton(0)),
            (button(">"), TurnButton(1)),
            (button("||"), observe(|_: On<Activate>, mut game: NonSendMut<Game>| game.paused = !game.paused)),
            // (#4) the saves: the panel with the save button and the list
            (button("S"), observe(|_: On<Activate>, mut action: ResMut<SaveAction>| action.0 = Some(SaveCmd::Toggle))),
            // (#62) the campaigns: the missions and the Generals' Challenge, and the score screen
            (button("C"), observe(|_: On<Activate>, mut action: ResMut<CampaignAction>| action.0 = Some(CampaignCmd::Toggle))),
            // (#136) the control bar hidden and shown: `MSG_META_TOGGLE_CONTROL_BAR` through the command translator, as
            // its key makes it (a phone has no keyboard; the bar's own `ButtonLarge` lowers and raises it, but is hidden
            // with it)
            (button("B"), observe(|_: On<Activate>, mut game: NonSendMut<Game>| {
                game.controls.meta(rts_lockstep_engine::common::message_types::MSG_META_TOGGLE_CONTROL_BAR);
            })),
            // (#135, owner decision 2026-11-11) a phone has no right button: `X` sends what a right click sends, at the
            // view's centre, through SelectionXlat's (#132/#134) and CommandXlat's (#131) right button: it cancels a
            // pending GUI command, otherwise it deselects
            (button("X"), observe(|_: On<Activate>, mut game: NonSendMut<Game>, mut rig: ResMut<Rig>, time: Res<Time>| cancel_button(&mut rig, &mut game, &time))),
        ],
    ));
}

/// GUI glue (a button's observer; the wheel's path is tested in `crate::camera`): the zoom buttons as `spin` wheel
/// notches at the screen's centre.
fn zoom_button(rig: &mut Rig, game: &Game, spin: i32) {
    use rts_lockstep_engine::client::window_xlat::RawMessage;
    let (w, h) = rig.cam.screen;
    let m = RawMessage { t: rts_lockstep_engine::common::message_types::MSG_RAW_MOUSE_WHEEL, pos: ICoord2D { x: w / 2, y: h / 2 }, delta: ICoord2D::default(), arg0: 0, arg1: spin };
    rig.cam.translate(&m, &game.controls.ui, &game.controls.client.world, false);
}

/// GUI glue (a button's observer; the click's path is tested in `crate::input`, `the_cancel_button_is_a_right_click`): the
/// phone's `X` as a right click at the view's centre ([`crate::input::Controls::cancel_click`]), the camera's translator
/// given the raw press and release the selection translator keeps, as `camera_input` gives a mouse's.
fn cancel_button(rig: &mut Rig, game: &mut Game, time: &Time) {
    use rts_lockstep_engine::client::window_xlat::RawMessage;
    use rts_lockstep_engine::common::message_types::{MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN, MSG_RAW_MOUSE_RIGHT_BUTTON_UP};
    let (w, h) = rig.cam.screen;
    let at = ICoord2D { x: w / 2, y: h / 2 };
    let ground = rig.cam.ground_under(at, |x, y| game.controls.client.world.terrain.ground_height(x, y, false).0).unwrap_or_default();
    let now_ms = (time.elapsed_secs_f64() * 1000.0) as u32;
    let mut stop = false;
    let (down, up) = game.controls.cancel_click(at, now_ms, rig.cam.view.pos, ground, |on| stop |= !on);
    if stop {
        rig.cam.set_ui_scrolling(false);
    }
    for (kept, t) in [(down, MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN), (up, MSG_RAW_MOUSE_RIGHT_BUTTON_UP)] {
        if kept {
            let m = RawMessage { t, pos: at, delta: ICoord2D::default(), arg0: 0, arg1: 0 };
            rig.cam.translate(&m, &game.controls.ui, &game.controls.client.world, false);
        }
    }
}

fn button(label: &str) -> impl Bundle {
    (
        Button,
        Node {
            width: px(48),
            height: px(48),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(8)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.1, 0.1, 0.12, 0.8)),
        children![(Text::new(label), TextColor(Color::WHITE))],
    )
}

/// A hint's name for the status line (`MSG_DO_MOVETO_HINT` reads "move").
fn hint_name(h: rts_lockstep_engine::common::message_stream::MessageType) -> String {
    use rts_lockstep_engine::common::message_types::*;
    let name = match h {
        x if x == MSG_DO_MOVETO_HINT => "move",
        x if x == MSG_DO_ATTACK_OBJECT_HINT => "attack",
        x if x == MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT => "attack (after moving)",
        x if x == MSG_IMPOSSIBLE_ATTACK_HINT => "cannot attack",
        x if x == MSG_DO_FORCE_ATTACK_OBJECT_HINT || x == MSG_DO_FORCE_ATTACK_GROUND_HINT => "force attack",
        x if x == MSG_DO_INVALID_HINT => "cannot go there",
        x if x == MSG_ENTER_HINT => "enter",
        x if x == MSG_DOCK_HINT => "dock",
        x if x == MSG_DO_REPAIR_HINT => "repair",
        x if x == MSG_GET_REPAIRED_HINT => "get repaired",
        x if x == MSG_GET_HEALED_HINT => "get healed",
        x if x == MSG_RESUME_CONSTRUCTION_HINT => "resume construction",
        x if x == MSG_VALID_GUICOMMAND_HINT => "use",
        x if x == MSG_INVALID_GUICOMMAND_HINT => "cannot use here",
        x if x == MSG_ADD_WAYPOINT_HINT => "waypoint",
        x if x == MSG_DO_ATTACKMOVETO_HINT => "attack move",
        x if x == MSG_SET_RALLY_POINT_HINT => "rally point",
        _ => return format!("hint {}", h.0),
    };
    name.to_string()
}

/// The left button's press, while it is held; (#124) and the operating system's double click; (#131) the same for a
/// mouse's right and middle buttons.
#[derive(Resource, Default)]
struct Pointer {
    press: Option<Vec2>,
    double: crate::input::DoubleClickDetector,
    right_press: Option<Vec2>,
    right_double: crate::input::DoubleClickDetector,
    middle_press: Option<Vec2>,
    middle_double: crate::input::DoubleClickDetector,
}

/// The status bar's and the control bar's height, and the zoom column's width, in logical pixels: the pointer over them is
/// the UI's, not the game's.
const BAR_HEIGHT: f32 = 64.0;
const ZOOM_COLUMN: f32 = 64.0;

fn over_ui(cursor: Vec2, window: &Window, bar: f32) -> bool {
    cursor.y > window.height() - BAR_HEIGHT.max(bar) || cursor.x > window.width() - ZOOM_COLUMN || cursor.y < 28.0
}

/// ~~The game point (z up) on the ground under `cursor`: where the camera ray first meets the logic's ground height~~
/// **Corrected 2026-11-11, #138:** the ground under the pointer is [`crate::camera::Camera::ground_under`], along the
/// translated view's pick ray (Bevy's camera ray no longer stands in for `getPickRay`).
///
/// The objects' positions on screen, from the last snapshot.
fn on_screen(game: &Game, camera: &Camera, cam: &GlobalTransform) -> Vec<(ObjectId, Vec2)> {
    game.cur
        .objects
        .iter()
        .filter_map(|o| camera.world_to_viewport(cam, Vec3::from(to_bevy(o.pos))).ok().map(|p| (o.id, p)))
        .collect()
}

fn pixel(v: Vec2) -> ICoord2D {
    ICoord2D { x: v.x as i32, y: v.y as i32 }
}

/// GUI glue (needs a window): the left button and the pointer's hover ((#87) Escape is `keyboard_ui`'s), through [`Controls`].
fn pointer_input(
    mut game: NonSendMut<Game>,
    mut pointer: ResMut<Pointer>,
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cams: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    rig: Option<Res<Rig>>,
    bar: Res<UiBar>,
    radar_box: Res<RadarBox>,
    shell: NonSend<ShellState>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
) {
    let bar = bar.height + bar.panel;
    // (#4) the saves panel takes the pointer while it is open (the game is paused under it); (#78) so does the shell;
    // (#87) and the in-game menus
    if crate::saves::page::panel_open() || crate::campaigns::page::panel_open() || shell.ui.as_ref().is_some_and(|s| s.takes_input()) {
        pointer.press = None;
        return;
    }
    let (Ok(window), Ok((camera, cam))) = (windows.single(), cams.single()) else { return };
    let g = &mut *game;
    // (~~Escape cancels here~~ **Corrected 2026-11-10 (#87):** in `keyboard_ui`, which also opens the quit menu)
    // (#124: a phone has no hover; a finger's place stands for the pointer's while it touches, so its hints, the
    // tooltip and the radius cursor follow it)
    let Some(cursor) = window.cursor_position().or_else(|| touches.iter().next().map(|t| t.position())) else { return };
    // (#138) along the translated view's pick ray (`getPickRay`), which follows the view's height
    let Some(rig) = rig else { return };
    let Some(ground) = rig.cam.ground_under(pixel(cursor), |x, y| g.controls.client.world.terrain.ground_height(x, y, false).0) else { return };
    // (#124) the mouse's place for the radius cursor and the guard hint's stash (`getMouseStatus()->pos`, its
    // `screenToTerrain`, and the radar's `screenPixelToWorld` over the radar)
    g.controls.client.view.mouse_pos = pixel(cursor);
    g.controls.client.view.mouse_terrain = ground;
    g.controls.client.view.radar_world_under_mouse = radar_box.rect.filter(|r| r.contains(cursor)).and_then(|r| {
        let local = cursor - r.min;
        g.controls.radar.local_pixel_to_world(&mut g.controls.client.world, &ICoord2D { x: local.x as i32, y: local.y as i32 })
    });
    g.controls.ui.set_mouse_terrain(ground);
    let screen = on_screen(g, camera, cam);
    let picked = screen.iter().filter(|(_, p)| p.distance(cursor) < 18.0).min_by(|a, b| a.1.distance(cursor).total_cmp(&b.1.distance(cursor))).map(|(id, _)| *id);
    g.controls.client.view.on_screen = screen.iter().map(|(id, _)| *id).collect();
    // (#122) what is under the pointer for the hints: the radar (`LeftHUDInput`'s window) or an opaque window of the
    // application's; the objects under its windows and the hidden ones, for `CanSelectDrawable`
    {
        use rts_lockstep_engine::client::in_game_ui_cursor::UiWindow;
        let view = &mut g.controls.client.view;
        view.windows_under_cursor = if radar_box.contains(cursor) {
            vec![UiWindow { left_hud_input: true, see_thru: false, radar: true }]
        } else if over_ui(cursor, window, bar) {
            vec![UiWindow { left_hud_input: false, see_thru: false, radar: false }]
        } else {
            Vec::new()
        };
        view.obscured = screen.iter().filter(|(_, p)| over_ui(*p, window, bar) || radar_box.contains(*p)).map(|(id, _)| *id).collect();
        let drawn = &g.models.drawables.drawables;
        view.hidden = screen.iter().filter(|(id, _)| drawn.get(id).is_some_and(|d| d.is_drawable_effectively_hidden())).map(|(id, _)| *id).collect();
    }
    let over_ui = |c: Vec2, w: &Window, b: f32| over_ui(c, w, b) || radar_box.contains(c);
    // (#124) `KEY_STATE_ALT` for the double click's select across the map
    g.controls.alt_held = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    if buttons.just_pressed(MouseButton::Left) && !over_ui(cursor, window, bar) {
        pointer.press = Some(cursor);
        // (#124) the second press of a double click is `MSG_RAW_MOUSE_LEFT_DOUBLE_CLICK`, not a press (Windows' rule, with
        // `TheGlobalData->m_doubleClickTimeMS`)
        let limit = g.controls.client.world.statics.global_data.the_global_data().map_or(rts_lockstep_engine::common::global_data::DEFAULT_DOUBLE_CLICK_TIME_MS, |d| d.double_click_time_ms);
        if pointer.double.press(pixel(cursor), (time.elapsed_secs_f64() * 1000.0) as u32, limit) {
            g.controls.left_double_press(pixel(cursor));
        } else {
            g.controls.left_press(pixel(cursor), ground);
        }
    }
    if pointer.press.is_some() {
        g.controls.pointer_moved(pixel(cursor), ground);
    }
    // (#131) a mouse's right and middle buttons through `MetaEventTranslator` (their clicks and double clicks) and, the
    // right one, `CommandTranslator`'s bookkeeping and its click (a right click cancels a building's placement; it orders
    // in alternate mouse mode). The camera's `LookAtTranslator` has the raw messages in `camera_input`. A touch is not a
    // mouse button here: one finger is the camera's scroll only, so a tap does not cancel a placement.
    {
        use rts_lockstep_engine::common::message_types::{MSG_RAW_MOUSE_MIDDLE_BUTTON_DOWN, MSG_RAW_MOUSE_MIDDLE_BUTTON_UP, MSG_RAW_MOUSE_MIDDLE_DOUBLE_CLICK};
        let now_ms = (time.elapsed_secs_f64() * 1000.0) as u32;
        let limit = g.controls.client.world.statics.global_data.the_global_data().map_or(rts_lockstep_engine::common::global_data::DEFAULT_DOUBLE_CLICK_TIME_MS, |d| d.double_click_time_ms);
        if buttons.just_pressed(MouseButton::Right) && !over_ui(cursor, window, bar) {
            pointer.right_press = Some(cursor);
            let double = pointer.right_double.press(pixel(cursor), now_ms, limit);
            g.controls.right_press(pixel(cursor), now_ms, double);
        }
        if buttons.just_released(MouseButton::Right) && pointer.right_press.take().is_some() {
            g.controls.right_release(pixel(cursor), now_ms, picked, ground);
        }
        if buttons.just_pressed(MouseButton::Middle) && !over_ui(cursor, window, bar) {
            pointer.middle_press = Some(cursor);
            let double = pointer.middle_double.press(pixel(cursor), now_ms, limit);
            g.controls.middle_button(if double { MSG_RAW_MOUSE_MIDDLE_DOUBLE_CLICK } else { MSG_RAW_MOUSE_MIDDLE_BUTTON_DOWN }, pixel(cursor));
        }
        if buttons.just_released(MouseButton::Middle) && pointer.middle_press.take().is_some() {
            g.controls.middle_button(MSG_RAW_MOUSE_MIDDLE_BUTTON_UP, pixel(cursor));
        }
    }
    if buttons.just_released(MouseButton::Left) {
        if let Some(press) = pointer.press.take() {
            let (lo, hi) = (press.min(cursor), press.max(cursor));
            let in_box: Vec<ObjectId> = if press.distance(cursor) as i32 <= crate::input::CLICK_TOLERANCE {
                picked.into_iter().collect()
            } else {
                screen.iter().filter(|(_, p)| p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y).map(|(id, _)| *id).collect()
            };
            g.controls.left_release(pixel(press), pixel(cursor), &in_box, picked, ground);
        }
    } else {
        // (#122: over the application's windows too: the hints then show the arrow, or over the radar its cursor)
        g.controls.hover(picked, ground);
    }
}

/// GUI glue (needs a window): a ring under each selected object.
fn selection_rings(game: NonSend<Game>, mut gizmos: Gizmos) {
    for &id in game.controls.ui.all_selected_drawables() {
        let Some(o) = game.cur.find(id) else { continue };
        let r = match o.shape {
            Shape::Sphere { radius } | Shape::Cylinder { radius, .. } => radius,
            Shape::Box { half_forward, half_side, .. } => half_forward.max(half_side),
        }
        .max(3.0)
            + 2.0;
        let at = Vec3::from(to_bevy([o.pos[0], o.pos[1], o.pos[2] + 0.3]));
        gizmos.circle(Isometry3d::new(at, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), r, Color::srgb(0.3, 1.0, 0.4));
    }
}

/// (#124) GUI glue (needs a window): the radius cursor's decal on the ground (`InGameUI::m_curRadiusCursor`'s projected
/// decal, made and moved by the translated `setRadiusCursor` and `handleRadiusCursor`) as its circle laid on the terrain
/// ([`crate::cursor::decal_ring`], tested there).
fn radius_decal(game: NonSend<Game>, mut gizmos: Gizmos) {
    let Some(d) = game.radius.shown() else { return };
    let world = &game.controls.client.world;
    let (pts, c) = crate::cursor::decal_ring(d, 64, |x, y| world.terrain.ground_height(x, y, false).0);
    if c[3] == 0 {
        return;
    }
    gizmos.linestrip(pts.into_iter().map(|p| Vec3::from(to_bevy(p))), Color::srgba_u8(c[0], c[1], c[2], c[3]));
}

/// (#130) GUI glue (needs a window): the logic's own radius decals (superweapon delivery decals, the shroud's grid,
/// the gunship's reticles; made, moved, throbbed and cleared by the translated
/// `rts_lockstep_engine::client::radius_cursor::LogicRadiusDecals`) as their circles laid on the terrain, the same
/// drawing as the radius cursor's ([`crate::cursor::decal_ring`], tested there). A decal at opacity 0 (the scripts' icon
/// flag off, or a grid decal faded out) is not drawn.
fn logic_decals(game: NonSend<Game>, mut gizmos: Gizmos) {
    let world = &game.controls.client.world;
    for d in game.logic_decals.shown() {
        let (pts, c) = crate::cursor::decal_ring(d, 64, |x, y| world.terrain.ground_height(x, y, false).0);
        if c[3] == 0 {
            continue;
        }
        gizmos.linestrip(pts.into_iter().map(|p| Vec3::from(to_bevy(p))), Color::srgba_u8(c[0], c[1], c[2], c[3]));
    }
}

/// (#124) The world's tooltip root and the shapes it shows.
#[derive(Component)]
struct WorldTooltip(Vec<crate::cursor::TooltipShape>);

/// (#124) GUI glue (needs a window): the mouse's tooltip over the world (`W3DMouse::draw`'s `drawTooltip`, translated;
/// its shapes from [`crate::cursor::tooltip_shapes`], tested there), as UI nodes over everything, rebuilt when its shapes
/// change: the box, its border, and each text in a node cut at its clip region, the text placed at its own corner. It
/// takes no pointer. At a phone's 390 pixels the box is moved left of the finger at the right edge (upstream's rule,
/// `xPos -= 20 + width`) and wraps at `TooltipWidth` of the screen (15 %: 58 pixels, so a long name takes several lines).
/// **(#136)** The mouse is the one `TheMouse`: the shell's and the control bar's windows set its tooltip too (their
/// managers borrow it, `with_mouse`), so it is drawn over the shell as well: ~~`ZIndex(50)`, under the shell's
/// `GlobalZIndex(10)`~~ **corrected 2026-11-11, #136:** `GlobalZIndex(20)`, over the shell's 10.
fn world_tooltip(mut commands: Commands, time: Res<Time>, mut game: NonSendMut<Game>, mut roots: Query<(Entity, &mut WorldTooltip)>) {
    let g = &mut *game;
    let now = (time.elapsed_secs_f64() * 1000.0) as u32;
    let fade = g.controls.client.world.script_engine.fade != rts_lockstep_engine::logic::world::script_engine::fade::NONE;
    let shapes = crate::cursor::tooltip_shapes(&mut g.pause.mouse, now, fade);
    let root = match roots.iter_mut().next() {
        Some((e, mut r)) => {
            if r.0 == shapes {
                return;
            }
            r.0 = shapes.clone();
            commands.entity(e).despawn_children();
            e
        }
        None => commands
            .spawn((WorldTooltip(shapes.clone()), Node { position_type: PositionType::Absolute, left: px(0), top: px(0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, Pickable::IGNORE, GlobalZIndex(20)))
            .id(),
    };
    for sh in &shapes {
        let child = match sh {
            crate::cursor::TooltipShape::Rect { rect, color } => commands
                .spawn((
                    Node { position_type: PositionType::Absolute, left: px(rect[0]), top: px(rect[1]), width: px(rect[2] - rect[0]), height: px(rect[3] - rect[1]), ..default() },
                    BackgroundColor(Color::srgba_u8(color[0], color[1], color[2], color[3])),
                    Pickable::IGNORE,
                ))
                .id(),
            crate::cursor::TooltipShape::Text { text, rect, clip, color, font_px } => {
                let clip_node = Node { position_type: PositionType::Absolute, left: px(clip[0]), top: px(clip[1]), width: px(clip[2] - clip[0]), height: px(clip[3] - clip[1]), overflow: Overflow::clip(), ..default() };
                let text_node = Node { position_type: PositionType::Absolute, left: px(rect[0] - clip[0]), top: px(rect[1] - clip[1]), width: px(rect[2] - rect[0]), ..default() };
                let colour = Color::srgba_u8(color[0], color[1], color[2], color[3]);
                let font = TextFont { font_size: bevy::text::FontSize::Px(*font_px), ..default() };
                commands
                    .spawn((clip_node, Pickable::IGNORE))
                    .with_children(|p| {
                        p.spawn((text_node, Text::new(text.clone()), font, TextColor(colour), Pickable::IGNORE));
                    })
                    .id()
            }
        };
        commands.entity(root).add_child(child);
    }
}

#[derive(Component)]
struct DragBox;

/// GUI glue (needs a window): the drag box, while the left button is held and moved. (#116: ~~while the pointer is
/// past the click tolerance~~ while the selection translator's drag is under way, `m_dragSelecting`, which its input gate
/// ends: `Controls::frame_tick`, tested in `input`.)
fn drag_box(game: NonSend<Game>, pointer: Res<Pointer>, windows: Query<&Window>, mut boxes: Query<(&mut Node, &mut Visibility), With<DragBox>>) {
    let Ok(window) = windows.single() else { return };
    let dragging = game.controls.selection.drag_selecting;
    for (mut node, mut vis) in &mut boxes {
        match (pointer.press, window.cursor_position()) {
            (Some(a), Some(b)) if dragging && a.distance(b) as i32 > crate::input::CLICK_TOLERANCE => {
                let (lo, hi) = (a.min(b), a.max(b));
                node.left = px(lo.x);
                node.top = px(lo.y);
                node.width = px(hi.x - lo.x);
                node.height = px(hi.y - lo.y);
                *vis = Visibility::Visible;
            }
            _ => *vis = Visibility::Hidden,
        }
    }
}

#[derive(Component)]
struct ControlBar {
    /// The selection the bar was built for (`getFrameSelectionChanged`).
    shown: u32,
}

/// GUI glue (needs a window): the first selected unit's command buttons, rebuilt when the selection changes.
fn control_bar(
    mut commands: Commands,
    mut game: NonSendMut<Game>,
    mut bars: Query<(Entity, &mut ControlBar, &mut Node, &mut Visibility)>,
    windows: Query<&Window>,
    mut ui_bar: ResMut<UiBar>,
) {
    let changed = game.controls.ui.frame_selection_changed();
    // (#12) with the game's control bar the debug panel is for narrow screens only (big buttons a finger can press),
    // above the bar
    let narrow = windows.single().map_or(true, |w| w.width() < 700.0);
    let shown = game.ui.is_none() || narrow;
    ui_bar.panel = if shown && game.ui.is_some() { 56.0 } else { 0.0 };
    for (e, mut bar, mut node, mut vis) in &mut bars {
        *vis = if shown { Visibility::Inherited } else { Visibility::Hidden };
        node.bottom = px(8.0 + ui_bar.height);
        if bar.shown == changed {
            continue;
        }
        bar.shown = changed;
        commands.entity(e).despawn_children();
        for b in game.controls.bar_buttons() {
            // (#12) the button's text label through the game's text when there is one, else its name
            let label = game.controls.bar_label(&b).unwrap_or_else(|| b.label.trim_start_matches("Command_").to_string());
            let button = b.button.clone();
            let child = commands
                .spawn((
                    bar_button(&label),
                    observe(move |_: On<Activate>, mut game: NonSendMut<Game>| {
                        game.controls.press(&button);
                    }),
                ))
                .id();
            commands.entity(e).add_child(child);
        }
    }
}

/// The zoom, reset and pause buttons' column.
#[derive(Component)]
struct ZoomColumn;

/// (#144) One of the letterbox's bars on screen.
#[derive(Component)]
struct LetterBoxBar;

/// (#144) GUI glue (needs a window; the bars and their fade are `rts_lockstep_engine::client::letterbox`, tested there and
/// against the C++): `W3DDisplay::draw`'s `renderLetterBox(now)`, once a rendered frame, on the window as the display
/// (`m_width`, `m_height` in logical pixels): each `drawFillRect` a black node with the fade as alpha. Upstream draws it
/// after the views, the in-game UI's windows and the mouse, so it is over all of them here too (`GlobalZIndex(21)`, over
/// the world tooltip's 20 and the shell's 10); it takes no input, as a draw does not. New: the client's own buttons
/// (the zoom column, which a phone needs) stay over it while it is drawn (`GlobalZIndex(22)`), so a cinematic does not
/// hide them; upstream has no such buttons.
fn letterbox_bars(mut commands: Commands, mut game: NonSendMut<Game>, time: Res<Time>, windows: Query<&Window>, bars: Query<Entity, With<LetterBoxBar>>, cols: Query<Entity, With<ZoomColumn>>) {
    let Ok(win) = windows.single() else { return };
    let now_ms = (time.elapsed_secs_f64() * 1000.0) as u32;
    let rects = game.letterbox.render_letter_box(now_ms, win.width() as i32, win.height() as i32);
    for e in &bars {
        commands.entity(e).despawn();
    }
    let mut drawn = false;
    for r in rects {
        // (a display wider than 16:9 gives a negative size: nothing to draw)
        if r.width <= 0 || r.height <= 0 {
            continue;
        }
        let alpha = (r.color >> 24) as f32 / 255.0;
        commands.spawn((
            LetterBoxBar,
            Node { position_type: PositionType::Absolute, left: px(r.x as f32), top: px(r.y as f32), width: px(r.width as f32), height: px(r.height as f32), ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, alpha)),
            Pickable::IGNORE,
            GlobalZIndex(21),
        ));
        drawn = true;
    }
    for e in &cols {
        if drawn {
            commands.entity(e).insert(GlobalZIndex(22));
        } else {
            commands.entity(e).remove::<GlobalZIndex>();
        }
    }
}

/// GUI glue (needs a window): the zoom column stays above the game's control bar (#12).
fn lift_zoom(bar: Res<UiBar>, mut cols: Query<&mut Node, With<ZoomColumn>>) {
    for mut n in &mut cols {
        n.bottom = px(8.0 + bar.height);
    }
}

/// The game's control bar on screen (#12): its height in logical pixels, and the debug panel's above it.
#[derive(Resource, Default)]
struct UiBar {
    height: f32,
    panel: f32,
    /// (#35) Where the bar's layout is on screen (for the radar's window), while the bar is drawn.
    placement: Option<crate::ui::LayoutPlacement>,
}

/// The game's control bar's root node, and the quads it was last drawn with.
#[derive(Component)]
struct GameUiRoot(Vec<crate::ui::UiQuad>);

/// GUI glue (needs the app): reads the game's control bar from the art's files once a game is running
/// ([`crate::ui::load_game_ui`], tested there); while a file is being fetched (the web) it tries again on a later frame.
/// The FX lists and particle systems from the art (#43, #44), once its files have arrived (`WorldDrawables::load_fx`).
/// GUI glue (needs the app): the audio's tables from the art's INI files once they have arrived (#13,
/// `crate::audio::load_tables`, tested through the engine's `AudioTables`), then each frame the device's files read
/// and decoded and its voices started and ended (`ClientDevice::pump`, tested there).
fn update_audio(mut game: NonSendMut<Game>, mut shell: NonSendMut<ShellState>, time: Res<Time>, rig: Option<Res<Rig>>) {
    let g = &mut *game;
    let Some(art) = &g.models.art else {
        // (#136: no art, no audio: the windows' clicks are dropped)
        g.gui_sounds.clear();
        return;
    };
    let open = |p: &[u8]| {
        if !art.fs.file_ready(p) {
            return None;
        }
        Some(art.fs.open_file(p))
    };
    if g.audio_pending {
        // #55: `Eva::init`'s `Data\INI\Eva.ini` too (`None`: still arriving)
        let eva_ini = open(rts_lockstep_engine::client::eva::EVA_INI_FILE);
        // (#88) `parseAudioSettingsDefinition` reads the volumes, provider and speakers from `Options.ini`
        let prefs = rts_lockstep_engine::client::options_menu::OptionPreferences::load_from(&mut rts_lockstep_engine::client::user_preferences::MemoryUserData {
            path: Vec::new(),
            files: shell.user_data().files.iter().map(|(n, b)| (n.as_bytes().to_vec(), b.clone())).collect(),
        })
        .audio_prefs();
        if let (Some(eva_ini), Some((tables, errors))) = (eva_ini, crate::audio::load_tables(open, prefs)) {
            g.audio_pending = false;
            info!("audio: {} events, {} music tracks, {} errors {errors:?}", tables.infos.len(), tables.music_tracks.len(), errors.len());
            let mut manager = rts_lockstep_engine::client::game_audio::AudioManager::new(tables);
            manager.random = rts_lockstep_engine::common::random_value::RandomValues::new(0);
            let mut audio = rts_lockstep_engine::client::client_audio::ClientAudio::new(manager);
            if let Some(text) = eva_ini {
                if let Err(e) = audio.eva.load_text(rts_lockstep_engine::client::eva::EVA_INI_FILE, &text) {
                    info!("EVA: Eva.ini did not parse: {e:?}");
                }
            }
            info!("EVA: {} messages described", audio.eva.all_check_infos.len());
            g.audio = Some(Box::new(GameSound { audio, device: crate::audio::ClientDevice::new(SoundOutput::default()) }));
        }
    }
    let Some(sound) = g.audio.as_deref_mut() else {
        // (#136: no audio yet, no sound for the windows' clicks)
        g.gui_sounds.clear();
        return;
    };
    // (#136) the window code's sounds (`AudioEventRTS buttonClick("GUIClick"); TheAudio->addAudioEvent(&buttonClick)`);
    // while the game is paused (the shell and the menus are over it) no logic frame runs the manager's update, so it runs
    // here, as upstream's `TheAudio->update()` runs every client frame
    let clicks = std::mem::take(&mut g.gui_sounds);
    if !clicks.is_empty() || g.paused {
        let files = |p: &[u8]| art.fs.does_file_exist(p);
        let host = rts_lockstep_engine::client::client_audio::WorldAudioHost {
            world: &g.controls.client.world,
            local_player_index: g.models.drawables.local_player_index,
            files: &files,
            language: rts_lockstep_engine::logic::world::install_world::DEFAULT_LANGUAGE,
        };
        for c in &clicks {
            sound.audio.play_gui_sound(&host, c);
        }
        if g.paused {
            let view = rig.as_deref().map_or_else(Default::default, |r| r.cam.listener());
            sound.audio.manager.update(&host, &mut sound.device, &view);
        }
    }
    sound.device.pump(|p| {
        if !art.fs.file_ready(p) {
            return crate::audio::Fetch::Waiting;
        }
        match art.fs.open_file(p) {
            Some(b) => crate::audio::Fetch::Bytes(b),
            None => crate::audio::Fetch::Missing,
        }
    });
    crate::audio::AudioOutput::advance(&mut sound.device.out, time.delta_secs_f64());
}

fn load_fx(mut game: NonSendMut<Game>) {
    if game.fx_pending && game.models.load_fx() {
        game.fx_pending = false;
        let fx = &game.models.drawables.fx;
        info!("effects: {} particle system templates, {} FX lists, {} errors", fx.particles.templates.len(), fx.fx_lists.fxmap.len(), fx.errors.len());
    }
}

/// Each particle batch's entity and mesh, by texture and blend, and the particle textures by name (`None`: missing).
#[derive(Resource, Default)]
struct ParticleDraw {
    batches: HashMap<(Vec<u8>, crate::particles::Blend), (Entity, Handle<Mesh>)>,
    textures: HashMap<Vec<u8>, Option<Handle<Image>>>,
}

/// GUI glue (needs a window): the particles (#43, #44) as one mesh per texture and blend (`crate::particles`, tested
/// there), facing the camera, rebuilt each frame; at most `MAX_CLIENT_PARTICLES` quads, so the work per frame is bounded.
#[allow(clippy::too_many_arguments)]
fn draw_particles(
    mut commands: Commands,
    mut game: NonSendMut<Game>,
    mut pd: ResMut<ParticleDraw>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
    mut vis: Query<&mut Visibility>,
) {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    let Ok(cam) = cams.single() else { return };
    let (right, up) = (cam.right(), cam.up());
    let g = &mut *game;
    let mut quads = crate::particles::particle_quads(&g.models.drawables.fx.particles, right.to_array(), up.to_array());
    // the W3D models' emitters (#49), in the same batches
    let w3d = crate::particles::emitter_quads(g.models.instances(), crate::particles::point_tables(), right.to_array(), up.to_array());
    crate::particles::merge_batches(&mut quads, w3d);
    // the batches no particle uses this frame are hidden, not freed
    for (key, (e, _)) in &pd.batches {
        if !quads.contains_key(key) {
            if let Ok(mut v) = vis.get_mut(*e) {
                *v = Visibility::Hidden;
            }
        }
    }
    for (key, b) in quads {
        // the texture, once (`None` while missing; a read still being fetched is tried again on a later frame)
        if !pd.textures.contains_key(&key.0) && !key.0.is_empty() {
            if let Some(art) = &g.models.art {
                let before = art.fs.local.device.unserved_reads();
                let r = rts_lockstep_engine::formats::w3d::render::load_texture_image(&art.assets.file_factory, &art.fs, &key.0);
                if art.fs.local.device.unserved_reads() == before {
                    let h = match r {
                        Ok(Ok(t)) => Some(images.add(crate::model::texture_image(&t, Some((true, true))))),
                        _ => None,
                    };
                    pd.textures.insert(key.0.clone(), h);
                }
            } else {
                pd.textures.insert(key.0.clone(), None);
            }
        }
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        let n = b.positions.len();
        let normal = (-cam.forward()).to_array();
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, b.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![normal; n]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, b.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, b.colors);
        mesh.insert_indices(Indices::U32(b.indices));
        match pd.batches.get(&key) {
            Some((e, h)) => {
                if let Some(mut m) = meshes.get_mut(h) {
                    *m = mesh;
                }
                if let Ok(mut v) = vis.get_mut(*e) {
                    *v = Visibility::Visible;
                }
            }
            None => {
                if !key.0.is_empty() && !pd.textures.contains_key(&key.0) {
                    // its texture is still being fetched: drawn once it has arrived
                    continue;
                }
                let texture = pd.textures.get(&key.0).cloned().flatten();
                let alpha_mode = match key.1 {
                    crate::particles::Blend::Add => AlphaMode::Add,
                    crate::particles::Blend::Alpha => AlphaMode::Blend,
                };
                let material = materials.add(StandardMaterial { base_color_texture: texture, unlit: true, alpha_mode, double_sided: true, cull_mode: None, ..default() });
                let h = meshes.add(mesh);
                let e = commands.spawn((Mesh3d(h.clone()), MeshMaterial3d(material), Transform::IDENTITY, bevy::camera::visibility::NoFrustumCulling)).id();
                pd.batches.insert(key, (e, h));
            }
        }
    }
}

fn load_game_ui(mut game: NonSendMut<Game>, shell: NonSend<ShellState>) {
    let g = &mut *game;
    if !g.ui_pending {
        return;
    }
    let Some(art) = &g.models.art else {
        g.ui_pending = false;
        return;
    };
    // (#78) the mapped images are the shell's, read first ([`load_shell`]), so that both share one collection
    let shared = match &shell.images {
        None => return,
        Some(Ok(i)) => Some(i.clone()),
        Some(Err(_)) => None,
    };
    let world = &g.controls.client.world;
    let side = world.players.get_nth(g.controls.client.player).map(|p| p.side.clone()).unwrap_or_default();
    match crate::ui::load_game_ui_with(&art.fs, &world.statics.game_text, &side, shared) {
        crate::ui::UiBuild::Ready(mut ui) => {
            // (#144) `ControlBar::init`'s `findCommandButton("NonCommand_...")`: the World's buttons
            let client = &g.controls.client;
            ui.init_tooltips(|name| {
                let i = client.world.cuts.command_buttons.iter().position(|b| b.name == name)?;
                rts_lockstep_engine::client::control_bar_command::ControlBarStateHost::button(client, i).map(|b| (i, b))
            });
            info!("control bar: {} windows, {} images", ui.wm.tree.windows.len(), ui.images.len());
            g.ui_status = format!("control bar: {} windows", ui.wm.tree.windows.len());
            g.ui = Some(ui);
            g.ui_pending = false;
        }
        crate::ui::UiBuild::Waiting => {}
        crate::ui::UiBuild::Missing(why) => {
            warn!("no control bar: {why}");
            g.ui_status = format!("debug panel ({why})");
            g.ui_pending = false;
        }
    }
}

/// A mapped image's texture and the rectangle of it, in texels: `None` while missing (or still being fetched).
fn ui_image(g: &mut Game, images: &mut Assets<Image>, name: &[u8]) -> Option<(Handle<Image>, Rect)> {
    let collection = g.ui.as_ref()?.images.clone();
    mapped_texture(g.models.art.as_ref()?, &mut g.ui_textures, &collection, images, name)
}

/// The UI's textures by file name (`None`: missing), with their sizes.
type UiTextures = HashMap<Vec<u8>, Option<(Handle<Image>, (u32, u32))>>;

/// A mapped image of `collection` as a texture of the art's (made once into `cache`) and its rectangle: the control bar's
/// and (#78) the shell's. GUI glue (needs the app's image assets).
fn mapped_texture(art: &Art<ArtDevice>, cache: &mut UiTextures, collection: &rts_lockstep_engine::client::image::ImageCollection, images: &mut Assets<Image>, name: &[u8]) -> Option<(Handle<Image>, Rect)> {
    let img = collection.find_image_by_name(name)?.clone();
    if !cache.contains_key(&img.filename) {
        let before = art.fs.local.device.unserved_reads();
        let r = rts_lockstep_engine::formats::w3d::render::load_texture_image(&art.assets.file_factory, &art.fs, &img.filename);
        if art.fs.local.device.unserved_reads() != before {
            return None;
        }
        let entry = match r {
            Ok(Ok(t)) => Some((images.add(crate::model::texture_image(&t, Some((true, true)))), (t.width, t.height))),
            _ => None,
        };
        cache.insert(img.filename.clone(), entry);
    }
    let (h, (w, ht)) = cache.get(&img.filename)?.clone()?;
    // (the UVs are the pixel edges over the texture size the INI gives; Coords before TextureWidth stay pixels)
    let to_px = |u: f32, size: u32, given: i32| if given != 0 { u * size as f32 } else { u };
    // (inset half a texel, so the linear filter does not bleed the neighbouring image of the page in)
    let rect = Rect::new(
        to_px(img.uv[0], w, img.texture_size.0) + 0.5,
        to_px(img.uv[1], ht, img.texture_size.1) + 0.5,
        to_px(img.uv[2], w, img.texture_size.0) - 0.5,
        to_px(img.uv[3], ht, img.texture_size.1) - 0.5,
    );
    Some((h, rect))
}

/// GUI glue (needs a window): the game's control bar drawn from [`crate::ui::GameUi::frame`]'s quads (the selected
/// object's buttons from `populateCommand`, the money, the power, the build queue), as Bevy UI nodes in a box of the
/// layout's 800-wide proportions along the bottom of the screen, as wide as the screen allows; rebuilt when the quads
/// change. ~~A command button presses through [`Controls::press`] (`processCommandUI`).~~ **Corrected 2026-11-10 (#34):**
/// the pointer over the bar goes through the window manager ([`crate::ui::GameUi::pointer`], tested there: the root's
/// pointer observers turn the screen position into the layout's); the button it selects presses through
/// [`Controls::press`] (`processCommandUI`). A release that happened off the bar ends the manager's grab here. (#109) The
/// bar's quit menu call ([`crate::ui::GameUi::take_quit_menu_call`]) is made on the shell's quit menu
/// ([`crate::shell::ShellUi::bar_quit_menu_call`], tested there).
#[allow(clippy::too_many_arguments)]
fn game_ui(
    mut commands: Commands,
    mut game: NonSendMut<Game>,
    mut images: ResMut<Assets<Image>>,
    mut roots: Query<(Entity, &mut GameUiRoot, &mut Node)>,
    windows: Query<&Window>,
    mut ui_bar: ResMut<UiBar>,
    buttons: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    mut shell: NonSendMut<ShellState>,
    time: Res<Time>,
) {
    let g = &mut *game;
    let popup_world = popup_world(g, &time);
    // (#109) the bar's options button (`ControlBarSystem`'s `ToggleQuitMenu`, `HideQuitMenu`): the shell's quit menu,
    // as Escape opens it (`keyboard_ui`); the phone's way in
    if let (Some(call), Some(s)) = (g.ui.as_mut().and_then(|u| u.take_quit_menu_call()), shell.ui.as_mut()) {
        let c = &g.controls;
        s.bar_quit_menu_call(call, || crate::shell::quit_menu_game(&c.client.world, c.ui.input_enabled()));
    }
    // (#136) `MSG_META_TOGGLE_CONTROL_BAR`'s `ToggleControlBar()` calls (the key's, or the phone column's `B`), on the
    // game's bar; the stages hold still during a replay's playback. Without the game's files there is no bar: the
    // calls are dropped.
    let toggles = std::mem::take(&mut g.controls.client.control_bar_toggles);
    if let Some(ui) = g.ui.as_mut() {
        ui.recorder_playback = rts_lockstep_engine::client::control_bar_stage::ToggleControlBarHost::toggle_bar_recorder_playback(&g.controls.client);
        for immediate in toggles {
            ui.toggle_control_bar(immediate);
        }
    }
    // (#131) the hot keys' `GUIClick` / `GUIClickDisabled` ~~are cleared: the client plays no GUI sound yet~~
    // **corrected 2026-11-11, #136:** played (`update_audio`), with the bar's windows' (its push buttons' `GUIClick`)
    g.gui_sounds.append(&mut g.controls.gui_sounds);
    if let Some(ui) = g.ui.as_mut() {
        g.gui_sounds.extend(ui.take_gui_sounds());
    }
    // (#78) the shell, while shown, is drawn in place of the bar (upstream's control bar is hidden in the shell)
    let shell_up = shell.ui.as_ref().is_some_and(|s| s.is_active());
    for (_, _, mut node) in roots.iter_mut() {
        node.display = if shell_up { Display::None } else { Display::Flex };
    }
    if g.ui.is_none() || shell_up {
        ui_bar.height = 0.0;
        ui_bar.placement = None;
        return;
    }
    let c = &mut g.controls;
    let sel = c.ui.first_selected_drawable().filter(|&o| rts_lockstep_engine::client::selection_info::SelectionInfoHost::is_locally_controlled(&c.client, o) && c.ui.select_count() == 1);
    let queue = sel.map(|o| c.client.production_queue(o)).unwrap_or_default();
    let (money, power) = c.client.money_and_power();
    let state = crate::ui::GameUi::bar_state(sel, &mut c.client, queue, money, power);
    // (#35) the popups of this selection's buttons, by button (#138: ~~a new selection shows none~~ the popup is the
    // bar's, which shows what `populateBuildTooltipLayout` was last given)
    g.tooltip_lines = state.commands.iter().zip(&state.tooltips).filter_map(|(c, t)| Some((c.as_ref()?.button, crate::ui::tooltip_lines(t.as_ref()?)))).collect();
    // (#144) and the non-command buttons' (`ControlBar::init`'s `NonCommand_*`)
    if let Some(ui) = g.ui.as_ref() {
        let more = ui.non_command_tooltips(&mut g.controls.client);
        g.tooltip_lines.extend(more);
    }
    let ui = g.ui.as_mut().expect("checked");
    // (#34) the windows as the state sets them; a grab whose release was not over the bar ends
    ui.sync(&state);
    // (#131) `TheHotKeyManager` for this context: the hot keys the command windows' buttons name, on those windows
    let hot_windows: Vec<Option<crate::input::HotKeyWindow>> = ui
        .command_window_status()
        .into_iter()
        .enumerate()
        .map(|(slot, st)| {
            let (hidden, enabled) = st?;
            let button = state.commands.get(slot).cloned().flatten().map_or(usize::MAX, |c| c.button);
            Some(crate::input::HotKeyWindow { hidden, enabled, button })
        })
        .collect();
    g.controls.set_bar_hot_keys(&state.hot_keys, hot_windows);
    if ui.wm.grab_window.is_some() && !buttons.pressed(MouseButton::Left) && touches.iter().next().is_none() {
        let _ = ui.with_mouse(&mut g.pause.mouse.tooltip, |ui| ui.pointer(rts_lockstep_engine::client::window_manager::gwm::LEFT_UP, (-1.0, -1.0), false, &popup_world));
    }
    // (#138) `ControlBar::update`'s popup part, then the frame's mouse event over the bar (upstream's
    // `MSG_RAW_MOUSE_POSITION` every frame, through `winProcessMouseEvent`, which calls `commandButtonTooltip`); a new
    // touch elsewhere ends a finger's pointer (a mouse's ends when it leaves the bar)
    ui.popup_frame(&popup_world);
    if touches.iter_just_pressed().next().is_some() && g.bar_pointer.is_some_and(|(_, t)| t) && !touches.iter_just_pressed().any(|t| ui_bar.placement.is_some_and(|p| bar_contains(&p, ui_bar.height, windows.single().map_or(600.0, |w| w.height()), t.position()))) {
        g.bar_pointer = None;
    }
    if let (Some((at, touch)), Some(p)) = (g.bar_pointer, ui_bar.placement) {
        let pos = p.to_layout((at.x, at.y));
        let _ = ui.with_mouse(&mut g.pause.mouse.tooltip, |ui| ui.pointer(rts_lockstep_engine::client::window_manager::gwm::MOUSE_POS, pos, touch, &popup_world));
    }
    // (#144) a `populateBuildTooltipLayout(NULL, window)` since the last frame fills the popup's text from the World,
    // once; an unknown window leaves it as it was (upstream's `return`)
    if ui.popup_fills != g.popup_fills_seen {
        g.popup_fills_seen = ui.popup_fills;
        if let Some((None, Some(w))) = ui.popup {
            if let Some(t) = ui.window_popup(w, &g.controls.client) {
                g.window_popup_lines = Some(crate::ui::tooltip_lines(&t));
            }
        }
    }
    let ui = g.ui.as_ref().expect("checked");
    let quads = ui.frame(&state);
    g.bar_buttons = state.commands.iter().map(|s| s.as_ref().map(|s| s.button)).collect();
    // the box: the quads' extent, the layout's full width; (#136) cut at the layout's bottom (the low stage moves the bar
    // down past it, its top tenth showing)
    let (y0, y1) = quads.iter().fold((f32::MAX, f32::MIN), |(a, b), q| (a.min(q.rect[1]), b.max(q.rect[3])));
    let y1 = y1.min(crate::ui::LAYOUT.1 as f32);
    if quads.is_empty() || y1 <= y0 {
        // (#136) the bar hidden (`ControlBarParent`): nothing drawn, nothing taken from the screen
        ui_bar.height = 0.0;
        ui_bar.placement = None;
        for (_, _, mut node) in roots.iter_mut() {
            node.display = Display::None;
        }
        return;
    }
    let (lw, bh) = (crate::ui::LAYOUT.0 as f32, y1 - y0);
    let (ww, wh) = windows.single().map_or((800.0, 600.0), |w| (w.width(), w.height()));
    let width = ww.min(wh * lw / crate::ui::LAYOUT.1 as f32);
    ui_bar.height = width * bh / lw;
    ui_bar.placement = Some(crate::ui::LayoutPlacement { left: (ww - width) / 2.0, top: wh - ui_bar.height, y0, scale: width / lw });
    let root = match roots.iter_mut().next() {
        Some((e, mut r, mut node)) => {
            node.width = px(width);
            node.height = px(ui_bar.height);
            node.left = px((ww - width) / 2.0);
            if r.0 == quads {
                return;
            }
            r.0 = quads.clone();
            commands.entity(e).despawn_children();
            e
        }
        None => {
            let mut root = commands.spawn((
                GameUiRoot(quads.clone()),
                Node { position_type: PositionType::Absolute, bottom: px(0), left: px((ww - width) / 2.0), width: px(width), height: px(ui_bar.height), overflow: Overflow::clip(), ..default() },
            ));
            // (#34) the pointer over the bar, into the window manager (events bubble up from the quads)
            root.observe(|ev: On<PointerMove>, time: Res<Time>, bar: Res<UiBar>, mut game: NonSendMut<Game>| {
                bar_pointer(&mut *game, &bar, rts_lockstep_engine::client::window_manager::gwm::MOUSE_POS, ev.pointer.position, &ev.pointer.id, &time);
            });
            root.observe(|ev: On<PointerLeave>, time: Res<Time>, bar: Res<UiBar>, mut game: NonSendMut<Game>| {
                bar_pointer(&mut *game, &bar, rts_lockstep_engine::client::window_manager::gwm::MOUSE_POS, ev.pointer.position, &ev.pointer.id, &time);
                // (#138) a mouse that left the bar sends it no more frames' events; a finger's place stays (a phone's
                // pointer stays where it lifted)
                if !matches!(ev.pointer.id, bevy::picking::pointer::PointerId::Touch(_)) {
                    game.bar_pointer = None;
                }
            });
            root.observe(|ev: On<PointerPress>, time: Res<Time>, bar: Res<UiBar>, mut game: NonSendMut<Game>| {
                if ev.button == PointerButton::Primary {
                    game.touch_pressed = matches!(ev.pointer.id, bevy::picking::pointer::PointerId::Touch(_));
                    bar_pointer(&mut *game, &bar, rts_lockstep_engine::client::window_manager::gwm::LEFT_DOWN, ev.pointer.position, &ev.pointer.id, &time);
                }
            });
            root.observe(|ev: On<PointerRelease>, time: Res<Time>, bar: Res<UiBar>, mut game: NonSendMut<Game>| {
                if ev.button == PointerButton::Primary {
                    bar_pointer(&mut *game, &bar, rts_lockstep_engine::client::window_manager::gwm::LEFT_UP, ev.pointer.position, &ev.pointer.id, &time);
                }
            });
            root.id()
        }
    };
    let frame = [0.0, y0, lw, y1];
    let scale = width / lw;
    spawn_quads(&mut commands, root, &quads, frame, scale, false, |n| ui_image(g, &mut images, n));
}

/// GUI glue (needs a window): `quads` as Bevy UI nodes, children of `root`, in percentages of `frame` (the layout's
/// rectangle the root covers), the first underneath; a text's size scaled by `scale` (screen pixels per layout pixel).
/// The control bar's drawing (#12, #35) and (#78) the shell's ([`crate::ui::draw_list_quads`]'s quads: tints, cut images,
/// placed and coloured text, turned lines).
fn spawn_quads(commands: &mut Commands, root: Entity, quads: &[crate::ui::UiQuad], frame: [f32; 4], scale: f32, ignore_picks: bool, mut image: impl FnMut(&[u8]) -> Option<(Handle<Image>, Rect)>) {
    // (#78) the shell's quads are rebuilt as it changes (a press redraws its button), so they take no pointer: the root,
    // which stays, gets every event (a release aimed at a quad despawned under it was lost)
    let pick = if ignore_picks { Pickable::IGNORE } else { Pickable::default() };
    let (fw, fh) = (frame[2] - frame[0], frame[3] - frame[1]);
    let pct = |v: f32, total: f32| Val::Percent(v / total * 100.0);
    let rgba = |c: [u8; 4], dim: bool| {
        let k = |v: u8| if dim { (v as u32 * 45 / 100) as u8 } else { v };
        Color::srgba_u8(k(c[0]), k(c[1]), k(c[2]), c[3])
    };
    for q in quads {
        let corner = q.text_top_left;
        let node = Node {
            position_type: PositionType::Absolute,
            left: pct(q.rect[0] - frame[0], fw),
            top: pct(q.rect[1] - frame[1], fh),
            width: pct(q.rect[2] - q.rect[0], fw),
            height: pct(q.rect[3] - q.rect[1], fh),
            justify_content: if corner { JustifyContent::Start } else { JustifyContent::Center },
            align_items: if corner { AlignItems::Start } else { AlignItems::Center },
            overflow: if corner { Overflow::clip() } else { Overflow::default() },
            ..default()
        };
        let mut e = commands.spawn((node, pick));
        if let Some((sin, cos)) = q.rotation {
            e.insert(UiTransform::from_rotation(Rot2::from_sin_cos(sin, cos)));
        }
        match q.image.as_ref().and_then(|n| image(n)) {
            Some((h, rect)) => {
                let rect = match q.uv {
                    Some([u0, v0, u1, v1]) => Rect::new(rect.min.x + u0 * rect.width(), rect.min.y + v0 * rect.height(), rect.min.x + u1 * rect.width(), rect.min.y + v1 * rect.height()),
                    None => rect,
                };
                let tint = rgba(q.tint.unwrap_or([255, 255, 255, 255]), q.dim);
                e.insert(ImageNode { image: h, rect: Some(rect), color: tint, image_mode: bevy::ui::widget::NodeImageMode::Stretch, ..default() });
            }
            None => {
                if let Some(c) = q.color {
                    e.insert(BackgroundColor(rgba(c, false)));
                } else if q.command.is_some() {
                    e.insert(BackgroundColor(Color::srgba(0.12, 0.16, 0.22, 0.9)));
                }
            }
        }
        if let Some(t) = &q.text {
            let size = q.font_px.map_or((scale * 11.0).clamp(8.0, 18.0), |f| (f * scale).max(6.0));
            let colour = q.text_color.map_or(Color::WHITE, |c| rgba(c, false));
            let layout = if corner && !q.text_wrap { TextLayout { linebreak: bevy::text::LineBreak::NoWrap, ..default() } } else { TextLayout::default() };
            e.with_children(|p| {
                p.spawn((Text::new(t.clone()), TextFont { font_size: bevy::text::FontSize::Px(size), ..default() }, TextColor(colour), layout, pick));
            });
        }
        if let Some((h, rect)) = q.overlay.as_ref().and_then(|n| image(n)) {
            // (#35) the portrait's veterancy chevron over it
            e.with_children(|p| {
                p.spawn((
                    Node { position_type: PositionType::Absolute, left: px(0), top: px(0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                    ImageNode { image: h, rect: Some(rect), image_mode: bevy::ui::widget::NodeImageMode::Stretch, ..default() },
                    pick,
                ));
            });
        }
        if let Some(pct_done) = q.clock {
            let rest = (100 - pct_done.clamp(0, 100)) as f32;
            e.with_children(|p| {
                p.spawn((
                    Node { position_type: PositionType::Absolute, left: px(0), top: px(0), width: Val::Percent(100.0), height: Val::Percent(rest), ..default() },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                    pick,
                ));
            });
        }
        if q.command.is_some() {
            // (#34) pickable, so the press is the bar's and not the world's; the manager decides what it presses
            e.insert(Button);
        }
        let child = e.id();
        commands.entity(root).add_child(child);
    }
}

/// (#138) What the bar's popup reads outside it this frame: the clock (`timeGetTime`), the World's game ending
/// (`TheScriptEngine->isGameEnding()`) and replay game, the quit menu. GUI glue: the parts are tested where they are kept.
fn popup_world(g: &Game, time: &Time) -> crate::ui::PopupWorld {
    let w = &g.controls.client.world;
    crate::ui::PopupWorld {
        now_ms: (time.elapsed_secs_f64() * 1000.0) as u32,
        tooltips_disabled: false,
        game_ending: w.script_engine.is_game_ending(),
        replay_game: w.cuts.game_loop.game_mode == rts_lockstep_engine::common::recorder::game_mode::REPLAY,
        quit_menu_visible: g.controls.client.modes.quit_menu_visible,
    }
}

/// (#138) Whether a window point is on the bar as drawn (`placement`, `height` logical pixels at the window's bottom).
fn bar_contains(p: &crate::ui::LayoutPlacement, height: f32, window_height: f32, at: Vec2) -> bool {
    at.x >= p.left && at.x <= p.left + crate::ui::LAYOUT.0 as f32 * p.scale && at.y >= window_height - height && at.y <= window_height
}

/// (#34) GUI glue (needs a window): a pointer event over the bar, from the screen (logical pixels) into the layout
/// ([`crate::ui::LayoutPlacement`] inverted) and through [`crate::ui::GameUi::pointer`]; a command button the manager
/// selected presses through [`Controls::press`].
fn bar_pointer(game: &mut Game, bar: &UiBar, msg: u32, screen: Vec2, id: &bevy::picking::pointer::PointerId, time: &Time) {
    let Some(p) = bar.placement else { return };
    let touch = matches!(id, bevy::picking::pointer::PointerId::Touch(_));
    let pos = p.to_layout((screen.x, screen.y));
    let g = &mut *game;
    // (#138) the pointer's place over the bar, for the frame's mouse event
    g.bar_pointer = Some((screen, touch));
    let world = popup_world(g, time);
    let Some(ui) = g.ui.as_mut() else { return };
    // (#136) through `winProcessMouseEvent` with the one mouse's tooltip lent
    let Some(slot) = ui.with_mouse(&mut g.pause.mouse.tooltip, |ui| ui.pointer(msg, pos, touch, &world)) else { return };
    let Some(Some(index)) = g.bar_buttons.get(slot).copied() else { return };
    if let Some(b) = rts_lockstep_engine::client::command_world::gui_button(&g.controls.client.world, index) {
        g.controls.press(&b);
    }
}

/// (#35) The radar's window on screen this frame (logical pixels), for the pointer: a press there is the radar's.
#[derive(Resource, Default)]
struct RadarBox {
    rect: Option<Rect>,
}

impl RadarBox {
    fn contains(&self, p: Vec2) -> bool {
        self.rect.is_some_and(|r| r.contains(p))
    }
}

/// The radar's image node.
#[derive(Component)]
struct RadarNode;

/// ~~The tactical view's corners on the plane of height `z` (`getScreenCornerWorldPointsAtZ`: upper left, upper right,
/// lower right, lower left), game axes, through Bevy's camera rays.~~ **Corrected 2026-11-11, #138:** the radar's view
/// box is the translated `View::getScreenCornerWorldPointsAtZ` ([`crate::camera::Camera::screen_corners_at_z`]), over the
/// view's own height.

/// GUI glue (needs a window): the radar (#35), drawn by the engine's `W3DRadar` translation
/// (`rts_lockstep_engine::client::w3d_radar`, tested there; `ClientRadar::draw`) into an image shown in `LeftHUD`, or
/// without the game's control bar in a box at the top right ([`crate::ui::radar_screen_rect`], tested there); redrawn when
/// the logic frame or the window's size changes, hidden while the radar is off. A press on it is `LeftHUDInput`'s
/// ([`Controls::radar_click`], tested in `input`): a look moves the camera there.
#[allow(clippy::too_many_arguments)]
fn radar_view(
    mut commands: Commands,
    mut game: NonSendMut<Game>,
    mut images: ResMut<Assets<Image>>,
    mut nodes: Query<(&mut Node, &mut Visibility, &mut ImageNode), With<RadarNode>>,
    windows: Query<&Window>,
    rig: Option<Res<Rig>>,
    ui_bar: Res<UiBar>,
    mut radar_box: ResMut<RadarBox>,
) {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let Ok(window) = windows.single() else { return };
    let g = &mut *game;
    // the water's radar colour from the art's INI, once (`GameEngine::init`'s `Water.ini` loads)
    if g.radar_water_pending {
        if let Some(art) = &g.models.art {
            let mut read = 0;
            for path in rts_lockstep_engine::client::water::WATER_INI_FILES {
                if let Some(text) = art.fs.open_file(path) {
                    if let Err(e) = g.controls.radar.water.load_text(path, &text) {
                        warn!("radar: {} did not read: {e:?}", String::from_utf8_lossy(path));
                    }
                    read += 1;
                }
            }
            info!("radar: {read} water files read");
            let colors = g.controls.radar.terrain_colors.take();
            g.controls.radar.set_terrain_colors(colors, &mut g.controls.client.world);
            g.radar_drawn = None;
        }
        g.radar_water_pending = false;
    }
    let screen = (window.width(), window.height());
    let r = crate::ui::radar_screen_rect(g.ui.as_ref().and_then(|u| u.radar_window()), ui_bar.placement, screen);
    let size = ICoord2D { x: (r[2] - r[0]).round() as i32, y: (r[3] - r[1]).round() as i32 };
    radar_box.rect = Some(Rect::new(r[0], r[1], r[2], r[3]));
    g.controls.radar.radar.set_radar_window(Some((ICoord2D { x: r[0].round() as i32, y: r[1].round() as i32 }, size)));
    let frame = g.controls.client.world.frame();
    let entity_node = |n: &mut Node| {
        n.left = px(r[0]);
        n.top = px(r[1]);
        n.width = px(size.x as f32);
        n.height = px(size.y as f32);
    };
    if g.radar_drawn == Some((frame, size)) {
        for (mut n, ..) in &mut nodes {
            entity_node(&mut n);
        }
        return;
    }
    g.radar_drawn = Some((frame, size));
    // (#138) `TheTacticalView->getScreenCornerWorldPointsAtZ` (W3DRadar.cpp:185), over the view's height
    let view = rig.map(|r| r.cam.screen_corners_at_z(g.controls.radar.radar.terrain_average_z));
    let img = g.controls.radar.draw(&mut g.controls.client.world, Some(&g.models.drawables), size, view);
    let Some(img) = img else {
        for (mut n, mut vis, _) in &mut nodes {
            entity_node(&mut n);
            *vis = Visibility::Hidden;
        }
        // (off: the press is not the radar's)
        radar_box.rect = None;
        return;
    };
    g.radar_event_sounds += img.event_sounds;
    let image = Image::new(
        Extent3d { width: img.width.max(1) as u32, height: img.height.max(1) as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        img.rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let handle = match &g.radar_image {
        Some(h) => {
            let _ = images.insert(h, image);
            h.clone()
        }
        None => {
            let h = images.add(image);
            g.radar_image = Some(h.clone());
            h
        }
    };
    match nodes.iter_mut().next() {
        Some((mut n, mut vis, mut image_node)) => {
            entity_node(&mut n);
            *vis = Visibility::Visible;
            image_node.image = handle;
        }
        None => {
            let mut n = Node { position_type: PositionType::Absolute, ..default() };
            entity_node(&mut n);
            commands
                .spawn((RadarNode, n, ImageNode { image: handle, image_mode: bevy::ui::widget::NodeImageMode::Stretch, ..default() }, ZIndex(5)))
                .observe(|ev: On<PointerPress>, mut game: NonSendMut<Game>, mut rig: ResMut<Rig>, radar_box: Res<RadarBox>| {
                    let Some(rect) = radar_box.rect else { return };
                    let local = ev.pointer.position - rect.min;
                    let right = matches!(ev.button, bevy::picking::pointer::PointerButton::Secondary);
                    let pixel = ICoord2D { x: local.x as i32, y: local.y as i32 };
                    if let Some(rts_lockstep_engine::client::control_bar_callback::RadarClick::LookAt(at)) = game.controls.radar_click(pixel, right) {
                        // `TheTacticalView->lookAt(&world)`
                        rig.cam.look_at(at, &game.controls.client.world);
                    }
                });
        }
    }
}

/// The popup description's panel (#35), and the lines it shows.
#[derive(Component)]
struct TooltipPanel(Vec<String>);

/// GUI glue (needs a window): the shown command button's popup (`ControlBarPopupDescription.wnd`'s name, cost and
/// description, from [`crate::ui::tooltip_lines`]; when, from ~~[`crate::ui::TooltipTimer`]~~ (#138) the bar's popup state,
/// `GameUi::shown_popup`, both tested in `ui`), as a
/// panel just above the control bar, no wider than the screen less its 8-pixel margins (a phone's too). Rebuilt when its
/// lines change; none while nothing is shown.
fn tooltip_panel(mut commands: Commands, game: NonSend<Game>, mut panels: Query<(Entity, &mut TooltipPanel, &mut Node)>, windows: Query<&Window>, ui_bar: Res<UiBar>) {
    // (#144) a window's popup (the money, power and general's experience windows) shows the text it was filled with
    let lines = match game.ui.as_ref().and_then(|u| u.shown_popup()) {
        Some((Some(b), _)) => game.tooltip_lines.iter().find(|(button, _)| *button == b).map(|(_, l)| l.clone()),
        Some((None, Some(_))) => game.window_popup_lines.clone(),
        _ => None,
    };
    let ww = windows.single().map_or(800.0, |w| w.width());
    let width = (ww - 16.0).clamp(120.0, 420.0);
    let mut existing = panels.iter_mut().next();
    let Some(lines) = lines.filter(|l| !l.is_empty()) else {
        if let Some((e, _, _)) = existing {
            commands.entity(e).despawn();
        }
        return;
    };
    if let Some((_, panel, node)) = existing.as_mut() {
        node.bottom = px(ui_bar.height + 6.0);
        node.left = px((ww - width) / 2.0);
        node.width = px(width);
        if panel.0 == lines {
            return;
        }
    }
    if let Some((e, _, _)) = existing {
        commands.entity(e).despawn();
    }
    let mut e = commands.spawn((
        TooltipPanel(lines.clone()),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(ui_bar.height + 6.0),
            left: px((ww - width) / 2.0),
            width: px(width),
            padding: UiRect::all(px(8)),
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            border_radius: BorderRadius::all(px(6)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.07, 0.10, 0.92)),
    ));
    e.with_children(|p| {
        for (i, l) in lines.iter().enumerate() {
            let (size, color) = if i == 0 { (16.0, Color::srgb(1.0, 0.85, 0.4)) } else { (13.0, Color::WHITE) };
            p.spawn((Text::new(l.clone()), TextFont { font_size: bevy::text::FontSize::Px(size), ..default() }, TextColor(color)));
        }
    });
}

fn bar_button(label: &str) -> impl Bundle {
    (
        Button,
        Node {
            min_width: px(96),
            height: px(48),
            padding: UiRect::horizontal(px(8)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(6)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.12, 0.16, 0.22, 0.9)),
        children![(Text::new(label), TextColor(Color::WHITE))],
    )
}

/// (#62) A list's button: [`bar_button`]'s look, as tall as its text, never wider than its row (a long mission name wraps
/// on a phone).
fn list_button(label: &str) -> impl Bundle {
    (
        Button,
        Node {
            min_width: px(96),
            max_width: percent(100),
            min_height: px(44),
            padding: UiRect::axes(px(8), px(4)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(6)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.12, 0.16, 0.22, 0.9)),
        children![(Text::new(label), TextFont { font_size: bevy::text::FontSize::Px(15.0), ..default() }, TextColor(Color::WHITE))],
    )
}

// ---------------------------------------------------------------------------
// Saved games (#4): the panel, its buttons and what they do, through `crate::saves` (tested there).
// ---------------------------------------------------------------------------

/// The save folder and the panel's state (a non-send resource: it lives across games, as the folder does).
struct Saves {
    folder: crate::saves::SaveFolder,
    /// Bumped when the folder or the status changes, so the panel is rebuilt.
    generation: u64,
    /// The generation the panel on screen was built for (`None`: no panel).
    built: Option<u64>,
    /// Whether the game was paused before the panel opened (it is paused while the panel is open, as upstream's save
    /// menu pauses the game).
    paused_before: bool,
    status: String,
    /// GitHub #64 part 2: the save being loaded, waiting for its map's folder (`crate::saves::PendingLoad`), by its name.
    pending: Option<(String, crate::saves::PendingLoad)>,
    /// (#78) How the last load ended (its name, or the error), for the shell's save/load screen, which shows the shell
    /// again after a failed load (`doLoadGame`'s `showShell(TRUE)`).
    last_load: Option<Result<String, String>>,
}

impl Saves {
    fn new() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let folder = crate::saves::native_save_dir().map_or_else(crate::saves::SaveFolder::default, crate::saves::SaveFolder::native);
        #[cfg(target_arch = "wasm32")]
        let folder = crate::saves::SaveFolder::default();
        let status = match &folder.dir {
            Some(d) => format!("saves in {}", d.display()),
            None => "saves in this browser (OPFS Save/); never uploaded".to_string(),
        };
        Saves { folder, generation: 0, built: None, paused_before: false, status, pending: None, last_load: None }
    }
    fn set_status(&mut self, s: String) {
        crate::saves::page::set_status(s.clone());
        self.status = s;
        self.generation += 1;
    }
}

/// A press of the saves panel's buttons, carried out by [`apply_save_action`].
#[derive(Clone, Debug)]
enum SaveCmd {
    Toggle,
    /// (#87) Under `name` (`None`: the folder's next name) with `description` (`None`: the client's own).
    Save { name: Option<String>, description: Option<String> },
    Load(String),
    Export(String),
}

#[derive(Resource, Default)]
struct SaveAction(Option<SaveCmd>);

/// The saves panel's root.
#[derive(Component)]
struct SavesPanel;

/// GUI glue (needs the app): carries out the panel's button presses through `crate::saves` (whose tests save, list and load
/// games the same way): open or close the panel (pausing the game under it); save the game (the camera's view with it)
/// under the folder's next name; load a save into a new game (the World made for its map, the client fresh; the camera
/// turned to the save's view); offer a save as a download.
#[allow(clippy::too_many_arguments)]
fn apply_save_action(
    mut commands: Commands,
    mut game: NonSendMut<Game>,
    mut saves: NonSendMut<Saves>,
    mut action: ResMut<SaveAction>,
    mut rig: ResMut<Rig>,
    mut drawn: ResMut<Drawn>,
    mut terrain: ResMut<TerrainState>,
    mut campaigns: NonSendMut<Campaigns>,
) {
    // (#64 part 2: a save waiting for its map's folder, read from the art's files; once mounted, the load)
    if let Some((name, mut p)) = saves.pending.take() {
        let mounted = match game.models.art.as_ref() {
            Some(art) => p.mount(&art.fs),
            None => p.mount(&rts_lockstep_engine::common::file_system::FileSystem::new(rts_lockstep_engine::common::file_system::MemoryDevice::new())),
        };
        match mounted {
            Ok(false) => saves.pending = Some((name, p)),
            Ok(true) => finish_load(&mut commands, &mut game, &mut saves, &mut rig, &mut drawn, &mut terrain, name, p),
            Err(e) => {
                saves.last_load = Some(Err(e.clone()));
                saves.set_status(format!("error: {e}"));
            }
        }
    }
    let Some(cmd) = action.0.take() else { return };
    match cmd {
        SaveCmd::Load(name) if saves.folder.files.get(&name).is_some_and(|b| crate::campaigns::Flow::is_mission_save(b)) => {
            // (#62 part 2: a mission save holds the campaign alone; its load starts the mission, whose map is read as a
            // chosen mission's is: the campaign flow's)
            let bytes = saves.folder.files.get(&name).cloned().unwrap_or_default();
            let status = match game.base.as_deref() {
                Some(base) => match campaigns.flow.load_mission_save(base, bytes) {
                    Ok(()) => {
                        let map = campaigns.flow.waiting_for().map(|m| String::from_utf8_lossy(m).into_owned()).unwrap_or_default();
                        campaigns.set_status(format!("waiting {map}"));
                        crate::saves::page::set_panel_open(false);
                        game.paused = saves.paused_before;
                        saves.last_load = Some(Ok(name.clone()));
                        format!("loading mission save {name}")
                    }
                    Err(e) => format!("error: {e}"),
                },
                None => "error: a mission save needs your Zero Hour files".to_string(),
            };
            if status.starts_with("error") {
                saves.last_load = Some(Err(status.clone()));
            }
            saves.set_status(status);
        }
        SaveCmd::Toggle => {
            let open = !crate::saves::page::panel_open();
            crate::saves::page::set_panel_open(open);
            if open {
                saves.paused_before = game.paused;
                game.paused = true;
            } else {
                game.paused = saves.paused_before;
            }
            saves.generation += 1;
        }
        SaveCmd::Save { name: save_name, description } => {
            let g = &mut *game;
            // (`View::xfer`: the camera's heading and the point it looks at)
            g.save_state.view.angle = rig.cam.view.get_angle();
            g.save_state.view.position = rig.cam.view.pos;
            let frame = g.controls.client.world.frame();
            let desc = description.unwrap_or_else(|| format!("{} | frame {frame}", g.playing.split(" | ").next().unwrap_or("")));
            // (#59: the drawables with their ambient sounds, as the audio has them)
            if let Some(sound) = g.audio.as_deref() {
                sound.audio.ambient.save_into(&mut g.models.drawables);
            }
            let clock = g.models_clock;
            let (controls, save_state) = (&mut g.controls, &mut g.save_state);
            let r = g.models.with_assets(|drawables, assets| {
                // (the particle systems' manager is the drawables' effects')
                let mut particles = std::mem::take(&mut drawables.fx.particles);
                let r = crate::saves::save_game(controls, &mut particles, save_state, Some((&mut *drawables, assets, clock)), &desc);
                drawables.fx.particles = particles;
                r
            });
            let status = match r {
                Ok(bytes) => {
                    let name = save_name.unwrap_or_else(|| saves.folder.next_name());
                    match saves.folder.write(&name, bytes) {
                        Ok(()) => {
                            // (`saveGame`'s message to the player)
                            g.controls.client.ui_messages.push(b"GUI:GameSaveComplete".to_vec());
                            format!("saved {name}")
                        }
                        Err(e) => format!("error: {e}"),
                    }
                }
                Err(e) => format!("error: {e}"),
            };
            info!("{status}");
            saves.set_status(status);
        }
        SaveCmd::Export(name) => {
            let s = match saves.folder.export(&name) {
                Some(p) if saves.folder.dir.is_some() => format!("{name} is the file {p}"),
                Some(_) => format!("exported {name}"),
                None => format!("error: no save {name}"),
            };
            saves.set_status(s);
        }
        SaveCmd::Load(name) => {
            let Some(bytes) = saves.folder.files.get(&name).cloned() else {
                saves.last_load = Some(Err(format!("no save {name}")));
                return;
            };
            // (#64 part 2: the load waits for the save's map folder, read from the art's files, as a mission's start does)
            match crate::saves::PendingLoad::new(bytes, game.base.as_deref()) {
                Ok(p) => {
                    saves.set_status(format!("loading {name}"));
                    saves.pending = Some((name, p));
                }
                Err(e) => {
                    saves.last_load = Some(Err(e.clone()));
                    saves.set_status(format!("error: {e}"));
                }
            }
        }
    }
}

/// GUI glue (needs the app): a save whose World is ready ([`crate::saves::PendingLoad`]) loaded into a new game, as the
/// panel's Load did at once before #64: the client fresh, the save's drawables made with the art, the camera turned to the
/// save's view.
#[allow(clippy::too_many_arguments)]
fn finish_load(
    commands: &mut Commands,
    game: &mut NonSendMut<Game>,
    saves: &mut NonSendMut<Saves>,
    rig: &mut ResMut<Rig>,
    drawn: &mut ResMut<Drawn>,
    terrain: &mut ResMut<TerrainState>,
    name: String,
    p: crate::saves::PendingLoad,
) {
    {
        {
            let base = game.base.take();
            // (#59: the save's drawables are made into the new game's, with the art; its clock starts at 0)
            let mut models = WorldDrawables::new(game.models.art.take());
            let loaded = models.with_assets(|drawables, assets| crate::saves::client_from_pending(p, Some((drawables, assets, 0))));
            match loaded {
                Ok((controls, particles, state, player, outcome)) => {
                    let label = String::from_utf8_lossy(&outcome.info.map_label).into_owned();
                    let mut g = Game::from_loaded(controls, player, format!("loaded {name} ({label})"), models);
                    g.ambient_restore_pending = true;
                    g.models.drawables.fx.particles.unique_system_id = particles.unique_system_id;
                    g.base = base;
                    g.save_state = state;
                    let read_past = g.save_state.read_past.len();
                    // the camera to the save's view (`setAngle`, `lookAt`)
                    **rig = Rig::new(&g.controls.client.world, rig.cam.screen);
                    rig.cam.set_saved_view(g.save_state.view.angle, g.save_state.view.position, &g.controls.client.world);
                    game.replace(g);
                    for (_, (e, _)) in drawn.0.drain() {
                        commands.entity(e).despawn();
                    }
                    terrain.pending = true;
                    crate::saves::page::set_panel_open(false);
                    game.paused = false;
                    saves.last_load = Some(Ok(name.clone()));
                    saves.set_status(format!("loaded {name}{}", if read_past > 0 { format!(" ({read_past} parts read past)") } else { String::new() }));
                    info!("loaded {name}: read past {:?}", game.save_state.read_past);
                }
                Err(e) => {
                    game.base = base;
                    game.models.art = models.art.take();
                    saves.last_load = Some(Err(e.clone()));
                    warn!("{e}");
                    saves.set_status(format!("error: {e}"));
                }
            }
        }
    }
}

/// GUI glue (needs a window): the files the page read from OPFS (or the player imported) join the folder; the panel is
/// (re)built when it opens or its folder or status changes: the save and close buttons, the status, and each save's line
/// (`SaveListing::line`, tested in `crate::saves`) with its load and export buttons. Full width less the zoom column, so a
/// phone's screen fits it; its rows wrap.
fn saves_panel(mut commands: Commands, mut saves: NonSendMut<Saves>, panels: Query<Entity, With<SavesPanel>>) {
    if saves.folder.take_page_files() > 0 {
        saves.generation += 1;
    }
    let open = crate::saves::page::panel_open();
    let want = open.then_some(saves.generation);
    if want == saves.built {
        return;
    }
    saves.built = want;
    for e in &panels {
        commands.entity(e).despawn();
    }
    if !open {
        return;
    }
    let small = |t: String| (Text::new(t), TextFont { font_size: bevy::text::FontSize::Px(14.0), ..default() }, TextColor(Color::WHITE));
    let root = commands
        .spawn((
            SavesPanel,
            Node {
                position_type: PositionType::Absolute,
                top: px(36),
                left: px(8),
                right: px(64),
                max_height: percent(80),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                padding: UiRect::all(px(8)),
                overflow: Overflow::clip_y(),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.08, 0.11, 0.94)),
        ))
        .id();
    let header = commands
        .spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: px(6), row_gap: px(6), ..default() })
        .id();
    let save = commands.spawn((bar_button("Save game"), observe(|_: On<Activate>, mut a: ResMut<SaveAction>| a.0 = Some(SaveCmd::Save { name: None, description: None })))).id();
    let close = commands.spawn((bar_button("Close"), observe(|_: On<Activate>, mut a: ResMut<SaveAction>| a.0 = Some(SaveCmd::Toggle)))).id();
    commands.entity(header).add_children(&[save, close]);
    if saves.folder.dir.is_none() {
        // (the browser: room for the page's import button, which `web/index.html` places here; on a phone it wraps to a
        // second row, where the page's narrow layout puts the button)
        let room = commands.spawn(Node { width: px(120), height: px(40), ..default() }).id();
        commands.entity(header).add_child(room);
    }
    let list = saves.folder.listing();
    let import_note = if saves.folder.dir.is_none() { " | import a .sav with the page's button" } else { "" };
    let status = commands.spawn(small(format!("{} | {} saves{import_note}", saves.status, list.len()))).id();
    commands.entity(root).add_children(&[header, status]);
    // (the newest twelve: the game's own menu, C5, will scroll)
    for l in list.iter().take(12) {
        let row = commands
            .spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, align_items: AlignItems::Center, column_gap: px(6), row_gap: px(4), ..default() })
            .id();
        let text = commands.spawn(small(l.line())).insert(Node { flex_grow: 1.0, flex_basis: px(160), ..default() }).id();
        let (n1, n2) = (l.name.clone(), l.name.clone());
        let load = commands.spawn((bar_button("Load"), observe(move |_: On<Activate>, mut a: ResMut<SaveAction>| a.0 = Some(SaveCmd::Load(n1.clone()))))).id();
        let export = commands.spawn((bar_button("Export"), observe(move |_: On<Activate>, mut a: ResMut<SaveAction>| a.0 = Some(SaveCmd::Export(n2.clone()))))).id();
        commands.entity(row).add_children(&[text, load, export]);
        commands.entity(root).add_child(row);
    }
}

// ---------------------------------------------------------------------------
// Campaigns (#62): the mission list, a mission's map read as the art is, and the score screen, through `crate::campaigns`
// (tested there).
// ---------------------------------------------------------------------------

/// The campaign panel's state and the flow (a non-send resource: the flow holds Worlds).
#[derive(Default)]
struct Campaigns {
    flow: crate::campaigns::Flow,
    /// The difficulty the list starts at (`DIFFICULTY_NORMAL` by default, as the difficulty menu's).
    difficulty: Option<i32>,
    status: String,
    /// The current mission's briefing text (the movies are Bink, never released).
    briefing: Vec<String>,
    /// Bumped when what the panel shows changes; the generation the panel on screen was built for.
    generation: u64,
    built: Option<u64>,
    paused_before: bool,
}

impl Campaigns {
    fn set_status(&mut self, s: String) {
        crate::campaigns::page::set_status(s.clone());
        self.status = s;
        self.generation += 1;
    }
    fn difficulty(&self) -> i32 {
        self.difficulty.unwrap_or(1)
    }
}

/// A press of the campaign panel's buttons, carried out by [`apply_campaign_action`].
#[derive(Clone, Debug)]
enum CampaignCmd {
    Toggle,
    Difficulty,
    Choose(rts_lockstep_engine::logic::world::campaign_world::MissionChoice),
    Continue,
}

#[derive(Resource, Default)]
struct CampaignAction(Option<CampaignCmd>);

/// The campaign panel's root.
#[derive(Component)]
struct CampaignPanel;

/// Opens or closes the campaign panel (pausing the game under it, as the saves panel does).
fn set_campaign_panel(open: bool, game: &mut Game, c: &mut Campaigns) {
    if open == crate::campaigns::page::panel_open() {
        return;
    }
    crate::campaigns::page::set_panel_open(open);
    if open {
        c.paused_before = game.paused;
        game.paused = true;
    } else {
        game.paused = c.paused_before;
    }
    c.generation += 1;
}

/// GUI glue (needs the app): carries out the panel's presses through `crate::campaigns::Flow` (whose tests, and the
/// engine's `campaign_world` tests, drive the same calls): open or close the panel; change the difficulty; a choice waits
/// for its map, in a copy of the install's World; the score screen's Continue.
fn apply_campaign_action(mut game: NonSendMut<Game>, mut campaigns: NonSendMut<Campaigns>, mut action: ResMut<CampaignAction>) {
    let Some(cmd) = action.0.take() else { return };
    let c = &mut *campaigns;
    match cmd {
        CampaignCmd::Toggle => {
            let open = !crate::campaigns::page::panel_open();
            set_campaign_panel(open, &mut game, c);
        }
        CampaignCmd::Difficulty => {
            c.difficulty = Some((c.difficulty() + 1) % 3);
            c.generation += 1;
        }
        CampaignCmd::Choose(choice) => {
            let Some(base) = game.base.as_deref() else {
                c.set_status("error: load your Zero Hour files first".to_string());
                return;
            };
            let d = c.difficulty();
            match c.flow.choose(base, choice, d) {
                Ok(()) => {
                    let map = c.flow.waiting_for().map(|m| String::from_utf8_lossy(m).into_owned()).unwrap_or_default();
                    c.set_status(format!("waiting {map}"));
                }
                Err(e) => c.set_status(format!("error: {e}")),
            }
        }
        CampaignCmd::Continue => {
            if c.flow.continue_campaign() {
                let map = c.flow.waiting_for().map(|m| String::from_utf8_lossy(m).into_owned()).unwrap_or_default();
                c.set_status(format!("waiting {map}"));
            } else {
                c.set_status("the campaign is over".to_string());
            }
        }
    }
}

/// GUI glue (needs the app): once a frame, a waiting mission reads its map from the art's files (the page's ranges for it
/// are exchanged by [`pump_art`]); once it has started, it replaces the game (the art and the install's World kept), as a
/// loaded save does. After each frame, a mission's end opens the score screen in the panel.
#[allow(clippy::too_many_arguments)]
fn campaign_flow(
    mut commands: Commands,
    mut game: NonSendMut<Game>,
    mut campaigns: NonSendMut<Campaigns>,
    mut rig: ResMut<Rig>,
    mut drawn: ResMut<Drawn>,
    mut terrain: ResMut<TerrainState>,
    mut saves: NonSendMut<Saves>,
    mut shell: NonSendMut<ShellState>,
) {
    let c = &mut *campaigns;
    if c.flow.pending.is_some() {
        let Some(art) = game.models.art.as_ref() else {
            c.flow.pending = None;
            c.set_status("error: no game files to read the map from".to_string());
            return;
        };
        let Some(started) = c.flow.poll(&art.fs) else { return };
        match started {
            Ok(s) => {
                info!("mission: {}", s.label);
                c.briefing = crate::campaigns::briefing(&s.world);
                let art = game.models.art.take();
                let base = game.base.take();
                game.replace(Game::new(s.world, s.player, s.label.clone(), art));
                game.base = base;
                for (_, (e, _)) in drawn.0.drain() {
                    commands.entity(e).despawn();
                }
                terrain.pending = true;
                *rig = Rig::new(&game.controls.client.world, rig.cam.screen);
                crate::campaigns::page::set_panel_open(false);
                c.paused_before = false;
                c.set_status(format!("playing {}", s.label));
            }
            Err(e) => {
                warn!("the mission did not start: {e}");
                c.set_status(format!("error: {e}"));
            }
        }
        return;
    }
    if game.base.is_none() {
        return;
    }
    // (#89 part 2) with the shell, a mission's or a skirmish's end opens the translated score screen, as upstream's
    // `clearGameData` pushes it (the game paused under it); its *Continue* and its mission save come back as the shell's
    // actions (`shell_ui`)
    let st = &mut *shell;
    if let Some(ui) = st.ui.as_mut() {
        if let Some(g) = c.flow.check_score_screen(&game.controls.client.world) {
            // (#98) the recorder's last replay (`00000000.rep`) into the replay folder, where upstream's recorder wrote
            // it, for the score screen's *Save Replay*; the shell's copy of the folder made again
            if let Some(bytes) = game.controls.client.world.last_replay().map(|b| b.to_vec()) {
                let folder = st.replays.get_or_insert_with(crate::replays::ReplayFolder::start);
                folder.take_page_files();
                if let Err(e) = folder.write("00000000.rep", bytes) {
                    warn!("the last replay was not kept: {e}");
                }
            }
            if let (Some(folder), Some(base)) = (st.replays.as_ref(), game.base.as_deref()) {
                ui.set_replays(folder, base);
                st.replays_generation = Some(folder.generation);
            }
            ui.score_screen(g);
            st.shown_over = game.serial;
            game.paused = true;
            c.set_status("over: the score screen".to_string());
        }
        return;
    }
    if c.flow.check_game_over(&game.controls.client.world) {
        let won = c.flow.over.as_ref().is_some_and(|o| o.victorious);
        let mut status = if won { "over victorious" } else { "over defeated" }.to_string();
        // (#62 part 2: `finishSinglePlayerInit`'s automatic mission save, into the save folder; the button reads "Save and
        // Continue")
        if let Some((desc, bytes)) = c.flow.over.as_mut().and_then(|o| o.mission_save.take()) {
            let name = saves.folder.next_name();
            match saves.folder.write(&name, bytes) {
                Ok(()) => {
                    status.push_str(&format!(" | mission saved {name} ({desc})"));
                    saves.generation += 1;
                }
                Err(e) => status.push_str(&format!(" | the mission save failed: {e}")),
            }
        }
        c.set_status(status);
        set_campaign_panel(true, &mut game, c);
    }
}

// ---------------------------------------------------------------------------
// The shell (#78): the game's menus from the owner's files, through `crate::shell` (tested there).
// ---------------------------------------------------------------------------

/// The shell and what the app keeps for it (a non-send resource: it lives across games, as upstream's `TheShell` does).
#[derive(Default)]
struct ShellState {
    ui: Option<Box<crate::shell::ShellUi>>,
    /// The mapped images, read once from the art (`TheMappedImageCollection`), shared with the control bar; `Err` when they
    /// did not load (the bar then reads them itself, and fails as before).
    images: Option<Result<std::sync::Arc<rts_lockstep_engine::client::image::ImageCollection>, String>>,
    /// Whether the shell has been read (or found missing): it is read once, when the owner's files are there.
    tried: bool,
    /// The game ([`Game::serial`]) the shell was shown over; another game is a game started: `hideShell`.
    shown_over: u64,
    /// Where the shell's layout is on screen this frame (for the pointer).
    placement: Option<crate::ui::LayoutPlacement>,
    /// The pointer's last screen point over the shell (a release not seen there is sent at it).
    last_pointer: Option<(f32, f32)>,
    textures: UiTextures,
    /// (#78) The saves panel's generation the shell's save list was last given ([`crate::shell::ShellUi::set_saves`]).
    saves_generation: Option<u64>,
    /// (#78) A load the shell's save/load screen asked for, until it ends (a failed one shows the shell again).
    loading: Option<String>,
    /// (#93) The user data folder's preference files (made at the first use: [`ShellState::user_data`]).
    user_data: Option<crate::user_data::UserDataFolder>,
    /// (#98) The replay folder (made at the first use), and its generation the shell's copy was last made from.
    replays: Option<crate::replays::ReplayFolder>,
    replays_generation: Option<u64>,
}

impl ShellState {
    /// (#93) The user data folder, with the files the page has handed over since the last call (the browser's).
    fn user_data(&mut self) -> &mut crate::user_data::UserDataFolder {
        let f = self.user_data.get_or_insert_with(crate::user_data::UserDataFolder::start);
        f.take_page_files();
        f
    }
}

/// (#88) The keyboard pan's speed from the options' scroll factor (1 at the default factor): the shell's
/// [`crate::shell::ShellUi::client_options`].
#[derive(Resource, Clone, Copy)]
struct KeyboardScroll(f32);

/// The shell's root node, and the quads it was last drawn with.
#[derive(Component)]
struct ShellRoot(Vec<crate::ui::UiQuad>);

/// GUI glue (needs the app): once the owner's files are there (the art, and the install's World), the mapped images
/// ([`crate::ui::load_mapped_images`]) and the shell ([`crate::shell::ShellUi::load`], tested there), read once; while a
/// file is being fetched (the web's ranged reads) it tries again on a later frame. The shell is shown over the install's
/// game, paused under it, as the game shows its menus before a game. Without `Menus\MainMenu.wnd` or
/// `WindowTransitions.ini` there is no shell, and the plain panels (saves, campaigns) are the way in.
fn load_shell(mut game: NonSendMut<Game>, mut shell: NonSendMut<ShellState>, time: Res<Time>) {
    let st = &mut *shell;
    if st.tried {
        return;
    }
    let g = &mut *game;
    let Some(art) = g.models.art.as_ref() else { return };
    let images = match &st.images {
        Some(r) => r.clone(),
        None => match crate::ui::load_mapped_images(&art.fs) {
            crate::ui::ImagesBuild::Waiting => return,
            crate::ui::ImagesBuild::Ready(i) => {
                st.images = Some(Ok(i.clone()));
                Ok(i)
            }
            crate::ui::ImagesBuild::Missing(why) => {
                warn!("no mapped images: {why}");
                st.images = Some(Err(why.clone()));
                Err(why)
            }
        },
    };
    if g.base.is_none() {
        return;
    }
    let images = images.unwrap_or_default();
    let before = art.fs.local.device.unserved_reads();
    let now = (time.elapsed_secs_f64() * 1000.0) as u32;
    let loaded = crate::shell::ShellUi::load(&art.fs, &g.controls.client.world.statics.game_text, images, now);
    if art.fs.local.device.unserved_reads() != before {
        return;
    }
    st.tried = true;
    match loaded {
        Some(mut ui) => {
            info!("shell: {} windows", ui.wm.tree.windows.len());
            // (#93) the preference files, and (#88) the options applied to the shell's copy of the global data
            let base = g.base.as_deref();
            ui.set_user_data(st.user_data(), base);
            // (#89) the skirmish screens' copy of the install's data
            if let Some(base) = g.base.as_deref() {
                ui.set_skirmish_data(base, &art.fs);
            }
            st.shown_over = g.serial;
            g.paused = true;
            st.ui = Some(Box::new(ui));
        }
        None => info!("no shell (no Menus\\MainMenu.wnd or Data\\INI\\WindowTransitions.ini): the plain panels"),
    }
}

/// GUI glue (needs a window): the shell each frame. A game started under it hides it (`hideShell`); while shown, its
/// clock ([`crate::shell::ShellUi::frame`]), its callbacks' requests ([`crate::shell::ShellUi::take_actions`]: a campaign's
/// start is the campaign panel's flow; a quit ends the native app, and on the web says so), and its windows drawn from the
/// window manager's draw list ([`crate::shell::ShellUi::draw`]) with the control bar's drawing ([`spawn_quads`]), in the
/// layout's 4:3 scaled over the 3D view ([`crate::shell::shell_placement`]); rebuilt when the quads change. A plain panel
/// open over it hides it. The pointer over it goes through [`crate::shell::ShellUi::pointer_at`]; a release off it ends
/// the window manager's grab.
#[allow(clippy::too_many_arguments)]
fn shell_ui(
    mut commands: Commands,
    mut game: NonSendMut<Game>,
    mut shell: NonSendMut<ShellState>,
    mut campaigns: NonSendMut<Campaigns>,
    mut images: ResMut<Assets<Image>>,
    mut roots: Query<(Entity, &mut ShellRoot, &mut Node)>,
    windows: Query<&Window>,
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    mut exit: MessageWriter<AppExit>,
    mut saves: NonSendMut<Saves>,
    mut save_action: ResMut<SaveAction>,
    mut keyboard_scroll: ResMut<KeyboardScroll>,
) {
    let st = &mut *shell;
    let Some(ui) = st.ui.as_mut() else { return };
    let g = &mut *game;
    // (#88) the options the client has: the keyboard pan's speed and the alternate mouse mode; the audio's settings
    // for the screen's defaults
    let o = ui.client_options();
    keyboard_scroll.0 = o.keyboard_scroll;
    // (#108) InGameUI's quit menu flag, for the translators; and the World's game cleared (`clearGameData`): the
    // engine reset's InGameUI flags and `closeWindows`, once
    g.controls.client.modes.quit_menu_visible = ui.quit_menu_visible();
    let cleared = g.controls.client.world.game_cleared.is_some();
    if cleared && !g.cleared_seen {
        // (#138: `clearGameData`'s `HideControlBar()`, GameLogicDispatch.cpp:291, before its `closeWindows()`)
        if let Some(bar) = g.ui.as_mut() {
            bar.clear_game_data();
        }
        // (#144: `TheGameEngine->reset()`'s `Display::reset`, before it: the letterbox off at once)
        g.letterbox.reset();
        ui.clear_game_data_windows();
        // (#117: then `clearGameData`'s `TheMouse->setVisibility(TRUE)`, GameLogicDispatch.cpp:294)
        g.pause.mouse.set_visibility(true);
    }
    g.cleared_seen = cleared;
    // (#117) a script's `closeWindows` (`doQuickVictory`); the quit menu's quiet flag for the audio; the shell's calls on
    // `TheMouse` (the main menu's `setVisibility`, the quit menu's `setCursor(ARROW)`)
    if std::mem::take(&mut g.close_windows_pending) {
        ui.close_windows();
    }
    g.client_quiet = ui.client_quiet();
    ui.take_mouse_calls(&mut g.pause.mouse);
    g.controls.client.modes.alternate_mouse = o.alternate_mouse;
    if let Some(sound) = g.audio.as_deref() {
        ui.gui.audio_settings = sound.audio.manager.tables.settings.clone();
    }
    // (#93) the preference files the shell wrote, kept in the folder (on disk, or in OPFS by the page)
    for (name, bytes) in ui.take_user_data_writes() {
        let folder = st.user_data.get_or_insert_with(crate::user_data::UserDataFolder::start);
        if let Err(e) = folder.write(&name, bytes) {
            warn!("the preference file {name} was not kept: {e}");
        }
    }
    // (#98) the replay folder, as the replay menu and the replay popup list it (its files the page handed over first)
    {
        let folder = st.replays.get_or_insert_with(crate::replays::ReplayFolder::start);
        folder.take_page_files();
        if st.replays_generation != Some(folder.generation) {
            if let Some(base) = g.base.as_deref() {
                ui.set_replays(folder, base);
                st.replays_generation = Some(folder.generation);
            }
        }
    }
    // (#78) the save folder, as the save/load screen lists it; a load it asked for that failed shows the shell again
    if st.saves_generation != Some(saves.generation) {
        ui.set_saves(&saves.folder);
        st.saves_generation = Some(saves.generation);
    }
    if st.loading.is_some() {
        match saves.last_load.take() {
            Some(Err(e)) => {
                warn!("the shell's load failed: {e}");
                st.loading = None;
                ui.show();
                st.shown_over = g.serial;
                g.paused = true;
            }
            Some(Ok(_)) => st.loading = None,
            None => {}
        }
    }
    if ui.is_active() && g.serial != st.shown_over {
        // (`GameLogic::startNewGame`'s `TheShell->hideShell()`)
        ui.hide();
        info!("shell hidden: a game started");
    }
    // (#87: also while the in-game menus are up)
    if ui.takes_input() {
        // (#136) the shell's update and its stray release with the one mouse's tooltip lent (the menus'
        // `setCursorTooltip`, `winProcessMouseEvent`'s)
        let (last_pointer, placement) = (st.last_pointer, st.placement);
        let pressed = buttons.pressed(MouseButton::Left) || touches.iter().next().is_some();
        ui.with_mouse(&mut g.pause.mouse.tooltip, |ui| {
            ui.frame((time.elapsed_secs_f64() * 1000.0) as u32);
            if ui.wm.grab_window.is_some() && !pressed {
                match (last_pointer, placement) {
                    (Some(at), Some(p)) => ui.pointer_at(rts_lockstep_engine::client::window_manager::gwm::LEFT_UP, at, &p, false),
                    _ => ui.pointer(rts_lockstep_engine::client::window_manager::gwm::LEFT_UP, (-1.0, -1.0), false),
                }
            }
        });
    }
    if let Some(base) = g.base.as_deref() {
        let c = &mut *campaigns;
        for a in ui.take_actions(base, &mut c.flow) {
            match a {
                crate::shell::ShellAction::CampaignChosen | crate::shell::ShellAction::SkirmishChosen | crate::shell::ShellAction::ReplayChosen => {
                    let map = c.flow.waiting_for().map(|m| String::from_utf8_lossy(m).into_owned()).unwrap_or_default();
                    c.set_status(format!("waiting {map}"));
                }
                crate::shell::ShellAction::Failed(e) => {
                    warn!("the shell's start failed: {e}");
                    c.set_status(format!("error: {e}"));
                }
                crate::shell::ShellAction::Quit => {
                    #[cfg(not(target_arch = "wasm32"))]
                    exit.write(AppExit::Success);
                    #[cfg(target_arch = "wasm32")]
                    {
                        let _ = &mut exit;
                        g.playing = "Exit: a web page cannot close itself; close the tab to leave".to_string();
                    }
                }
                crate::shell::ShellAction::LoadGame(name) => {
                    // (#78: the saves panel's own load)
                    saves.last_load = None;
                    st.loading = Some(name.clone());
                    save_action.0 = Some(SaveCmd::Load(name));
                }
                crate::shell::ShellAction::DeleteSave(name) => {
                    let s = match saves.folder.delete(&name) {
                        Some(n) => format!("deleted {n}"),
                        None => format!("error: no save {name}"),
                    };
                    saves.set_status(s);
                }
                crate::shell::ShellAction::MissionSaved(desc, bytes) => {
                    // (#89 part 2: the score screen's `missionSave`, into the save folder)
                    let name = saves.folder.next_name();
                    let s = match saves.folder.write(&name, bytes) {
                        Ok(()) => format!("mission saved {name} ({desc})"),
                        Err(e) => format!("the mission save failed: {e}"),
                    };
                    saves.set_status(s);
                    saves.generation += 1;
                }
                // (#98) the replay folder's changes the shell made (its own copy already has them)
                crate::shell::ShellAction::ReplayWritten(name, bytes) => {
                    let folder = st.replays.get_or_insert_with(crate::replays::ReplayFolder::start);
                    match folder.write(&name, bytes) {
                        Ok(()) => st.replays_generation = Some(folder.generation),
                        Err(e) => warn!("the replay {name} was not kept: {e}"),
                    }
                }
                crate::shell::ShellAction::ReplayDeleted(name) => {
                    let folder = st.replays.get_or_insert_with(crate::replays::ReplayFolder::start);
                    match folder.delete(&name) {
                        Ok(()) => st.replays_generation = Some(folder.generation),
                        Err(e) => warn!("the replay {name} was not deleted: {e}"),
                    }
                }
                crate::shell::ShellAction::ReplayExported(name, bytes) => {
                    let folder = st.replays.get_or_insert_with(crate::replays::ReplayFolder::start);
                    match folder.export(&name, bytes) {
                        Ok(to) => info!("replay copied: {to}"),
                        Err(e) => warn!("the replay {name} was not copied: {e}"),
                    }
                }
                crate::shell::ShellAction::SetVolume(volume, affect) => {
                    // (#88) `TheAudio->setVolume`: the client's audio manager, once it is up
                    if let Some(sound) = g.audio.as_deref_mut() {
                        sound.audio.manager.set_volume(volume, affect);
                    }
                }
                // (#87) the quit menu's: the save popup's save (the saves panel's), the pause, the exit
                crate::shell::ShellAction::SaveGame { name, description } => {
                    save_action.0 = Some(SaveCmd::Save { name, description: Some(description) });
                }
                // (#108: with `setGamePaused`'s side effects)
                crate::shell::ShellAction::SetGamePaused(p) => g.set_game_paused(p),
                crate::shell::ShellAction::ClearGameData => {
                    let player = g.controls.client.player;
                    crate::shell::send_clear_game_data(&mut g.controls.client.world, player);
                    // (the game's end is watched whatever started it: `clearGameData` opens the score screen)
                    c.flow.watching = true;
                }
                // (#110) the quit menu's restart: a copy of the game being played waits for its map, then starts again
                crate::shell::ShellAction::RestartMission(r) => match c.flow.restart(&g.controls.client.world, r) {
                    Ok(()) => {
                        let map = c.flow.waiting_for().map(|m| String::from_utf8_lossy(m).into_owned()).unwrap_or_default();
                        c.set_status(format!("waiting {map}"));
                    }
                    Err(e) => {
                        warn!("the restart failed: {e}");
                        c.set_status(format!("error: {e}"));
                    }
                },
                crate::shell::ShellAction::None(why) => {
                    if !why.is_empty() {
                        debug!("shell: {why}");
                    }
                }
            }
        }
    }
    // (~~the clicks' sounds and the script hooks are not run yet~~; the untranslated callbacks are logged once)
    // (**corrected 2026-11-11, #136:** the windows' sounds are played, `update_audio`)
    g.gui_sounds.append(&mut ui.gui.audio);
    // **Corrected 2026-11-11, #142:** the script hooks run. THE OWNER'S EXCEPTION to "the client never changes the
    // simulation" (#139, owner decision 2026-11-11): upstream's menus call `TheScriptEngine->signalUIInteract` directly
    // (ScriptEngine.cpp:7266, Scripts.cpp:125), writing logic state no game message carries and no CRC covers. The
    // client makes that call itself ONLY when this machine is the sole player and no replay is being recorded (a
    // campaign mission, a challenge, a skirmish against the AI only with the recorder off); `apply_ui_interacts`
    // decides from the World (game mode, `TheNetwork`, the recorder's file and playback, the human players). Everywhere
    // else the frame's hooks are only recorded (logged here) and dropped, never applied.
    if !ui.gui.ui_interacts.is_empty() {
        let names = ui.gui.ui_interacts.join(", ");
        match rts_lockstep_engine::client::shell_hooks::apply_ui_interacts(&mut g.controls.client.world, &mut ui.gui.ui_interacts) {
            Ok(n) => debug!("shell: {n} script hook(s) to the script engine: {names}"),
            Err(why) => debug!("shell: script hook(s) recorded only ({why:?}): {names}"),
        }
    }
    for f in ui.gui.untranslated.drain(..) {
        debug!("shell: untranslated callback {}", String::from_utf8_lossy(&f));
    }
    let panel = crate::saves::page::panel_open() || crate::campaigns::page::panel_open();
    let quads = if panel { Vec::new() } else { ui.draw() };
    let (ww, wh) = windows.single().map_or((800.0, 600.0), |w| (w.width(), w.height()));
    let placement = crate::shell::shell_placement((ww, wh));
    st.placement = (!quads.is_empty()).then_some(placement);
    let (lw, lh) = (crate::ui::LAYOUT.0 as f32, crate::ui::LAYOUT.1 as f32);
    let root = match roots.iter_mut().next() {
        Some((e, mut r, mut node)) => {
            node.left = px(placement.left);
            node.top = px(placement.top);
            node.width = px(lw * placement.scale);
            node.height = px(lh * placement.scale);
            node.display = if quads.is_empty() { Display::None } else { Display::Flex };
            if r.0 == quads {
                return;
            }
            r.0 = quads.clone();
            commands.entity(e).despawn_children();
            e
        }
        None => {
            if quads.is_empty() {
                return;
            }
            let mut root = commands.spawn((
                ShellRoot(quads.clone()),
                GlobalZIndex(10),
                Node { position_type: PositionType::Absolute, left: px(placement.left), top: px(placement.top), width: px(lw * placement.scale), height: px(lh * placement.scale), ..default() },
            ));
            let gwm = rts_lockstep_engine::client::window_manager::gwm::MOUSE_POS;
            root.observe(move |ev: On<PointerMove>, mut shell: NonSendMut<ShellState>, mut game: NonSendMut<Game>| shell_pointer(&mut shell, &mut game, gwm, ev.pointer.position, &ev.pointer.id));
            root.observe(move |ev: On<PointerLeave>, mut shell: NonSendMut<ShellState>, mut game: NonSendMut<Game>| shell_pointer(&mut shell, &mut game, gwm, ev.pointer.position, &ev.pointer.id));
            root.observe(|ev: On<PointerPress>, mut shell: NonSendMut<ShellState>, mut game: NonSendMut<Game>| {
                if ev.button == PointerButton::Primary {
                    shell_pointer(&mut shell, &mut game, rts_lockstep_engine::client::window_manager::gwm::LEFT_DOWN, ev.pointer.position, &ev.pointer.id);
                }
            });
            root.observe(|ev: On<PointerRelease>, mut shell: NonSendMut<ShellState>, mut game: NonSendMut<Game>| {
                if ev.button == PointerButton::Primary {
                    shell_pointer(&mut shell, &mut game, rts_lockstep_engine::client::window_manager::gwm::LEFT_UP, ev.pointer.position, &ev.pointer.id);
                }
            });
            root.id()
        }
    };
    let collection = match &st.images {
        Some(Ok(i)) => i.clone(),
        _ => Default::default(),
    };
    let art = g.models.art.as_ref();
    let textures = &mut st.textures;
    spawn_quads(&mut commands, root, &quads, [0.0, 0.0, lw, lh], placement.scale, true, |n| art.and_then(|a| mapped_texture(a, textures, &collection, &mut images, n)));
}

/// GUI glue (needs a window): a pointer event over the shell, from the screen (logical pixels) through
/// [`crate::shell::ShellUi::pointer_at`] (tested there).
fn shell_pointer(shell: &mut ShellState, game: &mut Game, msg: u32, screen: Vec2, id: &bevy::picking::pointer::PointerId) {
    let touch = matches!(id, bevy::picking::pointer::PointerId::Touch(_));
    let Some(p) = shell.placement else { return };
    shell.last_pointer = Some((screen.x, screen.y));
    if let Some(ui) = shell.ui.as_mut() {
        // (#136) `winProcessMouseEvent` sets (or clears) the one mouse's tooltip, which `world_tooltip` draws
        ui.with_mouse(&mut game.pause.mouse.tooltip, |ui| ui.pointer_at(msg, (screen.x, screen.y), &p, touch));
    }
}

/// GUI glue (needs a window): the campaign panel, rebuilt when it opens or what it shows changes: Close and the difficulty;
/// the status and the briefing; the score screen's Continue (Retry after a defeat) when a mission has ended; then each
/// campaign's missions and the challenge's generals (`crate::campaigns::sections`, tested there), each a button. Full
/// width less the zoom column, rows wrapping, so a phone's screen fits it; the list scrolls.
fn campaign_panel(mut commands: Commands, game: NonSend<Game>, mut campaigns: NonSendMut<Campaigns>, panels: Query<Entity, With<CampaignPanel>>) {
    let open = crate::campaigns::page::panel_open();
    let want = open.then_some(campaigns.generation);
    if want == campaigns.built {
        return;
    }
    campaigns.built = want;
    for e in &panels {
        commands.entity(e).despawn();
    }
    if !open {
        return;
    }
    let small = |t: String| (Text::new(t), TextFont { font_size: bevy::text::FontSize::Px(14.0), ..default() }, TextColor(Color::WHITE));
    let root = commands
        .spawn((
            CampaignPanel,
            Node {
                position_type: PositionType::Absolute,
                top: px(36),
                left: px(8),
                right: px(64),
                max_height: percent(80),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                padding: UiRect::all(px(8)),
                overflow: Overflow::scroll_y(),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.08, 0.11, 0.94)),
        ))
        .id();
    let row = |commands: &mut Commands| commands.spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, align_items: AlignItems::Center, column_gap: px(6), row_gap: px(6), ..default() }).id();
    let header = row(&mut commands);
    let close = commands.spawn((bar_button("Close"), observe(|_: On<Activate>, mut a: ResMut<CampaignAction>| a.0 = Some(CampaignCmd::Toggle)))).id();
    let diff = format!("Difficulty: {}", crate::campaigns::DIFFICULTIES[campaigns.difficulty() as usize]);
    let difficulty = commands.spawn((bar_button(&diff), observe(|_: On<Activate>, mut a: ResMut<CampaignAction>| a.0 = Some(CampaignCmd::Difficulty)))).id();
    commands.entity(header).add_children(&[close, difficulty]);
    if let Some(over) = &campaigns.flow.over {
        let label = match (&over.next, over.victorious) {
            (Ok(_), true) => Some("Save and Continue"),
            (Ok(_), false) => Some("Retry"),
            (Err(_), _) => None,
        };
        if let Some(label) = label {
            let b = commands.spawn((bar_button(label), observe(|_: On<Activate>, mut a: ResMut<CampaignAction>| a.0 = Some(CampaignCmd::Continue)))).id();
            commands.entity(header).add_child(b);
        }
    }
    let mut status = campaigns.status.clone();
    if campaigns.flow.over.as_ref().is_some_and(|o| o.next.is_err()) {
        status.push_str(" | the campaign is complete");
    }
    if status.is_empty() {
        status = "choose a mission (its briefing movie cannot play: Bink was never released)".to_string();
    }
    let status = commands.spawn(small(status)).id();
    commands.entity(root).add_children(&[header, status]);
    if !campaigns.briefing.is_empty() {
        let b = commands.spawn(small(format!("Briefing: {}", campaigns.briefing.join(" | ")))).id();
        commands.entity(root).add_child(b);
    }
    let sections = crate::campaigns::sections(&game.controls.client.world);
    if sections.is_empty() {
        let none = commands.spawn(small("no campaigns: load your Zero Hour files (Data\\INI\\Campaign.ini)".to_string())).id();
        commands.entity(root).add_child(none);
    }
    for s in sections {
        let title = commands.spawn(small(s.title)).id();
        let list = row(&mut commands);
        for e in s.entries {
            let choice = e.choice.clone();
            let b = commands.spawn((list_button(&e.line), observe(move |_: On<Activate>, mut a: ResMut<CampaignAction>| a.0 = Some(CampaignCmd::Choose(choice.clone()))))).id();
            commands.entity(list).add_child(b);
        }
        commands.entity(root).add_children(&[title, list]);
    }
}

// ---------------------------------------------------------------------------
// The model viewer (#10, step 4): one W3D model, animated, drawn through `crate::model`.
// ---------------------------------------------------------------------------

/// Where the model viewer's model comes from.
pub enum ViewerSource {
    /// The generated placeholder robot ([`crate::model::demo_files`]): no game files needed.
    Demo,
    /// A model (and optionally an animation, `"HIERARCHY.ANIM"`) from the player's Zero Hour folder.
    #[cfg(not(target_arch = "wasm32"))]
    Folder { folder: std::path::PathBuf, model: String, anim: Option<String> },
}

/// The viewer's state: the scene, its drawn parts and their entities.
struct Viewer {
    scene: Option<crate::model::ModelScene>,
    parts: Vec<crate::model::Part>,
    status: String,
    /// The orbit camera's heading, distance and target (the model's bounding centre, Bevy axes).
    yaw: f32,
    distance: f32,
    target: Vec3,
    paused: bool,
}

/// A drawn part: its index in `Viewer::parts`, and its mesh (rewritten every frame for a skin).
#[derive(Component)]
struct PartOf(usize, Handle<Mesh>);

/// Runs the model viewer: `rts-client --model NAME [--anim HIERARCHY.ANIM] [--zh <folder>]` natively (the generated robot
/// without `--zh`). Space pauses the animation, the arrows turn and zoom.
///
/// GUI glue (needs a window): what it draws is `crate::model`'s, which its tests check without one.
pub fn run_viewer(source: ViewerSource) {
    let loaded = match source {
        ViewerSource::Demo => crate::model::ModelScene::load(&crate::model::demo_files(), b"ROBOT", Some(b"ROBOT.WAVE")),
        #[cfg(not(target_arch = "wasm32"))]
        ViewerSource::Folder { folder, model, anim } => {
            let mut fs = rts_lockstep_engine::common::file_system::FileSystem::new(rts_lockstep_engine::common::file_system::NativeDevice::new(&folder));
            fs.init(None);
            crate::model::ModelScene::load(&fs, model.as_bytes(), anim.as_deref().map(str::as_bytes))
        }
    };
    let (scene, parts, status) = match loaded.and_then(|s| crate::model::parts(&s).map(|p| (s, p))) {
        Ok((s, p)) => {
            let status = s.report.clone();
            (Some(s), p, status)
        }
        Err(e) => (None, Vec::new(), format!("the model did not load: {e}")),
    };
    info!("model viewer: {status}");
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "rts-lockstep-engine: model viewer".into(), canvas: Some("#rts".into()), fit_canvas_to_parent: true, ..default() }),
            ..default()
        }))
        .insert_non_send(Viewer { scene, parts, status, yaw: 0.6, distance: 80.0, target: Vec3::ZERO, paused: false })
        .add_systems(Startup, viewer_setup)
        .add_systems(Update, (viewer_input, viewer_animate, viewer_camera).chain())
        .run();
}

fn viewer_setup(
    mut commands: Commands,
    mut viewer: NonSendMut<Viewer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(0.0, 1.0, 0.0).looking_at(Vec3::new(-0.4, 0.0, -0.3), Vec3::Y),
    ));
    commands.spawn((Camera3d::default(), Transform::default()));
    commands.spawn((Hud, Text::new(viewer.status.clone()), Node { position_type: PositionType::Absolute, top: px(8), left: px(8), ..default() }));
    let Some(scene) = &viewer.scene else { return };
    // textures per sub-object, made once
    let handles: Vec<Vec<Option<Handle<Image>>>> = scene
        .textures
        .iter()
        .zip(&scene.model.sub_objects)
        .map(|(t, s)| t.iter().zip(&s.model.textures).map(|(r, tex)| r.as_ref().ok().map(|img| images.add(crate::model::texture_image(img, tex.clamp)))).collect())
        .collect();
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for (i, part) in viewer.parts.iter().enumerate() {
        let sub = &scene.model.sub_objects[part.sub];
        let (v, n) = scene.model.deformed(part.sub).unwrap_or_else(|_| (sub.model.vertices.clone(), sub.model.normals.clone()));
        let mesh = meshes.add(crate::model::group_mesh(&sub.model, &part.group, &v, &n));
        let material = materials.add(part.material.to_standard_material(&handles[part.sub]));
        let tm = scene.model.sub_object_transform(part.sub).map(|m| crate::model::to_bevy_matrix(&m)).unwrap_or(Mat4::IDENTITY);
        for p in &v {
            let w = tm.transform_point3(Vec3::from(to_bevy(*p)));
            (lo, hi) = (lo.min(w), hi.max(w));
        }
        commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material), Transform::from_matrix(tm), PartOf(i, mesh)));
    }
    if lo.x <= hi.x {
        viewer.target = (lo + hi) / 2.0;
        viewer.distance = ((hi - lo).length() * 1.5).max(1.0);
    }
}

fn viewer_input(keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, mut viewer: NonSendMut<Viewer>) {
    let dt = time.delta_secs();
    if keys.just_pressed(KeyCode::Space) {
        viewer.paused = !viewer.paused;
    }
    if keys.pressed(KeyCode::ArrowLeft) {
        viewer.yaw -= dt;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        viewer.yaw += dt;
    }
    if keys.pressed(KeyCode::ArrowUp) {
        viewer.distance = (viewer.distance * (1.0 - dt)).max(1.0);
    }
    if keys.pressed(KeyCode::ArrowDown) {
        viewer.distance *= 1.0 + dt;
    }
}

fn viewer_animate(
    time: Res<Time>,
    mut viewer: NonSendMut<Viewer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut parts: Query<(&PartOf, &mut Transform)>,
    mut hud: Query<&mut Text, With<Hud>>,
) {
    let v = &mut *viewer;
    let Some(scene) = &mut v.scene else { return };
    if v.paused {
        return;
    }
    if let Err(e) = scene.advance(time.delta_secs() * 1000.0) {
        v.status = format!("the animation stopped: {e}");
        v.paused = true;
    }
    for (PartOf(i, handle), mut transform) in &mut parts {
        let part = &v.parts[*i];
        if let Ok(m) = scene.model.sub_object_transform(part.sub) {
            *transform = Transform::from_matrix(crate::model::to_bevy_matrix(&m));
        }
        if scene.model.sub_objects[part.sub].is_skin() {
            if let (Ok((pos, nrm)), Some(mut mesh)) = (scene.model.deformed(part.sub), meshes.get_mut(handle)) {
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos.iter().map(|p| to_bevy(*p)).collect::<Vec<_>>());
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm.iter().map(|p| to_bevy(*p)).collect::<Vec<_>>());
            }
        }
    }
    for mut t in &mut hud {
        *t = Text::new(format!("{} | frame {:.1}{}", v.status, scene.frame, if v.paused { " (paused)" } else { "" }));
    }
}

fn viewer_camera(viewer: NonSend<Viewer>, mut cams: Query<&mut Transform, (With<Camera3d>, Without<PartOf>)>) {
    let (s, c) = (rts_lockstep_engine::ww::wwmath::sin(viewer.yaw), rts_lockstep_engine::ww::wwmath::cos(viewer.yaw));
    let eye = viewer.target + Vec3::new(s * viewer.distance, viewer.distance * 0.5, c * viewer.distance);
    for mut t in &mut cams {
        *t = Transform::from_translation(eye).looking_at(viewer.target, Vec3::Y);
    }
}
