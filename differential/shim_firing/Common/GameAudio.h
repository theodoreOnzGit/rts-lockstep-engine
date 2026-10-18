/* Stand-in for GameEngine/Include/Common/GameAudio.h (layer 4 audio, not translated),
** FiringTracker tests only: the three AudioManager members FiringTracker.cpp calls and
** TheAudio. The wrapper renames them and logs each call (wrap_firing_tracker.cpp). */
#ifndef SHIM_FIRING_GAMEAUDIO_H
#define SHIM_FIRING_GAMEAUDIO_H
#include "Common/AudioEventRTS.h"
class AudioManager
{
public:
	AudioHandle addAudioEvent(const AudioEventRTS *event);
	void removeAudioEvent(AudioHandle handle);
	Bool isCurrentlyPlaying(AudioHandle handle) const;
};
extern AudioManager *TheAudio;
#endif
