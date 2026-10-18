/* Stand-ins for everything Locomotor.cpp reaches outside itself (Locomotor build only;
** differential tests; build.rs, differential/wrap_locomotor.cpp). Every file of this
** directory under GameLogic/ includes this one, so a `#include "GameLogic/Object.h"` in
** the unchanged upstream text finds these classes. The names are renamed by macro in
** PreRTS.h (so none is, to the linker, a class another fixture defines); here they are
** written with their upstream names.
**
** What each stands in for, and where its definition is (wrap_locomotor.cpp):
**  - Object: the Thing part (position, orientation, transform, direction vector, height
**    above terrain) is upstream's own text, cut verbatim from Thing.cpp by build.rs
**    (loco_gen/thing_methods.inc and thing_heights.inc, with `Thing::` read as `Object::`);
**    the rest (physics, body, status bits, layer, kind-of, geometry, disabled, model
**    conditions) answers from, and logs into, the scene the test installs.
**  - PhysicsBehavior, BodyModuleInterface: answer from the scene; every setter, force
**    and scrub is logged.
**  - TheTerrainLogic, ThePartitionManager, TheAI->pathfinder(): analytic tables of the
**    scene (see LocoEnv); every call is logged with its arguments.
**  - TheGlobalData, TheGameLogic: gravity, movement-penalty damage state, frame.
** The log is what the Rust test reproduces word for word: see wrap_locomotor.cpp. */
#ifndef SHIM_LOCOMOTOR_WORLD_H
#define SHIM_LOCOMOTOR_WORLD_H

#include "Lib/BaseType.h"
#include "Common/GameType.h"
#include "matrix3d.h"
#include "vector3.h"
#include "locomotor_gen_defs.h"

class Locomotor;

/* The scene of one call. The Rust test has the same struct (tests/differential_locomotor.rs,
** `Env`): same field order, 4-byte fields only. */
struct LocoEnv
{
	UnsignedInt frame;
	Real gravity;
	Int penalty;			/* TheGlobalData->m_movementPenaltyDamageState */
	Int bodyState;			/* the body module's damage state */
	Int hasPhysics, stunned, motive;
	Real vel[3];
	Real velMag, fwd2d, fwd3d, mass;
	Int layer;
	Int kindMask;			/* bit 0 KINDOF_PROJECTILE, bit 1 KINDOF_DOZER */
	Real deckHeight, majorRadius, boundingCircleRadius;
	Int disabledHeld;
	/* terrain: ground(x, y) = ga*x + gb*y + gc; water over every point with x < wx at height wz;
	** a layer above the ground is layerOffset higher; ground-or-structure is structOffset higher */
	Real ga, gb, gc, wx, wz, layerOffset, structOffset;
	Int highestLayer;
	/* the pathfinder: a point is invalid inside the box [x0, x1] x [y0, y1] when enabled */
	Int invEnabled;
	Real invBox[4];
};

extern LocoEnv g_locoEnv;
extern std::vector<unsigned> *g_locoLog;
extern int g_locoQuiet;

/* log record codes (the Rust test's `L_*` are the same numbers). LOG_TRANSFORM is the object's
** reactToTransformChange, which every Thing setter (setPosition, setPositionZ, setOrientation,
** setTransformMatrix) ends with: the new transform (12 words), cached position (3) and angle. */
enum
{
	LOG_TRANSFORM = 1, LOG_SET_STATUS, LOG_SET_MODEL_COND, LOG_CLEAR_MODEL_COND, LOG_APPLY_FORCE,
	LOG_SCRUB, LOG_SET_TURNING, LOG_SET_EXTRA_FRICTION, LOG_SET_AIRBORNE_FRICTION, LOG_SET_STICK,
	LOG_GROUND_HEIGHT = 20, LOG_LAYER_HEIGHT, LOG_UNDERWATER, LOG_HIGHEST_LAYER, LOG_STRUCT_HEIGHT,
	LOG_VALID_TERRAIN
};

class GlobalData
{
public:
	Real m_gravity;
	Int m_movementPenaltyDamageState;
};
extern GlobalData *TheGlobalData;

class GameLogic
{
public:
	UnsignedInt getFrame() const;
};
extern GameLogic *TheGameLogic;

class TerrainLogic
{
public:
	Real getGroundHeight(Real x, Real y, Coord3D *normal = NULL) const;
	Real getLayerHeight(Real x, Real y, PathfindLayerEnum layer, Coord3D *normal = NULL, Bool clip = TRUE) const;
	Bool isUnderwater(Real x, Real y, Real *waterZ = NULL, Real *terrainZ = NULL);
	PathfindLayerEnum getHighestLayerForDestination(const Coord3D *pos, Bool onlyHealthyBridges = FALSE);
	/* Thing::setPosition and setOrientation call it for KINDOF_STICK_TO_TERRAIN_SLOPE things */
	Int alignOnTerrain(Real angle, const Coord3D &pos, Bool stickToGround, Matrix3D &mtx);
};
extern TerrainLogic *TheTerrainLogic;

