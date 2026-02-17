use std::{env, fs, io, path::PathBuf};

pub const REQUIRE_ADMIN_MANIFEST: &str = r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
<security>
    <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
    </requestedPrivileges>
</security>
</trustInfo>
</assembly>
"#;

const ICON_ICO_BYTES: &[u8] = include_bytes!("../resources/icon.ico");
const ICON_SOURCE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/resources/icon.ico");

/// Compile Windows resources for a binary from its `build.rs`.
///
/// Example:
/// ```ignore
/// fn main() {
///     newbase::build_support::embed_windows_resources()
///         .expect("Failed to compile Windows resources");
/// }
/// ```
pub fn embed_windows_resources() -> io::Result<()> {
    println!("cargo:rerun-if-changed={}", ICON_SOURCE_PATH);

    let out_dir = env::var("OUT_DIR")
        .map(PathBuf::from)
        .map_err(|e| io::Error::other(format!("OUT_DIR is not set: {}", e)))?;
    let embedded_icon_path = out_dir.join("newbase-embedded-icon.ico");
    fs::write(&embedded_icon_path, ICON_ICO_BYTES)?;

    let icon_path = embedded_icon_path
        .to_str()
        .ok_or_else(|| io::Error::other("icon path is not valid UTF-8"))?;

    let mut res = winresource::WindowsResource::new();
    res.set_manifest(REQUIRE_ADMIN_MANIFEST);
    res.set_icon(icon_path);
    res.compile()
}
