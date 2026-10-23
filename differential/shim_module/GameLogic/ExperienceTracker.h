/* Stand-in for GameEngine/Include/GameLogic/ExperienceTracker.h, Module build
** only (differential tests, p1w3-modules): DieModule.cpp includes it and uses
** nothing from it. */
/* --- p1w10-sonnet --- (ExperienceScalarUpgrade reads and writes the scalar; the change is a line of the effect log, ModTestObject) */
#ifndef SHIM_MODULE_EXPERIENCETRACKER_H
#define SHIM_MODULE_EXPERIENCETRACKER_H
#include "Lib/BaseType.h"
#include "Common/GameCommon.h"   /* batch R5: VeterancyLevel */
class ExperienceTracker
{
public:
	ExperienceTracker() : m_scalar(1.0f) {}
	Real getExperienceScalar() const { return m_scalar; }
	void setExperienceScalar(Real s) { m_scalar = s; }
	Real m_scalar;
	/* --- p1w13-sonnet --- (batch R5: RiderChangeContain moves veterancy between bike and rider; differential/l9_open_contain_r.inc logs the calls) */
	/* asm labels: the experience tracker's own differential test links the real ExperienceTracker.cpp, whose symbols these would duplicate */
	VeterancyLevel getVeterancyLevel() const __asm__("l9m_ExperienceTracker_getVeterancyLevel");
	void setVeterancyLevel( VeterancyLevel newLevel, Bool provideFeedback = TRUE ) __asm__("l9m_ExperienceTracker_setVeterancyLevel");
	void setExperienceAndLevel( Int experienceIn, Bool provideFeedback = TRUE ) __asm__("l9m_ExperienceTracker_setExperienceAndLevel");
	void setExperienceSink( ObjectID sink ) __asm__("l9m_ExperienceTracker_setExperienceSink");   /* batch R6: OverlordContain */
	/* batch R13b: VeterancyCrateCollide asks whether the object can train and gain levels, and gives them (differential/l9_crates_r13b.inc) */
	Bool isTrainable() const __asm__("l9m_ExperienceTracker_isTrainable");
	Bool canGainExpForLevel(Int levelsToGain) const __asm__("l9m_ExperienceTracker_canGainExpForLevel");
	Bool gainExpForLevel(Int levelsToGain, Bool canScaleForBonus = TRUE) __asm__("l9m_ExperienceTracker_gainExpForLevel");
	Bool m_trainable = false;
	ObjectID m_sink = INVALID_ID;
	class Object *m_owner = 0;
	/* --- end p1w13-sonnet --- */
};
#endif
/* --- end p1w10-sonnet --- */
