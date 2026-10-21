/* The stand-ins of the world that BuildAssistant.cpp reaches (p1w10-sonnet). Included through the stand-in GameLogic/Object.h, with every
** class below renamed by the #define lines of differential/wrap_build_assistant.cpp (BaTest*), so that no symbol of this archive collides
** with another fixture's.
**
** Method: the classes answer from the program's configuration (members set by the driver from the program words); every call that has an
** effect writes a line to g_lines, formatted exactly as the Rust host (tests/differential_build_assistant.rs) writes it. Calls that only
** answer a question write nothing. Reals are written as their bit patterns.
**
** What is real: BuildAssistant.h and BuildAssistant.cpp (the cut one), Lib/BaseType.h (Coord3D, Region3D, the REAL_TO_INT macros),
** Common/Geometry.h (GeometryInfo, Geometry.cpp is another archive), Common/KindOf.h, Common/ObjectStatusTypes.h, Common/GameCommon.h,
** WWMath's Matrix3D and Vector3, and the random value generator (GameLogicRandomValueReal, RandomValue.cpp is another archive).
** What is a stand-in: the objects and their interfaces, the thing templates, the player, the terrain, the terrain visual, the partition
** manager's queries (verified by differential_partition.rs), the pathfinder's cells, the control bar's command sets, the audio, the game
** logic, the global data, the model condition flags (a list of indices), and the four Xfer calls the sell list uses. */
#ifndef BA_STANDINS_DECL_H
#define BA_STANDINS_DECL_H

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
#include <math.h>
#include "Lib/BaseType.h"
#include "Common/GameMemory.h"
#include "Common/GameCommon.h"
#include "Common/GameType.h"
#include "Common/KindOf.h"
#include "Common/ObjectStatusTypes.h"
#include "Common/Geometry.h"
#include "Common/SubsystemInterface.h"
#include "GameLogic/ObjectScriptStatusBits.h"
#include "GameLogic/LogicRandomValue.h"
extern "C" void ref_init_game_logic_random(unsigned int seed);   /* differential/wrap_random_value.cpp: InitGameLogicRandom */

/* BaseType.h lines 231, 232 and 244 on the transcriptions of its x87 helpers (differential/wrap_base_type.cpp, verified against the Rust by
** differential_base_type.rs): the shim BaseType.h has no asm, as in the Player build's prelude */
extern "C" {
int ref_fast_float2long_round(float f);
float ref_fast_float_trunc(float f);
float ref_fast_float_floor(float f);
}
#define REAL_TO_INT(x) ((Int)(ref_fast_float2long_round(ref_fast_float_trunc(x))))
#define REAL_TO_UNSIGNEDINT(x) ((UnsignedInt)(ref_fast_float2long_round(ref_fast_float_trunc(x))))
#define REAL_TO_INT_FLOOR(x) (ref_fast_float2long_round(ref_fast_float_floor(x)))

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
	static const bool trace = getenv("BA_TRACE") != NULL;
	if (trace) fprintf(stderr, "%s\n", buf);
}
#define FB(x) fbits(x)

class Object; class Player; class ThingTemplate;

/* ---- constants of headers the real BuildAssistant.cpp includes that are not the real ones here ---------------- */
#define MAP_XY_FACTOR (10.0f)        /* MapObject.h line 60 */
#define PATHFIND_CELL_SIZE 10        /* AIPathfind.h line 439 */
#define PATHFIND_CELL_SIZE_F 10.0f   /* AIPathfind.h line 440 */
enum { MAX_COMMANDS_PER_SET = 18 };  /* ControlBar.h line 406 */
/* ControlBar.h lines 169-174 */
enum GUICommandType { GUI_COMMAND_NONE = 0, GUI_COMMAND_DOZER_CONSTRUCT, GUI_COMMAND_DOZER_CONSTRUCT_CANCEL, GUI_COMMAND_UNIT_BUILD, GUI_COMMAND_CANCEL_UNIT_BUILD, GUI_COMMAND_PLAYER_UPGRADE };
/* ModelConditionFlags, ModelConditionFlagType and MAKE_MODELCONDITION_MASK* are the real ones (Common/ModelState.h, through the prelude) */

