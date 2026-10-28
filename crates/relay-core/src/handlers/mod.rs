//! Op handlers, one module per namespace. `register_all` wires everything the engine
//! implements; ops in the registry without a handler answer `bus.not_implemented`.

pub mod app;
pub mod audit;
pub mod bus;
pub mod design;
pub mod device;
pub mod device_lease;
pub mod file;
pub mod git;
pub mod guardrail;
pub mod import_v3;
pub mod integration;
pub mod module;
pub mod money;
pub mod arbiter;
pub mod gym;
pub mod aide;
pub mod thread;
pub mod notes;
pub mod notify;
pub mod overlap;
pub mod picture;
pub mod provider;
pub mod session;
pub mod settings;
pub mod task;
pub mod ui;
pub mod video;
pub mod workspace;

use crate::engine::Engine;

pub fn register_all(e: &mut Engine) {
    bus::register(e);
    device::register(e);
    device_lease::register(e);
    app::register(e);
    audit::register(e);
    settings::register(e);
    workspace::register(e);
    task::register(e);
    module::register(e);
    notes::register(e);
    design::register(e);
    picture::register(e);
    video::register(e);
    overlap::register(e);
    provider::register(e);
    session::register(e);
    file::register(e);
    git::register(e);
    integration::register(e);
    notify::register(e);
    ui::register(e);
    money::register(e);
    arbiter::register(e);
    gym::register(e);
    aide::register(e);
    thread::register(e);
    guardrail::register(e);
}

/// Shared row helpers.
pub(crate) mod util {
    pub fn json_vec(s: &str) -> Vec<String> {
        serde_json::from_str(s).unwrap_or_default()
    }
}
