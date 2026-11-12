/* Differential-test wrapper for TheGlobalLanguageData (GitHub #148; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameClient/GlobalLanguage.cpp, from `GlobalLanguage *TheGlobalLanguageData = NULL;`
**     to `// PRIVATE FUNCTIONS` (the field table, INI::parseLanguageDefinition, the constructor and destructor, init,
**     reset, parseFontDesc, parseFontFileName, adjustFontSize, FontDesc::FontDesc)
** cut by build.rs into OUT_DIR/global_language_gen/global_language_cut.inc, verbatim but for one mechanical rename,
** asserted once: `void INI::parseLanguageDefinition( INI *ini )` made `static void glParseLanguageDefinition( INI *ini )`
** (the INI class is the real INI.h's, whose members another archive defines; wrap_ini.cpp's handler reaches this one
** through ref_language_block_hook, which ref_global_language_reset sets). Compiled with the REAL GameClient/GlobalLanguage.h
** and GameClient/FontDesc.h, in the science build's setup (the real INI.h, GameMemory's zero-filling stand-in).
**
** Every class and global those files define is renamed here (Gl...), so that nothing clashes with another archive's
** stand-ins. The stand-ins for what `init` and the destructor call (never called here: the test loads its files through
** the real INI::load, wrap_ini.cpp's ref_ini_load_typed): OSVERSIONINFO, GetVersionEx, GetRegistryLanguage,
** AddFontResource, RemoveFontResource. `TheGlobalData->m_xResolution` (adjustFontSize) is the reset's.
**
** Entry points (extern "C"; tests/differential_global_language.rs documents the dump):
**   ref_global_language_reset(xResolution)  a new TheGlobalLanguageData (the old one dropped), the hook set
**   ref_global_language_dump(buf, cap)      every member, little-endian; returns the length
**   ref_global_language_adjust(size)        TheGlobalLanguageData->adjustFontSize(size) */
#include <stdlib.h>
#include <string.h>
#include <vector>
#include "Common/INI.h"
#include "Common/FileSystem.h"
/* REAL_TO_INT_FLOOR of the real BaseType.h is `fast_float2long_round(fast_float_floor(x))`, MSVC `__asm` upstream: the
** shared shim's BaseType.h does not have it; the transcription of the asm is wrap_base_type.cpp's, composed as upstream
** composes it (as wrap_weapon.cpp does). */
extern "C" { int ref_fast_float2long_round(float f); float ref_fast_float_floor(float f); }
#define REAL_TO_INT_FLOOR(x) (ref_fast_float2long_round(ref_fast_float_floor(x)))

#define GlobalLanguage GlGlobalLanguage
#define TheGlobalLanguageData GlTheGlobalLanguageData
#define FontDesc GlFontDesc
#define TheGlobalData GlTheGlobalData
#include "GameClient/GlobalLanguage.h"

/* ---- what init and the destructor call (Windows, the registry), never reached here ---- */
struct OSVERSIONINFO { UnsignedInt dwOSVersionInfoSize; UnsignedInt dwPlatformId; };
enum { VER_PLATFORM_WIN32_WINDOWS = 1 };
static Bool GetVersionEx( OSVERSIONINFO *v ) { v->dwPlatformId = 2; return TRUE; }
static AsciiString GetRegistryLanguage( void ) { return AsciiString("English"); }
static Int AddFontResource( const char * ) { return 1; }
static Int RemoveFontResource( const char * ) { return 1; }
/* `TheFileSystem->doesFileExist` (init's Language9x.ini test; FileSystem.cpp is not compiled): weak, never reached */
__attribute__((weak)) Bool FileSystem::doesFileExist( const Char * ) const { return FALSE; }

/* ---- TheGlobalData->m_xResolution ---- */
struct GlGlobalData { Int m_xResolution; };
static GlGlobalData g_globalData = { 800 };
static GlGlobalData *TheGlobalData = &g_globalData;

#include "global_language_cut.inc"

namespace {
std::vector<unsigned char> g_dump;
void putU32(unsigned v) { for (int i = 0; i < 4; ++i) g_dump.push_back((unsigned char)(v >> (8 * i))); }
void putStr(const AsciiString &s) { putU32((unsigned)s.getLength()); for (int i = 0; i < s.getLength(); ++i) g_dump.push_back((unsigned char)s.str()[i]); }
void putFont(const FontDesc &f) { putStr(f.name); putU32((unsigned)f.size); putU32(f.bold ? 1u : 0u); }
void languageBlock(INI *ini) { glParseLanguageDefinition(ini); }
}	/* namespace */

extern "C" {
extern void (*ref_language_block_hook)(INI *);

void ref_global_language_reset(int xResolution)
{
	TheGlobalLanguageData = NEW GlobalLanguage;
	g_globalData.m_xResolution = xResolution;
	ref_language_block_hook = languageBlock;
}

/* m_unicodeFontName, m_unicodeFontFileName, m_useHardWrap, m_militaryCaptionSpeed, m_militaryCaptionDelayMS, the 18 fonts
** in the header's order (name, size, bold), m_resolutionFontSizeAdjustment's bits, then m_localFonts (count, names). */
int ref_global_language_dump(unsigned char *buf, int cap)
{
	g_dump.clear();
	GlobalLanguage *g = TheGlobalLanguageData;
	putStr(g->m_unicodeFontName); putStr(g->m_unicodeFontFileName); putU32(g->m_useHardWrap ? 1u : 0u);
	putU32((unsigned)g->m_militaryCaptionSpeed); putU32((unsigned)g->m_militaryCaptionDelayMS);
	putFont(g->m_copyrightFont); putFont(g->m_messageFont); putFont(g->m_militaryCaptionTitleFont); putFont(g->m_militaryCaptionFont);
	putFont(g->m_superweaponCountdownNormalFont); putFont(g->m_superweaponCountdownReadyFont);
	putFont(g->m_namedTimerCountdownNormalFont); putFont(g->m_namedTimerCountdownReadyFont);
	putFont(g->m_drawableCaptionFont); putFont(g->m_defaultWindowFont); putFont(g->m_defaultDisplayStringFont);
	putFont(g->m_tooltipFontName); putFont(g->m_nativeDebugDisplay); putFont(g->m_drawGroupInfoFont);
	putFont(g->m_creditsTitleFont); putFont(g->m_creditsPositionFont); putFont(g->m_creditsNormalFont);
	unsigned r;
	memcpy(&r, &g->m_resolutionFontSizeAdjustment, 4);
	putU32(r);
	putU32((unsigned)g->m_localFonts.size());
	for (GlobalLanguage::StringListIt it = g->m_localFonts.begin(); it != g->m_localFonts.end(); ++it) putStr(*it);
	int len = (int)g_dump.size();
	if (len <= cap) memcpy(buf, g_dump.data(), (size_t)len);
	return len;
}

int ref_global_language_adjust(int size)
{
	return TheGlobalLanguageData->adjustFontSize(size);
}
}
