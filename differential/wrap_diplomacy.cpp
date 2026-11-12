/* Differential-test wrapper for the briefing history window (GitHub #148, upstream commit 0a05454d), cut verbatim by
** build.rs into OUT_DIR/diplomacy_gen:
**   dp_cut.inc   GameEngine/Source/GameClient/GUI/GUICallbacks/Diplomacy.cpp, from the statics (`static NameKeyType
**                staticTextPlayerID[MAX_SLOTS];`) to `void PopulateInGameDiplomacyPopup( void )`: grabWindowPointers,
**                releaseWindowPointers, updateFunc, theBriefingList, GetBriefingTextList, UpdateDiplomacyBriefingText,
**                ShowDiplomacy, ResetDiplomacy, HideDiplomacy, ToggleDiplomacy, DiplomacyInput and DiplomacySystem.
** No edit. Compiled inside namespace dp on stand-ins that record every call (the same model as the Rust test's host,
** tests/differential_diplomacy.rs): the window manager holds Diplomacy.wnd's named windows once winCreateLayout made the
** layout (the solo list box only when the sequence's layout has it), each window's hidden and enabled bits and a list
** box's entries; NAMEKEY interns names (the layout's names first, in a fixed order); TheGameText->fetch gives the label's
** bytes as units; the animation manager is finished after its third update once registered or reversed; the game's
** flags (input, intro movie, loading, quit menu, disconnect screen, multiplayer, m_animateWindows) are the sequence's.
** PopulateInGameDiplomacyPopup, HideQuitMenu, the buddy calls and TheGameSpyInfo (NULL) are recorded stand-ins.
**
** Entry point (extern "C", tests/differential_diplomacy.rs):
**   ref_diplomacy_seq(in, nin, out, cap): `in` is [the layout has ListboxSolo], then steps of 8 words [kind, a, b, c, d,
**   e, f, g]:
**     kind 0  UpdateDiplomacyBriefingText(label a (8: ""), clear b)    kind 1  ShowDiplomacy(a)
**     kind 2  HideDiplomacy(a)            kind 3  ToggleDiplomacy(a)    kind 4  ResetDiplomacy()
**     kind 5  the layout's update (updateFunc), while theLayout is not NULL
**     kind 6  DiplomacyInput(theWindow, msg a (0 GWM_CHAR, 1 other), key b)
**     kind 7  DiplomacySystem(theWindow, msg a (0 GGM_FOCUS_CHANGE, 1 GWM_INPUT_FOCUS, 2 GBM_SELECTED, 3 other), the
**             control: named window b (-1 a window of no name), mData1 c (GWM_INPUT_FOCUS)); a GBM_SELECTED only while
**             theWindow is not NULL (a click needs the window)
**     kind 8  the game's flags: input a, intro b, loading c, quit menu d, disconnect e, multiplayer f, animate g
**   The calls, as they are made: [1, w, hide] winHide, [2, w, enable] winEnable, [3, w] GadgetListBoxReset, [4, w, the
**   text's FNV-1a, colour] GadgetListBoxAddEntryText, [5, w] GadgetRadioSetSelection, [6] winCreateLayout, [7, hide]
**   layout hide, [8] destroyWindows, [9] NEW AnimateWindowManager, [10] delete it, [11] reset, [12, w]
**   registerGameWindow(w, SLIDE_TOP, TRUE, 200), [13] reverseAnimateWindow, [14] update, [15, 1|0]
**   register/unregisterWindowLayout, [16] HideQuitMenu, [17] PopulateInGameDiplomacyPopup; a window is its name's index
**   (-1 none). After each step [100, the handled result (-1 none), the focus flag, theWindow != NULL, the list's length,
**   each label's index]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <list>
#include <vector>
#include <string>
#include <algorithm>
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"

namespace dp {

std::vector<int> g_out;
static void rec(const std::vector<Int> &w) { g_out.insert(g_out.end(), w.begin(), w.end()); }

#define MAX_SLOTS 8
typedef Int NameKeyType;
enum { NAMEKEY_INVALID = 0 };
typedef Int Color;
typedef size_t WindowMsgData;   /* pointer-sized: mData1 carries the control, mData2 the focus flag's address */
enum WindowMsgHandledType { MSG_IGNORED, MSG_HANDLED };
enum { GWM_CHAR = 21, GWM_INPUT_FOCUS = 23, GGM_FOCUS_CHANGE = 16387, GBM_SELECTED = 16392 };
enum { KEY_ESC = 0x01 };
enum { WIN_ANIMATION_SLIDE_TOP = 4 };

