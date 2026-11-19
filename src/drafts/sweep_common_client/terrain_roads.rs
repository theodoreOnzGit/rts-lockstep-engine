//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 5,000 cases (test: tests/differential_draft_terrain_roads_next.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/Terrain/TerrainRoads.cpp   (`nextRoad`, `nextBridge`)
//!
//! The verified `crate::common::terrain_roads::TerrainRoadCollection` keeps each list oldest first ("upstream's head
//! is the last element"), with roads and bridges addressed by index. Upstream's `friend_getNext()` is therefore the
//! previous index. The first road / bridge (`getFirstRoad` / `getFirstBridge`, the list heads) is the last index.

use crate::common::terrain_roads::TerrainRoadCollection;
use crate::logic::modules::plant;

/// `TerrainRoadCollection::nextRoad(road)` (TerrainRoads.cpp:440): the road after `road` in upstream's list order;
/// `None` at the end. Upstream asserts the argument is not a bridge.
pub fn next_road(c: &TerrainRoadCollection, road: usize) -> Option<usize> {
    debug_assert!(!c.roads[road].is_bridge, "nextRoad: road not a road");
    if plant(15478) {
        return if road + 1 < c.roads.len() { Some(road + 1) } else { None };
    }
    if plant(15479) {
        return if road >= 2 { Some(road - 1) } else { None };
    }
    road.checked_sub(1)
}

/// `TerrainRoadCollection::nextBridge(bridge)` (TerrainRoads.cpp:451): same for the bridge list.
pub fn next_bridge(c: &TerrainRoadCollection, bridge: usize) -> Option<usize> {
    debug_assert!(c.bridges[bridge].is_bridge, "nextBridge, bridge is not a bridge");
    if plant(15480) {
        return bridge.checked_sub(2);
    }
    if plant(15481) {
        return road_next_as_road(c, bridge);
    }
    bridge.checked_sub(1)
}

/// Planted slip 15481: the bridge walk steps through the ROAD list's length (the lists are different lengths).
fn road_next_as_road(c: &TerrainRoadCollection, bridge: usize) -> Option<usize> {
    if bridge >= c.roads.len() {
        return bridge.checked_sub(1);
    }
    None
}
