/* Differential-test wrapper for the control bar draft (#215, upstream commit 0a05454d), src/drafts/sweep_gui_logic/control_bar.rs
** (populate_purchase_science, update_context_purchase_science, hide_communicator, set_control_bar_scheme_by_name, populate_special_power_shortcut,
** has_any_shortcut_selection, update_special_power_shortcut, draw_special_power_shortcut_multiplier_text), cut verbatim by build.rs (drafts_guilogic4):
**   cb_opt.inc, cb_cmd.inc, cb_avail.inc, cb_max1.inc, cb_max2.inc   GameClient/ControlBar.h: enum CommandOption, enum GUICommandType, enum CommandAvailability,
**                    MAX_COMMANDS_PER_SET, MAX_SPECIAL_POWER_SHORTCUTS;   cb_sci.inc   Common/Science.h: enum ScienceType;   gw_status.inc   the window status enum;
**   cb_fn_purchase.inc  ControlBar.cpp: ControlBar::populatePurchaseScience;   cb_fn_ctx.inc  updateContextPurchaseScience;   cb_fn_hide.inc  hideCommunicator;
**   cb_fn_scheme.inc  setControlBarSchemeByName;   cb_fn_popsp.inc  populateSpecialPowerShortcut;   cb_fn_has.inc  hasAnyShortcutSelection;
**   cb_fn_updsp.inc  updateSpecialPowerShortcut;   cb_fn_draw.inc  drawSpecialPowerShortcutMultiplierText.
** Built on upstream's own UnicodeString.cpp and AsciiString.cpp (unchanged; -fshort-wchar; classes renamed by build.rs; the g3 prelude's wide printf).
** Stand-ins (outside the cuts), each logging what it is asked to do: class ControlBar (the data members these functions read, the methods they call: findCommandSet,
** setControlCommand, getCommandAvailability, show / hide / animateSpecialPowerShortcut, the scheme manager call and switchControlBarStage); GameWindow (hidden,
** status and the button's data; winHide, winEnable, winIsHidden, winSetStatus, winClearStatus), GadgetButtonGetData, GadgetButtonSetText, GadgetButtonSetAltSound,
** GadgetStaticTextSetText, GadgetProgressBarSetProgress; CommandSet (getCommandButton), CommandButton (the getters, copyImagesFrom, copyButtonTextFrom), SpecialPowerTemplate,
** UpgradeTemplate, ThingTemplate; Player and PlayerTemplate (every query answers from the tables the test gives), Object, ThePlayerList, TheScienceStore, TheScriptEngine,
** TheGameText (fetch returns "T:" and the label), TheWindowManager->winGetWindowFromId and TheNameKeyGenerator->nameToKey (the three named windows by their key);
** ScienceVec (std::vector); CONTROL_BAR_STAGE_DEFAULT (0, ControlBar.h); MAX_PURCHASE_SCIENCE_RANK_1/3/8 (4, 15, 4: build.rs asserts the header says so); CP_MASTER, CP_PURCHASE_SCIENCE (array indices);
** UpgradeMaskType (a bit mask, UnsignedInt); `m_specialPowerShortcutButtons` etc. are arrays as in ControlBar.h (so the cut's `== NULL` is never true, as upstream).
** DEBUG_LOG, DEBUG_CRASH (compiled out in the release build).
**
** Entry point (extern "C", tests/differential_draft_cbar.rs): ref_cbar_run(world, nw, ops, nops, log, logcap): see the test for the formats; returns the log's length. */
#include "Common/STLTypedefs.h"
#include "Common/CriticalSection.h"
#include <string.h>
#include <stdio.h>
#include <stdarg.h>
#include <string>
#include <vector>

#define DEBUG_LOG(x)
#define DEBUG_CRASH(x)
#define DEBUG_ASSERTCRASH(x, y)
#ifndef NULL
#define NULL 0
#endif
namespace { DynamicMemoryAllocator theCbarAllocator; }
DynamicMemoryAllocator *TheDynamicMemoryAllocator = &theCbarAllocator;
CriticalSection *TheUnicodeStringCriticalSection = 0;

