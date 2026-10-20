/* The stand-ins of the world that Team.cpp and Player.cpp reach, part 1: the declarations that Team.h needs before the real
** Team classes (p1w9-sonnet, extended by p1w10-sonnet for Player). Included through the stand-in GameLogic/Object.h, with every class below renamed by the
** #define lines of differential/wrap_team.cpp (TmTest*), so that no symbol of this archive collides with another
** fixture's. The out-of-line members, which need the real Team classes, are in team_standins.h.
**
** Method: the classes answer from the program's configuration (members set by the driver from the program words) and
** every call that has an effect writes a line to g_lines, formatted exactly as the Rust host (tests/differential_team.rs)
** writes it. Calls that only answer a question write nothing. Reals are written as their bit patterns. */
#ifndef PLAYER_STANDINS_DECL_H
#define PLAYER_STANDINS_DECL_H

#include <vector>
#include <string>
#include <list>
#include <map>
#include <set>
#include <algorithm>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "Common/NameKeyGenerator.h"
#include "Common/Errors.h"
#include "Common/Money.h"
#include "Common/Handicap.h"
#include "Common/Science.h"
#include "Common/Upgrade.h"
#include "GameLogic/ObjectScriptStatusBits.h"
#include "Common/AudioEventRTS.h"
#include "GameClient/Color.h"

/* slice 2: forward declarations and the two enums the partition manager's members use */
class SupplyTruckAIInterface; class ObjectIterator; class BuildAssistant; class Radar; class GameClient; class PlayerTemplateStore;
enum ValueOrThreat { VOT_CashValue = 1, VOT_ThreatValue, VOT_NumItems };
enum IterOrderType { ITER_FASTEST = 0, ITER_SORTED_NEAR_TO_FAR, ITER_SORTED_FAR_TO_NEAR };
/* PlayerList.h lines 63-69, verbatim */
enum AllowPlayerRelationship
{
	ALLOW_SAME_PLAYER			= 0x01,
	ALLOW_ALLIES					= 0x02,
	ALLOW_ENEMIES					= 0x04,
	ALLOW_NEUTRAL					= 0x08
};
/* ActionManager.h lines 51-56, verbatim */
enum CanEnterType { CHECK_CAPACITY, DONT_CHECK_CAPACITY, COMBATDROP_INTO };
/* GameMemory.h lines 772-781 without the debug assertion */
class MemoryPoolObjectHolder
{
private:
	MemoryPoolObject *m_mpo;
public:
	MemoryPoolObjectHolder(MemoryPoolObject *mpo = NULL) : m_mpo(mpo) { }
	void hold(MemoryPoolObject *mpo) { m_mpo = mpo; }
	void release() { m_mpo = NULL; }
	~MemoryPoolObjectHolder() { m_mpo->deleteInstance(); }
};
class Pathfinder { public: void removeObjectFromPathfindMap(Object* obj); void addObjectToPathfindMap(Object* obj); };
class ActionManager { public: Bool canEnterObject(const Object* obj, const Object* objectToEnter, CommandSourceType commandSource, CanEnterType modifierFlags) const; };
extern ActionManager* TheActionManager;

/* Common/GameState.h's SaveCode, verbatim from upstream (GameState.h lines 129-138): Team.cpp throws SC_INVALID_DATA */
enum SaveCode
{
	SC_INVALID = -1,
	SC_OK,
	SC_NO_FILE_AVAILABLE,
	SC_FILE_NOT_FOUND,
	SC_UNABLE_TO_OPEN_FILE,
	SC_INVALID_XFER,
	SC_UNKNOWN_BLOCK,
	SC_INVALID_DATA,
	SC_ERROR,
};

/* ---- the trace ---------------------------------------------------------------------------------------------- */
static std::vector<std::string>* g_lines = NULL;
static unsigned fbits(float f) { unsigned u; memcpy(&u, &f, 4); return u; }
static void Rec(const char* fmt, ...)
{
	char buf[2048];
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(buf, sizeof buf, fmt, ap);
	va_end(ap);
	if (g_lines) g_lines->push_back(buf);
	static const bool trace = getenv("TEAM_TRACE") != NULL;
	if (trace) fprintf(stderr, "%s\n", buf);
}
#define FB(x) fbits(x)

class Team; class TeamPrototype; class Object; class Player; class ThingTemplate; class PolygonTrigger; class Xfer;

/* ---- the enums of the headers the real Team.cpp includes that are not the real ones here ------------------------ */
enum AttitudeType { AI_SLEEP = -2, AI_PASSIVE = -1, AI_NORMAL = 0, AI_ALERT = 1, AI_AGGRESSIVE = 2, AI_INVALID = 3 };
typedef UnsignedInt LocomotorSurfaceTypeMask;
enum LocomotorSurfaceType
{
	LOCOMOTORSURFACE_GROUND = (1 << 0),
	LOCOMOTORSURFACE_WATER = (1 << 1),
	LOCOMOTORSURFACE_CLIFF = (1 << 2),
	LOCOMOTORSURFACE_AIR = (1 << 3),
	LOCOMOTORSURFACE_RUBBLE = (1 << 4)
};
enum DistanceCalculationType { FROM_CENTER_2D = 0, FROM_CENTER_3D = 1, FROM_BOUNDINGSPHERE_2D = 2, FROM_BOUNDINGSPHERE_3D = 3 };

