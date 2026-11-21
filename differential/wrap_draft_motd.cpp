/* Differential-test wrapper for the message of the day window's system callback (#215, group ui3b, upstream commit 0a05454d):
** src/drafts/ui/w3d_motd.rs. Cut verbatim by build.rs (drafts_ui3b), no edit:
**   motd.inc   GameEngineDevice/Source/W3DDevice/GameClient/GUI/GUICallbacks/W3DMOTD.cpp, from `static NameKeyType closeButtonID` to the end
**              (the static `closeButtonID`, MOTDSystem);
**   ui3b_msgs.inc, ui3b_gwm.inc, ui3b_gadget.inc   GameWindow.h's WindowMsgHandledType and GameWindowMessage, Gadget.h's GadgetGameMessage.
** Stand-ins: the shared ones (differential/shim_ui3b/ui3b_stubs.h: name keys, windows). The static is reset to NAMEKEY_INVALID at the start of a sequence
** (the game starts it so).
** Not covered: GBM_SELECTED with a NULL control (upstream dereferences NULL).
** Entry point ref_ui3b_motd_run(in, n, out, cap): `in` = [scene block, nsteps, nsteps x [window index, msg, d1 (for GBM_SELECTED: control index + 1), d2]];
** output per step: [100, result (0 ignored, 1 handled), the scene's dump, the static id]. */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <string>
#include <vector>
#include "Lib/BaseType.h"

namespace ui3bmotd {

#include "ui3b_stubs.h"
#include "ui3b_msgs.inc"
#include "ui3b_gwm.inc"
#include "ui3b_gadget.inc"
#include "motd.inc"

}

extern "C" int ref_ui3b_motd_run(const unsigned *in, int n, unsigned *out, int cap)
{
	using namespace ui3bmotd;
	(void)n;
	int p = readScene(in, 0);
	closeButtonID = NAMEKEY_INVALID;
	unsigned nsteps = in[p++];
	std::vector<unsigned> all;
	for (unsigned s = 0; s < nsteps; ++s) {
		unsigned wi = in[p++], msg = in[p++], d1 = in[p++], d2 = in[p++];
		WindowMsgData m1 = msg == GBM_SELECTED ? (WindowMsgData)&g_wins[d1 - 1] : (WindowMsgData)d1;
		WindowMsgHandledType r = MOTDSystem(&g_wins[wi], msg, m1, d2);
		all.push_back(100); all.push_back(r == MSG_HANDLED ? 1 : 0);
		dumpScene(all);
		all.push_back((unsigned)closeButtonID);
	}
	if ((int)all.size() > cap) return -1;
	memcpy(out, &all[0], all.size() * sizeof(unsigned));
	return (int)all.size();
}
