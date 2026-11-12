// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file (this repository's own glue, not upstream code): the radar's questions answered from the
// `World` (`client::radar`, `client::w3d_radar`), and the radar the client keeps. It names the upstream calls each
// answer stands for.

//! The radar over a `World` (GitHub #35): [`WorldRadarHost`] answers [`RadarHost`] and [`RadarDrawHost`] from the logic,
//! reading only (the `&mut` answers call modules that only answer, as the control bar's questions do: `world_host`), and
//! [`ClientRadar`] holds the radar, its textures and what they are drawn from. The tests check that the logic's CRCs are
//! the same with the radar attached (`AGENTS.md`: the client never changes the simulation).

use crate::client::drawable::ClientDrawables;
use crate::client::radar::{Radar, RadarFrame, RadarHost, RadarUnderwater};
use crate::client::terrain_height_map::TerrainColors;
use crate::client::w3d_radar::{RadarDrawHost, RadarImage, W3dRadar};
use crate::client::water::WaterSettings;
use crate::common::base_type::{Coord3D, ICoord2D, RGBColor, Region3D};
use crate::common::game_common::{CellShroudStatus, ObjectShroudStatus, Relationship};
use crate::common::game_type::ObjectId;
use crate::common::kind_of::KindOfType;
use crate::common::object_status_types::ObjectStatusTypes;
use crate::logic::client_event::ClientEvent;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleReply};

/// The radar's terrain colour when no terrain texture has been read (no game files: the client draws the terrain
/// untextured, in this colour, `client/src/terrain.rs`). New: upstream always has the map's tiles; a tile it lacks reads as
/// black (`getTerrainColorAt`), which [`TerrainColors`] keeps.
pub const UNTEXTURED_TERRAIN_COLOR: RGBColor = RGBColor { red: 0.32, green: 0.42, blue: 0.26 };

/// The World as the radar reads it.
pub struct WorldRadarHost<'a> {
    pub world: &'a mut World,
    /// The drawables (for `isTemporarilyHidden`); `None`: nothing is hidden.
    pub drawables: Option<&'a ClientDrawables>,
    /// `getTerrainColorAt`'s tiles; `None` (no terrain textures read: no game files) gives [`UNTEXTURED_TERRAIN_COLOR`].
    pub colors: Option<&'a TerrainColors>,
    pub water_color: RGBColor,
}

impl RadarHost for WorldRadarHost<'_> {
    fn radar_frame(&self) -> u32 {
        self.world.frame()
    }
    fn radar_local_player(&self) -> Option<i32> {
        self.world.players.local_index()
    }
    fn radar_priority(&mut self, obj: ObjectId) -> u8 {
        self.world.get_radar_priority(obj)
    }
    fn radar_object_position(&self, obj: ObjectId) -> Option<Coord3D> {
        self.world.find_object(obj).map(|o| *o.thing.position())
    }
    fn radar_is_locally_controlled(&self, obj: ObjectId) -> bool {
        self.world.is_locally_controlled(obj)
    }
    fn radar_controlling_player(&self, obj: ObjectId) -> Option<i32> {
        self.world.get_controlling_player(obj)
    }
    fn radar_indicator_color(&self, obj: ObjectId) -> i32 {
        self.world.get_indicator_color(obj)
    }
    fn radar_is_kind_of(&self, obj: ObjectId, kind: KindOfType) -> bool {
        self.world.find_object(obj).is_some_and(|o| o.is_kind_of(kind))
    }
    fn radar_disguised_player(&mut self, obj: ObjectId) -> Option<i32> {
        let s = self.world.find_object(obj)?.stealth?;
        match self.world.module_call(obj, s, ModuleCall::StealthGetDisguisedPlayerIndex) {
            ModuleReply::Int(p) => Some(p),
            _ => None,
        }
    }
    fn radar_contain_apparent_player(&mut self, obj: ObjectId, observer: Option<i32>) -> Option<Option<i32>> {
        let c = self.world.get_contain(obj)?;
        Some(match self.world.module_call(obj, c, ModuleCall::GetApparentControllingPlayer { observer: observer.unwrap_or(-1) }) {
            ModuleReply::Int(p) if p >= 0 => Some(p),
            _ => None,
        })
    }
    fn radar_player_color(&self, player: i32) -> i32 {
        self.world.players.get_nth(player).map_or(0, |p| p.color as i32)
    }
    fn radar_player_exists(&self, player: i32) -> bool {
        self.world.players.get_nth(player).is_some()
    }
    fn radar_player_relationship_to_player(&self, player: i32, other: i32) -> Relationship {
        self.world.tq().player_relationship_to_player_default_team(player, other)
    }
    fn radar_player_relationship_to_object(&self, player: i32, obj: ObjectId) -> Relationship {
        self.world.tq().player_relationship_to_team(player, self.world.get_team(obj))
    }
    fn radar_is_player_active(&self, player: i32) -> bool {
        crate::logic::world::player_ops::player_is_player_active(&*self.world, player).unwrap_or(false)
    }
    fn radar_player_has_radar(&self, player: i32) -> bool {
        crate::logic::world::player_ops::player_has_radar(&*self.world, player).unwrap_or(false)
    }
    fn radar_map_extent(&self) -> Region3D {
        self.world.terrain.extent().unwrap_or_default()
    }
    fn radar_ground_height(&self, x: f32, y: f32) -> f32 {
        self.world.terrain.ground_height(x, y, false).0
    }
    fn radar_is_underwater(&self, x: f32, y: f32) -> RadarUnderwater {
        match self.world.terrain.is_underwater_uncounted(x, y) {
            Ok(u) => RadarUnderwater { underwater: u.underwater, water_z: u.water_z, terrain_z: u.terrain_z },
            Err(_) => RadarUnderwater { underwater: false, water_z: None, terrain_z: self.radar_ground_height(x, y) },
        }
    }
}

