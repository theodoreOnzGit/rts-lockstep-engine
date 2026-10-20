/* The stand-ins of the world that Weapon.cpp and WeaponSet.cpp reach (p1w8-sonnet, slice B). Included by
** differential/wrap_weapon.cpp after the real Weapon.h and WeaponSet.h, with every class below renamed by that file's
** #define lines (WpnTest*) so that no symbol of this archive collides with another fixture's.
**
** Method: the classes answer from the program's configuration (struct members set by the driver from the program
** words) and every call that has an effect writes a line to g_lines, formatted exactly as the Rust ScenarioHost
** (tests/differential_weapon_fire.rs) writes it. Reals are written as their bit patterns. The queries that stand in for
** other units (distance, line of sight, the terrain heights, the bone transforms) are small deterministic formulas
** that the Rust host computes with the same operations: they are not under test, what Weapon does with them is.
** The WeaponSet of an Object is the REAL WeaponSet (WeaponSet.cpp), so that the shared-reload loop of Weapon.cpp acts
** on the same weapons upstream's does. */
#ifndef WPNFIRE_STANDINS_H
#define WPNFIRE_STANDINS_H

#include <vector>
#include <string>
#include <list>
#include <map>
#include <set>
#include <stdarg.h>
#include <stdio.h>
#include "Common/Snapshot.h"
#include "GameLogic/ObjectScriptStatusBits.h"

/* ---- the trace -------------------------------------------------------------------------------------------- */
static std::vector<std::string>* g_lines = NULL;
static unsigned fbits(float f) { unsigned u; memcpy(&u, &f, 4); return u; }
static void Rec(const char* fmt, ...)
{
	char buf[1024];
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(buf, sizeof buf, fmt, ap);
	va_end(ap);
	if (g_lines) g_lines->push_back(buf);
	static const bool trace = getenv("WPNFIRE_TRACE") != NULL;
	if (trace) fprintf(stderr, "%s\n", buf);
}
#define FB(x) fbits(x)

class Object; class Drawable; class Player; class Team; class ThingTemplate; class BehaviorModule;
class ContainModuleInterface; class AIUpdateInterface; class StealthUpdate; class SpawnBehaviorInterface;

/* ---- the enums of the headers the real Weapon.cpp includes (the ones GameCommon.h and GameType.h have are the real ones) ------ */
enum LocomotorSetType { LOCOMOTORSET_INVALID = -1, LOCOMOTORSET_NORMAL = 0, LOCOMOTORSET_NORMAL_UPGRADED, LOCOMOTORSET_FREEFALL, LOCOMOTORSET_WANDER,
	LOCOMOTORSET_PANIC, LOCOMOTORSET_TAXIING, LOCOMOTORSET_SUPERSONIC, LOCOMOTORSET_SLUGGISH, LOCOMOTORSET_COUNT };
enum DistanceCalculationType { FROM_CENTER_2D = 0, FROM_CENTER_3D = 1, FROM_BOUNDINGSPHERE_2D = 2, FROM_BOUNDINGSPHERE_3D = 3 };
typedef UnsignedInt CommandSourceMask;
/* GameMemory.h's MemoryPoolObjectHolder */
class MemoryPoolObjectHolder
{
	MemoryPoolObject* m_mpo;
public:
	MemoryPoolObjectHolder(MemoryPoolObject* mpo) : m_mpo(mpo) {}
	// Stand-in difference: upstream relies on its pool's null-tolerant
	// deleteInstance(); a plain `this` test is optimised away by clang,
	// so the null check lives here.
	~MemoryPoolObjectHolder() { if (m_mpo) m_mpo->deleteInstance(); }
};

/* ---- configuration ------------------------------------------------------------------------------------------ */
struct TBridgeAttackInfo { Coord3D attackPoint1, attackPoint2; };

struct WpnPlayerCfg { int battlePlans; unsigned mask; int rel[4]; };

/* the world's configuration, filled by the driver */
struct WpnWorldCfg
{
	WpnPlayerCfg players[4]; int nPlayers;
	unsigned frame; unsigned historicDamageLimit; Region3D extent;
	WeaponBonusSet* globalBonus;
};
static static WpnWorldCfg g_wcfg;

