/* Differential-test wrapper for the terrain (#11, upstream commit 0a05454d):
**   GameEngineDevice/Source/W3DDevice/GameClient/WorldHeightMap.cpp: readTexClass, countTiles, readTiles,
**     updateTileTexturePositions, getUVForNdx, getUVForBlend, isCliffMappedTexture, getUVData, getUVForTileIndex,
**     getExtraAlphaUVData, getAlphaUVData, setTextureLOD, getTerrainTexture;
**   TerrainTex.cpp: TerrainTextureClass::update (the compiled one);
**   HeightMap.cpp: getXWithOrigin, getYWithOrigin, updateVB.
** Those functions are VERBATIM segments cut by build.rs (OUT_DIR/terrain_mesh_gen, see the block there), with the
** type definitions of TileData.h and WorldHeightMap.h. Transcribed here, and nothing else:
** - the base types of Lib/BaseType.h the segments use, MAP_XY_FACTOR/MAP_HEIGHT_SCALE (MapObject.h), TILE_OFFSET
**   (TerrainTex.h), VERTEX_BUFFER_TILE_LENGTH and FLIP_TRIANGLES (HeightMap.h);
** - WorldHeightMap's and HeightMapRenderObjClass's members and inline accessors the segments use (as declared in
**   WorldHeightMap.h, BaseHeightMap.h, HeightMap.h), and TileData's (TileData.h; updateMips does nothing: the mips are
**   not compared);
** - stand-ins: AsciiString (a std::string), TheTerrainTypes (terrain "C<k>"/"E<k>" with texture "c<k>.tga"/"e<k>.tga"
**   for class k), TheFileSystem/File/GDIFileStream (in-memory files), TheGlobalData (the four fields read), the D3D
**   texture and surface TerrainTextureClass::update locks (a zeroed A1R5G5B5 buffer), AlphaTerrainTextureClass and
**   AlphaEdgeTextureClass (whose update returns its height), the vertex buffer, and doTheLight, which records the
**   normal and the alpha it is given instead of lighting the vertex;
** - the driver: WorldHeightMap's members as the constructor and ParseBlendTileData leave them (the Rust side's
**   TerrainHeightMap after from_logic), readTexClass for each class, getTerrainTexture, then
**   HeightMapRenderObjClass::initHeightData's sizes and updateBlock's loop over the 32x32 vertex buffer tiles, with the
**   whole map as the draw window (m_drawEntireTerrain).
** Uninitialised upstream memory (new TileData, a new D3D texture) is zeroed here. */
#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#include <math.h>
#include <string>
#include <map>
/* <new> already declares the placement forms always.h would define again */
#define _OPERATOR_NEW_DEFINED_
#include "wwmath.h"
#include "vector3.h"

typedef int Int;
typedef float Real;
typedef bool Bool;
typedef short Short;
typedef unsigned short UnsignedShort;
typedef unsigned char UnsignedByte;
typedef unsigned int UnsignedInt;
struct ICoord2D { Int x, y; };
struct Coord2D { Real x, y; };
struct Coord3D { Real x, y, z; };
struct Region2D { Coord2D lo, hi; };
#ifndef TRUE
#define TRUE true
#define FALSE false
#endif
#define MAP_XY_FACTOR			(10.0f)
#define MAP_HEIGHT_SCALE	(MAP_XY_FACTOR/16.0f)
#define DEBUG_ASSERTCRASH(c, m)
#define DEBUG_LOG(m)
#define MSGNEW(x) new
#define _MAX_PATH 260
#define TERRAIN_TGA_DIR_PATH "Art/Terrain/"
#define PATHFIND_CELL_SIZE_F (MAP_XY_FACTOR)
#define MAX_GLOBAL_LIGHTS 3
#define VERTEX_BUFFER_TILE_LENGTH 32
#define TILE_OFFSET 8 /* TerrainTex.h */
#define FLIP_TRIANGLES 1 /* HeightMap.h, which WorldHeightMap.cpp and HeightMap.cpp include */

class AsciiString {
	std::string s;
public:
	AsciiString() {}
	AsciiString(const char *c) : s(c) {}
	const char *str() const { return s.c_str(); }
	bool operator<(const AsciiString &o) const { return s < o.s; }
};

#include "tile_defs.inc"

