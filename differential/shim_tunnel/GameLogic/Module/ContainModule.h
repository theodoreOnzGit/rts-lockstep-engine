/* Stand-in for GameEngine/Include/GameLogic/Module/ContainModule.h, TunnelTracker tests only: the two typedefs
** TunnelTracker.h takes from it (ContainModule.h:67 and :74, verbatim). */
#ifndef SHIM_TUNNEL_CONTAINMODULE_H
#define SHIM_TUNNEL_CONTAINMODULE_H
#include "tunnel_world.h"
typedef std::list<Object*> ContainedItemsList;
typedef void (*ContainIterateFunc)( Object *obj, void *userData );		///< callback type for contain iterate
#endif
