// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/colorspace.h
//   (RGB_To_HSV, HSV_To_RGB, both Recolor overloads)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, Colorspace, by Hector Yee)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The hue/saturation/value shift textures are loaded with (house colours
//! and the like): `Recolor`, in upstream's `float` operation order.
//!
//! Upstream quirks, kept: the hue is wrapped by one step of 360 only, so a
//! shift of more than a turn leaves it outside `[0, 360]`, and then
//! `HSV_To_RGB`'s sector switch matches no case and leaves the colour as it
//! was (the shift has no effect, not even on the value). A colour with no
//! saturation has "undefined" hue -1 and only its value is shifted.

use crate::formats::w3d::mesh::{dx8_convert_color, dx8_unpack_color};
use crate::ww::wwmath;

/// The colour space conversion went outside what upstream defines: the
/// sector `int(floor(h / 60))` of a hue that is NaN or outside `int`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HueOutOfRange;

/// `RGB_To_HSV(hsv, rgb)`: `rgb` in `[0, 1]` to `(h, s, v)`, `h` in
/// `[0, 360)` or -1 when undefined.
pub fn rgb_to_hsv(rgb: [f32; 3]) -> [f32; 3] {
    let mut max = wwmath::max(rgb[0], rgb[1]);
    max = wwmath::max(max, rgb[2]);
    let mut min = wwmath::min(rgb[0], rgb[1]);
    min = wwmath::min(min, rgb[2]);
    let z = max;
    let y = if max != 0.0 { (max - min) / max } else { 0.0 };
    // hsv.X is left as it was when no branch matches (a NaN maximum);
    // `Recolor` passes an uninitialised vector, but its colours come from
    // bytes and are never NaN.
    let mut x = 0.0f32;
    if y == 0.0 {
        x = -1.0;
    } else {
        let delta = max - min;
        if rgb[0] == max {
            x = (rgb[1] - rgb[2]) / delta;
        } else if rgb[1] == max {
            x = 2.0 + (rgb[2] - rgb[0]) / delta;
        } else if rgb[2] == max {
            x = 4.0 + (rgb[0] - rgb[1]) / delta;
        }
        x *= 60.0;
        if x < 0.0 {
            x += 360.0;
        }
    }
    [x, y, z]
}

/// `HSV_To_RGB(rgb, hsv)`: writes `rgb`, except when the sector is not 0..5
/// (then `rgb` keeps its value).
pub fn hsv_to_rgb(rgb: &mut [f32; 3], hsv: [f32; 3]) -> Result<(), HueOutOfRange> {
    let [mut h, s, v] = hsv;
    if hsv[1] == 0.0 {
        *rgb = [v, v, v];
        return Ok(());
    }
    if h == 360.0 {
        h = 0.0;
    }
    h /= 60.0;
    let fl = wwmath::floor(h);
    // `i = WWMath::Floor(h)`: float to int, undefined outside int
    if fl.is_nan() || !(-2147483648.0..2147483648.0).contains(&fl) {
        return Err(HueOutOfRange);
    }
    let i = fl as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - (s * f));
    let t = v * (1.0 - (s * (1.0 - f)));
    match i {
        0 => *rgb = [v, t, p],
        1 => *rgb = [q, v, p],
        2 => *rgb = [p, v, t],
        3 => *rgb = [p, q, v],
        4 => *rgb = [t, p, v],
        5 => *rgb = [v, p, q],
        _ => {}
    }
    Ok(())
}

/// `Recolor(Vector3 &rgb, hsv_shift)`.
pub fn recolor_rgb(rgb: &mut [f32; 3], hsv_shift: [f32; 3]) -> Result<(), HueOutOfRange> {
    let mut hsv = rgb_to_hsv(*rgb);
    if hsv[0] < 0.0 {
        hsv[2] += hsv_shift[2];
        // `hsv += Vector3(0, 0, shift.Z)` also adds 0 to X and Y, which
        // only matters for -0.0 (not reachable: X is -1, Y is +0)
        hsv[0] += 0.0;
        hsv[1] += 0.0;
    } else {
        hsv[0] += hsv_shift[0];
        hsv[1] += hsv_shift[1];
        hsv[2] += hsv_shift[2];
    }
    if hsv[0] < 0.0 {
        hsv[0] += 360.0;
    }
    if hsv[0] > 360.0 {
        hsv[0] -= 360.0;
    }
    hsv[1] = wwmath::clamp(hsv[1], 0.0, 1.0);
    hsv[2] = wwmath::clamp(hsv[2], 0.0, 1.0);
    hsv_to_rgb(rgb, hsv)
}

/// `Recolor(unsigned &rgba, hsv_shift)`: through `DX8Wrapper::Convert_Color`
/// both ways (channels over 255, back with x87 truncation).
pub fn recolor(rgba: u32, hsv_shift: [f32; 3]) -> Result<u32, HueOutOfRange> {
    let mut v = dx8_unpack_color(rgba);
    let mut rgb = [v[0], v[1], v[2]];
    recolor_rgb(&mut rgb, hsv_shift)?;
    v[0] = rgb[0];
    v[1] = rgb[1];
    v[2] = rgb[2];
    Ok(dx8_convert_color(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_shift_round_trip() {
        // channels 0 and 255 survive the float round trip; others can lose
        // one in the truncation (upstream only recolours with a shift)
        for c in [0u32, 0xFF00_0000, 0x00FF_0000, 0x0000_FF00, 0xFFFF_FFFF] {
            assert_eq!(recolor(c, [0.0, 0.0, 0.0]), Ok(c), "{c:08x}");
        }
        assert_eq!(recolor(0x0012_3456, [0.0, 0.0, 0.0]), Ok(0x0011_3456));
    }

    #[test]
    fn hue_past_a_turn_is_left_alone() {
        // red (h = 0) shifted by 400: 400 > 360 -> 40, sector 0: changes;
        // shifted by 800: 800 - 360 = 440, sector 7: the colour is unchanged
        assert_ne!(recolor(0x00FF_0000, [400.0, 0.0, 0.0]), Ok(0x00FF_0000));
        assert_eq!(recolor(0x00FF_0000, [800.0, 0.0, -0.5]), Ok(0x00FF_0000));
        assert_eq!(recolor(0x00FF_0000, [f32::NAN, 0.0, 0.0]), Err(HueOutOfRange));
    }
}