class TileData {
public:
	UnsignedByte m_tileData[DATA_LEN_BYTES];
	ICoord2D m_tileLocationInTexture;
	TileData() { memset(m_tileData, 0, sizeof(m_tileData)); m_tileLocationInTexture.x = m_tileLocationInTexture.y = 0; }
	UnsignedByte *getDataPtr(void) { return m_tileData; }
	void updateMips(void) {}
	Bool hasRGBDataForWidth(Int width) { return width == TILE_PIXEL_EXTENT; }
	UnsignedByte *getRGBDataForWidth(Int width) { return m_tileData; }
};

#include "whm_defs.inc"

/* ---- stand-ins ---- */
struct GlobalDataShim {
	Bool m_adjustCliffTextures;
	Int m_use3WayTerrainBlends;
	Int m_numGlobalLights;
	Coord3D m_terrainLightPos[MAX_GLOBAL_LIGHTS];
	Int m_textureReductionFactor;
};
static GlobalDataShim s_globalData;
static GlobalDataShim *TheGlobalData = &s_globalData;
static GlobalDataShim *TheWritableGlobalData = &s_globalData;

class TerrainType {
public:
	AsciiString m_texture;
	AsciiString getTexture(void) const { return m_texture; }
};
struct TerrainTypesShim {
	std::map<std::string, TerrainType> terrains;
	TerrainType *findTerrain(AsciiString name) {
		std::map<std::string, TerrainType>::iterator it = terrains.find(name.str());
		return it == terrains.end() ? NULL : &it->second;
	}
};
static TerrainTypesShim s_terrainTypes;
static TerrainTypesShim *TheTerrainTypes = &s_terrainTypes;

class File {
public:
	enum { READ = 1, BINARY = 2 };
	enum seekMode { START = 0, CURRENT, END };
	const std::string *bytes;
	Int pos;
	Int read(void *dst, Int n) {
		Int left = (Int)bytes->size() - pos;
		if (n > left) n = left;
		if (n < 0) n = 0;
		memcpy(dst, bytes->data() + pos, n);
		pos += n;
		return n;
	}
	Int seek(Int p, seekMode) { pos = p; return p; }
	void close(void) { delete this; }
};
struct FileSystemShim {
	std::map<std::string, std::string> files;
	File *openFile(const char *name, Int) {
		std::map<std::string, std::string>::iterator it = files.find(name);
		if (it == files.end()) return NULL;
		File *f = new File;
		f->bytes = &it->second;
		f->pos = 0;
		return f;
	}
};
static FileSystemShim s_fileSystem;
static FileSystemShim *TheFileSystem = &s_fileSystem;

class InputStream {
public:
	virtual Int read(void *pData, Int numBytes) = 0;
};
class GDIFileStream : public InputStream {
	File *m_file;
public:
	GDIFileStream(File *f) : m_file(f) {}
	virtual Int read(void *pData, Int numBytes) { return m_file->read(pData, numBytes); }
};

/* D3D 8 as TerrainTextureClass::update uses it */
#define D3DFMT_A1R5G5B5 25
#define D3DX_FILTER_BOX 0
#define DX8_ErrorCode(x) (x)
struct D3DSURFACE_DESC { UnsignedInt Width, Height; Int Format; };
struct D3DLOCKED_RECT { Int Pitch; void *pBits; };
struct IDirect3DSurface8 {
	UnsignedShort *px; UnsignedInt w, h;
	long GetDesc(D3DSURFACE_DESC *d) { d->Width = w; d->Height = h; d->Format = D3DFMT_A1R5G5B5; return 0; }
	long LockRect(D3DLOCKED_RECT *r, void *, Int) { r->pBits = px; r->Pitch = w * 2; return 0; }
	long UnlockRect(void) { return 0; }
	long Release(void) { return 0; }
};
struct IDirect3DTexture8 {
	IDirect3DSurface8 surf;
	long GetSurfaceLevel(Int, IDirect3DSurface8 **s) { *s = &surf; return 0; }
	void SetLOD(Int) {}
};
static long D3DXFilterTexture(IDirect3DTexture8 *, void *, Int, Int) { return 0; }
class TextureClass {
public:
	IDirect3DTexture8 tex;
	TextureClass() { tex.surf.px = NULL; tex.surf.w = tex.surf.h = 0; }
	virtual ~TextureClass() { free(tex.surf.px); }
	IDirect3DTexture8 *Peek_D3D_Texture(void) { return &tex; }
};
class WorldHeightMap;
class TerrainTextureClass : public TextureClass {
public:
	TerrainTextureClass(int height) { tex.surf.w = TEXTURE_WIDTH; tex.surf.h = height; tex.surf.px = (UnsignedShort *)calloc((size_t)TEXTURE_WIDTH * height, 2); }
	int update(WorldHeightMap *htMap);
	void setLOD(Int) {}
};
class AlphaTerrainTextureClass : public TextureClass {
public:
	AlphaTerrainTextureClass(TextureClass *) {}
};
class AlphaEdgeTextureClass : public TextureClass {
public:
	int m_h;
	AlphaEdgeTextureClass(int height) : m_h(height) {}
	int update(WorldHeightMap *) { return m_h; }
};
#define REF_PTR_RELEASE(x) { delete (x); (x) = NULL; }
#define REF_PTR_SET(dst, src) { (dst) = (src); }

