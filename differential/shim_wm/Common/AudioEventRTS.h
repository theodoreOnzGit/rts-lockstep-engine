/* Shim for GameEngine/Include/Common/AudioEventRTS.h, differential/wrap_wm.cpp only (#70): the gadgets build an
** event from its name and hand it to TheAudio->addAudioEvent, which the test records. */
#ifndef SHIM_WM_AUDIOEVENTRTS_H
#define SHIM_WM_AUDIOEVENTRTS_H
#include "Common/AsciiString.h"
class AudioEventRTS
{
public:
	AudioEventRTS() {}
	AudioEventRTS(const AsciiString &name) : m_name(name) {}
	AudioEventRTS(const char *name) : m_name(name) {}
	void setEventName(AsciiString name) { m_name = name; }
	AsciiString m_name;
};
#endif
