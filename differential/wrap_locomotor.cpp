/* Differential-test wrapper for Locomotor and LocomotorSet (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Locomotor.cpp
** (#included below, UNCHANGED, with the real Locomotor.h, LocomotorSet.h, INI.h,
** NameKeyGenerator.h, Override.h, Overridable.h, Snapshot.h, Xfer*.h and BaseType.h; the
** other headers it names are the stand-ins of differential/shim_locomotor, see
** locomotor_world.h and PreRTS.h there).
**
** What is real here: Locomotor.cpp in full (LocomotorTemplate and its INI block,
** LocomotorStore, Locomotor and every movement function, LocomotorSet and their xfer), the
** INI reader and field parsers it uses (upstream_ini), the name keys, the Xfer classes, the
** WWMath Matrix3D and Vector3, BaseType.h, Trig.cpp's Cos/Sin/ACos and RandomValue.cpp's
** GameLogicRandomValue, and, extracted verbatim by build.rs (OUT_DIR/loco_gen):
** - the enums ModelConditionFlagType, KindOfType, ObjectStatusTypes, DisabledType
**   (ModelState.h, KindOf.h, ObjectStatusTypes.h, DisabledTypes.h), BodyDamageType and its
**   IS_CONDITION_* macros (BodyModule.h), PhysicsTurningType (PhysicsUpdate.h) and
**   PATHFIND_CELL_SIZE{,_F} (AIPathfind.h) in locomotor_gen_defs.h;
** - the position, orientation, transform, direction-vector and height-above-terrain
**   methods of Thing (Thing.cpp, `Thing::` read as `Object::`) in thing_methods.inc and
**   thing_heights.inc, so that the object a Locomotor drives moves as the engine's does.
**
** What is a stand-in (locomotor_world.h declares them; the definitions are below): the rest
** of Object, PhysicsBehavior, BodyModule, TheTerrainLogic, ThePartitionManager,
** TheAI->pathfinder(), TheGlobalData and TheGameLogic. They answer from the scene (LocoEnv)
** the caller installs for each call, and LOG every call that crosses the cut, with its
** arguments, in order.
**
** Entry points (extern "C", tests/differential_locomotor.rs, which documents the encodings):
** ref_loco_reset, ref_loco_new, ref_loco_call (one Locomotor operation), ref_loco_obj (an
** operation on the object), ref_loco_xfer, ref_loco_dump_store, ref_locoset_*. The INI block
** is read by the REAL INI::load (ref_ini_* of wrap_ini.cpp), whose "Locomotor" handler is
** the Locomotor.cpp one (the weak STUB of wrap_ini.cpp gives way to it). */
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <string>
#include <vector>
#include <list>
#include <map>
#include <set>
#include <algorithm>
#include <initializer_list>
#define private public
#define protected public
#include "PreRTS.h"
#include "Common/NameKeyGenerator.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "locomotor_world.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Locomotor.cpp"
#undef private
#undef protected

/* ------------------------------------------------------------------------- */
/* The scene and its log                                                      */
/* ------------------------------------------------------------------------- */

LocoEnv g_locoEnv;
std::vector<unsigned> *g_locoLog = 0;
int g_locoQuiet = 0;

namespace {

unsigned bitsOf(float f) { unsigned u; memcpy(&u, &f, 4); return u; }
float floatOf(unsigned u) { float f; memcpy(&f, &u, 4); return f; }

/* one log record: code, number of words, the words */
void L(int code, std::initializer_list<unsigned> w)
{
	if (g_locoQuiet) return;
	g_locoLog->push_back((unsigned)code);
	g_locoLog->push_back((unsigned)w.size());
	for (unsigned x : w) g_locoLog->push_back(x);
}

Real groundAt(Real x, Real y) { return g_locoEnv.ga * x + g_locoEnv.gb * y + g_locoEnv.gc; }

Coord3D g_velocity;
Object g_object;
LocoGlobalData g_globalData;
LocoGameLogic g_gameLogic;
LocoTerrainLogic g_terrain;
LocoPartitionManager g_partition;
LocoAI g_ai;

}  // namespace

LocoGlobalData *LocoTheGlobalData = &g_globalData;
LocoGameLogic *LocoTheGameLogic = &g_gameLogic;
LocoTerrainLogic *LocoTheTerrainLogic = &g_terrain;
LocoPartitionManager *LocoThePartitionManager = &g_partition;
LocoAI *LocoTheAI = &g_ai;

UnsignedInt GameLogic::getFrame() const { return g_locoEnv.frame; }

Real TerrainLogic::getGroundHeight(Real x, Real y, Coord3D *normal) const
{
	L(LOG_GROUND_HEIGHT, { bitsOf(x), bitsOf(y) });
	return groundAt(x, y);
}

