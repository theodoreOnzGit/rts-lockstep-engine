/* Differential-test wrapper for the draft src/drafts/sweep_wwlib/ini.rs (#215, group wwlib3), upstream commit 0a05454d:
** the INIClass members below, cut VERBATIM from GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/ini.cpp by build.rs (drafts_wwlib3):
**   Enumerate_Entries; Put_TextBlock, Get_TextBlock, Put_Int, Get_Int, Put_Rect, Get_Rect, Put_Hex, Get_Hex, Get_Float, Put_Float,
**   Get_Double, Put_Double; Get_String (the buffer form); Get_Alloc_String, Get_List_Index, Get_Int_Bitfield, Get_Alloc_Int_Array,
**   Put_Bool, Get_Bool, Put_Point and Get_Point (all four); and trim.cpp's strtrim.
** Stand-ins (what the cuts call outside themselves): the INIClass class declaration (the members the cuts define or call; the real
** class holds sections, indices and files) over a vector of sections of (entry, value) pairs in insertion order, with Find_Entry
** (exact name match; the real one hashes the lower-cased name), Entry_Count, Get_Entry, Is_Present, Clear(section) and
** Put_String (the real function's effect: an existing entry is removed, a non-empty string is appended at the tail of the section;
** the DuplicateCRCError case, a differently-cased spelling of an entry, is not generated). INIEntry holds the Entry and Value
** pointers the cuts read. Rect and TPoint2D/3D are small stand-ins (below). Not covered (a stream, Base64 and the wide string
** class): Put_UUBlock, Get_UUBlock, Get_Wide_String, Put_Wide_String. assert is compiled out (NDEBUG). */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <strings.h>
#include <ctype.h>
#include <assert.h>
#include <string>
#include <vector>
#include <stdint.h>
#include "always.h"
#include "wwdebug.h"
/* stand-ins for trect.h's TRect<int> (Rect) and point.h's TPoint2D<T>, TPoint3D<T>: the real headers name members of dependent bases
** unqualified (g++ rejects them); only the public members the cuts use are declared, with the real names and constructors. */
template <class T> struct TPoint2D { T X, Y; TPoint2D() : X(0), Y(0) {} TPoint2D(T x, T y) : X(x), Y(y) {} };
template <class T> struct TPoint3D { T X, Y, Z; TPoint3D() : X(0), Y(0), Z(0) {} TPoint3D(T x, T y, T z) : X(x), Y(y), Z(z) {} };
template <class T> struct TRect { T X, Y, Width, Height; TRect() : X(0), Y(0), Width(0), Height(0) {} TRect(T x, T y, T w, T h) : X(x), Y(y), Width(w), Height(h) {} };
typedef TRect<int> Rect;
#define stricmp strcasecmp
/* strtrim copies a string onto its own tail with strcpy (source and destination overlap, destination first): undefined in C, and
** glibc's vectorised copy garbles it; MSVC's forward byte copy gives the intended result, which the game relies on. This is that copy. */
static inline char *shim_strcpy_forward(char *dst, const char *src) { char *d = dst; while ((*d++ = *src++) != 0) {} return dst; }
#define strcpy shim_strcpy_forward

struct INIEntry { char *Entry; char *Value; };

struct StandInSection {
	std::string name;
	std::vector<std::pair<std::string, std::string> > entries;
};

class INIClass {
public:
	std::vector<StandInSection> secs;
	mutable INIEntry scratch;

	INIClass() { scratch.Entry = 0; scratch.Value = 0; }
	StandInSection *find_sec(const char *s) const
	{
		for (size_t i = 0; i < secs.size(); i++) if (secs[i].name == s) return (StandInSection *)&secs[i];
		return 0;
	}
	INIEntry *Find_Entry(char const *section, char const *entry) const
	{
		StandInSection *s = find_sec(section);
		if (!s) return 0;
		for (size_t i = 0; i < s->entries.size(); i++)
			if (s->entries[i].first == entry) { scratch.Entry = (char *)s->entries[i].first.c_str(); scratch.Value = (char *)s->entries[i].second.c_str(); return &scratch; }
		return 0;
	}
	bool Is_Present(char const *section, char const *entry = 0) const { return entry ? Find_Entry(section, entry) != 0 : find_sec(section) != 0; }
	int Entry_Count(char const *section) const { StandInSection *s = find_sec(section); return s ? (int)s->entries.size() : 0; }
	char const *Get_Entry(char const *section, int index) const
	{
		StandInSection *s = find_sec(section);
		if (!s || index < 0 || index >= (int)s->entries.size()) return 0;
		return s->entries[index].first.c_str();
	}
	bool Clear(char const *section = 0)
	{
		for (size_t i = 0; i < secs.size(); i++) if (secs[i].name == section) { secs.erase(secs.begin() + i); break; }
		return true;
	}
	bool Put_String(char const *section, char const *entry, char const *string)
	{
		if (section == NULL || entry == NULL) return false;
		StandInSection *s = find_sec(section);
		if (!s) { secs.push_back(StandInSection()); secs.back().name = section; s = &secs.back(); }
		for (size_t i = 0; i < s->entries.size(); i++) if (s->entries[i].first == entry) { s->entries.erase(s->entries.begin() + i); break; }
		if (string != NULL && strlen(string) > 0) s->entries.push_back(std::make_pair(std::string(entry), std::string(string)));
		return true;
	}