/* ---- the dictionary (Dict is another unit, verified by differential_dict.rs: this stand-in has its getters' meaning --
** a value of another type, or none, reads as "does not exist" and gives 0, false or the empty string) ------------- */
class Dict
{
public:
	struct Val { int type; unsigned bits; AsciiString s; };
	std::map<unsigned, Val> m;
	void set(NameKeyType k, int type, unsigned bits, const AsciiString& s) { Val v; v.type = type; v.bits = bits; v.s = s; m[(unsigned)k] = v; }
	void setBool(NameKeyType k, Bool v) { set(k, 0, v ? 1 : 0, AsciiString()); }
	void setInt(NameKeyType k, Int v) { set(k, 1, (unsigned)v, AsciiString()); }
	void setReal(NameKeyType k, Real v) { unsigned b; memcpy(&b, &v, 4); set(k, 2, b, AsciiString()); }
	void setAsciiString(NameKeyType k, const AsciiString& v) { set(k, 3, 0, v); }
	const Val* find(NameKeyType k, int type) const { std::map<unsigned, Val>::const_iterator it = m.find((unsigned)k); return (it != m.end() && it->second.type == type) ? &it->second : NULL; }
	Bool getBool(NameKeyType k, Bool* exists = NULL) const { const Val* v = find(k, 0); if (exists) *exists = v != NULL; return v ? v->bits != 0 : false; }
	Int getInt(NameKeyType k, Bool* exists = NULL) const { const Val* v = find(k, 1); if (exists) *exists = v != NULL; return v ? (Int)v->bits : 0; }
	Real getReal(NameKeyType k, Bool* exists = NULL) const { const Val* v = find(k, 2); if (exists) *exists = v != NULL; Real r = 0; if (v) memcpy(&r, &v->bits, 4); return r; }
	AsciiString getAsciiString(NameKeyType k, Bool* exists = NULL) const { const Val* v = find(k, 3); if (exists) *exists = v != NULL; return v ? v->s : AsciiString(); }
};

/* ---- the thing templates ---------------------------------------------------------------------------------------- */
class ProductionPrerequisite;
class ThingTemplate
{
public:
	int m_idx; Int m_skill[4];
	Int getSkillPointValue(Int level) const { return m_skill[level & 3]; }
	KindOfMaskType m_kindof;
	Int m_buildCost; BuildableStatus m_buildable; Int m_nPrereq; unsigned m_prereqMask; UnsignedInt m_maxSimul; NameKeyType m_linkKey; AsciiString m_owningSide;
	Int getBuildCost() const { return m_buildCost; }
	BuildableStatus getBuildable() const { return m_buildable; }
	Int getPrereqCount() const { return m_nPrereq; }
	const ProductionPrerequisite* getNthPrereq(Int i) const;
	UnsignedInt getMaxSimultaneousOfType() const { return m_maxSimul; }
	NameKeyType getMaxSimultaneousLinkKey() const { return m_linkKey; }
	AsciiString getDefaultOwningSide() const { return m_owningSide; }
	Int calcCostToBuild(const Player* player) const;
	bool m_buildFacility;
	int m_equiv;
	AsciiString m_name;
	std::vector<AsciiString> m_variations;
	Bool isEquivalentTo(const ThingTemplate* tt) const { if (!tt) return false; if (this == tt) return true; return m_equiv == tt->m_equiv; }
	Bool isKindOf(KindOfType t) const { return m_kindof.test(t); }
	Bool isKindOfMulti(const KindOfMaskType& mustBeSet, const KindOfMaskType& mustBeClear) const { return m_kindof.testSetAndClear(mustBeSet, mustBeClear); }
	Bool isBuildFacility() const { return m_buildFacility; }
	const AsciiString& getName() const { return m_name; }
	const std::vector<AsciiString>& getBuildVariations() const { return m_variations; }
};

class ThingFactory
{
public:
	std::vector<ThingTemplate*> m_templates;
	ThingTemplate* findTemplate(const AsciiString& name, Bool check = TRUE) const
	{
		for (size_t i = 0; i < m_templates.size(); ++i) if (m_templates[i]->m_name == name) return m_templates[i];
		return NULL;
	}
};

