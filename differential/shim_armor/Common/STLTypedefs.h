/* Shim for GameEngine/Include/Common/STLTypedefs.h, for the Armor/Damage/
** DamageFX build only (differential tests): the BitFlags build's shim
** (differential/shim_flags/Common/STLTypedefs.h, which does not declare a
** stand-in KindOfType) plus what Armor.h and DamageFX.h use:
** rts::hash and rts::equal_to (the functors of upstream's hash_maps), and
** std::hash_map. g++ and clang have no std::hash_map; here it is an
** ORDERED std::map that ignores the hash and equality functors, which is
** also what the Rust translation uses (nothing iterates these maps
** upstream, so the order is unobservable). */
#ifndef SHIM_ARMOR_STLTYPEDEFS_H
#define SHIM_ARMOR_STLTYPEDEFS_H
#include <vector>
#include <list>
#include <map>
#include <set>
#include <string.h>
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
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
