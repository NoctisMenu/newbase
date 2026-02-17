fn main() {
    let app_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set");
    let app_dir = std::path::PathBuf::from(app_dir);

    let schema_path = app_dir.join("config_schema.toml");
    let keys_path = app_dir.join("src/config_system/keys.rs");
    let macros_path = app_dir.join("src/config_system/macros.rs");

    println!("cargo:rerun-if-changed={}", schema_path.display());
    println!("cargo:rerun-if-changed=build.rs");

    newbase::app::config_system::codegen::regenerate_generated_files_from_paths(
        &schema_path,
        &keys_path,
        &macros_path,
    )
    .expect("Failed to regenerate config keys/macros from schema");

    newbase::build_support::embed_windows_resources()
        .expect("Failed to compile Windows resources");
}
