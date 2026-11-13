/* Differential-test wrapper for Drawable's client physics models of GitHub #182 item 2 (upstream commit 0a05454d), cut
** verbatim by build.rs into OUT_DIR/drawable_loco_gen:
**   dl_appearance.inc   GameEngine/Include/GameLogic/Locomotor.h, `enum LocomotorAppearance` (to `enum
**                       LocomotorPriority`);
**   dl_turning.inc      GameEngine/Include/GameLogic/Module/PhysicsUpdate.h, `enum PhysicsTurningType` (42-47);
**   dl_loco_info_h.inc  GameEngine/Include/GameClient/Drawable.h, `struct TWheelInfo` and `class DrawableLocoInfo` (to the
**                       TintEnvelope comment, 126-160);
**   dl_physics_info.inc Drawable.h, `struct PhysicsXformInfo` (613-621, inside the Drawable stand-in);
**   dl_ctor.inc         GameEngine/Source/GameClient/Drawable.cpp, `DrawableLocoInfo::DrawableLocoInfo` and its
**                       destructor (185-216, to drawableIconIndexToName);
**   dl_calc.inc         Drawable.cpp, `calcPhysicsXform`, `calcPhysicsXformThrust`, `calcPhysicsXformHoverOrWings`,
**                       `calcPhysicsXformTreads`, `calcPhysicsXformWheels` and `calcPhysicsXformMotorcycle` (1414-2506, to
**                       getAmbientSoundByDamage);
**   dl_apply.inc        (#182 item 3) Drawable.cpp, `applyPhysicsXform` (1388-1410, to calcPhysicsXform).
** No edit. Compiled on stand-ins (namespace dl): the drawable (`m_locoInfo`, `getObject`, `getPosition`,
** `getUnitDirectionVector2D`, `getDrawableGeometryInfo`: the object's), the object (`getAIUpdateInterface`,
** `getPhysics`, `getLayer`, `isSignificantlyAboveTerrain`, `getGeometryInfo`, `getPosition`, `isKindOf`,
** `getBodyModule`'s crushed flags), the AI (`getCurLocomotor`, `getCurLocomotorSpeed`), the locomotor (its template's
** getters as Locomotor.h has them: each returns the case's word), the physics (`getAcceleration`, `getVelocity`,
** `getVelocityMagnitude`, `isMotive`, `getTurning`, `getForwardSpeed2D`, `getCurrentOverlap`, `getPreviousOverlap`),
** `TheTerrainLogic->getLayerHeight` (the case's height and normal) and `TheGameLogic->findObjectByID` (the overlapped
** object, id 2, when the case has one). `KINDOF_SHRUBBERY`, `KINDOF_LOW_OVERLAPPABLE`, `KINDOF_INFANTRY` are the
** stand-in's names for the kinds the case gives. Linked against the REAL `GetGameClientRandomValue(Real)` and
** `InitRandom` (RandomValue.cpp) and `Sin` (Trig.cpp) of upstream_gameengine; `sinf`, `sin`, `cos`, `atan2`, `sqrt`,
** `sqrtf` are the Rust's petir functions in the test binary, and the shim's math.h gives `fabs`, `sin`, `cos`, `sqrt`
** and `atan2` of a float MSVC's C semantics (the double functions). `sqr` is BaseType.h:143-147's template, written
** here (the shim's BaseType.h does not carry it), and `PI` BaseType.h:86's.
** (#182 item 3) applyPhysicsXform's stand-ins: the object's `isDisabledByType` (the case's held word; `DISABLED_HELD` is
** DisabledTypes.h's name, its value immaterial here), `TheGlobalData->m_showClientPhysics`, `TheTacticalView`'s
** `isTimeFrozen` and `isCameraMovementFinished`, `TheScriptEngine`'s `isTimeFrozenDebug` and `isTimeFrozenScript` (each
** the case's word), and a `Matrix3D` that records the calls made on it (`Translate` must be (0, 0, z): its x and y are
** asserted 0; then z, and each `Rotate_*`'s angle, in call order).
**
** Entry point (extern "C", tests/differential_drawable_loco.rs; floats travel as their bits):
**   ref_drawable_loco_seq(in, nin, out, cap): in = [seed, frame count], then per frame 63 words:
**     [kind (0 no object, 1 no AI, 2 no current locomotor, 3 no physics, 4 all),
**      the locomotor's 25: appearance, accelPitchLimit, decelPitchLimit, bounceKick, pitchStiffness, rollStiffness,
**        pitchDamping, rollDamping, pitchByZVelCoef, thrustRoll, wobbleRate, maxWobble, minWobble, forwardVelCoef,
**        lateralVelCoef, forwardAccelCoef, lateralAccelCoef, uniformAxialDamping, hasSuspension, maxWheelExtension,
**        wheelTurnAngle, rudderCorrectionDegree, rudderCorrectionRate, elevatorCorrectionDegree, elevatorCorrectionRate;
**      the object's 25: position xyz, direction xyz, acceleration xyz, velocity xyz, velocity magnitude, motive,
**        turning (-1, 0, 1), forward speed 2D, layer height, normal xyz, significantly above terrain, major radius,
**        minor radius, bounding circle radius, current locomotor speed;
**      the overlap's 11: present, position xyz, bounding circle radius, max height above position, shrubbery, low
**        overlappable, infantry, front crushed, back crushed;
**      previous overlap (0 INVALID_ID, else 3)].
**   InitRandom(seed) first. Each frame calls `calcPhysicsXform(info)` on a fresh PhysicsXformInfo and records 27 words:
**   [answer, totalPitch, totalRoll, totalYaw, totalZ, m_locoInfo present, its 14 Reals (m_pitch .. m_pitchModulator in
**   declaration order), m_wheelInfo's 5 Reals and 2 Ints (declaration order)]. Returns the word count.
**   (#182 item 3) ref_drawable_loco_apply_seq(in, nin, out, cap): the same, but each frame has 69 words, the 63 then
**   [held, showClientPhysics, view isTimeFrozen, isCameraMovementFinished, isTimeFrozenDebug, isTimeFrozenScript], and
**   calls `applyPhysicsXform(&mtx)` on a fresh recording Matrix3D (which calls the cut calcPhysicsXform past its gates).
**   It records 31 words: [the number of calls on the matrix, then for the first 4 the call's kind (0 Translate, 1
**   Rotate_Y, 2 Rotate_X, 3 Rotate_Z) and its value (zeros past the calls made)], then the 22 m_locoInfo words above. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stddef.h>
#include "Lib/BaseType.h"
#include "Lib/trig.h"
#include "Common/GameType.h"
#include "Common/GameMemory.h"
#include "Common/RandomValue.h"
#include "GameClient/ClientRandomValue.h"

/* Libraries/Include/Lib/BaseType.h:86 (the shim's BaseType.h does not carry it) */
#ifndef PI
#define PI 3.14159265359f
#endif

