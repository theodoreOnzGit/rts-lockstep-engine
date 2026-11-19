/* Stand-in for WW3D2/dx8wrapper.h (#215, group ww3dlogic): the two device calls MatrixMapperClass::Apply makes, logged
** instead of sent to Direct3D. The logging sink is g_dww3dl_log (differential/wrap_draft_matrixmapper.cpp). Values are
** the DirectX 8 constants' (d3d8types.h). */
#ifndef DRAFT_DX8WRAPPER_STANDIN_H
#define DX8Wrapper DX8WrapperMm /* distinct from the other stand-ins (ODR, issue #219) */
#define DRAFT_DX8WRAPPER_STANDIN_H

#include <stdint.h>
#include <string.h>
#include <vector>
#include "matrix4.h"

extern std::vector<uint32_t> g_dww3dl_log;

enum D3DTRANSFORMSTATETYPE { D3DTS_TEXTURE0 = 16 };
enum { D3DTSS_TEXCOORDINDEX = 11, D3DTSS_TEXTURETRANSFORMFLAGS = 24 };
enum {
	D3DTSS_TCI_CAMERASPACENORMAL = 0x00010000,
	D3DTSS_TCI_CAMERASPACEPOSITION = 0x00020000
};
enum { D3DTTFF_COUNT2 = 2, D3DTTFF_COUNT3 = 3, D3DTTFF_PROJECTED = 256 };

class DX8Wrapper
{
public:
	static void Set_Transform(D3DTRANSFORMSTATETYPE transform, const Matrix4x4 & m)
	{
		g_dww3dl_log.push_back(1000);
		g_dww3dl_log.push_back((uint32_t)transform);
		for (int i = 0; i < 4; i++)
			for (int j = 0; j < 4; j++) {
				float f = m[i][j];
				uint32_t w; memcpy(&w, &f, 4);
				g_dww3dl_log.push_back(w);
			}
	}
	static void Set_DX8_Texture_Stage_State(unsigned stage, int state, unsigned long value)
	{
		g_dww3dl_log.push_back(1001);
		g_dww3dl_log.push_back(stage);
		g_dww3dl_log.push_back((uint32_t)state);
		g_dww3dl_log.push_back((uint32_t)value);
	}
};

#endif
