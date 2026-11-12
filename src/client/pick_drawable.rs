// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/View.h (enum PickType)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DView.cpp (W3DView::pickDrawable, lines 2207-2259)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DScene.cpp (RTS3DScene::castRay, line 321)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp (setModelState's collision
//     types, lines 3088-3127; W3DModelDraw::setSelectable, line 3196)
//   GeneralsMD/Code/GameEngine/Source/GameClient/SelectionInfo.cpp (getPickTypesForContext, getPickTypesForCurrentSelection)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/rendobj.h (Get_Collision_Type, Set_Collision_Type, DEFAULT_BITS)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/coltype.h (COLL_TYPE_*)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/coltest.h (RayCollisionTestClass)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hlod.cpp (HLodClass::Cast_Ray)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/boxrobj.cpp (AABoxRenderObjClass::Cast_Ray, OBBoxRenderObjClass::Cast_Ray)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/lineseg.h (LineSegClass), castres.h (CastResultStruct)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmathline.cpp (CollisionMath::Collide of a line segment with an
//     AABoxClass and an OBBoxClass, Test_Aligned_Box)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   (Westwood Studios, WW3D2 and WWMath)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #148: `W3DView::pickDrawable`, the drawable under a pixel, found by casting the view's pick ray at the scene's
//! render objects.
//!
//! # As upstream does it
//!
//! - **`pickDrawable(screen, forceAttack, pickType)`** (W3DView.cpp:2207): nothing when a window is under the pixel
//!   that is not see-through, or has a parent that is not ([`window_blocks_pick`]); else the pick ray (`getPickRay`,
//!   the camera through the pixel to the far clip plane, `crate::client::tactical_view`) as a line segment, a
//!   `RayCollisionTestClass` of `COLL_TYPE_ALL` given to `W3DDisplay::m_3DScene->castRay(raytest, false, pickType)`, and
//!   the drawable of the render object hit (its user data, `DrawableInfo::m_drawable`; the terrain has none).
//!   `forceAttack` only asks for the contact point (`ComputeContactPoint`) of the caller's result, which `castRay` does
//!   not pass to the objects (it tests them with a result of its own): it changes nothing that is picked.
//! - **`RTS3DScene::castRay(raytest, testAll, collisionType)`** (W3DScene.cpp:321, [`cast_ray`]): every render object of
//!   the scene, in its list's order, whose collision type has a bit of `collisionType` and that is really visible (or
//!   all of them with `testAll`): a ray-sphere test on its bounding sphere (Graphics Gems I, p. 388) against the **line**
//!   (not the segment), then the object's own `Cast_Ray` with `CollisionType = COLL_TYPE_ALL` and `CheckTranslucent`.
//!   A hit makes the object the collided one and **shortens the ray to the hit** (its fraction reset to 1), so a later
//!   object hits only if it is nearer: the nearest object along the ray wins, ties to the first.
//! - **`HLodClass::Cast_Ray`** (hlod.cpp:2686): each model of the top LOD, then each additional model, all of them
//!   (`res |=`). A model is a mesh (its polygons) or a box (`AABoxRenderObjClass`, `OBBoxRenderObjClass`:
//!   `CollisionMath::Collide` of the segment with the box, colmathline.cpp:279 and 313, [`collide_aabox`],
//!   [`collide_obbox`]). A box the ray **starts inside** sets `StartBad` and counts as a hit at fraction 1; every later
//!   object then refuses the test (`if (raytest.Result->StartBad) return false;`).
//! - **The collision types** (rendobj.h:484): a render object's are the low byte of its bits, `COLL_TYPE_ALL` always on
//!   (`Set_Collision_Type`, [`set_collision_type`]); `DEFAULT_BITS` has only `COLL_TYPE_ALL`. `W3DModelDraw::setModelState`
//!   gives a drawable's render object `PICK_TYPE_SELECTABLE`, `SHRUBBERY`, `MINES` or `FORCEATTACKABLE` by its template's
//!   kinds (each **replacing** the one before, in that order), none for `KINDOF_CLICK_THROUGH`, and none for a dead
//!   object or a structure's rubble (not a bridge) ([`model_draw_collision_type`]); `setSelectable` turns the selectable
//!   bit on or off ([`set_selectable`]).
//! - **The pick types asked for** (SelectionInfo.cpp:252, [`get_pick_types_for_context`]): `PICK_TYPE_SELECTABLE`, with
//!   `FORCEATTACKABLE` in force-attack mode; the pending GUI command's mine and shrubbery options, or else the current
//!   selection's (`getPickTypesForCurrentSelection`: shrubbery when a controllable selected object has a flame weapon,
//!   in force-attack mode).
//!
//! # What the client has, and the fallback (where this differs from upstream)
//!
//! The scene's render objects are the client's: the scene list here ([`SceneRenderObj`]) is made by the client from what
//! it has. **Each model is a box**: a loaded W3D model's sub-object meshes are cast at as the oriented box of their
//! vertices in their bone's frame (`MeshClass::Cast_Ray` casts at the mesh's **polygons**, through its AABTree or by
//! brute force: not translated, so a ray through a box's corner a mesh does not fill picks it here), and its bounding
//! sphere is the sphere around those boxes (upstream's `Get_Bounding_Sphere` is the HLod's, from its meshes' own
//! spheres). Without the art (the demo), the object's logic geometry (its box, or its cylinder or sphere as the box
//! around it) is the one box. Meshes' translucency (`CheckTranslucent`: a mesh with alpha is skipped), the animation's
//! hidden sub-objects and the camera's visibility pass (`Is_Really_Visible`'s `IS_VISIBLE`, set by
//! `RTS3DScene::Visibility_Check`) are the client's to give in [`SceneRenderObj::really_visible`] and
//! [`BoxRenderObj::animation_hidden`].

