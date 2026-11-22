/* The stand-ins and the cut of group wol8's reference program for the lobby's right-click menu (#215, #232): `case GLM_RIGHT_CLICKED:` of WOLLobbyMenuSystem
** (WOLLobbyMenu.cpp:1719, both its branches: the player list's and the game list's), cut VERBATIM by build.rs `drafts_wol8` (lobby_rc.inc, no edit) into `w8_rc_system`, which
** is that function's `switch (msg)` reduced to the one case. Included by the program differential/wrap_draft_wol8_lobby.cpp (= wrap_draft_wol7_lobby.cpp, edited by build.rs),
** after the GameSpy stand-ins of group wol7. What the cut calls outside itself:
**   - `listboxLobbyPlayersID`, `GetGameListBoxID()`: the ids the case sets (`@ w8 plid`, `@ w8 gid`).
**   - TheGameSpyInfo (wol7's class, the three members added by build.rs's edit): `getLocalProfileID()` (the case's), `isBuddy(id)` (wol7's buddy set), `getStagingRoomList()` (the
**     case's rooms; a room can be NULL).
**   - GameSpyStagingRoom: `getLadderIP()` and `getLadderPort()`; TheLadderList->findLadder(ip, port): the case's ladders.
**   - `setUnignoreText(layout, nick, id)`: logs `unignore <layout> <nick as hex bytes> <id>` (a window-text edit in the game, WOLLobbyMenu.cpp, not part of the cut).
**   - GameSpyRCMenuData and RCItemType: cut verbatim from PeerDefs.h (peer_rc.inc). `NEW` is the shim's.
**   - TheWindowManager->winCreateLayout: ui4_host.h's (logs `layout <file>`, a layout of the windows of the last `@ lay`); TheDisplay: 800 x 600.
** The window user data the cut sets is dumped by the case operation `@ w8 rc` (a RC menu: `rcdata <window> <id> <nick hex> <type>`; the game list's: `ud <window> <id>`). */
static void setUnignoreText(WindowLayout *layout, AsciiString nick, GPProfile id)
{
	g_log << "unignore " << u4_lay_index(layout) << " " << w8_hex(nick.str()) << " " << id << "\n";
}
static WindowMsgHandledType w8_rc_system(GameWindow *window, UnsignedInt msg, WindowMsgData mData1, WindowMsgData mData2)
{
	switch (msg)
	{
#include "lobby_rc.inc"
	}
	return MSG_HANDLED;
}
/* the windows' user data before the call, to print the ones the call set */
static std::map<size_t, void *> w8_ud_before;
static void w8_rc_call(int ctrl, int pos, int x, int y)
{
	w8_ud_before.clear();
	for (size_t i = 0; i < g_windows.size(); ++i)
		if (!g_freed[i] && BitTest(g_windows[i]->winGetInstanceData()->getStyle(), GWS_USER_WINDOW)) w8_ud_before[i] = g_windows[i]->winGetUserData();
	RightClickStruct rc;
	rc.mouseX = x;
	rc.mouseY = y;
	rc.pos = pos;
	GameWindow *control = u4_win(ctrl);
	Bool playerBranch = control && control->winGetWindowId() == listboxLobbyPlayersID;
	w8_rc_system(NULL, GLM_RIGHT_CLICKED, (WindowMsgData)(uintptr_t)control, (WindowMsgData)(uintptr_t)&rc);
	for (std::map<size_t, void *>::iterator it = w8_ud_before.begin(); it != w8_ud_before.end(); ++it)
	{
		void *now = g_windows[it->first]->winGetUserData();
		if (now == it->second) continue;
		if (playerBranch)
		{
			GameSpyRCMenuData *d = (GameSpyRCMenuData *)now;
			g_log << "rcdata " << it->first << " " << d->m_id << " " << w8_hex(d->m_nick.str()) << " " << (int)d->m_itemType << "\n";
		}
		else g_log << "ud " << it->first << " " << (Int)(intptr_t)now << "\n";
	}
}
