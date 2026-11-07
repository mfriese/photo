//! Editing use case and session state, independent of UI and concrete adapters.
use crate::domain::{Media, commands::Command};
use crate::ports::OrientationWriter;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, mpsc},
};
#[derive(Default)]
pub struct EditingSession {
    rotations: HashMap<PathBuf, u8>,
    pending: Option<(PathBuf, mpsc::Receiver<Result<(), String>>)>,
}
pub struct SaveResult {
    pub path: PathBuf,
    pub result: Result<(), String>,
}
impl EditingSession {
    pub fn rotations(&self) -> &HashMap<PathBuf, u8> {
        &self.rotations
    }
    pub fn is_saving(&self) -> bool {
        self.pending.is_some()
    }
    pub fn execute(
        &mut self,
        command: Command,
        media: &Media,
        persist: bool,
        writer: Arc<dyn OrientationWriter>,
        wake: impl FnOnce() + Send + 'static,
    ) -> Result<(), String> {
        if command == Command::PlayVideo {
            return Err("Wiedergabe ist kein Bildbearbeitungsbefehl".into());
        }
        if self.is_saving() {
            return Err("Ein Speichervorgang läuft bereits".into());
        }
        if media.is_video() {
            return Err("Bildbearbeitung ist für Videos nicht verfügbar".into());
        }
        let turns = self
            .rotations
            .entry(media.path().to_path_buf())
            .or_default();
        *turns = command
            .apply(*turns)
            .ok_or("Dieser Befehl verändert keine Bildausrichtung")?;
        if persist && *turns != 0 {
            let path = media.path().to_path_buf();
            let amount = *turns;
            let (tx, rx) = mpsc::channel();
            self.pending = Some((path.clone(), rx));
            std::thread::spawn(move || {
                let _ = tx.send(writer.write_rotation(&path, amount));
                wake();
            });
        }
        Ok(())
    }
    pub fn poll(&mut self) -> Option<SaveResult> {
        let (path, rx) = self.pending.as_ref()?;
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Speicherauftrag wurde unerwartet beendet".into())
            }
        };
        let path = path.clone();
        if result.is_ok() {
            self.rotations.remove(&path);
        }
        self.pending = None;
        Some(SaveResult { path, result })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Writer(bool);
    impl OrientationWriter for Writer {
        fn write_rotation(&self, _: &std::path::Path, _: u8) -> Result<(), String> {
            if self.0 {
                Ok(())
            } else {
                Err("read only".into())
            }
        }
    }
    fn media() -> Media {
        Media::from_path("photo.jpg".into(), String::new()).unwrap()
    }
    fn wait(session: &mut EditingSession) -> SaveResult {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if let Some(result) = session.poll() {
                return result;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
    }
    #[test]
    fn successful_save_clears_session_rotation() {
        let mut session = EditingSession::default();
        let media = media();
        session
            .execute(
                Command::RotateRight,
                &media,
                true,
                Arc::new(Writer(true)),
                || {},
            )
            .unwrap();
        assert!(session.is_saving());
        assert!(wait(&mut session).result.is_ok());
        assert!(session.rotations().is_empty());
        assert!(!session.is_saving());
    }
    #[test]
    fn failure_keeps_rotation_and_allows_retry() {
        let mut session = EditingSession::default();
        let media = media();
        session
            .execute(
                Command::RotateLeft,
                &media,
                true,
                Arc::new(Writer(false)),
                || {},
            )
            .unwrap();
        assert!(wait(&mut session).result.is_err());
        assert_eq!(session.rotations()[media.path()], 3);
        session
            .execute(
                Command::ResetRotation,
                &media,
                false,
                Arc::new(Writer(false)),
                || {},
            )
            .unwrap();
        assert_eq!(session.rotations()[media.path()], 0);
    }
    #[test]
    fn viewing_mode_never_calls_writer_and_video_is_rejected() {
        let mut session = EditingSession::default();
        let mut media = media();
        session
            .execute(
                Command::RotateRight,
                &media,
                false,
                Arc::new(Writer(false)),
                || panic!("must not wake"),
            )
            .unwrap();
        assert!(!session.is_saving());
        assert_eq!(session.rotations()[media.path()], 1);
        media = Media::from_path("video.mp4".into(), String::new()).unwrap();
        assert!(
            session
                .execute(
                    Command::RotateRight,
                    &media,
                    true,
                    Arc::new(Writer(true)),
                    || {}
                )
                .is_err()
        );
    }
}
