/* The stand-ins for everything PartitionManager.cpp reaches outside itself (PartitionManager
** build only, differential tests; see differential/wrap_partition.cpp, which defines their
** members, installs the scenario they answer from, and says what each logs). The renames
** (PreRTS.h) keep every class here apart from the same upstream names in other fixtures.
**
** None of them has behaviour of its own beyond reading the scenario the test installed (g_pm in
** the wrapper) and logging the call that crosses the cut. The Rust side's `ScenarioHost`
** (src/logic/world/partition/scenario.rs) gives the same answers by the same rules; each rule is
** written once in the wrapper and once there, and the differential test is what keeps the two
** together.
**
** Upstream headers stood in for, and what from each:
**   Common/ActionManager.h       ActionManager::canPlayerGarrison, canEnterObject, canHijackVehicle
**   Common/Player.h, Team.h      Player (index, mask, relationship, default team, type), Team
**   Common/PlayerList.h          PlayerList (count, nth, local, getPlayersWithRelationship) and AllowPlayerRelationship
**   Common/Radar.h               Radar::setShroudLevel, clearShroud
**   Common/ThingTemplate.h       ThingTemplate (name, draw module names, isEquivalentTo, build cost)
**   GameClient/Display.h         Display::setShroudLevel, clearShroud and ShroudLevel
**   GameClient/ControlBar.h      CommandButton::isValidToUseOn
**   GameLogic/AI.h, AIPathfind.h AI::pathfinder, Pathfinder (3 members), PathfindCell
**   GameLogic/GameLogic.h        GameLogic::getFrame
**   GameLogic/GhostObject.h      GhostObjectManager, GhostObject (the members PartitionManager.cpp calls)
**   GameLogic/Object.h           Object (the members PartitionManager.cpp calls)
**   GameLogic/PolygonTrigger.h   PolygonTrigger::pointInTrigger
**   GameLogic/Squad.h            Squad::isOnSquad
**   GameLogic/TerrainLogic.h     TerrainLogic (the 9 members PartitionManager.cpp calls)
**   GameLogic/Module/*.h         ContainModuleInterface, StealthUpdate, BodyModuleInterface, AIUpdateInterface
**   Common/GlobalData.h          m_partitionCellSize, m_unlookPersistDuration */
#ifndef SHIM_PM_WORLD_H
#define SHIM_PM_WORLD_H
#include <vector>
#include <list>
#include <map>
#include "Common/AsciiString.h"
#include "Common/GameType.h"
#include "Common/GameCommon.h"
#include "Common/Geometry.h"
#include "Common/KindOf.h"
#include "Common/ObjectStatusTypes.h"
#include "Common/DisabledTypes.h"
#include "save_code.h"
#include "Common/Snapshot.h"
#include "WWMath/vector3.h"

#define GameLogicRandomValueReal PMGameLogicRandomValueReal
Real PMGameLogicRandomValueReal(Real lo, Real hi);

class Object;
class Player;
class Team;
class PartitionData;
class GhostObject;
class ThingTemplate;

/* ---- GameCommon.h / Player.h pieces PartitionManager.cpp names */
enum AllowPlayerRelationship	/* PlayerList.h lines 63-69, verbatim */
{
	ALLOW_SAME_PLAYER			= 0x01,
	ALLOW_ALLIES					= 0x02,
	ALLOW_ENEMIES					= 0x04,
	ALLOW_NEUTRAL					= 0x08
};
enum CanEnterType { CHECK_CAPACITY, DONT_CHECK_CAPACITY, COMBATDROP_INTO };	/* ActionManager.h */
enum CanAttackResult	/* WeaponSet.h lines 188-199, the enumerators in order */
{
	ATTACKRESULT_NOT_POSSIBLE,
	ATTACKRESULT_INVALID_SHOT,
	ATTACKRESULT_POSSIBLE_AFTER_MOVING,
	ATTACKRESULT_POSSIBLE
};

/* ---- the global data (the class named GlobalData is the shared shim's, in archive_prelude.h; this
** one has another name, and the two members PartitionManager.cpp reads) */
class PMGlobals
{
public:
	Real m_partitionCellSize;
	UnsignedInt m_unlookPersistDuration;
};
extern PMGlobals *PMTheGlobalData;
#define TheGlobalData PMTheGlobalData

/* ---- the game logic clock */
class GameLogic
{
public:
	UnsignedInt getFrame() const;
};
extern GameLogic *TheGameLogic;

