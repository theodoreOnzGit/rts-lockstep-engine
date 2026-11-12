/* Differential-test wrapper for ControlBar::init's tooltip callbacks and the money, power and general's experience
** popups (GitHub #144, upstream commit 0a05454d), cut verbatim by build.rs into OUT_DIR/bar_tooltips_gen:
**   bti_init.inc     GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp, ControlBar::init from "don't forget about
**                    the communicator button CCB" through the GeneralsExp window's winSetTooltipFunc (to the
**                    m_radarAttackGlowWindow line);
**   bti_set.inc      the same file, ControlBar::setCommandBarBorder and ControlBar::setControlCommand(GameWindow *, const
**                    CommandButton *) (to CommandButton::cacheButtonImage);
**   bti_pd_head.inc  GameEngine/Source/GameClient/GUI/GUICallbacks/ControlBarPopupDescription.cpp,
**                    populateBuildTooltipLayout's head (from `if(!m_buildToolTipLayout)` to `if(commandButton)`);
**   bti_pd_win.inc   the same function's `else if(tooltipWin)` branch (to the StaticTextName lookup).
** No edit. The wrapper writes `if(commandButton) { }` between the two popup cuts (the command button's branch is #35's
** and #138's, not this test's: every call here passes NULL), and records name, cost and description after the branch:
** the branch's `return` (an unknown window) records nothing.
** The stand-ins: ten windows (the six non-command buttons, the power, money and general's experience windows, and one
** other), each present or not, with GadgetPushButtonInput or another input function; TheNameKeyGenerator (a name's key is
** its index in the table below); TheWindowManager->winGetWindowFromId (the bar's windows, from any start: see
** client::control_bar_popup::tooltip_window_of for why the popup layout's first window reaches them); six command
** buttons (findCommandButton: present or NULL; options, button image or NULL, text label, sciences, border type); the
** border colours; TheHotKeyManager (present or NULL; searchHotKey gives a label's key, empty for every third label);
** the local and the observer's players (each present or NULL, with an energy's production and consumption), the
** observer's control bar on or off; TheGameText (a label's text is the label, except CONTROLBAR:PowerDescription, one of
** five formats); UnicodeString (std::wstring, `format` through vswprintf as upstream's `_vsnwprintf`). The gadget calls,
** winSetTooltipFunc and addHotKey are recorded.
**
** Entry point (extern "C", tests/differential_bar_tooltips.rs):
**   ref_bar_tooltips_case(in, nin, out, cap) runs one case and writes its words to out.
**   `in`: 10 windows of [present, push button input]; 6 commands of [present, options, image (-1 NULL), label (-1 empty),
**   sciences, border type]; the 4 border colours (build, upgrade, action, system); TheHotKeyManager present; the
**   observer's bar on, the local player [present, production, consumption], the observer's [present, production,
**   consumption], the power format (0..4); then the windows populateBuildTooltipLayout(NULL, window) is called with (-1 NULL).
**   Records: [1, w] winSetTooltipFunc; [2, w, checkLike, checked]; [3, w, image]; [4, w] the text cleared; [5, w, command];
**   [6, w, color, draw]; [7, w, key] addHotKey; [8, w, the sound is GUICommandBarClick]; each 4 words. After a populate call
**   that reached the end: [21, costToBuild, name length, description length, name..., description...]; then after each
**   call [22, the call's index, 0, 0]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <wchar.h>
#include <string>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/Debug.h"

namespace btix {

std::vector<int> g_out;
void rec(Int a, Int b = 0, Int c = 0, Int d = 0)
{
	Int w[4] = { a, b, c, d };
	g_out.insert(g_out.end(), w, w + 4);
}

typedef Int NameKeyType;
typedef Int Color;
enum { GAME_COLOR_UNDEFINED = 0x00FFFFFF };
enum CommandOption { CHECK_LIKE = 0x00000400 };
enum CommandButtonMappedBorderType
{
	COMMAND_BUTTON_BORDER_NONE = 0,
	COMMAND_BUTTON_BORDER_BUILD,
	COMMAND_BUTTON_BORDER_UPGRADE,
	COMMAND_BUTTON_BORDER_ACTION,
	COMMAND_BUTTON_BORDER_SYSTEM,
};

/* the names the cuts look up, by key */
static const char *s_names[] = {
	"ControlBar.wnd:PopupCommunicator", "ControlBar.wnd:ButtonOptions", "ControlBar.wnd:ButtonIdleWorker",
	"ControlBar.wnd:ButtonPlaceBeacon", "ControlBar.wnd:ButtonGeneral", "ControlBar.wnd:ButtonLarge",
	"ControlBar.wnd:PowerWindow", "ControlBar.wnd:MoneyDisplay", "ControlBar.wnd:GeneralsExp", "ControlBar.wnd:Other",
};
static const char *s_commands[] = {
	"NonCommand_Communicator", "NonCommand_Options", "NonCommand_IdleWorker", "NonCommand_Beacon",
	"NonCommand_GeneralsExperience", "NonCommand_UpDown",
};
struct NameKeyGenerator
{
	NameKeyType nameToKey( const char *name )
	{
		for (Int i = 0; i < 10; ++i)
			if (!strcmp(s_names[i], name))
				return i;
		return 99;
	}
};
NameKeyGenerator g_keys;
NameKeyGenerator *TheNameKeyGenerator = &g_keys;

