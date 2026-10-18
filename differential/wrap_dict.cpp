/* Differential-test wrapper for GameEngine/Source/Common/Dict.cpp and
** Include/Common/Dict.h (upstream commit 0a05454d), compiled unchanged with
** the pointer-sized string shims and the allocator of differential/shim_dict
** (see those files).
**
** ref_dict_run interprets a script on 4 Dict slots and logs every result
** (all values little-endian):
**   0 s n:i32              slot s = Dict(n)
**   1 s                    clear()
**   2 s key:u32 v:u8       setBool
**   3 s key v:i32          setInt
**   4 s key v:f32 bits     setReal
**   5 s key len:u16 bytes  setAsciiString
**   6 s key len:u16 u16s   setUnicodeString
**   7 s key                remove -> u8
**   8 s t key              s.copyPairFrom(t, key)
**   9 s t                  s = t (operator=)
**  10 s t                  slot s = Dict(t) (copy constructor)
**  11 s                    dump: count:i32, then per pair getNthKey:u32,
**                          getNthType:i32 and the value by getNth<type>
**  12 s key                getType:i32, then each of the five getters'
**                          value and exists:u8
**  13 s n:i32              getNthKey:u32, getNthType:i32 (any n)
**  14 s n:i32              getNthBool/Int/Real/AsciiString/UnicodeString(n)
**                          (only n where upstream is defined)
** Strings are logged as len:u32 + bytes (UnicodeString: len + 2*len
** bytes). Returns the log length, or -1 if the log did not fit. */
#include <stdlib.h>
#include <string.h>
#include <vector>
#include "Common/Dict.h"

const AsciiString AsciiString::TheEmptyString;
const UnicodeString UnicodeString::TheEmptyString;
static DynamicMemoryAllocator theShimAllocator;
DynamicMemoryAllocator *TheDynamicMemoryAllocator = &theShimAllocator;

namespace {

struct Log
{
	std::vector<unsigned char> b;
	void put(const void *p, int n) { b.insert(b.end(), (const unsigned char *)p, (const unsigned char *)p + n); }
	void u8(int v) { b.push_back((unsigned char)v); }
	void u32(unsigned v) { put(&v, 4); }
	void i32(int v) { put(&v, 4); }
	void f32(float v) { put(&v, 4); }
	void ascii(const AsciiString &s) { u32((unsigned)s.getLength()); put(s.str(), s.getLength()); }
	void uni(const UnicodeString &s)
	{
		u32((unsigned)s.getLength());
		for (int i = 0; i < s.getLength(); ++i) { unsigned short c = s.getCharAt(i); put(&c, 2); }
	}
};

void nth(Log &lg, const Dict &d, int n)
{
	switch (d.getNthType(n)) {
	case Dict::DICT_BOOL: lg.u8(d.getNthBool(n)); break;
	case Dict::DICT_INT: lg.i32(d.getNthInt(n)); break;
	case Dict::DICT_REAL: lg.f32(d.getNthReal(n)); break;
	case Dict::DICT_ASCIISTRING: lg.ascii(d.getNthAsciiString(n)); break;
	case Dict::DICT_UNICODESTRING: lg.uni(d.getNthUnicodeString(n)); break;
	default: break;
	}
}

} /* namespace */

extern "C" int ref_dict_run(const unsigned char *script, int len, unsigned char *out, int cap)
{
	Dict *slot[4] = { new Dict, new Dict, new Dict, new Dict };
	Log lg;
	int i = 0;
	auto u8 = [&]() { return (int)script[i++]; };
	auto u16 = [&]() { unsigned short v; memcpy(&v, script + i, 2); i += 2; return (int)v; };
	auto u32 = [&]() { unsigned v; memcpy(&v, script + i, 4); i += 4; return v; };
	while (i < len) {
		int op = u8();
		int s = u8() & 3;
		Dict &d = *slot[s];
		switch (op) {
		case 0: { int n = (int)u32(); delete slot[s]; slot[s] = new Dict(n); break; }
		case 1: d.clear(); break;
		case 2: { NameKeyType k = (NameKeyType)u32(); d.setBool(k, u8() != 0); break; }
		case 3: { NameKeyType k = (NameKeyType)u32(); d.setInt(k, (int)u32()); break; }
		case 4: { NameKeyType k = (NameKeyType)u32(); unsigned b = u32(); float f; memcpy(&f, &b, 4); d.setReal(k, f); break; }
		case 5: {
			NameKeyType k = (NameKeyType)u32(); int n = u16();
			std::string t((const char *)script + i, n); i += n;
			d.setAsciiString(k, AsciiString(t.c_str()));
			break;
		}
		case 6: {
			NameKeyType k = (NameKeyType)u32(); int n = u16();
			std::vector<WideChar> w(n);
			if (n) memcpy(&w[0], script + i, 2 * n);
			i += 2 * n;
			d.setUnicodeString(k, UnicodeString(n ? &w[0] : 0, n));
			break;
		}
		case 7: { NameKeyType k = (NameKeyType)u32(); lg.u8(d.remove(k)); break; }
		case 8: { int t = u8() & 3; NameKeyType k = (NameKeyType)u32(); d.copyPairFrom(*slot[t], k); break; }
		case 9: { int t = u8() & 3; d = *slot[t]; break; }
		case 10: { int t = u8() & 3; Dict *c = new Dict(*slot[t]); delete slot[s]; slot[s] = c; break; }
		case 11: {
			int c = d.getPairCount();
			lg.i32(c);
			for (int n = 0; n < c; ++n) { lg.u32((unsigned)d.getNthKey(n)); lg.i32((int)d.getNthType(n)); nth(lg, d, n); }
			break;
		}
		case 12: {
			NameKeyType k = (NameKeyType)u32();
			Bool e;
			lg.i32((int)d.getType(k));
			lg.u8(d.getBool(k, &e)); lg.u8(e);
			lg.i32(d.getInt(k, &e)); lg.u8(e);
			lg.f32(d.getReal(k, &e)); lg.u8(e);
			lg.ascii(d.getAsciiString(k, &e)); lg.u8(e);
			lg.uni(d.getUnicodeString(k, &e)); lg.u8(e);
			break;
		}
		case 13: { int n = (int)u32(); lg.u32((unsigned)d.getNthKey(n)); lg.i32((int)d.getNthType(n)); break; }
		case 14: {
			int n = (int)u32();
			lg.u8(d.getNthBool(n)); lg.i32(d.getNthInt(n)); lg.f32(d.getNthReal(n));
			lg.ascii(d.getNthAsciiString(n)); lg.uni(d.getNthUnicodeString(n));
			break;
		}
		default: abort();
		}
	}
	for (int k = 0; k < 4; ++k) delete slot[k];
	if ((int)lg.b.size() > cap) return -1;
	if (!lg.b.empty()) memcpy(out, &lg.b[0], lg.b.size());
	return (int)lg.b.size();
}
