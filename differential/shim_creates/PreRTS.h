/* Create-module differential tests only: the precompiled header the upstream files include first. Everything it
** would bring is in create_prelude.h, which the wrapper forces in before this file (the real PreRTS.h needs the
** Windows SDK). */
#ifndef SHIM_CREATES_PRERTS_H
#define SHIM_CREATES_PRERTS_H
#endif