/* ---- WorldHeightMap's members (WorldHeightMap.h) ---- */
class WorldHeightMap {
public:
	Int m_width, m_height, m_borderSize, m_dataSize;
	UnsignedByte *m_data;
	UnsignedByte *m_cellFlipState;
	Int m_flipStateWidth;
	UnsignedByte *m_cellCliffState;
	Short *m_tileNdxes, *m_blendTileNdxes, *m_cliffInfoNdxes, *m_extraBlendTileNdxes;
	Int m_numBitmapTiles, m_numEdgeTiles, m_numBlendedTiles;
	TileData *m_sourceTiles[NUM_SOURCE_TILES];
	TileData *m_edgeTiles[NUM_SOURCE_TILES];
	TBlendTileInfo m_blendedTiles[NUM_BLEND_TILES];
	TCliffInfo m_cliffInfo[NUM_CLIFF_INFO];
	Int m_numCliffInfo;
	int m_numTextureClasses;
	TXTextureClass m_textureClasses[NUM_TEXTURE_CLASSES];
	int m_numEdgeTextureClasses;
	TXTextureClass m_edgeTextureClasses[NUM_TEXTURE_CLASSES];
	TerrainTextureClass *m_terrainTex;
	Int m_terrainTexHeight;
	AlphaTerrainTextureClass *m_alphaTerrainTex;
	Int m_alphaTexHeight;
	AlphaEdgeTextureClass *m_alphaEdgeTex;
	Int m_alphaEdgeHeight;
	Int m_drawOriginX, m_drawOriginY, m_drawWidthX, m_drawHeightY;

	TileData *getSourceTile(UnsignedInt ndx) { if (ndx<NUM_SOURCE_TILES) return(m_sourceTiles[ndx]); return(NULL); };
	void getUVForNdx(Int ndx, float *minU, float *minV, float *maxU, float*maxV, Bool fullTile);
	Bool getUVForTileIndex(Int ndx, Short tileNdx, float U[4], float V[4], Bool fullTile);
	void readTexClass(TXTextureClass *texClass, TileData **tileData);
	Int updateTileTexturePositions(Int *edgeHeight);
	static Bool readTiles(InputStream *pStrm, TileData **tiles, Int numRows);
	static Int countTiles(InputStream *pStrm, Bool *halfTile=NULL);
	void getUVForBlend(Int edgeClass, Region2D *range);
	Bool isCliffMappedTexture(Int x, Int y);
	Bool getUVData(Int xIndex, Int yIndex, float U[4], float V[4], Bool fullTile);
	Bool getExtraAlphaUVData(Int xIndex, Int yIndex, float U[4], float V[4], UnsignedByte alpha[4], Bool *flip, Bool *cliff);
	void getAlphaUVData(Int xIndex, Int yIndex, float U[4], float V[4], UnsignedByte alpha[4], Bool *flip, Bool fullTile);
	void setTextureLOD(Int lod);
	TextureClass *getTerrainTexture(void);
	Int getXExtent(void) {return m_width;}
	Int getYExtent(void) {return m_height;}
	inline Int getDrawOrgX(void) {return m_drawOriginX;}
	inline Int getDrawOrgY(void) {return m_drawOriginY;}
	inline Int getBorderSizeInline(void) const { return m_borderSize; }
	inline UnsignedByte getDisplayHeight(Int x, Int y) { return m_data[x+m_drawOriginX+m_width*(y+m_drawOriginY)];}
	inline UnsignedByte getHeight(Int xIndex, Int yIndex)
	{
		Int ndx = (yIndex*m_width)+xIndex;
		if ((ndx>=0) && (ndx<m_dataSize) && m_data)
			return(m_data[ndx]);
		else
			return(0);
	};
	Bool getCliffState(Int, Int) const { return false; } /* read only with m_showImpassableAreas, which is false */
};

