/* Differential-test wrapper for W3DModelDraw's draw sequence with an animated render object, GitHub #181 item 7
** (upstream commit 0a05454d). What upstream does at one sync time, in doDrawModule's order: setPauseAnimation, the
** instance scale, the completed-animation branch (adjustAnimation's choice of animation, start frame and speed: client
** random values), adjustAnimSpeedToMovementSpeed, the render object's Set_Transform (the hierarchy no longer valid),
** handleClientTurretPositioning, recalcBonesForClientParticleSystems (its Get_Bone_Transform under the drawable's scale
** runs Update_Sub_Object_Transforms: Single_Anim_Progress moves the animation on to the sync time, and the pose is
** evaluated; the restore invalidates it again), updateBonesForClientParticleSystems (the first Get_Bone_Transform
** progresses again, by nothing at the same sync time, and evaluates the pose under the render object's transform; the
** others read it), handleClientRecoil; later the scene's Render (Single_Anim_Progress, the update when not valid).
**
** All of W3DModelDraw's text is cut verbatim by build.rs into OUT_DIR/model_draw_anim_gen:
**   ma_name_key.inc       GameEngine/Include/Common/NameKeyGenerator.h, `enum NameKeyType` (49-54);
**   ma_which_turret.inc   GameEngine/Include/Common/GameCommon.h, `enum WhichTurretType` (to MAX_TURRETS);
**   ma_turret_differ.inc  W3DModelDraw.cpp, `turretNamesDiffer` (2893-2905);
**   ma_ids.inc            GameEngine/Include/Common/GameType.h, `enum ObjectID` and `enum DrawableID` (41-52);
**   ma_wslot.inc          GameType.h, `enum WeaponSlotType` (173-180);
**   ma_frames.inc         GameEngine/Include/Common/GameCommon.h, LOGICFRAMES_PER_SECOND .. MSEC_PER_LOGICFRAME_REAL
**                         (70-76);
**   ma_recoil_info.inc    GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DModelDraw.h, `struct
**                         WeaponRecoilInfo` (456-478);
**   ma_maintain.inc       GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp,
**                         ALL_MAINTAIN_FRAME_FLAGS, isAnyMaintainFrameFlagSet, isCommonMaintainFrameFlagSet (251-267);
**   ma_no_next.inc        W3DModelDraw.cpp, `NO_NEXT_DURATION` (293);
**   ~~ma_test_flag.inc      W3DModelDraw.cpp, `testFlagBit` (433-436);~~ (#181 item 7b: lb_bones.inc's, below)
**   ma_ctor.inc           W3DModelDraw.cpp, the constructor's member initialisation (1722-1741);
**   ma_anim_complete.inc  W3DModelDraw.cpp, `isAnimationComplete` and `adjustAnimSpeedToMovementSpeed` (1955-1992);
**   ma_draw.inc           W3DModelDraw.cpp, `doDrawModule` (2040-2112);
**   ma_anim.inc           W3DModelDraw.cpp, `getCurrentAnimFraction`, `adjustAnimation`, `setCurAnimDurationInMsec`,
**                         `getCurAnimDistanceCovered` (2127-2270);
**   ma_recoil.inc         W3DModelDraw.cpp, `handleClientRecoil` (2491-2575);
**   ma_sms_head.inc       W3DModelDraw.cpp, setModelState from `// get this here, before we change anything` to the
**                         stop of the old particle systems (2997-3006);
**   ma_sms_tail.inc       W3DModelDraw.cpp, setModelState's end: prevState, m_curState, m_nextState, adjustAnimation
**                         (3177-3182);
**   ma_pause.inc          W3DModelDraw.cpp, `setPauseAnimation` (3833-3864);
**   (#181 item 7b)
**   ma_frames2.inc        GameCommon.h, LOGICFRAMES_PER_SECONDS_REAL .. ConvertVelocityInSecsToFrames (77-93);
**   ma_sms_mid.inc        W3DModelDraw.cpp, setModelState from `// and always hide muzzle flashes` to the first branch's
**                         doHideShowSubObjs (3008-3045): hideAllMuzzleFlashes, the model and turretNamesDiffer test,
**                         nukeCurrentRender, Create_Render_Obj, validateStuff on the new render object,
**                         rebuildWeaponRecoilInfo;
**   ma_sms_else.inc       W3DModelDraw.cpp, setModelState's else branch (3166-3172): validateStuff on the kept render
**                         object, rebuildWeaponRecoilInfo, doHideShowSubObjs;
**   ma_nuke.inc           W3DModelDraw.cpp, `nukeCurrentRender` (2787-2818);
**   ma_headlights.inc     W3DModelDraw.cpp, `hideAllHeadlights` and `hideAllMuzzleFlashes` (2849-2890);
**   ma_mf_hidden.inc      W3DModelDraw.cpp, `WeaponBarrelInfo::setMuzzleFlashHidden` (2324-2339);
**   ma_turret.inc         W3DModelDraw.cpp, `handleClientTurretPositioning` (2415-2466);
**   ma_rebuild_recoil.inc W3DModelDraw.cpp, `rebuildWeaponRecoilInfo` (3874-3906);
**   ma_fire_fx.inc        W3DModelDraw.cpp, `handleWeaponFireFX` (3696-3771);
** and re-used from GitHub #175's cuts (OUT_DIR/logic_bones_gen, see wrap_logic_bones.cpp; build.rs compiles this file
** after them): lb_valid_time.inc (isValidTimeToCalcLogicStuff), lb_pristine.inc (PristineBoneInfo, PristineBoneInfoMap),
** lb_turret_barrel.inc (TurretInfo, WeaponBarrelInfo), lb_valid_enum.inc (the m_validStuff bits), lb_public_bone.inc
** (addPublicBone), lb_bones.inc (testFlagBit, findSingleBone, findSingleSubObj, doSingleBoneName, validateStuff,
** validateCachedBones, validateWeaponBarrelInfo, validateTurretInfo, findPristineBone, findPristineBonePos,
** loadAnimations), lb_get_sub_object.inc (rendobj.cpp's Get_Sub_Object_By_Name);
**   ma_wind_defaults.inc  GameEngine/Source/GameClient/System/ParticleSys.cpp, ParticleSystemInfo()'s wind defaults
**                         (751-757);
**   ma_wind_draw.inc      ParticleSys.cpp, ParticleSystem's constructor's wind members and their client random values
**                         (1142-1148);
** and re-used from GitHub #171/#172/#174's cuts (OUT_DIR/model_draw_particles_gen, see wrap_model_draw_particles.cpp):
** mp_psid.inc, mp_tracker.inc, mp_cache_attach_bone.inc, mp_acbits.inc, mp_attach_offset.inc, mp_start_stop.inc
** (doStartOrStopParticleSys, setHidden), mp_shroud.inc, mp_stop.inc, mp_bones.inc (recalcBonesForClientParticleSystems,
** updateBonesForClientParticleSystems), mp_adjust.inc (adjustTransformMtx), mp_react.inc (reactToTransformChange),
** mp_psys_a.inc, mp_psys_b.inc (ParticleSystem's), mp_asset_scale.inc (ident_scale, scale_epsilon);
** and from GitHub #179's (OUT_DIR/w3d_animobj_gen, see wrap_w3d_eval.cpp): ao_anim_mode.inc, ao_members.inc,
** ao_body.inc, ao_progress.inc, ao_inline.inc (Animatable3DObjClass). No text is edited.
**
** The render object is that Animatable3DObjClass (namespace mda) on the stand-in base of wrap_w3d_eval.cpp (the
** transform, `Update_Sub_Object_Transforms` and `Validate_Transform` doing nothing, no embedded sounds), with HLodClass's
** `Class_ID()` (CLASSID_HLOD), RenderObjClass's `Set_Hidden`/`Is_Hidden`/`Is_Not_Hidden_At_All` and `Set_ObjectScale`
** (rendobj.h:454); made as W3DAssetManager::Create_Render_Obj makes it: `new HTreeClass(*source)` of the case's loaded
** hierarchy, then, when `reallyscale` (W3DAssetManager.cpp:738, cut), HLodClass::Scale's hierarchy part (hlod.cpp:
** 3264-3286). The hierarchy and the raw animations are the REAL ones, loaded from the case's W3D file by HTreeClass::
** Load_W3D and HRawAnimClass::Load_W3D (upstream_w3d; the hierarchy an animation names found through wrap_w3d.cpp's
** g_w3d_htree_hook), evaluated by the real htree.cpp, hrawanim.cpp and motchan.cpp; `Float_To_Long` in the Win32 branch
** (wrap_w3d.cpp's ref_w3d_set_float_to_long_mode(1)), the port's default. `WW3D::Get_Sync_Time()` is the driver's.
**
** GameClientRandomValue and GameClientRandomValueReal are the REAL GetGameClientRandomValue(Real) of RandomValue.cpp
** (through wrap_random_value.cpp's ref_client_value(_real), whose copy also answers ref_get_state); InitRandom(seed)
** first. `REAL_TO_INT` is BaseType.h:231's over wrap_base_type.cpp's transcriptions of fast_float2long_round and
** fast_float_trunc. `sinf`, `cosf`, `atan2f`, `sqrt` are the Rust's petir functions in the test binary.
**
** Stand-ins (namespace mda): the drawable (its id, hidden flag, status, `getInstanceScale()` = `getScale()` =
** `m_instanceScale`, Drawable.h:404 and Drawable.cpp:1093-1097, its transform, `getShouldAnimate(...)` the case's word,
** an object or none whose `getPhysics()->getVelocityMagnitude()` is the case's word), the module data
** (`m_animationsRequirePower`, `m_particlesAttachedToAnimatedBones`, no `AttachToBoneInAnotherModule`, the recoil
** numbers), the states (`m_animations`, `m_mode`, `m_flags`, `m_animMinSpeedFactor`, `m_animMaxSpeedFactor`,
** `m_particleSysBones`, `m_validStuff`, the barrels), W3DAnimationInfo (`getAnimHandle()`: the loaded animation of that
** name, case ignored, Add_Ref'd, else NULL; `isIdleAnim()`, `getDistanceCovered()`), ParticleSystem's members and the
** particle system manager (wrap_model_draw_particles.cpp's), and ~~setModelState's middle (the render object made again
** when the model differs or turretNamesDiffer, cut, says so: always, outside a logic time, `nukeCurrentRender`'s
** `m_pauseAnimation = false` and transform, `Set_Hidden(TRUE)` for a hidden drawable, `Set_Transform(transform)`; the rest
** of the middle has nothing to do here). The draw is outside a logic time (`isValidTimeToCalcLogicStuff()` false, as the
** port's client draws): validateStuff then leaves the pristine bones, the turrets and the barrels not valid
** (W3DModelDraw.cpp:592-598, 721-725 and validateTurretInfo's same test), so handleClientTurretPositioning and
** handleClientRecoil return at their first test (the latter is the cut; the former a stand-in that does the same) and no
** barrel is ever fired.~~ **Corrected 2026-11-13 (#181 item 7b):** setModelState's middle is cut (ma_sms_mid, ma_sms_else)
** but for the first branch's end (3047-3157: the garrison flags (debug), tracks (no TrackMarks), shadow (no shadow type),
** collision types and the scene's Add_Render_Object are not read here; the hidden test, Set_Transform(transform) and
** onRenderObjRecreated (W3DModelDraw.h:435, empty) are written as there). Op 13 runs setModelState inside the logic's
** update (`TheGameLogic->isInGameLogicUpdate()` true): validateStuff then validates the new state on the NEW render
** object (posed by validateCachedBones itself), its turrets and barrels, and handleClientTurretPositioning (cut) and
** handleClientRecoil turn and recoil real bones; handleWeaponFireFX (cut, op 14) fires a barrel. The tracks, shadow and
** decal stay NULL.
**
** (#181 item 7b) Further stand-ins: the render object's sub-objects (HLodClass's one LOD: `Get_Num_Sub_Objects`,
** `Get_Sub_Object` Add_Ref'd, `Get_Sub_Object_Bone_Index`, `Get_Sub_Object_On_Bone` as hlod.cpp:2488-2511, each a mesh
** `SKEL.SUBk` or `SKEL.HEADLIGHTk` on the case's bone, hidden as made or not; no requested bone name is a sub-object's,
** so findSingleSubObj finds none), reference counts (`Add_Ref`/`Release_Ref`), `W3DDisplay::m_assetManager`'s
** `Create_Render_Obj` (the case's M0..M2, any case, with those sub-objects; else NULL) and `m_3DScene` (nothing to
** remove); the name keys (`NAMEKEY`, each distinct string a number from 1, as wrap_logic_bones.cpp's);
** `TheGlobalData->m_standardPublicBones` (the case's); the module data's recoil numbers (the speeds through
** ConvertVelocityInSecsToFrames, cut, as INI::parseVelocityReal) and `m_extraPublicBones`; the states' turret and weapon
** bone names as the INI parse leaves them (lower-cased, `None` an invalid key or cleared, addPublicBone of each; a
** ConditionState copies the default state's and its own lines replace them); TurretArtAngle/Pitch as
** INI::parseAngleReal (`* (PI / 180.0f)`); the object's AI (`getTurretRotAndPitch`, AIUpdate.cpp:748-759: the case's
** turret, else FALSE), its transform and position (the drawable's); `FXList::doFXPos` records where a list plays
** ([620, x, y, z, the 12 words], wrap_ai.cpp's stand-in's data; nothing without a list). doHideShowSubObjs has nothing to
** do (no ShowSubObject lines, no showSubObject calls: it aborts otherwise).
**
** (#181 item 7c) Cut besides (OUT_DIR/model_draw_anim_gen):
**   ma_sms_pre.inc        W3DModelDraw.cpp, setModelState from `const ModelConditionInfo* nextState = NULL;` to the
**                         head: the early returns (the current or pending state), allow-to-finish, the transition lookup;
**   ma_find_trans.inc     W3DModelDraw.cpp, `findTransitionForSig` (2115-2125);
**   ma_hide_show_bone.inc W3DModelDraw.cpp, `doHideShowBoneSubObjs` (2271-2304);
**   ma_hide_show.inc      W3DModelDraw.cpp, `doHideShowSubObjs` (2342-2383);
**   ma_show_sub.inc       W3DModelDraw.cpp, `showSubObject` (1308-1330);
**   ma_update_subs.inc    W3DModelDraw.cpp, `updateSubObjects` (3978-4009);
**   ma_durations.inc      W3DModelDraw.cpp, `setAnimationLoopDuration` and `setAnimationCompletionTime` (3772-3819);
**   ma_anim_info.inc      W3DModelDraw.cpp, W3DAnimationInfo's constructors, assignment, getAnimHandle, destructor
**                         (296-384: m_naturalDurationInMsec -1 until the animation is first found);
**   ma_anim_info_h.inc    W3DModelDraw.h, `class W3DAnimationInfo` (96-120);
**   ma_trans_sig.inc      W3DModelDraw.h, TransitionSig, NO_TRANSITION, buildTransitionSig, recoverSrc/DstState;
**   ma_trans_map.inc      W3DModelDraw.h:272, `TransitionMap`.
** Stand-ins added: HLodClass (Set_Transform/Set_Position mark the sub-objects' transforms dirty, hlod.cpp:2222-2245;
** Update_Sub_Object_Transforms evaluates the hierarchy and gives each sub-object its bone's transform, 3351-3387, one
** LOD) and MeshStandIn (a sub-object: RenderObjClass's Set_Transform, no update); the render objects' container, dirty
** bit and Validate_Transform (rendobj.cpp:464-491, IsTransformIdentity not read), so findSingleSubObj's Get_Transform
** reads the live sub-object as upstream's; Get_HTree; W3DAssetManager::Get_HAnim; the states' TransitionKey,
** WaitForStateToFinishIfPossible, show/hide lists and the TransitionStates as the INI parse leaves them (a ConditionState
** and a TransitionState copy the default state; a TransitionState's keys cleared). After the header above come: 4
** sub-object name codes (0 as before, 1..4 `SKEL.` + SUB_NAMES), 4 states x (TransitionKey code or -1, wait-to-finish
** code or -1, show/hide count 0..2, 2 x (hide, name, name or 9)), the TransitionState count (0..3) and 3 x (key, key,
** model, mode, flags, min speed, max speed, animation count 0..2, 2 x (animation, distance), show/hide lines). Ops
** 16 setAnimationLoopDuration(a1), 17 setAnimationCompletionTime(a1), 18 showSubObject(HS_NAMES[a1 % 10], lower-cased
** when word 3, shown when word 2), 19 updateSubObjects. Records added before the client random state: the current and
** pending state codes (state_code), m_nextStateAnimLoopDuration, the showSubObject list, each TransitionState's caches.
**
** Entry point (extern "C", tests/differential_model_draw_anim.rs; floats travel as their bits):
**   ref_model_draw_anim_seq(file, flen, in, nin, out, cap): file = the W3D file (one hierarchy, then raw animations);
**   in = [seed, sync time, drawable id, scale, particlesAttachedToAnimatedBones, animationsRequirePower,
**     4 templates' slave (-1 none), state count s (1..4), 4 states x (model (0..2 the hierarchy's render object M0..M2,
**     3 None), mode (AnimMode), flags, min speed, max speed, animation count (0..3), 3 x (animation index (an index
**     into the file's animations, or 99: a name that is not there), idle, distance covered), bone count (0..3), 3 x
**     (bone name code, template code (0..3, 4 None, 5 unknown)))], then steps of 14 words [op, a1, 12 words]:
**     0 setModelState(state a1 % s); 1 the drawable's transform = the 12 words, then doDrawModule(&it);
**     2 the sync time += a1 (unsigned); 3 the render object's Render (the scene's); 4 the drawable hidden = a1, then
**     setHidden(a1); 5 getShouldAnimate answers a1; 6 the object present = a1 with velocity magnitude word 2;
**     7 Capture_Bone(b) and Control_Bone(b, the 12 words) on the render object, b = a1 % its pivot count, or
**     Release_Bone(b) when a1 is odd ... (see the case); 8 the drawable's transform = the 12 words, reactToTransformChange,
**     updateBonesForClientParticleSystems (AnimatedParticleSysBoneClientUpdate's call); 9 the system a1 % count (in id
**     order) dropped; 10 the instance scale = word 2's float; 11 setFullyObscuredByShroud(a1); 12 Get_Bone_Transform(b)
**     of the render object (b = a1 % its pivot count; recorded as [600, the 12 words]); (#181 item 7b) 13 setModelState(state
**     a1 % s) inside the logic's update; 14 handleWeaponFireFX(slot a1 % 3, barrel word 2, an FX list when word 3),
**     recorded [610, handled] after doFXPos's [620 ...] (a barrel equal to the slot's barrel count ends the case, [701]:
**     upstream reads one past the end); 15 the object's AI (word 2) and each turret's (present, angle, pitch) in words
**     3-8. (#181 item 7b) After the states, the header has the recoil numbers (InitialRecoilSpeed, MaxRecoilDistance,
**     RecoilDamping, RecoilSettleSpeed as written), the sub-object count (0..4) and 4 x (bone, hidden, headlight), the
**     standard public bones and the ExtraPublicBone names (each a count 0..2 and 2 name codes), then 4 states x (own
**     turrets, 2 x (angle name, pitch name, art angle, art pitch in degrees), 3 slots x (FX, recoil, muzzle flash, launch
**     name or 9: no line)).
**   The constructor runs setModelState(state 0) first. A setModelState that would read getCurrentAnimFraction's
**   uninitialised locals (a current state with a maintain-frame flag and a render object in BASE_POSE: undefined upstream)
**   ends the case instead, recorded as [700] (no state after it). Records: after the constructor and after each step the state:
**   [100, needRecalc, fullyObscured, whichAnim, pauseAnimation, animationMode, current state index, tracker count, then
**   (id, boneIndex) each, system count, per system in id order (id, attached drawable, saveable, stopped, destroyed,
**   skipParentXfrm, isLocalIdentity, slave id, master id, the local transform's 12 words), render object present, and
**   with one: CurMotionMode, IsTreeValid, in SINGLE_ANIM the motion's index (else -1), Frame, PrevFrame, LastSyncTime,
**   animDirection, frameRateMultiplier, AnimMode, Is_Animation_Complete (zeros outside SINGLE_ANIM), Is_Hidden, the
**   Transform's 12 words, ObjectScale, the pivot count and each pivot's Get_Transform (12 words, no update), (#181 item 7b)
**   the sub-object count and each one's Is_Hidden; then (#181 item 7b) per slot the recoils (count, then state, shift,
**   rate), per state its caches (m_validStuff & (PRISTINE_BONES_VALID | TURRETS_VALID | BARRELS_VALID), the pristine bones
**   in name order (length, characters, bone index, matrix), the turrets' bones, per slot the barrels (recoil, FX,
**   muzzle-flash bone, projectile offset) and m_hasRecoilBonesOrMuzzleFlashes); then the client random state (6 words)].
**   Returns the word count. */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <stdio.h>
#include <stdarg.h>
#include <strings.h>
#include <vector>
#include <map>
#include <string>
#include <algorithm>