use crate::logic::modules::plant;
use crate::ww::vector3::Vector3;

/// coltype.h's `Collision_Type`.
pub mod coll_type {
    /// `COLL_TYPE_ALL`: "perform this test against *EVERYTHING*", always on in a render object's collision type.
    pub const ALL: i32 = 0x01;
    pub const T0: i32 = 0x02;
    pub const T1: i32 = 0x04;
    pub const T2: i32 = 0x08;
    pub const T3: i32 = 0x10;
    pub const T4: i32 = 0x20;
    pub const T5: i32 = 0x40;
    pub const T6: i32 = 0x80;
}

/// `RenderObjClass::COLL_TYPE_MASK` (rendobj.h:528).
pub const COLL_TYPE_MASK: i32 = 0x0000_00FF;

/// `enum PickType` (View.h:56).
pub mod pick_type {
    use super::coll_type;
    pub const TERRAIN: i32 = coll_type::T0;
    pub const SELECTABLE: i32 = coll_type::T1;
    pub const SHRUBBERY: i32 = coll_type::T2;
    /// "mines aren't normally selectable, but workers/dozers need to"
    pub const MINES: i32 = coll_type::T3;
    pub const FORCEATTACKABLE: i32 = coll_type::T4;
    pub const ALL_DRAWABLES: i32 = SELECTABLE | SHRUBBERY | MINES | FORCEATTACKABLE;
}

/// `RenderObjClass::Set_Collision_Type(type)` on a render object's collision bits (`Bits & COLL_TYPE_MASK`): `Bits &=
/// ~COLL_TYPE_MASK; Bits |= (type & COLL_TYPE_MASK) | COLL_TYPE_ALL;`.
pub fn set_collision_type(_bits: i32, t: i32) -> i32 {
    if plant(9398) {
        return t & COLL_TYPE_MASK;
    }
    (t & COLL_TYPE_MASK) | coll_type::ALL
}

/// The template's kinds `W3DModelDraw::setModelState` reads for the collision types.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CollisionKinds {
    pub selectable: bool,
    pub shrubbery: bool,
    pub mine: bool,
    pub forceattackable: bool,
    pub click_through: bool,
}

/// What `setModelState` reads of the drawable's object (`draw->getObject()`), when it has one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CollisionObject {
    /// `obj->isKindOf(KINDOF_BRIDGE)`, `isKindOf(KINDOF_BRIDGE_TOWER)`, `isKindOf(KINDOF_STRUCTURE)`.
    pub bridge: bool,
    pub bridge_tower: bool,
    pub structure: bool,
    /// `draw->getModelConditionFlags().test(MODELCONDITION_RUBBLE)`.
    pub rubble: bool,
    /// `obj->isEffectivelyDead()`.
    pub effectively_dead: bool,
}

