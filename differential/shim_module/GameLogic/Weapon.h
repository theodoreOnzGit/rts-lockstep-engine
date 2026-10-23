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
	const WeaponTemplate *findWeaponTemplate(const char *name) const;   /* p1w13-sonnet: SlowDeathBehavior (differential/l9_slow_death_q.inc) */
};
extern WeaponStore *TheWeaponStore;
/* --- end p1w12-sonnet --- */
/* --- p1w12-sonnet --- (update batch M, FireOCLAfterWeaponCooldownUpdate.cpp) a Weapon with the three facts that module reads of one; renamed
** ModTestWeapon by module_prelude.h so that no inline member clashes with the weapon fixtures' real Weapon */
#include "Common/GameType.h"
/* --- p1w13-sonnet --- (batch R2: TransportContain) */
class WeaponTemplate
{
public:
	Bool isContactWeapon() const { return m_contact; }
	Bool m_contact = false;
};
/* --- end p1w13-sonnet --- */
class Weapon
{
public:
	WeaponSlotType getWeaponSlot() const { return m_slot; }
	UnsignedInt getLastShotFrame() const { return m_last; }
	UnsignedInt getPossibleNextShotFrame() const { return m_next; }
	WeaponSlotType m_slot = PRIMARY_WEAPON;
	UnsignedInt m_last = 0, m_next = 0;
	/* --- p1w13-sonnet --- (batch R2: TransportContain) */
	const WeaponTemplate *getTemplate() const { return &m_tmplObj; }
	Bool isDamageWeapon() const { return m_damageW; }
	void transferNextShotStatsFrom( const Weapon &weapon );   /* logs: differential/l9_open_contain_r.inc */
	WeaponTemplate m_tmplObj;
	Bool m_damageW = false;
	Int m_ownerId = 0;
	/* --- end p1w13-sonnet --- */
	/* --- p1w13-sonnet --- (batch R12: GarrisonContain) the range check (a line, the test's answer: differential/l9_open_contain_r.inc) and the damage type */
	Bool isWithinAttackRange(const Object *source, const Object *target) const;
	Bool isWithinAttackRange(const Object *source, const Coord3D *pos) const;
	Int getDamageType() const { return m_damageType; }   /* a DamageType */
	Int m_damageType = 0;
	/* --- end p1w13-sonnet --- */
};
/* --- end p1w12-sonnet --- */
/* --- p1w13-sonnet --- (batch P: ActiveBody asks the guard to retaliate with no limit on the shots; Weapon.h line 56, verbatim) */
const Int NO_MAX_SHOTS_LIMIT = 0x7fffffff;
/* --- end p1w13-sonnet --- */
#endif
/* --- p1w13-sonnet --- (batch R17: WeaponBonusUpdate.cpp defines DEFINE_WEAPONBONUSCONDITION_NAMES and includes this header again, after the
** include guard above has closed: the names are outside it, under their own guard) Weapon.h lines 211-249, verbatim */
#ifdef DEFINE_WEAPONBONUSCONDITION_NAMES
#ifndef SHIM_MODULE_WEAPON_BONUS_NAMES
#define SHIM_MODULE_WEAPON_BONUS_NAMES
static const char *TheWeaponBonusNames[] = 
{
	// This is a RHS enum (weapon.ini will have WeaponBonus = IT) so it is all caps
	"GARRISONED",
	"HORDE",
	"CONTINUOUS_FIRE_MEAN",
	"CONTINUOUS_FIRE_FAST",
	"NATIONALISM",
	"PLAYER_UPGRADE",
	"DRONE_SPOTTING",
#ifdef ALLOW_DEMORALIZE
	"DEMORALIZED",
#else
	"DEMORALIZED_OBSOLETE",
#endif
	"ENTHUSIASTIC",
	"VETERAN",
	"ELITE",
	"HERO",
	"BATTLEPLAN_BOMBARDMENT",
	"BATTLEPLAN_HOLDTHELINE",
	"BATTLEPLAN_SEARCHANDDESTROY",
	"SUBLIMINAL",
	"SOLO_HUMAN_EASY",
	"SOLO_HUMAN_NORMAL",
	"SOLO_HUMAN_HARD",
	"SOLO_AI_EASY",
	"SOLO_AI_NORMAL",
	"SOLO_AI_HARD",
	"TARGET_FAERIE_FIRE",
  "FANATICISM", // FOR THE NEW GC INFANTRY GENERAL... adds to nationalism
	"FRENZY_ONE",
	"FRENZY_TWO",
	"FRENZY_THREE",

	NULL
};
#endif
#endif
/* --- end p1w13-sonnet --- */
