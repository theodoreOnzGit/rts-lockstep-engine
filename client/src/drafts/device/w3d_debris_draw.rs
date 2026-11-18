//! UNTESTED DRAFT (#202, #205, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DDebrisDraw.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DDebrisDraw.h

use super::device_support::{draw_module_xfer, DrawableFacts, HAnimHandle, RenderObjHandle, ShadowHandle};
use rts_lockstep_engine::common::xfer::{Color, Xfer, XferMode, XferResult};
use rts_lockstep_engine::formats::w3d::render::AnimMode;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;

/// `W3DDebrisDraw::AnimStateType`.
pub const INITIAL: i32 = 0;
pub const FLYING: i32 = 1;
pub const FINAL: i32 = 2;
pub const STATECOUNT: usize = 3;

/// `ShadowType` `SHADOW_NONE` (Shadow.h).
pub const SHADOW_NONE: u8 = 0;

/// `FXList*`: the id of the FX list the final state plays (None for `NULL`).
pub type FxListHandle = Option<u32>;

/// What `W3DDebrisDraw` asks of the device: the asset manager (`W3DDisplay::m_assetManager`), the scene
/// (`W3DDisplay::m_3DScene`), the shadow manager (`TheW3DShadowManager`), the render object's animation and
/// `FXList::doFXPos`. One implementation per backend.
pub trait DebrisDrawDevice {
    /// `Create_Render_Obj(name, scale, hexColor)`: 0 when the model is not found.
    fn create_render_obj(&mut self, name: &[u8], scale: f32, hex_color: i32) -> RenderObjHandle;
    /// `Add_Render_Object` / `Remove_Render_Object` + `REF_PTR_RELEASE`.
    fn add_render_object(&mut self, obj: RenderObjHandle);
    fn remove_render_object(&mut self, obj: RenderObjHandle);
    /// `Set_Transform`, `Set_ObjectScale`.
    fn set_transform(&mut self, obj: RenderObjHandle, transform: &Matrix3D);
    fn set_object_scale(&mut self, obj: RenderObjHandle, scale: f32);
    /// `TheW3DShadowManager->addShadow(obj, &shadowInfo)` (type `t`, sizes 0), `removeShadow`,
    /// `enableShadowRender`, `enableShadowInvisible`.
    fn add_shadow(&mut self, obj: RenderObjHandle, shadow_type: u8) -> ShadowHandle;
    fn remove_shadow(&mut self, shadow: ShadowHandle);
    fn enable_shadow_render(&mut self, shadow: ShadowHandle, enable: bool);
    fn enable_shadow_invisible(&mut self, shadow: ShadowHandle, invisible: bool);
    /// `Get_HAnim(name)` (0 when not found) and `REF_PTR_RELEASE`.
    fn get_hanim(&mut self, name: &[u8]) -> HAnimHandle;
    fn release_hanim(&mut self, anim: HAnimHandle);
    /// `r->Class_ID() == CLASSID_HLOD` and `hlod->Is_Animation_Complete()`; true for any other class.
    fn is_animation_complete(&mut self, obj: RenderObjHandle) -> bool;
    /// `m_renderObject->Peek_Animation()`.
    fn peek_animation(&mut self, obj: RenderObjHandle) -> HAnimHandle;
    /// `m_renderObject->Set_Animation(hanim, 0, mode)`.
    fn set_animation(&mut self, obj: RenderObjHandle, anim: HAnimHandle, frame: f32, mode: AnimMode);
    /// `FXList::doFXPos(fx, pos, mtx, 0, NULL, 0.0f)`.
    fn do_fx_pos(&mut self, fx: u32, drawable: &DrawableFacts);
}

/// `class W3DDebrisDraw`.
#[derive(Clone, Debug)]
pub struct W3DDebrisDraw {
    /// `m_modelName`, `m_modelColor`.
    pub model_name: Vec<u8>,
    pub model_color: Color,
    /// `m_animInitial`, `m_animFlying`, `m_animFinal`.
    pub anim_initial: Vec<u8>,
    pub anim_flying: Vec<u8>,
    pub anim_final: Vec<u8>,
    /// `m_renderObject`.
    pub render_object: RenderObjHandle,
    /// `m_anims[STATECOUNT]`.
    pub anims: [HAnimHandle; STATECOUNT],
    /// `m_fxFinal`.
    pub fx_final: FxListHandle,
    /// `m_state`, `m_frames`, `m_finalStop`.
    pub state: i32,
    pub frames: i32,
    pub final_stop: bool,
    /// `m_shadow`.
    pub shadow: ShadowHandle,
}

