/* Differential-test wrapper for TerrainLogic (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Map/TerrainLogic.cpp
** (#included below through a copy that build.rs writes with ONE mechanical edit, see
** below; its headers are this build's, see build.rs and differential/shim_terrain_logic).
**
** The edit: `static const ThingTemplate* genericBridgeTemplate = TheThingFactory->
** findTemplate("GenericBridge");` in Bridge::Bridge(info, props, name) loses its `static`
** (asserted to occur once). Upstream's static asks the thing factory once per process and
** keeps the answer; the Rust asks the world each time, which is the same while the template
** set does not change (true of the shipped data). A static would make every case after the
** first in this process see the first case's answer.
**
** What is real here: TerrainLogic.cpp and TerrainLogic.h; the chunk reader
** (DataChunkInput, compiled in the chunk build); the Xfer base class; the
** WWMath Matrix3D, Vector3, PlaneClass (plane.h) and Point_In_Triangle_2D
** (tri.h); BaseType.h; Trig.cpp's Cos/Sin; and, extracted verbatim by
** build.rs (OUT_DIR/terrain_logic_gen), the enums and constants of Damage.h,
** BodyModule.h, PartitionManager.h, TerrainRoads.h, Geometry.h, MapObject.h,
** AIPathfind.h, Scripts.h and GameCommon.h, the W3DTerrainLogic methods
** getExtent .. getLayerHeight, BaseHeightMapRenderObjClass::getHeightMapHeight
** and getClipHeight, and PolygonTrigger::updateBounds and pointInTrigger.
**
** What is a stand-in (differential/shim_terrain_logic/terrain_logic_world.h
** declares them; the definitions are below): the object world, the
** ThingFactory, TerrainRoads, the pathfinder, the partition manager, the ghost
** objects, the radar, the rest of PolygonTrigger, MapObject, GlobalData, and
** the TerrainVisual. They answer from the scenario the test installs and LOG
** every call that crosses the cut (the world log); the TerrainVisual stand-in
** logs its writes separately (the visual log) and implements the members of
** W3DTerrainVisual and WaterRenderObjClass that the reads depend on, written
** from their source (W3DTerrainVisual.cpp, W3DWater.cpp/.h, WorldHeightMap.h)
** and NOT compiled from it: that part is a second transcription, which the
** differential compares the first (src/logic/terrain_visual.rs) against.
**
** Entry point (extern "C", tests/differential_terrain_logic.rs, which
** documents the encodings): ref_terrain_logic_run installs a scenario, runs a
** program of operations on one TerrainLogic (the base class, or a W3D-style
** subclass whose overrides are the extracted W3DTerrainLogic methods), and
** writes, per operation, its results, the world log and the visual log. */
#include "PreRTS.h"

#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <string>
#include <vector>
#include <list>
#include <map>
#include <algorithm>

#include "Common/DataChunk.h"
#include "Common/MapReaderWriterInfo.h"
#include "Common/WellKnownKeys.h"
#include "Common/Xfer.h"
#include "Common/GameState.h"
#include "GameLogic/TerrainLogic.h"

/* GeometryInfo::set, verbatim (geometry_set.inc), as the stand-in's member */
#define GeometryInfo TLGeometryInfo
#include "geometry_set.inc"
#undef GeometryInfo

/* ------------------------------------------------------------------------- */
/* The scenario                                                               */
/* ------------------------------------------------------------------------- */

namespace {

struct TLObj
{
	bool exists;		/* findObjectByID finds it */
	Int id;
	Coord3D pos;
	Real orient;
	Int layer;
	TLGeometryInfo geom;
	std::string tmplName;
	Int bodyState;
	Bool indestructible;
	Int drawable;		/* 0: no drawable */
	Int scaffold;		/* 0: no BridgeBehaviorInterface; 1: scaffold absent; 2: present */
	TLObj() : exists(true), id(0), orient(0), layer(1), bodyState(0), indestructible(false), drawable(0), scaffold(0) { pos.x = pos.y = pos.z = 0; }
};
struct TLMapObj
{
	bool isWaypoint;
	Coord3D loc;
	Int wpId;
	std::string name;
	std::string label[3];
	bool hasLabel[3];
	Int biDir;		/* 0: no such property; 1: false; 2: true */
};
struct TLTrig
{
	std::string name;
	Int id;
	bool water;
	std::vector<ICoord3D> pts;
};

struct TLScenario
{
	std::vector<std::string> strings;
	bool w3d;
	bool hasHeightMap;
	bool renderObject;
	bool genericBridgeTemplate, roadTemplate;
	unsigned towerMask;
	Real towerMajor[4];
	Real wallHeight;
	bool wallOn;
	Real wallX0, wallY0, wallX1, wallY1;
	UnsignedInt frame;
	Int nextObjectId;
	Int nextLayer;
	std::vector<TLObj> objects;
	std::vector<TLTrig> triggers;
	std::vector<TLMapObj> mapObjects;
	bool fileOpens;
	std::vector<unsigned char> fileBytes;
	std::vector<unsigned> log, vlog;
};
TLScenario g_tl;

unsigned bitsOf(float f) { unsigned u; memcpy(&u, &f, 4); return u; }
float floatOf(unsigned u) { float f; memcpy(&f, &u, 4); return f; }
unsigned hashOf(const char *s)
{
	unsigned h = 2166136261u;
	for (; *s; ++s) { h ^= (unsigned char)*s; h *= 16777619u; }
	return h;
}
void L(int code, std::initializer_list<unsigned> w)
{
	g_tl.log.push_back((unsigned)code);
	g_tl.log.push_back((unsigned)w.size());
	for (unsigned x : w) g_tl.log.push_back(x);
}
void V(int code, std::initializer_list<unsigned> w)
{
	g_tl.vlog.push_back((unsigned)code);
	g_tl.vlog.push_back((unsigned)w.size());
	for (unsigned x : w) g_tl.vlog.push_back(x);
}

enum { MAX_OBJ = 64, MAX_TRIG = 16 };
TLObject g_handle[MAX_OBJ];
struct DrawableSlot { int id; } g_draw[MAX_OBJ];
TLBodyModuleInterface g_body[MAX_OBJ];
TLBridgeBehaviorInterface g_bbi[MAX_OBJ];
TLBridgeTowerBehaviorInterface g_btbi[MAX_OBJ];
TLPolygonTrigger g_trig[MAX_TRIG];
TLMapObject g_mapObj[MAX_OBJ];
TLThingTemplate g_tmpl[8];
TLTerrainRoadType g_roadType;

TLObj &O(int index) { return g_tl.objects[index]; }

}  // namespace

/* ------------------------------------------------------------------------- */
/* Stand-ins: objects and the object world                                    */
/* ------------------------------------------------------------------------- */

ObjectID TLObject::getID() const { return (ObjectID)O(m_index).id; }
const Coord3D *TLObject::getPosition() const { return &O(m_index).pos; }
Real TLObject::getOrientation() const { return O(m_index).orient; }
void TLObject::setPosition(const Coord3D *pos)
{
	L(42, {(unsigned)O(m_index).id, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z)});
	O(m_index).pos = *pos;
}
void TLObject::setOrientation(Real angle)
{
	L(43, {(unsigned)O(m_index).id, bitsOf(angle)});
	O(m_index).orient = angle;
}
PathfindLayerEnum TLObject::getLayer() const { return (PathfindLayerEnum)O(m_index).layer; }
const TLGeometryInfo &TLObject::getGeometryInfo() const { return O(m_index).geom; }
const TLThingTemplate *TLObject::getTemplate() const
{
	g_tmpl[7].m_name = AsciiString(O(m_index).tmplName.c_str());
	return &g_tmpl[7];
}
TLObject *TLObject::getNextObject()
{
	for (int i = m_index + 1; i < (int)g_tl.objects.size(); ++i)
		if (O(i).exists) return &g_handle[i];
	return 0;
}
TLDrawable *TLObject::getDrawable() const
{
	L(13, {(unsigned)O(m_index).id, (unsigned)O(m_index).drawable});
	if (O(m_index).drawable == 0) return 0;
	g_draw[m_index].id = O(m_index).drawable;
	return (TLDrawable *)&g_draw[m_index];
}
TLBodyModuleInterface *TLObject::getBodyModule() const
{
	g_body[m_index].m_objIndex = m_index;
	return &g_body[m_index];
}
void TLObject::attemptDamage(TLDamageInfo *d)
{
	L(14, {(unsigned)O(m_index).id, (unsigned)d->in.m_damageType, (unsigned)d->in.m_deathType, (unsigned)d->in.m_sourceID, bitsOf(d->in.m_amount)});
}
void TLObject::updateObjValuesFromMapProperties(Dict *) { L(44, {(unsigned)O(m_index).id}); }
void TLObject::friend_prepareForMapBoundaryAdjust(void) { L(22, {(unsigned)O(m_index).id}); }
void TLObject::friend_notifyOfNewMapBoundary(void) { L(29, {(unsigned)O(m_index).id}); }

