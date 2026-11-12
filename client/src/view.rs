// SPDX-License-Identifier: GPL-3.0-only
//! The view model: what the client draws, read from the game logic's `World` after a logic frame. Plain Rust, no Bevy.
//!
//! Milestone C1 draws no art. Each object is its own collision geometry (`GeometryInfo`: a sphere, a cylinder or a box at
//! its radii and height), at its position and orientation, in its player's indicator colour, with a health bar. ~~The
//! terrain is the logic's height map, sampled on a grid.~~ **Corrected 2026-11-05 (#11):** the terrain is drawn by
//! `crate::terrain` from the game's own vertices (`updateVB`); [`terrain_grid`] (the logic's ground height on a grid)
//! is no longer drawn, and stays for its test.
//!
//! **Coordinates.** The game is right-handed with z up; Bevy is right-handed with y up. A game point `(x, y, z)` is drawn at
//! `(x, z, -y)` ([`to_bevy`]), which keeps handedness, so a game rotation by `angle` about +z is a Bevy rotation by the same
//! `angle` about +y.
//!
//! **Reading only.** [`snapshot`] takes `&mut World` because a body's health is a module call
//! (`getBodyModule()->getHealth()`). The tests check that taking a snapshot every frame leaves the logic's CRCs exactly as
//! the gate game's without it.

use rts_lockstep_engine::common::geometry::GeometryType;
use rts_lockstep_engine::logic::world::game_logic::World;

/// An object's drawn shape: its collision geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// `GEOMETRY_SPHERE`.
    Sphere { radius: f32 },
    /// `GEOMETRY_CYLINDER`: standing on its position.
    Cylinder { radius: f32, height: f32 },
    /// `GEOMETRY_BOX`: half-lengths forward (the object's x axis) and to the side; standing on its position.
    Box { half_forward: f32, half_side: f32, height: f32 },
}

/// One object as drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewObject {
    /// The logic's `ObjectID`.
    pub id: i32,
    /// Its template's name.
    pub template: String,
    /// Its position, game coordinates (z up).
    pub pos: [f32; 3],
    /// Its orientation about +z, radians.
    pub angle: f32,
    pub shape: Shape,
    /// Its controlling player's indicator colour, linear-ish sRGB components 0..1 (`getIndicatorColor()`).
    pub color: [f32; 3],
    /// Health over maximum health, for an object with a body; `None` without one.
    pub health: Option<f32>,
}

/// Every object of a logic frame, in the logic's own object order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    /// The logic frame this was taken after.
    pub frame: u32,
    pub objects: Vec<ViewObject>,
}

impl Snapshot {
    /// The object with this id.
    pub fn find(&self, id: i32) -> Option<&ViewObject> {
        self.objects.iter().find(|o| o.id == id)
    }
}

/// The objects to draw, read from the `World` after its last frame. Destroyed objects are left out (the logic removes
/// them at the end of the frame; one destroyed in this frame is already gone from what the player should see).
pub fn snapshot(w: &mut World) -> Snapshot {
    let ids = w.objects.ids_in_order();
    let mut objects = Vec::with_capacity(ids.len());
    for id in ids {
        let Some(o) = w.find_object(id) else { continue };
        if o.is_destroyed() {
            continue;
        }
        let p = o.get_position();
        let g = &o.geometry_info;
        let height = g.get_max_height_above_position();
        let shape = match g.get_geom_type() {
            GeometryType::Sphere => Shape::Sphere { radius: g.get_major_radius() },
            GeometryType::Cylinder => Shape::Cylinder { radius: g.get_major_radius(), height },
            GeometryType::Box => Shape::Box { half_forward: g.get_major_radius(), half_side: g.get_minor_radius(), height },
        };
        let template = String::from_utf8_lossy(o.template().get_name()).into_owned();
        let angle = o.thing.orientation();
        let player = w.get_controlling_player(id);
        let color = object_color(w.get_indicator_color(id), player);
        let health = w.body_health(id).map(|(h, max)| if max > 0.0 { (h / max).clamp(0.0, 1.0) } else { 0.0 });
        objects.push(ViewObject { id, template, pos: [p.x, p.y, p.z], angle, shape, color, health });
    }
    Snapshot { frame: w.frame(), objects }
}

