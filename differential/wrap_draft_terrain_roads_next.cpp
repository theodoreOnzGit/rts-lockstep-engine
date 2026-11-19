/* Differential-test wrapper for the draft src/drafts/sweep_common_client/terrain_roads.rs (#215, group ui2, upstream
** commit 0a05454d), cut verbatim by build.rs (fn drafts_ui2) into OUT_DIR/draft_ui2_gen:
**   u2_tr_c.inc   GameEngine/Source/GameClient/Terrain/TerrainRoads.cpp, TerrainRoadCollection::nextRoad and nextBridge
**                 (to the end of the file).
** No edit. Stand-ins: TerrainRoadType (m_id, m_isBridge, m_next, with isBridge, friend_getNext as TerrainRoads.h has
** them), TerrainRoadCollection (m_roadList, m_bridgeList, m_idCounter from 1; the two cut functions; newRoad and
** newBridge reduced to what builds the list: a new type, its id from m_idCounter++, its bridge flag, `friend_setNext(
** m_xList ); m_xList = x;`, the "link to list" lines of the real ones, which also copy the DefaultRoad and
** DefaultBridge fields the lists' order does not depend on); DEBUG_ASSERTCRASH as nothing (compiled out of the
** release build).
**
** Entry point (extern "C", tests/differential_draft_terrain_roads_next.rs):
**   ref_u2_tr_run(in, n, out, cap): `in` is n steps, 0 = newRoad, 1 = newBridge; the output is the ids of the road list
**   from its head (TerrainRoadCollection::m_roadList) following nextRoad, a 0 word, then the bridge list's ids following
**   nextBridge from m_bridgeList. */
#include "Lib/BaseType.h"

namespace u2tr {
#define DEBUG_ASSERTCRASH(a, b)
class TerrainRoadType
{
public:
	TerrainRoadType() : m_id(0), m_isBridge(FALSE), m_next(NULL) {}
	Bool isBridge() const { return m_isBridge; }
	TerrainRoadType *friend_getNext() { return m_next; }
	void friend_setNext( TerrainRoadType *next ) { m_next = next; }
	Int m_id; Bool m_isBridge; TerrainRoadType *m_next;
};
class TerrainRoadCollection
{
public:
	TerrainRoadCollection() : m_roadList(NULL), m_bridgeList(NULL), m_idCounter(1) {}
	TerrainRoadType *nextRoad( TerrainRoadType *road );
	TerrainRoadType *nextBridge( TerrainRoadType *bridge );
	TerrainRoadType *newRoad() { TerrainRoadType *r = new TerrainRoadType; r->m_id = m_idCounter++; r->m_isBridge = FALSE; r->friend_setNext( m_roadList ); m_roadList = r; return r; }
	TerrainRoadType *newBridge() { TerrainRoadType *b = new TerrainRoadType; b->m_id = m_idCounter++; b->m_isBridge = TRUE; b->friend_setNext( m_bridgeList ); m_bridgeList = b; return b; }
	TerrainRoadType *m_roadList, *m_bridgeList; Int m_idCounter;
};
#include "u2_tr_c.inc"
}

extern "C" int ref_u2_tr_run(const int *in, int n, int *out, int cap)
{
	u2tr::TerrainRoadCollection c;
	for (int i = 0; i < n; i++) { if (in[i] == 0) c.newRoad(); else c.newBridge(); }
	int o = 0;
	for (u2tr::TerrainRoadType *r = c.m_roadList; r; r = c.nextRoad(r)) { if (o < cap) out[o] = r->m_id; o++; }
	if (o < cap) out[o] = 0;
	o++;
	for (u2tr::TerrainRoadType *b = c.m_bridgeList; b; b = c.nextBridge(b)) { if (o < cap) out[o] = b->m_id; o++; }
	return o;
}
