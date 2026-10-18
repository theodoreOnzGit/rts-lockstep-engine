/* Shim for GameEngine/Include/Common/STLTypedefs.h, for the TerrainLogic build only
** (differential tests): differential/shim_terrain/Common/STLTypedefs.h (no UnicodeString
** include) plus the 4-byte KindOfType enum of differential/shim/Common/STLTypedefs.h that
** Xfer.h's `enum KindOfType;` forward declaration needs. */
#ifndef SHIM_TL_STLTYPEDEFS_H
#define SHIM_TL_STLTYPEDEFS_H
#include <vector>
#include <list>
#include <map>
#include <set>
#include <string.h>
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/GameType.h"
enum KindOfType
{
	KINDOF_INVALID = -1,
	KINDOF_SHIM_FORCE_INT = 0x7fffffff
};
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
