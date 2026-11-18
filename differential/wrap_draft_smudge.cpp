/* Differential-test wrapper for the smudge draft (#215, upstream commit 0a05454d), src/drafts/logic_common/smudge.rs:
**   GeneralsMD/Code/GameEngine/Include/GameClient/Smudge.h (read unchanged; WWVegas' dllist.h, Vector2.h and Vector3.h are upstream's own) and
**   GameEngine/Source/GameClient/System/Smudge.cpp from `DLListClass<Smudge> SmudgeSet::m_freeSmudgeList;` to the end of the file
**   (the static free list, SmudgeManager's constructor, destructor, init, reset, addSmudgeSet and removeSmudgeSet; SmudgeSet's
**   constructor, destructor, reset, addSmudgeToSet and removeSmudgeFromSet), cut verbatim by build.rs into smudge.inc.
**
** Stand-ins: W3DMPO (the shim's always.h: an empty base for the pool's `operator new` and `delete`), W3DMPO_GLUE (the memory pool's `operator new` glue, empty: `W3DNEW` is `new`); Vector2.h and Vector3.h are reached through
** differential/shim_draft_smudge forwarders (the header names them by another case than the files). `protected` and `private` are read as
** `public` (the test reads the lists). The destructor of SmudgeManager is NOT called: upstream's loop removes the head of the free SET
** list instead of the free SMUDGE list (the draft's header records it), which is undefined once that list is empty.
**
** Entry point (extern "C", tests/differential_draft_smudge.rs):
**   ref_smudge_run(ops, nops, out): runs the ops on one SmudgeManager. Each op is 3 words [kind, j, k]: j picks the (j mod n)-th
**   of the n used sets (a no-op when there is none), k picks the (k mod m)-th smudge of that set's used list:
**     0 addSmudgeSet()                    1 removeSmudgeSet(set j)
**     2 addSmudgeToSet() on set j         3 removeSmudgeFromSet(smudge k of set j) (a no-op when the set has none)
**     4 reset() of the manager            5 reset() of set j
**   Sets and smudges get numbers (separately) in the order they are first seen. After every op the state is written: the used sets' numbers, the free
**   sets', the free smudges', then for each set in number order its count and its used smudges. Each list is a length and its ids. */
#include "Lib/BaseType.h"
#include "always.h" /* the shim's: W3DMPO, the memory pool object base that dllist.h's nodes derive from, as an empty class */
#include "WW3D2/dllist.h"
#include <map>
#include <vector>
#define W3DMPO_GLUE(x)
#define W3DNEW new
#define protected public
#define private public
#include "GameClient/Smudge.h"
#undef protected
#undef private
#ifndef NULL
#define NULL 0
#endif
#include "smudge.inc"

namespace {
std::map<const void *, unsigned> theIds, theSmudgeIds;
std::vector<SmudgeSet *> theSets;
unsigned id_of(const void *p)
{
	std::map<const void *, unsigned>::iterator it = theIds.find(p);
	if (it != theIds.end()) return it->second;
	it = theSmudgeIds.find(p);
	return it == theSmudgeIds.end() ? 0xFFFFFFFFu : it->second;
}
void see_set(SmudgeSet *s) { if (theIds.find(s) == theIds.end()) { unsigned n = (unsigned)theIds.size(); theIds[s] = n; theSets.push_back(s); } }
void see_smudge(Smudge *s) { if (theSmudgeIds.find(s) == theSmudgeIds.end()) { unsigned n = (unsigned)theSmudgeIds.size(); theSmudgeIds[s] = n; } }
template <class T> int put_list(DLListClass<T> &l, unsigned *out, int &n)
{
	int at = n++;
	int count = 0;
	for (T *p = l.Head(); p; p = p->Succ()) { out[n++] = id_of(p); ++count; }
	out[at] = count;
	return count;
}
}

extern "C" int ref_smudge_run(const unsigned *ops, int nops, unsigned *out)
{
	theIds.clear();
	theSmudgeIds.clear();
	theSets.clear();
	/* the static free list starts empty in a fresh process; a previous run left its nodes owned by deleted managers: forget them */
	while (SmudgeSet::m_freeSmudgeList.Head()) SmudgeSet::m_freeSmudgeList.Remove_Head();
	SmudgeManager *m = new SmudgeManager;
	unsigned smudgeNo = 0; (void)smudgeNo;
	int n = 0;
	for (int k = 0; k < nops; ++k) {
		unsigned kind = ops[3 * k], j = ops[3 * k + 1], q = ops[3 * k + 2];
		std::vector<SmudgeSet *> used;
		for (SmudgeSet *s = m->m_usedSmudgeSetList.Head(); s; s = s->Succ()) used.push_back(s);
		SmudgeSet *set = used.empty() ? 0 : used[j % used.size()];
		if (kind == 0) see_set(m->addSmudgeSet());
		else if (kind == 1 && set) m->removeSmudgeSet(*set);
		else if (kind == 2 && set) see_smudge(set->addSmudgeToSet());
		else if (kind == 3 && set) {
			std::vector<Smudge *> sm;
			for (Smudge *p = set->m_usedSmudgeList.Head(); p; p = p->Succ()) sm.push_back(p);
			if (!sm.empty()) set->removeSmudgeFromSet(*sm[q % sm.size()]);
		}
		else if (kind == 4) m->reset();
		else if (kind == 5 && set) set->reset();
		/* the state */
		put_list(m->m_usedSmudgeSetList, out, n);
		put_list(m->m_freeSmudgeSetList, out, n);
		put_list(SmudgeSet::m_freeSmudgeList, out, n);
		for (size_t i = 0; i < theSets.size(); ++i) {
			out[n++] = (unsigned)theSets[i]->m_usedSmudgeCount;
			put_list(theSets[i]->m_usedSmudgeList, out, n);
		}
	}
	return n;
}
