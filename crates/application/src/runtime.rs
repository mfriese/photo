//! Small, typed library operating state, persisted independently of disposable indexes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimeState {
    pub transcription: TranscriptionState,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TranscriptionState {
    pub mode: crate::transcription::queue::Mode,
}
