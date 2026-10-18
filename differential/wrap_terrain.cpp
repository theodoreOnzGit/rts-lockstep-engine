/* Differential-test wrapper for TerrainTypes, MultiplayerSettings and
** CrateSystem (upstream commit 0a05454d), with their INI blocks:
**   GameEngine/Source/Common/TerrainTypes.cpp            (compiled unchanged)
**   GameEngine/Source/Common/MultiplayerSettings.cpp     (compiled unchanged)
**   GameEngine/Source/GameLogic/System/CrateSystem.cpp   (compiled unchanged)
**   GameEngine/Source/Common/INI/INITerrain.cpp          (compiled unchanged)
**   GameEngine/Source/Common/INI/INIMultiplayer.cpp      (compiled unchanged)
**   GameEngine/Source/Common/INI/INICrate.cpp            (compiled unchanged)
** built by build.rs in the BitFlags build's setup (clang++ -fms-extensions,
** the real BitFlagsIO.h through shim_flags/flags_prelude.h), with the
** stand-ins of differential/shim_terrain: GameClient/Color.h (`Int Color`),
** GameNetwork/GameInfo.h (the PLAYERTEMPLATE constants), Common/Ini.h (the
** forwarder CrateSystem.h needs). Money::parseMoneyAmount, KindOfMaskType's
** parser, INI::parseScience's lookup, GameCommon's TheVeterancyNames and the
** INI reader are the real definitions of the other builds (linked).
**
** The INI reader is the REAL INI::load (upstream_ini), through the hooks of
** wrap_ini.cpp (ref_ini_reset, ref_ini_add_file, ref_ini_load). The five
** block handlers of this file are strong there; wrap_ini.cpp's stand-ins for
** them are weak (p1w2-terrain).
**
** ScienceStore::friend_lookupScience is a stand-in that answers NAMEKEY(name)
** for every name: the real one (Science.cpp, not translated) throws
** INI_INVALID_DATA for a name it does not know, which the Rust does not check
** either (see logic::crate_system). TheScienceStore is pointed at it here.
**
** The dumps (extern "C", read by tests/differential_terrain.rs) are of every
** field of the three stores, floats as their bit patterns, every override
** chain node, and the name keys. Private members are reached through the
** same `#define private public` as the other wrappers, for this file only. */
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <string>
#include <map>
#include <list>
#include <vector>
#include "Common/NameKeyGenerator.h"
#define private public
#define protected public
#include "Common/Overridable.h"
#include "Common/Money.h"
#include "Common/TerrainTypes.h"
#include "Common/MultiplayerSettings.h"
#include "GameLogic/CrateSystem.h"
#include "Common/Science.h"
#undef private
#undef protected

/* ---- stand-ins */
namespace {
ScienceStore g_science;
}

ScienceType ScienceStore::friend_lookupScience(const char *scienceName) const
{
	return (ScienceType)TheNameKeyGenerator->nameToKey(scienceName);
}

namespace {

std::vector<unsigned char> g_dump;
void put(const void *p, int n) { g_dump.insert(g_dump.end(), (const unsigned char *)p, (const unsigned char *)p + n); }
void putU32(unsigned v) { put(&v, 4); }
void putI32(int v) { put(&v, 4); }
void putU8(unsigned v) { g_dump.push_back((unsigned char)v); }
void putReal(Real r) { put(&r, 4); }
void putStr(const AsciiString &s) { putU32((unsigned)s.getLength()); put(s.str(), s.getLength()); }
unsigned bitsOf(Real r) { unsigned u; memcpy(&u, &r, 4); return u; }
int finish(unsigned char *buf, int cap)
{
	int n = (int)g_dump.size();
	if (n <= cap && n > 0) memcpy(buf, &g_dump[0], n);
	return n;
}
void putRGB(const RGBColor &c)
{
	putU32(bitsOf(c.red));
	putU32(bitsOf(c.green));
	putU32(bitsOf(c.blue));
}
void putColorDef(const MultiplayerColorDefinition &d)
{
	putStr(d.m_tooltipName);
	putRGB(d.m_rgbValue);
	putI32(d.m_color);
	putRGB(d.m_rgbValueNight);
	putI32(d.m_colorNight);
}
void putMoney(const Money &m)
{
	putU32(m.countMoney());
	putI32(m.m_playerIndex);
}

} /* namespace */

