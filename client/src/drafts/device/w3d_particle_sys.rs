//! UNTESTED DRAFT (#202, #205, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DParticleSys.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DParticleSys.h
//!
//! `W3DParticleSystemManager::doParticles` is translated up to the device: it culls each system's particles against
//! the visible box, fills the position / size / colour / angle arrays and the smudge set exactly as upstream does, and
//! hands the arrays to the caller as [`ParticleBatch`]es instead of calling `PointGroupClass::Render`,
//! `StreakLineClass::Render` (device-only, not translated: the client's `particles.rs` draws a batch). The data
//! (`ParticleSystemManager`, `Particle`) is the port's own, `crate::client::particle_sys`.

use rts_lockstep_engine::client::particle_sys::{particle_type, priority, shader_type, ParticleSystemManager};
use rts_lockstep_engine::common::base_type::PI;
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::formats::w3d::segline::AABox;
use rts_lockstep_engine::ww::vector3::Vector3;

/// `MAX_POINTS_PER_GROUP` (W3DParticleSys.h).
pub const MAX_POINTS_PER_GROUP: usize = 512;

/// One system's particles as upstream's arrays hold them, ready for the point group or the streak line.
#[derive(Clone, Debug)]
pub struct ParticleBatch {
    /// `sys->getParticleTypeName()`: the texture, got with `Get_Texture`.
    pub texture_name: Vec<u8>,
    /// `sys->getShaderType()` (`ParticleSystemInfo::ADDITIVE`, `ALPHA`, `ALPHA_TEST`, `MULTIPLY`).
    pub shader_type: i32,
    /// `m_posBuffer`, `m_sizeBuffer`, `m_RGBABuffer` (x = red, y = green, z = blue, w = alpha), `m_angleBuffer`.
    pub positions: Vec<Vector3>,
    pub sizes: Vec<f32>,
    pub rgba: Vec<[f32; 4]>,
    pub angles: Vec<u8>,
    /// `personalities[]`.
    pub personalities: Vec<u32>,
    /// Drawn through `m_streakLine` (`sys->isUsingStreak() && count >= 2`; `rgba[0]` zeroed "to eliminate the scissor
    /// edge on the trailing edge of the streak") rather than `m_pointGroup`.
    pub is_streak: bool,
    /// `sys->shouldBillboard()`, `sys->getVolumeParticleDepth()` (point group only).
    pub billboard: bool,
    pub volume_particle_depth: u32,
}

/// `Smudge` (W3DSmudge.h) as `doParticles` fills it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Smudge {
    pub pos: Vector3,
    pub offset: (f32, f32),
    pub size: f32,
    pub opacity: f32,
}

/// What `doParticles` produced for one frame.
#[derive(Clone, Debug, Default)]
pub struct ParticleFrame {
    pub batches: Vec<ParticleBatch>,
    /// The global smudge set (`TheSmudgeManager->addSmudgeSet()`).
    pub smudges: Vec<Smudge>,
    /// `visibleSmudgeCount`.
    pub visible_smudge_count: i32,
}

/// `class W3DParticleSystemManager`.
#[derive(Clone, Debug, Default)]
pub struct W3DParticleSystemManager {
    /// `m_readyToRender`.
    pub ready_to_render: bool,
    /// `m_onScreenParticleCount`.
    pub on_screen_particle_count: i32,
    /// `m_fieldParticleCount`.
    pub field_particle_count: i32,
}

impl W3DParticleSystemManager {
    /// `W3DParticleSystemManager::W3DParticleSystemManager` (W3DParticleSys.cpp:54): the point group, streak line and
    /// share buffers are the batches' (device-only, not translated).
    pub fn new() -> Self {
        W3DParticleSystemManager { ready_to_render: false, on_screen_particle_count: 0, field_particle_count: 0 }
    }

    /// `W3DParticleSystemManager::queueParticleRender` (W3DParticleSys.cpp:95): the flag that makes `doParticles`
    /// render once per frame although `Flush()` calls it several times.
    pub fn queue_particle_render(&mut self) {
        self.ready_to_render = true;
    }

