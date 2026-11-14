#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp (doDrawModule,
//   setPauseAnimation, getCurrentAnimFraction, adjustAnimation, setCurAnimDurationInMsec, getCurAnimDistanceCovered,
//   adjustAnimSpeedToMovementSpeed, isAnimationComplete, handleClientRecoil, the bone particle systems, and setModelState's
//   head and tail; #181 item 7b: setModelState's middle, validateStuff, validateCachedBones, validateTurretInfo,
//   validateWeaponBarrelInfo, findPristineBone, nukeCurrentRender, hideAllHeadlights, hideAllMuzzleFlashes,
//   setMuzzleFlashHidden, rebuildWeaponRecoilInfo, handleClientTurretPositioning, handleWeaponFireFX; #181 item 7c:
//   setModelState's transition head, findTransitionForSig, setAnimationLoopDuration, setAnimationCompletionTime,
//   W3DAnimationInfo, doHideShowBoneSubObjs, doHideShowSubObjs, showSubObject, updateSubObjects, findSingleSubObj) over GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/animobj.cpp (Animatable3DObjClass), htree.cpp and
//   hrawanim.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! GitHub #181 item 7: `W3DModelDraw`'s draw sequence with an **animated** render object against the upstream text
//! compiled verbatim (`differential/wrap_model_draw_anim.cpp`, which lists the cuts, the stand-ins and the words).
//! `tests/differential_model_draw_particles.rs` covers the bone particle systems over models without animation, and
//! `tests/differential_w3d.rs` (ops 40-50) `Animatable3DObjClass` alone; this one covers their sequence.
//!
//! # What upstream does at one sync time
//!
//! `doDrawModule` (W3DModelDraw.cpp:2040-2112): `setPauseAnimation` (a pause re-sets the animation in `MANUAL`, keeping
//! the mode to resume in), the instance scale, then for a completed animation (`isAnimationComplete`: an HLOD's
//! `Is_Animation_Complete`) `adjustAnimation` again for an idle animation or `RESTART_ANIM_WHEN_COMPLETE` (client random
//! values: which animation, a random start frame, the speed factor), `adjustAnimSpeedToMovementSpeed`, the render
//! object's `Set_Transform` (the hierarchy no longer valid), then `recalcBonesForClientParticleSystems`: its
//! `Get_Bone_Transform` under `Matrix3D(true).Scale(getScale())` runs `Update_Sub_Object_Transforms`, whose
//! `Single_Anim_Progress` **moves the animation on** to the sync time before the pose is evaluated, and its restore
//! invalidates the hierarchy again; then (with `ParticlesAttachedToAnimatedBones`) `updateBonesForClientParticleSystems`,
//! whose first `Get_Bone_Transform` progresses again (by nothing at the same sync time) and evaluates the pose under the
//! render object's transform, the next ones reading it; then `handleClientRecoil`. The scene's `Render` later moves the
//! animation on again and evaluates the pose when the hierarchy is not valid.
//!
//! # Methodology
//!
//! Each case is a W3D file (one hierarchy `SKEL` of 2-6 pivots `B0`.., each under a random earlier pivot, with a random
//! translation in -2..2 and a unit rotation; then 1-4 raw animations `SKEL.A0`.. of 1-40 frames at 30 fps (15 or 60 one
//! in 8 each), with X, Y, Z and quaternion channels on random pivots and frame ranges), loaded by the REAL loaders on
//! the C++ side and by the port's on the Rust side; a drawable of a random id and scale (1, 0.5, 2, 1.005, 0.99 or any
//! in 0.5..2), with `ParticlesAttachedToAnimatedBones` (70 %) and `AnimationsRequirePower` (50 %); four particle system
//! templates with random slaves; 1-4 states, each a model (`M0`..`M2`, all on `SKEL`, or `None` 1 in 8), an
//! `AnimationMode` (any of the six), `Flags` (each of `RANDOMSTART`, `START_FRAME_FIRST`, `START_FRAME_LAST`,
//! `PRISTINE_BONE_POS_IN_FINAL_FRAME`, the four `MAINTAIN_FRAME_ACROSS_STATES*` and `RESTART_ANIM_WHEN_COMPLETE` 1 in 6),
//! an `AnimationSpeedFactorRange` (from 0.5 .. 2), 0-3 `Animation`/`IdleAnimation` lines (an animation of the file, or
//! one that is not there 1 in 12; a distance covered 0, 5, 12.5 or 30; a state's lines all idle 1 in 3, its mode then `ONCE` or
//! `ONCE_BACKWARDS`, as `parseConditionState` requires) and 0-3 `ParticleSysBone`s. The
//! sync time starts anywhere in 0..100,000 (1 in 20 near `u32::MAX`, to wrap). Then 1-60 steps (**Corrected 2026-11-13, #181 item 7b:** the percentages that follow are #181 item 7's; now 4, 22, 16, 12, 2, 3, 3, 3, 3, 1, 2, 2 and 4 % in this order, with ops 13-15 below; the generated pivots are 2-8): `setModelState` 8 %,
//! `doDrawModule` under a random world transform 25 %, the sync time moved on 20 % (0, 33, < 100, 100-3,100 or < 100,000
//! ms), the scene's `Render` 15 %, the drawable hidden or shown 3 %, `getShouldAnimate` 4 %, the object and its speed
//! 4 %, a bone captured and controlled (or released) 5 %, `reactToTransformChange` + `updateBonesForClientParticleSystems`
//! 4 %, a particle system dropped 2 %, the instance scale 2 %, the shroud 3 %, a `Get_Bone_Transform` query 5 %
//! (standing for another reader, as `handleWeaponFireFX`'s FX bone). Compared, every word as bits, after the
//! constructor and after each step: the module (`m_needRecalcBoneParticleSystems`, the shroud flag,
//! `m_whichAnimInCurState`, `m_pauseAnimation`, `m_animationMode`, the current state), the trackers, every particle
//! system (flags, slave, master, the local transform: where it is placed), the render object (motion mode, hierarchy
//! valid, the animation, `Frame`, `PrevFrame`, `LastSyncTime`, direction, speed multiplier, mode, complete, hidden,
//! transform, object scale, and **every pivot's transform**), and the client random state (6 words).
//!
//! ~~Both sides run outside a logic time, as the port's client draws (`isValidTimeToCalcLogicStuff()` false): the pristine
//! bones, turrets and barrels are never valid, so no barrel fires and the recoil does nothing (gaps).~~ **Corrected
//! 2026-11-13 (#181 item 7b):** draws run outside a logic time; `setModelState` runs outside one (op 0) or inside the
//! logic's update (op 13), where `validateStuff` validates the new state on the new render object.
//!
//! # #181 item 7b: the logic-time state change, the turrets, the fire and the recoil
//!
//! What upstream does: a `setModelState` inside the logic's update makes the render object again when the model differs
//! or `turretNamesDiffer` (true until both states are `TURRETS_VALID`), then `validateStuff` on the **new** render object:
//! `validateCachedBones` sets the state's first animation on it in `MANUAL` (frame 0, or the last under
//! `PRISTINE_BONE_POS_IN_FINAL_FRAME`), `Set_Transform(Matrix3D(true).Scale(scale))`, finds the global and the state's
//! public bones (and `NN` variants) by `Get_Bone_Index`, the first found evaluating the live hierarchy
//! (`Get_Bone_Transform`), restores the transform, and leaves the render object on that animation in `MANUAL` (it had
//! none); `validateTurretInfo` and `validateWeaponBarrelInfo` find the turrets' and barrels' bones among them;
//! `rebuildWeaponRecoilInfo` makes one cleared recoil per barrel. Each draw then turns the turret bones by the AI's
//! angle and pitch plus the art angle and pitch (`handleClientTurretPositioning`), and moves each barrel's recoil bone
//! (`handleClientRecoil`: the shift grows by the rate, damped, to `MaxRecoilDistance` or until the rate is under 0.01,
//! then settles back by `RecoilSettleSpeed`), showing a muzzle flash only on the draw after a fire. A fire
//! (`handleWeaponFireFX`) plays the FX list at the FX bone (`Get_Bone_Transform`: the animation moved on) or, hidden
//! with an object, at the logic object, starts the recoil and shows the muzzle flash. `handleWeaponPreAttackFX` and
//! `doClientRecoil` do not exist upstream (GeneralsMD/Code searched): nothing to translate.
//!
//! Methodology added: 2-8 pivots, each but the root `B<i>` or (1 in 2) a turret, barrel, muzzle or FX name, numbered
//! or not; `InitialRecoilSpeed`, `MaxRecoilDistance`, `RecoilDamping`, `RecoilSettleSpeed`; 0-4 sub-objects on bones
//! (2 in 3 on a muzzle pivot when there is one), hidden as made 1 in 3, a `HEADLIGHT` 1 in 5; 0-2 standard public bones
//! and `ExtraPublicBone` names; each state's `Turret`/`TurretPitch`/`AltTurret`/`AltTurretPitch` with art angles (a
//! condition state its own 1 in 2, else the default's) and `WeaponFireFXBone`/`WeaponRecoilBone`/`WeaponMuzzleFlash`/
//! `WeaponLaunchBone` per slot (a line 1 in 2). Ops: the logic-time `setModelState` 6 %, a fire 13 % (a barrel -1, 0-3
//! or 5, an FX list 3 in 4), the AI and its turrets 4 %. Compared besides: each sub-object's hidden flag, the recoils, and
//! every state's caches (the valid bits, the pristine bones with their matrices, the turrets' bones, the barrels), and
//! where each FX list plays.
//!
//! # Predictions (`issue181i7_predictions.md`, before any run)
//!
//! 0 mismatches over 20,000 cases, max ULP 0, client random state equal everywhere, 0 NaN words; plants 9956-9975 all
//! caught here, each 200 - 15,000 of 20,000. (The NaN count was not kept: most words are integers, so it measured
//! nothing.)
//!
//! # Results (2026-11-13)
//!
//! - **First build:** 3 errors, the wrapper's (`DEBUG_ASSERTCRASH` undefined in the W3D setting; an ambiguous
//!   `RenderInfoClass`). Then the generator broke `parseConditionState`'s rules (idle and non-idle animations in one state;
//!   idle animations outside `ONCE`/`ONCE_BACKWARDS`): fixed in the generator.
//! - **First run** (300 cases): 232 differ, every one first in the client random state: the wrapper's particle system
//!   manager drew none of `ParticleSystem`'s constructor's three wind angles (ParticleSys.cpp:1142-1148), which the port
//!   draws. The wrapper's error: those lines are cut verbatim now.
//! - **Second run** (300): 59 differ: upstream makes the render object again on every state change outside a logic time
//!   (`turretNamesDiffer` is true while no state is `TURRETS_VALID`), as the port does; the wrapper's stand-in kept it
//!   when the model was the same. The wrapper's error: `turretNamesDiffer` is cut now.
//! - **Third run** (20,000): 88 differ, each a `setModelState` from a state with a maintain-frame flag while the render
//!   object played no animation: upstream's `getCurrentAnimFraction` reads its uninitialised `framenum` and
//!   `numFrames` there (`Peek_Animation_And_Info` answers NULL and is not tested). **Undefined upstream**, not a port
//!   defect (the port answers -1); both sides now end the case there ([700]). **A prediction miss** (I predicted 0).
//! - **Fourth run (gated):** 20,000 cases (1,704 ended where upstream is undefined), 573,806 steps, 143,926 draws (99,680
//!   with an animation playing), 55,365 particle systems made, 220,148 client random values drawn: **0 mismatches**, max
//!   ULP 0, the client random state equal after every step.
//! - **Plants**, mismatching cases of 20,000: 9956 13,839; 9957 13,001; 9958 3,598; 9959 4,145; 9960 472; 9961 2,533;
//!   9962 642; 9963 1,243; 9964 2,816; 9965 1,064; 9966 5,373; 9967 4,439; 9968 4,439; 9969 2,069; 9970 9,763; 9971
//!   6,445; 9972 8,988; 9973 13,304; 9974 268; 9975 1,493. **20 of 20 caught.**
//!
//! # #181 item 7b predictions and results (`issue181i7b_predictions.md`, before any run)
//!
//! Predicted: two port defects show on the first run, then 0 mismatches. What happened: both were found by reading the
//! port against upstream and **fixed before the first run** (so the first comparison never saw them; their plants
//! below measure them): (1) the port's `validateCachedBones` read a fresh skeleton and did not pose the live render
//! object, so the live pivots kept their identity transforms where upstream's hold the evaluated pose (plant 9976,
//! which restores that effect: 6,456 of 20,000 cases differ); (2) `handleWeaponFireFX` did not ask the FX bone of a hidden
//! render object whose drawable has no object (plant 9991, the old code: 96 of 20,000).
//!
//! - **First build:** no error. **First run** (300 cases): 0 mismatches.
//! - **Gated run** (20,000): 2,888 cases ended where upstream is undefined (1,836 at a `setModelState`, 1,052 at a fire's
//!   barrel equal to the count), 548,231 steps, 121,072 draws (83,880 with an animation playing, 9,303 with turret bones,
//!   12,490 with recoil bones or muzzle flashes), 1,577 fires handled, 56,692 particle systems made, 225,758 client
//!   random values drawn: **0 mismatches**, max ULP 0.
//! - **Plants 9976-9995**, mismatching cases of 20,000: 9976 6,456; 9977 5,696; 9978 6,142; 9979 2,837; 9980 140; 9981
//!   767; 9982 947; 9983 1,150; 9984 507; 9985 237; 9986 463; 9987 477; 9988 11; 9989 178; 9990 55; 9991 96; 9992 2,001;
//!   9993 537; 9994 16; 9995 4,353. **20 of 20 caught.** 9995 first planted the restore of the old animation's speed
//!   multiplier in `validateCachedBones`: 0 caught, because that restore cannot run from `setModelState` (a render
//!   object made again has no animation; a kept one's state is already valid); moved to `hideAllHeadlights`.
//! - Prediction misses: the defects did not show in a run (fixed first); 9988 (11) and 9994 (16) are weaker than the
//!   100 predicted; 1,052 undefined barrels, inside the 100-1,000 predicted only just above it.
//!
//! # #181 item 7c: transition states and the show/hide sub-object lists
//!
//! What upstream does (W3DModelDraw.cpp): `setModelState` returns at once for the current state with nothing pending,
//! or for the pending state; a new state whose `WaitForStateToFinishIfPossible` key is the current state's
//! `TransitionKey`, while the render object's animation is not complete, only becomes pending (`m_nextState`); else, when
//! both states have a `TransitionKey` and a `TransitionState <current key> <new key>` exists (`findTransitionForSig`),
//! that TransitionState becomes current and the requested one pending. `doDrawModule` switches to the pending state
//! when the animation completes (`isAnimationComplete`: true without a render object), restoring a pending duration
//! (`m_nextStateAnimLoopDuration`) with `setAnimationLoopDuration`; `setAnimationCompletionTime` splits a duration
//! between a TransitionState and its pending state by their first animations' natural durations
//! (`W3DAnimationInfo::m_naturalDurationInMsec`, -1 until `getAnimHandle` first finds the animation). Each state change
//! applies the state's `ShowSubObject`/`HideSubObject` list (`doHideShowSubObjs`: the sub-object by name, exact or after
//! its first `.`, hidden or shown with every sub-object whose bone descends from its bone, `doHideShowBoneSubObjs`), then
//! the drawable's own `showSubObject` list (`updateSubObjects`). `validateCachedBones` finds a public bone that is no
//! pivot's as a sub-object (`findSingleSubObj`), reading the **live** sub-object's `Get_Transform`, whose
//! `Validate_Transform` has the HLOD update its sub-objects (evaluating its hierarchy) when their transforms are dirty.
//!
//! Methodology added: sub-objects named `SKEL.GUN`, `SKEL.GUN01`, `SKEL.GUN02`, `SKEL.DOOR` (1 in 2) besides `SUBk` and
//! `HEADLIGHTk`, and `GUN`/`DOOR` among the public, turret and weapon bone names; each state's `TransitionKey` (`k0`..`k2`,
//! no line 1 in 4: a ConditionState keeps the default state's), `WaitForStateToFinishIfPossible` (1 in 3) and 0-2
//! `ShowSubObject`/`HideSubObject` lines of one or two names; 0-3 `TransitionState`s (two different keys, a model,
//! `ONCE` or `ONCE_BACKWARDS`, flags, speeds, 0-2 animations, show/hide lines). Ops (now from 112): 16
//! `setAnimationLoopDuration`, 17 `setAnimationCompletionTime` (3 each), 18 `showSubObject` (a name, lower-cased 1 in 2,
//! shown 1 in 2; 4), 19 `updateSubObjects` (2). Compared besides: the current and pending states, the pending duration,
//! the `showSubObject` list, and every TransitionState's caches. The wrapper's render object now keeps its sub-objects'
//! transforms as upstream's HLOD does (the dirty bit, `HLodClass::Update_Sub_Object_Transforms`, the sub-objects'
//! `Validate_Transform`); the test's asset source poses its skeleton as upstream's render object does (animobj.h's `Anim_Update`, the raw
//! fast path); `W3dFiles::skeleton` still takes the virtual update, pending the owner (#181 item 13).
//!
//! Predictions (`issue181i7c_predictions.md`, before any run): 0 mismatches for transitions and lists; the live
//! sub-object matrices equal to the posed skeleton's in nearly every case (0-50 of 20,000), the port changed to read live
//! anyway; plants 9996-10015 caught, the findSingleSubObj one perhaps not.
//!
//! Results (2026-11-14):
//!
//! - **First build:** one wrapper error (`isAnimationComplete` used before its definition: declared). **First run:**
//!   SIGSEGV, the wrapper's `WEAPON_NAMES` not extended (fixed). Then 300 cases: 0 mismatches.
//! - **Gated run** (20,000; the port already reading the live sub-objects): 2,350 cases ended where upstream is undefined
//!   (1,570 at a `setModelState`, 780 at a fire's barrel), 559,091 steps, 110,682 draws (77,069 animated), 12,869 steps
//!   in a TransitionState, 37,183 with a pending state, 239,749 with a `showSubObject` list, 11,005 pristine bones found as
//!   sub-objects: **0 mismatches**, max ULP 0.
//! - **The sub-object question, measured:** plant 10013 (the port's old read of the posed skeleton) differs in 299 of
//!   20,000 cases. Of those, 298 are the live hierarchy not evaluated (upstream's sub-object `Get_Transform` evaluates it;
//!   the live pivots then differ) and 1 is the matrix itself, 2 ULP in one entry: the old `W3dFiles::skeleton` posed by
//!   the virtual `Anim_Update`, upstream's render object by animobj.h's fast path. **Both are port defects.** The first is fixed
//!   (`find_single_sub_obj` reads the live render object). The second is not: `W3dFiles::skeleton` feeds the logic's
//!   art bones (#175), so its switch to the fast path waits for the owner (#181 item 13). **A prediction
//!   miss:** the matrices agreed as predicted (1 case), but the hierarchy's evaluation was not foreseen.
//! - **Plants 9996-10015**, mismatching cases of 20,000: 9996 1,797; 9997 1,290; 9998 1,692; 9999 1,290; 10000 408; 10001
//!   5; 10002 46; 10003 256; 10004 257; 10005 8,836; 10006 302; 10007 2,462; 10008 851; 10009 526; 10010 277; 10011 1,138;
//!   10012 4,085; 10013 286; 10014 659; 10015 1,478. **20 of 20 caught.** 10010 first caught 0 (every `showSubObject` of a
//!   name had one spelling; the op now lower-cases 1 in 2). Weak: 10001 (5) and 10002 (46).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints `MEASURED` and fails on purpose; with `MODEL_DRAW_ANIM_DEBUG` set it also prints the
//! first differing cases):
//!
//! | id | planted bug |
//! |---|---|
//! | 9956 | `adjustAnimation` does not draw the speed factor (the multiplier stays 1) |
//! | 9957 | `adjustAnimation` never draws which animation (always the first) |
//! | 9958 | `adjustAnimation` ignores `RANDOMSTART` |
//! | 9959 | `adjustAnimation` starts the backwards modes at frame 0 |
//! | 9960 | `adjustAnimation`'s maintained frame is `fraction * frames` (no `- 1`) |
//! | 9961 | `setPauseAnimation` does not keep the mode to resume in |
//! | 9962 | `setPauseAnimation` resumes in `LOOP` |
//! | 9963 | `nukeCurrentRender` does not clear `m_pauseAnimation` |
//! | 9964 | `doDrawModule` skips the completed-animation branch |
//! | 9965 | the idle switch passes no previous state (the same idle animation may come again) |
//! | 9966 | `doDrawModule` moves the bone particle systems before making them |
//! | 9967 | `adjustAnimSpeedToMovementSpeed` leaves out `MSEC_PER_LOGICFRAME_REAL` |
//! | 9968 | `setCurAnimDurationInMsec`'s multiplier inverted |
//! | 9969 | `Render` moves a `MANUAL` animation on |
//! | 9970 | `Single_Anim_Progress` does not keep the sync time (`LastSyncTime`) |
//! | 9971 | `updateBonesForClientParticleSystems` reads the pose without writing (no progress, no validation) |
//! | 9972 | `Set_Animation`'s `PrevFrame` is the new frame |
//! | 9973 | `adjustAnimation` draws the speed factor between max and min |
//! | 9974 | `getCurrentAnimFraction` divides by the frame count (no `- 1`) |
//! | 9975 | the completed branch restarts a non-idle animation without `RESTART_ANIM_WHEN_COMPLETE` |
//! | 9976 | `validateCachedBones` reads the live bones without writing (no evaluation of the live hierarchy) |
//! | 9977 | `validateCachedBones` does not set the pose's animation in `MANUAL` |
//! | 9978 | `validateCachedBones` does not set the scale transform before reading |
//! | 9979 | `validateTurretInfo` finds the pitch bone by the angle key |
//! | 9980 | `turretNamesDiffer` always true (the render object always made again) |
//! | 9981 | `handleClientTurretPositioning` leaves out `TurretArtAngle` |
//! | 9982 | `handleClientTurretPositioning` pitches by `+pitch` |
//! | 9983 | `handleClientRecoil`'s muzzle flash hidden inverted |
//! | 9984 | `handleClientRecoil` does not damp the rate |
//! | 9985 | `handleClientRecoil`'s settle never reaches idle |
//! | 9986 | `handleClientRecoil` ignores `MaxRecoilDistance` |
//! | 9987 | `rebuildWeaponRecoilInfo` keeps the old recoils |
//! | 9988 | `setModelState` does not hide the muzzle flashes |
//! | 9989 | `handleClientRecoil` ignores `TINY_RECOIL` |
//! | 9990 | `handleWeaponFireFX` always fires barrel 0 |
//! | 9991 | `handleWeaponFireFX` does not ask the bone of a hidden render object without an object |
//! | 9992 | `handleWeaponFireFX` starts the recoil in `RECOIL`, not `RECOIL_START` |
//! | 9993 | `handleWeaponFireFX` does not show the muzzle flash |
//! | 9994 | `validateWeaponBarrelInfo` drops the shared-FX-bone special case |
//! | 9995 | `setModelState` does not call `hideAllHeadlights` |
//! | 9996 | `setModelState` never lets a state wait for the current one to finish (allow-to-finish) |
//! | 9997 | `setModelState` does not look up a TransitionState |
//! | 9998 | `findTransitionForSig` swaps the two keys |
//! | 9999 | `setModelState` does not keep the requested state pending after a TransitionState |
//! | 10000 | `doDrawModule` does not switch to the pending state when the animation completes |
//! | 10001 | `doDrawModule` does not restore the pending duration |
//! | 10002 | `setAnimationLoopDuration` keeps the pending duration |
//! | 10003 | `setAnimationCompletionTime` never splits the duration over a TransitionState |
//! | 10004 | `setAnimationCompletionTime`'s pending duration is the whole duration |
//! | 10005 | `setAnimationLoopDuration` floors the milliseconds (`ceilf` upstream) |
//! | 10006 | `setModelState`'s early return ignores a request for the pending state |
//! | 10007 | `doHideShowSubObjs` shows every listed sub-object (the hide flag ignored) |
//! | 10008 | `doHideShowSubObjs` leaves the sub-objects on child bones alone (`doHideShowBoneSubObjs` skipped) |
//! | 10009 | `doHideShowSubObjs` does not apply the `showSubObject` list (`updateSubObjects`) |
//! | 10010 | `showSubObject` compares names with case |
//! | 10011 | `showSubObject` always adds (a listed name not updated) |
//! | 10012 | `Get_Sub_Object_By_Name` without the suffix match (after the first `.`) |
//! | 10013 | `findSingleSubObj` reads the asset source's posed skeleton (the port's old read: no live evaluation) |
//! | 10014 | `findSingleSubObj`'s numbered names (`gun01`..) not looked for |
//! | 10015 | `doHideShowBoneSubObjs` counts a sub-object on the bone itself as its child |

