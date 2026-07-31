use super::config_system::{ConfigSection, ConfigStore, ConfigValue, FieldSchema, FieldType, Key};
use serde_json::json;
use std::collections::BTreeMap;

const TEMPLATE: &str = include_str!("../../resources/grim_menu.html");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuCommand {
    None,
    Ready,
    Quit,
    SetVisible(bool),
    SetLogLevel(log::LevelFilter),
    SetSearchFocused(bool),
}

/// Build the menu HTML: the Nimbus template with the config schema injected as
/// `window.__GRIM_DATA`. All rendering/interaction lives in the template JS.
pub(crate) fn build_html(store: &ConfigStore) -> String {
    TEMPLATE.replace("{{DATA}}", &build_menu_data(store))
}

/// Serialize the config schema into the JSON the template renders from:
/// sidebar categories ← `section.group`, module cards ← sections, module
/// settings ← the section's fields (with `enabled` pulled out as the toggle).
pub(crate) fn build_menu_data(store: &ConfigStore) -> String {
    let schema = store.schema();

    // group sections into sidebar categories (BTreeMap → stable, sorted order)
    let mut groups: BTreeMap<String, Vec<(&String, &ConfigSection)>> = BTreeMap::new();
    for (section_key, section) in &schema.sections {
        let group = if section.group.trim().is_empty() {
            "Features".to_owned()
        } else {
            section.group.clone()
        };
        groups.entry(group).or_default().push((section_key, section));
    }

    let categories: Vec<serde_json::Value> = groups
        .iter()
        .map(|(group_name, sections)| {
            let mut sections = sections.clone();
            sections.sort_by(|a, b| a.1.display_name.cmp(&b.1.display_name).then(a.0.cmp(b.0)));
            let modules: Vec<serde_json::Value> = sections
                .iter()
                .map(|(key, section)| build_module(store, key, section))
                .collect();
            json!({
                "id": slug(group_name),
                "name": group_name,
                "icon": slug(group_name),
                "modules": modules,
            })
        })
        .collect();

    json!({
        "brand": "Nimbus Menu",
        "version": "4.2",
        "categories": categories,
    })
    .to_string()
}

fn build_module(store: &ConfigStore, section_key: &str, section: &ConfigSection) -> serde_json::Value {
    let has_enabled = matches!(
        section.fields.get("enabled").map(|f| &f.field_type),
        Some(FieldType::Bool)
    );
    let enabled_key = format!("{section_key}.enabled");
    let enabled = has_enabled && matches!(store.value(&enabled_key), Some(ConfigValue::Bool(true)));

    // every public field except `enabled` becomes a settings row, ordered by
    // category → display name → key (mirrors the old render_sections ordering)
    let mut fields: Vec<(&String, &FieldSchema)> = section
        .fields
        .iter()
        .filter(|(field_key, field)| field_key.as_str() != "enabled" && field.public)
        .collect();
    fields.sort_by(|a, b| {
        a.1.metadata
            .category
            .cmp(&b.1.metadata.category)
            .then(a.1.metadata.display_name.cmp(&b.1.metadata.display_name))
            .then(a.0.cmp(b.0))
    });

    let settings: Vec<serde_json::Value> = fields
        .iter()
        .map(|(field_key, field)| {
            let key = format!("{section_key}.{field_key}");
            let value = store.value(&key);
            build_setting(field_key, field, &key, value)
        })
        .collect();

    json!({
        "key": section_key,
        "name": section.display_name,
        "desc": section.description,
        "enabled": enabled,
        "enabledKey": if has_enabled { Some(enabled_key) } else { None },
        "settings": settings,
    })
}

fn build_setting(
    field_key: &str,
    field: &FieldSchema,
    key: &str,
    value: Option<&ConfigValue>,
) -> serde_json::Value {
    let label = &field.metadata.display_name;
    let is_keybind = field_key.eq_ignore_ascii_case("keybind");
    match &field.field_type {
        FieldType::Color => {
            json!({"key": key, "label": label, "kind": "color", "value": color_hex(value)})
        }
        FieldType::Bool => {
            json!({"key": key, "label": label, "kind": "toggle", "value": as_bool(value)})
        }
        // a `keybind`-named enum/string field uses the strongly-typed Key list
        FieldType::Enum { .. } if is_keybind => json!({
            "key": key, "label": label, "kind": "keybind",
            "value": as_string(value), "variants": Key::variant_names(),
        }),
        FieldType::String if is_keybind => json!({
            "key": key, "label": label, "kind": "keybind",
            "value": as_string(value), "variants": Key::variant_names(),
        }),
        FieldType::Enum { variants } => json!({
            "key": key, "label": label, "kind": "enum",
            "value": as_string(value), "variants": variants,
        }),
        FieldType::Int { min, max } => json!({
            "key": key, "label": label, "kind": "number", "numberType": "int",
            "value": numeric_value(value), "min": min, "max": max,
        }),
        FieldType::Float { min, max } => json!({
            "key": key, "label": label, "kind": "number", "numberType": "float",
            "value": numeric_value(value), "min": min, "max": max,
        }),
        FieldType::String => {
            json!({"key": key, "label": label, "kind": "text", "value": as_string(value)})
        }
    }
}

