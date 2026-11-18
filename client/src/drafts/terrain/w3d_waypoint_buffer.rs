//! UNTESTED DRAFT (#202, #204, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DWaypointBuffer.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3dWaypointBuffer.cpp
//!
//! Waypoint and rally point lines for the selected drawables. What upstream reads from the game objects
//! (`Drawable`, `Object`, `AIUpdateInterface`, `ExitInterface`, `TheInGameUI`, `TheGameClient`) is a plain snapshot the
//! caller builds ([`WaypointDrawable`], [`WaypointContext`]); what it renders (`WW3D::Render(*m_waypointNodeRobj, ..)` for
//! the little "hockey puck" node and `m_line->Render`) is returned in order as [`WaypointDrawCommand`]s. The segmented
//! line's style (`m_line`) persists between lines within a call exactly as the members do upstream.
//!
//! Notes on upstream behaviour kept as it is:
//! - `NRPToCtrDelta.x = 0.0f` is set where `.z` was probably meant and the delta is never used.
//! - `(Real)cos(angle)` and `(Real)sin(angle)` are the double functions of <math.h> cast to float.
//! - After a listening outpost is handled, the loop `break`s ("the single path-line limit"), also when no line was drawn.
//! - `TheInGameUI->isInWaypointMode()` selects between the waypoint branch and the rally point branch.

use super::w3d_common::TextureHandle;
use rts_lockstep_engine::common::base_type::{Coord2D, Coord3D};
use rts_lockstep_engine::ww::vector3::Vector3;

/// `MAX_DISPLAY_NODES`.
pub const MAX_DISPLAY_NODES: usize = 512;

/// The AI part of an object: `AIUpdateInterface`'s `friend_getWaypointGoalPathSize`, `friend_getCurrentGoalPathIndex`,
/// `friend_getGoalPathPosition(i)` (`None` for NULL) and `getGoalPosition`.
#[derive(Clone, Debug, Default)]
pub struct WaypointAiInfo {
    /// `friend_getWaypointGoalPathSize()`.
    pub goal_path_size: i32,
    /// `friend_getCurrentGoalPathIndex()`.
    pub current_goal_path_index: i32,
    /// `friend_getGoalPathPosition(i)` for `i` in `0..goal_path_size`.
    pub goal_path: Vec<Option<Coord3D>>,
    /// `getGoalPosition()`.
    pub goal_position: Coord3D,
}

/// The exit part of an object: `ExitInterface` (`getObjectExitInterface`).
#[derive(Clone, Debug, Default)]
pub struct WaypointExitInfo {
    /// `getExitPosition(exitPoint)`: `Some` when it returns true.
    pub exit_position: Option<Coord3D>,
    /// `getNaturalRallyPoint(point, FALSE)`: `Some` when it returns true.
    pub natural_rally_point: Option<Coord3D>,
    /// `getRallyPoint()`.
    pub rally_point: Option<Coord3D>,
}

/// An `Object` as the waypoint buffer reads it.
#[derive(Clone, Debug, Default)]
pub struct WaypointObject {
    /// `getPosition()`.
    pub position: Coord3D,
    /// `isKindOf(KINDOF_IGNORED_IN_GUI)`.
    pub kind_ignored_in_gui: bool,
    /// `isLocallyControlled()`.
    pub locally_controlled: bool,
    /// `isKindOf(KINDOF_REVEALS_ENEMY_PATHS)`.
    pub kind_reveals_enemy_paths: bool,
    /// `getOrientation()`.
    pub orientation: f32,
    /// `getGeometryInfo().getMajorRadius()`.
    pub major_radius: f32,
    /// `getGeometryInfo().getMinorRadius()`.
    pub minor_radius: f32,
    /// `getVisionRange()`.
    pub vision_range: f32,
    /// `getAI()`.
    pub ai: Option<WaypointAiInfo>,
    /// `getObjectExitInterface()`.
    pub exit: Option<WaypointExitInfo>,
}

/// A selected `Drawable`: `getObject()` (`None` for NULL).
#[derive(Clone, Debug, Default)]
pub struct WaypointDrawable {
    /// `draw->getObject()`.
    pub object: Option<WaypointObject>,
}

