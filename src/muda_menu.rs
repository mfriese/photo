//! Main-thread-only native menu adapter. muda is the only menu implementation.
use crate::desktop_menu::{Action, DesktopMenu, Entry};
use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use photosuite_application::preferences::Shortcut;
use std::sync::mpsc::{self, Receiver};

enum Item {
    Normal(MenuItem),
    Check(CheckMenuItem),
}
impl Item {
    fn id(&self) -> muda::MenuId {
        match self {
            Self::Normal(i) => i.id().clone(),
            Self::Check(i) => i.id().clone(),
        }
    }
    fn update(&self, entry: &Entry) -> Result<(), String> {
        let accelerator = entry.shortcut.as_ref().map(accelerator).transpose()?;
        match self {
            Self::Normal(i) => {
                i.set_enabled(entry.enabled);
                i.set_accelerator(accelerator).map_err(|e| e.to_string())?;
            }
            Self::Check(i) => {
                i.set_enabled(entry.enabled);
                i.set_checked(entry.checked.unwrap_or(false));
                i.set_accelerator(accelerator).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}
pub struct MudaMenu {
    menu: Menu,
    #[cfg(target_os = "windows")]
    hwnd: isize,
    items: Vec<(Action, Item)>,
    events: Receiver<muda::MenuId>,
    previous: Vec<Entry>,
}
impl MudaMenu {
    pub fn new(
        context: eframe::egui::Context,
        handle: raw_window_handle::RawWindowHandle,
    ) -> Result<Self, String> {
        let menu = Menu::new();
        let mut items = Vec::new();
        let app = Submenu::new("PhotoSuite", true);
        app.append(&PredefinedMenuItem::about(
            Some("About PhotoSuite"),
            Some(muda::AboutMetadata {
                name: Some("PhotoSuite".into()),
                version: Some(env!("CARGO_PKG_VERSION").into()),
                ..Default::default()
            }),
        ))
        .map_err(|e| e.to_string())?;
        app.append(&PredefinedMenuItem::separator())
            .map_err(|e| e.to_string())?;
        add(&app, &mut items, Action::Settings, "Settings…", false)?;
        #[cfg(target_os = "macos")]
        app.append_items(&[
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::services(Some("Services")),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(Some("Hide PhotoSuite")),
            &PredefinedMenuItem::hide_others(Some("Hide Others")),
            &PredefinedMenuItem::show_all(Some("Show All")),
            &PredefinedMenuItem::separator(),
        ])
        .map_err(|e| e.to_string())?;
        add(&app, &mut items, Action::Quit, "Quit PhotoSuite", false)?;
        menu.append(&app).map_err(|e| e.to_string())?;
        let file = Submenu::new("File", true);
        add(
            &file,
            &mut items,
            Action::OpenLibrary,
            "Open Library…",
            false,
        )?;
        add(&file, &mut items, Action::Refresh, "Refresh Library", false)?;
        file.append(&PredefinedMenuItem::separator())
            .map_err(|e| e.to_string())?;
        add(&file, &mut items, Action::Import, "Import…", false)?;
        file.append(&PredefinedMenuItem::separator())
            .map_err(|e| e.to_string())?;
        file.append(&PredefinedMenuItem::close_window(Some("Close Window")))
            .map_err(|e| e.to_string())?;
        menu.append(&file).map_err(|e| e.to_string())?;
        let edit = Submenu::new("Edit", true);
        for (action, label) in [
            (Action::Undo, "Undo"),
            (Action::Redo, "Redo"),
            (Action::Cut, "Cut"),
            (Action::Copy, "Copy"),
            (Action::Paste, "Paste"),
            (Action::SelectAll, "Select All"),
        ] {
            add(&edit, &mut items, action, label, false)?;
        }
        edit.append(&PredefinedMenuItem::separator())
            .map_err(|e| e.to_string())?;
        for command in crate::commands::ALL {
            add(
                &edit,
                &mut items,
                Action::Edit(command),
                match command {
                    crate::commands::Command::RotateLeft => "Rotate Left",
                    crate::commands::Command::RotateRight => "Rotate Right",
                    crate::commands::Command::ResetRotation => "Reset to EXIF Orientation",
                    crate::commands::Command::PlayVideo => "Play Video",
                },
                false,
            )?;
        }
        edit.append(&PredefinedMenuItem::separator())
            .map_err(|e| e.to_string())?;
        add(&edit, &mut items, Action::Tags, "Tags and Albums…", false)?;
        add(
            &edit,
            &mut items,
            Action::Shortcuts,
            "Keyboard Shortcuts…",
            false,
        )?;
        add(&edit, &mut items, Action::Find, "Find…", false)?;
        menu.append(&edit).map_err(|e| e.to_string())?;
        let view = Submenu::new("View", true);
        add(
            &view,
            &mut items,
            Action::ToggleLibrary,
            "Show Library Sidebar",
            true,
        )?;
        add(
            &view,
            &mut items,
            Action::ToggleSquare,
            "Square Thumbnails",
            true,
        )?;
        add(&view, &mut items, Action::ToggleVideos, "Videos Only", true)?;
        let sizes = Submenu::new("Thumbnail Size", true);
        for (index, label) in ["Very Small", "Small", "Medium", "Large", "Very Large"]
            .into_iter()
            .enumerate()
        {
            add(&sizes, &mut items, Action::PreviewSize(index), label, true)?;
        }
        view.append(&sizes).map_err(|e| e.to_string())?;
        menu.append(&view).map_err(|e| e.to_string())?;
        let window = Submenu::new("Window", true);
        window
            .append_items(&[
                &PredefinedMenuItem::minimize(Some("Minimize")),
                &PredefinedMenuItem::maximize(Some("Zoom")),
                &PredefinedMenuItem::fullscreen(Some("Enter Full Screen")),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::bring_all_to_front(Some("Bring All to Front")),
            ])
            .map_err(|e| e.to_string())?;
        menu.append(&window).map_err(|e| e.to_string())?;
        let help = Submenu::new("Help", true);
        add(&help, &mut items, Action::Help, "PhotoSuite Help", false)?;
        menu.append(&help).map_err(|e| e.to_string())?;
        #[cfg(target_os = "macos")]
        {
            let _ = handle;
            menu.init_for_nsapp();
            window.set_as_windows_menu_for_nsapp();
            help.set_as_help_menu_for_nsapp();
        }
        #[cfg(target_os = "windows")]
        {
            let raw_window_handle::RawWindowHandle::Win32(handle) = handle else {
                return Err("Native menu requires a Win32 window".into());
            };
            // SAFETY: eframe owns this live window; menu is attached/detached on its UI thread.
            unsafe {
                menu.init_for_hwnd(handle.hwnd.get())
                    .map_err(|e| e.to_string())?;
            }
            WINDOWS_ACCEL.store(menu.haccel() as usize, std::sync::atomic::Ordering::Relaxed);
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = handle;
            return Err("Native muda menus need a GTK host on this platform; the current eframe host is not GTK. No fallback menu is installed.".into());
        }
        #[cfg(target_os = "windows")]
        let hwnd = match handle {
            raw_window_handle::RawWindowHandle::Win32(h) => h.hwnd.get(),
            _ => unreachable!(),
        };
        let (sender, events) = mpsc::sync_channel(64);
        let playback_id = items
            .iter()
            .find(|(action, _)| *action == Action::Edit(crate::commands::Command::PlayVideo))
            .map(|(_, item)| item.id());
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if playback_id.as_ref() == Some(&event.id) && repeated_key_event() {
                return;
            }

            let _ = sender.try_send(event.id);
            context.request_repaint();
        }));
        Ok(Self {
            menu,
            #[cfg(target_os = "windows")]
            hwnd,
            items,
            events,
            previous: Vec::new(),
        })
    }
}
fn add(
    parent: &Submenu,
    items: &mut Vec<(Action, Item)>,
    action: Action,
    label: &str,
    check: bool,
) -> Result<(), String> {
    let item = if check {
        let i = CheckMenuItem::new(label, false, false, None);
        parent.append(&i).map_err(|e| e.to_string())?;
        Item::Check(i)
    } else {
        let i = MenuItem::new(label, false, None);
        parent.append(&i).map_err(|e| e.to_string())?;
        Item::Normal(i)
    };
    items.push((action, item));
    Ok(())
}
impl DesktopMenu for MudaMenu {
    fn sync(&mut self, entries: &[Entry]) -> Result<(), String> {
        if self.previous == entries {
            return Ok(());
        }
        for entry in entries {
            if let Some((_, item)) = self
                .items
                .iter()
                .find(|(action, _)| *action == entry.action)
            {
                item.update(entry)?;
            }
        }
        #[cfg(target_os = "windows")]
        WINDOWS_ACCEL.store(
            self.menu.haccel() as usize,
            std::sync::atomic::Ordering::Relaxed,
        );
        self.previous = entries.to_vec();
        Ok(())
    }
    fn drain(&mut self) -> Vec<Action> {
        self.events
            .try_iter()
            .take(64)
            .filter_map(|id| {
                self.items
                    .iter()
                    .find(|(_, item)| item.id() == id)
                    .map(|(action, _)| *action)
            })
            .collect()
    }
}
impl Drop for MudaMenu {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        self.menu.remove_for_nsapp();
        #[cfg(target_os = "windows")]
        {
            WINDOWS_ACCEL.store(0, std::sync::atomic::Ordering::Relaxed);
            // SAFETY: the main-thread app is dropped before its eframe window.
            let _ = unsafe { self.menu.remove_for_hwnd(self.hwnd) };
        }
        // The process-global muda callback contains no native objects or application references.
    }
}
fn accelerator(shortcut: &Shortcut) -> Result<Accelerator, String> {
    let mut mods = Modifiers::empty();
    if shortcut.alt {
        mods |= Modifiers::ALT;
    }
    if shortcut.shift {
        mods |= Modifiers::SHIFT;
    }
    if shortcut.ctrl {
        mods |= Modifiers::CONTROL;
    }
    if shortcut.command || shortcut.mac_cmd {
        #[cfg(target_os = "macos")]
        {
            mods |= Modifiers::META;
        }
        #[cfg(not(target_os = "macos"))]
        {
            mods |= Modifiers::CONTROL;
        }
    }
    let name = match shortcut.key.as_str() {
        key if key.len() == 1 && key.as_bytes()[0].is_ascii_digit() => {
            format!("Digit{}", shortcut.key)
        }
        key if key.len() == 1 && key.as_bytes()[0].is_ascii_alphabetic() => {
            format!("Key{}", key.to_uppercase())
        }
        "ArrowUp" | "Up" => "ArrowUp".into(),
        "ArrowDown" | "Down" => "ArrowDown".into(),
        "ArrowLeft" | "Left" => "ArrowLeft".into(),
        "ArrowRight" | "Right" => "ArrowRight".into(),
        "Colon" => {
            mods |= Modifiers::SHIFT;
            "Semicolon".into()
        }
        "Plus" => {
            mods |= Modifiers::SHIFT;
            "Equal".into()
        }
        "Equals" => "Equal".into(),
        "Pipe" => {
            mods |= Modifiers::SHIFT;
            "Backslash".into()
        }
        "Questionmark" => {
            mods |= Modifiers::SHIFT;
            "Slash".into()
        }
        "Exclamationmark" => {
            mods |= Modifiers::SHIFT;
            "Digit1".into()
        }
        "OpenBracket" => "BracketLeft".into(),
        "CloseBracket" => "BracketRight".into(),
        "OpenCurlyBracket" => {
            mods |= Modifiers::SHIFT;
            "BracketLeft".into()
        }
        "CloseCurlyBracket" => {
            mods |= Modifiers::SHIFT;
            "BracketRight".into()
        }
        "Backtick" => "Backquote".into(),
        "," => "Comma".into(),
        "." => "Period".into(),
        "-" => "Minus".into(),
        "=" | "+" => "Equal".into(),
        "/" | "?" => "Slash".into(),
        "\\" | "|" => "Backslash".into(),
        ";" | ":" => "Semicolon".into(),
        "'" => "Quote".into(),
        "`" => "Backquote".into(),
        "[" | "{" => "BracketLeft".into(),
        "]" | "}" => "BracketRight".into(),
        key => key.into(),
    };
    let code = name
        .parse::<Code>()
        .map_err(|_| format!("Unsupported native shortcut: {}", shortcut.key))?;
    Ok(Accelerator::new(mods, code))
}
#[cfg(target_os = "windows")]
static WINDOWS_ACCEL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(target_os = "windows")]
pub fn windows_message_hook(message: *const std::ffi::c_void) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MSG, TranslateAcceleratorW};
    let accelerator = WINDOWS_ACCEL.load(std::sync::atomic::Ordering::Relaxed);
    if accelerator == 0 {
        return false;
    }
    // SAFETY: winit passes a live MSG on its event-loop thread.
    unsafe {
        let msg = message as *const MSG;
        TranslateAcceleratorW((*msg).hwnd, accelerator as _, msg) == 1
    }
}

