/* Differential-test wrapper for the logic's bone and launch-offset questions of GitHub #175 (upstream commit 0a05454d),
** cut verbatim by build.rs into OUT_DIR/logic_bones_gen:
**   lb_name_key.inc       GameEngine/Include/Common/NameKeyGenerator.h, `enum NameKeyType` (49-54);
**   lb_which_turret.inc   GameEngine/Include/Common/GameCommon.h, `enum WhichTurretType` (475-483);
**   lb_pristine.inc       GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DModelDraw.h, `struct
**                         PristineBoneInfo` and `PristineBoneInfoMap` (133-139);
**   lb_turret_barrel.inc  W3DModelDraw.h, ModelConditionInfo's `struct TurretInfo`, `struct WeaponBarrelInfo` and
**                         `WeaponBarrelInfoVec` (151-201);
**   lb_valid_enum.inc     W3DModelDraw.h, ModelConditionInfo's `m_validStuff` bits (232-239);
**   lb_acbits.inc         GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp, `enum ACBits`
**                         (221-233);
**   lb_valid_time.inc     W3DModelDraw.cpp, `isValidTimeToCalcLogicStuff` (78-82);
**   lb_public_bone.inc    W3DModelDraw.cpp, `ModelConditionInfo::addPublicBone` (403-415);
**   lb_bones.inc          W3DModelDraw.cpp, `testFlagBit`, `findSingleBone`, `findSingleSubObj`, `doSingleBoneName`,
**                         `ModelConditionInfo::validateStuff`, `validateCachedBones`, `validateWeaponBarrelInfo`,
**                         `validateTurretInfo`, `findPristineBone`, `findPristineBonePos`, `loadAnimations` (433-981);
**   lb_find_best.inc      W3DModelDraw.cpp, `W3DModelDrawModuleData::findBestInfo` (1708-1713);
**   lb_queries.inc        W3DModelDraw.cpp, `W3DModelDraw::getProjectileLaunchOffset` and
**                         `getPristineBonePositionsForConditionState` (3253-3506);
**   lb_barrel_count.inc   W3DModelDraw.cpp, `W3DModelDraw::getBarrelCount` (3689-3693);
**   lb_draw_launch.inc    GameEngine/Source/GameClient/Drawable.cpp, `Drawable::getProjectileLaunchOffset` (679-687);
**   lb_draw_pristine.inc  Drawable.cpp, `Drawable::getPristineBonePositions` (771-797);
**   lb_draw_barrels.inc   Drawable.cpp, `Drawable::getBarrelCount` (4266-4275);
**   lb_object.inc         GameEngine/Source/GameLogic/Object/Object.cpp, `Object::getSingleLogicalBonePosition`,
**                         `getSingleLogicalBonePositionOnTurret`, `getMultiLogicalBonePosition` (6014-6105);
**   lb_thing.inc          GameEngine/Source/Common/Thing/Thing.cpp, `Thing::convertBonePosToWorldPos` (341-362);
**   lb_get_sub_object.inc Libraries/Source/WWVegas/WW3D2/rendobj.cpp, `RenderObjClass::Get_Sub_Object_By_Name`
**                         (527-566);
**   lb_htree_scale.inc, lb_base_update.inc, lb_get_transform.inc, lb_asset_scale.inc: WW3D2/htree.cpp's
**                         `HTreeClass::Scale` and `Base_Update`, htree.h's `Get_Transform`, W3DAssetManager.cpp's
**                         `ident_scale` and `scale_epsilon` (the same pieces as #172's wrap_model_draw_particles.cpp).
** No text is edited. Compiled on stand-ins (namespace lb):
** - `ModelConditionFlags`: four 64-bit words (bit i in word i/64) with `clear(mask)`; `findBestInfo`'s
**   `m_conditionStateMap.findBestInfo(m_conditionStates, bits)` is SparseMatchFinder.h's, through wrap_flags.cpp's
**   `ref_sparse_match` (the header compiled unchanged there, over the real ModelConditionFlags);
** - the name keys: `NAMEKEY(s)` numbers each distinct string from 1 (TheNameKeyGenerator's case-sensitive table);
** - `ModelConditionInfo` and `W3DModelDrawModuleData`: the members the cuts read, filled as the INI parse leaves them
**   (names lower-cased, `None` cleared, `addPublicBone` of each weapon bone and turret name, the cut one);
** - the asset manager (`W3DDisplay::m_assetManager`): `Create_Render_Obj(name, scale, 0)` makes a render object of the
**   case's model `M0`..`M2` (any case) as W3DAssetManager.cpp:719-895 does (its own copy of the hierarchy, `Scale(scale)`
**   when `reallyscale`, the cut); `Get_HAnim(name)` the case's animation `A0` or `A1` (any case), else NULL;
** - the render object: `Class_ID` (an HLOD or a mesh), `Peek_Animation_And_Info`, `Set_Animation` (an HLOD's; a mesh
**   ignores it, RenderObjClass's default), `Set_Transform`/`Get_Transform`, `Get_Bone_Index` (htree.cpp:922-930: the
**   first pivot of that name, any case; 0 for a mesh or no pivots), `Get_Bone_Transform(i)` (animobj.cpp:658-675: the
**   pose's `Base_Update(Transform)` (cut) then `Get_Transform(i)` (cut); without a hierarchy the render object's
**   `Transform`), its sub-objects (each bound to a bone, its `Get_Transform` its bone's, as `Update_Sub_Object_Transforms`
**   leaves it; `Get_Sub_Object_By_Name` is the cut);
** - **the animated pose is the test's own** (the same in tests/differential_logic_bones.rs): each pivot but the root
**   has its base transform's translation moved by (frame + 1) times the animation's delta for it, then the chain as
**   `Base_Update`'s. What is under test is which animation and frame `validateCachedBones` poses (the first animation,
**   frame 0 or its last with PRISTINE_BONE_POS_IN_FINAL_FRAME, only an HLOD), not `Anim_Update`, which
**   differential_w3d checks against htree.cpp;
** - the drawable (`m_conditionState`, `getScale`, `getOrientation` = its `m_cachedAngle`, `Get_Z_Rotation` of the
**   object's transform, as Thing::setTransformMatrix caches it; `getObject`; its draw modules, each a `W3DModelDraw`
**   whose `m_renderObject` is NULL: the logic's), the object (`m_drawable`, its transform and position, `getAI()`, whose
**   `getTurretRotAndPitch(TURRET_MAIN)` gives the test's rotation or fails), and `TheGlobalData->m_standardPublicBones`;
** - `getBarrelCount` reads `m_curState`: the driver sets it to `findBestInfo(m_conditionState)`, validated as
**   `setModelState` validates the state it enters at a logic time (the logic's view: never a transition state);
** - `isValidTimeToCalcLogicStuff` (cut) reads `TheGameLogic->isInGameLogicUpdate()`: true (the logic asks in its
**   update); `TheGameState` NULL.
** `W3DModelDraw::getPristineBonePositionsForConditionState`'s second loop reuses the first loop's `i` (MSVC's old `for`
** scope); the stand-in class has a member `i` it then names (the cut is not edited). `atan2` and the other WWMath
** functions are the Rust's petir functions in the test binary.
**
** Entry point (extern "C", tests/differential_logic_bones.rs; floats travel as their bits): ref_logic_bones_case(in,
** nin, out, cap), the layout in that test's `random_case`. Records per operation: see `run_op`. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <vector>
#include <map>
#include <string>
#include <algorithm>
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/Debug.h"
#include "Common/GameType.h"
#include "matrix3d.h"
#include "vector3.h"
#include "wwmath.h"

#ifndef stricmp
#define stricmp strcasecmp
#endif

extern "C" int ref_sparse_match(int which, const unsigned long long *masks, const int *counts, int nEntries, const unsigned long long *queries, int nQueries, int *out);

/* W3DModelDraw.cpp's logging, compiled out as upstream builds it (DEBUG_CRC not defined) */
#define BONEPOS_LOG(x) {}
#define BONEPOS_DUMPMATRIX3D(x) {}
#define BONEPOS_DUMPMATRIX3DNAMED(x, y) {}
#define BONEPOS_DUMPREAL(x) {}
#define BONEPOS_DUMPREALNAMED(x, y) {}
#define CRCDEBUG_LOG(x) {}
#define DUMPMATRIX3D(x) {}
#define DUMPREAL(x) {}
/* WWLib/refcount.h */
#define REF_PTR_RELEASE(x) { if (x) x->Release_Ref(); x = NULL; }

