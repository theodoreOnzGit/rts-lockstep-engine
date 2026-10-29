/* Differential-test wrapper for AIGroup.cpp (Phase 1 layer 10, p1w13-sonnet, batches A24 and A25; upstream commit 0a05454d): the
** group, its members, centre and speed, its commands and (A25) its moves, cut verbatim by build.rs into OUT_DIR/ai_gen/ai_group.inc
** (Line2D.cpp's Coord3DInsideRect2D and ScaleRect2D, then AIGroup.cpp from the constructor to the end) and compiled here over a world of this
** file's own: up to GRP_OBJECTS objects (ids 1..), each answering from the scene (GrpScene, the same layout as src/differential.rs's
** GrpScene) or from grpAnswer, a hash of (kind, object, argument) both sides compute; every call that changes something is logged
** (GRP_LOG_*), as are TheAI's ids and destroyGroup. The class AIGroup is declared here with AI.h's members (m_memberList a
** std::list<Object *>); Object::enterGroup and leaveGroup are logged cut points (upstream's leave the former group).
**
** ref_ai_group_run(scene, ops, nOps, out, outCap, log, logCap, &logLen): a group made, then each op (GrpOp: its kind and
** arguments) applied in turn; out: per op its answer words; then the group's members, count, speed (bits), dirty flag and id.
** Returns the word count. */
#include "Lib/BaseType.h"
#include <math.h>
#include <string.h>
#include <list>
#include <vector>
#include <algorithm>

