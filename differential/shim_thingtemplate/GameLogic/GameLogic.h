/* Stand-in for GameEngine/Include/GameLogic/GameLogic.h, ThingTemplate build
** only (differential tests, p1w4-thingtemplate): the members ThingFactory.cpp
** and ThingTemplate.cpp call. friend_createObject is the layer 8 cut point of
** newObject: the wrapper's version records the template and returns an
** Object with no behavior modules. (Renamed Tt* by thingtemplate_prelude.h.) */
#ifndef SHIM_TT_GAMELOGIC_H
#define SHIM_TT_GAMELOGIC_H
#include "Lib/BaseType.h"
#include "Common/ObjectStatusTypes.h"
#include "GameLogic/Object.h"
class ThingTemplate;
class GameLogic
{
public:
	Object *friend_createObject(const ThingTemplate *thing, const ObjectStatusMaskType &objectStatusMask, Team *team);
	template<class E> Bool findBuildableStatusOverride(const ThingTemplate *, E &) const { return false; }
	UnsignedInt getSuperweaponRestriction() const;
};
extern GameLogic *TheGameLogic;
#endif
