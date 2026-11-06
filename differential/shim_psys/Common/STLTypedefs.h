/* Shim for GameEngine/Include/Common/STLTypedefs.h, for the ParticleSys build only
** (differential tests): differential/shim_terrain/Common/STLTypedefs.h (no
** UnicodeString stand-in: the real BaseType.h makes WideChar a wchar_t) plus what
** ParticleSys.h uses: rts::hash, rts::equal_to and std::hash_map (the template map).
** As in differential/shim_armor/Common/STLTypedefs.h, std::hash_map is an ORDERED
** std::map that ignores the hash and equality functors (g++ and clang have no
** hash_map); ParticleSys.cpp iterates the template map only in findParentTemplate,
** preloadAssets and the destructor, none of which the tests compare. */
#ifndef SHIM_PSYS_STLTYPEDEFS_H
#define SHIM_PSYS_STLTYPEDEFS_H
#include <vector>
#include <list>
#include <map>
#include <set>
#include <string.h>
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/GameType.h"
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
	template<typename T> struct hash { size_t operator()(const T &t) const { return (size_t)t; } };
	template<typename T> struct equal_to { bool operator()(const T &a, const T &b) const { return a == b; } };
}
namespace std
{
	template<typename K, typename V, typename H, typename E> using hash_map = map<K, V>;
}
#endif
