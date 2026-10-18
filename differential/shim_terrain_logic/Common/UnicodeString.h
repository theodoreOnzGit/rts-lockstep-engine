/* Stand-in for GameEngine/Include/Common/UnicodeString.h, for the TerrainLogic build only
** (differential tests): an empty class. It keeps the name, not a TL rename, so that Xfer's
** virtual xferUnicodeString(UnicodeString*) mangles as the Xfer library's does; no member of
** it is defined or called here, so it adds no symbol.
** The only user is the Dict stand-in of differential/shim_chunk that DataChunk.h includes
** (its Value struct holds one); this build never reads or writes a Dict, and the shared
** shim's UnicodeString cannot be compiled against the real BaseType.h this build uses
** (WideChar is wchar_t there, so its two constructors collide). */
#ifndef SHIM_TL_UNICODESTRING_H
#define SHIM_TL_UNICODESTRING_H
class UnicodeString
{
public:
	static const UnicodeString TheEmptyString;
};
#endif
