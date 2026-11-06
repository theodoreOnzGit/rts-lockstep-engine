// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp (the calls `GameLogic::xfer` makes out of `GameLogic`)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Map/PolygonTrigger.cpp (`xfer`, `updateBounds`)
//   GeneralsMD/Code/GameEngine/Source/GameClient/System/CampaignManager.cpp (`xfer`, outside a campaign)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Integration batch I7a: the world as `GameLogicSaveHost`, so `GameLogic::xfer` ([`xfer_game_logic`]) saves and loads
//! the world's own `GameLogic` block. [`World::xfer_game_logic_block`] is the entry point.
//!
//! - **The world:** the frame, the objects (each object's own `xfer`, with the upgrade center, the name keys, the thing
//!   factory and the weapon store), destroying and making objects (a load makes each object on the neutral player's
//!   default team), the bridges at an object's position, the cave system, the polygon triggers (the terrain's trigger
//!   areas), the build assistant's sell list, and the control bar's buttons by name (`cuts.command_buttons`).
//! - **`PolygonTrigger::xfer`** (version 1: the points, the bounds, the radius, `m_boundsNeedsUpdate`) is written here
//!   from the terrain's trigger area, the bounds and radius as `updateBounds` computes them, `halfHeight` from
//!   `hi.y + lo.y` as upstream does. **Deviation:** the trigger area keeps no bounds of its own, so it is saved as it is
//!   after its first query (bounds computed, `m_boundsNeedsUpdate` false); a save taken before any query of a trigger
//!   would write the constructor's bounds and TRUE upstream. A load reads them and keeps the map's.
//! - **`CampaignManager::xfer`** (version 5) is the client's: ~~the world writes what it writes outside a campaign (no
//!   campaign, no mission, the rank points and difficulty of `CampaignStandIn`, not a challenge, and `TheChallengeGenerals`'
//!   player template number) and reads the same~~ **corrected 2026-11-06 (GitHub #61):** translated whole
//!   (`crate::client::campaign_manager`) over the World's `TheCampaignManager` (`World::campaign`), with
//!   `TheChallengeGameInfo` and `TheChallengeGenerals` ([`CampaignXferHost`] below). It is also the save file's own
//!   `CHUNK_Campaign` block (`crate::logic::world::game_state_world`), as upstream registers it.
//! - **Cut points:** the rank level limit (`m_rankLevelLimit`, kept in `cuts.player_host.rank_level_limit`), the
//!   pathfinder's `addWallPiece` and `newMap` (batch I9).
//!
//! `p1w13-sonnet`, integration batch I7a, 2026-10-31.

use crate::common::base_type::{ICoord2D, ICoord3D, IRegion2D};
use crate::common::game_state::SaveError;
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferVersion};
use crate::common::kind_of::{KINDOF_BRIDGE, KINDOF_WALK_ON_TOP_OF_WALL};
use crate::logic::world::build_assistant::{build_assistant_xfer_the_sell_list, BuildError};
use crate::logic::world::game_logic::World;
use crate::logic::world::game_logic_xfer::{xfer_game_logic, GameLogicSaveHost, BRIDGE_MAX_TOWERS};
use crate::logic::world::ids::ObjectId;
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_xfer::{ObjectXferError, ObjectXferStores};
use crate::client::campaign_manager::CampaignXferHost;
use crate::common::game_info::GameInfo;
use crate::logic::world::game_logic_new_game::GameSource;

type SaveResult<T> = Result<T, SaveError>;

/// `PolygonTrigger::updateBounds`: the points' bounds and the radius it computes from them (`halfHeight` adds the two
/// y bounds, as upstream does).
pub(crate) fn polygon_bounds(points: &[ICoord3D]) -> (IRegion2D, f32) {
    const BIG_INT: i32 = 0x7ffff0;
    let mut b = IRegion2D { lo: ICoord2D { x: BIG_INT, y: BIG_INT }, hi: ICoord2D { x: -BIG_INT, y: -BIG_INT } };
    for p in points {
        b.lo.x = b.lo.x.min(p.x);
        b.lo.y = b.lo.y.min(p.y);
        b.hi.x = b.hi.x.max(p.x);
        b.hi.y = b.hi.y.max(p.y);
    }
    let half_width = (b.hi.x - b.lo.x) as f32 / 2.0;
    let half_height = (b.hi.y + b.lo.y) as f32 / 2.0;
    (b, (half_height * half_height + half_width * half_width).sqrt())
}

impl World {
    /// `GameLogic::xfer(xfer)`: the `CHUNK_GameLogic` block (see [`xfer_game_logic`]).
    pub fn xfer_game_logic_block(&mut self, xfer: &mut dyn Xfer) -> SaveResult<()> {
        let mut state = std::mem::take(&mut self.game_logic);
        let r = xfer_game_logic(&mut state, xfer, self);
        self.game_logic = state;
        r
    }

