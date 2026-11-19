/* Differential-test wrapper for the draft src/drafts/wwlib/sampler.rs (#215, group wwlib2), upstream commit 0a05454d:
**   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/sampler.h + sampler.cpp (SamplingClass and the Random, Regular, Stratified and
**   QMC samplers, and the file-scope `Random4Class Random`) and random.cpp (Random4Class, which Get_Float draws from), both
**   #included UNCHANGED.
** Stand-ins: RANDOM.H is named "random.h" by its includers (MSVC ignores case): a lower-case forwarder in
**   OUT_DIR/draft_wwlib2_gen (build.rs); <stddef.h> first (always.h uses size_t); the random classes and the global `Random`
**   carry -D renames (Ww2...), so that this translation unit's definitions never meet upstream_w3d_emit's own copy of random.cpp
**   (#219). The elementary functions are not used.
**
** Entry point ref_sampler_run(kind, dims, divs, seed, prog, nprog, out): a sampler of `kind` (0 Random, 1 Regular, 2 Stratified,
**   3 QMC) with `dims` dimensions and `divs` divisions, the global `Random` reseeded with `seed` (Random4Class(seed)), then the
**   program: 0 = Sample (appends dims floats as bits to `out`), 1 = Reset(), 2 x = QMC Set_Offset(x) (ignored for other kinds).
**   Returns the number of words written. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/random.cpp"
#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/sampler.cpp"

extern "C" int ref_sampler_run(int kind, unsigned dims, int divs, unsigned seed, const unsigned *prog, int nprog, unsigned *out)
{
	Random = Random4Class(seed);
	SamplingClass *s = 0;
	switch (kind) {
	case 0: s = new RandomSamplingClass(dims, (unsigned char)divs); break;
	case 1: s = new RegularSamplingClass(dims, (unsigned char)divs); break;
	case 2: s = new StratifiedSamplingClass(dims, (unsigned char)divs); break;
	default: s = new QMCSamplingClass(dims, (unsigned char)divs); break;
	}
	int w = 0;
	float *target = new float[dims + 1];
	for (int i = 0; i < nprog;) {
		switch (prog[i]) {
		case 0:
			s->Sample(target);
			for (unsigned d = 0; d < dims; d++) memcpy(&out[w++], &target[d], 4);
			i += 1;
			break;
		case 1: s->Reset(); i += 1; break;
		default:
			if (kind == 3) ((QMCSamplingClass *)s)->Set_Offset(prog[i + 1]);
			i += 2;
			break;
		}
	}
	delete[] target;
	delete s;
	return w;
}
