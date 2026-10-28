//! Pictures in a thread (docs/THREADS.md): those the person adds to the message box, and those
//! their messages show.
//!
//! A picture is chosen, pasted or dropped, made fit to send on a worker thread ([`prepare`]: at
//! most 2000 pixels on its long side and 4 MiB, as PNG for a screenshot and JPEG for a photo; one
//! that fits already goes as it is; one pasted is taken from its pixels, [`prepare_pasted`]), and
//! goes up at once (`thread.upload`), so sending only names it. In the message box it is a chip
//! ([`Attached`]): its thumbnail, a spinner while it goes up, a red mark and the reason when it was
//! refused, and a key that takes it out.
//!
//! A message's pictures are read back by id (`thread.image`) as thumbnails kept in memory, so a
//! redraw or a scroll does not read them again ([`thumbnail`]); a click opens one larger in a
//! window of its own ([`open_large`]). Pictures travel on a connection of their own
//! (`Client::bulk_request`): one of 4 MiB is past the control connection's 2 MiB lines.
use crate::app::{label, Ui};
use crate::client::{Client, Error};
use base64::Engine as _;
use gtk::gdk_pixbuf::{InterpType, Pixbuf, PixbufLoader};
use gtk::prelude::*;
use gtk::{gdk, gio};
use gtk4 as gtk;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime};

/// What `thread.upload` takes; the chooser offers these.
pub(super) const MEDIA_TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];
/// The file names a chooser, a drop or a paste takes as pictures.
pub(super) const SUFFIXES: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];
/// Pictures in one message.
pub(super) const MAX_PICTURES: usize = 4;
/// The longest side a picture goes up at. Past 20 pictures in a request (each turn sends the thread's
/// earlier ones again) the API refuses any side longer than 2000 pixels.
const MAX_SIDE: u32 = 2000;
/// The most `thread.upload` takes, decoded.
const MAX_BYTES: usize = 4 * 1024 * 1024;
/// A file past this is not read at all.
const MAX_FILE: u64 = 64 * 1024 * 1024;
/// Past this a picture is not decoded, as the editor's preview refuses one.
const MAX_PIXELS: u64 = 100_000_000;
/// A thumbnail's long side: a message shows one at most 240 pixels wide, at twice the scale.
const THUMB_EDGE: u32 = 480;
/// Thumbnails kept by id; past this the one read longest ago goes.
const KEPT: usize = 96;
/// A chip's square in the message box.
const CHIP: i32 = 56;

/// What a model on this PC says to a picture.
pub(super) const LOCAL_CANNOT: &str = "This model on your PC cannot read pictures: choose a Claude model";
const NOT_A_PICTURE: &str = "This is not a PNG, JPEG, GIF or WebP picture";
/// What a picture says once the engine has let it go.
const EXPIRED: &str = "Not sent within a day, so Relay let it go: add it again";
/// How long the engine keeps a picture no message carries (`threads::images::UNSENT`).
const UNSENT: Duration = Duration::from_secs(24 * 60 * 60);

/// Whether a picture that went up at `up_at` has been let go by `now`; one with no time, put back
/// from a sent message, is carried by it.
fn expired(up_at: Option<SystemTime>, now: SystemTime) -> bool {
    up_at.and_then(|at| now.duration_since(at).ok()).is_some_and(|age| age >= UNSENT)
}

/// A picture on its way into the message box.
pub(super) enum Source {
    /// A file chosen, dropped, or copied in a file manager.
    Path(PathBuf),
    /// A picture pasted, or dragged out of another app.
    Texture(gdk::Texture),
}

impl Source {
    fn name(&self) -> String {
        match self {
            Source::Path(path) => path.file_name().map_or_else(|| String::from("Picture"), |n| n.to_string_lossy().to_string()),
            Source::Texture(_) => String::from("Pasted picture"),
        }
    }
}

/// Whether a file's name says it is a picture a thread takes.
pub(super) fn is_picture(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| SUFFIXES.contains(&e.to_ascii_lowercase().as_str()))
}

/// A refusal in the person's words: the engine's own sentence, and what to do about a picture a
/// model on this PC cannot read or an engine too old to take one.
pub(super) fn said(e: &Error) -> String {
    match e {
        Error::Bus(b) if b.code == "thread.images_unsupported" => LOCAL_CANNOT.to_string(),
        Error::Bus(b) if b.code == "thread.image_unknown" => EXPIRED.to_string(),
        e => super::kit::said(e, "This engine does not handle pictures yet: update Relay's engine"),
    }
}

/// The type a picture's first bytes say it is, of those a thread takes.
fn media_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        None
    }
}

/// A JPEG's Exif orientation (1 is upright), when it carries one.
fn exif_orientation(bytes: &[u8]) -> Option<u16> {
    let mut at = 2;
    loop {
        if *bytes.get(at)? != 0xff {
            return None;
        }
        while *bytes.get(at)? == 0xff {
            at += 1;
        }
        let marker = *bytes.get(at)?;
        // The image data starts, or ends: no Exif came before it.
        if matches!(marker, 0xd9 | 0xda) {
            return None;
        }
        let length = usize::from(u16::from_be_bytes([*bytes.get(at + 1)?, *bytes.get(at + 2)?]));
        if marker == 0xe1 {
            if let Some(tiff) = bytes.get(at + 3..at + 1 + length).and_then(|s| s.strip_prefix(b"Exif\0\0")) {
                return tiff_orientation(tiff);
            }
        }
        at += 1 + length;
    }
}

/// The Orientation tag (0x0112) of a TIFF header's first directory.
fn tiff_orientation(tiff: &[u8]) -> Option<u16> {
    let big = match tiff.get(..2)? {
        b"MM" => true,
        b"II" => false,
        _ => return None,
    };
    let int = |at: usize, len: usize| -> Option<usize> {
        let field = tiff.get(at..at.checked_add(len)?)?;
        let fold = |n: usize, byte: &u8| n << 8 | usize::from(*byte);
        Some(if big { field.iter().fold(0, fold) } else { field.iter().rev().fold(0, fold) })
    };
    let directory = int(4, 4)?;
    for entry in 0..int(directory, 2)?.min(256) {
        let at = directory + 2 + entry * 12;
        if int(at, 2)? == 0x0112 {
            return u16::try_from(int(at + 8, 2)?).ok();
        }
    }
    None
}

