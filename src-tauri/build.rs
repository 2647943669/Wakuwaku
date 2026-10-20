fn main() {
    // The source tree she is built in (data.rs: an exe run from its target
    // folder keeps its data in the tree), unless this is a release asset
    // (scripts/build-release.ps1 sets WAKUWAKU_ASSET=1): one runs only from
    // where it was put, and carries no path of the machine that built it.
    println!("cargo:rerun-if-env-changed=WAKUWAKU_ASSET");
    if std::env::var_os("WAKUWAKU_ASSET").is_none() {
        let tree = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
        println!("cargo:rustc-env=WAKUWAKU_SOURCE_TREE={tree}");
    }
    tauri_build::build()
}
