#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp
//   (getPristineBonePositionsForConditionState, getProjectileLaunchOffset, getBarrelCount, findBestInfo, and the bone
//   caches: addPublicBone, findSingleBone, findSingleSubObj, doSingleBoneName, validateStuff, validateCachedBones,
//   validateWeaponBarrelInfo, validateTurretInfo, findPristineBone, findPristineBonePos)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Drawable.cpp (getPristineBonePositions, getProjectileLaunchOffset,
//   getBarrelCount)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp (getSingleLogicalBonePosition,
//   getSingleLogicalBonePositionOnTurret, getMultiLogicalBonePosition)
//   GeneralsMD/Code/GameEngine/Source/Common/Thing/Thing.cpp (convertBonePosToWorldPos)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/rendobj.cpp (Get_Sub_Object_By_Name), htree.cpp (Scale,
//   Base_Update), W3DAssetManager.cpp (Create_Render_Obj's scaling)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! GitHub #175: the logic's bone and launch-offset answers with `live_bones` on, against the upstream text compiled
//! verbatim (`differential/wrap_logic_bones.cpp`, which lists the cuts and the stand-ins).
//!
//! # Methodology
//!
//! One game World (the gate's, every switch on, `live_bones` included) holds two objects with drawables: `BoneGun`,
//! whose AI has a main turret (`NaturalTurretAngle = 30`), and `BonePlain`, whose AI has none. Each case:
//!
//! - **The art.** Three random models `M0`..`M2`, each an HLOD (80 %) or a mesh. Each has 0 to 6 pivots named from a
//!   pool (numbered and unnumbered, mixed case, duplicates, names the queries ask for). Each pivot is a random pose:
//!   70 % a turn about z with a translation, 30 % entries in -2..2 and translations in -50..50. Each pivot's parent is
//!   an earlier pivot. Each model has 0 to 3 sub-objects named from a pool (`<model>.NAME` or a bare name, some the
//!   pivots' names), each on a random pivot. There are two animations, `A0` and `A1` (1 to 4 frames, a random
//!   translation step per pivot).
//! - **The template's draw modules.** One to three `W3DModelDraw` modules, each with:
//!   - an ignore mask and an `AttachToBoneInAnotherModule` name (sometimes);
//!   - up to two extra public bones;
//!   - one to four states. Each state has one or two condition sets over the first 12 bits (the first state's first
//!     set is empty), a model (`M0`..`M2`, `None`, or a missing one) and `Flags` (`PRISTINE_BONE_POS_IN_FINAL_FRAME`
//!     30 %). It also has an animation (none, `A0`, `A1`, or a missing one), up to two public bones, each slot's four
//!     weapon bone names (half empty), and two turrets with their angle and pitch bones and art angle and pitch.
//!
//!   The Rust builds the module data as the parse leaves it (names lower-cased, `None` cleared, `addPublicBone` of each
//!   weapon bone and turret name). It validates the states' caches from the art at the case's scale (as
//!   `World::load_art_bones` does, `drawable_bones::validate_template_art`) and installs them as both templates' art
//!   (`World::art_bones`). The C++ is given the same words and validates when asked, as upstream does.
//! - **The scale.** 40 % a power of two, 20 % at the edge of `scale_epsilon` (0.9899 .. 1.0101), 40 % any value in
//!   0.5 .. 2.
//! - **The object.** One of the two objects, its transform a random turn about z (and, 1 in 5, skew entries) at a
//!   random place (`World::set_transform_matrix`; the C++ gets the transform the World holds, its position and its
//!   cached angle `Get_Z_Rotation`). Its turret's rotation is the AI's (`World::ai_turret_rot_and_pitch`), and its
//!   drawable's flags are random.
//! - **The operations.** 1 to 12 of them:
//!   - `Drawable::getPristineBonePositions` through `World::drawable_art`, at the level of the caller's arrays: each
//!     array given or `NULL`, 8 entries pre-filled with a sentinel, a start index of 0, 1, 2 or 98, a maximum of
//!     -1 .. 8. All 8 entries are compared, the miss's write of the object's transform included;
//!   - `Object::getSingleLogicalBonePosition` (`World::object_get_single_logical_bone_transform`);
//!   - `Object::getMultiLogicalBonePosition` (`object_weapons::get_multi_logical_bone_position` over the World): each
//!     array given or not, converted to the world or not, the bones found compared;
//!   - `Object::getSingleLogicalBonePositionOnTurret(TURRET_MAIN)` (`object_weapons`' over the World);
//!   - `Drawable::getProjectileLaunchOffset` through `World::drawable_art`: a slot, a barrel of -1 .. 5, a turret of
//!     -1, 0 or 1, each output given or `NULL` and pre-filled with a sentinel;
//!   - `Drawable::getBarrelCount` (`WeaponHost::barrel_count` of the World);
//!   - a change of the drawable's flags.
//!
//! Every word is compared as bits. A turret query whose answer reads a rotation that `getTurretRotAndPitch` did not
//! write (`BonePlain`, a bone found) is undefined upstream. It is excluded and counted (both sides record `-1`).
//!
//! # Predictions (written before the first run, `issue175b_predictions.md`)
//!
//! 20,000 cases, 0 mismatches, max ULP 0. A few hundred turret queries are excluded as undefined. Every plant
//! reachable here is caught (9745 and 9750 by the lib tests).
//!
//! # Results (2026-11-13)
//!
//! - **First build:** the wrapper lacked `Common/Debug.h` (the shim's `DEBUG_CRASH`, `DEBUG_ASSERTCRASH`, which drop
//!   their arguments, as the release build's do). The wrapper's error.
//! - **First run:** the test read an operation's argument as a name where it was a mask (an index out of range). The
//!   test's error.
//! - **Second run (300 cases):** 0 mismatches, but only 17 bones were found. The generator rarely named a bone the
//!   art had and cached. The names and the models were weighted towards each other, with no change to either side's
//!   code.
//! - **The gated run:** **20,000 cases, 129,837 operations, 0 mismatches, max ULP 0, NaN-payload-only 0.** 609 turret queries
//!   were undefined upstream and excluded (both sides recorded -1). The C++ answers found:
//!   - 1,928 bones by the raw pristine queries;
//!   - 2,836 single logical bones;
//!   - 1,639 bones by the multi queries;
//!   - 1,814 bones on the turret;
//!   - 9,103 launch offsets;
//!   - 4,234 barrel counts above 0.
//!
//!   The predictions held, but the undefined count was 609, not "a few hundred".
//! - **Plants** (mismatching cases of 20,000):
//!
//!   | id | caught | id | caught | id | caught |
//!   |---|---:|---|---:|---|---:|
//!   | 9721 | 4,506 | 9722 | 11,728 | 9723 | 5,364 |
//!   | 9724 | 282 | 9725 | 103 | 9726 | 196 |
//!   | 9727 | 906 | 9728 | 2,497 | 9729 | 2,891 |
//!   | 9730 | 287 | 9731 | 59 | 9733 | 3,297 |
//!   | 9734 | 1,425 | 9735 | 2,884 | 9737 | 1,632 |
//!   | 9738 | 169 | 9739 | 960 | 9740 | 6,147 |
//!   | 9742 | 5,454 | 9743 | 2,077 | 9744 | 14,877 |
//!   | 9746 | 1,074 | 9747 | 334 | 9748 | 286 |
//!   | 9749 | 685 | 9750 | 3,539 | | |
//!
//!   - 9745 (the flags copy) is caught by the lib test `issue175_the_logic_s_flags_choose_the_state`. 9750 is also caught by
//!     `issue175_a_game_s_weapon_fires_from_the_muzzle_bone` and `issue175_the_logic_s_bones_against_upstream_s`.
//!   - So 27 were planted, and all 27 were caught.
//!   - 9732 and 9736 are not used.
//!   - 9741 was planted, then found to be unobservable here: no logic answer shows the barrel's FX bone. It was removed,
//!     not counted.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints `MEASURED` lines and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 9721 | the pristine query ignores the flags (the default state) |
//! | 9722 | a miss writes the identity, not the object's transform |
//! | 9723 | the bone name is not lower-cased |
//! | 9724 | `startIndex` 0 reads the numbered bones too |
//! | 9725 | `maxBones` lets one more bone in |
//! | 9726 | the drawable does not lower `maxBones` by a module's count |
//! | 9727 | the drawable does not move the transforms on |
//! | 9728 | the launch offset is not turned by the turret's art angle |
//! | 9729 | the turret's art pitch turns the other way |
//! | 9730 | ~~the attach bone is not turned by the drawable's orientation~~ (retired 2026-11-13, #178: the undefined branch's) |
//! | 9731 | a barrel out of range is the last, not the first |
//! | 9732 | (unused) |
//! | 9733 | the drawable asks only its first module for the launch offset |
//! | 9734 | the barrel count is the last module's |
//! | 9735 | `convertBonePosToWorldPos` leaves the position in the model |
//! | 9736 | (unused) |
//! | 9737 | the turret's twist is the other way |
//! | 9738 | the turret's twist is not about the turret's bone |
//! | 9739 | `PRISTINE_BONE_POS_IN_FINAL_FRAME` is ignored |
//! | 9740 | the caches are made at scale 1 |
//! | 9741 | (unused: a barrel's FX bone is read only by the client's `handleWeaponFireFX`, which no logic answer shows) |
//! | 9742 | no barrel from the unadorned names |
//! | 9743 | no bone found as a sub-object |
//! | 9744 | `Get_Bone_Index` 0 counts as found |
//! | 9745 | (lib test) the flags copy ignores the body's damage state |
//! | 9746 | ~~the attach bone's offset is not added to the launch position~~ (retired 2026-11-13, #178: the undefined branch's) |
//! | 9747 | the drawable does not move the positions on |
//! | 9748 | the extra public bones are not cached |
//! | 9749 | the standard public bones are not cached |
//! | 9750 | (lib test) the weapon host ignores `live_bones` |
//! | 9781 | (#178) the attach offset is not added to the turret's rotation position |
//! | 9782 | (#178) the attach offset is not added to the turret's pitch position |
//! | 9783 | (#178) no cache: the asking drawable's attach bone is read at every call |
//! | 9784 | (#178) the attach offset is added to the launch position too (the undefined branch's adjust kept) |
//! | 9785 | (#178) the attach offset is added to the rotation position only when its bone key is valid |
//! | 9786 | (#178) the load's fill asks only the module's own bones, not the drawable's walk over every module |
//! | 9787 | (#178) the load does not fill the cache: the first query's drawable fills it, with its own flags |
//!
//! # GitHub #178: `CACHE_ATTACH_BONE`
//!
//! **Methodology.** ~~The wrapper compiled `getProjectileLaunchOffset` without `CACHE_ATTACH_BONE`~~ **Corrected
//! 2026-11-13 (#178):** W3DModelDraw.h:278 defines it, so upstream adds no tech offset to the launch position
//! (3306-3319 and 3346-3349 are the undefined branch, which #175 translated and the wrapper compiled, so the two agreed
//! on the branch upstream does not build); with a turret it adds the module data's cached attach-bone offset
//! (`getAttachToDrawableBoneOffset`, 1146-1174), unturned, to both turret positions (3362-3395). The wrapper now
//! includes #174's cuts of the define and of `getAttachToDrawableBoneOffset`, and keeps `None` as the parse does. The
//! cache's first asker is the port's (`drawable_bones::fill_attach_to_drawable_bone_offsets`: a drawable of the
//! template as made, no flags, at load); the wrapper asks upstream's function the same way after building the case,
//! then sets the case's flags. A second test, `the_attach_bone_offset_matches_upstream` (seed `0x178_0001`), names an
//! attach bone the art may have in every module and makes 2 in 5 operations launch offsets; the main test's draws are
//! #175's.
//!
//! **Predictions** (`issue178_predictions.md`, before the runs): with the define on and the Rust unchanged, the main
//! test mismatches in 1,100 - 1,700 cases (the 1,074 plant 9746 measured, plus turret-only cases) and the attach test
//! in 30 - 60 % of cases; after the fix 0 in both, every plant 9781-9787 caught.
//!
//! **Results (2026-11-13).**
//! - Define on, the Rust still on the undefined branch: main **1,306** of 20,000 cases, attach **7,180** of 20,000.
//!   The test now sees the branch. Both predictions held.
//! - After the fix: main **20,000 cases, 129,837 operations, 0 mismatches**, max ULP 0, NaN-payload-only 0, 609
//!   turret queries undefined (as #175); attach **20,000 cases, 130,370 operations, 0 mismatches**, max ULP 0, 335
//!   undefined, 25,070 launch offsets found.
//! - Plants (mismatching cases of 20,000, attach test / main test): 9781 4,538 / 372; 9782 4,446 / 316; 9783 3,549 /
//!   271; 9784 4,956 / 401; 9785 2,757 / 196; 9786 2,284 / 195; 9787 3,438 / 270. Seven planted, seven caught (9781,
//!   9782, 9783, 9786 and 9787 also by the lib test `issue178_the_logic_s_attach_offset_is_filled_at_load`). 9730 and
//!   9746 (#175's, the undefined branch's tech offset) are retired with the code they planted in.

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;