namespace lb {

std::vector<int> g_out;
void rec(int w) { g_out.push_back(w); }
int bits(float f) { int i; memcpy(&i, &f, 4); return i; }
float real(int i) { float f; memcpy(&f, &i, 4); return f; }
void recM(const Matrix3D &m) { for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) rec(bits(m[r][c])); }
void recC(const Coord3D &c) { rec(bits(c.x)); rec(bits(c.y)); rec(bits(c.z)); }

#include "lb_name_key.inc"
#include "lb_which_turret.inc"

/* TheNameKeyGenerator: each distinct string a number from 1 */
std::map<std::string, int> g_keys;
NameKeyType namekey(const char *s)
{
	std::map<std::string, int>::iterator it = g_keys.find(s);
	if (it != g_keys.end()) return (NameKeyType)it->second;
	int k = (int)g_keys.size() + 1;
	g_keys[s] = k;
	return (NameKeyType)k;
}
NameKeyType namekey(const AsciiString &s) { return namekey(s.str()); }
#define NAMEKEY(x) lb::namekey(x)
#define KEYNAME(x) AsciiString("")

void setFPMode() {}

/* ModelConditionFlags: 4 words */
class ModelConditionFlags
{
public:
	unsigned long long w[4];
	ModelConditionFlags() { w[0] = w[1] = w[2] = w[3] = 0; }
	void clear(const ModelConditionFlags &m) { for (int k = 0; k < 4; ++k) w[k] &= ~m.w[k]; }
	bool test(int i) const { return (w[i / 64] >> (i % 64)) & 1; }
};

