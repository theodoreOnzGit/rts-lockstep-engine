/* Stand-in for GameEngine/Include/Common/Player.h (layer 7, not translated),
** ObjectTypes tests only: the one member ObjectTypes.cpp calls, canBuild. The
** wrapper renames the class (wrap_object_types.cpp). */
#ifndef SHIM_OBJECTTYPES_PLAYER_H
#define SHIM_OBJECTTYPES_PLAYER_H
#include "Lib/BaseType.h"
class ThingTemplate;
class Player
{
public:
	Bool canBuild(const ThingTemplate *tmpl) const;
};
#endif