/// The colour an object is drawn in: its indicator colour (`getIndicatorColor()`: its own, else its controlling player's).
/// A player whose colour is white is drawn from [`PALETTE`] by player index instead. White is what a player gets when its
/// map side sets no `playerColor` and its template's preferred colour is white, as the gate map's two players do
/// (`Player::initFromDict`; their CRCs are the logic's and are not changed). On a real map each player's colour is set by
/// the game setup and is drawn as it is. This is the client's choice of drawing only.
pub fn object_color(indicator: i32, player: Option<i32>) -> [f32; 3] {
    if indicator as u32 & 0x00ff_ffff == 0x00ff_ffff {
        if let Some(p) = player {
            return PALETTE[p.rem_euclid(PALETTE.len() as i32) as usize];
        }
    }
    argb_to_rgb(indicator)
}

/// Stand-in player colours, by player index (index 0 is the neutral player).
pub const PALETTE: [[f32; 3]; 8] = [
    [0.6, 0.6, 0.6],
    [0.15, 0.4, 0.95],
    [0.9, 0.15, 0.1],
    [0.95, 0.8, 0.1],
    [0.1, 0.75, 0.3],
    [0.8, 0.35, 0.9],
    [0.95, 0.5, 0.1],
    [0.1, 0.8, 0.85],
];

/// A game colour (`0xAARRGGBB`, as `getIndicatorColor` gives it) as RGB components 0..1. An object of no player (0) is
/// drawn grey.
pub fn argb_to_rgb(c: i32) -> [f32; 3] {
    if c == 0 {
        return [0.6, 0.6, 0.6];
    }
    let c = c as u32;
    let ch = |s: u32| ((c >> s) & 0xff) as f32 / 255.0;
    [ch(16), ch(8), ch(0)]
}

/// A game point `(x, y, z)` (z up) in Bevy's axes (y up): `(x, z, -y)`.
pub fn to_bevy(p: [f32; 3]) -> [f32; 3] {
    [p[0], p[2], -p[1]]
}

/// An object between two snapshots, `t` from 0 (`a`) to 1 (`b`): position linearly, orientation the short way round.
/// The shape, colour and health are `b`'s.
pub fn interpolate(a: &ViewObject, b: &ViewObject, t: f32) -> ViewObject {
    let t = t.clamp(0.0, 1.0);
    let mut o = b.clone();
    for i in 0..3 {
        o.pos[i] = a.pos[i] + (b.pos[i] - a.pos[i]) * t;
    }
    let tau = core::f32::consts::TAU;
    let mut d = (b.angle - a.angle) % tau;
    if d > core::f32::consts::PI {
        d -= tau;
    } else if d < -core::f32::consts::PI {
        d += tau;
    }
    o.angle = a.angle + d * t;
    o
}

/// The terrain's height map sampled on a grid, for drawing.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainGrid {
    /// The grid's spacing, world units.
    pub step: f32,
    /// Points along x and along y.
    pub nx: usize,
    pub ny: usize,
    /// Heights, row by row in y, each row along x: point `(i, j)` is at `(i * step, j * step, heights[j * nx + i])`.
    pub heights: Vec<f32>,
}

/// The logic's ground height (`TheTerrainLogic->getGroundHeight`) on a grid over the map's extent (the active boundary),
/// `step` apart. `None` when the terrain has no extent (no map loaded).
pub fn terrain_grid(w: &World, step: f32) -> Option<TerrainGrid> {
    let e = w.terrain.extent().ok()?;
    if e.hi.x <= 0.0 || e.hi.y <= 0.0 || step <= 0.0 {
        return None;
    }
    let nx = (e.hi.x / step) as usize + 1;
    let ny = (e.hi.y / step) as usize + 1;
    let mut heights = Vec::with_capacity(nx * ny);
    for j in 0..ny {
        for i in 0..nx {
            heights.push(w.terrain.ground_height(i as f32 * step, j as f32 * step, false).0);
        }
    }
    Some(TerrainGrid { step, nx, ny, heights })
}

/// (#146, #148) One client frame of the display's cinematic text: `ticks` drawn frames (30 a second, at most 4 after a
/// stall) of `W3DDisplay::draw`'s block, each taking a frame from the count; the last frame's draw, and whether the
/// client's button column (`+ - R < > || S C B X`, the zoom column a phone needs) shows. **Owner decision 2026-11-12
/// (#147, #148):** the column hides while cinematic text is showing (a frame drew it) and comes back when the text
/// ends (new: upstream has no such column; the letterbox stays upstream's).
pub fn cinematic_frame(c: &mut rts_lockstep_engine::client::cinematic_text::CinematicText, ticks: u32, width: u32, height: u32) -> (Option<rts_lockstep_engine::client::cinematic_text::CinematicDraw>, bool) {
    let mut drawn = None;
    for _ in 0..ticks.min(4) {
        drawn = c.draw(width, height, |_| 0);
    }
    let column_shown = drawn.is_none();
    (drawn, column_shown)
}

