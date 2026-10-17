/* Shim for GameEngine/Include/Common/Debug.h (upstream commit 0a05454d),
** differential tests only: the release-build (no-op) forms of the macros. */
#ifndef SHIM_DEBUG_H
#define SHIM_DEBUG_H
#define DEBUG_LOG(m)
#define DEBUG_ASSERTLOG(c, m)
#define DEBUG_ASSERTCRASH(c, m)
#define DEBUG_CRASH(m)
#endif