/* ---- Xfer: the real Xfer (Xfer.h, Xfer.cpp in the Xfer archive) under a memory buffer -------------------------------------- */
class BaXferImpl : public Xfer
{
public:
	std::vector<unsigned char> m_data;
	size_t m_pos;
	BaXferImpl(XferMode mode) : m_pos(0) { m_xferMode = mode; }
	virtual void open(AsciiString) {}
	virtual void close(void) {}
	virtual Int beginBlock(void) { return 0; }
	virtual void endBlock(void) {}
	virtual void skip(Int) {}
	virtual void xferSnapshot(Snapshot*) {}
protected:
	virtual void xferImplementation(void* p, Int n)
	{
		if (m_xferMode == XFER_SAVE) { const unsigned char* c = (const unsigned char*)p; m_data.insert(m_data.end(), c, c + n); }
		else { memcpy(p, &m_data[m_pos], n); m_pos += n; }
	}
};

/* ---- the global data -------------------------------------------------------------------------------------------- */
struct GlobalData
{
	Int m_maxLineBuildObjects;
	Real m_MinDistFromEdgeOfMapForBuild;
	Real m_allowedHeightVariationForBuilding;
	Real m_SupplyBuildBorder;
	Real m_sellPercentage;
};
extern GlobalData* TheGlobalData;

/* ---- the thing templates ---------------------------------------------------------------------------------------- */
class AudioEventRTS
{
public:
	int m_tmpl;
	ObjectID m_id;
	void setObjectID(ObjectID id) { m_id = id; }
};
class ThingTemplate
{
public:
	int m_idx;
	KindOfMaskType m_kinds;
	GeometryInfo m_geom;
	Real m_exitWidth, m_extraBib, m_vision;
	Int m_refund, m_cost;
	int m_equiv;
	unsigned m_flags;     /* bit 0 AI, 1 drawable, 2 production, 3 contain, 4 parking place, 5..7 the number of create modules */
	AudioEventRTS m_voice;
	ThingTemplate() : m_idx(0), m_geom(GEOMETRY_SPHERE, false, 0.0f, 0.0f, 0.0f), m_exitWidth(0), m_extraBib(0), m_vision(0), m_refund(0), m_cost(0), m_equiv(0), m_flags(0) { m_voice.m_tmpl = 0; m_voice.m_id = (ObjectID)0; }
	Bool isKindOf(KindOfType t) const { return m_kinds.test(t); }
	const GeometryInfo& getTemplateGeometryInfo() const { return m_geom; }
	Real getFactoryExitWidth() const { return m_exitWidth; }
	Real getFactoryExtraBibWidth() const { return m_extraBib; }
	Real friend_calcVisionRange() const { return m_vision; }
	Int getRefundValue() const { return m_refund; }
	Int calcCostToBuild(const Player* player) const;
	Bool isEquivalentTo(const ThingTemplate* tt) const { if (!tt) return false; if (this == tt) return true; return m_equiv == tt->m_equiv; }
	const AudioEventRTS* getPerUnitSound(const char*) const { return &m_voice; }
};

