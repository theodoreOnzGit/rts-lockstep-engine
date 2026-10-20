/* Stand-in for GameEngine/Include/GameLogic/ExperienceTracker.h (layer 8), create-module build only (p1l9-creates):
** the two methods VeterancyGainCreate calls, logging through crt_log. Renamed to CrtExperienceTracker. */
#ifndef SHIM_CREATES_EXPERIENCETRACKER_H
#define SHIM_CREATES_EXPERIENCETRACKER_H
#include "Common/GameCommon.h"
class ExperienceTracker
{
public:
	ExperienceTracker() : m_trainable(false), m_level(LEVEL_REGULAR) {}
	Bool isTrainable() const;
	void setMinVeterancyLevel(VeterancyLevel newLevel);
	Bool m_trainable;
	VeterancyLevel m_level;
};
#endif