/* ---- the behaviour modules ------------------------------------------------------------------------------------ */
class ProjectileUpdateInterface
{
public:
	Object* m_owner;
	void projectileLaunchAtObjectOrPosition(const Object* victim, const Coord3D* victimPos, const Object* launcher, WeaponSlotType wslot,
		Int specificBarrelToUse, const WeaponTemplate* detWeap, const ParticleSystemTemplate* exhaustSysOverride);
	ObjectID projectileGetLauncherID() const;
};
class LandMineInterface { public: Object* m_owner; void disarm(); };
class ParkingPlaceBehaviorInterface { public: Object* m_owner; Bool hasReservedSpace(ObjectID id) const; };
class SpecialPowerCompletionDie { public: Object* m_owner; void notifyScriptEngine(); void setCreator(ObjectID id); };
class UpdateModule { public: virtual ~UpdateModule() {} };
class ProjectileStreamUpdate : public UpdateModule
{
public:
	Object* m_owner;
	void setPosition(const Coord3D* newPosition);
	void addProjectile(ObjectID sourceID, ObjectID newID, ObjectID victimID, const Coord3D* victimPos);
};
class AssistedTargetingUpdate : public UpdateModule
{
public:
	Object* m_owner;
	Bool isFreeToAssist() const;
	void assistAttack(const Object* requestingObject, Object* victimObject);
};
class LaserUpdate { public: Object* m_owner; void initLaser(const Object* parent, const Object* target, const Coord3D* startPos, const Coord3D* endPos, AsciiString parentBoneName); };
class PhysicsBehavior { public: Object* m_owner; void transferVelocityTo(PhysicsBehavior* that) const; void setIgnoreCollisionsWith(const Object* obj); };
class ExperienceTracker { public: Object* m_owner; void setExperienceSink(ObjectID sink); };
class AssaultTransportAIInterface { public: Object* m_owner; void beginAssault(const Object* designatedTarget) const; };
class AcademyStats { public: int m_player; void recordMineCleared(); };

class BehaviorModule
{
public:
	Object* m_owner; int m_kind; /* 0 land mine, 1 projectile update, 2 parking place */
	LandMineInterface* getLandMineInterface();
	ProjectileUpdateInterface* getProjectileUpdateInterface();
	ParkingPlaceBehaviorInterface* getParkingPlaceBehaviorInterface();
};

class AIUpdateInterface
{
public:
	Object* m_owner;
	Bool getSneakyTargetingOffset(Coord3D* offset) const;
	LocomotorSetType getCurLocomotorSetType() const;
	Bool isAircraftThatAdjustsDestination() const;
	AssaultTransportAIInterface* getAssaultTransportAIInterface() const;
	WhichTurretType getWhichTurretForWeaponSlot(WeaponSlotType wslot, Real* turretAngle, Real* turretPitch) const;
	Bool isWeaponSlotOnTurretAndAimingAtTarget(WeaponSlotType wslot, const Object* victim) const;
	Bool getTurretRotAndPitch(WhichTurretType tur, Real* turretAngle, Real* turretPitch) const;
};

typedef std::list<Object*> ContainedItemsList;
class ContainModuleInterface
{
public:
	Object* m_owner;
	Int getContainCount() const;
	Bool isGarrisonable() const;
	Bool isImmuneToClearBuildingAttacks() const;
	Bool isWeaponBonusPassedToPassengers() const;
	WeaponBonusConditionFlags getWeaponBonusPassedToPassengers() const;
	Bool isEnclosingContainerFor(const Object* obj) const;
	Bool calcBestGarrisonPosition(Coord3D* sourcePos, const Coord3D* targetPos);
	Bool isPassengerAllowedToFire() const;
	const ContainedItemsList* getContainedItemsList() const;
	const Player* getApparentControllingPlayer(const Player* observingPlayer) const;
};

class StealthUpdate { public: Object* m_owner; Bool isDisguised() const; Int getDisguisedPlayerIndex() const; };
class SpawnBehaviorInterface
{
public:
	Object* m_owner;
	CanAttackResult getCanAnySlavesUseWeaponAgainstTarget(AbleToAttackType attackType, const Object* victim, const Coord3D* pos, CommandSourceType commandSource) const;
};