/// Whether a picture `width` × `height` of `len` bytes is drawn again before it goes: when it is
/// longer than [`MAX_SIDE`], heavier than [`MAX_BYTES`], or a photo held sideways by its Exif.
fn redrawn(width: u64, height: u64, len: usize, upright: bool) -> bool {
    width.max(height) > u64::from(MAX_SIDE) || len > MAX_BYTES || !upright
}

/// `bytes` decoded no larger than `edge` on its long side, once their header shows a picture a
/// thread takes, of a size worth decoding: nothing else reaches whatever decoder is installed.
fn decode(bytes: &[u8], edge: u32) -> Result<Pixbuf, String> {
    media_type(bytes).ok_or(NOT_A_PICTURE)?;
    let (width, height) = crate::editor::image_preview::header_size(bytes)?.ok_or(NOT_A_PICTURE)?;
    if width.saturating_mul(height) > MAX_PIXELS {
        return Err(String::from("It is larger than 100 megapixels"));
    }
    let loader = PixbufLoader::new();
    loader.connect_size_prepared(move |loader, width, height| {
        let scale = (f64::from(edge) / f64::from(width.max(height).max(1))).min(1.0);
        if scale < 1.0 {
            loader.set_size(((f64::from(width) * scale).round() as i32).max(1), ((f64::from(height) * scale).round() as i32).max(1));
        }
    });
    let written = loader.write(bytes).map_err(|e| e.to_string());
    let closed = loader.close().map_err(|e| e.to_string());
    written?;
    closed?;
    let pixbuf = loader.pixbuf().ok_or("Its format could not be decoded")?;
    Ok(pixbuf.apply_embedded_orientation().unwrap_or(pixbuf))
}

/// Whether this PC has a loader for `media_type`: WebP's comes in a package of its own.
fn can_draw(media_type: &str) -> bool {
    Pixbuf::formats().iter().any(|format| format.mime_types().iter().any(|m| m == media_type))
}

/// `pixbuf` no larger than `edge` on its long side.
fn scaled(pixbuf: &Pixbuf, edge: u32) -> Pixbuf {
    let long = pixbuf.width().max(pixbuf.height()).max(1);
    let edge = i32::try_from(edge).unwrap_or(i32::MAX);
    if long <= edge {
        return pixbuf.clone();
    }
    let scale = f64::from(edge) / f64::from(long);
    let (width, height) = (((f64::from(pixbuf.width()) * scale).round() as i32).max(1), ((f64::from(pixbuf.height()) * scale).round() as i32).max(1));
    pixbuf.scale_simple(width, height, InterpType::Bilinear).unwrap_or_else(|| pixbuf.clone())
}

/// A redrawn picture as PNG when it came as one (a screenshot, a GIF) or has transparency, if that
/// fits; else as a JPEG, any transparency on white.
fn encode(pixbuf: &Pixbuf, from: &str) -> Result<(Vec<u8>, &'static str), String> {
    if matches!(from, "image/png" | "image/gif") || pixbuf.has_alpha() {
        let png = png(pixbuf);
        if png.len() <= MAX_BYTES {
            return Ok((png, "image/png"));
        }
    }
    jpeg(pixbuf)
}

/// `pixbuf` as PNG, by GDK's own writer: about four times quicker than gdk-pixbuf's at its default
/// (68 ms against 261 for a 1600x1200 photo), for a file about 1.5 % larger.
fn png(pixbuf: &Pixbuf) -> Vec<u8> {
    let format = if pixbuf.has_alpha() { gdk::MemoryFormat::R8g8b8a8 } else { gdk::MemoryFormat::R8g8b8 };
    let stride = pixbuf.rowstride().max(0) as usize;
    gdk::MemoryTexture::new(pixbuf.width(), pixbuf.height(), format, &pixbuf.read_pixel_bytes(), stride).save_to_png_bytes().to_vec()
}

/// `pixbuf` as a JPEG within [`MAX_BYTES`], any transparency on white.
fn jpeg(pixbuf: &Pixbuf) -> Result<(Vec<u8>, &'static str), String> {
    let flat = if pixbuf.has_alpha() {
        pixbuf.composite_color_simple(pixbuf.width(), pixbuf.height(), InterpType::Nearest, 255, 8, 0xffffff, 0xffffff).unwrap_or_else(|| pixbuf.clone())
    } else {
        pixbuf.clone()
    };
    for quality in ["88", "75", "60"] {
        let jpeg = flat.save_to_bufferv("jpeg", &[("quality", quality)]).map_err(|e| e.to_string())?;
        if jpeg.len() <= MAX_BYTES {
            return Ok((jpeg, "image/jpeg"));
        }
    }
    Err(String::from("It is too large to send, even made smaller"))
}

/// Decoded pixels, handed from a worker thread to the main loop, which makes the texture.
pub(super) struct Raw {
    pixels: Vec<u8>,
    width: i32,
    height: i32,
    stride: usize,
    alpha: bool,
}

impl Raw {
    fn of(pixbuf: &Pixbuf) -> Raw {
        Raw {
            pixels: pixbuf.read_pixel_bytes().as_ref().to_vec(),
            width: pixbuf.width(),
            height: pixbuf.height(),
            stride: pixbuf.rowstride().max(0) as usize,
            alpha: pixbuf.has_alpha(),
        }
    }

    fn texture(self) -> gdk::Texture {
        let format = if self.alpha { gdk::MemoryFormat::R8g8b8a8 } else { gdk::MemoryFormat::R8g8b8 };
        gdk::MemoryTexture::new(self.width, self.height, format, &glib::Bytes::from_owned(self.pixels), self.stride).upcast()
    }
}

/// A picture made fit to send: what goes up, its type, and its thumbnail when this PC can draw it.
pub(super) struct Prepared {
    bytes: Vec<u8>,
    media_type: &'static str,
    thumb: Option<Raw>,
}

/// Make a picture fit to send (on a worker thread): as it is when it fits, else upright, at most
/// [`MAX_SIDE`] on its long side and [`MAX_BYTES`] heavy.
fn prepare(bytes: Vec<u8>) -> Result<Prepared, String> {
    let media_type = media_type(&bytes).ok_or(NOT_A_PICTURE)?;
    let (width, height) = crate::editor::image_preview::header_size(&bytes)?.ok_or(NOT_A_PICTURE)?;
    let upright = media_type != "image/jpeg" || exif_orientation(&bytes).is_none_or(|o| o == 1);
    let redraw = redrawn(width, height, bytes.len(), upright);
    let pixbuf = match decode(&bytes, MAX_SIDE) {
        Ok(pixbuf) => pixbuf,
        // One this PC cannot draw (a WebP without its loader) still goes when it needs no
        // redrawing; its chip has no thumbnail. One it can draw but not read is damaged.
        Err(_) if !redraw && !can_draw(media_type) => return Ok(Prepared { bytes, media_type, thumb: None }),
        Err(why) => return Err(format!("It could not be read: {why}")),
    };
    let thumb = Some(Raw::of(&scaled(&pixbuf, THUMB_EDGE)));
    if !redraw {
        return Ok(Prepared { bytes, media_type, thumb });
    }
    let (bytes, media_type) = encode(&pixbuf, media_type)?;
    Ok(Prepared { bytes, media_type, thumb })
}

