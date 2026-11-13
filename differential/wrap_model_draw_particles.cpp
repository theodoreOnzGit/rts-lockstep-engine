/* Differential-test wrapper for W3DModelDraw's bone particle systems of GitHub #171 (upstream commit 0a05454d), cut
** verbatim by build.rs into OUT_DIR/model_draw_particles_gen:
**   mp_psid.inc           GameEngine/Include/GameClient/ParticleSys.h, `enum ParticleSystemID` (to
**                         MAX_VOLUME_PARTICLE_DEPTH);
**   mp_tracker.inc        GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DModelDraw.h, `struct
**                         ParticleSysTrackerType` (481-485);
**   mp_ctor_flags.inc     GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp, the constructor's
**                         `m_needRecalcBoneParticleSystems = false; m_fullyObscuredByShroud = false;` (1740-1741);
**   mp_start_stop.inc     W3DModelDraw.cpp, `doStartOrStopParticleSys` and `setHidden` (1803-1841);
**   mp_shroud.inc         W3DModelDraw.cpp, `setFullyObscuredByShroud` (1939-1952);
**   mp_stop.inc           W3DModelDraw.cpp, `stopClientParticleSystems` (2385-2397);
**   mp_bones.inc          W3DModelDraw.cpp, `recalcBonesForClientParticleSystems` and
**                         `updateBonesForClientParticleSystems` (2589-2722);
**   mp_set_model_state.inc  W3DModelDraw.cpp, setModelState's set of the flag and stop (3002-3006);
**   mp_draw.inc           W3DModelDraw.cpp, doDrawModule's recalc and update (2103-2107);
**   mp_psys_a.inc         GameEngine/Source/GameClient/System/ParticleSys.cpp, `ParticleSystem::setSaveable`, `start`,
**                         `stop`, `destroy` (1253-1286);
**   mp_psys_b.inc         ParticleSys.cpp, `setPosition`, `setLocalTransform`, `rotateLocalTransformX/Y/Z`,
**                         `attachToDrawable` (1305-1358);
** and since GitHub #172 (the drawable's scale as upstream's render object carries it):
**   mp_worldspace_bone.inc  W3DModelDraw.cpp, `getCurrentWorldspaceClientBonePositions` (3554-3566);
**   mp_htree_scale.inc    Libraries/Source/WWVegas/WW3D2/htree.cpp, `HTreeClass::Scale` (980-998);
**   mp_base_update.inc    htree.cpp, `HTreeClass::Base_Update` (531-548, and the comment block before Anim_Update);
**   mp_get_transform.inc  WW3D2/htree.h, `HTreeClass::Get_Transform` (202-208);
**   mp_asset_scale.inc    GameEngineDevice/Source/W3DDevice/GameClient/W3DAssetManager.cpp, `ident_scale` and
**                         `scale_epsilon` (84-85).
** and since GitHub #174 (the live root and the captured bones):
**   mp_cache_attach_bone.inc  W3DModelDraw.h, `#define CACHE_ATTACH_BONE` (278);
**   mp_acbits.inc         W3DModelDraw.cpp, `enum ACBits` (221-233);
**   mp_attach_offset.inc  W3DModelDraw.cpp, `W3DModelDrawModuleData::getAttachToDrawableBoneOffset` (1148-1173);
**   mp_adjust.inc         W3DModelDraw.cpp, `adjustTransformMtx` (1993-2036);
**   mp_draw_scale.inc     W3DModelDraw.cpp, doDrawModule's instance scale (2045-2054);
**   mp_draw_set.inc       W3DModelDraw.cpp, doDrawModule's `Set_Transform` (2093-2100);
**   mp_current_bones.inc  W3DModelDraw.cpp, `getCurrentBonePositions` (3569-3646, `DO_INV` defined in the cut);
**   mp_react.inc          W3DModelDraw.cpp, `reactToTransformChange` (3647-3678; the track branch is not reached);
**   mp_capture_update.inc  Libraries/Source/WWVegas/WW3D2/pivot.cpp, `PivotClass::Capture_Update` (137-171);
**   mp_htree_capture.inc  htree.cpp, `HTreeClass::Capture_Bone`, `Release_Bone`, `Is_Bone_Captured`, `Control_Bone`
**                         (1001-1053).
** No edit. Compiled on stand-ins (namespace mp): the drawable (its id, hidden flag, status bits, scale), the module data
** (`m_particlesAttachedToAnimatedBones`), the states (their `m_particleSysBones`), `ParticleSystem`'s members (the cut
** methods are its own), the particle system manager (`createParticleSystem`: NULL for a NULL template, ids counting up,
** the template's slave made inside, as ParticleSys.cpp:2968-2977 and ParticleSystem's constructor; `findParticleSystem`
** by id; a system dropped as `~ParticleSystem` unlinks it from its master and slave), and the render object
** (`Get_Bone_Index` as htree.cpp:922-930 for an HLOD with a hierarchy, else 0; `Get_Bone_Transform(i)` =
** `Transform * pose[i]` by WWMath's Matrix3D::Multiply; `Set_Transform`, `Get_Transform`, `Set_Hidden`). The shadow,
** decal and track pointers stay NULL.
** ~~The render object's `Get_Bone_Transform(i)` = `Transform * pose[i]`; the driver gives it `world * Scale(scale)`.~~
** **Corrected 2026-11-12 (#172):** that was the port's picture of the scale, not upstream's. The render object is now
** made as `W3DAssetManager::Create_Render_Obj(name, scale, color)` makes it (W3DAssetManager.cpp:719-895): its own copy
** of the model's hierarchy (pivot 0 the root, each other pivot's parent a model word, so chains up to 5 deep), and when
** `reallyscale = (WWMath::Fabs(scale - ident_scale) > scale_epsilon)` (738, written here: the line occurs twice)
** `rendobj->Scale(scale)` (858-859), whose `HLodClass::Scale` (hlod.cpp:3264-3290) has only the hierarchy to scale here
** (`HTree->Scale(scale)`, cut). `Get_Bone_Transform(i)` is `Animatable3DObjClass`'s (animobj.cpp:658-675): with a
** hierarchy, `Update_Sub_Object_Transforms`'s BASE_POSE `Base_Update(Transform)` (animobj.cpp:792-794, cut) then
** `HTree->Get_Transform(i)` (cut); without one (a mesh, or an HLOD without pivots) the render object's `Transform`.
** The driver gives the render object the drawable's transform `world`, no template scale (doDrawModule 2094-2100). setModelState's other parts are the wrapper's: the early return for the current
** state (the only one reachable: the states have no transition keys) and the render object of the new state's model.
** (#174) The pivots are `PivotClass`'s non-`LAZY_CAP_MTX_ALLOC` members (pivot.h:79-99) with `Is_Captured`
** (pivot.h:69-76) and the cut `Capture_Update`; the render object's `Capture_Bone` and `Control_Bone` are
** `Animatable3DObjClass`'s (animobj.cpp:690-695, 752-765: to the hierarchy when it has one; `Set_Hierarchy_Valid` is
** moot, `Get_Bone_Transform` runs `Base_Update` each call) and `Set_ObjectScale` is rendobj.h:454's. The drawable has
** `getInstanceScale()` = `getScale()` = `m_instanceScale` (Drawable.h:404, Drawable.cpp:1093-1097), its transform
** (`getTransformMatrix`), an object or none (`getConstructionPercent`, `getGeometryInfo().getMaxHeightAbovePosition()`,
** `testStatus`, `isSignificantlyAboveTerrain`), and `getPristineBonePositions(name, 0, NULL, &mtx, 1)` answering the
** case's attach bone (found or not, its matrix). The module data has `m_attachToDrawableBone` and the cache; every
** state has the case's `m_flags` (all states the same: `ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT` or none).
** `sinf`, `cosf`, `atan2` are the Rust's petir functions in the test binary.
**
** Entry point (extern "C", tests/differential_model_draw_particles.rs; floats travel as their bits):
**   ref_model_draw_particles_seq(in, nin, out, cap): in = [drawable id, scale, particlesAttachedToAnimatedBones,
**     4 templates' slave (-1 none, else a later template's index), 3 models x (hlod, bone count n <= 6, then 6 bones x
**     (name code, 12 pose words)), state count s (1..4), 4 states x (model code (0..2 a model, 3 None, 4 missing), bone
**     list count (<= 3), 3 x (bone name code, template code (0..3, 4 None, 5 unknown)))], then steps of 14 words [op,
**     a1, 12 words]: 0 setModelState(state a1 % s); 1 doDrawModule with the render object's transform ~~world *
**     Scale(scale)~~ world (#172) (world = the 12 words); 2 updateBonesForClientParticleSystems with the same transform
**     (AnimatedParticleSysBone's call); 3 the drawable's hidden = a1 then setHidden(a1); 4 setFullyObscuredByShroud(a1);
**     5 the system a1 % count (in id order) dropped; 6 the drawable's status NO_STATE_PARTICLES = a1; (#172) 7
**     getCurrentWorldspaceClientBonePositions(STATE_BONE_NAMES[a1 % 6]) with the render object's transform world,
**     recorded as [200, found, the 12 words (zeros when not found)]. The constructor runs setModelState(state 0) first.
**     (#172) Each model bone is (name code, parent (the pivot's parent index, < its own; unused for pivot 0), 12 pose
**     words). Records: after the constructor and after each step [100, needRecalc, fullyObscured, tracker count, then
**     (id, boneIndex) each, system count, then per system in id order: id, attached drawable, saveable, stopped,
**     destroyed, skipParentXfrm, isLocalIdentity, slave id (0 none), master id (0 none), the local transform's 12
**     words].
**   (#174) The header gains, after particlesAttachedToAnimatedBones, [AttachToBoneInAnotherModule (0/1),
**     attach bone found (0/1), its 12 words, ADJUST_HEIGHT flag (0/1), has object (0/1), construction percent, max
**     height]; the models have 9 bone name codes. Ops 2 and 7 now first set the drawable's transform to world and call
**     reactToTransformChange (the render object's transform is the module's, no longer the driver's); op 1 sets the
**     drawable's transform then doDrawModule(&world). New ops: 8 Capture_Bone(b) and Control_Bone(b, the 12 words) on
**     the render object, b = a1 % its bone count (only an HLOD with bones); 9 setInstanceScale(word 2's float); 10
**     getCurrentBonePositions(PREFIXES[a1 % 7], start word 2, positions NULL if word 4 is 0, transforms NULL if word 5 is
**     0, maxBones word 3) into 64-entry arrays, recorded as [300, count, then for each of min(count, 64): the position's
**     3 words (0 if NULL) and the transform's 12 (0 if NULL)]. Each state record ends with [render object present, its
**     Transform's 12 words, ObjectScale] (zeros without one).
**   (#179 item 3, Drawable::draw's root) Cut verbatim too, from GameEngine/Source/GameClient/Drawable.cpp:
**     mp_draw_root.inc      `Drawable::draw` from `// call the database defined draw action method` to its end
**                           (2660-2680: the instance matrix's postMul, applyPhysicsXform, every doDrawModule);
**     mp_apply_physics.inc  `applyPhysicsXform` (1388-1410);
**     mp_set_instance.inc   `setInstanceMatrix` (4281-4293);
**   and from GameEngine/Include/GameClient/Drawable.h: mp_physics_info.inc, `struct PhysicsXformInfo` (613-621).
**   Stand-ins: the drawable's `m_instance`, `m_instanceIsIdentity`, `getInstanceMatrix`, `isInstanceIdentity`
**   (Drawable.h:400-402), `getDrawModules` (the case's module behind a recorder of the root it is handed),
**   `calcPhysicsXform` (the case's answer and totals: the locomotor models are drawable_loco.rs's, not compared here),
**   ~~the object's `isDisabledByType` (never held), `TheGlobalData->m_showClientPhysics` (on) and the view's and script
**   engine's time freezes (none).~~ **Corrected 2026-11-13 (#182 item 3):** the object's `isDisabledByType`,
**   `TheGlobalData->m_showClientPhysics`, the view's `isTimeFrozen` and `isCameraMovementFinished` and the script
**   engine's `isTimeFrozenDebug` and `isTimeFrozenScript` are the case's words (op 13's words 6..11; a case starts not
**   held, shown, not frozen, the camera finished). The drawable starts as `setInstanceMatrix(NULL)` with no physics. New ops: 11 the
**   drawable's transform = the 12 words, then `Drawable::draw` from its root (recorded as [400, the root's 12 words]
**   before the module's doDrawModule); 12 `setInstanceMatrix(a1 ? &words : NULL)`; 13 `calcPhysicsXform` answers a1
**   with totals pitch, roll, yaw, z = words 2..5 for the next draws; (#182 item 3) and words 6..11 are the gates for
**   them: held, showClientPhysics, isTimeFrozen, isCameraMovementFinished, isTimeFrozenDebug, isTimeFrozenScript.
**   (#182, the instance matrices the logic's updates set) Cut verbatim too:
**     mp_collapse_wait.inc  GameEngine/Source/GameLogic/Object/Update/StructureCollapseUpdate.cpp, the waiting shudder
**                           from `shudder.Set(..., 0);` to `if (now >= m_collapseFrame)` (189-197);
**     mp_collapse_fall.inc  StructureCollapseUpdate.cpp, the collapsing shudder from `shudder.Set(..., m_currentHeight);`
**                           to `if (now >= m_burstFrame)` (217-224);
**     mp_collapse_done.inc  StructureCollapseUpdate.cpp, the final zero from `shudder.Set(0, 0, 0);` to `return
**                           UPDATE_SLEEP_FOREVER;` (259-265);
**     mp_float_bob.inc      GameEngine/Source/GameLogic/Object/Update/FloatUpdate.cpp, the drawable's bob from `Drawable
**                           *draw = getObject()->getDrawable();` to `return UPDATE_SLEEP_NONE;` (123-140).
**   Stand-ins: the building and `getObject()` (their `getDrawable()` the case's drawable), the module data's
**   `m_maxShudder`, `m_currentHeight`, `GameClientRandomValueReal` (the case's words in call order; which of them is x
**   is the compiler's order of `shudder.Set`'s arguments, which C++ leaves unspecified: `ref_mdp_shudder_order` tells
**   the test) and `TheGameLogic->getFrame()`; `INT_TO_REAL` is BaseType.h:240's. `sin` of a `Real` is C's double `sin`
**   (the shim's math.h: MSVC's C semantics), petir's in the test binary. New ops: 14 the collapse's translation of
**   phase a1 % 3 (0 waiting, 1 collapsing, 2 done) with the random values words 2, 3 and the height word 4; 15 the bob
**   at frame a1. Both recorded as [500, isInstanceIdentity, the instance matrix's 12 words]. */
#include <stdio.h>
#include <stdlib.h>
#include <assert.h>
#include <string.h>
#include <strings.h>
#include <stddef.h>
#include <vector>
#include <algorithm>
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/GameType.h"
#include "matrix3d.h"
#include "vector3.h"
#include "wwmath.h"
/* (#182) Libraries/Include/Lib/BaseType.h:240, which the shim's BaseType.h does not carry */
#ifndef INT_TO_REAL
#define INT_TO_REAL(x)						((Real) (x))
#endif