    /// Logs a call of `GameLogic::xfer` into a unit the world does not hold.
    fn save_cut(&mut self, call: &'static str, args: &[i64]) {
        self.cut(CutCall::Host { call, args: args.to_vec(), text: Vec::new() });
    }
}

impl GameLogicSaveHost for World {
    fn x_frame(&self) -> u32 {
        self.frame()
    }
    fn x_objects(&self) -> Vec<u32> {
        self.objects.ids_in_order().into_iter().map(|o| o as u32).collect()
    }
    fn x_process_destroy_list(&mut self) {
        self.process_destroy_list();
    }
    fn x_set_frame(&mut self, frame: u32) {
        self.set_frame(frame);
    }
    fn x_object_template_name(&self, id: u32) -> Vec<u8> {
        self.find_object(id as ObjectId).map(|o| o.template().get_name().to_vec()).unwrap_or_default()
    }
    fn x_object_is_bridge(&self, id: u32) -> bool {
        self.find_object(id as ObjectId).is_some_and(|o| o.is_kind_of(KINDOF_BRIDGE))
    }
    fn x_object_walks_on_walls(&self, id: u32) -> bool {
        self.find_object(id as ObjectId).is_some_and(|o| o.is_kind_of(KINDOF_WALK_ON_TOP_OF_WALL))
    }
    fn x_bridge_at_object(&self, id: u32) -> Option<(u32, [u32; BRIDGE_MAX_TOWERS])> {
        let pos = self.find_object(id as ObjectId)?.get_position();
        let i = self.terrain.find_bridge_at(&pos)?;
        let info = self.terrain.bridges()[i].info();
        Some((info.bridge_object_id as u32, info.tower_object_id.map(|t| t as u32)))
    }
    fn x_object_exists(&self, id: u32) -> bool {
        self.find_object(id as ObjectId).is_some()
    }
    fn x_destroy_object(&mut self, id: u32) {
        self.destroy_object(id as ObjectId);
    }
    fn x_xfer_object(&mut self, id: u32, xfer: &mut dyn Xfer) -> SaveResult<()> {
        let statics = self.statics.clone();
        let mut names = std::mem::take(&mut self.names);
        let mut stores = ObjectXferStores {
            upgrade_center: &statics.upgrade_center,
            names: &mut names,
            factory: &statics.thing_factory,
            weapon_store: &statics.weapon_store,
        };
        let r = self.object_xfer(id as ObjectId, xfer, &mut stores);
        self.names = names;
        r.map_err(|e| match e {
            ObjectXferError::Xfer(s) => SaveError::Xfer(s),
            // (an id taken or an upgrade mask that does not load: the save does not fit the world)
            ObjectXferError::InvalidData(_) | ObjectXferError::UpgradeMask(_) | ObjectXferError::IdTaken(_) => SaveError::InvalidData,
        })
    }
    fn x_has_neutral_player(&self) -> bool {
        self.players.get_nth(0).is_some()
    }
    fn x_template_exists(&self, name: &[u8]) -> bool {
        self.statics.thing_factory.find_template(name).is_some()
    }
    fn x_new_object(&mut self, template_name: &[u8]) -> SaveResult<u32> {
        let t = World::thing_factory_find_template(self, template_name).ok_or(SaveError::InvalidData)?;
        let team = self.players.get_nth(0).and_then(|p| p.default_team);
        World::thing_factory_new_object(self, t, team, ObjectStatusMaskType::default()).map(|o| o as u32).map_err(|_| SaveError::InvalidData)
    }
    fn x_add_wall_piece(&mut self, id: u32) {
        if self.pathfinder_op(crate::logic::world::pathfinder_host_world::PfOp::AddWallPiece(id as ObjectId)) {
            return;
        }
        self.save_cut("Pathfinder::addWallPiece", &[i64::from(id)]);
    }
    fn x_pathfinder_new_map(&mut self) {
        if self.pathfinder_op(crate::logic::world::pathfinder_host_world::PfOp::NewMap) {
            return;
        }
        self.save_cut("Pathfinder::newMap", &[]);
    }
    fn x_xfer_campaign_manager(&mut self, xfer: &mut dyn Xfer) -> SaveResult<()> {
        // (GitHub #61: `CampaignManager::xfer` itself, over the World's `TheCampaignManager`; ~~a stand-in outside a
        // campaign~~, whose load of a campaign's or a challenge's save was `SC_INVALID_DATA`)
        let mut campaign = std::mem::take(&mut self.campaign);
        let r = campaign.xfer(xfer, self);
        self.campaign = campaign;
        Ok(r?)
    }
    fn x_xfer_cave_system(&mut self, xfer: &mut dyn Xfer) -> SaveResult<()> {
        self.cave_system.xfer(xfer)?;
        Ok(())
    }
    fn x_polygon_triggers(&self) -> Vec<i32> {
        self.terrain.triggers().iter().map(|t| t.id()).collect()
    }
    fn x_xfer_polygon_trigger(&mut self, at: usize, xfer: &mut dyn Xfer) -> SaveResult<()> {
        let Some(t) = self.terrain.triggers().get(at) else { return Err(SaveError::InvalidData) };
        let mut points: Vec<ICoord3D> = (0..t.num_points()).map(|i| *t.point(i)).collect();
        let (mut bounds, mut radius) = polygon_bounds(&points);
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        let mut n = points.len() as i32;
        xfer.xfer_int(&mut n)?;
        if xfer.get_xfer_mode() == XferMode::Load && n as usize != points.len() {
            // upstream writes `n` points into the map's array: a count that differs does not fit
            return Err(SaveError::InvalidData);
        }
        for p in &mut points {
            xfer.xfer_icoord3d(p)?;
        }
        xfer.xfer_iregion2d(&mut bounds)?;
        xfer.xfer_real(&mut radius)?;
        let mut needs_update = false;
        xfer.xfer_bool(&mut needs_update)?;
        Ok(())
    }
    fn x_rank_level_limit(&self) -> i32 {
        self.cuts.player_host.rank_level_limit
    }
    fn x_set_rank_level_limit(&mut self, limit: i32) {
        self.cuts.player_host.rank_level_limit = limit;
    }
    fn x_xfer_sell_list(&mut self, xfer: &mut dyn Xfer) -> SaveResult<()> {
        build_assistant_xfer_the_sell_list(&mut self.build_assistant, xfer).map_err(|e| match e {
            BuildError::Xfer(s) => SaveError::Xfer(s),
            BuildError::Undefined(_) => SaveError::InvalidData,
        })
    }
    fn x_find_command_button(&self, name: &[u8]) -> Option<u32> {
        self.cuts.command_buttons.iter().position(|b| b.name == name).map(|i| i as u32)
    }
    fn x_command_button_name(&self, button: u32) -> Vec<u8> {
        self.cuts.command_buttons.get(button as usize).map(|b| b.name.clone()).unwrap_or_default()
    }
}

