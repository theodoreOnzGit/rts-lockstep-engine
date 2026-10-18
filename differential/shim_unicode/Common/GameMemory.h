/* Forwarder, UnicodeString build only (differential tests): the allocator
** shim of differential/shim_dict (it returns the size asked for from
** getActualAllocationSize, where the real one rounds up to pool sizes;
** that changes only how much spare room a buffer has, never a value, except
** where upstream is undefined, which the tests do not send). */
#include "../../shim_dict/Common/GameMemory.h"
