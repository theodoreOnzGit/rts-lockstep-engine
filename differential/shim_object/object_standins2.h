/* The stand-ins of the units Object.cpp calls into (p1w6-object), part 2: teams, players, the singletons.
** Declarations; the bodies need Object and are in object_standins3.h. */
#ifndef SHIM_OBJECT_STANDINS2_H
#define SHIM_OBJECT_STANDINS2_H

class AttackPriorityInfo
{
public:
	AsciiString m_name;
	const AsciiString& getName() const { return m_name; }
};
class TeamTemplateInfo { public: Int m_initialTeamAttitude; TeamTemplateInfo() : m_initialTeamAttitude(0) {} };
class TeamPrototype
{
public:
	TeamTemplateInfo m_info; Bool m_hasInfo; AsciiString m_attackName;
	TeamPrototype() : m_hasInfo(false) {}
	const TeamTemplateInfo* getTemplateInfo() const { return m_hasInfo ? &m_info : NULL; }
	const AsciiString& getAttackPriorityName() const { return m_attackName; }
};
class Team
{
public:
	TeamID m_id; Int m_player; AsciiString m_name; TeamPrototype m_proto; std::list<Object*> m_members;
	TeamID getID() const { return m_id; }
	const AsciiString& getName() const { return m_name; }
	Player* getControllingPlayer() const;
	const TeamPrototype* getPrototype() const { return &m_proto; }
	Relationship getRelationship(const Team* that) const;
	Bool isInList_TeamMemberList(const Object* o) const;
	void removeFrom_TeamMemberList(Object* o);
	void prependTo_TeamMemberList(Object* o);
	void notifyTeamOfObjectDeath();
	void setEnteredExited();
};
class ScoreKeeper
{
public:
	Player* m_p;
	void addObjectCaptured(const Object* o);
	void addObjectLost(const Object* o);
	void addObjectDestroyed(const Object* o);
};
class Energy
{
public:
	Player* m_p;
	void objectEnteringInfluence(Object* o);
	void objectLeavingInfluence(Object* o);
	void adjustPower(Int amount, Bool add);
};
class AcademyStats { public: void recordMine(); };
class Player
{
public:
	Int m_index; ScoreKeeper m_score; Energy m_energy;
	Int getPlayerIndex() const { return m_index; }
	PlayerMaskType getPlayerMask() const;
	Color getPlayerColor() const;
	Color getPlayerNightColor() const;
	Bool isPlayerActive() const;
	Bool isPlayableSide() const;
	Bool isSkirmishAIPlayer();
	Team* getDefaultTeam();
	const Team* getDefaultTeam() const;
	Relationship getRelationship(const Team* that) const;
	UpgradeMaskType getCompletedUpgradeMask() const;
	Int getNumBattlePlansActive() const;
	VeterancyLevel getProductionVeterancyLevel(AsciiString buildTemplateName) const;
	void becomingTeamMember(Object* o, Bool incoming);
	void applyBattlePlanBonusesForObject(Object* o) const;
	void friend_applyDifficultyBonusesForObject(Object* o, Bool apply);
	void addSkillPointsForKill(Object* killer, const Object* victim);
	void doBountyForKill(Object* killer, const Object* victim);
	void removeRadar(Bool disableProof);
	void addRadar(Bool disableProof);
	ScoreKeeper* getScoreKeeper() { m_score.m_p = this; return &m_score; }
	Energy* getEnergy() { m_energy.m_p = this; return &m_energy; }
	AcademyStats* getAcademyStats();
};
class PlayerList
{
public:
	std::vector<Player*> players;
	Player* getNeutralPlayer() { return players[0]; }
	Player* getLocalPlayer() { return players[g_cfg.localPlayer]; }
	Int getPlayerCount() const { return (Int)players.size(); }
	Player* getNthPlayer(Int i) { return (i >= 0 && i < (Int)players.size()) ? players[i] : NULL; }
	PlayerMaskType getPlayersWithRelationship(Int srcPlayerIndex, UnsignedInt allowedRelationships);
};
extern PlayerList* ThePlayerList;
class TeamFactory
{
public:
	std::map<int, Team*> teams;
	Team* findTeamByID(TeamID id) { std::map<int, Team*>::iterator it = teams.find((int)id); return it == teams.end() ? NULL : it->second; }
	Team* findTeam(const AsciiString& name)
	{
		for (std::map<int, Team*>::iterator it = teams.begin(); it != teams.end(); ++it)
			if (it->second->m_name == name) return it->second;
		return NULL;
	}
};
extern TeamFactory* TheTeamFactory;