static std::string theLog;
static void logf( const char *fmt, ... )
{
	char b[512];
	va_list ap;
	va_start( ap, fmt );
	vsnprintf( b, sizeof( b ), fmt, ap );
	va_end( ap );
	theLog += b;
	theLog += "\n";
}
static std::string narrow( const UnicodeString &u )
{
	std::string s;
	for (const WideChar *p = u.str(); *p; ++p) s += (char)*p;
	return s;
}

#include "gw_status.inc"
#include "cb_opt.inc"
#include "cb_cmd.inc"
#include "cb_avail.inc"
#include "cb_sci.inc"
typedef std::vector<ScienceType> ScienceVec;
enum { MAX_PURCHASE_SCIENCE_RANK_1 = 4, MAX_PURCHASE_SCIENCE_RANK_3 = 15, MAX_PURCHASE_SCIENCE_RANK_8 = 4 };
#include "cb_max1.inc"
#include "cb_max2.inc"
enum { CP_MASTER = 0, CP_PURCHASE_SCIENCE = 1, CP_COUNT = 2 };
enum { CONTROL_BAR_STAGE_DEFAULT = 0 };
typedef UnsignedInt UpgradeMaskType;


/* ---- the world, as the test gives it ---- */
enum { N_WIN = 71, N_SETS = 8, N_BUTTONS_MAX = 64, N_SCI = 9 };
class GameWindow
{
public:
	int h; bool hidden; UnsignedInt status; void *data;
	void winHide( Bool b ) { hidden = b; logf( "hide %d %d", h, (int)b ); }
	void winEnable( Bool e ) { logf( "enable %d %d", h, (int)e ); }
	Bool winIsHidden( void ) { return hidden; }
	UnsignedInt winSetStatus( UnsignedInt s ) { UnsignedInt o = status; status |= s; logf( "setstatus %d %x", h, s ); return o; }
	UnsignedInt winClearStatus( UnsignedInt s ) { UnsignedInt o = status; status &= ~s; logf( "clearstatus %d %x", h, s ); return o; }
};
static GameWindow theWins[N_WIN];
class ThingTemplate { public: int id; };
class SpecialPowerTemplate { public: Int type, required; Int getSpecialPowerType() const { return type; } ScienceType getRequiredScience() const { return (ScienceType)required; } };
class UpgradeTemplate { public: UnsignedInt mask; UpgradeMaskType getUpgradeMask() const { return mask; } };
class CommandButton
{
public:
	AsciiString name; UnsignedInt options; GUICommandType cmd; ScienceVec sciences; SpecialPowerTemplate sp; bool hasSp; UpgradeTemplate up; bool hasUp; ThingTemplate tt; bool hasTt;
	const AsciiString &getName() const { return name; }
	UnsignedInt getOptions() const { return options; }
	GUICommandType getCommandType() const { return cmd; }
	const ScienceVec &getScienceVec() const { return sciences; }
	const SpecialPowerTemplate *getSpecialPowerTemplate() const { return hasSp ? &sp : NULL; }
	const UpgradeTemplate *getUpgradeTemplate() const { return hasUp ? &up : NULL; }
	const ThingTemplate *getThingTemplate() const { return hasTt ? &tt : NULL; }
	void copyImagesFrom( const CommandButton *b, Bool markUnchanged ) const { logf( "copyimages %s %s", name.str(), b->name.str() ); }
	void copyButtonTextFrom( const CommandButton *b, Bool a, Bool c ) const { logf( "copytext %s %s", name.str(), b->name.str() ); }
};
static CommandButton theButtons[N_BUTTONS_MAX];
class CommandSet
{
public:
	bool exists; int btn[MAX_COMMANDS_PER_SET];
	const CommandButton *getCommandButton( Int i ) const { return ( i < 0 || i >= MAX_COMMANDS_PER_SET || btn[i] < 0 ) ? NULL : &theButtons[btn[i]]; }
};
static CommandSet theSets[N_SETS];
class Object { public: int id; AsciiString getCommandSetString() const { char b[16]; snprintf( b, sizeof( b ), "Set%d", id % 8 ); return AsciiString( b ); } };
static Object theObjs[64];
static Object *objOf( int id ) { return id ? &theObjs[id] : NULL; }
class PlayerTemplate
{
public:
	AsciiString r1, r3, r8, sc;
	const AsciiString &getPurchaseScienceCommandSetRank1() const { return r1; }
	const AsciiString &getPurchaseScienceCommandSetRank3() const { return r3; }
	const AsciiString &getPurchaseScienceCommandSetRank8() const { return r8; }
	const AsciiString &getSpecialPowerShortcutCommandSet() const { return sc; }
};
static PlayerTemplate thePt;
static bool havePt, haveLocal, local, active, gameEnding;
static bool hasSci[N_SCI], disabledSci[N_SCI], hiddenSci[N_SCI], prereqSci[N_SCI], rootSci[N_SCI];
static int costSci[N_SCI], points, skill, down, up, rank, findType[4], findAny[5], findThing[5], countType[4];
static bool upgrades[8], hasAnyShortcut;
static int availSalt;
class Player
{
public:
	PlayerTemplate *getPlayerTemplate() { return havePt ? &thePt : NULL; }
	Bool isLocalPlayer() { return local; }
	Bool isPlayerActive() { return active; }
	Bool hasScience( ScienceType s ) { return hasSci[s + 1]; }
	Bool isScienceDisabled( ScienceType s ) { return disabledSci[s + 1]; }
	Bool isScienceHidden( ScienceType s ) { return hiddenSci[s + 1]; }
	Int getSciencePurchasePoints() { return points; }
	Int getSkillPoints() { return skill; }
	Int getSkillPointsLevelDown() { return down; }
	Int getSkillPointsLevelUp() { return up; }
	Int getRankLevel() { return rank; }
	Bool hasUpgradeComplete( const UpgradeMaskType &m ) { for (int k = 0; k < 8; ++k) if (m == (1u << k)) return upgrades[k]; return 0; }
	Object *findMostReadyShortcutSpecialPowerOfType( Int t ) { return objOf( findType[t] ); }
	Object *findAnyExistingObjectWithThingTemplate( const ThingTemplate *tt ) { return objOf( findAny[tt->id] ); }
	Object *findMostReadyShortcutSpecialPowerForThing( const ThingTemplate *tt, UnsignedInt &pct ) { pct = 0; return objOf( findThing[tt->id] ); }
	Bool hasAnyShortcutSpecialPower() { return hasAnyShortcut; }
	Int countReadyShortcutSpecialPowersOfType( Int t ) { return countType[t]; }
};
static Player thePlayer;
class PlayerListStandIn { public: Player *getLocalPlayer() { return haveLocal ? &thePlayer : NULL; } };
static PlayerListStandIn thePlayerListObj;
static PlayerListStandIn *ThePlayerList = &thePlayerListObj;
class ScienceStoreStandIn
{
public:
	Bool playerHasPrereqsForScience( Player *, ScienceType s ) { return prereqSci[s + 1]; }
	Int getSciencePurchaseCost( ScienceType s ) { return costSci[s + 1]; }
	Bool playerHasRootPrereqsForScience( Player *, ScienceType s ) { return rootSci[s + 1]; }
};
static ScienceStoreStandIn theScienceStoreObj;
static ScienceStoreStandIn *TheScienceStore = &theScienceStoreObj;
class ScriptEngineStandIn { public: Bool isGameEnding() { return gameEnding; } };
static ScriptEngineStandIn theScriptEngineObj;
static ScriptEngineStandIn *TheScriptEngine = &theScriptEngineObj;
class GameTextStandIn { public: UnicodeString fetch( AsciiString label ) { UnicodeString u; AsciiString t( "T:" ); t.concat( label ); u.translate( t ); return u; } };
static GameTextStandIn theGameTextObj;
static GameTextStandIn *TheGameText = &theGameTextObj;
static bool named[3];
class NameKeyGeneratorStandIn
{
public:
	NameKeyType nameToKey( const char *n )
	{
		if (!strcmp( n, "GeneralsExpPoints.wnd:StaticTextRankPointsAvailable" )) return 1;
		if (!strcmp( n, "GeneralsExpPoints.wnd:ProgressBarExperience" )) return 2;
		if (!strcmp( n, "GeneralsExpPoints.wnd:StaticTextTitle" )) return 3;
		return 99;
	}
};
static NameKeyGeneratorStandIn theNkg;
static NameKeyGeneratorStandIn *TheNameKeyGenerator = &theNkg;
class WindowManagerStandIn
{
public:
	GameWindow *winGetWindowFromId( GameWindow *parent, NameKeyType key )
	{
		logf( "findwin %u parent %d", key, parent ? parent->h : -1 );
		return ( key >= 1 && key <= 3 && named[key - 1] ) ? &theWins[65 + key] : NULL;
	}
};
static WindowManagerStandIn theWmObj;
static WindowManagerStandIn *TheWindowManager = &theWmObj;
static void GadgetStaticTextSetText( GameWindow *w, UnicodeString t ) { logf( "statictext %d %s", w->h, narrow( t ).c_str() ); }
static void GadgetProgressBarSetProgress( GameWindow *w, Int p ) { logf( "progress %d %d", w->h, (int)p ); }
static void GadgetButtonSetText( GameWindow *w, UnicodeString t ) { logf( "buttontext %d %s", w->h, narrow( t ).c_str() ); }
static void GadgetButtonSetAltSound( GameWindow *w, const char *s ) { logf( "altsound %d %s", w->h, s ); }
static void *GadgetButtonGetData( GameWindow *w ) { return w->data; }
class AnimStandIn { public: bool finished; Bool isFinished() { return finished; } };

