/* Differential-test wrapper for the replay controls' callbacks (#215, group ui3b, upstream commit 0a05454d): src/drafts/ui/replay_controls.rs.
** Cut verbatim by build.rs (drafts_ui3b), no edit:
**   replay_controls.inc  GameClient/GUI/GUICallbacks/ReplayControls.cpp, from `WindowMsgHandledType ReplayControlInput` to the end
**                        (ReplayControlInput, ReplayControlSystem);
**   ui3b_msgs.inc, ui3b_gwm.inc, ui3b_gadget.inc   GameWindow.h's WindowMsgHandledType and GameWindowMessage, Gadget.h's GadgetGameMessage.
** Stand-ins: only the shared ones (differential/shim_ui3b/ui3b_stubs.h). Neither callback touches a window or a global.
** Entry point ref_ui3b_replay_run(in, n, out, cap): `in` = [nmsg, nmsg x [kind (0 input, 1 system), msg, d1, d2]]; output per message: [result (0 ignored, 1 handled)]. */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <string>
#include <vector>
#include "Lib/BaseType.h"

namespace ui3breplay {

#include "ui3b_stubs.h"
#include "ui3b_msgs.inc"
#include "ui3b_gwm.inc"
#include "ui3b_gadget.inc"
#include "replay_controls.inc"

}

extern "C" int ref_ui3b_replay_run(const unsigned *in, int n, unsigned *out, int cap)
{
	using namespace ui3breplay;
	(void)n;
	unsigned count = in[0];
	if ((int)count > cap) return -1;
	for (unsigned i = 0; i < count; ++i) {
		unsigned kind = in[1 + 4 * i], msg = in[2 + 4 * i], d1 = in[3 + 4 * i], d2 = in[4 + 4 * i];
		WindowMsgHandledType r = kind == 0 ? ReplayControlInput(NULL, msg, d1, d2) : ReplayControlSystem(NULL, msg, d1, d2);
		out[i] = r == MSG_HANDLED ? 1 : 0;
	}
	return (int)count;
}
