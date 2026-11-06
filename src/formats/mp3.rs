// SPDX-License-Identifier: GPL-3.0-only
//! MPEG audio (MP3) lengths from frame headers (GitHub #52). **New code, not a translation:** upstream hands its `.mp3`
//! music to the Miles Sound System, which was never released, and asks it only for a length
//! (`MilesAudioManager::getFileLengthMS`). Written from the published formats:
//!
//! - the MPEG-1/2/2.5 audio frame header (ISO/IEC 11172-3 and 13818-3): sync, version, layer, bit rate and sample-rate
//!   indices, padding; a frame of layer I is `(12 * bitrate / rate + padding) * 4` bytes, of layer II and of MPEG-1
//!   layer III `144 * bitrate / rate + padding`, of MPEG-2/2.5 layer III `72 * bitrate / rate + padding`; its samples
//!   384 (I), 1152 (II, and III of MPEG-1) or 576 (III of MPEG-2/2.5);
//! - the ID3v2 tag before the audio (a 10-byte header whose size is four 7-bit bytes, plus 10 for a footer) and the
//!   128-byte ID3v1 tag (`TAG`) after it;
//! - the Xing/`Info` header in the first frame, after the side information (MPEG-1: 32 bytes stereo, 17 mono;
//!   MPEG-2/2.5: 17 stereo, 9 mono), whose flag bit 0 gives the frame count that follows, and Fraunhofer's `VBRI`
//!   header 32 bytes after the frame header, frame count at its byte 14. The header frame itself is silent and is not
//!   counted.
//!
//! Two lengths are given: [`Mp3Info::header_length_ms`], from the first frame and the file's size alone (the frame
//! count of a Xing or VBRI header when there is one, else the size over the first frame's bit rate), which reads a few
//! hundred bytes; and [`mp3_walk`], which counts every frame and reads the whole file. For a constant-bit-rate file they
//! agree to within the padding bytes' share; the logic uses the first (see `crate::logic::audio_length`).

/// The bit rates in kbit/s, by `[MPEG-1?][layer - 1][index]` (index 0 is "free", 15 is bad: both 0 here).
const BITRATES: [[[u32; 16]; 3]; 2] = [
    // MPEG-2 and 2.5
    [
        [0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256, 0],
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0],
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0],
    ],
    // MPEG-1
    [
        [0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448, 0],
        [0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384, 0],
        [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0],
    ],
];

/// One frame header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameHeader {
    /// 1 (MPEG-1), 2 (MPEG-2) or 25 (MPEG-2.5).
    pub version: u8,
    /// 1, 2 or 3.
    pub layer: u8,
    pub bitrate_kbps: u32,
    pub sample_rate: u32,
    pub padding: bool,
    /// Channel mode 3.
    pub mono: bool,
}

impl FrameHeader {
    /// The header in the four bytes `b`, if they are one this reads (no free format, no reserved values).
    pub fn parse(b: &[u8]) -> Option<FrameHeader> {
        if b.len() < 4 || b[0] != 0xff || b[1] & 0xe0 != 0xe0 {
            return None;
        }
        let version = match (b[1] >> 3) & 3 {
            0 => 25,
            2 => 2,
            3 => 1,
            _ => return None,
        };
        let layer = match (b[1] >> 1) & 3 {
            1 => 3,
            2 => 2,
            3 => 1,
            _ => return None,
        };
        let bitrate_kbps = BITRATES[usize::from(version == 1)][usize::from(layer - 1)][usize::from(b[2] >> 4)];
        let base = match (b[2] >> 2) & 3 {
            0 => 44100,
            1 => 48000,
            2 => 32000,
            _ => return None,
        };
        if bitrate_kbps == 0 {
            return None;
        }
        let sample_rate = match version {
            1 => base,
            2 => base / 2,
            _ => base / 4,
        };
        Some(FrameHeader { version, layer, bitrate_kbps, sample_rate, padding: b[2] & 2 != 0, mono: b[3] >> 6 == 3 })
    }

    /// The samples (per channel) the frame holds.
    pub fn samples(&self) -> u32 {
        match (self.layer, self.version) {
            (1, _) => 384,
            (3, 2) | (3, 25) => 576,
            _ => 1152,
        }
    }

