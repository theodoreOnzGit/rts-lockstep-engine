/* Differential-test wrapper for the tank, truck and tank truck draws' doDrawModule of GitHub #184 item 1 (their time
** freezes; upstream commit 0a05454d), cut verbatim by build.rs into OUT_DIR/tank_truck_draw_gen:
**   tt_wheel_info.inc   GameEngine/Include/GameClient/Drawable.h, `struct TWheelInfo` and `class DrawableLocoInfo` (to
**                       the TintEnvelope comment; the same cut as wrap_drawable_loco.cpp's dl_loco_info_h.inc);
**   tt_tank_treads.inc  GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DTankDraw.h, `enum TreadType` to
**                       `createEmitters` (TreadObjectInfo, m_treads, m_treadCount, m_lastDirection; inside the stand-in
**                       class);
**   tt_tt_treads.inc    W3DTankTruckDraw.h, `enum TreadType` to `m_prevRenderObj` (TreadObjectInfo, m_treads,
**                       m_treadCount);
**   tt_tank_pos.inc     GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTankDraw.cpp,
**                       `W3DTankDraw::updateTreadPositions` (222-240);
**   tt_tank_draw.inc    W3DTankDraw.cpp, `W3DTankDraw::doDrawModule` (308-406, to `crc`);
**   tt_truck_draw.inc   W3DTruckDraw.cpp, `W3DTruckDraw::doDrawModule` (389-640, to `crc`);
**   tt_tt_draw.inc      W3DTankTruckDraw.cpp, `W3DTankTruckDraw::doDrawModule` (505-742, to `crc`).
** No edit. Compiled on stand-ins (namespace tt) and WWMath's own Matrix3D, Vector2 and WWMath::Floor (matrix3d.h,
** vector2.h, wwmath.h; `sinf`, `cosf`, `floorf` and the double `sqrt`, `fabs` are the Rust's petir functions in the test
** binary):
**   - `TheTacticalView` (`isTimeFrozen`, `isCameraMovementFinished`), `TheScriptEngine` (`isTimeFrozenDebug`,
**     `isTimeFrozenScript`) and `TheGlobalData->m_showClientPhysics`: the frame's words;
**   - `W3DModelDraw::doDrawModule`: counts its calls (the port's base draw is seen by the render object's transform);
**   - the drawable (`getObject`: none, or the object; `getWheelInfo`: the frame's, or NULL;
**     `isDrawableEffectivelyHidden`: false), `getFullyObscuredByShroud` false;
**   - the object (`getPhysics`: none or the physics; `getAIUpdateInterface`/`getAI`: always the AI;
**     `getUnitDirectionVector2D(dir)`: the frame's (x, y, 0); `isSignificantlyAboveTerrain` false; `getID`,
**     `getPosition`), the physics (`getVelocity`, `getVelocityMagnitude`, `getTurning`, `isMotive`, `getAcceleration`
**     (0)), the AI (`getCurLocomotorSpeed`; `getPath`: a path with a cached point, or NULL; `getCurLocomotor` with
**     `isMovingBackwards`), `ThePartitionManager->getRelativeAngle2D`: the frame's angle;
**   - the render object: one, the same every frame (`getRenderObject()` its address); `Capture_Bone(i)` and
**     `Control_Bone(i, m)` recorded for bones 0..12 (`HTreeClass`'s: capture, then the control kept);
**   - `updateTreadObjects`, `updateBones` (not cut: they search the model) give the port's model's answer: with a
**     nonzero `TreadAnimationRate`, the tank's treads TREADSL (left), TREADSR (right), TREADSX (middle), the tank truck's
**     the same three, all middle (W3DTankTruckDraw.cpp:426-482 sets no type from the name); offsets 0; the bones of the
**     case's mask at the model's indices (cab 1, trailer 2, tires 3..12); each sets `m_prevRenderObj`. The members start
**     as the constructors leave them: no treads, `m_prevRenderObj` NULL, the bones and rotations 0, `m_lastDirection`
**     (1, 0, 0) (the trucks' uninitialised `m_cabBone`, `m_curCabRotation`, `m_trailerBone`, `m_curTrailerRotation`
**     and the tank truck's mid members 0, as the port's);
**   - the effects (`m_dustEffect`, `m_dirtEffect`, `m_powerslideEffect`, the tank's debris) NULL or counting stand-ins,
**     `enableEmitters`, the audio and the debris calls counted, not compared (the port does not make them).
**
** Entry point (extern "C", tests/differential_tank_truck_draw.rs; floats travel as their bits):
**   ref_tank_truck_draw_seq(in, nin, out, cap): in = [kind (0 W3DTankDraw, 1 W3DTruckDraw, 2 W3DTankTruckDraw), bone
**     mask (1 front, 2 rear, 4 mid-front, 8 mid-rear, 16 mid-mid, 32 cab, 64 trailer), 8 data Reals (tread animation
**     rate, pivot fraction, drive fraction, cab rotation, trailer rotation, rotation damping, powerslide rotation
**     addition, tire rotation multiplier), frame count], then per frame 22 words [present (0 no object, 1 no physics,
**     2 all), showClientPhysics, view isTimeFrozen, isCameraMovementFinished, isTimeFrozenDebug, isTimeFrozenScript,
**     velocity magnitude, turning, motive, current locomotor speed, direction x, y, moving backwards, path present,
**     angle to the path's point, wheel info present, its 4 height offsets and wheel angle, frames airborne].
**   Each frame calls the kind's doDrawModule and records 195 words: [base draws so far, front, rear, mid-front, mid-rear
**   wheel rotations (0 for the tank), cab and trailer rotations (0 but the truck), powersliding, tread count, 3 tread U
**   offsets (0 past the count), last direction x, y (the tank's, else 0), then bones 0..12 x (captured, the control's 12
**   words), then the module's 12 bone indices (front left, front right, rear left, rear right, the mid-front, mid-rear
**   and mid-mid pairs, cab, trailer; 0 for the tank)]. Returns the word count. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stddef.h>
#include "Lib/BaseType.h"
#include "Common/GameType.h"
#include "Common/GameMemory.h"
#include "Common/Debug.h"
#include "matrix3d.h"
#include "vector2.h"
#include "wwmath.h"

namespace tt {

static inline Real wr(int w) { Real r; memcpy(&r, &w, 4); return r; }
static inline int rw(Real r) { int w; memcpy(&w, &r, 4); return w; }

/* GameEngine/Include/GameLogic/Module/PhysicsUpdate.h:42-47 (the values the cuts compare against) */
enum PhysicsTurningType { TURN_NEGATIVE = -1, TURN_NONE = 0, TURN_POSITIVE = 1 };

