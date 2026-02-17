/// Auto-generated config access macros
/// DO NOT EDIT - Generated from config_schema.toml
/// Regenerate via crate::app::config_system::codegen::regenerate_generated_files()
/// Get a config value by field name string
///
/// # Examples
/// ```
/// let enabled = config!(self, "aim_enabled");
/// let fov = config!(self, "aim_fov");
/// ```
#[macro_export]
macro_rules! config {
    ($app:expr, "aim_enabled") => {
        $app.config_store.read().get_bool($crate::app::config_system::keys::AIM_ENABLED).unwrap_or(false)
    };
    ($app:expr, "aim_fov") => {
        $app.config_store.read().get_float($crate::app::config_system::keys::AIM_FOV).unwrap_or(5.0)
    };
    ($app:expr, "aim_smoothing") => {
        $app.config_store.read().get_float($crate::app::config_system::keys::AIM_SMOOTHING).unwrap_or(0.0)
    };
    ($app:expr, "accent_color") => {
        $app.config_store.read().get_color($crate::app::config_system::keys::ACCENT_COLOR).unwrap_or(egui::Color32::GREEN)
    };
    ($app:expr, "discord_presence") => {
        $app.config_store.read().get_bool($crate::app::config_system::keys::DISCORD_PRESENCE).unwrap_or(true)
    };
    ($app:expr, "streamproof") => {
        $app.config_store.read().get_bool($crate::app::config_system::keys::STREAMPROOF).unwrap_or(true)
    };
    ($app:expr, "enable_esp") => {
        $app.config_store.read().get_bool($crate::app::config_system::keys::ENABLE_ESP).unwrap_or(true)
    };
}

/// Get a config value directly from ConfigStore
///
/// # Examples
/// ```
/// let enabled = config_get!(config_store, "aim_enabled");
/// ```
#[macro_export]
macro_rules! config_get {
    ($store:expr, "aim_enabled") => {
        $store.get_bool($crate::app::config_system::keys::AIM_ENABLED).unwrap_or(false)
    };
    ($store:expr, "aim_fov") => {
        $store.get_float($crate::app::config_system::keys::AIM_FOV).unwrap_or(5.0)
    };
    ($store:expr, "aim_smoothing") => {
        $store.get_float($crate::app::config_system::keys::AIM_SMOOTHING).unwrap_or(0.0)
    };
    ($store:expr, "accent_color") => {
        $store.get_color($crate::app::config_system::keys::ACCENT_COLOR).unwrap_or(egui::Color32::GREEN)
    };
    ($store:expr, "discord_presence") => {
        $store.get_bool($crate::app::config_system::keys::DISCORD_PRESENCE).unwrap_or(true)
    };
    ($store:expr, "streamproof") => {
        $store.get_bool($crate::app::config_system::keys::STREAMPROOF).unwrap_or(true)
    };
    ($store:expr, "enable_esp") => {
        $store.get_bool($crate::app::config_system::keys::ENABLE_ESP).unwrap_or(true)
    };
}

/// Set a config value by field name string
///
/// # Examples
/// ```
/// config_set!(self, "aim_enabled", true);
/// config_set!(self, "aim_fov", 10.0);
/// ```
#[macro_export]
macro_rules! config_set {
    ($app:expr, "aim_enabled", $value:expr) => {
        $app.config_store.write().set_bool($crate::app::config_system::keys::AIM_ENABLED, $value).ok()
    };
    ($app:expr, "aim_fov", $value:expr) => {
        $app.config_store.write().set_float($crate::app::config_system::keys::AIM_FOV, $value).ok()
    };
    ($app:expr, "aim_smoothing", $value:expr) => {
        $app.config_store.write().set_float($crate::app::config_system::keys::AIM_SMOOTHING, $value).ok()
    };
    ($app:expr, "accent_color", $value:expr) => {
        $app.config_store.write().set_color($crate::app::config_system::keys::ACCENT_COLOR, $value).ok()
    };
    ($app:expr, "discord_presence", $value:expr) => {
        $app.config_store.write().set_bool($crate::app::config_system::keys::DISCORD_PRESENCE, $value).ok()
    };
    ($app:expr, "streamproof", $value:expr) => {
        $app.config_store.write().set_bool($crate::app::config_system::keys::STREAMPROOF, $value).ok()
    };
    ($app:expr, "enable_esp", $value:expr) => {
        $app.config_store.write().set_bool($crate::app::config_system::keys::ENABLE_ESP, $value).ok()
    };
}
