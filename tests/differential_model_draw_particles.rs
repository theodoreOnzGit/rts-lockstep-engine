#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp (the bone particle
//   systems: recalcBonesForClientParticleSystems, updateBonesForClientParticleSystems, stopClientParticleSystems,
//   doStartOrStopParticleSys, setHidden, setFullyObscuredByShroud, and the parts of the constructor, setModelState and
//   doDrawModule that reach them) and its header (ParticleSysTrackerType)
//   GeneralsMD/Code/GameEngine/Source/GameClient/System/ParticleSys.cpp (ParticleSystem::setSaveable, start, stop,
//   destroy, setPosition, setLocalTransform, rotateLocalTransformZ, attachToDrawable)
//   (GitHub #172) W3DModelDraw.cpp (getCurrentWorldspaceClientBonePositions); W3DAssetManager.cpp (Create_Render_Obj's
//   scaling); GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/htree.cpp (HTreeClass::Scale, Base_Update), htree.h
//   (Get_Transform), hlod.cpp (HLodClass::Scale), animobj.cpp (Get_Bone_Transform)
//   (GitHub #174) W3DModelDraw.cpp (getAttachToDrawableBoneOffset, adjustTransformMtx, doDrawModule's instance scale and
//   Set_Transform, getCurrentBonePositions, reactToTransformChange, setModelState's transform hand-over), W3DModelDraw.h
//   (CACHE_ATTACH_BONE), pivot.cpp (PivotClass::Capture_Update), htree.cpp (Capture_Bone, Release_Bone,
//   Is_Bone_Captured, Control_Bone), animobj.cpp (Capture_Bone, Control_Bone)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! GitHub #171: `W3DModelDraw`'s bone particle systems against the upstream text compiled verbatim
//! (`differential/wrap_model_draw_particles.cpp`, which lists the cuts, the stand-ins and the words).
//!
//! # Methodology
//!
//! Each case makes a module (the port's whole [`W3DModelDraw`], parsed from a `Draw` block) of a drawable of a random id
//! and scale, with `ParticlesAttachedToAnimatedBones` on or off; four particle system templates `PS0`..`PS3`, each with a
//! later one as its slave or none; three models (HLOD or mesh, 0 to 6 bones named from a pool with case and duplicate
//! variations, each with a random pose: rotation-and-scale entries in -2..2, translation in -50..50, and (#172) a random
//! earlier pivot as its parent, so chains up to 5 deep) and the names `None`
//! (no model) and `Missing` (no such render object); one to four states, each with a model and up to three
//! `ParticleSysBone`s (a bone name from a pool that includes one no model has, and a template, `None` or an unknown
//! name). The asset source gives the port's skeleton as `W3dAssetSource::skeleton` promises it: ~~each bone under
//! `Matrix3D(true).Scale(scale)`, by WWMath's `Multiply`~~ **Corrected 2026-11-12 (#172):** the hierarchy scaled as
//! `Create_Render_Obj(name, scale, color)` scales it (the port's `scale_render_obj_hierarchy` and `HTree::scale`), posed
//! by the port's `Base_Update` under `Matrix3D(true).Scale(scale)`; the C++ render object is made the same way by the
//! cut upstream (`HTreeClass::Scale`, `Base_Update`, `Get_Transform`, the asset manager's `reallyscale`). Then 1 to 40
//! steps: `setModelState` 15 %, `doDrawModule` ~~35 %~~ 30 % (with a random world transform: ~~the C++ render object gets
//! `world * Scale(scale)`~~ (#172) the C++ render object gets `world`, as upstream's does (no template scale in its
//! transform), the port [`DrawFacts::transform`] = `world`), `updateBonesForClientParticleSystems` 15 % (the same), the
//! drawable hidden or shown 10 %, the shroud 10 %, a system dropped by the manager ~~8 %~~ 7 % (as its update deletes
//! one), the drawable's `NO_STATE_PARTICLES` status ~~7 %~~ 6 %, and (#172) `getCurrentWorldspaceClientBonePositions`
//! of a name from the bone pool under a random world 7 % (recorded: found, and the 12 words). ~~1 in 20 world
//! transforms of a case at scale 1 has entries from `Gen::finite` (huge, tiny and subnormal values, never NaN or
//! infinite: see the results).~~ (#172) The main test: 1 in 20 world transforms has entries from `Gen::finite` (huge,
//! tiny and subnormal) at any scale; the second test: 1 in 10 from `Gen::mixed` (infinities and NaN too). Compared, every
//! word as bits (NaN on both sides as equal), after the constructor and after each step: the module's
//! `m_needRecalcBoneParticleSystems`, `m_fullyObscuredByShroud`, its trackers (id, bone), and every system in the manager
//! in id order (attached drawable, saveable, stopped, destroyed, skip-parent, local identity, slave, master, local
//! transform).
//!
//! - ~~**The main test** draws the scale from the powers of two 0.25 .. 4, where the port's bone convention (the skeleton
//!   carries the scale: `world * (Scale * pose)`) and upstream's (`(world * Scale) * pose`) round the same.~~
//!   **Corrected 2026-11-12 (#172):** neither was upstream's. Upstream scales the render object's *hierarchy* (the
//!   pivots' translations, `HTreeClass::Scale`, when `|scale - 1| > 0.01`) and chains each bone from the render object's
//!   transform; the port now does the same (module docs of `client::w3d_model_draw`). **The main test** (gated, 20,000)
//!   draws the scale from the powers of two 0.25 .. 4 (40 %), the edge of `scale_epsilon` (0.9899 .. 1.0101, 20 %) and
//!   any in 0.5 .. 2 (40 %).
//! - ~~**The second test** (2,000 cases) draws any scale in 0.5 .. 2 and reports, not gates, how far the two conventions
//!   round apart.~~ **Corrected 2026-11-12 (#172): the second test** (gated, 20,000 cases) draws any scale in 0.25 .. 4,
//!   or from `Gen::mixed` (0, NaN, infinities) 1 in 10, with infinite and NaN world entries.
//!
//! # Predictions (written before the first run, `issue171_predictions.md`)
//!
//! Main: 0 mismatches, max ULP 0, no case undefined upstream. Second: mismatching words only in the local transforms
//! `updateBonesForClientParticleSystems` writes, max ULP <= 4. All 30 plants caught.
//!
//! # Results (2026-11-12)
//!
//! - First build: the wrapper declared its own `DrawableID` beside GameType.h's enum (removed; the wrapper's error).
//! - First run: 4,970 of 20,000 sequences mismatched: the wrapper's states did not start as a copy of the default state's
//!   `ParticleSysBone` list, as `parseConditionState` makes them (the port's parse does). The wrapper's error, fixed.
//! - Second run: 2 of 20,000 mismatched, both a world transform with an infinite entry (from `Gen::mixed`): upstream's
//!   `world * Scale(1)` makes `inf * 0 = NaN` in the row, the port's `world * (Scale * pose)` keeps the infinity. ~~Not a
//!   translation defect but the bone convention (module docs of `w3d_model_draw`), which no drawable's transform reaches;
//!   the mixed words are now finite (`Gen::finite`).~~ **Corrected 2026-11-12 (#172):** a translation defect (#172): the
//!   port's bone order was not upstream's, and the stand-in's `world * Scale(1)` was not upstream's either; with both
//!   fixed, infinite entries agree (second test). **A prediction miss** (I predicted 0 at scale 1).
//! - **Main, third run:** 20,000 sequences, 409,427 steps, 52,828 particle systems made; 0 NaN words; 0 undefined
//!   upstream (none generated: no template cycles, every bone index in range); **0 mismatches, max ULP 0**.
//! - ~~**Any scale (not gated):** 230 of 2,000 sequences differ, max ULP 32,768 (on words near zero after a
//!   cancellation), largest difference 3.8e-6 of max(|value|, 1): rounding only, every id, bone and flag word equal.
//!   **A prediction miss** on the size (I predicted max ULP <= 4; ULP is the wrong measure for words near zero).~~
//!   **Corrected 2026-11-12 (#172):** that measured the port against a stand-in that shared its mistaken picture of the
//!   scale; superseded below.
//!
//! # #172 (2026-11-12): predictions and results
//!
//! Predicted (`issue172_predictions.md`, before any change or run): both tests 0 mismatches over 20,000 cases; NaN on
//! both sides with differing payload bits 0; plants 9691-9698 caught (9697 by the lib test
//! `a_scaled_gun_s_bones_are_upstream_s`, which this test cannot reach); the 30 plants of #171 still caught.
//!
//! - **First run** (the port and the wrapper as described above): **main 0 mismatches** over 20,000 sequences (407,610
//!   steps, 50,486 systems made, 147 NaN words, max ULP 0); **any scale 0 mismatches** over 20,000 (409,774 steps,
//!   50,422 systems, 1,125 NaN words, max ULP 0). 204 words are NaN on both sides with different bits (counted and
//!   normalised as before): **a prediction miss** (I predicted 0; the NaN arise from NaN or infinite inputs on both
//!   sides, and their payload/sign bits are not specified by the arithmetic either side runs).
//! - Plants (main test, mismatching of 20,000): 9691 1,973; 9692 3,140; 9693 632; 9694 1,406; 9695 2,963; 9696 2,307;
//!   9698 1,449; 9697 0 here (not reachable), caught by the lib test. **8 of 8 caught.** The #171 plants re-run on
//!   the changed test: 30 of 30 caught (9661 18,834 ... 9690 1,381).
//!
//! Planted bugs, mismatching sequences of 20,000: 9661 19,070; 9662 1,708; 9663 2,894; 9664 1,358; 9665 1,466; 9666
//! 13,237; 9667 13,237; 9668 5,045; 9669 2,379; 9670 2,401; 9671 2,894; 9672 13,237; 9673 6,509; 9674 2,295; 9675 2,295;
//! 9676 2,295; 9677 1,534; 9678 3,957; 9679 7,426; 9680 6,397; 9681 3,751; 9682 7,426; 9683 4,065; 9684 2,588; 9685
//! 11,728; 9686 1,466; 9687 1,428; 9688 9,633; 9689 3,501; 9690 1,466. **30 of 30 caught.** (9665, 9686 and 9690 match
//! the same 1,466 sequences: a recalc's position is seen only where no update follows, i.e. without
//! `ParticlesAttachedToAnimatedBones`.)
//!
//! # #174 (2026-11-13): the live root, the captured bones, `getCurrentBonePositions`
//!
//! Added to each case: the module's `AttachToBoneInAnotherModule` (half the cases) with the drawable's pristine answer
//! for it (found 3 in 4, a random pose), every state's `ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT` (half), an object (4 in 5)
//! with a construction percent (-5 .. 110, 100 and -1 one in 8 each) and a height (0 .. 60); two more pivot names
//! (`Bone02`, `BONE03`) and `BONE`. The drawable's transform is now the case's: `doDrawModule(&world)` makes the render
//! object's (the instance scale, `Set_ObjectScale`, `adjustTransformMtx`), and the ops that read bones outside a draw (2,
//! 7) first call `reactToTransformChange` with the drawable at `world`. New ops: `Capture_Bone` + `Control_Bone` of a
//! random bone (10 %: a turn about Z, a pitch about Y, a recoil shift along -X, or any matrix), `setInstanceScale` (3 %:
//! 1, 0.5, 2, 0.25, 1.005 or any in 0.2 .. 2), `getCurrentBonePositions` (8 %: a prefix from `PREFIXES`, `startIndex`
//! 0..3, `maxBones` 1, 2, 3, 64, 70, 0 or -1, `positions` and `transforms` each `NULL` 1 in 4; recorded: the count, each
//! position and transform). Every state record also has the render object's transform and object scale. The port's
//! module data has its attach offset filled at the start from the same answer (as the client's drawable fills it when it
//! is made, `Drawable::validate_attach_to_drawable_bone_offsets`).
//!
//! Predicted (`issue174_predictions.md`, before any run): 0 mismatches in both tests; plants 9751-9766 (those defined)
//! caught, 9760 only by the lib test (`Drawable::set_instance_scale` is the drawable's).
//!
//! - **First build:** the cut `getCurrentBonePositions` reuses its first loop's `i` after the loop (MSVC 6's for-scope);
//!   g++ refuses it even with `-fpermissive`. Without editing the text, the wrapper's module class has a `mutable Int i`
//!   member the second loop finds (set to 0 before it is read, as MSVC's variable would be).
//! - **First run (2026-11-13):** main 0 mismatches over 20,000 sequences (407,965 steps, 45,344 systems, 35 NaN words);
//!   any scale 0 over 20,000 (407,905 steps, 44,672 systems, 2,624 NaN words, 315 NaN on both sides with different
//!   bits, normalised). Plants: 9763 (`startIndex` 0 goes on) was not caught (0): no model had a bone named by a prefix
//!   *and* `prefix01`. **A prediction miss**; the pivot name `BONE` added.
//! - **Second run:** main 0 mismatches over 20,000 (407,965 steps, 45,344 systems, 37 NaN words, max ULP 0); any scale 0
//!   over 20,000 (407,905 steps, 44,672 systems, 2,291 NaN words, 165 NaN-payload-only). Plants, mismatching of 20,000:
//!   9751 1,091; 9753 5,264; 9754 7,415; 9755 11,122; 9756 5,266; 9757 3,988; 9758 3,986; 9759 177; 9762 1,113; 9763 57;
//!   9764 1,257; 9765 1,369; 9766 1,152; 9760 0 here, caught by the lib test
//!   `the_instance_scale_and_the_attach_offset_reach_the_modules` (9751 and 9755 are also caught by the lib tests).
//!   **14 of 14 caught.** The #171/#172 plants re-run: 9661-9696 and 9698 all caught (9697 0 here, caught by its lib
//!   test as before).
//!
//! # #179 (2026-11-13): the render object's live hierarchy
//!
//! The port's bones now come from the render object's own hierarchy, evaluated when it is not valid and cached while it
//! is (`RenderObj::get_bone_transform`); this wrapper's stand-in still runs `Base_Update` on every query. Its models have
//! no animation, and every op that changes the base pose also invalidates the port's hierarchy, so predicted
//! (`issue179_predictions.md`): no change. Measured: main 0 mismatches over 20,000 (407,965 steps, 45,344 systems, 37 NaN
//! words); any scale 0 over 20,000 (407,905 steps, 44,672 systems, 2,291 NaN words, 165 NaN-payload-only): the #174
//! numbers. The animated pose itself is checked in `tests/differential_w3d.rs` (`animatable_bones_match_upstream`).
//!
//! # #179 item 3 (2026-11-13): `Drawable::draw`'s root
//!
//! Upstream hands every `doDrawModule` the drawable's transform post-multiplied by its instance matrix (unless flagged
//! the identity), with `applyPhysicsXform`'s `Translate`, `Rotate_Y`, `Rotate_X`, `Rotate_Z` on it (Drawable.cpp:2662-2673,
//! cut verbatim with `applyPhysicsXform`, `setInstanceMatrix` and Drawable.h's `PhysicsXformInfo`). New ops: 11 (12 %)
//! `Drawable::draw` with the drawable's transform the 12 words (drawn as op 1's): the root is recorded
//! ([400, 12 words]) and handed to `doDrawModule`; the port's side is `Drawable::draw_transform_mtx` with the drawable's
//! physics totals (none without an object, as `physics_xform_facts`' `apply`) and [`DrawFacts::drawable_transform`] the
//! words. 12 (4 %) `setInstanceMatrix`: `NULL` 1 in 4, else a sway's tilt (about X and Y), a float's lift and turn, the
//! identity (flagged not), or any matrix (`Gen::mixed` 1 in 10 in the second test); the port's
//! `Drawable::set_instance_matrix`. 13 (4 %) `calcPhysicsXform`'s answer (false 1 in 5) and totals (pitch and roll in
//! -0.6..0.6, yaw in -0.3..0.3, z in -3..3; all 0 one in 8; `Gen::mixed` 1 in 10 each in the second test). The other ops'
//! shares shrink (setModelState 11 %, doDrawModule 18 %, ...); the random stream is new, so the step and system counts
//! are new numbers. The locomotor models themselves (`calcPhysicsXform*`) are not compared here (no differential covers
//! them yet: #179 item 3's gaps).
//!
//! Predicted (`issue179i3_predictions.md`, before any run): 0 mismatches in both tests after the fix; the pre-fix root
//! (plant 9870) 9,000 - 13,000 of 20,000; plants 9846-9857 caught (9854 not in the main test, 9856 and 9857 only by the
//! lib test).
//!
//! - **First run (2026-11-13):** main **0 mismatches** over 20,000 (406,777 steps, 47,070 systems, 122 NaN words, max
//!   ULP 0); any scale **0** over 20,000 (409,698 steps, 47,435 systems, 3,034 NaN words, 134 NaN-payload-only,
//!   normalised; max ULP 0).
//! - **The pre-fix root** (9870: the drawable's transform alone, as `ClientDrawables::draw` gave it before):
//!   **7,363** of 20,000 mismatch. **A prediction miss** on the size (I predicted 9,000 - 13,000).
//! - Plants, mismatching of 20,000: 9846 4,491; 9847 4,490; 9848 1,387; 9849 4,373; 9850 4,374; 9851 4,373; 9852 4,373;
//!   9853 417; 9854 8 (**a prediction miss**: I predicted 0 here; the identity's post-multiply turns a `-0.0` entry
//!   into `+0.0`, `-0 * 1 + x * 0 + y * 0`); 9855 4,491; 9856 and 9857 0 here (the renderer's matrix and
//!   `ClientDrawables::draw` are not reached), caught by the lib test
//!   `the_draw_root_carries_the_instance_matrix_and_the_physics`. **12 of 12 caught.**
//!
//! # #182 item 1 (2026-11-13): the instance matrices the logic sets
//!
//! The logic's `DrawableSetInstanceTranslation` (StructureCollapseUpdate) and `DrawableFloatTilt` (FloatUpdate) reach the
//! client's `Drawable::set_instance_translation` and `Drawable::float_update_tilt`. Upstream has no
//! `setInstanceTranslation`: the lines the events stand for are cut verbatim from StructureCollapseUpdate.cpp (the
//! waiting shudder, the collapsing shudder and height, the final zero: `getInstanceMatrix`, `Set_Translation`,
//! `setInstanceMatrix`) and FloatUpdate.cpp (the bob: `Get_Z_Rotation`, `Make_Identity`, `Rotate_Z`, `Rotate_Y`,
//! `Rotate_X`). New ops: 14 (3 %) the translation of phase a1 % 3 with two client random values within a `MaxShudder`
//! of up to 3 and a height in -60..0 (`Gen::mixed` one in 10 each in the second test); 15 (2 %) the bob at a frame
//! (below 100, below 200,000, any 32 bits, or 0); both record `[500, isInstanceIdentity, the 12 words]`. Ops 11-13's
//! shares shrink to 10 %, 3 % and 2 %. C++ leaves the order of `shudder.Set`'s arguments unspecified; the compiled
//! wrapper (g++) evaluates them **right to left** (`ref_mdp_shudder_order`), so the test gives the port x as the second
//! value drawn. `sin` of a `Real` is C's double `sin` (differential/shim/math.h).
//!
//! Predicted (`issue182_predictions.md`, before any run): 0 mismatches in both tests, max ULP 0; plants 9871-9879 and
//! 9883 caught here, 9880-9882 only by the lib tests.
//!
//! - **First run:** did not build (`INT_TO_REAL` is not in the shim's BaseType.h: defined in the wrapper as
//!   BaseType.h:240). **Second:** the bob mismatched by 1-2 ULP: the port used `sinf`, I had predicted the `float`
//!   overload; the wrapper (shim math.h, MSVC's C semantics) calls the double `sin`. **A prediction miss and a port
//!   defect, fixed:** `float_update_tilt` widens to the double sine.
//! - **Third run (2026-11-13):** main **0 mismatches** over 20,000 (411,533 steps, 47,041 systems, 75 NaN words, max ULP
//!   0); any scale **0** over 20,000 (409,915 steps, 46,698 systems, 3,152 NaN words, 151 NaN-payload-only, max ULP 0).
//! - Plants, mismatching of 20,000: 9871 2,660; 9872 3,257; 9873 6,410; 9874 3,541; 9875 5,165; 9876 1,159; 9877 5,165;
//!   9878 1,734; 9879 5,201; 9883 3,610; 9880, 9881, 9882 0 here (the event dispatch is not reached), caught by the lib
//!   tests `the_logic_s_instance_matrices_reach_the_draw_root` and `the_logic_s_set_instance_matrix_reaches_the_drawable`.
//!   **13 of 13 caught.**
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints `MEASURED` lines and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 9661 | the recalc flag is never cleared |
//! | 9662 | the recalc ignores the drawable's `NO_STATE_PARTICLES` |
//! | 9663 | the recalc's bone index is always 0 |
//! | 9664 | the recalc does not turn the system by the bone's Z rotation |
//! | 9665 | the recalc swaps the bone's x and y |
//! | 9666 | the recalc does not attach the system to the drawable |
//! | 9667 | the recalc leaves the system saveable |
//! | 9668 | `setSaveable` does not reach the slave |
//! | 9669 | the recalc's hidden test ignores the shroud |
//! | 9670 | the recalc's hidden test ignores the drawable's hidden flag |
//! | 9671 | the tracker keeps bone 0 |
//! | 9672 | the recalc stops the systems of a drawable that is shown |
//! | 9673 | the update moves systems made at bone 0 |
//! | 9674 | the update does not skip the parent's transform |
//! | 9675 | the update does not set the local transform |
//! | 9676 | the update leaves the bone in model space |
//! | 9677 | the update moves every system to the first tracker's bone |
//! | 9678 | `doStartOrStopParticleSys` ignores the drawable's hidden flag |
//! | 9679 | `stopClientParticleSystems` does not clear the list |
//! | 9680 | `stopClientParticleSystems` stops, not destroys |
//! | 9681 | `setModelState` sets the flag despite `NO_STATE_PARTICLES` |
//! | 9682 | `setModelState` does not stop the old systems |
//! | 9683 | `doStartOrStopParticleSys` ignores the shroud |
//! | 9684 | `doStartOrStopParticleSys` never restarts a system |
//! | 9685 | `setFullyObscuredByShroud` does not keep the flag |
//! | 9686 | `doDrawModule` updates the bones without `ParticlesAttachedToAnimatedBones` |
//! | 9687 | `doDrawModule` never updates the bones |
//! | 9688 | `doDrawModule` recalcs only with a render object |
//! | 9689 | `setHidden` does not stop or start the systems |
//! | 9690 | the recalc's bone position has z 0 |
//! | 9691 | (#172) the update's bone is `world * (bone under Scale)` (the old convention) |
//! | 9692 | (#172) the render object's hierarchy is never scaled |
//! | 9693 | (#172) the hierarchy is scaled within `scale_epsilon` of 1 too |
//! | 9694 | (#172) the live bone is `root * (chain from the identity)` |
//! | 9695 | (#172) `HTreeClass::Scale` scales the pivots' rotations too |
//! | 9696 | (#172) `getCurrentWorldspaceClientBonePositions` is `world * (bone under Scale)` |
//! | 9697 | (#172) `handleWeaponFireFX`'s FX bone is the bone under Scale, not in the world (lib test) |
//! | 9698 | (#172) the live bone ignores its parents (every pivot under the root) |
//! | 9751 | (#174) the live bones ignore the render object's captured bones |
//! | 9753 | (#174) the attach offset is never the pristine bone's (always zero) |
//! | 9754 | (#174) a new render object does not take the old one's transform (`setModelState`) |
//! | 9755 | (#174) `doDrawModule` ignores the instance scale |
//! | 9756 | (#174) `adjustTransformMtx` adds the attach offset unturned |
//! | 9757 | (#174) `adjustTransformMtx` ignores `ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT` |
//! | 9758 | (#174) the construction height is added along world Z, not the matrix's Z (`Translate_Z`) |
//! | 9759 | (#174) the recalc's bone is the skeleton as made (no captured bones) |
//! | 9760 | (#174) `Drawable::set_instance_scale` does not reach the modules (lib test) |
//! | 9762 | (#174) `getCurrentBonePositions`' inverse is not scaled by `getScale()` |
//! | 9763 | (#174) `getCurrentBonePositions` with `startIndex` 0 goes on to `prefix01` ... |
//! | 9764 | (#174) `getCurrentBonePositions` reads the bone under the identity, not the render object's transform |
//! | 9765 | (#174) `getCurrentBonePositions` post-multiplies the inverse |
//! | 9766 | (#174) `getCurrentBonePositions`' positions have z 0 |
//! | 9846 | (#179 item 3) the draw's root ignores the instance matrix |
//! | 9847 | (#179 item 3) the instance matrix is pre-multiplied |
//! | 9848 | (#179 item 3) the physics is applied before the instance matrix |
//! | 9849 | (#179 item 3) the physics totals are never applied to the root |
//! | 9850 | (#179 item 3) `Rotate_X(+roll)` |
//! | 9851 | (#179 item 3) the Y and X rotations swapped |
//! | 9852 | (#179 item 3) the Z translation after the rotations |
//! | 9853 | (#179 item 3) `nukeCurrentRender`'s drawable transform is the draw's root |
//! | 9854 | (#179 item 3) the instance matrix is multiplied even when flagged the identity |
//! | 9855 | (#179 item 3) `setInstanceMatrix(m)` leaves the identity flag set |
//! | 9856 | (#179 item 3) the renderer's local matrix ignores the instance matrix (lib test) |
//! | 9857 | (#179 item 3) `ClientDrawables::draw` hands the modules the drawable's transform (lib test) |
//! | 9870 | (#179 item 3) the pre-fix root: the drawable's transform alone |
//! | 9871 | (#182) the collapse's translation is put on the identity, not on the drawable's instance matrix |
//! | 9872 | (#182) the translation is added (`Translate`), not set |
//! | 9873 | (#182) the translation's x and y swapped |
//! | 9874 | (#182) the translation's z (the collapse's height) dropped |
//! | 9875 | (#182) the bob's yaw and pitch frequencies swapped |
//! | 9876 | (#182) the bob does not keep the Z rotation |
//! | 9877 | (#182) the bob's `Rotate_X` before its `Rotate_Y` |
//! | 9878 | (#182) the bob keeps the old translation |
//! | 9879 | (#182) the bob's angle is the next frame's |
//! | 9880 | (#182) `apply_events` ignores `DrawableSetInstanceTranslation` (lib test) |
//! | 9881 | (#182) `apply_events` ignores `DrawableFloatTilt` (lib test) |
//! | 9882 | (#182) `apply_events` ignores `DrawableSetInstanceMatrix` (lib test) |
//! | 9883 | (#182) the collapse's final zero resets the matrix to the identity, flagged |

