/* Shim, differential tests only: the two DirectDraw caps bits ddsfile.cpp
** tests (DirectX SDK ddraw.h values; the SDK is not in EA's release). */
#ifndef SHIM_DDRAW_H
#define SHIM_DDRAW_H
#define DDSCAPS2_CUBEMAP 0x00000200L
#define DDSCAPS2_VOLUME 0x00200000L
#endif
