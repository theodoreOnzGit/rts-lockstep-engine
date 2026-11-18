//! UNTESTED DRAFT (#202, #205, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DInGameUI.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DInGameUI.h
//!
//! `W3DInGameUI` is `InGameUI` (the port's `crate::client::in_game_ui::InGameUi`, whose state this reads through
//! [`W3DInGameUiHost`]) plus the render objects for the move hints and the building placement arrow/anchor.
//! Left out: `DebugHintObject` is `_DEBUG`-only (kept as [`debug_hint_vertices`]); `loadText` and the MOTD creation in
//! `init` are a block comment upstream (only `loadText` is live code, and nothing calls it); `DO_UNIT_TIMINGS` and
//! `EXTENDED_STATS` are build options of the original and off.

use super::device_support::RenderObjHandle;
use rts_lockstep_engine::common::base_type::{Coord3D, ICoord2D};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;

/// `MAX_MOVE_HINTS` (InGameUI.h:631).
pub const MAX_MOVE_HINTS: usize = 256;

/// `InGameUI::MoveHintStruct`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MoveHintStruct {
    /// World coords of destination point.
    pub pos: Coord3D,
    /// id of who will move to this point.
    pub source_id: u32,
    /// frame the command was issued on.
    pub frame: u32,
}

/// `PathfindLayerEnum` `LAYER_GROUND` is the one `drawMoveHints` tests.
pub const LAYER_GROUND: i32 = 1;

/// What `W3DInGameUI`'s draw reads of the game and writes to the scene.
pub trait W3DInGameUiHost {
    /// `TheGameClient->getFrame()`.
    fn client_frame(&self) -> u32;
    /// `TheGlobalData->m_moveHintName`.
    fn move_hint_name(&self) -> Vec<u8>;
    /// `W3DDisplay::m_assetManager->Create_Render_Obj(name)`: 0 when not found.
    fn create_render_obj(&mut self, name: &[u8]) -> RenderObjHandle;
    /// `Get_HAnim(name)`: 0 when not found. `release_hanim`: `REF_PTR_RELEASE`.
    fn get_hanim(&mut self, name: &[u8]) -> u32;
    fn release_hanim(&mut self, anim: u32);
    /// `Is_Hidden`, `Set_Hidden`, `Peek_Scene() != NULL`, `Add_Render_Object`, `Remove_Render_Object`,
    /// `Set_Animation(anim, 0, ANIM_MODE_ONCE)`, `Set_Transform`.
    fn is_hidden(&self, obj: RenderObjHandle) -> bool;
    fn set_hidden(&mut self, obj: RenderObjHandle, hidden: bool);
    fn in_scene(&self, obj: RenderObjHandle) -> bool;
    fn add_render_object(&mut self, obj: RenderObjHandle);
    fn remove_render_object(&mut self, obj: RenderObjHandle);
    fn set_animation_once(&mut self, obj: RenderObjHandle, anim: u32);
    fn set_transform(&mut self, obj: RenderObjHandle, m: &Matrix3D);
    /// `TheTerrainLogic->alignOnTerrain(0, pos, true, transform)`: the layer (`LAYER_GROUND`...) and the matrix.
    fn align_on_terrain(&mut self, pos: &Coord3D) -> (i32, Matrix3D);
    /// `TheTerrainLogic->isUnderwater(x, y, &waterZ)`: the water height when underwater.
    fn is_underwater(&mut self, x: f32, y: f32) -> Option<f32>;
    /// `makeAlignToNormalMatrix(0, pos, normal, transform)`.
    fn make_align_to_normal_matrix(&mut self, pos: &Coord3D, normal: &Coord3D) -> Matrix3D;
    /// `isPlacementAnchored()`, `getPlacementPoints(&start, &end)`.
    fn is_placement_anchored(&self) -> bool;
    fn placement_points(&self) -> (ICoord2D, ICoord2D);
    /// `m_placeIcon[0]->getTransformMatrix()` when `m_placeIcon[0]` exists.
    fn place_icon_transform(&self) -> Option<Matrix3D>;
    /// `TheDisplay->drawOpenRect(x, y, w, h, width, color)`.
    fn draw_open_rect(&mut self, x: i32, y: i32, w: i32, h: i32, width: f32, color: u32);
}

/// `class W3DInGameUI` (its own members; the `InGameUI` base is the host's).
#[derive(Clone, Debug)]
pub struct W3DInGameUI {
    /// `m_moveHintRenderObj[MAX_MOVE_HINTS]`, `m_moveHintAnim[MAX_MOVE_HINTS]` (0 for `NULL`).
    pub move_hint_render_obj: Vec<RenderObjHandle>,
    pub move_hint_anim: Vec<u32>,
    /// `m_buildingPlacementAnchor`, `m_buildingPlacementArrow`.
    pub building_placement_anchor: RenderObjHandle,
    pub building_placement_arrow: RenderObjHandle,
}

