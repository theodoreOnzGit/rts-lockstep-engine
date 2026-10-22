/* Stand-in for GameEngine/Include/Common/PlayerList.h, Module build only (differential tests, p1w12-sonnet, special power batch H): the local
** player, which differential/l9_powers_h.inc answers. Renamed ModTestPlayerList by module_prelude.h. */
#ifndef SHIM_MODULE_PLAYERLIST_H
#define SHIM_MODULE_PLAYERLIST_H
#include "Lib/BaseType.h"
class Player;
/* --- p1w13-sonnet --- (batch P: ActiveBody's retaliation filter; Common/PlayerList.h lines 63-69, verbatim) */
enum AllowPlayerRelationship
{
	ALLOW_SAME_PLAYER			= 0x01,		///< allow only objects of the same player as m_obj
	ALLOW_ALLIES					= 0x02,		///< allow objects that m_obj considers allies
	ALLOW_ENEMIES					= 0x04,		///< allow objects that m_obj considers enemy 
	ALLOW_NEUTRAL					= 0x08		///< allow objects that m_obj considers neutral
};
/* --- end p1w13-sonnet --- */
class PlayerList
{
public:
	Player *getLocalPlayer();
	Int getPlayerCount();            /* special power batch H / update batch I: the three players of the test */
	Player *getNthPlayer(Int i);
	Player *getNeutralPlayer();   /* p1w13-sonnet, update batch O: TechBuildingBehavior (differential/l9_tech_building_o.inc) */
};
extern PlayerList *ThePlayerList;
#endif
