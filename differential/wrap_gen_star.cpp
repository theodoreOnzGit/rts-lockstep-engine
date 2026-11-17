/* Differential-test wrapper for the general's star and the ControlBarArrow group (GitHub #119, upstream commit
** 0a05454d), cut verbatim by build.rs into OUT_DIR/gen_star_gen:
**   gs_star.inc   GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp, `ControlBar::getStarImage`,
**                 `ControlBar::onPlayerRankChanged` and `ControlBar::onPlayerSciencePurchasePointsChanged` (to the
**                 comment of `evaluateContextUI`);
**   gs_init.inc   ControlBar.cpp, `ControlBar::init`'s lines from `m_genStarOn = TheMappedImageCollection` (to
**                 `m_rankVeteranIcon = `): the star images and the two values;
**   gs_reset.inc  ControlBar.cpp, the end of `ControlBar::reset` from `if(TheTransitionHandler)` (to `}  // end reset`;
**                 the wrapper closes the function).
** No edit. They are compiled inside namespace gs on stand-ins: a `ControlBar` declaring the members they use
** (ControlBar.h's names and types), the player list (the local player's index and science purchase points), the
** transition handler (present or NULL), the InGameUI's input, the window manager (ButtonGeneral found or not), the game
** logic's frame and the mapped image collection (present, with the two star images). The calls that leave are recorded.
**
** Entry point (extern "C", tests/differential_gen_star.rs):
**   ref_gen_star_seq(in, nin, out, cap) runs one sequence and writes its words to out.
**   `in` is the setup, 4 words [the local player's index, m_genStarFlash, m_lastFlashedAtPointValue, m_genArrow set],
**   then steps of 8 words [kind, player, points, handler, input, button, frame, collection]:
**     kind 0  onPlayerRankChanged(player)
**     kind 1  onPlayerSciencePurchasePointsChanged(player)
**     kind 2  getStarImage()
**     kind 3  the end of reset
**     kind 4  init's star lines
**   each step first sets the local player's points, TheTransitionHandler (NULL or not), the input, ButtonGeneral's
**   presence, the frame and TheMappedImageCollection (NULL or not).
**   Records of 3 words: [1, group] setGroup (1: "ControlBarArrow", 99 another), [2, group] remove, [3, 0] markUIDirty,
**   [4, image] GadgetButtonSetEnabledImage (1 m_generalButtonEnable, 2 m_generalButtonHighlight, 99 another),
**   [99, 0] a call the translation never makes (a TRUE `immidiate` or `skipPending`, another window, a non-NULL return),
**   [6, on, off] after init (the star images found: 1/0),
**   and after each step [100, m_genStarFlash, m_lastFlashedAtPointValue], [101, m_genArrow != NULL, 0]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "Common/AsciiString.h"

namespace gs {

std::vector<int> g_out;
void rec(Int tag, Int a = 0, Int b = 0)
{
	Int w[3] = { tag, a, b };
	g_out.insert(g_out.end(), w, w + 3);
}

enum { LOGICFRAMES_PER_SECOND = 30 };

/* ---- the images ---- */
struct Image { Int m_id; };
Image g_enable = { 1 }, g_highlight = { 2 }, g_starOn = { 3 }, g_starOff = { 4 }, g_arrow = { 5 };
class ImageCollection
{
public:
	const Image *findImageByName( const AsciiString &name )
	{
		if (!strcmp(name.str(), "BarButtonGenStarON")) return &g_starOn;
		if (!strcmp(name.str(), "BarButtonGenStarOFF")) return &g_starOff;
		return NULL;
	}
};
ImageCollection g_images;
ImageCollection *TheMappedImageCollection = NULL;

/* ---- the windows ---- */
class GameWindow { public: Int m_id; };
GameWindow g_general = { 7 };
Bool g_buttonExists;
typedef Int NameKeyType;
class NameKeyGenerator
{
public:
	NameKeyType nameToKey( const AsciiString &s ) { return !strcmp(s.str(), "ControlBar.wnd:ButtonGeneral") ? 1 : 2; }
};
NameKeyGenerator g_keys;
NameKeyGenerator *TheNameKeyGenerator = &g_keys;
class GameWindowManager
{
public:
	GameWindow *winGetWindowFromId( GameWindow *parent, NameKeyType id ) { return (parent == NULL && id == 1 && g_buttonExists) ? &g_general : NULL; }
};
GameWindowManager g_wm;
GameWindowManager *TheWindowManager = &g_wm;
void GadgetButtonSetEnabledImage( GameWindow *w, const Image *i )
{
	if (w != &g_general) rec(99);
	rec(4, i == &g_enable ? 1 : i == &g_highlight ? 2 : 99);
}