/* (the world in a namespace of its own: its TheAI, TheGlobalData, AIUpdateInterface .. are other archives' names too) */
namespace grp_a24 {

#ifndef MAX
#define MAX(x, y) (((x) > (y)) ? (x) : (y))	/* (WWLib always.h's: a > b ? a : b) */
#endif
#define DEBUG_ASSERTCRASH(c, m)
#define DEBUG_CRASH(m)
#define DEBUG_LOG(m)
#define CRCGEN_LOG(m)

enum { GRP_OBJECTS = 12 };
typedef Int ObjectID;
#define INVALID_ID 0
typedef std::vector<ObjectID> VecObjectID;
enum CommandSourceType { CMD_FROM_PLAYER = 0, CMD_FROM_SCRIPT, CMD_FROM_AI, CMD_FROM_DOZER };
enum KindOfType { KINDOF_STRUCTURE = 0, KINDOF_ALWAYS_SELECTABLE, KINDOF_IMMOBILE, KINDOF_AIRCRAFT,
	KINDOF_INFANTRY, KINDOF_VEHICLE, KINDOF_MOB_NEXUS, KINDOF_PRODUCED_AT_HELIPAD, KINDOF_CLIFF_JUMPER };
struct KindOfMaskType { unsigned m_bits; KindOfMaskType() : m_bits(0) {} void set(KindOfType k) { m_bits |= 1u << k; } };
enum ObjectStatusTypes { OBJECT_STATUS_CAN_STEALTH = 0, OBJECT_STATUS_STEALTHED, OBJECT_STATUS_DETECTED };
struct ObjectStatusMaskType { unsigned m_bits; Bool test(ObjectStatusTypes s) const { return (m_bits >> s) & 1; } };
enum DisabledType { DISABLED_HELD = 3 };
enum FormationID { NO_FORMATION_ID = 0 };
typedef Int BodyDamageType;
#define IS_CONDITION_BETTER(a, b) ((a) < (b))
#define LOGICFRAMES_PER_SECOND 30
enum CanAttackResult { ATTACKRESULT_NOT_POSSIBLE, ATTACKRESULT_INVALID_SHOT, ATTACKRESULT_POSSIBLE_AFTER_MOVING, ATTACKRESULT_POSSIBLE };
enum AbleToAttackType { ATTACK_NEW_TARGET = 0, ATTACK_NEW_TARGET_FORCED = 1 };
enum GuardMode { GUARDMODE_NORMAL = 0 };
enum AttitudeType { AI_SLEEP = -2, AI_PASSIVE, AI_NORMAL, AI_ALERT, AI_AGGRESSIVE, AI_INVALID };
typedef Int PathfindLayerEnum;
enum { LAYER_GROUND = 1 };
#define PATHFIND_CELL_SIZE_F 10.0f
enum PlayerType { PLAYER_HUMAN = 0, PLAYER_COMPUTER };
enum LocomotorPriority { LOCO_MOVES_BACK = 0, LOCO_MOVES_MIDDLE = 1, LOCO_MOVES_FRONT = 2 };
enum AIDebugOptions { AI_DEBUG_NONE = 0, AI_DEBUG_PATHS, AI_DEBUG_TERRAIN };
typedef Int LocomotorSurfaceTypeMask;
typedef Int ScienceType;
#define SCIENCE_INVALID ((ScienceType)-1)
enum ModelConditionFlagType { MODELCONDITION_SPECIAL_CHEERING = 0 };
enum WeaponSlotType { PRIMARY_WEAPON = 0 };
enum WeaponLockType { NOT_LOCKED = 0 };
enum WeaponSetType { WEAPONSET_MINE_CLEARING_DETAIL = 0 };
struct WeaponSetFlags { unsigned m_bits; WeaponSetFlags() : m_bits(0) {} void set(WeaponSetType w) { m_bits = (unsigned)w; } };
enum UpgradeType { UPGRADE_TYPE_PLAYER = 0, UPGRADE_TYPE_OBJECT };
enum CanMakeType { CANMAKE_OK = 0, CANMAKE_QUEUE_FULL };
typedef Int GUICommandType;
enum { MAX_COMMANDS_PER_SET = 18 };
typedef Int SpecialPowerType;
enum IterOrderType { ITER_FASTEST, ITER_SORTED_NEAR_TO_FAR, ITER_SORTED_FAR_TO_NEAR };
typedef UnsignedByte XferVersion;

struct GrpObj
{
	Int id, flags;		/* GRP_F_* */
	Int kinds, status;	/* KindOfType bits; ObjectStatusTypes bits */
	Real pos[3];
	Int player, formation, damage;	/* player: 0..3 */
	Real locoSpeed;
	Int contained[3];	/* ids, 0: none */
	Int stealthDelay;	/* -1: no stealth module */
	Int commandSet;		/* -1: none; else a bit per command type 0..7 */
	/* batch A25 */
	Real formationOffset[2];
	Real boundingRadius, majorRadius;
	Int tmp;			/* the AI's m_tmpInt at the start */
	Int locoPriority;	/* -1: no current locomotor */
};
enum {
	GRP_F_AI = 1, GRP_F_HELD = 2, GRP_F_DEAD = 4, GRP_F_ABLE = 8, GRP_F_AIRBORNE = 16, GRP_F_CONTAIN = 32, GRP_F_FIRE = 64,
	GRP_F_SPAWN = 128, GRP_F_FREEDOM = 256, GRP_F_ACQUIRE = 512, GRP_F_ACQUIRE_STEALTHED = 1024, GRP_F_IDLE = 2048, GRP_F_BUSY = 4096,
	GRP_F_DRAWABLE = 8192, GRP_F_PRODUCTION = 16384, GRP_F_GROUND = 32768,
};
struct GrpScene
{
	GrpObj o[GRP_OBJECTS]; UnsignedInt frame, seed; Int penalty, nextGroupId, nextFormationId;
	/* batch A25 */
	Real extent[4], pathExtent[4];	/* lo.x, lo.y, hi.x, hi.y: the map's; the pathable one */
	Real gatherFactor; Int debugTerrain;
	Real distanceRequiresGroup, minDistanceForGroup; Int minInfantryForGroup, minVehiclesForGroup;
	Int playerTypes;	/* bit p: player p is a computer */
};
struct GrpOp { Int kind, a, b, c; Real x, y, z, w; };

namespace {
const GrpScene *g_s;
std::vector<unsigned> *g_log;
unsigned g_rand;
unsigned bitsOf(Real r) { unsigned u; memcpy(&u, &r, 4); return u; }
void L(std::initializer_list<unsigned> w) { g_log->insert(g_log->end(), w); }
}
/* the answer both sides compute: (kind, object, argument) through the scene's seed */
extern "C" unsigned grpAnswer(unsigned seed, unsigned kind, unsigned obj, unsigned arg)
{
	unsigned x = seed ^ (kind * 0x85ebca6bu);
	x ^= obj; x *= 0x9E3779B1u; x ^= x >> 15;
	x ^= arg; x *= 0x9E3779B1u; x ^= x >> 15;
	return x;
}
namespace {
Bool ans(unsigned kind, unsigned obj, unsigned arg) { return (grpAnswer(g_s->seed, kind, obj, arg) & 3) != 0; }
}

enum { GRP_LOG_ENTER = 1, GRP_LOG_LEAVE, GRP_LOG_DESTROY, GRP_LOG_AI, GRP_LOG_MOOD, GRP_LOG_SLAVES, GRP_LOG_EXIT_ALL, GRP_LOG_SP,
	GRP_LOG_DEFECTOR, GRP_LOG_CHEER, GRP_LOG_SELL, GRP_LOG_OVERCHARGE, GRP_LOG_BUTTON, GRP_LOG_ATTITUDE, GRP_LOG_WEAPONSET,
	GRP_LOG_LOCK, GRP_LOG_UPGRADE, GRP_LOG_EMOTICON, GRP_LOG_OVERRIDE, GRP_LOG_FORMATION, GRP_LOG_RANDOM,
	/* batch A25: the pathfinder */
	GRP_LOG_REMOVE_GOAL, GRP_LOG_ADJUST, GRP_LOG_UPDATE_GOAL, GRP_LOG_UPDATE_POS, GRP_LOG_LINE, GRP_LOG_GROUND_PATH };
/* GRP_LOG_AI's commands */
enum { GC_IDLE, GC_ATTACK_OBJECT, GC_FORCE_ATTACK_OBJECT, GC_ATTACK_TEAM, GC_ATTACK_POSITION, GC_ATTACK_MOVE, GC_MOVE, GC_HUNT,
	GC_REPAIR, GC_RESUME, GC_HEALED, GC_REPAIRED, GC_ENTER, GC_DOCK, GC_EXIT, GC_MOVE_EVACUATE, GC_EVACUATE, GC_RAILED, GC_PRONE,
	GC_GUARD_POSITION, GC_GUARD_OBJECT, GC_GUARD_AREA, GC_ATTACK_AREA, GC_HACK, GC_COMBAT_DROP,
	/* batch A25 */
	GC_FOLLOW_PATH, GC_FOLLOW_PATH_APPEND, GC_TIGHTEN, GC_WAYPOINT_PATH, GC_EVACUATE_AND_EXIT };

class Object;
class AIGroup;
class Player { public: Int m_index; Bool hasScience(ScienceType s) const { return ans(20, m_index, s); }
	PlayerType getPlayerType() const { return ((g_s->playerTypes >> m_index) & 1) ? PLAYER_COMPUTER : PLAYER_HUMAN; } };
class Team { public: Int m_index; };
class PolygonTrigger { public: Int m_index; };
class Waypoint { public: Int m_index; };
class DamageInfo { public: Int m_index; };
class UpgradeTemplate { public: Int m_index; UpgradeType getUpgradeType() const { return ans(30, 0, m_index) ? UPGRADE_TYPE_OBJECT : UPGRADE_TYPE_PLAYER; } };
class CommandButton { public: Int m_index; Int m_type; GUICommandType getCommandType() const { return m_type; } };
class AsciiString { public: const char *m_s; const Object *m_obj; };	/* (a command set's name: its object) */
class Coord3DHolder;
void getHelicopterOffset( Coord3D& posOut, Int idx );
/* batch A25: the ground path (its nodes in their optimised order), the locomotors, the geometry */
class PathNode { public: Coord3D m_pos; PathNode *m_next; const Coord3D *getPosition() const { return &m_pos; } PathNode *getNextOptimized() { return m_next; } };
class Path
{
public:
	std::vector<PathNode> m_nodes;
	PathNode *getFirstNode() { return &m_nodes[0]; }
	PathNode *getLastNode() { return &m_nodes[m_nodes.size() - 1]; }
	void deleteInstance() { delete this; }
};
class LocomotorSet { public: LocomotorSurfaceTypeMask getValidSurfaces() const { return 0; } };
class Locomotor { public: Object *m_obj; LocomotorPriority getMovePriority() const; };
class GeometryInfo { public: Object *m_obj; Real getBoundingCircleRadius() const; Real getMajorRadius() const; };

class AIUpdateInterface
{
public:
	Object *m_obj;
	void cmd(unsigned c, std::initializer_list<unsigned> w, CommandSourceType src);
	void aiIdle(CommandSourceType s);
	void aiAttackObject(Object *v, Int shots, CommandSourceType s);
	void aiForceAttackObject(Object *v, Int shots, CommandSourceType s);
	void aiAttackTeam(const Team *t, Int shots, CommandSourceType s);
	void aiAttackPosition(const Coord3D *p, Int shots, CommandSourceType s);
	void aiAttackMoveToPosition(const Coord3D *p, Int shots, CommandSourceType s);
	void aiMoveToPosition(const Coord3D *p, CommandSourceType s);
	void aiHunt(CommandSourceType s);
	void aiRepair(Object *o, CommandSourceType s);
	void aiResumeConstruction(Object *o, CommandSourceType s);
	void aiGetHealed(Object *o, CommandSourceType s);
	void aiGetRepaired(Object *o, CommandSourceType s);
	void aiEnter(Object *o, CommandSourceType s);
	void aiDock(Object *o, CommandSourceType s);
	void aiExit(Object *o, CommandSourceType s);
	void aiMoveToAndEvacuate(const Coord3D *p, CommandSourceType s);
	void aiEvacuate(Bool expose, CommandSourceType s);
	void aiExecuteRailedTransport(CommandSourceType s);
	void aiGoProne(const DamageInfo *d, CommandSourceType s);
	void aiGuardPosition(const Coord3D *p, GuardMode m, CommandSourceType s);
	void aiGuardObject(Object *o, GuardMode m, CommandSourceType s);
	void aiGuardArea(const PolygonTrigger *a, GuardMode m, CommandSourceType s);
	void aiAttackArea(const PolygonTrigger *a, CommandSourceType s);
	void aiHackInternet(CommandSourceType s);
	void aiCombatDrop(Object *t, const Coord3D &p, CommandSourceType s);
	void setAttitude(AttitudeType t);
	Bool canAutoAcquire() const;
	Bool canAutoAcquireWhileStealthed() const;
	void setNextMoodCheckTime(UnsignedInt t);
	Bool isIdle() const;
	Bool isBusy() const;
	Real getCurLocomotorSpeed() const;
	/* batch A25 */
	Locomotor m_loco;
	LocomotorSet m_locoSet;
	Bool isDoingGroundMovement() const;
	const LocomotorSet &getLocomotorSet() const { return m_locoSet; }
	Locomotor *getCurLocomotor();
	Int getTmpValue();
	void setTmpValue(Int v);
	void aiFollowPath(const std::vector<Coord3D> *path, Object *ignoreObject, CommandSourceType s);
	void aiFollowPathAppend(const Coord3D *p, CommandSourceType s);
	void aiTightenToPosition(const Coord3D *p, CommandSourceType s);
	void aiFollowWaypointPath(const Waypoint *w, CommandSourceType s);
	void aiFollowWaypointPathExact(const Waypoint *w, CommandSourceType s);
	void aiFollowWaypointPathAsTeam(const Waypoint *w, CommandSourceType s);
	void aiFollowWaypointPathExactAsTeam(const Waypoint *w, CommandSourceType s);
	void aiMoveToAndEvacuateAndExit(const Coord3D *p, CommandSourceType s);
};
class StealthUpdate { public: Object *m_obj; UnsignedInt getStealthDelay() const; };
typedef std::list<Object *> ContainedItemsList;
typedef void (*ContainIterateFunc)( Object *obj, void *userData );
class ContainModuleInterface
{
public:
	Object *m_obj;
	ContainedItemsList m_items;
	void iterateContained(ContainIterateFunc func, void *userData, Bool reverse) { for (ContainedItemsList::iterator i = m_items.begin(); i != m_items.end(); ++i) func(*i, userData); }
	Bool isPassengerAllowedToFire() const;
	const ContainedItemsList *getContainedItemsList() const { return &m_items; }
	void orderAllPassengersToExit(CommandSourceType s, Bool instantly);
};
class SpawnBehaviorInterface
{
public:
	Object *m_obj;
	void orderSlavesToGoIdle(CommandSourceType s);
	Bool doSlavesHaveFreedom() const;
	void orderSlavesToAttackTarget(Object *v, Int shots, CommandSourceType s);
	void orderSlavesToAttackPosition(const Coord3D *p, Int shots, CommandSourceType s);
};
class SpecialPowerTemplate { public: UnsignedInt m_id; ScienceType getRequiredScience() const { return ans(40, 0, m_id) ? SCIENCE_INVALID : (ScienceType)(m_id % 3); } };
class SpecialPowerStore
{
public:
	SpecialPowerTemplate m_t[8];
	const SpecialPowerTemplate *findSpecialPowerTemplateByID(UnsignedInt id) { if (id < 8 && ans(41, 0, id)) { m_t[id].m_id = id; return &m_t[id]; } return NULL; }
};
class SpecialPowerModuleInterface
{
public:
	Object *m_obj; UnsignedInt m_sp;
	void doSpecialPower(UnsignedInt opts);
	void doSpecialPowerAtLocation(const Coord3D *loc, Real angle, UnsignedInt opts);
	void doSpecialPowerAtObject(Object *t, UnsignedInt opts);
};
class SpecialPowerUpdateInterface { public: Object *m_obj; void setSpecialPowerOverridableDestination(const Coord3D *loc); };
class ActionManager
{
public:
	Bool canDoSpecialPower(const Object *o, const SpecialPowerTemplate *t, CommandSourceType s, UnsignedInt opts);
	Bool canDoSpecialPowerAtLocation(const Object *o, const Coord3D *loc, CommandSourceType s, const SpecialPowerTemplate *t, const Object *inWay, UnsignedInt opts);
	Bool canDoSpecialPowerAtObject(const Object *o, const Object *target, CommandSourceType s, const SpecialPowerTemplate *t, UnsignedInt opts);
};
class OverchargeBehaviorInterface { public: Object *m_obj; void toggle(); };
class BehaviorModule { public: OverchargeBehaviorInterface *m_oc; OverchargeBehaviorInterface *getOverchargeBehaviorInterface() { return m_oc; } };
class WeaponTemplateSet {};
class ThingTemplate { public: Object *m_obj; const WeaponTemplateSet *findWeaponTemplateSet(const WeaponSetFlags &f) const; };
class ProductionUpdateInterface
{
public:
	Object *m_obj;
	CanMakeType canQueueUpgrade(const UpgradeTemplate *u) const;
	void queueUpgrade(const UpgradeTemplate *u);
};
class CommandSet { public: Object *m_obj; CommandButton m_b[MAX_COMMANDS_PER_SET]; const CommandButton *getCommandButton(Int i) const; };
class ControlBar { public: CommandSet m_sets[GRP_OBJECTS]; const CommandSet *findCommandSet(const AsciiString &name); };
class Drawable { public: Object *m_obj; void setEmoticon(const AsciiString &name, Int duration); };
class BodyModuleInterface { public: Object *m_obj; BodyDamageType getDamageState() const; };
class BuildAssistant { public: void sellObject(Object *o); };
class UpgradeCenter { public: Bool canAffordUpgrade(Player *p, const UpgradeTemplate *u, Bool displayReason) const; };
struct AIData { Int m_minInfantryForGroup, m_minVehiclesForGroup; Real m_minDistanceForGroup, m_distanceRequiresGroup; };
class Pathfinder
{
public:
	void removeGoal(Object *obj);
	Bool adjustDestination(Object *obj, const LocomotorSet &ls, Coord3D *dest, const Coord3D *groupDest);
	void updateGoal(Object *obj, const Coord3D *pos, PathfindLayerEnum layer);
	void updatePos(Object *obj, const Coord3D *pos);
	Bool isLinePassable(const Object *obj, LocomotorSurfaceTypeMask surfaces, PathfindLayerEnum layer, const Coord3D &from, const Coord3D &to, Bool blocked, Bool allowPinched);
	Path *findGroundPath(const Coord3D *from, const Coord3D *to, Int pathRadius, Bool crusher);
};
class AI
{
public:
	Pathfinder m_pf; AIData m_data;
	Pathfinder *pathfinder() { return &m_pf; }
	const AIData *getAiData() { return &m_data; }
	UnsignedInt m_nextGroup, m_nextFormation;
	UnsignedInt getNextGroupID() { L({ GRP_LOG_RANDOM, 0u, m_nextGroup }); return m_nextGroup++; }
	FormationID getNextFormationID() { return (FormationID)m_nextFormation++; }
	void destroyGroup(AIGroup *g);
};
class GameLogic { public: UnsignedInt getFrame() const { return g_s->frame; } };
class TerrainLogic
{
public:
	PathfindLayerEnum getHighestLayerForDestination(const Coord3D *p) { return (PathfindLayerEnum)(grpAnswer(g_s->seed, 50, bitsOf(p->x), bitsOf(p->y)) % 3); }
	Real getLayerHeight(Real x, Real y, PathfindLayerEnum l) { return 10.0f * (Real)l + 0.5f; }
	/* batch A25 */
	PathfindLayerEnum getLayerForDestination(const Coord3D *p) { return (PathfindLayerEnum)(grpAnswer(g_s->seed, 51, bitsOf(p->x), bitsOf(p->y)) % 3); }
	Real getGroundHeight(Real x, Real y) const { return x * 0.01f + y * 0.02f; }
	void getExtent(Region3D *e) const { region(e, g_s->extent); }
	void getMaximumPathfindExtent(Region3D *e) const { region(e, g_s->pathExtent); }
	static void region(Region3D *e, const Real *r) { e->lo.x = r[0]; e->lo.y = r[1]; e->lo.z = 0.0f; e->hi.x = r[2]; e->hi.y = r[3]; e->hi.z = 0.0f; }
};
struct GlobalData { BodyDamageType m_movementPenaltyDamageState; AIDebugOptions m_debugAI; Real m_groupMoveClickToGatherFactor; };
class Xfer
{
public:
	std::vector<unsigned char> m_bytes;
	void put(const void *p, int n) { const unsigned char *c = (const unsigned char *)p; m_bytes.insert(m_bytes.end(), c, c + n); }
	void xferUser(void *p, int n) { put(p, n); }
	void xferUnsignedInt(UnsignedInt *p) { put(p, 4); }
	void xferObjectID(ObjectID *p) { put(p, 4); }
	void xferReal(Real *p) { put(p, 4); }
	void xferBool(Bool *p) { unsigned char b = *p ? 1 : 0; put(&b, 1); }
	void xferVersion(XferVersion *v, XferVersion cur) { put(v, 1); }
};
class XferCRC : public Xfer { public: UnsignedInt getCRC() { return 0; } };

class Object
{
public:
	Int m_i;	/* the scene's index */
	AIUpdateInterface m_ai;
	ContainModuleInterface m_contain;
	SpawnBehaviorInterface m_spawn;
	StealthUpdate m_stealth;
	SpecialPowerModuleInterface m_sp;
	SpecialPowerUpdateInterface m_spu;
	OverchargeBehaviorInterface m_oc;
	BehaviorModule m_bm[3];
	BehaviorModule *m_bmp[4];
	ThingTemplate m_tt;
	ProductionUpdateInterface m_pu;
	Drawable m_draw;
	BodyModuleInterface m_body;
	Coord3D m_pos;
	const GrpObj &s() const { return g_s->o[m_i]; }
	ObjectID getID() const { return (ObjectID)s().id; }
	AIUpdateInterface *getAIUpdateInterface() { return (s().flags & GRP_F_AI) ? &m_ai : NULL; }
	const AIUpdateInterface *getAIUpdateInterface() const { return (s().flags & GRP_F_AI) ? &m_ai : NULL; }
	AIUpdateInterface *getAI() { return getAIUpdateInterface(); }
	const AIUpdateInterface *getAI() const { return getAIUpdateInterface(); }
	Bool isAnyKindOf(const KindOfMaskType &m) const { return (s().kinds & m.m_bits) != 0; }
	Bool isKindOf(KindOfType k) const { return (s().kinds >> k) & 1; }
	void enterGroup(AIGroup *g);
	void leaveGroup();
	Player *getControllingPlayer();
	const Player *getControllingPlayer() const { return const_cast<Object *>(this)->getControllingPlayer(); }
	Bool isDisabledByType(DisabledType t) const { return (s().flags & GRP_F_HELD) != 0; }
	const Coord3D *getPosition() const { return &m_pos; }
	/* batch A25 */
	GeometryInfo m_geom;
	void getFormationOffset(Coord2D *o) const { o->x = s().formationOffset[0]; o->y = s().formationOffset[1]; }
	const GeometryInfo &getGeometryInfo() const { return m_geom; }
	PathfindLayerEnum getLayer() const { return 0; }
	FormationID getFormationID() const { return (FormationID)s().formation; }
	void setFormationID(FormationID id) { L({ GRP_LOG_FORMATION, (unsigned)s().id, 0u, (unsigned)id }); }
	void setFormationOffset(const Coord2D &o) { L({ GRP_LOG_FORMATION, (unsigned)s().id, 1u, bitsOf(o.x), bitsOf(o.y) }); }
	BodyModuleInterface *getBodyModule() { return &m_body; }
	ObjectStatusMaskType getStatusBits() const { ObjectStatusMaskType m; m.m_bits = (unsigned)s().status; return m; }
	StealthUpdate *getStealth() { return s().stealthDelay >= 0 ? &m_stealth : NULL; }
	ContainModuleInterface *getContain() { return (s().flags & GRP_F_CONTAIN) ? &m_contain : NULL; }
	SpawnBehaviorInterface *getSpawnBehaviorInterface() { return (s().flags & GRP_F_SPAWN) ? &m_spawn : NULL; }
	CanAttackResult getAbleToAttackSpecificObject(AbleToAttackType t, const Object *v, CommandSourceType src) const
	{
		return (CanAttackResult)(grpAnswer(g_s->seed, 60, (unsigned)s().id, (unsigned)v->getID() * 4u + (unsigned)t) % 4u);
	}
	CanAttackResult getAbleToUseWeaponAgainstTarget(AbleToAttackType t, const Object *v, const Coord3D *p, CommandSourceType src) const
	{
		return (CanAttackResult)(grpAnswer(g_s->seed, 61, (unsigned)s().id, bitsOf(p->x) ^ bitsOf(p->y)) % 4u);
	}
	Bool isAbleToAttack() const { return (s().flags & GRP_F_ABLE) != 0; }
	Bool isAirborneTarget() const { return (s().flags & GRP_F_AIRBORNE) != 0; }
	Bool isEffectivelyDead() const { return (s().flags & GRP_F_DEAD) != 0; }
	SpecialPowerModuleInterface *getSpecialPowerModule(const SpecialPowerTemplate *t) { m_sp.m_sp = t->m_id; return ans(42, s().id, t->m_id) ? &m_sp : NULL; }
	void friend_setUndetectedDefector(Bool b) { L({ GRP_LOG_DEFECTOR, (unsigned)s().id }); }
	void setSpecialModelConditionState(ModelConditionFlagType t, UnsignedInt frames) { L({ GRP_LOG_CHEER, (unsigned)s().id, frames }); }
	BehaviorModule **getBehaviorModules() { return m_bmp; }
	void doCommandButton(const CommandButton *b, CommandSourceType src) { L({ GRP_LOG_BUTTON, (unsigned)s().id, 0u, (unsigned)b->m_index, (unsigned)src }); }
	void doCommandButtonAtPosition(const CommandButton *b, const Coord3D *p, CommandSourceType src) { L({ GRP_LOG_BUTTON, (unsigned)s().id, 1u, (unsigned)b->m_index, bitsOf(p->x), bitsOf(p->y), bitsOf(p->z), (unsigned)src }); }
	void doCommandButtonUsingWaypoints(const CommandButton *b, const Waypoint *w, CommandSourceType src) { L({ GRP_LOG_BUTTON, (unsigned)s().id, 2u, (unsigned)b->m_index, (unsigned)w->m_index, (unsigned)src }); }
	void doCommandButtonAtObject(const CommandButton *b, Object *o, CommandSourceType src) { L({ GRP_LOG_BUTTON, (unsigned)s().id, 3u, (unsigned)b->m_index, o ? (unsigned)o->getID() : 0u, (unsigned)src }); }
	void setWeaponSetFlag(WeaponSetType w) { L({ GRP_LOG_WEAPONSET, (unsigned)s().id, 1u, (unsigned)w }); }
	void clearWeaponSetFlag(WeaponSetType w) { L({ GRP_LOG_WEAPONSET, (unsigned)s().id, 0u, (unsigned)w }); }
	Bool setWeaponLock(WeaponSlotType slot, WeaponLockType lock) { L({ GRP_LOG_LOCK, (unsigned)s().id, 1u, (unsigned)slot, (unsigned)lock }); return ans(43, s().id, slot * 8 + lock); }
	void releaseWeaponLock(WeaponLockType lock) { L({ GRP_LOG_LOCK, (unsigned)s().id, 0u, (unsigned)lock }); }
	const ThingTemplate *getTemplate() const { return &m_tt; }
	Bool hasUpgrade(const UpgradeTemplate *u) const { return ans(31, s().id, u->m_index); }
	Bool affectedByUpgrade(const UpgradeTemplate *u) const { return ans(32, s().id, u->m_index); }
	Bool canProduceUpgrade(const UpgradeTemplate *u) { return ans(33, s().id, u->m_index); }
	ProductionUpdateInterface *getProductionUpdateInterface() { return (s().flags & GRP_F_PRODUCTION) ? &m_pu : NULL; }
	AsciiString m_commandSetName;
	const AsciiString &getCommandSetString() const { return m_commandSetName; }
	Drawable *getDrawable() { return (s().flags & GRP_F_DRAWABLE) ? &m_draw : NULL; }
	SpecialPowerUpdateInterface *findSpecialPowerWithOverridableDestinationActive(SpecialPowerType t) { return ans(44, s().id, t) ? &m_spu : NULL; }
};

typedef std::list<Object *> ListObjectPtr;
typedef ListObjectPtr::iterator ListObjectPtrIt;

namespace {
Object g_objs[GRP_OBJECTS];
AI g_ai;
GameLogic g_logic;
TerrainLogic g_terrain;
GlobalData g_global;
SpecialPowerStore g_spStore;
ActionManager g_actions;
BuildAssistant g_build;
UpgradeCenter g_upgrades;
ControlBar g_controlBar;
Player g_players[4];	/* the scene's players: an object's s().player (0..3) */
Object *findObj(ObjectID id) { for (int i = 0; i < GRP_OBJECTS; ++i) if (g_s->o[i].id == id && id != 0) return &g_objs[i]; return NULL; }
unsigned idOf(const Object *o) { return o ? (unsigned)o->getID() : 0u; }
}
AI *TheAI = &g_ai;
GameLogic *TheGameLogic = &g_logic;
TerrainLogic *TheTerrainLogic = &g_terrain;
GlobalData *TheGlobalData = &g_global;
SpecialPowerStore *TheSpecialPowerStore = &g_spStore;
ActionManager *TheActionManager = &g_actions;
BuildAssistant *TheBuildAssistant = &g_build;
UpgradeCenter *TheUpgradeCenter = &g_upgrades;
ControlBar *TheControlBar = &g_controlBar;
Int GameLogicRandomValue(Int lo, Int hi)
{
	g_rand = g_rand * 1664525u + 1013904223u;
	Int r = lo + (Int)((g_rand >> 8) % (unsigned)(hi - lo + 1));
	L({ GRP_LOG_RANDOM, 1u, (unsigned)lo, (unsigned)hi, (unsigned)r });
	return r;
}

/* SimpleObjectIterator: the (object, key) list, sorted near to far: as SimpleObjectIterator.cpp, each inserted at the front and the
** merge sort stable (ties keep the reversed order) */
class SimpleObjectIterator
{
public:
	std::vector<std::pair<Object *, Real> > m_v;
	size_t m_i;
	SimpleObjectIterator() : m_i(0) {}
	void insert(Object *o, Real key) { m_v.insert(m_v.begin(), std::make_pair(o, key)); }
	void insert(Object *o) { insert(o, 0.0f); }
	void makeEmpty() { m_v.clear(); m_i = 0; }
	void sort(IterOrderType t)
	{
		/* (upstream's merge takes the later one first only when the compare proc is > 0: a stable sort on proc < 0) */
		if (t == ITER_SORTED_NEAR_TO_FAR)
			std::stable_sort(m_v.begin(), m_v.end(), [](const std::pair<Object *, Real> &a, const std::pair<Object *, Real> &b) { return a.second - b.second < 0.0f; });
		else
			std::stable_sort(m_v.begin(), m_v.end(), [](const std::pair<Object *, Real> &a, const std::pair<Object *, Real> &b) { return b.second - a.second < 0.0f; });
		m_i = 0;
	}
	Object *first() { m_i = 0; return next(); }
	Object *next() { return m_i < m_v.size() ? m_v[m_i++].first : NULL; }
	void deleteInstance() { delete this; }
};
class MemoryPoolObjectHolder { public: SimpleObjectIterator *m_p; MemoryPoolObjectHolder() : m_p(NULL) {} void hold(SimpleObjectIterator *p) { m_p = p; } ~MemoryPoolObjectHolder() { if (m_p) m_p->deleteInstance(); } };
#define newInstance(T) new T

class AIGroup
{
public:
	AIGroup( void );
	~AIGroup();
	UnsignedInt getID( void );
	const VecObjectID& getAllIDs( void ) const;
	Real getSpeed( void );
	Bool isMember( Object *obj );
	void add( Object *obj );
	Bool remove( Object *obj );
	Bool containsAnyObjectsNotOwnedByPlayer( const Player *ownerPlayer );
	Bool removeAnyObjectsNotOwnedByPlayer( const Player *ownerPlayer );
	Bool getCenter( Coord3D *center );
	Bool getMinMaxAndCenter( Coord2D *min, Coord2D *max, Coord3D *center );
	void recompute( void );
	Int getCount( void );
	Bool isEmpty( void );
	void groupIdle(CommandSourceType cmdSource);
	/* batch A25 */
	void computeIndividualDestination( Coord3D *dest, const Coord3D *groupDest, Object *obj, const Coord3D *center, Bool isFormation );
	Bool friend_computeGroundPath( const Coord3D *pos, CommandSourceType cmdSource );
	Bool friend_moveInfantryToPos( const Coord3D *pos, CommandSourceType cmdSource );
	void friend_moveFormationToPos( const Coord3D *pos, CommandSourceType cmdSource );
	Bool friend_moveVehicleToPos( const Coord3D *pos, CommandSourceType cmdSource );
	void groupMoveToPosition( const Coord3D *pos, Bool addWaypoint, CommandSourceType cmdSource );
	void groupScatter( CommandSourceType cmdSource );
	void groupTightenToPosition( const Coord3D *pos, Bool addWaypoint, CommandSourceType cmdSource );
	void groupFollowWaypointPath( const Waypoint *way, CommandSourceType cmdSource );
	void groupFollowWaypointPathExact( const Waypoint *way, CommandSourceType cmdSource );
	void groupMoveToAndEvacuate( const Coord3D *pos, CommandSourceType cmdSource );
	void groupMoveToAndEvacuateAndExit( const Coord3D *pos, CommandSourceType cmdSource );
	void groupFollowWaypointPathAsTeam( const Waypoint *way, CommandSourceType cmdSource );
	void groupFollowWaypointPathAsTeamExact( const Waypoint *way, CommandSourceType cmdSource );
	void groupFollowPath( const std::vector<Coord3D>* path, Object *ignoreObject, CommandSourceType cmdSource );
	void groupAttackObjectPrivate( Bool forced, Object *victim, Int maxShotsToFire, CommandSourceType cmdSource );
	void groupAttackTeam( const Team *team, Int maxShotsToFire, CommandSourceType cmdSource );
	void groupAttackPosition( const Coord3D *pos, Int maxShotsToFire, CommandSourceType cmdSource );
	void groupAttackMoveToPosition( const Coord3D *pos, Int maxShotsToFire, CommandSourceType cmdSource );
	void groupHunt( CommandSourceType cmdSource );
	void groupRepair( Object *obj, CommandSourceType cmdSource );
	void groupResumeConstruction( Object *obj, CommandSourceType cmdSource );
	void groupGetHealed( Object *healDepot, CommandSourceType cmdSource );
	void groupGetRepaired( Object *repairDepot, CommandSourceType cmdSource );
	void groupEnter( Object *obj, CommandSourceType cmdSource );
	void groupDock( Object *obj, CommandSourceType cmdSource );
	void groupExit( Object *objectToExit, CommandSourceType cmdSource );
	void groupEvacuate( CommandSourceType cmdSource );
	void groupExecuteRailedTransport( CommandSourceType cmdSource );
	void groupGoProne( const DamageInfo *damageInfo, CommandSourceType cmdSource );
	void groupGuardPosition( const Coord3D *pos, GuardMode guardMode, CommandSourceType cmdSource );
	void groupGuardObject( Object *objToGuard, GuardMode guardMode, CommandSourceType cmdSource );
	void groupGuardArea( const PolygonTrigger *areaToGuard, GuardMode guardMode, CommandSourceType cmdSource );
	void groupAttackArea( const PolygonTrigger *areaToGuard, CommandSourceType cmdSource );
	void groupHackInternet( CommandSourceType cmdSource );
	void groupCreateFormation( CommandSourceType cmdSource );
	void groupDoSpecialPower( UnsignedInt specialPowerID, UnsignedInt commandOptions );
	void groupDoSpecialPowerAtLocation( UnsignedInt specialPowerID, const Coord3D *location, Real angle, const Object *objectInWay, UnsignedInt commandOptions );
	void groupDoSpecialPowerAtObject( UnsignedInt specialPowerID, Object *target, UnsignedInt commandOptions );
	void groupCheer( CommandSourceType cmdSource );
	void groupSell( CommandSourceType cmdSource );
	void groupToggleOvercharge( CommandSourceType cmdSource );
	void groupCombatDrop( Object *target, const Coord3D &pos, CommandSourceType cmdSource );
	void groupDoCommandButton( const CommandButton *commandButton, CommandSourceType cmdSource );
	void groupDoCommandButtonAtPosition( const CommandButton *commandButton, const Coord3D *pos, CommandSourceType cmdSource );
	void groupDoCommandButtonUsingWaypoints( const CommandButton *commandButton, const Waypoint *way, CommandSourceType cmdSource );
	void groupDoCommandButtonAtObject( const CommandButton *commandButton, Object *obj, CommandSourceType cmdSource );
	void setAttitude( AttitudeType tude );
	AttitudeType getAttitude( void ) const;
	void setMineClearingDetail( Bool set );
	Bool setWeaponLockForGroup( WeaponSlotType weaponSlot, WeaponLockType lockType );
	void releaseWeaponLockForGroup(WeaponLockType lockType);
	void setWeaponSetFlag( WeaponSetType wst );
	void queueUpgrade( const UpgradeTemplate *upgrade );
	Bool isIdle( void ) const;
	Bool isBusy( void ) const;
	Bool isGroupAiDead( void ) const;
	Object *getSpecialPowerSourceObject( UnsignedInt specialPowerID );
	Object *getCommandButtonSourceObject( GUICommandType type );
	void groupSetEmoticon( const AsciiString &name, Int duration );
	void groupOverrideSpecialPowerDestination( SpecialPowerType spType, const Coord3D *loc, CommandSourceType cmdSource );
	void crc( Xfer *xfer );
	void xfer( Xfer *xfer );
	void loadPostProcess( void );

