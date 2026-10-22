/* Stand-in for GameEngine/Include/Common/GameLOD.h, Module build only (p1w13-sonnet, batch Q): SlowDeathBehavior asks the game detail manager for the slow death
** scale, which the test sets. Renamed ModTestGameLODManager by module_prelude.h. */
#ifndef SHIM_MODULE_GAMELOD_H
#define SHIM_MODULE_GAMELOD_H
#include "Lib/BaseType.h"
class GameLODManager
{
public:
	Real getSlowDeathScale() const { return m_slowDeathScale; }
	Real m_slowDeathScale = 1.0f;
};
extern GameLODManager *TheGameLODManager;
#endif