mod support;

use std::sync::Arc;

use rts_lockstep_engine::client::particle_sys::ParticleSystemManager;
use rts_lockstep_engine::client::w3d_model_draw::{
    anim_mode_index, is_animation_complete, is_any_maintain_frame_flag_set, scale_render_obj_hierarchy, AnimData, AnimHandle, DrawEnv, DrawFacts, DrawParticles, Skeleton, SkeletonSubObject, StateRef, W3DModelDraw,
    W3DModelDrawModuleData, W3dAssetSource, THE_ANIM_MODE_NAMES,
};
use rts_lockstep_engine::common::model_state::{ModelConditionFlags, MODEL_CONDITION_NAMES};
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::thing_template::{ClientLine, ClientModuleData};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::w3d::assetmgr::HAnim;
use rts_lockstep_engine::formats::w3d::hanim::HRawAnim;
use rts_lockstep_engine::formats::w3d::htree::{self, HTree};
use rts_lockstep_engine::formats::w3d::render::get_frame_rate;
use rts_lockstep_engine::formats::w3d::w3d_file::*;
use rts_lockstep_engine::ww::chunkio::{ChunkLoad, GameFile};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use support::{Check, Gen};

/// The wrapper's records are one global buffer: one test at a time.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases() -> u64 {
    std::env::var("MODEL_DRAW_ANIM_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000)
}