	/* the members the cuts define */
	unsigned Enumerate_Entries(const char *Section, const char *Entry_Prefix, unsigned StartNumber, unsigned EndNumber);
	bool Put_TextBlock(char const *section, char const *text);
	int Get_TextBlock(char const *section, char *buffer, int len) const;
	bool Put_Int(char const *section, char const *entry, int number, int format = 0);
	int Get_Int(char const *section, char const *entry, int defvalue = 0) const;
	bool Put_Rect(char const *section, char const *entry, Rect const &value);
	Rect const Get_Rect(char const *section, char const *entry, Rect const &defvalue) const;
	bool Put_Hex(char const *section, char const *entry, int number);
	int Get_Hex(char const *section, char const *entry, int defvalue = 0) const;
	float Get_Float(char const *section, char const *entry, float defvalue = 0.0f) const;
	bool Put_Float(char const *section, char const *entry, float number);
	double Get_Double(char const *section, char const *entry, double defvalue = 0.0) const;
	bool Put_Double(char const *section, char const *entry, double number);
	int Get_String(char const *section, char const *entry, char const *defvalue, char *buffer, int size) const;
	char *Get_Alloc_String(char const *section, char const *entry, char const *defvalue) const;
	int Get_List_Index(char const *section, char const *entry, int const defvalue, char *list[]);
	int Get_Int_Bitfield(char const *section, char const *entry, int defvalue, char *list[]);
	int *Get_Alloc_Int_Array(char const *section, char const *entry, int listend = 0);
	bool Put_Bool(char const *section, char const *entry, bool value);
	bool Get_Bool(char const *section, char const *entry, bool defvalue = false) const;
	bool Put_Point(char const *section, char const *entry, TPoint2D<int> const &value);
	TPoint2D<int> const Get_Point(char const *section, char const *entry, TPoint2D<int> const &defvalue) const;
	bool Put_Point(char const *section, char const *entry, TPoint3D<int> const &value);
	TPoint3D<int> const Get_Point(char const *section, char const *entry, TPoint3D<int> const &defvalue) const;
	bool Put_Point(char const *section, char const *entry, TPoint3D<float> const &value);
	TPoint3D<float> const Get_Point(char const *section, char const *entry, TPoint3D<float> const &defvalue) const;
	TPoint2D<float> const Get_Point(char const *section, char const *entry, TPoint2D<float> const &defvalue) const;
};

#include "ini_strtrim.inc"
#include "ini_enum.inc"
#include "ini_text_to_double.inc"
#include "ini_get_string.inc"
#include "ini_alloc_to_points.inc"

static int g_cap, *g_out, g_n;
static void put(int w) { if (g_n < g_cap) g_out[g_n] = w; g_n++; }
static void putf(float f) { uint32_t b; memcpy(&b, &f, 4); put((int)b); }
static void putd(double d) { uint64_t b; memcpy(&b, &d, 8); put((int)(b >> 32)); put((int)(b & 0xffffffffu)); }
static std::string rd(const int *&p) { int n = *p++; std::string s; for (int i = 0; i < n; i++) s.push_back((char)*p++); return s; }
static float getf(const int *&p) { uint32_t b = (uint32_t)*p++; float f; memcpy(&f, &b, 4); return f; }
static void dump(const INIClass &ini)
{
	put((int)ini.secs.size());
	for (size_t i = 0; i < ini.secs.size(); i++) {
		put((int)ini.secs[i].name.size()); for (size_t k = 0; k < ini.secs[i].name.size(); k++) put((unsigned char)ini.secs[i].name[k]);
		put((int)ini.secs[i].entries.size());
		for (size_t j = 0; j < ini.secs[i].entries.size(); j++) {
			const std::string &e = ini.secs[i].entries[j].first, &v = ini.secs[i].entries[j].second;
			put((int)e.size()); for (size_t k = 0; k < e.size(); k++) put((unsigned char)e[k]);
			put((int)v.size()); for (size_t k = 0; k < v.size(); k++) put((unsigned char)v[k]);
		}
	}
}