class GameLogic { public: Bool isInGameLogicUpdate() const { return TRUE; } };
GameLogic g_logic;
GameLogic *TheGameLogic = &g_logic;
class GameState { public: Bool isInLoadGame() const { return FALSE; } };
GameState *TheGameState = NULL;
#include "lb_valid_time.inc"

class GlobalData { public: std::vector<AsciiString> m_standardPublicBones; };
GlobalData g_global;
GlobalData *TheGlobalData = &g_global;

/* ---- the case's art ---- */
struct FakeModel
{
	Bool hlod;
	Int n;
	char names[6][24];
	Int parent[6];
	Matrix3D pose[6];
	Int nsub;
	char subNames[3][40];
	Int subBone[3];
};
FakeModel g_models[3];
const char *const MODEL_NAMES[] = { "M0", "M1", "M2" };
struct FakeAnim
{
	Int numFrames;
	float delta[6][3];
	Int refs;
	const char *name;
	Int Get_Num_Frames() const { return numFrames; }
	void Add_Ref() { ++refs; }
	void Release_Ref() { --refs; }
};
typedef FakeAnim HAnimClass;
FakeAnim g_anims[2];

class PivotClass
{
public:
	PivotClass *Parent;
	Matrix3D BaseTransform;
	Matrix3D Transform;
	bool IsVisible;
	PivotClass() : Parent(NULL), BaseTransform(true), Transform(true), IsVisible(true) {}
	void Capture_Update( void ) { abort(); }
	bool Is_Captured() const { return false; }
};
class HTreeClass
{
public:
	int NumPivots;
	PivotClass Pivot[6];
	float ScaleFactor;
	HTreeClass() : NumPivots(0), ScaleFactor(1.0f) {}
	void Base_Update(const Matrix3D & root);
	void Scale(float factor);
	const Matrix3D & Get_Transform(int pivot) const;
};
#include "lb_base_update.inc"
#include "lb_htree_scale.inc"
#include "lb_get_transform.inc"
#include "lb_asset_scale.inc"

