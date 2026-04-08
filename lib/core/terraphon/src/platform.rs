crate::define_sexp_enum!(Platform, Any {
    MacOS => "macos",
    IOS => "ios",
    Android => "android",
    Linux => "linux",
    FreeBSD => "freebsd",
    Cloud => "cloud",
    Any => "any",
});

impl Platform {
    pub fn detect() -> Self {
        #[cfg(target_os = "macos")]  { return Self::MacOS; }
        #[cfg(target_os = "freebsd")] { return Self::FreeBSD; }
        #[cfg(target_os = "android")] { return Self::Android; }
        #[cfg(target_os = "ios")]     { return Self::IOS; }
        #[cfg(target_os = "linux")] {
            if is_cloud_env() { return Self::Cloud; }
            return Self::Linux;
        }
        #[allow(unreachable_code)]
        Self::Linux
    }
    pub fn matches(&self, lode_platform: Platform) -> bool {
        lode_platform == Platform::Any || lode_platform == *self
    }
}

#[cfg(target_os = "linux")]
fn is_cloud_env() -> bool {
    std::path::Path::new("/run/cloud-init").exists()
        || std::env::var("KUBERNETES_SERVICE_HOST").is_ok()
        || std::env::var("NOMAD_ALLOC_ID").is_ok()
        || std::env::var("AWS_EXECUTION_ENV").is_ok()
        || std::env::var("GOOGLE_CLOUD_PROJECT").is_ok()
}
