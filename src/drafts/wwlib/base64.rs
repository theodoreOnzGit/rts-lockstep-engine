//! UNTESTED DRAFT (#202, #209): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/base64.cpp
//!
//! The `PacketType` union (little-endian layout: `Char.C3` byte 0, `C2` byte 1, `C1` byte 2; `SubCode.O4` bits 0-5,
//! `O3` 6-11, `O2` 12-17, `O1` 18-23) is a `u32` with shifts. The `_decoder[256]` table is `decoder()`, giving the same
//! values (A-Z 0-25, a-z 26-51, 0-9 52-61, '+' 62, '/' 63, '=' END, everything else BAD).

/// "Ignore this character in source data."
const BAD: u8 = 0xFE;
/// "Signifies premature end of input data."
const END: u8 = 0xFF;
const PACKET_CHARS: i32 = 4;
const PAD: u8 = b'=';
const ENCODER: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `_decoder[c]`.
fn decoder(c: u8) -> u8 {
    match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        b'=' => END,
        _ => BAD,
    }
}

/// `Base64_Encode(source, slen, dest, dlen)` (base64.cpp:110): returns the characters written to `dest`.
pub fn base64_encode(source: &[u8], slen: i32, dest: &mut [u8], dlen: i32) -> i32 {
    if source.is_empty() || slen == 0 || dest.is_empty() || dlen == 0 {
        return 0;
    }
    let (mut slen, mut dlen) = (slen, dlen);
    let mut total = 0i32;
    let mut sptr = 0usize;
    let mut dptr = 0usize;

    while slen > 0 && dlen >= PACKET_CHARS {
        // Fetch 24 bits of source data.
        let mut raw: u32 = 0;
        let mut pad = 0;
        raw |= (source[sptr] as u32) << 16; // C1
        sptr += 1;
        slen -= 1;
        if slen != 0 {
            raw |= (source[sptr] as u32) << 8; // C2
            sptr += 1;
            slen -= 1;
        } else {
            pad += 1;
        }
        if slen != 0 {
            raw |= source[sptr] as u32; // C3
            sptr += 1;
            slen -= 1;
        } else {
            pad += 1;
        }

        // Translate and write 4 characters of Base64 data.
        let (o1, o2, o3, o4) = ((raw >> 18) & 63, (raw >> 12) & 63, (raw >> 6) & 63, raw & 63);
        dest[dptr] = ENCODER[o1 as usize];
        dptr += 1;
        dest[dptr] = ENCODER[o2 as usize];
        dptr += 1;
        if pad < 2 {
            dest[dptr] = ENCODER[o3 as usize];
        } else {
            dest[dptr] = PAD;
        }
        dptr += 1;
        if pad < 1 {
            dest[dptr] = ENCODER[o4 as usize];
        } else {
            dest[dptr] = PAD;
        }
        dptr += 1;

        dlen -= PACKET_CHARS;
        total += PACKET_CHARS;
    }

    // Add a trailing null as a courtesy measure.
    if dlen > 0 {
        dest[dptr] = 0;
    }
    total
}

/// `Base64_Decode(source, slen, dest, dlen)` (base64.cpp:198): returns the bytes written to `dest`.
pub fn base64_decode(source: &[u8], slen: i32, dest: &mut [u8], dlen: i32) -> i32 {
    if source.is_empty() || slen == 0 || dest.is_empty() || dlen == 0 {
        return 0;
    }
    let (mut slen, mut dlen) = (slen, dlen);
    let mut total = 0i32;
    let mut sptr = 0usize;
    let mut dptr = 0usize;

    while slen > 0 && dlen > 0 {
        let mut o = [0u32; 4]; // O1..O4 of the packet (Raw starts at 0)

        // Process input until a full packet has been accumulated or the source is exhausted.
        let mut pcount = 0usize;
        while pcount < PACKET_CHARS as usize && slen > 0 {
            let c = source[sptr];
            sptr += 1;
            slen -= 1;
            let code = decoder(c);

            // An unrecognized character is skipped.
            if code == BAD {
                continue;
            }
            // The "=" character signifies the end of data regardless of the source length.
            if code == END {
                slen = 0;
                break;
            }
            o[pcount] = code as u32;
            pcount += 1;
        }

        // A packet block is ready for output into the destination buffer.
        let raw: u32 = (o[0] << 18) | (o[1] << 12) | (o[2] << 6) | o[3];
        let (c1, c2, c3) = (((raw >> 16) & 0xff) as u8, ((raw >> 8) & 0xff) as u8, (raw & 0xff) as u8);
        dest[dptr] = c1;
        dptr += 1;
        dlen -= 1;
        total += 1;
        if dlen > 0 && pcount > 2 {
            dest[dptr] = c2;
            dptr += 1;
            dlen -= 1;
            total += 1;
        }
        if dlen > 0 && pcount > 3 {
            dest[dptr] = c3;
            dptr += 1;
            dlen -= 1;
            total += 1;
        }
    }
    total
}
