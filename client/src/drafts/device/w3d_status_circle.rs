//! UNTESTED DRAFT (#202, #205): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DStatusCircle.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DStatusCircle.h
//!
//! `W3DStatusCircle` is the render object that draws the team dot (a small circle in the screen corner) and the
//! script engine's full-screen fade quad. The vertex data and the choice of what to draw are translated; the DX8
//! buffers, shader presets (`SC_ALPHA`, `SC_ADD`) and render states are device-only, not translated: [`StatusCirclePass`]
//! names each pass for the client's renderer, with the blend it needs.

use rts_lockstep_engine::common::base_type::PI;
use rts_lockstep_engine::logic::world::script_engine::fade;
use rts_lockstep_engine::ww::vector3::Vector3;

/// `NUM_TRI`.
pub const NUM_TRI: i32 = 20;

/// `VertexFormatXYZDUV1`: position, diffuse (b | g<<8 | r<<16 | a<<24) and one UV pair.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VertexXyzDuv1 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub diffuse: i32,
    pub u1: f32,
    pub v1: f32,
}

/// The blends `Render` selects for the fade quad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FadeBlend {
    /// `FADE_ADD` (and the `default:` arm): the `SC_ADD` shader as set.
    Add,
    /// `FADE_SUBTRACT`: `D3DRS_BLENDOP = D3DBLENDOP_REVSUBTRACT` for the draw, then back to `ADD`.
    Subtract,
    /// `FADE_SATURATE`: src = DESTCOLOR, dest = SRCCOLOR, drawn twice ("4x multiply").
    Saturate,
    /// `FADE_MULTIPLY`: src = ZERO, dest = SRCCOLOR ("straight multiply").
    Multiply,
}

/// One draw `Render` issues.
#[derive(Clone, Debug, PartialEq)]
pub enum StatusCirclePass {
    /// The team dot: `NUM_TRI` triangles of the circle buffer at translation (0.95, 0.67, 0), `SC_ALPHA`.
    TeamDot { vertices: Vec<VertexXyzDuv1>, translation: Vector3 },
    /// The fade quad (two triangles over the screen, identity transform), drawn `draws` times with `blend`.
    Fade { vertices: [VertexXyzDuv1; 6], blend: FadeBlend, draws: i32 },
}

/// `class W3DStatusCircle`.
#[derive(Clone, Debug)]
pub struct W3DStatusCircle {
    /// `m_needUpdate` (static upstream).
    pub need_update: bool,
    /// `m_diffuse` (static upstream): 255, blue.
    pub diffuse: i32,
    /// `m_numTriangles`.
    pub num_triangles: i32,
    /// `m_indexBuffer != NULL` (the buffers exist after `initData`).
    pub initialised: bool,
    /// `m_vertexBufferCircle`'s contents.
    pub vertex_buffer_circle: Vec<VertexXyzDuv1>,
}

impl W3DStatusCircle {
    /// `W3DStatusCircle::W3DStatusCircle` (W3DStatusCircle.cpp:102): the buffers `NULL`.
    pub fn new() -> Self {
        W3DStatusCircle { need_update: false, diffuse: 255, num_triangles: 0, initialised: false, vertex_buffer_circle: Vec::new() }
    }

    /// `W3DStatusCircle::freeMapResources` (W3DStatusCircle.cpp:147).
    pub fn free_map_resources(&mut self) -> i32 {
        self.initialised = false;
        self.vertex_buffer_circle.clear();
        0
    }

    /// `W3DStatusCircle::initData` (W3DStatusCircle.cpp:165): the index buffer is 0, 1, 2, 3, ... (`3 * NUM_TRI`
    /// indices, device-only); the circle buffer is `3 * NUM_TRI` vertices.
    pub fn init_data(&mut self) -> i32 {
        self.need_update = true;
        self.free_map_resources(); //free old data and ib/vb
        self.num_triangles = NUM_TRI;
        self.vertex_buffer_circle = vec![VertexXyzDuv1::default(); (self.num_triangles * 3) as usize];
        self.initialised = true;
        0
    }

