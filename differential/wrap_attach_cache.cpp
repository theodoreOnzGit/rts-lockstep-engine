/* Differential-test wrapper for GitHub #179 items 4 and 5 (upstream commit 0a05454d), on cuts made verbatim by
** build.rs. From OUT_DIR/model_draw_particles_gen (written for wrap_model_draw_particles.cpp, #174 and #179 item 3):
**   mp_cache_attach_bone.inc  GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DModelDraw.h,
**                         `#define CACHE_ATTACH_BONE` (278);
**   mp_attach_offset.inc  GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp,
**                         `W3DModelDrawModuleData::getAttachToDrawableBoneOffset` (1148-1173);
**   mp_physics_info.inc   GameEngine/Include/GameClient/Drawable.h, `struct PhysicsXformInfo` (613-621);
**   mp_apply_physics.inc  GameEngine/Source/GameClient/Drawable.cpp, `applyPhysicsXform` (1388-1410);
**   mp_set_instance.inc   Drawable.cpp, `setInstanceMatrix` (4281-4293);
**   mp_draw_root.inc      Drawable.cpp, `Drawable::draw` from `// call the database defined draw action method` to its
**                         end (2660-2680);
** and from OUT_DIR/attach_cache_gen:
**   ac_get_transform.inc  Drawable.cpp, `Drawable::getTransformMatrix` (4297-4310, to `setCaptionText`'s comment).
** No edit. Compiled on stand-ins (namespace ac).
**
** Entry point 1, `ref_attach_cache_seq(in, nin, out, cap)` (#179 item 4, the cache's first filler). One module data
** (`W3DModelDrawModuleData`, the template's, shared by its drawables: `m_attachToDrawableBone` and the cache, the
** constructor's `m_attachToDrawableBoneOffsetValid(false)`, W3DModelDraw.cpp:1025-1027) and four drawables of the
** template. A drawable's `getPristineBonePositions(name, 0, NULL, &mtx, 1)` (Drawable.cpp:771-797) answers the attach
** bone of the state its flags select (the stand-in: one of four states, each found or not with its matrix; state 0 is
** the state for no flags). Words: in = [AttachToBoneInAnotherModule (0/1), 4 states x (found 0/1, 12 matrix words)],
** then steps of 3 words [op, drawable j (mod 4), state s (mod 4)]:
**   0 the drawable j is made (`newDrawable`, its flags cleared), bound with flags selecting state s
**     (`Object::friend_bindToDrawable`'s `clearAndSetModelConditionFlags`, Object.cpp:2975-3006), then its first
**     transform (`Object::reactToTransformChange` -> `Drawable::setTransformMatrix` -> each module's
**     `reactToTransformChange` -> `adjustTransformMtx`, W3DModelDraw.cpp:3658 -> 1999): it asks the cache. A drawable
**     already made is made again (a new object of the template).
**   1 the drawable j's flags select state s (no ask).
**   2 the drawable j asks the cache (`doDrawModule`'s `adjustTransformMtx`, 2098, or the logic's
**     `getProjectileLaunchOffset`, 3363); nothing for a drawable not made yet.
** Each ask is recorded as [700, offset non-NULL, x, y, z bits (0 for NULL)].
**
** Entry point 2, `ref_drawable_transform_seq(in, nin, out, cap)` (#179 item 5). A `Thing` (its `m_transform`,
** `getTransformMatrix` as Thing.h:142), the drawable a `Thing` with `getObject()` the case's object or NULL, the object
** its own transform (Thing.h:142 too) and `isDisabledByType(DISABLED_HELD)`. `TheGlobalData->m_showClientPhysics`, the
** view's and the script engine's freezes as wrap_model_draw_particles.cpp's (the case's show word; never frozen).
** `calcPhysicsXform` answers the case's totals. The draw modules: one recorder of the root `doDrawModule` is handed.
** Words: in = [object bound (0/1), the object's transform (12 words)], then steps of 14 words [op, a1, 12 words]:
**   0 the object bound if a1 (`friend_bindToObject`), else none.
**   1 with an object, its transform = the words, pushed to the drawable (`Object::reactToTransformChange`,
**     Object.cpp:1771-1774: `m_drawable->setTransformMatrix(this->getTransformMatrix())`); without one, nothing.
**   2 the drawable's own transform = the words (`Thing::setTransformMatrix`: a drawable's setPosition, the beams').
**   3 `setInstanceMatrix(a1 ? &words : NULL)`.
**   4 the next draws' `calcPhysicsXform` answers a1 with pitch, roll, yaw, z = words 0..3; held = word 4, show = word 5.
**   5 `Drawable::draw` from its root (the cut): recorded [400, the root's 12 words] by the module.
**   6 `getTransformMatrix()` (the cut): recorded [600, 12 words].
** The drawable starts as `setInstanceMatrix(NULL)`, its own transform the identity (Thing's constructor). */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/GameType.h"
#include "matrix3d.h"
#include "vector3.h"
#include "wwmath.h"

