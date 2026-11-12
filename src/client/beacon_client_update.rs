// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/Drawable/Update/BeaconClientUpdate.cpp (BeaconClientUpdateModuleData:
//     its constructor and buildFieldParse; BeaconClientUpdate: its constructor, createParticleSystem, hideBeacon,
//     clientUpdate; #165: xfer, which the header listed before it was written)
//   GeneralsMD/Code/GameEngine/Include/GameClient/Module/BeaconClientUpdate.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc. (Matthew D. Campbell, August 2002)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The beacon's client update module (GitHub #57, #162): `BeaconClientUpdate`, the first client update module the
//! client makes ([`ClientUpdateModule`]).
//!
//! # As upstream does it
//!
//! - **Its data** (`ClientUpdate = BeaconClientUpdate`): `RadarPulseFrequency` (default 30 frames) and
//!   `RadarPulseDuration` (default 15 frames), both `parseDurationUnsignedInt` (milliseconds to whole frames, rounded
//!   up).
//! - **Made** with no particle system and `m_lastRadarPulse` the logic's frame.
//! - **`createParticleSystem(draw)`**: with the drawable's object, the template `BeaconSmoke%6.6X` of the object's
//!   indicator colour (its low 24 bits, upper-case hexadecimal, six digits); found, a system of it attached to the
//!   drawable; not found, the "failsafe" `BeaconSmokeFFFFFF` attached and tinted with the indicator colour
//!   (`tintAllColors`); neither, none.
//! - **`clientUpdate`** (each client frame, `Drawable::updateDrawable`): without a system, one is made; then, if the
//!   drawable is not effectively hidden and the logic's frame is past `m_lastRadarPulse + RadarPulseFrequency`, a
//!   `RADAR_EVENT_BEACON_PULSE` radar event at the drawable's position living `RadarPulseDuration *
//!   SECONDS_PER_LOGICFRAME_REAL` seconds, and `m_lastRadarPulse` is the frame.
//! - **`hideBeacon`** (the logic's `MSG_REMOVE_BEACON` and `MSG_PLACE_BEACON`'s refusals, `ClientCall::HideBeacon`):
//!   the drawable hidden (`setDrawableHidden(true)`) with its shadows off, a system made if there is none, and the
//!   system stopped (`stop()`) if the manager still has it.
//! - **The save** (`xfer`, version 1): the base's, the system's id (as raw bytes) and `m_lastRadarPulse`. ~~(not
//!   written: #59 kept client update modules as raw blocks)~~ **Corrected 2026-11-12 (#165):** written and read by
//!   [`BeaconClientUpdate::xfer`] in the drawable's module list (`client::drawable_save`).
//!
//! # Upstream quirks that are kept
//!
//! - `m_lastRadarPulse + m_framesBetweenRadarPulses` is `UnsignedInt` arithmetic, wrapping.
//! - `hideBeacon` makes a system just to stop it when the beacon had none.
//! - The failsafe's lookup name is formatted with `format("BeaconSmokeFFFFFF")`, a constant.
//!
//! # Where this differs from upstream
//!
//! - **The host** ([`BeaconHost`]) answers what upstream reads of the drawable, the particle system manager, the radar
//!   and the logic; `setShadowsEnabled(false)` is recorded by the host (the client draws no shadows yet).

use crate::common::base_type::Coord3D;
use crate::common::game_common::SECONDS_PER_LOGICFRAME_REAL;
use crate::common::ini::{BlockParse, FieldParse, Ini, IniLoadType, IniResult, UserData};
use crate::common::thing_template::ClientModuleData;
use crate::client::particle_sys::{ParticleSystemId, INVALID_PARTICLE_SYSTEM_ID};
use crate::client::radar::{RadarEventType, RADAR_EVENT_BEACON_PULSE};
use crate::common::xfer::{Xfer, XferResult, XferVersion};
use crate::client::animated_particle_sys_bone_client_update::{AnimatedParticleSysBoneClientUpdate, ANIMATED_PARTICLE_SYS_BONE_CLIENT_UPDATE};
use crate::client::laser_update::{drawable_module_base_xfer, xfer_particle_system_id, LaserUpdate, LASER_UPDATE};
use crate::client::sway_client_update::{SwayClientUpdate, SWAY_CLIENT_UPDATE};
use crate::logic::modules::plant;

/// The module's name (`ClientUpdate = BeaconClientUpdate`).
pub const BEACON_CLIENT_UPDATE: &[u8] = b"BeaconClientUpdate";

/// `BeaconClientUpdateModuleData`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeaconClientUpdateModuleData {
    /// `m_framesBetweenRadarPulses`.
    pub frames_between_radar_pulses: u32,
    /// `m_radarPulseDuration`.
    pub radar_pulse_duration: u32,
}

