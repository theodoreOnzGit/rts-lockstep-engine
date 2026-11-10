/* differential/wrap_wm.cpp only (#70): the Win32 calls the gadget files make. */
#ifndef SHIM_WM_WINDOWS_H
#define SHIM_WM_WINDOWS_H
#include <stdint.h>
#include <wctype.h>
#define VK_RETURN 0x0D
/* MSVC's iswascii (Common/Language.h uses it) */
inline int iswascii(wint_t c) { return c < 0x80; }
unsigned int GetDoubleClickTime(void);
unsigned int timeGetTime(void);
/* windows.h's min and max macros (GadgetTabControl.cpp), as functions so the C++ library headers still compile */
template <class T> inline T min(T a, T b) { return a < b ? a : b; }
template <class T> inline T max(T a, T b) { return a > b ? a : b; }
#endif