	std::list<Object *>	m_memberList;
	UnsignedInt m_memberListSize;
	Real m_speed;
	Bool m_dirty;
	UnsignedInt m_id;
	Path *m_groundPath;
	mutable VecObjectID m_lastRequestedIDList;
};

/* ---- the stand-ins' members that log */
void AIUpdateInterface::cmd(unsigned c, std::initializer_list<unsigned> w, CommandSourceType src)
{
	std::vector<unsigned> v = { GRP_LOG_AI, idOf(m_obj), c };
	v.insert(v.end(), w);
	v.push_back((unsigned)src);
	g_log->insert(g_log->end(), v.begin(), v.end());
}
void AIUpdateInterface::aiIdle(CommandSourceType s) { cmd(GC_IDLE, {}, s); }
void AIUpdateInterface::aiAttackObject(Object *v, Int n, CommandSourceType s) { cmd(GC_ATTACK_OBJECT, { idOf(v), (unsigned)n }, s); }
void AIUpdateInterface::aiForceAttackObject(Object *v, Int n, CommandSourceType s) { cmd(GC_FORCE_ATTACK_OBJECT, { idOf(v), (unsigned)n }, s); }
void AIUpdateInterface::aiAttackTeam(const Team *t, Int n, CommandSourceType s) { cmd(GC_ATTACK_TEAM, { (unsigned)t->m_index, (unsigned)n }, s); }
void AIUpdateInterface::aiAttackPosition(const Coord3D *p, Int n, CommandSourceType s) { cmd(GC_ATTACK_POSITION, { bitsOf(p->x), bitsOf(p->y), bitsOf(p->z), (unsigned)n }, s); }
void AIUpdateInterface::aiAttackMoveToPosition(const Coord3D *p, Int n, CommandSourceType s) { cmd(GC_ATTACK_MOVE, { bitsOf(p->x), bitsOf(p->y), bitsOf(p->z), (unsigned)n }, s); }
void AIUpdateInterface::aiMoveToPosition(const Coord3D *p, CommandSourceType s) { cmd(GC_MOVE, { bitsOf(p->x), bitsOf(p->y), bitsOf(p->z) }, s); }
void AIUpdateInterface::aiHunt(CommandSourceType s) { cmd(GC_HUNT, {}, s); }
void AIUpdateInterface::aiRepair(Object *o, CommandSourceType s) { cmd(GC_REPAIR, { idOf(o) }, s); }
void AIUpdateInterface::aiResumeConstruction(Object *o, CommandSourceType s) { cmd(GC_RESUME, { idOf(o) }, s); }
void AIUpdateInterface::aiGetHealed(Object *o, CommandSourceType s) { cmd(GC_HEALED, { idOf(o) }, s); }
void AIUpdateInterface::aiGetRepaired(Object *o, CommandSourceType s) { cmd(GC_REPAIRED, { idOf(o) }, s); }
void AIUpdateInterface::aiEnter(Object *o, CommandSourceType s) { cmd(GC_ENTER, { idOf(o) }, s); }
void AIUpdateInterface::aiDock(Object *o, CommandSourceType s) { cmd(GC_DOCK, { idOf(o) }, s); }
void AIUpdateInterface::aiExit(Object *o, CommandSourceType s) { cmd(GC_EXIT, { idOf(o) }, s); }
void AIUpdateInterface::aiMoveToAndEvacuate(const Coord3D *p, CommandSourceType s) { cmd(GC_MOVE_EVACUATE, { bitsOf(p->x), bitsOf(p->y), bitsOf(p->z) }, s); }
void AIUpdateInterface::aiEvacuate(Bool e, CommandSourceType s) { cmd(GC_EVACUATE, { e ? 1u : 0u }, s); }
void AIUpdateInterface::aiExecuteRailedTransport(CommandSourceType s) { cmd(GC_RAILED, {}, s); }
void AIUpdateInterface::aiGoProne(const DamageInfo *d, CommandSourceType s) { cmd(GC_PRONE, { (unsigned)d->m_index }, s); }
void AIUpdateInterface::aiGuardPosition(const Coord3D *p, GuardMode m, CommandSourceType s) { cmd(GC_GUARD_POSITION, { bitsOf(p->x), bitsOf(p->y), bitsOf(p->z), (unsigned)m }, s); }
void AIUpdateInterface::aiGuardObject(Object *o, GuardMode m, CommandSourceType s) { cmd(GC_GUARD_OBJECT, { idOf(o), (unsigned)m }, s); }
void AIUpdateInterface::aiGuardArea(const PolygonTrigger *a, GuardMode m, CommandSourceType s) { cmd(GC_GUARD_AREA, { (unsigned)a->m_index, (unsigned)m }, s); }
void AIUpdateInterface::aiAttackArea(const PolygonTrigger *a, CommandSourceType s) { cmd(GC_ATTACK_AREA, { (unsigned)a->m_index }, s); }
void AIUpdateInterface::aiHackInternet(CommandSourceType s) { cmd(GC_HACK, {}, s); }
void AIUpdateInterface::aiCombatDrop(Object *t, const Coord3D &p, CommandSourceType s) { cmd(GC_COMBAT_DROP, { idOf(t), bitsOf(p.x), bitsOf(p.y), bitsOf(p.z) }, s); }
void AIUpdateInterface::setAttitude(AttitudeType t) { L({ GRP_LOG_ATTITUDE, idOf(m_obj), (unsigned)t }); }
Bool AIUpdateInterface::canAutoAcquire() const { return (m_obj->s().flags & GRP_F_ACQUIRE) != 0; }
Bool AIUpdateInterface::canAutoAcquireWhileStealthed() const { return (m_obj->s().flags & GRP_F_ACQUIRE_STEALTHED) != 0; }
void AIUpdateInterface::setNextMoodCheckTime(UnsignedInt t) { L({ GRP_LOG_MOOD, idOf(m_obj), t }); }
Bool AIUpdateInterface::isIdle() const { return (m_obj->s().flags & GRP_F_IDLE) != 0; }
Bool AIUpdateInterface::isBusy() const { return (m_obj->s().flags & GRP_F_BUSY) != 0; }
Real AIUpdateInterface::getCurLocomotorSpeed() const { return m_obj->s().locoSpeed; }
UnsignedInt StealthUpdate::getStealthDelay() const { return (UnsignedInt)m_obj->s().stealthDelay; }
Bool ContainModuleInterface::isPassengerAllowedToFire() const { return (m_obj->s().flags & GRP_F_FIRE) != 0; }
void ContainModuleInterface::orderAllPassengersToExit(CommandSourceType s, Bool instantly) { L({ GRP_LOG_EXIT_ALL, idOf(m_obj), (unsigned)s }); }
void SpawnBehaviorInterface::orderSlavesToGoIdle(CommandSourceType s) { L({ GRP_LOG_SLAVES, idOf(m_obj), 0u, (unsigned)s }); }
Bool SpawnBehaviorInterface::doSlavesHaveFreedom() const { return (m_obj->s().flags & GRP_F_FREEDOM) != 0; }
void SpawnBehaviorInterface::orderSlavesToAttackTarget(Object *v, Int n, CommandSourceType s) { L({ GRP_LOG_SLAVES, idOf(m_obj), 1u, idOf(v), (unsigned)n, (unsigned)s }); }
void SpawnBehaviorInterface::orderSlavesToAttackPosition(const Coord3D *p, Int n, CommandSourceType s) { L({ GRP_LOG_SLAVES, idOf(m_obj), 2u, bitsOf(p->x), bitsOf(p->y), bitsOf(p->z), (unsigned)n, (unsigned)s }); }
void SpecialPowerModuleInterface::doSpecialPower(UnsignedInt opts) { L({ GRP_LOG_SP, idOf(m_obj), 0u, m_sp, opts }); }
void SpecialPowerModuleInterface::doSpecialPowerAtLocation(const Coord3D *p, Real a, UnsignedInt opts) { L({ GRP_LOG_SP, idOf(m_obj), 1u, m_sp, bitsOf(p->x), bitsOf(p->y), bitsOf(p->z), bitsOf(a), opts }); }
void SpecialPowerModuleInterface::doSpecialPowerAtObject(Object *t, UnsignedInt opts) { L({ GRP_LOG_SP, idOf(m_obj), 2u, m_sp, idOf(t), opts }); }
void SpecialPowerUpdateInterface::setSpecialPowerOverridableDestination(const Coord3D *p) { L({ GRP_LOG_OVERRIDE, idOf(m_obj), bitsOf(p->x), bitsOf(p->y), bitsOf(p->z) }); }
Bool ActionManager::canDoSpecialPower(const Object *o, const SpecialPowerTemplate *t, CommandSourceType s, UnsignedInt opts) { return ans(45, o->getID(), t->m_id * 16 + opts); }
Bool ActionManager::canDoSpecialPowerAtLocation(const Object *o, const Coord3D *loc, CommandSourceType s, const SpecialPowerTemplate *t, const Object *w, UnsignedInt opts) { return ans(46, o->getID(), t->m_id * 16 + opts + 256 * idOf(w)); }
Bool ActionManager::canDoSpecialPowerAtObject(const Object *o, const Object *target, CommandSourceType s, const SpecialPowerTemplate *t, UnsignedInt opts) { return ans(47, o->getID(), t->m_id * 16 + opts + 256 * idOf(target)); }
void OverchargeBehaviorInterface::toggle() { L({ GRP_LOG_OVERCHARGE, idOf(m_obj) }); }
const WeaponTemplateSet *ThingTemplate::findWeaponTemplateSet(const WeaponSetFlags &f) const { static WeaponTemplateSet w; return ans(48, m_obj->getID(), f.m_bits) ? &w : NULL; }
CanMakeType ProductionUpdateInterface::canQueueUpgrade(const UpgradeTemplate *u) const { return ans(34, m_obj->getID(), u->m_index) ? CANMAKE_OK : CANMAKE_QUEUE_FULL; }
void ProductionUpdateInterface::queueUpgrade(const UpgradeTemplate *u) { L({ GRP_LOG_UPGRADE, idOf(m_obj), (unsigned)u->m_index }); }
const CommandButton *CommandSet::getCommandButton(Int i) const { return (i < 8 && ((m_obj->s().commandSet >> i) & 1)) ? &m_b[i] : NULL; }
const CommandSet *ControlBar::findCommandSet(const AsciiString &name)
{
	const Object *o = name.m_obj;
	if (o->s().commandSet < 0) return NULL;
	CommandSet &cs = m_sets[o->m_i];
	cs.m_obj = const_cast<Object *>(o);
	for (int i = 0; i < MAX_COMMANDS_PER_SET; ++i) { cs.m_b[i].m_index = i; cs.m_b[i].m_type = i; }
	return &cs;
}
void Drawable::setEmoticon(const AsciiString &name, Int duration) { L({ GRP_LOG_EMOTICON, idOf(m_obj), (unsigned)strlen(name.m_s), (unsigned)duration }); }
BodyDamageType BodyModuleInterface::getDamageState() const { return m_obj->s().damage; }
void BuildAssistant::sellObject(Object *o) { L({ GRP_LOG_SELL, idOf(o) }); }
Bool UpgradeCenter::canAffordUpgrade(Player *p, const UpgradeTemplate *u, Bool displayReason) const { return ans(35, p ? (unsigned)p->m_index + 1u : 0u, u->m_index); }
/* batch A25 */
namespace { Int g_tmp[GRP_OBJECTS]; }
Bool AIUpdateInterface::isDoingGroundMovement() const { return (m_obj->s().flags & GRP_F_GROUND) != 0; }
Locomotor *AIUpdateInterface::getCurLocomotor() { m_loco.m_obj = m_obj; return m_obj->s().locoPriority >= 0 ? &m_loco : NULL; }
LocomotorPriority Locomotor::getMovePriority() const { return (LocomotorPriority)m_obj->s().locoPriority; }
Real GeometryInfo::getBoundingCircleRadius() const { return m_obj->s().boundingRadius; }
Real GeometryInfo::getMajorRadius() const { return m_obj->s().majorRadius; }
Int AIUpdateInterface::getTmpValue() { return g_tmp[m_obj->m_i]; }
void AIUpdateInterface::setTmpValue(Int v) { g_tmp[m_obj->m_i] = v; }
void AIUpdateInterface::aiFollowPath(const std::vector<Coord3D> *path, Object *ignoreObject, CommandSourceType s)
{
	std::vector<unsigned> w = { GRP_LOG_AI, idOf(m_obj), (unsigned)GC_FOLLOW_PATH, (unsigned)path->size() };
	for (const Coord3D &p : *path) { w.push_back(bitsOf(p.x)); w.push_back(bitsOf(p.y)); w.push_back(bitsOf(p.z)); }
	w.push_back(idOf(ignoreObject));
	w.push_back((unsigned)s);
	g_log->insert(g_log->end(), w.begin(), w.end());
}
void AIUpdateInterface::aiFollowPathAppend(const Coord3D *p, CommandSourceType s) { cmd(GC_FOLLOW_PATH_APPEND, { bitsOf(p->x), bitsOf(p->y), bitsOf(p->z) }, s); }
void AIUpdateInterface::aiTightenToPosition(const Coord3D *p, CommandSourceType s) { cmd(GC_TIGHTEN, { bitsOf(p->x), bitsOf(p->y), bitsOf(p->z) }, s); }
void AIUpdateInterface::aiFollowWaypointPath(const Waypoint *w, CommandSourceType s) { cmd(GC_WAYPOINT_PATH, { (unsigned)w->m_index, 0u, 0u }, s); }
void AIUpdateInterface::aiFollowWaypointPathExact(const Waypoint *w, CommandSourceType s) { cmd(GC_WAYPOINT_PATH, { (unsigned)w->m_index, 1u, 0u }, s); }
void AIUpdateInterface::aiFollowWaypointPathAsTeam(const Waypoint *w, CommandSourceType s) { cmd(GC_WAYPOINT_PATH, { (unsigned)w->m_index, 0u, 1u }, s); }
void AIUpdateInterface::aiFollowWaypointPathExactAsTeam(const Waypoint *w, CommandSourceType s) { cmd(GC_WAYPOINT_PATH, { (unsigned)w->m_index, 1u, 1u }, s); }
void AIUpdateInterface::aiMoveToAndEvacuateAndExit(const Coord3D *p, CommandSourceType s) { cmd(GC_EVACUATE_AND_EXIT, { bitsOf(p->x), bitsOf(p->y), bitsOf(p->z) }, s); }
void Pathfinder::removeGoal(Object *obj) { L({ GRP_LOG_REMOVE_GOAL, idOf(obj) }); }
Bool Pathfinder::adjustDestination(Object *obj, const LocomotorSet &ls, Coord3D *dest, const Coord3D *groupDest)
{
	std::vector<unsigned> w = { GRP_LOG_ADJUST, idOf(obj), bitsOf(dest->x), bitsOf(dest->y), bitsOf(dest->z), groupDest ? 1u : 0u };
	if (groupDest) { w.push_back(bitsOf(groupDest->x)); w.push_back(bitsOf(groupDest->y)); w.push_back(bitsOf(groupDest->z)); }
	g_log->insert(g_log->end(), w.begin(), w.end());
	if (ans(57, idOf(obj), bitsOf(dest->x))) { dest->x += 1.0f; dest->y -= 0.5f; }
	return TRUE;
}
void Pathfinder::updateGoal(Object *obj, const Coord3D *p, PathfindLayerEnum layer) { L({ GRP_LOG_UPDATE_GOAL, idOf(obj), bitsOf(p->x), bitsOf(p->y), bitsOf(p->z), (unsigned)layer }); }
void Pathfinder::updatePos(Object *obj, const Coord3D *p) { L({ GRP_LOG_UPDATE_POS, idOf(obj), bitsOf(p->x), bitsOf(p->y), bitsOf(p->z) }); }
Bool Pathfinder::isLinePassable(const Object *obj, LocomotorSurfaceTypeMask surfaces, PathfindLayerEnum layer, const Coord3D &from, const Coord3D &to, Bool blocked, Bool allowPinched)
{
	L({ GRP_LOG_LINE, idOf(obj), bitsOf(from.x), bitsOf(from.y), bitsOf(to.x), bitsOf(to.y) });
	return ans(53, idOf(obj), bitsOf(to.x));
}
/* a path of 2 to 8 nodes from `from` toward `to`, each off the line by the hash's jitter; none for an eighth of the answers */
Path *Pathfinder::findGroundPath(const Coord3D *from, const Coord3D *to, Int pathRadius, Bool crusher)
{
	L({ GRP_LOG_GROUND_PATH, bitsOf(from->x), bitsOf(from->y), bitsOf(from->z), bitsOf(to->x), bitsOf(to->y), bitsOf(to->z), (unsigned)pathRadius });
	unsigned h = grpAnswer(g_s->seed, 54, bitsOf(from->x) ^ bitsOf(to->y), bitsOf(to->x));
	if ((h & 7) == 0) return NULL;
	int n = 2 + (int)((h >> 3) % 7u);
	Path *p = new Path;
	p->m_nodes.resize(n);
	for (int k = 0; k < n; ++k) {
		Real t = (Real)k / (Real)(n - 1);
		Real jx = (Real)(grpAnswer(g_s->seed, 55, (unsigned)k, h) % 401u) * 0.1f - 20.0f;
		Real jy = (Real)(grpAnswer(g_s->seed, 56, (unsigned)k, h) % 401u) * 0.1f - 20.0f;
		p->m_nodes[k].m_pos.x = from->x + (to->x - from->x) * t + jx;
		p->m_nodes[k].m_pos.y = from->y + (to->y - from->y) * t + jy;
		p->m_nodes[k].m_pos.z = from->z;
	}
	for (int k = 0; k < n; ++k) p->m_nodes[k].m_next = k + 1 < n ? &p->m_nodes[k + 1] : NULL;
	return p;
}
void AI::destroyGroup(AIGroup *g) { L({ GRP_LOG_DESTROY, g->m_id }); }
Player *Object::getControllingPlayer() { return &g_players[s().player & 3]; }
void Object::enterGroup(AIGroup *g) { L({ GRP_LOG_ENTER, idOf(this), g->m_id }); }
void Object::leaveGroup() { L({ GRP_LOG_LEAVE, idOf(this) }); }

#include "ai_group.inc"

}	/* namespace grp_a24 */
using namespace grp_a24;

