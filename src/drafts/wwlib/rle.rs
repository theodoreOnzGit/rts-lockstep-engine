//! UNTESTED DRAFT (#202, #209): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/rle.cpp
//!
//! Upstream's zero-run scan is `while (sptr[runcount] == 0 && runcount <= length)`, which reads one byte past the input
//! when the data ends in zeros (undefined behaviour); here a read past the slice end counts as non-zero.

/// `RLEEngine::Compress` (rle.cpp:41). `dest = None` only measures (upstream's `dest == NULL`). Returns the output length.
pub fn rle_compress(source: &[u8], mut dest: Option<&mut Vec<u8>>, length: i32) -> i32 {
    debug_assert!(length > 0);
    let mut length = length;
    let mut outlen = 0i32;
    let mut sptr = 0usize;

    while length > 0 {
        // Examine each source byte. If the byte is zero (transparent), then it is recorded as a run.
        if source[sptr] == 0 {
            // Count the number of transparent pixels in this run.
            let mut runcount = 0i32;
            while runcount <= length && source.get(sptr + runcount as usize).copied() == Some(0) {
                runcount += 1;
            }
            // Limit the run to 255 characters maximum.
            runcount = runcount.min(255);
            if let Some(d) = dest.as_deref_mut() {
                d.push(0);
                d.push(runcount as u8);
            }
            outlen += 2;
            sptr += runcount as usize;
            length -= runcount;
        } else {
            // Store the raw byte without any translation.
            if let Some(d) = dest.as_deref_mut() {
                d.push(source[sptr]);
            }
            sptr += 1;
            outlen += 1;
            length -= 1;
        }
    }
    outlen
}

/// `RLEEngine::Line_Compress` (rle.cpp:107): a 16-bit length prefix then the data. With `dest = None` only measures.
pub fn rle_line_compress(source: &[u8], dest: Option<&mut Vec<u8>>, length: i32) -> i32 {
    debug_assert!(length > 0);
    if let Some(d) = dest {
        let start = d.len();
        d.extend_from_slice(&[0, 0]); // placeholder for `*sizeptr`
        let complen = rle_compress(source, Some(d), length) + 2;
        d[start..start + 2].copy_from_slice(&(complen as u16).to_le_bytes());
        return complen;
    }
    rle_compress(source, None, length) + 2
}

/// `RLEEngine::Decompress` (rle.cpp:131): returns the bytes stored into `dest`.
pub fn rle_decompress(source: &[u8], dest: &mut [u8], length: i32) -> i32 {
    debug_assert!(length > 0);
    let mut length = length;
    let mut dptr = 0usize;
    let mut sptr = 0usize;

    while length > 0 {
        // Detect if a zero-run code is present. If so, then dump the desired number of bytes.
        let value = source[sptr];
        sptr += 1;
        length -= 1;
        if value == 0 {
            let mut outlen = source[sptr] as i32;
            sptr += 1;
            length -= 1;
            while outlen > 0 {
                dest[dptr] = 0;
                dptr += 1;
                outlen -= 1;
            }
        } else {
            dest[dptr] = value;
            dptr += 1;
        }
    }
    dptr as i32
}

/// `RLEEngine::Line_Decompress` (rle.cpp:170): reads the 16-bit length prefix then decompresses.
pub fn rle_line_decompress(source: &[u8], dest: &mut [u8]) -> i32 {
    let datalen = u16::from_le_bytes([source[0], source[1]]) as i32;
    rle_decompress(&source[2..], dest, datalen - 2)
}
