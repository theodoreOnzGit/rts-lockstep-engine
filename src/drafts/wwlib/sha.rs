//! UNTESTED DRAFT (#202, #209): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/sha.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/sha.cpp
//!
//! `SHAEngine`: SHA-1 over 16-word blocks. Upstream notes kept: `long` is 32 bits; the message length in bits is stored
//! as the LAST 32-bit word only (`Reverse_LONG(length*8)` written at offset 60, the high word left zero), so inputs over
//! 512 MiB wrap; `Result` has an `IsCached` early `memcpy` that does not `return` (it recomputes and overwrites with the
//! same value); `Hash` takes the length as a `long`. `Reverse_LONG` is a byte swap; `_rotl` a 32-bit rotate. The digest
//! words are byte-swapped on output so the result is the usual big-endian SHA-1.

const SA: u32 = 0x67452301;
const SB: u32 = 0xefcdab89;
const SC: u32 = 0x98badcfe;
const SD: u32 = 0x10325476;
const SE: u32 = 0xc3d2e1f0;
const K1: u32 = 0x5a827999;
const K2: u32 = 0x6ed9eba1;
const K3: u32 = 0x8f1bbcdc;
const K4: u32 = 0xca62c1d6;
/// `SRC_BLOCK_SIZE = 16*sizeof(long)`.
const SRC_BLOCK_SIZE: usize = 64;

/// `class SHAEngine`.
#[derive(Clone, Debug)]
pub struct SHAEngine {
    is_cached: bool,
    final_result: [u32; 5],
    acc: [u32; 5],
    length: i32,
    partial_count: usize,
    partial: [u8; SRC_BLOCK_SIZE],
}

impl SHAEngine {
    /// `SHAEngine()`: the five seeds; `IsCached(false)`, `Length(0)`, `PartialCount(0)` (`Partial` is uninitialised upstream).
    pub fn new() -> Self {
        SHAEngine { is_cached: false, final_result: [0; 5], acc: [SA, SB, SC, SD, SE], length: 0, partial_count: 0, partial: [0; SRC_BLOCK_SIZE] }
    }

    /// `Init()`: re-construct in place.
    pub fn init(&mut self) {
        *self = SHAEngine::new();
    }

    /// `Digest_Size()`: `sizeof(SHADigest)` = 20.
    pub fn digest_size() -> i32 {
        20
    }

    /// `Get_Constant(index)`.
    fn get_constant(index: usize) -> u32 {
        if index < 20 {
            return K1;
        }
        if index < 40 {
            return K2;
        }
        if index < 60 {
            return K3;
        }
        K4
    }

    /// `Do_Function(index, X, Y, Z)` (`Function1` .. `Function4`).
    fn do_function(index: usize, x: u32, y: u32, z: u32) -> u32 {
        if index < 20 {
            return z ^ (x & (y ^ z));
        }
        if index < 40 {
            return x ^ y ^ z;
        }
        if index < 60 {
            return (x & y) | (z & (x | y));
        }
        x ^ y ^ z
    }

    /// `Process_Block(source, acc)`: one 64-byte block; the 16 words are byte-reversed (the source is big-endian).
    fn process_block(source: &[u8], acc: &mut [u32; 5]) {
        let mut block = [0u32; 80];
        for index in 0..16 {
            // Reverse_LONG(data[index]) of a little-endian load is a big-endian load
            block[index] = u32::from_be_bytes([source[index * 4], source[index * 4 + 1], source[index * 4 + 2], source[index * 4 + 3]]);
        }
        for index in 16..80 {
            block[index] = (block[index - 3] ^ block[index - 8] ^ block[index - 14] ^ block[index - 16]).rotate_left(1);
        }

        let mut alt = *acc;
        for index in 0..80 {
            let temp = alt[0].rotate_left(5).wrapping_add(Self::do_function(index, alt[1], alt[2], alt[3])).wrapping_add(alt[4]).wrapping_add(block[index]).wrapping_add(Self::get_constant(index));
            alt[4] = alt[3];
            alt[3] = alt[2];
            alt[2] = alt[1].rotate_left(30);
            alt[1] = alt[0];
            alt[0] = temp;
        }

        acc[0] = acc[0].wrapping_add(alt[0]);
        acc[1] = acc[1].wrapping_add(alt[1]);
        acc[2] = acc[2].wrapping_add(alt[2]);
        acc[3] = acc[3].wrapping_add(alt[3]);
        acc[4] = acc[4].wrapping_add(alt[4]);
    }

