//! Presentation boundary for native menus. No native handles enter the application core.
use crate::commands::Command;
use photosuite_application::preferences::Shortcut;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    OpenLibrary,
    Refresh,
    Import,
    Settings,
    Shortcuts,
    Tags,
    AudioWindow,
    StartViewAudio,
    ToggleLibrary,
    ToggleSquare,
    ToggleVideos,
    PreviewSize(usize),
    Edit(Command),
    Find,
    Help,
    Quit,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub action: Action,
    pub enabled: bool,
    pub checked: Option<bool>,
    pub shortcut: Option<Shortcut>,
}
pub trait DesktopMenu {
    fn sync(&mut self, entries: &[Entry]) -> Result<(), String>;
    fn drain(&mut self) -> Vec<Action>;
}