/// GitHub #61: what `CampaignManager::xfer` reaches: `TheChallengeGameInfo` is the new game's challenge entry
/// (`cuts.new_game.games`, [`GameSource::Challenge`], which `startNewGame` reads in a challenge campaign), and
/// `TheChallengeGenerals` is the World's.
impl CampaignXferHost for World {
    fn take_challenge_game_info(&mut self) -> Option<GameInfo> {
        let games = &mut self.cuts.new_game.games;
        let at = games.iter().position(|(s, _)| *s == GameSource::Challenge)?;
        Some(games.remove(at).1)
    }
    fn set_challenge_game_info(&mut self, game: Option<GameInfo>) {
        let games = &mut self.cuts.new_game.games;
        games.retain(|(s, _)| *s != GameSource::Challenge);
        if let Some(g) = game {
            games.push((GameSource::Challenge, g));
        }
    }
    fn challenge_player_template_num(&self) -> i32 {
        self.challenge_generals.get_current_player_template_num()
    }
    fn set_challenge_player_template_num(&mut self, n: i32) {
        self.challenge_generals.set_current_player_template_num(n);
    }
}

#[cfg(test)]
mod tests {
    //! `GameLogic::xfer` itself is checked against the compiled GameLogic.cpp by `tests/differential_game_logic_save.rs`
    //! with a recording host; these check the world's block: a save loads into a fresh world as the same objects, frame
    //! and campaign, and a second round trip saves the same bytes (a load reverses the object list, as upstream's does).
    use super::*;
    use crate::common::base_type::Coord3D;
    use crate::common::xfer_load::XferLoad;
    use crate::common::xfer_save::XferSave;
    use crate::logic::world::player_host_world::tests::{make, world};

    fn save(w: &mut World) -> Vec<u8> {
        let mut x = XferSave::new();
        x.open(b"game_logic").unwrap();
        w.xfer_game_logic_block(&mut x).expect("save");
        x.close().unwrap();
        x.take_bytes()
    }

