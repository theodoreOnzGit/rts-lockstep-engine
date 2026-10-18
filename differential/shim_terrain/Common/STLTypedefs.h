/* Shim for GameEngine/Include/Common/STLTypedefs.h, for the Terrain, Multiplayer
** and Crate build only (differential tests): differential/shim_flags/Common/
** STLTypedefs.h without its include of the UnicodeString stand-in. That
** stand-in is written for a 16-bit WideChar, and the real Lib/BaseType.h this
** build uses (for RGBColor) makes WideChar a wchar_t: the two constructors
** would be the same. Nothing in this build uses UnicodeString. */
#ifndef SHIM_TERRAIN_STLTYPEDEFS_H
#define SHIM_TERRAIN_STLTYPEDEFS_H
#include <vector>
#include <list>
#include <map>
#include <set>
#include <string.h>
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
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