    /// `Process_Partial(data, length)`: tops up the partial block; "if (PartialCount == 0 && length >= SRC_BLOCK_SIZE)
    /// return" lets `Hash` process whole blocks directly. Advances `data`.
    fn process_partial(&mut self, data: &mut &[u8], length: &mut i32) {
        if *length == 0 || data.is_empty() {
            return;
        }

        // If there is no partial block and the data is at least one block, the caller does whole blocks.
        if self.partial_count == 0 && *length >= SRC_BLOCK_SIZE as i32 {
            return;
        }

        // Add as much data to the partial buffer as fits
        let add_count = std::cmp::min(*length, (SRC_BLOCK_SIZE - self.partial_count) as i32) as usize;
        self.partial[self.partial_count..self.partial_count + add_count].copy_from_slice(&data[..add_count]);
        *data = &data[add_count..];
        self.partial_count += add_count;
        *length -= add_count as i32;

        // If the partial block is full, process it
        if self.partial_count == SRC_BLOCK_SIZE {
            let p = self.partial;
            Self::process_block(&p, &mut self.acc);
            self.length = self.length.wrapping_add(SRC_BLOCK_SIZE as i32);
            self.partial_count = 0;
        }
    }

    /// `Hash(data, length)` (the whole slice is hashed; `length` is its length).
    pub fn hash(&mut self, data: &[u8]) {
        self.is_cached = false;
        let mut data = data;
        let mut length = data.len() as i32;

        // Fill up any partial block first
        self.process_partial(&mut data, &mut length);

        // If there is no more data, we are done
        if length == 0 {
            return;
        }

        // Process whole blocks directly from the source
        let blocks = length / SRC_BLOCK_SIZE as i32;
        for _ in 0..blocks {
            Self::process_block(&data[..SRC_BLOCK_SIZE], &mut self.acc);
            self.length = self.length.wrapping_add(SRC_BLOCK_SIZE as i32);
            data = &data[SRC_BLOCK_SIZE..];
            length -= SRC_BLOCK_SIZE as i32;
        }

        // Buffer the remainder
        self.process_partial(&mut data, &mut length);
    }

    /// `Result(result)`: the digest "as if source data were to stop now" (20 bytes). Does not disturb the running hash.
    pub fn result(&mut self) -> [u8; 20] {
        // (if IsCached the cached value is copied, then upstream falls through and recomputes the same value)
        let length = self.length.wrapping_add(self.partial_count as i32);
        let mut partialcount = self.partial_count;
        let mut partial = self.partial;

        // Add the 0x80 terminator
        partial[partialcount] = 0x80;

        let mut acc = self.acc;
        if (SRC_BLOCK_SIZE - partialcount) < 9 {
            if partialcount + 1 < SRC_BLOCK_SIZE {
                for b in partial[partialcount + 1..].iter_mut() {
                    *b = 0;
                }
            }
            Self::process_block(&partial, &mut acc);
            partialcount = 0;
        } else {
            partialcount += 1;
        }

        // Zero fill the rest and put the bit length in the last word (byte-reversed, i.e. big-endian)
        for b in partial[partialcount..].iter_mut() {
            *b = 0;
        }
        partial[SRC_BLOCK_SIZE - 4..].copy_from_slice(&(length.wrapping_mul(8) as u32).to_be_bytes());
        Self::process_block(&partial, &mut acc);

        // Reverse_LONG each word of the result
        for index in 0..5 {
            self.final_result[index] = acc[index].swap_bytes();
        }
        self.is_cached = true;

        // memcpy(result, &FinalResult, 20): the words in memory order (little-endian host)
        let mut out = [0u8; 20];
        for index in 0..5 {
            out[index * 4..index * 4 + 4].copy_from_slice(&self.final_result[index].to_le_bytes());
        }
        out
    }
}

impl Default for SHAEngine {
    fn default() -> Self {
        SHAEngine::new()
    }
}
