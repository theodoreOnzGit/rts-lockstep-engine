// SPDX-License-Identifier: GPL-3.0-only
//! The Bevy application, milestone C1: the running game logic drawn without art (`docs/phase2-client.md`).
//!
//! - **The logic** steps in `FixedUpdate` at 30 Hz (`LOGICFRAMES_PER_SECOND`). After each frame the view model takes a
//!   [`Snapshot`]. The logic is the only thing that changes the game (see the crate's docs).
//! - **Drawing** (`Update`, every rendered frame) interpolates each object between the last two snapshots, by how far
//!   the fixed clock has run past its last step. One entity per logic object, spawned when it appears and despawned when
//!   it goes. The shapes are its collision geometry, in its player's colour. A health bar is drawn above each object with
//!   a body.
//! - **The camera** is an RTS camera: a focus point on the ground, a distance and a heading. Keys W/A/S/D or the arrows
//!   pan, Q/E turn, the wheel or +/- zoom, and a right- or middle-drag pans. On a touch screen one finger pans and two
//!   pinch. On-screen buttons zoom in, zoom out, reset the view and pause, so a phone needs no keyboard.
//!
//! - **The player's input** (C2d): a left click selects or orders, a left drag selects the player's units in the box; the
//!   pointer's hint (what a click would order) is in the status line; selected units have a ring; ~~the first selected
//!   unit's command buttons are along the bottom~~ **corrected 2026-11-05 (#12):** the first selected unit's command
//!   buttons are the game's control bar's when its files are loaded (below), else a debug panel along the bottom (also
//!   shown, above the bar, on a narrow screen); a construct button puts the building at the cursor until a click
//!   places it. Escape cancels a placement or a pending command. Everything goes through [`crate::input::Controls`], the
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
//!   `Controls::press`. Without the files the debug panel stays.
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
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
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
    /// (#35) When a command button's popup shows, the popups' lines by command slot, the selection they were made for, and
    /// whether the last pointer to press a button was a touch.
    tooltip: crate::ui::TooltipTimer,
    tooltip_lines: Vec<Option<Vec<String>>>,
    tooltip_for: Option<ObjectId>,
    touch_pressed: bool,
    /// (#34) The command buttons of the bar's slots this frame (`populateCommand`'s), for a press the manager selects.
    bar_buttons: Vec<Option<usize>>,
    /// The FX lists and particle systems still to be read from the art (#43, #44).
    fx_pending: bool,
    /// The audio's tables still to be read from the art (#13).
    audio_pending: bool,
    /// The audio (#13): the translated manager and the device, once its tables are read.
    audio: Option<Box<GameSound>>,
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
            tooltip: crate::ui::TooltipTimer::default(),
            tooltip_lines: Vec::new(),
            tooltip_for: None,
            touch_pressed: false,
            bar_buttons: Vec::new(),
            fx_pending: true,
            audio_pending: true,
            audio: None,
            radar_image: None,
            radar_drawn: None,
            radar_event_sounds: 0,
            radar_water_pending: true,
            base: None,
            save_state: Default::default(),
            ambient_restore_pending: false,
            serial: GAME_SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        }
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

/// The RTS camera's state: what it looks at, from how far, from which heading.
#[derive(Resource, Clone, Copy)]
struct Rig {
    /// The focus point on the ground, game coordinates.
    focus: Vec2,
    distance: f32,
    /// Heading about +z, radians.
    yaw: f32,
    /// Elevation above the ground plane, radians.
    pitch: f32,
    /// The view [`Rig::reset`] returns to.
    home: (Vec2, f32),
}