/* the layout's names, in the Rust test's order */
static const char *const NAMES[] = { "Diplomacy.wnd:DiplomacyParent", "Diplomacy.wnd:ListboxSolo", "Diplomacy.wnd:RadioButtonInGame",
	"Diplomacy.wnd:RadioButtonBuddies", "Diplomacy.wnd:InGameParent", "Diplomacy.wnd:BuddiesParent", "Diplomacy.wnd:SoloParent",
	"Diplomacy.wnd:ButtonHide" };
static const Int NNAMES = 8;
static const char *const LABELS[] = { "SCRIPT:Brief1", "SCRIPT:Brief2", "SCRIPT:Pop", "DIALOGEVENT:Speech1Subtitle", "SCRIPT:Brief1 ", "script:brief1", "MISSING", "SCRIPT:Long briefing label", "" };

/* NAMEKEY: the layout's names first (keys 1..8), then each new name the next key */
std::vector<std::string> g_keys;
struct NameKeyGeneratorStub
{
	NameKeyType nameToKey( const AsciiString &n )
	{
		for (size_t i = 0; i < g_keys.size(); ++i)
			if (g_keys[i] == n.str())
				return (NameKeyType)i + 1;
		g_keys.push_back(n.str());
		return (NameKeyType)g_keys.size();
	}
};
NameKeyGeneratorStub g_nkg;
NameKeyGeneratorStub *TheNameKeyGenerator = &g_nkg;
#define NAMEKEY(s) TheNameKeyGenerator->nameToKey(s)

class GameWindow
{
public:
	Int m_index;
	Bool m_hidden, m_enabled;
	std::vector<std::pair<Int, Color> > m_entries;
	void winHide( Bool hide ) { m_hidden = hide; rec({1, m_index, hide ? 1 : 0}); }
	void winEnable( Bool enable ) { m_enabled = enable; rec({2, m_index, enable ? 1 : 0}); }
	Bool winIsHidden( void ) { return m_hidden; }
	Int winGetWindowId( void ) { return m_index < 0 ? 999 : NAMEKEY(AsciiString(NAMES[m_index])); }
};
static Int idx(GameWindow *w) { return w ? w->m_index : -1; }

GameWindow g_windows[8];
GameWindow g_foreign;
Bool g_made = FALSE;
Bool g_hasListbox = TRUE;

static Int fnv(const UnicodeString &t)
{
	UnsignedInt h = 2166136261u;
	for (Int i = 0; i < t.getLength(); ++i) {
		h ^= (UnsignedInt)(unsigned short)t.str()[i];
		h *= 16777619u;
	}
	return (Int)h;
}
static void GadgetListBoxReset( GameWindow *w ) { w->m_entries.clear(); rec({3, idx(w)}); }
static Int GadgetListBoxGetNumEntries( GameWindow *w ) { return (Int)w->m_entries.size(); }
static Int GadgetListBoxAddEntryText( GameWindow *w, UnicodeString text, Color color, Int row, Int column = -1 )
{
	w->m_entries.push_back(std::make_pair(fnv(text), color));
	rec({4, idx(w), fnv(text), color});
	return (Int)w->m_entries.size() - 1;
}
static void GadgetRadioSetSelection( GameWindow *w, Bool sendMsg ) { rec({5, idx(w)}); }

