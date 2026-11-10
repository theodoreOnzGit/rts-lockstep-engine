/* Differential-test wrapper for the input gates of GitHub #116 (upstream commit 0a05454d), cut verbatim by build.rs into
** OUT_DIR/input_gate_gen:
**   window_xlat.inc     GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/WindowXlat.cpp, from
**                       rawMouseToWindowMessage to the end of the file (WindowTranslator's constructor, destructor and
**                       translateGameMessage);
**   command_gate.inc    CommandXlat.cpp, CommandTranslator::translateGameMessage from its first line to the debug build's
**                       locals after the input gate, closed by build.rs with `return disp; }`;
**   system_message.inc  CommandXlat.cpp, isSystemMessage (to the end of the file);
**   selection_gate.inc  SelectionXlat.cpp, SelectionTranslator::translateGameMessage from its first line to its switch,
**                       closed by build.rs with `return disp; }`;
**   set_input_enabled.inc  InGameUI.cpp, InGameUI::setInputEnabled.
** They are compiled inside namespace igt on stand-ins for the classes and singletons they name. The messages are the
** real GameMessage of the message stream build (wrap_message_stream.cpp), under its names. The stand-ins answer from a
** case the test sets and log the calls that act, in the format the test's Rust host logs.
**
** Entry point (extern "C", tests/differential_input_gate.rs):
**   ref_input_gate_case(kind, in, nin, out, cap)  runs one case and writes its words to out; returns the count.
**     kind 0: WindowTranslator::translateGameMessage. in: type, pos.x, pos.y, arg1 (an integer, or for a drag the
**             delta's x), delta.y, flags (bit 0 mouse locked, 1 scrolling, 2 input enabled, 3 placement anchored, 4 shell
**             active, 5 the window uses the mouse, 6 the window uses the key, 7 a movie plays, 8 exit out of movies
**             allowed), mouse status pos.x, pos.y. out: disposition, stopMovie calls, then per winProcessMouseEvent call
**             (gwm, pos.x, pos.y, data kind (0 NULL, 1 delta, 2 wheel), data x, data y) and per winProcessKey call
**             (100, key, state, 0, 0, 0).
**     kind 1: CommandTranslator's gate. in: type, input enabled. out: disposition.
**     kind 2: SelectionTranslator's gate. in: type, input enabled, m_dragSelecting, isSelecting. out: disposition,
**             m_dragSelecting, isSelecting, endAreaSelectHint calls, setMouseLock(FALSE) calls.
**     kind 3: InGameUI::setInputEnabled. in: enable, m_inputEnabled, isSelecting, force attack, force move, waypoint,
**             prefer selection. out: those six after, then the four camera flags' clears (rotate left/right, zoom
**             in/out: 1 when set to FALSE). */
#define GameMessage MsTestGameMessage
#define GameMessageArgument MsTestGameMessageArgument
#define GameMessageList MsTestGameMessageList
#define GameMessageTranslator MsTestGameMessageTranslator
#define MessageStream MsTestMessageStream
#define CommandList MsTestCommandList
#define TheMessageStream MsTestTheMessageStream
#define TheCommandList MsTestTheCommandList
#define buildRegion MsTestBuildRegion

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <vector>
#define PI 3.14159265359f
#include "PreRTS.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#include "Common/MessageStream.h"