impl W3DInGameUI {
    /// `W3DInGameUI::W3DInGameUI` (W3DInGameUI.cpp:195).
    pub fn new() -> Self {
        W3DInGameUI {
            move_hint_render_obj: vec![RenderObjHandle(0); MAX_MOVE_HINTS],
            move_hint_anim: vec![0; MAX_MOVE_HINTS],
            building_placement_anchor: RenderObjHandle(0),
            building_placement_arrow: RenderObjHandle(0),
        }
    }

    /// `W3DInGameUI::~W3DInGameUI` (W3DInGameUI.cpp:213): the render objects and animations released.
    pub fn destroy(&mut self, host: &mut dyn W3DInGameUiHost) {
        // remove render objects for hints
        for i in 0..MAX_MOVE_HINTS {
            self.move_hint_render_obj[i] = RenderObjHandle(0);
            host.release_hanim(self.move_hint_anim[i]);
            self.move_hint_anim[i] = 0;
        }
        self.building_placement_anchor = RenderObjHandle(0);
        self.building_placement_arrow = RenderObjHandle(0);
    }

    /// `W3DInGameUI::init` (W3DInGameUI.cpp:290): extends `InGameUI::init()` (the host's); the MOTD creation is a
    /// block comment upstream.
    pub fn init(&mut self) {}

    /// `W3DInGameUI::update` (W3DInGameUI.cpp:322): calls the base.
    pub fn update(&mut self) {}

    /// `W3DInGameUI::reset` (W3DInGameUI.cpp:334): calls the base.
    pub fn reset(&mut self) {}

    /// `W3DInGameUI::draw` (W3DInGameUI.cpp:346). `preDraw()` / `postDraw()` / `winRepaint()` are the host's
    /// `InGameUI` and window manager (the caller runs them around this); `is_drag_selecting` and `drag_region`
    /// (`lo`, `hi`) are `m_isDragSelecting`, `m_dragSelectRegion`; one view is drawn (the game has one).
    pub fn draw(&mut self, host: &mut dyn W3DInGameUiHost, is_drag_selecting: bool, drag_region: (ICoord2D, ICoord2D)) {
        // preDraw(): the caller's
        // draw selection region if drag selecting
        if is_drag_selecting {
            self.draw_selection_region(host, drag_region);
        }

        // for each view draw hints
        // draw move hints
        self.draw_move_hints(host, &[]);
        // draw attack hints
        self.draw_attack_hints();
        // draw placement angle selection if needed
        self.draw_place_angle(host);

        // postDraw(); TheWindowManager->winRepaint(): the caller's
    }

    /// `W3DInGameUI::drawSelectionRegion` (W3DInGameUI.cpp:390): draw 2d selection region on screen.
    pub fn draw_selection_region(&mut self, host: &mut dyn W3DInGameUiHost, region: (ICoord2D, ICoord2D)) {
        let width = 2.0f32;
        let color: u32 = 0x9933FF33; //0xAARRGGBB
        let (lo, hi) = region;
        host.draw_open_rect(lo.x, lo.y, hi.x - lo.x, hi.y - lo.y, width, color);
    }

    /// `W3DInGameUI::drawMoveHints` (W3DInGameUI.cpp:405): the visual feedback for clicking in the world and telling
    /// units to move there. `hints`: `m_moveHint[]` (shorter than `MAX_MOVE_HINTS` stands for zeroed entries).
    pub fn draw_move_hints(&mut self, host: &mut dyn W3DInGameUiHost, hints: &[MoveHintStruct]) {
        for i in 0..MAX_MOVE_HINTS {
            let hint = hints.get(i).copied().unwrap_or_default();
            let elapsed = host.client_frame().wrapping_sub(hint.frame) as i32;
            if elapsed <= 40 {
                // if this hint is not in this view ignore it (@todo upstream: check if point is visible in view)

                // create render object and add to scene of needed
                if self.move_hint_render_obj[i].is_null() {
                    // create hint object
                    let name = host.move_hint_name();
                    let hint_obj = host.create_render_obj(&name);
                    let mut anim_name = name.clone();
                    anim_name.push(b'.');
                    anim_name.extend_from_slice(&name);
                    let anim = host.get_hanim(&anim_name);

                    // sanity
                    if hint_obj.is_null() {
                        // DEBUG_CRASH(("unable to create hint"));
                        return;
                    }

                    // asign render objects to GUI data
                    self.move_hint_render_obj[i] = hint_obj;

                    // note that 'anim' is returned from Get_HAnim with an AddRef, so we don't need to addref it again.
                    // however, we do need to release the contents of moveHintAnim (if any)
                    host.release_hanim(self.move_hint_anim[i]);
                    self.move_hint_anim[i] = anim;
                }

                // show the render object if hidden
                let obj = self.move_hint_render_obj[i];
                if host.is_hidden(obj) {
                    host.set_hidden(obj, false);

                    // add to scene
                    host.add_render_object(obj);
                    if self.move_hint_anim[i] != 0 {
                        host.set_animation_once(obj, self.move_hint_anim[i]);
                    }
                }

                // move this hint render object to the position and align with terrain
                let (layer, mut transform) = host.align_on_terrain(&hint.pos);
                if layer == LAYER_GROUND {
                    if let Some(water_z) = host.is_underwater(hint.pos.x, hint.pos.y) {
                        let mut tmp = hint.pos;
                        tmp.z = water_z;
                        let normal = Coord3D { x: 0.0, y: 0.0, z: 1.0 };
                        transform = host.make_align_to_normal_matrix(&tmp, &normal);
                    }
                }
                host.set_transform(obj, &transform);
                // (the `#if 0` source-to-destination line is dead upstream)
            } else {
                // hide hint marker
                let obj = self.move_hint_render_obj[i];
                if !obj.is_null() && !host.is_hidden(obj) {
                    host.set_hidden(obj, true);
                    host.remove_render_object(obj);
                }
            }
        }
    }

