/* The stand-ins of the units Object.cpp calls into (p1w6-object), part 1: base classes, templates,
** the module classes. Included by differential/wrap_object.cpp after object_prelude.h. See the header
** of that file for the method: every stand-in that has an effect writes a record (Rec) that the Rust
** side writes too; every answer comes from the program's configuration (struct ObjCfg). */
#ifndef SHIM_OBJECT_STANDINS_H
#define SHIM_OBJECT_STANDINS_H

class Object; class Team; class Player; class Drawable; class ThingTemplate; class AIGroup; class Waypoint;
class CommandButton; class PartitionData; class RadarObject; class Dict; class Weapon; class StealthUpdate;
class BehaviorModule; class Module; class SpecialPowerTemplate; class UpgradeTemplate; class AttackPriorityInfo;
class PolygonTrigger; class Anim2DTemplate; class ToppleUpdate; class StickyBombUpdate;

/* ---- memory pool and OVERRIDE ------------------------------------------------------------------- */
class MemoryPoolObject
{
public:
	/* MemoryPool::allocateBlockImplementation zero-fills the block ("allocate a block from this pool and return it,
	** and zero out the contents"): members a constructor leaves alone (m_destinationLayer) are zero */
	static void* operator new(size_t n) { void* p = ::operator new(n); memset(p, 0, n); return p; }
	static void operator delete(void* p) { ::operator delete(p); }
	virtual ~MemoryPoolObject() {}
	void deleteInstance() { delete this; }
};
class MemoryPoolObjectHolder
{
	MemoryPoolObject* m;
public:
	MemoryPoolObjectHolder(MemoryPoolObject* o) : m(o) {}
	~MemoryPoolObjectHolder() { if (m) m->deleteInstance(); }
};
template <class T> class OVERRIDE
{
	const T* p;
public:
	OVERRIDE() : p(0) {}
	OVERRIDE(const T* t) : p(t) {}
	OVERRIDE& operator=(const T* t) { p = t; return *this; }
	operator const T*() const { return p; }
	const T* operator->() const { return p; }
};

/* the program's configuration: what the cut points answer (the same numbers as ObjectCuts on the Rust side) */
struct ObjCfgPlayer
{
	int index; int defaultTeam; unsigned color, nightColor; bool active, playable, skirmish;
	unsigned long long upgradesLo, upgradesHi; int battlePlans; int productionLevel;
};
struct ObjCfgTeam { int id; int player; std::string name; bool templateInfo; bool attackInfo; };
struct ObjCfg
{
	std::vector<ObjCfgPlayer> players;
	std::vector<ObjCfgTeam> teams;
	std::map<std::pair<int, int>, int> teamRel;   /* (team, that team) -> Relationship */
	std::map<std::pair<int, int>, int> playerRel; /* (player, team) -> Relationship */
	int neutralDefaultTeam; int localPlayer; int playerCount;
	float gravity; bool followTimeOfDay, night, followWeather, snowy, levelGainAnim, enableRepulsors, drawIconUI, difficultyBonus;
	float extentHiX, extentHiY; bool extentKnown;
	float wallHeight;
	int veterancyUpgrade[4]; /* bit index + 1 of the veterancy upgrade mask, 0 = none */
	std::set<int> specialPowerDenied;
	std::set<std::pair<int, int> > weapons;
	std::set<int> weaponSetsWithWeapons;
	std::map<std::pair<int, int>, int> aiTurrets; /* (object, slot) -> turret */
	std::set<std::pair<int, int> > turretsDisabled;
	std::set<int> aiAirborne;
	std::set<int> dozerAIs;
	std::map<std::pair<int, int>, float> turretRotation;
	std::map<std::pair<int, int>, int> shroudedStatus;
	std::vector<int> potentialCollisions, boobyTrapsInRange;
	ObjCfg() : neutralDefaultTeam(0), localPlayer(0), playerCount(0), gravity(-1.0f), followTimeOfDay(true), night(false), followWeather(true),
		snowy(false), levelGainAnim(false), enableRepulsors(false), drawIconUI(true), difficultyBonus(false), extentHiX(0), extentHiY(0),
		extentKnown(false), wallHeight(0) { veterancyUpgrade[0] = veterancyUpgrade[1] = veterancyUpgrade[2] = veterancyUpgrade[3] = 0; }
};
extern ObjCfg g_cfg;

