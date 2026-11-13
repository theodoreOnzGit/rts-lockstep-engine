// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTruckDraw.cpp
//   (W3DTruckDraw: the constructor's members, updateBones, onRenderObjRecreated, doDrawModule
//   without the dust, dirt and powerslide effects and sounds)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTankTruckDraw.cpp
//   (W3DTankTruckDraw: the constructor's members, updateBones, updateTreadPositions,
//   updateTreadObjects, onRenderObjRecreated, doDrawModule without the effects, sounds and
//   tread debris)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DPoliceCarDraw.cpp
//   (W3DPoliceCarDraw: the constructor's m_curFrame, doDrawModule without the dynamic light)
//   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DTruckDraw.h,
//   W3DTankTruckDraw.h, W3DPoliceCarDraw.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (the game's draw modules by Steven Johnson, John Ahlquist, Mark Lorenzen and others)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The truck draw modules (GitHub #41): `W3DTruckDraw` (with `W3DOverlordTruckDraw`, whose own riders are not
//! translated, and `W3DPoliceCarDraw`) and `W3DTankTruckDraw`, built on [`W3DModelDraw`].
//!
//! # As upstream reads it
//!
//! - **The bones** (`updateBones`): each tire bone named in the module data is found by name in the render object
//!   (`Get_Bone_Index`, 0 when missing); a mid pair whose right bone is missing has its left one cleared too; the cab
//!   bone, and with it the trailer bone, when `CabBone` is named. A new render object (`onRenderObjRecreated`) clears
//!   the tire bones (not the cab's or trailer's) and finds them again; a draw that meets a render object other than the
//!   last one seen finds them again without clearing.
//! - **Each draw** (`doDrawModule`): the base class's first, then (with `ShowClientPhysics`, an object, a render object
//!   and physics) the truck's. The cab turns towards the wheel angle times `CabRotationMultiplier`, clamped between 0
//!   and the angle to the AI path's cached point, moving by `RotationDamping` of the difference each draw; the trailer
//!   the other way by `TrailerRotationMultiplier`. The tires spin by `TireRotationMultiplier` times the speed (negated
//!   when the locomotor moves backwards); the front tires (and mid-front) are raised by their suspension offsets and
//!   steered by the wheel angle, then spun (`Adjust_Z_Translation`, `Rotate_Z`, `Rotate_Y`); the rear (mid-rear,
//!   mid-mid) spun, then raised. Each bone is captured and controlled (#27's bone controls). The wheel offsets and angle
//!   are the drawable's (`getWheelInfo`, `crate::client::drawable_loco`); without them nothing is turned.
//! - **The tank truck**: the same tires (no mid-mid, cab or trailer), and its treads, every one a middle tread, scroll
//!   back by `TreadAnimationRate` each draw it moves under a motive force at `TreadDriveSpeedFraction` or more of its
//!   locomotor's speed.
//! - **The police car**: its own frame `m_curFrame` (a client random in [0, 10) at construction) advances by 0.25 each
//!   draw through the render object's animation (back to 0 past its last frame), set in `MANUAL` mode; then the truck's.
//!   Without a render object it draws nothing at all, not even the base class's.
//!
//! # Upstream quirks that are kept
//!
//! - The rear tires are always spun with `m_rearWheelRotation`, which adds `PowerslideRotationAddition` while
//!   powersliding; powersliding needs the powerslide effect, which is not made here (~~no particle systems~~
//!   **Corrected 2026-11-06 (GitHub #44):** the client has particle systems, but the truck draws do not make theirs
//!   yet), so it never happens, as upstream with a `PowerslideSpray` it cannot find.
//! - The mid-mid tires use the rear rotation and the rear offsets.
//! - The tank truck's mid-front tires start from the rear tires' last transform (no `Make_Identity` first), and its mid
//!   wheel rotations are never advanced.
//!
//! # Where this differs from upstream
//!
//! - **Uninitialised members** upstream are 0 here: `W3DTruckDraw`'s `m_cabBone`, `m_trailerBone`,
//!   `m_curCabRotation`, `m_curTrailerRotation`; `W3DTankTruckDraw`'s mid tire bones and mid wheel rotations; and
//!   `W3DTruckDrawModuleData`'s reals (its constructor sets none).
//! - The dust, dirt and powerslide particle systems, the landing and powerslide sounds, the tank truck's tread debris and
//!   the police car's dynamic light are not made (~~no particle systems,~~ no audio or dynamic lights yet; **Corrected
//!   2026-11-06 (GitHub #44):** the client's particle systems exist, `crate::client::particle_sys`, but these draws do
//!   not create theirs yet: `createEmitters`, `enableEmitters` and the speed multipliers are the remaining step). They
//!   change nothing that is drawn here.
//! - Time frozen is not checked (the client has none yet), as for the tank.
//! - The tank truck's locomotor speed: without an AI upstream dereferences null; here its treads do not move then.

use crate::common::random_value::RandomValues;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::wwmath;

use crate::formats::w3d::render::AnimMode;

use super::w3d_model_draw::{
    find_tread_objects, DrawEnv, DrawFacts, DrawModuleExtra, RenderObj, TreadObjectInfo, TruckFields, W3DModelDraw, W3dAssetSource,
};

/// `W3DTruckDraw`'s own members (also `W3DPoliceCarDraw`'s base).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TruckDraw {
    /// `m_isPowersliding`.
    pub is_powersliding: bool,
    /// `m_frontWheelRotation`, `m_rearWheelRotation`, `m_midFrontWheelRotation`, `m_midRearWheelRotation`.
    pub front_wheel_rotation: f32,
    pub rear_wheel_rotation: f32,
    pub mid_front_wheel_rotation: f32,
    pub mid_rear_wheel_rotation: f32,
    /// The tire bones, 0 for none: front left, front right, rear left, rear right, mid-front left and right,
    /// mid-rear left and right, mid-mid left and right.
    pub front_left_tire_bone: i32,
    pub front_right_tire_bone: i32,
    pub rear_left_tire_bone: i32,
    pub rear_right_tire_bone: i32,
    pub mid_front_left_tire_bone: i32,
    pub mid_front_right_tire_bone: i32,
    pub mid_rear_left_tire_bone: i32,
    pub mid_rear_right_tire_bone: i32,
    pub mid_mid_left_tire_bone: i32,
    pub mid_mid_right_tire_bone: i32,
    /// `m_cabBone`, `m_curCabRotation`, `m_trailerBone`, `m_curTrailerRotation`.
    pub cab_bone: i32,
    pub cur_cab_rotation: f32,
    pub trailer_bone: i32,
    pub cur_trailer_rotation: f32,
    /// `m_prevRenderObj`, by the render object's serial ([`W3DModelDraw::render_obj_serial`]).
    pub prev_render_obj: Option<u32>,
}

/// `W3DTankTruckDraw`'s own members.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TankTruckDraw {
    /// `m_isPowersliding`.
    pub is_powersliding: bool,
    /// `m_frontWheelRotation`, `m_rearWheelRotation`, `m_midFrontWheelRotation`, `m_midRearWheelRotation`.
    pub front_wheel_rotation: f32,
    pub rear_wheel_rotation: f32,
    pub mid_front_wheel_rotation: f32,
    pub mid_rear_wheel_rotation: f32,
    /// The tire bones, 0 for none.
    pub front_left_tire_bone: i32,
    pub front_right_tire_bone: i32,
    pub rear_left_tire_bone: i32,
    pub rear_right_tire_bone: i32,
    pub mid_front_left_tire_bone: i32,
    pub mid_front_right_tire_bone: i32,
    pub mid_rear_left_tire_bone: i32,
    pub mid_rear_right_tire_bone: i32,
    /// `m_prevRenderObj`, by serial.
    pub prev_render_obj: Option<u32>,
    /// `m_treads` (`m_treadCount` is the length).
    pub treads: Vec<TreadObjectInfo>,
}

