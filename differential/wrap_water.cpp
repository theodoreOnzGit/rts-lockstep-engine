/* Differential-test wrapper for the water's settings (GitHub #57: a map's WaterTransparency override; upstream commit
** 0a05454d), in the campaign build's setup (the real INI.h, GameMemory's stand-in), with the REAL GameClient/Water.h,
** Common/Overridable.h and Common/Override.h. Cut by build.rs into OUT_DIR/water_gen, verbatim but for two mechanical
** renames, asserted once:
**   water_cut.inc        GameEngine/Source/GameClient/Water.cpp, from `WaterSetting WaterSettings[ TIME_OF_DAY_COUNT ];`
**                        to its end (the globals, both field parse tables, WaterSetting's constructor and destructor);
**   ini_water_cut.inc    GameEngine/Source/Common/INI/INIWater.cpp, from `void INI::parseWaterSettingDefinition` to its
**                        end, with `void INI::parseWaterSettingDefinition( INI* ini )` made `static void
**                        wtParseWaterSettingDefinition( INI* ini )` and `void INI::parseWaterTransparencyDefinition( INI
**                        *ini )` made `static void wtParseWaterTransparencyDefinition( INI *ini )` (the INI class is the
**                        real INI.h's, whose members another archive defines; wrap_ini.cpp's two handlers reach these
**                        through ref_water_set_block_hook and ref_water_transparency_block_hook, which ref_water_reset
**                        sets);
**   water_reset.inc      GameEngine/Source/GameLogic/System/GameLogic.cpp, GameLogic::reset's two water transparency
**                        lines (from the comment "Clean up any water transparency overrides" to the weather's comment).
** Stand-ins: TheTerrainVisual, whose replaceSkyboxTextures records the ten names. TimeOfDayNames is GameType.cpp's
** (upstream_ini). Every global the cuts define is renamed here (Wt...), so that nothing clashes with another archive.
**
** A second override before a reset makes the chain a cycle upstream (see src/client/water.rs), and the next
** friend_getFinalOverride never returns: the handler's hook checks for it first (an override load while the original
** has a next override) and, instead of calling the cut, sets the undefined flag and throws, which INI::load reports as
** its own exception. A reset with no setting dereferences NULL: ref_water_delete_overrides returns -1 instead.
**
** Entry points (extern "C"; tests/differential_water.rs documents the dump):
**   ref_water_reset()                       fresh settings (the chain deleted), no skybox calls, the hooks set
**   ref_water_undefined()                   whether the last load stopped at the cycle (then cleared)
**   ref_water_delete_overrides()            GameLogic::reset's two lines; -1 (and nothing done) with no setting
**   ref_water_dump(buf, cap)                the settings, the chain and the skybox calls, little-endian; the length */
#include <stdlib.h>
#include <string.h>
#include <vector>
#include "Common/INI.h"
#include "Common/GameType.h"

#define WaterSettings WtWaterSettings
#define TheWaterTransparency WtTheWaterTransparency
#define WaterSetting WtWaterSetting
#define WaterTransparencySetting WtWaterTransparencySetting
#define TheTerrainVisual WtTheTerrainVisual
#define TerrainVisual WtTerrainVisual
#include "GameClient/Water.h"

/* ---- TheTerrainVisual: the skybox replacements recorded ---- */
class TerrainVisual
{
public:
	std::vector<AsciiString> m_calls;
	void replaceSkyboxTextures( const AsciiString *oldTexName[5], const AsciiString *newTexName[5] )
	{
		for (int i = 0; i < 5; ++i) m_calls.push_back(*oldTexName[i]);
		for (int i = 0; i < 5; ++i) m_calls.push_back(*newTexName[i]);
	}
};
static TerrainVisual g_terrainVisual;
static TerrainVisual *TheTerrainVisual = &g_terrainVisual;

#include "water_cut.inc"
#include "ini_water_cut.inc"

static void wtGameLogicReset( void )
{
#include "water_reset.inc"
}