use rts_lockstep_engine::client::w3d_model_draw::{
    scale_render_obj_hierarchy, AnimHandle, DrawEnv, DrawModuleExtra, ModelConditionInfo, Skeleton, SkeletonSubObject, W3DAnimationInfo, W3DModelDrawModuleData, W3dAssetSource,
};
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::model_state::ModelConditionFlags;
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::w3d::htree::{HTree, Pivot};
use rts_lockstep_engine::logic::weapon_host::WeaponHost;
use rts_lockstep_engine::logic::world::drawable_bones::{validate_template_art, ArtBones, ArtBonesReport, NoArt, TemplateArt};
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::object_weapons;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
use support::{Check, Gen};

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases() -> u64 {
    std::env::var("LOGIC_BONES_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000)
}

fn b(f: f32) -> i32 {
    f.to_bits() as i32
}

fn r(i: i32) -> f32 {
    f32::from_bits(i as u32)
}

/// The wrapper's pools (`BONE_NAMES`: the models' pivots; `QUERY_NAMES`: the names the module data and the queries use;
/// `SUB_NAMES`: the sub-objects', `M.` the model's name).
const BONE_NAMES: [&str; 14] =
    ["ROOTTRANSFORM", "FIREPOINT01", "FIREPOINT02", "FirePoint03", "MUZZLEFX01", "MuzzleFX02", "RECOIL01", "TURRET", "BARREL", "LAUNCH", "LAUNCH01", "ATTACH", "DOCKWAITING07", "Muzzle"];
const QUERY_NAMES: [&str; 14] =
    ["", "FirePoint", "FIREPOINT01", "MuzzleFX", "Recoil", "Turret", "Barrel", "Launch", "Attach", "DockWaiting", "nosuch", "None", "muzzlefx01", "MUZZLE"];
const SUB_NAMES: [&str; 6] = ["M.CHASSIS", "M.FIREPOINT02", "M.MUZZLE", "BARREL", "M.LAUNCH", "M.NOSUCH"];
const MODEL_NAMES: [&str; 3] = ["M0", "M1", "M2"];
const STATE_MODELS: [&str; 5] = ["M0", "M1", "M2", "", "MISSING"];
const STATE_ANIMS: [&str; 4] = ["", "A0", "A1", "AMISSING"];
/// `ACBits::PRISTINE_BONE_POS_IN_FINAL_FRAME` (W3DModelDraw.cpp:226).
const PRISTINE_BONE_POS_IN_FINAL_FRAME: i32 = 4;
const OP: usize = 8;
const SENT: i32 = 0x7fc0_dead;

const INI: &str = "Object BoneGun\n KindOf = VEHICLE\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n  Turret\n   TurretTurnRate = 60\n   ControlledWeaponSlots = PRIMARY\n   NaturalTurretAngle = 30\n  End\n End\nEnd\n\
Object BonePlain\n KindOf = VEHICLE\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n End\nEnd\n";

fn rigid(g: &mut Gen, t: f32) -> [i32; 12] {
    let mut m = Matrix3D::IDENTITY;
    m.rotate_z(g.uniform(-3.2, 3.2));
    m.set_translation(&Vector3::new(g.uniform(-t, t), g.uniform(-t, t), g.uniform(-t, t)));
    words(&m)
}

fn words(m: &Matrix3D) -> [i32; 12] {
    let mut w = [0; 12];
    for row in 0..3 {
        for c in 0..4 {
            w[row * 4 + c] = b(m.row[row][c]);
        }
    }
    w
}

fn matrix(w: &[i32]) -> Matrix3D {
    let mut m = Matrix3D::IDENTITY;
    for row in 0..3 {
        for c in 0..4 {
            m.row[row][c] = r(w[row * 4 + c]);
        }
    }
    m
}

fn mask(w: i32) -> ModelConditionFlags {
    let list: Vec<i32> = (0..32).filter(|&i| (w as u32 >> i) & 1 != 0).collect();
    ModelConditionFlags::from_indices(&list)
}

fn bits12(g: &mut Gen) -> i32 {
    let mut m = 0;
    for i in 0..12 {
        if g.below(4) == 0 {
            m |= 1 << i;
        }
    }
    m
}

/// One case's words (the wrapper's layout, read back by `ref_logic_bones_case`), but for the object's part, which the
/// World decides (see [`run`]).
///
/// (#178) `attach`: every module names an `AttachToBoneInAnotherModule` bone from the names the art may have
/// (`ATTACH_GOOD`), and 2 in 5 operations are launch offsets, so `getProjectileLaunchOffset`'s `CACHE_ATTACH_BONE`
/// branch (the cached offset added to the turret positions) is reached often. Off, the draws are #175's.
fn random_case(g: &mut Gen, attach: bool) -> (Vec<i32>, bool, [i32; 12], Vec<[i32; OP]>, i32) {
    let mut w = Vec::new();
    let scale = match g.below(10) {
        0..=3 => [0.25f32, 0.5, 1.0, 1.0, 1.0, 2.0, 4.0][g.below(7) as usize],
        4..=5 => [0.9899f32, 0.99, 0.995, 1.005, 1.0099, 1.01, 1.0101][g.below(7) as usize],
        _ => g.uniform(0.5, 2.0),
    };
    w.push(b(scale));
    w.push(if g.below(4) == 0 { g.below(4) as i32 } else { 3 });
    for _ in 0..3 {
        w.push(if g.below(4) != 0 { [1, 3, 9, 13, 4, 7][g.below(6) as usize] } else { g.below(QUERY_NAMES.len() as u32) as i32 });
    }
    for _ in 0..3 {
        let hlod = g.below(5) != 0;
        w.push(hlod as i32);
        let n = if hlod && g.below(10) != 0 { 4 + g.below(3) } else { g.below(7) };
        w.push(n as i32);
        for k in 0..6u32 {
            w.push(if k == 0 { 0 } else { 1 + g.below(BONE_NAMES.len() as u32 - 1) as i32 });
            w.push(if k == 0 { 0 } else { g.below(k) as i32 });
            if g.below(10) < 7 {
                w.extend(rigid(g, 20.0));
            } else {
                for i in 0..12 {
                    w.push(b(if i % 4 == 3 { g.uniform(-50.0, 50.0) } else { g.uniform(-2.0, 2.0) }));
                }
            }
        }
        w.push(g.below(4) as i32);
        for _ in 0..3 {
            w.push(g.below(SUB_NAMES.len() as u32) as i32);
            w.push(if n == 0 { 0 } else { g.below(n) as i32 });
        }
    }
    for _ in 0..2 {
        w.push(1 + g.below(4) as i32);
        for _ in 0..18 {
            w.push(b(g.uniform(-3.0, 3.0)));
        }
    }
    let nmod = 1 + g.below(3);
    w.push(nmod as i32);
    // a name: empty 1 in `empty_in`; else 7 in 10 one the art may have (`GOOD`), else any
    const GOOD: [i32; 11] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 12, 13];
    let q = |g: &mut Gen, empty_in: u32| {
        if g.below(empty_in) == 0 {
            0
        } else if g.below(10) < 7 {
            GOOD[g.below(GOOD.len() as u32) as usize]
        } else {
            1 + g.below(QUERY_NAMES.len() as u32 - 1) as i32
        }
    };
    for _ in 0..3 {
        w.push(if g.below(10) < 7 { 0 } else { bits12(g) });
        // (#178) the names a pivot of the art may have: FIREPOINT01, Turret, Barrel, Launch, Attach, muzzlefx01, MUZZLE
        const ATTACH_GOOD: [i32; 7] = [2, 5, 6, 7, 8, 12, 13];
        w.push(if attach {
            ATTACH_GOOD[g.below(ATTACH_GOOD.len() as u32) as usize]
        } else if g.below(5) == 0 {
            q(g, 100)
        } else {
            0
        });
        w.push(g.below(3) as i32);
        for _ in 0..2 {
            w.push(q(g, 100));
        }
        w.push(1 + g.below(4) as i32);
        for si in 0..4 {
            w.push(1 + g.below(2) as i32);
            for y in 0..2 {
                w.push(if si == 0 && y == 0 { 0 } else { bits12(g) });
            }
            w.push(match g.below(20) {
                0..=16 => g.below(3) as i32,
                17..=18 => 3,
                _ => 4,
            });
            w.push(if g.below(10) < 3 { 1 << PRISTINE_BONE_POS_IN_FINAL_FRAME } else { 0 } | g.below(2) as i32);
            w.push(g.below(4) as i32);
            w.push(if g.below(5) == 0 { g.below(2) as i32 } else { 2 });
            for _ in 0..2 {
                w.push(q(g, 100));
            }
            for _ in 0..12 {
                w.push(q(g, 2));
            }
            for _ in 0..2 {
                w.push(q(g, 2));
                w.push(q(g, 2));
                w.push(b(if g.below(2) == 0 { 0.0 } else { g.uniform(-3.2, 3.2) }));
                w.push(b(if g.below(2) == 0 { 0.0 } else { g.uniform(-1.0, 1.0) }));
            }
        }
    }
    // the object: which, and its transform
    let turret = g.below(4) != 0;
    let mut t = Matrix3D::IDENTITY;
    t.rotate_z(g.uniform(-3.2, 3.2));
    if g.below(5) == 0 {
        for row in 0..3 {
            for c in 0..3 {
                t.row[row][c] += g.uniform(-0.5, 0.5);
            }
        }
    }
    t.set_translation(&Vector3::new(g.uniform(50.0, 350.0), g.uniform(50.0, 350.0), g.uniform(0.0, 10.0)));
    let mut ops = Vec::new();
    for _ in 0..1 + g.below(12) {
        let mut a = [0i32; OP];
        a[0] = if attach && g.below(5) < 2 { 4 } else { match g.below(100) {
            0..=24 => 0,
            25..=39 => 1,
            40..=52 => 2,
            53..=65 => 3,
            66..=84 => 4,
            85..=91 => 5,
            _ => 6,
        } };
        match a[0] {
            0 => {
                a[1] = q(g, 10);
                a[2] = [0, 1, 1, 2, 98][g.below(5) as usize];
                a[3] = -1 + g.below(10) as i32;
                a[4] = (g.below(4) != 0) as i32;
                a[5] = (g.below(4) != 0) as i32;
            }
            1 | 3 => a[1] = q(g, 10),
            2 => {
                a[1] = q(g, 10);
                a[2] = 1 + g.below(8) as i32;
                a[3] = (g.below(2) == 0) as i32;
                a[4] = (g.below(4) != 0) as i32;
                a[5] = (g.below(3) != 0) as i32;
            }
            4 => {
                a[1] = g.below(3) as i32;
                a[2] = -1 + g.below(7) as i32;
                a[3] = (g.below(5) != 0) as i32;
                a[4] = -1 + g.below(3) as i32;
                a[5] = (g.below(5) != 0) as i32;
                a[6] = (g.below(5) != 0) as i32;
            }
            5 => a[1] = g.below(3) as i32,
            _ => a[1] = bits12(g),
        }
        ops.push(a);
    }
    (w, turret, words(&t), ops, bits12(g))
}

