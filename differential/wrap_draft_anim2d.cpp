/* Differential-test wrapper for the 2D animation draft (#215, upstream commit 0a05454d),
** src/drafts/sweep_common_client/anim2d.rs (Anim2DTemplate, Anim2D, Anim2DCollection), cut verbatim by build.rs into
** OUT_DIR/draft_guilogic2_gen:
**   anim2d_h1.inc   GameClient/Anim2D.h, enum Anim2DMode (and Anim2DModeNames, under DEFINE_ANIM_2D_MODE_NAMES as Anim2D.cpp has it)
**   anim2d_h2.inc   Anim2D.h, class Anim2DTemplate, enum Anim2DStatus, class Anim2D, class Anim2DCollection
**   anim2d_c1.inc   GameClient/System/Anim2D.cpp, Anim2DTemplate's constructor and destructor
**   anim2d_c2.inc   parseNumImages .. getFrame (not the field parse table)
**   anim2d_c3.inc   Anim2D::Anim2D .. Anim2D::draw (not Anim2D::xfer)
**   anim2d_c4.inc   Anim2DCollection's constructor and destructor
**   anim2d_c5.inc   Anim2DCollection::update .. unRegisterAnimation (to the end of the file)
** Names: build.rs renames Anim2D, Anim2DTemplate, Anim2DCollection, Image, INI, FieldParse, AsciiString and the allocator pointer
** (-D), so that no symbol of this wrapper can meet another wrapper's.
** Stand-ins (everything the cut calls outside itself):
**   Image (name, size: Image.h's getName, getImageSize, getImageWidth, getImageHeight), Display::drawImage (logs the call),
**   GameLogic::getFrame (a settable frame), GetGameClientRandomValue (the next queued value; logs lo and hi),
**   TheMappedImageCollection->findImageByName (a table of named images), INI (parseUnsignedInt, parseMappedImage and
**   getNextAsciiString answer from values the test gives; INI_INVALID_DATA is an int that the wrapper catches),
**   FieldParse (the field table's element type, never used), Snapshot (the base class; Anim2D::xfer is an empty body here),
**   Anim2DCollection::init (upstream loads Data\INI\Animation2D.ini), `protected` read as `public` for the cut so that the
**   protected members and tryNextFrame can be driven, the 16-bit/32-bit reading of nothing else.
** Entry points (extern "C", tests/differential_draft_anim2d.rs):
**   ref_anim2d_mode_name(i, out, cap)   Anim2DModeNames[i], NUL-terminated, length (-1: no such name)
**   ref_anim2d_run(in, nin, out, cap)   runs a program, appends each op's answer to out; the number of words (-1: too small) */

#define GetGameClientRandomValue g2anim_GetGameClientRandomValue
#include "PreRTS.h"
#include <vector>
#include <map>
#include <string>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include <new>

/* ---- stand-ins ------------------------------------------------------------------------------------------------------- */
#include "Common/AsciiString.h"
#include "GameClient/Color.h"
#include "Common/GameMemory.h"
#include "Common/SubsystemInterface.h"

typedef int G2AnimThrown;
static const G2AnimThrown INI_INVALID_DATA = 7;

class Xfer;
class Snapshot
{
public:
	virtual void crc( Xfer *xfer ) = 0;
	virtual void xfer( Xfer *xfer ) = 0;
	virtual void loadPostProcess( void ) = 0;
};

struct FieldParse { int unused; };

class Image
{
public:
	AsciiString m_name;
	ICoord2D m_imageSize;
	AsciiString getName( void ) const { return m_name; }
	const ICoord2D *getImageSize( void ) const { return &m_imageSize; }
	Int getImageWidth( void ) const { return m_imageSize.x; }
	Int getImageHeight( void ) const { return m_imageSize.y; }
};

