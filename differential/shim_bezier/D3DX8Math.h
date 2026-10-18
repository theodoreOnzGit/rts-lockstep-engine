/* Bezier tests only: the D3DX math of <D3DX8Math.h> that BezierSegment.cpp
** and BezFwdIterator.cpp call, transcribed from the D3DX8 inline formulas
** (d3dx8math.inl) and NOT checked against the d3dx8 library the game linked:
**   D3DXVec4Transform(out, v, m):  out[j] = m[0][j]*v[0] + m[1][j]*v[1]
**                                         + m[2][j]*v[2] + m[3][j]*v[3]
**   D3DXVec4Dot(a, b) = a[0]*b[0] + a[1]*b[1] + a[2]*b[2] + a[3]*b[3]
** (the sums left to right, one rounding per operation). */
#ifndef SHIM_BEZIER_D3DX8MATH_H
#define SHIM_BEZIER_D3DX8MATH_H
struct D3DXVECTOR4 { float x, y, z, w; D3DXVECTOR4() {} D3DXVECTOR4(float a, float b, float c, float d) : x(a), y(b), z(c), w(d) {} };
struct D3DXMATRIX
{
	float m[4][4];
	D3DXMATRIX() {}
	D3DXMATRIX(float _11, float _12, float _13, float _14, float _21, float _22, float _23, float _24,
		float _31, float _32, float _33, float _34, float _41, float _42, float _43, float _44)
	{
		m[0][0] = _11; m[0][1] = _12; m[0][2] = _13; m[0][3] = _14;
		m[1][0] = _21; m[1][1] = _22; m[1][2] = _23; m[1][3] = _24;
		m[2][0] = _31; m[2][1] = _32; m[2][2] = _33; m[2][3] = _34;
		m[3][0] = _41; m[3][1] = _42; m[3][2] = _43; m[3][3] = _44;
	}
};
inline D3DXVECTOR4 *D3DXVec4Transform(D3DXVECTOR4 *out, const D3DXVECTOR4 *v, const D3DXMATRIX *m)
{
	const float *a = &v->x;
	D3DXVECTOR4 r;
	r.x = m->m[0][0] * a[0] + m->m[1][0] * a[1] + m->m[2][0] * a[2] + m->m[3][0] * a[3];
	r.y = m->m[0][1] * a[0] + m->m[1][1] * a[1] + m->m[2][1] * a[2] + m->m[3][1] * a[3];
	r.z = m->m[0][2] * a[0] + m->m[1][2] * a[1] + m->m[2][2] * a[2] + m->m[3][2] * a[3];
	r.w = m->m[0][3] * a[0] + m->m[1][3] * a[1] + m->m[2][3] * a[2] + m->m[3][3] * a[3];
	*out = r;
	return out;
}
inline float D3DXVec4Dot(const D3DXVECTOR4 *a, const D3DXVECTOR4 *b)
{
	return a->x * b->x + a->y * b->y + a->z * b->z + a->w * b->w;
}
#endif
