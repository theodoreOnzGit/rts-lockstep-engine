/* Stand-ins for the HLOD's release of its particle emitters (GitHub #65, batch c;
** differential tests only; see build.rs and differential/wrap_hlod_release.cpp).
** Included by the generated w3d_gen/hlod_release_def.cpp, after part_emt_def.h
** (the particle classes and shim_w3d/part_prelude.h's RenderObjClass and
** SceneClass).
**
** TRANSCRIBED (stand-ins), because the real ones need the whole render-object
** hierarchy, the renderer and WWLib's multilist with its node pool:
**   - RefRenderObjListClass and RefRenderObjListIterator (robjlist.h's
**     RefMultiListClass<RenderObjClass>, multilist.h/.cpp): Add puts the object
**     at the head, once (Internal_Add's onlyonce), and adds a reference;
**     Add_Tail at the tail; Remove takes it out and releases the reference;
**     Release_Head takes the head out and releases it; the iterator walks from
**     the head, and an object added at the head during a walk is not reached
**     (a std::list keeps its iterators across push_front).
**   - SimpleSceneClass: its four lists (scene.h); the member functions are
**     scene.cpp's, VERBATIM (Add_Render_Object, Remove_Render_Object, Register,
**     Unregister, Post_Render_Processing: the generated hlod_release_def.cpp).
**     Its constructor and destructor are not upstream's (the driver empties
**     the lists itself).
**   - HLodClass: ModelNodeClass and ModelArrayClass are hlod.h's, VERBATIM
**     (w3d_gen/hlod_model_arrays.inc, included in the class body); the members
**     Remove_Sub_Object, Notify_Added and Notify_Removed read (Lod, LodCount,
**     CurLod, AdditionalModels) as hlod.h declares them; the member functions
**     Remove_Sub_Object, Notify_Added and Notify_Removed are hlod.cpp's,
**     VERBATIM. Update_Sub_Object_Bits and Update_Obj_Space_Bounding_Volumes
**     only count their calls (the Rust keeps neither: nothing reads the bounds
**     or the bits). The destructor is Free's release of the sub-objects
**     (Set_Container(NULL), Release_Ref; hlod.cpp), the LOD array deleted.
*/
#ifndef SHIM_HLOD_RELEASE_PRELUDE_H
#define SHIM_HLOD_RELEASE_PRELUDE_H

#include <list>
#include <algorithm>

class RefRenderObjListClass
{
public:
	std::list<RenderObjClass *> L;
	bool Contains(RenderObjClass *obj) const { return std::find(L.begin(), L.end(), obj) != L.end(); }
	bool Add(RenderObjClass *obj, bool onlyonce = true)
	{
		if (onlyonce && Contains(obj)) return false;
		L.push_front(obj);
		obj->Add_Ref();
		return true;
	}
	bool Add_Tail(RenderObjClass *obj, bool onlyonce = true)
	{
		if (onlyonce && Contains(obj)) return false;
		L.push_back(obj);
		obj->Add_Ref();
		return true;
	}
	bool Remove(RenderObjClass *obj)
	{
		std::list<RenderObjClass *>::iterator i = std::find(L.begin(), L.end(), obj);
		if (i == L.end()) return false;
		L.erase(i);
		obj->Release_Ref();
		return true;
	}
	bool Is_Empty(void) const { return L.empty(); }
	void Release_Head(void)
	{
		if (L.empty()) return;
		RenderObjClass *obj = L.front();
		L.pop_front();
		obj->Release_Ref();
	}
};

class RefRenderObjListIterator
{
public:
	RefRenderObjListIterator(RefRenderObjListClass *list) : List(list) { It = List->L.begin(); }
	void First(void) { It = List->L.begin(); }
	void First(RefRenderObjListClass *list) { List = list; It = List->L.begin(); }
	void Next(void) { ++It; }
	bool Is_Done(void) const { return It == List->L.end(); }
	RenderObjClass *Peek_Obj(void) { return *It; }
	RefRenderObjListClass *List;
	std::list<RenderObjClass *>::iterator It;
};

class SimpleSceneClass : public SceneClass
{
public:
	virtual void Add_Render_Object(RenderObjClass *obj);
	virtual void Remove_Render_Object(RenderObjClass *obj);
	virtual void Register(RenderObjClass *obj, RegType for_what);
	virtual void Unregister(RenderObjClass *obj, RegType for_what);
	virtual void Post_Render_Processing(RenderInfoClass &rinfo);

	RefRenderObjListClass RenderList;
	RefRenderObjListClass UpdateList;
	RefRenderObjListClass LightList;
	RefRenderObjListClass ReleaseList;
};

extern int g_hlod_bv_updates; /* wrap_hlod_release.cpp */

class HLodClass : public RenderObjClass
{
public:
#include "hlod_model_arrays.inc"

	HLodClass(void) : LodCount(0), CurLod(0), Lod(NULL) {}
	virtual ~HLodClass(void)
	{
		/* hlod.cpp HLodClass::Free: each sub-object's container cleared and its reference released */
		for (int lod = 0; lod < LodCount; lod++) {
			for (int model = 0; model < Lod[lod].Count(); model++) {
				RenderObjClass *robj = Lod[lod][model].Model;
				Lod[lod][model].Model = NULL;
				robj->Set_Container(NULL);
				robj->Release_Ref();
			}
			Lod[lod].Delete_All();
		}
		delete[] Lod;
		for (int model = 0; model < AdditionalModels.Count(); model++) {
			RenderObjClass *robj = AdditionalModels[model].Model;
			AdditionalModels[model].Model = NULL;
			robj->Set_Container(NULL);
			robj->Release_Ref();
		}
		AdditionalModels.Delete_All();
	}
	virtual void Notify_Added(SceneClass *scene);
	virtual void Notify_Removed(SceneClass *scene);
	virtual int Remove_Sub_Object(RenderObjClass *removeme);
	void Update_Sub_Object_Bits(void) {}
	void Update_Obj_Space_Bounding_Volumes(void) { g_hlod_bv_updates++; }

	int LodCount;
	int CurLod;
	ModelArrayClass *Lod;
	ModelArrayClass AdditionalModels;
};

#endif
