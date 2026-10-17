/* Differential-test wrapper for GameEngine/Source/Common/NameKeyGenerator.cpp
** and Include/Common/NameKeyGenerator.h (upstream commit 0a05454d), compiled
** unchanged in the chunk build (build.rs, with
** differential/shim_chunk/chunk_prelude.h force-included; `_stricmp` is
** strcasecmp, which in the C locale folds ASCII only, like MSVC's).
**
** ref_namekey_run interprets a script of operations on a NameKeyGenerator
** that it owns, installed as TheNameKeyGenerator for the duration (so that
** StaticNameKey, which uses the global, sees it), and logs every result:
**   0 len:u16 bytes     nameToKey(name)          -> key:u32
**   1 len:u16 bytes     nameToLowercaseKey(name) -> key:u32
**   2 key:u32           keyToName(key)           -> len:u32 bytes
**   3                   reset()
**   4                   init()
**   5                   a new, uninitialised generator
**   6 slot:u8 len:u16 bytes  StaticNameKey number `slot` (created with this
**                       name on first use, then kept): key() -> key:u32
** The test sends only names that are defined upstream (no byte >= 0x80 to
** nameToLowercaseKey). Returns the log length, or -1 if it did not fit. */
#include <stdlib.h>
#include <string.h>
#include <string>
#include <vector>

extern "C" int ref_namekey_run(const unsigned char *script, int len, unsigned char *log, int cap)
{
	std::vector<unsigned char> out;
	auto put = [&](const void *p, int n) { out.insert(out.end(), (const unsigned char *)p, (const unsigned char *)p + n); };
	auto putU32 = [&](unsigned v) { put(&v, 4); };
	NameKeyGenerator *saved = TheNameKeyGenerator;
	NameKeyGenerator *gen = new NameKeyGenerator;
	TheNameKeyGenerator = gen;
	std::vector<std::string> names(256);
	std::vector<StaticNameKey *> slots(256, (StaticNameKey *)0);
	int i = 0;
	auto u16 = [&]() { unsigned short v; memcpy(&v, script + i, 2); i += 2; return v; };
	auto str = [&]() { int n = u16(); std::string s((const char *)script + i, n); i += n; return s; };
	while (i < len) {
		int op = script[i++];
		switch (op) {
		case 0: { std::string s = str(); putU32((unsigned)gen->nameToKey(s.c_str())); break; }
		case 1: { std::string s = str(); putU32((unsigned)gen->nameToLowercaseKey(s.c_str())); break; }
		case 2: {
			unsigned k; memcpy(&k, script + i, 4); i += 4;
			AsciiString n = gen->keyToName((NameKeyType)k);
			putU32((unsigned)strlen(n.str())); put(n.str(), (int)strlen(n.str()));
			break;
		}
		case 3: gen->reset(); break;
		case 4: gen->init(); break;
		case 5: delete gen; gen = new NameKeyGenerator; TheNameKeyGenerator = gen; break;
		case 6: {
			int slot = script[i++];
			std::string s = str();
			if (!slots[slot]) { names[slot] = s; slots[slot] = new StaticNameKey(names[slot].c_str()); }
			putU32((unsigned)slots[slot]->key());
			break;
		}
		default: abort();
		}
	}
	for (size_t k = 0; k < slots.size(); ++k) delete slots[k];
	delete gen;
	TheNameKeyGenerator = saved;
	if ((int)out.size() > cap) return -1;
	memcpy(log, out.data(), out.size());
	return (int)out.size();
}

/* WellKnownKeys.h, unchanged, instantiated here as upstream instantiates it
** in WorldHeightMap.cpp. ref_well_known_key_name(i): on a fresh generator,
** the name entry i of differential/well_known_keys_list.h maps to through
** its own TheKey_<name> (key(), then keyToName), i.e. the #NAME of the
** header's DEFINE_KEY; also returns that key in *key. */
#define INSTANTIATE_WELL_KNOWN_KEYS
#include "Common/WellKnownKeys.h"

static const StaticNameKey *const k_well_known[] = {
#define X(n) &TheKey_##n,
#include "well_known_keys_list.h"
#undef X
};

extern "C" int ref_well_known_key_count() { return (int)(sizeof(k_well_known) / sizeof(k_well_known[0])); }

extern "C" int ref_well_known_key_name(int i, char *out, int cap, unsigned *key)
{
	NameKeyGenerator *saved = TheNameKeyGenerator;
	static NameKeyGenerator *gen = 0;
	if (!gen) { gen = new NameKeyGenerator; gen->init(); }
	TheNameKeyGenerator = gen;
	NameKeyType k = k_well_known[i]->key();
	AsciiString n = gen->keyToName(k);
	TheNameKeyGenerator = saved;
	*key = (unsigned)k;
	int len = (int)strlen(n.str());
	if (len >= cap) return -1;
	memcpy(out, n.str(), len + 1);
	return len;
}
