/* Shim for GameEngine/Include/Common/Science.h, differential tests only.
** ScienceType and ScienceVec are copied from the real header (lines 43-49);
** ScienceStore is a stand-in for the two lookups Xfer::xferScienceType calls
** and the one INI::scanScience calls
** (NOT translated, NOT tested: the stand-ins abort if reached). */
#ifndef SHIM_SCIENCE_H
#define SHIM_SCIENCE_H
#include <vector>
#include "Common/AsciiString.h"
enum ScienceType
{
	SCIENCE_INVALID = -1
};
typedef std::vector<ScienceType> ScienceVec;
class ScienceStore
{
public:
	AsciiString getInternalNameForScience(ScienceType) const;
	ScienceType getScienceFromInternalName(const AsciiString &) const;
	ScienceType friend_lookupScience(const char *) const;
};
extern ScienceStore *TheScienceStore;
#endif
