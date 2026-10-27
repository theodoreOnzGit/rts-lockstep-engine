/* Differential-test wrapper for the pathfinder (p1w13-sonnet, layer 10; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h            (#included, UNCHANGED)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp        (verbatim cuts by build.rs, OUT_DIR/pathfind_gen)
** on the stand-ins of differential/shim_pathfind (pathfind_world.h: the object, the terrain logic, the AI and the game logic; PreRTS.h
** renames them and every class of AIPathfind.h), with the real BaseType.h, Snapshot.h, the Xfer classes, GameMemory and LocomotorSet.h.
**
** Batch P4 (pathfinder_p4.inc): the Pathfinder's map, footprints, wall and units' cells, ref_pf_p4_run; the objects, the terrain
** and the AI's data the stand-ins of pathfind_world.h. The Pathfinder is built by its real constructor in zeroed memory (since batch
** P7 compiles findPath, the key function, the vtable is emitted here; the searches not yet ported are stubs, see below).
** Batch P7 (hierarchical_p7.inc): findPath, the hierarchical search, findBrokenBridge, the quick checks, ref_pf_p7_run; and the
** Snapshot members (crc, xfer, loadPostProcess).
** Batch P3 (zone_p3.inc): the zone helpers, ZoneBlock, PathfindZoneManager and PathfindLayer, ref_pf_p3_run; their world (bridges, the
** ground's height, the wall's pieces) the stand-ins of pathfind_world.h, answered from the same hash.
** Batch P2 (cell_p2.inc): PathfindCellInfo and PathfindCell, ref_pf_p2_run.
** Batch P1 (path_p1.inc): PathNode and Path. The pathfinder calls a path makes (isLinePassable, isGroundPathPassable, getCell) are
** answered here from a hash of their arguments that the Rust test computes too (`p1Hash`), and each is a line of the log. */
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
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/XferCRC.h"
#include "pathfind_world.h"
/* AIPathfind.h names PathfindCell in PathfindCellInfo before declaring it (MSVC took its friend declaration as one) */
class PathfindCell;
#include "GameLogic/AIPathfind.h"
/* CRCDebug.h's logging macros, as the release build has them (empty) */
#ifndef CRCDEBUG_LOG
#define CRCDEBUG_LOG(x)
#endif
#ifndef DUMPCOORD3D
#define DUMPCOORD3D(x)
#endif
/* batch P1's line tests (the paths' calls of them are edited to these by build.rs; batch P5 compiles the real ones) */
Bool p1IsLinePassable( Pathfinder *pf, const Object *obj, LocomotorSurfaceTypeMask acceptableSurfaces, PathfindLayerEnum layer,
	const Coord3D& startWorld, const Coord3D& endWorld, Bool blocked, Bool allowPinched );
Bool p1IsGroundPathPassable( Pathfinder *pf, Bool isCrusher, const Coord3D& startWorld, PathfindLayerEnum startLayer, const Coord3D& endWorld,
	Int pathDiameter );
#include "movement_p5_head.inc"
#include "path_p1.inc"
#include "cell_p2.inc"
/* batch P3: what the zone code takes from headers this build does not include: MIN (the Windows macro) and `register` (C++17
** removed it; -Wno-register in build.rs) */
#ifndef MIN
#define MIN(a, b) (((a) < (b)) ? (a) : (b))
#endif
#include "zone_p3.inc"
#include "pathfinder_p4.inc"
#include "movement_p5.inc"
#include "search_p6.inc"
#include "hierarchical_p7.inc"
#include "ground_p8.inc"
/* the Pathfinder's virtual searches not yet ported (later batches cut them; until then the vtable needs them): they find no path */
Path *Pathfinder::findClosestPath( Object *obj, const LocomotorSet& locomotorSet, const Coord3D *from, Coord3D *to, Bool blocked,
	Real pathCostMultiplier, Bool moveAllies ) { return NULL; }
Path *Pathfinder::findAttackPath( const Object *obj, const LocomotorSet& locomotorSet, const Coord3D *from, const Object *victim,
	const Coord3D* victimPos, const Weapon *weapon ) { return NULL; }
Path *Pathfinder::findSafePath( const Object *obj, const LocomotorSet& locomotorSet, const Coord3D *from, const Coord3D* repulsorPos1,
	const Coord3D* repulsorPos2, Real repulsorRadius ) { return NULL; }
Path *Pathfinder::patchPath( const Object *obj, const LocomotorSet& locomotorSet, Path *originalPath, Bool blocked ) { return NULL; }
#undef private
#undef protected

namespace {

unsigned bitsOf(float f) { unsigned u; memcpy(&u, &f, 4); return u; }
float floatOf(unsigned u) { float f; memcpy(&f, &u, 4); return f; }

/* the scene: the hash's seed and the answers' rates (percent), the object's layer */
struct PfScene { unsigned seed, lineRate, groundRate, cliffRate, nullCellRate, pinchRate, objLayer; };
PfScene g_scene;
std::vector<std::string> g_log;
void logf(const char *fmt, ...) __attribute__((format(printf, 1, 2)));
void logf(const char *fmt, ...)
{
	char b[512];
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(b, sizeof b, fmt, ap);
	va_end(ap);
	g_log.push_back(b);
}

/* the answers' hash: the seed, a tag, the words (the same function in tests/differential_path_p1.rs) */
unsigned p1Hash(unsigned tag, std::initializer_list<unsigned> words)
{
	unsigned x = g_scene.seed ^ (tag * 0x85ebca6bu);
	for (unsigned w : words) {
		x ^= w;
		x *= 0x9E3779B1u;
		x ^= x >> 15;
	}
	return x;
}
bool p1Pick(unsigned h, unsigned rate) { return (h >> 8) % 100u < rate; }

Object g_object;
AI g_ai;
TerrainLogic g_terrain;
GameLogic g_gameLogic;
alignas(16) unsigned char g_pathfinderMem[sizeof(Pathfinder)];

}  // namespace

AI *TheAI = &g_ai;
TerrainLogic *TheTerrainLogic = &g_terrain;
GameLogic *TheGameLogic = &g_gameLogic;
Pathfinder *AI::pathfinder() { return (Pathfinder *)g_pathfinderMem; }

PathfindLayerEnum TerrainLogic::getLayerForDestination(const Coord3D *pos)
{
	unsigned h = p1Hash(1, { bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) });
	PathfindLayerEnum layer = (PathfindLayerEnum)(1 + (h >> 8) % 3);
	logf("layerfor %08x %08x %08x %d", bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z), (int)layer);
	return layer;
}

bool g_p1RealLines = false;	/* batch P6: the real line tests (the paths a search builds are optimized with them) */
Bool p1IsLinePassable( Pathfinder *pf, const Object *obj, LocomotorSurfaceTypeMask acceptableSurfaces, PathfindLayerEnum layer,
	const Coord3D& startWorld, const Coord3D& endWorld, Bool blocked, Bool allowPinched )
{
	if (g_p1RealLines) return pf->isLinePassable(obj, acceptableSurfaces, layer, startWorld, endWorld, blocked, allowPinched);
	unsigned h = p1Hash(2, { (unsigned)obj->getID(), (unsigned)acceptableSurfaces, (unsigned)layer, bitsOf(startWorld.x), bitsOf(startWorld.y),
		bitsOf(endWorld.x), bitsOf(endWorld.y), (unsigned)blocked, (unsigned)allowPinched });
	bool r = p1Pick(h, g_scene.lineRate);
	logf("linepassable %d %u %d %08x %08x %08x %08x %d %d -> %d", (int)obj->getID(), (unsigned)acceptableSurfaces, (int)layer, bitsOf(startWorld.x),
		bitsOf(startWorld.y), bitsOf(endWorld.x), bitsOf(endWorld.y), blocked ? 1 : 0, allowPinched ? 1 : 0, r ? 1 : 0);
	return r;
}

Bool p1IsGroundPathPassable( Pathfinder *pf, Bool isCrusher, const Coord3D& startWorld, PathfindLayerEnum startLayer, const Coord3D& endWorld,
	Int pathDiameter )
{
	if (g_p1RealLines) return pf->isGroundPathPassable(isCrusher, startWorld, startLayer, endWorld, pathDiameter);
	unsigned h = p1Hash(3, { (unsigned)isCrusher, bitsOf(startWorld.x), bitsOf(startWorld.y), (unsigned)startLayer, bitsOf(endWorld.x), bitsOf(endWorld.y),
		(unsigned)pathDiameter });
	bool r = p1Pick(h, g_scene.groundRate);
	logf("groundpassable %d %08x %08x %d %08x %08x %d -> %d", isCrusher ? 1 : 0, bitsOf(startWorld.x), bitsOf(startWorld.y), (int)startLayer,
		bitsOf(endWorld.x), bitsOf(endWorld.y), (int)pathDiameter, r ? 1 : 0);
	return r;
}

/* batch P3: the scene's bridges, ground and wall pieces answer from the hash of their arguments (the same in
** tests/differential_zone_p3.rs): a point or a cell on a bridge at the case's rates, heights 0..39 */
struct P3Rates { unsigned onBridge, end, side, entry; };
P3Rates g_p3;
std::vector<Object> g_walls;
Bool Bridge::isPointOnBridge(const Coord3D *p) { return p1Pick(p1Hash(10, { m_index, bitsOf(p->x), bitsOf(p->y) }), g_p3.onBridge); }
Bool Bridge::isCellOnEnd(const Region2D *c)
{
	return p1Pick(p1Hash(11, { m_index, bitsOf(c->lo.x), bitsOf(c->lo.y), bitsOf(c->hi.x), bitsOf(c->hi.y) }), g_p3.end);
}
Bool Bridge::isCellOnSide(const Region2D *c)
{
	return p1Pick(p1Hash(12, { m_index, bitsOf(c->lo.x), bitsOf(c->lo.y), bitsOf(c->hi.x), bitsOf(c->hi.y) }), g_p3.side);
}
Bool Bridge::isCellEntryPoint(const Region2D *c)
{
	return p1Pick(p1Hash(13, { m_index, bitsOf(c->lo.x), bitsOf(c->lo.y), bitsOf(c->hi.x), bitsOf(c->hi.y) }), g_p3.entry);
}
Real Bridge::getBridgeHeight(const Coord3D *p, Coord3D *normal) { return (Real)((p1Hash(14, { m_index, bitsOf(p->x), bitsOf(p->y) }) >> 8) % 40u); }
Real TerrainLogic::getLayerHeight(Real x, Real y, PathfindLayerEnum layer, Coord3D *normal, Bool clip)
{
	return (Real)((p1Hash(15, { bitsOf(x), bitsOf(y), (unsigned)layer }) >> 8) % 40u);
}
/* batch P4: the terrain's cells, extent and bridge interactions answer from the hash too; the objects are g_walls (all of them, in
** order); a splat is logged */
struct P4World { unsigned cliff, water, layer, end; Region3D extent; };
P4World g_p4;
TAiData g_aiData;
std::vector<unsigned> g_splats;
const TAiData *AI::getAiData() { return &g_aiData; }
Real TerrainLogic::getGroundHeight(Real x, Real y, Coord3D *normal) const { return (Real)((p1Hash(16, { bitsOf(x), bitsOf(y) }) >> 8) % 40u); }
static GlobalData s_pfGlobalData = { 0 };
GlobalData *TheGlobalData = &s_pfGlobalData;
/* (the water word: the rate in its low 8 bits; batch P7, a river: the columns of cells (bits 8-15) .. (bits 16-23) under water, when
** bits 16-23 are not 0) */
Bool TerrainLogic::isUnderwater(Real x, Real y, Real *waterZ, Real *terrainZ)
{
	unsigned lo = (g_p4.water >> 8) & 255, hi = (g_p4.water >> 16) & 255;
	if (hi != 0 && x >= lo * 10.0f && x <= hi * 10.0f) return TRUE;
	return p1Pick(p1Hash(21, { bitsOf(x), bitsOf(y) }), g_p4.water & 255);
}
Bool TerrainLogic::isCliffCell(Real x, Real y) const { return p1Pick(p1Hash(20, { bitsOf(x), bitsOf(y) }), g_p4.cliff); }
void TerrainLogic::getMaximumPathfindExtent(Region3D *extent) const { *extent = g_p4.extent; }
Bool TerrainLogic::objectInteractsWithBridgeLayer(Object *obj, Int layer, Bool considerBridgeHealth) const
{
	return p1Pick(p1Hash(22, { (unsigned)obj->getID(), (unsigned)layer }), g_p4.layer);
}
Bool TerrainLogic::objectInteractsWithBridgeEnd(Object *obj, Int layer) const
{
	return p1Pick(p1Hash(23, { (unsigned)obj->getID(), (unsigned)layer }), g_p4.end);
}
Object *GameLogic::getFirstObject() { return g_walls.empty() ? NULL : &g_walls[0]; }
void Object::attemptDamage(DamageInfo *d)
{
	g_splats.push_back((unsigned)m_id); g_splats.push_back((unsigned)d->in.m_damageType); g_splats.push_back((unsigned)d->in.m_deathType);
	g_splats.push_back((unsigned)d->in.m_sourceID); g_splats.push_back(bitsOf(d->in.m_amount));
}
/* batch P5: relations, crush tests and weapons' terrain lines from the hash; the closest tall building by squared 2-D distance */
unsigned g_p5Allies = 50, g_p5Crush = 40, g_p5Line = 70;
Int Object::getRelationship(const Object *that) const { return p1Pick(p1Hash(30, { (unsigned)m_id, (unsigned)that->getID() }), g_p5Allies) ? ALLIES : ENEMIES; }
Bool Object::canCrushOrSquish(Object *other, CrushSquishTestType t) const
{
	return p1Pick(p1Hash(31, { (unsigned)m_id, (unsigned)other->getID(), (unsigned)t }), g_p5Crush);
}
Bool Weapon::isClearGoalFiringLineOfSightTerrain(const Object *source, const Coord3D &pos, const Object *victim) const
{
	return p1Pick(p1Hash(32, { (unsigned)source->getID(), bitsOf(pos.x), bitsOf(pos.y), (unsigned)victim->getID() }), g_p5Line);
}
Bool Weapon::isClearGoalFiringLineOfSightTerrain(const Object *source, const Coord3D &pos, const Coord3D &victimPos) const
{
	return p1Pick(p1Hash(33, { (unsigned)source->getID(), bitsOf(pos.x), bitsOf(pos.y), bitsOf(victimPos.x), bitsOf(victimPos.y) }), g_p5Line);
}
static PartitionManager s_pfPartition;
PartitionManager *ThePartitionManager = &s_pfPartition;
Object *PartitionManager::getClosestObject(const Coord3D *pos, Real maxDist, int dc, PartitionFilter **filters)
{
	int kind = ((PartitionFilterAcceptByKindOf *)filters[0])->m_kind;
	Object *best = NULL;
	Real bestD2 = 0;
	for (Object &o : g_walls) {
		if (!o.isKindOf((KindOfType)kind)) continue;
		Real dx = o.m_pos.x - pos->x, dy = o.m_pos.y - pos->y, d2 = dx * dx + dy * dy, r = maxDist + o.m_geom.m_boundingRadius;
		if (d2 <= r * r && (!best || d2 < bestD2)) { best = &o; bestD2 = d2; }
	}
	return best;
}
Object *GameLogic::findObjectByID(ObjectID id)
{
	for (Object &o : g_walls)
		if (o.m_id == id) return &o;
	return NULL;
}

