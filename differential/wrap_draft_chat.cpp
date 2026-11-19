/* Differential-test wrapper for the GameSpy chat draft (#215, upstream commit 0a05454d), src/drafts/sweep_gui_logic/chat.rs
** (game_spy_color_defaults, GAME_SPY_COLOR_FIELDS, send_chat, add_chat_by_nick, add_chat, add_text, register_text_window,
** unregister_text_window), cut verbatim by build.rs (drafts_guilogic3) into:
**   chat_enum.inc    GameNetwork/GameSpy/PeerDefs.h, `enum GameSpyColors` (to `extern Color GameSpyColor`);
**   chat_color.inc   GameClient/Color.h, `inline Color GameMakeColor` (to `extern void GameGetColorComponents`);
**   chat_fields.inc  GameNetwork/GameSpy/Chat.cpp, `#define OFFSET(x)` and `GameSpyColorFieldParse` (to `INI::parseOnlineChatColorDefinition`);
**   chat_table.inc   Chat.cpp, `Color GameSpyColor[GSCOLOR_MAX]` (to GameSpyInfo::sendChat);
**   chat_fns.inc     Chat.cpp, GameSpyInfo::sendChat to the end (both addChat, addText, registerTextWindow, unregisterTextWindow).
** Built on upstream's own UnicodeString.cpp and AsciiString.cpp (unchanged; -fshort-wchar; classes renamed by build.rs;
** shim_draft_g3/g3_prelude.h adds `%ls` to the wide printf stand-in).
** Stand-ins (outside the cuts): `GameSpyInfo` with the members and the virtual accessors the cuts call, as PeerDefsImplementation.h
** declares them (the real one is the whole GameSpy state): getCurrentGroupRoom (a flag), getLocalName (a name), isSavedIgnored (a bit mask by
** profile id 0..31), isIgnored (a name starting with `ig`, compared as stricmp), getBuddyMap()->find (a bit mask by profile id), m_disallowAsainText,
** m_disallowNonAsianText, m_textWindows (a std::set of window pointers, ordered by address: the test's windows are an array, so by id),
** getPlayerInfoMap() (a std::map with the real AsciiComparator, PeerDefs.cpp:59: stricmp < 0); `PlayerInfo` (m_name, m_profileID, m_flags);
** `PEER_FLAG_OP` (the peer SDK's flag: here 0x10); `PeerRequest` (text, nick, message.isAction, peerRequestType, MESSAGEROOM / MESSAGEPLAYER) and
** TheGameSpyPeerMessageQueue->addRequest (logs); `GameWindow` and the GadgetListBox calls (GetListLength, GetSelected: the selected rows and then
** -1, as the real one fills; GetText, which returns the row's text only for the last column; GetNumColumns; AddEntryText, which returns the next row
** index; SetItemData): all log; `AudioEventRTS` (a name) and TheAudio->addAudioEvent (logs); TheLanguageFilter->filterLine (replaces every
** `x` by `*`: the real filter is a different draft); TheGameSpyGame (isInGame, isGameInProgress: one flag); TheInGameUI->message(UnicodeString, ...)
** (logs the format as it is; the real one formats); INI::parseColorInt (a function); FieldParse (name, parse, userData, offset). DEBUG_*: none.
**
** Entry points (extern "C", tests/differential_draft_chat.rs):
**   ref_chat_colors(out): the 27 GameSpyColor values;   ref_chat_fields(i, name, off): field i's name and offset / sizeof(Int); returns 0 at the end marker;
**   ref_chat_run(in, n, out, cap): a program, see the test; out = the log, with each call's result. */
#include "Common/STLTypedefs.h"
#include "Common/CriticalSection.h"
#include <string.h>
#include <vector>
#include <set>
#include <map>
#include <string>

namespace {
DynamicMemoryAllocator theChatAllocator;
}
DynamicMemoryAllocator *TheDynamicMemoryAllocator = &theChatAllocator;
CriticalSection *TheUnicodeStringCriticalSection = 0;

