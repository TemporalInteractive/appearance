pub struct Appearance {}

static APPEARANCE_STATIC: std::sync::OnceLock<AppearanceStatic> = std::sync::OnceLock::new();

struct AppearanceStatic {
    // TODO: Sentry telemetry
}

impl AppearanceStatic {
    fn init(_app_name: &str) -> &'static Self {
        appearance_profiling::profile_function!();

        APPEARANCE_STATIC.get_or_init(|| {
            env_logger::builder()
                .filter_level(log::LevelFilter::Info)
                .filter_module("wgpu_core", log::LevelFilter::Warn)
                .filter_module("wgpu_hal", log::LevelFilter::Warn)
                .filter_module("naga", log::LevelFilter::Warn)
                .parse_default_env()
                .init();

            Self {}
        })
    }
}

impl Appearance {
    pub fn new(app_name: &str) -> Self {
        AppearanceStatic::init(app_name);

        Self {}
    }
}
