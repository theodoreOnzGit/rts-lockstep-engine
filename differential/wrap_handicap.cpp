/* Differential-test wrapper for Handicap (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/Common/RTS/Handicap.cpp
** (#included below, unchanged), with the stand-ins of differential/shim_money:
** the global name-key generator (first-seen keys, logging the names asked
** for), the dictionary (values by key), and the thing template (its
** structure flag is set by the caller of ref_handicap_read).
**
** ref_handicap_read builds a Dict from `nset` (name, value) pairs, runs
** readFromDict on a default Handicap, and returns the four multipliers
** getHandicap(t, template) for t in {BUILDCOST, BUILDTIME}, with a generic
** template and a structure template, in the order out[0..4] = (BUILDCOST,
** generic), (BUILDCOST, structure), (BUILDTIME, generic), (BUILDTIME,
** structure); out[1] and out[3] are the structure ones. The names readFromDict asked
** for are written to `log`, NUL-separated, in the order it asked. */
/* The stand-ins of this file are named apart from the same names in other
** differential builds (the name-key generator and the Dict of the chunk and
** dictionary builds): one binary links them all. See wrap_money.cpp. */
#define NameKeyGenerator HandicapTestNameKeyGenerator
#define TheNameKeyGenerator HandicapTestTheNameKeyGenerator
#define Dict HandicapTestDict
#define ThingTemplate HandicapTestThingTemplate

#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <map>
#include <string>
#include <vector>
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/Handicap.cpp"

namespace {

std::map<std::string, NameKeyType> g_keys;   /* name -> key, first-seen order */
std::vector<std::string> g_keyNames;         /* key - 1 -> name */
bool g_logging = false;
std::string g_log;
std::map<NameKeyType, float> g_dict;
bool g_isStructure = false;

NameKeyType keyOf(const std::string &name)
{
	std::map<std::string, NameKeyType>::iterator it = g_keys.find(name);
	if (it != g_keys.end()) return it->second;
	g_keyNames.push_back(name);
	NameKeyType k = (NameKeyType)g_keyNames.size();
	g_keys[name] = k;
	return k;
}

NameKeyGenerator g_nameKeys;

}  // namespace

NameKeyGenerator *TheNameKeyGenerator = &g_nameKeys;

NameKeyType NameKeyGenerator::nameToKey(const AsciiString &name)
{
	std::string s(name.str());
	if (g_logging) {
		g_log += s;
		g_log.push_back('\0');
	}
	return keyOf(s);
}

Real Dict::getReal(NameKeyType key, Bool *exists) const
{
	std::map<NameKeyType, float>::const_iterator it = g_dict.find(key);
	if (exists) *exists = (it != g_dict.end());
	return it != g_dict.end() ? it->second : 0.0f;
}

Bool ThingTemplate::isKindOf(KindOfType) const
{
	return g_isStructure;
}

extern "C" {

/* names: nset NUL-terminated names back to back; values: nset floats. */
void ref_handicap_read(int nset, const char *names, const float *values,
	float *out, char *log, int logCap, int *logLen)
{
	g_dict.clear();
	g_logging = false;
	const char *p = names;
	for (int i = 0; i < nset; ++i) {
		g_dict[keyOf(std::string(p))] = values[i];
		p += strlen(p) + 1;
	}
	Handicap h;
	g_log.clear();
	g_logging = true;
	Dict d;
	h.readFromDict(&d);
	g_logging = false;
	ThingTemplate generic, building;
	g_isStructure = false;
	out[0] = h.getHandicap(Handicap::BUILDCOST, &generic);
	out[2] = h.getHandicap(Handicap::BUILDTIME, &generic);
	g_isStructure = true;
	out[1] = h.getHandicap(Handicap::BUILDCOST, &building);
	out[3] = h.getHandicap(Handicap::BUILDTIME, &building);
	*logLen = (int)g_log.size();
	if (*logLen > logCap) *logLen = logCap;
	memcpy(log, g_log.data(), (size_t)*logLen);
}

} /* extern "C" */