/* ---- the interfaces of an object ----------------------------------------------------------------------------- */
class AIUpdateInterface
{
public:
	Object* m_owner;
	Bool isBusy() const;
	void aiIdle(CommandSourceType);
	Object* construct(const ThingTemplate* what, const Coord3D* pos, Real angle, Player* owner, Bool);
	Bool isQuickPathAvailable(const Coord3D* pos) const;
	void aiMoveToPositionEvenIfSleeping(const Coord3D* pos, CommandSourceType);
};
class CommandButton
{
public:
	int m_type;
	const ThingTemplate* m_tmpl;
	int m_id;
	int getCommandType() const { return m_type; }
	const ThingTemplate* getThingTemplate() const { return m_tmpl; }
};
/* the answer of canQueueCreateUnit, converted to BuildAssistant.h's CanMakeType (declared after this header) at the call */
struct CanMakeInt { int v; template<class E> operator E() const { return (E)v; } };
class ProductionUpdateInterface
{
public:
	Object* m_owner;
	const CommandButton* getSpecialPowerConstructionCommandButton() const;
	CanMakeInt canQueueCreateUnit(const ThingTemplate* t) const;
	void cancelAndRefundAllProduction();
};
class ContainModuleInterface
{
public:
	Object* m_owner;
	void onSelling();
};
class ParkingPlaceBehaviorInterface
{
public:
	Object* m_owner;
	int m_idx;
	void killAllParkedUnits();
};
class CreateModuleInterface
{
public:
	Object* m_owner;
	int m_n;
	void onBuildComplete();
};
class BehaviorModule
{
public:
	Object* m_owner;
	int m_idx;
	CreateModuleInterface m_create;
	ParkingPlaceBehaviorInterface m_parking;
	bool m_hasCreate, m_hasParking;
	BehaviorModule() : m_owner(NULL), m_idx(0), m_hasCreate(false), m_hasParking(false) { m_create.m_owner = NULL; m_create.m_n = 0; m_parking.m_owner = NULL; m_parking.m_idx = 0; }
	CreateModuleInterface* getCreate() { return m_hasCreate ? &m_create : NULL; }
	ParkingPlaceBehaviorInterface* getParkingPlaceBehaviorInterface() { return m_hasParking ? &m_parking : NULL; }
};
class Drawable
{
public:
	Object* m_owner;
	void setAnimationLoopDuration(UnsignedInt frames);
};
class Team;

/* ---- the object ------------------------------------------------------------------------------------------------ */
class Object
{
public:
	ObjectID m_id;
	ThingTemplate* m_tmpl;
	Player* m_player;
	Coord3D m_pos;
	Real m_orient;
	GeometryInfo m_geom;
	unsigned long long m_status;      /* bit i is ObjectStatusTypes i (there are 45) */
	bool m_disabled, m_dead, m_destroyed;
	unsigned m_scriptBits;
	bool m_aiBusy, m_quickPath;
	int m_cmdSet;
	Real m_cpct;
	ObjectID m_producer;
	int m_spConsTmpl;       /* index of a template, or -1 */
	int m_canQueue;
	AIUpdateInterface m_ai;
	ProductionUpdateInterface m_prod;
	ContainModuleInterface m_contain;
	Drawable m_draw;
	BehaviorModule m_modules[8];
	int m_nModules;
	BehaviorModule* m_modPtrs[9];
	Object() : m_id((ObjectID)0), m_tmpl(NULL), m_player(NULL), m_orient(0), m_geom(GEOMETRY_SPHERE, false, 0.0f, 0.0f, 0.0f), m_status(0), m_disabled(false), m_dead(false), m_destroyed(false), m_scriptBits(0),
		m_aiBusy(false), m_quickPath(false), m_cmdSet(-1), m_cpct(-1.0f), m_producer(INVALID_ID), m_spConsTmpl(-1), m_canQueue(0), m_nModules(0)
	{
		m_pos.x = m_pos.y = m_pos.z = 0;
		for (int i = 0; i < 9; ++i) m_modPtrs[i] = NULL;
	}

