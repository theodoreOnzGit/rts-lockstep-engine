/* Shim for GameEngine/Include/Common/GameMemory.h, for the Dict build only
** (differential tests): differential/shim/Common/GameMemory.h (see there),
** plus the dynamic allocator Dict.cpp allocates its pair arrays from.
** allocateBytes zero-fills, as the real DynamicMemoryAllocator does (Dict
** relies on it: a new pair's value slot is 0). getActualAllocationSize
** returns the size asked for, where the real one rounds up to its pool
** sizes; that changes only how many spare pairs an array has, i.e. when
** Dict reallocates, never a value. */
#ifndef SHIM_DICT_GAMEMEMORY_H
#define SHIM_DICT_GAMEMEMORY_H
#include <stdlib.h>
#include "../../shim/Common/GameMemory.h"
class DynamicMemoryAllocator
{
public:
	int getActualAllocationSize(int n) { return n; }
	void *allocateBytes(int n, const char *) { return calloc(1, (size_t)n); }
	void freeBytes(void *p) { free(p); }
};
extern DynamicMemoryAllocator *TheDynamicMemoryAllocator;
#endif
