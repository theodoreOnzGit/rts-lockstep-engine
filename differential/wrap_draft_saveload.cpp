/* Differential-test wrapper for the drafts src/drafts/wwlib/{saveload, saveloadsubsystem, persistfactory, definition, definitionmgr, wwsaveload}.rs
** (#215, group wwlib4), upstream commit 0a05454d, WWSaveLoad/: saveload.cpp, saveloadsubsystem.cpp, persistfactory.cpp, definition.cpp,
** definitionmgr.cpp (and their headers, definitionclassids.h, saveloadids.h, persist.h, postloadable.h, pointerremap.h/.cpp), copied by
** build.rs (drafts_wwlib4) with these asserted edits: persistfactory.h's SimplePersistFactoryClass saved `(uint32)obj` (a pointer cast to 32
** bits, which only exists on the 32-bit Win32 target) and loaded `sizeof(T *)` bytes: the object's address is Test_Object_Id(obj), the
** wrapper's per-object number, and the old pointer is read as a uint32 (the same 4 bytes the Win32 build wrote); saveload.cpp's two
** `unsigned long` times are `unsigned int` (Win32's 32-bit unsigned long, so that `time2 - time > 20` wraps as it does there); definitionmgr.cpp's
** Find_Typed_Definition loop over a cached vector runs to `Length()` (its capacity) instead of `Count()`: it reads uninitialised slots whenever no cached entry
** matches (an upstream defect), so the reference walks `Count()` entries.
** The upstream .cpp files are compiled as files of their own (each has an enum CHUNKID_VARIABLES). Force-included: shim_w3d/w3d_prelude.h
** (32-bit uint32) and shim_draft_wwlib4/compat.h (TCHAR, _strlwr). ChunkSaveClass / ChunkLoadClass are the real chunkio.cpp (linked from upstream_w3d).
** Stand-ins (shim_draft_wwlib4): editable.h (the property-sheet base, no effect on the bytes), twiddler.h (Twiddle() returns the definition
** itself), saveloadstatus.h (display only), systimer.h (TIMEGETTIME scripted), wwhack.h, wwprofile.h, windows.h, wwdebug.h (WWASSERT out,
** WWDEBUG_SAY captured), wwstring.h (StringClass over std::string); MemFile (FileClass over a byte vector, as wrap_draft_chunksave.cpp).
** Test types: TDef<K> for K = 0..4, DefinitionClass subclasses with class ids {0x3005, 0x3006, 0x5001, 0x0800, 0xE000 (CLASSID_TWIDDLERS)} and
** persist-factory chunk ids {0x7001, 0x7001, 0x7002, 0x7003, 0x7004}; each has an intrinsic SimplePersistFactoryClass (what Get_Factory
** returns), registered for the whole program, and the program may register more instances of any type (slots) or destroy them.
**
** ref_sl_run(names, nnames, prog, nwords, out, cap): ops, each followed by its outputs (all numbers are words):
**   0 s k id n en   def slot s = new TDef<k>, Set_Name(names[n]), Set_ID(id), Enable_Save(en)      -> oid
**   1 s / 2 s       Register_Definition / Unregister_Definition                                      -> link
**   3 s id          Set_ID(id)                                                                       -> link
**   4 s             delete the definition (only if it was never registered and is not in the post-load list: the manager's name cache keeps a
**                   pointer to a definition that was registered, even after Unregister_Definition, so deleting it leaves one dangling)       -> 1/0
**   5 id tw         Find_Definition             6 n tw   Find_Named_Definition   7 n c tw   Find_Typed_Definition (classes[c])   -> oid or 0
**   8               Get_First / Get_Next chain                                                       -> count, (oid, id, class id, link)*
**   9 c type        Get_First(classes[c], type) / Get_Next(cur, classes[c], type) chain              -> count, oid*
**   10 c            Get_New_ID(classes[c])        11   List_Available_Definitions()       12 c  List_Available_Definitions(classes[c])   -> count, names
**   13              ChunkSaveClass over a MemFile, SaveLoadSystemClass::Save(csave, _TheDefinitionMgr)  -> result, size, bytes (kept as the image)
**   14 auto         SaveLoadSystemClass::Load(cload, auto) over the kept image (then dropped; skipped (-17) if an id in it is already in the manager: the
**                   loaded definitions are appended and qsorted, and the order of equal ids is the C library's); each loaded object asks for a remap of `ref` = its own
**                   id (as a pointer value)                  -> result, loaded count, then for every definition (oid, the oid `ref` now points to or 0)
**   15 via          Free_Definitions (via = 0) or WWSaveLoad::Shutdown() (1) (slots of registered definitions are cleared)
**   16 f k / 17 f   create / destroy the extra persist factory f (a TDef<k> one)
**   18 s            Register_Post_Load_Callback(def s)  19 s  Is_Post_Load_Callback_Registered   -> flag
**   20 cb m t*      Post_Load_Processing(cb ? callback : NULL) with the clock t* (m values)          -> result, callbacks, registered flags of all slots
**   21 / 22         unregister / register _TheDefinitionMgr as a sub-system (only if it is / is not)  -> Find_Sub_System(chunk) != NULL
**   23 chunk        SaveLoadSystemClass::Find_Persist_Factory(chunk): which factory is found -> its type K (0..4), +100*(1+f) if it is extra factory f, -1 if none
** At the end everything is released (extra factories, definitions, the intrinsic factories), so that the next program starts clean. */
#define NDEBUG
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <vector>
#include <string>
#include "always.h"
#include "bittype.h"
#include "chunkio.h"
#include "wwfile.h"

