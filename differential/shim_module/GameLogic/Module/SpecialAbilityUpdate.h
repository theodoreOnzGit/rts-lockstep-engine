/* Stand-in for GameEngine/Include/GameLogic/Module/SpecialAbilityUpdate.h, Module build only (differential tests, p1w13-sonnet, update batch O): the one
** call SquishCollide makes of the module findSpecialAbilityUpdate returns. */
#ifndef SHIM_MODULE_SPECIALABILITYUPDATE_H
#define SHIM_MODULE_SPECIALABILITYUPDATE_H
#include "Lib/BaseType.h"
class SpecialAbilityUpdate { public: Bool isActive() const { return m_active; } Bool m_active = false; };
#endif
