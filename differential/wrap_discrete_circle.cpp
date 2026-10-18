/* Differential-test wrapper for GameEngine/Source/Common/DiscreteCircle.cpp
** and Include/Common/DiscreteCircle.h (upstream commit 0a05454d). The .cpp
** is #included unchanged, after <vector> (which the real PreRTS.h brings,
** through STLTypedefs.h, and the shim PreRTS.h does not).
**
** ref_discrete_circle builds DiscreteCircle(x, y, r) and writes its edges
** (yPos, xStart, xEnd per edge) and the calls drawCircle makes (xStart,
** xEnd, yPos per call). Returns the edge count, or -1 if a buffer was too
** small; *calls receives the call count. Callers pass only inputs where
** upstream is defined (r >= 0, no Int overflow). */
#include <vector>
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/DiscreteCircle.cpp"

namespace {
struct Calls { int *out; int cap; int n; };
void record(Int xStart, Int xEnd, Int yPos, void *p)
{
	Calls *c = (Calls *)p;
	if (3 * (c->n + 1) <= c->cap) {
		c->out[3 * c->n] = xStart; c->out[3 * c->n + 1] = xEnd; c->out[3 * c->n + 2] = yPos;
	}
	++c->n;
}
}

extern "C" int ref_discrete_circle(int x, int y, int r, int *edges, int edgeCap, int *calls, int callCap, int *callCount)
{
	DiscreteCircle c(x, y, r);
	const VecHorzLine &e = c.getEdges();
	if ((int)e.size() * 3 > edgeCap) return -1;
	for (size_t i = 0; i < e.size(); ++i) { edges[3 * i] = e[i].yPos; edges[3 * i + 1] = e[i].xStart; edges[3 * i + 2] = e[i].xEnd; }
	Calls k = { calls, callCap, 0 };
	c.drawCircle(record, &k);
	*callCount = k.n;
	if (3 * k.n > callCap) return -1;
	return c.getEdgeCount();
}