class PartitionManager
{
public:
	Real getGroundOrStructureHeight(Real posx, Real posy);
};
extern PartitionManager *ThePartitionManager;

class Pathfinder
{
public:
	Bool validMovementTerrain(PathfindLayerEnum layer, const Locomotor *locomotor, const Coord3D *pos);
};

class AI
{
public:
	Pathfinder *pathfinder() { return &m_pathfinder; }
private:
	Pathfinder m_pathfinder;
};
extern AI *TheAI;

class BodyModuleInterface
{
public:
	BodyDamageType getDamageState() const;
};

class PhysicsBehavior
{
public:
	Bool getIsStunned() const;
	void applyMotiveForce(const Coord3D *force);
	void scrubVelocity2D(Real desiredVelocity);
	void setTurning(PhysicsTurningType turning);
	const Coord3D *getVelocity() const;
	Real getVelocityMagnitude() const;
	Real getForwardSpeed2D() const;
	Real getForwardSpeed3D() const;
	Real getMass() const;
	Bool isMotive() const;
	void setExtraFriction(Real friction);
	void setAllowAirborneFriction(Bool allow);
	void setStickToGround(Bool stick);
};

class GeometryInfo
{
public:
	Real getMajorRadius() const;
	Real getBoundingCircleRadius() const;
};

/* ObjectStatusMaskType, as far as Locomotor.cpp uses it */
struct ObjectStatusMaskType
{
	UnsignedInt64 m_bits;
	Bool test(Int bit) const { return ((m_bits >> bit) & 1) != 0; }
};
inline ObjectStatusMaskType makeObjectStatusMask(Int bit) { ObjectStatusMaskType m; m.m_bits = 1ULL << bit; return m; }
#define MAKE_OBJECT_STATUS_MASK(k) makeObjectStatusMask(k)

/* what Object's m_template stands in for: Thing.cpp asks it isKindOf(KINDOF_STICK_TO_TERRAIN_SLOPE) */
struct LocoTemplateView
{
	const LocoTemplateView *operator->() const { return this; }
	Bool isKindOf(KindOfType t) const { return FALSE; }
};

class Object
{
public:
	/* ---- Thing (upstream text: loco_gen/thing_methods.inc, thing_heights.inc) */
	void setPosition(const Coord3D *pos);
	void setPositionZ(Real z);
	void setOrientation(Real angle);
	inline const Coord3D *getPosition() const { return &m_cachedPos; }
	inline Real getOrientation() const { return m_cachedAngle; }
	const Coord3D *getUnitDirectionVector2D() const;
	void getUnitDirectionVector2D(Coord3D &dir) const;
	void getUnitDirectionVector3D(Coord3D &dir) const;
	Real getHeightAboveTerrain() const;
	Real getHeightAboveTerrainOrWater() const;
	void setTransformMatrix(const Matrix3D *mx);
	const Matrix3D *getTransformMatrix() const { return &m_transform; }
	Real calculateHeightAboveTerrain(void) const;
	void reactToTransformChange(const Matrix3D *oldMtx, const Coord3D *oldPos, Real oldAngle);
	/* isAboveTerrain, quiet: the ground-height query of the Thing machinery is not logged */
	Bool isAboveTerrain() const;

	/* ---- the rest of Object, from the scene */
	PhysicsBehavior *getPhysics();
	BodyModuleInterface *getBodyModule();
	PathfindLayerEnum getLayer() const;
	Bool isKindOf(KindOfType t) const;
	ObjectStatusMaskType getStatusBits() const;
	void setStatus(ObjectStatusMaskType objectStatus, Bool set = TRUE);
	Real getCarrierDeckHeight() const;
	const GeometryInfo &getGeometryInfo() const;
	Bool isDisabledByType(DisabledType type) const;
	void setModelConditionState(ModelConditionFlagType a);
	void clearModelConditionState(ModelConditionFlagType a);

	Object();

	LocoTemplateView m_template;
	Matrix3D m_transform;
	enum
	{
		VALID_DIRVECTOR = 0x01,
		VALID_ALTITUDE_TERRAIN = 0x02,
		VALID_ALTITUDE_SEALEVEL = 0x04
	};
	mutable Coord3D m_cachedPos;
	mutable Real m_cachedAngle;
	mutable Coord3D m_cachedDirVector;
	mutable Real m_cachedAltitudeAboveTerrain;
	mutable Real m_cachedAltitudeAboveTerrainOrWater;
	mutable Int m_cacheFlags;
	UnsignedInt64 m_status;
	PhysicsBehavior m_physics;
	BodyModuleInterface m_body;
	GeometryInfo m_geometry;
};

#endif