/// Which truck draw a module is, and its own members.
#[derive(Clone, Debug, PartialEq)]
pub enum TruckKind {
    /// `W3DTruckDraw`, `W3DOverlordTruckDraw`.
    Truck(TruckDraw),
    /// `W3DPoliceCarDraw`: the truck's members and `m_curFrame`.
    PoliceCar(TruckDraw, f32),
    /// `W3DTankTruckDraw`.
    TankTruck(TankTruckDraw),
}

impl TruckKind {
    /// The subclass's members for a draw module's class: the trucks' constructors (the police car's
    /// `m_curFrame = GameClientRandomValueReal(0, 10)`, the client's generator). `None` for another module.
    pub fn new_for(module_name: &[u8], extra: &DrawModuleExtra, random: &mut RandomValues) -> Option<TruckKind> {
        match (module_name, extra) {
            (b"W3DPoliceCarDraw", DrawModuleExtra::Truck(_)) => Some(TruckKind::PoliceCar(TruckDraw::default(), random.game_client_random_value_real(0.0, 10.0))),
            (_, DrawModuleExtra::Truck(_)) => Some(TruckKind::Truck(TruckDraw::default())),
            (_, DrawModuleExtra::TankTruck(..)) => Some(TruckKind::TankTruck(TankTruckDraw::default())),
            _ => None,
        }
    }
}

/// `getRenderObject()->Get_Bone_Index(name)`: 0 without a hierarchy or such a bone.
fn bone_index(r: &RenderObj, name: &[u8]) -> i32 {
    if !r.is_hlod {
        return 0;
    }
    r.skeleton.as_ref().map_or(0, |k| k.get_bone_index(name))
}

/// A string field of the truck data (empty when not given).
fn s<'a>(f: &'a TruckFields, name: &str) -> &'a [u8] {
    f.strings.get(name).map_or(&[][..], |v| v.as_slice())
}

/// A real field of the truck data (0 when not given; uninitialised upstream).
fn real(f: &TruckFields, name: &str) -> f32 {
    f.reals.get(name).copied().unwrap_or(0.0)
}

/// `Capture_Bone(bone)` and `Control_Bone(bone, xfrm)`.
fn control(r: &mut RenderObj, bone: i32, xfrm: &Matrix3D) {
    r.capture_bone(bone);
    r.control_bone(bone, xfrm);
}

impl TruckDraw {
    /// `W3DTruckDraw::updateBones()`: the bones named in `f` found in `r`, then `m_prevRenderObj` is `r` (`id`).
    pub fn update_bones(&mut self, f: &TruckFields, r: &RenderObj, id: Option<u32>) {
        //Front tires
        if !s(f, "LeftFrontTireBone").is_empty() {
            self.front_left_tire_bone = bone_index(r, s(f, "LeftFrontTireBone"));
        }
        if !s(f, "RightFrontTireBone").is_empty() {
            self.front_right_tire_bone = bone_index(r, s(f, "RightFrontTireBone"));
        }
        //Rear tires
        if !s(f, "LeftRearTireBone").is_empty() {
            self.rear_left_tire_bone = bone_index(r, s(f, "LeftRearTireBone"));
        }
        if !s(f, "RightRearTireBone").is_empty() {
            self.rear_right_tire_bone = bone_index(r, s(f, "RightRearTireBone"));
        }
        //midFront tires
        if !s(f, "MidLeftFrontTireBone").is_empty() {
            self.mid_front_left_tire_bone = bone_index(r, s(f, "MidLeftFrontTireBone"));
            self.mid_front_right_tire_bone = bone_index(r, s(f, "MidRightFrontTireBone"));
            if self.mid_front_right_tire_bone == 0 {
                self.mid_front_left_tire_bone = 0;
            }
        }
        //midRear tires
        if !s(f, "MidLeftRearTireBone").is_empty() {
            self.mid_rear_left_tire_bone = bone_index(r, s(f, "MidLeftRearTireBone"));
            self.mid_rear_right_tire_bone = bone_index(r, s(f, "MidRightRearTireBone"));
            if self.mid_rear_right_tire_bone == 0 {
                self.mid_rear_left_tire_bone = 0;
            }
        }
        //midMid tires
        if !s(f, "MidLeftMidTireBone").is_empty() {
            self.mid_mid_left_tire_bone = bone_index(r, s(f, "MidLeftMidTireBone"));
            self.mid_mid_right_tire_bone = bone_index(r, s(f, "MidRightMidTireBone"));
            if self.mid_mid_right_tire_bone == 0 {
                self.mid_mid_left_tire_bone = 0;
            }
        }
        //Cab
        if !s(f, "CabBone").is_empty() {
            self.cab_bone = bone_index(r, s(f, "CabBone"));
            self.trailer_bone = bone_index(r, s(f, "TrailerBone"));
        }
        self.prev_render_obj = id;
    }

    /// `W3DTruckDraw::onRenderObjRecreated()`: the tire bones cleared, then [`TruckDraw::update_bones`].
    pub fn on_render_obj_recreated(&mut self, f: &TruckFields, r: &RenderObj, id: Option<u32>) {
        self.prev_render_obj = None;
        self.front_left_tire_bone = 0;
        self.front_right_tire_bone = 0;
        self.rear_left_tire_bone = 0;
        self.rear_right_tire_bone = 0;
        self.mid_front_left_tire_bone = 0;
        self.mid_front_right_tire_bone = 0;
        self.mid_rear_left_tire_bone = 0;
        self.mid_rear_right_tire_bone = 0;
        self.mid_mid_left_tire_bone = 0;
        self.mid_mid_right_tire_bone = 0;
        self.update_bones(f, r, id);
    }

