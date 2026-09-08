use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
struct Options {
    /// Explicit Identity catalog directory. No store is created on launch.
    #[arg(long)]
    store: Option<PathBuf>,
    /// Run the explicitly mocked one-page host lifecycle experiment.
    #[arg(long)]
    extension_demo: bool,
}

fn main() {
    let options = Options::parse();
    let config = styrene_identity_desktop::AppConfig {
        store: options.store,
        extension_demo: options.extension_demo,
    };
    dioxus::LaunchBuilder::new()
        .with_cfg(
            dioxus::desktop::Config::new()
                .with_window(dioxus::desktop::WindowBuilder::new().with_title("Styrene Identity")),
        )
        .with_context(config)
        .launch(styrene_identity_desktop::App);
}
