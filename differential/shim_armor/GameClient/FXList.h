/* Shim for GameEngine/Include/GameClient/FXList.h, for the Armor/Damage/
** DamageFX build only (differential tests): FXList as an opaque class with
** the static that DamageFX::doDamageFX calls, and the store lookup that
** INI::parseFXList calls. Both are defined in differential/wrap_armor.cpp. */
#ifndef SHIM_ARMOR_FXLIST_H
#define SHIM_ARMOR_FXLIST_H
#include "Common/AsciiString.h"
class Object;
class FXList
{
public:
	static void doFXObj(const FXList *fx, const Object *primary, const Object *secondary = 0);
};
class FXListStore { public: const FXList *findFXList(const char *name) const; };
extern FXListStore *TheFXListStore;
#endif