    /// `W3DTruckDraw::doDrawModule`'s own part, after the base class's (see the module docs): the cab, trailer and tire
    /// bones of `r` (the current render object, serial `id`) controlled from `facts`.
    pub fn do_draw_module(&mut self, f: &TruckFields, r: &mut RenderObj, id: Option<u32>, facts: &DrawFacts) {
        if !facts.show_client_physics {
            return;
        }
        if id != self.prev_render_obj {
            // ("shouldn't update bones. jba")
            self.update_bones(f, r, id);
        }
        // get object physics state
        let Some(m) = facts.motion else { return };
        let mut speed = m.velocity_magnitude;
        let wheel_info = facts.wheel_info; // note, can return null!

        if self.cab_bone != 0 {
            if let Some(wheel_info) = &wheel_info {
                let mut cab_xfrm = Matrix3D::IDENTITY;
                let mut desired_angle = wheel_info.wheel_angle * real(f, "CabRotationMultiplier");
                // Check goal angle.
                if let Some(angle_to_goal) = m.angle_to_path_point {
                    if angle_to_goal < 0.0 {
                        if desired_angle < angle_to_goal {
                            desired_angle = angle_to_goal;
                        }
                        if desired_angle > 0.0 {
                            desired_angle = 0.0;
                        }
                    } else {
                        if desired_angle > angle_to_goal {
                            desired_angle = angle_to_goal;
                        }
                        if desired_angle < 0.0 {
                            desired_angle = 0.0;
                        }
                    }
                }
                let mut delta_angle = desired_angle - self.cur_cab_rotation;
                delta_angle *= real(f, "RotationDamping");
                self.cur_cab_rotation += delta_angle;
                cab_xfrm.rotate_z(self.cur_cab_rotation);
                control(r, self.cab_bone, &cab_xfrm);
                if self.trailer_bone != 0 {
                    let desired_angle = -wheel_info.wheel_angle * real(f, "TrailerRotationMultiplier");
                    let mut delta_angle = desired_angle - self.cur_trailer_rotation;
                    delta_angle *= real(f, "RotationDamping");
                    self.cur_trailer_rotation += delta_angle;
                    cab_xfrm.make_identity();
                    cab_xfrm.rotate_z(self.cur_trailer_rotation);
                    control(r, self.trailer_bone, &cab_xfrm);
                }
            }
        }

        if self.front_left_tire_bone != 0 || self.rear_left_tire_bone != 0 {
            let mut powerslide_rotation_addition = real(f, "PowerslideRotationAddition");
            if m.moving_backwards {
                speed = -speed; // rotate wheels backwards.  jba.
                powerslide_rotation_addition = -powerslide_rotation_addition;
            }
            let rotation_factor = real(f, "TireRotationMultiplier");
            self.front_wheel_rotation += rotation_factor * speed;
            if self.is_powersliding {
                self.rear_wheel_rotation += rotation_factor * (speed + powerslide_rotation_addition);
            } else {
                self.rear_wheel_rotation += rotation_factor * speed;
            }
            // For now, just use the same values for mid wheels -- may want to do independent calcs later...
            self.mid_front_wheel_rotation = self.front_wheel_rotation;
            self.mid_rear_wheel_rotation = self.rear_wheel_rotation;

            if let Some(w) = &wheel_info {
                let front = |height: f32, rotation: f32| {
                    let mut x = Matrix3D::IDENTITY;
                    x.adjust_z_translation(height);
                    x.rotate_z(w.wheel_angle);
                    x.rotate_y(rotation);
                    x
                };
                let rear = |height: f32, rotation: f32| {
                    let mut x = Matrix3D::IDENTITY;
                    x.rotate_y(rotation);
                    x.adjust_z_translation(height);
                    x
                };
                if self.front_left_tire_bone != 0 {
                    control(r, self.front_left_tire_bone, &front(w.front_left_height_offset, self.front_wheel_rotation));
                    control(r, self.front_right_tire_bone, &front(w.front_right_height_offset, self.front_wheel_rotation));
                }
                if self.rear_left_tire_bone != 0 {
                    control(r, self.rear_left_tire_bone, &rear(w.rear_left_height_offset, self.rear_wheel_rotation));
                    control(r, self.rear_right_tire_bone, &rear(w.rear_right_height_offset, self.rear_wheel_rotation));
                }
                if self.mid_front_left_tire_bone != 0 {
                    control(r, self.mid_front_left_tire_bone, &front(w.front_left_height_offset, self.mid_front_wheel_rotation));
                    control(r, self.mid_front_right_tire_bone, &front(w.front_right_height_offset, self.mid_front_wheel_rotation));
                }
                if self.mid_rear_left_tire_bone != 0 {
                    control(r, self.mid_rear_left_tire_bone, &rear(w.rear_left_height_offset, self.mid_rear_wheel_rotation));
                    control(r, self.mid_rear_right_tire_bone, &rear(w.rear_right_height_offset, self.mid_rear_wheel_rotation));
                }
                if self.mid_mid_left_tire_bone != 0 {
                    control(r, self.mid_mid_left_tire_bone, &rear(w.rear_left_height_offset, self.mid_rear_wheel_rotation));
                    control(r, self.mid_mid_right_tire_bone, &rear(w.rear_right_height_offset, self.mid_rear_wheel_rotation));
                }
            }
        }

        // the effects and sounds are not made: no powerslide effect, so never powersliding
        self.is_powersliding = false;
    }
}

impl TankTruckDraw {
    /// `W3DTankTruckDraw::updateBones()`: each pair found when its left bone is named, its left cleared when its right
    /// is missing; then `m_prevRenderObj` is `r` (`id`).
    pub fn update_bones(&mut self, f: &TruckFields, r: &RenderObj, id: Option<u32>) {
        let pair = |l: &str, rt: &str, left: &mut i32, right: &mut i32| {
            if !s(f, l).is_empty() {
                *left = bone_index(r, s(f, l));
                *right = bone_index(r, s(f, rt));
                if *right == 0 {
                    *left = 0;
                }
            }
        };
        //Front tires
        pair("LeftFrontTireBone", "RightFrontTireBone", &mut self.front_left_tire_bone, &mut self.front_right_tire_bone);
        //Rear tires
        pair("LeftRearTireBone", "RightRearTireBone", &mut self.rear_left_tire_bone, &mut self.rear_right_tire_bone);
        //midFront tires
        pair("MidLeftFrontTireBone", "MidRightFrontTireBone", &mut self.mid_front_left_tire_bone, &mut self.mid_front_right_tire_bone);
        //midRear tires
        pair("MidLeftRearTireBone", "MidRightRearTireBone", &mut self.mid_rear_left_tire_bone, &mut self.mid_rear_right_tire_bone);
        self.prev_render_obj = id;
    }

    /// `W3DTankTruckDraw::updateTreadObjects()`: every tread a middle one ([`find_tread_objects`]); the render
    /// object's material overrides set to them; `m_prevRenderObj` is `r` (`id`).
    pub fn update_tread_objects(&mut self, rate: f32, r: Option<&mut RenderObj>, id: Option<u32>) {
        let (treads, overrides) = find_tread_objects(r.as_deref(), rate, false);
        if let Some(r) = r {
            r.material_overrides = overrides;
        }
        self.treads = treads;
        self.prev_render_obj = id;
    }

    /// `W3DTankTruckDraw::updateTreadPositions(uvDelta)`: a middle or left tread forwards by `uv_delta`, a right one
    /// backwards, wrapped into [0, 1). (Not called upstream: its pivot scrolling is commented out.)
    pub fn update_tread_positions(&mut self, uv_delta: f32) {
        for t in &mut self.treads {
            let mut offset_u = match t.kind {
                super::w3d_model_draw::TreadType::Right => t.custom_uv_offset[0] - uv_delta,
                _ => t.custom_uv_offset[0] + uv_delta,
            };
            // ensure coordinates of offset are in [0, 1] range:
            offset_u -= wwmath::floor(offset_u);
            t.custom_uv_offset = [offset_u, 0.0];
        }
    }

