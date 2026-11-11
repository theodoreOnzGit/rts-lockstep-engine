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
**   set_input_enabled.inc  InGameUI.cpp, InGameUI::setInputEnabled;
**   command_options.inc ControlBar.h, `enum CommandOption` to `#ifdef DEFINE_GUI_COMMMAND_NAMES` (GUICommandType);
**   command_mouse_cases.inc  CommandXlat.cpp, translateGameMessage's cases from `case GameMessage::MSG_MOUSEOVER_DRAWABLE_HINT:`
**                       to `#ifdef ALLOW_ALT_F4` (the mouse-over hints, the right button's raw messages, the right and
**                       left double clicks and clicks; #123, #124), compiled as the body of a switch in
**                       CommandTranslator::mouseCases, which the wrapper writes (`switch (t) { <cut> default: break; }`).
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
**             in/out: 1 when set to FALSE).
**     kind 4: (#123, #124) the command translator's gate then its mouse cases. in: type, input enabled, m_useAlternateMouse,
**             m_doubleClickAttackMove, getSelectCount(), areSelectedObjectsControllable() (with something selected), the
**             pending GUI command's type (-1: none), force attack mode, attack-move mode, the pixel region (lo.x, lo.y,
**             hi.x, hi.y), the drawable pickDrawable answers (0: none), the mouse-over drawable, the mouse-over location's
**             x, y, z (float bits), TheMouse->isClick's answer. out: disposition; per evaluateForceAttack (1) or
**             evaluateContextCommand (2) call: (kind, drawable id or 0, x, y, z float bits, CommandEvaluateType); -1; per
**             appended message: (type, argument count, then per argument its data type and value: an integer's value, a
**             location's three float bits); -2; academy records; the guard hint's timer; attack-move mode after. The
**             drawable n stands at (10n + 0.5, 3n - 2, n / 4); screenToTerrain(p) is (p.x / 2 + 3, p.y / 4 - 7, 1.5). */
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

/* (#123, #124) the drawables 1..9 and the tactical view's answers for the mouse cases */
class Drawable
{
public:
	Int m_id;
	Coord3D m_pos;
	const Coord3D *getPosition() const { return &m_pos; }
};
Drawable g_draws[10];
Int g_picked;
enum PickType { PICK_TYPE_SELECTABLE = 1 };
static UnsignedInt getPickTypesForContext(Bool) { return PICK_TYPE_SELECTABLE; }
class GameClient { public: Drawable *findDrawableByID(DrawableID id) { return (id >= 1 && id <= 9) ? &g_draws[id] : NULL; } };
GameClient g_client;
GameClient *TheGameClient = &g_client;

class View
{
public:
	Int m_unlocks;
	Bool isMouseLocked() { return flag(0); }
	void setMouseLock(Bool lock) { if (!lock) ++m_unlocks; }
	void screenToTerrain(const ICoord2D *p, Coord3D *pos) { pos->x = (Real)p->x * 0.5f + 3.0f; pos->y = (Real)p->y * 0.25f - 7.0f; pos->z = 1.5f; }
	Drawable *pickDrawable(const ICoord2D *, Bool, PickType) { return g_picked ? &g_draws[g_picked] : NULL; }
};
View g_view;
View *TheTacticalView = &g_view;

struct MouseIO { ICoord2D pos; };
Bool g_isClick;
class Mouse
{
public:
	MouseIO m_status;
	const MouseIO *getMouseStatus() { m_status.pos = g_mouseStatus; return &m_status; }
	Bool isClick(const ICoord2D *, const ICoord2D *, UnsignedInt, UnsignedInt) { return g_isClick; }
};
Mouse g_mouse;
Mouse *TheMouse = &g_mouse;

class Display { public: Bool isMoviePlaying() { return flag(7); } void stopMovie() { ++g_stopMovie; } };
Display g_display;
Display *TheDisplay = &g_display;

struct GlobalData { Bool m_allowExitOutOfMovies; Bool m_useAlternateMouse; Bool m_doubleClickAttackMove; };
GlobalData g_global;
GlobalData *TheGlobalData = &g_global;

#include "command_options.inc"
enum GuardMode { GUARDMODE_NORMAL, GUARDMODE_GUARD_WITHOUT_PURSUIT, GUARDMODE_GUARD_FLYING_UNITS_ONLY };
class CommandButton
{
public:
	GUICommandType m_type;
	GUICommandType getCommandType() const { return m_type; }
};
CommandButton g_command;

class AcademyStats { public: Int m_records; void recordDoubleClickAttackMoveOrderGiven() { ++m_records; } };
class Player { public: AcademyStats m_stats; AcademyStats *getAcademyStats() { return &m_stats; } };
class PlayerList { public: Player m_local; Player *getLocalPlayer() { return &m_local; } };
PlayerList g_players;
PlayerList *ThePlayerList = &g_players;

/* the stream the cases append to: the real GameMessage, kept for the dump */
class StreamStandIn
{
public:
	std::vector<GameMessage *> m_msgs;
	GameMessage *appendMessage(GameMessage::Type t) { GameMessage *m = newInstance(GameMessage)(t); m_msgs.push_back(m); return m; }
};
StreamStandIn g_stream;
StreamStandIn *TheMessageStream = &g_stream;

class InGameUI
{
public:
	Bool m_inputEnabled, m_isSelecting, m_forceAttack, m_forceMove, m_waypoint, m_prefer;
	Int m_endHints, m_camClears[4];
	/* (#123, #124) */
	Int m_selectCount, m_guardHintTimer;
	Bool m_controllable, m_attackMove;
	const CommandButton *m_command;
	const CommandButton *getGUICommand() const { return m_command; }
	Int getSelectCount() { return m_selectCount; }
	Bool isInForceAttackMode() { return m_forceAttack; }
	Bool areSelectedObjectsControllable() { return m_controllable; }
	void clearAttackMoveToMode() { m_attackMove = FALSE; }
	void placeBuildAvailable(const void *, Drawable *) {}
	void triggerDoubleClickAttackMoveGuardHint() { m_guardHintTimer = 11; }
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
std::vector<int> g_evals;
static Int realBits(Real r) { Int i; memcpy(&i, &r, sizeof i); return i; }
class CommandTranslator : public GameMessageTranslator
{
public:
	enum CommandEvaluateType { DO_COMMAND, DO_HINT, EVALUATE_ONLY };
	ICoord2D m_mouseRightDragAnchor;
	ICoord2D m_mouseRightDragLift;
	UnsignedInt m_mouseRightDown;
	UnsignedInt m_mouseRightUp;
	GameMessage::Type log(Int kind, Drawable *draw, const Coord3D *pos, CommandEvaluateType type)
	{
		Int w[6] = { kind, draw ? draw->m_id : 0, realBits(pos->x), realBits(pos->y), realBits(pos->z), (Int)type };
		g_evals.insert(g_evals.end(), w, w + 6);
		return GameMessage::MSG_INVALID;
	}
	/* stand-ins: the call is the case's answer */
	GameMessage::Type evaluateForceAttack( Drawable *draw, const Coord3D *pos, CommandEvaluateType type ) { return log(1, draw, pos, type); }
	GameMessage::Type evaluateContextCommand( Drawable *draw, const Coord3D *pos, CommandEvaluateType type ) { return log(2, draw, pos, type); }
	GameMessageDisposition mouseCases(const GameMessage *msg);
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

GameMessageDisposition CommandTranslator::mouseCases(const GameMessage *msg)
{
	GameMessage::Type t = msg->getType();
	GameMessageDisposition disp = KEEP_MESSAGE;
	switch (t)
	{
#include "command_mouse_cases.inc"
		default:
			break;
	}
	return disp;
}

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
	else if (kind == 4) {
		Int t = in[0];
		g_ui.m_inputEnabled = in[1] != 0;
		g_global.m_useAlternateMouse = in[2] != 0;
		g_global.m_doubleClickAttackMove = in[3] != 0;
		g_ui.m_selectCount = in[4];
		g_ui.m_controllable = in[5] != 0;
		g_command.m_type = (GUICommandType)in[6];
		g_ui.m_command = in[6] >= 0 ? &g_command : NULL;
		g_ui.m_forceAttack = in[7] != 0;
		g_ui.m_attackMove = in[8] != 0;
		g_ui.m_guardHintTimer = 0;
		g_players.m_local.m_stats.m_records = 0;
		g_picked = in[13];
		g_isClick = in[18] != 0;
		for (Int i = 1; i <= 9; ++i) {
			g_draws[i].m_id = i;
			g_draws[i].m_pos.x = (Real)i * 10.0f + 0.5f;
			g_draws[i].m_pos.y = (Real)i * 3.0f - 2.0f;
			g_draws[i].m_pos.z = (Real)i * 0.25f;
		}
		g_evals.clear();
		g_stream.m_msgs.clear();
		GameMessage *msg = newInstance(GameMessage)((GameMessage::Type)t);
		if (t == GameMessage::MSG_MOUSEOVER_DRAWABLE_HINT) {
			msg->appendDrawableIDArgument((DrawableID)in[14]);
		} else if (t == GameMessage::MSG_MOUSEOVER_LOCATION_HINT) {
			Coord3D c;
			memcpy(&c.x, &in[15], 4); memcpy(&c.y, &in[16], 4); memcpy(&c.z, &in[17], 4);
			msg->appendLocationArgument(c);
		} else {
			IRegion2D r;
			r.lo.x = in[9]; r.lo.y = in[10]; r.hi.x = in[11]; r.hi.y = in[12];
			msg->appendPixelRegionArgument(r);
			msg->appendIntegerArgument(0);
		}
		CommandTranslator x;
		Int disp = (Int)x.translateGameMessage(msg);
		if (disp == (Int)KEEP_MESSAGE)
			disp = (Int)x.mouseCases(msg);
		msg->deleteInstance();
		g_out.push_back(disp);
		g_out.insert(g_out.end(), g_evals.begin(), g_evals.end());
		g_out.push_back(-1);
		for (size_t i = 0; i < g_stream.m_msgs.size(); ++i) {
			GameMessage *m = g_stream.m_msgs[i];
			Int n = m->getArgumentCount();
			g_out.push_back((Int)m->getType());
			g_out.push_back(n);
			for (Int a = 0; a < n; ++a) {
				Int dt = (Int)m->getArgumentDataType(a);
				g_out.push_back(dt);
				if (dt == ARGUMENTDATATYPE_LOCATION) {
					const Coord3D &c = m->getArgument(a)->location;
					g_out.push_back(realBits(c.x)); g_out.push_back(realBits(c.y)); g_out.push_back(realBits(c.z));
				} else {
					g_out.push_back(m->getArgument(a)->integer);
				}
			}
			m->deleteInstance();
		}
		g_out.push_back(-2);
		g_out.push_back(g_players.m_local.m_stats.m_records);
		g_out.push_back(g_ui.m_guardHintTimer);
		g_out.push_back((Int)g_ui.m_attackMove);
	}
	int n = (int)g_out.size();
	for (int i = 0; i < n && i < cap; ++i) out[i] = g_out[i];
	return n;
}
