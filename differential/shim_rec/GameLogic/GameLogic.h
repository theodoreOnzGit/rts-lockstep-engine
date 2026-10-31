/* Stand-in for GameLogic/GameLogic.h, differential tests only (differential/wrap_recorder.cpp): the frame, the game mode (GameMode copied from GameLogic.h lines 75-84) and clearGameData. */
#ifndef SHIM_REC_GAMELOGIC_H
#define SHIM_REC_GAMELOGIC_H
enum GameMode
{
	GAME_SINGLE_PLAYER,
	GAME_LAN,
	GAME_SKIRMISH,
	GAME_REPLAY,
	GAME_SHELL,
	GAME_INTERNET,
	GAME_NONE
};
class GameLogic
{
public:
	UnsignedInt m_frame; Int m_gameMode;
	UnsignedInt getFrame() { return m_frame; }
	Bool isInGame() { return !(m_gameMode == GAME_NONE); }
	Int getGameMode() { return m_gameMode; }
	void clearGameData(Bool showScoreScreen = TRUE);
};
extern GameLogic *TheGameLogic;
#endif