class ControlBar
{
public:
	void populatePurchaseScience( Player *player );
	void updateContextPurchaseScience( void );
	void hideCommunicator( Bool b );
	void setControlBarSchemeByName( const AsciiString &name );
	void populateSpecialPowerShortcut( Player *player );
	Bool hasAnyShortcutSelection() const;
	void updateSpecialPowerShortcut( void );
	void drawSpecialPowerShortcutMultiplierText();

	const CommandSet *findCommandSet( const AsciiString &name )
	{
		for (int k = 0; k < N_SETS; ++k)
		{
			char b[16];
			snprintf( b, sizeof( b ), "Set%d", k );
			if (theSets[k].exists && name == AsciiString( b )) return &theSets[k];
		}
		return NULL;
	}
	void setControlCommand( GameWindow *w, const CommandButton *b ) { w->data = (void *)b; logf( "setcmd %d %s", w->h, b->name.str() ); }
	CommandAvailability getCommandAvailability( const CommandButton *b, Object *obj, GameWindow *win, GameWindow *applyTo = NULL, Bool force = FALSE ) const
	{
		int sum = 0;
		for (const char *p = b->name.str(); *p; ++p) sum += (unsigned char)*p;
		int o = obj ? obj->id : 0, w = win ? win->h : -1, a = applyTo ? applyTo->h : -1;
		logf( "avail %s %d %d %d", b->name.str(), o, w, a );
		static const CommandAvailability tab[6] = { COMMAND_RESTRICTED, COMMAND_AVAILABLE, COMMAND_ACTIVE, COMMAND_HIDDEN, COMMAND_NOT_READY, COMMAND_CANT_AFFORD };
		return tab[( sum + 7 * o + 3 * (w + 1) + 5 * (a + 1) + availSalt ) % 6];
	}
	void showSpecialPowerShortcut() { logf( "showshortcut" ); }
	void hideSpecialPowerShortcut() { logf( "hideshortcut" ); }
	void animateSpecialPowerShortcut( Bool on ) { logf( "animate %d", (int)on ); }
	void switchControlBarStage( int stage ) { logf( "stage %d", stage ); }
	struct SchemeManagerStandIn { void setControlBarScheme( const AsciiString &n ) { logf( "scheme %s", n.str() ); } };
	SchemeManagerStandIn *m_controlBarSchemeManager;