typedef void (*WindowLayoutCallback)( class WindowLayout *, void * );
class WindowLayout
{
public:
	GameWindow *getFirstWindow( void ) { return &g_windows[0]; }
	void setUpdate( void (*f)( WindowLayout *, void * ) ) { }
	void hide( Bool h ) { rec({7, h ? 1 : 0}); for (Int i = 0; i < NNAMES; ++i) g_windows[i].m_hidden = h; }
	void destroyWindows( void ) { rec({8}); g_made = FALSE; for (Int i = 0; i < NNAMES; ++i) g_windows[i].m_entries.clear(); }
	void deleteInstance( void ) { }
};
WindowLayout g_layout;
struct WindowManagerStub
{
	GameWindow *winGetWindowFromId( GameWindow *parent, NameKeyType key )
	{
		if (!g_made)
			return NULL;
		for (Int i = 0; i < NNAMES; ++i)
			if (NAMEKEY(AsciiString(NAMES[i])) == key)
				return (i == 1 && !g_hasListbox) ? NULL : &g_windows[i];
		return NULL;
	}
	WindowLayout *winCreateLayout( AsciiString name )
	{
		rec({6});
		g_made = TRUE;
		for (Int i = 0; i < NNAMES; ++i) {
			g_windows[i].m_hidden = FALSE;
			g_windows[i].m_enabled = TRUE;
			g_windows[i].m_entries.clear();
		}
		return &g_layout;
	}
};
WindowManagerStub g_wm;
WindowManagerStub *TheWindowManager = &g_wm;

/* the animation manager: finished after its third update once registered or reversed */
class AnimateWindowManager
{
public:
	Int m_left;
	Bool m_reverse;
	AnimateWindowManager() : m_left(0), m_reverse(FALSE) { rec({9}); }
	~AnimateWindowManager() { rec({10}); }
	void reset( void ) { m_left = 0; m_reverse = FALSE; rec({11}); }
	void registerGameWindow( GameWindow *w, Int type, Bool needsToFinish, Int ms, Int delayMs = 0 ) { m_left = 3; rec({12, idx(w)}); }
	void reverseAnimateWindow( void ) { m_reverse = TRUE; m_left = 3; rec({13}); }
	void update( void ) { if (m_left > 0) --m_left; rec({14}); }
	Bool isFinished( void ) { return m_left == 0; }
	Bool isReversed( void ) { return m_reverse; }
};
#define NEW new

struct Flags { Bool input, intro, loading, quit, disconnect, multiplayer, animate; };
Flags g_flags;
struct InGameUIStub
{
	Bool getInputEnabled( void ) { return g_flags.input; }
	Bool isQuitMenuVisible( void ) { return g_flags.quit; }
	Color getMessageColor( Bool alt ) { return alt ? (Color)0xFFB4B4B4 : (Color)0xFFFFFFFF; }
	void registerWindowLayout( WindowLayout * ) { rec({15, 1}); }
	void unregisterWindowLayout( WindowLayout * ) { rec({15, 0}); }
};
InGameUIStub g_ui;
InGameUIStub *TheInGameUI = &g_ui;
struct GameLogicStub { Bool isIntroMoviePlaying( void ) { return g_flags.intro; } Bool isLoadingMap( void ) { return g_flags.loading; } };
GameLogicStub g_logic;
GameLogicStub *TheGameLogic = &g_logic;
struct DisconnectMenuStub { Bool isScreenVisible( void ) { return g_flags.disconnect; } };
DisconnectMenuStub g_disconnect;
DisconnectMenuStub *TheDisconnectMenu = &g_disconnect;
struct GlobalDataStub { Bool m_animateWindows; };
GlobalDataStub g_gd;
GlobalDataStub *TheGlobalData = &g_gd;
struct RecorderStub { Bool isMultiplayer( void ) { return g_flags.multiplayer; } };
RecorderStub g_recorder;
RecorderStub *TheRecorder = &g_recorder;
struct GameTextStub { UnicodeString fetch( AsciiString label ) { UnicodeString u; u.translate(label); return u; } };
GameTextStub g_text;
GameTextStub *TheGameText = &g_text;
struct GameSpyInfoStub { Int getLocalProfileID( void ) { return 0; } };
GameSpyInfoStub *TheGameSpyInfo = NULL;
struct GameSlotStub { void mute( Bool ) { } };
struct GameInfoStub { GameSlotStub m_slot; GameSlotStub *getSlot( Int ) { return &m_slot; } };
GameInfoStub g_gameInfo;
GameInfoStub *TheGameInfo = &g_gameInfo;
static void PopulateInGameDiplomacyPopup( void ) { rec({17}); }
static void HideQuitMenu( void ) { rec({16}); }
static void PopulateOldBuddyMessages( void ) { }
WindowMsgHandledType BuddyControlSystem( GameWindow *window, UnsignedInt msg, WindowMsgData mData1, WindowMsgData mData2 ) { return MSG_IGNORED; }
void InitBuddyControls( Int type ) { }
void updateBuddyInfo( void ) { }
typedef std::list<AsciiString> BriefingList;
void ShowDiplomacy( Bool immediate );
void HideDiplomacy( Bool immediate = TRUE );
void ToggleDiplomacy( Bool immediate = TRUE );
void ResetDiplomacy( void );

