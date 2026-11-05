/* Differential tests only: the out-of-line StringClass members that the
** W3D loaders compiled here need (ProxyClass's name in collect.cpp, the pose
** name in hmorphanim.cpp), TRANSCRIBED from WWLib/wwstring.cpp (upstream
** commit 0a05454d), which casts pointers to a 32-bit `unsigned` and cannot
** be compiled for x86_64: the static m_NullChar/m_EmptyString, Get_String
** and Free_String (their non-temporary paths: nothing compiled here asks for
** a temporary string; the temporary paths abort), and Uninitialised_Grow
** (verbatim). The inline members are the real wwstring.h's.
**
** This file and the libraries that use it are compiled with StringClass
** renamed to ShimStringClass (build.rs), so these definitions do not clash
** with the mesh wrapper's empty-only stand-ins under the real name. */
#include <stdlib.h>
#include <string.h>
#include "wwstring.h"

TCHAR StringClass::m_NullChar = 0;
TCHAR *StringClass::m_EmptyString = &m_NullChar;

/* StringClass::Get_String: the non-temporary path. */
void StringClass::Get_String(int length, bool is_temp)
{
	if (!is_temp && length == 0) {
		m_Buffer = m_EmptyString;
		return;
	}
	if (is_temp) abort();
	if (length > 0) {
		Set_Buffer_And_Allocated_Length(Allocate_Buffer(length), length);
	} else {
		Free_String();
	}
}

/* StringClass::Free_String: the non-temporary path (no temporary buffers
** are handed out here). */
void StringClass::Free_String(void)
{
	if (m_Buffer != m_EmptyString) {
		char *buffer = ((char *)m_Buffer) - sizeof(StringClass::_HEADER);
		delete[] buffer;
		m_Buffer = m_EmptyString;
	}
}

/* StringClass::Uninitialised_Grow, verbatim (WWMEMLOG is empty). */
void StringClass::Uninitialised_Grow(int new_len)
{
	int allocated_len = Get_Allocated_Length();
	if (new_len > allocated_len) {
		TCHAR *new_buffer = Allocate_Buffer(new_len);
		Set_Buffer_And_Allocated_Length(new_buffer, new_len);
	}
	Store_Length(0);
	return;
}
