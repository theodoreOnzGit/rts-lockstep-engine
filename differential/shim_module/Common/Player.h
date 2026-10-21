/* Stand-in for GameEngine/Include/Common/Player.h (layer 7), Module build only (differential tests,
** p1l9-example): the one thing the layer-9 modules compiled here read of a Player, its completed
** upgrades. Renamed ModTestPlayer by module_prelude.h. Layer-9 batches add what their modules read,
** each in its marked block. */
#ifndef SHIM_MODULE_PLAYER_H
#define SHIM_MODULE_PLAYER_H
#include "Common/Upgrade.h"
#include "Common/Science.h"
/* --- p1w11-sonnet --- */
#include "Common/KindOf.h"
#include "Common/GameCommon.h"
#include <vector>
class SpecialPowerTemplate;
class Team;
#include <algorithm>
class Object;
/* --- end p1w11-sonnet --- */
/* crate batch F: the money and the score keeper of a player, whose calls are lines of the effect log (differential/l9_crates_f.inc) */
class Money { public: void deposit(UnsignedInt amountToDeposit, Bool playSound = TRUE); Int m_idx; };
class ScoreKeeper { public: void addMoneyEarned(Int money); Int m_idx; };
class Player
{
public:
	UpgradeMaskType getCompletedUpgradeMask() const { return m_upgradesCompleted; }
	UpgradeMaskType m_upgradesCompleted;
	/* --- p1l9-example --- */
	/* --- end p1l9-example --- */
	/* --- p1w10-sonnet --- (GrantScienceUpgrade: the grant is a line of the effect log) */
	void grantScience(ScienceType st);
	/* --- end p1w10-sonnet --- */
	/* --- p1w11-sonnet --- (upgrade batch C: what CostModifierUpgrade, PowerPlantUpgrade and RadarUpgrade call on their player; every
	** call is a line of the effect log of differential/l9_upgrades_c.inc, tagged with m_idx, the player's index in the test) */
	void addKindOfProductionCostChange(KindOfMaskType kindOf, Real percent);
	void removeKindOfProductionCostChange(KindOfMaskType kindOf, Real percent);
	void addPowerBonus(Object *obj);
	void removePowerBonus(Object *obj);
	void addRadar(Bool disableProof);
	void removeRadar(Bool disableProof);
	void onStructureConstructionComplete(Object *builder, Object *structure, Bool isRebuild);   /* ReplaceObjectUpgrade */
	Int m_idx;
	Int getPlayerIndex() const { return m_idx; }   /* the dies: SpecialPowerCompletionDie, EjectPilotDie */
	/* crate batch F: the crates ask a player its type and sciences, and heal all its objects (a line of the effect log) */
	PlayerType getPlayerType() const { return m_type; }
	Bool hasScience(ScienceType t) const { return std::find(m_sciences.begin(), m_sciences.end(), t) != m_sciences.end(); }
	void healAllObjects();   /* differential/l9_crates_f.inc */
	Money *getMoney() { return &m_money; }
	ScoreKeeper *getScoreKeeper() { return &m_score; }
	Bool hasUpgradeComplete(const UpgradeTemplate *upgradeTemplate) { return m_upgradesCompleted.testForAll(upgradeTemplate->getUpgradeMask()); }
	/* special power batch H: the player's shared special power timers, its academy stats, its relationship to a team and its default team */
	void expressSpecialPowerReadyFrame(const SpecialPowerTemplate *temp, UnsignedInt frame);
	UnsignedInt getOrStartSpecialPowerReadyFrame(const SpecialPowerTemplate *temp);
	void resetOrStartSpecialPowerReadyFrame(const SpecialPowerTemplate *temp);
	class PowerAcademyStats *getAcademyStats() { return (class PowerAcademyStats *)this; }
	Relationship getRelationship(const Team *that) const;
	Team *getDefaultTeam() { return m_defaultTeam; }
	void setUnitsVisionSpied(Bool setting, KindOfMaskType whichUnits, Int byWhom);   /* SpyVisionUpdate: a line of the effect log */
	Team *m_defaultTeam;
	Money m_money;
	ScoreKeeper m_score;
	PlayerType m_type;
	std::vector<ScienceType> m_sciences;
	/* --- end p1w11-sonnet --- */
};
/* special power batch H: what getAcademyStats() answers (it is the player itself, whose index the log wants) */
class PowerAcademyStats { public: void recordSpecialPowerUsed(const SpecialPowerTemplate *t); };
#endif
