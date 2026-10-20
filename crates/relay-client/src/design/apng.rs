//! Animated PNG (client-motion; feature-motion §2): one APNG from same-sized PNG frames, for an
//! animated export.
//!
//! An APNG is a PNG whose first frame is its ordinary image, so a reader that knows no animation
//! shows that frame: the first frame's header and the chunks before its image data, then `acTL`
//! (how many frames, how many plays), then per frame an `fcTL` (its size, place, delay and how it
//! is laid down) and its image data, `IDAT` for the first and `fdAT` (a sequence number, then the
//! same bytes) for the rest. The frames' compressed data is copied, never decoded.

/// Most frames one animation holds: 20 s at 30 frames a second.
pub const MAX_FRAMES: usize = 600;

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n'];

/// One chunk of a PNG: its type, its data, and the whole chunk as it was written (length to CRC).
struct Chunk<'a> {
    kind: [u8; 4],
    data: &'a [u8],
    whole: &'a [u8],
}

/// A PNG's chunks up to `IEND`, the first being a 13-byte `IHDR`.
fn chunks(png: &[u8]) -> Result<Vec<Chunk<'_>>, String> {
    let mut rest = png.strip_prefix(&SIGNATURE[..]).ok_or("it is not a PNG")?;
    let mut out = Vec::new();
    while !rest.is_empty() {
        let (Some(len), Some(kind)) = (rest.get(..4), rest.get(4..8)) else {
            return Err("it ends inside a chunk".into());
        };
        let len = u32::from_be_bytes(len.try_into().unwrap_or_default());
        if len > i32::MAX as u32 {
            return Err("a chunk is longer than a PNG allows".into());
        }
        let end = 12 + len as usize;
        let whole = rest.get(..end).ok_or("it ends inside a chunk")?;
        let kind: [u8; 4] = kind.try_into().unwrap_or_default();
        out.push(Chunk { kind, data: &whole[8..end - 4], whole });
        rest = &rest[end..];
        if &kind == b"IEND" {
            break;
        }
    }
    match out.first() {
        Some(c) if &c.kind == b"IHDR" && c.data.len() == 13 => Ok(out),
        _ => Err("it does not start with its header".into()),
    }
}

/// Write one chunk: its length, type, the `parts` of its data one after another, and its CRC.
fn put(out: &mut Vec<u8>, kind: &[u8; 4], parts: &[&[u8]]) {
    let len: usize = parts.iter().map(|p| p.len()).sum();
    out.extend_from_slice(&(len as u32).to_be_bytes());
    let mut crc = crc32fast::Hasher::new();
    out.extend_from_slice(kind);
    crc.update(kind);
    for part in parts {
        out.extend_from_slice(part);
        crc.update(part);
    }
    out.extend_from_slice(&crc.finalize().to_be_bytes());
}