namespace {
class MemFile : public FileClass
{
public:
	std::vector<unsigned char> data;
	int pos;
	MemFile() : pos(0) {}
	virtual char const *File_Name(void) const { return "mem"; }
	virtual char const *Set_Name(char const *) { return "mem"; }
	virtual int Create(void) { return 1; }
	virtual int Delete(void) { return 1; }
	virtual bool Is_Available(int = false) { return true; }
	virtual bool Is_Open(void) const { return true; }
	virtual int Open(char const *, int = READ) { return 1; }
	virtual int Open(int = READ) { return 1; }
	virtual int Read(void *buffer, int size)
	{
		int avail = (int)data.size() - pos;
		if (avail < 0) avail = 0;
		int n = size < avail ? size : avail;
		if (n > 0) memcpy(buffer, &data[pos], n);
		pos += n;
		return n;
	}
	virtual int Seek(int p, int dir = SEEK_CUR)
	{
		switch (dir) {
		case SEEK_SET: pos = p; break;
		case SEEK_END: pos = (int)data.size() + p; break;
		default: pos += p; break;
		}
		return pos;
	}
	virtual int Size(void) { return (int)data.size(); }
	virtual int Write(void const *buffer, int size)
	{
		if (pos + size > (int)data.size()) data.resize(pos + size, 0);
		memcpy(&data[pos], buffer, size);
		pos += size;
		return size;
	}
	virtual void Close(void) {}
};
}


#include "persistfactory.h"
#include "saveloadsubsystem.h"
#include "saveload.h"
#include "definition.h"
#include "definitionmgr.h"
#include "wwsaveload.h"

