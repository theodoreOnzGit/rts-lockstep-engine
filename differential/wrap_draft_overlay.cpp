/* Differential-test wrapper for the GameSpy overlay draft (#215, group net4, upstream commit 0a05454d), src/drafts/network/game_spy_overlay.rs, cut verbatim by
** build.rs (drafts_net4) into OUT_DIR/draft_net4_gen:
**   ov_enum.inc  Include/GameNetwork/GameSpyOverlay.h, enum GSOverlayType;
**   ov_cpp.inc   Source/GameNetwork/GameSpyOverlay.cpp from `void deleteNotificationBox( void );` to the end of the file (the message-box tracker, the nine overlay
**                screens, ReOpenPlayerInfo, CheckReOpenPlayerInfo).
** Built on upstream's own UnicodeString.cpp and AsciiString.cpp (unchanged; -fshort-wchar; classes renamed by build.rs).
** Stand-ins (outside the cut; each logs what it is asked, as tests/differential_draft_overlay.rs's host does): GameWindow (winBringToTop), the window manager
**   (winCreateLayout, winDestroy), WindowLayout (runInit, hide, bringForward, runShutdown, destroyWindows, deleteInstance, runUpdate; one object per overlay,
**   found by the .wnd name), MessageBoxOk/OkCancel/YesNo (they remember the tracker's two callbacks; the program presses them; the window they return is null
**   when the case says so), TheGameText (the text is the label widened), TheGameSpyBuddyMessageQueue (isConnected, getLocalProfileID from the case; addRequest
**   logs the request type), BuddyRequest (the type and the RELOGIN constant), AudioEventRTS and TheAudio, SignalUIInteraction, deleteNotificationBox;
**   four user callbacks cb0..cb3 (they log). DEBUG_* are nothing.
** Entry point (extern "C", tests/differential_draft_overlay.rs): ref_ov_run(in, n, out, cap), a program, see run(). */
#include "Common/STLTypedefs.h"
#include "Common/CriticalSection.h"
#include <string.h>
#include <stdio.h>
#include <vector>
#include <string>

/* the allocator and the string critical section are defined once, in wrap_draft_ping.cpp (the wrappers share one library) */
extern DynamicMemoryAllocator *TheDynamicMemoryAllocator;
extern CriticalSection *TheUnicodeStringCriticalSection;

#define NULL 0
#define DEBUG_LOG(x) ((void)0)
#define DEBUG_CRASH(x) ((void)0)
#define DEBUG_ASSERTCRASH(c, x) ((void)0)

typedef std::vector<unsigned> Words;
static Words theLog;
static void logStr(const char *s) { size_t n = strlen(s); theLog.push_back((unsigned)n); for (size_t i = 0; i < n; ++i) theLog.push_back((unsigned char)s[i]); }
static void logUnits(const WideChar *s) { unsigned n = 0; while (s[n]) ++n; theLog.push_back(n); for (unsigned i = 0; i < n; ++i) theLog.push_back((unsigned)s[i]); }

typedef void (*GameWinMsgBoxFunc)(void);
#include "ov_enum.inc"
void ClearGSMessageBoxes(void);
void GSMessageBoxOk(UnicodeString title, UnicodeString message, GameWinMsgBoxFunc newOkFunc);
void GSMessageBoxOkCancel(UnicodeString title, UnicodeString message, GameWinMsgBoxFunc newOkFunc, GameWinMsgBoxFunc newCancelFunc);
void GSMessageBoxYesNo(UnicodeString title, UnicodeString message, GameWinMsgBoxFunc newYesFunc, GameWinMsgBoxFunc newNoFunc);
void RaiseGSMessageBox(void);
void GameSpyOpenOverlay(GSOverlayType);
void GameSpyCloseOverlay(GSOverlayType);
void GameSpyCloseAllOverlays(void);
Bool GameSpyIsOverlayOpen(GSOverlayType);
void GameSpyToggleOverlay(GSOverlayType);
void GameSpyUpdateOverlays(void);
void ReOpenPlayerInfo(void);
void CheckReOpenPlayerInfo(void);

static const char *kNames[9] = { "Menus/PopupPlayerInfo.wnd", "Menus/WOLMapSelectMenu.wnd", "Menus/WOLBuddyOverlay.wnd", "Menus/WOLPageOverlay.wnd", "Menus/PopupHostGame.wnd",
	"Menus/PopupJoinGame.wnd", "Menus/PopupLadderSelect.wnd", "Menus/PopupLocaleSelect.wnd", "Menus/OptionsMenu.wnd" };
