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
//!   pointer's hint (what a click would order) is in the status line; selected units have a ring; the first selected
//!   unit's command buttons are along the bottom, and a construct button puts the building at the cursor until a click
//!   places it. Escape cancels a placement or a pending command. Everything goes through [`crate::input::Controls`], the
//!   translated client: the window only says what is under the pointer. Objects are picked by their projected position
//!   (within 18 pixels), the ground by the camera ray meeting the ground plane (the demo's ground is flat).
//!
//! - **Models** (#25): after each logic frame the frame's client events go to the drawables (`crate::drawables`); every
//!   rendered frame the draw modules choose and time each object's model and animation, and an object whose model the
//!   art has is drawn as it (its parts at their bones, skins deformed), else as its shape. Natively with `--zh` the art
//!   is the player's folder; ~~the demo and the web page have none yet (#24), so they draw shapes~~ **corrected
//!   2026-11-05 (#24):** on the web it is the page's imported files, the art archives read by range as their members are
//!   needed (`crate::import`); the demo has none, so it draws shapes.
//!
//! GUI glue: the drawing systems need a window and a GPU, so no test reaches them. What they draw is the view model's,
//! which `view`'s tests check against the `World`; what the input does is `input`'s, which its tests drive.

use std::collections::HashMap;

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};

use rts_lockstep_engine::common::base_type::{Coord3D, ICoord2D};
use rts_lockstep_engine::common::game_type::ObjectId;

use rts_lockstep_engine::logic::world::game_logic::World as GameWorld;

use crate::demo::demo_world;
use crate::drawables::{Art, WorldDrawables};
use rts_lockstep_engine::common::file_system::LocalDevice;
use crate::input::Controls;
use crate::view::{interpolate, snapshot, terrain_grid, to_bevy, Shape, Snapshot, TerrainGrid};

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
}

impl Game {
    fn new(world: GameWorld, player: i32, playing: String, art: Option<Art<ArtDevice>>) -> Self {
        let mut world = world;
        let cur = snapshot(&mut world);
        Game { controls: Controls::new(world, player), prev: cur.clone(), cur, paused: false, error: None, playing, models: WorldDrawables::new(art), images: HashMap::new(), models_clock: 0 }
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
                Game::new(s.world, s.player, s.report, Some(Art::new(fs)))
            }
            Err(e) => {
                let (w, _) = demo_world();
                Game::new(w, 1, format!("demo ({e})"), None)
            }
        },
        _ => {
            let (w, _) = demo_world();
            Game::new(w, 1, "demo game: load your Zero Hour folder to play".to_string(), None)
        }
    };
    let terrain = terrain_grid(&game.controls.client.world, 10.0);

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
        .insert_resource(TerrainData(terrain))
        .init_resource::<Drawn>()
        .init_resource::<Pointer>()
        .add_systems(Startup, (setup_scene, setup_ui))
        .add_systems(FixedUpdate, step_logic)
        .add_systems(Update, (poll_import, pump_art).chain())
        .add_systems(
            Update,
            (sync_objects, health_bars, camera_input, apply_camera, pointer_input, selection_rings, drag_box, control_bar, update_hud).chain(),
        )
        .run();
}

#[derive(Resource)]
struct TerrainData(Option<TerrainGrid>);

#[derive(Component)]
struct Ground;

/// The terrain's mesh (marked [`Ground`]), and the view's centre and size.
fn spawn_ground(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, terrain: &Option<TerrainGrid>) -> (Vec2, f32) {
    match terrain {
        Some(g) => {
            let w = (g.nx - 1) as f32 * g.step;
            let h = (g.ny - 1) as f32 * g.step;
            commands.spawn((
                Ground,
                Mesh3d(meshes.add(terrain_mesh(g))),
                MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.32, 0.42, 0.26), perceptual_roughness: 1.0, ..default() })),
            ));
            (Vec2::new(w / 2.0, h / 2.0), w.max(h))
        }
        None => (Vec2::new(200.0, 200.0), 400.0),
    }
}

/// GUI glue (needs a window): the player's files, once the page has handed them over, replace the demo with their game.
#[allow(clippy::too_many_arguments)]
fn poll_import(
    mut commands: Commands,
    mut game: NonSendMut<Game>,
    mut drawn: ResMut<Drawn>,
    mut rig: ResMut<Rig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    grounds: Query<Entity, With<Ground>>,
) {
    let Some(files) = crate::import::take_import() else { return };
    game.playing = "loading your Zero Hour files...".to_string();
    match crate::import::start_from_files(files, None) {
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
            for (_, (e, _)) in drawn.0.drain() {
                commands.entity(e).despawn();
            }
            for e in &grounds {
                commands.entity(e).despawn();
            }
            let terrain = terrain_grid(&game.controls.client.world, 10.0);
            let (centre, size) = spawn_ground(&mut commands, &mut meshes, &mut materials, &terrain);
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
fn pump_art(mut game: NonSendMut<Game>) {
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
    crate::import::set_art_status(format!(
        "models={} names={} remade={} missing={}{device}",
        g.models.instance_count(),
        models.into_iter().map(|n| String::from_utf8_lossy(n).into_owned()).collect::<Vec<_>>().join(","),
        g.models.remade,
        g.models.missing.len()
    ));
}

fn setup_scene(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, terrain: Res<TerrainData>) {
    let (centre, size) = spawn_ground(&mut commands, &mut meshes, &mut materials, &terrain.0);
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(0.0, 1.0, 0.0).looking_at(Vec3::new(-0.4, 0.0, -0.3), Vec3::Y),
    ));
    let distance = size * 1.1;
    let rig = Rig { focus: centre, distance, yaw: 0.0, pitch: 0.95, home: (centre, distance) };
    commands.spawn((Camera3d::default(), rig.transform()));
    commands.insert_resource(rig);
}

