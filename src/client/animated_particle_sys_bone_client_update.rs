// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/Drawable/Update/AnimatedParticleSysBoneClientUpdate.cpp
//     (AnimatedParticleSysBoneClientUpdate: its constructor, clientUpdate, crc, xfer, loadPostProcess)
//   GeneralsMD/Code/GameEngine/Include/GameClient/Module/AnimatedParticleSysBoneClientUpdate.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The client update module that moves a drawable's bone particle systems with its animation (GitHub #165):
//! `AnimatedParticleSysBoneClientUpdate`.
//!
//! # As upstream does it
//!
//! - **Its data** is the base `ModuleData`'s (no field).
//! - **Made** (AnimatedParticleSysBoneClientUpdate.cpp:48-53) with `m_life` 0.
//! - **`clientUpdate`** (AnimatedParticleSysBoneClientUpdate.cpp:66-92): `m_life` counts up (an `UnsignedInt`,
//!   wrapping); with a drawable, its draw modules are asked in order, each that has an `ObjectDrawInterface`
//!   `updateBonesForClientParticleSystems()`, and the first that answers true ends the walk.
//! - **The save** (`xfer`, version 1, AnimatedParticleSysBoneClientUpdate.cpp:110-122): the base's only (`m_life` is
//!   not saved). `crc` and `loadPostProcess` add nothing.
//!
//! # Where this differs from upstream
//!
//! - **The host** ([`AnimatedParticleSysBoneHost`]) answers the drawable's draw modules. On the client's drawables every
//!   draw module is a `W3DModelDraw` (or a subclass), whose `updateBonesForClientParticleSystems` always answers true
//!   ([`crate::client::w3d_model_draw::W3DModelDraw::update_bones_for_client_particle_systems`]); its systems are not
//!   made yet (#165's gaps), so the walk moves nothing.

use crate::common::xfer::{Xfer, XferResult, XferVersion};
use crate::client::laser_update::drawable_module_base_xfer;
use crate::logic::modules::plant;

/// The module's name (`ClientUpdate = AnimatedParticleSysBoneClientUpdate`).
pub const ANIMATED_PARTICLE_SYS_BONE_CLIENT_UPDATE: &[u8] = b"AnimatedParticleSysBoneClientUpdate";

/// What `AnimatedParticleSysBoneClientUpdate` reads and calls besides itself: the drawable's draw modules.
pub trait AnimatedParticleSysBoneHost {
    /// `getDrawable() != NULL`.
    fn has_drawable(&self) -> bool;
    /// How many draw modules `draw->getDrawModules()` lists.
    fn draw_module_count(&self) -> usize;
    /// `(*dm)->getObjectDrawInterface() != NULL` of draw module `i`.
    fn has_object_draw_interface(&self, i: usize) -> bool;
    /// `di->updateBonesForClientParticleSystems()` of draw module `i`.
    fn update_bones_for_client_particle_systems(&mut self, i: usize) -> bool;
}

/// `class AnimatedParticleSysBoneClientUpdate`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnimatedParticleSysBoneClientUpdate {
    /// `m_life`.
    pub life: u32,
}

impl AnimatedParticleSysBoneClientUpdate {
    /// `AnimatedParticleSysBoneClientUpdate(thing, moduleData)`.
    pub fn new() -> Self {
        AnimatedParticleSysBoneClientUpdate { life: 0 }
    }

    /// `clientUpdate()` (AnimatedParticleSysBoneClientUpdate.cpp:66-92).
    pub fn client_update(&mut self, host: &mut impl AnimatedParticleSysBoneHost) {
        self.life = self.life.wrapping_add(if plant(9611) { 2 } else { 1 });
        if host.has_drawable() {
            for i in 0..host.draw_module_count() {
                if host.has_object_draw_interface(i) && host.update_bones_for_client_particle_systems(i) != plant(9612) {
                    break;
                }
            }
        }
    }

    /// `xfer(xfer)` (version 1, AnimatedParticleSysBoneClientUpdate.cpp:110-122).
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = if plant(9613) { 2 } else { 1 };
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        drawable_module_base_xfer(xfer)
    }
}
