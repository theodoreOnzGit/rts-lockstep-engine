/* Forced include (-include, after w3d_prelude.h) for the particle-emitter,
** hierarchical-model and distance-LOD definition loaders, differential tests
** only (GitHub #17; see build.rs and differential/wrap_w3d_emit.cpp).
**
** The library is compiled with WW3DAssetManager renamed EmitAssetShim and
** TextureClass renamed EmitTextureShim (build.rs), so the stand-ins below
** and in wrap_w3d_emit.cpp do not clash with the other libraries' ones.
**
**   texture.h (guard TEXTURE_H): ParticleEmitterDefClass::Convert_To_Ver2
**     asks the asset manager for the emitter's texture and tests
**     Has_Alpha(Get_Texture_Format()). The real TextureClass needs the
**     renderer; the stand-in carries only the format the test chose.
**   WWLib/Vector.H: the generated vector_this.h (see build.rs) first, as in
**     mesh_prelude.h, for SnapPointsClass's DynamicVectorClass. */
#ifndef SHIM_EMIT_PRELUDE_H
#define SHIM_EMIT_PRELUDE_H

#include "always.h"
#include "vector_this.h"
#include "ww3dformat.h"

#define TEXTURE_H
class TextureClass
{
public:
	WW3DFormat Format;
	WW3DFormat Get_Texture_Format(void) const { return Format; }
	void Release_Ref(void) {}
};

#endif
