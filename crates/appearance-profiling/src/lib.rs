pub use puffin;
use puffin::ScopeId;

pub fn new_frame() {
    profile_function!();
    puffin::set_scopes_on(true);
    puffin::GlobalProfiler::lock().new_frame();
}

#[allow(dead_code)]
pub struct Marker(Option<puffin::ProfilerScope>);

impl Marker {
    /// Creates a new [`Marker`]. Note: the passed [`ScopeId`] should only be constructed once.
    #[inline(always)]
    pub fn new(_id: &'static str, puffin_scope_id: ScopeId, context: &str) -> Self {
        #[cfg(feature = "superluminal")]
        superluminal_perf::begin_event(_id);

        let puffin_scope = if puffin::are_scopes_on() {
            Some(puffin::ProfilerScope::new(puffin_scope_id, context))
        } else {
            None
        };

        Self(puffin_scope)
    }
}

impl Drop for Marker {
    /// End instrumentation event
    #[inline(always)]
    fn drop(&mut self) {
        #[cfg(feature = "superluminal")]
        superluminal_perf::end_event();
    }
}

#[macro_export]
macro_rules! function_name {
    () => {
        $crate::puffin::clean_function_name($crate::puffin::current_function_name!())
    };
}

#[macro_export]
macro_rules! file_name {
    () => {
        $crate::puffin::short_file_name(file!())
    };
}

#[macro_export]
macro_rules! profile_scope_owned {
    ($name:expr) => {
        $crate::profile_scope_owned!($name, "")
    };
    ($name:expr, $data:expr) => {
        {
            // Similar to Puffin's profile_scope macro.
            // https://github.com/EmbarkStudios/puffin/blob/5c4b6c597e98a1a5b96a21388a4df3bf542f7ed5/puffin/src/lib.rs#L250
            static FUNCTION_NAME: std::sync::OnceLock<String> =
                std::sync::OnceLock::new();
            let function_name = FUNCTION_NAME.get_or_init(|| {
                $crate::function_name!()
            });

            static SCOPE_ID: std::sync::OnceLock<$crate::puffin::ScopeId> =
                std::sync::OnceLock::new();
            let scope_id = SCOPE_ID.get_or_init(|| {
                $crate::puffin::ThreadProfiler::call(|tp| {
                    tp.register_named_scope(
                        $name,
                        function_name,
                        $crate::file_name!(),
                        line!(),
                    )
                })
            });
            $crate::Marker::new($name, *scope_id, $data)
        }
    };
}

#[macro_export]
macro_rules! profile_function_owned {
    () => {
        $crate::profile_function_owned!("")
    };
    ($data:expr) => {
        {
            // Similar to Puffin's profile_function macro.
            // https://github.com/EmbarkStudios/puffin/blob/5c4b6c597e98a1a5b96a21388a4df3bf542f7ed5/puffin/src/lib.rs#L213
            static FUNCTION_NAME: std::sync::OnceLock<String> =
                std::sync::OnceLock::new();
            let function_name = FUNCTION_NAME.get_or_init(|| {
                $crate::function_name!()
            });

            static SCOPE_ID: std::sync::OnceLock<$crate::puffin::ScopeId> =
                std::sync::OnceLock::new();
            let scope_id = SCOPE_ID.get_or_init(|| {
                $crate::puffin::ThreadProfiler::call(|tp| {
                    tp.register_function_scope(
                        function_name,
                        $crate::file_name!(),
                        line!(),
                    )
                })
            });
            $crate::Marker::new(
                function_name,
                *scope_id,
                $data,
            )
        }
    };
}

#[macro_export]
macro_rules! profile_scope {
    ($name:expr) => {
        $crate::profile_scope!($name, "")
    };
    ($name:expr, $data:expr) => {
        let _profiler_marker = $crate::profile_scope_owned!($name, $data);
    };
}

#[macro_export]
macro_rules! profile_function {
    () => {
        $crate::profile_function!("")
    };
    ($data:expr) => {
        let _profiler_marker = $crate::profile_function_owned!($data);
    };
}