/* ---- the teams, the players, the thing templates ------------------------------------------------------------- */
class Team { public: int m_index; Relationship getRelationship(const Team* that) const; };
class Player
{
public:
	int m_index; Team m_team; AcademyStats m_academy;
	Team* getDefaultTeam() { return &m_team; }
	const Team* getDefaultTeam() const { return &m_team; }
	Int getNumBattlePlansActive() const { return g_wcfg.players[m_index].battlePlans; }
	void applyBattlePlanBonusesForObject(Object* obj) const;
	AcademyStats* getAcademyStats() const { return const_cast<AcademyStats*>(&m_academy); }
	PlayerMaskType getPlayerMask() const { return (PlayerMaskType)g_wcfg.players[m_index].mask; }
	Relationship getRelationship(const Team* that) const { return m_team.getRelationship(that); }
	void iterateObjects(void (*func)(Object*, void*), void* userData);
};
class PlayerList { public: Player* getNthPlayer(Int i); };

class ThingTemplate
{
public:
	int m_id; int m_group; Real m_fenceWidth; AsciiString m_cmdSet;
	const AsciiString& friend_getCommandSetString() const { return m_cmdSet; }
	WeaponTemplateSetVector m_weaponTemplateSets;
	WeaponTemplateSetFinder m_weaponTemplateSetFinder;
	Bool isEquivalentTo(const ThingTemplate* tt) const { return m_group == tt->m_group; }
	Real getFenceWidth() const { return m_fenceWidth; }
	const WeaponTemplateSet* findWeaponTemplateSet(const WeaponSetFlags& t) const { return m_weaponTemplateSetFinder.findBestInfo(m_weaponTemplateSets, t); }
	AsciiString getName() const { char b[32]; sprintf(b, "T%d", m_id); return AsciiString(b); }
};

/* ---- the drawable and the client calls ---------------------------------------------------------------------- */
class FXList
{
public:
	static void doFXPos(const FXList* fx, const Coord3D* primary, const Matrix3D* primaryMtx, Real primarySpeed, const Coord3D* secondary, Real radius);
};
class ObjectCreationList
{
public:
	static Object* create(const ObjectCreationList* ocl, const Object* primary, const Object* secondary);
};
class Drawable
{
public:
	Object* m_owner;
	Int getBarrelCount(WeaponSlotType wslot) const;
	void updateDrawableClipStatus(UnsignedInt ammoInClip, UnsignedInt maxClipSize, WeaponSlotType slot);
	void clearAndSetModelConditionFlags(const ModelConditionFlags& clr, const ModelConditionFlags& set);
	void setAnimationLoopDuration(UnsignedInt numFrames);
	Bool handleWeaponFireFX(WeaponSlotType wslot, Int specificBarrelToUse, const FXList* fxl, Real weaponSpeed, Real recoilAmount, Real recoilAngle, const Coord3D* victimPos, Real damageRadius);
	const Coord3D* getPosition() const;
	const Matrix3D* getTransformMatrix() const;
	Bool getProjectileLaunchOffset(WeaponSlotType wslot, Int specificBarrelToUse, Matrix3D* launchPos, WhichTurretType tur, Coord3D* turretRotPos, Coord3D* turretPitchPos) const;
	Int getPristineBonePositions(const char* boneNamePrefix, Int startIndex, Coord3D* positions, Matrix3D* transforms, Int maxBones) const;
	void convertBonePosToWorldPos(const Coord3D* bonePos, const Matrix3D* boneTransform, Coord3D* worldPos, Matrix3D* worldTransform) const;
	void setDrawableHidden(Bool hidden);
	LaserUpdate* findClientUpdateModule(NameKeyType key);
};

class FiringTracker { public: Object* m_owner; void shotFired(const Weapon* weaponFired, ObjectID victimID); };
/* the control bar's command sets: a name "Cmd<k>" is a set whose buttons are formulas of k (fire_driver.h) */
class UpgradeTemplate { public: int dummy; };
class CommandButton { public: const UpgradeTemplate* m_up; const UpgradeTemplate* getUpgradeTemplate() const { return m_up; } };
enum { MAX_COMMANDS_PER_SET = 18 };
class CommandSet { public: int m_k; CommandButton m_buttons[MAX_COMMANDS_PER_SET]; const CommandButton* getCommandButton(Int i) const; };
class ControlBar { public: const CommandSet* findCommandSet(const AsciiString& name) const; };
extern ControlBar* TheControlBar;