    /// `W3DTankTruckDraw::doDrawModule`'s own part, after the base class's (see the module docs).
    pub fn do_draw_module(&mut self, f: &TruckFields, tank: (f32, f32), r: &mut RenderObj, id: Option<u32>, facts: &DrawFacts) {
        let (tread_animation_rate, tread_drive_speed_fraction) = tank;
        if !facts.show_client_physics {
            return;
        }
        if id != self.prev_render_obj {
            self.update_bones(f, r, id);
            self.update_tread_objects(tread_animation_rate, Some(&mut *r), id);
        }
        // get object physics state
        let Some(m) = facts.motion else { return };
        let speed = m.velocity_magnitude;
        if let Some(w) = &facts.wheel_info {
            if self.front_left_tire_bone != 0 || self.rear_left_tire_bone != 0 {
                let rotation_factor = real(f, "TireRotationMultiplier");
                self.front_wheel_rotation += rotation_factor * speed;
                if self.is_powersliding {
                    self.rear_wheel_rotation += rotation_factor * (speed + real(f, "PowerslideRotationAddition"));
                } else {
                    self.rear_wheel_rotation += rotation_factor * speed;
                }

                let mut wheel_xfrm = Matrix3D::IDENTITY;
                if self.front_left_tire_bone != 0 {
                    wheel_xfrm.adjust_z_translation(w.front_left_height_offset);
                    wheel_xfrm.rotate_z(w.wheel_angle);
                    wheel_xfrm.rotate_y(self.front_wheel_rotation);
                    control(r, self.front_left_tire_bone, &wheel_xfrm);
                    wheel_xfrm.make_identity();
                    wheel_xfrm.adjust_z_translation(w.front_right_height_offset);
                    wheel_xfrm.rotate_z(w.wheel_angle);
                    wheel_xfrm.rotate_y(self.front_wheel_rotation);
                    control(r, self.front_right_tire_bone, &wheel_xfrm);
                }
                if self.rear_left_tire_bone != 0 {
                    wheel_xfrm.make_identity();
                    wheel_xfrm.rotate_y(self.rear_wheel_rotation);
                    wheel_xfrm.adjust_z_translation(w.rear_left_height_offset);
                    control(r, self.rear_left_tire_bone, &wheel_xfrm);
                    wheel_xfrm.make_identity();
                    wheel_xfrm.rotate_y(self.rear_wheel_rotation);
                    wheel_xfrm.adjust_z_translation(w.rear_right_height_offset);
                    control(r, self.rear_right_tire_bone, &wheel_xfrm);
                }
                if self.mid_front_left_tire_bone != 0 {
                    // (no Make_Identity: from the last transform, as upstream)
                    wheel_xfrm.adjust_z_translation(w.front_left_height_offset);
                    wheel_xfrm.rotate_z(w.wheel_angle);
                    wheel_xfrm.rotate_y(self.mid_front_wheel_rotation);
                    control(r, self.mid_front_left_tire_bone, &wheel_xfrm);
                    wheel_xfrm.make_identity();
                    wheel_xfrm.adjust_z_translation(w.front_right_height_offset);
                    wheel_xfrm.rotate_z(w.wheel_angle);
                    wheel_xfrm.rotate_y(self.mid_front_wheel_rotation);
                    control(r, self.mid_front_right_tire_bone, &wheel_xfrm);
                }
                if self.mid_rear_left_tire_bone != 0 {
                    wheel_xfrm.make_identity();
                    wheel_xfrm.rotate_y(self.mid_rear_wheel_rotation);
                    wheel_xfrm.adjust_z_translation(w.rear_left_height_offset);
                    control(r, self.mid_rear_left_tire_bone, &wheel_xfrm);
                    wheel_xfrm.make_identity();
                    wheel_xfrm.rotate_y(self.mid_rear_wheel_rotation);
                    wheel_xfrm.adjust_z_translation(w.rear_right_height_offset);
                    control(r, self.mid_rear_right_tire_bone, &wheel_xfrm);
                }
            }
        }

        // the effects and sounds are not made: no powerslide effect, so never powersliding
        self.is_powersliding = false;

        //Update movement of treads
        if !self.treads.is_empty() {
            if let Some(max_speed) = m.cur_locomotor_speed {
                if m.is_motive && m.velocity_magnitude / max_speed >= tread_drive_speed_fraction {
                    //do simple scrolling based only on speed when tank is moving straight at high speed.
                    for t in &mut self.treads {
                        let mut offset_u = t.custom_uv_offset[0] - tread_animation_rate;
                        // ensure coordinates of offset are in [0, 1] range:
                        offset_u -= wwmath::floor(offset_u);
                        t.custom_uv_offset = [offset_u, 0.0];
                    }
                }
            }
            for t in &self.treads {
                r.material_overrides.insert(t.sub_object, t.custom_uv_offset);
            }
        }
    }
}

/// The truck data and, for the tank truck, its tread rate and drive fraction; `None` for another module's data.
fn truck_fields(extra: &DrawModuleExtra) -> Option<(&TruckFields, (f32, f32))> {
    match extra {
        DrawModuleExtra::Truck(f) => Some((f, (0.0, 0.0))),
        DrawModuleExtra::TankTruck(f, t) => Some((f, (t.tread_animation_rate, t.tread_drive_speed_fraction))),
        _ => None,
    }
}

impl W3DModelDraw {
    /// `onRenderObjRecreated()` of a truck draw: the truck's clears and finds its tire bones; the tank truck's too, and
    /// finds its treads.
    pub(crate) fn truck_on_render_obj_recreated(&mut self) {
        let id = self.render_obj_id();
        let data = self.data.clone();
        let Some((f, (rate, _))) = truck_fields(&data.extra) else { return };
        let Some(r) = self.render_object.as_mut() else { return };
        match &mut self.truck {
            Some(TruckKind::Truck(t)) | Some(TruckKind::PoliceCar(t, _)) => t.on_render_obj_recreated(f, r, id),
            Some(TruckKind::TankTruck(t)) => {
                t.prev_render_obj = None;
                t.front_left_tire_bone = 0;
                t.front_right_tire_bone = 0;
                t.rear_left_tire_bone = 0;
                t.rear_right_tire_bone = 0;
                t.mid_front_left_tire_bone = 0;
                t.mid_front_right_tire_bone = 0;
                t.mid_rear_left_tire_bone = 0;
                t.mid_rear_right_tire_bone = 0;
                t.update_bones(f, r, id);
                t.update_tread_objects(rate, Some(r), id);
            }
            None => {}
        }
    }