/// One animated PNG from same-sized PNG frames, each shown `1/fps` s, played `plays` times (0:
/// forever). The first frame's header and the chunks before its image data lead (a palette, colour
/// space, density), then `acTL`, then per frame an `fcTL` (laid over nothing: dispose none, blend
/// source) and its image data. Refuses no frames or more than [`MAX_FRAMES`], an fps of 0, a
/// frame that is not a PNG or has no image data, an interlaced frame, and frames whose size or
/// pixel format differs from the first's.
pub fn assemble(frames: &[Vec<u8>], fps: u16, plays: u32) -> Result<Vec<u8>, String> {
    if frames.is_empty() || frames.len() > MAX_FRAMES {
        return Err(format!("an animation holds 1 to {MAX_FRAMES} frames (this has {})", frames.len()));
    }
    if fps == 0 {
        return Err("fps: give 1 or more frames a second".into());
    }
    let parsed = frames.iter().enumerate()
        .map(|(i, png)| chunks(png).map_err(|why| format!("frame {i}: {why}")))
        .collect::<Result<Vec<_>, _>>()?;
    let header = parsed[0][0].data;
    let size = |h: &[u8]| (u32::from_be_bytes([h[0], h[1], h[2], h[3]]), u32::from_be_bytes([h[4], h[5], h[6], h[7]]));
    let (w, h) = size(header);
    for (i, frame) in parsed.iter().enumerate() {
        let own = frame[0].data;
        if own[12] != 0 {
            return Err(format!("frame {i} is interlaced; save frames without interlacing"));
        }
        if own != header {
            let (fw, fh) = size(own);
            return Err(format!("frame {i} is {fw}×{fh} px (bit depth {}, colour type {}), not {w}×{h} px (bit depth {}, colour type {}) like frame 0",
                own[8], own[9], header[8], header[9]));
        }
        if !frame.iter().any(|c| &c.kind == b"IDAT") {
            return Err(format!("frame {i} has no image data"));
        }
    }
    let mut out = Vec::with_capacity(frames.iter().map(Vec::len).sum::<usize>() + 64 * frames.len() + 64);
    out.extend_from_slice(&SIGNATURE);
    out.extend_from_slice(parsed[0][0].whole);
    // What the first frame says before its image data (a palette, gamma, colour space, density),
    // without animation chunks of its own.
    for c in parsed[0][1..].iter().take_while(|c| &c.kind != b"IDAT") {
        if !matches!(&c.kind, b"acTL" | b"fcTL" | b"fdAT" | b"IEND") {
            out.extend_from_slice(c.whole);
        }
    }
    put(&mut out, b"acTL", &[&(frames.len() as u32).to_be_bytes(), &plays.to_be_bytes()]);
    // Each frame shows for 1/fps s.
    let delay = [1u16.to_be_bytes(), fps.to_be_bytes()].concat();
    // fcTL and fdAT share one count, from 0, without gaps.
    let mut sequence = 0u32;
    for (i, frame) in parsed.iter().enumerate() {
        // The whole frame at 0,0; dispose none (0), blend source (0).
        put(&mut out, b"fcTL", &[&sequence.to_be_bytes(), &w.to_be_bytes(), &h.to_be_bytes(), &[0; 8], &delay, &[0, 0]]);
        sequence += 1;
        for c in frame.iter().filter(|c| &c.kind == b"IDAT") {
            if i == 0 {
                out.extend_from_slice(c.whole);
            } else {
                put(&mut out, b"fdAT", &[&sequence.to_be_bytes(), c.data]);
                sequence += 1;
            }
        }
    }
    put(&mut out, b"IEND", &[]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `w`×`h` PNG of 8-bit RGBA pixels all `rgba`, its data stored uncompressed in `idats`
    /// IDAT chunks, with an `sRGB` chunk before them and a `tEXt` after; `interlace` sets the
    /// header's interlace byte.
    fn png(w: u32, h: u32, rgba: [u8; 4], idats: usize, interlace: u8) -> Vec<u8> {
        // Each row: filter 0 (none), then its pixels.
        let row: Vec<u8> = std::iter::once(0).chain((0..w).flat_map(|_| rgba)).collect();
        let raw = row.repeat(h as usize);
        // zlib with one stored deflate block, and its Adler-32.
        let (mut a, mut b) = (1u32, 0u32);
        for &byte in &raw {
            a = (a + u32::from(byte)) % 65521;
            b = (b + a) % 65521;
        }
        let len = raw.len() as u16;
        let mut z = vec![0x78, 0x01, 0x01];
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(&raw);
        z.extend_from_slice(&((b << 16) | a).to_be_bytes());
        let mut out = SIGNATURE.to_vec();
        put(&mut out, b"IHDR", &[&w.to_be_bytes(), &h.to_be_bytes(), &[8, 6, 0, 0, interlace]]);
        put(&mut out, b"sRGB", &[&[0]]);
        let piece = z.len().div_ceil(idats);
        for part in z.chunks(piece) {
            put(&mut out, b"IDAT", &[part]);
        }
        put(&mut out, b"tEXt", &[b"Software\0relay"]);
        put(&mut out, b"IEND", &[]);
        out
    }

    /// An APNG's chunks as (type, data), each CRC checked.
    fn read(apng: &[u8]) -> Vec<(String, Vec<u8>)> {
        chunks(apng).unwrap().iter().map(|c| {
            let crc = u32::from_be_bytes(c.whole[c.whole.len() - 4..].try_into().unwrap());
            assert_eq!(crc, crc32fast::hash(&c.whole[4..c.whole.len() - 4]), "{}'s CRC", String::from_utf8_lossy(&c.kind));
            (String::from_utf8_lossy(&c.kind).into_owned(), c.data.to_vec())
        }).collect()
    }

    fn be32(b: &[u8]) -> u32 {
        u32::from_be_bytes(b[..4].try_into().unwrap())
    }

    #[test]
    fn assembles_frames_into_one_apng() {
        let frames = vec![png(1, 1, [255, 0, 0, 255], 1, 0), png(1, 1, [0, 255, 0, 128], 1, 0), png(1, 1, [0, 0, 255, 0], 2, 0)];
        let apng = assemble(&frames, 30, 0).unwrap();
        assert_eq!(&apng[..8], &SIGNATURE);
        let got = read(&apng);
        let kinds: Vec<&str> = got.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(kinds, ["IHDR", "sRGB", "acTL", "fcTL", "IDAT", "fcTL", "fdAT", "fcTL", "fdAT", "fdAT", "IEND"]);
        // The header and what comes before the image data are frame 0's.
        let first = read(&frames[0]);
        assert_eq!((&got[0], &got[1]), (&first[0], &first[1]));
        // acTL: three frames, played forever.
        assert_eq!((be32(&got[2].1), be32(&got[2].1[4..])), (3, 0));
        // fcTL and fdAT count 0, 1, 2… without gaps; each fcTL is 1×1 at 0,0 for 1/30 s, laid
        // over nothing.
        let sequence: Vec<u32> = got.iter().filter(|(k, _)| k == "fcTL" || k == "fdAT").map(|(_, d)| be32(d)).collect();
        assert_eq!(sequence, [0, 1, 2, 3, 4, 5]);
        for (_, d) in got.iter().filter(|(k, _)| k == "fcTL") {
            assert_eq!(d.len(), 26);
            assert_eq!((be32(&d[4..]), be32(&d[8..]), be32(&d[12..]), be32(&d[16..])), (1, 1, 0, 0));
            assert_eq!(&d[20..], &[0, 1, 0, 30, 0, 0]);
        }
        // Image data is copied as it was: frame 0's IDAT, then each later frame's IDATs as fdATs.
        let idat = |png: &[u8]| read(png).into_iter().filter(|(k, _)| k == "IDAT").map(|(_, d)| d).collect::<Vec<_>>();
        assert_eq!(got[4].1, idat(&frames[0])[0]);
        let fdat: Vec<Vec<u8>> = got.iter().filter(|(k, _)| k == "fdAT").map(|(_, d)| d[4..].to_vec()).collect();
        assert_eq!(fdat, [idat(&frames[1]), idat(&frames[2])].concat());
        // Played once.
        let once = read(&assemble(&frames, 12, 1).unwrap());
        assert_eq!(be32(&once[2].1[4..]), 1);
        assert_eq!(&once[3].1[20..24], &[0, 1, 0, 12]);
        // One frame is a still that a reader without animation shows the same.
        let still = read(&assemble(&frames[..1], 30, 0).unwrap());
        assert_eq!(still.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), ["IHDR", "sRGB", "acTL", "fcTL", "IDAT", "IEND"]);
    }

    #[test]
    fn frames_that_do_not_fit_are_refused() {
        let red = png(1, 1, [255, 0, 0, 255], 1, 0);
        let refused = |frames: Vec<Vec<u8>>, fps: u16| assemble(&frames, fps, 0).unwrap_err();
        assert_eq!(refused(vec![red.clone(), png(2, 1, [0; 4], 1, 0)], 30),
            "frame 1 is 2×1 px (bit depth 8, colour type 6), not 1×1 px (bit depth 8, colour type 6) like frame 0");
        let mut grey = png(1, 1, [0; 4], 1, 0);
        grey[8 + 8 + 9] = 0; // colour type 0 (grey) in the header, its CRC left wrong: nothing reads it
        assert!(refused(vec![red.clone(), grey], 30).contains("colour type 0), not 1×1 px"));
        assert_eq!(refused(vec![red.clone(), png(1, 1, [0; 4], 1, 1)], 30), "frame 1 is interlaced; save frames without interlacing");
        assert_eq!(refused(vec![], 30), "an animation holds 1 to 600 frames (this has 0)");
        assert_eq!(refused(vec![red.clone(); MAX_FRAMES + 1], 30), "an animation holds 1 to 600 frames (this has 601)");
        assert!(assemble(&vec![red.clone(); MAX_FRAMES], 30, 0).is_ok());
        assert_eq!(refused(vec![red.clone()], 0), "fps: give 1 or more frames a second");
        assert_eq!(refused(vec![red.clone(), b"GIF89a".to_vec()], 30), "frame 1: it is not a PNG");
        assert_eq!(refused(vec![red[..red.len() - 3].to_vec()], 30), "frame 0: it ends inside a chunk");
        let mut headless = SIGNATURE.to_vec();
        put(&mut headless, b"IEND", &[]);
        assert_eq!(refused(vec![headless], 30), "frame 0: it does not start with its header");
        let mut empty = SIGNATURE.to_vec();
        put(&mut empty, b"IHDR", &[&1u32.to_be_bytes(), &1u32.to_be_bytes(), &[8, 6, 0, 0, 0]]);
        put(&mut empty, b"IEND", &[]);
        assert_eq!(refused(vec![empty], 30), "frame 0 has no image data");
    }
}