/// Make a pasted or dragged-in picture fit to send (on a worker thread). It comes decoded, so its
/// pixels are taken as they are, at most [`MAX_SIDE`] on the long side, rather than written out as
/// a PNG at full size and read back.
fn prepare_pasted(texture: &gdk::Texture) -> Result<Prepared, String> {
    let (width, height) = (texture.width(), texture.height());
    if (width.max(0) as u64).saturating_mul(height.max(0) as u64) > MAX_PIXELS {
        return Err(String::from("It is larger than 100 megapixels"));
    }
    let download = |format: gdk::MemoryFormat| {
        let mut downloader = gdk::TextureDownloader::new(texture);
        downloader.set_format(format);
        downloader.download_bytes()
    };
    // RGBA, unless every pixel is opaque: a PNG then carries no alpha, and a JPEG may serve.
    let (mut bytes, mut stride) = download(gdk::MemoryFormat::R8g8b8a8);
    let alpha = bytes.chunks_exact(stride).any(|row| row[..width as usize * 4].as_chunks::<4>().0.iter().any(|px| px[3] != 255));
    if !alpha {
        (bytes, stride) = download(gdk::MemoryFormat::R8g8b8);
    }
    let full = Pixbuf::from_bytes(&bytes, gtk::gdk_pixbuf::Colorspace::Rgb, alpha, 8, width, height, stride as i32);
    let pixbuf = scaled(&full, MAX_SIDE);
    let thumb = Some(Raw::of(&scaled(&pixbuf, THUMB_EDGE)));
    let (bytes, media_type) = encode_pasted(&pixbuf)?;
    Ok(Prepared { bytes, media_type, thumb })
}

/// A pasted picture's bytes: PNG when it packs as a screenshot does, under a byte a pixel, which
/// keeps its text sharp, or when it has transparency, if that fits; else a JPEG, as a photo goes.
fn encode_pasted(pixbuf: &Pixbuf) -> Result<(Vec<u8>, &'static str), String> {
    let png = png(pixbuf);
    let pixels = (pixbuf.width().max(0) as usize) * (pixbuf.height().max(0) as usize);
    if png.len() <= MAX_BYTES && (pixbuf.has_alpha() || png.len() <= pixels) {
        return Ok((png, "image/png"));
    }
    jpeg(pixbuf)
}

/// Read a chosen or dropped file and make it fit to send (on a worker thread).
fn prepare_file(path: &Path) -> Result<Prepared, String> {
    let size = std::fs::metadata(path).map_err(|e| format!("It could not be read: {e}"))?.len();
    if size > MAX_FILE {
        return Err(String::from("It is larger than 64 MiB"));
    }
    prepare(std::fs::read(path).map_err(|e| format!("It could not be read: {e}"))?)
}

/// Where a picture in the message box stands.
enum Upload {
    /// Being made fit, then going up.
    Going,
    /// Up: the engine's `{id, media_type, width, height}`.
    Ready(Value),
    /// Refused, and why.
    Refused(String),
}

thread_local! {
    static SERIAL: Cell<u64> = const { Cell::new(0) };
}

/// A picture in the message box: its chip, and whether it is up. The chip stays with it while its
/// thread's draft is put away, so an upload that ends meanwhile shows when the thread comes back.
pub(super) struct Attached {
    pub(super) serial: u64,
    root: gtk::Overlay,
    face: gtk::Overlay,
    picture: gtk::Picture,
    glyph: gtk::Image,
    state: gtk::Box,
    name: String,
    upload: RefCell<Upload>,
    thumb: RefCell<Option<gdk::Texture>>,
    /// When it went up, by the wall clock the engine's sweep keeps; none when a sent message
    /// carries it.
    up_at: Cell<Option<SystemTime>>,
}

impl Attached {
    /// A chip for `name`, going up; its remove key runs `remove` with its serial.
    fn new(name: String, remove: impl Fn(u64) + 'static) -> Rc<Attached> {
        let serial = SERIAL.with(|s| {
            s.set(s.get() + 1);
            s.get()
        });
        // The face: a square the thumbnail covers, rounded; the size is the empty box's, since a
        // picture asks for its own size and would widen the strip.
        let face = gtk::Overlay::new();
        face.add_css_class("threads-chip");
        face.set_overflow(gtk::Overflow::Hidden);
        face.set_halign(gtk::Align::Start);
        face.set_valign(gtk::Align::End);
        face.set_margin_top(6);
        face.set_margin_end(6);
        let room = gtk::Box::new(gtk::Orientation::Vertical, 0);
        room.set_size_request(CHIP, CHIP);
        face.set_child(Some(&room));
        let glyph = crate::icons::image("file-image", 18);
        glyph.set_halign(gtk::Align::Center);
        glyph.set_valign(gtk::Align::Center);
        face.add_overlay(&glyph);
        let picture = gtk::Picture::new();
        picture.set_can_shrink(true);
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_alternative_text(Some(&name));
        face.add_overlay(&picture);
        let state = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        state.add_css_class("threads-chip-state");
        state.set_halign(gtk::Align::Center);
        state.set_valign(gtk::Align::Center);
        face.add_overlay(&state);
        let root = gtk::Overlay::new();
        root.add_css_class("threads-chip-slot");
        root.set_child(Some(&face));
        let key = gtk::Button::new();
        key.set_child(Some(&crate::icons::image("close", 10)));
        key.add_css_class("threads-chip-remove");
        key.set_halign(gtk::Align::End);
        key.set_valign(gtk::Align::Start);
        key.set_tooltip_text(Some("Remove this picture"));
        key.update_property(&[gtk::accessible::Property::Label("Remove this picture")]);
        key.connect_clicked(move |_| remove(serial));
        root.add_overlay(&key);
        let me = Rc::new(Attached { serial, root, face, picture, glyph, state, name, upload: RefCell::new(Upload::Going), thumb: RefCell::default(), up_at: Cell::new(None) });
        me.show_state();
        me
    }