#include "tt_wheel_info.inc"

enum { BONES = 13 };

/* the render object: its bones' captures and controls (HTreeClass::Capture_Bone, Control_Bone) */
class RenderObjClass
{
public:
	struct Material_Override
	{	Material_Override(void) : Struct_ID(0), customUVOffset(0,0) {}
		int Struct_ID;
		Vector2 customUVOffset;
	};
	Bool captured[BONES];
	Matrix3D control[BONES];
	RenderObjClass() { for (int i = 0; i < BONES; ++i) { captured[i] = FALSE; control[i].Make_Identity(); } }
	void Capture_Bone(int i) { if (i >= 0 && i < BONES) captured[i] = TRUE; }
	void Control_Bone(int i, const Matrix3D &m) { if (i >= 0 && i < BONES) control[i] = m; }
};

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

static int s_calls = 0; /* the effects, audio and debris calls: counted, not compared */
class ParticleSystem
{
public:
	void setVelocityMultiplier(const Coord3D *) { ++s_calls; }
	void setBurstCountMultiplier(Real) { ++s_calls; }
	void setSizeMultiplier(Real) { ++s_calls; }
	void trigger() { ++s_calls; }
	void start() { ++s_calls; }
	void stop() { ++s_calls; }
};
static ParticleSystem s_debris[2];
typedef int AudioHandle;
class AudioEventRTS
{
public:
	AudioHandle h;
	AudioEventRTS() : h(0) { }
	void setObjectID(ObjectID) { ++s_calls; }
	void setPosition(const Coord3D *) { ++s_calls; }
	void setPlayingHandle(AudioHandle x) { h = x; }
	AudioHandle getPlayingHandle() { return h; }
};
struct Audio
{
	AudioHandle addAudioEvent(AudioEventRTS *) { return ++s_calls; }
	void removeAudioEvent(AudioHandle) { ++s_calls; }
};
static Audio s_audio;
static Audio *TheAudio = &s_audio;