/* ---- teams and players */
class Team
{
public:
	Int m_owner;	/* the controlling player's index, -1 for none */
	Relationship getRelationship(const Team *that) const;
};
class Player
{
public:
	Int m_index;
	Int getPlayerIndex() const { return m_index; }
	PlayerMaskType getPlayerMask() const { return (PlayerMaskType)(1 << m_index); }
	Relationship getRelationship(const Team *that) const;
	const Team *getDefaultTeam() const;
	PlayerType getPlayerType() const;
};
class PlayerList
{
public:
	Int getPlayerCount() const;
	Player *getNthPlayer(Int i) const;
	Player *getLocalPlayer() const;
	PlayerMaskType getPlayersWithRelationship(Int srcPlayerIndex, UnsignedInt allowedRelationships);
};
extern PlayerList *ThePlayerList;

/* ---- thing templates */
class ModuleInfo
{
public:
	Int getCount() const;
	AsciiString getNthName(Int i) const;
	Int m_flavour;	/* 0: W3DDefaultDraw only, 1: a model draw */
};
class ThingTemplate
{
public:
	Int m_key;
	Int m_cost;
	ModuleInfo m_draw;
	Int friend_getBuildCost() const { return m_cost; }
	const ModuleInfo &getDrawModuleInfo() const { return m_draw; }
	Bool isEquivalentTo(const ThingTemplate *that) const { return that && that->m_key == m_key; }
	AsciiString getName() const { return AsciiString("t"); }
};

/* ---- the modules of an object */
class Squad
{
public:
	Bool isOnSquad(const Object *obj) const;
};
class DamageInfoInput { public: ObjectID m_sourceID; };
class DamageInfo { public: DamageInfoInput in; };
class BodyModuleInterface
{
public:
	DamageInfo m_last;
	const DamageInfo *getLastDamageInfo() const { return &m_last; }
};
class StealthUpdate
{
public:
	Bool m_disguised;
	Int m_idx;
	Bool isDisguised() const { return m_disguised; }
	Int getDisguisedPlayerIndex() const { return m_idx; }
};
typedef std::list<Object*> ContainedItemsList;
class ContainModuleInterface
{
public:
	Bool m_garrisonable;
	Int m_count;
	Int m_stealthUnits;
	Int m_apparent;	/* player index, -1 for none */
	ContainedItemsList m_items;
	Bool isGarrisonable() const { return m_garrisonable; }
	Int getContainCount() const { return m_count; }
	Int getStealthUnitsContained() const { return m_stealthUnits; }
	const Player *getApparentControllingPlayer(const Player *observer) const;
	const ContainedItemsList *getContainedItemsList() const { return &m_items; }
};
class LocomotorSet { public: Int m_source; };
class AIUpdateInterface
{
public:
	LocomotorSet m_set;
	const LocomotorSet &getLocomotorSet() const { return m_set; }
};

/* ---- the command bar's buttons and the action manager */
class CommandButton
{
public:
	Int m_key;
	Bool isValidToUseOn(const Object *source, const Object *target, const Object *, CommandSourceType) const;
};
class ActionManager
{
public:
	Bool canPlayerGarrison(const Player *player, const Object *target, CommandSourceType cmdSource);
	Bool canEnterObject(const Object *obj, const Object *objectToEnter, CommandSourceType cmdSource, CanEnterType modifier);
	Bool canHijackVehicle(const Object *hijacker, const Object *target, CommandSourceType cmdSource);
};
extern ActionManager *TheActionManager;

/* ---- polygon triggers */
class PolygonTrigger
{
public:
	Int m_key;
	Bool pointInTrigger(ICoord3D &point) const;
};

/* ---- the terrain, the pathfinder */
class TerrainLogic
{
public:
	void getExtent(Region3D *extent) const;
	Real getGroundHeight(Real x, Real y, Coord3D *normal = NULL) const;
	PathfindLayerEnum getHighestLayerForDestination(const Coord3D *pos) const;
	Real getLayerHeight(Real x, Real y, PathfindLayerEnum layer) const;
	Bool isCliffCell(Real x, Real y) const;
	Bool isUnderwater(Real x, Real y, Real *waterZ = NULL, Real *terrainZ = NULL) const;
	void getMaximumPathfindExtent(Region3D *extent) const;
	Bool isClearLineOfSight(const Coord3D &pos, const Coord3D &posOther) const;
};
extern TerrainLogic *TheTerrainLogic;
#define PATHFIND_CELL_SIZE 10
class PathfindCell
{
public:
	enum CellType { CELL_CLEAR = 0x00, CELL_WATER = 0x01, CELL_CLIFF = 0x02, CELL_RUBBLE = 0x03, CELL_OBSTACLE = 0x04, CELL_BRIDGE_IMPASSABLE = 0x05, CELL_IMPASSABLE = 0x06 };
	CellType m_type;
	CellType getType() const { return m_type; }
};
class Pathfinder
{
public:
	PathfindCell *getCell(PathfindLayerEnum layer, Int x, Int y);
	Bool clientSafeQuickDoesPathExist(const LocomotorSet &locomotorSet, const Coord3D *from, const Coord3D *to);
	Bool isViewBlockedByObstacle(const Object *obj, const Object *objOther);
};
class AI
{
public:
	Pathfinder *pathfinder() { return &m_pathfinder; }
	Pathfinder m_pathfinder;
};
extern AI *TheAI;

