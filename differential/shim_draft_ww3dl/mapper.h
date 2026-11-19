/* Stand-in for WW3D2/mapper.h (#215, group ww3dlogic; used only by differential/wrap_draft_matrixmapper.cpp). The real header
** pulls in the texture classes and the device. matrixmapper.h/.cpp need only TextureMapperClass's Stage, the reference
** count, Clone, Apply and Calculate_Texture_Matrix, which are what is declared here, with upstream's signatures. */
#ifndef DRAFT_MAPPER_STANDIN_H
#define DRAFT_MAPPER_STANDIN_H

#include "always.h"
#include "matrix4.h"

#define NEW_REF(C, P) (new C P)

class TextureMapperClass
{
public:
	TextureMapperClass(unsigned int stage = 0) : Stage(stage), refs(1) { }
	TextureMapperClass(const TextureMapperClass & src) : Stage(src.Stage), refs(1) { }
	virtual ~TextureMapperClass(void) { }
	virtual TextureMapperClass * Clone(void) const = 0;
	virtual void Apply(int uv_array_index) = 0;
	virtual void Calculate_Texture_Matrix(Matrix4x4 & tex_matrix) { }
	void Add_Ref(void) const { refs++; }
	void Release_Ref(void) const { refs--; }
protected:
	int Stage;
	mutable int refs;
};

#endif
