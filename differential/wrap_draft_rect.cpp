/* Differential-test wrapper for the draft src/drafts/ww3d/rect.rs (GitHub #215, group ww3dlogic; upstream commit 0a05454d):
** WWMath/rect.h (RectClass), included UNCHANGED; it is header-only. Vector2 is the upstream class already linked.
**
** Entry point (extern "C", tests/differential_draft_rect.rs): ref_dww3dl_rect_run(op, p, out) -> number of words written.
** p = [a: left top right bottom, b: left top right bottom, v: x y, k] (11 floats); out receives floats (or 1 word for the
** bool). Ops (one per RectClass member, in the order of the Rust test's list): see the switch. */
#include <stdint.h>
#include <string.h>
#include "always.h"
#include "vector2.h"
#include "rect.h"

static int ORect(const RectClass &r, float *o) { o[0] = r.Left; o[1] = r.Top; o[2] = r.Right; o[3] = r.Bottom; return 4; }
static int OVec(const Vector2 &v, float *o) { o[0] = v.X; o[1] = v.Y; return 2; }

extern "C" int ref_dww3dl_rect_run(int op, const float *p, float *out)
{
	RectClass a(p[0], p[1], p[2], p[3]);
	RectClass b(p[4], p[5], p[6], p[7]);
	Vector2 v(p[8], p[9]);
	float k = p[10];
	switch (op) {
	case 0: { RectClass r(Vector2(p[0], p[1]), Vector2(p[2], p[3])); return ORect(r, out); }
	case 1: { RectClass r; r.Set(p[0], p[1], p[2], p[3]); return ORect(r, out); }
	case 2: { out[0] = a.Width(); return 1; }
	case 3: { out[0] = a.Height(); return 1; }
	case 4: return OVec(a.Center(), out);
	case 5: return OVec(a.Extent(), out);
	case 6: return OVec(a.Upper_Left(), out) + OVec(a.Lower_Right(), out + 2) + OVec(a.Upper_Right(), out + 4) + OVec(a.Lower_Left(), out + 6);
	case 7: { a.Scale(k); return ORect(a, out); }
	case 8: { a.Scale(v); return ORect(a, out); }
	case 9: { a.Inverse_Scale(v); return ORect(a, out); }
	case 10: { a.Scale_Relative_Center(k); return ORect(a, out); }
	case 11: { a += v; return ORect(a, out); }
	case 12: { a -= v; return ORect(a, out); }
	case 13: { a.Inflate(v); return ORect(a, out); }
	case 14: { a += b; return ORect(a, out); }
	case 15: { out[0] = a.Contains(v) ? 1.0f : 0.0f; return 1; }
	case 16: { a.Snap_To_Units(v); return ORect(a, out); }
	case 17: { a /= k; return ORect(a, out); }
	case 18: { a *= k; return ORect(a, out); }
	case 19: { out[0] = (a == b) ? 1.0f : 0.0f; out[1] = (a != b) ? 1.0f : 0.0f; return 2; }
	}
	return 0;
}
