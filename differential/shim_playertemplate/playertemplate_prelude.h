/* Forced include (-include) for the PlayerTemplate build, differential tests
** only: the Science/RankInfo build's prelude (the real Science.h with
** `private` and `protected` made public, then the BitFlags build's prelude),
** and then the three typedefs of the real Common/STLTypedefs.h that
** PlayerTemplate.h uses and the stand-in STLTypedefs.h of the shims does not
** have (copied from GameEngine/Include/Common/STLTypedefs.h lines 83 and
** 112-113, after the headers that define NameKeyType and VeterancyLevel:
** the real header takes them from GameCommon.h and its own forward
** declarations). */
#ifndef SHIM_PLAYERTEMPLATE_PRELUDE_H
#define SHIM_PLAYERTEMPLATE_PRELUDE_H
#include "../shim_science/science_prelude.h"
#include "Common/NameKeyGenerator.h"
#include "Common/GameCommon.h"
typedef std::list< AsciiString >													AsciiStringList;
typedef std::map< NameKeyType, Real, std::less<NameKeyType> > ProductionChangeMap;
typedef std::map< NameKeyType, VeterancyLevel, std::less<NameKeyType> > ProductionVeterancyMap;
#endif