/// `W3DModelDraw::setModelState`'s collision types for a new render object (W3DModelDraw.cpp:3088): from the render
/// object's collision bits `bits` (a new one's are `DEFAULT_BITS`', [`coll_type::ALL`]), the template's kinds and the
/// object.
pub fn model_draw_collision_type(mut bits: i32, tmplate: &CollisionKinds, obj: Option<&CollisionObject>) -> i32 {
    // "set collision type for render object.  Used by WW3D2 collision code."
    if tmplate.selectable {
        bits = set_collision_type(bits, pick_type::SELECTABLE);
    }
    if tmplate.shrubbery {
        bits = set_collision_type(bits, pick_type::SHRUBBERY);
    }
    if tmplate.mine {
        bits = set_collision_type(bits, pick_type::MINES);
    }
    if tmplate.forceattackable {
        bits = set_collision_type(bits, pick_type::FORCEATTACKABLE);
    }
    if tmplate.click_through {
        bits = set_collision_type(bits, 0);
    }
    if let Some(obj) = obj {
        // "for non bridge objects we adjust some collision types"
        if !obj.bridge && !obj.bridge_tower {
            if obj.structure && obj.rubble {
                // "A dead building, -- don't allow the user to click on rubble! Treat it as a location instead."
                bits = set_collision_type(bits, 0);
            } else if obj.effectively_dead && !plant(9399) {
                // "A dead object, -- don't allow the user to click on rubble/hulks! Treat it as a location instead."
                bits = set_collision_type(bits, 0);
            }
        }
    }
    bits
}

/// `W3DModelDraw::setSelectable(selectable)` (W3DModelDraw.cpp:3196) on the render object's collision bits.
pub fn set_selectable(bits: i32, selectable: bool) -> i32 {
    let mut current = bits;
    if selectable {
        current |= pick_type::SELECTABLE;
    } else if plant(9406) {
        current = 0;
    } else {
        current &= !pick_type::SELECTABLE;
    }
    set_collision_type(bits, current)
}

/// `CommandButton` options `getPickTypesForContext` reads (ControlBar.h): `ALLOW_SHRUBBERY_TARGET`, `ALLOW_MINE_TARGET`.
pub const ALLOW_SHRUBBERY_TARGET: u32 = 0x0000_0010;
pub const ALLOW_MINE_TARGET: u32 = 0x0000_0800;

/// `getPickTypesForCurrentSelection(forceAttackMode)` (SelectionInfo.cpp:281): nothing unless the selection is
/// controllable (`TheInGameUI->areSelectedObjectsControllable()`); then `PICK_TYPE_SHRUBBERY` once a selected drawable's
/// object (`None`: a drawable without one, skipped) has a weapon dealing `DAMAGE_FLAME`, in force-attack mode.
pub fn get_pick_types_for_current_selection(force_attack_mode: bool, controllable: bool, selected_has_flame_weapon: impl IntoIterator<Item = Option<bool>>) -> u32 {
    let mut ret_val = 0u32;
    if !controllable {
        return ret_val;
    }
    for has_flame in selected_has_flame_weapon {
        let Some(has_flame) = has_flame else { continue };
        if has_flame && (force_attack_mode || plant(9407)) {
            ret_val |= pick_type::SHRUBBERY as u32;
        }
        // "For efficiency."
        if ret_val & (pick_type::MINES | pick_type::SHRUBBERY) as u32 != 0 {
            break;
        }
    }
    ret_val
}

/// `getPickTypesForContext(forceAttackMode)` (SelectionInfo.cpp:252): `gui_command_options` is the pending GUI command's
/// `getOptions()` (`None`: no command), `current_selection` the selection's types
/// ([`get_pick_types_for_current_selection`]), asked only without a command.
pub fn get_pick_types_for_context(force_attack_mode: bool, gui_command_options: Option<u32>, current_selection: impl FnOnce(bool) -> u32) -> u32 {
    let mut types = pick_type::SELECTABLE as u32;
    if force_attack_mode && !plant(9400) {
        types |= pick_type::FORCEATTACKABLE as u32;
    }
    // "if we have a gui context command that allows for a shrubbery target then we want to pick that type too"
    match gui_command_options {
        Some(options) => {
            if options & ALLOW_MINE_TARGET != 0 {
                types |= pick_type::MINES as u32;
            }
            if options & ALLOW_SHRUBBERY_TARGET != 0 {
                types |= pick_type::SHRUBBERY as u32;
            }
        }
        None => types |= current_selection(force_attack_mode),
    }
    types
}

/// `class LineSegClass` (lineseg.h): a segment from `p0` to `p1`, with `dp = p1 - p0`, its normalized direction and its
/// length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineSeg {
    pub p0: Vector3,
    pub p1: Vector3,
    pub dp: Vector3,
    pub dir: Vector3,
    pub length: f32,
}

impl LineSeg {
    /// `LineSegClass(p0, p1)`.
    pub fn new(p0: Vector3, p1: Vector3) -> Self {
        let mut l = LineSeg { p0, p1, dp: Vector3::new(0.0, 0.0, 0.0), dir: Vector3::new(0.0, 0.0, 0.0), length: 0.0 };
        l.recalculate();
        l
    }

