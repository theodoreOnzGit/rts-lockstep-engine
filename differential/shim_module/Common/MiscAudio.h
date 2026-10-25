/* Stand-in for GameEngine/Include/Common/MiscAudio.h, Module build only (differential tests, p1w12-sonnet, crate batch F): the sounds the crate
** modules copy. The events are the test's: each is named after its field. Renamed ModTestMiscAudio by module_prelude.h. */
#ifndef SHIM_MODULE_MISCAUDIO_H
#define SHIM_MODULE_MISCAUDIO_H
#include "Common/AudioEventRTS.h"
class MiscAudio
{
public:
	AudioEventRTS m_moneyWithdrawSound;
	AudioEventRTS m_crateHeal;
	AudioEventRTS m_crateShroud;
	AudioEventRTS m_crateSalvage;
	AudioEventRTS m_crateFreeUnit;
	AudioEventRTS m_crateMoney;
	AudioEventRTS m_sabotageShutDownBuilding;
	AudioEventRTS m_sabotageResetTimerBuilding;
	AudioEventRTS m_defectorTimerTickSound;
	AudioEventRTS m_defectorTimerDingSound;
	AudioEventRTS m_stealthDiscoveredSound;   /* p1w13-sonnet, batch R27: StealthDetectorUpdate */
	AudioEventRTS m_stealthNeutralizedSound;
	AudioEventRTS m_repairSparks;   /* p1w13-sonnet, batch R57: SlavedUpdate */
};
#endif
