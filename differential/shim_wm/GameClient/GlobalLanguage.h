/* Shim for GameEngine/Include/GameClient/GlobalLanguage.h, differential/wrap_wm.cpp only (#70): ~~no language data
** (TheGlobalLanguageData NULL), so the default font is Times New Roman 14~~ (corrected 2026-11-12, #151) the one member
** GameWindowManager.cpp reads, m_defaultWindowFont. TheGlobalLanguageData is NULL (the default font is then Times New
** Roman 14) unless a case's `l` operation gives it language data (wrap_wm.cpp); the other programs that include this
** shim (wt_ref, the hot key, command meta and keyboard camera wrappers) keep it NULL. */
#ifndef SHIM_WM_GLOBALLANGUAGE_H
#define SHIM_WM_GLOBALLANGUAGE_H
#include "Common/AsciiString.h"
struct FontDesc { AsciiString name; Int size; Bool bold; };
class GlobalLanguage
{
public:
	FontDesc m_defaultWindowFont;
};
extern GlobalLanguage *TheGlobalLanguageData;
#endif
