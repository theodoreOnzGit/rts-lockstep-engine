/* Differential-test wrapper for the registry draft (#215, group lc2, upstream commit 0a05454d), src/drafts/logic_common/registry.rs:
**   GeneralsMD/Code/GameEngine/Source/Common/System/registry.cpp from `getStringFromRegistry` to the end of the file, cut verbatim by
**   build.rs into registry.inc and #included FOUR times, each in its own namespace (R0..R3), because `GetRegistryLanguage` keeps its answer in
**   function-level statics (`cached`, `val`) that nothing can reset: each copy has its own, so the test can run four first-call scenarios.
**
** Stand-ins (outside the cut): AsciiString is a std::string with str(), concat(), getLength() and assignment; the Windows registry calls
** (RegOpenKeyEx, RegQueryValueEx, RegCloseKey, RegCreateKeyEx, RegSetValueEx) act on the test's fake registry, a vector of (root, path, key,
** kind, value) with Windows' contract that matters to the cut: RegOpenKeyEx fails unless the key path exists, RegQueryValueEx fails with
** ERROR_FILE_NOT_FOUND for a missing value and ERROR_MORE_DATA when the value is larger than the buffer (a string needs its terminating
** NUL: size = length + 1), else copies the bytes and returns the size; RegCreateKeyEx / RegSetValueEx fail for the roots in the test's
** write-deny mask. DEBUG_LOG is empty (compiled out of release).
**
** Entry points (tests/differential_draft_registry.rs): ref_lc2_reg_run, ref_lc2_reg_lang. */
#include <string>
#include <string.h>
#include <vector>

typedef int Int;
typedef bool Bool;
typedef unsigned int UnsignedInt;
typedef int HKEY;
#define TRUE true
#define FALSE false
#define NULL 0
#define DEBUG_LOG(x)

namespace {

class AsciiString
{
public:
	std::string s;
	AsciiString() {}
	AsciiString(const char *c) : s(c) {}
	AsciiString &operator=(const char *c) { s = c; return *this; }
	const char *str() const { return s.c_str(); }
	void concat(const AsciiString &o) { s += o.s; }
	int getLength() const { return (int)s.size(); }
};

const int ERROR_SUCCESS = 0;
const int ERROR_FILE_NOT_FOUND = 2;
const int ERROR_ACCESS_DENIED = 5;
const int ERROR_MORE_DATA = 234;
const int KEY_READ = 1;
const int KEY_WRITE = 2;
const int REG_OPTION_NON_VOLATILE = 0;
const unsigned long REG_SZ = 1;
const unsigned long REG_DWORD = 4;
const HKEY HKEY_LOCAL_MACHINE = 1;
const HKEY HKEY_CURRENT_USER = 2;

struct Entry { int root; std::string path; std::string key; bool dword; std::string bytes; };
std::vector<Entry> g_reg;
int g_denyWriteMask = 0; /* bit root */
std::vector<std::pair<int, std::string> > g_open; /* handles: index + 1 */

bool keyExists(int root, const std::string &path)
{
	for (size_t i = 0; i < g_reg.size(); ++i) if (g_reg[i].root == root && g_reg[i].path == path) return true;
	return false;
}

long RegOpenKeyEx(HKEY root, const char *path, int, int, HKEY *handle)
{
	if (!keyExists(root, path)) return ERROR_FILE_NOT_FOUND;
	g_open.push_back(std::make_pair((int)root, std::string(path)));
	*handle = (HKEY)g_open.size();
	return ERROR_SUCCESS;
}

long RegQueryValueEx(HKEY handle, const char *key, void *, unsigned long *type, unsigned char *buffer, unsigned long *size)
{
	const std::pair<int, std::string> &h = g_open[handle - 1];
	for (size_t i = 0; i < g_reg.size(); ++i)
	{
		const Entry &e = g_reg[i];
		if (e.root == h.first && e.path == h.second && e.key == key)
		{
			if (e.bytes.size() > *size) return ERROR_MORE_DATA;
			memcpy(buffer, e.bytes.data(), e.bytes.size());
			*size = (unsigned long)e.bytes.size();
			*type = e.dword ? REG_DWORD : REG_SZ;
			return ERROR_SUCCESS;
		}
	}
	return ERROR_FILE_NOT_FOUND;
}

long RegCloseKey(HKEY) { return ERROR_SUCCESS; }

long RegCreateKeyEx(HKEY root, const char *path, int, const char *, int, int, void *, HKEY *handle, void *)
{
	if ((g_denyWriteMask >> root) & 1) return ERROR_ACCESS_DENIED;
	g_open.push_back(std::make_pair((int)root, std::string(path)));
	*handle = (HKEY)g_open.size();
	return ERROR_SUCCESS;
}

long RegSetValueEx(HKEY handle, const char *key, int, unsigned long type, const unsigned char *data, int size)
{
	const std::pair<int, std::string> &h = g_open[handle - 1];
	if ((g_denyWriteMask >> h.first) & 1) return ERROR_ACCESS_DENIED;
	Entry e; e.root = h.first; e.path = h.second; e.key = key; e.dword = type == REG_DWORD; e.bytes = std::string((const char *)data, (size_t)size);
	for (size_t i = 0; i < g_reg.size(); ++i)
		if (g_reg[i].root == e.root && g_reg[i].path == e.path && g_reg[i].key == e.key) { g_reg[i] = e; return ERROR_SUCCESS; }
	g_reg.push_back(e);
	return ERROR_SUCCESS;
}

namespace R0 {
#include "registry.inc"
}
namespace R1 {
#include "registry.inc"
}
namespace R2 {
#include "registry.inc"
}
namespace R3 {
#include "registry.inc"
}

void addEntry(int root, const char *path, const char *key, bool dword, const std::string &bytes)
{
	Entry e; e.root = root; e.path = path; e.key = key; e.dword = dword; e.bytes = bytes;
	for (size_t i = 0; i < g_reg.size(); ++i)
		if (g_reg[i].root == root && g_reg[i].path == e.path && g_reg[i].key == e.key) { g_reg[i] = e; return; }
	g_reg.push_back(e);
}

void sig(const AsciiString &a, int *out)
{
	long sum = 0;
	for (size_t i = 0; i < a.s.size(); ++i) sum += (unsigned char)a.s[i] * (long)(i + 1);
	out[0] = (int)a.s.size();
	out[1] = (int)(sum & 0x7fffffff);
}

const char *const GEN = "SOFTWARE\\Electronic Arts\\EA Games\\Generals";
const char *const ZH = "SOFTWARE\\Electronic Arts\\EA Games\\Command and Conquer Generals Zero Hour";

}