namespace {
std::vector<int> *g_out = 0;
std::vector<Image *> g_images;       /* all images, by index */
std::vector<bool> g_known;           /* image i is in the mapped image collection */
UnsignedInt g_frame = 0;
std::vector<int> g_randoms;
size_t g_randomAt = 0;
UnsignedInt g_parseUnsigned = 0;
Image *g_parseImage = 0;
std::string g_nextToken;

int imageIndex( const Image *im )
{
	if( im == 0 )
		return -1;
	for( size_t i = 0; i < g_images.size(); ++i )
		if( g_images[i] == im )
			return (int)i;
	return -2;
}

struct G2AnimDisplay
{
	void drawImage( const Image *image, Int x0, Int y0, Int x1, Int y1, Color color )
	{
		g_out->push_back( 1000 );
		g_out->push_back( imageIndex( image ) );
		g_out->push_back( x0 ); g_out->push_back( y0 ); g_out->push_back( x1 ); g_out->push_back( y1 );
		g_out->push_back( (int)color );
	}
};
struct G2AnimGameLogic { UnsignedInt getFrame( void ) { return g_frame; } };
struct G2AnimImageCollection
{
	const Image *findImageByName( const AsciiString &name )
	{
		for( size_t i = 0; i < g_images.size(); ++i )
			if( g_known[i] && g_images[i]->m_name == name )
				return g_images[i];
		return 0;
	}
};
G2AnimDisplay theDisplay;
G2AnimGameLogic theLogic;
G2AnimImageCollection theImages;
G2AnimDisplay *TheDisplay = &theDisplay;
G2AnimGameLogic *TheGameLogic = &theLogic;
G2AnimImageCollection *TheMappedImageCollection = &theImages;
}

Int GetGameClientRandomValue( int lo, int hi, char *file, int line )
{
	g_out->push_back( 2000 );
	g_out->push_back( lo );
	g_out->push_back( hi );
	if( g_randomAt < g_randoms.size() )
		return g_randoms[g_randomAt++];
	return lo;
}

class INI
{
public:
	void parseUnsignedInt( INI *ini, void *instance, void *store, const void *userData ) { *(UnsignedInt *)store = g_parseUnsigned; }
	void parseMappedImage( INI *ini, void *instance, void *store, const void *userData ) { *(const Image **)store = g_parseImage; }
	AsciiString getNextAsciiString( void ) { return AsciiString( g_nextToken.c_str() ); }
};

class Anim2DCollection; /* (declared by an earlier include upstream) */
#define DEFINE_ANIM_2D_MODE_NAMES
#undef protected
#define protected public
#include "anim2d_h1.inc"
#include "anim2d_h2.inc"
#undef protected

#include "anim2d_c1.inc"
#include "anim2d_c2.inc"
#include "anim2d_c3.inc"
#include "anim2d_c4.inc"
#include "anim2d_c5.inc"

Anim2DCollection *TheAnim2DCollection = NULL;
void Anim2D::xfer( Xfer *xfer ) { }
void Anim2DCollection::init( void ) { }

extern "C" int ref_anim2d_mode_name( int i, char *out, int cap )
{
	if( i < 0 || i >= (int)ANIM_2D_NUM_MODES )
		return -1;
	int n = (int)strlen( Anim2DModeNames[i] );
	if( n + 1 > cap )
		return -1;
	memcpy( out, Anim2DModeNames[i], n + 1 );
	return n;
}