    /// The frame's length in bytes, header included.
    pub fn frame_len(&self) -> usize {
        let pad = u32::from(self.padding);
        (match (self.layer, self.version) {
            (1, _) => (12 * self.bitrate_kbps * 1000 / self.sample_rate + pad) * 4,
            (3, 2) | (3, 25) => 72 * self.bitrate_kbps * 1000 / self.sample_rate + pad,
            _ => 144 * self.bitrate_kbps * 1000 / self.sample_rate + pad,
        }) as usize
    }

    /// Where a Xing/`Info` header would start in the frame (after the side information).
    fn xing_offset(&self) -> usize {
        4 + match (self.version == 1, self.mono) {
            (true, false) => 32,
            (true, true) | (false, false) => 17,
            (false, true) => 9,
        }
    }
}

/// What the first frame and the file's size tell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mp3Info {
    pub first: FrameHeader,
    /// Where the first frame starts (after an ID3v2 tag).
    pub first_offset: usize,
    /// The audio frames counted by a Xing/`Info` or `VBRI` header in the first frame (that frame not counted).
    pub header_frames: Option<u32>,
    /// The bytes from the first frame to the end, less a trailing ID3v1 tag.
    pub audio_bytes: usize,
}

impl Mp3Info {
    /// The length in whole milliseconds (truncated): a Xing or VBRI header's `frames * samples * 1000 / rate`; else the
    /// audio bytes over the first frame's bit rate, `bytes * 8 / kbit/s` (exact for a constant bit rate up to the
    /// padding's share; the header frame, if any, is left out of the bytes).
    pub fn header_length_ms(&self) -> u32 {
        let ms = match self.header_frames {
            Some(n) => u64::from(n) * u64::from(self.first.samples()) * 1000 / u64::from(self.first.sample_rate),
            None => self.audio_bytes as u64 * 8 / u64::from(self.first.bitrate_kbps),
        };
        ms.min(u64::from(u32::MAX)) as u32
    }
}

/// How many bytes the reads below ask for at the first frame (the longest header plus the Xing/VBRI fields).
const FIRST_READ: usize = 64;

/// The ID3v2 tag's length at the start (0 without one).
fn id3v2_len(b: &[u8]) -> usize {
    if b.len() < 10 || &b[0..3] != b"ID3" || b[6..10].iter().any(|&c| c & 0x80 != 0) {
        return 0;
    }
    let size = b[6..10].iter().fold(0usize, |a, &c| (a << 7) | usize::from(c));
    10 + size + if b[5] & 0x10 != 0 { 10 } else { 0 }
}

/// The first frame of a file of `file_len` bytes, read by ranges (`read(offset, len)`, fewer bytes at the end): after
/// an ID3v2 tag, the first offset (within 4 KiB) holding a frame header that the next frame's header confirms (or that
/// ends at the file's end). Reads a few hundred bytes. `None`: no MPEG audio found.
pub fn mp3_info_ranged(file_len: usize, mut read: impl FnMut(usize, usize) -> Vec<u8>) -> Option<Mp3Info> {
    let start = id3v2_len(&read(0, 10));
    let window = read(start, 4096 + 4);
    let mut off = 0;
    let (first_offset, first) = loop {
        if off + 4 > window.len() {
            return None;
        }
        if let Some(h) = FrameHeader::parse(&window[off..off + 4]) {
            let at = start + off;
            let next = at + h.frame_len();
            let confirmed = next >= file_len || FrameHeader::parse(&read(next, 4)).is_some_and(|n| n.version == h.version && n.layer == h.layer && n.sample_rate == h.sample_rate);
            if confirmed {
                break (at, h);
            }
        }
        off += 1;
    };
    let b = read(first_offset, FIRST_READ.max(first.xing_offset() + 12));
    let xo = first.xing_offset();
    let mut header_frames = None;
    if b.len() >= xo + 12 && (&b[xo..xo + 4] == b"Xing" || &b[xo..xo + 4] == b"Info") {
        let flags = u32::from_be_bytes([b[xo + 4], b[xo + 5], b[xo + 6], b[xo + 7]]);
        if flags & 1 != 0 {
            header_frames = Some(u32::from_be_bytes([b[xo + 8], b[xo + 9], b[xo + 10], b[xo + 11]]));
        }
    } else if b.len() >= 36 + 18 && &b[36..40] == b"VBRI" {
        header_frames = Some(u32::from_be_bytes([b[36 + 14], b[36 + 15], b[36 + 16], b[36 + 17]]));
    }
    let mut end = file_len;
    if file_len >= first_offset + 128 && read(file_len - 128, 3) == b"TAG" {
        end -= 128;
    }
    // a header frame is silent: its bytes are not audio
    let audio_start = if header_frames.is_some() { first_offset + first.frame_len() } else { first_offset };
    Some(Mp3Info { first, first_offset, header_frames, audio_bytes: end.saturating_sub(audio_start) })
}

