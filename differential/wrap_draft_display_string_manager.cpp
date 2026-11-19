/* Differential-test wrapper for the DisplayStringManager draft (#215, upstream commit 0a05454d), src/drafts/logic_common/display_string_manager.rs:
**   GeneralsMD/Code/GameEngine/Source/GameClient/DisplayStringManager.cpp from `DisplayStringManager::DisplayStringManager( void )` to
**   the end of the file (the constructor, destructor, link and unLink), cut verbatim by build.rs into display_string_manager.inc.
**   (Upstream's file ends there: DisplayStringManager's other functions are in the header or the device's subclass.)
**
** Stand-ins (outside the cut, in an anonymous namespace): `assert` is empty (compiled out of the release build); DisplayString is
** the two link fields `m_next` and `m_prev` (DisplayString.h lines 112-113; the string's text is not touched by the cut); the
** declaration of DisplayStringManager keeps the two members and the four functions of DisplayStringManager.h.
**
** Entry point (extern "C", tests/differential_draft_display_string_manager.rs): ref_dsm_run(ops, nops, out) on an arena of 8 strings
** whose links start NULL: ops [0 i] link(string i), [1 i] unLink(string i); after each op the manager's head index (-1 for NULL) and
** every string's next and prev index. Returns the count of words. */
#include <string.h>
#include <vector>

namespace {

#define assert(x)

class DisplayString
{
public:
	DisplayString *m_next;
	DisplayString *m_prev;
};

class DisplayStringManager
{
public:
	DisplayStringManager(void);
	~DisplayStringManager(void);
	void link(DisplayString *string);
	void unLink(DisplayString *string);
	DisplayString *m_stringList;
	DisplayString *m_currentCheckpoint;
};

#include "display_string_manager.inc"

DisplayString arena[8];

int idx(const DisplayString *p) { return p == 0 ? -1 : (int)(p - arena); }

}

extern "C" {

int ref_dsm_run(const int *ops, int nops, int *out)
{
	std::vector<int> log;
	for (int i = 0; i < 8; ++i) { arena[i].m_next = 0; arena[i].m_prev = 0; }
	DisplayStringManager *m = new DisplayStringManager;
	for (int p = 0; p < nops; p += 2)
	{
		if (ops[p] == 0) m->link(&arena[ops[p + 1]]);
		else m->unLink(&arena[ops[p + 1]]);
		log.push_back(idx(m->m_stringList));
		for (int i = 0; i < 8; ++i) { log.push_back(idx(arena[i].m_next)); log.push_back(idx(arena[i].m_prev)); }
	}
	log.push_back(idx(m->m_currentCheckpoint));
	m->m_stringList = 0; /* the destructor asserts the list is empty (compiled out); leave it so */
	delete m;
	for (size_t i = 0; i < log.size(); ++i) out[i] = log[i];
	return (int)log.size();
}

}