/* ---- the name keys ---------------------------------------------------------------------------------- */
class NameKeyGenerator
{
public:
	std::map<std::string, int> keys;
	std::vector<std::string> names;
	NameKeyType nameToKey(const char* n)
	{
		std::map<std::string, int>::iterator it = keys.find(n);
		if (it != keys.end()) return (NameKeyType)it->second;
		int k = (int)names.size() + 1;
		keys[n] = k;
		names.push_back(n);
		return (NameKeyType)k;
	}
	NameKeyType nameToKey(const AsciiString& n) { return nameToKey(n.str()); }
	AsciiString keyToName(NameKeyType k)
	{
		if ((int)k < 1 || (int)k > (int)names.size()) return AsciiString();
		return AsciiString(names[(int)k - 1].c_str());
	}
};
extern NameKeyGenerator* TheNameKeyGenerator;
#define NAMEKEY(s) (TheNameKeyGenerator->nameToKey(s))

/* ---- the misc small value classes ----------------------------------------------------------------------- */
enum TimeOfDay { TIME_OF_DAY_MORNING = 1, TIME_OF_DAY_NIGHT = 4 };
enum Weather { WEATHER_NORMAL = 0, WEATHER_SNOWY = 1 };

class AudioEventRTS
{
public:
	const char* m_name; ObjectID m_objectID; bool m_hasPos; Coord3D m_pos;
	AudioEventRTS() : m_name(""), m_objectID(INVALID_ID), m_hasPos(false) { m_pos.zero(); }
	AudioEventRTS(const char* n) : m_name(n), m_objectID(INVALID_ID), m_hasPos(false) { m_pos.zero(); }
	void setPosition(const Coord3D* p) { m_pos = *p; m_hasPos = true; }
	void setObjectID(ObjectID id) { m_objectID = id; }
};
struct MiscAudio
{
	AudioEventRTS m_splatterVehiclePilotsBrain, m_buildingDisabled, m_vehicleDisabled, m_buildingReenabled, m_vehicleReenabled,
		m_unitPromoted, m_defectorTimerTickSound;
	MiscAudio() : m_splatterVehiclePilotsBrain("SplatterVehiclePilotsBrain"), m_buildingDisabled("BuildingDisabled"),
		m_vehicleDisabled("VehicleDisabled"), m_buildingReenabled("BuildingReenabled"), m_vehicleReenabled("VehicleReenabled"),
		m_unitPromoted("UnitPromoted"), m_defectorTimerTickSound("DefectorTimerTick") {}
};
class AudioManager
{
public:
	MiscAudio misc;
	MiscAudio* getMiscAudio() { return &misc; }
	void addAudioEvent(const AudioEventRTS* e);
};
extern AudioManager* TheAudio;

class SightingInfo : public Snapshot
{
public:
	Coord3D m_where; PlayerMaskType m_forWhom; Real m_howFar; UnsignedInt m_data;
	SightingInfo() { reset(); }
	void reset() { m_where.x = m_where.y = m_where.z = 0.0f; m_forWhom = 0; m_howFar = 0.0f; m_data = 0; }
	Bool isInvalid() const { return m_howFar == 0.0f; }
	void deleteInstance() { delete this; }
	void crc(Xfer* x) {}
	void xfer(Xfer* x)
	{
		XferVersion cur = 1, v = 1;
		x->xferVersion(&v, cur);
		x->xferCoord3D(&m_where);
		x->xferReal(&m_howFar);
		x->xferUser(&m_forWhom, sizeof(PlayerMaskType));
		x->xferUnsignedInt(&m_data);
	}
	void loadPostProcess() {}
};

/* ---- GeometryInfo (the class text and the functions Object reaches are generated from Geometry.h/.cpp) ---- */
template <typename NUM> inline NUM sqr(NUM x) { return x * x; }
template <typename NUM> inline NUM clamp(NUM lo, NUM val, NUM hi) { if (val < lo) return lo; else if (val > hi) return hi; else return val; }
struct Region2D { Coord2D lo, hi; };
#include "geometry_class.inc"

