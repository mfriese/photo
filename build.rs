fn main() {
    println!("cargo:rerun-if-changed=src/theme.rs");
    println!("cargo:rerun-if-changed=native/theme.h");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let theme = std::fs::read_to_string("src/theme.rs").unwrap();
    let mut header = String::from("// Generated from src/theme.rs; do not edit.\n");
    for line in theme.lines() {
        if let Some((name, value)) = line
            .strip_prefix("pub const ")
            .and_then(|s| s.split_once(": Color32 = Color32::from_rgb("))
        {
            let rgb = value.strip_suffix(");").expect("theme RGB literal");
            header.push_str(&format!("#define PS_THEME_{name} PSThemeColor({rgb})\n"));
        }
    }
    std::fs::write(out.join("theme_palette.h"), header).unwrap();
    for source in [
        "native/audio.m",
        "native/comment_recorder.m",
        "native/comment_edit.m",
        "native/comment_edit.h",
        "native/audio_transport.m",
        "native/audio_transport.h",
        "native/audio_timeline_view.m",
        "native/audio_timeline_view.h",
        "native/context_photos.m",
        "native/context_photo_view.m",
        "native/context_photo_view.h",
        "native/video.m",
        "native/player.m",
        "native/filesystem.m",
        "native/photosuite.h",
    ] {
        println!("cargo:rerun-if-changed={source}");
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok("x86_64") => "x86_64",
        _ => panic!("unsupported macOS architecture"),
    };
    let info = out.join("recording-info.plist");
    std::fs::write(&info,r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>de.photosuite.PhotoSuite</string><key>CFBundleName</key><string>PhotoSuite</string><key>NSMicrophoneUsageDescription</key><string>PhotoSuite nimmt auf deinen Wunsch Audiokommentare zu Fotos auf.</string></dict></plist>"#).unwrap();
    println!(
        "cargo:rustc-link-arg-bin=photosuite=-Wl,-sectcreate,__TEXT,__info_plist,{}",
        info.display()
    );
    let mut objects = Vec::new();
    for source in [
        "video",
        "player",
        "filesystem",
        "audio",
        "audio_transport",
        "comment_recorder",
        "comment_edit",
        "audio_timeline_view",
        "context_photos",
        "context_photo_view",
    ] {
        let object = out.join(format!("{source}.o"));
        assert!(
            std::process::Command::new("clang")
                .args([
                    "-fobjc-arc",
                    "-Wno-deprecated-declarations",
                    "-arch",
                    arch,
                    "-c"
                ])
                .arg("-I")
                .arg(&out)
                .arg(format!("native/{source}.m"))
                .arg("-o")
                .arg(&object)
                .status()
                .expect("Apple clang required")
                .success(),
            "native {source} adapter failed"
        );
        objects.push(object);
    }
    assert!(
        std::process::Command::new("ar")
            .arg("crs")
            .arg(out.join("libphotosuite_video.a"))
            .args(&objects)
            .status()
            .unwrap()
            .success()
    );
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=photosuite_video");
    for framework in [
        "AVFoundation",
        "AVKit",
        "AppKit",
        "Foundation",
        "CoreMedia",
        "CoreGraphics",
        "ImageIO",
    ] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}
