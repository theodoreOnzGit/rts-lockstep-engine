/* Shim for the DirectX 8 SDK header D3dx8math.h, differential tests only.
** The DirectX SDK is not part of the upstream release. matrix3d.cpp uses only
** D3DXMatrixInverse (in Matrix3D::Get_Inverse), which is NOT translated and
** NOT verified; the wrapper defines it to abort() so any accidental use fails. */
#ifndef SHIM_D3DX8MATH_H
#define SHIM_D3DX8MATH_H
struct D3DXMATRIX { float m[4][4]; };
D3DXMATRIX* D3DXMatrixInverse(D3DXMATRIX* out, float* det, const D3DXMATRIX* in);
#endif
