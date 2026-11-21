/* Differential-test wrapper for the credits menu (#215, group ui3b, upstream commit 0a05454d): src/drafts/ui/credits_menu.rs.
** Cut verbatim by build.rs (drafts_ui3b), no edit:
**   credits_menu.inc   GameClient/GUI/GUICallbacks/Menus/CreditsMenu.cpp, from `static NameKeyType parentMainMenuID` to the end (the two statics, CreditsMenuInit,
**                      CreditsMenuShutdown, CreditsMenuUpdate, CreditsMenuInput, CreditsMenuSystem);
**   the window manager's functions and enums (differential/shim_ui3b/ui3b_wm.h lists them), upstream's own KeyDefs.h and Common/AudioHandleSpecialValues.h.
** Stand-ins: the mini window manager of ui3b_wm.h; `AsciiString` (a name, converting to const char *); TheShell: showShellMap(b) logs [20] (FALSE) or [21] (TRUE), pop() [22],
**   shutdownComplete(layout) [23]; TheCredits (NULL or a CreditsManager): the constructor logs [30], load() [31], init() [32], update() [33], reset() [34], the destructor [35],
**   isFinished() answers the case's flag; TheAudio: removeAudioEvent(handle) logs [40], addAudioEvent(event) logs [41, the event's shouldFade]; AudioEventRTS(name) and setShouldFade.
** The Rust side decodes the same words from the verified Shell (see tests/differential_draft_credits_menu.rs): with the shell map on, not in a game, an active shell and no background,
** showShellMap(TRUE) requests a new shell game [21] and showShellMap(FALSE) loads the blank background [20]; with a screen on the stack whose shutdown is named, pop() runs it [22]; with a
** push pending, shutdownComplete() makes the pending screen [23].
** Not covered (upstream dereferences NULL): CreditsMenuShutdown with no TheCredits.
**
** Entry point ref_ui3b_cr_run(in, n, out, cap): `in` = [tree scene block, layout block (n then member indices, head first), TheCredits present, finished, nsteps, nsteps x [kind
** (0 CreditsMenuInit, 1 CreditsMenuShutdown, 2 CreditsMenuUpdate, 3 CreditsMenuInput(window a, msg b, d1 c, d2 d), 4 CreditsMenuSystem(window a, msg b, d1 c (GBM_SELECTED: window + 1), d2 d),
** 5 the finished flag becomes a), a, b, c, d]]. Output per step: the calls logged, then [100, result (-1 for the others), the scene's dump, parentMainMenuID, parentMainMenu + 1,
** TheCredits present, finished, the layout's hidden flag]. */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <string>
#include <vector>
#include "Lib/BaseType.h"
#include "GameClient/KeyDefs.h"
#include "Common/AudioHandleSpecialValues.h"

namespace ui3bcr {

#include "ui3b_msgs.inc"
#include "ui3b_gwm.inc"
#include "ui3b_gadget.inc"
#include "ui3b_status.inc"
#include "ui3b_winerr.inc"
#include "ui3b_wm.h"

struct AsciiString { const char *s; AsciiString(const char *x) : s(x) {} operator const char *() const { return s; } };
struct Shell
{
	void showShellMap(Bool b) { lg(b ? 21 : 20); }
	void pop() { lg(22); }
	void shutdownComplete(WindowLayout *) { lg(23); }
};
static Shell g_shell;
static Shell *TheShell = &g_shell;
static bool g_finished = false;
class CreditsManager
{
public:
	CreditsManager() { lg(30); }
	~CreditsManager() { lg(35); }
	void load() { lg(31); }
	void init() { lg(32); }
	void update() { lg(33); }
	Bool isFinished() { return g_finished; }
	void reset() { lg(34); }
};
static CreditsManager *TheCredits = NULL;
struct AudioEventRTS { bool fade; AudioEventRTS(AsciiString) : fade(false) {} void setShouldFade(Bool b) { fade = b; } };
struct GameAudio
{
	void removeAudioEvent(int) { lg(40); }
	void addAudioEvent(AudioEventRTS *e) { lg(41); lg(e->fade ? 1 : 0); }
};
static GameAudio g_audio;
static GameAudio *TheAudio = &g_audio;

#include "credits_menu.inc"

}

extern "C" int ref_ui3b_cr_run(const unsigned *in, int n, unsigned *out, int cap)
{
	using namespace ui3bcr;
	(void)n;
	g_unitSystem = CreditsMenuSystem;
	int p = readScene(in, 0);
	unsigned nl = in[p++];
	WindowLayout layout;
	for (unsigned i = 0; i < nl; ++i) { GameWindow *w = &g_wins[in[p++]]; layout.windows.push_back(w); w->m_layout = &layout; }
	parentMainMenuID = NAMEKEY_INVALID; parentMainMenu = NULL;
	TheCredits = in[p++] ? new CreditsManager : NULL;
	g_finished = in[p++] != 0;
	unsigned nsteps = in[p++];
	std::vector<unsigned> all;
	for (unsigned s = 0; s < nsteps; ++s) {
		unsigned kind = in[p++], a = in[p++], b = in[p++], c = in[p++], d = in[p++];
		g_log.clear();
		int res = -1;
		switch (kind) {
		case 0: CreditsMenuInit(&layout, NULL); break;
		case 1: CreditsMenuShutdown(&layout, NULL); break;
		case 2: CreditsMenuUpdate(&layout, NULL); break;
		case 3: res = CreditsMenuInput(&g_wins[a], b, c, d) == MSG_HANDLED ? 1 : 0; break;
		case 4: { WindowMsgData m1 = b == GBM_SELECTED ? (WindowMsgData)&g_wins[c - 1] : (WindowMsgData)c; Bool wf = d != 0; WindowMsgData m2 = b == GWM_INPUT_FOCUS ? (WindowMsgData)&wf : (WindowMsgData)d;
			res = CreditsMenuSystem(&g_wins[a], b, m1, m2) == MSG_HANDLED ? 1 : 0; if (b == GWM_INPUT_FOCUS) res = res * 2 + (wf ? 1 : 0); break; }
		default: g_finished = a != 0; break;
		}
		for (size_t i = 0; i < g_log.size(); ++i) all.push_back(g_log[i]);
		all.push_back(100); all.push_back((unsigned)res);
		dumpScene(all);
		all.push_back((unsigned)parentMainMenuID); all.push_back(winIndex(parentMainMenu)); all.push_back(TheCredits ? 1 : 0); all.push_back(g_finished ? 1 : 0); all.push_back(layout.hidden ? 1 : 0);
	}
	delete TheCredits; TheCredits = NULL;
	if ((int)all.size() > cap) return -1;
	memcpy(out, &all[0], all.size() * sizeof(unsigned));
	return (int)all.size();
}