#include "simplevec_this.h" /* generated by build.rs */
#include "htree.h"
#include "hanim.h"
#include "hrawanim.h"
#include "hcanim.h"
#include "motchan.h"
#include "w3d_file.h"
#include "chunkio.h"
#include "matrix3d.h"
#include "vector3.h"
#include "wwmath.h"
#include "quat.h"
#include "shim/Lib/BaseType.h"

extern "C" {
void *ref_w3d_ramfile_open(const char *data, int len);
int ref_w3d_ramfile_read(void *f, void *buffer, int bytes);
int ref_w3d_ramfile_seek(void *f, int pos, int mode);
int ref_w3d_ramfile_size(void *f);
void ref_w3d_ramfile_close(void *f);
void ref_wwmath_init_tables(void);
void ref_w3d_set_float_to_long_mode(int mode);
/* wrap_random_value.cpp: RandomValue.cpp verbatim */
void ref_init_random(unsigned int seed);
int ref_client_value(int lo, int hi);
float ref_client_value_real(float lo, float hi);
void ref_get_state(int which, unsigned int *out6);
/* wrap_base_type.cpp */
int ref_fast_float2long_round(float f);
float ref_fast_float_trunc(float f);
float ref_fast_float_floor(float f);
}

/* wrap_w3d.cpp's lookup hook */
extern HTreeClass *(*g_w3d_htree_hook)(const char *);

/* ClientRandomValue.h:42-43, on the real functions (see the header) */
#define GameClientRandomValue( lo, hi ) ref_client_value( lo, hi )
#define GameClientRandomValueReal( lo, hi ) ref_client_value_real( lo, hi )
/* Common/Debug.h in a release build (no _DEBUG, no _INTERNAL): the asserts and logs compile to nothing */
#define DEBUG_ASSERTCRASH(c, m)
#define DEBUG_LOG(m)
#define DEBUG_CRASH(m)
/* BaseType.h:231 */
#define REAL_TO_INT(x) ((Int)(ref_fast_float2long_round(ref_fast_float_trunc(x))))
/* (#181 item 7c) BaseType.h:244, on the same transcriptions */
#define REAL_TO_INT_FLOOR(x) (ref_fast_float2long_round(ref_fast_float_floor(x)))
/* (#181 item 7b) W3DModelDraw.cpp's logging, compiled out as upstream builds it (DEBUG_CRC not defined), as
** wrap_logic_bones.cpp has it */
#define BONEPOS_LOG(x) {}
#define BONEPOS_DUMPMATRIX3D(x) {}
#define BONEPOS_DUMPMATRIX3DNAMED(x, y) {}
#define BONEPOS_DUMPREAL(x) {}
#define BONEPOS_DUMPREALNAMED(x, y) {}
#define CRCDEBUG_LOG(x) {}
#define DUMPMATRIX3D(x) {}
#ifndef stricmp
#define stricmp strcasecmp
#endif

