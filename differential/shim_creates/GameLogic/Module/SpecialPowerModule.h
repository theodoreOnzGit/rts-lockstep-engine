/* Stand-in for GameEngine/Include/GameLogic/Module/SpecialPowerModule.h, create-module build only (p1l9-creates).
** The real header declares SpecialPowerModule, an abstract class whose inline member uses the module macros
** and does not compile under clang (it allocates the abstract class). SpecialPowerCreate.cpp needs only the
** interface it calls: onSpecialPowerCreation, reached through BehaviorModule::getSpecialPower, which the loop
** of SpecialPowerCreate::onBuildComplete never gets to (no behaviour modules are made in this build). */
#ifndef SHIM_CREATES_SPECIALPOWERMODULE_H
#define SHIM_CREATES_SPECIALPOWERMODULE_H
class SpecialPowerModuleInterface
{
public:
	virtual ~SpecialPowerModuleInterface() {}
	virtual void onSpecialPowerCreation(void) = 0;
};
#endif
