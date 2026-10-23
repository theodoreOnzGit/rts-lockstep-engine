/* Stand-in for GameEngine/Include/GameClient/GameClient.h, Module build only (differential tests, p1w13-sonnet, batch R12: GarrisonContain): the two
** calls a garrison makes on its muzzle-flash drawables, answered by differential/l9_open_contain_r.inc. */
#ifndef SHIM_MODULE_GAMECLIENT_H
#define SHIM_MODULE_GAMECLIENT_H
#include "Common/GameType.h"
class Drawable;
class GameClient
{
public:
	void destroyDrawable(Drawable *draw);
	Drawable *findDrawableByID(const DrawableID id);
};
extern GameClient *TheGameClient;
#endif