class AsciiString
{
public:
	Int m_label; /* -1: empty */
	AsciiString() : m_label(-1) {}
	Bool isEmpty() const { return m_label < 0; }
	Bool isNotEmpty() const { return m_label >= 0; }
};

class UnicodeString
{
public:
	std::wstring m_s;
	static UnicodeString TheEmptyString;
	UnicodeString() {}
	UnicodeString( const wchar_t *s ) : m_s(s) {}
	const wchar_t *str() const { return m_s.c_str(); }
	Bool isEmpty() const { return m_s.empty(); }
	void format_va( const wchar_t *f, va_list args )
	{
		wchar_t buf[2048];
		if (vswprintf(buf, sizeof(buf) / sizeof(wchar_t) - 1, f, args) < 0)
			abort();
		m_s = buf;
	}
	void format( UnicodeString f, ... ) { va_list a; va_start(a, f); format_va(f.str(), a); va_end(a); }
	void format( const wchar_t *f, ... ) { va_list a; va_start(a, f); format_va(f, a); va_end(a); }
};
UnicodeString UnicodeString::TheEmptyString;

static const wchar_t *s_powerFormats[] = { L"Power %d/%d", L"%d", L"no numbers", L"100%% %d used of %d", L"%d\n%d" };
Int g_powerFormat;
struct GameText
{
	UnicodeString fetch( const char *label )
	{
		if (!strcmp(label, "CONTROLBAR:PowerDescription"))
			return UnicodeString(s_powerFormats[g_powerFormat]);
		UnicodeString u;
		for (const char *c = label; *c; ++c)
			u.m_s.push_back((wchar_t)*c);
		return u;
	}
};
GameText g_text;
GameText *TheGameText = &g_text;

class GameWindow;
typedef Int (*GameWinInputFunc)( GameWindow *, UnsignedInt, UnsignedInt, UnsignedInt );
typedef void (*GameWinTooltipFunc)( GameWindow *, void *, UnsignedInt );
Int GadgetPushButtonInput( GameWindow *, UnsignedInt, UnsignedInt, UnsignedInt ) { return 0; }
Int OtherInput( GameWindow *, UnsignedInt, UnsignedInt, UnsignedInt ) { return 0; }