namespace mda {

std::vector<int> g_out;
void rec(const std::vector<int> &w) { g_out.insert(g_out.end(), w.begin(), w.end()); }
int bits(float f)
{
	int i;
	memcpy(&i, &f, 4);
	return i;
}
float real(int i)
{
	float f;
	memcpy(&f, &i, 4);
	return f;
}
void recM(const Matrix3D &m)
{
	for (int r = 0; r < 3; ++r)
		for (int c = 0; c < 4; ++c) rec({bits(m[r][c])});
}
Matrix3D matrix(const int *w)
{
	Matrix3D m(true);
	for (int r = 0; r < 3; ++r)
		for (int c = 0; c < 4; ++c) m[r][c] = real(w[r * 4 + c]);
	return m;
}

#include "ma_name_key.inc"
#include "ma_ids.inc"
#include "ma_wslot.inc"
#include "ma_which_turret.inc"
#include "ma_frames.inc"
#include "ma_frames2.inc"
#include "mp_psid.inc"
#include "mp_cache_attach_bone.inc"
#include "mp_acbits.inc"
#include "ma_maintain.inc"
#include "ma_no_next.inc"

/* (#181 item 7b) the logic time: isValidTimeToCalcLogicStuff (lb_valid_time.inc, W3DModelDraw.cpp:78-82) over
** TheGameLogic->isInGameLogicUpdate(), the case's op (13: a setModelState inside the logic's update), and no load */
Bool g_logic_time;
class GameLogic { public: Bool isInGameLogicUpdate() const { return g_logic_time; } };
GameLogic g_logic;
GameLogic *TheGameLogic = &g_logic;
class GameState { public: Bool isInLoadGame() const { return FALSE; } };
GameState *TheGameState = NULL;
#include "lb_valid_time.inc"

/* TheNameKeyGenerator: each distinct string a number from 1 (case-sensitive, as wrap_logic_bones.cpp's) */
std::map<std::string, int> g_keys;
NameKeyType namekey(const char *s)
{
	std::map<std::string, int>::iterator it = g_keys.find(s);
	if (it != g_keys.end()) return (NameKeyType)it->second;
	int k = (int)g_keys.size() + 1;
	g_keys[s] = k;
	return (NameKeyType)k;
}
#define NAMEKEY(x) mda::namekey_of(x)
#define KEYNAME(x) AsciiString("")
void setFPMode() {}

/* GameEngine/Include/GameClient/Drawable.h:237 */
enum DrawableStatus { DRAWABLE_STATUS_NO_STATE_PARTICLES = 0x00000008 };
/* the object adjustTransformMtx and reactToTransformChange name (no state is ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT here,
** and no track is made, so none of them is called) */
enum ObjectStatusTypes { OBJECT_STATUS_STEALTHED = 1 };

/* AsciiString's members the cuts call */
class AsciiString
{
public:
	std::string s;
	AsciiString() {}
	AsciiString(const char *c) : s(c) {}
	const char *str() const { return s.c_str(); }
	Bool isEmpty() const { return s.empty(); }
	Bool isNotEmpty() const { return !s.empty(); }
	void toLower() { for (size_t i = 0; i < s.size(); ++i) s[i] = (char)tolower((unsigned char)s[i]); }
	/* AsciiString.cpp: `compareNoCase("None") == 0` */
	Bool isNone() const { return strcasecmp(s.c_str(), "None") == 0; }
	void clear() { s.clear(); }
	void format(const char *f, ...)
	{
		char b[512];
		va_list ap;
		va_start(ap, f);
		vsnprintf(b, sizeof(b), f, ap);
		va_end(ap);
		s = b;
	}
	bool operator==(const AsciiString &o) const { return s == o.s; }
	bool operator!=(const AsciiString &o) const { return s != o.s; }
};
NameKeyType namekey_of(const char *s) { return namekey(s); }
NameKeyType namekey_of(const AsciiString &s) { return namekey(s.str()); }

/* ---- the render object (see the header) ---- */
unsigned int g_sync_time;
class WW3D
{
public:
	static unsigned int Get_Sync_Time(void) { return g_sync_time; }
};
class AnimatedSoundMgrClass
{
public:
	static const char *Get_Embedded_Sound_Name(HAnimClass *) { return NULL; }
	static float Trigger_Sound(HAnimClass *, float, float, const Matrix3D &) { abort(); }
};
class RenderInfoClass {};
class SpecialRenderInfoClass {};

class CompositeRenderObjClass
{
public:
#include "ao_anim_mode.inc"
	/* rendobj.h's class ids: the one this render object answers (HLodClass::Class_ID) */
	enum { CLASSID_HLOD = 1 };
	Matrix3D Transform;
	bool Hidden;
	float ObjectScale;
	/* (#181 item 7c) RenderObjClass's Container (the HLOD a sub-object is in, NULL for the HLOD) and its
	** SUBOBJ_TRANSFORMS_DIRTY bit (rendobj.h:494-495) */
	CompositeRenderObjClass *Container;
	bool SubObjTransformsDirty;
	CompositeRenderObjClass() : Transform(true), Hidden(false), ObjectScale(1.0f), Container(NULL), SubObjTransformsDirty(false) {}
	virtual ~CompositeRenderObjClass() {}
	/* RenderObjClass's Set_Collision_Type and the user data are not read here */
	int Class_ID(void) const { return CLASSID_HLOD; }
	/* (#181 item 7c) virtual, as RenderObjClass's: HLodClass's (below) also marks its sub-objects' transforms dirty */
	virtual void Set_Transform(const Matrix3D &m) { Transform = m; }
	virtual void Set_Position(const Vector3 &v) { Transform.Set_Translation(v); }
	/* RenderObjClass::Update_Sub_Object_Transforms (rendobj.cpp:777-779): nothing; virtual */
	virtual void Update_Sub_Object_Transforms(void) {}
	/* (#181 item 7c) RenderObjClass::Validate_Transform (rendobj.cpp:464-491) as there (IsTransformIdentity is not read
	** here): a sub-object whose containers say their sub-objects' transforms are dirty has the outermost one update them
	** (HLodClass::Update_Sub_Object_Transforms: the hierarchy evaluated, each sub-object given its bone's transform) */
	void Validate_Transform(void) const
	{
		CompositeRenderObjClass *con = Container;
		bool dirty = false;
		if (con != NULL) {
			dirty = con->Are_Sub_Object_Transforms_Dirty();
			while (con->Container != NULL) {
				dirty |= con->Are_Sub_Object_Transforms_Dirty();
				con = con->Container;
			}
			if (dirty) {
				con->Update_Sub_Object_Transforms();
			}
		}
	}
	/* rendobj.h:617-621 */
	const Matrix3D &Get_Transform(void) const { Validate_Transform(); return Transform; }
	int Is_Not_Hidden_At_All(void) { return !Hidden; }
	void Set_Sub_Object_Transforms_Dirty(bool onoff) { SubObjTransformsDirty = onoff; }
	bool Are_Sub_Object_Transforms_Dirty(void) { return SubObjTransformsDirty; }
	/* RenderObjClass's hidden bit and rendobj.h:454 */
	void Set_Hidden(int onoff) { Hidden = onoff != 0; }
	int Is_Hidden(void) const { return Hidden ? 1 : 0; }
	void Set_ObjectScale(float scale) { ObjectScale = scale; }
};

class Animatable3DObjClass : public CompositeRenderObjClass
{
public:
	Animatable3DObjClass(const HTreeClass *source, float scale);
	/* (#181 item 7b) a sub-object of an HLOD (a mesh: no hierarchy, plays no animation): its name, as made hidden or not */
	Animatable3DObjClass(const char *name, bool hidden);
	virtual ~Animatable3DObjClass(void)
	{
		Release();
		if (HTree) delete HTree;
		for (size_t i = 0; i < Subs.size(); ++i) Subs[i]->Release_Ref();
	}
	/* RefCountClass's */
	int Refs;
	void Add_Ref(void) { ++Refs; }
	void Release_Ref(void) { if (--Refs == 0) delete this; }
	/* (#181 item 7b) HLodClass's sub-objects, one LOD (hlod.cpp's `Lod[0]`, each a model on a bone, in order) */
	std::string Name;
	std::vector<Animatable3DObjClass *> Subs;
	std::vector<int> SubBones;
	const char *Get_Name(void) const { return Name.c_str(); }
	/* (#181 item 7c) animobj.h:112 */
	const HTreeClass *Get_HTree(void) const { return HTree; }
	int Get_Num_Sub_Objects(void) const { return (int)Subs.size(); }
	/* HLodClass::Get_Sub_Object (Add_Ref'd) */
	Animatable3DObjClass *Get_Sub_Object(int index) const
	{
		if (index < 0 || index >= (int)Subs.size()) return NULL;
		Subs[index]->Add_Ref();
		return Subs[index];
	}
	/* HLodClass::Get_Sub_Object_Bone_Index(LodIndex, ModelIndex) */
	int Get_Sub_Object_Bone_Index(int, int model) const { return SubBones[model]; }
	/* HLodClass::Get_Sub_Object_On_Bone (hlod.cpp:2488-2511), one LOD and no additional models */
	Animatable3DObjClass *Get_Sub_Object_On_Bone(int index, int boneindex) const
	{
		int count = 0;
		for (int model = 0; model < (int)Subs.size(); model++) {
			if (SubBones[model] == boneindex) {
				if (count == index) {
					Subs[model]->Add_Ref();
					return Subs[model];
				}
				count++;
			}
		}
		return NULL;
	}
	/* rendobj.cpp's (lb_get_sub_object.inc) */
	Animatable3DObjClass *Get_Sub_Object_By_Name(const char *name, int *index = NULL) const;
	void Render(RenderInfoClass &rinfo);
	void Special_Render(SpecialRenderInfoClass &rinfo);
	void Set_Transform(const Matrix3D &m);
	void Set_Position(const Vector3 &v);
	void Set_Animation(void);
	void Set_Animation(HAnimClass *motion, float frame, int anim_mode = ANIM_MODE_MANUAL);
	void Set_Animation(HAnimClass *motion0, float frame0, HAnimClass *motion1, float frame1, float percentage);
	void Set_Animation(HAnimComboClass *anim_combo);
	void Set_Animation_Frame_Rate_Multiplier(float multiplier);
	HAnimClass *Peek_Animation_And_Info(float &frame, int &numFrames, int &mode, float &mult);
	HAnimClass *Peek_Animation(void);
	bool Is_Animation_Complete(void) const;
	int Get_Num_Bones(void);
	const char *Get_Bone_Name(int bone_index);
	int Get_Bone_Index(const char *bonename);
	const Matrix3D &Get_Bone_Transform(const char *bonename);
	const Matrix3D &Get_Bone_Transform(int boneindex);
	void Capture_Bone(int boneindex);
	void Release_Bone(int boneindex);
	bool Is_Bone_Captured(int boneindex) const;
	void Control_Bone(int bindex, const Matrix3D &objtm, bool world_space_translation = false);
	float Compute_Current_Frame(float *newDirection = NULL) const;
	void Update_Sub_Object_Transforms(void);
	void Base_Update(const Matrix3D &root);
	void Anim_Update(const Matrix3D &root, HAnimClass *motion, float frame);
	void Blend_Update(const Matrix3D &root, HAnimClass *motion0, float frame0, HAnimClass *motion1, float frame1, float percentage);
	void Combo_Update(const Matrix3D &root, HAnimComboClass *anim);
	bool Is_Hierarchy_Valid(void) const { return IsTreeValid; }
	void Set_Hierarchy_Valid(bool onoff) const { IsTreeValid = onoff; }
	void Single_Anim_Progress(void);
	void Release(void);

#include "ao_members.inc"
};

#include "ao_body.inc"
#include "ao_progress.inc"
#include "ao_inline.inc"
#include "mp_asset_scale.inc"

/* animobj.cpp:84-103's initialisation, the hierarchy copied from the loaded one (animobj.cpp:113); then
** Create_Render_Obj's `reallyscale` (W3DAssetManager.cpp:738) and HLodClass::Scale's hierarchy part (hlod.cpp:3264-3286) */
Animatable3DObjClass::Animatable3DObjClass(const HTreeClass *source, float scale) : Refs(1), IsTreeValid(0), CurMotionMode(BASE_POSE)
{
	ModeAnim.Motion = NULL;
	ModeAnim.Frame = 0.0f;
	ModeAnim.PrevFrame = 0.0f;
	ModeAnim.LastSyncTime = WW3D::Get_Sync_Time();
	ModeAnim.frameRateMultiplier = 1.0;
	ModeAnim.animDirection = 1.0;
	ModeInterp.Motion0 = NULL;
	ModeInterp.Motion1 = NULL;
	ModeInterp.Frame0 = 0.0f;
	ModeInterp.PrevFrame0 = 0.0f;
	ModeInterp.PrevFrame1 = 0.0f;
	ModeInterp.Frame1 = 0.0f;
	ModeInterp.Percentage = 0.0f;
	ModeCombo.AnimCombo = NULL;
	HTree = new HTreeClass(*source);
	Bool reallyscale = (WWMath::Fabs(scale - ident_scale) > scale_epsilon);
	if (reallyscale && scale != 1.0f) {
		HTree->Scale(scale);
		Set_Hierarchy_Valid(false);
	}
}
Animatable3DObjClass::Animatable3DObjClass(const char *name, bool hidden) : Refs(1), IsTreeValid(0), CurMotionMode(BASE_POSE)
{
	ModeAnim.Motion = NULL;
	ModeAnim.Frame = 0.0f;
	ModeAnim.PrevFrame = 0.0f;
	ModeAnim.LastSyncTime = WW3D::Get_Sync_Time();
	ModeAnim.frameRateMultiplier = 1.0;
	ModeAnim.animDirection = 1.0;
	ModeInterp.Motion0 = NULL;
	ModeInterp.Motion1 = NULL;
	ModeCombo.AnimCombo = NULL;
	HTree = NULL;
	Name = name;
	Hidden = hidden;
}
typedef Animatable3DObjClass RenderObjClass;
/* (#181 item 7c) HLodClass's overrides that keep its sub-objects' transforms (hlod.cpp): Set_Transform and Set_Position
** mark them dirty (2222-2245); Update_Sub_Object_Transforms (3351-3387) evaluates the hierarchy (animobj.cpp's) and puts
** each sub-object's bone transform into it, then clears the bit (one LOD, no additional models; Set_Animation_Hidden is
** not read here) */
class HLodClass : public Animatable3DObjClass
{
public:
	HLodClass(const HTreeClass *source, float scale) : Animatable3DObjClass(source, scale) {}
	void Set_Transform(const Matrix3D &m)
	{
		Animatable3DObjClass::Set_Transform(m);
		Set_Sub_Object_Transforms_Dirty(true);
	}
	void Set_Position(const Vector3 &v)
	{
		Animatable3DObjClass::Set_Position(v);
		Set_Sub_Object_Transforms_Dirty(true);
	}
	void Update_Sub_Object_Transforms(void)
	{
		Animatable3DObjClass::Update_Sub_Object_Transforms();
		for (int model = 0; model < (int)Subs.size(); model++) {
			RenderObjClass *robj = Subs[model];
			int bone = SubBones[model];
			robj->Set_Transform(HTree->Get_Transform(bone));
			robj->Update_Sub_Object_Transforms();
		}
		Set_Sub_Object_Transforms_Dirty(false);
	}
};
/* (#181 item 7c) a sub-object (MeshClass): RenderObjClass's Set_Transform and Update_Sub_Object_Transforms (nothing) */
class MeshStandIn : public Animatable3DObjClass
{
public:
	MeshStandIn(const char *name, bool hidden) : Animatable3DObjClass(name, hidden) {}
	void Set_Transform(const Matrix3D &m) { CompositeRenderObjClass::Set_Transform(m); }
	void Set_Position(const Vector3 &v) { CompositeRenderObjClass::Set_Position(v); }
	void Update_Sub_Object_Transforms(void) {}
};
#include "lb_get_sub_object.inc"

/* ---- the loaded file ---- */
HTreeClass *g_tree;
std::vector<HAnimClass *> g_anims;
HTreeClass *find_tree(const char *name)
{
	if (g_tree && memcmp(g_tree->Get_Name(), name, W3D_NAME_LEN) == 0) return g_tree;
	return NULL;
}

/* (#181 item 7b) the models' sub-objects (the same on M0..M2): each a mesh on a bone, hidden as made or not, named
** `SKEL.SUBk` or `SKEL.HEADLIGHTk`; (#181 item 7c) or, by its name code (1..4), `SKEL.` and one of SUB_NAMES */
struct SubSpec { int bone; bool hidden; bool headlight; int name; };
std::vector<SubSpec> g_subs;
const char *const MODEL_NAMES[] = { "M0", "M1", "M2" };
const char *const SUB_NAMES[] = { "", "GUN", "GUN01", "GUN02", "DOOR" };
/* W3DAssetManager::Create_Render_Obj(name, scale, color): the case's HLOD M0..M2 (any case; see the header), else NULL */
class W3DAssetManager
{
public:
	RenderObjClass *Create_Render_Obj(const char *name, float scale, int)
	{
		for (int m = 0; m < 3; ++m)
			if (strcasecmp(name, MODEL_NAMES[m]) == 0) {
				RenderObjClass *r = new HLodClass(g_tree, scale);
				r->Name = MODEL_NAMES[m];
				for (size_t k = 0; k < g_subs.size(); ++k) {
					char n[40];
					if (g_subs[k].name > 0)
						snprintf(n, sizeof(n), "SKEL.%s", SUB_NAMES[g_subs[k].name]);
					else
						snprintf(n, sizeof(n), "SKEL.%s%d", g_subs[k].headlight ? "HEADLIGHT" : "SUB", (int)k);
					RenderObjClass *sub = new MeshStandIn(n, g_subs[k].hidden);
					/* HLodClass's constructor from its prototype: each LOD model's container is the HLOD (hlod.cpp:1273), and
					** the sub-objects' transforms are dirty as made (1311) */
					sub->Container = r;
					r->Subs.push_back(sub);
					r->SubBones.push_back(g_subs[k].bone % r->HTree->Num_Pivots());
				}
				r->Set_Sub_Object_Transforms_Dirty(true);
				return r;
			}
		return NULL;
	}
	/* (#181 item 7c) WW3DAssetManager::Get_HAnim: the loaded animation of that name (case ignored), Add_Ref'd, else NULL */
	HAnimClass *Get_HAnim(const char *name)
	{
		for (size_t i = 0; i < g_anims.size(); ++i)
			if (strcasecmp(g_anims[i]->Get_Name(), name) == 0) {
				g_anims[i]->Add_Ref();
				return g_anims[i];
			}
		return NULL;
	}
};
class SceneClass { public: void Remove_Render_Object(RenderObjClass *) {} };
W3DAssetManager g_am;
SceneClass g_scene;
class W3DDisplay
{
public:
	static W3DAssetManager *m_assetManager;
	static SceneClass *m_3DScene;
};
W3DAssetManager *W3DDisplay::m_assetManager = &g_am;
SceneClass *W3DDisplay::m_3DScene = &g_scene;

/* ---- the drawable, its object, the particle systems ---- */
class PhysicsBehavior
{
public:
	Real m_speed;
	Real getVelocityMagnitude() const { return m_speed; }
};
class GeometryInfo
{
public:
	Real getMaxHeightAbovePosition() const { abort(); return 0; }
};
/* (#181 item 7b) the object's AI: getTurretRotAndPitch (AIUpdate.cpp:748-759) answers the case's turret, or FALSE
** without one (the angles untouched) */
class AIUpdateInterface
{
public:
	Bool m_has[MAX_TURRETS];
	Real m_angle[MAX_TURRETS], m_pitch[MAX_TURRETS];
	Bool getTurretRotAndPitch(WhichTurretType tur, Real *turretAngle, Real *turretPitch) const
	{
		if (m_has[tur]) {
			if (turretAngle) *turretAngle = m_angle[tur];
			if (turretPitch) *turretPitch = m_pitch[tur];
			return TRUE;
		}
		return FALSE;
	}
};
AIUpdateInterface g_ai;
Bool g_hasAI;
class Object
{
public:
	PhysicsBehavior m_physics;
	Coord3D m_pos;
	const PhysicsBehavior *getPhysics() const { return &m_physics; }
	const AIUpdateInterface *getAIUpdateInterface() const { return g_hasAI ? &g_ai : NULL; }
	/* the logic object's transform and position: the drawable's (see the header) */
	const Matrix3D *getTransformMatrix() const;
	const Coord3D *getPosition() const;
	Real getConstructionPercent() const { abort(); return 0; }
	const GeometryInfo &getGeometryInfo() const { abort(); }
	Bool testStatus( ObjectStatusTypes ) const { abort(); return FALSE; }
	Bool isSignificantlyAboveTerrain() const { abort(); return FALSE; }
};
Object g_object;
Bool g_hasObject;

class Drawable
{
public:
	DrawableID m_id;
	Bool m_hidden;
	UnsignedInt m_status;
	Real m_scale;
	Coord3D m_pos;
	Matrix3D m_transform;
	Bool m_shouldAnimate;
	DrawableID getID() const { return m_id; }
	Bool isDrawableEffectivelyHidden() const { return m_hidden; }
	Bool testDrawableStatus( DrawableStatus bit ) const { return (m_status & bit) != 0; }
	const Real getScale( void ) const { return m_scale; }
	Real getInstanceScale( void ) const { return m_scale; }
	const Coord3D *getPosition() const { return &m_pos; }
	const Matrix3D *getTransformMatrix() const { return &m_transform; }
	Object *getObject() const { return g_hasObject ? &g_object : NULL; }
	/* Drawable::getShouldAnimate(considerPower): the case's answer */
	Bool getShouldAnimate( Bool ) const { return m_shouldAnimate; }
	Int getPristineBonePositions(const char*, Int, Coord3D*, Matrix3D*, Int) const { abort(); return 0; }
};
Drawable g_drawable;
const Matrix3D *Object::getTransformMatrix() const { return &g_drawable.m_transform; }
const Coord3D *Object::getPosition() const
{
	Coord3D *p = const_cast<Coord3D *>(&m_pos);
	p->x = g_drawable.m_transform.Get_X_Translation();
	p->y = g_drawable.m_transform.Get_Y_Translation();
	p->z = g_drawable.m_transform.Get_Z_Translation();
	return &m_pos;
}

/* (#181 item 7b) FXList::doFXPos (FXList.h's inline: nothing without a list), recording where the list plays, as
** wrap_ai.cpp's stand-in records it: [620, primary x, y, z, the primary matrix's 12 words] */
class FXList
{
public:
	static void doFXPos(const FXList *fx, const Coord3D *primary, const Matrix3D *primaryMtx, const Real, const Coord3D *, const Real)
	{
		if (!fx) return;
		rec({620, bits(primary->x), bits(primary->y), bits(primary->z)});
		recM(*primaryMtx);
	}
};
FXList g_fxl;

/* BaseType.h:86-87 (the shim's BaseType.h does not carry them) */
#ifndef PI
#define PI 3.14159265359f
#endif
#ifndef TWO_PI
#define TWO_PI 6.28318530718f
#endif

/* the template: its slave, and ParticleSystemInfo's wind members with ParticleSystemInfo()'s defaults (cut: the
** templates here set no wind field) */
class ParticleSystemTemplate
{
public:
	const ParticleSystemTemplate *m_slave;
	Real m_windAngle, m_windAngleChange, m_windAngleChangeMin, m_windAngleChangeMax;
	Real m_windMotionStartAngle, m_windMotionStartAngleMin, m_windMotionStartAngleMax;
	Real m_windMotionEndAngle, m_windMotionEndAngleMin, m_windMotionEndAngleMax;
	Byte m_windMotionMovingToEndAngle;
	ParticleSystemTemplate()
	{
#include "ma_wind_defaults.inc"
		/* 757 (the line also occurs elsewhere: written here) */
		m_windMotionMovingToEndAngle = TRUE;
	}
};
ParticleSystemTemplate g_templates[4];

/* wrap_model_draw_particles.cpp's ParticleSystem and manager */
class ParticleSystem
{
public:
	ParticleSystemID m_systemID;
	Matrix3D m_localTransform;
	Bool m_isLocalIdentity, m_isStopped, m_isDestroyed, m_isSaveable, m_skipParentXfrm;
	DrawableID m_attachedToDrawableID;
	ObjectID m_attachedToObjectID;
	ParticleSystem *m_slaveSystem, *m_masterSystem;
	Real m_windMotionStartAngle, m_windMotionStartAngleMin, m_windMotionStartAngleMax;
	Real m_windMotionEndAngle, m_windMotionEndAngleMin, m_windMotionEndAngleMax;
	Real m_windAngle;
	Byte m_windMotionMovingToEndAngle;
	/* (#181 item 7) ParticleSystem's constructor's client random values (ParticleSys.cpp:1142-1148, cut), drawn before
	** the slave is made, as there; the other draw it can make, `m_initialDelay.getValue()` (1106), draws nothing on the
	** templates' zero range (GetGameClientRandomValueReal returns at `delta <= 0`) */
	ParticleSystem( ParticleSystemID id, const ParticleSystemTemplate *sysTemplate ) : m_systemID(id), m_localTransform(true), m_isLocalIdentity(true), m_isStopped(false),
		m_isDestroyed(false), m_isSaveable(true), m_skipParentXfrm(false), m_attachedToDrawableID(INVALID_DRAWABLE_ID),
		m_attachedToObjectID(INVALID_ID), m_slaveSystem(NULL), m_masterSystem(NULL)
	{
#include "ma_wind_draw.inc"
	}
	inline ParticleSystemID getSystemID( void ) const { return m_systemID; }
	void setPosition( const Coord3D *pos );
	void setLocalTransform( const Matrix3D *matrix );
	void rotateLocalTransformX( Real x );
	void rotateLocalTransformY( Real y );
	void rotateLocalTransformZ( Real z );
	void setSkipParentXfrm(Bool enable) { m_skipParentXfrm = enable; }
	void attachToDrawable( const Drawable *draw );
	void start( void );
	void stop( void );
	void destroy( void );
	void setSaveable( Bool b );
};
#include "mp_psys_a.inc"
#include "mp_psys_b.inc"

class ParticleSystemManager
{
public:
	std::vector<ParticleSystem *> m_list;
	UnsignedInt m_uniqueSystemID;
	ParticleSystem *createParticleSystem( const ParticleSystemTemplate *sysTemplate, Bool createSlaves = TRUE )
	{
		if (sysTemplate == NULL)
			return NULL;
		m_uniqueSystemID = m_uniqueSystemID + 1;
		ParticleSystem *sys = new ParticleSystem((ParticleSystemID)m_uniqueSystemID, sysTemplate);
		if (createSlaves && sysTemplate->m_slave) {
			ParticleSystem *slave = createParticleSystem(sysTemplate->m_slave, createSlaves);
			if (slave) {
				sys->m_slaveSystem = slave;
				slave->m_masterSystem = sys;
			}
		}
		m_list.push_back(sys);
		return sys;
	}
	ParticleSystem *findParticleSystem( ParticleSystemID id )
	{
		if (id == INVALID_PARTICLE_SYSTEM_ID)
			return NULL;
		for (size_t i = 0; i < m_list.size(); ++i)
			if (m_list[i]->getSystemID() == id)
				return m_list[i];
		return NULL;
	}
	void drop( ParticleSystem *s )
	{
		if (s->m_slaveSystem) { s->m_slaveSystem->m_masterSystem = NULL; s->m_slaveSystem = NULL; }
		if (s->m_masterSystem) { s->m_masterSystem->m_slaveSystem = NULL; s->m_masterSystem = NULL; }
		m_list.erase(std::find(m_list.begin(), m_list.end(), s));
		delete s;
	}
	void clear()
	{
		for (size_t i = 0; i < m_list.size(); ++i) delete m_list[i];
		m_list.clear();
		m_uniqueSystemID = 0;
	}
};
ParticleSystemManager g_psm;
ParticleSystemManager *TheParticleSystemManager = &g_psm;

/* never made here: compiled for setHidden's and reactToTransformChange's branches */
class TerrainTracksRenderObjClass
{
public:
	void addCapEdgeToTrack( Real, Real ) { abort(); }
	void addEdgeToTrack( Real, Real ) { abort(); }
	void setAirborne() { abort(); }
};
class Shadow
{
public:
	void enableShadowRender( Bool ) { abort(); }
	void enableShadowInvisible( Bool ) { abort(); }
	void release( void ) { abort(); }
};

/* ---- the module data and its states ---- */
/* ~~W3DAnimationInfo: the animation by name through the asset manager (Get_HAnim: Add_Ref'd), its idle flag and
** distance~~ (#181 item 7c: cut, W3DModelDraw.h's class and W3DModelDraw.cpp's functions, with m_naturalDurationInMsec,
** -1 until getAnimHandle first finds the animation; and the TransitionSig helpers) */
#include "ma_trans_sig.inc"
#include "ma_anim_info_h.inc"
#include "ma_anim_info.inc"
struct ParticleSysBoneInfo
{
	AsciiString boneName;
	const ParticleSystemTemplate *particleSystemTemplate;
};
#include "lb_pristine.inc"
struct ModelConditionInfo
{
	struct HideShowSubObjInfo { AsciiString subObjName; Bool hide; };
#include "lb_turret_barrel.inc"
	/* W3DModelDraw.h:203-230's members the cuts read (m_hideShowVec empty: no ShowSubObject lines here) */
	Int model;
	AsciiString m_modelName;
	std::vector<HideShowSubObjInfo> m_hideShowVec;
	mutable std::vector<AsciiString> m_publicBones;
	AsciiString m_weaponFireFXBoneName[WEAPONSLOT_COUNT];
	AsciiString m_weaponRecoilBoneName[WEAPONSLOT_COUNT];
	AsciiString m_weaponMuzzleFlashName[WEAPONSLOT_COUNT];
	AsciiString m_weaponProjectileLaunchBoneName[WEAPONSLOT_COUNT];
	std::vector<W3DAnimationInfo> m_animations;
	Int m_mode;
	Int m_flags;
	Real m_animMinSpeedFactor, m_animMaxSpeedFactor;
	std::vector<ParticleSysBoneInfo> m_particleSysBones;
	mutable PristineBoneInfoMap m_pristineBones;
	mutable TurretInfo m_turrets[MAX_TURRETS];
	mutable WeaponBarrelInfoVec m_weaponBarrelInfoVec[WEAPONSLOT_COUNT];
	mutable Bool m_hasRecoilBonesOrMuzzleFlashes[WEAPONSLOT_COUNT];
	mutable Byte m_validStuff;
	/* (#181 item 7c) W3DModelDraw.h:216-222: TransitionKey, WaitForStateToFinishIfPossible (NAMEKEY_INVALID for none),
	** and a TransitionState's signature (NO_TRANSITION for a condition state) */
	NameKeyType m_transitionKey;
	NameKeyType m_allowToFinishKey;
	TransitionSig m_transitionSig;
#include "lb_valid_enum.inc"
	void addPublicBone(const AsciiString& boneName) const;
	void validateStuff(RenderObjClass* robj, Real scale, const std::vector<AsciiString>& extraPublicBones) const;
	void validateCachedBones(RenderObjClass* robj, Real scale) const;
	void validateTurretInfo() const;
	void validateWeaponBarrelInfo() const;
	const Matrix3D* findPristineBone(NameKeyType boneName, Int* boneIndex) const;
	Bool findPristineBonePos(NameKeyType boneName, Coord3D& pos) const;
	void loadAnimations() const;
};
ModelConditionInfo g_states[4];
int g_nstates;
/* TheGlobalData->m_standardPublicBones: the case's */
class GlobalData { public: std::vector<AsciiString> m_standardPublicBones; };
GlobalData g_global;
GlobalData *TheGlobalData = &g_global;
#include "lb_public_bone.inc"
#include "lb_bones.inc"
#include "ma_mf_hidden.inc"
#include "ma_turret_differ.inc"
/* (#181 item 7c) W3DModelDraw.h:272 */
#include "ma_trans_map.inc"
class W3DModelDrawModuleData
{
public:
	/* (#181 item 7c) the TransitionState blocks by signature */
	mutable TransitionMap m_transitionMap;
	Bool m_animationsRequirePower;
	Bool m_particlesAttachedToAnimatedBones;
	AsciiString m_attachToDrawableBone;
	std::vector<AsciiString> m_extraPublicBones;
	mutable Vector3 m_attachToDrawableBoneOffset;
	mutable Bool m_attachToDrawableBoneOffsetValid;
	Real m_initialRecoil, m_maxRecoil, m_recoilDamping, m_recoilSettle;
	const Vector3* getAttachToDrawableBoneOffset(const Drawable* draw) const;
};
W3DModelDrawModuleData g_data;
/* (#181 item 7c) the case's TransitionStates, in the order written: their keys' codes and signature */
struct TransSpec { int src; int dst; TransitionSig sig; };
std::vector<TransSpec> g_trans;
#include "mp_attach_offset.inc"

/* (#181 item 7c) ma_anim_complete.inc's, declared for setModelState's head (upstream defines it before, at 1955; here
** the member functions written in the class come first) */
static Bool isAnimationComplete(RenderObjClass* r);
class W3DModelDraw
{
public:
#include "ma_recoil_info.inc"
#include "mp_tracker.inc"
	typedef std::vector<WeaponRecoilInfo> WeaponRecoilInfoVec;
	typedef std::vector<ParticleSysTrackerType> ParticleSystemIDVec;
	Int m_animationMode;
	Bool m_hideHeadlights;
	Bool m_pauseAnimation;
	const ModelConditionInfo *m_curState;
	const ModelConditionInfo *m_nextState;
	UnsignedInt m_nextStateAnimLoopDuration;
	Int m_hexColor;
	Int m_whichAnimInCurState;
	RenderObjClass *m_renderObject;
	Shadow *m_shadow;
	Bool m_shadowEnabled;
	Shadow *m_terrainDecal;
	TerrainTracksRenderObjClass *m_trackRenderObject;
	WeaponRecoilInfoVec m_weaponRecoilInfoVec[WEAPONSLOT_COUNT];
	Bool m_needRecalcBoneParticleSystems;
	Bool m_fullyObscuredByShroud;
	ParticleSystemIDVec m_particleSystemIDs;
	/* (#181 item 7c) W3DModelDraw.h's m_subObjectVec: showSubObject's list */
	std::vector<ModelConditionInfo::HideShowSubObjInfo> m_subObjectVec;

