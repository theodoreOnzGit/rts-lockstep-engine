/* Differential-test wrapper for the control bar's scheme setters and colours (GitHub #145, upstream commit 0a05454d),
** cut verbatim by build.rs into OUT_DIR/scheme_bar_gen:
**   scb_color_undef.inc  GameEngine/Include/GameClient/Color.h, the GAME_COLOR_UNDEFINED enum line;
**   scb_make_color.inc   Color.h, GameMakeColor (to GameGetColorComponents' declaration);
**   scb_ctor.inc         GameEngine/Source/GameClient/GUI/ControlBar/ControlBarScheme.cpp, ControlBarScheme::ControlBarScheme
**                        (to ControlBarScheme::init);
**   scb_init_colors.inc  the same file, ControlBarScheme::init's three colour lines (updateBuildUpClockColor,
**                        updateCommanBarBorderColors, updateBorderColor; to the commented-out updateCommandMarkerImage);
**   scb_find.inc         the same file, ControlBarSchemeManager::findControlBarScheme (to the "Preload assets" comment);
**   scb_template.inc     the same file, ControlBarSchemeManager::setControlBarSchemeByPlayerTemplate (to the divider before
**                        setControlBarSchemeByPlayer);
**   scb_player.inc       the same file, ControlBarSchemeManager::setControlBarSchemeByPlayer (to the private functions'
**                        banner);
**   scb_bar_h.inc        GameEngine/Include/GameClient/ControlBar.h, getBorderColor and updateBorderColor;
**   scb_bar_ctor_a.inc   GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp, ControlBar::ControlBar's two colour
**                        lines (m_buildUpClockColor, m_commandBarBorderColor);
**   scb_bar_ctor_b.inc   the same constructor's updateCommanBarBorderColors(GAME_COLOR_UNDEFINED, ...) line;
**   scb_bar_by.inc       the same file, ControlBar::setControlBarSchemeByPlayer and setControlBarSchemeByPlayerTemplate (to
**                        setControlBarSchemeByName);
**   scb_bar_update.inc   the same file, ControlBar::updateBuildUpClockColor and updateCommanBarBorderColors (to the
**                        "hides the communicator button" comment);
**   scb_bar_border.inc   the same file, ControlBar::setCommandBarBorder (to setControlCommand's banner).
** No edit. The wrapper puts the constructor lines into a stand-in ControlBar's constructor and init's colour lines into
** the stand-in scheme's init(), inside `if(TheControlBar)` as upstream has them.
** The stand-ins: ControlBarScheme (every member the constructor sets; init() records its index, then the colour lines);
** ControlBarSchemeManager (m_schemeList, m_currentScheme, m_multiplyer); ControlBar (the colour members,
** m_controlBarSchemeManager, m_isObserverCommandBar as a recorded Bool, switchToContext, switchControlBarStage,
** hidePurchaseScience, findCommandButton, setControlCommand: recorded); Player (getSide, isPlayerActive); PlayerTemplate
** (getSide); ThePlayerTemplateStore (findPlayerTemplate("FactionObserver"): the observer's template or NULL);
** TheNameKeyGenerator and NAMEKEY (a name's key is the name); TheWindowManager (the communicator, beacon, idle worker and
** general's windows, each present or not; winHide and winEnable recorded); TheGameLogic (getGameMode); TheGameInfo
** (isMultiPlayer, recorded when asked); TheRecorder (isMultiplayer); TheDisplay; GadgetButtonSetBorder (recorded).
** AsciiString is the shim's.
**
** Entry point (extern "C", tests/differential_scheme_bar.rs):
**   ref_scheme_bar_case(in, nin, out, cap). `in`: [kind (0 ControlBar::setControlBarSchemeByPlayer, 1
**   ControlBar::setControlBarSchemeByPlayerTemplate, 2 the manager's setControlBarSchemeByPlayerTemplate(pt, useSmall)),
**   isMultiplayer, the layout has PopupCommunicator, NonCommand_Communicator found, NonCommand_BriefingHistory found,
**   display width, height, a player/template (0 NULL), its side (a string index), the current scheme (-1 NULL), the bar
**   has a scheme manager, the player is active, the game mode, TheGameInfo->isMultiPlayer(), the beacon, idle worker and
**   general's windows present, the store has FactionObserver, the template is FactionObserver's, useSmall, unused, n, then
**   n schemes of 11 words [name, side (string indices), m_ScreenCreationRes x, y, a mask of the colours set (bit i: colour
**   i), BuildUpClockColor, ButtonBorderBuildColor, CommandBarBorderColor, ButtonBorderActionColor, ButtonBorderUpgradeColor,
**   ButtonBorderSystemColor]].
**   Out, 4 words each: [10, the name looked up (0 Communicator, 1 BriefingHistory)]; [11, the button given]; [30, the
**   scheme init() ran on]; [40, context]; [41, m_isObserverCommandBar set to]; [42, window (0 beacon, 1 idle worker, 2
**   general), hide]; [43, window, enable]; [44] TheGameInfo asked; [45] switchControlBarStage(DEFAULT); [46]
**   hidePurchaseScience; then [20, current scheme (-1 NULL), m_multiplyer x bits, y bits], [21, m_buildUpClockColor,
**   getBorderColor()], [22, build, action, upgrade], [23, system], and for border types 0..5 setCommandBarBorder's
**   [6, type, color, draw]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <list>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "Common/AsciiString.h"

namespace scb {

std::vector<int> g_out;
static void rec(Int a, Int b = 0, Int c = 0, Int d = 0)
{
	Int w[4] = { a, b, c, d };
	g_out.insert(g_out.end(), w, w + 4);
}
static Int bitsOf(Real r) { Int i; memcpy(&i, &r, sizeof i); return i; }

/* the strings the cases use for names and sides (tests/differential_scheme_bar.rs has the same list) */
static const char *const STRINGS[] = { "", "America", "america", "AMERICA", "China", "GLA", "Observer", "observer",
	"AmericaSmall", "Americasmall", "Default", "default", "America8x6", "ChinaSmall", "Civilian" };
