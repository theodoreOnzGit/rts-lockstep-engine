// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h (PathNode, ClosestPointOnPathInfo, Path)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (lines 117-1099: PathNode, Path)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `PathNode` and `Path`: the path the pathfinder returns, a list of nodes with a second, "optimized" list threaded through them (each
//! node's `nextOpti`), and the queries a unit makes of it as it follows it (`computePointOnPath`, `computeFlightDistToGoal`).
//! `p1w13-sonnet`, layer 10, batch P1.
//!
//! # The shape
//!
//! The nodes live in the path's arena ([`Path::nodes`], never freed before the path, as upstream frees them only in `~Path`), linked by
//! index; the list order is upstream's (`m_path` through `m_next`). What the path asks of the pathfinder and the terrain is a
//! [`PathHost`]: `isLinePassable`, `isGroundPathPassable`, `getCell(layer, pos)` and `getLayerForDestination`, each called where
//! upstream calls it. The debug-only parts (`setDebugPathPosition`, the debug icons of `xfer`) have no effect on the logic and are left
//! out.
//!
//! # Upstream facts kept
//!
//! - `optimize` and `optimizeGroundPath` look from each anchor at most three steps past a change of layer, back towards the anchor, for
//!   the farthest node it can see; a straight, diagonal or one-cell step counts as seen even when the pathfinder says otherwise (the
//!   offsets are `Int`s of the `Real` differences, and the checks reuse the offsets the earlier checks changed); an unpinched cliff cell
//!   counts as seen in `optimize`. `optimizeGroundPath` then drops each optimized node closer than about two cells to its anchor.
//! - `prependNode` clears the optimized flag (and leaves the links); `appendNode` on an optimized path drops a node at the tail's x and
//!   y and links the old tail to the new one; `updateLastNode` asks the terrain for the layer first, whatever the path holds.
//! - `computePointOnPath` returns its cached answer up to 20 times for a position within 0.1 of the last; it computes in `double` where
//!   upstream's `0.5` and `1.0 / maxPathError` are `double`.
//! - The save writes the nodes from the tail back, numbering them down from the count (and keeps those numbers as their ids); the load
//!   prepends them, linking each to the optimized node with its saved id among those already read (none for an id of 0 or less).
//!
//! # Where upstream is undefined
//!
//! The save writes an uninitialised local (`obsolete2`); this port writes 0, as the differential build's zero-initialised locals do.
//! `optimize` and `optimizeGroundPath` on an empty path read its first node (the callers have nodes; this port does nothing).

use crate::common::base_type::{sqr, sqrt_via_double, Coord2D, Coord3D};
use crate::common::game_type::{ObjectId, PathfindLayer, LAYER_GROUND, LAYER_INVALID};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferVersion};
use crate::logic::locomotor::LocomotorSurfaceTypeMask;
use crate::logic::modules::plant;

/// `PATHFIND_CELL_SIZE`.
pub const PATHFIND_CELL_SIZE: i32 = 10;
/// `PATHFIND_CELL_SIZE_F`.
pub const PATHFIND_CELL_SIZE_F: f32 = 10.0;
/// `PATHFIND_CLOSE_ENOUGH`: "how close is close enough when moving".
pub const PATHFIND_CLOSE_ENOUGH: f32 = 1.0;
/// `PathfindCell::CELL_CLIFF`.
pub const CELL_CLIFF: i32 = 0x02;
/// `Path::MAX_CPOP`: "max times we will return the cached cpop".
const MAX_CPOP: i32 = 20;

/// What a path asks of the pathfinder and the terrain.
pub trait PathHost {
    /// `TheAI->pathfinder()->isLinePassable(obj, acceptableSurfaces, layer, start, end, blocked, allowPinched)`.
    #[allow(clippy::too_many_arguments)]
    fn is_line_passable(&mut self, obj: ObjectId, surfaces: LocomotorSurfaceTypeMask, layer: PathfindLayer, start: &Coord3D, end: &Coord3D, blocked: bool, allow_pinched: bool) -> bool;
    /// `TheAI->pathfinder()->isGroundPathPassable(crusher, start, layer, end, diameter)`.
    fn is_ground_path_passable(&mut self, crusher: bool, start: &Coord3D, layer: PathfindLayer, end: &Coord3D, diameter: i32) -> bool;
    /// `TheAI->pathfinder()->getCell(layer, pos)`: its type and whether it is pinched, `None` for `NULL`.
    fn cell_type_and_pinched(&mut self, layer: PathfindLayer, pos: &Coord3D) -> Option<(i32, bool)>;
    /// `TheTerrainLogic->getLayerForDestination(pos)`.
    fn layer_for_destination(&mut self, pos: &Coord3D) -> PathfindLayer;
    /// `obj->getLayer()`.
    fn object_layer(&mut self, obj: ObjectId) -> PathfindLayer;
}

