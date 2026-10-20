//! Pictures Relay takes in (`design.asset.add`, `session.upload`, `thread.upload`): a picture's
//! format from its first bytes and its natural size from its header, without decoding it, and a
//! picture sent as base64, decoded within what a door carries.

use relay_bus::error::BusError;
use relay_bus::design::MAX_COORD;

#[derive(Debug, PartialEq, Eq)]
pub struct Picture {
    /// The extension it is stored under.
    pub ext: &'static str,
    pub w: u32,
    pub h: u32,
}

/// PNG, JPEG, GIF, WebP or SVG, with a size; `None` for anything else, or a raster whose
/// header is cut short.
pub fn sniff(bytes: &[u8]) -> Option<Picture> {
    let (ext, size) = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        ("png", png(bytes))
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        ("jpg", jpeg(bytes))
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        ("gif", gif(bytes))
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(&b"WEBP"[..]) {
        ("webp", webp(bytes))
    } else {
        ("svg", svg(bytes))
    };
    let (w, h) = size?;
    (w > 0 && h > 0).then_some(Picture { ext, w, h })
}

/// [`sniff`], for the pictures a model reads: PNG, JPEG, GIF or WebP, never SVG.
pub fn raster(bytes: &[u8]) -> Option<Picture> {
    sniff(bytes).filter(|picture| picture.ext != "svg")
}

impl Picture {
    /// Its media type, as a model's image block names it.
    pub fn media_type(&self) -> &'static str {
        match self.ext {
            "png" => "image/png",
            "jpg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            _ => "image/svg+xml",
        }
    }
}

/// The longest side a model's API takes; it refuses a larger picture outright.
pub const MAX_SIDE: u32 = 8000;

/// Whether `bytes`, sniffed as `picture`, run to their format's end. A download or a copy cut
/// short reads its size from the header like a whole one, and a model's API refuses it, so the
/// thread it went to cannot be answered again.
pub fn whole(bytes: &[u8], picture: &Picture) -> bool {
    match picture.ext {
        // The IEND chunk ends every PNG; a little may follow it.
        "png" => within_tail(bytes, b"IEND\xAE\x42\x60\x82", 4096),
        // The end of the image after the main image's scan: entropy-coded data never holds FF D9,
        // and an EXIF thumbnail's own end comes before the scan. A motion photo's video follows it.
        "jpg" => jpeg_scan(bytes).is_some_and(|scan| bytes[scan..].windows(2).any(|w| w == [0xFF, 0xD9])),
        // The trailer, past any zero padding.
        "gif" => bytes.iter().rev().find(|&&b| b != 0) == Some(&0x3B),
        // The RIFF size counts every byte after its own eight.
        "webp" => le32(bytes, 4).is_some_and(|size| bytes.len() as u64 >= u64::from(size) + 8),
        _ => true,
    }
}

fn within_tail(bytes: &[u8], marker: &[u8], tail: usize) -> bool {
    bytes[bytes.len().saturating_sub(tail)..].windows(marker.len()).any(|w| w == marker)
}

/// Where the main image's entropy-coded data starts: past its start-of-scan segment, walking the
/// segments before it as [`jpeg`] does.
fn jpeg_scan(b: &[u8]) -> Option<usize> {
    let mut at = 2;
    loop {
        if *b.get(at)? != 0xFF {
            return None;
        }
        while *b.get(at)? == 0xFF {
            at += 1;
        }
        let marker = b[at];
        at += 1;
        match marker {
            0x01 | 0xD0..=0xD8 => continue,
            0xD9 => return None,
            _ => {
                let len = be16(b, at)? as usize;
                if len < 2 {
                    return None;
                }
                at += len;
                if marker == 0xDA {
                    return (at <= b.len()).then_some(at);
                }
            }
        }
    }
}