/* ---- templates ---------------------------------------------------------------------------------------- */
class WeaponTemplateSet
{
public:
	WeaponSetFlags m_types; Bool m_hasWeapon;
	Bool testWeaponSetFlag(WeaponSetType t) const { return m_types.test(t); }
};
class ModuleInfo
{
public:
	std::vector<std::pair<AsciiString, AsciiString> > v;
	Int getCount() const { return (Int)v.size(); }
	const AsciiString& getNthName(Int i) const { return v[i].first; }
	const void* getNthData(Int i) const { return NULL; }
};
class ThingTemplate
{
public:
	AsciiString name; int templateID; KindOfMaskType kindof; Real vision, shroudClearing, shroudRevealToAll; GeometryInfo geom;
	Int energyProduction, energyBonus; Int buildCost, threatValue; Int crusherLevel, crushableLevel, transportSlots; Bool enterGuard;
	UnsignedInt occlusionDelay; RadarPriorityType radarPriority; AsciiString commandSet; ModuleInfo modules; std::vector<WeaponTemplateSet> weaponSets;
	Int expValues[4]; Int expRequired[4]; Bool trainable; AudioEventRTS voiceDefect;
	ThingTemplate() : templateID(0), vision(0), shroudClearing(-1.0f), shroudRevealToAll(0), geom(GEOMETRY_SPHERE, false, 1.0f, 1.0f, 1.0f),
		energyProduction(0), energyBonus(0), buildCost(0), threatValue(0), crusherLevel(0), crushableLevel(0), transportSlots(0), enterGuard(false),
		occlusionDelay(0), radarPriority(RADAR_PRIORITY_INVALID), trainable(false), voiceDefect("VoiceDefect")
	{ for (int i = 0; i < 4; ++i) { expValues[i] = 0; expRequired[i] = 0; } }
	const ThingTemplate* getFinalOverride() const { return this; }
	const AsciiString& getName() const { return name; }
	Bool isKindOf(KindOfType t) const { return kindof.test(t); }
	Bool isKindOfMulti(const KindOfMaskType& a, const KindOfMaskType& b) const { return kindof.testForAll(a) && !kindof.testForAny(b); }
	Bool isAnyKindOf(const KindOfMaskType& a) const { return kindof.testForAny(a); }
	Real friend_calcVisionRange() const { return vision; }
	Real friend_calcShroudClearingRange() const { return shroudClearing; }
	Real getShroudRevealToAllRange() const { return shroudRevealToAll; }
	const GeometryInfo& getTemplateGeometryInfo() const { return geom; }
	const ModuleInfo& getBehaviorModuleInfo() const { return modules; }
	Bool canPossiblyHaveAnyWeapon() const { for (size_t i = 0; i < weaponSets.size(); ++i) if (weaponSets[i].m_hasWeapon) return true; return false; }
	Int getRawTransportSlotCount() const { return transportSlots; }
	UnsignedInt getOcclusionDelay() const { return occlusionDelay; }
	UnsignedByte getCrusherLevel() const { return (UnsignedByte)crusherLevel; }
	UnsignedByte getCrushableLevel() const { return (UnsignedByte)crushableLevel; }
	Int getEnergyProduction() const { return energyProduction; }
	Int getEnergyBonus() const { return energyBonus; }
	Int friend_getBuildCost() const { return buildCost; }
	Int getThreatValue() const { return threatValue; }
	Bool isEnterGuard() const { return enterGuard; }
	Int getExperienceValue(Int level) const { return expValues[level]; }
	Int getExperienceRequired(Int level) const { return expRequired[level]; }
	Bool isTrainable() const { return trainable; }
	RadarPriorityType getDefaultRadarPriority() const { return radarPriority; }
	const AsciiString& friend_getCommandSetString() const { return commandSet; }
	const AudioEventRTS* getVoiceDefect() const { return &voiceDefect; }
	UnsignedInt getTemplateID() const { return (UnsignedInt)templateID; }
	/* SparseMatchFinder::findBestInfoSlow over one condition per set (SparseMatchFinder.h) */
	const WeaponTemplateSet* findWeaponTemplateSet(const WeaponSetFlags& t) const
	{
		const WeaponTemplateSet* result = NULL;
		Int bestYesMatch = 0;
		Int bestYesExtraneousBits = 999;
		for (size_t idx = 0; idx < weaponSets.size(); ++idx)
		{
			const WeaponSetFlags& yesFlags = weaponSets[idx].m_types;
			Int yesMatch = (Int)(t.m_bits & yesFlags.m_bits).count();
			Int yesExtraneousBits = t.countInverseIntersection(yesFlags);
			if ((yesMatch > bestYesMatch) || (yesMatch >= bestYesMatch && yesExtraneousBits < bestYesExtraneousBits))
			{
				result = &weaponSets[idx];
				bestYesMatch = yesMatch;
				bestYesExtraneousBits = yesExtraneousBits;
			}
		}
		return result;
	}
};

