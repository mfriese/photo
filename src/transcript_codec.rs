//! The same optional transcript payload is used inside every media YAML.
use crate::domain::transcript::{Segment, SourceVersion, Transcript, TranscriptStatus};
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TranscriptDto {
    status: String,
    model: String,
    language: String,
    source_bytes: u64,
    source_modified: String,
    text: String,
    #[serde(default)]
    segments: Vec<SegmentDto>,
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SegmentDto {
    start_ms: u64,
    end_ms: u64,
    text: String,
}
impl TranscriptDto {
    pub fn from_value(value: &Transcript) -> Self {
        Self {
            status: match value.status() {
                TranscriptStatus::Complete => "complete",
                TranscriptStatus::NoSpeech => "no_speech",
                TranscriptStatus::NoAudio => "no_audio",
            }
            .into(),
            model: value.model().into(),
            language: value.language().into(),
            source_bytes: value.source().bytes(),
            source_modified: value.source().modified().into(),
            text: value.text().into(),
            segments: value
                .segments()
                .iter()
                .map(|s| SegmentDto {
                    start_ms: s.start_ms(),
                    end_ms: s.end_ms(),
                    text: s.text().into(),
                })
                .collect(),
        }
    }
    pub fn value(&self) -> Result<Transcript, String> {
        let status = match self.status.as_str() {
            "complete" => TranscriptStatus::Complete,
            "no_speech" => TranscriptStatus::NoSpeech,
            "no_audio" => TranscriptStatus::NoAudio,
            _ => return Err("Unbekannter Transkriptstatus".into()),
        };
        Transcript::new(
            SourceVersion::new(self.source_bytes, self.source_modified.clone())?,
            status,
            self.model.clone(),
            self.language.clone(),
            self.text.clone(),
            self.segments
                .iter()
                .map(|s| Segment::new(s.start_ms, s.end_ms, s.text.clone()))
                .collect::<Result<_, _>>()?,
        )
    }
}
