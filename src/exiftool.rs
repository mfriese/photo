//! Desktop helper location; bundled apps never depend on a shell's Homebrew PATH.
use std::{
    path::{Path, PathBuf},
    process::Command,
};
#[derive(Clone)]
pub struct ExifTool {
    script: PathBuf,
    bundled: bool,
}
impl ExifTool {
    pub fn for_executable(executable: &Path) -> Self {
        if let Some(contents) = executable
            .parent()
            .filter(|p| p.file_name().is_some_and(|n| n == "MacOS"))
            .and_then(Path::parent)
            .filter(|p| p.file_name().is_some_and(|n| n == "Contents"))
        {
            Self {
                script: contents.join("Resources/exiftool/exiftool"),
                bundled: true,
            }
        } else {
            Self::development()
        }
    }
    pub fn development() -> Self {
        Self {
            script: "exiftool".into(),
            bundled: false,
        }
    }
    pub fn command(&self) -> Command {
        let mut command = if self.bundled {
            let mut command = Command::new("/usr/bin/perl");
            command.arg(&self.script);
            command
        } else {
            Command::new(&self.script)
        };
        command.args(["-config", ""]);
        command
    }
    pub fn check(&self) -> Result<String, String> {
        let output = self
            .command()
            .arg("-ver")
            .output()
            .map_err(|e| format!("ExifTool nicht verfügbar: {e}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packaged_helpers_use_bundle_relative_paths() {
        let tool = ExifTool::for_executable(Path::new(
            "/Applications/PhotoSuite.app/Contents/MacOS/photosuite",
        ));
        let command = tool.command();
        assert_eq!(command.get_program(), "/usr/bin/perl");
        assert_eq!(
            command.get_args().next().unwrap(),
            "/Applications/PhotoSuite.app/Contents/Resources/exiftool/exiftool"
        );
    }
    #[test]
    fn cli_keeps_development_lookup() {
        assert_eq!(
            ExifTool::for_executable(Path::new("/tmp/photosuite"))
                .command()
                .get_program(),
            "exiftool"
        );
    }
}
