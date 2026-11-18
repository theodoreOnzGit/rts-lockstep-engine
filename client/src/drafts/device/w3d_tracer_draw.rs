//! UNTESTED DRAFT (#202, #205): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTracerDraw.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DTracerDraw.h
//!   (Line3DClass is WW3D2/Line3D.cpp, not in the port: [`Line3D`] below keeps the state the tracer reads and sets.)

use super::device_support::{draw_module_xfer, DrawableFacts};
use rts_lockstep_engine::common::base_type::RGBColor;
use rts_lockstep_engine::common::xfer::{Xfer, XferResult};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;

/// The state of a `Line3DClass` (Line3D.cpp) the draw modules read and set: `Reset`, `Re_Color`, `Set_Opacity`,
/// `Set_Transform`, `Get_Transform`. Its mesh (a camera-facing quad strip) is device-only, not translated.
#[derive(Clone, Debug)]
pub struct Line3D {
    pub start: Vector3,
    pub end: Vector3,
    pub width: f32,
    pub color: RGBColor,
    pub opacity: f32,
    pub transform: Matrix3D,
}

impl Line3D {
    /// `Line3DClass::Line3DClass(start, end, width, r, g, b, transparency)`.
    pub fn new(start: Vector3, end: Vector3, width: f32, r: f32, g: f32, b: f32, transparency: f32) -> Self {
        let mut transform = Matrix3D::default();
        transform.make_identity();
        Line3D { start, end, width, color: RGBColor { red: r, green: g, blue: b }, opacity: transparency, transform }
    }
    /// `Line3DClass::Reset(start, end, width)`: these calls nuke the internal transform (set to identity).
    pub fn reset(&mut self, start: Vector3, end: Vector3, width: Option<f32>) {
        self.start = start;
        self.end = end;
        if let Some(w) = width {
            self.width = w;
        }
        self.transform.make_identity();
    }
    /// `Line3DClass::Re_Color`.
    pub fn re_color(&mut self, r: f32, g: f32, b: f32) {
        self.color = RGBColor { red: r, green: g, blue: b };
    }
    /// `Line3DClass::Set_Opacity`.
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity;
    }
    /// `RenderObjClass::Set_Transform`.
    pub fn set_transform(&mut self, m: &Matrix3D) {
        self.transform = *m;
    }
    /// `RenderObjClass::Get_Transform`.
    pub fn get_transform(&self) -> Matrix3D {
        self.transform
    }
}

/// `class W3DTracerDraw`.
#[derive(Clone, Debug)]
pub struct W3DTracerDraw {
    /// `m_theTracer` (None for `NULL`; in the scene whenever Some).
    pub the_tracer: Option<Line3D>,
    /// `m_length`, `m_width`, `m_color`, `m_speedInDistPerFrame`, `m_opacity`.
    pub length: f32,
    pub width: f32,
    pub color: RGBColor,
    pub speed_in_dist_per_frame: f32,
    pub opacity: f32,
}

impl W3DTracerDraw {
    /// `W3DTracerDraw::W3DTracerDraw` (W3DTracerDraw.cpp:53).
    pub fn new() -> Self {
        W3DTracerDraw {
            opacity: 1.0,
            length: 20.0,
            width: 0.5,
            color: RGBColor { red: 0.9, green: 0.8, blue: 0.7 },
            speed_in_dist_per_frame: 1.0,
            the_tracer: None,
        }
    }

    /// `W3DTracerDraw::setTracerParms` (W3DTracerDraw.cpp:71).
    pub fn set_tracer_parms(&mut self, speed: f32, length: f32, width: f32, color: &RGBColor, initial_opacity: f32, drawable: &DrawableFacts) {
        self.speed_in_dist_per_frame = speed;
        self.length = length;
        self.width = width;
        self.color = *color;
        self.opacity = initial_opacity;
        if let Some(tracer) = self.the_tracer.as_mut() {
            let start = Vector3::new(0.0, 0.0, 0.0);
            let stop = Vector3::new(self.length, 0.0, 0.0);
            tracer.reset(start, stop, Some(self.width));
            tracer.re_color(self.color.red, self.color.green, self.color.blue);
            tracer.set_opacity(self.opacity);
            // these calls nuke the internal transform, so re-set it here
            tracer.set_transform(&drawable.transform);
        }
    }

    /// `W3DTracerDraw::~W3DTracerDraw` (W3DTracerDraw.cpp:90): the tracer out of the scene and released.
    pub fn destroy(&mut self) {
        // upstream: W3DDisplay::m_3DScene->Remove_Render_Object(m_theTracer) — device-only, not translated
        self.the_tracer = None;
    }

    /// `W3DTracerDraw::reactToTransformChange` (W3DTracerDraw.cpp:101).
    pub fn react_to_transform_change(&mut self, drawable: &DrawableFacts) {
        if let Some(tracer) = self.the_tracer.as_mut() {
            tracer.set_transform(&drawable.transform);
        }
    }

    /// `W3DTracerDraw::doDrawModule` (W3DTracerDraw.cpp:111). `expiration_date`: `getDrawable()->getExpirationDate()`;
    /// `logic_frame`: `TheGameLogic->getFrame()`.
    pub fn do_draw_module(&mut self, transform_mtx: &Matrix3D, expiration_date: u32, logic_frame: u32) {
        // create tracer
        if self.the_tracer.is_none() {
            let start = Vector3::new(0.0, 0.0, 0.0);
            let stop = Vector3::new(self.length, 0.0, 0.0);

            // create tracer render object for us to manipulate
            let mut tracer = Line3D::new(start, stop, self.width, self.color.red, self.color.green, self.color.blue, self.opacity);
            // upstream: W3DDisplay::m_3DScene->Add_Render_Object(m_theTracer) — device-only, not translated

            // set the transform for the tracer to that of the drawable
            tracer.set_transform(transform_mtx);
            self.the_tracer = Some(tracer);
        }

        let tracer = self.the_tracer.as_mut().unwrap();
        let exp_date = expiration_date;
        if exp_date != 0 {
            // (UnsignedInt)(expDate - frame) converted to Real: unsigned subtraction, as upstream
            let decay = self.opacity / (exp_date.wrapping_sub(logic_frame)) as f32;
            self.opacity -= decay;
            tracer.set_opacity(self.opacity);
        }

        // set the position for the tracer
        if self.speed_in_dist_per_frame != 0.0 {
            let mut pos = tracer.get_transform();
            pos.translate_v(&Vector3::new(self.speed_in_dist_per_frame, 0.0, 0.0));
            tracer.set_transform(&pos);
        }
    }

    /// `W3DTracerDraw::crc` (W3DTracerDraw.cpp:157).
    pub fn crc(&self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `W3DTracerDraw::xfer` (W3DTracerDraw.cpp:169): version 1; no data of its own.
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)?;
        draw_module_xfer(xfer)
    }

    /// `W3DTracerDraw::loadPostProcess` (W3DTracerDraw.cpp:186).
    pub fn load_post_process(&mut self) {}
}

impl Default for W3DTracerDraw {
    fn default() -> Self {
        Self::new()
    }
}
