// SPDX-License-Identifier: GPL-3.0-only
//! The objects' drawables and their posed models (GitHub #25, step 5 of #10). Plain Rust, no Bevy.
//!
//! What is drawn for an object is decided by the translated client in the engine crate: its drawable
//! (`client::drawable`) follows the logic's client events, and its `W3DModelDraw` modules (`client::w3d_model_draw`)
//! choose a model and an animation from the drawable's model condition flags, and time the animation. This module runs
//! them over a `World` after each logic frame, loads the chosen models through the W3D asset manager from the game's
//! files when there are any ([`Art`]), and poses one render model per object (its hierarchy at the animation's frame,
//! `Anim_Update`). The Bevy app draws those; an object with no model (no files, no `Draw` block, or a model that is not
//! found) is drawn as its primitive shape (`view`).
//!
//! **Reading only.** The drawables take the World's client events (`take_client_events`, which only drains the queue
//! the logic fills for its client) and read objects, as `view::snapshot` does; they never write the World. The tests
//! check the logic's CRCs are those of the gate game without them.
//!
//! **Files that arrive later** (GitHub #24). In the browser the art archives are read by range as they are needed
//! ([`RangedDevice`](rts_lockstep_engine::common::file_system::RangedDevice)): a read whose bytes have not arrived
//! fails and asks for them. The draw modules must not take such a failure for a missing model or animation (upstream's
//! asset manager remembers a missing animation for good), so they ask through [`Deferred`]: a model whose file is being
//! fetched gives no render object for now, and an animation whose file is being fetched (or asked while a model is) is
//! not looked for. The object is drawn as its shape meanwhile, and when the bytes have arrived its drawable is made
//! again with its flags ([`WorldDrawables::draw`]), so its modules choose as they would have with the files at hand. A
//! render model or texture whose read waited is not kept, and is made again on a later frame. The bones a module data
//! caches for its template (pristine, turret and barrel bones) stay not valid while their files are fetched, so the
//! drawable made again finds them ([`Deferred`] counts its "for now" answers, `W3dAssetSource::deferred_answers`; GitHub
//! #42).
//!
//! **Time.** The draw modules' sync time is the client's clock in milliseconds ([`WorldDrawables::update`]'s
//! `sync_time`), as `WW3D::Sync` is upstream: animations run in real time, not in logic frames.

use std::collections::{BTreeMap, BTreeSet};

use rts_lockstep_engine::client::drawable::ClientDrawables;
use rts_lockstep_engine::client::w3d_model_draw::{AnimHandle, DrawEnv, Skeleton, W3dAssetSource, W3dFiles};
use rts_lockstep_engine::common::file_system::{FileSystem, LocalDevice};
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::formats::w3d::assetmgr::{AssetManager, W3dFileFactory};
use rts_lockstep_engine::formats::w3d::part_buf::Ww3dClock;
use rts_lockstep_engine::formats::w3d::part_emt::ParticleRandom;
use rts_lockstep_engine::formats::w3d::render::RenderModel;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;

/// The game's art: its files and the asset manager reading them.
pub struct Art<D: LocalDevice> {
    pub fs: FileSystem<D>,
    pub assets: AssetManager,
}

impl<D: LocalDevice> Art<D> {
    /// The game's asset manager (`W3DAssetManager`, the game's file search) over `fs`.
    pub fn new(fs: FileSystem<D>) -> Self {
        Art { fs, assets: AssetManager::new_game(W3dFileFactory::default()) }
    }
}

/// No art: no render object and no animation is ever found, so every object is drawn as its shape.
pub struct NoArt;

impl W3dAssetSource for NoArt {
    fn create_render_obj(&mut self, _name: &[u8]) -> Option<bool> {
        None
    }
    fn get_hanim(&mut self, _name: &[u8]) -> Option<AnimHandle> {
        None
    }
}

/// The asset manager over files that may arrive later (see the module docs): the draw modules' questions, answered as
/// [`W3dFiles`] answers them once the files they need have arrived, and "none for now" before.
pub struct Deferred<'a, D: LocalDevice> {
    pub files: W3dFiles<'a, D>,
    /// Model names whose file was being fetched when a module asked.
    pub models: &'a mut BTreeSet<Vec<u8>>,
    /// Animation names not looked for because a file was being fetched.
    pub anims: &'a mut BTreeSet<Vec<u8>>,
    /// The questions answered "none for now" so far ([`W3dAssetSource::deferred_answers`], GitHub #42).
    pub deferred: usize,
}

/// Whether `Load_3D_Assets(filename)`'s read (the file, else `..\filename`, as `load_with_parent` tries) would be served
/// now; asks for the bytes when not.
pub fn w3d_file_ready<D: LocalDevice>(assets: &AssetManager, fs: &FileSystem<D>, filename: &[u8]) -> bool {
    let mut parent = b"..\\".to_vec();
    parent.extend_from_slice(filename);
    for f in [filename, &parent[..]] {
        if let Ok(Some(path)) = assets.file_factory.resolve(fs, f) {
            return fs.file_ready(&path);
        }
    }
    true
}

/// Whether `Get_HAnim(name)` can be asked now without a fetch in flight deciding its answer: found or registered
/// missing already, or its file (`A.w3d` of `H.A`) at hand and no model being fetched (the model's file may hold it).
fn anim_ready<D: LocalDevice>(assets: &AssetManager, fs: &FileSystem<D>, name: &[u8], models_waiting: bool) -> bool {
    if assets.peek_anim(name).is_some() || assets.is_missing(name) {
        return true;
    }
    let Some(dot) = name.iter().position(|&c| c == b'.') else { return true };
    let mut filename = name[dot + 1..].to_vec();
    filename.extend_from_slice(b".w3d");
    w3d_file_ready(assets, fs, &filename) && !models_waiting
}

impl<D: LocalDevice> W3dAssetSource for Deferred<'_, D> {
    fn create_render_obj(&mut self, name: &[u8]) -> Option<bool> {
        let before = self.files.fs.local.device.unserved_reads();
        let r = self.files.create_render_obj(name);
        if r.is_none() && self.files.fs.local.device.unserved_reads() != before {
            self.models.insert(name.to_vec());
            self.deferred += 1;
        }
        r
    }
    fn get_hanim(&mut self, name: &[u8]) -> Option<AnimHandle> {
        if !anim_ready(self.files.assets, self.files.fs, name, !self.models.is_empty()) {
            self.anims.insert(name.to_vec());
            self.deferred += 1;
            return None;
        }
        self.files.get_hanim(name)
    }
    /// The skeleton as [`W3dFiles`] makes it, or none when a file it read is being fetched (its hierarchy's file may be
    /// another than the model's).
    fn skeleton(&mut self, name: &[u8], pose: Option<(&[u8], f32)>, scale: f32) -> Option<Skeleton> {
        let before = self.files.fs.local.device.unserved_reads();
        let k = self.files.skeleton(name, pose, scale);
        if self.files.fs.local.device.unserved_reads() != before {
            self.models.insert(name.to_vec());
            self.deferred += 1;
            return None;
        }
        k
    }
    fn deferred_answers(&self) -> usize {
        self.deferred
    }
}

/// What one object's model draws: the model's name, its animation and frame.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelChoice {
    pub model: Vec<u8>,
    /// The animation playing and its current frame.
    pub anim: Option<(Vec<u8>, f32)>,
    pub hidden: bool,
}

/// The drawables of a game and their posed models.
pub struct WorldDrawables<D: LocalDevice> {
    pub drawables: ClientDrawables,
    pub art: Option<Art<D>>,
    /// Each object's render model (its first draw module's), posed, with the name it was made for.
    instances: BTreeMap<ObjectId, (Vec<u8>, RenderModel)>,
    /// Models that could not be made (`RenderModel::create` gave none or failed), by name, and why.
    pub missing: BTreeMap<Vec<u8>, String>,
    /// Models and animations a module asked for while their files were being fetched ([`Deferred`]).
    pub waiting_models: BTreeSet<Vec<u8>>,
    pub waiting_anims: BTreeSet<Vec<u8>>,
    /// Drawables made again because the files their modules waited for arrived (GitHub #24's count).
    pub remade: usize,
    /// The W3D particle generators (#49: `Vector3Randomizer::Randomizer` and part_buf.cpp's `rand_gen`, process globals
    /// upstream), seeded as upstream's statics are.
    pub particle_random: ParticleRandom,
    /// The WW3D clock of the last draw (frame time and count for the emitters, #49).
    pub particle_clock: Option<Ww3dClock>,
}

impl<D: LocalDevice> WorldDrawables<D> {
    pub fn new(art: Option<Art<D>>) -> Self {
        WorldDrawables {
            drawables: ClientDrawables::new(),
            art,
            instances: BTreeMap::new(),
            missing: BTreeMap::new(),
            waiting_models: BTreeSet::new(),
            waiting_anims: BTreeSet::new(),
            remade: 0,
            particle_random: ParticleRandom::default(),
            particle_clock: None,
        }
    }

    /// The client's FX lists and particle systems from the art's INI files (GitHub #43, #44,
    /// `ClientFx::load`): `true` once read (or when there is no art: nothing to read), `false` while a file is being
    /// fetched (the page's ranged reads; try again on a later frame).
    pub fn load_fx(&mut self) -> bool {
        let Some(art) = &self.art else { return true };
        let waiting = std::cell::Cell::new(false);
        let open = |p: &[u8]| {
            let before = art.fs.local.device.unserved_reads();
            let r = art.fs.open_file(p);
            if art.fs.local.device.unserved_reads() != before {
                waiting.set(true);
                return None;
            }
            r
        };
        let mut fx = rts_lockstep_engine::client::client_fx::ClientFx::new();
        fx.load(open);
        if waiting.get() {
            return false;
        }
        self.drawables.fx.fx_lists = fx.fx_lists;
        self.drawables.fx.particles.templates = fx.particles.templates;
        self.drawables.fx.errors = fx.errors;
        true
    }

    /// `f` given the drawables and the art their modules are made from (GitHub #59: a save writes the drawables, a load
    /// makes them; `Deferred`, as for the client's events, or no art).
    pub fn with_assets<R>(&mut self, f: impl FnOnce(&mut ClientDrawables, &mut dyn W3dAssetSource) -> R) -> R {
        match &mut self.art {
            Some(art) => {
                let mut src = Deferred { files: W3dFiles { assets: &mut art.assets, fs: &art.fs }, models: &mut self.waiting_models, anims: &mut self.waiting_anims, deferred: 0 };
                f(&mut self.drawables, &mut src)
            }
            None => f(&mut self.drawables, &mut NoArt),
        }
    }

