//! Cooperative I/O admission: audio preparation has precedence over gallery work.
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
#[derive(Default)]
pub struct WorkPriority {
    foreground: AtomicUsize,
}
pub struct AudioWork {
    priority: Arc<WorkPriority>,
}
impl WorkPriority {
    pub fn audio(self: &Arc<Self>) -> AudioWork {
        self.foreground.fetch_add(1, Ordering::SeqCst);
        AudioWork {
            priority: self.clone(),
        }
    }
    pub fn audio_active(&self) -> bool {
        self.foreground.load(Ordering::SeqCst) != 0
    }
    pub fn wait_background(&self, cancel: &AtomicBool) -> bool {
        while self.audio_active() {
            if cancel.load(Ordering::Relaxed) {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        !cancel.load(Ordering::Relaxed)
    }
}
impl Drop for AudioWork {
    fn drop(&mut self) {
        self.priority.foreground.fetch_sub(1, Ordering::SeqCst);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_audio_work_blocks_background_and_cancellation_does_not_wait() {
        let priority = Arc::new(WorkPriority::default());
        let first = priority.audio();
        let second = priority.audio();
        drop(first);
        assert!(priority.audio_active());
        assert!(!priority.wait_background(&AtomicBool::new(true)));
        drop(second);
        assert!(!priority.audio_active());
        assert!(priority.wait_background(&AtomicBool::new(false)));
    }
}
