/* Stand-in for GameEngine/Include/Common/AudioEventRTS.h, ObjectCreationList fixture only (differential/wrap_ocl.cpp, which renames the
** class OclTestAudioEventRTS). ObjectCreationList.cpp keeps one of them (the bounce sound a CreateDebris names) and makes one for the fade
** sound of a faded object; what it does with them is `INI::parseAudioEventRTS` (INI.cpp, upstream_ini), which calls
** `AudioEventRTS::setEventName` and `AudioManager::getInfoForAudioEvent` on the UNRENAMED class of the Upgrade fixture, whose layout is one
** `AsciiString m_eventName`: so m_eventName is the first member here too, and the second member is only ever written by this fixture. */
#ifndef SHIM_OCL_AUDIOEVENTRTS_H
#define SHIM_OCL_AUDIOEVENTRTS_H
#include "Common/AsciiString.h"
#include "Common/Snapshot.h"
#include "Common/SubsystemInterface.h"
#include "Common/GameMemory.h"
#include "Common/GameType.h"
class AudioEventRTS
{
public:
	AudioEventRTS() : m_obj((ObjectID)0) {}
	AudioEventRTS(const AsciiString& name) : m_eventName(name), m_obj((ObjectID)0) {}
	void setEventName(AsciiString name) { m_eventName = name; }
	void setObjectID(ObjectID id) { m_obj = id; }
	AsciiString m_eventName;
	ObjectID m_obj;
};
#endif
