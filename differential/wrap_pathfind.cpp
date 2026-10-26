/* Differential-test wrapper for the pathfinder (p1w13-sonnet, layer 10; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h            (#included, UNCHANGED)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp        (verbatim cuts by build.rs, OUT_DIR/pathfind_gen)
** on the stand-ins of differential/shim_pathfind (pathfind_world.h: the object, the terrain logic, the AI and the game logic; PreRTS.h
** renames them and every class of AIPathfind.h), with the real BaseType.h, Snapshot.h, the Xfer classes, GameMemory and LocomotorSet.h.
**
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
#include "path_p1.inc"
#include "cell_p2.inc"
/* batch P3: what the zone code takes from headers this build does not include: MIN (the Windows macro) and `register` (C++17
** removed it; -Wno-register in build.rs) */
#ifndef MIN
#define MIN(a, b) (((a) < (b)) ? (a) : (b))
#endif
#include "zone_p3.inc"
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

Bool Pathfinder::isLinePassable( const Object *obj, LocomotorSurfaceTypeMask acceptableSurfaces, PathfindLayerEnum layer, const Coord3D& startWorld,
	const Coord3D& endWorld, Bool blocked, Bool allowPinched )
{
	unsigned h = p1Hash(2, { (unsigned)obj->getID(), (unsigned)acceptableSurfaces, (unsigned)layer, bitsOf(startWorld.x), bitsOf(startWorld.y),
		bitsOf(endWorld.x), bitsOf(endWorld.y), (unsigned)blocked, (unsigned)allowPinched });
	bool r = p1Pick(h, g_scene.lineRate);
	logf("linepassable %d %u %d %08x %08x %08x %08x %d %d -> %d", (int)obj->getID(), (unsigned)acceptableSurfaces, (int)layer, bitsOf(startWorld.x),
		bitsOf(startWorld.y), bitsOf(endWorld.x), bitsOf(endWorld.y), blocked ? 1 : 0, allowPinched ? 1 : 0, r ? 1 : 0);
	return r;
}

Bool Pathfinder::isGroundPathPassable( Bool isCrusher, const Coord3D& startWorld, PathfindLayerEnum startLayer, const Coord3D& endWorld, Int pathDiameter )
{
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

}  /* extern "C" */