	GameWindow *m_sciencePurchaseWindowsRank1[MAX_PURCHASE_SCIENCE_RANK_1];
	GameWindow *m_sciencePurchaseWindowsRank3[MAX_PURCHASE_SCIENCE_RANK_3];
	GameWindow *m_sciencePurchaseWindowsRank8[MAX_PURCHASE_SCIENCE_RANK_8];
	GameWindow *m_specialPowerShortcutButtons[MAX_SPECIAL_POWER_SHORTCUTS];
	GameWindow *m_specialPowerShortcutButtonParents[MAX_SPECIAL_POWER_SHORTCUTS];
	void *m_shortcutDisplayStrings[MAX_SPECIAL_POWER_SHORTCUTS];
	Int m_currentlyUsedSpecialPowersButtons;
	GameWindow *m_specialPowerShortcutParent;
	AnimStandIn *m_animateWindowManagerForGenShortcuts;
	GameWindow *m_commandWindows[MAX_COMMANDS_PER_SET];
	GameWindow *m_contextParent[CP_COUNT];
	GameWindow *m_communicatorButton;
};
static ControlBar theBar;
static ControlBar *TheControlBar = &theBar;
static AnimStandIn theAnim;

#include "cb_fn_purchase.inc"
#include "cb_fn_ctx.inc"
#include "cb_fn_hide.inc"
#include "cb_fn_scheme.inc"
#include "cb_fn_popsp.inc"
#include "cb_fn_has.inc"
#include "cb_fn_updsp.inc"
#include "cb_fn_draw.inc"