Real TerrainLogic::getLayerHeight(Real x, Real y, PathfindLayerEnum layer, Coord3D *normal, Bool clip) const
{
	L(LOG_LAYER_HEIGHT, { bitsOf(x), bitsOf(y), (unsigned)layer, (unsigned)clip });
	Real h = groundAt(x, y);
	if (layer != LAYER_GROUND) h = h + g_locoEnv.layerOffset;
	if (!clip) h = h + 0.5f;
	return h;
}

Bool TerrainLogic::isUnderwater(Real x, Real y, Real *waterZ, Real *terrainZ)
{
	L(LOG_UNDERWATER, { bitsOf(x), bitsOf(y) });
	if (x < g_locoEnv.wx) {
		if (waterZ) *waterZ = g_locoEnv.wz;
		if (terrainZ) *terrainZ = groundAt(x, y);
		return TRUE;
	}
	if (terrainZ) *terrainZ = groundAt(x, y);
	return FALSE;
}

PathfindLayerEnum TerrainLogic::getHighestLayerForDestination(const Coord3D *pos, Bool onlyHealthyBridges)
{
	L(LOG_HIGHEST_LAYER, { bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) });
	return (PathfindLayerEnum)g_locoEnv.highestLayer;
}

Int TerrainLogic::alignOnTerrain(Real angle, const Coord3D &pos, Bool stickToGround, Matrix3D &mtx)
{
	/* only for KINDOF_STICK_TO_TERRAIN_SLOPE things, which this scene has none of */
	abort();
}

Real PartitionManager::getGroundOrStructureHeight(Real posx, Real posy)
{
	L(LOG_STRUCT_HEIGHT, { bitsOf(posx), bitsOf(posy) });
	return groundAt(posx, posy) + g_locoEnv.structOffset;
}

Bool Pathfinder::validMovementTerrain(PathfindLayerEnum layer, const Locomotor *locomotor, const Coord3D *pos)
{
	L(LOG_VALID_TERRAIN, { (unsigned)layer, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) });
	if (g_locoEnv.invEnabled &&
		pos->x >= g_locoEnv.invBox[0] && pos->x <= g_locoEnv.invBox[1] &&
		pos->y >= g_locoEnv.invBox[2] && pos->y <= g_locoEnv.invBox[3])
		return FALSE;
	return TRUE;
}

BodyDamageType BodyModuleInterface::getDamageState() const { return (BodyDamageType)g_locoEnv.bodyState; }

Bool PhysicsBehavior::getIsStunned() const { return g_locoEnv.stunned != 0; }
void PhysicsBehavior::applyMotiveForce(const Coord3D *force) { L(LOG_APPLY_FORCE, { bitsOf(force->x), bitsOf(force->y), bitsOf(force->z) }); }
void PhysicsBehavior::scrubVelocity2D(Real desiredVelocity) { L(LOG_SCRUB, { bitsOf(desiredVelocity) }); }
void PhysicsBehavior::setTurning(PhysicsTurningType turning) { L(LOG_SET_TURNING, { (unsigned)(Int)turning }); }
const Coord3D *PhysicsBehavior::getVelocity() const
{
	g_velocity.x = g_locoEnv.vel[0];
	g_velocity.y = g_locoEnv.vel[1];
	g_velocity.z = g_locoEnv.vel[2];
	return &g_velocity;
}
Real PhysicsBehavior::getVelocityMagnitude() const { return g_locoEnv.velMag; }
Real PhysicsBehavior::getForwardSpeed2D() const { return g_locoEnv.fwd2d; }
Real PhysicsBehavior::getForwardSpeed3D() const { return g_locoEnv.fwd3d; }
Real PhysicsBehavior::getMass() const { return g_locoEnv.mass; }
Bool PhysicsBehavior::isMotive() const { return g_locoEnv.motive != 0; }
void PhysicsBehavior::setExtraFriction(Real friction) { L(LOG_SET_EXTRA_FRICTION, { bitsOf(friction) }); }
void PhysicsBehavior::setAllowAirborneFriction(Bool allow) { L(LOG_SET_AIRBORNE_FRICTION, { (unsigned)allow }); }
void PhysicsBehavior::setStickToGround(Bool stick) { L(LOG_SET_STICK, { (unsigned)stick }); }

Real GeometryInfo::getMajorRadius() const { return g_locoEnv.majorRadius; }
Real GeometryInfo::getBoundingCircleRadius() const { return g_locoEnv.boundingCircleRadius; }

/* the owner's reaction to a transform change (Thing's pure virtual): logged with the NEW state */
void Object::reactToTransformChange(const Matrix3D *oldMtx, const Coord3D *oldPos, Real oldAngle)
{
	L(LOG_TRANSFORM, { bitsOf(m_transform[0][0]), bitsOf(m_transform[0][1]), bitsOf(m_transform[0][2]), bitsOf(m_transform[0][3]),
		bitsOf(m_transform[1][0]), bitsOf(m_transform[1][1]), bitsOf(m_transform[1][2]), bitsOf(m_transform[1][3]),
		bitsOf(m_transform[2][0]), bitsOf(m_transform[2][1]), bitsOf(m_transform[2][2]), bitsOf(m_transform[2][3]),
		bitsOf(m_cachedPos.x), bitsOf(m_cachedPos.y), bitsOf(m_cachedPos.z), bitsOf(m_cachedAngle) });
}

