/* Shim for GameEngine/Include/Common/STLTypedefs.h, differential tests only.
** Xfer.h needs from it std::vector, std::list, AsciiString, UnicodeString,
** and (before its own `enum ObjectID;`-style forward declarations, which
** standard C++ accepts only for an enum already declared) the enums
** ObjectID, DrawableID (from the unchanged upstream Common/GameType.h,
** which needs only Lib/BaseType.h; they were copied here until the INI
** tests, where GameLogic/Damage.h includes GameType.h itself) and
** KindOfType (Common/KindOf.h; its enumerators are not needed here, only a
** 4-byte enum), and the containers and comparator the archive classes use. */
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
enum KindOfType
{
	KINDOF_INVALID = -1,
	KINDOF_SHIM_FORCE_INT = 0x7fffffff
};
/* rts::less_than_nocase<AsciiString> (real STLTypedefs.h lines 144-150 and
** 208-214), the ordering of FileSystem.h's FilenameList. */
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