class PhysicsBehavior
{
public:
	Coord3D vel, accel;
	Real velMag;
	Int turning;
	Bool motive;
	const Coord3D *getVelocity() const { return &vel; }
	const Coord3D *getAcceleration() const { return &accel; }
	Real getVelocityMagnitude() const { return velMag; }
	PhysicsTurningType getTurning() const { return (PhysicsTurningType)turning; }
	Bool isMotive() const { return motive; }
};

class Locomotor
{
public:
	Bool backwards;
	Bool isMovingBackwards() const { return backwards; }
};

class Path
{
public:
	void peekCachedPointOnPath(Coord3D &p) const { p.x = 1.0f; p.y = 2.0f; p.z = 0.0f; }
};

class AIUpdateInterface
{
public:
	Real speed;
	Path path;
	Bool hasPath;
	Locomotor loco;
	Real getCurLocomotorSpeed() const { return speed; }
	Path *getPath() { return hasPath ? &path : NULL; }
	Locomotor *getCurLocomotor() { return &loco; }
};

class Object
{
public:
	PhysicsBehavior *physics;
	AIUpdateInterface ai;
	Coord3D dir, pos;
	PhysicsBehavior *getPhysics() const { return physics; }
	AIUpdateInterface *getAIUpdateInterface() { return &ai; }
	AIUpdateInterface *getAI() { return &ai; }
	void getUnitDirectionVector2D(Coord3D &d) const { d = dir; }
	Bool isSignificantlyAboveTerrain() const { return FALSE; }
	ObjectID getID() const { return (ObjectID)1; }
	const Coord3D *getPosition() const { return &pos; }
};

struct PartitionManager
{
	Real angle;
	Real getRelativeAngle2D(const Object *, const Coord3D *) const { return angle; }
};
static PartitionManager s_partition;
static PartitionManager *ThePartitionManager = &s_partition;

class Drawable
{
public:
	Object *obj;
	TWheelInfo wheel;
	Bool hasWheel;
	Object *getObject() { return obj; }
	const TWheelInfo *getWheelInfo() const { return hasWheel ? &wheel : NULL; }
	Bool isDrawableEffectivelyHidden() const { return FALSE; }
};

struct ModuleData
{
	Real m_treadAnimationRate, m_treadPivotSpeedFraction, m_treadDriveSpeedFraction;
	Real m_cabRotationFactor, m_trailerRotationFactor, m_rotationDampingFactor, m_powerslideRotationAddition;
	Real m_rotationSpeedMultiplier;
};
/* the modules' data classes, by the names the cuts use */
typedef ModuleData W3DTruckDrawModuleData;

/* the case: one render object, the module data, the bone mask */
static RenderObjClass *s_robj;
static Drawable s_drawable;
static ModuleData s_data;
static int s_mask;
static int s_baseDraws;

class W3DModelDraw
{
public:
	virtual ~W3DModelDraw() { }
	virtual void doDrawModule(const Matrix3D* transformMtx);
	RenderObjClass *getRenderObject() { return s_robj; }
	Drawable *getDrawable() { return &s_drawable; }
	Bool getFullyObscuredByShroud() const { return FALSE; }
};
void W3DModelDraw::doDrawModule(const Matrix3D*) { ++s_baseDraws; }

