/* Differential-test wrapper for the purchase science screen's callbacks (#215, group ui3b, upstream commit 0a05454d):
** src/drafts/ui/generals_exp_points.rs. Cut verbatim by build.rs (drafts_ui3b), no edit:
**   exp_points.inc   GameClient/GUI/GUICallbacks/GeneralsExpPoints.cpp, from `WindowMsgHandledType GeneralsExpPointsInput` to the end
**                    (GeneralsExpPointsInput, GeneralsExpPointsSystem);
**   ui3b_msgs.inc, ui3b_gwm.inc   GameClient/GameWindow.h: WindowMsgHandledType, and GameWindowMessage;
**   ui3b_gadget.inc  GameClient/Gadget.h: GadgetGameMessage;
** and upstream's own GameClient/KeyDefs.h (KEY_ESC), with the DIK_ codes from differential/shim_ui3b/dinput.h.
** Stand-ins (the rest in differential/shim_ui3b/ui3b_stubs.h): TheInGameUI (present or NULL per case) whose placeBuildAvailable(NULL, NULL) is
** logged as [1]; TheControlBar whose hidePurchaseScience() is logged [2] and processContextSensitiveButtonClick(control, msg) [3, control id, msg].
** Statics: `static NameKeyType buttonExitID` inside the cut is function-local upstream and initialised once; the stand-in NAMEKEY is pure, so
** that does not change an answer.
** Not covered: GBM_SELECTED with a NULL control (upstream dereferences NULL).
**
** Entry point ref_ui3b_exp_run(in, n, out, cap): `in` = [nwin, ids x nwin, nsteps, steps x [kind (0 input, 1 system), msg, d1, d1_is_window (then d1 = index + 1
** of a window), d2 (0/1: the initial *(Bool *)mData2), has_ui]]. Output per step: the calls logged, then [100, result (0 ignored, 1 handled), the Bool after]. */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <string>
#include <vector>
#include "Lib/BaseType.h"
#include "GameClient/KeyDefs.h"

namespace ui3bexp {

#include "ui3b_stubs.h"
#include "ui3b_msgs.inc"
#include "ui3b_gwm.inc"
#include "ui3b_gadget.inc"

struct InGameUI { void placeBuildAvailable(void *, void *) { lg(1); } };
static InGameUI g_ui;
static InGameUI *TheInGameUI = &g_ui;
struct ControlBar
{
	void hidePurchaseScience() { lg(2); }
	void processContextSensitiveButtonClick(GameWindow *control, GadgetGameMessage msg) { lg(3); lg((unsigned)control->id); lg((unsigned)msg); }
};
static ControlBar g_cb;
static ControlBar *TheControlBar = &g_cb;

#include "exp_points.inc"

}

extern "C" int ref_ui3b_exp_run(const unsigned *in, int n, unsigned *out, int cap)
{
	using namespace ui3bexp;
	(void)n;
	int p = 0;
	std::vector<GameWindow> wins(in[p++]);
	for (size_t i = 0; i < wins.size(); ++i) wins[i].id = (Int)in[p++];
	g_wm.table.clear();
	for (size_t i = 0; i < wins.size(); ++i) g_wm.table.push_back(&wins[i]);
	unsigned nsteps = in[p++];
	g_log.clear();
	std::vector<unsigned> all;
	for (unsigned s = 0; s < nsteps; ++s) {
		unsigned kind = in[p++], msg = in[p++], d1 = in[p++], d1win = in[p++], d2 = in[p++], hasUi = in[p++];
		TheInGameUI = hasUi ? &g_ui : NULL;
		g_log.clear();
		Bool flag = d2 != 0;
		WindowMsgData m1 = d1win ? (WindowMsgData)g_wm.table[d1 - 1] : (WindowMsgData)d1;
		WindowMsgData m2 = (WindowMsgData)&flag;
		WindowMsgHandledType r = kind == 0 ? GeneralsExpPointsInput(NULL, msg, m1, m2) : GeneralsExpPointsSystem(NULL, msg, m1, m2);
		for (size_t i = 0; i < g_log.size(); ++i) all.push_back(g_log[i]);
		all.push_back(100); all.push_back(r == MSG_HANDLED ? 1 : 0); all.push_back(flag ? 1 : 0);
	}
	if ((int)all.size() > cap) return -1;
	memcpy(out, &all[0], all.size() * sizeof(unsigned));
	return (int)all.size();
}
