/* Stand-in for GameEngine/Include/Common/AudioEventRTS.h, Upgrade/SpecialPower
** build only (differential tests, p1w2-upgrade). The real class is the
** client's audio event; the templates hold two of them and INI sets only
** their names (INI::parseAudioEventRTS -> setEventName, in INI.cpp). So this
** is a class that keeps the name, the cut point of the translation. The
** out-of-line setEventName is defined in differential/wrap_upgrade.cpp (the
** INI build calls it by this mangled name). */
#ifndef SHIM_UPGRADE_AUDIOEVENTRTS_H
#define SHIM_UPGRADE_AUDIOEVENTRTS_H
#include "Common/AsciiString.h"
class AudioEventRTS
{
public:
	AudioEventRTS() {}
	void setEventName(AsciiString name);
	const AsciiString &getEventName() const { return m_eventName; }
private:
	AsciiString m_eventName;
};
#endif