mod support;

use std::sync::Arc;

use rts_lockstep_engine::client::drawable::Drawable;
use rts_lockstep_engine::client::drawable_loco::PhysicsXformInfo;
use rts_lockstep_engine::client::particle_sys::ParticleSystemManager;
use rts_lockstep_engine::client::w3d_model_draw::{scale_render_obj_hierarchy, AnimHandle, DrawEnv, DrawFacts, DrawParticles, Skeleton, StateRef, W3DModelDraw, W3DModelDrawModuleData, W3dAssetSource};
use rts_lockstep_engine::formats::w3d::htree::{HTree, Pivot};
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::model_state::{ModelConditionFlags, MODEL_CONDITION_NAMES};
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::thing_template::{ClientLine, ClientModuleData};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use support::{Check, Gen};

/// The wrapper's records are one global buffer: one test at a time.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

/// (#182) `ref_mdp_shudder_order`: 0 when the compiled `shudder.Set(...)` evaluates its arguments left to right (x the
/// first client random value), 1 right to left.
fn shudder_order() -> i32 {
    static ORDER: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    *ORDER.get_or_init(|| {
        let o = unsafe { cpp::ref_mdp_shudder_order() };
        assert!(o == 0 || o == 1, "the shudder's order probe: {o}");
        println!("the compiled shudder.Set evaluates its arguments {}", if o == 0 { "left to right" } else { "right to left" });
        o
    })
}