BodyDamageType TLBodyModuleInterface::getDamageState(void) const
{
	L(11, {(unsigned)O(m_objIndex).id});
	return (BodyDamageType)O(m_objIndex).bodyState;
}
Bool TLBodyModuleInterface::isIndestructible(void) const { return O(m_objIndex).indestructible; }
void TLBodyModuleInterface::setIndestructible(Bool indestructible)
{
	L(48, {(unsigned)O(m_objIndex).id, (unsigned)indestructible});
	O(m_objIndex).indestructible = indestructible;
}

TLBridgeBehaviorInterface *TLBridgeBehavior::getBridgeBehaviorInterfaceFromObject(TLObject *obj)
{
	L(12, {(unsigned)obj->getID(), (unsigned)O(obj->m_index).scaffold});
	if (O(obj->m_index).scaffold == 0) return 0;
	g_bbi[obj->m_index].m_objIndex = obj->m_index;
	return &g_bbi[obj->m_index];
}
void TLBridgeBehaviorInterface::setTower(BridgeTowerType towerType, TLObject *tower)
{
	L(45, {(unsigned)O(m_objIndex).id, (unsigned)towerType, (unsigned)tower->getID()});
}
Bool TLBridgeBehaviorInterface::isScaffoldPresent(void) { return O(m_objIndex).scaffold == 2; }
TLBridgeTowerBehaviorInterface *TLBridgeTowerBehavior::getBridgeTowerBehaviorInterfaceFromObject(TLObject *obj)
{
	L(49, {(unsigned)obj->getID()});
	g_btbi[obj->m_index].m_objIndex = obj->m_index;
	return &g_btbi[obj->m_index];
}
void TLBridgeTowerBehaviorInterface::setBridge(TLObject *bridge) { L(46, {(unsigned)O(m_objIndex).id, (unsigned)bridge->getID()}); }
void TLBridgeTowerBehaviorInterface::setTowerType(BridgeTowerType type) { L(47, {(unsigned)O(m_objIndex).id, (unsigned)type}); }

static TLThingFactory g_factory;
TLThingFactory *TLTheThingFactory = &g_factory;
const TLThingTemplate *TLThingFactory::findTemplate(const AsciiString &name, Bool)
{
	const char *n = name.str();
	bool found = false;
	int slot = 0;
	if (strcmp(n, "GenericBridge") == 0) {
		found = g_tl.genericBridgeTemplate;
		slot = 4;
	} else if (strncmp(n, "BridgeTower", 11) == 0 && n[11] >= '0' && n[11] <= '3' && n[12] == 0) {
		int t = n[11] - '0';
		found = (g_tl.towerMask >> t) & 1;
		slot = t;
		g_tmpl[slot].m_geom = TLGeometryInfo(GEOMETRY_BOX, false, 1.0f, g_tl.towerMajor[t], 1.0f);
	}
	L(40, {hashOf(n), (unsigned)found});
	if (!found) return 0;
	g_tmpl[slot].m_name = name;
	return &g_tmpl[slot];
}
TLObject *TLThingFactory::newObject(const TLThingTemplate *tmplate, TLTeam *)
{
	TLObj o;
	o.id = g_tl.nextObjectId++;
	o.tmplName = tmplate->getName().str();
	o.scaffold = 1;
	g_tl.objects.push_back(o);
	int index = (int)g_tl.objects.size() - 1;
	g_handle[index].m_index = index;
	L(41, {hashOf(tmplate->getName().str()), (unsigned)o.id});
	return &g_handle[index];
}

static TLGameLogic g_gameLogic;
TLGameLogic *TLTheGameLogic = &g_gameLogic;
UnsignedInt TLGameLogic::getFrame(void)
{
	L(18, {g_tl.frame});
	return g_tl.frame;
}
TLObject *TLGameLogic::findObjectByID(ObjectID id)
{
	bool found = false;
	int index = -1;
	if (id != 0)
		for (int i = 0; i < (int)g_tl.objects.size(); ++i)
			if (O(i).exists && O(i).id == (Int)id) { found = true; index = i; break; }
	L(10, {(unsigned)id, (unsigned)found});
	return found ? &g_handle[index] : 0;
}
TLObject *TLGameLogic::getFirstObject(void)
{
	L(17, {});
	for (int i = 0; i < (int)g_tl.objects.size(); ++i)
		if (O(i).exists) return &g_handle[i];
	return 0;
}
void TLGameLogic::destroyObject(TLObject *obj)
{
	L(15, {(unsigned)obj->getID()});
	O(obj->m_index).exists = false;
}

/* ---- the pathfinder */
static TLAI g_ai;
TLAI *TLTheAI = &g_ai;
PathfindLayerEnum TLPathfinder::addBridge(Bridge *theBridge)
{
	const Region2D *b = theBridge->getBounds();
	PathfindLayerEnum layer = (PathfindLayerEnum)g_tl.nextLayer++;
	L(5, {bitsOf(b->lo.x), bitsOf(b->lo.y), bitsOf(b->hi.x), bitsOf(b->hi.y), (unsigned)theBridge->peekBridgeInfo()->bridgeObjectID, (unsigned)layer});
	return layer;
}
void TLPathfinder::changeBridgeState(PathfindLayerEnum layer, Bool repaired) { L(6, {(unsigned)layer, (unsigned)repaired}); }
void TLPathfinder::forceMapRecalculation() { L(7, {}); }
Real TLPathfinder::getWallHeight(void) const
{
	L(8, {});
	return g_tl.wallHeight;
}
Bool TLPathfinder::isPointOnWall(const Coord3D *pos)
{
	bool on = g_tl.wallOn && pos->x >= g_tl.wallX0 && pos->x <= g_tl.wallX1 && pos->y >= g_tl.wallY0 && pos->y <= g_tl.wallY1;
	L(9, {bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z), (unsigned)on});
	return on;
}

/* ---- the bridge templates */
static TLTerrainRoads g_roads;
TLTerrainRoads *TLTheTerrainRoads = &g_roads;
TLTerrainRoadType *TLTerrainRoads::findBridge(AsciiString name)
{
	L(50, {hashOf(name.str()), (unsigned)g_tl.roadTemplate});
	if (!g_tl.roadTemplate) return 0;
	g_roadType.m_bridgeName = name;
	return &g_roadType;
}
AsciiString TLTerrainRoadType::getTowerObjectName(BridgeTowerType tower)
{
	char buf[16];
	snprintf(buf, sizeof buf, "BridgeTower%d", (int)tower);
	return AsciiString(buf);
}

