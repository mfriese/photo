//! A cursor inside an existing, uncommitted audio take.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioCursor {
    seconds: f64,
}
impl AudioCursor {
    pub fn new(seconds: f64, duration: f64) -> Result<Self, String> {
        if !duration.is_finite()
            || duration <= 0.
            || !seconds.is_finite()
            || seconds < 0.
            || seconds > duration
        {
            return Err("Abspielposition muss innerhalb der vorhandenen Aufnahme liegen".into());
        }
        Ok(Self { seconds })
    }
    pub fn seconds(self) -> f64 {
        self.seconds
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_accepts_endpoints_and_rejects_nonfinite_or_outside_positions() {
        for second in [0., 2.5, 10.] {
            assert_eq!(AudioCursor::new(second, 10.).unwrap().seconds(), second);
        }
        for second in [-1., 11., f64::NAN, f64::INFINITY] {
            assert!(AudioCursor::new(second, 10.).is_err());
        }
        for duration in [0., -1., f64::NAN, f64::INFINITY] {
            assert!(AudioCursor::new(0., duration).is_err());
        }
    }
}