    /// A picture added now: it is made fit on a worker thread and goes up at once. `changed` runs
    /// with it once it is up or refused.
    pub(super) fn add(ui: &Rc<Ui>, source: Source, changed: fn(&Attached), remove: impl Fn(u64) + 'static) -> Rc<Attached> {
        let me = Attached::new(source.name(), remove);
        let (ui, chip) = (ui.clone(), me.clone());
        glib::spawn_future_local(async move {
            let prepared = gio::spawn_blocking(move || match source {
                Source::Path(path) => prepare_file(&path),
                Source::Texture(texture) => prepare_pasted(&texture),
            })
            .await
            .unwrap_or_else(|_| Err(String::from("It could not be read")));
            let Prepared { bytes, media_type, thumb } = match prepared {
                Ok(prepared) => prepared,
                Err(why) => {
                    chip.finish(Upload::Refused(why));
                    changed(&chip);
                    return;
                }
            };
            if let Some(thumb) = thumb {
                chip.show_thumb(&thumb.texture());
            }
            let encoded = gio::spawn_blocking(move || base64::engine::general_purpose::STANDARD.encode(bytes)).await.unwrap_or_default();
            let answer = Client::bulk_request(&ui.rt, ui.path.clone(), "thread.upload", json!({"media_type": media_type, "bytes_b64": encoded})).await;
            let upload = match answer {
                Ok(answer) if !answer["image"]["id"].is_null() => {
                    // The message it goes in shows it from here, without reading it back.
                    if let Some(thumb) = chip.thumb.borrow().clone() {
                        remember(key(&answer["image"]["id"]), Ok(thumb));
                    }
                    chip.up_at.set(Some(SystemTime::now()));
                    Upload::Ready(answer["image"].clone())
                }
                Ok(_) => Upload::Refused(String::from("The engine did not keep it")),
                Err(e) => Upload::Refused(said(&e)),
            };
            chip.finish(upload);
            changed(&chip);
        });
        me
    }

    /// A picture already up, from a message put back in the box to change and send again.
    pub(super) fn again(ui: &Rc<Ui>, image: &Value, remove: impl Fn(u64) + 'static) -> Rc<Attached> {
        let me = Attached::new(String::from("Picture"), remove);
        *me.upload.borrow_mut() = Upload::Ready(image.clone());
        me.show_state();
        let chip = Rc::downgrade(&me);
        thumbnail(ui, &image["id"], move |read| {
            if let (Some(chip), Ok(texture)) = (chip.upgrade(), read) {
                chip.show_thumb(texture);
            }
        });
        me
    }

    pub(super) fn widget(&self) -> &gtk::Overlay {
        &self.root
    }

    /// The file's name, or "Pasted picture".
    pub(super) fn name(&self) -> &str {
        &self.name
    }

    /// The engine's picture, once it is up.
    pub(super) fn image(&self) -> Option<Value> {
        match &*self.upload.borrow() {
            Upload::Ready(image) => Some(image.clone()),
            _ => None,
        }
    }

    pub(super) fn going(&self) -> bool {
        matches!(*self.upload.borrow(), Upload::Going)
    }

    pub(super) fn refused(&self) -> Option<String> {
        match &*self.upload.borrow() {
            Upload::Refused(why) => Some(why.clone()),
            _ => None,
        }
    }

    /// A send refused for a picture the engine no longer holds: each one here that went up a day
    /// ago or more was let go unsent, and says so, rather than failing every send the same way.
    pub(super) fn expire(e: &Error, attached: &[Rc<Attached>]) {
        if !matches!(e, Error::Bus(b) if b.code == "thread.image_unknown") {
            return;
        }
        let now = SystemTime::now();
        for chip in attached.iter().filter(|chip| expired(chip.up_at.get(), now)) {
            chip.finish(Upload::Refused(String::from(EXPIRED)));
        }
    }

    fn show_thumb(&self, texture: &gdk::Texture) {
        *self.thumb.borrow_mut() = Some(texture.clone());
        self.picture.set_paintable(Some(texture));
        self.show_glyph();
    }

    /// A picture's glyph stands in for a thumbnail this PC cannot draw, once nothing else (the
    /// spinner, the refusal's mark) takes the middle of the chip.
    fn show_glyph(&self) {
        let up = matches!(*self.upload.borrow(), Upload::Ready(_));
        self.glyph.set_visible(up && self.thumb.borrow().is_none());
    }

    fn finish(&self, upload: Upload) {
        *self.upload.borrow_mut() = upload;
        self.show_state();
    }

    /// The chip's mark and words for where it stands: a spinner while it goes up, a red mark and
    /// the reason once refused, nothing once up.
    fn show_state(&self) {
        while let Some(child) = self.state.first_child() {
            self.state.remove(&child);
        }
        let mark: Option<gtk::Widget> = match &*self.upload.borrow() {
            Upload::Going => {
                let spinner = crate::app::spinner();
                spinner.set_size_request(14, 14);
                Some(spinner.upcast())
            }
            Upload::Ready(_) => None,
            Upload::Refused(_) => Some(crate::icons::image("close", 12).upcast()),
        };
        if let Some(mark) = mark {
            mark.set_hexpand(true);
            mark.set_vexpand(true);
            mark.set_halign(gtk::Align::Center);
            mark.set_valign(gtk::Align::Center);
            self.state.append(&mark);
        }
        let about = match &*self.upload.borrow() {
            Upload::Going => format!("{} · Going up…", self.name),
            Upload::Ready(_) => self.name.clone(),
            Upload::Refused(why) => format!("{} · Not added: {why}", self.name),
        };
        if self.refused().is_some() {
            self.face.add_css_class("refused");
        } else {
            self.face.remove_css_class("refused");
        }
        self.state.set_visible(self.state.first_child().is_some());
        self.show_glyph();
        self.root.set_tooltip_text(Some(&about));
        self.face.update_property(&[gtk::accessible::Property::Label(&about)]);
    }
}

/// Thumbnails kept by id, at most `limit` of them. Each carries when it was last asked for, and past
/// the limit the one asked for longest ago goes: a thread drawn whole asks oldest first, so it keeps
/// its newest pictures, the ones on screen, whatever order their reads finish in.
struct Kept<V> {
    limit: usize,
    clock: u64,
    entries: HashMap<String, (u64, V)>,
}

impl<V: Clone> Kept<V> {
    fn new(limit: usize) -> Self {
        Kept { limit, clock: 0, entries: HashMap::new() }
    }

