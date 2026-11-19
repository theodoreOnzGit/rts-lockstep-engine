/* Forced include (-include) for differential/wrap_draft_w3ddep.cpp's translation units (GitHub #215, group ww3dpure): the two MSVC CRT
** functions w3d_dep.cpp calls that glibc does not have, and MSVC's _MAX_PATH. In the "C" locale (the one the game runs in) they change the ASCII letters
** only, as these do. */
#ifndef DEP_PRELUDE_H
#define DEP_PRELUDE_H
#ifndef _MAX_PATH
#define _MAX_PATH 260 /* MSVC's stdlib.h */
#endif
static inline char *strupr(char *s) { for (char *p = s; *p; p++) if (*p >= 'a' && *p <= 'z') *p -= 32; return s; }
static inline char *strlwr(char *s) { for (char *p = s; *p; p++) if (*p >= 'A' && *p <= 'Z') *p += 32; return s; }
#endif