/// The extension a raster of `media_type` is stored under; `None` for any other type.
pub fn raster_ext(media_type: &str) -> Option<&'static str> {
    match media_type.trim().to_ascii_lowercase().as_str() {
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

/// Pictures larger than this are refused; the phone sends a photo shrunk well under it, and a
/// door takes at most 8 MiB in one message.
pub const UPLOAD_MAX: usize = 4 << 20;

/// A picture sent as base64, at most [`UPLOAD_MAX`] decoded; refused under `code` otherwise.
pub fn decode_upload(encoded: &str, code: &'static str) -> Result<Vec<u8>, BusError> {
    use base64::Engine as _;
    let too_big = || BusError::invalid(code, format!("the picture is over {} MiB", UPLOAD_MAX >> 20))
        .with_hint("send it smaller");
    if encoded.len() > UPLOAD_MAX.div_ceil(3) * 4 + 4 {
        return Err(too_big());
    }
    let compact: String = encoded.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD.decode(compact.as_bytes())
        .map_err(|_| BusError::invalid(code, "bytes_b64 is not base64"))?;
    if bytes.len() > UPLOAD_MAX {
        return Err(too_big());
    }
    Ok(bytes)
}

fn be16(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?)))
}
fn le16(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?)))
}
fn le24(b: &[u8], at: usize) -> Option<u32> {
    let b = b.get(at..at + 3)?;
    Some(u32::from(b[0]) | u32::from(b[1]) << 8 | u32::from(b[2]) << 16)
}
fn le32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// The IHDR chunk, always first.
fn png(b: &[u8]) -> Option<(u32, u32)> {
    if b.get(12..16)? != b"IHDR" {
        return None;
    }
    Some((u32::from_be_bytes(b.get(16..20)?.try_into().ok()?), u32::from_be_bytes(b.get(20..24)?.try_into().ok()?)))
}

/// The logical screen.
fn gif(b: &[u8]) -> Option<(u32, u32)> {
    Some((le16(b, 6)?, le16(b, 8)?))
}

/// The first frame header (SOF0 to SOF15, less the DHT, JPG and DAC markers that share
/// the range), walking the segments before it.
fn jpeg(b: &[u8]) -> Option<(u32, u32)> {
    let mut at = 2;
    loop {
        if *b.get(at)? != 0xFF {
            return None;
        }
        while *b.get(at)? == 0xFF {
            at += 1;
        }
        let marker = b[at];
        at += 1;
        match marker {
            // Markers with no length.
            0x01 | 0xD0..=0xD8 => continue,
            // The image data or its end, and no frame header before them.
            0xD9 | 0xDA => return None,
            0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
                // length(2), precision(1), height(2), width(2)
                return Some((be16(b, at + 5)?, be16(b, at + 3)?));
            }
            _ => {
                let len = be16(b, at)? as usize;
                if len < 2 {
                    return None;
                }
                at += len;
            }
        }
    }
}

