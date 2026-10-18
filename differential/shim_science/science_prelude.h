/* Forced include (-include) for the Science/RankInfo build, differential
** tests only. The BitFlags build's prelude (shim_flags/flags_prelude.h) ends
** by including the ScienceType enum through "Common/Science.h"; here that is
** the REAL header, included first with `private` and `protected` made public
** (the wrapper reads the stores' members), after the standard headers it and
** its includes use, so that those are not affected. */
#ifndef SHIM_SCIENCE_PRELUDE_H
#define SHIM_SCIENCE_PRELUDE_H
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <string>
#include <vector>
#include <list>
#include <map>
#include <set>
#include <bitset>
#include <algorithm>
#include <new>
#include "../shim/archive_prelude.h"
#include "Common/GameMemory.h"
class INI;
#define private public
#define protected public
#include "Common/Science.h"
#undef private
#undef protected
#include "../shim_flags/flags_prelude.h"
#endif
