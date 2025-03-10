pub const APPS: &[&str] = &["render-host", "render-node"];

pub const CRATES: &[&str] = &[
    "appearance",
    "appearance-asset-database",
    "appearance-build",
    "appearance-camera",
    "appearance-input",
    "appearance-model",
    "appearance-packing",
    "appearance-path-tracer",
    "appearance-path-tracer-gpu",
    "appearance-profiling",
    "appearance-render-loop",
    "appearance-texture",
    "appearance-time",
    "appearance-transform",
    "appearance-wgpu",
    "appearance-world",
];

pub fn resolve_asset_path(path: &str, asset_dir: &str) -> String {
    let mut app_or_crate_name = None;
    if let Some(app_delimiter) = path.split_once("::") {
        app_or_crate_name = Some(app_delimiter.0);
    }

    let mut is_global = false;
    let prefix = if let Some(app_or_crate_name) = app_or_crate_name {
        if APPS.contains(&app_or_crate_name) {
            "assets/apps/"
        } else if CRATES.contains(&app_or_crate_name) {
            "assets/crates/"
        } else {
            is_global = true;
            "assets/"
        }
    } else {
        ""
    }
    .to_owned();

    if is_global {
        prefix + &path.replace("::", &format!("assets/{}", asset_dir))
    } else {
        prefix + &path.replace("::", &format!("/assets/{}", asset_dir))
    }
}