#define NULL 0
#define PEER_FLAG_OP 0x10
#define DEBUG_ASSERTCRASH(c, m) ((void)0)
typedef Int Color;

#include "chat_enum.inc"
#include "chat_color.inc"

struct FieldParse { const char *name; void (*parse)(int); const void *userData; unsigned long offset; };
struct INI { static void parseColorInt(int) {} };
#include "chat_fields.inc"
#undef OFFSET
#include "chat_table.inc"

/* ---- the world the chat functions ask of ---- */
static std::vector<int> theLog;
static void logUnits(const WideChar *s) { Int n = 0; while (s[n]) ++n; theLog.push_back(n); for (Int i = 0; i < n; ++i) theLog.push_back(s[i]); }
static void logBytes(const char *s) { Int n = (Int)strlen(s); theLog.push_back(n); for (Int i = 0; i < n; ++i) theLog.push_back((unsigned char)s[i]); }

class GameWindow
{
public:
	int id;
	std::vector<UnicodeString> rows;   /* the last column's text of each row */
	int ncols;
	std::vector<Int> selected;
	int added;
};
static GameWindow theWins[8];
static const int NWINS = 8;

static Int GadgetListBoxGetListLength(GameWindow *w) { return (Int)w->rows.size(); }
static void GadgetListBoxGetSelected(GameWindow *w, Int *sel)
{
	static std::vector<Int> a;
	a = w->selected;
	a.push_back(-1);
	*(Int **)sel = &a[0];
}
static Int GadgetListBoxGetNumColumns(GameWindow *w) { return w->ncols; }
static UnicodeString GadgetListBoxGetText(GameWindow *w, Int row, Int col)
{
	if (col == w->ncols - 1 && row >= 0 && row < (Int)w->rows.size()) return w->rows[row];
	return UnicodeString(L"?");
}
static Int GadgetListBoxAddEntryText(GameWindow *w, UnicodeString message, Color c, Int a, Int b)
{
	theLog.push_back(5); theLog.push_back(w->id); theLog.push_back(c); logUnits(message.str());
	return w->added++;
}
static void GadgetListBoxSetItemData(GameWindow *w, void *data, Int index, Int column = 0)
{
	if (!w) return; /* GadgetListBox.cpp:2685: `if (listbox)` */
	theLog.push_back(6); theLog.push_back(w->id); theLog.push_back((Int)(long)data); theLog.push_back(index);
}

struct AudioEventRTS { AudioEventRTS(const char *n) : name(n) {} const char *name; };
struct AudioStandIn { void addAudioEvent(AudioEventRTS *e) { theLog.push_back(3); logBytes(e->name); } };
static AudioStandIn theAudioObj;
static AudioStandIn *TheAudio = &theAudioObj;
struct LanguageFilterStandIn { void filterLine(UnicodeString &s) { UnicodeString r; for (Int i = 0; i < s.getLength(); ++i) r.concat(s.getCharAt(i) == 'x' ? (WideChar)'*' : s.getCharAt(i)); s = r; } };
static LanguageFilterStandIn theFilter;
static LanguageFilterStandIn *TheLanguageFilter = &theFilter;
static bool gameInProgress;
struct GameSpyGameStandIn { bool isInGame() { return gameInProgress; } bool isGameInProgress() { return gameInProgress; } };
static GameSpyGameStandIn theGame;
static GameSpyGameStandIn *TheGameSpyGame = &theGame;
struct InGameUIStandIn { void message(UnicodeString format, ...) { theLog.push_back(4); logUnits(format.str()); } };
static InGameUIStandIn theUI;
static InGameUIStandIn *TheInGameUI = &theUI;

