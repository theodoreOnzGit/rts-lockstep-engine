/* Stand-in for GameEngine/Include/GameLogic/Weapon.h, Module build only (p1w10-sonnet, the layer-9 upgrade batch): WeaponBonusUpgrade.cpp
** includes it for WeaponBonusConditionType and uses nothing else. The enum is copied verbatim from Weapon.h lines 172-211 (commit 0a05454d), with
** ALLOW_DEMORALIZE undefined, as the shipped game. The real header is the weapon fixtures' (shim_weapon) and needs the whole weapon layer. */
#ifndef SHIM_MODULE_WEAPON_H
#define SHIM_MODULE_WEAPON_H
enum WeaponBonusConditionType
{
	// The access and use of this enum has the bit shifting built in, so this is a 0,1,2,3,4,5 enum
	WEAPONBONUSCONDITION_INVALID = -1,

	WEAPONBONUSCONDITION_GARRISONED = 0,
	WEAPONBONUSCONDITION_HORDE,
	WEAPONBONUSCONDITION_CONTINUOUS_FIRE_MEAN,
	WEAPONBONUSCONDITION_CONTINUOUS_FIRE_FAST,
	WEAPONBONUSCONDITION_NATIONALISM,
	WEAPONBONUSCONDITION_PLAYER_UPGRADE,
	WEAPONBONUSCONDITION_DRONE_SPOTTING,
	WEAPONBONUSCONDITION_DEMORALIZED_OBSOLETE,
	WEAPONBONUSCONDITION_ENTHUSIASTIC,
	WEAPONBONUSCONDITION_VETERAN,
	WEAPONBONUSCONDITION_ELITE,
	WEAPONBONUSCONDITION_HERO,
	WEAPONBONUSCONDITION_BATTLEPLAN_BOMBARDMENT,
	WEAPONBONUSCONDITION_BATTLEPLAN_HOLDTHELINE,
	WEAPONBONUSCONDITION_BATTLEPLAN_SEARCHANDDESTROY,
	WEAPONBONUSCONDITION_SUBLIMINAL,
	WEAPONBONUSCONDITION_SOLO_HUMAN_EASY,
	WEAPONBONUSCONDITION_SOLO_HUMAN_NORMAL,
	WEAPONBONUSCONDITION_SOLO_HUMAN_HARD,
	WEAPONBONUSCONDITION_SOLO_AI_EASY,
	WEAPONBONUSCONDITION_SOLO_AI_NORMAL,
	WEAPONBONUSCONDITION_SOLO_AI_HARD,
	WEAPONBONUSCONDITION_TARGET_FAERIE_FIRE,
	WEAPONBONUSCONDITION_FANATICISM,
	WEAPONBONUSCONDITION_FRENZY_ONE,
	WEAPONBONUSCONDITION_FRENZY_TWO,
	WEAPONBONUSCONDITION_FRENZY_THREE,

	WEAPONBONUSCONDITION_COUNT
};
/* --- p1w12-sonnet --- (behavior batch K: FireWeaponWhenDeadBehavior.cpp) what that module asks of the weapon layer: a template it was given by the INI
** (opaque here) and the store that fires a temporary weapon, which differential/l9_behaviors_k.inc defines to log the call. Renamed by module_prelude.h. */
class Object;
#include "Lib/BaseType.h"
class WeaponTemplate;
class WeaponStore
{
public:
	void createAndFireTempWeapon(const WeaponTemplate *wt, const Object *source, const Coord3D *pos);
};
extern WeaponStore *TheWeaponStore;
/* --- end p1w12-sonnet --- */
/* --- p1w12-sonnet --- (update batch M, FireOCLAfterWeaponCooldownUpdate.cpp) a Weapon with the three facts that module reads of one; renamed
** ModTestWeapon by module_prelude.h so that no inline member clashes with the weapon fixtures' real Weapon */
#include "Common/GameType.h"
class Weapon
{
public:
	WeaponSlotType getWeaponSlot() const { return m_slot; }
	UnsignedInt getLastShotFrame() const { return m_last; }
	UnsignedInt getPossibleNextShotFrame() const { return m_next; }
	WeaponSlotType m_slot = PRIMARY_WEAPON;
	UnsignedInt m_last = 0, m_next = 0;
};
/* --- end p1w12-sonnet --- */
/* --- p1w13-sonnet --- (batch P: ActiveBody asks the guard to retaliate with no limit on the shots; Weapon.h line 56, verbatim) */
const Int NO_MAX_SHOTS_LIMIT = 0x7fffffff;
/* --- end p1w13-sonnet --- */
#endif
