/* Shim for WWDebug/wwdebug.h (upstream commit 0a05454d), differential tests only.
** The real header includes the GameEngine Debug.h by a Windows relative path.
** These are the release-build (no-op) definitions from the real header. */
#ifndef WWDEBUG_H
#define WWDEBUG_H
#define WWDEBUG_SAY(x)
#define WWDEBUG_WARNING(x)
#define WWRELEASE_SAY(x)
#define WWRELEASE_WARNING(x)
#define WWRELEASE_ERROR(x)
#define WWASSERT( expr )
#define WWASSERT_PRINT( expr, string )
#define W3D_DIE
#define WWDEBUG_ERROR(x)
#endif
