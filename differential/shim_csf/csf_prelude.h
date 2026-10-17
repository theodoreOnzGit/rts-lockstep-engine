/* Forced include (-include) for GameText.cpp, differential tests only. The
** real precompiled header PreRTS.h brings in windows.h and most of the
** engine; GameText.cpp relies on it for: SubsystemInterface (GameText.h's
** base class), <wctype.h> (iswspace), MSVC's `_cdecl` (nothing on x86_64),
** BaseType.h's PI (GameCommon.h, real Lib/BaseType.h line 86), and, for
** setting the window title after loading, the Win32 HWND type and
** SetWindowText/SetWindowTextW. The two functions are declared here and
** defined in differential/wrap_csf.cpp to abort: the tests leave
** ApplicationHWnd NULL, so they are never called. The .big prelude is
** reused for the MSVC string names (stricmp). */
#ifndef SHIM_CSF_PRELUDE_H
#define SHIM_CSF_PRELUDE_H
#include "../shim/archive_prelude.h"
#include <wctype.h>
#include <stdarg.h>
#include "Common/SubsystemInterface.h"
#define _cdecl
#ifndef PI
#define PI 3.14159265359f
#endif
typedef struct ShimHWND__ *HWND;
int SetWindowText(HWND, const char *);
int SetWindowTextW(HWND, const WideChar *);
#endif