/* ---- the object -------------------------------------------------------------------------------------------- */
class Object
{
public:
	/* identity */
	int m_id; ThingTemplate* m_template; int m_player; /* -1: none */
	/* transform */
	Coord3D m_pos; Real m_orientation; Matrix3D m_xform; GeometryInfo m_geom;
	/* state */
	KindOfMaskType m_kindOf; unsigned long long m_statusBits; unsigned m_scriptStatus; int m_veterancy; Bool m_locallyControlled;
	WeaponBonusConditionFlags m_bonusCond; int m_containedBy; int m_producer;
	Bool m_effDead, m_destroyed, m_ableToAttack; Int m_layer; Bool m_aboveTerrain, m_signifAbove;
	Int m_consecutiveShots; Real m_estimate; WeaponSetFlags m_wsFlags;
	/* the drawable */
	Bool m_hasDrawable; Int m_barrels[3]; Drawable m_drawableStorage; Drawable* m_drawable; Bool m_fxHandled;
	Bool m_hasLaunchOffset; Matrix3D m_launchXform; Coord3D m_turretRotPos, m_turretPitchPos;
	Bool m_hasLaserUpdate; LaserUpdate m_laser;
	/* the AI */
	Bool m_hasAI; Bool m_sneaky; Coord3D m_sneakyOffset; Int m_locoSet; Bool m_adjustsDest; Bool m_assaultIface;
	Int m_turret[3]; Real m_turretAngle[3]; Real m_turretPitch[3]; Bool m_aiming[3]; AIUpdateInterface m_ai; AssaultTransportAIInterface m_assault;
	/* the contain module */
	Bool m_hasContain; Int m_containCount; Bool m_garrisonable, m_immuneClear, m_passedBonus; WeaponBonusConditionFlags m_passedFlags;
	std::set<int> m_enclosing; Bool m_bestGarrison; Bool m_passengerFire; std::list<int> m_itemIds; ContainedItemsList m_items; bool m_hasItems;
	int m_apparent; /* -2: none, else the player index */
	ContainModuleInterface m_contain;
	/* stealth, spawn */
	Bool m_hasStealth, m_disguised; Int m_disguisedPlayer; StealthUpdate m_stealth;
	Bool m_hasSpawn; Int m_spawnResult; SpawnBehaviorInterface m_spawn;
	/* modules */
	Bool m_hasLandMine, m_hasPUI, m_hasParking, m_hasSPCDie, m_hasStream, m_hasAssist, m_assistFree, m_hasCountermeasures, m_hasPhysics;
	int m_launcherId; std::set<int> m_reserved; std::set<int> m_collides;
	BehaviorModule m_mods[3]; BehaviorModule* m_modList[4]; ProjectileUpdateInterface m_pui; LandMineInterface m_lmi; ParkingPlaceBehaviorInterface m_ppi;
	SpecialPowerCompletionDie m_spcDie; ProjectileStreamUpdate m_stream; AssistedTargetingUpdate m_assist; PhysicsBehavior m_phys; ExperienceTracker m_xp;
	/* the weapon set */
	WeaponSet m_weaponSet;
	/* Object.cpp's weapon functions run on these (Object_weapons.cpp) */
	FiringTracker* m_firingTracker; FiringTracker m_tracker; Byte m_lastWeaponCondition[WEAPONSLOT_COUNT]; Bool m_hasTracker; AsciiString m_commandSetStringOverride;

	Object();

