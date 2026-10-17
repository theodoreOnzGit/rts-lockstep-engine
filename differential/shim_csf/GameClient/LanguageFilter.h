/* Shim for GameEngine/Include/GameClient/LanguageFilter.h (upstream commit
** 0a05454d), differential tests only. GameText.cpp calls only
** TheLanguageFilter->filterLine, in the map string-file parser, which the
** tests do not reach; the real header's inline helpers need
** UnicodeString::compareNoCase, which the shim does not provide. The stand-in
** filterLine is defined in differential/wrap_csf.cpp to abort. */
#ifndef SHIM_LANGUAGEFILTER_H
#define SHIM_LANGUAGEFILTER_H
#include "Common/UnicodeString.h"
class LanguageFilter
{
public:
	void filterLine(UnicodeString &line);
};
extern LanguageFilter *TheLanguageFilter;
#endif
