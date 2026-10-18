/* Stand-in for Include/Precompiled/PreRTS.h, STLTypedefs-with-UnicodeString
** build only (differential tests): differential/shim_stl/PreRTS.h, then the
** REAL AsciiString.h and UnicodeString.h, which UnicodeString.cpp expects the
** precompiled header to bring (it includes this header first, and then
** Common/CriticalSection.h, which shim_ascii supplies). This directory's
** Common/ holds forwarders to the real headers, so the stand-ins of
** shim_stl/Common (a UnicodeString whose compareNoCase aborts) are not seen. */
#ifndef SHIM_STL_UNICODE_PRERTS_H
#define SHIM_STL_UNICODE_PRERTS_H
#include "../shim_stl/PreRTS.h"
#include "Common/GameMemory.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#endif
