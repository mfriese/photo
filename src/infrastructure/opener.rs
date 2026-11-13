//! Explicit system-program opening adapter.
use std::path::Path;
pub struct SystemMediaOpener;
impl crate::ports::MediaOpener for SystemMediaOpener {
    fn reveal(&self, path: &Path) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            let output = reveal_command(path)?.output().map_err(|e| e.to_string())?;
            if output.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "Finder konnte die Datei nicht anzeigen: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ))
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = path;
            Err("Im Finder anzeigen ist nur auf macOS verfügbar".into())
        }
    }
    fn open(&self, path: &Path) -> Result<(), String> {
        open::that(path).map_err(|e| e.to_string())
    }
}

#[cfg(target_os = "macos")]
fn reveal_command(path: &Path) -> Result<std::process::Command, String> {
    if !path.is_absolute() {
        return Err("Finder benötigt einen absoluten Dateipfad".into());
    }
    let mut command = std::process::Command::new("/usr/bin/open");
    command.arg("-R").arg(path);
    Ok(command)
}
#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn finder_selects_a_literal_path_without_launching_any_user_application() {
        let path = Path::new("/Fotos/-a $(example)/Bild mit Leerzeichen.jpg");
        let command = reveal_command(path).unwrap();
        assert_eq!(command.get_program(), "/usr/bin/open");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            vec![std::ffi::OsStr::new("-R"), path.as_os_str()]
        );
        assert!(reveal_command(Path::new("-a")).is_err());
    }
}