/// `class PathNode`.
#[derive(Clone, Debug, PartialEq)]
pub struct PathNode {
    /// `m_id`: "used in Path::xfer() to save & recreate the path list".
    pub id: i32,
    next_opti: Option<usize>,
    next: Option<usize>,
    prev: Option<usize>,
    pos: Coord3D,
    layer: PathfindLayer,
    can_optimize: bool,
    next_opti_dist_2d: f32,
    next_opti_dir_norm_2d: Coord2D,
}

impl PathNode {
    /// `PathNode()`: no links, at the origin, `LAYER_INVALID`, not optimizable, id -1.
    fn new() -> Self {
        PathNode {
            id: -1,
            next_opti: None,
            next: None,
            prev: None,
            pos: Coord3D::default(),
            layer: LAYER_INVALID,
            can_optimize: false,
            next_opti_dist_2d: 0.0,
            next_opti_dir_norm_2d: Coord2D { x: 0.0, y: 0.0 },
        }
    }
    pub fn position(&self) -> &Coord3D {
        &self.pos
    }
    pub fn layer(&self) -> PathfindLayer {
        self.layer
    }
    pub fn can_optimize(&self) -> bool {
        self.can_optimize
    }
    /// `setCanOptimize(canOpt)`.
    pub fn set_can_optimize(&mut self, can_opt: bool) {
        self.can_optimize = can_opt;
    }
    pub fn next(&self) -> Option<usize> {
        self.next
    }
    pub fn previous(&self) -> Option<usize> {
        self.prev
    }
    /// `getNextOptimized(&dir, &dist)`: the next node of the optimized path, with the direction and distance to it.
    pub fn next_optimized(&self) -> (Option<usize>, Coord2D, f32) {
        (self.next_opti, self.next_opti_dir_norm_2d, self.next_opti_dist_2d)
    }
}

/// `struct ClosestPointOnPathInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClosestPointOnPathInfo {
    pub dist_along_path: f32,
    pub pos_on_path: Coord3D,
    pub layer: PathfindLayer,
}

/// `class Path`.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    /// The nodes, by index (the links are indices into it).
    nodes: Vec<PathNode>,
    /// `m_path`: the first node.
    path: Option<usize>,
    /// `m_pathTail`.
    path_tail: Option<usize>,
    /// `m_isOptimized`, `m_blockedByAlly`.
    is_optimized: bool,
    blocked_by_ally: bool,
    /// `m_cpopValid`, `m_cpopCountdown`, `m_cpopIn`, `m_cpopOut`, `m_cpopRecentStart`: the cache of `computePointOnPath`.
    cpop_valid: bool,
    cpop_countdown: i32,
    cpop_in: Coord3D,
    cpop_out: ClosestPointOnPathInfo,
    cpop_recent_start: Option<usize>,
}

impl Default for Path {
    fn default() -> Self {
        Path::new()
    }
}

/// `inline Int IABS(Int x)`.
fn iabs(x: i32) -> i32 {
    if x >= 0 {
        x
    } else {
        x.wrapping_neg()
    }
}

/// `isReallyClose(a, b)`: within 0.1 on each axis.
fn is_really_close(a: &Coord3D, b: &Coord3D) -> bool {
    const CLOSE_ENOUGH: f32 = 0.1;
    (a.x - b.x).abs() <= CLOSE_ENOUGH && (a.y - b.y).abs() <= CLOSE_ENOUGH && (a.z - b.z).abs() <= CLOSE_ENOUGH
}

/// `(Int)r`: C's conversion toward zero (the differences of path positions, well in range).
fn to_int(r: f32) -> i32 {
    r as i32
}

impl Path {
    /// `Path()`: empty, not optimized, the cache invalid with its countdown full and its answer on the ground layer.
    pub fn new() -> Self {
        Path {
            nodes: Vec::new(),
            path: None,
            path_tail: None,
            is_optimized: false,
            blocked_by_ally: false,
            cpop_valid: false,
            cpop_countdown: MAX_CPOP,
            cpop_in: Coord3D::default(),
            cpop_out: ClosestPointOnPathInfo { dist_along_path: 0.0, pos_on_path: Coord3D::default(), layer: LAYER_GROUND },
            cpop_recent_start: None,
        }
    }

