/* Forced include (-include) for DataChunk.cpp and NameKeyGenerator.cpp,
** differential tests only. The real precompiled header PreRTS.h brings in,
** before their own includes: Common/NameKeyGenerator.h (NameKeyGenerator.cpp
** does not include its own header) and, through windows.h, the `max` macro
** DataChunkTableOfContents::read uses (here a function template with the
** macro's meaning, so the C++ standard headers stay intact), and
** Common/INI.h for NameKeyGenerator's INI field parser (with what INI.h
** itself needs first, as in shim_ini/ini_prelude.h: <bitset>, ScienceType,
** BaseType.h's PI). The .big prelude is reused for the MSVC string names. */
#ifndef SHIM_CHUNK_PRELUDE_H
#define SHIM_CHUNK_PRELUDE_H
#include "../shim/archive_prelude.h"
#include <bitset>
#include "Common/Errors.h"
#include "Common/Science.h"
#ifndef PI
#define PI 3.14159265359f
#endif
#include "Common/NameKeyGenerator.h"
#include "Common/INI.h"
template <class T> inline T max(T a, T b) { return (a > b) ? a : b; }
#endif