namespace {

/* the map: W x H cells from (0, 0) (Pathfinder::m_map, m_extent); each cell's type and pinch from the hash of its coordinates */
enum { P1_W = 24, P1_H = 16 };
/* (raw memory: PathfindCell's constructor is batch P2's code) */
alignas(16) unsigned char g_cellMem[P1_W * P1_H * sizeof(PathfindCell)];
PathfindCell *g_columns[P1_W];

void p1Map()
{
	Pathfinder *pf = TheAI->pathfinder();
	memset(g_pathfinderMem, 0, sizeof g_pathfinderMem);
	pf->m_extent.lo.x = 0; pf->m_extent.lo.y = 0;
	pf->m_extent.hi.x = P1_W - 1; pf->m_extent.hi.y = P1_H - 1;
	memset(g_cellMem, 0, sizeof g_cellMem);
	PathfindCell *cells = (PathfindCell *)g_cellMem;
	for (int x = 0; x < P1_W; ++x) {
		g_columns[x] = cells + x * P1_H;
		for (int y = 0; y < P1_H; ++y) {
			unsigned h = p1Hash(5, { (unsigned)x, (unsigned)y });
			g_columns[x][y].m_type = p1Pick(h, g_scene.cliffRate) ? PathfindCell::CELL_CLIFF : PathfindCell::CELL_CLEAR;
			g_columns[x][y].m_pinched = p1Pick(p1Hash(6, { h }), g_scene.pinchRate);
		}
	}
	pf->m_map = g_columns;
}

}  // namespace

namespace {

std::vector<unsigned> g_out;
void putU32(unsigned v) { g_out.push_back(v); }
void putStr(const std::string &s)
{
	putU32((unsigned)s.size());
	for (size_t i = 0; i < s.size(); i += 4) {
		unsigned w = 0;
		for (size_t k = 0; k < 4 && i + k < s.size(); ++k) w |= (unsigned)(unsigned char)s[i + k] << (8 * k);
		putU32(w);
	}
}

int nodeIndex(const Path *p, const PathNode *n)
{
	int i = 0;
	for (const PathNode *x = p->m_path; x; x = x->m_next, ++i) if (x == n) return i;
	return n ? 1000 : -1;
}
PathNode *nodeAt(Path *p, unsigned k)
{
	PathNode *n = p->m_path;
	for (unsigned i = 0; n && i < k; ++i) n = n->m_next;
	return n;
}

/* the path's state: the nodes (position, layer, can-optimize, id, the optimized next (its index), its distance and direction, the
** back link (its index)), the tail's index, the flags and the cached closest point */
void putPath(const Path *p)
{
	unsigned count = 0;
	for (const PathNode *n = p->m_path; n; n = n->m_next) ++count;
	putU32(count);
	for (const PathNode *n = p->m_path; n; n = n->m_next) {
		putU32(bitsOf(n->m_pos.x)); putU32(bitsOf(n->m_pos.y)); putU32(bitsOf(n->m_pos.z));
		putU32((unsigned)n->m_layer);
		putU32(n->m_canOptimize ? 1u : 0u);
		putU32((unsigned)n->m_id);
		putU32((unsigned)nodeIndex(p, n->m_nextOpti));
		putU32(bitsOf(n->m_nextOptiDist2D));
		putU32(bitsOf(n->m_nextOptiDirNorm2D.x)); putU32(bitsOf(n->m_nextOptiDirNorm2D.y));
		putU32((unsigned)nodeIndex(p, n->m_prev));
	}
	putU32((unsigned)nodeIndex(p, p->m_pathTail));
	putU32(p->m_isOptimized ? 1u : 0u);
	putU32(p->m_blockedByAlly ? 1u : 0u);
	putU32(p->m_cpopValid ? 1u : 0u);
	putU32((unsigned)p->m_cpopCountdown);
	putU32(bitsOf(p->m_cpopIn.x)); putU32(bitsOf(p->m_cpopIn.y)); putU32(bitsOf(p->m_cpopIn.z));
	putU32(bitsOf(p->m_cpopOut.distAlongPath));
	putU32(bitsOf(p->m_cpopOut.posOnPath.x)); putU32(bitsOf(p->m_cpopOut.posOnPath.y)); putU32(bitsOf(p->m_cpopOut.posOnPath.z));
	putU32((unsigned)p->m_cpopOut.layer);
	putU32((unsigned)nodeIndex(p, p->m_cpopRecentStart));
}

}  // namespace


namespace {

/* batch P2: the obstacle setTypeAsObstacle and removeObstacle are given: an Object of the id the test gives, whose body is rubble and
** which is KINDOF_CAN_SEE_THROUGH_STRUCTURE as the test says (pathfind_world.h) */
Bool p2SetTypeAsObstacle(PathfindCell *cell, ObjectID id, bool rubble, bool seeThrough, bool fence, const ICoord2D &pos)
{
	Object o;
	o.m_id = id;
	o.m_layer = LAYER_GROUND;
	o.m_rubble = rubble;
	o.m_seeThrough = seeThrough;
	return cell->setTypeAsObstacle(&o, fence, pos);
}
Bool p2RemoveObstacle(PathfindCell *cell, ObjectID id)
{
	Object o;
	o.m_id = id;
	return cell->removeObstacle(&o);
}

}  // namespace

extern "C" {

/* Batch P1: a program of path operations on one path. `cfg` (8 words): the hash's seed; the rates (percent) of a passable line, a
** passable ground path, a cliff cell, (unused) and a pinched cell; the object's layer; its id. The map is 24 x 16 cells from (0, 0). Then `n` ops of 6 words [op, a, b, c,
** d, e] (positions as float bits): 0 prependNode(a, b, c, layer d)  1 appendNode(a, b, c, layer d)  2 updateLastNode(a, b, c)
** 3 optimize(obj, surfaces d, blocked e)  4 optimizeGroundPath(crusher d, diameter e)  5 computePointOnPath(obj, surfaces d, (a, b, c))
** 6 computeFlightDistToGoal((a, b, c))  7 setCanOptimize(node a, b)  8 setBlockedByAlly(a)  9 markOptimized  10 xfer save
** 11 xfer load of the last save into a new path  12 xfer CRC  13 node a's computeDirectionVector  14 node a's getNextOptimized(dir, dist)
** 15 a new path. An op that needs a node the path does not have (optimize on an empty path, a node index beyond it) is not run (the
** Rust test excludes the undefined ones). Each op appends: the result words (a count, then each), the log, the xfer status and bytes
** (save) or CRC, the path. */
int ref_pf_p1_run(const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned *buf, int cap, int *len)
{
	g_out.clear();
	g_scene.seed = cfg[0];
	g_scene.lineRate = cfg[1];
	g_scene.groundRate = cfg[2];
	g_scene.cliffRate = cfg[3];
	g_scene.nullCellRate = cfg[4];
	g_scene.pinchRate = cfg[5];
	g_object.m_layer = (PathfindLayerEnum)cfg[6];
	g_object.m_id = (ObjectID)cfg[7];
	p1Map();
	LocomotorSet *locos = (LocomotorSet *)calloc(1, sizeof(LocomotorSet));
	Path *path = newInstance(Path);
	std::vector<unsigned char> saved;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		g_log.clear();
		std::vector<unsigned> res;
		int xst = -2;
		std::vector<unsigned char> bytes;
		unsigned crc = 0;
		Coord3D pos;
		pos.x = floatOf(o[1]); pos.y = floatOf(o[2]); pos.z = floatOf(o[3]);
		switch (o[0]) {
		case 0: path->prependNode(&pos, (PathfindLayerEnum)o[4]); break;
		case 1: path->appendNode(&pos, (PathfindLayerEnum)o[4]); break;
		case 2: path->updateLastNode(&pos); break;
		case 3: if (path->m_path) path->optimize(&g_object, (LocomotorSurfaceTypeMask)o[4], o[5] != 0); break;
		case 4: if (path->m_path) path->optimizeGroundPath(o[4] != 0, (Int)o[5]); break;
		case 5: {
			locos->m_validLocomotorSurfaces = (LocomotorSurfaceTypeMask)o[4];
			ClosestPointOnPathInfo out;
			path->computePointOnPath(&g_object, *locos, pos, out);
			res.push_back(bitsOf(out.distAlongPath));
			res.push_back(bitsOf(out.posOnPath.x)); res.push_back(bitsOf(out.posOnPath.y)); res.push_back(bitsOf(out.posOnPath.z));
			res.push_back((unsigned)out.layer);
			break;
		}
		case 6: {
			Coord3D goal;
			goal.zero();
			Real d = path->computeFlightDistToGoal(&pos, goal);
			res.push_back(bitsOf(d));
			res.push_back(bitsOf(goal.x)); res.push_back(bitsOf(goal.y)); res.push_back(bitsOf(goal.z));
			break;
		}
		case 7: if (PathNode *nd = nodeAt(path, o[1])) nd->setCanOptimize(o[2] != 0); break;
		case 8: path->setBlockedByAlly(o[1] != 0); break;
		case 9: path->markOptimized(); break;
		case 10: {
			XferSave x;
			xst = 0;
			try { x.open(AsciiString(xfer_path)); x.xferSnapshot(path); } catch (XferStatus s) { xst = (int)s; } catch (...) { xst = -1; }
			try { x.close(); } catch (...) {}
			if (FILE *f = fopen(xfer_path, "rb")) {
				unsigned char b[65536];
				size_t m = fread(b, 1, sizeof b, f);
				fclose(f);
				bytes.assign(b, b + m);
			}
			saved = bytes;
			break;
		}
		case 11: {
			if (FILE *f = fopen(xfer_path, "wb")) { if (!saved.empty()) fwrite(&saved[0], 1, saved.size(), f); fclose(f); }
			path->deleteInstance();
			path = newInstance(Path);
			XferLoad x;
			xst = 0;
			try { x.open(AsciiString(xfer_path)); x.xferSnapshot(path); } catch (XferStatus s) { xst = (int)s; } catch (...) { xst = -1; }
			try { x.close(); } catch (...) {}
			break;
		}
		case 12: {
			XferCRC x;
			xst = 0;
			try { x.open(AsciiString("crc")); x.xferSnapshot(path); } catch (XferStatus s) { xst = (int)s; } catch (...) { xst = -1; }
			crc = x.getCRC();
			try { x.close(); } catch (...) {}
			break;
		}
		case 13: if (PathNode *nd = nodeAt(path, o[1])) {
			const Coord3D *d = nd->computeDirectionVector();
			res.push_back(bitsOf(d->x)); res.push_back(bitsOf(d->y)); res.push_back(bitsOf(d->z));
			break;
		}
		break;
		case 14: if (PathNode *nd = nodeAt(path, o[1])) {
			Coord2D dir;
			dir.x = dir.y = 0;
			Real dist = 0;
			PathNode *nx = nd->getNextOptimized(&dir, &dist);
			res.push_back((unsigned)nodeIndex(path, nx)); res.push_back(bitsOf(dir.x)); res.push_back(bitsOf(dir.y)); res.push_back(bitsOf(dist));
			break;
		}
		break;
		case 15: path->deleteInstance(); path = newInstance(Path); break;
		}
		putU32((unsigned)res.size());
		for (unsigned r : res) putU32(r);
		putU32((unsigned)g_log.size());
		for (const std::string &l : g_log) putStr(l);
		putU32((unsigned)xst);
		putU32((unsigned)bytes.size());
		for (unsigned char b : bytes) putU32(b);
		putU32(crc);
		putPath(path);
	}
	path->deleteInstance();
	free(locos);
	*len = (int)g_out.size();
	if (*len > cap) return 1;
	memcpy(buf, &g_out[0], g_out.size() * sizeof(unsigned));
	return 0;
}