impl Default for BeaconClientUpdateModuleData {
    /// `BeaconClientUpdateModuleData()`: 30 and 15 frames.
    fn default() -> Self {
        BeaconClientUpdateModuleData { frames_between_radar_pulses: 30, radar_pulse_duration: 15 }
    }
}

/// `buildFieldParse`'s table (the base `ClientUpdateModuleData` adds no field).
pub static BEACON_CLIENT_UPDATE_FIELDS: [FieldParse<BeaconClientUpdateModuleData>; 3] = [
    FieldParse::new("RadarPulseFrequency", |i, d, _| i.parse_duration_unsigned_int().map(|v| d.frames_between_radar_pulses = v), UserData::None),
    FieldParse::new("RadarPulseDuration", |i, d, _| i.parse_duration_unsigned_int().map(|v| d.radar_pulse_duration = v), UserData::None),
    FieldParse::end(),
];

fn parse_client_update_block(ini: &mut Ini<'_>, d: &mut BeaconClientUpdateModuleData) -> IniResult<()> {
    ini.init_from_ini(d, &BEACON_CLIENT_UPDATE_FIELDS)
}

impl BeaconClientUpdateModuleData {
    /// The module data from the `ClientUpdate` block the logic keeps as tokens, read through
    /// [`BEACON_CLIENT_UPDATE_FIELDS`] (an unknown field is the INI error upstream's `initFromINI` gives).
    pub fn parse(block: &ClientModuleData) -> IniResult<Self> {
        let mut text = b"ClientUpdate".to_vec();
        text.extend_from_slice(&crate::client::w3d_model_draw::client_block_text(block)[b"Draw".len()..]);
        let mut d = Self::default();
        let blocks = [BlockParse { token: "ClientUpdate", parse: parse_client_update_block }];
        Ini::load(b"ClientUpdate", &text, IniLoadType::Overwrite, None, &blocks, &mut d)?;
        Ok(d)
    }
}

/// What `BeaconClientUpdate` reads and calls besides itself: its drawable, the drawable's object, the particle system
/// manager, the radar and the logic.
pub trait BeaconHost {
    /// `getDrawable() != NULL`.
    fn has_drawable(&self) -> bool;
    /// `draw->getObject()->getIndicatorColor()`, `None` without an object.
    fn object_indicator_color(&self) -> Option<i32>;
    /// `TheParticleSystemManager->findTemplate(name) != NULL`.
    fn find_template(&self, name: &[u8]) -> bool;
    /// `createParticleSystem(template)` then `system->attachToDrawable(draw)`: the system's id, `None` when none was
    /// made.
    fn create_attached_particle_system(&mut self, name: &[u8]) -> Option<ParticleSystemId>;
    /// `system->tintAllColors(color)`.
    fn tint_all_colors(&mut self, id: ParticleSystemId, color: i32);
    /// `TheParticleSystemManager->findParticleSystem(id)`, then `system->stop()` if found.
    fn stop_particle_system(&mut self, id: ParticleSystemId);
    /// `draw->setDrawableHidden(true)` and `draw->setShadowsEnabled(false)`.
    fn hide_drawable(&mut self);
    /// `draw->isDrawableEffectivelyHidden()`.
    fn is_drawable_effectively_hidden(&self) -> bool;
    /// `draw->getPosition()`.
    fn drawable_position(&self) -> Coord3D;
    /// `TheGameLogic->getFrame()`.
    fn logic_frame(&self) -> u32;
    /// `TheRadar->createEvent(pos, type, secondsToLive)`.
    fn radar_create_event(&mut self, pos: &Coord3D, event_type: RadarEventType, seconds_to_live: f32);
}

/// `createParticleSystem(draw)` (the file's static function): the system's id, or none.
pub fn create_particle_system(host: &mut impl BeaconHost) -> Option<ParticleSystemId> {
    if !host.has_drawable() {
        return None;
    }
    let color = host.object_indicator_color()?;
    let mut template_name = format!("BeaconSmoke{:06X}", 0xffffff & color).into_bytes();
    if plant(9585) {
        template_name = format!("BeaconSmoke{:06x}", 0xffffff & color).into_bytes();
    }
    if host.find_template(&template_name) {
        host.create_attached_particle_system(&template_name)
    } else {
        // This is a failsafe... if someone has monkeyed with the particle system names, or the MP house colors
        let failsafe = b"BeaconSmokeFFFFFF";
        if !host.find_template(failsafe) {
            return None;
        }
        let system = host.create_attached_particle_system(failsafe)?;
        if !plant(9586) {
            host.tint_all_colors(system, color);
        }
        Some(system)
    }
}