#ifndef stricmp
#define stricmp strcasecmp
#endif

namespace mp {

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

#include "mp_psid.inc"

/* GameEngine/Include/GameClient/Drawable.h:237 */
enum DrawableStatus { DRAWABLE_STATUS_NO_STATE_PARTICLES = 0x00000008 };

#include "mp_cache_attach_bone.inc"
#include "mp_acbits.inc"

/* (#174) the object adjustTransformMtx and reactToTransformChange read */
enum ObjectStatusTypes { OBJECT_STATUS_STEALTHED = 1 };
/* (#179 item 3) GameEngine/Include/Common/DisabledTypes.h's DisabledType, the one value applyPhysicsXform names */
enum DisabledType { DISABLED_HELD = 6 };
class GeometryInfo
{
public:
	Real m_maxHeight;
	Real getMaxHeightAbovePosition() const { return m_maxHeight; }
};
class Object
{
public:
	Real m_constructionPercent;
	GeometryInfo m_geometryInfo;
	/* Object.h:401 */
	Real getConstructionPercent() const { return m_constructionPercent; }
	const GeometryInfo& getGeometryInfo() const { return m_geometryInfo; }
	Bool testStatus( ObjectStatusTypes ) const { abort(); return FALSE; }
	Bool isSignificantlyAboveTerrain() const { abort(); return FALSE; }
	/* (#179 item 3) applyPhysicsXform's DISABLED_HELD test: ~~never held here (the port's gate is
	** physics_xform_facts', read from the World)~~ (#182 item 3) the case's word */
	Bool m_held;
	Bool isDisabledByType( DisabledType ) const { return m_held; }
};
Object g_object;
Bool g_hasObject;

/* (#179 item 3) what applyPhysicsXform reads besides the object: ShowClientPhysics, the tactical view's and the
** script engine's time freezes. ~~(on; none: the port's client has no frozen time, drawable_loco.rs's module docs)~~
** (#182 item 3) the case's words (op 13) */
struct GlobalData { Bool m_showClientPhysics; };
GlobalData g_globalData = { TRUE };
GlobalData *TheGlobalData = &g_globalData;
struct View
{
	Bool m_frozen, m_finished;
	Bool isTimeFrozen() const { return m_frozen; }
	Bool isCameraMovementFinished() const { return m_finished; }
};
View g_view = { FALSE, TRUE };
View *TheTacticalView = &g_view;
struct ScriptEngine
{
	Bool m_debug, m_script;
	Bool isTimeFrozenDebug() const { return m_debug; }
	Bool isTimeFrozenScript() const { return m_script; }
};
ScriptEngine g_scriptEngine = { FALSE, FALSE };
ScriptEngine *TheScriptEngine = &g_scriptEngine;
/* the draw module list Drawable::draw walks: the case's W3DModelDraw behind a recorder of the root it is handed */
class DrawModule;
Bool g_attachFound;
Matrix3D g_attachMtx;

class Drawable
{
public:
	DrawableID m_id;
	Bool m_hidden;
	UnsignedInt m_status;
	Real m_scale;
	Coord3D m_pos;
	Matrix3D m_transform;
	DrawableID getID() const { return m_id; }
	/* Drawable.h:329 (m_hiddenByStealth is false here) */
	Bool isDrawableEffectivelyHidden() const { return m_hidden; }
	/* Drawable.h:368 */
	Bool testDrawableStatus( DrawableStatus bit ) const { return (m_status & bit) != 0; }
	/* Drawable.cpp:1093-1097 and Drawable.h:404: both m_instanceScale */
	const Real getScale( void ) const { return m_scale; }
	Real getInstanceScale( void ) const { return m_scale; }
	const Coord3D *getPosition() const { return &m_pos; }
	const Matrix3D *getTransformMatrix() const { return &m_transform; }
	Object *getObject() const { return g_hasObject ? &g_object : NULL; }
	/* (#179 item 3) Drawable.h:400-402, 611-623 (PhysicsXformInfo cut) and the members m_instance, m_instanceIsIdentity */
	Matrix3D m_instance;
	Bool m_instanceIsIdentity;
	void setInstanceMatrix( const Matrix3D *instance );
	const Matrix3D *getInstanceMatrix( void ) const { return &m_instance; }
	inline Bool isInstanceIdentity() const { return m_instanceIsIdentity; }
	void applyPhysicsXform(Matrix3D* mtx);
#include "mp_physics_info.inc"
	/* calcPhysicsXform: the case's answer (its totals, and whether a locomotor appearance ran); the models themselves
	** are drawable_loco.rs's, not compared here */
	Bool calcPhysicsXform(PhysicsXformInfo& info);
	DrawModule *m_modules[2];
	DrawModule** getDrawModules() { return m_modules; }
	/* Drawable::draw from its root on (the cut), with the early returns before it taken */
	void drawRoot();
	/* (#174) the case's attach bone, as the drawable's pristine bones answer it for (name, 0, NULL, &mtx, 1) */
	Int getPristineBonePositions(const char*, Int startIndex, Coord3D* positions, Matrix3D* transforms, Int maxBones) const
	{
		if (startIndex != 0 || positions != NULL || transforms == NULL || maxBones != 1) abort();
		if (!g_attachFound) return 0;
		*transforms = g_attachMtx;
		return 1;
	}
};
Drawable g_drawable;

class ParticleSystemTemplate
{
public:
	const ParticleSystemTemplate *m_slave;
};
ParticleSystemTemplate g_templates[4];

/* ParticleSys.h's members the cut methods use; the methods are the cuts' */
class ParticleSystem
{
public:
	ParticleSystemID m_systemID;
	Matrix3D m_localTransform;
	Bool m_isLocalIdentity, m_isStopped, m_isDestroyed, m_isSaveable, m_skipParentXfrm;
	DrawableID m_attachedToDrawableID;
	ObjectID m_attachedToObjectID;
	ParticleSystem *m_slaveSystem, *m_masterSystem;
	ParticleSystem( ParticleSystemID id ) : m_systemID(id), m_localTransform(true), m_isLocalIdentity(true), m_isStopped(false),
		m_isDestroyed(false), m_isSaveable(true), m_skipParentXfrm(false), m_attachedToDrawableID(INVALID_DRAWABLE_ID),
		m_attachedToObjectID(INVALID_ID), m_slaveSystem(NULL), m_masterSystem(NULL) {}
	/* ParticleSys.h:539 */
	inline ParticleSystemID getSystemID( void ) const { return m_systemID; }
	void setPosition( const Coord3D *pos );
	void setLocalTransform( const Matrix3D *matrix );
	void rotateLocalTransformX( Real x );
	void rotateLocalTransformY( Real y );
	void rotateLocalTransformZ( Real z );
	/* ParticleSys.h:547 */
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
	/* ParticleSys.cpp:2968-2977, and ParticleSystem's constructor's slave (1160-1175: made before the system is listed) */
	ParticleSystem *createParticleSystem( const ParticleSystemTemplate *sysTemplate, Bool createSlaves = TRUE )
	{
		if (sysTemplate == NULL)
			return NULL;
		m_uniqueSystemID = m_uniqueSystemID + 1;
		ParticleSystem *sys = new ParticleSystem((ParticleSystemID)m_uniqueSystemID);
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
	/* ParticleSys.cpp:2996-3015 */
	ParticleSystem *findParticleSystem( ParticleSystemID id )
	{
		if (id == INVALID_PARTICLE_SYSTEM_ID)
			return NULL;
		for (size_t i = 0; i < m_list.size(); ++i)
			if (m_list[i]->getSystemID() == id)
				return m_list[i];
		return NULL;
	}
	/* ~ParticleSystem (ParticleSys.cpp:1190-1229): unlinked from its slave and master, then unlisted */
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

/* ---- the render object ---- */
struct FakeModel
{
	Bool hlod;
	Int n;
	char names[6][16];
	Int parent[6];
	Matrix3D pose[6];
};

/* (#172) the hierarchy: PivotClass's members that Base_Update and HTreeClass::Scale read (pivot.h:79-99; ~~never
** captured here~~ (#174) the non-LAZY_CAP_MTX_ALLOC capture members too, as PivotClass() sets them, pivot.cpp:58-76),
** and HTreeClass's (htree.h) */
class PivotClass
{
public:
	PivotClass *Parent;
	Matrix3D BaseTransform;
	Matrix3D Transform;
	Matrix3D CapTransform;
	int Index;
	bool IsVisible;
	bool WorldSpaceTranslation;
	bool IsCaptured;
	PivotClass() : Parent(NULL), BaseTransform(true), Transform(true), CapTransform(true), Index(0), IsVisible(true),
		WorldSpaceTranslation(false), IsCaptured(false) {}
	void Capture_Update( void );
	/* pivot.h:69-76 (no LAZY_CAP_MTX_ALLOC) */
	bool Is_Captured() const { return IsCaptured; }
};
#include "mp_capture_update.inc"
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
	void Capture_Bone(int boneindex);
	void Release_Bone(int boneindex);
	bool Is_Bone_Captured(int boneindex) const;
	void Control_Bone(int boneindex,const Matrix3D & relative_tm,bool world_space_translation = false);
};
#include "mp_base_update.inc"
#include "mp_htree_scale.inc"
#include "mp_get_transform.inc"
#include "mp_htree_capture.inc"
#include "mp_asset_scale.inc"
FakeModel g_models[3];
const char *const BONE_NAMES[] = { "RootTransform", "Bone01", "Smoke01", "FX_A", "Muzzle", "SMOKE01", "Bone02", "BONE03", "BONE" };
/* (#174) getCurrentBonePositions' prefixes */
const char *const PREFIXES[] = { "Bone", "bone", "Smoke", "FX_A", "Muzzle", "RootTransform", "nosuch" };
const char *const STATE_BONE_NAMES[] = { "RootTransform", "bone01", "SMOKE01", "fx_a", "Muzzle", "nosuchbone" };

class RenderObjClass
{
public:
	const FakeModel *m;
	HTreeClass HTree;
	Matrix3D Transform;
	Bool hidden;
	float ObjectScale;
	RenderObjClass() : m(NULL), Transform(true), hidden(FALSE), ObjectScale(1.0f) {}
	/* (#172) W3DAssetManager::Create_Render_Obj(name, scale, color) for model `fm` (see the header) */
	void Create( const FakeModel *fm, float scale )
	{
		m = fm;
		HTree = HTreeClass();
		ObjectScale = 1.0f;
		HTree.NumPivots = fm->n;
		for (int i = 0; i < fm->n; i++) {
			HTree.Pivot[i].BaseTransform = fm->pose[i];
			HTree.Pivot[i].Parent = i == 0 ? NULL : &HTree.Pivot[fm->parent[i]];
		}
		Bool reallyscale = (WWMath::Fabs(scale - ident_scale) > scale_epsilon);
		if (reallyscale)
			Scale(scale);	//this also makes it unique
	}
	/* HLodClass::Scale (hlod.cpp:3264-3290): no LOD models here, the hierarchy */
	void Scale( float scale )
	{
		if (scale==1.0f) return;
		HTree.Scale(scale);
	}
	int Get_Bone_Index( const char *name )
	{
		if (!m->hlod || m->n == 0)
			return 0;
		for (int i = 0; i < m->n; i++)
			if (stricmp(m->names[i], name) == 0)
				return i;
		return 0;
	}
	const Matrix3D &Get_Transform( void ) const { return Transform; }
	void Set_Transform( const Matrix3D &m3 ) { Transform = m3; }
	/* rendobj.h:454 */
	void Set_ObjectScale( float scale ) { ObjectScale = scale; }
	/* (#174) Animatable3DObjClass::Capture_Bone and Control_Bone (animobj.cpp:690-695, 752-765): to the hierarchy, which
	** an HLOD with pivots has */
	void Capture_Bone( int boneindex ) { if (m->hlod && m->n != 0) HTree.Capture_Bone(boneindex); }
	void Control_Bone( int bindex, const Matrix3D & objtm, bool world_space_translation = false )
	{
		if (m->hlod && m->n != 0) HTree.Control_Bone(bindex, objtm, world_space_translation);
	}
	/* Animatable3DObjClass::Get_Bone_Transform (animobj.cpp:658-675): BASE_POSE's Base_Update(Transform)
	** (animobj.cpp:792-794), then the hierarchy's; without one, the render object's Transform */
	const Matrix3D &Get_Bone_Transform( int i )
	{
		if (!m->hlod || m->n == 0)
			return Transform;
		HTree.Base_Update(Transform);
		return HTree.Get_Transform(i);
	}
	void Set_Hidden( int h ) { hidden = h != 0; }
};
RenderObjClass g_ro;

/* (#174) never made here (m_trackRenderObject stays NULL): compiled for reactToTransformChange's branch */
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

struct ParticleSysBoneInfo
{
	AsciiString boneName;
	const ParticleSystemTemplate *particleSystemTemplate;
};
struct ModelConditionInfo
{
	Int model;
	std::vector<ParticleSysBoneInfo> m_particleSysBones;
	/* (#174) the ACBits flags */
	Int m_flags;
};
ModelConditionInfo g_states[4];
class W3DModelDrawModuleData
{
public:
	Bool m_particlesAttachedToAnimatedBones;
	/* (#174) W3DModelDraw.h:290-324 (CACHE_ATTACH_BONE) */
	AsciiString m_attachToDrawableBone;
	mutable Vector3 m_attachToDrawableBoneOffset;
	mutable Bool m_attachToDrawableBoneOffsetValid;
	const Vector3* getAttachToDrawableBoneOffset(const Drawable* draw) const;
};
W3DModelDrawModuleData g_data;
#include "mp_attach_offset.inc"

class W3DModelDraw
{
public:
#include "mp_tracker.inc"
	typedef std::vector<ParticleSysTrackerType> ParticleSystemIDVec;
	const ModelConditionInfo *m_curState;
	Bool m_needRecalcBoneParticleSystems;
	Bool m_fullyObscuredByShroud;
	Bool m_shadowEnabled;
	RenderObjClass *m_renderObject;
	Shadow *m_shadow;
	Shadow *m_terrainDecal;
	TerrainTracksRenderObjClass *m_trackRenderObject;
	ParticleSystemIDVec m_particleSystemIDs;
	/* (#174) getCurrentBonePositions' second loop reuses its first loop's `i` (MSVC 6's for-scope: the first loop's
	** variable lives on). Unedited, the cut's `for (i = 0; ...)` finds this member instead; it is set to 0 before it is
	** read, as MSVC's would be. */
	mutable Int i;