    /// Picture `key` is asked for now: the time it is asked at, and its thumbnail if kept.
    fn ask(&mut self, key: &str) -> (u64, Option<V>) {
        self.clock += 1;
        let now = self.clock;
        (now, self.entries.get_mut(key).map(|(at, v)| {
            *at = now;
            v.clone()
        }))
    }

    fn peek(&self, key: &str) -> Option<V> {
        self.entries.get(key).map(|(_, v)| v.clone())
    }

    /// Keep `value` for `key`, asked for at `at`; past the limit, the one asked for longest ago goes,
    /// which may be this one.
    fn keep(&mut self, key: String, at: u64, value: V) {
        self.entries.insert(key, (at, value));
        while self.entries.len() > self.limit {
            let Some(oldest) = self.entries.iter().min_by_key(|(_, (at, _))| *at).map(|(k, _)| k.clone()) else { break };
            self.entries.remove(&oldest);
        }
    }
}

impl<V: Clone> Default for Kept<V> {
    fn default() -> Self {
        Kept::new(KEPT)
    }
}

/// What a read of a picture by id gave: its texture, or why not.
type Read = Result<gdk::Texture, String>;
type Waiting = Vec<Box<dyn FnOnce(&Read)>>;

/// Thumbnails read by id, and those being read: when each was last asked for, and who waits.
#[derive(Default)]
struct Seen {
    kept: Kept<Read>,
    waiting: HashMap<String, (u64, Waiting)>,
    /// Pictures asked for and not yet being read, the newest asked last: a thread drawn whole asks
    /// for its newest pictures last, and those are the ones in view, so they are read first.
    asked: Vec<Value>,
    /// How many are being read: at most [`READING`], so a thread of many pictures does not open a
    /// connection for each at once.
    reading: usize,
    /// Reading starts on the next idle, once the drawing that asks has asked for all it shows.
    starting: bool,
}

/// Thumbnails read at once.
const READING: usize = 2;