/// `class BeaconClientUpdate`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeaconClientUpdate {
    pub data: BeaconClientUpdateModuleData,
    /// `m_particleSystemID`.
    pub particle_system_id: ParticleSystemId,
    /// `m_lastRadarPulse`.
    pub last_radar_pulse: u32,
}

impl BeaconClientUpdate {
    /// `BeaconClientUpdate(thing, moduleData)` at logic frame `frame`.
    pub fn new(data: BeaconClientUpdateModuleData, frame: u32) -> Self {
        BeaconClientUpdate { data, particle_system_id: INVALID_PARTICLE_SYSTEM_ID, last_radar_pulse: frame }
    }

    /// `hideBeacon()`.
    pub fn hide_beacon(&mut self, host: &mut impl BeaconHost) {
        let draw = host.has_drawable();
        if draw {
            host.hide_drawable();
        }
        if draw && self.particle_system_id == INVALID_PARTICLE_SYSTEM_ID && !plant(9587) {
            if let Some(system) = create_particle_system(host) {
                self.particle_system_id = system;
            }
        }
        // clean up particle system
        if self.particle_system_id != INVALID_PARTICLE_SYSTEM_ID {
            host.stop_particle_system(self.particle_system_id);
        }
    }

    /// `clientUpdate()`.
    pub fn client_update(&mut self, host: &mut impl BeaconHost) {
        if !host.has_drawable() {
            return;
        }
        if self.particle_system_id == INVALID_PARTICLE_SYSTEM_ID {
            if let Some(system) = create_particle_system(host) {
                self.particle_system_id = system;
            }
        }
        if !host.is_drawable_effectively_hidden() || plant(9588) {
            let frame = host.logic_frame();
            let due = if plant(9589) { frame >= self.last_radar_pulse.wrapping_add(self.data.frames_between_radar_pulses) } else { frame > self.last_radar_pulse.wrapping_add(self.data.frames_between_radar_pulses) };
            if due {
                let pos = host.drawable_position();
                let seconds = if plant(9590) { self.data.radar_pulse_duration as f32 } else { self.data.radar_pulse_duration as f32 * SECONDS_PER_LOGICFRAME_REAL };
                host.radar_create_event(&pos, RADAR_EVENT_BEACON_PULSE, seconds);
                if !plant(9591) {
                    self.last_radar_pulse = frame;
                }
            }
        }
    }

    /// `xfer(xfer)` (version 1, BeaconClientUpdate.cpp:202-219; #165): the base's (`ClientUpdateModule::xfer`, which is
    /// `DrawableModule::xfer` and `Module::xfer`), the system's id (`xferUser`, its four bytes) and `m_lastRadarPulse`.
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        drawable_module_base_xfer(xfer)?;
        if plant(9614) {
            xfer.xfer_unsigned_int(&mut self.last_radar_pulse)?;
            xfer_particle_system_id(xfer, &mut self.particle_system_id)?;
        } else {
            xfer_particle_system_id(xfer, &mut self.particle_system_id)?;
            xfer.xfer_unsigned_int(&mut self.last_radar_pulse)?;
        }
        Ok(())
    }
}

/// A drawable's client update module (`MODULETYPE_CLIENT_UPDATE`): the ones the client makes. ~~(#162) Only the beacon's.~~
/// **Corrected 2026-11-12 (#165):** all four of upstream's client update classes (`ModuleFactory.cpp`'s
/// `addModule` list): the beacon's, the tree sway, the animated particle system bones and the laser.
#[derive(Clone, Debug, PartialEq)]
pub enum ClientUpdateModule {
    Beacon(BeaconClientUpdate),
    /// (#165) `SwayClientUpdate`.
    Sway(SwayClientUpdate),
    /// (#165) `AnimatedParticleSysBoneClientUpdate`.
    AnimatedParticleSysBone(AnimatedParticleSysBoneClientUpdate),
    /// (#165) `LaserUpdate`.
    Laser(LaserUpdate),
}

impl ClientUpdateModule {
    /// The module's class name (`getModuleNameKey()`'s name: what `findClientUpdateModule` compares).
    pub fn module_name(&self) -> &'static [u8] {
        match self {
            ClientUpdateModule::Beacon(_) => BEACON_CLIENT_UPDATE,
            ClientUpdateModule::Sway(_) => SWAY_CLIENT_UPDATE,
            ClientUpdateModule::AnimatedParticleSysBone(_) => ANIMATED_PARTICLE_SYS_BONE_CLIENT_UPDATE,
            ClientUpdateModule::Laser(_) => LASER_UPDATE,
        }
    }

    /// (#165) The module's `xfer` (each class's own, which layers the base's).
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        match self {
            ClientUpdateModule::Beacon(m) => m.xfer(xfer),
            ClientUpdateModule::Sway(m) => m.xfer(xfer),
            ClientUpdateModule::AnimatedParticleSysBone(m) => m.xfer(xfer),
            ClientUpdateModule::Laser(m) => m.xfer(xfer),
        }
    }
}