#[cfg(target_os = "macos")]
fn repeated_key_event() -> bool {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSEventType};
    MainThreadMarker::new()
        .and_then(|main| NSApplication::sharedApplication(main).currentEvent())
        .is_some_and(|event| event.r#type() == NSEventType::KeyDown && event.isARepeat())
}
#[cfg(not(target_os = "macos"))]
fn repeated_key_event() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_assignable_egui_key_can_be_projected_into_a_native_accelerator() {
        for key in eframe::egui::Key::ALL {
            let shortcut = Shortcut {
                key: key.name().into(),
                alt: true,
                ctrl: false,
                shift: false,
                command: false,
                mac_cmd: false,
            };
            assert!(
                accelerator(&shortcut).is_ok(),
                "native key mapping missing: {key:?}"
            );
        }
    }
    #[test]
    fn command_shortcut_uses_the_platform_primary_modifier() {
        let shortcut = Shortcut {
            key: "O".into(),
            alt: false,
            ctrl: false,
            shift: false,
            command: true,
            mac_cmd: false,
        };
        let native = accelerator(&shortcut).unwrap();
        #[cfg(target_os = "macos")]
        assert!(native.matches(Modifiers::META, Code::KeyO));
        #[cfg(target_os = "windows")]
        assert!(native.matches(Modifiers::CONTROL, Code::KeyO));
    }
}