fn b(f: f32) -> i32 {
    f.to_bits() as i32
}

fn r(i: i32) -> f32 {
    f32::from_bits(i as u32)
}

/// The wrapper's `BONE_NAMES` (the `ParticleSysBone` lines').
const BONE_NAMES: [&str; 7] = ["B1", "b2", "B3", "b4", "B5", "RootTransform", "nosuchbone"];
const MODEL_NAMES: [&str; 4] = ["M0", "M1", "M2", "None"];
const TEMPLATE_NAMES: [&str; 6] = ["PS0", "PS1", "PS2", "PS3", "None", "NoSuch"];
/// The states' condition bits (the first state is the default).
const STATE_BITS: [&str; 4] = ["", "DAMAGED", "RUBBLE", "GARRISONED"];
/// `ACBits` the cases set (not `ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT`, 3: no object is under construction here).
const FLAG_BITS: [u32; 9] = [0, 1, 2, 4, 5, 6, 7, 8, 9];
/// `ACBitsNames`.
const AC_BITS_NAMES: [&str; 10] = [
    "RANDOMSTART",
    "START_FRAME_FIRST",
    "START_FRAME_LAST",
    "ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT",
    "PRISTINE_BONE_POS_IN_FINAL_FRAME",
    "MAINTAIN_FRAME_ACROSS_STATES",
    "RESTART_ANIM_WHEN_COMPLETE",
    "MAINTAIN_FRAME_ACROSS_STATES2",
    "MAINTAIN_FRAME_ACROSS_STATES3",
    "MAINTAIN_FRAME_ACROSS_STATES4",
];
/// Speed factors and distances that print and parse back exactly.
const SPEEDS: [f32; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];
const DISTANCES: [f32; 4] = [0.0, 5.0, 12.5, 30.0];
/// The wrapper's index for an animation that is not in the file.
const MISSING_ANIM: i32 = 99;

/// The #181 item 7 header, then (#181 item 7b) the recoil numbers, the sub-objects, the standard and extra public
/// bones, and each state's turrets and weapon bones.
const HEADER_7: usize = 6 + 4 + 1 + 4 * (5 + 1 + 3 * 3 + 1 + 3 * 2);
const HEADER_7B: usize = HEADER_7 + 4 + 1 + 4 * 3 + 3 + 3 + 4 * (1 + MAX_TURRETS * 4 + 3 * 4);
/// (#181 item 7c) Then the sub-objects' name codes, each state's transition keys and show/hide lines, and the
/// TransitionStates.
const HEADER: usize = HEADER_7B + 4 + 4 * (2 + HS_WORDS) + 1 + 3 * (8 + 2 * 2 + HS_WORDS);
/// A state's ShowSubObject/HideSubObject lines: a count (0..2), then 2 x (hide, name, name or `NO_NAME`).
const HS_WORDS: usize = 1 + 2 * 3;
const STEP: usize = 14;
const MAX_TURRETS: usize = 2;
/// (#181 item 7b) The wrapper's `WEAPON_NAMES`: turret, weapon and public bone names (`NO_LINE`: the line not written).
/// (#181 item 7c) `GUN` and `DOOR` added (sub-objects' names, no pivot's); ~~`NO_LINE` 9~~ now 11.
const WEAPON_NAMES: [&str; 11] = ["TURRET", "PITCH", "BARREL", "MUZZLE", "FX", "B1", "B3", "NOSUCH", "None", "GUN", "DOOR"];
const NO_LINE: i32 = 11;
/// (#181 item 7c) The wrapper's `SUB_NAMES`: a sub-object's name after `SKEL.` by its name code (0: `SUBk` or
/// `HEADLIGHTk`, as before).
const SUB_NAMES: [&str; 5] = ["", "GUN", "GUN01", "GUN02", "DOOR"];
/// (#181 item 7c) The wrapper's `HS_NAMES`: the names of ShowSubObject/HideSubObject lines and of `showSubObject`
/// (`NO_NAME`: no second token, or an empty name).
const HS_NAMES: [&str; 10] = ["sub0", "SUB1", "skel.sub2", "HEADLIGHT3", "gun", "GUN01", "door", "nosuch", "None", ""];
const NO_NAME: i32 = 9;
/// (#181 item 7c) The wrapper's `KEY_NAMES`: TransitionKey, WaitForStateToFinishIfPossible and TransitionState keys.
const KEY_NAMES: [&str; 3] = ["k0", "k1", "k2"];
/// The pivots' names other than `B<i>`: turrets, barrels, muzzle flashes and FX bones, numbered or not.
const PIVOT_NAMES: [&str; 12] = ["TURRET", "PITCH", "BARREL", "BARREL01", "BARREL02", "MUZZLE", "MUZZLE01", "MUZZLE02", "FX", "FX01", "FX02", "B3"];
const SLOT_NAMES: [&str; 3] = ["PRIMARY", "SECONDARY", "TERTIARY"];
const WEAPON_FIELDS: [&str; 4] = ["WeaponFireFXBone", "WeaponRecoilBone", "WeaponMuzzleFlash", "WeaponLaunchBone"];
const TURRET_FIELDS: [[&str; 4]; MAX_TURRETS] =
    [["Turret", "TurretArtAngle", "TurretPitch", "TurretArtPitch"], ["AltTurret", "AltTurretArtAngle", "AltTurretPitch", "AltTurretArtPitch"]];
