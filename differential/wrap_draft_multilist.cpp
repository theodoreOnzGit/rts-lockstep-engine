/* Differential-test wrapper for the draft src/drafts/sweep_wwlib/multilist.rs (#215, group wwlib3), upstream commit 0a05454d:
**   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/multilist.h and multilist.cpp (copied by build.rs so that their quoted includes find the stand-ins; the
**   one edit is `this->` before the five uses of CurNode in PriorityMultiListIterator, a dependent-base lookup g++ rejects).
** Stand-ins: mempool.h (AutoPoolClass, DEFINE_AUTO_POOL: allocation machinery) and wwmemlog.h (WWMEMLOG, a log): see
** differential/shim_draft_wwlib3. `assert` is compiled out (NDEBUG), as in the release build. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include "multilist.h"
#include "multilist.cpp"

/* GenericMultiListClass's operations are protected: a list class that publishes them */
class TestList : public GenericMultiListClass {
public:
	bool add(MultiListObjectClass *o, bool once) { return Internal_Add(o, once); }
	bool add_tail(MultiListObjectClass *o, bool once) { return Internal_Add_Tail(o, once); }
	bool add_after(MultiListObjectClass *o, MultiListObjectClass *e, bool once) { return Internal_Add_After(o, e, once); }
	bool remove(MultiListObjectClass *o) { return Internal_Remove(o); }
	MultiListObjectClass *remove_head() { return Internal_Remove_List_Head(); }
	~TestList() { while (!Is_Empty()) Internal_Remove_List_Head(); }
};

class TestIt : public GenericMultiListIterator {
public:
	TestIt(GenericMultiListClass *l) : GenericMultiListIterator(l) {}
	MultiListObjectClass *cur() { return Current_Object(); }
};

static const int NL = 3, NO = 8;

extern "C" int ref_ml_run(const int *prog, int nops, int *out, int cap)
{
	TestList lists[NL];
	MultiListObjectClass *objs[NO];
	for (int i = 0; i < NO; i++) objs[i] = new MultiListObjectClass;
	int n = 0;
	auto put = [&](int w) { if (n < cap) out[n] = w; n++; };
	auto idx = [&](MultiListObjectClass *o) { if (o == 0) return -1; for (int i = 0; i < NO; i++) if (objs[i] == o) return i; return -2; };
	const int *p = prog;
	for (int k = 0; k < nops; k++) {
		int op = *p++;
		int l = *p++, o = *p++;
		switch (op) {
		case 0: put(lists[l].add(objs[o], *p++ != 0)); break;
		case 1: put(lists[l].add_tail(objs[o], *p++ != 0)); break;
		case 2: { int e = *p++; bool once = *p++ != 0; put(lists[l].add_after(objs[o], objs[e], once)); break; }
		case 3: put(lists[l].remove(objs[o])); break;
		case 4: put(idx(lists[l].remove_head())); break;
		case 5: delete objs[o]; objs[o] = new MultiListObjectClass; break; /* ~MultiListObjectClass removes it from every list */
		case 6: put(lists[l].Contains(objs[o])); break;
		}
		for (int li = 0; li < NL; li++) {
			put(lists[li].Count());
			put(lists[li].Is_Empty());
			TestIt it(&lists[li]);
			for (it.First(); !it.Is_Done(); it.Next()) put(idx(it.cur()));
		}
	}
	for (int i = 0; i < NO; i++) delete objs[i];
	return n;
}
