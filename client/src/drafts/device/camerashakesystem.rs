//! UNTESTED DRAFT (#202, #205): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/camerashakesystem.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/camerashakesystem.h
//!
//! `src/client/tactical_view.rs` notes the shaker's angles as 0 (it has no shaker yet); this is the shaker it asks for.
//! `WWMath::Random_Float(min, max)` is `Random_Float() * (max - min) + min` over C's `rand()`; the caller supplies the
//! unit-interval `Random_Float()` as `unit_random`. Client-only: shake is never read by the logic.
//!
//! Upstream quirk kept: `Update_Camera_Shaker`'s "Clamp the result" calls `WWMath::Clamp(angles[i], ...)` and discards
//! the returned value, so the angles are NOT clamped.

use rts_lockstep_engine::common::base_type::PI;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::wwmath::{sin, sqrt, WWMATH_PI};

fn deg_to_radf(x: f32) -> f32 {
    x * WWMATH_PI / 180.0
}

/// `MIN_OMEGA` ... `AXIS_ROTATION` (camerashakesystem.cpp:100-105).
fn min_omega() -> f32 {
    deg_to_radf(12.5 * 360.0)
}
fn max_omega() -> f32 {
    deg_to_radf(15.0 * 360.0)
}
fn end_omega() -> f32 {
    deg_to_radf(360.0)
}
const MIN_PHI: f32 = 0.0; // DEG_TO_RADF(0.0f)
fn max_phi() -> f32 {
    deg_to_radf(360.0)
}
fn axis_rotation() -> [f32; 3] {
    [deg_to_radf(7.5), deg_to_radf(15.0), deg_to_radf(5.0)]
}

/// `WWMath::Random_Float(min, max)` over a `Random_Float()` in [0, 1].
fn random_float(unit_random: &mut dyn FnMut() -> f32, min: f32, max: f32) -> f32 {
    unit_random() * (max - min) + min
}

/// `CameraShakeSystemClass::CameraShakerClass`.
#[derive(Clone, Debug)]
pub struct CameraShakerClass {
    pub position: Vector3,
    pub radius: f32,
    pub duration: f32,
    pub intensity: f32,
    pub elapsed_time: f32,
    pub omega: [f32; 3],
    pub phi: [f32; 3],
}

impl CameraShakerClass {
    /// `CameraShakerClass::CameraShakerClass` (camerashakesystem.cpp:118): random sinusoid values.
    pub fn new(position: Vector3, radius: f32, duration: f32, intensity: f32, unit_random: &mut dyn FnMut() -> f32) -> Self {
        let mut omega = [0.0f32; 3];
        let mut phi = [0.0f32; 3];
        omega[0] = random_float(unit_random, min_omega(), max_omega());
        omega[1] = random_float(unit_random, min_omega(), max_omega());
        omega[2] = random_float(unit_random, min_omega(), max_omega());
        phi[0] = random_float(unit_random, MIN_PHI, max_phi());
        phi[1] = random_float(unit_random, MIN_PHI, max_phi());
        phi[2] = random_float(unit_random, MIN_PHI, max_phi());
        CameraShakerClass { position, radius, duration, intensity, elapsed_time: 0.0, omega, phi }
    }

    /// `Timestep` (camerashakesystem.h:89).
    pub fn timestep(&mut self, dt: f32) {
        self.elapsed_time += dt;
    }

    /// `Is_Expired` (camerashakesystem.h:90).
    pub fn is_expired(&self) -> bool {
        self.elapsed_time >= self.duration
    }

    /// `CameraShakerClass::Compute_Rotations` (camerashakesystem.cpp:145): adds this shaker's angles to `set_angles`.
    pub fn compute_rotations(&self, camera_position: &Vector3, set_angles: &mut [f32; 3], unit_random: &mut dyn FnMut() -> f32) {
        // We want several different sinusiods, each with a different phase shift and frequency. ...
        let diff = *camera_position - self.position;
        let len2 = diff.length2();
        if len2 > self.radius * self.radius {
            return;
        }

        // f(t) = intensity(t,pos) * sin( omega(t) * t + phi );
        let intensity = self.intensity * (1.0 - sqrt(len2) / self.radius) * (1.0 - self.elapsed_time / self.duration);
        let axis = axis_rotation();
        for i in 0..3 {
            let omega = self.omega[i] + (end_omega() - self.omega[i]) * self.elapsed_time;
            set_angles[i] += axis[i] * intensity * sin(omega * self.elapsed_time + self.phi[i]);
            //WST 11/14/2002. Add in additional random fudge.  There seems to be a too mathematical pattern of shake with the above
            let minor_intensity = intensity * 0.5;
            let secondary = [
                random_float(unit_random, -minor_intensity, minor_intensity),
                random_float(unit_random, -minor_intensity, minor_intensity),
                random_float(unit_random, -minor_intensity, minor_intensity),
            ];
            // (*set_angles) += secondary_angles — all three axes, inside the per-axis loop, as upstream
            set_angles[0] += secondary[0];
            set_angles[1] += secondary[1];
            set_angles[2] += secondary[2];
        }
    }
}

/// `class CameraShakeSystemClass`.
#[derive(Clone, Debug, Default)]
pub struct CameraShakeSystemClass {
    /// `CameraShakerList`.
    pub camera_shaker_list: Vec<CameraShakerClass>,
}

impl CameraShakeSystemClass {
    /// `CameraShakeSystemClass::Add_Camera_Shake` (camerashakesystem.cpp:213).
    pub fn add_camera_shake(&mut self, position: &Vector3, radius: f32, duration: f32, power: f32, unit_random: &mut dyn FnMut() -> f32) {
        //Power is in degrees of amplitude.
        let power = power * PI / 180.0;
        self.camera_shaker_list.push(CameraShakerClass::new(*position, radius, duration, power, unit_random));
    }

    /// `CameraShakeSystemClass::IsCameraShaking` (camerashakesystem.cpp:232): any active shaker.
    pub fn is_camera_shaking(&self) -> bool {
        !self.camera_shaker_list.is_empty()
    }

    /// `CameraShakeSystemClass::Timestep` (camerashakesystem.cpp:249): each shaker steps; the expired ones are removed.
    pub fn timestep(&mut self, dt: f32) {
        for obj in self.camera_shaker_list.iter_mut() {
            obj.timestep(dt);
        }
        self.camera_shaker_list.retain(|o| !o.is_expired());
    }

    /// `CameraShakeSystemClass::Update_Camera_Shaker` (camerashakesystem.cpp:280): the accumulated angles.
    pub fn update_camera_shaker(&self, camera_position: Vector3, unit_random: &mut dyn FnMut() -> f32) -> Vector3 {
        let mut angles = [0.0f32; 3];

        // Accumulate the effects of any active camera shakers
        for obj in self.camera_shaker_list.iter() {
            obj.compute_rotations(&camera_position, &mut angles, unit_random);
        }

        // Clamp the result: WWMath::Clamp(angles[i], -AXIS_ROTATION[i], AXIS_ROTATION[i]) — its value is discarded upstream.
        Vector3::new(angles[0], angles[1], angles[2])
    }
}
