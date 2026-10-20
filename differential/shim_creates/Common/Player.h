/* Stand-in for GameEngine/Include/Common/Player.h (layer 7), create-module build only (p1l9-creates): the members
** the create modules call, each logging through crt_log. Renamed to CrtPlayer. */
#ifndef SHIM_CREATES_PLAYER_H
#define SHIM_CREATES_PLAYER_H
#include "Common/GameCommon.h"
#include "Common/Science.h"
#include "Common/AcademyStats.h"
#include "Common/ResourceGatheringManager.h"
#include "Common/Upgrade.h"
class Player
{
public:
	Player() : m_index(0), m_preorder(false), m_sciences(0), m_rgm(0) {}
	Bool didPlayerPreorder(void) const { return m_preorder; }
	Bool hasScience(ScienceType t) const;
	Upgrade *addUpgrade(const UpgradeTemplate *upgradeTemplate, UpgradeStatusType status);
	AcademyStats *getAcademyStats() { return &m_academyStats; }
	ResourceGatheringManager *getResourceGatheringManager() { return m_rgm; }

	int m_index;
	Bool m_preorder;
	unsigned m_sciences;     /* bit k: the science of crt_science_name(k) */
	ResourceGatheringManager *m_rgm;
	AcademyStats m_academyStats;
};
#endif