static const Int NSTRINGS = (Int)(sizeof STRINGS / sizeof STRINGS[0]);

typedef Int Color;
#include "scb_color_undef.inc"
#include "scb_make_color.inc"

typedef const char *NameKeyType;
#define NAMEKEY(x) (x)
struct NameKeyGeneratorStub { NameKeyType nameToKey( const char *n ) { return n; } };
NameKeyGeneratorStub g_nkg;
NameKeyGeneratorStub *TheNameKeyGenerator = &g_nkg;

enum ControlBarContext
{
	CB_CONTEXT_NONE,
	CB_CONTEXT_COMMAND,
	CB_CONTEXT_STRUCTURE_INVENTORY,
	CB_CONTEXT_BEACON,
	CB_CONTEXT_UNDER_CONSTRUCTION,
	CB_CONTEXT_MULTI_SELECT,
	CB_CONTEXT_OBSERVER_INFO,
	CB_CONTEXT_OBSERVER_LIST,
	CB_CONTEXT_OCL_TIMER,
	NUM_CB_CONTEXTS
};
enum ControlBarStages { CONTROL_BAR_STAGE_DEFAULT = 0 };
enum CommandButtonMappedBorderType
{
	COMMAND_BUTTON_BORDER_NONE = 0,
	COMMAND_BUTTON_BORDER_BUILD,
	COMMAND_BUTTON_BORDER_UPGRADE,
	COMMAND_BUTTON_BORDER_ACTION,
	COMMAND_BUTTON_BORDER_SYSTEM,
};
enum GameMode { GAME_SINGLE_PLAYER, GAME_LAN, GAME_SKIRMISH, GAME_REPLAY, GAME_SHELL, GAME_INTERNET, GAME_NONE };

class Drawable {};
class Image {};
class ControlBarSchemeImage {};
class ControlBarSchemeAnimation {};
enum { MAX_CONTROL_BAR_SCHEME_IMAGE_LAYERS = 6 };

