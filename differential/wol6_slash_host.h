/* The stand-ins shared by the two slash-command references of group wol6 (#215), included INSIDE a namespace of
** differential/wrap_draft_wol_slash.cpp after the system headers and "Lib/BaseType.h" (so no global is shared between the lobby's and the
** game setup's, nor with another wrapper). They replace what the cuts call outside themselves:
**   AsciiString   the members the cuts use, transcribed from AsciiString.cpp / AsciiString.h: `translate` (each wide character cast to `char` and
**                 concatenated with `concat(const char c)`, which is `strlen(tmp) == 0 -> return`: a character whose low byte is 0 adds NOTHING),
**                 `getCharAt` (0 on an empty string), `nextToken` (skipSeps / skipNonSeps, the default separators " \n\r\t"), `toLower` (tolower),
**                 `operator==(const char *)`, the `const char *` constructor, `str()`.
**   UnicodeString the members the cuts use: `getLength`, `str`, the `const WideChar *` constructor, `format(const wchar_t *, ...)` (vswprintf, the
**                 case's two `%d`; the result kept as 16-bit units).
**   TheGameSpyInfo->addText / sendChat   log the call (the text, the colour or the emote flag, the window's id or -1).
**   GameSpyColor  the case's default colour (the enum is PeerDefs.h's, cut verbatim before this header).
**   timeGetTime() the case's clock.   hasStagingRoomListChanged(), RefreshGameListBoxes(), PopulateLobbyPlayerListbox()   logged / the case's answer.
** A text on the program's input is its UTF-16 units in hex, comma separated; `-` is the empty text. */
struct GameWindow
{
	int id;
};
static std::string w6log;
static void w6printf(const char *fmt, ...)
{
	char b[1 << 15];
	va_list a;
	va_start(a, fmt);
	vsnprintf(b, sizeof b, fmt, a);
	va_end(a);
	w6log += b;
	w6log += "\n";
}
class UnicodeString;
class AsciiString
{
public:
	std::string s;
	AsciiString() {}
	AsciiString(const char *c) : s(c) {}
	const char *str() const { return s.c_str(); }
	Bool isEmpty() const { return s.empty(); }
	Int getLength() const { return (Int)s.size(); }
	char getCharAt(Int i) const { return s.empty() ? 0 : s[i]; }
	void clear() { s.clear(); }
	void translate(const UnicodeString &u);
	void concat(const char c)
	{
		char tmp[2] = { c, 0 };
		if (strlen(tmp) == 0) return;
		s += tmp;
	}
	void toLower() { for (size_t i = 0; i < s.size(); ++i) s[i] = (char)tolower((unsigned char)s[i]); }
	Bool nextToken(AsciiString *tok, const char *seps = NULL)
	{
		if (isEmpty() || tok == this) return false;
		if (seps == NULL) seps = " \n\r\t";
		size_t start = 0;
		while (start < s.size() && strchr(seps, s[start])) ++start;
		size_t end = start;
		while (end < s.size() && !strchr(seps, s[end])) ++end;
		if (end > start) { tok->s = s.substr(start, end - start); s = s.substr(end); return true; }
		s.clear();
		tok->s.clear();
		return false;
	}
};
inline Bool operator==(const AsciiString &a, const char *b) { return a.s == b; }
class UnicodeString
{
public:
	std::vector<WideChar> v;   /* the units, then a NUL kept at the end for str() */
	UnicodeString() { v.push_back(0); }
	explicit UnicodeString(const WideChar *w) { for (; *w; ++w) v.push_back(*w); v.push_back(0); }
	const WideChar *str() const { return &v[0]; }
	Int getLength() const { return (Int)v.size() - 1; }
	WideChar getCharAt(Int i) const { return v[i]; }
	void format(const wchar_t *fmt, ...)
	{
		wchar_t b[1024];
		va_list a;
		va_start(a, fmt);
		vswprintf(b, 1024, fmt, a);
		va_end(a);
		v.clear();
		for (const wchar_t *c = b; *c; ++c) v.push_back((WideChar)*c);
		v.push_back(0);
	}
};
void AsciiString::translate(const UnicodeString &u)   /* AsciiString::translate */
{
	clear();
	Int len = u.getLength();
	for (Int i = 0; i < len; i++) concat((char)u.getCharAt(i));
}
static std::string w6hex(const WideChar *p, Int n)
{
	if (n == 0) return "-";
	std::string o;
	char b[16];
	for (Int i = 0; i < n; ++i)
	{
		snprintf(b, sizeof b, "%s%x", i ? "," : "", (unsigned)p[i]);
		o += b;
	}
	return o;
}
static GameWindow w6_players_window = { 7 };
typedef Int Color;
Color GameSpyColor[GSCOLOR_MAX];
static unsigned w6_now = 0;
static Bool w6_list_changed = FALSE;
static Int w6_win_id(GameWindow *w) { return w ? w->id : -1; }
class GameSpyInfo
{
public:
	void addText(UnicodeString text, Color color, GameWindow *win) { w6printf("addText %s color %d win %d", w6hex(text.str(), text.getLength()).c_str(), (int)color, (int)w6_win_id(win)); }
	void sendChat(UnicodeString text, Bool isEmote, GameWindow *win) { w6printf("sendChat %s emote %d win %d", w6hex(text.str(), text.getLength()).c_str(), (int)isEmote, (int)w6_win_id(win)); }
	Bool hasStagingRoomListChanged() { w6printf("hasStagingRoomListChanged -> %d", (int)w6_list_changed); return w6_list_changed; }
};
static GameSpyInfo w6_info;
static GameSpyInfo *TheGameSpyInfo = &w6_info;
static unsigned timeGetTime() { return w6_now; }
static void RefreshGameListBoxes() { w6printf("RefreshGameListBoxes"); }
