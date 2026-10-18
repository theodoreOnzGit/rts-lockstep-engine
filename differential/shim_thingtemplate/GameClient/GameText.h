/* Stand-in for GameEngine/Include/GameClient/GameText.h, ThingTemplate build
** only (differential tests, p1w4-thingtemplate): included by
** ProductionPrerequisite.cpp (getRequiresList, not translated). */
#ifndef SHIM_TT_GAMETEXT_H
#define SHIM_TT_GAMETEXT_H
#include "Common/UnicodeString.h"
class GameTextInterface
{
public:
	UnicodeString fetch(const char *label, Bool *exists = NULL);
};
extern GameTextInterface *TheGameText;
#endif
