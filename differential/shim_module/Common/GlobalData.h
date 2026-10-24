/* Stand-in for GameEngine/Include/Common/GlobalData.h, Module build only (differential tests, p1w12-sonnet, special power batch H): the one field
** SpecialPowerModule::createViewObject reads (and, in the debug build, m_specialPowerUsesDelay), in a class and global renamed so that they cannot
** clash with the GlobalData of the other builds (TheGlobalData is defined in differential/l9_powers_h.inc). */
#ifndef SHIM_MODULE_GLOBALDATA_H
#define SHIM_MODULE_GLOBALDATA_H
#include "Common/AsciiString.h"
#include "Lib/BaseType.h"
#define GlobalData ModTestPowGlobalData
#define TheGlobalData ModTestPowTheGlobalData
class GlobalData
{
public:
	AsciiString m_specialPowerViewObjectName;
	Bool m_specialPowerUsesDelay;
	/* --- p1w12-sonnet --- (update batch M: AutoHealBehavior's icon) */
	AsciiString m_getHealedAnimationName;
	Real m_getHealedAnimationDisplayTimeInSeconds;
	Real m_getHealedAnimationZRisePerSecond;
	/* --- end p1w12-sonnet --- */
	/* --- p1w13-sonnet --- (batch P: ActiveBody; set by differential/l9_active_body_p.inc) */
	Real m_unitDamagedThresh;
	Real m_unitReallyDamagedThresh;
	Real m_defaultStructureRubbleHeight;
	Real m_healthBonus[4];
	AsciiString m_autoFireParticleSmallPrefix, m_autoFireParticleSmallSystem; Int m_autoFireParticleSmallMax;
	AsciiString m_autoFireParticleMediumPrefix, m_autoFireParticleMediumSystem; Int m_autoFireParticleMediumMax;
	AsciiString m_autoFireParticleLargePrefix, m_autoFireParticleLargeSystem; Int m_autoFireParticleLargeMax;
	AsciiString m_autoSmokeParticleSmallPrefix, m_autoSmokeParticleSmallSystem; Int m_autoSmokeParticleSmallMax;
	AsciiString m_autoSmokeParticleMediumPrefix, m_autoSmokeParticleMediumSystem; Int m_autoSmokeParticleMediumMax;
	AsciiString m_autoSmokeParticleLargePrefix, m_autoSmokeParticleLargeSystem; Int m_autoSmokeParticleLargeMax;
	AsciiString m_autoAflameParticlePrefix, m_autoAflameParticleSystem; Int m_autoAflameParticleMax;
	Int m_maxTunnelCapacity = 0;   /* batch R10: TunnelTracker */
	Int m_timeOfDay = 0;   /* batch R11: CaveContain (a TimeOfDay) */
	Real m_gravity = -1.0f;   /* batch R29: StructureCollapseUpdate */
	Real m_standardMinefieldDensity = 0.01f, m_standardMinefieldDistance = 40.0f;
	Int m_baseValuePerSupplyBox = 100;   /* batch R44: the supply docks (set by the test) */   /* batch R41: GenerateMinefieldBehavior (GlobalData.cpp's values) */
	/* --- end p1w13-sonnet --- */
};
extern GlobalData *TheGlobalData;
#endif
