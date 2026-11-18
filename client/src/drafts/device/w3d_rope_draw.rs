//! UNTESTED DRAFT (#202, #205, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DRopeDraw.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DRopeDraw.h
//!
//! `m_wobbleRate` is not set by upstream's constructor (read by `doDrawModule` before `initRopeParms` would set it);
//! the draft starts it at 0, the value an allocator's zero fill gives.

use super::device_support::{draw_module_xfer, DrawableFacts};
use super::w3d_tracer_draw::Line3D;
use rts_lockstep_engine::common::base_type::{RGBColor, PI};
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::trig::{cos, sin};
use rts_lockstep_engine::common::xfer::{Xfer, XferMode, XferResult};
use rts_lockstep_engine::ww::vector3::Vector3;

/// `W3DRopeDraw::SegInfo`.
#[derive(Clone, Debug)]
pub struct SegInfo {
    pub line: Option<Line3D>,
    pub soft_line: Option<Line3D>,
    pub wobble_axis_x: f32,
    pub wobble_axis_y: f32,
}

/// `class W3DRopeDraw`.
#[derive(Clone, Debug)]
pub struct W3DRopeDraw {
    /// `m_segments`.
    pub segments: Vec<SegInfo>,
    pub cur_len: f32,
    pub max_len: f32,
    pub width: f32,
    pub color: RGBColor,
    pub cur_speed: f32,
    pub max_speed: f32,
    pub accel: f32,
    pub wobble_len: f32,
    pub wobble_amp: f32,
    pub wobble_rate: f32,
    pub cur_wobble_phase: f32,
    pub cur_z_offset: f32,
}

impl W3DRopeDraw {
    /// `W3DRopeDraw::W3DRopeDraw` (W3DRopeDraw.cpp:56).
    pub fn new() -> Self {
        let max_len = 1.0;
        W3DRopeDraw {
            cur_len: 0.0,
            max_len,
            width: 0.5,
            color: RGBColor { red: 0.0, green: 0.0, blue: 0.0 },
            cur_speed: 0.0,
            max_speed: 0.0,
            accel: 0.0,
            wobble_len: max_len, // huge
            wobble_amp: 0.0,
            wobble_rate: 0.0,
            segments: Vec::new(),
            cur_wobble_phase: 0.0,
            cur_z_offset: 0.0,
        }
    }

    /// `W3DRopeDraw::buildSegments` (W3DRopeDraw.cpp:74). `random`: `GameClientRandomValueReal`'s generator.
    pub fn build_segments(&mut self, drawable: &DrawableFacts, random: &mut RandomValues) {
        // DEBUG_ASSERTCRASH(m_segments.empty(), ("Hmmn, not empty"));
        self.segments.clear();

        let num_segs = (self.max_len / self.wobble_len).ceil() as i32;
        let each_len = self.max_len / num_segs as f32;
        let mut pos = drawable.position;
        let mut i = 0;
        while i < num_segs {
            let axis = random.game_client_random_value_real(0.0, 2.0 * PI);
            let wobble_axis_x = cos(axis);
            let wobble_axis_y = sin(axis);
            let line = Line3D::new(
                Vector3::new(pos.x, pos.y, pos.z),
                Vector3::new(pos.x, pos.y, pos.z + each_len),
                self.width * 0.5, // width
                self.color.red,
                self.color.green,
                self.color.blue,
                1.0, // transparency
            );
            let soft_line = Line3D::new(
                Vector3::new(pos.x, pos.y, pos.z),
                Vector3::new(pos.x, pos.y, pos.z + each_len),
                self.width, // width
                self.color.red,
                self.color.green,
                self.color.blue,
                0.5, // transparency
            );
            // upstream: W3DDisplay::m_3DScene->Add_Render_Object(info.line / info.softLine) — device-only, not translated
            self.segments.push(SegInfo { line: Some(line), soft_line: Some(soft_line), wobble_axis_x, wobble_axis_y });
            i += 1;
            pos.z += each_len;
        }
    }

    /// `W3DRopeDraw::tossSegments` (W3DRopeDraw.cpp:112): every line out of the scene and released.
    pub fn toss_segments(&mut self) {
        // upstream: Remove_Render_Object(it->line / it->softLine) — device-only, not translated
        self.segments.clear();
    }