	W3DModelDraw()
	{
		int i;
#include "ma_ctor.inc"
		setModelState(&g_states[0]);
	}
	~W3DModelDraw() { REF_PTR_RELEASE(m_renderObject); }
	Drawable *getDrawable() const { return &g_drawable; }
	const W3DModelDrawModuleData *getW3DModelDrawModuleData() const { return &g_data; }

	void doDrawModule(const Matrix3D* transformMtx);
	void setPauseAnimation(Bool pauseAnim);
	Real getCurrentAnimFraction() const;
	void adjustAnimation(const ModelConditionInfo* prevState, Real prevAnimFraction);
	Bool setCurAnimDurationInMsec(Real desiredDurationInMsec);
	Real getCurAnimDistanceCovered() const;
	void adjustAnimSpeedToMovementSpeed();
	void adjustTransformMtx(Matrix3D& mtx) const;
	void doStartOrStopParticleSys();
	void setHidden(Bool hidden);
	void setFullyObscuredByShroud(Bool fullyObscured);
	void stopClientParticleSystems();
	void recalcBonesForClientParticleSystems();
	Bool updateBonesForClientParticleSystems();
	void reactToTransformChange(const Matrix3D* oldMtx, const Coord3D* oldPos, Real oldAngle);
	void handleClientRecoil();
	/* (#181 item 7b) cut, no longer a stand-in */
	void handleClientTurretPositioning();
	void nukeCurrentRender(Matrix3D* xform);
	void hideAllHeadlights(Bool hide);
	static void hideAllMuzzleFlashes(const ModelConditionInfo* state, RenderObjClass* renderObject);
	void rebuildWeaponRecoilInfo(const ModelConditionInfo* state);
	Bool handleWeaponFireFX(WeaponSlotType wslot, Int specificBarrelToUse, const FXList* fxl, Real weaponSpeed, const Coord3D* victimPos, Real damageRadius);
	/* W3DModelDraw.h:435 */
	virtual void onRenderObjRecreated(void){};
	/* ~~doHideShowSubObjs (W3DModelDraw.cpp:2342-2378): the states have no ShowSubObject/HideSubObject lines and nothing
	** calls showSubObject here (m_subObjectVec empty): nothing to do~~ (#181 item 7c: cut, with showSubObject and
	** updateSubObjects) */
	void doHideShowSubObjs(const std::vector<ModelConditionInfo::HideShowSubObjInfo>* vec);
	void showSubObject(const AsciiString& name, Bool show);
	void updateSubObjects();
	/* ~~reached only with a next state, which no state here makes (no transition keys)~~ (#181 item 7c: cut, with
	** setAnimationCompletionTime and findTransitionForSig) */
	void setAnimationLoopDuration(UnsignedInt numFrames);
	void setAnimationCompletionTime(UnsignedInt numFrames);
	const ModelConditionInfo* findTransitionForSig(TransitionSig sig) const;

