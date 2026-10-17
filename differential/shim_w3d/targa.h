/* Shim, differential tests only: WWLib/TARGA.H under the lower-case name
** the sources include, with `long` spelled `int` while it is read. Targa's
** structs (TGA2Footer, TGA2Extension) and its size arithmetic use `long`,
** which is 32 bits on Win32 but 64 on x86_64 Linux; nothing else changes.
** The standard headers TARGA.CPP includes are included first, so the
** definition cannot reach them. build.rs compiles TARGA.CPP
** as a copy that defines `long` as `int` in the same way (see build.rs). */
#ifndef SHIM_TARGA_H
#define SHIM_TARGA_H
#include <stdlib.h>
#include <malloc.h>
#include <memory.h>
#include <string.h>
#define long int
#include "TARGA.H"
#undef long
#endif
