//! UNTESTED DRAFT (#202, #204): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DDynamicLight.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDynamicLight.cpp
//!
//! `LightClass` (WW3D2/Light.h) is not in the port yet: [`LightClass`] below holds only the members this class reads and
//! writes (`Ambient`, `Diffuse`, `FarAttenStart`, `FarAttenEnd`), under upstream's names, for the draft.

use rts_lockstep_engine::ww::vector3::Vector3;

/// The members of `LightClass` (Light.h) that `W3DDynamicLight` uses. `LightClass::POINT` is the only kind it makes.
#[derive(Clone, Copy, Debug, Default)]
pub struct LightClass {
    /// `LightClass::Ambient`.
    pub ambient: Vector3,
    /// `LightClass::Diffuse`.
    pub diffuse: Vector3,
    /// `LightClass::FarAttenStart`.
    pub far_atten_start: f32,
    /// `LightClass::FarAttenEnd`.
    pub far_atten_end: f32,
}

/// `class W3DDynamicLight : public LightClass` (W3DDynamicLight.h).
#[derive(Clone, Copy, Debug, Default)]
pub struct W3DDynamicLight {
    /// The `LightClass` base (`LightClass(LightClass::POINT)`).
    pub light: LightClass,
    /// `m_priorEnable`: values used by `HeightMapRenderObjClass` to update the height map.
    pub prior_enable: bool,
    /// `m_processMe`.
    pub process_me: bool,
    /// `m_prevMinX`.
    pub prev_min_x: i32,
    /// `m_prevMinY`.
    pub prev_min_y: i32,
    /// `m_prevMaxX`.
    pub prev_max_x: i32,
    /// `m_prevMaxY`.
    pub prev_max_y: i32,
    /// `m_minX`.
    pub min_x: i32,
    /// `m_minY`.
    pub min_y: i32,
    /// `m_maxX`.
    pub max_x: i32,
    /// `m_maxY`.
    pub max_y: i32,
    /// `m_enabled`.
    pub enabled: bool,
    /// `m_decayRange`.
    pub decay_range: bool,
    /// `m_decayColor`.
    pub decay_color: bool,
    /// `m_curDecayFrameCount`.
    pub cur_decay_frame_count: u32,
    /// `m_curIncreaseFrameCount`.
    pub cur_increase_frame_count: u32,
    /// `m_decayFrameCount`.
    pub decay_frame_count: u32,
    /// `m_increaseFrameCount`.
    pub increase_frame_count: u32,
    /// `m_targetRange`.
    pub target_range: f32,
    /// `m_targetAmbient`.
    pub target_ambient: Vector3,
    /// `m_targetDiffuse`.
    pub target_diffuse: Vector3,
}

impl W3DDynamicLight {
    /// `W3DDynamicLight::W3DDynamicLight` (W3DDynamicLight.cpp:32). The members upstream leaves uninitialised are zero.
    pub fn new() -> Self {
        W3DDynamicLight { prior_enable: false, enabled: true, ..Default::default() }
    }

    /// `W3DDynamicLight::On_Frame_Update` (W3DDynamicLight.cpp:43).
    pub fn on_frame_update(&mut self) {
        if !self.enabled {
            return;
        }
        let factor: f32;
        if self.cur_increase_frame_count > 0 && self.increase_frame_count > 0 {
            // increasing
            self.cur_increase_frame_count -= 1;
            factor = (self.increase_frame_count - self.cur_increase_frame_count) as f32 / self.increase_frame_count as f32;
        } else if self.decay_frame_count == 0 {
            factor = 1.0; // never decays,
        } else {
            // upstream: UnsignedInt `--`; a count already 0 wraps. Kept.
            self.cur_decay_frame_count = self.cur_decay_frame_count.wrapping_sub(1);
            if self.cur_decay_frame_count == 0 {
                self.enabled = false;
                return;
            }
            factor = self.cur_decay_frame_count as f32 / self.decay_frame_count as f32;
        }
        if self.decay_range {
            self.light.far_atten_end = factor * self.target_range;
            if self.light.far_atten_end < self.light.far_atten_start {
                self.light.far_atten_end = self.light.far_atten_start;
            }
        }
        if self.decay_color {
            self.light.ambient = self.target_ambient * factor;
            self.light.diffuse = self.target_diffuse * factor;
        }
    }

    /// `W3DDynamicLight::setEnabled` (W3DDynamicLight.h).
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.decay_range = false;
        self.decay_frame_count = 0;
        self.decay_color = false;
        self.increase_frame_count = 0;
    }

    /// `W3DDynamicLight::isEnabled` (W3DDynamicLight.h).
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// `W3DDynamicLight::setFrameFade` (W3DDynamicLight.cpp:78). 0 `frame_increase_time` means it starts out full
    /// size/intensity, 0 decay time means it lasts forever.
    pub fn set_frame_fade(&mut self, frame_increase_time: u32, decay_frame_time: u32) {
        self.decay_frame_count = decay_frame_time;
        self.cur_decay_frame_count = decay_frame_time;
        self.cur_increase_frame_count = frame_increase_time;
        self.increase_frame_count = frame_increase_time;
        self.target_ambient = self.light.ambient;
        self.target_diffuse = self.light.diffuse;
        self.target_range = self.light.far_atten_end;
    }

    /// `W3DDynamicLight::setDecayRange` (W3DDynamicLight.h).
    pub fn set_decay_range(&mut self) {
        self.decay_range = true;
    }

    /// `W3DDynamicLight::setDecayColor` (W3DDynamicLight.h).
    pub fn set_decay_color(&mut self) {
        self.decay_color = true;
    }

    /// `W3DDynamicLight::cull` (W3DDynamicLight.h): true if the terrain vertex at x,y is outside of the light's influence.
    pub fn cull(&self, x: i32, y: i32) -> bool {
        x < self.min_x || y < self.min_y || x > self.max_x || y > self.max_y
    }
}