    /// `Set(p0, p1)`.
    pub fn set(&mut self, p0: Vector3, p1: Vector3) {
        self.p0 = p0;
        self.p1 = p1;
        self.recalculate();
    }

    /// `recalculate()`: `DP = P1 - P0; Dir = DP; Dir.Normalize(); Length = DP.Length();`.
    fn recalculate(&mut self) {
        self.dp = self.p1 - self.p0;
        self.dir = self.dp;
        self.dir.normalize();
        self.length = self.dp.length();
    }

    /// `Compute_Point(t, &set)`: `Vector3::Add(P0, t * DP, set)`.
    pub fn compute_point(&self, t: f32) -> Vector3 {
        Vector3::add_vectors(&self.p0, &(t * self.dp))
    }
}

/// `struct CastResultStruct` (castres.h), as `Reset()` leaves it ([`Default`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CastResult {
    /// "was the inital configuration interpenetrating something?"
    pub start_bad: bool,
    /// "fraction of the move up until collision"
    pub fraction: f32,
    pub normal: Vector3,
    pub surface_type: u32,
    pub compute_contact_point: bool,
    pub contact_point: Vector3,
}

impl Default for CastResult {
    fn default() -> Self {
        CastResult { start_bad: false, fraction: 1.0, normal: Vector3::new(0.0, 0.0, 0.0), surface_type: 0, compute_contact_point: false, contact_point: Vector3::new(0.0, 0.0, 0.0) }
    }
}

/// `struct BoxTestStruct` (colmathline.cpp): the box (`Min`, `Max`) and the segment in the box's frame.
#[derive(Clone, Copy, Debug)]
struct BoxTest {
    min: Vector3,
    max: Vector3,
    p0: Vector3,
    dp: Vector3,
    fraction: f32,
    inside: bool,
    axis: usize,
    side: i32,
}

/// `enum BoxSideType`.
const BOX_SIDE_NEGATIVE: i32 = 0;
const BOX_SIDE_POSITIVE: i32 = 1;
const BOX_SIDE_MIDDLE: i32 = 2;

/// `_box_normal[AXIS][SIDE]`.
fn box_normal(axis: usize, side: i32) -> Vector3 {
    let mut n = Vector3::new(0.0, 0.0, 0.0);
    n[axis] = if side == BOX_SIDE_NEGATIVE { -1.0 } else { 1.0 };
    n
}

/// `Test_Aligned_Box(test)` (colmathline.cpp): the slab test of the segment against the axis-aligned box (the fraction
/// is not bounded by 1: the callers compare it with the result's).
fn test_aligned_box(test: &mut BoxTest) -> bool {
    // "candidate intersection plane distance for each axis"
    let mut candidateplane = [0.0f32; 3];
    // "t value along the ray for each axis"
    let mut maxt = [0.0f32; 3];
    let mut coord = [0.0f32; 3];
    let mut quadrant = [0i32; 3];
    let mut inside = true;
    for i in 0..3 {
        if test.p0[i] < test.min[i] {
            quadrant[i] = BOX_SIDE_NEGATIVE;
            candidateplane[i] = test.min[i];
            inside = false;
        } else if test.p0[i] > test.max[i] {
            quadrant[i] = BOX_SIDE_POSITIVE;
            candidateplane[i] = test.max[i];
            inside = false;
        } else {
            quadrant[i] = BOX_SIDE_MIDDLE;
        }
    }
    // "Ray started out inside the box..."
    if inside {
        test.fraction = 0.0;
        test.inside = true;
        return true;
    }
    // "Calculate the 3 distances to the candidate planes"
    for i in 0..3 {
        if quadrant[i] != BOX_SIDE_MIDDLE && test.dp[i] != 0.0f32 {
            maxt[i] = (candidateplane[i] - test.p0[i]) / test.dp[i];
        } else {
            maxt[i] = -1.0;
        }
    }
    // "Get the largest of the maxt's for the final choice of intersection"
    let mut intersection_plane = 0;
    for i in 1..3 {
        let larger = if plant(9394) { maxt[i] < maxt[intersection_plane] } else { maxt[i] > maxt[intersection_plane] };
        if larger {
            intersection_plane = i;
        }
    }
    // "If the ray is "in front" of all of the planes, return"
    if maxt[intersection_plane] < 0.0f32 {
        return false;
    }
    // "Check if the ray is inside the box, i.e. on the two planes which are not the intersection planes, the
    // intersection point should be between the min and max of the box."
    for i in 0..3 {
        if intersection_plane != i {
            coord[i] = test.p0[i] + maxt[intersection_plane] * test.dp[i];
            if coord[i] < test.min[i] || coord[i] > test.max[i] {
                return false; // "ray is outside the box"
            }
        } else {
            coord[i] = candidateplane[i];
        }
    }
    // "Fill in the intersection values"
    test.fraction = maxt[intersection_plane];
    test.inside = false;
    test.axis = intersection_plane;
    test.side = quadrant[intersection_plane];
    true
}