/* ---- the client: drawables, audio, radar, UI -------------------------------------------------------------- */
class Drawable
{
public:
	DrawableID m_id; Object* m_obj;
	DrawableID getID() const { return m_id; }
	void setID(DrawableID id);
	void changedTeam();
	void reactToGeometryChange();
	void setSelectable(Bool s);
	void setTintStatus(TintStatus s);
	void clearTintStatus(TintStatus s);
	void setModelConditionState(ModelConditionFlagType a);
	void clearModelConditionState(ModelConditionFlagType a);
	void clearAndSetModelConditionState(ModelConditionFlagType c, ModelConditionFlagType s);
	void setModelConditionFlags(const ModelConditionFlags& f);
	void clearModelConditionFlags(const ModelConditionFlags& f);
	void clearAndSetModelConditionFlags(const ModelConditionFlags& c, const ModelConditionFlags& s);
	void setTerrainDecalFadeTarget(Real target, Real rate);
	void flashAsSelected();
	void setTransformMatrix(const Matrix3D* m) {}
};
class RadarObject { public: int dummy; };
class Radar
{
public:
	void addObject(Object* o);
	void removeObject(Object* o);
	void tryUnderAttackEvent(const Object* o);
	void tryInfiltrationEvent(const Object* o);
	void tryEvent(RadarEventType t, const Coord3D* p);
};
extern Radar* TheRadar;
class Eva { public: void setShouldPlay(EvaMessage m); };
extern Eva* TheEva;
class Anim2DTemplate { public: int dummy; };
class Anim2DCollection { public: Anim2DTemplate* findTemplate(const AsciiString& n) { static Anim2DTemplate t; return &t; } };
extern Anim2DCollection* TheAnim2DCollection;
class InGameUI
{
public:
	void objectChangedTeam(const Object* o, Int oldP, Int newP);
	void removeIdleWorker(Object* o, Int p);
	void addWorldAnimation(const Object* o, Anim2DTemplate* t, const Coord3D* pos, Int options, Real displayTime, Real zRise);
};
extern InGameUI* TheInGameUI;
class ControlBar { public: void markUIDirty(); };
extern ControlBar* TheControlBar;
class GameClient { public: void notifyTerrainObjectMoved(Object* o); };
extern GameClient* TheGameClient;
class BuildAssistant { public: void sellObject(Object* o); };
extern BuildAssistant* TheBuildAssistant;
class SpecialPowerStore { public: Bool canUseSpecialPower(const Object* o, const SpecialPowerTemplate* t); };
extern SpecialPowerStore* TheSpecialPowerStore;
class UpgradeCenter
{
public:
	const UpgradeTemplate* findVeterancyUpgrade(VeterancyLevel l); UpgradeTemplate templates[4];
	std::vector<UpgradeTemplate*> list; /* creation order; the list is linked at the head: newest first */
	const UpgradeTemplate* findUpgrade(const AsciiString& n) { for (size_t i = 0; i < list.size(); ++i) if (list[i]->m_name == n) return list[i]; return NULL; }
};
extern UpgradeCenter* TheUpgradeCenter;
class ScriptEngine
{
public:
	void notifyOfObjectCreationOrDestruction();
	void notifyOfObjectDestruction(Object* o);
	void AppendDebugMessage(const char* s, Bool b);
	const AttackPriorityInfo* getAttackInfo(AsciiString name);
	Bool getObjectsShouldReceiveDifficultyBonus() { return g_cfg.difficultyBonus; }
};
extern ScriptEngine* TheScriptEngine;