	ObjectID getID() const { return (ObjectID)m_id; }
	const Coord3D* getPosition() const { return &m_pos; }
	Real getOrientation() const { return m_orientation; }
	const Matrix3D* getTransformMatrix() const { return &m_xform; }
	const GeometryInfo& getGeometryInfo() const { return m_geom; }
	Bool isKindOf(KindOfType t) const { return m_kindOf.test(t); }
	Bool isKindOfMulti(const KindOfMaskType& mustBeSet, const KindOfMaskType& mustBeClear) const { return m_kindOf.testSetAndClear(mustBeSet, mustBeClear); }
	Bool testStatus(ObjectStatusTypes s) const { return ((m_statusBits >> s) & 1) != 0; }
	Bool testScriptStatusBit(unsigned b) const { return (m_scriptStatus & b) != 0; }
	VeterancyLevel getVeterancyLevel() const { return (VeterancyLevel)m_veterancy; }
	Drawable* getDrawable() const { return m_drawable; }
	Bool isLocallyControlled() const { return m_locallyControlled; }
	WeaponBonusConditionFlags getWeaponBonusCondition() const { return m_bonusCond; }
	const WeaponSetFlags& getWeaponSetFlags() const { return m_wsFlags; }
	const ThingTemplate* getTemplate() const { return m_template; }
	Bool isReloadTimeShared() const { return m_weaponSet.isSharedReloadTime(); }
	Weapon* getWeaponInWeaponSlot(WeaponSlotType wslot) const { return m_weaponSet.getWeaponInWeaponSlot(wslot); }
	Int getNumConsecutiveShotsFiredAtTarget(const Object* victim) const { return m_consecutiveShots; }
	Bool isAboveTerrain() const { return m_aboveTerrain; }
	Bool isAirborneTarget() const { return testStatus(OBJECT_STATUS_AIRBORNE_TARGET); }
	Bool isSignificantlyAboveTerrain() const { return m_signifAbove; }
	PathfindLayerEnum getLayer() const { return (PathfindLayerEnum)m_layer; }
	ObjectID getProducerID() const { return (ObjectID)m_producer; }
	Bool isEffectivelyDead() const { return m_effDead; }
	Bool isDestroyed() const { return m_destroyed; }
	Bool isAbleToAttack() const { return m_ableToAttack; }
	Real estimateDamage(DamageInfoInput& damageInfo) const;
	void attemptDamage(DamageInfo* damageInfo);
	Relationship getRelationship(const Object* that) const;
	Player* getControllingPlayer() const;
	Team* getTeam() const;
	const Object* getContainedBy() const;
	ContainModuleInterface* getContain() const { return m_hasContain ? const_cast<ContainModuleInterface*>(&m_contain) : NULL; }
	AIUpdateInterface* getAI() const { return m_hasAI ? const_cast<AIUpdateInterface*>(&m_ai) : NULL; }
	const AIUpdateInterface* getAIUpdateInterface() const { return getAI(); }
	StealthUpdate* getStealth() const { return m_hasStealth ? const_cast<StealthUpdate*>(&m_stealth) : NULL; }
	SpawnBehaviorInterface* getSpawnBehaviorInterface() const { return m_hasSpawn ? const_cast<SpawnBehaviorInterface*>(&m_spawn) : NULL; }
	PhysicsBehavior* getPhysics() const { return m_hasPhysics ? const_cast<PhysicsBehavior*>(&m_phys) : NULL; }
	ExperienceTracker* getExperienceTracker() const { return const_cast<ExperienceTracker*>(&m_xp); }
	SpecialPowerCompletionDie* findSpecialPowerCompletionDie() const { return m_hasSPCDie ? const_cast<SpecialPowerCompletionDie*>(&m_spcDie) : NULL; }
	UpdateModule* findUpdateModule(NameKeyType key) const;
	BehaviorModule** getBehaviorModules() const { return const_cast<BehaviorModule**>(m_modList); }
	Bool hasCountermeasures() const { return m_hasCountermeasures; }
	void reportMissileForCountermeasures(Object* missile);
	void setProducer(const Object* obj);
	void setPosition(const Coord3D* pos);
	void setTransformMatrix(const Matrix3D* mx);
	/* declared as Object.h declares them; defined by Object_weapons.cpp, the text of Object.cpp */
	void reloadAllAmmo(Bool now);
	Bool isOutOfAmmo() const;
	Bool hasAnyWeapon() const;
	Bool hasAnyDamageWeapon() const;
	Bool hasWeaponToDealDamageType(DamageType typeToDeal) const;
	Real getLargestWeaponRange() const;
	UnsignedInt getMostPercentReadyToFireAnyWeapon() const;
	void setFiringConditionForCurrentWeapon() const;
	void adjustModelConditionForWeaponStatus();
	void fireCurrentWeapon(Object *target);
	void fireCurrentWeapon(const Coord3D* pos);
	void preFireCurrentWeapon( const Object *victim );
	Weapon* getCurrentWeapon(WeaponSlotType* wslot = NULL);
	const Weapon* getCurrentWeapon(WeaponSlotType* wslot = NULL) const;
	Weapon* findWaypointFollowingCapableWeapon();
	void notifyFiringTrackerShotFired( const Weapon* weaponFired, ObjectID victimID );
	UnsignedInt getLastShotFiredFrame() const;
	Bool getAmmoPipShowingInfo(Int& numTotal, Int& numFull) const;
	CanAttackResult getAbleToAttackSpecificObject( AbleToAttackType t, const Object* target, CommandSourceType commandSource, WeaponSlotType specificSlot = (WeaponSlotType)-1 ) const;
	CanAttackResult getAbleToUseWeaponAgainstTarget( AbleToAttackType attackType, const Object *victim, const Coord3D *pos, CommandSourceType commandSource, WeaponSlotType specificSlot = (WeaponSlotType)-1 ) const;
	Bool chooseBestWeaponForTarget(const Object* target, WeaponChoiceCriteria criteria, CommandSourceType cmdSource);
	void clearLeechRangeModeForAllWeapons();
	Bool getSingleLogicalBonePosition(const char* boneName, Coord3D* position, Matrix3D* transform) const;
	Bool getSingleLogicalBonePositionOnTurret( WhichTurretType whichTurret, const char* boneName, Coord3D* position, Matrix3D* transform ) const;
	Int getMultiLogicalBonePosition(const char* boneNamePrefix, Int maxBones, Coord3D* positions, Matrix3D* transforms, Bool convertToWorld = FALSE ) const;
	const AsciiString& getCommandSetString() const;
	Bool canProduceUpgrade( const UpgradeTemplate *upgrade );
	/* the inline forwarders of Object.h, as written there */
	UnsignedInt getWeaponInWeaponSlotCommandSourceMask( WeaponSlotType wSlot ) const { return m_weaponSet.getNthCommandSourceMask( wSlot ); }
	Bool setWeaponLock( WeaponSlotType weaponSlot, WeaponLockType lockType ){ return m_weaponSet.setWeaponLock( weaponSlot, lockType ); }
	void releaseWeaponLock(WeaponLockType lockType){ m_weaponSet.releaseWeaponLock(lockType); }
	Bool isCurWeaponLocked() const { return m_weaponSet.isCurWeaponLocked(); }
	void friend_setUndetectedDefector(Bool status);
	void convertBonePosToWorldPos(const Coord3D* bonePos, const Matrix3D* boneTransform, Coord3D* worldPos, Matrix3D* worldTransform) const;
};