namespace ac {

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
Matrix3D matrix(const int *w)
{
	Matrix3D m(true);
	for (int r = 0; r < 3; ++r)
		for (int c = 0; c < 4; ++c) m[r][c] = real(w[r * 4 + c]);
	return m;
}
void recMatrix(const Matrix3D &m)
{
	for (int r = 0; r < 3; ++r)
		for (int c = 0; c < 4; ++c) rec({bits(m[r][c])});
}
int copy_out(int *out, int cap)
{
	int n = (int)g_out.size();
	if (n > cap) n = cap;
	if (n > 0) memcpy(out, &g_out[0], n * sizeof(int));
	return (int)g_out.size();
}

#include "mp_cache_attach_bone.inc"

/* GameEngine/Include/Common/DisabledTypes.h's DisabledType, the one value applyPhysicsXform names */
enum DisabledType { DISABLED_HELD = 6 };

/* Thing.h:142 */
class Thing
{
public:
	Matrix3D m_transform;
	Thing() : m_transform(true) {}
	const Matrix3D* getTransformMatrix() const { return &m_transform; }
};
class Object : public Thing
{
public:
	Bool m_held;
	Bool isDisabledByType( DisabledType ) const { return m_held; }
};
Object g_object;
Bool g_hasObject;

struct GlobalData { Bool m_showClientPhysics; };
GlobalData g_globalData = { TRUE };
GlobalData *TheGlobalData = &g_globalData;
struct View
{
	Bool isTimeFrozen() const { return FALSE; }
	Bool isCameraMovementFinished() const { return TRUE; }
};
View g_view;
View *TheTacticalView = &g_view;
struct ScriptEngine
{
	Bool isTimeFrozenDebug() const { return FALSE; }
	Bool isTimeFrozenScript() const { return FALSE; }
};
ScriptEngine g_scriptEngine;
ScriptEngine *TheScriptEngine = &g_scriptEngine;

/* entry point 1's states: the attach bone the drawable's pristine bones answer in each */
Bool g_stateFound[4];
Matrix3D g_stateMtx[4];

class DrawModule;
class Drawable : public Thing
{
public:
	/* entry point 1: the state the drawable's flags select */
	Int m_state;
	Int getPristineBonePositions(const char*, Int startIndex, Coord3D* positions, Matrix3D* transforms, Int maxBones) const
	{
		if (startIndex != 0 || positions != NULL || transforms == NULL || maxBones != 1) abort();
		if (!g_stateFound[m_state]) return 0;
		*transforms = g_stateMtx[m_state];
		return 1;
	}
	/* entry point 2 */
	Object *getObject() const { return g_hasObject ? &g_object : NULL; }
	const Matrix3D *getTransformMatrix( void ) const;
	Matrix3D m_instance;
	Bool m_instanceIsIdentity;
	void setInstanceMatrix( const Matrix3D *instance );
	const Matrix3D *getInstanceMatrix( void ) const { return &m_instance; }
	inline Bool isInstanceIdentity() const { return m_instanceIsIdentity; }
	void applyPhysicsXform(Matrix3D* mtx);
#include "mp_physics_info.inc"
	Bool calcPhysicsXform(PhysicsXformInfo& info);
	DrawModule *m_modules[2];
	DrawModule** getDrawModules() { return m_modules; }
	void drawRoot();
};

class W3DModelDrawModuleData
{
public:
	/* W3DModelDraw.h:290-324 (CACHE_ATTACH_BONE) */
	AsciiString m_attachToDrawableBone;
	mutable Vector3 m_attachToDrawableBoneOffset;
	mutable Bool m_attachToDrawableBoneOffsetValid;
	const Vector3* getAttachToDrawableBoneOffset(const Drawable* draw) const;
};
#include "mp_attach_offset.inc"

Bool g_physicsHas;
Drawable::PhysicsXformInfo g_physicsInfo;
Bool Drawable::calcPhysicsXform(PhysicsXformInfo& info)
{
	info = g_physicsInfo;
	return g_physicsHas;
}
#include "mp_apply_physics.inc"
#include "mp_set_instance.inc"
#include "ac_get_transform.inc"
class DrawModule
{
public:
	void doDrawModule( const Matrix3D* transformMtx )
	{
		rec({400});
		recMatrix(*transformMtx);
	}
};
DrawModule g_drawModule;
void Drawable::drawRoot()
{
#include "mp_draw_root.inc"

} /* namespace ac */

extern "C" {

int ref_attach_cache_seq(const int *in, int nin, int *out, int cap)
{
	using namespace ac;
	g_out.clear();
	int k = 0;
	W3DModelDrawModuleData data;
	data.m_attachToDrawableBone = in[k++] ? "attachbone" : "";
	data.m_attachToDrawableBoneOffsetValid = false;
	for (int s = 0; s < 4; ++s) {
		g_stateFound[s] = in[k++] != 0;
		g_stateMtx[s] = matrix(in + k);
		k += 12;
	}
	Drawable draws[4];
	Bool made[4] = { FALSE, FALSE, FALSE, FALSE };
	for (int st = k; st + 2 < nin; st += 3) {
		const int *a = in + st;
		int j = (unsigned)a[1] % 4;
		int s = (unsigned)a[2] % 4;
		if (a[0] == 1) {
			if (made[j]) draws[j].m_state = s;
			continue;
		}
		if (a[0] == 0) {
			made[j] = TRUE;
			draws[j].m_state = s;
		} else if (!made[j]) {
			continue;
		}
		const Vector3 *o = data.getAttachToDrawableBoneOffset(&draws[j]);
		rec({700, o ? 1 : 0, o ? bits(o->X) : 0, o ? bits(o->Y) : 0, o ? bits(o->Z) : 0});
	}
	return copy_out(out, cap);
}

int ref_drawable_transform_seq(const int *in, int nin, int *out, int cap)
{
	using namespace ac;
	g_out.clear();
	int k = 0;
	Drawable draw;
	draw.m_state = 0;
	draw.setInstanceMatrix(NULL);
	draw.m_modules[0] = &g_drawModule;
	draw.m_modules[1] = NULL;
	g_hasObject = in[k++] != 0;
	g_object.m_transform = matrix(in + k);
	k += 12;
	g_object.m_held = FALSE;
	g_globalData.m_showClientPhysics = TRUE;
	g_physicsHas = FALSE;
	g_physicsInfo = Drawable::PhysicsXformInfo();
	for (int st = k; st + 13 < nin; st += 14) {
		const int *a = in + st;
		switch (a[0]) {
		case 0: g_hasObject = a[1] != 0; break;
		case 1:
			if (g_hasObject) {
				g_object.m_transform = matrix(a + 2);
				draw.m_transform = *g_object.getTransformMatrix();
			}
			break;
		case 2: draw.m_transform = matrix(a + 2); break;
		case 3:
			if (a[1]) {
				Matrix3D m = matrix(a + 2);
				draw.setInstanceMatrix(&m);
			} else {
				draw.setInstanceMatrix(NULL);
			}
			break;
		case 4:
			g_physicsHas = a[1] != 0;
			g_physicsInfo.m_totalPitch = real(a[2]);
			g_physicsInfo.m_totalRoll = real(a[3]);
			g_physicsInfo.m_totalYaw = real(a[4]);
			g_physicsInfo.m_totalZ = real(a[5]);
			g_object.m_held = a[6] != 0;
			g_globalData.m_showClientPhysics = a[7] != 0;
			break;
		case 5: draw.drawRoot(); break;
		default:
			rec({600});
			recMatrix(*draw.getTransformMatrix());
			break;
		}
	}
	return copy_out(out, cap);
}

} /* extern "C" */