    /// `W3DParticleSystemManager::doParticles` (W3DParticleSys.cpp:110). `bbox`: `TheTerrainRenderObject->
    /// getMaximumVisibleBox(frustum, &bbox, TRUE)`; `smudge_support`: `TheSmudgeManager` exists, its
    /// `getHardwareSupport()` and `TheGlobalData->m_useHeatEffects` (None: no smudge manager).
    /// Sets `particles.field_particle_count` and (as `setOnScreenParticleCount`) the on-screen count.
    pub fn do_particles(&mut self, particles: &mut ParticleSystemManager, bbox: &AABox, smudge_manager_present: bool, smudge_hardware_and_heat: bool, random: &mut RandomValues) -> ParticleFrame {
        let mut frame = ParticleFrame::default();
        if !self.ready_to_render {
            return frame;
        }

        // external mechanism must tell us when it's OK to render again...
        self.ready_to_render = false;

        //reset each frame
        self.on_screen_particle_count = 0;

        let mut visible_smudge_count = 0;

        let (bc_x, bc_y, bc_z) = (bbox.center.x, bbox.center.y, bbox.center.z);
        let (be_x, be_y, be_z) = (bbox.extent.x, bbox.extent.y, bbox.extent.z);

        self.field_particle_count = 0;

        for sys in particles.systems.iter() {
            // only look at particle/point style systems
            if sys.info.particle_type == particle_type::DRAWABLE {
                continue;
            }

            //temporary hack that checks if texture name starts with "SMUD" - if so, we can assume it's a smudge type
            // (upstream reads the first four bytes as a DWORD, 0x44554D53)
            if sys.info.particle_type_name.starts_with(b"SMUD") {
                if smudge_manager_present && smudge_hardware_and_heat {
                    //set-up all the per-particle
                    for p in sys.particles.iter() {
                        let pos = p.info.pos;
                        let psize = p.info.size;

                        //Cull particle to edges of screen and terrain.
                        if (pos.x - bc_x).abs() > (be_x + psize) {
                            continue;
                        }
                        if (pos.y - bc_y).abs() > (be_y + psize) {
                            continue;
                        }
                        if (pos.z - bc_z).abs() > (be_z + psize) {
                            continue;
                        }

                        let offset_x = random.game_client_random_value_real(-0.06, 0.06);
                        let offset_y = random.game_client_random_value_real(-0.03, 0.03);
                        frame.smudges.push(Smudge { pos: Vector3::new(pos.x, pos.y, pos.z), offset: (offset_x, offset_y), size: psize, opacity: p.alpha });
                        visible_smudge_count += 1;
                    }
                }
                continue;
            }

            // build W3D particle buffer
            let mut count: usize = 0;
            let mut batch = ParticleBatch {
                texture_name: sys.info.particle_type_name.clone(),
                shader_type: sys.info.shader_type,
                positions: Vec::new(),
                sizes: Vec::new(),
                rgba: Vec::new(),
                angles: Vec::new(),
                personalities: Vec::new(),
                is_streak: false,
                billboard: sys.should_billboard(),
                volume_particle_depth: sys.info.volume_particle_depth,
            };

            //set-up all the per-particle
            for p in sys.particles.iter() {
                let pos = p.info.pos;
                let psize = p.info.size;

                //Cull particle to edges of screen and terrain.
                if (pos.x - bc_x).abs() > (be_x + psize) {
                    continue;
                }
                if (pos.y - bc_y).abs() > (be_y + psize) {
                    continue;
                }
                if (pos.z - bc_z).abs() > (be_z + psize) {
                    continue;
                }

                self.field_particle_count += (sys.info.priority == priority::AREA_EFFECT && sys.info.is_ground_aligned) as i32;

                batch.personalities.push(p.personality);
                batch.positions.push(Vector3::new(pos.x, pos.y, pos.z));
                batch.sizes.push(psize);
                let color = p.color;
                batch.rgba.push([color.red, color.green, color.blue, p.alpha]);
                batch.angles.push((p.info.angle_z * 255.0 / (2.0 * PI)) as u8);

                count += 1;
                if count == MAX_POINTS_PER_GROUP {
                    break;
                }
            }

            if count == 0 {
                continue; //this system has no particles to render
            }

            if sys.is_using_streak() && count >= 2 {
                // This is the happy place for this!
                batch.rgba[0] = [0.0, 0.0, 0.0, 0.0]; //eliminates the scissor edge on the trailing edge of the streak
                batch.is_streak = true;
            }
            // upstream: m_streakLine->Render(rinfo) / m_pointGroup->Render(rinfo) / RenderVolumeParticle — device-only, not translated
            let _ = shader_type::ADDITIVE; // the shader switch maps shader_type to a preset sprite shader: the batch's, drawn by the client
            frame.batches.push(batch);

            self.on_screen_particle_count += count as i32;
        } // next system

        particles.field_particle_count = self.field_particle_count as u32;
        //TheParticleSystemManager->setOnScreenParticleCount(m_onScreenParticleCount)
        // upstream: W3DSnowManager::render, W3DSmudgeManager::render / reset — device-only, not translated
        frame.visible_smudge_count = visible_smudge_count;
        frame
    }
}