/// `isNearlyZero(vel)` (W3DDebrisDraw.cpp:182): static and unused upstream, kept for completeness.
pub fn is_nearly_zero(x: f32, y: f32, z: f32) -> bool {
    const TINY: f32 = 0.01;
    x.abs() < TINY && y.abs() < TINY && z.abs() < TINY
}

impl W3DDebrisDraw {
    /// `W3DDebrisDraw::W3DDebrisDraw` (W3DDebrisDraw.cpp:47).
    pub fn new() -> Self {
        W3DDebrisDraw {
            model_name: Vec::new(),
            model_color: 0,
            anim_initial: Vec::new(),
            anim_flying: Vec::new(),
            anim_final: Vec::new(),
            render_object: RenderObjHandle(0),
            anims: [HAnimHandle(0); STATECOUNT],
            fx_final: None,
            state: INITIAL,
            frames: 0,
            final_stop: false,
            shadow: ShadowHandle(0),
        }
    }

    /// `W3DDebrisDraw::~W3DDebrisDraw` (W3DDebrisDraw.cpp:60).
    pub fn destroy(&mut self, dev: &mut dyn DebrisDrawDevice) {
        if self.shadow != ShadowHandle(0) {
            dev.remove_shadow(self.shadow);
            self.shadow = ShadowHandle(0);
        }
        if !self.render_object.is_null() {
            dev.remove_render_object(self.render_object);
            self.render_object = RenderObjHandle(0);
        }
        for i in 0..STATECOUNT {
            dev.release_hanim(self.anims[i]);
            self.anims[i] = HAnimHandle(0);
        }
    }

    /// `W3DDebrisDraw::setShadowsEnabled` (W3DDebrisDraw.cpp:85).
    pub fn set_shadows_enabled(&mut self, enable: bool, dev: &mut dyn DebrisDrawDevice) {
        if self.shadow != ShadowHandle(0) {
            dev.enable_shadow_render(self.shadow, enable);
        }
    }