/* ---- the AI, the contain module, the groups, the triggers ------------------------------------------------------- */
class LocomotorSet
{
public:
	LocomotorSurfaceTypeMask m_valid;
	LocomotorSurfaceTypeMask getValidSurfaces() const { return m_valid; }
};
class AIUpdateInterface
{
public:
	Object* m_owner; bool m_idle, m_recruitable; LocomotorSet m_loco;
	Bool isIdle() const { return m_idle; }
	Bool isRecruitable() const { return m_recruitable; }
	const LocomotorSet& getLocomotorSet() const { return m_loco; }
	void aiHunt(CommandSourceType source);
	void aiIdle(CommandSourceType source);
	void aiEnter(Object* obj, CommandSourceType source);
	void aiEvacuate(Bool exposeStealthUnits, CommandSourceType source);
	void aiMoveToPosition(const Coord3D* pos, CommandSourceType source);
	SupplyTruckAIInterface* getSupplyTruckAIInterface() const;
};
class ContainModuleInterface
{
public:
	Object* m_owner; int m_count;
	Int getContainCount() const { return m_count; }
	void removeAllContained(Bool exposeStealthUnits = FALSE);
	unsigned m_whoEntered;
	void recalcApparentControllingPlayer();
	PlayerMaskType getPlayerWhoEntered() const { return (PlayerMaskType)m_whoEntered; }
};
class AIGroup
{
public:
	int m_id; std::vector<ObjectID> m_ids;
	void add(Object* obj);
	const std::vector<ObjectID>& getAllIDs() const { return m_ids; }
};
class PolygonTrigger { public: int m_id; };

/* ---- the terrain ------------------------------------------------------------------------------------------------- */
class Waypoint
{
public:
	UnsignedInt m_id; AsciiString m_name; Coord3D m_loc; Waypoint* m_next;
	Waypoint* getNext() const { return m_next; }
	const AsciiString& getName() const { return m_name; }
	const Coord3D* getLocation() const { return &m_loc; }
	UnsignedInt getID() const { return m_id; }
};
class TerrainLogic
{
public:
	std::vector<Waypoint*> m_ways;
	Waypoint* getFirstWaypoint() const { return m_ways.empty() ? NULL : m_ways[0]; }
	Waypoint* getWaypointByID(UnsignedInt id) const
	{
		for (size_t i = 0; i < m_ways.size(); ++i) if (m_ways[i]->m_id == id) return m_ways[i];
		return NULL;
	}
};

/* ---- the script engine -------------------------------------------------------------------------------------------- */
class ScriptAction { public: int m_serial; };
class Script
{
public:
	int m_serial; AsciiString m_name; UnsignedInt m_frameToEvaluate; Int m_delay; bool m_easy, m_normal, m_hard, m_oneShot;
	unsigned m_pattern; unsigned m_calls; ScriptAction m_action;
	Script* duplicate() const;
	void deleteInstance() { Rec("script.delete %d", m_serial); }
	UnsignedInt getFrameToEvaluate() const { return m_frameToEvaluate; }
	void setFrameToEvaluate(UnsignedInt f) { Rec("script.frame %d %u", m_serial, (unsigned)f); m_frameToEvaluate = f; }
	Int getDelayEvalSeconds() const { return m_delay; }
	Bool isEasy() const { return m_easy; }
	Bool isNormal() const { return m_normal; }
	Bool isHard() const { return m_hard; }
	Bool isOneShot() const { return m_oneShot; }
	AsciiString getName() const { return m_name; }
	ScriptAction* getAction() const { return const_cast<ScriptAction*>(&m_action); }
};
class ScriptEngine
{
public:
	std::vector<Script*> m_scripts;
	int m_nextSerial;
	const Script* findScriptByName(const AsciiString& name) const
	{
		for (size_t i = 0; i < m_scripts.size(); ++i) if (m_scripts[i]->m_name == name) return m_scripts[i];
		return NULL;
	}
	void friend_executeAction(ScriptAction* action, Team* team = NULL);
	Bool evaluateConditions(Script* script, Team* team = NULL, Player* player = NULL);
	void runScript(const AsciiString& name, Team* team);
	void notifyOfTeamDestruction(Team* team);
	void AppendDebugMessage(const AsciiString& msg, Bool forcePause = TRUE) { Rec("dbg %s %d", msg.str(), forcePause ? 1 : 0); }
	GameDifficulty m_globalDifficulty;
	GameDifficulty getGlobalDifficulty() const { return m_globalDifficulty; }
	void notifyOfObjectCreationOrDestruction();
	void notifyOfAcquiredScience(Int playerIndex, ScienceType science);
};

