/* Differential-test wrapper for the popup communicator (#215, group ui3b, upstream commit 0a05454d): src/drafts/ui/popup_communicator.rs.
** Cut verbatim by build.rs (drafts_ui3b), no edit:
**   popup_communicator.inc   GameClient/GUI/GUICallbacks/Menus/PopupCommunicator.cpp, from `static NameKeyType buttonOkID` to the end (the three statics,
**                            PopupCommunicatorInit, PopupCommunicatorShutdown, PopupcommunicatorUpdate, PopupCommunicatorInput, PopupCommunicatorSystem);
**   the window manager's functions and enums (differential/shim_ui3b/ui3b_wm.h lists them: winSetFocus, winSendSystemMsg, winSetModal, winGetWindowFromId, winHide, ...)
**   and upstream's own KeyDefs.h.
** Stand-ins: the mini window manager of ui3b_wm.h (windows in a tree, layouts, focus, modal stack; a window whose system callback is the unit's own runs
** PopupCommunicatorSystem, the others TestSystem which logs [900, id, mData1] and wants the focus), `AsciiString` (a name, converting to const char *).
** Not covered (upstream dereferences NULL): GBM_SELECTED to a window with no layout, or with a NULL control.
**
** Entry point ref_ui3b_pc_run(in, n, out, cap): `in` = [tree scene block, layout block (n members then member indices, head first: every window is a member), nsteps, nsteps x [kind
** (0 PopupCommunicatorInit, 1 PopupCommunicatorInput(window a, msg b, d1 c, d2 d), 2 PopupCommunicatorSystem(window a, msg b, d1 c, d2 d; for GBM_SELECTED c is a window index + 1)), a, b, c, d]].
** Output per step: the calls logged ([900, id, d1] focus messages, [911] destroyWindows, [913] deleteInstance), then [100, result (-1 for init; for GWM_INPUT_FOCUS handled * 2 + the Bool afterwards, whose first value is d2), the scene's dump, buttonOkID, parent + 1,
** buttonOk + 1]. */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <string>
#include <vector>
#include "Lib/BaseType.h"
#include "GameClient/KeyDefs.h"

namespace ui3bpc {

#include "ui3b_msgs.inc"
#include "ui3b_gwm.inc"
#include "ui3b_gadget.inc"
#include "ui3b_status.inc"
#include "ui3b_winerr.inc"
#include "ui3b_wm.h"

struct AsciiString { const char *s; AsciiString(const char *x) : s(x) {} operator const char *() const { return s; } };

#include "popup_communicator.inc"

}

extern "C" int ref_ui3b_pc_run(const unsigned *in, int n, unsigned *out, int cap)
{
	using namespace ui3bpc;
	(void)n;
	g_unitSystem = PopupCommunicatorSystem;
	int p = readScene(in, 0);
	unsigned nl = in[p++];
	WindowLayout layout;
	for (unsigned i = 0; i < nl; ++i) { GameWindow *w = &g_wins[in[p++]]; layout.windows.push_back(w); w->m_layout = &layout; }
	buttonOkID = NAMEKEY_INVALID; buttonOk = NULL; parent = NULL;
	unsigned nsteps = in[p++];
	std::vector<unsigned> all;
	for (unsigned s = 0; s < nsteps; ++s) {
		unsigned kind = in[p++], a = in[p++], b = in[p++], c = in[p++], d = in[p++];
		g_log.clear();
		int res = -1;
		if (kind == 0) PopupCommunicatorInit(NULL, NULL);
		else {
			WindowMsgData m1 = (kind == 2 && b == GBM_SELECTED) ? (WindowMsgData)&g_wins[c - 1] : (WindowMsgData)c;
			Bool wf = d != 0;
			WindowMsgData m2 = (kind == 2 && b == GWM_INPUT_FOCUS) ? (WindowMsgData)&wf : (WindowMsgData)d;
			WindowMsgHandledType r = kind == 1 ? PopupCommunicatorInput(&g_wins[a], b, m1, m2) : PopupCommunicatorSystem(&g_wins[a], b, m1, m2);
			res = r == MSG_HANDLED ? 1 : 0;
			if (kind == 2 && b == GWM_INPUT_FOCUS) res = res * 2 + (wf ? 1 : 0);
		}
		for (size_t i = 0; i < g_log.size(); ++i) all.push_back(g_log[i]);
		all.push_back(100); all.push_back((unsigned)res);
		dumpScene(all);
		all.push_back((unsigned)buttonOkID); all.push_back(winIndex(parent)); all.push_back(winIndex(buttonOk));
	}
	if ((int)all.size() > cap) return -1;
	memcpy(out, &all[0], all.size() * sizeof(unsigned));
	return (int)all.size();
}
