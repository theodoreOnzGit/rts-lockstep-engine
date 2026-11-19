/* Differential-test wrapper for the draft src/drafts/sweep_common_client/credits.rs (#215, group ui2, upstream commit
** 0a05454d), cut verbatim by build.rs (fn drafts_ui2) into OUT_DIR/draft_ui2_gen:
**   u2_cr_h.inc    GameEngine/Include/GameClient/Credits.h, the style enum (CREDIT_STYLE_TITLE ... MAX_CREDIT_STYLES);
**   u2_cr_c.inc    GameEngine/Source/GameClient/Credits.cpp, CreditsManager::addBlank and parseBlank (to parseText).
** No edit. Stand-ins: CreditsLine (m_style only; the real one's constructor sets the drawing members the cuts never
** read), CreditsManager (a std::list of CreditsLine pointers as the real one, and the two functions' declarations), INI
** (declared, never used: parseBlank reads nothing from it).
**
** Entry point (extern "C", tests/differential_draft_credits.rs):
**   ref_u2_cr_run(in, n, out, cap): `in` is n steps, 0 = addBlank(), 1 = parseBlank(NULL, manager, NULL, NULL); the
**   output is, after every step, the list's length, then at the end every line's m_style in list order. */
#include <list>
#include "Lib/BaseType.h"

namespace u2cr {
class INI;
#include "u2_cr_h.inc"
class CreditsLine { public: Int m_style; CreditsLine() : m_style(-1) {} };
class CreditsManager
{
public:
	static void parseBlank( INI* ini, void *instance, void *store, const void *userData );
	void addBlank( void );
	typedef std::list<CreditsLine *> CreditsLineList;
	CreditsLineList m_creditLineList;
};
#include "u2_cr_c.inc"
}

extern "C" int ref_u2_cr_run(const int *in, int n, int *out, int cap)
{
	u2cr::CreditsManager m;
	int o = 0;
	for (int i = 0; i < n; i++) {
		if (in[i] == 0) m.addBlank(); else u2cr::CreditsManager::parseBlank(NULL, &m, NULL, NULL);
		if (o < cap) out[o] = (int)m.m_creditLineList.size();
		o++;
	}
	for (u2cr::CreditsManager::CreditsLineList::iterator it = m.m_creditLineList.begin(); it != m.m_creditLineList.end(); ++it) {
		if (o < cap) out[o] = (*it)->m_style;
		o++;
	}
	return o;
}