/* a member not held without an AI: the formation move commands it through its null AI (undefined), so ops 60 and 72 are not run */
static bool memberWithoutAi(AIGroup &g)
{
	for (Object *m : g.m_memberList) if (!m->isDisabledByType(DISABLED_HELD) && !m->getAI()) return true;
	return false;
}

/* the group's ground path: its node count and their positions (0: none) */
static void pathWords(AIGroup &g, std::vector<unsigned> &r)
{
	if (!g.m_groundPath) { r.push_back(0u); return; }
	r.push_back((unsigned)g.m_groundPath->m_nodes.size());
	for (PathNode &n : g.m_groundPath->m_nodes) { r.push_back(bitsOf(n.m_pos.x)); r.push_back(bitsOf(n.m_pos.y)); r.push_back(bitsOf(n.m_pos.z)); }
}

extern "C" int ref_ai_group_run(const GrpScene *scene, const GrpOp *ops, int nOps, unsigned *out, int outCap, unsigned *log, int logCap, int *logLen)
{
	std::vector<unsigned> lg, r;
	g_s = scene;
	g_log = &lg;
	g_rand = scene->seed;
	g_ai.m_nextGroup = (UnsignedInt)scene->nextGroupId;
	g_ai.m_nextFormation = (UnsignedInt)scene->nextFormationId;
	g_global.m_movementPenaltyDamageState = scene->penalty;
	g_global.m_debugAI = scene->debugTerrain ? AI_DEBUG_TERRAIN : AI_DEBUG_NONE;
	g_global.m_groupMoveClickToGatherFactor = scene->gatherFactor;
	g_ai.m_data.m_distanceRequiresGroup = scene->distanceRequiresGroup;
	g_ai.m_data.m_minDistanceForGroup = scene->minDistanceForGroup;
	g_ai.m_data.m_minInfantryForGroup = scene->minInfantryForGroup;
	g_ai.m_data.m_minVehiclesForGroup = scene->minVehiclesForGroup;
	for (int k = 0; k < 4; ++k) g_players[k].m_index = k;
	for (int i = 0; i < GRP_OBJECTS; ++i) {
		Object &o = g_objs[i];
		const GrpObj &s = scene->o[i];
		o.m_i = i;
		o.m_pos.x = s.pos[0]; o.m_pos.y = s.pos[1]; o.m_pos.z = s.pos[2];
		o.m_ai.m_obj = &o; o.m_contain.m_obj = &o; o.m_spawn.m_obj = &o; o.m_stealth.m_obj = &o; o.m_sp.m_obj = &o; o.m_spu.m_obj = &o;
		o.m_oc.m_obj = &o; o.m_tt.m_obj = &o; o.m_pu.m_obj = &o; o.m_draw.m_obj = &o; o.m_body.m_obj = &o;
		o.m_geom.m_obj = &o; g_tmp[i] = s.tmp;
		o.m_commandSetName.m_s = ""; o.m_commandSetName.m_obj = &o;
		o.m_contain.m_items.clear();
		for (int k = 0; k < 3; ++k) if (s.contained[k]) { Object *c = findObj(s.contained[k]); if (c) o.m_contain.m_items.push_back(c); }
		/* the behaviour modules: an overcharge interface on the first and, for odd ids, the third */
		o.m_bm[0].m_oc = &o.m_oc; o.m_bm[1].m_oc = NULL; o.m_bm[2].m_oc = (s.id & 1) ? &o.m_oc : NULL;
		o.m_bmp[0] = &o.m_bm[0]; o.m_bmp[1] = &o.m_bm[1]; o.m_bmp[2] = &o.m_bm[2]; o.m_bmp[3] = NULL;
	}
	{
		AIGroup g;
		for (int k = 0; k < nOps; ++k) {
			const GrpOp &op = ops[k];
			Coord3D p; p.x = op.x; p.y = op.y; p.z = op.z;
			CommandSourceType src = (CommandSourceType)(op.c & 3);
			Object *oa = findObj((ObjectID)op.a), *ob = findObj((ObjectID)op.b);
			Team team; team.m_index = op.a;
			PolygonTrigger poly; poly.m_index = op.a;
			Waypoint way; way.m_index = op.b;
			DamageInfo dmg; dmg.m_index = op.a;
			CommandButton button; button.m_index = op.a; button.m_type = op.a;
			UpgradeTemplate up; up.m_index = op.a;
			AsciiString name; name.m_s = (op.a & 1) ? "Emoticon" : "E";
			switch (op.kind) {
			case 0: if (oa && !g.isMember(oa)) g.add(oa); break;	/* (only non-members: the callers check) */
			case 1: if (oa) r.push_back(g.remove(oa) ? 1u : 0u); else r.push_back(2u); break;
			case 2: r.push_back(g.isMember(oa) ? 1u : 0u); break;
			case 3: r.push_back(bitsOf(g.getSpeed())); break;
			case 4: { Coord3D c; Bool b = g.getCenter(&c); r.push_back(b ? 1u : 0u); r.push_back(bitsOf(c.x)); r.push_back(bitsOf(c.y)); r.push_back(bitsOf(c.z)); break; }
			case 5: { Coord2D mn, mx; Coord3D c; Bool b = g.getMinMaxAndCenter(&mn, &mx, &c); r.push_back(b ? 1u : 0u); r.push_back(bitsOf(mn.x)); r.push_back(bitsOf(mn.y)); r.push_back(bitsOf(mx.x)); r.push_back(bitsOf(mx.y)); r.push_back(bitsOf(c.x)); r.push_back(bitsOf(c.y)); r.push_back(bitsOf(c.z)); break; }
			case 6: g.recompute(); break;
			case 7: r.push_back(g.containsAnyObjectsNotOwnedByPlayer(op.a >= 0 ? &g_players[op.a & 3] : NULL) ? 1u : 0u); break;
			case 8: r.push_back(g.removeAnyObjectsNotOwnedByPlayer(op.a >= 0 ? &g_players[op.a & 3] : NULL) ? 1u : 0u); break;
			case 9: g.groupIdle(src); break;
			case 10: g.groupAttackObjectPrivate(op.b != 0, oa, op.c >> 2, src); break;
			case 11: g.groupAttackTeam(op.a >= 0 ? &team : NULL, op.b, src); break;
			case 12: g.groupAttackPosition(op.a ? &p : NULL, op.b, src); break;
			case 13: g.groupAttackMoveToPosition(&p, op.b, src); break;
			case 14: g.groupHunt(src); break;
			case 15: g.groupRepair(oa, src); break;
			case 16: g.groupResumeConstruction(oa, src); break;
			case 17: g.groupGetHealed(oa, src); break;
			case 18: g.groupGetRepaired(oa, src); break;
			case 19: g.groupEnter(oa, src); break;
			case 20: g.groupDock(oa, src); break;
			case 21: g.groupExit(oa, src); break;
			case 22: g.groupEvacuate(src); break;
			case 23: g.groupExecuteRailedTransport(src); break;
			case 24: g.groupGoProne(&dmg, src); break;
			case 25: g.groupGuardPosition(op.b ? &p : NULL, (GuardMode)(op.a & 3), src); break;
			case 26: g.groupGuardObject(oa, (GuardMode)(op.b & 3), src); break;
			case 27: g.groupGuardArea(op.b ? &poly : NULL, (GuardMode)(op.b & 3), src); break;
			case 28: g.groupAttackArea(op.b ? &poly : NULL, src); break;
			case 29: g.groupHackInternet(src); break;
			case 30: g.groupCreateFormation(src); break;
			case 31: g.groupDoSpecialPower((UnsignedInt)op.a, (UnsignedInt)op.b); break;
			case 32: g.groupDoSpecialPowerAtLocation((UnsignedInt)op.a, &p, op.w, ob, (UnsignedInt)(op.c >> 2)); break;
			case 33: g.groupDoSpecialPowerAtObject((UnsignedInt)op.a, ob, (UnsignedInt)(op.c >> 2)); break;
			case 34: g.groupCheer(src); break;
			case 35: g.groupSell(src); break;
			case 36: g.groupToggleOvercharge(src); break;
			case 37: g.groupCombatDrop(ob, p, src); break;
			case 38: g.groupDoCommandButton(&button, src); break;
			case 39: g.groupDoCommandButtonAtPosition(&button, &p, src); break;
			case 40: g.groupDoCommandButtonUsingWaypoints(&button, &way, src); break;
			case 41: g.groupDoCommandButtonAtObject(&button, ob, src); break;
			case 42: g.setAttitude((AttitudeType)(op.a % 5 - 2)); break;
			case 43: r.push_back((unsigned)g.getAttitude()); break;
			case 44: g.setMineClearingDetail(op.a != 0); break;
			case 45: r.push_back(g.setWeaponLockForGroup((WeaponSlotType)(op.a & 3), (WeaponLockType)(op.b & 3)) ? 1u : 0u); break;
			case 46: g.releaseWeaponLockForGroup((WeaponLockType)(op.a & 3)); break;
			case 47: g.setWeaponSetFlag((WeaponSetType)(op.a & 7)); break;
			case 48: g.queueUpgrade(op.b ? &up : NULL); break;
			case 49: r.push_back(g.isIdle() ? 1u : 0u); break;
			case 50: r.push_back(g.isBusy() ? 1u : 0u); break;
			case 51: r.push_back(g.isGroupAiDead() ? 1u : 0u); break;
			case 52: r.push_back(idOf(g.getSpecialPowerSourceObject((UnsignedInt)op.a))); break;
			case 53: r.push_back(idOf(g.getCommandButtonSourceObject((GUICommandType)(op.a & 7)))); break;
			case 54: g.groupSetEmoticon(name, op.b); break;
			case 55: g.groupOverrideSpecialPowerDestination((SpecialPowerType)op.a, &p, src); break;
			case 56: { Xfer x; g.crc(&x); r.push_back((unsigned)x.m_bytes.size()); for (unsigned char c : x.m_bytes) r.push_back(c); break; }
			case 57: { Xfer x; g.xfer(&x); r.push_back((unsigned)x.m_bytes.size()); for (unsigned char c : x.m_bytes) r.push_back(c); break; }
			case 58: r.push_back((unsigned)g.getCount()); r.push_back(g.isEmpty() ? 1u : 0u); r.push_back(g.getID()); break;
			case 59: { const VecObjectID &ids = g.getAllIDs(); r.push_back((unsigned)ids.size()); for (ObjectID i : ids) r.push_back((unsigned)i); break; }
			/* batch A25 */
			case 60: if (!memberWithoutAi(g)) g.groupMoveToPosition(&p, op.b != 0, src); break;
			case 61: g.groupScatter(src); break;
			case 62: g.groupTightenToPosition(&p, op.b != 0, src); break;
			case 63: g.groupFollowWaypointPath(&way, src); break;
			case 64: g.groupFollowWaypointPathExact(&way, src); break;
			case 65: g.groupMoveToAndEvacuate(&p, src); break;
			case 66: g.groupMoveToAndEvacuateAndExit(&p, src); break;
			case 67: g.groupFollowWaypointPathAsTeam(&way, src); break;
			case 68: g.groupFollowWaypointPathAsTeamExact(&way, src); break;
			case 69: { Bool b = g.friend_computeGroundPath(&p, src); r.push_back(b ? 1u : 0u); pathWords(g, r); break; }
			case 70: r.push_back(g.friend_moveInfantryToPos(&p, src) ? 1u : 0u); pathWords(g, r); break;
			case 71: r.push_back(g.friend_moveVehicleToPos(&p, src) ? 1u : 0u); pathWords(g, r); break;
			case 72: if (!memberWithoutAi(g)) { g.friend_moveFormationToPos(&p, src); pathWords(g, r); } break;
			case 73: { Coord3D d; Coord3D c; c.x = op.w; c.y = op.w * 0.5f; c.z = 0.0f;
				if (oa && oa->getAI()) { g.computeIndividualDestination(&d, &p, oa, &c, op.b != 0); r.push_back(bitsOf(d.x)); r.push_back(bitsOf(d.y)); r.push_back(bitsOf(d.z)); } break; }
			case 74: { Coord3D h = p; getHelicopterOffset(h, op.a); r.push_back(bitsOf(h.x)); r.push_back(bitsOf(h.y)); break; }
			}
		}
		r.push_back((unsigned)g.m_memberList.size());
		for (Object *m : g.m_memberList) r.push_back(idOf(m));
		pathWords(g, r);
		for (int i = 0; i < GRP_OBJECTS; ++i) r.push_back((unsigned)g_tmp[i]);
		r.push_back(g.m_memberListSize);
		r.push_back(bitsOf(g.m_speed));
		r.push_back(g.m_dirty ? 1u : 0u);
		r.push_back(g.m_id);
		g.m_memberList.clear();	/* (the destructor's leaving not tested here) */
	}
	for (int i = 0; i < (int)r.size() && i < outCap; ++i) out[i] = r[i];
	*logLen = (int)lg.size();
	for (int i = 0; i < (int)lg.size() && i < logCap; ++i) log[i] = lg[i];
	g_log = NULL;
	return (int)r.size();
}