/// (#151) The military subtitle's host in the client (`rts_lockstep_engine::client::military_subtitle`): the World's frame
/// and its scripts' frozen time, the game's text, the display's size, `TheGlobalLanguageData`, and the fonts measured as a
/// monospaced font measures (`DisplayString::getSize`: each unit 0.6 of the point size wide, the line the point size
/// high; the client draws in its own font, not the game's). The typing sound joins the frame's GUI sounds; the label
/// joins the briefings for the shell's history.
pub struct SubtitleHost<'a> {
    pub frame: u32,
    pub frozen: bool,
    pub display: (i32, i32),
    pub text: &'a rts_lockstep_engine::logic::world::game_data::TextTable,
    pub language: &'a rts_lockstep_engine::client::global_language::GlobalLanguage,
    pub sounds: &'a mut Vec<Vec<u8>>,
    pub briefings: &'a mut Vec<Vec<u8>>,
}

impl rts_lockstep_engine::client::military_subtitle::MilitarySubtitleHost for SubtitleHost<'_> {
    fn logic_frame(&self) -> u32 {
        self.frame
    }
    fn fetch(&mut self, label: &[u8]) -> Vec<u16> {
        use rts_lockstep_engine::common::science::GameTextFetch;
        self.text.fetch(label).str().to_vec()
    }
    fn display_size(&self) -> (i32, i32) {
        self.display
    }
    fn military_caption_delay_ms(&self) -> i32 {
        self.language.military_caption_delay_ms
    }
    fn language_military_caption_speed(&self) -> Option<i32> {
        Some(self.language.military_caption_speed)
    }
    fn adjust_font_size(&self, size: i32) -> i32 {
        self.language.adjust_font_size(size, self.display.0)
    }
    fn string_size(&self, text: &[u16], font: &rts_lockstep_engine::client::window_script::GameFont) -> (i32, i32) {
        ((text.len() as f32 * font.point_size as f32 * 0.6) as i32, font.point_size)
    }
    fn font_height(&self, font: &rts_lockstep_engine::client::window_script::GameFont) -> i32 {
        font.point_size
    }
    fn add_audio_event(&mut self, name: &[u8]) {
        self.sounds.push(name.to_vec());
    }
    fn is_time_frozen_script(&self) -> bool {
        self.frozen
    }
    fn update_diplomacy_briefing_text(&mut self, label: &[u8]) {
        self.briefings.push(label.to_vec());
    }
}