/* ---- the world ---------------------------------------------------------------------------------------------- */
static std::vector<Object*> g_objs;      /* ids 1..N in order */
static std::vector<ThingTemplate*> g_thingTemplates;
static std::vector<Player*> g_players;
static int g_nextObjectId = 1;

static Object* wpnFind(int id) { return (id >= 1 && id <= (int)g_objs.size()) ? g_objs[id - 1] : NULL; }

class SimpleObjectIterator : public MemoryPoolObject
{
public:
	std::vector<std::pair<Object*, Real> > m_list; size_t m_at;
	SimpleObjectIterator() : m_at(0) {}
	Object* first() { return firstWithNumeric(NULL); }
	Object* next() { return nextWithNumeric(NULL); }
	Object* firstWithNumeric(Real* num = NULL) { m_at = 0; return nextWithNumeric(num); }
	Object* nextWithNumeric(Real* num = NULL)
	{
		if (m_at >= m_list.size()) return NULL;
		if (num) *num = m_list[m_at].second;
		return m_list[m_at++].first;
	}
};
typedef SimpleObjectIterator ObjectIterator;

static Real wpnR(const Object* o) { return o->m_geom.getBoundingCircleRadius(); }
static Real wpnS(const Object* o) { return o->m_geom.getBoundingSphereRadius(); }
/* the distance the stand-in partition manager measures (squared): the plain squared distance, less the squared sum of the radii for the bounding forms,
** floored at zero. The Rust ScenarioHost computes the same operations. */
static Real wpnDist(const Coord3D& a, Real ra, const Coord3D& b, Real rb, DistanceCalculationType t)
{
	Real dx = a.x - b.x, dy = a.y - b.y, dz = a.z - b.z;
	Real d = dx * dx + dy * dy;
	if (t == FROM_CENTER_3D || t == FROM_BOUNDINGSPHERE_3D) d = d + dz * dz;
	if (t == FROM_BOUNDINGSPHERE_2D || t == FROM_BOUNDINGSPHERE_3D)
	{
		Real r = ra + rb;
		d = d - r * r;
		if (d < 0.0f) d = 0.0f;
	}
	return d;
}
static Real wpnRad(const Object* o, DistanceCalculationType t) { return (t == FROM_BOUNDINGSPHERE_3D) ? wpnS(o) : wpnR(o); }

