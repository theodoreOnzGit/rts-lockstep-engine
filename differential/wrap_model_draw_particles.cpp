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
**     words]. */
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

class Object;
class Drawable
{
public:
	DrawableID m_id;
	Bool m_hidden;
	UnsignedInt m_status;
	Real m_scale;
	Coord3D m_pos;
	DrawableID getID() const { return m_id; }
	/* Drawable.h:329 (m_hiddenByStealth is false here) */
	Bool isDrawableEffectivelyHidden() const { return m_hidden; }
	/* Drawable.h:368 */
	Bool testDrawableStatus( DrawableStatus bit ) const { return (m_status & bit) != 0; }
	const Real getScale( void ) const { return m_scale; }
	const Coord3D *getPosition() const { return &m_pos; }
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

/* (#172) the hierarchy: PivotClass's members that Base_Update and HTreeClass::Scale read (pivot.h:79-99; never
** captured here), and HTreeClass's (htree.h) */
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
#include "mp_base_update.inc"
#include "mp_htree_scale.inc"
#include "mp_get_transform.inc"
#include "mp_asset_scale.inc"
FakeModel g_models[3];
const char *const BONE_NAMES[] = { "RootTransform", "Bone01", "Smoke01", "FX_A", "Muzzle", "SMOKE01" };
const char *const STATE_BONE_NAMES[] = { "RootTransform", "bone01", "SMOKE01", "fx_a", "Muzzle", "nosuchbone" };

class RenderObjClass
{
public:
	const FakeModel *m;
	HTreeClass HTree;
	Matrix3D Transform;
	Bool hidden;
	RenderObjClass() : m(NULL), Transform(true), hidden(FALSE) {}
	/* (#172) W3DAssetManager::Create_Render_Obj(name, scale, color) for model `fm` (see the header) */
	void Create( const FakeModel *fm, float scale )
	{
		m = fm;
		HTree = HTreeClass();
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

class Shadow
{
public:
	void enableShadowRender( Bool ) { abort(); }
	void enableShadowInvisible( Bool ) { abort(); }
};
class TerrainTracksRenderObjClass { public: void addCapEdgeToTrack( Real, Real ) { abort(); } };

struct ParticleSysBoneInfo
{
	AsciiString boneName;
	const ParticleSystemTemplate *particleSystemTemplate;
};
struct ModelConditionInfo
{
	Int model;
	std::vector<ParticleSysBoneInfo> m_particleSysBones;
};
ModelConditionInfo g_states[4];
class W3DModelDrawModuleData
{
public:
	Bool m_particlesAttachedToAnimatedBones;
};
W3DModelDrawModuleData g_data;

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

	/* setModelState: the early return for the current state, the cut, then the new state's render object */
	void setModelState( const ModelConditionInfo *newState )
	{
		if (m_curState != NULL && newState == m_curState)
			return;
#include "mp_set_model_state.inc"
		m_renderObject = newState->model < 3 ? &g_ro : NULL;
		if (m_renderObject) {
			/* (#172) Create_Render_Obj(newState->m_modelName.str(), draw->getScale(), m_hexColor) (3034) */
			g_ro.Create(&g_models[newState->model], getDrawable()->getScale());
			g_ro.Transform.Make_Identity();
		}
		m_curState = newState;
	}
	/* doDrawModule: the render object's transform, then the cut */
	void doDrawModule( const Matrix3D &mtx )
	{
		if (m_renderObject)
			m_renderObject->Set_Transform(mtx);
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
	}
};
#include "mp_start_stop.inc"
#include "mp_shroud.inc"
#include "mp_stop.inc"
#include "mp_bones.inc"
#include "mp_worldspace_bone.inc"

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
	g_data.m_particlesAttachedToAnimatedBones = in[k++] != 0;
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
	draw->state();
	for (int st = k; st + 13 < nin; st += 14) {
		const int *a = in + st;
		switch (a[0]) {
		case 0: draw->setModelState(&g_states[a[1] % nstates]); break;
		case 1:
		case 2: {
			/* (#172) the drawable's transform: the template scale is in the render object's hierarchy */
			Matrix3D mtx = matrix(a + 2);
			if (a[0] == 1) {
				draw->doDrawModule(mtx);
			} else {
				if (draw->m_renderObject) draw->m_renderObject->Set_Transform(mtx);
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
		default: {
			/* (#172) getCurrentWorldspaceClientBonePositions with the render object's transform world */
			if (draw->m_renderObject) draw->m_renderObject->Set_Transform(matrix(a + 2));
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