#include "whm_read_tex_class.inc"
#include "whm_tiles_uv.inc"
#include "terrain_tex_update.inc"

/* ---- HeightMapRenderObjClass's members (BaseHeightMap.h, HeightMap.h) ---- */
struct VertexShim { float x, y, z; UnsignedInt diffuse; float u1, v1, u2, v2; float nx, ny, nz; UnsignedByte alpha; };
#define VERTEX_FORMAT VertexShim
class DX8VertexBufferClass {
public:
	VertexShim *hw;
	class WriteLockClass {
		VertexShim *p;
	public:
		WriteLockClass(DX8VertexBufferClass *vb) : p(vb->hw) {}
		VertexShim *Get_Vertex_Array(void) { return p; }
	};
};
class RefRenderObjListIterator;
const Bool HALF_RES_MESH = false;
#define ADJUST_FROM_INDEX_TO_REAL(k) ((k-m_map->getBorderSizeInline())*MAP_XY_FACTOR)
class HeightMapRenderObjClass {
public:
	Int m_x, m_y, m_originX, m_originY;
	WorldHeightMap *m_map;
	DX8VertexBufferClass **m_vertexBufferTiles;
	Bool m_showImpassableAreas;
	Int getXWithOrigin(Int x);
	Int getYWithOrigin(Int y);
	Int updateVB(DX8VertexBufferClass *pVB, char *data, Int x0, Int y0, Int x1, Int y1, Int originX, Int originY, WorldHeightMap *pMap, RefRenderObjListIterator *pLightsIterator);
	void doTheLight(VERTEX_FORMAT *vb, Vector3*light, Vector3*normal, RefRenderObjListIterator *pLightsIterator, UnsignedByte alpha)
	{
		vb->nx = normal->X; vb->ny = normal->Y; vb->nz = normal->Z; vb->alpha = alpha; vb->diffuse = (UnsignedInt)alpha << 24;
	}
	Bool showAsVisibleCliff(Int, Int) { return false; }
};

#include "height_map_vb.inc"

/* ---- the driver ---- */
static void put_f(float *out, int k, float v) { out[k] = v; }

