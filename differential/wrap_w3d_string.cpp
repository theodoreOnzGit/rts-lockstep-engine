/* Differential tests only: the out-of-line StringClass members that the
** W3D loaders compiled here need (ProxyClass's name in collect.cpp, the pose
** name in hmorphanim.cpp), TRANSCRIBED from WWLib/wwstring.cpp (upstream
** commit 0a05454d), which casts pointers to a 32-bit `unsigned` and cannot
** be compiled for x86_64: the static m_NullChar/m_EmptyString, Get_String
** and Free_String (their non-temporary paths; a temporary string gets an
** ordinary buffer, see Get_String), and Resize and Uninitialised_Grow
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

/* StringClass::Get_String: the non-temporary path. A temporary string
** (the asset manager's "..\\" + filename, htreemgr.cpp's lower-cased
** names) gets an ordinary buffer of MAX_TEMP_LEN characters when it would
** fit one of upstream's eight temporary buffers (which hold MAX_TEMP_LEN):
** the same allocated length and contents, without the pool of buffers whose
** addresses upstream computes in 32 bits. */
void StringClass::Get_String(int length, bool is_temp)
{
	if (!is_temp && length == 0) {
		m_Buffer = m_EmptyString;
		return;
	}
	if (is_temp && length <= MAX_TEMP_LEN) {
		Set_Buffer_And_Allocated_Length(Allocate_Buffer(MAX_TEMP_LEN), MAX_TEMP_LEN);
		return;
	}
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

/* StringClass::Resize, verbatim (WWMEMLOG is empty). */
void StringClass::Resize(int new_len)
{
	int allocated_len = Get_Allocated_Length();
	if (new_len > allocated_len) {
		TCHAR *new_buffer = Allocate_Buffer(new_len);
		_tcscpy(new_buffer, m_Buffer);
		Set_Buffer_And_Allocated_Length(new_buffer, new_len);
	}
	return;
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