/* The world (world, nw 64-bit values), in the order the test writes it; the operations (ops: kind, argument pairs); the log of the stand-ins' calls.
** Kinds: 0 populatePurchaseScience, 1 updateContextPurchaseScience, 2 populateSpecialPowerShortcut, 3 hasAnyShortcutSelection (logs `has <0|1>`),
** 4 updateSpecialPowerShortcut, 5 drawSpecialPowerShortcutMultiplierText, 6 hideCommunicator(a), 7 setControlBarSchemeByName(manager present = a),
** and after the last one the state of every window is logged. */
extern "C" int ref_cbar_run( const long long *w, int nw, const long long *ops, int nops, char *log, int logcap )
{
	TheDynamicMemoryAllocator = &theCbarAllocator;
	theLog.clear();
	int k = 0;
	auto next = [&]() -> long long { return w[k++]; };
	int nUsed = (int)next();
	bool buttonsNull[11], parentsNull[11];
	for (int i = 0; i < 11; ++i) buttonsNull[i] = next() != 0;
	for (int i = 0; i < 11; ++i) parentsNull[i] = next() != 0;
	bool ctxPurchase = next() != 0, master = next() != 0, shortcutParent = next() != 0, animFinished = next() != 0, communicator = next() != 0;
	gameEnding = next() != 0; bool playerExists = next() != 0; havePt = next() != 0; local = next() != 0; active = next() != 0; haveLocal = next() != 0;
	for (int i = 0; i < 3; ++i) named[i] = next() != 0;
	for (int i = 0; i < N_WIN; ++i) { theWins[i].h = i; theWins[i].hidden = next() != 0; theWins[i].status = 0; theWins[i].data = NULL; }
	int shortcutData[11];
	for (int i = 0; i < 11; ++i) shortcutData[i] = (int)next();
	int pn[3], sn;
	for (int i = 0; i < 3; ++i) pn[i] = (int)next();
	sn = (int)next();
	for (int s = 0; s < N_SETS; ++s)
	{
		theSets[s].exists = next() != 0;
		for (int i = 0; i < MAX_COMMANDS_PER_SET; ++i) theSets[s].btn[i] = (int)next();
	}
	int nb = (int)next();
	for (int b = 0; b < nb; ++b)
	{
		CommandButton &c = theButtons[b];
		char nm[16];
		snprintf( nm, sizeof( nm ), "B%d", b );
		c.name = AsciiString( nm );
		c.options = (UnsignedInt)next();
		int kind = (int)next();
		c.cmd = kind == 0 ? GUI_COMMAND_PURCHASE_SCIENCE : kind == 1 ? GUI_COMMAND_SELECT_ALL_UNITS_OF_TYPE : kind == 2 ? GUI_COMMAND_SPECIAL_POWER : GUI_COMMAND_STOP;
		c.sciences.clear();
		int ns = (int)next();
		for (int i = 0; i < ns; ++i) c.sciences.push_back( (ScienceType)next() );
		c.hasSp = next() != 0; c.sp.type = (Int)next(); c.sp.required = (Int)next();
		c.hasUp = next() != 0; c.up.mask = 1u << (int)next();
		c.hasTt = next() != 0; c.tt.id = (int)next();
	}
	for (int i = 0; i < N_SCI; ++i) { hasSci[i] = next() != 0; disabledSci[i] = next() != 0; hiddenSci[i] = next() != 0; prereqSci[i] = next() != 0; rootSci[i] = next() != 0; costSci[i] = (int)next(); }
	points = (int)next(); skill = (int)next(); down = (int)next(); up = (int)next(); rank = (int)next();
	for (int i = 0; i < 8; ++i) upgrades[i] = next() != 0;
	for (int i = 0; i < 4; ++i) findType[i] = (int)next();
	for (int i = 0; i < 5; ++i) findAny[i] = (int)next();
	for (int i = 0; i < 5; ++i) findThing[i] = (int)next();
	hasAnyShortcut = next() != 0;
	for (int i = 0; i < 4; ++i) countType[i] = (int)next();
	availSalt = (int)next();
	for (int i = 0; i < 64; ++i) theObjs[i].id = i;
	/* the control bar */
	ControlBar &cb = theBar;
	for (int i = 0; i < MAX_PURCHASE_SCIENCE_RANK_1; ++i) cb.m_sciencePurchaseWindowsRank1[i] = &theWins[i];
	for (int i = 0; i < MAX_PURCHASE_SCIENCE_RANK_3; ++i) cb.m_sciencePurchaseWindowsRank3[i] = &theWins[4 + i];
	for (int i = 0; i < MAX_PURCHASE_SCIENCE_RANK_8; ++i) cb.m_sciencePurchaseWindowsRank8[i] = &theWins[19 + i];
	for (int i = 0; i < MAX_SPECIAL_POWER_SHORTCUTS; ++i)
	{
		cb.m_specialPowerShortcutButtons[i] = buttonsNull[i] ? NULL : &theWins[23 + i];
		cb.m_specialPowerShortcutButtonParents[i] = parentsNull[i] ? NULL : &theWins[34 + i];
		cb.m_shortcutDisplayStrings[i] = NULL;
		if (!buttonsNull[i] && shortcutData[i] >= 0) theWins[23 + i].data = &theButtons[shortcutData[i]];
	}
	cb.m_currentlyUsedSpecialPowersButtons = nUsed;
	cb.m_specialPowerShortcutParent = shortcutParent ? &theWins[46] : NULL;
	theAnim.finished = animFinished;
	cb.m_animateWindowManagerForGenShortcuts = &theAnim;
	for (int i = 0; i < MAX_COMMANDS_PER_SET; ++i) cb.m_commandWindows[i] = &theWins[47 + i];
	cb.m_contextParent[CP_MASTER] = master ? &theWins[45] : NULL;
	cb.m_contextParent[CP_PURCHASE_SCIENCE] = ctxPurchase ? &theWins[70] : NULL;
	cb.m_communicatorButton = communicator ? &theWins[65] : NULL;
	/* the player's template: the set names */
	char nm[16];
	AsciiString *names[4] = { &thePt.r1, &thePt.r3, &thePt.r8, &thePt.sc };
	int sel[4] = { pn[0], pn[1], pn[2], sn };
	for (int i = 0; i < 4; ++i) { if (sel[i] < 0) *names[i] = AsciiString( "" ); else { snprintf( nm, sizeof( nm ), "Set%d", sel[i] ); *names[i] = AsciiString( nm ); } }
	Player *player = playerExists ? &thePlayer : NULL;
	for (int o = 0; o + 1 < nops; o += 2)
	{
		int kind = (int)ops[o], a = (int)ops[o + 1];
		switch (kind)
		{
			case 0: cb.populatePurchaseScience( player ); break;
			case 1: cb.updateContextPurchaseScience(); break;
			case 2: cb.populateSpecialPowerShortcut( player ); break;
			case 3: logf( "has %d", (int)cb.hasAnyShortcutSelection() ); break;
			case 4: cb.updateSpecialPowerShortcut(); break;
			case 5: cb.drawSpecialPowerShortcutMultiplierText(); break;
			case 6: cb.hideCommunicator( a != 0 ); break;
			case 7: { ControlBar::SchemeManagerStandIn mgr; cb.m_controlBarSchemeManager = a ? &mgr : NULL; cb.setControlBarSchemeByName( AsciiString( "Scheme" ) ); break; }
		}
	}
	for (int i = 0; i < N_WIN; ++i)
		logf( "state %d hidden %d status %x data %s", i, (int)theWins[i].hidden, theWins[i].status, theWins[i].data ? ((CommandButton *)theWins[i].data)->name.str() : "-" );
	if ((int)theLog.size() + 1 > logcap) return -1;
	memcpy( log, theLog.c_str(), theLog.size() + 1 );
	return (int)theLog.size();
}