    /// The objects with a posed render model.
    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }

    /// After a logic frame: the frame's client events, then a client draw at `sync_time` ms (the modules choose and
    /// time their animations), then each object's model posed.
    pub fn update(&mut self, w: &mut World, events: &[ClientEvent], sync_time: u32) {
        self.apply_events(w, events, sync_time);
        self.draw(w, sync_time);
    }

    /// A logic frame's client events (`ClientDrawables::apply_events`).
    pub fn apply_events(&mut self, w: &World, events: &[ClientEvent], sync_time: u32) {
        match &mut self.art {
            Some(art) => {
                let mut src = Deferred { files: W3dFiles { assets: &mut art.assets, fs: &art.fs }, models: &mut self.waiting_models, anims: &mut self.waiting_anims, deferred: 0 };
                self.drawables.apply_events(w, events, &mut src, sync_time)
            }
            None => self.drawables.apply_events(w, events, &mut NoArt, sync_time),
        }
    }

    /// A client draw at `sync_time` ms (`ClientDrawables::draw`), then each object's model posed. First, when files a
    /// module waited for have arrived, the drawables still lacking a model or an animation are made again.
    pub fn draw(&mut self, w: &mut World, sync_time: u32) {
        self.remake_when_arrived(w, sync_time);
        match &mut self.art {
            Some(art) => {
                let mut src = Deferred { files: W3dFiles { assets: &mut art.assets, fs: &art.fs }, models: &mut self.waiting_models, anims: &mut self.waiting_anims, deferred: 0 };
                self.drawables.draw(w, &mut src, sync_time)
            }
            None => self.drawables.draw(w, &mut NoArt, sync_time),
        }
        self.pose();
        self.update_emitters(w, sync_time);
    }

    /// The posed models' particle emitters, one W3D frame at `sync_time` ms (#49): the clock advances as `WW3D::Sync`
    /// does (frame time = the time since the last draw, frame count + 1), and each emitter runs on its bone under its
    /// object's transform (position, orientation, and the client-side physics transform the mesh is drawn with), since
    /// W3D particles live in the world. Reading only: the World is not written.
    fn update_emitters(&mut self, w: &World, sync_time: u32) {
        let clock = match self.particle_clock {
            Some(c) => Ww3dClock { sync_time, frame_time: sync_time.wrapping_sub(c.sync_time), frame_count: c.frame_count.wrapping_add(1) },
            None => Ww3dClock { sync_time, frame_time: 0, frame_count: 0 },
        };
        self.particle_clock = Some(clock);
        let ids: Vec<ObjectId> = self.instances.keys().copied().collect();
        for id in ids {
            let Some(o) = w.find_object(id) else { continue };
            let p = o.get_position();
            let mut world = Matrix3D::from_translation(&rts_lockstep_engine::ww::vector3::Vector3::new(p.x, p.y, p.z));
            world.rotate_z(o.thing.orientation());
            if let Some(x) = self.physics_xform(id) {
                world = Matrix3D::multiply(&world, &x);
            }
            if let Some((_, m)) = self.instances.get_mut(&id) {
                if !m.emitters.is_empty() {
                    m.update_emitters(&world, &clock, &mut self.particle_random);
                }
            }
        }
    }

    /// The posed models, for drawing their particles (#49).
    pub fn instances(&self) -> impl Iterator<Item = &RenderModel> {
        self.instances.values().map(|(_, m)| m)
    }

    /// The waiting names whose files have arrived are dropped; if any was, each drawable with a module lacking its
    /// model, or its animation, is made again (`ClientDrawables::create_drawable`), given its flags back
    /// (`replaceModelConditionFlags`, so its modules choose on the next draw as they would have) and its hidden state
    /// (and, since GitHub #45, its stealth look and shroud state).
    /// New (not upstream's: upstream's reads never wait).
    fn remake_when_arrived(&mut self, w: &World, sync_time: u32) {
        let Some(art) = &mut self.art else { return };
        if self.waiting_models.is_empty() && self.waiting_anims.is_empty() {
            return;
        }
        let before = self.waiting_models.len() + self.waiting_anims.len();
        self.waiting_models.retain(|name| {
            let mut f = name.split(|&c| c == b'.').next().unwrap_or(name).to_vec();
            f.extend_from_slice(b".w3d");
            !w3d_file_ready(&art.assets, &art.fs, &f)
        });
        let models_waiting = !self.waiting_models.is_empty();
        self.waiting_anims.retain(|name| !anim_ready(&art.assets, &art.fs, name, models_waiting));
        if self.waiting_models.len() + self.waiting_anims.len() == before {
            return;
        }
        let incomplete: Vec<ObjectId> = self
            .drawables
            .drawables
            .iter()
            .filter(|(_, d)| {
                d.draw_modules.iter().any(|m| {
                    let Some(cur) = m.get_cur_state() else { return false };
                    match &m.render_object {
                        None => !cur.model_name.is_empty(),
                        Some(r) => r.is_hlod && ((r.anim.is_none() && !cur.animations.is_empty()) || r.skeleton.is_none()),
                    }
                })
            })
            .map(|(id, _)| *id)
            .collect();
        for id in incomplete {
            let Some(old) = self.drawables.drawables.remove(&id) else { continue };
            let mut src = Deferred { files: W3dFiles { assets: &mut art.assets, fs: &art.fs }, models: &mut self.waiting_models, anims: &mut self.waiting_anims, deferred: 0 };
            if !self.drawables.create_drawable(w, id, &mut src, sync_time) {
                continue;
            }
            self.remade += 1;
            let flags = *old.get_model_condition_flags();
            let d = self.drawables.drawables.get_mut(&id).expect("made");
            let mut env = DrawEnv { assets: &mut src, random: &mut self.drawables.random, sync_time, logic_time: false };
            d.replace_model_condition_flags(&flags, false, &mut env);
            if old.hidden {
                d.set_drawable_hidden(true);
            }
            // (GitHub #45: its stealth look and shroud state too)
            d.set_stealth_look(old.stealth_look);
            d.set_fully_obscured_by_shroud(old.fully_obscured_by_shroud);
            d.shroud_clear_frame = old.shroud_clear_frame;
            // (GitHub #59: its saved members, its client physics and its `showSubObject` lists, which a load may have given)
            d.saved = old.saved.clone();
            d.loco_info = old.loco_info;
            for (m, o) in d.draw_modules.iter_mut().zip(&old.draw_modules) {
                m.sub_object_vec = o.sub_object_vec.clone();
                m.update_sub_objects();
            }
        }
    }

    /// Each object's model choice: its first draw module's render object, if it has one.
    pub fn choice(&self, id: ObjectId) -> Option<ModelChoice> {
        Self::choice_in(&self.drawables, id)
    }

    fn choice_in(drawables: &ClientDrawables, id: ObjectId) -> Option<ModelChoice> {
        let d = drawables.drawables.get(&id)?;
        let m = d.draw_modules.first()?;
        let r = m.render_object.as_ref()?;
        Some(ModelChoice { model: r.model_name.clone(), anim: r.anim.as_ref().map(|a| (a.motion.name.clone(), a.frame)), hidden: r.hidden })
    }

    /// The object's posed render model, when its model was found in the art.
    pub fn instance(&self, id: ObjectId) -> Option<&RenderModel> {
        self.instances.get(&id).map(|(_, m)| m)
    }

    /// The last draw's client-side physics transform of the object's drawable, in the model's own frame (GitHub #41,
    /// `Drawable::applyPhysicsXform`: the suspension's height, pitch, roll and yaw, `PhysicsXformInfo::apply` to the
    /// identity), for the app to put after the object's own transform; `None` when none applied.
    pub fn physics_xform(&self, id: ObjectId) -> Option<Matrix3D> {
        self.drawables.drawables.get(&id)?.physics_xform.map(|x| x.local_matrix())
    }

    /// Each object's render model made (when its model changes) and posed at its animation's frame.
    fn pose(&mut self) {
        let Some(art) = &mut self.art else { return };
        self.instances.retain(|id, _| self.drawables.drawables.contains_key(id));
        let ids: Vec<ObjectId> = self.drawables.drawables.keys().copied().collect();
        for id in ids {
            let Some(c) = Self::choice_in(&self.drawables, id) else {
                self.instances.remove(&id);
                continue;
            };
            if self.instances.get(&id).is_none_or(|(n, _)| *n != c.model) {
                self.instances.remove(&id);
                if self.missing.contains_key(&c.model) {
                    continue;
                }
                let unserved = art.fs.local.device.unserved_reads();
                let made = RenderModel::create(&mut art.assets, &art.fs, &c.model);
                if art.fs.local.device.unserved_reads() != unserved {
                    // a file it read is being fetched: made again on a later frame
                    continue;
                }
                match made {
                    Ok(Some(m)) => {
                        self.instances.insert(id, (c.model.clone(), m));
                    }
                    Ok(None) => {
                        self.missing.insert(c.model.clone(), "no such render object".into());
                        continue;
                    }
                    Err(e) => {
                        self.missing.insert(c.model.clone(), format!("{e}"));
                        continue;
                    }
                }
            }
            let (_, m) = self.instances.get_mut(&id).expect("made above");
            // the draw module's bone captures (turrets, recoil) and hidden sub-objects, on the model drawn for it
            if let Some(r) = self.drawables.drawables.get(&id).and_then(|d| d.draw_modules.first()).and_then(|dm| dm.render_object.as_ref()) {
                r.pose_render_model(m);
            }
            let root = Matrix3D::IDENTITY;
            let posed = match &c.anim {
                Some((name, frame)) => match art.assets.peek_anim(name) {
                    Some(a) => m.update(&root, Some((a, &art.assets, *frame))),
                    None => m.update::<AssetManager>(&root, None),
                },
                None => m.update::<AssetManager>(&root, None),
            };
            if let Err(e) = posed {
                self.missing.insert(c.model.clone(), format!("posing: {e}"));
                self.instances.remove(&id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_lockstep_engine::common::base_type::Coord3D;
    use rts_lockstep_engine::common::file_system::{MemoryDevice, RangedDevice};
    use rts_lockstep_engine::formats::big::{build_big, ArchiveFile};
    use rts_lockstep_engine::formats::w3d::synth::{self, ChannelSpec, MeshSpec, PivotSpec};
    use rts_lockstep_engine::formats::w3d::w3d_file::ANIM_CHANNEL_Q;
    use rts_lockstep_engine::logic::world::game_logic_update::CRC_RECALC;
    use rts_lockstep_engine::logic::world::gate_scenario::{gate_map_world_with, gate_world, make, run_gate_scenario, INI};
    use rts_lockstep_engine::logic::world::module_call::ModuleCall;
    use rts_lockstep_engine::logic::world::team_ops::factory_find_team;

    /// The client never changes the simulation: the gate game played with the drawables fed every frame (its events
    /// drained and applied, a client draw, the models posed) gives exactly the CRCs of the gate game without them.
    #[test]
    fn the_drawables_leave_the_crcs_alone() {
        let frames = 120;
        let plain = run_gate_scenario(frames);
        let mut w = gate_world();
        let mut d = WorldDrawables::<MemoryDevice>::new(Some(Art::new(synthetic_art())));
        let mut crcs = Vec::new();
        for f in 0..frames {
            w.logic_update().expect("a frame");
            let ev = w.take_client_events();
            d.update(&mut w, &ev, f * 33);
            crcs.push(w.logic_crc(CRC_RECALC, None).expect("the CRC"));
        }
        assert_eq!(crcs, plain);
        // the gate's templates have no Draw blocks: drawables, but no modules and nothing to pose
        assert_eq!(d.drawables.drawables.len(), w.objects.ids_in_order().len());
        assert!(d.drawables.drawables.values().all(|x| x.draw_modules.is_empty()));
    }

    /// A one-bone HLOD `NAME` (a cube on bone 1) and a 4-frame animation `NAME.SPIN` turning the bone, 10 frames a second.
    fn model(name: &[u8], anims: &[&[u8]]) -> Vec<u8> {
        let ident = [0.0, 0.0, 0.0, 1.0];
        let mut f = synth::htree_w3d(name, &[
            PivotSpec { name: b"ROOTTRANSFORM".to_vec(), parent: -1, translation: [0.0; 3], rotation: ident },
            PivotSpec { name: b"BODY".to_vec(), parent: 0, translation: [0.0, 0.0, 2.0], rotation: ident },
        ]);
        let mut body = MeshSpec::cuboid(name, b"BODY", [-2.0, -2.0, -2.0], [2.0, 2.0, 2.0]);
        body.stage0.texture = None;
        f.extend(body.to_w3d());
        let mut sub = name.to_vec();
        sub.extend_from_slice(b".BODY");
        f.extend(synth::hlod_w3d(name, name, &[(&sub, 1)]));
        let q = |deg: f32| {
            let h = deg.to_radians() / 2.0;
            [0.0, 0.0, rts_lockstep_engine::ww::wwmath::sin(h), rts_lockstep_engine::ww::wwmath::cos(h)]
        };
        for a in anims {
            let data: Vec<f32> = [0.0, 90.0, 180.0, 270.0].iter().flat_map(|&d| q(d)).collect();
            f.extend(synth::raw_anim_w3d(name, a, 4, 10, &[ChannelSpec { pivot: 1, kind: ANIM_CHANNEL_Q, vector_len: 4, data }]));
        }
        f
    }

    /// Generated art (no game files): `RUNNER` with `IDLE` and `ROLL`, `RUNNERMV` (the moving model) with `ROLL`,
    /// `SOLDIER` with `IDLE`, `SOLDIERFR` (firing) with `FIRE`.
    fn synthetic_art() -> FileSystem<MemoryDevice> {
        let mut d = MemoryDevice::new();
        d.insert(b"Art/W3D/runner.w3d", model(b"RUNNER", &[b"IDLE"]));
        d.insert(b"Art/W3D/runnermv.w3d", model(b"RUNNERMV", &[b"ROLL"]));
        d.insert(b"Art/W3D/soldier.w3d", model(b"SOLDIER", &[b"IDLE"]));
        d.insert(b"Art/W3D/soldierfr.w3d", model(b"SOLDIERFR", &[b"FIRE"]));
        FileSystem::new(d)
    }

    /// The gate INI's `Object name` block renamed `Drawn<name>`, with `draw` (a `Draw` block) added.
    fn drawn(name: &str, draw: &str) -> String {
        let start = INI.find(&format!("Object {name}\n")).expect("the template");
        let end = start + INI[start..].find("\nEnd\n").expect("its end") + 1;
        format!("{}{draw}End\n", INI[start..end].replacen(&format!("Object {name}"), &format!("Object Drawn{name}"), 1))
    }

    const RUNNER_DRAW: &str = " Draw = W3DModelDraw ModuleTag_Draw\n  DefaultConditionState\n   Model = Runner\n   IdleAnimation = Runner.Idle\n  End\n  \
ConditionState = MOVING\n   Model = RunnerMv\n   Animation = RunnerMv.Roll\n   AnimationMode = LOOP\n  End\n End\n";
    const SOLDIER_DRAW: &str = " Draw = W3DModelDraw ModuleTag_Draw\n  DefaultConditionState\n   Model = Soldier\n   Animation = Soldier.Idle\n   AnimationMode = LOOP\n  End\n  \
ConditionState = FIRING_A\n   Model = SoldierFr\n   Animation = SoldierFr.Fire\n   AnimationMode = ONCE\n  End\n  AliasConditionState = BETWEEN_FIRING_SHOTS_A\n End\n";

    /// The demo game with drawn units, played with generated W3D art. Predictions, written before the first run:
    /// - at the start the runner shows `runner` with its idle animation `RUNNER.IDLE`, the soldiers `soldier` with
    ///   `SOLDIER.IDLE`;
    /// - ordered to move, the runner's logic sets `MOVING` within a few frames and its model becomes `runnermv` playing
    ///   `RUNNERMV.ROLL` (looping, so its frame keeps changing);
    /// - the two soldiers 90 apart acquire each other and fire (`FIRING_A`, then `BETWEEN_FIRING_SHOTS_A`, its alias):
    ///   within 150 frames each shows `soldierfr` with `SOLDIERFR.FIRE` at least once;
    /// - when the runner arrives, `MOVING` clears and it is back to `runner`;
    /// - every object with art has a posed render model: one sub-object on bone 1.
    ///
    /// *Results (2026-11-05), first run:* all held. The runner's model changed at logic frame 2 and was back to `runner`
    /// at frame 133 (120 units at 30 a second, with its acceleration); 131 distinct frames of `RUNNERMV.ROLL` were seen;
    /// both soldiers showed `soldierfr`. The CRC test beside it held too (120 frames equal to the plain gate game's).
    #[test]
    fn the_demo_with_synthetic_models_changes_model_when_units_move_and_fire() {
        let r = play_demo(synthetic_art(), |_| {});
        assert!(r.moved <= 10, "moved at frame {}", r.moved);
        assert!(r.roll_frames > 3, "the roll animation runs: {} frames seen", r.roll_frames);
        assert!(r.back.is_some(), "the runner stopped and is back to its idle model");
        assert_eq!(r.fired, [true, true], "both soldiers showed their firing model");
    }

    /// What [`play_demo`] saw: the frame the runner's moving model showed, the distinct frames of its roll, the frame it
    /// was back to its idle model, whether each soldier showed its firing model, and the frame each object first had
    /// a posed model (runner, soldier, soldier).
    struct DemoRun {
        moved: u32,
        roll_frames: usize,
        back: Option<u32>,
        fired: [bool; 2],
        first_posed: [Option<u32>; 3],
        remade: usize,
        /// The logic's CRC after each frame.
        crcs: Vec<u32>,
    }

    /// The demo game with drawn units over `art`, 300 frames; `pump` runs after each frame's update (the page's part, for
    /// a ranged device). Every object whose drawable chose a model has it posed, except while its file is fetched.
    fn play_demo<D: LocalDevice>(art: FileSystem<D>, mut pump: impl FnMut(&mut FileSystem<D>)) -> DemoRun {
        let extra = format!("{}{}", drawn("Runner", RUNNER_DRAW), drawn("Soldier", SOLDIER_DRAW));
        let mut w = gate_map_world_with(&extra);
        let team = factory_find_team(&mut w, b"team1").unwrap().unwrap();
        let enemy = factory_find_team(&mut w, b"team2").unwrap().unwrap();
        let at = |x: f32, y: f32| Coord3D { x, y, z: 0.0 };
        let runner = make(&mut w, b"DrawnRunner", team, at(40.0, 50.0));
        let s1 = make(&mut w, b"DrawnSoldier", team, at(150.0, 380.0));
        let s2 = make(&mut w, b"DrawnSoldier", enemy, at(240.0, 380.0));
        w.pathfinder_op(rts_lockstep_engine::logic::world::pathfinder_host_world::PfOp::NewMap);
        let mut d = WorldDrawables::new(Some(Art::new(art)));
        let ev = w.take_client_events();
        d.update(&mut w, &ev, 0);
        assert!(d.drawables.errors.is_empty(), "{:?}", d.drawables.errors);
        let name = |c: Option<ModelChoice>| c.map(|c| (String::from_utf8_lossy(&c.model).into_owned(), c.anim.map(|(a, _)| String::from_utf8_lossy(&a).into_owned())));
        let mut first_posed = [None; 3];
        let see = |d: &WorldDrawables<D>, f: u32, first_posed: &mut [Option<u32>; 3]| {
            for (i, id) in [runner, s1, s2].into_iter().enumerate() {
                if d.instance(id).is_some() {
                    first_posed[i].get_or_insert(f);
                }
            }
        };
        see(&d, 0, &mut first_posed);
        if first_posed[0] == Some(0) {
            assert_eq!(name(d.choice(runner)), Some(("runner".into(), Some("RUNNER.IDLE".into()))));
            assert_eq!(name(d.choice(s1)), Some(("soldier".into(), Some("SOLDIER.IDLE".into()))));
            assert_eq!(d.instance(runner).map(|m| (m.sub_objects.len(), m.sub_objects[0].bone_index)), Some((1, 1)));
        }
        pump(&mut d.art.as_mut().unwrap().fs);

        let ai = w.find_object(runner).and_then(|o| o.ai).expect("the runner's AI");
        w.module_call(runner, ai, ModuleCall::AiMoveToPosition { pos: at(160.0, 50.0), command_source: 0 });
        let (mut runner_moved_at, mut runner_back_at) = (None, None);
        let mut fired = [false, false];
        let mut roll_frames = std::collections::BTreeSet::new();
        let mut crcs = Vec::new();
        for f in 1..=300u32 {
            w.logic_update().expect("a frame");
            let ev = w.take_client_events();
            d.update(&mut w, &ev, f * 33);
            crcs.push(w.logic_crc(CRC_RECALC, None).expect("the CRC"));
            see(&d, f, &mut first_posed);
            if f == 1 {
                assert_eq!(name(d.choice(s1)), Some(("soldier".into(), Some("SOLDIER.IDLE".into()))), "by frame 1");
            }
            let rc = name(d.choice(runner));
            if rc == Some(("runnermv".into(), Some("RUNNERMV.ROLL".into()))) {
                runner_moved_at.get_or_insert(f);
                roll_frames.insert(d.choice(runner).unwrap().anim.unwrap().1.to_bits());
            } else if runner_moved_at.is_some() && rc.as_ref().is_some_and(|c| c.0 == "runner") {
                runner_back_at.get_or_insert(f);
            }
            for (i, s) in [s1, s2].iter().enumerate() {
                if name(d.choice(*s)) == Some(("soldierfr".into(), Some("SOLDIERFR.FIRE".into()))) {
                    fired[i] = true;
                }
            }
            let waiting = !d.waiting_models.is_empty() || !d.waiting_anims.is_empty();
            for id in [runner, s1, s2] {
                if w.find_object(id).is_some() && d.choice(id).is_some() && !waiting {
                    assert!(d.instance(id).is_some(), "object {id} at frame {f}: {:?}", d.missing);
                }
            }
            pump(&mut d.art.as_mut().unwrap().fs);
        }
        assert!(d.missing.is_empty(), "{:?}", d.missing);
        DemoRun {
            moved: runner_moved_at.expect("the runner's model changed when it moved"),
            roll_frames: roll_frames.len(),
            back: runner_back_at,
            fired,
            first_posed,
            remade: d.remade,
            crcs,
        }
    }

    /// A turreted gun `GUNT`: pivots ROOTTRANSFORM, TURRET (0, 0, 2), BARREL (1, 0, 0) under it, RECOIL01 (2, 0, 0)
    /// under that, MUZZLEFX01 (1, 0, 0) under that; a cube on each but the root.
    fn gun_model() -> Vec<u8> {
        let ident = [0.0, 0.0, 0.0, 1.0];
        let p = |n: &[u8], parent: i32, t: [f32; 3]| PivotSpec { name: n.to_vec(), parent, translation: t, rotation: ident };
        let mut f = synth::htree_w3d(b"GUNT", &[
            p(b"ROOTTRANSFORM", -1, [0.0; 3]),
            p(b"TURRET", 0, [0.0, 0.0, 2.0]),
            p(b"BARREL", 1, [1.0, 0.0, 0.0]),
            p(b"RECOIL01", 2, [2.0, 0.0, 0.0]),
            p(b"MUZZLEFX01", 3, [1.0, 0.0, 0.0]),
        ]);
        let subs: [(&[u8], u32); 4] = [(b"TURRET", 1), (b"BARREL", 2), (b"GUN", 3), (b"MUZZLEFX01", 4)];
        let mut names = Vec::new();
        for (n, _) in subs {
            let mut m = MeshSpec::cuboid(b"GUNT", n, [-0.5; 3], [0.5; 3]);
            m.stage0.texture = None;
            f.extend(m.to_w3d());
            names.push([&b"GUNT."[..], n].concat());
        }
        let hl: Vec<(&[u8], u32)> = names.iter().zip(subs).map(|(n, (_, b))| (n.as_slice(), b)).collect();
        f.extend(synth::hlod_w3d(b"GUNT", b"GUNT", &hl));
        f
    }

    const GUN_DRAW: &str = " WeaponSet\n  Conditions = None\n  Weapon = PRIMARY GateGun\n End\n Draw = W3DModelDraw ModuleTag_Draw\n  DefaultConditionState\n   \
Model = GunT\n   Turret = Turret\n   TurretPitch = Barrel\n   WeaponRecoilBone = PRIMARY Recoil\n   WeaponMuzzleFlash = PRIMARY MuzzleFX\n   \
WeaponFireFXBone = PRIMARY MuzzleFX\n  End\n End\n";

    /// What [`play_gun`] saw.
    struct GunRun {
        crcs: Vec<u32>,
        /// The frames the turret bone was captured with a turn (not the identity).
        turned: usize,
        /// The frames a recoil was under way, and those the muzzle flash showed on the drawn model.
        recoiling: usize,
        flashed: usize,
        /// The last frame's turret angle in the World, and the drawn turret bone's control.
        last: Option<(f32, Matrix3D)>,
        /// The largest turret angle seen, and the frame the target died (if it did).
        max_angle: f32,
        target_died: Option<u32>,
        /// The frames the drawn turret control was not exactly `Rz` of the World's angle.
        mismatched: usize,
        /// The first frame the gun's current state had its pristine bones valid and turret bones `(1, 2)` (GitHub #42),
        /// and the frames (from 0) its drawable had no render object.
        bones_found: Option<u32>,
        unmodelled: usize,
    }

    /// GitHub #27: the gate map with a drawn turreted gun (the gate's `Gun` given `GateGun` and a `Draw` block) told to
    /// attack an unarmed runner 80 to its north; 200 frames, with the drawables fed every frame when `drawn`.
    fn play_gun(drawn_client: bool) -> GunRun {
        let mut art = MemoryDevice::new();
        art.insert(b"Art/W3D/gunt.w3d", gun_model());
        play_gun_on(drawn_client, FileSystem::new(art), |_| {})
    }

    /// [`play_gun`] over `art`; `pump` runs after each frame's update (the page's part, for a ranged device).
    fn play_gun_on<D: LocalDevice>(drawn_client: bool, art: FileSystem<D>, mut pump: impl FnMut(&mut FileSystem<D>)) -> GunRun {
        let mut w = gate_map_world_with(&drawn("Gun", GUN_DRAW));
        let team = factory_find_team(&mut w, b"team1").unwrap().unwrap();
        let enemy = factory_find_team(&mut w, b"team2").unwrap().unwrap();
        let at = |x: f32, y: f32| Coord3D { x, y, z: 0.0 };
        let gun = make(&mut w, b"DrawnGun", team, at(150.0, 380.0));
        let target = make(&mut w, b"Runner", enemy, at(150.0, 460.0));
        w.pathfinder_op(rts_lockstep_engine::logic::world::pathfinder_host_world::PfOp::NewMap);
        let mut d = WorldDrawables::new(Some(Art::new(art)));
        let ev = w.take_client_events();
        let mut run = GunRun { crcs: Vec::new(), turned: 0, recoiling: 0, flashed: 0, last: None, max_angle: 0.0, target_died: None, mismatched: 0, bones_found: None, unmodelled: 0 };
        let see_bones = |d: &WorldDrawables<D>, f: u32, run: &mut GunRun| {
            let m = &d.drawables.drawables[&gun].draw_modules[0];
            if m.render_object.is_none() {
                run.unmodelled += 1;
            }
            let s = m.get_cur_state().expect("a state");
            if run.bones_found.is_none() && s.pristine_bones_valid() && s.turret_bones(0) == (1, 2) {
                run.bones_found = Some(f);
            }
        };
        if drawn_client {
            d.update(&mut w, &ev, 0);
            assert!(d.drawables.errors.is_empty(), "{:?}", d.drawables.errors);
            see_bones(&d, 0, &mut run);
            pump(&mut d.art.as_mut().unwrap().fs);
        }
        let ai = w.find_object(gun).and_then(|o| o.ai).expect("the gun's AI");
        w.module_call(gun, ai, ModuleCall::AiAttackObject { victim: target, max_shots: i32::MAX, command_source: 0 });
        for f in 1..=200u32 {
            w.logic_update().expect("a frame");
            let ev = w.take_client_events();
            if drawn_client {
                d.update(&mut w, &ev, f * 33);
                see_bones(&d, f, &mut run);
                pump(&mut d.art.as_mut().unwrap().fs);
                let m = &d.drawables.drawables[&gun].draw_modules[0];
                let r = m.render_object.as_ref().expect("the gun's render object");
                if let Some(c) = r.bone_controls.get(&1) {
                    if c.captured && c.transform != Matrix3D::IDENTITY {
                        run.turned += 1;
                    }
                    let angle = w.ai_turret_rot_and_pitch(gun, 0).map(|t| t.0);
                    run.last = angle.map(|a| (a, c.transform));
                    run.max_angle = run.max_angle.max(angle.unwrap_or(0.0));
                    let mut rz = Matrix3D::IDENTITY;
                    rz.rotate_z(angle.unwrap_or(0.0));
                    if !(c.captured && c.transform == rz) {
                        run.mismatched += 1;
                    }
                }
                if m.weapon_recoil_info_vec[0].iter().any(|x| x.state != rts_lockstep_engine::client::w3d_model_draw::RecoilState::Idle) {
                    run.recoiling += 1;
                }
                if d.instance(gun).is_some_and(|i| !i.sub_objects[3].hidden) {
                    run.flashed += 1;
                }
            }
            if run.target_died.is_none() && w.find_object(target).is_none_or(|o| o.is_effectively_dead()) {
                run.target_died = Some(f);
            }
            run.crcs.push(w.logic_crc(CRC_RECALC, None).expect("the CRC"));
        }
        run
    }

    /// GitHub #27: the drawn gun's turret follows the World's turret angle, its barrel recoils and its muzzle flashes,
    /// and the logic's CRCs are those of the same game without the client.
    ///
    /// *Predictions, written before the first run:* the CRCs are equal frame for frame; the turret turns toward the
    /// runner (a quarter turn at 60 degrees a second, about 45 frames) and is captured turned on most frames; at the
    /// end the drawn turret bone's control is exactly `Rz(angle)` of the World's angle (no art angle) and the angle is
    /// near pi/2; the gun fires within the 200 frames, so a recoil is under way on some frames and the muzzle flash
    /// shows on the drawn model on some.
    ///
    /// *Results (2026-11-06), first run:* the CRCs were equal over 200 frames and the drawn control was exactly
    /// `Rz(angle)`; turned on 199 frames, a recoil under way on 111, the flash shown on 7. **Miss:** the final angle was
    /// 0.988, not near pi/2: the runner dies after a few shots (frame 109) and the turret turns back toward its natural
    /// angle. **Second miss:** the largest angle reached was 1.512, 0.059 short of pi/2 (the logic's turret stops
    /// turning before it is exactly on the target; the logic's behaviour, not the client's). The test now checks what
    /// the client owns: on every frame the drawn turret control is exactly `Rz` of the World's angle, the turret turned
    /// past 1.4, and the target died.
    #[test]
    fn a_drawn_gun_turns_its_turret_and_recoils_without_changing_the_crcs() {
        let plain = play_gun(false);
        let r = play_gun(true);
        assert_eq!(r.crcs, plain.crcs);
        let (angle, ctl) = r.last.expect("the turret bone was controlled");
        let mut rz = Matrix3D::IDENTITY;
        rz.rotate_z(angle);
        assert_eq!(ctl, rz);
        println!("turned {} recoiling {} flashed {} final angle {angle} max {} target died {:?}", r.turned, r.recoiling, r.flashed, r.max_angle, r.target_died);
        assert_eq!(r.mismatched, 0);
        assert!(r.max_angle > 1.4, "largest angle {}", r.max_angle);
        assert!(r.target_died.is_some());
        assert!(r.turned > 100, "turned on {} frames", r.turned);
        assert!(r.recoiling > 0 && r.flashed > 0, "recoiling {} flashed {}", r.recoiling, r.flashed);
    }

    /// The gun of [`play_gun`] firing `FxGun` (the gate's gun with `FireFX = FX_Gun`), with the client's FX lists and
    /// particle systems loaded from synthetic INI text (GitHub #43): the CRCs, the systems made, where the first was
    /// placed, and the most particles alive.
    fn play_fx_gun(drawn_client: bool) -> (Vec<u32>, u32, Option<Coord3D>, u32, u32, u64) {
        let weapon = "Weapon FxGun\n PrimaryDamage = 20\n PrimaryDamageRadius = 0\n AttackRange = 100\n DamageType = ARMOR_PIERCING\n DeathType = NORMAL\n WeaponSpeed = 999999\n DelayBetweenShots = 500\n FireFX = FX_Gun\nEnd\n";
        // (`Scale = 1.0`: the gate's templates give none, and a template's asset scale is 0 until one is read, which
        // collapses the skeleton the FX bone is read from onto the origin)
        let mut w = gate_map_world_with(&format!("{weapon}{}", drawn("Gun", &format!(" Scale = 1.0\n{}", GUN_DRAW.replace("GateGun", "FxGun")))));
        let team = factory_find_team(&mut w, b"team1").unwrap().unwrap();
        let enemy = factory_find_team(&mut w, b"team2").unwrap().unwrap();
        let at = |x: f32, y: f32| Coord3D { x, y, z: 0.0 };
        let gun = make(&mut w, b"DrawnGun", team, at(150.0, 380.0));
        let target = make(&mut w, b"Runner", enemy, at(150.0, 460.0));
        w.pathfinder_op(rts_lockstep_engine::logic::world::pathfinder_host_world::PfOp::NewMap);
        let mut art = MemoryDevice::new();
        art.insert(b"Art/W3D/gunt.w3d", gun_model());
        let mut d = WorldDrawables::new(Some(Art::new(FileSystem::new(art))));
        let fx = &mut d.drawables.fx;
        fx.particles.load_text(b"ParticleSystem.ini", b"ParticleSystem MuzzleSmoke\n  Shader = ADDITIVE\n  Type = PARTICLE\n  ParticleName = EXSmoke.tga\n  Lifetime = 10 10\n  SystemLifetime = 3\n  Size = 1 1\n  BurstCount = 2 2\n  BurstDelay = 100 100\n  Color1 = R:255 G:200 B:100 0\n  VelocityDamping = 1 1\n  VelocityType = ORTHO\n  VelOrthoZ = 0.5 0.5\nEnd\n").unwrap();
        fx.fx_lists.load_text(b"FXList.ini", b"FXList FX_Gun\n  ParticleSystem\n    Name = MuzzleSmoke\n  End\n  Sound\n    Name = GunShot\n  End\nEnd\n").unwrap();
        let ev = w.take_client_events();
        if drawn_client {
            d.update(&mut w, &ev, 0);
        }
        let ai = w.find_object(gun).and_then(|o| o.ai).expect("the gun's AI");
        w.module_call(gun, ai, ModuleCall::AiAttackObject { victim: target, max_shots: i32::MAX, command_source: 0 });
        // #13: the audio, attached with the client: the FX list's `GunShot` (a WAV the test makes) played through the
        // translated manager and the device, the listener over the gun
        let mut tables = rts_lockstep_engine::client::audio_info::AudioTables::new();
        tables
            .load_text(b"a.ini", b"AudioSettings\n AudioRoot = Data\\Audio\n SoundsFolder = Sounds\n SoundsExtension = wav\n SampleCount2D = 4\n SampleCount3D = 8\n MinSampleVolume = 1%\n Default3DSoundVolume = 100%\n DefaultSoundVolume = 100%\n MicrophoneMaxPercentageBetweenGroundAndCamera = 50%\nEnd\nAudioEvent GunShot\n Sounds = gunshot\n Type = world everyone\n MinRange = 50\n MaxRange = 900\n Volume = 100%\n Limit = 3\nEnd\n")
            .expect("the audio INI");
        let mut audio = rts_lockstep_engine::client::client_audio::ClientAudio::new(rts_lockstep_engine::client::game_audio::AudioManager::new(tables));
        let mut device = crate::audio::ClientDevice::new(crate::audio::NullOutput::default());
        let shot = {
            let mut b = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x22\x56\0\0\x44\xac\0\0\x02\0\x10\0data".to_vec();
            b.extend_from_slice(&(2 * 4410u32).to_le_bytes());
            b.extend(std::iter::repeat_n(0u8, 2 * 4410));
            b
        };
        let view = crate::audio::listener_view([150.0, 400.0], 0.0, 0.95, 300.0);
        let (mut crcs, mut first, mut most, mut sounds) = (Vec::new(), None, 0, 0);
        for f in 1..=200u32 {
            w.logic_update().expect("a frame");
            let ev = w.take_client_events();
            if drawn_client {
                d.update(&mut w, &ev, f * 33);
                let fx = &mut d.drawables.fx;
                if first.is_none() {
                    first = fx.particles.systems.first().map(|s| s.get_position());
                }
                most = most.max(fx.particles.particle_count);
                let requests: Vec<_> = fx.requests.drain(..).collect();
                sounds += requests.iter().filter(|r| matches!(r, rts_lockstep_engine::client::fx_list::FxRequest::Sound { .. })).count() as u32;
                let files = |p: &[u8]| p == b"Data\\Audio\\Sounds\\gunshot.wav";
                let host = rts_lockstep_engine::client::client_audio::WorldAudioHost { world: &w, local_player_index: None, files: &files, language: b"English" };
                for e in &ev {
                    audio.apply_event(&host, &mut device, e);
                }
                for r in &requests {
                    audio.fx_request(&host, r);
                }
                audio.manager.update(&host, &mut device, &view);
                device.pump(|p| if files(p) { crate::audio::Fetch::Bytes(shot.clone()) } else { crate::audio::Fetch::Missing });
                crate::audio::AudioOutput::advance(&mut device.out, 1.0 / 30.0);
            }
            crcs.push(w.logic_crc(CRC_RECALC, None).expect("the CRC"));
        }
        (crcs, d.drawables.fx.particles.unique_system_id, first, most, sounds, device.stats.voices_played)
    }

    /// GitHub #43: the drawn gun's fire FX plays at its FX bone and makes particle systems, and the logic's CRCs are
    /// those of the same game without the client.
    ///
    /// *Predictions, written before the first run:* the CRCs are equal frame for frame; the gun fires about 5 shots
    /// (20 damage against 100 health, one shot every 15 frames) before the runner dies, each shot's `FX_Gun` handled by
    /// the draw module at `MuzzleFX` and making one `MuzzleSmoke` system: 5 systems and 5 sound requests; the first is
    /// placed at the bone's pose as made, (4, 0, 2) from the gun (the turret's 2 up, the barrel 1, the recoil bone 2
    /// and the muzzle 1 along x), so (154, 380, 2) (the turret's turn is not applied, see `client_fx`); each system
    /// emits 2 particles on its first update and lives 3 frames, its particles 10, so at most 2 alive at once (shots
    /// are 15 frames apart).
    ///
    /// *Results (2026-11-06):* the CRCs were equal over 200 frames; 5 systems, 5 sound requests, at most 2 particles.
    ///
    /// **#13 (2026-11-06), the audio attached too:** the logic's events and the FX requests go to the translated audio
    /// manager (`ClientAudio`) and the device (`crate::audio`, its silent output). *Predicted:* the CRCs still equal;
    /// each of the 5 `GunShot` requests (a positional sound about 150 from the microphone, inside its 900 range, under
    /// its limit of 3 as each 0.1 s shot ends long before the next) is played: 5 voices. *Result (first run):* as
    /// predicted, CRCs equal over 200 frames and 5 voices played.
    /// **Miss:** the first system was at the gun, (150, 380, 0): the FX bone's transform was all zeros, because the
    /// gate's templates have no `Scale` and the asset scale is then 0 (the skeleton is posed under `root.scale(0)`).
    /// With `Scale = 1.0` on the test's template (as the game's data gives its objects), see the assertion.
    #[test]
    fn a_drawn_gun_s_fire_fx_makes_particles_without_changing_the_crcs() {
        let (plain, ..) = play_fx_gun(false);
        let (crcs, systems, first, most, sounds, played) = play_fx_gun(true);
        assert_eq!(crcs, plain);
        println!("systems {systems} first {first:?} most {most} sounds {sounds} played {played}");
        assert_eq!(played, 5, "#13: each shot's GunShot played by the audio, the CRCs unchanged");
        assert_eq!(systems, 5);
        assert_eq!(sounds, 5);
        assert_eq!(first, Some(Coord3D { x: 154.0, y: 380.0, z: 2.0 }));
        assert_eq!(most, 2);
    }

    /// GitHub #51: a script game. Side 0 holds one active, one-shot script on `CONDITION_TRUE` whose actions play a
    /// sound (`PLAY_SOUND_EFFECT Blip`), change the music (`MUSIC_SET_TRACK Theme 0 0`), play a speech (`SPEECH_PLAY
    /// Hello 1`) and set the sound volume (`SOUND_SET_VOLUME 80`). 90 frames, the logic's events (the script's
    /// `ScriptAudio` ones among them) given to the translated audio manager and the device (`drawn_client`), or dropped.
    /// Returns the CRCs, the device's voices played, the script audio events seen, the manager's sound volume, the
    /// events the manager was given and the files still playing at the end.
    #[allow(clippy::type_complexity)]
    fn play_script_audio(drawn_client: bool) -> (Vec<u32>, u64, usize, f32, Vec<Vec<u8>>, Vec<Vec<u8>>) {
        use rts_lockstep_engine::common::dict::Dict;
        use rts_lockstep_engine::logic::client_event::ClientEvent;
        use rts_lockstep_engine::logic::scripts::{action_type as at, condition_type as ct, Condition, OrCondition, Parameter, Script, ScriptAction, ScriptList};
        let mut w = gate_map_world_with("");
        w.players.set_local_player(Some(1));
        let act = |t: i32, strings: &[&[u8]], ints: &[i32], reals: &[f32]| {
            let mut a = ScriptAction::new_for_read();
            a.action_type = t;
            let mut i = 0;
            for x in strings {
                let mut p = Parameter::new(0, 0);
                p.string = x.to_vec();
                a.parms[i] = Some(p);
                i += 1;
            }
            for n in ints {
                a.parms[i] = Some(Parameter::new(0, *n));
                i += 1;
            }
            for r in reals {
                let mut p = Parameter::new(0, 0);
                p.real = *r;
                a.parms[i] = Some(p);
                i += 1;
            }
            a.num_parms = i as i32;
            a
        };
        let mut c = Condition::new_for_read();
        c.condition_type = ct::CONDITION_TRUE;
        let mut sc = Script::new();
        sc.script_name = b"Audio".to_vec();
        (sc.is_active, sc.is_one_shot, sc.easy, sc.normal, sc.hard) = (true, true, true, true, true);
        sc.conditions = vec![OrCondition { conditions: vec![c] }];
        sc.actions = vec![
            act(at::PLAY_SOUND_EFFECT, &[b"Blip"], &[], &[]),
            act(at::MUSIC_SET_TRACK, &[b"Theme"], &[0, 0], &[]),
            act(at::SPEECH_PLAY, &[b"Hello"], &[1], &[]),
            act(at::SOUND_SET_VOLUME, &[], &[], &[80.0]),
        ];
        if w.sides.get_num_sides() == 0 {
            w.sides.add_side(&Dict::default());
        }
        w.sides.get_side_info_mut(0).expect("side 0").scripts = Some(ScriptList { groups: Vec::new(), scripts: vec![sc] });
        w.script_engine_new_map().expect("the engine's newMap");
        w.take_client_events();
        let mut tables = rts_lockstep_engine::client::audio_info::AudioTables::new();
        tables
            .load_text(b"a.ini", b"AudioSettings\n AudioRoot = Data\\Audio\n SoundsFolder = Sounds\n MusicFolder = Tracks\n StreamingFolder = Speech\n SoundsExtension = wav\n SampleCount2D = 4\n SampleCount3D = 8\n MinSampleVolume = 1%\n DefaultSoundVolume = 100%\n Default3DSoundVolume = 100%\n DefaultSpeechVolume = 100%\n DefaultMusicVolume = 100%\nEnd\nAudioEvent Blip\n Sounds = blip\n Type = ui everyone\n Volume = 100%\nEnd\nDialogEvent Hello\n Filename = hello.wav\n Type = ui player\n Volume = 100%\nEnd\nMusicTrack Theme\n Filename = theme.wav\n Volume = 100%\nEnd\n")
            .expect("the audio INI");
        let mut audio = rts_lockstep_engine::client::client_audio::ClientAudio::new(rts_lockstep_engine::client::game_audio::AudioManager::new(tables));
        let mut device = crate::audio::ClientDevice::new(crate::audio::NullOutput::default());
        let wav = {
            let mut b = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x22\x56\0\0\x44\xac\0\0\x02\0\x10\0data".to_vec();
            b.extend_from_slice(&(2 * 4410u32).to_le_bytes());
            b.extend(std::iter::repeat_n(0u8, 2 * 4410));
            b
        };
        let known: [&[u8]; 3] = [b"Data\\Audio\\Sounds\\blip.wav", b"Data\\Audio\\Tracks\\theme.wav", b"Data\\Audio\\Speech\\hello.wav"];
        let files = |p: &[u8]| known.contains(&p);
        let view = crate::audio::listener_view([150.0, 400.0], 0.0, 0.95, 300.0);
        let (mut crcs, mut seen) = (Vec::new(), 0);
        for _ in 0..90 {
            w.logic_update().expect("a frame");
            let ev = w.take_client_events();
            seen += ev.iter().filter(|e| matches!(e, ClientEvent::ScriptAudio(_))).count();
            if drawn_client {
                let host = rts_lockstep_engine::client::client_audio::WorldAudioHost { world: &w, local_player_index: None, files: &files, language: b"English" };
                for e in &ev {
                    audio.apply_event(&host, &mut device, e);
                }
                audio.manager.update(&host, &mut device, &view);
                device.pump(|p| if files(p) { crate::audio::Fetch::Bytes(wav.clone()) } else { crate::audio::Fetch::Missing });
                crate::audio::AudioOutput::advance(&mut device.out, 1.0 / 30.0);
            }
            crcs.push(w.logic_crc(CRC_RECALC, None).expect("the CRC"));
        }
        let vol = audio.manager.get_volume(rts_lockstep_engine::client::game_audio::AUDIO_AFFECT_SOUND);
        let log: Vec<Vec<u8>> = audio.log.iter().map(|(n, _)| n.clone()).collect();
        let playing: Vec<Vec<u8>> = device.out.playing.values().map(|v| v.0.clone()).collect();
        (crcs, device.stats.voices_played, seen, vol, log, playing)
    }

    /// GitHub #51: a script game that plays a sound, music and a speech leaves the logic's CRCs as without the client.
    ///
    /// *Predictions, written before the first run:* the CRCs are equal frame for frame; the script runs once, on the
    /// first frame, and sends 5 script audio events (the sound, the music's stop and its track, the speech, the volume);
    /// with the client, 3 voices are played (`blip.wav` 2-D, `theme.wav` as music, `hello.wav` as speech), and the
    /// manager's sound volume is the script's 0.8 (times the system's 1).
    ///
    /// *Results (2026-11-06), first run:* the CRCs equal over 90 frames, 5 events, the volume 0.8, the manager given
    /// `Blip`, `Theme`, `Hello` (handles 6, 7, 8). **Miss: 14 voices, not 3.** The music loops: the manager restarts a
    /// track when it ends (`game_audio`'s module docs: upstream sets Miles' infinite loop count), and the test's 0.1 s
    /// `theme.wav` ended and was restarted 11 times in the 3 s; `theme.wav` is still playing at the end. My prediction
    /// forgot the loop, not a defect.
    #[test]
    fn a_script_game_s_sounds_and_music_leave_the_crcs_alone() {
        let (plain, _, seen_plain, ..) = play_script_audio(false);
        let (crcs, played, seen, vol, log, playing) = play_script_audio(true);
        assert_eq!(crcs, plain);
        assert_eq!((seen, seen_plain), (5, 5));
        assert_eq!(log, [b"Blip".to_vec(), b"Theme".to_vec(), b"Hello".to_vec()]);
        assert_eq!(playing, [b"Data\\Audio\\Tracks\\theme.wav".to_vec()], "the music loops");
        assert_eq!(played, 14, "Blip, Hello, and Theme 12 times (it loops)");
        assert_eq!(vol, 0.8);
    }

    /// A tank `TANKT`: ROOTTRANSFORM and BODY pivots, a cube for the chassis and one for each tread (`TREADSL`, `TREADSR`)
    /// with a linear-offset mapper on stage 0 (GitHub #41).
    fn tank_model() -> Vec<u8> {
        let ident = [0.0, 0.0, 0.0, 1.0];
        let mut f = synth::htree_w3d(b"TANKT", &[
            PivotSpec { name: b"ROOTTRANSFORM".to_vec(), parent: -1, translation: [0.0; 3], rotation: ident },
            PivotSpec { name: b"BODY".to_vec(), parent: 0, translation: [0.0, 0.0, 1.0], rotation: ident },
        ]);
        let mut names = Vec::new();
        for (n, mapping) in [(&b"CHASSIS"[..], 0), (b"TREADSL", 4), (b"TREADSR", 4)] {
            let mut m = MeshSpec::cuboid(b"TANKT", n, [-0.5; 3], [0.5; 3]);
            m.stage0.texture = None;
            m.stage0.mapping = mapping;
            f.extend(m.to_w3d());
            names.push([&b"TANKT."[..], n].concat());
        }
        let hl: Vec<(&[u8], u32)> = names.iter().map(|n| (n.as_slice(), 1)).collect();
        f.extend(synth::hlod_w3d(b"TANKT", b"TANKT", &hl));
        f
    }

    const TANK_DRAW: &str = " Draw = W3DTankDraw ModuleTag_Draw\n  DefaultConditionState\n   Model = TankT\n  End\n  TreadAnimationRate = 3\n End\n";

    /// GitHub #41: the gate's runner drawn as a tank (`W3DTankDraw`, two treads), ordered 120 east as in [`play_demo`],
    /// 150 frames with the drawables fed every frame, and the same game without them.
    ///
    /// *Predictions, written before the first run:*
    /// - the logic's CRCs are equal frame for frame with and without the client (the new reads are `peek_*`, which write
    ///   nothing);
    /// - the runner starts facing east, so it never pivots: every change of the left tread's U is one drive step,
    ///   `-rate` wrapped (rate 3 a second, 0.10000001 a draw), on the frames it moves at 0.3 of its locomotor's speed or
    ///   more under a motive force; that is most of its ~130 frames of travel (say more than 60), and none once it has
    ///   stopped;
    /// - the drawn render model carries the left and right treads' offsets (equal, both driven the same way), none on
    ///   the chassis.
    ///
    /// *Results (2026-11-06), first run:* all held. CRCs equal over 150 frames; 129 drive steps, each exactly `-rate`
    /// wrapped, none of another size (no pivot); the last at frame 132, as the runner arrives; the drawn model's treads
    /// at U 0.09999767 (129 steps of 0.10000001 from 0, wrapped, f32 rounding included), the chassis without one.
    #[test]
    fn a_drawn_tank_scrolls_its_treads_without_changing_the_crcs() {
        let play = |drawn_client: bool| {
            let mut w = gate_map_world_with(&drawn("Runner", TANK_DRAW));
            let team = factory_find_team(&mut w, b"team1").unwrap().unwrap();
            let at = |x: f32, y: f32| Coord3D { x, y, z: 0.0 };
            let tank = make(&mut w, b"DrawnRunner", team, at(40.0, 50.0));
            w.pathfinder_op(rts_lockstep_engine::logic::world::pathfinder_host_world::PfOp::NewMap);
            let mut art = MemoryDevice::new();
            art.insert(b"Art/W3D/tankt.w3d", tank_model());
            let mut d = WorldDrawables::new(Some(Art::new(FileSystem::new(art))));
            let ev = w.take_client_events();
            if drawn_client {
                d.update(&mut w, &ev, 0);
                assert!(d.drawables.errors.is_empty(), "{:?}", d.drawables.errors);
            }
            let ai = w.find_object(tank).and_then(|o| o.ai).expect("the AI");
            w.module_call(tank, ai, ModuleCall::AiMoveToPosition { pos: at(160.0, 50.0), command_source: 0 });
            let (mut crcs, mut steps, mut odd, mut last_step) = (Vec::new(), 0usize, 0usize, None);
            let mut prev = 0.0f32;
            let mut posed = None;
            for f in 1..=150u32 {
                w.logic_update().expect("a frame");
                let ev = w.take_client_events();
                if drawn_client {
                    d.update(&mut w, &ev, f * 33);
                    let m = &d.drawables.drawables[&tank].draw_modules[0];
                    let t = m.tank.as_ref().expect("a tank draw");
                    let rate = 3.0f32 * rts_lockstep_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL;
                    let u = t.treads[0].custom_uv_offset[0];
                    if u != prev {
                        let mut want = prev - rate;
                        want -= rts_lockstep_engine::ww::wwmath::floor(want);
                        if u == want {
                            steps += 1;
                        } else {
                            odd += 1;
                        }
                        last_step = Some(f);
                    }
                    prev = u;
                    posed = d.instance(tank).map(|i| i.sub_objects.iter().map(|s| s.uv_offset).collect::<Vec<_>>());
                }
                crcs.push(w.logic_crc(CRC_RECALC, None).expect("the CRC"));
            }
            (crcs, steps, odd, last_step, posed, prev)
        };
        let (plain, ..) = play(false);
        let (crcs, steps, odd, last_step, posed, u) = play(true);
        println!("steps {steps} odd {odd} last step at {last_step:?} posed {posed:?}");
        assert_eq!(crcs, plain);
        assert_eq!(odd, 0);
        assert!(steps > 60, "{steps} drive steps");
        assert!(last_step.is_some_and(|f| f < 150));
        assert_eq!(posed, Some(vec![None, Some([u, 0.0]), Some([u, 0.0])]));
    }

    /// A smoker `SMOKER` (GitHub #49): pivots ROOTTRANSFORM and STACK (3 up), a cube on the stack, and the particle
    /// emitter `SMOKE` (a puff: 10 a second, a second's life, rising, alpha-blended `puff.tga`) on the stack too.
    fn smoker_model() -> Vec<u8> {
        let ident = [0.0, 0.0, 0.0, 1.0];
        let mut f = synth::htree_w3d(b"SMOKER", &[
            PivotSpec { name: b"ROOTTRANSFORM".to_vec(), parent: -1, translation: [0.0; 3], rotation: ident },
            PivotSpec { name: b"STACK".to_vec(), parent: 0, translation: [0.0, 0.0, 3.0], rotation: ident },
        ]);
        let mut m = MeshSpec::cuboid(b"SMOKER", b"BODY", [-0.5; 3], [0.5; 3]);
        m.stage0.texture = None;
        f.extend(m.to_w3d());
        f.extend(synth::EmitterSpec::puff(b"SMOKE").to_w3d());
        f.extend(synth::hlod_w3d(b"SMOKER", b"SMOKER", &[(b"SMOKER.BODY", 1), (b"SMOKE", 1)]));
        f
    }

    const SMOKER_DRAW: &str = " Draw = W3DModelDraw ModuleTag_Draw\n  DefaultConditionState\n   Model = Smoker\n  End\n End\n";

    /// GitHub #49: the gate's runner drawn as `SMOKER`, a model whose HLOD has a W3D particle emitter on a bone, ordered
    /// 120 east as in [`a_drawn_tank_scrolls_its_treads_without_changing_the_crcs`], 120 frames at 33 ms with the
    /// drawables fed every frame, and the same game without them.
    ///
    /// *Predictions, written before the first run:*
    /// - the logic's CRCs are equal frame for frame with and without the client (the emitters read the World's
    ///   positions only);
    /// - the emitter is made and started at the first draw; it emits one particle every 100 ms, each living a second:
    ///   once past the first second, 9 to 11 particles are drawn each frame, and every frame from the fourth (99 ms is
    ///   under one period; 132 is over) has some;
    /// - the particles are in the world: while the runner moves east the oldest one is more than half a unit west of
    ///   the runner (a trail), and all are above the stack's height minus the creation box (z over 2.5);
    /// - the client's batches (`emitter_quads`) hold them under (`puff.tga`, alpha), 4 corners and 6 indices each.
    ///
    /// *Results (2026-11-07), first run:* all held. CRCs equal over 120 frames; the first particle at frame 4, then one
    /// more every third frame, 10 from frame 31 on (9 once, at frame 100); the oldest particle west of the runner by
    /// more than half a unit on 117 frames; none below 2.5; at frame 60, one batch (`puff.tga`, alpha) of 10 quads and
    /// 60 indices.
    #[test]
    fn a_drawn_model_s_emitter_draws_particles_without_changing_the_crcs() {
        let play = |drawn_client: bool| {
            let mut w = gate_map_world_with(&drawn("Runner", SMOKER_DRAW));
            let team = factory_find_team(&mut w, b"team1").unwrap().unwrap();
            let at = |x: f32, y: f32| Coord3D { x, y, z: 0.0 };
            let runner = make(&mut w, b"DrawnRunner", team, at(40.0, 50.0));
            w.pathfinder_op(rts_lockstep_engine::logic::world::pathfinder_host_world::PfOp::NewMap);
            let mut art = MemoryDevice::new();
            art.insert(b"Art/W3D/smoker.w3d", smoker_model());
            let mut d = WorldDrawables::new(Some(Art::new(FileSystem::new(art))));
            let ev = w.take_client_events();
            if drawn_client {
                d.update(&mut w, &ev, 0);
                assert!(d.drawables.errors.is_empty(), "{:?}", d.drawables.errors);
            }
            let ai = w.find_object(runner).and_then(|o| o.ai).expect("the AI");
            w.module_call(runner, ai, ModuleCall::AiMoveToPosition { pos: at(160.0, 50.0), command_source: 0 });
            let (mut crcs, mut counts, mut trails, mut low, mut batch) = (Vec::new(), Vec::new(), 0usize, 0usize, None);
            for f in 1..=120u32 {
                w.logic_update().expect("a frame");
                let ev = w.take_client_events();
                if drawn_client {
                    d.update(&mut w, &ev, f * 33);
                    let m = d.instance(runner).expect("the smoker");
                    let e = &m.emitters[0];
                    assert!(e.failed.is_none(), "{:?}", e.failed);
                    let pts = match &e.draw {
                        Some(rts_lockstep_engine::formats::w3d::part_buf::ParticleDraw::Points(p)) => p.points.clone(),
                        other => panic!("{other:?}"),
                    };
                    counts.push(pts.len());
                    let x = w.find_object(runner).unwrap().get_position().x;
                    if pts.first().is_some_and(|p| p.position.x < x - 0.5) {
                        trails += 1;
                    }
                    low += pts.iter().filter(|p| p.position.z <= 2.5).count();
                    if f == 60 {
                        let q = crate::particles::emitter_quads(d.instances(), crate::particles::point_tables(), [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
                        batch = Some((q.keys().cloned().collect::<Vec<_>>(), q.values().map(|b| (b.len(), b.indices.len())).collect::<Vec<_>>(), pts.len()));
                    }
                }
                crcs.push(w.logic_crc(CRC_RECALC, None).expect("the CRC"));
            }
            (crcs, counts, trails, low, batch)
        };
        let (plain, ..) = play(false);
        let (crcs, counts, trails, low, batch) = play(true);
        println!("counts {counts:?} trails {trails} low {low} batch {batch:?}");
        assert_eq!(crcs, plain);
        assert!(counts[3..].iter().all(|&n| n > 0), "{counts:?}");
        assert!(counts[31..].iter().all(|&n| (9..=11).contains(&n)), "{counts:?}");
        assert!(trails > 30, "{trails}");
        assert_eq!(low, 0);
        let (keys, sizes, n) = batch.expect("frame 60");
        assert_eq!(keys, vec![(b"puff.tga".to_vec(), crate::particles::Blend::Alpha)]);
        assert_eq!(sizes, vec![(n, 6 * n)]);
    }

    /// A truck `TRUCKT`: pivots ROOTTRANSFORM (0), TIREFL (1), TIREFR (2), TIRERL (3), TIRERR (4), a cube on each (GitHub #41).
    fn truck_model() -> Vec<u8> {
        let ident = [0.0, 0.0, 0.0, 1.0];
        let p = |n: &[u8], parent: i32, t: [f32; 3]| PivotSpec { name: n.to_vec(), parent, translation: t, rotation: ident };
        let mut f = synth::htree_w3d(b"TRUCKT", &[
            p(b"ROOTTRANSFORM", -1, [0.0; 3]),
            p(b"TIREFL", 0, [2.0, 1.0, 0.0]),
            p(b"TIREFR", 0, [2.0, -1.0, 0.0]),
            p(b"TIRERL", 0, [-2.0, 1.0, 0.0]),
            p(b"TIRERR", 0, [-2.0, -1.0, 0.0]),
        ]);
        let mut names = Vec::new();
        for (n, bone) in [(&b"CHASSIS"[..], 0u32), (b"TIREFL", 1), (b"TIREFR", 2), (b"TIRERL", 3), (b"TIRERR", 4)] {
            let mut m = MeshSpec::cuboid(b"TRUCKT", n, [-0.5; 3], [0.5; 3]);
            m.stage0.texture = None;
            f.extend(m.to_w3d());
            names.push(([&b"TRUCKT."[..], n].concat(), bone));
        }
        let hl: Vec<(&[u8], u32)> = names.iter().map(|(n, b)| (n.as_slice(), *b)).collect();
        f.extend(synth::hlod_w3d(b"TRUCKT", b"TRUCKT", &hl));
        f
    }

    const TRUCK_DRAW: &str = " Draw = W3DTruckDraw ModuleTag_Draw\n  DefaultConditionState\n   Model = TruckT\n  End\n  \
LeftFrontTireBone = TireFL\n  RightFrontTireBone = TireFR\n  LeftRearTireBone = TireRL\n  RightRearTireBone = TireRR\n  \
TireRotationMultiplier = 0.2\n End\n";

    /// A wheeled locomotor for the gate's runner: the gate's speeds, `FOUR_WHEELS`, with a suspension.
    const GATE_WHEELS: &str = "Locomotor GateWheels\n Surfaces = GROUND\n Speed = 30\n Acceleration = 100\n Braking = 100\n TurnRate = 180\n \
Appearance = FOUR_WHEELS\n HasSuspension = Yes\n FrontWheelTurnAngle = 20\n MaximumWheelExtension = -2.3\n PitchStiffness = 0.1\n \
RollStiffness = 0.1\n PitchDamping = 0.4\n RollDamping = 0.4\n ForwardAccelerationPitchFactor = 0.1\n LateralAccelerationRollFactor = 0.1\n \
UniformAxialDamping = 1.0\n BounceAmount = 30\n AccelerationPitchLimit = 10\n DecelerationPitchLimit = 10\nEnd\n";

    /// GitHub #41: the gate's runner on wheels drawn as a truck (`W3DTruckDraw`, four tire bones, `TireRotationMultiplier`
    /// 0.2), from (40, 50) facing east ordered to (100, 150), so it turns left as it drives; 200 frames with the drawables
    /// fed every frame, and the same game without them.
    ///
    /// *Predictions, written before the first run:*
    /// - the logic's CRCs are equal frame for frame with and without the client (the new reads are `peek_*` and `&World`
    ///   queries, which write nothing; the bounce's random is the client's own generator);
    /// - the drawable has a physics transform on every drawn frame (a `FOUR_WHEELS` locomotor), and the drawable's wheel
    ///   info from the first;
    /// - the front tires spin by 0.2 times the speed each draw: the final rotation is 0.2 times the distance driven
    ///   (summed frame by frame from the positions; within 5%, the speed being the velocity's and the distance the
    ///   position's), which is at least the straight line, 116.6;
    /// - the runner turns left (`TURN_POSITIVE`) to face its goal: on some frame the front tire's control is steered left
    ///   (its Z rotation above 0.01), never more than the turn angle, 20 degrees;
    /// - accelerating from rest pitches the chassis nose up: the total pitch is below 0 on some frame of the first ten;
    /// - the posed model's front-left tire is not where it was made (its control is on the drawn model).
    ///
    /// *Results (2026-11-06), first run:* all held. CRCs equal over 200 frames; a physics transform and wheel info on all
    /// 200 frames; 195 units driven (the path is longer than the straight line's 116.6), front tires at 39.000076 rad
    /// (0.2 x 195, within f32 rounding); the steering reached 0.34906572, the full 20 degrees, and never right of 0;
    /// the lowest pitch of the first ten frames -0.0847; the drawn tire moved.
    #[test]
    fn a_drawn_truck_spins_and_steers_its_wheels_without_changing_the_crcs() {
        let extra = format!("{GATE_WHEELS}{}", drawn("Runner", TRUCK_DRAW).replace("GateTreads", "GateWheels"));
        let play = |drawn_client: bool| {
            let mut w = gate_map_world_with(&extra);
            let team = factory_find_team(&mut w, b"team1").unwrap().unwrap();
            let at = |x: f32, y: f32| Coord3D { x, y, z: 0.0 };
            let truck = make(&mut w, b"DrawnRunner", team, at(40.0, 50.0));
            w.pathfinder_op(rts_lockstep_engine::logic::world::pathfinder_host_world::PfOp::NewMap);
            let mut art = MemoryDevice::new();
            art.insert(b"Art/W3D/truckt.w3d", truck_model());
            let mut d = WorldDrawables::new(Some(Art::new(FileSystem::new(art))));
            let ev = w.take_client_events();
            if drawn_client {
                d.update(&mut w, &ev, 0);
                assert!(d.drawables.errors.is_empty(), "{:?}", d.drawables.errors);
            }
            let ai = w.find_object(truck).and_then(|o| o.ai).expect("the AI");
            w.module_call(truck, ai, ModuleCall::AiMoveToPosition { pos: at(100.0, 150.0), command_source: 0 });
            let start = w.find_object(truck).unwrap().get_position();
            let (mut crcs, mut xformed, mut max_steer, mut min_steer, mut early_pitch, mut wheel_frames) = (Vec::new(), 0, 0.0f32, 0.0f32, 0.0f32, 0);
            let (mut rotation, mut distance, mut prev_pos, mut moved_tire) = (0.0f32, 0.0f32, start, false);
            for f in 1..=200u32 {
                w.logic_update().expect("a frame");
                let ev = w.take_client_events();
                let pos = w.find_object(truck).unwrap().get_position();
                distance += rts_lockstep_engine::ww::wwmath::sqrt((pos.x - prev_pos.x) * (pos.x - prev_pos.x) + (pos.y - prev_pos.y) * (pos.y - prev_pos.y));
                prev_pos = pos;
                if drawn_client {
                    d.update(&mut w, &ev, f * 33);
                    let dr = &d.drawables.drawables[&truck];
                    if let Some(x) = dr.physics_xform {
                        xformed += 1;
                        if f <= 10 {
                            early_pitch = early_pitch.min(x.total_pitch);
                        }
                    }
                    if dr.get_wheel_info().is_some() {
                        wheel_frames += 1;
                    }
                    let m = &dr.draw_modules[0];
                    let Some(rts_lockstep_engine::client::w3d_truck_draw::TruckKind::Truck(t)) = &m.truck else { panic!("a truck draw") };
                    rotation = t.front_wheel_rotation;
                    if let Some(c) = m.render_object.as_ref().unwrap().bone_controls.get(&1) {
                        // the steering is the Z rotation before the spin: read it from the X axis's heading in the XY plane
                        // of the control with the spin about Y taken out (Rotate_Y keeps Y's axis; Z rotation turns it)
                        let yx = c.transform.row[0][1];
                        let yy = c.transform.row[1][1];
                        let steer = -rts_lockstep_engine::ww::wwmath::atan2(yx, yy);
                        max_steer = max_steer.max(steer);
                        min_steer = min_steer.min(steer);
                    }
                    if let Some(i) = d.instance(truck) {
                        let tire = i.sub_objects.iter().position(|s| s.name.eq_ignore_ascii_case(b"TRUCKT.TIREFL")).unwrap();
                        let x = i.sub_object_transform(tire).unwrap();
                        if (x.row[0][0] - 1.0).abs() > 1e-3 {
                            moved_tire = true;
                        }
                    }
                }
                crcs.push(w.logic_crc(CRC_RECALC, None).expect("the CRC"));
            }
            (crcs, xformed, wheel_frames, rotation, distance, max_steer, min_steer, early_pitch, moved_tire)
        };
        let (plain, ..) = play(false);
        let (crcs, xformed, wheel_frames, rotation, distance, max_steer, min_steer, early_pitch, moved_tire) = play(true);
        println!(
            "xformed {xformed} wheel frames {wheel_frames} rotation {rotation} distance {distance} steer [{min_steer}, {max_steer}] early pitch {early_pitch} tire moved {moved_tire}"
        );
        assert_eq!(crcs, plain);
        assert_eq!((xformed, wheel_frames), (200, 200));
        assert!(distance >= 116.0, "{distance}");
        assert!((rotation - 0.2 * distance).abs() < 0.2 * distance * 0.05, "{rotation} for {distance}");
        assert!(max_steer > 0.01 && max_steer <= 20.0 * core::f32::consts::PI / 180.0 + 1e-4, "{max_steer}");
        assert!(early_pitch < 0.0, "{early_pitch}");
        assert!(moved_tire);
    }

    /// GitHub #42: the drawn gun of [`play_gun`] with its model read by range, the page's part played by the test. Before
    /// the fix, the module data's states were validated at frame 0 while `gunt.w3d` was being fetched, marked valid with
    /// no bones, and never looked at again: the remade drawable had no turret, recoil or muzzle-flash bones for good.
    ///
    /// *Predictions, written before the first run:*
    /// - with the files at hand the turret bones are found at frame 0 and the gun always has a render object (as
    ///   [`a_drawn_gun_turns_its_turret_and_recoils_without_changing_the_crcs`] already relies on);
    /// - ranged: at frame 0 the gun has no render object and its state's bones are not valid; the bytes arrive after
    ///   frame 0, the drawable is made again at frame 1 (one remade), and the bones are found at frame 1;
    /// - from then on the ranged run is the at-hand run's: the turret control is `Rz` of the World's angle on every
    ///   frame, the barrel recoils and flashes on the same number of frames, and the logic's CRCs equal the plain game's.
    ///
    /// *Results (2026-11-06), first run:* all held. At hand: bones at frame 0, a render object on every frame. Ranged:
    /// one frame without a render object, the bones found at frame 1; turned 199, recoiling 111, flashed 7 (the at-hand
    /// numbers); the drawn control exactly `Rz` on every frame; CRCs equal over 200 frames. **The defect reproduced:**
    /// with `Deferred::deferred_answers` made to answer 0 (the behaviour before the fix) the same test fails: bones never
    /// found, the turret never turned (0 frames), and the muzzle flash shown on all 200 frames (never hidden: no bone).
    #[test]
    fn a_ranged_gun_finds_its_bones_once_its_model_arrives() {
        let plain = play_gun(false);
        let at_hand = play_gun(true);
        let (art, big) = ranged(vec![(b"Art\\W3D\\gunt.w3d", gun_model())]);
        let r = play_gun_on(true, art, |fs| {
            for (key, o, l) in fs.local.device.take_wanted() {
                fs.local.device.provide(key, o, big[o..o + l].to_vec());
            }
        });
        println!(
            "at hand: bones at {:?}, unmodelled {}; ranged: bones at {:?}, unmodelled {}, turned {} recoiling {} flashed {}",
            at_hand.bones_found, at_hand.unmodelled, r.bones_found, r.unmodelled, r.turned, r.recoiling, r.flashed
        );
        assert_eq!((at_hand.bones_found, at_hand.unmodelled), (Some(0), 0));
        assert_eq!((r.bones_found, r.unmodelled), (Some(1), 1));
        assert_eq!(r.mismatched, 0);
        assert_eq!((r.recoiling, r.flashed), (at_hand.recoiling, at_hand.flashed));
        assert_eq!(r.crcs, plain.crcs);
    }

    /// The generated art of [`synthetic_art`] in a `W3DZH.big` after a 4 MiB member nobody opens, on a ranged device as the
    /// page gives it (the archive's length and the first 64 KiB, which hold its directory), and the archive itself.
    fn ranged_art() -> (FileSystem<RangedDevice>, Vec<u8>) {
        ranged(vec![
            (b"Art\\W3D\\runner.w3d", model(b"RUNNER", &[b"IDLE"])),
            (b"Art\\W3D\\runnermv.w3d", model(b"RUNNERMV", &[b"ROLL"])),
            (b"Art\\W3D\\soldier.w3d", model(b"SOLDIER", &[b"IDLE"])),
            (b"Art\\W3D\\soldierfr.w3d", model(b"SOLDIERFR", &[b"FIRE"])),
        ])
    }

    /// `files` in a `W3DZH.big` after a 4 MiB member nobody opens, on a ranged device as the page gives it (see
    /// [`ranged_art`]), and the archive itself.
    fn ranged(models: Vec<(&'static [u8], Vec<u8>)>) -> (FileSystem<RangedDevice>, Vec<u8>) {
        let filler = vec![7u8; 4 << 20];
        let mut files = vec![(&b"Art\\W3D\\zzfiller.w3d"[..], filler)];
        files.extend(models);
        let members: Vec<(&[u8], &[u8])> = files.iter().map(|(p, b)| (*p, b.as_slice())).collect();
        let big = build_big(&members);
        let prefix = ArchiveFile::directory_prefix_needed(big.len(), &big[..big.len().min(64 * 1024)]);
        assert!(prefix <= 64 * 1024, "a short directory: the first read holds it");
        let mut dev = RangedDevice::new();
        dev.insert_ranged(b"W3DZH.big", big.len(), 3, big[..big.len().min(64 * 1024)].to_vec());
        let mut fs = FileSystem::new(dev);
        fs.init(None);
        assert!(fs.archive_errors.is_empty());
        (fs, big)
    }

    /// GitHub #24: the same demo with its models read by range from an archive, the page's part played by the test
    /// (after each frame every wanted range is read from the archive and handed back, as `web/index.html` does).
    ///
    /// *Predictions, written before the first run:*
    /// - at frame 0 no object has a model: the runner's and the soldiers' files are being fetched (two ranges, the
    ///   soldier's asked once for both soldiers), the modules have no render object and no animation, and nothing is
    ///   registered missing;
    /// - their bytes arrive after frame 0, so at frame 1 the waiting drawables are made again (three) and all three
    ///   objects are posed, the runner with `RUNNER.IDLE`;
    /// - the runner's moving model is fetched when it first moves, so it shows one frame later than with the files at
    ///   hand (frame 3 instead of 2), and everything else holds as in the memory run: the roll runs, the runner is back
    ///   to its idle model, both soldiers fire;
    /// - the device ends holding the first 64 KiB and the four models' ranges (four ranges arrived), and nothing of the
    ///   4 MiB filler past the first 64 KiB.
    ///
    /// *Results (2026-11-05):* all held but one. Frame 0 shapes, all three posed at frame 1, the moving model at frame 3
    /// (2 with the files at hand), four ranges of 1,713 bytes read from a 4,201,315-byte archive (held 72,388), the CRCs
    /// equal. **Miss:** 6 drawables were made again, not 3: each later fetch (the runner's moving model, the soldiers'
    /// firing model, for both soldiers) remakes the drawables waiting on it too, as designed. The assertion is now 6.
    ///
    /// ~~The moving model at frame 3; 6 remade.~~ **Corrected 2026-11-06 (#27):** with the bone caches translated, each
    /// draw module's constructor validates every state of its mode (`validateCachedBones` makes each state's render
    /// object, as upstream's does), so the four models are all asked for at frame 0: 3 drawables remade (at frame 1),
    /// and the moving model shows at frame 2, as with the files at hand. Not predicted before the run: found as a
    /// failure of the old assertions.
    #[test]
    fn the_demo_reads_its_models_by_range_and_draws_shapes_until_they_arrive() {
        let (art, big) = ranged_art();
        let models: usize = ["runner.w3d", "runnermv.w3d", "soldier.w3d", "soldierfr.w3d"]
            .iter()
            .map(|m| art.archives.member_location(format!("Art\\W3D\\{m}").as_bytes()).unwrap().2)
            .sum();
        let mut ranges = Vec::new();
        let mut stats = None;
        let r = play_demo(art, |fs| {
            for (key, o, l) in fs.local.device.take_wanted() {
                ranges.push((key, o, l));
                fs.local.device.provide(key, o, big[o..o + l].to_vec());
            }
            stats = Some(fs.local.device.stats());
        });
        println!("ranges read: {ranges:?}; {stats:?}; archive {} bytes; remade {}", big.len(), r.remade);
        assert_eq!(r.first_posed, [Some(1), Some(1), Some(1)]);
        // three at frame 1 (the runner and both soldiers), then one each time a later model arrived: the runner's moving
        // model, and the soldiers' firing model (both soldiers)
        // ~~assert_eq!(r.remade, 6)~~ Corrected 2026-11-06 (#27): 3. The draw modules' constructors now validate every
        // state's bones (`validateCachedBones`, `Create_Render_Obj` of each state's model, as upstream), so all four
        // models are asked for at frame 0 and arrive together.
        assert_eq!(r.remade, 3);
        let stats = stats.unwrap();
        assert_eq!((stats.ranges_arrived, stats.bytes_arrived), (4, models));
        assert_eq!(stats.held, 64 * 1024 + models);
        let at_hand = play_demo(synthetic_art(), |_| {});
        // ~~(2, 3), one frame later~~ Corrected 2026-11-06 (#27): (2, 2), the moving model was fetched at frame 0 (above).
        assert_eq!((at_hand.moved, r.moved), (2, 2), "the moving model is at hand when the runner moves");
        // the client never changes the simulation: the same logic CRCs whether the art is at hand or fetched
        assert_eq!(r.crcs, at_hand.crcs);
        assert!(r.roll_frames > 3);
        assert!(r.back.is_some());
        assert_eq!(r.fired, [true, true]);
    }
}
