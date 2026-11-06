// SPDX-License-Identifier: GPL-3.0-only
//! RIFF WAVE files and the IMA ADPCM codec (GitHub #13, C6). **New code, not a translation:** upstream hands its
//! `.wav` files to the Miles Sound System, which was never released. Written from the published formats: Microsoft's
//! RIFF/WAVE layout (`fmt ` with `WAVE_FORMAT_PCM` = 1 and `WAVE_FORMAT_IMA_ADPCM` = 0x11, `data`) and the IMA ADPCM
//! reference algorithm (IMA Digital Audio Focus and Technical Working Groups, "Recommended Practices for Enhancing
//! Digital Audio Compatibility in Multimedia Systems", 1992): its 89-step table, its 16-entry index table, and the
//! Microsoft block layout (a 4-byte header per channel: the first sample, the step index and a reserved byte; then, for
//! more than one channel, 4-byte groups of 8 samples per channel, low nibble first).
//!
//! Decoding is by blocks, so a caller can spread a long file over several frames ([`WavDecoder::decode_some`]).

/// `WAVE_FORMAT_PCM`.
pub const WAVE_FORMAT_PCM: u16 = 1;
/// `WAVE_FORMAT_IMA_ADPCM`.
pub const WAVE_FORMAT_IMA_ADPCM: u16 = 0x11;

/// The IMA step sizes.
const STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209,
    230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749,
    3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623,
    27086, 29794, 32767,
];

/// The IMA step index changes, by the nibble's low three bits.
const INDEX_TABLE: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];

/// What a wave file holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WavInfo {
    pub format: u16,
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub block_align: u16,
    /// IMA ADPCM: samples per channel per block (the `fmt ` extension's, else computed from the block size).
    pub samples_per_block: u32,
    /// The `data` chunk's offset and length in the file.
    pub data: (usize, usize),
}

/// Why a file is not a wave this reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WavError {
    NotRiffWave,
    NoFmt,
    NoData,
    Unsupported { format: u16, bits: u16, channels: u16 },
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// Reads the RIFF chunks of a wave file: its format and where its samples are. A `data` chunk longer than the file is
/// cut to the file.
pub fn parse_wav(b: &[u8]) -> Result<WavInfo, WavError> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err(WavError::NotRiffWave);
    }
    let mut fmt: Option<(u16, u16, u32, u16, u16, u32)> = None;
    let mut i = 12;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32_at(b, i + 4) as usize;
        let body = i + 8;
        if id == b"fmt " && body + 16 <= b.len() {
            let format = u16_at(b, body);
            let channels = u16_at(b, body + 2);
            let rate = u32_at(b, body + 4);
            let align = u16_at(b, body + 12);
            let bits = u16_at(b, body + 14);
            let spb = if len >= 20 && body + 20 <= b.len() { u32::from(u16_at(b, body + 18)) } else { 0 };
            fmt = Some((format, channels, rate, align, bits, spb));
        } else if id == b"data" {
            let (format, channels, sample_rate, block_align, bits_per_sample, spb) = fmt.ok_or(WavError::NoFmt)?;
            let ok = match format {
                WAVE_FORMAT_PCM => (bits_per_sample == 8 || bits_per_sample == 16) && channels >= 1,
                WAVE_FORMAT_IMA_ADPCM => bits_per_sample == 4 && (1..=2).contains(&channels) && usize::from(block_align) > 4 * usize::from(channels),
                _ => false,
            };
            if !ok {
                return Err(WavError::Unsupported { format, bits: bits_per_sample, channels });
            }
            let samples_per_block = if format == WAVE_FORMAT_IMA_ADPCM {
                let computed = (u32::from(block_align) - 4 * u32::from(channels)) * 8 / (4 * u32::from(channels)) + 1;
                if spb != 0 { spb.min(computed) } else { computed }
            } else {
                1
            };
            let len = len.min(b.len() - body);
            return Ok(WavInfo { format, channels, sample_rate, bits_per_sample, block_align, samples_per_block, data: (body, len) });
        }
        i = body + len + (len & 1);
    }
    Err(if fmt.is_some() { WavError::NoData } else { WavError::NoFmt })
}

