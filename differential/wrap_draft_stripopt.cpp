/* Differential-test wrapper for the triangle-strip drafts (GitHub #215, upstream commit 0a05454d), group "ww3dpure":
** src/drafts/ww3d/stripoptimizer.rs against WW3D2/stripoptimizer.cpp.
**
** The upstream .cpp is compiled WHOLE (build.rs `drafts_ww3dpure` copies it, and stripoptimizer.h, byte for byte except three
** mechanical edits: `for (i = 0; ...)` -> `for (int i = 0; ...)` at the three loops that reuse a loop variable declared by an
** earlier `for` (VC6's pre-standard for-scope, which clang has no flag for), each edit asserted to apply once) and #included
** here, so the file-local templates (Insertion_Sort, Quick_Sort) and `struct Tri` are instantiated in this translation unit.
** hashtemplate.h, always.h and wwstring.h are the real headers; wwdebug.h is differential/shim/wwdebug.h (release macros).
** No stand-ins.
**
** Entry points (extern "C", tests/differential_draft_stripopt.rs); each returns the number of ints written to `out`
** (or -1 for a NULL result):
**   ref_dpw_stripify(tris, n, out, cap)        StripOptimizerClass::Stripify
**   ref_dpw_combine(strips, count, out, cap)   Combine_Strips (out = [length, indices...])
**   ref_dpw_opt_strips(strips, count, total)   Optimize_Strip_Order (in place; `strips` holds `total` ints)
**   ref_dpw_opt_tris(tris, n)                  Optimize_Triangle_Order (in place)
**   ref_dpw_index_count(strips, count)         Get_Strip_Index_Count
**   ref_dpw_insertion_sort(a, n), ref_dpw_quick_sort(a, n)          on int
**   ref_dpw_insertion_sort_tri(a, n), ref_dpw_quick_sort_tri(a, n)  on `struct Tri` (ordered by `a` only), 3 ints each */
#include <stdint.h>
#include "stripoptimizer.cpp"

extern "C" {

int ref_dpw_stripify(const int *tris, int n, int *out, int cap) {
	int *r = StripOptimizerClass::Stripify(tris, n);
	if (!r) return -1;
	int len = 0;
	// layout: strip_count, then strip_count strips of (len, indices)
	int p = 1;
	for (int i = 0; i < r[0]; i++) p += 1 + r[p];
	len = p;
	if (len > cap) { delete[] r; return -2; }
	for (int i = 0; i < len; i++) out[i] = r[i];
	delete[] r;
	return len;
}

int ref_dpw_combine(const int *strips, int count, int *out, int cap) {
	int *r = StripOptimizerClass::Combine_Strips(strips, count);
	int len = r[0] + 1;
	if (len > cap) { delete[] r; return -2; }
	for (int i = 0; i < len; i++) out[i] = r[i];
	delete[] r;
	return len;
}

void ref_dpw_opt_strips(int *strips, int count, int total) {
	(void)total;
	StripOptimizerClass::Optimize_Strip_Order(strips, count);
}

void ref_dpw_opt_tris(int *tris, int n) { StripOptimizerClass::Optimize_Triangle_Order(tris, n); }

int ref_dpw_index_count(const int *strips, int count) { return StripOptimizerClass::Get_Strip_Index_Count(strips, count); }

void ref_dpw_insertion_sort(int *a, int n) { Insertion_Sort<int>(a, n); }
void ref_dpw_quick_sort(int *a, int n) { Quick_Sort<int>(a, n); }
void ref_dpw_insertion_sort_tri(int *a, int n) { Insertion_Sort<Tri>((Tri *)a, n); }
void ref_dpw_quick_sort_tri(int *a, int n) { Quick_Sort<Tri>((Tri *)a, n); }

}
