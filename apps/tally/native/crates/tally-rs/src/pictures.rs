//! The pictures (DESIGN.md, "Pictures"): one studio render per subject, a light one for Light and a
//! dark one for Dark, made on the owner's PC with Qwen-Image.
//!
//! They are decoded off the UI thread the first time a screen asks for one, and handed to the
//! canvas's picture atlas on the next frame. The atlas holds about a dozen; when it is full it is
//! emptied and what is still on screen is asked for again.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Mutex, OnceLock};

use crate::draw::{Canvas, Rgba};

/// Called from the decoding thread when a picture is ready, so a sleeping app draws it at once.
static WAKE: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

pub fn has_waker() -> bool {
    WAKE.get().is_some()
}

/// How the platform wakes the app's loop (Android: the activity's waker).
pub fn set_waker(f: Box<dyn Fn() + Send + Sync>) {
    let _ = WAKE.set(f);
}

/// Every picture, by the name screens use; `_d` is its Dark version.
pub const FILES: &[(&str, &[u8])] = &[
    ("bank", include_bytes!("../assets/pictures/bank.webp")),
    ("bank_d", include_bytes!("../assets/pictures/bank_d.webp")),
    ("baby", include_bytes!("../assets/pictures/baby.webp")),
    ("baby_d", include_bytes!("../assets/pictures/baby_d.webp")),
    ("cards", include_bytes!("../assets/pictures/cards.webp")),
    ("cards_d", include_bytes!("../assets/pictures/cards_d.webp")),
    ("carkey", include_bytes!("../assets/pictures/carkey.webp")),
    ("carkey_d", include_bytes!("../assets/pictures/carkey_d.webp")),
    ("chart", include_bytes!("../assets/pictures/chart.webp")),
    ("chart_d", include_bytes!("../assets/pictures/chart_d.webp")),
    ("coin", include_bytes!("../assets/pictures/coin.webp")),
    ("debt", include_bytes!("../assets/pictures/debt.webp")),
    ("debt_d", include_bytes!("../assets/pictures/debt_d.webp")),
    ("fitness", include_bytes!("../assets/pictures/fitness.webp")),
    ("fitness_d", include_bytes!("../assets/pictures/fitness_d.webp")),
    ("gift", include_bytes!("../assets/pictures/gift.webp")),
    ("gift_d", include_bytes!("../assets/pictures/gift_d.webp")),
    ("hero", include_bytes!("../assets/pictures/hero.webp")),
    ("icon", include_bytes!("../assets/pictures/icon.webp")),
    ("icon_d", include_bytes!("../assets/pictures/icon_d.webp")),
    ("house", include_bytes!("../assets/pictures/house.webp")),
    ("house_d", include_bytes!("../assets/pictures/house_d.webp")),
    ("laptop", include_bytes!("../assets/pictures/laptop.webp")),
    ("laptop_d", include_bytes!("../assets/pictures/laptop_d.webp")),
    ("pet", include_bytes!("../assets/pictures/pet.webp")),
    ("pet_d", include_bytes!("../assets/pictures/pet_d.webp")),
    ("piggy", include_bytes!("../assets/pictures/piggy.webp")),
    ("piggy_d", include_bytes!("../assets/pictures/piggy_d.webp")),
    ("receipts", include_bytes!("../assets/pictures/receipts.webp")),
    ("receipts_d", include_bytes!("../assets/pictures/receipts_d.webp")),
    ("renovate", include_bytes!("../assets/pictures/renovate.webp")),
    ("renovate_d", include_bytes!("../assets/pictures/renovate_d.webp")),
    ("retire", include_bytes!("../assets/pictures/retire.webp")),
    ("retire_d", include_bytes!("../assets/pictures/retire_d.webp")),
    ("rings", include_bytes!("../assets/pictures/rings.webp")),
    ("rings_d", include_bytes!("../assets/pictures/rings_d.webp")),
    ("school", include_bytes!("../assets/pictures/school.webp")),
    ("school_d", include_bytes!("../assets/pictures/school_d.webp")),
    ("suitcase", include_bytes!("../assets/pictures/suitcase.webp")),
    ("suitcase_d", include_bytes!("../assets/pictures/suitcase_d.webp")),
    ("umbrella", include_bytes!("../assets/pictures/umbrella.webp")),
    ("umbrella_d", include_bytes!("../assets/pictures/umbrella_d.webp")),
    ("wallet", include_bytes!("../assets/pictures/wallet.webp")),
    ("wallet_d", include_bytes!("../assets/pictures/wallet_d.webp")),
];

/// The name to draw for `name` on this palette, and whether that picture is a dark one (light
/// words go on it). `coin` and `hero` are dark on both.
pub fn themed(name: &'static str, dark: bool) -> (&'static str, bool) {
    if matches!(name, "coin" | "hero") {
        return (name, true);
    }
    if dark {
        for (n, _) in FILES {
            if n.len() == name.len() + 2 && n.starts_with(name) && n.ends_with("_d") {
                return (n, true);
            }
        }
    }
    (name, false)
}