/* ---- players: the template and the list (the real Player, Player.h unchanged, is defined after Team.h) --------------- */
class PtColor { public: Int m_asInt; Int getAsInt() const { return m_asInt; } };
class PlayerTemplate
{
public:
	AsciiString m_beacon, m_side, m_baseSide; PtColor m_color; Money m_money; Handicap m_handicap;
	ProductionChangeMap m_cost, m_time; ProductionVeterancyMap m_vet; bool m_observer, m_playable; ScienceVec m_intrinsic; Int m_intrinsicSPP;
	AsciiString getBeaconTemplate() const { return m_beacon; }
	AsciiString getSide() const { return m_side; }
	AsciiString getBaseSide() const { return m_baseSide; }
	const PtColor* getPreferredColor() const { return &m_color; }
	const Money* getMoney() const { return &m_money; }
	const Handicap* getHandicap() const { return &m_handicap; }
	const ProductionChangeMap& getProductionCostChanges() const { return m_cost; }
	const ProductionChangeMap& getProductionTimeChanges() const { return m_time; }
	const ProductionVeterancyMap& getProductionVeterancyLevels() const { return m_vet; }
	Bool isObserver() const { return m_observer; }
	Bool isPlayableSide() const { return m_playable; }
	const ScienceVec& getIntrinsicSciences() const { return m_intrinsic; }
	Int getIntrinsicSciencePurchasePoints() const { return m_intrinsicSPP; }
};
class PlayerList
{
public:
	Player* m_players[16]; int m_count; Player* m_local; std::map<unsigned, int> m_byKey;
	Int getPlayerCount() const { return m_count; }
	Player* getNthPlayer(Int i) const { return (i >= 0 && i < 16) ? m_players[i] : NULL; }
	Player* getNeutralPlayer() const { return m_players[0]; }
	Player* getLocalPlayer() const { return m_local; }
	Player* findPlayerWithNameKey(NameKeyType key) const
	{
		std::map<unsigned, int>::const_iterator it = m_byKey.find((unsigned)key);
		return (it == m_byKey.end() || it->second >= m_count) ? NULL : m_players[it->second];
	}
	void teamAboutToBeDeleted(Team* team);
};

/* ---- the global data (the fields Player.cpp reads), game info, the message stream ------------------------------- */
class GlobalData
{
public:
	Bool m_clientRetaliationModeEnabled; UnsignedInt m_baseValuePerSupplyBox; Real m_soloPlayerHealthBonusForDifficulty[PLAYERTYPE_COUNT][DIFFICULTY_COUNT];
	Money m_defaultStartingCash; TimeOfDay m_timeOfDay;
};
class GameInfo { public: Money m_cash; Money getStartingCash() const { return m_cash; } };
union GameMessageArgumentType { Int integer; Bool boolean; ObjectID objectID; };
class GameMessageArgument { public: GameMessageArgumentType m_u; };
class GameMessage
{
public:
	enum Type { MSG_ENABLE_RETALIATION_MODE = 1 };
	std::vector<GameMessageArgument> m_args;
	void appendIntegerArgument(Int a);
	void appendBooleanArgument(Bool a);
	UnsignedByte getArgumentCount() const { return (UnsignedByte)m_args.size(); }
	const GameMessageArgumentType* getArgument(Int i) const { return &m_args[i].m_u; }
};
class MessageStream { public: GameMessage* appendMessage(GameMessage::Type t); };

/* ---- the AI --------------------------------------------------------------------------------------------------------- */
class SpecialPowerTemplate;
class AIPlayer : public MemoryPoolObject, public Snapshot
{
public:
	Player* m_player; bool m_skirmish; GameDifficulty m_diff; Player* m_enemy; bool m_supplySafe, m_supplyAttacked; bool m_checkBridges; bool m_hasBase; Coord3D m_base; bool m_superOK; bool m_construct; Coord3D m_constructAt;
	AIPlayer(Player* p);
	virtual ~AIPlayer();
	void update();
	void newMap();
	void setAIDifficulty(GameDifficulty d);
	GameDifficulty getAIDifficulty() const { return m_diff; }
	Bool isSkirmishAI() const { return m_skirmish; }
	Bool computeSuperweaponTarget(const SpecialPowerTemplate* power, Coord3D* pos, Int playerNdx, Real radius);
	Player* getAiEnemy() const { return m_enemy; }
	Bool checkBridges(Object* unit, Waypoint* way);
	Bool getBaseCenter(Coord3D* pos);
	void repairStructure(ObjectID id);
	void onUnitProduced(Object* factory, Object* unit);
	void onStructureProduced(Object* builder, Object* structure);
	Bool isSupplySourceSafe(Int minSupplies);
	Bool isSupplySourceAttacked();
	void setTeamDelaySeconds(Int delay);
	void guardSupplyCenter(Team* team, Int minSupplies);
	void aiPreTeamDestroy(const Team* team);
	void selectSkillset(Int skillSet);
	void buildSpecificAITeam(TeamPrototype* proto, Bool priority);
	void buildAIBaseDefense(Bool flank);
	void buildAIBaseDefenseStructure(const AsciiString& name, Bool flank);
	void buildSpecificAIBuilding(const AsciiString& name);
	void buildBySupplies(Int cash, const AsciiString& name);
	void buildSpecificBuildingNearestTeam(const AsciiString& name, const Team* team);
	void buildUpgrade(const AsciiString& name);
	void recruitSpecificAITeam(TeamPrototype* proto, Real radius);
	Bool calcClosestConstructionZoneLocation(const ThingTemplate* t, Coord3D* loc);
protected:
	virtual void crc(Xfer*) {}
	/* the AI is another unit: this stand-in saves a version, as the Rust stand-in does */
	virtual void xfer(Xfer* xfer) { XferVersion currentVersion = 1; XferVersion version = currentVersion; xfer->xferVersion(&version, currentVersion); }
	virtual void loadPostProcess(void) {}
};
class AISkirmishPlayer : public AIPlayer { public: AISkirmishPlayer(Player* p) : AIPlayer(p) { m_skirmish = true; } };
class AIData { public: Bool m_forceSkirmishAI; };
class AI { public: AIData m_data; Pathfinder m_pf; AIData* getAiData() { return &m_data; } Pathfinder* pathfinder() { return &m_pf; } };