impl Rig {
    fn reset(&mut self) {
        (self.focus, self.distance) = self.home;
        self.yaw = 0.0;
    }
    fn zoom(&mut self, factor: f32) {
        self.distance = (self.distance * factor).clamp(40.0, 4000.0);
    }
    /// Moves the focus by `(right, forward)` in screen terms, world units.
    fn pan(&mut self, right: f32, forward: f32) {
        let (s, c) = self.yaw.sin_cos();
        // the camera looks along +y at yaw 0 (game coordinates): forward is (−sin, cos), right is (cos, sin)
        self.focus += Vec2::new(c * right - s * forward, s * right + c * forward);
    }
    fn transform(&self) -> Transform {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        // behind the focus (−forward) and up, game coordinates
        let eye = [self.focus.x + sy * cp * self.distance, self.focus.y - cy * cp * self.distance, sp * self.distance];
        let target = [self.focus.x, self.focus.y, 0.0];
        Transform::from_translation(Vec3::from(to_bevy(eye))).looking_at(Vec3::from(to_bevy(target)), Vec3::Y)
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
            (load_fx, update_audio, sync_objects, draw_particles, health_bars, keyboard_ui, camera_input, apply_camera, pointer_input, selection_rings, drag_box, load_game_ui, game_ui, radar_view, tooltip_panel, lift_zoom, control_bar, update_hud).chain(),
        )
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
            *game = Game::new(s.world, s.player, s.report, art);
            game.base = Some(Box::new(s.base));
            for (_, (e, _)) in drawn.0.drain() {
                commands.entity(e).despawn();
            }
            terrain.pending = true;
            let (centre, size) = view_frame(&game.controls.client.world);
            let distance = size * 0.6;
            *rig = Rig { focus: centre, distance, yaw: 0.0, pitch: 0.95, home: (centre, distance) };
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
    let (centre, size) = view_frame(&game.controls.client.world);
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(0.0, 1.0, 0.0).looking_at(Vec3::new(-0.4, 0.0, -0.3), Vec3::Y),
    ));
    let distance = size * 1.1;
    let rig = Rig { focus: centre, distance, yaw: 0.0, pitch: 0.95, home: (centre, distance) };
    commands.spawn((Camera3d::default(), rig.transform()));
    commands.insert_resource(rig);
}