	/* setModelState: ~~the early return for the current state (no next state: the states have no transition keys), then
	** the cuts:~~ (#181 item 7c) the cuts: the early returns (the current or pending state; allow-to-finish) and the
	** transition lookup (ma_sms_pre), the head, the middle to the first branch's doHideShowSubObjs (hideAllMuzzleFlashes, the model test with
	** turretNamesDiffer, nukeCurrentRender, Create_Render_Obj, validateStuff, rebuildWeaponRecoilInfo), the first branch's
	** end (W3DModelDraw.cpp:3047-3157, not cut: the garrison flags (debug), the tracks (no TrackMarks), the shadow (no
	** shadow type), the collision types and the scene's Add_Render_Object (not read here), then as cut: hidden for a
	** hidden drawable, Set_Transform(transform), onRenderObjRecreated), the else branch (cut), hideAllHeadlights (cut),
	** the tail (cut) */
	void setModelState( const ModelConditionInfo *newState )
	{
#include "ma_sms_pre.inc"
#include "ma_sms_head.inc"
#include "ma_sms_mid.inc"
			if( m_renderObject )
			{
				if (draw->isDrawableEffectivelyHidden())
				{
					m_renderObject->Set_Hidden(TRUE);
					if (m_shadow)
						m_shadow->enableShadowRender(FALSE);
					m_shadowEnabled = FALSE;
				}
				m_renderObject->Set_Transform(transform);
				onRenderObjRecreated();
			}
		}
		else
		{
#include "ma_sms_else.inc"
		}
		hideAllHeadlights(m_hideHeadlights);
#include "ma_sms_tail.inc"
	}

