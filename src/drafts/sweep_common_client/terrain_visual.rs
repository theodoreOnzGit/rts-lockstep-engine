//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/Terrain/TerrainVisual.cpp   (`DomeStyleSeismicFilter`)
//!   GeneralsMD/Code/GameEngine/Include/GameClient/TerrainVisual.h            (`SeismicSimulationNode`, status codes)
//!
//! The seismic filter's virtual interface (`SeismicSimulationFilterBase`) is one concrete type here: the dome-style
//! filter is the only implementation in the release. The height map (`WorldHeightMapInterfaceClass`) is the
//! [`SeismicHeightMap`] trait. Maths follow the port's `petir::Real` convention.

use petir::Real;

/// `SeismicSimulationFilterBase::SeismicSimStatusCode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeismicSimStatusCode {
    /// `SEISMIC_STATUS_INVALID`.
    Invalid,
    /// `SEISMIC_STATUS_ACTIVE`.
    Active,
    /// `SEISMIC_STATUS_ZERO_ENERGY`.
    ZeroEnergy,
}

/// The members of `SeismicSimulationNode` the filter reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeismicSimulationNode {
    /// `m_center` (`ICoord2D`).
    pub center: (i32, i32),
    /// `m_radius`.
    pub radius: i32,
    /// `m_life`.
    pub life: i32,
    /// `m_magnitude`.
    pub magnitude: f32,
}

/// The part of `WorldHeightMapInterfaceClass` the filter uses.
pub trait SeismicHeightMap {
    /// `getBorderSize()`.
    fn get_border_size(&self) -> i32;
    /// `getBilinearSampleSeismicZVelocity(x, y)`.
    fn get_bilinear_sample_seismic_z_velocity(&self, x: i32, y: i32) -> f32;
    /// `setSeismicZVelocity(x, y, value)`.
    fn set_seismic_z_velocity(&mut self, x: i32, y: i32, value: f32);
}

/// `DomeStyleSeismicFilter::filterCallback(heightMap, node)` (TerrainVisual.cpp:143).
pub fn dome_style_filter_callback<M: SeismicHeightMap + ?Sized>(height_map: Option<&mut M>, node: &SeismicSimulationNode) -> SeismicSimStatusCode {
    let life = node.life;
    let height_map = match height_map {
        Some(h) => h,
        None => return SeismicSimStatusCode::Invalid,
    };
    if life == 0 {
        return SeismicSimStatusCode::Active;
    }
    if life < 15 {
        // ADD HEIGHT BECAUSE THE EXPLOSION IS PUSHING DIRT UP
        let magnitude = node.magnitude;
        let offs_scalar = magnitude / life as f32; // real-life, get it?
        let radius = node.radius;
        let border = height_map.get_border_size();
        let center_x = node.center.0 + border;
        let center_y = node.center.1 + border;
        let workspace_width = (radius * 2) as usize;
        let mut workspace = vec![0.0f32; workspace_width * workspace_width]; // clear the workspace
        let w = workspace_width as i32;
        for x in 0..radius {
            for y in 0..radius {
                let distance = Real::sqrt(f64::from(x * x + y * y)) as f32; // Pythagoras
                if distance < radius as f32 {
                    let dist_scalar = Real::cos(f64::from(distance / radius as f32 * (crate::common::base_type::PI / 2.0))) as f32;
                    let height = offs_scalar * dist_scalar;
                    workspace[((radius + x) + w * (radius + y)) as usize] = height + height_map.get_bilinear_sample_seismic_z_velocity(center_x + x, center_y + y); // kaleidoscope
                    if x != 0 {
                        // non-zero test prevents cross-shaped double stamp
                        workspace[((radius - x) + w * (radius + y)) as usize] = height + height_map.get_bilinear_sample_seismic_z_velocity(center_x - x, center_y + y);
                        if y != 0 {
                            workspace[((radius - x) + w * (radius - y)) as usize] = height + height_map.get_bilinear_sample_seismic_z_velocity(center_x - x, center_y - y);
                        }
                    }
                    if y != 0 {
                        workspace[((radius + x) + w * (radius - y)) as usize] = height + height_map.get_bilinear_sample_seismic_z_velocity(center_x + x, center_y - y);
                    }
                }
            }
        }
        // stuff the values from the workspace into the heightmap's velocities
        for x in 0..w {
            for y in 0..w {
                let v = workspace[(x + w * y) as usize];
                height_map.set_seismic_z_velocity(center_x - radius + x, center_y - radius + y, if 9.0f32 < v { 9.0 } else { v });
            }
        }
        SeismicSimStatusCode::Active
    } else {
        SeismicSimStatusCode::ZeroEnergy
    }
}

/// `DomeStyleSeismicFilter::applyGravityCallback(velocityIn)` (TerrainVisual.cpp:212): `velocityIn - 1.5f`.
pub fn dome_style_apply_gravity_callback(velocity_in: f32) -> f32 {
    let mut velocity_out = velocity_in;
    velocity_out -= 1.5f32;
    velocity_out
}
