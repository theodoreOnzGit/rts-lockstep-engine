/* The stand-in world of the turret build (differential tests only; p1w13-sonnet, layer 10 batch T1): what TurretAI.cpp, AIStates.cpp's
** fire state and StateMachine.cpp call on objects, their AI, weapons, teams and players, the partition manager, the terrain, the
** pathfinder, the audio, the game logic and the global data. Every class is renamed Tu* by PreRTS.h. Pure questions are answered
** from the scene (differential/wrap_turret.cpp) or from a hash of their arguments that the Rust test computes too; every call with an
** effect writes a line to the wrapper's log. Upstream's enums are the real ones (GameCommon.h, GameType.h, loco_gen's, and the
** verbatim cuts of turret_gen/turret_defs.h). */
#ifndef SHIM_TURRET_WORLD_H
#define SHIM_TURRET_WORLD_H
#include "Lib/BaseType.h"
#include "Common/GameCommon.h"
#include "Common/GameType.h"
#include "turret_defs.h"

class Object;
class Team;
class Player;
class Weapon;
class MultiIniFieldParse;
class AIUpdateInterface;

/* ObjectStatusTypes.h's mask (its BitFlags is not in this build): the bits set */
struct ObjectStatusMaskType
{
	enum { kInit };
	unsigned long long lo, hi;
	ObjectStatusMaskType(int, int a) : lo(0), hi(0) { set(a); }
	ObjectStatusMaskType(int, int a, int b) : lo(0), hi(0) { set(a); set(b); }
	void set(int b) { if (b < 64) lo |= 1ull << b; else hi |= 1ull << (b - 64); }
};
#define MAKE_OBJECT_STATUS_MASK(k) ObjectStatusMaskType(ObjectStatusMaskType::kInit, (k))
#define MAKE_OBJECT_STATUS_MASK2(k,a) ObjectStatusMaskType(ObjectStatusMaskType::kInit, (k), (a))

/* the audio: a playing handle per event, the manager's set of playing handles */
class AudioEventRTS
{
public:
	AudioEventRTS() : m_handle(0), m_object(INVALID_ID) {}
	Bool isCurrentlyPlaying() const;
	void setObjectID(ObjectID id) { m_object = id; }
	void setPlayingHandle(UnsignedInt h) { m_handle = h; }
	UnsignedInt getPlayingHandle() const { return m_handle; }
	UnsignedInt m_handle;
	ObjectID m_object;
};
class AudioManager
{
public:
	UnsignedInt addAudioEvent(const AudioEventRTS *e);
	void removeAudioEvent(UnsignedInt handle);
};
extern AudioManager *TheAudio;

class ThingTemplate
{
public:
	const AudioEventRTS *getPerUnitSound(const char *name) const;
};
class GeometryInfo
{
public:
	Real getMaxHeightAbovePosition() const { return m_maxHeight; }
	Real m_maxHeight;
};
class TeamTemplateInfo { public: Bool m_attackCommonTarget; };
class TeamPrototype
{
public:
	const TeamTemplateInfo *getTemplateInfo() const { return &m_info; }
	TeamTemplateInfo m_info;
};
class Team
{
public:
	TeamPrototype *getPrototype() { return &m_prototype; }
	Object *getTeamTargetObject();
	void setTeamTargetObject(const Object *target);
	TeamPrototype m_prototype;
	UnsignedInt m_id;
	ObjectID m_target;
};
class Player { public: Int m_index; };

class Weapon
{
public:
	WeaponStatus getStatus() const;
	Bool isWithinAttackRange(const Object *source, const Object *target) const;
	Bool isWithinAttackRange(const Object *source, const Coord3D *pos) const;
	Real getAttackRange(const Object *source) const;
	Bool isContactWeapon() const;
	Bool hasLeechRange() const;
	Real getContinueAttackRange() const;
	Bool fireWeapon(const Object *source, const Coord3D *pos);
	void setPreAttackFinishedFrame(UnsignedInt frame);
	Int m_owner;
	Int m_slot;
};

class AIUpdateInterface
{
public:
	UnsignedInt getMoodMatrixActionAdjustment(MoodMatrixAction action) const;
	Bool areTurretsLinked() const;
	Object *getCurrentVictim() const;
	CommandSourceType getLastCommandSource() const;
	Bool isDoingGroundMovement() const;
	UnsignedInt getNextMoodCheckTime() const;
	void resetNextMoodCheckTime();
	WhichTurretType friend_getTurretSync() const;
	void friend_setTurretSync(WhichTurretType t);
	Object *getGoalObject();
	Object *getNextMoodTarget(Bool calledByAI, Bool calledDuringIdle);
	void addTargeter(ObjectID id, Bool add);
	Bool isTemporarilyPreventingAimSuccess() const;
	Int m_owner;
};