/// The mouse-over enemy of a listening outpost: `TheGameClient->findDrawableByID(TheInGameUI->getMousedOverDrawableID())`
/// then its object, and `enemy->getRelationship(obj) == ENEMIES` (the relationship is to the selected object).
#[derive(Clone, Debug, Default)]
pub struct WaypointEnemy {
    /// `enemyDraw->getObject()`.
    pub object: WaypointObject,
    /// `enemy->getRelationship(obj) == ENEMIES`.
    pub is_enemy: bool,
}

/// What `drawWaypoints` reads from `TheInGameUI` and `TheGameClient`.
#[derive(Clone, Debug, Default)]
pub struct WaypointContext {
    /// `TheInGameUI != NULL`.
    pub in_game_ui_present: bool,
    /// `TheInGameUI->isInWaypointMode()`.
    pub in_waypoint_mode: bool,
    /// `TheInGameUI->getAllSelectedDrawables()`.
    pub selected: Vec<WaypointDrawable>,
    /// The mouse-over drawable's object when it exists (`None` when no drawable or no object).
    pub moused_over_enemy: Option<WaypointEnemy>,
}

/// One thing `drawWaypoints` renders.
#[derive(Clone, Debug, PartialEq)]
pub enum WaypointDrawCommand {
    /// `m_waypointNodeRobj->Set_Position(pos)` then `WW3D::Render(*m_waypointNodeRobj, localRinfo)`.
    Node(Vector3),
    /// `m_line->Set_Points(numPoints, points)` then `m_line->Render(localRinfo)`, with the line's current style.
    Line {
        /// The points.
        points: Vec<Vector3>,
        /// `Set_Color` in effect.
        color: Vector3,
        /// `Set_Width` in effect.
        width: f32,
    },
}

/// `class W3DWaypointBuffer`.
#[derive(Clone, Debug)]
pub struct W3DWaypointBuffer {
    /// `m_waypointNodeRobj`: the "SCMNode" render object (the renderer's); whether it was created.
    pub waypoint_node_robj_present: bool,
    /// `m_texture`: "EXLaser.tga".
    pub texture: Option<TextureHandle>,
    /// `m_line->Set_Color`.
    pub line_color: Vector3,
    /// `m_line->Set_Width`.
    pub line_width: f32,
    /// Whether `m_line` has its texture set (`Set_Texture(m_texture)`).
    pub line_has_texture: bool,
}

impl W3DWaypointBuffer {
    /// `W3DWaypointBuffer::W3DWaypointBuffer` (W3dWaypointBuffer.cpp:79). `node_robj_present` is
    /// `Create_Render_Obj("SCMNode") != NULL`, `texture` is `Get_Texture("EXLaser.tga")`.
    pub fn new(node_robj_present: bool, texture: Option<TextureHandle>) -> Self {
        let mut b = W3DWaypointBuffer {
            waypoint_node_robj_present: node_robj_present,
            texture,
            line_color: Vector3::default(),
            line_width: 0.0,
            line_has_texture: false,
        };
        b.set_default_line_style();
        b
    }

    /// `W3DWaypointBuffer::~W3DWaypointBuffer` (W3dWaypointBuffer.cpp:92): releases the node, texture and line.
    pub fn destroy(&mut self) {
        self.waypoint_node_robj_present = false;
        self.texture = None;
    }

    /// `W3DWaypointBuffer::freeWaypointBuffers` (W3dWaypointBuffer.cpp:105): empty.
    pub fn free_waypoint_buffers(&mut self) {}

    /// `W3DWaypointBuffer::setDefaultLineStyle` (W3dWaypointBuffer.cpp:111): texture, additive shader with
    /// `PASS_ALWAYS` depth compare, width 1.5, colour (0.25, 0.5, 1.0), tiled texture mapping.
    pub fn set_default_line_style(&mut self) {
        if self.texture.is_some() {
            self.line_has_texture = true;
        }
        // lineShader = ShaderClass::_PresetAdditiveShader with Set_Depth_Compare(PASS_ALWAYS); Set_Texture_Mapping_Mode
        // (TILED_TEXTURE_MAP) — the renderer's
        self.line_width = 1.5;
        self.line_color = Vector3::new(0.25, 0.5, 1.0);
    }

