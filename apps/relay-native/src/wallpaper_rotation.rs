//! One window-owned timer. A rotation paints the next image in this window only: it is not
//! written to `appearance.wallpaper`, because every audited settings.set keeps the previous
//! image in the audit table for months. The saved wallpaper stays the user's choice.
use crate::app::Ui;
use crate::client::Error;
use serde_json::{json, Value};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// Offer bundled gradients only before the library has been configured.
pub(crate) fn library_or_defaults(library: &Value) -> Value {
    use base64::Engine;
    if !library.is_null() {
        return library.clone();
    }
    let presets: [(&str, &str, &[u8]); 3] = [
        (
            "charcoal",
            "Charcoal",
            include_bytes!("../resources/wallpapers/charcoal.png"),
        ),
        (
            "teal",
            "Deep teal",
            include_bytes!("../resources/wallpapers/teal.png"),
        ),
        (
            "indigo",
            "Midnight indigo",
            include_bytes!("../resources/wallpapers/indigo.png"),
        ),
    ];
    presets
        .into_iter()
        .map(|(id, name, bytes)| {
            let image = format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            );
            json!({"id":format!("builtin-{id}"),"name":name,"image":image})
        })
        .collect()
}

#[derive(Default)]
pub(crate) struct Rotation {
    timer: Option<glib::SourceId>,
    signature: u64,
    revision: u64,
    running: bool,
    /// The image a rotation put up, with the saved wallpaper it stood in for: a different
    /// saved wallpaper (the user chose one) retires it.
    rotated: Option<(Value, String)>,
}

/// The image this window shows for the saved wallpaper `saved`: the rotated one while the
/// saved choice is unchanged, else the saved one.
pub(crate) fn shown(ui: &Ui, saved: &Value) -> Value {
    let mut state = ui.wallpaper_rotation.borrow_mut();
    match &state.rotated {
        Some((base, image)) if base == saved => json!(image),
        Some(_) => {
            state.rotated = None;
            saved.clone()
        }
        None => saved.clone(),
    }
}

impl Drop for Rotation {
    fn drop(&mut self) {
        if let Some(timer) = self.timer.take() {
            timer.remove();
        }
    }
}

/// What the timer depends on: the config and which images there are. A library can be
/// megabytes of base64; each tick reads it afresh, so its bytes need not be hashed here.
fn signature(config: &Value, library: &Value) -> u64 {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    config.to_string().hash(&mut hash);
    images(library).len().hash(&mut hash);
    for item in library.as_array().into_iter().flatten() {
        item["id"].as_str().hash(&mut hash);
    }
    hash.finish()
}

/// The distinct images a rotation shuffles through; Settings counts them too.
pub(crate) fn images(library: &Value) -> Vec<&str> {
    let mut images = Vec::new();
    if let Some(library) = library.as_array() {
        for item in library {
            if let Some(image) = item["image"]
                .as_str()
                .filter(|image| image.starts_with("data:image/"))
            {
                if !images.contains(&image) {
                    images.push(image);
                }
            }
        }
    }
    images
}

pub(crate) fn refresh(ui: &Rc<Ui>) {
    let revision = {
        let mut state = ui.wallpaper_rotation.borrow_mut();
        state.revision += 1;
        state.revision
    };
    let generation = ui.generation.get();
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let stale = |ui: &Ui| {
            generation != ui.generation.get() || revision != ui.wallpaper_rotation.borrow().revision
        };
        let Ok(config) = ui
            .call(
                "settings.get",
                json!({"path":"appearance.wallpaper_rotation"}),
            )
            .await
        else {
            return;
        };
        if stale(&ui) {
            return;
        }
        // Rotation off: the library is not needed, and it is the expensive read.
        if config["value"]["enabled"] != true {
            configure(&ui, &config["value"], &Value::Null);
            return;
        }
        let library = ui
            .call("settings.get", json!({"path":"appearance.wallpapers"}))
            .await;
        if stale(&ui) {
            return;
        }
        if let Ok(library) = library {
            configure(&ui, &config["value"], &library["value"]);
        }
    });
}

