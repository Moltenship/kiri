fn main() {
    #[cfg(windows)]
    windows_resources();
}

/// Kiri.exe's icon, which GPUI also gives Kiri's windows, and the name and version Windows shows.
#[cfg(windows)]
fn windows_resources() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=windows/Kiri.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let numbers: Vec<&str> = version.split(['.', '-']).take(3).collect();
    let numbers = format!("{},0", numbers.join(","));
    let icon = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join(r"windows\Kiri.ico");
    let icon = icon.to_string_lossy().replace('\\', r"\\");
    let rc = format!(
        r#"1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {numbers}
PRODUCTVERSION {numbers}
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "Kiri"
      VALUE "FileDescription", "Kiri"
      VALUE "FileVersion", "{version}"
      VALUE "OriginalFilename", "Kiri.exe"
      VALUE "ProductName", "Kiri"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    );
    let path = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("kiri.rc");
    std::fs::write(&path, rc).unwrap();
    embed_resource::compile(&path, embed_resource::NONE).manifest_optional().unwrap();
}
