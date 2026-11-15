//! Source-bound transcription values shared by photos (comment audio), audio and video.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceVersion {
    bytes: u64,
    modified: String,
}
impl SourceVersion {
    pub fn new(bytes: u64, modified: String) -> Result<Self, String> {
        if modified.is_empty()
            || modified.len() > 64
            || !modified
                .bytes()
                .all(|b| b.is_ascii_digit() || b == b'.' || b == b'-')
        {
            return Err("Ungültige Medienrevision".into());
        }
        Ok(Self { bytes, modified })
    }
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
    pub fn modified(&self) -> &str {
        &self.modified
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranscriptStatus {
    Complete,
    NoSpeech,
    NoAudio,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    start_ms: u64,
    end_ms: u64,
    text: String,
}
impl Segment {
    pub fn new(start_ms: u64, end_ms: u64, text: String) -> Result<Self, String> {
        if end_ms < start_ms || text.len() > 65536 {
            return Err("Ungültiger Transkriptabschnitt".into());
        }
        Ok(Self {
            start_ms,
            end_ms,
            text,
        })
    }
    pub fn start_ms(&self) -> u64 {
        self.start_ms
    }
    pub fn end_ms(&self) -> u64 {
        self.end_ms
    }
    pub fn text(&self) -> &str {
        &self.text
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transcript {
    source: SourceVersion,
    status: TranscriptStatus,
    model: String,
    language: String,
    text: String,
    segments: Vec<Segment>,
}
impl Transcript {
    pub fn new(
        source: SourceVersion,
        status: TranscriptStatus,
        model: String,
        language: String,
        text: String,
        segments: Vec<Segment>,
    ) -> Result<Self, String> {
        if model.is_empty()
            || model.len() > 80
            || !model
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            || language.is_empty()
            || language.len() > 16
            || !language
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b == b'-')
        {
            return Err("Ungültige Transkript-Herkunft".into());
        }
        if text.len() > 2 * 1024 * 1024
            || segments.len() > 50000
            || segments.iter().map(|s| s.text.len()).sum::<usize>() > 2 * 1024 * 1024
        {
            return Err("Transkript überschreitet das Größenlimit".into());
        }
        if status != TranscriptStatus::Complete && (!text.is_empty() || !segments.is_empty()) {
            return Err("Leere Medien dürfen kein gesprochenes Transkript enthalten".into());
        }
        if status == TranscriptStatus::Complete && text.trim().is_empty() {
            return Err("Ein vollständiges Transkript benötigt Text".into());
        }
        if segments.windows(2).any(|s| s[0].start_ms > s[1].start_ms) {
            return Err("Transkriptabschnitte sind nicht zeitlich sortiert".into());
        }
        Ok(Self {
            source,
            status,
            model,
            language,
            text,
            segments,
        })
    }
    pub fn source(&self) -> &SourceVersion {
        &self.source
    }
    pub fn status(&self) -> TranscriptStatus {
        self.status
    }
    pub fn model(&self) -> &str {
        &self.model
    }
    pub fn language(&self) -> &str {
        &self.language
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_multiline_text_is_preserved_and_invalid_results_are_rejected() {
        let version = SourceVersion::new(100, "123.456".into()).unwrap();
        let text = "Er sagte: \"Grüße!\" # Erinnerung\n\n第二段";
        let t = Transcript::new(
            version.clone(),
            TranscriptStatus::Complete,
            "large-v3-turbo".into(),
            "de".into(),
            text.into(),
            vec![],
        )
        .unwrap();
        assert_eq!(t.text(), text);
        assert!(
            Transcript::new(
                version,
                TranscriptStatus::NoAudio,
                "tiny".into(),
                "en".into(),
                "wrong".into(),
                vec![]
            )
            .is_err()
        );
        assert!(Segment::new(4, 3, "x".into()).is_err());
    }
}
