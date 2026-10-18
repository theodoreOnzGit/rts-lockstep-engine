/* Stand-in for GameEngine/Include/Common/ModelState.h, FiringTracker tests only: the three
** MODELCONDITION_CONTINUOUS_FIRE_* states (indices as upstream's enum, with the ALLOW_SURRENDER
** entry absent, as in the shipped build: 84, 85, 86, the values src/common/model_state.rs has) and
** ModelConditionFlags, a 128-bit set with the set/clear/test the tracker uses. */
#ifndef SHIM_FIRING_MODELSTATE_H
#define SHIM_FIRING_MODELSTATE_H
#include "Lib/BaseType.h"
enum ModelConditionType
{
	MODELCONDITION_CONTINUOUS_FIRE_SLOW = 84,
	MODELCONDITION_CONTINUOUS_FIRE_MEAN = 85,
	MODELCONDITION_CONTINUOUS_FIRE_FAST = 86
};
class ModelConditionFlags
{
public:
	ModelConditionFlags() { m_bits[0] = m_bits[1] = 0; }
	void set(int i) { m_bits[i / 64] |= 1ULL << (i % 64); }
	void clear(int i) { m_bits[i / 64] &= ~(1ULL << (i % 64)); }
	bool test(int i) const { return (m_bits[i / 64] >> (i % 64)) & 1ULL; }
	unsigned long long m_bits[2];
};
#endif
