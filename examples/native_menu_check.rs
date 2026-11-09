//! Explicit native smoke test: isolated window, no library/preferences or user media.
#[cfg(target_os = "macos")]
use photosuite_domain::commands;
#[cfg(target_os = "macos")]
#[path = "../src/desktop_menu.rs"]
mod desktop_menu;
#[cfg(target_os = "macos")]
#[path = "../src/muda_menu.rs"]
mod muda_menu;
#[cfg(target_os = "macos")]
fn main() -> eframe::Result {
    use desktop_menu::{Action, DesktopMenu, Entry};
    use eframe::egui;
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    use raw_window_handle::HasWindowHandle;
    struct Check {
        menu: muda_menu::MudaMenu,
        done: bool,
    }
    impl eframe::App for Check {
        fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
            if self.done {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            let mut entry = Entry {
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
            self.menu.sync(&[entry.clone(), open]).unwrap();
            let app = NSApplication::sharedApplication(MainThreadMarker::new().unwrap());
            let menu = app.mainMenu().unwrap();
            let labels: Vec<_> = (0..menu.numberOfItems())
                .map(|i| menu.itemAtIndex(i).unwrap().title().to_string())
                .collect();
            assert_eq!(
                labels,
                ["PhotoSuite", "File", "Edit", "View", "Window", "Help"]
            );
            let file = menu.itemAtIndex(1).unwrap().submenu().unwrap();
            assert_eq!(
                file.itemAtIndex(0).unwrap().keyEquivalent().to_string(),
                "o"
            );
            assert!(!file.itemAtIndex(0).unwrap().isEnabled());
            let view = menu.itemAtIndex(3).unwrap().submenu().unwrap();
            let square = view.itemAtIndex(1).unwrap();
            assert!(square.isEnabled());
            assert_eq!(square.state(), 0);
            view.performActionForItemAtIndex(1);
            assert_eq!(self.menu.drain(), vec![Action::ToggleSquare]);
            assert!(self.menu.drain().is_empty());
            entry.checked = Some(true);
            self.menu.sync(&[entry.clone()]).unwrap();
            assert_eq!(square.state(), 1);
            entry.enabled = false;
            self.menu.sync(&[entry]).unwrap();
            assert!(!square.isEnabled());
            println!(
                "PASS: native menu titles, real AppKit activation, single delivery, checked state and disabled state"
            );
            self.done = true;
            ctx.request_repaint();
        }
    }
    eframe::run_native(
        "PhotoSuite menu check",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([320., 120.]),
            event_loop_builder: Some(Box::new(|b| {
                use winit::platform::macos::EventLoopBuilderExtMacOS;
                b.with_default_menu(false);
            })),
            ..Default::default()
        },
        Box::new(|cc| {
            Ok(Box::new(Check {
                menu: muda_menu::MudaMenu::new(cc.egui_ctx.clone(), cc.window_handle()?.as_raw())
                    .map_err(std::io::Error::other)?,
                done: false,
            }))
        }),
    )
}
#[cfg(not(target_os = "macos"))]
fn main() {
    println!("Native AppKit menu test runs only on macOS");
}
