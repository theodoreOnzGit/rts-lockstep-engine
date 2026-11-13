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
**   ma_test_flag.inc      W3DModelDraw.cpp, `testFlagBit` (433-436);
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
** particle system manager (wrap_model_draw_particles.cpp's), and setModelState's middle (the render object made again
** when the model differs or turretNamesDiffer, cut, says so: always, outside a logic time, `nukeCurrentRender`'s `m_pauseAnimation = false` and transform, `Set_Hidden(TRUE)` for a
** hidden drawable, `Set_Transform(transform)`; the rest of the middle has nothing to do here). The draw is outside a logic
** time (`isValidTimeToCalcLogicStuff()` false, as the port's client draws): validateStuff then leaves the pristine bones,
** the turrets and the barrels not valid (W3DModelDraw.cpp:592-598, 721-725 and validateTurretInfo's same test), so
** handleClientTurretPositioning and handleClientRecoil return at their first test (the latter is the cut; the former a
** stand-in that does the same) and no barrel is ever fired. The tracks, shadow and decal stay NULL.
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
**     of the render object (b = a1 % its pivot count; recorded as [600, the 12 words]).
**   The constructor runs setModelState(state 0) first. A setModelState that would read getCurrentAnimFraction's
**   uninitialised locals (a current state with a maintain-frame flag and a render object in BASE_POSE: undefined upstream)
**   ends the case instead, recorded as [700] (no state after it). Records: after the constructor and after each step the state:
**   [100, needRecalc, fullyObscured, whichAnim, pauseAnimation, animationMode, current state index, tracker count, then
**   (id, boneIndex) each, system count, per system in id order (id, attached drawable, saveable, stopped, destroyed,
**   skipParentXfrm, isLocalIdentity, slave id, master id, the local transform's 12 words), render object present, and
**   with one: CurMotionMode, IsTreeValid, in SINGLE_ANIM the motion's index (else -1), Frame, PrevFrame, LastSyncTime,
**   animDirection, frameRateMultiplier, AnimMode, Is_Animation_Complete (zeros outside SINGLE_ANIM), Is_Hidden, the
**   Transform's 12 words, ObjectScale, the pivot count and each pivot's Get_Transform (12 words, no update); then the
**   client random state (6 words)]. Returns the word count. */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <vector>
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
#include "mp_psid.inc"
#include "mp_cache_attach_bone.inc"
#include "mp_acbits.inc"
#include "ma_maintain.inc"
#include "ma_no_next.inc"
#include "ma_test_flag.inc"

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
};

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
	CompositeRenderObjClass() : Transform(true), Hidden(false), ObjectScale(1.0f) {}
	int Class_ID(void) const { return CLASSID_HLOD; }
	void Set_Transform(const Matrix3D &m) { Transform = m; }
	void Set_Position(const Vector3 &v) { Transform.Set_Translation(v); }
	void Update_Sub_Object_Transforms(void) {}
	void Validate_Transform(void) const {}
	const Matrix3D &Get_Transform(void) const { return Transform; }
	int Is_Not_Hidden_At_All(void) { return !Hidden; }
	bool Are_Sub_Object_Transforms_Dirty(void) { return false; }
	/* RenderObjClass's hidden bit and rendobj.h:454 */
	void Set_Hidden(int onoff) { Hidden = onoff != 0; }
	int Is_Hidden(void) const { return Hidden ? 1 : 0; }
	void Set_ObjectScale(float scale) { ObjectScale = scale; }
};

class Animatable3DObjClass : public CompositeRenderObjClass
{
public:
	Animatable3DObjClass(const HTreeClass *source, float scale);
	~Animatable3DObjClass(void)
	{
		Release();
		if (HTree) delete HTree;
	}
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
Animatable3DObjClass::Animatable3DObjClass(const HTreeClass *source, float scale) : IsTreeValid(0), CurMotionMode(BASE_POSE)
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
typedef Animatable3DObjClass HLodClass;
typedef Animatable3DObjClass RenderObjClass;

/* ---- the loaded file ---- */
HTreeClass *g_tree;
std::vector<HAnimClass *> g_anims;
HTreeClass *find_tree(const char *name)
{
	if (g_tree && memcmp(g_tree->Get_Name(), name, W3D_NAME_LEN) == 0) return g_tree;
	return NULL;
}

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
class Object
{
public:
	PhysicsBehavior m_physics;
	const PhysicsBehavior *getPhysics() const { return &m_physics; }
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
};

/* ---- the module data and its states ---- */
/* W3DAnimationInfo: the animation by name through the asset manager (Get_HAnim: Add_Ref'd), its idle flag and distance */
class W3DAnimationInfo
{
public:
	AsciiString m_name;
	Bool m_isIdleAnim;
	Real m_distanceCovered;
	HAnimClass *getAnimHandle() const
	{
		for (size_t i = 0; i < g_anims.size(); ++i)
			if (strcasecmp(g_anims[i]->Get_Name(), m_name.str()) == 0) {
				g_anims[i]->Add_Ref();
				return g_anims[i];
			}
		return NULL;
	}
	Bool isIdleAnim() const { return m_isIdleAnim; }
	Real getDistanceCovered() const { return m_distanceCovered; }
};
struct ParticleSysBoneInfo
{
	AsciiString boneName;
	const ParticleSystemTemplate *particleSystemTemplate;
};
/* W3DModelDraw.h:173-200's barrel, never made here (outside a logic time) */
struct WeaponBarrelInfo
{
	Int m_recoilBone;
	Int m_fxBone;
	Int m_muzzleFlashBone;
	void setMuzzleFlashHidden(RenderObjClass *, Bool) const { abort(); }
};
struct ModelConditionInfo
{
	/* W3DModelDraw.h's validStuff bits */
	enum { PRISTINE_BONES_VALID = 0x0001, TURRETS_VALID = 0x0002, BARRELS_VALID = 0x0004, PUBLIC_BONES_VALID = 0x0008 };
	typedef std::vector<WeaponBarrelInfo> WeaponBarrelInfoVec;
	Int model;
	std::vector<W3DAnimationInfo> m_animations;
	Int m_mode;
	Int m_flags;
	Real m_animMinSpeedFactor, m_animMaxSpeedFactor;
	std::vector<ParticleSysBoneInfo> m_particleSysBones;
	mutable Int m_validStuff;
	mutable WeaponBarrelInfoVec m_weaponBarrelInfoVec[WEAPONSLOT_COUNT];
	mutable Bool m_hasRecoilBonesOrMuzzleFlashes[WEAPONSLOT_COUNT];
	/* W3DModelDraw.h's TurretInfo, its name keys (no turret here) */
	struct TurretInfo { NameKeyType m_turretAngleNameKey, m_turretPitchNameKey; };
	TurretInfo m_turrets[MAX_TURRETS];
};
ModelConditionInfo g_states[4];
#include "ma_turret_differ.inc"
class W3DModelDrawModuleData
{
public:
	Bool m_animationsRequirePower;
	Bool m_particlesAttachedToAnimatedBones;
	AsciiString m_attachToDrawableBone;
	mutable Vector3 m_attachToDrawableBoneOffset;
	mutable Bool m_attachToDrawableBoneOffsetValid;
	Real m_initialRecoil, m_maxRecoil, m_recoilDamping, m_recoilSettle;
	const Vector3* getAttachToDrawableBoneOffset(const Drawable* draw) const;
};
W3DModelDrawModuleData g_data;
#include "mp_attach_offset.inc"

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

	W3DModelDraw()
	{
		int i;
#include "ma_ctor.inc"
		setModelState(&g_states[0]);
	}
	~W3DModelDraw() { delete m_renderObject; }
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
	/* handleClientTurretPositioning (W3DModelDraw.cpp:2415-2418): `if (!m_curState || !(m_curState->m_validStuff &
	** ModelConditionInfo::TURRETS_VALID)) return;` is all that runs here (see the header) */
	void handleClientTurretPositioning()
	{
		if (!m_curState || !(m_curState->m_validStuff & ModelConditionInfo::TURRETS_VALID))
			return;
		abort();
	}
	/* reached only with a next state, which no state here makes (no transition keys) */
	void setAnimationLoopDuration(UnsignedInt) { abort(); }

	/* setModelState: the early return for the current state (no next state: the states have no transition keys), the
	** head cut, the middle (see the header), the tail cut */
	void setModelState( const ModelConditionInfo *newState )
	{
		const ModelConditionInfo* nextState = NULL;
		if (m_curState != NULL && newState == m_curState && m_nextState == NULL)
			return;
#include "ma_sms_head.inc"
		/* 3016-3023: `newState->m_modelName != m_curState->m_modelName` is the model codes' comparison here; outside a
		** logic time no state is TURRETS_VALID, so turretNamesDiffer (cut) answers true: the render object is always made
		** again */
		if (m_curState == NULL || newState->model != m_curState->model || turretNamesDiffer(newState, m_curState)) {
			/* nukeCurrentRender(&transform) (2787-2818) */
			m_pauseAnimation = false;
			Matrix3D transform = m_renderObject ? m_renderObject->Get_Transform() : *getDrawable()->getTransformMatrix();
			delete m_renderObject;
			m_renderObject = NULL;
			/* Create_Render_Obj(newState->m_modelName.str(), draw->getScale(), m_hexColor) (3034) */
			if (newState->model < 3)
				m_renderObject = new RenderObjClass(g_tree, getDrawable()->getScale());
			/* validateStuff outside a logic time: nothing valid (see the header); rebuildWeaponRecoilInfo: no barrels */
			if (m_renderObject) {
				if (getDrawable()->isDrawableEffectivelyHidden())
					m_renderObject->Set_Hidden(TRUE);
				m_renderObject->Set_Transform(transform);
			}
		}
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
		}
		unsigned int seed[6];
		ref_get_state(1, seed);
		for (int i = 0; i < 6; ++i) rec({(int)seed[i]});
	}
};
#include "ma_anim_complete.inc"
#include "mp_adjust.inc"
#include "ma_draw.inc"
#include "ma_anim.inc"
#include "ma_recoil.inc"
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

} /* namespace mda */

