/* Shim for GameEngine/Include/Common/GameAudio.h, differential/wrap_wm.cpp only (#70): addAudioEvent, recorded by
** the test (wrap_wm.cpp). */
#ifndef SHIM_WM_GAMEAUDIO_H
#define SHIM_WM_GAMEAUDIO_H
#include "Common/AudioEventRTS.h"
class AudioManager
{
public:
	void addAudioEvent(const AudioEventRTS *e);
};
extern AudioManager *TheAudio;
#endif