/// `CollisionMath::Collide(line, AABoxClass box, res)` (colmathline.cpp:279): the box by its centre and extent.
pub fn collide_aabox(line: &LineSeg, center: &Vector3, extent: &Vector3, res: &mut CastResult) -> bool {
    let mut test = BoxTest { min: *center - *extent, max: *center + *extent, p0: line.p0, dp: line.dp, fraction: 0.0, inside: false, axis: 0, side: 0 };
    // "check ray against the box, exit if the ray totally missed the box,"
    if !test_aligned_box(&mut test) {
        return false;
    }
    // "if ray starts inside the box, note that fact and bail."
    if test.inside {
        res.start_bad = true;
        return true;
    }
    // "Now, if this intersection is before any current intersection that we've found, install our intersection"
    if test.fraction < res.fraction {
        res.fraction = test.fraction;
        res.normal = box_normal(test.axis, test.side);
        if res.compute_contact_point {
            res.contact_point = line.p0 + res.fraction * line.dp;
        }
        return true;
    }
    false
}

/// `Matrix3x3::Transpose() * v` (matrix3.h): row `i` of the transpose (column `i` of `basis`) dotted with `v`, summed
/// left to right.
fn transpose_times(basis: &[[f32; 3]; 3], v: &Vector3) -> Vector3 {
    if plant(9395) {
        return Vector3::new(basis[0][0] * v.x + basis[0][1] * v.y + basis[0][2] * v.z, basis[1][0] * v.x + basis[1][1] * v.y + basis[1][2] * v.z, basis[2][0] * v.x + basis[2][1] * v.y + basis[2][2] * v.z);
    }
    Vector3::new(basis[0][0] * v.x + basis[1][0] * v.y + basis[2][0] * v.z, basis[0][1] * v.x + basis[1][1] * v.y + basis[2][1] * v.z, basis[0][2] * v.x + basis[1][2] * v.y + basis[2][2] * v.z)
}

/// `CollisionMath::Collide(line, OBBoxClass box, result)` (colmathline.cpp:313): the box by its centre, extent and basis
/// (`Basis[row][column]`, the box's axes as its columns); the segment taken into the box's frame about its centre.
pub fn collide_obbox(line: &LineSeg, center: &Vector3, extent: &Vector3, basis: &[[f32; 3]; 3], result: &mut CastResult) -> bool {
    let mut test = BoxTest {
        min: *center - *extent,
        max: *center + *extent,
        p0: transpose_times(basis, &(line.p0 - *center)) + *center,
        dp: transpose_times(basis, &line.dp),
        fraction: 0.0,
        inside: false,
        axis: 0,
        side: 0,
    };
    if !test_aligned_box(&mut test) {
        return false;
    }
    // "if ray starts inside the box, don't collide"
    if test.inside {
        result.start_bad = true;
        return true;
    }
    if test.fraction < result.fraction {
        result.fraction = test.fraction;
        let a = test.axis;
        result.normal.set(basis[0][a], basis[1][a], basis[2][a]);
        if test.side == BOX_SIDE_NEGATIVE {
            result.normal = -result.normal;
        }
        if result.compute_contact_point {
            result.contact_point = line.p0 + result.fraction * line.dp;
        }
        return true;
    }
    false
}

/// A box model: `AABoxRenderObjClass` (`basis: None`) or `OBBoxRenderObjClass`, by its world box (`CachedBox`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxRenderObj {
    /// `Get_Collision_Type()`.
    pub collision_type: i32,
    /// `Is_Animation_Hidden()`.
    pub animation_hidden: bool,
    pub center: Vector3,
    pub extent: Vector3,
    /// The oriented box's `Basis[row][column]`; `None`: axis-aligned.
    pub basis: Option<[[f32; 3]; 3]>,
}

/// `class RayCollisionTestClass` (coltest.h): the segment, the result (`Result` points at it), the collision type asked
/// for, the two checks, and the render object hit (an index into the list cast at).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayTest {
    pub ray: LineSeg,
    pub result: CastResult,
    pub collision_type: i32,
    pub check_translucent: bool,
    pub check_hidden: bool,
    pub collided: Option<usize>,
}

