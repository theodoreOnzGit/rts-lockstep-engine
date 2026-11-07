// SPDX-License-Identifier: GPL-3.0-only
//
// Partly translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/pointgr.cpp
//   (PointGroupClass::_Init's vertex location and frame UV tables; [`PointTables`])
// Original copyright notice: Command & Conquer Generals Zero Hour(tm), Copyright 2025 Electronic Arts Inc.
// (Westwood Studios, WW3D). Licensed under the GNU General Public License version 3, with the additional terms in
// NOTICE.md. Translated to Rust and modified; not the original program. The rest of this file is new.
//! The particles as quads (GitHub #43, #44). Plain Rust, no Bevy: the app (`app::draw_particles`) puts each batch in
//! one mesh.
//!
//! Upstream's W3D side (`W3DParticleSystemManager::doParticles`) hands each system's particles to a point group
//! (`PointGroupClass`): screen-facing sprites of the particle's size, turned by its Z angle, coloured by its colour and
//! alpha, with the system's texture and shader (`ADDITIVE`, `ALPHA`, `ALPHA_TEST`, `MULTIPLY`); ground-aligned systems
//! lie flat. **New** (the DirectX 8 device is replaced): each batch is a list of quads in Bevy's axes, grouped by texture
//! and blend, built on the CPU each frame facing the camera. Streaks (`STREAK`, a line strip upstream), drawable
//! particles and volume particles are drawn as plain quads for now.
//!
//! **Added 2026-11-07 (GitHub #49):** the W3D model emitters' particles ([`emitter_quads`], a parallel batch merged
//! into the same keys), from the translated `ParticleEmitterClass`/`ParticleBufferClass`; their point-group geometry is
//! `PointGroupClass::_Init`'s (pointgr.cpp, translated in [`PointTables`]).

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

/// `PointGroupClass::_Init`'s vertex tables (pointgr.cpp, GitHub #49): for each of the 256 orientations, a point's
/// triangle (`(0, -2)`, `(-1.732, 1)`, `(1.732, 1)`) and quad (`(+-0.5, +-0.5)`) corners turned by `i * 2 pi / 256`
/// (the angle accumulated in `float`, `Fast_Cos`, `Fast_Sin`), in the camera's x (right) and y (up), times the size.
pub struct PointTables {
    pub tri: Vec<[[f32; 2]; 3]>,
    pub quad: Vec<[[f32; 2]; 4]>,
}

impl PointTables {
    pub fn new() -> Self {
        use rts_lockstep_engine::ww::wwmath::{fast_cos, fast_sin, WWMATH_PI};
        let tri_locs = [[0.0f32, -2.0], [-1.732, 1.0], [1.732, 1.0]];
        let quad_locs = [[-0.5f32, 0.5], [-0.5, -0.5], [0.5, -0.5], [0.5, 0.5]];
        let (mut tri, mut quad) = (Vec::with_capacity(256), Vec::with_capacity(256));
        let mut angle = 0.0f32;
        let angle_step = (WWMATH_PI * 2.0f32) / 256.0f32;
        for _ in 0..256 {
            let (c, s) = (fast_cos(angle), fast_sin(angle));
            let turn = |l: [f32; 2]| [l[0] * c - l[1] * s, l[0] * s + l[1] * c];
            tri.push(tri_locs.map(turn));
            quad.push(quad_locs.map(turn));
            angle += angle_step;
        }
        PointTables { tri, quad }
    }
}

/// The tables, made once.
pub fn point_tables() -> &'static PointTables {
    static T: std::sync::OnceLock<PointTables> = std::sync::OnceLock::new();
    T.get_or_init(PointTables::new)
}

impl Default for PointTables {
    fn default() -> Self {
        Self::new()
    }
}

/// How a W3D emitter's shader blends: additive for `ONE/ONE` (and screen), else alpha.
fn emitter_blend(shader: rts_lockstep_engine::formats::w3d::shader::ShaderClass) -> Blend {
    use rts_lockstep_engine::formats::w3d::shader::{DSTBLEND_ONE, DSTBLEND_ONE_MINUS_SRC_COLOR, SRCBLEND_ONE};
    if shader.get_src_blend_func() == SRCBLEND_ONE && matches!(shader.get_dst_blend_func(), DSTBLEND_ONE | DSTBLEND_ONE_MINUS_SRC_COLOR) {
        Blend::Add
    } else {
        Blend::Alpha
    }
}