/* ---- the client */
class Display
{
public:
	void setShroudLevel(Int x, Int y, CellShroudStatus setting);
	void clearShroud();
};
extern Display *TheDisplay;
class Radar
{
public:
	void setShroudLevel(Int x, Int y, CellShroudStatus setting);
	void clearShroud();
};
extern Radar *TheRadar;
struct ShroudLevel	/* Display.h lines 42-46, verbatim */
{
	Short m_currentShroud;
	Short m_activeShroudLevel;
};
class GhostObject
{
public:
	Int m_handle;
	GeometryType m_type;
	Bool m_small;
	Real m_major, m_minor;
	Coord3D m_pos;
	Real m_angle;
	PartitionData *m_partition;
	GeometryType getGeometryType() const { return m_type; }
	Bool getGeometrySmall() const { return m_small; }
	Real getGeometryMajorRadius() const { return m_major; }
	Real getGeometryMinorRadius() const { return m_minor; }
	const Coord3D *getParentPosition() const { return &m_pos; }
	Real getParentAngle() const { return m_angle; }
	PartitionData *friend_getPartitionData() const { return m_partition; }
	void snapShot(int playerIndex);
	void freeSnapShot(int playerIndex);
	void updateParentObject(Object *object, PartitionData *mod);
};
class GhostObjectManager
{
public:
	GhostObject *addGhostObject(Object *object, PartitionData *pd);
	void removeGhostObject(GhostObject *mod);
};
extern GhostObjectManager *TheGhostObjectManager;

/* ---- the object */
class Object
{
public:
	int m_idx;	/* into the scenario */
	PartitionData *m_partitionData;
	Object() : m_idx(-1), m_partitionData(NULL) {}

	ObjectID getID() const;
	const Coord3D *getPosition() const;
	Real getOrientation() const;
	const GeometryInfo &getGeometryInfo() const;
	const ThingTemplate *getTemplate() const;
	Bool isKindOf(KindOfType t) const;
	Bool isKindOfMulti(const KindOfMaskType &mustBeSet, const KindOfMaskType &mustBeClear) const;
	ObjectStatusMaskType getStatusBits() const;
	Bool testStatus(ObjectStatusTypes bit) const;
	Bool isDestroyed() const;
	Bool isEffectivelyDead() const;
	Bool isOffMap() const;
	Bool isUsingAirborneLocomotor() const;
	Bool isStructure() const;
	Bool isNonFactionStructure() const;
	Bool isAbleToAttack() const;
	Bool isDisabledByType(Int type) const;
	Player *getControllingPlayer() const;
	const Team *getTeam() const;
	Relationship getRelationship(const Object *that) const;
	const Coord3D *getUnitDirectionVector2D() const;
	struct XMatrix { Vector3 Get_X_Vector() const; Real x, y, z; };
	const XMatrix *getTransformMatrix() const;
	ContainModuleInterface *getContain() const;
	StealthUpdate *getStealth() const;
	BodyModuleInterface *getBodyModule() const;
	AIUpdateInterface *getAIUpdateInterface() const;
	CanAttackResult getAbleToAttackSpecificObject(AbleToAttackType t, const Object *target, CommandSourceType commandSource) const;
	ObjectShroudStatus getShroudedStatus(Int playerIndex) const;
	void onPartitionCellChange();
	void onCollide(Object *other, const Coord3D *loc, const Coord3D *normal);
	PartitionData *friend_getPartitionData() { return m_partitionData; }
	const PartitionData *friend_getConstPartitionData() const { return m_partitionData; }
	void friend_setPartitionData(PartitionData *pd) { m_partitionData = pd; }
};
#endif