    fn line(&self, points: &[Vector3]) -> WaypointDrawCommand {
        WaypointDrawCommand::Line { points: points.to_vec(), color: self.line_color, width: self.line_width }
    }

    /// `W3DWaypointBuffer::drawWaypoints` (W3dWaypointBuffer.cpp:128): draws the waypoints. (The default light environment
    /// with only full ambient, and the local `RenderInfoClass`, are the renderer's.)
    pub fn draw_waypoints(&mut self, ctx: &WaypointContext) -> Vec<WaypointDrawCommand> {
        let mut out: Vec<WaypointDrawCommand> = Vec::new();

        if !ctx.in_game_ui_present {
            return out;
        }

        self.set_default_line_style();

        if ctx.in_waypoint_mode {
            let mut points: Vec<Vector3> = vec![Vector3::default(); MAX_DISPLAY_NODES + 1]; // Lines have nodes + 1 points.

            for draw in ctx.selected.iter() {
                let obj = draw.object.as_ref();
                let mut num_points: usize = 1;
                if let Some(obj) = obj {
                    if !obj.kind_ignored_in_gui {
                        // so mobs and stuff dont make a gazillion lines
                        let ai = obj.ai.as_ref();
                        let goal_size = ai.map_or(0, |a| a.goal_path_size);
                        let gp_idx = ai.map_or(0, |a| a.current_goal_path_index);
                        if let Some(ai) = ai {
                            if gp_idx >= 0 && gp_idx < goal_size {
                                let pos = obj.position;
                                points[0].set(pos.x, pos.y, pos.z);

                                for i in gp_idx..goal_size {
                                    let waypoint = ai.goal_path.get(i as usize).copied().flatten();
                                    if let Some(waypoint) = waypoint {
                                        // Render line from previous point to current node.

                                        if num_points < MAX_DISPLAY_NODES + 1 {
                                            points[num_points].set(waypoint.x, waypoint.y, waypoint.z);
                                            num_points += 1;
                                        }

                                        out.push(WaypointDrawCommand::Node(Vector3::new(waypoint.x, waypoint.y, waypoint.z)));
                                    }
                                }
                                // Now render the lines in one pass!
                                out.push(self.line(&points[..num_points]));
                            }
                        }
                    }
                }
            }
        } else {
            // maybe we want to draw rally points, then?
            let mut points: Vec<Vector3> = vec![Vector3::default(); MAX_DISPLAY_NODES + 1]; // Lines have nodes + 1 points.

            for draw in ctx.selected.iter() {
                let obj = draw.object.as_ref();

                let mut num_points: usize = 0;
                if let Some(obj) = obj {
                    if !obj.locally_controlled {
                        continue;
                    }

                    // WAIT! before we go browsing the drawable list for buildings that want to draw their rally points
                    // lets test for that very special case of having a listeningoutpost selected, and some enemy
                    // drawable moused-over
                    if obj.kind_reveals_enemy_paths {
                        if let Some(enemy_info) = ctx.moused_over_enemy.as_ref() {
                            let enemy = &enemy_info.object;
                            if enemy_info.is_enemy {
                                let mut delta = obj.position;
                                delta.sub(&enemy.position);
                                if delta.length() <= obj.vision_range {
                                    // is listening outpost close enough to do this?
                                    let ai = enemy.ai.as_ref();
                                    let goal_size = ai.map_or(0, |a| a.goal_path_size);
                                    let gp_idx = ai.map_or(0, |a| a.current_goal_path_index);
                                    if let Some(ai) = ai {
                                        let mut line_exists = false;

                                        let pos = enemy.position;
                                        points[num_points].set(pos.x, pos.y, pos.z);
                                        num_points += 1;

                                        if gp_idx >= 0 && gp_idx < goal_size {
                                            // Ooh, the enemy is in waypoint mode
                                            for i in gp_idx..goal_size {
                                                let waypoint = ai.goal_path.get(i as usize).copied().flatten();
                                                if let Some(waypoint) = waypoint {
                                                    // Render line from previous point to current node.

                                                    if num_points < MAX_DISPLAY_NODES + 1 {
                                                        points[num_points].set(waypoint.x, waypoint.y, waypoint.z);
                                                        num_points += 1;
                                                    }

                                                    out.push(WaypointDrawCommand::Node(Vector3::new(waypoint.x, waypoint.y, waypoint.z)));
                                                    line_exists = true;
                                                }
                                            }
                                        } else {
                                            // then enemy may be moving to a goal position
                                            let destination_point = ai.goal_position;
                                            if destination_point.length() > 1.0 {
                                                points[num_points].set(destination_point.x, destination_point.y, destination_point.z);
                                                num_points += 1;
                                                out.push(WaypointDrawCommand::Node(Vector3::new(destination_point.x, destination_point.y, destination_point.z)));
                                                line_exists = true;
                                            }
                                        }

                                        if line_exists {
                                            // Now render the lines in one pass!
                                            self.line_color = Vector3::new(0.95, 0.5, 0.0);
                                            self.line_width = 3.0;

                                            out.push(self.line(&points[..num_points]));
                                        }
                                    }
                                }
                            }
                        }

                        break; // dont even bother with the rest, since this one listening outpost satisfies the single path-line limit
                    }

                    let exit_interface = match obj.exit.as_ref() {
                        Some(e) => e,
                        None => continue,
                    };

                    let exit_point: Coord3D = match exit_interface.exit_position {
                        Some(p) => p,
                        None => obj.position,
                    };

                    points[num_points].set(exit_point.x, exit_point.y, exit_point.z);
                    num_points += 1;

                    let mut box_wrap = true;
                    let natural_rally_point: Coord3D;
                    if let Some(nrp) = exit_interface.natural_rally_point {
                        // FALSE means "without the extra offset"
                        natural_rally_point = nrp;
                        if !natural_rally_point.equals(&exit_point) {
                            points[num_points].set(natural_rally_point.x, natural_rally_point.y, natural_rally_point.z);
                            num_points += 1;
                        } else {
                            // Helipad rally point -- so don't use box wrapping.
                            box_wrap = false;
                        }
                    } else {
                        continue; // next drawable
                    }

                    if let Some(rally_point) = exit_interface.rally_point {
                        if box_wrap {
                            // test to se whether the rally point flanks the side of the natural rally point
                            // to do this we find the two corners of the geometry extents that are nearest the natural
                            // rally point; these define the natural rally point edge. an intermediate point should be
                            // inserted at the corner nearest the rally point
                            let ctr = obj.position;
                            let mut nrp_delta = Coord3D::default();
                            nrp_delta.x = natural_rally_point.x - exit_point.x;
                            nrp_delta.y = natural_rally_point.y - exit_point.y;
                            nrp_delta.z = 0.0;

                            // This is a quick idiot test to se whether the rally line needs to wrap the box at all
                            let mut way_out_point = nrp_delta;
                            way_out_point.normalize();
                            way_out_point.scale(99999.9);
                            let way_out_length = way_out_point.length();
                            way_out_point.add(&natural_rally_point);

                            // if the rallypoint is closer to the wayoutpoint than it is to the natural rally point then
                            // we definitely do not wrap
                            let mut rally_to_way_out_delta = way_out_point;
                            rally_to_way_out_delta.sub(&rally_point);
                            if (100.0 + rally_to_way_out_delta.length()) > way_out_length {
                                // if we passed the above idiot test, now lets be sure by testing the dotproduct of the rp
                                // against the wayoutpoint
                                way_out_point.normalize(); // a normal shooting straight out the door
                                // next comes the delta between the NRP and the RP
                                let mut nrp_to_rp_delta = natural_rally_point;
                                nrp_to_rp_delta.sub(&rally_point);
                                nrp_to_rp_delta.normalize();
                                let mut dot = nrp_to_rp_delta.x * way_out_point.x + nrp_to_rp_delta.y * way_out_point.y;
                                if dot > 0.0 {
                                    let angle = obj.orientation;
                                    // (Real)cos(angle): the double function cast to float
                                    let c = super::double_math::c_cos(angle);
                                    let s = super::double_math::c_sin(angle);

                                    // NRPToCtrDelta (x, y, then x = 0.0f) is never used upstream

                                    let exc = obj.major_radius * c;
                                    let eyc = obj.minor_radius * c;
                                    let exs = obj.major_radius * s;
                                    let eys = obj.minor_radius * s;

                                    let mut corners = [Coord2D::default(); 4];

                                    corners[0].x = ctr.x - exc - eys;
                                    corners[0].y = ctr.y + eyc - exs;
                                    corners[1].x = ctr.x + exc - eys;
                                    corners[1].y = ctr.y + eyc + exs;
                                    corners[2].x = ctr.x + exc + eys;
                                    corners[2].y = ctr.y - eyc + exs;
                                    corners[3].x = ctr.x - exc + eys;
                                    corners[3].y = ctr.y - eyc - exs;

                                    let mut p_near_elbow: Option<Coord2D> = None; // find the closest corner to the rallyPoint same end as door
                                    let mut p_far_elbow: Option<Coord2D> = None; // find the closest corner to the rallypoint away from door
                                    let mut corner_to_rp_delta = Coord3D::default();
                                    let mut corner_to_exit_delta = Coord3D::default();
                                    corner_to_rp_delta.z = 0.0;
                                    corner_to_exit_delta.z = 0.0;
                                    let mut elbow_distance_near: f32 = 99999.9;
                                    let mut elbow_distance_far: f32 = 99999.9;

                                    for corner_index in 0..4usize {
                                        let near_candidate = corners[corner_index]; // for quicker array access
                                        corner_to_exit_delta.x = exit_point.x - near_candidate.x;
                                        corner_to_exit_delta.y = exit_point.y - near_candidate.y;
                                        corner_to_exit_delta.normalize();

                                        dot = corner_to_exit_delta.x * way_out_point.x + corner_to_exit_delta.y * way_out_point.y;
                                        if dot < 0.0 {
                                            corner_to_rp_delta.x = rally_point.x - near_candidate.x;
                                            corner_to_rp_delta.y = rally_point.y - near_candidate.y;
                                            if corner_to_rp_delta.length() < elbow_distance_near {
                                                elbow_distance_near = corner_to_rp_delta.length();
                                                p_near_elbow = Some(near_candidate);
                                            }
                                        } else {
                                            corner_to_rp_delta.x = rally_point.x - near_candidate.x;
                                            corner_to_rp_delta.y = rally_point.y - near_candidate.y;
                                            if corner_to_rp_delta.length() < elbow_distance_far {
                                                elbow_distance_far = corner_to_rp_delta.length();
                                                p_far_elbow = Some(near_candidate);
                                            }
                                        }
                                    }

                                    if let Some(near) = p_near_elbow {
                                        // did we find a nearest corner?
                                        out.push(WaypointDrawCommand::Node(Vector3::new(near.x, near.y, ctr.z))); // The little hockey puck
                                        points[num_points].set(near.x, near.y, ctr.z);
                                        num_points += 1;

                                        // and for that matter did we find a far side coner?
                                        if let Some(far) = p_far_elbow {
                                            // but let's test the dot of the first elbow against the rally point to find
                                            // out whethet the rally point wraps around this one, too
                                            let mut first_elbow_delta = Coord3D::default();
                                            first_elbow_delta.x = natural_rally_point.x - near.x;
                                            first_elbow_delta.y = natural_rally_point.y - near.y;
                                            first_elbow_delta.z = 0.0;
                                            first_elbow_delta.normalize();

                                            let mut first_to_rp_delta = Coord3D::default();
                                            first_to_rp_delta.x = near.x - rally_point.x;
                                            first_to_rp_delta.y = near.y - rally_point.y;
                                            first_to_rp_delta.z = 0.0;
                                            first_to_rp_delta.normalize();

                                            dot = first_to_rp_delta.x * first_elbow_delta.x + first_to_rp_delta.y * first_elbow_delta.y;
                                            if dot < 0.0 {
                                                // we have a second elbow
                                                out.push(WaypointDrawCommand::Node(Vector3::new(far.x, far.y, ctr.z))); // The little hockey puck
                                                points[num_points].set(far.x, far.y, ctr.z);
                                                num_points += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Finally draw the line out to the RallyPoint
                        points[num_points].set(rally_point.x, rally_point.y, rally_point.z);
                        num_points += 1;
                    } else {
                        continue;
                    }

                    out.push(WaypointDrawCommand::Node(Vector3::new(natural_rally_point.x, natural_rally_point.y, natural_rally_point.z))); // The little hockey puck

                    out.push(self.line(&points[..num_points]));
                } // end if exit interface
            }
        }
        out
    }
}
