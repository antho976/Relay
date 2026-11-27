//! The emulator smoke run's screens (`tools/smoke.sh`): a debug build started with the intent
//! extra `screen` opens on sample data at that screen, as `--shot` draws it on the desktop, so the
//! run needs no PC and no pairing. Release builds never call it.

use crate::app::{App, Route};

/// The screens the smoke run looks at.
pub const SCREENS: [&str; 4] = ["home", "chat", "changes", "settings"];

/// Fill the app with sample data and open `screen` (one of [`SCREENS`]; anything else is Home).
pub fn show(app: &mut App, screen: &str) {
    app.sample();
    match screen {
        "chat" => app.open("bold-puffin"),
        "changes" => {
            app.reset_to(Route::Home);
            app.push(Route::Changes);
        }
        "settings" => {
            app.reset_to(Route::Home);
            app.push(Route::Settings);
        }
        _ => app.reset_to(Route::Home),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoHost(std::path::PathBuf);

    impl crate::app::Host for NoHost {
        fn data_dir(&self) -> std::path::PathBuf {
            self.0.clone()
        }
        fn log(&self, _text: &str) {}
    }

    #[test]
    fn each_smoke_screen_opens_where_it_says() {
        let dir = std::env::temp_dir().join(format!("relay-code-smoke-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::new(std::sync::Arc::new(NoHost(dir)), 1_000_000_000);
        a.resize(390.0, 844.0, 0.0, 0.0);
        let want = [Route::Home, Route::Chat("bold-puffin".into()), Route::Changes, Route::Settings];
        for (screen, route) in SCREENS.iter().zip(want) {
            show(&mut a, screen);
            a.frame(2_000_000_000);
            assert_eq!(*a.route(), route, "{screen}");
        }
    }
}
