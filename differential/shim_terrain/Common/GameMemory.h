/* Shim for GameEngine/Include/Common/GameMemory.h, for the Terrain, Multiplayer
** and Crate build only (differential tests): differential/shim's GameMemory.h
** (the zero-filling allocator, the pool glue as plain new/delete), with
** newInstance spelled with a braced placement argument. Its parenthesised
** `new(ShimZeroFill())` is read by clang as a function type when the class
** is a plain name (TerrainTypes.cpp's newInstance(TerrainType)). */
#include "../../shim/Common/GameMemory.h"
#undef newInstance
#define newInstance(ARGCLASS) new (ShimZeroFill{}) ARGCLASS
#undef NEW
#define NEW new (ShimZeroFill{})
#undef MSGNEW
#define MSGNEW(MSG) new (ShimZeroFill{})