/// (#165) A client update module with its module tag (`getModuleTagNameKey()`'s name: what a save finds it by).
#[derive(Clone, Debug, PartialEq)]
pub struct TaggedClientUpdateModule {
    pub module_tag: Vec<u8>,
    pub module: ClientUpdateModule,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::thing_template::ClientLine;

    #[derive(Default)]
    struct Host {
        templates: Vec<Vec<u8>>,
        color: Option<i32>,
        hidden: bool,
        frame: u32,
        log: Vec<String>,
        next_id: u32,
    }

    impl BeaconHost for Host {
        fn has_drawable(&self) -> bool {
            true
        }
        fn object_indicator_color(&self) -> Option<i32> {
            self.color
        }
        fn find_template(&self, name: &[u8]) -> bool {
            self.templates.iter().any(|t| t == name)
        }
        fn create_attached_particle_system(&mut self, name: &[u8]) -> Option<ParticleSystemId> {
            self.next_id += 1;
            self.log.push(format!("create {} -> {}", String::from_utf8_lossy(name), self.next_id));
            Some(self.next_id)
        }
        fn tint_all_colors(&mut self, id: ParticleSystemId, color: i32) {
            self.log.push(format!("tint {id} {color:#x}"));
        }
        fn stop_particle_system(&mut self, id: ParticleSystemId) {
            self.log.push(format!("stop {id}"));
        }
        fn hide_drawable(&mut self) {
            self.hidden = true;
            self.log.push("hide".into());
        }
        fn is_drawable_effectively_hidden(&self) -> bool {
            self.hidden
        }
        fn drawable_position(&self) -> Coord3D {
            Coord3D { x: 1.0, y: 2.0, z: 3.0 }
        }
        fn logic_frame(&self) -> u32 {
            self.frame
        }
        fn radar_create_event(&mut self, pos: &Coord3D, event_type: RadarEventType, seconds_to_live: f32) {
            self.log.push(format!("event {} {} {event_type} {seconds_to_live}", pos.x, pos.y));
        }
    }

    /// Predicted: the first update makes the colour's system; a pulse only after more than 30 frames, every 31; hidden,
    /// none; `hideBeacon` hides and stops the system. A colour without its template takes the tinted white one.
    #[test]
    fn pulses_and_hides() {
        let mut h = Host { templates: vec![b"BeaconSmokeFF0000".to_vec(), b"BeaconSmokeFFFFFF".to_vec()], color: Some(0x7FFF_0000), frame: 10, ..Default::default() };
        let mut b = BeaconClientUpdate::new(BeaconClientUpdateModuleData::default(), 10);
        b.client_update(&mut h);
        assert_eq!(h.log, ["create BeaconSmokeFF0000 -> 1"]);
        h.frame = 40;
        b.client_update(&mut h);
        assert_eq!(h.log.len(), 1);
        h.frame = 41;
        b.client_update(&mut h);
        assert_eq!(h.log[1], format!("event 1 2 {RADAR_EVENT_BEACON_PULSE} {}", 15.0 * SECONDS_PER_LOGICFRAME_REAL));
        b.hide_beacon(&mut h);
        assert_eq!(&h.log[2..], ["hide", "stop 1"]);
        h.frame = 100;
        b.client_update(&mut h);
        assert_eq!(h.log.len(), 4);
        let mut h = Host { templates: vec![b"BeaconSmokeFFFFFF".to_vec()], color: Some(0x123456), ..Default::default() };
        let mut b = BeaconClientUpdate::new(BeaconClientUpdateModuleData::default(), 0);
        b.hide_beacon(&mut h);
        assert_eq!(h.log, ["hide", "create BeaconSmokeFFFFFF -> 1", "tint 1 0x123456", "stop 1"]);
    }

    /// The data: the defaults, and milliseconds rounded up to frames.
    #[test]
    fn module_data_parses() {
        let line = |f: &str, a: &str| ClientLine { depth: 0, field: f.as_bytes().to_vec(), args: vec![a.as_bytes().to_vec()] };
        let block = ClientModuleData { module_tag_name_key: Default::default(), lines: vec![line("RadarPulseFrequency", "1000"), line("RadarPulseDuration", "510")] };
        let d = BeaconClientUpdateModuleData::parse(&block).unwrap();
        assert_eq!((d.frames_between_radar_pulses, d.radar_pulse_duration), (30, 16));
        let empty = ClientModuleData { module_tag_name_key: Default::default(), lines: vec![] };
        assert_eq!(BeaconClientUpdateModuleData::parse(&empty).unwrap(), BeaconClientUpdateModuleData::default());
    }
}
