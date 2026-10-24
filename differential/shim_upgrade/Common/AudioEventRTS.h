/* Stand-in for GameEngine/Include/Common/AudioEventRTS.h, Upgrade/SpecialPower
** build only (differential tests, p1w2-upgrade). The real class is the
** client's audio event; the templates hold two of them and INI sets only
** their names (INI::parseAudioEventRTS -> setEventName, in INI.cpp). So this
** is a class that keeps the name, the cut point of the translation. The
** out-of-line setEventName is defined in differential/wrap_upgrade.cpp (the
** INI build calls it by this mangled name). */
#ifndef SHIM_UPGRADE_AUDIOEVENTRTS_H
#define SHIM_UPGRADE_AUDIOEVENTRTS_H
#include "Common/AsciiString.h"
/* --- p1w11-sonnet --- (die batch D: what EjectPilotDie sets on the copy of the template's event before it hands it to the audio manager) */
#include "Lib/BaseType.h"
/* --- end p1w11-sonnet --- */
class AudioEventRTS
{
private:
	AsciiString m_eventName;   /* p1w11-sonnet: the FIRST member, as the ObjectCreationList fixture's stand-in (shim_ocl) relies on */
public:
	/* --- p1w12-sonnet --- (update batch N: FlammableUpdate's burning sound) */
	AudioEventRTS( const AsciiString &eventName, ObjectID ownerID ) : m_eventName( eventName ), m_hasPos(false), m_playerIndex(-1), m_objectID(ownerID) { m_pos.x = m_pos.y = m_pos.z = 0; }
	/* --- end p1w12-sonnet --- */
	AudioEventRTS() : m_hasPos(false), m_playerIndex(-1), m_objectID((ObjectID)0) { m_pos.x = m_pos.y = m_pos.z = 0; }
	void setEventName(AsciiString name);
	const AsciiString &getEventName() const { return m_eventName; }
	/* --- p1w11-sonnet --- */
	void setPosition(const Coord3D *pos) { m_pos = *pos; m_hasPos = true; }
	void setPlayerIndex(Int playerIndex) { m_playerIndex = playerIndex; }
	void setObjectID(ObjectID objID) { m_objectID = objID; }
	Coord3D m_pos;
	Bool m_hasPos;
	Int m_playerIndex;
	ObjectID m_objectID;
	/* --- end p1w11-sonnet --- */
	/* --- p1w13-sonnet --- (batch R34: TensileFormationUpdate's crack sound; isCurrentlyPlaying asks TheAudio, defined by differential/l9_tensile_r34.inc) */
	UnsignedInt m_playingHandle = 0;
	void setPlayingHandle(UnsignedInt handle) { m_playingHandle = handle; }
	UnsignedInt getPlayingHandle() const { return m_playingHandle; }
	Bool isCurrentlyPlaying() const;
	/* --- end p1w13-sonnet --- */
};
/* --- p1w12-sonnet --- */
typedef UnsignedInt AudioHandle;
/* --- end p1w12-sonnet --- */
#endif
