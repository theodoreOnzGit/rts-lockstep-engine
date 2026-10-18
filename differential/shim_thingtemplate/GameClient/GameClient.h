/* Stand-in for GameEngine/Include/GameClient/GameClient.h, ThingTemplate
** build only (differential tests, p1w4-thingtemplate): the one call
** ThingFactory::newDrawable makes, defined in the wrapper to record the
** template and the status bits. (Renamed Tt* by thingtemplate_prelude.h.) */
#ifndef SHIM_TT_GAMECLIENT_H
#define SHIM_TT_GAMECLIENT_H
#include "GameClient/Drawable.h"
class ThingTemplate;
class GameClient
{
public:
	Drawable *friend_createDrawable(const ThingTemplate *thing, DrawableStatus statusBits = DRAWABLE_STATUS_NONE);
};
extern GameClient *TheGameClient;
#endif
