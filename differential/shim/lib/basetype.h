/* Shim, differential tests only: upstream includes BaseType.h under several
** spellings (file.h: "lib/basetype.h", List.h: <lib/BaseType.h>), which a
** case-insensitive Windows file system resolves to Lib/BaseType.h. This
** forwards to the shim Lib/BaseType.h.
**
** It also makes NULL the integer 0, as the real Lib/BaseType.h does (lines
** 90-93; MSVC's C++ NULL is 0 too): file.h declares its pure virtuals as
** `= NULL`, which g++ rejects for its own `__null`. This is done here, not
** in Lib/BaseType.h, because system headers included later redefine NULL,
** and file.h includes this spelling right before its class. */
#include "Lib/BaseType.h"
#undef NULL
#define NULL 0
