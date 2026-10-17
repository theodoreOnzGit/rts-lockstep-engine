/* Forced include (-include) for the W3D sources (WWLib chunkio, WW3D2
** loaders), differential tests only. Each block stands in for something the
** real build got from MSVC, windows.h or a header that cannot be compiled
** here. A header is replaced by defining its include guard first (the
** sources include their neighbours with quotes, which always finds the real
** file next to them, so a shim directory alone cannot replace it).
**
** 1. bittype.h (guard BITTYPE_H): the same typedefs with the Win32 data
**    model. Upstream's `typedef unsigned long uint32` is 32 bits on Win32 but
**    64 on x86_64 Linux, which would change every W3D struct and the chunk
**    header; `long` is spelled `int` here, nothing else changes.
** 2. always.h needs size_t before its operator new declarations; those
**    declarations route allocation to the game's memory manager, so they are
**    skipped (_OPERATOR_NEW_DEFINED_) and the standard allocator stands in.
**    The W3DMPO pool functions it declares are defined in wrap_w3d.cpp on
**    malloc/free.
** 3. mutex.h (guard MUTEX_H): its critical sections are MSVC x86 inline
**    assembly; the loaders run single-threaded, so the locks are no-ops.
** 4. wwmemlog.h (guard WWMEMLOG_H): memory-usage statistics only.
** 5. assetmgr.h (guard ASSETMGR_H): the animation loaders ask the asset
**    manager for the hierarchy named in their header (Get_HTree); the
**    stand-in returns the hierarchies the test has loaded (wrap_w3d.cpp).
** 6. The MSVC CRT names stricmp/_strlwr/_isnan. */
#ifndef SHIM_W3D_PRELUDE_H
#define SHIM_W3D_PRELUDE_H
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <math.h>
#include <new>

#define BITTYPE_H
typedef unsigned char	uint8;
typedef unsigned short	uint16;
typedef unsigned int	uint32;
typedef unsigned int	uint;
typedef signed char	sint8;
typedef signed short	sint16;
typedef signed int	sint32;
typedef signed int	sint;
typedef float		float32;
typedef double		float64;
typedef unsigned int	DWORD;
typedef unsigned short	WORD;
typedef unsigned char	BYTE;
typedef int		BOOL;
typedef unsigned short	USHORT;
typedef const char *	LPCSTR;
typedef unsigned int	UINT;
typedef unsigned int	ULONG;

#define _OPERATOR_NEW_DEFINED_

#define MUTEX_H
class CriticalSectionClass { public: class LockClass { public: LockClass(CriticalSectionClass &) {} }; };
class FastCriticalSectionClass { public: class LockClass { public: LockClass(FastCriticalSectionClass &) {} }; };
class MutexClass { public: class LockClass { public: LockClass(MutexClass &, int = 0) {} }; };

#define WWMEMLOG_H
#define WWMEMLOG(category)

#define ASSETMGR_H
class HTreeClass;
class TextureClass;
class WW3DAssetManager
{
public:
	static WW3DAssetManager *Get_Instance();
	HTreeClass *Get_HTree(const char *name);
	TextureClass *Get_Texture(const char *filename);
	TextureClass *Get_Texture(const char *filename, int mip_level_count, int texture_format);
	bool Get_Activate_Fog_On_Load() { return false; }
};

#define stricmp strcasecmp
#define _stricmp strcasecmp
#define strnicmp strncasecmp
#define _strnicmp strncasecmp
#define _isnan isnan
#endif