class RenderObjClass
{
public:
	enum { CLASSID_MESH = 0, CLASSID_HLOD = 16 };
	enum AnimMode { ANIM_MODE_MANUAL = 0, ANIM_MODE_LOOP };
	const FakeModel *m;
	HTreeClass HTree;
	Matrix3D Transform;
	HAnimClass *anim;
	float frame;
	Int refs;
	/* a sub-object: its name and bone, its parent */
	RenderObjClass *owner;
	std::string name;
	Int bone;
	std::vector<RenderObjClass *> subs;
	RenderObjClass() : m(NULL), Transform(true), anim(NULL), frame(0), refs(1), owner(NULL), bone(0) {}
	~RenderObjClass() { for (size_t i = 0; i < subs.size(); ++i) delete subs[i]; }
	void Add_Ref() { ++refs; }
	void Release_Ref() { if (owner) return; if (--refs == 0) delete this; }
	int Class_ID() const { return m && m->hlod ? CLASSID_HLOD : CLASSID_MESH; }
	const char *Get_Name() const { return name.c_str(); }
	/* HLodClass::Scale (hlod.cpp:3264-3290): the hierarchy */
	void Scale( float scale )
	{
		if (scale==1.0f) return;
		HTree.Scale(scale);
	}
	/* W3DAssetManager::Create_Render_Obj(name, scale, color) (W3DAssetManager.cpp:719-895) for model `fm` */
	void Create( const FakeModel *fm, float scale )
	{
		m = fm;
		HTree = HTreeClass();
		HTree.NumPivots = fm->hlod ? fm->n : 0;
		for (int i = 0; i < HTree.NumPivots; i++) {
			HTree.Pivot[i].BaseTransform = fm->pose[i];
			HTree.Pivot[i].Parent = i == 0 ? NULL : &HTree.Pivot[fm->parent[i]];
		}
		Bool reallyscale = (WWMath::Fabs(scale - ident_scale) > scale_epsilon);
		if (reallyscale && fm->hlod)
			Scale(scale);
		if (fm->hlod && fm->n > 0) {
			for (int s = 0; s < fm->nsub; ++s) {
				RenderObjClass *c = new RenderObjClass();
				c->owner = this;
				c->name = fm->subNames[s];
				c->bone = fm->subBone[s];
				subs.push_back(c);
			}
		}
	}
	HAnimClass *Peek_Animation_And_Info( float &f, int &numFrames, int &mode, float &mult )
	{
		f = frame;
		numFrames = anim ? anim->numFrames : 0;
		mode = 0;
		mult = 1.0f;
		return anim;
	}
	/* RenderObjClass's default does nothing; an HLOD's (Animatable3DObjClass) plays it */
	void Set_Animation( HAnimClass *a, float f, int mode = ANIM_MODE_MANUAL )
	{
		if (!m || !m->hlod) return;
		anim = a;
		frame = f;
	}
	void Set_Animation_Frame_Rate_Multiplier( float ) {}
	const Matrix3D &Get_Transform( void ) const
	{
		if (owner) {
			/* a sub-object's transform: its bone's in the owner's pose */
			static Matrix3D t;
			t = owner->Get_Bone_Transform(bone);
			return t;
		}
		return Transform;
	}
	void Set_Transform( const Matrix3D &m3 ) { Transform = m3; }
	int Get_Bone_Index( const char *n )
	{
		if (!m || !m->hlod || m->n == 0)
			return 0;
		for (int i = 0; i < m->n; i++)
			if (stricmp(m->names[i], n) == 0)
				return i;
		return 0;
	}
	/* the pose: Base_Update (cut) over the base transforms, the animation's moving each pivot's translation (the test's
	** pose, see the header) */
	const Matrix3D &Get_Bone_Transform( int i )
	{
		if (!m || !m->hlod || m->n == 0)
			return Transform;
		if (anim) {
			HTreeClass posed = HTree;
			for (int p = 1; p < posed.NumPivots; ++p) {
				int k = (int)(anim - g_anims);
				float steps = frame + 1.0f;
				Matrix3D &b = posed.Pivot[p].BaseTransform;
				b[0][3] = b[0][3] + g_anims[k].delta[p][0] * steps;
				b[1][3] = b[1][3] + g_anims[k].delta[p][1] * steps;
				b[2][3] = b[2][3] + g_anims[k].delta[p][2] * steps;
				posed.Pivot[p].Parent = &posed.Pivot[m->parent[p]];
			}
			posed.Base_Update(Transform);
			static Matrix3D t;
			t = posed.Get_Transform(i);
			return t;
		}
		HTree.Base_Update(Transform);
		return HTree.Get_Transform(i);
	}
	int Get_Num_Sub_Objects() const { return (int)subs.size(); }
	RenderObjClass *Get_Sub_Object( int i ) const { return subs[i]; }
	int Get_Sub_Object_Bone_Index( int lod, int i ) const { return subs[i]->bone; }
	RenderObjClass *Get_Sub_Object_By_Name( const char *name, int *index = NULL ) const;
};
#include "lb_get_sub_object.inc"
typedef RenderObjClass HLodClass;

class AssetManager
{
public:
	RenderObjClass *Create_Render_Obj( const char *name, float scale, int color )
	{
		for (int i = 0; i < 3; ++i)
			if (stricmp(MODEL_NAMES[i], name) == 0) {
				RenderObjClass *r = new RenderObjClass();
				r->Create(&g_models[i], scale);
				return r;
			}
		return NULL;
	}
	/* Get_HAnim: addref'd */
	HAnimClass *Get_HAnim( const char *name )
	{
		for (int i = 0; i < 2; ++i)
			if (stricmp(g_anims[i].name, name) == 0) {
				g_anims[i].Add_Ref();
				return &g_anims[i];
			}
		return NULL;
	}
};
AssetManager g_am;
class W3DDisplay { public: static AssetManager *m_assetManager; };
AssetManager *W3DDisplay::m_assetManager = &g_am;

/* W3DAnimationInfo::getAnimHandle (W3DModelDraw.cpp:346-375, without RETAIN_ANIM_HANDLES): Get_HAnim's addref'd
** handle */
class W3DAnimationInfo
{
public:
	AsciiString m_name;
	HAnimClass *getAnimHandle() const { return W3DDisplay::m_assetManager->Get_HAnim(m_name.str()); }
};

#include "lb_pristine.inc"
#include "lb_acbits.inc"