namespace igt {

std::vector<int> g_out;
Int g_flags;
ICoord2D g_mouseStatus;
Int g_stopMovie;

/* ---- GameWindow.h's enums and KeyDefs.h's two names (values as upstream) ---- */
enum GameWindowMessage
{
	GWM_NONE = 0,
	GWM_CREATE, GWM_DESTROY, GWM_ACTIVATE, GWM_ENABLE,
	GWM_LEFT_DOWN, GWM_LEFT_UP, GWM_LEFT_DOUBLE_CLICK, GWM_LEFT_DRAG,
	GWM_MIDDLE_DOWN, GWM_MIDDLE_UP, GWM_MIDDLE_DOUBLE_CLICK, GWM_MIDDLE_DRAG,
	GWM_RIGHT_DOWN, GWM_RIGHT_UP, GWM_RIGHT_DOUBLE_CLICK, GWM_RIGHT_DRAG,
	GWM_MOUSE_ENTERING, GWM_MOUSE_LEAVING, GWM_WHEEL_UP, GWM_WHEEL_DOWN,
	GWM_CHAR, GWM_SCRIPT_CREATE, GWM_INPUT_FOCUS, GWM_MOUSE_POS, GWM_IME_CHAR, GWM_IME_STRING,
};
enum WinInputReturnCode { WIN_INPUT_NOT_USED = 0, WIN_INPUT_USED };
typedef UnsignedInt WindowMsgData;
enum { KEY_ESC = 0x01 };
enum { KEY_STATE_UP = 0x0001 };

Bool flag(Int bit) { return (g_flags >> bit) & 1; }

/* ---- the singletons ---- */
class GameWindowManager
{
public:
	WinInputReturnCode winProcessMouseEvent(GameWindowMessage msg, ICoord2D *mousePos, void *data)
	{
		Int kind = 0, x = 0, y = 0;
		if (data && (msg == GWM_LEFT_DRAG || msg == GWM_MIDDLE_DRAG || msg == GWM_RIGHT_DRAG)) { kind = 1; x = ((ICoord2D *)data)->x; y = ((ICoord2D *)data)->y; }
		else if (data && (msg == GWM_WHEEL_UP || msg == GWM_WHEEL_DOWN)) { kind = 2; x = *(Int *)data; }
		else if (data) { kind = 3; }
		Int w[6] = { (Int)msg, mousePos->x, mousePos->y, kind, x, y };
		g_out.insert(g_out.end(), w, w + 6);
		return flag(5) ? WIN_INPUT_USED : WIN_INPUT_NOT_USED;
	}
	WinInputReturnCode winProcessKey(UnsignedByte key, WindowMsgData state)
	{
		Int w[6] = { 100, (Int)key, (Int)state, 0, 0, 0 };
		g_out.insert(g_out.end(), w, w + 6);
		return flag(6) ? WIN_INPUT_USED : WIN_INPUT_NOT_USED;
	}
};
GameWindowManager g_wm;
GameWindowManager *TheWindowManager = &g_wm;

class Shell { public: Bool isShellActive() { return flag(4); } };
Shell g_shell;
Shell *TheShell = &g_shell;

class View
{
public:
	Int m_unlocks;
	Bool isMouseLocked() { return flag(0); }
	void setMouseLock(Bool lock) { if (!lock) ++m_unlocks; }
};
View g_view;
View *TheTacticalView = &g_view;

struct MouseIO { ICoord2D pos; };
class Mouse { public: MouseIO m_status; const MouseIO *getMouseStatus() { m_status.pos = g_mouseStatus; return &m_status; } };
Mouse g_mouse;
Mouse *TheMouse = &g_mouse;

class Display { public: Bool isMoviePlaying() { return flag(7); } void stopMovie() { ++g_stopMovie; } };
Display g_display;
Display *TheDisplay = &g_display;

struct GlobalData { Bool m_allowExitOutOfMovies; };
GlobalData g_global;
GlobalData *TheGlobalData = &g_global;

class Drawable;
class InGameUI
{
public:
	Bool m_inputEnabled, m_isSelecting, m_forceAttack, m_forceMove, m_waypoint, m_prefer;
	Int m_endHints, m_camClears[4];
	Bool getInputEnabled() { return m_inputEnabled; }
	Bool isScrolling() { return flag(1); }
	Bool isPlacementAnchored() { return flag(3); }
	void setSelecting(Bool b) { m_isSelecting = b; }
	void endAreaSelectHint(const void *) { ++m_endHints; }
	void setForceAttackMode(Bool b) { m_forceAttack = b; }
	void setForceMoveMode(Bool b) { m_forceMove = b; }
	void setWaypointMode(Bool b) { m_waypoint = b; }
	void setPreferSelectionMode(Bool b) { m_prefer = b; }
	void setCameraRotateLeft(Bool b) { if (!b) ++m_camClears[0]; }
	void setCameraRotateRight(Bool b) { if (!b) ++m_camClears[1]; }
	void setCameraZoomIn(Bool b) { if (!b) ++m_camClears[2]; }
	void setCameraZoomOut(Bool b) { if (!b) ++m_camClears[3]; }
	void setInputEnabled(Bool enable);
};
InGameUI g_ui;
InGameUI *TheInGameUI = &g_ui;

/* ---- the translators, as their headers declare them (the members the cuts read) ---- */
class WindowTranslator : public GameMessageTranslator
{
public:
	WindowTranslator();
	~WindowTranslator();
	virtual GameMessageDisposition translateGameMessage(const GameMessage *msg);
};
class CommandTranslator : public GameMessageTranslator
{
public:
	virtual GameMessageDisposition translateGameMessage(const GameMessage *msg);
};
class SelectionTranslator : public GameMessageTranslator
{
public:
	Bool m_dragSelecting;
	virtual GameMessageDisposition translateGameMessage(const GameMessage *msg);
};
static Bool isSystemMessage( const GameMessage *msg );

#include "window_xlat.inc"
#include "command_gate.inc"
#include "system_message.inc"
#include "selection_gate.inc"
#include "set_input_enabled.inc"

} // namespace igt

