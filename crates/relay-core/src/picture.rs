//! Pictures Relay takes in (`design.asset.add`, `session.upload`, `thread.upload`): a picture's
//! format from its first bytes and its natural size from its header, without decoding it, a
//! picture sent as base64, decoded within what a door carries, and a picture kept on disk under
//! its content's name so that a crash never leaves half of one there.

use relay_bus::error::BusError;
use relay_bus::design::MAX_COORD;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Mutex;

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

/// Keep `bytes` as `dir/name`, where the name is made from their hash, durably (B51): a copy
/// already there whole is left as it is, its time set to now (the design-assets sweep dates a
/// picture's last use by it); anything else at the name, a copy a crash cut short or a link, is
/// replaced through a temporary file synced to disk and renamed into place, then the directory
/// is synced, so a reader never sees half a picture and a crash never leaves a short one behind
/// a name every later add would trust. `true` when the copy was there already. Slow on a large
/// picture: call it with the store unlocked.
pub fn keep(dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<bool> {
    keep_in(dir, name, bytes, None)
}

/// [`keep`] in a folder a sweep removes from (`design-assets`, B66), where `files` is the lock
/// the sweep holds for its last look at a file and its removal: dating the copy found whole,
/// and renaming a new one into place, are done under it, so neither falls between the two and
/// the picture this answers for is never removed right after.
pub fn keep_swept(dir: &Path, name: &str, bytes: &[u8], files: &Mutex<()>) -> std::io::Result<bool> {
    keep_in(dir, name, bytes, Some(files))
}

fn keep_in(dir: &Path, name: &str, bytes: &[u8], files: Option<&Mutex<()>>) -> std::io::Result<bool> {
    let hold = || files.map(|files| files.lock().unwrap_or_else(|poisoned| poisoned.into_inner()));
    let path = dir.join(name);
    if let Some(file) = holding(&path, bytes) {
        // Read with nothing held; dated under the lock only while it is still the file at the
        // name, which a sweep may have removed meanwhile.
        let _files = hold();
        if same_file(&file, &path) {
            let _ = file.set_modified(std::time::SystemTime::now());
            return Ok(true);
        }
    }
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    // A rename that replaces: the same name holds the same bytes, so a copy another request
    // stored meanwhile is replaced by an identical one, and a reader keeps the file it opened.
    {
        let _files = hold();
        tmp.persist(&path).map_err(|error| error.error)?;
    }
    // The rename itself through a crash; best effort, as the store's own files are.
    let _ = std::fs::File::open(dir).and_then(|dir| dir.sync_all());
    Ok(false)
}

/// Whether `path` still names the file `file` has open.
fn same_file(file: &std::fs::File, path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (file.metadata(), std::fs::symlink_metadata(path)) {
        (Ok(open), Ok(named)) => open.dev() == named.dev() && open.ino() == named.ino(),
        _ => false,
    }
}

/// `path`, open, when it is a regular file holding exactly `bytes`. Opened without following a
/// link or waiting on a pipe: an agent owns the checkout its uploads go to.
fn holding(path: &Path, bytes: &[u8]) -> Option<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() || meta.len() != bytes.len() as u64 {
        return None;
    }
    let mut held = Vec::with_capacity(bytes.len());
    (&file).take(bytes.len() as u64 + 1).read_to_end(&mut held).ok()?;
    (held == bytes).then_some(file)
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
/// the range), walking the segments before it; its sides swapped when an Exif segment before it
/// says the picture is shown turned a quarter (orientations 5 to 8, as a phone tags a portrait
/// photo stored landscape), so the size is the one it is shown at (TM-24).
fn jpeg(b: &[u8]) -> Option<(u32, u32)> {
    let mut at = 2;
    let mut turned = false;
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
                let (w, h) = (be16(b, at + 5)?, be16(b, at + 3)?);
                return Some(if turned { (h, w) } else { (w, h) });
            }
            _ => {
                let len = be16(b, at)? as usize;
                if len < 2 {
                    return None;
                }
                if marker == 0xE1 {
                    let segment = b.get(at + 2..(at + len).min(b.len())).unwrap_or_default();
                    if let Some(orientation) = exif_orientation(segment) {
                        turned = (5..=8).contains(&orientation);
                    }
                }
                at += len;
            }
        }
    }
}