/// The first chunk: lossy (`VP8 `), lossless (`VP8L`) or extended (`VP8X`, its canvas).
fn webp(b: &[u8]) -> Option<(u32, u32)> {
    match b.get(12..16)? {
        b"VP8 " => {
            if b.get(23..26)? != [0x9Du8, 0x01, 0x2A] {
                return None;
            }
            Some((le16(b, 26)? & 0x3FFF, le16(b, 28)? & 0x3FFF))
        }
        b"VP8L" => {
            if *b.get(20)? != 0x2F {
                return None;
            }
            let bits = le32(b, 21)?;
            Some(((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1))
        }
        b"VP8X" => Some((le24(b, 24)? + 1, le24(b, 27)? + 1)),
        _ => None,
    }
}

/// Text whose first tag, past an XML declaration, comments and a doctype, is `<svg`. Its
/// size is `width`/`height` in px (or unitless), the other taken from `viewBox`'s aspect
/// when only one is given, else the `viewBox`, else a browser's default 300 by 150.
fn svg(b: &[u8]) -> Option<(u32, u32)> {
    let head = &b[..b.len().min(64 * 1024)];
    let text = match std::str::from_utf8(head) {
        Ok(text) => text,
        Err(e) => std::str::from_utf8(&head[..e.valid_up_to()]).ok()?,
    };
    let mut rest = text.trim_start_matches('\u{feff}');
    loop {
        rest = rest.trim_start();
        if let Some(r) = rest.strip_prefix("<?") {
            rest = &r[r.find("?>")? + 2..];
        } else if let Some(r) = rest.strip_prefix("<!--") {
            rest = &r[r.find("-->")? + 3..];
        } else if let Some(r) = rest.strip_prefix("<!") {
            // A doctype, past its internal subset when it has one.
            let end = match (r.find('['), r.find('>')) {
                (Some(open), Some(close)) if open < close => r[open..].find("]>")? + open + 1,
                (_, close) => close?,
            };
            rest = &r[end + 1..];
        } else {
            break;
        }
    }
    let tag = rest.strip_prefix("<svg")?;
    if !tag.starts_with(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/') {
        return None;
    }
    let tag = &tag[..tag.find('>')?];
    let (w, h) = (attr(tag, "width").and_then(px), attr(tag, "height").and_then(px));
    let view = attr(tag, "viewBox").and_then(|v| {
        let n: Vec<f64> = v.split(|c: char| c.is_ascii_whitespace() || c == ',').filter(|s| !s.is_empty())
            .map(str::parse).collect::<Result<_, _>>().ok()?;
        match n[..] {
            [_, _, vw, vh] if vw > 0.0 && vh > 0.0 && vw.is_finite() && vh.is_finite() => Some((vw, vh)),
            _ => None,
        }
    });
    let (w, h) = match (w, h, view) {
        (Some(w), Some(h), _) => (w, h),
        (Some(w), None, Some((vw, vh))) => (w, w * vh / vw),
        (None, Some(h), Some((vw, vh))) => (h * vw / vh, h),
        (None, None, Some(view)) => view,
        (w, h, None) => (w.unwrap_or(300.0), h.unwrap_or(150.0)),
    };
    let dim = |v: f64| v.round().clamp(1.0, MAX_COORD) as u32;
    Some((dim(w), dim(h)))
}

/// The value of attribute `name` in the inside of a start tag.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag;
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == '/');
        let eq = rest.find('=')?;
        let key = rest[..eq].trim();
        let after = rest[eq + 1..].trim_start();
        let quote = after.chars().next().filter(|q| *q == '"' || *q == '\'')?;
        let end = after[1..].find(quote)? + 1;
        if key == name {
            return Some(&after[1..end]);
        }
        rest = &after[end + 1..];
    }
}