/* Batch P2: a program of cell operations on a grid of `cfg[0]` x `cfg[1]` real PathfindCells (cell k is (k / h, k % h), its position
** that) and the REAL info pool, allocated for the case. Two lists the driver keeps: the open list and the closed list. Ops of 6 words
** [op, a, b, c, d, e] (cells a and b): 0 allocateInfo(a)  1 releaseInfo(a)  2 setGoalUnit(a, id b)  3 setGoalAircraft(a, id b)
** 4 setPosUnit(a, id b)  5 setTypeAsObstacle(a, obstacle id b, rubble c, see-through d, fence e)  6 setType(a, type b)
** 7 removeObstacle(a, obstacle id b)  8 reset(a)  9 startPathfind(a, goal b (c: none))  10 setParentCell(a, b)
** 11 setParentCellHierarchical(a, b)  12 clearParentCell(a)  13 putOnSortedOpenList(a)  14 removeFromOpenList(a)
** 15 putOnClosedList(a)  16 removeFromClosedList(a)  17 releaseOpenList  18 releaseClosedList  19 costToGoal(a, b)
** 20 costToHierGoal(a, b)  21 costSoFar(a, parent b (c: NULL))  22 setCostSoFar(a, b)  23 setTotalCost(a, b)  24 setBlockedByAlly(a, b)
** 25 setPinched(a, b)  26 setZone(a, b)  27 setLayer(a, b)  28 setConnectLayer(a, b)  29 the getters of a. (The caller keeps to the
** calls upstream defines.) Each op appends: the results, the lists' heads, the cells (type, flags, zone, aircraft goal, pinched, layers,
** info), the first free info, and the infos below the highest one a cell has held (each its fields). */
int ref_pf_p2_run(const unsigned *cfg, int n, const unsigned *ops, unsigned *buf, int cap, int *len)
{
	g_out.clear();
	int w = (int)cfg[0], h = (int)cfg[1], count = w * h;
	PathfindCellInfo::allocateCellInfos();
	PathfindCell *cells = new PathfindCell[count];
	PathfindCell *openList = NULL, *closedList = NULL;
	int highWater = 0;
	auto cellOf = [&](unsigned k) -> PathfindCell * { return &cells[k % count]; };
	auto posOf = [&](unsigned k) -> ICoord2D { ICoord2D p; p.x = (Int)((k % count) / h); p.y = (Int)((k % count) % h); return p; };
	auto infoIndex = [](const PathfindCellInfo *i) -> unsigned { return i ? (unsigned)(i - PathfindCellInfo::s_infoArray) : 0xffffffffu; };
	auto cellIndex = [&](const PathfindCell *c) -> unsigned { return c ? (unsigned)(c - cells) : 0xffffffffu; };
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		std::vector<unsigned> res;
		PathfindCell *a = cellOf(o[1]), *b = cellOf(o[2]);
		ICoord2D pa = posOf(o[1]);
		/* The test issues only the calls upstream defines, judged on the Rust store; a Rust defect (a planted one) can make one of them
		** undefined here (a cell without the info it dereferences). Then the dump ends with a marker, which the test counts a mismatch. */
		{
			unsigned op = o[0];
			bool needA = (op >= 9 && op <= 16) || op == 19 || op == 21 || op == 24;
			bool needB = ((op == 9 || op == 21) && o[3] == 0) || op == 10 || op == 11 || op == 19 || op == 20;
			bool bad = (needA && !a->hasInfo()) || (needB && !b->hasInfo());
			if (!bad && op == 21 && o[3] == 0 && b->getParentCell() && !b->getParentCell()->hasInfo()) bad = true;
			if (bad) { g_out.push_back(0xbadc0de5u); break; }
		}
		switch (o[0]) {
		case 0: res.push_back(a->allocateInfo(pa) ? 1u : 0u); break;
		case 1: a->releaseInfo(); break;
		case 2: a->setGoalUnit((ObjectID)o[2], pa); break;
		case 3: a->setGoalAircraft((ObjectID)o[2], pa); break;
		case 4: a->setPosUnit((ObjectID)o[2], pa); break;
		case 5: {
			/* the obstacle: an Object stand-in whose id is b, its rubble and see-through the test's (P2Obstacle) */
			res.push_back(p2SetTypeAsObstacle(a, (ObjectID)o[2], o[3] != 0, o[4] != 0, o[5] != 0, pa) ? 1u : 0u);
			break;
		}
		case 6: a->setType((PathfindCell::CellType)o[2]); break;
		case 7: res.push_back(p2RemoveObstacle(a, (ObjectID)o[2]) ? 1u : 0u); break;
		case 8: a->reset(); break;
		case 9: res.push_back(a->startPathfind(o[3] ? NULL : b) ? 1u : 0u); break;
		case 10: a->setParentCell(b); break;
		case 11: a->setParentCellHierarchical(b); break;
		case 12: a->clearParentCell(); break;
		case 13: openList = a->putOnSortedOpenList(openList); break;
		case 14: openList = a->removeFromOpenList(openList); break;
		case 15: closedList = a->putOnClosedList(closedList); break;
		case 16: closedList = a->removeFromClosedList(closedList); break;
		case 17: res.push_back((unsigned)PathfindCell::releaseOpenList(openList)); openList = NULL; break;
		case 18: res.push_back((unsigned)PathfindCell::releaseClosedList(closedList)); closedList = NULL; break;
		case 19: res.push_back(a->costToGoal(b)); break;
		case 20: res.push_back(a->costToHierGoal(b)); break;
		case 21: res.push_back(a->costSoFar(o[3] ? NULL : b)); break;
		case 22: a->setCostSoFar(o[2]); break;
		case 23: a->setTotalCost(o[2]); break;
		case 24: a->setBlockedByAlly(o[2] != 0); break;
		case 25: a->setPinched(o[2] != 0); break;
		case 26: a->setZone((zoneStorageType)o[2]); break;
		case 27: a->setLayer((PathfindLayerEnum)o[2]); break;
		case 28: a->setConnectLayer((PathfindLayerEnum)o[2]); break;
		case 29:
			res.push_back((unsigned)a->getGoalUnit()); res.push_back((unsigned)a->getGoalAircraft()); res.push_back((unsigned)a->getPosUnit());
			res.push_back((unsigned)a->getObstacleID()); res.push_back(a->isObstacleTransparent() ? 1u : 0u); res.push_back(a->isObstacleFence() ? 1u : 0u);
			res.push_back(a->isObstaclePresent((ObjectID)o[2]) ? 1u : 0u); res.push_back(cellIndex(a->getParentCell()));
			if (a->hasInfo()) { res.push_back(cellIndex(a->getNextOpen())); res.push_back(a->getXIndex()); res.push_back(a->getYIndex()); }
			break;
		}
		putU32((unsigned)res.size());
		for (unsigned r : res) putU32(r);
		putU32(cellIndex(openList));
		putU32(cellIndex(closedList));
		for (int k = 0; k < count; ++k) {
			const PathfindCell &c = cells[k];
			putU32((unsigned)c.getType()); putU32((unsigned)c.getFlags()); putU32((unsigned)c.getZone()); putU32(c.isAircraftGoal() ? 1u : 0u);
			putU32(c.getPinched() ? 1u : 0u); putU32((unsigned)c.getLayer()); putU32((unsigned)c.getConnectLayer()); putU32(infoIndex(c.m_info));
			if (c.m_info && (int)infoIndex(c.m_info) + 1 > highWater) highWater = (int)infoIndex(c.m_info) + 1;
		}
		putU32(infoIndex(PathfindCellInfo::s_firstFree));
		putU32((unsigned)highWater);
		for (int k = 0; k < highWater; ++k) {
			const PathfindCellInfo &f = PathfindCellInfo::s_infoArray[k];
			putU32(f.m_isFree ? 1u : 0u); putU32(infoIndex(f.m_nextOpen)); putU32(infoIndex(f.m_prevOpen)); putU32(infoIndex(f.m_pathParent));
			putU32(cellIndex(f.m_cell)); putU32(f.m_totalCost); putU32(f.m_costSoFar); putU32((unsigned)f.m_pos.x); putU32((unsigned)f.m_pos.y);
			putU32((unsigned)f.m_goalUnitID); putU32((unsigned)f.m_posUnitID); putU32((unsigned)f.m_goalAircraftID); putU32((unsigned)f.m_obstacleID);
			putU32(f.m_blockedByAlly); putU32(f.m_obstacleIsFence); putU32(f.m_obstacleIsTransparent); putU32(f.m_open); putU32(f.m_closed);
		}
	}
	delete[] cells;
	PathfindCellInfo::releaseCellInfos();
	*len = (int)g_out.size();
	if (*len > cap) return 1;
	memcpy(buf, &g_out[0], g_out.size() * sizeof(unsigned));
	return 0;
}