/* the bones of the case's mask at the port's model's indices (see the header) */
struct Bones
{
	Int m_frontLeftTireBone, m_frontRightTireBone, m_rearLeftTireBone, m_rearRightTireBone;
	Int m_midFrontLeftTireBone, m_midFrontRightTireBone, m_midRearLeftTireBone, m_midRearRightTireBone;
	Int m_midMidLeftTireBone, m_midMidRightTireBone;
	Int m_cabBone, m_trailerBone;
	void find(bool truck)
	{
		if (s_mask & 1) { m_frontLeftTireBone = 3; m_frontRightTireBone = 4; }
		if (s_mask & 2) { m_rearLeftTireBone = 5; m_rearRightTireBone = 6; }
		if (s_mask & 4) { m_midFrontLeftTireBone = 7; m_midFrontRightTireBone = 8; }
		if (s_mask & 8) { m_midRearLeftTireBone = 9; m_midRearRightTireBone = 10; }
		if (truck && (s_mask & 16)) { m_midMidLeftTireBone = 11; m_midMidRightTireBone = 12; }
		if (truck && (s_mask & 32)) { m_cabBone = 1; m_trailerBone = (s_mask & 64) ? 2 : 0; }
	}
};

class W3DTankDraw : public W3DModelDraw
{
public:
	W3DTankDraw() : m_treadDebrisLeft(&s_debris[0]), m_treadDebrisRight(&s_debris[1]), m_prevRenderObj(NULL)
	{
		m_treadCount = 0;
		m_lastDirection.x = 1.0f;
		m_lastDirection.y = 0.0f;
		m_lastDirection.z = 0.0f;
	}
	virtual void doDrawModule(const Matrix3D* transformMtx);
	const ModuleData *getW3DTankDrawModuleData() const { return &s_data; }
	ParticleSystem* m_treadDebrisLeft;
	ParticleSystem* m_treadDebrisRight;
	RenderObjClass *m_prevRenderObj;
#include "tt_tank_treads.inc"
	void startMoveDebris( void ) { ++s_calls; }
	void stopMoveDebris( void ) { ++s_calls; }
	void updateTreadObjects(void)
	{
		m_treadCount = 0;
		if (s_data.m_treadAnimationRate && getRenderObject())
		{
			TreadType types[3] = { TREAD_LEFT, TREAD_RIGHT, TREAD_MIDDLE };
			for (int i = 0; i < 3; ++i)
			{
				m_treads[i].m_robj = NULL;
				m_treads[i].m_type = types[i];
				m_treads[i].m_materialSettings.customUVOffset = Vector2(0,0);
				m_treadCount++;
			}
		}
		m_prevRenderObj = getRenderObject();
	}
	void updateTreadPositions(Real uvDelta);
};

class W3DTruckDraw : public W3DModelDraw, public Bones
{
public:
	W3DTruckDraw() : m_wasAirborne(FALSE), m_isPowersliding(FALSE), m_dustEffect(NULL), m_dirtEffect(NULL),
		m_powerslideEffect(NULL), m_frontWheelRotation(0), m_rearWheelRotation(0), m_midFrontWheelRotation(0),
		m_midRearWheelRotation(0), m_curCabRotation(0), m_curTrailerRotation(0), m_prevRenderObj(NULL)
	{
		memset(static_cast<Bones *>(this), 0, sizeof(Bones));
	}
	virtual void doDrawModule(const Matrix3D* transformMtx);
	const ModuleData *getW3DTruckDrawModuleData() const { return &s_data; }
	Bool m_wasAirborne, m_isPowersliding;
	ParticleSystem *m_dustEffect, *m_dirtEffect, *m_powerslideEffect;
	Real m_frontWheelRotation, m_rearWheelRotation, m_midFrontWheelRotation, m_midRearWheelRotation;
	Real m_curCabRotation, m_curTrailerRotation;
	AudioEventRTS m_powerslideSound, m_landingSound;
	RenderObjClass *m_prevRenderObj;
	void enableEmitters(Bool) { ++s_calls; }
	void updateBones(void) { find(true); m_prevRenderObj = getRenderObject(); }
};

