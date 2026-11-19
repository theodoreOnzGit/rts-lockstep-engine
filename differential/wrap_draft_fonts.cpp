/* Differential-test wrapper for the font library and header template drafts (#215, upstream commit 0a05454d),
** src/drafts/sweep_gui_logic/game_font.rs (FontLibrary::link_font, unlink_font, delete_all_fonts) and header_template.rs
** (HeaderTemplateManager::find_header_template, new_header_template, get_first_header, get_next_header, populate_game_fonts),
** cut verbatim by build.rs (drafts_guilogic3) into:
**   font_class.inc  GameFont.h, `class FontLibrary : public SubsystemInterface` to `// INLINING`;
**   font_fns.inc    GameFont.cpp, linkFont, unlinkFont, deleteAllFonts (to `// PUBLIC FUNCTIONS`);
**   ht_class.inc    HeaderTemplate.h, `class HeaderTemplate` to `// INLINING`;
**   ht_ctor.inc     HeaderTemplate.cpp, HeaderTemplate::HeaderTemplate to HeaderTemplateManager::init (the constructors, destructors);
**   ht_fns1.inc     findHeaderTemplate .. getNextHeader (to headerNotifyResolutionChange);   ht_fns2.inc  populateGameFonts to the end.
** Built on the REAL AsciiString.h and AsciiString.cpp (shim_ascii; classes renamed by build.rs).
** Stand-ins (outside the cuts): GameFont (the fields the cuts read: next, nameString, pointSize, height, fontData, bold; the real one is
** a MemoryPoolObject; `deleteInstance` is `delete this`); SubsystemInterface (virtual init, reset, update); FontLibrary's constructor,
** destructor, init, reset (declared in the cut class, defined outside it) and getFont, which returns a font made for (name, size,
** bold), or NULL for a name starting with `n` (the real one loads a device font); a FontLibrary subclass that defines loadFontData
** (pure in the real class) and `releaseFontData` (`W3DFontLibrary::releaseFontData` releases the device's font pages, W3DGameFont.cpp:130,
** then sets fontData NULL: here it only sets fontData NULL, as the draft does; `deleteInstance` follows at once, so nothing else
** is observable); `TheGlobalLanguageData->adjustFontSize(Int)` (GlobalLanguage.cpp:192 depends on the resolution; here p * 2 - 7, an
** Int, as upstream's return type); `FieldParse` (declared by the header class; an empty struct); DEBUG_CRASH / DEBUG_ASSERTCRASH
** are no-ops (as the release build); `NEW` is `new`.
**
** Entry points (extern "C", tests/differential_draft_fonts.rs):
**   ref_fonts_run(in, n, out, cap)   ops: 0 id: make font id (height = id); 1 id: linkFont; 2 id: unlinkFont (id -1: NULL); 3: deleteAllFonts
**       (fonts left in the list are then dead); 4: dump -> [m_count, then the ids along m_fontList, -1 ending].
**   ref_ht_run(in, n, out, cap)      ops: 0 k: newHeaderTemplate("h<k>", or "H<k-100>" for k >= 100) -> [list place of the new one (0)]; 1 k: findHeaderTemplate(the same names) -> [place or -1];
**       2: getFirstHeader -> [place or -1]; 3 place: getNextHeader(the template at that place; a place >= size is a template
**       outside the list) -> [place or -1]; 4 place fontnameid point bold: sets the template at that place; 5: populateGameFonts;
**       6: dump -> [size, then per template: has font, font size, font bold]. A font name is "n<k>" (has no font) or "f<k>".
** Both return the number of ints written (-1: cap too small). */
#include <string.h>
#include <list>
#include <vector>
/* the cut classes keep members `private` / `protected`; the test subclasses read them */
#define private public
#define protected public

DynamicMemoryAllocator *TheDynamicMemoryAllocator = 0;
static DynamicMemoryAllocator theFontsAllocator;

#define DEBUG_CRASH(x) ((void)0)
#define DEBUG_ASSERTCRASH(c, m) ((void)0)
#define NEW new

struct FieldParse { int dummy; };
class INI;

class GameFont
{
public:
	void deleteInstance() { delete this; }
	GameFont*			next;
	AsciiString		nameString;
	Int						pointSize;
	Int						height;
	void*					fontData;
	Bool					bold;
};

class SubsystemInterface
{
public:
	virtual ~SubsystemInterface() {}
	virtual void init() {}
	virtual void reset() {}
	virtual void update() {}
};

#include "font_class.inc"

FontLibrary::FontLibrary( void ) { m_fontList = NULL; m_count = 0; }
FontLibrary::~FontLibrary( void ) {}
void FontLibrary::init( void ) {}
void FontLibrary::reset( void ) {}
static std::vector<GameFont *> theMadeFonts;
GameFont *FontLibrary::getFont( AsciiString name, Int pointSize, Bool bold )
{
	if (name.str()[0] == 'n') return NULL;
	GameFont *f = new GameFont;
	f->next = NULL;
	f->nameString = name;
	f->pointSize = pointSize;
	f->height = 0;
	f->fontData = NULL;
	f->bold = bold;
	theMadeFonts.push_back(f);
	return f;
}