fn configure(ui: &Rc<Ui>, config: &Value, library: &Value) {
    let signature = signature(config, library);
    let mut state = ui.wallpaper_rotation.borrow_mut();
    if state.signature == signature {
        return;
    }
    state.signature = signature;
    if let Some(timer) = state.timer.take() {
        timer.remove();
    }
    if config["enabled"] != true || images(library).len() < 2 {
        // Rotation off, or the library changed under it: show the saved wallpaper again.
        if state.rotated.take().is_some() {
            drop(state);
            ui.load_appearance();
        }
        return;
    }
    let minutes = config["interval_minutes"]
        .as_u64()
        .unwrap_or(15)
        .clamp(1, 1440);
    let weak = Rc::downgrade(ui);
    state.timer = Some(glib::timeout_add_local(
        std::time::Duration::from_secs(minutes * 60),
        move || {
            let Some(ui) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            // No engine connection: skip the tick. The banner already says the engine is gone,
            // and a rotation error every interval would only cover that up.
            if ui.wallpaper_rotation.borrow().running || ui.client.borrow().is_none() {
                return glib::ControlFlow::Continue;
            }
            ui.wallpaper_rotation.borrow_mut().running = true;
            glib::spawn_future_local(async move {
                match rotate_once(&ui).await {
                    // The connection dropped mid-tick: the disconnect is reported already.
                    Ok(_) | Err(Error::Disconnected) => {}
                    Err(error) => ui.show_error(&format!("Wallpaper rotation failed: {error}")),
                }
                ui.wallpaper_rotation.borrow_mut().running = false;
            });
            glib::ControlFlow::Continue
        },
    ));
}

pub(crate) async fn rotate_once(ui: &Rc<Ui>) -> Result<bool, Error> {
    let generation = ui.generation.get();
    let revision = ui.wallpaper_rotation.borrow().revision;
    let (config, library, current) = tokio::join!(
        ui.call(
            "settings.get",
            json!({"path":"appearance.wallpaper_rotation"})
        ),
        ui.call("settings.get", json!({"path":"appearance.wallpapers"})),
        ui.call("settings.get", json!({"path":"appearance.wallpaper"}))
    );
    let (config, library, current) = (config?, library?, current?);
    if generation != ui.generation.get() || revision != ui.wallpaper_rotation.borrow().revision {
        return Ok(false);
    }
    if config["value"]["enabled"] != true {
        return Ok(false);
    }
    let images = images(&library["value"]);
    if images.len() < 2 {
        return Ok(false);
    }
    let showing = shown(ui, &current["value"]);
    let candidates: Vec<_> = images
        .into_iter()
        .filter(|image| Some(*image) != showing.as_str())
        .collect();
    let index = uuid::Uuid::new_v4().as_u128() % candidates.len() as u128;
    ui.wallpaper_rotation.borrow_mut().rotated =
        Some((current["value"].clone(), candidates[index as usize].to_string()));
    ui.load_appearance();
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_library_only_fills_an_unset_library() {
        let defaults = library_or_defaults(&Value::Null);
        assert_eq!(defaults.as_array().unwrap().len(), 3);
        assert_eq!(images(&defaults).len(), 3);
        assert_eq!(library_or_defaults(&json!([])), json!([]));
        let custom = json!([{"id":"mine","image":"data:image/png;base64,custom"}]);
        assert_eq!(library_or_defaults(&custom), custom);
    }
    #[test]
    fn rotation_counts_distinct_images_and_ignores_metadata() {
        let library = json!([{ "image":"data:image/png;base64,one" }, {"image":"data:image/png;base64,one"}, {"image":"data:image/png;base64,two"}, {"image":null}]);
        assert_eq!(images(&library).len(), 2);
        let config = json!({"enabled":true,"interval_minutes":15});
        assert_ne!(
            signature(&config, &library),
            signature(&json!({"enabled":false}), &library)
        );
    }
    #[test]
    fn signature_follows_the_images_without_hashing_them() {
        let config = json!({"enabled":true,"interval_minutes":15});
        let two = json!([{"id":"a","image":"data:image/png;base64,one"},{"id":"b","image":"data:image/png;base64,two"}]);
        let three = json!([{"id":"a","image":"data:image/png;base64,one"},{"id":"b","image":"data:image/png;base64,two"},{"id":"c","image":"data:image/png;base64,three"}]);
        let renamed = json!([{"id":"a","image":"data:image/png;base64,one"},{"id":"d","image":"data:image/png;base64,two"}]);
        assert_eq!(signature(&config, &two), signature(&config, &two.clone()));
        assert_ne!(signature(&config, &two), signature(&config, &three));
        assert_ne!(signature(&config, &two), signature(&config, &renamed));
    }
}
