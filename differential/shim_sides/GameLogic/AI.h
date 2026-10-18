/* Stand-in for GameLogic/AI.h in the sides/scripts build: AttitudeType (AI.h
** line 315, which Parameter::getUiText names) and AISideBuildList, the
** class BuildListInfo::parseStructure (INI, not run here) hands each
** BuildListInfo to. */
#ifndef SHIM_SIDES_AI_H
#define SHIM_SIDES_AI_H
#include "GameLogic/SidesList.h"
enum AttitudeType { AI_SLEEP = -2, AI_PASSIVE=-1, AI_NORMAL=0, AI_ALERT=1, AI_AGGRESSIVE=2, AI_INVALID=3 };
class AISideBuildList { public: void addInfo(BuildListInfo *); };
#endif