fn color_hex(value: Option<&ConfigValue>) -> String {
    match value {
        Some(ConfigValue::Color { r, g, b, .. }) => format!("#{r:02X}{g:02X}{b:02X}"),
        _ => "#8B5CF6".to_owned(),
    }
}

fn as_bool(value: Option<&ConfigValue>) -> bool {
    matches!(value, Some(ConfigValue::Bool(true)))
}

fn as_string(value: Option<&ConfigValue>) -> String {
    match value {
        Some(ConfigValue::Enum(s) | ConfigValue::String(s)) => s.clone(),
        Some(other) => value_display(Some(other)),
        None => "None".to_owned(),
    }
}

fn value_display(value: Option<&ConfigValue>) -> String {
    match value {
        Some(ConfigValue::Bool(v)) => v.to_string(),
        Some(ConfigValue::Int(v)) => v.to_string(),
        Some(ConfigValue::Float(v)) => format!("{v:.2}"),
        Some(ConfigValue::Enum(v) | ConfigValue::String(v)) => v.clone(),
        Some(ConfigValue::Color { r, g, b, .. }) => format!("#{r:02X}{g:02X}{b:02X}"),
        None => "—".to_owned(),
    }
}

fn numeric_value(value: Option<&ConfigValue>) -> f64 {
    match value {
        Some(ConfigValue::Int(value)) => *value as f64,
        Some(ConfigValue::Float(value)) => *value as f64,
        _ => 0.0,
    }
}

/// Lowercase alnum slug; spaces/dashes/underscores become '-'.
fn slug(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if matches!(ch, ' ' | '-' | '_') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_owned()
}

