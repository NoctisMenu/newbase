use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

const DEFAULT_SCHEMA_PATH: &str = "app/config_schema.toml";
const DEFAULT_KEYS_OUTPUT_PATH: &str = "src/app/config_system/keys.rs";
const DEFAULT_MACROS_OUTPUT_PATH: &str = "src/app/config_system/macros.rs";

/// Regenerate generated config source files using the default project paths.
pub fn regenerate_generated_files() -> Result<()> {
    regenerate_generated_files_from_paths(
        DEFAULT_SCHEMA_PATH,
        DEFAULT_KEYS_OUTPUT_PATH,
        DEFAULT_MACROS_OUTPUT_PATH,
    )
}

/// Regenerate `keys.rs` and `macros.rs` from a schema path.
pub fn regenerate_generated_files_from_paths(
    schema_path: impl AsRef<Path>,
    keys_output_path: impl AsRef<Path>,
    macros_output_path: impl AsRef<Path>,
) -> Result<()> {
    let schema_content = fs::read_to_string(schema_path.as_ref()).with_context(|| {
        format!(
            "Failed to read schema file at '{}'",
            schema_path.as_ref().display()
        )
    })?;

    let schema: toml::Value = toml::from_str(&schema_content).context("Failed to parse schema TOML")?;

    let keys_output = generate_keys_source(&schema);
    let macros_output = generate_macros_source(&schema);

    fs::write(keys_output_path.as_ref(), keys_output).with_context(|| {
        format!(
            "Failed to write keys output file at '{}'",
            keys_output_path.as_ref().display()
        )
    })?;

    fs::write(macros_output_path.as_ref(), macros_output).with_context(|| {
        format!(
            "Failed to write macros output file at '{}'",
            macros_output_path.as_ref().display()
        )
    })?;

    Ok(())
}

fn generate_keys_source(schema: &toml::Value) -> String {
    let mut output = String::from("// Auto-generated config key constants\n");
    output.push_str("// DO NOT EDIT - Generated from config_schema.toml\n");
    output.push_str("//\n");
    output.push_str("// Regenerate via crate::app::config_system::codegen::regenerate_generated_files()\n");
    output.push_str("\n");

    if let Some(sections) = schema.get("sections").and_then(|s| s.as_table()) {
        for (section_name, section_data) in sections {
            output.push_str(&format!("\n// {} section\n", section_name));

            if let Some(fields) = section_data.get("fields").and_then(|f| f.as_table()) {
                for field_name in fields.keys() {
                    let const_name = field_name.to_uppercase();
                    let key_value = format!("{}.{}", section_name, field_name);

                    output.push_str(&format!(
                        "pub const {}: &str = \"{}\";\n",
                        const_name, key_value
                    ));
                }
            }
        }
    }

    output
}