/* ---- the partition manager, ghost objects, radar, view */
namespace {
class TLRangeIterator : public TLObjectIterator
{
public:
	std::vector<int> m_items;
	size_t m_next;
	TLRangeIterator() : m_next(0) {}
	virtual TLObject *first() { m_next = 0; return next(); }
	virtual TLObject *next() { return m_next < m_items.size() ? &g_handle[m_items[m_next++]] : 0; }
};
}  // namespace
static TLPartitionManager g_pm;
TLPartitionManager *TLThePartitionManager = &g_pm;
TLObjectIterator *TLPartitionManager::iterateObjectsInRange(const Coord3D *center, Real maxDist, DistanceCalculationType dc, PartitionFilter **)
{
	L(16, {bitsOf(center->x), bitsOf(center->y), bitsOf(center->z), bitsOf(maxDist), (unsigned)dc});
	TLRangeIterator *it = new TLRangeIterator;
	for (int i = 0; i < (int)g_tl.objects.size(); ++i) {
		if (!O(i).exists) continue;
		Real dx = O(i).pos.x - center->x;
		Real dy = O(i).pos.y - center->y;
		if (dx * dx + dy * dy <= maxDist * maxDist) it->m_items.push_back(i);
	}
	return it;
}
void TLPartitionManager::processEntirePendingUndoShroudRevealQueue() { L(19, {}); }
void TLPartitionManager::storeFoggedCells(TLShroudStatusStoreRestore &, Bool storeClear) { L(20, {(unsigned)storeClear}); }
void TLPartitionManager::restoreFoggedCells(const TLShroudStatusStoreRestore &, Bool restoreClear) { L(27, {(unsigned)restoreClear}); }
void TLPartitionManager::reset() { L(24, {}); }
void TLPartitionManager::init() { L(25, {}); }
static TLGhostObjectManager g_ghost;
TLGhostObjectManager *TLTheGhostObjectManager = &g_ghost;
void TLGhostObjectManager::releasePartitionData(void) { L(21, {}); }
void TLGhostObjectManager::lockGhostObjects(Bool lock) { L(28, {(unsigned)lock}); }
void TLGhostObjectManager::restorePartitionData(void) { L(31, {}); }
static TLRadar g_radar;
TLRadar *TLTheRadar = &g_radar;
void TLRadar::newMap(TerrainLogic *terrain) { L(26, {(unsigned)terrain->getActiveBoundary()}); }
static TLView g_view;
TLView *TLTheTacticalView = &g_view;
void TLView::forceCameraConstraintRecalc(void) { L(33, {}); }

/* ---- the global data */
static TLGlobalData g_gd;
TLGlobalData *TLTheGlobalData = &g_gd;

/* ------------------------------------------------------------------------- */
/* Stand-ins: MapObject, PolygonTrigger                                       */
/* ------------------------------------------------------------------------- */

AsciiString TLMapProps::getAsciiString(NameKeyType key, Bool *exists) const
{
	const TLMapObj &m = g_tl.mapObjects[m_mapObjIndex];
	NameKeyType keys[3] = { TheKey_waypointPathLabel1, TheKey_waypointPathLabel2, TheKey_waypointPathLabel3 };
	for (int i = 0; i < 3; ++i)
		if (key == keys[i]) {
			if (exists) *exists = m.hasLabel[i];
			return m.hasLabel[i] ? AsciiString(m.label[i].c_str()) : AsciiString::TheEmptyString;
		}
	if (key == TheKey_waypointName) { if (exists) *exists = true; return AsciiString(m.name.c_str()); }
	if (exists) *exists = false;
	return AsciiString::TheEmptyString;
}
Bool TLMapProps::getBool(NameKeyType key, Bool *exists) const
{
	const TLMapObj &m = g_tl.mapObjects[m_mapObjIndex];
	if (key == TheKey_waypointPathBiDirectional) {
		if (exists) *exists = (m.biDir != 0);
		return m.biDir == 2;
	}
	if (exists) *exists = false;
	return false;
}
const Coord3D *TLMapObject::getLocation(void) const { return &g_tl.mapObjects[m_index].loc; }
TLMapObject *TLMapObject::getNext(void) const
{
	return m_index + 1 < (int)g_tl.mapObjects.size() ? &g_mapObj[m_index + 1] : 0;
}
Bool TLMapObject::isWaypoint(void) const { return g_tl.mapObjects[m_index].isWaypoint; }
TLWaypointIdValue TLMapObject::getWaypointID() { TLWaypointIdValue v = { g_tl.mapObjects[m_index].wpId }; return v; }
AsciiString TLMapObject::getWaypointName() { return AsciiString(g_tl.mapObjects[m_index].name.c_str()); }
TLMapObject *TLMapObject::getFirstMapObject(void) { return g_tl.mapObjects.empty() ? 0 : &g_mapObj[0]; }

/* The polygon trigger: the members TerrainLogic.cpp calls, over the scenario's
** triggers. updateBounds and pointInTrigger are PolygonTrigger.cpp's own,
** extracted verbatim (polygon_trigger_methods.inc). */
namespace {
struct TLPolyData
{
	Int m_numPoints;
	ICoord3D *m_points;
	mutable IRegion2D m_bounds;
	mutable Real m_radius;
	mutable Bool m_boundsNeedsUpdate;
};
}  // namespace
/* the members of PolygonTrigger the two extracted functions use, kept beside the handle */
static std::vector<TLPolyData> g_polyData(MAX_TRIG);
#define PT_DATA g_polyData[m_index]
#define m_numPoints PT_DATA.m_numPoints
#define m_points PT_DATA.m_points
#define m_bounds PT_DATA.m_bounds
#define m_radius PT_DATA.m_radius
#define m_boundsNeedsUpdate PT_DATA.m_boundsNeedsUpdate
#include "polygon_trigger_methods.inc"
#undef m_numPoints
#undef m_points
#undef m_bounds
#undef m_radius
#undef m_boundsNeedsUpdate
#undef PT_DATA
/* (the extracted definitions are of PolygonTrigger::updateBounds and
** pointInTrigger, i.e. of TLPolygonTrigger's members: declared in the header) */

TLPolygonTrigger *TLPolygonTrigger::getFirstPolygonTrigger(void) { return g_tl.triggers.empty() ? 0 : &g_trig[0]; }
TLPolygonTrigger *TLPolygonTrigger::getPolygonTriggerByID(Int triggerID)
{
	for (int i = 0; i < (int)g_tl.triggers.size(); ++i)
		if (g_tl.triggers[i].id == triggerID) return &g_trig[i];
	return 0;
}
void TLPolygonTrigger::deleteTriggers(void) { g_tl.triggers.clear(); }
const ICoord3D *TLPolygonTrigger::getPoint(Int ndx) const
{
	if (ndx < 0) ndx = 0;
	if (ndx >= g_polyData[m_index].m_numPoints) ndx = g_polyData[m_index].m_numPoints - 1;
	return g_polyData[m_index].m_points + ndx;
}
void TLPolygonTrigger::setPoint(const ICoord3D &point, Int ndx)
{
	if (ndx < 0) return;
	if (ndx >= g_polyData[m_index].m_numPoints) return;
	g_polyData[m_index].m_points[ndx] = point;
	g_polyData[m_index].m_boundsNeedsUpdate = true;
}
Int TLPolygonTrigger::getNumPoints(void) const { return g_polyData[m_index].m_numPoints; }
Int TLPolygonTrigger::getID(void) const { return g_tl.triggers[m_index].id; }
TLPolygonTrigger *TLPolygonTrigger::getNext(void) { return m_index + 1 < (int)g_tl.triggers.size() ? &g_trig[m_index + 1] : 0; }
AsciiString TLPolygonTrigger::getTriggerName(void) const { return AsciiString(g_tl.triggers[m_index].name.c_str()); }
Bool TLPolygonTrigger::isWaterArea(void) const { return g_tl.triggers[m_index].water; }
const TLWaterHandle *TLPolygonTrigger::getWaterHandle(void) const
{
	if (isWaterArea()) return &m_waterHandle;
	return 0;
}