	void state()
	{
		int si = -1;
		for (int i = 0; i < 4; ++i)
			if (m_curState == &g_states[i]) si = i;
		rec({100, m_needRecalcBoneParticleSystems ? 1 : 0, m_fullyObscuredByShroud ? 1 : 0, m_whichAnimInCurState, m_pauseAnimation ? 1 : 0,
			m_animationMode, si, (int)m_particleSystemIDs.size()});
		for (size_t i = 0; i < m_particleSystemIDs.size(); ++i) rec({(int)m_particleSystemIDs[i].id, m_particleSystemIDs[i].boneIndex});
		std::vector<ParticleSystem *> l = g_psm.m_list;
		std::sort(l.begin(), l.end(), [](ParticleSystem *a, ParticleSystem *b) { return a->m_systemID < b->m_systemID; });
		rec({(int)l.size()});
		for (size_t i = 0; i < l.size(); ++i) {
			ParticleSystem *s = l[i];
			rec({(int)s->m_systemID, (int)s->m_attachedToDrawableID, s->m_isSaveable ? 1 : 0, s->m_isStopped ? 1 : 0, s->m_isDestroyed ? 1 : 0,
				s->m_skipParentXfrm ? 1 : 0, s->m_isLocalIdentity ? 1 : 0, s->m_slaveSystem ? (int)s->m_slaveSystem->m_systemID : 0,
				s->m_masterSystem ? (int)s->m_masterSystem->m_systemID : 0});
			recM(s->m_localTransform);
		}
		RenderObjClass *r = m_renderObject;
		rec({r ? 1 : 0});
		if (r) {
			rec({r->CurMotionMode, r->IsTreeValid ? 1 : 0});
			if (r->CurMotionMode == Animatable3DObjClass::SINGLE_ANIM) {
				int ai = -1;
				for (size_t i = 0; i < g_anims.size(); ++i)
					if (g_anims[i] == r->ModeAnim.Motion) ai = (int)i;
				rec({ai, bits(r->ModeAnim.Frame), bits(r->ModeAnim.PrevFrame), (int)r->ModeAnim.LastSyncTime, bits(r->ModeAnim.animDirection),
					bits(r->ModeAnim.frameRateMultiplier), r->ModeAnim.AnimMode, r->Is_Animation_Complete() ? 1 : 0});
			} else {
				rec({-1, 0, 0, 0, 0, 0, 0, 0});
			}
			rec({r->Is_Hidden()});
			recM(r->Transform);
			rec({bits(r->ObjectScale), r->HTree->Num_Pivots()});
			for (int p = 0; p < r->HTree->Num_Pivots(); ++p) recM(r->HTree->Get_Transform(p));
			/* (#181 item 7b) each sub-object's Is_Hidden */
			rec({r->Get_Num_Sub_Objects()});
			for (int k = 0; k < r->Get_Num_Sub_Objects(); ++k) rec({r->Subs[k]->Is_Hidden()});
		}
		/* (#181 item 7b) the recoils, then each state's caches: m_validStuff's PRISTINE_BONES_VALID, TURRETS_VALID and
		** BARRELS_VALID; the pristine bones by name (length, characters, bone index, matrix); the turrets' bones; the
		** barrels (recoil, FX, muzzle-flash bone, projectile offset); m_hasRecoilBonesOrMuzzleFlashes */
		for (int w = 0; w < WEAPONSLOT_COUNT; ++w) {
			rec({(int)m_weaponRecoilInfoVec[w].size()});
			for (size_t i = 0; i < m_weaponRecoilInfoVec[w].size(); ++i)
				rec({(int)m_weaponRecoilInfoVec[w][i].m_state, bits(m_weaponRecoilInfoVec[w][i].m_shift), bits(m_weaponRecoilInfoVec[w][i].m_recoilRate)});
		}
		for (int i = 0; i < g_nstates; ++i) caches(g_states[i]);
		/* (#181 item 7c) the current and the pending state (state_code), m_nextStateAnimLoopDuration, the showSubObject
		** list (name length, characters, hide), then each of the case's TransitionStates' caches (its map entry) */
		rec({state_code(m_curState), state_code(m_nextState), (int)m_nextStateAnimLoopDuration, (int)m_subObjectVec.size()});
		for (size_t i = 0; i < m_subObjectVec.size(); ++i) {
			const std::string &n = m_subObjectVec[i].subObjName.s;
			rec({(int)n.size()});
			for (size_t c = 0; c < n.size(); ++c) rec({(int)(unsigned char)n[c]});
			rec({m_subObjectVec[i].hide ? 1 : 0});
		}
		for (size_t t = 0; t < g_trans.size(); ++t) caches(g_data.m_transitionMap.find(g_trans[t].sig)->second);
		unsigned int seed[6];
		ref_get_state(1, seed);
		for (int i = 0; i < 6; ++i) rec({(int)seed[i]});
	}

	/* (#181 item 7c) a state as recorded: -1 none, a condition state's index, a TransitionState 10 + 3 x its first key's
	** code + its second's */
	static int state_code(const ModelConditionInfo *st)
	{
		if (st == NULL) return -1;
		for (int i = 0; i < 4; ++i)
			if (st == &g_states[i]) return i;
		for (size_t t = 0; t < g_trans.size(); ++t)
			if (st == &g_data.m_transitionMap.find(g_trans[t].sig)->second) return 10 + 3 * g_trans[t].src + g_trans[t].dst;
		return -2;
	}

	/* a state's caches (see the header) */
	static void caches(const ModelConditionInfo &st)
	{
		{
			rec({st.m_validStuff & (ModelConditionInfo::PRISTINE_BONES_VALID | ModelConditionInfo::TURRETS_VALID | ModelConditionInfo::BARRELS_VALID)});
			std::vector<std::pair<std::string, PristineBoneInfo> > bones;
			for (PristineBoneInfoMap::const_iterator it = st.m_pristineBones.begin(); it != st.m_pristineBones.end(); ++it) {
				std::string name;
				for (std::map<std::string, int>::const_iterator k = g_keys.begin(); k != g_keys.end(); ++k)
					if (k->second == (int)it->first) name = k->first;
				bones.push_back(std::make_pair(name, it->second));
			}
			std::sort(bones.begin(), bones.end(), [](const std::pair<std::string, PristineBoneInfo> &x, const std::pair<std::string, PristineBoneInfo> &y) { return x.first < y.first; });
			rec({(int)bones.size()});
			for (size_t b = 0; b < bones.size(); ++b) {
				rec({(int)bones[b].first.size()});
				for (size_t c = 0; c < bones[b].first.size(); ++c) rec({(int)(unsigned char)bones[b].first[c]});
				rec({bones[b].second.boneIndex});
				recM(bones[b].second.mtx);
			}
			for (int t = 0; t < MAX_TURRETS; ++t) rec({st.m_turrets[t].m_turretAngleBone, st.m_turrets[t].m_turretPitchBone});
			for (int w = 0; w < WEAPONSLOT_COUNT; ++w) {
				rec({(int)st.m_weaponBarrelInfoVec[w].size()});
				for (size_t b = 0; b < st.m_weaponBarrelInfoVec[w].size(); ++b) {
					const ModelConditionInfo::WeaponBarrelInfo &bi = st.m_weaponBarrelInfoVec[w][b];
					rec({bi.m_recoilBone, bi.m_fxBone, bi.m_muzzleFlashBone});
					recM(bi.m_projectileOffsetMtx);
				}
				rec({st.m_hasRecoilBonesOrMuzzleFlashes[w] ? 1 : 0});
			}
		}
	}
};
#include "ma_anim_complete.inc"
#include "mp_adjust.inc"
#include "ma_draw.inc"
#include "ma_anim.inc"
#include "ma_recoil.inc"
#include "ma_turret.inc"
#include "ma_nuke.inc"
#include "ma_headlights.inc"
#include "ma_rebuild_recoil.inc"
#include "ma_fire_fx.inc"
/* (#181 item 7c) */
#include "ma_find_trans.inc"
#include "ma_hide_show_bone.inc"
#include "ma_hide_show.inc"
#include "ma_show_sub.inc"
#include "ma_update_subs.inc"
#include "ma_durations.inc"
#include "mp_start_stop.inc"
#include "mp_shroud.inc"
#include "mp_stop.inc"
#include "mp_bones.inc"
#include "mp_react.inc"
#include "ma_pause.inc"

