/* (#215) The curve and spline wrappers use the real WWLib/always.h (WWLib/Vector.H's `W3DNEWARRAY` is defined there; the
** shim's always.h, which the other wrappers use, does not define it). Forwarded unchanged, with _OPERATOR_NEW_DEFINED_ defined
** first so that its replacement operator new and delete (declared, never defined in these tests) and its placement operator
** new[] (which collides with <new>'s) are skipped and the C++ runtime's are used. <stddef.h> is read first by the wrapper
** (always.h uses size_t, which the Windows headers gave it). */
#define _OPERATOR_NEW_DEFINED_
#include "../../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/always.h"