struct GameWindow { void winBringToTop() { theLog.push_back(19); } };
static GameWindow theBoxWindow;
struct WindowLayout
{
	int id;
	void runInit() { theLog.push_back(10); theLog.push_back((unsigned)id); }
	void hide(Bool h) { theLog.push_back(11); theLog.push_back((unsigned)id); theLog.push_back(h ? 1u : 0u); }
	void bringForward() { theLog.push_back(12); theLog.push_back((unsigned)id); }
	void runShutdown() { theLog.push_back(15); theLog.push_back((unsigned)id); }
	void destroyWindows() { theLog.push_back(16); theLog.push_back((unsigned)id); }
	void deleteInstance() { theLog.push_back(17); theLog.push_back((unsigned)id); }
	void runUpdate() { theLog.push_back(18); theLog.push_back((unsigned)id); }
};
static WindowLayout theLayouts[9];
struct WindowManagerStandIn
{
	WindowLayout *winCreateLayout(AsciiString name)
	{
		theLog.push_back(13); logStr(name.str());
		for (int i = 0; i < 9; ++i) if (strcmp(name.str(), kNames[i]) == 0) { theLayouts[i].id = i; return &theLayouts[i]; }
		return &theLayouts[0];
	}
	void winDestroy(GameWindow *) { theLog.push_back(14); }
};
static WindowManagerStandIn theWinMgrObj;
static WindowManagerStandIn *TheWindowManager = &theWinMgrObj;

static GameWinMsgBoxFunc g_boxOk = 0, g_boxCancel = 0;
static int g_boxNull = 0;
static GameWindow *box(unsigned kind, UnicodeString t, UnicodeString x, GameWinMsgBoxFunc ok, GameWinMsgBoxFunc cancel)
{
	theLog.push_back(20); theLog.push_back(kind); logUnits(t.str()); logUnits(x.str());
	g_boxOk = ok; g_boxCancel = cancel;
	return g_boxNull ? 0 : &theBoxWindow;
}
static GameWindow *MessageBoxOk(UnicodeString t, UnicodeString x, GameWinMsgBoxFunc ok) { return box(0, t, x, ok, 0); }
static GameWindow *MessageBoxOkCancel(UnicodeString t, UnicodeString x, GameWinMsgBoxFunc ok, GameWinMsgBoxFunc cancel) { return box(1, t, x, ok, cancel); }
static GameWindow *MessageBoxYesNo(UnicodeString t, UnicodeString x, GameWinMsgBoxFunc yes, GameWinMsgBoxFunc no) { return box(2, t, x, yes, no); }
struct GameTextStandIn
{
	UnicodeString fetch(const char *label)
	{
		WideChar w[80];
		int n = 0;
		for (const char *c = label; *c && n < 78; ++c) w[n++] = (WideChar)(unsigned char)*c;
		w[n] = 0;
		UnicodeString u;
		u.set(w);
		return u;
	}
};
static GameTextStandIn theTextObj;
static GameTextStandIn *TheGameText = &theTextObj;
struct BuddyRequest { enum { BUDDYREQUEST_LOGIN, BUDDYREQUEST_RELOGIN } buddyRequestType; };
static int g_buddyConnected = 0, g_buddyProfile = 0;
struct BuddyQueueStandIn
{
	Bool isConnected() { return g_buddyConnected != 0; }
	int getLocalProfileID() { return g_buddyProfile; }
	void addRequest(const BuddyRequest &r) { theLog.push_back(24); theLog.push_back((unsigned)r.buddyRequestType); }
};
static BuddyQueueStandIn theBuddyObj;
static BuddyQueueStandIn *TheGameSpyBuddyMessageQueue = &theBuddyObj;
struct AudioEventRTS { AudioEventRTS(const char *) {} };
struct AudioStandIn { void addAudioEvent(AudioEventRTS *) { theLog.push_back(21); } };
static AudioStandIn theAudioObj;
static AudioStandIn *TheAudio = &theAudioObj;
enum { SHELL_SCRIPT_HOOK_OPTIONS_CLOSED = 1 };
static void SignalUIInteraction(int) { theLog.push_back(22); }
void deleteNotificationBox(void) { theLog.push_back(23); }
static void cb0() { theLog.push_back(25); theLog.push_back(0); }
static void cb1() { theLog.push_back(25); theLog.push_back(1); }
static void cb2() { theLog.push_back(25); theLog.push_back(2); }
static void cb3() { theLog.push_back(25); theLog.push_back(3); }
static GameWinMsgBoxFunc CBS[4] = { cb0, cb1, cb2, cb3 };

#include "ov_cpp.inc"