fn step_logic(mut game: NonSendMut<Game>, rig: Option<Res<Rig>>) {
    if game.paused || game.error.is_some() {
        return;
    }
    let g = &mut *game;
    g.controls.observe_sent();
    // (`GameEngine::update`'s logic half: the message stream's commands, then the frame)
    match g.controls.world().engine_update() {
        Ok(()) => {
            // (#4: the frame, for the page's checks)
            crate::saves::page::set_frame(g.controls.client.world.frame());
            g.prev = std::mem::take(&mut g.cur);
            g.cur = snapshot(g.controls.world());
            // the frame's client events, to the drawables (their flags; the draw itself is every rendered frame)
            let events = g.controls.world().take_client_events();
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
                // #59: a loaded game's drawables' ambient sounds, once (`Drawable::xfer`'s and `loadPostProcess`'s)
                if g.ambient_restore_pending {
                    g.ambient_restore_pending = false;
                    let saved: Vec<_> = g.models.drawables.drawables.iter().map(|(id, d)| (*id, d.saved.ambient.clone())).collect();
                    sound.audio.ambient.restore_saved(&saved, &mut sound.audio.manager, &mut sound.device, &host);
                }
                for v in &voices {
                    sound.audio.play_unit_voice(&host, v);
                }
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
                let view = rig.as_deref().map_or_else(Default::default, |r| crate::audio::listener_view(r.focus.to_array(), r.yaw, r.pitch, r.distance));
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
    // the draw modules' clock: the client's own (upstream's sync time), not the logic's frames
    g.models_clock = (time.elapsed_secs_f64() * 1000.0) as u32;
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
    let right = Vec3::from(to_bevy([rts_lockstep_engine::ww::wwmath::cos(rig.yaw), rts_lockstep_engine::ww::wwmath::sin(rig.yaw), 0.0]));
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
/// typed. The modifier states (`KEY_STATE_CONTROL`, ...) are not passed yet.
fn keyboard_ui(mut events: MessageReader<KeyboardInput>, mut game: NonSendMut<Game>, mut taken: ResMut<UiKeyboard>, mut shell: NonSendMut<ShellState>) {
    // (#78) the shell, while it is shown, has the keyboard (`crate::shell::ShellUi::keyboard`, tested there)
    if let Some(s) = shell.ui.as_mut().filter(|s| s.is_active()) {
        for k in events.read() {
            s.keyboard(dik_of(k.key_code), k.state == ButtonState::Pressed, k.text.as_deref());
        }
        taken.0 = true;
        return;
    }
    let Some(ui) = game.ui.as_mut() else {
        events.clear();
        taken.0 = false;
        return;
    };
    for k in events.read() {
        ui.keyboard(dik_of(k.key_code), k.state == ButtonState::Pressed, k.text.as_deref());
    }
    taken.0 = ui.has_keyboard_focus();
}

fn camera_input(
    taken: Res<UiKeyboard>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    touches: Res<Touches>,
    time: Res<Time>,
    mut rig: ResMut<Rig>,
) {
    let dt = time.delta_secs();
    let speed = rig.distance * 0.9 * dt;
    // (#74) a UI window with the focus has the keyboard
    let empty = ButtonInput::<KeyCode>::default();
    let keys: &ButtonInput<KeyCode> = if taken.0 { &empty } else { &keys };
    let mut right = 0.0;
    let mut forward = 0.0;
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        right -= speed;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        right += speed;
    }
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        forward += speed;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        forward -= speed;
    }
    if keys.pressed(KeyCode::KeyQ) {
        rig.yaw += 1.5 * dt;
    }
    if keys.pressed(KeyCode::KeyE) {
        rig.yaw -= 1.5 * dt;
    }
    if keys.any_pressed([KeyCode::Equal, KeyCode::NumpadAdd]) {
        rig.zoom(1.0 - 1.2 * dt);
    }
    if keys.any_pressed([KeyCode::Minus, KeyCode::NumpadSubtract]) {
        rig.zoom(1.0 + 1.2 * dt);
    }
    if scroll.delta.y != 0.0 {
        rig.zoom((1.0 - scroll.delta.y.clamp(-5.0, 5.0) * 0.1).max(0.2));
    }
    // a right or middle drag moves the ground with the pointer (roughly: a pixel is distance/800 world units)
    if buttons.any_pressed([MouseButton::Right, MouseButton::Middle]) && motion.delta != Vec2::ZERO {
        let k = rig.distance / 800.0;
        right -= motion.delta.x * k;
        forward += motion.delta.y * k;
    }
    // touch: one finger pans, two pinch
    let active: Vec<_> = touches.iter().collect();
    match active.as_slice() {
        [t] => {
            let d = t.delta();
            let k = rig.distance / 800.0;
            right -= d.x * k;
            forward += d.y * k;
        }
        [a, b] => {
            let now = a.position().distance(b.position());
            let before = (a.position() - a.delta()).distance(b.position() - b.delta());
            if now > 1.0 && before > 1.0 {
                rig.zoom(before / now);
            }
        }
        _ => {}
    }
    if right != 0.0 || forward != 0.0 {
        rig.pan(right, forward);
    }
}

fn apply_camera(rig: Res<Rig>, mut cams: Query<&mut Transform, With<Camera3d>>) {
    for mut t in &mut cams {
        *t = rig.transform();
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
            (button("+"), observe(|_: On<Activate>, mut rig: ResMut<Rig>| rig.zoom(0.8))),
            (button("-"), observe(|_: On<Activate>, mut rig: ResMut<Rig>| rig.zoom(1.25))),
            (button("R"), observe(|_: On<Activate>, mut rig: ResMut<Rig>| rig.reset())),
            (button("||"), observe(|_: On<Activate>, mut game: NonSendMut<Game>| game.paused = !game.paused)),
            // (#4) the saves: the panel with the save button and the list
            (button("S"), observe(|_: On<Activate>, mut action: ResMut<SaveAction>| action.0 = Some(SaveCmd::Toggle))),
            // (#62) the campaigns: the missions and the Generals' Challenge, and the score screen
            (button("C"), observe(|_: On<Activate>, mut action: ResMut<CampaignAction>| action.0 = Some(CampaignCmd::Toggle))),
        ],
    ));
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

/// The left button's press, while it is held.
#[derive(Resource, Default)]
struct Pointer {
    press: Option<Vec2>,
}

