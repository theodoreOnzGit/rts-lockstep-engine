/* Stand-in for GameEngine/Include/Common/KindOf.h, TunnelTracker tests only. The real header's BitFlags cannot
** compile in this build, and differential/shim/Common/STLTypedefs.h already declares an (empty) enum KindOfType.
** The four enumerators TunnelTracker.cpp names, with their values in upstream's KindOf.h (counted in order from
** KINDOF_OBSTACLE = 0: SELECTABLE 1, IMMOBILE 2, CAN_ATTACK 3, STICK_TO_TERRAIN_SLOPE 4, CAN_CAST_REFLECTIONS 5,
** SHRUBBERY 6, STRUCTURE 7, INFANTRY 8, VEHICLE 9, AIRCRAFT 10). */
#ifndef SHIM_TUNNEL_KINDOF_H
#define SHIM_TUNNEL_KINDOF_H
#include "Common/STLTypedefs.h"
enum TunnelTestKinds { KINDOF_STRUCTURE = 7, KINDOF_INFANTRY = 8, KINDOF_VEHICLE = 9, KINDOF_AIRCRAFT = 10 };
#endif
