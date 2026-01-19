use std::fs;
use std::path::Path;

fn generate_config_keys() {
    // Tell cargo to rerun if config_schema.toml changes
    println!("cargo:rerun-if-changed=config_schema.toml");

    // Read and parse the schema TOML
    let schema_content =
        fs::read_to_string("config_schema.toml").expect("Failed to read config_schema.toml");

    let schema: toml::Value =
        toml::from_str(&schema_content).expect("Failed to parse config_schema.toml");

    // Generate the keys code
    let mut output = String::from("// Auto-generated config key constants\n");
    output.push_str("// DO NOT EDIT - Generated from config_schema.toml by build.rs\n");
    output.push_str("//\n");
    output.push_str(
        "// This file is automatically regenerated whenever you modify config_schema.toml\n",
    );
    output.push_str("// To add a new config key:\n");
    output.push_str("//   1. Add the field to config_schema.toml\n");
    output.push_str("//   2. Run `cargo build` - the constant will be auto-generated here\n");
    output.push_str("//   3. Use it like: config_store.get_bool(keys::YOUR_NEW_KEY)\n");
    output.push_str("\n");

    if let Some(sections) = schema.get("sections").and_then(|s| s.as_table()) {
        for (section_name, section_data) in sections {
            output.push_str(&format!("\n// {} section\n", section_name));

            if let Some(fields) = section_data.get("fields").and_then(|f| f.as_table()) {
                for field_name in fields.keys() {
                    // Convert field_name to SCREAMING_SNAKE_CASE for the constant name
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

    // Write to src/app/config_system/keys.rs directly
    let dest_path = Path::new("src/app/config_system/keys.rs");
    fs::write(&dest_path, output).expect("Failed to write keys.rs");
}

fn generate_config_macros() {
    // Tell cargo to rerun if config_schema.toml changes
    println!("cargo:rerun-if-changed=config_schema.toml");

    // Read and parse the schema TOML
    let schema_content =
        fs::read_to_string("config_schema.toml").expect("Failed to read config_schema.toml");

    let schema: toml::Value =
        toml::from_str(&schema_content).expect("Failed to parse config_schema.toml");

    let mut output = String::from("/// Auto-generated config access macros\n");
    output.push_str("/// DO NOT EDIT - Generated from config_schema.toml by build.rs\n");

    // Generate config! macro (getter through App)
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
                    // Only generate for public fields
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

                    // Determine getter method and default value
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
                            // Ensure float literals always have decimal point
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

    // Generate config_get! macro (getter through ConfigStore)
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
                            // Ensure float literals always have decimal point
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

    // Generate config_set! macro (setter through App)
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

    // Write to src/app/config_system/macros.rs
    let dest_path = Path::new("src/app/config_system/macros.rs");
    fs::write(&dest_path, output).expect("Failed to write macros.rs");
}

fn main() {
    use std::io::Write;

    // Generate config keys from schema
    generate_config_keys();

    // Generate config macros from schema
    generate_config_macros();

    // Windows resource compilation
    let mut res = winresource::WindowsResource::new();
    res.set_manifest(
        r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
<security>
    <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
    </requestedPrivileges>
</security>
</trustInfo>
</assembly>
"#,
    );
    res.set_icon("./resources/icon.ico");
    match res.compile() {
        Err(error) => {
            write!(std::io::stderr(), "{}", error).unwrap();
            std::process::exit(1);
        }
        Ok(_) => {}
    }
}
