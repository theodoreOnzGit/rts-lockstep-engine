/* Differential-test wrapper for the drafts src/drafts/wwlib/math_util.rs and misc_util.rs (#215, group wwlib2),
** upstream commit 0a05454d:
**   GeneralsMD/Code/Libraries/Source/WWVegas/Wwutil/mathutil.h, mathutil.cpp (cMathUtil), compiled UNCHANGED (#included);
**   miscutil.h's class and constant, and miscutil.cpp's Seconds_To_Hms, Is_String_Same, Is_String_Different, Is_Alphabetic,
**   Is_Numeric, Is_Alphanumeric, Is_Whitespace and Trim_Trailing_Whitespace, cut VERBATIM by build.rs (drafts_wwlib2).
** Stand-ins (what the cuts call outside themselves):
**   - LPCSTR (a Windows typedef) and stricmp (MSVC's case-insensitive compare; glibc's strcasecmp, same ASCII folding in the
**     "C" locale);
**   - StringClass, only named by the declarations of the functions that are not compiled here (Get_File_Id_String);
**   - ::rand() and RAND_MAX: the MSVC C runtime's generator returns 0..32767 and is not reproducible; mathutil.cpp's calls go to
**     ref_mu_rand, which reads the next word of the sequence the test supplies (and counts them), RAND_MAX is MSVC's 32767.
** Not compiled (Windows or the file system): Get_Text_Time, File_Exists, File_Is_Read_Only, Get_File_Id_String, Remove_File.
** The elementary functions (sinf, cosf, atanf, sqrt) are the test binary's interposers (tests/support). */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <math.h>

typedef const char *LPCSTR;
#define stricmp strcasecmp
class StringClass;
#define MISCUTIL_H
#include "miscutil_h.inc"

#undef RAND_MAX
#define RAND_MAX 32767
static const int *g_seq = 0;
static int g_nseq = 0;
static int g_used = 0;
static int ref_mu_rand(void)
{
	if (g_used < g_nseq) return g_seq[g_used++];
	g_used++;
	return 0;
}
#define rand ref_mu_rand
#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/Wwutil/mathutil.cpp"
#undef rand

#include "miscutil_c1.inc"
#include "miscutil_c2.inc"

extern "C" {

void ref_mu_angle_to_vector(double angle, double *out)
{
	double dx = 0, dy = 0;
	cMathUtil::Angle_To_Vector(angle, dx, dy);
	out[0] = dx;
	out[1] = dy;
}

double ref_mu_vector_to_angle(double dx, double dy)
{
	double a = 0;
	cMathUtil::Vector_To_Angle(dx, dy, a);
	return a;
}

double ref_mu_simple_distance(double x1, double y1, double x2, double y2) { return cMathUtil::Simple_Distance(x1, y1, x2, y2); }

int ref_mu_round(double arg) { return cMathUtil::Round(arg); }

void ref_mu_rotate(double vx, double vy, double angle, double *out)
{
	cMathUtil::Rotate_Vector(vx, vy, angle);
	out[0] = vx;
	out[1] = vy;
}

/* op: 0 Get_Uniform_Pdf_Double(lower, upper)  1 Get_Normalized_Uniform_Pdf_Double  2 Get_Uniform_Pdf_Int(ilower, iupper)
**     3 Get_Hat_Pdf_Double(lower, upper)  4 Get_Normalized_Hat_Pdf_Double  5 Get_Hat_Pdf_Int(ilower, iupper)
** `seq` are the words ::rand() returns, in order; *used is the number of rand() calls made. The int results are returned as doubles. */
double ref_mu_pdf(int op, double lower, double upper, int ilower, int iupper, const int *seq, int nseq, int *used)
{
	g_seq = seq;
	g_nseq = nseq;
	g_used = 0;
	double r = 0;
	switch (op) {
	case 0: r = cMathUtil::Get_Uniform_Pdf_Double(lower, upper); break;
	case 1: r = cMathUtil::Get_Normalized_Uniform_Pdf_Double(); break;
	case 2: r = (double)cMathUtil::Get_Uniform_Pdf_Int(ilower, iupper); break;
	case 3: r = cMathUtil::Get_Hat_Pdf_Double(lower, upper); break;
	case 4: r = cMathUtil::Get_Normalized_Hat_Pdf_Double(); break;
	case 5: r = (double)cMathUtil::Get_Hat_Pdf_Int(ilower, iupper); break;
	}
	*used = g_used;
	return r;
}

void ref_mu_seconds_to_hms(float seconds, int *out)
{
	int h = 0, m = 0, s = 0;
	cMiscUtil::Seconds_To_Hms(seconds, h, m, s);
	out[0] = h;
	out[1] = m;
	out[2] = s;
}

int ref_mu_string_cmp(const char *a, const char *b) { return (cMiscUtil::Is_String_Same(a, b) ? 1 : 0) | (cMiscUtil::Is_String_Different(a, b) ? 2 : 0); }

int ref_mu_char_class(char c)
{
	return (cMiscUtil::Is_Alphabetic(c) ? 1 : 0) | (cMiscUtil::Is_Numeric(c) ? 2 : 0) | (cMiscUtil::Is_Alphanumeric(c) ? 4 : 0) | (cMiscUtil::Is_Whitespace(c) ? 8 : 0);
}

/* Trim_Trailing_Whitespace on `text` (NUL-terminated, in place); returns the new length */
int ref_mu_trim(char *text)
{
	cMiscUtil::Trim_Trailing_Whitespace(text);
	return (int)strlen(text);
}

}