static unsigned idOf(GameWinMsgBoxFunc f)
{
	if (!f) return 0xFFFFFFFEu;
	if (f == buddyTryReconnect) return 0xFFFFFFFFu;
	for (unsigned i = 0; i < 4; ++i) if (f == CBS[i]) return i;
	return 0xFFFFFFFDu;
}
static GameWinMsgBoxFunc fnOf(unsigned id) { return id >= 4 ? (GameWinMsgBoxFunc)0 : CBS[id]; }
static std::string getStr(const unsigned *&p)
{
	unsigned n = *p++;
	std::string s;
	for (unsigned i = 0; i < n; ++i) s.push_back((char)*p++);
	return s;
}
static UnicodeString getUnits(const unsigned *&p)
{
	unsigned n = *p++;
	std::vector<WideChar> v;
	for (unsigned i = 0; i < n; ++i) v.push_back((WideChar)*p++);
	v.push_back(0);
	UnicodeString u;
	u.set(&v[0]);
	return u;
}
static void dumpState()
{
	theLog.push_back(0xD0D0D0D0u);
	theLog.push_back(messageBoxWindow ? 1u : 0u);
	theLog.push_back(idOf(okFunc));
	theLog.push_back(idOf(cancelFunc));
	theLog.push_back(reOpenPlayerInfoFlag ? 1u : 0u);
	for (int i = 0; i < 9; ++i) theLog.push_back(overlayLayouts[i] ? 1u : 0u);
}

/* A program: [number of operations], then per operation its opcode and arguments (a callback is 0..3, or 4 for none; an overlay is 0..8; units as length + words).
**  0 ClearGSMessageBoxes   1 GSMessageBoxOk(title, message, cb)   2 GSMessageBoxOkCancel(t, m, ok, cancel)   3 GSMessageBoxYesNo(t, m, yes, no)   4 RaiseGSMessageBox
**  5 GameSpyOpenOverlay(o)   6 GameSpyCloseOverlay(o)   7 GameSpyIsOverlayOpen(o)   8 GameSpyToggleOverlay(o)   9 GameSpyCloseAllOverlays   10 GameSpyUpdateOverlays
**  11 ReOpenPlayerInfo   12 CheckReOpenPlayerInfo   13 press the box's OK (messageBoxOK, if a box is up)   14 press the box's Cancel
**  15 configure(buddy connected, buddy profile id, box window null) */
extern "C" int ref_ov_run(const unsigned *in, int n, unsigned *out, int cap)
{
	(void)n;
	const unsigned *p = in;
	theLog.clear();
	for (int i = 0; i < 9; ++i) overlayLayouts[i] = NULL;
	messageBoxWindow = NULL; okFunc = NULL; cancelFunc = NULL; reOpenPlayerInfoFlag = FALSE;
	g_boxOk = 0; g_boxCancel = 0; g_boxNull = 0; g_buddyConnected = 0; g_buddyProfile = 0;
	unsigned ops = *p++;
	for (unsigned o = 0; o < ops; ++o)
	{
		unsigned op = *p++;
		theLog.push_back(0xA5A5A5A5u);
		theLog.push_back(op);
		switch (op)
		{
		case 0: ClearGSMessageBoxes(); break;
		case 1: { UnicodeString t = getUnits(p), m = getUnits(p); GSMessageBoxOk(t, m, fnOf(*p++)); break; }
		case 2: { UnicodeString t = getUnits(p), m = getUnits(p); unsigned a = *p++, b = *p++; GSMessageBoxOkCancel(t, m, fnOf(a), fnOf(b)); break; }
		case 3: { UnicodeString t = getUnits(p), m = getUnits(p); unsigned a = *p++, b = *p++; GSMessageBoxYesNo(t, m, fnOf(a), fnOf(b)); break; }
		case 4: RaiseGSMessageBox(); break;
		case 5: GameSpyOpenOverlay((GSOverlayType)*p++); break;
		case 6: GameSpyCloseOverlay((GSOverlayType)*p++); break;
		case 7: theLog.push_back(GameSpyIsOverlayOpen((GSOverlayType)*p++) ? 1u : 0u); break;
		case 8: GameSpyToggleOverlay((GSOverlayType)*p++); break;
		case 9: GameSpyCloseAllOverlays(); break;
		case 10: GameSpyUpdateOverlays(); break;
		case 11: ReOpenPlayerInfo(); break;
		case 12: CheckReOpenPlayerInfo(); break;
		case 13: if (g_boxOk) { GameWinMsgBoxFunc f = g_boxOk; theLog.push_back(71); theLog.push_back(0); f(); } break;
		case 14: if (g_boxCancel) { GameWinMsgBoxFunc f = g_boxCancel; theLog.push_back(71); theLog.push_back(1); f(); } break;
		case 15: g_buddyConnected = (int)*p++; g_buddyProfile = (int)*p++; g_boxNull = (int)*p++; break;
		}
		dumpState();
	}
	if ((int)theLog.size() > cap) return -1;
	for (size_t i = 0; i < theLog.size(); ++i) out[i] = theLog[i];
	return (int)theLog.size();
}
