/* Stand-in for GameNetwork/GameSpy/StagingRoomGameInfo.h, differential tests only (differential/wrap_game_info.cpp): TheGameSpyGame, NULL (setState's only use of it is a DEBUG_CRASH). */
#ifndef SHIM_GINFO_STAGING_H
#define SHIM_GINFO_STAGING_H
class GameInfo;
extern GameInfo *TheGameSpyGame;
#endif