/// The orientation (tag 0x0112 of IFD0, 1 to 8) an APP1 segment's Exif data gives, if it is
/// Exif and gives one.
fn exif_orientation(segment: &[u8]) -> Option<u16> {
    let tiff = segment.strip_prefix(b"Exif\0\0")?;
    let big = match tiff.get(0..2)? {
        b"MM" => true,
        b"II" => false,
        _ => return None,
    };
    let u16_at = |at: usize| -> Option<u16> {
        let two: [u8; 2] = tiff.get(at..at.checked_add(2)?)?.try_into().ok()?;
        Some(if big { u16::from_be_bytes(two) } else { u16::from_le_bytes(two) })
    };
    let u32_at = |at: usize| -> Option<u32> {
        let four: [u8; 4] = tiff.get(at..at.checked_add(4)?)?.try_into().ok()?;
        Some(if big { u32::from_be_bytes(four) } else { u32::from_le_bytes(four) })
    };
    if u16_at(2)? != 42 {
        return None;
    }
    let ifd = u32_at(4)? as usize;
    // 12 bytes an entry, after a count of them; a SHORT's value sits in the first two bytes of
    // the entry's value field.
    for entry in (0..u16_at(ifd)? as usize).map(|i| ifd + 2 + i * 12) {
        if u16_at(entry)? == 0x0112 {
            return (u16_at(entry + 2)? == 3).then(|| u16_at(entry + 8)).flatten();
        }
    }
    None
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

    /// A JPEG of `w` by `h` stored, whose Exif says it is shown at `orientation`, in `order`'s
    /// byte order (`II` or `MM`).
    fn exif_jpeg(w: u16, h: u16, orientation: u16, order: &[u8; 2]) -> Vec<u8> {
        let big = order == b"MM";
        let n16 = |v: u16| if big { v.to_be_bytes() } else { v.to_le_bytes() };
        let n32 = |v: u32| if big { v.to_be_bytes() } else { v.to_le_bytes() };
        // The TIFF header, IFD0 at 8 with two entries (an ImageWidth first, so the walk steps
        // over one), and no next IFD.
        let entry = |tag: u16, value: u16| [n16(tag).as_slice(), &n16(3), &n32(1), &n16(value), &[0, 0]].concat();
        let mut tiff = [order.as_slice(), &n16(42), &n32(8), &n16(2)].concat();
        tiff.extend(entry(0x0100, w));
        tiff.extend(entry(0x0112, orientation));
        tiff.extend([0u8; 4]);
        let app1 = [b"Exif\0\0".as_slice(), &tiff].concat();
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1];
        jpeg.extend((app1.len() as u16 + 2).to_be_bytes());
        jpeg.extend(app1);
        jpeg.extend([0xFF, 0xC0, 0x00, 0x11, 0x08]);
        jpeg.extend(h.to_be_bytes());
        jpeg.extend(w.to_be_bytes());
        jpeg
    }

    /// TM-24: a phone's portrait photo is stored landscape and tagged to be shown turned; its
    /// size is the one it is shown at, as every viewer shows it.
    #[test]
    fn a_jpeg_turned_by_its_exif_is_sized_as_shown() {
        for order in [b"II", b"MM"] {
            assert_eq!(sniff(&exif_jpeg(200, 100, 6, order)), pic("jpg", 100, 200), "{order:?}");
            assert_eq!(sniff(&exif_jpeg(200, 100, 8, order)), pic("jpg", 100, 200));
            assert_eq!(sniff(&exif_jpeg(200, 100, 5, order)), pic("jpg", 100, 200), "turned and mirrored");
            assert_eq!(sniff(&exif_jpeg(200, 100, 1, order)), pic("jpg", 200, 100), "upright");
            assert_eq!(sniff(&exif_jpeg(200, 100, 3, order)), pic("jpg", 200, 100), "upside down keeps its sides");
        }
        // Exif cut short, or not Exif at all: the size as stored.
        let mut cut = exif_jpeg(200, 100, 6, b"II");
        cut[14] = b'X';
        assert_eq!(sniff(&cut), pic("jpg", 200, 100), "a TIFF header that is not one");
        assert_eq!(exif_orientation(b"Exif\0\0II*\0\x08\0\0\0\x05\0"), None, "entries past the end");
        assert_eq!(exif_orientation(b"http://ns.adobe.com/xap/1.0/\0"), None);
    }

    /// B65's fix is in design.asset.add: the uploads a model reads keep the size a header
    /// says, however large.
    #[test]
    fn a_raster_larger_than_a_node_is_sized_as_it_says() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend(4_000_000_000u32.to_be_bytes());
        png.extend(1u32.to_be_bytes());
        assert_eq!(sniff(&png), pic("png", 4_000_000_000, 1));
    }

    /// B51: a copy a crash cut short is replaced, a whole one is kept and dated now, and what
    /// sits at the name without being the picture (a link) is replaced, never written through.
    #[test]
    fn a_picture_is_kept_whole() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = b"\x89PNG\r\n\x1a\n picture".to_vec();
        assert!(!keep(dir.path(), "a.png", &bytes).unwrap(), "new");
        let path = dir.path().join("a.png");
        assert_eq!(std::fs::read(&path).unwrap(), bytes);

        let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(946_684_800);
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(old).unwrap();
        assert!(keep(dir.path(), "a.png", &bytes).unwrap(), "already there");
        assert!(std::fs::metadata(&path).unwrap().modified().unwrap() > old, "seen now");

        for torn in [&b""[..], &bytes[..5], &[0u8; 16][..]] {
            std::fs::write(&path, torn).unwrap();
            assert!(!keep(dir.path(), "a.png", &bytes).unwrap(), "{torn:?} is not the picture");
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }

        let elsewhere = dir.path().join("elsewhere");
        std::fs::write(&elsewhere, &bytes).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &path).unwrap();
        assert!(!keep(dir.path(), "a.png", &bytes).unwrap(), "a link is not a copy");
        assert!(!std::fs::symlink_metadata(&path).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read(&elsewhere).unwrap(), bytes, "the link's target is untouched");
        let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(left.len(), 2, "no temporary file stays: {left:?}");
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