/* ---- polygon triggers and terrain -------------------------------------------------------------------------- */
class PolygonTrigger
{
public:
	AsciiString m_name; PolygonTrigger* m_next; std::vector<ICoord3D> m_points;
	static PolygonTrigger* s_first;
	static PolygonTrigger* getFirstPolygonTrigger() { return s_first; }
	PolygonTrigger* getNext() const { return m_next; }
	const AsciiString& getTriggerName() const { return m_name; }
	/* PolygonTrigger.cpp's pointInTrigger, with the bounds computed here (updateBounds) */
	Bool pointInTrigger(ICoord3D& point) const
	{
		const Int BIG_INT = 0x7ffff0;
		Int loX = BIG_INT, loY = BIG_INT, hiX = -BIG_INT, hiY = -BIG_INT;
		Int n = (Int)m_points.size();
		for (Int i = 0; i < n; i++)
		{
			if (m_points[i].x < loX) loX = m_points[i].x;
			if (m_points[i].y < loY) loY = m_points[i].y;
			if (m_points[i].x > hiX) hiX = m_points[i].x;
			if (m_points[i].y > hiY) hiY = m_points[i].y;
		}
		if (point.x < loX) return false;
		if (point.y < loY) return false;
		if (point.x > hiX) return false;
		if (point.y > hiY) return false;
		Bool inside = false;
		for (Int i = 0; i < n; i++)
		{
			ICoord3D pt1 = m_points[i];
			ICoord3D pt2;
			if (i == n - 1) pt2 = m_points[0]; else pt2 = m_points[i + 1];
			if (pt1.y == pt2.y) continue;
			if (pt1.y < point.y && pt2.y < point.y) continue;
			if (pt1.y >= point.y && pt2.y >= point.y) continue;
			if (pt1.x < point.x && pt2.x < point.x) continue;
			Int dy = pt2.y - pt1.y;
			Int dx = pt2.x - pt1.x;
			Real intersectionX = pt1.x + (dx * (point.y - pt1.y)) / ((Real)dy);
			if (intersectionX >= point.x) inside = !inside;
		}
		return inside;
	}
};
class TerrainLogic
{
public:
	void getExtent(Region3D* r);
	Real getLayerHeight(Real x, Real y, PathfindLayerEnum layer);
	Real getGroundHeight(Real x, Real y, Coord3D* n = NULL) { return 0.0f; }
	Bool isUnderwater(Real x, Real y, Real* waterZ = NULL, Real* terrainZ = NULL) { return false; }
	void alignOnTerrain(Real angle, const Coord3D& pos, Bool stick, Matrix3D& mtx);
	PolygonTrigger* getTriggerAreaByName(AsciiString name);
};
extern TerrainLogic* TheTerrainLogic;
class Pathfinder
{
public:
	void updatePos(Object* o, const Coord3D* p);
	void removePos(Object* o);
	void addObjectToPathfindMap(Object* o);
	void removeObjectFromPathfindMap(Object* o);
	void removeWallPiece(Object* o);
};
class AIData { public: Bool m_enableRepulsors; };
class AIManager { public: Pathfinder pf; AIData data; Pathfinder* pathfinder() { return &pf; } AIData* getAiData() { return &data; } };
extern AIManager* TheAI;

/* ---- the partition manager ------------------------------------------------------------------------------------ */
class PartitionData
{
public:
	Object* m_obj;
	void makeDirty(Bool b);
	ObjectShroudStatus getShroudedStatus(Int playerIndex) const;
};
class ObjectIterator : public MemoryPoolObject
{
public:
	std::vector<Object*> v; size_t at;
	ObjectIterator() : at(0) {}
	Object* first() { at = 0; return at < v.size() ? v[at] : NULL; }
	Object* next() { ++at; return at < v.size() ? v[at] : NULL; }
};
class PartitionFilter {};
class PartitionFilterAcceptByKindOf : public PartitionFilter { public: PartitionFilterAcceptByKindOf(const KindOfMaskType& a, const KindOfMaskType& b) {} };
class PartitionFilterSameMapStatus : public PartitionFilter { public: PartitionFilterSameMapStatus(const Object* o) {} };
class PartitionManager
{
public:
	void registerObject(Object* o);
	void unRegisterObject(Object* o);
	void doShroudReveal(Real x, Real y, Real r, PlayerMaskType m);
	void queueUndoShroudReveal(Real x, Real y, Real r, PlayerMaskType m);
	void doShroudCover(Real x, Real y, Real r, PlayerMaskType m);
	void undoShroudCover(Real x, Real y, Real r, PlayerMaskType m);
	void doValueAffect(Real x, Real y, Real r, UnsignedInt d, PlayerMaskType m);
	void undoValueAffect(Real x, Real y, Real r, UnsignedInt d, PlayerMaskType m);
	void doThreatAffect(Real x, Real y, Real r, UnsignedInt d, PlayerMaskType m);
	void undoThreatAffect(Real x, Real y, Real r, UnsignedInt d, PlayerMaskType m);
	ObjectIterator* iteratePotentialCollisions(const Coord3D* pos, const GeometryInfo& geom, Real angle);
	ObjectIterator* iterateObjectsInRange(const Coord3D* pos, Real maxDist, Int distType, PartitionFilter** filters, Int order);
};
extern PartitionManager* ThePartitionManager;