/* ------------------------------------------------------------------------- */
/* Stand-ins: the logic height map, the render object that reads it, and the   */
/* water render object (W3DTerrainVisual.cpp, W3DWater.cpp/.h)                 */
/* ------------------------------------------------------------------------- */

namespace {
WorldHeightMap g_hm;
bool g_hmPresent = false;

/* WaterRenderObjClass and W3DTerrainVisual members, from their source */
struct WaterState
{
	bool renderObject;			/* m_waterRenderObject != NULL */
	bool renderingEnabled;		/* m_isWaterGridRenderingEnabled */
	bool doWaterGrid;			/* m_doWaterGrid */
	Matrix3D transform;			/* Get_Transform() */
	Vector2 gridDirectionX, gridDirectionY, gridOrigin;
	Real gridCellSize;
	Int gridCellsX, gridCellsY;
	std::vector<Real> mesh;		/* m_meshData[].height; empty: m_meshData == NULL */
	bool hasMesh;
};
WaterState g_water;

void waterRenderEnable(bool state)
{
	/* WaterRenderObjClass::enableWaterGrid */
	g_water.doWaterGrid = state;
	if (state && !g_water.hasMesh) {
		Int size = (g_water.gridCellsX + 1 + 2) * (g_water.gridCellsY + 1 + 2);
		g_water.mesh.assign(size, 0.0f);
		g_water.hasMesh = true;
	}
}
bool worldToGridSpace(Real worldX, Real worldY, Real &gridX, Real &gridY)
{
	/* WaterRenderObjClass::worldToGridSpace (W3DWater.h) */
	Real dx, dy;
	Real ooGridCellSize = 1.0f / g_water.gridCellSize;
	dx = worldX - g_water.gridOrigin.X;
	dy = worldY - g_water.gridOrigin.Y;
	gridX = ooGridCellSize * (dx * g_water.gridDirectionX.X + dy * g_water.gridDirectionX.Y);
	gridY = ooGridCellSize * (dx * g_water.gridDirectionY.X + dy * g_water.gridDirectionY.Y);
	if (gridX < 0) return false;
	if (gridX > g_water.gridCellsX - 1) return false;
	if (gridY < 0) return false;
	if (gridY > g_water.gridCellsY - 1) return false;
	return true;
}
}  // namespace

void TLTerrainVisual::load(AsciiString filename) { V(60, {hashOf(filename.str())}); }
void TLTerrainVisual::enableWaterGrid(Bool enable)
{
	V(61, {(unsigned)enable});
	g_water.renderingEnabled = enable;
	if (g_water.renderObject) waterRenderEnable(enable);
}
void TLTerrainVisual::setWaterGridHeightClamps(const TLWaterHandle *, Real minZ, Real maxZ) { V(62, {bitsOf(minZ), bitsOf(maxZ)}); }
void TLTerrainVisual::setWaterAttenuationFactors(const TLWaterHandle *, Real a, Real b, Real c, Real range) { V(66, {bitsOf(a), bitsOf(b), bitsOf(c), bitsOf(range)}); }
void TLTerrainVisual::setWaterTransform(const TLWaterHandle *, Real angle, Real x, Real y, Real z)
{
	V(63, {bitsOf(angle), bitsOf(x), bitsOf(y), bitsOf(z)});
	if (!g_water.renderObject) return;
	/* WaterRenderObjClass::setGridTransform(angle, x, y, z) */
	g_water.gridDirectionX = Vector2(1.0f, 0.0f);
	g_water.gridOrigin.X = x;
	g_water.gridOrigin.Y = y;
	Matrix3D xform(1);
	xform.Rotate_Z(angle);
	g_water.gridDirectionX.X = xform.Get_X_Vector().X;
	g_water.gridDirectionX.Y = xform.Get_X_Vector().Y;
	g_water.gridDirectionY.X = xform.Get_Y_Vector().X;
	g_water.gridDirectionY.Y = xform.Get_Y_Vector().Y;
	xform.Set_Translation(Vector3(x, y, z));
	g_water.transform = xform;
}
void TLTerrainVisual::setWaterTransform(const Matrix3D *transform)
{
	unsigned w[12];
	for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) w[4 * r + c] = bitsOf((*transform)[r][c]);
	V(64, {w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7], w[8], w[9], w[10], w[11]});
	if (g_water.renderObject && transform) g_water.transform = *transform;
}
void TLTerrainVisual::getWaterTransform(const TLWaterHandle *, Matrix3D *transform)
{
	if (g_water.renderObject && transform) *transform = g_water.transform;
}
void TLTerrainVisual::setWaterGridResolution(const TLWaterHandle *, Real gridCellsX, Real gridCellsY, Real cellSize)
{
	V(65, {bitsOf(gridCellsX), bitsOf(gridCellsY), bitsOf(cellSize)});
	if (!g_water.renderObject) return;
	/* WaterRenderObjClass::setGridResolution */
	g_water.gridCellSize = cellSize;
	if (g_water.gridCellsX != gridCellsX || g_water.gridCellsY != g_water.gridCellsY) {
		g_water.gridCellsX = (Int)gridCellsX;
		g_water.gridCellsY = (Int)gridCellsY;
		if (g_water.hasMesh) {
			g_water.mesh.clear();
			g_water.hasMesh = false;
			bool enable = g_water.doWaterGrid;
			waterRenderEnable(true);
			g_water.doWaterGrid = enable;
		}
	}
}
void TLTerrainVisual::getWaterGridResolution(const TLWaterHandle *, Real *gridCellsX, Real *gridCellsY, Real *cellSize)
{
	if (!g_water.renderObject) return;
	if (gridCellsX) *gridCellsX = g_water.gridCellsX;
	if (gridCellsY) *gridCellsY = g_water.gridCellsY;
	if (cellSize) *cellSize = g_water.gridCellSize;
}
Bool TLTerrainVisual::getWaterGridHeight(Real worldX, Real worldY, Real *height)
{
	Real gridX, gridY;
	if (g_water.renderingEnabled && g_water.renderObject && worldToGridSpace(worldX, worldY, gridX, gridY)) {
		/* getGridVertexHeight(REAL_TO_INT(gridX), REAL_TO_INT(gridY), height) */
		Int x = REAL_TO_INT(gridX), y = REAL_TO_INT(gridY);
		if (g_water.hasMesh) *height = g_water.mesh[(y + 1) * (g_water.gridCellsX + 1 + 2) + x + 1] + g_water.transform.Get_Translation().Z;
		return TRUE;
	}
	return FALSE;
}
void TLTerrainVisual::setRawMapHeight(const ICoord2D *gridPos, Int height)
{
	V(67, {(unsigned)gridPos->x, (unsigned)gridPos->y, (unsigned)height});
	if (g_hmPresent) {
		Int x = gridPos->x + g_hm.getBorderSizeInline();
		Int y = gridPos->y + g_hm.getBorderSizeInline();
		if (g_hm.getHeight(x, y) > height) g_hm.setRawHeight(x, y, height);
	}
}
Int TLTerrainVisual::getRawMapHeight(const ICoord2D *gridPos)
{
	if (g_hmPresent) {
		Int x = gridPos->x + g_hm.getBorderSizeInline();
		Int y = gridPos->y + g_hm.getBorderSizeInline();
		return g_hm.getHeight(x, y);
	}
	return 0;
}
WorldHeightMap *TLTerrainVisual::getLogicHeightMap(void) { return g_hmPresent ? &g_hm : 0; }
static TLTerrainVisual g_visual;
TLTerrainVisual *TLTheTerrainVisual = &g_visual;