namespace {
uint32 g_oid = 0;
std::vector<unsigned int> g_times;
size_t g_time_at = 0;
int g_callbacks = 0;

class TestDef : public DefinitionClass
{
public:
	uint32 oid;
	void *ref;      /* a pointer the object's Load asks to have remapped (set by TDef<K>::Load) */
	TestDef() : oid(++g_oid), ref(0) {}
	int Link() const { return m_DefinitionMgrLink; }
	virtual PersistClass *Create(void) const { return 0; }
};

const uint32 CLS[5] = {0x3005, 0x3006, 0x5001, 0x0800, 0xE000};
const uint32 CHK[5] = {0x7001, 0x7001, 0x7002, 0x7003, 0x7004};
PersistFactoryClass *g_intrinsic[5] = {0, 0, 0, 0, 0};

template <int K> class TDef : public TestDef
{
public:
	virtual uint32 Get_Class_ID(void) const { return CLS[K]; }
	virtual const PersistFactoryClass &Get_Factory(void) const { return *g_intrinsic[K]; }
	/* DefinitionClass::Load, then a pointer remap request on `ref`, which holds the object's own id as a pointer value (so that it is the
	   old pointer of a saved object whenever the id equals that object's number) */
	virtual bool Load(ChunkLoadClass &cload)
	{
		bool r = DefinitionClass::Load(cload);
		ref = (void *)(uintptr_t)Get_ID();
		SaveLoadSystemClass::Request_Pointer_Remap(&ref);
		return r;
	}
};
typedef SimplePersistFactoryClass<TDef<0>, 0x7001> F0;
typedef SimplePersistFactoryClass<TDef<1>, 0x7001> F1;
typedef SimplePersistFactoryClass<TDef<2>, 0x7002> F2;
typedef SimplePersistFactoryClass<TDef<3>, 0x7003> F3;
typedef SimplePersistFactoryClass<TDef<4>, 0x7004> F4;

PersistFactoryClass *new_factory(int k)
{
	switch (k) {
	case 0: return new F0;
	case 1: return new F1;
	case 2: return new F2;
	case 3: return new F3;
	default: return new F4;
	}
}
TDef<0> *new_def0() { return new TDef<0>; }
TestDef *new_def(int k)
{
	switch (k) {
	case 0: return new TDef<0>;
	case 1: return new TDef<1>;
	case 2: return new TDef<2>;
	case 3: return new TDef<3>;
	default: return new TDef<4>;
	}
}
int type_of_factory(PersistFactoryClass *f)
{
	if (f == 0) return -1;
	/* the type a factory makes: load nothing, ask its chunk and compare with what it creates is not possible without a stream; use RTTI */
	if (dynamic_cast<F0 *>(f)) return 0;
	if (dynamic_cast<F1 *>(f)) return 1;
	if (dynamic_cast<F2 *>(f)) return 2;
	if (dynamic_cast<F3 *>(f)) return 3;
	if (dynamic_cast<F4 *>(f)) return 4;
	return -2;
}

/* protected static members of SaveLoadSystemClass, reached through a derived class */
struct Hack : public SaveLoadSystemClass
{
	static void Reg(SaveLoadSubSystemClass *s) { Register_Sub_System(s); }
	static void Unreg(SaveLoadSubSystemClass *s) { Unregister_Sub_System(s); }
	static bool Find(uint32 id) { return Find_Sub_System(id) != 0; }
	static bool IsPL(PostLoadableClass *o) { return Is_Post_Load_Callback_Registered(o); }
};

void net_callback(void) { g_callbacks++; }
}

uint32 Test_Object_Id(PersistClass *obj) { return ((TestDef *)obj)->oid; }
unsigned int Test_Time(void)
{
	if (g_times.empty()) return 0;
	unsigned int v = g_times[g_time_at < g_times.size() ? g_time_at : g_times.size() - 1];
	g_time_at++;
	return v;
}
unsigned int _strlwr_unused;

