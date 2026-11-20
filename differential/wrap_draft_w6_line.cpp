/* Differential-test wrapper for the 3D line render object's arithmetic (GitHub #215, upstream commit 0a05454d), group "ww3d6":
** src/drafts/ww3d/line3d.rs against WW3D2/line3d.cpp: the constructor, both Scale overloads, Get_Obj_Space_Bounding_Sphere / _Box, both Reset overloads,
** Re_Color, Set_Opacity and Get_Num_Polys, cut VERBATIM by build.rs `drafts_ww3d6` (line_*.inc, each anchor asserted to occur once). Matrix3D (Obj_Look_At),
** Vector3, Vector4, SphereClass and AABoxClass are the real headers. Render's vertex fill is not cut (it is interleaved with the DX8 dynamic buffers); its colour,
** `DX8Wrapper::Convert_Color(Color)`, is called on the wrapper's side (the transcription in wrap_w3d_mesh.cpp), and the vertices are read from `vert`.
**
** Stand-ins, each for something the cut calls outside itself:
**  - the class Line3DClass (line3d.h) with its members Length, Width, Shader, vert[8], Color, SortLevel (a char) and the three accessors the cut uses, over a
**    RenderObjClass base reduced to Transform, Set_Transform, Invalidate_Cached_Bounding_Volumes (counted) and Get_Container (NULL: no container);
**  - ShaderClass: only the two presets the cut assigns (values 1 for _PresetAlphaSolidShader, 2 for _PresetOpaqueSolidShader) and SORT_LEVEL_NONE (0, shader.h).
**
** Entry point (extern "C", tests/differential_draft_line3d.rs), uint32 words, floats by bits:
**   ref_w6_line_run(in, out) -> n    in = [start 3, end 3, width, r, g, b, opacity, nops, per op: kind + 7 words]
**       kind 0 Scale(s)  1 Scale(x, y, z)  2 Reset(start, end)  3 Reset(start, end, width)  4 Re_Color(r, g, b)  5 Set_Opacity(o)  6 Set_Sort_Level(int)
**   out = after the constructor and after every op: [shader id, sort level, Transform 12, vert 8x3, converted colour, sphere centre 3, radius,
**         box centre 3, extent 3, Get_Num_Polys, Invalidate_Cached_Bounding_Volumes calls] */
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include "always.h"
#include "vector3.h"
#include "vector4.h"
#include "matrix3d.h"
#include "sphere.h"
#include "aabox.h"

typedef uint32_t W;
static float F(W w) { float f; memcpy(&f, &w, 4); return f; }
static W B(float f) { W w; memcpy(&w, &f, 4); return w; }

namespace {

#define SORT_LEVEL_NONE 0

struct ShaderClass {
	int Id;
	static ShaderClass _PresetAlphaSolidShader;
	static ShaderClass _PresetOpaqueSolidShader;
};
ShaderClass ShaderClass::_PresetAlphaSolidShader = { 1 };
ShaderClass ShaderClass::_PresetOpaqueSolidShader = { 2 };

class RenderObjClass {
public:
	Matrix3D Transform;
	int invalidates = 0;
	void Set_Transform(const Matrix3D & m) { Transform = m; }
	void Invalidate_Cached_Bounding_Volumes(void) { invalidates++; }
	RenderObjClass * Get_Container(void) const { return NULL; }
	void Update_Obj_Space_Bounding_Volumes(void) {}
};

class Line3DClass : public RenderObjClass {
public:
	Line3DClass (const Vector3 & start, const Vector3 & end, float width, float r, float g, float b, float opacity = 1.0f);
	void Scale(float scale);
	void Scale(float scalex, float scaley, float scalez);
	int Get_Num_Polys(void) const;
	void Get_Obj_Space_Bounding_Sphere(SphereClass & sphere) const;
	void Get_Obj_Space_Bounding_Box(AABoxClass & box) const;
	void Reset(const Vector3 & new_start, const Vector3 & new_end);
	void Reset(const Vector3 & new_start, const Vector3 & new_end, float new_width);
	void Re_Color(float r, float g, float b);
	void Set_Opacity(float opacity);
	void Set_Sort_Level(int level) { SortLevel = level; }
	int Get_Sort_Level(void) const { return SortLevel; }

	float Length;
	float Width;
	ShaderClass Shader;
	Vector3 vert[8];
	Vector4 Color;
	char SortLevel;
};

#include "line_ctor.inc"
#include "line_scale1.inc"
#include "line_scale3.inc"
#include "line_sphere.inc"
#include "line_box.inc"
#include "line_reset2.inc"
#include "line_reset3.inc"
#include "line_recolor.inc"
#include "line_opacity.inc"
#include "line_polys.inc"

void dump(const Line3DClass & l, W *& o)
{
	*o++ = (W)l.Shader.Id;
	*o++ = (W)(int)l.Get_Sort_Level();
	for (int r = 0; r < 3; r++) { *o++ = B(l.Transform[r][0]); *o++ = B(l.Transform[r][1]); *o++ = B(l.Transform[r][2]); *o++ = B(l.Transform[r][3]); }
	for (int i = 0; i < 8; i++) { *o++ = B(l.vert[i].X); *o++ = B(l.vert[i].Y); *o++ = B(l.vert[i].Z); }
	*o++ = (W)DX8Wrapper::Convert_Color(l.Color);
	SphereClass s;
	l.Get_Obj_Space_Bounding_Sphere(s);
	*o++ = B(s.Center.X); *o++ = B(s.Center.Y); *o++ = B(s.Center.Z); *o++ = B(s.Radius);
	AABoxClass bx;
	l.Get_Obj_Space_Bounding_Box(bx);
	*o++ = B(bx.Center.X); *o++ = B(bx.Center.Y); *o++ = B(bx.Center.Z); *o++ = B(bx.Extent.X); *o++ = B(bx.Extent.Y); *o++ = B(bx.Extent.Z);
	*o++ = (W)l.Get_Num_Polys();
	*o++ = (W)l.invalidates;
}

}

extern "C" int ref_w6_line_run(const W *in, W *out)
{
	W *o = out;
	const W *p = in;
	Vector3 start(F(p[0]), F(p[1]), F(p[2])), end(F(p[3]), F(p[4]), F(p[5]));
	Line3DClass line(start, end, F(p[6]), F(p[7]), F(p[8]), F(p[9]), F(p[10]));
	p += 11;
	dump(line, o);
	int nops = (int)*p++;
	for (int i = 0; i < nops; i++, p += 8) {
		int kind = (int)p[0];
		const W *a = p + 1;
		Vector3 s(F(a[0]), F(a[1]), F(a[2])), e(F(a[3]), F(a[4]), F(a[5]));
		switch (kind) {
		case 0: line.Scale(F(a[0])); break;
		case 1: line.Scale(F(a[0]), F(a[1]), F(a[2])); break;
		case 2: line.Reset(s, e); break;
		case 3: line.Reset(s, e, F(a[6])); break;
		case 4: line.Re_Color(F(a[0]), F(a[1]), F(a[2])); break;
		case 5: line.Set_Opacity(F(a[0])); break;
		default: line.Set_Sort_Level((int)a[0]); break;
		}
		dump(line, o);
	}
	return (int)(o - out);
}