/// [`mp3_info_ranged`] of a file in memory.
pub fn mp3_info(b: &[u8]) -> Option<Mp3Info> {
    mp3_info_ranged(b.len(), |o, l| b[o.min(b.len())..o.saturating_add(l).min(b.len())].to_vec())
}

/// Every frame counted, from the first frame to the first offset that is not a frame header (an ID3v1 tag, junk) or
/// a frame running past the end: `(frames, samples per channel, sample rate)`, a Xing/VBRI header frame not counted.
/// Reads the whole file.
pub fn mp3_walk(b: &[u8]) -> Option<(u32, u64, u32)> {
    let info = mp3_info(b)?;
    let mut at = info.first_offset;
    let (mut frames, mut samples) = (0u32, 0u64);
    while let Some(h) = b.get(at..at + 4).and_then(FrameHeader::parse) {
        let len = h.frame_len();
        if at + len > b.len() {
            break;
        }
        frames += 1;
        samples += u64::from(h.samples());
        at += len;
    }
    if info.header_frames.is_some() && frames > 0 {
        frames -= 1;
        samples -= u64::from(info.first.samples());
    }
    Some((frames, samples, info.first.sample_rate))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A frame of the given header bytes 1..4 (sync first), its body filled with `fill` (never 0xff, so no false sync).
    pub fn frame(b1: u8, b2: u8, b3: u8, fill: u8) -> Vec<u8> {
        let h = FrameHeader::parse(&[0xff, b1, b2, b3]).expect("a header");
        let mut f = vec![0xff, b1, b2, b3];
        f.resize(h.frame_len(), fill);
        f
    }

    /// `n` MPEG-1 layer III frames at 128 kbit/s, 44100 Hz, stereo, padding on every third frame (as an encoder keeps
    /// the average at 128 kbit/s: 417 or 418 bytes).
    pub fn cbr_file(n: usize) -> Vec<u8> {
        let mut out = Vec::new();
        for i in 0..n {
            let pad = if i % 3 == 2 { 2 } else { 0 };
            out.extend_from_slice(&frame(0xfb, 0x90 | pad, 0x00, 0x11));
        }
        out
    }

    /// *Predictions, written before the first run.* The header bytes `fb 90 00`: MPEG-1 layer III, 128 kbit/s,
    /// 44100 Hz, stereo, 1152 samples, `144 * 128000 / 44100` = 417 bytes (418 padded). `f3 80 c0` (MPEG-2 layer III,
    /// 64 kbit/s, 22050 Hz, mono; first run: the fixture had `f3 50 c0`, index 5, which is 40 kbit/s in the MPEG-2 table, a
    /// fixture error, not the reader's): 576 samples, `72 * 64000 / 22050` = 208 bytes; its Xing header at 4 + 9 = 13.
    ///
    /// A CBR file of 300 such MPEG-1 frames (100 padded) behind a 1,000-byte ID3v2 tag and before an ID3v1 tag: the walk
    /// counts 300 frames, 345,600 samples, 7836.73 ms, so 7836 ms; the header estimate is (200 * 417 + 100 * 418) * 8 /
    /// 128 = 7825.0 ms (the padding bytes are the 11 ms between them: the estimate is the length at exactly 128 kbit/s,
    /// the walk at 128.18). The same frames with a Xing header frame (frames = 300) in front: 7836 ms from the header,
    /// and the walk still 300 frames. An MPEG-2 file with a Xing frame claiming 1,000 frames: 1000 * 576 * 1000 / 22050 =
    /// 26122.4, so 26122 ms, though only 3 frames follow (the header is believed). A VBRI header claiming 50 frames
    /// on MPEG-1: 50 * 1152 * 1000 / 44100 = 1306.1, so 1306 ms. Junk before the first frame within 4 KiB is skipped,
    /// a lone false sync (`ff fb` not followed by a frame) is not taken; a file of no frames is `None`.
    #[test]
    fn mp3_lengths_from_headers_and_from_the_walk() {
        let h = FrameHeader::parse(&[0xff, 0xfb, 0x90, 0x00]).unwrap();
        assert_eq!((h.version, h.layer, h.bitrate_kbps, h.sample_rate, h.samples(), h.frame_len()), (1, 3, 128, 44100, 1152, 417));
        let m = FrameHeader::parse(&[0xff, 0xf3, 0x80, 0xc0]).unwrap();
        assert_eq!((m.version, m.layer, m.bitrate_kbps, m.sample_rate, m.samples(), m.frame_len(), m.mono), (2, 3, 64, 22050, 576, 208, true));
        assert_eq!(m.xing_offset(), 13);

        // CBR behind ID3v2, before ID3v1
        let mut f = b"ID3\x03\x00\x00\x00\x00\x07\x5e".to_vec(); // size 0x07 << 7 | 0x5e = 990: 1,000 bytes in all
        f.resize(1000, 0);
        f.extend(cbr_file(300));
        let mut tag = b"TAG".to_vec();
        tag.resize(128, b' ');
        f.extend(tag);
        let i = mp3_info(&f).unwrap();
        assert_eq!((i.first_offset, i.header_frames, i.audio_bytes), (1000, None, 200 * 417 + 100 * 418));
        assert_eq!(i.header_length_ms(), 7825);
        let (frames, samples, rate) = mp3_walk(&f).unwrap();
        assert_eq!((frames, samples, rate), (300, 345_600, 44100));
        assert_eq!(samples * 1000 / u64::from(rate), 7836);

        // a Xing (`Info`) header frame in front
        let mut x = frame(0xfb, 0x90, 0x00, 0x11);
        x[36..40].copy_from_slice(b"Info");
        x[40..44].copy_from_slice(&1u32.to_be_bytes());
        x[44..48].copy_from_slice(&300u32.to_be_bytes());
        x.extend(cbr_file(300));
        let i = mp3_info(&x).unwrap();
        assert_eq!(i.header_frames, Some(300));
        assert_eq!(i.header_length_ms(), 7836);
        assert_eq!(mp3_walk(&x).unwrap().0, 300);

        // MPEG-2 mono, Xing claiming 1,000 frames
        let mut y = frame(0xf3, 0x80, 0xc0, 0x22);
        y[13..17].copy_from_slice(b"Xing");
        y[17..21].copy_from_slice(&1u32.to_be_bytes());
        y[21..25].copy_from_slice(&1000u32.to_be_bytes());
        for _ in 0..3 {
            y.extend(frame(0xf3, 0x80, 0xc0, 0x22));
        }
        assert_eq!(mp3_info(&y).unwrap().header_length_ms(), 26122);
        assert_eq!(mp3_walk(&y).unwrap().0, 3);

        // VBRI
        let mut v = frame(0xfb, 0x90, 0x00, 0x11);
        v[36..40].copy_from_slice(b"VBRI");
        v[50..54].copy_from_slice(&50u32.to_be_bytes());
        v.extend(cbr_file(2));
        assert_eq!(mp3_info(&v).unwrap().header_length_ms(), 1306);

        // junk and a false sync before the audio
        let mut j = vec![0x00, 0x01, 0xff, 0xfb, 0x90, 0x00, 0x02];
        j.extend(cbr_file(5));
        let i = mp3_info(&j).unwrap();
        assert_eq!(i.first_offset, 7);
        assert_eq!(mp3_walk(&j).unwrap().0, 5);
        assert_eq!(mp3_info(&[0u8; 500]), None);
    }
}