#include "font_fns.inc"

class TestFontLibrary : public FontLibrary
{
public:
	virtual Bool loadFontData( GameFont *font ) { return TRUE; }
	virtual void releaseFontData( GameFont *font ) { font->fontData = NULL; }
	void link(GameFont *f) { linkFont(f); }
	void unlink(GameFont *f) { unlinkFont(f); }
	void deleteAll() { deleteAllFonts(); }
	GameFont *head() { return m_fontList; }
	Int count() { return m_count; }
};

extern "C" int ref_fonts_run(const int *in, int nin, int *out, int cap)
{
	TheDynamicMemoryAllocator = &theFontsAllocator;
	std::vector<int> o;
	GameFont *tbl[512];
	for (int i = 0; i < 512; ++i) tbl[i] = 0;
	TestFontLibrary lib;
	int at = 0;
	while (at < nin)
	{
		int kind = in[at++];
		switch (kind)
		{
			case 0:
			{
				GameFont *f = new GameFont;
				f->next = NULL; f->pointSize = 0; f->height = in[at]; f->fontData = (void *)1; f->bold = FALSE;
				tbl[in[at]] = f;
				at += 1;
				break;
			}
			case 1: lib.link(tbl[in[at]]); at += 1; break;
			case 2: lib.unlink(in[at] < 0 ? NULL : tbl[in[at]]); at += 1; break;
			case 3: lib.deleteAll(); at += 0; break;
			default:
			{
				o.push_back(lib.count());
				for (GameFont *f = lib.head(); f; f = f->next) o.push_back(f->height);
				o.push_back(-1);
				break;
			}
		}
	}
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); ++i) out[i] = o[i];
	return (int)o.size();
}

#include "ht_class.inc"

struct GlobalLanguage { Int adjustFontSize(Int p) { return p * 2 - 7; } };
static GlobalLanguage theGlobalLanguage;
GlobalLanguage *TheGlobalLanguageData = &theGlobalLanguage;
static TestFontLibrary theTestFontLibrary;
FontLibrary *TheFontLibrary = &theTestFontLibrary;

#include "ht_ctor.inc"
#include "ht_fns1.inc"
#include "ht_fns2.inc"

class TestHeaderManager : public HeaderTemplateManager
{
public:
	int placeOf(HeaderTemplate *h) { int i = 0; for (HeaderTemplateListIt it = m_headerTemplateList.begin(); it != m_headerTemplateList.end(); ++it, ++i) if (*it == h) return i; return -1; }
	HeaderTemplate *at(int place) { int i = 0; for (HeaderTemplateListIt it = m_headerTemplateList.begin(); it != m_headerTemplateList.end(); ++it, ++i) if (i == place) return *it; return NULL; }
	int size() { return (int)m_headerTemplateList.size(); }
	void populate() { populateGameFonts(); }
	std::list<HeaderTemplate *> &list() { return m_headerTemplateList; }
};

extern "C" int ref_ht_run(const int *in, int nin, int *out, int cap)
{
	TheDynamicMemoryAllocator = &theFontsAllocator;
	std::vector<int> o;
	TestHeaderManager m;
	HeaderTemplate stray;
	int at = 0;
	char name[16];
	while (at < nin)
	{
		int kind = in[at++];
		switch (kind)
		{
			case 0: { sprintf(name, "%c%d", in[at] >= 100 ? 'H' : 'h', in[at] % 100); at += 1; o.push_back(m.placeOf(m.newHeaderTemplate(AsciiString(name)))); break; }
			case 1: { sprintf(name, "%c%d", in[at] >= 100 ? 'H' : 'h', in[at] % 100); at += 1; o.push_back(m.placeOf(m.findHeaderTemplate(AsciiString(name)))); break; }
			case 2: o.push_back(m.placeOf(m.getFirstHeader())); break;
			case 3:
			{
				HeaderTemplate *h = in[at] >= m.size() ? &stray : m.at(in[at]);
				at += 1;
				o.push_back(m.placeOf(m.getNextHeader(h)));
				break;
			}
			case 4:
			{
				HeaderTemplate *h = m.at(in[at]);
				sprintf(name, "%c%d", in[at + 1] < 100 ? 'f' : 'n', in[at + 1] % 100);
				h->m_fontName = AsciiString(name);
				h->m_point = in[at + 2];
				h->m_bold = in[at + 3] != 0;
				at += 4;
				break;
			}
			case 5: m.populate(); break;
			default:
			{
				o.push_back(m.size());
				for (TestHeaderManager::HeaderTemplateListIt it = m.list().begin(); it != m.list().end(); ++it)
				{
					HeaderTemplate *h = *it;
					o.push_back(h->m_font != NULL);
					o.push_back(h->m_font ? h->m_font->pointSize : 0);
					o.push_back(h->m_font ? h->m_font->bold : 0);
				}
				break;
			}
		}
	}
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); ++i) out[i] = o[i];
	return (int)o.size();
}
