//! Explicit native smoke test: isolated AppKit menu, no library/preferences or user media.
#[cfg(target_os = "macos")]
use photosuite_domain::commands;
#[cfg(target_os = "macos")]
#[path = "../src/desktop_menu.rs"]
mod desktop_menu;
#[cfg(target_os = "macos")]
#[path = "../src/muda_menu.rs"]
mod muda_menu;

#[cfg(target_os = "macos")]
fn main() {
    use desktop_menu::{Action, DesktopMenu, Entry};
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    use raw_window_handle::{AppKitWindowHandle, RawWindowHandle};
    use std::ptr::NonNull;

    let marker = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(marker);
    // Muda ignores the window handle on macOS; its constructor keeps the same
    // signature on platforms where the menu is attached to a native window.
    let handle = RawWindowHandle::AppKit(AppKitWindowHandle::new(NonNull::dangling()));
    let mut menu = muda_menu::MudaMenu::new(eframe::egui::Context::default(), handle)
        .expect("native menu initialization");

    let mut square = Entry {
        action: Action::ToggleSquare,
        enabled: true,
        checked: Some(false),
        shortcut: None,
    };
    let open = Entry {
        action: Action::OpenLibrary,
        enabled: false,
        checked: None,
        shortcut: Some(photosuite_application::preferences::Shortcut {
            key: "O".into(),
            alt: false,
            ctrl: false,
            shift: false,
            command: true,
            mac_cmd: false,
        }),
    };
    let mut duplicates = Entry {
        action: Action::FindDuplicates,
        enabled: true,
        checked: None,
        shortcut: None,
    };
    menu.sync(&[square.clone(), open, duplicates.clone()])
        .expect("sync initial entries");

    let native = app.mainMenu().expect("AppKit main menu");
    let labels: Vec<_> = (0..native.numberOfItems())
        .map(|index| native.itemAtIndex(index).unwrap().title().to_string())
        .collect();
    assert_eq!(
        labels,
        ["PhotoSuite", "File", "Edit", "View", "Window", "Help"]
    );

    let file = native.itemAtIndex(1).unwrap().submenu().unwrap();
    assert_eq!(
        file.itemAtIndex(0).unwrap().keyEquivalent().to_string(),
        "o"
    );
    assert!(!file.itemAtIndex(0).unwrap().isEnabled());
    let duplicate_index = (0..file.numberOfItems())
        .find(|&index| file.itemAtIndex(index).unwrap().title().to_string() == "Find Duplicates…")
        .expect("separate File menu action");
    assert!(file.itemAtIndex(duplicate_index).unwrap().isEnabled());
    file.performActionForItemAtIndex(duplicate_index);
    assert_eq!(menu.drain(), vec![Action::FindDuplicates]);
    duplicates.enabled = false;
    menu.sync(&[duplicates]).expect("disable duplicates action");
    assert!(!file.itemAtIndex(duplicate_index).unwrap().isEnabled());

    let view_menu = native.itemAtIndex(3).unwrap().submenu().unwrap();
    let square_item = view_menu.itemAtIndex(1).unwrap();
    assert!(square_item.isEnabled());
    assert_eq!(square_item.state(), 0);
    view_menu.performActionForItemAtIndex(1);
    assert_eq!(menu.drain(), vec![Action::ToggleSquare]);
    assert!(menu.drain().is_empty());
    square.checked = Some(true);
    menu.sync(&[square.clone()]).expect("check square action");
    assert_eq!(square_item.state(), 1);
    square.enabled = false;
    menu.sync(&[square]).expect("disable square action");
    assert!(!square_item.isEnabled());

    println!("PASS: native menu titles, action delivery, checked state and disabled state");
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("Native AppKit menu test runs only on macOS");
}