extern "C" {

/* The fake registry is cleared and filled from `entries` (5 words each: root 1|2, path string, key string, kind 0 string | 1 dword, value), where the path
** and key are passed as NUL-terminated strings packed in `blob` (offsets in words 1 and 2) and string values are offsets into `blob` too; a dword's value is
** the number. Operations (4 words [kind, root-or-0, pathOff, keyOff] + value word) are described in the test. Simplest contract: the test passes `n`
** operations as 6 words [kind, root, path offset, key offset, value offset or number, 0]. Results: 3 words per operation. */
int ref_lc2_reg_run(int denyMask, const int *entries, int nentries, const char *blob, const int *ops, int nops, int *out)
{
	g_reg.clear();
	g_open.clear();
	g_denyWriteMask = 0;
	for (int i = 0; i < nentries; ++i)
	{
		const int *e = entries + 5 * i;
		std::string v = e[3] ? std::string((const char *)&e[4], 4) : std::string(blob + e[4]) + std::string(1, '\0');
		addEntry(e[0], blob + e[1], blob + e[2], e[3] != 0, v);
	}
	g_denyWriteMask = denyMask;
	int w = 0;
	for (int i = 0; i < nops; ++i)
	{
		const int *o = ops + 6 * i;
		int kind = o[0], root = o[1];
		AsciiString path(blob + o[2]), key(blob + o[3]);
		AsciiString sval("init");
		UnsignedInt uval = 0xDEADBEEFu;
		int ok = 0, a = 0, b = 0;
		switch (kind)
		{
		case 0: ok = R0::GetStringFromGeneralsRegistry(path, key, sval); break;
		case 1: ok = R0::GetStringFromRegistry(path, key, sval); break;
		case 2: ok = R0::GetUnsignedIntFromRegistry(path, key, uval); break;
		case 3: sval = R0::GetRegistryGameName().str(); ok = 1; break;
		case 4: uval = R0::GetRegistryVersion(); ok = 1; break;
		case 5: uval = R0::GetRegistryMapPackVersion(); ok = 1; break;
		case 6: ok = R0::getStringFromRegistry(root, path, key, sval); break;
		case 7: ok = R0::getUnsignedIntFromRegistry(root, path, key, uval); break;
		case 8: ok = R0::setStringInRegistry(root, path, key, AsciiString(blob + o[4])); break;
		case 9: ok = R0::setUnsignedIntInRegistry(root, path, key, (UnsignedInt)o[4]); break;
		}
		int s[2] = { 0, 0 };
		sig(sval, s);
		out[w++] = ok;
		if (kind == 2 || kind == 4 || kind == 5 || kind == 7) { out[w++] = (int)uval; out[w++] = 0; }
		else { out[w++] = s[0]; out[w++] = s[1]; }
		(void)a; (void)b;
	}
	return w;
}

/* scenario k (0..3): the fake registry holds what `entries` says; GetRegistryLanguage of copy Rk is called, the registry is changed to `after`, and it is
** called twice more. out: 3 signatures (length, byte sum), 6 words. */
int ref_lc2_reg_lang(int scenario, const int *entries, int nentries, const int *after, int nafter, const char *blob, int *out)
{
	g_reg.clear();
	g_open.clear();
	g_denyWriteMask = 0;
	for (int i = 0; i < nentries; ++i) addEntry(entries[3 * i], ZH, "Language", false, std::string(blob + entries[3 * i + 1]) + std::string(1, '\0'));
	AsciiString v[3];
	switch (scenario) { case 0: v[0] = R0::GetRegistryLanguage(); break; case 1: v[0] = R1::GetRegistryLanguage(); break; case 2: v[0] = R2::GetRegistryLanguage(); break; default: v[0] = R3::GetRegistryLanguage(); break; }
	g_reg.clear();
	for (int i = 0; i < nafter; ++i) addEntry(after[3 * i], ZH, "Language", false, std::string(blob + after[3 * i + 1]) + std::string(1, '\0'));
	for (int k = 1; k < 3; ++k)
		switch (scenario) { case 0: v[k] = R0::GetRegistryLanguage(); break; case 1: v[k] = R1::GetRegistryLanguage(); break; case 2: v[k] = R2::GetRegistryLanguage(); break; default: v[k] = R3::GetRegistryLanguage(); break; }
	for (int k = 0; k < 3; ++k) sig(v[k], out + 2 * k);
	return 6;
}

}
