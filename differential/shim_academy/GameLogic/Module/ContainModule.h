/* Stand-in for GameEngine/Include/GameLogic/Module/ContainModule.h (layer 9),
** academy tests only: isTunnelContain(), the one question recordProduction asks
** of an object's container. */
#ifndef SHIM_ACADEMY_CONTAINMODULE_H
#define SHIM_ACADEMY_CONTAINMODULE_H
#include "Lib/BaseType.h"
class ContainModuleInterface
{
public:
	ContainModuleInterface() : m_tunnel(false) {}
	Bool isTunnelContain() const { return m_tunnel; }
	Bool m_tunnel;
};
#endif