#include "dp_cut.inc"

} /* namespace dp */

extern "C" int ref_diplomacy_seq(const int *in, int nin, int *out, int cap)
{
	using namespace dp;
	g_out.clear();
	g_keys.clear();
	for (Int i = 0; i < NNAMES; ++i) {
		g_keys.push_back(NAMES[i]);
		g_windows[i].m_index = i;
		g_windows[i].m_hidden = FALSE;
		g_windows[i].m_enabled = TRUE;
		g_windows[i].m_entries.clear();
	}
	g_foreign.m_index = -1;
	g_made = FALSE;
	g_hasListbox = in[0] != 0;
	g_flags.input = TRUE; g_flags.intro = FALSE; g_flags.loading = FALSE; g_flags.quit = FALSE;
	g_flags.disconnect = FALSE; g_flags.multiplayer = FALSE; g_flags.animate = TRUE;
	g_gd.m_animateWindows = TRUE;
	/* the file's statics, as a new program starts them */
	theLayout = NULL;
	theWindow = NULL;
	theAnimateWindowManager = NULL;
	theBriefingList.clear();
	radioButtonInGame = radioButtonBuddies = winInGame = winBuddies = winSolo = NULL;
	for (int k = 1; k + 8 <= nin; k += 8) {
		const int *w = in + k + 1;
		Int handled = -1, focus = 1;
		switch (in[k]) {
		case 0: UpdateDiplomacyBriefingText(AsciiString(LABELS[w[0] % 9]), w[1] != 0); break;
		case 1: ShowDiplomacy(w[0] != 0); break;
		case 2: HideDiplomacy(w[0] != 0); break;
		case 3: ToggleDiplomacy(w[0] != 0); break;
		case 4: ResetDiplomacy(); break;
		case 5:
			if (theLayout)
				updateFunc(theLayout, NULL);
			break;
		case 6: handled = DiplomacyInput(theWindow, w[0] == 0 ? GWM_CHAR : 7, (WindowMsgData)w[1], 0); break;
		case 7: {
			UnsignedInt msg = w[0] == 0 ? GGM_FOCUS_CHANGE : w[0] == 1 ? GWM_INPUT_FOCUS : w[0] == 2 ? GBM_SELECTED : 7;
			Bool f = TRUE;
			GameWindow *control = w[1] >= 0 && w[1] < NNAMES ? &g_windows[w[1]] : &g_foreign;
			WindowMsgData d1 = msg == GBM_SELECTED ? (WindowMsgData)control : (WindowMsgData)(w[2] != 0 ? 1 : 0);
			/* (a control's click needs the window: before ShowDiplomacy made it, or after ResetDiplomacy, the radio
			** buttons' case would call winHide on the NULL winInGame, as upstream would; no click reaches it there) */
			if (msg != GBM_SELECTED || theWindow)
				handled = DiplomacySystem(theWindow, msg, d1, msg == GWM_INPUT_FOCUS ? (WindowMsgData)&f : 0);
			focus = f ? 1 : 0;
			break;
		}
		case 8:
			g_flags.input = w[0] != 0; g_flags.intro = w[1] != 0; g_flags.loading = w[2] != 0; g_flags.quit = w[3] != 0;
			g_flags.disconnect = w[4] != 0; g_flags.multiplayer = w[5] != 0; g_gd.m_animateWindows = w[6] != 0;
			break;
		}
		rec({100, handled, focus, theWindow ? 1 : 0, (Int)theBriefingList.size()});
		for (BriefingList::iterator it = theBriefingList.begin(); it != theBriefingList.end(); ++it) {
			Int li = -1;
			for (Int i = 0; i < 9; ++i)
				if (*it == AsciiString(LABELS[i]))
					li = i;
			rec({li});
		}
	}
	if (theAnimateWindowManager) {
		/* (the program's end: dropped without the call record) */
		theAnimateWindowManager = NULL;
	}
	int n = (int)g_out.size();
	if (n > cap)
		n = cap;
	if (n > 0)
		memcpy(out, &g_out[0], n * sizeof(int));
	return n;
}
