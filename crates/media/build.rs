fn main() {
    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");
    let includes = if let Some(dir) = std::env::var_os("FFMPEG_DIR") {
        vec![std::path::PathBuf::from(dir).join("include")]
    } else {
        pkg_config::Config::new()
            .cargo_metadata(false)
            .probe("libavutil")
            .expect("FFmpeg headers")
            .include_paths
    };
    let bindings = bindgen::Builder::default()
        .header_contents(
            "hdr_metadata.h",
            "#include <libavutil/mastering_display_metadata.h>",
        )
        .clang_args(includes.iter().map(|p| format!("-I{}", p.display())))
        .allowlist_type("AVMasteringDisplayMetadata|AVContentLightMetadata")
        .generate()
        .expect("FFmpeg HDR metadata ABI");
    bindings
        .write_to_file(
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("hdr_metadata.rs"),
        )
        .unwrap();
}