    // ---- the inline members -----------------------------------------------------------------------------------------
    /// `getFirstNode()`.
    pub fn first_node(&self) -> Option<usize> {
        self.path
    }
    /// `getLastNode()`.
    pub fn last_node(&self) -> Option<usize> {
        self.path_tail
    }
    pub fn node(&self, i: usize) -> &PathNode {
        &self.nodes[i]
    }
    /// The node `i` mutably (`setCanOptimize`, `setLayer`, `setPosition` on a node).
    pub fn node_mut(&mut self, i: usize) -> &mut PathNode {
        &mut self.nodes[i]
    }
    pub fn set_blocked_by_ally(&mut self, blocked: bool) {
        self.blocked_by_ally = blocked;
    }
    pub fn blocked_by_ally(&self) -> bool {
        self.blocked_by_ally
    }
    /// `markOptimized()`.
    pub fn mark_optimized(&mut self) {
        self.is_optimized = true;
    }
    pub fn is_optimized(&self) -> bool {
        self.is_optimized
    }
    /// `peekCachedPointOnPath(pos)`.
    pub fn peek_cached_point_on_path(&self) -> Coord3D {
        self.cpop_out.pos_on_path
    }
    /// The cache of `computePointOnPath`: valid, the countdown, the position asked, the answer, the recent start.
    pub fn cpop_state(&self) -> (bool, i32, Coord3D, ClosestPointOnPathInfo, Option<usize>) {
        (self.cpop_valid, self.cpop_countdown, self.cpop_in, self.cpop_out, self.cpop_recent_start)
    }

    /// The nodes in list order (`m_path` through `m_next`).
    pub fn list(&self) -> Vec<usize> {
        let mut v = Vec::new();
        let mut n = self.path;
        while let Some(i) = n {
            v.push(i);
            n = self.nodes[i].next;
        }
        v
    }

