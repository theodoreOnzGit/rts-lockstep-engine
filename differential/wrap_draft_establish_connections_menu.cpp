/* Differential-test wrapper for the establish connections menu (#215, group ui3b, upstream commit 0a05454d): src/drafts/ui/establish_connections_menu.rs.
** Cut verbatim by build.rs (drafts_ui3b), no edit:
**   ecm_cpp.inc   GameClient/GUI/EstablishConnectionsMenu/EstablishConnectionsMenu.cpp, from `char *EstablishConnectionsMenu::m_playerReadyControlNames[]` to the end (the three name
**                 tables, the constructor and destructor, initMenu, endMenu, abortGame, setPlayerName, setPlayerStatus);
**   ecm_nat.inc   GameNetwork/NAT.h: the enum NATConnectionState;
**   the window manager's functions and enums (differential/shim_ui3b/ui3b_wm.h lists them: winGetWindowFromId is the one used here).
** Stand-ins: the mini window manager of ui3b_wm.h; class EstablishConnectionsMenu (EstablishConnectionsMenu.h's members, written out); MAX_SLOTS (8); UnicodeString (a vector of
**   16-bit units); TheGameText->fetch(label) answers [FNV-1a of the label & 0xFFFF, the label's length]; GadgetStaticTextSetText(control, text) logs [3, window id, length, units];
**   ShowEstablishConnectionsWindow() logs [1], HideEstablishConnectionsWindow() logs [2]; DEBUG_ASSERTCRASH is compiled out (release).
** The draft's `NetgearDelay` variant has no C++ counterpart (NATCONNECTIONSTATE_NETGEARDELAY is commented out in NAT.h); the test sends NATCONNECTIONSTATE_NOSTATE for it, which also
** takes upstream's final `else` (the unknown label), and the draft's other variants are mapped by name below.
** Not covered (upstream reads a NULL name for it): slot 7 and above.
**
** Entry point ref_ui3b_ecm_run(in, n, out, cap): `in` = [tree scene block, nsteps, nsteps x [kind (0 initMenu, 1 endMenu, 2 setPlayerName(slot a, name of b units: the b units follow), 3 setPlayerStatus(slot a,
** state ordinal b)), a, b, (units)]]. State ordinals: 0 NOSTATE (the draft's NetgearDelay), 1 WAITINGTOBEGIN, 2 WAITINGFORMANGLERRESPONSE, 3 WAITINGFORMANGLEDPORT, 4 WAITINGFORRESPONSE, 5 DONE, 6 FAILED.
** Output per step: the calls logged, then [100, the scene's dump]. */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <string>
#include <vector>
#include "Lib/BaseType.h"

namespace ui3becm {

#include "ui3b_msgs.inc"
#include "ui3b_gwm.inc"
#include "ui3b_gadget.inc"
#include "ui3b_status.inc"
#include "ui3b_winerr.inc"
#include "ui3b_wm.h"
#include "ecm_nat.inc"

enum { MAX_SLOTS = 8 };
#define DEBUG_ASSERTCRASH(cond, msg)
struct UnicodeString { std::vector<unsigned short> w; };
struct GameText
{
	UnicodeString fetch(const char *label)
	{
		UnicodeString u;
		u.w.push_back((unsigned short)(fnv(label) & 0xFFFF)); u.w.push_back((unsigned short)strlen(label));
		return u;
	}
};
static GameText g_text;
static GameText *TheGameText = &g_text;
static void GadgetStaticTextSetText(GameWindow *w, UnicodeString t)
{
	lg(3); lg((unsigned)w->m_id); lg((unsigned)t.w.size());
	for (size_t i = 0; i < t.w.size(); ++i) lg(t.w[i]);
}
static void ShowEstablishConnectionsWindow() { lg(1); }
static void HideEstablishConnectionsWindow() { lg(2); }
enum EstablishConnectionsMenuStateType { ESTABLISHCONNECTIONSMENUSTATETYPE_SCREENON, ESTABLISHCONNECTIONSMENUSTATETYPE_SCREENOFF };
class EstablishConnectionsMenu
{
public:
	EstablishConnectionsMenu();
	virtual ~EstablishConnectionsMenu();
	void initMenu();
	void endMenu();
	void abortGame();
	void setPlayerName(Int slot, UnicodeString name);
	void setPlayerStatus(Int slot, NATConnectionState state);
protected:
	EstablishConnectionsMenuStateType m_menuState;
	static char *m_playerReadyControlNames[MAX_SLOTS];
	static char *m_playerNameControlNames[MAX_SLOTS];
	static char *m_playerStatusControlNames[MAX_SLOTS];
};

#include "ecm_cpp.inc"

}

extern "C" int ref_ui3b_ecm_run(const unsigned *in, int n, unsigned *out, int cap)
{
	using namespace ui3becm;
	(void)n;
	int p = readScene(in, 0);
	static const NATConnectionState states[7] = { NATCONNECTIONSTATE_NOSTATE, NATCONNECTIONSTATE_WAITINGTOBEGIN, NATCONNECTIONSTATE_WAITINGFORMANGLERRESPONSE, NATCONNECTIONSTATE_WAITINGFORMANGLEDPORT,
		NATCONNECTIONSTATE_WAITINGFORRESPONSE, NATCONNECTIONSTATE_DONE, NATCONNECTIONSTATE_FAILED };
	EstablishConnectionsMenu menu;
	unsigned nsteps = in[p++];
	std::vector<unsigned> all;
	for (unsigned s = 0; s < nsteps; ++s) {
		unsigned kind = in[p++], a = in[p++], b = in[p++];
		g_log.clear();
		switch (kind) {
		case 0: menu.initMenu(); break;
		case 1: menu.endMenu(); break;
		case 2: { UnicodeString u; for (unsigned i = 0; i < b; ++i) u.w.push_back((unsigned short)in[p++]); menu.setPlayerName((Int)a, u); break; }
		default: menu.setPlayerStatus((Int)a, states[b]); break;
		}
		for (size_t i = 0; i < g_log.size(); ++i) all.push_back(g_log[i]);
		all.push_back(100);
		dumpScene(all);
	}
	if ((int)all.size() > cap) return -1;
	memcpy(out, &all[0], all.size() * sizeof(unsigned));
	return (int)all.size();
}