/* ---- the special power template and the upgrade template ---------------------------------------------------- */
class SpecialPowerTemplate
{
public:
	int m_id; SpecialPowerType m_type; Bool m_shortcut; BehaviorModule* m_owner;
	SpecialPowerTemplate() : m_id(0), m_type(SPECIAL_INVALID), m_shortcut(false), m_owner(NULL) {}
	SpecialPowerType getSpecialPowerType() const;
	Bool isShortcutPower() const;
};
class UpgradeTemplate
{
public:
	UpgradeMaskType m_mask; AsciiString m_name;
	const UpgradeMaskType& getUpgradeMask() const { return m_mask; }
	const AsciiString& getUpgradeName() const { return m_name; }
};

/* ---- Thing, Experience (generated class texts) --------------------------------------------------------------- */
#include "thing_class.inc"
#include "experience_class.inc"


/* ---- DamageInfo (Damage.h, trimmed: no Snapshot) ----------------------------------------------------------- */
struct DamageInfoInput
{
	ObjectID m_sourceID; const ThingTemplate* m_sourceTemplate; PlayerMaskType m_sourcePlayerMask; DamageType m_damageType;
	ObjectStatusTypes m_damageStatusType; DamageType m_damageFXOverride; DeathType m_deathType; Real m_amount; Bool m_kill;
	Coord3D m_shockWaveVector; Real m_shockWaveAmount, m_shockWaveRadius, m_shockWaveTaperOff;
	DamageInfoInput()
	{
		m_sourceID = INVALID_ID; m_sourceTemplate = NULL; m_sourcePlayerMask = 0; m_damageType = DAMAGE_EXPLOSION;
		m_damageStatusType = OBJECT_STATUS_NONE; m_damageFXOverride = DAMAGE_UNRESISTABLE; m_deathType = DEATH_NORMAL;
		m_amount = 0; m_kill = FALSE; m_shockWaveVector.zero(); m_shockWaveAmount = 0.0f; m_shockWaveRadius = 0.0f; m_shockWaveTaperOff = 0.0f;
	}
};
struct DamageInfoOutput
{
	Real m_actualDamageDealt, m_actualDamageClipped; Bool m_noEffect;
	DamageInfoOutput() { m_actualDamageDealt = 0; m_actualDamageClipped = 0; m_noEffect = false; }
};
struct DamageInfo { DamageInfoInput in; DamageInfoOutput out; };

/* ---- the module classes -------------------------------------------------------------------------------------
** One class stands for BehaviorModule and every interface it hands out (BodyModuleInterface, DieModuleInterface,
** ...: the typedefs below). The module's answers are scripted (ProbeAnswers on the Rust side, ModuleProbe) and
** every call it receives is recorded by name. A helper (m_helper) answers nothing and records nothing: on the Rust
** side a helper has no body, and a call to it is `Unhandled`. */
typedef std::list<Object*> ContainedItemsList;
struct ProbeAnswers
{
	bool yes; unsigned has; bool salvage, subObjects, enclosing, garrisonable, kickOut, passengerMayFire, zeroSlot, heroInside;
	int specialPowerType; bool shortcut; unsigned powerTemplate; int rider; std::vector<int> contained; int slaver; float deckHeight;
	int apparentPlayer; int countermeasure; bool collideSetsNoCollisions;
	ProbeAnswers() : yes(false), has(0), salvage(false), subObjects(false), enclosing(false), garrisonable(false), kickOut(false),
		passengerMayFire(false), zeroSlot(false), heroInside(false), specialPowerType(0), shortcut(false), powerTemplate(0), rider(0),
		slaver(0), deckHeight(0.0f / 0.0f), apparentPlayer(-1), countermeasure(0), collideSetsNoCollisions(false) {}
};
enum { MODULEINTERFACE_UPDATE = 0x01, MODULEINTERFACE_DIE = 0x02, MODULEINTERFACE_DAMAGE = 0x04, MODULEINTERFACE_CREATE = 0x08,
	MODULEINTERFACE_COLLIDE = 0x10, MODULEINTERFACE_BODY = 0x20, MODULEINTERFACE_CONTAIN = 0x40, MODULEINTERFACE_UPGRADE = 0x80,
	MODULEINTERFACE_SPECIAL_POWER = 0x100, MODULEINTERFACE_DESTROY = 0x200 };