/* Libraries/Include/Lib/BaseType.h:143-147 (the shim's BaseType.h does not carry it) */
template <typename NUM>
inline NUM sqr(NUM x)
{
	return x*x;
}

namespace dl {

#include "dl_appearance.inc"
#include "dl_turning.inc"
#include "dl_loco_info_h.inc"
#include "dl_ctor.inc"

static inline Real wr(int w) { Real r; memcpy(&r, &w, 4); return r; }
static inline int rw(Real r) { int w; memcpy(&w, &r, 4); return w; }

enum KindOfType { KINDOF_SHRUBBERY, KINDOF_LOW_OVERLAPPABLE, KINDOF_INFANTRY };
/* (#182 item 3) GameEngine/Include/Common/DisabledTypes.h's name applyPhysicsXform tests */
enum DisabledType { DISABLED_HELD = 6 };
typedef int PathfindLayerEnum;

class Locomotor
{
public:
	Int appearance;
	Real f[24];
	LocomotorAppearance getAppearance() const { return (LocomotorAppearance)appearance; }
	Real getAccelPitchLimit() const { return f[0]; }
	Real getDecelPitchLimit() const { return f[1]; }
	Real getBounceKick() const { return f[2]; }
	Real getPitchStiffness() const { return f[3]; }
	Real getRollStiffness() const { return f[4]; }
	Real getPitchDamping() const { return f[5]; }
	Real getRollDamping() const { return f[6]; }
	Real getPitchByZVelCoef() const { return f[7]; }
	Real getThrustRoll() const { return f[8]; }
	Real getWobbleRate() const { return f[9]; }
	Real getMaxWobble() const { return f[10]; }
	Real getMinWobble() const { return f[11]; }
	Real getForwardVelCoef() const { return f[12]; }
	Real getLateralVelCoef() const { return f[13]; }
	Real getForwardAccelCoef() const { return f[14]; }
	Real getLateralAccelCoef() const { return f[15]; }
	Real getUniformAxialDamping() const { return f[16]; }
	Bool hasSuspension() const { return rw(f[17]) != 0; }
	Real getMaxWheelExtension() const { return f[18]; }
	Real getWheelTurnAngle() const { return f[19]; }
	Real getRudderCorrectionDegree() const { return f[20]; }
	Real getRudderCorrectionRate() const { return f[21]; }
	Real getElevatorCorrectionDegree() const { return f[22]; }
	Real getElevatorCorrectionRate() const { return f[23]; }
};

class PhysicsBehavior
{
public:
	Coord3D accel, vel;
	Real velMag, fwd2d;
	Bool motive;
	Int turning;
	ObjectID cur, prev;
	const Coord3D *getAcceleration() const { return &accel; }
	const Coord3D *getVelocity() const { return &vel; }
	Real getVelocityMagnitude() const { return velMag; }
	Bool isMotive() const { return motive; }
	PhysicsTurningType getTurning() const { return (PhysicsTurningType)turning; }
	Real getForwardSpeed2D() const { return fwd2d; }
	ObjectID getCurrentOverlap() const { return cur; }
	ObjectID getPreviousOverlap() const { return prev; }
};

class AIUpdateInterface
{
public:
	const Locomotor *loco;
	Real speed;
	const Locomotor *getCurLocomotor() const { return loco; }
	Real getCurLocomotorSpeed() const { return speed; }
};

class GeometryInfo
{
public:
	Real major, minor, bound, maxHeight;
	Real getMajorRadius() const { return major; }
	Real getMinorRadius() const { return minor; }
	Real getBoundingCircleRadius() const { return bound; }
	Real getMaxHeightAbovePosition() const { return maxHeight; }
};

class BodyModuleInterface
{
public:
	Bool front, back;
	Bool getFrontCrushed() const { return front; }
	Bool getBackCrushed() const { return back; }
};

class Object
{
public:
	AIUpdateInterface *ai;
	PhysicsBehavior *physics;
	Bool above;
	GeometryInfo geom;
	Coord3D pos;
	Bool kinds[3];
	BodyModuleInterface body;
	Bool held;
	Bool isDisabledByType(DisabledType) const { return held; }
	AIUpdateInterface *getAIUpdateInterface() const { return ai; }
	PhysicsBehavior *getPhysics() const { return physics; }
	PathfindLayerEnum getLayer() const { return 0; }
	Bool isSignificantlyAboveTerrain() const { return above; }
	const GeometryInfo &getGeometryInfo() const { return geom; }
	const Coord3D *getPosition() const { return &pos; }
	Bool isKindOf(KindOfType k) const { return kinds[k]; }
	BodyModuleInterface *getBodyModule() const { return (BodyModuleInterface *)&body; }
};

/* (#182 item 3) applyPhysicsXform's reads besides the drawable (see the header) */
struct GlobalData { Bool m_showClientPhysics; };
static GlobalData s_globalData = { TRUE };
static GlobalData *TheGlobalData = &s_globalData;
struct View
{
	Bool frozen, finished;
	Bool isTimeFrozen() const { return frozen; }
	Bool isCameraMovementFinished() const { return finished; }
};
static View s_view = { FALSE, TRUE };
static View *TheTacticalView = &s_view;
struct ScriptEngine
{
	Bool debug, script;
	Bool isTimeFrozenDebug() const { return debug; }
	Bool isTimeFrozenScript() const { return script; }
};
static ScriptEngine s_scriptEngine = { FALSE, FALSE };
static ScriptEngine *TheScriptEngine = &s_scriptEngine;
/* a Matrix3D that records the calls applyPhysicsXform makes on it */
class Matrix3D
{
public:
	int n;
	int kind[4];
	Real value[4];
	Matrix3D() : n(0) { }
	void put(int k, Real v) { if (n < 4) { kind[n] = k; value[n] = v; } ++n; }
	void Translate(Real x, Real y, Real z) { if (x != 0.0f || y != 0.0f) abort(); put(0, z); }
	void Rotate_Y(Real theta) { put(1, theta); }
	void Rotate_X(Real theta) { put(2, theta); }
	void Rotate_Z(Real theta) { put(3, theta); }
};

class TerrainLogic
{
public:
	Real height;
	Coord3D normal;
	Real getLayerHeight(Real x, Real y, PathfindLayerEnum layer, Coord3D *n) { if (n) *n = normal; return height; }
};

class GameLogic
{
public:
	Object *overlap;
	Object *findObjectByID(ObjectID id) { return (id == (ObjectID)2) ? overlap : NULL; }
};

static TerrainLogic s_terrain;
static GameLogic s_logic;
static TerrainLogic *TheTerrainLogic = &s_terrain;
static GameLogic *TheGameLogic = &s_logic;

class Drawable
{
public:
	DrawableLocoInfo *m_locoInfo;
	Object *m_object;
	Coord3D m_pos, m_dir;
	Drawable() : m_locoInfo(NULL), m_object(NULL) { }
	~Drawable() { if (m_locoInfo) m_locoInfo->deleteInstance(); }
	Object *getObject() { return m_object; }
	const Coord3D *getPosition() const { return &m_pos; }
	const Coord3D *getUnitDirectionVector2D() const { return &m_dir; }
	const GeometryInfo &getDrawableGeometryInfo() const { return m_object->getGeometryInfo(); }

#include "dl_physics_info.inc"
	Bool calcPhysicsXform(PhysicsXformInfo& info);
	void calcPhysicsXformThrust(const Locomotor *locomotor, PhysicsXformInfo& info);
	void calcPhysicsXformHoverOrWings(const Locomotor *locomotor, PhysicsXformInfo& info);
	void calcPhysicsXformTreads(const Locomotor *locomotor, PhysicsXformInfo& info);
	void calcPhysicsXformWheels(const Locomotor *locomotor, PhysicsXformInfo& info);
	void calcPhysicsXformMotorcycle( const Locomotor *locomotor, PhysicsXformInfo& info );
	void applyPhysicsXform(Matrix3D* mtx);
};

#include "dl_calc.inc"
#include "dl_apply.inc"

static Coord3D c3(const int *w) { Coord3D c; c.x = wr(w[0]); c.y = wr(w[1]); c.z = wr(w[2]); return c; }

} /* namespace dl */