/* Batch P3: zones and layers. The scene: a map of cfg[0] x cfg[1] real PathfindCells (Pathfinder::m_map, m_extent (0, 0) ..
** (w - 1, h - 1)), the REAL info pool, a real PathfindZoneManager and LAYER_LAST + 1 real PathfindLayers (in zeroed memory), the
** bridges and wall pieces of the cfg. cfg: [w, h, seed, onBridge rate, end rate, side rate, entry rate, bridge count, per bridge
** (bounds lo.x lo.y hi.x hi.y, from x y z, to x y z; float bits; its object 1000 + index), wall piece count, per piece (id, x, y,
** angle, major, minor, sphere; floats as bits), then the map's w * h types]. Ops of 6 words [op, a, b, c, d, e] (cells k are (k / h,
** k % h); x, y signed): 0 calculateZones  1 updateZonesForModify(lo a b, hi c d)  2 setType(cell a, b)  3 setZone(cell a, b)
** 4 setConnectLayer(cell a, b)  5 setTypeAsObstacle(cell a, id b, fence c)  6 removeObstacle(cell a, id b)
** 7 getEffectiveZone(surfaces a, crusher b, zone c)  8 getEffectiveTerrainZone(a)  9 getBlockZone(surfaces a, crusher b, x c, y d)
** 10 clearPassableFlags  11 setAllPassable  12 setPassable(x a, y b, c)  13 isPassable(x a, y b)  14 clipIsPassable(x a, y b)
** 15 setBridge(x a, y b, c)  16 interactsWithBridge(x a, y b)  17 markZonesDirty(insert b) at frame a, then needToCalculateZones
** 18 allocateBlocks(the map)  19 the manager's reset  20 layer a: init(bridge b (0xffffffff: NULL), layer c)  21 allocateCells
** 22 classifyCells  23 setDestroyed(b)  24 reset  25 getCell(x b, y c)  26 connectsZones(surfaces b, zones c d)  27 getBridgeID
** 28 allocateCellsForWallLayer(the pieces)  29 classifyWallCells(the pieces)  30 isPointOnWall(the pieces, x bits b, y bits c)
** 31 setZone(b) then applyZone  32 the map's zones in x a..b, y c..d unset (the setup of a modification). (The caller keeps to the calls upstream defines; see the test.) Each op appends: the results; the
** manager (blocks or not, block extent, max zone, next frame, zones allocated, tables or not and their first max zone + 1 entries);
** each block (origin, first zone, zone count, allocated, bridge, passable, tables or not and their first zone-count entries); each
** map cell (type, zone, connect layer, layer); each layer (extent, origin, ends, layer, zone, bridge, destroyed, unused, then its
** cells: type, zone, connect layer, layer, pinched), after a word that counts the rest. */
int ref_pf_p3_run(const unsigned *cfg, int n, const unsigned *ops, unsigned *buf, int cap, int *len)
{
	g_out.clear();
	int w = (int)cfg[0], h = (int)cfg[1], count = w * h;
	g_scene.seed = cfg[2];
	g_p3.onBridge = cfg[3]; g_p3.end = cfg[4]; g_p3.side = cfg[5]; g_p3.entry = cfg[6];
	int nb = (int)cfg[7], at = 8;
	std::vector<Bridge> bridges(nb);
	for (int k = 0; k < nb; ++k, at += 10) {
		Bridge &b = bridges[k];
		b.m_index = (unsigned)k;
		b.m_bounds.lo.x = floatOf(cfg[at]); b.m_bounds.lo.y = floatOf(cfg[at + 1]);
		b.m_bounds.hi.x = floatOf(cfg[at + 2]); b.m_bounds.hi.y = floatOf(cfg[at + 3]);
		b.m_info.from.x = floatOf(cfg[at + 4]); b.m_info.from.y = floatOf(cfg[at + 5]); b.m_info.from.z = floatOf(cfg[at + 6]);
		b.m_info.to.x = floatOf(cfg[at + 7]); b.m_info.to.y = floatOf(cfg[at + 8]); b.m_info.to.z = floatOf(cfg[at + 9]);
		b.m_info.bridgeObjectID = (ObjectID)(1000 + k);
	}
	int nw = (int)cfg[at++];
	g_walls.assign(nw, Object());
	std::vector<ObjectID> pieces(nw);
	for (int k = 0; k < nw; ++k, at += 7) {
		Object &o = g_walls[k];
		o.m_id = (ObjectID)cfg[at]; pieces[k] = o.m_id;
		o.m_pos.x = floatOf(cfg[at + 1]); o.m_pos.y = floatOf(cfg[at + 2]); o.m_pos.z = 0;
		o.m_angle = floatOf(cfg[at + 3]); o.m_geom.m_major = floatOf(cfg[at + 4]); o.m_geom.m_minor = floatOf(cfg[at + 5]);
		o.m_geom.m_sphere = cfg[at + 6] != 0;
	}
	PathfindCellInfo::allocateCellInfos();
	PathfindCell *cells = new PathfindCell[count];
	std::vector<PathfindCell *> columns(w);
	for (int x = 0; x < w; ++x) columns[x] = cells + x * h;
	for (int k = 0; k < count; ++k) cells[k].setType((PathfindCell::CellType)cfg[at + k]);
	Pathfinder *pf = TheAI->pathfinder();
	memset(g_pathfinderMem, 0, sizeof g_pathfinderMem);
	pf->m_extent.lo.x = 0; pf->m_extent.lo.y = 0; pf->m_extent.hi.x = w - 1; pf->m_extent.hi.y = h - 1;
	pf->m_map = &columns[0];
	IRegion2D extent = pf->m_extent;
	PathfindZoneManager *zm = new PathfindZoneManager;
	alignas(16) static unsigned char layerMem[(LAYER_LAST + 1) * sizeof(PathfindLayer)];
	memset(layerMem, 0, sizeof layerMem);
	PathfindLayer *layers = (PathfindLayer *)layerMem;
	for (int k = 0; k <= LAYER_LAST; ++k) new (&layers[k]) PathfindLayer();
	alignas(16) unsigned char locoMem[sizeof(LocomotorSet)];
	memset(locoMem, 0, sizeof locoMem);
	LocomotorSet *loco = (LocomotorSet *)locoMem;
	Object obstacle;
	auto layerOf = [&](unsigned k) -> PathfindLayer & { return layers[k % (LAYER_LAST + 1)]; };
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		std::vector<unsigned> res;
		PathfindCell *a = &cells[o[1] % count];
		/* As in P2: a call the test judged defined on the Rust state but undefined here (a planted defect) ends the dump with a marker. */
		{
			unsigned op = o[0];
			PathfindLayer &l = layerOf(o[1]);
			bool bad = ((op == 0 || op == 1 || op == 9) && !zm->m_zoneBlocks) || ((op == 7 || op == 8) && !zm->m_groundCliffZones)
				|| (op == 8 && o[1] >= zm->m_zonesAllocated) || ((op == 22 || op == 23) && l.m_width > 0 && !l.m_bridge)
				|| (op == 27 && !l.m_bridge) || (op == 26 && l.m_destroyed && !zm->m_groundCliffZones)
				|| ((op == 21 || op == 28) && l.m_layerCells);
			if (bad) { putU32(0xbadc0de5u); break; }
		}
		size_t mark = g_out.size();
		putU32(0);
		switch (o[0]) {
		case 0: zm->calculateZones(pf->m_map, layers, extent); break;
		case 1: {
			IRegion2D sb;
			sb.lo.x = (Int)o[1]; sb.lo.y = (Int)o[2]; sb.hi.x = (Int)o[3]; sb.hi.y = (Int)o[4];
			zm->updateZonesForModify(pf->m_map, layers, sb, extent);
			break;
		}
		case 2: a->setType((PathfindCell::CellType)o[2]); break;
		case 3: a->setZone((zoneStorageType)o[2]); break;
		case 4: a->setConnectLayer((PathfindLayerEnum)o[2]); break;
		case 5: {
			ICoord2D p; p.x = (Int)((o[1] % count) / h); p.y = (Int)((o[1] % count) % h);
			res.push_back(p2SetTypeAsObstacle(a, (ObjectID)o[2], false, false, o[3] != 0, p) ? 1u : 0u);
			break;
		}
		case 6: res.push_back(p2RemoveObstacle(a, (ObjectID)o[2]) ? 1u : 0u); break;
		case 7: res.push_back(zm->getEffectiveZone((LocomotorSurfaceTypeMask)o[1], o[2] != 0, (zoneStorageType)o[3])); break;
		case 8: res.push_back(zm->getEffectiveTerrainZone((zoneStorageType)o[1])); break;
		case 9: res.push_back(zm->getBlockZone((LocomotorSurfaceTypeMask)o[1], o[2] != 0, (Int)o[3], (Int)o[4], pf->m_map)); break;
		case 10: zm->clearPassableFlags(); break;
		case 11: zm->setAllPassable(); break;
		case 12: zm->setPassable((Int)o[1], (Int)o[2], o[3] != 0); break;
		case 13: res.push_back(zm->isPassable((Int)o[1], (Int)o[2]) ? 1u : 0u); break;
		case 14: res.push_back(zm->clipIsPassable((Int)o[1], (Int)o[2]) ? 1u : 0u); break;
		case 15: zm->setBridge((Int)o[1], (Int)o[2], o[3] != 0); break;
		case 16: res.push_back(zm->interactsWithBridge((Int)o[1], (Int)o[2]) ? 1u : 0u); break;
		case 17:
			g_gameLogic.m_frame = o[1];
			zm->markZonesDirty(o[2] != 0);
			res.push_back(zm->needToCalculateZones() ? 1u : 0u);
			break;
		case 18: zm->allocateBlocks(extent); break;
		case 19: zm->reset(); break;
		case 20: res.push_back(layerOf(o[1]).init(o[2] == 0xffffffffu ? NULL : &bridges[o[2] % nb], (PathfindLayerEnum)o[3]) ? 1u : 0u); break;
		case 21: layerOf(o[1]).allocateCells(&extent); break;
		case 22: layerOf(o[1]).classifyCells(); break;
		case 23: res.push_back(layerOf(o[1]).setDestroyed(o[2] != 0) ? 1u : 0u); break;
		case 24: layerOf(o[1]).reset(); break;
		case 25: {
			PathfindLayer &l = layerOf(o[1]);
			PathfindCell *c = l.getCell((Int)o[2], (Int)o[3]);
			res.push_back(c ? (unsigned)(c - l.m_blockOfMapCells) : 0xffffffffu);
			break;
		}
		case 26:
			loco->m_validLocomotorSurfaces = (LocomotorSurfaceTypeMask)o[2];
			res.push_back(layerOf(o[1]).connectsZones(zm, *loco, (Int)o[3], (Int)o[4]) ? 1u : 0u);
			break;
		case 27: res.push_back((unsigned)layerOf(o[1]).getBridgeID()); break;
		case 28: layerOf(o[1]).allocateCellsForWallLayer(&extent, nw ? &pieces[0] : NULL, nw); break;
		case 29: layerOf(o[1]).classifyWallCells(nw ? &pieces[0] : NULL, nw); break;
		case 30: {
			Coord3D pt; pt.x = floatOf(o[2]); pt.y = floatOf(o[3]); pt.z = 0;
			res.push_back(layerOf(o[1]).isPointOnWall(nw ? &pieces[0] : NULL, nw, &pt) ? 1u : 0u);
			break;
		}
		case 31: layerOf(o[1]).setZone((Int)o[2]); layerOf(o[1]).applyZone(); break;
		case 32:
			for (int x = (Int)o[1]; x <= (Int)o[2]; ++x)
				for (int y = (Int)o[3]; y <= (Int)o[4]; ++y)
					if (x >= 0 && x < w && y >= 0 && y < h) columns[x][y].setZone(PathfindZoneManager::UNINITIALIZED_ZONE);
			break;
		}
		putU32((unsigned)res.size());
		for (unsigned r : res) putU32(r);
		/* the manager */
		putU32(zm->m_zoneBlocks ? 1u : 0u); putU32((unsigned)zm->m_zoneBlockExtent.x); putU32((unsigned)zm->m_zoneBlockExtent.y);
		putU32(zm->m_maxZone); putU32(zm->m_nextFrameToCalculateZones); putU32(zm->m_zonesAllocated);
		putU32(zm->m_groundCliffZones ? 1u : 0u);
		if (zm->m_groundCliffZones) {
			int m = (int)zm->m_maxZone + 1 < (int)zm->m_zonesAllocated ? (int)zm->m_maxZone + 1 : (int)zm->m_zonesAllocated;
			for (zoneStorageType *t : { zm->m_groundCliffZones, zm->m_groundWaterZones, zm->m_groundRubbleZones, zm->m_terrainZones,
					zm->m_crusherZones, zm->m_hierarchicalZones })
				for (int k = 0; k < m; ++k) putU32(t[k]);
		}
		if (zm->m_zoneBlocks) {
			for (int bx = 0; bx < zm->m_zoneBlockExtent.x; ++bx)
				for (int by = 0; by < zm->m_zoneBlockExtent.y; ++by) {
					const ZoneBlock &b = zm->m_zoneBlocks[bx][by];
					putU32((unsigned)b.m_cellOrigin.x); putU32((unsigned)b.m_cellOrigin.y); putU32(b.m_firstZone); putU32(b.m_numZones);
					putU32(b.m_zonesAllocated); putU32(b.m_interactsWithBridge ? 1u : 0u); putU32(b.m_markedPassable ? 1u : 0u);
					putU32(b.m_groundCliffZones ? 1u : 0u);
					if (b.m_groundCliffZones) {
						int m = b.m_numZones < b.m_zonesAllocated ? (int)b.m_numZones : (int)b.m_zonesAllocated;
						for (zoneStorageType *t : { b.m_groundCliffZones, b.m_groundWaterZones, b.m_groundRubbleZones, b.m_crusherZones })
							for (int k = 0; k < m; ++k) putU32(t[k]);
					}
				}
		}
		for (int k = 0; k < count; ++k) {
			const PathfindCell &c = cells[k];
			putU32((unsigned)c.getType()); putU32((unsigned)c.getZone()); putU32((unsigned)c.getConnectLayer()); putU32((unsigned)c.getLayer());
		}
		for (int k = 0; k <= LAYER_LAST; ++k) {
			PathfindLayer &l = layers[k];
			putU32((unsigned)l.m_width); putU32((unsigned)l.m_height); putU32((unsigned)l.m_xOrigin); putU32((unsigned)l.m_yOrigin);
			putU32((unsigned)l.m_startCell.x); putU32((unsigned)l.m_startCell.y); putU32((unsigned)l.m_endCell.x); putU32((unsigned)l.m_endCell.y);
			putU32((unsigned)l.m_layer); putU32((unsigned)l.m_zone); putU32(l.m_bridge ? l.m_bridge->m_index : 0xffffffffu);
			putU32(l.m_destroyed ? 1u : 0u); putU32(l.isUnused() ? 1u : 0u); putU32(l.m_layerCells ? 1u : 0u);
			if (l.m_layerCells)
				for (int x = 0; x < l.m_width; ++x)
					for (int y = 0; y < l.m_height; ++y) {
						const PathfindCell &c = l.m_layerCells[x][y];
						putU32((unsigned)c.getType()); putU32((unsigned)c.getZone()); putU32((unsigned)c.getConnectLayer());
						putU32((unsigned)c.getLayer()); putU32(c.getPinched() ? 1u : 0u);
					}
		}
		g_out[mark] = (unsigned)(g_out.size() - mark - 1);
	}
	for (int k = 0; k <= LAYER_LAST; ++k) layers[k].~PathfindLayer();
	delete zm;
	delete[] cells;
	PathfindCellInfo::releaseCellInfos();
	g_walls.clear();
	*len = (int)g_out.size();
	if (*len > cap) return 1;
	memcpy(buf, &g_out[0], g_out.size() * sizeof(unsigned));
	return 0;
}