/// Art angles in degrees, and the recoil numbers, that print and parse back exactly.
const ART_DEGREES: [f32; 6] = [0.0, 15.0, -30.0, 45.0, 90.0, 180.0];
const RECOIL_SPEEDS: [f32; 5] = [0.0, 15.0, 30.0, 60.0, 90.0];
const MAX_RECOILS: [f32; 4] = [0.5, 1.0, 2.0, 3.0];
const DAMPINGS: [f32; 5] = [0.25, 0.5, 0.75, 0.9, 1.0];
const SETTLES: [f32; 4] = [3.0, 6.0, 15.0, 30.0];

// ---- the W3D file ----

fn w32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn wf(v: &mut Vec<u8>, x: f32) {
    w32(v, x.to_bits());
}

fn hchunk(id: u32, body: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(body.len() + 8);
    w32(&mut v, id);
    w32(&mut v, body.len() as u32);
    v.extend_from_slice(body);
    v
}

fn name16(s: &[u8]) -> [u8; W3D_NAME_LEN] {
    let mut n = [0u8; W3D_NAME_LEN];
    n[..s.len()].copy_from_slice(s);
    n
}

/// A unit quaternion (normalised in `f32`).
fn quat(g: &mut Gen) -> [f32; 4] {
    let mut q = [0.0f32; 4];
    for v in q.iter_mut() {
        *v = g.uniform(-1.0, 1.0);
    }
    let l = petir::Real::sqrt(q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]);
    if l < 1e-3 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    q.map(|v| v / l)
}

/// The file, its animations' frame counts and (#181 item 7b) its pivots' names.
fn gen_file(g: &mut Gen) -> (Vec<u8>, Vec<u32>, Vec<String>) {
    let npiv = 2 + g.below(7);
    let names: Vec<String> = (0..npiv).map(|i| if i == 0 || g.below(2) == 0 { format!("B{i}") } else { PIVOT_NAMES[g.below(12) as usize].to_string() }).collect();
    let mut f = Vec::new();
    // W3D_CHUNK_HIERARCHY: the header (version 4.1, name, pivot count, center) and the pivots
    let mut h = Vec::new();
    w32(&mut h, (4 << 16) | 1);
    h.extend_from_slice(&name16(b"SKEL"));
    w32(&mut h, npiv);
    for _ in 0..3 {
        wf(&mut h, 0.0);
    }
    let mut body = hchunk(W3D_CHUNK_HIERARCHY_HEADER, &h);
    let mut pb = Vec::new();
    for i in 0..npiv {
        pb.extend_from_slice(&name16(names[i as usize].as_bytes()));
        w32(&mut pb, if i == 0 { 0xFFFF_FFFF } else { g.below(i) });
        for _ in 0..3 {
            wf(&mut pb, g.uniform(-2.0, 2.0));
        }
        for _ in 0..3 {
            wf(&mut pb, 0.0);
        }
        for v in quat(g) {
            wf(&mut pb, v);
        }
    }
    body.extend(hchunk(W3D_CHUNK_PIVOTS, &pb));
    f.extend(hchunk(W3D_CHUNK_HIERARCHY, &body));
    let mut frames = Vec::new();
    for a in 0..1 + g.below(4) {
        let nf = if g.below(10) == 0 { 1 } else { 2 + g.below(39) };
        frames.push(nf);
        let mut h = Vec::new();
        w32(&mut h, (4 << 16) | 1);
        h.extend_from_slice(&name16(format!("A{a}").as_bytes()));
        h.extend_from_slice(&name16(b"SKEL"));
        w32(&mut h, nf);
        w32(&mut h, match g.below(8) {
            0 => 15,
            1 => 60,
            _ => 30,
        });
        let mut body = hchunk(W3D_CHUNK_ANIMATION_HEADER, &h);
        for pivot in 0..npiv {
            for ty in [ANIM_CHANNEL_X, ANIM_CHANNEL_Y, ANIM_CHANNEL_Z, ANIM_CHANNEL_Q] {
                if g.below(2) != 0 {
                    continue;
                }
                let first = g.below(3).min(nf - 1) as u16;
                let last = first + g.below(nf - u32::from(first)) as u16;
                let vlen: u16 = if ty == ANIM_CHANNEL_Q { 4 } else { 1 };
                let mut c = Vec::new();
                for v in [first, last, vlen, ty as u16, pivot as u16, 0] {
                    c.extend_from_slice(&v.to_le_bytes());
                }
                for _ in first..=last {
                    if ty == ANIM_CHANNEL_Q {
                        for v in quat(g) {
                            wf(&mut c, v);
                        }
                    } else {
                        wf(&mut c, g.uniform(-3.0, 3.0));
                    }
                }
                body.extend(hchunk(W3D_CHUNK_ANIMATION_CHANNEL, &c));
            }
        }
        f.extend(hchunk(W3D_CHUNK_ANIMATION, &body));
    }
    (f, frames, names)
}

/// The port's load of the file: the hierarchy and the raw animations (the port's default `Float_To_Long`, Win32's).
fn rust_load(data: &[u8]) -> (HTree, Vec<HAnim>) {
    let mut c = ChunkLoad::new(GameFile::new(data).expect("a file"));
    let mut tree: Option<HTree> = None;
    let mut anims = Vec::new();
    while c.open_chunk().expect("a chunk") {
        match c.cur_chunk_id().expect("an id") {
            W3D_CHUNK_HIERARCHY => tree = htree::load_w3d(&mut c).expect("loads"),
            W3D_CHUNK_ANIMATION => {
                let t = tree.as_ref().map(|t| (t.name, t.num_pivots));
                let mut lookup = |n: &[u8; W3D_NAME_LEN]| t.filter(|(tn, _)| tn == n).map(|(_, p)| p);
                anims.push(HAnim::Raw(HRawAnim::load_w3d(&mut c, &mut lookup).expect("loads").expect("an animation")));
            }
            _ => panic!("only hierarchies and raw animations are made"),
        }
        c.close_chunk().expect("closes");
    }
    (tree.expect("a hierarchy"), anims)
}

// ---- the case ----

fn matrix(w: &[i32]) -> Matrix3D {
    let mut m = Matrix3D::IDENTITY;
    for row in 0..3 {
        for c in 0..4 {
            m.row[row][c] = r(w[row * 4 + c]);
        }
    }
    m
}

fn matrix_words(m: &Matrix3D) -> [i32; 12] {
    let mut w = [0i32; 12];
    for row in 0..3 {
        for c in 0..4 {
            w[row * 4 + c] = b(m.row[row][c]);
        }
    }
    w
}

/// A world transform: a turn about Z and a translation, or (1 in 4) any entries in -2..2 with a translation.
fn world(g: &mut Gen) -> [i32; 12] {
    let mut m = Matrix3D::IDENTITY;
    if g.below(4) == 0 {
        for row in 0..3 {
            for c in 0..3 {
                m.row[row][c] = g.uniform(-2.0, 2.0);
            }
        }
    } else {
        m.rotate_z(g.uniform(-4.0, 4.0));
    }
    m.row[0][3] = g.uniform(-500.0, 500.0);
    m.row[1][3] = g.uniform(-500.0, 500.0);
    m.row[2][3] = g.uniform(-20.0, 20.0);
    matrix_words(&m)
}