class PlayerInfo { public: PlayerInfo() { m_profileID = m_flags = 0; } AsciiString m_name; Int m_profileID; Int m_flags; };
struct AsciiComparator { bool operator()(AsciiString s1, AsciiString s2) const { return stricmp(s1.str(), s2.str()) < 0; } };
typedef std::map<AsciiString, PlayerInfo, AsciiComparator> PlayerInfoMap;
struct BuddyStandIn { unsigned mask; };
struct PeerRequest
{
	enum { PEERREQUEST_MESSAGEROOM = 7, PEERREQUEST_MESSAGEPLAYER = 8 };
	int peerRequestType;
	std::string nick;
	struct WText { std::vector<WideChar> v; WText &operator=(const WideChar *s) { v.clear(); for (; *s; ++s) v.push_back(*s); return *this; } size_t size() const { return v.size(); } WideChar operator[](size_t i) const { return v[i]; } } text; /* the real field is a std::wstring: with -fshort-wchar libstdc++'s wide traits would read 32-bit units */
	struct { Bool isAction; } message;
};
struct MessageQueueStandIn
{
	void addRequest(const PeerRequest &r)
	{
		theLog.push_back(1); theLog.push_back(r.peerRequestType == PeerRequest::PEERREQUEST_MESSAGEROOM ? 1 : 2); theLog.push_back(r.message.isAction ? 1 : 0);
		theLog.push_back((Int)r.text.size()); for (size_t i = 0; i < r.text.size(); ++i) theLog.push_back(r.text[i]);
		if (r.peerRequestType == PeerRequest::PEERREQUEST_MESSAGEPLAYER) logBytes(r.nick.c_str());
	}
};
static MessageQueueStandIn theQueue;
static MessageQueueStandIn *TheGameSpyPeerMessageQueue = &theQueue;
enum RoomType { StagingRoom, GroupRoom };

struct BuddyMapStandIn { unsigned mask; struct It { int v; bool operator!=(const It &o) const { return v != o.v; } }; It find(Int id) const { It i; i.v = (id >= 0 && id < 32 && ((mask >> id) & 1)) ? 1 : 0; return i; } It end() const { It i; i.v = 0; return i; } };

class GameSpyInfo
{
public:
	Int getCurrentGroupRoom() { return m_groupRoom; }
	PlayerInfoMap *getPlayerInfoMap() { return &m_playerInfoMap; }
	AsciiString getLocalName() { return m_localName; }
	BuddyMapStandIn *getBuddyMap() { return &m_buddy; }
	Bool isSavedIgnored( Int profileID ) { return profileID >= 0 && profileID < 32 && ((m_savedMask >> profileID) & 1); }
	Bool isIgnored( AsciiString nick ) { return strnicmp(nick.str(), "ig", 2) == 0; }
	Int addText( UnicodeString message, Color c, GameWindow *win );
	void addChat( PlayerInfo p, UnicodeString msg, Bool isPublic, Bool isAction, GameWindow *win );
	void addChat( AsciiString nick, Int profileID, UnicodeString msg, Bool isPublic, Bool isAction, GameWindow *win );
	Bool sendChat( UnicodeString message, Bool isAction, GameWindow *playerListbox );
	void registerTextWindow( GameWindow *win );
	void unregisterTextWindow( GameWindow *win );
	Int m_groupRoom;
	AsciiString m_localName;
	unsigned m_savedMask;
	BuddyMapStandIn m_buddy;
	Bool m_disallowAsainText;
	Bool m_disallowNonAsianText;
	std::set<GameWindow *> m_textWindows;
	PlayerInfoMap m_playerInfoMap;
};
static GameSpyInfo *TheGameSpyInfo;

#include "chat_fns.inc"

extern "C" int ref_chat_colors(int *out) { for (int i = 0; i < GSCOLOR_MAX; ++i) out[i] = GameSpyColor[i]; return GSCOLOR_MAX; }
extern "C" int ref_chat_fields(int i, char *name, int *off)
{
	if (GameSpyColorFieldParse[i].name == NULL) return 0;
	strcpy(name, GameSpyColorFieldParse[i].name);
	*off = (int)(GameSpyColorFieldParse[i].offset / sizeof(Int));
	return 1;
}