/// One channel's IMA decoder state.
#[derive(Clone, Copy, Debug, Default)]
struct ImaState {
    predictor: i32,
    index: i32,
}

impl ImaState {
    /// One nibble: the reference algorithm's step.
    fn decode(&mut self, nibble: u8) -> i16 {
        let step = STEP_TABLE[self.index as usize];
        let mut diff = step >> 3;
        if nibble & 4 != 0 {
            diff += step;
        }
        if nibble & 2 != 0 {
            diff += step >> 1;
        }
        if nibble & 1 != 0 {
            diff += step >> 2;
        }
        if nibble & 8 != 0 {
            self.predictor -= diff;
        } else {
            self.predictor += diff;
        }
        self.predictor = self.predictor.clamp(-32768, 32767);
        self.index = (self.index + INDEX_TABLE[usize::from(nibble & 7)]).clamp(0, 88);
        self.predictor as i16
    }
}

/// Decodes one IMA ADPCM block into interleaved samples (`out` gets `samples_per_block * channels`, fewer for a short
/// last block).
pub fn decode_ima_block(block: &[u8], channels: usize, samples_per_block: usize, out: &mut Vec<i16>) {
    if block.len() < 4 * channels {
        return;
    }
    let mut st = [ImaState::default(); 2];
    for (c, s) in st.iter_mut().enumerate().take(channels) {
        let h = &block[4 * c..4 * c + 4];
        s.predictor = i32::from(i16::from_le_bytes([h[0], h[1]]));
        s.index = i32::from(h[2]).clamp(0, 88);
    }
    let start = out.len();
    for s in st.iter().take(channels) {
        out.push(s.predictor as i16);
    }
    let data = &block[4 * channels..];
    let mut frames = 1usize;
    if channels == 1 {
        for &byte in data {
            for nib in [byte & 0x0f, byte >> 4] {
                if frames >= samples_per_block {
                    break;
                }
                out.push(st[0].decode(nib));
                frames += 1;
            }
        }
    } else {
        // groups of 4 bytes per channel: 8 samples of the left, then 8 of the right
        let group = 4 * channels;
        for g in data.chunks(group) {
            if g.len() < group || frames >= samples_per_block {
                break;
            }
            let n = (samples_per_block - frames).min(8);
            let base = out.len();
            out.resize(base + n * channels, 0);
            for c in 0..channels {
                let bytes = &g[4 * c..4 * c + 4];
                for k in 0..n {
                    let byte = bytes[k / 2];
                    let nib = if k % 2 == 0 { byte & 0x0f } else { byte >> 4 };
                    out[base + k * channels + c] = st[c].decode(nib);
                }
            }
            frames += n;
        }
    }
    debug_assert!(out.len() - start <= samples_per_block * channels);
}

/// A wave file decoded in steps.
#[derive(Clone, Debug)]
pub struct WavDecoder {
    pub info: WavInfo,
    /// Interleaved 16-bit samples so far.
    pub samples: Vec<i16>,
    /// The data bytes read so far.
    pos: usize,
}

impl WavDecoder {
    pub fn new(file: &[u8]) -> Result<Self, WavError> {
        let info = parse_wav(file)?;
        Ok(WavDecoder { info, samples: Vec::new(), pos: 0 })
    }

    /// The data bytes decoded so far.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Whether every sample is decoded.
    pub fn done(&self) -> bool {
        self.pos >= self.info.data.1
    }