class W3DTankTruckDraw : public W3DModelDraw, public Bones
{
public:
	W3DTankTruckDraw() : m_wasAirborne(FALSE), m_isPowersliding(FALSE), m_dustEffect(NULL), m_dirtEffect(NULL),
		m_powerslideEffect(NULL), m_frontWheelRotation(0), m_rearWheelRotation(0), m_midFrontWheelRotation(0),
		m_midRearWheelRotation(0), m_prevRenderObj(NULL)
	{
		memset(static_cast<Bones *>(this), 0, sizeof(Bones));
		m_treadDebrisLeft = NULL;
		m_treadDebrisRight = NULL;
		m_treadCount = 0;
	}
	virtual void doDrawModule(const Matrix3D* transformMtx);
	const ModuleData *getW3DTankTruckDrawModuleData() const { return &s_data; }
	Bool m_wasAirborne, m_isPowersliding;
	ParticleSystem *m_dustEffect, *m_dirtEffect, *m_powerslideEffect;
	Real m_frontWheelRotation, m_rearWheelRotation, m_midFrontWheelRotation, m_midRearWheelRotation;
	AudioEventRTS m_powerslideSound, m_landingSound;
	ParticleSystem* m_treadDebrisLeft;
	ParticleSystem* m_treadDebrisRight;
#include "tt_tt_treads.inc"
	RenderObjClass *m_prevRenderObj;
	void enableEmitters(Bool) { ++s_calls; }
	void updateBones(void) { find(false); m_prevRenderObj = getRenderObject(); }
	void updateTreadObjects(void)
	{
		m_treadCount = 0;
		if (s_data.m_treadAnimationRate && getRenderObject())
		{
			for (int i = 0; i < 3; ++i)
			{
				m_treads[i].m_robj = NULL;
				m_treads[i].m_type = TREAD_MIDDLE;
				m_treads[i].m_materialSettings.customUVOffset = Vector2(0,0);
				m_treadCount++;
			}
		}
		m_prevRenderObj = getRenderObject();
	}
};

#include "tt_tank_pos.inc"
#include "tt_tank_draw.inc"
#include "tt_truck_draw.inc"
#include "tt_tt_draw.inc"

} /* namespace tt */