	W3DModelDraw() : m_curState(NULL), m_shadowEnabled(TRUE), m_renderObject(NULL), m_shadow(NULL), m_terrainDecal(NULL), m_trackRenderObject(NULL)
	{
#include "mp_ctor_flags.inc"
		setModelState(&g_states[0]);
	}
	Drawable *getDrawable() const { return &g_drawable; }
	const W3DModelDrawModuleData *getW3DModelDrawModuleData() const { return &g_data; }

	void doStartOrStopParticleSys();
	void setHidden(Bool hidden);
	void setFullyObscuredByShroud(Bool fullyObscured);
	void stopClientParticleSystems();
	void recalcBonesForClientParticleSystems();
	Bool updateBonesForClientParticleSystems();
	Bool getCurrentWorldspaceClientBonePositions(const char* boneName, Matrix3D& transform) const;
	void adjustTransformMtx(Matrix3D& mtx) const;
	Int getCurrentBonePositions(const char* boneNamePrefix, Int startIndex, Coord3D* positions, Matrix3D* transforms, Int maxBones) const;
	void reactToTransformChange(const Matrix3D* oldMtx, const Coord3D* oldPos, Real oldAngle);

	/* setModelState: the early return for the current state, the cut, then the new state's render object */
	void setModelState( const ModelConditionInfo *newState )
	{
		if (m_curState != NULL && newState == m_curState)
			return;
#include "mp_set_model_state.inc"
		/* (#174) nukeCurrentRender(&transform) (2787-2818): the old render object's transform, else the drawable's */
		Matrix3D transform = m_renderObject ? m_renderObject->Get_Transform() : *getDrawable()->getTransformMatrix();
		m_renderObject = newState->model < 3 ? &g_ro : NULL;
		if (m_renderObject) {
			/* (#172) Create_Render_Obj(newState->m_modelName.str(), draw->getScale(), m_hexColor) (3034) */
			g_ro.Create(&g_models[newState->model], getDrawable()->getScale());
			/* (#174) m_renderObject->Set_Transform(transform) (3155) */
			g_ro.Set_Transform(transform);
		}
		m_curState = newState;
	}
	/* doDrawModule: (#174) the instance scale and the render object's transform (cut), then the cut */
	void doDrawModule( const Matrix3D* transformMtx )
	{
#include "mp_draw_scale.inc"
#include "mp_draw_set.inc"
#include "mp_draw.inc"
	}
	void state()
	{
		rec({100, m_needRecalcBoneParticleSystems ? 1 : 0, m_fullyObscuredByShroud ? 1 : 0, (int)m_particleSystemIDs.size()});
		for (size_t i = 0; i < m_particleSystemIDs.size(); ++i) rec({(int)m_particleSystemIDs[i].id, m_particleSystemIDs[i].boneIndex});
		std::vector<ParticleSystem *> l = g_psm.m_list;
		std::sort(l.begin(), l.end(), [](ParticleSystem *a, ParticleSystem *b) { return a->m_systemID < b->m_systemID; });
		rec({(int)l.size()});
		for (size_t i = 0; i < l.size(); ++i) {
			ParticleSystem *s = l[i];
			rec({(int)s->m_systemID, (int)s->m_attachedToDrawableID, s->m_isSaveable ? 1 : 0, s->m_isStopped ? 1 : 0, s->m_isDestroyed ? 1 : 0,
				s->m_skipParentXfrm ? 1 : 0, s->m_isLocalIdentity ? 1 : 0, s->m_slaveSystem ? (int)s->m_slaveSystem->m_systemID : 0,
				s->m_masterSystem ? (int)s->m_masterSystem->m_systemID : 0});
			for (int r = 0; r < 3; ++r)
				for (int c = 0; c < 4; ++c) rec({bits(s->m_localTransform[r][c])});
		}
		/* (#174) the render object's transform and object scale */
		rec({m_renderObject ? 1 : 0});
		for (int r = 0; r < 3; ++r)
			for (int c = 0; c < 4; ++c) rec({m_renderObject ? bits(m_renderObject->Transform[r][c]) : 0});
		rec({m_renderObject ? bits(m_renderObject->ObjectScale) : 0});
	}
};
#include "mp_start_stop.inc"
#include "mp_shroud.inc"
#include "mp_stop.inc"
#include "mp_bones.inc"
#include "mp_worldspace_bone.inc"
#include "mp_adjust.inc"
#include "mp_current_bones.inc"
#include "mp_react.inc"

/* (#179 item 3) */
Bool g_physicsHas;
Drawable::PhysicsXformInfo g_physicsInfo;
Bool Drawable::calcPhysicsXform(PhysicsXformInfo& info)
{
	info = g_physicsInfo;
	return g_physicsHas;
}
#include "mp_apply_physics.inc"
#include "mp_set_instance.inc"
class DrawModule
{
public:
	W3DModelDraw *m_draw;
	/* the root recorded as [400, its 12 words], then the module's doDrawModule */
	void doDrawModule( const Matrix3D* transformMtx )
	{
		rec({400});
		for (int r = 0; r < 3; ++r)
			for (int c = 0; c < 4; ++c) rec({bits((*transformMtx)[r][c])});
		m_draw->doDrawModule(transformMtx);
	}
};
DrawModule g_drawModule;
void Drawable::drawRoot()
{
#include "mp_draw_root.inc"

/* (#182) StructureCollapseUpdate's and FloatUpdate's stand-ins (see the header) */
struct CollapseData { Real m_maxShudder; };
Real g_clientRandom[2];
int g_clientRandomAt;
Real GameClientRandomValueReal(Real lo, Real hi)
{
	(void)lo;
	(void)hi;
	if (g_clientRandomAt >= 2) abort();
	return g_clientRandom[g_clientRandomAt++];
}
struct Building { Drawable *getDrawable() const { return &g_drawable; } };
Building g_building;
struct GameLogicStandIn { UnsignedInt m_frame; UnsignedInt getFrame() const { return m_frame; } };
GameLogicStandIn g_gameLogic;
GameLogicStandIn *TheGameLogic = &g_gameLogic;
void collapseShudder(int phase, Real x, Real y, Real height)
{
	CollapseData data;
	data.m_maxShudder = 1.0f;
	const CollapseData *d = &data;
	(void)d;
	Building *building = &g_building;
	Real m_currentHeight = height;
	(void)m_currentHeight;
	g_clientRandom[0] = x;
	g_clientRandom[1] = y;
	g_clientRandomAt = 0;
	if (phase == 0) {
		Vector3 shudder;
#include "mp_collapse_wait.inc"
	} else if (phase == 1) {
		Vector3 shudder;
#include "mp_collapse_fall.inc"
	} else {
		Vector3 shudder;
#include "mp_collapse_done.inc"
	}
}
struct FloatUpdateStandIn
{
	Building *getObject() const { return &g_building; }
	void bob()
	{
#include "mp_float_bob.inc"
	}
};
void recInstance()
{
	rec({500, g_drawable.m_instanceIsIdentity ? 1 : 0});
	for (int r = 0; r < 3; ++r)
		for (int c = 0; c < 4; ++c) rec({bits(g_drawable.m_instance[r][c])});
}

Matrix3D matrix(const int *w)
{
	Matrix3D m(true);
	for (int r = 0; r < 3; ++r)
		for (int c = 0; c < 4; ++c) m[r][c] = real(w[r * 4 + c]);
	return m;
}

int copy_out(int *out, int cap)
{
	int n = (int)g_out.size();
	if (n > cap) n = cap;
	if (n > 0) memcpy(out, &g_out[0], n * sizeof(int));
	return (int)g_out.size();
}

} /* namespace mp */

extern "C" {

/* (#182) Which of the case's two client random values this compiler gives `shudder`'s x: 0 the first drawn (its
** arguments evaluated left to right), 1 the second. C++ leaves the order unspecified; the waiting cut is run on 1 and 2
** and the drawable put back as it was. */
int ref_mdp_shudder_order(void)
{
	using namespace mp;
	Matrix3D keep = g_drawable.m_instance;
	Bool keepIdentity = g_drawable.m_instanceIsIdentity;
	collapseShudder(0, 1.0f, 2.0f, 0.0f);
	Real x = g_drawable.m_instance[0][3];
	g_drawable.m_instance = keep;
	g_drawable.m_instanceIsIdentity = keepIdentity;
	return x == 1.0f ? 0 : x == 2.0f ? 1 : -1;
}

int ref_model_draw_particles_seq(const int *in, int nin, int *out, int cap)
{
	using namespace mp;
	g_out.clear();
	g_psm.clear();
	int k = 0;
	g_drawable.m_id = (DrawableID)in[k++];
	g_drawable.m_scale = real(in[k++]);
	g_drawable.m_hidden = FALSE;
	g_drawable.m_status = 0;
	g_drawable.m_transform.Make_Identity();
	/* (#179 item 3) Drawable's constructor: setInstanceMatrix(NULL); no physics until the case says */
	g_drawable.setInstanceMatrix(NULL);
	g_physicsHas = FALSE;
	g_physicsInfo = Drawable::PhysicsXformInfo();
	/* (#182 item 3) the gates open until op 13 says */
	g_object.m_held = FALSE;
	g_globalData.m_showClientPhysics = TRUE;
	g_view.m_frozen = FALSE;
	g_view.m_finished = TRUE;
	g_scriptEngine.m_debug = FALSE;
	g_scriptEngine.m_script = FALSE;
	g_data.m_particlesAttachedToAnimatedBones = in[k++] != 0;
	/* (#174) */
	g_data.m_attachToDrawableBone = in[k++] ? "attachbone" : "";
	g_data.m_attachToDrawableBoneOffsetValid = false;
	g_attachFound = in[k++] != 0;
	g_attachMtx = matrix(in + k);
	k += 12;
	int stateFlags = in[k++] ? (1 << ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT) : 0;
	g_hasObject = in[k++] != 0;
	g_object.m_constructionPercent = real(in[k++]);
	g_object.m_geometryInfo.m_maxHeight = real(in[k++]);
	for (int t = 0; t < 4; ++t) {
		int sl = in[k++];
		g_templates[t].m_slave = sl < 0 ? NULL : &g_templates[sl];
	}
	for (int mi = 0; mi < 3; ++mi) {
		g_models[mi].hlod = in[k++] != 0;
		g_models[mi].n = in[k++];
		for (int b = 0; b < 6; ++b) {
			int code = in[k++];
			strcpy(g_models[mi].names[b], BONE_NAMES[code]);
			g_models[mi].parent[b] = in[k++];
			g_models[mi].pose[b] = matrix(in + k);
			k += 12;
		}
	}
	int nstates = in[k++];
	for (int si = 0; si < 4; ++si) {
		g_states[si].model = in[k++];
		g_states[si].m_flags = stateFlags;
		int nb = in[k++];
		g_states[si].m_particleSysBones.clear();
		/* parseConditionState (W3DModelDraw.cpp:1470-1480): a ConditionState starts as a copy of the default state, its
		** ParticleSysBones included; its own lines add to them */
		if (si > 0) g_states[si].m_particleSysBones = g_states[0].m_particleSysBones;
		for (int b = 0; b < 3; ++b) {
			int bone = in[k++];
			int tmpl = in[k++];
			if (b >= nb) continue;
			/* parseParticleSysBone (W3DModelDraw.cpp:1351-1359): the bone lower-cased, the template found or NULL */
			ParticleSysBoneInfo info;
			info.boneName = STATE_BONE_NAMES[bone];
			info.boneName.toLower();
			info.particleSystemTemplate = tmpl < 4 ? &g_templates[tmpl] : NULL;
			g_states[si].m_particleSysBones.push_back(info);
		}
	}
	W3DModelDraw *draw = new W3DModelDraw();
	g_drawModule.m_draw = draw;
	g_drawable.m_modules[0] = &g_drawModule;
	g_drawable.m_modules[1] = NULL;
	draw->state();
	for (int st = k; st + 13 < nin; st += 14) {
		const int *a = in + st;
		switch (a[0]) {
		case 0: draw->setModelState(&g_states[a[1] % nstates]); break;
		case 1:
		case 2: {
			/* (#172) the drawable's transform: the template scale is in the render object's hierarchy; (#174) the
			** drawable's transform: doDrawModule(&transformMtx) makes the render object's, reactToTransformChange too */
			Matrix3D mtx = matrix(a + 2);
			g_drawable.m_transform = mtx;
			if (a[0] == 1) {
				Matrix3D transformMtx = *g_drawable.getTransformMatrix();
				draw->doDrawModule(&transformMtx);
			} else {
				draw->reactToTransformChange(NULL, NULL, 0.0f);
				draw->updateBonesForClientParticleSystems();
			}
			break;
		}
		case 3:
			g_drawable.m_hidden = a[1] != 0;
			draw->setHidden(a[1] != 0);
			break;
		case 4: draw->setFullyObscuredByShroud(a[1] != 0); break;
		case 5:
			if (!g_psm.m_list.empty()) {
				std::vector<ParticleSystem *> l = g_psm.m_list;
				std::sort(l.begin(), l.end(), [](ParticleSystem *x, ParticleSystem *y) { return x->m_systemID < y->m_systemID; });
				g_psm.drop(l[(unsigned)a[1] % l.size()]);
			}
			break;
		case 6: g_drawable.m_status = a[1] ? DRAWABLE_STATUS_NO_STATE_PARTICLES : 0; break;
		case 8:
			/* (#174) handleClientTurretPositioning's (and handleClientRecoil's) calls on a bone */
			if (draw->m_renderObject && draw->m_renderObject->m->hlod && draw->m_renderObject->m->n != 0) {
				int bone = (unsigned)a[1] % (unsigned)draw->m_renderObject->m->n;
				draw->m_renderObject->Capture_Bone(bone);
				draw->m_renderObject->Control_Bone(bone, matrix(a + 2));
			}
			break;
		case 9: g_drawable.m_scale = real(a[2]); break;
		case 11:
			/* (#179 item 3) Drawable::draw with the drawable's transform = the words: its root (the instance matrix,
			** applyPhysicsXform) recorded and handed to doDrawModule */
			g_drawable.m_transform = matrix(a + 2);
			g_drawable.drawRoot();
			break;
		case 12:
			/* (#179 item 3) setInstanceMatrix(a1 ? &words : NULL) (SwayClientUpdate's, FloatUpdate's, ...) */
			if (a[1]) {
				Matrix3D m = matrix(a + 2);
				g_drawable.setInstanceMatrix(&m);
			} else {
				g_drawable.setInstanceMatrix(NULL);
			}
			break;
		case 13:
			/* (#179 item 3) the next draws' calcPhysicsXform: answers a1, totals pitch, roll, yaw, z = words 2..5 */
			g_physicsHas = a[1] != 0;
			g_physicsInfo.m_totalPitch = real(a[2]);
			g_physicsInfo.m_totalRoll = real(a[3]);
			g_physicsInfo.m_totalYaw = real(a[4]);
			g_physicsInfo.m_totalZ = real(a[5]);
			/* (#182 item 3) applyPhysicsXform's gates for the next draws */
			g_object.m_held = a[6] != 0;
			g_globalData.m_showClientPhysics = a[7] != 0;
			g_view.m_frozen = a[8] != 0;
			g_view.m_finished = a[9] != 0;
			g_scriptEngine.m_debug = a[10] != 0;
			g_scriptEngine.m_script = a[11] != 0;
			break;
		case 14:
			/* (#182) StructureCollapseUpdate's translation of phase a1 % 3 */
			collapseShudder((unsigned)a[1] % 3, real(a[2]), real(a[3]), real(a[4]));
			recInstance();
			break;
		case 15: {
			/* (#182) FloatUpdate's bob at frame a1 */
			g_gameLogic.m_frame = (UnsignedInt)a[1];
			FloatUpdateStandIn f;
			f.bob();
			recInstance();
			break;
		}
		case 10: {
			Coord3D pos[64];
			Matrix3D mtx[64];
			memset(pos, 0, sizeof(pos));
			for (int i = 0; i < 64; ++i) mtx[i].Make_Identity();
			Int n = draw->getCurrentBonePositions(PREFIXES[(unsigned)a[1] % 7], a[2], a[4] ? pos : NULL, a[5] ? mtx : NULL, a[3]);
			rec({300, n});
			for (int i = 0; i < n && i < 64; ++i) {
				rec({a[4] ? bits(pos[i].x) : 0, a[4] ? bits(pos[i].y) : 0, a[4] ? bits(pos[i].z) : 0});
				for (int r = 0; r < 3; ++r)
					for (int c = 0; c < 4; ++c) rec({a[5] ? bits(mtx[i][r][c]) : 0});
			}
			break;
		}
		default: {
			/* (#172) getCurrentWorldspaceClientBonePositions with the render object's transform world; (#174) the
			** render object's transform is the module's: the drawable's set to world, then reactToTransformChange */
			g_drawable.m_transform = matrix(a + 2);
			draw->reactToTransformChange(NULL, NULL, 0.0f);
			Matrix3D t(true);
			Bool found = draw->getCurrentWorldspaceClientBonePositions(STATE_BONE_NAMES[(unsigned)a[1] % 6], t);
			rec({200, found ? 1 : 0});
			for (int r = 0; r < 3; ++r)
				for (int c = 0; c < 4; ++c) rec({found ? bits(t[r][c]) : 0});
			break;
		}
		}
		draw->state();
	}
	delete draw;
	return copy_out(out, cap);
}

} /* extern "C" */
