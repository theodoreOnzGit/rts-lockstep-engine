/* Differential-test wrapper for the map-transfer path helpers of the file transfer draft (#215, upstream commit 0a05454d),
** src/drafts/sweep_gui_logic/file_transfer.rs (get_base_path_from_path, get_file_from_path, get_extension_from_file,
** get_base_file_from_file, get_preview_from_map, get_ini_from_map, get_str_file_from_map, get_solo_ini_from_map,
** get_asset_usage_from_map, get_readme_from_map):
**   GeneralsMD/Code/GameEngine/Source/GameNetwork/FileTransfer.cpp from `AsciiString GetBasePathFromPath( AsciiString path )` to
**   `Bool DoAnyMapTransfers(GameInfo *game)`, cut verbatim by build.rs into ft_paths.inc.
** Built on the REAL AsciiString.h and AsciiString.cpp (shim_ascii, as the user preferences build; classes renamed by build.rs).
** Stand-ins (outside the cut): the allocator pointer only.
**
** Entry point (extern "C", tests/differential_draft_ft_paths.rs):
**   ref_ft_path(which, path, out, cap)  which 0 GetBasePathFromPath, 1 GetFileFromPath, 2 GetExtensionFromFile,
**       3 GetBaseFileFromFile, 4 GetPreviewFromMap, 5 GetINIFromMap, 6 GetStrFileFromMap, 7 GetSoloINIFromMap,
**       8 GetAssetUsageFromMap, 9 GetReadmeFromMap; writes the result (NUL-terminated) to out and returns its length (-1: too small). */
#include <string.h>

DynamicMemoryAllocator *TheDynamicMemoryAllocator = 0;
static DynamicMemoryAllocator theFtPathsAllocator;

#include "ft_paths.inc"

extern "C" int ref_ft_path(int which, const char *path, char *out, int cap)
{
	TheDynamicMemoryAllocator = &theFtPathsAllocator;
	AsciiString p(path);
	AsciiString r;
	switch (which)
	{
		case 0: r = GetBasePathFromPath(p); break;
		case 1: r = GetFileFromPath(p); break;
		case 2: r = GetExtensionFromFile(p); break;
		case 3: r = GetBaseFileFromFile(p); break;
		case 4: r = GetPreviewFromMap(p); break;
		case 5: r = GetINIFromMap(p); break;
		case 6: r = GetStrFileFromMap(p); break;
		case 7: r = GetSoloINIFromMap(p); break;
		case 8: r = GetAssetUsageFromMap(p); break;
		default: r = GetReadmeFromMap(p); break;
	}
	int n = (int)strlen(r.str());
	if (n + 1 > cap) return -1;
	memcpy(out, r.str(), n + 1);
	return n;
}