/* The render object whose getHeightMapHeight (BaseHeightMap.cpp) the W3D terrain
** logic asks, and whose getClipHeight (BaseHeightMap.h) it uses: both verbatim. */
class TLTerrainRenderObject
{
public:
	WorldHeightMap *m_map;
	TLTerrainRenderObject() : m_map(0) {}
	Real getHeightMapHeight(Real x, Real y, Coord3D *normal) const;
	WorldHeightMap *getMap() { return m_map; }
	Bool isClearLineOfSight(const Coord3D &, const Coord3D &) const { return false; }
#include "clip_height.inc"
};
static TLTerrainRenderObject g_renderObject;
static TLTerrainRenderObject *TheTerrainRenderObject = 0;
#define BaseHeightMapRenderObjClass TLTerrainRenderObject
#define fast_float2long_round ref_fast_float2long_round
#include "height_map_height.inc"
#undef BaseHeightMapRenderObjClass

/* ------------------------------------------------------------------------- */
/* The terrain logics under test                                              */
/* ------------------------------------------------------------------------- */

/* the file the chunk reader reads: TerrainLogic::loadMap opens a CachedFileInputStream by name */
namespace {
class TLStream : public CachedFileInputStream
{
public:
	Bool open(AsciiString)
	{
		if (!g_tl.fileOpens) return FALSE;
		int len = (int)g_tl.fileBytes.size();
		m_buffer = new char[len ? len : 1];
		if (len) memcpy(m_buffer, &g_tl.fileBytes[0], len);
		m_size = len;
		m_pos = 0;
		return TRUE;
	}
	void close(void) {}
};
}  // namespace
#define CachedFileInputStream TLStream
#include "TerrainLogic_nostatic.cpp"
#undef CachedFileInputStream

namespace {

struct TLWaterEntry { const WaterHandle *waterTable; Real changePerFrame, targetHeight, damageAmount, currentHeight; };

/* what both subclasses expose of the protected members */
#define TL_EXPOSE \
public: \
	void callXfer(Xfer *x) { xfer(x); } \
	void callLoadPost() { loadPostProcess(); } \
	const WaterHandle *gridHandle() const { return &m_gridWaterHandle; } \
	int numWaterToUpdate() const { return m_numWaterToUpdate; } \
	TLWaterEntry waterEntry(int i) const { TLWaterEntry e = { m_waterToUpdate[i].waterTable, m_waterToUpdate[i].changePerFrame, m_waterToUpdate[i].targetHeight, m_waterToUpdate[i].damageAmount, m_waterToUpdate[i].currentHeight }; return e; } \
	void callFindRect(const WaterHandle *h, Region3D *r) { findAxisAlignedBoundingRect(h, r); } \
	Bool waterGridEnabled() const { return m_waterGridEnabled; } \
	void setBoundaries(const std::vector<ICoord2D> &b, Int active) { m_boundaries = b; m_activeBoundary = active; } \
	void setMapFields(Int dx, Int dy) { m_mapDX = dx; m_mapDY = dy; } \
	Bridge *bridgeList() { return m_bridgeListHead; } \
	Waypoint *waypointList() { return m_waypointListHead; } \
	AsciiString filename() { return m_filenameString; }

class TLBaseTerrainLogic : public TerrainLogic
{
	TL_EXPOSE
};

/* W3DTerrainLogic: its overrides are the extracted methods (w3d_terrain_logic_methods.inc) */
class TLW3DTerrainLogic : public TerrainLogic
{
	TL_EXPOSE
public:
	Real m_mapMinZ, m_mapMaxZ;
	TLW3DTerrainLogic() : m_mapMinZ(0), m_mapMaxZ(1) {}
	virtual Real getGroundHeight( Real x, Real y, Coord3D* normal = NULL ) const;
	virtual Real getLayerHeight(Real x, Real y, PathfindLayerEnum layer, Coord3D* normal = NULL, Bool clip = true) const;
	virtual void getExtent( Region3D *extent ) const ;
	virtual void getMaximumPathfindExtent( Region3D *extent ) const;
	virtual void getExtentIncludingBorder( Region3D *extent ) const;
	virtual Bool isClearLineOfSight(const Coord3D& pos, const Coord3D& posOther) const;
};
}  // namespace
#define W3DTerrainLogic TLW3DTerrainLogic
#include "w3d_terrain_logic_methods.inc"
#undef W3DTerrainLogic

/* ------------------------------------------------------------------------- */
/* Xfer into memory                                                           */
/* ------------------------------------------------------------------------- */

namespace {
class TLMemXfer : public Xfer
{
public:
	std::vector<unsigned char> m_bytes;
	size_t m_pos;
	TLMemXfer(XferMode mode) : m_pos(0) { m_xferMode = mode; }
	virtual void open(AsciiString) {}
	virtual void close(void) {}
	virtual Int beginBlock(void) { return 0; }
	virtual void endBlock(void) {}
	virtual void skip(Int) {}
	virtual void xferSnapshot(Snapshot *) {}
protected:
	virtual void xferImplementation(void *data, Int dataSize)
	{
		if (m_xferMode == XFER_SAVE) {
			m_bytes.insert(m_bytes.end(), (unsigned char *)data, (unsigned char *)data + dataSize);
		} else {
			if (m_pos + dataSize > m_bytes.size()) throw XFER_READ_ERROR;
			memcpy(data, &m_bytes[m_pos], dataSize);
			m_pos += dataSize;
		}
	}
};
}  // namespace

/* ------------------------------------------------------------------------- */
/* The driver                                                                 */
/* ------------------------------------------------------------------------- */

namespace {

struct Rd
{
	const unsigned *p;
	int n, i;
	unsigned u() { if (i >= n) abort(); return p[i++]; }
	int s() { return (int)u(); }
	float f() { return floatOf(u()); }
};

struct Out
{
	std::vector<unsigned> w;
	void u(unsigned v) { w.push_back(v); }
	void s(int v) { w.push_back((unsigned)v); }
	void f(float v) { w.push_back(bitsOf(v)); }
};

std::vector<unsigned char> g_lastSave;

const char *Str(int idx) { return g_tl.strings[idx].c_str(); }

int bridgeCount(TerrainLogic *t)
{
	int n = 0;
	for (Bridge *b = t->getFirstBridge(); b; b = b->getNext()) ++n;
	return n;
}
Bridge *bridgeAt(TerrainLogic *t, int i)
{
	Bridge *b = t->getFirstBridge();
	while (b && i-- > 0) b = b->getNext();
	return b;
}
int bridgeIndexOf(TerrainLogic *t, Bridge *which)
{
	int i = 0;
	for (Bridge *b = t->getFirstBridge(); b; b = b->getNext(), ++i)
		if (b == which) return i;
	return -1;
}
int waypointIndexOf(TerrainLogic *t, Waypoint *which)
{
	int i = 0;
	for (Waypoint *w = t->getFirstWaypoint(); w; w = w->getNext(), ++i)
		if (w == which) return i;
	return -1;
}
int triggerIndexOf(PolygonTrigger *which)
{
	int i = 0;
	for (PolygonTrigger *p = PolygonTrigger::getFirstPolygonTrigger(); p; p = p->getNext(), ++i)
		if (p == which) return i;
	return -1;
}

/* a water handle as the scenario codes it: -1 NULL, -2 the grid, else the trigger's position */
int waterCode(const WaterHandle *h, const WaterHandle *grid)
{
	if (!h) return -1;
	if (h == grid) return -2;
	return h->m_polygon->m_index;
}

unsigned heightMapDigest()
{
	unsigned h = 2166136261u;
	if (!g_hmPresent) return h;
	for (size_t i = 0; i < g_hm.m_data.size(); ++i) { h ^= g_hm.m_data[i]; h *= 16777619u; }
	return h;
}

void putCoord(Out &o, const Coord3D &c) { o.f(c.x); o.f(c.y); o.f(c.z); }

/* Object k of the scenario as TerrainLogic.cpp is handed it */
TLObject *objectAt(int k) { return &g_handle[k]; }

}  // namespace