fn generate_macros_source(schema: &toml::Value) -> String {
    let mut output = String::from("/// Auto-generated config access macros\n");
    output.push_str("/// DO NOT EDIT - Generated from config_schema.toml\n");
    output.push_str(
        "/// Regenerate via crate::app::config_system::codegen::regenerate_generated_files()\n",
    );

    output.push_str("/// Get a config value by field name string\n");
    output.push_str("///\n");
    output.push_str("/// # Examples\n");
    output.push_str("/// ```\n");
    output.push_str("/// let enabled = config!(self, \"aim_enabled\");\n");
    output.push_str("/// let fov = config!(self, \"aim_fov\");\n");
    output.push_str("/// ```\n");
    output.push_str("#[macro_export]\n");
    output.push_str("macro_rules! config {\n");

    if let Some(sections) = schema.get("sections").and_then(|s| s.as_table()) {
        for (_, section_data) in sections {
            if let Some(fields) = section_data.get("fields").and_then(|f| f.as_table()) {
                for (field_name, field_data) in fields {
                    if let Some(public) = field_data.get("public").and_then(|p| p.as_bool()) {
                        if !public {
                            continue;
                        }
                    }

                    let const_name = field_name.to_uppercase();
                    let field_type = field_data
                        .get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("string");
                    let default_val = field_data.get("default");

                    let (getter, default) = match field_type {
                        "bool" => {
                            let def = default_val.and_then(|v| v.as_bool()).unwrap_or(false);
                            ("get_bool", format!("{}", def))
                        }
                        "float" => {
                            let def = default_val
                                .and_then(|v| {
                                    v.as_float().or_else(|| v.as_integer().map(|i| i as f64))
                                })
                                .unwrap_or(0.0);
                            let def_str = if def.fract() == 0.0 {
                                format!("{:.1}", def)
                            } else {
                                format!("{}", def)
                            };
                            ("get_float", def_str)
                        }
                        "int" => {
                            let def = default_val.and_then(|v| v.as_integer()).unwrap_or(0);
                            ("get_int", format!("{}", def))
                        }
                        "color" => ("get_color", "egui::Color32::GREEN".to_string()),
                        "enum" => {
                            let def = default_val.and_then(|v| v.as_str()).unwrap_or("");
                            ("get_enum", format!("\"{}\".to_string()", def))
                        }
                        "string" => ("get_string", "String::new()".to_string()),
                        _ => ("get_string", "String::new()".to_string()),
                    };

                    output.push_str(&format!(
                        "    ($app:expr, \"{}\") => {{\n        $app.config_store.read().{}($crate::app::config_system::keys::{}).unwrap_or({})\n    }};\n",
                        field_name, getter, const_name, default
                    ));
                }
            }
        }
    }

    output.push_str("}\n\n");

    output.push_str("/// Get a config value directly from ConfigStore\n");
    output.push_str("///\n");
    output.push_str("/// # Examples\n");
    output.push_str("/// ```\n");
    output.push_str("/// let enabled = config_get!(config_store, \"aim_enabled\");\n");
    output.push_str("/// ```\n");
    output.push_str("#[macro_export]\n");
    output.push_str("macro_rules! config_get {\n");

    if let Some(sections) = schema.get("sections").and_then(|s| s.as_table()) {
        for (_, section_data) in sections {
            if let Some(fields) = section_data.get("fields").and_then(|f| f.as_table()) {
                for (field_name, field_data) in fields {
                    if let Some(public) = field_data.get("public").and_then(|p| p.as_bool()) {
                        if !public {
                            continue;
                        }
                    }

                    let const_name = field_name.to_uppercase();
                    let field_type = field_data
                        .get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("string");
                    let default_val = field_data.get("default");

                    let (getter, default) = match field_type {
                        "bool" => {
                            let def = default_val.and_then(|v| v.as_bool()).unwrap_or(false);
                            ("get_bool", format!("{}", def))
                        }
                        "float" => {
                            let def = default_val
                                .and_then(|v| {
                                    v.as_float().or_else(|| v.as_integer().map(|i| i as f64))
                                })
                                .unwrap_or(0.0);
                            let def_str = if def.fract() == 0.0 {
                                format!("{:.1}", def)
                            } else {
                                format!("{}", def)
                            };
                            ("get_float", def_str)
                        }
                        "int" => {
                            let def = default_val.and_then(|v| v.as_integer()).unwrap_or(0);
                            ("get_int", format!("{}", def))
                        }
                        "color" => ("get_color", "egui::Color32::GREEN".to_string()),
                        "enum" => {
                            let def = default_val.and_then(|v| v.as_str()).unwrap_or("");
                            ("get_enum", format!("\"{}\".to_string()", def))
                        }
                        "string" => ("get_string", "String::new()".to_string()),
                        _ => ("get_string", "String::new()".to_string()),
                    };

                    output.push_str(&format!(
                        "    ($store:expr, \"{}\") => {{\n        $store.{}($crate::app::config_system::keys::{}).unwrap_or({})\n    }};\n",
                        field_name, getter, const_name, default
                    ));
                }
            }
        }
    }

    output.push_str("}\n\n");

    output.push_str("/// Set a config value by field name string\n");
    output.push_str("///\n");
    output.push_str("/// # Examples\n");
    output.push_str("/// ```\n");
    output.push_str("/// config_set!(self, \"aim_enabled\", true);\n");
    output.push_str("/// config_set!(self, \"aim_fov\", 10.0);\n");
    output.push_str("/// ```\n");
    output.push_str("#[macro_export]\n");
    output.push_str("macro_rules! config_set {\n");

    if let Some(sections) = schema.get("sections").and_then(|s| s.as_table()) {
        for (_, section_data) in sections {
            if let Some(fields) = section_data.get("fields").and_then(|f| f.as_table()) {
                for (field_name, field_data) in fields {
                    if let Some(public) = field_data.get("public").and_then(|p| p.as_bool()) {
                        if !public {
                            continue;
                        }
                    }

                    let const_name = field_name.to_uppercase();
                    let field_type = field_data
                        .get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("string");

                    let setter = match field_type {
                        "bool" => "set_bool",
                        "float" => "set_float",
                        "int" => "set_int",
                        "color" => "set_color",
                        "enum" => "set_enum",
                        "string" => "set_string",
                        _ => "set_string",
                    };

                    output.push_str(&format!(
                        "    ($app:expr, \"{}\", $value:expr) => {{\n        $app.config_store.write().{}($crate::app::config_system::keys::{}, $value).ok()\n    }};\n",
                        field_name, setter, const_name
                    ));
                }
            }
        }
    }

    output.push_str("}\n");

    output
}
