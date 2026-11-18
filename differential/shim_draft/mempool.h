/* Stand-in for WWLib/mempool.h, for the draft culling-system differential tests only (#215, build.rs drafts_wwmath).
** Upstream's ObjectPoolClass keeps each block's link to the previous block in the block's first sizeof(void *) bytes, but
** starts the objects at `BlockListHead + 1`, a `uint32 *`: on 32-bit Win32 that is 4 bytes in, past the 4-byte link. On
** x86_64 the link is 8 bytes, the first object's free-list link overwrites half of it, and ~ObjectPoolClass (run at
** process exit for the static pools of AABTreeNodeClass, AABTreeLinkClass and GridLinkClass) follows a corrupt link and
** crashes. This keeps the interface and the pooling (a free list of fixed-size objects, blocks of BLOCK_SIZE) with a
** 16-byte block header. The pool is allocation machinery: no behaviour of the culling systems depends on it. */
#ifndef MEMPOOL_H
#define MEMPOOL_H

#include <new>
#include <stdlib.h>
#include <stddef.h>

template<class T,int BLOCK_SIZE = 64>
class ObjectPoolClass
{
public:
	ObjectPoolClass(void) : FreeListHead(NULL), BlockListHead(NULL), FreeObjectCount(0), TotalObjectCount(0) { }
	~ObjectPoolClass(void)
	{
		while (BlockListHead != NULL) {
			char * next_block = *(char **)BlockListHead;
			::operator delete(BlockListHead);
			BlockListHead = next_block;
		}
	}
	T * Allocate_Object(void) { T * obj = Allocate_Object_Memory(); return new (obj) T; }
	void Free_Object(T * obj) { obj->T::~T(); Free_Object_Memory(obj); }
	T * Allocate_Object_Memory(void)
	{
		if (FreeListHead == 0) {
			char * tmp_block_head = BlockListHead;
			BlockListHead = (char *)::operator new(sizeof(T) * BLOCK_SIZE + 16);
			*(char **)BlockListHead = tmp_block_head;
			FreeListHead = (T*)(BlockListHead + 16);
			for (int i = 0; i < BLOCK_SIZE; i++) {
				*(T**)(&(FreeListHead[i])) = &(FreeListHead[i+1]);
			}
			*(T**)(&(FreeListHead[BLOCK_SIZE-1])) = 0;
			FreeObjectCount += BLOCK_SIZE;
			TotalObjectCount += BLOCK_SIZE;
		}
		T * obj = FreeListHead;
		FreeListHead = *(T**)(FreeListHead);
		FreeObjectCount--;
		return obj;
	}
	void Free_Object_Memory(T * obj)
	{
		*(T**)(obj) = FreeListHead;
		FreeListHead = obj;
		FreeObjectCount++;
	}
protected:
	T *		FreeListHead;
	char *	BlockListHead;
	int		FreeObjectCount;
	int		TotalObjectCount;
};

template<class T,int BLOCK_SIZE = 64>
class AutoPoolClass
{
public:
	static void * operator new(size_t size) { return (void *)(Allocator.Allocate_Object_Memory()); }
	static void operator delete(void * memory) { if (memory == 0) return; Allocator.Free_Object_Memory((T*)memory); }
private:
	static void * operator new [] (size_t size);
	static void operator delete[] (void * memory);
	static ObjectPoolClass<T,BLOCK_SIZE>	Allocator;
};

#define DEFINE_AUTO_POOL(T,BLOCKSIZE) \
ObjectPoolClass<T,BLOCKSIZE> AutoPoolClass<T,BLOCKSIZE>::Allocator;

#endif // MEMPOOL_H