extern "C" int ref_sl_run(const char *names, int nnames, const int *prog, int nwords, int *out, int cap)
{
	g_oid = 0;
	g_times.clear();
	g_time_at = 0;
	g_callbacks = 0;
	std::vector<const char *> nm;
	const char *q = names;
	for (int i = 0; i < nnames; i++) { nm.push_back(q); q += strlen(q) + 1; }
	int n = 0;
	auto put = [&](int w) { if (n < cap) out[n] = w; n++; };
	for (int k = 0; k < 5; k++) g_intrinsic[k] = new_factory(k);
	TestDef *defs[8] = {0, 0, 0, 0, 0, 0, 0, 0};
	bool ever[8] = {false, false, false, false, false, false, false, false};   /* Register_Definition was called on the slot's definition */
	PersistFactoryClass *extra[6] = {0, 0, 0, 0, 0, 0};
	int extra_type[6] = {0, 0, 0, 0, 0, 0};
	std::vector<unsigned char> image;
	std::vector<uint32> image_ids;   /* ids of the save-enabled definitions in the manager when the image was saved */
	bool sub_reg = true;
	auto oid_of = [&](DefinitionClass *d) { return d ? (int)((TestDef *)d)->oid : 0; };
	auto clear_log = [&]() { wwdebug_log().clear(); };
	auto put_log_names = [&]() {
		std::vector<std::string> v;
		const std::string &l = wwdebug_log();
		size_t at = 0;
		while ((at = l.find("  >", at)) != std::string::npos) {
			size_t e = l.find("<\n", at + 3);
			if (e == std::string::npos) break;
			v.push_back(l.substr(at + 3, e - at - 3));
			at = e + 2;
		}
		put((int)v.size());
		for (size_t i = 0; i < v.size(); i++) { put((int)v[i].size()); for (size_t j = 0; j < v[i].size(); j++) put((unsigned char)v[i][j]); }
	};
	const int *p = prog, *end = prog + nwords;
	while (p < end) {
		int op = *p++;
		switch (op) {
		case 0: { int s = *p++, k = *p++, id = *p++, a = *p++, en = *p++;
			if (defs[s]) { put(-5); break; }
			TestDef *d = new_def(k);
			d->Set_Name(nm[a]); d->Set_ID((uint32)id); d->Enable_Save(en != 0);
			defs[s] = d; ever[s] = false; put((int)d->oid); break; }
		case 1: { int s = *p++; if (!defs[s]) { put(-6); break; } ever[s] = true; DefinitionMgrClass::Register_Definition(defs[s]); put(defs[s]->Link()); break; }
		case 2: { int s = *p++; if (!defs[s]) { put(-6); break; } DefinitionMgrClass::Unregister_Definition(defs[s]); put(defs[s]->Link()); break; }
		case 3: { int s = *p++, id = *p++; if (!defs[s]) { put(-6); break; } defs[s]->Set_ID((uint32)id); put(defs[s]->Link()); break; }
		case 4: { int s = *p++; if (!defs[s]) { put(-6); break; }
			if (ever[s] || defs[s]->Link() != -1 || defs[s]->Is_Post_Load_Registered()) { put(0); break; }
			delete defs[s]; defs[s] = 0; put(1); break; }
		case 5: { int id = *p++, tw = *p++; put(oid_of(DefinitionMgrClass::Find_Definition((uint32)id, tw != 0))); break; }
		case 6: { int a = *p++, tw = *p++; put(oid_of(DefinitionMgrClass::Find_Named_Definition(nm[a], tw != 0))); break; }
		case 7: { int a = *p++, c = *p++, tw = *p++; put(oid_of(DefinitionMgrClass::Find_Typed_Definition(nm[a], (uint32)c, tw != 0))); break; }
		case 8: { std::vector<TestDef *> v;
			for (DefinitionClass *d = DefinitionMgrClass::Get_First(); d; d = DefinitionMgrClass::Get_Next(d)) v.push_back((TestDef *)d);
			put((int)v.size());
			for (size_t i = 0; i < v.size(); i++) { put((int)v[i]->oid); put((int)v[i]->Get_ID()); put((int)v[i]->Get_Class_ID()); put(v[i]->Link()); }
			break; }
		case 9: { uint32 c = (uint32)*p++; int t = *p++; DefinitionMgrClass::ID_TYPE ty = t == 1 ? DefinitionMgrClass::ID_CLASS : DefinitionMgrClass::ID_SUPERCLASS;
			std::vector<TestDef *> v;
			for (DefinitionClass *d = DefinitionMgrClass::Get_First(c, ty); d; d = DefinitionMgrClass::Get_Next(d, c, ty)) v.push_back((TestDef *)d);
			put((int)v.size());
			for (size_t i = 0; i < v.size(); i++) put((int)v[i]->oid);
			break; }
		case 10: { uint32 c = (uint32)*p++; put((int)DefinitionMgrClass::Get_New_ID(c)); break; }
		case 11: clear_log(); DefinitionMgrClass::List_Available_Definitions(); put_log_names(); break;
		case 12: { int c = *p++; clear_log(); DefinitionMgrClass::List_Available_Definitions(c); put_log_names(); break; }
		case 13: { MemFile mf; ChunkSaveClass cs(&mf);
			bool ok = SaveLoadSystemClass::Save(cs, _TheDefinitionMgr);
			image = mf.data; image_ids.clear();
			for (DefinitionClass *d = DefinitionMgrClass::Get_First(); d; d = DefinitionMgrClass::Get_Next(d)) if (d->Is_Save_Enabled()) image_ids.push_back(d->Get_ID());
			put(ok ? 1 : 0); put((int)image.size()); for (size_t i = 0; i < image.size(); i++) put(image[i]); break; }
		case 14: { int autop = *p++;
			bool clash = false;
			for (DefinitionClass *d = DefinitionMgrClass::Get_First(); d; d = DefinitionMgrClass::Get_Next(d)) for (size_t i = 0; i < image_ids.size(); i++) if (image_ids[i] == d->Get_ID()) clash = true;
			if (clash) { image.clear(); put(-17); break; }
			MemFile mf; mf.data = image; image.clear();
			ChunkLoadClass cl(&mf);
			int before = 0; for (DefinitionClass *d = DefinitionMgrClass::Get_First(); d; d = DefinitionMgrClass::Get_Next(d)) before++;
			bool ok = SaveLoadSystemClass::Load(cl, autop != 0);
			int after = 0; for (DefinitionClass *d = DefinitionMgrClass::Get_First(); d; d = DefinitionMgrClass::Get_Next(d)) after++;
			put(ok ? 1 : 0); put(after - before);
			for (DefinitionClass *d = DefinitionMgrClass::Get_First(); d; d = DefinitionMgrClass::Get_Next(d)) {
				TestDef *t = (TestDef *)d;
				put((int)t->oid); put(t->ref ? (int)((TestDef *)(DefinitionClass *)t->ref)->oid : 0);
			}
			break; }
		case 15: { int via = *p++; bool pl = false; for (int i = 0; i < 8; i++) if (defs[i] && defs[i]->Link() != -1 && defs[i]->Is_Post_Load_Registered()) pl = true;
			if (pl) { put(-16); break; }
			bool reg[8]; for (int i = 0; i < 8; i++) reg[i] = defs[i] && defs[i]->Link() != -1;
			if (via) WWSaveLoad::Shutdown(); else DefinitionMgrClass::Free_Definitions();
			for (int i = 0; i < 8; i++) { if (reg[i]) defs[i] = 0; }
			put(-15); break; }
		case 16: { int f = *p++, k = *p++; if (extra[f]) { put(-5); break; } extra[f] = new_factory(k); extra_type[f] = k; put(1); break; }
		case 17: { int f = *p++; if (!extra[f]) { put(-6); break; } delete extra[f]; extra[f] = 0; put(1); break; }
		case 18: { int s = *p++; if (!defs[s]) { put(-6); break; } SaveLoadSystemClass::Register_Post_Load_Callback(defs[s]); put(defs[s]->Is_Post_Load_Registered() ? 1 : 0); break; }
		case 19: { int s = *p++; if (!defs[s]) { put(-6); break; } put(Hack::IsPL(defs[s]) ? 1 : 0); break; }
		case 20: { int cb = *p++, m = *p++; g_times.clear(); for (int i = 0; i < m; i++) g_times.push_back((unsigned int)*p++); g_time_at = 0; g_callbacks = 0;
			bool ok = SaveLoadSystemClass::Post_Load_Processing(cb ? net_callback : 0);
			put(ok ? 1 : 0); put(g_callbacks); for (int i = 0; i < 8; i++) put(defs[i] ? (defs[i]->Is_Post_Load_Registered() ? 1 : 0) : -1);
			g_times.clear(); break; }
		case 21: if (sub_reg) { Hack::Unreg(&_TheDefinitionMgr); sub_reg = false; } put(Hack::Find(CHUNKID_SAVELOAD_DEFMGR) ? 1 : 0); break;
		case 22: if (!sub_reg) { Hack::Reg(&_TheDefinitionMgr); sub_reg = true; } put(Hack::Find(CHUNKID_SAVELOAD_DEFMGR) ? 1 : 0); break;
		case 23: { uint32 chunk = (uint32)*p++; PersistFactoryClass *f = SaveLoadSystemClass::Find_Persist_Factory(chunk);
			int t = type_of_factory(f); if (f) { for (int i = 0; i < 6; i++) if (extra[i] == f) t += 100 * (1 + i); } put(t); break; }
		}
	}
	/* release everything */
	if (!sub_reg) Hack::Reg(&_TheDefinitionMgr);
	SaveLoadSystemClass::Post_Load_Processing(0);
	for (int i = 0; i < 6; i++) { delete extra[i]; extra[i] = 0; }
	{ bool reg[8]; for (int i = 0; i < 8; i++) reg[i] = defs[i] && defs[i]->Link() != -1;
	DefinitionMgrClass::Free_Definitions();
	for (int i = 0; i < 8; i++) { if (defs[i] && !reg[i]) delete defs[i]; defs[i] = 0; } }
	for (int k = 4; k >= 0; k--) { delete g_intrinsic[k]; g_intrinsic[k] = 0; }
	return n;
}
