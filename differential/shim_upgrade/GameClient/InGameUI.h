/* Stand-in for GameEngine/Include/GameClient/InGameUI.h, Upgrade/SpecialPower
** build only (differential tests, p1w2-upgrade): the one call
** UpgradeCenter::canAffordUpgrade makes (a message to the player), which the
** tests never reach. */
#ifndef SHIM_UPGRADE_INGAMEUI_H
#define SHIM_UPGRADE_INGAMEUI_H
class InGameUI
{
public:
	void message(const char *) {}
};
extern InGameUI *TheInGameUI;
#endif
