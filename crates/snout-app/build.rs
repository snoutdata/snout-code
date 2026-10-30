//! On Windows: the program's icon and version, as resources of `snout-code.exe` (Explorer, the
//! taskbar and the window take the icon from it; the manifest is GPUI's).

fn main() {
    println!("cargo:rerun-if-changed=windows/snout-code.rc");
    println!("cargo:rerun-if-changed=../../assets/icon/snout.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
        let parts: Vec<&str> = version.split(['.', '-']).take(3).collect();
        let numeric = format!("{},{},{},0", parts[0], parts.get(1).unwrap_or(&"0"), parts.get(2).unwrap_or(&"0"));
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default().replace('\\', "/");
        let icon = format!("{dir}/../../assets/icon/snout.ico");
        let defines = [
            format!("SNOUT_VERSION=\"{version}\""),
            format!("SNOUT_VERSION_NUMBERS={numeric}"),
            format!("SNOUT_ICON=\"{icon}\""),
        ];
        embed_resource::compile_for("windows/snout-code.rc", ["snout-code"], &defines).manifest_required().unwrap();
    }
}
