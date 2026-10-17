/* Shim for GameEngine/Include/Common/GameAudio.h, differential tests only:
** Win32BIGFileSystem::closeArchiveFile stops the music through TheAudio when
** Music.big is closed, and INI.cpp's audio-event field parsers look events
** up through it. Only those calls are declared, with stand-ins for the
** AudioEventRTS classes they take; none is exercised. */
#ifndef SHIM_GAMEAUDIO_H
#define SHIM_GAMEAUDIO_H
#include "Common/AudioAffect.h"
#include "Common/AsciiString.h"
#include "Common/GameMemory.h"
class AudioEventRTS
{
public:
	void setEventName(AsciiString name);
};
class DynamicAudioEventRTS : public MemoryPoolObject
{
public:
	AudioEventRTS m_event;
};
class AudioManager
{
public:
	virtual ~AudioManager() {}
	virtual void stopAudio( AudioAffect which ) = 0;
	void getInfoForAudioEvent( const AudioEventRTS *eventToFindAndFill ) const;
};
extern AudioManager *TheAudio;
#endif