using namespace igt;

static Bool isMouseType(Int t)
{
	return t >= (Int)GameMessage::MSG_RAW_MOUSE_BEGIN && t <= (Int)GameMessage::MSG_RAW_MOUSE_END;
}

extern "C" int ref_input_gate_case(int kind, const int *in, int nin, int *out, int cap)
{
	(void)nin;
	g_out.clear();
	if (kind == 0) {
		g_flags = in[5];
		g_global.m_allowExitOutOfMovies = flag(8);
		g_ui.m_inputEnabled = flag(2);
		g_mouseStatus.x = in[6];
		g_mouseStatus.y = in[7];
		g_stopMovie = 0;
		GameMessage *msg = newInstance(GameMessage)((GameMessage::Type)in[0]);
		ICoord2D pos; pos.x = in[1]; pos.y = in[2];
		ICoord2D delta; delta.x = in[3]; delta.y = in[4];
		Int t = in[0];
		if (isMouseType(t)) msg->appendPixelArgument(pos); else msg->appendIntegerArgument(in[1]);
		if (t == GameMessage::MSG_RAW_MOUSE_LEFT_DRAG || t == GameMessage::MSG_RAW_MOUSE_MIDDLE_DRAG || t == GameMessage::MSG_RAW_MOUSE_RIGHT_DRAG)
			msg->appendPixelArgument(delta);
		else
			msg->appendIntegerArgument(in[3]);
		WindowTranslator x;
		Int disp = (Int)x.translateGameMessage(msg);
		msg->deleteInstance();
		g_out.insert(g_out.begin(), g_stopMovie);
		g_out.insert(g_out.begin(), disp);
	} else if (kind == 1) {
		g_ui.m_inputEnabled = in[1] != 0;
		GameMessage *msg = newInstance(GameMessage)((GameMessage::Type)in[0]);
		CommandTranslator x;
		g_out.push_back((Int)x.translateGameMessage(msg));
		msg->deleteInstance();
	} else if (kind == 2) {
		g_ui.m_inputEnabled = in[1] != 0;
		g_ui.m_isSelecting = in[3] != 0;
		g_ui.m_endHints = 0;
		g_view.m_unlocks = 0;
		GameMessage *msg = newInstance(GameMessage)((GameMessage::Type)in[0]);
		SelectionTranslator x;
		x.m_dragSelecting = in[2] != 0;
		Int disp = (Int)x.translateGameMessage(msg);
		msg->deleteInstance();
		Int w[5] = { disp, (Int)x.m_dragSelecting, (Int)g_ui.m_isSelecting, g_ui.m_endHints, g_view.m_unlocks };
		g_out.insert(g_out.end(), w, w + 5);
	} else if (kind == 3) {
		g_ui.m_inputEnabled = in[1] != 0;
		g_ui.m_isSelecting = in[2] != 0;
		g_ui.m_forceAttack = in[3] != 0;
		g_ui.m_forceMove = in[4] != 0;
		g_ui.m_waypoint = in[5] != 0;
		g_ui.m_prefer = in[6] != 0;
		memset(g_ui.m_camClears, 0, sizeof g_ui.m_camClears);
		g_ui.setInputEnabled(in[0] != 0);
		Int w[10] = { (Int)g_ui.m_inputEnabled, (Int)g_ui.m_isSelecting, (Int)g_ui.m_forceAttack, (Int)g_ui.m_forceMove, (Int)g_ui.m_waypoint, (Int)g_ui.m_prefer,
			g_ui.m_camClears[0], g_ui.m_camClears[1], g_ui.m_camClears[2], g_ui.m_camClears[3] };
		g_out.insert(g_out.end(), w, w + 10);
	}
	int n = (int)g_out.size();
	for (int i = 0; i < n && i < cap; ++i) out[i] = g_out[i];
	return n;
}