class Module : public MemoryPoolObject, public Snapshot
{
public:
	AsciiString m_className, m_tag; NameKeyType m_classKey, m_tagKey;
	NameKeyType getModuleNameKey() const { return m_classKey; }
	NameKeyType getModuleTagNameKey() const { return m_tagKey; }
	virtual void onDelete() {}
};
class UpgradeMaskTypeHolder;
class BehaviorModule : public Module
{
public:
	Object* m_object; Bool m_helper; Int m_helperKind; unsigned m_interfaces; Bool m_stealth, m_ai;
	ProbeAnswers a; mutable Real health, maxHealth, damageScalar; mutable Bool died; mutable ObjectID lastDamageSource; mutable UnsignedInt armorFlags; UpgradeMaskType upgradeTrigger;
	mutable Bool upgraded; mutable Bool running; mutable ContainedItemsList m_items; mutable UnsignedInt calls; mutable SpecialPowerTemplate spTemplate; mutable DamageInfo lastDamage;
	BehaviorModule() : m_object(NULL), m_helper(false), m_helperKind(0), m_interfaces(0), m_stealth(false), m_ai(false), health(0), maxHealth(0),
		damageScalar(1.0f), died(false), lastDamageSource(INVALID_ID), armorFlags(0), upgraded(false), running(false), calls(0) { spTemplate.m_owner = this; }
	/* one call is counted and recorded; a helper records nothing */
	BehaviorModule* self() const { return const_cast<BehaviorModule*>(this); }
	/* a call to a module whose body is running is not delivered (module_call.rs: the body is checked out while it runs) */
	bool probe(const char* name) const { if (m_helper || running) return false; calls = calls + 1; MODC(name); return true; }

	/* the interface getters: the interface mask of the slot, as the Rust ModuleSlot::interfaces */
	BehaviorModule* getBody() const { return (m_interfaces & MODULEINTERFACE_BODY) ? self() : NULL; }
	BehaviorModule* getContain() const { return (m_interfaces & MODULEINTERFACE_CONTAIN) ? self() : NULL; }
	BehaviorModule* getDie() const { return (m_interfaces & MODULEINTERFACE_DIE) ? self() : NULL; }
	BehaviorModule* getCreate() const { return (m_interfaces & MODULEINTERFACE_CREATE) ? self() : NULL; }
	BehaviorModule* getDestroy() const { return (m_interfaces & MODULEINTERFACE_DESTROY) ? self() : NULL; }
	BehaviorModule* getCollide() const { return (m_interfaces & MODULEINTERFACE_COLLIDE) ? self() : NULL; }
	BehaviorModule* getUpgrade() const { return (m_interfaces & MODULEINTERFACE_UPGRADE) ? self() : NULL; }
	BehaviorModule* getSpecialPower() const { return (m_interfaces & MODULEINTERFACE_SPECIAL_POWER) ? self() : NULL; }
	BehaviorModule* getStealth() const { return m_stealth ? self() : NULL; }
	BehaviorModule* getAIUpdateInterface() const { return m_ai ? self() : NULL; }
	/* the interfaces the Rust side asks with a Has... call */
	BehaviorModule* getUpdateExitInterface() const { return probe("HasUpdateExitInterface") && (a.has & (1u << 0)) ? self() : NULL; }
	BehaviorModule* getProductionUpdateInterface() const { return probe("HasProductionUpdateInterface") && (a.has & (1u << 1)) ? self() : NULL; }
	BehaviorModule* getDockUpdateInterface() const { return probe("HasDockUpdateInterface") && (a.has & (1u << 2)) ? self() : NULL; }
	BehaviorModule* getSpawnBehaviorInterface() const { return probe("HasSpawnBehaviorInterface") && (a.has & (1u << 3)) ? self() : NULL; }
	BehaviorModule* getProjectileUpdateInterface() const { return probe("HasProjectileUpdateInterface") && (a.has & (1u << 4)) ? self() : NULL; }
	BehaviorModule* getCountermeasuresBehaviorInterface() const { return probe("HasCountermeasuresInterface") && (a.has & (1u << 5)) ? self() : NULL; }
	BehaviorModule* getParkingPlaceBehaviorInterface() const { return probe("HasParkingPlaceInterface") && (a.has & (1u << 6)) ? self() : NULL; }
	BehaviorModule* getContainExitInterface() const { return probe("HasContainExitInterface") && (a.has & (1u << 7)) ? self() : NULL; }
	BehaviorModule* getSlavedUpdateInterface() const { return probe("GetSlaverId") && a.slaver != 0 ? self() : NULL; } /* the one GetSlaverId call: getSlaverID() below is silent */
	BehaviorModule* getSpecialPowerUpdateInterface() const { return self(); }