/// One case: the file and the wrapper's words.
fn random_case(g: &mut Gen) -> (Vec<u8>, Vec<i32>) {
    let (file, frames, pivots) = gen_file(g);
    let nanims = frames.len() as u32;
    let mut w = Vec::new();
    w.push(g.u32() as i32);
    w.push(if g.below(20) == 0 { u32::MAX - g.below(5000) } else { g.below(100_000) } as i32);
    w.push(1 + g.below(1000) as i32);
    w.push(b(match g.below(8) {
        0..=2 => 1.0,
        3 => 0.5,
        4 => 2.0,
        5 => [1.005f32, 0.99][g.below(2) as usize],
        _ => g.uniform(0.5, 2.0),
    }));
    w.push(i32::from(g.below(10) < 7));
    w.push(i32::from(g.below(2) == 0));
    for t in 0..4u32 {
        w.push(if t < 3 && g.below(3) == 0 { (t + 1 + g.below(3 - t)) as i32 } else { -1 });
    }
    w.push(1 + g.below(4) as i32);
    // parseConditionState's rules (W3DModelDraw.cpp, and the port's parse): a state's animations are all idle or all
    // not, and idle animations need `ONCE` or `ONCE_BACKWARDS`; a state without its own animations has the default
    // state's (and its idle flag)
    let mut default_idle = false;
    for si in 0..4 {
        w.push(if g.below(8) == 0 { 3 } else { g.below(3) as i32 });
        let na = if g.below(5) == 0 { 0 } else { 1 + g.below(3) as i32 };
        let idle = g.below(3) == 0;
        let effective_idle = if na > 0 { idle } else { si > 0 && default_idle };
        if si == 0 {
            default_idle = na > 0 && idle;
        }
        w.push(if effective_idle { [2, 5][g.below(2) as usize] } else { g.below(6) as i32 });
        let mut flags = 0i32;
        for bit in FLAG_BITS {
            if g.below(6) == 0 {
                flags |= 1 << bit;
            }
        }
        w.push(flags);
        w.push(b(SPEEDS[g.below(6) as usize]));
        w.push(b(SPEEDS[g.below(6) as usize]));
        w.push(na);
        for _ in 0..3 {
            w.push(if g.below(12) == 0 { MISSING_ANIM } else { g.below(nanims) as i32 });
            w.push(i32::from(idle));
            w.push(b(DISTANCES[g.below(4) as usize]));
        }
        w.push(g.below(4) as i32);
        for _ in 0..3 {
            w.push(g.below(7) as i32);
            w.push(g.below(6) as i32);
        }
    }
    assert_eq!(w.len(), HEADER_7);
    // (#181 item 7b) InitialRecoilSpeed, MaxRecoilDistance, RecoilDamping, RecoilSettleSpeed (as written)
    w.push(b(RECOIL_SPEEDS[g.below(5) as usize]));
    w.push(b(MAX_RECOILS[g.below(4) as usize]));
    w.push(b(DAMPINGS[g.below(5) as usize]));
    w.push(b(SETTLES[g.below(4) as usize]));
    // the sub-objects: on a muzzle-flash pivot 2 in 3 when there is one
    let muzzles: Vec<u32> = (0..pivots.len() as u32).filter(|&i| pivots[i as usize].starts_with("MUZZLE")).collect();
    w.push(g.below(5) as i32);
    for _ in 0..4 {
        w.push(if !muzzles.is_empty() && g.below(3) != 0 { muzzles[g.below(muzzles.len() as u32) as usize] } else { g.below(pivots.len() as u32) } as i32);
        w.push(i32::from(g.below(3) == 0));
        w.push(i32::from(g.below(5) == 0));
    }
    // the standard public bones, the ExtraPublicBone names
    for _ in 0..2 {
        w.push(g.below(3) as i32);
        for _ in 0..2 {
            w.push(g.below(11) as i32);
        }
    }
    // each state: its own turrets (the default state always), 2 x (angle bone, pitch bone, art angle, art pitch); its
    // weapon bones, 3 slots x (FX, recoil, muzzle flash, launch), a name or no line
    for _ in 0..4 {
        w.push(i32::from(g.below(2) == 0));
        for _ in 0..MAX_TURRETS {
            w.push(if g.below(3) == 0 { g.below(11) as i32 } else { [0, 1, 8][g.below(3) as usize] });
            w.push(if g.below(3) == 0 { g.below(11) as i32 } else { [0, 1, 8][g.below(3) as usize] });
            w.push(b(ART_DEGREES[g.below(6) as usize]));
            w.push(b(ART_DEGREES[g.below(6) as usize]));
        }
        for _ in 0..3 {
            for f in 0..4 {
                w.push(match g.below(4) {
                    0 | 1 => NO_LINE,
                    _ if g.below(4) == 0 => g.below(11) as i32,
                    _ => [4, 2, 3, 2][f],
                });
            }
        }
    }
    assert_eq!(w.len(), HEADER_7B);
    // (#181 item 7c) the sub-objects' name codes (0 1 in 2)
    for _ in 0..4 {
        w.push(if g.below(2) == 0 { 0 } else { 1 + g.below(4) as i32 });
    }
    let hide_show = |g: &mut Gen, w: &mut Vec<i32>| {
        w.push(g.below(3) as i32);
        for _ in 0..2 {
            w.push(g.below(2) as i32);
            w.push(g.below(9) as i32);
            w.push(if g.below(2) == 0 { NO_NAME } else { g.below(9) as i32 });
        }
    };
    // each state's TransitionKey (no line 1 in 4), WaitForStateToFinishIfPossible (a line 1 in 3), show/hide lines
    for _ in 0..4 {
        w.push(if g.below(4) == 0 { -1 } else { g.below(3) as i32 });
        w.push(if g.below(3) == 0 { g.below(3) as i32 } else { -1 });
        hide_show(g, &mut w);
    }
    // the TransitionStates (0..3): two different keys, a model, `ONCE` or `ONCE_BACKWARDS` (parseConditionState's rule),
    // flags, speeds, 0..2 Animation lines (at least one when the default state's are idle: a copied idle animation is
    // refused), show/hide lines
    w.push(g.below(4) as i32);
    for _ in 0..3 {
        let src = g.below(3);
        w.push(src as i32);
        w.push(((src + 1 + g.below(2)) % 3) as i32);
        w.push(if g.below(8) == 0 { 3 } else { g.below(3) as i32 });
        w.push([2, 5][g.below(2) as usize]);
        let mut flags = 0i32;
        for bit in FLAG_BITS {
            if g.below(6) == 0 {
                flags |= 1 << bit;
            }
        }
        w.push(flags);
        w.push(b(SPEEDS[g.below(6) as usize]));
        w.push(b(SPEEDS[g.below(6) as usize]));
        let na = g.below(3);
        w.push(if default_idle && na == 0 { 1 } else { na as i32 });
        for _ in 0..2 {
            w.push(if g.below(12) == 0 { MISSING_ANIM } else { g.below(nanims) as i32 });
            w.push(b(DISTANCES[g.below(4) as usize]));
        }
        hide_show(g, &mut w);
    }
    assert_eq!(w.len(), HEADER);
    for _ in 0..1 + g.below(60) {
        let mut s = [0i32; STEP];
        // (#181 item 7c) ops 16-19 from 100 of 112
        let op = match g.below(112) {
            0..=3 => 0,
            4..=9 => 13,
            10..=31 => 1,
            32..=47 => 2,
            48..=59 => 3,
            60..=61 => 4,
            62..=64 => 5,
            65..=67 => 6,
            68..=70 => 7,
            71..=73 => 8,
            74 => 9,
            75..=76 => 10,
            77..=78 => 11,
            79..=82 => 12,
            83..=95 => 14,
            96..=99 => 15,
            100..=102 => 16,
            103..=105 => 17,
            106..=109 => 18,
            _ => 19,
        };
        s[0] = op;
        s[1] = match op {
            2 => match g.below(10) {
                0 => 0,
                1..=4 => 33,
                5..=7 => g.below(100) as i32,
                8 => 100 + g.below(3000) as i32,
                _ => g.below(100_000) as i32,
            },
            4 | 11 => i32::from(g.below(2) == 0),
            5 => i32::from(g.below(4) != 0),
            6 => i32::from(g.below(4) != 0),
            _ => g.below(1000) as i32,
        };
        match op {
            1 | 8 => s[2..].copy_from_slice(&world(g)),
            // (#181 item 7b) the fire: a barrel (-1, 0..3 or 5), an FX list 3 in 4
            14 => {
                s[2] = [-1, 0, 0, 1, 2, 3, 5][g.below(7) as usize];
                s[3] = i32::from(g.below(4) != 0);
            }
            // the AI 3 in 4, each turret 3 in 4, its angle and pitch
            15 => {
                s[2] = i32::from(g.below(4) != 0);
                for t in 0..MAX_TURRETS {
                    s[3 + 3 * t] = i32::from(g.below(4) != 0);
                    s[4 + 3 * t] = b(g.uniform(-4.0, 4.0));
                    s[5 + 3 * t] = b(g.uniform(-1.5, 1.5));
                }
            }
            6 => s[2] = b(if g.below(4) == 0 { 0.0 } else { g.uniform(0.0, 3.0) }),
            7 => {
                // a turret's turn about Z, a pitch about Y, a recoil's shift along -X
                let mut m = Matrix3D::IDENTITY;
                match g.below(3) {
                    0 => m.rotate_z(g.uniform(-4.0, 4.0)),
                    1 => m.rotate_y(g.uniform(-1.5, 1.5)),
                    _ => m.translate_x(-g.uniform(0.0, 3.0)),
                }
                s[2..].copy_from_slice(&matrix_words(&m));
            }
            10 => s[2] = b([1.0f32, 0.5, 2.0, 1.005][g.below(4) as usize]),
            // (#181 item 7c) showSubObject: shown 1 in 2, the name lower-cased 1 in 2
            18 => {
                s[2] = g.below(2) as i32;
                s[3] = g.below(2) as i32;
            }
            _ => {}
        }
        w.extend(s);
    }
    (file, w)
}

/// The test's asset manager: `M0`..`M2` (HLODs on the file's hierarchy) and the file's animations (each one shared
/// object, as upstream's reference-counted `HAnimClass`).
struct Assets {
    tree: HTree,
    anims: Vec<Arc<AnimData>>,
    /// (#181 item 7b) The models' sub-objects (bone, hidden as made, a headlight), the wrapper's `g_subs`; (#181 item 7c)
    /// and the name code (`SUB_NAMES`).
    subs: Vec<(i32, bool, bool, i32)>,
}

impl Assets {
    fn anim_index(&self, name: &[u8]) -> Option<usize> {
        self.anims.iter().position(|a| a.motion.name().eq_ignore_ascii_case(name))
    }
}

