/* Stand-in for GameEngine/Include/GameLogic/Weapon.h, ThingTemplate build
** only (differential tests, p1w4-thingtemplate): the lookup that a
** WeaponSet's `Weapon =` makes. The store is layer 8; this one knows the
** names the wrapper registers. (Renamed Tt* by thingtemplate_prelude.h.) */
#ifndef SHIM_TT_WEAPON_H
#define SHIM_TT_WEAPON_H
#include "Common/AsciiString.h"
class WeaponTemplate
{
public:
	AsciiString m_name;
};
class WeaponStore
{
public:
	const WeaponTemplate *findWeaponTemplate(AsciiString name) const;
};
extern WeaponStore *TheWeaponStore;
#endif
