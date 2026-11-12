fn main() {
    for source in [
        "native/audio.m",
        "native/comment_recorder.m",
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
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
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