impl W3dAssetSource for Assets {
    fn create_render_obj(&mut self, name: &[u8]) -> Option<bool> {
        MODEL_NAMES[..3].iter().any(|n| n.as_bytes().eq_ignore_ascii_case(name)).then_some(true)
    }
    fn get_hanim(&mut self, name: &[u8]) -> Option<AnimHandle> {
        let i = self.anim_index(name)?;
        let a = &self.anims[i];
        Some(AnimHandle { name: a.motion.name().to_vec(), num_frames: a.motion.get_num_frames(), frame_rate: get_frame_rate(&a.motion), data: Some(a.clone()) })
    }
    /// The render object's hierarchy as `Create_Render_Obj(name, scale, color)` makes it (`scale_render_obj_hierarchy`),
    /// posed by `Base_Update` under `Matrix3D(true).Scale(scale)`. ~~(#181 item 7b) The pose asked is not applied: the
    /// port's `validateCachedBones` reads the bones of the live render object it is given (the only kind this test
    /// makes); only the sub-objects are read from here, and no requested name is a sub-object's.~~ **Corrected
    /// 2026-11-14 (#181 item 7c):** posed as upstream's render object made for the purpose poses it: the asked
    /// animation's `HTree::anim_update_animobj` at the asked frame (animobj.h's `Anim_Update`; `W3dFiles::skeleton`
    /// still takes the virtual `HTree::anim_update`, pending the owner, #181 item 13; with that path here, 1 case of 20,000 differed by 2 ULP under plant
    /// 10013), else `Base_Update`; each sub-object's transform its bone's (`Skeleton::of`). Requested names are now sub-objects' too; the port reads the live render object's (plant 10013
    /// reads these). The sub-objects are the wrapper's: `SKEL.SUBk` or `SKEL.HEADLIGHTk`, or (#181 item 7c) `SKEL.` and a
    /// `SUB_NAMES` entry, on a bone (modulo the pivot count), hidden as made or not.
    fn skeleton(&mut self, name: &[u8], pose: Option<(&[u8], f32)>, scale: f32) -> Option<Skeleton> {
        self.create_render_obj(name)?;
        let mut tree = self.tree.clone();
        scale_render_obj_hierarchy(&mut tree, scale);
        let mut root = Matrix3D::IDENTITY;
        root.scale(scale);
        match pose.and_then(|(a, frame)| self.anim_index(a).map(|i| (i, frame))) {
            Some((i, frame)) => {
                let a = self.anims[i].clone();
                tree.anim_update_animobj(&root, &a.motion, &*a, frame).expect("parents come first");
            }
            None => tree.base_update(&root).expect("parents come first"),
        }
        let n = tree.num_pivots;
        let sub_objects = self
            .subs
            .iter()
            .enumerate()
            .map(|(k, &(bone, hidden, headlight, code))| SkeletonSubObject {
                name: if code > 0 { format!("SKEL.{}", SUB_NAMES[code as usize]) } else { format!("SKEL.{}{k}", if headlight { "HEADLIGHT" } else { "SUB" }) }.into_bytes(),
                bone_index: bone % n,
                hidden,
                transform: tree.get_transform(bone % n).expect("a pivot"),
                linear_offset_materials: 0,
            })
            .collect();
        Some(Skeleton { bones: Skeleton::bones_of(&tree), sub_objects, scale_factor: tree.scale_factor })
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

fn push_matrix(out: &mut Vec<i32>, m: &Matrix3D) {
    out.extend(matrix_words(m));
}

/// The wrapper's `state()`.
fn state_words(out: &mut Vec<i32>, m: &W3DModelDraw, mgr: &ParticleSystemManager, states: &[usize], transitions: &[(usize, usize)], assets: &Assets, random: &RandomValues) {
    let si = match m.cur_state_ref() {
        Some(StateRef::Normal(i)) => states.iter().position(|s| s == i).map_or(-1, |p| p as i32),
        _ => -1,
    };
    out.extend([
        100,
        m.need_recalc_bone_particle_systems as i32,
        m.fully_obscured_by_shroud as i32,
        m.which_anim_in_cur_state(),
        m.pause_animation() as i32,
        anim_mode_index(m.animation_mode()),
        si,
        m.particle_system_ids.len() as i32,
    ]);
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
        push_matrix(out, &s.local_transform);
    }
    match &m.render_object {
        None => out.push(0),
        Some(ro) => {
            out.extend([1, if ro.anim.is_some() { 2 } else { 1 }, ro.is_hierarchy_valid() as i32]);
            match &ro.anim {
                Some(a) => out.extend([
                    assets.anim_index(&a.motion.name).map_or(-1, |i| i as i32),
                    b(a.frame),
                    b(a.prev_frame),
                    a.last_sync_time as i32,
                    b(a.anim_direction),
                    b(a.frame_rate_multiplier),
                    anim_mode_index(a.anim_mode),
                    ro.is_animation_complete_hlod() as i32,
                ]),
                None => out.extend([-1, 0, 0, 0, 0, 0, 0, 0]),
            }
            out.push(ro.hidden as i32);
            push_matrix(out, &ro.transform);
            let pivots = ro.live_pivot_transforms();
            out.extend([b(ro.object_scale), pivots.len() as i32]);
            for p in &pivots {
                push_matrix(out, p);
            }
            // (#181 item 7b) each sub-object's `Is_Hidden`
            out.push(ro.sub_object_hidden.len() as i32);
            out.extend(ro.sub_object_hidden.iter().map(|&h| h as i32));
        }
    }
    // (#181 item 7b) the recoils, then each state's caches (the wrapper's)
    for v in &m.weapon_recoil_info_vec {
        out.push(v.len() as i32);
        for r in v {
            out.extend([r.state as i32, b(r.shift), b(r.recoil_rate)]);
        }
    }
    for &i in states {
        caches_words(out, m.data.state(&StateRef::Normal(i)));
    }
    // (#181 item 7c) the current and the pending state (`state_code`), `m_nextStateAnimLoopDuration`, the showSubObject
    // list, then each of the case's TransitionStates' caches
    let code = |s: Option<&StateRef>| match s {
        None => -1,
        Some(StateRef::Normal(i)) => states.iter().position(|s| s == i).map_or(-2, |p| p as i32),
        Some(StateRef::Transition((a, bb))) => {
            let key = |k: &[u8]| KEY_NAMES.iter().position(|n| n.as_bytes() == k).expect("a case key") as i32;
            10 + 3 * key(a) + key(bb)
        }
    };
    out.extend([code(m.cur_state_ref()), code(m.next_state_ref()), m.next_state_anim_loop_duration() as i32, m.sub_object_vec.len() as i32]);
    for it in &m.sub_object_vec {
        out.push(it.sub_obj_name.len() as i32);
        out.extend(it.sub_obj_name.iter().map(|&ch| i32::from(ch)));
        out.push(i32::from(it.hide));
    }
    for &(a, bb) in transitions {
        caches_words(out, m.data.state(&StateRef::Transition((KEY_NAMES[a].as_bytes().to_vec(), KEY_NAMES[bb].as_bytes().to_vec()))));
    }
    out.extend(random.client.map(|x| x as i32));
}

/// A state's caches (the wrapper's `caches`).
fn caches_words(out: &mut Vec<i32>, st: &rts_lockstep_engine::client::w3d_model_draw::ModelConditionInfo) {
    {
        out.push(i32::from(st.pristine_bones_valid()) | i32::from(st.turrets_valid()) << 1 | i32::from(st.barrels_valid()) << 3);
        let c = st.bone_cache();
        out.push(c.pristine_bones.len() as i32);
        for (name, info) in &c.pristine_bones {
            out.push(name.len() as i32);
            out.extend(name.iter().map(|&ch| i32::from(ch)));
            out.push(info.bone_index);
            push_matrix(out, &info.mtx);
        }
        for t in 0..MAX_TURRETS {
            out.extend([c.turret_bones[t].0, c.turret_bones[t].1]);
        }
        for w in 0..3 {
            out.push(c.weapon_barrel_info_vec[w].len() as i32);
            for bi in &c.weapon_barrel_info_vec[w] {
                out.extend([bi.recoil_bone, bi.fx_bone, bi.muzzle_flash_bone]);
                push_matrix(out, &bi.projectile_offset_mtx);
            }
            out.push(i32::from(c.has_recoil_bones_or_muzzle_flashes[w]));
        }
    }
}

thread_local! {
    /// Where each step's records end in the last Rust case (for the mismatch report).
    static STEP_ENDS: std::cell::RefCell<Vec<usize>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The last Rust case's module block (for the mismatch report).
    static BLOCK: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// The systems the last Rust case made (its manager's last id).
static SYSTEMS_MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// The cases ended at a `setModelState` that is undefined upstream (see `rust_case`).
static UNDEFINED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// (#181 item 7b) The cases ended at a fire whose barrel is the barrel count (undefined upstream), and the fires
/// handled (an FX list played at a bone or the logic object).
static UNDEFINED_BARREL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static FIRES_HANDLED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// (#181 item 7b) Draws whose state had valid turrets with a bone, and valid barrels with a recoil bone or muzzle flash.
static TURRET_DRAWS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static RECOIL_DRAWS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// (#181 item 7c) Steps after which the current state is a TransitionState, after which a state is pending
/// (`m_nextState`), after which the showSubObject list is not empty; and pristine bones found as sub-objects (a
/// `GUN`/`DOOR` name), counted where a state's bones become valid.
static TRANSITION_STEPS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static PENDING_STEPS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static SUB_LIST_STEPS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static SUB_OBJ_BONES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// The client random values the last Rust case drew (the generator's counter word moved on).
static RANDOM_DRAWN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// The last Rust case's draws with an animation playing (the render object in `SINGLE_ANIM` when `doDrawModule` ran).
static ANIMATED_DRAWS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The Rust's words for a case (the wrapper's records).
fn rust_case(file: &[u8], w: &[i32]) -> Vec<i32> {
    STEP_ENDS.with(|e| e.borrow_mut().clear());
    let (tree, anims) = rust_load(file);
    let mut assets = Assets { tree, anims: anims.into_iter().map(|a| Arc::new(AnimData { motion: Arc::new(a), poses: Vec::new(), embedded_sound_bone_index: Default::default() })).collect(), subs: Vec::new() };
    let mut k = 0;
    let mut next = || {
        k += 1;
        w[k - 1]
    };
    let seed = next() as u32;
    let mut now = next() as u32;
    let drawable_id = next();
    let mut scale = r(next());
    let attached = next() != 0;
    let require_power = next() != 0;
    let mut mgr = ParticleSystemManager::new();
    let mut slaves = [-1i32; 4];
    for s in &mut slaves {
        *s = next();
    }
    for (t, &sl) in slaves.iter().enumerate() {
        let tm = mgr.new_template(TEMPLATE_NAMES[t].as_bytes());
        tm.info.slave_system_name = if sl < 0 { Vec::new() } else { TEMPLATE_NAMES[sl as usize].as_bytes().to_vec() };
    }
    let nstates = next() as usize;
    let mut lines = vec![
        line(0, "ParticlesAttachedToAnimatedBones", if attached { "Yes" } else { "No" }),
        line(0, "AnimationsRequirePower", if require_power { "Yes" } else { "No" }),
    ];
    let names: Vec<String> = assets.anims.iter().map(|a| String::from_utf8(a.motion.name().to_vec()).expect("ASCII")).collect();
    let mut state_lines: Vec<Vec<ClientLine>> = Vec::new();
    for (si, bit) in STATE_BITS.iter().enumerate() {
        let mut lines = Vec::new();
        let model = next() as usize;
        let mode = next() as usize;
        let fl = next();
        let (lo, hi) = (r(next()), r(next()));
        let na = next() as usize;
        let mut anim_lines = Vec::new();
        for ai in 0..3 {
            let a = next();
            let idle = next() != 0;
            let dist = r(next());
            if ai < na {
                let name = if a == MISSING_ANIM { "SKEL.NOSUCH".to_string() } else { names[a as usize].clone() };
                anim_lines.push(line(1, if idle { "IdleAnimation" } else { "Animation" }, &format!("{name} {dist:?}")));
            }
        }
        let nb = next() as usize;
        let mut bones = Vec::new();
        for bi in 0..3 {
            let bone = next() as usize;
            let tmpl = next() as usize;
            if bi < nb {
                bones.push(format!("{} {}", BONE_NAMES[bone], TEMPLATE_NAMES[tmpl]));
            }
        }
        lines.push(if si == 0 { line(0, "DefaultConditionState", "") } else { line(0, "ConditionState", bit) });
        lines.push(line(1, "Model", MODEL_NAMES[model]));
        lines.push(line(1, "AnimationMode", THE_ANIM_MODE_NAMES[mode]));
        let set: Vec<&str> = (0..10).filter(|&i| fl & (1 << i) != 0).map(|i| AC_BITS_NAMES[i]).collect();
        lines.push(line(1, "Flags", &if set.is_empty() { "NONE".to_string() } else { set.join(" ") }));
        lines.push(line(1, "AnimationSpeedFactorRange", &format!("{lo:?} {hi:?}")));
        lines.extend(anim_lines);
        for bl in &bones {
            lines.push(line(1, "ParticleSysBone", bl));
        }
        state_lines.push(lines);
    }
    // (#181 item 7b) the recoil numbers, the sub-objects, the standard public bones, ExtraPublicBone, then each state's
    // turret and weapon bone lines (see the wrapper)
    for (field, v) in ["InitialRecoilSpeed", "MaxRecoilDistance", "RecoilDamping", "RecoilSettleSpeed"].iter().zip([r(next()), r(next()), r(next()), r(next())]) {
        lines.push(line(0, field, &format!("{v:?}")));
    }
    let nsubs = next() as usize;
    for k in 0..4 {
        let (bone, hidden, headlight) = (next(), next() != 0, next() != 0);
        if k < nsubs {
            assets.subs.push((bone, hidden, headlight, 0));
        }
    }
    let mut standard = Vec::new();
    let mut extra = Vec::new();
    for list in [&mut standard, &mut extra] {
        let n = next() as usize;
        for i in 0..2 {
            let c = next() as usize;
            if i < n {
                list.push(WEAPON_NAMES[c]);
            }
        }
    }
    if !extra.is_empty() {
        lines.push(line(0, "ExtraPublicBone", &extra.join(" ")));
    }
    for (si, sl) in state_lines.iter_mut().enumerate() {
        let own_turrets = next() != 0 || si == 0;
        for tf in &TURRET_FIELDS {
            let (ac, pc, aa, pa) = (next() as usize, next() as usize, r(next()), r(next()));
            if own_turrets {
                sl.push(line(1, tf[0], WEAPON_NAMES[ac]));
                sl.push(line(1, tf[1], &format!("{aa:?}")));
                sl.push(line(1, tf[2], WEAPON_NAMES[pc]));
                sl.push(line(1, tf[3], &format!("{pa:?}")));
            }
        }
        for slot in SLOT_NAMES {
            for field in WEAPON_FIELDS {
                let c = next();
                if c != NO_LINE {
                    sl.push(line(1, field, &format!("{slot} {}", WEAPON_NAMES[c as usize])));
                }
            }
        }
    }
    // (#181 item 7c) the sub-objects' name codes, each state's TransitionKey, WaitForStateToFinishIfPossible and
    // ShowSubObject/HideSubObject lines, then the TransitionStates (see the wrapper)
    for i in 0..4 {
        let code = next();
        if let Some(sub) = assets.subs.get_mut(i) {
            sub.3 = code;
        }
    }
    let hide_show_lines = |next: &mut dyn FnMut() -> i32, out: &mut Vec<ClientLine>| {
        let n = next();
        for i in 0..2 {
            let (hide, a, bb) = (next() != 0, next(), next());
            if i < n {
                let names = if bb == NO_NAME { HS_NAMES[a as usize].to_string() } else { format!("{} {}", HS_NAMES[a as usize], HS_NAMES[bb as usize]) };
                out.push(line(1, if hide { "HideSubObject" } else { "ShowSubObject" }, &names));
            }
        }
    };
    for sl in state_lines.iter_mut() {
        let (tk, allow) = (next(), next());
        if tk >= 0 {
            sl.push(line(1, "TransitionKey", KEY_NAMES[tk as usize]));
        }
        if allow >= 0 {
            sl.push(line(1, "WaitForStateToFinishIfPossible", KEY_NAMES[allow as usize]));
        }
        hide_show_lines(&mut next, sl);
    }
    let mut transitions = Vec::new();
    let mut transition_lines = Vec::new();
    let nt = next() as usize;
    for t in 0..3 {
        let (src, dst, model, mode, fl, lo, hi, na) = (next(), next(), next() as usize, next() as usize, next(), r(next()), r(next()), next() as usize);
        let mut tl = vec![
            line(0, "TransitionState", &format!("{} {}", KEY_NAMES[src as usize], KEY_NAMES[dst as usize])),
            line(1, "Model", MODEL_NAMES[model]),
            line(1, "AnimationMode", THE_ANIM_MODE_NAMES[mode]),
        ];
        let set: Vec<&str> = (0..10).filter(|&i| fl & (1 << i) != 0).map(|i| AC_BITS_NAMES[i]).collect();
        tl.push(line(1, "Flags", &if set.is_empty() { "NONE".to_string() } else { set.join(" ") }));
        tl.push(line(1, "AnimationSpeedFactorRange", &format!("{lo:?} {hi:?}")));
        for ai in 0..2 {
            let (a, dist) = (next(), r(next()));
            if ai < na {
                let name = if a == MISSING_ANIM { "SKEL.NOSUCH".to_string() } else { names[a as usize].clone() };
                tl.push(line(1, "Animation", &format!("{name} {dist:?}")));
            }
        }
        hide_show_lines(&mut next, &mut tl);
        if t < nt {
            transitions.push((src as usize, dst as usize));
            transition_lines.extend(tl);
        }
    }
    for sl in state_lines.into_iter().take(nstates) {
        lines.extend(sl);
    }
    lines.extend(transition_lines);
    let standard_public_bones: Arc<Vec<Vec<u8>>> = Arc::new(standard.iter().map(|n| n.as_bytes().to_vec()).collect());
    let block = ClientModuleData { module_tag_name_key: Default::default(), lines };
    BLOCK.with(|t| *t.borrow_mut() = String::from_utf8_lossy(&rts_lockstep_engine::client::w3d_model_draw::client_block_text(&block)).into_owned());
    let data = match W3DModelDrawModuleData::parse(b"W3DModelDraw", &block) {
        Ok(d) => Arc::new(d.expect("a model draw")),
        Err(e) => panic!("{e:?}: {}", String::from_utf8_lossy(&rts_lockstep_engine::client::w3d_model_draw::client_block_text(&block))),
    };
    let states: Vec<usize> = STATE_BITS[..nstates].iter().map(|bit| data.find_best_info(&flags(bit)).expect("a state")).collect();
    let mut random = RandomValues::new(seed);
    let counter0 = random.client[5];
    let mut animated_draws = 0u64;
    let mut out = Vec::new();
    let mut m = {
        let mut env = DrawEnv { assets: &mut assets, random: &mut random, sync_time: now, logic_time: false, particles: Some(DrawParticles { manager: &mut mgr, frame: 0 }) };
        W3DModelDraw::new_scaled(data.clone(), 0, false, false, scale, standard_public_bones, &mut env).expect("an idle state")
    };
    m.drawable_id = drawable_id;
    let (mut should_animate, mut has_object, mut speed) = (true, false, 0.0f32);
    // (#181 item 7b) the object's AI and its turrets; the drawable's transform (the logic object's, see the wrapper)
    let mut has_ai = false;
    let mut ai_turrets: [Option<(f32, f32)>; MAX_TURRETS] = [None, None];
    let mut drawable_tf = Matrix3D::IDENTITY;
    state_words(&mut out, &m, &mgr, &states, &transitions, &assets, &random);
    for s in w[k..].chunks_exact(STEP) {
        match s[0] {
            0 | 13 => {
                let new_state = StateRef::Normal(states[(s[1] as u32 % nstates as u32) as usize]);
                // `getCurrentAnimFraction` reads uninitialised locals upstream when the render object plays no animation
                // (`Peek_Animation_And_Info` answers NULL) under a maintain-frame state: undefined, the case ends there.
                // (#181 item 7c) Only when setModelState gets past its early returns (the current or the pending state; a
                // state allowed to wait for the current one to finish).
                let reaches = match m.cur_state_ref() {
                    None => true,
                    Some(cur) => {
                        let next = m.next_state_ref();
                        let (ns, cs) = (data.state(&new_state), data.state(cur));
                        !((*cur == new_state && next.is_none()) || next == Some(&new_state))
                            && !(new_state != *cur
                                && !ns.allow_to_finish_key.is_empty()
                                && ns.allow_to_finish_key == cs.transition_key
                                && m.render_object.is_some()
                                && !is_animation_complete(m.render_object.as_ref()))
                    }
                };
                let undefined = reaches
                    && m.get_cur_state().is_some_and(|c| is_any_maintain_frame_flag_set(c.flags))
                    && m.render_object.as_ref().is_some_and(|ro| ro.anim.is_none());
                if undefined {
                    out.push(700);
                    UNDEFINED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    break;
                }
                // (#181 item 7b) 13: inside the logic's update
                let mut env = DrawEnv { assets: &mut assets, random: &mut random, sync_time: now, logic_time: s[0] == 13, particles: Some(DrawParticles { manager: &mut mgr, frame: 0 }) };
                m.set_model_state(new_state, &mut env);
            }
            1 => {
                let world = matrix(&s[2..]);
                drawable_tf = world;
                let facts = DrawFacts {
                    should_animate_with_power: should_animate,
                    should_animate_without_power: should_animate,
                    speed: has_object.then_some(speed),
                    turrets: if has_object && has_ai { ai_turrets } else { [None, None] },
                    motion: None,
                    show_client_physics: true,
                    wheel_info: None,
                    transform: world,
                    drawable_transform: world,
                    construction: None,
                    frozen: false,
                    has_object,
                };
                animated_draws += u64::from(m.render_object.as_ref().is_some_and(|ro| ro.anim.is_some()));
                if let Some(c) = m.get_cur_state() {
                    if c.turrets_valid() && (0..MAX_TURRETS).any(|t| c.turret_bones(t) != (0, 0)) {
                        TURRET_DRAWS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    if c.barrels_valid() && (0..3).any(|w| c.has_recoil_bones_or_muzzle_flashes(w)) {
                        RECOIL_DRAWS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                let mut env = DrawEnv { assets: &mut assets, random: &mut random, sync_time: now, logic_time: false, particles: Some(DrawParticles { manager: &mut mgr, frame: 0 }) };
                m.do_draw_module(&facts, &mut env);
            }
            2 => now = now.wrapping_add(s[1] as u32),
            3 => m.render(now),
            4 => m.set_hidden(s[1] != 0, Some(&mut mgr)),
            5 => should_animate = s[1] != 0,
            6 => {
                has_object = s[1] != 0;
                speed = r(s[2]);
            }
            7 => {
                if let Some(ro) = m.render_object.as_mut() {
                    let n = ro.skeleton.as_ref().map_or(0, |k| k.bones.len()) as u32;
                    let bone = (s[1] as u32 / 2 % n) as i32;
                    if s[1] & 1 != 0 {
                        ro.release_bone(bone);
                    } else {
                        ro.capture_bone(bone);
                        ro.control_bone(bone, &matrix(&s[2..]));
                    }
                }
            }
            8 => {
                drawable_tf = matrix(&s[2..]);
                m.react_to_transform_change(&matrix(&s[2..]), None);
                m.update_bones_for_client_particle_systems(Some(&mut mgr), now);
            }
            9 => {
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
            10 => {
                scale = r(s[2]);
                m.scale = scale;
            }
            11 => m.set_fully_obscured_by_shroud(s[1] != 0, Some(&mut mgr)),
            12 => {
                if let Some(ro) = m.render_object.as_mut() {
                    let n = ro.skeleton.as_ref().map_or(0, |k| k.bones.len()) as u32;
                    let t = ro.get_bone_transform((s[1] as u32 % n) as i32, now);
                    out.push(600);
                    push_matrix(&mut out, &t);
                }
            }
            14 => {
                // (#181 item 7b) the fire; a barrel equal to the count is undefined upstream: the case ends ([701])
                let wslot = (s[1] as u32 % 3) as usize;
                let barrel = s[2];
                let count = m.get_cur_state().filter(|c| c.barrels_valid()).map_or(0, |c| c.weapon_barrels(wslot).len());
                if count > 0 && barrel == count as i32 {
                    out.push(701);
                    UNDEFINED_BARREL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    break;
                }
                let at = m.handle_weapon_fire_fx_at(wslot, barrel, s[3] != 0, has_object, now);
                if let Some(at) = at {
                    // `FXList::doFXPos` at the logic object (the drawable's transform here) or at the FX bone
                    let mtx = if at.render_hidden { Some(drawable_tf) } else { at.bone_transform };
                    if let Some(mtx) = mtx {
                        let t = mtx.get_translation();
                        out.extend([620, b(t.x), b(t.y), b(t.z)]);
                        push_matrix(&mut out, &mtx);
                    }
                    FIRES_HANDLED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                out.extend([610, i32::from(at.is_some())]);
            }
            15 => {
                has_ai = s[2] != 0;
                for (t, slot) in ai_turrets.iter_mut().enumerate() {
                    *slot = (s[3 + 3 * t] != 0).then(|| (r(s[4 + 3 * t]), r(s[5 + 3 * t])));
                }
            }
            // (#181 item 7c) the logic's durations; showSubObject; updateSubObjects
            16 => m.set_animation_loop_duration(s[1] as u32),
            17 => m.set_animation_completion_time(s[1] as u32),
            18 => {
                let name = HS_NAMES[(s[1] as u32 % 10) as usize];
                m.show_sub_object(if s[3] != 0 { name.to_ascii_lowercase() } else { name.to_string() }.as_bytes(), s[2] != 0);
            }
            19 => m.update_sub_objects(),
            op => panic!("op {op}"),
        }
        state_words(&mut out, &m, &mgr, &states, &transitions, &assets, &random);
        STEP_ENDS.with(|e| e.borrow_mut().push(out.len()));
        let count = |a: &std::sync::atomic::AtomicU64, yes: bool| {
            a.fetch_add(u64::from(yes), std::sync::atomic::Ordering::Relaxed);
        };
        count(&TRANSITION_STEPS, matches!(m.cur_state_ref(), Some(StateRef::Transition(_))));
        count(&PENDING_STEPS, m.next_state_ref().is_some());
        count(&SUB_LIST_STEPS, !m.sub_object_vec.is_empty());
        if matches!(s[0], 0 | 1 | 13) {
            if let Some(c) = m.get_cur_state() {
                let n = c.bone_cache().pristine_bones.keys().filter(|k| k.starts_with(b"gun") || k.starts_with(b"door")).count();
                SUB_OBJ_BONES.fetch_add(n as u64, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }
    SYSTEMS_MADE.store(u64::from(mgr.unique_system_id), std::sync::atomic::Ordering::Relaxed);
    RANDOM_DRAWN.store(u64::from(random.client[5].wrapping_sub(counter0)), std::sync::atomic::Ordering::Relaxed);
    ANIMATED_DRAWS.store(animated_draws, std::sync::atomic::Ordering::Relaxed);
    out
}

fn run_cpp(file: &[u8], input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 1 << 21];
    let n = unsafe { cpp::ref_model_draw_anim_seq(file.as_ptr() as *const _, file.len() as i32, input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0, "the C++ loaders refused the file");
    assert!(n as usize <= out.len());
    out.truncate(n as usize);
    out
}

/// Every case's comparison; returns the check and (steps, draws, draws with an animation playing, particle systems
/// made, client random values drawn, max ULP among differing finite words).
fn compare(name: &str, seed: u64, n: u64) -> (Check, [u64; 6]) {
    let mut c = Check::new(name);
    let mut g = Gen::new(seed);
    let (mut steps, mut draws, mut animated, mut systems, mut drawn, mut max_ulp) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for k in 0..n {
        let (file, w) = random_case(&mut g);
        let rust = rust_case(&file, &w);
        let cc = run_cpp(&file, &w);
        // the steps run (a case ends early where upstream is undefined)
        let ran = STEP_ENDS.with(|e| e.borrow().len());
        steps += ran as u64;
        draws += w[HEADER..].chunks_exact(STEP).take(ran).filter(|s| s[0] == 1).count() as u64;
        systems += SYSTEMS_MADE.swap(0, std::sync::atomic::Ordering::Relaxed);
        drawn += RANDOM_DRAWN.swap(0, std::sync::atomic::Ordering::Relaxed);
        animated += ANIMATED_DRAWS.swap(0, std::sync::atomic::Ordering::Relaxed);
        if rust != cc {
            if rust.len() == cc.len() {
                for (a, bb) in rust.iter().zip(&cc) {
                    let (fa, fb) = (r(*a), r(*bb));
                    if a != bb && fa.is_finite() && fb.is_finite() {
                        max_ulp = max_ulp.max(support::ulp_distance(fa, fb));
                    }
                }
            }
            let at = rust.iter().zip(&cc).position(|(a, bb)| a != bb).unwrap_or(rust.len().min(cc.len()));
            if (!planted() || std::env::var("MODEL_DRAW_ANIM_DEBUG").is_ok()) && c.mismatches < 4 {
                let step = STEP_ENDS.with(|e| e.borrow().iter().position(|&end| end > at));
                let ops: Vec<(i32, i32)> = w[HEADER..].chunks_exact(STEP).map(|s| (s[0], s[1])).collect();
                println!("case {k}: first difference in step {step:?} of {:?}", ops);
                if std::env::var("MODEL_DRAW_ANIM_DEBUG").is_ok() {
                    BLOCK.with(|t| println!("{}", t.borrow()));
                    let ends = STEP_ENDS.with(|e| e.borrow().clone());
                    let st = step.unwrap_or(0);
                    let from = if st == 0 { 0 } else { ends[st - 1] };
                    println!("rust step {st}: {:?}", &rust[from.min(rust.len())..ends[st].min(rust.len())]);
                    println!("c++  step {st}: {:?}", &cc[from.min(cc.len())..ends[st].min(cc.len())]);
                    if st > 0 {
                        let f0 = if st == 1 { 0 } else { ends[st - 2] };
                        println!("before: {:?}", &rust[f0..from]);
                    }
                }
                let lo = at.saturating_sub(8);
                println!(
                    "case {k}: at word {at} (lengths {} {}): rust {:?} c++ {:?}",
                    rust.len(),
                    cc.len(),
                    &rust[lo.min(rust.len())..(at + 16).min(rust.len())],
                    &cc[lo.min(cc.len())..(at + 16).min(cc.len())]
                );
            }
        }
        c.eq(rust, cc, || format!("case {k}"));
    }
    (c, [steps, draws, animated, systems, drawn, max_ulp])
}

/// The gated comparison: `MODEL_DRAW_ANIM_CASES` cases (20,000 by default), 0 mismatches.
#[test]
fn the_animated_draw_sequence_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (c, [steps, draws, animated, systems, drawn, max_ulp]) = compare("W3DModelDraw animated draw sequence", 0x181_0007, cases());
    let ld = |a: &std::sync::atomic::AtomicU64| a.load(std::sync::atomic::Ordering::Relaxed);
    println!(
        "W3DModelDraw animated: {} cases ({} ended where upstream is undefined: {} at a setModelState, {} at a fire's barrel), {steps} steps, {draws} draws ({animated} with an animation playing, {} with turret bones, {} with recoil bones or muzzle flashes), {} fires handled, {systems} particle systems made, {drawn} client random values drawn; (#181 item 7c) {} steps in a TransitionState, {} with a pending state, {} with a showSubObject list, {} sub-object bones counted; max ULP among differing finite words {max_ulp}",
        cases(),
        ld(&UNDEFINED) + ld(&UNDEFINED_BARREL),
        ld(&UNDEFINED),
        ld(&UNDEFINED_BARREL),
        ld(&TURRET_DRAWS),
        ld(&RECOIL_DRAWS),
        ld(&FIRES_HANDLED),
        ld(&TRANSITION_STEPS),
        ld(&PENDING_STEPS),
        ld(&SUB_LIST_STEPS),
        ld(&SUB_OBJ_BONES),
    );
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    }
    c.finish();
}
