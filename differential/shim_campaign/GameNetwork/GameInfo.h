/* Shim for GameEngine/Include/GameNetwork/GameInfo.h (upstream commit 0a05454d), the campaign differential only (GitHub
** #64). CampaignManager::xfer makes, xfers and deletes TheChallengeGameInfo (a SkirmishGameInfo); the test does not run
** the xfer (the save's own differential is the game info's, differential/wrap_game_info.cpp), so the class is the members
** that xfer calls, empty. The wrapper renames the class and the global (CampSkirmishGameInfo, CampTheChallengeGameInfo). */
#ifndef SHIM_CAMPAIGN_GAMEINFO_H
#define SHIM_CAMPAIGN_GAMEINFO_H
#include "Common/Snapshot.h"
class SkirmishGameInfo : public Snapshot
{
public:
	void init( void ) { }
	void clearSlotList( void ) { }
	void reset( void ) { }
protected:
	virtual void crc( Xfer * ) { }
	virtual void xfer( Xfer * ) { }
	virtual void loadPostProcess( void ) { }
};
extern SkirmishGameInfo *TheChallengeGameInfo;
#endif
