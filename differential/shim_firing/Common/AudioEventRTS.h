/* Stand-in for GameEngine/Include/Common/AudioEventRTS.h (layer 4 audio, not translated),
** FiringTracker tests only: an event (its name and the object it is about) and the handle
** type. The wrapper renames the class (FireTest*, wrap_firing_tracker.cpp). */
#ifndef SHIM_FIRING_AUDIOEVENTRTS_H
#define SHIM_FIRING_AUDIOEVENTRTS_H
#include "Common/AsciiString.h"
#include "Common/GameType.h"
typedef UnsignedInt AudioHandle;
class AudioEventRTS
{
public:
	AudioEventRTS() : m_objectID(INVALID_ID) {}
	AudioEventRTS(const AsciiString &name) : m_eventName(name), m_objectID(INVALID_ID) {}
	void setObjectID(ObjectID id) { m_objectID = id; }
	ObjectID getObjectID() const { return m_objectID; }
	AsciiString m_eventName;
	ObjectID m_objectID;
};
#endif