    /// `W3DInGameUI::drawAttackHints` (W3DInGameUI.cpp:530): empty upstream.
    pub fn draw_attack_hints(&mut self) {}

    /// `W3DInGameUI::drawPlaceAngle` (W3DInGameUI.cpp:540): the angle selection for placing a building.
    pub fn draw_place_angle(&mut self, host: &mut dyn W3DInGameUiHost) {
        //Create the anchor & arrow if not already created!
        if self.building_placement_anchor.is_null() {
            self.building_placement_anchor = host.create_render_obj(b"Locater01");
            // sanity
            if self.building_placement_anchor.is_null() {
                // DEBUG_CRASH(("Unable to create BuildingPlacementAnchor (Locator01.w3d) -- cursor for placing buildings"));
                return;
            }
        }
        if self.building_placement_arrow.is_null() {
            self.building_placement_arrow = host.create_render_obj(b"Locater02");
            // sanity
            if self.building_placement_arrow.is_null() {
                // DEBUG_CRASH(("Unable to create BuildingPlacementArrow (Locator02.w3d) -- cursor for placing buildings"));
                return;
            }
        }

        let mut anchor_in_scene = host.in_scene(self.building_placement_anchor);
        let mut arrow_in_scene = host.in_scene(self.building_placement_arrow);

        // get out of here if this display isn't up anyway
        if !host.is_placement_anchored() {
            if anchor_in_scene {
                //If our anchor is in the scene, remove it from the scene but don't delete it.
                host.remove_render_object(self.building_placement_anchor);
            }
            if arrow_in_scene {
                //If our arrow is in the scene, remove it from the scene but don't delete it.
                host.remove_render_object(self.building_placement_arrow);
            }
            return;
        }

        // get the anchor points
        let (start, end) = host.placement_points();
        let vector = Coord3D { x: (end.x - start.x) as f32, y: (end.y - start.y) as f32, z: 0.0 };
        let length = rts_lockstep_engine::common::base_type::sqrt_via_double(vector.x * vector.x + vector.y * vector.y + vector.z * vector.z);
        let show_arrow = length >= 5.0;

        if show_arrow {
            if anchor_in_scene {
                //We're switching to the arrow!
                host.remove_render_object(self.building_placement_anchor);
            }
            if !arrow_in_scene {
                host.add_render_object(self.building_placement_arrow);
                arrow_in_scene = true;
            }
        } else {
            if arrow_in_scene {
                //We're switching to the anchor!
                host.remove_render_object(self.building_placement_arrow);
            }
            if !anchor_in_scene {
                host.add_render_object(self.building_placement_anchor);
                anchor_in_scene = true;
            }
        }

        //The proper way to orient the placement arrow is to copy the matrix from the m_placeIcon[0]!
        // (anchorInScene / arrowInScene are the values from BEFORE the switch above for the one not touched, as upstream)
        if anchor_in_scene {
            if let Some(m) = host.place_icon_transform() {
                host.set_transform(self.building_placement_anchor, &m);
            }
        } else if arrow_in_scene {
            if let Some(m) = host.place_icon_transform() {
                host.set_transform(self.building_placement_arrow, &m);
            }
        }
        // (the commented-out line drawing is dead upstream)
    }
}

impl Default for W3DInGameUI {
    fn default() -> Self {
        Self::new()
    }
}

/// `DebugHintObject::setLocAndColorAndSize`'s triangle (W3DInGameUI.cpp:170-196, `_DEBUG` only): three vertices
/// `(x, y, z, diffuse)`, counterclockwise. `loc.z < 0` is set from the terrain height by the caller.
pub fn debug_hint_vertices(argb: i32, size: i32) -> [(f32, f32, f32, i32); 3] {
    let size = size as f32;
    let x1 = size * 0.866; // cos(30)
    let y1 = size * 0.5; // sin(30)
    // note, pts must go in a counterclockwise order!
    [(0.0, size, 0.0, argb), (-x1, -y1, 0.0, argb), (x1, -y1, 0.0, argb)]
}