/* ints: [0] width, [1] height, [2] border, [3] numBitmapTiles, [4] numBlendedTiles, [5] numCliffInfo,
**   [6] numTextureClasses, [7] numEdgeTextureClasses, [8] adjustCliffTextures, [9] numEdgeTiles.
** blends: 8 ints per blended tile (entry 0 included): blendNdx, horiz, vert, rightDiagonal, leftDiagonal, inverted,
**   longDiagonal, customBlendEdgeClass. cliffs: 8 floats (u0 v0 u1 v1 u2 v2 u3 v3), cliffInts: flip, mutant, tileIndex.
** classes: firstTile, numTiles, width per texture class then per edge class. tga/tgaLens: a file per class (texture
**   classes, then edge classes): length >= 0 the file, -1 no such terrain, -2 the terrain but no file.
** Outputs, per cell (x, y) of the (w-1)*(h-1) cells, row by row:
**   uv: 24 floats: getUVData's U[4] V[4], getAlphaUVData's U[4] V[4], getExtraAlphaUVData's U[4] V[4];
**   cell: 13 ints: getUVData's result, getAlphaUVData's alpha[4] and flip, getExtraAlphaUVData's result, alpha[4],
**     flip, cliff;
**   vtx: 4 vertices of 10 floats (x y z nx ny nz u1 v1 u2 v2) as updateVB left them, and vtxAlpha their alphas.
** info: [0] m_terrainTexHeight, [1] m_alphaEdgeHeight, [2] the cell flip state bits set, then per texture class and
**   edge class positionInTexture x, y, then per source tile and edge tile (NUM_SOURCE_TILES each) present, x, y.
** atlas: TEXTURE_WIDTH * m_terrainTexHeight words if they fit in atlasCap. Returns 0, or -1 when the atlas did not fit. */
extern "C" int ref_terrain_mesh(const int *ints, const unsigned char *data, const short *tile, const short *blend,
	const short *extra, const short *cliffNdx, const int *blends, const float *cliffs, const int *cliffInts,
	const int *classes, const unsigned char *tga, const int *tgaLens, float *uv, int *cell, float *vtx, int *vtxAlpha,
	int *info, unsigned short *atlas, int atlasCap)
{
	s_globalData.m_adjustCliffTextures = ints[8] != 0;
	s_globalData.m_use3WayTerrainBlends = 1;
	s_globalData.m_numGlobalLights = 0;
	s_globalData.m_textureReductionFactor = 0;
	WorldHeightMap *m = new WorldHeightMap(); /* value-initialised: the members zero, the names empty */
	Int w = ints[0], h = ints[1], n = w * h;
	m->m_width = w; m->m_height = h; m->m_borderSize = ints[2]; m->m_dataSize = n;
	m->m_data = new UnsignedByte[n]; memcpy(m->m_data, data, n);
	m->m_tileNdxes = new Short[n]; memcpy(m->m_tileNdxes, tile, n * 2);
	m->m_blendTileNdxes = new Short[n]; memcpy(m->m_blendTileNdxes, blend, n * 2);
	m->m_extraBlendTileNdxes = new Short[n]; memcpy(m->m_extraBlendTileNdxes, extra, n * 2);
	m->m_cliffInfoNdxes = new Short[n]; memcpy(m->m_cliffInfoNdxes, cliffNdx, n * 2);
	m->m_flipStateWidth = (w + 7) / 8;
	m->m_cellFlipState = new UnsignedByte[m->m_flipStateWidth * h]; memset(m->m_cellFlipState, 0, m->m_flipStateWidth * h);
	m->m_numBitmapTiles = ints[3]; m->m_numBlendedTiles = ints[4]; m->m_numCliffInfo = ints[5];
	m->m_numTextureClasses = ints[6]; m->m_numEdgeTextureClasses = ints[7]; m->m_numEdgeTiles = ints[9];
	m->m_terrainTexHeight = 1; m->m_alphaTexHeight = 1; m->m_alphaEdgeHeight = 1;
	m->m_drawWidthX = w; m->m_drawHeightY = h;
	for (Int i = 0; i < m->m_numBlendedTiles; i++) {
		const int *b = blends + 8 * i;
		TBlendTileInfo &t = m->m_blendedTiles[i];
		t.blendNdx = b[0]; t.horiz = b[1]; t.vert = b[2]; t.rightDiagonal = b[3]; t.leftDiagonal = b[4];
		t.inverted = b[5]; t.longDiagonal = b[6]; t.customBlendEdgeClass = b[7];
	}
	for (Int i = 0; i < m->m_numCliffInfo; i++) {
		TCliffInfo &c = m->m_cliffInfo[i];
		const float *f = cliffs + 8 * i;
		c.u0 = f[0]; c.v0 = f[1]; c.u1 = f[2]; c.v1 = f[3]; c.u2 = f[4]; c.v2 = f[5]; c.u3 = f[6]; c.v3 = f[7];
		c.flip = cliffInts[3 * i] != 0; c.mutant = cliffInts[3 * i + 1] != 0; c.tileIndex = (Short)cliffInts[3 * i + 2];
	}
	s_terrainTypes.terrains.clear();
	s_fileSystem.files.clear();
	Int nc = m->m_numTextureClasses + m->m_numEdgeTextureClasses;
	Int off = 0;
	for (Int k = 0; k < nc; k++) {
		bool edge = k >= m->m_numTextureClasses;
		Int kk = edge ? k - m->m_numTextureClasses : k;
		TXTextureClass &c = edge ? m->m_edgeTextureClasses[kk] : m->m_textureClasses[kk];
		c.globalTextureClass = -1;
		c.firstTile = classes[3 * k]; c.numTiles = classes[3 * k + 1]; c.width = classes[3 * k + 2];
		char name[32], texture[32], path[64];
		sprintf(name, "%s%d", edge ? "E" : "C", kk);
		sprintf(texture, "%s%d.tga", edge ? "e" : "c", kk);
		c.name = AsciiString(name);
		if (tgaLens[k] != -1) {
			TerrainType t; t.m_texture = AsciiString(texture);
			s_terrainTypes.terrains[name] = t;
		}
		if (tgaLens[k] >= 0) {
			sprintf(path, "%s%s", TERRAIN_TGA_DIR_PATH, texture);
			s_fileSystem.files[path] = std::string((const char *)tga + off, tgaLens[k]);
			off += tgaLens[k];
		}
	}
	/* ParseBlendTileData's order: the texture classes, then the edge classes */
	for (Int k = 0; k < m->m_numTextureClasses; k++) m->readTexClass(&m->m_textureClasses[k], m->m_sourceTiles);
	for (Int k = 0; k < m->m_numEdgeTextureClasses; k++) m->readTexClass(&m->m_edgeTextureClasses[k], m->m_edgeTiles);
	m->getTerrainTexture();
	int ret = 0;
	Int ah = m->m_terrainTexHeight;
	if ((long)TEXTURE_WIDTH * ah <= atlasCap) {
		memcpy(atlas, m->m_terrainTex->tex.surf.px, (size_t)TEXTURE_WIDTH * ah * 2);
	} else {
		ret = -1;
	}
	info[0] = m->m_terrainTexHeight;
	info[1] = m->m_alphaEdgeHeight;
	Int bits = 0;
	for (Int i = 0; i < m->m_flipStateWidth * h; i++) for (Int b = 0; b < 8; b++) bits += (m->m_cellFlipState[i] >> b) & 1;
	info[2] = bits;
	Int p = 3;
	for (Int k = 0; k < nc; k++) {
		TXTextureClass &c = k < m->m_numTextureClasses ? m->m_textureClasses[k] : m->m_edgeTextureClasses[k - m->m_numTextureClasses];
		info[p++] = c.positionInTexture.x; info[p++] = c.positionInTexture.y;
	}
	for (Int e = 0; e < 2; e++) {
		for (Int i = 0; i < NUM_SOURCE_TILES; i++) {
			TileData *t = e ? m->m_edgeTiles[i] : m->m_sourceTiles[i];
			info[p++] = t != NULL;
			info[p++] = t ? t->m_tileLocationInTexture.x : 0;
			info[p++] = t ? t->m_tileLocationInTexture.y : 0;
		}
	}
	/* the per-cell lookups */
	Int nCells = 0;
	for (Int y = 0; y < h - 1; y++) {
		for (Int x = 0; x < w - 1; x++) {
			float *o = uv + 24 * nCells;
			int *c = cell + 13 * nCells;
			float U[4] = {0, 0, 0, 0}, V[4] = {0, 0, 0, 0}, UA[4] = {0, 0, 0, 0}, VA[4] = {0, 0, 0, 0};
			float UE[4] = {0, 0, 0, 0}, VE[4] = {0, 0, 0, 0};
			UnsignedByte alpha[4] = {0, 0, 0, 0}, alphaE[4] = {0, 0, 0, 0};
			Bool flip = false, flipE = false, cliffE = false;
			c[0] = m->getUVData(x, y, U, V, false);
			m->getAlphaUVData(x, y, UA, VA, alpha, &flip, false);
			c[6] = m->getExtraAlphaUVData(x, y, UE, VE, alphaE, &flipE, &cliffE);
			for (Int k = 0; k < 4; k++) {
				put_f(o, k, U[k]); put_f(o, 4 + k, V[k]); put_f(o, 8 + k, UA[k]); put_f(o, 12 + k, VA[k]);
				put_f(o, 16 + k, UE[k]); put_f(o, 20 + k, VE[k]);
				c[1 + k] = alpha[k]; c[7 + k] = alphaE[k];
			}
			c[5] = flip; c[11] = flipE; c[12] = cliffE;
			nCells++;
		}
	}
	/* initHeightData(w, h)'s sizes and updateBlock(0, 0, w-1, h-1) */
	HeightMapRenderObjClass r;
	r.m_x = w; r.m_y = h; r.m_originX = 0; r.m_originY = 0; r.m_map = m; r.m_showImpassableAreas = false;
	Int numVBTilesX = 1, numVBTilesY = 1, i, j;
	for (i = VERTEX_BUFFER_TILE_LENGTH + 1; i < w;) { i += VERTEX_BUFFER_TILE_LENGTH; numVBTilesX++; }
	for (j = VERTEX_BUFFER_TILE_LENGTH + 1; j < h;) { j += VERTEX_BUFFER_TILE_LENGTH; numVBTilesY++; }
	Int numVertex = VERTEX_BUFFER_TILE_LENGTH * 2 * VERTEX_BUFFER_TILE_LENGTH * 2;
	DX8VertexBufferClass *tiles = new DX8VertexBufferClass[numVBTilesX * numVBTilesY];
	DX8VertexBufferClass **tilePtrs = new DX8VertexBufferClass *[numVBTilesX * numVBTilesY];
	char **backup = new char *[numVBTilesX * numVBTilesY];
	for (i = 0; i < numVBTilesX * numVBTilesY; i++) {
		tiles[i].hw = (VertexShim *)calloc(numVertex, sizeof(VertexShim));
		tilePtrs[i] = &tiles[i];
		backup[i] = (char *)calloc(numVertex, sizeof(VertexShim));
	}
	r.m_vertexBufferTiles = tilePtrs;
	Int x0 = 0, y0 = 0, x1 = w - 1, y1 = h - 1;
	for (j = 0; j < numVBTilesY; j++) {
		Int originY = j * VERTEX_BUFFER_TILE_LENGTH;
		Int yMin = originY; if (y0 > yMin) yMin = y0;
		Int yMax = originY + VERTEX_BUFFER_TILE_LENGTH; if (y1 < yMax) yMax = y1;
		if (yMin >= yMax) continue;
		for (i = 0; i < numVBTilesX; i++) {
			Int originX = i * VERTEX_BUFFER_TILE_LENGTH;
			Int xMin = originX; if (xMin < x0) xMin = x0;
			Int xMax = originX + VERTEX_BUFFER_TILE_LENGTH; if (xMax > x1) xMax = x1;
			if (xMin >= xMax) continue;
			r.updateVB(tilePtrs[j * numVBTilesX + i], backup[j * numVBTilesX + i], xMin, yMin, xMax, yMax, originX, originY, m, NULL);
		}
	}
	for (Int y = 0; y < h - 1; y++) {
		for (Int x = 0; x < w - 1; x++) {
			Int tx = x / VERTEX_BUFFER_TILE_LENGTH, ty = y / VERTEX_BUFFER_TILE_LENGTH;
			VertexShim *v = tiles[ty * numVBTilesX + tx].hw + (y - ty * VERTEX_BUFFER_TILE_LENGTH) * VERTEX_BUFFER_TILE_LENGTH * 4 + (x - tx * VERTEX_BUFFER_TILE_LENGTH) * 4;
			Int c = y * (w - 1) + x;
			for (Int k = 0; k < 4; k++) {
				float *o = vtx + (c * 4 + k) * 10;
				o[0] = v[k].x; o[1] = v[k].y; o[2] = v[k].z; o[3] = v[k].nx; o[4] = v[k].ny; o[5] = v[k].nz;
				o[6] = v[k].u1; o[7] = v[k].v1; o[8] = v[k].u2; o[9] = v[k].v2;
				vtxAlpha[c * 4 + k] = v[k].alpha;
			}
		}
	}
	for (i = 0; i < numVBTilesX * numVBTilesY; i++) { free(tiles[i].hw); free(backup[i]); }
	delete [] tiles; delete [] tilePtrs; delete [] backup;
	for (i = 0; i < NUM_SOURCE_TILES; i++) { delete m->m_sourceTiles[i]; delete m->m_edgeTiles[i]; }
	delete m->m_terrainTex; delete m->m_alphaTerrainTex; delete m->m_alphaEdgeTex;
	delete [] m->m_data; delete [] m->m_tileNdxes; delete [] m->m_blendTileNdxes; delete [] m->m_extraBlendTileNdxes;
	delete [] m->m_cliffInfoNdxes; delete [] m->m_cellFlipState;
	delete m;
	return ret;
}