struct ModelConditionInfo
{
#include "lb_turret_barrel.inc"
	std::vector<ModelConditionFlags>	m_conditionsYesVec;
	AsciiString												m_modelName;
	mutable std::vector<AsciiString>	m_publicBones;
	AsciiString												m_weaponFireFXBoneName[WEAPONSLOT_COUNT];
	AsciiString												m_weaponRecoilBoneName[WEAPONSLOT_COUNT];
	AsciiString												m_weaponMuzzleFlashName[WEAPONSLOT_COUNT];
	AsciiString												m_weaponProjectileLaunchBoneName[WEAPONSLOT_COUNT];
	std::vector<W3DAnimationInfo>			m_animations;
	Int																m_flags;
	mutable PristineBoneInfoMap				m_pristineBones;
	mutable TurretInfo								m_turrets[MAX_TURRETS];
	mutable WeaponBarrelInfoVec				m_weaponBarrelInfoVec[WEAPONSLOT_COUNT];
	mutable Bool											m_hasRecoilBonesOrMuzzleFlashes[WEAPONSLOT_COUNT];
	mutable Byte											m_validStuff;
#include "lb_valid_enum.inc"
	ModelConditionInfo() : m_flags(0), m_validStuff(0)
	{
		for (int t = 0; t < MAX_TURRETS; ++t) m_turrets[t].clear();
		for (int w = 0; w < WEAPONSLOT_COUNT; ++w) m_hasRecoilBonesOrMuzzleFlashes[w] = FALSE;
	}
	void addPublicBone(const AsciiString& boneName) const;
	void validateStuff(RenderObjClass* robj, Real scale, const std::vector<AsciiString>& extraPublicBones) const;
	void validateCachedBones(RenderObjClass* robj, Real scale) const;
	void validateWeaponBarrelInfo() const;
	void validateTurretInfo() const;
	const Matrix3D* findPristineBone(NameKeyType boneName, Int* boneIndex) const;
	Bool findPristineBonePos(NameKeyType boneName, Coord3D& pos) const;
	void loadAnimations() const;
};
typedef std::vector<ModelConditionInfo> ModelConditionVector;

/* m_conditionStateMap: SparseMatchFinder's findBestInfo, through ref_sparse_match (see the header) */
class ConditionStateMap
{
public:
	const ModelConditionInfo *findBestInfo( const ModelConditionVector &v, const ModelConditionFlags &bits ) const
	{
		std::vector<unsigned long long> masks;
		std::vector<int> counts;
		for (size_t e = 0; e < v.size(); ++e) {
			counts.push_back((int)v[e].m_conditionsYesVec.size());
			for (size_t c = 0; c < v[e].m_conditionsYesVec.size(); ++c)
				for (int k = 0; k < 4; ++k) masks.push_back(v[e].m_conditionsYesVec[c].w[k]);
		}
		int out[2] = { -1, -1 };
		if (masks.empty()) masks.push_back(0);
		ref_sparse_match(0, &masks[0], counts.empty() ? out : &counts[0], (int)v.size(), bits.w, 1, out);
		return out[0] < 0 ? NULL : &v[out[0]];
	}
};

class W3DModelDrawModuleData
{
public:
	ModelConditionVector m_conditionStates;
	mutable ConditionStateMap m_conditionStateMap;
	ModelConditionFlags m_ignoreConditionStates;
	std::vector<AsciiString> m_extraPublicBones;
	AsciiString m_attachToDrawableBone;
	const ModelConditionInfo* findBestInfo(const ModelConditionFlags& c) const;
};

class Object;
class Drawable;

class Thing
{
public:
	Matrix3D m_transform;
	Real m_cachedAngle;
	Coord3D m_cachedPos;
	const Matrix3D *getTransformMatrix() const { return &m_transform; }
	const Coord3D *getPosition() const { return &m_cachedPos; }
	Real getOrientation() const { return m_cachedAngle; }
	void convertBonePosToWorldPos(const Coord3D* bonePos, const Matrix3D* boneTransform, Coord3D* worldPos, Matrix3D* worldTransform) const;
};
#include "lb_thing.inc"

class W3DModelDraw;
typedef W3DModelDraw ObjectDrawInterface;
class DrawModule
{
public:
	W3DModelDraw *di;
	ObjectDrawInterface *getObjectDrawInterface() { return di; }
	const ObjectDrawInterface *getObjectDrawInterface() const { return di; }
};

class Drawable : public Thing
{
public:
	ModelConditionFlags m_conditionState;
	Real m_instanceScale;
	Object *m_object;
	DrawModule *m_modules[4];
	const Object *getObject() const { return m_object; }
	const Real getScale( void ) const { return m_instanceScale; }
	const DrawModule **getDrawModules() const { return (const DrawModule **)m_modules; }
	Int getPristineBonePositions(const char* boneNamePrefix, Int startIndex, Coord3D* positions, Matrix3D* transforms, Int maxBones) const;
	Bool getProjectileLaunchOffset(WeaponSlotType wslot, Int specificBarrelToUse, Matrix3D* launchPos, WhichTurretType tur, Coord3D* turretRotPos, Coord3D* turretPitchPos) const;
	Int getBarrelCount(WeaponSlotType wslot) const;
};
Drawable g_drawable;