extern "C" {

int ref_model_draw_anim_seq(const char *file, int flen, const int *in, int nin, int *out, int cap)
{
	using namespace mda;
	g_out.clear();
	g_psm.clear();
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
	for (int si = 0; si < 4; ++si) {
		ModelConditionInfo &st = g_states[si];
		st.model = in[k++];
		st.m_mode = in[k++];
		st.m_flags = in[k++];
		st.m_animMinSpeedFactor = real(in[k++]);
		st.m_animMaxSpeedFactor = real(in[k++]);
		st.m_validStuff = 0;
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
			W3DAnimationInfo info;
			info.m_name = ai < (int)g_anims.size() ? g_anims[ai]->Get_Name() : "SKEL.NOSUCH";
			info.m_isIdleAnim = idle != 0;
			info.m_distanceCovered = dist;
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
	}
	W3DModelDraw *draw = new W3DModelDraw();
	draw->state();
	for (int st = k; st + 13 < nin; st += 14) {
		const int *a = in + st;
		switch (a[0]) {
		case 0: {
			/* getCurrentAnimFraction (W3DModelDraw.cpp:2127-2148) reads its uninitialised `framenum` and `numFrames`
			** when Peek_Animation_And_Info answers NULL: a current state with a maintain-frame flag whose render object
			** plays no animation. Undefined upstream: the case ends there, recorded as [700] */
			const ModelConditionInfo *ns = &g_states[(unsigned)a[1] % (unsigned)nstates];
			float fr, mult;
			int mo, nf;
			if (draw->m_curState != NULL && ns != draw->m_curState && isAnyMaintainFrameFlagSet(draw->m_curState->m_flags) &&
				draw->m_renderObject != NULL && draw->m_renderObject->Peek_Animation_And_Info(fr, nf, mo, mult) == NULL) {
				rec({700});
				goto done;
			}
			draw->setModelState(ns);
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
