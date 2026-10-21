/* Stand-in for GameEngine/Include/GameLogic/ExperienceTracker.h, Module build
** only (differential tests, p1w3-modules): DieModule.cpp includes it and uses
** nothing from it. */
/* --- p1w10-sonnet --- (ExperienceScalarUpgrade reads and writes the scalar; the change is a line of the effect log, ModTestObject) */
#ifndef SHIM_MODULE_EXPERIENCETRACKER_H
#define SHIM_MODULE_EXPERIENCETRACKER_H
#include "Lib/BaseType.h"
class ExperienceTracker
{
public:
	ExperienceTracker() : m_scalar(1.0f) {}
	Real getExperienceScalar() const { return m_scalar; }
	void setExperienceScalar(Real s) { m_scalar = s; }
	Real m_scalar;
};
#endif
/* --- end p1w10-sonnet --- */
