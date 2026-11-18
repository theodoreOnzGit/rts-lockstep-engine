//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/TerrainTex.cpp
//!
//! Sweep 2 (#213): `TerrainTextureClass::updateFlat` and `AlphaEdgeTextureClass::update256`. The D3D surface lock and the
//! `D3DXFilterTexture` mip filter are device-only; the pixel conversion is drafted on a byte buffer.

use rts_lockstep_engine::client::terrain_height_map::TILE_BYTES_PER_PIXEL;

/// `TerrainTextureClass::updateFlat` (TerrainTex.cpp:375): fills a `cellWidth * pixelsPerCell` square A1R5G5B5 surface
/// (`surface`, `width` pixels per row, 2 bytes per pixel) with the terrain tiles of the `cellWidth` x `cellWidth` cells
/// from (`xCell`, `yCell`), each cell `pixelsPerCell` pixels square, rows flipped (cell row 0 at the bottom). A cell past
/// the defined terrain (`get_pointer_to_tile_data` None) is skipped. Returns the surface height as upstream does
/// (`return(surface_desc.Height)`, non-zero = success), or 0 when the width does not match (`false`).
/// The caller then filters the mips (`D3DXFilterTexture(.., D3DX_FILTER_BOX)`, device-only).
pub fn update_flat(
    surface: &mut [u8],
    width: i32,
    height: i32,
    x_cell: i32,
    y_cell: i32,
    cell_width: i32,
    pixels_per_cell: i32,
    get_pointer_to_tile_data: &impl Fn(i32, i32, i32) -> Option<Vec<u8>>,
) -> i32 {
    if width != cell_width * pixels_per_cell {
        return 0; // Bitmap too small.
    }
    let pixel_bytes = 2;
    for cell_x in 0..cell_width {
        for cell_y in 0..cell_width {
            let bgr = match get_pointer_to_tile_data(x_cell + cell_x, y_cell + cell_y, pixels_per_cell) {
                Some(b) => b,
                None => continue, // past end of defined terrain. [3/24/2003]
            };
            let mut src = 0usize;
            let mut k = pixels_per_cell - 1;
            while k >= 0 {
                let mut dst = ((pixels_per_cell * (cell_width - cell_y - 1) + k) * width * pixel_bytes + cell_x * pixels_per_cell * pixel_bytes) as usize;
                for _l in 0..pixels_per_cell {
                    let v: u16 = 0x8000 + (((bgr[src + 2] >> 3) as u16) << 10) + (((bgr[src + 1] >> 3) as u16) << 5) + ((bgr[src] >> 3) as u16);
                    surface[dst..dst + 2].copy_from_slice(&v.to_le_bytes());
                    dst += pixel_bytes as usize;
                    src += TILE_BYTES_PER_PIXEL;
                }
                k -= 1;
            }
        }
    }
    height
}

/// `AlphaEdgeTextureClass::update256` (TerrainTex.cpp:764): a no-op returning 1.
pub fn alpha_edge_update256() -> i32 {
    1
}