impl RadarDrawHost for WorldRadarHost<'_> {
    fn radar_terrain_color_at(&self, x: f32, y: f32) -> RGBColor {
        self.colors.map_or(UNTEXTURED_TERRAIN_COLOR, |c| c.get_terrain_color_at(x, y))
    }
    fn radar_water_color(&self) -> RGBColor {
        self.water_color
    }
    fn radar_working_bridge_at(&self, pos: &Coord3D) -> Option<(Option<RGBColor>, f32)> {
        let t = &self.world.terrain;
        let b = &t.bridges()[t.find_bridge_at(pos)?];
        let info = b.info();
        // the bridge's object, if it is there and not rubble
        if self.world.find_object(info.bridge_object_id).is_none() || self.world.peek_damage_state(info.bridge_object_id) == crate::logic::module::body_module::BODY_RUBBLE {
            return None;
        }
        let color = self.world.statics.terrain_roads.find_bridge(b.template_name().str()).map(|r| r.radar_color);
        let height = (info.from_left.z + info.from_right.z + info.to_left.z + info.to_right.z) / 4.0;
        Some((color, height))
    }
    fn radar_temporarily_hidden(&self, obj: ObjectId) -> bool {
        use crate::logic::modules::stealth_update::stealth_look;
        self.drawables.and_then(|d| d.drawables.get(&obj)).is_some_and(|d| d.stealth_look == stealth_look::INVISIBLE || d.is_drawable_effectively_hidden())
    }
    fn radar_is_hero(&mut self, obj: ObjectId) -> bool {
        self.world.is_hero(obj)
    }
    fn radar_object_shrouded_status(&self, obj: ObjectId, player: i32) -> ObjectShroudStatus {
        self.world.get_shrouded_status(obj, player)
    }
    fn radar_test_status(&self, obj: ObjectId, status: ObjectStatusTypes) -> bool {
        self.world.find_object(obj).is_some_and(|o| o.test_status(status))
    }
    fn radar_shroud_grid(&self) -> Option<(i32, i32, f32)> {
        let p = &self.world.partition;
        (p.get_cell_count_x() > 0 && p.get_cell_count_y() > 0).then(|| (p.get_cell_count_x(), p.get_cell_count_y(), p.get_cell_size()))
    }
    fn radar_cell_shroud(&self, x: i32, y: i32) -> CellShroudStatus {
        // (`setShroudLevel` is called for the local player's cells; with none, `getShroudStatusForPlayer` of -1: shrouded)
        let local = self.world.players.local_index().unwrap_or(-1);
        self.world.partition.get_shroud_status_for_player_at_cell(local, x, y)
    }
}

/// The client's radar: `TheRadar` and its device half, what its terrain is drawn from, and the water INI.
#[derive(Clone, Debug, Default)]
pub struct ClientRadar {
    pub radar: Radar,
    pub device: W3dRadar,
    /// `getTerrainColorAt`'s tiles, once the terrain's textures are read.
    pub terrain_colors: Option<TerrainColors>,
    pub water: WaterSettings,
}

