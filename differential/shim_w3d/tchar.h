/* Shim, differential tests only: the MSVC <tchar.h> names WWLib string
** classes use, for the non-Unicode build (TCHAR is char). */
#ifndef SHIM_TCHAR_H
#define SHIM_TCHAR_H
#include <string.h>
typedef char TCHAR;
typedef wchar_t WCHAR;
#define _tcslen strlen
#define _tcsclen strlen
#define _tcscmp strcmp
#define _tcsicmp strcasecmp
#define _tcscpy strcpy
#define _tcsncpy strncpy
#define _tcscat strcat
#endif