	/* BehaviorModule virtuals */
	void onObjectCreated() const { probe("OnObjectCreated"); }
	void onDrawableBoundToObject() const { probe("OnDrawableBoundToObject"); }
	virtual void onDelete() { probe("OnDelete"); }
	void onDisabledEdge(Bool b) const;
	void onCapture(Player* o, Player* n) const;
	/* Create, Destroy, Die, Collide */
	void onCreate() const { probe("OnCreate"); }
	void onDestroy() const { probe("OnDestroy"); }
	void onDie(DamageInfo* i) const;
	void onCollide(Object* other, const Coord3D* loc, const Coord3D* normal) const;
	Bool isSalvageCrateCollide() const { if (!probe("IsSalvageCrateCollide")) return false; return a.salvage; }
	/* Upgrade */
	Bool isAlreadyUpgraded() const { if (!probe("IsAlreadyUpgraded")) return false; return upgraded; }
	Bool attemptUpgrade(UpgradeMaskType m) const { if (!probe("AttemptUpgrade")) return false; if (m.testForAny(upgradeTrigger)) { upgraded = true; return true; } return false; }
	Bool wouldUpgrade(UpgradeMaskType m) const { if (!probe("WouldUpgrade")) return false; return !upgraded && m.testForAny(upgradeTrigger); }
	Bool resetUpgrade(UpgradeMaskType m) const { if (!probe("ResetUpgrade")) return false; if (m.testForAny(upgradeTrigger)) { upgraded = false; return true; } return false; }
	Bool isSubObjectsUpgrade() const { if (!probe("IsSubObjectsUpgrade")) return false; return a.subObjects; }
	void forceRefreshUpgrade() const { probe("ForceRefreshUpgrade"); }
	/* SpecialPower */
	void pauseCountdown(Bool d) const { probe("PauseCountdown"); }
	const SpecialPowerTemplate* getSpecialPowerTemplate() const { probe("GetSpecialPowerType"); return a.specialPowerType != 0 ? &spTemplate : NULL; }
	Bool isModuleForPower(const SpecialPowerTemplate* t) const { if (!probe("IsModuleForPower")) return false; return a.powerTemplate == (unsigned)t->m_id; }
	void doSpecialPower(UnsignedInt o) const { probe("DoSpecialPower"); }
	void doSpecialPowerAtObject(Object* obj, UnsignedInt o) const { probe("DoSpecialPowerAtObject"); }
	void doSpecialPowerAtLocation(const Coord3D* loc, Real angle, UnsignedInt o) const { probe("DoSpecialPowerAtLocation"); }
	void doSpecialPowerUsingWaypoints(const Waypoint* w, UnsignedInt o) const { probe("DoSpecialPowerUsingWaypoints"); }
	/* Body */
	void attemptDamage(DamageInfo* info) const;
	void attemptHealing(DamageInfo* info) const;
	Real estimateDamage(DamageInfoInput& in) const { if (!probe("EstimateDamage")) return 0.0f; return in.m_amount * damageScalar; }
	Real getMaxHealth() const { if (!probe("GetMaxHealth")) return 0.0f; return maxHealth; }
	Real getHealth() const { if (!probe("GetHealth")) return 0.0f; return health; }
	Real getDamageScalar() const { if (!probe("GetDamageScalar")) return 0.0f; return damageScalar; }
	const DamageInfo* getLastDamageInfo() const { probe("GetLastDamageSource"); lastDamage.in.m_sourceID = lastDamageSource; return &lastDamage; }
	void setArmorSetFlag(ArmorSetType f) const { if (!probe("SetArmorSetFlag")) return; armorFlags |= 1u << f; }
	void clearArmorSetFlag(ArmorSetType f) const { if (!probe("ClearArmorSetFlag")) return; armorFlags &= ~(1u << f); }
	Bool testArmorSetFlag(ArmorSetType f) const { if (!probe("TestArmorSetFlag")) return false; return (armorFlags & (1u << f)) != 0; }
	void onVeterancyLevelChanged(VeterancyLevel o, VeterancyLevel n, Bool b) const { probe("OnVeterancyLevelChanged"); }
	/* Contain */
	void removeFromContain(Object* o) const { probe("RemoveFromContain"); }
	Bool isEnclosingContainerFor(const Object* o) const { if (!probe("IsEnclosingContainerFor")) return false; return a.enclosing; }
	Bool isSpecialZeroSlotContainer() const { if (!probe("IsSpecialZeroSlotContainer")) return false; return a.zeroSlot; }
	const ContainedItemsList* getContainedItemsList() const;
	Bool isPassengerAllowedToFire(ObjectID id) const { if (!probe("IsPassengerAllowedToFire")) return false; return a.passengerMayFire; }
	Int getContainCount() const { if (!probe("GetContainCount")) return 0; return (Int)a.contained.size(); }
	void containReactToTransformChange() const { probe("ContainReactToTransformChange"); }
	Bool isGarrisonable() const { if (!probe("IsGarrisonable")) return false; return a.garrisonable; }
	Bool isKickOutOnCapture() const { if (!probe("IsKickOutOnCapture")) return false; return a.kickOut; }
	void removeAllContained(Bool b) const { probe("RemoveAllContained"); }
	const Object* friend_getRider() const;
	void iterateContained(void (*func)(Object*, void*), void* userData, Bool reverse) const;
	Player* getApparentControllingPlayer(const Player* observer) const;
	/* the others */
	void orderSlavesDisabledUntil(DisabledType t, UnsignedInt f) const { probe("OrderSlavesDisabledUntil"); }
	void orderSlavesToClearDisabled(DisabledType t) const { probe("OrderSlavesToClearDisabled"); }
	void onSpawnDeath(ObjectID dead, DamageInfo* i) const { probe("OnSpawnDeath"); }
	Bool canAnySlavesAttack() const { if (!probe("CanAnySlavesAttack")) return false; return a.yes; }
	Bool getIsDisableProof() const { if (!probe("GetRadarDisableProof")) return false; return a.yes; }
	Bool isOverchargeActive() const { if (!probe("IsOverchargeActive")) return false; return a.yes; }
	void undoUpgrade() const { probe("UndoUpgrade"); }
	Bool isAbleToBeToppled() const { if (!probe("IsAbleToBeToppled")) return false; return a.yes; }
	void applyTopplingForce(const Coord3D* d, Real s, UnsignedInt o) const { probe("ApplyTopplingForce"); }
	void detonate() const { probe("Detonate"); }
	void startDefectionTimer(UnsignedInt t, Bool withFx = TRUE) const { probe("StartDefectionTimer"); }
	void cancelAndRefundAllProduction() const { probe("CancelAndRefundAllProduction"); }
	void startRebuildProcess(const ThingTemplate* t, ObjectID spawner) const { probe("StartRebuildProcess"); }
	ObjectID getSlaverID() const { return (ObjectID)a.slaver; }
	Bool isActive() const { if (!probe("CountermeasuresIsActive")) return false; return a.yes; }
	void reportMissileForCountermeasures(Object* m) const { probe("ReportMissileForCountermeasures"); }
	Bool doesSpecialPowerHaveOverridableDestinationActive() const { if (!probe("DoesSpecialPowerHaveOverridableDestinationActive")) return false; return (a.has & (1u << 8)) && a.yes; }
	Bool doesSpecialPowerHaveOverridableDestination() const { if (!probe("DoesSpecialPowerHaveOverridableDestination")) return false; return (a.has & (1u << 8)) && a.yes; }
	Bool isSpecialAbility() const { if (!probe("GetSpecialAbilityType")) return false; return (a.has & (1u << 9)) != 0; }
	SpecialPowerType getSpecialPowerType() const { return (SpecialPowerType)a.specialPowerType; }
	Real getLandingDeckHeightOffset() const { if (!probe("GetLandingDeckHeightOffset")) return 0.0f; return a.deckHeight; }
	void defectAllParkedUnits(Team* t, UnsignedInt d) const;
	ObjectID calculateCountermeasureToDivertTo(const Object& v) const { if (!probe("CalculateCountermeasureToDivertTo")) return INVALID_ID; return (ObjectID)a.countermeasure; }
	/* AIUpdateInterface, DozerAIInterface, Locomotor */
	void setAttitude(Int a) const;
	void setAttackInfo(const AttackPriorityInfo* i) const;
	void aiIdle(CommandSourceType c) const;
	void transferAttack(ObjectID oldVictim, ObjectID newVictim) const;
	void getTurretRotAndPitch(WhichTurretType t, Real* rot, Real* pitch) const;
	WhichTurretType getWhichTurretForWeaponSlot(WeaponSlotType s, Real* dummy) const;
	Bool isTurretEnabled(WhichTurretType t) const;
	BehaviorModule* getCurLocomotor() const;
	BehaviorModule* getDozerAIInterface() const;
	Int getCurrentTask() const { return 0; }
	void cancelTask(Int t) const;
	UnsignedInt getLegalSurfaces() const;
	/* PhysicsBehavior */
	void applyShock(const Coord3D* f) const { probe("PhysicsApplyShock"); }
	void applyRandomRotation() const { probe("PhysicsApplyRandomRotation"); }
	void setStunned(Bool b) const { probe("PhysicsSetStunned"); }
	/* the helpers */
	void sleepUntil(UnsignedInt when) const;
	void notifySubdualDamage(Real amount) const { probe("NotifySubdualDamage"); }
	void doStatusDamage(ObjectStatusTypes s, Real d) const { probe("DoStatusDamage"); }
	void doTempWeaponBonus(WeaponBonusConditionType s, UnsignedInt d) const { probe("DoTempWeaponBonus"); }