/* Program (ints): ops, each with its operands; strings are (n, units..).
**   0 groupRoom disallowAsian disallowNonAsian savedMask buddyMask gameInProgress localName(str)   configure (fresh GameSpyInfo, windows cleared)
**   1 win nrows ncols (row text str)..  selectedCount (row)..        set a window's list
**   2 name(str) profile flags                                        add a player to the info map
**   3 message(str) isAction listboxWin (0 none)                      sendChat -> log [0, result]
**   4 nick(str) profile msg(str) isPublic isAction win (0 none)      addChat by nick
**   5 name(str) profile flags msg(str) isPublic isAction win         addChat(PlayerInfo)
**   6 msg(str) color win                                             addText -> log [7, index]
**   7 win: registerTextWindow      8 win: unregisterTextWindow */
static UnicodeString readStr(const int *in, int &at)
{
	int n = in[at++];
	UnicodeString s;
	for (int i = 0; i < n; ++i) s.concat((WideChar)in[at++]);
	return s;
}
static AsciiString asciiOf(const UnicodeString &u) { AsciiString a; for (Int i = 0; i < u.getLength(); ++i) a.concat((char)u.getCharAt(i)); return a; }
static GameWindow *winOf(int w) { return w == 0 ? NULL : &theWins[w - 1]; }

extern "C" int ref_chat_run(const int *in, int nin, int *out, int cap)
{
	TheDynamicMemoryAllocator = &theChatAllocator;
	theLog.clear();
	static GameSpyInfo info;
	TheGameSpyInfo = &info;
	int at = 0;
	while (at < nin)
	{
		int op = in[at++];
		switch (op)
		{
			case 0:
			{
				info.m_textWindows.clear(); info.m_playerInfoMap.clear();
				info.m_groupRoom = in[at++]; info.m_disallowAsainText = in[at++]; info.m_disallowNonAsianText = in[at++];
				info.m_savedMask = (unsigned)in[at++]; info.m_buddy.mask = (unsigned)in[at++]; gameInProgress = in[at++] != 0;
				info.m_localName = asciiOf(readStr(in, at));
				for (int i = 0; i < NWINS; ++i) { theWins[i].id = i + 1; theWins[i].rows.clear(); theWins[i].selected.clear(); theWins[i].ncols = 1; theWins[i].added = 0; }
				break;
			}
			case 1:
			{
				GameWindow *w = winOf(in[at++]);
				int nr = in[at++]; w->ncols = in[at++];
				w->rows.clear(); for (int i = 0; i < nr; ++i) w->rows.push_back(readStr(in, at));
				int ns = in[at++]; w->selected.clear(); for (int i = 0; i < ns; ++i) w->selected.push_back(in[at++]);
				break;
			}
			case 2: { PlayerInfo p; p.m_name = asciiOf(readStr(in, at)); p.m_profileID = in[at++]; p.m_flags = in[at++]; info.m_playerInfoMap[p.m_name] = p; break; }
			case 3: { UnicodeString m = readStr(in, at); int act = in[at++]; GameWindow *w = winOf(in[at++]); Bool r = info.sendChat(m, act != 0, w); theLog.push_back(0); theLog.push_back(r ? 1 : 0); break; }
			case 4: { AsciiString nick = asciiOf(readStr(in, at)); int prof = in[at++]; UnicodeString m = readStr(in, at); int pub = in[at++], act = in[at++]; info.addChat(nick, prof, m, pub != 0, act != 0, winOf(in[at++])); break; }
			case 5: { PlayerInfo p; p.m_name = asciiOf(readStr(in, at)); p.m_profileID = in[at++]; p.m_flags = in[at++]; UnicodeString m = readStr(in, at); int pub = in[at++], act = in[at++]; info.addChat(p, m, pub != 0, act != 0, winOf(in[at++])); break; }
			case 6: { UnicodeString m = readStr(in, at); int c = in[at++]; Int idx = info.addText(m, c, winOf(in[at++])); theLog.push_back(7); theLog.push_back(idx); break; }
			case 7: info.registerTextWindow(winOf(in[at++])); break;
			default: info.unregisterTextWindow(winOf(in[at++])); break;
		}
	}
	if ((int)theLog.size() > cap) return -1;
	for (size_t i = 0; i < theLog.size(); ++i) out[i] = theLog[i];
	return (int)theLog.size();
}
