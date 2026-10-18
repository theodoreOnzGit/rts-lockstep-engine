/* Shim for GameEngine/Include/Common/STLTypedefs.h, for the BitFlags build
** only (differential tests). The same as differential/shim/Common/
** STLTypedefs.h (see there) except that it does not declare a stand-in
** KindOfType: this build compiles the real Common/KindOf.h. (clang with
** -fms-extensions accepts the headers' MSVC-style forward declarations of
** enums such as Xfer.h's `enum KindOfType;`, as MSVC did.) */
#ifndef SHIM_STLTYPEDEFS_H
#define SHIM_STLTYPEDEFS_H
#include <vector>
#include <list>
#include <map>
#include <set>
#include <string.h>
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#include "Common/GameType.h"
namespace rts
{
	template<typename T> struct less_than_nocase
	{
		bool operator()(const T& __t1, const T& __t2) const
		{
			return (__t1 < __t2);
		}
	};
	template<> struct less_than_nocase<AsciiString>
	{
		bool operator()(const AsciiString& __t1, const AsciiString& __t2) const
		{
			return (__t1.compareNoCase(__t2) < 0);
		}
	};
}
#endif