/* Batch P4: the Pathfinder's map, footprints, wall and units' cells. cfg: [seed, cliff, water, bridge-layer and bridge-end rates,
** the terrain's extent (lo x y, hi x y; float bits), the wall height (bits), the bridges' four rates and the bridges (as P3's), the
** object count, per object 22 words: id, x, y, z, angle, kinds (bits: mine, projectile, bridge tower, defensive wall, blast crater,
** walk on top of wall, structure, immobile, produced at helipad, see-through), rubble, fence width, fence offset, mobile, geometry
** type, major, minor, small, bounding radius, height above terrain, layer, destination layer, AI, ground movement, adjusts
** destination, unmanned (floats as bits)]. Ops of 6 words [op, a, b, c, d, e] (objects by index a; positions as float bits):
** 0 newMap  1 reset  2 addBridge(bridge a)  3 addWallPiece(a)  4 removeWallPiece(a)  5 isPointOnWall(x a, y b)
** 6 updateLayer(a, layer b)  7 addObjectToPathfindMap(a)  8 removeObjectFromPathfindMap(a)  9 forceMapRecalculation
** 10 changeBridgeState(layer a, repaired b)  11 updateGoal(a, x b, y c, layer d)  12 updateAircraftGoal(a, x b, y c)  13 removeGoal(a)
** 14 updatePos(a, x b, y c)  15 removePos(a)  16 removeUnitFromPathfindMap(a)  17 getCell(layer a, x b, y c)
** 18 getCell and getClippedCell(layer a, x b, y c)  19 worldToCell(x a, y b)  20 object a's layer b  21 object a's ground movement b
** 22 the frame a. (The caller keeps to the calls upstream defines; a call this side finds undefined ends the dump with a marker.)
** Each op appends, after a word that counts the rest: the results; the pathfinder (extents, map ready, tunneling, wall pieces, wall
** height); the manager and its blocks (as P3's); the map's cells and each layer (as P3's, each cell also its flags, aircraft goal and
** info: goal, position, aircraft and obstacle units, fence, see-through, index); the info pool (first free, and to the highest info
** a cell has held: free, next); the objects (layer, destination layer, goal and position cells); the splats so far. */
int ref_pf_p4_run(const unsigned *cfg, int n, const unsigned *ops, unsigned *buf, int cap, int *len)
{
	g_out.clear();
	g_splats.clear();
	g_gameLogic.m_frame = 0;
	g_scene.seed = cfg[0];
	g_p4.cliff = cfg[1]; g_p4.water = cfg[2]; g_p4.layer = cfg[3]; g_p4.end = cfg[4];
	g_p4.extent.lo.x = floatOf(cfg[5]); g_p4.extent.lo.y = floatOf(cfg[6]); g_p4.extent.lo.z = 0;
	g_p4.extent.hi.x = floatOf(cfg[7]); g_p4.extent.hi.y = floatOf(cfg[8]); g_p4.extent.hi.z = 0;
	g_aiData.m_wallHeight = floatOf(cfg[9]);
	g_p3.onBridge = cfg[10]; g_p3.end = cfg[11]; g_p3.side = cfg[12]; g_p3.entry = cfg[13];
	int nb = (int)cfg[14], at = 15;
	std::vector<Bridge> bridges(nb);
	for (int k = 0; k < nb; ++k, at += 10) {
		Bridge &b = bridges[k];
		b.m_index = (unsigned)k;
		b.m_bounds.lo.x = floatOf(cfg[at]); b.m_bounds.lo.y = floatOf(cfg[at + 1]);
		b.m_bounds.hi.x = floatOf(cfg[at + 2]); b.m_bounds.hi.y = floatOf(cfg[at + 3]);
		b.m_info.from.x = floatOf(cfg[at + 4]); b.m_info.from.y = floatOf(cfg[at + 5]); b.m_info.from.z = floatOf(cfg[at + 6]);
		b.m_info.to.x = floatOf(cfg[at + 7]); b.m_info.to.y = floatOf(cfg[at + 8]); b.m_info.to.z = floatOf(cfg[at + 9]);
		b.m_info.bridgeObjectID = (ObjectID)(1000 + k);
	}
	int no = (int)cfg[at++];
	g_walls.assign(no, Object());
	static const KindOfType kinds[10] = { KINDOF_MINE, KINDOF_PROJECTILE, KINDOF_BRIDGE_TOWER, KINDOF_DEFENSIVE_WALL, KINDOF_BLAST_CRATER,
		KINDOF_WALK_ON_TOP_OF_WALL, KINDOF_STRUCTURE, KINDOF_IMMOBILE, KINDOF_PRODUCED_AT_HELIPAD, KINDOF_CAN_SEE_THROUGH_STRUCTURE };
	for (int k = 0; k < no; ++k, at += 22) {
		Object &o = g_walls[k];
		const unsigned *c = cfg + at;
		o.m_id = (ObjectID)c[0];
		o.m_pos.x = floatOf(c[1]); o.m_pos.y = floatOf(c[2]); o.m_pos.z = floatOf(c[3]); o.m_angle = floatOf(c[4]);
		for (int b = 0; b < 10; ++b) o.m_kinds[kinds[b]] = ((c[5] >> b) & 1) != 0;
		o.m_rubble = c[6] != 0;
		o.m_template.m_fenceWidth = floatOf(c[7]); o.m_template.m_fenceXOffset = floatOf(c[8]);
		o.m_mobile = c[9] != 0;
		o.m_geom.m_sphere = c[10] == GEOMETRY_SPHERE; o.m_geom.m_type = (int)c[10];
		o.m_geom.m_major = floatOf(c[11]); o.m_geom.m_minor = floatOf(c[12]); o.m_geom.m_small = c[13] != 0;
		o.m_geom.m_boundingRadius = floatOf(c[14]); o.m_heightAboveTerrain = floatOf(c[15]);
		o.m_layer = (PathfindLayerEnum)c[16]; o.m_destLayer = (PathfindLayerEnum)c[17];
		o.m_hasAI = c[18] != 0; o.m_ai.m_ground = c[19] != 0; o.m_ai.m_adjusts = c[20] != 0; o.m_unmanned = c[21] != 0;
		o.m_next = k + 1 < no ? &g_walls[k + 1] : NULL;
	}
	/* the real constructor (batch P7: the vtable is emitted with findPath): the info pool, then reset; zeroed memory first, as the
	** layers' unset m_layer is 0 in it */
	memset(g_pathfinderMem, 0, sizeof g_pathfinderMem);
	Pathfinder *pf = new (g_pathfinderMem) Pathfinder();
	int highWater = 0;
	auto objAt = [&](unsigned k) -> Object * { return &g_walls[k % (unsigned)no]; };
	auto posOf = [&](unsigned x, unsigned y) -> Coord3D { Coord3D p; p.x = floatOf(x); p.y = floatOf(y); p.z = 0; return p; };
	auto cellCode = [&](PathfindCell *c) -> unsigned {
		if (!c) return 0xffffffffu;
		int mh = pf->m_extent.hi.y + 1, count = (pf->m_extent.hi.x + 1) * mh;
		if (pf->m_blockOfMapCells && c >= pf->m_blockOfMapCells && c < pf->m_blockOfMapCells + count) return (unsigned)(c - pf->m_blockOfMapCells);
		for (int l = 0; l <= LAYER_LAST; ++l) {
			PathfindLayer &ly = pf->m_layers[l];
			if (ly.m_blockOfMapCells && c >= ly.m_blockOfMapCells && c < ly.m_blockOfMapCells + ly.m_width * ly.m_height)
				return 0x100000u | ((unsigned)l << 16) | (unsigned)(c - ly.m_blockOfMapCells);
		}
		return 0xfffffffeu;
	};
	auto infoIndex = [](const PathfindCellInfo *i) -> unsigned { return i ? (unsigned)(i - PathfindCellInfo::s_infoArray) : 0xffffffffu; };
	auto putCell = [&](const PathfindCell &c) {
		putU32((unsigned)c.getType()); putU32((unsigned)c.getFlags()); putU32((unsigned)c.getZone()); putU32(c.isAircraftGoal() ? 1u : 0u);
		putU32(c.getPinched() ? 1u : 0u); putU32((unsigned)c.getLayer()); putU32((unsigned)c.getConnectLayer()); putU32(infoIndex(c.m_info));
		if (c.m_info) {
			const PathfindCellInfo &f = *c.m_info;
			putU32((unsigned)f.m_goalUnitID); putU32((unsigned)f.m_posUnitID); putU32((unsigned)f.m_goalAircraftID); putU32((unsigned)f.m_obstacleID);
			putU32(f.m_obstacleIsFence); putU32(f.m_obstacleIsTransparent); putU32((unsigned)f.m_pos.x); putU32((unsigned)f.m_pos.y);
			if ((int)infoIndex(c.m_info) + 1 > highWater) highWater = (int)infoIndex(c.m_info) + 1;
		}
	};
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		std::vector<unsigned> res;
		{
			unsigned op = o[0];
			bool needMap = !(op == 1 || op == 2 || op == 3 || op == 4 || op == 6 || op == 19 || op == 20 || op == 21 || op == 22 || op == 0);
			bool bad = (needMap && !pf->m_map) || (op == 10 && (o[1] % (LAYER_LAST + 1)) == LAYER_WALL && !pf->m_layers[LAYER_WALL].m_bridge
				&& pf->m_layers[LAYER_WALL].m_width > 0);
			if (bad) { putU32(0xbadc0de5u); break; }
		}
		size_t mark = g_out.size();
		putU32(0);
		switch (o[0]) {
		case 0: pf->newMap(); break;
		case 1: pf->reset(); break;
		case 2: res.push_back((unsigned)pf->addBridge(&bridges[o[1] % nb])); break;
		case 3: pf->addWallPiece(objAt(o[1])); break;
		case 4: pf->removeWallPiece(objAt(o[1])); break;
		case 5: { Coord3D p = posOf(o[1], o[2]); res.push_back(pf->isPointOnWall(&p) ? 1u : 0u); break; }
		case 6: pf->updateLayer(objAt(o[1]), (PathfindLayerEnum)o[2]); break;
		case 7: pf->addObjectToPathfindMap(objAt(o[1])); break;
		case 8: pf->removeObjectFromPathfindMap(objAt(o[1])); break;
		case 9: pf->forceMapRecalculation(); break;
		case 10: pf->changeBridgeState((PathfindLayerEnum)(o[1] % (LAYER_LAST + 1)), o[2] != 0); break;
		case 11: { Coord3D p = posOf(o[2], o[3]); pf->updateGoal(objAt(o[1]), &p, (PathfindLayerEnum)o[4]); break; }
		case 12: { Coord3D p = posOf(o[2], o[3]); pf->updateAircraftGoal(objAt(o[1]), &p); break; }
		case 13: pf->removeGoal(objAt(o[1])); break;
		case 14: { Coord3D p = posOf(o[2], o[3]); pf->updatePos(objAt(o[1]), &p); break; }
		case 15: pf->removePos(objAt(o[1])); break;
		case 16: pf->removeUnitFromPathfindMap(objAt(o[1])); break;
		case 17: res.push_back(cellCode(pf->getCell((PathfindLayerEnum)o[1], (Int)o[2], (Int)o[3]))); break;
		case 18: {
			Coord3D p = posOf(o[2], o[3]);
			res.push_back(cellCode(pf->getCell((PathfindLayerEnum)o[1], &p)));
			res.push_back(cellCode(pf->getClippedCell((PathfindLayerEnum)o[1], &p)));
			break;
		}
		case 19: {
			Coord3D p = posOf(o[1], o[2]);
			ICoord2D c;
			Bool overflow = pf->worldToCell(&p, &c);
			res.push_back((unsigned)c.x); res.push_back((unsigned)c.y); res.push_back(overflow ? 1u : 0u);
			break;
		}
		case 20: objAt(o[1])->m_layer = (PathfindLayerEnum)o[2]; break;
		case 21: objAt(o[1])->m_ai.m_ground = o[2] != 0; break;
		case 22: g_gameLogic.m_frame = o[1]; break;
		}
		putU32((unsigned)res.size());
		for (unsigned r : res) putU32(r);
		putU32((unsigned)pf->m_extent.lo.x); putU32((unsigned)pf->m_extent.lo.y); putU32((unsigned)pf->m_extent.hi.x); putU32((unsigned)pf->m_extent.hi.y);
		putU32(pf->m_map ? 1u : 0u); putU32(pf->m_isMapReady ? 1u : 0u); putU32((unsigned)pf->m_numWallPieces);
		for (int k = 0; k < MAX_WALL_PIECES; ++k) putU32((unsigned)pf->m_wallPieces[k]);
		putU32(bitsOf(pf->m_wallHeight));
		PathfindZoneManager *zm = &pf->m_zoneManager;
		putU32(zm->m_zoneBlocks ? 1u : 0u); putU32((unsigned)zm->m_zoneBlockExtent.x); putU32((unsigned)zm->m_zoneBlockExtent.y);
		putU32(zm->m_maxZone); putU32(zm->m_nextFrameToCalculateZones); putU32(zm->m_zonesAllocated);
		putU32(zm->m_groundCliffZones ? 1u : 0u);
		if (zm->m_groundCliffZones) {
			int m = (int)zm->m_maxZone + 1 < (int)zm->m_zonesAllocated ? (int)zm->m_maxZone + 1 : (int)zm->m_zonesAllocated;
			for (zoneStorageType *t : { zm->m_groundCliffZones, zm->m_groundWaterZones, zm->m_groundRubbleZones, zm->m_terrainZones,
					zm->m_crusherZones, zm->m_hierarchicalZones })
				for (int k = 0; k < m; ++k) putU32(t[k]);
		}
		if (zm->m_zoneBlocks) {
			for (int bx = 0; bx < zm->m_zoneBlockExtent.x; ++bx)
				for (int by = 0; by < zm->m_zoneBlockExtent.y; ++by) {
					const ZoneBlock &b = zm->m_zoneBlocks[bx][by];
					putU32((unsigned)b.m_cellOrigin.x); putU32((unsigned)b.m_cellOrigin.y); putU32(b.m_firstZone); putU32(b.m_numZones);
					putU32(b.m_zonesAllocated); putU32(b.m_interactsWithBridge ? 1u : 0u); putU32(b.m_markedPassable ? 1u : 0u);
					putU32(b.m_groundCliffZones ? 1u : 0u);
					if (b.m_groundCliffZones) {
						int m = b.m_numZones < b.m_zonesAllocated ? (int)b.m_numZones : (int)b.m_zonesAllocated;
						for (zoneStorageType *t : { b.m_groundCliffZones, b.m_groundWaterZones, b.m_groundRubbleZones, b.m_crusherZones })
							for (int k = 0; k < m; ++k) putU32(t[k]);
					}
				}
		}
		if (pf->m_map) {
			int count = (pf->m_extent.hi.x + 1) * (pf->m_extent.hi.y + 1);
			for (int k = 0; k < count; ++k) putCell(pf->m_blockOfMapCells[k]);
		}
		for (int k = 0; k <= LAYER_LAST; ++k) {
			PathfindLayer &l = pf->m_layers[k];
			putU32((unsigned)l.m_width); putU32((unsigned)l.m_height); putU32((unsigned)l.m_xOrigin); putU32((unsigned)l.m_yOrigin);
			putU32((unsigned)l.m_startCell.x); putU32((unsigned)l.m_startCell.y); putU32((unsigned)l.m_endCell.x); putU32((unsigned)l.m_endCell.y);
			putU32((unsigned)l.m_layer); putU32((unsigned)l.m_zone); putU32(l.m_bridge ? l.m_bridge->m_index : 0xffffffffu);
			putU32(l.m_destroyed ? 1u : 0u); putU32(l.isUnused() ? 1u : 0u); putU32(l.m_layerCells ? 1u : 0u);
			if (l.m_layerCells)
				for (int x = 0; x < l.m_width; ++x)
					for (int y = 0; y < l.m_height; ++y) putCell(l.m_layerCells[x][y]);
		}
		putU32(infoIndex(PathfindCellInfo::s_firstFree));
		putU32((unsigned)highWater);
		for (int k = 0; k < highWater; ++k) {
			const PathfindCellInfo &f = PathfindCellInfo::s_infoArray[k];
			putU32(f.m_isFree ? 1u : 0u); putU32(infoIndex(f.m_pathParent));
		}
		for (Object &ob : g_walls) {
			putU32((unsigned)ob.m_layer); putU32((unsigned)ob.m_destLayer);
			putU32((unsigned)ob.m_ai.m_goal.x); putU32((unsigned)ob.m_ai.m_goal.y); putU32((unsigned)ob.m_ai.m_cur.x); putU32((unsigned)ob.m_ai.m_cur.y);
		}
		putU32((unsigned)g_splats.size());
		for (unsigned v : g_splats) putU32(v);
		g_out[mark] = (unsigned)(g_out.size() - mark - 1);
	}
	pf->reset();	/* (the cells first: the destructor releases the pool before its members' destructors run) */
	pf->~Pathfinder();
	g_walls.clear();
	*len = (int)g_out.size();
	if (*len > cap) return 1;
	memcpy(buf, &g_out[0], g_out.size() * sizeof(unsigned));
	return 0;
}

}  /* extern "C" */