/// A camera-facing ribbon of `width` through `points` (game axes) with their colours, into `b` (Bevy axes).
fn ribbon(b: &mut QuadBatch, points: &[[f32; 3]], colors: &[[f32; 4]], width: f32, right: [f32; 3], up: [f32; 3]) {
    use rts_lockstep_engine::ww::wwmath::sqrt;
    // the view direction (right x up)
    let view = [right[1] * up[2] - right[2] * up[1], right[2] * up[0] - right[0] * up[2], right[0] * up[1] - right[1] * up[0]];
    for k in 0..points.len().saturating_sub(1) {
        let (a, c) = (to_bevy(points[k]), to_bevy(points[k + 1]));
        let d = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let mut o = [d[1] * view[2] - d[2] * view[1], d[2] * view[0] - d[0] * view[2], d[0] * view[1] - d[1] * view[0]];
        let l = sqrt(o[0] * o[0] + o[1] * o[1] + o[2] * o[2]);
        if l <= 0.0 {
            continue;
        }
        let h = 0.5 * width / l;
        o = [o[0] * h, o[1] * h, o[2] * h];
        let base = b.positions.len() as u32;
        for (p, sgn, uv) in [(a, -1.0f32, [0.0, 1.0]), (c, -1.0, [1.0, 1.0]), (c, 1.0, [1.0, 0.0]), (a, 1.0, [0.0, 0.0])] {
            b.positions.push([p[0] + sgn * o[0], p[1] + sgn * o[1], p[2] + sgn * o[2]]);
            b.uvs.push(uv);
        }
        b.colors.extend([colors[k], colors[k + 1], colors[k + 1], colors[k]]);
        b.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

/// The W3D emitters' particles (GitHub #49), in batches keyed as [`particle_quads`]'s (texture, blend), to merge with
/// them: what each emitter's buffer handed its renderer at the last draw (`RenderModel::update_emitters`). **New** (the
/// DirectX 8 device is replaced): triangle and quad particles as `PointGroupClass` builds them (its vertex tables,
/// [`PointTables`], facing the camera; the frame's cell of the texture, `frame & mask` in rows of `1 << log2`);
/// line-mode runs and line-group lines as camera-facing ribbons of the line's width or the particle's size (upstream's
/// segment-line renderer subdivides, adds noise and merges intersections; its line groups draw tetrahedra or prisms:
/// not modelled).
pub fn emitter_quads<'a>(
    models: impl Iterator<Item = &'a rts_lockstep_engine::formats::w3d::render::RenderModel>,
    tables: &PointTables,
    right: [f32; 3],
    up: [f32; 3],
) -> BTreeMap<(Vec<u8>, Blend), QuadBatch> {
    use rts_lockstep_engine::formats::w3d::part_buf::ParticleDraw;
    let mut out: BTreeMap<(Vec<u8>, Blend), QuadBatch> = BTreeMap::new();
    for m in models {
        for e in &m.emitters {
            let (Some(d), Some(em)) = (&e.draw, &e.emitter) else { continue };
            let blend = emitter_blend(em.buffer.shader);
            let tex = em.buffer.texture.clone().unwrap_or_default().to_ascii_lowercase();
            let rgba = |c: rts_lockstep_engine::ww::vector4::Vector4| match blend {
                Blend::Add => [c.x, c.y, c.z, 1.0],
                Blend::Alpha => [c.x, c.y, c.z, c.w],
            };
            match d {
                ParticleDraw::Points(p) if !p.points.is_empty() => {
                    let b = out.entry((tex, blend)).or_default();
                    let rows = 1u32 << p.frame_row_column_count_log2;
                    let mask = !(0xFFFF_FFFFu32 << (2 * p.frame_row_column_count_log2));
                    let scale = 1.0f32 / rows as f32;
                    for s in &p.points {
                        let c = to_bevy([s.position.x, s.position.y, s.position.z]);
                        let f = u32::from(s.frame) & mask;
                        let corner = [(f % rows) as f32 * scale, (f / rows) as f32 * scale];
                        let at = |l: [f32; 2]| {
                            [c[0] + (right[0] * l[0] + up[0] * l[1]) * s.size, c[1] + (right[1] * l[0] + up[1] * l[1]) * s.size, c[2] + (right[2] * l[0] + up[2] * l[1]) * s.size]
                        };
                        let base = b.positions.len() as u32;
                        if p.quads {
                            let uvs = [[0.0f32, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];
                            for (l, uv) in tables.quad[s.orientation as usize].iter().zip(uvs) {
                                b.positions.push(at(*l));
                                b.uvs.push([corner[0] + uv[0] * scale, corner[1] + uv[1] * scale]);
                            }
                            b.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
                        } else {
                            let uvs = [[0.5f32, 0.0], [0.0, 0.866], [1.0, 0.866]];
                            for (l, uv) in tables.tri[s.orientation as usize].iter().zip(uvs) {
                                b.positions.push(at(*l));
                                b.uvs.push([corner[0] + uv[0] * scale, corner[1] + uv[1] * scale]);
                            }
                            // (four corners per entry: the triangle's last again, unused)
                            let last = *b.positions.last().expect("pushed");
                            b.positions.push(last);
                            b.uvs.push([0.0, 0.0]);
                            b.indices.extend([base, base + 1, base + 2]);
                        }
                        b.colors.extend([rgba(s.diffuse); 4]);
                    }
                }
                ParticleDraw::Lines { width, runs } => {
                    for r in runs {
                        let b = out.entry((tex.clone(), blend)).or_default();
                        let pts: Vec<[f32; 3]> = r.points.iter().map(|p| [p.x, p.y, p.z]).collect();
                        let cols: Vec<[f32; 4]> = r.colors.iter().map(|c| rgba(*c)).collect();
                        ribbon(b, &pts, &cols, *width, right, up);
                    }
                }
                ParticleDraw::LineGroup(g) => {
                    let b = out.entry((tex, blend)).or_default();
                    for l in &g.lines {
                        let pts = [[l.tail.x, l.tail.y, l.tail.z], [l.head.x, l.head.y, l.head.z]];
                        ribbon(b, &pts, &[rgba(l.tail_diffuse), rgba(l.head_diffuse)], l.size, right, up);
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// `b`'s quads appended to `a` (the INI particles' batches and the W3D emitters' share keys).
pub fn merge_batches(a: &mut BTreeMap<(Vec<u8>, Blend), QuadBatch>, b: BTreeMap<(Vec<u8>, Blend), QuadBatch>) {
    for (k, v) in b {
        let t = a.entry(k).or_default();
        let base = t.positions.len() as u32;
        t.positions.extend(v.positions);
        t.colors.extend(v.colors);
        t.uvs.extend(v.uvs);
        t.indices.extend(v.indices.into_iter().map(|i| i + base));
    }
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
