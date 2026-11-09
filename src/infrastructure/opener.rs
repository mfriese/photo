//! Explicit system-program opening adapter.
use std::path::Path;
pub struct SystemMediaOpener;
impl crate::ports::MediaOpener for SystemMediaOpener {
    fn open(&self, path: &Path) -> Result<(), String> {
        open::that(path).map_err(|e| e.to_string())
    }
}