    /// Decodes about `budget` more bytes of the data (whole blocks, at least one); returns whether it is done.
    pub fn decode_some(&mut self, file: &[u8], budget: usize) -> bool {
        let (off, len) = self.info.data;
        let data = &file[off..off + len];
        let end = (self.pos + budget.max(1)).min(len);
        match self.info.format {
            WAVE_FORMAT_PCM => {
                let width = usize::from(self.info.bits_per_sample / 8);
                let mut e = self.pos + (end - self.pos) / width * width;
                if e == self.pos {
                    e = (self.pos + width).min(len);
                }
                let chunk = &data[self.pos..e];
                if width == 1 {
                    self.samples.extend(chunk.iter().map(|&u| (i16::from(u) - 128) << 8));
                } else {
                    self.samples.extend(chunk.chunks_exact(2).map(|p| i16::from_le_bytes([p[0], p[1]])));
                }
                // a trailing partial sample is dropped
                self.pos = if len - e < width { len } else { e };
            }
            _ => {
                let align = usize::from(self.info.block_align);
                let ch = usize::from(self.info.channels);
                let spb = self.info.samples_per_block as usize;
                loop {
                    let block = &data[self.pos..(self.pos + align).min(len)];
                    decode_ima_block(block, ch, spb, &mut self.samples);
                    self.pos = (self.pos + align).min(len);
                    if self.pos >= end || self.pos >= len {
                        break;
                    }
                }
            }
        }
        self.done()
    }

    /// The whole file at once.
    pub fn decode_all(file: &[u8]) -> Result<(WavInfo, Vec<i16>), WavError> {
        let mut d = WavDecoder::new(file)?;
        while !d.decode_some(file, usize::MAX / 2) {}
        Ok((d.info, d.samples))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A RIFF file of one `fmt ` (with the IMA extension when `spb` is given) and one `data` chunk.
    pub fn wav(format: u16, channels: u16, rate: u32, align: u16, bits: u16, spb: Option<u16>, data: &[u8]) -> Vec<u8> {
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&format.to_le_bytes());
        fmt.extend_from_slice(&channels.to_le_bytes());
        fmt.extend_from_slice(&rate.to_le_bytes());
        fmt.extend_from_slice(&(rate * u32::from(align)).to_le_bytes());
        fmt.extend_from_slice(&align.to_le_bytes());
        fmt.extend_from_slice(&bits.to_le_bytes());
        if let Some(s) = spb {
            fmt.extend_from_slice(&2u16.to_le_bytes());
            fmt.extend_from_slice(&s.to_le_bytes());
        }
        let mut b = b"RIFF\0\0\0\0WAVE".to_vec();
        b.extend_from_slice(b"fmt ");
        b.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
        b.extend_from_slice(&fmt);
        b.extend_from_slice(b"LIST\x03\0\0\0abc\0"); // an odd chunk, padded
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend_from_slice(data);
        let n = (b.len() - 8) as u32;
        b[4..8].copy_from_slice(&n.to_le_bytes());
        b
    }

    /// The reference IMA encoder (for the round trip only).
    fn encode(samples: &[i16]) -> (i16, Vec<u8>) {
        let mut st = ImaState { predictor: i32::from(samples[0]), index: 0 };
        let mut nibbles = Vec::new();
        for &s in &samples[1..] {
            let step = STEP_TABLE[st.index as usize];
            let mut diff = i32::from(s) - st.predictor;
            let mut n = 0u8;
            if diff < 0 {
                n = 8;
                diff = -diff;
            }
            let mut tmp = step;
            if diff >= tmp {
                n |= 4;
                diff -= tmp;
            }
            tmp >>= 1;
            if diff >= tmp {
                n |= 2;
                diff -= tmp;
            }
            tmp >>= 1;
            if diff >= tmp {
                n |= 1;
            }
            st.decode(n);
            nibbles.push(n);
        }
        let bytes = nibbles.chunks(2).map(|p| p[0] | p.get(1).copied().unwrap_or(0) << 4).collect();
        (samples[0], bytes)
    }

    /// Predicted, by hand from the reference tables: a mono block starting at 0, step index 0, nibbles 7 then 8: the
    /// first sample 0, then 0 + (7>>3) + 7 + 3 + 1 = 11 (index 8), then 11 - (16>>3) = 9 (index 7).
    #[test]
    fn ima_by_hand() {
        let mut out = Vec::new();
        decode_ima_block(&[0, 0, 0, 0, 0x87], 1, 3, &mut out);
        assert_eq!(out, [0, 11, 9]);
    }

