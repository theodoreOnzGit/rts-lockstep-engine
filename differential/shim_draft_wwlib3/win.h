/* (#215, group wwlib3) stand-in for WWLib's win.h as widestring.h and widestring.cpp use it: BOOL, WCHAR, the code page
** constant and MultiByteToWideChar / WideCharToMultiByte (declared; defined by wrap_draft_widestring.cpp). */
#ifndef WWLIB3_WIN_H
#define WWLIB3_WIN_H
typedef int BOOL;
typedef wchar_t WCHAR;
#define CP_ACP 0
int MultiByteToWideChar(unsigned cp, unsigned long flags, const char *mb, int mbn, wchar_t *wc, int wcn);
int WideCharToMultiByte(unsigned cp, unsigned long flags, const wchar_t *wc, int wcn, char *mb, int mbn, const char *def, BOOL *used);
#endif