	/* Snapshot: ProbeModule's xfer (module_probe.rs) */
	void crc(Xfer* x) { if (m_helper) return; x->xferReal(&health); }
	void xfer(Xfer* x)
	{
		if (m_helper) return;
		XferVersion cur = 1, v = 1;
		x->xferVersion(&v, cur);
		x->xferReal(&health); x->xferReal(&maxHealth); x->xferReal(&damageScalar); x->xferBool(&died);
		x->xferObjectID(&lastDamageSource); x->xferUnsignedInt(&armorFlags); x->xferBool(&upgraded); x->xferUnsignedInt(&calls);
	}
	void loadPostProcess() {}
};
/* the helper-only entry points that the Object code names by their own class */
typedef BehaviorModule UpdateModule;
typedef BehaviorModule BodyModuleInterface;
typedef BehaviorModule ContainModuleInterface;
typedef BehaviorModule DieModuleInterface;
typedef BehaviorModule CreateModuleInterface;
typedef BehaviorModule DestroyModuleInterface;
typedef BehaviorModule CollideModuleInterface;
typedef BehaviorModule UpgradeModuleInterface;
typedef BehaviorModule SpecialPowerModuleInterface;
typedef BehaviorModule SpecialPowerUpdateInterface;
typedef BehaviorModule SpecialAbilityUpdate;
typedef BehaviorModule SpecialPowerCompletionDie;
typedef BehaviorModule SpawnBehaviorInterface;
typedef BehaviorModule ProjectileUpdateInterface;
typedef BehaviorModule ProductionUpdateInterface;
typedef BehaviorModule DockUpdateInterface;
typedef BehaviorModule ExitInterface;
typedef BehaviorModule SlavedUpdateInterface;
typedef BehaviorModule CountermeasuresBehaviorInterface;
typedef BehaviorModule ParkingPlaceBehaviorInterface;
typedef BehaviorModule PhysicsBehavior;
typedef BehaviorModule AIUpdateInterface;
typedef BehaviorModule DozerAIInterface;
typedef BehaviorModule Locomotor;
typedef BehaviorModule DamageModule;
typedef BehaviorModule RadarUpgrade;
typedef BehaviorModule PowerPlantUpgrade;
typedef BehaviorModule OverchargeBehavior;
typedef BehaviorModule AutoHealBehavior;
typedef BehaviorModule ToppleUpdate_;
typedef BehaviorModule RebuildHoleBehaviorInterface;

