/* Shim for GameEngine/Include/Common/AudioEventRTS.h (upstream commit 0a05454d), the campaign differential only (GitHub #64;
** build.rs, differential/wrap_campaign.cpp). CampaignManager.h holds a Mission's `m_briefingVoice` as an AudioEventRTS,
** which `INI::parseAudioEventRTS` (cut verbatim from INI.cpp) fills with `setEventName`; the event's name is all that this
** build reads of it (the real class, its description lookup and its play state are the audio differential's,
** differential/wrap_audio.cpp). The wrapper renames the class (CampAudioEventRTS) so that it is no other archive's. */
#ifndef SHIM_CAMPAIGN_AUDIOEVENTRTS_H
#define SHIM_CAMPAIGN_AUDIOEVENTRTS_H
#include "Common/AsciiString.h"
class AudioEventRTS
{
public:
	AsciiString m_eventName;
	void setEventName( AsciiString name ) { m_eventName = name; }
	const AsciiString &getEventName( void ) const { return m_eventName; }
};
#endif
