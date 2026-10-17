/* Shim for GameEngine/Include/Common/GameAudio.h, differential tests only:
** Win32BIGFileSystem::closeArchiveFile stops the music through TheAudio when
** Music.big is closed. Only that call is declared; it is not exercised. */
#ifndef SHIM_GAMEAUDIO_H
#define SHIM_GAMEAUDIO_H
#include "Common/AudioAffect.h"
class AudioManager
{
public:
	virtual ~AudioManager() {}
	virtual void stopAudio( AudioAffect which ) = 0;
};
extern AudioManager *TheAudio;
#endif