    /// `doDrawModule` of a truck draw (see the module docs): the police car's animation frame first (nothing at all
    /// without a render object), then the base class's, then the truck's or tank truck's own part. The tank's tread draw
    /// does not apply (a truck's data is not a tank's).
    pub(crate) fn truck_kind_do_draw_module<A: W3dAssetSource>(&mut self, facts: &DrawFacts, env: &mut DrawEnv<'_, A>) {
        if let Some(TruckKind::PoliceCar(_, cur_frame)) = &mut self.truck {
            const ANIM_AMT: f32 = 0.25;
            // get pointers to our render objects that we'll need
            let Some(r) = self.render_object.as_mut() else { return };
            if let Some(anim) = r.anim.as_ref().map(|a| a.motion.clone()) {
                let frames = anim.num_frames as f32;
                *cur_frame += ANIM_AMT;
                if *cur_frame > frames - 1.0 {
                    *cur_frame = 0.0;
                }
                r.set_animation(Some(anim), *cur_frame, AnimMode::Manual, env.sync_time);
            }
            // (the dynamic light's colour from the frame is not drawn: no dynamic lights)
        }
        self.model_draw_do_draw_module(facts, env);
        let id = self.render_obj_id();
        let data = self.data.clone();
        let Some((f, tank)) = truck_fields(&data.extra) else { return };
        if !facts.show_client_physics {
            return;
        }
        let Some(r) = self.render_object.as_mut() else { return };
        match &mut self.truck {
            Some(TruckKind::Truck(t)) | Some(TruckKind::PoliceCar(t, _)) => t.do_draw_module(f, r, id, facts),
            Some(TruckKind::TankTruck(t)) => t.do_draw_module(f, tank, r, id, facts),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::drawable_loco::{DrawableLocoInfo, LocoFacts, PhysicsXformInfo, TWheelInfo, XformObjectFacts};
    use crate::client::w3d_model_draw::{MotionFacts, TreadType, W3DModelDrawModuleData, W3dFiles, TURN_NEGATIVE, TURN_NONE, TURN_POSITIVE};
    use crate::common::base_type::Coord3D;
    use crate::common::file_system::{FileSystem, MemoryDevice};
    use crate::common::model_state::{ModelConditionFlags, MODELCONDITION_DAMAGED};
    use crate::common::thing_template::{ClientLine, ClientModuleData};
    use crate::formats::w3d::assetmgr::{AssetManager, W3dFileFactory};
    use crate::formats::w3d::synth::*;
    use crate::formats::w3d::w3d_file::ANIM_CHANNEL_Q;
    use crate::logic::locomotor::LocomotorAppearance;
    use std::sync::Arc;

    /// A `Draw` block as the logic keeps it, from `(depth, field, args)`.
    fn block(lines: &[(u32, &str, &str)]) -> ClientModuleData {
        ClientModuleData {
            module_tag_name_key: Default::default(),
            lines: lines
                .iter()
                .map(|(d, f, a)| ClientLine { depth: *d, field: f.as_bytes().to_vec(), args: a.split_whitespace().map(|t| t.as_bytes().to_vec()).collect() })
                .collect(),
        }
    }

    /// A truck `NAME`: pivots ROOTTRANSFORM (0), CAB (1), TRAILER (2), TIREFL (3), TIREFR (4), TIRERL (5), TIRERR (6), a
    /// cube on each; `treads`: two more cubes on the root, `TREADSL` and `TREADSR`, each with a linear-offset mapper. With
    /// `anim`, a 4-frame animation `NAME.LIGHTS` turning nothing much (one quaternion channel on the cab).
    fn truck_model(name: &[u8], treads: bool, anim: bool) -> Vec<u8> {
        let ident = [0.0, 0.0, 0.0, 1.0];
        let p = |n: &[u8], parent: i32, t: [f32; 3]| PivotSpec { name: n.to_vec(), parent, translation: t, rotation: ident };
        let mut f = htree_w3d(name, &[
            p(b"ROOTTRANSFORM", -1, [0.0; 3]),
            p(b"CAB", 0, [2.0, 0.0, 1.0]),
            p(b"TRAILER", 0, [-3.0, 0.0, 1.0]),
            p(b"TIREFL", 0, [2.0, 1.0, 0.0]),
            p(b"TIREFR", 0, [2.0, -1.0, 0.0]),
            p(b"TIRERL", 0, [-2.0, 1.0, 0.0]),
            p(b"TIRERR", 0, [-2.0, -1.0, 0.0]),
        ]);
        let mut subs: Vec<(&[u8], u32, u32)> =
            vec![(b"CHASSIS", 0, 0), (b"CAB", 1, 0), (b"TRAILER", 2, 0), (b"TIREFL", 3, 0), (b"TIREFR", 4, 0), (b"TIRERL", 5, 0), (b"TIRERR", 6, 0)];
        if treads {
            subs.push((b"TREADSL", 0, 4));
            subs.push((b"TREADSR", 0, 4));
        }
        let mut names = Vec::new();
        for (n, bone, mapping) in &subs {
            let mut m = MeshSpec::cuboid(name, n, [-0.5; 3], [0.5; 3]);
            m.stage0.texture = None;
            m.stage0.mapping = *mapping;
            f.extend(m.to_w3d());
            names.push(([name, b".", n].concat(), *bone));
        }
        let hl: Vec<(&[u8], u32)> = names.iter().map(|(n, b)| (n.as_slice(), *b)).collect();
        f.extend(hlod_w3d(name, name, &hl));
        if anim {
            let q = [0.0, 0.0, 0.0, 1.0];
            let data: Vec<f32> = (0..4).flat_map(|_| q).collect();
            f.extend(raw_anim_w3d(name, b"LIGHTS", 4, 10, &[ChannelSpec { pivot: 1, kind: ANIM_CHANNEL_Q, vector_len: 4, data }]));
        }
        f
    }

    const ROOT: i32 = 0;
    const CAB: i32 = 1;
    const TRAILER: i32 = 2;
    const FL: i32 = 3;
    const FR: i32 = 4;
    const RL: i32 = 5;
    const RR: i32 = 6;

    const TIRES: &[(u32, &str, &str)] = &[
        (0, "LeftFrontTireBone", "TireFL"),
        (0, "RightFrontTireBone", "TireFR"),
        (0, "LeftRearTireBone", "TireRL"),
        (0, "RightRearTireBone", "TireRR"),
        (0, "TireRotationMultiplier", "0.5"),
    ];

    struct Rig {
        fs: FileSystem<MemoryDevice>,
        am: AssetManager,
        random: RandomValues,
    }

    impl Rig {
        fn new() -> Self {
            let mut d = MemoryDevice::new();
            d.insert(b"Art/W3D/truckt.w3d", truck_model(b"TRUCKT", false, false));
            d.insert(b"Art/W3D/trucku.w3d", truck_model(b"TRUCKU", false, false));
            d.insert(b"Art/W3D/tankru.w3d", truck_model(b"TANKRU", true, false));
            d.insert(b"Art/W3D/copcar.w3d", truck_model(b"COPCAR", false, true));
            Rig { fs: FileSystem::new(d), am: AssetManager::new_game(W3dFileFactory::default()), random: RandomValues::new(0) }
        }
        /// A draw module `module` of `model` (`TRUCKU` when `DAMAGED`) with `extra` lines after the states.
        fn draw(&mut self, module: &str, model: &str, anim: Option<&str>, extra: &[(u32, &str, &str)]) -> W3DModelDraw {
            let mut l = vec![(0, "DefaultConditionState", ""), (1, "Model", model)];
            if let Some(a) = anim {
                l.push((1, "Animation", a));
                l.push((1, "AnimationMode", "LOOP"));
            }
            l.push((0, "ConditionState", "DAMAGED"));
            l.push((1, "Model", "TRUCKU"));
            l.extend_from_slice(extra);
            let d = Arc::new(W3DModelDrawModuleData::parse(module.as_bytes(), &block(&l)).unwrap().unwrap());
            let mut a = W3dFiles { assets: &mut self.am, fs: &self.fs };
            let mut env = DrawEnv { assets: &mut a, random: &mut self.random, sync_time: 0, logic_time: true, particles: None };
            W3DModelDraw::new(d, 0, false, false, &mut env).unwrap()
        }
        fn frame(&mut self, m: &mut W3DModelDraw, facts: &DrawFacts) {
            let mut a = W3dFiles { assets: &mut self.am, fs: &self.fs };
            let mut env = DrawEnv { assets: &mut a, random: &mut self.random, sync_time: 0, logic_time: false, particles: None };
            m.do_draw_module(facts, &mut env);
        }
        fn set_flags(&mut self, m: &mut W3DModelDraw, bits: &[i32]) {
            let mut a = W3dFiles { assets: &mut self.am, fs: &self.fs };
            let mut env = DrawEnv { assets: &mut a, random: &mut self.random, sync_time: 0, logic_time: false, particles: None };
            m.replace_model_condition_state(&ModelConditionFlags::from_indices(bits), &mut env);
        }
    }

    fn motion(v: f32, backwards: bool, path_angle: Option<f32>) -> MotionFacts {
        MotionFacts {
            velocity_magnitude: v,
            turning: TURN_NONE,
            is_motive: true,
            cur_locomotor_speed: Some(4.0),
            unit_direction_2d: (1.0, 0.0),
            velocity: Coord3D { x: v, y: 0.0, z: 0.0 },
            moving_backwards: backwards,
            angle_to_path_point: path_angle,
        }
    }

    fn facts(m: Option<MotionFacts>, wheel_info: Option<TWheelInfo>) -> DrawFacts {
        DrawFacts {
            should_animate_with_power: true,
            should_animate_without_power: true,
            speed: m.map(|m| m.velocity_magnitude),
            turrets: [None, None],
            motion: m,
            show_client_physics: true,
            wheel_info,
            transform: crate::ww::matrix3d::Matrix3D::IDENTITY,
            construction: None,
        }
    }

    fn truck(m: &W3DModelDraw) -> &TruckDraw {
        match m.truck.as_ref().unwrap() {
            TruckKind::Truck(t) | TruckKind::PoliceCar(t, _) => t,
            TruckKind::TankTruck(_) => panic!("a truck"),
        }
    }

    /// The bone's control, if captured.
    fn ctl(m: &W3DModelDraw, bone: i32) -> Option<Matrix3D> {
        m.render_object.as_ref().unwrap().bone_controls.get(&bone).filter(|c| c.captured).map(|c| c.transform)
    }

    fn rot_y(a: f32) -> Matrix3D {
        let mut x = Matrix3D::IDENTITY;
        x.rotate_y(a);
        x
    }

    fn rot_z(a: f32) -> Matrix3D {
        let mut x = Matrix3D::IDENTITY;
        x.rotate_z(a);
        x
    }

    /// *Predictions, written before the first run* (`TireRotationMultiplier` 0.5):
    /// - the constructor finds no bones (its `setModelState` calls the base class's `onRenderObjRecreated`, C++'s
    ///   dispatch during construction); the first draw finds them: front left 3, front right 4, rear left 5, rear
    ///   right 6; no cab (`CabBone` not named);
    /// - driving at speed 2 with the wheel info level (offsets 0, angle 0), each draw spins every tire by
    ///   0.5 * 2 = 1 rad: after three draws the front and rear rotations are exactly 3, and each tire bone is captured
    ///   with the control `Rotate_Y(3)` of the identity (the front's `Adjust_Z_Translation(0)` and `Rotate_Z(0)` leave
    ///   it as it is);
    /// - with the locomotor moving backwards the tires turn back: one draw takes them to 2;
    /// - without the drawable's wheel info the rotations still advance, but no bone is controlled;
    /// - without `ShowClientPhysics`, nothing moves;
    /// - the client's render model for the truck carries the controls (its posed tire is turned).
    ///
    /// *Results (2026-11-06), first run:* all held: no bones after the constructor, (3, 4, 5, 6) after the first draw;
    /// rotations exactly 3 and every tire control exactly `Rotate_Y(3)`; backwards to exactly 2; no controls without
    /// the wheel info; nothing without `ShowClientPhysics`; the posed front-left tire within 1e-5 of its pivot turned by
    /// `Rotate_Y(2)`.
    #[test]
    fn a_moving_truck_s_tires_spin_by_its_speed() {
        let mut rig = Rig::new();
        let mut m = rig.draw("W3DTruckDraw", "TRUCKT", None, TIRES);
        let t = truck(&m);
        assert_eq!((t.front_left_tire_bone, t.rear_left_tire_bone, t.prev_render_obj), (0, 0, None));

        let level = Some(TWheelInfo::default());
        for _ in 0..3 {
            rig.frame(&mut m, &facts(Some(motion(2.0, false, None)), level));
        }
        let t = truck(&m);
        assert_eq!((t.front_left_tire_bone, t.front_right_tire_bone, t.rear_left_tire_bone, t.rear_right_tire_bone, t.cab_bone), (FL, FR, RL, RR, 0));
        assert_eq!((t.front_wheel_rotation, t.rear_wheel_rotation), (3.0, 3.0));
        for b in [FL, FR, RL, RR] {
            assert_eq!(ctl(&m, b), Some(rot_y(3.0)), "bone {b}");
        }
        assert_eq!(ctl(&m, CAB), None);
        assert_eq!(ctl(&m, ROOT), None);

        rig.frame(&mut m, &facts(Some(motion(2.0, true, None)), level));
        assert_eq!(truck(&m).front_wheel_rotation, 2.0);
        assert_eq!(ctl(&m, FL), Some(rot_y(2.0)));

        let mut m2 = rig.draw("W3DTruckDraw", "TRUCKT", None, TIRES);
        rig.frame(&mut m2, &facts(Some(motion(2.0, false, None)), None));
        assert_eq!(truck(&m2).front_wheel_rotation, 1.0);
        assert!(m2.render_object.as_ref().unwrap().bone_controls.is_empty());

        let mut off = facts(Some(motion(2.0, false, None)), level);
        off.show_client_physics = false;
        rig.frame(&mut m2, &off);
        assert_eq!(truck(&m2).front_wheel_rotation, 1.0);

        let mut model = crate::formats::w3d::render::RenderModel::create(&mut rig.am, &rig.fs, b"truckt").unwrap().unwrap();
        m.render_object.as_ref().unwrap().pose_render_model(&mut model);
        model.update::<AssetManager>(&Matrix3D::IDENTITY, None).unwrap();
        let tire = model.sub_objects.iter().position(|s| s.name.eq_ignore_ascii_case(b"TRUCKT.TIREFL")).unwrap();
        let posed = model.sub_object_transform(tire).unwrap();
        let mut want = Matrix3D::IDENTITY;
        want.set_translation(&crate::ww::vector3::Vector3::new(2.0, 1.0, 0.0));
        want.post_mul(&rot_y(2.0));
        assert!((0..3).all(|r| (0..4).all(|c| (posed.row[r][c] - want.row[r][c]).abs() < 1e-5)), "{posed:?} {want:?}");
    }

    /// The level `LocoFacts` of the wheel tests: stiffness 0.1, damping 0.4, axial damping 1, acceleration coefficients
    /// 0.5, pitch limits 0.5, no bounce, suspension with extension -2.3, wheel turn angle 0.4.
    fn wheels() -> LocoFacts {
        LocoFacts {
            appearance: LocomotorAppearance::LocoWheelsFour,
            accel_pitch_limit: 0.5,
            decel_pitch_limit: 0.5,
            bounce_kick: 0.0,
            pitch_stiffness: 0.1,
            roll_stiffness: 0.1,
            pitch_damping: 0.4,
            roll_damping: 0.4,
            pitch_by_z_vel_coef: 0.0,
            thrust_roll: 0.0,
            wobble_rate: 0.0,
            max_wobble: 0.0,
            min_wobble: 0.0,
            forward_vel_coef: 0.0,
            lateral_vel_coef: 0.0,
            forward_accel_coef: 0.5,
            lateral_accel_coef: 0.5,
            uniform_axial_damping: 1.0,
            has_suspension: true,
            max_wheel_extension: -2.3,
            wheel_turn_angle: 0.4,
            rudder_correction_degree: 0.0,
            rudder_correction_rate: 0.0,
            elevator_correction_degree: 0.0,
            elevator_correction_rate: 0.0,
            is_moving_backwards: false,
        }
    }

    /// A truck at rest on level ground facing +x, speed `v`, turning `turning`, forward speed `fwd`, acceleration `ax`.
    fn on_level(v: f32, turning: i32, fwd: f32, ax: f32, motive: bool) -> XformObjectFacts {
        XformObjectFacts {
            position: Coord3D::default(),
            dir: Coord3D { x: 1.0, y: 0.0, z: 0.0 },
            accel: Coord3D { x: ax, y: 0.0, z: 0.0 },
            vel: Coord3D { x: fwd, y: 0.0, z: 0.0 },
            velocity_magnitude: v,
            is_motive: motive,
            turning,
            forward_speed_2d: fwd,
            layer_height: 0.0,
            normal: Coord3D { x: 0.0, y: 0.0, z: 1.0 },
            airborne: false,
            major_radius: 5.0,
            minor_radius: 2.0,
            bounding_circle_radius: 5.0,
            cur_locomotor_speed: 4.0,
            overlap: None,
            had_previous_overlap: false,
        }
    }

    /// *Predictions, written before the first run* (wheel turn angle 0.4, `WHEEL_SMOOTHNESS` 10; cab multiplier 2,
    /// trailer multiplier 1, damping 0.5):
    /// - turning left (`TURN_POSITIVE`) at rest, `calcPhysicsXformWheels` moves the wheel angle a tenth of the way to 0.4
    ///   each draw: 0.04, then 0.04 + 0.036 = 0.076 (in f32, to within 1e-7); the suspension offsets stay 0 (level
    ///   ground, no pitch);
    /// - the truck's front tires are steered by it: their control turns about Z by 0.076 (no spin at rest), the rear
    ///   tires' control is the identity (`Rotate_Y(0)`);
    /// - turning right (`TURN_NEGATIVE`) while moving backwards (`getForwardSpeed2D` < 0) steers left too: the target is
    ///   +0.4 again;
    /// - the cab follows the wheel angle times 2, half the remaining way each draw (0.5 damping): with the wheel angle
    ///   0.1, 0.1 then 0.15; the trailer the other way: -0.05, then -0.075; with the AI path's point 0.05 to the left
    ///   the cab's goal is clamped to 0.05.
    ///
    /// *Results (2026-11-06), first run:* all held (wheel angle 0.04 and 0.076 within 1e-7, offsets exactly 0; the
    /// front controls exactly `Rotate_Z(0.076)`, the rear exactly the identity; cab 0.1, 0.15 and trailer -0.05, -0.075
    /// within 1e-7; the clamp to the path's point as predicted).
    #[test]
    fn front_wheels_steer_while_turning() {
        let l = wheels();
        let mut li = DrawableLocoInfo::default();
        let mut random = RandomValues::new(0);
        let mut info = PhysicsXformInfo::default();
        li.calc_wheels(&l, Some(&on_level(0.0, TURN_POSITIVE, 0.0, 0.0, false)), &mut random, &mut info);
        assert!((li.wheel_info.wheel_angle - 0.04).abs() < 1e-7, "{}", li.wheel_info.wheel_angle);
        li.calc_wheels(&l, Some(&on_level(0.0, TURN_POSITIVE, 0.0, 0.0, false)), &mut random, &mut info);
        let angle = li.wheel_info.wheel_angle;
        assert!((angle - 0.076).abs() < 1e-7, "{angle}");
        let w = li.wheel_info;
        assert_eq!([w.front_left_height_offset, w.front_right_height_offset, w.rear_left_height_offset, w.rear_right_height_offset], [0.0; 4]);

        let mut rig = Rig::new();
        let mut m = rig.draw("W3DTruckDraw", "TRUCKT", None, TIRES);
        rig.frame(&mut m, &facts(Some(motion(0.0, false, None)), Some(w)));
        let front = ctl(&m, FL).unwrap();
        assert!((front.get_z_rotation() - angle).abs() < 1e-6, "{}", front.get_z_rotation());
        assert_eq!(front, {
            let mut x = Matrix3D::IDENTITY;
            x.adjust_z_translation(0.0);
            x.rotate_z(angle);
            x.rotate_y(0.0);
            x
        });
        assert_eq!(ctl(&m, FR).unwrap().get_z_rotation(), front.get_z_rotation());
        assert_eq!(ctl(&m, RL), Some(rot_y(0.0)));
        assert_eq!(ctl(&m, RL).unwrap().get_z_rotation(), 0.0);

        let mut back = DrawableLocoInfo::default();
        back.calc_wheels(&l, Some(&on_level(1.0, TURN_NEGATIVE, -1.0, 0.0, false)), &mut random, &mut info);
        assert!((back.wheel_info.wheel_angle - 0.04).abs() < 1e-7);

        let cab = [(0, "CabBone", "Cab"), (0, "TrailerBone", "Trailer"), (0, "CabRotationMultiplier", "2"), (0, "TrailerRotationMultiplier", "1"), (0, "RotationDamping", "0.5")];
        let mut m = rig.draw("W3DTruckDraw", "TRUCKT", None, &[TIRES, &cab].concat());
        let w = TWheelInfo { wheel_angle: 0.1, ..TWheelInfo::default() };
        rig.frame(&mut m, &facts(Some(motion(0.0, false, None)), Some(w)));
        let t = truck(&m);
        assert_eq!((t.cab_bone, t.trailer_bone), (CAB, TRAILER));
        assert!((t.cur_cab_rotation - 0.1).abs() < 1e-7 && (t.cur_trailer_rotation + 0.05).abs() < 1e-7);
        rig.frame(&mut m, &facts(Some(motion(0.0, false, None)), Some(w)));
        let t = truck(&m);
        assert!((t.cur_cab_rotation - 0.15).abs() < 1e-7 && (t.cur_trailer_rotation + 0.075).abs() < 1e-7, "{} {}", t.cur_cab_rotation, t.cur_trailer_rotation);
        assert_eq!(ctl(&m, CAB), Some(rot_z(t.cur_cab_rotation)));
        assert_eq!(ctl(&m, TRAILER), Some(rot_z(t.cur_trailer_rotation)));
        let before = t.cur_cab_rotation;
        rig.frame(&mut m, &facts(Some(motion(0.0, false, Some(0.05))), Some(w)));
        assert!((truck(&m).cur_cab_rotation - (before + (0.05 - before) * 0.5)).abs() < 1e-7);
    }

    /// *Predictions, written before the first run* (stiffness 0.1, damping 0.4, forward acceleration coefficient 0.5,
    /// major radius 5, level ground, motive, accelerating forward at 1):
    /// - first draw: the totals are taken before the kick, so pitch 0 and Z 0; the kick leaves the acceleration pitch
    ///   rate at -0.5;
    /// - second draw: the rate damps to -0.5 + 0.4 * 0.5 = -0.3 and the acceleration pitch takes it: total pitch -0.3
    ///   (within an ulp), the nose up (`Rotate_Y` of a negative angle); the pitch height 5 sin(-0.3) = -1.4776 is under
    ///   0, so the front wheels drop (new offset 0.9 * (h/3 + h/2) = -1.108, half of it this draw: -0.554) and the rear
    ///   rise (-h/2 + h/4 = 0.369, at once); the total Z is |h|/4 = 0.369 (0.3 rad is under π/8, divisor 4);
    /// - the transform `applyPhysicsXform` puts on the model is translated by that Z;
    /// - nothing random was drawn (speed 0, no bounce).
    ///
    /// *Results (2026-11-06), first run:* all held: totals 0 then pitch -0.3 within 1e-7; front offsets -0.554 and rear
    /// 0.369 within 1e-3, Z 0.369 within 1e-3; the generator untouched.
    #[test]
    fn suspension_pitches_under_acceleration() {
        let l = wheels();
        let mut li = DrawableLocoInfo::default();
        let mut random = RandomValues::new(7);
        let untouched = random.clone();
        let o = on_level(0.0, TURN_NONE, 0.0, 1.0, true);
        let mut info = PhysicsXformInfo::default();
        li.calc_wheels(&l, Some(&o), &mut random, &mut info);
        assert_eq!((info.total_pitch, info.total_z), (0.0, 0.0));
        assert_eq!(li.acceleration_pitch_rate, -0.5);
        let mut info = PhysicsXformInfo::default();
        li.calc_wheels(&l, Some(&o), &mut random, &mut info);
        assert!((info.total_pitch + 0.3).abs() < 1e-7, "{}", info.total_pitch);
        let h = 5.0 * crate::common::trig::sin(info.total_pitch);
        assert!((h + 1.4776).abs() < 1e-4, "{h}");
        let w = li.wheel_info;
        assert!((w.front_left_height_offset + 0.554).abs() < 1e-3 && w.front_left_height_offset == w.front_right_height_offset, "{w:?}");
        assert!((w.rear_left_height_offset - 0.369).abs() < 1e-3 && w.rear_left_height_offset == w.rear_right_height_offset, "{w:?}");
        assert!((info.total_z - 0.369).abs() < 1e-3, "{}", info.total_z);
        assert_eq!(info.local_matrix().get_z_translation(), info.total_z);
        assert_eq!(format!("{random:?}"), format!("{untouched:?}"));
    }

    /// *Predictions, written before the first run* (bounce kick 0.2, speed 2 of the locomotor's 4): above a tenth of the
    /// locomotor's speed with the rates at rest, one `GameClientRandomValue(0, 3)` is drawn from the client's generator
    /// and kicks the pitch rate by ±0.2 * 0.5 = ±0.1 and the roll rate by ±0.05 (case 0: -, -; 1: +, -; 2: -, +; 3: +, +),
    /// before the springs: the pitch rate is then that, damped (×0.6, and halved when positive), the roll rate ×0.6.
    ///
    /// *Results (2026-11-06), first run:* held: one draw from the generator, the rates exactly as the drawn case gives.
    #[test]
    fn the_bounce_draws_the_client_s_random() {
        let l = LocoFacts { bounce_kick: 0.2, ..wheels() };
        let mut random = RandomValues::new(3);
        let mut expect = random.clone();
        let case = expect.game_client_random_value(0, 3);
        let mut li = DrawableLocoInfo::default();
        let mut info = PhysicsXformInfo::default();
        li.calc_wheels(&l, Some(&on_level(2.0, TURN_NONE, 2.0, 0.0, false)), &mut random, &mut info);
        assert_eq!(format!("{random:?}"), format!("{expect:?}"));
        let (p, r): (f32, f32) = match case {
            0 => (-0.1, -0.05),
            1 => (0.1, -0.05),
            2 => (-0.1, 0.05),
            _ => (0.1, 0.05),
        };
        let mut pr = p + (-0.4 * p);
        if pr > 0.0 {
            pr *= 0.5;
        }
        assert_eq!(li.pitch_rate, pr, "case {case}");
        assert_eq!(li.roll_rate, r + (-0.4 * r), "case {case}");
    }

    /// *Predictions, written before the first run* (rung 3, `W3DTankTruckDraw`, `TreadAnimationRate` 3 a second =
    /// 0.10000001 a draw, drive fraction 0.3):
    /// - its first draw finds the four tire bones and two treads, both middle treads (the tank truck's switch on `L`/`R`
    ///   is commented out upstream), offsets 0;
    /// - at speed 2 (half its locomotor's 4) under a motive force the tires spin as the truck's (rotation 1 a draw) and
    ///   each tread's U goes down by the rate, wrapped: 0.9 after one draw; the render object's material overrides
    ///   carry it;
    /// - a new render object (`DAMAGED`, `TRUCKU`, no treads) clears the bones and finds them again, with no treads.
    ///
    /// *Results (2026-11-06), first run:* all held (treads on sub-objects 7 and 8, both middle, U exactly the wrapped
    /// step; the remade object's bones found again and no treads).
    #[test]
    fn the_tank_truck_spins_its_tires_and_scrolls_its_treads() {
        let mut rig = Rig::new();
        let mut l = TIRES.to_vec();
        l.push((0, "TreadAnimationRate", "3"));
        let mut m = rig.draw("W3DTankTruckDraw", "TANKRU", None, &l);
        rig.frame(&mut m, &facts(Some(motion(2.0, false, None)), Some(TWheelInfo::default())));
        let Some(TruckKind::TankTruck(t)) = &m.truck else { panic!("a tank truck") };
        assert_eq!((t.front_left_tire_bone, t.front_right_tire_bone, t.rear_left_tire_bone, t.rear_right_tire_bone), (FL, FR, RL, RR));
        assert_eq!(t.front_wheel_rotation, 1.0);
        assert_eq!(t.treads.iter().map(|x| (x.sub_object, x.kind)).collect::<Vec<_>>(), vec![(7, TreadType::Middle), (8, TreadType::Middle)]);
        let rate = 3.0f32 * crate::common::game_common::SECONDS_PER_LOGICFRAME_REAL;
        let mut want = -rate;
        want -= crate::ww::wwmath::floor(want);
        assert!(t.treads.iter().all(|x| x.custom_uv_offset == [want, 0.0]));
        assert_eq!(m.render_object.as_ref().unwrap().material_overrides.get(&7), Some(&[want, 0.0]));
        assert_eq!(ctl(&m, FL), Some(rot_y(1.0)));

        rig.set_flags(&mut m, &[MODELCONDITION_DAMAGED]);
        let Some(TruckKind::TankTruck(t)) = &m.truck else { panic!("a tank truck") };
        assert_eq!(m.render_object.as_ref().unwrap().model_name, b"trucku");
        assert_eq!((t.front_left_tire_bone, t.rear_right_tire_bone), (FL, RR));
        assert!(t.treads.is_empty());
    }

    /// *Predictions, written before the first run* (rung 3, `W3DPoliceCarDraw` of `COPCAR` playing its 4-frame
    /// `COPCAR.LIGHTS`):
    /// - the constructor draws `m_curFrame` from the client's generator after the base class's draws: the generator's
    ///   next real in [0, 10);
    /// - each draw adds 0.25 and sets the animation to it in `MANUAL` mode, back to 0 once past frame 3;
    /// - the truck's own part runs after it: the tires spin.
    ///
    /// *Results (2026-11-06), first run:* all held (each frame exactly the predicted one, in `MANUAL`; tires at 3).
    #[test]
    fn the_police_car_steps_its_lights_and_drives_as_a_truck() {
        let mut rig = Rig::new();
        let mut m = rig.draw("W3DPoliceCarDraw", "COPCAR", Some("COPCAR.LIGHTS"), TIRES);
        let Some(TruckKind::PoliceCar(_, f0)) = m.truck else { panic!("a police car") };
        assert!((0.0..10.0).contains(&f0), "{f0}");
        let mut f = f0;
        for _ in 0..3 {
            rig.frame(&mut m, &facts(Some(motion(2.0, false, None)), Some(TWheelInfo::default())));
            f += 0.25;
            if f > 3.0 {
                f = 0.0;
            }
            let Some(TruckKind::PoliceCar(_, cur)) = m.truck else { panic!() };
            assert_eq!(cur, f);
            let a = m.render_object.as_ref().unwrap().anim.as_ref().unwrap();
            assert_eq!((a.frame, a.anim_mode), (f, AnimMode::Manual));
        }
        assert_eq!(truck(&m).front_wheel_rotation, 3.0);
    }
}
