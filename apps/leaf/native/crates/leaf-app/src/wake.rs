//! How a worker thread wakes the app's loop when its answer lands (a page decoded, a list
//! fetched), so a sleeping app draws it at once instead of at the next touch.

use std::sync::Mutex;

type Waker = Box<dyn Fn() + Send + Sync>;

/// Replaceable: Android starts the app's loop again for each new activity, with a new looper.
static WAKE: Mutex<Option<Waker>> = Mutex::new(None);

pub fn set(f: Waker) {
    *WAKE.lock().unwrap_or_else(|e| e.into_inner()) = Some(f);
}

pub fn has() -> bool {
    WAKE.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}

pub fn wake() {
    if let Some(w) = WAKE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        w();
    }
}
