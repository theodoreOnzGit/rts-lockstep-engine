/* Stand-in for GameEngine/Include/Common/AudioEventRTS.h (the client's audio, not translated), Weapon tests
** only: the event name, which is all `WeaponTemplate` keeps of a fire sound for the logic. `INI::parseAudioEventRTS`
** (INI.cpp, upstream_ini) calls `AudioEventRTS::setEventName` and `AudioManager::getInfoForAudioEvent`, which the
** Upgrade fixture (differential/wrap_upgrade.cpp) defines on a class with this same single member, `m_eventName`
** (differential/shim_upgrade/Common/AudioEventRTS.h): the layouts must match. Weapon.h also needs Snapshot and
** SubsystemInterface, which the real AudioEventRTS.h brings. */
#ifndef SHIM_WEAPON_AUDIOEVENTRTS_H
#define SHIM_WEAPON_AUDIOEVENTRTS_H
#include "Common/AsciiString.h"
#include "Common/Snapshot.h"
#include "Common/SubsystemInterface.h"
#include "Common/GameMemory.h"
#include "Common/GameType.h"
class AudioEventRTS
{
public:
	void setEventName(AsciiString name);
	AsciiString m_eventName;
};
#endif
