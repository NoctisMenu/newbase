pub mod codegen;
mod error;
pub mod keys;
pub mod macros;
mod schema;

pub use error::{ConfigError, Result};
pub use schema::{ConfigSchema, ConfigSection, FieldMetadata, FieldSchema, FieldType, WidgetType};

//use crate::widgets::{Checkbox, SmoothSlider, ToggleSwitch};
use newoverlay::imgui::ImColor32;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Runtime configuration value
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigValue {
    Bool(bool),
    Float(f32),
    Int(i32),
    Color { r: u8, g: u8, b: u8, a: u8 },
    Enum(String),
    String(String),
}

impl ConfigValue {
    /// Get type name for error messages
    pub fn type_name(&self) -> &str {
        match self {
            ConfigValue::Bool(_) => "bool",
            ConfigValue::Float(_) => "float",
            ConfigValue::Int(_) => "int",
            ConfigValue::Color { .. } => "color",
            ConfigValue::Enum(_) => "enum",
            ConfigValue::String(_) => "string",
        }
    }
}

/// Widget state wrapper for persistent UI state
pub enum WidgetState {
    //Checkbox { checkbox: Checkbox },
    //ToggleSwitch { toggle: ToggleSwitch },
    //SmoothSlider { slider: SmoothSlider },
    //ColorPicker { picker: crate::widgets::ColorPicker },
    //ComboBox { combobox: crate::widgets::ComboBox },
    None, // For fields without widgets (like hex strings)
}

/// Main configuration store
pub struct ConfigStore {
    schema: ConfigSchema,
    values: HashMap<String, ConfigValue>,
    widgets: HashMap<String, WidgetState>,
    user_config_path: PathBuf,
    dirty: bool,
    pub highlighted_field: Option<String>, // Field to highlight with glow effect
}

impl ConfigStore {
    const SCHEMA_PATH: &'static str = "app/config_schema.toml";
    const USER_CONFIG_PATH: &'static str = "config.toml";
    const CURRENT_VERSION: u32 = 1;

    /// Load config from schema and user overrides
    pub fn load() -> Result<Self> {
        Self::load_from_schema_path(Self::SCHEMA_PATH, Self::USER_CONFIG_PATH)
    }

    /// Load with explicit user config path and default schema path.
    ///
    /// Prefer `load_from_schema_path` or `load_from_schema_str` when the schema
    /// lives outside the library crate (e.g. in a consuming binary crate).
    pub fn load_from_paths(user_config_path: &str) -> Result<Self> {
        Self::load_from_schema_path(Self::SCHEMA_PATH, user_config_path)
    }

    /// Load config from a schema file path and user config path.
    pub fn load_from_schema_path(schema_path: &str, user_config_path: &str) -> Result<Self> {
        let schema_toml = std::fs::read_to_string(schema_path)?;
        Self::load_from_schema_str(&schema_toml, user_config_path)
    }