/* both entry points: `apply` false calls calcPhysicsXform on 63-word frames, true applyPhysicsXform on 69-word ones */
static int loco_seq(const int *in, int nin, int *out, int cap, bool apply)
{
	using namespace dl;
	if (nin < 2)
		return 0;
	InitRandom((UnsignedInt)in[0]);
	int frames = in[1];
	const int stride = apply ? 69 : 63;
	const int *p = in + 2;
	int n = 0;
	Drawable d;
	for (int f = 0; f < frames && (p + stride) <= in + nin; ++f, p += stride)
	{
		int kind = p[0];
		Locomotor loco;
		loco.appearance = p[1];
		for (int i = 0; i < 24; ++i)
			loco.f[i] = wr(p[2 + i]);
		const int *o = p + 26;
		PhysicsBehavior physics;
		physics.accel = c3(o + 6);
		physics.vel = c3(o + 9);
		physics.velMag = wr(o[12]);
		physics.motive = o[13] != 0;
		physics.turning = o[14];
		physics.fwd2d = wr(o[15]);
		AIUpdateInterface ai;
		ai.loco = kind >= 3 ? &loco : NULL;
		ai.speed = wr(o[24]);
		Object obj;
		memset(&obj, 0, sizeof(obj));
		obj.ai = kind >= 2 ? &ai : NULL;
		obj.physics = kind >= 4 ? &physics : NULL;
		obj.above = o[20] != 0;
		obj.geom.major = wr(o[21]);
		obj.geom.minor = wr(o[22]);
		obj.geom.bound = wr(o[23]);
		obj.pos = c3(o);
		d.m_object = kind >= 1 ? &obj : NULL;
		d.m_pos = c3(o);
		d.m_dir = c3(o + 3);
		s_terrain.height = wr(o[16]);
		s_terrain.normal = c3(o + 17);
		const int *v = o + 25;
		Object over;
		memset(&over, 0, sizeof(over));
		over.pos = c3(v + 1);
		over.geom.bound = wr(v[4]);
		over.geom.maxHeight = wr(v[5]);
		over.kinds[KINDOF_SHRUBBERY] = v[6] != 0;
		over.kinds[KINDOF_LOW_OVERLAPPABLE] = v[7] != 0;
		over.kinds[KINDOF_INFANTRY] = v[8] != 0;
		over.body.front = v[9] != 0;
		over.body.back = v[10] != 0;
		s_logic.overlap = &over;
		physics.cur = v[0] ? (ObjectID)2 : INVALID_ID;
		physics.prev = v[11] ? (ObjectID)3 : INVALID_ID;

		if (apply)
		{
			const int *gw = p + 63;
			obj.held = gw[0] != 0;
			s_globalData.m_showClientPhysics = gw[1] != 0;
			s_view.frozen = gw[2] != 0;
			s_view.finished = gw[3] != 0;
			s_scriptEngine.debug = gw[4] != 0;
			s_scriptEngine.script = gw[5] != 0;
			Matrix3D mtx;
			d.applyPhysicsXform(&mtx);
			if (n + 31 > cap)
				break;
			out[n++] = mtx.n;
			for (int i = 0; i < 4; ++i)
			{
				out[n++] = i < mtx.n ? mtx.kind[i] : 0;
				out[n++] = i < mtx.n ? rw(mtx.value[i]) : 0;
			}
		}
		else
		{
			Drawable::PhysicsXformInfo info;
			Bool answer = d.calcPhysicsXform(info);
			if (n + 27 > cap)
				break;
			out[n++] = answer ? 1 : 0;
			out[n++] = rw(info.m_totalPitch);
			out[n++] = rw(info.m_totalRoll);
			out[n++] = rw(info.m_totalYaw);
			out[n++] = rw(info.m_totalZ);
		}
		const DrawableLocoInfo *li = d.m_locoInfo;
		out[n++] = li ? 1 : 0;
		Real r[14] = { 0 };
		Real wf[5] = { 0 };
		Int wi[2] = { 0, 0 };
		if (li)
		{
			Real t[14] = { li->m_pitch, li->m_pitchRate, li->m_roll, li->m_rollRate, li->m_yaw, li->m_accelerationPitch,
				li->m_accelerationPitchRate, li->m_accelerationRoll, li->m_accelerationRollRate, li->m_overlapZVel,
				li->m_overlapZ, li->m_wobble, li->m_yawModulator, li->m_pitchModulator };
			memcpy(r, t, sizeof(r));
			const TWheelInfo &w = li->m_wheelInfo;
			Real u[5] = { w.m_frontLeftHeightOffset, w.m_frontRightHeightOffset, w.m_rearLeftHeightOffset,
				w.m_rearRightHeightOffset, w.m_wheelAngle };
			memcpy(wf, u, sizeof(wf));
			wi[0] = w.m_framesAirborneCounter;
			wi[1] = w.m_framesAirborne;
		}
		for (int i = 0; i < 14; ++i)
			out[n++] = rw(r[i]);
		for (int i = 0; i < 5; ++i)
			out[n++] = rw(wf[i]);
		out[n++] = wi[0];
		out[n++] = wi[1];
	}
	return n;
}

extern "C" int ref_drawable_loco_seq(const int *in, int nin, int *out, int cap)
{
	return loco_seq(in, nin, out, cap, false);
}

/* (#182 item 3) */
extern "C" int ref_drawable_loco_apply_seq(const int *in, int nin, int *out, int cap)
{
	return loco_seq(in, nin, out, cap, true);
}