    /// Predicted: a 440 Hz sine at 22050 Hz, encoded by the reference encoder in 512-byte mono blocks (1017 samples per
    /// block) and wrapped in a wave, decodes to as many samples, within 3 % of full scale of the original after the
    /// first few dozen samples of each block (the test encoder starts every block at step index 0); stereo blocks keep the channels apart (left silent,
    /// right the sine); 8- and 16-bit PCM decode exactly; decoding in 100-byte steps gives the same samples as at once.
    #[test]
    fn ima_round_trip_stereo_and_pcm() {
        let sine: Vec<i16> = (0..1017 * 3).map(|i| (12000.0 * petir::Real::sin(i as f64 * 2.0 * 3.141592653589793 * 440.0 / 22050.0)) as i16).collect();
        let mut data = Vec::new();
        for blk in sine.chunks(1017) {
            let (first, bytes) = encode(blk);
            data.extend_from_slice(&first.to_le_bytes());
            data.extend_from_slice(&[0, 0]);
            data.extend_from_slice(&bytes);
            data.resize(data.len().div_ceil(512) * 512, 0);
        }
        let f = wav(WAVE_FORMAT_IMA_ADPCM, 1, 22050, 512, 4, Some(1017), &data);
        let (info, out) = WavDecoder::decode_all(&f).unwrap();
        assert_eq!(info.samples_per_block, 1017);
        assert_eq!(out.len(), sine.len());
        let worst = out.iter().zip(&sine).enumerate().filter(|(i, _)| i % 1017 >= 40).map(|(_, (a, b))| (i32::from(*a) - i32::from(*b)).abs()).max().unwrap();
        assert!(worst < 32768 * 3 / 100, "worst {worst}");
        let mut stepwise = WavDecoder::new(&f).unwrap();
        let mut steps = 0;
        while !stepwise.decode_some(&f, 100) {
            steps += 1;
        }
        assert_eq!(steps, 2, "three blocks, one per step");
        assert_eq!(stepwise.samples, out);

        // stereo: left all zeros, right the first 9 samples of the sine
        let (first, bytes) = encode(&sine[..9]);
        let mut blk = vec![0, 0, 0, 0];
        blk.extend_from_slice(&first.to_le_bytes());
        blk.extend_from_slice(&[0, 0]);
        blk.extend_from_slice(&[0u8; 4]);
        blk.extend_from_slice(&bytes[..4]);
        let mut out = Vec::new();
        decode_ima_block(&blk, 2, 9, &mut out);
        assert_eq!(out.len(), 18);
        let mono_r = {
            let mut o = Vec::new();
            let mut b = first.to_le_bytes().to_vec();
            b.extend_from_slice(&[0, 0]);
            b.extend_from_slice(&bytes[..4]);
            decode_ima_block(&b, 1, 9, &mut o);
            o
        };
        assert!(out.iter().step_by(2).all(|&l| l == 0 || l.abs() < 8), "{out:?}");
        assert_eq!(out.iter().skip(1).step_by(2).copied().collect::<Vec<_>>(), mono_r);

        let p8 = wav(WAVE_FORMAT_PCM, 1, 11025, 1, 8, None, &[128, 255, 0]);
        assert_eq!(WavDecoder::decode_all(&p8).unwrap().1, [0, 127 << 8, -128 << 8]);
        let p16 = wav(WAVE_FORMAT_PCM, 2, 44100, 4, 16, None, &[1, 0, 0xff, 0xff]);
        assert_eq!(WavDecoder::decode_all(&p16).unwrap().1, [1, -1]);
        assert_eq!(parse_wav(b"RIFX"), Err(WavError::NotRiffWave));
        assert!(matches!(parse_wav(&wav(0x55, 2, 44100, 1, 0, None, &[])), Err(WavError::Unsupported { format: 0x55, .. })));
    }
}