/* ---- squads and the build list ------------------------------------------------------------------------------------- */
/* the REAL Squad.h (the class, unchanged; its functions are Squad.cpp's, taken whole by build.rs) */
#include "GameLogic/Squad.h"
/* BuildListInfo (SidesList.h lines 150-250): the members of the real class, in the real order, with the accessors the player code and the
** driver use; its constructor, crc, xfer and loadPostProcess are SidesList.cpp's, taken whole by build.rs. The rest of the class is the
** editor's and the map's (parseStructure, duplicate, ...) and is not reached from Player. */
class BuildListInfo : public MemoryPoolObject, public Snapshot
{
public:
	enum {UNLIMITED_REBUILDS=0xFFFFFFFF};
	enum {MAX_RESOURCE_GATHERERS = 10};
	BuildListInfo(void);
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );
	AsciiString			m_buildingName;
	AsciiString			m_templateName;
	Coord3D					m_location;
	Coord2D					m_rallyPointOffset;
	Real						m_angle;
	Bool						m_isInitiallyBuilt;
	UnsignedInt			m_numRebuilds;
	BuildListInfo*	m_nextBuildList;
	AsciiString		m_script;
	Int				m_health;
	Bool			m_whiner;
	Bool			m_unsellable;
	Bool			m_repairable;
	Bool			m_automaticallyBuild;
	void*	m_renderObj;
	void*	m_shadowObj;
	Bool				m_selected;
	ObjectID m_objectID;
	UnsignedInt m_objectTimestamp;
	Bool		 m_underConstruction;
	ObjectID m_resourceGatherers[MAX_RESOURCE_GATHERERS];
	Bool		 m_isSupplyBuilding;
	Int			 m_desiredGatherers;
	Int			 m_currentGatherers;
	Bool		 m_priorityBuild;
	void setNumRebuilds(UnsignedInt numRebuilds) {m_numRebuilds = numRebuilds;}
	void setNextBuildList(BuildListInfo *pNext) {m_nextBuildList = pNext;}
	void setLocation(Coord3D loc) {m_location = loc;}
	void setAngle(Real angle) {m_angle = angle;}
	BuildListInfo *getNext(void) const {return m_nextBuildList;}
	void setTemplateName(AsciiString name) {m_templateName = name;}
	void setObjectID( ObjectID objID ) { m_objectID = objID; }
	void markPriorityBuild(void) {m_priorityBuild = true; }
};

/* ---- the client and the other services ------------------------------------------------------------------------------ */
enum EvaMessage { EVA_GeneralLevelUp = 1, EVA_SuperweaponDetected_Own_ParticleCannon, EVA_SuperweaponDetected_Ally_ParticleCannon, EVA_SuperweaponDetected_Enemy_ParticleCannon, EVA_SuperweaponDetected_Own_Nuke, EVA_SuperweaponDetected_Ally_Nuke, EVA_SuperweaponDetected_Enemy_Nuke, EVA_SuperweaponDetected_Own_ScudStorm, EVA_SuperweaponDetected_Ally_ScudStorm, EVA_SuperweaponDetected_Enemy_ScudStorm };
class Eva { public: void setShouldPlay(EvaMessage m); };
class ControlBar { public: void markUIDirty(); void onPlayerSciencePurchasePointsChanged(const Player* p); void onPlayerRankChanged(const Player* p); void setControlBarSchemeByPlayer(Player* p); void setControlBarSchemeByPlayerTemplate(const PlayerTemplate* pt); };
class InGameUI { public: Bool m_quiet; Bool isClientQuiet() const { return m_quiet; } void addIdleWorker(Object* obj); void removeIdleWorker(Object* obj, Int playerNumber); void addFloatingText(const UnicodeString& text, const Coord3D* pos, Color color); };
class GameText { public: UnicodeString fetch(const char* label); };
class VictoryConditions { public: bool m_defeated[16]; Bool hasSinglePlayerBeenDefeated(const Player* p); };
class MiscAudio { public: AudioEventRTS m_radarOnlineSound, m_radarOfflineSound, m_moneyDepositSound, m_moneyWithdrawSound; MiscAudio() : m_radarOnlineSound(3), m_radarOfflineSound(4), m_moneyDepositSound(1), m_moneyWithdrawSound(2) {} };
class AudioManager { public: MiscAudio m_misc; MiscAudio* getMiscAudio() { return &m_misc; } void addAudioEvent(const AudioEventRTS* e); };
class RankInfo { public: Int m_skillPointsNeeded; Int m_sciencePurchasePointsGranted; ScienceVec m_sciencesGranted; };
class RankInfoStore
{
public:
	std::vector<RankInfo*> m_ranks;
	Int getRankLevelCount() const { return (Int)m_ranks.size(); }
	const RankInfo* getRankInfo(Int level) const { return (level >= 1 && level <= (Int)m_ranks.size()) ? m_ranks[level - 1] : NULL; }
};

