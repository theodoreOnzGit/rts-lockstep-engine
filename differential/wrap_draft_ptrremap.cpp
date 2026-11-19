/* Differential-test wrapper for the draft src/drafts/wwlib/pointer_remap.rs (#215, group wwlib2), upstream commit 0a05454d:
**   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/pointerremap.h + pointerremap.cpp (PointerRemapClass), #included
**   UNCHANGED, in the release configuration (no WWDEBUG: Request_Pointer_Remap takes only the location), with the real
**   always.h, the generated Vector.H (this-> qualified, build.rs drafts_codec), refcount.h and listnode.h.
** Stand-ins: <stddef.h> first; refcount.h's lower-case spelling is the same file. RefCountClass is the real header's (Add_Ref is
**   inline under NDEBUG), so the ref-counted requests really do Add_Ref() the new object.
**
** Entry point ref_ptrremap_run(in, n, out): `in` words (64-bit):
**   [K] the number of RefCountClass objects the new pointers point to (a fresh object each, Num_Refs 1);
**   [S] stale requests registered and then Reset() before the real ones: S, then S old values (a plain request each), then S
**       old values (ref-counted requests), plus one stale pair (0xdead, object 0) when K > 0;
**   [P] then P pairs (old pointer value, object index);
**   [Q] then Q old values for Request_Pointer_Remap, [R] then R old values for Request_Ref_Counted_Pointer_Remap.
**   Process() is called once. `out`: for each plain request in registration order the object index its location now holds, or -1
**   for NULL; the same for the ref-counted requests; then Num_Refs() of each object. Returns the words written. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/pointerremap.cpp"

extern "C" int ref_ptrremap_run(const unsigned long long *in, int n, long long *out)
{
	int i = 0;
	int K = (int)in[i++];
	RefCountClass **objs = new RefCountClass *[K + 1];
	for (int k = 0; k < K; k++) objs[k] = new RefCountClass();
	PointerRemapClass pr;
	int S = (int)in[i++];
	void **stale = new void *[2 * S + 2];
	for (int k = 0; k < 2 * S; k++) stale[k] = (void *)in[i++];
	for (int k = 0; k < S; k++) pr.Request_Pointer_Remap(&stale[k]);
	for (int k = 0; k < S; k++) pr.Request_Ref_Counted_Pointer_Remap((RefCountClass **)&stale[S + k]);
	if (K > 0 && S > 0) pr.Register_Pointer((void *)0xdead, objs[0]);
	pr.Reset();
	int P = (int)in[i++];
	for (int k = 0; k < P; k++) {
		unsigned long long old = in[i++];
		int idx = (int)in[i++];
		pr.Register_Pointer((void *)old, objs[idx]);
	}
	int Q = (int)in[i++];
	void **plain = new void *[Q + 1];
	for (int k = 0; k < Q; k++) plain[k] = (void *)in[i++];
	for (int k = 0; k < Q; k++) pr.Request_Pointer_Remap(&plain[k]);
	int R = (int)in[i++];
	void **counted = new void *[R + 1];
	for (int k = 0; k < R; k++) counted[k] = (void *)in[i++];
	for (int k = 0; k < R; k++) pr.Request_Ref_Counted_Pointer_Remap((RefCountClass **)&counted[k]);
	pr.Process();
	int w = 0;
	void **sets[2] = {plain, counted};
	int counts[2] = {Q, R};
	for (int s = 0; s < 2; s++) {
		for (int k = 0; k < counts[s]; k++) {
			long long r = -1;
			for (int o = 0; o < K; o++) if (sets[s][k] == (void *)objs[o]) r = o;
			if (sets[s][k] != 0 && r < 0) r = -2; /* a pointer that is neither NULL nor an object: the old value left in place */
			out[w++] = r;
		}
	}
	for (int o = 0; o < K; o++) out[w++] = objs[o]->Num_Refs();
	return w;
}