thread_local! {
    static SEEN: RefCell<Seen> = RefCell::default();
    /// The thumbnails read, by key, while a measurement watches (`scripts/perf/thumbs-reopen.py`).
    static READS: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// Start (`true`) or stop recording the thumbnails read; what was recorded since the last call.
pub(crate) fn watch_reads(on: bool) -> Vec<String> {
    READS.with(|r| std::mem::replace(&mut *r.borrow_mut(), on.then(Vec::new))).unwrap_or_default()
}

/// Whether a thumbnail is being read or waits to be.
pub(crate) fn reading() -> bool {
    SEEN.with(|seen| {
        let seen = seen.borrow();
        seen.reading > 0 || !seen.asked.is_empty()
    })
}

/// The widget name of a message's picture, for finding which show.
pub(crate) fn picture_name(id: &Value) -> String {
    format!("thread-picture-{}", key(id))
}

/// A picture's id as a key: the engine's id, whatever its JSON kind.
fn key(id: &Value) -> String {
    id.as_str().map_or_else(|| id.to_string(), str::to_string)
}

fn remember(key: String, read: Read) {
    SEEN.with(|seen| {
        let kept = &mut seen.borrow_mut().kept;
        let (at, _) = kept.ask(&key);
        kept.keep(key, at, read);
    });
}

fn cached(id: &Value) -> Option<Read> {
    SEEN.with(|seen| seen.borrow().kept.peek(&key(id)))
}

/// Read picture `id` (`thread.image`) no larger than `edge`: its texture or why not, and whether
/// that is the picture's own answer to keep. A lost connection or a timeout is not: the next
/// showing tries again.
async fn read_picture(ui: &Rc<Ui>, id: &Value, edge: u32) -> (Read, bool) {
    match Client::bulk_request(&ui.rt, ui.path.clone(), "thread.image", json!({"id": id})).await {
        Ok(answer) => {
            let decoded = gio::spawn_blocking(move || {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(answer["bytes_b64"].as_str().unwrap_or(""))
                    .map_err(|_| String::from("It came back unreadable"))?;
                decode(&bytes, edge).map(|pixbuf| Raw::of(&scaled(&pixbuf, edge)))
            })
            .await
            .unwrap_or_else(|_| Err(String::from("It could not be decoded")));
            (decoded.map(Raw::texture), true)
        }
        Err(e @ Error::Bus(_)) => (Err(said(&e)), true),
        Err(e) => (Err(said(&e)), false),
    }
}

/// Picture `id`'s thumbnail, for `then`: from memory when it was read, else read once however
/// many ask for it meanwhile.
pub(super) fn thumbnail(ui: &Rc<Ui>, id: &Value, then: impl FnOnce(&Read) + 'static) {
    let key = key(id);
    let (at, kept) = SEEN.with(|seen| seen.borrow_mut().kept.ask(&key));
    if let Some(read) = kept {
        then(&read);
        return;
    }
    let first = SEEN.with(|seen| {
        let mut seen = seen.borrow_mut();
        let (asked_at, waiting) = seen.waiting.entry(key).or_default();
        *asked_at = at;
        waiting.push(Box::new(then));
        waiting.len() == 1
    });
    if !first {
        return;
    }
    SEEN.with(|seen| seen.borrow_mut().asked.push(id.clone()));
    start_reading(ui);
}

/// Start reading on the next idle, once the drawing that asks has asked for all it shows: the
/// newest are asked for last and read first, and the first reads are theirs too.
fn start_reading(ui: &Rc<Ui>) {
    if SEEN.with(|seen| std::mem::replace(&mut seen.borrow_mut().starting, true)) {
        return;
    }
    let ui = ui.clone();
    glib::idle_add_local_once(move || {
        SEEN.with(|seen| seen.borrow_mut().starting = false);
        read_asked(&ui);
    });
}

/// Start reading the pictures asked for, the newest first, while fewer than [`READING`] are.
fn read_asked(ui: &Rc<Ui>) {
    loop {
        let next = SEEN.with(|seen| {
            let mut seen = seen.borrow_mut();
            if seen.reading >= READING {
                return None;
            }
            let id = seen.asked.pop()?;
            seen.reading += 1;
            Some(id)
        });
        let Some(id) = next else { return };
        READS.with(|r| {
            if let Some(log) = r.borrow_mut().as_mut() {
                log.push(key(&id));
            }
        });
        let ui = ui.clone();
        glib::spawn_future_local(async move {
            let key = key(&id);
            let (read, keep) = read_picture(&ui, &id, THUMB_EDGE).await;
            let waiting = SEEN.with(|seen| {
                let mut seen = seen.borrow_mut();
                seen.reading = seen.reading.saturating_sub(1);
                let (at, waiting) = seen.waiting.remove(&key).unwrap_or_default();
                if keep {
                    seen.kept.keep(key.clone(), at, read.clone());
                }
                waiting
            });
            for then in waiting {
                then(&read);
            }
            read_asked(&ui);
        });
    }
}

/// The size a picture `width` × `height` shows at within `most`, its shape kept: never larger than
/// it is, nor so small it cannot be seen. Without a size it takes a 4:3 box.
fn shown_size(width: i64, height: i64, most: (f64, f64)) -> (i32, i32) {
    if width <= 0 || height <= 0 {
        return (most.0.round() as i32, (most.0 * 0.75).round() as i32);
    }
    let (w, h) = (width as f64, height as f64);
    let scale = (most.0 / w).min(most.1 / h).min(1.0);
    (((w * scale).round() as i32).max(24), ((h * scale).round() as i32).max(24))
}

/// A message's pictures, above its words: each at most 240 pixels wide (160 beside others), its
/// shape kept and its corners rounded as the bubble's are. A click opens one larger.
pub(super) fn in_message(ui: &Rc<Ui>, images: &[Value]) -> gtk::FlowBox {
    let flow = gtk::FlowBox::new();
    flow.add_css_class("threads-user-pictures");
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_homogeneous(false);
    flow.set_halign(gtk::Align::End);
    flow.set_column_spacing(6);
    flow.set_row_spacing(6);
    flow.set_max_children_per_line(MAX_PICTURES as u32);
    let most = if images.len() > 1 { (160.0, 200.0) } else { (240.0, 320.0) };
    for image in images {
        let picture = message_picture(ui, image, most);
        flow.append(&picture);
        // The picture's key takes the keyboard, not the box the flow puts around it.
        if let Some(child) = picture.parent() {
            child.set_focusable(false);
        }
    }
    flow
}

/// One of a message's pictures: a key the thumbnail covers, sized from the message before it is
/// read, so the conversation does not jump when it comes.
fn message_picture(ui: &Rc<Ui>, image: &Value, most: (f64, f64)) -> gtk::Button {
    let (width, height) = (image["width"].as_i64().unwrap_or(0), image["height"].as_i64().unwrap_or(0));
    let (w, h) = shown_size(width, height, most);
    let key = gtk::Button::new();
    key.set_widget_name(&picture_name(&image["id"]));
    key.add_css_class("threads-user-picture");
    key.set_overflow(gtk::Overflow::Hidden);
    let frame = gtk::Overlay::new();
    let room = gtk::Box::new(gtk::Orientation::Vertical, 0);
    room.set_size_request(w, h);
    frame.set_child(Some(&room));
    let picture = gtk::Picture::new();
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk::ContentFit::Cover);
    picture.set_alternative_text(Some("Picture"));
    frame.add_overlay(&picture);
    key.set_child(Some(&frame));
    let about = if width > 0 && height > 0 { format!("Open the picture · {width} × {height}") } else { String::from("Open the picture") };
    key.set_tooltip_text(Some(&about));
    key.update_property(&[gtk::accessible::Property::Label("Picture")]);
    let (shown, held, framed, sized) = (picture.downgrade(), key.downgrade(), frame.downgrade(), room.downgrade());
    thumbnail(ui, &image["id"], move |read| match read {
        Ok(texture) => {
            if let Some(picture) = shown.upgrade() {
                picture.set_paintable(Some(texture));
            }
        }
        Err(why) => {
            let (Some(key), Some(frame), Some(room)) = (held.upgrade(), framed.upgrade(), sized.upgrade()) else { return };
            // A quiet stand-in where the picture would be, saying why, with room for the words.
            room.set_size_request(w.max(150), h.max(84));
            let missing = gtk::Box::new(gtk::Orientation::Vertical, 6);
            missing.add_css_class("threads-picture-missing");
            missing.set_halign(gtk::Align::Center);
            missing.set_valign(gtk::Align::Center);
            missing.append(&crate::icons::image("file-image", 16));
            let words = label(&format!("Not shown: {why}"), "threads-picture-why");
            words.set_wrap(true);
            words.set_lines(3);
            words.set_ellipsize(gtk::pango::EllipsizeMode::End);
            words.set_justify(gtk::Justification::Center);
            words.set_xalign(0.5);
            missing.append(&words);
            frame.add_overlay(&missing);
            key.set_sensitive(false);
            key.set_tooltip_text(Some(why));
            key.update_property(&[gtk::accessible::Property::Label(&format!("Picture not shown: {why}"))]);
        }
    });
    let (ui, image) = (Rc::downgrade(ui), image.clone());
    key.connect_clicked(move |_| {
        if let Some(ui) = ui.upgrade() {
            open_large(&ui, &image);
        }
    });
    key
}