extern "C" int ref_ini_run(const int *prog, int nops, int *out, int cap)
{
	g_out = out; g_cap = cap; g_n = 0;
	INIClass ini;
	const int *p = prog;
	for (int k = 0; k < nops; k++) {
		int op = *p++;
		std::string sec = rd(p), ent = rd(p);
		const char *S = sec.c_str(), *E = ent.c_str();
		switch (op) {
		case 0: { std::string v = rd(p); put(ini.Put_String(S, E, v.c_str())); break; }
		case 1: { int n = *p++, f = *p++; put(ini.Put_Int(S, E, n, f)); break; }
		case 2: { Rect r(*p, p[1], p[2], p[3]); p += 4; put(ini.Put_Rect(S, E, r)); break; }
		case 3: { int n = *p++; put(ini.Put_Hex(S, E, n)); break; }
		case 4: { float f = getf(p); put(ini.Put_Float(S, E, f)); break; }
		case 5: { uint64_t b = ((uint64_t)(uint32_t)p[0] << 32) | (uint32_t)p[1]; p += 2; double d; memcpy(&d, &b, 8); put(ini.Put_Double(S, E, d)); break; }
		case 6: { int b = *p++; put(ini.Put_Bool(S, E, b != 0)); break; }
		case 7: { TPoint2D<int> v(*p, p[1]); p += 2; put(ini.Put_Point(S, E, v)); break; }
		case 8: { TPoint3D<int> v(*p, p[1], p[2]); p += 3; put(ini.Put_Point(S, E, v)); break; }
		case 9: { float x = getf(p), y = getf(p), z = getf(p); TPoint3D<float> v(x, y, z); put(ini.Put_Point(S, E, v)); break; }
		case 10: { std::string t = rd(p); put(ini.Put_TextBlock(S, t.c_str())); break; }
		case 11: { int len = *p++; std::vector<char> buf(len > 0 ? len + 8 : 8, 'x'); int r = ini.Get_TextBlock(S, &buf[0], len); put(r); if (len > 0) { put((int)strlen(&buf[0])); for (size_t i = 0; buf[i]; i++) put((unsigned char)buf[i]); } break; }
		case 12: { int d = *p++; put(ini.Get_Int(S, E, d)); break; }
		case 13: { int d = *p++; put(ini.Get_Hex(S, E, d)); break; }
		case 14: { float d = getf(p); putf(ini.Get_Float(S, E, d)); break; }
		case 15: { uint64_t b = ((uint64_t)(uint32_t)p[0] << 32) | (uint32_t)p[1]; p += 2; double d; memcpy(&d, &b, 8); putd(ini.Get_Double(S, E, d)); break; }
		case 16: { Rect d(*p, p[1], p[2], p[3]); p += 4; Rect r = ini.Get_Rect(S, E, d); put(r.X); put(r.Y); put(r.Width); put(r.Height); break; }
		case 17: { int d = *p++; put(ini.Get_Bool(S, E, d != 0)); break; }
		case 18: { TPoint2D<int> d(*p, p[1]); p += 2; TPoint2D<int> r = ini.Get_Point(S, E, d); put(r.X); put(r.Y); break; }
		case 19: { TPoint3D<int> d(*p, p[1], p[2]); p += 3; TPoint3D<int> r = ini.Get_Point(S, E, d); put(r.X); put(r.Y); put(r.Z); break; }
		case 20: { float x = getf(p), y = getf(p), z = getf(p); TPoint3D<float> d(x, y, z); TPoint3D<float> r = ini.Get_Point(S, E, d); putf(r.X); putf(r.Y); putf(r.Z); break; }
		case 21: { float x = getf(p), y = getf(p); TPoint2D<float> d(x, y); TPoint2D<float> r = ini.Get_Point(S, E, d); putf(r.X); putf(r.Y); break; }
		case 22: case 23: { int d = *p++; int nl = *p++; std::vector<std::string> toks; for (int i = 0; i < nl; i++) toks.push_back(rd(p));
			std::vector<char *> list; for (size_t i = 0; i < toks.size(); i++) list.push_back((char *)toks[i].c_str()); list.push_back(0);
			put(op == 22 ? ini.Get_List_Index(S, E, d, &list[0]) : ini.Get_Int_Bitfield(S, E, d, &list[0])); break; }
		case 24: { int le = *p++; int *a = ini.Get_Alloc_Int_Array(S, E, le); int c = 0; while (a[c] != le) c++; put(c); for (int i = 0; i < c; i++) put(a[i]); delete[] a; break; }
		case 25: { int hasdef = *p++; std::string d = rd(p); char *r = ini.Get_Alloc_String(S, E, hasdef ? d.c_str() : (const char *)0);
			if (!r) put(-1); else { put((int)strlen(r)); for (size_t i = 0; r[i]; i++) put((unsigned char)r[i]); free(r); } break; }
		case 26: { int size = *p++; std::string d = rd(p); std::vector<char> buf(size > 0 ? size + 8 : 8, 'x'); int r = ini.Get_String(S, E, d.c_str(), &buf[0], size);
			put(r); if (size >= 2) { put((int)strlen(&buf[0])); for (size_t i = 0; buf[i]; i++) put((unsigned char)buf[i]); } break; }
		case 27: { std::string pre = rd(p); unsigned a = (unsigned)*p++, b = (unsigned)*p++; put((int)ini.Enumerate_Entries(S, pre.c_str(), a, b)); break; }
		}
		dump(ini);
	}
	return g_n;
}
