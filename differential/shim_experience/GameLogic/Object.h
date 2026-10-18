/* Stand-in for GameEngine/Include/GameLogic/Object.h (layer 8, not translated),
** ExperienceTracker tests only: the members ExperienceTracker.cpp calls. An object owns
** its template, its tracker and, per other object, the relationship of this object
** (the killer) toward it. The wrapper renames the class (wrap_experience_tracker.cpp)
** and defines onVeterancyLevelChanged, which records the call. */
#ifndef SHIM_EXPERIENCE_OBJECT_H
#define SHIM_EXPERIENCE_OBJECT_H
#include "Common/GameCommon.h"
#include "Common/ThingTemplate.h"
#include "GameLogic/ExperienceTracker.h"
class Object
{
public:
	Object() : m_index(0), m_template(NULL), m_tracker(NULL) { for (int i = 0; i < 4; ++i) m_relations[i] = ENEMIES; }
	Relationship getRelationship(const Object *other) const { return m_relations[other->m_index]; }
	const ThingTemplate *getTemplate() const { return m_template; }
	ExperienceTracker *getExperienceTracker() const { return m_tracker; }
	void onVeterancyLevelChanged(VeterancyLevel oldLevel, VeterancyLevel newLevel, Bool provideFeedback = TRUE);
	Int m_index;
	const ThingTemplate *m_template;
	ExperienceTracker *m_tracker;
	Relationship m_relations[4];
};
#endif