impl RayTest {
    /// `RayCollisionTestClass(ray, res, collision_type, check_translucent, check_hidden)`.
    pub fn new(ray: LineSeg, result: CastResult, collision_type: i32, check_translucent: bool, check_hidden: bool) -> Self {
        RayTest { ray, result, collision_type, check_translucent, check_hidden, collided: None }
    }
}

impl BoxRenderObj {
    /// `AABoxRenderObjClass::Cast_Ray` / `OBBoxRenderObjClass::Cast_Ray` (boxrobj.cpp:805, 1188). Whether it hit; the
    /// caller notes the object hit.
    pub fn cast_ray(&self, raytest: &mut RayTest) -> bool {
        if self.collision_type & raytest.collision_type == 0 {
            return false;
        }
        if self.animation_hidden {
            return false;
        }
        if raytest.result.start_bad && !plant(9396) {
            return false;
        }
        match &self.basis {
            None => collide_aabox(&raytest.ray, &self.center, &self.extent, &mut raytest.result),
            Some(b) => collide_obbox(&raytest.ray, &self.center, &self.extent, b, &mut raytest.result),
        }
    }
}

/// One render object of the scene (`RTS3DScene`'s `RenderList`): a drawable's model (an `HLodClass`).
#[derive(Clone, Debug, PartialEq)]
pub struct SceneRenderObj<T> {
    /// `Get_User_Data()`'s `DrawableInfo::m_drawable` (`None`: no drawable, as the terrain's).
    pub drawable: Option<T>,
    /// `Get_Collision_Type()`.
    pub collision_type: i32,
    /// `Is_Really_Visible()`: visible to the camera, not hidden, not hidden by the animation.
    pub really_visible: bool,
    /// `Get_Bounding_Sphere()`: its centre and radius.
    pub sphere_center: Vector3,
    pub sphere_radius: f32,
    /// The top LOD's models and the additional models, in order.
    pub models: Vec<BoxRenderObj>,
}

impl<T> SceneRenderObj<T> {
    /// `HLodClass::Cast_Ray(raytest)` (hlod.cpp:2686): each model, all of them.
    pub fn cast_ray(&self, raytest: &mut RayTest) -> bool {
        let mut res = false;
        for m in &self.models {
            res |= m.cast_ray(raytest);
        }
        res
    }
}

/// `RTS3DScene::castRay(raytest, testAll, collisionType)` (W3DScene.cpp:321): the nearest render object of `list` along
/// the ray, in `raytest.collided`; `raytest.ray` is left clipped to end on it. Whether one was hit.
pub fn cast_ray<T>(list: &[SceneRenderObj<T>], raytest: &mut RayTest, test_all: bool, collision_type: i32) -> bool {
    // "temporary results for each object tested"
    let mut temp_ray_test = RayTest::new(raytest.ray, CastResult::default(), coll_type::T0, false, false);
    let mut hit = false;
    temp_ray_test.collision_type = coll_type::ALL;
    // "check if a mesh is translucent before colliding with it. Skips headlights, etc."
    temp_ray_test.check_translucent = true;
    for (i, robj) in list.iter().enumerate() {
        // "only intersect if it was visible or if we must test all"
        if robj.collision_type & collision_type != 0 && (test_all || robj.really_visible || plant(9391)) {
            // "Do a quick ray-sphere test (Graphics Gems I,  p388)"
            let sphere_vector = robj.sphere_center - temp_ray_test.ray.p0;
            let alpha = Vector3::dot_product(&sphere_vector, &temp_ray_test.ray.dir);
            let beta = robj.sphere_radius * robj.sphere_radius - (Vector3::dot_product(&sphere_vector, &sphere_vector) - alpha * alpha);
            if beta < 0.0f32 && !plant(9393) {
                continue; // "no intersection"
            }
            // "Do a more accurate test against object geometry"
            if robj.cast_ray(&mut temp_ray_test) {
                // "Found an object closer than last closest object / Adjust our results and refine search"
                raytest.collided = Some(i);
                hit = true;
                if !plant(9392) {
                    // "Refine search by making ray shorter"
                    let new_end_point = temp_ray_test.ray.compute_point(temp_ray_test.result.fraction);
                    temp_ray_test.ray.set(raytest.ray.p0, new_end_point);
                    // "intersection point is at the end of shortened ray, so adjust fraction to 1.0"
                    temp_ray_test.result.fraction = 1.0;
                }
            }
        }
    }
    // "Store results of ray intersection test including a clipped ray that ends on object."
    raytest.ray = temp_ray_test.ray;
    hit
}