    fn new_node(&mut self, pos: &Coord3D, layer: PathfindLayer) -> usize {
        let mut node = PathNode::new();
        node.pos = *pos;
        node.layer = layer;
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    // ---- PathNode's list members ------------------------------------------------------------------------------------
    /// `node->setNextOptimized(next)`: the link, the 2D distance to it (0.01 for none) and the unit direction; no link: distance 0.
    pub fn set_next_optimized(&mut self, node: usize, next: Option<usize>) {
        self.nodes[node].next_opti = next;
        match next {
            Some(n) => {
                let to = self.nodes[n].pos;
                let me = &mut self.nodes[node];
                me.next_opti_dir_norm_2d.x = to.x - me.pos.x;
                me.next_opti_dir_norm_2d.y = to.y - me.pos.y;
                me.next_opti_dist_2d = me.next_opti_dir_norm_2d.length();
                if me.next_opti_dist_2d == 0.0 {
                    me.next_opti_dist_2d = if plant(3000) { 0.1 } else { 0.01 };
                }
                me.next_opti_dir_norm_2d.x /= me.next_opti_dist_2d;
                me.next_opti_dir_norm_2d.y /= me.next_opti_dist_2d;
            }
            None => self.nodes[node].next_opti_dist_2d = 0.0,
        }
    }

    /// `node->prependToList(list)`: the node before the list; returns the new list.
    fn prepend_to_list(&mut self, node: usize, list: Option<usize>) -> Option<usize> {
        self.nodes[node].next = list;
        if let Some(l) = list {
            self.nodes[l].prev = Some(node);
        }
        self.nodes[node].prev = None;
        Some(node)
    }

    /// `node->appendToList(list)`: the node after the list's last (found by walking it); returns the list.
    fn append_to_list(&mut self, node: usize, list: Option<usize>) -> Option<usize> {
        let Some(list) = list else {
            self.nodes[node].next = None;
            self.nodes[node].prev = None;
            return Some(node);
        };
        let mut tail = list;
        while let Some(n) = self.nodes[tail].next {
            tail = n;
        }
        self.nodes[tail].next = Some(node);
        self.nodes[node].prev = Some(tail);
        self.nodes[node].next = None;
        Some(list)
    }

    /// `node->computeDirectionVector()`: to the next node; the tail continues its previous node's; a lone node has none.
    pub fn compute_direction_vector(&self, node: usize) -> Coord3D {
        let n = &self.nodes[node];
        match (n.next, n.prev) {
            (None, None) => Coord3D::default(),
            (None, Some(p)) => self.compute_direction_vector(p),
            (Some(next), _) => {
                let m = &self.nodes[next];
                Coord3D { x: m.pos.x - n.pos.x, y: m.pos.y - n.pos.y, z: m.pos.z - n.pos.z }
            }
        }
    }

    // ---- Path ---------------------------------------------------------------------------------------------------------
    /// `prependNode(pos, layer)`.
    pub fn prepend_node(&mut self, pos: &Coord3D, layer: PathfindLayer) {
        let node = self.new_node(pos, layer);
        self.path = self.prepend_to_list(node, self.path);
        if self.path_tail.is_none() {
            self.path_tail = Some(node);
        }
        self.is_optimized = false;
    }

    /// `appendNode(pos, layer)`.
    pub fn append_node(&mut self, pos: &Coord3D, layer: PathfindLayer) {
        if self.is_optimized {
            if let Some(t) = self.path_tail {
                // "Check for duplicates."
                let tp = self.nodes[t].pos;
                if pos.x == tp.x && (pos.y == tp.y || plant(3008)) {
                    return;
                }
            }
        }
        let node = self.new_node(pos, layer);
        self.path = self.append_to_list(node, self.path);
        if self.is_optimized {
            if let Some(t) = self.path_tail {
                self.set_next_optimized(t, Some(node));
            }
        }
        self.path_tail = Some(node);
    }

    /// `updateLastNode(pos)`.
    pub fn update_last_node<H: PathHost>(&mut self, host: &mut H, pos: &Coord3D) {
        let layer = host.layer_for_destination(pos);
        if let Some(t) = self.path_tail {
            self.nodes[t].pos = *pos;
            self.nodes[t].layer = layer;
        }
        if self.is_optimized {
            if let Some(tail) = self.path_tail {
                let mut node = self.path;
                while let Some(n) = node {
                    if self.nodes[n].next_opti == Some(tail) {
                        break;
                    }
                    node = self.nodes[n].next_opti;
                }
                if let Some(n) = node {
                    if self.nodes[n].next_opti == Some(tail) {
                        self.set_next_optimized(n, Some(tail));
                    }
                }
            }
        }
    }

    /// The node at most three steps past a change of layer from `anchor` (or the last but one), and the layer to look on: the first
    /// loop of `optimize` and `optimizeGroundPath` (`stop_at_no_opt`: `optimize` also stops at a node that cannot be optimized).
    fn farthest_candidate(&self, anchor: usize, stop_at_no_opt: bool) -> (usize, PathfindLayer) {
        const ALLOWED_STEPS: i32 = 3; // "we can optimize 3 steps to or from a bridge. Otherwise, we need to insert a point. jba."
        let mut layer = self.nodes[anchor].layer;
        let mut cur_layer = self.nodes[anchor].layer;
        let mut count = 0;
        let mut node = self.nodes[anchor].next.expect("an anchor before the last node has a next");
        while let Some(next) = self.nodes[node].next {
            count += 1;
            if cur_layer == LAYER_GROUND {
                if self.nodes[node].layer != cur_layer {
                    layer = self.nodes[node].layer;
                    // (upstream's `curLayer = layer` here is overwritten below by the same layer before it is read)
                    if count > ALLOWED_STEPS {
                        break;
                    }
                }
            } else if self.nodes[next].layer != cur_layer && count > ALLOWED_STEPS {
                break;
            }
            cur_layer = self.nodes[node].layer;
            if stop_at_no_opt && !self.nodes[node].can_optimize {
                break;
            }
            node = next;
        }
        (node, layer)
    }

    /// The step checks of both optimizations: "Horizontal, diagonal, and vertical steps are passable." `one_cell` adds `optimize`'s
    /// check of a step of exactly one cell on both axes. Each check walks the steps back from `node` to the anchor (`tmpNode =
    /// node->getPrevious(); tmpNode && tmpNode != anchor`), and leaves in `dx` and `dy` the last step it read, which the later checks
    /// compare (upstream reuses the variables).
    fn step_is_passable(&self, anchor: usize, node: usize, one_cell: bool) -> bool {
        let (np, ap) = (self.nodes[node].pos, self.nodes[anchor].pos);
        let mut dx = to_int(np.x - ap.x);
        let mut dy = to_int(np.y - ap.y);
        let mut passable = false;
        let mut might_be_passable = false;
        if one_cell && iabs(dx) == PATHFIND_CELL_SIZE && iabs(dy) == PATHFIND_CELL_SIZE && !plant(3001) {
            passable = true;
        }
        let mut steps: Vec<(i32, i32)> = Vec::new();
        let mut tmp = self.nodes[node].prev;
        while let Some(t) = tmp {
            if t == anchor {
                break;
            }
            let next = self.nodes[t].next.expect("a node before `node` has a next");
            let (tp, xp) = (self.nodes[t].pos, self.nodes[next].pos);
            steps.push((to_int(xp.x - tp.x), to_int(xp.y - tp.y)));
            tmp = self.nodes[t].prev;
        }
        if dx == 0 {
            might_be_passable = true;
            for &(x, _) in &steps {
                dx = x;
                if dx != 0 {
                    might_be_passable = false;
                }
            }
        }
        if dy == 0 {
            might_be_passable = true;
            for &(_, y) in &steps {
                dy = y;
                if dy != 0 {
                    might_be_passable = false;
                }
            }
        }
        if dx == dy {
            might_be_passable = true;
            for &(x, y) in &steps {
                dx = x;
                dy = y;
                if dy != dx {
                    might_be_passable = false;
                }
            }
        }
        if dx == dy.wrapping_neg() {
            might_be_passable = true;
            for &(x, y) in &steps {
                if y != x.wrapping_neg() {
                    might_be_passable = false;
                }
            }
        }
        passable || might_be_passable
    }

    /// `optimize(obj, acceptableSurfaces, blocked)`: "Optimize the path by checking line of sight". An empty path is undefined
    /// upstream (the first node is read); the caller has nodes.
    pub fn optimize<H: PathHost>(&mut self, host: &mut H, obj: ObjectId, acceptable_surfaces: LocomotorSurfaceTypeMask, blocked: bool) {
        let Some(mut anchor) = self.path else { return };
        let mut first_node = true;
        let first_layer = self.nodes[anchor].layer;
        while Some(anchor) != self.path_tail {
            // "find the farthest node in the path that has a clear line-of-sight to this anchor"
            let mut optimized_segment = false;
            let (mut node, mut layer) = self.farthest_candidate(anchor, true);
            if first_node {
                layer = first_layer;
                first_node = false;
            }
            while node != anchor {
                let (ap, np) = (self.nodes[anchor].pos, self.nodes[node].pos);
                let mut is_passable = host.is_line_passable(obj, acceptable_surfaces, layer, &ap, &np, blocked, false);
                if let Some((ty, pinched)) = host.cell_type_and_pinched(layer, &np) {
                    if ty == CELL_CLIFF && !pinched && !plant(3002) {
                        is_passable = true;
                    }
                }
                if !is_passable && self.step_is_passable(anchor, node, true) {
                    is_passable = true;
                }
                if is_passable {
                    // "anchor can directly see this node, make it next in the optimized path"
                    self.set_next_optimized(anchor, Some(node));
                    anchor = node;
                    optimized_segment = true;
                    break;
                }
                node = self.nodes[node].prev.expect("the walk back reaches the anchor");
            }
            if !optimized_segment {
                // "for some reason, there is no clear LOS between the anchor node and the very next node"
                let next = self.nodes[anchor].next;
                self.set_next_optimized(anchor, next);
                anchor = next.expect("an anchor before the last node has a next");
            }
        }
        self.is_optimized = true;
    }

    /// `optimizeGroundPath(crusher, pathDiameter)`. An empty path is undefined upstream; the caller has nodes.
    pub fn optimize_ground_path<H: PathHost>(&mut self, host: &mut H, crusher: bool, path_diameter: i32) {
        let Some(mut anchor) = self.path else { return };
        while Some(anchor) != self.path_tail {
            let mut optimized_segment = false;
            let (mut node, layer) = self.farthest_candidate(anchor, false);
            while node != anchor {
                let (ap, np) = (self.nodes[anchor].pos, self.nodes[node].pos);
                let mut is_passable = host.is_ground_path_passable(crusher, &ap, layer, &np, path_diameter);
                if !is_passable && self.step_is_passable(anchor, node, false) {
                    is_passable = true;
                }
                if is_passable {
                    self.set_next_optimized(anchor, Some(node));
                    anchor = node;
                    optimized_segment = true;
                    break;
                }
                node = self.nodes[node].prev.expect("the walk back reaches the anchor");
            }
            if !optimized_segment {
                let next = self.nodes[anchor].next;
                self.set_next_optimized(anchor, next);
                anchor = next.expect("an anchor before the last node has a next");
            }
        }
        // "Remove jig/jogs :) jba."
        let mut anchor = self.path;
        while let Some(a) = anchor {
            if let Some(node) = self.nodes[a].next_opti {
                if let Some(after) = self.nodes[node].next_opti {
                    let dx = self.nodes[node].pos.x - self.nodes[a].pos.x;
                    let dy = self.nodes[node].pos.y - self.nodes[a].pos.y;
                    // "If the x & y offsets are less than 2 pathfind cells, kill it."
                    if dx * dx + dy * dy < sqr(PATHFIND_CELL_SIZE_F) * if plant(3003) { 4.1 } else { 3.9 } {
                        self.set_next_optimized(a, Some(after));
                    }
                }
            }
            anchor = self.nodes[a].next_opti;
        }
        self.is_optimized = true;
    }

    /// `computePointOnPath(obj, locomotorSet, pos, out)`: the closest point on the optimized path to `pos`, led a little towards the
    /// next node, and the distance along the path from it to the end. `surfaces` is the locomotor set's `getValidSurfaces()`.
    pub fn compute_point_on_path<H: PathHost>(&mut self, host: &mut H, obj: ObjectId, surfaces: LocomotorSurfaceTypeMask, pos: &Coord3D) -> ClosestPointOnPathInfo {
        let mut out = ClosestPointOnPathInfo { dist_along_path: 0.0, pos_on_path: Coord3D::default(), layer: LAYER_GROUND };
        let Some(first) = self.path else {
            self.cpop_valid = false;
            return out;
        };
        out.layer = self.nodes[first].layer;
        if self.cpop_valid && self.cpop_countdown > 0 && is_really_close(pos, &self.cpop_in) {
            out = self.cpop_out;
            if !plant(3004) {
                self.cpop_countdown -= 1;
            }
            return out;
        }
        self.cpop_countdown = MAX_CPOP;
        // "default pathPos to end of the path"
        out.pos_on_path = self.nodes[self.path_tail.expect("a path with a first node has a tail")].pos;
        let mut close_node: Option<usize> = None;
        let mut close_dist_sqr: f32 = 99_999_999.9;
        let mut total_path_length: f32 = 0.0;
        let mut length_along_path_to_pos: f32 = 0.0;
        // "Find the closest segment of the path"
        let mut prev_node = first;
        let (mut node, mut segment_dir_norm, mut segment_length) = self.nodes[prev_node].next_optimized();
        while let Some(n) = node {
            let prev_node_pos = self.nodes[prev_node].pos;
            let node_pos = self.nodes[n].pos;
            let to_pos = Coord2D { x: pos.x - prev_node_pos.x, y: pos.y - prev_node_pos.y };
            let mut along_path_dist = segment_dir_norm.x * to_pos.x + segment_dir_norm.y * to_pos.y;
            let point_on_path;
            if along_path_dist < 0.0 {
                along_path_dist = 0.0;
                point_on_path = prev_node_pos;
            } else if along_path_dist > segment_length {
                if self.nodes[n].next_opti.is_none() {
                    along_path_dist = segment_length;
                    point_on_path = node_pos;
                } else {
                    // "beyond the end of this segment, skip this segment"
                    total_path_length += segment_length;
                    prev_node = n;
                    (node, segment_dir_norm, segment_length) = self.nodes[n].next_optimized();
                    continue;
                }
            } else {
                point_on_path = Coord3D {
                    x: prev_node_pos.x + along_path_dist * segment_dir_norm.x,
                    y: prev_node_pos.y + along_path_dist * segment_dir_norm.y,
                    z: 0.0,
                };
            }
            let offset = Coord2D { x: pos.x - point_on_path.x, y: pos.y - point_on_path.y };
            let offset_dist_sqr = offset.x * offset.x + offset.y * offset.y;
            if offset_dist_sqr < close_dist_sqr {
                close_dist_sqr = offset_dist_sqr;
                close_node = Some(prev_node);
                out.pos_on_path = point_on_path;
                length_along_path_to_pos = total_path_length + along_path_dist;
            }
            total_path_length += segment_length;
            prev_node = n;
            (node, segment_dir_norm, segment_length) = self.nodes[n].next_optimized();
        }
        // "Compute the goal movement position for this agent"
        if let Some(close) = close_node.filter(|&c| self.nodes[c].next_opti.is_some()) {
            let (close_next, segment_dir_norm, segment_length) = self.nodes[close].next_optimized();
            let close_next = close_next.expect("the closest node has an optimized next");
            let next_node_pos = self.nodes[close_next].pos;
            let close_node_pos = self.nodes[close].pos;
            if let Some(p) = self.nodes[close].prev {
                if self.nodes[p].layer > LAYER_GROUND {
                    out.layer = self.nodes[close].layer;
                }
            }
            if self.nodes[close].layer > LAYER_GROUND {
                out.layer = self.nodes[close].layer;
            }
            if self.nodes[close_next].layer > LAYER_GROUND {
                out.layer = self.nodes[close_next].layer;
            }
            let to_pos = Coord2D { x: pos.x - close_node_pos.x, y: pos.y - close_node_pos.y };
            let mut along_path_dist = segment_dir_norm.x * to_pos.x + segment_dir_norm.y * to_pos.y;
            // "we know this is the closest segment, so don't allow farther back than the start node"
            if along_path_dist < 0.0 {
                along_path_dist = 0.0;
            }
            let to_dist_sqr = sqr(to_pos.x) + sqr(to_pos.y);
            let offset_dist_sq = to_dist_sqr - sqr(along_path_dist);
            let offset_dist = if f64::from(offset_dist_sq) <= 0.0 { 0.0 } else { sqrt_via_double(offset_dist_sq) };
            let max_path_error: f32 = 3.0 * PATHFIND_CELL_SIZE_F;
            let max_path_error_inv = (1.0f64 / f64::from(max_path_error)) as f32;
            let mut k = offset_dist * max_path_error_inv;
            if k > 1.0 {
                k = 1.0;
            }
            let mut got_pos = false;
            if host.is_line_passable(obj, surfaces, out.layer, pos, &next_node_pos, false, true) {
                out.pos_on_path = next_node_pos;
                got_pos = true;
                let mut try_ahead = f64::from(along_path_dist) > f64::from(segment_length) * if plant(3005) { 0.6 } else { 0.5 };
                if !self.nodes[close_next].can_optimize {
                    try_ahead = false; // "don't go past no-opt nodes."
                }
                if self.nodes[close].layer != self.nodes[close_next].layer {
                    try_ahead = false; // "don't go past layers."
                }
                if host.object_layer(obj) != LAYER_GROUND {
                    try_ahead = false;
                }
                let mut very_close = false;
                if segment_length - along_path_dist < 1.0 {
                    try_ahead = true;
                    very_close = true;
                }
                if try_ahead {
                    // "try next segment middle."
                    if let Some(next) = self.nodes[close_next].next_opti {
                        let np = self.nodes[next].pos;
                        let try_pos = Coord3D {
                            x: (f64::from(next_node_pos.x + np.x) * 0.5) as f32,
                            y: (f64::from(next_node_pos.y + np.y) * 0.5) as f32,
                            z: next_node_pos.z,
                        };
                        if very_close || host.is_line_passable(obj, surfaces, self.nodes[close_next].layer, pos, &try_pos, false, true) {
                            got_pos = true;
                            out.pos_on_path = try_pos;
                        }
                    }
                }
            } else if k > 0.5 {
                let try_dist = (f64::from(along_path_dist) + 0.5 * f64::from(segment_length - along_path_dist)) as f32;
                out.pos_on_path.x = close_node_pos.x + try_dist * segment_dir_norm.x;
                out.pos_on_path.y = close_node_pos.y + try_dist * segment_dir_norm.y;
                out.pos_on_path.z = close_node_pos.z;
                let try_pos = out.pos_on_path;
                if host.is_line_passable(obj, surfaces, out.layer, pos, &try_pos, false, true) {
                    k = 0.5;
                    got_pos = true;
                }
            }
            // "if we are on the path (k == 0), then alongPathDist == segmentLength"
            along_path_dist += (1.0 - k) * (segment_length - along_path_dist);
            if !got_pos {
                if along_path_dist > segment_length {
                    out.pos_on_path = next_node_pos;
                } else {
                    out.pos_on_path.x = close_node_pos.x + along_path_dist * segment_dir_norm.x;
                    out.pos_on_path.y = close_node_pos.y + along_path_dist * segment_dir_norm.y;
                    out.pos_on_path.z = close_node_pos.z;
                    let dx = (pos.x - out.pos_on_path.x).abs();
                    let dy = (pos.y - out.pos_on_path.y).abs();
                    if dx < 1.0 && dy < 1.0 {
                        if let Some(n2) = self.nodes[close].next_opti.and_then(|n| self.nodes[n].next_opti) {
                            out.pos_on_path = self.nodes[n2].pos;
                        }
                    }
                }
            }
        }
        out.dist_along_path = total_path_length - length_along_path_to_pos;
        let delta = Coord3D { x: out.pos_on_path.x - pos.x, y: out.pos_on_path.y - pos.y, z: 0.0 };
        let len_delta = delta.length();
        if len_delta > out.dist_along_path && out.dist_along_path > PATHFIND_CLOSE_ENOUGH {
            out.dist_along_path = len_delta;
        }
        self.cpop_in = *pos;
        self.cpop_out = out;
        self.cpop_valid = true;
        out
    }

    /// `computeFlightDistToGoal(pos, goalPos)`: "the distance to the goal (0 if we are past it)" and the goal position, for a flight
    /// path that goes straight to the goal.
    pub fn compute_flight_dist_to_goal(&mut self, pos: &Coord3D) -> (f32, Coord3D) {
        let Some(first) = self.path else { return (0.0, Coord3D::default()) };
        let mut cur_node = first;
        match self.cpop_recent_start {
            Some(s) => cur_node = s,
            None => self.cpop_recent_start = Some(cur_node),
        }
        let mut next_node = self.nodes[cur_node].next_opti;
        let mut goal_pos = self.nodes[cur_node].pos;
        let mut distance: f32 = 0.0;
        let mut use_next = true;
        while let Some(next) = next_node {
            if use_next {
                goal_pos = self.nodes[next].pos;
            }
            let start_pos = self.nodes[cur_node].pos;
            let end_pos = self.nodes[next].pos;
            let pos_to_goal = Coord2D { x: end_pos.x - pos.x, y: end_pos.y - pos.y };
            let mut path_vector = Coord2D { x: end_pos.x - start_pos.x, y: end_pos.y - start_pos.y };
            path_vector.normalize();
            let dot_product = pos_to_goal.x * path_vector.x + pos_to_goal.y * path_vector.y;
            if dot_product > 0.0 || (dot_product == 0.0 && !plant(3007)) {
                distance += dot_product;
                use_next = false;
            } else if use_next {
                self.cpop_recent_start = Some(next);
            }
            cur_node = next;
            next_node = self.nodes[cur_node].next_opti;
        }
        (distance, goal_pos)
    }
}

impl Snapshot for Path {
    /// `crc`: nothing.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }
    /// `xfer` (version 1): the version, the node count, the nodes from the tail back (each its number, position, layer as four bytes,
    /// optimizable flag and the number of its optimized next, -1 for none), the optimized flag, two obsolete words, the blocked flag.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        let mut count = self.list().len() as i32;
        xfer.xfer_int(&mut count)?;
        if xfer.get_xfer_mode() == XferMode::Save {
            // "Write them out backwards."
            let mut node = self.path_tail;
            while let Some(n) = node {
                self.nodes[n].id = count;
                xfer.xfer_int(&mut count)?;
                let mut pos = self.nodes[n].pos;
                xfer.xfer_coord3d(&mut pos)?;
                let mut layer = self.nodes[n].layer.to_le_bytes();
                xfer.xfer_user(&mut layer)?;
                let mut can_opt = self.nodes[n].can_optimize;
                xfer.xfer_bool(&mut can_opt)?;
                let mut id = self.nodes[n].next_opti.map_or(-1, |o| self.nodes[o].id);
                xfer.xfer_int(&mut id)?;
                count -= 1;
                node = if plant(3006) { self.nodes[n].next } else { self.nodes[n].prev };
            }
        } else {
            self.cpop_valid = false;
            while count != 0 {
                let mut node_id = 0i32;
                xfer.xfer_int(&mut node_id)?;
                let mut pos = Coord3D::default();
                xfer.xfer_coord3d(&mut pos)?;
                let mut layer = [0u8; 4];
                xfer.xfer_user(&mut layer)?;
                let mut can_opt = false;
                xfer.xfer_bool(&mut can_opt)?;
                let mut opt_id = -1i32;
                xfer.xfer_int(&mut opt_id)?;
                let node = self.new_node(&pos, i32::from_le_bytes(layer));
                self.nodes[node].id = node_id;
                self.nodes[node].can_optimize = can_opt;
                let mut opt_node = None;
                if opt_id > 0 {
                    let mut o = self.path;
                    while let Some(x) = o {
                        if self.nodes[x].id == opt_id {
                            break;
                        }
                        o = self.nodes[x].next;
                    }
                    opt_node = o;
                }
                self.path = self.prepend_to_list(node, self.path);
                if self.path_tail.is_none() {
                    self.path_tail = Some(node);
                }
                if opt_node.is_some() {
                    self.set_next_optimized(node, opt_node);
                }
                count -= 1;
            }
        }
        xfer.xfer_bool(&mut self.is_optimized)?;
        let mut obsolete1 = 0i32;
        xfer.xfer_int(&mut obsolete1)?;
        let mut obsolete2 = 0u32;
        xfer.xfer_unsigned_int(&mut obsolete2)?;
        xfer.xfer_bool(&mut self.blocked_by_ally)
    }
    fn load_post_process(&mut self) {}
}