class TestFile : public FileClass
{
public:
	TestFile(const char *data, int len) : m_theFile(ref_w3d_ramfile_open(data, len)) {}
	~TestFile() { ref_w3d_ramfile_close(m_theFile); }
	virtual char const *File_Name(void) const { return "test.w3d"; }
	virtual char const *Set_Name(char const *) { return "test.w3d"; }
	virtual int Create(void) { abort(); }
	virtual int Delete(void) { abort(); }
	virtual bool Is_Available(int = false) { return true; }
	virtual bool Is_Open(void) const { return true; }
	virtual int Open(char const *, int = READ) { return 1; }
	virtual int Open(int = READ) { return 1; }
	virtual int Read(void *buffer, int len) { return ref_w3d_ramfile_read(m_theFile, buffer, len); }
	virtual int Seek(int pos, int dir = SEEK_CUR)
	{
		int mode = 1;
		switch (dir) {
		default:
		case SEEK_CUR: mode = 1; break;
		case SEEK_SET: mode = 0; break;
		case SEEK_END: mode = 2; break;
		}
		return ref_w3d_ramfile_seek(m_theFile, pos, mode);
	}
	virtual int Size(void) { return ref_w3d_ramfile_size(m_theFile); }
	virtual int Write(void const *, int) { abort(); }
	virtual void Close(void) {}
private:
	void *m_theFile;
};

/* the file: the hierarchy first, then raw animations (anything else aborts: the test makes only these); returns false when
** a loader refuses */
bool load(const char *data, int len)
{
	TestFile file(data, len);
	ChunkLoadClass cload(&file);
	g_w3d_htree_hook = find_tree;
	bool ok = true;
	while (cload.Open_Chunk()) {
		switch (cload.Cur_Chunk_ID()) {
		case W3D_CHUNK_HIERARCHY: {
			HTreeClass *t = new HTreeClass;
			if (t->Load_W3D(cload) == HTreeClass::OK && g_tree == NULL) g_tree = t; else { delete t; ok = false; }
			break;
		}
		case W3D_CHUNK_ANIMATION: {
			HRawAnimClass *a = new HRawAnimClass;
			if (a->Load_W3D(cload) == HRawAnimClass::OK) g_anims.push_back(a); else { a->Release_Ref(); ok = false; }
			break;
		}
		default:
			abort();
		}
		cload.Close_Chunk();
	}
	g_w3d_htree_hook = NULL;
	return ok && g_tree != NULL;
}

void free_all()
{
	for (size_t i = g_anims.size(); i-- > 0;) g_anims[i]->Release_Ref();
	g_anims.clear();
	delete g_tree;
	g_tree = NULL;
	g_psm.clear();
}

const char *const BONE_NAMES[] = { "B1", "b2", "B3", "b4", "B5", "RootTransform", "nosuchbone" };
/* (#181 item 7b) the turrets', weapons' and public bones' names (as the INI writes them; ~~9~~ (#181 item 7c) 11 is "no
** line"); (#181 item 7c) GUN and DOOR added: sub-objects' names (SUB_NAMES), no pivot's */
const char *const WEAPON_NAMES[] = { "TURRET", "PITCH", "BARREL", "MUZZLE", "FX", "B1", "B3", "NOSUCH", "None", "GUN", "DOOR" };
const int NO_LINE = 11;

/* (#181 item 7b) the INI parse of a turret or weapon bone (parseBoneNameKey, parseWeaponBoneName, W3DModelDraw.cpp:
** 1335-1397): lower-cased, `None` an invalid key (cleared for a weapon), addPublicBone of it */
NameKeyType parse_bone_key(ModelConditionInfo &st, int code)
{
	AsciiString tmp = WEAPON_NAMES[code];
	tmp.toLower();
	st.addPublicBone(tmp);
	if (tmp.isEmpty() || tmp.isNone())
		return NAMEKEY_INVALID;
	return NAMEKEY(tmp.str());
}
void parse_weapon_name(ModelConditionInfo &st, AsciiString &dst, int code)
{
	dst = WEAPON_NAMES[code];
	dst.toLower();
	if (dst.isNone())
		dst.clear();
	st.addPublicBone(dst);
}

/* (#181 item 7c) the sub-object names of ShowSubObject/HideSubObject lines and showSubObject calls (as written; 9: no
** token, or an empty name) */
const char *const HS_NAMES[] = { "sub0", "SUB1", "skel.sub2", "HEADLIGHT3", "gun", "GUN01", "door", "nosuch", "None", "" };
const int NO_NAME = 9;
const char *const KEY_NAMES[] = { "k0", "k1", "k2" };
/* parseShowHideSubObject (W3DModelDraw.cpp:1270-1305) of a line of one or two names: lower-cased; a first `None` clears
** the list; each name already listed (case ignored) has its flag set, else is added */
void parse_hide_show(std::vector<ModelConditionInfo::HideShowSubObjInfo> &vec, bool hide, int a, int b)
{
	const char *tokens[2] = { HS_NAMES[a], HS_NAMES[b] };
	int k = 0;
	AsciiString subObjName = tokens[k++];
	subObjName.toLower();
	if (subObjName.isNone()) {
		vec.clear();
		return;
	}
	while (subObjName.isNotEmpty()) {
		Bool found = false;
		for (size_t i = 0; i < vec.size(); ++i)
			if (strcasecmp(vec[i].subObjName.str(), subObjName.str()) == 0) {
				vec[i].hide = hide;
				found = true;
			}
		if (!found) {
			ModelConditionInfo::HideShowSubObjInfo info;
			info.subObjName = subObjName;
			info.hide = hide;
			vec.push_back(info);
		}
		subObjName = k < 2 ? tokens[k++] : "";
		subObjName.toLower();
	}
}
/* a state's ShowSubObject/HideSubObject lines: a count (0..2), then 2 x (hide, name, name) */
void read_hide_show(std::vector<ModelConditionInfo::HideShowSubObjInfo> &vec, const int *in, int &k)
{
	int n = in[k++];
	for (int i = 0; i < 2; ++i) {
		int hide = in[k++], a = in[k++], b = in[k++];
		if (i < n) parse_hide_show(vec, hide != 0, a, b);
	}
}
NameKeyType key_of(int code) { return code < 0 ? NAMEKEY_INVALID : NAMEKEY(KEY_NAMES[code]); }

} /* namespace mda */