/// `pickDrawable`'s window test: `TheWindowManager->getWindowUnderCursor(x, y)` and its parents, each window's
/// `WIN_STATUS_SEE_THRU` (`see_thru`, from the window under the pixel up to the root): "check to see if it or any of its
/// parents are opaque.  If so, we can't select anything."
pub fn window_blocks_pick(see_thru: impl IntoIterator<Item = bool>) -> bool {
    let mut chain = see_thru.into_iter();
    if plant(9397) {
        return chain.next().is_some_and(|s| !s);
    }
    chain.any(|s| !s)
}

/// `W3DView::pickDrawable(screen, forceAttack, pickType)` (W3DView.cpp:2207) once `getPickRay` has given the ray from
/// `ray_start` to `ray_end`: nothing when a window blocks the pick ([`window_blocks_pick`]); else the drawable of the render
/// object `castRay(raytest, false, pickType)` hits. `force_attack` asks for the contact point of a result `castRay` does
/// not use (see the module docs).
pub fn pick_drawable_on_ray<T: Copy>(windows_see_thru: impl IntoIterator<Item = bool>, ray_start: Vector3, ray_end: Vector3, force_attack: bool, pick_type: i32, list: &[SceneRenderObj<T>]) -> Option<T> {
    // "don't pick a drawable if there is a window under the cursor"
    if window_blocks_pick(windows_see_thru) {
        return None;
    }
    let lineseg = LineSeg::new(ray_start, ray_end);
    let mut result = CastResult::default();
    if force_attack {
        result.compute_contact_point = true;
    }
    // "Don't check against translucent or hidden objects"
    let mut raytest = RayTest::new(lineseg, result, coll_type::ALL, false, false);
    let render_obj = if cast_ray(list, &mut raytest, false, pick_type) { raytest.collided } else { None };
    // ("the terrain, therefore the userdata is NULL")
    render_obj.and_then(|i| list[i].drawable)
}

/// [`pick_drawable_on_ray`] on the view's pick ray through `screen` (`getPickRay`).
pub fn pick_drawable<T: Copy>(
    view: &crate::client::tactical_view::TacticalView,
    screen: crate::common::base_type::ICoord2D,
    windows_see_thru: impl IntoIterator<Item = bool>,
    force_attack: bool,
    pick_type: i32,
    list: &[SceneRenderObj<T>],
) -> Option<T> {
    if window_blocks_pick(windows_see_thru) {
        return None;
    }
    let (ray_start, ray_end) = view.get_pick_ray(screen);
    pick_drawable_on_ray(std::iter::empty(), ray_start, ray_end, force_attack, pick_type, list)
}

/// The client's box of a set of points in a frame (new: what the client casts at for a mesh, see the module docs): the
/// points' axis-aligned bounds in the frame `basis` (`Basis[row][column]`, the frame's axes as columns) at `origin`, as a
/// world [`BoxRenderObj`] (its centre taken back to the world).
pub fn oriented_bounds(points: impl IntoIterator<Item = Vector3>, basis: [[f32; 3]; 3], origin: Vector3, collision_type: i32) -> Option<BoxRenderObj> {
    let (mut lo, mut hi) = (Vector3::new(f32::MAX, f32::MAX, f32::MAX), Vector3::new(f32::MIN, f32::MIN, f32::MIN));
    let mut any = false;
    for p in points {
        any = true;
        lo.update_min(&p);
        hi.update_max(&p);
    }
    if !any {
        return None;
    }
    let c = 0.5f32 * (lo + hi);
    let extent = 0.5f32 * (hi - lo);
    let center = Vector3::new(
        origin.x + basis[0][0] * c.x + basis[0][1] * c.y + basis[0][2] * c.z,
        origin.y + basis[1][0] * c.x + basis[1][1] * c.y + basis[1][2] * c.z,
        origin.z + basis[2][0] * c.x + basis[2][1] * c.y + basis[2][2] * c.z,
    );
    Some(BoxRenderObj { collision_type, animation_hidden: false, center, extent, basis: Some(basis) })
}