/// The height map as a triangle mesh, in Bevy's axes, with smooth normals.
fn terrain_mesh(g: &TerrainGrid) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    let mut positions = Vec::with_capacity(g.nx * g.ny);
    for j in 0..g.ny {
        for i in 0..g.nx {
            positions.push(to_bevy([i as f32 * g.step, j as f32 * g.step, g.heights[j * g.nx + i]]));
        }
    }
    let mut indices = Vec::with_capacity((g.nx - 1) * (g.ny - 1) * 6);
    for j in 0..g.ny - 1 {
        for i in 0..g.nx - 1 {
            let a = (j * g.nx + i) as u32;
            let b = a + 1;
            let c = a + g.nx as u32;
            let d = c + 1;
            // counter-clockwise seen from above in Bevy's axes (game +y is Bevy −z)
            indices.extend_from_slice(&[a, b, d, a, d, c]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_indices(Indices::U32(indices));
    mesh.compute_smooth_normals();
    mesh
}

fn step_logic(mut game: NonSendMut<Game>) {
    if game.paused || game.error.is_some() {
        return;
    }
    let g = &mut *game;
    g.controls.observe_sent();
    // (`GameEngine::update`'s logic half: the message stream's commands, then the frame)
    match g.controls.world().engine_update() {
        Ok(()) => {
            g.prev = std::mem::take(&mut g.cur);
            g.cur = snapshot(g.controls.world());
            // the frame's client events, to the drawables (their flags; the draw itself is every rendered frame)
            let events = g.controls.world().take_client_events();
            let now = g.models_clock;
            g.models.apply_events(g.controls.world(), &events, now);
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
    mut parts: Query<(&ModelPart, &mut Transform), Without<Body>>,
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
        let transform = Transform::from_translation(Vec3::from(to_bevy(o.pos))).with_rotation(Quat::from_rotation_y(o.angle));
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
                        let material = materials.add(part.material.to_standard_material(handles.get(part.sub).unwrap_or(&no_textures)));
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
    // the models' poses: each part at its bone, each skin deformed
    for (part, mut transform) in &mut parts {
        let Some(m) = g.models.instance(part.object) else { continue };
        if let Ok(x) = m.sub_object_transform(part.sub) {
            *transform = Transform::from_matrix(crate::model::to_bevy_matrix(&x));
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
    let right = Vec3::from(to_bevy([rig.yaw.cos(), rig.yaw.sin(), 0.0]));
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

fn camera_input(
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
            "{} | logic frame {} | {} objects{art} | {} selected{hint}{placing} | {state}",
            game.playing,
            game.cur.frame,
            game.cur.objects.len(),
            game.controls.ui.select_count()
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

fn over_ui(cursor: Vec2, window: &Window) -> bool {
    cursor.y > window.height() - BAR_HEIGHT || cursor.x > window.width() - ZOOM_COLUMN || cursor.y < 28.0
}

/// The game point (z up) on the ground plane under `cursor`.
fn ground_under(camera: &Camera, cam: &GlobalTransform, cursor: Vec2) -> Option<Coord3D> {
    let ray = camera.viewport_to_world(cam, cursor).ok()?;
    let d = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))?;
    let p = ray.get_point(d);
    // Bevy (x, y up, -game y) to the game's (x, y, z up)
    Some(Coord3D { x: p.x, y: -p.z, z: p.y })
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
) {
    let (Ok(window), Ok((camera, cam))) = (windows.single(), cams.single()) else { return };
    let g = &mut *game;
    if keys.just_pressed(KeyCode::Escape) {
        let c = &mut g.controls;
        c.ui.place_build_available(None, None, 0.0, &c.client);
        c.ui.set_gui_command(None);
    }
    let Some(cursor) = window.cursor_position() else { return };
    let Some(ground) = ground_under(camera, cam, cursor) else { return };
    let screen = on_screen(g, camera, cam);
    let picked = screen.iter().filter(|(_, p)| p.distance(cursor) < 18.0).min_by(|a, b| a.1.distance(cursor).total_cmp(&b.1.distance(cursor))).map(|(id, _)| *id);
    g.controls.client.view.on_screen = screen.iter().map(|(id, _)| *id).collect();
    if buttons.just_pressed(MouseButton::Left) && !over_ui(cursor, window) {
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
    } else if !over_ui(cursor, window) {
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
fn control_bar(mut commands: Commands, mut game: NonSendMut<Game>, mut bars: Query<(Entity, &mut ControlBar)>) {
    let changed = game.controls.ui.frame_selection_changed();
    for (e, mut bar) in &mut bars {
        if bar.shown == changed {
            continue;
        }
        bar.shown = changed;
        commands.entity(e).despawn_children();
        for b in game.controls.bar_buttons() {
            let label = b.label.trim_start_matches("Command_").to_string();
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
