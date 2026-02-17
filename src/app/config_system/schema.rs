use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigSchema {
    #[serde(default = "default_version")]
    pub version: u32,
    pub sections: HashMap<String, ConfigSection>,
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigSection {
    pub display_name: String,
    #[serde(default)]
    pub fields: HashMap<String, FieldSchema>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FieldSchema {
    #[serde(flatten)]
    pub field_type: FieldType,
    pub widget_type: WidgetType,
    pub default: toml::Value,
    #[serde(default = "default_public")]
    pub public: bool,
    pub metadata: FieldMetadata,
}

fn default_public() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum FieldType {
    Bool,
    Float { min: f32, max: f32 },
    Int { min: i32, max: i32 },
    Color,
    Enum { variants: Vec<String> },
    String,
}

impl FieldType {
    pub fn type_name(&self) -> &str {
        match self {
            FieldType::Bool => "bool",
            FieldType::Float { .. } => "float",
            FieldType::Int { .. } => "int",
            FieldType::Color => "color",
            FieldType::Enum { .. } => "enum",
            FieldType::String => "string",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum WidgetType {
    Checkbox,
    Toggle,
    SmoothSlider {
        width: Option<f32>,
        height: Option<f32>,
    },
    ColorPicker,
    ComboBox,
    None,
}

// Custom deserializer to support both simple strings and detailed objects
impl<'de> serde::Deserialize<'de> for WidgetType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum WidgetTypeHelper {
            // Simple string format: widget_type = "checkbox"
            String(String),
            // Detailed format: widget_type = { widget = "smoothslider", width = 100.0 }
            Detailed {
                widget: String,
                #[serde(default)]
                width: Option<f32>,
                #[serde(default)]
                height: Option<f32>,
            },
        }

        match WidgetTypeHelper::deserialize(deserializer)? {
            WidgetTypeHelper::String(s) => match s.as_str() {
                "checkbox" => Ok(WidgetType::Checkbox),
                "toggle" => Ok(WidgetType::Toggle),
                "smoothslider" => Ok(WidgetType::SmoothSlider {
                    width: None,
                    height: None,
                }),
                "colorpicker" => Ok(WidgetType::ColorPicker),
                "combobox" => Ok(WidgetType::ComboBox),
                "none" => Ok(WidgetType::None),
                _ => Err(D::Error::custom(format!("Unknown widget type: {}", s))),
            },
            WidgetTypeHelper::Detailed {
                widget,
                width,
                height,
            } => match widget.as_str() {
                "checkbox" => Ok(WidgetType::Checkbox),
                "toggle" => Ok(WidgetType::Toggle),
                "smoothslider" => Ok(WidgetType::SmoothSlider { width, height }),
                "colorpicker" => Ok(WidgetType::ColorPicker),
                "combobox" => Ok(WidgetType::ComboBox),
                "none" => Ok(WidgetType::None),
                _ => Err(D::Error::custom(format!("Unknown widget type: {}", widget))),
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FieldMetadata {
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tooltip: String,
    pub category: String,
}
