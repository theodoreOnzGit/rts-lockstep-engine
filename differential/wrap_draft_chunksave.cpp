/* Differential-test wrapper for the drafts src/drafts/wwlib/chunk_save.rs (ChunkSaveClass) and twiddler.rs
** (TwiddlerClass::Save_Variables and Load_Variables) (#215, group wwlib2), upstream commit 0a05454d:
**   WWLib/chunkio.cpp's ChunkSaveClass and ChunkLoadClass (already compiled UNCHANGED into upstream_w3d; this wrapper calls them
**   through chunkio.h), and WWSaveLoad/twiddler.cpp's two enums and its Save_Variables and Load_Variables, cut VERBATIM by build.rs
**   (drafts_wwlib2: twiddler_enums.inc, twiddler_vars.inc).
** Compiled with differential/shim_w3d/w3d_prelude.h force-included, as the W3D build that holds chunkio.cpp is (32-bit uint32, which the
** C++ names of the members depend on).
** Stand-ins: TestFile, a FileClass over a growing byte vector (Write overwrites then extends, Seek with SEEK_SET/CUR/END, Read);
**   TwiddlerClass, reduced to the two members the cut uses (m_IndirectClassID, m_DefinitionList) and the two functions. The base
**   class DefinitionClass, the factories and Twiddle() (random: TIMEGETTIME) are not compiled.
**
** Entry points (extern "C", tests/differential_draft_chunksave.rs):
**   ref_cs_run(prog, n, bytes, cap, res): `prog` is a stream of 32-bit words: op, then its operands:
**     0 id            Begin_Chunk(id)              -> res: the bool
**     1               End_Chunk()                  -> res: the bool
**     2 id            Begin_Micro_Chunk(id)        -> res: the bool
**     3               End_Micro_Chunk()            -> res: the bool
**     4 len w0 w1..   Write(buf, len) of len bytes, the bytes being the low bytes of the following len words -> res: the count
**     5 k f0 f1..     Write(IOVector2/3/4Struct or IOQuaternionStruct) for k = 2, 3, 4, 5 (5 is the quaternion) -> res: the count
**     6               Cur_Chunk_Depth()            -> res: the depth
**   `bytes` receives the final file image; the return value is its length; `res` gets one word per op.
**   ref_tw_save(indirect, list, n, bytes, cap): Begin_Chunk(CHUNKID_VARIABLES), Save_Variables, End_Chunk; returns the length.
**   ref_tw_load(bytes, n, out): ChunkLoadClass over the image: Open_Chunk loop, Load_Variables for CHUNKID_VARIABLES, Close_Chunk;
**     out = [m_IndirectClassID, count, list...]; returns the words written. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <vector>
#include "always.h"
#include "chunkio.h"
#include "wwfile.h"
#include "vector.h"
#include "iostruct.h"

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

#include "twiddler_enums.inc"

class TwiddlerClass
{
public:
	TwiddlerClass() : m_IndirectClassID(0) {}
	bool Save_Variables(ChunkSaveClass &csave);
	bool Load_Variables(ChunkLoadClass &cload);
	uint32 m_IndirectClassID;
	DynamicVectorClass<int> m_DefinitionList;
};

#include "twiddler_vars.inc"

float fOf(unsigned w) { float f; memcpy(&f, &w, 4); return f; }

}

extern "C" {

int ref_cs_run(const unsigned *prog, int n, unsigned char *bytes, int cap, unsigned *res)
{
	MemFile file;
	ChunkSaveClass cs(&file);
	int r = 0;
	for (int i = 0; i < n;) {
		switch (prog[i]) {
		case 0: res[r++] = cs.Begin_Chunk(prog[i + 1]); i += 2; break;
		case 1: res[r++] = cs.End_Chunk(); i += 1; break;
		case 2: res[r++] = cs.Begin_Micro_Chunk(prog[i + 1]); i += 2; break;
		case 3: res[r++] = cs.End_Micro_Chunk(); i += 1; break;
		case 4: {
			unsigned len = prog[i + 1];
			std::vector<unsigned char> b(len + 1);
			for (unsigned k = 0; k < len; k++) b[k] = (unsigned char)prog[i + 2 + k];
			res[r++] = cs.Write(&b[0], len);
			i += 2 + len;
			break;
		}
		case 5: {
			unsigned k = prog[i + 1];
			float f[4];
			for (unsigned q = 0; q < (k == 5 ? 4u : k); q++) f[q] = fOf(prog[i + 2 + q]);
			if (k == 2) { IOVector2Struct v; v.X = f[0]; v.Y = f[1]; res[r++] = cs.Write(v); }
			else if (k == 3) { IOVector3Struct v; v.X = f[0]; v.Y = f[1]; v.Z = f[2]; res[r++] = cs.Write(v); }
			else if (k == 4) { IOVector4Struct v; v.X = f[0]; v.Y = f[1]; v.Z = f[2]; v.W = f[3]; res[r++] = cs.Write(v); }
			else { IOQuaternionStruct v; for (int q = 0; q < 4; q++) v.Q[q] = f[q]; res[r++] = cs.Write(v); }
			i += 2 + (k == 5 ? 4 : k);
			break;
		}
		default: res[r++] = cs.Cur_Chunk_Depth(); i += 1; break;
		}
	}
	int len = (int)file.data.size();
	if (len > cap) len = cap;
	if (len > 0) memcpy(bytes, &file.data[0], len);
	return (int)file.data.size();
}

int ref_tw_save(unsigned indirect, const int *list, int n, unsigned char *bytes, int cap)
{
	MemFile file;
	ChunkSaveClass cs(&file);
	TwiddlerClass t;
	t.m_IndirectClassID = indirect;
	for (int i = 0; i < n; i++) t.m_DefinitionList.Add(list[i]);
	cs.Begin_Chunk(CHUNKID_VARIABLES);
	t.Save_Variables(cs);
	cs.End_Chunk();
	int len = (int)file.data.size();
	if (len > cap) len = cap;
	if (len > 0) memcpy(bytes, &file.data[0], len);
	return (int)file.data.size();
}

int ref_tw_load(const unsigned char *bytes, int n, unsigned *out)
{
	MemFile file;
	file.data.assign(bytes, bytes + n);
	ChunkLoadClass cl(&file);
	TwiddlerClass t;
	t.m_IndirectClassID = 0xA5A5A5A5u; /* so that a load that never sets it is visible */
	t.m_DefinitionList.Add(777);        /* "Start fresh": Load_Variables must clear this */
	while (cl.Open_Chunk()) {
		if (cl.Cur_Chunk_ID() == CHUNKID_VARIABLES) t.Load_Variables(cl);
		cl.Close_Chunk();
	}
	int w = 0;
	out[w++] = t.m_IndirectClassID;
	out[w++] = (unsigned)t.m_DefinitionList.Count();
	for (int i = 0; i < t.m_DefinitionList.Count(); i++) out[w++] = (unsigned)t.m_DefinitionList[i];
	return w;
}

}