extern "C" {

int ref_model_draw_anim_seq(const char *file, int flen, const int *in, int nin, int *out, int cap)
{
	using namespace mda;
	g_out.clear();
	g_psm.clear();
	g_keys.clear();
	g_logic_time = FALSE;
	ref_wwmath_init_tables();
	ref_w3d_set_float_to_long_mode(1);
	if (!load(file, flen)) {
		free_all();
		return -1;
	}
	int k = 0;
	ref_init_random((unsigned int)in[k++]);
	g_sync_time = (unsigned int)in[k++];
	g_drawable.m_id = (DrawableID)in[k++];
	g_drawable.m_scale = real(in[k++]);
	g_drawable.m_hidden = FALSE;
	g_drawable.m_status = 0;
	g_drawable.m_transform.Make_Identity();
	g_drawable.m_shouldAnimate = TRUE;
	g_hasObject = FALSE;
	g_hasAI = FALSE;
	for (int t = 0; t < MAX_TURRETS; ++t) {
		g_ai.m_has[t] = FALSE;
		g_ai.m_angle[t] = g_ai.m_pitch[t] = 0.0f;
	}
	g_object.m_physics.m_speed = 0.0f;
	g_data.m_particlesAttachedToAnimatedBones = in[k++] != 0;
	g_data.m_animationsRequirePower = in[k++] != 0;
	g_data.m_attachToDrawableBone = "";
	g_data.m_attachToDrawableBoneOffsetValid = false;
	g_data.m_initialRecoil = g_data.m_maxRecoil = g_data.m_recoilDamping = g_data.m_recoilSettle = 0.0f;
	for (int t = 0; t < 4; ++t) {
		int sl = in[k++];
		g_templates[t].m_slave = sl < 0 ? NULL : &g_templates[sl];
	}
	int nstates = in[k++];
	g_nstates = nstates;
	for (int si = 0; si < 4; ++si) {
		ModelConditionInfo &st = g_states[si];
		st.model = in[k++];
		st.m_mode = in[k++];
		st.m_flags = in[k++];
		st.m_animMinSpeedFactor = real(in[k++]);
		st.m_animMaxSpeedFactor = real(in[k++]);
		st.m_validStuff = 0;
		st.m_pristineBones.clear();
		for (int w = 0; w < WEAPONSLOT_COUNT; ++w) {
			st.m_weaponBarrelInfoVec[w].clear();
			st.m_hasRecoilBonesOrMuzzleFlashes[w] = FALSE;
		}
		int na = in[k++];
		st.m_animations.clear();
		for (int a = 0; a < 3; ++a) {
			int ai = in[k++];
			int idle = in[k++];
			Real dist = real(in[k++]);
			if (a >= na) continue;
			/* parseAnimation (W3DModelDraw.cpp:1233-1267): the name lower-cased */
			AsciiString animName = ai < (int)g_anims.size() ? g_anims[ai]->Get_Name() : "SKEL.NOSUCH";
			animName.toLower();
			W3DAnimationInfo info(animName, idle != 0, dist);
			st.m_animations.push_back(info);
		}
		/* parseConditionState (W3DModelDraw.cpp:1470-1480): a ConditionState starts as a copy of the default state; its
		** own Animation lines replace the copied ones (parseAnimation's ANIMS_COPIED_FROM_DEFAULT_STATE), its
		** ParticleSysBone lines add to the copied ones */
		if (si > 0 && na == 0) st.m_animations = g_states[0].m_animations;
		int nb = in[k++];
		st.m_particleSysBones.clear();
		if (si > 0) st.m_particleSysBones = g_states[0].m_particleSysBones;
		for (int b = 0; b < 3; ++b) {
			int bone = in[k++];
			int tmpl = in[k++];
			if (b >= nb) continue;
			ParticleSysBoneInfo info;
			info.boneName = BONE_NAMES[bone];
			info.boneName.toLower();
			info.particleSystemTemplate = tmpl < 4 ? &g_templates[tmpl] : NULL;
			st.m_particleSysBones.push_back(info);
		}
		/* the Model line (parseAsciiStringLC; `None` cleared, 1639-1641) */
		st.m_modelName = st.model < 3 ? MODEL_NAMES[st.model] : "";
		st.m_modelName.toLower();
	}
	/* (#181 item 7b) the module's recoil numbers (parseVelocityReal for the two speeds), the models' sub-objects, the
	** global data's standard public bones, the module's ExtraPublicBone names, then each state's turrets and weapon
	** bones (a ConditionState copies the default state's, and its own lines replace them: the turrets all four lines,
	** a weapon bone its own line; the public bones the copy's and its own) */
	g_data.m_initialRecoil = ConvertVelocityInSecsToFrames(real(in[k++]));
	g_data.m_maxRecoil = real(in[k++]);
	g_data.m_recoilDamping = real(in[k++]);
	g_data.m_recoilSettle = ConvertVelocityInSecsToFrames(real(in[k++]));
	g_subs.clear();
	{
		int ns = in[k++];
		for (int i = 0; i < 4; ++i) {
			SubSpec sp;
			sp.bone = in[k++];
			sp.hidden = in[k++] != 0;
			sp.headlight = in[k++] != 0;
			if (i < ns) g_subs.push_back(sp);
		}
	}
	g_global.m_standardPublicBones.clear();
	{
		int n = in[k++];
		for (int i = 0; i < 2; ++i) {
			int c = in[k++];
			if (i < n) g_global.m_standardPublicBones.push_back(AsciiString(WEAPON_NAMES[c]));
		}
	}
	g_data.m_extraPublicBones.clear();
	{
		int n = in[k++];
		for (int i = 0; i < 2; ++i) {
			int c = in[k++];
			if (i < n) g_data.m_extraPublicBones.push_back(AsciiString(WEAPON_NAMES[c]));
		}
	}
	for (int si = 0; si < 4; ++si) {
		ModelConditionInfo &st = g_states[si];
		st.m_publicBones.clear();
		for (int t = 0; t < MAX_TURRETS; ++t) st.m_turrets[t].clear();
		for (int w = 0; w < WEAPONSLOT_COUNT; ++w) {
			st.m_weaponFireFXBoneName[w].clear();
			st.m_weaponRecoilBoneName[w].clear();
			st.m_weaponMuzzleFlashName[w].clear();
			st.m_weaponProjectileLaunchBoneName[w].clear();
		}
		if (si > 0) {
			const ModelConditionInfo &d = g_states[0];
			st.m_publicBones = d.m_publicBones;
			for (int t = 0; t < MAX_TURRETS; ++t) st.m_turrets[t] = d.m_turrets[t];
			for (int w = 0; w < WEAPONSLOT_COUNT; ++w) {
				st.m_weaponFireFXBoneName[w] = d.m_weaponFireFXBoneName[w];
				st.m_weaponRecoilBoneName[w] = d.m_weaponRecoilBoneName[w];
				st.m_weaponMuzzleFlashName[w] = d.m_weaponMuzzleFlashName[w];
				st.m_weaponProjectileLaunchBoneName[w] = d.m_weaponProjectileLaunchBoneName[w];
			}
		}
		int ownTurrets = in[k++];
		for (int t = 0; t < MAX_TURRETS; ++t) {
			int ac = in[k++], pc = in[k++];
			Real aa = real(in[k++]), pa = real(in[k++]);
			if (si > 0 && !ownTurrets) continue;
			/* `Turret`/`AltTurret`, `TurretArtAngle` (INI::parseAngleReal, INI.cpp:584-592), `TurretPitch`,
			** `TurretArtPitch` */
			const Real RADS_PER_DEGREE = PI / 180.0f;
			st.m_turrets[t].m_turretAngleNameKey = parse_bone_key(st, ac);
			st.m_turrets[t].m_turretArtAngle = aa * RADS_PER_DEGREE;
			st.m_turrets[t].m_turretPitchNameKey = parse_bone_key(st, pc);
			st.m_turrets[t].m_turretArtPitch = pa * RADS_PER_DEGREE;
		}
		for (int w = 0; w < WEAPONSLOT_COUNT; ++w) {
			int c[4];
			for (int f = 0; f < 4; ++f) c[f] = in[k++];
			AsciiString *dst[4] = { &st.m_weaponFireFXBoneName[w], &st.m_weaponRecoilBoneName[w], &st.m_weaponMuzzleFlashName[w], &st.m_weaponProjectileLaunchBoneName[w] };
			for (int f = 0; f < 4; ++f)
				if (c[f] != NO_LINE) parse_weapon_name(st, *dst[f], c[f]);
		}
	}
	/* (#181 item 7c) the sub-objects' name codes; each state's TransitionKey and WaitForStateToFinishIfPossible (a key
	** code, -1 no line) and its ShowSubObject/HideSubObject lines (a ConditionState copies the default state's keys and
	** list, and its own lines replace a key or apply to the copied list); then the TransitionStates (a count 0..3, each
	** its two keys, model, mode, flags, speeds, 0..2 Animation lines and its show/hide lines: parseConditionState's
	** PARSE_TRANSITION copies the default state, clears both keys and sets the signature, 1491-1515) */
	for (int i = 0; i < 4; ++i) {
		int code = in[k++];
		if (i < (int)g_subs.size()) g_subs[i].name = code;
	}
	for (int si = 0; si < 4; ++si) {
		ModelConditionInfo &st = g_states[si];
		st.m_transitionSig = NO_TRANSITION;
		st.m_transitionKey = si > 0 ? g_states[0].m_transitionKey : NAMEKEY_INVALID;
		st.m_allowToFinishKey = si > 0 ? g_states[0].m_allowToFinishKey : NAMEKEY_INVALID;
		st.m_hideShowVec.clear();
		if (si > 0) st.m_hideShowVec = g_states[0].m_hideShowVec;
		int tk = in[k++], allow = in[k++];
		if (tk >= 0) st.m_transitionKey = key_of(tk);
		if (allow >= 0) st.m_allowToFinishKey = key_of(allow);
		read_hide_show(st.m_hideShowVec, in, k);
	}
	g_data.m_transitionMap.clear();
	g_trans.clear();
	{
		int nt = in[k++];
		for (int t = 0; t < 3; ++t) {
			int src = in[k++], dst = in[k++];
			ModelConditionInfo st = g_states[0];
			st.m_transitionKey = NAMEKEY_INVALID;
			st.m_allowToFinishKey = NAMEKEY_INVALID;
			st.model = in[k++];
			st.m_mode = in[k++];
			st.m_flags = in[k++];
			st.m_animMinSpeedFactor = real(in[k++]);
			st.m_animMaxSpeedFactor = real(in[k++]);
			int na = in[k++];
			if (na > 0) st.m_animations.clear();
			for (int a = 0; a < 2; ++a) {
				int ai = in[k++];
				Real dist = real(in[k++]);
				if (a >= na) continue;
				AsciiString animName = ai < (int)g_anims.size() ? g_anims[ai]->Get_Name() : "SKEL.NOSUCH";
				animName.toLower();
				st.m_animations.push_back(W3DAnimationInfo(animName, FALSE, dist));
			}
			st.m_modelName = st.model < 3 ? MODEL_NAMES[st.model] : "";
			st.m_modelName.toLower();
			read_hide_show(st.m_hideShowVec, in, k);
			if (t >= nt) continue;
			TransSpec ts;
			ts.src = src;
			ts.dst = dst;
			ts.sig = buildTransitionSig(key_of(src), key_of(dst));
			st.m_transitionSig = ts.sig;
			g_data.m_transitionMap[ts.sig] = st;
			g_trans.push_back(ts);
		}
	}
	W3DModelDraw *draw = new W3DModelDraw();
	draw->state();
	for (int st = k; st + 13 < nin; st += 14) {
		const int *a = in + st;
		switch (a[0]) {
		case 0:
		case 13: {
			/* getCurrentAnimFraction (W3DModelDraw.cpp:2127-2148) reads its uninitialised `framenum` and `numFrames`
			** when Peek_Animation_And_Info answers NULL: a current state with a maintain-frame flag whose render object
			** plays no animation. Undefined upstream: the case ends there, recorded as [700] */
			const ModelConditionInfo *ns = &g_states[(unsigned)a[1] % (unsigned)nstates];
			float fr, mult;
			int mo, nf;
			/* (#181 item 7c) only when setModelState gets past its early returns (the current or pending state; a state
			** allowed to wait for the current one to finish) */
			const ModelConditionInfo *cs = draw->m_curState;
			Bool reaches = TRUE;
			if (cs != NULL) {
				if ((cs == ns && draw->m_nextState == NULL) || draw->m_nextState == ns)
					reaches = FALSE;
				else if (ns != cs && ns->m_allowToFinishKey != NAMEKEY_INVALID && ns->m_allowToFinishKey == cs->m_transitionKey &&
					draw->m_renderObject && !isAnimationComplete(draw->m_renderObject))
					reaches = FALSE;
			}
			if (reaches && cs != NULL && isAnyMaintainFrameFlagSet(cs->m_flags) &&
				draw->m_renderObject != NULL && draw->m_renderObject->Peek_Animation_And_Info(fr, nf, mo, mult) == NULL) {
				rec({700});
				goto done;
			}
			/* (#181 item 7b) 13: inside the logic's update (isValidTimeToCalcLogicStuff true) */
			g_logic_time = a[0] == 13;
			draw->setModelState(ns);
			g_logic_time = FALSE;
			break;
		}
		case 1: {
			g_drawable.m_transform = matrix(a + 2);
			Matrix3D transformMtx = *g_drawable.getTransformMatrix();
			draw->doDrawModule(&transformMtx);
			break;
		}
		case 2: g_sync_time += (unsigned int)a[1]; break;
		case 3:
			if (draw->m_renderObject) {
				mda::RenderInfoClass rinfo;
				draw->m_renderObject->Render(rinfo);
			}
			break;
		case 4:
			g_drawable.m_hidden = a[1] != 0;
			draw->setHidden(a[1] != 0);
			break;
		case 5: g_drawable.m_shouldAnimate = a[1] != 0; break;
		case 6:
			g_hasObject = a[1] != 0;
			g_object.m_physics.m_speed = real(a[2]);
			break;
		case 7:
			/* handleClientRecoil's (and the turret's) calls on a bone, or a release */
			if (draw->m_renderObject) {
				int n = draw->m_renderObject->HTree->Num_Pivots();
				int bone = (int)((unsigned)a[1] / 2 % (unsigned)n);
				if (a[1] & 1) {
					draw->m_renderObject->Release_Bone(bone);
				} else {
					draw->m_renderObject->Capture_Bone(bone);
					draw->m_renderObject->Control_Bone(bone, matrix(a + 2));
				}
			}
			break;
		case 8:
			g_drawable.m_transform = matrix(a + 2);
			draw->reactToTransformChange(NULL, NULL, 0.0f);
			draw->updateBonesForClientParticleSystems();
			break;
		case 9:
			if (!g_psm.m_list.empty()) {
				std::vector<ParticleSystem *> l = g_psm.m_list;
				std::sort(l.begin(), l.end(), [](ParticleSystem *x, ParticleSystem *y) { return x->m_systemID < y->m_systemID; });
				g_psm.drop(l[(unsigned)a[1] % l.size()]);
			}
			break;
		case 10: g_drawable.m_scale = real(a[2]); break;
		case 11: draw->setFullyObscuredByShroud(a[1] != 0); break;
		case 12:
			if (draw->m_renderObject) {
				int n = draw->m_renderObject->HTree->Num_Pivots();
				Matrix3D m = draw->m_renderObject->Get_Bone_Transform((int)((unsigned)a[1] % (unsigned)n));
				rec({600});
				recM(m);
			}
			break;
		case 14: {
			/* (#181 item 7b) the logic's fire (Drawable::handleWeaponFireFX's call): slot a1 % 3, barrel word 2, an FX list
			** when word 3; recorded [610, handled] after doFXPos's records. A barrel equal to the count is read one past
			** the end upstream (`specificBarrelToUse > wbvec.size()`): undefined, the case ends, recorded as [701] */
			WeaponSlotType wslot = (WeaponSlotType)((unsigned)a[1] % 3u);
			Int barrel = a[2];
			const ModelConditionInfo *cs = draw->m_curState;
			if (cs && (cs->m_validStuff & ModelConditionInfo::BARRELS_VALID) && !cs->m_weaponBarrelInfoVec[wslot].empty() &&
				barrel == (Int)cs->m_weaponBarrelInfoVec[wslot].size()) {
				rec({701});
				goto done;
			}
			Bool handled = draw->handleWeaponFireFX(wslot, barrel, a[3] ? &g_fxl : NULL, 0.0f, NULL, 0.0f);
			rec({610, handled ? 1 : 0});
			break;
		}
		case 15:
			/* (#181 item 7b) the object's AI and its turrets (word 2: an AI; per turret, present, angle, pitch) */
			g_hasAI = a[2] != 0;
			for (int t = 0; t < MAX_TURRETS; ++t) {
				g_ai.m_has[t] = a[3 + 3 * t] != 0;
				g_ai.m_angle[t] = real(a[4 + 3 * t]);
				g_ai.m_pitch[t] = real(a[5 + 3 * t]);
			}
			break;
		/* (#181 item 7c) the logic's Drawable::setAnimationLoopDuration and setAnimationCompletionTime (frames a1); a
		** showSubObject(name a1 % 10, lower-cased when word 3, show word 2) as SubObjectsUpgrade and DeliverPayloadAIUpdate
		** call it; updateSubObjects */
		case 16: draw->setAnimationLoopDuration((UnsignedInt)a[1]); break;
		case 17: draw->setAnimationCompletionTime((UnsignedInt)a[1]); break;
		case 18: {
			AsciiString name = HS_NAMES[(unsigned)a[1] % 10u];
			if (a[3]) name.toLower();
			draw->showSubObject(name, a[2] != 0);
			break;
		}
		case 19: draw->updateSubObjects(); break;
		default: abort();
		}
		draw->state();
	}
done:
	delete draw;
	free_all();
	int n = (int)g_out.size();
	if (n > cap) n = cap;
	if (n > 0) memcpy(out, &g_out[0], n * sizeof(int));
	return (int)g_out.size();
}

} /* extern "C" */