namespace {
std::vector<unsigned char> g_dump;
int g_undefined = 0;
void putU32(unsigned v) { for (int i = 0; i < 4; ++i) g_dump.push_back((unsigned char)(v >> (8 * i))); }
void putReal(Real r) { unsigned v; memcpy(&v, &r, 4); putU32(v); }
void putStr(const AsciiString &s) { putU32((unsigned)s.getLength()); for (int i = 0; i < s.getLength(); ++i) g_dump.push_back((unsigned char)s.str()[i]); }
void putRgba(const RGBAColorInt &c) { putU32((unsigned)c.red); putU32((unsigned)c.green); putU32((unsigned)c.blue); putU32((unsigned)c.alpha); }
void putRgb(const RGBColor &c) { putReal(c.red); putReal(c.green); putReal(c.blue); }
void putTransparency(const WaterTransparencySetting &t)
{
	putReal(t.m_transparentWaterDepth); putReal(t.m_minWaterOpacity); putRgb(t.m_standingWaterColor); putRgb(t.m_radarColor);
	putU32(t.m_additiveBlend ? 1u : 0u); putStr(t.m_standingWaterTexture);
	putStr(t.m_skyboxTextureN); putStr(t.m_skyboxTextureE); putStr(t.m_skyboxTextureS); putStr(t.m_skyboxTextureW);
	putStr(t.m_skyboxTextureT);
}
void setBlock(INI *ini) { wtParseWaterSettingDefinition(ini); }
void transparencyBlock(INI *ini)
{
	const WaterTransparencySetting *wt = TheWaterTransparency.getNonOverloadedPointer();
	if (wt != NULL && ini->getLoadType() == INI_LOAD_CREATE_OVERRIDES && wt->getNextOverride() != NULL) {
		g_undefined = 1;
		throw INI_INVALID_DATA;
	}
	wtParseWaterTransparencyDefinition(ini);
}
void deleteChain(Overridable *o)
{
	/* the chain is never a cycle here (see the header comment); ~Overridable deletes the rest */
	if (o) o->deleteInstance();
}
}	/* namespace */

extern "C" {
extern void (*ref_water_set_block_hook)(INI *);
extern void (*ref_water_transparency_block_hook)(INI *);

void ref_water_reset(void)
{
	deleteChain((Overridable *)TheWaterTransparency.getNonOverloadedPointer());
	TheWaterTransparency = NULL;
	for (int i = 0; i < TIME_OF_DAY_COUNT; ++i) {
		WaterSettings[i] = WaterSetting();
	}
	g_terrainVisual.m_calls.clear();
	g_undefined = 0;
	ref_water_set_block_hook = setBlock;
	ref_water_transparency_block_hook = transparencyBlock;
}

int ref_water_undefined(void)
{
	int u = g_undefined;
	g_undefined = 0;
	return u;
}

int ref_water_delete_overrides(void)
{
	if (TheWaterTransparency.getNonOverloadedPointer() == NULL) return -1;
	wtGameLogicReset();
	return 0;
}

/* the TIME_OF_DAY_COUNT water settings (sky texture, water texture, repeat count, texels per unit, the six RGBAColorInts
** in the header's order, the two scrolls); then the chain from the original, following getNextOverride: its length, and
** each node's setting; then the skybox calls: their count, and each call's ten names. */
int ref_water_dump(unsigned char *buf, int cap)
{
	g_dump.clear();
	for (int i = 0; i < TIME_OF_DAY_COUNT; ++i) {
		const WaterSetting &w = WaterSettings[i];
		putStr(w.m_skyTextureFile); putStr(w.m_waterTextureFile); putU32((unsigned)w.m_waterRepeatCount);
		putReal(w.m_skyTexelsPerUnit); putRgba(w.m_vertex00Diffuse); putRgba(w.m_vertex10Diffuse);
		putRgba(w.m_vertex11Diffuse); putRgba(w.m_vertex01Diffuse); putRgba(w.m_waterDiffuseColor);
		putRgba(w.m_transparentWaterDiffuse); putReal(w.m_uScrollPerMs); putReal(w.m_vScrollPerMs);
	}
	std::vector<const WaterTransparencySetting *> chain;
	for (const Overridable *o = TheWaterTransparency.getNonOverloadedPointer(); o; o = o->getNextOverride())
		chain.push_back((const WaterTransparencySetting *)o);
	putU32((unsigned)chain.size());
	for (size_t i = 0; i < chain.size(); ++i) putTransparency(*chain[i]);
	putU32((unsigned)(g_terrainVisual.m_calls.size() / 10));
	for (size_t i = 0; i < g_terrainVisual.m_calls.size(); ++i) putStr(g_terrainVisual.m_calls[i]);
	int len = (int)g_dump.size();
	if (len <= cap) memcpy(buf, g_dump.data(), (size_t)len);
	return len;
}
}