	ObjectID getID() const { return m_id; }
	const ThingTemplate* getTemplate() const { return m_tmpl; }
	Bool isKindOf(KindOfType t) const { return m_tmpl->isKindOf(t); }
	Bool testStatus(ObjectStatusTypes s) const;
	Bool testScriptStatusBit(ObjectScriptStatusBit b) const { return (m_scriptBits & b) != 0; }
	Bool isDisabled() const { return m_disabled; }
	Bool isEffectivelyDead() const { return m_dead; }
	const GeometryInfo& getGeometryInfo() const { return m_geom; }
	const Coord3D* getPosition() const { return &m_pos; }
	Real getOrientation() const { return m_orient; }
	Player* getControllingPlayer() const { return m_player; }
	Team* getTeam() const;
	Relationship getRelationship(const Object* that) const;
	AIUpdateInterface* getAI() { return (m_tmpl->m_flags & 1) ? &m_ai : NULL; }
	AIUpdateInterface* getAIUpdateInterface() { return getAI(); }
	ProductionUpdateInterface* getProductionUpdateInterface() { return (m_tmpl->m_flags & 4) ? &m_prod : NULL; }
	ContainModuleInterface* getContain() { return (m_tmpl->m_flags & 8) ? &m_contain : NULL; }
	Drawable* getDrawable() { return (m_tmpl->m_flags & 2) ? &m_draw : NULL; }
	BehaviorModule** getBehaviorModules() { return m_modPtrs; }
	AsciiString getCommandSetString() const;
	ObjectID getProducerID() const { return m_producer; }
	Real getConstructionPercent() const { return m_cpct; }
	void setConstructionPercent(Real p);
	void setProducer(const Object* p);
	void setPosition(const Coord3D* p);
	void setOrientation(Real a);
	void clearAndSetModelConditionFlags(const ModelConditionFlags& clr, const ModelConditionFlags& set);
	void setModelConditionFlags(const ModelConditionFlags& set);
	void setStatus(ObjectStatusMaskType objectStatus, Bool set = true);
	void handlePartitionCellMaintenance();
	Object* getNextObject() const;
};

/* ---- the player ------------------------------------------------------------------------------------------------- */
class Money
{
public:
	UnsignedInt m_money;
	int m_idx;
	UnsignedInt countMoney() const { return m_money; }
	void deposit(UnsignedInt amount);
};
class Player
{
public:
	int m_index;
	PlayerType m_type;
	bool m_skirmish, m_hasTeam;
	Money m_moneyObj;
	unsigned m_canBuildMask, m_canMoreMask;
	PlayerType getPlayerType() const { return m_type; }
	Bool isSkirmishAIPlayer() const { return m_skirmish; }
	Int getPlayerIndex() const { return m_index; }
	Team* getDefaultTeam() const;
	Relationship getRelationship(const Team* that) const;
	Bool canBuild(const ThingTemplate* t) const;
	Bool canBuildMoreOfType(const ThingTemplate* t) const;
	Money* getMoney() { return &m_moneyObj; }
	void onStructureCreated(Object* builder, Object* structure);
	void onStructureConstructionComplete(Object* builder, Object* structure, Bool);
	void onUnitCreated(Object* factory, Object* unit);
};

/* ---- the terrain, its visual and the pathfinder ------------------------------------------------------------------- */
class TerrainLogic
{
public:
	Real getGroundHeight(Real x, Real y, Coord3D* normal = NULL) const;
	void getExtent(Region3D* r) const;
	void getMaximumPathfindExtent(Region3D* r) const;
	PathfindLayerEnum getLayerForDestination(const Coord3D* pos) const;
};
extern TerrainLogic* TheTerrainLogic;
class TerrainType
{
public:
	bool m_restrict;
	Bool getRestrictConstruction() const { return m_restrict; }
};
class TerrainVisual
{
public:
	TerrainType* getTerrainTile(Real x, Real y);
	void addFactionBib(Object* factionBuilding, Bool highlight, Real extra = 0);
	void removeTreesAndPropsForConstruction(const Coord3D* pos, const GeometryInfo& geom, Real angle);
};
extern TerrainVisual* TheTerrainVisual;
class PathfindCell
{
public:
	enum CellType { CELL_CLEAR = 0, CELL_WATER = 1, CELL_CLIFF = 2, CELL_RUBBLE = 3, CELL_OBSTACLE = 4, CELL_BRIDGE_IMPASSABLE = 5, CELL_IMPASSABLE = 6 };
	CellType m_type;
	CellType getType() const { return m_type; }
};
class Pathfinder
{
public:
	PathfindCell* getCell(PathfindLayerEnum layer, Int x, Int y);
	void addObjectToPathfindMap(Object* obj);
};
class AI { public: Pathfinder* pathfinder(); };
extern AI* TheAI;