/* the windows: the communicator (-1) and, recorded by index, the beacon (0), idle worker (1) and general (2) */
class GameWindow
{
public:
	Int m_index;
	Int winHide( Bool hide ) { rec(42, m_index, hide ? 1 : 0); return 0; }
	Int winEnable( Bool enable ) { rec(43, m_index, enable ? 1 : 0); return 0; }
};
GameWindow g_windows[4] = { { 0 }, { 1 }, { 2 }, { -1 } };
struct WindowManagerStub
{
	Bool m_has[4];
	GameWindow *winGetWindowFromId( GameWindow *, NameKeyType name )
	{
		static const char *const names[4] = { "ControlBar.wnd:ButtonPlaceBeacon", "ControlBar.wnd:ButtonIdleWorker",
			"ControlBar.wnd:ButtonGeneral", "ControlBar.wnd:PopupCommunicator" };
		for (Int i = 0; i < 4; ++i)
			if (strcmp(name, names[i]) == 0)
				return m_has[i] ? &g_windows[i] : NULL;
		return NULL;
	}
};
WindowManagerStub g_wm;
WindowManagerStub *TheWindowManager = &g_wm;

/* GadgetButtonSetBorder (GadgetPushButton.h: drawBorder defaults to TRUE) */
static void GadgetButtonSetBorder( GameWindow *, Color color, Bool drawBorder = TRUE );
static Int g_borderType;
static void GadgetButtonSetBorder( GameWindow *, Color color, Bool drawBorder )
{
	rec(6, g_borderType, color, drawBorder ? 1 : 0);
}

struct RecorderStub { Bool m_mp; Bool isMultiplayer() { return m_mp; } };
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

struct GameLogicStub { Int m_mode; GameMode getGameMode() { return (GameMode)m_mode; } };
GameLogicStub g_logic;
GameLogicStub *TheGameLogic = &g_logic;
struct GameInfoStub { Bool m_mp; Bool isMultiPlayer() { rec(44); return m_mp; } };
GameInfoStub g_gameInfo;
GameInfoStub *TheGameInfo = &g_gameInfo;

class Player
{
public:
	AsciiString m_side;
	Bool m_active;
	AsciiString getSide() { return m_side; }
	Bool isPlayerActive() const { return m_active; }
};
class PlayerTemplate
{
public:
	AsciiString m_side;
	AsciiString getSide() const { return m_side; }
};
PlayerTemplate g_templates[2]; /* a side's, FactionObserver's */
struct PlayerTemplateStoreStub
{
	Bool m_hasObserver;
	const PlayerTemplate *findPlayerTemplate( NameKeyType key ) const
	{
		return (m_hasObserver && strcmp(key, "FactionObserver") == 0) ? &g_templates[1] : NULL;
	}
};
PlayerTemplateStoreStub g_store;
PlayerTemplateStoreStub *ThePlayerTemplateStore = &g_store;

class CommandButton { public: Int m_id; };
CommandButton g_buttons[2] = { { 0 }, { 1 } };

class ControlBarSchemeManager;

/* m_isObserverCommandBar: each assignment recorded */
struct RecordedBool
{
	Bool m_value;
	RecordedBool &operator=( Bool b ) { m_value = b; rec(41, b ? 1 : 0); return *this; }
};

class ControlBar
{
public:
	ControlBar();
	ControlBarSchemeManager *m_controlBarSchemeManager;
	RecordedBool m_isObserverCommandBar;
	Bool m_found[2];
	Color m_buildUpClockColor;
	Color m_commandButtonBorderBuildColor;
	Color m_commandButtonBorderActionColor;
	Color m_commandButtonBorderUpgradeColor;
	Color m_commandButtonBorderSystemColor;
	Color m_commandBarBorderColor;
#include "scb_bar_h.inc"
	void updateBuildUpClockColor( Color color);
	void updateCommanBarBorderColors(Color build, Color action, Color upgrade, Color system );
	void setControlBarSchemeByPlayer(Player *p);
	void setControlBarSchemeByPlayerTemplate( const PlayerTemplate *pt);
	void setCommandBarBorder( GameWindow *button, CommandButtonMappedBorderType type);
	void switchToContext( ControlBarContext context, Drawable *draw ) { rec(40, context); }
	void switchControlBarStage( ControlBarStages ) { rec(45); }
	void hidePurchaseScience( void ) { rec(46); }
	const CommandButton *findCommandButton( const AsciiString &name )
	{
		Int id = (strcmp(name.str(), "NonCommand_Communicator") == 0) ? 0 : (strcmp(name.str(), "NonCommand_BriefingHistory") == 0) ? 1 : 99;
		rec(10, id);
		return (id < 2 && m_found[id]) ? &g_buttons[id] : NULL;
	}
	/* (setControlCommand is differential-tested in differential_bar_tooltips: here only the button given is recorded) */
	void setControlCommand( GameWindow *, const CommandButton *b )
	{
		if (!b)
			return;
		rec(11, b->m_id);
	}
};
ControlBar *TheControlBar = NULL;

