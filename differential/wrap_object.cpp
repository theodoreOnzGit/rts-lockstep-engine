/* Differential-test wrapper for Object (p1w6-object). Compiles the text of
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp  and  Include/GameLogic/Object.h (class Object)
** with the includes and the untranslated regions removed by build.rs (upstream is never edited; every cut is
** asserted to occur once), together with the class texts and the functions Object reaches in Thing.cpp,
** ExperienceTracker.cpp and Geometry.cpp, and the enums, Coord3D, ICoord3D and Region3D verbatim from the
** headers, on the stand-ins of differential/shim_object (forced in: object_prelude.h). Everything is in the
** namespace ObjOpWorld so that no symbol of this archive collides with another fixture's.
**
** Entry point (extern "C", tests/differential_object.rs): ref_object_run runs a program of operations on a
** world of objects and writes what each one observes, as text lines: the records of the cut points, the
** client events and the module calls, the return values, and a snapshot of every object after each
** operation (the XferSave bytes of the object, hashed). The Rust interpreter writes the same lines for the
** same program. The words of a program are listed at ref_object_run. */
namespace ObjOpWorld {
#include "shim_object/object_standins.h"
#include "shim_object/object_standins2.h"
#include "object_class.inc"
#include "geometry_cpp.inc"
#include "thing_cpp.inc"
#include "experience_cpp.inc"
#include "object_cpp.inc"
#include "shim_object/object_standins3.h"
#include "shim_object/object_driver.h"
}