    /// A tank's and a barracks' world (ids 3 and 4) at frame 77 with rank points 5, difficulty 2 and (GitHub #58) the challenge template 3 saved, then loaded into a fresh world: the
    /// same objects (ids, templates, positions), frame and campaign. (**Corrected 2026-11-06 (GitHub #61):** in campaign
    /// `usa`, mission `m02`, of a synthetic `Campaign.ini` in each world: outside a campaign, upstream's load
    /// (`setCampaign("")`) resets the rank points to 0 and the difficulty to normal, which the stand-in did not; the
    /// template number reaches `TheChallengeGenerals` by `loadPostProcess`, run here as the save file's post-process runs it.) Its save lists the objects in the reverse order, and
    /// loading that into a third world saves the first bytes again.
    #[test]
    fn a_saved_block_loads_into_a_fresh_world_and_saves_the_same() {
        let (mut a, team) = world();
        // two objects made and gone first: the saved ids (3, 4) are not the temporary ids a load gives (1, 2)
        for _ in 0..2 {
            let gone = make(&mut a, b"Tank", team, Coord3D { x: 50.0, y: 50.0, z: 0.0 });
            a.destroy_object(gone);
            a.process_destroy_list();
        }
        let t1 = make(&mut a, b"Tank", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        let b1 = make(&mut a, b"Barracks", team, Coord3D { x: 200.0, y: 200.0, z: 0.0 });
        a.set_frame(77);
        let campaigns = |w: &mut World| w.campaign.init(crate::client::campaign_manager::tests::INI).expect("the campaigns");
        campaigns(&mut a);
        a.campaign.set_campaign_and_mission(b"usa", b"m02");
        a.campaign.set_rank_points(5);
        a.campaign.set_game_difficulty(2);
        a.challenge_generals.set_current_player_template_num(3);
        let bytes = save(&mut a);

        let (mut b, _) = world();
        campaigns(&mut b);
        let mut load = XferLoad::new(bytes.clone());
        load.open(b"game_logic").unwrap();
        b.xfer_game_logic_block(&mut load).expect("load");
        b.process_destroy_list();
        assert_eq!(b.frame(), 77);
        b.campaign.clone().load_post_process(&mut b);
        assert_eq!(b.campaign.get_current_mission().map(|m| m.name.clone()), Some(b"m02".to_vec()));
        assert_eq!((b.campaign.current_rank_points(), b.campaign.get_game_difficulty()), (5, 2));
        assert_eq!(b.challenge_generals.get_current_player_template_num(), 3);
        let mut ids = b.objects.ids_in_order();
        ids.sort();
        assert_eq!(ids, {
            let mut v = vec![t1, b1];
            v.sort();
            v
        });
        assert_eq!(b.find_object(t1).unwrap().template().get_name(), b"Tank");
        assert_eq!(b.find_object(b1).unwrap().get_position(), Coord3D { x: 200.0, y: 200.0, z: 0.0 });
        // a load prepends each object it makes (`registerObject`), so the loaded world's list is the saved one reversed,
        // and so is its save; a second round trip restores the order and the bytes
        let again = save(&mut b);
        assert_ne!(again, bytes);
        let (mut c, _) = world();
        campaigns(&mut c);
        let mut load = XferLoad::new(again);
        load.open(b"game_logic").unwrap();
        c.xfer_game_logic_block(&mut load).expect("load");
        c.process_destroy_list();
        c.campaign.clone().load_post_process(&mut c);
        assert_eq!(save(&mut c), bytes);
    }

    /// A load makes each object with the next id and then gives it its saved id (`Object::xfer`'s `setID`). Upstream's
    /// lookup is a vector that `addObjectToLookupTable` overwrites, so when a new object's temporary id is an id already
    /// loaded, the earlier object is silently lost from `findObjectByID`. The world's arena refuses the duplicate id
    /// instead: the load is `SC_INVALID_DATA`. Here the saved ids are 2 and 1 (newest first), and the second object's
    /// temporary id is 2.
    #[test]
    fn a_load_whose_temporary_id_is_taken_is_invalid_data() {
        let (mut a, team) = world();
        make(&mut a, b"Tank", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        make(&mut a, b"Barracks", team, Coord3D { x: 200.0, y: 200.0, z: 0.0 });
        let bytes = save(&mut a);
        let (mut b, _) = world();
        let mut load = XferLoad::new(bytes);
        load.open(b"game_logic").unwrap();
        assert!(matches!(b.xfer_game_logic_block(&mut load), Err(SaveError::InvalidData)));
    }

    /// `updateBounds`' radius: half the width, and half the *sum* of the y bounds (upstream's).
    #[test]
    fn polygon_bounds_follow_upstream() {
        let pts = [ICoord3D { x: 10, y: 20, z: 0 }, ICoord3D { x: 30, y: 60, z: 0 }];
        let (b, r) = polygon_bounds(&pts);
        assert_eq!((b.lo.x, b.lo.y, b.hi.x, b.hi.y), (10, 20, 30, 60));
        assert_eq!(r, (10.0f32 * 10.0 + 40.0 * 40.0).sqrt());
    }
}