class W3DModelDraw
{
public:
	const W3DModelDrawModuleData *m_data;
	const ModelConditionInfo *m_curState;
	RenderObjClass *m_renderObject;
	/* the second loop of getPristineBonePositionsForConditionState names it (see the header) */
	mutable Int i;
	const W3DModelDrawModuleData *getW3DModelDrawModuleData() const { return m_data; }
	const ModelConditionInfo *findBestInfo(const ModelConditionFlags& c) const { return m_data->findBestInfo(c); }
	Drawable *getDrawable() const { return &g_drawable; }
	Bool getProjectileLaunchOffset(const ModelConditionFlags& condition, WeaponSlotType wslot, Int specificBarrelToUse, Matrix3D* launchPos, WhichTurretType tur, Coord3D* turretRotPos, Coord3D* turretPitchPos) const;
	Int getPristineBonePositionsForConditionState(const ModelConditionFlags& condition, const char* boneNamePrefix, Int startIndex, Coord3D* positions, Matrix3D* transforms, Int maxBones) const;
	Int getBarrelCount(WeaponSlotType wslot) const;
};

class AIUpdateInterface
{
public:
	Bool m_turretValid;
	Real m_turretRotation;
	Bool getTurretRotAndPitch(WhichTurretType tur, Real* turretAngle, Real* turretPitch) const
	{
		if (tur != TURRET_MAIN || !m_turretValid) return FALSE;
		if (turretAngle) *turretAngle = m_turretRotation;
		return TRUE;
	}
};
AIUpdateInterface g_ai;
/* whether the last getTurretRotAndPitch failed (its rotation then read uninitialised: undefined) */

class Object : public Thing
{
public:
	Drawable *m_drawable;
	AIUpdateInterface *m_ai;
	Drawable *getDrawable() const { return m_drawable; }
	const AIUpdateInterface *getAI() const { return m_ai; }
	Bool getSingleLogicalBonePosition(const char* boneName, Coord3D* position, Matrix3D* transform) const;
	Bool getSingleLogicalBonePositionOnTurret(WhichTurretType whichTurret, const char* boneName, Coord3D* position, Matrix3D* transform) const;
	Int getMultiLogicalBonePosition(const char* boneNamePrefix, Int maxBones, Coord3D* positions, Matrix3D* transforms, Bool convertToWorld = TRUE) const;
};
Object g_object;

#include "lb_public_bone.inc"
#include "lb_bones.inc"
#include "lb_find_best.inc"
#include "lb_queries.inc"
#include "lb_barrel_count.inc"
#include "lb_draw_launch.inc"
#include "lb_draw_pristine.inc"
#include "lb_draw_barrels.inc"
#include "lb_object.inc"

/* ---- the driver ---- */
const char *const BONE_NAMES[] = { "ROOTTRANSFORM", "FIREPOINT01", "FIREPOINT02", "FirePoint03", "MUZZLEFX01", "MuzzleFX02", "RECOIL01",
	"TURRET", "BARREL", "LAUNCH", "LAUNCH01", "ATTACH", "DOCKWAITING07", "Muzzle" };
const char *const QUERY_NAMES[] = { "", "FirePoint", "FIREPOINT01", "MuzzleFX", "Recoil", "Turret", "Barrel", "Launch", "Attach", "DockWaiting",
	"nosuch", "None", "muzzlefx01", "MUZZLE" };
const char *const SUB_NAMES[] = { "M.CHASSIS", "M.FIREPOINT02", "M.MUZZLE", "BARREL", "M.LAUNCH", "M.NOSUCH" };

W3DModelDrawModuleData g_data[3];
W3DModelDraw g_draws[3];
DrawModule g_dm[3];

Matrix3D matrix(const int *w)
{
	Matrix3D m(true);
	for (int r = 0; r < 3; ++r)
		for (int c = 0; c < 4; ++c) m[r][c] = real(w[r * 4 + c]);
	return m;
}
ModelConditionFlags mask(int w)
{
	ModelConditionFlags f;
	f.w[0] = (unsigned int)w;
	return f;
}
/* the parse's name: lower-cased, `None` cleared */
AsciiString lowered(int code)
{
	AsciiString s = QUERY_NAMES[code];
	s.toLower();
	if (s.isNone()) s.clear();
	return s;
}

const int SENT = 0x7fc0dead;

