/* Differential-test wrapper for GameEngine/Source/Common/System/UnicodeString.cpp
** and Include/Common/UnicodeString.h (upstream commit 0a05454d), compiled
** unchanged with differential/shim_unicode/unicode_prelude.h force-included
** and -fshort-wchar (16-bit WideChar, 16-bit stand-ins for the wide C runtime;
** see the prelude and build.rs, which renames the classes).
**
** ref_unicode_run interprets a script on 4 UnicodeString slots and logs every
** result (little-endian). "lit" is len:u16 then len u16 units, used as a C
** string (it ends at the first 0 unit). Operations:
**   0 s lit          set(const WideChar*)
**   1 s lit          concat(const WideChar*)
**   2 s c:u16        concat(WideChar)
**   3 s              trim()
**   4 s              removeLastChar()
**   5 s len bytes    translate(AsciiString of the bytes as a C string)
**   6 s t kind       nextToken(&slot t, delimiters) -> u8; kind 0: the
**                    default argument; 1 lit: UnicodeString(lit); 2 d:u8: a
**                    copy of slot d
**   7 s              clear()
**   8 s t            s = t        (operator=)
**   9 s t            s.set(t)     (set(const UnicodeString&))
**  10 s              dump: has buffer:u8, getLength:i32, isEmpty:u8,
**                    len:u32 + str() as u16 units
**  11 s i:i32        getCharAt(i) -> u16 (only where defined)
**  12 s t lit        compare(lit), compareNoCase(lit), compare(t),
**                    compareNoCase(t) signs (i8 each), then ==, !=, <, <=,
**                    >, >= of (s, t) (u8 each)
**  13 s t            s.concat(t)  (s != t: concat of a string onto itself
**                    overlaps wcscat's source and destination when the
**                    buffer has room, undefined)
**  14 s lit          s = UnicodeString(lit)  (the constructor)
**  15 s k:i32        s.set(s.str() + k)  (a set from the string's own tail:
**                    overlapping wcscpy, destination first; only with a
**                    buffer and 0 <= k <= getLength)
** Each operation logs first a status byte: 0, or 1 if it threw (the only
** throw here is ERROR_OUT_OF_MEMORY). Returns the log length, or -1. */
#include <stdlib.h>
#include <string.h>
#include <vector>
#include <string>

/* (TheDynamicMemoryAllocator is renamed in this build, see build.rs.) */
DynamicMemoryAllocator *TheDynamicMemoryAllocator = 0;
static DynamicMemoryAllocator theShimAllocator;
CriticalSection *TheUnicodeStringCriticalSection = 0;

namespace {
/* exposes the protected m_data (a test-only view of the same object) */
struct Probe : public UnicodeString
{
	bool hasBuffer() const { return m_data != 0; }
};
int sgn(int v) { return v < 0 ? -1 : v > 0 ? 1 : 0; }
}

extern "C" int ref_unicode_run(const unsigned char *script, int len, unsigned char *out, int cap)
{
	TheDynamicMemoryAllocator = &theShimAllocator;
	UnicodeString *slot[4] = { new UnicodeString, new UnicodeString, new UnicodeString, new UnicodeString };
	std::vector<unsigned char> lg;
	auto put = [&](const void *p, int n) { lg.insert(lg.end(), (const unsigned char *)p, (const unsigned char *)p + n); };
	auto u8 = [&](int v) { lg.push_back((unsigned char)v); };
	auto i32 = [&](int v) { put(&v, 4); };
	int i = 0;
	auto rd8 = [&]() { return (int)script[i++]; };
	auto rd16 = [&]() { unsigned short v; memcpy(&v, script + i, 2); i += 2; return (int)v; };
	/* a literal, with the terminating 0 unit upstream's C string would have */
	auto rdLit = [&]() {
		int n = rd16();
		std::vector<WideChar> w(n + 1);
		if (n) memcpy(&w[0], script + i, 2 * n);
		w[n] = 0;
		i += 2 * n;
		return w;
	};
	while (i < len) {
		int op = rd8();
		int s = rd8() & 3;
		UnicodeString &a = *slot[s];
		size_t statusAt = lg.size();
		u8(0);
		try {
			switch (op) {
			case 0: { std::vector<WideChar> w = rdLit(); a.set(&w[0]); break; }
			case 1: { std::vector<WideChar> w = rdLit(); a.concat(&w[0]); break; }
			case 2: { WideChar c = (WideChar)rd16(); a.concat(c); break; }
			case 3: a.trim(); break;
			case 4: a.removeLastChar(); break;
			case 5: {
				int n = rd16();
				std::string b((const char *)script + i, n);
				i += n;
				a.translate(AsciiString(b.c_str()));
				break;
			}
			case 6: {
				int t = rd8() & 3;
				int kind = rd8();
				bool r;
				if (kind == 0) {
					r = a.nextToken(slot[t]);
				} else if (kind == 1) {
					std::vector<WideChar> w = rdLit();
					r = a.nextToken(slot[t], UnicodeString(&w[0]));
				} else {
					int d = rd8() & 3;
					r = a.nextToken(slot[t], *slot[d]);
				}
				u8(r);
				break;
			}
			case 7: a.clear(); break;
			case 8: { int t = rd8() & 3; a = *slot[t]; break; }
			case 9: { int t = rd8() & 3; a.set(*slot[t]); break; }
			case 10: {
				u8(static_cast<const Probe &>(a).hasBuffer());
				i32(a.getLength());
				u8(a.isEmpty());
				int n = (int)shim_wcslen16(a.str());
				i32(n); put(a.str(), 2 * n);
				break;
			}
			case 11: { int k; memcpy(&k, script + i, 4); i += 4; int c = (unsigned short)a.getCharAt(k); u8(c & 0xff); u8(c >> 8); break; }
			case 12: {
				int t = rd8() & 3;
				std::vector<WideChar> w = rdLit();
				const UnicodeString &b = *slot[t];
				u8(sgn(a.compare(&w[0])) & 0xff); u8(sgn(a.compareNoCase(&w[0])) & 0xff);
				u8(sgn(a.compare(b)) & 0xff); u8(sgn(a.compareNoCase(b)) & 0xff);
				u8(a == b); u8(a != b); u8(a < b); u8(a <= b); u8(a > b); u8(a >= b);
				break;
			}
			case 13: { int t = rd8() & 3; if (t == s) abort(); a.concat(*slot[t]); break; }
			case 14: { std::vector<WideChar> w = rdLit(); a = UnicodeString(&w[0]); break; }
			case 15: { int k; memcpy(&k, script + i, 4); i += 4; a.set(a.str() + k); break; }
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
