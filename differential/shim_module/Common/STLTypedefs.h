/* Shim for GameEngine/Include/Common/STLTypedefs.h, Module build only (differential tests, p1w13-sonnet, batch P): the BitFlags build's shim
** (differential/shim_flags/Common/STLTypedefs.h) plus what Armor.h and DamageFX.h use, as the Armor build's shim has them: rts::hash and rts::equal_to (the
** functors of upstream's hash_maps) and std::hash_map as an ORDERED std::map that ignores the hash and equality functors (nothing iterates these maps
** upstream, so the order is unobservable). */
#ifndef SHIM_MODULE_STLTYPEDEFS_H
#define SHIM_MODULE_STLTYPEDEFS_H
#include "../../shim_flags/Common/STLTypedefs.h"
namespace rts
{
	template<typename T> struct hash { size_t operator()(const T &t) const { return (size_t)t; } };
	template<typename T> struct equal_to { bool operator()(const T &a, const T &b) const { return a == b; } };
}
/* STLTypedefs.h's AsciiString list (batch R14c: CreateCrateDie) */
#include <list>
#include "Common/AsciiString.h"
typedef std::list<AsciiString> AsciiStringList;
typedef std::list<AsciiString>::iterator AsciiStringListIterator;
typedef std::list<AsciiString>::const_iterator AsciiStringListConstIterator;
/* STLTypedefs.h lines 91, 95 and 109, verbatim (batch R43: DockUpdate.h) */
#include <vector>
typedef std::vector<ObjectID>															ObjectIDVector;
typedef std::vector<Coord3D> VecCoord3D;
typedef std::vector<Bool>																	BoolVector;
namespace std
{
	template<typename K, typename V, typename H, typename E> using hash_map = map<K, V>;
}
#endif
