/* The reference PROGRAM for src/drafts/ui/popup_join_game.rs (#215, group ui4, upstream commit 0a05454d): upstream's PopupJoinGame.cpp on upstream's OWN window
** manager and gadgets (see differential/wrap_draft_ui4_disconnect.cpp for how the program is built; the shared stand-ins are in ui4_host.h and ui4_main.h).
** The path is `UI4_JOIN_EXE`.
**
** Upstream text, cut VERBATIM by build.rs (anchor asserted once): popup_join_game.inc, GUICallbacks/Menus/PopupJoinGame.cpp from `static NameKeyType parentPopupID`
** to the end, ONE edit: `munkee.translate(txtInput);` becomes `ui4_translate(munkee, txtInput);` (the shim's AsciiString::translate appends the truncated unit
** as a char, a NUL included, where upstream's `concat((char)c)` appends nothing for a NUL: AsciiString.h:536 builds `{c, 0}` and concats that string).
**
** Stand-ins of this file:
**   - NAMEKEY(name): `TheNameKeyGenerator->nameToKey(name)` (NameKeyGenerator.h's macro).
**   - TheGameSpyInfo (GameSpyInfo): the case's staging room (`@ jn room <exists> <id> <text>`): `getCurrentStagingRoomID()` is that id, `findStagingRoomByID(id)` is
**     the room when it exists and the id is the current one; GameSpyStagingRoom's `getGameName()` and `getID()`.
**   - GameSpyCloseOverlay(GSOVERLAY_GAMEPASSWORD), SetLobbyAttemptHostJoin(b): log `overlay close 1`, `lobby attemptjoin <b>`.
**   - PeerRequest and TheGameSpyPeerMessageQueue->addRequest: log `peer join <id> <room text> pw [<password bytes in hex>]`.
**
** Operations (besides ui4_main.h's): `@ jn init` (PopupJoinGameInit), `@ jn room <exists> <id> <text>`. The dump adds the file's statics. */
#include "ui4_wm_base.inc"
#include "ui4_host.h"
#include "GameClient/KeyDefs.h"

/* the shim's AsciiString::translate, with upstream's treatment of a unit whose low byte is 0 */
static void ui4_translate(AsciiString &out, const UnicodeString &src)
{
	out.clear();
	for (Int i = 0; i < src.getLength(); ++i)
	{
		char c = (char)src.getCharAt(i);
		if (c != 0) out.concat(c);
	}
}
#define NAMEKEY(name) TheNameKeyGenerator->nameToKey(name)
enum { GSOVERLAY_GAMEPASSWORD = 1 };
static void GameSpyCloseOverlay(int o) { g_log << "overlay close " << o << "\n"; }
static void SetLobbyAttemptHostJoin(Bool b) { g_log << "lobby attemptjoin " << (int)b << "\n"; }
class GameSpyStagingRoom
{
public:
	Int id;
	UnicodeString name;
	UnicodeString getGameName() { return name; }
	Int getID() { return id; }
};
static GameSpyStagingRoom u4_room;
static bool u4_room_exists = false;
class U4GameSpyInfo
{
public:
	Int getCurrentStagingRoomID() { return u4_room.id; }
	GameSpyStagingRoom *findStagingRoomByID(Int id) { return (u4_room_exists && id == u4_room.id) ? &u4_room : NULL; }
};
static U4GameSpyInfo u4_gsi;
static U4GameSpyInfo *TheGameSpyInfo = &u4_gsi;
struct PeerRequest
{
	enum PeerRequestType { PEERREQUEST_JOINSTAGINGROOM };
	PeerRequestType peerRequestType;
	UnicodeString text;
	struct { Int id; } stagingRoom;
	AsciiString password;
};
class U4Queue
{
public:
	void addRequest(PeerRequest &r)
	{
		g_log << "peer join " << r.stagingRoom.id;
		dump_text(g_log, r.text);
		g_log << " pw [";
		for (const char *c = r.password.str(); *c; ++c) g_log << (c == r.password.str() ? "" : ",") << std::hex << (unsigned)(unsigned char)*c << std::dec;
		g_log << "]\n";
	}
};
static U4Queue u4_queue;
static U4Queue *TheGameSpyPeerMessageQueue = &u4_queue;

#include "popup_join_game.inc"

static const U4Menu u4_menus[] = {
	{ "join", PopupJoinGameSystem, PopupJoinGameInput },
};
static void u4_menu_setup(void) {}
static bool u4_menu_op(TestWindowManager *wm, const std::string &cmd, std::istringstream &in)
{
	if (cmd != "jn") return false;
	std::string f;
	in >> f;
	if (f == "init") PopupJoinGameInit(NULL, NULL);
	else if (f == "room")
	{
		int e, id;
		std::string t;
		in >> e >> id >> t;
		u4_room_exists = e != 0;
		u4_room.id = id;
		u4_room.name = u4_text(t);
	}
	return true;
}
static void u4_menu_dump(std::ostringstream &o)
{
	o << "jn parent " << widx(parentPopup) << " te " << widx(textEntryGamePassword) << " ids " << (parentPopupID != NAMEKEY_INVALID) << (textEntryGamePasswordID != NAMEKEY_INVALID) << (buttonCancelID != NAMEKEY_INVALID) << "\n";
}
#include "ui4_main.h"
