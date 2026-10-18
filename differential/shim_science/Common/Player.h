/* Shim for GameEngine/Include/Common/Player.h, Science/RankInfo build only
** (differential tests). The real header is the player of the whole game
** (layer 7). Science.cpp calls Player::hasScience from the three methods
** that take a Player (playerHasPrereqsForScience, playerHasRootPrereqsForScience,
** getPurchasableSciences); they are NOT translated and NOT tested, and the
** stand-in (wrap_science.cpp) aborts if reached. */
#ifndef SHIM_SCIENCE_PLAYER_H
#define SHIM_SCIENCE_PLAYER_H
#include "Common/Science.h"
class Player
{
public:
	Bool hasScience(ScienceType) const;
};
#endif
