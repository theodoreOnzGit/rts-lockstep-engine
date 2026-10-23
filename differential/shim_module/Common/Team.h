/* Stand-in for GameEngine/Include/Common/Team.h, Module build only (differential tests, p1w12-sonnet, special power batch H): a team is an opaque
** pointer the test numbers (the real header needs the whole team and object layer). */
#ifndef SHIM_MODULE_TEAM_H
#define SHIM_MODULE_TEAM_H
#include "Common/GameCommon.h"
class Team { public: int m_dummy;
	/* --- p1w13-sonnet --- (update batch O: Object::getRelationship, cut from Object.cpp by build.rs, asks it; differential/l9_squish_collide_o.inc) */
	Relationship getRelationship(const Team *that) const;
	class Player *m_controller = 0;   /* update batch O: the player of the team (the test's) */
	/* batch R11 (CaveContain): Team.h's id, which the test's number is */
	UnsignedInt getID() const { return (UnsignedInt)m_dummy; }
	class Player *getControllingPlayer() const { return m_controller; }   /* batch R12: GarrisonContain */
	/* --- end p1w13-sonnet --- */
};
/* --- p1w13-sonnet --- (batch R11: CaveContain's xfer; Team.h lines 41-42 and the factory's lookup, answered by differential/l9_open_contain_r.inc) */
typedef UnsignedInt TeamID;
#define TEAM_ID_INVALID 0
class TeamFactory { public: Team *findTeamByID( TeamID teamID ); };
extern TeamFactory *TheTeamFactory;
/* --- end p1w13-sonnet --- */
#endif
