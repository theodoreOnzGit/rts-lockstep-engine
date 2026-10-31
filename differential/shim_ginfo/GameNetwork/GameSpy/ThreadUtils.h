/* Stand-in for GameNetwork/GameSpy/ThreadUtils.h, differential tests only (differential/wrap_game_info.cpp): the two UTF-8 conversions, on the 16-bit WideChar (the wrapper's stand-ins; ThreadUtils.cpp calls Windows' converters). */
#ifndef SHIM_GINFO_THREADUTILS_H
#define SHIM_GINFO_THREADUTILS_H
#include <string>
typedef std::basic_string<WideChar> GiWString;
GiWString MultiByteToWideCharSingleLine( const char *orig );
std::string WideCharStringToMultiByte( const WideChar *orig );
#endif
