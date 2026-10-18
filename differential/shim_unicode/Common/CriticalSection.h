/* Shim, UnicodeString build only (differential tests): UnicodeString.cpp
** guards its reference counting with a ScopedCriticalSection on
** TheUnicodeStringCriticalSection. The real header needs windows.h; the
** differential tests are single-threaded, so the lock does nothing. The
** pointer is defined by wrap_unicode_string.cpp. */
#ifndef SHIM_UNICODE_CRITICALSECTION_H
#define SHIM_UNICODE_CRITICALSECTION_H
class CriticalSection {};
class ScopedCriticalSection
{
public:
	explicit ScopedCriticalSection(CriticalSection *) {}
};
extern CriticalSection *TheUnicodeStringCriticalSection;
#endif
