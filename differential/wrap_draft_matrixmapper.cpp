/* Differential-test wrapper for the draft src/drafts/ww3d/matrixmapper.rs (GitHub #215, group ww3dlogic; upstream commit
** 0a05454d): WW3D2/matrixmapper.cpp, compiled WHOLE and unchanged (#included below), over matrixmapper.h unchanged.
** Stand-ins (differential/shim_draft_ww3dl/): mapper.h (TextureMapperClass: Stage, reference count, the virtuals) and
** dx8wrapper.h (DX8Wrapper::Set_Transform and Set_DX8_Texture_Stage_State, which log their arguments to g_dww3dl_log as
** 1000 stage m[16] and 1001 stage state value). The inner mapper of a CompositeMatrixMapperClass is InnerMapper, whose
** Calculate_Texture_Matrix returns a stored matrix (upstream's inner mappers are other classes of mapper.h).
** Matrix4x4 and Matrix3D are the upstream objects already linked (matrix4.cpp, matrix3d.cpp in upstream_draft_wwmath).
**
** Entry point (extern "C", tests/differential_draft_matrixmapper.rs), every word a float's bits unless noted:
**   ref_dww3dl_mm_run(in, nin, out, cap): in = [stage, composite (0/1), has_inner, inner matrix 16, nops, ops...];
**     op 0 Set_Flag(flag, on)               -> nothing
**     op 1 Set_Type(type)                   -> nothing
**     op 2 Set_Gradient_U_Coord(f)          -> nothing
**     op 3 Set_Texture_Transform(Matrix4x4 16, texsize)
**     op 4 Set_Texture_Transform(Matrix3D 12, texsize)
**     op 5 Compute_Texture_Coordinate(x, y, z)  -> 3 words
**     op 6 Apply(uv_array_index)            -> the log (1000 ..., 1001 ... x2), prefixed by its length
**     op 7 Calculate_Texture_Matrix         -> 16 words
**     op 8 Get_Flag(flag), Get_Type, Get_Gradient_U_Coord, Get_Texture_Transform -> 1 + 1 + 1 + 16 words
**   returns the number of words written, or -1 if cap was too small. */
#include <stdint.h>
#include <string.h>
#include <vector>
#include "always.h"
#include "wwdebug.h"
#include "vector3.h"
#include "matrix3d.h"
#include "matrix4.h"
#include "dx8wrapper.h"
#include "matrixmapper.h"

std::vector<uint32_t> g_dww3dl_log;

#include "matrixmapper.cpp"

namespace {

static float f_of(uint32_t w) { float f; memcpy(&f, &w, 4); return f; }
static uint32_t w_of(float f) { uint32_t w; memcpy(&w, &f, 4); return w; }

static Matrix4x4 M4(const uint32_t *a)
{
	return Matrix4x4(f_of(a[0]), f_of(a[1]), f_of(a[2]), f_of(a[3]), f_of(a[4]), f_of(a[5]), f_of(a[6]), f_of(a[7]),
		f_of(a[8]), f_of(a[9]), f_of(a[10]), f_of(a[11]), f_of(a[12]), f_of(a[13]), f_of(a[14]), f_of(a[15]));
}

class InnerMapper : public TextureMapperClass
{
public:
	Matrix4x4 m;
	InnerMapper(const uint32_t *a) : TextureMapperClass(0), m(M4(a)) { }
	TextureMapperClass * Clone(void) const { return NULL; }
	void Apply(int) { }
	void Calculate_Texture_Matrix(Matrix4x4 & t) { t = m; }
};

}

extern "C" int ref_dww3dl_mm_run(const uint32_t *in, int nin, uint32_t *out, int cap)
{
	std::vector<uint32_t> o;
	int p = 0;
	int stage = (int)in[p++];
	int composite = (int)in[p++];
	int has_inner = (int)in[p++];
	InnerMapper *inner = has_inner ? new InnerMapper(in + p) : NULL;
	p += 16;
	MatrixMapperClass *mm = NULL;
	if (composite) {
		mm = new CompositeMatrixMapperClass(inner, (unsigned)stage);
	} else {
		mm = new MatrixMapperClass(stage);
	}
	int nops = (int)in[p++];
	for (int k = 0; k < nops; k++) {
		int op = (int)in[p++];
		switch (op) {
		case 0: { uint32 flag = in[p++]; bool on = in[p++] != 0; mm->Set_Flag(flag, on); break; }
		case 1: { mm->Set_Type((MatrixMapperClass::MappingType)in[p++]); break; }
		case 2: { mm->Set_Gradient_U_Coord(f_of(in[p++])); break; }
		case 3: { Matrix4x4 m = M4(in + p); p += 16; float ts = f_of(in[p++]); mm->Set_Texture_Transform(m, ts); break; }
		case 4: {
			Matrix3D m(f_of(in[p]), f_of(in[p+1]), f_of(in[p+2]), f_of(in[p+3]), f_of(in[p+4]), f_of(in[p+5]),
				f_of(in[p+6]), f_of(in[p+7]), f_of(in[p+8]), f_of(in[p+9]), f_of(in[p+10]), f_of(in[p+11]));
			p += 12; float ts = f_of(in[p++]); mm->Set_Texture_Transform(m, ts); break;
		}
		case 5: {
			Vector3 pt(f_of(in[p]), f_of(in[p+1]), f_of(in[p+2])); p += 3;
			Vector3 r;
			mm->Compute_Texture_Coordinate(pt, &r);
			o.push_back(w_of(r.X)); o.push_back(w_of(r.Y)); o.push_back(w_of(r.Z));
			break;
		}
		case 6: {
			int uv = (int)in[p++];
			g_dww3dl_log.clear();
			mm->Apply(uv);
			o.push_back((uint32_t)g_dww3dl_log.size());
			for (size_t i = 0; i < g_dww3dl_log.size(); i++) o.push_back(g_dww3dl_log[i]);
			break;
		}
		case 7: {
			Matrix4x4 t;
			mm->Calculate_Texture_Matrix(t);
			for (int i = 0; i < 4; i++) for (int j = 0; j < 4; j++) o.push_back(w_of(t[i][j]));
			break;
		}
		case 8: {
			uint32 flag = in[p++];
			o.push_back(mm->Get_Flag(flag) ? 1 : 0);
			o.push_back((uint32_t)mm->Get_Type());
			o.push_back(w_of(mm->Get_Gradient_U_Coord()));
			const Matrix4x4 & t = mm->Get_Texture_Transform();
			for (int i = 0; i < 4; i++) for (int j = 0; j < 4; j++) o.push_back(w_of(t[i][j]));
			break;
		}
		}
	}
	delete mm;
	delete inner;
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); i++) out[i] = o[i];
	return (int)o.size();
}
