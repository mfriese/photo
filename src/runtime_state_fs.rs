//! Typed runtime YAML; no arbitrary key/value bag and no database settings.
use photosuite_application::{
    runtime::{RuntimeState, TranscriptionState},
    transcription::queue::Mode,
};
use std::{fs, io::Read, path::Path};
const FILE: &str = "yaml/runtime.yaml";
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    transcription: Transcription,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Transcription {
    mode: ModeDto,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum ModeDto {
    Running,
    Paused,
    Stopped,
}
pub fn read(root: &Path) -> Result<(RuntimeState, Option<Vec<u8>>), String> {
    let path = crate::metadata_fs::library_path(root, FILE, false)?;
    if !crate::metadata_fs::check_file(&path)? {
        return Ok((RuntimeState::default(), None));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(16385)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 16384 {
        return Err("runtime.yaml überschreitet 16 KiB".into());
    }
    let doc: Document = serde_saphyr::from_slice(&bytes)
        .map_err(|e| format!("Betriebszustand bleibt unverändert: {e}"))?;
    if doc.version != 1 {
        return Err("Unbekannte Version von runtime.yaml".into());
    }
    Ok((
        RuntimeState {
            transcription: TranscriptionState {
                mode: match doc.transcription.mode {
                    ModeDto::Running => Mode::Running,
                    ModeDto::Paused => Mode::Paused,
                    ModeDto::Stopped => Mode::Stopped,
                },
            },
        },
        Some(bytes),
    ))
}
pub fn save(root: &Path, state: &RuntimeState, expected: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let doc = Document {
        version: 1,
        transcription: Transcription {
            mode: match state.transcription.mode {
                Mode::Running => ModeDto::Running,
                Mode::Paused => ModeDto::Paused,
                Mode::Stopped => ModeDto::Stopped,
            },
        },
    };
    let bytes = serde_saphyr::to_string(&doc)
        .map_err(|e| e.to_string())?
        .into_bytes();
    let path = crate::metadata_fs::library_path(root, FILE, true)?;
    crate::metadata_fs::atomic_write_if_current(&path, &bytes, expected)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_runtime_value_has_a_typed_model_and_external_yaml_edits_are_preserved() {
        let root = tempfile::tempdir().unwrap();
        let (mut state, revision) = read(root.path()).unwrap();
        assert_eq!(state.transcription.mode, Mode::Running);
        assert!(!root.path().join("yaml").exists());
        state.transcription.mode = Mode::Paused;
        let revision = save(root.path(), &state, revision.as_deref()).unwrap();
        assert_eq!(read(root.path()).unwrap().0, state);
        let path = root.path().join(FILE);
        let edited = b"version: 1\ntranscription:\n  mode: stopped\n";
        std::fs::write(&path, edited).unwrap();
        assert!(save(root.path(), &state, Some(&revision)).is_err());
        assert_eq!(
            read(root.path()).unwrap().0.transcription.mode,
            Mode::Stopped
        );
        assert_eq!(std::fs::read(&path).unwrap(), edited);
        std::fs::write(
            &path,
            b"version: 1\ntranscription:\n  mode: paused\nunknown: true\n",
        )
        .unwrap();
        assert!(read(root.path()).is_err());
    }
}
