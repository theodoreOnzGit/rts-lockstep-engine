/* (#215, group wwlib3) stand-in for wwstring.h: what widestring.h needs of StringClass (its Convert_To calls Copy_Wide) and the
** FastCriticalSectionClass the temp-string pool locks (single-threaded here, no lock). Copy_Wide is the upstream function
** itself, cut verbatim by build.rs (wwstring_copy_wide.inc, included by the wrapper). */
#ifndef WWLIB3_WWSTRING_H
#define WWLIB3_WWSTRING_H
#include <string>
class FastCriticalSectionClass { public: class LockClass { public: LockClass(FastCriticalSectionClass &) {} }; };
class StringClass {
public:
	std::string s;
	bool Copy_Wide(const wchar_t *source);
	char *Get_Buffer(int n) { s.assign((size_t)n, '\0'); return &s[0]; }
	void Store_Length(int n) { s.resize((size_t)n); }
};
#endif
