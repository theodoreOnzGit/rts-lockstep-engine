/* Differential-test wrapper for the establish connections window (#215, group ui3b, upstream commit 0a05454d): src/drafts/ui/establish_connections_window.rs.
** Cut verbatim by build.rs (drafts_ui3b), no edit:
**   ecw.inc   GameClient/GUI/GUICallbacks/Menus/EstablishConnectionsWindow.cpp, from `static WindowLayout *establishConnectionsLayout` to the end (the statics and tables,
**             showGameSpyGameOptionsUnderlyingGUIElements, showGameSpyQMUnderlyingGUIElements, InitEstablishConnectionsDialog, ShowEstablishConnectionsWindow,
**             HideEstablishConnectionsWindow, EstablishConnectionsControlInput, EstablishConnectionsControlSystem);
**   the window manager's functions and enums (differential/shim_ui3b/ui3b_wm.h lists them).
** Stand-ins: the mini window manager of ui3b_wm.h; `AsciiString`; ShowUnderlyingGUIElements(show, layout file, parent name, gadgets, per-player gadgets) logs [10, show, FNV-1a of the file, of the
**   parent name, the number of gadgets and their FNV-1a, the number of per-player gadgets and theirs]; TheGameSpyGame->isQMGame() (the case's flag); TheEstablishConnectionsMenu->abortGame() logs [30].
** Not covered (upstream dereferences NULL): the layout file failing to load (`establishConnectionsLayout->hide`), GBM_SELECTED with a NULL control.
**
** Entry point ref_ui3b_ecw_run(in, n, out, cap): `in` = [tree scene block, nmembers then the layout's member indices (head first), is QM, nsteps, nsteps x [kind (0 ShowEstablishConnectionsWindow,
** 1 HideEstablishConnectionsWindow, 2 EstablishConnectionsControlInput(msg b), 3 EstablishConnectionsControlSystem(window a, msg b, d1 c (GBM_SELECTED: window + 1)), 4 the QM flag becomes a), a, b, c]].
** Output per step: the calls logged, then [100, result (-1 for the others), the scene's dump, buttonQuitID, buttonQuitWindow + 1, the layout exists]. */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <string>
#include <vector>
#include "Lib/BaseType.h"

namespace ui3becw {

#include "ui3b_msgs.inc"
#include "ui3b_gwm.inc"
#include "ui3b_gadget.inc"
#include "ui3b_status.inc"
#include "ui3b_winerr.inc"
#include "ui3b_wm.h"

struct AsciiString { const char *s; AsciiString(const char *x) : s(x) {} operator const char *() const { return s; } };
static void ShowUnderlyingGUIElements(Bool show, const char *layoutFilename, const char *parentName, const char **gadgetsToHide, const char **perPlayerGadgetsToHide)
{
	lg(10); lg(show ? 1 : 0); lg(fnv(layoutFilename)); lg(fnv(parentName));
	unsigned n = 0; while (gadgetsToHide[n]) ++n;
	lg(n); for (unsigned i = 0; i < n; ++i) lg(fnv(gadgetsToHide[i]));
	n = 0; while (perPlayerGadgetsToHide[n]) ++n;
	lg(n); for (unsigned i = 0; i < n; ++i) lg(fnv(perPlayerGadgetsToHide[i]));
}
static bool g_qm = false;
struct GameSpyGame { Bool isQMGame() { return g_qm; } };
static GameSpyGame g_game;
static GameSpyGame *TheGameSpyGame = &g_game;
struct EstablishConnectionsMenu { void abortGame() { lg(30); } };
static EstablishConnectionsMenu g_ecm;
static EstablishConnectionsMenu *TheEstablishConnectionsMenu = &g_ecm;

#include "ecw.inc"

}

extern "C" int ref_ui3b_ecw_run(const unsigned *in, int n, unsigned *out, int cap)
{
	using namespace ui3becw;
	(void)n;
	int p = readScene(in, 0);
	unsigned nm = in[p++];
	g_wm.layoutLoads = true;
	for (unsigned i = 0; i < nm; ++i) g_wm.layoutMembers.push_back(&g_wins[in[p++]]);
	g_qm = in[p++] != 0;
	establishConnectionsLayout = NULL; buttonQuitID = NAMEKEY_INVALID; buttonQuitWindow = NULL;
	unsigned nsteps = in[p++];
	std::vector<unsigned> all;
	for (unsigned s = 0; s < nsteps; ++s) {
		unsigned kind = in[p++], a = in[p++], b = in[p++], c = in[p++];
		g_log.clear();
		int res = -1;
		switch (kind) {
		case 0: ShowEstablishConnectionsWindow(); break;
		case 1: HideEstablishConnectionsWindow(); break;
		case 2: res = EstablishConnectionsControlInput(NULL, b, c, 0) == MSG_HANDLED ? 1 : 0; break;
		case 3: { WindowMsgData m1 = b == GBM_SELECTED ? (WindowMsgData)&g_wins[c - 1] : (WindowMsgData)c; res = EstablishConnectionsControlSystem(&g_wins[a], b, m1, 0) == MSG_HANDLED ? 1 : 0; break; }
		default: g_qm = a != 0; break;
		}
		for (size_t i = 0; i < g_log.size(); ++i) all.push_back(g_log[i]);
		all.push_back(100); all.push_back((unsigned)res);
		dumpScene(all);
		all.push_back((unsigned)buttonQuitID); all.push_back(winIndex(buttonQuitWindow)); all.push_back(establishConnectionsLayout ? 1 : 0);
	}
	if ((int)all.size() > cap) return -1;
	memcpy(out, &all[0], all.size() * sizeof(unsigned));
	return (int)all.size();
}
