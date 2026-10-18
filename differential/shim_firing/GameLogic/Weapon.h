/* Stand-in for GameEngine/Include/GameLogic/Weapon.h (layer 8, not translated), FiringTracker tests
** only. The enum is upstream's text, verbatim (Weapon.h lines 172-210); the class is the eight getters
** FiringTracker::shotFired reads, and getFireSound, which returns the event by value as upstream does.
** The wrapper renames the class (FireTestWeapon). */
#ifndef SHIM_FIRING_WEAPON_H
#define SHIM_FIRING_WEAPON_H
#include "Common/AudioEventRTS.h"
#include "Common/GameType.h"
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
#ifdef ALLOW_DEMORALIZE
	WEAPONBONUSCONDITION_DEMORALIZED,
#else
	WEAPONBONUSCONDITION_DEMORALIZED_OBSOLETE,
#endif
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
  WEAPONBONUSCONDITION_FANATICISM, // FOR THE NEW GC INFANTRY GENERAL... adds to nationalism
	WEAPONBONUSCONDITION_FRENZY_ONE,
	WEAPONBONUSCONDITION_FRENZY_TWO,
	WEAPONBONUSCONDITION_FRENZY_THREE,

	WEAPONBONUSCONDITION_COUNT
};

class Weapon
{
public:
	Weapon() : m_autoReload(0), m_coast(0), m_possibleNext(0), m_reloading(FALSE), m_one(0), m_two(0), m_loopTime(0) {}
	UnsignedInt getAutoReloadWhenIdleFrames() const;
	UnsignedInt getContinuousFireCoastFrames() const;
	UnsignedInt getPossibleNextShotFrame() const;
	Int getContinuousFireOneShotsNeeded() const;
	Int getContinuousFireTwoShotsNeeded() const;
	UnsignedInt getFireSoundLoopTime() const;
	AudioEventRTS getFireSound() const;
	UnsignedInt m_autoReload, m_coast, m_possibleNext;
	Bool m_reloading;
	Int m_one, m_two;
	UnsignedInt m_loopTime;
	AsciiString m_fireSound;
};
#endif
