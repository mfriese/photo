//! Embedded application branding, shared by the viewport and the macOS Dock.
const PNG: &[u8] = include_bytes!("../assets/app-icon/photosuite.png");
pub fn window_icon() -> Result<eframe::egui::IconData, String> {
    let image = image::load_from_memory(PNG)
        .map_err(|error| error.to_string())?
        .into_rgba8();
    Ok(eframe::egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    })
}
#[cfg(target_os = "macos")]
pub fn install_dock_icon() -> Result<(), String> {
    unsafe extern "C" {
        fn photosuite_set_application_icon(bytes: *const u8, length: usize) -> std::ffi::c_int;
    }
    // Called by eframe's creation callback on the main thread, after NSApplication exists.
    if unsafe { photosuite_set_application_icon(PNG.as_ptr(), PNG.len()) } == 1 {
        Ok(())
    } else {
        Err("PhotoSuite-Symbol konnte nicht im Dock gesetzt werden".into())
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn embedded_icon_is_square_valid_and_contains_visible_pixels() {
        let icon = super::window_icon().unwrap();
        assert_eq!(icon.width, icon.height);
        assert!(icon.width >= 256);
        assert_eq!(
            icon.rgba.len(),
            icon.width as usize * icon.height as usize * 4
        );
        assert!(
            icon.rgba
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] > 0 && pixel[..3].iter().any(|&channel| channel > 32))
        );
    }
}
