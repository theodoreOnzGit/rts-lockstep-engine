/* Copy of differential/shim/Common/GameMemory.h for the Science/RankInfo build,
** with `new(ShimZeroFill())` spelled `new(ShimZeroFill{})`: clang reads the
** first as a function-type allocation (a vexing parse) in `newInstance(X)`.
** Original comment follows.
**
** Shim for GameEngine/Include/Common/GameMemory.h, differential tests only.
** XferSave.cpp (XferBlockData) and the file classes (File, RAMFile,
** StreamingArchiveFile) allocate from memory pools. The pool glue macros are
** replaced by plain new/delete, which changes no behaviour under test, with
** one thing kept from the real allocator: every allocation through it is
** ZERO-FILLED (GameMemory.cpp: DynamicMemoryAllocator::allocateBytesImplementation
** and MemoryPool::allocateBlockImplementation both memset 0). Upstream code
** depends on that: ArchiveFile's constructor never initialises m_file, and
** attachFile() then closes it if it is not NULL. */
#ifndef SHIM_GAMEMEMORY_H
#define SHIM_GAMEMEMORY_H
#include <new>
#include <string.h>
struct ShimZeroFill {};
inline void *operator new(size_t n, ShimZeroFill) { void *p = ::operator new(n); memset(p, 0, n); return p; }
inline void *operator new[](size_t n, ShimZeroFill) { void *p = ::operator new[](n); memset(p, 0, n); return p; }
inline void operator delete(void *p, ShimZeroFill) { ::operator delete(p); }
inline void operator delete[](void *p, ShimZeroFill) { ::operator delete[](p); }
class MemoryPoolObject
{
public:
	virtual ~MemoryPoolObject() {}
	void deleteInstance() { delete this; }
};
#define MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(ARGCLASS, ARGPOOLNAME) public: virtual ~ARGCLASS();
#define EMPTY_DTOR(CLASS) inline CLASS::~CLASS() { }  /* as upstream, GameMemory.h line 903 */
#define MEMORY_POOL_GLUE_ABC(ARGCLASS) protected: virtual ~ARGCLASS();
#define newInstance(ARGCLASS) new(ShimZeroFill{}) ARGCLASS
#define NEW new(ShimZeroFill{})
#define MSGNEW(MSG) new(ShimZeroFill{})
#endif
