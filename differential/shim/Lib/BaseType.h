/* Shim for Libraries/Include/Lib/BaseType.h (upstream commit 0a05454d),
** differential tests only. The real header uses MSVC-only constructs
** (__int64, __forceinline, __asm blocks) that g++ cannot compile. Only the
** fundamental typedefs are reproduced here, copied from the real header with
** __int64 spelled as long long. The asm helpers of the real header are
** transcribed separately in differential/wrap_base_type.cpp. */
#ifndef _BASE_TYPE_H_
#define _BASE_TYPE_H_
#include <math.h>
#include <string.h>
typedef float              Real;
typedef int                Int;
typedef unsigned int       UnsignedInt;
typedef unsigned short     UnsignedShort;
typedef short              Short;
typedef unsigned char      UnsignedByte;
typedef char               Byte;
typedef char               Char;
typedef bool               Bool;
typedef long long          Int64;
typedef unsigned long long UnsignedInt64;
#ifndef TRUE
#define TRUE true
#endif
#ifndef FALSE
#define FALSE false
#endif
#endif