extern "C" int ref_tank_truck_draw_seq(const int *in, int nin, int *out, int cap)
{
	using namespace tt;
	const int head = 11, stride = 22, words = 195;
	if (nin < head)
		return 0;
	int kind = in[0];
	s_mask = in[1];
	s_data.m_treadAnimationRate = wr(in[2]);
	s_data.m_treadPivotSpeedFraction = wr(in[3]);
	s_data.m_treadDriveSpeedFraction = wr(in[4]);
	s_data.m_cabRotationFactor = wr(in[5]);
	s_data.m_trailerRotationFactor = wr(in[6]);
	s_data.m_rotationDampingFactor = wr(in[7]);
	s_data.m_powerslideRotationAddition = wr(in[8]);
	s_data.m_rotationSpeedMultiplier = wr(in[9]);
	int frames = in[10];
	RenderObjClass robj;
	s_robj = &robj;
	s_baseDraws = 0;
	W3DTankDraw tank;
	W3DTruckDraw truck;
	W3DTankTruckDraw tankTruck;
	PhysicsBehavior physics;
	Object obj;
	memset(&physics, 0, sizeof(physics));
	const int *p = in + head;
	int n = 0;
	for (int f = 0; f < frames && p + stride <= in + nin; ++f, p += stride)
	{
		s_globalData.m_showClientPhysics = p[1] != 0;
		s_view.frozen = p[2] != 0;
		s_view.finished = p[3] != 0;
		s_scriptEngine.debug = p[4] != 0;
		s_scriptEngine.script = p[5] != 0;
		physics.velMag = wr(p[6]);
		physics.vel.x = wr(p[6]);
		physics.vel.y = 0.0f;
		physics.vel.z = 0.0f;
		physics.turning = p[7];
		physics.motive = p[8] != 0;
		obj.physics = p[0] >= 2 ? &physics : NULL;
		obj.ai.speed = wr(p[9]);
		obj.dir.x = wr(p[10]);
		obj.dir.y = wr(p[11]);
		obj.dir.z = 0.0f;
		obj.pos.x = obj.pos.y = obj.pos.z = 0.0f;
		obj.ai.loco.backwards = p[12] != 0;
		obj.ai.hasPath = p[13] != 0;
		s_partition.angle = wr(p[14]);
		s_drawable.obj = p[0] >= 1 ? &obj : NULL;
		s_drawable.hasWheel = p[15] != 0;
		s_drawable.wheel.m_frontLeftHeightOffset = wr(p[16]);
		s_drawable.wheel.m_frontRightHeightOffset = wr(p[17]);
		s_drawable.wheel.m_rearLeftHeightOffset = wr(p[18]);
		s_drawable.wheel.m_rearRightHeightOffset = wr(p[19]);
		s_drawable.wheel.m_wheelAngle = wr(p[20]);
		s_drawable.wheel.m_framesAirborne = p[21];
		s_drawable.wheel.m_framesAirborneCounter = 0;
		Matrix3D mtx(1);
		Real rot[4] = { 0, 0, 0, 0 };
		Real cab[2] = { 0, 0 };
		Bool slide = FALSE;
		int treads = 0;
		Real u[3] = { 0, 0, 0 };
		Real last[2] = { 0, 0 };
		if (kind == 0)
		{
			tank.doDrawModule(&mtx);
			treads = tank.m_treadCount;
			for (int i = 0; i < treads && i < 3; ++i)
				u[i] = tank.m_treads[i].m_materialSettings.customUVOffset.X;
			last[0] = tank.m_lastDirection.x;
			last[1] = tank.m_lastDirection.y;
		}
		else if (kind == 1)
		{
			truck.doDrawModule(&mtx);
			Real r[4] = { truck.m_frontWheelRotation, truck.m_rearWheelRotation, truck.m_midFrontWheelRotation, truck.m_midRearWheelRotation };
			memcpy(rot, r, sizeof(rot));
			cab[0] = truck.m_curCabRotation;
			cab[1] = truck.m_curTrailerRotation;
			slide = truck.m_isPowersliding;
		}
		else
		{
			tankTruck.doDrawModule(&mtx);
			Real r[4] = { tankTruck.m_frontWheelRotation, tankTruck.m_rearWheelRotation, tankTruck.m_midFrontWheelRotation, tankTruck.m_midRearWheelRotation };
			memcpy(rot, r, sizeof(rot));
			slide = tankTruck.m_isPowersliding;
			treads = tankTruck.m_treadCount;
			for (int i = 0; i < treads && i < 3; ++i)
				u[i] = tankTruck.m_treads[i].m_materialSettings.customUVOffset.X;
		}
		if (n + words > cap)
			break;
		out[n++] = s_baseDraws;
		for (int i = 0; i < 4; ++i)
			out[n++] = rw(rot[i]);
		out[n++] = rw(cab[0]);
		out[n++] = rw(cab[1]);
		out[n++] = slide ? 1 : 0;
		out[n++] = treads;
		for (int i = 0; i < 3; ++i)
			out[n++] = rw(u[i]);
		out[n++] = rw(last[0]);
		out[n++] = rw(last[1]);
		for (int b = 0; b < BONES; ++b)
		{
			out[n++] = robj.captured[b] ? 1 : 0;
			for (int r = 0; r < 3; ++r)
				for (int c = 0; c < 4; ++c)
					out[n++] = rw(robj.control[b][r][c]);
		}
		Bones none;
		memset(&none, 0, sizeof(none));
		const Bones &bn = kind == 1 ? (const Bones &)truck : kind == 2 ? (const Bones &)tankTruck : none;
		int bw[12] = { bn.m_frontLeftTireBone, bn.m_frontRightTireBone, bn.m_rearLeftTireBone, bn.m_rearRightTireBone,
			bn.m_midFrontLeftTireBone, bn.m_midFrontRightTireBone, bn.m_midRearLeftTireBone, bn.m_midRearRightTireBone,
			bn.m_midMidLeftTireBone, bn.m_midMidRightTireBone, bn.m_cabBone, bn.m_trailerBone };
		for (int i = 0; i < 12; ++i)
			out[n++] = bw[i];
	}
	return n;
}