class GameWindow
{
public:
	Int m_id;
	GameWinInputFunc m_input;
	GameWinInputFunc winGetInputFunc( void ) { return m_input; }
	void winSetTooltipFunc( GameWinTooltipFunc ) { rec(1, m_id); }
};
GameWindow g_win[10];
Bool g_present[10];

struct WindowManager
{
	GameWindow *winGetWindowFromId( GameWindow *, Int id )
	{
		if (id >= 0 && id < 10 && g_present[id])
			return &g_win[id];
		return NULL;
	}
};
WindowManager g_wm;
WindowManager *TheWindowManager = &g_wm;

class WindowLayout
{
public:
	GameWindow *getFirstWindow( void ) { return NULL; }
};
WindowLayout g_layout;

class Image { public: Int m_id; };
Image g_images[64];

class CommandButton
{
public:
	Int m_id;
	UnsignedInt m_options;
	const Image *m_image;
	AsciiString m_label;
	std::vector<Int> m_science;
	CommandButtonMappedBorderType m_border;
	UnsignedInt getOptions() const { return m_options; }
	const Image *getButtonImage() const { return m_image; }
	const AsciiString &getTextLabel() const { return m_label; }
	const std::vector<Int> &getScienceVec() const { return m_science; }
	CommandButtonMappedBorderType getCommandButtonMappedBorderType() const { return m_border; }
};
CommandButton g_commands[6];
Bool g_commandPresent[6];

void GadgetButtonEnableCheckLike( GameWindow *g, Bool makeCheckLike, Bool initiallyChecked ) { rec(2, g->m_id, makeCheckLike ? 1 : 0, initiallyChecked ? 1 : 0); }
void GadgetButtonSetEnabledImage( GameWindow *g, const Image *image ) { rec(3, g->m_id, image->m_id); }
void GadgetButtonSetText( GameWindow *g, UnicodeString text ) { rec(4, g->m_id, (Int)text.m_s.size()); }
void GadgetButtonSetData( GameWindow *g, void *data ) { rec(5, g->m_id, ((CommandButton *)data)->m_id); }
void GadgetButtonSetBorder( GameWindow *g, Color color, Bool drawBorder = TRUE ) { rec(6, g->m_id, color, drawBorder ? 1 : 0); }
void GadgetButtonSetAltSound( GameWindow *g, const char *sound ) { rec(8, g->m_id, !strcmp(sound, "GUICommandBarClick") ? 1 : 0); }

class HotKeyManager
{
public:
	AsciiString searchHotKey( const AsciiString &label )
	{
		AsciiString k;
		if (label.m_label >= 0 && label.m_label % 3 != 0)
			k.m_label = 1000 + label.m_label;
		return k;
	}
	void addHotKey( GameWindow *win, const AsciiString &key ) { rec(7, win->m_id, key.m_label); }
};
HotKeyManager g_hotKeys;
HotKeyManager *TheHotKeyManager = &g_hotKeys;

class Energy
{
public:
	Int m_production, m_consumption;
	Int getProduction() const { return m_production; }
	Int getConsumption() const { return m_consumption; }
};
class Player
{
public:
	Energy m_energy;
	Energy *getEnergy() { return &m_energy; }
};
Player g_local, g_observer;
Bool g_localPresent, g_observerPresent;
struct PlayerList { Player *getLocalPlayer() { return g_localPresent ? &g_local : NULL; } };
PlayerList g_players;
PlayerList *ThePlayerList = &g_players;

class ProductionPrerequisite;

static void commandButtonTooltip( GameWindow *window, void *instData, UnsignedInt mouse ) {}