/* Batches P5 and P6: the scene of P5's cfg (see ref_pf_p5_run), and the zeroed Pathfinder on it, reset. */
Player g_pfHuman = { 0 }, g_pfComputer = { PLAYER_COMPUTER };
Pathfinder *pfP5Setup(const unsigned *cfg, std::vector<Bridge> &bridges, int &no)
{
	g_out.clear();
	g_splats.clear();
	g_gameLogic.m_frame = 0;
	g_scene.seed = cfg[0];
	g_p4.cliff = cfg[1]; g_p4.water = cfg[2]; g_p4.layer = cfg[3]; g_p4.end = cfg[4];
	g_p4.extent.lo.x = floatOf(cfg[5]); g_p4.extent.lo.y = floatOf(cfg[6]); g_p4.extent.lo.z = 0;
	g_p4.extent.hi.x = floatOf(cfg[7]); g_p4.extent.hi.y = floatOf(cfg[8]); g_p4.extent.hi.z = 0;
	g_aiData.m_wallHeight = floatOf(cfg[9]);
	g_aiData.m_attackUsesLineOfSight = cfg[10] != 0;
	g_p3.onBridge = cfg[11]; g_p3.end = cfg[12]; g_p3.side = cfg[13]; g_p3.entry = cfg[14];
	g_p5Allies = cfg[15]; g_p5Crush = cfg[16]; g_p5Line = cfg[17];
	int nb = (int)cfg[18], at = 19;
	bridges.assign(nb, Bridge());
	for (int k = 0; k < nb; ++k, at += 10) {
		Bridge &b = bridges[k];
		b.m_index = (unsigned)k;
		b.m_bounds.lo.x = floatOf(cfg[at]); b.m_bounds.lo.y = floatOf(cfg[at + 1]);
		b.m_bounds.hi.x = floatOf(cfg[at + 2]); b.m_bounds.hi.y = floatOf(cfg[at + 3]);
		b.m_info.from.x = floatOf(cfg[at + 4]); b.m_info.from.y = floatOf(cfg[at + 5]); b.m_info.from.z = floatOf(cfg[at + 6]);
		b.m_info.to.x = floatOf(cfg[at + 7]); b.m_info.to.y = floatOf(cfg[at + 8]); b.m_info.to.z = floatOf(cfg[at + 9]);
		b.m_info.bridgeObjectID = (ObjectID)(1000 + k);
	}
	no = (int)cfg[at++];
	g_walls.assign(no, Object());
	static const KindOfType kinds[10] = { KINDOF_MINE, KINDOF_PROJECTILE, KINDOF_BRIDGE_TOWER, KINDOF_DEFENSIVE_WALL, KINDOF_BLAST_CRATER,
		KINDOF_WALK_ON_TOP_OF_WALL, KINDOF_STRUCTURE, KINDOF_IMMOBILE, KINDOF_PRODUCED_AT_HELIPAD, KINDOF_CAN_SEE_THROUGH_STRUCTURE };
	for (int k = 0; k < no; ++k, at += 32) {
		Object &o = g_walls[k];
		const unsigned *c = cfg + at;
		o.m_id = (ObjectID)c[0];
		o.m_pos.x = floatOf(c[1]); o.m_pos.y = floatOf(c[2]); o.m_pos.z = floatOf(c[3]); o.m_angle = floatOf(c[4]);
		for (int b = 0; b < 10; ++b) o.m_kinds[kinds[b]] = ((c[5] >> b) & 1) != 0;
		o.m_rubble = c[6] != 0;
		o.m_template.m_fenceWidth = floatOf(c[7]); o.m_template.m_fenceXOffset = floatOf(c[8]);
		o.m_mobile = c[9] != 0;
		o.m_geom.m_sphere = c[10] == GEOMETRY_SPHERE; o.m_geom.m_type = (int)c[10];
		o.m_geom.m_major = floatOf(c[11]); o.m_geom.m_minor = floatOf(c[12]); o.m_geom.m_small = c[13] != 0;
		o.m_geom.m_boundingRadius = floatOf(c[14]); o.m_heightAboveTerrain = floatOf(c[15]);
		o.m_layer = (PathfindLayerEnum)c[16]; o.m_destLayer = (PathfindLayerEnum)c[17];
		o.m_hasAI = c[18] != 0; o.m_ai.m_ground = c[19] != 0; o.m_ai.m_adjusts = c[20] != 0; o.m_unmanned = c[21] != 0;
		o.m_ai.m_ignored = (ObjectID)c[22]; o.m_crusherLevel = (Int)c[23]; o.m_crushableLevel = (Int)c[24]; o.m_above = c[25] != 0;
		o.m_kinds[KINDOF_ATTACK_NEEDS_LINE_OF_SIGHT] = (c[26] & 1) != 0; o.m_kinds[KINDOF_AIRCRAFT_PATH_AROUND] = (c[26] & 2) != 0;
		o.m_hasWeapon = c[27] != 0;
		if (c[28] != 0) {
			o.m_slavedUpdate.m_slaver = (ObjectID)c[28];
			o.m_slavedModule.m_slaved = &o.m_slavedUpdate;
			o.m_modules[0] = &o.m_slavedModule;
		}
	}
	at -= 32 * no;
	for (int k = 0; k < no; ++k, at += 32) {
		Object &o = g_walls[k];
		o.m_next = k + 1 < no ? &g_walls[k + 1] : NULL;
		o.m_container = cfg[at + 29] ? &g_walls[(cfg[at + 29] - 1) % no] : NULL;
		/* batch P6: word 30's bits: a player, a computer player, can path through units, dozer, vehicle */
		unsigned w30 = cfg[at + 30];
		o.m_player = (w30 & 1) ? ((w30 & 2) ? &g_pfComputer : &g_pfHuman) : NULL;
		o.m_ai.m_pathThrough = (w30 & 4) != 0;
		o.m_kinds[KINDOF_DOZER] = (w30 & 8) != 0;
		o.m_kinds[KINDOF_VEHICLE] = (w30 & 16) != 0;
	}
	/* the real constructor (batch P7: the vtable is emitted with findPath): the info pool, then reset; zeroed memory first, as the
	** layers' unset m_layer is 0 in it */
	memset(g_pathfinderMem, 0, sizeof g_pathfinderMem);
	Pathfinder *pf = new (g_pathfinderMem) Pathfinder();
	return pf;
}