fn cases() -> u64 {
    std::env::var("MODEL_DRAW_PARTICLES_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000)
}

fn b(f: f32) -> i32 {
    f.to_bits() as i32
}

fn r(i: i32) -> f32 {
    f32::from_bits(i as u32)
}

/// The wrapper's `BONE_NAMES` (the models' pivots) and `STATE_BONE_NAMES` (the `ParticleSysBone` lines'). (#174) Two
/// more pivot names, so `getCurrentBonePositions` finds runs of `Bone01`, `Bone02`, `BONE03`, and `BONE` itself (a
/// prefix that is also a bone: `startIndex` 0 must stop after it).
const BONE_NAMES: [&str; 9] = ["RootTransform", "Bone01", "Smoke01", "FX_A", "Muzzle", "SMOKE01", "Bone02", "BONE03", "BONE"];
/// (#174) The wrapper's `PREFIXES` (`getCurrentBonePositions`').
const PREFIXES: [&str; 7] = ["Bone", "bone", "Smoke", "FX_A", "Muzzle", "RootTransform", "nosuch"];
const STATE_BONE_NAMES: [&str; 6] = ["RootTransform", "bone01", "SMOKE01", "fx_a", "Muzzle", "nosuchbone"];
const MODEL_NAMES: [&str; 5] = ["M0", "M1", "M2", "None", "Missing"];
const TEMPLATE_NAMES: [&str; 6] = ["PS0", "PS1", "PS2", "PS3", "None", "NoSuch"];
/// The states' condition bits (the first state is the default).
const STATE_BITS: [&str; 4] = ["", "DAMAGED", "RUBBLE", "GARRISONED"];

const HEADER: usize = 7 + 18 + 3 * (2 + 6 * 14) + 1 + 4 * (2 + 3 * 2);
const STEP: usize = 14;

/// A world entry that is not well scaled: `Gen::finite` (huge, tiny, subnormal), or (#172, the second test) `Gen::mixed`,
/// infinities and NaN included.
#[derive(Clone, Copy, PartialEq)]
enum Odd {
    None,
    Finite,
    Any,
}

fn matrix_words(g: &mut Gen, lo: f32, hi: f32, t: f32, odd: Odd) -> [i32; 12] {
    let mut w = [0i32; 12];
    for (k, v) in w.iter_mut().enumerate() {
        let f = if odd != Odd::None && g.below(3) == 0 {
            // ~~finite: an infinite world entry separates the two bone conventions too (`inf * 0` in upstream's
            // `world * Scale`, none in the port's), see the results~~ (#172) both sides now run the same chain: the
            // second test draws infinities and NaN too
            if odd == Odd::Any { g.mixed() } else { g.finite() }
        } else if k % 4 == 3 {
            g.uniform(-t, t)
        } else {
            g.uniform(lo, hi)
        };
        *v = b(f);
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

/// One case's words (the wrapper's layout). ~~`any_scale`: a scale in 0.5..2, else a power of two.~~ **Corrected
/// 2026-11-12 (#172):** `any_scale` false (the main test): a power of two (1 included) 40 %, a scale at
/// `scale_epsilon`'s edge (0.99 .. 1.0101) 20 %, any in 0.5..2 40 %, with 1 in 20 world transforms `Gen::finite`; true
/// (the second test): any in 0.25..4 90 %, `Gen::mixed` (0, NaN and infinities included) 10 %, with 1 in 10 world
/// transforms `Gen::mixed`.
fn random_case(g: &mut Gen, any_scale: bool) -> Vec<i32> {
    let mut w = Vec::new();
    w.push(1 + g.below(1000) as i32);
    let scale = if any_scale {
        if g.below(10) == 0 { g.mixed() } else { g.uniform(0.25, 4.0) }
    } else {
        match g.below(10) {
            0..=3 => [0.25f32, 0.5, 1.0, 1.0, 1.0, 2.0, 4.0][g.below(7) as usize],
            // `WWMath::Fabs(scale - 1) > 0.01`: 0.99 and 1.01 are not "really" scales in f32, 0.9899 and 1.0101 are
            4..=5 => [0.9899f32, 0.99, 0.995, 1.005, 1.0099, 1.01, 1.0101][g.below(7) as usize],
            _ => g.uniform(0.5, 2.0),
        }
    };
    w.push(b(scale));
    w.push((g.below(2) == 0) as i32);
    // (#174) AttachToBoneInAnotherModule, its pristine bone (found or not, a pose), ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT,
    // the object (or none), its construction percent (-1 .. 110, a quarter of them 100 or -1) and height
    w.push((g.below(2) == 0) as i32);
    w.push((g.below(4) != 0) as i32);
    w.extend(matrix_words(g, -2.0, 2.0, 50.0, Odd::None));
    w.push((g.below(2) == 0) as i32);
    w.push((g.below(5) != 0) as i32);
    w.push(b(match g.below(8) {
        0 => 100.0,
        1 => -1.0,
        _ => g.uniform(-5.0, 110.0),
    }));
    w.push(b(g.uniform(0.0, 60.0)));
    for t in 0..4u32 {
        w.push(if t < 3 && g.below(3) == 0 { (t + 1 + g.below(3 - t)) as i32 } else { -1 });
    }
    for _ in 0..3 {
        w.push((g.below(5) != 0) as i32);
        w.push(g.below(7) as i32);
        for k in 0..6 {
            w.push(if k == 0 { 0 } else { 1 + g.below(8) as i32 });
            // (#172) the pivot's parent: an earlier pivot (chains up to 5 deep); unused for pivot 0
            w.push(if k == 0 { 0 } else { g.below(k) as i32 });
            if g.below(10) == 0 {
                let mut p = [0i32; 12];
                for (i, v) in p.iter_mut().enumerate() {
                    *v = b(if i % 5 == 0 { 1.0 } else if i % 4 == 3 { g.uniform(-50.0, 50.0) } else { 0.0 });
                }
                w.extend(p);
            } else {
                w.extend(matrix_words(g, -2.0, 2.0, 50.0, Odd::None));
            }
        }
    }
    w.push(1 + g.below(4) as i32);
    for _ in 0..4 {
        w.push(g.below(5) as i32);
        w.push(g.below(4) as i32);
        for _ in 0..3 {
            w.push(g.below(6) as i32);
            w.push(g.below(6) as i32);
        }
    }
    assert_eq!(w.len(), HEADER);
    // ~~only at scale 1~~ (#172) at any scale
    for _ in 0..1 + g.below(40) {
        let mut s = [0i32; STEP];
        // (#174) the captures (8), the instance scale (9) and getCurrentBonePositions (10); (#179 item 3) the draw's
        // root (11), the instance matrix (12) and the physics totals (13)
        let op = match g.below(100) {
            0..=10 => 0,
            11..=28 => 1,
            29..=38 => 2,
            39..=44 => 3,
            45..=50 => 4,
            51..=54 => 5,
            55..=58 => 6,
            59..=63 => 7,
            64..=71 => 8,
            72..=73 => 9,
            74..=79 => 10,
            80..=89 => 11,
            90..=92 => 12,
            93..=94 => 13,
            // (#182) the collapse's translation (14) and the float's bob (15)
            95..=97 => 14,
            _ => 15,
        };
        s[0] = op;
        s[1] = match op {
            3 | 4 | 6 => (g.below(2) == 0) as i32,
            // (#179 item 3) setInstanceMatrix(NULL) 1 in 4; calcPhysicsXform answers false 1 in 5
            12 => (g.below(4) != 0) as i32,
            13 => (g.below(5) != 0) as i32,
            // (#182) the bob's frame: early in a game, any game, any 32 bits, or 0
            15 => match g.below(4) {
                0 => g.below(100) as i32,
                1 => g.below(200_000) as i32,
                2 => g.below(u32::MAX) as i32,
                _ => 0,
            },
            _ => g.below(1000) as i32,
        };
        if op == 8 {
            // a turret's turn about Z (as handleClientTurretPositioning's), a pitch about Y, a recoil's shift along X, or
            // any matrix
            let mut m = Matrix3D::IDENTITY;
            match g.below(4) {
                0 => m.rotate_z(g.uniform(-4.0, 4.0)),
                1 => m.rotate_y(g.uniform(-1.5, 1.5)),
                2 => m.translate_x(-g.uniform(0.0, 3.0)),
                _ => m = matrix(&matrix_words(g, -2.0, 2.0, 10.0, Odd::None)),
            }
            for row in 0..3 {
                for c in 0..4 {
                    s[2 + row * 4 + c] = b(m.row[row][c]);
                }
            }
        } else if op == 9 {
            s[2] = b(match g.below(4) {
                0 => 1.0,
                1 => [0.5f32, 2.0, 0.25, 1.005][g.below(4) as usize],
                _ => g.uniform(0.2, 2.0),
            });
        } else if op == 10 {
            s[2] = [0, 0, 1, 1, 2, 3][g.below(6) as usize];
            s[3] = [1, 2, 3, 64, 70, 0, -1][g.below(7) as usize];
            s[4] = (g.below(4) != 0) as i32;
            s[5] = (g.below(4) != 0) as i32;
        }
        if op == 12 && s[1] != 0 {
            // a sway's tilt (SwayClientUpdate: about X and Y), a float's bob (a lift and a turn), the identity (flagged
            // not), or any matrix
            let mut m = Matrix3D::IDENTITY;
            match g.below(4) {
                0 => {
                    m.rotate_x(g.uniform(-0.2, 0.2));
                    m.rotate_y(g.uniform(-0.2, 0.2));
                }
                1 => {
                    m.translate(0.0, 0.0, g.uniform(-3.0, 3.0));
                    m.rotate_z(g.uniform(-0.3, 0.3));
                }
                2 => {}
                _ => {
                    let odd = if any_scale && g.below(10) == 0 { Odd::Any } else { Odd::None };
                    m = matrix(&matrix_words(g, -2.0, 2.0, 10.0, odd));
                }
            }
            for row in 0..3 {
                for c in 0..4 {
                    s[2 + row * 4 + c] = b(m.row[row][c]);
                }
            }
        } else if op == 14 {
            // (#182) the shudder's two client random values (within a `MaxShudder` up to 3) and the height (falling from
            // 0); `Gen::mixed` one in 10 each in the second test
            let max = g.uniform(0.0, 3.0);
            for i in 0..2 {
                s[2 + i] = b(if any_scale && g.below(10) == 0 { g.mixed() } else { g.uniform(-max, max) });
            }
            s[4] = b(if any_scale && g.below(10) == 0 { g.mixed() } else { -g.uniform(0.0, 60.0) });
        } else if op == 13 {
            // the totals: pitch, roll, yaw, z (all 0 one in 8; `Gen::mixed` one in 10 each in the second test)
            if g.below(8) != 0 {
                for (i, (lo, hi)) in [(-0.6f32, 0.6f32), (-0.6, 0.6), (-0.3, 0.3), (-3.0, 3.0)].into_iter().enumerate() {
                    s[2 + i] = b(if any_scale && g.below(10) == 0 { g.mixed() } else { g.uniform(lo, hi) });
                }
            }
        }
        if op == 1 || op == 2 || op == 7 || op == 11 {
            let odd = if any_scale {
                if g.below(10) == 0 { Odd::Any } else { Odd::None }
            } else if g.below(20) == 0 {
                Odd::Finite
            } else {
                Odd::None
            };
            let m = matrix_words(g, -2.0, 2.0, 1000.0, odd);
            s[2..].copy_from_slice(&m);
        }
        w.extend(s);
    }
    w
}

/// The test's asset manager: the case's three models.
struct Models {
    /// (hlod, bones (name, parent, pose)) of `M0`..`M2`.
    models: Vec<(bool, Vec<(&'static str, u32, Matrix3D)>)>,
}

impl Models {
    fn find(&self, name: &[u8]) -> Option<usize> {
        MODEL_NAMES[..3].iter().position(|n| n.as_bytes().eq_ignore_ascii_case(name))
    }
}

impl W3dAssetSource for Models {
    fn create_render_obj(&mut self, name: &[u8]) -> Option<bool> {
        self.find(name).map(|i| self.models[i].0)
    }
    fn get_hanim(&mut self, _: &[u8]) -> Option<AnimHandle> {
        None
    }
    fn skeleton(&mut self, name: &[u8], _pose: Option<(&[u8], f32)>, scale: f32) -> Option<Skeleton> {
        let i = self.find(name)?;
        let (hlod, bones) = &self.models[i];
        if !hlod {
            return Some(Skeleton::default());
        }
        if bones.is_empty() {
            // an HLOD without pivots: no hierarchy
            return Some(Skeleton::default());
        }
        // ~~each bone under `Matrix3D(true).Scale(scale)` (`W3dFiles::skeleton`'s root)~~ (#172) as `W3dFiles::skeleton`
        // makes it: the hierarchy scaled as `Create_Render_Obj(name, scale, color)` scales it (the port's
        // `scale_render_obj_hierarchy`, `HTree::scale`), posed by the port's `Base_Update` under
        // `Matrix3D(true).Scale(scale)`
        let pivots = bones
            .iter()
            .enumerate()
            .map(|(i, (n, parent, pose))| {
                let mut name = [0u8; 16];
                name[..n.len()].copy_from_slice(n.as_bytes());
                Pivot::new(name, if i == 0 { None } else { Some(*parent) }, *pose, i as i32)
            })
            .collect();
        let mut tree = HTree { name: [0; 16], num_pivots: bones.len() as i32, pivots, scale_factor: 1.0 };
        scale_render_obj_hierarchy(&mut tree, scale);
        let mut root = Matrix3D::IDENTITY;
        root.scale(scale);
        tree.base_update(&root).expect("parents come first");
        Some(Skeleton { bones: Skeleton::bones_of(&tree), sub_objects: Vec::new(), scale_factor: tree.scale_factor })
    }
}

fn line(depth: u32, field: &str, args: &str) -> ClientLine {
    ClientLine { depth, field: field.as_bytes().to_vec(), args: args.split_whitespace().map(|t| t.as_bytes().to_vec()).collect() }
}

fn flags(name: &str) -> ModelConditionFlags {
    if name.is_empty() {
        return ModelConditionFlags::new();
    }
    let i = MODEL_CONDITION_NAMES.iter().position(|n| *n == name).expect("a condition") as i32;
    ModelConditionFlags::from_indices(&[i])
}

fn state_words(out: &mut Vec<i32>, m: &W3DModelDraw, mgr: &ParticleSystemManager) {
    out.extend([100, m.need_recalc_bone_particle_systems as i32, m.fully_obscured_by_shroud as i32, m.particle_system_ids.len() as i32]);
    for t in &m.particle_system_ids {
        out.extend([t.id as i32, t.bone_index]);
    }
    let mut l: Vec<_> = mgr.systems.iter().collect();
    l.sort_by_key(|s| s.id);
    out.push(l.len() as i32);
    for s in l {
        out.extend([
            s.id as i32,
            s.attached_to_drawable as i32,
            s.is_saveable as i32,
            s.is_stopped as i32,
            s.is_destroyed as i32,
            s.skip_parent_xfrm as i32,
            s.is_local_identity as i32,
            s.slave.unwrap_or(0) as i32,
            s.master.unwrap_or(0) as i32,
        ]);
        for row in 0..3 {
            for c in 0..4 {
                out.push(b(s.local_transform.row[row][c]));
            }
        }
    }
    // (#174) the render object's transform and object scale
    let ro = m.render_object.as_ref();
    out.push(ro.is_some() as i32);
    for row in 0..3 {
        for c in 0..4 {
            out.push(ro.map_or(0, |ro| b(ro.transform.row[row][c])));
        }
    }
    out.push(ro.map_or(0, |ro| b(ro.object_scale)));
}

/// The Rust's words for a case (the wrapper's records).
fn rust_case(w: &[i32]) -> Vec<i32> {
    let mut k = 0;
    let mut next = || {
        k += 1;
        w[k - 1]
    };
    let drawable_id = next();
    let scale = r(next());
    let attached = next() != 0;
    // (#174)
    let attach_bone = next() != 0;
    let attach_found = next() != 0;
    let mut am = [0i32; 12];
    for v in &mut am {
        *v = next();
    }
    let attach_mtx = matrix(&am);
    let adjust_height = next() != 0;
    let has_object = next() != 0;
    let construction_percent = r(next());
    let max_height = r(next());
    let construction = has_object.then_some((construction_percent, max_height));
    let mut mgr = ParticleSystemManager::new();
    let mut slaves = [-1i32; 4];
    for s in &mut slaves {
        *s = next();
    }
    for (t, &sl) in slaves.iter().enumerate() {
        let tm = mgr.new_template(TEMPLATE_NAMES[t].as_bytes());
        tm.info.slave_system_name = if sl < 0 { Vec::new() } else { TEMPLATE_NAMES[sl as usize].as_bytes().to_vec() };
    }
    let mut models = Models { models: Vec::new() };
    for _ in 0..3 {
        let hlod = next() != 0;
        let n = next() as usize;
        let mut bones = Vec::new();
        for bi in 0..6 {
            let code = next() as usize;
            let parent = next() as u32;
            let mut p = [0i32; 12];
            for v in &mut p {
                *v = next();
            }
            if bi < n {
                bones.push((BONE_NAMES[code], parent, matrix(&p)));
            }
        }
        models.models.push((hlod, bones));
    }
    let nstates = next() as usize;
    let mut lines = vec![line(0, "ParticlesAttachedToAnimatedBones", if attached { "Yes" } else { "No" })];
    if attach_bone {
        lines.push(line(0, "AttachToBoneInAnotherModule", "attachbone"));
    }
    for (si, bit) in STATE_BITS.iter().enumerate() {
        let model = next() as usize;
        let nb = next() as usize;
        let mut bones = Vec::new();
        for bi in 0..3 {
            let bone = next() as usize;
            let tmpl = next() as usize;
            if bi < nb {
                bones.push(format!("{} {}", STATE_BONE_NAMES[bone], TEMPLATE_NAMES[tmpl]));
            }
        }
        if si >= nstates {
            continue;
        }
        lines.push(if si == 0 { line(0, "DefaultConditionState", "") } else { line(0, "ConditionState", bit) });
        lines.push(line(1, "Model", MODEL_NAMES[model]));
        if adjust_height {
            lines.push(line(1, "Flags", "ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT"));
        }
        for bl in &bones {
            lines.push(line(1, "ParticleSysBone", bl));
        }
    }
    let block = ClientModuleData { module_tag_name_key: Default::default(), lines };
    let data = Arc::new(W3DModelDrawModuleData::parse(b"W3DModelDraw", &block).expect("parses").expect("a model draw"));
    let states: Vec<usize> = STATE_BITS[..nstates].iter().map(|bit| data.find_best_info(&flags(bit)).expect("a state")).collect();
    let mut random = RandomValues::new(1);
    let mut out = Vec::new();
    // (#179 item 3) the drawable (its instance matrix and physics totals) and the case's `calcPhysicsXform` answer
    let mut d = Drawable::bare(1, b"T".to_vec());
    let mut physics: Option<PhysicsXformInfo> = None;
    let mut m = {
        let mut env = DrawEnv { assets: &mut models, random: &mut random, sync_time: 0, logic_time: false, particles: Some(DrawParticles { manager: &mut mgr, frame: 0 }) };
        W3DModelDraw::new_scaled(data.clone(), 0, false, false, scale, Arc::new(Vec::new()), &mut env).expect("an idle state")
    };
    m.drawable_id = drawable_id;
    // (#174) the drawable fills the module data's attach offset from its pristine bones (the case's answer), as the
    // client's `Drawable::validate_attach_to_drawable_bone_offsets` does when it is made
    data.get_attach_to_drawable_bone_offset(|| attach_found.then_some(attach_mtx));
    state_words(&mut out, &m, &mgr);
    for s in w[k..].chunks_exact(STEP) {
        match s[0] {
            0 => {
                let mut env = DrawEnv { assets: &mut models, random: &mut random, sync_time: 0, logic_time: false, particles: Some(DrawParticles { manager: &mut mgr, frame: 0 }) };
                m.set_model_state(StateRef::Normal(states[s[1] as usize % nstates]), &mut env);
            }
            1 => {
                let facts = DrawFacts {
                    should_animate_with_power: true,
                    should_animate_without_power: true,
                    speed: None,
                    turrets: [None, None],
                    motion: None,
                    show_client_physics: true,
                    wheel_info: None,
                    transform: matrix(&s[2..]),
                    drawable_transform: matrix(&s[2..]),
                    construction,
                };
                let mut env = DrawEnv { assets: &mut models, random: &mut random, sync_time: 0, logic_time: false, particles: Some(DrawParticles { manager: &mut mgr, frame: 0 }) };
                m.do_draw_module(&facts, &mut env);
            }
            2 => {
                // (#174) the render object's transform is the module's: `reactToTransformChange` first
                let world = matrix(&s[2..]);
                m.react_to_transform_change(&world, construction);
                // (#179) the render object's own transform (no animation here: sync time 0)
                m.update_bones_for_client_particle_systems(Some(&mut mgr), 0);
            }
            3 => m.set_hidden(s[1] != 0, Some(&mut mgr)),
            4 => m.set_fully_obscured_by_shroud(s[1] != 0, Some(&mut mgr)),
            5 => {
                if !mgr.systems.is_empty() {
                    let mut ids: Vec<u32> = mgr.systems.iter().map(|x| x.id).collect();
                    ids.sort_unstable();
                    let id = ids[s[1] as usize % ids.len()];
                    // `~ParticleSystem`: unlinked from its slave and master, then unlisted
                    let i = mgr.systems.iter().position(|x| x.id == id).expect("listed");
                    if let Some(sl) = mgr.systems[i].slave.take() {
                        if let Some(j) = mgr.systems.iter().position(|x| x.id == sl) {
                            mgr.systems[j].master = None;
                        }
                    }
                    if let Some(ma) = mgr.systems[i].master.take() {
                        if let Some(j) = mgr.systems.iter().position(|x| x.id == ma) {
                            mgr.systems[j].slave = None;
                        }
                    }
                    mgr.systems.remove(i);
                }
            }
            6 => m.drawable_no_state_particles = s[1] != 0,
            8 => {
                // (#174) `Capture_Bone(b)`, `Control_Bone(b, m)` (handleClientTurretPositioning's and handleClientRecoil's)
                let n = m.render_object.as_ref().filter(|ro| ro.is_hlod).and_then(|ro| ro.skeleton.as_ref()).map_or(0, |k| k.bones.len());
                if n != 0 {
                    let bone = (s[1] as u32 % n as u32) as i32;
                    let ro = m.render_object.as_mut().expect("a render object");
                    ro.capture_bone(bone);
                    ro.control_bone(bone, &matrix(&s[2..]));
                }
            }
            // (#174) `setInstanceScale` (`Drawable::set_instance_scale`: the modules' `getScale()`)
            9 => m.scale = r(s[2]),
            11 => {
                // (#179 item 3) `Drawable::draw`: `applyPhysicsXform`'s totals are the drawable's for this draw
                // (`Drawable::apply_physics_xform`: none without an object, `physics_xform_facts`' `apply`), then the root
                // (`Drawable::draw_transform_mtx`) handed to `doDrawModule` as `ClientDrawables::draw` hands it
                let world = matrix(&s[2..]);
                d.physics_xform = if has_object { physics } else { None };
                let root = d.draw_transform_mtx(&world);
                out.push(400);
                for row in 0..3 {
                    for c in 0..4 {
                        out.push(b(root.row[row][c]));
                    }
                }
                let facts = DrawFacts {
                    should_animate_with_power: true,
                    should_animate_without_power: true,
                    speed: None,
                    turrets: [None, None],
                    motion: None,
                    show_client_physics: true,
                    wheel_info: None,
                    transform: root,
                    drawable_transform: world,
                    construction,
                };
                let mut env = DrawEnv { assets: &mut models, random: &mut random, sync_time: 0, logic_time: false, particles: Some(DrawParticles { manager: &mut mgr, frame: 0 }) };
                m.do_draw_module(&facts, &mut env);
            }
            12 => {
                let mtx = matrix(&s[2..]);
                d.set_instance_matrix(if s[1] != 0 { Some(&mtx) } else { None });
            }
            // (#182) `StructureCollapseUpdate`'s translation as the logic's event carries it (`shudder`: the two client
            // random values and 0, the height, or zero), then the client's `Drawable::set_instance_translation`
            14 | 15 => {
                if s[0] == 14 {
                    // the two random values in the order the compiled `shudder.Set(...)` draws them (C++ leaves it
                    // unspecified; the logic's event carries x and y whichever was drawn first)
                    let (x, y) = if shudder_order() == 0 { (r(s[2]), r(s[3])) } else { (r(s[3]), r(s[2])) };
                    let shudder = match s[1] as u32 % 3 {
                        0 => Coord3D { x, y, z: 0.0 },
                        1 => Coord3D { x, y, z: r(s[4]) },
                        _ => Coord3D { x: 0.0, y: 0.0, z: 0.0 },
                    };
                    d.set_instance_translation(&shudder);
                } else {
                    // `FloatUpdate`'s bob (the logic's `DrawableFloatTilt { frame }`)
                    d.float_update_tilt(s[1] as u32);
                }
                out.extend([500, d.saved.instance_is_identity as i32]);
                for row in 0..3 {
                    for c in 0..4 {
                        out.push(b(d.saved.instance.row[row][c]));
                    }
                }
            }
            13 => {
                physics = (s[1] != 0).then_some(PhysicsXformInfo { total_pitch: r(s[2]), total_roll: r(s[3]), total_yaw: r(s[4]), total_z: r(s[5]) });
            }
            10 => {
                let mut pos = [Coord3D::default(); 64];
                let mut mtx = [Matrix3D::IDENTITY; 64];
                let n = m.get_current_bone_positions(
                    PREFIXES[s[1] as usize % 7].as_bytes(),
                    s[2],
                    if s[4] != 0 { Some(&mut pos) } else { None },
                    if s[5] != 0 { Some(&mut mtx) } else { None },
                    s[3],
                    0,
                );
                out.extend([300, n]);
                for i in 0..(n.max(0) as usize).min(64) {
                    if s[4] != 0 {
                        out.extend([b(pos[i].x), b(pos[i].y), b(pos[i].z)]);
                    } else {
                        out.extend([0; 3]);
                    }
                    for row in 0..3 {
                        for c in 0..4 {
                            out.push(if s[5] != 0 { b(mtx[i].row[row][c]) } else { 0 });
                        }
                    }
                }
            }
            _ => {
                // (#172) `getCurrentWorldspaceClientBonePositions(name, transform)` under the render object's transform;
                // (#174) the module's: `reactToTransformChange` first
                let world = matrix(&s[2..]);
                m.react_to_transform_change(&world, construction);
                match m.get_current_worldspace_client_bone_positions(STATE_BONE_NAMES[s[1] as usize % 6].as_bytes(), 0) {
                    Some(t) => {
                        out.extend([200, 1]);
                        for row in 0..3 {
                            for c in 0..4 {
                                out.push(b(t.row[row][c]));
                            }
                        }
                    }
                    None => {
                        out.extend([200, 0]);
                        out.extend([0; 12]);
                    }
                }
            }
        }
        state_words(&mut out, &m, &mgr);
    }
    SYSTEMS_MADE.store(u64::from(mgr.unique_system_id), std::sync::atomic::Ordering::Relaxed);
    out
}

fn run_cpp(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 1 << 20];
    let n = unsafe { cpp::ref_model_draw_particles_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n as usize <= out.len());
    out.truncate(n as usize);
    out
}

/// The largest |rust - c++| / max(|rust|, |c++|, 1) over differing finite words (f64 bits).
static MAX_REL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The systems the last Rust case made (its manager's last id).
static SYSTEMS_MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Both runs of `n` cases; returns the check and (steps, systems made, NaN words, NaN-only differences, max ULP).
fn compare(name: &str, seed: u64, n: u64, any_scale: bool) -> (Check, [u64; 5]) {
    let mut c = Check::new(name);
    let mut g = Gen::new(seed);
    let (mut steps, mut systems, mut nan_words, mut nan_only, mut max_ulp) = (0u64, 0u64, 0u64, 0u64, 0u64);
    for k in 0..n {
        let w = random_case(&mut g, any_scale);
        let mut rust = rust_case(&w);
        let mut cc = run_cpp(&w);
        if rust.len() == cc.len() {
            for (a, bb) in rust.iter_mut().zip(cc.iter_mut()) {
                if a != bb && r(*a).is_nan() && r(*bb).is_nan() {
                    nan_only += 1;
                    *a = 0x7FC0_0000;
                    *bb = 0x7FC0_0000;
                }
            }
        }
        steps += ((w.len() - HEADER) / STEP) as u64;
        systems += SYSTEMS_MADE.swap(0, std::sync::atomic::Ordering::Relaxed);
        nan_words += rust.iter().filter(|&&x| r(x).is_nan()).count() as u64;
        if rust != cc {
            if rust.len() == cc.len() {
                for (a, bb) in rust.iter().zip(&cc) {
                    let (fa, fb) = (r(*a), r(*bb));
                    if a != bb && fa.is_finite() && fb.is_finite() {
                        max_ulp = max_ulp.max(support::ulp_distance(fa, fb));
                        // the difference against the larger of the two magnitudes and 1 (a word near zero after a
                        // cancellation can be many ULP apart and still be a rounding)
                        let d = f64::from(fa) - f64::from(fb);
                        let scale = f64::from(fa).abs().max(f64::from(fb).abs()).max(1.0);
                        MAX_REL.store(f64::to_bits((d.abs() / scale).max(f64::from_bits(MAX_REL.load(std::sync::atomic::Ordering::Relaxed)))), std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }
            let at = rust.iter().zip(&cc).position(|(a, bb)| a != bb).unwrap_or(rust.len().min(cc.len()));
            if !planted() && c.mismatches < 4 {
                let lo = at.saturating_sub(6);
                println!("case {k}: at word {at}: rust {:?} c++ {:?}", &rust[lo.min(rust.len())..(at + 16).min(rust.len())], &cc[lo.min(cc.len())..(at + 16).min(cc.len())]);
            }
        }
        c.eq(rust, cc, || format!("case {k}"));
    }
    (c, [steps, systems, nan_words, nan_only, max_ulp])
}

#[test]
fn the_bone_particle_systems_match_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (c, [steps, systems, nan_words, nan_only, max_ulp]) = compare("W3DModelDraw bone particle systems", 0x171_0001, cases(), false);
    println!(
        "W3DModelDraw particles: {} cases, {steps} steps, {systems} particle systems made, {nan_words} NaN words ({nan_only} NaN on both sides with different bits); 0 undefined upstream (none generated); max ULP among differing finite words {max_ulp}",
        cases()
    );
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    }
    c.finish();
}

/// ~~Not gated: the rounding of the port's bone convention at scales that are not powers of two (module docs).~~
/// **Corrected 2026-11-12 (#172):** gated at 0 mismatches: any scale (0.25..4, and `Gen::mixed`'s 0, NaN, infinities),
/// world transforms with infinite and NaN entries, 20,000 cases (`MODEL_DRAW_PARTICLES_ANY_CASES`).
#[test]
fn any_scale_matches_upstream() {
    if planted() {
        return;
    }
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let n = std::env::var("MODEL_DRAW_PARTICLES_ANY_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000);
    let (c, [steps, systems, nan_words, nan_only, max_ulp]) = compare("W3DModelDraw particles, any scale", 0x171_0002, n, true);
    let rel = f64::from_bits(MAX_REL.load(std::sync::atomic::Ordering::Relaxed));
    println!(
        "any scale: {n} cases, {steps} steps, {systems} particle systems made, {nan_words} NaN words ({nan_only} NaN on both sides with different bits); max ULP among differing finite words {max_ulp}, largest difference {rel:e} of max(|value|, 1)"
    );
    c.finish();
}