/* ---- the players, the transitions, the InGameUI, the logic ---- */
class Player
{
public:
	Int m_index;
	Int m_points;
	Bool isLocalPlayer() const;
	Int getSciencePurchasePoints() const { return m_points; }
};
Player g_local, g_other;
Bool Player::isLocalPlayer() const { return this == &g_local; }
class PlayerList { public: Player *getLocalPlayer() { return &g_local; } };
PlayerList g_players;
PlayerList *ThePlayerList = &g_players;
class GameWindowTransitionsHandler
{
public:
	void setGroup( AsciiString groupName, Bool immidiate = FALSE )
	{
		rec(1, !strcmp(groupName.str(), "ControlBarArrow") ? 1 : 99);
		if (immidiate) rec(99);
	}
	void remove( AsciiString groupName, Bool skipPending = FALSE )
	{
		rec(2, !strcmp(groupName.str(), "ControlBarArrow") ? 1 : 99);
		if (skipPending) rec(99);
	}
};
GameWindowTransitionsHandler g_handler;
GameWindowTransitionsHandler *TheTransitionHandler = NULL;
struct InGameUIStub { Bool m_input; Bool getInputEnabled() { return m_input; } };
InGameUIStub g_ui;
InGameUIStub *TheInGameUI = &g_ui;
struct GameLogicStub { UnsignedInt m_frame; UnsignedInt getFrame() { return m_frame; } };
GameLogicStub g_logic;
GameLogicStub *TheGameLogic = &g_logic;

/* ---- TheControlBar ---- */
class ControlBar
{
public:
	const Image *m_genStarOn;
	const Image *m_genStarOff;
	Bool m_genStarFlash;
	Int m_lastFlashedAtPointValue;
	const Image *m_genArrow;
	const Image *m_generalButtonEnable;
	const Image *m_generalButtonHighlight;
	void markUIDirty( void ) { rec(3); }
	const Image *getStarImage( void );
	void onPlayerRankChanged( const Player *p );
	void onPlayerSciencePurchasePointsChanged( const Player *p );
	void initStar( void );
	void resetTail( void );
};

#include "gs_star.inc"

void ControlBar::initStar( void )
{
#include "gs_init.inc"
}

void ControlBar::resetTail( void )
{
#include "gs_reset.inc"
}

}  // namespace gs

extern "C" int ref_gen_star_seq(const int *in, int nin, int *out, int cap)
{
	using namespace gs;
	g_out.clear();
	ControlBar cb;
	g_local.m_index = in[0];
	g_other.m_index = -1;
	cb.m_genStarFlash = in[1] != 0;
	cb.m_lastFlashedAtPointValue = in[2];
	cb.m_genArrow = in[3] ? &g_arrow : NULL;
	cb.m_genStarOn = cb.m_genStarOff = NULL;
	cb.m_generalButtonEnable = &g_enable;
	cb.m_generalButtonHighlight = &g_highlight;
	for (int i = 4; i + 8 <= nin; i += 8)
	{
		const int *s = in + i;
		g_local.m_points = s[2];
		TheTransitionHandler = s[3] ? &g_handler : NULL;
		g_ui.m_input = s[4] != 0;
		g_buttonExists = s[5] != 0;
		g_logic.m_frame = (UnsignedInt)s[6];
		TheMappedImageCollection = s[7] ? &g_images : NULL;
		const Player *p = s[1] == g_local.m_index ? &g_local : &g_other;
		switch (s[0])
		{
			case 0: cb.onPlayerRankChanged(p); break;
			case 1: cb.onPlayerSciencePurchasePointsChanged(p); break;
			case 2: if (cb.getStarImage() != NULL) rec(99); break;
			case 3: cb.resetTail(); break;
			case 4: cb.initStar(); rec(6, cb.m_genStarOn == &g_starOn ? 1 : 0, cb.m_genStarOff == &g_starOff ? 1 : 0); break;
		}
		rec(100, cb.m_genStarFlash ? 1 : 0, cb.m_lastFlashedAtPointValue);
		rec(101, cb.m_genArrow != NULL ? 1 : 0);
	}
	int n = (int)g_out.size();
	for (int i = 0; i < n && i < cap; ++i) out[i] = g_out[i];
	return n;
}
