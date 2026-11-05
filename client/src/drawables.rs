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
//! render model or texture whose read waited is not kept, and is made again on a later frame.
//!
//! **Time.** The draw modules' sync time is the client's clock in milliseconds ([`WorldDrawables::update`]'s
//! `sync_time`), as `WW3D::Sync` is upstream: animations run in real time, not in logic frames.

use std::collections::{BTreeMap, BTreeSet};

use rts_lockstep_engine::client::drawable::ClientDrawables;
use rts_lockstep_engine::client::w3d_model_draw::{AnimHandle, DrawEnv, W3dAssetSource, W3dFiles};
use rts_lockstep_engine::common::file_system::{FileSystem, LocalDevice};
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::formats::w3d::assetmgr::{AssetManager, W3dFileFactory};
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
        }
        r
    }
    fn get_hanim(&mut self, name: &[u8]) -> Option<AnimHandle> {
        if !anim_ready(self.files.assets, self.files.fs, name, !self.models.is_empty()) {
            self.anims.insert(name.to_vec());
            return None;
        }
        self.files.get_hanim(name)
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
                let mut src = Deferred { files: W3dFiles { assets: &mut art.assets, fs: &art.fs }, models: &mut self.waiting_models, anims: &mut self.waiting_anims };
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
                let mut src = Deferred { files: W3dFiles { assets: &mut art.assets, fs: &art.fs }, models: &mut self.waiting_models, anims: &mut self.waiting_anims };
                self.drawables.draw(w, &mut src, sync_time)
            }
            None => self.drawables.draw(w, &mut NoArt, sync_time),
        }
        self.pose();
    }

    /// The waiting names whose files have arrived are dropped; if any was, each drawable with a module lacking its
    /// model, or its animation, is made again (`ClientDrawables::create_drawable`), given its flags back
    /// (`replaceModelConditionFlags`, so its modules choose on the next draw as they would have) and its hidden state.
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
                        Some(r) => r.is_hlod && r.anim.is_none() && !cur.animations.is_empty(),
                    }
                })
            })
            .map(|(id, _)| *id)
            .collect();
        for id in incomplete {
            let Some(old) = self.drawables.drawables.remove(&id) else { continue };
            let mut src = Deferred { files: W3dFiles { assets: &mut art.assets, fs: &art.fs }, models: &mut self.waiting_models, anims: &mut self.waiting_anims };
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
            [0.0, 0.0, h.sin(), h.cos()]
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

    /// The generated art of [`synthetic_art`] in a `W3DZH.big` after a 4 MiB member nobody opens, on a ranged device as the
    /// page gives it (the archive's length and the first 64 KiB, which hold its directory), and the archive itself.
    fn ranged_art() -> (FileSystem<RangedDevice>, Vec<u8>) {
        let filler = vec![7u8; 4 << 20];
        let files = [
            (&b"Art\\W3D\\zzfiller.w3d"[..], filler),
            (b"Art\\W3D\\runner.w3d", model(b"RUNNER", &[b"IDLE"])),
            (b"Art\\W3D\\runnermv.w3d", model(b"RUNNERMV", &[b"ROLL"])),
            (b"Art\\W3D\\soldier.w3d", model(b"SOLDIER", &[b"IDLE"])),
            (b"Art\\W3D\\soldierfr.w3d", model(b"SOLDIERFR", &[b"FIRE"])),
        ];
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
        assert_eq!(r.remade, 6);
        let stats = stats.unwrap();
        assert_eq!((stats.ranges_arrived, stats.bytes_arrived), (4, models));
        assert_eq!(stats.held, 64 * 1024 + models);
        let at_hand = play_demo(synthetic_art(), |_| {});
        assert_eq!((at_hand.moved, r.moved), (2, 3), "one frame later than with the files at hand");
        // the client never changes the simulation: the same logic CRCs whether the art is at hand or fetched
        assert_eq!(r.crcs, at_hand.crcs);
        assert!(r.roll_frames > 3);
        assert!(r.back.is_some());
        assert_eq!(r.fired, [true, true]);
    }
}