/// (#151) The military subtitle's engine frames since the last client frame: each one's `update` (at most 4, as
/// [`cinematic_frame`] draws), then what `postDraw` draws.
pub fn subtitle_frame(u: &mut rts_lockstep_engine::client::military_subtitle::MilitarySubtitleUi, h: &mut SubtitleHost<'_>, settings: &rts_lockstep_engine::client::radius_cursor::InGameUiIniSettings, ticks: u32) -> Vec<rts_lockstep_engine::client::military_subtitle::SubtitleDraw> {
    use rts_lockstep_engine::client::military_subtitle::MilitarySubtitleHost as _;
    for _ in 0..ticks.min(4) {
        u.update(h, settings);
    }
    let _ = h.logic_frame();
    u.draw(h)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #151, predicted before the run: on a phone's 390x844 a 1,000 ms caption "Hi" made at frame 10 sits at (4, 534) (10
    /// and 380 of 800x600 scaled), its title font the constructor's Courier 12 bold at `adjustFontSize`'s size for a
    /// 390-wide display (12: the adjustment lowers nothing under 800 wide); the language's 750 ms delay is 22 frames, so
    /// frame 33 types `H` with the typing sound and the block 7 to the right (0.6 x 12); frame 34 types `i` (speed 0: the
    /// next frame); updated every frame the block is on at 33 (off at 20, on at 30); the label went to the briefings once.
    #[test]
    fn the_military_subtitle_types_out_on_a_phone() {
        use rts_lockstep_engine::client::military_subtitle::{MilitarySubtitleUi, SubtitleDraw};
        use rts_lockstep_engine::client::radius_cursor::InGameUiIniSettings;
        let mut text = rts_lockstep_engine::logic::world::game_data::TextTable::default();
        text.strings.insert(b"SCRIPT:Hi".to_vec(), rts_lockstep_engine::common::unicode_string::UnicodeString::try_from_chars(&[72, 105]).expect("text"));
        let language = rts_lockstep_engine::client::global_language::GlobalLanguage::default();
        let (mut sounds, mut briefings) = (Vec::new(), Vec::new());
        let mut settings = InGameUiIniSettings::default();
        settings.set_constructor_caption_values();
        let mut u = MilitarySubtitleUi::default();
        let mut h = SubtitleHost { frame: 10, frozen: false, display: (390, 844), text: &text, language: &language, sounds: &mut sounds, briefings: &mut briefings };
        u.military_subtitle(&mut h, &settings, b"SCRIPT:Hi", 1000);
        let m = u.military_subtitle.clone().expect("made");
        assert_eq!((m.position.x, m.position.y, m.increment_on_frame), (4, 534, 32));
        assert_eq!(m.display_strings[0].as_ref().map(|d| (d.font.point_size, d.font.bold)), Some((12, true)));
        // (an update every frame: the block toggles off at 20 and on at 30)
        let mut d = Vec::new();
        for f in 11..=33 {
            h.frame = f;
            d = subtitle_frame(&mut u, &mut h, &settings, 1);
        }
        assert!(matches!(&d[0], SubtitleDraw::Text { x: 4, y: 534, text, .. } if text == &[72]), "{d:?}");
        assert!(matches!(d[1], SubtitleDraw::FillRect { x: 11, y: 534, width: 9, height: 12, .. }), "{d:?}");
        h.frame = 34;
        let d = subtitle_frame(&mut u, &mut h, &settings, 1);
        assert!(matches!(&d[0], SubtitleDraw::Text { text, .. } if text == &[72, 105]), "{d:?}");
        drop(h);
        assert_eq!(sounds.len(), 2);
        assert_eq!(briefings, vec![b"SCRIPT:Hi".to_vec()]);
    }

    use rts_lockstep_engine::logic::world::gate_scenario::{gate_world, run_gate_scenario};
    use rts_lockstep_engine::logic::world::game_logic_update::CRC_RECALC;

    /// #148 (owner decision 2026-11-12), predicted before the run: on a phone's 390x844 the column shows with no cinematic
    /// text; a script's text for 2 frames hides it on the two frames that draw the text (at y 759) and it comes back on
    /// the third, when the text is no longer drawn; a stall of 10 ticks draws at most 4; text with no frames left, or an
    /// empty text, never hides it.
    #[test]
    fn the_column_hides_while_cinematic_text_shows() {
        use rts_lockstep_engine::client::cinematic_text::{CinematicText, DEFAULT_RESOLUTION_FONT_SIZE_ADJUSTMENT as ADJ};
        use rts_lockstep_engine::logic::client_event::CinematicTextCall;
        let mut c = CinematicText::default();
        assert_eq!(cinematic_frame(&mut c, 1, 390, 844), (None, true));
        c.apply(&CinematicTextCall::Text(b"The base is under attack".to_vec()), 390, ADJ);
        c.apply(&CinematicTextCall::Frames(2), 390, ADJ);
        let (d, shown) = cinematic_frame(&mut c, 1, 390, 844);
        assert_eq!((d.map(|d| d.y), shown), (Some(759), false));
        assert!(!cinematic_frame(&mut c, 1, 390, 844).1);
        assert_eq!(cinematic_frame(&mut c, 1, 390, 844), (None, true), "the text has ended: the column is back");
        c.apply(&CinematicTextCall::Frames(30), 390, ADJ);
        let (d, shown) = cinematic_frame(&mut c, 10, 390, 844);
        assert!(d.is_some() && !shown);
        assert_eq!(c.frames, 26, "a stall draws at most 4 frames");
        c.apply(&CinematicTextCall::Text(Vec::new()), 390, ADJ);
        assert_eq!(cinematic_frame(&mut c, 1, 390, 844), (None, true), "an empty text is not shown");
    }

    /// The client never changes the simulation: the gate game played with a snapshot taken after every frame gives
    /// exactly the CRCs of the gate game played without (`run_gate_scenario`, which gate (a) compares between the native
    /// and wasm builds).
    #[test]
    fn taking_snapshots_leaves_the_crcs_alone() {
        let frames = 120;
        let plain = run_gate_scenario(frames);
        let mut w = gate_world();
        let mut viewed = Vec::new();
        for _ in 0..frames {
            w.logic_update().expect("a frame");
            let _ = snapshot(&mut w);
            let _ = terrain_grid(&w, 10.0);
            viewed.push(w.logic_crc(CRC_RECALC, None).expect("the CRC"));
        }
        assert_eq!(viewed, plain);
    }

    /// What is drawn is the World's: every live object once, at its own position and orientation, with its geometry and
    /// health; the snapshots repeat exactly between two runs; the runners move.
    #[test]
    fn the_snapshot_is_the_worlds() {
        let mut w = gate_world();
        let first = snapshot(&mut w);
        // the gate game: a depot, six runners, a gun, a dozer, four soldiers
        assert_eq!(first.objects.len(), 13, "{:?}", first.objects.iter().map(|o| &o.template).collect::<Vec<_>>());
        let depot = first.objects.iter().find(|o| o.template == "Depot").expect("the depot");
        assert_eq!(depot.pos[..2], [200.0, 200.0]);
        assert_eq!(depot.shape, Shape::Box { half_forward: 20.0, half_side: 20.0, height: 10.0 });
        assert_eq!(depot.health, Some(1.0));
        let runner = first.objects.iter().find(|o| o.template == "Runner").expect("a runner");
        assert_eq!(runner.shape, Shape::Cylinder { radius: 5.0, height: 5.0 });
        // the gate's two players are white in the logic (no `playerColor` on their sides); drawn from the palette, they differ
        let colours: std::collections::BTreeSet<String> = first.objects.iter().map(|o| format!("{:?}", o.color)).collect();
        assert_eq!(colours.len(), 2, "one colour per player: {colours:?}");

        for _ in 0..60 {
            w.logic_update().unwrap();
        }
        let later = snapshot(&mut w);
        for o in &later.objects {
            let x = w.find_object(o.id).expect("a live object");
            let p = x.get_position();
            assert_eq!(o.pos, [p.x, p.y, p.z]);
            assert_eq!(o.angle, x.thing.orientation());
        }
        let moved = later.objects.iter().filter(|o| first.find(o.id).is_some_and(|f| f.pos != o.pos)).count();
        assert!(moved >= 6, "the runners move: {moved}");
        assert_eq!(later.frame, w.frame());

        let mut again = gate_world();
        for _ in 0..60 {
            again.logic_update().unwrap();
        }
        assert_eq!(snapshot(&mut again), later, "the same game draws the same");
    }

    #[test]
    fn coordinates_colours_and_interpolation() {
        assert_eq!(to_bevy([1.0, 2.0, 3.0]), [1.0, 3.0, -2.0]);
        assert_eq!(argb_to_rgb(0xffff0000u32 as i32), [1.0, 0.0, 0.0]);
        assert_eq!(argb_to_rgb(0), [0.6, 0.6, 0.6]);
        assert_eq!(object_color(0xffffffffu32 as i32, Some(2)), PALETTE[2], "a white player: the palette");
        assert_eq!(object_color(0xff00ff00u32 as i32, Some(2)), [0.0, 1.0, 0.0], "a coloured player: its colour");
        let a = ViewObject { id: 1, template: "T".into(), pos: [0.0, 0.0, 0.0], angle: 3.0, shape: Shape::Sphere { radius: 1.0 }, color: [0.0; 3], health: None };
        let mut b = a.clone();
        b.pos = [10.0, -4.0, 2.0];
        b.angle = -3.0; // the short way from 3 to -3 is through pi, +0.283
        let m = interpolate(&a, &b, 0.5);
        assert_eq!(m.pos, [5.0, -2.0, 1.0]);
        assert!((m.angle - (3.0 + (core::f32::consts::TAU - 6.0) / 2.0)).abs() < 1e-5, "{}", m.angle);
        assert_eq!(interpolate(&a, &b, 1.0).pos, b.pos);
    }

    /// The gate map's terrain: a grid over its extent at the logic's own heights.
    #[test]
    fn the_terrain_grid_is_the_logics() {
        let w = gate_world();
        let g = terrain_grid(&w, 10.0).expect("the gate map has an extent");
        assert!(g.nx > 1 && g.ny > 1);
        assert_eq!(g.heights.len(), g.nx * g.ny);
        assert_eq!(g.heights[g.nx + 1], w.terrain.ground_height(10.0, 10.0, false).0);
    }
}