extern "C" {

/* ref_terrain_logic_run(scenario, nScenario, program, nProgram, out, cap): see
** tests/differential_terrain_logic.rs for the encodings. Returns the number of
** words written, or -1 if they do not fit. */
int ref_terrain_logic_run(const unsigned *scn, int nScn, const unsigned *prog, int nProg, unsigned *out, int cap)
{
	g_tl = TLScenario();
	g_lastSave.clear();
	g_hmPresent = false;
	g_hm = WorldHeightMap();
	g_water = WaterState();
	g_tl.objects.clear();
	g_tl.objects.reserve(MAX_OBJ);
	for (int i = 0; i < MAX_OBJ; ++i) g_handle[i].m_index = i;
	for (int i = 0; i < MAX_OBJ; ++i) { g_mapObj[i].m_index = i; g_mapObj[i].m_props.m_mapObjIndex = i; }
	for (int i = 0; i < MAX_TRIG; ++i) { g_trig[i].m_index = i; g_trig[i].m_waterHandle.m_polygon = &g_trig[i]; }

	/* the name key generator the well-known keys use (process-wide, as upstream's) */
	if (!TheNameKeyGenerator) { TheNameKeyGenerator = new NameKeyGenerator; TheNameKeyGenerator->init(); }

	Rd r = { scn, nScn, 0 };
	/* ---- the scenario */
	int nstr = r.s();
	for (int i = 0; i < nstr; ++i) {
		int len = r.s();
		std::string s;
		for (int k = 0; k < len; ++k) s.push_back((char)r.u());
		g_tl.strings.push_back(s);
	}
	g_tl.w3d = r.u();
	g_tl.hasHeightMap = r.u();
	if (g_tl.hasHeightMap) {
		g_hm.m_width = r.s();
		g_hm.m_height = r.s();
		g_hm.m_borderSize = r.s();
		int nb = r.s();
		for (int i = 0; i < nb; ++i) { ICoord2D c; c.x = r.s(); c.y = r.s(); g_hm.m_boundaries.push_back(c); }
		g_hm.m_dataSize = g_hm.m_width * g_hm.m_height;
		for (int i = 0; i < g_hm.m_dataSize; ++i) g_hm.m_data.push_back((UnsignedByte)r.u());
		g_hmPresent = true;
	}
	g_tl.renderObject = r.u();
	g_water.renderObject = g_tl.renderObject;
	g_water.renderingEnabled = false;
	g_water.doWaterGrid = false;
	g_water.transform.Make_Identity();
	g_water.gridDirectionX = Vector2(1.0f, 0.0f);
	g_water.gridDirectionY = Vector2(1.0f, 0.0f);
	g_water.gridOrigin = Vector2(0, 0);
	g_water.gridCellSize = MAP_XY_FACTOR;
	g_water.gridCellsX = 128;
	g_water.gridCellsY = 128;
	g_water.hasMesh = false;
	/* terrain boundaries and extent fields (what W3DTerrainLogic::loadMap copies) */
	int nbound = r.s();
	std::vector<ICoord2D> bounds;
	for (int i = 0; i < nbound; ++i) { ICoord2D c; c.x = r.s(); c.y = r.s(); bounds.push_back(c); }
	int active = r.s();
	int mapDX = r.s(), mapDY = r.s();
	float minZ = r.f(), maxZ = r.f();
	/* global data */
	g_gd.m_mapName = AsciiString(Str(r.s()));
	for (int i = 0; i < 4; ++i) {
		g_gd.m_vertexWaterAvailableMaps[i] = AsciiString(Str(r.s()));
		g_gd.m_vertexWaterHeightClampLow[i] = r.f();
		g_gd.m_vertexWaterHeightClampHi[i] = r.f();
		g_gd.m_vertexWaterAngle[i] = r.f();
		g_gd.m_vertexWaterXPosition[i] = r.f();
		g_gd.m_vertexWaterYPosition[i] = r.f();
		g_gd.m_vertexWaterZPosition[i] = r.f();
		g_gd.m_vertexWaterXGridCells[i] = r.s();
		g_gd.m_vertexWaterYGridCells[i] = r.s();
		g_gd.m_vertexWaterGridSize[i] = r.f();
		g_gd.m_vertexWaterAttenuationA[i] = r.f();
		g_gd.m_vertexWaterAttenuationB[i] = r.f();
		g_gd.m_vertexWaterAttenuationC[i] = r.f();
		g_gd.m_vertexWaterAttenuationRange[i] = r.f();
	}
	/* world */
	g_tl.genericBridgeTemplate = r.u();
	g_tl.roadTemplate = r.u();
	g_tl.towerMask = r.u();
	for (int i = 0; i < 4; ++i) g_tl.towerMajor[i] = r.f();
	g_tl.wallHeight = r.f();
	g_tl.wallOn = r.u();
	g_tl.wallX0 = r.f(); g_tl.wallY0 = r.f(); g_tl.wallX1 = r.f(); g_tl.wallY1 = r.f();
	g_tl.frame = r.u();
	g_tl.nextLayer = 2;
	int nobj = r.s();
	for (int i = 0; i < nobj; ++i) {
		TLObj o;
		o.id = r.s();
		o.pos.x = r.f(); o.pos.y = r.f(); o.pos.z = r.f();
		o.orient = r.f();
		o.layer = r.s();
		int gt = r.s();
		int small_ = r.s();
		float gh = r.f(), gmaj = r.f(), gmin = r.f();
		o.geom = TLGeometryInfo((GeometryType)gt, small_ != 0, gh, gmaj, gmin);
		o.tmplName = Str(r.s());
		o.bodyState = r.s();
		o.indestructible = r.u();
		o.drawable = r.s();
		o.scaffold = r.s();
		o.exists = r.u();
		g_tl.objects.push_back(o);
	}
	g_tl.nextObjectId = 1000;
	int ntrig = r.s();
	for (int i = 0; i < ntrig; ++i) {
		TLTrig t;
		t.name = Str(r.s());
		t.id = r.s();
		t.water = r.u();
		int np = r.s();
		for (int k = 0; k < np; ++k) { ICoord3D p; p.x = r.s(); p.y = r.s(); p.z = r.s(); t.pts.push_back(p); }
		g_tl.triggers.push_back(t);
	}
	static ICoord3D polyStore[MAX_TRIG][32];
	for (int i = 0; i < ntrig; ++i) {
		TLPolyData &d = g_polyData[i];
		d.m_numPoints = (Int)g_tl.triggers[i].pts.size();
		for (int k = 0; k < d.m_numPoints; ++k) polyStore[i][k] = g_tl.triggers[i].pts[k];
		d.m_points = polyStore[i];
		d.m_boundsNeedsUpdate = true;
		d.m_radius = 0;
	}
	int nmap = r.s();
	for (int i = 0; i < nmap; ++i) {
		TLMapObj m;
		m.isWaypoint = r.u();
		m.loc.x = r.f(); m.loc.y = r.f(); m.loc.z = r.f();
		m.wpId = r.s();
		m.name = Str(r.s());
		for (int k = 0; k < 3; ++k) {
			int si = r.s();
			m.hasLabel[k] = si >= 0;
			if (si >= 0) m.label[k] = Str(si);
		}
		m.biDir = r.s();
		g_tl.mapObjects.push_back(m);
	}
	g_tl.fileOpens = r.u();
	int nbytes = r.s();
	for (int i = 0; i < nbytes; ++i) g_tl.fileBytes.push_back((unsigned char)r.u());

	/* ---- the terrain */
	TerrainLogic *t;
	TLW3DTerrainLogic *w3d = 0;
	TLBaseTerrainLogic *base = 0;
	if (g_tl.w3d) { w3d = new TLW3DTerrainLogic; t = w3d; }
	else { base = new TLBaseTerrainLogic; t = base; }
	TheTerrainLogic = t;
	TheTerrainRenderObject = 0;
	if (g_tl.w3d) {
		if (g_hmPresent) { g_renderObject.m_map = &g_hm; TheTerrainRenderObject = &g_renderObject; }
		w3d->m_mapMinZ = minZ;
		w3d->m_mapMaxZ = maxZ;
	}
	if (g_tl.w3d) { w3d->setBoundaries(bounds, active); w3d->setMapFields(mapDX, mapDY); }
	else { base->setBoundaries(bounds, active); base->setMapFields(mapDX, mapDY); }
#define TCALL(expr) (g_tl.w3d ? w3d->expr : base->expr)
	const WaterHandle *grid = g_tl.w3d ? w3d->gridHandle() : base->gridHandle();

	std::vector<unsigned> result;
	Rd p = { prog, nProg, 0 };
	int nops = p.s();
	for (int opn = 0; opn < nops; ++opn) {
		int op = p.s();
		g_tl.log.clear();
		g_tl.vlog.clear();
		Out o;
		switch (op) {
		case 0: g_tl.frame = p.u(); break;
		case 1: {
			TLObj &ob = O(p.s());
			ob.bodyState = p.s();
			ob.scaffold = p.s();
			ob.exists = p.u();
			ob.pos.x = p.f(); ob.pos.y = p.f(); ob.pos.z = p.f();
			ob.orient = p.f();
			ob.layer = p.s();
			break;
		}
		case 2: {
			int x = p.s(), y = p.s();
			float v = p.f();
			/* setGridVertexHeight(x, y, value) */
			if (g_water.hasMesh) g_water.mesh[(y + 1) * (g_water.gridCellsX + 1 + 2) + x + 1] = v;
			break;
		}
		case 3: g_gd.m_mapName = AsciiString(Str(p.s())); break;
		case 10: {
			AsciiString fn(Str(p.s()));
			int query = p.s();
			o.u(t->loadMap(fn, query != 0));
			break;
		}
		case 11: t->newMap(false); break;
		case 12: {
			float x = p.f(), y = p.f();
			int want = p.s();
			Coord3D n;
			Real h = t->getGroundHeight(x, y, want ? &n : NULL);
			o.f(h);
			if (want) putCoord(o, n);
			break;
		}
		case 13: {
			float x = p.f(), y = p.f();
			int layer = p.s(), want = p.s(), clip = p.s();
			Coord3D n;
			Real h = t->getLayerHeight(x, y, (PathfindLayerEnum)layer, want ? &n : NULL, clip != 0);
			o.f(h);
			if (want) putCoord(o, n);
			break;
		}
		case 14: {
			int which = p.s();
			Region3D e;
			if (which == 2) {
				t->getExtentIncludingBorder(&e);
				/* its z is never written */
				o.f(e.lo.x); o.f(e.lo.y); o.f(e.hi.x); o.f(e.hi.y);
				break;
			}
			if (which == 0) t->getExtent(&e);
			else t->getMaximumPathfindExtent(&e);
			putCoord(o, e.lo);
			putCoord(o, e.hi);
			break;
		}
		case 15: { Coord3D c; c.x = p.f(); c.y = p.f(); c.z = p.f(); putCoord(o, t->findClosestEdgePoint(&c)); break; }
		case 16: { Coord3D c; c.x = p.f(); c.y = p.f(); c.z = p.f(); putCoord(o, t->findFarthestEdgePoint(&c)); break; }
		case 17: {
			float angle = p.f();
			Coord3D pos; pos.x = p.f(); pos.y = p.f(); pos.z = p.f();
			int stick = p.s();
			Matrix3D m;
			PathfindLayerEnum layer = t->alignOnTerrain(angle, pos, stick != 0, m);
			o.s(layer);
			for (int rr = 0; rr < 3; ++rr) for (int c = 0; c < 4; ++c) o.f(m[rr][c]);
			break;
		}
		case 20: {
			int n = 0;
			for (Waypoint *w = t->getFirstWaypoint(); w; w = w->getNext()) ++n;
			o.s(n);
			for (Waypoint *w = t->getFirstWaypoint(); w; w = w->getNext()) {
				o.s(w->getID());
				o.u(hashOf(w->getName().str()));
				putCoord(o, *w->getLocation());
				o.s(w->getNumLinks());
				for (int k = 0; k < w->getNumLinks(); ++k) o.s(waypointIndexOf(t, w->getLink(k)));
				o.u(hashOf(w->getPathLabel1().str()));
				o.u(hashOf(w->getPathLabel2().str()));
				o.u(hashOf(w->getPathLabel3().str()));
				o.u(w->getBiDirectional());
			}
			break;
		}
		case 21: o.s(waypointIndexOf(t, t->getWaypointByName(AsciiString(Str(p.s()))))); break;
		case 22: o.s(waypointIndexOf(t, t->getWaypointByID(p.u()))); break;
		case 23: {
			Coord3D c; c.x = p.f(); c.y = p.f(); c.z = p.f();
			o.s(waypointIndexOf(t, t->getClosestWaypointOnPath(&c, AsciiString(Str(p.s())))));
			break;
		}
		case 24: {
			int wi = p.s();
			Waypoint *w = 0;
			if (wi >= 0) { w = t->getFirstWaypoint(); for (int k = 0; k < wi && w; ++k) w = w->getNext(); }
			o.u(t->isPurposeOfPath(w, AsciiString(Str(p.s()))));
			break;
		}
		case 25: o.s(triggerIndexOf(t->getTriggerAreaByName(AsciiString(Str(p.s()))))); break;
		case 30: {
			BridgeInfo info;
			info.from.x = p.f(); info.from.y = p.f(); info.from.z = p.f();
			info.to.x = p.f(); info.to.y = p.f(); info.to.z = p.f();
			info.bridgeWidth = p.f();
			info.fromLeft.x = p.f(); info.fromLeft.y = p.f(); info.fromLeft.z = p.f();
			info.fromRight.x = p.f(); info.fromRight.y = p.f(); info.fromRight.z = p.f();
			info.toLeft.x = p.f(); info.toLeft.y = p.f(); info.toLeft.z = p.f();
			info.toRight.x = p.f(); info.toRight.y = p.f(); info.toRight.z = p.f();
			info.bridgeIndex = p.s();
			info.bridgeObjectID = (ObjectID)p.s();
			AsciiString nm(Str(p.s()));
			t->addBridgeToLogic(&info, NULL, nm);
			break;
		}
		case 31: t->addLandmarkBridgeToLogic(objectAt(p.s())); break;
		case 32: {
			o.s(bridgeCount(t));
			for (Bridge *b = t->getFirstBridge(); b; b = b->getNext()) {
				BridgeInfo i;
				b->getBridgeInfo(&i);
				o.u(hashOf(b->getBridgeTemplateName().str()));
				o.s(b->getLayer());
				putCoord(o, i.from); putCoord(o, i.to);
				o.f(i.bridgeWidth);
				putCoord(o, i.fromLeft); putCoord(o, i.fromRight); putCoord(o, i.toLeft); putCoord(o, i.toRight);
				o.s(i.bridgeIndex);
				o.s(i.curDamageState);
				o.s(i.bridgeObjectID);
				for (int k = 0; k < 4; ++k) o.s(i.towerObjectID[k]);
				o.u(i.damageStateChanged);
				const Region2D *rg = b->getBounds();
				o.f(rg->lo.x); o.f(rg->lo.y); o.f(rg->hi.x); o.f(rg->hi.y);
			}
			break;
		}
		case 33: { Coord3D c; c.x = p.f(); c.y = p.f(); c.z = p.f(); o.s(bridgeIndexOf(t, t->findBridgeAt(&c))); break; }
		case 34: {
			Coord3D c; c.x = p.f(); c.y = p.f(); c.z = p.f();
			int layer = p.s(), clip = p.s();
			o.s(bridgeIndexOf(t, t->findBridgeLayerAt(&c, (PathfindLayerEnum)layer, clip != 0)));
			break;
		}
		case 35: { Coord3D c; c.x = p.f(); c.y = p.f(); c.z = p.f(); o.s(t->getLayerForDestination(&c)); break; }
		case 36: { Coord3D c; c.x = p.f(); c.y = p.f(); c.z = p.f(); int h = p.s(); o.s(t->getHighestLayerForDestination(&c, h != 0)); break; }
		case 37: { int k = p.s(), layer = p.s(), health = p.s(); o.u(t->objectInteractsWithBridgeLayer(objectAt(k), layer, health != 0)); break; }
		case 38: { int k = p.s(), layer = p.s(); o.u(t->objectInteractsWithBridgeEnd(objectAt(k), layer)); break; }
		case 39: t->updateBridgeDamageStates(); o.u(t->anyBridgesDamageStatesChanged()); break;
		case 41: {
			int k = p.s();
			Object *ob = k >= 0 ? objectAt(k) : NULL;
			o.u(t->isBridgeRepaired(ob));
			o.u(t->isBridgeBroken(ob));
			break;
		}
		case 43: {
			int k = p.s();
			TBridgeAttackInfo ai;
			t->getBridgeAttackPoints(objectAt(k), &ai);
			putCoord(o, ai.attackPoint1);
			putCoord(o, ai.attackPoint2);
			break;
		}
		case 44: {
			Vector3 from, to, pos(0, 0, 0);
			from.X = p.f(); from.Y = p.f(); from.Z = p.f();
			to.X = p.f(); to.Y = p.f(); to.Z = p.f();
			Drawable *d = t->pickBridge(from, to, &pos);
			o.s(d ? ((DrawableSlot *)d)->id : 0);
			o.f(pos.X); o.f(pos.Y); o.f(pos.Z);
			break;
		}
		case 45: t->deleteBridge(bridgeAt(t, p.s())); break;
		case 46: TCALL(callLoadPost()); break;
		case 47: {
			int bi = p.s();
			Coord3D c; c.x = p.f(); c.y = p.f(); c.z = p.f();
			int nb = bridgeCount(t);
			if (nb == 0) break;
			Bridge *b = bridgeAt(t, (int)((unsigned)bi % (unsigned)nb));
			o.u(b->isPointOnBridge(&c));
			Coord3D n;
			o.f(b->getBridgeHeight(&c, &n));
			putCoord(o, n);
			break;
		}
		case 48: {
			int bi = p.s();
			Region2D rg;
			rg.lo.x = p.f(); rg.lo.y = p.f(); rg.hi.x = p.f(); rg.hi.y = p.f();
			int nb = bridgeCount(t);
			if (nb == 0) break;
			Bridge *b = bridgeAt(t, (int)((unsigned)bi % (unsigned)nb));
			o.u(b->isCellOnEnd(&rg));
			o.u(b->isCellOnSide(&rg));
			o.u(b->isCellEntryPoint(&rg));
			break;
		}
		case 50: { float x = p.f(), y = p.f(); o.s(waterCode(t->getWaterHandle(x, y), grid)); break; }
		case 51: {
			float x = p.f(), y = p.f();
			Real wz = floatOf(0x7fc12345u), tz = floatOf(0x7fc54321u);
			Bool u = t->isUnderwater(x, y, &wz, &tz);
			o.u(u);
			o.u(bitsOf(wz) != 0x7fc12345u);	/* waterZ written */
			o.f(wz);
			o.f(tz);
			break;
		}
		case 52: o.s(waterCode(t->getWaterHandleByName(AsciiString(Str(p.s()))), grid)); break;
		case 53: {
			int code = p.s();
			const WaterHandle *h = code == -1 ? NULL : code == -2 ? grid : g_trig[code].getWaterHandle();
			o.f(t->getWaterHeight(h));
			break;
		}
		case 54: {
			int code = p.s();
			float h = p.f(), dmg = p.f();
			int force = p.s();
			const WaterHandle *wh = code == -1 ? NULL : code == -2 ? grid : g_trig[code].getWaterHandle();
			t->setWaterHeight(wh, h, dmg, force != 0);
			break;
		}
		case 55: {
			int code = p.s();
			float h = p.f(), tm = p.f(), dmg = p.f();
			const WaterHandle *wh = code == -1 ? NULL : code == -2 ? grid : g_trig[code].getWaterHandle();
			t->changeWaterHeightOverTime(wh, h, tm, dmg);
			break;
		}
		case 56: t->update(); break;
		case 57: {
			int n = TCALL(numWaterToUpdate());
			o.s(n);
			for (int k = 0; k < n; ++k) {
				TLWaterEntry e = g_tl.w3d ? w3d->waterEntry(k) : base->waterEntry(k);
				o.s(waterCode(e.waterTable, grid));
				o.f(e.changePerFrame); o.f(e.targetHeight); o.f(e.damageAmount); o.f(e.currentHeight);
			}
			break;
		}
		case 58: {
			int code = p.s();
			const WaterHandle *wh = code == -1 ? NULL : code == -2 ? grid : g_trig[code].getWaterHandle();
			Region3D rg;
			rg.zero();
			TCALL(callFindRect(wh, &rg));
			putCoord(o, rg.lo);
			putCoord(o, rg.hi);
			break;
		}
		case 59: { int en = p.s(); t->enableWaterGrid(en != 0); break; }
		case 60: { int idx = p.s(); t->setActiveBoundary(idx); o.s(t->getActiveBoundary()); break; }
		case 61: t->flattenTerrain(objectAt(p.s())); o.u(heightMapDigest()); break;
		case 62: t->createCraterInTerrain(objectAt(p.s())); o.u(heightMapDigest()); break;
		case 63: {
			TLMemXfer xs(XFER_SAVE);
			TCALL(callXfer(&xs));
			g_lastSave = xs.m_bytes;
			o.s((int)g_lastSave.size());
			for (size_t k = 0; k < g_lastSave.size(); ++k) o.u(g_lastSave[k]);
			break;
		}
		case 64: {
			TLMemXfer xl(XFER_LOAD);
			xl.m_bytes = g_lastSave;
			int status = 0;
			try { TCALL(callXfer(&xl)); }
			catch (XferStatus s) { status = 100 + (int)s; }
			catch (SaveCode c) { status = 200 + (int)c; }
			o.s(status);
			break;
		}
		case 65: t->reset(); break;
		case 66: o.u(TCALL(waterGridEnabled())); o.s(t->getActiveBoundary()); o.u(hashOf(TCALL(filename()).str())); break;
		case 70: { ICoord2D g; g.x = p.s(); g.y = p.s(); o.s(g_visual.getRawMapHeight(&g)); break; }
		case 72: {
			float x = p.f(), y = p.f();
			Real z = 777.0f;
			Bool on = g_visual.getWaterGridHeight(x, y, &z);
			o.u(on);
			o.f(z);
			break;
		}
		case 73: {
			int n = (int)g_tl.triggers.size();
			o.s(n);
			for (int k = 0; k < n; ++k) {
				o.s(g_trig[k].getID());
				o.u(hashOf(g_trig[k].getTriggerName().str()));
				o.u(g_trig[k].isWaterArea());
				o.s(g_trig[k].getNumPoints());
				for (int q = 0; q < g_trig[k].getNumPoints(); ++q) { const ICoord3D *pt = g_trig[k].getPoint(q); o.s(pt->x); o.s(pt->y); o.s(pt->z); }
			}
			break;
		}
		default: abort();
		}
		result.push_back((unsigned)op);
		result.push_back((unsigned)o.w.size());
		result.insert(result.end(), o.w.begin(), o.w.end());
		result.push_back((unsigned)g_tl.log.size());
		result.insert(result.end(), g_tl.log.begin(), g_tl.log.end());
		result.push_back((unsigned)g_tl.vlog.size());
		result.insert(result.end(), g_tl.vlog.begin(), g_tl.vlog.end());
	}
	TheTerrainLogic = 0;
	TheTerrainRenderObject = 0;
	delete t;
	g_tl.log.clear();
	g_tl.vlog.clear();
	if ((int)result.size() > cap) return -1;
	memcpy(out, result.data(), result.size() * sizeof(unsigned));
	return (int)result.size();
}

}  // extern "C"
