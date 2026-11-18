/* (#215) STAND-IN for WWSaveLoad/persistfactory.h in the curve and spline wrappers (curve.cpp, hermitespline.cpp,
** cardinalspline.cpp, catmullromspline.cpp, tcbspline.cpp). The real header pulls in the save-load system (saveload.h,
** chunkio.h with its file classes, the factory registry), none of which the curves' numeric functions touch. What the .cpp files
** need of it, and nothing more:
**   - SimplePersistFactoryClass<T, ID>, the type of the `_XxxFactory` globals that Get_Factory returns (here an empty class);
**   - ChunkSaveClass and ChunkLoadClass with the members Save and Load call, as inline no-ops (a chunk file is not written or
**     read; the curves' Save and Load are NOT covered by these tests);
**   - the two micro chunk macros the Save and Load functions use, copied verbatim from WWLib/chunkio.h. */
#ifndef PERSISTFACTORY_H
#define PERSISTFACTORY_H

#include "always.h"
#include "wwdebug.h"
#include "persist.h"

class PersistFactoryClass
{
public:
	virtual ~PersistFactoryClass(void) {}
};

template <class T, int CHUNKID>
class SimplePersistFactoryClass : public PersistFactoryClass
{
};

class ChunkSaveClass
{
public:
	bool Begin_Chunk(unsigned int) { return true; }
	bool End_Chunk(void) { return true; }
	bool Begin_Micro_Chunk(unsigned int) { return true; }
	bool End_Micro_Chunk(void) { return true; }
	unsigned int Write(const void *, unsigned int size) { return size; }
};

class ChunkLoadClass
{
public:
	bool Open_Chunk(void) { return false; }
	bool Close_Chunk(void) { return true; }
	unsigned int Cur_Chunk_ID(void) { return 0; }
	bool Open_Micro_Chunk(void) { return false; }
	bool Close_Micro_Chunk(void) { return true; }
	unsigned int Cur_Micro_Chunk_ID(void) { return 0; }
	unsigned int Read(void *, unsigned int size) { return size; }
};

#define WRITE_MICRO_CHUNK(csave,id,var) { \
	csave.Begin_Micro_Chunk(id); \
	csave.Write(&var,sizeof(var)); \
	csave.End_Micro_Chunk(); }

#define READ_MICRO_CHUNK(cload,id,var)						\
	case (id):	cload.Read(&var,sizeof(var)); break;	\

#endif
