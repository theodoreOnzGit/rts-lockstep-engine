/* Shim for GameEngine/Include/Common/STLTypedefs.h, differential tests only.
** Xfer.h needs from it std::vector, std::list, AsciiString, UnicodeString,
** and (before its own `enum ObjectID;`-style forward declarations, which
** standard C++ accepts only for an enum already declared) the enums
** ObjectID, DrawableID (copied from Common/GameType.h lines 41-52) and
** KindOfType (Common/KindOf.h; its enumerators are not needed here, only a
** 4-byte enum). */
#ifndef SHIM_STLTYPEDEFS_H
#define SHIM_STLTYPEDEFS_H
#include <vector>
#include <list>
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
enum ObjectID
{
	INVALID_ID = 0,
	FORCE_OBJECTID_TO_LONG_SIZE = 0x7ffffff
};
enum DrawableID
{
	INVALID_DRAWABLE_ID = 0,
	FORCE_DRAWABLEID_TO_LONG_SIZE = 0x7ffffff
};
enum KindOfType
{
	KINDOF_INVALID = -1,
	KINDOF_SHIM_FORCE_INT = 0x7fffffff
};
#endif
