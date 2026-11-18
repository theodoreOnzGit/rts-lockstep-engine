//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/lcw.cpp
//!
//! Not translated: `LCW_Comp` is `#if defined(_MSC_VER)` x86 inline assembly (`__asm`), a compressor only; nothing in
//! the game's load paths needs it.
//!
//! Deviation, noted: in the 0xFE long-run opcode upstream aligns the destination pointer to 4 bytes using the real
//! machine address and fills by words; the bytes written are simply `count` copies of `data`, which is what this does.
//! (If `count` is smaller than the alignment pad upstream underflows `count` and runs off the buffer; not reproduced.)

/// `LCW_Uncomp(source, dest, length)` (lcw.cpp:42): returns the number of destination bytes written. Slice bounds
/// replace the unchecked pointer arithmetic (a malformed stream panics instead of corrupting memory).
pub fn lcw_uncomp(source: &[u8], dest: &mut [u8]) -> usize {
    let mut source_ptr = 0usize;
    let mut dest_ptr = 0usize;

    loop {
        // Read in the operation code.
        let op_code = source[source_ptr];
        source_ptr += 1;

        if op_code & 0x80 == 0 {
            // Do a short copy from destination.
            let mut count = (op_code >> 4) as usize + 3;
            let mut copy_ptr = dest_ptr - (source[source_ptr] as usize + (((op_code & 0x0f) as usize) << 8));
            source_ptr += 1;
            while count > 0 {
                dest[dest_ptr] = dest[copy_ptr];
                dest_ptr += 1;
                copy_ptr += 1;
                count -= 1;
            }
        } else if op_code & 0x40 == 0 {
            if op_code == 0x80 {
                // Return # of destination bytes written.
                return dest_ptr;
            }
            // Do a medium copy from source.
            let mut count = (op_code & 0x3f) as usize;
            while count > 0 {
                dest[dest_ptr] = source[source_ptr];
                dest_ptr += 1;
                source_ptr += 1;
                count -= 1;
            }
        } else if op_code == 0xfe {
            // Do a long run.
            let count = source[source_ptr] as usize + ((source[source_ptr + 1] as usize) << 8);
            let data = source[source_ptr + 2];
            source_ptr += 3;
            for b in &mut dest[dest_ptr..dest_ptr + count] {
                *b = data;
            }
            dest_ptr += count;
        } else if op_code == 0xff {
            // Do a long copy from destination (absolute offset from the start of dest).
            let mut count = source[source_ptr] as usize + ((source[source_ptr + 1] as usize) << 8);
            let mut copy_ptr = source[source_ptr + 2] as usize + ((source[source_ptr + 3] as usize) << 8);
            source_ptr += 4;
            while count > 0 {
                dest[dest_ptr] = dest[copy_ptr];
                dest_ptr += 1;
                copy_ptr += 1;
                count -= 1;
            }
        } else {
            // Do a medium copy from destination (absolute offset from the start of dest).
            let mut count = (op_code & 0x3f) as usize + 3;
            let mut copy_ptr = source[source_ptr] as usize + ((source[source_ptr + 1] as usize) << 8);
            source_ptr += 2;
            while count > 0 {
                dest[dest_ptr] = dest[copy_ptr];
                dest_ptr += 1;
                copy_ptr += 1;
                count -= 1;
            }
        }
    }
}