    /// `W3DDebrisDraw::setFullyObscuredByShroud` (W3DDebrisDraw.cpp:92).
    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool, dev: &mut dyn DebrisDrawDevice) {
        if self.shadow != ShadowHandle(0) {
            dev.enable_shadow_invisible(self.shadow, fully_obscured);
        }
    }

    /// `W3DDebrisDraw::setModelName` (W3DDebrisDraw.cpp:100).
    pub fn set_model_name(&mut self, name: &[u8], color: Color, t: u8, drawable: &DrawableFacts, dev: &mut dyn DebrisDrawDevice) {
        if self.render_object.is_null() && !name.is_empty() {
            let mut hex_color = 0;
            if color != 0 {
                hex_color = color | 0xFF00_0000u32 as i32;
            }
            self.render_object = dev.create_render_obj(name, drawable.scale, hex_color);
            // DEBUG_ASSERTCRASH(m_renderObject, ("Debris model %s not found!\n",name.str()));
            if !self.render_object.is_null() {
                dev.add_render_object(self.render_object);
                // upstream: m_renderObject->Set_User_Data(getDrawable()->getDrawableInfo()) — device-only, not translated
                let mut transform = Matrix3D::default();
                transform.make_identity(); // transform.Set(Vector3(0,0,0))
                dev.set_transform(self.render_object, &transform);
            }

            if t != SHADOW_NONE {
                self.shadow = dev.add_shadow(self.render_object, t);
            } else if self.shadow != ShadowHandle(0) {
                dev.remove_shadow(self.shadow);
                self.shadow = ShadowHandle(0);
            }

            // save the model name and color
            self.model_name = name.to_vec();
            self.model_color = color;
        }
    }

    /// `W3DDebrisDraw::setAnimNames` (W3DDebrisDraw.cpp:147).
    pub fn set_anim_names(&mut self, initial: &[u8], flying: &[u8], final_name: &[u8], final_fx: FxListHandle, dev: &mut dyn DebrisDrawDevice) {
        for i in 0..STATECOUNT {
            dev.release_hanim(self.anims[i]);
            self.anims[i] = HAnimHandle(0);
        }

        self.anims[INITIAL as usize] = if initial.is_empty() { HAnimHandle(0) } else { dev.get_hanim(initial) };
        self.anims[FLYING as usize] = if flying.is_empty() { HAnimHandle(0) } else { dev.get_hanim(flying) };
        let mut final_name = final_name.to_vec();
        if final_name.eq_ignore_ascii_case(b"STOP") {
            self.final_stop = true;
            final_name = flying.to_vec();
        } else {
            self.final_stop = false;
        }
        self.anims[FINAL as usize] = if final_name.is_empty() { HAnimHandle(0) } else { dev.get_hanim(&final_name) };
        self.state = 0;
        self.frames = 0;
        self.fx_final = final_fx;

        self.anim_initial = initial.to_vec();
        self.anim_flying = flying.to_vec();
        self.anim_final = final_name;
    }

    /// `W3DDebrisDraw::reactToTransformChange` (W3DDebrisDraw.cpp:202).
    pub fn react_to_transform_change(&mut self, drawable: &DrawableFacts, dev: &mut dyn DebrisDrawDevice) {
        if !self.render_object.is_null() {
            dev.set_transform(self.render_object, &drawable.transform);
        }
    }

    /// `W3DDebrisDraw::doDrawModule` (W3DDebrisDraw.cpp:212).
    pub fn do_draw_module(&mut self, transform_mtx: &Matrix3D, drawable: &DrawableFacts, dev: &mut dyn DebrisDrawDevice) {
        if !self.render_object.is_null() {
            let mut scaled_transform;
            let mut transform_mtx = transform_mtx;
            if drawable.instance_scale != 1.0 {
                //do custom scaling of the W3D model.
                scaled_transform = *transform_mtx;
                scaled_transform.scale(drawable.instance_scale);
                transform_mtx = &scaled_transform;
                dev.set_object_scale(self.render_object, drawable.instance_scale);
            }
            dev.set_transform(self.render_object, transform_mtx);

            // static const RenderObjClass::AnimMode TheAnimModes[STATECOUNT]
            const THE_ANIM_MODES: [AnimMode; STATECOUNT] = [AnimMode::Once, AnimMode::Loop, AnimMode::Once];

            let old_state = self.state;
            const MIN_FINAL_FRAMES: i32 = 3;
            if self.state != FINAL && drawable.has_object && !drawable.object_is_above_terrain && self.frames > MIN_FINAL_FRAMES {
                self.state = FINAL;
            } else if self.state < FINAL && dev.is_animation_complete(self.render_object) {
                self.state += 1;
            }
            let hanim = self.anims[self.state as usize];
            if hanim != HAnimHandle(0) && (hanim != dev.peek_animation(self.render_object) || old_state != self.state) {
                let mut m = THE_ANIM_MODES[self.state as usize];
                if self.state == FINAL {
                    if let Some(fx) = self.fx_final {
                        dev.do_fx_pos(fx, drawable); // FXList::doFXPos(m_fxFinal, getPosition(), getTransformMatrix(), 0, NULL, 0.0f)
                    }
                    if self.final_stop {
                        m = AnimMode::Manual;
                    }
                }
                dev.set_animation(self.render_object, hanim, 0.0, m);
            }
            self.frames += 1;
        }
    }

    /// `W3DDebrisDraw::crc` (W3DDebrisDraw.cpp:262): `DrawModule::crc` adds nothing.
    pub fn crc(&self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `W3DDebrisDraw::xfer` (W3DDebrisDraw.cpp:274): version 1.
    pub fn xfer(&mut self, xfer: &mut dyn Xfer, drawable: &DrawableFacts, dev: &mut dyn DebrisDrawDevice) -> XferResult<()> {
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)?;
        draw_module_xfer(xfer)?;

        xfer.xfer_ascii_string(&mut self.model_name)?;
        xfer.xfer_color(&mut self.model_color)?;

        // set the model and color
        if xfer.get_xfer_mode() == XferMode::Load {
            let (name, color) = (self.model_name.clone(), self.model_color);
            self.set_model_name(&name, color, SHADOW_NONE, drawable, dev);
        }

        xfer.xfer_ascii_string(&mut self.anim_initial)?;
        xfer.xfer_ascii_string(&mut self.anim_flying)?;
        xfer.xfer_ascii_string(&mut self.anim_final)?;

        // when loading, set the animations
        if xfer.get_xfer_mode() == XferMode::Load {
            let (i, f, fi) = (self.anim_initial.clone(), self.anim_flying.clone(), self.anim_final.clone());
            self.set_anim_names(&i, &f, &fi, None, dev);
        }

        xfer.xfer_int(&mut self.state)?;
        xfer.xfer_int(&mut self.frames)?;
        xfer.xfer_bool(&mut self.final_stop)?;
        Ok(())
    }

    /// `W3DDebrisDraw::loadPostProcess` (W3DDebrisDraw.cpp:326): `DrawModule::loadPostProcess` does nothing.
    pub fn load_post_process(&mut self) {}
}

impl Default for W3DDebrisDraw {
    fn default() -> Self {
        Self::new()
    }
}
