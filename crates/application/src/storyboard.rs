//! Six-frame video overview use case; sequential decoding, canceled on receiver drop.
use crate::ports::{VideoFrame, VideoPreviewSource};
use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
};
pub const FRAME_COUNT: usize = 6;
pub fn sample_times(duration: f64) -> Result<[f64; FRAME_COUNT], String> {
    if !duration.is_finite() || duration <= 0. {
        return Err("Videodauer ist nicht verfügbar".into());
    }
    Ok(std::array::from_fn(|i| {
        duration * (i as f64 + 0.5) / FRAME_COUNT as f64
    }))
}
pub enum Event {
    Started(f64),
    Frame(usize, Result<VideoFrame, String>),
    Failed(String),
    Done,
}
pub fn load(source: Arc<dyn VideoPreviewSource>, path: PathBuf) -> mpsc::Receiver<Event> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = (|| {
            let duration = source.duration(&path)?;
            let times = sample_times(duration)?;
            if tx.send(Event::Started(duration)).is_err() {
                return Ok(());
            }
            for (index, seconds) in times.into_iter().enumerate() {
                if tx
                    .send(Event::Frame(index, source.frame_at(&path, seconds)))
                    .is_err()
                {
                    return Ok(());
                }
            }
            Ok::<(), String>(())
        })();
        if let Err(error) = result {
            let _ = tx.send(Event::Failed(error));
        }
        let _ = tx.send(Event::Done);
    });
    rx
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn six_samples_are_even_and_inside_video() {
        assert_eq!(sample_times(120.).unwrap(), [10., 30., 50., 70., 90., 110.]);
        for d in [0., -1., f64::NAN, f64::INFINITY] {
            assert!(sample_times(d).is_err());
        }
        let short = sample_times(0.1).unwrap();
        assert!(short.windows(2).all(|v| v[0] < v[1]));
        assert!(short[5] < 0.1);
    }
}