impl ClientRadar {
    fn host<'a>(&'a self, world: &'a mut World, drawables: Option<&'a ClientDrawables>) -> WorldRadarHost<'a> {
        WorldRadarHost { world, drawables, colors: self.terrain_colors.as_ref(), water_color: self.water.radar_color() }
    }

    /// `TheRadar->newMap(TheTerrainLogic)` (`GameLogic::startNewGame`): the extent, the averages and the terrain texture.
    pub fn new_map(&mut self, world: &mut World) {
        let mut radar = std::mem::take(&mut self.radar);
        let mut device = std::mem::take(&mut self.device);
        device.new_map(&mut radar, &self.host(world, None));
        self.radar = radar;
        self.device = device;
    }

    /// The terrain's tiles have been read: the terrain texture is rebuilt with their colours.
    pub fn set_terrain_colors(&mut self, colors: Option<TerrainColors>, world: &mut World) {
        self.terrain_colors = colors;
        let mut device = std::mem::take(&mut self.device);
        device.build_terrain_texture(&self.radar, &self.host(world, None));
        self.device = device;
    }

    /// A frame's client events ([`Radar::take_events`]).
    pub fn take_events(&mut self, world: &mut World, events: &[ClientEvent]) -> RadarFrame {
        let mut radar = std::mem::take(&mut self.radar);
        let f = radar.take_events(events, &mut self.host(world, None));
        self.radar = radar;
        f
    }

    /// (#162) `TheRadar->createEvent(pos, type, secondsToLive)` (a client update module's: `BeaconClientUpdate`'s pulse).
    pub fn create_event(&mut self, world: &mut World, pos: &Coord3D, event_type: crate::client::radar::RadarEventType, seconds_to_live: f32) {
        let mut radar = std::mem::take(&mut self.radar);
        radar.create_event(pos, event_type, seconds_to_live, &self.host(world, None));
        self.radar = radar;
    }

    /// `W3DLeftHUDDraw` into a window of `size` ([`W3dRadar::draw_left_hud`]).
    pub fn draw(&mut self, world: &mut World, drawables: Option<&ClientDrawables>, size: ICoord2D, view: Option<[Coord3D; 4]>) -> Option<RadarImage> {
        let mut radar = std::mem::take(&mut self.radar);
        let mut device = std::mem::take(&mut self.device);
        let img = device.draw_left_hud(&mut radar, &mut self.host(world, drawables), size, view);
        self.radar = radar;
        self.device = device;
        img
    }

    /// `LeftHUDInput`'s test and conversion: with the radar on for the local player, the ground point under a pixel of the
    /// radar window (`localPixelToRadar`, `radarToWorld`); `None` when the radar is off or the pixel is off the map.
    pub fn local_pixel_to_world(&self, world: &mut World, pixel: &ICoord2D) -> Option<Coord3D> {
        let host = self.host(world, None);
        if !self.radar.is_on_for(&host) {
            return None;
        }
        let r = self.radar.local_pixel_to_radar(pixel)?;
        Some(self.radar.radar_to_world(&r, &host))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::world::gate_scenario;

    /// #35: the client CRC test. Predicted before the run: the gate game run twice, once with the radar attached (a new
    /// map, every frame's events taken, the radar drawn into a 200 x 150 window and a click converted every frame) and once
    /// without, gives the same logic CRC at each of 120 frames, since every radar answer reads the World or calls modules
    /// that only answer; and with the radar forced on, a click in the window's middle is the map's middle.
    ///
    /// ~~The radar lists at least the game's six runners and its gun.~~ *Miss (2026-11-06, first run):* none were listed:
    /// the gate game's templates have no `RadarPriority`, so `isPriorityVisible` keeps all of them off the radar (as
    /// upstream would). The test now adds three `RadarPriority = UNIT` objects of its own (two of player 1, made local, and
    /// one of player 2) after the radar's new map, and predicts two on the local list and one on the other.
    ///
    /// *Results (2026-11-06):* the CRCs equal at every frame (first run); the lists as predicted (second run).
    #[test]
    fn the_radar_leaves_the_logics_crcs_as_they_were() {
        use crate::logic::world::player_list::player_list_set_local_player;
        use crate::logic::world::team_ops::factory_find_team;
        let extra = "Object Blip\n KindOf = VEHICLE\n RadarPriority = UNIT\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\nEnd\n";
        let run = |attach: bool| {
            let mut w = gate_scenario::gate_world_with(extra);
            player_list_set_local_player(&mut w, Some(1)).expect("the local player");
            let mut radar = ClientRadar::default();
            if attach {
                radar.new_map(&mut w);
                radar.radar.set_radar_window(Some((ICoord2D::default(), ICoord2D { x: 200, y: 150 })));
            }
            let team1 = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
            let team2 = factory_find_team(&mut w, b"team2").expect("the team").expect("team2");
            for (team, x) in [(team1, 50.0), (team1, 70.0), (team2, 300.0)] {
                gate_scenario::make(&mut w, b"Blip", team, Coord3D { x, y: 50.0, z: 0.0 });
            }
            let mut crcs = Vec::new();
            for _ in 0..120 {
                w.logic_update().expect("frame");
                let events = w.take_client_events();
                if attach {
                    radar.take_events(&mut w, &events);
                    radar.radar.force_on(true);
                    let img = radar.draw(&mut w, None, ICoord2D { x: 200, y: 150 }, None);
                    assert!(img.is_some());
                    let at = radar.local_pixel_to_world(&mut w, &ICoord2D { x: 100, y: 75 }).expect("on the map");
                    let ext = w.terrain.extent().expect("a map");
                    assert!((at.x - ext.width() / 2.0).abs() <= radar.radar.x_sample * 2.0 && (at.y - ext.height() / 2.0).abs() <= radar.radar.y_sample * 2.0, "{at:?}");
                }
                crcs.push(w.logic_crc(crate::logic::world::game_logic_update::CRC_RECALC, None).expect("the CRC"));
            }
            (crcs, radar.radar.local_object_list.len(), radar.radar.object_list.len())
        };
        let (without, ..) = run(false);
        let (with, local, other) = run(true);
        assert_eq!(without, with);
        assert_eq!((local, other), (2, 1));
    }
}