    /// Load config from a provided schema TOML string and user config path.
    ///
    /// This is useful for binaries that keep `config_schema.toml` in their own
    /// crate and pass it with:
    /// `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/config_schema.toml"))`.
    pub fn load_from_schema_str(schema_toml: &str, user_config_path: &str) -> Result<Self> {
        let schema: ConfigSchema = toml::from_str(schema_toml)
            .map_err(|e| ConfigError::TomlParse(format!("Failed to parse schema TOML: {}", e)))?;

        // Verify schema version
        if schema.version != Self::CURRENT_VERSION {
            return Err(ConfigError::VersionMismatch {
                got: schema.version,
                expected: Self::CURRENT_VERSION,
            });
        }

        // Initialize values from defaults
        let mut values = HashMap::new();
        for (section_name, section) in &schema.sections {
            for (field_name, field) in &section.fields {
                let key = format!("{}.{}", section_name, field_name);
                let value = Self::parse_default(&field.field_type, &field.default, &key)?;
                values.insert(key, value);
            }
        }

        // Load user config if exists, overriding defaults
        if let Ok(config_str) = std::fs::read_to_string(user_config_path) {
            match toml::from_str::<HashMap<String, toml::Value>>(&config_str) {
                Ok(user_values) => {
                    for (key, toml_value) in user_values {
                        // Find field schema
                        if let Some(field) = Self::find_field_schema(&schema, &key) {
                            if field.public {
                                match Self::parse_value(&field.field_type, &toml_value, &key) {
                                    Ok(value) => {
                                        values.insert(key.clone(), value);
                                    }
                                    Err(e) => {
                                        log::warn!(
                                            "Failed to parse config value for '{}': {}. Using default.",
                                            key,
                                            e
                                        );
                                    }
                                }
                            } else {
                                log::warn!("Ignoring private field '{}' in user config", key);
                            }
                        } else {
                            log::warn!("Unknown config key '{}' in user config - ignoring", key);
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to parse user config: {}. Using defaults.", e);
                }
            }
        }

        // Create widgets from schema
        let widgets = Self::create_widgets(&schema, &values);

        Ok(Self {
            schema,
            values,
            widgets,
            user_config_path: PathBuf::from(user_config_path),
            dirty: false,
            highlighted_field: None,
        })
    }

    /// Load with fallback to defaults on error
    pub fn load_with_fallback() -> Self {
        match Self::load() {
            Ok(store) => store,
            Err(e) => {
                log::error!("Failed to load config: {}. Using defaults.", e);
                Self::empty_store(Self::USER_CONFIG_PATH)
            }
        }
    }

    /// Create config from schema defaults only
    fn from_defaults() -> Self {
        Self::empty_store(Self::USER_CONFIG_PATH)
    }

    fn empty_store(user_config_path: &str) -> Self {
        Self {
            schema: ConfigSchema {
                version: Self::CURRENT_VERSION,
                sections: HashMap::new(),
            },
            values: HashMap::new(),
            widgets: HashMap::new(),
            user_config_path: PathBuf::from(user_config_path),
            dirty: false,
            highlighted_field: None,
        }
    }

    /// Reload configuration from disk
    pub fn reload(&mut self) -> Result<()> {
        let new_store = Self::load()?;
        self.values = new_store.values;
        // Don't replace widgets - sync values to existing widgets to preserve animation state
        self.dirty = false;
        Ok(())
    }

    /// Get reference to schema
    pub fn schema(&self) -> &ConfigSchema {
        &self.schema
    }

    /// Return the current value for a dotted schema key.
    pub fn value(&self, key: &str) -> Option<&ConfigValue> {
        self.values.get(key)
    }

    /// Find field schema by dotted key
    fn find_field_schema<'a>(schema: &'a ConfigSchema, key: &str) -> Option<&'a FieldSchema> {
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() != 2 {
            return None;
        }
        let (section_name, field_name) = (parts[0], parts[1]);
        schema.sections.get(section_name)?.fields.get(field_name)
    }

    /// Create widget instances from schema
    fn create_widgets(
        _schema: &ConfigSchema,
        _values: &HashMap<String, ConfigValue>,
    ) -> HashMap<String, WidgetState> {
        let widgets = HashMap::new();

        /*
        for (section_name, section) in &schema.sections {
            for (field_name, field) in &section.fields {
                let key = format!("{}.{}", section_name, field_name);

                let widget = match &field.widget_type {
                    WidgetType::Checkbox => {
                        let enabled = if let Some(ConfigValue::Bool(v)) = values.get(&key) {
                            *v
                        } else {
                            false
                        };
                        let mut checkbox = Checkbox::new(enabled);
                        let target = if enabled { 1.0 } else { 0.0 };
                        // Set animation to final state (no animation on init)
                        checkbox.animation.progress = target;
                        checkbox.animation.set_values((target, target));
                        checkbox.animation.animation_start = None;
                        WidgetState::Checkbox { checkbox }
                    }
                    WidgetType::Toggle => {
                        let mut toggle = ToggleSwitch::default();
                        // Initialize with config value
                        if let Some(ConfigValue::Bool(v)) = values.get(&key) {
                            toggle.enabled = *v;
                            let target = if *v { 1.0 } else { 0.0 };
                            // Set animation to final state (no animation on init)
                            toggle.animation.progress = target;
                            toggle.animation.set_values((target, target));
                            toggle.animation.animation_start = None;
                        }
                        WidgetState::ToggleSwitch { toggle }
                    }
                    WidgetType::SmoothSlider { width, height } => {
                        let (min, max) = match &field.field_type {
                            FieldType::Float { min, max } => (*min, *max),
                            _ => (0.0, 1.0),
                        };
                        let default_value = if let Some(ConfigValue::Float(v)) = values.get(&key) {
                            *v
                        } else {
                            min
                        };

                        let mut slider = SmoothSlider::new(default_value, min, max);
                        if let Some(w) = width {
                            slider = slider.width(*w);
                        }
                        if let Some(h) = height {
                            slider = slider.height(*h);
                        }
                        WidgetState::SmoothSlider { slider }
                    }
                    WidgetType::ColorPicker => {
                        let default_color =
                            if let Some(ConfigValue::Color { r, g, b, a }) = values.get(&key) {
                                egui::Color32::from_rgba_unmultiplied(*r, *g, *b, *a)
                            } else {
                                egui::Color32::WHITE
                            };
                        let picker = crate::widgets::ColorPicker::new(default_color);
                        WidgetState::ColorPicker { picker }
                    }
                    WidgetType::ComboBox => {
                        // Get enum variants from field type
                        if let FieldType::Enum { variants } = &field.field_type {
                            let current_value = if let Some(ConfigValue::Enum(v)) = values.get(&key)
                            {
                                v.clone()
                            } else {
                                variants.first().cloned().unwrap_or_default()
                            };

                            let selected_index = variants
                                .iter()
                                .position(|v| v == &current_value)
                                .unwrap_or(0);
                            let combobox = crate::widgets::ComboBox::new(
                                &key,
                                variants.clone(),
                                selected_index,
                            );
                            WidgetState::ComboBox { combobox }
                        } else {
                            WidgetState::None
                        }
                    }
                    WidgetType::None => WidgetState::None,
                };

                widgets.insert(key, widget);
            }
        }
         */

        widgets
    }

