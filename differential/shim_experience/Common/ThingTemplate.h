/* Stand-in for GameEngine/Include/Common/ThingTemplate.h (layer 5, not translated),
** ExperienceTracker tests only: the three experience queries the tracker makes, with the
** values the wrapper sets for each object. The wrapper renames the class
** (ExpTrack*, wrap_experience_tracker.cpp). */
#ifndef SHIM_EXPERIENCE_THINGTEMPLATE_H
#define SHIM_EXPERIENCE_THINGTEMPLATE_H
#include "Common/GameCommon.h"
class ThingTemplate
{
public:
	ThingTemplate() : m_trainable(FALSE) { for (int i = 0; i < LEVEL_COUNT; ++i) m_values[i] = m_required[i] = 0; }
	Int getExperienceValue(Int level) const { return m_values[level]; }
	Int getExperienceRequired(Int level) const { return m_required[level]; }
	Bool isTrainable() const { return m_trainable; }
	Int m_values[LEVEL_COUNT];
	Int m_required[LEVEL_COUNT];
	Bool m_trainable;
};
#endif