/* Batches P5 and P6: the Pathfinder and its members torn down, the pool released. */
void pfP5Teardown(Pathfinder *pf)
{
	pf->m_openList = pf->m_closedList = NULL;
	pf->reset();	/* (the cells first: the destructor releases the pool before its members' destructors run) */
	pf->~Pathfinder();
	g_walls.clear();
}

extern "C" {

/* Batch P5: the Pathfinder's movement checks and lines, on a map built as P4's. cfg as P4's (with the AI data's attack-uses-line-of-
** sight word after the wall height, and the relation, crush and weapon-line rates after the bridges' rates), each object 32 words: P4's
** 22, then ignored obstacle, crusher level, crushable level, significantly above the terrain, kinds 2 (bits: attack needs line of
** sight, aircraft path around), weapon, slaver (0: no slaved module), container (object index + 1, or 0), (two unused). Ops of 8
** words [op, a, b, c, d, e, f, g] (objects by index, 0xffffffff none; positions as float bits):
** 0 newMap  1 addObjectToPathfindMap(a)  2 updateGoal(a, x b, y c, layer d)  3 updatePos(a, x b, y c)  4 setIgnoreObstacleID(a)
** 5 validMovementTerrain(layer a, legal surfaces b, x c, y d)  6 validMovementPosition(crusher a, surfaces b, getCell(layer c, d, e))
** 7 checkDestination(obj a, cell b c, layer d, radius e, centre f)  8 checkForMovement(obj a, cell b c, layer d, radius e, centre f,
** transient g)  9 snapPosition(a, x b, y c)  10 snapClosestGoalPosition(a, x b, y c)  11 goalPosition(a)
** 12 adjustToLandingDestination(a, x b, y c)  13 clearCellForDiameter(crusher a, cell b c, layer d, diameter e)
** 14 adjustCoordToCell(cell a b, centre c, layer d)  15 isLinePassable(obj a, surfaces b, layer c & 255 (blocked bit 8, pinched
** bit 9), from d e, to f g)  16 isGroundPathPassable(crusher a, from b c, layer d, to e f, diameter g)  17 isViewBlockedByObstacle(a, b)
** 18 isAttackViewBlockedByObstacle(a, at b c, victim d, at e f)  19 segmentIntersectsTallBuilding(from a b, to c d, ignoring e)
** 20 circleClipsTallBuilding(from a b, to c d, radius e)  21 the line of lineBlockedByObstacleCallback(a, b, from c d, to e f, layer g)
** 22 cell (a, b) of layer c onto the sorted open list (an info allocated if none; costs d)  23 onto the closed list
** 24 cleanOpenAndClosedLists  25 validLocomotorSurfacesForCellType(a). (The caller keeps to defined calls.) Each op appends, after a
** word that counts the rest: the results, the lists' heads, the cumulative cell count, the first free info. */
int ref_pf_p5_run(const unsigned *cfg, int n, const unsigned *ops, unsigned *buf, int cap, int *len)
{
	std::vector<Bridge> bridges;
	int no = 0;
	Pathfinder *pf = pfP5Setup(cfg, bridges, no);
	auto objAt = [&](unsigned k) -> Object * { return k == 0xffffffffu ? NULL : &g_walls[k % (unsigned)no]; };
	auto posOf = [&](unsigned x, unsigned y) -> Coord3D { Coord3D p; p.x = floatOf(x); p.y = floatOf(y); p.z = 0; return p; };
	auto cellCode = [&](PathfindCell *c) -> unsigned {
		if (!c) return 0xffffffffu;
		int mh = pf->m_extent.hi.y + 1, count = (pf->m_extent.hi.x + 1) * mh;
		if (pf->m_blockOfMapCells && c >= pf->m_blockOfMapCells && c < pf->m_blockOfMapCells + count) return (unsigned)(c - pf->m_blockOfMapCells);
		for (int l = 0; l <= LAYER_LAST; ++l) {
			PathfindLayer &ly = pf->m_layers[l];
			if (ly.m_blockOfMapCells && c >= ly.m_blockOfMapCells && c < ly.m_blockOfMapCells + ly.m_width * ly.m_height)
				return 0x100000u | ((unsigned)l << 16) | (unsigned)(c - ly.m_blockOfMapCells);
		}
		return 0xfffffffeu;
	};
	auto infoIndex = [](const PathfindCellInfo *i) -> unsigned { return i ? (unsigned)(i - PathfindCellInfo::s_infoArray) : 0xffffffffu; };
	auto putPos = [&](std::vector<unsigned> &res, const Coord3D &p) { res.push_back(bitsOf(p.x)); res.push_back(bitsOf(p.y)); res.push_back(bitsOf(p.z)); };
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 8 * i;
		std::vector<unsigned> res;
		if (o[0] != 0 && o[0] != 4 && o[0] != 25 && !pf->m_map) { putU32(0xbadc0de5u); break; }
		size_t mark = g_out.size();
		putU32(0);
		switch (o[0]) {
		case 0: pf->newMap(); break;
		case 1: pf->addObjectToPathfindMap(objAt(o[1])); break;
		case 2: { Coord3D p = posOf(o[2], o[3]); pf->updateGoal(objAt(o[1]), &p, (PathfindLayerEnum)o[4]); break; }
		case 3: { Coord3D p = posOf(o[2], o[3]); pf->updatePos(objAt(o[1]), &p); break; }
		case 4: pf->setIgnoreObstacleID(o[1] == 0xffffffffu ? INVALID_ID : objAt(o[1])->getID()); break;
		case 5: {
			Locomotor loco; loco.m_legal = (Int)o[2];
			Coord3D p = posOf(o[3], o[4]);
			res.push_back(pf->validMovementTerrain((PathfindLayerEnum)o[1], &loco, &p) ? 1u : 0u);
			break;
		}
		case 6: res.push_back(pf->validMovementPosition(o[1] != 0, (LocomotorSurfaceTypeMask)o[2], pf->getCell((PathfindLayerEnum)o[3], (Int)o[4], (Int)o[5])) ? 1u : 0u); break;
		case 7: res.push_back(pf->checkDestination(objAt(o[1]), (Int)o[2], (Int)o[3], (PathfindLayerEnum)o[4], (Int)o[5], o[6] != 0) ? 1u : 0u); break;
		case 8: {
			TCheckMovementInfo info;
			info.cell.x = (Int)o[2]; info.cell.y = (Int)o[3]; info.layer = (PathfindLayerEnum)o[4]; info.radius = (Int)o[5];
			info.centerInCell = o[6] != 0; info.considerTransient = o[7] != 0; info.acceptableSurfaces = 0;
			info.allyFixedCount = 77; info.enemyFixed = info.allyMoving = info.allyGoal = true;
			res.push_back(pf->checkForMovement(objAt(o[1]), info) ? 1u : 0u);
			res.push_back((unsigned)info.allyFixedCount); res.push_back(info.enemyFixed); res.push_back(info.allyMoving); res.push_back(info.allyGoal);
			break;
		}
		case 9: { Coord3D p = posOf(o[2], o[3]); pf->snapPosition(objAt(o[1]), &p); putPos(res, p); break; }
		case 10: { Coord3D p = posOf(o[2], o[3]); pf->snapClosestGoalPosition(objAt(o[1]), &p); putPos(res, p); break; }
		case 11: { Coord3D p = posOf(0x3f800000u, 0x3f800000u); res.push_back(pf->goalPosition(objAt(o[1]), &p) ? 1u : 0u); putPos(res, p); break; }
		case 12: { Coord3D p = posOf(o[2], o[3]); res.push_back(pf->adjustToLandingDestination(objAt(o[1]), &p) ? 1u : 0u); putPos(res, p); break; }
		case 13: res.push_back((unsigned)pf->clearCellForDiameter(o[1] != 0, (Int)o[2], (Int)o[3], (PathfindLayerEnum)o[4], (Int)o[5])); break;
		case 14: { Coord3D p = posOf(0, 0); pf->adjustCoordToCell((Int)o[1], (Int)o[2], o[3] != 0, p, (PathfindLayerEnum)o[4]); putPos(res, p); break; }
		case 15: {
			Coord3D a = posOf(o[4], o[5]), b = posOf(o[6], o[7]);
			res.push_back(pf->isLinePassable(objAt(o[1]), (LocomotorSurfaceTypeMask)o[2], (PathfindLayerEnum)(o[3] & 255), a, b, (o[3] >> 8) & 1, (o[3] >> 9) & 1) ? 1u : 0u);
			break;
		}
		case 16: {
			Coord3D a = posOf(o[2], o[3]), b = posOf(o[5], o[6]);
			res.push_back(pf->isGroundPathPassable(o[1] != 0, a, (PathfindLayerEnum)o[4], b, (Int)o[7]) ? 1u : 0u);
			break;
		}
		case 17: res.push_back(pf->isViewBlockedByObstacle(objAt(o[1]), objAt(o[2])) ? 1u : 0u); break;
		case 18: {
			Coord3D a = posOf(o[2], o[3]), b = posOf(o[5], o[6]);
			res.push_back(pf->isAttackViewBlockedByObstacle(objAt(o[1]), a, objAt(o[4]), b) ? 1u : 0u);
			break;
		}
		case 19: {
			Path *path = newInstance(Path);
			Coord3D a = posOf(o[1], o[2]), b = posOf(o[3], o[4]);
			path->appendNode(&a, LAYER_GROUND);
			path->appendNode(&b, LAYER_GROUND);
			Coord3D p1, p2, p3;
			p1.zero(); p2.zero(); p3.zero();
			Bool r = pf->segmentIntersectsTallBuilding(path->getFirstNode(), path->getFirstNode()->getNext(), (ObjectID)o[5], &p1, &p2, &p3);
			res.push_back(r ? 1u : 0u);
			putPos(res, *path->getFirstNode()->getNext()->getPosition());
			if (r) { putPos(res, p1); putPos(res, p2); putPos(res, p3); }
			path->deleteInstance();
			break;
		}
		case 20: {
			Coord3D a = posOf(o[1], o[2]), b = posOf(o[3], o[4]), adj;
			adj.zero();
			Bool r = pf->circleClipsTallBuilding(&a, &b, floatOf(o[5]), INVALID_ID, &adj);
			res.push_back(r ? 1u : 0u);
			if (r) putPos(res, adj);
			break;
		}
		case 21: {
			ViewBlockedStruct info;
			info.obj = objAt(o[1]); info.objOther = objAt(o[2]);
			Coord3D a = posOf(o[3], o[4]), b = posOf(o[5], o[6]);
			res.push_back((unsigned)pf->iterateCellsAlongLine(a, b, (PathfindLayerEnum)o[7], Pathfinder::lineBlockedByObstacleCallback, &info));
			break;
		}
		case 22: case 23: {
			PathfindCell *c = pf->getCell((PathfindLayerEnum)o[3], (Int)o[1], (Int)o[2]);
			if (!c->hasInfo()) { ICoord2D p; p.x = (Int)o[1]; p.y = (Int)o[2]; c->allocateInfo(p); }
			c->setCostSoFar((UnsignedShort)o[4]); c->setTotalCost((UnsignedShort)o[4]);
			if (o[0] == 22) pf->m_openList = c->putOnSortedOpenList(pf->m_openList);
			else pf->m_closedList = c->putOnClosedList(pf->m_closedList);
			break;
		}
		case 24: pf->cleanOpenAndClosedLists(); break;
		case 25: res.push_back((unsigned)Pathfinder::validLocomotorSurfacesForCellType((PathfindCell::CellType)o[1])); break;
		}
		putU32((unsigned)res.size());
		for (unsigned r : res) putU32(r);
		putU32(cellCode(pf->m_openList)); putU32(cellCode(pf->m_closedList)); putU32((unsigned)pf->m_cumulativeCellsAllocated);
		putU32(infoIndex(PathfindCellInfo::s_firstFree));
		g_out[mark] = (unsigned)(g_out.size() - mark - 1);
	}
	pfP5Teardown(pf);
	*len = (int)g_out.size();
	if (*len > cap) return 1;
	memcpy(buf, &g_out[0], g_out.size() * sizeof(unsigned));
	return 0;
}