void run_op(const int *a, Int nmodules)
{
	switch (a[0]) {
	case 0: {
		/* Drawable::getPristineBonePositions(prefix, start, positions?, transforms?, max): [10, count, 8 positions, 8 transforms] */
		Coord3D pos[8];
		Matrix3D tr[8];
		for (int k = 0; k < 8; ++k) {
			pos[k].x = pos[k].y = pos[k].z = real(SENT);
			for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) tr[k][r][c] = real(SENT);
		}
		Int n = g_drawable.getPristineBonePositions(QUERY_NAMES[a[1]], a[2], a[4] ? pos : NULL, a[5] ? tr : NULL, a[3]);
		rec(10); rec(n);
		for (int k = 0; k < 8; ++k) recC(pos[k]);
		for (int k = 0; k < 8; ++k) recM(tr[k]);
		break;
	}
	case 1: {
		/* Object::getSingleLogicalBonePosition(name, &pos, &mtx): [11, found, pos, mtx] */
		Coord3D p; Matrix3D m(true);
		Bool f = g_object.getSingleLogicalBonePosition(QUERY_NAMES[a[1]], &p, &m);
		rec(11); rec(f ? 1 : 0); recC(p); recM(m);
		break;
	}
	case 2: {
		/* Object::getMultiLogicalBonePosition(prefix, max, positions?, transforms?, convert): [12, count, the first count
		** of each array that was given] */
		Coord3D pos[8];
		Matrix3D tr[8];
		Int n = g_object.getMultiLogicalBonePosition(QUERY_NAMES[a[1]], a[2], a[3] ? pos : NULL, a[4] ? tr : NULL, a[5] != 0);
		rec(12); rec(n);
		for (int k = 0; k < n; ++k) { if (a[3]) recC(pos[k]); if (a[4]) recM(tr[k]); }
		break;
	}
	case 3: {
		/* Object::getSingleLogicalBonePositionOnTurret(TURRET_MAIN, name, &pos, &mtx): [13, ret, pos, mtx], or [13, -1]
		** when it reads a rotation getTurretRotAndPitch did not write (undefined upstream) */
		Coord3D p; p.x = p.y = p.z = 0; Matrix3D m(true);
		Coord3D bp;
		Bool reads = g_object.getDrawable() && g_object.getAI() && !g_ai.m_turretValid &&
			g_drawable.getPristineBonePositions(QUERY_NAMES[a[1]], 0, &bp, NULL, 1) == 1;
		if (reads) { rec(13); rec(-1); break; }
		Bool r = g_object.getSingleLogicalBonePositionOnTurret(TURRET_MAIN, QUERY_NAMES[a[1]], &p, &m);
		rec(13); rec(r ? 1 : 0);
		if (r) { recC(p); recM(m); }
		break;
	}
	case 4: {
		/* Drawable::getProjectileLaunchOffset(wslot, barrel, launch?, tur, rot?, pitch?): [14, ret, launch, rot, pitch] */
		Matrix3D l; Coord3D rp, pp;
		for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) l[r][c] = real(SENT);
		rp.x = rp.y = rp.z = real(SENT);
		pp.x = pp.y = pp.z = real(SENT);
		Bool r = g_drawable.getProjectileLaunchOffset((WeaponSlotType)a[1], a[2], a[3] ? &l : NULL, (WhichTurretType)a[4], a[5] ? &rp : NULL, a[6] ? &pp : NULL);
		rec(14); rec(r ? 1 : 0); recM(l); recC(rp); recC(pp);
		break;
	}
	case 5: {
		/* Drawable::getBarrelCount(wslot), each module's m_curState the logic's state, validated: [15, count] */
		for (int k = 0; k < nmodules; ++k) {
			g_draws[k].m_curState = g_data[k].findBestInfo(g_drawable.m_conditionState);
			if (g_draws[k].m_curState)
				g_draws[k].m_curState->validateStuff(NULL, g_drawable.getScale(), g_data[k].m_extraPublicBones);
		}
		rec(15); rec(g_drawable.getBarrelCount((WeaponSlotType)a[1]));
		break;
	}
	default:
		/* the drawable's flags */
		g_drawable.m_conditionState = mask(a[1]);
		rec(16);
		break;
	}
}

} /* namespace lb */

