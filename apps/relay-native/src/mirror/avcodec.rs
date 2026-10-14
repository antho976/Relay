//! libavcodec's H.264 decoder, in process: whole access units in, YUV planes out.
//!
//! The library is opened at run time, not linked: `ffmpeg-sys-next` gives the types, constants
//! and the major version the headers describe, and `libavcodec.so.<major>` is `dlopen`ed the first
//! time a mirror starts. A machine without it still runs every other part of Relay, and the
//! mirror says what is missing ([`MISSING`]) instead of the process failing to start. The soname
//! carries the ABI: a libavcodec of another major version has other struct layouts, so it is not
//! looked for.
//!
//! The settings are the measured ones (`scripts/mirror-latency/`): `AV_CODEC_FLAG_LOW_DELAY`, one
//! thread and never frame threading (it holds a picture per thread), and no parser — each packet
//! is one whole access unit, so its picture comes out of the same `avcodec_send_packet` call.
use ffmpeg_sys_next as ff;
use gtk4::gdk;
use relay_client::mirror::decode::{color, Color, RANGE_FULL};
use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
use std::sync::OnceLock;

/// How a failure to load the decoder begins, so the stage can title it.
pub const MISSING: &str = "The H.264 decoder (libavcodec) could not be loaded";

/// Declares [`Lib`], the functions this file calls, and how each is resolved from
/// `libavcodec.so.<major>` and the libavutil it pulled in. Each type is the bindgen declaration's
/// in ffmpeg-sys-next, for the same major version.
macro_rules! functions {
    ($($field:ident = $symbol:literal: $type:ty;)*) => {
        struct Lib {
            $($field: $type,)*
        }

        fn resolve(symbol: &dyn Fn(&str) -> Result<*mut c_void, String>) -> Result<Lib, String> {
            // SAFETY: each symbol is the C function its field's type spells out.
            Ok(Lib { $($field: unsafe { std::mem::transmute::<*mut c_void, $type>(symbol($symbol)?) },)* })
        }
    };
}

functions! {
    codec_version = "avcodec_version": unsafe extern "C" fn() -> c_uint;
    util_version = "avutil_version": unsafe extern "C" fn() -> c_uint;
    log_level = "av_log_set_level": unsafe extern "C" fn(c_int);
    find_decoder = "avcodec_find_decoder": unsafe extern "C" fn(ff::AVCodecID) -> *const ff::AVCodec;
    alloc_context = "avcodec_alloc_context3": unsafe extern "C" fn(*const ff::AVCodec) -> *mut ff::AVCodecContext;
    open = "avcodec_open2": unsafe extern "C" fn(*mut ff::AVCodecContext, *const ff::AVCodec, *mut *mut ff::AVDictionary) -> c_int;
    free_context = "avcodec_free_context": unsafe extern "C" fn(*mut *mut ff::AVCodecContext);
    packet_alloc = "av_packet_alloc": unsafe extern "C" fn() -> *mut ff::AVPacket;
    packet_free = "av_packet_free": unsafe extern "C" fn(*mut *mut ff::AVPacket);
    new_packet = "av_new_packet": unsafe extern "C" fn(*mut ff::AVPacket, c_int) -> c_int;
    packet_unref = "av_packet_unref": unsafe extern "C" fn(*mut ff::AVPacket);
    send_packet = "avcodec_send_packet": unsafe extern "C" fn(*mut ff::AVCodecContext, *const ff::AVPacket) -> c_int;
    receive_frame = "avcodec_receive_frame": unsafe extern "C" fn(*mut ff::AVCodecContext, *mut ff::AVFrame) -> c_int;
    frame_alloc = "av_frame_alloc": unsafe extern "C" fn() -> *mut ff::AVFrame;
    frame_free = "av_frame_free": unsafe extern "C" fn(*mut *mut ff::AVFrame);
    frame_unref = "av_frame_unref": unsafe extern "C" fn(*mut ff::AVFrame);
}

fn soname() -> String {
    format!("libavcodec.so.{}", ff::LIBAVCODEC_VERSION_MAJOR)
}

fn dlerror() -> String {
    let error = unsafe { libc::dlerror() };
    if error.is_null() {
        "unknown error".into()
    } else {
        unsafe { CStr::from_ptr(error) }.to_string_lossy().into_owned()
    }
}

