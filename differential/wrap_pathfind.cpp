/* Differential-test wrapper for the pathfinder (p1w13-sonnet, layer 10; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h            (#included, UNCHANGED)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp        (verbatim cuts by build.rs, OUT_DIR/pathfind_gen)
** on the stand-ins of differential/shim_pathfind (pathfind_world.h: the object, the terrain logic, the AI and the game logic; PreRTS.h
** renames them and every class of AIPathfind.h), with the real BaseType.h, Snapshot.h, the Xfer classes, GameMemory and LocomotorSet.h.
**
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

/* the bridge layers' cells (batch P3's): none yet, so a layer's cell is the ground's (Pathfinder::getCell, inline) */
PathfindCell *PathfindLayer::getCell( Int x, Int y )
{
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

}  /* extern "C" */