/* ---- the special power modules of an object (addScience reaches them through the behaviour list) --------------------- */
class SpecialPowerTemplate { public: UnsignedInt m_id; UnsignedInt m_reload; bool m_shared; UnsignedInt getID() const { return m_id; } UnsignedInt getReloadTime() const { return m_reload; } Bool isSharedNSync() const { return m_shared; } };
/* ResourceGatheringManager.h lines 68-72 and TunnelTracker.h lines 81-88: the members the xfer reads, with the real types; the xfers are the real files' */
class ResourceGatheringManager : public MemoryPoolObject, public Snapshot
{
public:
	typedef std::list<ObjectID> objectIDList;
	virtual void crc(Xfer*);
	virtual void xfer(Xfer*);
	virtual void loadPostProcess(void) {}
	objectIDList m_supplyWarehouses;
	objectIDList m_supplyCenters;
};
class TunnelTracker : public MemoryPoolObject, public Snapshot
{
public:
	typedef std::list<Object*> ContainedItemsList;
	TunnelTracker() : m_containListSize(0), m_tunnelCount(0) {}
	virtual void crc(Xfer*);
	virtual void xfer(Xfer*);
	virtual void loadPostProcess(void) {}
	std::list< ObjectID > m_tunnelIDs;
	ContainedItemsList m_containList;
	std::list< ObjectID > m_xferContainList;
	Int m_containListSize;
	UnsignedInt m_tunnelCount;
};
class SpecialPowerModuleInterface
{
public:
	int m_serial; ScienceType m_required; int m_type; bool m_scriptOnly; unsigned m_ready, m_percent; SpecialPowerTemplate m_tmpl;
	ScienceType getRequiredScience() const { return m_required; }
	Bool isScriptOnly() const { return m_scriptOnly; }
	UnsignedInt getReadyFrame() const { return m_ready; }
	UnsignedInt getPercentReady() const { return m_percent; }
	const SpecialPowerTemplate* getSpecialPowerTemplate() const { return &m_tmpl; }
	void onSpecialPowerCreation();
	void setReadyFrame(UnsignedInt f);
};
class BehaviorModule { public: SpecialPowerModuleInterface* m_sp; SpecialPowerModuleInterface* getSpecialPower() { return m_sp; } };
/* ---- the partition manager and the filters Team::updateState builds ----------------------------------------------- */
class PartitionFilter { public: virtual ~PartitionFilter() {} };
class PartitionFilterRelationship : public PartitionFilter
{
public:
	enum { ALLOW_SAME_PLAYER = 1, ALLOW_ALLIES = 2, ALLOW_ENEMIES = 4, ALLOW_NEUTRAL = 8 };
	PartitionFilterRelationship(const Object* obj, unsigned flags) {}
};
class PartitionFilterAlive : public PartitionFilter { public: PartitionFilterAlive() {} };
class PartitionFilterAcceptByKindOf : public PartitionFilter { public: KindOfMaskType m_set, m_clear; PartitionFilterAcceptByKindOf(const KindOfMaskType& mustBeSet, const KindOfMaskType& mustBeClear) : m_set(mustBeSet), m_clear(mustBeClear) {} };
class PartitionFilterSameMapStatus : public PartitionFilter { public: PartitionFilterSameMapStatus(const Object* obj) {} };
class PartitionManager
{
public:
	Object* getClosestObject(const Object* obj, Real maxDist, DistanceCalculationType dc, PartitionFilter** filters);
	Real getDistanceSquared(const Object* a, const Object* b, DistanceCalculationType dc);
	void getMostValuableLocation(Int playerIndex, unsigned relationship, ValueOrThreat vot, Coord3D* pos);
	ObjectIterator* iterateAllObjects();
	ObjectIterator* iterateObjectsInRange(const Coord3D* pos, Real maxDist, DistanceCalculationType dc, PartitionFilter** filters, IterOrderType order);
};

/* ---- the game logic ----------------------------------------------------------------------------------------------- */
class GameLogic
{
public:
	UnsignedInt m_frame; std::vector<Object*> m_objs;
	UnsignedInt getFrame() const { return m_frame; }
	Object* getFirstObject() const { return m_objs.empty() ? NULL : m_objs[0]; }
	Object* findObjectByID(ObjectID id) const;
	void destroyObject(Object* obj);
	bool m_singlePlayer, m_shellGame, m_inUpdate; Int m_rankLimit;
	Bool isInSinglePlayerGame() const { return m_singlePlayer; }
	Bool isInShellGame() const { return m_shellGame; }
	Bool isInGameLogicUpdate() const { return m_inUpdate; }
	Int getRankLevelLimit() const { return m_rankLimit; }
};

/* ---- the sides list (Team::initFromSides reads the team dictionaries) -------------------------------------------- */
class TeamsInfo
{
public:
	Dict m_dict;
	Dict* getDict() { return &m_dict; }
};
class SidesList
{
public:
	std::vector<TeamsInfo*> m_teams;
	Int getNumTeams() const { return (Int)m_teams.size(); }
	TeamsInfo* getTeamInfo(Int i) { return m_teams[i]; }
};

