//! UNTESTED DRAFT (#202, #205): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DDefaultDraw.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DDefaultDraw.h
//!
//! Upstream compiles the body under `LOAD_TEST_ASSETS`, which FileSystem.h defines as 1: the module draws the
//! template's "LTA" (load test asset) model if the template names one. The port's `ThingTemplate` has no LTA name in the
//! parsed INI (the INI key is not in the shipped data), so [`W3DDefaultDraw::new`] takes it as an argument.

use super::device_support::{draw_module_xfer, DrawableFacts, RenderObjHandle, ShadowHandle};
use rts_lockstep_engine::common::xfer::{Xfer, XferResult};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;

/// `class W3DDefaultDraw`.
#[derive(Clone, Debug, Default)]
pub struct W3DDefaultDraw {
    /// `m_renderObject`.
    pub render_object: RenderObjHandle,
    /// `m_shadow`.
    pub shadow: ShadowHandle,
    /// `m_renderObject`'s transform, as last set by `Set_Transform` (the scene's copy).
    pub render_transform: Matrix3D,
    /// `m_renderObject`'s `Set_ObjectScale`, as last set.
    pub render_object_scale: f32,
}

impl W3DDefaultDraw {
    /// `W3DDefaultDraw::W3DDefaultDraw` (W3DDefaultDraw.cpp:50). `lta_name`: `getTemplate()->getLTAName()`;
    /// `create_render_obj`: `W3DDisplay::m_assetManager->Create_Render_Obj(name, scale, 0)` (0 for a model not found,
    /// which upstream asserts on); `add_shadow`: `TheW3DShadowManager->addShadow(obj, &shadowInfo)` with
    /// `SHADOW_VOLUME` and the default sizes.
    pub fn new(
        drawable: &DrawableFacts,
        lta_name: &[u8],
        create_render_obj: &mut dyn FnMut(&[u8], f32, i32) -> RenderObjHandle,
        add_shadow: &mut dyn FnMut(RenderObjHandle) -> ShadowHandle,
    ) -> Self {
        let mut me = W3DDefaultDraw::default();
        if !lta_name.is_empty() {
            me.render_object = create_render_obj(lta_name, drawable.scale, 0);
            me.shadow = add_shadow(me.render_object);
            // DEBUG_ASSERTCRASH(m_renderObject, ("Test asset %s not found", ...));
            if !me.render_object.is_null() {
                // upstream: W3DDisplay::m_3DScene->Add_Render_Object(m_renderObject) — device-only, not translated
                // upstream: m_renderObject->Set_User_Data(getDrawable()->getDrawableInfo()) — device-only, not translated
                let mut transform = Matrix3D::default();
                transform.make_identity(); // transform.Set(Vector3(0,0,0)): the matrix at the origin
                me.render_transform = transform; // m_renderObject->Set_Transform(transform)
            }
        }
        me
    }

    /// `W3DDefaultDraw::~W3DDefaultDraw` (W3DDefaultDraw.cpp:100): the shadow removed, the render object out of the
    /// scene and released.
    pub fn destroy(&mut self, remove_shadow: &mut dyn FnMut(ShadowHandle)) {
        if self.shadow != ShadowHandle(0) {
            remove_shadow(self.shadow);
            self.shadow = ShadowHandle(0);
        }
        if !self.render_object.is_null() {
            // upstream: W3DDisplay::m_3DScene->Remove_Render_Object(m_renderObject) — device-only, not translated
            self.render_object = RenderObjHandle(0);
        }
    }

    /// `W3DDefaultDraw::reactToTransformChange` (W3DDefaultDraw.cpp:85): the render object follows the drawable's
    /// transform.
    pub fn react_to_transform_change(&mut self, drawable: &DrawableFacts) {
        if !self.render_object.is_null() {
            self.render_transform = drawable.transform; // m_renderObject->Set_Transform(*getTransformMatrix())
        }
    }

    /// `W3DDefaultDraw::setShadowsEnabled` (W3DDefaultDraw.cpp:122). `enable_shadow_render`:
    /// `m_shadow->enableShadowRender(enable)`.
    pub fn set_shadows_enabled(&mut self, enable: bool, enable_shadow_render: &mut dyn FnMut(ShadowHandle, bool)) {
        if self.shadow != ShadowHandle(0) {
            enable_shadow_render(self.shadow, enable);
        }
    }

    /// `W3DDefaultDraw::setFullyObscuredByShroud` (W3DDefaultDraw.cpp:131). `enable_shadow_invisible`:
    /// `m_shadow->enableShadowInvisible(fullyObscured)`.
    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool, enable_shadow_invisible: &mut dyn FnMut(ShadowHandle, bool)) {
        if self.shadow != ShadowHandle(0) {
            enable_shadow_invisible(self.shadow, fully_obscured);
        }
    }

    /// `W3DDefaultDraw::doDrawModule` (W3DDefaultDraw.cpp:140). Kept as upstream has it: with an instance scale other
    /// than 1 the render object gets only `Set_ObjectScale` (the scaled copy of the transform is made, then dropped,
    /// and `Set_Transform` is not called); otherwise `Set_Transform(*transformMtx)`.
    pub fn do_draw_module(&mut self, drawable: &DrawableFacts, transform_mtx: &Matrix3D) {
        if !self.render_object.is_null() {
            if drawable.instance_scale != 1.0 {
                //do custom scaling of the W3D model.
                let mut scaled_transform = *transform_mtx;
                scaled_transform.scale(drawable.instance_scale);
                let _ = scaled_transform; // transformMtx = &scaledTransform (never used again upstream)
                self.render_object_scale = drawable.instance_scale; // m_renderObject->Set_ObjectScale(...)
            } else {
                self.render_transform = *transform_mtx; // m_renderObject->Set_Transform(*transformMtx)
            }
        }
    }

    /// `W3DDefaultDraw::crc` (W3DDefaultDraw.cpp:166): `DrawModule::crc` adds nothing.
    pub fn crc(&self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `W3DDefaultDraw::xfer` (W3DDefaultDraw.cpp:178): version 1, then `DrawModule::xfer`.
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)?;
        draw_module_xfer(xfer)
    }

    /// `W3DDefaultDraw::loadPostProcess` (W3DDefaultDraw.cpp:194): `DrawModule::loadPostProcess` does nothing.
    pub fn load_post_process(&mut self) {}
}