/// The test's asset manager: the case's models and animations (see the wrapper's header for the animated pose).
struct Models {
    /// (hlod, pivots (name, parent, pose), sub-objects (name, bone)).
    models: Vec<(bool, Vec<(&'static str, u32, Matrix3D)>, Vec<(Vec<u8>, i32)>)>,
    /// (frames, each pivot's step).
    anims: Vec<(i32, [[f32; 3]; 6])>,
}

impl W3dAssetSource for Models {
    fn create_render_obj(&mut self, name: &[u8]) -> Option<bool> {
        MODEL_NAMES.iter().position(|n| n.as_bytes().eq_ignore_ascii_case(name)).map(|i| self.models[i].0)
    }
    fn get_hanim(&mut self, name: &[u8]) -> Option<AnimHandle> {
        let i = ["A0", "A1"].iter().position(|n| n.as_bytes().eq_ignore_ascii_case(name))?;
        Some(AnimHandle { name: name.to_vec(), num_frames: self.anims[i].0, frame_rate: 30.0, data: None })
    }
    fn skeleton(&mut self, name: &[u8], pose: Option<(&[u8], f32)>, scale: f32) -> Option<Skeleton> {
        let i = MODEL_NAMES.iter().position(|n| n.as_bytes().eq_ignore_ascii_case(name))?;
        let (hlod, bones, subs) = &self.models[i];
        if !hlod || bones.is_empty() {
            return Some(Skeleton::default());
        }
        let pivots = bones
            .iter()
            .enumerate()
            .map(|(k, (n, parent, p))| {
                let mut nm = [0u8; 16];
                nm[..n.len()].copy_from_slice(n.as_bytes());
                Pivot::new(nm, if k == 0 { None } else { Some(*parent) }, *p, k as i32)
            })
            .collect();
        let mut tree = HTree { name: [0; 16], num_pivots: bones.len() as i32, pivots, scale_factor: 1.0 };
        scale_render_obj_hierarchy(&mut tree, scale);
        // the animated pose (the wrapper's): each pivot but the root moved by (frame + 1) times the animation's step
        if let Some((anim, frame)) = pose {
            if let Some(a) = ["A0", "A1"].iter().position(|n| n.as_bytes().eq_ignore_ascii_case(anim)) {
                let steps = frame + 1.0;
                for (k, p) in tree.pivots.iter_mut().enumerate().skip(1) {
                    let d = self.anims[a].1[k];
                    p.base_transform.row[0].w += d[0] * steps;
                    p.base_transform.row[1].w += d[1] * steps;
                    p.base_transform.row[2].w += d[2] * steps;
                }
            }
        }
        let mut root = Matrix3D::IDENTITY;
        root.scale(scale);
        tree.base_update(&root).expect("parents come first");
        let sub_objects = subs
            .iter()
            .map(|(n, bone)| SkeletonSubObject { name: n.clone(), bone_index: *bone, hidden: false, transform: tree.get_transform(*bone).unwrap(), linear_offset_materials: 0 })
            .collect();
        Some(Skeleton { bones: Skeleton::bones_of(&tree), sub_objects, scale_factor: tree.scale_factor })
    }
}

/// The parse's name: lower-cased, `None` cleared.
fn lowered(code: i32) -> Vec<u8> {
    let s = QUERY_NAMES[code as usize].to_ascii_lowercase().into_bytes();
    if s == b"none" { Vec::new() } else { s }
}

/// The case's art and module data (the World's part), from its words.
fn build(w: &[i32]) -> (f32, Vec<Vec<u8>>, Models, Vec<Arc<W3DModelDrawModuleData>>, usize) {
    let mut k = 0;
    let mut next = || {
        k += 1;
        w[k - 1]
    };
    let scale = r(next());
    let nstd = next();
    let mut std = Vec::new();
    for s in 0..3 {
        let c = next();
        if s < nstd {
            std.push(QUERY_NAMES[c as usize].as_bytes().to_vec());
        }
    }
    let mut models = Models { models: Vec::new(), anims: Vec::new() };
    for mi in 0..3 {
        let hlod = next() != 0;
        let n = next() as usize;
        let mut bones = Vec::new();
        for bi in 0..6 {
            let name = BONE_NAMES[next() as usize];
            let parent = next() as u32;
            let mut p = [0; 12];
            for v in p.iter_mut() {
                *v = next();
            }
            if bi < n {
                bones.push((name, parent, matrix(&p)));
            }
        }
        let nsub = next() as usize;
        let mut subs = Vec::new();
        for s in 0..3 {
            let sn = SUB_NAMES[next() as usize];
            let bone = next();
            if s < nsub && hlod && n > 0 {
                let full = match sn.strip_prefix("M.") {
                    Some(rest) => format!("{}.{rest}", MODEL_NAMES[mi]),
                    None => sn.to_string(),
                };
                subs.push((full.into_bytes(), bone));
            }
        }
        models.models.push((hlod, bones, subs));
    }
    for _ in 0..2 {
        let frames = next();
        let mut d = [[0f32; 3]; 6];
        for p in d.iter_mut() {
            for c in p.iter_mut() {
                *c = r(next());
            }
        }
        models.anims.push((frames, d));
    }
    let nmod = next() as usize;
    let mut modules = Vec::new();
    for _ in 0..3 {
        let mut d = W3DModelDrawModuleData::new(b"W3DModelDraw", DrawModuleExtra::None);
        d.ignore_condition_states = mask(next());
        // `parseAsciiStringLC`: lower-cased (a `None` is kept)
        d.attach_to_drawable_bone = QUERY_NAMES[next() as usize].to_ascii_lowercase().into_bytes();
        let nextra = next();
        for e in 0..2 {
            let c = next();
            if e < nextra {
                d.extra_public_bones.push(QUERY_NAMES[c as usize].as_bytes().to_vec());
            }
        }
        let nstates = next();
        for si in 0..4 {
            let mut s = ModelConditionInfo::default();
            let nyes = next();
            for y in 0..2 {
                let m = next();
                if y < nyes {
                    s.conditions_yes_vec.push(mask(m));
                }
            }
            s.model_name = STATE_MODELS[next() as usize].to_ascii_lowercase().into_bytes();
            s.flags = next() as u32;
            let anim = next();
            if anim != 0 {
                s.animations.push(W3DAnimationInfo::new(STATE_ANIMS[anim as usize].as_bytes().to_vec(), false, 0.0));
            }
            let npub = next();
            for p in 0..2 {
                let c = next();
                if p < npub {
                    s.add_public_bone(QUERY_NAMES[c as usize].as_bytes());
                }
            }
            for slot in 0..3 {
                for kind in 0..4 {
                    let n = lowered(next());
                    match kind {
                        0 => s.weapon_fire_fx_bone_name[slot] = n.clone(),
                        1 => s.weapon_recoil_bone_name[slot] = n.clone(),
                        2 => s.weapon_muzzle_flash_name[slot] = n.clone(),
                        _ => s.weapon_projectile_launch_bone_name[slot] = n.clone(),
                    }
                    s.add_public_bone(&n);
                }
            }
            for t in 0..2 {
                for which in 0..2 {
                    let n = QUERY_NAMES[next() as usize].to_ascii_lowercase().into_bytes();
                    s.add_public_bone(&n);
                    let key = if n.is_empty() || n == b"none" { Vec::new() } else { n };
                    if which == 0 {
                        s.turrets[t].turret_angle_name_key = key;
                    } else {
                        s.turrets[t].turret_pitch_name_key = key;
                    }
                }
                s.turrets[t].turret_art_angle = r(next());
                s.turrets[t].turret_art_pitch = r(next());
            }
            if si < nstates {
                d.condition_states.push(s);
            }
        }
        modules.push(Arc::new(d));
    }
    modules.truncate(nmod);
    (scale, std, models, modules, k)
}

fn push_m(out: &mut Vec<i32>, m: &Matrix3D) {
    out.extend(words(m));
}

fn push_c(out: &mut Vec<i32>, c: &Coord3D) {
    out.extend([b(c.x), b(c.y), b(c.z)]);
}

/// The Rust's records of a case (the wrapper's), on the World.
fn rust_case(w: &mut World, id: ObjectId, ops: &[[i32; OP]]) -> Vec<i32> {
    let mut out = Vec::new();
    let sent_m = matrix(&[SENT; 12]);
    let sent_c = Coord3D { x: r(SENT), y: r(SENT), z: r(SENT) };
    for a in ops {
        let name = QUERY_NAMES.get(a[1] as usize).map_or(&b""[..], |n| n.as_bytes());
        match a[0] {
            0 => {
                let mut pos = [sent_c; 8];
                let mut tr = [sent_m; 8];
                let art = w.drawable_art(id).expect("a drawable");
                let mut na = NoArt;
                let mut random = RandomValues::default();
                let mut env = DrawEnv { assets: &mut na, random: &mut random, sync_time: 0, logic_time: true, particles: None };
                let n = art.get_pristine_bone_positions(name, a[2], (a[4] != 0).then_some(&mut pos[..]), (a[5] != 0).then_some(&mut tr[..]), a[3], &mut env);
                out.extend([10, n]);
                for p in &pos {
                    push_c(&mut out, p);
                }
                for m in &tr {
                    push_m(&mut out, m);
                }
            }
            1 => {
                let (found, p, m) = w.object_get_single_logical_bone_transform(id, name);
                out.extend([11, found as i32]);
                push_c(&mut out, &p);
                push_m(&mut out, &m);
            }
            2 => {
                let found = object_weapons::get_multi_logical_bone_position(&*w, id, name, a[2], a[3] != 0, a[4] != 0, a[5] != 0);
                out.extend([12, found.len() as i32]);
                for (p, m) in &found {
                    if let Some(p) = p {
                        push_c(&mut out, p);
                    }
                    if let Some(m) = m {
                        push_m(&mut out, m);
                    }
                }
            }
            3 => match object_weapons::get_single_logical_bone_position_on_turret(w, id, 0, name, true, true) {
                Ok(q) => {
                    out.extend([13, q.found as i32]);
                    if q.found {
                        push_c(&mut out, &q.position.unwrap());
                        push_m(&mut out, &q.transform.unwrap());
                    }
                }
                Err(_) => out.extend([13, -1]),
            },
            4 => {
                let (mut l, mut rp, mut pp) = (sent_m, sent_c, sent_c);
                let art = w.drawable_art(id).expect("a drawable");
                let mut na = NoArt;
                let mut random = RandomValues::default();
                let mut env = DrawEnv { assets: &mut na, random: &mut random, sync_time: 0, logic_time: true, particles: None };
                let found = art.get_projectile_launch_offset(
                    a[1] as usize,
                    a[2],
                    (a[3] != 0).then_some(&mut l),
                    a[4],
                    (a[5] != 0).then_some(&mut rp),
                    (a[6] != 0).then_some(&mut pp),
                    &mut env,
                );
                out.extend([14, found as i32]);
                push_m(&mut out, &l);
                push_c(&mut out, &rp);
                push_c(&mut out, &pp);
            }
            5 => {
                let n = WeaponHost::barrel_count(w, id, a[1]).expect("a drawable");
                out.extend([15, n]);
            }
            _ => {
                w.drawable_condition.insert(id, mask(a[1]));
                out.push(16);
            }
        }
    }
    out
}

fn run_cpp(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 1 << 18];
    let n = unsafe { cpp::ref_logic_bones_case(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n as usize <= out.len());
    out.truncate(n as usize);
    out
}

/// The World of the cases and its two objects (`BoneGun`, `BonePlain`).
fn world() -> (World, ObjectId, ObjectId) {
    use rts_lockstep_engine::logic::world::gate_scenario::{gate_world_with, make};
    use rts_lockstep_engine::logic::world::team_ops::factory_find_team;
    let mut w = gate_world_with(INI);
    assert!(w.cuts.live_bones);
    let team = factory_find_team(&mut w, b"team1").unwrap().unwrap();
    let gun = make(&mut w, b"BoneGun", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
    let plain = make(&mut w, b"BonePlain", team, Coord3D { x: 120.0, y: 100.0, z: 0.0 });
    (w, gun, plain)
}

/// (cases, operations, undefined turret queries, bones found by the pristine queries)
fn compare(c: &mut Check, seed: u64, n: u64, attach: bool) -> [u64; 4] {
    let (mut w, gun, plain) = world();
    let mut g = Gen::new(seed);
    let (mut ops_run, mut undefined, mut found) = (0u64, 0u64, 0u64);
    // (single bones found, multi bones found, turret bones found, launch offsets found, barrel counts > 0)
    let mut hits = [0u64; 5];
    for k in 0..n {
        let (mut words_in, turret, transform, ops, cond) = random_case(&mut g, attach);
        let (scale, std, mut models, modules, _) = build(&words_in);
        let mut report = ArtBonesReport::default();
        validate_template_art(&modules, scale, &std, &mut models, &mut report);
        let art = TemplateArt { modules: Arc::new(modules), scale };
        let mut templates = BTreeMap::new();
        templates.insert(b"BoneGun".to_vec(), art.clone());
        templates.insert(b"BonePlain".to_vec(), art);
        w.art_bones = Arc::new(ArtBones { templates, standard_public_bones: Arc::new(std), report });
        let id = if turret { gun } else { plain };
        w.set_transform_matrix(id, &matrix(&transform));
        w.drawable_condition.insert(id, mask(cond));
        let held = *w.find_object(id).unwrap().thing.transform_matrix();
        let rot = w.ai_turret_rot_and_pitch(id, 0);
        words_in.extend(words(&held));
        words_in.push(rot.is_some() as i32);
        words_in.push(b(rot.map_or(0.0, |x| x.0)));
        words_in.push(cond);
        for a in &ops {
            words_in.extend(a);
        }
        let rust = rust_case(&mut w, id, &ops);
        // (the events the transform change sent: the client's, dropped)
        let _ = w.take_client_events();
        let cc = run_cpp(&words_in);
        ops_run += ops.len() as u64;
        // (each operation's record, in order: count the undefined turret queries and the bones found)
        let mut i = 0;
        for a in &ops {
            if i + 1 >= cc.len() {
                break;
            }
            i += match a[0] {
                0 => {
                    found += cc[i + 1].max(0) as u64;
                    2 + 8 * 15
                }
                1 => {
                    hits[0] += u64::from(cc[i + 1] == 1);
                    2 + 15
                }
                2 => {
                    hits[1] += cc[i + 1].max(0) as u64;
                    2 + cc[i + 1].max(0) as usize * (3 * a[3] as usize + 12 * a[4] as usize)
                }
                3 => {
                    undefined += u64::from(cc[i + 1] == -1);
                    hits[2] += u64::from(cc[i + 1] == 1);
                    2 + if cc[i + 1] == 1 { 15 } else { 0 }
                }
                4 => {
                    hits[3] += u64::from(cc[i + 1] == 1);
                    2 + 18
                }
                5 => {
                    hits[4] += u64::from(cc[i + 1] > 0);
                    2
                }
                _ => 1,
            };
        }
        if rust != cc && !planted() && c.mismatches < 4 {
            let at = rust.iter().zip(&cc).position(|(a, bb)| a != bb).unwrap_or(rust.len().min(cc.len()));
            let lo = at.saturating_sub(4);
            println!("case {k}: at word {at}: rust {:?} c++ {:?}", &rust[lo.min(rust.len())..(at + 12).min(rust.len())], &cc[lo.min(cc.len())..(at + 12).min(cc.len())]);
            println!("    ops {ops:?}");
        }
        c.eq(rust, cc, || format!("case {k}"));
    }
    println!("hits: single bones found {}, multi bones found {}, turret bones found {}, launch offsets found {}, barrel counts above 0 {}", hits[0], hits[1], hits[2], hits[3], hits[4]);
    [n, ops_run, undefined, found]
}

#[test]
fn the_logic_s_bones_match_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut c = Check::new("logic bones and launch offsets (#175)");
    let [n, ops, undefined, found] = compare(&mut c, 0x175_0001, cases(), false);
    println!("logic bones: {n} cases, {ops} operations, {undefined} turret queries undefined upstream (excluded: both sides record -1), {found} bones found by the pristine queries");
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    }
    c.finish();
}

/// (#178) The same comparison with every module naming an `AttachToBoneInAnotherModule` bone the art may have and more
/// launch-offset queries (`random_case`'s `attach`), so the `CACHE_ATTACH_BONE` branch of `getProjectileLaunchOffset`
/// (W3DModelDraw.cpp:3362-3395) is reached in most cases.
#[test]
fn the_attach_bone_offset_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut c = Check::new("logic launch offsets with an attach bone (#178)");
    let [n, ops, undefined, found] = compare(&mut c, 0x178_0001, cases(), true);
    println!("attach bone: {n} cases, {ops} operations, {undefined} turret queries undefined upstream (excluded: both sides record -1), {found} bones found by the pristine queries");
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    }
    c.finish();
}