/// The client's bounding sphere of boxes (new, see the module docs): the centre of the boxes' corners' bounds, and the
/// distance to the farthest corner.
pub fn bounding_sphere(models: &[BoxRenderObj]) -> (Vector3, f32) {
    let corners = |b: &BoxRenderObj| -> Vec<Vector3> {
        let basis = b.basis.unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        (0..8)
            .map(|k| {
                let s = |bit: usize, e: f32| if k & (1 << bit) != 0 { e } else { -e };
                let (x, y, z) = (s(0, b.extent.x), s(1, b.extent.y), s(2, b.extent.z));
                Vector3::new(
                    b.center.x + basis[0][0] * x + basis[0][1] * y + basis[0][2] * z,
                    b.center.y + basis[1][0] * x + basis[1][1] * y + basis[1][2] * z,
                    b.center.z + basis[2][0] * x + basis[2][1] * y + basis[2][2] * z,
                )
            })
            .collect()
    };
    let (mut lo, mut hi) = (Vector3::new(f32::MAX, f32::MAX, f32::MAX), Vector3::new(f32::MIN, f32::MIN, f32::MIN));
    for b in models {
        for p in corners(b) {
            lo.update_min(&p);
            hi.update_max(&p);
        }
    }
    if models.is_empty() {
        return (Vector3::new(0.0, 0.0, 0.0), 0.0);
    }
    let center = 0.5f32 * (lo + hi);
    let mut r2 = 0.0f32;
    for b in models {
        for p in corners(b) {
            let d = (p - center).length2();
            if d > r2 {
                r2 = d;
            }
        }
    }
    (center, crate::ww::wwmath::sqrt(r2))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(id: i32, x: f32, collision_type: i32) -> SceneRenderObj<i32> {
        let models = vec![BoxRenderObj { collision_type: coll_type::ALL, animation_hidden: false, center: Vector3::new(x, 0.0, 0.0), extent: Vector3::new(1.0, 1.0, 1.0), basis: None }];
        let (c, r) = bounding_sphere(&models);
        SceneRenderObj { drawable: Some(id), collision_type, really_visible: true, sphere_center: c, sphere_radius: r, models }
    }

    /// #148, predicted before the run: along +x from x 0 to 100, two selectable boxes at x 10 and x 20 (in either order)
    /// pick the one at 10, the nearer; asked for shrubbery only, nothing; a box at 50 not selectable is skipped and one
    /// that is is picked once the nearer two are gone; an opaque window (or an opaque parent) blocks the pick; a ray
    /// starting inside the first box is a hit on it (`StartBad`) and every later one refuses; the collision types of the
    /// model draw: a selectable, dead unit has none (`COLL_TYPE_ALL` only), a shrubbery's replaces selectable, and
    /// `setSelectable(false)` clears the bit.
    #[test]
    fn the_pick_is_the_nearest_box_along_the_ray() {
        let sel = set_collision_type(coll_type::ALL, pick_type::SELECTABLE);
        let (a, b) = (Vector3::new(0.0, 0.0, 0.0), Vector3::new(100.0, 0.0, 0.0));
        for list in [vec![obj(1, 10.0, sel), obj(2, 20.0, sel)], vec![obj(2, 20.0, sel), obj(1, 10.0, sel)]] {
            assert_eq!(pick_drawable_on_ray([], a, b, false, pick_type::SELECTABLE, &list), Some(1));
            assert_eq!(pick_drawable_on_ray([], a, b, false, pick_type::SHRUBBERY, &list), None);
            assert_eq!(pick_drawable_on_ray([true, false], a, b, false, pick_type::SELECTABLE, &list), None);
            assert_eq!(pick_drawable_on_ray([true, true], a, b, true, pick_type::SELECTABLE, &list), Some(1));
        }
        let list = vec![obj(3, 50.0, coll_type::ALL), obj(4, 60.0, sel)];
        assert_eq!(pick_drawable_on_ray([], a, b, false, pick_type::SELECTABLE, &list), Some(4));
        let list = vec![obj(5, 0.5, sel), obj(6, 20.0, sel)];
        assert_eq!(pick_drawable_on_ray([], a, b, false, pick_type::SELECTABLE, &list), Some(5));
        let unit = CollisionKinds { selectable: true, ..Default::default() };
        let dead = CollisionObject { effectively_dead: true, ..Default::default() };
        assert_eq!(model_draw_collision_type(coll_type::ALL, &unit, None), coll_type::ALL | pick_type::SELECTABLE);
        assert_eq!(model_draw_collision_type(coll_type::ALL, &unit, Some(&dead)), coll_type::ALL);
        let tree = CollisionKinds { selectable: true, shrubbery: true, ..Default::default() };
        assert_eq!(model_draw_collision_type(coll_type::ALL, &tree, None), coll_type::ALL | pick_type::SHRUBBERY);
        assert_eq!(set_selectable(sel, false), coll_type::ALL);
        assert_eq!(get_pick_types_for_context(true, None, |f| get_pick_types_for_current_selection(f, true, [Some(true)])), (pick_type::SELECTABLE | pick_type::FORCEATTACKABLE | pick_type::SHRUBBERY) as u32);
        assert_eq!(get_pick_types_for_context(false, Some(ALLOW_MINE_TARGET), |_| unreachable!()), (pick_type::SELECTABLE | pick_type::MINES) as u32);
    }
}
