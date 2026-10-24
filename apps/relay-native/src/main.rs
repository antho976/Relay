mod app;
mod assistant;
mod clamp;
mod client;
mod design;
mod editor;
mod fit_label;
mod fold;
mod fonts;
mod icons;
mod mirror;
mod money;
mod pages;
mod panel;
mod provider_updates;
mod relative;
mod roadmap_smoke;
mod shortcuts;
mod smoke;
mod smoke_project_files;
mod smoke_registry;
mod sounds;
mod start;
mod terminal;
mod tools;
mod wallpaper_rotation;

fn main() -> glib::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("I/O runtime");
    app::run(runtime.handle().clone())
}