fn open() -> Result<Lib, String> {
    let name = soname();
    let missing = |why: String| {
        format!("{MISSING}: {why}. Install your distribution's FFmpeg libraries (the package that provides {name}), then retry.")
    };
    let path = CString::new(name.clone()).map_err(|e| e.to_string())?;
    let handle = unsafe { libc::dlopen(path.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if handle.is_null() {
        return Err(missing(dlerror()));
    }
    // A handle's lookups cover the libraries it pulled in, which is where libavutil's are.
    let lib = resolve(&|name: &str| {
        let c = CString::new(name).map_err(|e| e.to_string())?;
        let found = unsafe { libc::dlsym(handle, c.as_ptr() as *const c_char) };
        if found.is_null() { Err(missing(format!("{name} is not in {}", soname()))) } else { Ok(found) }
    })?;
    let (codec_major, util_major) = unsafe { ((lib.codec_version)() >> 16, (lib.util_version)() >> 16) };
    if codec_major != ff::LIBAVCODEC_VERSION_MAJOR as c_uint || util_major != ff::LIBAVUTIL_VERSION_MAJOR as c_uint {
        return Err(missing(format!(
            "this build expects libavcodec {} with libavutil {}, and found {codec_major} with {util_major}",
            ff::LIBAVCODEC_VERSION_MAJOR,
            ff::LIBAVUTIL_VERSION_MAJOR
        )));
    }
    // The decoder's complaints about a damaged packet would otherwise land on stderr; the trace
    // has them instead (`RELAY_MIRROR_TRACE`, `damaged`).
    unsafe { (lib.log_level)(ff::AV_LOG_QUIET) };
    Ok(lib)
}

/// Open libavcodec once per process; every later call returns the same library or the same error.
fn lib() -> Result<&'static Lib, String> {
    static LIB: OnceLock<Result<Lib, String>> = OnceLock::new();
    LIB.get_or_init(open).as_ref().map_err(Clone::clone)
}

/// Whether the decoder can be loaded, with the reason when it cannot.
#[cfg(test)]
pub fn available() -> Result<(), String> {
    lib().map(|_| ())
}

/// One decoded picture: its planes packed back to back, as GTK takes them.
pub struct Picture {
    /// The mirror and the engine packet this picture came from (the trace's join key).
    pub mirror: i64,
    pub seq: u64,
    /// The decoded size: the size touches are sent in.
    pub width: i32,
    pub height: i32,
    pub format: gdk::MemoryFormat,
    /// Each plane's (offset, stride) into `bytes`.
    pub planes: Vec<(usize, usize)>,
    pub bytes: Vec<u8>,
    pub color: Color,
}

impl Picture {
    /// The size GTK can take: a subsampled format needs whole chroma blocks, so an odd edge (which
    /// the scrcpy server never sends — it rounds to 8) loses its last pixel row or column.
    pub fn texture_size(&self) -> (i32, i32) {
        match self.format {
            gdk::MemoryFormat::G8B8R8420 | gdk::MemoryFormat::G8B8r8420 => (self.width & !1, self.height & !1),
            gdk::MemoryFormat::G8B8R8422 => (self.width & !1, self.height),
            _ => (self.width, self.height),
        }
    }
}

impl Picture {
    /// The latency probe's barcode and touch field (`trace::probe`), for the trace only.
    pub fn probe(&self) -> Option<(u32, bool)> {
        if self.format != gdk::MemoryFormat::G8B8R8420 {
            return None;
        }
        let ((_, luma_stride), (cr, cr_stride)) = (self.planes[0], self.planes[2]);
        super::trace::probe(self.width as usize, self.height as usize, &self.bytes[..cr], luma_stride, &self.bytes[cr..], cr_stride)
    }
}

/// A decode that failed: libavcodec's error code. The stream stays usable from the next key frame.
#[derive(Debug)]
pub struct Damaged(pub c_int);

pub struct Decoder {
    lib: &'static Lib,
    context: *mut ff::AVCodecContext,
    packet: *mut ff::AVPacket,
    frame: *mut ff::AVFrame,
}

// SAFETY: a libavcodec context may move between threads; it must only never be used from two at
// once, which `&mut self` on every call already guarantees.
unsafe impl Send for Decoder {}

impl Decoder {
    pub fn new() -> Result<Self, String> {
        let lib = lib()?;
        unsafe {
            let codec = (lib.find_decoder)(ff::AVCodecID::AV_CODEC_ID_H264);
            if codec.is_null() {
                return Err(format!("{MISSING}: this libavcodec was built without its H.264 decoder."));
            }
            let decoder = Self { lib, context: (lib.alloc_context)(codec), packet: (lib.packet_alloc)(), frame: (lib.frame_alloc)() };
            if decoder.context.is_null() || decoder.packet.is_null() || decoder.frame.is_null() {
                return Err("The video decoder could not allocate memory.".into());
            }
            let context = &mut *decoder.context;
            context.flags |= ff::AV_CODEC_FLAG_LOW_DELAY as c_int;
            context.thread_count = 1;
            context.thread_type = ff::FF_THREAD_SLICE;
            let opened = (lib.open)(decoder.context, codec, std::ptr::null_mut());
            if opened < 0 {
                return Err(format!("The H.264 decoder would not open (libavcodec error {opened})."));
            }
            Ok(decoder)
        }
    }

    /// Decode one whole access unit. `Ok(None)` when it completes no picture (it should, with
    /// these settings, unless it is damaged or the stream has no key frame yet).
    pub fn decode(&mut self, unit: &[u8], mirror: i64, seq: u64) -> Result<Option<Picture>, Damaged> {
        let lib = self.lib;
        unsafe {
            let size = c_int::try_from(unit.len()).map_err(|_| Damaged(-libc::EINVAL))?;
            // A fresh, padded packet: the decoder reads past the end of a slice by design.
            let made = (lib.new_packet)(self.packet, size);
            if made < 0 {
                return Err(Damaged(made));
            }
            std::ptr::copy_nonoverlapping(unit.as_ptr(), (*self.packet).data, unit.len());
            let sent = (lib.send_packet)(self.context, self.packet);
            (lib.packet_unref)(self.packet);
            if sent < 0 && sent != -libc::EAGAIN {
                return Err(Damaged(sent));
            }
            let mut newest = None;
            while (lib.receive_frame)(self.context, self.frame) >= 0 {
                newest = picture(&*self.frame, mirror, seq);
                (lib.frame_unref)(self.frame);
            }
            Ok(newest)
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe {
            (self.lib.frame_free)(&mut self.frame);
            (self.lib.packet_free)(&mut self.packet);
            (self.lib.free_context)(&mut self.context);
        }
    }
}

/// Copy a decoded frame's planes into one buffer. `None` for a layout GTK has no format for
/// (Android's encoders only send 8-bit 4:2:0).
unsafe fn picture(frame: &ff::AVFrame, mirror: i64, seq: u64) -> Option<Picture> {
    use ff::AVPixelFormat as P;
    let (width, height) = (frame.width, frame.height);
    if width <= 0 || height <= 0 {
        return None;
    }
    let (w, h) = (width as usize, height as usize);
    let (half_w, half_h) = (w.div_ceil(2), h.div_ceil(2));
    // Per plane: (bytes in a row, rows).
    let (format, shape, range_full): (_, Vec<(usize, usize)>, bool) = match frame.format {
        f if f == P::AV_PIX_FMT_YUV420P as c_int => (gdk::MemoryFormat::G8B8R8420, vec![(w, h), (half_w, half_h), (half_w, half_h)], false),
        f if f == P::AV_PIX_FMT_YUVJ420P as c_int => (gdk::MemoryFormat::G8B8R8420, vec![(w, h), (half_w, half_h), (half_w, half_h)], true),
        f if f == P::AV_PIX_FMT_YUV422P as c_int => (gdk::MemoryFormat::G8B8R8422, vec![(w, h), (half_w, h), (half_w, h)], false),
        f if f == P::AV_PIX_FMT_YUVJ422P as c_int => (gdk::MemoryFormat::G8B8R8422, vec![(w, h), (half_w, h), (half_w, h)], true),
        f if f == P::AV_PIX_FMT_YUV444P as c_int => (gdk::MemoryFormat::G8B8R8444, vec![(w, h), (w, h), (w, h)], false),
        f if f == P::AV_PIX_FMT_YUVJ444P as c_int => (gdk::MemoryFormat::G8B8R8444, vec![(w, h), (w, h), (w, h)], true),
        f if f == P::AV_PIX_FMT_NV12 as c_int => (gdk::MemoryFormat::G8B8r8420, vec![(w, h), (half_w * 2, half_h)], false),
        _ => return None,
    };
    let mut planes = Vec::with_capacity(shape.len());
    let mut sizes = Vec::with_capacity(shape.len());
    let mut total = 0;
    for (plane, &(row, rows)) in shape.iter().enumerate() {
        let stride = frame.linesize[plane];
        if stride < row as c_int || frame.data[plane].is_null() {
            return None;
        }
        let stride = stride as usize;
        // Up to the end of the last row's pixels: the padding after it is not ours to read.
        let size = stride * (rows - 1) + row;
        planes.push((total, stride));
        sizes.push(size);
        total += size;
    }
    let mut bytes = Vec::with_capacity(total);
    for (plane, size) in sizes.into_iter().enumerate() {
        bytes.extend_from_slice(std::slice::from_raw_parts(frame.data[plane], size));
    }
    // Read as the C ints they are: a newer libavcodec may report a value the Rust enum lacks.
    let matrix = std::ptr::addr_of!(frame.colorspace).cast::<u32>().read();
    let range = std::ptr::addr_of!(frame.color_range).cast::<u32>().read();
    let color = color(matrix, if range_full { RANGE_FULL } else { range });
    Some(Picture { mirror, seq, width, height, format, planes, bytes, color })
}
