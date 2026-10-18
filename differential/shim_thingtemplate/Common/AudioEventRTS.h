/* Stand-in for GameEngine/Include/Common/AudioEventRTS.h, ThingTemplate build
** only (differential tests, p1w4-thingtemplate). The real class is the
** client's audio event; a thing template holds 36 of them (as
** DynamicAudioEventRTS) and a map of more, and INI sets only their names. So
** this keeps the name, the cut point of the translation. (Renamed Tt* by
** thingtemplate_prelude.h.) */
#ifndef SHIM_TT_AUDIOEVENTRTS_H
#define SHIM_TT_AUDIOEVENTRTS_H
#include "Common/AsciiString.h"
#include "Common/GameMemory.h"
class AudioEventRTS
{
public:
	AudioEventRTS() {}
	void setEventName(AsciiString name) { m_eventName = name; }
	const AsciiString &getEventName() const { return m_eventName; }
	AsciiString m_eventName;
};
class DynamicAudioEventRTS : public MemoryPoolObject
{
public:
	DynamicAudioEventRTS() {}
	DynamicAudioEventRTS(const DynamicAudioEventRTS &that) : MemoryPoolObject(), m_event(that.m_event) {}
	DynamicAudioEventRTS &operator=(const DynamicAudioEventRTS &that) { m_event = that.m_event; return *this; }
	AudioEventRTS m_event;
};
#endif