class ControlBarScheme
{
public:
	ControlBarScheme( void );
	Int m_index;
	std::list< ControlBarSchemeAnimation * > m_animations;
	std::list< ControlBarSchemeImage * > m_layer[MAX_CONTROL_BAR_SCHEME_IMAGE_LAYERS];
	AsciiString m_name;
	ICoord2D m_ScreenCreationRes;
	AsciiString m_side;
	Image *m_buttonQueueImage, *m_rightHUDImage;
	Color m_buildUpClockColor, m_borderBuildColor, m_borderActionColor, m_borderUpgradeColor, m_borderSystemColor;
	Color m_commandBarBorderColor;
	Image *m_optionsButtonEnable, *m_optionsButtonHightlited, *m_optionsButtonPushed, *m_optionsButtonDisabled;
	Image *m_idleWorkerButtonEnable, *m_idleWorkerButtonHightlited, *m_idleWorkerButtonPushed, *m_idleWorkerButtonDisabled;
	Image *m_buddyButtonEnable, *m_buddyButtonHightlited, *m_buddyButtonPushed, *m_buddyButtonDisabled;
	Image *m_beaconButtonEnable, *m_beaconButtonHightlited, *m_beaconButtonPushed, *m_beaconButtonDisabled;
	Image *m_genBarButtonIn, *m_genBarButtonOn;
	Image *m_toggleButtonUpIn, *m_toggleButtonUpOn, *m_toggleButtonUpPushed;
	Image *m_toggleButtonDownIn, *m_toggleButtonDownOn, *m_toggleButtonDownPushed;
	Image *m_commandMarkerImage, *m_expBarForeground, *m_powerPurchaseImage;
	Image *m_generalButtonEnable, *m_generalButtonHightlited, *m_generalButtonPushed, *m_generalButtonDisabled;
	Image *m_uAttackButtonEnable, *m_uAttackButtonHightlited, *m_uAttackButtonPushed;
	Image *m_genArrow;
	Image *m_minMaxButtonEnable, *m_minMaxButtonHightlited, *m_minMaxButtonPushed;
	ICoord2D m_minMaxUL, m_minMaxLR, m_generalUL, m_generalLR, m_uAttackUL, m_uAttackLR, m_optionsUL, m_optionsLR;
	ICoord2D m_workerUL, m_workerLR, m_chatUL, m_chatLR, m_beaconUL, m_beaconLR, m_powerBarUL, m_powerBarLR;
	ICoord2D m_moneyUL, m_moneyLR;
	void init()
	{
		rec(30, m_index);
		if(TheControlBar)
		{
#include "scb_init_colors.inc"
		}
	}
};
typedef std::list< ControlBarScheme * > ControlBarSchemeList;

class ControlBarSchemeManager
{
public:
	ControlBarSchemeList m_schemeList;
	ControlBarScheme *m_currentScheme;
	Coord2D m_multiplyer;
	ControlBarScheme *findControlBarScheme( AsciiString name );
	void setControlBarSchemeByPlayerTemplate( const PlayerTemplate *pt, Bool useSmall = FALSE);
	void setControlBarSchemeByPlayer(Player *p);
};

#include "scb_ctor.inc"
#include "scb_find.inc"
#include "scb_template.inc"
#include "scb_player.inc"

ControlBar::ControlBar()
{
	m_controlBarSchemeManager = NULL;
	m_isObserverCommandBar.m_value = FALSE;
#include "scb_bar_ctor_a.inc"
#include "scb_bar_ctor_b.inc"
}
#include "scb_bar_by.inc"
#include "scb_bar_update.inc"
#include "scb_bar_border.inc"

} // namespace scb