class Object
{
public:
	ObjectID getID() const;
	const Coord3D *getPosition() const;
	const ThingTemplate *getTemplate() const;
	Bool isKindOf(KindOfType t) const;
	Bool testStatus(ObjectStatusTypes bit) const;
	void setStatus(ObjectStatusMaskType mask, Bool set = true);
	void clearStatus(ObjectStatusMaskType mask) { setStatus(mask, false); }
	Bool isEffectivelyDead() const;
	Bool isDestroyed() const;
	Object *getContainedBy();
	Bool isAirborneTarget() const;
	Bool isSignificantlyAboveTerrain() const;
	Player *getControllingPlayer() const;
	Team *getTeam();
	const Team *getTeam() const;
	Weapon *getCurrentWeapon(WeaponSlotType *wslot = NULL);
	const Weapon *getCurrentWeapon(WeaponSlotType *wslot = NULL) const;
	Weapon *getWeaponInWeaponSlot(WeaponSlotType wslot) const;
	Bool isAbleToAttack() const;
	CanAttackResult getAbleToAttackSpecificObject(AbleToAttackType t, const Object *target, CommandSourceType commandSource) const;
	void chooseBestWeaponForTarget(const Object *target, WeaponChoiceCriteria criteria, CommandSourceType commandSource);
	void preFireCurrentWeapon(const Object *victim);
	void setFiringConditionForCurrentWeapon() const;
	void fireCurrentWeapon(Object *target);
	void fireCurrentWeapon(const Coord3D *pos);
	void releaseWeaponLock(WeaponLockType lock);
	void notifyFiringTrackerShotFired(const Weapon *weaponFired, ObjectID victimID);
	void setModelConditionState(ModelConditionFlagType a);
	void clearModelConditionState(ModelConditionFlagType a);
	void reactToTurretChange(WhichTurretType turret, Real oldRotation, Real oldPitch);
	const GeometryInfo &getGeometryInfo() const;
	AIUpdateInterface *getAI();
	const AIUpdateInterface *getAI() const;
	AIUpdateInterface *getAIUpdateInterface() { return getAI(); }
	const AIUpdateInterface *getAIUpdateInterface() const { return getAI(); }
	Int m_index;
};

class GameLogic
{
public:
	UnsignedInt getFrame();
	Object *findObjectByID(ObjectID id);
};
extern GameLogic *TheGameLogic;
class GlobalData { public: Int m_debugAI; };
extern GlobalData *TheGlobalData;

class PartitionFilter { public: virtual ~PartitionFilter() {} };
class PartitionFilterSamePlayer : public PartitionFilter
{
public:
	PartitionFilterSamePlayer(const Player *p) : m_player(p) {}
	const Player *m_player;
};
class PartitionFilterSameMapStatus : public PartitionFilter
{
public:
	PartitionFilterSameMapStatus(const Object *obj) : m_obj(obj) {}
	const Object *m_obj;
};
class PartitionFilterPossibleToAttack : public PartitionFilter
{
public:
	PartitionFilterPossibleToAttack(AbleToAttackType t, const Object *obj, CommandSourceType source) : m_type(t), m_obj(obj), m_source(source) {}
	AbleToAttackType m_type;
	const Object *m_obj;
	CommandSourceType m_source;
};
class PartitionManager
{
public:
	Real getRelativeAngle2D(const Object *obj, const Coord3D *pos) const;
	Real getDistanceSquared(const Object *obj, const Coord3D *pos, DistanceCalculationType dc) const;
	void getVectorTo(const Object *obj, const Coord3D *pos, DistanceCalculationType dc, Coord3D &vec) const;
	Object *getClosestObject(const Coord3D *pos, Real maxDist, DistanceCalculationType dc, PartitionFilter **filters);
};
extern PartitionManager *ThePartitionManager;
class TerrainLogic
{
public:
	void getBridgeAttackPoints(const Object *bridge, TBridgeAttackInfo *info);
};
extern TerrainLogic *TheTerrainLogic;
class Pathfinder
{
public:
	Bool isAttackViewBlockedByObstacle(const Object *attacker, const Coord3D &attackerPos, const Object *victim, const Coord3D &victimPos);
};
class AI
{
public:
	Pathfinder *pathfinder();
};
extern AI *TheAI;

/* PerfTimer.h's macros, as the release build has them (empty) */
#ifndef DECLARE_PERF_TIMER
#define DECLARE_PERF_TIMER(x)
#endif
#ifndef USE_PERF_TIMER
#define USE_PERF_TIMER(x)
#endif
#endif