pub(crate) fn apply_message(store: &mut ConfigStore, message: &str) -> Result<MenuCommand, String> {
    let payload: serde_json::Value =
        serde_json::from_str(message).map_err(|error| error.to_string())?;
    match payload.get("type").and_then(serde_json::Value::as_str) {
        Some("debug") => {
            let message = payload
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("(no message)");
            let details = payload
                .get("details")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            log::info!("WebView menu: {message} (details={details})");
            Ok(MenuCommand::None)
        }
        Some("ready") => Ok(MenuCommand::Ready),
        Some("quit") => Ok(MenuCommand::Quit),
        Some("visibility") => Ok(MenuCommand::SetVisible(
            payload
                .get("visible")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
        )),
        Some("search_focus") => Ok(MenuCommand::SetSearchFocused(true)),
        Some("search_blur") => Ok(MenuCommand::SetSearchFocused(false)),
        Some("config") => {
            let key = payload
                .get("key")
                .and_then(serde_json::Value::as_str)
                .ok_or("config message has no key")?;
            let value = payload.get("value").ok_or("config message has no value")?;
            let field_type = store
                .get_field_schema(key)
                .map(|field| field.field_type.clone())
                .ok_or_else(|| format!("unknown config key '{key}'"))?;
            match field_type {
                FieldType::Bool => store.set_bool(key, value.as_bool().ok_or("expected bool")?),
                FieldType::Float { .. } => {
                    store.set_float(key, value.as_f64().ok_or("expected number")? as f32)
                }
                FieldType::Int { .. } => {
                    store.set_int(key, value.as_i64().ok_or("expected integer")? as i32)
                }
                FieldType::Color => {
                    let channel = |name| {
                        value
                            .get(name)
                            .and_then(serde_json::Value::as_u64)
                            .map(|v| v.min(255) as u8)
                    };
                    store.set_color_rgba(
                        key,
                        channel("r").ok_or("color has no red channel")?,
                        channel("g").ok_or("color has no green channel")?,
                        channel("b").ok_or("color has no blue channel")?,
                        channel("a").unwrap_or(255),
                    )
                }
                FieldType::Enum { .. } => store.set_enum(
                    key,
                    value.as_str().ok_or("expected enum string")?.to_owned(),
                ),
                FieldType::String => {
                    store.set_string(key, value.as_str().ok_or("expected string")?.to_owned())
                }
            }
            .map_err(|error| error.to_string())?;
            Ok(MenuCommand::None)
        }
        Some("js_error") => Err(format!("WebView JavaScript error: {payload}")),
        Some("log_level") => {
            let level = match payload
                .get("level")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("info")
            {
                "error" => log::LevelFilter::Error,
                "warn" => log::LevelFilter::Warn,
                "debug" => log::LevelFilter::Debug,
                "trace" => log::LevelFilter::Trace,
                _ => log::LevelFilter::Info,
            };
            Ok(MenuCommand::SetLogLevel(level))
        }
        _ => Ok(MenuCommand::None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEMA: &str = r##"version = 1
[sections.skeleton_esp]
display_name = "Skeleton ESP"
group = "Visuals"
description = "Renders skeleton on player models"
[sections.skeleton_esp.fields.enabled]
type = "bool"
widget_type = "toggle"
default = false
[sections.skeleton_esp.fields.enabled.metadata]
display_name = "Enabled"
category = "General"
[sections.skeleton_esp.fields.color]
type = "color"
widget_type = "colorpicker"
default = "#8B5CF6FF"
[sections.skeleton_esp.fields.color.metadata]
display_name = "Color"
category = "General"
[sections.skeleton_esp.fields.keybind]
type = "string"
widget_type = "none"
default = "None"
[sections.skeleton_esp.fields.keybind.metadata]
display_name = "Keybind"
category = "General"
"##;

    #[test]
    fn build_html_injects_grouped_menu_data() {
        let store = ConfigStore::load_from_schema_str(SCHEMA, "target/test-grim.toml").unwrap();
        let html = build_html(&store);
        assert!(html.contains("window.__GRIM_DATA = "));
        assert!(!html.contains("{{DATA}}"));

        let data = build_menu_data(&store);
        assert!(data.contains("\"id\":\"visuals\""));
        assert!(data.contains("Skeleton ESP"));
        assert!(data.contains("\"enabledKey\":\"skeleton_esp.enabled\""));
        assert!(data.contains("\"kind\":\"keybind\""));
        assert!(data.contains("\"kind\":\"color\""));
    }

    #[test]
    fn ipc_messages_update_typed_config_values() {
        let mut store = ConfigStore::load_from_schema_str(
            r##"version = 1
[sections.runtime]
display_name = "Runtime"
[sections.runtime.fields.enabled]
type = "bool"
widget_type = "toggle"
default = false
[sections.runtime.fields.enabled.metadata]
display_name = "Enabled"
category = "General"
[sections.runtime.fields.amount]
type = "int"
min = 0
max = 10
widget_type = "smoothslider"
default = 1
[sections.runtime.fields.amount.metadata]
display_name = "Amount"
category = "General"
[sections.runtime.fields.tint]
type = "color"
widget_type = "colorpicker"
default = "#000000FF"
[sections.runtime.fields.tint.metadata]
display_name = "Tint"
category = "General"
"##,
            "target/test-web-menu-ipc.toml",
        )
        .unwrap();

        let data = build_menu_data(&store);
        assert!(data.contains("\"kind\":\"number\""));
        assert!(data.contains("\"numberType\":\"int\""));
        assert!(data.contains("\"min\":0"));
        assert!(data.contains("\"max\":10"));

        assert_eq!(
            apply_message(&mut store, r#"{"type":"ready"}"#).unwrap(),
            MenuCommand::Ready
        );
        apply_message(
            &mut store,
            r#"{"type":"config","key":"runtime.enabled","value":true}"#,
        )
        .unwrap();
        apply_message(
            &mut store,
            r#"{"type":"config","key":"runtime.amount","value":7}"#,
        )
        .unwrap();
        apply_message(
            &mut store,
            r#"{"type":"config","key":"runtime.tint","value":{"r":10,"g":20,"b":30,"a":40}}"#,
        )
        .unwrap();

        assert!(matches!(
            store.value("runtime.enabled"),
            Some(ConfigValue::Bool(true))
        ));
        assert!(matches!(
            store.value("runtime.amount"),
            Some(ConfigValue::Int(7))
        ));
        assert!(matches!(
            store.value("runtime.tint"),
            Some(ConfigValue::Color {
                r: 10,
                g: 20,
                b: 30,
                a: 40
            })
        ));
    }
}