Object::Object()
{
	m_transform.Make_Identity();
	m_cachedPos.x = m_cachedPos.y = m_cachedPos.z = 0.0f;
	m_cachedAngle = 0.0f;
	m_cachedDirVector.x = m_cachedDirVector.y = m_cachedDirVector.z = 0.0f;
	m_cachedAltitudeAboveTerrain = 0.0f;
	m_cachedAltitudeAboveTerrainOrWater = 0.0f;
	m_cacheFlags = 0;
	m_status = 0;
}

PhysicsBehavior *Object::getPhysics() { return g_locoEnv.hasPhysics ? &m_physics : 0; }
BodyModuleInterface *Object::getBodyModule() { return &m_body; }
PathfindLayerEnum Object::getLayer() const { return (PathfindLayerEnum)g_locoEnv.layer; }
Bool Object::isKindOf(KindOfType t) const
{
	if (t == KINDOF_PROJECTILE) return (g_locoEnv.kindMask & 1) != 0;
	if (t == KINDOF_DOZER) return (g_locoEnv.kindMask & 2) != 0;
	return FALSE;
}
ObjectStatusMaskType Object::getStatusBits() const { ObjectStatusMaskType m; m.m_bits = m_status; return m; }
void Object::setStatus(ObjectStatusMaskType objectStatus, Bool set)
{
	int bit = 0;
	while (bit < 63 && !((objectStatus.m_bits >> bit) & 1)) ++bit;
	L(LOG_SET_STATUS, { (unsigned)bit, (unsigned)set });
	if (set) m_status |= objectStatus.m_bits; else m_status &= ~objectStatus.m_bits;
}
Real Object::getCarrierDeckHeight() const { return g_locoEnv.deckHeight; }
const GeometryInfo &Object::getGeometryInfo() const { return m_geometry; }
Bool Object::isDisabledByType(DisabledType type) const { return type == DISABLED_HELD && g_locoEnv.disabledHeld != 0; }
void Object::setModelConditionState(ModelConditionFlagType a) { L(LOG_SET_MODEL_COND, { (unsigned)a }); }
void Object::clearModelConditionState(ModelConditionFlagType a) { L(LOG_CLEAR_MODEL_COND, { (unsigned)a }); }

/* isAboveTerrain() of Thing.h: getHeightAboveTerrain() > 0; its ground-height query is the
** Thing machinery's, not the locomotor's, so it is not logged */
Bool Object::isAboveTerrain() const
{
	++g_locoQuiet;
	Bool r = getHeightAboveTerrain() > 0.0f;
	--g_locoQuiet;
	return r;
}

/* ---- Thing, verbatim (build.rs: `Thing::` is read as `Object::`) */
#include "thing_methods.inc"
#include "thing_heights.inc"

/* ------------------------------------------------------------------------- */
/* Entry points                                                               */
/* ------------------------------------------------------------------------- */

