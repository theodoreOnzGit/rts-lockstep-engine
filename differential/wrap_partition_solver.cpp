/* Differential-test wrapper for GameEngine/Source/Common/PartitionSolver.cpp
** and Include/Common/PartitionSolver.h (upstream commit 0a05454d). The .cpp
** is #included unchanged, after the headers the real PreRTS.h brings
** (<vector>, <algorithm>, GameType.h for ObjectID). DEBUG_CRASH is the shim's
** no-op, as in a release build.
**
** ref_partition_solve(how, ...) builds PartitionSolver from the entries
** (ids, sizes) and spaces (ids, capacities), runs solve() and writes the
** solution as (entry id, space id) pairs to out. Returns the pair count, or
** -1 if out (capacity outCap pairs) is too small. how: 0 fast, 1 correct.
** std::sort here is libstdc++'s: the caller must send only inputs whose sort
** keys (sizes) are distinct within the entries and within the spaces. */
#include <vector>
#include <algorithm>
#include "Common/GameType.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/PartitionSolver.cpp"

extern "C" int ref_partition_solve(int how, const int *dataIds, const unsigned *dataSizes, int nData,
                                   const int *spaceIds, const unsigned *spaceSizes, int nSpaces,
                                   int *out, int outCap)
{
	EntriesVec e;
	SpacesVec s;
	for (int i = 0; i < nData; ++i) e.push_back(std::make_pair((ObjectID)dataIds[i], (UnsignedInt)dataSizes[i]));
	for (int i = 0; i < nSpaces; ++i) s.push_back(std::make_pair((ObjectID)spaceIds[i], (UnsignedInt)spaceSizes[i]));
	PartitionSolver p(e, s, how == 0 ? PREFER_FAST_SOLUTION : PREFER_CORRECT_SOLUTION);
	p.solve();
	const SolutionVec &r = p.getSolution();
	if ((int)r.size() > outCap) return -1;
	for (size_t i = 0; i < r.size(); ++i) { out[2 * i] = (int)r[i].first; out[2 * i + 1] = (int)r[i].second; }
	return (int)r.size();
}