class ControlBar
{
public:
	GameWindow *m_communicatorButton;
	WindowLayout *m_buildToolTipLayout;
	Color m_commandButtonBorderBuildColor, m_commandButtonBorderActionColor, m_commandButtonBorderUpgradeColor, m_commandButtonBorderSystemColor;
	Bool m_observerOn;
	const CommandButton *findCommandButton( const char *name )
	{
		for (Int i = 0; i < 6; ++i)
			if (!strcmp(s_commands[i], name))
				return g_commandPresent[i] ? &g_commands[i] : NULL;
		return NULL;
	}
	Bool isObserverControlBarOn( void ) { return m_observerOn; }
	Player *getObserverLookAtPlayer( void ) { return g_observerPresent ? &g_observer : NULL; }
	void setCommandBarBorder( GameWindow *button, CommandButtonMappedBorderType type );
	void setControlCommand( GameWindow *button, const CommandButton *commandButton );
	void populateBuildTooltipLayout( const CommandButton *commandButton, GameWindow *tooltipWin = NULL );
	void initTooltips( void );
};
ControlBar g_bar;
ControlBar *TheControlBar = &g_bar;

#include "bti_set.inc"

void ControlBar::initTooltips( void )
{
	NameKeyType id;
#include "bti_init.inc"
}

void ControlBar::populateBuildTooltipLayout( const CommandButton *commandButton, GameWindow *tooltipWin)
{
#include "bti_pd_head.inc"
	if(commandButton)
	{
	}
#include "bti_pd_win.inc"
	rec(21, (Int)costToBuild, (Int)name.m_s.size(), (Int)descrip.m_s.size());
	for (size_t i = 0; i < name.m_s.size(); ++i)
		g_out.push_back((int)name.m_s[i]);
	for (size_t i = 0; i < descrip.m_s.size(); ++i)
		g_out.push_back((int)descrip.m_s[i]);
}

} // namespace btix

extern "C" int ref_bar_tooltips_case(const int *in, int nin, int *out, int cap)
{
	using namespace btix;
	g_out.clear();
	const int *p = in;
	for (Int i = 0; i < 10; ++i, p += 2) {
		g_win[i].m_id = i;
		g_present[i] = p[0] != 0;
		g_win[i].m_input = p[1] ? GadgetPushButtonInput : OtherInput;
	}
	for (Int i = 0; i < 64; ++i)
		g_images[i].m_id = i;
	for (Int i = 0; i < 6; ++i, p += 6) {
		CommandButton &c = g_commands[i];
		c.m_id = i;
		g_commandPresent[i] = p[0] != 0;
		c.m_options = (UnsignedInt)p[1];
		c.m_image = p[2] < 0 ? NULL : &g_images[p[2] & 63];
		c.m_label.m_label = p[3];
		c.m_science.clear();
		if (p[4])
			c.m_science.push_back(p[4]);
		c.m_border = (CommandButtonMappedBorderType)p[5];
	}
	g_bar.m_commandButtonBorderBuildColor = p[0];
	g_bar.m_commandButtonBorderUpgradeColor = p[1];
	g_bar.m_commandButtonBorderActionColor = p[2];
	g_bar.m_commandButtonBorderSystemColor = p[3];
	p += 4;
	TheHotKeyManager = p[0] ? &g_hotKeys : NULL;
	g_bar.m_observerOn = p[1] != 0;
	g_localPresent = p[2] != 0;
	g_local.m_energy.m_production = p[3];
	g_local.m_energy.m_consumption = p[4];
	g_observerPresent = p[5] != 0;
	g_observer.m_energy.m_production = p[6];
	g_observer.m_energy.m_consumption = p[7];
	g_powerFormat = p[8] % 5;
	p += 9;
	g_bar.m_communicatorButton = NULL;
	g_bar.m_buildToolTipLayout = &g_layout;
	g_bar.initTooltips();
	Int k = 0;
	for (; p < in + nin; ++p, ++k) {
		Int w = *p;
		g_bar.populateBuildTooltipLayout(NULL, (w >= 0 && w < 10) ? &g_win[w] : NULL);
		rec(22, k);
	}
	int n = (int)g_out.size();
	if (n > cap)
		n = cap;
	if (n > 0)
		memcpy(out, &g_out[0], n * sizeof(int));
	return n;
}
