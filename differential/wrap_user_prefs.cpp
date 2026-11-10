/* Differential-test wrapper for the user preference files (GitHub #93, upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Include/Common/UserPreferences.h (PreferenceMap, class UserPreferences) and
**   GeneralsMD/Code/GameEngine/Source/Common/UserPreferences.cpp (intAsStr, boolAsStr, realAsStr, LINE_LEN, the
**   constructor and destructor, load, write, getBool, getReal, getInt, getAsciiString, setBool, setReal, setInt,
**   setAsciiString), the pieces build.rs cuts verbatim into OUT_DIR/user_prefs_gen/user_prefs_cut.inc,
** compiled with the REAL AsciiString.h and AsciiString.cpp (differential/shim_ascii, as the AsciiString build; the
** classes renamed by build.rs) and the C library's own fopen / fgets / fprintf on a real file.
**
** Everything below is a STAND-IN, written for this test: TheGlobalData (getPath_UserData only: the folder the test
** gives) and the allocator pointer.
**
** Entry point (extern "C"):
**   ref_user_prefs_run(in, len, dir, out, cap)  runs one case and writes the dump; its length (-1: too small).
** The case (little-endian): the file's length (u32) and bytes, then operations, each a kind byte and its fields:
**   1 getBool key def:u8   2 getInt key def:i32   3 getReal key def:f32   4 getAsciiString key def
**   5 setBool key v:u8     6 setInt key v:i32     7 setReal key v:f32     8 setAsciiString key val
** (a string is u16 length and bytes). The dump: load's answer (u8), the map (u32 count, then each key and value as
** u32 length and bytes), the file write() wrote (u8 answer, u32 length, bytes), each operation's answer (u8 / i32 /
** f32 bits / string), and the map and the written file again after all the operations. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <map>
#include <string>
#include <vector>

/* (the allocator pointer is renamed in this build, see build.rs) */
DynamicMemoryAllocator *TheDynamicMemoryAllocator = 0;
static DynamicMemoryAllocator theUserPrefsAllocator;

namespace {
struct UpTestGlobalData
{
	AsciiString m_userData;
	AsciiString getPath_UserData() const { return m_userData; }
};
UpTestGlobalData *TheGlobalData = 0;
}

#include "user_prefs_cut.inc"

namespace {
struct Out
{
	std::vector<unsigned char> b;
	void u8(int v) { b.push_back((unsigned char)v); }
	void u32(unsigned v) { b.insert(b.end(), (unsigned char *)&v, (unsigned char *)&v + 4); }
	void bytes(const char *p, size_t n) { u32((unsigned)n); b.insert(b.end(), p, p + n); }
};
void dumpMap(Out &o, const UserPreferences &p)
{
	o.u32((unsigned)p.size());
	for (PreferenceMap::const_iterator it = p.begin(); it != p.end(); ++it) {
		o.bytes(it->first.str(), strlen(it->first.str()));
		o.bytes(it->second.str(), strlen(it->second.str()));
	}
}
void dumpFile(Out &o, const UserPreferences &p, const char *path)
{
	Bool ok = const_cast<UserPreferences &>(p).write();
	o.u8(ok ? 1 : 0);
	std::string s;
	FILE *fp = fopen(path, "rb");
	if (fp) {
		char buf[4096];
		size_t n;
		while ((n = fread(buf, 1, sizeof buf, fp)) > 0)
			s.append(buf, n);
		fclose(fp);
	}
	o.bytes(s.data(), s.size());
}
}

extern "C" int ref_user_prefs_run(const unsigned char *in, int len, const char *dir, unsigned char *out, int cap)
{
	TheDynamicMemoryAllocator = &theUserPrefsAllocator;
	UpTestGlobalData g;
	g.m_userData = dir;
	TheGlobalData = &g;
	int i = 0;
	auto rd32 = [&]() { unsigned v; memcpy(&v, in + i, 4); i += 4; return v; };
	auto rd16 = [&]() { unsigned short v; memcpy(&v, in + i, 2); i += 2; return (int)v; };
	auto rdStr = [&]() { int n = rd16(); std::string s((const char *)in + i, n); i += n; return s; };
	unsigned flen = rd32();
	std::string path = std::string(dir) + "Prefs.ini";
	FILE *fp = fopen(path.c_str(), "wb");
	if (!fp)
		return -1;
	fwrite(in + i, 1, flen, fp);
	fclose(fp);
	i += flen;
	Out o;
	{
		UserPreferences p;
		Bool loaded = p.load("Prefs.ini");
		o.u8(loaded ? 1 : 0);
		dumpMap(o, p);
		dumpFile(o, p, path.c_str());
		while (i < len) {
			int op = in[i++];
			AsciiString key = rdStr().c_str();
			switch (op) {
			case 1: { int d = in[i++]; o.u8(p.getBool(key, d != 0) ? 1 : 0); break; }
			case 2: { int d; memcpy(&d, in + i, 4); i += 4; o.u32((unsigned)p.getInt(key, d)); break; }
			case 3: { float d; memcpy(&d, in + i, 4); i += 4; float r = p.getReal(key, d); unsigned u; memcpy(&u, &r, 4); o.u32(u); break; }
			case 4: { AsciiString d = rdStr().c_str(); AsciiString r = p.getAsciiString(key, d); o.bytes(r.str(), strlen(r.str())); break; }
			case 5: { int v = in[i++]; p.setBool(key, v != 0); break; }
			case 6: { int v; memcpy(&v, in + i, 4); i += 4; p.setInt(key, v); break; }
			case 7: { float v; memcpy(&v, in + i, 4); i += 4; p.setReal(key, v); break; }
			case 8: { AsciiString v = rdStr().c_str(); p.setAsciiString(key, v); break; }
			default: return -1;
			}
		}
		dumpMap(o, p);
		dumpFile(o, p, path.c_str());
	}
	remove(path.c_str());
	if ((int)o.b.size() > cap)
		return -1;
	memcpy(out, o.b.data(), o.b.size());
	return (int)o.b.size();
}
