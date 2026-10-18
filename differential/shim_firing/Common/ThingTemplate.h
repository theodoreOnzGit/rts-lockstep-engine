/* Stand-in for GameEngine/Include/Common/ThingTemplate.h (layer 5, not translated),
** FiringTracker tests only: the one query the tracker makes, getPerUnitSound. The wrapper
** renames the class (FireTest*). */
#ifndef SHIM_FIRING_THINGTEMPLATE_H
#define SHIM_FIRING_THINGTEMPLATE_H
#include "Common/AudioEventRTS.h"
class ThingTemplate
{
public:
	const AudioEventRTS *getPerUnitSound(const AsciiString &soundName) const;
};
#endif