namespace {

std::vector<Locomotor *> g_locos;
std::vector<LocomotorSet *> g_sets;
std::vector<unsigned> g_dumpWords;

void putU32(unsigned v) { g_dumpWords.push_back(v); }
void putReal(Real r) { g_dumpWords.push_back(bitsOf(r)); }
void putStr(const AsciiString &s)
{
	putU32((unsigned)s.getLength());
	const char *p = s.str();
	for (int i = 0; i < s.getLength(); ++i) putU32((unsigned char)p[i]);
}

/* one LocomotorTemplate, in the order of the declarations in Locomotor.h */
void dumpTemplate(const LocomotorTemplate &t)
{
	putStr(t.m_name);
	putU32((unsigned)t.m_surfaces);
	putReal(t.m_maxSpeed);
	putReal(t.m_maxSpeedDamaged);
	putReal(t.m_minSpeed);
	putReal(t.m_maxTurnRate);
	putReal(t.m_maxTurnRateDamaged);
	putReal(t.m_acceleration);
	putReal(t.m_accelerationDamaged);
	putReal(t.m_lift);
	putReal(t.m_liftDamaged);
	putReal(t.m_braking);
	putReal(t.m_minTurnSpeed);
	putReal(t.m_preferredHeight);
	putReal(t.m_preferredHeightDamping);
	putReal(t.m_circlingRadius);
	putReal(t.m_speedLimitZ);
	putReal(t.m_extra2DFriction);
	putReal(t.m_maxThrustAngle);
	putU32((unsigned)t.m_behaviorZ);
	putU32((unsigned)t.m_appearance);
	putU32((unsigned)t.m_movePriority);
	putReal(t.m_accelPitchLimit);
	putReal(t.m_decelPitchLimit);
	putReal(t.m_bounceKick);
	putReal(t.m_pitchStiffness);
	putReal(t.m_rollStiffness);
	putReal(t.m_pitchDamping);
	putReal(t.m_rollDamping);
	putReal(t.m_pitchByZVelCoef);
	putReal(t.m_thrustRoll);
	putReal(t.m_wobbleRate);
	putReal(t.m_minWobble);
	putReal(t.m_maxWobble);
	putReal(t.m_forwardVelCoef);
	putReal(t.m_lateralVelCoef);
	putReal(t.m_forwardAccelCoef);
	putReal(t.m_lateralAccelCoef);
	putReal(t.m_uniformAxialDamping);
	putReal(t.m_turnPivotOffset);
	putU32((unsigned)t.m_airborneTargetingHeight);
	putReal(t.m_closeEnoughDist);
	putU32(t.m_isCloseEnoughDist3D ? 1 : 0);
	putReal(t.m_ultraAccurateSlideIntoPlaceFactor);
	putU32(t.m_locomotorWorksWhenDead ? 1 : 0);
	putU32(t.m_allowMotiveForceWhileAirborne ? 1 : 0);
	putU32(t.m_apply2DFrictionWhenAirborne ? 1 : 0);
	putU32(t.m_downhillOnly ? 1 : 0);
	putU32(t.m_stickToGround ? 1 : 0);
	putU32(t.m_canMoveBackward ? 1 : 0);
	putU32(t.m_hasSuspension ? 1 : 0);
	putReal(t.m_maximumWheelExtension);
	putReal(t.m_maximumWheelCompression);
	putReal(t.m_wheelTurnAngle);
	putReal(t.m_wanderWidthFactor);
	putReal(t.m_wanderLengthFactor);
	putReal(t.m_wanderAboutPointRadius);
	putReal(t.m_rudderCorrectionDegree);
	putReal(t.m_rudderCorrectionRate);
	putReal(t.m_elevatorCorrectionDegree);
	putReal(t.m_elevatorCorrectionRate);
}

int finishWords(unsigned *buf, int cap)
{
	int n = (int)g_dumpWords.size();
	if (n <= cap && n > 0) memcpy(buf, &g_dumpWords[0], 4 * n);
	return n;
}

}  // namespace

