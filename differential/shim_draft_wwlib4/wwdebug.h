/* (#215, group wwlib4) stand-in for wwdebug.h: WWASSERT compiles to nothing (the release build upstream); WWDEBUG_SAY((fmt, ...)) appends the
** formatted text to a log string the wrapper can read (DefinitionMgrClass::List_Available_Definitions prints the names that way). */
#ifndef WWLIB4_WWDEBUG_H
#define WWLIB4_WWDEBUG_H
#include <string>
#include <stdarg.h>
#include <stdio.h>
#define WWASSERT(x) ((void)0)
inline std::string &wwdebug_log(void) { static std::string s; return s; }
inline void wwdebug_say(const char *fmt, ...) { char b[2048]; va_list a; va_start(a, fmt); vsnprintf(b, sizeof(b), fmt, a); va_end(a); wwdebug_log() += b; }
#define WWDEBUG_SAY(x) wwdebug_say x
#define WWRELEASE_SAY(x) ((void)0)
#define WWDEBUG_WARNING(x) ((void)0)
#define WWDEBUG_ERROR(x) ((void)0)
#endif