    /// Parse default value from TOML
    fn parse_default(
        field_type: &FieldType,
        default: &toml::Value,
        key: &str,
    ) -> Result<ConfigValue> {
        Self::parse_value(field_type, default, key)
    }

    /// Parse value from TOML based on field type
    fn parse_value(field_type: &FieldType, value: &toml::Value, key: &str) -> Result<ConfigValue> {
        match field_type {
            FieldType::Bool => {
                if let Some(b) = value.as_bool() {
                    Ok(ConfigValue::Bool(b))
                } else {
                    Err(ConfigError::InvalidValue(format!(
                        "Expected bool for '{}', got {:?}",
                        key, value
                    )))
                }
            }
            FieldType::Float { min, max } => {
                let f = if let Some(f) = value.as_float() {
                    f as f32
                } else if let Some(i) = value.as_integer() {
                    i as f32
                } else {
                    return Err(ConfigError::InvalidValue(format!(
                        "Expected float for '{}', got {:?}",
                        key, value
                    )));
                };

                if f < *min || f > *max {
                    return Err(ConfigError::OutOfRange {
                        key: key.to_string(),
                        value: f,
                        min: *min,
                        max: *max,
                    });
                }
                Ok(ConfigValue::Float(f))
            }
            FieldType::Int { min, max } => {
                if let Some(i) = value.as_integer() {
                    let i = i as i32;
                    if i < *min || i > *max {
                        return Err(ConfigError::InvalidValue(format!(
                            "Value {} out of range [{}, {}] for '{}'",
                            i, min, max, key
                        )));
                    }
                    Ok(ConfigValue::Int(i))
                } else {
                    Err(ConfigError::InvalidValue(format!(
                        "Expected int for '{}', got {:?}",
                        key, value
                    )))
                }
            }
            FieldType::Color => {
                if let Some(s) = value.as_str() {
                    // Parse hex color like "#FF0000" or "#FF0000FF"
                    Self::parse_hex_color(s)
                } else if let Some(table) = value.as_table() {
                    // Parse RGBA table like { r = 255, g = 0, b = 0, a = 255 }
                    let r = table.get("r").and_then(|v| v.as_integer()).unwrap_or(0) as u8;
                    let g = table.get("g").and_then(|v| v.as_integer()).unwrap_or(0) as u8;
                    let b = table.get("b").and_then(|v| v.as_integer()).unwrap_or(0) as u8;
                    let a = table.get("a").and_then(|v| v.as_integer()).unwrap_or(255) as u8;
                    Ok(ConfigValue::Color { r, g, b, a })
                } else {
                    Err(ConfigError::InvalidValue(format!(
                        "Expected color (hex string or RGBA table) for '{}', got {:?}",
                        key, value
                    )))
                }
            }
            FieldType::Enum { variants } => {
                if let Some(s) = value.as_str() {
                    if variants.contains(&s.to_string()) {
                        Ok(ConfigValue::Enum(s.to_string()))
                    } else {
                        Err(ConfigError::InvalidValue(format!(
                            "Invalid enum value '{}' for '{}'. Valid values: {:?}",
                            s, key, variants
                        )))
                    }
                } else {
                    Err(ConfigError::InvalidValue(format!(
                        "Expected string for enum '{}', got {:?}",
                        key, value
                    )))
                }
            }
            FieldType::String => {
                if let Some(s) = value.as_str() {
                    Ok(ConfigValue::String(s.to_string()))
                } else {
                    Err(ConfigError::InvalidValue(format!(
                        "Expected string for '{}', got {:?}",
                        key, value
                    )))
                }
            }
        }
    }

