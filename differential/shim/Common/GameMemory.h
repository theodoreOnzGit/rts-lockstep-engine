/* Shim for GameEngine/Include/Common/GameMemory.h, differential tests only.
** XferSave.cpp allocates its XferBlockData from a memory pool. The pool glue
** macros are replaced by plain new/delete, which changes no Xfer behaviour. */
#ifndef SHIM_GAMEMEMORY_H
#define SHIM_GAMEMEMORY_H
class MemoryPoolObject
{
public:
	virtual ~MemoryPoolObject() {}
	void deleteInstance() { delete this; }
};
#define MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(ARGCLASS, ARGPOOLNAME) public: virtual ~ARGCLASS();
#define EMPTY_DTOR(CLASS) CLASS::~CLASS() {}
#define newInstance(ARGCLASS) new ARGCLASS
#endif
