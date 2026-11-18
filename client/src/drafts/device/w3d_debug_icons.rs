//! UNTESTED DRAFT (#202, #205, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDebugIcons.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DDebugIcons.h
//!
//! Upstream compiles the whole file under `_DEBUG || _INTERNAL` (a debug tool: pathfinding icons). The icon store and
//! the quads `Render` builds are translated; the DX8 buffers, shader presets (`SC_ALPHA` ...) and the render-object
//! plumbing (`Cast_Ray`, `Class_ID`, `Clone`, bounding volumes) are device-only, not translated.
//! `W3DDebugIcons`' icons array is a static shared by all instances; here it is the struct's.

use rts_lockstep_engine::common::base_type::{Coord3D, RGBColor};

/// `MAX_ICONS` (W3DDebugIcons.h:72).
pub const MAX_ICONS: usize = 100000;

/// `struct DebugIcon`: all are squares centered about pos.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DebugIcon {
    pub position: Coord3D,
    pub width: f32,
    pub color: RGBColor,
    /// Frame when this disappears.
    pub end_frame: i32,
}

/// One quad as `Render` writes it: four corners and the diffuse colour (b | g<<8 | r<<16 | a<<24).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconQuad {
    pub corners: [(f32, f32, f32); 4],
    pub diffuse: i32,
}

/// `class W3DDebugIcons`.
#[derive(Clone, Debug, Default)]
pub struct W3DDebugIcons {
    /// `m_debugIcons` (the first `m_numDebugIcons`).
    pub debug_icons: Vec<DebugIcon>,
    /// `static Int maxIcons`.
    pub max_icons: i32,
    /// `static Real offset` of `Render` (30, +0.5 per render; read by nothing).
    pub offset: f32,
}

impl W3DDebugIcons {
    /// `W3DDebugIcons::W3DDebugIcons` (W3DDebugIcons.cpp:96): `allocateIconsArray`.
    pub fn new() -> Self {
        W3DDebugIcons { debug_icons: Vec::new(), max_icons: 0, offset: 30.0 }
    }

    /// `W3DDebugIcons::allocateIconsArray` (W3DDebugIcons.cpp:148).
    pub fn allocate_icons_array(&mut self) {
        self.debug_icons = Vec::new();
    }

    /// `W3DDebugIcons::compressIconsArray` (W3DDebugIcons.cpp:156): keeps the icons still alive. Upstream's test is
    /// `endFrame >= frame && i > newNum`: the first icon (i == newNum == 0) is never kept even when alive, and an
    /// icon equal to its own slot is dropped without advancing `newNum`. Kept as written.
    pub fn compress_icons_array(&mut self, logic_frame: i32) {
        let n = self.debug_icons.len();
        if n > 0 {
            let mut new_num = 0usize;
            for i in 0..n {
                if self.debug_icons[i].end_frame >= logic_frame && i > new_num {
                    self.debug_icons[new_num] = self.debug_icons[i];
                    new_num += 1;
                }
            }
            self.debug_icons.truncate(new_num);
        }
    }

    /// `W3DDebugIcons::addIcon` (W3DDebugIcons.cpp:175): `pos = None` clears the list.
    pub fn add_icon(&mut self, pos: Option<&Coord3D>, width: f32, num_frames_duration: i32, color: RGBColor, logic_frame: i32) {
        let Some(pos) = pos else {
            if self.debug_icons.len() as i32 > self.max_icons {
                // DEBUG_LOG(("Max icons %d\n", m_numDebugIcons));
                self.max_icons = self.debug_icons.len() as i32;
            }
            self.debug_icons.clear();
            return;
        };
        if self.debug_icons.len() >= MAX_ICONS {
            return;
        }
        self.debug_icons.push(DebugIcon { position: *pos, width, color, end_frame: logic_frame + num_frames_duration });
    }

    /// `W3DDebugIcons::Render` (W3DDebugIcons.cpp:195), up to the device: the quads it writes (alpha 64, fading over
    /// the last 100 frames; icons past their end frame are skipped), and whether any vanished (then the array is
    /// compressed, as upstream does after drawing).
    pub fn render(&mut self, logic_frame: i32) -> Vec<IconQuad> {
        let mut quads = Vec::new();
        let mut any_vanished = false;
        if self.debug_icons.is_empty() {
            return quads;
        }
        self.offset += 0.5;
        for icon in self.debug_icons.iter() {
            let mut the_alpha: i32 = 64;
            const FADE_FRAMES: i32 = 100;
            let frames_left = icon.end_frame - logic_frame;
            if frames_left < 1 {
                any_vanished = true;
                continue;
            }
            if frames_left < FADE_FRAMES {
                the_alpha = (the_alpha as f32 * (frames_left as f32 / FADE_FRAMES as f32)) as i32; // theAlpha *= (Real)framesLeft/FADE_FRAMES
            }
            let half_width = icon.width / 2.0;
            let diffuse = icon.color.get_as_int() | (the_alpha << 24);
            let pt1 = icon.position;
            quads.push(IconQuad {
                corners: [
                    (pt1.x - half_width, pt1.y - half_width, pt1.z),
                    (pt1.x + half_width, pt1.y - half_width, pt1.z),
                    (pt1.x + half_width, pt1.y + half_width, pt1.z),
                    (pt1.x - half_width, pt1.y + half_width, pt1.z),
                ],
                diffuse,
            });
        }
        // upstream: Draw_Triangles in batches of at most 5000 quads — device-only, not translated
        if any_vanished {
            self.compress_icons_array(logic_frame);
        }
        quads
    }
}
