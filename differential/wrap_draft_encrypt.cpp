/* Differential-test wrapper for the encrypt draft (#215, upstream commit 0a05454d), src/drafts/logic_common/encrypt.rs:
**   GeneralsMD/Code/GameEngine/Source/Common/System/encrypt.cpp, from `#define MAX_CHARS 65` to `#ifdef UNIT_TEST`
**   (Base_String, Return_Buffer, Temp_Buffer, EncryptString), cut verbatim by build.rs into encrypt.inc.
**
** Stand-ins: PreRTS.h and Common/encrypt.h are not read (the cut starts after them); MAX_ENCRYPTED_STRING (encrypt.h's
** `#define MAX_ENCRYPTED_STRING 8`) and the prototype are written here; <stdio.h>, <string.h> as the file includes them.
**
** Entry points (extern "C", tests/differential_draft_encrypt.rs):
**   ref_encrypt_reset()                  zeroes the two static buffers (as a fresh process has them)
**   ref_encrypt(string, out)             EncryptString(string); copies the 8 returned characters to out (the 9th is NUL) */
#include <stdio.h>
#include <string.h>
#define MAX_ENCRYPTED_STRING 8
#ifndef NULL
#define NULL 0
#endif
#include "encrypt.inc"

extern "C" {

void ref_encrypt_reset(void)
{
	memset(Return_Buffer, 0, sizeof(Return_Buffer));
	memset(Temp_Buffer, 0, sizeof(Temp_Buffer));
}

void ref_encrypt(const char *string, char *out)
{
	const char *r = EncryptString(string);
	memcpy(out, r, MAX_ENCRYPTED_STRING);
}

}