extern "C" {

int ref_logic_bones_case(const int *in, int nin, int *out, int cap)
{
	using namespace lb;
	g_out.clear();
	g_keys.clear();
	int k = 0;
	g_drawable.m_instanceScale = real(in[k++]);
	g_global.m_standardPublicBones.clear();
	int nstd = in[k++];
	for (int s = 0; s < 3; ++s) {
		int code = in[k++];
		if (s < nstd) g_global.m_standardPublicBones.push_back(AsciiString(QUERY_NAMES[code]));
	}
	for (int mi = 0; mi < 3; ++mi) {
		FakeModel &fm = g_models[mi];
		fm.hlod = in[k++] != 0;
		fm.n = in[k++];
		for (int b = 0; b < 6; ++b) {
			strcpy(fm.names[b], BONE_NAMES[in[k++]]);
			fm.parent[b] = in[k++];
			fm.pose[b] = matrix(in + k);
			k += 12;
		}
		fm.nsub = in[k++];
		for (int s = 0; s < 3; ++s) {
			const char *sn = SUB_NAMES[in[k++]];
			/* `M.` stands for the model's name */
			if (sn[0] == 'M' && sn[1] == '.') sprintf(fm.subNames[s], "%s.%s", MODEL_NAMES[mi], sn + 2);
			else strcpy(fm.subNames[s], sn);
			fm.subBone[s] = in[k++];
		}
	}
	static const char *const ANIM_NAMES[] = { "A0", "A1" };
	for (int ai = 0; ai < 2; ++ai) {
		g_anims[ai].name = ANIM_NAMES[ai];
		g_anims[ai].numFrames = in[k++];
		g_anims[ai].refs = 1;
		for (int p = 0; p < 6; ++p)
			for (int c = 0; c < 3; ++c) g_anims[ai].delta[p][c] = real(in[k++]);
	}
	int nmod = in[k++];
	static const char *const STATE_MODELS[] = { "M0", "M1", "M2", "", "MISSING" };
	static const char *const STATE_ANIMS[] = { "", "A0", "A1", "AMISSING" };
	for (int mi = 0; mi < 3; ++mi) {
		W3DModelDrawModuleData &d = g_data[mi];
		d = W3DModelDrawModuleData();
		d.m_ignoreConditionStates = mask(in[k++]);
		d.m_attachToDrawableBone = lowered(in[k++]);
		int nextra = in[k++];
		for (int e = 0; e < 2; ++e) {
			int code = in[k++];
			if (e < nextra) d.m_extraPublicBones.push_back(AsciiString(QUERY_NAMES[code]));
		}
		int nstates = in[k++];
		for (int si = 0; si < 4; ++si) {
			ModelConditionInfo s;
			int nyes = in[k++];
			for (int y = 0; y < 2; ++y) {
				int m = in[k++];
				if (y < nyes) s.m_conditionsYesVec.push_back(mask(m));
			}
			s.m_modelName = STATE_MODELS[in[k++]];
			s.m_modelName.toLower();
			s.m_flags = in[k++];
			int anim = in[k++];
			if (anim != 0) { W3DAnimationInfo a; a.m_name = STATE_ANIMS[anim]; s.m_animations.push_back(a); }
			int npub = in[k++];
			for (int p = 0; p < 2; ++p) {
				int code = in[k++];
				/* parsed public bones (the parse's addPublicBone) */
				if (p < npub) s.addPublicBone(AsciiString(QUERY_NAMES[code]));
			}
			AsciiString *arrs[4] = { s.m_weaponFireFXBoneName, s.m_weaponRecoilBoneName, s.m_weaponMuzzleFlashName, s.m_weaponProjectileLaunchBoneName };
			for (int w = 0; w < 3; ++w)
				for (int kind = 0; kind < 4; ++kind) {
					/* parseWeaponBoneName: lower-cased, None cleared, addPublicBone */
					AsciiString n = lowered(in[k++]);
					arrs[kind][w] = n;
					s.addPublicBone(n);
				}
			for (int t = 0; t < MAX_TURRETS; ++t) {
				/* parseBoneNameKey: lower-cased, addPublicBone, the key (NAMEKEY_INVALID for empty or None) */
				for (int which = 0; which < 2; ++which) {
					AsciiString n = QUERY_NAMES[in[k++]];
					n.toLower();
					s.addPublicBone(n);
					NameKeyType key = (n.isEmpty() || n.isNone()) ? NAMEKEY_INVALID : NAMEKEY(n.str());
					if (which == 0) s.m_turrets[t].m_turretAngleNameKey = key;
					else s.m_turrets[t].m_turretPitchNameKey = key;
				}
				s.m_turrets[t].m_turretArtAngle = real(in[k++]);
				s.m_turrets[t].m_turretArtPitch = real(in[k++]);
			}
			if (si < nstates) d.m_conditionStates.push_back(s);
		}
		g_draws[mi].m_data = &d;
		g_draws[mi].m_curState = NULL;
		g_draws[mi].m_renderObject = NULL;
		g_dm[mi].di = &g_draws[mi];
	}
	for (int mi = 0; mi < 4; ++mi) g_drawable.m_modules[mi] = mi < nmod ? &g_dm[mi] : NULL;
	/* the object: its transform (Thing::setTransformMatrix's caches), its AI's turret, the drawable's flags */
	g_object.m_transform = matrix(in + k);
	k += 12;
	g_object.m_cachedPos.x = g_object.m_transform.Get_X_Translation();
	g_object.m_cachedPos.y = g_object.m_transform.Get_Y_Translation();
	g_object.m_cachedPos.z = g_object.m_transform.Get_Z_Translation();
	g_object.m_cachedAngle = g_object.m_transform.Get_Z_Rotation();
	g_ai.m_turretValid = in[k++] != 0;
	g_ai.m_turretRotation = real(in[k++]);
	g_object.m_ai = &g_ai;
	g_object.m_drawable = &g_drawable;
	g_drawable.m_transform = g_object.m_transform;
	g_drawable.m_cachedPos = g_object.m_cachedPos;
	g_drawable.m_cachedAngle = g_object.m_cachedAngle;
	g_drawable.m_object = &g_object;
	g_drawable.m_conditionState = mask(in[k++]);
	for (int st = k; st + 7 < nin; st += 8) run_op(in + st, nmod);
	int n = (int)g_out.size();
	if (n > cap) n = cap;
	if (n > 0) memcpy(out, &g_out[0], n * sizeof(int));
	return (int)g_out.size();
}

} /* extern "C" */
