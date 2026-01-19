use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Configuration key not found: {0}")]
    KeyNotFound(String),

    #[error("Type mismatch for key '{key}': expected {expected}, got {got}")]
    TypeMismatch {
        key: String,
        expected: String,
        got: String,
    },

    #[error("Value out of range for key '{key}': value {value} not in [{min}, {max}]")]
    OutOfRange {
        key: String,
        value: f32,
        min: f32,
        max: f32,
    },

    #[error("Widget type mismatch for key: {0}")]
    WidgetTypeMismatch(String),

    #[error("Schema parse error: {0}")]
    SchemaParseError(#[from] toml::de::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(String),

    #[error("File I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Schema version mismatch: got {got}, expected {expected}")]
    VersionMismatch { got: u32, expected: u32 },

    #[error("Invalid config value: {0}")]
    InvalidValue(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] toml::ser::Error),
}

pub type Result<T> = std::result::Result<T, ConfigError>;
