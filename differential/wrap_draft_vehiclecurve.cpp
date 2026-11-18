/* Differential-test wrapper for the draft src/drafts/wwlib/vehiclecurve.rs (and the Curve3DClass base it uses,
** curve.rs) (#215), upstream commit 0a05454d:
**   Libraries/Source/WWVegas/WWMath/vehiclecurve.h + vehiclecurve.cpp and curve.h + curve.cpp, both compiled UNCHANGED
**   as their own translation units (clang++ -fms-extensions as the W3D builds, with vector_this.h and the
**   lower-case forwarders build.rs writes).
** Stand-ins (what the cuts call outside themselves): PersistFactoryClass's constructor and destructor
** (WWSaveLoad/persistfactory.cpp, which registers the factory with SaveLoadSystemClass; the factories are never used
** here, so the registration is omitted). ChunkSaveClass and ChunkLoadClass are chunkio.cpp's, already linked.
** Save/Load are not exercised.
**
** Entry point draft_vc_run(prog, n, out): `prog` is a sequence of operations, each an opcode float followed by its
** arguments; every operation appends its results to `out` (returns the count written):
**   0 radius                new VehicleCurveClass(radius) (a fresh curve)            -> nothing
**   1 x y z t               Add_Key(point, t)                                       -> index
**   2 i x y z               Set_Key(i, point)                                       -> nothing
**   3 i                     Remove_Key(i)                                           -> nothing
**   4                       Clear_Keys()                                            -> nothing
**   5 t                     Evaluate(t, &v)                                         -> v.x v.y v.z, sharpness, sharp pos
**                                                                                    x y z, Get_Last_Eval_Time()
**   6                       Key_Count, Get_Start_Time, Get_End_Time, Is_Looping      -> 4 values
**   7 on                    Set_Looping(on)                                         -> nothing
**   8 i                     Get_Key(i, &p, &t)                                      -> p.x p.y p.z t */
#include <stdlib.h>
#include <string.h>
#include "vehiclecurve.h"
#include "persistfactory.h"

PersistFactoryClass::PersistFactoryClass(void) : NextFactory(NULL)
{
}
/* SimplePersistFactoryClass::Copy_Pointer (persistfactory.h) calls this; the factories are never used here */
#include "saveload.h"
void SaveLoadSystemClass::Register_Pointer(void * old_pointer, void * new_pointer)
{
}
PersistFactoryClass::~PersistFactoryClass(void)
{
}

extern "C" int draft_vc_run(const float *prog, int n, float *out)
{
	VehicleCurveClass *vc = new VehicleCurveClass(0.0f);
	int w = 0;
	int i = 0;
	while (i < n) {
		int op = (int)prog[i++];
		switch (op) {
		case 0: delete vc; vc = new VehicleCurveClass(prog[i]); i += 1; break;
		case 1: out[w++] = (float)vc->Add_Key(Vector3(prog[i], prog[i + 1], prog[i + 2]), prog[i + 3]); i += 4; break;
		case 2: vc->Set_Key((int)prog[i], Vector3(prog[i + 1], prog[i + 2], prog[i + 3])); i += 4; break;
		case 3: vc->Remove_Key((int)prog[i]); i += 1; break;
		case 4: vc->Clear_Keys(); break;
		case 5: {
			Vector3 v(0, 0, 0);
			vc->Evaluate(prog[i], &v);
			i += 1;
			Vector3 sp;
			float sh = vc->Get_Current_Sharpness(&sp);
			out[w++] = v.X; out[w++] = v.Y; out[w++] = v.Z;
			out[w++] = sh; out[w++] = sp.X; out[w++] = sp.Y; out[w++] = sp.Z;
			out[w++] = vc->Get_Last_Eval_Time();
			break;
		}
		case 6:
			out[w++] = (float)vc->Key_Count();
			out[w++] = vc->Get_Start_Time();
			out[w++] = vc->Get_End_Time();
			out[w++] = vc->Is_Looping() ? 1.0f : 0.0f;
			break;
		case 7: vc->Set_Looping(prog[i] != 0.0f); i += 1; break;
		case 8: {
			Vector3 p; float t;
			vc->Get_Key((int)prog[i], &p, &t);
			i += 1;
			out[w++] = p.X; out[w++] = p.Y; out[w++] = p.Z; out[w++] = t;
			break;
		}
		}
	}
	delete vc;
	return w;
}