/* ---- the partition manager's queries (the real unit is verified by differential_partition.rs) ------------------------ */
class MemoryPoolObjectHolder
{
	MemoryPoolObject* m_mpo;
public:
	MemoryPoolObjectHolder(MemoryPoolObject* mpo = NULL) : m_mpo(mpo) {}
	~MemoryPoolObjectHolder() { m_mpo->deleteInstance(); }
};
class ObjectIterator : public MemoryPoolObject
{
public:
	std::vector<Object*> m_objs;
	size_t m_at;
	ObjectIterator() : m_at(0) {}
	Object* first() { m_at = 0; return m_at < m_objs.size() ? m_objs[m_at] : NULL; }
	Object* next() { ++m_at; return m_at < m_objs.size() ? m_objs[m_at] : NULL; }
};
enum DistanceCalculationType { FROM_CENTER_2D = 0, FROM_CENTER_3D = 1, FROM_BOUNDINGSPHERE_2D = 2, FROM_BOUNDINGSPHERE_3D = 3 };
class PartitionFilter { public: KindOfMaskType m_must; virtual ~PartitionFilter() {} };
class PartitionFilterAcceptByKindOf : public PartitionFilter
{
public:
	PartitionFilterAcceptByKindOf(const KindOfMaskType& must, const KindOfMaskType&) { m_must = must; }
};
class PartitionManager
{
public:
	ObjectIterator* iteratePotentialCollisions(const Coord3D* pos, const GeometryInfo& geom, Real angle, Bool use2D = false);
	ObjectIterator* iterateObjectsInRange(const Coord3D* pos, Real maxDist, DistanceCalculationType dc, PartitionFilter** filters);
	Object* getClosestObject(const Coord3D* pos, Real maxDist, DistanceCalculationType dc, PartitionFilter** filters);
	Bool geomCollidesWithGeom(const Coord3D* pos1, const GeometryInfo& geom1, Real angle1, const Coord3D* pos2, const GeometryInfo& geom2, Real angle2) const;
	void worldToCell(Real wx, Real wy, Int* cx, Int* cy) const;
	CellShroudStatus getShroudStatusForPlayer(Int playerIndex, Int x, Int y) const;
};
extern PartitionManager* ThePartitionManager;

/* ---- the control bar, the audio, the game logic, the thing factory ----------------------------------------------- */
class CommandSet
{
public:
	const CommandButton* m_command[MAX_COMMANDS_PER_SET];
	const CommandButton* getCommandButton(Int i) const { return m_command[i]; }
};
class ControlBar { public: const CommandSet* findCommandSet(const AsciiString& name); };
extern ControlBar* TheControlBar;
class AudioManager { public: void addAudioEvent(const AudioEventRTS* e); };
extern AudioManager* TheAudio;
class GameLogic
{
public:
	UnsignedInt getFrame() const;
	Object* findObjectByID(ObjectID id);
	void destroyObject(Object* obj);
	void deselectObject(Object* obj, PlayerMaskType mask, Bool affectClient);
	Object* getFirstObject();
};
extern GameLogic* TheGameLogic;
class ThingFactory { public: Object* newObject(const ThingTemplate* tmplate, Team* team, ObjectStatusMaskType status); };
extern ThingFactory* TheThingFactory;

#endif