extern "C" int ref_scheme_bar_case(const int *in, int nin, int *out, int cap)
{
	using namespace scb;
	g_out.clear();
	Int kind = in[0];
	g_recorder.m_mp = in[1] != 0;
	g_wm.m_has[3] = in[2] != 0;
	g_display.m_width = (UnsignedInt)in[5];
	g_display.m_height = (UnsignedInt)in[6];
	g_logic.m_mode = in[12];
	g_gameInfo.m_mp = in[13] != 0;
	g_wm.m_has[0] = in[14] != 0;
	g_wm.m_has[1] = in[15] != 0;
	g_wm.m_has[2] = in[16] != 0;
	g_store.m_hasObserver = in[17] != 0;
	Player player;
	player.m_side.set(STRINGS[in[8] % NSTRINGS]);
	player.m_active = in[11] != 0;
	g_templates[0].m_side.set(STRINGS[in[8] % NSTRINGS]);
	g_templates[1].m_side.set(STRINGS[in[8] % NSTRINGS]);
	const PlayerTemplate *pt = in[7] ? &g_templates[in[18] ? 1 : 0] : NULL;

	Int n = in[21];
	std::vector<ControlBarScheme> schemes(n);
	ControlBarSchemeManager m;
	for (Int i = 0; i < n && 22 + i * 11 + 10 < nin; ++i) {
		const int *w = &in[22 + i * 11];
		ControlBarScheme &s = schemes[i];
		s.m_index = i;
		/* newControlBarScheme lowercases the name */
		s.m_name.set(STRINGS[w[0] % NSTRINGS]);
		s.m_name.toLower();
		s.m_side.set(STRINGS[w[1] % NSTRINGS]);
		s.m_ScreenCreationRes.x = w[2];
		s.m_ScreenCreationRes.y = w[3];
		/* the INI's colours (parseColorInt), each when the block has it */
		if (w[4] & 1) s.m_buildUpClockColor = w[5];
		if (w[4] & 2) s.m_borderBuildColor = w[6];
		if (w[4] & 4) s.m_commandBarBorderColor = w[7];
		if (w[4] & 8) s.m_borderActionColor = w[8];
		if (w[4] & 16) s.m_borderUpgradeColor = w[9];
		if (w[4] & 32) s.m_borderSystemColor = w[10];
		m.m_schemeList.push_back(&s);
	}
	m.m_currentScheme = (in[9] >= 0 && in[9] < n) ? &schemes[in[9]] : NULL;
	m.m_multiplyer.x = 1.0f;
	m.m_multiplyer.y = 1.0f;

	ControlBar bar;
	bar.m_found[0] = in[3] != 0;
	bar.m_found[1] = in[4] != 0;
	bar.m_controlBarSchemeManager = in[10] ? &m : NULL;
	TheControlBar = &bar;
	if (kind == 0)
		bar.setControlBarSchemeByPlayer(&player);
	else if (kind == 1)
		bar.setControlBarSchemeByPlayerTemplate(pt);
	else
		m.setControlBarSchemeByPlayerTemplate(pt, in[19] != 0);

	rec(20, m.m_currentScheme ? m.m_currentScheme->m_index : -1, bitsOf(m.m_multiplyer.x), bitsOf(m.m_multiplyer.y));
	rec(21, bar.m_buildUpClockColor, bar.getBorderColor());
	rec(22, bar.m_commandButtonBorderBuildColor, bar.m_commandButtonBorderActionColor, bar.m_commandButtonBorderUpgradeColor);
	rec(23, bar.m_commandButtonBorderSystemColor);
	for (g_borderType = 0; g_borderType < 6; ++g_borderType)
		bar.setCommandBarBorder(&g_windows[0], (CommandButtonMappedBorderType)g_borderType);
	TheControlBar = NULL;

	int nout = (int)g_out.size();
	if (nout > cap)
		nout = cap;
	if (nout > 0)
		memcpy(out, &g_out[0], nout * sizeof(int));
	return nout;
}
