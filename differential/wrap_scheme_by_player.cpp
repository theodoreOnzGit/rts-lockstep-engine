/* Differential-test wrapper for ControlBarSchemeManager::setControlBarSchemeByPlayer (GitHub #146, upstream commit
** 0a05454d), cut verbatim by build.rs into OUT_DIR/scheme_by_player_gen:
**   sbp_find.inc     GameEngine/Source/GameClient/GUI/ControlBar/ControlBarScheme.cpp,
**                    ControlBarSchemeManager::findControlBarScheme (to the "Preload assets" comment);
**   sbp_player.inc   ControlBarScheme.cpp, ControlBarSchemeManager::setControlBarSchemeByPlayer (to the banner of the
**                    private functions).
** No edit. The stand-ins: ControlBarScheme (m_name, m_side, m_ScreenCreationRes; init() records its index),
** ControlBarSchemeManager (m_schemeList, m_currentScheme, m_multiplyer), Player (getSide), TheRecorder (isMultiplayer),
** TheWindowManager (winGetWindowFromId, by name: NAMEKEY is the name itself here), TheControlBar (findCommandButton by
** name; setControlCommand records the button), TheDisplay (getWidth, getHeight). AsciiString is the shim's
** (strcmp/strcasecmp, tolower).
**
** Entry point (extern "C", tests/differential_scheme_by_player.rs):
**   ref_scheme_by_player_case(in, nin, out, cap): `in` is [isMultiplayer, the layout has PopupCommunicator,
**   NonCommand_Communicator found, NonCommand_BriefingHistory found, display width, height, a player (0: NULL), the
**   player's side (a string index), the current scheme (-1: NULL), n, then n schemes of 4 words [name, side (string
**   indices), m_ScreenCreationRes x, y]]. Out: [10, the name looked up (0 Communicator, 1 BriefingHistory)] per
**   findCommandButton, [11, the button given (0, 1)] per setControlCommand with a button, [30, index] per init(), then [20, the
**   current scheme (-1 NULL), m_multiplyer x bits, y bits]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <list>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "Common/AsciiString.h"

namespace sbp {

std::vector<int> g_out;
static Int bitsOf(Real r) { Int i; memcpy(&i, &r, sizeof i); return i; }

/* the strings the cases use for names and sides (tests/differential_scheme_by_player.rs has the same list) */
static const char *const STRINGS[] = { "", "America", "america", "AMERICA", "China", "GLA", "Observer", "observer",
	"AmericaSmall", "Default", "default", "America8x6", "America16x12", "ChinaSuper", "Civilian" };
static const Int NSTRINGS = (Int)(sizeof STRINGS / sizeof STRINGS[0]);

#define NAMEKEY(x) (x)

class GameWindow {};
GameWindow g_communicator;
struct WindowManagerStub
{
	Bool m_has;
	GameWindow *winGetWindowFromId( GameWindow *, const char *name ) { return (m_has && strcmp(name, "ControlBar.wnd:PopupCommunicator") == 0) ? &g_communicator : NULL; }
};
WindowManagerStub g_wm;
WindowManagerStub *TheWindowManager = &g_wm;

class CommandButton { public: Int m_id; };
CommandButton g_buttons[2] = { { 0 }, { 1 } };
struct ControlBarStub
{
	Bool m_found[2];
	const CommandButton *findCommandButton( const AsciiString &name )
	{
		Int id = (strcmp(name.str(), "NonCommand_Communicator") == 0) ? 0 : (strcmp(name.str(), "NonCommand_BriefingHistory") == 0) ? 1 : 99;
		g_out.push_back(10);
		g_out.push_back(id);
		return (id < 2 && m_found[id]) ? &g_buttons[id] : NULL;
	}
	/* (ControlBar::setControlCommand does nothing for a NULL button, ControlBar.cpp:2427; it is differential-tested in
	** differential_bar_tooltips; here only the button it is given is recorded) */
	void setControlCommand( GameWindow *, const CommandButton *b )
	{
		if (!b)
			return;
		g_out.push_back(11);
		g_out.push_back(b->m_id);
	}
};
ControlBarStub g_bar;
ControlBarStub *TheControlBar = &g_bar;

struct RecorderStub
{
	Bool m_mp;
	Bool isMultiplayer() { return m_mp; }
};
RecorderStub g_recorder;
RecorderStub *TheRecorder = &g_recorder;

struct DisplayStub
{
	UnsignedInt m_width, m_height;
	UnsignedInt getWidth() { return m_width; }
	UnsignedInt getHeight() { return m_height; }
};
DisplayStub g_display;
DisplayStub *TheDisplay = &g_display;

class Player
{
public:
	AsciiString m_side;
	AsciiString getSide() { return m_side; }
};

class ControlBarScheme
{
public:
	Int m_index;
	AsciiString m_name;
	AsciiString m_side;
	ICoord2D m_ScreenCreationRes;
	void init() { g_out.push_back(30); g_out.push_back(m_index); }
};
typedef std::list< ControlBarScheme * > ControlBarSchemeList;

class ControlBarSchemeManager
{
public:
	ControlBarSchemeList m_schemeList;
	ControlBarScheme *m_currentScheme;
	Coord2D m_multiplyer;
	ControlBarScheme *findControlBarScheme( AsciiString name );
	void setControlBarSchemeByPlayer(Player *p);
};
#include "sbp_find.inc"
#include "sbp_player.inc"

} // namespace sbp

extern "C" int ref_scheme_by_player_case(const int *in, int nin, int *out, int cap)
{
	using namespace sbp;
	g_out.clear();
	g_recorder.m_mp = in[0] != 0;
	g_wm.m_has = in[1] != 0;
	g_bar.m_found[0] = in[2] != 0;
	g_bar.m_found[1] = in[3] != 0;
	g_display.m_width = (UnsignedInt)in[4];
	g_display.m_height = (UnsignedInt)in[5];
	Player player;
	player.m_side.set(STRINGS[in[7] % NSTRINGS]);
	Int n = in[9];
	std::vector<ControlBarScheme> schemes(n);
	ControlBarSchemeManager m;
	for (Int i = 0; i < n && 10 + i * 4 + 3 < nin; ++i) {
		ControlBarScheme &s = schemes[i];
		s.m_index = i;
		/* newControlBarScheme lowercases the name */
		s.m_name.set(STRINGS[in[10 + i * 4] % NSTRINGS]);
		s.m_name.toLower();
		s.m_side.set(STRINGS[in[11 + i * 4] % NSTRINGS]);
		s.m_ScreenCreationRes.x = in[12 + i * 4];
		s.m_ScreenCreationRes.y = in[13 + i * 4];
		m.m_schemeList.push_back(&s);
	}
	m.m_currentScheme = (in[8] >= 0 && in[8] < n) ? &schemes[in[8]] : NULL;
	m.m_multiplyer.x = 1.0f;
	m.m_multiplyer.y = 1.0f;
	m.setControlBarSchemeByPlayer(in[6] ? &player : NULL);
	Int cur = m.m_currentScheme ? m.m_currentScheme->m_index : -1;
	g_out.push_back(20);
	g_out.push_back(cur);
	g_out.push_back(bitsOf(m.m_multiplyer.x));
	g_out.push_back(bitsOf(m.m_multiplyer.y));
	int nout = (int)g_out.size();
	if (nout > cap)
		nout = cap;
	if (nout > 0)
		memcpy(out, &g_out[0], nout * sizeof(int));
	return nout;
}