/* the helpers: modules with a name, a tag and no behaviour (module data: the tag key) */
struct HelperModuleData { NameKeyType tag; void setModuleTagNameKey(NameKeyType k) { tag = k; } };
#define HELPER_CLASS(C, KIND, NAME) \
	typedef HelperModuleData C##ModuleData; \
	class C : public BehaviorModule { public: \
		C(Object* o, const HelperModuleData* d) { m_object = o; m_helper = true; m_helperKind = KIND; m_classKey = NAMEKEY(NAME); m_tagKey = d->tag; } };
HELPER_CLASS(ObjectSMCHelper, 0, "ObjectSMCHelper")
HELPER_CLASS(StatusDamageHelper, 1, "StatusDamageHelper")
HELPER_CLASS(SubdualDamageHelper, 2, "SubdualDamageHelper")
HELPER_CLASS(ObjectRepulsorHelper, 3, "ObjectRepulsorHelper")
HELPER_CLASS(ObjectDefectionHelper, 4, "ObjectDefectionHelper")
HELPER_CLASS(ObjectWeaponStatusHelper, 5, "ObjectWeaponStatusHelper")
HELPER_CLASS(FiringTracker, 6, "FiringTracker")
HELPER_CLASS(TempWeaponBonusHelper, 7, "TempWeaponBonusHelper")
#define ToppleUpdate BehaviorModule
#define StickyBombUpdate BehaviorModule

#endif