extern PlayerList* ThePlayerList;
extern ScriptEngine* TheScriptEngine;
extern TerrainLogic* TheTerrainLogic;
extern GameLogic* TheGameLogic;
extern PartitionManager* ThePartitionManager;
extern ThingFactory* TheThingFactory;
extern GlobalData* TheGlobalData;
extern GameInfo* TheGameInfo;
extern MessageStream* TheMessageStream;
extern AI* TheAI;
extern Eva* TheEva;
extern ControlBar* TheControlBar;
extern InGameUI* TheInGameUI;
extern GameText* TheGameText;
extern VictoryConditions* TheVictoryConditions;
extern AudioManager* TheAudio;
extern RankInfoStore* TheRankInfoStore;
extern BuildAssistant* TheBuildAssistant;
extern Radar* TheRadar;
extern GameClient* TheGameClient;
extern PlayerTemplateStore* ThePlayerTemplateStore;

/* ---- the rest of what slice 2 of Player reaches ------------------------------------------------------------------------ */
class ProductionPrerequisite { public: bool m_ok; Bool isSatisfied(const Player* p) const { return m_ok; } };
class ProductionUpdateInterface { public: int m_queued; Int countUnitTypeInQueue(const ThingTemplate* t) const { return m_queued; } };
class ProductionUpdate { public: static ProductionUpdateInterface* getProductionUpdateInterfaceFromObject(const Object* obj); };
class UpdateModule { public: int m_dummy; };
class AutoDepositUpdate { public: Object* m_owner; void awardInitialCaptureBonus(Player* p); };
class StealthUpdate { public: int m_player; Bool isDisguised() const { return m_player >= 0; } Int getDisguisedPlayerIndex() const { return m_player; } };
class Drawable { public: Object* m_owner; void setIndicatorColor(Color c); };
enum MaxHealthChangeType { SAME_CURRENTHEALTH, PRESERVE_RATIO, ADD_CURRENT_HEALTH_TOO, FULLY_HEAL };
class BodyModuleInterface
{
public:
	Object* m_owner;
	Real getMaxHealth() const;
	void setMaxHealth(Real maxHealth, MaxHealthChangeType healthChangeType = SAME_CURRENTHEALTH);
	void applyDamageScalar(Real scalar);
};
class SupplyTruckAIInterface { public: Object* m_owner; void setForceWantingState(Bool v); };
class ActionManagerStandIn;
class BuildAssistant { public: void sellObject(Object* obj); };
class Radar { public: void removeObject(Object* obj); void addObject(Object* obj); };
class GameClient { public: void setTeamColor(Int red, Int green, Int blue); };
class PlayerTemplateStore { public: PlayerTemplate* m_observer; const PlayerTemplate* findPlayerTemplate(NameKeyType key) const; };
class ObjectIterator : public MemoryPoolObject
{
public:
	std::vector<Object*> m_objs; size_t m_at;
	Object* first() { m_at = 0; return m_objs.empty() ? NULL : m_objs[0]; }
	Object* next() { ++m_at; return m_at < m_objs.size() ? m_objs[m_at] : NULL; }
};

/* ---- the object ---------------------------------------------------------------------------------------------------- */
class Object : public MemoryPoolObject
{
	MAKE_DLINK(Object, TeamMemberList)
public:
	ObjectID m_id; ThingTemplate* m_tpl; Team* m_team; Player* m_player; ObjectStatusMaskType m_status;
	bool m_dead, m_destroyed, m_contained, m_held, m_enemyNear;
	bool m_hasAI; AIUpdateInterface m_ai; bool m_hasContain; ContainModuleInterface m_contain;
	Coord3D m_pos; float m_vision;
	unsigned m_didEnter, m_didExit, m_inside;
	unsigned m_reactions; /* 1 kill, 2 destroyObject, 4 removeAllContained, 8 attemptDamage: the object moves to the neutral player's default team */
	Object* m_nextObj;

