/* Shim for GameEngine/Include/GameClient/GlobalLanguage.h, differential/wrap_wm.cpp only (#70): no language data
** (TheGlobalLanguageData NULL), so the default font is Times New Roman 14. */
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