/* Batch P6: the A* search (internalFindPath), on P5's scene (cfg as P5's; each object's word 30: bits player, computer player, can path
** through units, dozer, vehicle). The paths a search builds are optimized with the REAL line tests (g_p1RealLines). Ops of 8 words
** [op, a, b, c, d, e, f, g] (objects by index, 0xffffffff none; positions as float bits):
** 0 newMap  1 addObjectToPathfindMap(a)  2 updateGoal(a, x b, y c, layer d)  3 updatePos(a, x b, y c)  4 setIgnoreObstacleID(a)
** 5 internalFindPath(obj a, surfaces b, downhill only c, from d e, to f g)  6 clip(from a b, to c d)  7 ClipLine2D(p1 (a, b), p2 (c, d),
** region lo (e, f), hi (g & 0xffff, g >> 16))  8 clearPassableFlags  9 setAllPassable  10 the logical extent (a, b) .. (c, d)
** 11 addBridge(bridge a).  Batch P7's: 12 findPath(as 5)  13 findHierarchicalPath(human a, surfaces b, crusher c, from d e, to f g)
** 14 findClosestHierarchicalPath(as 13)  15 clientSafeQuickDoesPathExist(surfaces a, from b c, to d e)  16 ...ForUI(as 15)
** 17 findBrokenBridge(as 15)  18 changeBridgeState(layer a, repaired b)  19 forceMapRecalculation.  Batch P8's: 20 findGroundPath(
** diameter a, crusher b, from d e, to f g).
** Each op appends, after a word that counts the rest: the results (a path: its nodes, each position, layer, can-optimize and next
** optimized node's order + 1, then blocked-by-ally), tunneling, the lists' heads, the cumulative cell count, the first free info, the
** zone blocks' passable flags. */
int ref_pf_p6_run(const unsigned *cfg, int n, const unsigned *ops, unsigned *buf, int cap, int *len)
{
	g_out.clear();
	std::vector<Bridge> bridges;
	int no = 0;
	Pathfinder *pf = pfP5Setup(cfg, bridges, no);
	g_p1RealLines = true;
	auto objAt = [&](unsigned k) -> Object * { return k == 0xffffffffu ? NULL : &g_walls[k % (unsigned)no]; };
	auto posOf = [&](unsigned x, unsigned y) -> Coord3D { Coord3D p; p.x = floatOf(x); p.y = floatOf(y); p.z = 0; return p; };
	auto cellCode = [&](PathfindCell *c) -> unsigned {
		if (!c) return 0xffffffffu;
		int mh = pf->m_extent.hi.y + 1, count = (pf->m_extent.hi.x + 1) * mh;
		if (pf->m_blockOfMapCells && c >= pf->m_blockOfMapCells && c < pf->m_blockOfMapCells + count) return (unsigned)(c - pf->m_blockOfMapCells);
		for (int l = 0; l <= LAYER_LAST; ++l) {
			PathfindLayer &ly = pf->m_layers[l];
			if (ly.m_blockOfMapCells && c >= ly.m_blockOfMapCells && c < ly.m_blockOfMapCells + ly.m_width * ly.m_height)
				return 0x100000u | ((unsigned)l << 16) | (unsigned)(c - ly.m_blockOfMapCells);
		}
		return 0xfffffffeu;
	};
	auto infoIndex = [](const PathfindCellInfo *i) -> unsigned { return i ? (unsigned)(i - PathfindCellInfo::s_infoArray) : 0xffffffffu; };
	alignas(16) unsigned char locoMem[sizeof(LocomotorSet)];
	memset(locoMem, 0, sizeof locoMem);
	LocomotorSet *loco = (LocomotorSet *)locoMem;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 8 * i;
		std::vector<unsigned> res;
		if (o[0] != 0 && o[0] != 4 && o[0] != 7 && o[0] != 10 && o[0] != 11 && !pf->m_map) { putU32(0xbadc0de5u); break; }
		size_t mark = g_out.size();
		putU32(0);
		switch (o[0]) {
		case 0: pf->newMap(); break;
		case 1: pf->addObjectToPathfindMap(objAt(o[1])); break;
		case 2: { Coord3D p = posOf(o[2], o[3]); pf->updateGoal(objAt(o[1]), &p, (PathfindLayerEnum)o[4]); break; }
		case 3: { Coord3D p = posOf(o[2], o[3]); pf->updatePos(objAt(o[1]), &p); break; }
		case 4: pf->setIgnoreObstacleID(o[1] == 0xffffffffu ? INVALID_ID : objAt(o[1])->getID()); break;
		case 5: case 12: case 13: case 14: case 20: {
			loco->m_validLocomotorSurfaces = (LocomotorSurfaceTypeMask)o[2];
			loco->m_downhillOnly = o[3] != 0;
			Coord3D from = posOf(o[4], o[5]), to = posOf(o[6], o[7]);
			Path *path = NULL;
			if (o[0] == 5) path = pf->Pathfinder::internalFindPath(objAt(o[1]), *loco, &from, &to);
			else if (o[0] == 12) path = pf->findPath(objAt(o[1]), *loco, &from, &to);
			/* 13, 14: a is isHuman, c is crusher (not downhill) */
			/* 20: a is the diameter, b crusher (not surfaces), c unused */
			else if (o[0] == 20) path = pf->findGroundPath(&from, &to, (Int)o[1], o[2] != 0);
			else if (o[0] == 13) path = pf->findHierarchicalPath(o[1] != 0, *loco, &from, &to, o[3] != 0);
			else path = pf->findClosestHierarchicalPath(o[1] != 0, *loco, &from, &to, o[3] != 0);
			res.push_back(path ? 1u : 0u);
			if (path) {
				std::vector<PathNode *> nodes;
				for (PathNode *nd = path->getFirstNode(); nd; nd = nd->getNext()) nodes.push_back(nd);
				res.push_back((unsigned)nodes.size());
				for (PathNode *nd : nodes) {
					res.push_back(bitsOf(nd->getPosition()->x)); res.push_back(bitsOf(nd->getPosition()->y)); res.push_back(bitsOf(nd->getPosition()->z));
					res.push_back((unsigned)nd->getLayer()); res.push_back(nd->getCanOptimize() ? 1u : 0u);
					unsigned next = 0;
					for (size_t k = 0; k < nodes.size(); ++k) if (nodes[k] == nd->m_nextOpti) next = (unsigned)k + 1;
					res.push_back(next);
				}
				res.push_back(path->getBlockedByAlly() ? 1u : 0u);
				path->deleteInstance();
			}
			break;
		}
		case 6: {
			Coord3D from = posOf(o[1], o[2]), to = posOf(o[3], o[4]);
			pf->clip(&from, &to);
			res.push_back(bitsOf(from.x)); res.push_back(bitsOf(from.y)); res.push_back(bitsOf(to.x)); res.push_back(bitsOf(to.y));
			break;
		}
		case 7: {
			ICoord2D p1, p2, c1, c2;
			IRegion2D r;
			p1.x = (Int)o[1]; p1.y = (Int)o[2]; p2.x = (Int)o[3]; p2.y = (Int)o[4];
			r.lo.x = (Int)o[5]; r.lo.y = (Int)o[6]; r.hi.x = (Int)(o[7] & 0xffff); r.hi.y = (Int)(o[7] >> 16);
			c1.x = c1.y = c2.x = c2.y = 0x7777;
			Bool v = ClipLine2D(&p1, &p2, &c1, &c2, &r);
			res.push_back(v ? 1u : 0u); res.push_back((unsigned)c1.x); res.push_back((unsigned)c1.y); res.push_back((unsigned)c2.x); res.push_back((unsigned)c2.y);
			break;
		}
		case 8: pf->m_zoneManager.clearPassableFlags(); break;
		case 9: pf->m_zoneManager.setAllPassable(); break;
		case 11: res.push_back((unsigned)pf->addBridge(&bridges[o[1] % bridges.size()])); break;
		case 15: case 16: {
			loco->m_validLocomotorSurfaces = (LocomotorSurfaceTypeMask)o[1];
			Coord3D from = posOf(o[2], o[3]), to = posOf(o[4], o[5]);
			Bool r = o[0] == 15 ? pf->clientSafeQuickDoesPathExist(*loco, &from, &to) : pf->clientSafeQuickDoesPathExistForUI(*loco, &from, &to);
			res.push_back(r ? 1u : 0u);
			break;
		}
		case 17: {
			loco->m_validLocomotorSurfaces = (LocomotorSurfaceTypeMask)o[1];
			Coord3D from = posOf(o[2], o[3]), to = posOf(o[4], o[5]);
			ObjectID id = (ObjectID)12345;
			Bool r = pf->findBrokenBridge(*loco, &from, &to, &id);
			res.push_back(r ? 1u : 0u); res.push_back((unsigned)id);
			break;
		}
		case 18: pf->changeBridgeState((PathfindLayerEnum)(o[1] % (LAYER_LAST + 1)), o[2] != 0); break;
		case 19: pf->forceMapRecalculation(); break;
		case 10: pf->m_logicalExtent.lo.x = (Int)o[1]; pf->m_logicalExtent.lo.y = (Int)o[2]; pf->m_logicalExtent.hi.x = (Int)o[3]; pf->m_logicalExtent.hi.y = (Int)o[4]; break;
		}
		putU32((unsigned)res.size());
		for (unsigned r : res) putU32(r);
		putU32(pf->m_isTunneling ? 1u : 0u);
		putU32(cellCode(pf->m_openList)); putU32(cellCode(pf->m_closedList)); putU32((unsigned)pf->m_cumulativeCellsAllocated);
		putU32(infoIndex(PathfindCellInfo::s_firstFree));
		PathfindZoneManager *zm = &pf->m_zoneManager;
		for (int bx = 0; bx < zm->m_zoneBlockExtent.x; ++bx)
			for (int by = 0; by < zm->m_zoneBlockExtent.y; ++by) putU32(zm->m_zoneBlocks[bx][by].m_markedPassable ? 1u : 0u);
		g_out[mark] = (unsigned)(g_out.size() - mark - 1);
	}
	g_p1RealLines = false;
	pfP5Teardown(pf);
	*len = (int)g_out.size();
	if (*len > cap) return 1;
	memcpy(buf, &g_out[0], g_out.size() * sizeof(unsigned));
	return 0;
}

}  /* extern "C" */