    /// `W3DStatusCircle::updateCircleVB` (W3DStatusCircle.cpp:205): a circle with a team colour.
    pub fn update_circle_vb(&mut self) -> i32 {
        if self.initialised {
            self.need_update = false;
            let the_z = 0.0f32;
            let the_radius = 0.02f32;
            let the_alpha = 127;
            let diffuse = self.diffuse + (the_alpha << 24); // b g<<8 r<<16 a<<24.
            let limit = self.num_triangles;
            let mut cur_angle = 0.0f32;
            let delta_angle = 2.0 * PI / limit as f32;
            let mut n = 0usize;
            for i in 0..limit {
                for k in 0..3 {
                    let vb = &mut self.vertex_buffer_circle[n];
                    vb.z = the_z;
                    if k == 0 {
                        vb.x = 0.0;
                        vb.y = 0.0;
                    } else if k == 1 {
                        let mut vec = Vector3::new(the_radius, 0.0, the_z);
                        vec.rotate_z(cur_angle);
                        vb.x = vec.x;
                        vb.y = vec.y;
                    } else {
                        let mut angle = cur_angle + delta_angle;
                        if i == limit - 1 {
                            angle = 0.0;
                        }
                        let mut vec = Vector3::new(the_radius, 0.0, the_z);
                        vec.rotate_z(angle);
                        vb.x = vec.x;
                        vb.y = vec.y;
                    }
                    vb.diffuse = diffuse;
                    vb.u1 = 0.0;
                    vb.v1 = 0.0;
                    n += 1;
                }
                cur_angle += delta_angle;
            }
            return 0; //success.
        }
        -1
    }

    /// `W3DStatusCircle::updateScreenVB` (W3DStatusCircle.cpp:269): the screen quad's six vertices.
    pub fn update_screen_vb(diffuse: i32) -> [VertexXyzDuv1; 6] {
        let v = |x: f32, y: f32| VertexXyzDuv1 { x, y, z: 0.0, diffuse, u1: 0.0, v1: 0.0 };
        [v(-1.0, -1.0), v(1.0, 1.0), v(-1.0, 1.0), v(-1.0, -1.0), v(1.0, -1.0), v(1.0, 1.0)]
    }

    /// `W3DStatusCircle::Render` (W3DStatusCircle.cpp:340). `is_in_game`: `TheGameLogic->isInGame()`;
    /// `is_shell`: `getGameMode() == GAME_SHELL`; `show_team_dot`: `TheGlobalData->m_showTeamDot`;
    /// `fade_kind`/`fade_value`: `TheScriptEngine->getFade()` and `getFadeValue()` ([`fade`]).
    pub fn render(&mut self, is_in_game: bool, is_shell: bool, show_team_dot: bool, fade_kind: i32, fade_value: f32) -> Vec<StatusCirclePass> {
        let mut passes = Vec::new();
        if !is_in_game || is_shell {
            return passes;
        }
        if !self.initialised {
            self.init_data();
        }
        if !self.initialised {
            return passes;
        }

        if show_team_dot {
            if self.need_update {
                self.update_circle_vb();
            }
            passes.push(StatusCirclePass::TeamDot { vertices: self.vertex_buffer_circle.clone(), translation: Vector3::new(0.95, 0.67, 0.0) });
        }

        if fade_kind == fade::NONE {
            return passes;
        }

        let intensity = fade_value;
        let clr = (255.0 * intensity) as i32;
        let diffuse = (0xff << 24) | (clr << 16) | (clr << 8) | clr; // b g<<8 r<<16 a<<24.
        let vertices = Self::update_screen_vb(diffuse);
        // upstream's switch: FADE_ADD and the default arm draw once with the SC_ADD shader as set
        let (blend, draws) = match fade_kind {
            fade::SUBTRACT => (FadeBlend::Subtract, 1),
            fade::SATURATE => (FadeBlend::Saturate, 2),
            fade::MULTIPLY => (FadeBlend::Multiply, 1),
            _ => (FadeBlend::Add, 1),
        };
        passes.push(StatusCirclePass::Fade { vertices, blend, draws });
        passes
    }
}

impl Default for W3DStatusCircle {
    fn default() -> Self {
        Self::new()
    }
}
