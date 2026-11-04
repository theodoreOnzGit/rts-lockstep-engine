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
//! GUI glue: the drawing systems need a window and a GPU, so no test reaches them. What they draw is the view model's,
//! which `view`'s tests check against the `World`.

use std::collections::HashMap;

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};

use rts_lockstep_engine::logic::world::game_logic::World as GameWorld;
use rts_lockstep_engine::logic::world::gate_scenario::gate_world;

use crate::view::{interpolate, snapshot, terrain_grid, to_bevy, Shape, Snapshot, TerrainGrid};

/// The logic's frame rate (`LOGICFRAMES_PER_SECOND`).
const LOGIC_HZ: f64 = 30.0;

/// The game: the logic's `World` and the last two snapshots. A non-send resource: the `World` is the logic's, used only
/// from the main thread, as upstream's single-threaded logic is.
struct Game {
    world: GameWorld,
    prev: Snapshot,
    cur: Snapshot,
    paused: bool,
    /// The logic's error, if a frame failed; the game stops there and the HUD shows it.
    error: Option<String>,
}

/// The drawn entity of each logic object, by `ObjectID`.
#[derive(Resource, Default)]
struct Drawn(HashMap<i32, Entity>);

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

/// Runs the client on the gate game (milestone C1).
pub fn run() {
    let mut world = gate_world();
    let cur = snapshot(&mut world);
    let terrain = terrain_grid(&world, 10.0);
    let game = Game { world, prev: cur.clone(), cur, paused: false, error: None };

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
        .add_systems(Startup, (setup_scene, setup_ui))
        .add_systems(FixedUpdate, step_logic)
        .add_systems(Update, (sync_objects, health_bars, camera_input, apply_camera, update_hud).chain())
        .run();
}

#[derive(Resource)]
struct TerrainData(Option<TerrainGrid>);

fn setup_scene(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, terrain: Res<TerrainData>) {
    let (centre, size) = match &terrain.0 {
        Some(g) => {
            let w = (g.nx - 1) as f32 * g.step;
            let h = (g.ny - 1) as f32 * g.step;
            commands.spawn((
                Mesh3d(meshes.add(terrain_mesh(g))),
                MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.32, 0.42, 0.26), perceptual_roughness: 1.0, ..default() })),
            ));
            (Vec2::new(w / 2.0, h / 2.0), w.max(h))
        }
        None => (Vec2::new(200.0, 200.0), 400.0),
    };
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
    match g.world.logic_update() {
        Ok(()) => {
            g.prev = std::mem::take(&mut g.cur);
            g.cur = snapshot(&mut g.world);
        }
        Err(e) => g.error = Some(format!("logic frame {}: {e:?}", g.world.frame())),
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

fn sync_objects(
    mut commands: Commands,
    game: NonSend<Game>,
    fixed: Res<Time<Fixed>>,
    mut drawn: ResMut<Drawn>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut transforms: Query<&mut Transform, Without<Body>>,
) {
    let t = if game.paused || game.error.is_some() { 1.0 } else { fixed.overstep_fraction() };
    let mut alive = std::collections::HashSet::new();
    for b in &game.cur.objects {
        alive.insert(b.id);
        let o = match game.prev.find(b.id) {
            Some(a) => interpolate(a, b, t),
            None => b.clone(),
        };
        let transform = Transform::from_translation(Vec3::from(to_bevy(o.pos))).with_rotation(Quat::from_rotation_y(o.angle));
        match drawn.0.get(&b.id) {
            Some(&e) => {
                if let Ok(mut tr) = transforms.get_mut(e) {
                    *tr = transform;
                }
            }
            None => {
                let (mesh, lift) = shape_mesh(o.shape);
                let [r, g, bl] = o.color;
                let material = materials.add(StandardMaterial { base_color: Color::srgb(r, g, bl), perceptual_roughness: 0.7, ..default() });
                let e = commands
                    .spawn((transform, Visibility::default(), Name::new(format!("{} #{}", o.template, o.id))))
                    .with_children(|c| {
                        c.spawn((Body, Mesh3d(meshes.add(mesh)), MeshMaterial3d(material), Transform::from_xyz(0.0, lift, 0.0)));
                    })
                    .id();
                drawn.0.insert(b.id, e);
            }
        }
    }
    drawn.0.retain(|id, e| {
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
        **text = format!("logic frame {} | {} objects | {state}", game.cur.frame, game.cur.objects.len());
    }
}

/// The on-screen controls (zoom in, zoom out, reset, pause) and the status line. The buttons are the controls a phone
/// has; the keys and the mouse are extras.
fn setup_ui(mut commands: Commands) {
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
