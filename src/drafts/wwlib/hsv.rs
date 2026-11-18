//! UNTESTED DRAFT (#202, #209): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/hsv.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/hsv.cpp
//!
//! `HSVClass`: hue, saturation, value as bytes, `Adjust`, `Difference` and the conversion to `RGBClass`
//! (`HSVClass::operator RGBClass`, hsv.cpp:84). `HSVClass::BlackColor` is [`HSV_BLACK_COLOR`]. `HSV_BASE`-rounded
//! divides are as upstream's comments describe (`values[3] = (v * (255 - ((s*f)/255))) / 255` etc.).

use super::rgb::RGBClass;

/// `HSVClass::BlackColor` (all 0).
pub const HSV_BLACK_COLOR: HSVClass = HSVClass { hue: 0, saturation: 0, value: 0 };

/// `class HSVClass`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HSVClass {
    hue: u8,
    saturation: u8,
    value: u8,
}

impl HSVClass {
    /// `MAX_VALUE = 255`.
    pub const MAX_VALUE: i32 = 255;

    /// `HSVClass(hue, saturation, value)`.
    pub fn new(hue: u8, saturation: u8, value: u8) -> Self {
        HSVClass { hue, saturation, value }
    }

    /// `Get_Hue()`.
    pub fn get_hue(&self) -> i32 {
        self.hue as i32
    }
    /// `Get_Saturation()`.
    pub fn get_saturation(&self) -> i32 {
        self.saturation as i32
    }
    /// `Get_Value()`.
    pub fn get_value(&self) -> i32 {
        self.value as i32
    }
    /// `Set_Hue(value)`.
    pub fn set_hue(&mut self, value: u8) {
        self.hue = value;
    }
    /// `Set_Saturation(value)`.
    pub fn set_saturation(&mut self, value: u8) {
        self.saturation = value;
    }
    /// `Set_Value(value)`.
    pub fn set_value(&mut self, value: u8) {
        self.value = value;
    }

    /// `HSVClass::Adjust(ratio, hsv)` (hsv.cpp:46): each of value, saturation, hue moves toward `hsv` by `ratio`/256.
    pub fn adjust(&mut self, ratio: i32, hsv: &HSVClass) {
        // Ratio conversion.
        let ratio = ratio & 0x00FF;

        // Adjust the color guns by the ratio specified toward the destination color.
        let value = hsv.get_value() - self.get_value();
        self.value = (self.get_value() + (value * ratio) / 256) as u8;

        let saturation = hsv.get_saturation() - self.get_saturation();
        self.saturation = (self.get_saturation() + (saturation * ratio) / 256) as u8;

        let hue = hsv.get_hue() - self.get_hue();
        self.hue = (self.get_hue() + (hue * ratio) / 256) as u8;
    }

    /// `HSVClass::Difference(hsv)` (hsv.cpp:71): the sum of the squared channel differences.
    pub fn difference(&self, hsv: &HSVClass) -> i32 {
        let mut hue = self.hue as i32 - hsv.hue as i32;
        if hue < 0 {
            hue = -hue;
        }

        let mut saturation = self.saturation as i32 - hsv.saturation as i32;
        if saturation < 0 {
            saturation = -saturation;
        }

        let mut value = self.value as i32 - hsv.value as i32;
        if value < 0 {
            value = -value;
        }

        hue * hue + saturation * saturation + value * value
    }

    /// `HSVClass::operator RGBClass()` (hsv.cpp:84): the six-sector hexagon conversion with rounded divides.
    pub fn to_rgb(&self) -> RGBClass {
        let mut values = [0u32; 7]; // Possible rgb values.  Don't use zero.

        let mut hue = self.get_hue();
        let saturation = self.get_saturation();
        let value = self.get_value();

        hue *= 6;
        let f: u32 = (hue % 255) as u32; // fractional or remainder part

        // Set up possible red, green and blue values.
        values[1] = value as u32;
        values[2] = value as u32;

        // (rounded divides)
        let mut tmp: u32 = ((saturation as u32) * f) / 255;
        values[3] = ((value as u32) * (255 - tmp)) / 255;

        values[4] = ((value as u32) * (255 - saturation as u32)) / 255;
        values[5] = values[4];

        tmp = 255 - ((saturation as u32) * (255 - f)) / 255;
        values[6] = ((value as u32) * tmp) / 255;

        // This should not be rounded.
        let mut i: u32 = (hue / 255) as u32;
        i = i.wrapping_add(if i > 4 { (-4i32) as u32 } else { 2 });
        let red = values[i as usize];

        i = i.wrapping_add(if i > 4 { (-4i32) as u32 } else { 2 });
        let blue = values[i as usize];

        i = i.wrapping_add(if i > 4 { (-4i32) as u32 } else { 2 });
        let green = values[i as usize];

        RGBClass::new(red as u8, green as u8, blue as u8)
    }
}