extern "C" {

/* The name-key generator (installed as TheNameKeyGenerator, reset: keys start
** at 1 again), and empty terrain, multiplayer and crate stores, with the
** science stand-in in place. */
void ref_terrain_reset(void)
{
	static NameKeyGenerator *gen = 0;
	if (!gen) gen = new NameKeyGenerator;
	gen->reset();
	TheNameKeyGenerator = gen;
	delete TheTerrainTypes;
	TheTerrainTypes = new TerrainTypeCollection;
	delete TheMultiplayerSettings;
	TheMultiplayerSettings = new MultiplayerSettings;
	delete TheCrateSystem;
	TheCrateSystem = new CrateSystem;
	TheScienceStore = &g_science;
}

unsigned ref_terrain_name_key(const char *name) { return (unsigned)TheNameKeyGenerator->nameToKey(name); }

/* TheTerrainTypes: count, then per terrain in list order (the head first):
** name, texture (AsciiStrings), blend edges, class, restrict construction. */
int ref_terrain_dump(unsigned char *buf, int cap)
{
	g_dump.clear();
	unsigned count = 0;
	for (TerrainType *t = TheTerrainTypes->m_terrainList; t; t = t->m_next) ++count;
	putU32(count);
	for (TerrainType *t = TheTerrainTypes->m_terrainList; t; t = t->m_next) {
		putStr(t->m_name);
		putStr(t->m_texture);
		putU8(t->m_blendEdgeTexture ? 1 : 0);
		putI32((int)t->m_class);
		putU8(t->m_restrictConstruction ? 1 : 0);
	}
	return finish(buf, cap);
}

/* TheMultiplayerSettings: the six fields, the colour count, the colour list
** (key, then the definition), the observer's and the random colour, the
** starting-money list, the default money and whether there was one. */
int ref_mp_dump(unsigned char *buf, int cap)
{
	g_dump.clear();
	MultiplayerSettings *s = TheMultiplayerSettings;
	putI32(s->m_startCountdownTimerSeconds);
	putI32(s->m_maxBeaconsPerPlayer);
	putU8(s->m_isShroudInMultiplayer ? 1 : 0);
	putU8(s->m_showRandomPlayerTemplate ? 1 : 0);
	putU8(s->m_showRandomStartPos ? 1 : 0);
	putU8(s->m_showRandomColor ? 1 : 0);
	putU32((unsigned)s->m_colorList.size());
	for (MultiplayerColorIter it = s->m_colorList.begin(); it != s->m_colorList.end(); ++it) {
		putI32(it->first);
		putColorDef(it->second);
	}
	putColorDef(s->m_observerColor);
	putColorDef(s->m_randomColor);
	putU32((unsigned)s->m_startingMoneyList.size());
	for (size_t i = 0; i < s->m_startingMoneyList.size(); ++i) putMoney(s->m_startingMoneyList[i]);
	putMoney(s->m_defaultStartingMoney);
	putU8(s->m_gotDefaultStartingMoney ? 1 : 0);
	return finish(buf, cap);
}

/* getColor(which) for each which in [from, to]: a 0 byte where it is NULL,
** else 1 and the colour. Upstream's getNumColors is read by the caller. */
int ref_mp_colors_by_query(unsigned char *buf, int cap, int from, int to)
{
	g_dump.clear();
	for (int which = from; which <= to; ++which) {
		MultiplayerColorDefinition *d = TheMultiplayerSettings->getColor(which);
		if (!d) {
			putU8(0);
		} else {
			putU8(1);
			putColorDef(*d);
		}
	}
	return finish(buf, cap);
}

/* findMultiplayerColorDefinitionByName(name): a 0 byte for none, else 1 and
** the first colour (in key order) whose tooltip is the name. */
int ref_mp_find_by_name(const char *name, unsigned char *buf, int cap)
{
	g_dump.clear();
	MultiplayerColorDefinition *d = TheMultiplayerSettings->findMultiplayerColorDefinitionByName(AsciiString(name));
	if (!d) {
		putU8(0);
	} else {
		putU8(1);
		putColorDef(*d);
	}
	return finish(buf, cap);
}

/* TheCrateSystem: the number of base templates, then for each base (vector
** order) its override chain: per node a 1 byte, then the node's fields (the
** isOverride flag, name, creation chance, veterancy level (-1 for
** LEVEL_INVALID), the 38-bit KindOf mask as one byte per bit, the killer
** science key (-1 for SCIENCE_INVALID), the possible crates, owned by maker);
** a 0 byte ends the chain. */
int ref_crate_dump(unsigned char *buf, int cap)
{
	g_dump.clear();
	putU32((unsigned)TheCrateSystem->m_crateTemplateVector.size());
	for (size_t i = 0; i < TheCrateSystem->m_crateTemplateVector.size(); ++i) {
		for (Overridable *n = TheCrateSystem->m_crateTemplateVector[i]; n; n = n->m_nextOverride) {
			CrateTemplate *ct = static_cast<CrateTemplate *>(n);
			putU8(1);
			putU8(n->m_isOverride ? 1 : 0);
			putStr(ct->m_name);
			putU32(bitsOf(ct->m_creationChance));
			putI32(ct->m_veterancyLevel == LEVEL_INVALID ? -1 : (int)ct->m_veterancyLevel);
			for (int b = 0; b < KINDOF_COUNT; ++b) putU8(ct->m_killedByTypeKindof.test(b) ? 1 : 0);
			putI32((int)ct->m_killerScience);
			putU32((unsigned)ct->m_possibleCrates.size());
			for (crateCreationEntryConstIterator it = ct->m_possibleCrates.begin(); it != ct->m_possibleCrates.end(); ++it) {
				putStr(it->crateName);
				putU32(bitsOf(it->crateChance));
			}
			putU8(ct->m_isOwnedByMaker ? 1 : 0);
		}
		putU8(0);
	}
	return finish(buf, cap);
}

/* TheCrateSystem->reset(): the INI-independent clean-up of overrides. */
void ref_crate_reset(void) { TheCrateSystem->reset(); }

/* The names' template as TheCrateSystem's findCrateTemplate sees it: 0 when
** there is none, else 1 and the same dump as one chain node (the final
** override's fields). */
int ref_crate_find(const char *name, unsigned char *buf, int cap)
{
	g_dump.clear();
	const CrateTemplate *ct = TheCrateSystem->findCrateTemplate(AsciiString(name));
	if (!ct) {
		putU8(0);
		return finish(buf, cap);
	}
	putU8(1);
	putStr(ct->m_name);
	putU32(bitsOf(ct->m_creationChance));
	putI32(ct->m_veterancyLevel == LEVEL_INVALID ? -1 : (int)ct->m_veterancyLevel);
	for (int b = 0; b < KINDOF_COUNT; ++b) putU8(ct->m_killedByTypeKindof.test(b) ? 1 : 0);
	putI32((int)ct->m_killerScience);
	putU32((unsigned)ct->m_possibleCrates.size());
	for (crateCreationEntryConstIterator it = ct->m_possibleCrates.begin(); it != ct->m_possibleCrates.end(); ++it) {
		putStr(it->crateName);
		putU32(bitsOf(it->crateChance));
	}
	putU8(ct->m_isOwnedByMaker ? 1 : 0);
	return finish(buf, cap);
}

} /* extern "C" */
