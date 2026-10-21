/* Stand-in for GameEngine/Include/Common/ThingTemplate.h, Module build only (differential tests, p1w3-modules): Module.cpp includes it and uses nothing
** from it. */
/* --- p1w12-sonnet --- (die batch D: EjectPilotDie asks its object's template for the per-unit sounds VoiceEject and SoundEject; renamed
** ModTestThingTemplate by module_prelude.h, getPerUnitSound answered by differential/l9_dies_d.inc) */
#ifndef SHIM_MODULE_THINGTEMPLATE_H
#define SHIM_MODULE_THINGTEMPLATE_H
#include "Common/AsciiString.h"
#include "Common/AudioEventRTS.h"
class ThingTemplate
{
public:
	const AudioEventRTS *getPerUnitSound(const AsciiString &soundNameToGet) const;
	AudioEventRTS m_voiceEject;
	AudioEventRTS m_soundEject;
};
#endif
/* --- end p1w12-sonnet --- */