	ObjectID getID() const { return m_id; }
	const ThingTemplate* getTemplate() const { return m_tpl; }
	Bool isEffectivelyDead() const { return m_dead; }
	Bool isDestroyed() const { return m_destroyed; }
	Bool isKindOf(KindOfType t) const { return m_tpl->isKindOf(t); }
	Bool isKindOfMulti(KindOfMaskType mustBeSet, KindOfMaskType mustBeClear) const { return m_tpl->isKindOfMulti(mustBeSet, mustBeClear); }
	const ObjectStatusMaskType& getStatusBits() const { return m_status; }
	Bool testStatus(ObjectStatusTypes s) const { return m_status.test(s); }
	Bool isDisabledByType(DisabledType type) const { return m_held && type == DISABLED_HELD; }
	Object* getContainedBy() const { return m_contained ? const_cast<Object*>(this) : NULL; }
	Player* getControllingPlayer() const { return m_player; }
	Team* getTeam() const { return m_team; }
	void setTeam(Team* team);
	const Coord3D* getPosition() const { return &m_pos; }
	Real getVisionRange() const { return m_vision; }
	void healCompletely();
	void kill();
	void attemptDamage(DamageInfo* info);
	void handlePartitionCellMaintenance();
	ContainModuleInterface* getContain() const { return m_hasContain ? const_cast<ContainModuleInterface*>(&m_contain) : NULL; }
	AIUpdateInterface* getAIUpdateInterface() const { return m_hasAI ? const_cast<AIUpdateInterface*>(&m_ai) : NULL; }
	Bool didEnter(const PolygonTrigger* t) const { return (m_didEnter >> t->m_id) & 1; }
	Bool didExit(const PolygonTrigger* t) const { return (m_didExit >> t->m_id) & 1; }
	Bool isInside(const PolygonTrigger* t) const { return (m_inside >> t->m_id) & 1; }
	Object* getNextObject() const { return m_nextObj; }
	BehaviorModule** getBehaviorModules() { return m_modules; }
	unsigned m_wb;
	bool m_modulesReady, m_hasAutoDeposit, m_hasDrawable, m_disabled, m_factionStructure, m_hasWeapon, m_selectable, m_supplyTruck;
	int m_disguisedPlayer; unsigned m_nightColor, m_indicatorColor, m_weaponPercent, m_whoEntered, m_canEnter; ObjectID m_producer; float m_health, m_shroud; int m_prodQueue;
	AutoDepositUpdate m_adu; StealthUpdate m_stealth; Drawable m_drawable; BodyModuleInterface m_body; SupplyTruckAIInterface m_truck; ProductionUpdateInterface m_pui;
	void friend_adjustPowerForPlayer(Bool yes);
	UpdateModule* findUpdateModule(NameKeyType key) const { return m_hasAutoDeposit ? (UpdateModule*)(const_cast<AutoDepositUpdate*>(&m_adu)) : NULL; }
	Bool areModulesReady() const { return m_modulesReady; }
	StealthUpdate* getStealth() const { return const_cast<StealthUpdate*>(&m_stealth); }
	Drawable* getDrawable() const { return m_hasDrawable ? const_cast<Drawable*>(&m_drawable) : NULL; }
	Color getNightIndicatorColor() const { return m_nightColor; }
	Color getIndicatorColor() const { return m_indicatorColor; }
	Bool isDisabled() const { return m_disabled; }
	Bool hasAnySpecialPower() const { for (int k = 0; k < 4; ++k) if (m_modules[k] && m_modules[k]->m_sp) return true; return false; }
	SpecialPowerModuleInterface* findAnyShortcutSpecialPowerModuleInterface() const { for (int k = 0; k < 4; ++k) if (m_modules[k] && m_modules[k]->m_sp) return m_modules[k]->m_sp; return NULL; }
	Bool hasSpecialPower(SpecialPowerType t) const { for (int k = 0; k < 4; ++k) if (m_modules[k] && m_modules[k]->m_sp && m_modules[k]->m_sp->m_type == (int)t) return true; return false; }
	SpecialPowerModuleInterface* findSpecialPowerModuleInterface(SpecialPowerType t) const { for (int k = 0; k < 4; ++k) if (m_modules[k] && m_modules[k]->m_sp && m_modules[k]->m_sp->m_type == (int)t) return m_modules[k]->m_sp; return NULL; }
	Bool hasAnyWeapon() const { return m_hasWeapon; }
	UnsignedInt getMostPercentReadyToFireAnyWeapon() const { return m_weaponPercent; }
	void leaveGroup();
	Bool isAnyKindOf(KindOfMaskType t) const { return m_tpl->m_kindof.anyIntersectionWith(t); }
	void setVisionSpied(Bool setting, Int byWhom);
	void updateUpgradeModules();
	BodyModuleInterface* getBodyModule() const { return const_cast<BodyModuleInterface*>(&m_body); }
	void setWeaponBonusCondition(WeaponBonusConditionType c);
	void clearWeaponBonusCondition(WeaponBonusConditionType c);
	void setVisionRange(Real r) { Rec("vision %d %08x", (int)m_id, fbits(r)); m_vision = r; }
	Real getShroudClearingRange() const { return m_shroud; }
	void setShroudClearingRange(Real r) { Rec("shroud %d %08x", (int)m_id, fbits(r)); m_shroud = r; }
	ObjectID getProducerID() const { return m_producer; }
	Bool isFactionStructure() const { return m_factionStructure; }
	Bool isSelectable() const { return m_selectable; }
	Real m_orientation; Real getOrientation() const { return m_orientation; }
	VeterancyLevel m_vet; VeterancyLevel getVeterancyLevel() const { return m_vet; }
	BehaviorModule* m_modules[4]; BehaviorModule m_mod[3]; SpecialPowerModuleInterface m_spm[3];
	void setScriptStatus(ObjectScriptStatusBit bit, Bool set = true);
	void setDisabled(DisabledType type);
	void clearDisabled(DisabledType type);
};

#endif
