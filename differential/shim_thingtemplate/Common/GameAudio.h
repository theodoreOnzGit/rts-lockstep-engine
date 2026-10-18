/* Stand-in for GameEngine/Include/Common/GameAudio.h, ThingTemplate build
** only (differential tests, p1w4-thingtemplate): the one call the field
** parser makes. */
#ifndef SHIM_TT_GAMEAUDIO_H
#define SHIM_TT_GAMEAUDIO_H
#include "Common/AudioEventRTS.h"
class AudioManager
{
public:
	void getInfoForAudioEvent(const AudioEventRTS *) const {}
};
extern AudioManager *TheAudio;
#endif