/* ---- AIGroup, WeaponSet, the global data ----------------------------------------------------------------------- */
class AIGroup { public: UnsignedInt m_id; void remove(Object* o); };
class Weapon : public Snapshot { public: int dummy; void crc(Xfer* x) {} void xfer(Xfer* x) {} void loadPostProcess() {} };
class WeaponSet : public Snapshot
{
public:
	Weapon* getWeaponInWeaponSlot(WeaponSlotType s) const;
	Bool hasAnyWeapon() const;
	UnsignedInt getNthCommandSourceMask(WeaponSlotType s) const { return 0; }
	Bool isSharedReloadTime() const { return false; }
	Bool setWeaponLock(WeaponSlotType s, WeaponLockType t) { return false; }
	void releaseWeaponLock(WeaponLockType t) {}
	Bool isCurWeaponLocked() const { return false; }
	void updateWeaponSet(const Object* o);
	void weaponSetOnWeaponBonusChange(Object* o);
	Object* m_owner;
	WeaponSet() : m_owner(NULL) {}
	void crc(Xfer* x) {}
	void xfer(Xfer* x);
	void loadPostProcess() {}
};
class GlobalData
{
public:
	Bool m_forceModelsToFollowTimeOfDay; Int m_timeOfDay; Bool m_forceModelsToFollowWeather; Int m_weather;
	AsciiString m_levelGainAnimationName; Real m_levelGainAnimationDisplayTimeInSeconds, m_levelGainAnimationZRisePerSecond; Real m_gravity;
};
extern GlobalData* TheGlobalData;
class ModuleFactory { public: BehaviorModule* newModule(Object* o, const AsciiString& name, const void* data, Int type); };
extern ModuleFactory* TheModuleFactory;


struct RebuildHoleBehavior { static BehaviorModule* getRebuildHoleBehaviorInterfaceFromObject(Object* o); };

/* ---- the game logic -------------------------------------------------------------------------------------------- */
struct TTriggerInfo { Byte entered; Byte exited; Byte isInside; const PolygonTrigger* pTrigger; TTriggerInfo() : entered(0), exited(0), isInside(0), pTrigger(NULL) {} };
class Waypoint { public: int dummy; };
class GameLogic
{
public:
	UnsignedInt m_frame; ObjectID m_nextID; std::map<int, Object*> m_table; Object* m_objList; int m_nextDrawable;
	std::map<int, Drawable*> m_drawables;
	GameLogic() : m_frame(0), m_nextID(INVALID_ID), m_objList(NULL), m_nextDrawable(1) {}
	UnsignedInt getFrame() const { return m_frame; }
	ObjectID allocateObjectID() { m_nextID = (ObjectID)((int)m_nextID + 1); return m_nextID; }
	void addObjectToLookupTable(Object* o);
	void removeObjectFromLookupTable(Object* o);
	Object* findObjectByID(ObjectID id);
	void registerObject(Object* o);
	Object* getFirstObject() { return m_objList; }
	void destroyObject(Object* o);
	void sendObjectCreated(Object* o);
	void sendObjectDestroyed(Object* o);
	void updateObjectsChangedTriggerAreas();
	void deselectObject(const Object* o, PlayerMaskType mask, Bool affectClient);
	Bool getDrawIconUI() const { return g_cfg.drawIconUI; }
};
extern GameLogic* TheGameLogic;
#include "weaponset_map.inc"
#define MAX_TRIGGER_AREA_INFOS_STUB 5

#endif
