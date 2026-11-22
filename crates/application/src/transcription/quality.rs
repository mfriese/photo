//! Conservative detection of pathological ASR repetition, not text rewriting.
use crate::domain::transcript::Segment;
use std::collections::VecDeque;
const ERROR: &str = "Transkription wegen auffälliger Wiederholungsschleife verworfen. Bisheriger Text bleibt erhalten; Aufnahme prüfen und gegebenenfalls erneut transkribieren.";
#[derive(Default)]
pub struct RepetitionGuard {
    words: VecDeque<String>,
}
impl RepetitionGuard {
    /// Retains at most 48 words, including across segment/chunk boundaries.
    pub fn push_text(&mut self, text: &str) -> Result<(), String> {
        for word in text
            .split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
        {
            self.words.push_back(word.to_lowercase());
            if self.words.len() > 48 {
                self.words.pop_front();
            }
            for period in 1usize..=12 {
                let repeats = if period == 1 {
                    8
                } else {
                    4.max(16usize.div_ceil(period))
                };
                let length = period * repeats;
                if self.words.len() < length {
                    continue;
                }
                let start = self.words.len() - length;
                if (period..length)
                    .all(|offset| self.words[start + offset] == self.words[start + offset % period])
                {
                    return Err(ERROR.into());
                }
            }
        }
        Ok(())
    }
}
pub fn validate(segments: &[Segment]) -> Result<(), String> {
    let mut guard = RepetitionGuard::default();
    for segment in segments {
        guard.push_text(segment.text())?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_repetition_and_separated_references_are_preserved() {
        let mut guard = RepetitionGuard::default();
        guard
            .push_text("Das war sehr, sehr schön. Ja, ja. Wir gingen nach Hause.")
            .unwrap();
        guard
            .push_text("Am nächsten Tag gingen wir wieder nach Hause.")
            .unwrap();
    }
    #[test]
    fn loops_are_detected_despite_case_punctuation_and_segment_boundaries() {
        let mut guard = RepetitionGuard::default();
        for _ in 0..7 {
            guard.push_text("Danke!").unwrap();
        }
        assert!(guard.push_text("DANKE.").is_err());
        let mut guard = RepetitionGuard::default();
        for _ in 0..3 {
            guard
                .push_text("Vielen Dank für Ihre Aufmerksamkeit.")
                .unwrap();
        }
        assert!(
            guard
                .push_text("Vielen Dank für Ihre Aufmerksamkeit.")
                .is_err()
        );
    }
    #[test]
    fn state_is_bounded_and_a_new_recording_starts_clean() {
        let mut guard = RepetitionGuard::default();
        for index in 0..1000 {
            guard.push_text(&format!("Wort{index}")).unwrap();
        }
        assert_eq!(guard.words.len(), 48);
        let mut next = RepetitionGuard::default();
        next.push_text("Danke danke danke").unwrap();
    }
}