/// A length in px: unitless or `px`, above zero.
fn px(value: &str) -> Option<f64> {
    let value = value.trim();
    let n: f64 = value.strip_suffix("px").unwrap_or(value).trim().parse().ok()?;
    (n.is_finite() && n > 0.0).then_some(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pic(ext: &'static str, w: u32, h: u32) -> Option<Picture> {
        Some(Picture { ext, w, h })
    }

    #[test]
    fn rasters_are_sized_from_their_headers() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend(640u32.to_be_bytes());
        png.extend(480u32.to_be_bytes());
        assert_eq!(sniff(&png), pic("png", 640, 480));
        assert_eq!(sniff(&png[..20]), None, "a PNG cut short has no size");

        let gif = [b"GIF89a".as_slice(), &[0x20, 0x01, 0x10, 0x00]].concat();
        assert_eq!(sniff(&gif), pic("gif", 288, 16));

        // SOI, an APP0 segment of 16 bytes, then SOF0: length, precision, height, width.
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        jpeg.extend([0u8; 14]);
        jpeg.extend([0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x96, 0x01, 0x2C]);
        assert_eq!(sniff(&jpeg), pic("jpg", 300, 150));
        // A DHT (0xC4) before the frame is skipped, not read as one.
        let mut dht = vec![0xFF, 0xD8, 0xFF, 0xC4, 0x00, 0x04, 0x00, 0x00];
        dht.extend([0xFF, 0xC2, 0x00, 0x11, 0x08, 0x00, 0x02, 0x00, 0x03]);
        assert_eq!(sniff(&dht), pic("jpg", 3, 2));
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xDA, 0x00]), None, "scan data before any frame header");

        let riff = |chunk: &[u8], data: &[u8]| [b"RIFF\0\0\0\0WEBP".as_slice(), chunk, b"\0\0\0\0", data].concat();
        let lossy = riff(b"VP8 ", &[0, 0, 0, 0x9D, 0x01, 0x2A, 0x90, 0x01, 0x2C, 0x01]);
        assert_eq!(sniff(&lossy), pic("webp", 400, 300));
        // 14 bits of width - 1 (99), then 14 of height - 1 (49).
        let bits: u32 = 99 | 49 << 14;
        let lossless = riff(b"VP8L", &[[0x2F].as_slice(), &bits.to_le_bytes()].concat());
        assert_eq!(sniff(&lossless), pic("webp", 100, 50));
        let extended = riff(b"VP8X", &[0, 0, 0, 0, 0xFF, 0x03, 0x00, 0xFF, 0x01, 0x00]);
        assert_eq!(sniff(&extended), pic("webp", 1024, 512));
    }

    #[test]
    fn a_picture_is_whole_only_when_it_reaches_its_formats_end() {
        let whole_of = |bytes: &[u8]| whole(bytes, &sniff(bytes).unwrap());
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend([0, 0, 0, 2, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0, 0, 0, 0]);
        assert!(!whole_of(&png), "no IEND");
        png.extend(b"\0\0\0\0IEND\xAE\x42\x60\x82");
        assert!(whole_of(&png));
        assert!(whole_of(&[png.as_slice(), &[0; 100]].concat()), "a little after IEND");

        // SOI; an APP1 holding a thumbnail with its own scan and end; SOF0; SOS; scan data, with
        // a stuffed FF 00; then the end of the image.
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x0A, 0xFF, 0xD8, 0xFF, 0xDA, 0xFF, 0xD9, 0x00, 0x00];
        jpeg.extend([0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x96, 0x01, 0x2C]);
        jpeg.extend([0u8; 10]);
        jpeg.extend([0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
        jpeg.extend([0x12, 0xFF, 0x00, 0x34, 0xFF, 0xD3, 0x56]);
        assert_eq!(sniff(&jpeg), pic("jpg", 300, 150));
        assert!(!whole_of(&jpeg), "cut short in the scan: the thumbnail's end does not count");
        jpeg.extend([0xFF, 0xD9]);
        assert!(whole_of(&jpeg));
        assert!(whole_of(&[jpeg.as_slice(), &[0x66; 5000]].concat()), "a motion photo's video after the end");

        let gif = [b"GIF89a".as_slice(), &[0x20, 0x01, 0x10, 0x00, 0x2C, 0x01]].concat();
        assert!(!whole_of(&gif));
        assert!(whole_of(&[gif.as_slice(), &[0x3B, 0, 0]].concat()), "the trailer, then padding");

        let lossy = |size: u32, data: &[u8]| [b"RIFF".as_slice(), &size.to_le_bytes(), b"WEBPVP8 \0\0\0\0", data].concat();
        let data = [0, 0, 0, 0x9D, 0x01, 0x2A, 0x90, 0x01, 0x2C, 0x01];
        assert!(whole_of(&lossy(4 + 8 + data.len() as u32, &data)));
        assert!(!whole_of(&lossy(4 + 8 + data.len() as u32 + 100, &data)), "a RIFF size past the bytes");
    }

    #[test]
    fn svg_is_sized_from_its_root_tag() {
        let svg = |text: &str| sniff(text.as_bytes());
        assert_eq!(svg(r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80px"/>"#), pic("svg", 120, 80));
        assert_eq!(svg("\u{feff}<?xml version=\"1.0\"?>\n<!-- logo -->\n<!DOCTYPE svg [ <!ENTITY a \"b\"> ]>\n<svg viewBox='0 0 64 32'></svg>"), pic("svg", 64, 32));
        assert_eq!(svg(r#"<svg width="200" viewBox="0,0,100,50"/>"#), pic("svg", 200, 100), "the other side from the aspect");
        assert_eq!(svg(r#"<svg stroke-width="9" width="100%"><rect/></svg>"#), pic("svg", 300, 150), "neither is in px");
        assert_eq!(svg("<svg>"), pic("svg", 300, 150));
        assert_eq!(svg("<svgx width='1'>"), None);
        assert_eq!(svg("<html><svg/></html>"), None, "the first tag must be the svg");
        assert_eq!(svg("hello"), None);
        assert_eq!(sniff(&[0u8, 1, 2, 3]), None);
    }
}