/// A picture for a goal, from what its name says. Anything unknown gets the piggy bank.
pub fn for_goal(name: &str, invest: bool) -> &'static str {
    let n = name.to_lowercase();
    let any = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if any(&["emergency", "cushion", "rainy", "safety", "buffer"]) {
        "umbrella"
    } else if any(&["trip", "travel", "vacation", "voyage", "holiday trip", "flight", "japan", "europe", "ski", "beach"]) {
        "suitcase"
    } else if any(&["house", "home", "condo", "down payment", "downpayment", "apartment", "mortgage", "move"]) {
        "house"
    } else if any(&["car", "truck", "vehicle", "bike", "motorcycle"]) {
        "carkey"
    } else if any(&["wedding", "ring", "engagement", "honeymoon"]) {
        "rings"
    } else if any(&["laptop", "computer", "phone", "camera", "console", "tv", "gadget", "tablet", "pc"]) {
        "laptop"
    } else if any(&["school", "tuition", "course", "college", "university", "education", "degree", "class"]) {
        "school"
    } else if any(&["baby", "kid", "child", "family", "daycare", "parental"]) {
        "baby"
    } else if any(&["dog", "cat", "pet", "vet", "puppy", "kitten"]) {
        "pet"
    } else if any(&["retire", "pension", "someday", "freedom", "rrsp"]) {
        "retire"
    } else if any(&["gym", "fitness", "health", "bike race", "marathon", "sport"]) {
        "fitness"
    } else if any(&["gift", "christmas", "birthday", "holiday", "present", "noel"]) {
        "gift"
    } else if any(&["renovat", "kitchen", "bathroom", "repair", "furniture", "reno", "paint"]) {
        "renovate"
    } else if any(&["debt", "loan", "credit", "pay off", "payoff", "student loan"]) {
        "debt"
    } else if invest || any(&["invest", "tfsa", "fhsa", "stock", "etf", "portfolio"]) {
        "chart"
    } else {
        "piggy"
    }
}

struct Shared {
    want: Sender<&'static str>,
    done: Receiver<(&'static str, u32, u32, Vec<u8>, [u8; 4])>,
    asked: HashSet<&'static str>,
    /// Each decoded picture's ground at its foot, where words go.
    grounds: HashMap<&'static str, [u8; 4]>,
}

/// The average colour of a picture's bottom rows.
fn foot(px: &[u8], w: u32, h: u32) -> [u8; 4] {
    let rows = 12.min(h);
    let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
    for y in h - rows..h {
        for x in (0..w).step_by(4) {
            let i = ((y * w + x) * 4) as usize;
            r += px[i] as u64;
            g += px[i + 1] as u64;
            b += px[i + 2] as u64;
            n += 1;
        }
    }
    let n = n.max(1);
    [(r / n) as u8, (g / n) as u8, (b / n) as u8, 255]
}

fn shared() -> &'static Mutex<Shared> {
    static S: OnceLock<Mutex<Shared>> = OnceLock::new();
    S.get_or_init(|| {
        let (want, rx) = channel::<&'static str>();
        let (tx, done) = channel();
        std::thread::Builder::new()
            .name("pictures".into())
            .spawn(move || {
                for key in rx {
                    let Some((_, bytes)) = FILES.iter().find(|(n, _)| *n == key) else { continue };
                    let Ok(img) = image::load_from_memory_with_format(bytes, image::ImageFormat::WebP) else { continue };
                    let img = img.to_rgba8();
                    let (w, h) = img.dimensions();
                    let px = img.into_raw();
                    let ground = foot(&px, w, h);
                    if tx.send((key, w, h, px, ground)).is_err() {
                        break;
                    }
                    if let Some(wake) = WAKE.get() {
                        wake();
                    }
                }
            })
            .expect("the pictures thread starts");
        Mutex::new(Shared { want, done, asked: HashSet::new(), grounds: HashMap::new() })
    })
}

/// Make sure `key` is in the canvas's atlas, or on its way. True when it can be drawn now.
pub fn ensure(c: &mut Canvas, key: &'static str) -> bool {
    if c.has_image(key) {
        return true;
    }
    let mut s = shared().lock().expect("pictures lock");
    if s.asked.insert(key) {
        let _ = s.want.send(key);
    }
    false
}

/// Hand what was decoded to the canvas. True when a picture arrived (draw again) or some are
/// still being decoded.
pub fn drain(c: &mut Canvas) -> bool {
    let mut s = shared().lock().expect("pictures lock");
    let mut arrived = false;
    while let Ok((key, w, h, px, ground)) = s.done.try_recv() {
        s.grounds.insert(key, ground);
        if !c.add_image(key, w, h, px.clone()) {
            // Full: start the atlas over with this one; the rest are asked for again as drawn.
            c.clear_images();
            s.asked.clear();
            c.add_image(key, w, h, px);
        }
        s.asked.remove(key);
        arrived = true;
    }
    arrived || !s.asked.is_empty()
}

/// Pictures asked for and not yet in the atlas.
pub fn pending() -> bool {
    !shared().lock().expect("pictures lock").asked.is_empty()
}

/// The colour at the foot of a picture already decoded (for the words over it and the box it
/// fills before it arrives).
pub fn ground(key: &str) -> Option<Rgba> {
    shared().lock().expect("pictures lock").grounds.get(key).map(|c| Rgba(*c))
}
