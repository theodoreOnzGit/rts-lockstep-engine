/* Stand-in for GameEngine/Include/Common/GameAudio.h, AI build only (differential tests, batch A7; shadows differential/shim's):
** the audio event the move states copy from the template, name its object in and hand to TheAudio, and TheAudio's two calls
** (logged by the wrapper). PreRTS.h renames the classes Ai*. */
#ifndef SHIM_AI_GAMEAUDIO_H
#define SHIM_AI_GAMEAUDIO_H
#include "Common/AudioAffect.h"
#include "Common/AsciiString.h"
#include "Common/GameMemory.h"
#include "Common/GameType.h"
#include "Common/AudioHandleSpecialValues.h"
typedef UnsignedInt AudioHandle;
class AudioEventRTS
{
public:
	AudioEventRTS() : m_objectID(INVALID_ID) {}
	void setEventName(AsciiString name) { m_eventName = name; }
	const AsciiString& getEventName() const { return m_eventName; }
	void setObjectID(ObjectID id) { m_objectID = id; }
	ObjectID getObjectID() const { return m_objectID; }
	/* batch A35 (the depleted voice keeps its handle) */
	void setPlayingHandle(AudioHandle h) { m_playingHandle = h; }
	AudioHandle m_playingHandle;
	AsciiString m_eventName;
	ObjectID m_objectID;
};
class DynamicAudioEventRTS : public MemoryPoolObject
{
public:
	AudioEventRTS m_event;
};
class AudioManager
{
public:
	AudioHandle addAudioEvent( const AudioEventRTS *eventToAdd );
	void removeAudioEvent( AudioHandle audioEvent );
};
extern AudioManager *TheAudio;
#endif