/// `image` larger, in a window of its own over Relay's: its thumbnail at once, then the picture
/// itself. Escape or the close key closes it.
pub(super) fn open_large(ui: &Rc<Ui>, image: &Value) {
    let id = image["id"].clone();
    let window = gtk::Window::new();
    window.set_title(Some("Picture"));
    window.set_transient_for(Some(&ui.window));
    window.set_modal(true);
    window.set_destroy_with_parent(true);
    window.add_css_class("threads-viewer");
    // As large as the picture, up to most of Relay's window.
    let room = (f64::from(ui.window.width().max(640)) * 0.86, f64::from(ui.window.height().max(480)) * 0.86);
    let (w, h) = shown_size(image["width"].as_i64().unwrap_or(0), image["height"].as_i64().unwrap_or(0), room);
    window.set_default_size(w.max(320) + 32, h.max(240) + 32);
    let overlay = gtk::Overlay::new();
    let picture = gtk::Picture::new();
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk::ContentFit::Contain);
    picture.set_alternative_text(Some("Picture"));
    picture.set_margin_top(16);
    picture.set_margin_bottom(16);
    picture.set_margin_start(16);
    picture.set_margin_end(16);
    overlay.set_child(Some(&picture));
    let note = label("Reading the picture…", "threads-viewer-note");
    note.set_halign(gtk::Align::Center);
    note.set_valign(gtk::Align::Center);
    note.set_wrap(true);
    note.set_max_width_chars(48);
    overlay.add_overlay(&note);
    let close = crate::app::icon_button("close", "Close · Esc");
    close.set_halign(gtk::Align::End);
    close.set_valign(gtk::Align::Start);
    close.set_margin_top(8);
    close.set_margin_end(8);
    let shut = window.downgrade();
    close.connect_clicked(move |_| {
        if let Some(window) = shut.upgrade() {
            window.close();
        }
    });
    overlay.add_overlay(&close);
    window.set_child(Some(&overlay));
    let keys = gtk::EventControllerKey::new();
    let shut = window.downgrade();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key != gdk::Key::Escape {
            return glib::Propagation::Proceed;
        }
        if let Some(window) = shut.upgrade() {
            window.close();
        }
        glib::Propagation::Stop
    });
    window.add_controller(keys);
    if let Some(Ok(texture)) = cached(&id) {
        picture.set_paintable(Some(&texture));
        note.set_visible(false);
    }
    let (ui, shown, said_note) = (ui.clone(), picture.downgrade(), note.downgrade());
    glib::spawn_future_local(async move {
        let (read, _) = read_picture(&ui, &id, MAX_SIDE).await;
        let (Some(picture), Some(note)) = (shown.upgrade(), said_note.upgrade()) else { return };
        match read {
            Ok(texture) => {
                picture.set_paintable(Some(&texture));
                note.set_visible(false);
            }
            // The thumbnail stays when the picture itself could not be read.
            Err(why) if picture.paintable().is_none() => note.set_text(&format!("This picture could not be read: {why}")),
            Err(_) => {}
        }
    });
    window.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: i32, height: i32, alpha: bool) -> Vec<u8> {
        let pixbuf = Pixbuf::new(gtk::gdk_pixbuf::Colorspace::Rgb, alpha, 8, width, height).unwrap();
        pixbuf.fill(0x3377aaff);
        pixbuf.save_to_bufferv("png", &[]).unwrap()
    }

    #[test]
    fn only_what_a_thread_takes_is_a_picture() {
        assert_eq!(media_type(&png(4, 4, false)), Some("image/png"));
        assert_eq!(media_type(b"\xff\xd8\xff\xe0rest"), Some("image/jpeg"));
        assert_eq!(media_type(b"GIF89a...."), Some("image/gif"));
        assert_eq!(media_type(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(media_type(b"BM...."), None);
        assert_eq!(media_type(b"<svg/>"), None);
        assert_eq!(prepare(b"BM not this one".to_vec()).err().as_deref(), Some(NOT_A_PICTURE));
        assert!(is_picture(Path::new("Shot.PNG")) && is_picture(Path::new("a.jpeg")) && is_picture(Path::new("b.webp")));
        assert!(!is_picture(Path::new("photo.heic")) && !is_picture(Path::new("notes.png.txt")));
    }

    #[test]
    fn a_damaged_picture_stays_here() {
        // Cut short: its header reads, its pixels do not. The API would refuse it, and a thread
        // resumed with it in its history would go on failing.
        let mut cut = png(300, 200, false);
        cut.truncate(cut.len() / 2);
        let refused = prepare(cut).err().unwrap_or_default();
        assert!(refused.starts_with("It could not be read"), "{refused}");
        assert!(can_draw("image/png") && can_draw("image/jpeg"));
    }

    #[test]
    fn a_picture_that_fits_goes_as_it_is_and_a_long_one_is_shrunk() {
        assert!(!redrawn(2000, 1500, MAX_BYTES, true));
        assert!(redrawn(2001, 10, 100, true));
        assert!(redrawn(800, 600, MAX_BYTES + 1, true));
        assert!(redrawn(800, 600, 100, false), "a sideways photo is turned upright");
        let small = png(300, 200, false);
        let kept = prepare(small.clone()).unwrap();
        assert_eq!(kept.bytes, small);
        assert_eq!(kept.media_type, "image/png");
        // A screenshot stays PNG, at most 2000 on its long side; its thumbnail is smaller still.
        let long = prepare(png(5000, 100, true)).unwrap();
        assert_eq!(long.media_type, "image/png");
        let pixbuf = decode(&long.bytes, 10_000).unwrap();
        assert_eq!((pixbuf.width(), pixbuf.height()), (2000, 40));
        let thumb = long.thumb.unwrap();
        assert_eq!(thumb.width, THUMB_EDGE as i32);
    }

    #[test]
    fn a_photo_shrinks_to_a_jpeg() {
        let pixbuf = Pixbuf::new(gtk::gdk_pixbuf::Colorspace::Rgb, false, 8, 3000, 2000).unwrap();
        pixbuf.fill(0x88664422);
        let photo = pixbuf.save_to_bufferv("jpeg", &[("quality", "90")]).unwrap();
        let sent = prepare(photo).unwrap();
        assert_eq!(sent.media_type, "image/jpeg");
        assert!(sent.bytes.len() <= MAX_BYTES);
        let back = decode(&sent.bytes, 10_000).unwrap();
        assert_eq!((back.width(), back.height()), (2000, 1333));
    }

    #[test]
    fn a_jpeg_says_how_it_is_held() {
        // SOI, then an APP1 Exif segment whose first directory holds Orientation 6, little-endian.
        let mut tiff = b"II*\0\x08\0\0\0\x01\0".to_vec();
        tiff.extend_from_slice(&[0x12, 0x01, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0]);
        tiff.extend_from_slice(&[0, 0, 0, 0]);
        let mut segment = b"Exif\0\0".to_vec();
        segment.extend_from_slice(&tiff);
        let mut jpeg = vec![0xff, 0xd8, 0xff, 0xe1];
        jpeg.extend_from_slice(&u16::try_from(segment.len() + 2).unwrap().to_be_bytes());
        jpeg.extend_from_slice(&segment);
        jpeg.extend_from_slice(&[0xff, 0xda, 0, 2]);
        assert_eq!(exif_orientation(&jpeg), Some(6));
        // A JFIF header and then the image data: no orientation at all.
        assert_eq!(exif_orientation(&[0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0, 0xff, 0xda]), None);
        assert_eq!(exif_orientation(&[0xff, 0xd8]), None);
    }

    #[test]
    fn a_message_picture_keeps_its_shape_within_its_room() {
        assert_eq!(shown_size(1200, 900, (240.0, 320.0)), (240, 180));
        // A tall screenshot is held by its height.
        assert_eq!(shown_size(1080, 2400, (240.0, 320.0)), (144, 320));
        // A small one is not blown up, nor a sliver lost.
        assert_eq!(shown_size(100, 50, (240.0, 320.0)), (100, 50));
        assert_eq!(shown_size(4000, 20, (240.0, 320.0)), (240, 24));
        assert_eq!(shown_size(0, 0, (160.0, 200.0)), (160, 120));
    }

    #[test]
    fn a_refusal_reads_as_what_to_do() {
        let local = Error::Bus(Box::new(relay_bus::BusError::refused("thread.images_unsupported", "images are not supported by local models")));
        assert_eq!(said(&local), LOCAL_CANNOT);
        let other = Error::Bus(Box::new(relay_bus::BusError::invalid("thread.image_too_large", "A picture may be at most 4 MiB.")));
        assert_eq!(said(&other), "A picture may be at most 4 MiB.");
        assert_eq!(key(&json!(12)), "12");
        assert_eq!(key(&json!("a1b2")), "a1b2");
    }

    #[test]
    fn a_reopened_thread_keeps_the_pictures_on_screen() {
        // A thread of 120 pictures is drawn whole: it asks for them oldest first, and the misses
        // are read newest first (those are on screen). Then it is drawn again, and again.
        let open = |kept: &mut Kept<u32>| -> Vec<u32> {
            let asked: Vec<(u32, u64, bool)> = (0..120).map(|n| {
                let (at, hit) = kept.ask(&n.to_string());
                (n, at, hit.is_some())
            }).collect();
            let mut read = Vec::new();
            for (n, at, _) in asked.iter().rev().filter(|(_, _, hit)| !hit) {
                kept.keep(n.to_string(), *at, *n);
                read.push(*n);
            }
            read
        };
        let mut kept = Kept::new(KEPT);
        assert_eq!(open(&mut kept).len(), 120);
        for _ in 0..3 {
            // Only those past the limit at the top of the thread are read again; the newest stay.
            let again = open(&mut kept);
            assert_eq!(again.len(), 120 - KEPT);
            assert!(again.iter().all(|n| *n < (120 - KEPT) as u32), "read again: {again:?}");
        }
    }

    /// Deterministic pixels: a photo's (gradients under fine noise, which PNG cannot pack) or a
    /// screenshot's (flat panels, lines of text-like dashes).
    fn pasted(width: i32, height: i32, photo: bool) -> gdk::Texture {
        let mut seed = 0x2545_f491_u32;
        let mut pixels = Vec::with_capacity((width * height * 3) as usize);
        for y in 0..height {
            for x in 0..width {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let rgb = if photo {
                    let n = (seed & 31) as i32 - 16;
                    [(x * 200 / width + 30 + n).clamp(0, 255), (y * 180 / height + 40 + n).clamp(0, 255), ((x + y) * 120 / (width + height) + 60 + n).clamp(0, 255)]
                } else if (y / 18) % 3 == 1 && (x % 240) < 180 && (x / 7 + y) % 5 != 0 && x > 300 {
                    [40, 40, 44]
                } else if x < 280 {
                    [235, 236, 240]
                } else {
                    [255, 255, 255]
                };
                pixels.extend(rgb.map(|c| c as u8));
            }
        }
        gdk::MemoryTexture::new(width, height, gdk::MemoryFormat::R8g8b8, &glib::Bytes::from_owned(pixels), (width * 3) as usize).upcast()
    }

    #[test]
    fn a_pasted_photo_goes_as_jpeg_and_a_screenshot_as_png() {
        let photo = prepare_pasted(&pasted(1600, 1200, true)).unwrap();
        assert_eq!(photo.media_type, "image/jpeg");
        let shot = prepare_pasted(&pasted(2560, 1440, false)).unwrap();
        assert_eq!(shot.media_type, "image/png");
        let back = decode(&shot.bytes, 10_000).unwrap();
        assert_eq!((back.width(), back.height()), (2000, 1125));
        // A picture with transparency keeps it.
        let mut pixels = vec![0_u8; 64 * 48 * 4];
        for (i, px) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            *px = [200, 40, 40, if i % 2 == 0 { 255 } else { 0 }];
        }
        let clear: gdk::Texture = gdk::MemoryTexture::new(64, 48, gdk::MemoryFormat::R8g8b8a8, &glib::Bytes::from_owned(pixels), 64 * 4).upcast();
        let kept = prepare_pasted(&clear).unwrap();
        assert_eq!(kept.media_type, "image/png");
        assert!(decode(&kept.bytes, 10_000).unwrap().has_alpha());
    }

    /// `cargo test -p relay-native paste_cost -- --ignored --nocapture`: what a pasted picture costs
    /// to make fit, and what goes up. RELAY_PASTE_FILES=a.jpg,b.png adds pictures from disk.
    #[test]
    #[ignore]
    fn paste_cost() {
        let mut inputs: Vec<(String, gdk::Texture)> = [("photo 4032x3024", 4032, 3024, true), ("photo 1600x1200", 1600, 1200, true), ("screenshot 2560x1440", 2560, 1440, false)]
            .into_iter()
            .map(|(name, w, h, photo)| (name.to_string(), pasted(w, h, photo)))
            .collect();
        for path in std::env::var("RELAY_PASTE_FILES").unwrap_or_default().split(',').filter(|p| !p.is_empty()) {
            let texture = gdk::Texture::from_filename(path).unwrap();
            inputs.push((format!("{} {}x{}", Path::new(path).file_name().unwrap().to_string_lossy(), texture.width(), texture.height()), texture));
        }
        for (name, texture) in &inputs {
            let mut times = Vec::new();
            let mut sent = (0, "");
            for _ in 0..5 {
                let t = std::time::Instant::now();
                let p = prepare_pasted(texture).unwrap();
                times.push(t.elapsed().as_secs_f64() * 1000.0);
                sent = (p.bytes.len() / 1024, p.media_type);
            }
            times.sort_by(f64::total_cmp);
            println!("PASTE {name}: {:.0} ms [{:.0}-{:.0}] sends {} KB {}", times[2], times[0], times[4], sent.0, sent.1);
        }
    }

    #[test]
    fn a_picture_left_unsent_a_day_is_let_go() {
        let gone = Error::Bus(Box::new(relay_bus::BusError::invalid("thread.image_unknown", "No picture 3fa2")));
        assert_eq!(said(&gone), EXPIRED);
        let now = SystemTime::now();
        let ago = |hours: u64| Some(now - Duration::from_secs(hours * 60 * 60));
        assert!(expired(ago(24), now) && expired(ago(30), now));
        assert!(!expired(ago(23), now));
        // One put back from a sent message is carried by it, and never let go.
        assert!(!expired(None, now));
    }
}