    /// Parse hex color string
    fn parse_hex_color(hex: &str) -> Result<ConfigValue> {
        let hex = hex.trim_start_matches('#');

        let (r, g, b, a) = match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| {
                    ConfigError::InvalidValue(format!("Invalid hex color: #{}", hex))
                })?;
                let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| {
                    ConfigError::InvalidValue(format!("Invalid hex color: #{}", hex))
                })?;
                let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| {
                    ConfigError::InvalidValue(format!("Invalid hex color: #{}", hex))
                })?;
                (r, g, b, 255)
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| {
                    ConfigError::InvalidValue(format!("Invalid hex color: #{}", hex))
                })?;
                let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| {
                    ConfigError::InvalidValue(format!("Invalid hex color: #{}", hex))
                })?;
                let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| {
                    ConfigError::InvalidValue(format!("Invalid hex color: #{}", hex))
                })?;
                let a = u8::from_str_radix(&hex[6..8], 16).map_err(|_| {
                    ConfigError::InvalidValue(format!("Invalid hex color: #{}", hex))
                })?;
                (r, g, b, a)
            }
            _ => {
                return Err(ConfigError::InvalidValue(format!(
                    "Invalid hex color length: #{}",
                    hex
                )));
            }
        };

        Ok(ConfigValue::Color { r, g, b, a })
    }

    /// Convert ConfigValue to TOML value
    fn value_to_toml(value: &ConfigValue) -> toml::Value {
        match value {
            ConfigValue::Bool(b) => toml::Value::Boolean(*b),
            ConfigValue::Float(f) => toml::Value::Float(*f as f64),
            ConfigValue::Int(i) => toml::Value::Integer(*i as i64),
            ConfigValue::Color { r, g, b, a } => {
                // Save as hex string
                toml::Value::String(format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, a))
            }
            ConfigValue::Enum(s) | ConfigValue::String(s) => toml::Value::String(s.clone()),
        }
    }

    /// Save only public fields to user config
    pub fn save(&mut self) -> Result<()> {
        let mut public_values = toml::map::Map::new();

        for (section_name, section) in &self.schema.sections {
            for (field_name, field) in &section.fields {
                let key = format!("{}.{}", section_name, field_name);

                if field.public
                    && let Some(value) = self.values.get(&key)
                {
                    public_values.insert(key, Self::value_to_toml(value));
                }
            }
        }

        let toml_str = toml::to_string_pretty(&public_values)?;
        std::fs::write(&self.user_config_path, toml_str)?;
        self.dirty = false;

        log::info!("Saved config to {:?}", self.user_config_path);
        Ok(())
    }

    /// Save if dirty
    pub fn save_if_dirty(&mut self) -> Result<()> {
        if self.dirty {
            self.save()?;
        }
        Ok(())
    }

    // Type-safe getters

    pub fn get_bool(&self, key: &str) -> Result<bool> {
        match self.values.get(key) {
            Some(ConfigValue::Bool(v)) => Ok(*v),
            Some(v) => Err(ConfigError::TypeMismatch {
                key: key.to_string(),
                expected: "bool".to_string(),
                got: v.type_name().to_string(),
            }),
            None => Err(ConfigError::KeyNotFound(key.to_string())),
        }
    }

    pub fn get_float(&self, key: &str) -> Result<f32> {
        match self.values.get(key) {
            Some(ConfigValue::Float(v)) => Ok(*v),
            Some(v) => Err(ConfigError::TypeMismatch {
                key: key.to_string(),
                expected: "float".to_string(),
                got: v.type_name().to_string(),
            }),
            None => Err(ConfigError::KeyNotFound(key.to_string())),
        }
    }

    pub fn get_color(&self, key: &str) -> Result<ImColor32> {
        match self.values.get(key) {
            Some(ConfigValue::Color { r, g, b, a }) => Ok(ImColor32::from_rgba(*r, *g, *b, *a)),
            Some(v) => Err(ConfigError::TypeMismatch {
                key: key.to_string(),
                expected: "color".to_string(),
                got: v.type_name().to_string(),
            }),
            None => Err(ConfigError::KeyNotFound(key.to_string())),
        }
    }

    pub fn get_enum(&self, key: &str) -> Result<String> {
        match self.values.get(key) {
            Some(ConfigValue::Enum(v)) => Ok(v.clone()),
            Some(v) => Err(ConfigError::TypeMismatch {
                key: key.to_string(),
                expected: "enum".to_string(),
                got: v.type_name().to_string(),
            }),
            None => Err(ConfigError::KeyNotFound(key.to_string())),
        }
    }

    pub fn get_string(&self, key: &str) -> Result<String> {
        match self.values.get(key) {
            Some(ConfigValue::String(v)) => Ok(v.clone()),
            Some(v) => Err(ConfigError::TypeMismatch {
                key: key.to_string(),
                expected: "string".to_string(),
                got: v.type_name().to_string(),
            }),
            None => Err(ConfigError::KeyNotFound(key.to_string())),
        }
    }

    // Type-safe setters

    pub fn set_bool(&mut self, key: &str, value: bool) -> Result<()> {
        if Self::find_field_schema(&self.schema, key).is_some() {
            self.values
                .insert(key.to_string(), ConfigValue::Bool(value));
            self.dirty = true;
            Ok(())
        } else {
            Err(ConfigError::KeyNotFound(key.to_string()))
        }
    }

    pub fn set_float(&mut self, key: &str, value: f32) -> Result<()> {
        // Validate range from schema
        if let Some(field) = Self::find_field_schema(&self.schema, key) {
            if let FieldType::Float { min, max } = field.field_type
                && (value < min || value > max)
            {
                return Err(ConfigError::OutOfRange {
                    key: key.to_string(),
                    value,
                    min,
                    max,
                });
            }
            self.values
                .insert(key.to_string(), ConfigValue::Float(value));
            self.dirty = true;
            Ok(())
        } else {
            Err(ConfigError::KeyNotFound(key.to_string()))
        }
    }

    pub fn set_int(&mut self, key: &str, value: i32) -> Result<()> {
        if let Some(field) = Self::find_field_schema(&self.schema, key) {
            if let FieldType::Int { min, max } = field.field_type {
                if value < min || value > max {
                    return Err(ConfigError::InvalidValue(format!(
                        "Value {} out of range [{}, {}] for '{}'",
                        value, min, max, key
                    )));
                }
            } else {
                return Err(ConfigError::TypeMismatch {
                    key: key.to_string(),
                    expected: "int".to_string(),
                    got: field.field_type.type_name().to_string(),
                });
            }
            self.values.insert(key.to_string(), ConfigValue::Int(value));
            self.dirty = true;
            Ok(())
        } else {
            Err(ConfigError::KeyNotFound(key.to_string()))
        }
    }

    pub fn set_color(&mut self, key: &str, color: ImColor32) -> Result<()> {
        let [r, g, b, a] = color.to_rgba();
        self.set_color_rgba(key, r, g, b, a)
    }

    pub fn set_color_rgba(&mut self, key: &str, r: u8, g: u8, b: u8, a: u8) -> Result<()> {
        match Self::find_field_schema(&self.schema, key) {
            Some(field) if matches!(field.field_type, FieldType::Color) => {
                self.values
                    .insert(key.to_string(), ConfigValue::Color { r, g, b, a });
                self.dirty = true;
                Ok(())
            }
            Some(field) => Err(ConfigError::TypeMismatch {
                key: key.to_string(),
                expected: "color".to_string(),
                got: field.field_type.type_name().to_string(),
            }),
            None => Err(ConfigError::KeyNotFound(key.to_string())),
        }
    }

    pub fn set_enum(&mut self, key: &str, value: String) -> Result<()> {
        // Validate against variants
        if let Some(field) = Self::find_field_schema(&self.schema, key) {
            if let FieldType::Enum { variants } = &field.field_type
                && !variants.contains(&value)
            {
                return Err(ConfigError::InvalidValue(format!(
                    "Invalid enum value '{}' for '{}'. Valid values: {:?}",
                    value, key, variants
                )));
            }
            self.values
                .insert(key.to_string(), ConfigValue::Enum(value));
            self.dirty = true;
            Ok(())
        } else {
            Err(ConfigError::KeyNotFound(key.to_string()))
        }
    }

    pub fn set_string(&mut self, key: &str, value: String) -> Result<()> {
        if Self::find_field_schema(&self.schema, key).is_some() {
            self.values
                .insert(key.to_string(), ConfigValue::String(value));
            self.dirty = true;
            Ok(())
        } else {
            Err(ConfigError::KeyNotFound(key.to_string()))
        }
    }

    /// Get field schema by key
    pub fn get_field_schema(&self, key: &str) -> Option<&FieldSchema> {
        Self::find_field_schema(&self.schema, key)
    }

    /// Highlight a field (used when navigating from search)
    pub fn highlight_field(&mut self, key: String) {
        self.highlighted_field = Some(key);
    }

    /// Clear field highlight
    pub fn clear_highlight(&mut self) {
        self.highlighted_field = None;
    }
}
