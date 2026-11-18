//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/Terrain/TerrainRoads.cpp   (`nextRoad`, `nextBridge`)
//!
//! The verified `crate::common::terrain_roads::TerrainRoadCollection` keeps each list oldest first ("upstream's head
//! is the last element"), with roads and bridges addressed by index. Upstream's `friend_getNext()` is therefore the
//! previous index. The first road / bridge (`getFirstRoad` / `getFirstBridge`, the list heads) is the last index.

use crate::common::terrain_roads::TerrainRoadCollection;

/// `TerrainRoadCollection::nextRoad(road)` (TerrainRoads.cpp:440): the road after `road` in upstream's list order;
/// `None` at the end. Upstream asserts the argument is not a bridge.
pub fn next_road(c: &TerrainRoadCollection, road: usize) -> Option<usize> {
    debug_assert!(!c.roads[road].is_bridge, "nextRoad: road not a road");
    road.checked_sub(1)
}

/// `TerrainRoadCollection::nextBridge(bridge)` (TerrainRoads.cpp:451): same for the bridge list.
pub fn next_bridge(c: &TerrainRoadCollection, bridge: usize) -> Option<usize> {
    debug_assert!(c.bridges[bridge].is_bridge, "nextBridge, bridge is not a bridge");
    bridge.checked_sub(1)
}
