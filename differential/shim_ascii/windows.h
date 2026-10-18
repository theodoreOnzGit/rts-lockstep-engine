/* Shim for <windows.h>, AsciiString build only (differential tests): the two
** functions AsciiString.h calls on its reference count,
** InterlockedIncrement/Decrement((long *)&m_data->m_refCount). On Win32
** `long` is 32 bits, so they change the 32-bit word at the address (the
** 16-bit count is its low half); this does the same, single-threaded. */
#ifndef SHIM_ASCII_WINDOWS_H
#define SHIM_ASCII_WINDOWS_H
inline long InterlockedIncrement(long *p) { int *q = (int *)p; return ++*q; }
inline long InterlockedDecrement(long *p) { int *q = (int *)p; return --*q; }
#endif
