fn main() {
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=../src/Resources/icon.ico");

    embed_resource::compile("app.rc", embed_resource::NONE)
        .manifest_required()
        .unwrap();

    // The MSVC linker embeds the application manifest through these flags.
    // `embed-resource` compiles the icon/version resources but does not add the
    // manifest itself.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let manifest = std::fs::canonicalize("app.manifest").unwrap();
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
}