class PartitionManager
{
public:
	Real getDistanceSquared(const Object* obj, const Object* other, DistanceCalculationType t) const
	{ return wpnDist(obj->m_pos, wpnRad(obj, t), other->m_pos, wpnRad(other, t), t); }
	Real getDistanceSquared(const Object* obj, const Coord3D* other, DistanceCalculationType t) const
	{ return wpnDist(obj->m_pos, wpnRad(obj, t), *other, 0.0f, t); }
	Real getGoalDistanceSquared(const Object* obj, const Coord3D* goalPos, const Object* other, DistanceCalculationType t) const
	{ return wpnDist(*goalPos, wpnRad(obj, t), other->m_pos, wpnRad(other, t), t); }
	Real getGoalDistanceSquared(const Object* obj, const Coord3D* goalPos, const Coord3D* other, DistanceCalculationType t) const
	{ return wpnDist(*goalPos, wpnRad(obj, t), *other, 0.0f, t); }
	void getVectorTo(const Object* obj, const Object* other, DistanceCalculationType t, Coord3D& vec) const
	{ vec.x = other->m_pos.x - obj->m_pos.x; vec.y = other->m_pos.y - obj->m_pos.y; vec.z = other->m_pos.z - obj->m_pos.z; }
	void getVectorTo(const Object* obj, const Coord3D* other, DistanceCalculationType t, Coord3D& vec) const
	{ vec.x = other->x - obj->m_pos.x; vec.y = other->y - obj->m_pos.y; vec.z = other->z - obj->m_pos.z; }
	SimpleObjectIterator* iterateObjectsInRange(const Coord3D* pos, Real maxDist, DistanceCalculationType t) const
	{
		SimpleObjectIterator* it = new SimpleObjectIterator;
		for (size_t i = 0; i < g_objs.size(); ++i)
		{
			Real d = wpnDist(*pos, 0.0f, g_objs[i]->m_pos, wpnRad(g_objs[i], t), t);
			if (d <= maxDist * maxDist) it->m_list.push_back(std::make_pair(g_objs[i], d));
		}
		return it;
	}
	ObjectIterator* iteratePotentialCollisions(const Coord3D* pos, const GeometryInfo& geom, Real angle) const;
	Bool isClearLineOfSightTerrain(const Object* obj, const Coord3D& objPos, const Object* obj2, const Coord3D& obj2Pos) const
	{ return objPos.z + 0.5f * obj2Pos.z >= 3.0f; }
};
class TerrainLogic
{
public:
	void getBridgeAttackPoints(const Object* bridge, TBridgeAttackInfo* info) const
	{
		info->attackPoint1.x = bridge->m_pos.x + 10.0f; info->attackPoint1.y = bridge->m_pos.y; info->attackPoint1.z = bridge->m_pos.z;
		info->attackPoint2.x = bridge->m_pos.x - 10.0f; info->attackPoint2.y = bridge->m_pos.y + 5.0f; info->attackPoint2.z = bridge->m_pos.z;
	}
	Real getGroundHeight(Real x, Real y) const { return x * 0.01f + y * 0.02f; }
	Real getLayerHeight(Real x, Real y, PathfindLayerEnum layer) const { return x * 0.01f + y * 0.02f + (Real)layer * 1.5f; }
	void getExtent(Region3D* extent) const { *extent = g_wcfg.extent; }
};
class Pathfinder { public: void adjustTargetDestination(const Object* obj, const Object* target, const Coord3D* targetPos, const Weapon* weapon, Coord3D* destination) const; };
class AIManager { public: Pathfinder m_pf; Pathfinder* pathfinder() { return &m_pf; } };
class GameLogic
{
public:
	UnsignedInt getFrame() const { return g_wcfg.frame; }
	Object* findObjectByID(ObjectID id) const { return wpnFind((int)id); }
	void destroyObject(Object* obj);
};
class ThingFactory
{
public:
	const ThingTemplate* findTemplate(const AsciiString& name, Bool check = TRUE);
	Object* newObject(const ThingTemplate* tmplt, Team* team);
};
extern ThingFactory* TheThingFactory;
class GlobalData { public: WeaponBonusSet* m_weaponBonusSet; UnsignedInt m_historicDamageLimit; };
extern GlobalData* TheGlobalData; extern GameLogic* TheGameLogic; extern PartitionManager* ThePartitionManager; extern TerrainLogic* TheTerrainLogic;
extern AIManager* TheAI; extern PlayerList* ThePlayerList;

#endif
