/* Shim, AsciiString build only (differential tests): AsciiString.cpp
** includes Common/CriticalSection.h but uses nothing from it (the critical
** sections guard other code); the real header needs windows.h. */
