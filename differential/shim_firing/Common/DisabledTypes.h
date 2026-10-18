/* Stand-in for GameEngine/Include/Common/DisabledTypes.h, FiringTracker tests only: the mask
** type and DISABLEDMASK_ALL, which FiringTracker::getDisabledTypesToProcess returns (not
** compared by the tests; the Rust side is checked by its unit test). The wrapper renames both. */
#ifndef SHIM_FIRING_DISABLEDTYPES_H
#define SHIM_FIRING_DISABLEDTYPES_H
#include "Lib/BaseType.h"
class DisabledMaskType
{
public:
	DisabledMaskType() : m_bits(0) {}
	unsigned int m_bits;
};
extern DisabledMaskType DISABLEDMASK_ALL;
#endif