extern "C" {

/* a NameKeyGenerator (installed as TheNameKeyGenerator, reset: keys start at 1 again),
** an empty LocomotorStore (TheLocomotorStore), an empty scene and object, no Locomotors */
void ref_loco_reset(void)
{
	static NameKeyGenerator *gen = 0;
	if (!gen) gen = new NameKeyGenerator;
	gen->reset();
	TheNameKeyGenerator = gen;
	for (size_t i = 0; i < g_locos.size(); ++i) g_locos[i]->deleteInstance();
	g_locos.clear();
	for (size_t i = 0; i < g_sets.size(); ++i) delete g_sets[i];
	g_sets.clear();
	delete TheLocomotorStore;
	TheLocomotorStore = new LocomotorStore;
	memset(&g_locoEnv, 0, sizeof g_locoEnv);
	g_locoQuiet = 0;
	static std::vector<unsigned> log;
	g_locoLog = &log;
	log.clear();
	g_object = Object();
	g_globalData.m_gravity = 0.0f;
	g_globalData.m_movementPenaltyDamageState = 0;
}

unsigned ref_loco_name_key(const char *name) { return (unsigned)TheNameKeyGenerator->nameToKey(name); }

/* batch A1, for the ThingTemplate build (wrap_thingtemplate.cpp): a fresh TheLocomotorStore without touching the
** name keys (ref_tt_reset makes them), the template of a key (findLocomotorTemplate, as AIUpdateModuleData's
** parseLocomotorSet asks), and the key of a template the store holds (0 if none does) */
void ref_loco_store_new(void)
{
	for (size_t i = 0; i < g_locos.size(); ++i) g_locos[i]->deleteInstance();
	g_locos.clear();
	delete TheLocomotorStore;
	TheLocomotorStore = new LocomotorStore;
}
const void *ref_loco_find_template(unsigned key)
{
	return TheLocomotorStore ? TheLocomotorStore->findLocomotorTemplate((NameKeyType)key) : 0;
}
unsigned ref_loco_template_key(const void *tmpl)
{
	for (LocomotorStore::LocomotorTemplateMap::const_iterator it = TheLocomotorStore->m_locomotorTemplates.begin();
			 it != TheLocomotorStore->m_locomotorTemplates.end(); ++it)
		if ((const void *)it->second == tmpl) return (unsigned)it->first;
	return 0;
}

/* the dump of every LocomotorTemplate: the count, then per template (in key order) its key
** and its chain: the number of nodes, then per node the override flag and the fields (see
** dumpTemplate). Returns the length in words (may exceed cap). */
int ref_loco_dump_store(unsigned *buf, int cap)
{
	g_dumpWords.clear();
	putU32((unsigned)TheLocomotorStore->m_locomotorTemplates.size());
	for (LocomotorStore::LocomotorTemplateMap::const_iterator it = TheLocomotorStore->m_locomotorTemplates.begin();
		it != TheLocomotorStore->m_locomotorTemplates.end(); ++it) {
		putU32((unsigned)it->first);
		int n = 0;
		for (LocomotorTemplate *t = it->second; t; t = (LocomotorTemplate *)t->friend_getNextOverride()) ++n;
		putU32((unsigned)n);
		for (LocomotorTemplate *t = it->second; t; t = (LocomotorTemplate *)t->friend_getNextOverride()) {
			putU32(t->m_isOverride ? 1 : 0);
			dumpTemplate(*t);
		}
	}
	return finishWords(buf, cap);
}

/* LocomotorStore::reset() (cleans up the overrides) */
void ref_loco_store_reset(void) { TheLocomotorStore->reset(); }

/* newLocomotor(findLocomotorTemplate(NAMEKEY(name))) at `frame`. Returns the handle, or -1
** if the store has no such template. */
int ref_loco_new(const char *name, unsigned frame)
{
	g_locoEnv.frame = frame;
	g_locoQuiet = 0;
	const LocomotorTemplate *t = TheLocomotorStore->findLocomotorTemplate(TheNameKeyGenerator->nameToKey(name));
	if (!t) return -1;
	g_locos.push_back(TheLocomotorStore->newLocomotor(t));
	return (int)g_locos.size() - 1;
}

/* Locomotor::xfer through the real XferSave to `path`: the file's bytes (returns the XferStatus
** thrown, 0 none, -1 anything else; *len the file's length) */
int ref_loco_xfer_save(int handle, const char *path, unsigned char *out, int cap, int *len)
{
	int status = 0;
	{
		XferSave x;
		try {
			x.open(AsciiString(path));
			x.xferSnapshot(g_locos[handle]);
		} catch (XferStatus s) { status = (int)s; } catch (...) { status = -1; }
		try { x.close(); } catch (...) {}
	}
	*len = 0;
	if (FILE *f = fopen(path, "rb")) {
		*len = (int)fread(out, 1, cap, f);
		fclose(f);
	}
	return status;
}

/* XferLoad of the bytes at `path` into a NEW Locomotor on the template of `name` (made at
** `frame`); returns the XferStatus thrown (0 none), and registers the Locomotor (its handle
** in *newHandle, -1 if there is no such template) */
int ref_loco_xfer_load(const char *name, unsigned frame, const char *path, int *newHandle)
{
	*newHandle = ref_loco_new(name, frame);
	if (*newHandle < 0) return 0;
	int status = 0;
	XferLoad x;
	try {
		x.open(AsciiString(path));
		x.xferSnapshot(g_locos[*newHandle]);
	} catch (XferStatus s) { status = (int)s; } catch (...) { status = -1; }
	try { x.close(); } catch (...) {}
	return status;
}

/* An operation on the object (the Thing part), no scene: op 0 setTransformMatrix(fa[0..12],
** row-major), 1 setPosition(fa[0..3]), 2 setOrientation(fa[0]), 3 setStatus(bit ia[0], set
** ia[1]), 4 setPositionZ(fa[0]), 5 clear the status bits. The log is not recorded. */
void ref_loco_obj(int op, const float *fa, const int *ia)
{
	std::vector<unsigned> scratch;
	std::vector<unsigned> *saved = g_locoLog;
	g_locoLog = &scratch;
	switch (op) {
	case 0: {
		Matrix3D m(fa[0], fa[1], fa[2], fa[3], fa[4], fa[5], fa[6], fa[7], fa[8], fa[9], fa[10], fa[11]);
		g_object.setTransformMatrix(&m);
		break;
	}
	case 1: { Coord3D p; p.x = fa[0]; p.y = fa[1]; p.z = fa[2]; g_object.setPosition(&p); break; }
	case 2: g_object.setOrientation(fa[0]); break;
	case 3: g_object.setStatus(makeObjectStatusMask(ia[0]), ia[1] != 0); break;
	case 4: g_object.setPositionZ(fa[0]); break;
	case 5: g_object.m_status = 0; break;
	}
	g_locoLog = saved;
}

/* The object's state: out[0..12] the transform (row-major, bits), out[12..15] the cached
** position, out[15] the cached angle, out[16..19] the cached direction vector, out[19] the
** cache flags, out[20] the cached altitude above terrain, out[21], out[22] the status bits
** (low, high). */
void ref_loco_obj_state(unsigned *out)
{
	for (int r = 0; r < 3; ++r)
		for (int c = 0; c < 4; ++c) out[4 * r + c] = bitsOf(g_object.m_transform[r][c]);
	out[12] = bitsOf(g_object.m_cachedPos.x);
	out[13] = bitsOf(g_object.m_cachedPos.y);
	out[14] = bitsOf(g_object.m_cachedPos.z);
	out[15] = bitsOf(g_object.m_cachedAngle);
	out[16] = bitsOf(g_object.m_cachedDirVector.x);
	out[17] = bitsOf(g_object.m_cachedDirVector.y);
	out[18] = bitsOf(g_object.m_cachedDirVector.z);
	out[19] = (unsigned)g_object.m_cacheFlags;
	out[20] = bitsOf(g_object.m_cachedAltitudeAboveTerrain);
	out[21] = (unsigned)(g_object.m_status & 0xffffffffULL);
	out[22] = (unsigned)(g_object.m_status >> 32);
}

/* One Locomotor operation in the scene `env`; fa (8 floats) and ia (4 ints) are the operands,
** rf (16 floats, as bits) and ri (4 ints) the results, the log goes to log[0..cap) (*logLen
** its length, may exceed cap).
**   0 locoUpdate_moveTowardsPosition(goal fa[0..3], onPathDistToGoal fa[3], desiredSpeed
**     fa[4], blocked ia[0]); ri[0] = blocked afterwards
**   1 locoUpdate_moveTowardsAngle(fa[0])
**   2 locoUpdate_maintainCurrentPosition; ri[0] = the result
**   3 setPhysicsOptions
**   4 startMove
**   5 the getters for the condition ia[0]: rf[0] getMaxSpeedForCondition, rf[1] getMaxTurnRate,
**     rf[2] getMaxAcceleration, rf[3] getMaxLift, rf[4] getBraking, rf[5], rf[6]
**     calcMinTurnRadius (radius, time)
**   10 + k: the setters: setMaxLift(fa[0]) 10, setMaxSpeed 11, setMaxAcceleration 12,
**     setMaxBraking 13, setMaxTurnRate 14, setAllowInvalidPosition(ia[0]) 15,
**     setCloseEnoughDist(fa[0]) 16, setCloseEnoughDist3D(ia[0]) 17, setPreferredHeight(fa[0])
**     18, setUsePreciseZPos(ia[0]) 19, setNoSlowDownAsApproachingDest(ia[0]) 20,
**     setUltraAccurate(ia[0]) 21, restorePreferredHeightFromTemplate 22 */
void ref_loco_call(int handle, int op, const LocoEnv *env, const float *fa, const int *ia, unsigned *rf, int *ri,
	unsigned *log, int cap, int *logLen)
{
	g_locoEnv = *env;
	g_locoQuiet = 0;
	g_locoLog->clear();
	Locomotor *l = g_locos[handle];
	Object *obj = &g_object;
	TheGlobalData->m_gravity = env->gravity;
	TheGlobalData->m_movementPenaltyDamageState = env->penalty;
	switch (op) {
	case 0: {
		Coord3D goal; goal.x = fa[0]; goal.y = fa[1]; goal.z = fa[2];
		Bool blocked = ia[0] != 0;
		l->locoUpdate_moveTowardsPosition(obj, goal, fa[3], fa[4], &blocked);
		ri[0] = blocked ? 1 : 0;
		break;
	}
	case 1: l->locoUpdate_moveTowardsAngle(obj, fa[0]); break;
	case 2: ri[0] = l->locoUpdate_maintainCurrentPosition(obj) ? 1 : 0; break;
	case 3: l->setPhysicsOptions(obj); break;
	case 4: l->startMove(); break;
	case 5: {
		BodyDamageType c = (BodyDamageType)ia[0];
		rf[0] = bitsOf(l->getMaxSpeedForCondition(c));
		rf[1] = bitsOf(l->getMaxTurnRate(c));
		rf[2] = bitsOf(l->getMaxAcceleration(c));
		rf[3] = bitsOf(l->getMaxLift(c));
		rf[4] = bitsOf(l->getBraking());
		Real t = 0.0f;
		rf[5] = bitsOf(l->calcMinTurnRadius(c, &t));
		rf[6] = bitsOf(t);
		break;
	}
	case 10: l->setMaxLift(fa[0]); break;
	case 11: l->setMaxSpeed(fa[0]); break;
	case 12: l->setMaxAcceleration(fa[0]); break;
	case 13: l->setMaxBraking(fa[0]); break;
	case 14: l->setMaxTurnRate(fa[0]); break;
	case 15: l->setAllowInvalidPosition(ia[0] != 0); break;
	case 16: l->setCloseEnoughDist(fa[0]); break;
	case 17: l->setCloseEnoughDist3D(ia[0] != 0); break;
	case 18: l->setPreferredHeight(fa[0]); break;
	case 19: l->setUsePreciseZPos(ia[0] != 0); break;
	case 20: l->setNoSlowDownAsApproachingDest(ia[0] != 0); break;
	case 21: l->setUltraAccurate(ia[0] != 0); break;
	case 22: l->restorePreferredHeightFromTemplate(); break;
	}
	int n = (int)g_locoLog->size();
	*logLen = n;
	if (n <= cap && n > 0) memcpy(log, &(*g_locoLog)[0], 4 * n);
}

/* the Locomotor's data members in Locomotor::xfer's order (after the version): donutTimer,
** maintainPos x y z, brakingFactor, maxLift, maxSpeed, maxAccel, maxBraking, maxTurnRate,
** closeEnoughDist, flags, preferredHeight, preferredHeightDamping, angleOffset, offsetIncrement
** (16 words) */
void ref_loco_state(int handle, unsigned *o)
{
	const Locomotor *l = g_locos[handle];
	int n = 0;
	o[n++] = (unsigned)l->m_donutTimer;
	o[n++] = bitsOf(l->m_maintainPos.x);
	o[n++] = bitsOf(l->m_maintainPos.y);
	o[n++] = bitsOf(l->m_maintainPos.z);
	o[n++] = bitsOf(l->m_brakingFactor);
	o[n++] = bitsOf(l->m_maxLift);
	o[n++] = bitsOf(l->m_maxSpeed);
	o[n++] = bitsOf(l->m_maxAccel);
	o[n++] = bitsOf(l->m_maxBraking);
	o[n++] = bitsOf(l->m_maxTurnRate);
	o[n++] = bitsOf(l->m_closeEnoughDist);
	o[n++] = (unsigned)l->m_flags;
	o[n++] = bitsOf(l->m_preferredHeight);
	o[n++] = bitsOf(l->m_preferredHeightDamping);
	o[n++] = bitsOf(l->m_angleOffset);
	o[n++] = bitsOf(l->m_offsetIncrement);
}

/* the getters that need no scene: out = appearance, legal surfaces, min speed, the pitch and
** wheel fields... (every inline getter of Locomotor.h, in the order of the Rust test) */
void ref_loco_getters(int handle, unsigned *o)
{
	const Locomotor *l = g_locos[handle];
	int n = 0;
	o[n++] = (unsigned)l->getAppearance();
	o[n++] = (unsigned)l->getMovePriority();
	o[n++] = (unsigned)l->getLegalSurfaces();
	o[n++] = bitsOf(l->getMinSpeed());
	o[n++] = bitsOf(l->getAccelPitchLimit());
	o[n++] = bitsOf(l->getDecelPitchLimit());
	o[n++] = bitsOf(l->getBounceKick());
	o[n++] = bitsOf(l->getPitchStiffness());
	o[n++] = bitsOf(l->getRollStiffness());
	o[n++] = bitsOf(l->getPitchDamping());
	o[n++] = bitsOf(l->getRollDamping());
	o[n++] = bitsOf(l->getPitchByZVelCoef());
	o[n++] = bitsOf(l->getThrustRoll());
	o[n++] = bitsOf(l->getWobbleRate());
	o[n++] = bitsOf(l->getMaxWobble());
	o[n++] = bitsOf(l->getMinWobble());
	o[n++] = bitsOf(l->getForwardVelCoef());
	o[n++] = bitsOf(l->getLateralVelCoef());
	o[n++] = bitsOf(l->getForwardAccelCoef());
	o[n++] = bitsOf(l->getLateralAccelCoef());
	o[n++] = bitsOf(l->getUniformAxialDamping());
	o[n++] = bitsOf(l->getTurnPivotOffset());
	o[n++] = l->getApply2DFrictionWhenAirborne() ? 1 : 0;
	o[n++] = l->getIsDownhillOnly() ? 1 : 0;
	o[n++] = l->getAllowMotiveForceWhileAirborne() ? 1 : 0;
	o[n++] = (unsigned)l->getAirborneTargetingHeight();
	o[n++] = l->getLocomotorWorksWhenDead() ? 1 : 0;
	o[n++] = l->getStickToGround() ? 1 : 0;
	o[n++] = bitsOf(l->getCloseEnoughDist());
	o[n++] = l->isCloseEnoughDist3D() ? 1 : 0;
	o[n++] = l->hasSuspension() ? 1 : 0;
	o[n++] = l->canMoveBackwards() ? 1 : 0;
	o[n++] = bitsOf(l->getMaxWheelExtension());
	o[n++] = bitsOf(l->getMaxWheelCompression());
	o[n++] = bitsOf(l->getWheelTurnAngle());
	o[n++] = bitsOf(l->getRudderCorrectionDegree());
	o[n++] = bitsOf(l->getRudderCorrectionRate());
	o[n++] = bitsOf(l->getElevatorCorrectionDegree());
	o[n++] = bitsOf(l->getElevatorCorrectionRate());
	o[n++] = bitsOf(l->getWanderWidthFactor());
	o[n++] = bitsOf(l->getWanderAboutPointRadius());
	o[n++] = bitsOf(l->getPreferredHeight());
	o[n++] = bitsOf(l->getPreferredHeightDamping());
	o[n++] = l->isUltraAccurate() ? 1 : 0;
	o[n++] = l->isMovingBackwards() ? 1 : 0;
	o[n++] = l->isInvalidPositionAllowed() ? 1 : 0;
	o[n++] = (unsigned)l->m_donutTimer;
}

/* ---- LocomotorSet */

int ref_locoset_new(void)
{
	g_sets.push_back(new LocomotorSet);
	return (int)g_sets.size() - 1;
}

/* addLocomotor(findLocomotorTemplate(NAMEKEY(name))) at `frame`; 1 if there is no such template */
int ref_locoset_add(int set, const char *name, unsigned frame)
{
	g_locoEnv.frame = frame;
	const LocomotorTemplate *t = TheLocomotorStore->findLocomotorTemplate(TheNameKeyGenerator->nameToKey(name));
	if (!t) return 1;
	g_sets[set]->addLocomotor(t);
	return 0;
}

void ref_locoset_clear(int set) { g_sets[set]->clear(); }

/* findLocomotor(mask): the index in the set, or -1 */
int ref_locoset_find(int set, int mask)
{
	LocomotorSet *s = g_sets[set];
	Locomotor *l = s->findLocomotor(mask);
	if (!l) return -1;
	for (size_t i = 0; i < s->m_locomotors.size(); ++i)
		if (s->m_locomotors[i] == l) return (int)i;
	return -2;
}

/* count, valid surfaces, downhill flag, then per locomotor its template name (length, chars) */
int ref_locoset_dump(int set, unsigned *buf, int cap)
{
	LocomotorSet *s = g_sets[set];
	g_dumpWords.clear();
	putU32((unsigned)s->m_locomotors.size());
	putU32((unsigned)s->getValidSurfaces());
	putU32(s->isDownhillOnly() ? 1 : 0);
	for (size_t i = 0; i < s->m_locomotors.size(); ++i) putStr(s->m_locomotors[i]->getTemplateName());
	return finishWords(buf, cap);
}

/* the data members of locomotor `index` of the set, as ref_loco_state gives them */
void ref_locoset_state(int set, int index, unsigned *o)
{
	const Locomotor *l = g_sets[set]->m_locomotors[index];
	int n = 0;
	o[n++] = (unsigned)l->m_donutTimer;
	o[n++] = bitsOf(l->m_maintainPos.x);
	o[n++] = bitsOf(l->m_maintainPos.y);
	o[n++] = bitsOf(l->m_maintainPos.z);
	o[n++] = bitsOf(l->m_brakingFactor);
	o[n++] = bitsOf(l->m_maxLift);
	o[n++] = bitsOf(l->m_maxSpeed);
	o[n++] = bitsOf(l->m_maxAccel);
	o[n++] = bitsOf(l->m_maxBraking);
	o[n++] = bitsOf(l->m_maxTurnRate);
	o[n++] = bitsOf(l->m_closeEnoughDist);
	o[n++] = (unsigned)l->m_flags;
	o[n++] = bitsOf(l->m_preferredHeight);
	o[n++] = bitsOf(l->m_preferredHeightDamping);
	o[n++] = bitsOf(l->m_angleOffset);
	o[n++] = bitsOf(l->m_offsetIncrement);
}

/* xferSelfAndCurLocoPtr through XferSave to `path`, with the current locomotor at `cur` (-1:
** none); the file's bytes. Returns the XferStatus thrown (0 none, -1 other). */
int ref_locoset_xfer_save(int set, int cur, const char *path, unsigned char *out, int cap, int *len)
{
	LocomotorSet *s = g_sets[set];
	Locomotor *loco = cur >= 0 ? s->m_locomotors[cur] : 0;
	int status = 0;
	{
		XferSave x;
		try {
			x.open(AsciiString(path));
			s->xferSelfAndCurLocoPtr(&x, &loco);
		} catch (XferStatus st) { status = (int)st; } catch (...) { status = -1; }
		try { x.close(); } catch (...) {}
	}
	*len = 0;
	if (FILE *f = fopen(path, "rb")) {
		*len = (int)fread(out, 1, cap, f);
		fclose(f);
	}
	return status;
}

/* xferSelfAndCurLocoPtr through XferLoad of `path` into set `set` (which should be empty),
** at `frame` (the constructor's frame); *cur gets the current locomotor's index (-1 none).
** Returns the XferStatus thrown (0 none, -1 other). */
int ref_locoset_xfer_load(int set, unsigned frame, const char *path, int *cur)
{
	LocomotorSet *s = g_sets[set];
	g_locoEnv.frame = frame;
	Locomotor *loco = 0;
	int status = 0;
	XferLoad x;
	try {
		x.open(AsciiString(path));
		s->xferSelfAndCurLocoPtr(&x, &loco);
	} catch (XferStatus st) { status = (int)st; } catch (...) { status = -1; }
	try { x.close(); } catch (...) {}
	*cur = -1;
	for (size_t i = 0; loco && i < s->m_locomotors.size(); ++i)
		if (s->m_locomotors[i] == loco) *cur = (int)i;
	return status;
}

}  /* extern "C" */
