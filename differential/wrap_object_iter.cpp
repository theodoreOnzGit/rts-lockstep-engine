/* Differential-test wrapper for SimpleObjectIterator (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SimpleObjectIterator.cpp
** (#included below, unchanged; GameLogic/ObjectIter.h is the real header,
** GameType.h the real one, and the Object and ThingTemplate of
** differential/shim_thing are its stand-ins for layers 5 and 8).
**
** The stand-ins are renamed as in wrap_thing.cpp, so that one binary can
** link both. The renames must match there: the two units share the stand-in
** headers, and identical class names would be one class to the linker. */
#define Coord3D ThingTestCoord3D
#define ThingTemplate ThingTestTemplate
#define Object ThingTestObject

#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SimpleObjectIterator.cpp"

namespace {
const int MAX_OBJECTS = 64;
ThingTestTemplate *g_templates[MAX_OBJECTS + 1];
ThingTestObject *g_objects[MAX_OBJECTS + 1];

/* the id of a stand-in object (1..nObjs), 0 for none */
unsigned idOf(const ThingTestObject *obj, int nObjs)
{
	for (int k = 1; k <= nObjs; ++k)
		if (g_objects[k] == obj) return (unsigned)k;
	return 0;
}
}

extern "C" {

/* ref_object_iter_run: a program on one SimpleObjectIterator of objects
** 1..nObjs, object i having build cost costs[i].
**   ops[3*i..] = (code, arg1, nums[i] = numeric)
**   0 insert(object arg1, numeric)
**   1 sort(order arg1)                                   (none)
**   2 firstWithNumeric / nextWithNumeric to the end: for each element its
**     object id and numeric bits, then (0xFFFFFFFF, numeric bits of the end)
**   3 first / next to the end: each object id, then 0xFFFFFFFF
**   4 getCount()                                         1 word
**   5 makeEmpty()                                        (none)
** The program's output is the concatenation of the words written.
** Returns the number of words, or -1 if they would not fit in cap.
** Object ids are never 0xFFFFFFFF, so the end marker is unambiguous. */
int ref_object_iter_run(int nObjs, const int *costs, int nOps, const int *ops, const float *nums,
	unsigned *out, int cap)
{
	if (nObjs > MAX_OBJECTS) return -2;
	for (int i = 1; i <= nObjs; ++i) {
		if (!g_templates[i]) g_templates[i] = new ThingTestTemplate(KindOfMaskType(), costs[i]);
		else *g_templates[i] = ThingTestTemplate(KindOfMaskType(), costs[i]);
		if (!g_objects[i]) g_objects[i] = new ThingTestObject(g_templates[i]);
	}
	SimpleObjectIterator it;
	int n = 0;
	for (int i = 0; i < nOps; ++i) {
		if (n + 4096 > cap) return -1;
		const int code = ops[3 * i];
		const int arg = ops[3 * i + 1];
		switch (code) {
		case 0:
			it.insert(g_objects[arg], nums[i]);
			break;
		case 1:
			it.sort((IterOrderType)arg);
			break;
		case 2: {
			Real num;
			Object *obj = it.firstWithNumeric(&num);
			while (obj) {
				out[n++] = idOf(obj, nObjs);
				unsigned u;
				memcpy(&u, &num, 4);
				out[n++] = u;
				obj = it.nextWithNumeric(&num);
			}
			unsigned u;
			memcpy(&u, &num, 4);
			out[n++] = 0xFFFFFFFFu;
			out[n++] = u;
			break;
		}
		case 3: {
			Object *obj = it.first();
			while (obj) {
				out[n++] = idOf(obj, nObjs);
				obj = it.next();
			}
			out[n++] = 0xFFFFFFFFu;
			break;
		}
		case 4:
			out[n++] = (unsigned)it.getCount();
			break;
		case 5:
			it.makeEmpty();
			break;
		default:
			return -3;
		}
	}
	return n;
}

} /* extern "C" */
