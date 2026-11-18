//! UNTESTED DRAFT (#202, #209): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/md5.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/md5.cpp
//!
//! RSA Data Security, Inc. MD5 Message-Digest Algorithm (RFC 1321 reference code, "derived from the RSA Data Security,
//! Inc. MD5 Message-Digest Algorithm"; the notice in md5.cpp must be retained in copies of any part of it). The 64
//! `FF`/`GG`/`HH`/`II` steps are the same steps in a loop over the constant table, each round using the same word
//! order (round 1: i; round 2: (1+5i) mod 16; round 3: (5+3i) mod 16; round 4: 7i mod 16) and the `S11..S44` shifts.
//! `MD5_memcpy` / `MD5_memset` are slice copies; `unsigned int` counts are 32-bit and wrap as in C.

/// `MD5_CTX`.
#[derive(Clone, Copy, Debug)]
pub struct MD5Ctx {
    /// "state (ABCD)".
    pub state: [u32; 4],
    /// "number of bits, modulo 2^64 (lsb first)".
    pub count: [u32; 2],
    /// "input buffer".
    pub buffer: [u8; 64],
}

/// `PADDING[64]`: 0x80 then zeros.
const PADDING: [u8; 64] = {
    let mut p = [0u8; 64];
    p[0] = 0x80;
    p
};

/// `S11..S44`, per round (four shifts each).
const SHIFTS: [[u32; 4]; 4] = [[7, 12, 17, 22], [5, 9, 14, 20], [4, 11, 16, 23], [6, 10, 15, 21]];

/// The 64 additive constants (`0xd76aa478` .. `0xeb86d391`), steps 1..64.
const K: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1, 0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

/// `F`, `G`, `H`, `I`: "basic MD5 functions".
fn f(x: u32, y: u32, z: u32) -> u32 {
    (x & y) | (!x & z)
}
fn g(x: u32, y: u32, z: u32) -> u32 {
    (x & z) | (y & !z)
}
fn h(x: u32, y: u32, z: u32) -> u32 {
    x ^ y ^ z
}
fn i(x: u32, y: u32, z: u32) -> u32 {
    y ^ (x | !z)
}

/// `MD5Init(context)`: "Load magic initialization constants".
pub fn md5_init(context: &mut MD5Ctx) {
    context.count[0] = 0;
    context.count[1] = 0;
    context.state[0] = 0x67452301;
    context.state[1] = 0xefcdab89;
    context.state[2] = 0x98badcfe;
    context.state[3] = 0x10325476;
}

impl MD5Ctx {
    /// A context after `MD5Init`.
    pub fn new() -> Self {
        let mut c = MD5Ctx { state: [0; 4], count: [0; 2], buffer: [0; 64] };
        md5_init(&mut c);
        c
    }
}

impl Default for MD5Ctx {
    fn default() -> Self {
        MD5Ctx::new()
    }
}

/// `MD5Update(context, input, inputLen)`: continues the digest with another block of the message.
pub fn md5_update(context: &mut MD5Ctx, input: &[u8]) {
    let input_len = input.len() as u32;

    // Compute number of bytes mod 64
    let mut index = ((context.count[0] >> 3) & 0x3F) as usize;

    // Update number of bits
    context.count[0] = context.count[0].wrapping_add(input_len << 3);
    if context.count[0] < (input_len << 3) {
        context.count[1] = context.count[1].wrapping_add(1);
    }
    context.count[1] = context.count[1].wrapping_add(input_len >> 29);

    let part_len = 64 - index;
    let mut idx = 0usize;

    // Transform as many times as possible.
    if input.len() >= part_len {
        context.buffer[index..index + part_len].copy_from_slice(&input[..part_len]);
        let buf = context.buffer;
        md5_transform(&mut context.state, &buf);

        idx = part_len;
        while idx + 63 < input.len() {
            let mut block = [0u8; 64];
            block.copy_from_slice(&input[idx..idx + 64]);
            md5_transform(&mut context.state, &block);
            idx += 64;
        }
        index = 0;
    }

    // Buffer remaining input
    let rest = input.len() - idx;
    context.buffer[index..index + rest].copy_from_slice(&input[idx..]);
}

/// `MD5Final(digest, context)`: pads, appends the length, outputs the digest and zeroizes the context.
pub fn md5_final(context: &mut MD5Ctx) -> [u8; 16] {
    // Save number of bits
    let mut bits = [0u8; 8];
    encode(&mut bits, &context.count);

    // Pad out to 56 mod 64.
    let index = ((context.count[0] >> 3) & 0x3f) as usize;
    let pad_len = if index < 56 { 56 - index } else { 120 - index };
    md5_update(context, &PADDING[..pad_len]);

    // Append length (before padding)
    md5_update(context, &bits);

    // Store state in digest
    let mut digest = [0u8; 16];
    encode(&mut digest, &context.state);

    // Zeroize sensitive information.
    *context = MD5Ctx { state: [0; 4], count: [0; 2], buffer: [0; 64] };
    digest
}

/// `MD5Transform(state, block)`: "MD5 basic transformation. Transforms state based on block."
fn md5_transform(state: &mut [u32; 4], block: &[u8; 64]) {
    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];
    let x = decode(block);

    for step in 0..64usize {
        let round = step / 16;
        let k = step % 16;
        // the message word and the round function of this step
        let (word, fv) = match round {
            0 => (k, f(b, c, d)),
            1 => ((1 + 5 * k) % 16, g(b, c, d)),
            2 => ((5 + 3 * k) % 16, h(b, c, d)),
            _ => ((7 * k) % 16, i(b, c, d)),
        };
        // (a) += F((b),(c),(d)) + (x) + (UINT4)(ac); (a) = ROTATE_LEFT((a),(s)); (a) += (b);
        let mut t = a.wrapping_add(fv).wrapping_add(x[word]).wrapping_add(K[step]);
        t = t.rotate_left(SHIFTS[round][k % 4]);
        t = t.wrapping_add(b);
        // the (a, d, c, b) rotation of the argument order: FF(d, a, b, c), FF(c, d, a, b), FF(b, c, d, a)
        a = d;
        d = c;
        c = b;
        b = t;
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    // Zeroize sensitive information: `x` is dropped.
}

/// `Encode(output, input, len)`: u32 words to little-endian bytes.
fn encode(output: &mut [u8], input: &[u32]) {
    let len = output.len();
    let mut i = 0usize;
    let mut j = 0usize;
    while j < len {
        output[j] = (input[i] & 0xff) as u8;
        output[j + 1] = ((input[i] >> 8) & 0xff) as u8;
        output[j + 2] = ((input[i] >> 16) & 0xff) as u8;
        output[j + 3] = ((input[i] >> 24) & 0xff) as u8;
        i += 1;
        j += 4;
    }
}

/// `Decode(output, input, len)`: little-endian bytes to u32 words.
fn decode(input: &[u8; 64]) -> [u32; 16] {
    let mut output = [0u32; 16];
    let mut i = 0usize;
    let mut j = 0usize;
    while j < 64 {
        output[i] = (input[j] as u32) | ((input[j + 1] as u32) << 8) | ((input[j + 2] as u32) << 16) | ((input[j + 3] as u32) << 24);
        i += 1;
        j += 4;
    }
    output
}