/* Program (every word an int): image table first, then ops.
**   nimg, then per image: known, width, height. Image i's name is "I<i>" for i < 8, else "Sq<i-8, 3 digits>".
**   randoms: n, then n values (GameClientRandomValue answers in order; lo when exhausted).
** Ops, each a kind word then its fields (templates and instances are numbered in creation order):
**   0 newTemplate(nameIdx)                -> [template count]      names: "A0".."A3"
**   1 setTemplate t mode between rand     (fields set through the protected members, as the INI field table does)
**   2 parseNumImages t n                  -> [thrown 0/1] ; n is the unsigned value parsed
**   3 parseImage t img                    -> [thrown]  (img -1: not found)
**   4 parseImageSequence t base           -> [thrown]  base 0 "Sq", 1 "Zz", 2 ""
**   5 allocateImages t n                  6 storeImage t img -> [thrown]   (img -1: NULL)
**   7 getFrame t frame -> [image index, -1 for NULL]
**   8 newAnim t register -> [id]          (logs the random call, if any)
**   9 setMinMax a min max                 10 setStatus a bits      11 clearStatus a bits     12 setAlpha a f32bits
**   13 setFrameNumber F                   14 update
**   15 draw a x y   (skipped, answer -9, when the current frame has no image: upstream dereferences NULL)
**   16 drawSized a x y w h (same)
**   17 query a -> [currentFrame, status, lastUpdate?, width, height]  (getCurrentFrame, getStatus, getCurrentFrameWidth, Height)
**   18 tryNextFrame a     19 setCurrentFrame a f      20 randomizeCurrentFrame a     21 reset a
**   22 deleteAnim a       23 findTemplate nameIdx -> [index or -1]    24 getNextTemplate t -> [index or -1]
**   25 registerAnimation a   26 unRegisterAnimation a
**   27 dumpAnim a -> [currentFrame, lastUpdateFrame, status, minFrame, maxFrame, framesBetweenUpdates, alpha bits]
**   28 dumpTemplate t -> [numFrames, framesBetween, mode, randomize, image index of each slot (-1 NULL)] */
extern "C" int ref_anim2d_run( const int *in, int nin, int *outp, int cap )
{
	std::vector<int> out;
	g_out = &out;
	int at = 0;
	for( size_t i = 0; i < g_images.size(); ++i ) delete g_images[i];
	g_images.clear(); g_known.clear();
	g_frame = 0; g_randoms.clear(); g_randomAt = 0; g_parseUnsigned = 0; g_parseImage = 0;
	int nimg = in[at++];
	for( int i = 0; i < nimg; ++i )
	{
		Image *im = new Image;
		char nm[16];
		if( i < 8 ) snprintf( nm, sizeof nm, "I%d", i ); else snprintf( nm, sizeof nm, "Sq%03d", i - 8 );
		im->m_name = nm;
		g_known.push_back( in[at] != 0 );
		im->m_imageSize.x = in[at + 1]; im->m_imageSize.y = in[at + 2];
		at += 3;
		g_images.push_back( im );
	}
	int nr = in[at++];
	for( int i = 0; i < nr; ++i ) g_randoms.push_back( in[at++] );

	Anim2DCollection *coll = new Anim2DCollection;
	TheAnim2DCollection = coll;
	std::vector<Anim2DTemplate *> templates;
	std::vector<Anim2D *> anims;
	std::vector<bool> alive;
	const char *tnames[4] = { "A0", "A1", "A2", "A3" };
	const char *bases[3] = { "Sq", "Zz", "" };
	INI ini;
	auto tindex = []( std::vector<Anim2DTemplate *> &v, Anim2DTemplate *t ) -> int { for( size_t i = 0; i < v.size(); ++i ) if( v[i] == t ) return (int)i; return -1; };
	auto imgOrNull = []( int i ) -> Image * { return ( i >= 0 && i < (int)g_images.size() ) ? g_images[i] : 0; };

	while( at < nin )
	{
		int kind = in[at++];
		switch( kind )
		{
			case 0: { Anim2DTemplate *t = coll->newTemplate( AsciiString( tnames[in[at]] ) ); at += 1; templates.push_back( t ); out.push_back( (int)templates.size() ); break; }
			case 1: { Anim2DTemplate *t = templates[in[at]]; t->m_animMode = (Anim2DMode)in[at + 1]; t->m_framesBetweenUpdates = (UnsignedShort)in[at + 2]; t->m_randomizeStartFrame = in[at + 3] != 0; at += 4; break; }
			case 2: { Anim2DTemplate *t = templates[in[at]]; g_parseUnsigned = (UnsignedInt)in[at + 1]; at += 2;
				int th = 0; try { Anim2DTemplate::parseNumImages( &ini, t, 0, 0 ); } catch( G2AnimThrown ) { th = 1; } out.push_back( th ); break; }
			case 3: { Anim2DTemplate *t = templates[in[at]]; g_parseImage = imgOrNull( in[at + 1] ); at += 2;
				int th = 0; try { Anim2DTemplate::parseImage( &ini, t, 0, 0 ); } catch( G2AnimThrown ) { th = 1; } out.push_back( th ); break; }
			case 4: { Anim2DTemplate *t = templates[in[at]]; g_nextToken = bases[in[at + 1]]; at += 2;
				int th = 0; try { Anim2DTemplate::parseImageSequence( &ini, t, 0, 0 ); } catch( G2AnimThrown ) { th = 1; } out.push_back( th ); break; }
			case 5: { templates[in[at]]->allocateImages( (UnsignedShort)in[at + 1] ); at += 2; break; }
			case 6: { Anim2DTemplate *t = templates[in[at]]; const Image *im = imgOrNull( in[at + 1] ); at += 2;
				int th = 0; try { t->storeImage( im ); } catch( G2AnimThrown ) { th = 1; } out.push_back( th ); break; }
			case 7: { out.push_back( imageIndex( templates[in[at]]->getFrame( (UnsignedShort)in[at + 1] ) ) ); at += 2; break; }
			case 8: { Anim2DTemplate *t = templates[in[at]]; int reg = in[at + 1]; at += 2;
				Anim2D *a = newInstance( Anim2D )( t, reg ? coll : NULL ); anims.push_back( a ); alive.push_back( true ); out.push_back( (int)anims.size() - 1 ); break; }
			case 9: { anims[in[at]]->setMinFrame( (UnsignedShort)in[at + 1] ); anims[in[at]]->setMaxFrame( (UnsignedShort)in[at + 2] ); at += 3; break; }
			case 10: { anims[in[at]]->setStatus( (UnsignedByte)in[at + 1] ); at += 2; break; }
			case 11: { anims[in[at]]->clearStatus( (UnsignedByte)in[at + 1] ); at += 2; break; }
			case 12: { float f; memcpy( &f, &in[at + 1], 4 ); anims[in[at]]->setAlpha( f ); at += 2; break; }
			case 13: { g_frame = (UnsignedInt)in[at]; at += 1; break; }
			case 14: { coll->update(); break; }
			case 15: case 16:
			{
				Anim2D *a = anims[in[at]];
				bool has = a->getAnimTemplate()->getFrame( a->getCurrentFrame() ) != 0;
				if( kind == 15 ) { int x = in[at + 1], y = in[at + 2]; at += 3; if( has ) a->draw( x, y ); else out.push_back( -9 ); }
				else { int x = in[at + 1], y = in[at + 2], w = in[at + 3], h = in[at + 4]; at += 5; if( has ) a->draw( x, y, w, h ); else out.push_back( -9 ); }
				break;
			}
			case 17: { Anim2D *a = anims[in[at]]; at += 1; out.push_back( a->getCurrentFrame() ); out.push_back( a->getStatus() ); out.push_back( (int)a->getCurrentFrameWidth() ); out.push_back( (int)a->getCurrentFrameHeight() ); break; }
			case 18: { anims[in[at]]->tryNextFrame(); at += 1; break; }
			case 19: { anims[in[at]]->setCurrentFrame( (UnsignedShort)in[at + 1] ); at += 2; break; }
			case 20: { anims[in[at]]->randomizeCurrentFrame(); at += 1; break; }
			case 21: { anims[in[at]]->reset(); at += 1; break; }
			case 22: { int i = in[at++]; anims[i]->deleteInstance(); alive[i] = false; break; }
			case 23: { out.push_back( tindex( templates, coll->findTemplate( AsciiString( tnames[in[at]] ) ) ) ); at += 1; break; }
			case 24: { out.push_back( tindex( templates, coll->getNextTemplate( templates[in[at]] ) ) ); at += 1; break; }
			case 25: { coll->registerAnimation( anims[in[at]] ); at += 1; break; }
			case 26: { coll->unRegisterAnimation( anims[in[at]] ); at += 1; break; }
			case 27:
			{
				Anim2D *a = anims[in[at]]; at += 1;
				out.push_back( a->m_currentFrame ); out.push_back( (int)a->m_lastUpdateFrame ); out.push_back( a->m_status );
				out.push_back( a->m_minFrame ); out.push_back( a->m_maxFrame ); out.push_back( (int)a->m_framesBetweenUpdates );
				int ab; memcpy( &ab, &a->m_alpha, 4 ); out.push_back( ab );
				break;
			}
			default:
			{
				Anim2DTemplate *t = templates[in[at]]; at += 1;
				out.push_back( t->m_numFrames ); out.push_back( t->m_framesBetweenUpdates ); out.push_back( (int)t->m_animMode ); out.push_back( t->m_randomizeStartFrame );
				for( int i = 0; i < t->m_numFrames; ++i ) out.push_back( imageIndex( t->m_images ? t->m_images[i] : 0 ) );
				break;
			}
		}
	}
	/* tidy up: the instances are left (deleting one that was unlinked by hand would unlink it again from stale pointers); the collection frees its templates */
	delete coll;
	TheAnim2DCollection = NULL;
	g_out = 0;
	if( (int)out.size() > cap )
		return -1;
	memcpy( outp, out.data(), out.size() * sizeof( int ) );
	return (int)out.size();
}
