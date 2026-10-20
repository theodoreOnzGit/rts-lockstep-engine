/* Stand-in for GameEngine/Include/Common/AudioEventRTS.h (the client's audio, not translated), Player build only
** (p1w10-sonnet): the event name and the player index, which is all the logic reads or sets; the sound is an id so that
** the log can say which one was asked for. The class is renamed by wrap_player.cpp. */
#ifndef PLAYER_SHIM_AUDIOEVENTRTS_H
#define PLAYER_SHIM_AUDIOEVENTRTS_H
#include "Common/AsciiString.h"
#include "Common/Snapshot.h"
#include "Common/SubsystemInterface.h"
#include "Common/GameMemory.h"
#include "Common/GameType.h"
class AudioEventRTS
{
public:
	AudioEventRTS(Int id = 0) : m_id(id), m_playerIndex(0) {}
	void setEventName(AsciiString name) { m_eventName = name; }
	void setPlayerIndex(Int ndx) { m_playerIndex = ndx; }
	AsciiString m_eventName;
	Int m_id;
	Int m_playerIndex;
};
#endif