    /// `W3DRopeDraw::initRopeParms` (W3DRopeDraw.cpp:134).
    pub fn init_rope_parms(&mut self, length: f32, width: f32, color: &RGBColor, wobble_len: f32, wobble_amp: f32, wobble_rate: f32, drawable: &DrawableFacts, random: &mut RandomValues) {
        self.max_len = if 1.0 > length { 1.0 } else { length }; // max(1.0f, length)
        self.cur_len = 0.0;
        self.width = width;
        self.color = *color;
        self.wobble_len = if self.max_len < wobble_len { self.max_len } else { wobble_len }; // min(m_maxLen, wobbleLen)
        self.wobble_amp = wobble_amp;
        self.wobble_rate = wobble_rate;
        self.cur_z_offset = 0.0;

        self.toss_segments();
        self.build_segments(drawable, random);
    }

    /// `W3DRopeDraw::setRopeCurLen` (W3DRopeDraw.cpp:151).
    pub fn set_rope_cur_len(&mut self, length: f32) {
        self.cur_len = length;
    }

    /// `W3DRopeDraw::setRopeSpeed` (W3DRopeDraw.cpp:158).
    pub fn set_rope_speed(&mut self, cur_speed: f32, max_speed: f32, accel: f32) {
        self.cur_speed = cur_speed;
        self.max_speed = max_speed;
        self.accel = accel;
    }

    /// `W3DRopeDraw::~W3DRopeDraw` (W3DRopeDraw.cpp:167).
    pub fn destroy(&mut self) {
        self.toss_segments();
    }

    /// `W3DRopeDraw::doDrawModule` (W3DRopeDraw.cpp:174).
    pub fn do_draw_module(&mut self, drawable: &DrawableFacts, random: &mut RandomValues) {
        if self.segments.is_empty() {
            self.build_segments(drawable, random);
        }

        if !self.segments.is_empty() {
            let deflection = sin(self.cur_wobble_phase) * self.wobble_amp;
            let pos = drawable.position;
            let mut start = Vector3::new(pos.x, pos.y, pos.z + self.cur_z_offset);
            let each_len = self.cur_len / self.segments.len() as f32;
            for it in self.segments.iter_mut() {
                let end = Vector3::new(pos.x + deflection * it.wobble_axis_x, pos.y + deflection * it.wobble_axis_y, start.z - each_len);
                if let Some(line) = it.line.as_mut() {
                    line.reset(start, end, None);
                }
                if let Some(soft_line) = it.soft_line.as_mut() {
                    soft_line.reset(start, end, None);
                }
                start = end;
            }
        }

        self.cur_wobble_phase += self.wobble_rate;
        if self.cur_wobble_phase > 2.0 * PI {
            self.cur_wobble_phase -= 2.0 * PI;
        }

        self.cur_z_offset += self.cur_speed;
        self.cur_speed += self.accel;
        if self.cur_speed > self.max_speed {
            self.cur_speed = self.max_speed;
        } else if self.cur_speed < -self.max_speed {
            self.cur_speed = -self.max_speed;
        }
    }

    /// `W3DRopeDraw::crc` (W3DRopeDraw.cpp:218).
    pub fn crc(&self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `W3DRopeDraw::xfer` (W3DRopeDraw.cpp:230): version 1; `m_segments` is not saved.
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)?;
        draw_module_xfer(xfer)?;

        xfer.xfer_real(&mut self.cur_len)?;
        xfer.xfer_real(&mut self.max_len)?;
        xfer.xfer_real(&mut self.width)?;
        xfer.xfer_rgb_color(&mut self.color)?;
        xfer.xfer_real(&mut self.cur_speed)?;
        xfer.xfer_real(&mut self.max_speed)?;
        xfer.xfer_real(&mut self.accel)?;
        xfer.xfer_real(&mut self.wobble_len)?;
        xfer.xfer_real(&mut self.wobble_amp)?;
        xfer.xfer_real(&mut self.wobble_rate)?;
        xfer.xfer_real(&mut self.cur_wobble_phase)?;
        xfer.xfer_real(&mut self.cur_z_offset)?;

        if xfer.get_xfer_mode() == XferMode::Load {
            self.toss_segments();
        }
        Ok(())
    }

    /// `W3DRopeDraw::loadPostProcess` (W3DRopeDraw.cpp:289).
    pub fn load_post_process(&mut self) {}
}

impl Default for W3DRopeDraw {
    fn default() -> Self {
        Self::new()
    }
}
