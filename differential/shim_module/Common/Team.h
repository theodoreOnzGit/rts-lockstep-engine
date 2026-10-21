/* Stand-in for GameEngine/Include/Common/Team.h, Module build only (differential tests, p1w12-sonnet, special power batch H): a team is an opaque
** pointer the test numbers (the real header needs the whole team and object layer). */
#ifndef SHIM_MODULE_TEAM_H
#define SHIM_MODULE_TEAM_H
#include "Common/GameCommon.h"
class Team { public: int m_dummy;
	/* --- p1w13-sonnet --- (update batch O: Object::getRelationship, cut from Object.cpp by build.rs, asks it; differential/l9_squish_collide_o.inc) */
	Relationship getRelationship(const Team *that) const;
	class Player *m_controller = 0;   /* update batch O: the player of the team (the test's) */
	/* --- end p1w13-sonnet --- */
};
#endif
