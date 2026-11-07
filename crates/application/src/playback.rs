//! Playback use case: only video media can reach the native playback port.
use crate::{domain::Media, ports::VideoPlayer};
pub fn play(media: &Media, player: &dyn VideoPlayer) -> Result<(), String> {
    if !media.is_video() {
        return Err("Bitte ein Video auswählen".into());
    }
    player.play(media.path())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Player(AtomicUsize);
    impl VideoPlayer for Player {
        fn play(&self, _: &std::path::Path) -> Result<(), String> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }
    #[test]
    fn playback_rejects_images_and_dispatches_video() {
        let p = Player(AtomicUsize::new(0));
        assert!(
            play(
                &Media::from_path("photo.jpg".into(), String::new()).unwrap(),
                &p
            )
            .is_err()
        );
        play(
            &Media::from_path("movie.mp4".into(), String::new()).unwrap(),
            &p,
        )
        .unwrap();
        assert_eq!(p.0.load(Ordering::Relaxed), 1);
    }
}