/// The status bar's and the control bar's height, and the zoom column's width, in logical pixels: the pointer over them is
/// the UI's, not the game's.
const BAR_HEIGHT: f32 = 64.0;
const ZOOM_COLUMN: f32 = 64.0;

fn over_ui(cursor: Vec2, window: &Window, bar: f32) -> bool {
    cursor.y > window.height() - BAR_HEIGHT.max(bar) || cursor.x > window.width() - ZOOM_COLUMN || cursor.y < 28.0
}

/// The game point (z up) on the ground under `cursor`: where the camera ray first meets the logic's ground height
/// (`height(x, y)`, game axes; #11: the terrain is no longer flat), marched in steps of 2 units then halved 16 times; the
/// plane at height 0 when the ray meets no ground within 20,000 units.
fn ground_under(camera: &Camera, cam: &GlobalTransform, cursor: Vec2, height: impl Fn(f32, f32) -> f32) -> Option<Coord3D> {
    let ray = camera.viewport_to_world(cam, cursor).ok()?;
    // Bevy (x, y up, -game y) to the game's (x, y, z up)
    let game = |p: Vec3| Coord3D { x: p.x, y: -p.z, z: p.y };
    let above = |t: f32| {
        let p = ray.get_point(t);
        p.y > height(p.x, -p.z)
    };
    let (mut lo, mut hi) = (0.0f32, 0.0f32);
    let mut found = false;
    while hi < 20_000.0 {
        if !above(hi) {
            found = true;
            break;
        }
        lo = hi;
        hi += 2.0;
    }
    if !found {
        let d = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))?;
        return Some(game(ray.get_point(d)));
    }
    for _ in 0..16 {
        let mid = (lo + hi) / 2.0;
        if above(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let p = ray.get_point(hi);
    Some(Coord3D { x: p.x, y: -p.z, z: height(p.x, -p.z) })
}

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

/// GUI glue (needs a window): the left button, the pointer's hover and Escape, through [`Controls`].
fn pointer_input(
    mut game: NonSendMut<Game>,
    mut pointer: ResMut<Pointer>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    cams: Query<(&Camera, &GlobalTransform)>,
    bar: Res<UiBar>,
    radar_box: Res<RadarBox>,
    shell: NonSend<ShellState>,
) {
    let bar = bar.height + bar.panel;
    // (#4) the saves panel takes the pointer while it is open (the game is paused under it); (#78) so does the shell
    if crate::saves::page::panel_open() || crate::campaigns::page::panel_open() || shell.ui.as_ref().is_some_and(|s| s.is_active()) {
        pointer.press = None;
        return;
    }
    let (Ok(window), Ok((camera, cam))) = (windows.single(), cams.single()) else { return };
    let g = &mut *game;
    if keys.just_pressed(KeyCode::Escape) {
        let c = &mut g.controls;
        c.ui.place_build_available(None, None, 0.0, &c.client);
        c.ui.set_gui_command(None);
    }
    let Some(cursor) = window.cursor_position() else { return };
    let Some(ground) = ground_under(camera, cam, cursor, |x, y| g.controls.client.world.terrain.ground_height(x, y, false).0) else { return };
    let screen = on_screen(g, camera, cam);
    let picked = screen.iter().filter(|(_, p)| p.distance(cursor) < 18.0).min_by(|a, b| a.1.distance(cursor).total_cmp(&b.1.distance(cursor))).map(|(id, _)| *id);
    g.controls.client.view.on_screen = screen.iter().map(|(id, _)| *id).collect();
    let over_ui = |c: Vec2, w: &Window, b: f32| over_ui(c, w, b) || radar_box.contains(c);
    if buttons.just_pressed(MouseButton::Left) && !over_ui(cursor, window, bar) {
        pointer.press = Some(cursor);
        g.controls.left_press(pixel(cursor), ground);
    }
    if pointer.press.is_some() {
        g.controls.pointer_moved(pixel(cursor), ground);
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
    } else if !over_ui(cursor, window, bar) {
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

#[derive(Component)]
struct DragBox;

/// GUI glue (needs a window): the drag box, while the left button is held and moved.
fn drag_box(pointer: Res<Pointer>, windows: Query<&Window>, mut boxes: Query<(&mut Node, &mut Visibility), With<DragBox>>) {
    let Ok(window) = windows.single() else { return };
    for (mut node, mut vis) in &mut boxes {
        match (pointer.press, window.cursor_position()) {
            (Some(a), Some(b)) if a.distance(b) as i32 > crate::input::CLICK_TOLERANCE => {
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
fn update_audio(mut game: NonSendMut<Game>, time: Res<Time>) {
    let g = &mut *game;
    let Some(art) = &g.models.art else { return };
    let open = |p: &[u8]| {
        if !art.fs.file_ready(p) {
            return None;
        }
        Some(art.fs.open_file(p))
    };
    if g.audio_pending {
        // #55: `Eva::init`'s `Data\INI\Eva.ini` too (`None`: still arriving)
        let eva_ini = open(rts_lockstep_engine::client::eva::EVA_INI_FILE);
        if let (Some(eva_ini), Some((tables, errors))) = (eva_ini, crate::audio::load_tables(open)) {
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
    let Some(sound) = g.audio.as_deref_mut() else { return };
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
        crate::ui::UiBuild::Ready(ui) => {
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
/// [`Controls::press`] (`processCommandUI`). A release that happened off the bar ends the manager's grab here.
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
    shell: NonSend<ShellState>,
) {
    let g = &mut *game;
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
    // (#35) the popups of this selection's buttons; a new selection shows none
    if sel != g.tooltip_for {
        g.tooltip.reset();
        g.tooltip_for = sel;
    }
    g.tooltip_lines = state.tooltips.iter().map(|t| t.as_ref().map(crate::ui::tooltip_lines)).collect();
    let ui = g.ui.as_mut().expect("checked");
    // (#34) the windows as the state sets them; a grab whose release was not over the bar ends
    ui.sync(&state);
    if ui.wm.grab_window.is_some() && !buttons.pressed(MouseButton::Left) && touches.iter().next().is_none() {
        let _ = ui.pointer(rts_lockstep_engine::client::window_manager::gwm::LEFT_UP, (-1.0, -1.0), 0.0, false, &mut g.tooltip);
    }
    let ui = g.ui.as_ref().expect("checked");
    let quads = ui.frame(&state);
    g.bar_buttons = state.commands.iter().map(|s| s.as_ref().map(|s| s.button)).collect();
    // the box: the quads' extent, the layout's full width
    let (y0, y1) = quads.iter().fold((f32::MAX, f32::MIN), |(a, b), q| (a.min(q.rect[1]), b.max(q.rect[3])));
    if quads.is_empty() || y1 <= y0 {
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
                Node { position_type: PositionType::Absolute, bottom: px(0), left: px((ww - width) / 2.0), width: px(width), height: px(ui_bar.height), ..default() },
            ));
            // (#34) the pointer over the bar, into the window manager (events bubble up from the quads)
            root.observe(|ev: On<PointerMove>, time: Res<Time>, bar: Res<UiBar>, mut game: NonSendMut<Game>| {
                bar_pointer(&mut *game, &bar, rts_lockstep_engine::client::window_manager::gwm::MOUSE_POS, ev.pointer.position, &ev.pointer.id, time.elapsed_secs_f64());
            });
            root.observe(|ev: On<PointerLeave>, time: Res<Time>, bar: Res<UiBar>, mut game: NonSendMut<Game>| {
                bar_pointer(&mut *game, &bar, rts_lockstep_engine::client::window_manager::gwm::MOUSE_POS, ev.pointer.position, &ev.pointer.id, time.elapsed_secs_f64());
            });
            root.observe(|ev: On<PointerPress>, time: Res<Time>, bar: Res<UiBar>, mut game: NonSendMut<Game>| {
                if ev.button == PointerButton::Primary {
                    game.touch_pressed = matches!(ev.pointer.id, bevy::picking::pointer::PointerId::Touch(_));
                    bar_pointer(&mut *game, &bar, rts_lockstep_engine::client::window_manager::gwm::LEFT_DOWN, ev.pointer.position, &ev.pointer.id, time.elapsed_secs_f64());
                }
            });
            root.observe(|ev: On<PointerRelease>, time: Res<Time>, bar: Res<UiBar>, mut game: NonSendMut<Game>| {
                if ev.button == PointerButton::Primary {
                    bar_pointer(&mut *game, &bar, rts_lockstep_engine::client::window_manager::gwm::LEFT_UP, ev.pointer.position, &ev.pointer.id, time.elapsed_secs_f64());
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

/// (#34) GUI glue (needs a window): a pointer event over the bar, from the screen (logical pixels) into the layout
/// ([`crate::ui::LayoutPlacement`] inverted) and through [`crate::ui::GameUi::pointer`]; a command button the manager
/// selected presses through [`Controls::press`].
fn bar_pointer(game: &mut Game, bar: &UiBar, msg: u32, screen: Vec2, id: &bevy::picking::pointer::PointerId, now: f64) {
    let Some(p) = bar.placement else { return };
    let touch = matches!(id, bevy::picking::pointer::PointerId::Touch(_));
    let pos = p.to_layout((screen.x, screen.y));
    let g = &mut *game;
    let Some(ui) = g.ui.as_mut() else { return };
    let Some(slot) = ui.pointer(msg, pos, now, touch, &mut g.tooltip) else { return };
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

/// The tactical view's corners on the plane of height `z` (`getScreenCornerWorldPointsAtZ`: upper left, upper right, lower
/// right, lower left), game axes; `None` if a corner's ray does not meet it.
fn screen_corners_at_z(camera: &Camera, cam: &GlobalTransform, size: Vec2, z: f32) -> Option<[Coord3D; 4]> {
    let at = |p: Vec2| -> Option<Coord3D> {
        let ray = camera.viewport_to_world(cam, p).ok()?;
        let d = ray.intersect_plane(Vec3::new(0.0, z, 0.0), InfinitePlane3d::new(Vec3::Y))?;
        let w = ray.get_point(d);
        Some(Coord3D { x: w.x, y: -w.z, z })
    };
    Some([at(Vec2::ZERO)?, at(Vec2::new(size.x, 0.0))?, at(size)?, at(Vec2::new(0.0, size.y))?])
}

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
    cams: Query<(&Camera, &GlobalTransform)>,
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
    let view = cams.single().ok().and_then(|(camera, cam)| screen_corners_at_z(camera, cam, Vec2::new(screen.0, screen.1), g.controls.radar.radar.terrain_average_z));
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
                        rig.focus = Vec2::new(at.x, at.y);
                    }
                });
        }
    }
}

/// The popup description's panel (#35), and the lines it shows.
#[derive(Component)]
struct TooltipPanel(Vec<String>);

/// GUI glue (needs a window): the shown command button's popup (`ControlBarPopupDescription.wnd`'s name, cost and
/// description, from [`crate::ui::tooltip_lines`]; when, from [`crate::ui::TooltipTimer`], both tested in `ui`), as a
/// panel just above the control bar, no wider than the screen less its 8-pixel margins (a phone's too). Rebuilt when its
/// lines change; none while nothing is shown.
fn tooltip_panel(mut commands: Commands, time: Res<Time>, game: NonSend<Game>, mut panels: Query<(Entity, &mut TooltipPanel, &mut Node)>, windows: Query<&Window>, ui_bar: Res<UiBar>) {
    let lines = game.tooltip.shown(time.elapsed_secs_f64()).and_then(|s| game.tooltip_lines.get(s).cloned().flatten());
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
    Save,
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
        SaveCmd::Save => {
            let g = &mut *game;
            // (`View::xfer`: the camera's heading and the point it looks at)
            g.save_state.view.angle = rig.yaw;
            g.save_state.view.position = Coord3D { x: rig.focus.x, y: rig.focus.y, z: 0.0 };
            let frame = g.controls.client.world.frame();
            let desc = format!("{} | frame {frame}", g.playing.split(" | ").next().unwrap_or(""));
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
                    let name = saves.folder.next_name();
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
                    rig.yaw = g.save_state.view.angle;
                    rig.focus = Vec2::new(g.save_state.view.position.x, g.save_state.view.position.y);
                    **game = g;
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
    let save = commands.spawn((bar_button("Save game"), observe(|_: On<Activate>, mut a: ResMut<SaveAction>| a.0 = Some(SaveCmd::Save)))).id();
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
                *game = Game::new(s.world, s.player, s.label.clone(), art);
                game.base = base;
                for (_, (e, _)) in drawn.0.drain() {
                    commands.entity(e).despawn();
                }
                terrain.pending = true;
                let (centre, size) = view_frame(&game.controls.client.world);
                let distance = size * 0.6;
                *rig = Rig { focus: centre, distance, yaw: 0.0, pitch: 0.95, home: (centre, distance) };
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
}

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
) {
    let st = &mut *shell;
    let Some(ui) = st.ui.as_mut() else { return };
    let g = &mut *game;
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
    if ui.is_active() {
        ui.frame((time.elapsed_secs_f64() * 1000.0) as u32);
        if ui.wm.grab_window.is_some() && !buttons.pressed(MouseButton::Left) && touches.iter().next().is_none() {
            match (st.last_pointer, st.placement) {
                (Some(at), Some(p)) => ui.pointer_at(rts_lockstep_engine::client::window_manager::gwm::LEFT_UP, at, &p, false),
                _ => ui.pointer(rts_lockstep_engine::client::window_manager::gwm::LEFT_UP, (-1.0, -1.0), false),
            }
        }
    }
    if let Some(base) = g.base.as_deref() {
        let c = &mut *campaigns;
        for a in ui.take_actions(base, &mut c.flow) {
            match a {
                crate::shell::ShellAction::CampaignChosen | crate::shell::ShellAction::SkirmishChosen => {
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
                crate::shell::ShellAction::None(why) => {
                    if !why.is_empty() {
                        debug!("shell: {why}");
                    }
                }
            }
        }
    }
    // (the clicks' sounds and the script hooks are not played or run yet; the untranslated callbacks are logged once)
    ui.gui.audio.clear();
    ui.gui.ui_interacts.clear();
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
            root.observe(move |ev: On<PointerMove>, mut shell: NonSendMut<ShellState>| shell_pointer(&mut shell, gwm, ev.pointer.position, &ev.pointer.id));
            root.observe(move |ev: On<PointerLeave>, mut shell: NonSendMut<ShellState>| shell_pointer(&mut shell, gwm, ev.pointer.position, &ev.pointer.id));
            root.observe(|ev: On<PointerPress>, mut shell: NonSendMut<ShellState>| {
                if ev.button == PointerButton::Primary {
                    shell_pointer(&mut shell, rts_lockstep_engine::client::window_manager::gwm::LEFT_DOWN, ev.pointer.position, &ev.pointer.id);
                }
            });
            root.observe(|ev: On<PointerRelease>, mut shell: NonSendMut<ShellState>| {
                if ev.button == PointerButton::Primary {
                    shell_pointer(&mut shell, rts_lockstep_engine::client::window_manager::gwm::LEFT_UP, ev.pointer.position, &ev.pointer.id);
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
fn shell_pointer(shell: &mut ShellState, msg: u32, screen: Vec2, id: &bevy::picking::pointer::PointerId) {
    let touch = matches!(id, bevy::picking::pointer::PointerId::Touch(_));
    let Some(p) = shell.placement else { return };
    shell.last_pointer = Some((screen.x, screen.y));
    if let Some(ui) = shell.ui.as_mut() {
        ui.pointer_at(msg, (screen.x, screen.y), &p, touch);
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
    let (s, c) = viewer.yaw.sin_cos();
    let eye = viewer.target + Vec3::new(s * viewer.distance, viewer.distance * 0.5, c * viewer.distance);
    for mut t in &mut cams {
        *t = Transform::from_translation(eye).looking_at(viewer.target, Vec3::Y);
    }
}
