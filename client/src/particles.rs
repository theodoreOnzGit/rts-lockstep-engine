// SPDX-License-Identifier: GPL-3.0-only
//! The particles as quads (GitHub #43, #44). Plain Rust, no Bevy: the app (`app::draw_particles`) puts each batch in
//! one mesh.
//!
//! Upstream's W3D side (`W3DParticleSystemManager::doParticles`) hands each system's particles to a point group
//! (`PointGroupClass`): screen-facing sprites of the particle's size, turned by its Z angle, coloured by its colour and
//! alpha, with the system's texture and shader (`ADDITIVE`, `ALPHA`, `ALPHA_TEST`, `MULTIPLY`); ground-aligned systems
//! lie flat. **New** (the DirectX 8 device is replaced): each batch is a list of quads in Bevy's axes, grouped by texture
//! and blend, built on the CPU each frame facing the camera. Streaks (`STREAK`, a line strip upstream), drawable
//! particles and volume particles are drawn as plain quads for now.

use std::collections::BTreeMap;

use rts_lockstep_engine::client::particle_sys::{shader_type, ParticleSystemManager};

use crate::view::to_bevy;

/// How a batch is blended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Blend {
    /// `ADDITIVE`.
    Add,
    /// `ALPHA`, `ALPHA_TEST` and `MULTIPLY` (the last drawn as alpha for now).
    Alpha,
}

/// One texture's and blend's quads: four corners each, in Bevy's axes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QuadBatch {
    pub positions: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

impl QuadBatch {
    /// The number of quads.
    pub fn len(&self) -> usize {
        self.positions.len() / 4
    }
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }
}

/// The batches of every live particle, keyed by texture name (lower case) and blend. `right` and `up` are the
/// camera's axes in Bevy's space (for billboards).
pub fn particle_quads(m: &ParticleSystemManager, right: [f32; 3], up: [f32; 3]) -> BTreeMap<(Vec<u8>, Blend), QuadBatch> {
    let mut out: BTreeMap<(Vec<u8>, Blend), QuadBatch> = BTreeMap::new();
    for s in &m.systems {
        if s.particles.is_empty() {
            continue;
        }
        let blend = if s.info.shader_type == shader_type::ADDITIVE { Blend::Add } else { Blend::Alpha };
        let b = out.entry((s.info.particle_type_name.to_ascii_lowercase(), blend)).or_default();
        // ground-aligned: flat on the ground plane (game x and y)
        let (r0, u0) = if s.should_billboard() { (right, up) } else { (to_bevy([1.0, 0.0, 0.0]), to_bevy([0.0, 1.0, 0.0])) };
        for p in &s.particles {
            let c = to_bevy([p.info.pos.x, p.info.pos.y, p.info.pos.z]);
            let half = p.info.size * 0.5;
            let (sa, ca) = (rts_lockstep_engine::ww::wwmath::sin(p.info.angle_z), rts_lockstep_engine::ww::wwmath::cos(p.info.angle_z));
            let r = [(r0[0] * ca + u0[0] * sa) * half, (r0[1] * ca + u0[1] * sa) * half, (r0[2] * ca + u0[2] * sa) * half];
            let u = [(u0[0] * ca - r0[0] * sa) * half, (u0[1] * ca - r0[1] * sa) * half, (u0[2] * ca - r0[2] * sa) * half];
            let base = b.positions.len() as u32;
            for (sx, sy, uv) in [(-1.0f32, -1.0f32, [0.0, 1.0]), (1.0, -1.0, [1.0, 1.0]), (1.0, 1.0, [1.0, 0.0]), (-1.0, 1.0, [0.0, 0.0])] {
                b.positions.push([c[0] + sx * r[0] + sy * u[0], c[1] + sx * r[1] + sy * u[1], c[2] + sx * r[2] + sy * u[2]]);
                b.uvs.push(uv);
            }
            let col = match blend {
                Blend::Add => [p.color.red, p.color.green, p.color.blue, 1.0],
                Blend::Alpha => [p.color.red, p.color.green, p.color.blue, p.alpha],
            };
            b.colors.extend([col; 4]);
            b.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_lockstep_engine::client::particle_sys::{NoParticleHost, ParticleSettings};
    use rts_lockstep_engine::common::random_value::RandomValues;

    /// *Predictions, written before the first run:* a system of 3 additive particles of size 2 at the origin, drawn
    /// with the camera's right (1, 0, 0) and up (0, 1, 0): one batch keyed by its lower-cased texture and `Add`, 12
    /// corners and 18 indices; each particle's corners are 1 from its centre along x and y (angle 0), alpha 1.
    ///
    /// *Results (2026-11-06), first run:* all held.
    #[test]
    fn particles_become_camera_facing_quads() {
        let mut m = ParticleSystemManager::new();
        m.load_text(b"p.ini", b"ParticleSystem Spark\n  Shader = ADDITIVE\n  Type = PARTICLE\n  ParticleName = EXSpark.TGA\n  Lifetime = 10 10\n  Size = 2 2\n  BurstCount = 3 3\n  BurstDelay = 100 100\n  Color1 = R:255 G:255 B:255 0\n  VelocityDamping = 1 1\nEnd\n").unwrap();
        let mut r = RandomValues::new(1);
        m.create_particle_system(b"Spark", true, &mut r, 0).unwrap();
        m.update(&NoParticleHost, &mut r, &ParticleSettings { max_particle_count: 100, ..ParticleSettings::default() }, 1, 1);
        let q = particle_quads(&m, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert_eq!(q.len(), 1);
        let b = &q[&(b"exspark.tga".to_vec(), Blend::Add)];
        assert_eq!((b.len(), b.indices.len()), (3, 18));
        let c = to_bevy([0.0; 3]);
        assert_eq!(b.positions[0], [c[0] - 1.0, c[1] - 1.0, c[2]]);
        assert_eq!(b.positions[2], [c[0] + 1.0, c[1] + 1.0, c[2]]);
        assert!(b.colors.iter().all(|c| c[3] == 1.0));
    }
}
