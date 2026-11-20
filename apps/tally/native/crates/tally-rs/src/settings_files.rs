//! Files in and out of the setup pages, and work that must not run on the UI thread.
//!
//! - A picked file comes back from the platform on its own thread (Android's activity result, the
//!   desktop's `TALLY_PICK`). It is left in [`deliver`]'s inbox; the Android loop moves it into
//!   [`App::file_picked`] and wakes, and [`take_picked`] (what a page calls each frame) drains the
//!   inbox too, so a page reads a picked file the same way on either platform.
//! - [`Job`] runs a closure on a thread (reading a big CSV, encoding a backup) and is polled each
//!   frame until it answers.

use std::sync::Mutex;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use crate::app::App;

/// The largest file a page reads: a bank statement or a backup is far smaller.
pub const MAX_PICK_BYTES: usize = 32 * 1024 * 1024;

static INBOX: Mutex<Option<(String, Vec<u8>)>> = Mutex::new(None);

/// A picked file arrived (any thread). An empty `bytes` means it could not be read.
pub fn deliver(name: String, bytes: Vec<u8>) {
    if let Ok(mut inbox) = INBOX.lock() {
        *inbox = Some((name, bytes));
    }
}

/// What [`deliver`] left, once.
pub fn take_delivered() -> Option<(String, Vec<u8>)> {
    INBOX.lock().ok().and_then(|mut inbox| inbox.take())
}

/// The file the person picked, once: whatever reached the app or is still in the inbox.
pub fn take_picked(app: &mut App) -> Option<(String, Vec<u8>)> {
    if let Some((name, bytes)) = take_delivered() {
        app.file_picked(name, bytes);
    }
    app.picked.take()
}

/// Work on a thread of its own, polled from the frame.
pub struct Job<T> {
    rx: Receiver<T>,
}

/// Where a job stands.
pub enum Poll<T> {
    Running,
    Done(T),
    /// The thread died without answering (a panic): say so rather than wait for ever.
    Lost,
}

impl<T: Send + 'static> Job<T> {
    pub fn spawn(f: impl FnOnce() -> T + Send + 'static) -> Job<T> {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        Job { rx }
    }

    pub fn poll(&self) -> Poll<T> {
        match self.rx.try_recv() {
            Ok(v) => Poll::Done(v),
            Err(TryRecvError::Empty) => Poll::Running,
            Err(TryRecvError::Disconnected) => Poll::Lost,
        }
    }

    /// Waits for the answer: for tests only, never from a frame.
    pub fn wait(self) -> Option<T> {
        self.rx.recv().ok()
    }
}

/// Today as the backup and export file names carry it: "2026-11-19".
pub fn stamp_today() -> String {
    crate::model::today().to_string()
}
