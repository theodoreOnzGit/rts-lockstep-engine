/* Differential-test wrapper for GameEngine/Source/Common/System/AsciiString.cpp
** and Include/Common/AsciiString.h (upstream commit 0a05454d), compiled
** unchanged with differential/shim_ascii/ascii_prelude.h force-included
** (see differential/shim_ascii and build.rs, which renames the classes).
**
** ref_ascii_run interprets a script on 4 AsciiString slots and logs every
** result (little-endian):
**   0 s len:u16 bytes   set(bytes as a C string)
**   1 s len bytes       concat(const char*)
**   2 s c:u8            concat(char)
**   3 s                 trim()
**   4 s                 toLower()
**   5 s                 removeLastChar()
**   6 s len:u16 u16s    translate(UnicodeString)
**   7 s t sepsLen:i16 bytes   nextToken(&slot t, seps or NULL if -1) -> u8
**   8 s                 clear()
**   9 s t               s = t
**  10 s                 dump: has buffer:u8, getLength:i32, isEmpty, isNone,
**                       isNotEmpty, isNotNone (u8 each), len:u32 + str()
**  11 s i:i32           getCharAt(i) -> u8 (only where defined)
**  12 s len bytes       startsWith, startsWithNoCase, endsWith,
**                       endsWithNoCase (u8 each), compare and compareNoCase
**                       signs (i8 each)
**  13 s c:u8            find, reverseFind -> offset:i32 or -1
** Each operation logs first a status byte: 0, or 1 if it threw (the only
** throw here is ERROR_OUT_OF_MEMORY). Returns the log length, or -1. */
#include <stdlib.h>
#include <string.h>
#include <vector>
#include <string>

/* (TheDynamicMemoryAllocator is renamed in this build, see build.rs; the
** UnicodeString shim's TheEmptyString is defined by wrap_dict.cpp.) */
DynamicMemoryAllocator *TheDynamicMemoryAllocator = 0;
static DynamicMemoryAllocator theShimAllocator;

namespace {
/* exposes the protected m_data (a test-only view of the same object) */
struct Probe : public AsciiString
{
	bool hasBuffer() const { return m_data != 0; }
};
int sgn(int v) { return v < 0 ? -1 : v > 0 ? 1 : 0; }
}

extern "C" int ref_ascii_run(const unsigned char *script, int len, unsigned char *out, int cap)
{
	TheDynamicMemoryAllocator = &theShimAllocator;
	AsciiString *slot[4] = { new AsciiString, new AsciiString, new AsciiString, new AsciiString };
	std::vector<unsigned char> lg;
	auto put = [&](const void *p, int n) { lg.insert(lg.end(), (const unsigned char *)p, (const unsigned char *)p + n); };
	auto u8 = [&](int v) { lg.push_back((unsigned char)v); };
	auto i32 = [&](int v) { put(&v, 4); };
	int i = 0;
	auto rd8 = [&]() { return (int)script[i++]; };
	auto rd16 = [&]() { unsigned short v; memcpy(&v, script + i, 2); i += 2; return (int)v; };
	auto rdStr = [&]() { int n = rd16(); std::string s((const char *)script + i, n); i += n; return s; };
	while (i < len) {
		int op = rd8();
		int s = rd8() & 3;
		AsciiString &a = *slot[s];
		size_t statusAt = lg.size();
		u8(0);
		try {
			switch (op) {
			case 0: { std::string t = rdStr(); a.set(t.c_str()); break; }
			case 1: { std::string t = rdStr(); a.concat(t.c_str()); break; }
			case 2: { char c = (char)rd8(); a.concat(c); break; }
			case 3: a.trim(); break;
			case 4: a.toLower(); break;
			case 5: a.removeLastChar(); break;
			case 6: {
				int n = rd16();
				std::vector<WideChar> w(n);
				if (n) memcpy(&w[0], script + i, 2 * n);
				i += 2 * n;
				a.translate(UnicodeString(n ? &w[0] : 0, n));
				break;
			}
			case 7: {
				int t = rd8() & 3;
				short sl; memcpy(&sl, script + i, 2); i += 2;
				std::string seps;
				if (sl >= 0) { seps.assign((const char *)script + i, sl); i += sl; }
				Bool r = a.nextToken(slot[t], sl >= 0 ? seps.c_str() : NULL);
				u8(r);
				break;
			}
			case 8: a.clear(); break;
			case 9: { int t = rd8() & 3; a = *slot[t]; break; }
			case 10: {
				u8(static_cast<const Probe &>(a).hasBuffer());
				i32(a.getLength());
				u8(a.isEmpty()); u8(a.isNone()); u8(a.isNotEmpty()); u8(a.isNotNone());
				int n = (int)strlen(a.str());
				i32(n); put(a.str(), n);
				break;
			}
			case 11: { int k; memcpy(&k, script + i, 4); i += 4; u8((unsigned char)a.getCharAt(k)); break; }
			case 12: {
				std::string p = rdStr();
				u8(a.startsWith(p.c_str())); u8(a.startsWithNoCase(p.c_str()));
				u8(a.endsWith(p.c_str())); u8(a.endsWithNoCase(p.c_str()));
				u8(sgn(a.compare(p.c_str())) & 0xff); u8(sgn(a.compareNoCase(p.c_str())) & 0xff);
				break;
			}
			case 13: {
				char c = (char)rd8();
				const char *f = a.find(c), *r = a.reverseFind(c);
				i32(f ? (int)(f - a.str()) : -1);
				i32(r ? (int)(r - a.str()) : -1);
				break;
			}
			default: abort();
			}
		} catch (...) {
			lg[statusAt] = 1;
		}
	}
	for (int k = 0; k < 4; ++k) delete slot[k];
	if ((int)lg.size() > cap) return -1;
	if (!lg.empty()) memcpy(out, &lg[0], lg.size());
	return (int)lg.size();
}
