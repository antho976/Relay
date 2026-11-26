//! Pictures saved from the reader (the owner, 2026-11-25: "you like watching the art"). A long
//! press on a page offers Save picture: what of that page is on screen is cut out, kept here
//! (`panels/p<id>.jpg`, listed in `panels/index.json`) and handed to the phone's gallery
//! ([`crate::app::Host::save_picture`]: Pictures/Leaf). The series page shows the ones saved from
//! it; a tap opens one full screen ([`crate::zoom`], as the cover).
//!
//! The cut and the encoding run on a thread of their own: a page is a big picture.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The cut is kept no bigger than this on its longer side (pixels).
pub const MAX_SIDE: u32 = 2000;
/// JPEG quality of what is kept.
const QUALITY: u8 = 90;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Panel {
    pub id: i64,
    pub series: i64,
    pub title: String,
    pub chapter: f64,
    pub page: u32,
    pub at_ms: i64,
}

fn dir(data: &Path) -> PathBuf {
    data.join("panels")
}

pub fn file(data: &Path, id: i64) -> PathBuf {
    dir(data).join(format!("p{id}.jpg"))
}

/// Every picture saved, newest first.
pub fn list(data: &Path) -> Vec<Panel> {
    let mut v: Vec<Panel> = std::fs::read(dir(data).join("index.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    v.retain(|p| file(data, p.id).is_file());
    v.sort_by(|a, b| b.at_ms.cmp(&a.at_ms).then(b.id.cmp(&a.id)));
    v
}

fn write_list(data: &Path, v: &[Panel]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir(data))?;
    let tmp = dir(data).join("index.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(v).unwrap_or_default())?;
    std::fs::rename(tmp, dir(data).join("index.json"))
}

/// The rows `top`..`bottom` (shares of its height, 0 to 1) of a page's picture, kept no bigger than
/// [`MAX_SIDE`], as JPEG.
pub fn cut(bytes: &[u8], top: f32, bottom: f32) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("This page didn't decode: {e}"))?.to_rgb8();
    let (w, h) = img.dimensions();
    let (t, b) = (top.clamp(0.0, 1.0), bottom.clamp(0.0, 1.0));
    let y0 = (t * h as f32).floor() as u32;
    let y1 = ((b * h as f32).ceil() as u32).clamp(y0 + 1, h.max(1));
    let part = image::imageops::crop_imm(&img, 0, y0.min(h.saturating_sub(1)), w, y1 - y0.min(h.saturating_sub(1))).to_image();
    let (pw, ph) = part.dimensions();
    let part = if pw.max(ph) > MAX_SIDE {
        let s = MAX_SIDE as f32 / pw.max(ph) as f32;
        image::imageops::resize(&part, ((pw as f32 * s) as u32).max(1), ((ph as f32 * s) as u32).max(1), image::imageops::FilterType::Triangle)
    } else {
        part
    };
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, QUALITY).encode_image(&part).map_err(|e| format!("Couldn't save it: {e}"))?;
    Ok(out)
}

/// Keep a cut: its file and its row in the list. Answers the new picture.
pub fn keep(data: &Path, mut p: Panel, jpeg: &[u8]) -> Result<Panel, String> {
    let mut all = list(data);
    p.id = all.iter().map(|x| x.id).max().unwrap_or(0) + 1;
    std::fs::create_dir_all(dir(data)).map_err(|e| e.to_string())?;
    std::fs::write(file(data, p.id), jpeg).map_err(|e| format!("Couldn't save it: {e}"))?;
    all.push(p.clone());
    write_list(data, &all).map_err(|e| e.to_string())?;
    Ok(p)
}

/// Let one go (from Leaf only: the gallery keeps its copy).
pub fn remove(data: &Path, id: i64) {
    let mut all = list(data);
    all.retain(|p| p.id != id);
    let _ = std::fs::remove_file(file(data, id));
    let _ = write_list(data, &all);
}

/// The page's bytes as this phone holds them: downloaded, else the page cache.
pub fn page_bytes(data: &Path, chapter: i64, n: u32) -> Option<Vec<u8>> {
    [crate::offline::chapter_dir(data, chapter), crate::images::chapter_dir(data, chapter)].iter().find_map(|d| std::fs::read(d.join(n.to_string())).ok().filter(|b| !b.is_empty()))
}

/// The gallery's name for a picture: "Nano Machine ch. 140 p. 3".
pub fn gallery_name(p: &Panel) -> String {
    let safe: String = p.title.chars().map(|c| if c.is_alphanumeric() || c == ' ' { c } else { ' ' }).collect();
    format!("{} ch. {} p. {}.jpg", safe.split_whitespace().collect::<Vec<_>>().join(" "), crate::logic::num(p.chapter), p.page + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |_, y| image::Rgb([(y % 255) as u8, 0, 0]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn what_was_on_screen_is_cut_kept_listed_and_let_go() {
        let jpeg = cut(&png(400, 3000), 0.5, 0.75).unwrap();
        let img = image::load_from_memory(&jpeg).unwrap();
        assert_eq!((img.width(), img.height()), (400, 750));
        let dir = std::env::temp_dir().join(format!("leaf-panels-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = Panel { id: 0, series: 3, title: "Nano: Machine".into(), chapter: 140.0, page: 2, at_ms: 5 };
        let a = keep(&dir, p.clone(), &jpeg).unwrap();
        let b = keep(&dir, Panel { at_ms: 9, ..p }, &jpeg).unwrap();
        assert_eq!((a.id, b.id), (1, 2));
        assert_eq!(list(&dir).iter().map(|p| p.id).collect::<Vec<_>>(), vec![2, 1], "newest first");
        assert_eq!(gallery_name(&a), "Nano Machine ch. 140 p. 3.jpg");
        remove(&dir, 2);
        assert_eq!(list(&dir).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
