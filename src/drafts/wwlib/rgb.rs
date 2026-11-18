//! UNTESTED DRAFT (#202, #209): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/RGB.H
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/rgb.cpp
//!
//! `RGBClass`: an 8-bit-per-channel colour, with `Adjust`, `Difference` and the conversion to `HSVClass`
//! (`RGBClass::operator HSVClass`, rgb.cpp:105). `PaletteClass` (a friend upstream) is a Windows palette type and not
//! translated. `BlackColor` is [`BLACK_COLOR`].

use super::hsv::HSVClass;

/// `extern RGBClass const BlackColor;` (all channels 0).
pub const BLACK_COLOR: RGBClass = RGBClass { red: 0, green: 0, blue: 0 };

/// `class RGBClass`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RGBClass {
    red: u8,
    green: u8,
    blue: u8,
}

impl RGBClass {
    /// `MAX_VALUE = 255`.
    pub const MAX_VALUE: i32 = 255;

    /// `RGBClass(red, green, blue)`.
    pub fn new(red: u8, green: u8, blue: u8) -> Self {
        RGBClass { red, green, blue }
    }

    /// `Get_Red()`.
    pub fn get_red(&self) -> i32 {
        self.red as i32
    }
    /// `Get_Green()`.
    pub fn get_green(&self) -> i32 {
        self.green as i32
    }
    /// `Get_Blue()`.
    pub fn get_blue(&self) -> i32 {
        self.blue as i32
    }
    /// `Set_Red(value)`.
    pub fn set_red(&mut self, value: u8) {
        self.red = value;
    }
    /// `Set_Green(value)`.
    pub fn set_green(&mut self, value: u8) {
        self.green = value;
    }
    /// `Set_Blue(value)`.
    pub fn set_blue(&mut self, value: u8) {
        self.blue = value;
    }

    /// `RGBClass::Adjust(ratio, rgb)` (rgb.cpp:48): moves each channel toward `rgb` by `ratio`/256 (`ratio &= 0xFF`).
    pub fn adjust(&mut self, ratio: i32, rgb: &RGBClass) {
        // Adjust the color guns by the ratio specified toward the destination color.
        let ratio = ratio & 0x00FF;

        let mut value = rgb.red as i32 - self.red as i32;
        self.red = (self.red as i32 + (value * ratio) / 256) as u8;

        value = rgb.green as i32 - self.green as i32;
        self.green = (self.green as i32 + (value * ratio) / 256) as u8;

        value = rgb.blue as i32 - self.blue as i32;
        self.blue = (self.blue as i32 + (value * ratio) / 256) as u8;
    }

    /// `RGBClass::Difference(rgb)` (rgb.cpp:76): "return(4*g + 3*b + 2*r)" of the absolute channel differences.
    pub fn difference(&self, rgb: &RGBClass) -> i32 {
        let mut r = self.red as i32 - rgb.red as i32;
        if r < 0 {
            r = -r;
        }

        let mut g = self.green as i32 - rgb.green as i32;
        if g < 0 {
            g = -g;
        }

        let mut b = self.blue as i32 - rgb.blue as i32;
        if b < 0 {
            b = -b;
        }

        // Perform a weighted compare based on human sensitivity (green, blue, red)
        4 * g + 3 * b + 2 * r
    }

    /// `RGBClass::operator HSVClass()` (rgb.cpp:105).
    pub fn to_hsv(&self) -> HSVClass {
        let red = self.get_red();
        let green = self.get_green();
        let blue = self.get_blue();

        let mut hue: i32 = 0;

        // Value is the brightest primary
        let mut value = if red > green { red } else { green };
        if blue > value {
            value = blue;
        }

        // White is the dimmest primary
        let mut white = if red < green { red } else { green };
        if blue < white {
            white = blue;
        }

        // Saturation is the spread between them, relative to the value
        let mut saturation: i32 = 0;
        if value != 0 {
            saturation = ((value - white) * 255) / value;
        }

        // Hue is meaningless for a grey
        if saturation != 0 {
            let mut tmp: u32 = (value - white) as u32;
            let r1: u32 = (((value - red) * 255) as u32) / tmp;
            let g1: u32 = (((value - green) * 255) as u32) / tmp;
            let b1: u32 = (((value - blue) * 255) as u32) / tmp;

            // Find effect of second most predominant color. In which section of the hexagon of colors does the color lie?
            if value == red {
                if white == green {
                    tmp = 5 * 256 + b1;
                } else {
                    tmp = (1u32 * 256).wrapping_sub(g1);
                }
            } else if value == green {
                if white == blue {
                    tmp = 1 * 256 + r1;
                } else {
                    tmp = (3u32 * 256).wrapping_sub(b1);
                }
            } else if white == red {
                tmp = 3 * 256 + g1;
            } else {
                tmp = (5u32 * 256).wrapping_sub(r1);
            }

            // Divide by six and round.
            hue = (tmp / 6) as i32;
        }

        HSVClass::new(hue as u8, saturation as u8, value as u8)
    }
}
